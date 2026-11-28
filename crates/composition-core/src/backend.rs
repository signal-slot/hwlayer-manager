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
use crate::readiness::ReadyToken;
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
    /// Describes the outputs and planes the backend can drive right now, by
    /// adding them to `caps`, which arrives cleared
    /// ([`Capabilities::clear`]).
    ///
    /// The description must reflect what is actually usable, for example only
    /// the objects granted by a DRM lease.
    ///
    /// The description is written in place because it is large (about 9 KiB):
    /// returned by value, it and its temporaries would be reserved on the
    /// caller's stack, which on an MCU is a thread stack of a few KiB.
    fn capabilities(&mut self, caps: &mut Capabilities);

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
    /// replaced submission must never be reported as latched, with one
    /// exception: the hardware latched it before this commit took it over
    /// (for example at a vertical blank while the new frame was being
    /// written). That latch is real, and must be queued for
    /// [`DisplayBackend::poll_event`] by the time `commit` returns; the
    /// engine records it before it records the new submission. The new
    /// submission's own latch may be reported as soon as it happened, even
    /// before `commit` returns to the engine.
    fn commit(
        &mut self,
        submission: SubmissionId,
        frame: &Frame<'_>,
        plan: &Plan,
    ) -> Result<(), BackendError>;

    /// Takes the next pending event, if any. Must not block.
    fn poll_event(&mut self) -> Option<BackendEvent>;

    /// Whether the backend waits for readiness tokens ([`ReadyToken`])
    /// itself, so that a frame may be committed before its content is
    /// complete (see [`crate::readiness`]).
    ///
    /// The default is `false`: the engine then refuses a frame that uses a
    /// buffer whose readiness is pending, with
    /// [`crate::PresentError::BufferNotReady`], and never calls the other
    /// readiness methods.
    ///
    /// A backend that returns `true` must never let the hardware latch a
    /// commit before every token given for its buffers has signalled, or
    /// been cleared by [`DisplayBackend::clear_readiness`]. The engine takes
    /// the latch as proof that the content was complete.
    fn waits_for_readiness(&self) -> bool {
        false
    }

    /// Gives the token `buffer`'s content is complete on.
    ///
    /// The engine calls it before the checks of a validation, for every
    /// buffer of the frame whose readiness is pending, and again before each
    /// [`DisplayBackend::commit`] for those the plan shows. The token stays
    /// attached to the buffer until the next call for the buffer replaces
    /// it, [`DisplayBackend::clear_readiness`] or
    /// [`DisplayBackend::readiness_failed`] removes it, or a commit showing
    /// the buffer latched.
    ///
    /// A backend that cannot wait for the token must refuse it, never
    /// ignore it; the default refuses with [`BackendError::Rejected`]. An
    /// error fails the validation or the commit with it.
    fn set_readiness(&mut self, buffer: BufferId, token: ReadyToken) -> Result<(), BackendError> {
        let _ = (buffer, token);
        Err(BackendError::Rejected)
    }

    /// The content of `buffer` no longer depends on the token given for it:
    /// the client reported it signalled, or the buffer is written again or
    /// unregistered. A commit waiting only on it may latch.
    ///
    /// Called only on a backend that waits for readiness. The default does
    /// nothing.
    fn clear_readiness(&mut self, buffer: BufferId) {
        let _ = buffer;
    }

    /// `token` will never signal (the client reported an error, or the
    /// engine's readiness timeout expired).
    ///
    /// The backend must forget the token and must never report
    /// [`BackendEvent::Latched`] for a submission that waits on it: the
    /// engine reports such a submission as dropped and releases its buffers
    /// as soon as this returns. What is on screen stays.
    ///
    /// Called only on a backend that waits for readiness. The default does
    /// nothing.
    fn readiness_failed(&mut self, token: ReadyToken) {
        let _ = token;
    }
}
