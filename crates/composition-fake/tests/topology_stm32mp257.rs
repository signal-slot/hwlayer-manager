//! The observed STM32MP257F-DK (LTDC) topology as a planner scenario on the
//! fake backend (SoW §6, §10).
//!
//! This is a MODEL of the observed topology, not a hardware result. The
//! capabilities below are typed in from `docs/observed/stm32mp257-topology.md`
//! and `docs/observed/stm32mp257.md` (observed 2026-11-27 and 2026-11-28 on
//! the board's primary CRTC): what the planner and the fake engine accept or
//! refuse here only shows that the shared engine reaches the verdicts the
//! board's harness recorded. Each test names the observed row it models.
//!
//! Limits of the model:
//! - Only the 1920x1080@60 Hz CRTC and its three planes (34 Primary, 42 and
//!   49 Overlay) are modelled. All planes reach the one CRTC (mask 0x1).
//! - Every modifier is LINEAR. Formats follow the observed lists.
//! - zpos is the mutable range 0..=2 on all planes. Blend is premultiplied
//!   only (no `pixel blend mode` property, so the KMS default). Plane alpha
//!   is supported. Transforms are none: composition-drm programs no rotation
//!   although plane 34 lists rotate-90/180/270 and every plane the
//!   reflections.
//! - Scaling: the observed caps claim none (composition-drm reads no scaler
//!   limits). `docs/stm32mp257-ltdc-board.md` ("Scaling bounds", observed
//!   2026-11-27) recorded per-plane TEST_ONLY results: planes 34 and 42
//!   accepted upscales up to the mode size, plane 49 refused any upscale above
//!   1.0, and downscales of 0.5, 0.75 and 0.9 were refused on all three.
//!   `caps_with_scaling` and `caps_claiming` model the harness's hand-claimed
//!   `--scaling 8,1` and `--scaling 8,2` caps, which claim scaling on EVERY
//!   plane; they do not model the hardware's per-plane scalers. Only
//!   `upscale_claim_on_34_and_42_keeps_plane_49_out` models the per-plane
//!   observation, and no limit beyond the measured ones is modelled.
//! - Pixel colours on screen were never verified (the KVM gave no picture),
//!   so nothing here says anything about blending results.
//!
//! The fake backend's TEST_ONLY stand-in is a check hook; the real kernel's
//! verdicts are reproduced only for the cases the board recorded.

use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, DisplayEvent, Engine, Error, FormatModifier,
    Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId,
    PlaneKind, Rect, RejectReasons, ScaleLimits, Size, Transform, Zpos,
};
use composition_fake::{FakeBackend, PlaneState};

const OUT: OutputId = OutputId(0);
const SCREEN: Size = Size::new(1920, 1080);
const FULL: Rect = Rect::new(0, 0, 1920, 1080);
/// The overlay of the harness scene: 384x540 at 960,270.
const WINDOW_SRC: Rect = Rect::new(0, 0, 384, 540);
const WINDOW: Rect = Rect::new(960, 270, 384, 540);
const WINDOW_SIZE: Size = Size::new(384, 540);

const P34: PlaneId = PlaneId(34);
const P42: PlaneId = PlaneId(42);
const P49: PlaneId = PlaneId(49);

const XR24: PixelFormat = PixelFormat::XRGB8888;
const AR24: PixelFormat = PixelFormat::ARGB8888;
/// The harness's made-up fourcc that no plane lists.
const HWL0: PixelFormat = PixelFormat::fourcc(b"HWL0");
/// `I915_FORMAT_MOD_X_TILED`, a modifier no plane lists.
const I915_X_TILED: Modifier = Modifier(0x0100_0000_0000_0001);

const BG: BufferId = BufferId(1);
const OVL: BufferId = BufferId(2);
const THIRD: BufferId = BufferId(3);
const FOURTH: BufferId = BufferId(4);

