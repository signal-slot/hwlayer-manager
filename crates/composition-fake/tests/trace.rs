//! Structured traces: the exact sequence of entries the engine records for
//! a normal rotation, refused frames, a replaced pending submission, a latch
//! timeout and a device loss.
//!
//! Two layers, an RGB565 application layer below an ARGB8888 safety layer,
//! each on its own plane, rotate three buffers each.

use composition_core::{
    Assignment, BackendError, BlendMode, BufferId, BufferInfo, Capabilities, DisplayEvent,
    DropReason, Engine, Error, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId,
    PixelFormat, PlaneCaps, PlaneId, PlaneKind, Rect, Size, SubmissionId, Trace, TraceEvent, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const FULL: Rect = Rect::new(0, 0, 1280, 720);
const BOTTOM: PlaneId = PlaneId(31);
const TOP: PlaneId = PlaneId(41);

const APP: [BufferId; 3] = [BufferId(1), BufferId(2), BufferId(3)];
const SAFE: [BufferId; 3] = [BufferId(11), BufferId(12), BufferId(13)];

/// The assignment of every successful plan here: layer 0 on the bottom
/// plane, layer 1 on the top one.
const PLACED: [(usize, PlaneId); 2] = [(0, BOTTOM), (1, TOP)];

const S1: SubmissionId = SubmissionId(1);
const S2: SubmissionId = SubmissionId(2);
const S3: SubmissionId = SubmissionId(3);

/// An owned copy of a [`TraceEvent`], without the output, which is always
/// `OUT`, and with assignments reduced to layer and plane.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Entry {
    Check(Vec<(usize, PlaneId)>, Result<(), BackendError>),
    Validate(usize, Result<Vec<(usize, PlaneId)>, Error>),
    Commit(Result<SubmissionId, Error>),
    Latched(SubmissionId),
    Released(BufferId, Option<SubmissionId>),
    Dropped(SubmissionId, DropReason),
    LatchTimeout(SubmissionId),
    DeviceLost,
    CapabilitiesChanged,
}

fn placed(assignments: &[Assignment]) -> Vec<(usize, PlaneId)> {
    assignments.iter().map(|a| (a.layer, a.plane)).collect()
}

/// Keeps every entry with its time stamp.
#[derive(Default)]
struct Recorder(Vec<(u64, Entry)>);

impl Recorder {
    fn take(&mut self) -> Vec<(u64, Entry)> {
        core::mem::take(&mut self.0)
    }
}

impl Trace for Recorder {
    fn record(&mut self, at: u64, event: TraceEvent<'_>) {
        let entry = match event {
            TraceEvent::Check {
                output,
                assignments,
                result,
            } => {
                assert_eq!(output, OUT);
                Entry::Check(placed(assignments), result)
            }
            TraceEvent::Validate {
                output,
                layers,
                result,
            } => {
                assert_eq!(output, OUT);
                Entry::Validate(layers, result.map(placed))
            }
            TraceEvent::Commit { output, result } => {
                assert_eq!(output, OUT);
                Entry::Commit(result)
            }
            TraceEvent::Latched { output, submission } => {
                assert_eq!(output, OUT);
                Entry::Latched(submission)
            }
            TraceEvent::Released { buffer, submission } => Entry::Released(buffer, submission),
            TraceEvent::Dropped {
                output,
                submission,
                reason,
            } => {
                assert_eq!(output, OUT);
                Entry::Dropped(submission, reason)
            }
            TraceEvent::LatchTimeout { output, submission } => {
                assert_eq!(output, OUT);
                Entry::LatchTimeout(submission)
            }
            TraceEvent::DeviceLost => Entry::DeviceLost,
            TraceEvent::CapabilitiesChanged => Entry::CapabilitiesChanged,
        };
        self.0.push((at, entry));
    }
}

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
    caps.planes
        .push(plane(BOTTOM, PlaneKind::Primary, 0, PixelFormat::RGB565))
        .unwrap();
    caps.planes
        .push(plane(TOP, PlaneKind::Overlay, 1, PixelFormat::ARGB8888))
        .unwrap();
    caps
}

fn register_all<T: Trace>(engine: &mut Engine<FakeBackend, T>) {
    for (pool, format) in [(APP, PixelFormat::RGB565), (SAFE, PixelFormat::ARGB8888)] {
        for buffer in pool {
            let info = BufferInfo {
                size: Size::new(1280, 720),
                format,
                modifier: Modifier::LINEAR,
            };
            engine.register_buffer(buffer, info).unwrap();
            engine.finish_write(buffer).unwrap();
        }
    }
}

fn traced(replace_pending: bool) -> Engine<FakeBackend, Recorder> {
    let mut engine =
        Engine::new(FakeBackend::new(caps(replace_pending))).with_trace(Recorder::default());
    register_all(&mut engine);
    engine
}

fn layers(app: BufferId, safe: BufferId) -> [Layer; 2] {
    [
        Layer::new(app, FULL, FULL, 0),
        Layer {
            blend: BlendMode::Premultiplied,
            ..Layer::new(safe, FULL, FULL, 1)
        },
    ]
}

