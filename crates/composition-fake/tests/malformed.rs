//! Deterministic, dependency-free fuzzing of the engine with malformed frame
//! descriptions, against the fake backend.
//!
//! A xorshift generator with fixed seeds produces frames with zero or extreme
//! sizes, unknown formats, unknown or removed buffers, extreme positions,
//! duplicate z positions and too many layers. For each frame an independent
//! oracle says whether the description is invalid; then the engine must not
//! panic, must refuse invalid frames with an explicit error, must only accept
//! frames whose plan is consistent, and must still commit a valid frame
//! afterwards.
//!
//! `BufferInfo` has no pitch field (strides belong to platform code), so zero
//! or overflowing pitches are not expressible here; the extreme buffer sizes
//! below cover the corresponding arithmetic in the core.

use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, Engine, Error, FormatModifier, Frame, Layer,
    Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    Size, Transform, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const OUT_SIZE: Size = Size::new(1280, 720);
const FULL: Rect = Rect::new(0, 0, 1280, 720);

const GOOD_RGB: BufferId = BufferId(1);
const GOOD_ARGB: BufferId = BufferId(2);
const WEIRD_FORMAT: BufferId = BufferId(3);
const ZERO_SIZE: BufferId = BufferId(4);
const HUGE: BufferId = BufferId(5);
const TINY: BufferId = BufferId(6);
const REMOVED: BufferId = BufferId(7);
const UNKNOWN: BufferId = BufferId(1000);
const MAX_ID: BufferId = BufferId(u32::MAX);

const BUFFERS: [BufferId; 9] = [
    GOOD_RGB,
    GOOD_ARGB,
    WEIRD_FORMAT,
    ZERO_SIZE,
    HUGE,
    TINY,
    REMOVED,
    UNKNOWN,
    MAX_ID,
];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }

    fn i32(&mut self) -> i32 {
        const EDGE: [i32; 9] = [
            i32::MIN,
            i32::MIN + 1,
            -1,
            0,
            1,
            1279,
            1280,
            i32::MAX - 1,
            i32::MAX,
        ];
        match self.below(4) {
            0 => self.pick(&EDGE),
            1 => self.below(3000) as i32 - 1000,
            2 => self.next() as i32,
            _ => self.below(1300) as i32,
        }
    }

    fn u32(&mut self) -> u32 {
        const EDGE: [u32; 9] = [0, 1, 2, 719, 720, 1280, 1281, u32::MAX - 1, u32::MAX];
        match self.below(4) {
            0 => self.pick(&EDGE),
            1 => self.below(1500) as u32,
            2 => self.next() as u32,
            _ => 1 + self.below(1280) as u32,
        }
    }

    fn rect(&mut self) -> Rect {
        Rect::new(self.i32(), self.i32(), self.u32(), self.u32())
    }
}

fn plane(id: u32, kind: PlaneKind, zpos: i32, format: PixelFormat) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs: 1,
        zpos: Zpos::Fixed(zpos),
        blend_modes: BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask(),
        ..PlaneCaps::default()
    };
    caps.formats.push(FormatModifier::linear(format)).unwrap();
    caps
}

fn caps() -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: OUT_SIZE,
            refresh_mhz: 60_000,
        })
        .unwrap();
    caps.planes
        .push(plane(31, PlaneKind::Primary, 0, PixelFormat::RGB565))
        .unwrap();
    caps.planes
        .push(plane(41, PlaneKind::Overlay, 1, PixelFormat::ARGB8888))
        .unwrap();
    caps
}