/// Formats on all three planes (observed; the RGB rows plus C8).
const RGB_FORMATS: [&[u8; 4]; 17] = [
    b"AR24", b"XR24", b"AB24", b"XB24", b"RA24", b"RX24", b"BA24", b"BX24", b"RG16", b"BG16",
    b"RG24", b"BG24", b"AR15", b"XR15", b"AR12", b"XR12", b"C8  ",
];
/// Extra on planes 34 and 42 (observed: YUYV YVYU UYVY VYUY).
const PACKED_YUV: [&[u8; 4]; 4] = [b"YUYV", b"YVYU", b"UYVY", b"VYUY"];
/// Extra on plane 42 only (observed: NV12 NV21 YU12 YV12).
const PLANAR_YUV: [&[u8; 4]; 4] = [b"NV12", b"NV21", b"YU12", b"YV12"];

fn ltdc_plane(
    id: PlaneId,
    kind: PlaneKind,
    extra: &[&[u8; 4]],
    scaling: Option<ScaleLimits>,
) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id,
        kind,
        outputs: 1,
        scaling,
        plane_alpha: true,
        // Observed: no `pixel blend mode` property, composition-drm reports
        // premultiplied only.
        blend_modes: BlendMode::Premultiplied.mask(),
        transforms: 0,
        zpos: Zpos::Range { min: 0, max: 2 },
        ..PlaneCaps::default()
    };
    for code in RGB_FORMATS.iter().copied().chain(extra.iter().copied()) {
        caps.formats
            .push(FormatModifier::linear(PixelFormat::fourcc(code)))
            .unwrap();
    }
    caps
}

/// The observed caps: no plane claims scaling.
fn observed_caps() -> Capabilities {
    caps_with_scaling(None)
}

/// The observed planes of CRTC 41, with `scaling` claimed on every plane (the
/// harness's hand-claimed caps, not the hardware's per-plane scalers).
fn caps_with_scaling(scaling: Option<ScaleLimits>) -> Capabilities {
    caps_per_plane([scaling; 3])
}

/// The observed planes of CRTC 41 with `scaling` given per plane (34, 42, 49).
fn caps_per_plane(scaling: [Option<ScaleLimits>; 3]) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SCREEN,
            refresh_mhz: 60_000,
        })
        .unwrap();
    let mut yuv42 = PACKED_YUV.to_vec();
    yuv42.extend_from_slice(&PLANAR_YUV);
    for plane in [
        ltdc_plane(P34, PlaneKind::Primary, &PACKED_YUV, scaling[0]),
        ltdc_plane(P42, PlaneKind::Overlay, &yuv42, scaling[1]),
        ltdc_plane(P49, PlaneKind::Overlay, &[], scaling[2]),
    ] {
        caps.planes.push(plane).unwrap();
    }
    caps
}

/// The observed caps with every plane claiming scaling of `up` and `down`
/// (whole factors), as the harness's `--scaling` flag does.
fn caps_claiming(up: u32, down: u32) -> Capabilities {
    caps_with_scaling(Some(ScaleLimits {
        max_upscale: up << 16,
        max_downscale: down << 16,
    }))
}

fn info(size: Size, format: PixelFormat, modifier: Modifier) -> BufferInfo {
    BufferInfo {
        size,
        format,
        modifier,
    }
}

fn register(engine: &mut Engine<FakeBackend>, id: BufferId, info: BufferInfo) {
    engine.register_buffer(id, info).unwrap();
    engine.finish_write(id).unwrap();
}

fn engine_with(caps: Capabilities) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(caps));
    register(&mut engine, BG, info(SCREEN, XR24, Modifier::LINEAR));
    register(&mut engine, OVL, info(WINDOW_SIZE, AR24, Modifier::LINEAR));
    engine
}

fn premul(mut layer: Layer) -> Layer {
    layer.blend = BlendMode::Premultiplied;
    layer
}

/// The harness scene: background full screen at z 0, the window at z 1.
fn scene() -> [Layer; 2] {
    [
        premul(Layer::new(BG, FULL, FULL, 0)),
        premul(Layer::new(OVL, WINDOW_SRC, WINDOW, 1)),
    ]
}

