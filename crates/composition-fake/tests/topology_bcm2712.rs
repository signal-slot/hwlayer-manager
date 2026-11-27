//! Planner and fake-engine checks on the observed BCM2712 (Raspberry Pi 5,
//! VC4/HVS "vc6") display topology.
//!
//! This is a MODEL of the observed topology built from the recorded probe and
//! two-plane harness output (`docs/observed/bcm2712-topology.md` and
//! `docs/observed/bcm2712.md`, observed 2026-11-28, kernel
//! 6.18.50+rpt-rpi-2712). It is not a hardware result: the fake backend has no
//! scanout, timing or driver, and its check accepts whatever the planner
//! produced. What is verified is that the planner accepts the layouts the
//! board accepted and refuses the ones the board's own capability detection
//! and planner refused, as SoW.md §6 specifies. The kernel `TEST_ONLY`
//! verdicts cannot be reproduced here; where they differ from the capability
//! description, the test says so.
//!
//! Each test names the observed row it models.
//!
//! Plane count: the active CRTC (index 2, id 94) can really use 18 planes
//! (primary 83, 16 shared overlays 107..=272, plane 657). `MAX_PLANES` is 16,
//! so the main model holds the primary and 15 overlays; the ignored test
//! `all_planes_of_the_crtc_fit_in_the_capabilities` records the gap. The
//! engine's own truncated 16-plane view (four primaries and overlays
//! 107..=228, 13 usable on the output) is built by `observed_engine_view`.

use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, DisplayEvent, Engine, Error, FormatModifier,
    Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId,
    PlaneKind, Rect, RejectReasons, ScaleLimits, Size, SubmissionId, Transform, Zpos, ALPHA_OPAQUE,
};
use composition_fake::{FakeBackend, PlaneState};

const OUT: OutputId = OutputId(0);
const SCREEN: Size = Size::new(1920, 1080);
const FULL: Rect = Rect::new(0, 0, 1920, 1080);
/// The overlay of the harness: 384x540 at 960,270.
const OVL_SRC: Rect = Rect::new(0, 0, 384, 540);
const OVL_DST: Rect = Rect::new(960, 270, 384, 540);

const PRIMARY: PlaneId = PlaneId(83);

/// Shared overlay plane ids: 107, 118, ... (step 11).
fn overlay_id(n: u32) -> PlaneId {
    PlaneId(107 + 11 * n)
}

/// The 32 linear formats the engine's view recorded per plane (probe output,
/// `MAX_FORMATS` cut), in order.
const FORMATS: [&[u8; 4]; 32] = [
    b"XR24", b"AR24", b"AB24", b"XB24", b"RX24", b"RA24", b"BX24", b"BA24", b"RG16", b"BG16",
    b"AR15", b"XR15", b"RG24", b"BG24", b"YU16", b"YV16", b"YU24", b"YV24", b"YU12", b"YV12",
    b"NV12", b"NV21", b"NV16", b"NV61", b"XR30", b"AR30", b"AB30", b"XB30", b"RGB8", b"BGR8",
    b"XR12", b"AR12",
];

const XR24: PixelFormat = PixelFormat::XRGB8888;
const AR24: PixelFormat = PixelFormat::ARGB8888;
/// "HWL0": a format no plane lists (the harness' unsupported-format row).
const HWL0: PixelFormat = PixelFormat::fourcc(b"HWL0");
/// `I915_FORMAT_MOD_X_TILED`, a modifier no plane lists.
const I915_X_TILED: Modifier = Modifier(0x0100_0000_0000_0001);
/// Broadcom modifier code 1 (VC4 T-tiled) of the kernel's IN_FORMATS.
const BROADCOM_T: Modifier = Modifier(0x0700_0000_0000_0001);

/// Plane as the probe recorded it: plane alpha, none/premultiplied/coverage
/// blend, no scaling claimed, no transforms read.
fn base_plane(id: PlaneId, kind: PlaneKind, zpos: Zpos, outputs: u32) -> PlaneCaps {
    let mut plane = PlaneCaps {
        id,
        kind,
        outputs,
        zpos,
        plane_alpha: true,
        blend_modes: BlendMode::Opaque.mask()
            | BlendMode::Premultiplied.mask()
            | BlendMode::Coverage.mask(),
        ..PlaneCaps::default()
    };
    for code in FORMATS {
        plane
            .formats
            .push(FormatModifier::linear(PixelFormat::fourcc(code)))
            .unwrap();
    }
    plane
}

