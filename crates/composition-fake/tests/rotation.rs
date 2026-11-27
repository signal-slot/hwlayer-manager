//! Buffer rotation and event order under adversarial vsync timing.
//!
//! Two layers (an RGB565 application layer below an ARGB8888 safety layer,
//! each on its own plane) rotate three buffers over many frames. A seeded
//! schedule delays and skips vsyncs, replaces frames before they latch, makes
//! the vertical blank arrive inside a commit, and fails commits. A shadow
//! model is fed only from the engine's events and checks, after every step:
//!
//! * every submission ends as `Presented` (then superseded) or `Dropped`,
//!   exactly once, and a `Presented` never follows a `Dropped`;
//! * a buffer is `Released` exactly once per retirement, only when no pending
//!   or displayed submission references it, and never before the event that
//!   retired its submission;
//! * the engine's buffer states agree with the model, and the fake's scanout
//!   shows exactly the buffers of the last `Presented` submission;
//! * the counts add up.

use std::collections::BTreeMap;

use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, BufferState, Capabilities, DisplayEvent, Engine,
    Error, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat, PlaneCaps,
    PlaneId, PlaneKind, PresentError, Rect, Size, SubmissionId, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const FULL: Rect = Rect::new(0, 0, 1280, 720);
const BOTTOM: PlaneId = PlaneId(31);
const TOP: PlaneId = PlaneId(41);

/// Buffers of the application layer and of the safety layer.
const APP: [BufferId; 3] = [BufferId(1), BufferId(2), BufferId(3)];
const SAFE: [BufferId; 3] = [BufferId(11), BufferId(12), BufferId(13)];

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

fn new_engine(replace_pending: bool) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(caps(replace_pending)));
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

