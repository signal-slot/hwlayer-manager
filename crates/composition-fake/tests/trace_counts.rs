//! Count relations of the engine's [`Trace`] for the sequences the board
//! scenarios drive: a steady hold, a stop, a failing or lost display, and a
//! frame dropped for each [`DropReason`].
//!
//! `tests/trace.rs` pins the exact entries of single operations. This file
//! pins what a consumer that only counts entries (the harness's trace
//! emission, `scripts/board-scenarios.sh`) may rely on. Each relation is a
//! named `inv_*` helper with its doc comment; a test asserts the exact
//! counts of its sequence and then every helper that applies.
//!
//! Every frame here has [`LAYERS`] layers, and every buffer is in at most
//! one submission at a time (three buffers per layer rotate), so one
//! submission holds exactly [`LAYERS`] buffers.
//!
//! What the engine does not guarantee is not tested here.

#![forbid(unsafe_code)]

use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, Capabilities, DisplayEvent, DropReason, Engine,
    Error, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat, PlaneCaps,
    PlaneId, PlaneKind, Rect, Size, SubmissionId, Trace, TraceEvent, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const FULL: Rect = Rect::new(0, 0, 1280, 720);
const BOTTOM: PlaneId = PlaneId(31);
const TOP: PlaneId = PlaneId(41);
const LAYERS: usize = 2;

const APP: [BufferId; 3] = [BufferId(1), BufferId(2), BufferId(3)];
const SAFE: [BufferId; 3] = [BufferId(11), BufferId(12), BufferId(13)];

/// An owned copy of a [`TraceEvent`]: the output is always `OUT`, and check
/// and validate keep only their result.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Ev {
    Check(Result<(), BackendError>),
    Validate(Result<(), Error>),
    Commit(Result<SubmissionId, Error>),
    Latched(SubmissionId),
    Released(BufferId, Option<SubmissionId>),
    Dropped(SubmissionId, DropReason),
    LatchTimeout(SubmissionId),
    DeviceLost,
    CapabilitiesChanged,
}

#[derive(Default)]
struct Recorder(Vec<(u64, Ev)>);

impl Trace for Recorder {
    fn record(&mut self, at: u64, event: TraceEvent<'_>) {
        let ev = match event {
            TraceEvent::Check { result, .. } => Ev::Check(result),
            TraceEvent::Validate { result, .. } => Ev::Validate(result.map(|_| ())),
            TraceEvent::Commit { result, .. } => Ev::Commit(result),
            TraceEvent::Latched { submission, .. } => Ev::Latched(submission),
            TraceEvent::Released { buffer, submission } => Ev::Released(buffer, submission),
            TraceEvent::Dropped {
                submission, reason, ..
            } => Ev::Dropped(submission, reason),
            TraceEvent::LatchTimeout { submission, .. } => Ev::LatchTimeout(submission),
            TraceEvent::DeviceLost => Ev::DeviceLost,
            TraceEvent::CapabilitiesChanged => Ev::CapabilitiesChanged,
        };
        self.0.push((at, ev));
    }
}

// ---------------------------------------------------------------- scaffold

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

type Eng = Engine<FakeBackend, Recorder>;

fn traced(replace_pending: bool) -> Eng {
    let mut engine =
        Engine::new(FakeBackend::new(caps(replace_pending))).with_trace(Recorder::default());
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
    engine
}

/// Presents frame `n` of the rotation.
fn present(engine: &mut Eng, n: usize) -> Result<SubmissionId, Error> {
    let layers = [
        Layer::new(APP[n % 3], FULL, FULL, 0),
        Layer {
            blend: BlendMode::Premultiplied,
            ..Layer::new(SAFE[n % 3], FULL, FULL, 1)
        },
    ];
    engine.present(&Frame {
        output: OUT,
        layers: &layers,
    })
}

/// Polls every display event: backend events enter the trace when the
/// client polls (or at a commit), so a count is complete only after this.
fn drain(engine: &mut Eng) {
    while engine.poll_event().unwrap().is_some() {}
}

/// Presents frame `n`, takes a vertical blank, and polls.
fn present_and_latch(engine: &mut Eng, n: usize) -> SubmissionId {
    let s = present(engine, n).unwrap();
    engine.backend_mut().vsync();
    drain(engine);
    s
}

