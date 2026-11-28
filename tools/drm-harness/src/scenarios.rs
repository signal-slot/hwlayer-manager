//! The scenarios. Each opens its own session, prints records and returns a
//! verdict; an `Err` is a failure that stopped the scenario.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use composition_core::limits::MAX_LAYERS;
use composition_core::planner;
use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, BufferState, DisplayEvent, Error, Layer,
    Modifier, PixelFormat, PlanError, Rect, Size, Transform,
};

use crate::draw::{colour, Op};
use crate::record::{list, Record};
use crate::session::{describe, planes_of, rect, Options, Session, Shown, LATCH_TIMEOUT};
use crate::stats::{micros, missed_vblanks, period_us, summary_fields, Usage};

/// The result of a scenario.
pub struct Outcome {
    /// Whether every check passed.
    pub pass: bool,
    /// One-line explanation.
    pub reason: String,
    /// Whether a stop (SIGTERM, SIGINT) ended the scenario at its hold, after
    /// the checks that were made. `pass` still says what they found.
    pub stopped: bool,
    /// Whether the scenario could not be run here (`VERDICT ... BLOCKED`):
    /// what it needs is missing, such as a second mode. Never a pass.
    pub blocked: bool,
}

impl Outcome {
    fn new(failures: &[String], ok: String) -> Self {
        Self::with_stop(failures, ok, crate::stop::requested())
    }

    fn with_stop(failures: &[String], ok: String, stopped: bool) -> Self {
        if failures.is_empty() {
            Self {
                pass: true,
                reason: ok,
                stopped,
                blocked: false,
            }
        } else {
            Self {
                pass: false,
                reason: failures.join("; "),
                stopped,
                blocked: false,
            }
        }
    }

    /// A scenario that could not be run, and why.
    pub fn blocked(reason: String) -> Self {
        Self {
            pass: false,
            reason,
            stopped: false,
            blocked: true,
        }
    }
}

/// Default frame count of `move`.
pub const MOVE_FRAMES: u32 = 240;
/// Default frame count of `rotate`.
pub const ROTATE_FRAMES: u32 = 600;

/// Opaque pictures are drawn in a format without alpha: with the default
/// [`BlendMode::Opaque`] such a layer needs no blending, so it fits planes
/// without a `pixel blend mode` property (KMS then blends premultiplied).
/// [`ARGB`] is only for the layers that test pixel alpha or blend modes.
const XRGB: PixelFormat = PixelFormat::XRGB8888;
const ARGB: PixelFormat = PixelFormat::ARGB8888;

fn at(x: u32, y: u32, size: Size) -> Rect {
    Rect::new(x as i32, y as i32, size.width, size.height)
}

fn whole(size: Size) -> Rect {
    Rect::new(0, 0, size.width, size.height)
}

/// Fills `rect` with `colour`.
fn fill(rect: Rect, colour: u32) -> Op {
    Op::Fill { rect, colour }
}

