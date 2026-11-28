//! `--trace FILE`: the engine's structured trace as `CETRACE` lines
//! (`docs/trace-format.md`), and `trace-check FILE`, which parses such a file
//! strictly and prints the counts per event type.
//!
//! One process-wide sink serves every engine of the run (a scenario may
//! reopen its backend, and `all` runs several sessions), so `seq` counts
//! across them. Lines are collected in memory and appended to the file when
//! the buffer is large, after each scenario and at exit, so the file is
//! complete after SIGTERM too (a stop ends the scenarios normally, see
//! `stop.rs`). Without `--trace` the sink reports itself disabled and the
//! engine prepares no entry.

use std::fs::File;
use std::io::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use composition_core::{DropReason, Trace, TraceEvent};
use composition_trace::{parse_line, Clock, Event, Header, LineTrace, Record, Source};

/// Buffered text that triggers a write to the file.
const FLUSH_AT: usize = 64 * 1024;

type Line = LineTrace<String, fn() -> u64>;

struct Sink {
    file: File,
    line: Line,
    /// Writes to the file that failed.
    write_failures: u64,
}

impl Sink {
    fn flush(&mut self) {
        let text = std::mem::take(self.line.writer_mut());
        if text.is_empty() {
            return;
        }
        if self.file.write_all(text.as_bytes()).is_err() {
            self.write_failures += 1;
        }
    }
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static SINK: Mutex<Option<Sink>> = Mutex::new(None);

/// `CLOCK_MONOTONIC` in nanoseconds.
fn now_ns() -> u64 {
    let t = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    u64::try_from(t.tv_sec)
        .unwrap_or(0)
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::try_from(t.tv_nsec).unwrap_or(0))
}

/// Creates FILE and writes the `hdr` line; engines built with
/// [`SharedTrace`] write into it from now on.
pub fn enable(path: &Path) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("--trace: cannot create the file: {e}"))?;
    let clock: fn() -> u64 = now_ns;
    let mut line = LineTrace::new(String::new(), clock);
    line.header(&Header {
        src: Source::Linux,
        clock: Clock::MonotonicNs,
        freq_hz: 1_000_000_000,
        t0: now_ns(),
        at_unit: "ms",
        build: env!("CARGO_PKG_VERSION"),
        board: "linux-drm",
    })
    .map_err(|_| "--trace: cannot format the header".to_string())?;
    let mut sink = Sink {
        file,
        line,
        write_failures: 0,
    };
    sink.flush();
    if sink.write_failures > 0 {
        return Err("--trace: cannot write the file".into());
    }
    *SINK.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink);
    ENABLED.store(true, Ordering::Relaxed);
    Ok(())
}

/// Writes what is buffered to the file.
pub fn flush() {
    if let Some(sink) = SINK.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        sink.flush();
    }
}

/// Flushes and returns `(lines, failures)`: lines handed to the sink, and
/// lines it failed to format plus writes that failed. `None` without
/// `--trace`.
pub fn finish() -> Option<(u64, u64)> {
    let mut guard = SINK.lock().unwrap_or_else(|e| e.into_inner());
    let sink = guard.as_mut()?;
    sink.flush();
    Some((sink.line.seq(), sink.line.failed() + sink.write_failures))
}

/// The engine's sink: forwards to the process-wide one.
#[derive(Clone, Copy, Debug, Default)]
pub struct SharedTrace;

impl Trace for SharedTrace {
    fn enabled(&self) -> bool {
        ENABLED.load(Ordering::Relaxed)
    }

    fn record(&mut self, at: u64, event: TraceEvent<'_>) {
        let mut guard = SINK.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sink) = guard.as_mut() {
            sink.line.record(at, event);
            if sink.line.writer().len() >= FLUSH_AT {
                sink.flush();
            }
        }
    }
}

/// Counts of a trace file, in the order `trace-check` prints them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub lines: u64,
    pub hdr: u64,
    pub check: u64,
    pub validate: u64,
    pub commit_ok: u64,
    pub commit_err: u64,
    pub latched: u64,
    pub released: u64,
    pub dropped_replaced: u64,
    pub dropped_device_lost: u64,
    pub latch_timeout: u64,
    pub device_lost: u64,
    pub caps_changed: u64,
}

impl Counts {
    /// The `TRACE <type> <count>` lines, in a fixed order, zeros included.
    pub fn lines_text(&self) -> String {
        let rows: [(&str, u64); 15] = [
            ("lines", self.lines),
            ("hdr", self.hdr),
            ("check", self.check),
            ("validate", self.validate),
            ("commit", self.commit_ok + self.commit_err),
            ("commit_ok", self.commit_ok),
            ("commit_err", self.commit_err),
            ("latched", self.latched),
            ("released", self.released),
            ("dropped", self.dropped_replaced + self.dropped_device_lost),
            ("dropped_replaced", self.dropped_replaced),
            ("dropped_device_lost", self.dropped_device_lost),
            ("latch_timeout", self.latch_timeout),
            ("device_lost", self.device_lost),
            ("caps_changed", self.caps_changed),
        ];
        rows.iter()
            .map(|(k, n)| format!("TRACE {k} {n}\n"))
            .collect()
    }
}

