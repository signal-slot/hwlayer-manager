//! Modeset and recovery scenarios on the recorded NanoPC-T6 (RK3588 VOP2)
//! two-port topology, on the fake backend.
//!
//! FAKE-BACKEND RESULTS ON A MODEL. Nothing here ran on a board and no
//! hardware claim is made. The topology is the one `rk3588_two_ports.rs`
//! types in (planes 33 and 39 Primary, six Overlay planes, outputs 0 =
//! `video_port0` and 1 = `video_port1`; reach: 33 on port 0 only, 39 on port 1
//! only, the overlays on both; Esmart planes 57..75 add NV12; linear XR24 and
//! AR24; zpos 0..=7; no scaling, no transform). The tests show only how the
//! shared planner and engine react on that description to a mode change, a
//! commit failure, an output loss and a plane moving between ports.
//!
//! Modeled stand-ins: a mode change is `set_capabilities` with another output
//! size (the fake has no modeset); a lost output is `set_capabilities`
//! without it, a lost device is `lose_device` and `restore`.
//!
//! The helpers repeat those of `rk3588_two_ports.rs`, which this file does
//! not change.

#![forbid(unsafe_code)]

use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, Capabilities, DisplayEvent, Engine, Error,
    FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat, PlanError,
    PlaneCaps, PlaneId, PlaneKind, Rect, RejectReasons, Size, SubmissionId, Zpos,
};
use composition_fake::{FakeBackend, PlaneState};

const VP0: OutputId = OutputId(0);
const VP1: OutputId = OutputId(1);
const FULL_HD: Size = Size::new(1920, 1080);
const HD: Size = Size::new(1280, 720);
const XR24: PixelFormat = PixelFormat::XRGB8888;
const AR24: PixelFormat = PixelFormat::ARGB8888;
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

/// The recorded planes with the given outputs.
fn caps_with(outputs: &[(OutputId, Size)]) -> Capabilities {
    let mut caps = Capabilities::default();
    for (id, size) in outputs {
        caps.outputs
            .push(OutputCaps {
                id: *id,
                size: *size,
                refresh_mhz: 60_000,
            })
            .unwrap();
    }
    for (id, kind, reach, esmart) in PLANES {
        let mut p = PlaneCaps {
            id: PlaneId(id),
            kind,
            outputs: reach,
            zpos: Zpos::Range { min: 0, max: 7 },
            plane_alpha: true,
            blend_modes: BlendMode::Opaque.mask()
                | BlendMode::Premultiplied.mask()
                | BlendMode::Coverage.mask(),
            ..PlaneCaps::default()
        };
        p.formats.push(FormatModifier::linear(XR24)).unwrap();
        p.formats.push(FormatModifier::linear(AR24)).unwrap();
        if esmart {
            p.formats.push(FormatModifier::linear(NV12)).unwrap();
        }
        caps.planes.push(p).unwrap();
    }
    caps
}

fn two_ports() -> Capabilities {
    caps_with(&[(VP0, FULL_HD), (VP1, FULL_HD)])
}

type Eng = Engine<FakeBackend>;

fn engine(caps: Capabilities) -> Eng {
    Engine::new(FakeBackend::new(caps))
}

fn register(e: &mut Eng, id: u32, size: Size, format: PixelFormat) -> BufferId {
    let id = BufferId(id);
    e.register_buffer(
        id,
        BufferInfo {
            size,
            format,
            modifier: Modifier::LINEAR,
        },
    )
    .unwrap();
    e.finish_write(id).unwrap();
    id
}

fn full(size: Size) -> Rect {
    Rect::new(0, 0, size.width, size.height)
}

fn layer(buffer: BufferId, size: Size, z: i32) -> Layer {
    Layer::new(buffer, full(size), full(size), z)
}

fn frame(output: OutputId, layers: &[Layer]) -> Frame<'_> {
    Frame { output, layers }
}

