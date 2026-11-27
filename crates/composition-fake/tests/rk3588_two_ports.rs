//! The observed NanoPC-T6 (RK3588 VOP2) topology with two active video ports
//! as a planner scenario on the fake backend.
//!
//! This is a MODEL, not a hardware result: nothing here ran on the board and no
//! TEST_ONLY or commit reached a kernel. The per-port reach is typed in below:
//!
//! - Two CRTCs, `video_port0` (output 0) and `video_port1` (output 1), plane
//!   ids 33, 39, 45, 51, 57, 63, 69, 75: observed 2026-11-28
//!   (`docs/observed/rk3588-topology.md`, kernel 6.18.55). That probe lists
//!   only connected outputs, so it printed mask 0b0 for plane 39 and 0b1 for
//!   the other seven.
//! - Reach on `video_port1`: read from source (mainline 6.18
//!   `vop2_create_crtcs`, `docs/rk3588-dt-source.md`, 2026-11-28): plane 33 is
//!   the primary of the first used port (`0b01`), plane 39 the primary of the
//!   second (`0b10`), the six overlays reach both (`0b11`). Matches the probe's
//!   masks on bit 0.
//! - The 4-planes-per-CRTC budget (`vp->nlayers = 8 / 2`) is read from source
//!   in the same document; it is NOT modelled by the planner, and the test
//!   below pins what the planner does today (no cap, no cross-output
//!   bookkeeping).
//!
//! Other limits as in `topology_rk3588.rs`: every plane is modelled with
//! linear XR24/AR24/NV12 only (Esmart) or XR24/AR24 (Cluster), zpos 0..=7,
//! alpha and all three blend modes, no scaling, no transform.

use composition_core::BlendMode;
use composition_core::{
    BufferId, BufferInfo, Capabilities, Engine, Error, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, Size, Zpos,
};
use composition_fake::FakeBackend;

const VP0: OutputId = OutputId(0);
const VP1: OutputId = OutputId(1);
const SCREEN: Size = Size::new(1920, 1080);
const FULL: Rect = Rect::new(0, 0, 1920, 1080);
const XR24: PixelFormat = PixelFormat::XRGB8888;
const NV12: PixelFormat = PixelFormat::NV12;

/// (plane id, kind, possible_crtcs mask over [vp0, vp1], is Esmart).
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

fn caps() -> Capabilities {
    let mut caps = Capabilities::default();
    for id in [VP0, VP1] {
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
            p.formats.push(FormatModifier::linear(NV12)).unwrap();
        }
        caps.planes.push(p).unwrap();
    }
    caps
}

fn engine(n_buffers: u32, format: PixelFormat) -> Engine<FakeBackend> {
    let mut e = Engine::new(FakeBackend::new(caps()));
    for i in 1..=n_buffers {
        e.register_buffer(
            BufferId(i),
            BufferInfo {
                size: SCREEN,
                format,
                modifier: Modifier::LINEAR,
            },
        )
        .unwrap();
        e.finish_write(BufferId(i)).unwrap();
    }
    e
}

fn layers(first: u32, n: u32) -> Vec<Layer> {
    (0..n)
        .map(|i| Layer::new(BufferId(first + i), FULL, FULL, i as i32))
        .collect()
}

/// Plane ids chosen for `layers` on `output`.
fn planes_of(e: &mut Engine<FakeBackend>, output: OutputId, layers: &[Layer]) -> Vec<u32> {
    let plan = e.validate(&Frame { output, layers }).unwrap();
    plan.assignments().iter().map(|a| a.plane.0).collect()
}

