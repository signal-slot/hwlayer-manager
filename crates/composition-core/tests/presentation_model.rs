//! Model-based tests of the presentation state machine (SoW §10).
//!
//! `Presentation` is driven directly, without engine or backend, by
//! seeded pseudo-random operation sequences. A small reference model
//! (plain `Vec`s) is stepped in parallel; after every step buffer states,
//! `on_screen`, `has_pending`, `next_submission_id`, `events_room` and every
//! polled event are compared. A failing sequence prints its seed and all
//! steps up to the failure. Re-run one with `PRESENTATION_MODEL_SEED=<n>`
//! (or a range, `<first>-<last>`).
//!
//! The generator is hand-written: `composition-core` has no dependencies.

use composition_core::limits::{MAX_BUFFERS, MAX_EVENTS, MAX_LAYERS, MAX_OUTPUTS};
use composition_core::{
    BufferId, BufferInfo, BufferState, DisplayEvent, OutputId, PresentError, Presentation,
    SubmissionId,
};
use std::collections::VecDeque;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU32, Ordering};

/// How often each interesting outcome was reached, to prove that the
/// generator gets there.
static HITS: [AtomicU32; 9] = [const { AtomicU32::new(0) }; 9];
const HIT_NAMES: [&str; 9] = [
    "submit EventQueueFull",
    "latched EventQueueFull",
    "reset EventQueueFull",
    "TooManyOutputs",
    "TooManyBuffers",
    "BufferTableFull",
    "UnexpectedLatch",
    "replaced pending",
    "LatchTimeout",
];

fn hit(i: usize) {
    HITS[i].fetch_add(1, Ordering::Relaxed);
}

// ---------------------------------------------------------------- generator

/// splitmix64: tiny, deterministic, good enough for test generation.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

// ------------------------------------------------------------------- model

#[derive(Clone, Debug)]
struct Sub {
    id: u64,
    bufs: Vec<u32>,
    at: u64,
    overdue: bool,
}

#[derive(Clone, Debug)]
struct Slot {
    output: u32,
    pending: Option<Sub>,
    current: Option<Sub>,
}

#[derive(Clone, Debug)]
struct Model {
    /// (buffer, writable)
    bufs: Vec<(u32, bool)>,
    /// In order of first submission since the last reset.
    slots: Vec<Slot>,
    events: VecDeque<DisplayEvent>,
    next_id: u64,
    now: u64,
}

fn distinct(bufs: &[u32]) -> Vec<u32> {
    let mut d = Vec::new();
    for b in bufs {
        if !d.contains(b) {
            d.push(*b);
        }
    }
    d
}

impl Model {
    fn new() -> Self {
        Model {
            bufs: Vec::new(),
            slots: Vec::new(),
            events: VecDeque::new(),
            next_id: 1,
            now: 0,
        }
    }

    fn room(&self) -> usize {
        MAX_EVENTS - self.events.len()
    }

    fn emit(&mut self, e: DisplayEvent) {
        assert!(self.events.len() < MAX_EVENTS, "model: event overflow");
        self.events.push_back(e);
    }

    fn known(&self, b: u32) -> Option<bool> {
        self.bufs.iter().find(|e| e.0 == b).map(|e| e.1)
    }

    /// (pending, current) usage.
    fn usage(&self, b: u32) -> (bool, bool) {
        let mut u = (false, false);
        for s in &self.slots {
            u.0 |= s.pending.as_ref().is_some_and(|p| p.bufs.contains(&b));
            u.1 |= s.current.as_ref().is_some_and(|p| p.bufs.contains(&b));
        }
        u
    }

    fn in_use(&self, b: u32) -> bool {
        let u = self.usage(b);
        u.0 || u.1
    }

    fn state(&self, b: u32) -> Option<BufferState> {
        let w = self.known(b)?;
        Some(match self.usage(b) {
            (_, true) => BufferState::Scanning,
            (true, false) => BufferState::Submitted,
            (false, false) if w => BufferState::Writable,
            (false, false) => BufferState::Ready,
        })
    }

