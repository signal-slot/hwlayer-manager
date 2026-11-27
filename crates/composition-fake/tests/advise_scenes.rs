//! `composition_core::advise` over the recorded driver scenes of the
//! composition-fake planner tests: the two RK3588 video ports with per-port
//! plane reach (`rk3588_two_ports.rs`) and the BCM2711 HVS5 plane types with
//! the `MAX_PLANES` cut (`bcm2711_hvs5.rs`).
//!
//! The scene builders are copied from those files in minimal form (the
//! originals are private to them). Like them these are MODELS, not hardware
//! results: no TEST_ONLY or commit reached a kernel here.
//!
//! Checked per scene:
//! - (a) a frame the planner accepts needs nothing composed;
//! - (b) a frame the planner rejects for plane reach or a plane-count cut
//!   gets one contiguous run of the minimal length, consistent with the
//!   planner's reasons;
//! - (c) the same input gives the same advice;
//! - (d) scenes nothing can help give the expected blockers or errors.

use composition_core::planner::plan;
use composition_core::{
    advise, Advice, AdviceError, AdviseOptions, BlendMode, Blocker, BufferId, BufferInfo,
    BufferLookup, Capabilities, ComposeTarget, Engine, Error, FormatModifier, Frame, Layer,
    Modifier, OutputCaps, OutputId, PixelFormat, Placement, PlanError, PlaneCaps, PlaneId,
    PlaneKind, Rect, RejectReasons, SearchLimits, Size, Verdict, Zpos,
};
use composition_fake::FakeBackend;

const XR24: PixelFormat = PixelFormat::XRGB8888;
/// Buffer id of the composition layer in the reduced frames built below.
const TARGET_BUFFER: BufferId = BufferId(0xc0de);

struct Buffers(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

fn info(size: Size, format: PixelFormat) -> BufferInfo {
    BufferInfo {
        size,
        format,
        modifier: Modifier::LINEAR,
    }
}

fn options() -> AdviseOptions {
    AdviseOptions {
        compose: Some(ComposeTarget {
            format: XR24,
            modifier: Modifier::LINEAR,
            blend: BlendMode::Opaque,
        }),
        ..AdviseOptions::default()
    }
}

fn run_advise(
    frame: &Frame<'_>,
    caps: &Capabilities,
    buffers: &Buffers,
    options: &AdviseOptions,
) -> Result<Advice, AdviceError> {
    advise(frame, caps, buffers, options, |_, _| Verdict::Accept)
}

/// Planner verdict on `frame`.
fn planner(
    frame: &Frame<'_>,
    caps: &Capabilities,
    buffers: &Buffers,
) -> Result<composition_core::Plan, PlanError> {
    plan(frame, caps, buffers)
}

/// The planner accepts the frame with the composed run replaced by one
/// full-output layer of the composition buffer, and every other layer as it
/// was.
fn reduced_frame_plans(
    frame: &Frame<'_>,
    caps: &Capabilities,
    buffers: &Buffers,
    advice: &Advice,
    output_size: Size,
) {
    let full = Rect::new(0, 0, output_size.width, output_size.height);
    let mut reduced: Vec<Layer> = Vec::new();
    let mut inserted = false;
    for (layer, a) in frame.layers.iter().zip(advice.layers()) {
        if a.placement == Placement::Compose {
            if !inserted {
                reduced.push(Layer::new(TARGET_BUFFER, full, full, layer.z_order));
                inserted = true;
            }
        } else {
            reduced.push(*layer);
        }
    }
    let mut all = Buffers(buffers.0.clone());
    all.0.push((TARGET_BUFFER, info(output_size, XR24)));
    let reduced = Frame {
        output: frame.output,
        layers: &reduced,
    };
    planner(&reduced, caps, &all).expect("the reduced frame is legal");
}

/// Checks that `advice` composes exactly `want` layers as one contiguous run
/// (positions follow the stacking order, equal to the frame order in every
/// scene here), and that the rest sit on distinct planes reaching the output.
fn check_run(frame: &Frame<'_>, caps: &Capabilities, advice: &Advice, want: usize) {
    let output_bit = 1u32 << caps.output_index(frame.output).unwrap();
    let composed: Vec<usize> = advice
        .layers()
        .iter()
        .enumerate()
        .filter(|(_, l)| l.placement == Placement::Compose)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(composed.len(), want, "composed layers: {composed:?}");
    assert!(
        composed.windows(2).all(|w| w[1] == w[0] + 1),
        "{composed:?}"
    );
    let (low, high) = advice.compose_run().expect("a run");
    assert_eq!(high - low + 1, want);
    assert_eq!((low, high), (composed[0], *composed.last().unwrap()));
    assert!(!advice.is_direct());

    let mut planes = Vec::new();
    for (i, l) in advice.layers().iter().enumerate() {
        match l.placement {
            Placement::Plane(a) => {
                assert_eq!(l.blocker, Blocker::None, "layer {i}");
                assert_eq!(a.layer, i);
                planes.push(a.plane);
            }
            Placement::Compose => {
                // Statics are checked by the callers; the others fit a plane
                // alone and are composed for the count only.
                assert!(
                    matches!(l.blocker, Blocker::Combination | Blocker::Static),
                    "layer {i}: {:?}",
                    l.blocker
                );
            }
            Placement::Hidden => panic!("layer {i} hidden"),
        }
    }
    let target = advice.compose().expect("a composition assignment");
    assert_eq!(target.layer, composition_core::COMPOSE_LAYER);
    planes.push(target.plane);
    assert_eq!(advice.promoted(), planes.len() - 1);
    for p in &planes {
        let reach = caps.planes.iter().find(|c| c.id == *p).unwrap().outputs;
        assert!(reach & output_bit != 0, "plane {p:?} does not reach");
    }
    let mut sorted: Vec<u32> = planes.iter().map(|p| p.0).collect();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), planes.len(), "a plane is used twice");
}