fn no_plane(
    e: &mut Engine<FakeBackend>,
    output: OutputId,
    layers: &[Layer],
) -> (usize, RejectReasons) {
    match e.validate(&Frame { output, layers }) {
        Err(Error::Plan(PlanError::NoPlane { layer, reasons })) => (layer, reasons),
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

#[test]
fn model_has_two_outputs_and_the_recorded_reach() {
    let c = caps();
    assert_eq!(c.outputs.len(), 2);
    let reach = |id: u32| c.planes.iter().find(|p| p.id.0 == id).unwrap().outputs;
    // Observed probe view (bit 0): only 39 misses video_port0.
    for (id, _, _, _) in PLANES {
        assert_eq!(reach(id) & 1 == 0, id == 39, "plane {id}");
    }
    assert_eq!(reach(33), 0b01);
    assert_eq!(reach(39), 0b10);
}

#[test]
fn plane_39_is_never_given_to_port_0() {
    // Every layer count the planner accepts on port 0 avoids plane 39.
    for n in 1..=7u32 {
        let mut e = engine(n, XR24);
        let ls = layers(1, n);
        let used = planes_of(&mut e, VP0, &ls);
        assert_eq!(used.len(), n as usize);
        assert!(!used.contains(&39), "{n} layers: {used:?}");
    }
    // Eight layers do not fit on port 0: seven reachable planes.
    let mut e = engine(8, XR24);
    let (layer, reasons) = no_plane(&mut e, VP0, &layers(1, 8));
    assert_eq!(layer, 7);
    assert!(reasons.contains(RejectReasons::IN_USE), "{reasons:?}");
}

#[test]
fn plane_39_is_the_only_primary_of_port_1_and_port_1_cannot_use_plane_33() {
    let mut e = engine(8, XR24);
    // One layer on port 1 lands on its own primary, not on 33.
    assert_eq!(planes_of(&mut e, VP1, &layers(1, 1)), [39]);
    // Eight layers on port 1: 33 is unreachable, so 7 planes at most.
    let seven = layers(1, 7);
    let used = planes_of(&mut e, VP1, &seven);
    assert!(!used.contains(&33), "{used:?}");
    assert_eq!(used.len(), 7);
    let (layer, _) = no_plane(&mut e, VP1, &layers(1, 8));
    assert_eq!(layer, 7);
}

#[test]
fn port_0_takes_its_primary_first_and_port_1_takes_39() {
    // Both ports ask for one plane: the primaries are chosen, no overlap.
    let mut e = engine(2, XR24);
    assert_eq!(planes_of(&mut e, VP0, &layers(1, 1)), [33]);
    assert_eq!(planes_of(&mut e, VP1, &layers(2, 1)), [39]);
}

#[test]
fn four_layers_per_port_reuse_the_same_overlays_when_planned_in_turn() {
    // The source budget is 4 planes per port with two ports. Planned in turn
    // on the same engine, the planner does not track the other output's
    // choice, so overlap is possible; pin the current picks.
    let mut e = engine(8, XR24);
    let a = planes_of(&mut e, VP0, &layers(1, 4));
    let b = planes_of(&mut e, VP1, &layers(5, 4));
    assert_eq!(a.len(), 4);
    assert_eq!(b.len(), 4);
    assert!(a.contains(&33) && !a.contains(&39), "{a:?}");
    assert!(b.contains(&39) && !b.contains(&33), "{b:?}");
    let shared: Vec<u32> = a.iter().copied().filter(|p| b.contains(p)).collect();
    // Pinned: the planner reuses the same overlays for both ports (no
    // cross-output bookkeeping, as in drm's shared-plane blind-spot test).
    assert_eq!(
        shared,
        a.iter().copied().filter(|p| *p != 33).collect::<Vec<_>>()
    );
}

#[test]
fn per_port_cap_of_four_is_not_enforced_by_the_planner() {
    // Source: more than 4 planes on one CRTC with two ports is -EINVAL in
    // vop2_crtc_atomic_check. The planner has no such budget today: five and
    // six layers on port 0 are planned. Change this test on purpose when a
    // profile adds the cap.
    for n in [5u32, 6, 7] {
        let mut e = engine(n, XR24);
        let used = planes_of(&mut e, VP0, &layers(1, n));
        assert_eq!(used.len(), n as usize);
    }
}

#[test]
fn present_on_both_ports_commits_each_output() {
    let mut e = engine(2, XR24);
    let l0 = layers(1, 1);
    let l1 = layers(2, 1);
    e.present(&Frame {
        output: VP0,
        layers: &l0,
    })
    .unwrap();
    e.present(&Frame {
        output: VP1,
        layers: &l1,
    })
    .unwrap();
    assert_eq!(e.backend().commit_count(), 2);
}
