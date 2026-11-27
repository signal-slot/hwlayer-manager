//! The composition engine for one backend.
//!
//! [`Engine`] is the API a client uses: register buffers, describe a frame,
//! and follow what happens to it through events.

use crate::backend::{BackendError, BackendEvent, DisplayBackend};
use crate::caps::Capabilities;
use crate::fixed::FixedVec;
use crate::limits::MAX_LAYERS;
use crate::planner::{self, Plan, PlanError, SearchLimits, Verdict};
use crate::presentation::{BufferState, DisplayEvent, PresentError, Presentation};
use crate::trace::{DropReason, NoTrace, Trace, TraceEvent};
use crate::types::{BufferId, BufferInfo, Frame, OutputId, SubmissionId};

/// Why the engine refused an operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The frame cannot be mapped onto the hardware.
    Plan(PlanError),
    /// The buffer or submission lifecycle forbids the operation.
    Present(PresentError),
    /// The backend refused.
    Backend(BackendError),
    /// The plan was made before the backend's capabilities changed.
    StalePlan,
    /// The plan was made for a different frame.
    PlanMismatch,
}

impl From<PlanError> for Error {
    fn from(error: PlanError) -> Self {
        Error::Plan(error)
    }
}

impl From<PresentError> for Error {
    fn from(error: PresentError) -> Self {
        Error::Present(error)
    }
}

impl From<BackendError> for Error {
    fn from(error: BackendError) -> Self {
        Error::Backend(error)
    }
}

/// Plans frames, submits them to a backend and tracks buffer lifetimes.
///
/// `T` receives the engine's structured trace (see [`crate::trace`]); the
/// default, [`NoTrace`], keeps nothing and costs nothing.
pub struct Engine<B: DisplayBackend, T: Trace = NoTrace> {
    backend: B,
    caps: Capabilities,
    generation: u32,
    presentation: Presentation,
    limits: SearchLimits,
    latch_timeout: Option<u64>,
    trace: T,
}

impl<B: DisplayBackend> Engine<B> {
    /// Creates an engine driving `backend`.
    pub fn new(backend: B) -> Self {
        let mut engine = Self::new_unprobed(backend);
        engine.probe();
        engine
    }

    /// Creates an engine that knows no outputs or planes until
    /// [`Engine::probe`] is called.
    ///
    /// For callers that build the engine in place, such as the C ABI on an
    /// MCU: [`Engine::new`] asks the backend before it returns, so the engine
    /// is assembled on the stack and then moved. Write this one into its
    /// final storage, then probe it there.
    // The large parts come from constants, which are copied straight into
    // the engine, and the function is always inlined: built from calls, or
    // out of line, they are assembled in temporaries on the caller's stack
    // and copied (the engine is about 14 KiB on a 32-bit MCU), whatever the
    // optimisation level.
    #[inline(always)]
    pub fn new_unprobed(backend: B) -> Self {
        Self {
            backend,
            caps: Capabilities::EMPTY,
            generation: 0,
            presentation: Presentation::EMPTY,
            limits: SearchLimits::default(),
            latch_timeout: None,
            trace: NoTrace,
        }
    }
}

impl<B: DisplayBackend, T: Trace> Engine<B, T> {
    /// The engine with its trace going to `trace` instead.
    ///
    /// This moves the engine; on an MCU, attach the sink before the engine
    /// is written into its final storage.
    pub fn with_trace<U: Trace>(self, trace: U) -> Engine<B, U> {
        Engine {
            backend: self.backend,
            caps: self.caps,
            generation: self.generation,
            presentation: self.presentation,
            limits: self.limits,
            latch_timeout: self.latch_timeout,
            trace,
        }
    }

    /// Replaces the trace sink, returning the previous one.
    pub fn set_trace(&mut self, trace: T) -> T {
        core::mem::replace(&mut self.trace, trace)
    }

    /// The trace sink.
    pub fn trace(&self) -> &T {
        &self.trace
    }

    /// The trace sink, for reading out or clearing what it recorded.
    pub fn trace_mut(&mut self) -> &mut T {
        &mut self.trace
    }

    /// Asks the backend for its capabilities again, as
    /// [`BackendEvent::CapabilitiesChanged`] does, without reporting
    /// [`DisplayEvent::CapabilitiesChanged`]. Plans made before are stale.
    pub fn probe(&mut self) {
        self.refresh_capabilities();
    }