fn check_direct(advice: &Advice, count: usize) {
    assert!(advice.is_direct());
    assert!(advice.compose().is_none());
    assert!(advice.compose_run().is_none());
    assert_eq!(advice.promoted(), count);
    assert!(advice
        .layers()
        .iter()
        .all(|l| matches!(l.placement, Placement::Plane(_)) && l.blocker == Blocker::None));
}

// ------------------------------------------------------------------ RK3588

mod rk3588 {
    use super::*;

    pub const VP0: OutputId = OutputId(0);
    pub const VP1: OutputId = OutputId(1);
    /// A third output no plane reaches.
    pub const VP2: OutputId = OutputId(2);
    pub const SCREEN: Size = Size::new(1920, 1080);
    pub const FULL: Rect = Rect::new(0, 0, 1920, 1080);

    const PLANES: [(u32, PlaneKind, u32, bool); 8] = [
        (33, PlaneKind::Primary, 0b01, false),
        (39, PlaneKind::Primary, 0b10, false),
        (45, PlaneKind::Overlay, 0b11, false),
        (51, PlaneKind::Overlay, 0b11, false),
        (57, PlaneKind::Overlay, 0b11, true),
        (63, PlaneKind::Overlay, 0b11, true),
        (69, PlaneKind::Overlay, 0b11, true),
        (75, PlaneKind::Overlay, 0b11, true),
    ];

    pub fn caps() -> Capabilities {
        let mut caps = Capabilities::default();
        for id in [VP0, VP1, VP2] {
            caps.outputs
                .push(OutputCaps {
                    id,
                    size: SCREEN,
                    refresh_mhz: 60_000,
                })
                .unwrap();
        }
        for (id, kind, outputs, esmart) in PLANES {
            let mut p = PlaneCaps {
                id: PlaneId(id),
                kind,
                outputs,
                zpos: Zpos::Range { min: 0, max: 7 },
                plane_alpha: true,
                blend_modes: BlendMode::Opaque.mask()
                    | BlendMode::Premultiplied.mask()
                    | BlendMode::Coverage.mask(),
                ..PlaneCaps::default()
            };
            p.formats.push(FormatModifier::linear(XR24)).unwrap();
            p.formats
                .push(FormatModifier::linear(PixelFormat::ARGB8888))
                .unwrap();
            if esmart {
                p.formats
                    .push(FormatModifier::linear(PixelFormat::NV12))
                    .unwrap();
            }
            caps.planes.push(p).unwrap();
        }
        caps
    }

