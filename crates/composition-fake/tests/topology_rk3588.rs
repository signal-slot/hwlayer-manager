//! Planner and fake-engine checks on a model of the RK3588 (Rockchip VOP2)
//! topology recorded on a NanoPC-T6 in `docs/observed/rk3588-topology.md`
//! (kernel 6.18.55 `rockchip`, `/dev/dri/card0`, `video_port0`, 1920x1080 at
//! 60 Hz, observed 2026-11-28).
//!
//! This is a MODEL of the observed topology on the fake backend, not a
//! hardware result: nothing here ran on the board, and no TEST_ONLY or commit
//! was issued to the kernel. Each test names the observed row it models.
//!
//! Limits of the model, all inherited from the probe record:
//! - The probe prints at most 32 (format, modifier) pairs per plane, so the
//!   Cluster planes are modelled with the 32 pairs it printed, except that the
//!   last one is replaced by linear `XR24`, which the debugfs state shows
//!   Cluster0 scanning out (fbcon, modifier 0). Whether Cluster2/3 also
//!   accept linear is not recorded; the model says they do for XR24 only.
//! - "scaling not claimed" and the unread rotation bitmask are modelled as no
//!   scaling and `Normal` only. That is the probe's view, not a measured
//!   absence: the planner's rejections below say what it does with that
//!   description, not what the hardware does.
//! - Plane 39 (Cluster1, `video_port1`) has outputs mask 0 in the probe's
//!   single-output view, as recorded.

use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, DisplayEvent, Engine, Error, FormatModifier,
    Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId,
    PlaneKind, Rect, RejectReasons, Size, Transform, Zpos,
};
use composition_fake::{FakeBackend, PlaneState};

const OUT: OutputId = OutputId(0);
const SCREEN: Size = Size::new(1920, 1080);
const FULL: Rect = Rect::new(0, 0, 1920, 1080);

const XR24: PixelFormat = PixelFormat::XRGB8888;
const AR24: PixelFormat = PixelFormat::ARGB8888;
const NV12: PixelFormat = PixelFormat::NV12;

/// Arm AFBC modifiers as printed by the probe (`0x800000000000001`, `...041`
/// and `...011`).
const AFBC_1: Modifier = Modifier(0x800_0000_0000_0001);
const AFBC_41: Modifier = Modifier(0x800_0000_0000_0041);
const AFBC_11: Modifier = Modifier(0x800_0000_0000_0011);

// Plane ids as recorded.
const CLUSTER0: PlaneId = PlaneId(33);
const CLUSTER1: PlaneId = PlaneId(39);
const CLUSTER2: PlaneId = PlaneId(45);
const CLUSTER3: PlaneId = PlaneId(51);
const ESMART0: PlaneId = PlaneId(57);
const ESMART1: PlaneId = PlaneId(63);
const ESMART2: PlaneId = PlaneId(69);
const ESMART3: PlaneId = PlaneId(75);

fn fourcc(code: &[u8; 4]) -> PixelFormat {
    PixelFormat::fourcc(code)
}

/// Rows "plane 33 .. 51" (Cluster, 32 shown pairs): 14 formats with
/// `...001`, 14 with `...041`, the first 4 with `...011`.
fn cluster_formats() -> Vec<FormatModifier> {
    let all: [&[u8; 4]; 14] = [
        b"XR30", b"XB30", b"XR24", b"AR24", b"XB24", b"AB24", b"RG24", b"BG24", b"RG16", b"BG16",
        b"YU08", b"YU10", b"YUYV", b"Y210",
    ];
    let mut out = Vec::new();
    for (modifier, count) in [(AFBC_1, 14), (AFBC_41, 14), (AFBC_11, 4)] {
        for code in all.iter().take(count) {
            out.push(FormatModifier {
                format: fourcc(code),
                modifier,
            });
        }
    }
    assert_eq!(out.len(), 32, "the probe printed 32 pairs");
    // Observed in debugfs: Cluster0 scans out linear XR24 (fbcon).
    *out.last_mut().unwrap() = FormatModifier::linear(XR24);
    out
}

/// Rows "plane 57 .. 75" (Esmart, 21 linear formats).
fn esmart_formats() -> Vec<FormatModifier> {
    [
        b"XR24", b"AR24", b"XB24", b"AB24", b"RG24", b"BG24", b"RG16", b"BG16", b"NV12", b"NV21",
        b"NV16", b"NV61", b"NV20", b"NV24", b"NV42", b"NV30", b"NV15", b"YVYU", b"VYUY", b"YUYV",
        b"UYVY",
    ]
    .iter()
    .map(|c| FormatModifier::linear(fourcc(c)))
    .collect()
}