fn log(engine: &Eng) -> Vec<Ev> {
    engine.trace().0.iter().map(|(_, e)| e.clone()).collect()
}

// ------------------------------------------------------------------ counts

/// Event counts of a trace: the numbers a script can read off the lines.
#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    check: usize,
    validate_ok: usize,
    validate_err: usize,
    commit_ok: usize,
    commit_err: usize,
    latched: usize,
    released: usize,
    /// `Released` entries with no submission (a device loss).
    released_unattributed: usize,
    dropped_replaced: usize,
    dropped_lost: usize,
    latch_timeout: usize,
    device_lost: usize,
    caps_changed: usize,
}

fn counts(log: &[Ev]) -> Counts {
    let mut c = Counts::default();
    for ev in log {
        match ev {
            Ev::Check(_) => c.check += 1,
            Ev::Validate(Ok(())) => c.validate_ok += 1,
            Ev::Validate(Err(_)) => c.validate_err += 1,
            Ev::Commit(Ok(_)) => c.commit_ok += 1,
            Ev::Commit(Err(_)) => c.commit_err += 1,
            Ev::Latched(_) => c.latched += 1,
            Ev::Released(_, owner) => {
                c.released += 1;
                c.released_unattributed += usize::from(owner.is_none());
            }
            Ev::Dropped(_, DropReason::Replaced) => c.dropped_replaced += 1,
            Ev::Dropped(_, DropReason::DeviceLost) => c.dropped_lost += 1,
            Ev::LatchTimeout(_) => c.latch_timeout += 1,
            Ev::DeviceLost => c.device_lost += 1,
            Ev::CapabilitiesChanged => c.caps_changed += 1,
        }
    }
    c
}

// -------------------------------------------------------------- invariants

/// What the display still holds after a trace, found by replaying it.
#[derive(Debug, PartialEq, Eq)]
struct Held {
    /// Committed, neither latched nor dropped.
    pending: Vec<SubmissionId>,
    /// Latched and not yet taken off screen.
    on_screen: Option<SubmissionId>,
}

impl Held {
    fn submissions(&self) -> usize {
        self.pending.len() + usize::from(self.on_screen.is_some())
    }
}

/// INV-LIFECYCLE: replays the trace as a state machine, panics on the first
/// entry that does not fit, and returns what is still held.
///
/// * Submission ids of successful commits are strictly increasing; a refused
///   commit takes none.
/// * A submission ends at most once: `Latched` and `Dropped` are only of a
///   pending submission.
/// * A latch puts its submission on screen and takes the previous one off.
/// * `Released(_, Some(s))` follows the end of `s` (taken off screen, or
///   dropped as replaced), `layers` per submission.
/// * A `DeviceLost` entry leaves nothing pending and nothing on screen; the
///   buffers of everything lost were released unattributed
///   (`Released(_, None)`), `layers` per submission lost.
fn inv_lifecycle(log: &[Ev], layers: usize) -> Held {
    let mut last = 0;
    let mut pending: Vec<SubmissionId> = Vec::new();
    let mut on_screen = None;
    // Releases still owed per ended submission.
    let mut owed: Vec<(SubmissionId, usize)> = Vec::new();
    // Submissions dropped for a loss, and unattributed releases, since the
    // last `DeviceLost` entry.
    let mut lost_drops = 0;
    let mut unattributed = 0;
    for ev in log {
        match ev {
            Ev::Commit(Ok(s)) => {
                assert!(s.0 > last, "ids increase: {s:?} after {last}");
                last = s.0;
                pending.push(*s);
            }
            Ev::Latched(s) => {
                take(&mut pending, *s);
                if let Some(old) = on_screen.replace(*s) {
                    owed.push((old, layers));
                }
            }
            Ev::Dropped(s, DropReason::Replaced) => {
                take(&mut pending, *s);
                owed.push((*s, layers));
            }
            Ev::Dropped(s, DropReason::DeviceLost) => {
                take(&mut pending, *s);
                lost_drops += 1;
            }
            Ev::Released(b, Some(s)) => {
                let entry = owed.iter_mut().find(|(e, left)| e == s && *left > 0);
                let entry = entry.unwrap_or_else(|| panic!("release of {b:?} for {s:?}"));
                entry.1 -= 1;
            }
            Ev::Released(_, None) => unattributed += 1,
            Ev::DeviceLost => {
                assert!(pending.is_empty(), "pending survived a loss: {pending:?}");
                let lost = lost_drops + usize::from(on_screen.is_some());
                assert_eq!(unattributed, layers * lost, "releases of a loss");
                (on_screen, lost_drops, unattributed) = (None, 0, 0);
            }
            _ => {}
        }
    }
    assert!(
        owed.iter().all(|(_, left)| *left == 0),
        "ended submissions with unreleased buffers: {owed:?}"
    );
    Held { pending, on_screen }
}

