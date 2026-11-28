//! Profile constraints added because a board's `TEST_ONLY` runs showed a
//! limit that KMS properties do not carry.
//!
//! RK3588 VOP2 (`DriverProfile::Rk3588Vop2`), at most 4 planes per CRTC:
//! - `tests/data/rk3588/sweep-combos-2026-11-27.txt`, observed 2026-11-27
//!   (NanoPC-T6, kernel 6.18.55-current-rockchip64, CRTC 81): every 1- to
//!   4-plane set of the 7 reachable planes passed `TEST_ONLY` (1/1, 6/6,
//!   15/15, 20/20), every 5-, 6- and 7-plane set returned EINVAL (0/15, 0/6,
//!   0/1). Full-screen 1920x1080 layers at 1:1.
//! - `tests/data/rk3588/sweep-combos-scaled-2026-11-27.txt`, observed
//!   2026-11-27: the same boundary with every layer scaled (v 2x, h 2x,
//!   h 0.5).
//! - Source: `vp->nlayers = win_size / nvps` refused with `-EINVAL` above it
//!   (`rockchip_drm_vop2.c` 2483 and 1816-1835, `docs/rk3588-dt-source.md`).
//!
//! The fixture is the recorded probe summary
//! (`tests/data/rk3588/probe-raw-2026-11-27.txt`, observed 2026-11-27),
//! loaded through `common::recorded`. These tests prove planner behaviour on
//! that recorded description, not hardware behaviour.
//!
//! BCM2711, BCM2712 and STM32MP257 have no entry here: their recorded
//! `TEST_ONLY` refusals are already modelled by `Vc4Hvs5`, `Vc4Hvs6` and
//! `Stm32Ltdc` (see their tests).

mod common;

use common::recorded;
use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BufferId, BufferInfo, Capabilities, Frame, Layer, LayerTraits, Modifier, PixelFormat,
    PlanError, Rect, RejectReasons, ResourceGroup, Size,
};
use composition_drm::DriverProfile;

// Recorded on the NanoPC-T6, 2026-11-27 (see the module comment).
const PROBE: &str = include_str!("data/rk3588/probe-raw-2026-11-27.txt");

const SCREEN: Rect = Rect::new(0, 0, 1920, 1080);

/// First AFBC modifier of the Cluster list (observed 2026-11-27).
const AFBC: Modifier = Modifier(0x0800_0000_0000_0001);

const AFBC_XR24: BufferId = BufferId(1);
const LINEAR_XR24: BufferId = BufferId(2);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let modifier = match buffer {
            AFBC_XR24 => AFBC,
            LINEAR_XR24 => Modifier::LINEAR,
            _ => return None,
        };
        Some(BufferInfo {
            size: Size::new(1920, 1080),
            format: PixelFormat::XRGB8888,
            modifier,
        })
    }
}

fn recorded_caps() -> Capabilities {
    recorded::load(PROBE).expect("probe dump loads").caps
}

fn rk3588() -> Capabilities {
    let mut caps = recorded_caps();
    DriverProfile::Rk3588Vop2.apply(&mut caps);
    caps
}

/// `k` layers that the 7 reachable planes can hold: AFBC for the 3 Cluster
/// planes first, linear for the 4 Esmart planes after, each `src` onto `dst`.
fn layers(k: usize, src: Rect, dst: Rect) -> Vec<Layer> {
    (0..k)
        .map(|z| {
            let buffer = if z < 3 { AFBC_XR24 } else { LINEAR_XR24 };
            Layer::new(buffer, src, dst, z as i32)
        })
        .collect()
}

/// `k` full-screen layers at 1:1.
fn unscaled(k: usize) -> Vec<Layer> {
    layers(k, SCREEN, SCREEN)
}

fn plan_on(caps: &Capabilities, layers: &[Layer]) -> Result<usize, PlanError> {
    // The loader keeps the CRTC id as the output id; take it from the caps.
    let frame = Frame {
        output: caps.outputs.as_slice()[0].id,
        layers,
    };
    plan(&frame, caps, &Buffers).map(|p| p.assignments().len())
}

fn refused_for_resource(result: Result<usize, PlanError>) -> bool {
    matches!(result, Err(PlanError::NoPlane { reasons, .. }) if reasons.contains(RejectReasons::RESOURCE))
}

#[test]
fn rk3588_adds_one_four_layer_budget_over_every_plane() {
    let caps = rk3588();
    assert!(recorded_caps().resource_groups.is_empty());
    assert_eq!(
        caps.resource_groups.as_slice(),
        &[ResourceGroup {
            consumers: LayerTraits::ANY,
            planes: (1 << caps.planes.len()) - 1,
            capacity: 4,
        }]
    );
}

#[test]
fn rk3588_budget_matches_the_unscaled_sweep() {
    // Observed 2026-11-27: k = 1..=4 accepted, k = 5..=7 EINVAL.
    let caps = rk3588();
    for k in 1..=4 {
        assert_eq!(plan_on(&caps, &unscaled(k)), Ok(k), "k={k}");
    }
    for k in 5..=7 {
        assert!(refused_for_resource(plan_on(&caps, &unscaled(k))), "k={k}");
    }
}

#[test]
fn rk3588_budget_matches_the_scaled_sweeps() {
    // Observed 2026-11-27: the same boundary at v 2x, h 2x and h 0.5.
    let caps = rk3588();
    let runs = [
        (Rect::new(0, 0, 1920, 540), SCREEN),
        (Rect::new(0, 0, 960, 1080), SCREEN),
        (SCREEN, Rect::new(0, 0, 960, 1080)),
    ];
    for (src, dst) in runs {
        assert_eq!(plan_on(&caps, &layers(4, src, dst)), Ok(4), "{src:?}");
        assert!(
            refused_for_resource(plan_on(&caps, &layers(5, src, dst))),
            "{src:?}"
        );
    }
}

#[test]
fn generic_lets_through_what_the_kernel_refused() {
    // Without the profile the planner places all 7 layers; the board refused
    // every set above 4 in TEST_ONLY (observed 2026-11-27).
    assert_eq!(plan_on(&recorded_caps(), &unscaled(7)), Ok(7));
}

#[test]
fn rk3588_budget_is_not_duplicated() {
    let once = rk3588();
    let mut twice = once;
    DriverProfile::Rk3588Vop2.apply(&mut twice);
    assert_eq!(twice, once);
}

#[test]
fn rk3588_budget_on_an_empty_plane_list_names_no_plane() {
    let mut caps = Capabilities::default();
    DriverProfile::Rk3588Vop2.apply(&mut caps);
    assert!(caps.resource_groups.iter().all(|g| g.planes == 0));
}
