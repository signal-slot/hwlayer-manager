//! Readiness tokens (docs/sync-fences.md §3.1) against the fake backend.
//!
//! Host tests only: the fake models a backend that waits for a token the
//! way a kernel waits on an `IN_FENCE_FD`, nothing more.

use composition_core::{
    BackendError, BufferId, BufferInfo, BufferState, Capabilities, DisplayBackend, DisplayEvent,
    Engine, Error, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat,
    PlaneCaps, PlaneId, PlaneKind, PresentError, ReadinessState, ReadyToken, Rect, Size,
    SubmissionId, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const FULL: Rect = Rect::new(0, 0, 640, 480);
const PRIMARY: PlaneId = PlaneId(1);

const A: BufferId = BufferId(1);
const B: BufferId = BufferId(2);
const C: BufferId = BufferId(3);

const T: ReadyToken = ReadyToken(17);

fn caps(replace_pending: bool) -> Capabilities {
    let mut caps = Capabilities {
        replace_pending,
        ..Capabilities::default()
    };
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(640, 480),
            refresh_mhz: 60_000,
        })
        .unwrap();
    let mut plane = PlaneCaps {
        id: PRIMARY,
        kind: PlaneKind::Primary,
        outputs: 1,
        zpos: Zpos::Fixed(0),
        ..PlaneCaps::default()
    };
    plane
        .formats
        .push(FormatModifier::linear(PixelFormat::XRGB8888))
        .unwrap();
    caps.planes.push(plane).unwrap();
    caps
}

/// An engine with buffers A, B and C registered and writable.
fn engine(waits: bool, replace_pending: bool) -> Engine<FakeBackend> {
    let mut backend = FakeBackend::new(caps(replace_pending));
    backend.set_waits_for_readiness(waits);
    let mut engine = Engine::new(backend);
    for buffer in [A, B, C] {
        let info = BufferInfo {
            size: Size::new(640, 480),
            format: PixelFormat::XRGB8888,
            modifier: Modifier::LINEAR,
        };
        engine.register_buffer(buffer, info).unwrap();
    }
    engine
}

fn show(engine: &mut Engine<FakeBackend>, buffer: BufferId) -> Result<SubmissionId, Error> {
    let layers = [Layer::new(buffer, FULL, FULL, 0)];
    engine.present(&Frame {
        output: OUT,
        layers: &layers,
    })
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

fn not_ready(buffer: BufferId) -> Result<SubmissionId, Error> {
    Err(Error::Present(PresentError::BufferNotReady(buffer)))
}

/// B finished without a token and on screen.
fn with_b_on_screen(engine: &mut Engine<FakeBackend>) -> SubmissionId {
    engine.finish_write(B).unwrap();
    let shown = show(engine, B).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(engine), [presented(shown)]);
    shown
}

#[test]
fn readiness_pending_refuses_submit_on_a_non_waiting_backend() {
    let mut e = engine(false, false);
    e.finish_write_pending(A, T).unwrap();
    assert_eq!(e.buffer_state(A), Some(BufferState::Ready));
    assert_eq!(e.readiness(A), Some(ReadinessState::Pending(T)));

    assert_eq!(show(&mut e, A), not_ready(A));
    assert_eq!(e.backend().commit_count(), 0);
    // The engine never handed the token to a backend that cannot wait.
    assert!(e.backend().readiness_log().is_empty());
    // Asked directly, the default fake refuses the token.
    assert_eq!(
        e.backend_mut().set_readiness(A, T),
        Err(BackendError::Rejected)
    );
    assert!(drain(&mut e).is_empty());
}

#[test]
fn readiness_signalled_allows_submit() {
    let mut e = engine(false, false);
    e.finish_write_pending(A, T).unwrap();
    assert_eq!(show(&mut e, A), not_ready(A));

    assert_eq!(e.readiness_signalled(T), 1);
    assert_eq!(e.readiness(A), None);
    let first = show(&mut e, A).unwrap();
    e.backend_mut().vsync();
    assert_eq!(drain(&mut e), [presented(first)]);
    // A second signal of the same token finds nothing.
    assert_eq!(e.readiness_signalled(T), 0);
}

#[test]
fn readiness_waiting_backend_receives_the_token_and_latches_after_signal() {
    let mut e = engine(true, false);
    e.finish_write_pending(A, T).unwrap();
    let first = show(&mut e, A).unwrap();
    assert_eq!(e.buffer_state(A), Some(BufferState::Submitted));
    // Given before the checks and again before the commit.
    assert!(e.backend().readiness_log().len() >= 2);
    assert!(e.backend().readiness_log().iter().all(|g| *g == (A, T)));
    assert_eq!(e.backend().waiting_tokens(OUT), [T]);

    // Not latched while the token is unsignalled.
    e.backend_mut().vsync();
    assert!(drain(&mut e).is_empty());
    assert_eq!(e.backend().scanout(OUT), None);

    // The backend learns the signal itself, as a kernel does.
    e.backend_mut().signal_token(T);
    e.backend_mut().vsync();
    assert_eq!(drain(&mut e), [presented(first)]);
    assert_eq!(e.buffer_state(A), Some(BufferState::Scanning));
    // Latched by a waiting backend: plainly ready from then on.
    assert_eq!(e.readiness(A), None);
}