    pub fn buffers(n: u32) -> Buffers {
        Buffers((1..=n).map(|i| (BufferId(i), info(SCREEN, XR24))).collect())
    }

    pub fn layers(n: u32) -> Vec<Layer> {
        (0..n)
            .map(|i| Layer::new(BufferId(1 + i), FULL, FULL, i as i32))
            .collect()
    }
}

/// Planes reaching a video port of the model: seven on either.
const RK_REACH: usize = 7;

#[test]
fn rk3588_frames_the_planner_accepts_need_nothing_composed() {
    let caps = rk3588::caps();
    for output in [rk3588::VP0, rk3588::VP1] {
        for n in 1..=RK_REACH as u32 {
            let buffers = rk3588::buffers(n);
            let layers = rk3588::layers(n);
            let frame = Frame {
                output,
                layers: &layers,
            };
            assert!(planner(&frame, &caps, &buffers).is_ok());
            for opts in [options(), AdviseOptions::default()] {
                let advice = run_advise(&frame, &caps, &buffers, &opts).unwrap();
                check_direct(&advice, n as usize);
            }
        }
    }
}

#[test]
fn rk3588_eight_layers_on_one_port_compose_the_minimal_run() {
    let caps = rk3588::caps();
    let buffers = rk3588::buffers(8);
    let layers = rk3588::layers(8);
    for output in [rk3588::VP0, rk3588::VP1] {
        let frame = Frame {
            output,
            layers: &layers,
        };
        // The planner: the eighth layer finds all seven planes in use.
        match planner(&frame, &caps, &buffers) {
            Err(PlanError::NoPlane { layer, reasons }) => {
                assert_eq!(layer, 7);
                assert!(reasons.contains(RejectReasons::IN_USE), "{reasons:?}");
            }
            other => panic!("expected NoPlane, got {other:?}"),
        }
        // Advice: seven planes for eight layers, one of them the composition
        // buffer, so exactly two layers are composed.
        let advice = run_advise(&frame, &caps, &buffers, &options()).unwrap();
        check_run(&frame, &caps, &advice, 2);
        for l in advice.layers() {
            if l.placement == Placement::Compose {
                assert_eq!(l.blocker, Blocker::Combination);
            }
        }
        reduced_frame_plans(&frame, &caps, &buffers, &advice, rk3588::SCREEN);
        // Port 1 cannot use plane 33, port 0 cannot use plane 39.
        let unreachable = if output == rk3588::VP0 { 39 } else { 33 };
        for l in advice.layers() {
            if let Placement::Plane(a) = l.placement {
                assert_ne!(a.plane.0, unreachable);
            }
        }
        assert_ne!(advice.compose().unwrap().plane.0, unreachable);
    }
}

#[test]
fn rk3588_run_length_grows_by_one_per_layer_over_the_reach() {
    // The run is `layers - planes + 1` long, up to MAX_LAYERS = 16.
    let caps = rk3588::caps();
    for n in 8..=16u32 {
        let buffers = rk3588::buffers(n);
        let layers = rk3588::layers(n);
        let frame = Frame {
            output: rk3588::VP0,
            layers: &layers,
        };
        let advice = run_advise(&frame, &caps, &buffers, &options()).unwrap();
        check_run(&frame, &caps, &advice, n as usize - RK_REACH + 1);
        assert_eq!(advice.promoted(), RK_REACH - 1);
        reduced_frame_plans(&frame, &caps, &buffers, &advice, rk3588::SCREEN);
    }
}

#[test]
fn rk3588_without_a_composition_target_every_layer_is_reported_composed() {
    let caps = rk3588::caps();
    let buffers = rk3588::buffers(8);
    let layers = rk3588::layers(8);
    let frame = Frame {
        output: rk3588::VP0,
        layers: &layers,
    };
    // `compose: None` forbids composition: the failing frame is reported as
    // all composed, with no composition assignment.
    let advice = run_advise(&frame, &caps, &buffers, &AdviseOptions::default()).unwrap();
    assert!(advice.compose().is_none());
    assert_eq!(advice.promoted(), 0);
    assert!(advice
        .layers()
        .iter()
        .all(|l| l.placement == Placement::Compose));
}

