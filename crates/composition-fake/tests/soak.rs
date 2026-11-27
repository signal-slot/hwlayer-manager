//! Seeded soak of the engine on the fake backend (SoW §10): malformed frame
//! descriptions interleaved with adversarial event ordering.
//!
//! For each seed of a fixed list a SplitMix64 generator drives thousands of
//! steps: valid and malformed frames (zero-size or out-of-buffer regions,
//! too many layers, unknown formats, unknown or unfinished buffers, unknown
//! outputs, duplicated buffer ids), vertical blanks that are taken, missing
//! or repeated, latch events that are spurious, duplicated, stale or for
//! another output, device loss and recovery (also reported only as an event),
//! capability changes and replaced pending submissions.
//!
//! A model kept here, fed by the calls' results and by the engine's events,
//! is checked after every step:
//!
//! * a buffer is released only after the submissions holding it were
//!   retired, and once per time it became idle;
//! * at most one submission is pending and one latched per output, and only
//!   the pending one can be latched;
//! * a frame the engine refuses changes no state: no event, no buffer state,
//!   no commit, no trace entry;
//! * a frame that is surely malformed is never accepted, a surely valid one
//!   is, and a device that is lost refuses everything;
//! * after a flush, or a loss and recovery, every submitted buffer was
//!   released exactly once;
//! * the engine never panics.
//!
//! Each seed runs twice with a [`Trace`] sink attached and the two traces
//! must be identical; the trace must agree with the model's counts.
//!
//! A failing seed prints the seed and the last steps. Replay one with
//! `SOAK_SEED=<n>`; run more seeds with `SOAK_SEEDS=<n>`.
//!
//! Bugs found are kept as `#[ignore]`d reproducers at the end of the file.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use composition_core::{
    BackendEvent, BlendMode, BufferId, BufferInfo, BufferState, Capabilities, DisplayEvent, Engine,
    Error, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat, PlaneCaps,
    PlaneId, PlaneKind, PresentError, Rect, Size, SubmissionId, Trace, TraceEvent, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const OUT_SIZE: Size = Size::new(1280, 720);
const FULL: Rect = Rect::new(0, 0, 1280, 720);

/// Buffers 1..=4 are good ARGB8888 and rotate; the others are fixed.
const GOOD: [BufferId; 4] = [BufferId(1), BufferId(2), BufferId(3), BufferId(4)];
const WEIRD_FORMAT: BufferId = BufferId(5);
const ZERO_SIZE: BufferId = BufferId(6);
const UNKNOWN: BufferId = BufferId(9);
const KNOWN: [BufferId; 6] = [
    BufferId(1),
    BufferId(2),
    BufferId(3),
    BufferId(4),
    WEIRD_FORMAT,
    ZERO_SIZE,
];

const SEEDS: [u64; 8] = [
    1,
    2,
    3,
    0x9E37_79B9_7F4A_7C15,
    0x0123_4567_89AB_CDEF,
    0xDEAD_BEEF_CAFE_F00D,
    42,
    2024,
];
const STEPS: usize = 4000;

type R = Result<(), String>;

macro_rules! ensure {
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            return Err(format!($($arg)+));
        }
    };
}

// ---------------------------------------------------------------------------
// Generator and fixture
// ---------------------------------------------------------------------------

/// SplitMix64.
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

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
}

fn plane(id: u32, kind: PlaneKind, zpos: i32) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs: 1,
        zpos: Zpos::Fixed(zpos),
        blend_modes: BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask(),
        ..PlaneCaps::default()
    };
    caps.formats
        .push(FormatModifier::linear(PixelFormat::ARGB8888))
        .unwrap();
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
            size: OUT_SIZE,
            refresh_mhz,
        })
        .unwrap();
    caps.planes.push(plane(10, PlaneKind::Primary, 0)).unwrap();
    caps.planes.push(plane(11, PlaneKind::Overlay, 1)).unwrap();
    caps
}

