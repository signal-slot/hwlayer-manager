//! `DriverProfile::Generic` on the recorded RK3588 (NanoPC-T6) topology, for
//! frames whose layers need upscaling, downscaling or no scaling.
//!
//! Baseline tests for the later `Rk3588Vop2` profile: they pin what the
//! generic planner does today. No recorded plane exposes scaling, so the
//! planner refuses every scaled layer. The planner has no fallback
//! composition: a frame either maps completely onto planes or is rejected
//! with `PlanError::NoPlane`, which names the first layer it could not place.
//! "Goes to composition" therefore shows up here as that error, and a layer
//! that needs scaling is never split off while the others get planes.
//!
//! Fixture data, copied here so that nothing reads `docs/`:
//! - `tests/data/rk3588/topology-2026-11-28.md`, observed 2026-11-28 (kernel
//!   6.18.55-current-rockchip64): plane kinds, outputs masks, zpos 0..=7, plane
//!   alpha, blend modes, and the probe's format lists (at most 32 pairs per
//!   plane, so the Cluster lists are cut and hold no linear pair).
//! - `tests/data/rk3588/source-2026-11-28.md`, read 2026-11-28 (vendor develop-6.1):
//!   Esmart planes list linear only; AFBC only on Cluster planes.
//! - Plan #457 scen-t6 handoff, observed 2026-11-27 (kernel
//!   6.18.55-current-rockchip64): 8 planes, zpos 0..7 on every plane, no plane
//!   scales, planes 39 and 69 cannot reach video_port0.

use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, Size, Zpos,
};
use composition_drm::DriverProfile;

const OUT: OutputId = OutputId(0);

/// Arm AFBC modifiers as the probe printed them (observed 2026-11-28).
const AFBC_A: Modifier = Modifier(0x0800_0000_0000_0001);
const AFBC_B: Modifier = Modifier(0x0800_0000_0000_0041);
const AFBC_C: Modifier = Modifier(0x0800_0000_0000_0011);

const CLUSTER_FORMATS: [&[u8; 4]; 14] = [
    b"XR30", b"XB30", b"XR24", b"AR24", b"XB24", b"AB24", b"RG24", b"BG24", b"RG16", b"BG16",
    b"YU08", b"YU10", b"YUYV", b"Y210",
];

const ESMART_FORMATS: [&[u8; 4]; 21] = [
    b"XR24", b"AR24", b"XB24", b"AB24", b"RG24", b"BG24", b"RG16", b"BG16", b"NV12", b"NV21",
    b"NV16", b"NV61", b"NV20", b"NV24", b"NV42", b"NV30", b"NV15", b"YVYU", b"VYUY", b"YUYV",
    b"UYVY",
];

/// The probe's 32 pairs of a Cluster plane: 14 + 14 + 4.
fn cluster_pairs() -> Vec<FormatModifier> {
    let mut pairs = Vec::new();
    for (modifier, count) in [(AFBC_A, 14), (AFBC_B, 14), (AFBC_C, 4)] {
        for code in &CLUSTER_FORMATS[..count] {
            pairs.push(FormatModifier {
                format: PixelFormat::fourcc(code),
                modifier,
            });
        }
    }
    pairs
}

fn esmart_pairs() -> Vec<FormatModifier> {
    ESMART_FORMATS
        .iter()
        .map(|c| FormatModifier::linear(PixelFormat::fourcc(c)))
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Cluster,
    Esmart,
}

/// (plane id, kind, class, outputs mask on output 0)
const PLANES: [(u32, PlaneKind, Class, u32); 8] = [
    (33, PlaneKind::Primary, Class::Cluster, 0b1),
    (39, PlaneKind::Primary, Class::Cluster, 0b0),
    (45, PlaneKind::Overlay, Class::Cluster, 0b1),
    (51, PlaneKind::Overlay, Class::Cluster, 0b1),
    (57, PlaneKind::Overlay, Class::Esmart, 0b1),
    (63, PlaneKind::Overlay, Class::Esmart, 0b1),
    (69, PlaneKind::Overlay, Class::Esmart, 0b0),
    (75, PlaneKind::Overlay, Class::Esmart, 0b1),
];