#[test]
fn rk3588_a_layer_no_plane_can_scale_is_composed_with_a_static_blocker() {
    // Every modelled plane is unscaled, so a 960x540 source on a full
    // destination has no plane on its own: a Static blocker with the
    // planner's SCALING reason, and the run must cover it even though the
    // frame has fewer layers than planes.
    let caps = rk3588::caps();
    let buffers = Buffers(vec![
        (BufferId(1), info(rk3588::SCREEN, XR24)),
        (BufferId(2), info(Size::new(960, 540), XR24)),
        (BufferId(3), info(rk3588::SCREEN, XR24)),
    ]);
    let layers = [
        Layer::new(BufferId(1), rk3588::FULL, rk3588::FULL, 0),
        Layer::new(BufferId(2), Rect::new(0, 0, 960, 540), rk3588::FULL, 1),
        Layer::new(BufferId(3), rk3588::FULL, rk3588::FULL, 2),
    ];
    let frame = Frame {
        output: rk3588::VP0,
        layers: &layers,
    };
    match planner(&frame, &caps, &buffers) {
        Err(PlanError::NoPlane { layer, reasons }) => {
            assert_eq!(layer, 1);
            assert!(reasons.contains(RejectReasons::SCALING), "{reasons:?}");
        }
        other => panic!("expected NoPlane, got {other:?}"),
    }
    let advice = run_advise(&frame, &caps, &buffers, &options()).unwrap();
    check_run(&frame, &caps, &advice, 1);
    assert_eq!(advice.compose_run(), Some((1, 1)));
    let l = advice.layers()[1];
    assert_eq!(l.placement, Placement::Compose);
    assert_eq!(l.blocker, Blocker::Static);
    assert!(
        l.reasons.contains(RejectReasons::SCALING),
        "{:?}",
        l.reasons
    );
    // Layers 0 and 2 stay on planes.
    assert_eq!(advice.promoted(), 2);
    reduced_frame_plans(&frame, &caps, &buffers, &advice, rk3588::SCREEN);
}

#[test]
fn rk3588_an_output_no_plane_reaches_cannot_be_helped() {
    let caps = rk3588::caps();
    let buffers = rk3588::buffers(2);
    let layers = rk3588::layers(2);
    let frame = Frame {
        output: rk3588::VP2,
        layers: &layers,
    };
    match planner(&frame, &caps, &buffers) {
        Err(PlanError::NoPlane { reasons, .. }) => {
            assert!(reasons.contains(RejectReasons::OUTPUT), "{reasons:?}");
        }
        other => panic!("expected NoPlane, got {other:?}"),
    }
    // Even the composition buffer has no plane.
    match run_advise(&frame, &caps, &buffers, &options()) {
        Err(AdviceError::NoComposePlane { reasons }) => {
            assert!(reasons.contains(RejectReasons::OUTPUT), "{reasons:?}");
        }
        other => panic!("expected NoComposePlane, got {other:?}"),
    }
    // Without composition every layer is Static with the OUTPUT reason.
    let advice = run_advise(&frame, &caps, &buffers, &AdviseOptions::default()).unwrap();
    for l in advice.layers() {
        assert_eq!(l.placement, Placement::Compose);
        assert_eq!(l.blocker, Blocker::Static);
        assert!(l.reasons.contains(RejectReasons::OUTPUT), "{:?}", l.reasons);
    }
}

#[test]
fn rk3588_advice_is_deterministic() {
    let caps = rk3588::caps();
    let buffers = rk3588::buffers(10);
    let layers = rk3588::layers(10);
    let frame = Frame {
        output: rk3588::VP1,
        layers: &layers,
    };
    let first = run_advise(&frame, &caps, &buffers, &options()).unwrap();
    for _ in 0..8 {
        assert_eq!(run_advise(&frame, &caps, &buffers, &options()), Ok(first));
    }
    // Nothing is remembered: another call in between changes nothing.
    let other_layers = rk3588::layers(3);
    let other = Frame {
        output: rk3588::VP0,
        layers: &other_layers,
    };
    run_advise(&other, &caps, &buffers, &options()).unwrap();
    assert_eq!(run_advise(&frame, &caps, &buffers, &options()), Ok(first));
}