fn take(pending: &mut Vec<SubmissionId>, s: SubmissionId) {
    let at = pending.iter().position(|p| *p == s);
    pending.remove(at.unwrap_or_else(|| panic!("{s:?} is not pending")));
}

/// INV-RESOLVED: every successful commit is latched, dropped, or still
/// pending, and never two of these:
/// `commit_ok == latched + dropped_replaced + dropped_lost + pending`.
/// With nothing pending (after a vertical blank, or after a loss) the last
/// term is 0.
fn inv_resolved(c: &Counts, pending: usize) {
    assert_eq!(
        c.commit_ok,
        c.latched + c.dropped_replaced + c.dropped_lost + pending,
        "{c:?}"
    );
}

/// INV-RELEASE-BALANCE: buffers are released exactly when their submission
/// ends, so `released + layers * held == layers * commit_ok`, where `held`
/// counts the pending submissions plus the one on screen. A device loss ends
/// every submission at once, so `held` is 0 after one.
fn inv_release_balance(c: &Counts, layers: usize, held: &Held) {
    assert_eq!(
        c.released + layers * held.submissions(),
        layers * c.commit_ok,
        "{c:?} {held:?}"
    );
}

/// INV-PIPELINE: every `Commit` entry follows a `Validate` that planned, so
/// `commit_ok + commit_err <= validate_ok`. (Not every `Validate` has a
/// `Check` before it: a frame the planner refuses is never shown to the
/// backend.)
fn inv_pipeline(c: &Counts) {
    assert!(c.commit_ok + c.commit_err <= c.validate_ok, "{c:?}");
}

/// INV-NOTHING-HELD-AFTER-LOSS: when the trace ends in a loss (and no frame
/// was committed since), nothing is held, and exactly
/// `layers * held_at_loss` releases were unattributed, where `held_at_loss`
/// is the submissions (pending plus on screen) the display held when it was
/// lost.
fn inv_nothing_held_after_loss(log: &[Ev], layers: usize, held_at_loss: usize) {
    let held = inv_lifecycle(log, layers);
    assert_eq!(
        held,
        Held {
            pending: vec![],
            on_screen: None
        }
    );
    let c = counts(log);
    assert_eq!(c.released_unattributed, layers * held_at_loss, "{c:?}");
}

// ------------------------------------------------------------------- tests

#[test]
fn steady_hold_of_n_frames() {
    const N: usize = 40;
    let mut engine = traced(false);
    for n in 0..N {
        engine.advance_time(16 * (n as u64 + 1));
        assert_eq!(
            present_and_latch(&mut engine, n),
            SubmissionId(n as u64 + 1)
        );
    }
    let trace = log(&engine);
    let c = counts(&trace);
    assert_eq!(
        c,
        Counts {
            check: N,
            validate_ok: N,
            commit_ok: N,
            latched: N,
            released: LAYERS * (N - 1),
            ..Counts::default()
        }
    );
    let held = inv_lifecycle(&trace, LAYERS);
    assert_eq!(held.on_screen, Some(SubmissionId(N as u64)));
    assert!(held.pending.is_empty());
    inv_resolved(&c, 0);
    inv_release_balance(&c, LAYERS, &held);
    inv_pipeline(&c);
}

#[test]
fn idle_hold_adds_no_entries() {
    let mut engine = traced(false);
    present_and_latch(&mut engine, 0);
    let before = engine.trace().0.len();
    for now in 1..=100 {
        engine.advance_time(now * 16);
        drain(&mut engine);
    }
    assert_eq!(engine.trace().0.len(), before);
}

