//! A second nonblocking commit while one is pending is refused with a busy
//! result, not queued or replaced (the shape the merged source read
//! `docs/rk3588-vop2-flip-source.md` gives for a `-EBUSY`).
//!
//! Fake-backend, host-only results. Nothing here is a hardware claim and none
//! of it counts toward a milestone.
//!
//! Two paths are pinned:
//!
//! * the engine's own bookkeeping: with `replace_pending` false the engine
//!   refuses the second frame with `PresentError::Busy` before the backend is
//!   asked;
//! * the backend's busy result: a thin test-local wrapper makes commit return
//!   `BackendError::Busy` (the fake cannot be made to do so through the engine
//!   without a src change: the engine's refusal comes first).
//!
//! In both, the first frame still reaches `Presented` (the engine's name for
//! latched) when its flip event arrives, the busy frame is neither presented
//! nor silently lost (the call fails, the display and buffer states are
//! unchanged, the caller retries), and nothing is presented without a flip.

use composition_core::{
    BackendError, BackendEvent, BlendMode, BufferId, BufferInfo, BufferState, Capabilities,
    DisplayBackend, DisplayEvent, Engine, Error, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, Plan, PlaneCaps, PlaneId, PlaneKind, PresentError, Rect,
    Size, SubmissionId, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const FULL: Rect = Rect::new(0, 0, 1280, 720);
const A: BufferId = BufferId(1);
const B: BufferId = BufferId(2);

fn caps(replace_pending: bool) -> Capabilities {
    let mut caps = Capabilities {
        replace_pending,
        ..Capabilities::default()
    };
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(1280, 720),
            refresh_mhz: 60_000,
        })
        .unwrap();
    let mut plane = PlaneCaps {
        id: PlaneId(1),
        kind: PlaneKind::Primary,
        outputs: 1,
        zpos: Zpos::Fixed(0),
        blend_modes: BlendMode::Opaque.mask(),
        ..PlaneCaps::default()
    };
    plane
        .formats
        .push(FormatModifier::linear(PixelFormat::ARGB8888))
        .unwrap();
    caps.planes.push(plane).unwrap();
    caps
}

/// Wraps the fake backend and answers the next `busy` commits made while a
/// frame is pending with `BackendError::Busy`, as a driver would with
/// `-EBUSY`, without changing the fake's state.
struct BusyBackend {
    inner: FakeBackend,
    busy: u32,
    busy_seen: u32,
}

impl DisplayBackend for BusyBackend {
    fn capabilities(&mut self, caps: &mut Capabilities) {
        self.inner.capabilities(caps);
    }
    fn check(&mut self, frame: &Frame<'_>, plan: &Plan) -> Result<(), BackendError> {
        self.inner.check(frame, plan)
    }
    fn commit(
        &mut self,
        submission: SubmissionId,
        frame: &Frame<'_>,
        plan: &Plan,
    ) -> Result<(), BackendError> {
        if self.busy > 0 && self.inner.pending(frame.output).is_some() {
            self.busy -= 1;
            self.busy_seen += 1;
            return Err(BackendError::Busy);
        }
        self.inner.commit(submission, frame, plan)
    }
    fn poll_event(&mut self) -> Option<BackendEvent> {
        self.inner.poll_event()
    }
}

fn info() -> BufferInfo {
    BufferInfo {
        size: Size::new(1280, 720),
        format: PixelFormat::ARGB8888,
        modifier: Modifier::LINEAR,
    }
}

fn setup<B: DisplayBackend>(backend: B) -> Engine<B> {
    let mut engine = Engine::new(backend);
    for buffer in [A, B] {
        engine.register_buffer(buffer, info()).unwrap();
        engine.finish_write(buffer).unwrap();
    }
    engine
}

fn layers(buffer: BufferId) -> [Layer; 1] {
    [Layer::new(buffer, FULL, FULL, 0)]
}

fn frame(layers: &[Layer]) -> Frame<'_> {
    Frame {
        output: OUT,
        layers,
    }
}

fn drain<B: DisplayBackend>(engine: &mut Engine<B>) -> Vec<DisplayEvent> {
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

fn is_presented(events: &[DisplayEvent]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, DisplayEvent::Presented { .. }))
}