    /// The backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// The backend, for backend-specific operations such as buffer import.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Capabilities the engine currently plans against.
    pub fn capabilities(&self) -> &Capabilities {
        &self.caps
    }

    /// Replaces the bounds of the planning search.
    pub fn set_search_limits(&mut self, limits: SearchLimits) {
        self.limits = limits;
    }

    /// Sets how long a submission may wait for its latch before
    /// [`DisplayEvent::LatchTimeout`] is reported, in the ticks passed to
    /// [`Engine::advance_time`]. `None` turns the report off.
    pub fn set_latch_timeout(&mut self, timeout: Option<u64>) {
        self.latch_timeout = timeout;
    }

    /// Tells the engine the current time, in ticks of the client's choice
    /// (milliseconds, vsync counts, ...). The values must not decrease.
    ///
    /// The engine has no clock of its own. Call this regularly, typically
    /// right before [`Engine::poll_event`], if a latch timeout is set.
    pub fn advance_time(&mut self, now: u64) {
        self.presentation.set_time(now);
        if let Some(timeout) = self.latch_timeout {
            let start = self.presentation.events_len();
            self.presentation.report_overdue(timeout);
            self.trace_events(start, None, DropReason::Replaced);
        }
    }

    /// Registers a buffer. It starts out [`BufferState::Writable`].
    pub fn register_buffer(&mut self, buffer: BufferId, info: BufferInfo) -> Result<(), Error> {
        Ok(self.presentation.register(buffer, info)?)
    }

    /// Removes a buffer that is neither submitted nor on screen.
    pub fn unregister_buffer(&mut self, buffer: BufferId) -> Result<(), Error> {
        Ok(self.presentation.unregister(buffer)?)
    }

    /// Hands an idle buffer to the producer for drawing.
    pub fn begin_write(&mut self, buffer: BufferId) -> Result<(), Error> {
        Ok(self.presentation.begin_write(buffer)?)
    }

    /// Declares that the producer finished drawing into the buffer.
    pub fn finish_write(&mut self, buffer: BufferId) -> Result<(), Error> {
        Ok(self.presentation.finish_write(buffer)?)
    }

    /// Current state of a buffer, or `None` if it is not registered.
    pub fn buffer_state(&self, buffer: BufferId) -> Option<BufferState> {
        self.presentation.state(buffer)
    }

    /// Finds a plane assignment for `frame` that the backend accepts.
    ///
    /// Nothing on the display changes.
    pub fn validate(&mut self, frame: &Frame<'_>) -> Result<Plan, Error> {
        let mut plan = Plan::EMPTY;
        self.validate_into(frame, &mut plan)?;
        Ok(plan)
    }

    /// [`Engine::validate`] into `plan`; see [`planner::plan_into`].
    fn validate_into(&mut self, frame: &Frame<'_>, plan: &mut Plan) -> Result<(), Error> {
        let result = self.plan_frame(frame, plan);
        if self.trace.enabled() {
            let event = TraceEvent::Validate {
                output: frame.output,
                layers: frame.layers.len(),
                result: result.map(|()| plan.assignments()),
            };
            self.trace.record(self.presentation.now(), event);
        }
        result
    }

    /// [`Engine::validate_into`] without its trace entry.
    fn plan_frame(&mut self, frame: &Frame<'_>, plan: &mut Plan) -> Result<(), Error> {
        let backend = &mut self.backend;
        let trace = &mut self.trace;
        let now = self.presentation.now();
        // An assignment the hardware rejects is worth replacing by another;
        // any other failure would repeat for every assignment.
        let mut fatal = None;
        let planned = planner::plan_into(
            frame,
            &self.caps,
            &self.presentation,
            self.limits,
            |candidate| {
                let result = backend.check(frame, candidate);
                if trace.enabled() {
                    let event = TraceEvent::Check {
                        output: frame.output,
                        assignments: candidate.assignments(),
                        result,
                    };
                    trace.record(now, event);
                }
                match result {
                    Ok(()) => Verdict::Accept,
                    Err(BackendError::Rejected) => Verdict::TryNext,
                    Err(error) => {
                        fatal = Some(error);
                        Verdict::GiveUp
                    }
                }
            },
            plan,
        );
        match (planned, fatal) {
            (Err(PlanError::Aborted), Some(error)) => return Err(Error::Backend(error)),
            (planned, _) => planned?,
        }
        plan.generation = self.generation;
        Ok(())
    }