/// Primaries: `zpos` immutable 0.
fn primary(id: PlaneId, outputs: u32) -> PlaneCaps {
    base_plane(id, PlaneKind::Primary, Zpos::Fixed(0), outputs)
}

/// Shared overlays: `zpos` mutable 1..=17.
fn overlay(id: PlaneId) -> PlaneCaps {
    base_plane(id, PlaneKind::Overlay, Zpos::Range { min: 1, max: 17 }, 1)
}

fn caps_of(planes: impl IntoIterator<Item = PlaneCaps>) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SCREEN,
            refresh_mhz: 60_000,
        })
        .unwrap();
    for plane in planes {
        caps.planes.push(plane).unwrap();
    }
    caps
}

/// The CRTC's own planes: primary 83 plus `overlays` shared overlays (at most
/// 15 fit).
fn crtc_model(overlays: u32) -> Capabilities {
    caps_of(
        std::iter::once(primary(PRIMARY, 1)).chain((0..overlays).map(|n| overlay(overlay_id(n)))),
    )
}

/// Row "Planes the engine can use on HDMI-A-1: 13": the four primaries (three
/// with mask 0b0) first, then overlays 107..=228.
fn observed_engine_view() -> Capabilities {
    caps_of(
        [
            primary(PlaneId(48), 0),
            primary(PlaneId(67), 0),
            primary(PRIMARY, 1),
            primary(PlaneId(95), 0),
        ]
        .into_iter()
        .chain((0..12).map(|n| overlay(overlay_id(n)))),
    )
}

/// `caps` with `edit` applied to a copy of every plane.
fn edit_planes(caps: Capabilities, edit: impl Fn(&mut PlaneCaps)) -> Capabilities {
    caps_of(caps.planes.iter().map(|plane| {
        let mut plane = *plane;
        edit(&mut plane);
        plane
    }))
}

fn scaling_claimed(caps: Capabilities) -> Capabilities {
    edit_planes(caps, |plane| {
        plane.scaling = Some(ScaleLimits {
            max_upscale: 16 << 16,
            max_downscale: 16 << 16,
        });
    })
}

fn info(size: Size, format: PixelFormat, modifier: Modifier) -> BufferInfo {
    BufferInfo {
        size,
        format,
        modifier,
    }
}

fn engine(caps: Capabilities) -> Engine<FakeBackend> {
    Engine::new(FakeBackend::new(caps))
}

fn register(engine: &mut Engine<FakeBackend>, buffer: u32, info: BufferInfo) -> BufferId {
    let id = BufferId(buffer);
    engine.register_buffer(id, info).unwrap();
    engine.finish_write(id).unwrap();
    id
}

fn bg_info() -> BufferInfo {
    info(SCREEN, XR24, Modifier::LINEAR)
}

fn ovl_info() -> BufferInfo {
    info(Size::new(384, 540), AR24, Modifier::LINEAR)
}

fn bg_layer(buffer: BufferId) -> Layer {
    Layer::new(buffer, FULL, FULL, 0)
}

fn ovl_layer(buffer: BufferId) -> Layer {
    let mut layer = Layer::new(buffer, OVL_SRC, OVL_DST, 1);
    layer.blend = BlendMode::Premultiplied;
    layer
}

fn frame(layers: &[Layer]) -> Frame<'_> {
    Frame {
        output: OUT,
        layers,
    }
}

fn present(engine: &mut Engine<FakeBackend>, layers: &[Layer]) -> Result<SubmissionId, Error> {
    engine.present(&frame(layers))
}

fn drain(engine: &mut Engine<FakeBackend>) -> Vec<DisplayEvent> {
    let mut events = Vec::new();
    while let Some(event) = engine.poll_event().unwrap() {
        events.push(event);
    }
    events
}

