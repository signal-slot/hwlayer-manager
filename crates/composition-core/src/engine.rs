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
use crate::types::{BufferId, BufferInfo, Frame, SubmissionId};

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
pub struct Engine<B: DisplayBackend> {
    backend: B,
    caps: Capabilities,
    generation: u32,
    presentation: Presentation,
    limits: SearchLimits,
    latch_timeout: Option<u64>,
}

impl<B: DisplayBackend> Engine<B> {
    /// Creates an engine driving `backend`.
    pub fn new(backend: B) -> Self {
        let caps = backend.capabilities();
        Self {
            backend,
            caps,
            generation: 0,
            presentation: Presentation::new(),
            limits: SearchLimits::default(),
            latch_timeout: None,
        }
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
            self.presentation.report_overdue(timeout);
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
        let backend = &mut self.backend;
        // An assignment the hardware rejects is worth replacing by another;
        // any other failure would repeat for every assignment.
        let mut fatal = None;
        let planned = planner::plan_with(
            frame,
            &self.caps,
            &self.presentation,
            self.limits,
            |candidate| match backend.check(frame, candidate) {
                Ok(()) => Verdict::Accept,
                Err(BackendError::Rejected) => Verdict::TryNext,
                Err(error) => {
                    fatal = Some(error);
                    Verdict::GiveUp
                }
            },
        );
        let mut plan = match (planned, fatal) {
            (Err(PlanError::Aborted), Some(error)) => return Err(Error::Backend(error)),
            (planned, _) => planned?,
        };
        plan.generation = self.generation;
        Ok(plan)
    }

    /// Submits `frame` using a plan from [`Engine::validate`].
    ///
    /// On success the frame has been accepted by the hardware, nothing more.
    /// [`DisplayEvent::Presented`] follows once it is on screen. On failure
    /// the display and all buffer states are unchanged.
    pub fn commit(&mut self, frame: &Frame<'_>, plan: &Plan) -> Result<SubmissionId, Error> {
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
        self.collect_backend_events()?;
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
        let recorded = self.presentation.submit(frame.output, &buffers, replace)?;
        debug_assert_eq!(recorded, submission);
        if let Some(output) = own_latch {
            self.presentation.latched(output, recorded)?;
        }
        Ok(recorded)
    }

    /// Validates and submits `frame` in one step.
    pub fn present(&mut self, frame: &Frame<'_>) -> Result<SubmissionId, Error> {
        let plan = self.validate(frame)?;
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
            BackendEvent::Latched { output, submission } => {
                self.presentation.latched(output, submission)?;
            }
            BackendEvent::DeviceLost => self.device_lost()?,
            BackendEvent::CapabilitiesChanged => {
                self.refresh_capabilities();
                self.presentation
                    .push_event(DisplayEvent::CapabilitiesChanged)?;
            }
        }
        Ok(())
    }

    // Out of line and cold, like refresh_capabilities: forgetting every
    // submission needs a large frame (about 2 KiB on an MCU), which inlined
    // would be reserved by every poll and every commit.
    #[cold]
    #[inline(never)]
    fn device_lost(&mut self) -> Result<(), Error> {
        self.presentation.reset()?;
        self.refresh_capabilities();
        self.presentation.push_event(DisplayEvent::DeviceLost)?;
        Ok(())
    }

    // Out of line and cold: the backend's description is large (about 9 KiB),
    // and inlined it would be reserved on the stack of every caller,
    // `commit` included, on every frame, though it changes only when the
    // display does. MCU firmwares present from small thread stacks.
    #[cold]
    #[inline(never)]
    fn refresh_capabilities(&mut self) {
        self.caps = self.backend.capabilities();
        self.generation = self.generation.wrapping_add(1);
    }
}