#[test]
fn readiness_signalled_at_the_engine_clears_the_backend_wait() {
    let mut e = engine(true, false);
    e.finish_write_pending(A, T).unwrap();
    let first = show(&mut e, A).unwrap();
    e.backend_mut().vsync();
    assert!(drain(&mut e).is_empty());

    assert_eq!(e.readiness_signalled(T), 1);
    assert!(e.backend().waiting_tokens(OUT).is_empty());
    e.backend_mut().vsync();
    assert_eq!(drain(&mut e), [presented(first)]);
}

#[test]
fn readiness_failed_refuses_a_new_frame_until_rewritten() {
    let mut e = engine(false, false);
    e.finish_write_pending(A, T).unwrap();
    assert_eq!(e.readiness_failed(T), Ok(1));
    assert_eq!(e.readiness(A), Some(ReadinessState::Failed(T)));
    assert_eq!(e.buffer_state(A), Some(BufferState::Ready));
    assert_eq!(show(&mut e, A), not_ready(A));
    // Nothing was submitted, so there is nothing to drop.
    assert!(drain(&mut e).is_empty());

    // A late signal does not make the content good.
    assert_eq!(e.readiness_signalled(T), 0);
    assert_eq!(show(&mut e, A), not_ready(A));

    e.begin_write(A).unwrap();
    assert_eq!(e.readiness(A), None);
    e.finish_write(A).unwrap();
    show(&mut e, A).unwrap();
}

#[test]
fn readiness_failed_on_a_waiting_backend_drops_the_submission() {
    let mut e = engine(true, false);
    with_b_on_screen(&mut e);

    e.finish_write_pending(A, T).unwrap();
    let waiting = show(&mut e, A).unwrap();
    e.backend_mut().vsync();
    assert!(drain(&mut e).is_empty());

    assert_eq!(e.readiness_failed(T), Ok(1));
    assert_eq!(e.backend().failed_tokens(), [T]);
    assert_eq!(
        drain(&mut e),
        [dropped(waiting), DisplayEvent::Released { buffer: A }]
    );
    // What was on screen stays, and the withdrawn commit never latches.
    assert_eq!(e.backend().scanout(OUT).unwrap()[0].buffer, B);
    assert_eq!(e.backend().pending(OUT), None);
    e.backend_mut().signal_token(T);
    e.backend_mut().vsync();
    assert!(drain(&mut e).is_empty());
    assert_eq!(e.buffer_state(B), Some(BufferState::Scanning));
    assert_eq!(e.buffer_state(A), Some(BufferState::Ready));
    assert_eq!(e.readiness(A), Some(ReadinessState::Failed(T)));
    // A new frame with A is refused, by the engine, before the backend.
    let commits = e.backend().commit_count();
    assert_eq!(show(&mut e, A), not_ready(A));
    assert_eq!(e.backend().commit_count(), commits);

    // The output is free again for a good frame.
    e.begin_write(A).unwrap();
    e.finish_write(A).unwrap();
    let next = show(&mut e, A).unwrap();
    e.backend_mut().vsync();
    assert_eq!(
        drain(&mut e),
        [presented(next), DisplayEvent::Released { buffer: B }]
    );
}

#[test]
fn readiness_timed_out_on_a_waiting_backend_drops_the_submission() {
    let mut e = engine(true, false);
    e.set_readiness_timeout(Some(100));
    e.advance_time(1000);
    e.finish_write_pending(A, T).unwrap();
    let waiting = show(&mut e, A).unwrap();

    e.advance_time(1099);
    assert!(drain(&mut e).is_empty());
    assert_eq!(e.readiness(A), Some(ReadinessState::Pending(T)));

    e.advance_time(1100);
    assert_eq!(e.readiness(A), Some(ReadinessState::TimedOut(T)));
    assert_eq!(e.backend().failed_tokens(), [T]);
    assert_eq!(
        drain(&mut e),
        [dropped(waiting), DisplayEvent::Released { buffer: A }]
    );
    // Reported once.
    e.advance_time(5000);
    assert!(drain(&mut e).is_empty());
    assert_eq!(e.backend().failed_tokens(), [T]);

    // A late signal, at the engine or at the backend, changes nothing.
    assert_eq!(e.readiness_signalled(T), 0);
    e.backend_mut().signal_token(T);
    e.backend_mut().vsync();
    assert!(drain(&mut e).is_empty());
    assert_eq!(e.readiness(A), Some(ReadinessState::TimedOut(T)));
    assert_eq!(show(&mut e, A), not_ready(A));
}

#[test]
fn readiness_timed_out_on_a_non_waiting_backend_is_refused() {
    let mut e = engine(false, false);
    e.set_readiness_timeout(Some(10));
    e.finish_write_pending(A, T).unwrap();
    e.advance_time(10);
    assert_eq!(e.readiness(A), Some(ReadinessState::TimedOut(T)));
    // Nothing had been submitted: no event, only the refusal.
    assert!(drain(&mut e).is_empty());
    assert!(e.backend().failed_tokens().is_empty());
    assert_eq!(e.readiness_signalled(T), 0);
    assert_eq!(show(&mut e, A), not_ready(A));
}

