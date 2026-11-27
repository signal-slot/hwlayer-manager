//! SoW §10 lifecycle scenarios (startup, steady state, application stop and
//! restart, lease loss, modeset, recovery) on the fake backend.
//!
//! These model the engine-side logic only. Nothing here is a hardware
//! result: the fake backend has no real timing, scanout or driver.
//!
//! Scenarios the fake backend cannot express:
//! - A real lease revocation or reset: `lose_device` is instantaneous and
//!   total. There is no partial loss (one output of several), no loss
//!   observed only by a failing ioctl, and no revoke racing a commit that is
//!   already in the kernel.
//! - A modeset that the display itself performs: `set_capabilities` replaces
//!   the whole capability set at once and does not touch the pending or
//!   latched commit (real hardware may blank or drop it).
//! - Process lifetime: an application stopping is modelled only as its layers
//!   disappearing from the frames and its buffers being unregistered; there
//!   is no client connection to drop and no fd or dma-buf lifetime.
//! - Real latch timing, vblank cadence, missed or late flips, and the time
//!   plane assignment takes on a target.
//! - Display recovery details: hotplug, EDID, link retraining, and a backend
//!   re-creating its resources beyond what `restore` with a new capability
//!   set expresses.

use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, BufferState, Capabilities, DisplayEvent, Engine,
    Error, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat, PlaneCaps,
    PlaneId, PlaneKind, PresentError, Rect, Size, SubmissionId, Zpos,
};
use composition_fake::{FakeBackend, PlaneState};

const OUT: OutputId = OutputId(0);
const FULL: Rect = Rect::new(0, 0, 1280, 720);

const APP_A: BufferId = BufferId(1);
const APP_B: BufferId = BufferId(2);
const SAFETY: BufferId = BufferId(3);

const VID: PlaneId = PlaneId(31);
const VIDL: PlaneId = PlaneId(41);

fn plane(id: PlaneId, kind: PlaneKind, zpos: i32, format: PixelFormat) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id,
        kind,
        outputs: 1,
        zpos: Zpos::Fixed(zpos),
        blend_modes: BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask(),
        ..PlaneCaps::default()
    };
    caps.formats.push(FormatModifier::linear(format)).unwrap();
    caps
}

type PlaneSpec = (PlaneId, PlaneKind, i32, PixelFormat);

fn caps_with(size: Size, refresh_mhz: u32, planes: &[PlaneSpec]) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size,
            refresh_mhz,
        })
        .unwrap();
    for (id, kind, zpos, format) in planes {
        caps.planes.push(plane(*id, *kind, *zpos, *format)).unwrap();
    }
    caps
}

const TWO_PLANES: [PlaneSpec; 2] = [
    (VID, PlaneKind::Primary, 0, PixelFormat::RGB565),
    (VIDL, PlaneKind::Overlay, 1, PixelFormat::ARGB8888),
];

/// An RGB565 plane underneath an ARGB8888 plane, 1280x720 at 60 Hz.
fn two_plane_caps() -> Capabilities {
    caps_with(Size::new(1280, 720), 60_000, &TWO_PLANES)
}

fn info(format: PixelFormat) -> BufferInfo {
    BufferInfo {
        size: Size::new(1280, 720),
        format,
        modifier: Modifier::LINEAR,
    }
}

fn register(engine: &mut Engine<FakeBackend>, buffer: BufferId, format: PixelFormat) {
    engine.register_buffer(buffer, info(format)).unwrap();
    engine.finish_write(buffer).unwrap();
}

fn engine() -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(two_plane_caps()));
    register(&mut engine, APP_A, PixelFormat::RGB565);
    register(&mut engine, APP_B, PixelFormat::RGB565);
    register(&mut engine, SAFETY, PixelFormat::ARGB8888);
    engine
}

fn app(buffer: BufferId) -> Layer {
    Layer::new(buffer, FULL, FULL, 0)
}

fn safety() -> Layer {
    let mut layer = Layer::new(SAFETY, FULL, FULL, 10);
    layer.blend = BlendMode::Premultiplied;
    layer
}

fn drain(engine: &mut Engine<FakeBackend>) -> Vec<DisplayEvent> {
    let mut events = Vec::new();
    while let Some(event) = engine.poll_event().unwrap() {
        events.push(event);
    }
    events
}

