//! Adversarial event ordering on the fake backend (SoW §10).
//!
//! A seeded generator drives random sequences of buffer operations, frames,
//! vertical blanks, spurious or duplicated latches, missing latches, device
//! loss and recovery, capability changes and frames that replace a pending
//! one. A model kept in this file is fed by what the engine reports and by the
//! results of the calls, and is checked after every step:
//!
//! * every buffer is `Released` exactly once per use, and only after the
//!   submissions that held it were retired (latched over, dropped, or lost);
//! * lifecycle calls give exactly the documented error: a buffer that is
//!   submitted or on screen cannot be written or unregistered;
//! * a `Dropped` submission is never `Presented`, latches come in submission
//!   order per output, a `LatchTimeout` comes once and only for an overdue
//!   pending submission, and an overdue one is never missed;
//! * a latch that matches no pending submission is reported as
//!   `UnexpectedLatch`, never as `Presented`, and changes nothing;
//! * after a `DeviceLost` nothing is pending or shown and every buffer was
//!   released;
//! * the buffer states reported by the engine agree with the model.
//!
//! A failing seed prints its seed and the trace of steps. Replay one seed with
//! `EVENT_ORDER_SEED=<n>`; raise the number of seeds with
//! `EVENT_ORDER_SEEDS=<n>`.
//!
//! The run takes every queued event before it presents, for the model's
//! sake; a plan made stale by a queued capability change is covered by
//! directed cases.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};

use composition_core::{
    BackendError, BackendEvent, BlendMode, BufferId, BufferInfo, BufferState, Capabilities,
    DisplayEvent, Engine, Error, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId,
    PixelFormat, PlaneCaps, PlaneId, PlaneKind, PresentError, Rect, Size, SubmissionId, Zpos,
};
use composition_fake::FakeBackend;

const OUTS: [OutputId; 2] = [OutputId(0), OutputId(1)];
const FULL: Rect = Rect::new(0, 0, 1280, 720);
/// Buffers 1..=POOL may be registered; buffer 9 never is.
const POOL: u32 = 8;

type R = Result<(), String>;

macro_rules! ensure {
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            return Err(format!($($arg)+));
        }
    };
}

// ---------------------------------------------------------------------------
// Scenario
// ---------------------------------------------------------------------------

fn plane(id: u32, kind: PlaneKind, zpos: i32, output: usize) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs: 1 << output,
        zpos: Zpos::Fixed(zpos),
        blend_modes: BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask(),
        ..PlaneCaps::default()
    };
    caps.formats
        .push(FormatModifier::linear(PixelFormat::ARGB8888))
        .unwrap();
    caps
}

/// Two outputs with two fixed planes each.
fn caps(replace_pending: bool, refresh_mhz: u32) -> Capabilities {
    let mut caps = Capabilities {
        replace_pending,
        ..Capabilities::default()
    };
    for (index, id) in OUTS.iter().enumerate() {
        caps.outputs
            .push(OutputCaps {
                id: *id,
                size: Size::new(1280, 720),
                refresh_mhz,
            })
            .unwrap();
        let base = 10 * (index as u32 + 1);
        caps.planes
            .push(plane(base, PlaneKind::Primary, 0, index))
            .unwrap();
        caps.planes
            .push(plane(base + 1, PlaneKind::Overlay, 1, index))
            .unwrap();
    }
    caps
}

fn info() -> BufferInfo {
    BufferInfo {
        size: Size::new(1280, 720),
        format: PixelFormat::ARGB8888,
        modifier: Modifier::LINEAR,
    }
}

fn layers(buffers: &[BufferId]) -> Vec<Layer> {
    buffers
        .iter()
        .enumerate()
        .map(|(index, buffer)| {
            let mut layer = Layer::new(*buffer, FULL, FULL, index as i32 * 10);
            layer.blend = BlendMode::Premultiplied;
            layer
        })
        .collect()
}

/// splitmix64.
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

    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fate {
    Pending,
    OnScreen,
    Superseded,
    Dropped,
    Lost,
}

struct Sub {
    output: OutputId,
    buffers: Vec<BufferId>,
    fate: Fate,
    submitted_at: u64,
    reported: bool,
}

/// A submission the engine accepted, recorded when its first consequence
/// shows in the event stream (events of a commit precede its recording).
struct Deferred {
    id: SubmissionId,
    sub: Sub,
}

struct Model {
    /// Registered buffers and whether the producer holds them.
    buffers: BTreeMap<BufferId, bool>,
    subs: BTreeMap<SubmissionId, Sub>,
    deferred: Option<Deferred>,
    next_id: u64,
    held: BTreeMap<BufferId, u32>,
    owed: BTreeMap<BufferId, u32>,
    zeroed: BTreeMap<BufferId, u32>,
    releases: BTreeMap<BufferId, u32>,
    last_presented: BTreeMap<OutputId, SubmissionId>,
    now: u64,
    timeout: Option<u64>,
    reset_run: bool,
    reset_dropped: BTreeSet<SubmissionId>,
    lost_seen: u32,
    caps_seen: u32,
}

impl Model {
    fn new() -> Self {
        Self {
            buffers: BTreeMap::new(),
            subs: BTreeMap::new(),
            deferred: None,
            next_id: 1,
            held: BTreeMap::new(),
            owed: BTreeMap::new(),
            zeroed: BTreeMap::new(),
            releases: BTreeMap::new(),
            last_presented: BTreeMap::new(),
            now: 0,
            timeout: None,
            reset_run: false,
            reset_dropped: BTreeSet::new(),
            lost_seen: 0,
            caps_seen: 0,
        }
    }

    fn held(&self, buffer: BufferId) -> bool {
        self.held.get(&buffer).copied().unwrap_or(0) > 0
    }