fn window_at(destination: Rect) -> [Layer; 2] {
    [
        premul(Layer::new(BG, FULL, FULL, 0)),
        premul(Layer::new(OVL, WINDOW_SRC, destination, 1)),
    ]
}

fn frame(layers: &[Layer]) -> Frame<'_> {
    Frame {
        output: OUT,
        layers,
    }
}

fn no_plane(error: Error) -> (usize, RejectReasons) {
    match error {
        Error::Plan(PlanError::NoPlane { layer, reasons }) => (layer, reasons),
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

fn drain(engine: &mut Engine<FakeBackend>) -> Vec<DisplayEvent> {
    let mut events = Vec::new();
    while let Some(event) = engine.poll_event().unwrap() {
        events.push(event);
    }
    events
}

/// (layer, plane id, zpos) of every assignment.
fn placement(engine: &mut Engine<FakeBackend>, layers: &[Layer]) -> Vec<(usize, u32, i32)> {
    engine
        .validate(&frame(layers))
        .unwrap()
        .assignments()
        .iter()
        .map(|a| (a.layer, a.plane.0, a.zpos))
        .collect()
}

// Models: "Two-plane run (twoplane, default options)": background XR24 on
// plane 34 at zpos 0, AR24 window on plane 42 at zpos 1, committed and
// latched, plane 49 left off.
#[test]
fn default_scene_uses_primary_and_first_overlay() {
    let mut engine = engine_with(observed_caps());
    let layers = scene();
    assert_eq!(
        placement(&mut engine, &layers),
        [(0, 34, 0), (1, 42, 1)],
        "plane 49 stays off"
    );
    let submission = engine.present(&frame(&layers)).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(
        drain(&mut engine),
        [DisplayEvent::Presented {
            output: OUT,
            submission
        }]
    );
    assert_eq!(
        engine.backend().scanout(OUT).unwrap(),
        [
            PlaneState {
                plane: P34,
                buffer: BG,
                zpos: 0
            },
            PlaneState {
                plane: P42,
                buffer: OVL,
                zpos: 1
            },
        ]
    );
}

// Models: "Two-plane run" and "Errors and dropped frames": three buffers per
// layer rotated, every frame latched (0 dropped), every use released once.
#[test]
fn buffer_rotation_latches_every_frame_and_releases_each_use_once() {
    let mut engine = engine_with(observed_caps());
    let bgs = [BG, BufferId(11), BufferId(12)];
    let ovls = [OVL, BufferId(21), BufferId(22)];
    for id in &bgs[1..] {
        register(&mut engine, *id, info(SCREEN, XR24, Modifier::LINEAR));
    }
    for id in &ovls[1..] {
        register(&mut engine, *id, info(WINDOW_SIZE, AR24, Modifier::LINEAR));
    }
    let (mut presented, mut released, mut dropped) = (0, 0, 0);
    for n in 0..60 {
        let layers = [
            premul(Layer::new(bgs[n % 3], FULL, FULL, 0)),
            premul(Layer::new(ovls[n % 3], WINDOW_SRC, WINDOW, 1)),
        ];
        engine.present(&frame(&layers)).unwrap();
        engine.backend_mut().vsync();
        for event in drain(&mut engine) {
            match event {
                DisplayEvent::Presented { .. } => presented += 1,
                DisplayEvent::Released { .. } => released += 1,
                DisplayEvent::Dropped { .. } => dropped += 1,
                other => panic!("unexpected {other:?}"),
            }
        }
    }
    assert_eq!((presented, dropped), (60, 0));
    // The last frame is still on screen: every earlier use was released once.
    assert_eq!(released, 2 * 59);
}

// Models: "--swap-z": the overlay layer goes below the background. The board
// showed the planner moved layers between planes (overlay onto primary plane
// 34 zpos 0, background onto plane 42 zpos 1), not crossed zpos values.
#[test]
fn swap_z_moves_layers_between_planes() {
    let mut engine = engine_with(observed_caps());
    let layers = [
        premul(Layer::new(BG, FULL, FULL, 1)),
        premul(Layer::new(OVL, WINDOW_SRC, WINDOW, 0)),
    ];
    assert_eq!(placement(&mut engine, &layers), [(1, 34, 0), (0, 42, 1)]);
    engine.present(&frame(&layers)).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(
        engine.backend().scanout(OUT).unwrap(),
        [
            PlaneState {
                plane: P34,
                buffer: OVL,
                zpos: 0
            },
            PlaneState {
                plane: P42,
                buffer: BG,
                zpos: 1
            },
        ]
    );
}

// Models: "Non-full-screen primary": plane 34 accepted the 384x540 window at
// 960,270, so the primary plane is not full-output-only.
#[test]
fn primary_plane_takes_a_partial_window() {
    let mut engine = engine_with(observed_caps());
    let layers = [premul(Layer::new(OVL, WINDOW_SRC, WINDOW, 0))];
    assert_eq!(placement(&mut engine, &layers), [(0, 34, 0)]);
}

// Models: "Alpha variants" `--plane-alpha 32768` (commit only) and the
// `UNSUPPORTED plane-alpha` line: caps accept, planner plans, TEST_ONLY ok.
#[test]
fn constant_plane_alpha_is_accepted() {
    let mut engine = engine_with(observed_caps());
    let mut layers = scene();
    layers[1].alpha = 0x8000;
    assert_eq!(placement(&mut engine, &layers), [(0, 34, 0), (1, 42, 1)]);
    engine.present(&frame(&layers)).unwrap();
}

// Models: "Alpha variants" `--blend coverage` and `--blend none`:
// `NoPlane { layer: 1, reasons: BLEND }` from the planner, nothing committed.
#[test]
fn coverage_and_opaque_blend_are_refused_for_lack_of_a_blend_property() {
    for blend in [BlendMode::Coverage, BlendMode::Opaque] {
        let mut engine = engine_with(observed_caps());
        let mut layers = scene();
        layers[1].blend = blend;
        let error = engine.present(&frame(&layers)).unwrap_err();
        assert_eq!(no_plane(error), (1, RejectReasons::BLEND), "{blend:?}");
        assert_eq!(engine.backend().commit_count(), 0);
    }
}

// Models: `UNSUPPORTED format`: an unlisted fourcc, planner `NoPlane FORMAT`
// on layer 1, TEST_ONLY not reached.
#[test]
fn unknown_format_is_refused_by_format() {
    let mut engine = engine_with(observed_caps());
    register(
        &mut engine,
        THIRD,
        info(WINDOW_SIZE, HWL0, Modifier::LINEAR),
    );
    let layers = [
        premul(Layer::new(BG, FULL, FULL, 0)),
        premul(Layer::new(THIRD, WINDOW_SRC, WINDOW, 1)),
    ];
    let error = engine.validate(&frame(&layers)).unwrap_err();
    assert_eq!(no_plane(error), (1, RejectReasons::FORMAT));
    assert_eq!(engine.backend().check_count(), 0, "TEST_ONLY not reached");
}

// Models: `UNSUPPORTED modifier`: AR24 with a tiled modifier, `NoPlane
// FORMAT` (IN_FORMATS lists LINEAR only on every plane).
#[test]
fn tiled_modifier_is_refused_by_format() {
    let mut engine = engine_with(observed_caps());
    register(&mut engine, THIRD, info(WINDOW_SIZE, AR24, I915_X_TILED));
    let layers = [
        premul(Layer::new(BG, FULL, FULL, 0)),
        premul(Layer::new(THIRD, WINDOW_SRC, WINDOW, 1)),
    ];
    let error = engine.validate(&frame(&layers)).unwrap_err();
    assert_eq!(no_plane(error), (1, RejectReasons::FORMAT));
    assert_eq!(engine.backend().check_count(), 0);
}

// Models: `UNSUPPORTED layers`: 4 layers for 3 planes, `NoPlane IN_USE` on
// layer 3; three layers fit on 34/42/49 (zpos 0..2, "Planes" row).
#[test]
fn three_layers_fit_and_a_fourth_is_in_use() {
    let mut engine = engine_with(observed_caps());
    register(
        &mut engine,
        THIRD,
        info(WINDOW_SIZE, AR24, Modifier::LINEAR),
    );
    register(
        &mut engine,
        FOURTH,
        info(WINDOW_SIZE, AR24, Modifier::LINEAR),
    );
    let mut layers = vec![
        premul(Layer::new(BG, FULL, FULL, 0)),
        premul(Layer::new(OVL, WINDOW_SRC, WINDOW, 1)),
        premul(Layer::new(
            THIRD,
            WINDOW_SRC,
            Rect::new(100, 100, 384, 540),
            2,
        )),
    ];
    assert_eq!(
        placement(&mut engine, &layers),
        [(0, 34, 0), (1, 42, 1), (2, 49, 2)]
    );
    layers.push(premul(Layer::new(
        FOURTH,
        WINDOW_SRC,
        Rect::new(200, 200, 384, 540),
        3,
    )));
    let error = engine.validate(&frame(&layers)).unwrap_err();
    assert_eq!(no_plane(error), (3, RejectReasons::IN_USE));
}

// Models: `UNSUPPORTED rotation`: Rotate180 with composition-drm reporting no
// transforms, `NoPlane TRANSFORM`. (The kernel lists rotate-180 on plane 34;
// that is a composition-drm gap and is not modelled as a capability.)
#[test]
fn rotation_is_refused_by_transform() {
    let mut engine = engine_with(observed_caps());
    let mut layers = scene();
    layers[1].transform = Transform::Rotate180;
    let error = engine.validate(&frame(&layers)).unwrap_err();
    assert_eq!(no_plane(error), (1, RejectReasons::TRANSFORM));
}

// Models: `UNSUPPORTED scale-up` and `scale-down` with no scaling claimed:
// both `NoPlane SCALING`, TEST_ONLY not reached.
#[test]
fn scaling_is_refused_when_no_plane_claims_it() {
    for dst in [
        Rect::new(960, 270, 768, 1080),
        Rect::new(960, 270, 192, 270),
    ] {
        let mut engine = engine_with(observed_caps());
        let error = engine.validate(&frame(&window_at(dst))).unwrap_err();
        assert_eq!(no_plane(error), (1, RejectReasons::SCALING), "{dst:?}");
        assert_eq!(engine.backend().check_count(), 0);
    }
}

// Models: `UNSUPPORTED scale-up` with scaling claimed (`--scaling 8,1`):
// planner plans, kernel TEST_ONLY ok. The claim is made on every plane, as the
// harness does; it is not the per-plane hardware result.
#[test]
fn claimed_2x_upscale_is_planned_and_accepted() {
    let mut engine = engine_with(caps_claiming(2, 1));
    let layers = window_at(Rect::new(480, 0, 768, 1080));
    assert_eq!(placement(&mut engine, &layers), [(0, 34, 0), (1, 42, 1)]);
    engine.present(&frame(&layers)).unwrap();
    assert_eq!(engine.backend().commit_count(), 1);
}

// Models: `UNSUPPORTED scale-down` with limits that exclude it (`--scaling
// 8,1`): the planner refuses with `NoPlane SCALING` before any TEST_ONLY.
#[test]
fn downscale_is_refused_by_the_planner_when_limits_exclude_it() {
    let mut engine = engine_with(caps_claiming(2, 1));
    let layers = window_at(Rect::new(960, 270, 192, 270));
    let error = engine.validate(&frame(&layers)).unwrap_err();
    assert_eq!(no_plane(error), (1, RejectReasons::SCALING));
    assert_eq!(engine.backend().check_count(), 0);
}

// Models: `UNSUPPORTED scale-down` with a claim that allows 2x down
// (`--scaling 8,2`): the planner plans, the kernel's TEST_ONLY rejected every
// candidate. The fake's check hook stands in for that kernel verdict (it
// refuses any layer that shrinks); the engine must report BackendRejected and
// commit nothing.
#[test]
fn claimed_downscale_is_planned_but_rejected_by_test_only() {
    let mut engine = engine_with(caps_claiming(2, 2));
    engine.backend_mut().set_check_hook(|_, plan| {
        plan.assignments().iter().all(|a| {
            a.source.width <= a.destination.width && a.source.height <= a.destination.height
        })
    });
    let layers = window_at(Rect::new(960, 270, 192, 270));
    match engine.present(&frame(&layers)).unwrap_err() {
        Error::Plan(PlanError::BackendRejected { candidates }) => assert!(candidates >= 1),
        other => panic!("expected BackendRejected, got {other:?}"),
    }
    assert!(engine.backend().check_count() >= 1);
    assert_eq!(engine.backend().commit_count(), 0);
    // The same hook lets the unscaled scene through.
    engine.present(&frame(&scene())).unwrap();
}

// Models: `UNSUPPORTED offscreen` and `UNSUPPORTED odd`: caps accept (the
// planner clips to the output; the caps model has no alignment rule),
// TEST_ONLY ok.
#[test]
fn partly_offscreen_and_odd_geometry_are_planned() {
    let mut engine = engine_with(observed_caps());
    let layers = window_at(Rect::new(1700, 900, 384, 540));
    let plan = engine.validate(&frame(&layers)).unwrap();
    assert_eq!(plan.assignments()[1].plane, P42);
    assert_eq!(
        plan.assignments()[1].destination,
        Rect::new(1700, 900, 220, 180),
        "clipped to the output"
    );
    let layers = window_at(Rect::new(961, 271, 384, 540));
    assert_eq!(placement(&mut engine, &layers), [(0, 34, 0), (1, 42, 1)]);
}

// Models: "Formats, plane 42": YUV 4:2:0 (NV12) is listed on plane 42 only,
// so an NV12 layer lands there even though it is the lowest layer.
#[test]
fn planar_yuv_only_reaches_plane_42() {
    let mut engine = engine_with(observed_caps());
    register(
        &mut engine,
        THIRD,
        info(SCREEN, PixelFormat::NV12, Modifier::LINEAR),
    );
    let layers = [premul(Layer::new(THIRD, FULL, FULL, 0))];
    assert_eq!(placement(&mut engine, &layers), [(0, 42, 0)]);
}

// Models: "Scaling bounds" (docs/stm32mp257-ltdc-board.md, observed
// 2026-11-27): planes 34 and 42 accepted upscales, plane 49 refused any above
// 1.0. Upscaling is claimed on 34 and 42 only, so a third, scaled layer finds
// no plane for it; nothing beyond the measured per-plane result is modelled.
#[test]
fn upscale_claim_on_34_and_42_keeps_plane_49_out() {
    let up = Some(ScaleLimits {
        max_upscale: 2 << 16,
        max_downscale: 1 << 16,
    });
    let mut engine = engine_with(caps_per_plane([up, up, None]));
    register(
        &mut engine,
        THIRD,
        info(WINDOW_SIZE, AR24, Modifier::LINEAR),
    );
    // Background (960x540 source) and window both upscaled 2x: planes 34, 42.
    let mut layers = vec![
        premul(Layer::new(BG, Rect::new(0, 0, 960, 540), FULL, 0)),
        premul(Layer::new(OVL, WINDOW_SRC, Rect::new(480, 0, 768, 1080), 1)),
    ];
    let planes: Vec<u32> = placement(&mut engine, &layers)
        .iter()
        .map(|a| a.1)
        .collect();
    assert!(!planes.contains(&49), "{planes:?}");
    // A third scaled layer would need plane 49, which claims no scaling.
    layers.push(premul(Layer::new(
        THIRD,
        WINDOW_SRC,
        Rect::new(0, 0, 768, 1080),
        2,
    )));
    let error = engine.validate(&frame(&layers)).unwrap_err();
    assert!(no_plane(error).1.contains(RejectReasons::SCALING));
}
