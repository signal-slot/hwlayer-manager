//! Host regression tests for the plane selection of the DRM backend, on
//! hand-built plane lists and without a DRM device.
//!
//! The backend enumerates every plane of the card, including those whose
//! possible-CRTC mask covers no probed output. The `MAX_PLANES` cap must be
//! applied only after the planes that reach a probed output have been ranked
//! first: the BCM2712 HVS (Raspberry Pi 5) exposes 56 planes for one
//! connected output, and cutting the raw list left the output with planes of
//! unconnected CRTCs while dropping usable overlays of the connected one.
//! See `docs/observed/bcm2712.md`, finding 1.

use composition_core::limits::MAX_PLANES;
use composition_core::PlaneKind;
use composition_drm::keep_planes;

/// A plane: id, whether its possible CRTCs cover the probed output, kind.
type Plane = (u32, bool, PlaneKind);

fn select(planes: &[Plane]) -> Vec<Plane> {
    keep_planes(planes.to_vec(), |plane| (plane.1, plane.2))
}

#[test]
fn planes_of_the_crtc_survive_when_the_device_lists_them_last() {
    // The harsh shape: the device enumerates 20 planes of unconnected CRTCs
    // first and the connected CRTC's planes last. The pre-fix selection
    // (sort by kind over the whole list, then cut at `MAX_PLANES`) kept 16
    // of the first group and lost 12 of the connected CRTC's planes.
    let mut planes: Vec<Plane> = (0..20).map(|id| (id, false, PlaneKind::Overlay)).collect();
    let mut crtc: Vec<Plane> = vec![(100, true, PlaneKind::Primary)];
    crtc.extend((101..113).map(|id| (id, true, PlaneKind::Overlay)));
    crtc.push((113, true, PlaneKind::Cursor));
    assert_eq!(crtc.len(), 14);
    assert!(crtc.len() < MAX_PLANES, "the CRTC's planes must all fit");
    planes.extend(crtc.iter().copied());
    assert!(planes.len() > MAX_PLANES);

    let kept = select(&planes);
    // Exact expectation: every plane of the connected CRTC (primary, the
    // overlays in device order, the cursor), then two unreachable planes
    // filling the free slots at the back (their caps name no output, so
    // the planner never assigns them).
    let mut expected: Vec<u32> = vec![100];
    expected.extend(101..113);
    expected.push(113);
    expected.extend([0, 1]);
    assert_eq!(expected.len(), MAX_PLANES);
    let kept_ids: Vec<u32> = kept.iter().map(|p| p.0).collect();
    assert_eq!(kept_ids, expected);
    // None of the connected CRTC's planes was cut off, and only the free
    // slots behind the last reachable one are filled with unreachable ones.
    assert_eq!(kept.iter().filter(|p| p.1).count(), crtc.len());
}

#[test]
fn many_plane_device_keeps_every_slot_of_the_probed_output() {
    // Shaped like the BCM2712 HVS: 4 primaries (only CRTC 2's reaches the
    // connected HDMI-A-1), 16 overlays shared by CRTCs 1..3, a run of
    // CRTC-0-only overlays, and one extra plane per CRTC 1..3 (cursor). The
    // connected output can use 18 planes, more than `MAX_PLANES`, so every
    // kept slot must be one it can actually use.
    let mut planes: Vec<Plane> = vec![
        (48, false, PlaneKind::Primary),
        (67, false, PlaneKind::Primary),
        (83, true, PlaneKind::Primary),
        (95, false, PlaneKind::Primary),
    ];
    let shared: Vec<u32> = (0..16).map(|n| 107 + 11 * n).collect();
    planes.extend(shared.iter().map(|id| (*id, true, PlaneKind::Overlay)));
    planes.extend((300..333).map(|id| (id, false, PlaneKind::Overlay)));
    planes.push((646, false, PlaneKind::Cursor));
    planes.push((657, true, PlaneKind::Cursor));
    planes.push((668, false, PlaneKind::Cursor));
    assert_eq!(planes.len(), 56);
    let reachable = planes.iter().filter(|p| p.1).count();
    assert!(reachable > MAX_PLANES, "the shape must overfill the cap");

    let kept = select(&planes);
    assert_eq!(kept.len(), MAX_PLANES);
    assert!(
        kept.iter().all(|p| p.1),
        "a plane of an unconnected CRTC took a usable slot"
    );
    // The reachable primary heads the list, the reachable cursor is kept
    // ahead of the overlays' tail, and only the tail of the shared overlays
    // is cut, in device order.
    assert_eq!(kept[0].0, 83);
    assert_eq!(kept[MAX_PLANES - 1].0, 657);
    let kept_overlays: Vec<u32> = kept[1..MAX_PLANES - 1].iter().map(|p| p.0).collect();
    assert_eq!(kept_overlays, shared[..MAX_PLANES - 2]);
    // One primary and one cursor kept leaves 14 of the 16 shared overlays.
    assert_eq!(kept_overlays.len(), MAX_PLANES - 2);
}

#[test]
fn selection_is_deterministic() {
    let mut planes: Vec<Plane> = (0..24)
        .map(|id| (id, id % 3 == 0, PlaneKind::Overlay))
        .collect();
    planes.push((900, true, PlaneKind::Primary));
    planes.push((901, false, PlaneKind::Primary));
    planes.push((902, true, PlaneKind::Cursor));
    let once = select(&planes);
    assert!(once.len() <= MAX_PLANES);
    for _ in 0..3 {
        assert_eq!(select(&planes), once);
    }
}
