//! `DriverProfile::Generic` and modifiers on the recorded RK3588 (NanoPC-T6)
//! Cluster (AFBC) and Esmart (linear) planes.
//!
//! Baseline tests: they pin which plane the generic planner picks for linear
//! frames, AFBC-modified frames and a frame that needs a Cluster plane when the
//! Cluster planes are taken, so the later `Rk3588Vop2` tests show the
//! difference. Blind spots of the current planner are pinned as they are and
//! marked `BLIND SPOT`.
//!
//! Fixture data, copied here so that nothing reads `docs/observed`:
//! - `docs/observed/rk3588-topology.md` and
//!   `crates/composition-drm/tests/data/rk3588/probe-raw-2026-11-27.txt`,
//!   observed 2026-11-27/28 (kernel 6.18.55-current-rockchip64): plane kinds,
//!   outputs masks, zpos 0..=7. Cluster planes (33, 39, 45, 51) list 126
//!   IN_FORMATS pairs: 14 formats (XR30 XB30 XR24 AR24 XB24 AB24 RG24 BG24 RG16
//!   BG16 YU08 YU10 YUYV Y210) times 9 Arm AFBC modifiers (0x...01, 11, 41, 51,
//!   71, 81, 91, c1, d1) and no linear pair. Esmart planes (57, 63, 69, 75)
//!   list 21 linear pairs. `Capabilities` holds 32 pairs per plane, so a Cluster
//!   plane is cut to the first 32: 14 pairs with 0x...01, 14 with 0x...41 and 4
//!   with 0x...11 (XR30 XB30 XR24 AR24).
//! - Plan #457 scen-t6 handoff, observed 2026-11-27: no plane scales, planes 39
//!   and 69 cannot reach video_port0.
//!
//! The recorded debugfs state shows fbcon scanning out linear XR24 on
//! `Cluster0-win0` (observed 2026-11-28) although the IN_FORMATS list holds no
//! linear pair for it; the fixture follows IN_FORMATS.

use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, Size, Zpos,
};
use composition_drm::DriverProfile;

const OUT: OutputId = OutputId(0);

const AFBC_01: Modifier = Modifier(0x0800_0000_0000_0001);
const AFBC_41: Modifier = Modifier(0x0800_0000_0000_0041);
const AFBC_11: Modifier = Modifier(0x0800_0000_0000_0011);
/// Listed by the Cluster planes (observed) but beyond the 32-pair cut.
const AFBC_81: Modifier = Modifier(0x0800_0000_0000_0081);

const CLUSTER_FORMATS: [&[u8; 4]; 14] = [
    b"XR30", b"XB30", b"XR24", b"AR24", b"XB24", b"AB24", b"RG24", b"BG24", b"RG16", b"BG16",
    b"YU08", b"YU10", b"YUYV", b"Y210",
];

const ESMART_FORMATS: [&[u8; 4]; 21] = [
    b"XR24", b"AR24", b"XB24", b"AB24", b"RG24", b"BG24", b"RG16", b"BG16", b"NV12", b"NV21",
    b"NV16", b"NV61", b"NV20", b"NV24", b"NV42", b"NV30", b"NV15", b"YVYU", b"VYUY", b"YUYV",
    b"UYVY",
];