/// One recorded plane: zpos 0..=7, plane alpha, blend none/premultiplied/
/// coverage, no scaling, no transform, `outputs` as probed.
fn plane(id: PlaneId, kind: PlaneKind, outputs: u32, formats: Vec<FormatModifier>) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id,
        kind,
        outputs,
        zpos: Zpos::Range { min: 0, max: 7 },
        plane_alpha: true,
        blend_modes: BlendMode::Opaque.mask()
            | BlendMode::Premultiplied.mask()
            | BlendMode::Coverage.mask(),
        ..PlaneCaps::default()
    };
    for f in formats {
        caps.formats.push(f).unwrap();
    }
    caps
}

/// The eight recorded planes of `/dev/dri/card0`, output 0 = `video_port0`.
fn planes() -> Vec<PlaneCaps> {
    use PlaneKind::{Overlay, Primary};
    vec![
        plane(CLUSTER0, Primary, 0b1, cluster_formats()),
        plane(CLUSTER1, Primary, 0b0, cluster_formats()),
        plane(CLUSTER2, Overlay, 0b1, cluster_formats()),
        plane(CLUSTER3, Overlay, 0b1, cluster_formats()),
        plane(ESMART0, Overlay, 0b1, esmart_formats()),
        plane(ESMART1, Overlay, 0b1, esmart_formats()),
        plane(ESMART2, Overlay, 0b1, esmart_formats()),
        plane(ESMART3, Overlay, 0b1, esmart_formats()),
    ]
}

fn caps_of(planes: &[PlaneCaps]) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SCREEN,
            refresh_mhz: 60_000,
        })
        .unwrap();
    for p in planes {
        caps.planes.push(*p).unwrap();
    }
    caps
}

fn rk3588_caps() -> Capabilities {
    caps_of(&planes())
}

/// An engine with buffers `1..=n` as given by (format, modifier).
fn engine_with(caps: Capabilities, buffers: &[(PixelFormat, Modifier)]) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(caps));
    for (i, (format, modifier)) in buffers.iter().enumerate() {
        let id = BufferId(i as u32 + 1);
        engine
            .register_buffer(
                id,
                BufferInfo {
                    size: SCREEN,
                    format: *format,
                    modifier: *modifier,
                },
            )
            .unwrap();
        engine.finish_write(id).unwrap();
    }
    engine
}

fn layer(buffer: u32, z: i32) -> Layer {
    Layer::new(BufferId(buffer), FULL, FULL, z)
}

fn present(engine: &mut Engine<FakeBackend>, layers: &[Layer]) -> Result<(), Error> {
    engine
        .present(&Frame {
            output: OUT,
            layers,
        })
        .map(|_| ())
}

fn placement(engine: &mut Engine<FakeBackend>, layers: &[Layer]) -> Vec<(usize, u32, i32)> {
    let plan = engine
        .validate(&Frame {
            output: OUT,
            layers,
        })
        .unwrap();
    plan.assignments()
        .iter()
        .map(|a| (a.layer, a.plane.0, a.zpos))
        .collect()
}

fn no_plane(result: Result<(), Error>, expected_layer: usize) -> RejectReasons {
    match result {
        Err(Error::Plan(PlanError::NoPlane { layer, reasons })) => {
            assert_eq!(layer, expected_layer);
            reasons
        }
        other => panic!("expected NoPlane at layer {expected_layer}, got {other:?}"),
    }
}

#[test]
fn model_matches_the_recorded_rows() {
    // Rows "Planes": eight planes, two Primary, six Overlay, zpos 0..=7.
    let caps = rk3588_caps();
    assert_eq!(caps.planes.len(), 8);
    assert_eq!(caps.outputs.len(), 1);
    assert_eq!(
        caps.planes
            .iter()
            .filter(|p| p.kind == PlaneKind::Primary)
            .count(),
        2
    );
    assert!(caps
        .planes
        .iter()
        .all(|p| p.zpos == Zpos::Range { min: 0, max: 7 }));
}

