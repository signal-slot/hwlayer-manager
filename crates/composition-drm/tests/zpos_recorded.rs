//! The zpos values `DriverProfile::Generic` writes on the recorded NanoPC-T6
//! (RK3588 VOP2) and Raspberry Pi 5 (BCM2712 HVS6) topologies, for frames of
//! 2, 4 and 8 layers, and that the layer order is kept.
//!
//! Baseline tests: they pin what the planner does today, including results
//! that look wrong for a VOP2 or an HVS. A profile item may change them.
//!
//! Fixture data is copied inline; nothing reads `docs/`.
//! - RK3588: `tests/data/rk3588/topology-2026-11-28.md`, observed 2026-11-28 (kernel
//!   6.18.55-current-rockchip64), and the plan #457 scen-t6 handoff, observed
//!   2026-11-27 (same kernel): 8 planes, zpos 0..=7 on every plane, plane
//!   alpha, blend [none, premultiplied, coverage], scaling not claimed, AFBC
//!   only on Cluster planes, planes 39 and 69 cannot reach video_port0.
//!   The probe's Cluster lists are the AFBC pairs only (no linear pair).
//! - BCM2712: `docs/platform-capabilities.md`, observed 2026-11-28 (kernel
//!   6.18.50+rpt-rpi-2712): primary 83 with zpos 0, shared overlays with zpos
//!   1..=17 mutable, 16 planes kept after the `MAX_PLANES` cut (plan #457
//!   fix-x09), linear formats only after the `MAX_FORMATS` cut.

use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect, Size, Zpos,
};
use composition_drm::DriverProfile;

const OUT: OutputId = OutputId(0);
const SIZE: Size = Size::new(1920, 1080);

/// Arm AFBC modifier as the probe printed it (observed 2026-11-28).
const AFBC: Modifier = Modifier(0x0800_0000_0000_0001);

const LINEAR_BUF: BufferId = BufferId(1);
const AFBC_BUF: BufferId = BufferId(2);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let modifier = match buffer {
            LINEAR_BUF => Modifier::LINEAR,
            AFBC_BUF => AFBC,
            _ => return None,
        };
        Some(BufferInfo {
            size: SIZE,
            format: PixelFormat::XRGB8888,
            modifier,
        })
    }
}

fn base_caps() -> Capabilities {
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

fn plane(id: u32, kind: PlaneKind, outputs: u32, zpos: Zpos, modifier: Modifier) -> PlaneCaps {
    let mut p = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs,
        plane_alpha: true,
        blend_modes: BlendMode::Opaque.mask()
            | BlendMode::Premultiplied.mask()
            | BlendMode::Coverage.mask(),
        zpos,
        ..PlaneCaps::default()
    };
    for format in [PixelFormat::XRGB8888, PixelFormat::ARGB8888] {
        p.formats.push(FormatModifier { format, modifier }).unwrap();
    }
    p
}

/// NanoPC-T6: (id, kind, outputs on output 0, Cluster?). Cluster planes list
/// AFBC pairs only, Esmart planes linear only.
const RK_PLANES: [(u32, PlaneKind, u32, bool); 8] = [
    (33, PlaneKind::Primary, 0b1, true),
    (39, PlaneKind::Primary, 0b0, true),
    (45, PlaneKind::Overlay, 0b1, true),
    (51, PlaneKind::Overlay, 0b1, true),
    (57, PlaneKind::Overlay, 0b1, false),
    (63, PlaneKind::Overlay, 0b1, false),
    (69, PlaneKind::Overlay, 0b0, false),
    (75, PlaneKind::Overlay, 0b1, false),
];

fn rk_caps() -> Capabilities {
    let mut caps = base_caps();
    for (id, kind, outputs, cluster) in RK_PLANES {
        let modifier = if cluster { AFBC } else { Modifier::LINEAR };
        let zpos = Zpos::Range { min: 0, max: 7 };
        caps.planes
            .push(plane(id, kind, outputs, zpos, modifier))
            .unwrap();
    }
    caps
}

/// BCM2712: primary 83 (zpos fixed 0) and 15 shared overlays (zpos 1..=17).
fn pi5_caps() -> Capabilities {
    let mut caps = base_caps();
    caps.planes
        .push(plane(
            83,
            PlaneKind::Primary,
            1,
            Zpos::Fixed(0),
            Modifier::LINEAR,
        ))
        .unwrap();
    for i in 0..15 {
        let zpos = Zpos::Range { min: 1, max: 17 };
        caps.planes
            .push(plane(
                107 + 11 * i,
                PlaneKind::Overlay,
                1,
                zpos,
                Modifier::LINEAR,
            ))
            .unwrap();
    }
    caps
}