    fn pending_on(&self, output: OutputId) -> Option<SubmissionId> {
        self.subs
            .iter()
            .filter(|(_, s)| s.output == output && s.fate == Fate::Pending)
            .map(|(id, _)| *id)
            .max()
    }

    fn on_screen(&self, output: OutputId) -> Option<SubmissionId> {
        self.subs
            .iter()
            .find(|(_, s)| s.output == output && s.fate == Fate::OnScreen)
            .map(|(id, _)| *id)
    }

    fn record(&mut self, id: SubmissionId, sub: Sub) {
        for buffer in &sub.buffers {
            *self.held.entry(*buffer).or_default() += 1;
        }
        self.subs.insert(id, sub);
    }

    fn flush_deferred(&mut self) {
        if let Some(d) = self.deferred.take() {
            self.record(d.id, d.sub);
        }
    }

    fn retire(&mut self, id: SubmissionId, fate: Fate) {
        let sub = self.subs.get_mut(&id).unwrap();
        sub.fate = fate;
        for buffer in sub.buffers.clone() {
            let held = self.held.get_mut(&buffer).unwrap();
            *held -= 1;
            if *held == 0 {
                *self.owed.entry(buffer).or_default() += 1;
                *self.zeroed.entry(buffer).or_default() += 1;
            }
        }
    }

    /// The device reset began: everything pending is dropped and everything
    /// shown is lost; each buffer is released once.
    fn enter_reset(&mut self) {
        self.reset_run = true;
        let ids: Vec<_> = self.subs.keys().copied().collect();
        for id in ids {
            match self.subs[&id].fate {
                Fate::Pending => {
                    self.reset_dropped.insert(id);
                    self.retire(id, Fate::Dropped);
                }
                Fate::OnScreen => self.retire(id, Fate::Lost),
                _ => {}
            }
        }
    }

