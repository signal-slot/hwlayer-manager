//! The scenarios. Each opens its own session, prints records and returns a
//! verdict; an `Err` is a failure that stopped the scenario.

use std::collections::HashMap;
use std::time::Instant;

use composition_core::limits::MAX_LAYERS;
use composition_core::planner;
use composition_core::{
    BlendMode, BufferId, BufferInfo, BufferState, DisplayEvent, Error, Layer, Modifier,
    PixelFormat, PlanError, Rect, Size, Transform,
};

use crate::draw::{self, colour};
use crate::record::{list, Record};
use crate::session::{describe, planes_of, rect, Options, Session, Shown, LATCH_TIMEOUT};
use crate::stats::{micros, missed_vblanks, period_us, summary_fields, Usage};

/// The result of a scenario.
pub struct Outcome {
    /// Whether every check passed.
    pub pass: bool,
    /// One-line explanation.
    pub reason: String,
}

impl Outcome {
    fn new(failures: &[String], ok: String) -> Self {
        if failures.is_empty() {
            Self {
                pass: true,
                reason: ok,
            }
        } else {
            Self {
                pass: false,
                reason: failures.join("; "),
            }
        }
    }
}

/// Default frame count of `move`.
pub const MOVE_FRAMES: u32 = 240;
/// Default frame count of `rotate`.
pub const ROTATE_FRAMES: u32 = 600;

const XRGB: PixelFormat = PixelFormat::XRGB8888;
const ARGB: PixelFormat = PixelFormat::ARGB8888;

fn at(x: u32, y: u32, size: Size) -> Rect {
    Rect::new(x as i32, y as i32, size.width, size.height)
}

fn whole(size: Size) -> Rect {
    Rect::new(0, 0, size.width, size.height)
}

/// A buffer of `size` filled with `fill`, framed with `edge`.
fn framed(
    s: &mut Session,
    size: Size,
    format: PixelFormat,
    fill: u32,
    edge: u32,
) -> Result<BufferId, String> {
    s.dumb(size, format, |pixels, pitch| {
        draw::fill(pixels, pitch, whole(size), fill);
        draw::border(pixels, pitch, whole(size), 8, edge);
    })
}

fn print_commit(step: &str, shown: &Shown) {
    Record::new("commit")
        .field("step", step)
        .field("submission", shown.submission.0)
        .field("validate_us", shown.validate_us)
        .field("commit_us", shown.commit_us)
        .field("latency_us", shown.latency_us)
        .emit();
}

fn print_resources(scenario: &str, usage: &Usage) {
    usage
        .fields_since(Record::new("resources").field("scenario", scenario))
        .emit();
}

/// Checks that a plan puts each layer on its own plane and that the stacking
/// follows the layers' z-order. Returns the failures.
fn check_distinct_and_order(step: &str, layers: &[Layer], shown: &Shown) -> Vec<String> {
    let mut failures = Vec::new();
    let assignments = shown.plan.assignments();
    if assignments.len() != layers.len() {
        failures.push(format!(
            "{step}: {} of {} layers on planes",
            assignments.len(),
            layers.len()
        ));
    }
    let mut planes = planes_of(&shown.plan);
    planes.sort_unstable();
    planes.dedup();
    if planes.len() != assignments.len() {
        failures.push(format!("{step}: layers share a plane"));
    }
    for a in assignments {
        for b in assignments {
            let (la, lb) = (&layers[a.layer], &layers[b.layer]);
            if la.z_order < lb.z_order && a.zpos >= b.zpos {
                failures.push(format!(
                    "{step}: layer {} (z {}) not below layer {} (z {}): zpos {} vs {}",
                    a.layer, la.z_order, b.layer, lb.z_order, a.zpos, b.zpos
                ));
            }
        }
    }
    failures
}

