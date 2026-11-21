//! The contract between the core and a display backend.
//!
//! A backend owns the display resources and is the only component that
//! programs them. It does not plan: the core proposes an assignment and the
//! backend says whether the hardware takes it, then commits it.
//!
//! Importing platform buffers (DMA-BUF file descriptors, physical addresses)
//! is not part of this trait. Each backend offers its own import function and
//! associates the result with a [`crate::BufferId`].

use crate::caps::Capabilities;
use crate::planner::Plan;
use crate::types::{BufferId, Frame, OutputId, SubmissionId};

/// Why a backend refused an operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackendError {
    /// The hardware or driver does not accept this configuration.
    Rejected,
    /// A previous commit has not been latched and cannot be replaced.
    Busy,
    /// The buffer was not imported into this backend.
    UnknownBuffer(BufferId),
    /// The display resources are gone (reset, lease revoked).
    DeviceLost,
    /// A platform error, with a backend-defined code such as `errno`.
    Platform(i32),
}

/// What a backend reports back to the core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackendEvent {
    /// The hardware latched a commit: its buffers are now being scanned out.
    Latched {
        /// Output the commit was for.
        output: OutputId,
        /// Identifier passed to [`DisplayBackend::commit`].
        submission: SubmissionId,
    },
    /// The display resources were lost. Nothing committed earlier is shown
    /// or will be latched.
    DeviceLost,
    /// [`DisplayBackend::capabilities`] now returns something different.
    CapabilitiesChanged,
}

/// A display backend: Linux DRM/KMS, an MCU display controller, or a fake.
pub trait DisplayBackend {
    /// Describes the outputs and planes the backend can drive right now.
    ///
    /// The description must reflect what is actually usable, for example only
    /// the objects granted by a DRM lease.
    fn capabilities(&self) -> Capabilities;

    /// Final validation of a planned frame, without changing the display.
    ///
    /// On Linux this is an atomic `TEST_ONLY` commit. A backend with nothing
    /// to add beyond its capabilities returns `Ok(())`.
    fn check(&mut self, frame: &Frame<'_>, plan: &Plan) -> Result<(), BackendError>;

    /// Programs the frame.
    ///
    /// Success means the hardware accepted the configuration, not that it is
    /// on screen. The backend later reports [`BackendEvent::Latched`] with
    /// the same `submission` once the hardware has latched it.
    ///
    /// If a commit is still unlatched and the backend replaces it, the
    /// replaced submission must never be reported as latched.
    fn commit(
        &mut self,
        submission: SubmissionId,
        frame: &Frame<'_>,
        plan: &Plan,
    ) -> Result<(), BackendError>;

    /// Takes the next pending event, if any. Must not block.
    fn poll_event(&mut self) -> Option<BackendEvent>;
}
