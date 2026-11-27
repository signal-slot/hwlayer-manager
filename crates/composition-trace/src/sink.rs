//! A [`Trace`] sink that writes one line per entry.

use core::fmt::{self, Write};

use composition_core::{Trace, TraceEvent};

use crate::{write_event, write_header, Header, Stamp};

/// Formats every trace entry as one line into a writer the caller supplies.
///
/// `clock` gives the `t` field of each line, read inside
/// [`Trace::record`]; its unit is the one named in the `hdr` line. Write
/// the header with [`LineTrace::header`] before attaching the sink with
/// [`Engine::with_trace`](composition_core::Engine::with_trace); the sink
/// does not enforce it.
///
/// `seq` counts every line handed to the writer, including the header and
/// lines the writer failed to take, so a failed write shows as a gap in
/// `seq`. [`LineTrace::failed`] counts those failures. A writer that fails
/// in the middle of a line may leave a partial line, which the parser
/// refuses.
///
/// The sink cannot fill `tev` or `dur`: the core's entries carry neither.
pub struct LineTrace<W, C> {
    out: W,
    clock: C,
    seq: u64,
    failed: u64,
}

impl<W: Write, C: FnMut() -> u64> LineTrace<W, C> {
    /// A sink writing into `out`, stamping lines with `clock`. The first
    /// line gets `seq=0`.
    pub fn new(out: W, clock: C) -> Self {
        Self {
            out,
            clock,
            seq: 0,
            failed: 0,
        }
    }

    /// Writes the `hdr` line with the next `seq`.
    pub fn header(&mut self, header: &Header<'_>) -> fmt::Result {
        let seq = self.seq;
        self.seq += 1;
        let result = write_header(&mut self.out, seq, header);
        if result.is_err() {
            self.failed += 1;
        }
        result
    }

    /// The `seq` of the next line.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Number of lines that failed to format or write.
    pub fn failed(&self) -> u64 {
        self.failed
    }

    /// The writer.
    pub fn writer(&self) -> &W {
        &self.out
    }

    /// The writer, for example to drain a buffer between frames.
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.out
    }

    /// Gives the writer back.
    pub fn into_inner(self) -> W {
        self.out
    }
}

impl<W: Write, C: FnMut() -> u64> Trace for LineTrace<W, C> {
    fn record(&mut self, at: u64, event: TraceEvent<'_>) {
        let stamp = Stamp {
            seq: self.seq,
            t: (self.clock)(),
            at,
            tev: None,
            dur: None,
        };
        self.seq += 1;
        if write_event(&mut self.out, &stamp, &event).is_err() {
            self.failed += 1;
        }
    }
}