    fn held(&self) -> Vec<u32> {
        let mut h = Vec::new();
        for s in &self.slots {
            for sub in [&s.pending, &s.current].into_iter().flatten() {
                for b in &sub.bufs {
                    if !h.contains(b) {
                        h.push(*b);
                    }
                }
            }
        }
        h
    }

    /// Invariant of every operation: a buffer gets a `Released` event
    /// exactly when it stops being referenced by the display, once.
    fn check_released_exactly(&self, held_before: &[u32], emitted_from: usize) {
        let held_after = self.held();
        let mut expect: Vec<u32> = held_before
            .iter()
            .copied()
            .filter(|b| !held_after.contains(b))
            .collect();
        let mut got: Vec<u32> = self
            .events
            .iter()
            .skip(emitted_from)
            .filter_map(|e| match e {
                DisplayEvent::Released { buffer } => Some(buffer.0),
                _ => None,
            })
            .collect();
        expect.sort();
        got.sort();
        assert_eq!(
            got, expect,
            "released set differs from set of buffers that left the display"
        );
    }

    /// Everything `check_submit` checks except the event-queue room.
    fn check_submit(&self, out: u32, bufs: &[u32], replace: bool) -> Result<(), PresentError> {
        if distinct(bufs).len() > MAX_LAYERS {
            hit(4);
            return Err(PresentError::TooManyBuffers);
        }
        for b in bufs {
            match self.known(*b) {
                None => return Err(PresentError::UnknownBuffer(BufferId(*b))),
                Some(true) => return Err(PresentError::BufferNotReady(BufferId(*b))),
                Some(false) => {}
            }
        }
        match self.slots.iter().find(|s| s.output == out) {
            Some(s) if s.pending.is_some() && !replace => Err(PresentError::Busy(OutputId(out))),
            Some(_) => Ok(()),
            None if self.slots.len() >= MAX_OUTPUTS => {
                hit(3);
                Err(PresentError::TooManyOutputs)
            }
            None => Ok(()),
        }
    }

    /// Room the real tracker must have for a submit to be accepted: the
    /// documented worst case of the engine's commit sequence.
    fn submit_room_bound(&self) -> usize {
        let pending = self.slots.iter().filter(|s| s.pending.is_some()).count();
        (pending + 1) * (MAX_LAYERS + 1) + 1
    }

    fn apply_submit(&mut self, out: u32, bufs: &[u32]) -> u64 {
        let before = self.held();
        let mark = self.events.len();
        let id = self.next_id;
        self.next_id += 1;
        let sub = Sub {
            id,
            bufs: distinct(bufs),
            at: self.now,
            overdue: false,
        };
        let i = match self.slots.iter().position(|s| s.output == out) {
            Some(i) => i,
            None => {
                self.slots.push(Slot {
                    output: out,
                    pending: None,
                    current: None,
                });
                self.slots.len() - 1
            }
        };
        let old = self.slots[i].pending.replace(sub);
        if let Some(old) = old {
            hit(7);
            self.emit(DisplayEvent::Dropped {
                output: OutputId(out),
                submission: SubmissionId(old.id),
            });
            for b in &old.bufs {
                if !self.in_use(*b) {
                    self.emit(DisplayEvent::Released {
                        buffer: BufferId(*b),
                    });
                }
            }
        }
        self.check_released_exactly(&before, mark);
        id
    }

    fn latch_check(&self, out: u32, sid: u64) -> Result<(), ()> {
        match self.slots.iter().find(|s| s.output == out) {
            Some(Slot {
                pending: Some(p), ..
            }) if p.id == sid => Ok(()),
            _ => Err(()),
        }
    }