#[test]
fn two_layers_on_two_planes_commit_and_scan_out_in_z_order() {
    // Rows "plane 33" (Cluster0, linear XR24 per debugfs) and "plane 57"
    // (Esmart0, AR24), reset-state zpos order Cluster0 < Esmart0.
    let mut engine = engine_with(
        rk3588_caps(),
        &[(XR24, Modifier::LINEAR), (AR24, Modifier::LINEAR)],
    );
    let mut top = layer(2, 1);
    top.blend = BlendMode::Premultiplied;
    top.alpha = 0x8000;
    let layers = [layer(1, 0), top];
    // Plane 39 is skipped (outputs 0b0); Cluster2/3 lack linear AR24 here.
    assert_eq!(placement(&mut engine, &layers), [(0, 33, 0), (1, 57, 1)]);

    let sub = engine
        .present(&Frame {
            output: OUT,
            layers: &layers,
        })
        .unwrap();
    engine.backend_mut().vsync();
    let mut events = Vec::new();
    while let Some(e) = engine.poll_event().unwrap() {
        events.push(e);
    }
    assert_eq!(
        events,
        [DisplayEvent::Presented {
            output: OUT,
            submission: sub
        }]
    );
    assert_eq!(
        engine.backend().scanout(OUT).unwrap(),
        [
            PlaneState {
                plane: CLUSTER0,
                buffer: BufferId(1),
                zpos: 0
            },
            PlaneState {
                plane: ESMART0,
                buffer: BufferId(2),
                zpos: 1
            },
        ]
    );
}

#[test]
fn afbc_layers_take_cluster_planes_only() {
    // Rows "plane 33/45/51": AFBC modifiers only on Cluster planes; Esmart
    // planes are "linear only". Plane 39 has outputs 0b0.
    let afbc = (XR24, AFBC_1);
    let mut engine = engine_with(rk3588_caps(), &[afbc; 4]);
    let three = [layer(1, 0), layer(2, 1), layer(3, 2)];
    assert_eq!(
        placement(&mut engine, &three),
        [(0, 33, 0), (1, 45, 1), (2, 51, 2)]
    );
    // A fourth AFBC layer has no plane: Esmart lacks the modifier, Cluster1
    // is not attached to this output.
    let four = [layer(1, 0), layer(2, 1), layer(3, 2), layer(4, 3)];
    let reasons = no_plane(present(&mut engine, &four), 3);
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
    assert_eq!(engine.backend().commit_count(), 0);
}

#[test]
fn unsupported_format_or_modifier_is_rejected_by_capability_check() {
    // Rows "plane 33..75" formats: no AR30 anywhere, no unlisted modifier,
    // YU08 only with AFBC on Cluster, NV12 only on Esmart.
    let bufs = [
        (fourcc(b"AR30"), Modifier::LINEAR),
        (XR24, Modifier(0x0100_0000_0000_0001)),
        (fourcc(b"YU08"), Modifier::LINEAR),
    ];
    let mut engine = engine_with(rk3588_caps(), &bufs);
    for i in 1..=3 {
        let reasons = no_plane(present(&mut engine, &[layer(i, 0)]), 0);
        assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
    }
    assert_eq!(
        engine.backend().check_count(),
        0,
        "refused before TEST_ONLY"
    );

    // NV12 linear is accepted, but only by an Esmart plane.
    let mut engine = engine_with(rk3588_caps(), &[(NV12, Modifier::LINEAR)]);
    assert_eq!(placement(&mut engine, &[layer(1, 0)]), [(0, 57, 0)]);
}

#[test]
fn plane_39_is_not_usable_on_the_probed_output() {
    // Row "plane 39": Primary, outputs 0b0 (possible CRTC not in the probe's
    // single-output view).
    let only39: Vec<_> = planes().into_iter().filter(|p| p.id == CLUSTER1).collect();
    let mut engine = engine_with(caps_of(&only39), &[(XR24, Modifier::LINEAR)]);
    let reasons = no_plane(present(&mut engine, &[layer(1, 0)]), 0);
    assert!(reasons.contains(RejectReasons::OUTPUT), "{reasons:?}");
}

#[test]
fn more_layers_than_usable_planes_is_refused() {
    // Rows "Planes": eight planes, one of them (39) not on video_port0, so
    // seven can be used: three AFBC layers on Cluster, four AR24 on Esmart.
    let mut bufs = vec![(XR24, AFBC_1); 3];
    bufs.extend(vec![(AR24, Modifier::LINEAR); 5]);
    let mut engine = engine_with(rk3588_caps(), &bufs);
    let seven: Vec<_> = (0..7).map(|i| layer(i + 1, i as i32)).collect();
    let placed = placement(&mut engine, &seven);
    let used: Vec<u32> = placed.iter().map(|p| p.1).collect();
    assert_eq!(used, [33, 45, 51, 57, 63, 69, 75]);
    assert_eq!(
        placed.iter().map(|p| p.2).collect::<Vec<_>>(),
        (0..7).collect::<Vec<_>>(),
        "zpos stays inside the recorded 0..=7"
    );
    present(&mut engine, &seven).unwrap();

    // An eighth layer exceeds the usable planes, and a fifth linear AR24
    // layer exceeds the Esmart planes that take it.
    let eight: Vec<_> = (0..8).map(|i| layer(i + 1, i as i32)).collect();
    let reasons = no_plane(present(&mut engine, &eight), 7);
    assert!(reasons.contains(RejectReasons::IN_USE), "{reasons:?}");
    let five_linear: Vec<_> = (4..9).map(|i| layer(i, i as i32)).collect();
    let reasons = no_plane(present(&mut engine, &five_linear), 4);
    assert!(reasons.contains(RejectReasons::IN_USE), "{reasons:?}");
}

