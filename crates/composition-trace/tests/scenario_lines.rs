//! The board scenarios' sequences, driven through the fake engine, written
//! as CETRACE lines by [`LineTrace`], parsed back with the strict parser,
//! and checked with the count relations of
//! `crates/composition-fake/tests/trace_counts.rs` (`inv_resolved`,
//! `inv_release_balance`, `inv_pipeline`, `inv_lifecycle`).
//!
//! This is what a script that only reads the log lines may rely on. It says
//! nothing about hardware: the fake backend is the only source of events.
//!
//! Sequences, one test each (plus the negative tests below):
//!
//! * `hold`: steady commits, each latched by a vertical blank;
//! * `stop`: commits, then the client stops with a submission pending and
//!   buffers held (the engine has no teardown);
//! * `revoke`: the device is lost, so every buffer is released;
//! * `modeset`: the capabilities change under a running engine (the fake's
//!   only way to model a mode switch), and committing goes on;
//! * `recovery`: frames are refused after a loss, then a restored backend
//!   latches again.
//!
//! The checks return `Err` instead of panicking so that the negative tests
//! can feed them damaged logs: one line removed (a `seq` gap), and one line
//! removed with the later `seq` renumbered (only the counts can tell).
//! Only public APIs are used.

use std::cell::Cell;

use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, Capabilities, DropReason, Engine, Error,
    FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat, PlaneCaps, PlaneId,
    PlaneKind, Rect, Size, SubmissionId, Zpos,
};
use composition_fake::FakeBackend;
use composition_trace::{parse_line, Clock, Event, Header, LineTrace, Record, Source};

const OUT: OutputId = OutputId(0);
const FULL: Rect = Rect::new(0, 0, 1280, 720);
const LAYERS: usize = 2;
const APP: [BufferId; 3] = [BufferId(1), BufferId(2), BufferId(3)];
const SAFE: [BufferId; 3] = [BufferId(11), BufferId(12), BufferId(13)];

const HEADER: Header<'static> = Header {
    src: Source::Linux,
    clock: Clock::MonotonicNs,
    freq_hz: 1_000_000_000,
    t0: 0,
    at_unit: "same",
    build: "test",
    board: "fake",
};

// ---------------------------------------------------------------- scaffold

fn plane(id: u32, kind: PlaneKind, zpos: i32, format: PixelFormat) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs: 1,
        zpos: Zpos::Fixed(zpos),
        blend_modes: BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask(),
        ..PlaneCaps::default()
    };
    caps.formats.push(FormatModifier::linear(format)).unwrap();
    caps
}

fn caps(replace_pending: bool, refresh_mhz: u32) -> Capabilities {
    let mut caps = Capabilities {
        replace_pending,
        ..Capabilities::default()
    };
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(1280, 720),
            refresh_mhz,
        })
        .unwrap();
    let bottom = plane(31, PlaneKind::Primary, 0, PixelFormat::RGB565);
    caps.planes.push(bottom).unwrap();
    let top = plane(41, PlaneKind::Overlay, 1, PixelFormat::ARGB8888);
    caps.planes.push(top).unwrap();
    caps
}

thread_local! {
    /// Fake clock of the sink: 10 ticks per line.
    static NOW: Cell<u64> = const { Cell::new(0) };
}

fn tick() -> u64 {
    NOW.with(|n| {
        n.set(n.get() + 10);
        n.get()
    })
}

type Eng = Engine<FakeBackend, LineTrace<String, fn() -> u64>>;