    fn apply_latch(&mut self, out: u32, sid: u64) {
        let before = self.held();
        let mark = self.events.len();
        let i = self.slots.iter().position(|s| s.output == out).unwrap();
        let new = self.slots[i].pending.take();
        let old = std::mem::replace(&mut self.slots[i].current, new);
        self.emit(DisplayEvent::Presented {
            output: OutputId(out),
            submission: SubmissionId(sid),
        });
        if let Some(old) = old {
            for b in &old.bufs {
                if !self.in_use(*b) {
                    self.emit(DisplayEvent::Released {
                        buffer: BufferId(*b),
                    });
                }
            }
        }
        self.check_released_exactly(&before, mark);
    }

    fn reset_needed(&self) -> usize {
        let mut n = 0;
        for s in &self.slots {
            n += s.pending.as_ref().map_or(0, |p| 1 + p.bufs.len());
            n += s.current.as_ref().map_or(0, |p| p.bufs.len());
        }
        n
    }

    fn apply_reset(&mut self) {
        let before = self.held();
        let mark = self.events.len();
        let slots = std::mem::take(&mut self.slots);
        for s in &slots {
            if let Some(p) = &s.pending {
                self.emit(DisplayEvent::Dropped {
                    output: OutputId(s.output),
                    submission: SubmissionId(p.id),
                });
            }
        }
        let mut done: Vec<u32> = Vec::new();
        for s in &slots {
            for sub in [&s.pending, &s.current].into_iter().flatten() {
                for b in &sub.bufs {
                    if !done.contains(b) {
                        done.push(*b);
                        self.emit(DisplayEvent::Released {
                            buffer: BufferId(*b),
                        });
                    }
                }
            }
        }
        // `slots` is gone, so nothing is held any more.
        self.check_released_exactly(&before, mark);
        assert!(self.held().is_empty());
    }

    fn apply_overdue(&mut self, timeout: u64) {
        for i in 0..self.slots.len() {
            let out = self.slots[i].output;
            let Some(p) = self.slots[i].pending.clone() else {
                continue;
            };
            if p.overdue || self.now.saturating_sub(p.at) < timeout {
                continue;
            }
            if self.room() > 0 {
                hit(8);
                self.emit(DisplayEvent::LatchTimeout {
                    output: OutputId(out),
                    submission: SubmissionId(p.id),
                });
                self.slots[i].pending.as_mut().unwrap().overdue = true;
            }
        }
    }
}

// ------------------------------------------------------------------- steps

const OUTPUTS: u32 = MAX_OUTPUTS as u32 + 2;
const BUF_UNIVERSE: u32 = MAX_BUFFERS as u32 + 6;

#[derive(Clone, Debug)]
enum Op {
    Register(u32),
    Unregister(u32),
    BeginWrite(u32),
    FinishWrite(u32),
    CheckSubmit(u32, Vec<u32>, bool),
    Submit(u32, Vec<u32>, bool),
    Latched(u32, u64),
    Reset,
    SetTime(u64),
    Overdue(u64),
    PushEvent(DisplayEvent),
    Poll(usize),
    /// Push foreign events until only this much room is left.
    FillTo(usize),
    /// Register every identifier of the universe, filling the table.
    RegisterAll,
}

struct Gen {
    rng: Rng,
    /// Percent of steps that poll; low values keep the queue full.
    poll_weight: u64,
    /// Percent of buffer picks outside the small hot set.
    wide: u64,
    last_latched: Vec<u64>,
}

impl Gen {
    fn buf(&mut self) -> u32 {
        if self.rng.chance(self.wide) {
            self.rng.below(BUF_UNIVERSE as u64) as u32
        } else {
            self.rng.below(8) as u32
        }
    }

    fn bufs(&mut self) -> Vec<u32> {
        let n = match self.rng.below(10) {
            0 => 0,
            1 => MAX_LAYERS as u64 + 1 + self.rng.below(3),
            2..=3 => 1 + self.rng.below(5),
            _ => 1 + self.rng.below(3),
        };
        if n > MAX_LAYERS as u64 {
            // Distinct identifiers, known or not: the count is checked first.
            let start = self.rng.below(8) as u32;
            return (0..n as u32).map(|i| start + i).collect();
        }
        (0..n).map(|_| self.buf()).collect()
    }