#[test]
fn scaling_is_rejected_because_the_probe_claims_none() {
    // Rows "scaling not claimed" (all planes). Model only: the probe does not
    // claim scaling, so the planner has no plane that may scale.
    let mut engine = engine_with(rk3588_caps(), &[(XR24, Modifier::LINEAR)]);
    let scaled = Layer::new(BufferId(1), FULL, Rect::new(0, 0, 960, 540), 0);
    let reasons = no_plane(present(&mut engine, &[scaled]), 0);
    assert!(reasons.contains(RejectReasons::SCALING), "{reasons:?}");
}

#[test]
fn rotation_is_rejected_because_only_the_none_value_was_read() {
    // Row "rotation=1 only" (debugfs state; bitmask not read). Model only.
    let mut engine = engine_with(rk3588_caps(), &[(XR24, Modifier::LINEAR)]);
    for t in [Transform::Rotate90, Transform::Rotate180, Transform::FlipH] {
        let mut l = layer(1, 0);
        l.transform = t;
        let reasons = no_plane(present(&mut engine, &[l]), 0);
        assert!(
            reasons.contains(RejectReasons::TRANSFORM),
            "{t:?}: {reasons:?}"
        );
    }
}

#[test]
fn plane_alpha_and_all_three_blend_modes_are_accepted() {
    // Rows "alpha yes, blend none/premultiplied/coverage" on every plane.
    let mut engine = engine_with(rk3588_caps(), &[(AR24, Modifier::LINEAR)]);
    for blend in [
        BlendMode::Opaque,
        BlendMode::Premultiplied,
        BlendMode::Coverage,
    ] {
        let mut l = layer(1, 0);
        l.blend = blend;
        l.alpha = 0x4000;
        assert_eq!(placement(&mut engine, &[l]).len(), 1, "{blend:?}");
    }
}

#[test]
fn alpha_on_a_plane_without_alpha_is_rejected_hypothetical() {
    // No recorded plane lacks alpha, so this is NOT an observed row: the same
    // topology with plane alpha cleared, to show the planner's answer.
    let mut no_alpha = planes();
    for p in &mut no_alpha {
        p.plane_alpha = false;
    }
    let mut engine = engine_with(caps_of(&no_alpha), &[(AR24, Modifier::LINEAR)]);
    let mut l = layer(1, 0);
    l.alpha = 0x8000;
    let reasons = no_plane(present(&mut engine, &[l]), 0);
    assert!(reasons.contains(RejectReasons::ALPHA), "{reasons:?}");
    // Opaque alpha still places.
    assert_eq!(placement(&mut engine, &[layer(1, 0)]).len(), 1);
}

#[test]
fn test_only_rejection_falls_back_to_the_next_plane() {
    // Rows "plane 33 / 45": an AFBC layer fits Cluster0, Cluster2 and
    // Cluster3. When the backend check (the TEST_ONLY stand-in) refuses
    // Cluster0, the planner offers the next assignment.
    let mut engine = engine_with(rk3588_caps(), &[(XR24, AFBC_41)]);
    engine
        .backend_mut()
        .set_check_hook(|_, plan| plan.assignments().iter().all(|a| a.plane != CLUSTER0));
    assert_eq!(placement(&mut engine, &[layer(1, 0)]), [(0, 45, 0)]);

    // When it refuses everything the planner reports a backend rejection,
    // not a capability failure.
    engine.backend_mut().set_check_hook(|_, _| false);
    let result = present(&mut engine, &[layer(1, 0)]);
    assert!(
        matches!(
            result,
            Err(Error::Plan(PlanError::BackendRejected { candidates })) if candidates >= 1
        ),
        "{result:?}"
    );
    assert_eq!(engine.backend().commit_count(), 0);
}