fn info(size: Size, format: PixelFormat) -> BufferInfo {
    BufferInfo {
        size,
        format,
        modifier: Modifier::LINEAR,
    }
}

fn fixture<T: Trace>(replace: bool, trace: T) -> Engine<FakeBackend, T> {
    let mut engine = Engine::new(FakeBackend::new(caps(replace, 60_000))).with_trace(trace);
    let table = [
        (GOOD[0], OUT_SIZE, PixelFormat::ARGB8888),
        (GOOD[1], OUT_SIZE, PixelFormat::ARGB8888),
        (GOOD[2], OUT_SIZE, PixelFormat::ARGB8888),
        (GOOD[3], OUT_SIZE, PixelFormat::ARGB8888),
        (WEIRD_FORMAT, OUT_SIZE, PixelFormat(0xDEAD_BEEF)),
        (ZERO_SIZE, Size::new(0, 0), PixelFormat::ARGB8888),
    ];
    for (buffer, size, format) in table {
        engine.register_buffer(buffer, info(size, format)).unwrap();
        engine.finish_write(buffer).unwrap();
    }
    engine
}

// ---------------------------------------------------------------------------
// Trace sink
// ---------------------------------------------------------------------------

/// What the sink kept: every entry as text, and counts of the kinds the model
/// can be compared with.
#[derive(Default)]
struct Log {
    lines: Vec<String>,
    commits: u64,
    latched: u64,
    released: u64,
    dropped: u64,
    lost: u64,
    caps: u64,
}

#[derive(Clone, Default)]
struct Sink(Rc<RefCell<Log>>);