    /// Applies one display event; `reset_ahead` tells whether a `DeviceLost`
    /// follows in the same batch.
    fn apply(&mut self, event: DisplayEvent, reset_ahead: bool) -> R {
        if let Some(d) = &self.deferred {
            let replaces = matches!(event, DisplayEvent::Dropped { output, submission }
                if output == d.sub.output && submission < d.id);
            let own =
                matches!(event, DisplayEvent::Presented { submission, .. } if submission == d.id);
            if replaces || own {
                self.flush_deferred();
            }
        }
        match event {
            DisplayEvent::Presented { output, submission } => {
                let Some(sub) = self.subs.get(&submission) else {
                    return Err(format!("Presented for unknown {submission:?}"));
                };
                ensure!(
                    sub.fate == Fate::Pending && sub.output == output,
                    "Presented {submission:?} on {output:?} but it is {:?} on {:?}",
                    sub.fate,
                    sub.output
                );
                ensure!(
                    !self.subs.iter().any(|(id, s)| s.output == output
                        && s.fate == Fate::Pending
                        && *id < submission),
                    "{submission:?} latched while an older submission is pending on {output:?}"
                );
                ensure!(
                    self.last_presented
                        .get(&output)
                        .is_none_or(|last| *last < submission),
                    "latches out of order on {output:?}"
                );
                self.last_presented.insert(output, submission);
                if let Some(previous) = self.on_screen(output) {
                    self.retire(previous, Fate::Superseded);
                }
                self.subs.get_mut(&submission).unwrap().fate = Fate::OnScreen;
            }
            DisplayEvent::Dropped { output, submission } => {
                let Some(sub) = self.subs.get(&submission) else {
                    return Err(format!("Dropped for unknown {submission:?}"));
                };
                let (sub_output, fate) = (sub.output, sub.fate);
                ensure!(sub_output == output, "Dropped on the wrong output");
                let newer = self
                    .subs
                    .iter()
                    .any(|(id, s)| s.output == output && *id > submission);
                if !newer && !self.reset_run {
                    ensure!(
                        fate == Fate::Pending,
                        "Dropped {submission:?} which is {fate:?}"
                    );
                    self.enter_reset();
                }
                if !self.reset_dropped.remove(&submission) {
                    ensure!(
                        fate == Fate::Pending && newer,
                        "Dropped {submission:?} which is {fate:?} (newer: {newer})"
                    );
                    self.retire(submission, Fate::Dropped);
                }
            }
            DisplayEvent::Released { buffer } => {
                if self.held(buffer) {
                    ensure!(
                        reset_ahead && !self.reset_run,
                        "{buffer:?} released while pending or displayed"
                    );
                    self.enter_reset();
                }
                let owed = self.owed.entry(buffer).or_default();
                ensure!(
                    *owed > 0,
                    "{buffer:?} released without being retired (or twice)"
                );
                *owed -= 1;
                *self.releases.entry(buffer).or_default() += 1;
            }
            DisplayEvent::LatchTimeout { output, submission } => {
                let timeout = self.timeout;
                let now = self.now;
                let Some(sub) = self.subs.get_mut(&submission) else {
                    return Err(format!("LatchTimeout for unknown {submission:?}"));
                };
                ensure!(
                    sub.fate == Fate::Pending && sub.output == output,
                    "LatchTimeout for {submission:?} which is {:?}",
                    sub.fate
                );
                ensure!(!sub.reported, "LatchTimeout twice for {submission:?}");
                ensure!(
                    timeout.is_some_and(|t| now.saturating_sub(sub.submitted_at) >= t),
                    "LatchTimeout for {submission:?} that is not overdue"
                );
                sub.reported = true;
            }
            DisplayEvent::DeviceLost => {
                ensure!(
                    self.reset_dropped.is_empty(),
                    "DeviceLost before the Dropped of {:?}",
                    self.reset_dropped
                );
                ensure!(
                    !self
                        .subs
                        .values()
                        .any(|s| matches!(s.fate, Fate::Pending | Fate::OnScreen)),
                    "DeviceLost reported but submissions were not dropped or released"
                );
                ensure!(
                    self.owed.values().all(|n| *n == 0),
                    "DeviceLost with buffers not released: {:?}",
                    self.owed
                );
                self.reset_run = false;
                self.lost_seen += 1;
            }
            DisplayEvent::CapabilitiesChanged => self.caps_seen += 1,
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Harness<'a> {
    engine: Engine<FakeBackend>,
    m: Model,
    rng: Rng,
    trace: &'a RefCell<Vec<String>>,
    replace: bool,
    fake_lost: bool,
    fail_armed: bool,
    in_armed: bool,
    after_armed: bool,
    armed_event: Option<BackendEvent>,
    expect_lost: u32,
    expect_caps: u32,
    dirty: bool,
    unexpected_latches: u32,
}

impl<'a> Harness<'a> {
    fn new(seed: u64, trace: &'a RefCell<Vec<String>>) -> Self {
        let mut rng = Rng(seed);
        let replace = rng.chance(2, 3);
        Self {
            engine: Engine::new(FakeBackend::new(caps(replace, 60_000))),
            m: Model::new(),
            rng,
            trace,
            replace,
            fake_lost: false,
            fail_armed: false,
            in_armed: false,
            after_armed: false,
            armed_event: None,
            expect_lost: 0,
            expect_caps: 0,
            dirty: false,
            unexpected_latches: 0,
        }
    }

    fn log(&self, text: String) {
        self.trace.borrow_mut().push(text);
    }

    fn lost_armed(&self) -> bool {
        self.armed_event == Some(BackendEvent::DeviceLost)
    }

    /// Takes every event out of the engine and applies it to the model.
    fn drain(&mut self) -> R {
        let mut batch = Vec::new();
        for _ in 0..100_000 {
            match self.engine.poll_event() {
                Ok(None) => break,
                Ok(Some(event)) => batch.push(Ok(event)),
                Err(error) => batch.push(Err(error)),
            }
        }
        ensure!(
            matches!(self.engine.poll_event(), Ok(None)),
            "poll_event did not run dry"
        );
        for index in 0..batch.len() {
            let reset_ahead = batch[index + 1..]
                .iter()
                .any(|e| matches!(e, Ok(DisplayEvent::DeviceLost)));
            match batch[index] {
                Ok(event) => {
                    self.log(format!("    event {event:?}"));
                    self.m.apply(event, reset_ahead)?;
                }
                Err(Error::Present(PresentError::UnexpectedLatch { output, submission })) => {
                    self.log(format!("    UnexpectedLatch {output:?} {submission:?}"));
                    self.m.flush_deferred();
                    ensure!(
                        self.m.pending_on(output) != Some(submission),
                        "UnexpectedLatch for the pending {submission:?} on {output:?}"
                    );
                    self.unexpected_latches += 1;
                }
                Err(error) => return Err(format!("unexpected error from poll_event: {error:?}")),
            }
        }
        self.m.flush_deferred();
        self.dirty = false;
        self.check_consistency()
    }

    /// The engine agrees with the model once every event was taken.
    fn check_consistency(&self) -> R {
        ensure!(
            self.m.lost_seen == self.expect_lost,
            "{} DeviceLost reported, {} expected",
            self.m.lost_seen,
            self.expect_lost
        );
        ensure!(
            self.m.caps_seen == self.expect_caps,
            "{} CapabilitiesChanged reported, {} expected",
            self.m.caps_seen,
            self.expect_caps
        );
        ensure!(!self.m.reset_run, "a device reset was left unfinished");
        for id in 1..=POOL + 1 {
            let buffer = BufferId(id);
            let expected = match self.m.buffers.get(&buffer) {
                None => None,
                Some(_)
                    if self
                        .m
                        .subs
                        .values()
                        .any(|s| s.fate == Fate::OnScreen && s.buffers.contains(&buffer)) =>
                {
                    Some(BufferState::Scanning)
                }
                Some(_)
                    if self
                        .m
                        .subs
                        .values()
                        .any(|s| s.fate == Fate::Pending && s.buffers.contains(&buffer)) =>
                {
                    Some(BufferState::Submitted)
                }
                Some(true) => Some(BufferState::Writable),
                Some(false) => Some(BufferState::Ready),
            };
            ensure!(
                self.engine.buffer_state(buffer) == expected,
                "{buffer:?} is {:?}, model says {expected:?}",
                self.engine.buffer_state(buffer)
            );
        }
        Ok(())
    }

    fn ready_buffers(&self) -> Vec<BufferId> {
        self.m
            .buffers
            .iter()
            .filter(|(_, writable)| !**writable)
            .map(|(b, _)| *b)
            .collect()
    }

    // --- operations --------------------------------------------------------

    fn register(&mut self) -> R {
        let buffer = BufferId(1 + self.rng.below(POOL as u64) as u32);
        self.log(format!("register {buffer:?}"));
        let result = self.engine.register_buffer(buffer, info());
        if let std::collections::btree_map::Entry::Vacant(slot) = self.m.buffers.entry(buffer) {
            ensure!(result.is_ok(), "register gave {result:?}");
            slot.insert(true);
        } else {
            ensure!(
                result == Err(Error::Present(PresentError::DuplicateBuffer(buffer))),
                "duplicate register gave {result:?}"
            );
        }
        Ok(())
    }

    fn pick_buffer(&mut self) -> BufferId {
        BufferId(1 + self.rng.below(POOL as u64 + 1) as u32)
    }

    fn begin_write(&mut self) -> R {
        let buffer = self.pick_buffer();
        self.log(format!("begin_write {buffer:?}"));
        let result = self.engine.begin_write(buffer);
        if !self.m.buffers.contains_key(&buffer) {
            ensure!(
                result == Err(Error::Present(PresentError::UnknownBuffer(buffer))),
                "begin_write of unknown gave {result:?}"
            );
        } else if self.m.held(buffer) {
            ensure!(
                result == Err(Error::Present(PresentError::BufferInUse(buffer))),
                "begin_write of a buffer in use gave {result:?}: it could be written while shown"
            );
        } else {
            ensure!(result.is_ok(), "begin_write gave {result:?}");
            self.m.buffers.insert(buffer, true);
        }
        Ok(())
    }

    fn finish_write(&mut self) -> R {
        let buffer = self.pick_buffer();
        self.log(format!("finish_write {buffer:?}"));
        let result = self.engine.finish_write(buffer);
        match self.m.buffers.get(&buffer).copied() {
            None => ensure!(
                result == Err(Error::Present(PresentError::UnknownBuffer(buffer))),
                "finish_write of unknown gave {result:?}"
            ),
            Some(false) => ensure!(
                result == Err(Error::Present(PresentError::BufferNotWritable(buffer))),
                "finish_write of a finished buffer gave {result:?}"
            ),
            Some(true) => {
                ensure!(result.is_ok(), "finish_write gave {result:?}");
                self.m.buffers.insert(buffer, false);
            }
        }
        Ok(())
    }

    fn unregister(&mut self) -> R {
        let buffer = self.pick_buffer();
        self.log(format!("unregister {buffer:?}"));
        let result = self.engine.unregister_buffer(buffer);
        if !self.m.buffers.contains_key(&buffer) {
            ensure!(
                result == Err(Error::Present(PresentError::UnknownBuffer(buffer))),
                "unregister of unknown gave {result:?}"
            );
        } else if self.m.held(buffer) {
            ensure!(
                result == Err(Error::Present(PresentError::BufferInUse(buffer))),
                "unregister of a buffer in use gave {result:?}"
            );
        } else {
            ensure!(result.is_ok(), "unregister gave {result:?}");
            self.m.buffers.remove(&buffer);
        }
        Ok(())
    }

    fn present(&mut self) -> R {
        // The engine records what the backend queued before it commits; take
        // it first so that the state it commits against is the model's.
        self.drain()?;
        let output = if self.rng.chance(1, 24) {
            OutputId(7)
        } else {
            OUTS[self.rng.below(2) as usize]
        };
        let ready = self.ready_buffers();
        let count = 1 + self.rng.below(3) as usize;
        let mut buffers = Vec::new();
        for _ in 0..count {
            if !ready.is_empty() && !self.rng.chance(1, 8) {
                buffers.push(ready[self.rng.below(ready.len() as u64) as usize]);
            } else {
                buffers.push(self.pick_buffer());
            }
        }
        // Arm one of the timing hooks now and then.
        match self.rng.below(12) {
            0 if !self.fail_armed => {
                self.fail_armed = true;
                self.engine.backend_mut().fail_next_commits(1);
                self.log("  arm fail_next_commits".into());
            }
            1 if !self.in_armed && !self.after_armed && !self.lost_armed() => {
                self.in_armed = true;
                self.engine.backend_mut().vsync_in_next_commit();
                self.log("  arm vsync_in_next_commit".into());
            }
            2 if !self.after_armed && !self.in_armed && !self.lost_armed() => {
                self.after_armed = true;
                self.engine.backend_mut().vsync_after_next_commit();
                self.log("  arm vsync_after_next_commit".into());
            }
            3 => {
                let event = if self.rng.chance(1, 2) {
                    BackendEvent::CapabilitiesChanged
                } else {
                    BackendEvent::DeviceLost
                };
                if self.armed_event.is_none()
                    && (event == BackendEvent::CapabilitiesChanged
                        || !(self.in_armed || self.after_armed))
                {
                    self.engine.backend_mut().inject_event_in_next_commit(event);
                    self.armed_event = Some(event);
                    self.log(format!("  arm event in commit {event:?}"));
                }
            }
            _ => {}
        }
        self.log(format!("present {output:?} {buffers:?}"));

        // Expected outcome, from the documented rules.
        let mut distinct = buffers.clone();
        distinct.sort();
        distinct.dedup();
        let planned = OUTS.contains(&output)
            && buffers.len() <= 2
            && buffers.iter().all(|b| self.m.buffers.contains_key(b));
        let writable = buffers
            .iter()
            .find(|b| self.m.buffers.get(b) == Some(&true));
        let fake_busy = !self.replace && self.engine.backend().pending(output).is_some();
        let commits_before = self.engine.backend().commit_count();
        let frame_layers = layers(&buffers);
        let frame = Frame {
            output,
            layers: &frame_layers,
        };
        let result = self.engine.present(&frame);
        let reached = self.engine.backend().commit_count() != commits_before;
        match result {
            Ok(id) => {
                ensure!(
                    planned && writable.is_none() && !self.fake_lost,
                    "present succeeded with buffers {buffers:?} (planned {planned}, writable {writable:?}, lost {})",
                    self.fake_lost
                );
                ensure!(!self.fail_armed, "a commit set to fail succeeded");
                ensure!(
                    self.armed_event != Some(BackendEvent::DeviceLost),
                    "a commit during which the device was lost succeeded"
                );
                ensure!(
                    !(self.m.pending_on(output).is_some() && !self.replace),
                    "a second submission was accepted while one is pending"
                );
                ensure!(
                    id == SubmissionId(self.m.next_id),
                    "submission id {id:?}, expected {}",
                    self.m.next_id
                );
                self.m.next_id += 1;
                self.in_armed = false;
                self.after_armed = false;
                self.m.flush_deferred();
                self.m.deferred = Some(Deferred {
                    id,
                    sub: Sub {
                        output,
                        buffers: distinct,
                        fate: Fate::Pending,
                        submitted_at: self.m.now,
                        reported: false,
                    },
                });
            }
            Err(error) => {
                let expected_lost_error = self.fake_lost && planned && writable.is_none();
                let ok = if !planned {
                    matches!(
                        error,
                        Error::Plan(_)
                            | Error::Present(PresentError::UnknownBuffer(_))
                            | Error::Backend(BackendError::DeviceLost)
                    )
                } else if self.fake_lost {
                    error == Error::Backend(BackendError::DeviceLost)
                } else if let Some(b) = writable {
                    error == Error::Present(PresentError::BufferNotReady(*b))
                } else if self.m.pending_on(output).is_some() && !self.replace {
                    error == Error::Present(PresentError::Busy(output))
                } else if self.fail_armed {
                    error == Error::Backend(BackendError::Platform(-5))
                } else if fake_busy {
                    error == Error::Backend(BackendError::Busy)
                } else if self.armed_event == Some(BackendEvent::DeviceLost) {
                    error == Error::Backend(BackendError::DeviceLost)
                } else {
                    false
                };
                ensure!(
                    ok,
                    "present gave {error:?} (lost-expected {expected_lost_error})"
                );
                if error == Error::Backend(BackendError::Platform(-5)) {
                    self.fail_armed = false;
                }
                if error == Error::Backend(BackendError::Busy) && reached {
                    self.in_armed = false;
                }
            }
        }
        if reached {
            match self.armed_event.take() {
                Some(BackendEvent::DeviceLost) => self.expect_lost += 1,
                Some(BackendEvent::CapabilitiesChanged) => self.expect_caps += 1,
                _ => {}
            }
        }
        self.drain()
    }

    fn vsync(&mut self) -> R {
        self.log("vsync".into());
        let synced: Vec<_> = OUTS
            .iter()
            .filter_map(|o| {
                let fake = self.engine.backend().pending(*o)?;
                (self.m.pending_on(*o) == Some(fake)).then_some(fake)
            })
            .collect();
        self.engine.backend_mut().vsync();
        self.drain()?;
        for id in synced {
            ensure!(
                self.m.subs[&id].fate != Fate::Pending,
                "{id:?} was latched by the hardware but is still pending"
            );
        }
        Ok(())
    }

    fn inject_latch(&mut self) -> R {
        let output = if self.rng.chance(1, 8) {
            OutputId(9)
        } else {
            OUTS[self.rng.below(2) as usize]
        };
        let other = OUTS[1 - (output.0 as usize).min(1)];
        let retired: Vec<_> = self
            .m
            .subs
            .iter()
            .filter(|(_, s)| !matches!(s.fate, Fate::Pending | Fate::OnScreen))
            .map(|(id, _)| *id)
            .collect();
        let kind = self.rng.below(7);
        let submission = match kind {
            0 | 1 => {
                // The right one, possibly twice.
                let Some(id) = self.m.pending_on(output) else {
                    return Ok(());
                };
                id
            }
            2 => match self.m.pending_on(other) {
                Some(id) => id,
                None => SubmissionId(self.m.next_id + 5),
            },
            3 => match retired.len() {
                0 => SubmissionId(0),
                n => retired[self.rng.below(n as u64) as usize],
            },
            4 => match self.m.on_screen(output) {
                Some(id) => id,
                None => SubmissionId(u64::MAX),
            },
            5 => SubmissionId(self.m.next_id + self.rng.below(3)),
            _ => SubmissionId(1 + self.rng.below(self.m.next_id + 2)),
        };
        let copies = if kind == 1 { 2 } else { 1 };
        self.log(format!(
            "inject Latched {output:?} {submission:?} x{copies}"
        ));
        for _ in 0..copies {
            self.engine
                .backend_mut()
                .inject_event(BackendEvent::Latched { output, submission });
        }
        if self.rng.chance(1, 4) {
            self.dirty = true;
            return Ok(());
        }
        self.drain()
    }

    fn lose(&mut self) -> R {
        self.log("lose_device".into());
        self.engine.backend_mut().lose_device();
        self.fake_lost = true;
        self.expect_lost += 1;
        if self.rng.chance(1, 2) {
            self.dirty = true;
            return Ok(());
        }
        self.drain()?;
        ensure!(
            !self
                .m
                .subs
                .values()
                .any(|s| matches!(s.fate, Fate::Pending | Fate::OnScreen)),
            "submissions survive a lost device"
        );
        Ok(())
    }

    fn restore(&mut self) -> R {
        let refresh = 60_000 + self.rng.below(2) as u32 * 30_000;
        self.log("restore".into());
        self.engine
            .backend_mut()
            .restore(caps(self.replace, refresh));
        self.fake_lost = false;
        self.expect_caps += 1;
        if self.rng.chance(1, 3) {
            self.dirty = true;
            return Ok(());
        }
        self.drain()
    }

    fn change_caps(&mut self) -> R {
        // The fake's own rule changes at once; the engine learns at the
        // event, which the next present takes first.
        self.replace = !self.replace;
        self.log(format!("set_capabilities replace_pending={}", self.replace));
        self.engine
            .backend_mut()
            .set_capabilities(caps(self.replace, 60_000));
        self.expect_caps += 1;
        self.drain()
    }

    fn raw_events(&mut self) -> R {
        let event = if self.rng.chance(1, 2) || self.in_armed || self.after_armed {
            self.expect_caps += 1;
            BackendEvent::CapabilitiesChanged
        } else {
            self.expect_lost += 1;
            BackendEvent::DeviceLost
        };
        self.log(format!("inject {event:?}"));
        self.engine.backend_mut().inject_event(event);
        self.drain()
    }

    fn advance(&mut self) -> R {
        self.m.now += self.rng.below(4);
        let now = self.m.now;
        self.log(format!("advance_time {now}"));
        self.engine.advance_time(now);
        self.drain()?;
        if let Some(timeout) = self.m.timeout {
            for (id, sub) in &self.m.subs {
                ensure!(
                    !(sub.fate == Fate::Pending
                        && !sub.reported
                        && now.saturating_sub(sub.submitted_at) >= timeout),
                    "{id:?} is overdue at {now} (timeout {timeout}) but no LatchTimeout came"
                );
            }
        }
        Ok(())
    }

    fn set_timeout(&mut self) -> R {
        let timeout = if self.rng.chance(1, 4) {
            None
        } else {
            Some(1 + self.rng.below(5))
        };
        self.log(format!("set_latch_timeout {timeout:?}"));
        self.engine.set_latch_timeout(timeout);
        self.m.timeout = timeout;
        Ok(())
    }

    fn step(&mut self) -> R {
        match self.rng.below(100) {
            0..=5 => self.register(),
            6..=15 => self.begin_write(),
            16..=25 => self.finish_write(),
            26..=29 => self.unregister(),
            30..=57 => self.present(),
            58..=70 => self.vsync(),
            71..=82 => self.inject_latch(),
            83..=88 => self.advance(),
            89..=90 => self.set_timeout(),
            91..=93 => self.lose(),
            94..=96 => {
                if self.fake_lost || self.rng.chance(1, 4) {
                    self.restore()
                } else {
                    self.raw_events()
                }
            }
            97 => self.change_caps(),
            _ => self.raw_events(),
        }
    }

    /// Ends the run: the device comes back, every pending frame latches and
    /// nothing is owed.
    fn finish(&mut self) -> R {
        self.log("finish".into());
        self.drain()?;
        if self.fake_lost {
            self.restore()?;
            self.drain()?;
        }
        for _ in 0..3 {
            self.engine.backend_mut().vsync();
            self.drain()?;
        }
        ensure!(
            !self.m.subs.values().any(|s| s.fate == Fate::Pending),
            "a submission is still pending after every latch"
        );
        ensure!(
            self.m.owed.values().all(|n| *n == 0),
            "buffers owed a Released: {:?}",
            self.m.owed
        );
        // A buffer is released each time the last submission holding it is
        // retired, and then only.
        for (buffer, zeroed) in &self.m.zeroed {
            let released = self.m.releases.get(buffer).copied().unwrap_or(0);
            ensure!(
                released == *zeroed,
                "{buffer:?}: released {released} times, became idle {zeroed} times"
            );
        }
        Ok(())
    }
}

fn run_seed(seed: u64, steps: usize) -> Result<(), String> {
    let trace = RefCell::new(Vec::new());
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| -> R {
        let mut h = Harness::new(seed, &trace);
        for index in 0..steps {
            trace.borrow_mut().push(format!("step {index}"));
            h.step().map_err(|e| format!("step {index}: {e}"))?;
            if !h.dirty {
                h.check_consistency()
                    .map_err(|e| format!("step {index}: {e}"))?;
            }
        }
        h.finish()
    }));
    let failure = match outcome {
        Ok(Ok(())) => return Ok(()),
        Ok(Err(message)) => message,
        Err(panic) => {
            let text = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            format!("panic: {text}")
        }
    };
    Err(format!(
        "seed {seed}: {failure}\ntrace (last 80 lines):\n{}",
        trace.borrow()[trace.borrow().len().saturating_sub(80)..].join("\n")
    ))
}