/// `two-planes`: two separately allocated buffers on two planes, held.
pub fn two_planes(o: &Options) -> Result<Outcome, String> {
    let mut s = Session::open(o, "two-planes")?;
    let usage = Usage::now();
    let size = s.size;
    let (w, h) = (size.width, size.height);
    let top_size = Size::new(w / 2, h / 2);
    let bottom = framed(&mut s, size, XRGB, colour::BLUE, colour::WHITE)?;
    let top = framed(&mut s, top_size, ARGB, colour::RED, colour::YELLOW)?;
    let layers = [
        Layer::new(bottom, whole(size), whole(size), 0),
        Layer::new(top, whole(top_size), at(w / 4, h / 4, top_size), 1),
    ];
    Record::new("step")
        .field("label", "two-planes")
        .field(
            "expect",
            "blue full screen with white edge, red rectangle with yellow edge on top in the middle",
        )
        .emit();
    let shown = s.present(&layers)?;
    s.print_plan("two-planes", &layers, &shown.plan);
    print_commit("two-planes", &shown);
    let failures = check_distinct_and_order("two-planes", &layers, &shown);
    s.hold("two-planes", o.seconds)?;
    print_resources("two-planes", &usage);
    Ok(Outcome::new(
        &failures,
        format!(
            "2 buffers on planes {} (bottom,top)",
            list(planes_of(&shown.plan))
        ),
    ))
}

/// `zorder-alpha`: swap the stacking, then plane alpha, then pixel alpha.
pub fn zorder_alpha(o: &Options) -> Result<Outcome, String> {
    let mut s = Session::open(o, "zorder-alpha")?;
    let usage = Usage::now();
    let size = s.size;
    let (w, h) = (size.width, size.height);
    let half = Size::new(w / 2, h / 2);
    let a = framed(&mut s, half, ARGB, colour::RED, colour::WHITE)?;
    let b = framed(&mut s, half, ARGB, colour::GREEN, colour::WHITE)?;
    // Left half opaque red, right half red at half pixel alpha, premultiplied.
    let a_alpha = s.dumb(half, ARGB, |pixels, pitch| {
        let left = Rect::new(0, 0, half.width / 2, half.height);
        let right = Rect::new(
            (half.width / 2) as i32,
            0,
            half.width - half.width / 2,
            half.height,
        );
        draw::fill(pixels, pitch, left, colour::RED);
        draw::fill(pixels, pitch, right, colour::RED_HALF_PREMULTIPLIED);
    })?;
    let a_at = at(w / 8, h / 8, half);
    let b_at = at(3 * w / 8, 3 * h / 8, half);
    let layer = |buffer, dst, z| Layer::new(buffer, whole(half), dst, z);

    let steps: [(&str, &str, [Layer; 2]); 4] = [
        (
            "a-over-b",
            "red (upper left) above green (lower right): overlap is red",
            [layer(a, a_at, 1), layer(b, b_at, 0)],
        ),
        (
            "b-over-a",
            "green above red: overlap is green",
            [layer(a, a_at, 0), layer(b, b_at, 1)],
        ),
        (
            "plane-alpha-50",
            "red at 50% plane alpha above green: overlap is a red/green mix, rest of red is dark red",
            [
                Layer {
                    alpha: 0x8000,
                    ..layer(a, a_at, 1)
                },
                layer(b, b_at, 0),
            ],
        ),
        (
            "pixel-alpha-premultiplied",
            "left half of red opaque, right half 50% pixel alpha over green and black",
            [
                Layer {
                    blend: BlendMode::Premultiplied,
                    ..layer(a_alpha, a_at, 1)
                },
                layer(b, b_at, 0),
            ],
        ),
    ];
    let mut failures = Vec::new();
    let mut done = Vec::new();
    for (label, expect, layers) in steps {
        Record::new("step")
            .field("label", label)
            .field("expect", expect)
            .emit();
        match s.present(&layers) {
            Ok(shown) => {
                s.print_plan(label, &layers, &shown.plan);
                print_commit(label, &shown);
                let step_failures = check_distinct_and_order(label, &layers, &shown);
                if step_failures.is_empty() {
                    done.push(label);
                }
                failures.extend(step_failures);
                s.hold(label, o.seconds)?;
            }
            Err(error) => {
                Record::new("step_result")
                    .field("label", label)
                    .field("result", "refused")
                    .field("error", &error)
                    .emit();
                failures.push(format!("{label}: {error}"));
            }
        }
    }
    print_resources("zorder-alpha", &usage);
    Ok(Outcome::new(&failures, format!("steps {}", list(done))))
}