fn engine() -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(caps()));
    let table = [
        (GOOD_RGB, OUT_SIZE, PixelFormat::RGB565),
        (GOOD_ARGB, OUT_SIZE, PixelFormat::ARGB8888),
        (WEIRD_FORMAT, OUT_SIZE, PixelFormat(0xDEAD_BEEF)),
        (ZERO_SIZE, Size::new(0, 0), PixelFormat::ARGB8888),
        (HUGE, Size::new(u32::MAX, u32::MAX), PixelFormat::ARGB8888),
        (TINY, Size::new(1, 1), PixelFormat::RGB565),
        (REMOVED, OUT_SIZE, PixelFormat::RGB565),
        (MAX_ID, OUT_SIZE, PixelFormat::ARGB8888),
    ];
    for (id, size, format) in table {
        let info = BufferInfo {
            size,
            format,
            modifier: Modifier::LINEAR,
        };
        engine.register_buffer(id, info).unwrap();
        engine.finish_write(id).unwrap();
    }
    engine.unregister_buffer(REMOVED).unwrap();
    engine
}

fn random_layer(rng: &mut Rng) -> Layer {
    let mut layer = Layer::new(rng.pick(&BUFFERS), rng.rect(), rng.rect(), 0);
    // Often well-formed geometry, so that frames get past the first checks.
    if rng.below(2) == 0 {
        layer.source = Rect::new(0, 0, 1 + rng.below(1280) as u32, 1 + rng.below(720) as u32);
    }
    if rng.below(2) == 0 {
        layer.destination = Rect::new(
            rng.below(1400) as i32 - 100,
            rng.below(800) as i32 - 50,
            1 + rng.below(1400) as u32,
            1 + rng.below(800) as u32,
        );
    }
    // Core bug (see `clipping_overflows_i64_on_extreme_scale`): clipping
    // multiplies source and destination extents in i64. Keep the fuzzer clear
    // of it so that it can look for other defects.
    let wide = |source: u32, destination: u32| source as u64 * destination as u64 >= 1 << 62;
    if wide(layer.source.width, layer.destination.width) {
        layer.source.width = 1 << 20;
    }
    if wide(layer.source.height, layer.destination.height) {
        layer.source.height = 1 << 20;
    }
    layer.z_order = match rng.below(3) {
        0 => rng.i32(),
        1 => rng.below(2) as i32, // duplicates
        _ => 0,
    };
    layer.alpha = rng.next() as u16;
    layer.blend = rng.pick(&[
        BlendMode::Opaque,
        BlendMode::Premultiplied,
        BlendMode::Coverage,
    ]);
    layer.transform = rng.pick(&[
        Transform::Normal,
        Transform::Rotate90,
        Transform::Rotate180,
        Transform::Rotate270,
        Transform::FlipH,
        Transform::FlipV,
    ]);
    layer
}

/// Size of a buffer as the fixture registered it; `None` if not registered.
fn registered_size(buffer: BufferId) -> Option<Size> {
    match buffer {
        ZERO_SIZE => Some(Size::new(0, 0)),
        HUGE => Some(Size::new(u32::MAX, u32::MAX)),
        TINY => Some(Size::new(1, 1)),
        REMOVED | UNKNOWN => None,
        _ => Some(OUT_SIZE),
    }
}

/// Independent statement of when the description alone is invalid.
fn is_malformed(output: OutputId, layers: &[Layer]) -> bool {
    output != OUT
        || layers.len() > 16
        || layers
            .iter()
            .any(|layer| match registered_size(layer.buffer) {
                None => true,
                Some(size) => {
                    layer.source.is_empty()
                        || !layer.source.fits_within(size)
                        || layer.destination.is_empty()
                }
            })
}

