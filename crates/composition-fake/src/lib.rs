//! A fake display backend for deterministic host tests.
//!
//! It models a display controller with shadow registers: a commit is held
//! until the test calls [`FakeBackend::vsync`], which latches it. Tests
//! control time, so delayed or missing vsyncs, failed commits, rejected
//! configurations and device loss are all reproducible.
//!
//! By default the fake does not wait for readiness tokens, so the engine
//! refuses frames whose buffers are pending on one. With
//! [`FakeBackend::set_waits_for_readiness`] it waits: a commit is latched by
//! [`FakeBackend::vsync`] only once every token of its buffers was signalled
//! ([`FakeBackend::signal_token`], or cleared by the engine), the way a
//! kernel waits on an `IN_FENCE_FD`.

#![forbid(unsafe_code)]

use std::collections::VecDeque;

use composition_core::{
    BackendError, BackendEvent, BufferId, Capabilities, DisplayBackend, Frame, OutputId, Plan,
    PlaneId, ReadyToken, SubmissionId,
};

/// What one plane shows in a committed frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaneState {
    /// The plane.
    pub plane: PlaneId,
    /// Buffer scanned out by the plane.
    pub buffer: BufferId,
    /// Programmed stacking position.
    pub zpos: i32,
}

#[derive(Clone, Debug)]
struct Commit {
    submission: SubmissionId,
    planes: Vec<PlaneState>,
    /// Tokens of its buffers that have not signalled yet.
    waiting: Vec<(BufferId, ReadyToken)>,
}

#[derive(Clone, Debug, Default)]
struct OutputState {
    pending: Option<Commit>,
    latched: Option<Commit>,
}

