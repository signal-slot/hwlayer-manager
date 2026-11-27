//! Pins the `MAX_PLANES` cut on the recorded BCM2711 and BCM2712 plane reach.
//!
//! `DrmBackend` scans every plane the kernel exposes, but `Capabilities`
//! holds at most `MAX_PLANES` (16). `keep_planes` in `src/lib.rs` makes the
//! cut: planes that reach a probed output first, then primaries, cursors and
//! overlays in that priority, then primary, overlays, cursor in the final
//! order. Both sorts are stable, so the device's order breaks ties.
//!
//! `keep_planes` is private and only runs inside the device scan, so an
//! integration test cannot call it. `cut` below mirrors it; the tests pin the
//! rule on recorded topologies, and the survivors go through the public
//! `Capabilities` types. If `src/lib.rs` changes the rule, this mirror must
//! change with it (the unit tests in `src/lib.rs` cover the real function on
//! a synthetic shape). Nothing reads docs/observed at runtime.
//!
//! Difference from the recorded observation: docs/observed/bcm2711-cm4.md
//! (2026-11-28) shows the engine keeping 6 primaries and 10 overlays, five
//! primaries of unreachable CRTCs among them, so output 0 saw 11 planes. That
//! was the cut before plan #457 fix-x09. The code now sorts reachable planes
//! first: output 0 reaches 18 planes (primary, 16 shared overlays, cursor)
//! and keeps 16, namely the primary, the cursor and 14 overlays. The two
//! overlays dropped are the last two in device order.

use composition_core::limits::MAX_PLANES;
use composition_core::{
    Capabilities, FormatModifier, OutputCaps, OutputId, PixelFormat, PlaneCaps, PlaneId, PlaneKind,
    Size, Zpos,
};
use composition_drm::DriverProfile;

/// Mirror of `keep_planes` in `src/lib.rs`, keyed on `outputs != 0` and kind.
fn cut(mut planes: Vec<PlaneCaps>) -> Vec<PlaneCaps> {
    planes.sort_by_key(|p| {
        let priority = match p.kind {
            PlaneKind::Primary => 0,
            PlaneKind::Cursor => 1,
            PlaneKind::Overlay => 2,
        };
        (p.outputs == 0, priority)
    });
    planes.truncate(MAX_PLANES);
    planes.sort_by_key(|p| {
        let order = match p.kind {
            PlaneKind::Primary => 0,
            PlaneKind::Overlay => 1,
            PlaneKind::Cursor => 2,
        };
        (p.outputs == 0, order)
    });
    planes
}

fn plane(id: u32, kind: PlaneKind, outputs: u32) -> PlaneCaps {
    let zpos = match kind {
        PlaneKind::Overlay => Zpos::Range { min: 1, max: 17 },
        _ => Zpos::Fixed(0),
    };
    let mut p = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs,
        zpos,
        ..PlaneCaps::default()
    };
    p.formats
        .push(FormatModifier::linear(PixelFormat::XRGB8888))
        .unwrap();
    p
}

fn ids(planes: &[PlaneCaps]) -> Vec<u32> {
    planes.iter().map(|p| p.id.0).collect()
}

/// Shared overlays: ids `first + 11 * k`, all with reach `mask`.
fn shared_overlays(first: u32, count: u32, mask: u32) -> Vec<PlaneCaps> {
    (0..count)
        .map(|k| plane(first + 11 * k, PlaneKind::Overlay, mask))
        .collect()
}

/// BCM2711 (CM4, reTerminal), 60 planes in the kernel's device order, one
/// probed output (DSI-1 on pixelvalve-1, output 0).
///
/// Source: docs/observed/bcm2711-cm4.md, observed 2026-11-28 ("Topology":
/// ids 48..710 step 11, plane-0..5 primaries, plane-6..21 16 shared overlays,
/// plane-22..53 32 TXP overlays, plane-54..59 cursors). Primary ids 48, 67,
/// 79, 91, 103, 115 and overlay ids 127..292 are recorded. The TXP overlay
/// ids (303..644) and the cursor ids (655..710) follow the step-11 rule
/// (inferred). Which cursor belongs to pixelvalve-1 was not read: 666 is
/// inferred. TXP overlays reach no probed output (the writeback CRTC is not
/// an output).
fn bcm2711() -> Vec<PlaneCaps> {
    let mut v = Vec::new();
    for id in [48, 67, 79, 91, 103, 115] {
        v.push(plane(id, PlaneKind::Primary, u32::from(id == 79)));
    }
    v.extend(shared_overlays(127, 16, 0b1));
    v.extend(shared_overlays(303, 32, 0));
    for id in [655, 666, 677, 688, 699, 710] {
        v.push(plane(id, PlaneKind::Cursor, u32::from(id == 666)));
    }
    assert_eq!(v.len(), 60);
    v
}

