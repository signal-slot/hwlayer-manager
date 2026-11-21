//! Buffer and submission lifecycle.
//!
//! A buffer moves through four states:
//!
//! ```text
//! Writable --finish_write--> Ready --submit--> Submitted --latched--> Scanning
//!     ^                        |  ^                |                     |
//!     +------begin_write-------+  +----------------+---------------------+
//!                                          Released event
//! ```
//!
//! - **Writable**: the producer is drawing; the buffer must not be shown.
//! - **Ready**: the content is complete and the buffer is not in use.
//! - **Submitted**: part of an accepted submission the hardware has not
//!   latched yet.
//! - **Scanning**: part of the submission the hardware is showing.
//!
//! An accepted submission proves neither that the frame reached the screen
//! nor that the previous buffers are free. Those are reported separately, as
//! [`DisplayEvent::Presented`] and [`DisplayEvent::Released`]. A submission
//! replaced before it was latched is reported as [`DisplayEvent::Dropped`].

use crate::fixed::{FixedQueue, FixedVec};
use crate::limits::{MAX_BUFFERS, MAX_EVENTS, MAX_LAYERS, MAX_OUTPUTS};
use crate::planner::BufferLookup;
use crate::types::{BufferId, BufferInfo, OutputId, SubmissionId};

/// Lifecycle state of a registered buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BufferState {
    /// The producer may draw into the buffer.
    Writable,
    /// The content is complete and the buffer is idle.
    Ready,
    /// Accepted for display but not latched by the hardware yet.
    Submitted,
    /// Being shown by the hardware.
    Scanning,
}

/// What happened on the display.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DisplayEvent {
    /// The hardware latched a submission; it is now on screen.
    Presented {
        /// Output it was latched on.
        output: OutputId,
        /// The latched submission.
        submission: SubmissionId,
    },
    /// A submission was accepted but never reached the screen.
    Dropped {
        /// Output it was submitted to.
        output: OutputId,
        /// The dropped submission.
        submission: SubmissionId,
    },
    /// A buffer is no longer referenced by the display and may be reused.
    Released {
        /// The released buffer.
        buffer: BufferId,
    },
    /// The backend lost its display resources (reset, lease revoked).
    /// Everything pending was dropped and every buffer released.
    #[default]
    DeviceLost,
    /// The backend's capabilities changed; earlier plans are stale.
    CapabilitiesChanged,
    /// A submission has waited longer than the configured time for its
    /// latch. Reported once per submission. Nothing else changes: the
    /// submission stays pending and its buffers stay in use, because the
    /// hardware may still latch it. Recovery is the client's decision.
    LatchTimeout {
        /// Output the submission is pending on.
        output: OutputId,
        /// The overdue submission.
        submission: SubmissionId,
    },
}

/// Why a lifecycle operation was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PresentError {
    /// The buffer is not registered.
    UnknownBuffer(BufferId),
    /// A buffer with this identifier is already registered.
    DuplicateBuffer(BufferId),
    /// No room for another buffer.
    BufferTableFull,
    /// The buffer is submitted or on screen.
    BufferInUse(BufferId),
    /// The buffer is still being drawn and must not be shown.
    BufferNotReady(BufferId),
    /// The buffer is not being drawn, so there is nothing to finish.
    BufferNotWritable(BufferId),
    /// A submission is still waiting to be latched on this output.
    Busy(OutputId),
    /// More outputs in use than the core can track.
    TooManyOutputs,
    /// More buffers in one submission than the core can track.
    TooManyBuffers,
    /// The event queue has no room; drain it and retry.
    EventQueueFull,
    /// The backend reported a latch that matches no pending submission.
    UnexpectedLatch {
        /// Output the latch was reported for.
        output: OutputId,
        /// Submission the backend named.
        submission: SubmissionId,
    },
}