fn engine(replace_pending: bool) -> Eng {
    NOW.with(|n| n.set(0));
    let mut sink = LineTrace::new(String::new(), tick as fn() -> u64);
    sink.header(&HEADER).unwrap();
    let mut engine = Engine::new(FakeBackend::new(caps(replace_pending, 60_000))).with_trace(sink);
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

fn drain(engine: &mut Eng) {
    while engine.poll_event().unwrap().is_some() {}
}

fn present_and_latch(engine: &mut Eng, n: usize) -> SubmissionId {
    let s = present(engine, n).unwrap();
    engine.backend_mut().vsync();
    drain(engine);
    s
}

/// A finished run: its log and what the engine's backend still held.
struct Run {
    log: String,
    /// The backend's pending submission.
    pending: Option<SubmissionId>,
    /// Whether the backend scans a frame out.
    on_screen: bool,
}

fn finish(engine: Eng) -> Run {
    let pending = engine.backend().pending(OUT);
    let on_screen = engine.backend().scanout(OUT).is_some();
    let sink = engine.trace();
    assert_eq!(sink.failed(), 0);
    Run {
        log: sink.writer().clone(),
        pending,
        on_screen,
    }
}

// ----------------------------------------------------------------- the runs

fn hold() -> Run {
    let mut e = engine(false);
    for n in 0..30 {
        e.advance_time(16 * (n as u64 + 1));
        assert_eq!(present_and_latch(&mut e, n), SubmissionId(n as u64 + 1));
    }
    finish(e)
}

fn stop() -> Run {
    let mut e = engine(false);
    for n in 0..5 {
        present_and_latch(&mut e, n);
    }
    // The client stops with a frame pending: the buffers stay held.
    present(&mut e, 5).unwrap();
    e.advance_time(10_000);
    drain(&mut e);
    assert!(e.unregister_buffer(APP[4 % 3]).is_err());
    assert!(e.unregister_buffer(APP[5 % 3]).is_err());
    finish(e)
}

fn revoke() -> Run {
    let mut e = engine(false);
    for n in 0..4 {
        present_and_latch(&mut e, n);
    }
    present(&mut e, 4).unwrap();
    e.backend_mut().lose_device();
    drain(&mut e);
    finish(e)
}

// `modeset` is driven as a capability change: the fake has no mode object,
// only `set_capabilities`, so a new refresh rate stands for the new mode.
// With a frame pending across the change, replacing it needs
// `replace_pending`.
fn modeset() -> Run {
    let mut e = engine(true);
    for n in 0..3 {
        present_and_latch(&mut e, n);
    }
    present(&mut e, 3).unwrap();
    e.backend_mut().set_capabilities(caps(true, 30_000));
    drain(&mut e);
    assert_eq!(e.capabilities().outputs[0].refresh_mhz, 30_000);
    // Keep committing under the new capabilities: the first frame replaces
    // the pending one. (A buffer must not be in two submissions at once,
    // or its release waits for both.)
    for n in 4..9 {
        present_and_latch(&mut e, n);
    }
    finish(e)
}

fn recovery() -> Run {
    let mut e = engine(false);
    for n in 0..3 {
        present_and_latch(&mut e, n);
    }
    present(&mut e, 3).unwrap();
    e.backend_mut().lose_device();
    drain(&mut e);
    // Refused after the loss: no commit, no submission.
    for n in 4..7 {
        assert_eq!(
            present(&mut e, n),
            Err(Error::Backend(BackendError::DeviceLost))
        );
    }
    // A new run latches; ids go on from the old run.
    e.backend_mut().restore(caps(false, 60_000));
    drain(&mut e);
    for n in 0..4 {
        present_and_latch(&mut e, n);
    }
    finish(e)
}

// ------------------------------------------------------------------ checks

macro_rules! ensure {
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            return Err(format!($($arg)+));
        }
    };
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    check: usize,
    validate_ok: usize,
    validate_err: usize,
    commit_ok: usize,
    commit_err: usize,
    latched: usize,
    released: usize,
    released_unattributed: usize,
    dropped_replaced: usize,
    dropped_lost: usize,
    latch_timeout: usize,
    device_lost: usize,
    caps_changed: usize,
}