fn env_number(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}

#[test]
fn random_event_order_keeps_the_invariants() {
    if let Some(seed) = env_number("EVENT_ORDER_SEED") {
        if let Err(report) = run_seed(seed, 200) {
            panic!("{report}");
        }
        return;
    }
    let seeds = env_number("EVENT_ORDER_SEEDS").unwrap_or(300);
    let mut failures = Vec::new();
    for seed in 0..seeds {
        if let Err(report) = run_seed(seed, 150) {
            failures.push(report);
            if failures.len() >= 3 {
                break;
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

// ---------------------------------------------------------------------------
// Directed cases
// ---------------------------------------------------------------------------

fn engine_with_buffers(replace_pending: bool) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(caps(replace_pending, 60_000)));
    for id in 1..=POOL {
        engine.register_buffer(BufferId(id), info()).unwrap();
        engine.finish_write(BufferId(id)).unwrap();
    }
    engine
}

fn show(
    engine: &mut Engine<FakeBackend>,
    output: OutputId,
    buffers: &[BufferId],
) -> Result<SubmissionId, Error> {
    let layers = layers(buffers);
    engine.present(&Frame {
        output,
        layers: &layers,
    })
}

fn drain_events(engine: &mut Engine<FakeBackend>) -> Vec<Result<DisplayEvent, Error>> {
    let mut events = Vec::new();
    loop {
        match engine.poll_event() {
            Ok(None) => return events,
            Ok(Some(event)) => events.push(Ok(event)),
            Err(error) => events.push(Err(error)),
        }
    }
}

const O0: OutputId = OutputId(0);
const B1: BufferId = BufferId(1);
const B2: BufferId = BufferId(2);

fn unexpected(output: OutputId, submission: SubmissionId) -> Error {
    Error::Present(PresentError::UnexpectedLatch { output, submission })
}

#[test]
fn spurious_latch_queued_before_a_commit_fails_it_without_side_effects() {
    let mut engine = engine_with_buffers(true);
    let bogus = SubmissionId(77);
    engine.backend_mut().inject_event(BackendEvent::Latched {
        output: O0,
        submission: bogus,
    });
    let commits = engine.backend().commit_count();
    assert_eq!(show(&mut engine, O0, &[B1]), Err(unexpected(O0, bogus)));
    assert_eq!(engine.backend().commit_count(), commits);
    assert_eq!(engine.buffer_state(B1), Some(BufferState::Ready));
    // The spurious event is gone; the frame goes through on retry.
    let id = show(&mut engine, O0, &[B1]).unwrap();
    assert_eq!(id, SubmissionId(1));
    assert_eq!(engine.buffer_state(B1), Some(BufferState::Submitted));
}

#[test]
fn events_after_device_lost_are_errors_and_change_nothing() {
    let mut engine = engine_with_buffers(true);
    let id = show(&mut engine, O0, &[B1]).unwrap();
    engine.backend_mut().vsync();
    let second = show(&mut engine, O0, &[B2]).unwrap();
    engine.backend_mut().lose_device();
    // Late and duplicate latches of what was lost, queued behind the loss.
    for submission in [id, second, second] {
        engine.backend_mut().inject_event(BackendEvent::Latched {
            output: O0,
            submission,
        });
    }
    let events = drain_events(&mut engine);
    assert_eq!(
        events,
        [
            Ok(DisplayEvent::Presented {
                output: O0,
                submission: id
            }),
            Ok(DisplayEvent::Dropped {
                output: O0,
                submission: second
            }),
            // Pending buffers before displayed ones; the order between
            // buffers is not part of the contract.
            Ok(DisplayEvent::Released { buffer: B2 }),
            Ok(DisplayEvent::Released { buffer: B1 }),
            Ok(DisplayEvent::DeviceLost),
            Err(unexpected(O0, id)),
            Err(unexpected(O0, second)),
            Err(unexpected(O0, second)),
        ]
    );
    assert_eq!(engine.buffer_state(B1), Some(BufferState::Ready));
    assert_eq!(engine.buffer_state(B2), Some(BufferState::Ready));
    engine.begin_write(B1).unwrap();
    engine.unregister_buffer(B2).unwrap();
}

#[test]
fn a_flood_of_events_is_neither_lost_nor_reordered() {
    let mut engine = engine_with_buffers(true);
    let id = show(&mut engine, O0, &[B1]).unwrap();
    for _ in 0..200 {
        engine
            .backend_mut()
            .inject_event(BackendEvent::CapabilitiesChanged);
    }
    engine.backend_mut().vsync();
    for _ in 0..200 {
        engine
            .backend_mut()
            .inject_event(BackendEvent::CapabilitiesChanged);
    }
    // A frame while the backend queue is flooded may be refused for lack of
    // room, but only with the documented error, and a retry after draining
    // works.
    let mut events = Vec::new();
    let mut second = None;
    for _ in 0..1000 {
        match show(&mut engine, O0, &[B2]) {
            Ok(id) => {
                second = Some(id);
                break;
            }
            Err(Error::Present(PresentError::EventQueueFull)) => {
                events.extend(drain_events(&mut engine));
            }
            Err(error) => panic!("unexpected {error:?}"),
        }
    }
    assert!(second.is_some(), "the frame was never accepted");
    events.extend(drain_events(&mut engine));
    let caps_events = events
        .iter()
        .filter(|e| **e == Ok(DisplayEvent::CapabilitiesChanged))
        .count();
    assert_eq!(caps_events, 400, "events were lost or duplicated");
    let presented: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, Ok(DisplayEvent::Presented { .. })))
        .collect();
    assert_eq!(
        presented,
        [&Ok(DisplayEvent::Presented {
            output: O0,
            submission: id
        })]
    );
}

