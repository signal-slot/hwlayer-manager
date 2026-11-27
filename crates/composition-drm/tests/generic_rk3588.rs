//! `DriverProfile::Generic` on the recorded RK3588 (NanoPC-T6) topology.
//!
//! Baseline tests: they pin what the generic planner does with the recorded
//! data, so that the later `Rk3588Vop2` tests show the difference.
//!
//! Fixture data, copied here so that nothing reads `docs/observed`:
//! - `docs/observed/rk3588-topology.md`, observed 2026-11-28 (kernel
//!   6.18.55-current-rockchip64): plane kinds, outputs masks, zpos 0..=7, plane
//!   alpha, blend modes (none, premultiplied, coverage), and the probe's format
//!   lists (the probe prints at most 32 pairs per plane, so the Cluster lists
//!   are cut: they hold no linear pair and only the first four formats of the
//!   third AFBC group).
//! - `docs/observed/rk3588-source.md`, read 2026-11-28 (vendor develop-6.1):
//!   the Esmart planes list linear only; AFBC only on Cluster planes.
//! - Plan #457 scen-t6 handoff, observed 2026-11-27 (kernel
//!   6.18.55-current-rockchip64): 8 planes, zpos 0..7 on every plane, no plane
//!   scales, planes 39 and 69 cannot reach video_port0, and the kernel lease
//!   holds primary 33 plus overlays 45/51/57/63/69/75.
//!
//! The topology note shows plane 69 with outputs 0b1; the handoff says it
//! cannot reach video_port0. The fixtures follow the handoff.

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

/// Planes the kernel lease holds (observed 2026-11-27).
const LEASED: [u32; 7] = [33, 45, 51, 57, 63, 69, 75];

fn caps_with(ids: &[u32]) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(1920, 1080),
            refresh_mhz: 60_000,
        })
        .unwrap();
    for &(id, kind, class, outputs) in PLANES.iter().filter(|p| ids.contains(&p.0)) {
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
    caps
}

/// All eight planes.
fn full_caps() -> Capabilities {
    let ids: Vec<u32> = PLANES.iter().map(|p| p.0).collect();
    caps_with(&ids)
}

/// Only the seven leased planes.
fn leased_caps() -> Capabilities {
    caps_with(&LEASED)
}

fn generic(mut caps: Capabilities) -> Capabilities {
    DriverProfile::Generic.apply(&mut caps);
    caps
}

const LIN: BufferId = BufferId(1);
const AFBC: BufferId = BufferId(2);
const SMALL: BufferId = BufferId(3);
const NV12: BufferId = BufferId(4);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let (size, format, modifier) = match buffer {
            LIN => (Size::new(64, 64), PixelFormat::XRGB8888, Modifier::LINEAR),
            AFBC => (Size::new(64, 64), PixelFormat::XRGB8888, AFBC_A),
            SMALL => (Size::new(32, 32), PixelFormat::XRGB8888, Modifier::LINEAR),
            NV12 => (Size::new(64, 64), PixelFormat::NV12, AFBC_A),
            _ => return None,
        };
        Some(BufferInfo {
            size,
            format,
            modifier,
        })
    }
}

/// A 64x64 layer in its own cell, so unscaled and not overlapping.
fn layer(buffer: BufferId, n: usize) -> Layer {
    let x = (n as i32) * 100;
    Layer::new(
        buffer,
        Rect::new(0, 0, 64, 64),
        Rect::new(x, 0, 64, 64),
        n as i32,
    )
}

fn plane_ids(caps: &Capabilities, layers: &[Layer]) -> Result<Vec<u32>, PlanError> {
    let frame = Frame {
        output: OUT,
        layers,
    };
    let p = plan(&frame, caps, &Buffers)?;
    let mut ids: Vec<u32> = p.assignments().iter().map(|a| a.plane.0).collect();
    ids.sort_unstable();
    Ok(ids)
}

fn reachable(caps: &Capabilities) -> Vec<u32> {
    caps.planes
        .iter()
        .filter(|p| p.outputs & 1 != 0)
        .map(|p| p.id.0)
        .collect()
}

const CLUSTER_REACH: [u32; 3] = [33, 45, 51];
const ESMART_REACH: [u32; 3] = [57, 63, 75];

