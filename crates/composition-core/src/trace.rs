//! Structured traces of what the engine decided and what the display did.
//!
//! An [`crate::Engine`] reports every validation, every backend check
//! (`TEST_ONLY` on Linux), every commit and every lifecycle event to a
//! [`Trace`] sink as a [`TraceEvent`], stamped with the time last given to
//! [`crate::Engine::advance_time`]. The sink decides what to keep: a ring
//! buffer on an MCU, a log line or a `Vec` on a host.
//!
//! Tracing is off by default: an engine built with [`crate::Engine::new`]
//! traces into [`NoTrace`], whose calls compile to nothing. Attach a sink
//! with [`crate::Engine::with_trace`].
//!
//! Lifecycle events are traced when the engine records them, not when the
//! client polls them, so the trace shows the order in which things
//! happened. An operation's own entry ([`TraceEvent::Validate`],
//! [`TraceEvent::Commit`]) is traced when the operation returns, after the
//! entries of what happened during it.

use crate::backend::BackendError;
use crate::engine::Error;
use crate::planner::Assignment;
use crate::types::{BufferId, OutputId, SubmissionId};

/// Why a submission never reached the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DropReason {
    /// A newer submission replaced it before it was latched.
    Replaced,
    /// The display resources were lost.
    DeviceLost,
}

/// One entry of a trace.
///
/// Slices borrow from the engine and live only for the call to
/// [`Trace::record`]; copy what is to be kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceEvent<'a> {
    /// The backend was asked whether it takes a candidate assignment, one
    /// entry per candidate the planner tried.
    Check {
        /// Output of the frame.
        output: OutputId,
        /// The candidate.
        assignments: &'a [Assignment],
        /// The backend's answer. [`BackendError::Rejected`] makes the
        /// planner try the next candidate; any other error ends the search.
        result: Result<(), BackendError>,
    },
    /// A frame was validated.
    Validate {
        /// Output of the frame.
        output: OutputId,
        /// Number of layers in the frame.
        layers: usize,
        /// The resource assignment, layer to plane, or why there is none.
        result: Result<&'a [Assignment], Error>,
    },
    /// A frame was committed.
    Commit {
        /// Output of the frame.
        output: OutputId,
        /// The submission the hardware accepted, or why it was refused.
        result: Result<SubmissionId, Error>,
    },
    /// The hardware latched a submission; it is on screen.
    Latched {
        /// Output it was latched on.
        output: OutputId,
        /// The latched submission.
        submission: SubmissionId,
    },
    /// A buffer is no longer referenced by the display.
    Released {
        /// The released buffer.
        buffer: BufferId,
        /// The submission whose end released it: the one taken off screen
        /// by a latch, or the one replaced. `None` when the device was lost,
        /// which releases every buffer at once.
        submission: Option<SubmissionId>,
    },
    /// A submission was accepted but never reached the screen.
    Dropped {
        /// Output it was submitted to.
        output: OutputId,
        /// The dropped submission.
        submission: SubmissionId,
        /// Why.
        reason: DropReason,
    },
    /// A submission has waited longer than the latch timeout. It stays
    /// pending; see [`crate::DisplayEvent::LatchTimeout`].
    LatchTimeout {
        /// Output the submission is pending on.
        output: OutputId,
        /// The overdue submission.
        submission: SubmissionId,
    },
    /// The backend lost its display resources.
    DeviceLost,
    /// The backend's capabilities changed; earlier plans are stale.
    CapabilitiesChanged,
}

/// Receives the entries of a trace.
pub trait Trace {
    /// Whether entries are wanted at all. The engine prepares no entry while
    /// this is `false`; [`NoTrace`] returns `false`, so with it tracing
    /// costs nothing.
    #[inline(always)]
    fn enabled(&self) -> bool {
        true
    }

    /// Records one entry. `at` is the time last given to
    /// [`crate::Engine::advance_time`], 0 before the first call.
    fn record(&mut self, at: u64, event: TraceEvent<'_>);
}

/// The sink that keeps nothing, used when no trace is attached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NoTrace;

impl Trace for NoTrace {
    #[inline(always)]
    fn enabled(&self) -> bool {
        false
    }

    #[inline(always)]
    fn record(&mut self, _at: u64, _event: TraceEvent<'_>) {}
}

/// A borrowed sink, so that the client keeps the sink (and can read it)
/// while the engine writes into it, including `&mut dyn Trace`.
impl<T: Trace + ?Sized> Trace for &mut T {
    #[inline(always)]
    fn enabled(&self) -> bool {
        (**self).enabled()
    }

    #[inline(always)]
    fn record(&mut self, at: u64, event: TraceEvent<'_>) {
        (**self).record(at, event);
    }
}