#[test]
fn engine_refuses_second_frame_and_first_still_latches() {
    let mut engine = setup(FakeBackend::new(caps(false)));
    let (la, lb) = (layers(A), layers(B));
    let first = engine.present(&frame(&la)).unwrap();

    // Second frame while the first is pending: refused, not replaced.
    assert_eq!(
        engine.present(&frame(&lb)),
        Err(Error::Present(PresentError::Busy(OUT)))
    );
    assert_eq!(engine.backend().commit_count(), 1);
    assert_eq!(engine.backend().pending(OUT), Some(first));
    assert_eq!(engine.buffer_state(A), Some(BufferState::Submitted));
    // The refused frame's buffer is untouched: usable for the retry.
    assert_eq!(engine.buffer_state(B), Some(BufferState::Ready));

    // No flip event yet: nothing is reported, even after more polling and a
    // further refused attempt.
    assert!(drain(&mut engine).is_empty());
    assert!(engine.present(&frame(&lb)).is_err());
    assert!(drain(&mut engine).is_empty());
    assert_eq!(engine.backend().scanout(OUT), None);

    // The flip event arrives: the first frame is reported, only it.
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);

    // The caller's retry now succeeds and stays pending until its own flip.
    let second = engine.present(&frame(&lb)).unwrap();
    assert_ne!(first, second);
    assert!(drain(&mut engine).is_empty());
    engine.backend_mut().vsync();
    assert_eq!(
        drain(&mut engine),
        [presented(second), DisplayEvent::Released { buffer: A }]
    );
}

#[test]
fn backend_busy_is_reported_and_leaves_the_pending_frame_alone() {
    // The engine believes it may replace; the driver says busy anyway.
    let backend = BusyBackend {
        inner: FakeBackend::new(caps(true)),
        busy: 1,
        busy_seen: 0,
    };
    let mut engine = setup(backend);
    let (la, lb) = (layers(A), layers(B));
    let first = engine.present(&frame(&la)).unwrap();

    assert_eq!(
        engine.present(&frame(&lb)),
        Err(Error::Backend(BackendError::Busy))
    );
    // Nothing changed: the first frame is still the pending one, the busy
    // frame's buffer is still Ready, and nothing was presented or dropped.
    // The engine's own bookkeeping did not refuse it first.
    assert_eq!(engine.backend().busy_seen, 1);
    assert_eq!(engine.backend().inner.pending(OUT), Some(first));
    assert_eq!(engine.buffer_state(A), Some(BufferState::Submitted));
    assert_eq!(engine.buffer_state(B), Some(BufferState::Ready));
    let events = drain(&mut engine);
    assert!(events.is_empty(), "{events:?}");

    // The first frame latches when its flip event arrives.
    engine.backend_mut().inner.vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);

    // Retrying the busy frame after the flip succeeds and is reported only
    // at its own flip.
    let second = engine.present(&frame(&lb)).unwrap();
    assert!(drain(&mut engine).is_empty());
    engine.backend_mut().inner.vsync();
    assert_eq!(
        drain(&mut engine),
        [presented(second), DisplayEvent::Released { buffer: A }]
    );
}

#[test]
fn retry_the_backend_accepts_replaces_and_never_presents_the_first() {
    // After a busy result, a retry the backend accepts replaces the pending
    // frame: the replaced one is dropped, never presented, and the new one is
    // reported at its own flip.
    let backend = BusyBackend {
        inner: FakeBackend::new(caps(true)),
        busy: 1,
        busy_seen: 0,
    };
    let mut engine = setup(backend);
    let (la, lb) = (layers(A), layers(B));
    let first = engine.present(&frame(&la)).unwrap();
    assert_eq!(
        engine.present(&frame(&lb)),
        Err(Error::Backend(BackendError::Busy))
    );
    let second = engine.present(&frame(&lb)).unwrap();
    let events = drain(&mut engine);
    assert!(!is_presented(&events), "{events:?}");
    assert!(
        events.iter().any(|e| matches!(
            e,
            DisplayEvent::Dropped { submission, .. } if *submission == first
        )),
        "{events:?}"
    );
    engine.backend_mut().inner.vsync();
    assert_eq!(drain(&mut engine), [presented(second)]);
}