/// Presents frame `n` of the rotation.
fn present<T: Trace>(engine: &mut Engine<FakeBackend, T>, n: usize) -> Result<SubmissionId, Error> {
    let layers = layers(APP[n % 3], SAFE[n % 3]);
    engine.present(&Frame {
        output: OUT,
        layers: &layers,
    })
}

fn drain<T: Trace>(engine: &mut Engine<FakeBackend, T>) -> Vec<DisplayEvent> {
    core::iter::from_fn(|| engine.poll_event().unwrap()).collect()
}

/// The entries of a frame the backend takes at the first try.
fn accepted(at: u64, submission: SubmissionId) -> [(u64, Entry); 3] {
    [
        (at, Entry::Check(PLACED.to_vec(), Ok(()))),
        (at, Entry::Validate(2, Ok(PLACED.to_vec()))),
        (at, Entry::Commit(Ok(submission))),
    ]
}

/// The entries of the latch of `submission`, which took `previous` (frame
/// `n`) off screen.
fn latch(
    at: u64,
    submission: SubmissionId,
    previous: Option<(SubmissionId, usize)>,
) -> Vec<(u64, Entry)> {
    let mut entries = vec![(at, Entry::Latched(submission))];
    if let Some((previous, n)) = previous {
        entries.push((at, Entry::Released(APP[n % 3], Some(previous))));
        entries.push((at, Entry::Released(SAFE[n % 3], Some(previous))));
    }
    entries
}

#[test]
fn rotation_traces_validate_commit_latch_and_release() {
    let mut engine = traced(false);
    let mut expected = Vec::new();
    for n in 0..5 {
        let at = 16 * (n as u64 + 1);
        engine.advance_time(at);
        let submission = present(&mut engine, n).unwrap();
        assert_eq!(submission, SubmissionId(n as u64 + 1));
        engine.backend_mut().vsync();
        drain(&mut engine);

        expected.extend(accepted(at, submission));
        let previous = (n > 0).then(|| (SubmissionId(n as u64), n - 1));
        expected.extend(latch(at, submission, previous));
    }
    assert_eq!(engine.trace_mut().take(), expected);
}

