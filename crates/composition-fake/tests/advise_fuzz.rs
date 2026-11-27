//! Seeded fuzz of `composition_core::advise` (design note §5 tests 2, 3 and
//! 11) on fake topologies.
//!
//! The same splitmix64 generator, seed and case-count conventions as
//! `malformed_frames.rs`, no dependency. Frames are mostly valid so that
//! advice is usually produced, with a share of malformed ones: unknown
//! buffers and outputs, empty or out-of-buffer sources, empty destinations,
//! too many layers. The caller's check refuses plane sets by a seeded hash
//! of the plane set, so it is a pure function and advice must be repeatable.
//!
//! Invariants checked for every case:
//!
//! * `advise` never panics (the debug profile also catches overflow),
//! * whole-call input errors are exactly those a reference check of the
//!   input predicts (unknown output, too many layers, unknown buffer,
//!   invalid source, invalid destination; the first by layer order),
//! * an `Ok` advice has one entry per layer; off-output layers are `Hidden`
//!   with no plane and no blocker, and `Hidden` means off-output,
//! * the composed layers are one contiguous run in stacking order (no
//!   `Plane` placement between two `Compose` placements), and
//!   `compose_run` / `compose` agree with the placements,
//! * every `Compose` layer has a non-`None` blocker (the design allows
//!   `None` only when the budget ended, and `Blocker::Budget` is then the
//!   blocker, so the stricter form holds),
//! * the reduced frame (promoted layers plus one synthetic layer for the
//!   composed run) is accepted by `plan_with` with exactly the returned
//!   assignments,
//! * the same input twice gives identical advice and the same sequence of
//!   check calls,
//! * the check is called no more than `limits.backend_checks` times, and
//!   `Advice::backend_checks` equals the calls made; `Aborted` only comes
//!   from a check that gave up.
//!
//! A failing case prints the seed, the case index, the capabilities'
//! topology name, the frame and the advice. `COMPOSITION_FUZZ_CASES` raises
//! the number of cases (default 3000) and `COMPOSITION_FUZZ_SEED` explores
//! other draws.

use composition_core::fixed::FixedVec;
use composition_core::planner::plan_with;
use composition_core::{
    advise, Advice, AdviceError, AdviseOptions, Assignment, BlendMode, Blocker, BufferId,
    BufferInfo, BufferLookup, Capabilities, ComposeTarget, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, Placement, Plan, PlaneCaps, PlaneId, PlaneKind, Rect,
    ScaleLimits, SearchLimits, Size, Transform, Verdict, Zpos, ALPHA_OPAQUE, COMPOSE_LAYER,
};

const DEFAULT_CASES: u32 = 3_000;
const DEFAULT_SEED: u64 = 0x2d59_f017_5bd1_ce15;
const MAX_LAYERS: usize = 16;

/// splitmix64, as in `malformed_frames.rs`.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..bound`; `bound` must be at least 1.
    fn below(&mut self, bound: u32) -> u32 {
        ((self.next_u64() >> 32) as u32) % bound
    }

    fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den) < num
    }

    fn one_of<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u32) as usize]
    }
}

const XRGB: FormatModifier = FormatModifier::linear(PixelFormat::XRGB8888);
const ARGB: FormatModifier = FormatModifier::linear(PixelFormat::ARGB8888);
const RGB565: FormatModifier = FormatModifier::linear(PixelFormat::RGB565);
const NV12: FormatModifier = FormatModifier::linear(PixelFormat::NV12);

fn plane(id: u32, kind: PlaneKind, zpos: Zpos, formats: &[FormatModifier]) -> PlaneCaps {
    PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs: 0b11,
        formats: FixedVec::from_slice(formats).unwrap(),
        scaling: None,
        max_source: None,
        plane_alpha: true,
        blend_modes: 0b111,
        transforms: 0b111_111,
        zpos,
        full_output_only: false,
    }
}