/// Ordered after the loss: a latch for the replaced submission must not
/// be taken for the new one.
#[test]
fn latch_of_a_replaced_submission_is_not_the_new_one() {
    let mut engine = engine_with_buffers(true);
    let old = show(&mut engine, O0, &[B1]).unwrap();
    let new = show(&mut engine, O0, &[B2]).unwrap();
    engine.backend_mut().inject_event(BackendEvent::Latched {
        output: O0,
        submission: old,
    });
    assert_eq!(
        drain_events(&mut engine),
        [
            Ok(DisplayEvent::Dropped {
                output: O0,
                submission: old
            }),
            Ok(DisplayEvent::Released { buffer: B1 }),
            Err(unexpected(O0, old)),
        ]
    );
    assert_eq!(engine.buffer_state(B1), Some(BufferState::Ready));
    assert_eq!(engine.buffer_state(B2), Some(BufferState::Submitted));
    engine.backend_mut().vsync();
    assert_eq!(
        drain_events(&mut engine),
        [Ok(DisplayEvent::Presented {
            output: O0,
            submission: new
        })]
    );
}

/// Regression: a latch that matched nothing, seen while a commit the
/// backend had accepted collected its events, failed the commit. The
/// hardware then held a frame whose buffers the engine counted as idle.
#[test]
fn duplicate_latch_in_commit_does_not_fail_the_accepted_frame() {
    let mut engine = engine_with_buffers(true);
    let first = show(&mut engine, O0, &[B1]).unwrap();
    engine.backend_mut().vsync();
    assert_eq!(
        drain_events(&mut engine),
        [Ok(DisplayEvent::Presented {
            output: O0,
            submission: first
        })]
    );
    // The hardware reports the latch of `first` again while the next frame
    // is being committed.
    engine
        .backend_mut()
        .inject_event_in_next_commit(BackendEvent::Latched {
            output: O0,
            submission: first,
        });
    let second = show(&mut engine, O0, &[B2]).unwrap();
    assert_eq!(engine.backend().pending(O0), Some(second));
    assert_eq!(engine.buffer_state(B2), Some(BufferState::Submitted));
    assert_eq!(engine.buffer_state(B1), Some(BufferState::Scanning));
    // The duplicate is still reported, once, and changed nothing.
    assert_eq!(drain_events(&mut engine), [Err(unexpected(O0, first))]);
    engine.backend_mut().vsync();
    assert_eq!(
        drain_events(&mut engine),
        [
            Ok(DisplayEvent::Presented {
                output: O0,
                submission: second
            }),
            Ok(DisplayEvent::Released { buffer: B1 }),
        ]
    );
}