/// A buffer of `size` for `layer`, filled with `fill`, framed with `edge`.
fn framed(
    s: &mut Session,
    layer: usize,
    size: Size,
    format: PixelFormat,
    fill_colour: u32,
    edge: u32,
) -> Result<BufferId, String> {
    s.dumb(
        layer,
        size,
        format,
        &[
            fill(whole(size), fill_colour),
            Op::Border {
                rect: whole(size),
                thickness: 8,
                colour: edge,
            },
        ],
    )
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

pub fn print_resources(scenario: &str, usage: &Usage) {
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
    let bottom = framed(&mut s, 0, size, XRGB, colour::BLUE, colour::WHITE)?;
    let top = framed(&mut s, 1, top_size, XRGB, colour::RED, colour::YELLOW)?;
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
    let a = framed(&mut s, 0, half, XRGB, colour::RED, colour::WHITE)?;
    let b = framed(&mut s, 1, half, XRGB, colour::GREEN, colour::WHITE)?;
    // Left half opaque red, right half red at half pixel alpha, premultiplied.
    let left = Rect::new(0, 0, half.width / 2, half.height);
    let right = Rect::new(
        (half.width / 2) as i32,
        0,
        half.width - half.width / 2,
        half.height,
    );
    let a_alpha = s.dumb(
        0,
        half,
        ARGB,
        &[
            fill(left, colour::RED),
            fill(right, colour::RED_HALF_PREMULTIPLIED),
        ],
    )?;
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
                if crate::stop::requested() {
                    break;
                }
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
    let bottom = s.dumb(
        0,
        size,
        XRGB,
        &[
            Op::Checker {
                rect: whole(size),
                cell: 64,
                a: colour::GREY,
                b: colour::BLACK,
            },
            Op::Border {
                rect: whole(size),
                thickness: 8,
                colour: colour::WHITE,
            },
        ],
    )?;
    let top_size = Size::new((w / 4).max(16), (h / 4).max(16));
    let top = framed(&mut s, 1, top_size, XRGB, colour::YELLOW, colour::RED)?;
    let before = s.checksum(bottom)?;
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
    let after = s.checksum(bottom)?;
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
    let bg = framed(&mut s, 0, size, XRGB, colour::BLUE, colour::WHITE)?;
    let mut smalls = Vec::new();
    for layer in 1..=plane_count.min(MAX_LAYERS) {
        smalls.push(framed(
            &mut s,
            layer,
            small,
            XRGB,
            colour::RED,
            colour::WHITE,
        )?);
    }
    // The same picture with an alpha channel, for the blend-mode combos.
    let small_alpha = framed(&mut s, 1, small, ARGB, colour::RED, colour::WHITE)?;
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
        let with_alpha = Layer {
            buffer: small_alpha,
            ..one
        };
        combos.push(("argb-opaque".into(), Expect::Any, pair(with_alpha)));
        combos.push((
            "blend-premultiplied".into(),
            Expect::Any,
            pair(Layer {
                blend: BlendMode::Premultiplied,
                ..with_alpha
            }),
        ));
        combos.push((
            "blend-coverage".into(),
            Expect::Any,
            pair(Layer {
                blend: BlendMode::Coverage,
                ..with_alpha
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
        // Every byte 0x80, whatever the pixel size.
        Some(format) => match s.dumb(1, odd_size, format, &[fill(whole(odd_size), 0x8080_8080)]) {
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
                s.tick();
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
    let formats = [XRGB, XRGB];
    let period = period_us(s.refresh_mhz).unwrap_or(0);

    let mut state = RotateState::default();
    let mut chains: [Vec<Rotated>; 2] = [Vec::new(), Vec::new()];
    for c in 0..2 {
        for i in 0..k {
            let (sz, base) = (sizes[c], bases[c]);
            let id = s.dumb(c, sz, formats[c], &[fill(whole(sz), base)])?;
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
            let mut ops = Vec::new();
            if let Some(old) = chains[c][i].band {
                ops.push(fill(old, base));
            }
            ops.push(fill(band, colour::WHITE));
            s.redraw(id, &ops)?;
            chains[c][i].band = Some(band);
            current[c] = id;
        }
        let layers = [
            Layer::new(current[0], whole(size), whole(size), 0),
            Layer::new(current[1], whole(top_size), top_at, 1),
        ];
        s.tick();
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

/// Waits `seconds` without touching the (lost) device, so that a camera can
/// look at the screen. Returns early, still `Ok`, if a stop is asked for: the
/// caller's checks were made and the stop is reported with them.
fn hold_idle(step: &str, seconds: u64, stopped: &dyn Fn() -> bool) -> Result<bool, String> {
    Record::new("hold")
        .field("step", step)
        .field("seconds", seconds)
        .emit();
    crate::stop::hold_for::<String>(seconds, Duration::from_millis(100), stopped, |slice| {
        std::thread::sleep(slice);
        Ok(())
    })
}

/// Default number of frames `revoke` shows before the lessor revokes.
pub const REVOKE_FRAMES: u32 = 60;

/// `revoke`: rotate buffers on two planes through a kernel lease, then the
/// lessor revokes the lease between two frames. The engine must report
/// the loss once, release every buffer that was on screen, and report no
/// outputs any more.
pub fn revoke(o: &Options) -> Result<Outcome, String> {
    let mut s = Session::open(o, "revoke")?;
    if !s.can_revoke() {
        return Err("revoke needs --source kernel-lease: this process must be the lessor".into());
    }
    let usage = Usage::now();
    let frames = o.frames.unwrap_or(REVOKE_FRAMES).max(2) as usize;
    let k = o.buffers as usize;
    let size = s.size;
    let (w, h) = (size.width, size.height);
    let top_size = Size::new(w / 2, h / 2);
    let sizes = [size, top_size];
    let bases = [colour::BLUE, colour::RED];
    let formats = [XRGB, XRGB];
    let mut chains: [Vec<BufferId>; 2] = [Vec::new(), Vec::new()];
    for c in 0..2 {
        for _ in 0..k {
            let id = s.dumb(c, sizes[c], formats[c], &[fill(whole(sizes[c]), bases[c])])?;
            chains[c].push(id);
        }
    }
    let all: Vec<BufferId> = chains.iter().flatten().copied().collect();
    Record::new("step")
        .field("label", "revoke")
        .field("frames_before", frames)
        .field("buffers_per_plane", k)
        .field(
            "expect",
            "white bars sweep, then the lease is revoked and the picture stops changing",
        )
        .emit();

    let mut failures = Vec::new();
    let mut releases_before = 0u32;
    let layers_for = |n: usize| {
        [
            Layer::new(chains[0][n % k], whole(size), whole(size), 0),
            Layer::new(
                chains[1][n % k],
                whole(top_size),
                at(w / 4, h / 4, top_size),
                1,
            ),
        ]
    };
    // Waits until the buffers of frame `n` are idle and draws them.
    let prepare = |s: &mut Session, n: usize, releases: &mut u32| -> Result<(), String> {
        for c in 0..2 {
            let id = chains[c][n % k];
            let deadline = Instant::now() + LATCH_TIMEOUT;
            while s.engine.buffer_state(id) != Some(BufferState::Ready) {
                if Instant::now() >= deadline {
                    return Err(format!(
                        "frame {n}: buffer {} not released ({:?})",
                        id.0,
                        s.engine.buffer_state(id)
                    ));
                }
                s.wait(LATCH_TIMEOUT / 10)?;
                *releases += s
                    .drain()?
                    .iter()
                    .filter(|e| matches!(e, DisplayEvent::Released { .. }))
                    .count() as u32;
            }
            let sz = sizes[c];
            let band_w = 32.min(sz.width);
            let x = ((n as u32 * 8) % (sz.width - band_w + 1)) as i32;
            s.redraw(
                id,
                &[
                    fill(whole(sz), bases[c]),
                    fill(Rect::new(x, 0, band_w, sz.height), colour::WHITE),
                ],
            )?;
        }
        Ok(())
    };
    let mut first_plan = None;
    for n in 0..frames {
        prepare(&mut s, n, &mut releases_before)?;
        let layers = layers_for(n);
        let shown = s.present(&layers)?;
        releases_before += shown
            .events
            .iter()
            .filter(|e| matches!(e, DisplayEvent::Released { .. }))
            .count() as u32;
        if first_plan.is_none() {
            s.print_plan("revoke", &layers, &shown.plan);
            failures.extend(check_distinct_and_order("revoke", &layers, &shown));
            first_plan = Some(planes_of(&shown.plan));
        }
    }
    let on_screen: Vec<BufferId> = all
        .iter()
        .copied()
        .filter(|b| s.engine.buffer_state(*b) == Some(BufferState::Scanning))
        .collect();
    Record::new("before_revoke")
        .field("frames", frames)
        .field("releases", releases_before)
        .field("on_screen", list(on_screen.iter().map(|b| b.0)))
        .emit();
    if on_screen.len() != 2 {
        failures.push(format!(
            "{} buffers on screen before the revocation, expected 2",
            on_screen.len()
        ));
    }

    // The lessor revokes between two frames; the next frame is prepared
    // as usual and must fail with the loss of the device.
    s.revoke()?;
    let revoked_at = Instant::now();
    prepare(&mut s, frames, &mut releases_before)?;
    let layers = layers_for(frames);
    s.tick();
    let first = s.engine.validate(&s.frame(&layers)).map(|_| ());
    let first_us = micros(revoked_at.elapsed());
    let mut events = Vec::new();
    let deadline = Instant::now() + LATCH_TIMEOUT;
    loop {
        events.extend(s.drain()?);
        if events.contains(&DisplayEvent::DeviceLost) || Instant::now() >= deadline {
            break;
        }
        s.wait(LATCH_TIMEOUT / 10)?;
    }
    // Nothing more may come. Once the loss is applied the backend reports
    // no outputs, so a further frame stops at the planner: the output it
    // names is unknown.
    let second = s.engine.validate(&s.frame(&layers)).map(|_| ());
    events.extend(s.drain()?);
    let lost = events
        .iter()
        .filter(|e| **e == DisplayEvent::DeviceLost)
        .count();
    let released: Vec<BufferId> = events
        .iter()
        .filter_map(|e| match e {
            DisplayEvent::Released { buffer } => Some(*buffer),
            _ => None,
        })
        .collect();
    let busy: Vec<BufferId> = all
        .iter()
        .copied()
        .filter(|b| {
            matches!(
                s.engine.buffer_state(*b),
                Some(BufferState::Scanning | BufferState::Submitted)
            )
        })
        .collect();
    let outputs = s.engine.capabilities().outputs.len();
    let planes = s.engine.capabilities().planes.len();
    Record::new("revoke_result")
        .field("first_attempt", format!("{first:?}"))
        .field("first_attempt_us", first_us)
        .field("second_attempt", format!("{second:?}"))
        .field("device_lost_events", lost)
        .field("released", list(released.iter().map(|b| b.0)))
        .field("still_busy", list(busy.iter().map(|b| b.0)))
        .field("outputs_after", outputs)
        .field("planes_after", planes)
        .field(
            "events",
            list(events.iter().map(|e| format!("{e:?}").replace(' ', ""))),
        )
        .emit();
    let lost_error = Err(Error::Backend(BackendError::DeviceLost));
    if first != lost_error {
        failures.push(format!(
            "first frame after revocation: {first:?}, expected DeviceLost"
        ));
    }
    if second != Err(Error::Plan(PlanError::UnknownOutput(s.output))) {
        failures.push(format!(
            "second frame after revocation: {second:?}, expected UnknownOutput({:?})",
            s.output
        ));
    }
    if lost != 1 {
        failures.push(format!("{lost} DeviceLost events, expected 1"));
    }
    for buffer in &on_screen {
        let count = released.iter().filter(|b| *b == buffer).count();
        if count != 1 {
            failures.push(format!(
                "buffer {} was on screen and released {count} times after the revocation",
                buffer.0
            ));
        }
    }
    if let Some(extra) = released.iter().find(|b| !on_screen.contains(b)) {
        failures.push(format!(
            "buffer {} released after the revocation but was not on screen",
            extra.0
        ));
    }
    if !busy.is_empty() {
        failures.push(format!(
            "buffers {} still submitted or scanning",
            list(busy.iter().map(|b| b.0))
        ));
    }
    if outputs != 0 || planes != 0 {
        failures.push(format!(
            "{outputs} outputs and {planes} planes still reported after the loss"
        ));
    }
    // The screen stays as the revocation left it, for a camera.
    hold_idle("revoke", o.seconds, &crate::stop::requested)?;
    print_resources("revoke", &usage);
    Ok(Outcome::new(
        &failures,
        format!(
            "{frames} frames on planes {}, then revoked: DeviceLost once, buffers {} released",
            first_plan.map_or("-".into(), |p| list(p.iter())),
            list(on_screen.iter().map(|b| b.0))
        ),
    ))
}

/// Seconds of commits on the original mode before `modeset` switches.
pub const MODESET_BEFORE_SECONDS: u64 = 1;

/// What one phase of `modeset` committed.
#[derive(Debug, Default)]
struct Phase {
    commits: u32,
    latched: u32,
    /// The error that ended the phase early, if any.
    error: Option<String>,
    /// A stop (SIGTERM, SIGINT) ended the phase.
    stopped: bool,
    /// Failures of the first frame's plan.
    plan_failures: Vec<String>,
}

/// A two-layer picture of [`commit_for`]: a framed full-screen bottom
/// buffer and a framed half-size top one in the middle.
struct Picture {
    /// Fill and edge of the bottom buffer.
    bottom: (u32, u32),
    /// Fill and edge of the top buffer.
    top: (u32, u32),
    /// What should be seen, for the `step` record.
    expect: &'static str,
}

/// The `two-planes` picture.
const TWO_PLANES: Picture = Picture {
    bottom: (colour::BLUE, colour::WHITE),
    top: (colour::RED, colour::YELLOW),
    expect: "blue full screen with white edge, red rectangle with yellow edge on top in the middle",
};

/// Commits `picture` on the session's current size, flipping between two
/// buffers per layer, until `seconds` have passed (at least one commit),
/// and waits for each latch. Prints a `commit` record per latched frame.
fn commit_for(
    s: &mut Session,
    step: &str,
    seconds: u64,
    picture: &Picture,
) -> Result<Phase, String> {
    let size = s.size;
    let (w, h) = (size.width, size.height);
    let top_size = Size::new(w / 2, h / 2);
    let ((bottom, bottom_edge), (top, top_edge)) = (picture.bottom, picture.top);
    // Two buffers per layer: [bottom, top] of even frames, then of odd ones.
    let mut pairs = [[BufferId(0); 2]; 2];
    for pair in &mut pairs {
        pair[0] = framed(s, 0, size, XRGB, bottom, bottom_edge)?;
        pair[1] = framed(s, 1, top_size, XRGB, top, top_edge)?;
    }
    Record::new("step")
        .field("label", step)
        .field("width", w)
        .field("height", h)
        .field("seconds", seconds)
        .field("expect", picture.expect)
        .emit();
    let mut phase = Phase::default();
    let deadline = Instant::now() + Duration::from_secs(seconds.min(crate::stop::MAX_SECONDS));
    let mut n = 0usize;
    loop {
        if crate::stop::requested() {
            phase.stopped = true;
            break;
        }
        let current = pairs[n % 2];
        // The buffers flipped away from in the last frame are released
        // with that frame's successor's latch; never commit them busy.
        let ready_by = Instant::now() + LATCH_TIMEOUT;
        while current
            .iter()
            .any(|id| s.engine.buffer_state(*id) != Some(BufferState::Ready))
        {
            if Instant::now() >= ready_by {
                phase.error = Some(format!("{step}: frame {n}: buffers not released in time"));
                return Ok(phase);
            }
            match s.wait(LATCH_TIMEOUT / 10).and_then(|()| s.drain()) {
                Ok(_) => {}
                Err(e) if e == crate::stop::STOPPED => {
                    phase.stopped = true;
                    return Ok(phase);
                }
                Err(e) => {
                    phase.error = Some(format!("{step}: {e}"));
                    return Ok(phase);
                }
            }
        }
        if n >= 2 {
            // Back to writable and ready, as a producer reuses a buffer
            // (its producer expects a draw between two releases); the
            // content stays as drawn.
            for id in current {
                if let Err(e) = s.redraw(id, &[]) {
                    phase.error = Some(format!("{step}: frame {n}: {e}"));
                    return Ok(phase);
                }
            }
        }
        let layers = [
            Layer::new(current[0], whole(size), whole(size), 0),
            Layer::new(current[1], whole(top_size), at(w / 4, h / 4, top_size), 1),
        ];
        match s.present(&layers) {
            Ok(shown) => {
                phase.commits += 1;
                if n == 0 {
                    s.print_plan(step, &layers, &shown.plan);
                    phase.plan_failures = check_distinct_and_order(step, &layers, &shown);
                }
                print_commit(step, &shown);
                phase.latched += 1;
            }
            Err(e) if e == crate::stop::STOPPED => {
                // Stopped before the commit was made.
                phase.stopped = true;
                break;
            }
            Err(e) => {
                // Validated or committed, but not latched.
                phase.commits += 1;
                phase.error = Some(format!("{step}: frame {n}: {e}"));
                break;
            }
        }
        n += 1;
        if Instant::now() >= deadline {
            break;
        }
    }
    Ok(phase)
}

fn mode_size(mode: &drm::control::Mode) -> Size {
    let (w, h) = mode.size();
    Size::new(u32::from(w), u32::from(h))
}

/// `modeset`: commit on the connector's current mode, switch to the first
/// other mode the sink offers that the device accepts, re-plan on the new
/// size and keep committing for `--seconds`, then put the original mode
/// back. BLOCKED when the connector offers only one mode.
pub fn modeset(o: &Options) -> Result<Outcome, String> {
    use crate::modeset::{blocked_reason, candidates, ModeKey, Switcher};
    use drm::control::Device as _;

    let mut s = Session::open(o, "modeset")?;
    let usage = Usage::now();
    let pairing = s.pairing()?;
    let original = pairing.mode;
    let current = ModeKey::of(&original);
    let modes: Vec<drm::control::Mode> = s
        .engine
        .backend()
        .card()
        .get_connector(pairing.handle, false)
        .map_err(|e| format!("connector {}: {e}", pairing.connector))?
        .modes()
        .to_vec();
    let keys: Vec<ModeKey> = modes.iter().map(ModeKey::of).collect();
    Record::new("connector_modes")
        .field("connector", &pairing.connector)
        .field("current", current.text())
        .field("count", modes.len())
        .field("modes", list(keys.iter().map(ModeKey::text)))
        .emit();
    if let Some(reason) = blocked_reason(current, &keys) {
        print_resources("modeset", &usage);
        return Ok(Outcome::blocked(reason));
    }
    let mut switcher = Switcher::new(s.engine.backend().card(), &pairing)
        .map_err(|e| format!("mode switch objects: {e}"))?;
    let mut target = None;
    let mut refused = Vec::new();
    for index in candidates(current, &keys) {
        let tested = switcher.test(s.engine.backend().card(), &modes[index]);
        Record::new("mode_candidate")
            .field("index", index)
            .field("mode", keys[index].text())
            .field("name", modes[index].name().to_string_lossy())
            .field(
                "test_only",
                match &tested {
                    Ok(()) => "ok".to_string(),
                    Err(e) => e.to_string(),
                },
            )
            .emit();
        if tested.is_ok() {
            target = Some(modes[index]);
            break;
        }
        refused.push(keys[index].text());
    }
    let Some(target) = target else {
        switcher.release(s.engine.backend().card(), false);
        print_resources("modeset", &usage);
        return Ok(Outcome::new(
            &[format!(
                "no mode other than {} passed TEST_ONLY (refused: {})",
                current.text(),
                list(refused.iter())
            )],
            String::new(),
        ));
    };
    let wanted = ModeKey::of(&target);

    let mut failures = Vec::new();
    let before = commit_for(
        &mut s,
        "modeset-before",
        MODESET_BEFORE_SECONDS,
        &TWO_PLANES,
    )?;
    failures.extend(before.plan_failures.iter().cloned());
    failures.extend(before.error.iter().cloned());
    if before.latched == 0 && !before.stopped && failures.is_empty() {
        failures.push("no latch on the original mode".into());
    }
    let mut switch_us = None;
    let mut after = Phase::default();
    if failures.is_empty() && !before.stopped {
        let blank = [fill(whole(mode_size(&target)), colour::BLACK)];
        match switcher.switch(s.engine.backend().card(), "switch", &target, &blank) {
            Ok(us) => switch_us = Some(us),
            Err(e) => failures.push(format!("switch to {}: {e}", wanted.text())),
        }
    }
    if switch_us.is_some() {
        // A new engine on the same descriptor finds the new mode; its plan
        // is made for the new size.
        let replanned = s.reopen().and_then(|()| {
            Record::new("replan")
                .field("width", s.size.width)
                .field("height", s.size.height)
                .field("refresh_mhz", s.refresh_mhz)
                .emit();
            if s.size != mode_size(&target) {
                return Err(format!(
                    "the engine reports {}x{} after the switch to {}",
                    s.size.width,
                    s.size.height,
                    wanted.text()
                ));
            }
            commit_for(&mut s, "modeset-after", o.seconds, &TWO_PLANES)
        });
        match replanned {
            Ok(phase) => after = phase,
            Err(e) => failures.push(e),
        }
        failures.extend(after.plan_failures.iter().cloned());
        failures.extend(after.error.iter().cloned());
        if after.latched == 0 && !after.stopped && failures.is_empty() {
            failures.push(format!("no latch after the switch to {}", wanted.text()));
        }
    }
    // Put the original mode back, whatever happened after the switch.
    let mut restored = "-".to_string();
    if switch_us.is_some() {
        let blank = [fill(whole(mode_size(&original)), colour::BLACK)];
        let card = s.engine.backend().card();
        if let Err(e) = switcher.switch(card, "restore", &original, &blank) {
            failures.push(format!("restore {}: {e}", current.text()));
        }
        restored = card
            .get_crtc(pairing.crtc)
            .ok()
            .and_then(|c| c.mode())
            .map_or("-".to_string(), |m| ModeKey::of(&m).text());
        if restored != current.text() {
            failures.push(format!(
                "the CRTC runs {restored} after the restore, not {}",
                current.text()
            ));
        }
    }
    switcher.release(s.engine.backend().card(), switch_us.is_some());
    Record::new("modeset_result")
        .field("from", current.text())
        .field("to", wanted.text())
        .field("refused", list(refused.iter()))
        .field("before_commits", before.commits)
        .field("before_latched", before.latched)
        .field(
            "switch_us",
            switch_us.map_or("-".to_string(), |us| us.to_string()),
        )
        .field("after_commits", after.commits)
        .field("after_latched", after.latched)
        .field("restored", &restored)
        .emit();
    // A stop before anything was checked after the switch is a cut-off.
    let cut_off = failures.is_empty()
        && ((before.stopped && switch_us.is_none()) || (after.stopped && after.latched == 0));
    if cut_off {
        return Err(crate::stop::STOPPED.into());
    }
    print_resources("modeset", &usage);
    Ok(Outcome::new(
        &failures,
        format!(
            "{} -> {}: {} commits latched before, {} after the switch; {} restored",
            current.text(),
            wanted.text(),
            before.latched,
            after.latched,
            current.text()
        ),
    ))
}

/// How long the engine has to report the loss of the display, from the
/// frame offered after it.
pub const RECOVERY_BUDGET: Duration = Duration::from_secs(2);

/// The engine's own latch timeout in `recovery`, in milliseconds of the
/// harness's clock (`Engine::advance_time`), well inside the budget.
pub const RECOVERY_LATCH_TIMEOUT_MS: u64 = 500;

/// The picture of the run after the recovery, unlike the one before the
/// loss, so that a picture tells the two apart.
const RECOVERED: Picture = Picture {
    bottom: (colour::GREEN, colour::WHITE),
    top: (colour::YELLOW, colour::BLUE),
    expect:
        "green full screen with white edge, yellow rectangle with blue edge on top in the middle",
};

/// Offers one frame of `buffers` (bottom, top) after the loss and waits up
/// to [`RECOVERY_BUDGET`] for the engine's answer, telling it the time as
/// it goes so that its latch timeout can fire. Prints a `loss_attempt`
/// record; returns the answer and how long it took in milliseconds.
fn loss_attempt(
    s: &mut Session,
    step: &str,
    buffers: [BufferId; 2],
) -> (crate::recovery::Report, u64) {
    use crate::recovery::{report_of, Report};

    let size = s.size;
    let (w, h) = (size.width, size.height);
    let top_size = Size::new(w / 2, h / 2);
    let layers = [
        Layer::new(buffers[0], whole(size), whole(size), 0),
        Layer::new(buffers[1], whole(top_size), at(w / 4, h / 4, top_size), 1),
    ];
    let started = Instant::now();
    s.tick();
    let frame = s.frame(&layers);
    let committed = match s.engine.validate(&frame) {
        Err(e) => Err(format!("validate: {}", describe(&e))),
        Ok(plan) => s
            .engine
            .commit(&frame, &plan)
            .map_err(|e| format!("commit: {}", describe(&e))),
    };
    let mut events = Vec::new();
    let report = match committed {
        Err(e) => Report::Error(e),
        Ok(submission) => loop {
            s.tick();
            match s.drain() {
                Ok(seen) => events.extend(seen),
                Err(e) => break Report::Error(e),
            }
            if let Some(report) = report_of(submission, &events) {
                break report;
            }
            let deadline = started + RECOVERY_BUDGET;
            let now = Instant::now();
            if now >= deadline {
                break Report::Silent;
            }
            let slice = (deadline - now).min(LATCH_TIMEOUT / 10);
            if let Err(e) = s.engine.backend().wait_for_events(Some(slice)) {
                break Report::Error(format!("poll: {e}"));
            }
        },
    };
    let after_ms = micros(started.elapsed()) / 1000;
    // What came with the error (a DeviceLost, releases), for the record.
    events.extend(s.drain().unwrap_or_default());
    let failures: Vec<String> = s
        .engine
        .backend_mut()
        .take_failures()
        .map(|f| format!("{f:?}").replace(' ', ""))
        .collect();
    let backend = s.engine.backend();
    Record::new("loss_attempt")
        .field("step", step)
        .field("report", report.text())
        .field("after_ms", after_ms)
        .field("backend_failures", list(failures.iter()))
        .field("device_lost", backend.is_lost())
        .field("pending_latch", backend.pending_latch(s.output).is_some())
        .field(
            "events",
            list(events.iter().map(|e| format!("{e:?}").replace(' ', ""))),
        )
        .emit();
    (report, after_ms)
}

/// Turns the CRTC of `pairing` back on with its original mode if it is
/// off, with the `modeset` scenario's own commit (a black buffer on the
/// primary plane). Returns whether it is on afterwards, `None` if that
/// cannot be read.
fn ensure_on(card: &composition_drm::Card, pairing: &crate::session::Pairing) -> Option<bool> {
    use crate::recovery::crtc_active;

    let active = crtc_active(card, pairing.crtc).ok();
    if active != Some(false) {
        return active;
    }
    let black = [fill(whole(mode_size(&pairing.mode)), colour::BLACK)];
    match crate::modeset::Switcher::new(card, pairing) {
        Ok(mut switcher) => {
            let switched = switcher.switch(card, "restore", &pairing.mode, &black);
            switcher.release(card, switched.is_ok());
        }
        Err(e) => Record::new("error")
            .field("scenario", "recovery")
            .field("message", format!("restore the CRTC: {e}"))
            .emit(),
    }
    crtc_active(card, pairing.crtc).ok()
}

/// `recovery`: commit the two-planes picture for `--seconds`, lose the
/// display (`--loss master`: another descriptor takes DRM master and turns
/// the CRTC off; `--loss disable`: the CRTC is turned off on the engine's
/// descriptor), check that the engine reports the loss within
/// [`RECOVERY_BUDGET`] and holds no buffer afterwards, then take master
/// back, start a new engine run and check that it latches for `--seconds`.
pub fn recovery(o: &Options) -> Result<Outcome, String> {
    use crate::recovery::{
        busy, judge, open_master, result_text, states_text, turn_off, Findings, Loss,
    };
    use crate::source::Source;
    use drm::Device as _;

    let device = match (&o.source, o.loss) {
        (Source::Master { device }, _) => Some(device.clone()),
        (_, Loss::Master) => {
            return Err(
                "recovery --loss master needs --source master: the engine's \
                        descriptor must be the DRM master that is lost"
                    .into(),
            )
        }
        _ => None,
    };
    let mut s = Session::open(o, "recovery")?;
    let usage = Usage::now();
    s.engine.set_latch_timeout(Some(RECOVERY_LATCH_TIMEOUT_MS));
    let pairing = s.pairing()?;
    let crtc = pairing.crtc;
    let size = s.size;
    let top_size = Size::new(size.width / 2, size.height / 2);
    // The frames offered after the loss, drawn while the display is there.
    let mut after_loss = [[BufferId(0); 2]; 2];
    for pair in &mut after_loss {
        pair[0] = framed(&mut s, 0, size, XRGB, colour::GREY, colour::BLACK)?;
        pair[1] = framed(&mut s, 1, top_size, XRGB, colour::GREY, colour::BLACK)?;
    }
    Record::new("hold")
        .field("step", "recovery-before")
        .field("seconds", o.seconds)
        .emit();
    let before = commit_for(&mut s, "recovery-before", o.seconds, &TWO_PLANES)?;
    if before.stopped {
        // Nothing was lost yet, nothing checked.
        return Err(crate::stop::STOPPED.into());
    }
    let mut failures = before.plan_failures.clone();
    failures.extend(before.error.iter().cloned());
    if before.latched == 0 && failures.is_empty() {
        failures.push("no latch before the loss".into());
    }
    let on_screen: Vec<BufferId> = s
        .buffer_ids()
        .into_iter()
        .filter(|b| s.engine.buffer_state(*b) == Some(BufferState::Scanning))
        .collect();
    Record::new("before_loss")
        .field("commits", before.commits)
        .field("latched", before.latched)
        .field("on_screen", list(on_screen.iter().map(|b| b.0)))
        .emit();
    if !failures.is_empty() {
        print_resources("recovery", &usage);
        return Ok(Outcome::new(&failures, String::new()));
    }

    // The loss. The second descriptor is kept until master is taken back.
    let mut other = None;
    let made: Result<(), String> = match o.loss {
        Loss::Master => (|| {
            s.engine
                .backend()
                .card()
                .release_master_lock()
                .map_err(|e| format!("drop DRM master: {e}"))?;
            let device = device.as_deref().ok_or("no device node")?;
            let card =
                open_master(device).map_err(|e| format!("second descriptor as DRM master: {e}"))?;
            let off = turn_off(&card, crtc).map_err(|e| format!("CRTC off: {e}"));
            other = Some(card);
            off
        })(),
        Loss::Disable => {
            turn_off(s.engine.backend().card(), crtc).map_err(|e| format!("CRTC off: {e}"))
        }
    };
    Record::new("loss")
        .field("method", o.loss.name())
        .field("crtc", u32::from(crtc))
        .field("result", result_text(&made))
        .field(
            "crtc_active",
            crate::recovery::crtc_active(s.engine.backend().card(), crtc)
                .map_or("?".to_string(), |a| a.to_string()),
        )
        .emit();
    let (first, first_ms) = loss_attempt(&mut s, "after-loss", after_loss[0]);
    let mut master_back = None;
    let mut second = None;
    if o.loss == Loss::Master {
        // The other master goes; the engine's descriptor takes the role back.
        let closed = other.take().is_some();
        let back = s
            .engine
            .backend()
            .card()
            .acquire_master_lock()
            .map_err(|e| e.to_string());
        Record::new("master_back")
            .field("other_closed", closed)
            .field("result", result_text(&back))
            .emit();
        if back.is_ok() {
            second = Some(loss_attempt(&mut s, "master-back", after_loss[1]).0);
        }
        master_back = Some(back);
    }
    let states: Vec<(BufferId, Option<BufferState>)> = s
        .buffer_ids()
        .into_iter()
        .map(|b| (b, s.engine.buffer_state(b)))
        .collect();
    let still_busy = busy(&states);
    Record::new("buffers_after_loss")
        .field("states", states_text(&states))
        .field("busy", list(still_busy.iter().map(|b| b.0)))
        .emit();
    // The old engine goes, and its buffers with its descriptor.
    drop(s);

    // A new run, as a restarted application would make it: its first
    // commit is a modeset, which turns the CRTC on again.
    let (after, active_at_end) =
        match Session::open(o, "recovery") {
            Ok(mut s) => {
                Record::new("hold")
                    .field("step", "recovery-after")
                    .field("seconds", o.seconds)
                    .emit();
                let after = commit_for(&mut s, "recovery-after", o.seconds, &RECOVERED)
                    .unwrap_or_else(|e| Phase {
                        error: Some(e),
                        ..Phase::default()
                    });
                (after, ensure_on(s.engine.backend().card(), &pairing))
            }
            Err(e) => {
                // Put the CRTC back on from a descriptor of its own, if one can
                // be master.
                let active = device
                    .as_deref()
                    .and_then(|d| open_master(d).ok())
                    .and_then(|card| ensure_on(&card, &pairing));
                let after = Phase {
                    error: Some(e),
                    ..Phase::default()
                };
                (after, active)
            }
        };
    failures.extend(after.plan_failures);
    let (relatched, new_run_error, after_stopped) = (after.latched, after.error, after.stopped);
    let findings = Findings {
        loss: o.loss,
        made,
        first,
        second,
        busy: still_busy,
        master_back,
        relatched,
        new_run_error,
        active_at_end,
    };
    failures.extend(judge(&findings, micros(RECOVERY_BUDGET) / 1000));
    Record::new("recovery_result")
        .field("loss", o.loss.name())
        .field("made", result_text(&findings.made))
        .field("first", findings.first.text())
        .field("first_ms", first_ms)
        .field("reported", findings.first.reported())
        .field(
            "second",
            findings
                .second
                .as_ref()
                .map_or("-".to_string(), |r| r.text()),
        )
        .field("still_busy", list(findings.busy.iter().map(|b| b.0)))
        .field(
            "master_back",
            findings
                .master_back
                .as_ref()
                .map_or("-".to_string(), result_text),
        )
        .field("relatched", findings.relatched)
        .field(
            "crtc_active_end",
            findings
                .active_at_end
                .map_or("?".to_string(), |a| a.to_string()),
        )
        .emit();
    if after_stopped && relatched == 0 && findings.new_run_error.is_none() {
        // Stopped before the new run showed anything: not checked.
        return Err(crate::stop::STOPPED.into());
    }
    print_resources("recovery", &usage);
    Ok(Outcome::new(
        &failures,
        format!(
            "{} loss reported as {} after {first_ms} ms; no buffer held; \
             a new run latched {relatched} commits",
            o.loss.name(),
            findings.first.text()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revoke_holds_the_screen_for_seconds() {
        let started = Instant::now();
        assert!(hold_idle("revoke", 1, &|| false).unwrap());
        assert!(started.elapsed() >= Duration::from_millis(990));
        let started = Instant::now();
        assert!(hold_idle("revoke", 0, &|| false).unwrap());
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn a_stop_ends_the_revoke_hold_at_once() {
        let started = Instant::now();
        let ended = hold_idle("revoke", 3600, &|| true);
        assert_eq!(ended, Ok(false));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn the_recovered_picture_differs_where_a_picture_is_judged() {
        // The corner (bottom fill) and the centre (top fill) both change.
        assert_ne!(RECOVERED.bottom.0, TWO_PLANES.bottom.0);
        assert_ne!(RECOVERED.top.0, TWO_PLANES.top.0);
        assert_ne!(RECOVERED.expect, TWO_PLANES.expect);
    }

    #[test]
    fn the_engine_latch_timeout_fires_inside_the_budget() {
        assert!(Duration::from_millis(RECOVERY_LATCH_TIMEOUT_MS) < RECOVERY_BUDGET);
        // Polled in slices of LATCH_TIMEOUT / 10, so it is seen in time.
        assert!(
            Duration::from_millis(RECOVERY_LATCH_TIMEOUT_MS) + LATCH_TIMEOUT / 10 < RECOVERY_BUDGET
        );
    }

    #[test]
    fn a_stop_keeps_what_the_checks_found() {
        let failed = Outcome::with_stop(&["no event".to_string()], "ok".into(), true);
        assert!(!failed.pass && failed.stopped);
        assert_eq!(failed.reason, "no event");
        let passed = Outcome::with_stop(&[], "ok".into(), true);
        assert!(passed.pass && passed.stopped);
    }
}