fn counts(events: &[Event]) -> Counts {
    let mut c = Counts::default();
    for ev in events {
        match ev {
            Event::Check { .. } => c.check += 1,
            Event::Validate { result: Ok(_), .. } => c.validate_ok += 1,
            Event::Validate { result: Err(_), .. } => c.validate_err += 1,
            Event::Commit { result: Ok(_), .. } => c.commit_ok += 1,
            Event::Commit { result: Err(_), .. } => c.commit_err += 1,
            Event::Latched { .. } => c.latched += 1,
            Event::Released { submission, .. } => {
                c.released += 1;
                c.released_unattributed += usize::from(submission.is_none());
            }
            Event::Dropped {
                reason: DropReason::Replaced,
                ..
            } => c.dropped_replaced += 1,
            Event::Dropped {
                reason: DropReason::DeviceLost,
                ..
            } => c.dropped_lost += 1,
            Event::LatchTimeout { .. } => c.latch_timeout += 1,
            Event::DeviceLost => c.device_lost += 1,
            Event::CapabilitiesChanged => c.caps_changed += 1,
        }
    }
    c
}

/// Parses every line: the first is the header, `seq` counts lines with no
/// gap, and `t` never goes back.
fn parse_all(log: &str) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    let mut last_t = 0;
    for (i, line) in log.lines().enumerate() {
        let record = parse_line(line)
            .map_err(|e| format!("line {i} {line:?}: {e}"))?
            .ok_or_else(|| format!("line {i} is blank"))?;
        match record {
            Record::Header { seq, header } => {
                ensure!(i == 0, "header at line {i}");
                ensure!(seq == 0 && header == HEADER, "header {seq} {header:?}");
            }
            Record::Event { stamp, event } => {
                ensure!(i > 0, "no header first");
                ensure!(stamp.seq == i as u64, "seq gap: {} at line {i}", stamp.seq);
                ensure!(stamp.t >= last_t, "t went back at line {i}");
                last_t = stamp.t;
                events.push(event);
            }
        }
    }
    ensure!(!events.is_empty(), "no events");
    Ok(events)
}

/// Replays the events as `inv_lifecycle` does and returns the submissions
/// still pending and the one on screen.
fn lifecycle(events: &[Event]) -> Result<(Vec<SubmissionId>, Option<SubmissionId>), String> {
    let mut last = 0;
    let mut pending: Vec<SubmissionId> = Vec::new();
    let mut on_screen = None;
    let mut owed: Vec<(SubmissionId, usize)> = Vec::new();
    let (mut lost_drops, mut unattributed) = (0, 0);
    let take = |pending: &mut Vec<SubmissionId>, s: SubmissionId| match pending
        .iter()
        .position(|p| *p == s)
    {
        Some(at) => {
            pending.remove(at);
            Ok(())
        }
        None => Err(format!("{s:?} is not pending")),
    };
    for ev in events {
        match ev {
            Event::Commit { result: Ok(s), .. } => {
                ensure!(s.0 > last, "ids increase: {s:?} after {last}");
                last = s.0;
                pending.push(*s);
            }
            Event::Latched { submission, .. } => {
                take(&mut pending, *submission)?;
                if let Some(old) = on_screen.replace(*submission) {
                    owed.push((old, LAYERS));
                }
            }
            Event::Dropped {
                submission, reason, ..
            } => {
                take(&mut pending, *submission)?;
                match reason {
                    DropReason::Replaced => owed.push((*submission, LAYERS)),
                    DropReason::DeviceLost => lost_drops += 1,
                }
            }
            Event::Released {
                buffer,
                submission: Some(s),
            } => {
                let entry = owed.iter_mut().find(|(e, left)| e == s && *left > 0);
                let Some(entry) = entry else {
                    return Err(format!("release of {buffer:?} for {s:?}"));
                };
                entry.1 -= 1;
            }
            Event::Released {
                submission: None, ..
            } => unattributed += 1,
            Event::DeviceLost => {
                ensure!(pending.is_empty(), "pending survived a loss: {pending:?}");
                let lost = lost_drops + usize::from(on_screen.is_some());
                ensure!(
                    unattributed == LAYERS * lost,
                    "releases of a loss: {unattributed} for {lost} submissions"
                );
                (on_screen, lost_drops, unattributed) = (None, 0, 0);
            }
            _ => {}
        }
    }
    ensure!(
        owed.iter().all(|(_, left)| *left == 0),
        "ended submissions with unreleased buffers: {owed:?}"
    );
    Ok((pending, on_screen))
}