/// Small deterministic generator (Numerical Recipes LCG).
struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 >> 8
    }

    /// True with probability `num` in `den`.
    fn chance(&mut self, num: u32, den: u32) -> bool {
        self.next() % den < num
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fate {
    Pending,
    OnScreen,
    Superseded,
    Dropped,
}

#[derive(Default)]
struct Counts {
    submitted: u32,
    presented: u32,
    dropped: u32,
    refused_busy: u32,
    failed_commits: u32,
    skipped_vsyncs: u32,
    releases: BTreeMap<BufferId, u32>,
    uses: BTreeMap<BufferId, u32>,
}

/// Shadow of what the events say about submissions and buffers.
struct Model {
    subs: BTreeMap<SubmissionId, (Fate, Vec<BufferId>)>,
    /// References from pending or displayed submissions.
    held: BTreeMap<BufferId, u32>,
    /// Buffers retired but whose `Released` has not arrived yet.
    owed: BTreeMap<BufferId, u32>,
    last_presented: Option<SubmissionId>,
    on_screen: Option<SubmissionId>,
    counts: Counts,
}

impl Model {
    fn new() -> Self {
        Self {
            subs: BTreeMap::new(),
            held: BTreeMap::new(),
            owed: BTreeMap::new(),
            last_presented: None,
            on_screen: None,
            counts: Counts::default(),
        }
    }

    fn submitted(&mut self, id: SubmissionId, buffers: &[BufferId]) {
        if let Some(last) = self.subs.keys().last() {
            assert_eq!(id.0, last.0 + 1, "submission ids must stay dense");
        }
        self.counts.submitted += 1;
        for buffer in buffers {
            *self.held.entry(*buffer).or_default() += 1;
            *self.counts.uses.entry(*buffer).or_default() += 1;
        }
        self.subs.insert(id, (Fate::Pending, buffers.to_vec()));
    }

    fn pending(&self) -> Option<SubmissionId> {
        self.subs
            .iter()
            .find(|(_, (fate, _))| *fate == Fate::Pending)
            .map(|(id, _)| *id)
    }

    fn retire(&mut self, id: SubmissionId, fate: Fate) {
        let (old, buffers) = self.subs.get_mut(&id).unwrap();
        *old = fate;
        for buffer in buffers.clone() {
            let held = self.held.get_mut(&buffer).unwrap();
            *held -= 1;
            if *held == 0 {
                *self.owed.entry(buffer).or_default() += 1;
            }
        }
    }

    fn apply(&mut self, event: DisplayEvent) {
        match event {
            DisplayEvent::Presented { output, submission } => {
                assert_eq!(output, OUT);
                assert_eq!(
                    self.subs[&submission].0,
                    Fate::Pending,
                    "Presented for a submission that is not pending: {submission:?}"
                );
                assert!(
                    self.last_presented.is_none_or(|last| last < submission),
                    "latches out of order"
                );
                self.last_presented = Some(submission);
                self.subs.get_mut(&submission).unwrap().0 = Fate::OnScreen;
                if let Some(previous) = self.on_screen.replace(submission) {
                    self.retire(previous, Fate::Superseded);
                }
                self.counts.presented += 1;
            }
            DisplayEvent::Dropped { output, submission } => {
                assert_eq!(output, OUT);
                assert_eq!(
                    self.subs[&submission].0,
                    Fate::Pending,
                    "Dropped for a submission that is not pending: {submission:?}"
                );
                // Only a newer submission replaces a pending one.
                assert!(
                    self.subs
                        .keys()
                        .last()
                        .is_some_and(|last| *last > submission),
                    "{submission:?} dropped without a replacement"
                );
                self.retire(submission, Fate::Dropped);
                self.counts.dropped += 1;
            }
            DisplayEvent::Released { buffer } => {
                assert_eq!(
                    self.held.get(&buffer).copied().unwrap_or(0),
                    0,
                    "{buffer:?} released while pending or displayed"
                );
                let owed = self.owed.entry(buffer).or_default();
                assert!(
                    *owed > 0,
                    "{buffer:?} released without being retired first (or twice)"
                );
                *owed -= 1;
                *self.counts.releases.entry(buffer).or_default() += 1;
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    /// The engine and the fake must agree with what the events said.
    fn check_states(&self, engine: &Engine<FakeBackend>) {
        let mut pending = Vec::new();
        let mut shown = Vec::new();
        for (fate, buffers) in self.subs.values() {
            match fate {
                Fate::Pending => pending.extend(buffers),
                Fate::OnScreen => shown.extend(buffers),
                _ => {}
            }
        }
        for buffer in APP.into_iter().chain(SAFE) {
            let expected = if shown.contains(&&buffer) {
                BufferState::Scanning
            } else if pending.contains(&&buffer) {
                BufferState::Submitted
            } else {
                BufferState::Ready
            };
            assert_eq!(engine.buffer_state(buffer), Some(expected), "{buffer:?}");
        }
        assert_eq!(
            engine.backend().pending(OUT).is_some(),
            self.pending().is_some()
        );
        if let Some(on_screen) = self.on_screen {
            let mut scanned: Vec<BufferId> = engine
                .backend()
                .scanout(OUT)
                .unwrap()
                .iter()
                .map(|p| p.buffer)
                .collect();
            scanned.sort();
            let mut shown = self.subs[&on_screen].1.clone();
            shown.sort();
            assert_eq!(scanned, shown, "fake scanout differs from last Presented");
        }
    }

    fn check_totals(&self, replace_pending: bool) {
        let c = &self.counts;
        assert!(self.pending().is_none() && self.owed.values().all(|n| *n == 0));
        // Every accepted submission was either shown or dropped.
        assert_eq!(c.submitted, c.presented + c.dropped);
        if !replace_pending {
            assert_eq!(c.dropped, 0, "nothing may be replaced without the cap");
        }
        // A buffer is released once for each submission that used it, except
        // the submission still on screen.
        let shown = self
            .on_screen
            .map(|id| self.subs[&id].1.clone())
            .unwrap_or_default();
        for (buffer, uses) in &c.uses {
            let released = c.releases.get(buffer).copied().unwrap_or(0);
            let on_screen = u32::from(shown.contains(buffer));
            assert_eq!(released + on_screen, *uses, "{buffer:?}");
        }
    }
}

fn layers(app: BufferId, safe: BufferId) -> [Layer; 2] {
    let mut safety = Layer::new(safe, FULL, FULL, 10);
    safety.blend = BlendMode::Premultiplied;
    [safety, Layer::new(app, FULL, FULL, 0)]
}

/// Picks the next buffer of a pool in rotation order whose state is `Ready`.
fn next_free(engine: &Engine<FakeBackend>, pool: [BufferId; 3], cursor: &mut usize) -> BufferId {
    for _ in 0..pool.len() {
        let buffer = pool[*cursor % pool.len()];
        *cursor += 1;
        if engine.buffer_state(buffer) == Some(BufferState::Ready) {
            return buffer;
        }
    }
    panic!("no free buffer in {pool:?}: three buffers per layer must suffice");
}

fn drain(engine: &mut Engine<FakeBackend>, model: &mut Model) {
    while let Some(event) = engine.poll_event().unwrap() {
        model.apply(event);
    }
}

fn drain_all(engine: &mut Engine<FakeBackend>) -> Vec<DisplayEvent> {
    let mut events = Vec::new();
    while let Some(event) = engine.poll_event().unwrap() {
        events.push(event);
    }
    events
}

fn present(
    engine: &mut Engine<FakeBackend>,
    app: BufferId,
    safe: BufferId,
) -> Result<SubmissionId, Error> {
    let layers = layers(app, safe);
    engine.present(&Frame {
        output: OUT,
        layers: &layers,
    })
}

fn run(seed: u32, replace_pending: bool, steps: u32) -> Model {
    let mut engine = new_engine(replace_pending);
    let mut model = Model::new();
    let mut rng = Lcg(seed);
    let (mut app_cursor, mut safe_cursor) = (0, 0);
    let mut fail_armed = false;

    for _ in 0..steps {
        // A burst of 0..=2 frames per step; two frames without a vsync in
        // between make the second replace the first before it latches.
        for _ in 0..(rng.next() % 3) {
            // A refused (Busy) frame never reaches the backend, so an armed
            // failure or vsync flag survives it and fires on a later commit.
            if !fail_armed && rng.chance(1, 8) {
                fail_armed = true;
                engine.backend_mut().fail_next_commits(1);
            } else if rng.chance(1, 6) {
                engine.backend_mut().vsync_in_next_commit();
            } else if rng.chance(1, 6) {
                engine.backend_mut().vsync_after_next_commit();
            }
            let app = next_free(&engine, APP, &mut app_cursor);
            let safe = next_free(&engine, SAFE, &mut safe_cursor);
            for buffer in [app, safe] {
                engine.begin_write(buffer).unwrap();
                engine.finish_write(buffer).unwrap();
            }
            let had_pending = model.pending().is_some();
            match present(&mut engine, app, safe) {
                Ok(id) => {
                    assert!(!fail_armed, "a commit set to fail succeeded");
                    model.submitted(id, &[app, safe]);
                }
                Err(Error::Backend(BackendError::Platform(-5))) => {
                    assert!(fail_armed);
                    fail_armed = false;
                    model.counts.failed_commits += 1;
                }
                Err(Error::Present(PresentError::Busy(OUT))) => {
                    assert!(!replace_pending && had_pending);
                    model.counts.refused_busy += 1;
                }
                Err(error) => panic!("unexpected {error:?}"),
            }
            // A failed or refused frame leaves its buffers idle; the vsync
            // flags it did not consume stay armed for the next commit.
            drain(&mut engine, &mut model);
            // A vblank inside the commit latches the pending frame, so the
            // model's notion of "pending" is only valid after the drain.
            model.check_states(&engine);
        }
        // The vertical blank comes late or not at all.
        if rng.chance(1, 2) {
            engine.backend_mut().vsync();
        } else {
            model.counts.skipped_vsyncs += 1;
        }
        drain(&mut engine, &mut model);
        model.check_states(&engine);
    }

    // Let the last frame latch; then nothing may be owed or pending.
    engine.backend_mut().vsync();
    drain(&mut engine, &mut model);
    model.check_states(&engine);
    model.check_totals(replace_pending);
    // The fake took exactly the commits the engine accepted.
    assert_eq!(engine.backend().commit_count(), model.counts.submitted);
    model
}

#[test]
fn seeded_rotation_with_replacement_keeps_events_ordered() {
    let mut total = Counts::default();
    for seed in 1..=24 {
        let model = run(seed, true, 400);
        let c = &model.counts;
        total.submitted += c.submitted;
        total.presented += c.presented;
        total.dropped += c.dropped;
        total.failed_commits += c.failed_commits;
        total.skipped_vsyncs += c.skipped_vsyncs;
    }
    // The schedules must actually exercise the adversarial paths.
    assert!(total.submitted > 5_000, "{}", total.submitted);
    assert!(total.dropped > 300, "replacements: {}", total.dropped);
    assert!(total.failed_commits > 100);
    assert!(total.skipped_vsyncs > 1_000);
    assert!(total.presented > 2_000);
}

#[test]
fn seeded_rotation_without_replacement_refuses_instead_of_dropping() {
    let mut busy = 0;
    for seed in 100..=115 {
        let model = run(seed, false, 400);
        assert_eq!(model.counts.dropped, 0);
        busy += model.counts.refused_busy;
    }
    assert!(busy > 300, "busy refusals: {busy}");
}

#[test]
fn replaced_submission_is_dropped_and_its_buffers_released() {
    let mut engine = new_engine(true);
    let first = present(&mut engine, APP[0], SAFE[0]).unwrap();
    // Replaced before any latch: the first never reaches the screen.
    let second = present(&mut engine, APP[1], SAFE[1]).unwrap();
    let events = drain_all(&mut engine);
    assert_eq!(
        events[0],
        DisplayEvent::Dropped {
            output: OUT,
            submission: first
        }
    );
    let mut released: Vec<_> = events[1..].to_vec();
    released.sort_by_key(|e| format!("{e:?}"));
    assert_eq!(
        released,
        [
            DisplayEvent::Released { buffer: APP[0] },
            DisplayEvent::Released { buffer: SAFE[0] },
        ]
    );
    engine.backend_mut().vsync();
    assert_eq!(
        drain_all(&mut engine),
        [DisplayEvent::Presented {
            output: OUT,
            submission: second
        }]
    );
}

#[test]
fn vblank_inside_commit_latches_the_replaced_frame_before_releasing_it() {
    let mut engine = new_engine(true);
    let first = present(&mut engine, APP[0], SAFE[0]).unwrap();
    // The blank latches `first` while the driver writes `second`.
    engine.backend_mut().vsync_in_next_commit();
    let second = present(&mut engine, APP[1], SAFE[1]).unwrap();
    // `first` is presented, not dropped, and nothing is released yet: it is
    // on screen while `second` is pending.
    assert_eq!(
        drain_all(&mut engine),
        [DisplayEvent::Presented {
            output: OUT,
            submission: first
        }]
    );
    assert_eq!(engine.buffer_state(APP[0]), Some(BufferState::Scanning));
    assert_eq!(engine.buffer_state(APP[1]), Some(BufferState::Submitted));
    engine.backend_mut().vsync();
    let events = drain_all(&mut engine);
    assert_eq!(
        events[0],
        DisplayEvent::Presented {
            output: OUT,
            submission: second
        }
    );
    let mut released: Vec<_> = events[1..].to_vec();
    released.sort_by_key(|e| format!("{e:?}"));
    assert_eq!(
        released,
        [
            DisplayEvent::Released { buffer: APP[0] },
            DisplayEvent::Released { buffer: SAFE[0] },
        ]
    );
}

#[test]
fn failed_commit_mid_rotation_changes_nothing_and_keeps_ids_dense() {
    let mut engine = new_engine(true);
    let first = present(&mut engine, APP[0], SAFE[0]).unwrap();
    engine.backend_mut().vsync();
    drain_all(&mut engine);

    engine.backend_mut().fail_next_commits(1);
    assert_eq!(
        present(&mut engine, APP[1], SAFE[1]),
        Err(Error::Backend(BackendError::Platform(-5)))
    );
    // The buffers of the failed frame stay idle and nothing was reported.
    assert_eq!(engine.buffer_state(APP[1]), Some(BufferState::Ready));
    assert_eq!(engine.buffer_state(SAFE[1]), Some(BufferState::Ready));
    assert_eq!(engine.buffer_state(APP[0]), Some(BufferState::Scanning));
    assert!(drain_all(&mut engine).is_empty());
    assert_eq!(engine.backend().pending(OUT), None);

    // The retry takes the next identifier: the failure used none.
    let retry = present(&mut engine, APP[1], SAFE[1]).unwrap();
    assert_eq!(retry.0, first.0 + 1);
}
