//! Seeded property checks of `composition_core::advise`
//! (`docs/advisory-planner-design.md` §2.1, §5 and §6).
//!
//! `tests/advise.rs` holds the reference-answer tests (minimality by brute
//! force, the reduced frame, fixtures). This file checks only properties that
//! must hold for any input: determinism, the bounds on the composed-layer
//! count, whole-call input errors and the contiguity of the composed run.
//!
//! Frames and capabilities come from a small xorshift generator with a fixed
//! seed; there is no dependency. A failure prints the case number and seed.
//! `ADVISE_PROPS_CASES=<n>` changes the number of cases.

use composition_core::limits::MAX_LAYERS;
use composition_core::planner::plan;
use composition_core::{
    advise, Advice, AdviceError, AdviseOptions, BlendMode, BufferId, BufferInfo, BufferLookup,
    Capabilities, ComposeTarget, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId,
    PixelFormat, Placement, PlaneCaps, PlaneId, PlaneKind, Rect, ScaleLimits, Size, Transform,
    Verdict, Zpos,
};

const SEED: u64 = 0x5eed_ad71_5e00_0001;
const CASES: u64 = 2000;
const OUT: OutputId = OutputId(0);
const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: u64) -> u64 {
        (self.next() >> 11) % n
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

struct Case {
    caps: Capabilities,
    buffers: Vec<(BufferId, BufferInfo)>,
    layers: Vec<Layer>,
    options: AdviseOptions,
}

impl BufferLookup for Case {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.buffers
            .iter()
            .find(|(id, _)| *id == buffer)
            .map(|(_, info)| *info)
    }
}

impl Case {
    fn frame(&self) -> Frame<'_> {
        Frame {
            output: OUT,
            layers: &self.layers,
        }
    }

    fn advise(&self) -> Result<Advice, AdviceError> {
        self.advise_counting(&mut 0)
    }

    fn advise_counting(&self, calls: &mut u32) -> Result<Advice, AdviceError> {
        advise(&self.frame(), &self.caps, self, &self.options, |_, _| {
            *calls += 1;
            Verdict::Accept
        })
    }
}

const FORMATS: [PixelFormat; 3] = [
    PixelFormat::XRGB8888,
    PixelFormat::ARGB8888,
    PixelFormat::RGB565,
];
const BLENDS: [BlendMode; 3] = [
    BlendMode::Opaque,
    BlendMode::Premultiplied,
    BlendMode::Coverage,
];

fn random_case(rng: &mut Xorshift) -> Case {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(WIDTH, HEIGHT),
            refresh_mhz: 60_000,
        })
        .unwrap();
    let plane_count = rng.range(1, 4) as u32;
    for i in 0..plane_count {
        let mut plane = PlaneCaps {
            id: PlaneId(10 + i),
            kind: if i == 0 {
                PlaneKind::Primary
            } else {
                PlaneKind::Overlay
            },
            outputs: 1,
            plane_alpha: rng.chance(70),
            ..PlaneCaps::default()
        };
        // XRGB8888 everywhere so that a composition target is placeable on
        // most hardware; the other formats vary.
        plane
            .formats
            .push(FormatModifier::linear(FORMATS[0]))
            .unwrap();
        for format in &FORMATS[1..] {
            if rng.chance(60) {
                plane.formats.push(FormatModifier::linear(*format)).unwrap();
            }
        }
        for blend in BLENDS {
            if rng.chance(80) {
                plane.blend_modes |= blend.mask();
            }
        }
        plane.blend_modes |= BlendMode::Opaque.mask();
        plane.transforms = Transform::Normal.mask();
        if rng.chance(50) {
            plane.scaling = Some(ScaleLimits {
                max_upscale: 0x1_0000 + rng.below(0x3_0000) as u32,
                max_downscale: 0x1_0000 + rng.below(0x3_0000) as u32,
            });
        }
        plane.zpos = if rng.chance(50) {
            Zpos::Range {
                min: 0,
                max: rng.range(0, 3) as i32,
            }
        } else {
            Zpos::Fixed(i as i32)
        };
        caps.planes.push(plane).unwrap();
    }

    let layer_count = rng.range(0, 6) as usize;
    let mut buffers = Vec::new();
    let mut layers = Vec::new();
    for i in 0..layer_count {
        let size = Size::new(rng.range(8, 64) as u32, rng.range(8, 48) as u32);
        let id = BufferId(1 + i as u32);
        buffers.push((
            id,
            BufferInfo {
                size,
                format: FORMATS[rng.below(3) as usize],
                modifier: Modifier::LINEAR,
            },
        ));
        let (dw, dh) = if rng.chance(60) {
            (size.width, size.height)
        } else {
            (rng.range(4, 64) as u32, rng.range(4, 48) as u32)
        };
        let dx = rng.range(0, (WIDTH - dw.min(WIDTH)) as i64) as i32;
        let dy = rng.range(0, (HEIGHT - dh.min(HEIGHT)) as i64) as i32;
        let mut layer = Layer::new(
            id,
            Rect::new(0, 0, size.width, size.height),
            Rect::new(dx, dy, dw, dh),
            rng.range(0, 4) as i32,
        );
        layer.blend = BLENDS[rng.below(3) as usize];
        if rng.chance(15) {
            // May be entirely off the output.
            layer.destination.x = rng.range(-80, 80) as i32;
        }
        if rng.chance(10) {
            layer.transform = Transform::Rotate180;
        }
        layers.push(layer);
    }
    let options = AdviseOptions {
        compose: Some(ComposeTarget {
            format: PixelFormat::XRGB8888,
            modifier: Modifier::LINEAR,
            blend: BlendMode::Opaque,
        }),
        ..AdviseOptions::default()
    };
    Case {
        caps,
        buffers,
        layers,
        options,
    }
}

