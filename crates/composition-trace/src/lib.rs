//! Text line format for the structured traces of `composition-core`.
//!
//! One [`composition_core::TraceEvent`] is one line of ASCII text:
//!
//! ```text
//! CETRACE v=1 ev=commit seq=3 t=1000300000 at=1000200000 out=0 result=ok sub=1
//! ```
//!
//! The format is specified in `docs/trace-format.md` of the repository
//! (version 1). This crate provides:
//!
//! - a formatter, [`write_header`] and [`write_event`], that writes one line
//!   into any [`core::fmt::Write`];
//! - a strict parser, [`parse_line`], that turns one line back into a typed
//!   [`Record`] and refuses anything the format does not allow with a
//!   [`ParseError`];
//! - a [`Trace`](composition_core::Trace) sink, [`LineTrace`], that formats
//!   every entry into a writer the caller supplies, for
//!   [`Engine::with_trace`](composition_core::Engine::with_trace).
//!
//! The crate is `no_std`, allocation-free and contains no `unsafe`, like the
//! core.

#![no_std]
#![forbid(unsafe_code)]

mod format;
mod parse;
mod sink;

pub use format::{write_event, write_header};
pub use parse::{parse_line, ErrorKind, Event, ParseError, Record};
pub use sink::LineTrace;

use composition_core::fixed::FixedVec;
use composition_core::limits::MAX_LAYERS;
use composition_core::Assignment;

/// The token every trace line starts with.
pub const PREFIX: &str = "CETRACE";

/// The format version this crate writes and the only one it parses.
pub const VERSION: u32 = 1;

/// Owned resource assignments of a parsed line, at most [`MAX_LAYERS`].
pub type Assignments = FixedVec<Assignment, MAX_LAYERS>;

/// Which kind of platform produced a log (`src=` of the `hdr` line).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// `linux`.
    Linux,
    /// `zephyr`.
    Zephyr,
}

impl Source {
    /// The value written for it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Source::Linux => "linux",
            Source::Zephyr => "zephyr",
        }
    }
}

/// The clock of the `t`, `tev` and `dur` fields (`clock=` of the `hdr`
/// line).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Clock {
    /// `monotonic_ns`: Linux `CLOCK_MONOTONIC` in nanoseconds.
    MonotonicNs,
    /// `cyc32`: a 32-bit cycle counter, such as Zephyr `k_cycle_get_32`.
    Cyc32,
    /// `cyc64`: a 64-bit cycle counter.
    Cyc64,
    /// `uptime_ticks`: Zephyr `k_uptime_ticks`.
    UptimeTicks,
}

impl Clock {
    /// The value written for it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Clock::MonotonicNs => "monotonic_ns",
            Clock::Cyc32 => "cyc32",
            Clock::Cyc64 => "cyc64",
            Clock::UptimeTicks => "uptime_ticks",
        }
    }
}

/// The fields of the `hdr` line, written once per run before the first
/// event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Header<'a> {
    /// The producing platform.
    pub src: Source,
    /// The clock of `t`, `tev` and `dur`.
    pub clock: Clock,
    /// Ticks per second of that clock, read from the running system. Never
    /// 0.
    pub freq_hz: u64,
    /// The clock value at the time of the header.
    pub t0: u64,
    /// `same` if `at` uses the clock of `t`, otherwise a unit such as `ms`.
    /// One word: ASCII letters, digits and `-_.:/,;`.
    pub at_unit: &'a str,
    /// The producer's source revision. Printable ASCII without `"`.
    pub build: &'a str,
    /// Free text such as the SoC name; no address, host name, user or
    /// serial path. Printable ASCII without `"`.
    pub board: &'a str,
}

/// The fields every event line carries besides the event itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Stamp {
    /// Line counter of the run, starting at 0 (the `hdr` line).
    pub seq: u64,
    /// Time the sink took inside `Trace::record`, in the `hdr` clock.
    pub t: u64,
    /// The `at` argument of `Trace::record`.
    pub at: u64,
    /// Time of the hardware event. Only allowed on `latched`.
    pub tev: Option<u64>,
    /// Ticks spent in the operation. Only allowed on `validate` and
    /// `commit`.
    pub dur: Option<u64>,
}

/// Whether `s` is a `word` value of the grammar.
pub(crate) fn is_word(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(is_word_byte)
}

pub(crate) fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':' | b'/' | b',' | b';')
}

/// Whether `s` may stand between the quotes of a `quoted` value.
pub(crate) fn is_quotable(s: &str) -> bool {
    s.bytes().all(|b| (0x20..=0x7e).contains(&b) && b != b'"')
}