/// The same with the replaced pending submission latched inside the commit
/// next to the spurious latches: it is recorded before the new submission,
/// and the spurious ones are reported after the commit's own events, in the
/// order they came.
#[test]
fn spurious_latches_in_commit_are_reported_after_its_events() {
    let mut engine = engine_with_buffers(true);
    let first = show(&mut engine, O0, &[B1]).unwrap();
    engine.backend_mut().vsync_in_next_commit();
    engine
        .backend_mut()
        .inject_event_in_next_commit(BackendEvent::Latched {
            output: OutputId(9),
            submission: SubmissionId(40),
        });
    let second = show(&mut engine, O0, &[B2]).unwrap();
    engine.backend_mut().inject_event(BackendEvent::Latched {
        output: O0,
        submission: SubmissionId(41),
    });
    assert_eq!(
        drain_events(&mut engine),
        [
            Ok(DisplayEvent::Presented {
                output: O0,
                submission: first
            }),
            Err(unexpected(OutputId(9), SubmissionId(40))),
            Err(unexpected(O0, SubmissionId(41))),
        ]
    );
    assert_eq!(engine.buffer_state(B1), Some(BufferState::Scanning));
    assert_eq!(engine.buffer_state(B2), Some(BufferState::Submitted));
    assert_eq!(engine.backend().pending(O0), Some(second));
}