/// BCM2712 (Raspberry Pi 5), 56 planes, one probed output (HDMI-A-1 on CRTC
/// index 2, output 0).
///
/// Source: docs/observed/bcm2712-topology.md, observed 2026-11-28: primaries
/// 48, 67, 83, 95 (own CRTC each), 16 shared overlays 107..272 reaching CRTC
/// 1, 2, 3, CRTC-0-only planes (ids not listed except 382), cursors 646, 657,
/// 668 (inferred to be cursors) one per CRTC 1, 2, 3. The note counts 34
/// CRTC-0-only planes, which with 56 in total leaves 33: this fixture uses 33
/// with ids 283 + 11 * k (382 is k = 9), the id rule being inferred.
fn bcm2712() -> Vec<PlaneCaps> {
    let mut v = Vec::new();
    for id in [48, 67, 83, 95] {
        v.push(plane(id, PlaneKind::Primary, u32::from(id == 83)));
    }
    v.extend(shared_overlays(107, 16, 0b1));
    v.extend(shared_overlays(283, 33, 0));
    for id in [646, 657, 668] {
        v.push(plane(id, PlaneKind::Cursor, u32::from(id == 657)));
    }
    assert_eq!(v.len(), 56);
    v
}

/// Deterministic Fisher-Yates shuffle (xorshift), no `rand` dependency.
fn shuffled(mut v: Vec<PlaneCaps>, mut seed: u64) -> Vec<PlaneCaps> {
    for i in (1..v.len()).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        v.swap(i, (seed % (i as u64 + 1)) as usize);
    }
    v
}

/// The kept planes as `Capabilities` with one output, built through the
/// public types the way the backend does.
fn caps_of(kept: &[PlaneCaps]) -> Capabilities {
    let mut caps = Capabilities::default();
    for p in kept {
        caps.planes.push(*p).unwrap();
    }
    caps.outputs
        .push(OutputCaps {
            id: OutputId(0),
            size: Size::new(1920, 1080),
            refresh_mhz: 60_000,
        })
        .unwrap();
    DriverProfile::Generic.apply(&mut caps);
    caps
}

fn reach(planes: &[PlaneCaps]) -> usize {
    planes.iter().filter(|p| p.outputs & 1 != 0).count()
}

fn check_fixture(all: Vec<PlaneCaps>, primary: u32, cursor: u32, overlays: &[u32]) {
    assert_eq!(reach(&all), 18, "fixture must reach 18 planes on output 0");
    let kept = cut(all);
    assert_eq!(kept.len(), MAX_PLANES);
    // Every kept plane reaches output 0.
    assert!(kept.iter().all(|p| p.outputs & 1 != 0));
    // 18 reachable, 16 kept: the primary, 14 overlays, the cursor.
    let mut expected = vec![primary];
    expected.extend(&overlays[..MAX_PLANES - 2]);
    expected.push(cursor);
    assert_eq!(ids(&kept), expected);
    assert_eq!(kept[0].kind, PlaneKind::Primary);
    assert_eq!(kept[MAX_PLANES - 1].kind, PlaneKind::Cursor);
    // The public Capabilities hold exactly the kept planes.
    let caps = caps_of(&kept);
    assert_eq!(caps.planes.len(), MAX_PLANES);
    assert_eq!(caps.planes[0].id, PlaneId(primary));
    assert!(caps.planes.iter().all(|p| p.outputs & 1 != 0));
}