fn drain(e: &mut Eng) -> Vec<DisplayEvent> {
    let mut events = Vec::new();
    while let Some(ev) = e.poll_event().unwrap() {
        events.push(ev);
    }
    events
}

fn planes_of(e: &mut Eng, output: OutputId, layers: &[Layer]) -> Vec<u32> {
    e.validate(&frame(output, layers))
        .unwrap()
        .assignments()
        .iter()
        .map(|a| a.plane.0)
        .collect()
}

fn presented(output: OutputId, submission: SubmissionId) -> DisplayEvent {
    DisplayEvent::Presented { output, submission }
}

fn state(plane: u32, buffer: BufferId, zpos: i32) -> PlaneState {
    PlaneState {
        plane: PlaneId(plane),
        buffer,
        zpos,
    }
}

/// First modeset on one CRTC: only `video_port0` is driven. Its first frame
/// takes the primary 33, stays pending until the vsync, then is scanned out;
/// `video_port1` shows nothing.
#[test]
fn first_frame_on_port_0_latches_on_its_primary_only() {
    let mut e = engine(two_ports());
    let bg = register(&mut e, 1, FULL_HD, XR24);
    let ls = [layer(bg, FULL_HD, 0)];
    let s = e.present(&frame(VP0, &ls)).unwrap();
    assert_eq!(e.backend().pending(VP0), Some(s));
    assert_eq!(e.backend().scanout(VP0), None);
    assert!(
        drain(&mut e).is_empty(),
        "nothing is presented before vsync"
    );

    e.backend_mut().vsync();
    assert_eq!(drain(&mut e), [presented(VP0, s)]);
    assert_eq!(e.backend().pending(VP0), None);
    assert_eq!(e.backend().scanout(VP0).unwrap(), [state(33, bg, 0)]);
    assert_eq!(e.backend().scanout(VP1), None);
    assert_eq!(e.backend().commit_count(), 1);
}