/// Regression: a latch on another output that carried the new submission's
/// id was taken for the commit's own latch and then failed the accepted
/// commit. Ids are reused after a commit that failed once the backend had
/// taken it; here a DeviceLost the backend reports while it keeps the frame.
#[test]
fn latch_on_another_output_with_the_new_id_is_not_the_commits_own() {
    let mut engine = engine_with_buffers(true);
    let o1 = OUTS[1];
    engine
        .backend_mut()
        .inject_event_in_next_commit(BackendEvent::DeviceLost);
    assert_eq!(
        show(&mut engine, o1, &[B1]),
        Err(Error::Backend(BackendError::DeviceLost))
    );
    assert_eq!(drain_events(&mut engine), [Ok(DisplayEvent::DeviceLost)]);
    let stale = engine.backend().pending(o1).unwrap();
    engine.backend_mut().vsync_in_next_commit();
    let id = show(&mut engine, O0, &[B2]).unwrap();
    assert_eq!(id, stale);
    assert_eq!(engine.backend().pending(O0), Some(id));
    assert_eq!(engine.buffer_state(B2), Some(BufferState::Submitted));
    assert_eq!(drain_events(&mut engine), [Err(unexpected(o1, stale))]);
    engine.backend_mut().vsync();
    assert_eq!(
        drain_events(&mut engine),
        [Ok(DisplayEvent::Presented {
            output: O0,
            submission: id
        })]
    );
}