/// `move`: the top plane moves, the bottom buffer is never touched.
pub fn move_top(o: &Options) -> Result<Outcome, String> {
    let mut s = Session::open(o, "move")?;
    let usage = Usage::now();
    let frames = o.frames.unwrap_or(MOVE_FRAMES).max(2);
    let size = s.size;
    let (w, h) = (size.width, size.height);
    let bottom = s.dumb(size, XRGB, |pixels, pitch| {
        draw::checker(pixels, pitch, whole(size), 64, colour::GREY, colour::BLACK);
        draw::border(pixels, pitch, whole(size), 8, colour::WHITE);
    })?;
    let top_size = Size::new((w / 4).max(16), (h / 4).max(16));
    let top = framed(&mut s, top_size, ARGB, colour::YELLOW, colour::RED)?;
    let checksum = |s: &mut Session| {
        s.inspect(bottom, |pixels, pitch| {
            draw::checksum(pixels, pitch, size.width, size.height)
        })
    };
    let before = checksum(&mut s)?;
    Record::new("step")
        .field("label", "move")
        .field("frames", frames)
        .field(
            "expect",
            "yellow rectangle moves left to right over a still checkerboard",
        )
        .emit();

    let span = w.saturating_sub(top_size.width);
    let y = (h - top_size.height) / 2;
    let mut failures = Vec::new();
    let mut bottom_plane = None;
    let mut latencies = Vec::new();
    let mut intervals = Vec::new();
    let mut last_latch: Option<Instant> = None;
    let mut bottom_released = 0u32;
    for n in 0..frames {
        let x = (u64::from(span) * u64::from(n) / u64::from(frames - 1)) as u32;
        let layers = [
            Layer::new(bottom, whole(size), whole(size), 0),
            Layer::new(top, whole(top_size), at(x, y, top_size), 1),
        ];
        let shown = match s.present(&layers) {
            Ok(shown) => shown,
            Err(error) => {
                failures.push(format!("frame {n}: {error}"));
                break;
            }
        };
        let now = Instant::now();
        if let Some(last) = last_latch {
            intervals.push(micros(now - last));
        }
        last_latch = Some(now);
        if n > 0 {
            latencies.push(shown.latency_us);
        }
        bottom_released += shown
            .events
            .iter()
            .filter(|e| matches!(e, DisplayEvent::Released { buffer } if *buffer == bottom))
            .count() as u32;
        let plane_of = |layer: usize| {
            shown
                .plan
                .assignments()
                .iter()
                .find(|a| a.layer == layer)
                .map(|a| a.plane.0)
        };
        let (bp, tp) = (plane_of(0), plane_of(1));
        Record::new("frame")
            .field("n", n)
            .field("submission", shown.submission.0)
            .field_opt("bottom_plane", bp)
            .field_opt("top_plane", tp)
            .field("bottom_buffer", bottom.0)
            .field("top_buffer", top.0)
            .field("top_x", x)
            .field("validate_us", shown.validate_us)
            .field("latency_us", shown.latency_us)
            .emit();
        match (bp, tp) {
            (Some(bp), Some(tp)) if bp != tp => {
                if *bottom_plane.get_or_insert(bp) != bp {
                    failures.push(format!("frame {n}: bottom moved to plane {bp}"));
                }
            }
            _ => failures.push(format!("frame {n}: not on two distinct planes")),
        }
        if s.engine.buffer_state(bottom) != Some(BufferState::Scanning) {
            failures.push(format!("frame {n}: bottom buffer not scanning"));
        }
        if failures.len() > 8 {
            break;
        }
    }
    let after = checksum(&mut s)?;
    let mut record = Record::new("move_result")
        .field("frames", frames)
        .field("bottom_checksum_before", format!("{before:016x}"))
        .field("bottom_checksum_after", format!("{after:016x}"))
        .field("bottom_released", bottom_released);
    record = summary_fields(record, "latency", &latencies);
    record = summary_fields(record, "interval", &intervals);
    record.emit();
    if before != after {
        failures.push("bottom buffer content changed".into());
    }
    if bottom_released != 0 {
        failures.push(format!("bottom buffer released {bottom_released} times"));
    }
    print_resources("move", &usage);
    Ok(Outcome::new(
        &failures,
        format!(
            "{frames} frames, bottom buffer {} unchanged on plane {}, top on its own plane",
            bottom.0,
            bottom_plane.map_or("-".into(), |p| p.to_string())
        ),
    ))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Expect {
    Accept,
    PlannerRefuse,
    Any,
}

impl Expect {
    fn name(self) -> &'static str {
        match self {
            Expect::Accept => "accept",
            Expect::PlannerRefuse => "planner-refuse",
            Expect::Any => "any",
        }
    }
}