fn presented(submission: SubmissionId) -> DisplayEvent {
    DisplayEvent::Presented {
        output: OUT,
        submission,
    }
}

fn dropped(submission: SubmissionId) -> DisplayEvent {
    DisplayEvent::Dropped {
        output: OUT,
        submission,
    }
}

fn released(buffer: BufferId) -> DisplayEvent {
    DisplayEvent::Released { buffer }
}

fn present(engine: &mut Engine<FakeBackend>, layers: &[Layer]) -> Result<SubmissionId, Error> {
    engine.present(&Frame {
        output: OUT,
        layers,
    })
}

#[test]
fn startup_reports_capabilities_then_validates_and_commits_the_first_frame() {
    let caps = two_plane_caps();
    let mut engine = Engine::new_unprobed(FakeBackend::new(caps));
    assert_eq!(*engine.capabilities(), Capabilities::default());
    engine.probe();
    assert_eq!(*engine.capabilities(), caps);
    assert_eq!(engine.capabilities().outputs.len(), 1);
    assert_eq!(engine.capabilities().planes.len(), 2);
    assert_eq!(engine.capabilities().outputs[0].refresh_mhz, 60_000);

    register(&mut engine, APP_A, PixelFormat::RGB565);
    register(&mut engine, SAFETY, PixelFormat::ARGB8888);
    let layers = [app(APP_A), safety()];
    let frame = Frame {
        output: OUT,
        layers: &layers,
    };

    // Validation changes nothing.
    let plan = engine.validate(&frame).unwrap();
    assert_eq!(plan.assignments().len(), 2);
    assert_eq!(engine.backend().commit_count(), 0);
    assert!(engine.backend().check_count() >= 1);
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Ready));

    let first = engine.commit(&frame, &plan).unwrap();
    assert_eq!(engine.backend().commit_count(), 1);
    assert_eq!(engine.backend().pending(OUT), Some(first));
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Submitted));
    assert_eq!(engine.buffer_state(SAFETY), Some(BufferState::Submitted));
    assert!(drain(&mut engine).is_empty(), "nothing is on screen yet");

    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Scanning));
    assert_eq!(
        engine.backend().scanout(OUT).unwrap(),
        [
            PlaneState {
                plane: VID,
                buffer: APP_A,
                zpos: 0
            },
            PlaneState {
                plane: VIDL,
                buffer: SAFETY,
                zpos: 1
            },
        ]
    );
}

#[test]
fn steady_state_releases_every_rotated_buffer_exactly_once() {
    const FRAMES: usize = 40;
    let mut engine = engine();
    let mut released_count = [0usize; 4];
    let mut shown = APP_A;
    let mut last = None;

    for frame in 0..FRAMES {
        let next = if frame % 2 == 0 { APP_A } else { APP_B };
        let layers = [app(next), safety()];
        let id = present(&mut engine, &layers).unwrap();
        if let Some(last) = last {
            assert!(id > last, "submission ids increase");
        }
        last = Some(id);
        engine.backend_mut().vsync();
        let events = drain(&mut engine);
        assert_eq!(events[0], presented(id), "frame {frame}");
        for event in &events[1..] {
            match event {
                DisplayEvent::Released { buffer } => {
                    released_count[buffer.0 as usize] += 1;
                    // Only the previously shown application buffer is freed.
                    assert_eq!(*buffer, shown, "frame {frame}");
                    assert_eq!(engine.buffer_state(*buffer), Some(BufferState::Ready));
                    // It can be redrawn at once.
                    engine.begin_write(*buffer).unwrap();
                    engine.finish_write(*buffer).unwrap();
                }
                other => panic!("unexpected event {other:?} at frame {frame}"),
            }
        }
        assert_eq!(
            events.len(),
            if frame == 0 { 1 } else { 2 },
            "frame {frame}"
        );
        shown = next;
        assert_eq!(engine.buffer_state(next), Some(BufferState::Scanning));
        assert_eq!(engine.buffer_state(SAFETY), Some(BufferState::Scanning));
    }

    // The safety layer is never released; each rotation frees one buffer once.
    assert_eq!(
        released_count[APP_A.0 as usize] + released_count[APP_B.0 as usize],
        FRAMES - 1
    );
    assert_eq!(released_count[SAFETY.0 as usize], 0);
    assert!(drain(&mut engine).is_empty());
    assert_eq!(engine.backend().pending(OUT), None);
    assert_eq!(engine.backend().commit_count() as usize, FRAMES);
}