fn no_plane(result: Result<SubmissionId, Error>) -> (usize, RejectReasons) {
    match result {
        Err(Error::Plan(PlanError::NoPlane { layer, reasons })) => (layer, reasons),
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

// Row: "Two-plane runs, default": background -> primary 83 zpos 0, overlay ->
// overlay 107 zpos 1; verdict PASS.
#[test]
fn background_goes_to_the_primary_and_overlay_to_the_first_overlay() {
    let mut engine = engine(crtc_model(15));
    let bg = register(&mut engine, 1, bg_info());
    let ovl = register(&mut engine, 2, ovl_info());
    let layers = [bg_layer(bg), ovl_layer(ovl)];
    let plan = engine.validate(&frame(&layers)).unwrap();
    let a = plan.assignments();
    assert_eq!(a.len(), 2);
    assert_eq!((a[0].plane, a[0].zpos), (PRIMARY, 0));
    assert_eq!((a[0].source, a[0].destination), (FULL, FULL));
    assert_eq!((a[1].plane, a[1].zpos), (overlay_id(0), 1));
    assert_eq!((a[1].source, a[1].destination), (OVL_SRC, OVL_DST));
}

// Rows: "Two-plane runs, default" (600 committed / 600 latched, 1198 released
// of 1200 uses, 0 dropped) and "BUFFERS 3 per layer".
#[test]
fn three_buffers_per_layer_rotate_with_every_commit_latched_and_released() {
    const FRAMES: u32 = 600;
    let mut engine = engine(crtc_model(15));
    let bgs: Vec<_> = (0..3)
        .map(|n| register(&mut engine, 1 + n, bg_info()))
        .collect();
    let ovls: Vec<_> = (0..3)
        .map(|n| register(&mut engine, 10 + n, ovl_info()))
        .collect();

    let (mut latched, mut released, mut dropped) = (0u32, 0u32, 0u32);
    for n in 0..FRAMES {
        let i = n as usize % 3;
        let layers = [bg_layer(bgs[i]), ovl_layer(ovls[i])];
        let submission = present(&mut engine, &layers).unwrap();
        assert_eq!(engine.backend().pending(OUT), Some(submission));
        engine.backend_mut().vsync();
        for event in drain(&mut engine) {
            match event {
                DisplayEvent::Presented { submission: s, .. } => {
                    assert_eq!(s, submission);
                    latched += 1;
                }
                DisplayEvent::Released { .. } => released += 1,
                DisplayEvent::Dropped { .. } => dropped += 1,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(
            engine.backend().scanout(OUT).unwrap(),
            [
                PlaneState {
                    plane: PRIMARY,
                    buffer: bgs[i],
                    zpos: 0
                },
                PlaneState {
                    plane: overlay_id(0),
                    buffer: ovls[i],
                    zpos: 1
                },
            ]
        );
    }
    assert_eq!((latched, dropped), (FRAMES, 0));
    // Two layers, each frame but the first retires one buffer per layer: the
    // board's 1198 of 1200 at 600 frames.
    assert_eq!(released, 2 * (FRAMES - 1));
}

// Row: "--swap-z ... SWAPZ skipped: a plane of the plan has a fixed zpos". The
// primary's zpos is fixed 0, so the lowest layer must land on it and every
// other layer goes above it on an overlay.
#[test]
fn the_lowest_layer_takes_the_fixed_zpos_primary() {
    let mut engine = engine(crtc_model(15));
    let ovl = register(&mut engine, 1, ovl_info());
    let bg = register(&mut engine, 2, bg_info());
    let layers = [
        Layer::new(ovl, OVL_SRC, OVL_DST, 0),
        Layer::new(bg, FULL, FULL, 1),
    ];
    let plan = engine.validate(&frame(&layers)).unwrap();
    let a = plan.assignments();
    assert_eq!((a[0].plane, a[0].zpos), (PRIMARY, 0));
    assert_ne!(a[1].plane, PRIMARY);
    assert!(a[1].zpos > a[0].zpos);
}

// Rows: "zpos: overlays mutable 1..=17" and the overlay-overlay swap that the
// harness could not exercise ("would be possible on this hardware",
// inferred): zpos follows z_order for two overlays, in both orders.
#[test]
fn overlay_zpos_follows_the_layer_order_in_both_orders() {
    for swapped in [false, true] {
        let mut engine = engine(crtc_model(15));
        let bg = register(&mut engine, 1, bg_info());
        let a = register(&mut engine, 2, ovl_info());
        let b = register(&mut engine, 3, ovl_info());
        let (za, zb) = if swapped { (2, 1) } else { (1, 2) };
        let layers = [
            bg_layer(bg),
            Layer::new(a, OVL_SRC, OVL_DST, za),
            Layer::new(b, OVL_SRC, Rect::new(1000, 300, 384, 540), zb),
        ];
        let plan = engine.validate(&frame(&layers)).unwrap();
        // Assignments come ordered bottom to top; look layers up by index.
        let asg = plan.assignments();
        assert_eq!(asg.len(), 3);
        let of = |layer: usize| *asg.iter().find(|a| a.layer == layer).unwrap();
        assert_eq!(of(0).zpos, 0);
        assert!(of(1).zpos >= 1 && of(2).zpos >= 1);
        assert_eq!(of(1).zpos < of(2).zpos, !swapped);
        assert_ne!(of(1).plane, of(2).plane);
    }
}

// Rows: "plane-alpha", "blend-coverage" (caps accepts, planner plan, TEST_ONLY
// ok) and the `--blend coverage --plane-alpha 32768` run (PASS).
#[test]
fn plane_alpha_and_coverage_blend_are_planned() {
    let mut engine = engine(crtc_model(15));
    let bg = register(&mut engine, 1, bg_info());
    let ovl = register(&mut engine, 2, ovl_info());
    let mut layer = ovl_layer(ovl);
    layer.blend = BlendMode::Coverage;
    layer.alpha = 0x8000;
    present(&mut engine, &[bg_layer(bg), layer]).unwrap();
    engine.backend_mut().vsync();
    assert!(drain(&mut engine)
        .iter()
        .any(|e| matches!(e, DisplayEvent::Presented { .. })));
    assert_eq!(engine.backend().scanout(OUT).unwrap().len(), 2);
}

// Rows: "offscreen: caps accepts (the planner clips to the output)" and
// "odd: caps accepts (no alignment rule)".
#[test]
fn offscreen_and_odd_geometry_are_clipped_not_refused() {
    let mut engine = engine(crtc_model(15));
    let bg = register(&mut engine, 1, bg_info());
    let ovl = register(&mut engine, 2, ovl_info());
    // Half of the overlay hangs off the right edge: 192 of 384 columns show.
    let off = Layer::new(ovl, OVL_SRC, Rect::new(1728, 270, 384, 540), 1);
    let layers = [bg_layer(bg), off];
    let plan = engine.validate(&frame(&layers)).unwrap();
    let o = plan.assignments()[1];
    assert_eq!(o.destination, Rect::new(1728, 270, 192, 540));
    assert_eq!(o.source, Rect::new(0, 0, 192, 540));

    let odd = Layer::new(
        ovl,
        Rect::new(1, 1, 301, 401),
        Rect::new(7, 13, 301, 401),
        1,
    );
    engine.validate(&frame(&[bg_layer(bg), odd])).unwrap();
}

// Row: "UNSUPPORTED format HWL0": caps refuses, planner NoPlane (FORMAT),
// TEST_ONLY not reached.
#[test]
fn unlisted_format_has_no_plane_on_the_output() {
    let mut engine = engine(observed_engine_view());
    let bg = register(&mut engine, 1, bg_info());
    let hwl = register(
        &mut engine,
        2,
        info(Size::new(384, 540), HWL0, Modifier::LINEAR),
    );
    let (layer, reasons) = no_plane(present(&mut engine, &[bg_layer(bg), ovl_layer(hwl)]));
    // Board: NoPlane{layer:1,reasons:OUTPUT|FORMAT}: OUTPUT from the other
    // three primaries, FORMAT from every plane on the output.
    assert_eq!(layer, 1);
    assert_eq!(reasons, RejectReasons::OUTPUT | RejectReasons::FORMAT);
    assert_eq!(engine.backend().commit_count(), 0);
}

// Row: "UNSUPPORTED modifier I915_X_TILED": caps refuses, NoPlane (FORMAT).
// Also the "Modifiers: none in the engine's view" row: the engine view lists
// no Broadcom modifier, so one is refused; with the kernel's IN_FORMATS
// modelled (inferred from the topology file, not a probe result) it is
// accepted.
#[test]
fn unlisted_modifier_is_refused_and_a_listed_broadcom_modifier_is_accepted() {
    let mut engine = engine(observed_engine_view());
    let bg = register(&mut engine, 1, bg_info());
    let tiled = register(
        &mut engine,
        2,
        info(Size::new(384, 540), AR24, I915_X_TILED),
    );
    let (layer, reasons) = no_plane(present(&mut engine, &[bg_layer(bg), ovl_layer(tiled)]));
    assert_eq!(layer, 1);
    assert_eq!(reasons, RejectReasons::OUTPUT | RejectReasons::FORMAT);

    let bcm = register(&mut engine, 3, info(Size::new(384, 540), AR24, BROADCOM_T));
    let (_, reasons) = no_plane(present(&mut engine, &[bg_layer(bg), ovl_layer(bcm)]));
    assert!(reasons.contains(RejectReasons::FORMAT));

    let caps = edit_planes(observed_engine_view(), |plane| {
        // The format list is full (32); drop the last two to make room.
        plane.formats.pop();
        plane.formats.pop();
        for format in [XR24, AR24] {
            plane
                .formats
                .push(FormatModifier {
                    format,
                    modifier: BROADCOM_T,
                })
                .unwrap();
        }
    });
    let mut engine = Engine::new(FakeBackend::new(caps));
    let bg = register(&mut engine, 1, bg_info());
    let bcm = register(&mut engine, 3, info(Size::new(384, 540), AR24, BROADCOM_T));
    present(&mut engine, &[bg_layer(bg), ovl_layer(bcm)]).unwrap();
}

// Row: "UNSUPPORTED layers: 14 layers for 13 planes on the output": caps
// refuses, NoPlane{layer:13, OUTPUT|IN_USE}, TEST_ONLY not reached.
#[test]
fn fourteen_layers_do_not_fit_the_thirteen_usable_planes() {
    let mut engine = engine(observed_engine_view());
    let buffers: Vec<_> = (0..14u32)
        .map(|n| {
            register(
                &mut engine,
                1 + n,
                info(Size::new(64, 64), XR24, Modifier::LINEAR),
            )
        })
        .collect();
    let layer = |n: usize| {
        Layer::new(
            buffers[n],
            Rect::new(0, 0, 64, 64),
            Rect::new(10 * n as i32, 10 * n as i32, 64, 64),
            n as i32,
        )
    };
    let thirteen: Vec<_> = (0..13).map(layer).collect();
    present(&mut engine, &thirteen).unwrap();
    engine.backend_mut().vsync();
    let fourteen: Vec<_> = (0..14).map(layer).collect();
    let (index, reasons) = no_plane(present(&mut engine, &fourteen));
    assert_eq!(index, 13);
    assert_eq!(reasons, RejectReasons::OUTPUT | RejectReasons::IN_USE);
}

// Rows: "scale-up / scale-down": caps refuses (no scaling claimed), planner
// NoPlane (SCALING), TEST_ONLY ok with a hand-built plan whose caps claim
// scaling by up to 16. The planner agrees with the caps as probed; the second
// half models the hand-built plan's caps.
#[test]
fn scaling_is_refused_unless_claimed_and_planned_once_claimed() {
    let up = |buffer| Layer::new(buffer, Rect::new(0, 0, 192, 270), OVL_DST, 1);
    let down = |buffer| Layer::new(buffer, Rect::new(0, 0, 768, 1080), OVL_DST, 1);
    let big = info(Size::new(768, 1080), AR24, Modifier::LINEAR);

    let mut probed = engine(observed_engine_view());
    let bg = register(&mut probed, 1, bg_info());
    let ovl = register(&mut probed, 2, ovl_info());
    let ovl_big = register(&mut probed, 3, big);
    for layer in [up(ovl), down(ovl_big)] {
        let (index, reasons) = no_plane(present(&mut probed, &[bg_layer(bg), layer]));
        assert_eq!(index, 1);
        assert_eq!(reasons, RejectReasons::OUTPUT | RejectReasons::SCALING);
    }

    let mut claimed = engine(scaling_claimed(observed_engine_view()));
    let bg = register(&mut claimed, 1, bg_info());
    let ovl = register(&mut claimed, 2, ovl_info());
    let ovl_big = register(&mut claimed, 3, big);
    present(&mut claimed, &[bg_layer(bg), up(ovl)]).unwrap();
    claimed.backend_mut().vsync();
    present(&mut claimed, &[bg_layer(bg), down(ovl_big)]).unwrap();
}

// Row: "scale-up / scale-down ... TEST_ONLY ok": the kernel accepted 2x up and
// 1/2 down, yet the probed capabilities (scaling not claimed, since KMS has no
// scaling property) make the planner refuse them. The gap is in capability
// detection, not in the planner, which follows the caps; until composition-drm
// knows the HVS scaler limits the engine falls back to refusing.
#[test]
#[ignore = "gap: probed caps claim no scaling, but vc4 TEST_ONLY accepted 2x up and 1/2 down"]
fn probed_caps_accept_what_test_only_accepted_for_scaling() {
    let mut engine = engine(observed_engine_view());
    let bg = register(&mut engine, 1, bg_info());
    let ovl = register(&mut engine, 2, ovl_info());
    let up = Layer::new(ovl, Rect::new(0, 0, 192, 270), OVL_DST, 1);
    present(&mut engine, &[bg_layer(bg), up]).expect("TEST_ONLY accepted 2x up on the board");
}

// Row: "UNSUPPORTED rotation": caps refuses (transforms 0x0), planner NoPlane
// (TRANSFORM), TEST_ONLY not reached. The kernel offers rotate-0/180 and
// reflect-x/y (observed), composition-drm does not read them; the second half
// models caps that would (inferred), and Rotate90 stays refused.
#[test]
fn rotation_is_refused_with_probed_caps_and_planned_when_modelled() {
    let rotate = |buffer, transform| {
        let mut layer = ovl_layer(buffer);
        layer.transform = transform;
        layer
    };
    let mut probed = engine(observed_engine_view());
    let bg = register(&mut probed, 1, bg_info());
    let ovl = register(&mut probed, 2, ovl_info());
    let (index, reasons) = no_plane(present(
        &mut probed,
        &[bg_layer(bg), rotate(ovl, Transform::Rotate180)],
    ));
    assert_eq!(index, 1);
    assert_eq!(reasons, RejectReasons::OUTPUT | RejectReasons::TRANSFORM);

    let caps = edit_planes(observed_engine_view(), |plane| {
        plane.transforms = Transform::Normal.mask()
            | Transform::Rotate180.mask()
            | Transform::FlipH.mask()
            | Transform::FlipV.mask();
    });
    let mut modelled = engine(caps);
    let bg = register(&mut modelled, 1, bg_info());
    let ovl = register(&mut modelled, 2, ovl_info());
    present(
        &mut modelled,
        &[bg_layer(bg), rotate(ovl, Transform::Rotate180)],
    )
    .unwrap();
    modelled.backend_mut().vsync();
    let (_, reasons) = no_plane(present(
        &mut modelled,
        &[bg_layer(bg), rotate(ovl, Transform::Rotate90)],
    ));
    assert!(reasons.contains(RejectReasons::TRANSFORM));
}

// Row: "Plane alpha: alpha 0..65535 on every plane". Not a board rejection:
// shows the model's alpha flag decides, so the accepted rows above are not
// vacuous.
#[test]
fn removing_plane_alpha_from_the_model_makes_translucent_layers_unplaceable() {
    let caps = edit_planes(crtc_model(15), |plane| plane.plane_alpha = false);
    let mut engine = engine(caps);
    let bg = register(&mut engine, 1, bg_info());
    let ovl = register(&mut engine, 2, ovl_info());
    let mut layer = ovl_layer(ovl);
    layer.alpha = 0x8000;
    let (index, reasons) = no_plane(present(&mut engine, &[bg_layer(bg), layer]));
    assert_eq!(index, 1);
    assert!(reasons.contains(RejectReasons::ALPHA));
    layer.alpha = ALPHA_OPAQUE;
    present(&mut engine, &[bg_layer(bg), layer]).unwrap();
}

// Rows: "Planes in the kernel: 56" and "the kernel offers 18 planes on CRTC
// index 2". The real CRTC has the primary, 16 shared overlays and plane 657;
// the capabilities hold at most MAX_PLANES = 16 (the DRM backend's truncation
// bug is fixed elsewhere; this limit is the core's own).
#[test]
#[ignore = "gap: MAX_PLANES is 16, the observed CRTC can use 18 planes"]
fn all_planes_of_the_crtc_fit_in_the_capabilities() {
    let mut caps = Capabilities::default();
    let planes = std::iter::once(primary(PRIMARY, 1))
        .chain((0..16).map(|n| overlay(overlay_id(n))))
        .chain(std::iter::once(overlay(PlaneId(657))));
    for plane in planes {
        caps.planes.push(plane).expect("18 planes on the CRTC");
    }
}

// As far as the model can go on that row: 16 planes (the cap) take a
// 16-layer frame (MAX_LAYERS), one layer per plane.
#[test]
fn sixteen_planes_take_sixteen_layers() {
    let mut engine = engine(crtc_model(15));
    let buffers: Vec<_> = (0..16u32)
        .map(|n| {
            register(
                &mut engine,
                1 + n,
                info(Size::new(64, 64), XR24, Modifier::LINEAR),
            )
        })
        .collect();
    let layers: Vec<_> = buffers
        .iter()
        .enumerate()
        .map(|(n, b)| {
            Layer::new(
                *b,
                Rect::new(0, 0, 64, 64),
                Rect::new(20 * n as i32, 20 * n as i32, 64, 64),
                n as i32,
            )
        })
        .collect();
    let plan = engine.validate(&frame(&layers)).unwrap();
    let mut planes: Vec<_> = plan.assignments().iter().map(|a| a.plane.0).collect();
    planes.sort_unstable();
    planes.dedup();
    assert_eq!(planes.len(), 16);
}