/// `combos`: which layer combinations the planner and `TEST_ONLY` take.
/// Nothing is committed.
pub fn combos(o: &Options) -> Result<Outcome, String> {
    let mut s = Session::open(o, "combos")?;
    let usage = Usage::now();
    let size = s.size;
    let (w, h) = (size.width, size.height);
    let plane_count = s.planes_on_output().len();
    let small = Size::new((w / 8).max(16), (h / 8).max(16));
    let bg = framed(&mut s, size, XRGB, colour::BLUE, colour::WHITE)?;
    let mut smalls = Vec::new();
    for _ in 0..plane_count.min(MAX_LAYERS) {
        smalls.push(framed(&mut s, small, ARGB, colour::RED, colour::WHITE)?);
    }
    let bg_layer = Layer::new(bg, whole(size), whole(size), 0);
    let small_layer = |index: usize, count: usize| {
        let step_x = w / (count as u32 + 2);
        let step_y = h / (count as u32 + 2);
        Layer::new(
            smalls[index],
            whole(small),
            at(
                step_x * (index as u32 + 1),
                step_y * (index as u32 + 1),
                small,
            ),
            index as i32 + 1,
        )
    };

    let mut combos: Vec<(String, Expect, Vec<Layer>)> = Vec::new();
    combos.push(("1-layer-full".into(), Expect::Accept, vec![bg_layer]));
    for count in 2..=plane_count.min(MAX_LAYERS) {
        let mut layers = vec![bg_layer];
        layers.extend((0..count - 1).map(|i| small_layer(i, count - 1)));
        combos.push((format!("{count}-layers"), Expect::Any, layers));
    }
    if plane_count < MAX_LAYERS && !smalls.is_empty() {
        // One more layer than there are planes: the background plus one
        // small layer per plane.
        let mut layers = vec![bg_layer];
        layers.extend((0..smalls.len()).map(|i| small_layer(i, smalls.len())));
        combos.push((
            format!("{}-layers-beyond-planes", layers.len()),
            Expect::PlannerRefuse,
            layers,
        ));
    }
    if let Some(&first) = smalls.first() {
        let one = Layer::new(first, whole(small), at(w / 4, h / 4, small), 1);
        let pair = |layer: Layer| vec![bg_layer, layer];
        combos.push(("1-layer-partial".into(), Expect::Any, vec![one]));
        combos.push((
            "plane-alpha-50".into(),
            Expect::Any,
            pair(Layer {
                alpha: 0x8000,
                ..one
            }),
        ));
        combos.push((
            "blend-premultiplied".into(),
            Expect::Any,
            pair(Layer {
                blend: BlendMode::Premultiplied,
                ..one
            }),
        ));
        combos.push((
            "blend-coverage".into(),
            Expect::Any,
            pair(Layer {
                blend: BlendMode::Coverage,
                ..one
            }),
        ));
        let double = Size::new(small.width * 2, small.height * 2);
        combos.push((
            "upscale-2x".into(),
            Expect::Any,
            pair(Layer {
                destination: at(w / 4, h / 4, double),
                ..one
            }),
        ));
        let halved = Size::new(small.width / 2, small.height / 2);
        combos.push((
            "downscale-2x".into(),
            Expect::Any,
            pair(Layer {
                destination: at(w / 4, h / 4, halved),
                ..one
            }),
        ));
        let turned = Size::new(small.height, small.width);
        combos.push((
            "rotate-90".into(),
            Expect::Any,
            pair(Layer {
                destination: at(w / 4, h / 4, turned),
                transform: Transform::Rotate90,
                ..one
            }),
        ));
        combos.push((
            "off-screen".into(),
            Expect::Any,
            pair(Layer {
                destination: Rect::new(w as i32 + 16, 0, small.width, small.height),
                ..one
            }),
        ));
        combos.push((
            "partly-off-screen".into(),
            Expect::Any,
            pair(Layer {
                destination: Rect::new(
                    (w - small.width / 2) as i32,
                    (h / 2) as i32,
                    small.width,
                    small.height,
                ),
                ..one
            }),
        ));
    }

    // YCbCr through DMA-BUF import, unscaled and scaled.
    let video = Size::new((w / 4) & !1, (h / 4) & !1);
    match s.nv12(video, (81, 90, 240)) {
        Ok(nv12) => {
            let layer = Layer::new(nv12, whole(video), at(w / 2, h / 2, video), 1);
            combos.push(("nv12".into(), Expect::Any, vec![bg_layer, layer]));
            let doubled = Size::new(video.width * 2, video.height * 2);
            combos.push((
                "nv12-upscale-2x".into(),
                Expect::Any,
                vec![
                    bg_layer,
                    Layer {
                        destination: at(w / 4, h / 4, doubled),
                        ..layer
                    },
                ],
            ));
        }
        Err(error) => Record::new("combo")
            .field("name", "nv12")
            .field("result", "skipped")
            .field("error", error)
            .emit(),
    }

    // A format none of the output's planes lists.
    let planes = s.planes_on_output();
    let listed = |format: PixelFormat| {
        planes
            .iter()
            .any(|p| p.formats.iter().any(|f| f.format == format))
    };
    let candidate = [
        PixelFormat::RGB565,
        PixelFormat::XBGR8888,
        PixelFormat::ABGR8888,
    ]
    .into_iter()
    .find(|f| !listed(*f));
    let odd_size = Size::new(small.width, small.height);
    let odd_format = candidate.unwrap_or(PixelFormat::fourcc(b"HWLX"));
    let odd_info = BufferInfo {
        size: odd_size,
        format: odd_format,
        modifier: Modifier::LINEAR,
    };
    // Linux refuses a framebuffer in a format no plane of the device lists,
    // so the allocation may fail; the planner must still see the layer.
    let odd = match candidate {
        Some(format) => match s.dumb(odd_size, format, |pixels, _| pixels.fill(0x80)) {
            Ok(id) => id,
            Err(error) => {
                Record::new("alloc")
                    .field("name", "unsupported-format")
                    .field("format", crate::session::fourcc(format))
                    .field("result", "register-only")
                    .field("error", error)
                    .emit();
                s.register_only(odd_info)?
            }
        },
        None => s.register_only(odd_info)?,
    };
    combos.push((
        format!("unsupported-format-{}", crate::session::fourcc(odd_format)),
        Expect::PlannerRefuse,
        vec![
            bg_layer,
            Layer::new(odd, whole(odd_size), at(w / 4, h / 4, odd_size), 1),
        ],
    ));

    let mut failures = Vec::new();
    let (mut accepted, mut planner_refused, mut test_refused, mut errors) = (0, 0, 0, 0);
    for (name, expect, layers) in &combos {
        let frame = s.frame(layers);
        let planned = planner::plan(&frame, s.engine.capabilities(), s.infos());
        let mut record = Record::new("combo")
            .field("name", name)
            .field("layers", layers.len())
            .field("expect", expect.name());
        let outcome;
        match planned {
            Err(error) => {
                planner_refused += 1;
                outcome = Expect::PlannerRefuse;
                record = record
                    .field("planner", "refuse")
                    .field("planner_reason", format!("{error:?}"))
                    .field("test_only", "not-run");
            }
            Ok(plan) => {
                record = record
                    .field("planner", "accept")
                    .field("planner_planes", list(planes_of(&plan)));
                let start = Instant::now();
                let validated = s.engine.validate(&frame);
                record = record.field("validate_us", micros(start.elapsed()));
                match validated {
                    Ok(plan) => {
                        accepted += 1;
                        outcome = Expect::Accept;
                        record = record
                            .field("test_only", "accept")
                            .field("planes", list(planes_of(&plan)))
                            .field("assigned", plan.assignments().len());
                        for a in plan.assignments() {
                            record = record.field(
                                &format!("layer{}", a.layer),
                                format!(
                                    "plane:{},zpos:{},dst:{}",
                                    a.plane.0,
                                    a.zpos,
                                    rect(a.destination)
                                ),
                            );
                        }
                    }
                    Err(Error::Plan(PlanError::BackendRejected { candidates })) => {
                        test_refused += 1;
                        outcome = Expect::Any;
                        record = record
                            .field("test_only", "refuse")
                            .field("candidates", candidates);
                    }
                    Err(error) => {
                        errors += 1;
                        outcome = Expect::Any;
                        record = record
                            .field("test_only", "error")
                            .field("error", describe(&error));
                        failures.push(format!("{name}: {}", describe(&error)));
                    }
                }
            }
        }
        let met = *expect == Expect::Any || *expect == outcome;
        record.field("as_expected", met).emit();
        if !met {
            failures.push(format!("{name}: expected {}", expect.name()));
        }
    }
    if accepted == 0 || planner_refused + test_refused == 0 {
        failures.push("no differentiation between accepted and refused frames".into());
    }
    Record::new("combos_result")
        .field("combos", combos.len())
        .field("accepted", accepted)
        .field("planner_refused", planner_refused)
        .field("test_only_refused", test_refused)
        .field("errors", errors)
        .field("planes_on_output", plane_count)
        .emit();
    print_resources("combos", &usage);
    Ok(Outcome::new(
        &failures,
        format!(
            "{accepted} accepted, {planner_refused} refused by the planner, \
             {test_refused} refused by TEST_ONLY, nothing committed"
        ),
    ))
}