impl Trace for Sink {
    fn record(&mut self, at: u64, event: TraceEvent<'_>) {
        let mut log = self.0.borrow_mut();
        match &event {
            TraceEvent::Commit { result: Ok(_), .. } => log.commits += 1,
            TraceEvent::Latched { .. } => log.latched += 1,
            TraceEvent::Released { .. } => log.released += 1,
            TraceEvent::Dropped { .. } => log.dropped += 1,
            TraceEvent::DeviceLost => log.lost += 1,
            TraceEvent::CapabilitiesChanged => log.caps += 1,
            _ => {}
        }
        log.lines.push(format!("{at} {event:?}"));
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Sub {
    id: SubmissionId,
    buffers: Vec<BufferId>,
}

#[derive(Default)]
struct Model {
    /// `true` while the producer holds the buffer for writing.
    writable: BTreeMap<BufferId, bool>,
    pending: Option<Sub>,
    latched: Option<Sub>,
    next_id: u64,
    /// Releases due per buffer: it became idle and no `Released` came yet.
    owed: BTreeMap<BufferId, u32>,
    idle: BTreeMap<BufferId, u32>,
    released: BTreeMap<BufferId, u32>,
    /// Submissions retired by a replacement or a loss whose `Dropped` is due.
    dropped_due: BTreeSet<SubmissionId>,
    expect_lost: u64,
    expect_caps: u64,
    seen_lost: u64,
    seen_caps: u64,
    seen_presented: u64,
    seen_dropped: u64,
    seen_released: u64,
    accepted: u64,
    refused: u64,
    unexpected: u64,
}

impl Model {
    fn new(writable: impl IntoIterator<Item = BufferId>) -> Self {
        Self {
            writable: writable.into_iter().map(|b| (b, false)).collect(),
            next_id: 1,
            ..Self::default()
        }
    }

    fn held(&self, buffer: BufferId) -> bool {
        [&self.pending, &self.latched]
            .into_iter()
            .flatten()
            .any(|s| s.buffers.contains(&buffer))
    }

    /// A submission leaves the display; buffers nobody else holds fall idle.
    fn retire(&mut self, sub: &Sub) {
        for buffer in &sub.buffers {
            if !self.held(*buffer) {
                *self.owed.entry(*buffer).or_default() += 1;
                *self.idle.entry(*buffer).or_default() += 1;
            }
        }
    }

    /// Everything on the display is lost; the engine releases it.
    fn lose_all(&mut self) {
        if let Some(sub) = self.pending.take() {
            self.dropped_due.insert(sub.id);
            self.retire(&sub);
        }
        if let Some(sub) = self.latched.take() {
            self.retire(&sub);
        }
        self.expect_lost += 1;
    }

    fn apply(&mut self, event: DisplayEvent) -> R {
        match event {
            DisplayEvent::Presented { output, submission } => {
                ensure!(output == OUT, "Presented on {output:?}");
                ensure!(
                    self.pending.as_ref().map(|s| s.id) == Some(submission),
                    "Presented {submission:?} but pending is {:?}",
                    self.pending
                );
                let old = self.latched.take();
                self.latched = self.pending.take();
                if let Some(old) = old {
                    self.retire(&old);
                }
                self.seen_presented += 1;
            }
            DisplayEvent::Dropped { output, submission } => {
                ensure!(output == OUT, "Dropped on {output:?}");
                ensure!(
                    self.dropped_due.remove(&submission),
                    "Dropped {submission:?} which was not replaced or lost"
                );
                self.seen_dropped += 1;
            }
            DisplayEvent::Released { buffer } => {
                ensure!(
                    !self.held(buffer),
                    "{buffer:?} released while pending or displayed"
                );
                let owed = self.owed.entry(buffer).or_default();
                ensure!(*owed > 0, "{buffer:?} released twice or before it was idle");
                *owed -= 1;
                *self.released.entry(buffer).or_default() += 1;
                self.seen_released += 1;
            }
            DisplayEvent::DeviceLost => self.seen_lost += 1,
            DisplayEvent::CapabilitiesChanged => self.seen_caps += 1,
            DisplayEvent::LatchTimeout { .. } => {
                return Err("LatchTimeout without a timeout".into())
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    /// Must be accepted when nothing else stands in the way.
    Valid,
    /// Must be refused.
    Malformed,
    /// May go either way (duplicated buffers, regions off the output).
    Open,
}

struct Generated {
    output: OutputId,
    layers: Vec<Layer>,
    class: Class,
}

fn layer_pair(rng: &mut Rng, a: BufferId, b: BufferId) -> Vec<Layer> {
    let (w, h) = (1 + rng.below(1280) as u32, 1 + rng.below(720) as u32);
    let x = rng.below(1280 - w as u64 + 1) as i32;
    let y = rng.below(720 - h as u64 + 1) as i32;
    let bottom = Layer::new(a, FULL, FULL, 0);
    let top = Layer {
        blend: BlendMode::Premultiplied,
        ..Layer::new(b, Rect::new(0, 0, w, h), Rect::new(x, y, w, h), 1)
    };
    vec![bottom, top]
}

/// Whether any part of `rect` is on the output.
fn visible(rect: &Rect) -> bool {
    (rect.x as i64) < OUT_SIZE.width as i64
        && (rect.y as i64) < OUT_SIZE.height as i64
        && rect.x as i64 + rect.width as i64 > 0
        && rect.y as i64 + rect.height as i64 > 0
}

fn generate(rng: &mut Rng, model: &Model) -> Generated {
    let ready: Vec<BufferId> = GOOD
        .iter()
        .copied()
        .filter(|b| model.writable.get(b) == Some(&false))
        .collect();
    let mut layers = match (ready.len(), rng.below(4)) {
        (0, _) => Vec::new(),
        (1, _) | (_, 0) => vec![Layer::new(rng.pick(&ready), FULL, FULL, 0)],
        (_, _) => {
            let a = rng.pick(&ready);
            let others: Vec<_> = ready.iter().copied().filter(|b| *b != a).collect();
            let b = rng.pick(&others);
            layer_pair(rng, a, b)
        }
    };
    if rng.chance(1, 20) {
        layers.clear(); // blanks the output
    }
    let mut output = OUT;
    let mut class = Class::Valid;
    // Mutations, often more than one.
    while rng.chance(1, 3) {
        match rng.below(12) {
            0 if !layers.is_empty() => {
                let i = rng.below(layers.len() as u64) as usize;
                layers[i].source.width = 0;
                class = Class::Malformed;
            }
            1 if !layers.is_empty() => {
                let i = rng.below(layers.len() as u64) as usize;
                layers[i].destination.height = 0;
                class = Class::Malformed;
            }
            2 if !layers.is_empty() => {
                let i = rng.below(layers.len() as u64) as usize;
                layers[i].source = Rect::new(-1 - rng.below(100) as i32, 0, 10, 10);
                class = Class::Malformed;
            }
            3 if !layers.is_empty() => {
                let i = rng.below(layers.len() as u64) as usize;
                layers[i].source = Rect::new(
                    rng.pick(&[i32::MAX, 1279, 5000]),
                    rng.pick(&[0, i32::MAX, 719]),
                    rng.pick(&[1, 2, u32::MAX]),
                    rng.pick(&[1, 721, u32::MAX]),
                );
                class = Class::Malformed;
            }
            4 => {
                let n = 17 + rng.below(4) as usize;
                let seed = layers
                    .first()
                    .copied()
                    .unwrap_or(Layer::new(GOOD[0], FULL, FULL, 0));
                layers = vec![seed; n];
                class = Class::Malformed;
            }
            5 if !layers.is_empty() => {
                let i = rng.below(layers.len() as u64) as usize;
                layers[i].buffer = WEIRD_FORMAT;
                class = Class::Malformed;
            }
            6 if !layers.is_empty() => {
                let i = rng.below(layers.len() as u64) as usize;
                layers[i].buffer = ZERO_SIZE;
                class = Class::Malformed;
            }
            7 if !layers.is_empty() => {
                let i = rng.below(layers.len() as u64) as usize;
                layers[i].buffer = rng.pick(&[UNKNOWN, BufferId(u32::MAX), BufferId(0)]);
                class = Class::Malformed;
            }
            8 => {
                output = OutputId(1 + rng.below(1000) as u32);
                class = Class::Malformed;
            }
            9 if layers.len() == 2 && class == Class::Valid => {
                // Duplicated buffer id.
                layers[1].buffer = layers[0].buffer;
                class = Class::Open;
            }
            10 if !layers.is_empty() => {
                // Destination partly or wholly off the output.
                let i = rng.below(layers.len() as u64) as usize;
                layers[i].destination.x = rng.pick(&[-100, 1200, i32::MAX - 5, i32::MIN]);
                if class == Class::Valid {
                    class = Class::Open;
                }
            }
            _ => {}
        }
    }
    // A layer wholly off the output may be left out before its buffer is
    // looked at, so a defect of the buffer alone is not certain to show.
    if class == Class::Malformed
        && output == OUT
        && layers.len() <= 16
        && layers.iter().any(|l| !l.destination.fits_within(OUT_SIZE))
    {
        class = Class::Open;
    }
    Generated {
        output,
        layers,
        class,
    }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Harness {
    engine: Engine<FakeBackend, Sink>,
    log: Rc<RefCell<Log>>,
    m: Model,
    rng: Rng,
    replace: bool,
    fake_lost: bool,
    steps: Vec<String>,
}

impl Harness {
    fn new(seed: u64) -> Self {
        let mut rng = Rng(seed);
        let replace = rng.chance(1, 2);
        let sink = Sink::default();
        let log = sink.0.clone();
        let engine = fixture(replace, sink);
        let m = Model::new(KNOWN);
        Self {
            engine,
            log,
            m,
            rng,
            replace,
            fake_lost: false,
            steps: Vec::new(),
        }
    }

    fn note(&mut self, text: String) {
        self.steps.push(text);
    }

    /// Takes every event out of the engine and applies it to the model.
    /// Returns the number of `UnexpectedLatch` errors seen.
    fn drain(&mut self) -> Result<u32, String> {
        let mut unexpected = 0;
        for _ in 0..10_000 {
            match self.engine.poll_event() {
                Ok(None) => {
                    ensure!(
                        self.m.dropped_due.is_empty(),
                        "Dropped never came for {:?}",
                        self.m.dropped_due
                    );
                    ensure!(
                        self.m.owed.values().all(|n| *n == 0),
                        "Released never came for {:?} (pending {:?}, latched {:?}, states {:?})",
                        self.m.owed,
                        self.m.pending,
                        self.m.latched,
                        self.states()
                    );
                    return Ok(unexpected);
                }
                Ok(Some(event)) => {
                    self.note(format!("    event {event:?}"));
                    self.m.apply(event)?;
                }
                Err(Error::Present(PresentError::UnexpectedLatch { .. })) => {
                    self.note("    UnexpectedLatch".into());
                    unexpected += 1;
                }
                Err(error) => return Err(format!("poll_event gave {error:?}")),
            }
        }
        Err("poll_event did not run dry".into())
    }

    fn expected_state(&self, buffer: BufferId) -> Option<BufferState> {
        let writable = *self.m.writable.get(&buffer)?;
        let has = |s: &Option<Sub>| s.as_ref().is_some_and(|s| s.buffers.contains(&buffer));
        Some(if has(&self.m.latched) {
            BufferState::Scanning
        } else if has(&self.m.pending) {
            BufferState::Submitted
        } else if writable {
            BufferState::Writable
        } else {
            BufferState::Ready
        })
    }

    fn states(&self) -> Vec<Option<BufferState>> {
        (0..=10)
            .map(|id| self.engine.buffer_state(BufferId(id)))
            .collect()
    }

    /// The engine agrees with the model once every event was taken.
    fn check(&self) -> R {
        for id in 0..=10 {
            let buffer = BufferId(id);
            ensure!(
                self.engine.buffer_state(buffer) == self.expected_state(buffer),
                "{buffer:?} is {:?}, model says {:?}",
                self.engine.buffer_state(buffer),
                self.expected_state(buffer)
            );
        }
        ensure!(
            self.m.seen_lost == self.m.expect_lost,
            "{} DeviceLost seen, {} expected",
            self.m.seen_lost,
            self.m.expect_lost
        );
        ensure!(
            self.m.seen_caps == self.m.expect_caps,
            "{} CapabilitiesChanged seen, {} expected",
            self.m.seen_caps,
            self.m.expect_caps
        );
        let log = self.log.borrow();
        ensure!(
            log.commits == self.m.accepted
                && log.latched == self.m.seen_presented
                && log.dropped == self.m.seen_dropped
                && log.released == self.m.seen_released
                && log.lost == self.m.seen_lost
                && log.caps == self.m.seen_caps,
            "trace counts disagree with the model: commits {}/{} latched {}/{} dropped {}/{} released {}/{} lost {}/{} caps {}/{}",
            log.commits,
            self.m.accepted,
            log.latched,
            self.m.seen_presented,
            log.dropped,
            self.m.seen_dropped,
            log.released,
            self.m.seen_released,
            log.lost,
            self.m.seen_lost,
            log.caps,
            self.m.seen_caps
        );
        Ok(())
    }

    // --- operations --------------------------------------------------------

    fn present(&mut self) -> R {
        let generated = generate(&mut self.rng, &self.m);
        let Generated {
            output,
            layers,
            class,
        } = generated;
        self.note(format!("present {class:?} {output:?} {layers:?}"));
        let unready = layers
            .iter()
            .any(|l| self.m.writable.get(&l.buffer) == Some(&true));
        let busy = !self.replace && self.m.pending.is_some();
        let fake_busy = !self.replace && self.engine.backend().pending(OUT).is_some();
        let must_reject = class == Class::Malformed || unready || self.fake_lost || busy;
        let must_accept = class == Class::Valid && !must_reject && !fake_busy;

        let before = (
            self.states(),
            self.engine.backend().commit_count(),
            self.engine.backend().pending(OUT),
            self.log.borrow().lines.len(),
        );
        let frame = Frame {
            output,
            layers: &layers,
        };
        match self.engine.present(&frame) {
            Ok(id) => {
                ensure!(
                    !must_reject,
                    "a frame that must be refused ({class:?}, unready {unready}, lost {}, busy {busy}) was accepted as {id:?}",
                    self.fake_lost
                );
                ensure!(
                    id == SubmissionId(self.m.next_id),
                    "submission id {id:?}, expected {}",
                    self.m.next_id
                );
                self.m.next_id += 1;
                self.m.accepted += 1;
                // A layer wholly off the output shows nothing and holds no
                // buffer.
                let mut buffers: Vec<_> = layers
                    .iter()
                    .filter(|l| visible(&l.destination))
                    .map(|l| l.buffer)
                    .collect();
                buffers.sort();
                buffers.dedup();
                if let Some(old) = self.m.pending.replace(Sub { id, buffers }) {
                    self.m.dropped_due.insert(old.id);
                    self.m.retire(&old);
                }
                self.drain()?;
            }
            Err(error) => {
                ensure!(
                    !must_accept,
                    "a valid frame was refused with {error:?} (lost {})",
                    self.fake_lost
                );
                // A refused frame changes nothing.
                let after = (
                    self.states(),
                    self.engine.backend().commit_count(),
                    self.engine.backend().pending(OUT),
                    self.log.borrow().lines.len(),
                );
                ensure!(
                    before.0 == after.0 && before.1 == after.1 && before.2 == after.2,
                    "a refused frame ({error:?}) changed state: {before:?} -> {after:?}"
                );
                let new = self.log.borrow().lines[before.3..].to_vec();
                ensure!(
                    new.iter()
                        .all(|l| !l.contains("Commit { output: OutputId(0), result: Ok")
                            && !l.contains("Dropped")
                            && !l.contains("Released")
                            && !l.contains("Latched")),
                    "a refused frame ({error:?}) traced side effects: {new:?}"
                );
                self.m.refused += 1;
                let events = self.drain()?;
                ensure!(events == 0, "a refused frame queued a spurious latch");
            }
        }
        Ok(())
    }

    fn vsync(&mut self) -> R {
        self.note("vsync".into());
        self.engine.backend_mut().vsync();
        self.drain()?;
        Ok(())
    }

    /// A latch event the hardware did not send, or sent twice.
    fn inject_latch(&mut self) -> R {
        let pending = self.m.pending.as_ref().map(|s| s.id);
        let latched = self.m.latched.as_ref().map(|s| s.id);
        let (output, submission) = match self.rng.below(6) {
            0 | 1 => match pending {
                Some(id) => (OUT, id),
                None => return Ok(()),
            },
            2 => (OUT, latched.unwrap_or(SubmissionId(0))),
            3 => (
                OutputId(1 + self.rng.below(5) as u32),
                pending.unwrap_or(SubmissionId(1)),
            ),
            4 => (OUT, SubmissionId(self.m.next_id + self.rng.below(3))),
            _ => (OUT, SubmissionId(self.rng.below(self.m.next_id + 1))),
        };
        let copies = 1 + self.rng.below(2) as usize;
        self.note(format!(
            "inject Latched {output:?} {submission:?} x{copies}"
        ));
        for _ in 0..copies {
            self.engine
                .backend_mut()
                .inject_event(BackendEvent::Latched { output, submission });
        }
        let matches = output == OUT && Some(submission) == pending;
        let unexpected = self.drain()?;
        self.m.unexpected += u64::from(unexpected);
        let want = copies as u32 - u32::from(matches);
        ensure!(
            unexpected == want,
            "{copies} latch(es) of {submission:?} on {output:?} gave {unexpected} UnexpectedLatch, expected {want}"
        );
        Ok(())
    }

    fn lose(&mut self) -> R {
        self.note("lose_device".into());
        self.engine.backend_mut().lose_device();
        self.fake_lost = true;
        self.m.lose_all();
        self.drain()?;
        Ok(())
    }

    fn restore(&mut self) -> R {
        let refresh = 60_000 + self.rng.below(2) as u32 * 30_000;
        self.note("restore".into());
        self.engine
            .backend_mut()
            .restore(caps(self.replace, refresh));
        self.fake_lost = false;
        self.m.expect_caps += 1;
        self.drain()?;
        Ok(())
    }

    fn change_caps(&mut self) -> R {
        self.replace = !self.replace;
        self.note(format!("set_capabilities replace_pending={}", self.replace));
        self.engine
            .backend_mut()
            .set_capabilities(caps(self.replace, 60_000));
        self.m.expect_caps += 1;
        self.drain()?;
        Ok(())
    }

    /// The loss or the change only as an event of the backend's queue.
    fn raw_event(&mut self) -> R {
        if self.rng.chance(1, 2) || self.fake_lost {
            self.note("inject CapabilitiesChanged".into());
            self.engine
                .backend_mut()
                .inject_event(BackendEvent::CapabilitiesChanged);
            self.m.expect_caps += 1;
        } else {
            self.note("inject DeviceLost".into());
            self.engine
                .backend_mut()
                .inject_event(BackendEvent::DeviceLost);
            self.m.lose_all();
        }
        self.drain()?;
        Ok(())
    }

    fn begin_write(&mut self) -> R {
        let buffer = self
            .rng
            .pick(&[GOOD[0], GOOD[1], GOOD[2], GOOD[3], UNKNOWN]);
        self.note(format!("begin_write {buffer:?}"));
        let result = self.engine.begin_write(buffer);
        if !self.m.writable.contains_key(&buffer) {
            ensure!(
                result == Err(Error::Present(PresentError::UnknownBuffer(buffer))),
                "begin_write of unknown gave {result:?}"
            );
        } else if self.m.held(buffer) {
            ensure!(
                result == Err(Error::Present(PresentError::BufferInUse(buffer))),
                "begin_write of a buffer in use gave {result:?}"
            );
        } else {
            ensure!(result.is_ok(), "begin_write gave {result:?}");
            self.m.writable.insert(buffer, true);
        }
        Ok(())
    }

    fn finish_write(&mut self) -> R {
        let buffer = self
            .rng
            .pick(&[GOOD[0], GOOD[1], GOOD[2], GOOD[3], UNKNOWN]);
        self.note(format!("finish_write {buffer:?}"));
        let result = self.engine.finish_write(buffer);
        match self.m.writable.get(&buffer).copied() {
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
                self.m.writable.insert(buffer, false);
            }
        }
        Ok(())
    }

    fn step(&mut self) -> R {
        match self.rng.below(100) {
            0..=44 => self.present(),
            45..=62 => self.vsync(),
            63..=77 => self.inject_latch(),
            78..=84 => self.begin_write(),
            85..=90 => self.finish_write(),
            91..=93 => {
                if self.fake_lost {
                    self.restore()
                } else {
                    self.lose()
                }
            }
            94..=95 if self.fake_lost => self.restore(),
            94..=96 => self.change_caps(),
            _ => self.raw_event(),
        }
    }

    /// Ends the run: every buffer must come back. Even seeds blank the
    /// output and let the last frame leave the screen, odd seeds lose the
    /// device and recover it.
    fn flush(&mut self, odd: bool) -> R {
        self.note(format!("flush odd={odd}"));
        self.drain()?;
        if self.fake_lost {
            self.restore()?;
        }
        // Whatever is pending latches, or is lost.
        self.vsync()?;
        if odd {
            self.lose()?;
            self.restore()?;
        } else {
            // Finish every buffer the producer still holds, then blank.
            let blank = Frame {
                output: OUT,
                layers: &[],
            };
            if self.replace || self.m.pending.is_none() {
                let id = self.engine.present(&blank);
                match id {
                    Ok(id) => {
                        self.m.next_id += 1;
                        self.m.accepted += 1;
                        ensure!(id == SubmissionId(self.m.next_id - 1), "blank frame id");
                        let sub = Sub {
                            id,
                            buffers: Vec::new(),
                        };
                        if let Some(old) = self.m.pending.replace(sub) {
                            self.m.dropped_due.insert(old.id);
                            self.m.retire(&old);
                        }
                        self.drain()?;
                    }
                    Err(error) => return Err(format!("the blank frame was refused: {error:?}")),
                }
            }
            self.vsync()?;
            self.vsync()?;
        }
        ensure!(
            self.m.pending.is_none() || self.m.pending.as_ref().unwrap().buffers.is_empty(),
            "a submission with buffers is still pending after the flush"
        );
        ensure!(
            self.m.latched.as_ref().is_none_or(|s| s.buffers.is_empty()),
            "a buffer is still on screen after the flush: {:?}",
            self.m.latched
        );
        ensure!(
            self.m.owed.values().all(|n| *n == 0),
            "buffers owed a Released: {:?}",
            self.m.owed
        );
        for (buffer, idle) in &self.m.idle {
            let released = self.m.released.get(buffer).copied().unwrap_or(0);
            ensure!(
                released == *idle,
                "{buffer:?}: released {released} times, became idle {idle} times"
            );
        }
        for buffer in GOOD {
            let state = self.engine.buffer_state(buffer);
            ensure!(
                matches!(state, Some(BufferState::Ready | BufferState::Writable)),
                "{buffer:?} is {state:?} after the flush"
            );
        }
        self.check()
    }
}

/// One run of one seed; returns the trace on success.
fn run_once(seed: u64, steps: usize) -> Result<Vec<String>, String> {
    let mut h = Harness::new(seed);
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| -> R {
        for index in 0..steps {
            h.note(format!("step {index}"));
            h.step().map_err(|e| format!("step {index}: {e}"))?;
            h.check().map_err(|e| format!("step {index}: {e}"))?;
        }
        h.flush(seed % 2 == 1)
    }));
    let failure = match outcome {
        Ok(Ok(())) => {
            let m = &h.m;
            // The generator reaches every kind of step.
            if steps >= 1000
                && (m.accepted < 100
                    || m.refused < 100
                    || m.seen_dropped == 0
                    || m.seen_lost == 0
                    || m.seen_caps == 0
                    || m.seen_presented < 50
                    || m.unexpected == 0)
            {
                return Err(format!("seed {seed}: the run was too tame"));
            }
            return Ok(h.log.borrow().lines.clone());
        }
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
    let tail = h.steps[h.steps.len().saturating_sub(40)..].join("\n");
    Err(format!("seed {seed}: {failure}\nlast steps:\n{tail}"))
}

fn run_seed(seed: u64, steps: usize) -> R {
    let first = run_once(seed, steps)?;
    let second = run_once(seed, steps)?;
    ensure!(
        first == second,
        "seed {seed}: the trace differs between two runs (first difference at entry {:?})",
        first.iter().zip(&second).position(|(a, b)| a != b)
    );
    ensure!(
        first.len() > steps / 4,
        "seed {seed}: the trace is too short"
    );
    Ok(())
}

fn env_number(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}

#[test]
fn soak_keeps_the_invariants() {
    if let Some(seed) = env_number("SOAK_SEED") {
        if let Err(report) = run_seed(seed, STEPS) {
            panic!("{report}");
        }
        return;
    }
    let extra = env_number("SOAK_SEEDS").unwrap_or(0);
    let mut failures = Vec::new();
    for seed in SEEDS.iter().copied().chain(1000..1000 + extra) {
        if let Err(report) = run_seed(seed, STEPS) {
            failures.push(report);
            if failures.len() >= 3 {
                break;
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