/// The first 32 pairs of a Cluster plane: 14 + 14 + 4.
fn cluster_pairs() -> Vec<FormatModifier> {
    let mut pairs = Vec::new();
    for (modifier, count) in [(AFBC_01, 14), (AFBC_41, 14), (AFBC_11, 4)] {
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

/// (plane id, kind, is Cluster, outputs mask on output 0)
const PLANES: [(u32, PlaneKind, bool, u32); 8] = [
    (33, PlaneKind::Primary, true, 0b1),
    (39, PlaneKind::Primary, true, 0b0),
    (45, PlaneKind::Overlay, true, 0b1),
    (51, PlaneKind::Overlay, true, 0b1),
    (57, PlaneKind::Overlay, false, 0b1),
    (63, PlaneKind::Overlay, false, 0b1),
    (69, PlaneKind::Overlay, false, 0b0),
    (75, PlaneKind::Overlay, false, 0b1),
];

const CLUSTER_REACH: [u32; 3] = [33, 45, 51];
const ESMART_REACH: [u32; 3] = [57, 63, 75];

fn caps() -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(1920, 1080),
            refresh_mhz: 60_000,
        })
        .unwrap();
    for &(id, kind, cluster, outputs) in PLANES.iter() {
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
        let pairs = if cluster {
            cluster_pairs()
        } else {
            esmart_pairs()
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
const AFBC_B: BufferId = BufferId(3);
const AFBC_C: BufferId = BufferId(4);
const AFBC_BEYOND_MODIFIER: BufferId = BufferId(5);
const AFBC_BEYOND_FORMAT: BufferId = BufferId(6);
const AFBC_ARGB: BufferId = BufferId(7);
const LIN_NV12: BufferId = BufferId(8);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let (format, modifier) = match buffer {
            LIN => (PixelFormat::XRGB8888, Modifier::LINEAR),
            AFBC => (PixelFormat::XRGB8888, AFBC_01),
            AFBC_B => (PixelFormat::XRGB8888, AFBC_41),
            AFBC_C => (PixelFormat::XRGB8888, AFBC_11),
            AFBC_BEYOND_MODIFIER => (PixelFormat::XRGB8888, AFBC_81),
            // XB24 with 0x...11 is in the plane's list but past the cut.
            AFBC_BEYOND_FORMAT => (PixelFormat::fourcc(b"XB24"), AFBC_11),
            AFBC_ARGB => (PixelFormat::ARGB8888, AFBC_01),
            LIN_NV12 => (PixelFormat::NV12, Modifier::LINEAR),
            _ => return None,
        };
        Some(BufferInfo {
            size: Size::new(64, 64),
            format,
            modifier,
        })
    }
}

fn layer(buffer: BufferId, n: usize) -> Layer {
    Layer::new(
        buffer,
        Rect::new(0, 0, 64, 64),
        Rect::new((n as i32) * 100, 0, 64, 64),
        n as i32,
    )
}

fn layers(buffers: &[BufferId]) -> Vec<Layer> {
    buffers
        .iter()
        .enumerate()
        .map(|(n, &b)| layer(b, n))
        .collect()
}

/// Plane ids per layer, in layer order.
fn placement(caps: &Capabilities, layers: &[Layer]) -> Result<Vec<u32>, PlanError> {
    let frame = Frame {
        output: OUT,
        layers,
    };
    let p = plan(&frame, caps, &Buffers)?;
    let mut by_layer = vec![0; layers.len()];
    for a in p.assignments() {
        by_layer[a.layer] = a.plane.0;
    }
    Ok(by_layer)
}