    fn output(&mut self) -> u32 {
        if self.rng.chance(85) {
            self.rng.below(2) as u32
        } else {
            self.rng.below(OUTPUTS as u64) as u32
        }
    }

    fn event(&mut self, m: &Model) -> DisplayEvent {
        let out = OutputId(self.output());
        let sub = SubmissionId(self.rng.below(m.next_id + 3));
        match self.rng.below(6) {
            0 => DisplayEvent::DeviceLost,
            1 => DisplayEvent::CapabilitiesChanged,
            2 => DisplayEvent::Presented {
                output: out,
                submission: sub,
            },
            3 => DisplayEvent::Dropped {
                output: out,
                submission: sub,
            },
            4 => DisplayEvent::LatchTimeout {
                output: out,
                submission: sub,
            },
            _ => DisplayEvent::Released {
                buffer: BufferId(self.buf()),
            },
        }
    }

    fn op(&mut self, m: &Model) -> Op {
        if self.rng.chance(self.poll_weight) {
            return Op::Poll(1 + self.rng.below(4) as usize);
        }
        match self.rng.below(100) {
            0..=11 => Op::Register(self.buf()),
            12..=15 => Op::Unregister(self.buf()),
            16..=23 => Op::BeginWrite(self.buf()),
            24..=33 => Op::FinishWrite(self.buf()),
            34..=39 => Op::CheckSubmit(self.output(), self.bufs(), self.rng.chance(50)),
            40..=59 => Op::Submit(self.output(), self.bufs(), self.rng.chance(40)),
            60..=79 => {
                let out = self.output();
                let pending = m
                    .slots
                    .iter()
                    .find(|s| s.output == out)
                    .and_then(|s| s.pending.as_ref())
                    .map(|p| p.id);
                let sid = match (pending, self.rng.below(10)) {
                    (Some(p), 0..=5) => p,
                    // Duplicate or stale: something latched before.
                    (_, 6) if !self.last_latched.is_empty() => {
                        let n = self.last_latched.len() as u64;
                        self.last_latched[self.rng.below(n) as usize]
                    }
                    // Future.
                    (_, 7) => m.next_id + self.rng.below(3),
                    // Anything, including zero.
                    _ => self.rng.below(m.next_id + 3),
                };
                Op::Latched(out, sid)
            }
            80..=82 => Op::Reset,
            83..=86 => Op::SetTime(if self.rng.chance(10) {
                self.rng.below(50)
            } else {
                m.now + self.rng.below(20)
            }),
            87..=90 => Op::Overdue(self.rng.below(30)),
            91..=96 => Op::PushEvent(self.event(m)),
            97 => Op::FillTo(self.rng.below(40) as usize),
            98 if self.rng.chance(30) => Op::RegisterAll,
            _ => Op::Poll(1 + self.rng.below(4) as usize),
        }
    }
}

fn ids(b: &[u32]) -> Vec<BufferId> {
    b.iter().map(|b| BufferId(*b)).collect()
}

fn same_state(p: &Presentation, m: &Model) {
    for b in 0..BUF_UNIVERSE {
        assert_eq!(p.state(BufferId(b)), m.state(b), "state of buffer {b}");
        // Never writable while pending or on screen.
        if m.in_use(b) {
            assert!(matches!(
                p.state(BufferId(b)),
                Some(BufferState::Submitted | BufferState::Scanning)
            ));
        }
    }
    for o in 0..OUTPUTS {
        let slot = m.slots.iter().find(|s| s.output == o);
        assert_eq!(
            p.has_pending(OutputId(o)),
            slot.is_some_and(|s| s.pending.is_some()),
            "has_pending({o})"
        );
        assert_eq!(
            p.on_screen(OutputId(o)),
            slot.and_then(|s| s.current.as_ref())
                .map(|c| SubmissionId(c.id)),
            "on_screen({o})"
        );
    }
    assert_eq!(p.next_submission_id(), SubmissionId(m.next_id));
    assert_eq!(p.events_room(), m.room(), "events_room");
    assert_eq!(p.events_empty(), m.events.is_empty());
}

