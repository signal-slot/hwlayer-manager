//! Planner and fake-engine test on the observed BCM2711 (Raspberry Pi CM4,
//! reTerminal, vc4 / HVS5) topology.
//!
//! This is a MODEL of the topology recorded in
//! `docs/observed/bcm2711-cm4-topology.md` and `docs/observed/bcm2711-cm4.md`
//! (observed 2026-11-28, kernel 6.12.47+rpt-rpi-v8). It is not a hardware
//! result: the fake backend has no HVS, no kernel and no `TEST_ONLY`. What
//! the tests show is that composition-core's planner and engine, given the
//! capabilities composition-drm built on that board, accept what the board's
//! `TEST_ONLY` accepted and refuse or fall back on what its capability
//! detection and planner refused (SoW.md section 6).
//!
//! The model is the board's "probe's cut view": 16 planes (the six primaries
//! and the first ten overlays, `MAX_PLANES`), of which 11 reach output 0 (the
//! DSI panel, 720x1280 at 60 Hz). The kernel's real 17-plane view is not
//! modelled. The capabilities are what composition-drm reported: the same 32
//! formats on every plane, linear only, no scaling, no transforms, plane
//! alpha, three blend modes, primary zpos fixed 0, overlays zpos 1..=17.
//!
//! The kernel accepted scaling in `TEST_ONLY` where the capabilities claimed
//! none. That is modelled only as "with scaling claimed, the planner accepts";
//! the kernel's answer itself is the board's and is not reproduced.

use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, DisplayEvent, Engine, Error, FormatModifier,
    Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId,
    PlaneKind, Rect, RejectReasons, ScaleLimits, Size, Transform, Zpos,
};
use composition_fake::{FakeBackend, PlaneState};

const OUT: OutputId = OutputId(0);
const W: u32 = 720;
const H: u32 = 1280;
const OUTPUT_RECT: Rect = Rect::new(0, 0, W, H);
/// The overlay of the harness: 144x640 at 360,320.
const OVERLAY_SIZE: Size = Size::new(144, 640);
const OVERLAY_RECT: Rect = Rect::new(360, 320, 144, 640);

const XR24: PixelFormat = PixelFormat::XRGB8888;
const AR24: PixelFormat = PixelFormat::ARGB8888;

const PRIMARY_IDS: [u32; 6] = [48, 67, 79, 91, 103, 115];
/// Primary of the DSI CRTC (pixelvalve-1).
const PRIMARY: PlaneId = PlaneId(79);
const OVERLAY_IDS: [u32; 10] = [127, 138, 149, 160, 171, 182, 193, 204, 215, 226];
const FIRST_OVERLAY: PlaneId = PlaneId(127);

/// The 32 formats probe printed, on every plane.
const FORMATS: [&[u8; 4]; 32] = [
    b"XR24", b"AR24", b"AB24", b"XB24", b"RG16", b"BG16", b"AR15", b"XR15", b"RG24", b"BG24",
    b"YU16", b"YV16", b"YU24", b"YV24", b"YU12", b"YV12", b"NV12", b"NV21", b"NV16", b"NV61",
    b"XR30", b"AR30", b"AB30", b"XB30", b"RGB8", b"BGR8", b"XR12", b"AR12", b"XB12", b"AB12",
    b"BX12", b"BA12",
];

fn plane(id: u32, kind: PlaneKind, outputs: u32, scaling: Option<ScaleLimits>) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs,
        scaling,
        plane_alpha: true,
        blend_modes: BlendMode::Opaque.mask()
            | BlendMode::Premultiplied.mask()
            | BlendMode::Coverage.mask(),
        transforms: 0,
        zpos: match kind {
            PlaneKind::Overlay => Zpos::Range { min: 1, max: 17 },
            _ => Zpos::Fixed(0),
        },
        ..PlaneCaps::default()
    };
    for code in FORMATS {
        caps.formats
            .push(FormatModifier::linear(PixelFormat::fourcc(code)))
            .unwrap();
    }
    caps
}

/// Probe's 16 planes of the board, with the given scaling claim.
fn caps_with(scaling: Option<ScaleLimits>) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(W, H),
            refresh_mhz: 60_000,
        })
        .unwrap();
    for id in PRIMARY_IDS {
        // Only primary 79 reaches the DSI CRTC; the others belong to
        // unconnected CRTCs (probe mask 0b0).
        let outputs = u32::from(PlaneId(id) == PRIMARY);
        caps.planes
            .push(plane(id, PlaneKind::Primary, outputs, scaling))
            .unwrap();
    }
    for id in OVERLAY_IDS {
        caps.planes
            .push(plane(id, PlaneKind::Overlay, 1, scaling))
            .unwrap();
    }
    caps
}