fn refused(r: Result<Vec<u32>, PlanError>) -> RejectReasons {
    match r {
        Err(PlanError::NoPlane { reasons, .. }) => reasons,
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

#[test]
fn fixture_is_cut_to_32_pairs_on_cluster_and_has_no_linear_there() {
    let caps = caps();
    for p in caps.planes.iter() {
        let cluster = matches!(p.id.0, 33 | 39 | 45 | 51);
        let linear = p.formats.iter().filter(|f| f.modifier == Modifier::LINEAR);
        assert_eq!(
            linear.count(),
            if cluster { 0 } else { 21 },
            "plane {}",
            p.id.0
        );
        assert_eq!(p.formats.len(), if cluster { 32 } else { 21 });
    }
}

// ---- linear frames ----

#[test]
fn linear_frames_use_esmart_planes_only() {
    let caps = caps();
    for n in 1..=3 {
        let ids = placement(&caps, &layers(&vec![LIN; n])).unwrap();
        assert!(ids.iter().all(|id| ESMART_REACH.contains(id)), "{ids:?}");
    }
    // BLIND SPOT / current behaviour: the fourth linear layer has no plane
    // although three reachable Cluster planes are free, because their lists
    // hold no linear pair. fbcon scans out linear on Cluster0-win0 (observed
    // 2026-11-28), so a profile that knows Cluster takes linear could place it.
    let reasons = refused(placement(&caps, &layers(&[LIN; 4])));
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
}

#[test]
fn linear_yuv_is_placed_on_esmart() {
    let caps = caps();
    let ids = placement(&caps, &layers(&[LIN_NV12])).unwrap();
    assert!(ESMART_REACH.contains(&ids[0]), "{ids:?}");
}

// ---- AFBC-modified frames ----

#[test]
fn afbc_frames_use_cluster_planes_only() {
    let caps = caps();
    for buffer in [AFBC, AFBC_B, AFBC_C, AFBC_ARGB] {
        let ids = placement(&caps, &layers(&[buffer])).unwrap();
        assert!(CLUSTER_REACH.contains(&ids[0]), "{buffer:?} {ids:?}");
    }
}

#[test]
fn a_mixed_frame_puts_each_buffer_on_its_class() {
    let caps = caps();
    let ids = placement(&caps, &layers(&[AFBC, LIN, AFBC_B, LIN])).unwrap();
    assert!(CLUSTER_REACH.contains(&ids[0]) && CLUSTER_REACH.contains(&ids[2]));
    assert!(ESMART_REACH.contains(&ids[1]) && ESMART_REACH.contains(&ids[3]));
}

#[test]
fn afbc_modifier_choice_is_the_buffers_own() {
    // The planner never picks a modifier: it only checks that the buffer's
    // pair is listed. Every listed variant is accepted on every Cluster plane.
    let caps = caps();
    let mut ids = placement(&caps, &layers(&[AFBC, AFBC_B, AFBC_C])).unwrap();
    ids.sort_unstable();
    assert_eq!(ids, CLUSTER_REACH);
}

#[test]
fn blind_spot_modifiers_and_formats_past_the_cut_are_refused() {
    // BLIND SPOT: the plane lists AFBC 0x...81 and XB24 with 0x...11 (observed
    // 2026-11-27, 126 pairs) but the 32-pair cut drops them, so the planner
    // refuses buffers the hardware accepts. An Rk3588Vop2 profile needs a
    // class rule (any AFBC modifier on Cluster) instead of the cut list.
    let caps = caps();
    for buffer in [AFBC_BEYOND_MODIFIER, AFBC_BEYOND_FORMAT] {
        let reasons = refused(placement(&caps, &layers(&[buffer])));
        assert!(
            reasons.contains(RejectReasons::FORMAT),
            "{buffer:?} {reasons:?}"
        );
    }
}

// ---- AFBC needs a Cluster plane while they are taken ----

#[test]
fn afbc_frame_with_the_cluster_planes_taken_is_refused() {
    let caps = caps();
    // Three reachable Cluster planes take three AFBC layers.
    let ids = placement(&caps, &layers(&[AFBC; 3])).unwrap();
    assert!(ids.iter().all(|id| CLUSTER_REACH.contains(id)), "{ids:?}");
    // The fourth AFBC layer has no plane even though all three reachable
    // Esmart planes are free: they list no AFBC pair. The planner does not
    // fall back to a linear copy; that is the caller's job.
    let reasons = refused(placement(&caps, &layers(&[AFBC; 4])));
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
    // Linear layers use Esmart and never take a Cluster plane, so the
    // Cluster budget stays three.
    let reasons = refused(placement(&caps, &layers(&[LIN, AFBC, AFBC, AFBC, AFBC])));
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
    let ids = placement(&caps, &layers(&[LIN, AFBC, AFBC, AFBC, LIN, LIN])).unwrap();
    assert_eq!(ids.len(), 6);
}

#[test]
fn six_mixed_layers_fill_all_reachable_planes_in_either_order() {
    // The fixture gives linear and AFBC layers disjoint plane sets, so the
    // planner has no class choice to make and the order does not matter. A
    // profile that lets linear use Cluster planes must keep AFBC layers first.
    let caps = caps();
    for order in [
        [AFBC, AFBC, AFBC, LIN, LIN, LIN],
        [LIN, LIN, LIN, AFBC, AFBC, AFBC],
    ] {
        let mut ids = placement(&caps, &layers(&order)).unwrap();
        ids.sort_unstable();
        assert_eq!(ids, [33, 45, 51, 57, 63, 75]);
    }
}