    /// Submits `frame` using a plan from [`Engine::validate`].
    ///
    /// On success the frame has been accepted by the hardware, nothing more.
    /// [`DisplayEvent::Presented`] follows once it is on screen. On failure
    /// the display and all buffer states are unchanged.
    ///
    /// The events the backend queued are taken first. If they lost the
    /// device the commit fails with [`BackendError::DeviceLost`]; if they
    /// changed the capabilities, with [`Error::StalePlan`].
    pub fn commit(&mut self, frame: &Frame<'_>, plan: &Plan) -> Result<SubmissionId, Error> {
        let result = self.commit_untraced(frame, plan);
        if self.trace.enabled() {
            let event = TraceEvent::Commit {
                output: frame.output,
                result,
            };
            self.trace.record(self.presentation.now(), event);
        }
        result
    }

    /// [`Engine::commit`] without its trace entry; what happens on the way
    /// is traced.
    fn commit_untraced(&mut self, frame: &Frame<'_>, plan: &Plan) -> Result<SubmissionId, Error> {
        if plan.generation != self.generation {
            return Err(Error::StalePlan);
        }
        if !plan.matches(frame) {
            return Err(Error::PlanMismatch);
        }
        let mut buffers: FixedVec<BufferId, MAX_LAYERS> = FixedVec::new();
        // Only layers that got a plane are shown; a layer entirely off the
        // output does not tie up its buffer.
        for assignment in plan.assignments() {
            buffers
                .push(frame.layers[assignment.layer].buffer)
                .map_err(|_| PresentError::TooManyBuffers)?;
        }
        let replace = self.caps.replace_pending;
        // Latches the backend has already reported are recorded first, so
        // that the check below sees what is really pending.
        if self.collect_backend_events()? {
            return Err(Error::Backend(BackendError::DeviceLost));
        }
        // Among them may be a capability change: the plan was made for the
        // hardware as it was before, and the backend must not be asked to
        // commit it.
        if plan.generation != self.generation {
            return Err(Error::StalePlan);
        }
        // Verify the bookkeeping first so that a commit the hardware accepts
        // can always be recorded.
        self.presentation
            .check_submit(frame.output, &buffers, replace)?;
        let submission = self.presentation.next_submission_id();
        self.backend.commit(submission, frame, plan)?;
        // A pending submission can be latched while the backend commits the
        // one that replaces it. The backend then reports that latch by the
        // time `commit` returns (see `DisplayBackend::commit`), and it must be
        // recorded before the new submission: recorded after it, the latched
        // submission would be counted as dropped and its buffers released
        // while they are being scanned out. The new submission itself may
        // have latched already too (a vertical blank right after its writes,
        // or a commit that changed nothing): that latch is held back and
        // recorded after the submission. `check_submit` reserved the room
        // for all of these.
        let mut own_latch = None;
        let mut lost = false;
        while self.presentation.events_room() > MAX_LAYERS + 1 {
            let Some(event) = self.backend.poll_event() else {
                break;
            };
            match event {
                BackendEvent::Latched {
                    output,
                    submission: latched,
                } if latched == submission => own_latch = Some(output),
                event => {
                    lost |= event == BackendEvent::DeviceLost;
                    self.apply_backend_event(event)?;
                }
            }
        }
        if lost {
            // The display went away during the commit: nothing will latch.
            return Err(Error::Backend(BackendError::DeviceLost));
        }
        let start = self.presentation.events_len();
        let recorded = self.presentation.submit(frame.output, &buffers, replace)?;
        self.trace_events(start, None, DropReason::Replaced);
        debug_assert_eq!(recorded, submission);
        if let Some(output) = own_latch {
            self.latched(output, recorded)?;
        }
        Ok(recorded)
    }

    /// Validates and submits `frame` in one step.
    pub fn present(&mut self, frame: &Frame<'_>) -> Result<SubmissionId, Error> {
        // One plan on this stack, filled in place (MCU firmwares present
        // from small thread stacks).
        let mut plan = Plan::EMPTY;
        // What the backend queued is taken first, as `commit` would: a
        // capability change among it would make the plan stale at once.
        self.collect_backend_events()?;
        self.validate_into(frame, &mut plan)?;
        self.commit(frame, &plan)
    }

    /// Takes the next display event, pulling from the backend as needed.
    ///
    /// Returns an error if the backend reports something inconsistent, such
    /// as a latch for a submission that is not pending.
    pub fn poll_event(&mut self) -> Result<Option<DisplayEvent>, Error> {
        loop {
            if let Some(event) = self.presentation.poll_event() {
                return Ok(Some(event));
            }
            // The queue is empty here, so the events of one backend event
            // always fit.
            match self.backend.poll_event() {
                None => return Ok(None),
                Some(event) => self.apply_backend_event(event)?,
            }
        }
    }