/// What composition-drm reported: no scaling claimed.
fn board_caps() -> Capabilities {
    caps_with(None)
}

fn register_info(engine: &mut Engine<FakeBackend>, id: BufferId, info: BufferInfo) {
    engine.register_buffer(id, info).unwrap();
    engine.finish_write(id).unwrap();
}

fn register(engine: &mut Engine<FakeBackend>, id: BufferId, size: Size, format: PixelFormat) {
    let info = BufferInfo {
        size,
        format,
        modifier: Modifier::LINEAR,
    };
    register_info(engine, id, info);
}

const BG: BufferId = BufferId(1);
const OV: BufferId = BufferId(2);
const ODD: BufferId = BufferId(3);

fn background(id: BufferId) -> Layer {
    Layer::new(id, OUTPUT_RECT, OUTPUT_RECT, 0)
}

fn overlay(id: BufferId) -> Layer {
    let source = Rect::new(0, 0, OVERLAY_SIZE.width, OVERLAY_SIZE.height);
    let mut layer = Layer::new(id, source, OVERLAY_RECT, 1);
    layer.blend = BlendMode::Premultiplied;
    layer
}

fn drain(engine: &mut Engine<FakeBackend>) -> Vec<DisplayEvent> {
    let mut events = Vec::new();
    while let Some(event) = engine.poll_event().unwrap() {
        events.push(event);
    }
    events
}

fn validate(
    engine: &mut Engine<FakeBackend>,
    layers: &[Layer],
) -> Result<composition_core::Plan, PlanError> {
    match engine.validate(&Frame {
        output: OUT,
        layers,
    }) {
        Ok(plan) => Ok(plan),
        Err(Error::Plan(error)) => Err(error),
        Err(other) => panic!("unexpected error {other:?}"),
    }
}

fn no_plane(layer: usize, why: RejectReasons) -> PlanError {
    PlanError::NoPlane {
        layer,
        reasons: RejectReasons::OUTPUT.union(why),
    }
}

/// Engine with a harness-like background (XR24, full output) and overlay
/// (AR24, 144x640).
fn engine_with(caps: Capabilities) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(caps));
    register(&mut engine, BG, Size::new(W, H), XR24);
    register(&mut engine, OV, OVERLAY_SIZE, AR24);
    engine
}

// Rows "Planes probe reports", "Planes usable on output 0", "Plane -> CRTC
// masks", "zpos", "Formats" (bcm2711-cm4.md, "Topology").
#[test]
fn model_matches_the_probe_output() {
    let caps = board_caps();
    assert_eq!(caps.outputs.len(), 1);
    assert_eq!(caps.planes.len(), 16);
    let on_output: Vec<_> = caps.planes.iter().filter(|p| p.outputs & 1 != 0).collect();
    assert_eq!(on_output.len(), 11, "primary 79 plus ten overlays");
    assert_eq!(on_output[0].id, PRIMARY);
    assert!(on_output.iter().all(|p| p.formats.len() == 32));
    let engine = Engine::new(FakeBackend::new(caps));
    assert_eq!(*engine.capabilities(), caps);
}

// Row "Two-plane harness / Plan": layer 0 -> plane 79 Primary zpos 0, layer 1
// -> plane 127 Overlay zpos 1; every frame passed TEST_ONLY.
#[test]
fn two_plane_plan_is_primary_plus_first_overlay() {
    let mut engine = engine_with(board_caps());
    let layers = [background(BG), overlay(OV)];
    let plan = validate(&mut engine, &layers).unwrap();
    let a = plan.assignments();
    assert_eq!(a.len(), 2);
    assert_eq!((a[0].plane, a[0].zpos), (PRIMARY, 0));
    assert_eq!(a[0].destination, OUTPUT_RECT);
    assert_eq!((a[1].plane, a[1].zpos), (FIRST_OVERLAY, 1));
    assert_eq!(a[1].destination, OVERLAY_RECT);
}

