//! Edge cases of `DriverProfile::apply`, on hand-built capabilities.

use composition_core::limits::{MAX_PLANES, MAX_RESOURCE_GROUPS};
use composition_core::{
    Capabilities, LayerTraits, PlaneCaps, PlaneId, PlaneKind, ResourceGroup, ScaleLimits, Size,
};
use composition_drm::DriverProfile;

fn caps_of(kinds: &[PlaneKind]) -> Capabilities {
    let mut caps = Capabilities::default();
    for (i, kind) in kinds.iter().enumerate() {
        caps.planes
            .push(PlaneCaps {
                id: PlaneId(i as u32),
                kind: *kind,
                outputs: 1,
                ..PlaneCaps::default()
            })
            .unwrap();
    }
    caps
}

fn applied(kinds: &[PlaneKind]) -> Capabilities {
    let mut caps = caps_of(kinds);
    DriverProfile::Imx95Dpu.apply(&mut caps);
    caps
}

fn mask(n: usize) -> u32 {
    ((1u64 << n) - 1) as u32
}

fn check_invariants(caps: &Capabilities) {
    let all = mask(caps.planes.len());
    for g in caps.resource_groups.as_slice() {
        assert_eq!(g.planes & !all, 0, "group names a missing plane");
    }
    for p in caps.planes.as_slice() {
        if p.kind == PlaneKind::Cursor {
            assert_eq!(p.scaling, None);
            assert_eq!(p.max_source, None);
        } else {
            assert_eq!(
                p.scaling,
                Some(ScaleLimits {
                    max_upscale: 2048 << 16,
                    max_downscale: 1 << 16
                })
            );
            assert_eq!(p.max_source, Some(Size::new(8192, 8192)));
        }
    }
}

#[test]
fn generic_leaves_populated_caps_unchanged() {
    let mut caps = caps_of(&[PlaneKind::Primary, PlaneKind::Overlay, PlaneKind::Cursor]);
    let before = caps;
    DriverProfile::Generic.apply(&mut caps);
    assert_eq!(caps, before);
}

#[test]
fn zero_planes() {
    let caps = applied(&[]);
    assert!(caps.planes.is_empty());
    check_invariants(&caps);
}

#[test]
fn one_plane() {
    let caps = applied(&[PlaneKind::Primary]);
    assert_eq!(caps.resource_groups.len(), 4);
    assert!(caps.resource_groups.iter().all(|g| g.planes == 1));
    check_invariants(&caps);
}

#[test]
fn only_cursor_planes() {
    let caps = applied(&[PlaneKind::Cursor, PlaneKind::Cursor]);
    check_invariants(&caps);
}

#[test]
fn mixed_planes() {
    let caps = applied(&[
        PlaneKind::Primary,
        PlaneKind::Overlay,
        PlaneKind::Cursor,
        PlaneKind::Overlay,
    ]);
    assert!(caps.resource_groups.iter().all(|g| g.planes == 0b1111));
    check_invariants(&caps);
}

#[test]
fn maximum_plane_count() {
    // The plane limit is below 32, so the shift in `apply` cannot overflow
    // through `Capabilities`.
    const { assert!(MAX_PLANES < 32) };
    let caps = applied(&[PlaneKind::Overlay; MAX_PLANES]);
    assert_eq!(caps.planes.len(), MAX_PLANES);
    assert!(caps
        .resource_groups
        .iter()
        .all(|g| g.planes == mask(MAX_PLANES)));
    check_invariants(&caps);
}

/// Regression: a second `apply` used to push the four groups again.
#[test]
fn applying_twice_is_idempotent() {
    let mut caps = caps_of(&[PlaneKind::Primary, PlaneKind::Overlay, PlaneKind::Cursor]);
    DriverProfile::Imx95Dpu.apply(&mut caps);
    let once = caps;
    DriverProfile::Imx95Dpu.apply(&mut caps);
    assert_eq!(caps, once, "second apply changed the caps");
    DriverProfile::Imx95Dpu.apply(&mut caps);
    assert_eq!(caps, once);
}

#[test]
fn full_group_table_does_not_panic_and_keeps_existing_groups() {
    let mut caps = caps_of(&[PlaneKind::Primary, PlaneKind::Overlay]);
    let own = ResourceGroup {
        consumers: LayerTraits::YUV,
        planes: 0b10,
        capacity: 1,
    };
    for _ in 0..MAX_RESOURCE_GROUPS {
        caps.resource_groups.push(own).unwrap();
    }
    DriverProfile::Imx95Dpu.apply(&mut caps);
    assert_eq!(caps.resource_groups.len(), MAX_RESOURCE_GROUPS);
    assert!(caps.resource_groups.iter().all(|g| *g == own));
}