/// What a plan the engine produced must satisfy whatever the frame was.
fn check_plan(layers: &[Layer], plan: &composition_core::Plan) {
    let mut planes = Vec::new();
    for a in plan.assignments() {
        assert!(a.layer < layers.len());
        assert!(!planes.contains(&a.plane), "plane used twice: {plan:?}");
        planes.push(a.plane);
        assert!(!a.source.is_empty() && !a.destination.is_empty());
        let size = registered_size(layers[a.layer].buffer).expect("unregistered buffer planned");
        assert!(a.source.fits_within(size), "source outside buffer: {a:?}");
        assert!(
            a.destination.fits_within(OUT_SIZE),
            "destination off output: {a:?}"
        );
        let plane_format = if a.plane == PlaneId(31) {
            PixelFormat::RGB565
        } else {
            PixelFormat::ARGB8888
        };
        let buffer_format = match layers[a.layer].buffer {
            GOOD_RGB | TINY => PixelFormat::RGB565,
            WEIRD_FORMAT => PixelFormat(0xDEAD_BEEF),
            _ => PixelFormat::ARGB8888,
        };
        assert_eq!(plane_format, buffer_format, "format mismatch: {a:?}");
    }
}

fn settle(engine: &mut Engine<FakeBackend>) {
    // Two vertical blanks: whatever is pending latches, and the previous
    // frame's buffers are released.
    for _ in 0..2 {
        engine.backend_mut().vsync();
        while engine.poll_event().unwrap().is_some() {}
    }
}

fn valid_frame_commits(engine: &mut Engine<FakeBackend>, context: &str) {
    let layers = [
        Layer::new(GOOD_RGB, FULL, FULL, 0),
        Layer::new(
            GOOD_ARGB,
            Rect::new(0, 0, 200, 100),
            Rect::new(10, 10, 200, 100),
            5,
        ),
    ];
    settle(engine);
    let frame = Frame {
        output: OUT,
        layers: &layers,
    };
    let plan = engine
        .validate(&frame)
        .unwrap_or_else(|e| panic!("valid frame refused ({e:?}) after {context}"));
    engine
        .commit(&frame, &plan)
        .unwrap_or_else(|e| panic!("valid frame not committed ({e:?}) after {context}"));
}

fn run(seed: u64, iterations: usize) {
    let mut rng = Rng(seed);
    let mut engine = engine();
    let (mut refused, mut accepted) = (0, 0);
    for i in 0..iterations {
        let count = match rng.below(10) {
            0 => 17 + rng.below(40) as usize,
            1 => 0,
            _ => 1 + rng.below(4) as usize,
        };
        let layers: Vec<Layer> = (0..count).map(|_| random_layer(&mut rng)).collect();
        let output = if rng.below(20) == 0 {
            OutputId(rng.next() as u32 | 1)
        } else {
            OUT
        };
        let frame = Frame {
            output,
            layers: &layers,
        };
        let malformed = is_malformed(output, &layers);
        let context = format!("seed {seed:#x} iteration {i}: {frame:?}");
        match engine.validate(&frame) {
            Ok(plan) => {
                assert!(!malformed, "malformed frame accepted, {context}");
                accepted += 1;
                check_plan(&layers, &plan);
                // Commit may be refused (a submission is pending, a buffer is
                // in use), but never with a planning error.
                if let Err(Error::Plan(error)) = engine.commit(&frame, &plan) {
                    panic!("planning error {error:?} at commit, {context}");
                }
                settle(&mut engine);
            }
            Err(error) => {
                refused += 1;
                if malformed {
                    assert!(matches!(error, Error::Plan(_)), "{error:?}, {context}");
                }
            }
        }
        // A malformed frame is refused by `present` too and never reaches the
        // hardware.
        if malformed {
            let before = engine.backend().commit_count();
            assert!(engine.present(&frame).is_err(), "{context}");
            assert_eq!(engine.backend().commit_count(), before, "{context}");
        }
        if i % 64 == 0 {
            valid_frame_commits(&mut engine, &context);
        }
    }
    valid_frame_commits(&mut engine, "the last iteration");
    assert!(
        refused > iterations / 4,
        "generator too tame: {refused} refused"
    );
    assert!(accepted > 0, "generator never produced a valid frame");
}

#[test]
fn fuzz_seed_1() {
    run(0x9E37_79B9_7F4A_7C15, 20_000);
}

#[test]
fn fuzz_seed_2() {
    run(0x0123_4567_89AB_CDEF, 20_000);
}