/// Every relation of `trace_counts.rs` on a parsed log, and the end state
/// against the engine's. Returns the counts.
fn check(run: &Run, log: &str) -> Result<Counts, String> {
    let events = parse_all(log)?;
    let c = counts(&events);
    let (pending, on_screen) = lifecycle(&events)?;
    // INV-RESOLVED
    ensure!(
        c.commit_ok == c.latched + c.dropped_replaced + c.dropped_lost + pending.len(),
        "unresolved commits: {c:?} pending {pending:?}"
    );
    // INV-RELEASE-BALANCE
    let held = pending.len() + usize::from(on_screen.is_some());
    ensure!(
        c.released + LAYERS * held == LAYERS * c.commit_ok,
        "release balance: {c:?} held {held}"
    );
    // INV-PIPELINE
    ensure!(
        c.commit_ok + c.commit_err <= c.validate_ok,
        "commits without a plan: {c:?}"
    );
    // The end state is the engine's.
    ensure!(
        pending.first().copied() == run.pending && pending.len() <= 1,
        "pending {pending:?}, engine {:?}",
        run.pending
    );
    ensure!(
        on_screen.is_some() == run.on_screen,
        "on screen {on_screen:?}, engine {}",
        run.on_screen
    );
    Ok(c)
}

fn check_run(run: &Run) -> Counts {
    check(run, &run.log).unwrap_or_else(|e| panic!("{e}\n{}", run.log))
}

// ------------------------------------------------------------------- tests

#[test]
fn hold_lines_parse_and_count() {
    let run = hold();
    let c = check_run(&run);
    assert_eq!(
        c,
        Counts {
            check: 30,
            validate_ok: 30,
            commit_ok: 30,
            latched: 30,
            released: LAYERS * 29,
            ..Counts::default()
        }
    );
    assert_eq!((run.pending, run.on_screen), (None, true));
}

#[test]
fn stop_lines_parse_and_count() {
    let run = stop();
    let c = check_run(&run);
    assert_eq!((c.commit_ok, c.latched, c.released), (6, 5, LAYERS * 4));
    // A pending frame and a frame on screen are held at the end.
    assert_eq!((run.pending, run.on_screen), (Some(SubmissionId(6)), true));
}

#[test]
fn revoke_lines_parse_and_count() {
    let run = revoke();
    let c = check_run(&run);
    assert_eq!(
        (c.commit_ok, c.latched, c.dropped_lost, c.device_lost),
        (5, 4, 1, 1)
    );
    // Every buffer is released, two submissions of it with no owner.
    assert_eq!(c.released, LAYERS * c.commit_ok);
    assert_eq!(c.released_unattributed, LAYERS * 2);
    assert_eq!((run.pending, run.on_screen), (None, false));
}

#[test]
fn modeset_lines_parse_and_count() {
    let run = modeset();
    let c = check_run(&run);
    assert_eq!(c.caps_changed, 1);
    assert_eq!((c.commit_ok, c.dropped_replaced, c.dropped_lost), (9, 1, 0));
    assert_eq!(c.latched, 8);
    assert_eq!((run.pending, run.on_screen), (None, true));
}

#[test]
fn recovery_lines_parse_and_count() {
    let run = recovery();
    let c = check_run(&run);
    assert_eq!(
        (c.commit_ok, c.commit_err, c.latched, c.dropped_lost),
        (8, 0, 7, 1)
    );
    // The refused frames were planned and refused by validation.
    assert_eq!((c.check, c.validate_err), (4 + 3 + 4, 3));
    assert_eq!((c.device_lost, c.caps_changed), (1, 1));
    assert_eq!((run.pending, run.on_screen), (None, true));
}