/// Reachable planes on output 0.
const CLUSTER_REACH: [u32; 3] = [33, 45, 51];
const ESMART_REACH: [u32; 3] = [57, 63, 75];

/// All eight planes, no plane scales, after the Generic profile.
fn caps() -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(1920, 1080),
            refresh_mhz: 60_000,
        })
        .unwrap();
    for &(id, kind, class, outputs) in PLANES.iter() {
        let mut plane = PlaneCaps {
            id: PlaneId(id),
            kind,
            outputs,
            scaling: None,
            max_source: None,
            plane_alpha: true,
            blend_modes: BlendMode::Opaque.mask()
                | BlendMode::Premultiplied.mask()
                | BlendMode::Coverage.mask(),
            zpos: Zpos::Range { min: 0, max: 7 },
            ..PlaneCaps::default()
        };
        let pairs = match class {
            Class::Cluster => cluster_pairs(),
            Class::Esmart => esmart_pairs(),
        };
        for pair in pairs {
            plane.formats.push(pair).unwrap();
        }
        caps.planes.push(plane).unwrap();
    }
    DriverProfile::Generic.apply(&mut caps);
    caps
}

const LIN: BufferId = BufferId(1);
const AFBC: BufferId = BufferId(2);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let modifier = match buffer {
            LIN => Modifier::LINEAR,
            AFBC => AFBC_A,
            _ => return None,
        };
        Some(BufferInfo {
            size: Size::new(64, 64),
            format: PixelFormat::XRGB8888,
            modifier,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scale {
    None,
    Up,
    Down,
}

/// A 64x64 source in its own destination cell: 64x64 (no scaling), 128x128
/// (2x up) or 32x32 (2x down).
fn layer(buffer: BufferId, n: usize, scale: Scale) -> Layer {
    let side = match scale {
        Scale::None => 64,
        Scale::Up => 128,
        Scale::Down => 32,
    };
    Layer::new(
        buffer,
        Rect::new(0, 0, 64, 64),
        Rect::new((n as i32) * 130, 0, side, side),
        n as i32,
    )
}

/// Layers `0..n`: the first three are AFBC (Cluster planes), the rest linear
/// (Esmart planes). Layer `scaled` gets `scale`, the others none.
fn mixed(n: usize, scaled: Option<usize>, scale: Scale) -> Vec<Layer> {
    (0..n)
        .map(|i| {
            let buffer = if i < 3 { AFBC } else { LIN };
            let s = if Some(i) == scaled {
                scale
            } else {
                Scale::None
            };
            layer(buffer, i, s)
        })
        .collect()
}

fn plan_frame(caps: &Capabilities, layers: &[Layer]) -> Result<Vec<u32>, PlanError> {
    let frame = Frame {
        output: OUT,
        layers,
    };
    let p = plan(&frame, caps, &Buffers)?;
    let mut ids: Vec<u32> = p.assignments().iter().map(|a| a.plane.0).collect();
    ids.sort_unstable();
    Ok(ids)
}

fn scaling_refusal(r: Result<Vec<u32>, PlanError>, expected_layer: usize) {
    match r {
        Err(PlanError::NoPlane { layer, reasons }) => {
            assert_eq!(layer, expected_layer, "{reasons:?}");
            assert!(reasons.contains(RejectReasons::SCALING), "{reasons:?}");
        }
        other => panic!("expected NoPlane on layer {expected_layer}, got {other:?}"),
    }
}

// ---- no scaling: planes as far as the plane count allows ----

#[test]
fn unscaled_one_layer_gets_a_plane() {
    let caps = caps();
    for buffer in [LIN, AFBC] {
        let ids = plan_frame(&caps, &[layer(buffer, 0, Scale::None)]).unwrap();
        assert_eq!(ids.len(), 1);
        let reach = if buffer == AFBC {
            CLUSTER_REACH
        } else {
            ESMART_REACH
        };
        assert!(reach.contains(&ids[0]), "{ids:?}");
    }
}

#[test]
fn unscaled_four_layers_split_across_both_classes() {
    let caps = caps();
    let ids = plan_frame(&caps, &mixed(4, None, Scale::None)).unwrap();
    assert_eq!(ids.iter().filter(|i| CLUSTER_REACH.contains(i)).count(), 3);
    assert_eq!(ids.iter().filter(|i| ESMART_REACH.contains(i)).count(), 1);
}

#[test]
fn unscaled_eight_layers_fail_on_plane_count_not_scaling() {
    // Six planes reach the output (39 and 69 do not). The seventh layer is
    // the one that finds no plane, and scaling is not the reason.
    let caps = caps();
    assert_eq!(
        plan_frame(&caps, &mixed(6, None, Scale::None))
            .unwrap()
            .len(),
        6
    );
    match plan_frame(&caps, &mixed(8, None, Scale::None)) {
        Err(PlanError::NoPlane { layer, reasons }) => {
            assert_eq!(layer, 6);
            assert!(!reasons.contains(RejectReasons::SCALING), "{reasons:?}");
        }
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

// ---- every layer scaled: refused at layer 0 ----

/// Every layer needs `scale`. Up to the six reachable planes the first layer
/// is refused for scaling. With eight layers the plane count check comes
/// first: layer 6 gets the IN_USE and OUTPUT reasons and scaling is not
/// mentioned, so the scaling refusal is hidden behind the count refusal.
fn all_scaled(scale: Scale) {
    let caps = caps();
    for buffer in [LIN, AFBC] {
        for n in [1, 4] {
            let layers: Vec<Layer> = (0..n).map(|i| layer(buffer, i, scale)).collect();
            scaling_refusal(plan_frame(&caps, &layers), 0);
        }
        let layers: Vec<Layer> = (0..8).map(|i| layer(buffer, i, scale)).collect();
        match plan_frame(&caps, &layers) {
            Err(PlanError::NoPlane { layer, reasons }) => {
                assert_eq!(layer, 6);
                assert!(reasons.contains(RejectReasons::IN_USE), "{reasons:?}");
                assert!(reasons.contains(RejectReasons::OUTPUT), "{reasons:?}");
                assert!(!reasons.contains(RejectReasons::SCALING), "{reasons:?}");
            }
            other => panic!("expected NoPlane, got {other:?}"),
        }
    }
}

#[test]
fn upscaled_layers_are_refused_for_1_4_and_8_layers() {
    all_scaled(Scale::Up);
}

#[test]
fn downscaled_layers_are_refused_for_1_4_and_8_layers() {
    all_scaled(Scale::Down);
}

// ---- one scaled layer among unscaled ones: the whole frame is refused ----

#[test]
fn one_scaled_layer_rejects_the_whole_frame() {
    // The unscaled layers would fit on planes, but nothing is split off to
    // composition: the error names the scaled layer.
    let caps = caps();
    for scale in [Scale::Up, Scale::Down] {
        for (n, scaled) in [(1, 0), (4, 0), (4, 2), (4, 3), (6, 5)] {
            scaling_refusal(plan_frame(&caps, &mixed(n, Some(scaled), scale)), scaled);
        }
    }
}

#[test]
fn an_unscaled_prefix_does_not_help_the_scaled_layer() {
    // The same frames without the scaled layer are planned.
    let caps = caps();
    for n in [1, 3, 5] {
        assert_eq!(
            plan_frame(&caps, &mixed(n, None, Scale::None))
                .unwrap()
                .len(),
            n
        );
    }
}