/// Mode change while planes are assigned: port 0 goes to 1280x720 with three
/// layers on 33, 45 and 51. The capabilities change is announced, a scene
/// sized for the new mode keeps the same planes, and a buffer still sized for
/// 1920x1080 would need a downscale, which no plane claims (SCALING).
#[test]
fn mode_change_on_port_0_keeps_planes_and_refuses_a_stale_big_buffer() {
    let mut e = engine(two_ports());
    let big = [
        register(&mut e, 1, FULL_HD, XR24),
        register(&mut e, 2, FULL_HD, AR24),
        register(&mut e, 3, FULL_HD, AR24),
    ];
    let scene_big: Vec<Layer> = big
        .iter()
        .enumerate()
        .map(|(i, b)| layer(*b, FULL_HD, i as i32))
        .collect();
    assert_eq!(planes_of(&mut e, VP0, &scene_big), [33, 45, 51]);
    let s1 = e.present(&frame(VP0, &scene_big)).unwrap();
    e.backend_mut().vsync();
    assert_eq!(drain(&mut e), [presented(VP0, s1)]);

    e.backend_mut()
        .set_capabilities(caps_with(&[(VP0, HD), (VP1, FULL_HD)]));
    assert_eq!(drain(&mut e), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(e.capabilities().outputs[0].size, HD);
    assert_eq!(e.capabilities().outputs[1].size, FULL_HD);

    let stale = [Layer::new(big[0], full(FULL_HD), full(HD), 0)];
    match e.validate(&frame(VP0, &stale)) {
        Err(Error::Plan(PlanError::NoPlane { layer, reasons })) => {
            assert_eq!(layer, 0);
            assert!(reasons.contains(RejectReasons::SCALING), "{reasons:?}");
        }
        other => panic!("expected NoPlane, got {other:?}"),
    }
    // Port 1 is unaffected: a full-size layer still plans on its primary.
    assert_eq!(planes_of(&mut e, VP1, &[layer(big[0], FULL_HD, 0)]), [39]);

    let small = [
        register(&mut e, 4, HD, XR24),
        register(&mut e, 5, HD, AR24),
        register(&mut e, 6, HD, AR24),
    ];
    let scene_small: Vec<Layer> = small
        .iter()
        .enumerate()
        .map(|(i, b)| layer(*b, HD, i as i32))
        .collect();
    assert_eq!(planes_of(&mut e, VP0, &scene_small), [33, 45, 51]);
    let s2 = e.present(&frame(VP0, &scene_small)).unwrap();
    assert!(s2 > s1);
    e.backend_mut().vsync();
    assert!(drain(&mut e).contains(&presented(VP0, s2)));
    assert_eq!(
        e.backend().scanout(VP0).unwrap(),
        [
            state(33, small[0], 0),
            state(45, small[1], 1),
            state(51, small[2], 2)
        ]
    );
}

/// A commit failure after a good frame: the failed commit changes nothing,
/// the last good frame stays scanned out, and the retry takes the next
/// submission id and replaces the screen.
#[test]
fn commit_failure_keeps_the_last_good_frame_and_the_retry_succeeds() {
    let mut e = engine(two_ports());
    let a = register(&mut e, 1, FULL_HD, XR24);
    let b = register(&mut e, 2, FULL_HD, XR24);
    let la = [layer(a, FULL_HD, 0)];
    let lb = [layer(b, FULL_HD, 0)];
    let good = e.present(&frame(VP0, &la)).unwrap();
    e.backend_mut().vsync();
    assert_eq!(drain(&mut e), [presented(VP0, good)]);
    let commits = e.backend().commit_count();

    e.backend_mut().fail_next_commits(1);
    assert_eq!(
        e.present(&frame(VP0, &lb)),
        Err(Error::Backend(BackendError::Platform(-5)))
    );
    assert_eq!(e.backend().commit_count(), commits);
    assert_eq!(e.backend().pending(VP0), None);
    assert_eq!(e.backend().scanout(VP0).unwrap(), [state(33, a, 0)]);
    assert!(drain(&mut e).is_empty());

    let retry = e.present(&frame(VP0, &lb)).unwrap();
    assert_eq!(retry, SubmissionId(good.0 + 1), "failure used no id");
    e.backend_mut().vsync();
    let events = drain(&mut e);
    assert!(events.contains(&presented(VP0, retry)), "{events:?}");
    assert_eq!(e.backend().scanout(VP0).unwrap(), [state(33, b, 0)]);
}

/// Port 1 goes away (capabilities without it) and comes back. While it is
/// absent the planner reports an unknown output for it and port 0 keeps
/// working; when it returns, its frames plan on plane 39 again.
#[test]
fn port_1_going_away_and_coming_back() {
    let mut e = engine(two_ports());
    let a = register(&mut e, 1, FULL_HD, XR24);
    let b = register(&mut e, 2, FULL_HD, XR24);
    let la = [layer(a, FULL_HD, 0)];
    let lb = [layer(b, FULL_HD, 0)];
    e.present(&frame(VP1, &lb)).unwrap();
    e.backend_mut().vsync();
    drain(&mut e);

    e.backend_mut()
        .set_capabilities(caps_with(&[(VP0, FULL_HD)]));
    assert_eq!(drain(&mut e), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(e.capabilities().outputs.len(), 1);
    assert_eq!(
        e.validate(&frame(VP1, &lb)).unwrap_err(),
        Error::Plan(PlanError::UnknownOutput(VP1))
    );
    assert_eq!(planes_of(&mut e, VP0, &la), [33]);

    e.backend_mut().set_capabilities(two_ports());
    assert_eq!(drain(&mut e), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(planes_of(&mut e, VP1, &lb), [39]);
    let s = e.present(&frame(VP1, &lb)).unwrap();
    e.backend_mut().vsync();
    assert!(drain(&mut e).contains(&presented(VP1, s)));
}

/// A whole-device loss with a frame pending on each port, then recovery on
/// the same topology: presenting fails while lost and commits nothing, and
/// after `restore` both ports latch again with ids above the dropped ones.
#[test]
fn device_loss_with_both_ports_pending_and_recovery_relatches_them() {
    let mut e = engine(two_ports());
    let a = register(&mut e, 1, FULL_HD, XR24);
    let b = register(&mut e, 2, FULL_HD, XR24);
    let la = [layer(a, FULL_HD, 0)];
    let lb = [layer(b, FULL_HD, 0)];
    e.present(&frame(VP0, &la)).unwrap();
    let p1 = e.present(&frame(VP1, &lb)).unwrap();

    e.backend_mut().lose_device();
    assert_eq!(drain(&mut e).last(), Some(&DisplayEvent::DeviceLost));
    let commits = e.backend().commit_count();
    assert_eq!(
        e.present(&frame(VP0, &la)),
        Err(Error::Backend(BackendError::DeviceLost))
    );
    assert_eq!(e.backend().commit_count(), commits);

    e.backend_mut().restore(two_ports());
    assert_eq!(drain(&mut e), [DisplayEvent::CapabilitiesChanged]);
    let r0 = e.present(&frame(VP0, &la)).unwrap();
    let r1 = e.present(&frame(VP1, &lb)).unwrap();
    assert!(r0 > p1 && r1 > r0);
    e.backend_mut().vsync();
    let events = drain(&mut e);
    assert!(events.contains(&presented(VP0, r0)), "{events:?}");
    assert!(events.contains(&presented(VP1, r1)), "{events:?}");
    assert_eq!(e.backend().scanout(VP0).unwrap(), [state(33, a, 0)]);
    assert_eq!(e.backend().scanout(VP1).unwrap(), [state(39, b, 0)]);
}

/// An overlay moves between the two CRTCs: plane 45 is the second layer on
/// port 0, then port 1 takes two layers and gets 45 too, and port 0 later
/// drops to one layer. Each output's scanout lists its own planes.
///
/// The planner has no cross-output bookkeeping (see `rk3588_two_ports.rs`),
/// so port 0 keeps 45 in its scanout until its own next frame latches; this
/// pins the engine's model and does not claim the kernel allows it.
#[test]
fn overlay_45_moves_from_port_0_to_port_1() {
    let mut e = engine(two_ports());
    let a = register(&mut e, 1, FULL_HD, XR24);
    let ov = register(&mut e, 2, FULL_HD, AR24);
    let c = register(&mut e, 3, FULL_HD, XR24);
    let two = [layer(a, FULL_HD, 0), layer(ov, FULL_HD, 1)];
    assert_eq!(planes_of(&mut e, VP0, &two), [33, 45]);
    e.present(&frame(VP0, &two)).unwrap();
    e.backend_mut().vsync();
    drain(&mut e);
    assert_eq!(
        e.backend().scanout(VP0).unwrap(),
        [state(33, a, 0), state(45, ov, 1)]
    );

    let on1 = [layer(c, FULL_HD, 0), layer(ov, FULL_HD, 1)];
    assert_eq!(planes_of(&mut e, VP1, &on1), [39, 45]);
    let s1 = e.present(&frame(VP1, &on1)).unwrap();
    e.backend_mut().vsync();
    assert!(drain(&mut e).contains(&presented(VP1, s1)));
    assert_eq!(
        e.backend().scanout(VP1).unwrap(),
        [state(39, c, 0), state(45, ov, 1)]
    );
    assert_eq!(
        e.backend().scanout(VP0).unwrap(),
        [state(33, a, 0), state(45, ov, 1)],
        "port 0 is unchanged until its own frame latches"
    );

    let one = [layer(a, FULL_HD, 0)];
    let s0 = e.present(&frame(VP0, &one)).unwrap();
    e.backend_mut().vsync();
    assert!(drain(&mut e).contains(&presented(VP0, s0)));
    assert_eq!(e.backend().scanout(VP0).unwrap(), [state(33, a, 0)]);
    assert_eq!(
        e.backend().scanout(VP1).unwrap(),
        [state(39, c, 0), state(45, ov, 1)]
    );
}