#[test]
fn readiness_timeout_is_off_by_default() {
    let mut e = engine(false, false);
    e.finish_write_pending(A, T).unwrap();
    e.advance_time(u64::MAX);
    assert_eq!(e.readiness(A), Some(ReadinessState::Pending(T)));
}

#[test]
fn readiness_token_that_never_signals_keeps_the_old_frame() {
    let mut e = engine(true, true);
    e.set_latch_timeout(Some(50));
    with_b_on_screen(&mut e);

    e.finish_write_pending(A, T).unwrap();
    let waiting = show(&mut e, A).unwrap();
    for now in 1..=200 {
        e.advance_time(now);
        e.backend_mut().vsync();
    }
    // Overdue is reported once; nothing is presented or released.
    assert_eq!(
        drain(&mut e),
        [DisplayEvent::LatchTimeout {
            output: OUT,
            submission: waiting
        }]
    );
    assert_eq!(e.backend().scanout(OUT).unwrap()[0].buffer, B);
    assert_eq!(e.buffer_state(B), Some(BufferState::Scanning));
    assert_eq!(e.buffer_state(A), Some(BufferState::Submitted));
    assert_eq!(e.readiness(A), Some(ReadinessState::Pending(T)));

    // The client gives up on it by replacing it with a good frame.
    e.finish_write(C).unwrap();
    let next = show(&mut e, C).unwrap();
    assert_eq!(
        drain(&mut e),
        [dropped(waiting), DisplayEvent::Released { buffer: A }]
    );
    e.backend_mut().vsync();
    assert_eq!(
        drain(&mut e),
        [presented(next), DisplayEvent::Released { buffer: B }]
    );
    // A still waits on its token and may be shown once it signals.
    assert_eq!(e.readiness(A), Some(ReadinessState::Pending(T)));
}

#[test]
fn readiness_never_releases_twice() {
    let mut e = engine(true, true);
    with_b_on_screen(&mut e);
    let mut events = Vec::new();

    // Waiting frame, failed: dropped, A released once.
    e.finish_write_pending(A, T).unwrap();
    show(&mut e, A).unwrap();
    e.readiness_failed(T).unwrap();
    events.extend(drain(&mut e));
    // Failing again, or after a timeout, finds nothing more to release.
    assert_eq!(e.readiness_failed(T), Ok(0));
    e.set_readiness_timeout(Some(1));
    e.advance_time(100);
    events.extend(drain(&mut e));

    // A rewritten with a new token, latched after it signals, then
    // replaced: released once more, after its own latch was replaced.
    let u = ReadyToken(18);
    e.begin_write(A).unwrap();
    e.finish_write_pending(A, u).unwrap();
    let a = show(&mut e, A).unwrap();
    e.backend_mut().signal_token(u);
    e.backend_mut().vsync();
    events.extend(drain(&mut e));
    // The latch made A plainly ready, so the timeout no longer applies.
    e.advance_time(1000);
    assert_eq!(e.readiness(A), None);
    e.finish_write(C).unwrap();
    let c = show(&mut e, C).unwrap();
    e.backend_mut().vsync();
    events.extend(drain(&mut e));

    assert!(events.contains(&presented(a)));
    assert!(events.contains(&presented(c)));
    let released_a = events
        .iter()
        .filter(|ev| **ev == DisplayEvent::Released { buffer: A })
        .count();
    let released_b = events
        .iter()
        .filter(|ev| **ev == DisplayEvent::Released { buffer: B })
        .count();
    assert_eq!(released_a, 2, "{events:?}");
    assert_eq!(released_b, 1, "{events:?}");
}

#[test]
fn readiness_shared_token_signals_every_buffer() {
    let mut e = engine(false, false);
    e.finish_write_pending(A, T).unwrap();
    e.finish_write_pending(C, T).unwrap();
    assert_eq!(e.readiness_signalled(T), 2);
    assert_eq!(e.readiness(A), None);
    assert_eq!(e.readiness(C), None);
}

/// The same frames with and without the waiting mode, none using a token.
fn run_untokened(waits: bool) -> (Vec<DisplayEvent>, u32, u32) {
    let mut e = engine(waits, true);
    let mut events = Vec::new();
    for buffer in [A, B, C] {
        e.finish_write(buffer).unwrap();
    }
    for round in 0..6 {
        let buffer = [A, B, C][round % 3];
        show(&mut e, buffer).unwrap();
        if round % 2 == 0 {
            e.backend_mut().vsync();
        }
        e.advance_time(round as u64);
        events.extend(drain(&mut e));
    }
    assert!(e.backend().readiness_log().is_empty());
    (
        events,
        e.backend().check_count(),
        e.backend().commit_count(),
    )
}

#[test]
fn readiness_frames_without_tokens_behave_as_before() {
    let plain = run_untokened(false);
    let waiting = run_untokened(true);
    assert_eq!(plain, waiting);
    assert!(!plain.0.is_empty());
}
