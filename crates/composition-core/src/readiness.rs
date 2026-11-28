//! Readiness tokens for buffers whose content completes asynchronously.
//!
//! A producer that hands over a buffer before its writes have completed (a
//! GPU job, a DMA transfer, a camera frame) finishes the write with a
//! [`ReadyToken`] instead of declaring the content complete
//! ([`crate::Engine::finish_write_pending`]). The token names "a thing a
//! backend knows how to wait for": a `sync_file` descriptor number on Linux,
//! an index into a flag table on an MCU. The core never interprets it; it
//! stores one token per buffer and hands it to the backend with the frame.
//!
//! While the token is pending, a frame using the buffer is refused with
//! [`crate::PresentError::BufferNotReady`] unless the backend says it waits
//! for readiness itself ([`crate::DisplayBackend::waits_for_readiness`]).
//! The pending state ends when:
//!
//! - the client reports the token signalled
//!   ([`crate::Engine::readiness_signalled`]): the buffer is plainly ready;
//! - a waiting backend latches a submission showing the buffer: it waited,
//!   so the content was complete;
//! - the client reports the token failed ([`crate::Engine::readiness_failed`])
//!   or the readiness timeout expires
//!   ([`crate::Engine::set_readiness_timeout`]): the buffer is refused until
//!   it is written again, and a submission a waiting backend still holds for
//!   it is reported as [`crate::DisplayEvent::Dropped`];
//! - the buffer is written again ([`crate::Engine::begin_write`]) or
//!   unregistered.
//!
//! A signal that arrives after the failure or timeout changes nothing.

use crate::fixed::FixedVec;
use crate::limits::MAX_BUFFERS;
use crate::types::BufferId;

/// An opaque name for "the content will be complete once this signals".
///
/// Chosen by the producer-side glue and meaningful only to the backend that
/// waits for it, as a [`BufferId`] is meaningful only to the backend that
/// imported the buffer. The core compares tokens for equality and nothing
/// else; several buffers may share one token.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReadyToken(pub u64);

/// Readiness of a buffer that was finished with a token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReadinessState {
    /// The content is complete once the token signals.
    Pending(ReadyToken),
    /// The client reported that the token failed.
    Failed(ReadyToken),
    /// The token did not signal within the readiness timeout.
    TimedOut(ReadyToken),
}

impl ReadinessState {
    /// The token the buffer was finished with.
    pub fn token(self) -> ReadyToken {
        match self {
            Self::Pending(token) | Self::Failed(token) | Self::TimedOut(token) => token,
        }
    }