#[derive(Clone, Copy, Debug, Default)]
struct Entry {
    id: BufferId,
    info: BufferInfo,
    writable: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct Submission {
    id: SubmissionId,
    /// Distinct buffers of the submission.
    buffers: FixedVec<BufferId, MAX_LAYERS>,
    /// Time of submission, in the client's ticks.
    submitted_at: u64,
    /// Whether a latch timeout has been reported for it.
    overdue: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct OutputSlot {
    output: OutputId,
    pending: Option<Submission>,
    current: Option<Submission>,
}

/// Tracks registered buffers and what each output is showing or about to
/// show.
///
/// This is pure bookkeeping: it never touches hardware. The caller reports
/// what the backend accepted ([`Presentation::submit`]) and what the hardware
/// latched ([`Presentation::latched`]).
#[derive(Clone, Debug)]
pub struct Presentation {
    buffers: FixedVec<Entry, MAX_BUFFERS>,
    slots: FixedVec<OutputSlot, MAX_OUTPUTS>,
    events: FixedQueue<DisplayEvent, MAX_EVENTS>,
    next_id: u64,
    now: u64,
}

impl Default for Presentation {
    fn default() -> Self {
        Self::new()
    }
}

impl Presentation {
    /// Creates an empty tracker.
    pub fn new() -> Self {
        Self {
            buffers: FixedVec::new(),
            slots: FixedVec::new(),
            events: FixedQueue::new(),
            next_id: 1,
            now: 0,
        }
    }

    fn entry_index(&self, buffer: BufferId) -> Option<usize> {
        self.buffers.iter().position(|e| e.id == buffer)
    }

    fn usage(&self, buffer: BufferId) -> (bool, bool) {
        let mut pending = false;
        let mut current = false;
        for slot in self.slots.iter() {
            pending |= slot.pending.is_some_and(|s| s.buffers.contains(&buffer));
            current |= slot.current.is_some_and(|s| s.buffers.contains(&buffer));
        }
        (pending, current)
    }

    fn in_use(&self, buffer: BufferId) -> bool {
        let (pending, current) = self.usage(buffer);
        pending || current
    }

    /// Registers a buffer. It starts out [`BufferState::Writable`].
    pub fn register(&mut self, buffer: BufferId, info: BufferInfo) -> Result<(), PresentError> {
        if self.entry_index(buffer).is_some() {
            return Err(PresentError::DuplicateBuffer(buffer));
        }
        self.buffers
            .push(Entry {
                id: buffer,
                info,
                writable: true,
            })
            .map_err(|_| PresentError::BufferTableFull)
    }

    /// Removes a buffer that is neither submitted nor on screen.
    pub fn unregister(&mut self, buffer: BufferId) -> Result<(), PresentError> {
        let index = self
            .entry_index(buffer)
            .ok_or(PresentError::UnknownBuffer(buffer))?;
        if self.in_use(buffer) {
            return Err(PresentError::BufferInUse(buffer));
        }
        self.buffers.swap_remove(index);
        Ok(())
    }

    /// Current state of a buffer, or `None` if it is not registered.
    pub fn state(&self, buffer: BufferId) -> Option<BufferState> {
        let entry = self.buffers[self.entry_index(buffer)?];
        Some(match self.usage(buffer) {
            (_, true) => BufferState::Scanning,
            (true, false) => BufferState::Submitted,
            (false, false) if entry.writable => BufferState::Writable,
            (false, false) => BufferState::Ready,
        })
    }

    /// Hands an idle buffer to the producer for drawing.
    ///
    /// Fails with [`PresentError::BufferInUse`] while the buffer is submitted
    /// or on screen. Calling it on a buffer that is already writable is
    /// allowed.
    pub fn begin_write(&mut self, buffer: BufferId) -> Result<(), PresentError> {
        let index = self
            .entry_index(buffer)
            .ok_or(PresentError::UnknownBuffer(buffer))?;
        if self.in_use(buffer) {
            return Err(PresentError::BufferInUse(buffer));
        }
        self.buffers.as_mut_slice()[index].writable = true;
        Ok(())
    }

    /// Declares that the producer finished drawing; the buffer may be shown.
    pub fn finish_write(&mut self, buffer: BufferId) -> Result<(), PresentError> {
        let index = self
            .entry_index(buffer)
            .ok_or(PresentError::UnknownBuffer(buffer))?;
        let entry = &mut self.buffers.as_mut_slice()[index];
        if !entry.writable {
            return Err(PresentError::BufferNotWritable(buffer));
        }
        entry.writable = false;
        Ok(())
    }

    /// Identifier the next successful [`Presentation::submit`] will return.
    pub fn next_submission_id(&self) -> SubmissionId {
        SubmissionId(self.next_id)
    }

    /// Returns `true` if `output` has a submission waiting to be latched.
    pub fn has_pending(&self, output: OutputId) -> bool {
        self.slots
            .iter()
            .any(|s| s.output == output && s.pending.is_some())
    }

    /// Submission currently on screen on `output`, if any.
    pub fn on_screen(&self, output: OutputId) -> Option<SubmissionId> {
        self.slots
            .iter()
            .find(|s| s.output == output)
            .and_then(|s| s.current)
            .map(|s| s.id)
    }

    fn distinct(buffers: &[BufferId]) -> Result<FixedVec<BufferId, MAX_LAYERS>, PresentError> {
        let mut distinct = FixedVec::new();
        for buffer in buffers {
            if !distinct.contains(buffer) {
                distinct
                    .push(*buffer)
                    .map_err(|_| PresentError::TooManyBuffers)?;
            }
        }
        Ok(distinct)
    }

    /// Checks whether [`Presentation::submit`] would succeed, without
    /// changing anything.
    ///
    /// Call this before asking the hardware to commit, so that a commit the
    /// hardware accepted can always be recorded.
    pub fn check_submit(
        &self,
        output: OutputId,
        buffers: &[BufferId],
        replace_pending: bool,
    ) -> Result<(), PresentError> {
        Self::distinct(buffers)?;
        for buffer in buffers {
            let index = self
                .entry_index(*buffer)
                .ok_or(PresentError::UnknownBuffer(*buffer))?;
            if self.buffers[index].writable {
                return Err(PresentError::BufferNotReady(*buffer));
            }
        }
        match self.slots.iter().find(|s| s.output == output) {
            Some(slot) if slot.pending.is_some() && !replace_pending => {
                return Err(PresentError::Busy(output));
            }
            Some(_) => {}
            None if self.slots.remaining() == 0 => return Err(PresentError::TooManyOutputs),
            None => {}
        }
        // Worst case: the latch of every pending submission, which the engine
        // may still have to record between the backend's commit and this
        // submission (`Engine::commit`), then a Dropped plus a Released per
        // buffer of the replaced one if it did not latch, and the latch of
        // the new submission itself, which `Engine::commit` records right
        // after it. A latch is a Presented plus a Released per buffer of the
        // frame it replaces on screen; with a pending submission, either it
        // latches or it is dropped, so one share covers both. One more for a
        // DeviceLost or CapabilitiesChanged collected then.
        let pending = self.slots.iter().filter(|s| s.pending.is_some()).count();
        if self.events.remaining() < (pending + 1) * (MAX_LAYERS + 1) + 1 {
            return Err(PresentError::EventQueueFull);
        }
        Ok(())
    }

    /// Records that the backend accepted a submission showing `buffers` on
    /// `output`.
    ///
    /// With `replace_pending`, a submission still waiting to be latched is
    /// replaced and reported as [`DisplayEvent::Dropped`]; without it, the
    /// call fails with [`PresentError::Busy`].
    pub fn submit(
        &mut self,
        output: OutputId,
        buffers: &[BufferId],
        replace_pending: bool,
    ) -> Result<SubmissionId, PresentError> {
        self.check_submit(output, buffers, replace_pending)?;
        let id = SubmissionId(self.next_id);
        self.next_id += 1;
        let submission = Submission {
            id,
            buffers: Self::distinct(buffers)?,
            submitted_at: self.now,
            overdue: false,
        };

        let index = match self.slots.iter().position(|s| s.output == output) {
            Some(index) => index,
            None => {
                // Room was verified by `check_submit`.
                let _ = self.slots.push(OutputSlot {
                    output,
                    pending: None,
                    current: None,
                });
                self.slots.len() - 1
            }
        };
        let replaced = self.slots.as_mut_slice()[index].pending.replace(submission);
        if let Some(old) = replaced {
            self.emit(DisplayEvent::Dropped {
                output,
                submission: old.id,
            });
            self.release_unused(&old.buffers);
        }
        Ok(id)
    }

    /// Records that the hardware latched `submission` on `output`.
    ///
    /// Emits [`DisplayEvent::Presented`], then [`DisplayEvent::Released`] for
    /// every buffer of the previous frame that is no longer referenced.
    pub fn latched(
        &mut self,
        output: OutputId,
        submission: SubmissionId,
    ) -> Result<(), PresentError> {
        let unexpected = PresentError::UnexpectedLatch { output, submission };
        let index = self
            .slots
            .iter()
            .position(|s| s.output == output)
            .ok_or(unexpected)?;
        if self.slots[index].pending.map(|s| s.id) != Some(submission) {
            return Err(unexpected);
        }
        if self.events.remaining() < MAX_LAYERS + 1 {
            return Err(PresentError::EventQueueFull);
        }
        let slot = &mut self.slots.as_mut_slice()[index];
        let previous = core::mem::replace(&mut slot.current, slot.pending.take());
        self.emit(DisplayEvent::Presented { output, submission });
        if let Some(old) = previous {
            self.release_unused(&old.buffers);
        }
        Ok(())
    }

    /// Forgets everything submitted or on screen, on every output.
    ///
    /// Use it when the backend lost the display. Emits
    /// [`DisplayEvent::Dropped`] for pending submissions and
    /// [`DisplayEvent::Released`] for every buffer that was in use.
    pub fn reset(&mut self) -> Result<(), PresentError> {
        let mut needed = 0;
        for slot in self.slots.iter() {
            needed += slot.pending.map_or(0, |s| 1 + s.buffers.len());
            needed += slot.current.map_or(0, |s| s.buffers.len());
        }
        if self.events.remaining() < needed {
            return Err(PresentError::EventQueueFull);
        }
        let slots = core::mem::take(&mut self.slots);
        for slot in slots.iter() {
            if let Some(pending) = slot.pending {
                self.emit(DisplayEvent::Dropped {
                    output: slot.output,
                    submission: pending.id,
                });
            }
        }
        // With the slots gone nothing is in use, so each buffer is released
        // exactly once even if several submissions referenced it.
        let mut released: FixedVec<BufferId, MAX_BUFFERS> = FixedVec::new();
        for slot in slots.iter() {
            for submission in [slot.pending, slot.current].into_iter().flatten() {
                for buffer in submission.buffers.iter() {
                    if !released.contains(buffer) {
                        // Every referenced buffer is registered, so this fits.
                        let _ = released.push(*buffer);
                        self.emit(DisplayEvent::Released { buffer: *buffer });
                    }
                }
            }
        }
        Ok(())
    }

    /// Tells the tracker the current time, in ticks of the client's choice.
    ///
    /// The core has no clock. Time only matters for
    /// [`Presentation::report_overdue`]; submissions are stamped with the
    /// latest value given here.
    pub fn set_time(&mut self, now: u64) {
        self.now = now;
    }

    /// Emits [`DisplayEvent::LatchTimeout`] for every pending submission
    /// that has waited at least `timeout` ticks and was not reported yet.
    pub fn report_overdue(&mut self, timeout: u64) {
        for index in 0..self.slots.len() {
            let slot = self.slots[index];
            let Some(pending) = slot.pending else {
                continue;
            };
            if pending.overdue || self.now.saturating_sub(pending.submitted_at) < timeout {
                continue;
            }
            let event = DisplayEvent::LatchTimeout {
                output: slot.output,
                submission: pending.id,
            };
            // With a full queue the report is retried on the next call.
            if self.events.push(event).is_ok() {
                if let Some(pending) = &mut self.slots.as_mut_slice()[index].pending {
                    pending.overdue = true;
                }
            }
        }
    }

    /// Queues an event raised outside the tracker.
    pub fn push_event(&mut self, event: DisplayEvent) -> Result<(), PresentError> {
        self.events
            .push(event)
            .map_err(|_| PresentError::EventQueueFull)
    }

    /// Takes the oldest queued event.
    pub fn poll_event(&mut self) -> Option<DisplayEvent> {
        self.events.pop()
    }

    /// Room left in the event queue.
    pub fn events_room(&self) -> usize {
        self.events.remaining()
    }

    /// Returns `true` if no event is queued.
    pub fn events_empty(&self) -> bool {
        self.events.is_empty()
    }

    fn emit(&mut self, event: DisplayEvent) {
        // Callers verify the room beforehand.
        let pushed = self.events.push(event);
        debug_assert!(pushed.is_ok(), "event queue overflow");
    }

    fn release_unused(&mut self, buffers: &[BufferId]) {
        for buffer in buffers {
            if !self.in_use(*buffer) {
                self.emit(DisplayEvent::Released { buffer: *buffer });
            }
        }
    }
}

impl BufferLookup for Presentation {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.entry_index(buffer).map(|i| self.buffers[i].info)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec::Vec;

    const OUT: OutputId = OutputId(0);
    const A: BufferId = BufferId(1);
    const B: BufferId = BufferId(2);
    const C: BufferId = BufferId(3);

    fn tracker(ready: &[BufferId]) -> Presentation {
        let mut p = Presentation::new();
        for buffer in ready {
            p.register(*buffer, BufferInfo::default()).unwrap();
            p.finish_write(*buffer).unwrap();
        }
        p
    }

    fn drain(p: &mut Presentation) -> Vec<DisplayEvent> {
        core::iter::from_fn(|| p.poll_event()).collect()
    }

    fn presented(id: SubmissionId) -> DisplayEvent {
        DisplayEvent::Presented {
            output: OUT,
            submission: id,
        }
    }

    #[test]
    fn buffer_walks_through_every_state() {
        let mut p = Presentation::new();
        p.register(A, BufferInfo::default()).unwrap();
        p.register(B, BufferInfo::default()).unwrap();
        assert_eq!(p.state(A), Some(BufferState::Writable));
        p.finish_write(A).unwrap();
        p.finish_write(B).unwrap();
        assert_eq!(p.state(A), Some(BufferState::Ready));

        let first = p.submit(OUT, &[A], false).unwrap();
        assert_eq!(p.state(A), Some(BufferState::Submitted));
        // Accepted is not presented: nothing has been reported yet.
        assert!(drain(&mut p).is_empty());

        p.latched(OUT, first).unwrap();
        assert_eq!(p.state(A), Some(BufferState::Scanning));
        assert_eq!(drain(&mut p), [presented(first)]);

        // A stays on screen until the next frame is latched, not submitted.
        let second = p.submit(OUT, &[B], false).unwrap();
        assert_eq!(p.state(A), Some(BufferState::Scanning));
        assert!(drain(&mut p).is_empty());

        p.latched(OUT, second).unwrap();
        assert_eq!(
            drain(&mut p),
            [presented(second), DisplayEvent::Released { buffer: A }]
        );
        assert_eq!(p.state(A), Some(BufferState::Ready));
        assert_eq!(p.on_screen(OUT), Some(second));
    }

    #[test]
    fn unready_buffer_is_never_submitted() {
        let mut p = Presentation::new();
        p.register(A, BufferInfo::default()).unwrap();
        assert_eq!(
            p.submit(OUT, &[A], false),
            Err(PresentError::BufferNotReady(A))
        );
        assert_eq!(
            p.submit(OUT, &[B], false),
            Err(PresentError::UnknownBuffer(B))
        );
    }

    #[test]
    fn buffer_in_use_cannot_be_written_or_removed() {
        let mut p = tracker(&[A, B]);
        let first = p.submit(OUT, &[A], false).unwrap();
        assert_eq!(p.begin_write(A), Err(PresentError::BufferInUse(A)));
        assert_eq!(p.unregister(A), Err(PresentError::BufferInUse(A)));
        p.latched(OUT, first).unwrap();
        assert_eq!(p.begin_write(A), Err(PresentError::BufferInUse(A)));

        let second = p.submit(OUT, &[B], false).unwrap();
        // Still on screen while the replacement is only submitted.
        assert_eq!(p.begin_write(A), Err(PresentError::BufferInUse(A)));
        p.latched(OUT, second).unwrap();
        p.begin_write(A).unwrap();
        assert_eq!(p.state(A), Some(BufferState::Writable));
        assert_eq!(p.finish_write(B), Err(PresentError::BufferNotWritable(B)));
        p.unregister(A).unwrap();
        assert_eq!(p.state(A), None);
    }

    #[test]
    fn pending_submission_blocks_or_is_dropped() {
        let mut p = tracker(&[A, B, C]);
        let first = p.submit(OUT, &[A], false).unwrap();
        p.latched(OUT, first).unwrap();
        drain(&mut p);

        let second = p.submit(OUT, &[B], false).unwrap();
        assert_eq!(p.submit(OUT, &[C], false), Err(PresentError::Busy(OUT)));

        let third = p.submit(OUT, &[C], true).unwrap();
        assert_eq!(
            drain(&mut p),
            [
                DisplayEvent::Dropped {
                    output: OUT,
                    submission: second
                },
                DisplayEvent::Released { buffer: B }
            ]
        );
        // The dropped submission can no longer be latched.
        assert_eq!(
            p.latched(OUT, second),
            Err(PresentError::UnexpectedLatch {
                output: OUT,
                submission: second
            })
        );
        p.latched(OUT, third).unwrap();
        assert_eq!(
            drain(&mut p),
            [presented(third), DisplayEvent::Released { buffer: A }]
        );
    }

    #[test]
    fn buffer_kept_across_frames_is_not_released() {
        let mut p = tracker(&[A, B, C]);
        let first = p.submit(OUT, &[A, B], false).unwrap();
        p.latched(OUT, first).unwrap();
        drain(&mut p);

        // A stays, B is swapped for C; A also appears twice in one frame.
        let second = p.submit(OUT, &[A, C, A], false).unwrap();
        p.latched(OUT, second).unwrap();
        assert_eq!(
            drain(&mut p),
            [presented(second), DisplayEvent::Released { buffer: B }]
        );
        assert_eq!(p.state(A), Some(BufferState::Scanning));
    }

    #[test]
    fn buffer_shared_between_outputs_is_released_once_free_everywhere() {
        let other = OutputId(1);
        let mut p = tracker(&[A, B]);
        let on_first = p.submit(OUT, &[A], false).unwrap();
        let on_second = p.submit(other, &[A], false).unwrap();
        p.latched(OUT, on_first).unwrap();
        p.latched(other, on_second).unwrap();
        drain(&mut p);

        let next = p.submit(OUT, &[B], false).unwrap();
        p.latched(OUT, next).unwrap();
        // Still on the second output.
        assert_eq!(drain(&mut p), [presented(next)]);
        assert_eq!(p.state(A), Some(BufferState::Scanning));
    }

    #[test]
    fn latch_must_match_the_pending_submission() {
        let mut p = tracker(&[A]);
        let wrong = PresentError::UnexpectedLatch {
            output: OUT,
            submission: SubmissionId(1),
        };
        assert_eq!(p.latched(OUT, SubmissionId(1)), Err(wrong));
        let first = p.submit(OUT, &[A], false).unwrap();
        p.latched(OUT, first).unwrap();
        // A second latch report for the same submission is not accepted.
        assert_eq!(
            p.latched(OUT, first),
            Err(PresentError::UnexpectedLatch {
                output: OUT,
                submission: first
            })
        );
    }

    #[test]
    fn reset_drops_and_releases_everything() {
        let mut p = tracker(&[A, B]);
        let first = p.submit(OUT, &[A], false).unwrap();
        p.latched(OUT, first).unwrap();
        let second = p.submit(OUT, &[A, B], false).unwrap();
        drain(&mut p);

        p.reset().unwrap();
        assert_eq!(
            drain(&mut p),
            [
                DisplayEvent::Dropped {
                    output: OUT,
                    submission: second
                },
                DisplayEvent::Released { buffer: A },
                DisplayEvent::Released { buffer: B },
            ]
        );
        assert_eq!(p.state(A), Some(BufferState::Ready));
        assert_eq!(p.on_screen(OUT), None);
        // Identifiers are not reused after a reset.
        assert!(p.submit(OUT, &[A], false).unwrap() > second);
    }

    #[test]
    fn overdue_latch_is_reported_once_and_changes_nothing() {
        let mut p = tracker(&[A, B]);
        p.set_time(1000);
        let first = p.submit(OUT, &[A], false).unwrap();
        p.set_time(1015);
        p.report_overdue(16);
        assert!(drain(&mut p).is_empty());

        p.set_time(1016);
        p.report_overdue(16);
        p.report_overdue(16);
        assert_eq!(
            drain(&mut p),
            [DisplayEvent::LatchTimeout {
                output: OUT,
                submission: first
            }]
        );
        // Still pending and still protected.
        assert_eq!(p.state(A), Some(BufferState::Submitted));
        assert!(p.has_pending(OUT));

        // A late latch is still a latch.
        p.latched(OUT, first).unwrap();
        assert_eq!(drain(&mut p), [presented(first)]);

        // The next submission gets its own deadline.
        let second = p.submit(OUT, &[B], false).unwrap();
        p.report_overdue(16);
        assert!(drain(&mut p).is_empty());
        p.set_time(1040);
        p.report_overdue(16);
        assert_eq!(
            drain(&mut p),
            [DisplayEvent::LatchTimeout {
                output: OUT,
                submission: second
            }]
        );
    }

    #[test]
    fn full_event_queue_refuses_without_changing_state() {
        let mut p = tracker(&[A]);
        while p.push_event(DisplayEvent::CapabilitiesChanged).is_ok() {}
        assert_eq!(
            p.submit(OUT, &[A], false),
            Err(PresentError::EventQueueFull)
        );
        assert_eq!(p.state(A), Some(BufferState::Ready));
        drain(&mut p);
        p.submit(OUT, &[A], false).unwrap();
    }
}