/// One plane per output, as a backend might announce after a mode change.
fn one_plane_caps() -> Capabilities {
    let mut reduced = caps(true, 60_000);
    reduced.planes.clear();
    reduced
        .planes
        .push(plane(10, PlaneKind::Primary, 0, 0))
        .unwrap();
    reduced
        .planes
        .push(plane(20, PlaneKind::Primary, 0, 1))
        .unwrap();
    reduced
}

/// Regression: `Engine::commit` learnt of a queued capability change only
/// while it collected the backend's events, after it had checked the plan's
/// generation, and committed a two-plane plan to a one-plane backend.
#[test]
fn commit_refuses_a_plan_made_stale_by_a_queued_capability_change() {
    let mut engine = engine_with_buffers(true);
    let frame_layers = layers(&[B1, B2]);
    let frame = Frame {
        output: O0,
        layers: &frame_layers,
    };
    let plan = engine.validate(&frame).unwrap();
    // The backend now has one plane per output; the engine has not polled.
    engine.backend_mut().set_capabilities(one_plane_caps());
    let commits = engine.backend().commit_count();
    let result = engine.commit(&frame, &plan);
    assert_eq!(
        result,
        Err(Error::StalePlan),
        "a two-plane plan was committed to a backend that announced one plane"
    );
    assert_eq!(engine.backend().commit_count(), commits);
    assert_eq!(engine.backend().pending(O0), None);
    assert_eq!(engine.buffer_state(B1), Some(BufferState::Ready));
    assert_eq!(engine.buffer_state(B2), Some(BufferState::Ready));
    assert_eq!(engine.capabilities().planes.len(), 2);
    // The change is still reported, once.
    assert_eq!(
        drain_events(&mut engine),
        [Ok(DisplayEvent::CapabilitiesChanged)]
    );
    // Planned again, the frame no longer fits; one layer does.
    assert!(matches!(engine.validate(&frame), Err(Error::Plan(_))));
    let single = layers(&[B1]);
    let single = Frame {
        output: O0,
        layers: &single,
    };
    let plan = engine.validate(&single).unwrap();
    assert_eq!(engine.commit(&single, &plan), Ok(SubmissionId(1)));
}

/// Regression, the same through `Engine::present`: it plans against the
/// capabilities the backend announced before the call, instead of failing
/// with a stale plan of its own making or committing an outdated one.
#[test]
fn present_plans_against_a_queued_capability_change() {
    let mut engine = engine_with_buffers(true);
    engine.backend_mut().set_capabilities(one_plane_caps());
    let commits = engine.backend().commit_count();
    assert!(matches!(
        show(&mut engine, O0, &[B1, B2]),
        Err(Error::Plan(_))
    ));
    assert_eq!(engine.backend().commit_count(), commits);
    assert_eq!(engine.buffer_state(B1), Some(BufferState::Ready));
    engine.backend_mut().set_capabilities(caps(true, 60_000));
    let id = show(&mut engine, O0, &[B1, B2]).unwrap();
    assert_eq!(engine.backend().scanout(O0), None);
    assert_eq!(engine.backend().pending(O0), Some(id));
    assert_eq!(
        drain_events(&mut engine),
        [
            Ok(DisplayEvent::CapabilitiesChanged),
            Ok(DisplayEvent::CapabilitiesChanged)
        ]
    );
}