// ---------------------------------------------------------------- negative

type Sequence = (&'static str, fn() -> Run);

const RUNS: [Sequence; 5] = [
    ("hold", hold),
    ("stop", stop),
    ("revoke", revoke),
    ("modeset", modeset),
    ("recovery", recovery),
];

/// The log without line `drop`.
fn without_line(log: &str, drop: usize) -> String {
    log.lines()
        .enumerate()
        .filter(|(i, _)| *i != drop)
        .map(|(_, l)| format!("{l}\n"))
        .collect()
}

/// The log without line `drop`, the `seq` of the later lines lowered by
/// one: the gap is hidden and only the counts can show the loss.
fn without_line_renumbered(log: &str, drop: usize) -> String {
    let mut out = String::new();
    for (i, line) in log.lines().enumerate() {
        if i == drop {
            continue;
        }
        if i > drop {
            let fields: Vec<String> = line
                .split(' ')
                .map(|f| match f.strip_prefix("seq=") {
                    Some(n) => format!("seq={}", n.parse::<u64>().unwrap() - 1),
                    None => f.to_string(),
                })
                .collect();
            out.push_str(&fields.join(" "));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Index of the first line of event `ev` (`ev=<name>`) in `log`.
fn first_line_of(log: &str, ev: &str) -> usize {
    let key = format!(" ev={ev} ");
    log.lines()
        .position(|l| l.contains(&key))
        .unwrap_or_else(|| panic!("no {ev} line"))
}

#[test]
fn a_dropped_line_is_a_seq_gap_in_every_sequence() {
    for (name, run) in RUNS {
        let run = run();
        for target in ["commit", "latched", "validate"] {
            // A sequence may lack a kind (none lacks these three).
            let at = first_line_of(&run.log, target);
            let log = without_line(&run.log, at);
            let err = check(&run, &log).expect_err(name);
            assert!(err.contains("seq gap"), "{name} {target}: {err}");
        }
    }
}

#[test]
fn a_dropped_line_with_seq_renumbered_is_a_wrong_count_in_every_sequence() {
    for (name, run) in RUNS {
        let run = run();
        // No released line: nothing of the buffers is lost, but a commit
        // line is, so the commit/resolve relation breaks instead.
        for (target, expect) in [("released", "release"), ("latched", ""), ("commit", "")] {
            let at = first_line_of(&run.log, target);
            let log = without_line_renumbered(&run.log, at);
            parse_all(&log).unwrap_or_else(|e| panic!("{name} {target}: still parses: {e}"));
            let err = check(&run, &log).expect_err(name);
            assert!(err.contains(expect), "{name} {target}: {err}");
            assert!(!err.contains("seq gap"), "{name} {target}: {err}");
        }
    }
}

#[test]
fn a_wrong_end_state_is_detected_in_every_sequence() {
    for (name, run) in RUNS {
        let mut run = run();
        run.on_screen = !run.on_screen;
        let log = run.log.clone();
        assert!(check(&run, &log).is_err(), "{name}: on screen");

        let mut run = RUNS.iter().find(|(n, _)| *n == name).unwrap().1();
        run.pending = match run.pending {
            Some(_) => None,
            None => Some(SubmissionId(99)),
        };
        let log = run.log.clone();
        assert!(check(&run, &log).is_err(), "{name}: pending");
    }
}

#[test]
fn a_changed_field_is_detected() {
    // A latched line for a submission that was never committed.
    let run = hold();
    let at = first_line_of(&run.log, "latched");
    let log: String = run
        .log
        .lines()
        .enumerate()
        .map(|(i, l)| {
            if i == at {
                format!("{}\n", l.replacen("sub=1", "sub=77", 1))
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    let err = check(&run, &log).expect_err("changed sub");
    assert!(err.contains("not pending"), "{err}");

    // A log cut in the middle of a line does not parse.
    let cut = &run.log[..run.log.len() - 4];
    assert!(parse_all(cut).is_err());
}