#[test]
fn hold_with_a_vsync_every_other_frame_drops_the_replaced_ones() {
    const N: usize = 20;
    let mut engine = traced(true);
    for n in 0..N {
        present(&mut engine, n).unwrap();
        if n % 2 == 1 {
            engine.backend_mut().vsync();
        }
        drain(&mut engine);
    }
    let trace = log(&engine);
    let c = counts(&trace);
    assert_eq!(
        (c.commit_ok, c.latched, c.dropped_replaced),
        (N, N / 2, N / 2)
    );
    let held = inv_lifecycle(&trace, LAYERS);
    inv_resolved(&c, 0);
    inv_release_balance(&c, LAYERS, &held);
    // Replaced buffers are attributed to the dropped submission.
    assert_eq!(c.released_unattributed, 0);
    assert_eq!(c.released, LAYERS * (N - 1));
}

#[test]
fn stop_keeps_the_last_frames_held_and_releases_nothing_more() {
    let mut engine = traced(false);
    for n in 0..5 {
        present_and_latch(&mut engine, n);
    }
    // The application stops with a frame pending: no more presents.
    let pending = present(&mut engine, 5).unwrap();
    let before = counts(&log(&engine));
    engine.advance_time(10_000);
    drain(&mut engine);
    let trace = log(&engine);
    let c = counts(&trace);
    assert_eq!(c, before, "polling and time alone change no count");
    let held = inv_lifecycle(&trace, LAYERS);
    assert_eq!(held.pending, [pending]);
    assert_eq!(held.on_screen, Some(SubmissionId(5)));
    inv_resolved(&c, 1);
    inv_release_balance(&c, LAYERS, &held);

    // The engine has no teardown: the buffers of the held frames cannot be
    // unregistered while the display holds them.
    assert!(engine.unregister_buffer(APP[4 % 3]).is_err());
    assert!(engine.unregister_buffer(APP[5 % 3]).is_err());

    // The last vertical blank latches the pending frame and releases the
    // one it replaces; one submission stays held.
    engine.backend_mut().vsync();
    drain(&mut engine);
    let trace = log(&engine);
    let c = counts(&trace);
    let held = inv_lifecycle(&trace, LAYERS);
    assert_eq!((c.commit_ok, c.latched, c.released), (6, 6, LAYERS * 5));
    assert_eq!(held.submissions(), 1);
    inv_resolved(&c, 0);
    inv_release_balance(&c, LAYERS, &held);
}

#[test]
fn teardown_by_device_loss_releases_everything() {
    let mut engine = traced(false);
    for n in 0..4 {
        present_and_latch(&mut engine, n);
    }
    present(&mut engine, 4).unwrap(); // pending
    engine.backend_mut().lose_device();
    drain(&mut engine);
    let trace = log(&engine);
    let c = counts(&trace);
    assert_eq!(
        c,
        Counts {
            check: 5,
            validate_ok: 5,
            commit_ok: 5,
            latched: 4,
            released: LAYERS * 5,
            released_unattributed: LAYERS * 2,
            dropped_lost: 1,
            device_lost: 1,
            ..Counts::default()
        }
    );
    inv_nothing_held_after_loss(&trace, LAYERS, 2);
    inv_resolved(&c, 0);
    // Every buffer of every committed frame has been released.
    assert_eq!(c.released, LAYERS * c.commit_ok);
}

#[test]
fn refused_commits_take_no_submission_and_hold_nothing() {
    let mut engine = traced(false);
    present_and_latch(&mut engine, 0);
    engine.backend_mut().fail_next_commits(3);
    for n in 1..=3 {
        assert_eq!(
            present(&mut engine, n),
            Err(Error::Backend(BackendError::Platform(-5)))
        );
    }
    // The next present gets the next id, as if the failures never happened.
    assert_eq!(present_and_latch(&mut engine, 4), SubmissionId(2));
    let trace = log(&engine);
    let c = counts(&trace);
    assert_eq!(
        c,
        Counts {
            check: 5,
            validate_ok: 5,
            commit_ok: 2,
            commit_err: 3,
            latched: 2,
            released: LAYERS,
            ..Counts::default()
        }
    );
    let held = inv_lifecycle(&trace, LAYERS);
    inv_resolved(&c, 0);
    inv_release_balance(&c, LAYERS, &held);
    inv_pipeline(&c);
}