fn cases() -> u64 {
    std::env::var("ADVISE_PROPS_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(CASES)
}

/// Runs `check` on every generated case with the case number as context.
fn for_each_case(check: impl Fn(&mut Xorshift, &Case)) {
    let mut rng = Xorshift(SEED);
    for n in 0..cases() {
        let case = random_case(&mut rng);
        // Each check gets its own stream so one check's draws never shift
        // the cases of another.
        let mut local = Xorshift(SEED ^ (n + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        check(&mut local, &case);
    }
}

/// Positions in stacking order (bottom first, equal z in frame order) of the
/// layers the advice does not call hidden.
fn stacking(case: &Case, advice: &Advice) -> Vec<usize> {
    let mut order: Vec<usize> = (0..case.layers.len())
        .filter(|&i| advice.layers()[i].placement != Placement::Hidden)
        .collect();
    order.sort_by_key(|&i| (case.layers[i].z_order, i));
    order
}

#[test]
fn advice_is_deterministic() {
    for_each_case(|_, case| {
        let first = case.advise();
        let second = case.advise();
        assert_eq!(first, second, "determinism, seed {SEED:#x}");
    });
}

#[test]
fn composed_count_is_bounded_and_zero_when_all_fit() {
    let mut direct = 0;
    let mut composed = 0;
    for_each_case(|_, case| {
        let direct_plan = plan(&case.frame(), &case.caps, case);
        let Ok(advice) = case.advise() else { return };
        let count = advice
            .layers()
            .iter()
            .filter(|l| l.placement == Placement::Compose)
            .count();
        assert!(count <= case.layers.len(), "seed {SEED:#x}");
        assert_eq!(advice.layers().len(), case.layers.len());
        if direct_plan.is_ok() {
            // Every layer fits on a plane: nothing is composed, and the
            // planner's own assignments are the advice.
            assert_eq!(count, 0, "seed {SEED:#x}");
            assert!(advice.is_direct());
            assert!(advice.compose().is_none() && advice.compose_run().is_none());
            assert_eq!(advice.candidates(), 1);
        } else {
            // The frame as it is fails, so something has to be composed.
            assert!(count >= 1, "seed {SEED:#x}");
            assert!(!advice.is_direct());
        }
        // The composed layers are not on planes.
        assert_eq!(advice.promoted() + count, advice_visible(&advice));
    });
    // Count the mix once, so a generator that never exercises one side
    // fails loudly.
    let mut rng = Xorshift(SEED);
    for _ in 0..cases() {
        let case = random_case(&mut rng);
        if let Ok(advice) = case.advise() {
            if advice.is_direct() {
                direct += 1;
            } else {
                composed += 1;
            }
        }
    }
    assert!(
        direct > 50 && composed > 50,
        "direct {direct} composed {composed}"
    );
}

fn advice_visible(advice: &Advice) -> usize {
    advice
        .layers()
        .iter()
        .filter(|l| l.placement != Placement::Hidden)
        .count()
}

#[test]
fn input_errors_are_whole_call_and_call_no_check() {
    for_each_case(|rng, case| {
        if case.layers.is_empty() {
            return;
        }
        let victim = rng.below(case.layers.len() as u64) as usize;
        let mut bad = Case {
            caps: case.caps,
            buffers: case.buffers.clone(),
            layers: case.layers.clone(),
            options: case.options,
        };
        let expected = match rng.below(4) {
            0 => {
                bad.layers[victim].buffer = BufferId(0xffff);
                AdviceError::UnknownBuffer {
                    layer: victim,
                    buffer: BufferId(0xffff),
                }
            }
            1 => {
                bad.layers[victim].source = Rect::new(0, 0, 0, 0);
                AdviceError::InvalidSource { layer: victim }
            }
            2 => {
                bad.layers[victim].source.width += 1000;
                AdviceError::InvalidSource { layer: victim }
            }
            _ => {
                bad.layers[victim].destination.height = 0;
                AdviceError::InvalidDestination { layer: victim }
            }
        };
        // The first bad layer in frame order is the one reported; the
        // generator's other layers are valid, so it is the victim.
        let mut calls = 0;
        let result = bad.advise_counting(&mut calls);
        assert_eq!(result, Err(expected), "seed {SEED:#x}");
        assert_eq!(calls, 0, "the check ran before the input was refused");
    });
}

#[test]
fn call_level_errors_are_whole_call() {
    for_each_case(|_, case| {
        // Unknown output.
        let frame = Frame {
            output: OutputId(9),
            layers: &case.layers,
        };
        let mut calls = 0;
        let result = advise(&frame, &case.caps, case, &case.options, |_, _| {
            calls += 1;
            Verdict::Accept
        });
        assert_eq!(result, Err(AdviceError::UnknownOutput(OutputId(9))));
        assert_eq!(calls, 0);

        // One layer too many.
        let mut many = case.layers.clone();
        let filler = Layer::new(
            BufferId(0xf000),
            Rect::new(0, 0, 8, 8),
            Rect::new(0, 0, 8, 8),
            0,
        );
        many.resize(MAX_LAYERS + 1, filler);
        let frame = Frame {
            output: OUT,
            layers: &many,
        };
        let result = advise(&frame, &case.caps, case, &case.options, |_, _| {
            calls += 1;
            Verdict::Accept
        });
        assert_eq!(
            result,
            Err(AdviceError::TooManyLayers {
                count: MAX_LAYERS + 1
            })
        );
        assert_eq!(calls, 0);
    });
}

#[test]
fn composed_layers_form_one_contiguous_run() {
    for_each_case(|_, case| {
        let Ok(advice) = case.advise() else { return };
        let order = stacking(case, &advice);
        let composed: Vec<usize> = order
            .iter()
            .enumerate()
            .filter(|(_, &i)| advice.layers()[i].placement == Placement::Compose)
            .map(|(position, _)| position)
            .collect();
        match advice.compose_run() {
            None => assert!(composed.is_empty(), "composed layers without a run"),
            Some((low, high)) => {
                assert!(low <= high && high < order.len());
                // Exactly the positions low..=high, with no promoted layer
                // inside.
                let run: Vec<usize> = (low..=high).collect();
                assert_eq!(composed, run, "seed {SEED:#x}");
                // The target takes one plane assignment of its own.
                assert!(advice.compose().is_some());
            }
        }
        // Everything outside the run is on a plane; hidden layers are
        // neither.
        for (position, &index) in order.iter().enumerate() {
            let in_run = advice
                .compose_run()
                .is_some_and(|(low, high)| low <= position && position <= high);
            let placement = advice.layers()[index].placement;
            if in_run {
                assert_eq!(placement, Placement::Compose);
            } else {
                assert!(matches!(placement, Placement::Plane(_)), "seed {SEED:#x}");
            }
        }
    });
}