fn register(p: &mut Presentation, m: &mut Model, b: u32) {
    let want = if m.known(b).is_some() {
        Err(PresentError::DuplicateBuffer(BufferId(b)))
    } else if m.bufs.len() >= MAX_BUFFERS {
        hit(5);
        Err(PresentError::BufferTableFull)
    } else {
        m.bufs.push((b, true));
        Ok(())
    };
    assert_eq!(p.register(BufferId(b), BufferInfo::default()), want);
}

/// Applies one operation to both sides and compares the results.
fn step(p: &mut Presentation, m: &mut Model, op: &Op) {
    match op {
        Op::Register(b) => register(p, m, *b),
        Op::RegisterAll => {
            for b in 0..BUF_UNIVERSE {
                register(p, m, b);
            }
        }
        Op::Unregister(b) => {
            let want = if m.known(*b).is_none() {
                Err(PresentError::UnknownBuffer(BufferId(*b)))
            } else if m.in_use(*b) {
                Err(PresentError::BufferInUse(BufferId(*b)))
            } else {
                m.bufs.retain(|e| e.0 != *b);
                Ok(())
            };
            assert_eq!(p.unregister(BufferId(*b)), want);
        }
        Op::BeginWrite(b) => {
            let want = if m.known(*b).is_none() {
                Err(PresentError::UnknownBuffer(BufferId(*b)))
            } else if m.in_use(*b) {
                Err(PresentError::BufferInUse(BufferId(*b)))
            } else {
                m.bufs.iter_mut().find(|e| e.0 == *b).unwrap().1 = true;
                Ok(())
            };
            assert_eq!(p.begin_write(BufferId(*b)), want);
        }
        Op::FinishWrite(b) => {
            let want = match m.known(*b) {
                None => Err(PresentError::UnknownBuffer(BufferId(*b))),
                Some(false) => Err(PresentError::BufferNotWritable(BufferId(*b))),
                Some(true) => {
                    m.bufs.iter_mut().find(|e| e.0 == *b).unwrap().1 = false;
                    Ok(())
                }
            };
            assert_eq!(p.finish_write(BufferId(*b)), want);
        }
        Op::CheckSubmit(o, bufs, replace) => {
            let got = p.check_submit(OutputId(*o), &ids(bufs), *replace);
            match m.check_submit(*o, bufs, *replace) {
                Err(e) => assert_eq!(got, Err(e)),
                Ok(()) => match got {
                    Ok(()) => {}
                    // Documented: refused when the room cannot cover the
                    // worst case of the following commit sequence.
                    Err(PresentError::EventQueueFull) => {
                        assert!(m.room() < m.submit_room_bound(), "needless EventQueueFull")
                    }
                    other => panic!("check_submit: {other:?}"),
                },
            }
        }
        Op::Submit(o, bufs, replace) => {
            let got = p.submit(OutputId(*o), &ids(bufs), *replace);
            match m.check_submit(*o, bufs, *replace) {
                Err(e) => assert_eq!(got, Err(e)),
                Ok(()) => match got {
                    Ok(id) => {
                        let want = m.apply_submit(*o, bufs);
                        assert_eq!(id, SubmissionId(want));
                    }
                    Err(PresentError::EventQueueFull) => {
                        hit(0);
                        assert!(m.room() < m.submit_room_bound(), "needless EventQueueFull")
                    }
                    other => panic!("submit: {other:?}"),
                },
            }
        }
        Op::Latched(o, sid) => {
            let got = p.latched(OutputId(*o), SubmissionId(*sid));
            let verdict = m.latch_check(*o, *sid);
            if verdict.is_err() {
                hit(6);
            }
            match verdict {
                Err(()) => assert_eq!(
                    got,
                    Err(PresentError::UnexpectedLatch {
                        output: OutputId(*o),
                        submission: SubmissionId(*sid)
                    })
                ),
                Ok(()) if m.room() < MAX_LAYERS + 1 => {
                    hit(1);
                    assert_eq!(got, Err(PresentError::EventQueueFull))
                }
                Ok(()) => {
                    assert_eq!(got, Ok(()));
                    m.apply_latch(*o, *sid);
                }
            }
        }
        Op::Reset => {
            let got = p.reset();
            if got == Err(PresentError::EventQueueFull) {
                hit(2);
                assert!(m.room() < m.reset_needed(), "needless EventQueueFull");
            } else {
                assert_eq!(got, Ok(()));
                m.apply_reset();
            }
        }
        Op::SetTime(t) => {
            p.set_time(*t);
            m.now = *t;
        }
        Op::Overdue(t) => {
            p.report_overdue(*t);
            m.apply_overdue(*t);
        }
        Op::PushEvent(e) => {
            let got = p.push_event(*e);
            if m.room() == 0 {
                assert_eq!(got, Err(PresentError::EventQueueFull));
            } else {
                assert_eq!(got, Ok(()));
                m.emit(*e);
            }
        }
        Op::FillTo(room) => {
            while m.room() > *room {
                assert_eq!(p.push_event(DisplayEvent::CapabilitiesChanged), Ok(()));
                m.emit(DisplayEvent::CapabilitiesChanged);
            }
        }
        Op::Poll(n) => {
            for _ in 0..*n {
                assert_eq!(p.poll_event(), m.events.pop_front());
            }
        }
    }
}