#[test]
fn lost_output_reports_once_fails_cleanly_and_a_new_run_latches() {
    let mut engine = traced(false);
    for n in 0..3 {
        present_and_latch(&mut engine, n);
    }
    let pending = present(&mut engine, 3).unwrap();
    engine.backend_mut().lose_device();
    drain(&mut engine);
    let after_loss = log(&engine);
    assert_eq!(counts(&after_loss).device_lost, 1);
    inv_nothing_held_after_loss(&after_loss, LAYERS, 2);
    assert!(after_loss.contains(&Ev::Dropped(pending, DropReason::DeviceLost)));

    // Presenting into the loss fails before the backend's commit: a refused
    // validation, no Commit entry, no submission, nothing held.
    for n in 4..7 {
        assert_eq!(
            present(&mut engine, n),
            Err(Error::Backend(BackendError::DeviceLost))
        );
    }
    let c = counts(&log(&engine));
    assert_eq!((c.commit_ok, c.commit_err), (4, 0));
    assert_eq!((c.check, c.validate_err), (4 + 3, 3));

    // A new run: restore, then frames latch and the relations hold again.
    // Submission ids go on from the old run.
    engine.backend_mut().restore(caps(false));
    drain(&mut engine);
    for n in 0..4 {
        present_and_latch(&mut engine, n);
    }
    let trace = log(&engine);
    let c = counts(&trace);
    assert_eq!(
        (c.commit_ok, c.latched, c.dropped_lost, c.device_lost),
        (8, 7, 1, 1)
    );
    assert_eq!(c.caps_changed, 1);
    let held = inv_lifecycle(&trace, LAYERS);
    assert_eq!(held.on_screen, Some(SubmissionId(8)));
    inv_resolved(&c, 0);
    inv_release_balance(&c, LAYERS, &held);
}

#[test]
fn repeated_loss_and_recovery_balance_every_cycle() {
    let mut engine = traced(false);
    for cycle in 1..=5 {
        present_and_latch(&mut engine, 0);
        present(&mut engine, 1).unwrap();
        engine.backend_mut().lose_device();
        drain(&mut engine);
        engine.backend_mut().restore(caps(false));
        drain(&mut engine);
        let trace = log(&engine);
        let c = counts(&trace);
        assert_eq!(
            (c.commit_ok, c.latched, c.dropped_lost, c.device_lost),
            (2 * cycle, cycle, cycle, cycle)
        );
        assert_eq!(c.released, LAYERS * c.commit_ok, "cycle {cycle}");
        assert_eq!(inv_lifecycle(&trace, LAYERS).submissions(), 0);
        inv_resolved(&c, 0);
    }
}

#[test]
fn drop_reason_replaced() {
    let mut engine = traced(true);
    present_and_latch(&mut engine, 0);
    let s2 = present(&mut engine, 1).unwrap();
    let s3 = present(&mut engine, 2).unwrap();
    engine.backend_mut().vsync();
    drain(&mut engine);
    let trace = log(&engine);
    let c = counts(&trace);
    assert_eq!(
        (c.commit_ok, c.latched, c.dropped_replaced, c.dropped_lost),
        (3, 2, 1, 0)
    );
    assert!(trace.contains(&Ev::Dropped(s2, DropReason::Replaced)));
    assert!(trace.contains(&Ev::Latched(s3)));
    for b in [APP[1], SAFE[1]] {
        assert!(trace.contains(&Ev::Released(b, Some(s2))));
    }
    let held = inv_lifecycle(&trace, LAYERS);
    inv_resolved(&c, 0);
    inv_release_balance(&c, LAYERS, &held);
}