/// Where a rotated buffer is in its life, as the harness sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Life {
    /// Released (or never shown): may be drawn.
    Idle,
    /// Committed, not latched yet.
    Submitted(u64),
    /// Latched; on screen until a later frame replaces it.
    Latched(u64),
    /// Its submission was dropped; the release follows.
    Dropped(u64),
}

struct Rotated {
    id: BufferId,
    life: Life,
    band: Option<Rect>,
}

#[derive(Default)]
struct RotateState {
    submissions: HashMap<u64, (Instant, usize, Vec<BufferId>)>,
    where_is: HashMap<BufferId, (usize, usize)>,
    latest_presented: u64,
    last_latch: Option<(Instant, usize)>,
    latencies: Vec<u64>,
    intervals: Vec<u64>,
    missed: u64,
    presented: u32,
    dropped: u32,
    releases: u32,
    violations: u32,
    latch_timeouts: u32,
    lost: bool,
}

impl RotateState {
    fn violation(&mut self, what: String) {
        self.violations += 1;
        if self.violations <= 16 {
            Record::new("violation").field("what", what).emit();
        }
    }

    fn on_event(&mut self, chains: &mut [Vec<Rotated>; 2], event: DisplayEvent, period: u64) {
        let now = Instant::now();
        match event {
            DisplayEvent::Presented { submission, .. } => {
                let s = submission.0;
                let Some((submitted, frame, buffers)) = self.submissions.get(&s).cloned() else {
                    self.violation(format!("presented unknown submission {s}"));
                    return;
                };
                self.presented += 1;
                self.latest_presented = self.latest_presented.max(s);
                // The first frame includes the modeset: not representative.
                if frame > 0 {
                    self.latencies.push(micros(now - submitted));
                }
                if let Some((last, last_frame)) = self.last_latch {
                    if last_frame > 0 {
                        let interval = micros(now - last);
                        self.intervals.push(interval);
                        self.missed += missed_vblanks(interval, period);
                    }
                }
                self.last_latch = Some((now, frame));
                for buffer in buffers {
                    let (c, i) = self.where_is[&buffer];
                    let life = chains[c][i].life;
                    if life == Life::Submitted(s) {
                        chains[c][i].life = Life::Latched(s);
                    } else {
                        self.violation(format!(
                            "buffer {} latched in submission {s} while {life:?}",
                            buffer.0
                        ));
                    }
                }
            }
            DisplayEvent::Dropped { submission, .. } => {
                let s = submission.0;
                self.dropped += 1;
                let buffers = self
                    .submissions
                    .get(&s)
                    .map(|(_, _, b)| b.clone())
                    .unwrap_or_default();
                for buffer in buffers {
                    let (c, i) = self.where_is[&buffer];
                    if chains[c][i].life == Life::Submitted(s) {
                        chains[c][i].life = Life::Dropped(s);
                    } else {
                        let life = chains[c][i].life;
                        self.violation(format!(
                            "buffer {} dropped with submission {s} while {life:?}",
                            buffer.0
                        ));
                    }
                }
            }
            DisplayEvent::Released { buffer } => {
                self.releases += 1;
                let Some(&(c, i)) = self.where_is.get(&buffer) else {
                    self.violation(format!("released unknown buffer {}", buffer.0));
                    return;
                };
                match chains[c][i].life {
                    Life::Latched(s) if s < self.latest_presented => {
                        chains[c][i].life = Life::Idle;
                    }
                    Life::Dropped(_) => chains[c][i].life = Life::Idle,
                    life => self.violation(format!(
                        "buffer {} released while {life:?} (latest presented {})",
                        buffer.0, self.latest_presented
                    )),
                }
            }
            DisplayEvent::LatchTimeout { .. } => self.latch_timeouts += 1,
            DisplayEvent::DeviceLost => self.lost = true,
            DisplayEvent::CapabilitiesChanged => {}
        }
    }
}