/// Runs one seeded sequence. `log` receives every step before it runs.
fn run(seed: u64, steps: usize, log: &mut Vec<String>) {
    let mut rng = Rng(seed);
    let poll_weight = [0, 2, 8, 25, 50][rng.below(5) as usize];
    let wide = [0, 3, 20][rng.below(3) as usize];
    let mut gen = Gen {
        rng,
        poll_weight,
        wide,
        last_latched: Vec::new(),
    };
    let mut p = Presentation::new();
    let mut m = Model::new();
    for _ in 0..steps {
        let op = gen.op(&m);
        log.push(format!("{op:?}"));
        let on_screen: Vec<u64> = m
            .slots
            .iter()
            .filter_map(|s| s.current.as_ref().map(|c| c.id))
            .collect();
        step(&mut p, &mut m, &op);
        for id in on_screen {
            if !gen.last_latched.contains(&id) {
                gen.last_latched.push(id);
            }
        }
        same_state(&p, &m);
    }
    // Drain: nothing may be lost or invented.
    while let Some(e) = p.poll_event() {
        assert_eq!(Some(e), m.events.pop_front());
    }
    assert!(m.events.is_empty(), "model has events the tracker lost");
    // Reset must always be able to finish once the queue is empty, and
    // leave nothing in use.
    p.reset().unwrap();
    m.apply_reset();
    while let Some(e) = p.poll_event() {
        assert_eq!(Some(e), m.events.pop_front());
    }
    assert!(m.events.is_empty());
    for b in 0..BUF_UNIVERSE {
        assert_ne!(p.state(BufferId(b)), Some(BufferState::Submitted));
        assert_ne!(p.state(BufferId(b)), Some(BufferState::Scanning));
    }
}

fn check_seed(seed: u64, steps: usize) {
    let mut log = Vec::new();
    let result = catch_unwind(AssertUnwindSafe(|| run(seed, steps, &mut log)));
    if let Err(panic) = result {
        let what = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        let mut text = format!(
            "presentation model mismatch, seed {seed} ({} steps run, last one failed): {what}\n",
            log.len()
        );
        for (i, s) in log.iter().enumerate() {
            text.push_str(&format!("  {i:4}: {s}\n"));
        }
        panic!("{text}");
    }
}