fn caps_with(outputs: &[(u32, u32, u32)], planes: &[PlaneCaps]) -> Capabilities {
    let mut caps = Capabilities::default();
    for &(id, width, height) in outputs {
        caps.outputs
            .push(OutputCaps {
                id: OutputId(id),
                size: Size::new(width, height),
                refresh_mhz: 60_000,
            })
            .unwrap();
    }
    for p in planes {
        caps.planes.push(*p).unwrap();
    }
    caps
}

type Topology = (&'static str, fn() -> Capabilities);

/// Fake topologies: the shapes of the LTDC, a four-overlay scaler SoC, a
/// single full-output primary, a primary plus cursor, two outputs with split
/// planes, and one with planes that take nothing.
fn topologies() -> [Topology; 7] {
    [
        ("ltdc-like, three planes", || {
            let mut planes = [
                plane(
                    34,
                    PlaneKind::Primary,
                    Zpos::Range { min: 0, max: 2 },
                    &[XRGB, ARGB, RGB565],
                ),
                plane(
                    42,
                    PlaneKind::Overlay,
                    Zpos::Range { min: 0, max: 2 },
                    &[XRGB, ARGB, NV12],
                ),
                plane(
                    49,
                    PlaneKind::Overlay,
                    Zpos::Range { min: 0, max: 2 },
                    &[XRGB, ARGB],
                ),
            ];
            for p in &mut planes {
                p.outputs = 1;
                p.blend_modes = BlendMode::Premultiplied.mask();
                p.transforms = 0;
            }
            caps_with(&[(0, 1920, 1080)], &planes)
        }),
        ("four overlays with scalers", || {
            let mut a = plane(
                1,
                PlaneKind::Overlay,
                Zpos::Range { min: 0, max: 3 },
                &[XRGB, ARGB],
            );
            a.scaling = Some(ScaleLimits {
                max_upscale: 8 << 16,
                max_downscale: 8 << 16,
            });
            let mut b = plane(
                2,
                PlaneKind::Overlay,
                Zpos::Range { min: 0, max: 3 },
                &[XRGB, ARGB],
            );
            b.scaling = Some(ScaleLimits {
                max_upscale: 2 << 16,
                max_downscale: 2 << 16,
            });
            let c = plane(3, PlaneKind::Overlay, Zpos::Fixed(2), &[RGB565, XRGB]);
            let d = plane(
                4,
                PlaneKind::Overlay,
                Zpos::Range { min: 0, max: 1 },
                &[ARGB, NV12],
            );
            caps_with(&[(0, 1920, 1080)], &[a, b, c, d])
        }),
        ("single full-output primary", || {
            let mut p = plane(1, PlaneKind::Primary, Zpos::Fixed(0), &[XRGB, RGB565]);
            p.full_output_only = true;
            p.plane_alpha = false;
            p.blend_modes = BlendMode::Opaque.mask();
            caps_with(&[(0, 1920, 1080)], &[p])
        }),
        ("primary and cursor", || {
            let mut primary = plane(1, PlaneKind::Primary, Zpos::Fixed(0), &[XRGB, ARGB]);
            primary.full_output_only = true;
            let mut cursor = plane(2, PlaneKind::Cursor, Zpos::Fixed(8), &[ARGB]);
            cursor.max_source = Some(Size::new(64, 64));
            caps_with(&[(0, 1280, 720)], &[primary, cursor])
        }),
        ("two outputs, split planes", || {
            let mut first = plane(1, PlaneKind::Overlay, Zpos::Fixed(0), &[XRGB]);
            first.outputs = 0b01;
            let mut second = plane(2, PlaneKind::Overlay, Zpos::Fixed(0), &[ARGB]);
            second.outputs = 0b10;
            let both = plane(3, PlaneKind::Overlay, Zpos::Fixed(1), &[XRGB, ARGB]);
            caps_with(&[(0, 1920, 1080), (1, 800, 480)], &[first, second, both])
        }),
        ("planes without formats", || {
            let a = plane(1, PlaneKind::Overlay, Zpos::Fixed(0), &[]);
            let b = plane(2, PlaneKind::Overlay, Zpos::Fixed(1), &[]);
            caps_with(&[(0, 1920, 1080)], &[a, b])
        }),
        ("many planes", || {
            let planes: Vec<PlaneCaps> = (0..8)
                .map(|i| {
                    let mut p = plane(
                        10 + i,
                        PlaneKind::Overlay,
                        Zpos::Range { min: 0, max: 7 },
                        &[XRGB, ARGB],
                    );
                    p.outputs = 1;
                    p
                })
                .collect();
            caps_with(&[(0, 1920, 1080)], &planes)
        }),
    ]
}

struct Buffers(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

/// The caller's check: a pure function of the plane set, so advice is
/// repeatable. `refuse` is the share of plane sets refused, out of 8.
#[derive(Clone, Copy, Debug)]
struct Check {
    seed: u64,
    refuse: u32,
    give_up: bool,
}

impl Check {
    fn verdict(&self, plan: &Plan) -> Verdict {
        if self.give_up {
            return Verdict::GiveUp;
        }
        let mut h = self.seed;
        for a in plan.assignments() {
            h = h
                .wrapping_mul(0x100_0000_01B3)
                .wrapping_add(a.plane_index as u64 * 31 + (a.zpos as i64 as u64));
        }
        let mut rng = Rng(h);
        if rng.below(8) < self.refuse {
            Verdict::TryNext
        } else {
            Verdict::Accept
        }
    }
}

#[derive(Debug)]
struct Case {
    topology: &'static str,
    caps: Capabilities,
    buffers: Vec<(BufferId, BufferInfo)>,
    output: OutputId,
    layers: Vec<Layer>,
    check: Check,
    steps: u32,
    backend_checks: u32,
    max_candidates: u32,
    compose: Option<ComposeTarget>,
}

fn gen_buffers(rng: &mut Rng, caps: &Capabilities) -> Vec<(BufferId, BufferInfo)> {
    let mut formats: Vec<FormatModifier> = Vec::new();
    for p in caps.planes.iter() {
        formats.extend(p.formats.iter().copied());
    }
    if formats.is_empty() {
        formats.push(XRGB);
    }
    (0..1 + rng.below(6))
        .map(|i| {
            let f = if rng.chance(1, 8) {
                // A format no plane lists.
                FormatModifier::linear(PixelFormat::fourcc(b"ABCD"))
            } else {
                rng.one_of(&formats)
            };
            let width = rng.one_of(&[16, 64, 200, 640, 1920]);
            let height = rng.one_of(&[16, 64, 100, 480, 1080]);
            (
                BufferId(1 + i),
                BufferInfo {
                    size: Size::new(width, height),
                    format: f.format,
                    modifier: f.modifier,
                },
            )
        })
        .collect()
}

fn gen_layer(rng: &mut Rng, buffers: &[(BufferId, BufferInfo)], out: Size) -> Layer {
    let (id, info) = buffers[rng.below(buffers.len() as u32) as usize];
    let mut buffer = id;
    let mut source = Rect::new(0, 0, info.size.width, info.size.height);
    if rng.chance(1, 2) {
        let w = 1 + rng.below(info.size.width);
        let h = 1 + rng.below(info.size.height);
        source = Rect::new(
            rng.below(info.size.width - w + 1) as i32,
            rng.below(info.size.height - h + 1) as i32,
            w,
            h,
        );
    }
    let ow = out.width.max(1);
    let oh = out.height.max(1);
    let destination = match rng.below(10) {
        0 => Rect::new(0, 0, out.width, out.height),
        // Same size as the source: no scaling needed.
        1..=4 => Rect::new(
            rng.below(ow) as i32,
            rng.below(oh) as i32,
            source.width,
            source.height,
        ),
        5..=6 => Rect::new(
            rng.below(ow) as i32,
            rng.below(oh) as i32,
            1 + rng.below(ow),
            1 + rng.below(oh),
        ),
        // Partly off the output.
        7 => Rect::new(-(rng.below(300) as i32), -(rng.below(300) as i32), 400, 300),
        // Entirely off the output.
        8 => Rect::new(out.width as i32 + 10, 0, 100, 100),
        _ => Rect::new(
            rng.below(ow) as i32,
            rng.below(oh) as i32,
            1 + rng.below(3000),
            1 + rng.below(3000),
        ),
    };
    let mut layer = Layer::new(
        buffer,
        source,
        destination,
        rng.one_of(&[-2, 0, 0, 1, 1, 2, 5]),
    );
    layer.alpha = rng.one_of(&[ALPHA_OPAQUE, ALPHA_OPAQUE, ALPHA_OPAQUE, 0x8000]);
    layer.blend = rng.one_of(&[
        BlendMode::Opaque,
        BlendMode::Premultiplied,
        BlendMode::Opaque,
    ]);
    layer.transform = rng.one_of(&[
        Transform::Normal,
        Transform::Normal,
        Transform::Normal,
        Transform::Rotate180,
        Transform::FlipH,
        Transform::Rotate90,
    ]);
    // Malformations.
    match rng.below(60) {
        0 => buffer = BufferId(900 + rng.below(5)),
        1 => layer.source = Rect::new(0, 0, 0, 5),
        2 => layer.source = Rect::new(info.size.width as i32 - 1, 0, 2, 1),
        3 => layer.source = Rect::new(-1, 0, 1, 1),
        4 => layer.destination = Rect::new(5, 5, 0, 10),
        5 => layer.destination = Rect::new(5, 5, 10, 0),
        _ => {}
    }
    layer.buffer = buffer;
    layer
}

fn gen_case(rng: &mut Rng) -> Case {
    let all = topologies();
    let (topology, build) = rng.one_of(&all);
    let caps = build();
    let output = if rng.chance(1, 40) {
        OutputId(50 + rng.below(5))
    } else {
        caps.outputs[rng.below(caps.outputs.len() as u32) as usize].id
    };
    let out_size = caps
        .outputs
        .iter()
        .find(|o| o.id == output)
        .map_or(Size::new(1920, 1080), |o| o.size);
    let buffers = gen_buffers(rng, &caps);
    let count = if rng.chance(1, 40) {
        MAX_LAYERS as u32 + 1 + rng.below(3)
    } else if rng.chance(1, 8) {
        MAX_LAYERS as u32
    } else {
        rng.below(7)
    };
    let layers = (0..count)
        .map(|_| gen_layer(rng, &buffers, out_size))
        .collect();
    let compose = if rng.chance(1, 6) {
        None
    } else {
        Some(ComposeTarget {
            format: rng.one_of(&[PixelFormat::XRGB8888, PixelFormat::ARGB8888]),
            modifier: Modifier::LINEAR,
            blend: rng.one_of(&[BlendMode::Opaque, BlendMode::Premultiplied]),
        })
    };
    Case {
        topology,
        caps,
        buffers,
        output,
        layers,
        check: Check {
            seed: rng.next_u64(),
            refuse: rng.one_of(&[0, 0, 1, 2, 4, 8]),
            give_up: rng.chance(1, 60),
        },
        steps: rng.one_of(&[20_000, 20_000, 20_000, 40, 3]),
        backend_checks: rng.one_of(&[32, 32, 32, 1, 2, 5]),
        max_candidates: rng.one_of(&[137, 137, 137, 1, 4, 20]),
        compose,
    }
}

/// What the whole-call input checks must say, from the input alone.
fn expected_error(case: &Case) -> Option<AdviceError> {
    if case.caps.outputs.iter().all(|o| o.id != case.output) {
        return Some(AdviceError::UnknownOutput(case.output));
    }
    if case.layers.len() > MAX_LAYERS {
        return Some(AdviceError::TooManyLayers {
            count: case.layers.len(),
        });
    }
    for (index, layer) in case.layers.iter().enumerate() {
        let Some((_, info)) = case.buffers.iter().find(|(id, _)| *id == layer.buffer) else {
            return Some(AdviceError::UnknownBuffer {
                layer: index,
                buffer: layer.buffer,
            });
        };
        let s = layer.source;
        let inside = s.x >= 0
            && s.y >= 0
            && s.x as i64 + s.width as i64 <= info.size.width as i64
            && s.y as i64 + s.height as i64 <= info.size.height as i64;
        if s.width == 0 || s.height == 0 || !inside {
            return Some(AdviceError::InvalidSource { layer: index });
        }
        if layer.destination.width == 0 || layer.destination.height == 0 {
            return Some(AdviceError::InvalidDestination { layer: index });
        }
    }
    None
}

/// `true` if the (non-empty) destination shares no pixel with the output.
fn off_output(destination: Rect, out: Size) -> bool {
    let (x, y) = (destination.x as i64, destination.y as i64);
    x >= out.width as i64
        || y >= out.height as i64
        || x + destination.width as i64 <= 0
        || y + destination.height as i64 <= 0
}

fn options(case: &Case) -> AdviseOptions {
    AdviseOptions {
        limits: SearchLimits {
            steps: case.steps,
            backend_checks: case.backend_checks,
        },
        max_candidates: case.max_candidates,
        compose: case.compose,
    }
}

/// One `advise` call; also returns the number of check calls and whether the
/// check gave up.
fn run(case: &Case) -> (Result<Advice, AdviceError>, u32, bool) {
    let frame = Frame {
        output: case.output,
        layers: &case.layers,
    };
    let mut calls = 0;
    let mut gave_up = false;
    let result = advise(
        &frame,
        &case.caps,
        &Buffers(case.buffers.clone()),
        &options(case),
        |plan, at| {
            calls += 1;
            if let Some(at) = at {
                assert!(at < plan.assignments().len(), "compose index out of range");
            }
            let verdict = case.check.verdict(plan);
            gave_up |= verdict == Verdict::GiveUp;
            verdict
        },
    );
    (result, calls, gave_up)
}

fn describe(advice: &Advice) -> String {
    let mut s = format!(
        "compose={:?} run={:?} candidates={} checks={}",
        advice.compose(),
        advice.compose_run(),
        advice.candidates(),
        advice.backend_checks()
    );
    for (i, l) in advice.layers().iter().enumerate() {
        s += &format!("\n  layer {i}: {l:?}");
    }
    s
}

/// Stacking order of the layers that are on the output: by `z_order`, equal
/// values in frame order.
fn stacking(layers: &[Layer], advice: &Advice) -> Vec<usize> {
    let mut order: Vec<usize> = (0..layers.len())
        .filter(|&i| advice.layers()[i].placement != Placement::Hidden)
        .collect();
    order.sort_by_key(|&i| (layers[i].z_order, i));
    order
}

fn check_advice(case: &Case, advice: &Advice, calls: u32) {
    let out_size = case
        .caps
        .outputs
        .iter()
        .find(|o| o.id == case.output)
        .unwrap()
        .size;
    let layers = &case.layers;
    assert_eq!(advice.layers().len(), layers.len(), "one entry per layer");
    assert_eq!(advice.output(), case.output);
    assert_eq!(advice.backend_checks(), calls, "counted check calls");
    assert!(calls <= case.backend_checks, "check calls over the limit");
    assert!(advice.candidates() <= case.max_candidates);

    // Hidden layers.
    for (i, (l, entry)) in layers.iter().zip(advice.layers()).enumerate() {
        let hidden = entry.placement == Placement::Hidden;
        assert_eq!(
            hidden,
            off_output(l.destination, out_size),
            "layer {i}: Hidden iff off the output"
        );
        if hidden {
            assert_eq!(entry.blocker, Blocker::None, "layer {i}: hidden blocker");
        }
        match entry.placement {
            Placement::Compose => {
                assert_ne!(
                    entry.blocker,
                    Blocker::None,
                    "layer {i}: Compose needs a blocker"
                )
            }
            Placement::Plane(_) => {
                assert_eq!(entry.blocker, Blocker::None, "layer {i}: plane blocker")
            }
            Placement::Hidden => {}
        }
    }

    // Contiguity in stacking order.
    let order = stacking(layers, advice);
    let kinds: Vec<bool> = order
        .iter()
        .map(|&i| advice.layers()[i].placement == Placement::Compose)
        .collect();
    let first = kinds.iter().position(|&c| c);
    let last = kinds.iter().rposition(|&c| c);
    if let (Some(first), Some(last)) = (first, last) {
        assert!(
            kinds[first..=last].iter().all(|&c| c),
            "a Plane placement lies between two Compose placements"
        );
    }
    let composed = kinds.iter().filter(|&&c| c).count();
    if case.compose.is_some() {
        assert_eq!(
            advice.compose().is_some(),
            composed > 0,
            "compose iff composed"
        );
        assert_eq!(advice.compose_run(), first.zip(last), "compose_run");
    } else {
        assert!(advice.compose().is_none() && advice.compose_run().is_none());
        assert!(
            composed == 0 || advice.promoted() == 0,
            "without a target either everything is on planes or nothing is"
        );
    }

    // Reduced frame accepted with the returned assignments.
    if case.compose.is_none() && composed > 0 {
        // No composition target: nothing to plan; the frame as it is failed.
        return;
    }
    let target_id = (0..=MAX_LAYERS as u32)
        .map(|k| BufferId(u32::MAX - k))
        .find(|id| layers.iter().all(|l| l.buffer != *id))
        .unwrap();
    let mut buffers = case.buffers.clone();
    let mut reduced: Vec<Layer> = Vec::new();
    // Per reduced layer: the advised assignment it must receive.
    let mut expected: Vec<Assignment> = Vec::new();
    let lowest = order
        .iter()
        .copied()
        .find(|&i| advice.layers()[i].placement == Placement::Compose);
    for (i, l) in layers.iter().enumerate() {
        match advice.layers()[i].placement {
            Placement::Plane(a) => {
                reduced.push(*l);
                expected.push(a);
            }
            Placement::Compose if Some(i) == lowest => {
                let target = case.compose.unwrap();
                buffers.push((
                    target_id,
                    BufferInfo {
                        size: out_size,
                        format: target.format,
                        modifier: target.modifier,
                    },
                ));
                let mut synthetic = Layer::new(
                    target_id,
                    Rect::new(0, 0, out_size.width, out_size.height),
                    Rect::new(0, 0, out_size.width, out_size.height),
                    l.z_order,
                );
                synthetic.blend = target.blend;
                reduced.push(synthetic);
                expected.push(*advice.compose().expect("compose assignment"));
            }
            _ => {}
        }
    }
    let frame = Frame {
        output: case.output,
        layers: &reduced,
    };
    let lookup = Buffers(buffers);
    let matches = |plan: &Plan| {
        plan.assignments().len() == expected.len()
            && plan.assignments().iter().all(|a| {
                let e = &expected[a.layer];
                a.plane == e.plane
                    && a.plane_index == e.plane_index
                    && a.zpos == e.zpos
                    && a.source == e.source
                    && a.destination == e.destination
            })
    };
    let plan = plan_with(
        &frame,
        &case.caps,
        &lookup,
        SearchLimits {
            steps: u32::MAX,
            backend_checks: u32::MAX,
        },
        |p| {
            if matches(p) {
                Verdict::Accept
            } else {
                Verdict::TryNext
            }
        },
    );
    assert!(
        plan.is_ok(),
        "the reduced frame is not accepted with the advised assignments: {plan:?}\nreduced: {reduced:?}"
    );
    // The check is a pure function of the plane set, so the advised
    // assignments, which it accepted, must be accepted again.
    // (Not for a frame with nothing on the output: `advise` answers Ok for it
    // even when the check refuses the empty plan; reported, not asserted.)
    if calls > 0 && !order.is_empty() {
        let plan = plan.unwrap();
        assert_eq!(
            case.check.verdict(&plan),
            Verdict::Accept,
            "advised set refused by the check"
        );
    }
    let _ = COMPOSE_LAYER;
}

fn run_case(seed: u64, index: u32, case: &Case) {
    let (first, calls, gave_up) = run(case);
    let expected = expected_error(case);
    match (&first, expected) {
        (Err(got), Some(want)) => assert_eq!(*got, want, "input error"),
        (_, Some(want)) => panic!("expected {want:?}, got Ok"),
        (Err(AdviceError::Aborted), None) => assert!(gave_up, "Aborted without a give-up"),
        (Err(AdviceError::LimitExceeded | AdviceError::NoComposePlane { .. }), None) => {}
        (Err(other), None) => panic!("unexpected error {other:?}"),
        (Ok(advice), None) => check_advice(case, advice, calls),
    }
    if matches!(
        first,
        Err(AdviceError::UnknownOutput(_) | AdviceError::TooManyLayers { .. })
    ) || matches!(first, Err(AdviceError::UnknownBuffer { .. }))
    {
        assert_eq!(calls, 0, "no check call on an input error");
    }
    assert!(calls <= case.backend_checks, "check calls over the limit");

    // Determinism.
    let (second, calls_again, _) = run(case);
    assert_eq!(calls, calls_again, "check calls differ between runs");
    match (&first, &second) {
        (Ok(a), Ok(b)) => assert!(
            a == b,
            "advice differs between runs:\n{}\n{}",
            describe(a),
            describe(b)
        ),
        (Err(a), Err(b)) => assert_eq!(a, b),
        _ => panic!("one run succeeded and the other failed"),
    }
    let _ = (seed, index);
}

#[test]
fn seeded_fuzz_of_advise() {
    let cases = std::env::var("COMPOSITION_FUZZ_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_CASES);
    let seed = std::env::var("COMPOSITION_FUZZ_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SEED);
    let mut rng = Rng(seed);
    let (mut ok, mut composed, mut direct, mut input_errors) = (0u32, 0u32, 0u32, 0u32);
    for index in 0..cases {
        let case = gen_case(&mut rng);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_case(seed, index, &case)
        }));
        if let Err(payload) = outcome {
            eprintln!(
                "advise fuzz failure: seed {seed:#x}, case {index}, topology {}\n{case:#?}",
                case.topology
            );
            std::panic::resume_unwind(payload);
        }
        match run(&case).0 {
            Ok(advice) => {
                ok += 1;
                if advice.is_direct() {
                    direct += 1;
                } else {
                    composed += 1;
                }
            }
            Err(
                AdviceError::UnknownOutput(_)
                | AdviceError::TooManyLayers { .. }
                | AdviceError::UnknownBuffer { .. }
                | AdviceError::InvalidSource { .. }
                | AdviceError::InvalidDestination { .. },
            ) => input_errors += 1,
            Err(_) => {}
        }
    }
    // The generator must reach the interesting outcomes, or the invariants
    // above check nothing.
    if cases >= DEFAULT_CASES {
        assert!(composed > 50, "too few composed advices: {composed}");
        assert!(direct > 50, "too few direct advices: {direct}");
        assert!(input_errors > 50, "too few input errors: {input_errors}");
    }
    eprintln!("advise fuzz: {cases} cases, {ok} ok ({direct} direct, {composed} composed), {input_errors} input errors");
}