#[test]
fn drop_reason_replaced_after_a_latch_timeout() {
    let mut engine = traced(true);
    engine.set_latch_timeout(Some(20));
    engine.advance_time(100);
    let s1 = present(&mut engine, 0).unwrap();
    for now in [110, 120, 130, 140] {
        engine.advance_time(now);
    }
    drain(&mut engine);
    // Reported once; the submission stays pending.
    let c = counts(&log(&engine));
    assert_eq!((c.latch_timeout, c.latched, c.dropped_replaced), (1, 0, 0));
    inv_resolved(&c, 1);

    let s2 = present(&mut engine, 1).unwrap();
    engine.backend_mut().vsync();
    drain(&mut engine);
    let trace = log(&engine);
    let c = counts(&trace);
    assert_eq!((c.latch_timeout, c.latched, c.dropped_replaced), (1, 1, 1));
    assert!(trace.contains(&Ev::Dropped(s1, DropReason::Replaced)));
    assert!(trace.contains(&Ev::Latched(s2)));
    let held = inv_lifecycle(&trace, LAYERS);
    inv_resolved(&c, 0);
    inv_release_balance(&c, LAYERS, &held);
}

#[test]
fn drop_reason_device_lost() {
    let mut engine = traced(true);
    present_and_latch(&mut engine, 0);
    let s2 = present(&mut engine, 1).unwrap();
    engine.backend_mut().lose_device();
    drain(&mut engine);
    let trace = log(&engine);
    let c = counts(&trace);
    assert_eq!(
        (c.dropped_lost, c.dropped_replaced, c.device_lost),
        (1, 0, 1)
    );
    assert!(trace.contains(&Ev::Dropped(s2, DropReason::DeviceLost)));
    inv_nothing_held_after_loss(&trace, LAYERS, 2);
    inv_resolved(&c, 0);
    assert_eq!(c.released, LAYERS * c.commit_ok);
}

#[test]
fn a_latch_the_client_has_not_polled_is_still_traced_before_the_loss() {
    let mut engine = traced(false);
    present(&mut engine, 0).unwrap();
    engine.backend_mut().vsync();
    // Not polled: the latch and the loss are both queued in the backend.
    engine.backend_mut().lose_device();
    drain(&mut engine);
    let trace = log(&engine);
    let c = counts(&trace);
    assert_eq!(
        (c.commit_ok, c.latched, c.dropped_lost, c.device_lost),
        (1, 1, 0, 1)
    );
    inv_nothing_held_after_loss(&trace, LAYERS, 1);
    inv_resolved(&c, 0);
}

#[test]
fn capability_changes_do_not_end_submissions() {
    let mut engine = traced(false);
    present_and_latch(&mut engine, 0);
    present(&mut engine, 1).unwrap();
    engine.backend_mut().set_capabilities(caps(false));
    drain(&mut engine);
    let trace = log(&engine);
    let c = counts(&trace);
    assert_eq!(
        (c.caps_changed, c.dropped_replaced, c.dropped_lost),
        (1, 0, 0)
    );
    let held = inv_lifecycle(&trace, LAYERS);
    assert_eq!(held.submissions(), 2);
    inv_resolved(&c, 1);
    inv_release_balance(&c, LAYERS, &held);
}

#[test]
fn display_events_and_trace_agree() {
    // What the client polls and what the trace counts are the same things.
    let mut engine = traced(true);
    let (mut presented, mut dropped, mut released, mut lost) = (0, 0, 0, 0);
    let mut tally = |engine: &mut Eng| {
        while let Some(ev) = engine.poll_event().unwrap() {
            match ev {
                DisplayEvent::Presented { .. } => presented += 1,
                DisplayEvent::Dropped { .. } => dropped += 1,
                DisplayEvent::Released { .. } => released += 1,
                DisplayEvent::DeviceLost => lost += 1,
                _ => {}
            }
        }
    };
    for n in 0..9 {
        present(&mut engine, n).unwrap();
        if n % 3 == 2 {
            engine.backend_mut().vsync();
        }
        tally(&mut engine);
    }
    present(&mut engine, 9).unwrap();
    engine.backend_mut().lose_device();
    tally(&mut engine);
    let c = counts(&log(&engine));
    assert_eq!(presented, c.latched);
    assert_eq!(dropped, c.dropped_replaced + c.dropped_lost);
    assert_eq!(released, c.released);
    assert_eq!(lost, c.device_lost);
}