fn layers(buffers: &[BufferId]) -> Vec<Layer> {
    buffers
        .iter()
        .enumerate()
        .map(|(i, &b)| {
            let r = Rect::new(i as i32 * 40, i as i32 * 40, 32, 32);
            Layer::new(b, r, r, i as i32)
        })
        .collect()
}

type Placed = Vec<(usize, u32, i32)>;

/// (layer, plane, zpos) in the planner's assignment order.
fn place(caps: &Capabilities, layers: &[Layer]) -> Result<Placed, PlanError> {
    let frame = Frame {
        output: OUT,
        layers,
    };
    plan(&frame, caps, &Buffers).map(|p| {
        p.assignments()
            .iter()
            .map(|a| (a.layer, a.plane.0, a.zpos))
            .collect()
    })
}

fn zpos_by_layer(placed: &Placed) -> Vec<i32> {
    let mut v = placed.clone();
    v.sort_by_key(|a| a.0);
    v.iter().map(|a| a.2).collect()
}

/// The layer order is kept: by layer index, zpos rises strictly.
fn assert_order_kept(placed: &Placed) {
    let z = zpos_by_layer(placed);
    assert!(z.windows(2).all(|w| w[0] < w[1]), "layer order lost: {z:?}");
}

fn generic(mut caps: Capabilities) -> Capabilities {
    DriverProfile::Generic.apply(&mut caps);
    caps
}

// ---- BCM2712 ----

#[test]
fn pi5_two_layers_use_zpos_0_and_1() {
    let placed = place(&generic(pi5_caps()), &layers(&[LINEAR_BUF; 2])).unwrap();
    assert_order_kept(&placed);
    assert_eq!(zpos_by_layer(&placed), vec![0, 1]);
    assert_eq!(placed[0].1, 83, "bottom layer on the primary");
}

#[test]
fn pi5_four_and_eight_layers_use_consecutive_zpos_from_0() {
    for n in [4usize, 8] {
        let placed = place(&generic(pi5_caps()), &layers(&vec![LINEAR_BUF; n])).unwrap();
        assert_order_kept(&placed);
        assert_eq!(
            zpos_by_layer(&placed),
            (0..n as i32).collect::<Vec<_>>(),
            "n={n}"
        );
        assert_eq!(placed[0].1, 83, "n={n}");
    }
}

// ---- RK3588 ----

#[test]
fn rk_linear_layers_only_reach_the_three_reachable_esmart_planes() {
    // Cluster planes list AFBC only, so linear layers can use only Esmart
    // planes 57, 63, 75 (69 cannot reach the output). That looks wrong for a
    // VOP2, where Cluster windows also scan out linear buffers (inferred, not
    // measured); pinned as recorded. A profile item may change it.
    let caps = generic(rk_caps());
    let placed = place(&caps, &layers(&[LINEAR_BUF; 2])).unwrap();
    assert_order_kept(&placed);
    assert!(
        placed.iter().all(|a| [57, 63, 75].contains(&a.1)),
        "{placed:?}"
    );

    assert!(place(&caps, &layers(&[LINEAR_BUF; 3])).is_ok());
    for n in [4usize, 8] {
        assert!(
            matches!(
                place(&caps, &layers(&vec![LINEAR_BUF; n])),
                Err(PlanError::NoPlane { .. })
            ),
            "n={n}"
        );
    }
}

#[test]
fn rk_mixed_afbc_and_linear_frames_keep_order() {
    let caps = generic(rk_caps());
    for buffers in [
        vec![AFBC_BUF, LINEAR_BUF],
        vec![AFBC_BUF, AFBC_BUF, LINEAR_BUF, LINEAR_BUF],
        vec![
            AFBC_BUF, AFBC_BUF, AFBC_BUF, LINEAR_BUF, LINEAR_BUF, LINEAR_BUF,
        ],
    ] {
        let placed = place(&caps, &layers(&buffers)).unwrap();
        assert_order_kept(&placed);
        assert!(placed.iter().all(|a| a.1 != 39 && a.1 != 69));
        assert!(zpos_by_layer(&placed).iter().all(|z| (0..=7).contains(z)));
        let mut ids: Vec<u32> = placed.iter().map(|a| a.1).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), buffers.len(), "a plane was used twice");
    }
}

#[test]
fn rk_eight_layers_do_not_fit_because_two_planes_cannot_reach_the_output() {
    // 8 planes, but 39 and 69 cannot reach video_port0: at most 6 layers.
    let caps = generic(rk_caps());
    let eight = [
        AFBC_BUF, AFBC_BUF, AFBC_BUF, AFBC_BUF, LINEAR_BUF, LINEAR_BUF, LINEAR_BUF, LINEAR_BUF,
    ];
    assert!(matches!(
        place(&caps, &layers(&eight)),
        Err(PlanError::NoPlane { .. })
    ));
}