#[test]
fn application_stop_and_restart_frees_and_reassigns_resources() {
    let mut engine = engine();
    let first = present(&mut engine, &[app(APP_A), safety()]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);

    // The application's layers vanish mid-stream: its plane and buffer are
    // freed once the new frame is latched, the safety layer carries on.
    let second = present(&mut engine, &[safety()]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(second), released(APP_A)]);
    assert_eq!(
        engine.backend().scanout(OUT).unwrap(),
        [PlaneState {
            plane: VIDL,
            buffer: SAFETY,
            zpos: 1
        }]
    );

    // The application's buffers are unregistered (process gone).
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Ready));
    engine.unregister_buffer(APP_A).unwrap();
    engine.unregister_buffer(APP_B).unwrap();
    assert_eq!(engine.buffer_state(APP_A), None);
    assert!(
        present(&mut engine, &[app(APP_A), safety()]).is_err(),
        "a frame naming a gone client's buffer is refused, not committed"
    );
    assert_eq!(engine.backend().commit_count(), 2);
    // Unregistering what the display still shows is refused.
    assert_eq!(
        engine.unregister_buffer(SAFETY),
        Err(Error::Present(PresentError::BufferInUse(SAFETY)))
    );

    // The restarted client registers new buffers under the old ids and gets
    // the freed plane back.
    register(&mut engine, APP_A, PixelFormat::RGB565);
    let third = present(&mut engine, &[safety(), app(APP_A)]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(third)]);
    assert_eq!(
        engine.backend().scanout(OUT).unwrap(),
        [
            PlaneState {
                plane: VID,
                buffer: APP_A,
                zpos: 0
            },
            PlaneState {
                plane: VIDL,
                buffer: SAFETY,
                zpos: 1
            },
        ]
    );
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Scanning));
}

#[test]
fn application_stopping_with_a_frame_pending_still_releases_it() {
    let mut engine = engine();
    let first = present(&mut engine, &[app(APP_A), safety()]).unwrap();
    // Its buffer cannot be unregistered while submitted.
    assert_eq!(
        engine.unregister_buffer(APP_A),
        Err(Error::Present(PresentError::BufferInUse(APP_A)))
    );
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);
    let second = present(&mut engine, &[safety()]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(second), released(APP_A)]);
    engine.unregister_buffer(APP_A).unwrap();
}

#[test]
fn lease_loss_drops_pending_buffers_and_later_submits_fail_cleanly() {
    let mut engine = engine();
    let first = present(&mut engine, &[app(APP_A), safety()]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);
    let second = present(&mut engine, &[app(APP_B), safety()]).unwrap();

    engine.backend_mut().lose_device();
    assert_eq!(
        drain(&mut engine),
        [
            dropped(second),
            released(APP_B),
            released(SAFETY),
            released(APP_A),
            DisplayEvent::DeviceLost,
        ]
    );
    for buffer in [APP_A, APP_B, SAFETY] {
        assert_eq!(engine.buffer_state(buffer), Some(BufferState::Ready));
    }
    assert_eq!(engine.backend().scanout(OUT), None);
    assert_eq!(engine.backend().pending(OUT), None);

    // Later submits fail cleanly and repeatedly, without side effects.
    let commits = engine.backend().commit_count();
    for _ in 0..3 {
        assert_eq!(
            present(&mut engine, &[app(APP_A), safety()]),
            Err(Error::Backend(BackendError::DeviceLost))
        );
    }
    assert_eq!(engine.backend().commit_count(), commits);
    for buffer in [APP_A, APP_B, SAFETY] {
        assert_eq!(engine.buffer_state(buffer), Some(BufferState::Ready));
    }
    assert!(drain(&mut engine).is_empty());

    // Buffers can be redrawn and unregistered while the device is gone.
    engine.begin_write(APP_A).unwrap();
    engine.finish_write(APP_A).unwrap();
    engine.unregister_buffer(APP_B).unwrap();
}