#[test]
fn random_sequences_match_the_model() {
    if let Ok(seed) = std::env::var("PRESENTATION_MODEL_SEED") {
        // `<n>` or `<first>-<last>`.
        let (first, last) = seed.split_once('-').unwrap_or((&seed, &seed));
        let (first, last): (u64, u64) = (first.parse().unwrap(), last.parse().unwrap());
        for seed in first..=last {
            check_seed(seed, 4000);
        }
        return;
    }
    for seed in 0..150 {
        check_seed(seed, 800);
    }
    for (name, n) in HIT_NAMES.iter().zip(HITS.iter()) {
        assert!(n.load(Ordering::Relaxed) > 0, "never reached: {name}");
    }
}

// ------------------------------------------------- directed edge scenarios

const OUT: OutputId = OutputId(0);

fn ready(ids: &[u32]) -> Presentation {
    let mut p = Presentation::new();
    for b in ids {
        p.register(BufferId(*b), BufferInfo::default()).unwrap();
        p.finish_write(BufferId(*b)).unwrap();
    }
    p
}

fn drain(p: &mut Presentation) -> Vec<DisplayEvent> {
    std::iter::from_fn(|| p.poll_event()).collect()
}

#[test]
fn full_queue_never_loses_a_latch() {
    // Fill the queue with foreign events: latch/submit/reset are refused
    // with an error and change nothing; after draining they go through.
    let mut p = ready(&[1, 2]);
    let s1 = p.submit(OUT, &[BufferId(1)], false).unwrap();
    while p.push_event(DisplayEvent::CapabilitiesChanged).is_ok() {}
    assert_eq!(p.events_room(), 0);
    assert_eq!(p.latched(OUT, s1), Err(PresentError::EventQueueFull));
    assert!(p.has_pending(OUT));
    assert_eq!(p.on_screen(OUT), None);
    assert_eq!(p.reset(), Err(PresentError::EventQueueFull));
    assert!(p.has_pending(OUT));
    assert_eq!(
        p.submit(OUT, &[BufferId(2)], true),
        Err(PresentError::EventQueueFull)
    );
    assert_eq!(p.state(BufferId(1)), Some(BufferState::Submitted));
    drain(&mut p);
    p.latched(OUT, s1).unwrap();
    assert_eq!(
        drain(&mut p),
        [DisplayEvent::Presented {
            output: OUT,
            submission: s1
        }]
    );
}

#[test]
fn overdue_report_is_retried_when_the_queue_is_full() {
    let mut p = ready(&[1]);
    p.set_time(10);
    let s = p.submit(OUT, &[BufferId(1)], false).unwrap();
    while p.push_event(DisplayEvent::CapabilitiesChanged).is_ok() {}
    p.set_time(100);
    p.report_overdue(5);
    drain(&mut p);
    p.report_overdue(5);
    p.report_overdue(5);
    assert_eq!(
        drain(&mut p),
        [DisplayEvent::LatchTimeout {
            output: OUT,
            submission: s
        }]
    );
}

#[test]
fn reset_with_only_pending_releases_once_and_keeps_ids_growing() {
    let mut p = ready(&[1, 2]);
    let a = p.submit(OUT, &[BufferId(1), BufferId(2)], false).unwrap();
    p.reset().unwrap();
    assert_eq!(
        drain(&mut p),
        [
            DisplayEvent::Dropped {
                output: OUT,
                submission: a
            },
            DisplayEvent::Released {
                buffer: BufferId(1)
            },
            DisplayEvent::Released {
                buffer: BufferId(2)
            },
        ]
    );
    assert_eq!(
        p.latched(OUT, a),
        Err(PresentError::UnexpectedLatch {
            output: OUT,
            submission: a
        })
    );
    assert!(p.next_submission_id() > a);
}