// Row "Default run" (600 committed and latched, 1198 released + 2 at the end,
// 0 dropped, three buffers per layer rotated), scaled down to 60 frames.
#[test]
fn two_plane_buffer_rotation_releases_every_use_once() {
    const FRAMES: usize = 60;
    const PER_LAYER: usize = 3;
    let mut engine = Engine::new(FakeBackend::new(board_caps()));
    let bg: Vec<_> = (0..PER_LAYER).map(|i| BufferId(10 + i as u32)).collect();
    let ov: Vec<_> = (0..PER_LAYER).map(|i| BufferId(20 + i as u32)).collect();
    for id in &bg {
        register(&mut engine, *id, Size::new(W, H), XR24);
    }
    for id in &ov {
        register(&mut engine, *id, OVERLAY_SIZE, AR24);
    }
    let (mut presented, mut released, mut dropped) = (0, 0, 0);
    let mut tally = |events: Vec<DisplayEvent>| {
        for event in events {
            match event {
                DisplayEvent::Presented { .. } => presented += 1,
                DisplayEvent::Released { .. } => released += 1,
                DisplayEvent::Dropped { .. } => dropped += 1,
                _ => {}
            }
        }
    };
    for frame in 0..FRAMES {
        let (b, o) = (bg[frame % PER_LAYER], ov[frame % PER_LAYER]);
        engine
            .present(&Frame {
                output: OUT,
                layers: &[background(b), overlay(o)],
            })
            .unwrap();
        engine.backend_mut().vsync();
        tally(drain(&mut engine));
        assert_eq!(
            engine.backend().scanout(OUT).unwrap(),
            [
                PlaneState {
                    plane: PRIMARY,
                    buffer: b,
                    zpos: 0
                },
                PlaneState {
                    plane: FIRST_OVERLAY,
                    buffer: o,
                    zpos: 1
                },
            ]
        );
    }
    // Blanking releases the last two uses (the "+ 2 at the end").
    engine
        .present(&Frame {
            output: OUT,
            layers: &[],
        })
        .unwrap();
    engine.backend_mut().vsync();
    tally(drain(&mut engine));
    assert_eq!(presented, FRAMES + 1);
    assert_eq!(released, 2 * FRAMES);
    assert_eq!(dropped, 0);
}

// Row "--swap-z run": the harness printed `SWAPZ skipped: a plane of the plan
// has a fixed zpos`. The planner is asked for the large layer above the small
// one. The planner can still honour that by exchanging the planes: the
// harness only skips because it swaps zpos on the planes it already chose.
#[test]
fn swapped_z_order_puts_the_lower_layer_on_the_fixed_primary() {
    let mut engine = engine_with(board_caps());
    let mut bg = background(BG);
    bg.z_order = 5;
    let layers = [bg, overlay(OV)];
    let plan = validate(&mut engine, &layers).unwrap();
    let a = plan.assignments();
    let bg_a = a.iter().find(|x| x.layer == 0).unwrap();
    let ov_a = a.iter().find(|x| x.layer == 1).unwrap();
    assert!(bg_a.zpos > ov_a.zpos, "layer 0 must end above layer 1");
    // The fixed zpos-0 primary can only carry the lower layer; the upper one
    // goes on an overlay (the harness instead skipped the swap).
    assert_eq!((ov_a.plane, ov_a.zpos), (PRIMARY, 0));
    assert!(bg_a.plane != PRIMARY);
}

// Rows "plane alpha", "blend coverage", "offscreen", "odd" of "Unsupported
// combinations": capabilities accept, planner plans, TEST_ONLY ok.
#[test]
fn plane_alpha_coverage_offscreen_and_odd_geometry_are_accepted() {
    let mut engine = engine_with(board_caps());

    let mut ov = overlay(OV);
    ov.alpha = 0x8000;
    ov.blend = BlendMode::Coverage;
    assert!(validate(&mut engine, &[background(BG), ov]).is_ok());

    // The planner clips to the output.
    let mut off = overlay(OV);
    off.destination = Rect::new(W as i32 - 72, 320, 144, 640);
    let plan = validate(&mut engine, &[background(BG), off]).unwrap();
    assert_eq!(plan.assignments()[1].destination.width, 72);

    // The caps model has no alignment rule.
    let mut odd = overlay(OV);
    odd.source = Rect::new(1, 1, 143, 639);
    odd.destination = Rect::new(361, 321, 143, 639);
    assert!(validate(&mut engine, &[background(BG), odd]).is_ok());
}