#[test]
fn lease_loss_after_an_unpolled_latch_still_reports_presented_first() {
    let mut engine = engine();
    let first = present(&mut engine, &[app(APP_A), safety()]).unwrap();
    engine.backend_mut().vsync();
    // The latch is still queued in the backend when the device goes away.
    engine.backend_mut().lose_device();
    assert_eq!(
        drain(&mut engine),
        [
            presented(first),
            released(APP_A),
            released(SAFETY),
            DisplayEvent::DeviceLost
        ]
    );
    for buffer in [APP_A, SAFETY] {
        assert_eq!(engine.buffer_state(buffer), Some(BufferState::Ready));
    }
}

#[test]
fn lease_loss_with_nothing_pending_reports_only_the_loss_and_repeats_are_harmless() {
    let mut engine = engine();
    engine.backend_mut().lose_device();
    engine.backend_mut().lose_device();
    assert_eq!(
        drain(&mut engine),
        [DisplayEvent::DeviceLost, DisplayEvent::DeviceLost]
    );
    assert_eq!(
        present(&mut engine, &[app(APP_A)]),
        Err(Error::Backend(BackendError::DeviceLost))
    );
    for buffer in [APP_A, APP_B, SAFETY] {
        assert_eq!(engine.buffer_state(buffer), Some(BufferState::Ready));
    }
}

#[test]
fn lease_loss_between_validate_and_commit_fails_the_commit() {
    let mut engine = engine();
    let layers = [app(APP_A), safety()];
    let frame = Frame {
        output: OUT,
        layers: &layers,
    };
    let plan = engine.validate(&frame).unwrap();
    engine.backend_mut().lose_device();
    assert_eq!(
        engine.commit(&frame, &plan),
        Err(Error::Backend(BackendError::DeviceLost))
    );
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Ready));
    assert_eq!(engine.backend().commit_count(), 0);
}

#[test]
fn modeset_replans_for_the_new_output_and_stales_old_plans() {
    let mut engine = engine();
    let layers = [app(APP_A), safety()];
    let frame = Frame {
        output: OUT,
        layers: &layers,
    };
    let old_plan = engine.validate(&frame).unwrap();
    let first = engine.commit(&frame, &old_plan).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);

    // The refresh rate changes; same size, same planes.
    let faster = caps_with(Size::new(1280, 720), 120_000, &TWO_PLANES);
    engine.backend_mut().set_capabilities(faster);
    assert_eq!(drain(&mut engine), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(engine.capabilities().outputs[0].refresh_mhz, 120_000);
    assert_eq!(engine.commit(&frame, &old_plan), Err(Error::StalePlan));
    // The latched frame is untouched by the announcement.
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Scanning));
    let second = present(&mut engine, &[app(APP_B), safety()]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(second), released(APP_A)]);

    // The output shrinks: the planner decides again for the new size.
    engine
        .backend_mut()
        .set_capabilities(caps_with(Size::new(640, 360), 60_000, &TWO_PLANES));
    assert_eq!(drain(&mut engine), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(engine.capabilities().outputs[0].size, Size::new(640, 360));
    let fitted = Rect::new(0, 0, 640, 360);
    // The old full-size frame is no longer rejected for scaling: it reaches
    // beyond the output and the planner clips it (see planner.rs).
    let clipped = present(&mut engine, &[app(APP_A), safety()]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(clipped), released(APP_B)]);
    let a = Layer::new(APP_A, fitted, fitted, 0);
    let mut s = Layer::new(SAFETY, fitted, fitted, 10);
    s.blend = BlendMode::Premultiplied;
    let third = present(&mut engine, &[a, s]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(third)]);
}

#[test]
fn modeset_with_a_pending_frame_keeps_it_pending() {
    let mut engine = engine();
    let first = present(&mut engine, &[app(APP_A), safety()]).unwrap();
    let slower = caps_with(Size::new(1280, 720), 30_000, &TWO_PLANES);
    engine.backend_mut().set_capabilities(slower);
    assert_eq!(drain(&mut engine), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(engine.backend().pending(OUT), Some(first));
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Submitted));
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);
}