    /// Returns `true` while the token may still signal.
    pub fn is_pending(self) -> bool {
        matches!(self, Self::Pending(_))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReadinessEntry {
    pub(crate) buffer: BufferId,
    pub(crate) state: ReadinessState,
    /// When the buffer was finished with the token, in the client's ticks.
    pub(crate) since: u64,
}

impl Default for ReadinessEntry {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl ReadinessEntry {
    const EMPTY: Self = Self {
        buffer: BufferId(0),
        state: ReadinessState::Pending(ReadyToken(0)),
        since: 0,
    };
}

/// One readiness entry per registered buffer that was finished with a token
/// and has not become plainly ready since. Sized like the buffer table, so
/// it never fills up.
#[derive(Clone, Debug)]
pub(crate) struct Readiness {
    entries: FixedVec<ReadinessEntry, MAX_BUFFERS>,
}

impl Readiness {
    pub(crate) const EMPTY: Self = Self {
        entries: FixedVec::empty_filled(ReadinessEntry::EMPTY),
    };

    fn index(&self, buffer: BufferId) -> Option<usize> {
        self.entries.iter().position(|e| e.buffer == buffer)
    }

    /// Readiness of `buffer`, or `None` if it has no token.
    pub(crate) fn get(&self, buffer: BufferId) -> Option<ReadinessState> {
        self.index(buffer).map(|i| self.entries[i].state)
    }

    /// Records that `buffer` is complete once `token` signals.
    pub(crate) fn set_pending(&mut self, buffer: BufferId, token: ReadyToken, now: u64) {
        let entry = ReadinessEntry {
            buffer,
            state: ReadinessState::Pending(token),
            since: now,
        };
        match self.index(buffer) {
            Some(index) => self.entries.as_mut_slice()[index] = entry,
            // One entry per registered buffer: the table cannot be full.
            None => {
                let pushed = self.entries.push(entry);
                debug_assert!(pushed.is_ok(), "readiness table overflow");
            }
        }
    }

    /// Forgets the token of `buffer`. Returns `true` if it had one.
    pub(crate) fn remove(&mut self, buffer: BufferId) -> bool {
        match self.index(buffer) {
            Some(index) => {
                self.entries.swap_remove(index);
                true
            }
            None => false,
        }
    }

    /// Buffers whose readiness is pending on `token`.
    pub(crate) fn pending_on(&self, token: ReadyToken) -> impl Iterator<Item = BufferId> + '_ {
        self.entries
            .iter()
            .filter(move |e| e.state == ReadinessState::Pending(token))
            .map(|e| e.buffer)
    }

    /// Forgets every pending entry on `token`. Returns how many there were.
    pub(crate) fn signal(&mut self, token: ReadyToken) -> usize {
        let mut count = 0;
        let mut index = 0;
        while index < self.entries.len() {
            if self.entries[index].state == ReadinessState::Pending(token) {
                self.entries.swap_remove(index);
                count += 1;
            } else {
                index += 1;
            }
        }
        count
    }

    /// Turns every pending entry on `token` into `failed`, which is
    /// [`ReadinessState::Failed`] or [`ReadinessState::TimedOut`] of the same
    /// token. Returns how many there were.
    pub(crate) fn fail(&mut self, token: ReadyToken, failed: ReadinessState) -> usize {
        let mut count = 0;
        for entry in self.entries.as_mut_slice() {
            if entry.state == ReadinessState::Pending(token) {
                entry.state = failed;
                count += 1;
            }
        }
        count
    }

    /// A token pending since at least `timeout` ticks before `now`.
    pub(crate) fn expired(&self, now: u64, timeout: u64) -> Option<ReadyToken> {
        self.entries
            .iter()
            .find(|e| e.state.is_pending() && now.saturating_sub(e.since) >= timeout)
            .map(|e| e.state.token())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec::Vec;

    const A: BufferId = BufferId(1);
    const B: BufferId = BufferId(2);
    const T: ReadyToken = ReadyToken(7);
    const U: ReadyToken = ReadyToken(8);

    #[test]
    fn readiness_entry_is_pending_until_signalled() {
        let mut r = Readiness::EMPTY;
        assert_eq!(r.get(A), None);
        r.set_pending(A, T, 10);
        assert_eq!(r.get(A), Some(ReadinessState::Pending(T)));
        assert_eq!(r.signal(U), 0);
        assert_eq!(r.signal(T), 1);
        assert_eq!(r.get(A), None);
        // A second signal finds nothing.
        assert_eq!(r.signal(T), 0);
    }

    #[test]
    fn readiness_token_may_be_shared_by_buffers() {
        let mut r = Readiness::EMPTY;
        r.set_pending(A, T, 0);
        r.set_pending(B, T, 0);
        assert_eq!(r.pending_on(T).collect::<Vec<_>>().len(), 2);
        assert_eq!(r.fail(T, ReadinessState::Failed(T)), 2);
        assert_eq!(r.get(A), Some(ReadinessState::Failed(T)));
        assert_eq!(r.get(B), Some(ReadinessState::Failed(T)));
        assert_eq!(r.pending_on(T).count(), 0);
    }

    #[test]
    fn readiness_late_signal_after_failure_changes_nothing() {
        let mut r = Readiness::EMPTY;
        r.set_pending(A, T, 0);
        r.fail(T, ReadinessState::TimedOut(T));
        assert_eq!(r.signal(T), 0);
        assert_eq!(r.get(A), Some(ReadinessState::TimedOut(T)));
        // Only a new write clears it.
        assert!(r.remove(A));
        assert!(!r.remove(A));
        assert_eq!(r.get(A), None);
    }

    #[test]
    fn readiness_expiry_uses_the_time_of_finishing() {
        let mut r = Readiness::EMPTY;
        r.set_pending(A, T, 100);
        r.set_pending(B, U, 120);
        assert_eq!(r.expired(149, 50), None);
        assert_eq!(r.expired(150, 50), Some(T));
        r.fail(T, ReadinessState::TimedOut(T));
        assert_eq!(r.expired(150, 50), None);
        assert_eq!(r.expired(170, 50), Some(U));
    }

    #[test]
    fn readiness_new_token_replaces_the_old_one() {
        let mut r = Readiness::EMPTY;
        r.set_pending(A, T, 0);
        r.fail(T, ReadinessState::Failed(T));
        r.set_pending(A, U, 5);
        assert_eq!(r.get(A), Some(ReadinessState::Pending(U)));
        assert_eq!(r.signal(T), 0);
    }

    #[test]
    fn readiness_table_holds_one_entry_per_buffer() {
        let mut r = Readiness::EMPTY;
        for id in 0..MAX_BUFFERS as u32 {
            r.set_pending(BufferId(id), ReadyToken(u64::from(id)), 0);
        }
        // Setting an existing buffer again takes no new room.
        r.set_pending(BufferId(0), U, 0);
        assert_eq!(r.get(BufferId(0)), Some(ReadinessState::Pending(U)));
        assert_eq!(
            r.get(BufferId(MAX_BUFFERS as u32 - 1)),
            Some(ReadinessState::Pending(ReadyToken(MAX_BUFFERS as u64 - 1)))
        );
    }

    #[test]
    fn readiness_state_reports_its_token() {
        assert_eq!(ReadinessState::Pending(T).token(), T);
        assert_eq!(ReadinessState::Failed(T).token(), T);
        assert_eq!(ReadinessState::TimedOut(U).token(), U);
        assert!(ReadinessState::Pending(T).is_pending());
        assert!(!ReadinessState::Failed(T).is_pending());
    }
}