    /// Records the events the backend has queued, leaving the display events
    /// they cause for [`Engine::poll_event`]. Stops while the queue lacks the
    /// room for one more backend event, so that none is taken and lost; the
    /// rest stays with the backend. Returns `true` if the device was lost.
    fn collect_backend_events(&mut self) -> Result<bool, Error> {
        let mut lost = false;
        while self.presentation.events_room() > MAX_LAYERS + 1 {
            let Some(event) = self.backend.poll_event() else {
                break;
            };
            lost |= event == BackendEvent::DeviceLost;
            self.apply_backend_event(event)?;
        }
        Ok(lost)
    }

    fn apply_backend_event(&mut self, event: BackendEvent) -> Result<(), Error> {
        match event {
            BackendEvent::Latched { output, submission } => self.latched(output, submission)?,
            BackendEvent::DeviceLost => self.device_lost()?,
            BackendEvent::CapabilitiesChanged => {
                self.refresh_capabilities();
                let start = self.presentation.events_len();
                self.presentation
                    .push_event(DisplayEvent::CapabilitiesChanged)?;
                self.trace_events(start, None, DropReason::Replaced);
            }
        }
        Ok(())
    }

    fn latched(&mut self, output: OutputId, submission: SubmissionId) -> Result<(), Error> {
        // The buffers a latch releases belonged to the frame it takes off
        // screen.
        let previous = if self.trace.enabled() {
            self.presentation.on_screen(output)
        } else {
            None
        };
        let start = self.presentation.events_len();
        self.presentation.latched(output, submission)?;
        self.trace_events(start, previous, DropReason::Replaced);
        Ok(())
    }

    /// Traces the display events queued since the queue held `start`.
    ///
    /// A [`DisplayEvent::Released`] after a [`DisplayEvent::Presented`] is
    /// attributed to `previous`, the submission the latch took off screen;
    /// after a [`DisplayEvent::Dropped`] for `reason`
    /// [`DropReason::Replaced`], to the dropped submission. A device loss
    /// releases everything at once and attributes nothing.
    fn trace_events(&mut self, start: usize, previous: Option<SubmissionId>, reason: DropReason) {
        if !self.trace.enabled() {
            return;
        }
        let now = self.presentation.now();
        let mut owner = None;
        for index in start..self.presentation.events_len() {
            let Some(event) = self.presentation.queued_event(index) else {
                break;
            };
            let event = match event {
                DisplayEvent::Presented { output, submission } => {
                    owner = previous;
                    TraceEvent::Latched { output, submission }
                }
                DisplayEvent::Dropped { output, submission } => {
                    owner = (reason == DropReason::Replaced).then_some(submission);
                    TraceEvent::Dropped {
                        output,
                        submission,
                        reason,
                    }
                }
                DisplayEvent::Released { buffer } => TraceEvent::Released {
                    buffer,
                    submission: owner,
                },
                DisplayEvent::LatchTimeout { output, submission } => {
                    TraceEvent::LatchTimeout { output, submission }
                }
                DisplayEvent::DeviceLost => TraceEvent::DeviceLost,
                DisplayEvent::CapabilitiesChanged => TraceEvent::CapabilitiesChanged,
            };
            self.trace.record(now, event);
        }
    }

    // Out of line and cold, like refresh_capabilities: forgetting every
    // submission needs a large frame (about 2 KiB on an MCU), which inlined
    // would be reserved by every poll and every commit.
    #[cold]
    #[inline(never)]
    fn device_lost(&mut self) -> Result<(), Error> {
        let start = self.presentation.events_len();
        self.presentation.reset()?;
        self.refresh_capabilities();
        let pushed = self.presentation.push_event(DisplayEvent::DeviceLost);
        self.trace_events(start, None, DropReason::DeviceLost);
        Ok(pushed?)
    }

    // Out of line and cold: it runs only when the display changes, and the
    // backend's code to describe itself has no place on the stack of every
    // poll and commit. The description itself is written in place.
    #[cold]
    #[inline(never)]
    fn refresh_capabilities(&mut self) {
        self.caps.clear();
        self.backend.capabilities(&mut self.caps);
        self.generation = self.generation.wrapping_add(1);
    }
}