#[test]
fn fuzz_seed_3() {
    run(0xDEAD_BEEF_CAFE_F00D, 20_000);
}

#[test]
fn each_defect_class_is_refused_explicitly() {
    let mut engine = engine();
    let huge_source = Rect::new(i32::MAX, i32::MAX, u32::MAX, u32::MAX);
    let cases: Vec<(&str, Vec<Layer>)> = vec![
        (
            "zero source",
            vec![Layer::new(GOOD_RGB, Rect::new(0, 0, 0, 5), FULL, 0)],
        ),
        (
            "zero destination",
            vec![Layer::new(GOOD_RGB, FULL, Rect::new(0, 0, 5, 0), 0)],
        ),
        (
            "oversize source",
            vec![Layer::new(GOOD_RGB, Rect::new(0, 0, 1281, 720), FULL, 0)],
        ),
        (
            "source overflow",
            vec![Layer::new(GOOD_RGB, huge_source, FULL, 0)],
        ),
        (
            "negative source",
            vec![Layer::new(GOOD_RGB, Rect::new(-1, 0, 10, 10), FULL, 0)],
        ),
        (
            "zero-size buffer",
            vec![Layer::new(ZERO_SIZE, FULL, FULL, 0)],
        ),
        ("unknown buffer", vec![Layer::new(UNKNOWN, FULL, FULL, 0)]),
        ("removed buffer", vec![Layer::new(REMOVED, FULL, FULL, 0)]),
        (
            "too many layers",
            vec![Layer::new(GOOD_RGB, FULL, FULL, 0); 17],
        ),
        (
            "unknown format",
            vec![Layer::new(WEIRD_FORMAT, FULL, FULL, 0)],
        ),
    ];
    for (name, layers) in cases {
        let frame = Frame {
            output: OUT,
            layers: &layers,
        };
        let error = engine.validate(&frame).expect_err(name);
        assert!(matches!(error, Error::Plan(_)), "{name}: {error:?}");
        assert!(engine.present(&frame).is_err(), "{name}");
        valid_frame_commits(&mut engine, name);
    }
    let layers = [Layer::new(GOOD_RGB, FULL, FULL, 0)];
    let frame = Frame {
        output: OutputId(9),
        layers: &layers,
    };
    assert_eq!(
        engine.validate(&frame).unwrap_err(),
        Error::Plan(PlanError::UnknownOutput(OutputId(9)))
    );
}

#[test]
fn bad_buffer_table_operations_are_refused_and_harmless() {
    let mut engine = engine();
    let again = BufferInfo {
        size: OUT_SIZE,
        format: PixelFormat::RGB565,
        modifier: Modifier::LINEAR,
    };
    assert!(engine.register_buffer(GOOD_RGB, again).is_err());
    assert!(engine.register_buffer(GOOD_RGB, again).is_err());
    assert!(engine.unregister_buffer(UNKNOWN).is_err());
    assert!(engine.finish_write(UNKNOWN).is_err());
    assert!(engine.begin_write(UNKNOWN).is_err());
    valid_frame_commits(&mut engine, "bad table operations");
}

/// Bug: `planner::clip` computes `near * length` in i64 with both factors up
/// to `u32::MAX`, so a huge flipped layer cut at one edge overflows (panic in
/// debug builds, wrong clip in release). Should be an `Err`, not a panic.
#[test]
#[ignore = "bug: planner clip overflows i64 for extreme source and destination extents"]
fn clipping_overflows_i64_on_extreme_scale() {
    let mut engine = engine();
    let mut layer = Layer::new(
        HUGE,
        Rect::new(0, 0, u32::MAX, 720),
        Rect::new(-2_000_000_000, 0, u32::MAX, 720),
        0,
    );
    layer.transform = Transform::FlipH;
    let layers = [layer];
    let frame = Frame {
        output: OUT,
        layers: &layers,
    };
    assert!(engine.validate(&frame).is_err());
}