#[test]
fn modeset_that_loses_a_plane_makes_the_old_frame_unplannable() {
    let mut engine = engine();
    let layers = [app(APP_A), safety()];
    present(&mut engine, &layers).unwrap();
    engine.backend_mut().vsync();
    drain(&mut engine);

    let one = caps_with(Size::new(1280, 720), 60_000, &TWO_PLANES[..1]);
    engine.backend_mut().set_capabilities(one);
    assert_eq!(drain(&mut engine), [DisplayEvent::CapabilitiesChanged]);
    assert!(matches!(present(&mut engine, &layers), Err(Error::Plan(_))));
    // The application layer alone still fits the remaining plane.
    present(&mut engine, &[app(APP_B)]).unwrap();
}

#[test]
fn recovery_accepts_a_new_capability_set_and_frames_flow_again() {
    let mut engine = engine();
    let first = present(&mut engine, &[app(APP_A), safety()]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);
    engine.backend_mut().lose_device();
    assert_eq!(drain(&mut engine).last(), Some(&DisplayEvent::DeviceLost));
    assert_eq!(
        present(&mut engine, &[app(APP_A)]),
        Err(Error::Backend(BackendError::DeviceLost))
    );

    // The device returns with different plane identifiers and refresh.
    let (new_base, new_over) = (PlaneId(7), PlaneId(9));
    let recovered = caps_with(
        Size::new(1280, 720),
        50_000,
        &[
            (new_base, PlaneKind::Primary, 0, PixelFormat::RGB565),
            (new_over, PlaneKind::Overlay, 1, PixelFormat::ARGB8888),
        ],
    );
    engine.backend_mut().restore(recovered);
    assert_eq!(drain(&mut engine), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(*engine.capabilities(), recovered);

    let second = present(&mut engine, &[app(APP_B), safety()]).unwrap();
    assert!(second > first, "submission identifiers keep increasing");
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(second)]);
    assert_eq!(
        engine.backend().scanout(OUT).unwrap(),
        [
            PlaneState {
                plane: new_base,
                buffer: APP_B,
                zpos: 0
            },
            PlaneState {
                plane: new_over,
                buffer: SAFETY,
                zpos: 1
            },
        ]
    );

    // Steady state resumes with rotation.
    let third = present(&mut engine, &[app(APP_A), safety()]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(third), released(APP_B)]);
}

#[test]
fn recovery_with_fewer_planes_degrades_instead_of_failing_open() {
    let mut engine = engine();
    engine.backend_mut().lose_device();
    drain(&mut engine);
    let reduced = caps_with(
        Size::new(1280, 720),
        60_000,
        &[(VIDL, PlaneKind::Primary, 0, PixelFormat::ARGB8888)],
    );
    engine.backend_mut().restore(reduced);
    assert_eq!(drain(&mut engine), [DisplayEvent::CapabilitiesChanged]);
    // Two layers no longer fit; the engine says so rather than committing.
    assert!(matches!(
        present(&mut engine, &[app(APP_A), safety()]),
        Err(Error::Plan(_))
    ));
    assert_eq!(engine.backend().commit_count(), 0);
    // The safety layer alone does.
    let id = present(&mut engine, &[safety()]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(id)]);
}

#[test]
fn repeated_loss_and_recovery_cycles_leak_nothing() {
    let mut engine = engine();
    for cycle in 0..10 {
        let id = present(&mut engine, &[app(APP_A), safety()]).unwrap();
        if cycle % 2 == 0 {
            engine.backend_mut().vsync();
        }
        engine.backend_mut().lose_device();
        let events = drain(&mut engine);
        assert_eq!(
            events.last(),
            Some(&DisplayEvent::DeviceLost),
            "cycle {cycle}"
        );
        assert!(
            events.contains(&released(APP_A)) && events.contains(&released(SAFETY)),
            "cycle {cycle}: {events:?}"
        );
        if cycle % 2 == 1 {
            assert!(events.contains(&dropped(id)), "cycle {cycle}: {events:?}");
        }
        engine.backend_mut().restore(two_plane_caps());
        assert_eq!(drain(&mut engine), [DisplayEvent::CapabilitiesChanged]);
        for buffer in [APP_A, SAFETY] {
            assert_eq!(engine.buffer_state(buffer), Some(BufferState::Ready));
        }
    }
}