/// Parses `text` strictly: a line that starts with `CETRACE` must be a valid
/// trace line, and there must be at least one. Other lines are ignored, so a
/// whole harness log can be given.
pub fn count(text: &str) -> Result<Counts, String> {
    let mut c = Counts::default();
    for (index, line) in text.lines().enumerate() {
        let record = match parse_line(line) {
            Ok(None) => continue,
            Ok(Some(record)) => record,
            Err(e) => {
                return Err(format!(
                    "line {}: malformed trace line: {:?} at offset {}",
                    index + 1,
                    e.kind,
                    e.offset
                ))
            }
        };
        c.lines += 1;
        match record {
            Record::Header { .. } => c.hdr += 1,
            Record::Event { event, .. } => match event {
                Event::Check { .. } => c.check += 1,
                Event::Validate { .. } => c.validate += 1,
                Event::Commit { result, .. } => {
                    if result.is_ok() {
                        c.commit_ok += 1;
                    } else {
                        c.commit_err += 1;
                    }
                }
                Event::Latched { .. } => c.latched += 1,
                Event::Released { .. } => c.released += 1,
                Event::Dropped { reason, .. } => match reason {
                    DropReason::Replaced => c.dropped_replaced += 1,
                    DropReason::DeviceLost => c.dropped_device_lost += 1,
                },
                Event::LatchTimeout { .. } => c.latch_timeout += 1,
                Event::DeviceLost => c.device_lost += 1,
                Event::CapabilitiesChanged => c.caps_changed += 1,
            },
        }
    }
    if c.lines == 0 {
        return Err("no trace line (CETRACE) in the file".into());
    }
    Ok(c)
}

/// `drm-harness trace-check FILE`: prints the counts, or why not.
/// Exit 0 good, 1 malformed or empty, 2 unreadable.
pub fn check_file(path: &Path) -> std::process::ExitCode {
    use std::process::ExitCode;
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("drm-harness trace-check: cannot read the file: {e}");
            return ExitCode::from(2);
        }
    };
    match count(&text) {
        Ok(counts) => {
            print!("{}", counts.lines_text());
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("drm-harness trace-check: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use composition_core::{OutputId, SubmissionId};

    fn sample() -> String {
        let clock: fn() -> u64 = || 5;
        let mut sink = LineTrace::new(String::new(), clock);
        sink.header(&Header {
            src: Source::Linux,
            clock: Clock::MonotonicNs,
            freq_hz: 1_000_000_000,
            t0: 1,
            at_unit: "ms",
            build: "t",
            board: "t",
        })
        .unwrap();
        let out = OutputId(1);
        let sub = SubmissionId(1);
        sink.record(
            0,
            TraceEvent::Commit {
                output: out,
                result: Ok(sub),
            },
        );
        sink.record(
            1,
            TraceEvent::Latched {
                output: out,
                submission: sub,
            },
        );
        sink.record(
            2,
            TraceEvent::Dropped {
                output: out,
                submission: SubmissionId(2),
                reason: DropReason::DeviceLost,
            },
        );
        sink.record(3, TraceEvent::DeviceLost);
        sink.into_inner()
    }

    #[test]
    fn counts_each_event_type() {
        let c = count(&sample()).unwrap();
        assert_eq!(c.lines, 5);
        assert_eq!(c.hdr, 1);
        assert_eq!(c.commit_ok, 1);
        assert_eq!(c.latched, 1);
        assert_eq!(c.dropped_device_lost, 1);
        assert_eq!(c.device_lost, 1);
        let text = c.lines_text();
        assert!(text.starts_with("TRACE lines 5\nTRACE hdr 1\n"));
        assert!(text.contains("TRACE dropped 1\n"));
        assert!(text.contains("TRACE released 0\n"));
    }

    #[test]
    fn other_lines_are_ignored() {
        let text = format!("harness version=1\n{}VERDICT x PASS ok\n", sample());
        assert_eq!(count(&text).unwrap().lines, 5);
    }

    #[test]
    fn a_malformed_line_fails_the_file() {
        let text = format!("{}CETRACE v=1 ev=latched seq=abc\n", sample());
        let e = count(&text).unwrap_err();
        assert!(e.starts_with("line 6: malformed trace line"), "{e}");
        let text = sample().replace("ev=commit", "ev=frobnicate");
        assert!(count(&text).is_err());
        let text = sample().replace("v=1", "v=2");
        assert!(count(&text).is_err());
    }

    #[test]
    fn a_file_without_trace_lines_fails() {
        assert!(count("").is_err());
        assert!(count("VERDICT a PASS b\n").is_err());
    }

    #[test]
    fn disabled_sink_asks_for_no_entries() {
        // Nothing in the unit tests calls `enable`.
        assert!(!SharedTrace.enabled());
    }
}