#[test]
fn refused_frames_trace_why() {
    let mut engine = traced(false);
    engine.advance_time(5);

    // The planner refuses without asking the backend: the buffer is unknown.
    let unknown = layers(BufferId(99), SAFE[0]);
    let frame = Frame {
        output: OUT,
        layers: &unknown,
    };
    let error = engine.present(&frame).unwrap_err();
    assert!(matches!(error, Error::Plan(_)), "{error:?}");
    assert_eq!(
        engine.trace_mut().take(),
        [(5, Entry::Validate(2, Err(error)))]
    );

    // The backend's TEST_ONLY rejects the only candidate.
    engine.backend_mut().set_check_hook(|_, _| false);
    let error = present(&mut engine, 0).unwrap_err();
    assert!(matches!(error, Error::Plan(_)), "{error:?}");
    assert_eq!(
        engine.trace_mut().take(),
        [
            (
                5,
                Entry::Check(PLACED.to_vec(), Err(BackendError::Rejected))
            ),
            (5, Entry::Validate(2, Err(error))),
        ]
    );
    engine.backend_mut().set_check_hook(|_, _| true);

    // A plan made before the capabilities changed is refused at commit.
    let ok = layers(APP[0], SAFE[0]);
    let frame = Frame {
        output: OUT,
        layers: &ok,
    };
    let plan = engine.validate(&frame).unwrap();
    engine.backend_mut().set_capabilities(caps(false));
    assert_eq!(drain(&mut engine), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(engine.commit(&frame, &plan), Err(Error::StalePlan));

    // The hardware refuses the commit.
    engine.backend_mut().fail_next_commits(1);
    let refused = Error::Backend(BackendError::Platform(-5));
    assert_eq!(present(&mut engine, 0), Err(refused));

    let mut expected = vec![
        (5, Entry::Check(PLACED.to_vec(), Ok(()))),
        (5, Entry::Validate(2, Ok(PLACED.to_vec()))),
        (5, Entry::CapabilitiesChanged),
        (5, Entry::Commit(Err(Error::StalePlan))),
        (5, Entry::Check(PLACED.to_vec(), Ok(()))),
        (5, Entry::Validate(2, Ok(PLACED.to_vec()))),
        (5, Entry::Commit(Err(refused))),
    ];
    // Nothing was submitted, so the next submission is the first.
    assert_eq!(present(&mut engine, 0), Ok(S1));
    expected.extend(accepted(5, S1));
    assert_eq!(engine.trace_mut().take(), expected);
}

#[test]
fn replaced_pending_submission_traces_drop_and_release() {
    let mut engine = traced(true);
    engine.advance_time(16);
    present(&mut engine, 0).unwrap();
    engine.backend_mut().vsync();
    drain(&mut engine);
    engine.trace_mut().take();

    // S2 is replaced by S3 before it latched.
    engine.advance_time(32);
    assert_eq!(present(&mut engine, 1), Ok(S2));
    assert_eq!(present(&mut engine, 2), Ok(S3));
    engine.backend_mut().vsync();
    drain(&mut engine);

    let mut expected = accepted(32, S2).to_vec();
    expected.extend([
        (32, Entry::Check(PLACED.to_vec(), Ok(()))),
        (32, Entry::Validate(2, Ok(PLACED.to_vec()))),
        (32, Entry::Dropped(S2, DropReason::Replaced)),
        (32, Entry::Released(APP[1], Some(S2))),
        (32, Entry::Released(SAFE[1], Some(S2))),
        (32, Entry::Commit(Ok(S3))),
    ]);
    expected.extend(latch(32, S3, Some((S1, 0))));
    assert_eq!(engine.trace_mut().take(), expected);

    // A vertical blank inside the replacing commit: the pending submission
    // latched after all, and is traced as latched before the commit.
    engine.advance_time(48);
    let s4 = present(&mut engine, 3).unwrap();
    engine.backend_mut().vsync_in_next_commit();
    let s5 = present(&mut engine, 4).unwrap();
    engine.backend_mut().vsync();
    drain(&mut engine);

    let mut expected = accepted(48, s4).to_vec();
    expected.extend([
        (48, Entry::Check(PLACED.to_vec(), Ok(()))),
        (48, Entry::Validate(2, Ok(PLACED.to_vec()))),
    ]);
    expected.extend(latch(48, s4, Some((S3, 2))));
    expected.push((48, Entry::Commit(Ok(s5))));
    expected.extend(latch(48, s5, Some((s4, 3))));
    assert_eq!(engine.trace_mut().take(), expected);
}

#[test]
fn latch_timeout_is_traced_once_then_the_submission_is_dropped() {
    let mut engine = traced(true);
    engine.set_latch_timeout(Some(20));
    engine.advance_time(100);
    assert_eq!(present(&mut engine, 0), Ok(S1));
    // No vertical blank: S1 stays pending.
    for now in [110, 119, 120, 130] {
        engine.advance_time(now);
    }
    assert_eq!(
        drain(&mut engine),
        [DisplayEvent::LatchTimeout {
            output: OUT,
            submission: S1
        }]
    );

    // The client gives up on S1 and replaces it.
    engine.advance_time(140);
    assert_eq!(present(&mut engine, 1), Ok(S2));
    engine.backend_mut().vsync();
    drain(&mut engine);

    let mut expected = accepted(100, S1).to_vec();
    expected.push((120, Entry::LatchTimeout(S1)));
    expected.extend([
        (140, Entry::Check(PLACED.to_vec(), Ok(()))),
        (140, Entry::Validate(2, Ok(PLACED.to_vec()))),
        (140, Entry::Dropped(S1, DropReason::Replaced)),
        (140, Entry::Released(APP[0], Some(S1))),
        (140, Entry::Released(SAFE[0], Some(S1))),
        (140, Entry::Commit(Ok(S2))),
    ]);
    expected.extend(latch(140, S2, None));
    assert_eq!(engine.trace_mut().take(), expected);
}

#[test]
fn device_lost_traces_drops_releases_and_recovery() {
    // A borrowed sink: the test keeps the recorder.
    let mut recorder = Recorder::default();
    let mut engine = Engine::new(FakeBackend::new(caps(false))).with_trace(&mut recorder);
    register_all(&mut engine);

    engine.advance_time(16);
    present(&mut engine, 0).unwrap();
    engine.backend_mut().vsync();
    drain(&mut engine);
    present(&mut engine, 1).unwrap();

    engine.advance_time(32);
    engine.backend_mut().lose_device();
    assert_eq!(drain(&mut engine).last(), Some(&DisplayEvent::DeviceLost));

    // Nothing can be shown until the device is back.
    let lost = Error::Backend(BackendError::DeviceLost);
    assert_eq!(present(&mut engine, 2), Err(lost));

    engine.advance_time(48);
    engine.backend_mut().restore(caps(false));
    assert_eq!(drain(&mut engine), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(present(&mut engine, 2), Ok(S3));
    drop(engine);

    let mut expected = accepted(16, S1).to_vec();
    expected.extend(latch(16, S1, None));
    expected.extend(accepted(16, S2));
    expected.extend([
        (32, Entry::Dropped(S2, DropReason::DeviceLost)),
        // Pending first, then what was on screen; nothing is attributed.
        (32, Entry::Released(APP[1], None)),
        (32, Entry::Released(SAFE[1], None)),
        (32, Entry::Released(APP[0], None)),
        (32, Entry::Released(SAFE[0], None)),
        (32, Entry::DeviceLost),
        (
            32,
            Entry::Check(PLACED.to_vec(), Err(BackendError::DeviceLost)),
        ),
        (32, Entry::Validate(2, Err(lost))),
        (48, Entry::CapabilitiesChanged),
    ]);
    expected.extend(accepted(48, S3));
    assert_eq!(recorder.0, expected);
}