fn overlay_ids(first: u32) -> Vec<u32> {
    (0..16).map(|k| first + 11 * k).collect()
}

#[test]
fn bcm2711_output0_keeps_primary_cursor_and_14_overlays() {
    check_fixture(bcm2711(), 79, 666, &overlay_ids(127));
}

#[test]
fn bcm2712_output0_keeps_primary_cursor_and_14_overlays() {
    check_fixture(bcm2712(), 83, 657, &overlay_ids(107));
}

#[test]
fn unreachable_primaries_do_not_take_slots() {
    // The pre-fix-x09 observation: five foreign primaries kept, 11 planes
    // usable. The cut now drops them.
    for all in [bcm2711(), bcm2712()] {
        let kept = cut(all);
        let primaries = kept.iter().filter(|p| p.kind == PlaneKind::Primary).count();
        assert_eq!(primaries, 1);
    }
}

#[test]
fn cut_is_deterministic_for_the_same_input() {
    for all in [bcm2711(), bcm2712()] {
        assert_eq!(ids(&cut(all.clone())), ids(&cut(all)));
    }
}

/// The cut does not give the same overlay set for every input order:
/// overlays tie, so the stable sort keeps the first 14 in the order the
/// device lists them. A shuffled input keeps a different overlay set, but
/// always the same primary, cursor, count and reach. The kernel's plane order
/// is fixed per boot, so the real backend is deterministic; this pins that
/// the device order is the tie-break.
#[test]
fn shuffled_input_keeps_primary_cursor_and_count_but_follows_input_order() {
    for (all, primary, cursor) in [(bcm2711(), 79, 666), (bcm2712(), 83, 657)] {
        for seed in [1_u64, 0x9e37_79b9, 0xdead_beef, 42] {
            let input = shuffled(all.clone(), seed);
            let kept = cut(input.clone());
            assert_eq!(kept.len(), MAX_PLANES);
            assert!(kept.iter().all(|p| p.outputs & 1 != 0));
            assert_eq!(kept[0].id, PlaneId(primary));
            assert_eq!(kept[MAX_PLANES - 1].id, PlaneId(cursor));
            assert_eq!(ids(&kept), ids(&cut(input.clone())));
            // The kept overlays are the first 14 reachable ones in input order.
            let expected: Vec<u32> = input
                .iter()
                .filter(|p| p.kind == PlaneKind::Overlay && p.outputs & 1 != 0)
                .take(MAX_PLANES - 2)
                .map(|p| p.id.0)
                .collect();
            assert_eq!(ids(&kept[1..MAX_PLANES - 1]), expected);
        }
    }
}

/// Two probed outputs on BCM2712 (hypothetical: HDMI-A-1 on CRTC 2 and a
/// second display on CRTC 1; only the first is recorded). Reach masks follow
/// the recorded rule: the primary and cursor of each CRTC reach their own
/// output, the shared overlays reach both. The cut keeps planes reaching any
/// output, so every kept plane reaches at least one, and each output keeps
/// its own primary and cursor.
#[test]
fn two_outputs_keep_planes_that_reach_at_least_one() {
    let mut v = Vec::new();
    for id in [48, 67, 83, 95] {
        let mask = match id {
            83 => 0b01,
            67 => 0b10,
            _ => 0,
        };
        v.push(plane(id, PlaneKind::Primary, mask));
    }
    v.extend(shared_overlays(107, 16, 0b11));
    v.extend(shared_overlays(283, 33, 0));
    for id in [646, 657, 668] {
        let mask = match id {
            657 => 0b01,
            646 => 0b10,
            _ => 0,
        };
        v.push(plane(id, PlaneKind::Cursor, mask));
    }
    let kept = cut(v);
    assert_eq!(kept.len(), MAX_PLANES);
    assert!(kept.iter().all(|p| p.outputs != 0));
    // Both primaries, 12 overlays, both cursors.
    assert_eq!(ids(&kept[..2]), [67, 83]);
    assert_eq!(ids(&kept[MAX_PLANES - 2..]), [646, 657]);
    assert!(kept[2..MAX_PLANES - 2]
        .iter()
        .all(|p| p.kind == PlaneKind::Overlay));
}
