//! A fake display backend for deterministic host tests.
//!
//! It models a display controller with shadow registers: a commit is held
//! until the test calls [`FakeBackend::vsync`], which latches it. Tests
//! control time, so delayed or missing vsyncs, failed commits, rejected
//! configurations and device loss are all reproducible.

#![forbid(unsafe_code)]

use std::collections::VecDeque;

use composition_core::{
    BackendError, BackendEvent, BufferId, Capabilities, DisplayBackend, Frame, OutputId, Plan,
    PlaneId, SubmissionId,
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
        }
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

    /// Simulates a vertical blank: every pending commit is latched.
    pub fn vsync(&mut self) {
        for (output, state) in &mut self.outputs {
            if let Some(commit) = state.pending.take() {
                self.events.push_back(BackendEvent::Latched {
                    output: *output,
                    submission: commit.submission,
                });
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
            .collect();
        state.pending = Some(Commit { submission, planes });
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
}