// Row "Unknown format HWL0": capabilities refuse, planner NoPlane
// OUTPUT|FORMAT, TEST_ONLY not reached.
#[test]
fn unknown_format_is_refused_by_format() {
    let mut engine = engine_with(board_caps());
    register(&mut engine, ODD, OVERLAY_SIZE, PixelFormat::fourcc(b"HWL0"));
    assert_eq!(
        validate(&mut engine, &[background(BG), overlay(ODD)]).unwrap_err(),
        no_plane(1, RejectReasons::FORMAT)
    );
    assert_eq!(engine.backend().check_count(), 0, "TEST_ONLY not reached");
}

// Row "Modifier I915_X_TILED": capabilities refuse AR24 with that modifier,
// planner NoPlane OUTPUT|FORMAT (every plane lists linear only).
#[test]
fn tiled_modifier_is_refused_by_format() {
    let mut engine = engine_with(board_caps());
    let tiled = BufferInfo {
        size: OVERLAY_SIZE,
        format: AR24,
        modifier: Modifier(0x0100_0000_0000_0001), // I915_FORMAT_MOD_X_TILED
    };
    register_info(&mut engine, ODD, tiled);
    assert_eq!(
        validate(&mut engine, &[background(BG), overlay(ODD)]).unwrap_err(),
        no_plane(1, RejectReasons::FORMAT)
    );
}

// Row "12 layers": capabilities refuse (11 planes on the output), planner
// NoPlane OUTPUT|IN_USE at layer 11. Eleven layers fit.
#[test]
fn more_layers_than_planes_on_the_output_is_refused() {
    let mut engine = Engine::new(FakeBackend::new(board_caps()));
    let mut layers = Vec::new();
    for i in 0..12u32 {
        let id = BufferId(100 + i);
        register(&mut engine, id, Size::new(64, 64), AR24);
        let at = i as i32 * 8;
        let mut layer = Layer::new(
            id,
            Rect::new(0, 0, 64, 64),
            Rect::new(at, at, 64, 64),
            i as i32,
        );
        layer.blend = BlendMode::Premultiplied;
        layers.push(layer);
    }
    assert_eq!(
        validate(&mut engine, &layers[..11])
            .unwrap()
            .assignments()
            .len(),
        11
    );
    assert_eq!(
        validate(&mut engine, &layers).unwrap_err(),
        no_plane(11, RejectReasons::IN_USE)
    );
}

// Rows "Scale-up", "Scale-down": capabilities refuse (nothing claims scaling),
// planner NoPlane OUTPUT|SCALING, while the kernel's TEST_ONLY said ok for a
// plan with scaling claimed up and down by 16. With that claim the planner
// accepts both.
#[test]
fn scaling_is_refused_unless_claimed_and_accepted_when_claimed() {
    let scaled_up = || {
        let mut layer = overlay(OV);
        layer.source = Rect::new(0, 0, 72, 320);
        [background(BG), layer]
    };
    let scaled_down = || {
        let mut layer = overlay(OV);
        layer.destination = Rect::new(360, 320, 72, 320);
        [background(BG), layer]
    };

    let mut engine = engine_with(board_caps());
    for layers in [scaled_up(), scaled_down()] {
        assert_eq!(
            validate(&mut engine, &layers).unwrap_err(),
            no_plane(1, RejectReasons::SCALING)
        );
    }

    let sixteen = ScaleLimits {
        max_upscale: 16 << 16,
        max_downscale: 16 << 16,
    };
    let mut engine = engine_with(caps_with(Some(sixteen)));
    for layers in [scaled_up(), scaled_down()] {
        assert!(validate(&mut engine, &layers).is_ok());
    }
}

// Row "Rotate180": capabilities refuse (`transforms 0x0`), planner NoPlane
// OUTPUT|TRANSFORM, TEST_ONLY not reached.
#[test]
fn rotation_is_refused_by_transform() {
    let mut engine = engine_with(board_caps());
    let mut ov = overlay(OV);
    ov.transform = Transform::Rotate180;
    assert_eq!(
        validate(&mut engine, &[background(BG), ov]).unwrap_err(),
        no_plane(1, RejectReasons::TRANSFORM)
    );
}

// Row "Plane -> CRTC masks": the five primaries of other CRTCs (mask 0b0)
// never take a layer.
#[test]
fn primaries_of_other_crtcs_are_never_used() {
    let mut engine = engine_with(board_caps());
    let plan = validate(&mut engine, &[background(BG)]).unwrap();
    assert_eq!(plan.assignments()[0].plane, PRIMARY);
}