/// `rotate`: K buffers per plane, redrawn only after their release.
pub fn rotate(o: &Options) -> Result<Outcome, String> {
    let mut s = Session::open(o, "rotate")?;
    let usage = Usage::now();
    let frames = o.frames.unwrap_or(ROTATE_FRAMES).max(2);
    let k = o.buffers as usize;
    let size = s.size;
    let (w, h) = (size.width, size.height);
    let top_size = Size::new(w / 2, h / 2);
    let top_at = at(w / 4, h / 4, top_size);
    let sizes = [size, top_size];
    let bases = [colour::BLUE, colour::RED];
    let formats = [XRGB, ARGB];
    let period = period_us(s.refresh_mhz).unwrap_or(0);

    let mut state = RotateState::default();
    let mut chains: [Vec<Rotated>; 2] = [Vec::new(), Vec::new()];
    for c in 0..2 {
        for i in 0..k {
            let (sz, base) = (sizes[c], bases[c]);
            let id = s.dumb(sz, formats[c], |pixels, pitch| {
                draw::fill(pixels, pitch, whole(sz), base)
            })?;
            state.where_is.insert(id, (c, i));
            chains[c].push(Rotated {
                id,
                life: Life::Idle,
                band: None,
            });
        }
    }
    Record::new("step")
        .field("label", "rotate")
        .field("frames", frames)
        .field("buffers_per_plane", k)
        .field("period_us", period)
        .field(
            "expect",
            "white bars sweep across the blue background and the red rectangle",
        )
        .emit();

    let mut validate_us = Vec::new();
    let mut commit_us = Vec::new();
    let mut overwrite_avoided = 0u32;
    let mut assignment_changes = 0u32;
    let mut first_planes: Option<Vec<u32>> = None;
    let mut error: Option<String> = None;

    'frames: for n in 0..frames as usize {
        let mut current = [BufferId(0); 2];
        for c in 0..2 {
            let i = n % k;
            let id = chains[c][i].id;
            // Wait for the release if it has not come yet; never draw into a
            // buffer the display may still read.
            let deadline = Instant::now() + LATCH_TIMEOUT;
            while chains[c][i].life != Life::Idle
                || s.engine.buffer_state(id) != Some(BufferState::Ready)
            {
                if Instant::now() >= deadline {
                    overwrite_avoided += 1;
                    error = Some(format!(
                        "frame {n}: buffer {} not released in time ({:?}, {:?})",
                        id.0,
                        chains[c][i].life,
                        s.engine.buffer_state(id)
                    ));
                    break 'frames;
                }
                s.wait(LATCH_TIMEOUT / 10)?;
                for event in s.drain()? {
                    state.on_event(&mut chains, event, period);
                }
            }
            let sz = sizes[c];
            let base = bases[c];
            let band_w = 32.min(sz.width);
            let x = ((n as u32 * 8) % (sz.width - band_w + 1)) as i32;
            let band = Rect::new(x, 0, band_w, sz.height);
            let old = chains[c][i].band;
            s.redraw(id, |pixels, pitch| {
                if let Some(old) = old {
                    draw::fill(pixels, pitch, old, base);
                }
                draw::fill(pixels, pitch, band, colour::WHITE);
            })?;
            chains[c][i].band = Some(band);
            current[c] = id;
        }
        let layers = [
            Layer::new(current[0], whole(size), whole(size), 0),
            Layer::new(current[1], whole(top_size), top_at, 1),
        ];
        let frame = s.frame(&layers);
        let start = Instant::now();
        let plan = match s.engine.validate(&frame) {
            Ok(plan) => plan,
            Err(e) => {
                error = Some(format!("frame {n}: validate: {}", describe(&e)));
                break;
            }
        };
        validate_us.push(micros(start.elapsed()));
        let planes = planes_of(&plan);
        match &first_planes {
            None => {
                s.print_plan("rotate", &layers, &plan);
                first_planes = Some(planes.clone());
            }
            Some(first) if *first != planes => assignment_changes += 1,
            _ => {}
        }
        let submitted = Instant::now();
        let submission = match s.engine.commit(&frame, &plan) {
            Ok(id) => id,
            Err(e) => {
                error = Some(format!("frame {n}: commit: {}", describe(&e)));
                break;
            }
        };
        commit_us.push(micros(submitted.elapsed()));
        state
            .submissions
            .insert(submission.0, (submitted, n, current.to_vec()));
        for (c, id) in current.iter().enumerate() {
            chains[c][n % k].life = Life::Submitted(submission.0);
            debug_assert_eq!(chains[c][n % k].id, *id);
        }
        // Wait for this frame's latch before preparing the next one.
        let deadline = Instant::now() + LATCH_TIMEOUT;
        while state.latest_presented < submission.0 {
            for event in s.drain()? {
                state.on_event(&mut chains, event, period);
            }
            if state.lost {
                error = Some("device lost".into());
                break 'frames;
            }
            if state.latest_presented >= submission.0 {
                break;
            }
            let now = Instant::now();
            if now >= deadline {
                state.latch_timeouts += 1;
                error = Some(format!(
                    "frame {n}: submission {} not latched",
                    submission.0
                ));
                break 'frames;
            }
            s.wait(deadline - now)?;
        }
        if (n + 1) % 100 == 0 {
            Record::new("progress")
                .field("frames", n + 1)
                .field("presented", state.presented)
                .field("releases", state.releases)
                .field("violations", state.violations)
                .emit();
        }
    }
    // Late events, if any.
    for event in s.drain()? {
        state.on_event(&mut chains, event, period);
    }

    let mut record = Record::new("rotate_result")
        .field("frames", frames)
        .field("buffers_per_plane", k)
        .field("presented", state.presented)
        .field("dropped", state.dropped)
        .field("missed_vblanks", state.missed)
        .field("latch_timeouts", state.latch_timeouts)
        .field("releases", state.releases)
        .field("order_violations", state.violations)
        .field("overwrite_avoided", overwrite_avoided)
        .field("assignment_changes", assignment_changes)
        .field(
            "planes",
            first_planes.as_ref().map_or("-".into(), |p| list(p.iter())),
        )
        .field("refresh_mhz", s.refresh_mhz)
        .field("period_us", period)
        .field("warmup_frames", 1);
    record = summary_fields(record, "latency", &state.latencies);
    record = summary_fields(record, "interval", &state.intervals);
    record = summary_fields(record, "validate", &validate_us);
    record = summary_fields(record, "commit", &commit_us);
    record.emit();
    print_resources("rotate", &usage);

    let mut failures = Vec::new();
    if let Some(error) = error {
        failures.push(error);
    }
    if state.presented != frames {
        failures.push(format!("{} of {frames} frames presented", state.presented));
    }
    if state.violations > 0 {
        failures.push(format!("{} lifecycle order violations", state.violations));
    }
    // Every latch after the first releases the two buffers it replaced.
    let expected_releases = 2 * state.presented.saturating_sub(1) + 2 * state.dropped;
    if state.releases != expected_releases {
        failures.push(format!(
            "{} releases, expected {expected_releases}",
            state.releases
        ));
    }
    if first_planes
        .as_ref()
        .is_some_and(|p| p.len() != 2 || p[0] == p[1])
    {
        failures.push("not on two distinct planes".into());
    }
    Ok(Outcome::new(
        &failures,
        format!(
            "{frames} frames, {k} buffers per plane, {} releases in order, \
             {} dropped, {} missed vblanks",
            state.releases, state.dropped, state.missed
        ),
    ))
}