fn no_plane_reasons(r: Result<Vec<u32>, PlanError>) -> RejectReasons {
    match r {
        Err(PlanError::NoPlane { reasons, .. }) => reasons,
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

// ---- the fixtures ----

#[test]
fn fixture_matches_the_recorded_topology() {
    let caps = full_caps();
    assert_eq!(caps.planes.len(), 8);
    for p in caps.planes.iter() {
        assert_eq!(p.zpos, Zpos::Range { min: 0, max: 7 });
        assert_eq!(p.scaling, None);
        assert!(p.plane_alpha);
        let afbc = p.formats.iter().any(|f| f.modifier != Modifier::LINEAR);
        let cluster = matches!(p.id.0, 33 | 39 | 45 | 51);
        assert_eq!(afbc, cluster, "plane {}", p.id.0);
        assert_eq!(p.formats.len(), if cluster { 32 } else { 21 });
    }
    assert_eq!(reachable(&caps), [33, 45, 51, 57, 63, 75]);
}

#[test]
fn leased_fixture_has_the_seven_leased_planes() {
    let caps = leased_caps();
    let ids: Vec<u32> = caps.planes.iter().map(|p| p.id.0).collect();
    assert_eq!(ids, LEASED);
    assert_eq!(reachable(&caps), [33, 45, 51, 57, 63, 75]);
}

// ---- Generic changes nothing ----

#[test]
fn generic_leaves_both_fixtures_unchanged() {
    for caps in [full_caps(), leased_caps()] {
        assert_eq!(generic(caps), caps);
    }
}

// ---- how many layers are placed ----

#[test]
fn afbc_layers_fill_the_reachable_cluster_planes() {
    for caps in [generic(full_caps()), generic(leased_caps())] {
        for n in 1..=3 {
            let layers: Vec<Layer> = (0..n).map(|i| layer(AFBC, i)).collect();
            let ids = plane_ids(&caps, &layers).unwrap();
            assert_eq!(ids.len(), n);
            assert!(ids.iter().all(|id| CLUSTER_REACH.contains(id)), "{ids:?}");
        }
        let layers: Vec<Layer> = (0..4).map(|i| layer(AFBC, i)).collect();
        let reasons = no_plane_reasons(plane_ids(&caps, &layers));
        assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
    }
}

#[test]
fn linear_layers_fill_the_reachable_esmart_planes() {
    // The recorded Cluster lists hold no linear pair (the probe cut them at
    // 32), so linear RGB lands on the Esmart planes only.
    for caps in [generic(full_caps()), generic(leased_caps())] {
        for n in 1..=3 {
            let layers: Vec<Layer> = (0..n).map(|i| layer(LIN, i)).collect();
            let ids = plane_ids(&caps, &layers).unwrap();
            assert_eq!(ids.len(), n);
            assert!(ids.iter().all(|id| ESMART_REACH.contains(id)), "{ids:?}");
        }
        let layers: Vec<Layer> = (0..4).map(|i| layer(LIN, i)).collect();
        assert!(matches!(
            plane_ids(&caps, &layers),
            Err(PlanError::NoPlane { .. })
        ));
    }
}

#[test]
fn mixed_layers_use_all_six_reachable_planes_and_a_seventh_fails() {
    for caps in [generic(full_caps()), generic(leased_caps())] {
        let mut layers: Vec<Layer> = (0..3).map(|i| layer(AFBC, i)).collect();
        layers.extend((3..6).map(|i| layer(LIN, i)));
        let ids = plane_ids(&caps, &layers).unwrap();
        assert_eq!(ids, reachable(&caps));
        layers.push(layer(LIN, 6));
        assert!(matches!(
            plane_ids(&caps, &layers),
            Err(PlanError::NoPlane { .. })
        ));
    }
}

// ---- AFBC only where listed ----

#[test]
fn afbc_buffer_is_placed_on_a_cluster_plane_only() {
    let caps = generic(full_caps());
    let ids = plane_ids(&caps, &[layer(AFBC, 0)]).unwrap();
    assert_eq!(ids.len(), 1);
    assert!(CLUSTER_REACH.contains(&ids[0]));
    // With the Cluster planes removed, the AFBC buffer has no plane.
    let mut esmart_only = Capabilities {
        outputs: caps.outputs,
        ..Capabilities::default()
    };
    for p in caps
        .planes
        .iter()
        .filter(|p| ESMART_REACH.contains(&p.id.0))
    {
        esmart_only.planes.push(*p).unwrap();
    }
    let reasons = no_plane_reasons(plane_ids(&esmart_only, &[layer(AFBC, 0)]));
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
}

#[test]
fn a_pair_that_no_plane_lists_is_refused() {
    // NV12 with AFBC is on no plane: the Cluster lists have YU08 instead and
    // the Esmart planes take NV12 linear only.
    let caps = generic(full_caps());
    let reasons = no_plane_reasons(plane_ids(&caps, &[layer(NV12, 0)]));
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
}

// ---- no plane scales ----

#[test]
fn scaled_layer_is_refused_on_every_plane() {
    let caps = generic(full_caps());
    for buffer in [LIN, AFBC] {
        let up = Layer::new(
            buffer,
            Rect::new(0, 0, 64, 64),
            Rect::new(0, 0, 128, 128),
            0,
        );
        let reasons = no_plane_reasons(plane_ids(&caps, &[up]));
        assert!(reasons.contains(RejectReasons::SCALING), "{reasons:?}");
        let down = Layer::new(buffer, Rect::new(0, 0, 64, 64), Rect::new(0, 0, 32, 32), 0);
        let reasons = no_plane_reasons(plane_ids(&caps, &[down]));
        assert!(reasons.contains(RejectReasons::SCALING), "{reasons:?}");
    }
    // The same size is fine.
    let same = Layer::new(SMALL, Rect::new(0, 0, 32, 32), Rect::new(0, 0, 32, 32), 0);
    assert_eq!(plane_ids(&caps, &[same]).unwrap().len(), 1);
}

// ---- unreachable planes ----

#[test]
fn planes_39_and_69_are_never_proposed_for_output_0() {
    for caps in [generic(full_caps()), generic(leased_caps())] {
        for n in 1..=6 {
            for buffers in [[AFBC, LIN], [LIN, AFBC]] {
                let layers: Vec<Layer> = (0..n).map(|i| layer(buffers[i % 2], i)).collect();
                if let Ok(ids) = plane_ids(&caps, &layers) {
                    assert!(!ids.contains(&39) && !ids.contains(&69), "{ids:?}");
                }
            }
        }
    }
}

#[test]
fn only_planes_39_and_69_gives_no_plane_with_output_reason() {
    let caps = generic(caps_with(&[39, 69]));
    for buffer in [AFBC, LIN] {
        let reasons = no_plane_reasons(plane_ids(&caps, &[layer(buffer, 0)]));
        assert!(reasons.contains(RejectReasons::OUTPUT), "{reasons:?}");
    }
}