/// Decides whether the fake hardware accepts a planned frame.
pub type CheckHook = Box<dyn FnMut(&Frame<'_>, &Plan) -> bool>;

/// The fake backend.
pub struct FakeBackend {
    caps: Capabilities,
    outputs: Vec<(OutputId, OutputState)>,
    events: VecDeque<BackendEvent>,
    check_hook: Option<CheckHook>,
    failing_commits: u32,
    lost: bool,
    checks: u32,
    commits: u32,
    vsync_in_next_commit: bool,
    vsync_after_next_commit: bool,
    event_in_next_commit: Option<BackendEvent>,
    waits_for_readiness: bool,
    /// Token given for each buffer and not cleared yet.
    tokens: Vec<(BufferId, ReadyToken)>,
    /// Tokens signalled at the fake.
    signalled: Vec<ReadyToken>,
    /// Every accepted [`DisplayBackend::set_readiness`] call, in order.
    readiness_log: Vec<(BufferId, ReadyToken)>,
    /// Every [`DisplayBackend::readiness_failed`] call, in order.
    failed_log: Vec<ReadyToken>,
}

impl FakeBackend {
    /// Creates a backend with the given capabilities.
    pub fn new(caps: Capabilities) -> Self {
        Self {
            caps,
            outputs: Vec::new(),
            events: VecDeque::new(),
            check_hook: None,
            failing_commits: 0,
            lost: false,
            checks: 0,
            commits: 0,
            vsync_in_next_commit: false,
            vsync_after_next_commit: false,
            event_in_next_commit: None,
            waits_for_readiness: false,
            tokens: Vec::new(),
            signalled: Vec::new(),
            readiness_log: Vec::new(),
            failed_log: Vec::new(),
        }
    }

    /// Makes the fake wait for readiness tokens (see the crate
    /// documentation), or, with `false`, refuse them as it does by default.
    pub fn set_waits_for_readiness(&mut self, waits: bool) {
        self.waits_for_readiness = waits;
    }

    /// Signals `token` at the fake, as a completed fence would: commits
    /// waiting on it may be latched by the next [`FakeBackend::vsync`]. The
    /// engine is not told; it learns it from the latch.
    pub fn signal_token(&mut self, token: ReadyToken) {
        if !self.signalled.contains(&token) {
            self.signalled.push(token);
        }
        self.tokens.retain(|(_, t)| *t != token);
        for (_, state) in &mut self.outputs {
            if let Some(commit) = &mut state.pending {
                commit.waiting.retain(|(_, t)| *t != token);
            }
        }
    }

    /// Every token the engine gave with [`DisplayBackend::set_readiness`]
    /// and the fake accepted, in order, repetitions included.
    pub fn readiness_log(&self) -> &[(BufferId, ReadyToken)] {
        &self.readiness_log
    }

    /// Every token the engine reported failed, in order.
    pub fn failed_tokens(&self) -> &[ReadyToken] {
        &self.failed_log
    }

    /// Tokens the commit pending on `output` still waits for.
    pub fn waiting_tokens(&self, output: OutputId) -> Vec<ReadyToken> {
        self.state(output)
            .and_then(|s| s.pending.as_ref())
            .map(|c| c.waiting.iter().map(|(_, t)| *t).collect())
            .unwrap_or_default()
    }

    /// Installs a hook that can reject otherwise legal assignments, like a
    /// driver constraint the capability model does not describe.
    pub fn set_check_hook(&mut self, hook: impl FnMut(&Frame<'_>, &Plan) -> bool + 'static) {
        self.check_hook = Some(Box::new(hook));
    }

    /// Makes the next `count` commits fail with a platform error.
    pub fn fail_next_commits(&mut self, count: u32) {
        self.failing_commits = count;
    }

    /// Makes the next commit meet a vertical blank before it takes over
    /// the pending commit, as an interrupt can while a driver writes the new
    /// frame: the pending commit is latched and reported, then replaced.
    pub fn vsync_in_next_commit(&mut self) {
        self.vsync_in_next_commit = true;
    }

    /// Makes the next commit meet a vertical blank right after its writes,
    /// before it returns to the engine: it is latched and reported at once.
    pub fn vsync_after_next_commit(&mut self) {
        self.vsync_after_next_commit = true;
    }

    /// Queues `event` while the next commit runs, after the commit took
    /// effect and before it returns to the engine, as a driver interrupt
    /// would. It is queued only if that commit succeeds.
    pub fn inject_event_in_next_commit(&mut self, event: BackendEvent) {
        self.event_in_next_commit = Some(event);
    }

    /// Simulates a vertical blank: every pending commit is latched, except
    /// one still waiting for a readiness token.
    pub fn vsync(&mut self) {
        for (output, state) in &mut self.outputs {
            if state
                .pending
                .as_ref()
                .is_some_and(|c| !c.waiting.is_empty())
            {
                continue;
            }
            if let Some(commit) = state.pending.take() {
                self.events.push_back(BackendEvent::Latched {
                    output: *output,
                    submission: commit.submission,
                });
                // A latched buffer's token is used up.
                self.tokens
                    .retain(|(b, _)| !commit.planes.iter().any(|p| p.buffer == *b));
                state.latched = Some(commit);
            }
        }
    }

    /// Simulates losing the display: nothing is shown or pending any more,
    /// and every operation fails until [`FakeBackend::restore`].
    pub fn lose_device(&mut self) {
        self.outputs.clear();
        self.lost = true;
        self.events.push_back(BackendEvent::DeviceLost);
    }

    /// Brings a lost device back with the given capabilities.
    pub fn restore(&mut self, caps: Capabilities) {
        self.lost = false;
        self.set_capabilities(caps);
    }

    /// Changes the capabilities and announces it.
    pub fn set_capabilities(&mut self, caps: Capabilities) {
        self.caps = caps;
        self.events.push_back(BackendEvent::CapabilitiesChanged);
    }

    /// Queues an arbitrary event, for tests of misbehaving backends.
    pub fn inject_event(&mut self, event: BackendEvent) {
        self.events.push_back(event);
    }

    /// Planes currently scanned out on `output`, bottom to top.
    pub fn scanout(&self, output: OutputId) -> Option<&[PlaneState]> {
        self.state(output)?.latched.as_ref().map(|c| &c.planes[..])
    }

    /// Submission committed on `output` but not latched yet.
    pub fn pending(&self, output: OutputId) -> Option<SubmissionId> {
        self.state(output)?.pending.as_ref().map(|c| c.submission)
    }

    /// Number of [`DisplayBackend::check`] calls so far.
    pub fn check_count(&self) -> u32 {
        self.checks
    }

    /// Number of successful commits so far.
    pub fn commit_count(&self) -> u32 {
        self.commits
    }

    fn state(&self, output: OutputId) -> Option<&OutputState> {
        self.outputs
            .iter()
            .find(|(id, _)| *id == output)
            .map(|(_, s)| s)
    }
}

impl DisplayBackend for FakeBackend {
    fn capabilities(&mut self, caps: &mut Capabilities) {
        *caps = self.caps;
    }

    fn check(&mut self, frame: &Frame<'_>, plan: &Plan) -> Result<(), BackendError> {
        self.checks += 1;
        if self.lost {
            return Err(BackendError::DeviceLost);
        }
        let accepted = self
            .check_hook
            .as_mut()
            .is_none_or(|hook| hook(frame, plan));
        if accepted {
            Ok(())
        } else {
            Err(BackendError::Rejected)
        }
    }

    fn commit(
        &mut self,
        submission: SubmissionId,
        frame: &Frame<'_>,
        plan: &Plan,
    ) -> Result<(), BackendError> {
        if self.lost {
            return Err(BackendError::DeviceLost);
        }
        if self.failing_commits > 0 {
            self.failing_commits -= 1;
            return Err(BackendError::Platform(-5));
        }
        if core::mem::take(&mut self.vsync_in_next_commit) {
            self.vsync();
        }
        let index = match self.outputs.iter().position(|(id, _)| *id == frame.output) {
            Some(index) => index,
            None => {
                self.outputs.push((frame.output, OutputState::default()));
                self.outputs.len() - 1
            }
        };
        let state = &mut self.outputs[index].1;
        if state.pending.is_some() && !self.caps.replace_pending {
            return Err(BackendError::Busy);
        }
        let planes = plan
            .assignments()
            .iter()
            .map(|a| PlaneState {
                plane: a.plane,
                buffer: frame.layers[a.layer].buffer,
                zpos: a.zpos,
            })
            .collect::<Vec<_>>();
        let waiting = self
            .tokens
            .iter()
            .filter(|(b, t)| planes.iter().any(|p| p.buffer == *b) && !self.signalled.contains(t))
            .copied()
            .collect();
        state.pending = Some(Commit {
            submission,
            planes,
            waiting,
        });
        self.commits += 1;
        if let Some(event) = self.event_in_next_commit.take() {
            self.events.push_back(event);
        }
        if core::mem::take(&mut self.vsync_after_next_commit) {
            self.vsync();
        }
        Ok(())
    }

    fn poll_event(&mut self) -> Option<BackendEvent> {
        self.events.pop_front()
    }

    fn waits_for_readiness(&self) -> bool {
        self.waits_for_readiness
    }

    fn set_readiness(&mut self, buffer: BufferId, token: ReadyToken) -> Result<(), BackendError> {
        if !self.waits_for_readiness {
            return Err(BackendError::Rejected);
        }
        if self.lost {
            return Err(BackendError::DeviceLost);
        }
        self.tokens.retain(|(b, _)| *b != buffer);
        self.tokens.push((buffer, token));
        self.readiness_log.push((buffer, token));
        Ok(())
    }

    fn clear_readiness(&mut self, buffer: BufferId) {
        self.tokens.retain(|(b, _)| *b != buffer);
        for (_, state) in &mut self.outputs {
            if let Some(commit) = &mut state.pending {
                commit.waiting.retain(|(b, _)| *b != buffer);
            }
        }
    }

    fn readiness_failed(&mut self, token: ReadyToken) {
        self.failed_log.push(token);
        self.tokens.retain(|(_, t)| *t != token);
        // A commit waiting on the token is withdrawn: never latched, and
        // what was latched before stays on screen.
        for (_, state) in &mut self.outputs {
            if state
                .pending
                .as_ref()
                .is_some_and(|c| c.waiting.iter().any(|(_, t)| *t == token))
            {
                state.pending = None;
            }
        }
    }
}