// ---------------------------------------------------------------- BCM2711

mod bcm2711 {
    use super::*;

    pub const OUT: OutputId = OutputId(0);
    pub const SIZE: Size = Size::new(720, 1280);
    pub const PRIMARY_IDS: [u32; 6] = [48, 67, 79, 91, 103, 115];
    pub const DSI_PRIMARY: u32 = 79;
    pub const KEPT_OVERLAYS: [u32; 10] = [127, 138, 149, 160, 171, 182, 193, 204, 215, 226];
    pub const DROPPED_OVERLAYS: [u32; 6] = [237, 248, 259, 270, 281, 292];

    const FORMATS: [&[u8; 4]; 8] = [
        b"XR24", b"AR24", b"AB24", b"XB24", b"RG16", b"BG16", b"NV12", b"NV21",
    ];

    fn plane(id: u32, kind: PlaneKind, outputs: u32) -> PlaneCaps {
        let mut caps = PlaneCaps {
            id: PlaneId(id),
            kind,
            outputs,
            plane_alpha: true,
            blend_modes: BlendMode::Opaque.mask()
                | BlendMode::Premultiplied.mask()
                | BlendMode::Coverage.mask(),
            zpos: match kind {
                PlaneKind::Primary => Zpos::Fixed(0),
                _ => Zpos::Range { min: 1, max: 17 },
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

    fn base() -> Capabilities {
        let mut caps = Capabilities::default();
        caps.outputs
            .push(OutputCaps {
                id: OUT,
                size: SIZE,
                refresh_mhz: 60_000,
            })
            .unwrap();
        caps
    }

    /// Probe's cut view: six primaries then ten overlays; eleven planes
    /// reach the DSI output.
    pub fn cut_view() -> Capabilities {
        let mut caps = base();
        for id in PRIMARY_IDS {
            let outputs = u32::from(id == DSI_PRIMARY);
            caps.planes
                .push(plane(id, PlaneKind::Primary, outputs))
                .unwrap();
        }
        for id in KEPT_OVERLAYS {
            caps.planes.push(plane(id, PlaneKind::Overlay, 1)).unwrap();
        }
        caps
    }

    /// The reachable-first cut: the primary and fifteen overlays, sixteen
    /// planes reaching the output.
    pub fn reachable_first() -> Capabilities {
        let mut caps = base();
        caps.planes
            .push(plane(DSI_PRIMARY, PlaneKind::Primary, 1))
            .unwrap();
        for id in KEPT_OVERLAYS.iter().chain(&DROPPED_OVERLAYS[..5]) {
            caps.planes.push(plane(*id, PlaneKind::Overlay, 1)).unwrap();
        }
        caps
    }

    pub fn layers(n: u32) -> Vec<Layer> {
        (0..n)
            .map(|i| {
                let z = i as i32;
                let mut layer = Layer::new(
                    BufferId(100 + i),
                    Rect::new(0, 0, 64, 64),
                    Rect::new(z * 8, z * 8, 64, 64),
                    z,
                );
                layer.blend = BlendMode::Premultiplied;
                layer
            })
            .collect()
    }

    pub fn buffers(n: u32) -> Buffers {
        Buffers(
            (0..n)
                .map(|i| (BufferId(100 + i), info(Size::new(64, 64), XR24)))
                .collect(),
        )
    }
}

/// Planes reaching the DSI output in the cut view.
const HVS_REACH: usize = 11;

#[test]
fn hvs5_frames_the_planner_accepts_need_nothing_composed() {
    let caps = bcm2711::cut_view();
    for n in 1..=HVS_REACH as u32 {
        let buffers = bcm2711::buffers(n);
        let layers = bcm2711::layers(n);
        let frame = Frame {
            output: bcm2711::OUT,
            layers: &layers,
        };
        assert!(planner(&frame, &caps, &buffers).is_ok(), "{n} layers");
        let advice = run_advise(&frame, &caps, &buffers, &options()).unwrap();
        check_direct(&advice, n as usize);
    }
    // Sixteen layers on the sixteen planes of the reachable-first cut.
    let caps = bcm2711::reachable_first();
    let buffers = bcm2711::buffers(16);
    let layers = bcm2711::layers(16);
    let frame = Frame {
        output: bcm2711::OUT,
        layers: &layers,
    };
    assert!(planner(&frame, &caps, &buffers).is_ok());
    check_direct(
        &run_advise(&frame, &caps, &buffers, &options()).unwrap(),
        16,
    );
}

#[test]
fn hvs5_the_cut_view_composes_the_layers_over_eleven_planes() {
    let caps = bcm2711::cut_view();
    for n in 12..=16u32 {
        let buffers = bcm2711::buffers(n);
        let layers = bcm2711::layers(n);
        let frame = Frame {
            output: bcm2711::OUT,
            layers: &layers,
        };
        // The planner stops at the twelfth layer: no plane left.
        match planner(&frame, &caps, &buffers) {
            Err(PlanError::NoPlane { layer, reasons }) => {
                assert_eq!(layer, 11);
                assert!(reasons.contains(RejectReasons::IN_USE), "{reasons:?}");
                assert!(reasons.contains(RejectReasons::OUTPUT), "{reasons:?}");
            }
            other => panic!("expected NoPlane, got {other:?}"),
        }
        let advice = run_advise(&frame, &caps, &buffers, &options()).unwrap();
        check_run(&frame, &caps, &advice, n as usize - HVS_REACH + 1);
        reduced_frame_plans(&frame, &caps, &buffers, &advice, bcm2711::SIZE);
        // The five primaries of unconnected CRTCs are never used.
        let mut used: Vec<u32> = advice
            .layers()
            .iter()
            .filter_map(|l| match l.placement {
                Placement::Plane(a) => Some(a.plane.0),
                _ => None,
            })
            .collect();
        used.push(advice.compose().unwrap().plane.0);
        for id in bcm2711::PRIMARY_IDS {
            if id != bcm2711::DSI_PRIMARY {
                assert!(!used.contains(&id), "unreachable primary {id} used");
            }
        }
    }
}

#[test]
fn hvs5_seventeen_layers_are_a_whole_call_error_not_advice() {
    // Seventeen layers are the engine's limit, not a plane-count cut.
    let caps = bcm2711::reachable_first();
    let buffers = bcm2711::buffers(17);
    let layers = bcm2711::layers(17);
    let frame = Frame {
        output: bcm2711::OUT,
        layers: &layers,
    };
    assert_eq!(
        planner(&frame, &caps, &buffers).unwrap_err(),
        PlanError::TooManyLayers { count: 17 }
    );
    assert_eq!(
        run_advise(&frame, &caps, &buffers, &options()),
        Err(AdviceError::TooManyLayers { count: 17 })
    );
}

#[test]
fn hvs5_a_format_no_plane_lists_is_composed_with_a_format_blocker() {
    let caps = bcm2711::cut_view();
    let mut buffers = bcm2711::buffers(3);
    buffers.0[1].1.format = PixelFormat::fourcc(b"HWL0");
    let layers = bcm2711::layers(3);
    let frame = Frame {
        output: bcm2711::OUT,
        layers: &layers,
    };
    match planner(&frame, &caps, &buffers) {
        Err(PlanError::NoPlane { layer, reasons }) => {
            assert_eq!(layer, 1);
            assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
        }
        other => panic!("expected NoPlane, got {other:?}"),
    }
    let advice = run_advise(&frame, &caps, &buffers, &options()).unwrap();
    check_run(&frame, &caps, &advice, 1);
    assert_eq!(advice.compose_run(), Some((1, 1)));
    assert_eq!(advice.layers()[1].blocker, Blocker::Static);
    assert!(advice.layers()[1].reasons.contains(RejectReasons::FORMAT));
    assert_eq!(advice.layers()[0].blocker, Blocker::None);
    assert_eq!(advice.layers()[2].blocker, Blocker::None);
}

#[test]
fn hvs5_a_static_layer_inside_a_cut_frame_is_covered_by_the_run() {
    // Sixteen layers over eleven planes need six composed (16 - 11 + 1); the
    // static layer at position 7 lies inside the run the search finds.
    let caps = bcm2711::cut_view();
    let mut buffers = bcm2711::buffers(16);
    buffers.0[7].1.format = PixelFormat::fourcc(b"HWL0");
    let layers = bcm2711::layers(16);
    let frame = Frame {
        output: bcm2711::OUT,
        layers: &layers,
    };
    let advice = run_advise(&frame, &caps, &buffers, &options()).unwrap();
    check_run(&frame, &caps, &advice, 6);
    let (low, high) = advice.compose_run().unwrap();
    assert!(low <= 7 && 7 <= high, "{low}..={high}");
    assert_eq!(advice.layers()[7].blocker, Blocker::Static);
    for (i, l) in advice.layers().iter().enumerate() {
        if l.placement == Placement::Compose && i != 7 {
            assert_eq!(l.blocker, Blocker::Combination, "layer {i}");
        }
    }
}

#[test]
fn hvs5_advice_is_deterministic() {
    let caps = bcm2711::cut_view();
    let buffers = bcm2711::buffers(14);
    let layers = bcm2711::layers(14);
    let frame = Frame {
        output: bcm2711::OUT,
        layers: &layers,
    };
    let first = run_advise(&frame, &caps, &buffers, &options()).unwrap();
    for _ in 0..8 {
        assert_eq!(run_advise(&frame, &caps, &buffers, &options()), Ok(first));
    }
    // Capabilities rebuilt from scratch give the same advice.
    assert_eq!(
        run_advise(&frame, &bcm2711::cut_view(), &buffers, &options()),
        Ok(first)
    );
}

#[test]
fn hvs5_the_caller_check_sees_the_composing_candidate() {
    let caps = bcm2711::cut_view();
    let buffers = bcm2711::buffers(13);
    let layers = bcm2711::layers(13);
    let frame = Frame {
        output: bcm2711::OUT,
        layers: &layers,
    };
    let mut seen = 0;
    let advice = advise(&frame, &caps, &buffers, &options(), |p, at| {
        seen += 1;
        assert!(at.is_some(), "every candidate that plans composes");
        assert!(p.assignments().len() <= HVS_REACH);
        Verdict::Accept
    })
    .unwrap();
    assert!(seen >= 1);
    assert_eq!(advice.backend_checks(), seen);
    check_run(&frame, &caps, &advice, 3);
}

// -------------------------------------------------- planner through Engine

#[test]
fn the_engine_rejects_what_advice_composes_and_limits_end_in_an_error() {
    // The RK3588 scene through the engine on the fake backend: the frame is
    // refused with the reason the advice acts on.
    let mut e = Engine::new(FakeBackend::new(rk3588::caps()));
    for i in 1..=8 {
        e.register_buffer(BufferId(i), info(rk3588::SCREEN, XR24))
            .unwrap();
        e.finish_write(BufferId(i)).unwrap();
    }
    let layers = rk3588::layers(8);
    let frame = Frame {
        output: rk3588::VP0,
        layers: &layers,
    };
    match e.validate(&frame) {
        Err(Error::Plan(PlanError::NoPlane { reasons, .. })) => {
            assert!(reasons.contains(RejectReasons::IN_USE));
        }
        other => panic!("expected NoPlane, got {other:?}"),
    }
    let caps = rk3588::caps();
    let buffers = rk3588::buffers(8);
    let advice = run_advise(&frame, &caps, &buffers, &options()).unwrap();
    check_run(&frame, &caps, &advice, 2);
    // Only the frame as it is may be tried: it fails, nothing is accepted.
    let one = AdviseOptions {
        max_candidates: 1,
        limits: SearchLimits::default(),
        ..options()
    };
    assert_eq!(
        run_advise(&frame, &caps, &buffers, &one),
        Err(AdviceError::LimitExceeded)
    );
}
