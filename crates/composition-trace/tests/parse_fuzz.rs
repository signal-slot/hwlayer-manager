//! Deterministic fuzz of the strict parser.
//!
//! A hand-written SplitMix64 generator with fixed seeds, no dependency.
//! Covers: generated events of every kind and reason with extreme numbers
//! round-trip; one-byte mutations, truncations and insertions of valid
//! lines are refused or parse to a record that formats back to exactly the
//! mutated input; arbitrary bytes never panic; non-trace lines in a mixed
//! log give `Ok(None)` as `parse_line` documents.

use composition_core::{
    Assignment, BackendError, BufferId, DropReason, Error, OutputId, PlanError, PlaneId,
    PresentError, Rect, RejectReasons, SubmissionId, TraceEvent,
};
use composition_trace::{
    parse_line, write_event, write_header, Clock, Header, Record, Source, Stamp,
};

const SEEDS: [u64; 4] = [0, 1, 0x5eed_0fd1, u64::MAX];
const ROUNDS: usize = 200;

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn u64(&mut self) -> u64 {
        match self.below(5) {
            0 => 0,
            1 => u64::MAX,
            2 => self.below(100),
            _ => self.next(),
        }
    }

    fn u32(&mut self) -> u32 {
        match self.below(5) {
            0 => 0,
            1 => u32::MAX,
            2 => self.below(100) as u32,
            _ => self.next() as u32,
        }
    }

    fn i32(&mut self) -> i32 {
        match self.below(6) {
            0 => 0,
            1 => i32::MIN,
            2 => i32::MAX,
            3 => -(self.below(5000) as i32),
            _ => self.below(5000) as i32,
        }
    }

    fn rect(&mut self) -> Rect {
        Rect::new(self.i32(), self.i32(), self.u32(), self.u32())
    }

    fn assignments(&mut self) -> Vec<Assignment> {
        (0..self.below(5))
            .map(|_| Assignment {
                layer: self.below(16) as usize,
                plane: PlaneId(self.u32()),
                plane_index: self.below(16) as usize,
                zpos: self.i32(),
                source: self.rect(),
                destination: self.rect(),
            })
            .collect()
    }

    fn reasons(&mut self) -> RejectReasons {
        let all = [
            RejectReasons::OUTPUT,
            RejectReasons::FORMAT,
            RejectReasons::SCALING,
            RejectReasons::SOURCE_SIZE,
            RejectReasons::ALPHA,
            RejectReasons::BLEND,
            RejectReasons::TRANSFORM,
            RejectReasons::FULL_OUTPUT,
            RejectReasons::Z_ORDER,
            RejectReasons::RESOURCE,
            RejectReasons::IN_USE,
        ];
        let mut r = RejectReasons::NONE;
        for a in all {
            if self.below(2) == 0 {
                r |= a;
            }
        }
        r
    }

    fn backend_error(&mut self) -> BackendError {
        match self.below(5) {
            0 => BackendError::Rejected,
            1 => BackendError::Busy,
            2 => BackendError::UnknownBuffer(BufferId(self.u32())),
            3 => BackendError::DeviceLost,
            _ => BackendError::Platform(self.i32()),
        }
    }

    fn error(&mut self) -> Error {
        let out = OutputId(self.u32());
        let buf = BufferId(self.u32());
        let layer = self.below(16) as usize;
        match self.below(24) {
            0 => Error::StalePlan,
            1 => Error::PlanMismatch,
            2 => Error::Backend(self.backend_error()),
            3 => Error::Plan(PlanError::UnknownOutput(out)),
            4 => Error::Plan(PlanError::TooManyLayers {
                count: self.below(1000) as usize,
            }),
            5 => Error::Plan(PlanError::UnknownBuffer { layer, buffer: buf }),
            6 => Error::Plan(PlanError::InvalidSource { layer }),
            7 => Error::Plan(PlanError::InvalidDestination { layer }),
            8 => Error::Plan(PlanError::CannotClip { layer }),
            9 => Error::Plan(PlanError::NoPlane {
                layer,
                reasons: self.reasons(),
            }),
            10 => Error::Plan(PlanError::BackendRejected {
                candidates: self.below(1000) as u32,
            }),
            11 => Error::Plan(PlanError::LimitExceeded),
            12 => Error::Plan(PlanError::Aborted),
            13 => Error::Present(PresentError::UnknownBuffer(buf)),
            14 => Error::Present(PresentError::DuplicateBuffer(buf)),
            15 => Error::Present(PresentError::BufferTableFull),
            16 => Error::Present(PresentError::BufferInUse(buf)),
            17 => Error::Present(PresentError::BufferNotReady(buf)),
            18 => Error::Present(PresentError::BufferNotWritable(buf)),
            19 => Error::Present(PresentError::Busy(out)),
            20 => Error::Present(PresentError::TooManyOutputs),
            21 => Error::Present(PresentError::TooManyBuffers),
            22 => Error::Present(PresentError::EventQueueFull),
            _ => Error::Present(PresentError::UnexpectedLatch {
                output: out,
                submission: SubmissionId(self.u64()),
            }),
        }
    }

    fn opt(&mut self) -> Option<u64> {
        if self.below(2) == 0 {
            None
        } else {
            Some(self.u64())
        }
    }
}

/// The line of an event of kind `kind` (0..9: every `TraceEvent` variant)
/// with random fields.
fn gen_line(rng: &mut SplitMix64, kind: u64) -> String {
    let assignments = rng.assignments();
    let output = OutputId(rng.u32());
    let submission = SubmissionId(rng.u64());
    let mut stamp = Stamp {
        seq: rng.u64(),
        t: rng.u64(),
        at: rng.u64(),
        tev: None,
        dur: None,
    };
    let event = match kind {
        0 => TraceEvent::Check {
            output,
            assignments: &assignments,
            result: if rng.below(2) == 0 {
                Ok(())
            } else {
                Err(rng.backend_error())
            },
        },
        1 => {
            stamp.dur = rng.opt();
            TraceEvent::Validate {
                output,
                layers: assignments.len(),
                result: if rng.below(2) == 0 {
                    Ok(&assignments)
                } else {
                    Err(rng.error())
                },
            }
        }
        2 => {
            stamp.dur = rng.opt();
            TraceEvent::Commit {
                output,
                result: if rng.below(2) == 0 {
                    Ok(submission)
                } else {
                    Err(rng.error())
                },
            }
        }
        3 => {
            stamp.tev = rng.opt();
            TraceEvent::Latched { output, submission }
        }
        4 => TraceEvent::Released {
            buffer: BufferId(rng.u32()),
            submission: if rng.below(2) == 0 {
                None
            } else {
                Some(submission)
            },
        },
        5 => TraceEvent::Dropped {
            output,
            submission,
            reason: if rng.below(2) == 0 {
                DropReason::Replaced
            } else {
                DropReason::DeviceLost
            },
        },
        6 => TraceEvent::LatchTimeout { output, submission },
        7 => TraceEvent::DeviceLost,
        _ => TraceEvent::CapabilitiesChanged,
    };
    let mut s = String::new();
    write_event(&mut s, &stamp, &event).unwrap();
    s
}

fn header_line(rng: &mut SplitMix64) -> String {
    let h = Header {
        src: if rng.below(2) == 0 {
            Source::Linux
        } else {
            Source::Zephyr
        },
        clock: [
            Clock::MonotonicNs,
            Clock::Cyc32,
            Clock::Cyc64,
            Clock::UptimeTicks,
        ][rng.below(4) as usize],
        freq_hz: rng.u64().max(1),
        t0: rng.u64(),
        at_unit: ["same", "ms", "us"][rng.below(3) as usize],
        build: ["", "abc123", "a b;c=d"][rng.below(3) as usize],
        board: ["", "RK3588", "x y z"][rng.below(3) as usize],
    };
    let mut s = String::new();
    write_header(&mut s, rng.u64(), &h).unwrap();
    s
}

/// One valid line of each kind, and a header.
fn valid_lines(rng: &mut SplitMix64) -> Vec<String> {
    let mut v: Vec<String> = (0..9).map(|k| gen_line(rng, k)).collect();
    v.push(header_line(rng));
    v
}

/// Formats a parsed record back into a line ending in `\n`.
fn reformat(record: &Record<'_>) -> String {
    let mut s = String::new();
    match record {
        Record::Header { seq, header } => write_header(&mut s, *seq, header).unwrap(),
        Record::Event { stamp, event } => {
            write_event(&mut s, stamp, &event.as_trace_event()).unwrap()
        }
    }
    s
}

/// The line without one trailing `\n` or `\r\n`.
fn body(line: &str) -> &str {
    line.strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .unwrap_or(line)
}

/// A line is refused, skipped, or parses to a record that formats back to
/// exactly its body. Never panics.
fn check_mutant(mutant: &str, what: &str) {
    if let Ok(Some(record)) = parse_line(mutant) {
        let back = reformat(&record);
        assert_eq!(
            body(&back),
            body(mutant),
            "{what}: {mutant:?} parsed but does not format back"
        );
    }
}

#[test]
fn generated_events_round_trip() {
    for seed in SEEDS {
        let mut rng = SplitMix64(seed);
        for _ in 0..ROUNDS {
            for kind in 0..9 {
                let line = gen_line(&mut rng, kind);
                let record = parse_line(&line)
                    .unwrap_or_else(|e| panic!("{line:?}: {e}"))
                    .unwrap();
                assert_eq!(reformat(&record), line);
                // Also without the line ending and with CRLF.
                assert_eq!(parse_line(body(&line)).unwrap(), Some(record));
                let crlf = format!("{}\r\n", body(&line));
                assert_eq!(parse_line(&crlf).unwrap(), Some(record));
            }
            let line = header_line(&mut rng);
            let record = parse_line(&line)
                .unwrap_or_else(|e| panic!("{line:?}: {e}"))
                .unwrap();
            assert_eq!(reformat(&record), line);
        }
    }
}

#[test]
fn extreme_numbers_round_trip() {
    let max = Stamp {
        seq: u64::MAX,
        t: u64::MAX,
        at: u64::MAX,
        tev: None,
        dur: None,
    };
    let zero = Stamp {
        seq: 0,
        t: 0,
        at: 0,
        tev: None,
        dur: None,
    };
    let big = SubmissionId(u64::MAX);
    let out = OutputId(u32::MAX);
    let mut events = vec![
        (
            Stamp {
                tev: Some(u64::MAX),
                ..max
            },
            TraceEvent::Latched {
                output: out,
                submission: big,
            },
        ),
        (
            Stamp {
                tev: Some(0),
                ..zero
            },
            TraceEvent::Latched {
                output: OutputId(0),
                submission: SubmissionId(0),
            },
        ),
        (
            Stamp {
                dur: Some(u64::MAX),
                ..max
            },
            TraceEvent::Commit {
                output: out,
                result: Ok(big),
            },
        ),
        (
            zero,
            TraceEvent::Released {
                buffer: BufferId(u32::MAX),
                submission: Some(big),
            },
        ),
        (
            max,
            TraceEvent::Released {
                buffer: BufferId(0),
                submission: None,
            },
        ),
        (
            zero,
            TraceEvent::LatchTimeout {
                output: out,
                submission: big,
            },
        ),
    ];
    for reason in [DropReason::Replaced, DropReason::DeviceLost] {
        events.push((
            max,
            TraceEvent::Dropped {
                output: out,
                submission: big,
                reason,
            },
        ));
    }
    for (stamp, event) in events {
        let mut s = String::new();
        write_event(&mut s, &stamp, &event).unwrap();
        let rec = parse_line(&s).unwrap().unwrap();
        assert_eq!(reformat(&rec), s);
    }
}

#[test]
fn single_byte_mutations_are_refused_or_format_back() {
    const BYTES: &[u8] = b" =\"09-;:,x\n\r";
    for seed in SEEDS {
        let mut rng = SplitMix64(seed);
        for line in valid_lines(&mut rng) {
            let base = body(&line).as_bytes().to_vec();
            for i in 0..base.len() {
                for &b in BYTES {
                    let mut m = base.clone();
                    m[i] = b;
                    if let Ok(s) = String::from_utf8(m) {
                        check_mutant(&s, "replace");
                    }
                }
                let mut m = base.clone();
                m[i] = rng.next() as u8;
                if let Ok(s) = String::from_utf8(m) {
                    check_mutant(&s, "random replace");
                }
                let mut m = base.clone();
                m.remove(i);
                check_mutant(&String::from_utf8(m).unwrap(), "delete");
            }
        }
    }
}

#[test]
fn insertions_and_truncations_are_refused_or_format_back() {
    const BYTES: &[u8] = b" =\"0-;\nz";
    for seed in SEEDS {
        let mut rng = SplitMix64(seed);
        for line in valid_lines(&mut rng) {
            let base = body(&line).as_bytes().to_vec();
            for i in 0..=base.len() {
                for &b in BYTES {
                    let mut m = base.clone();
                    m.insert(i, b);
                    check_mutant(&String::from_utf8(m).unwrap(), "insert");
                }
                check_mutant(std::str::from_utf8(&base[..i]).unwrap(), "truncate");
                check_mutant(std::str::from_utf8(&base[i..]).unwrap(), "cut front");
            }
            check_mutant(&format!("{} ", body(&line)), "trailing space");
            check_mutant(&format!("{0}{0}", body(&line)), "doubled");
            check_mutant(&format!("{} x=1", body(&line)), "unknown key");
        }
    }
}

#[test]
fn multi_byte_mutations_never_panic() {
    for seed in SEEDS {
        let mut rng = SplitMix64(seed);
        for _ in 0..ROUNDS {
            let lines = valid_lines(&mut rng);
            let line = &lines[rng.below(lines.len() as u64) as usize];
            let mut m = body(line).as_bytes().to_vec();
            for _ in 0..1 + rng.below(4) {
                if m.is_empty() {
                    break;
                }
                let i = rng.below(m.len() as u64) as usize;
                match rng.below(3) {
                    0 => m[i] = rng.next() as u8,
                    1 => {
                        m.remove(i);
                    }
                    _ => m.insert(i, rng.next() as u8),
                }
            }
            check_mutant(&String::from_utf8_lossy(&m), "multi");
        }
    }
}

#[test]
fn arbitrary_bytes_never_panic() {
    for seed in SEEDS {
        let mut rng = SplitMix64(seed);
        for _ in 0..ROUNDS * 5 {
            let len = rng.below(200) as usize;
            let bytes: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
            // Invalid UTF-8 cannot reach `parse_line(&str)`; a log reader
            // decodes lossily or skips the line.
            if let Ok(s) = std::str::from_utf8(&bytes) {
                check_mutant(s, "bytes");
            }
            check_mutant(&String::from_utf8_lossy(&bytes), "lossy");
            for prefix in [&b"CETRACE v=1 "[..], b"CETRACE v=1 ev=latched seq="] {
                let mut line = prefix.to_vec();
                line.extend_from_slice(&bytes);
                check_mutant(&String::from_utf8_lossy(&line), "prefixed");
            }
        }
    }
    // Invalid UTF-8 is replaced by U+FFFD, which no field accepts.
    let mut line = b"CETRACE v=1 ev=device_lost seq=1 t=2 at=3".to_vec();
    line.push(0xff);
    assert!(parse_line(&String::from_utf8_lossy(&line)).is_err());
}

#[test]
fn non_trace_lines_in_a_mixed_log_are_skipped() {
    for junk in [
        "",
        "\n",
        " ",
        "[    1.234] drm: card0 ready",
        "VERDICT pass",
        "cetrace v=1 ev=device_lost seq=1 t=2 at=3",
        " CETRACE v=1 ev=device_lost seq=1 t=2 at=3",
        "CETRACEv=1",
        "CETRACES v=1",
        "CETRACE2 v=1 ev=device_lost seq=1 t=2 at=3",
        "CETRACE:",
    ] {
        assert_eq!(parse_line(junk), Ok(None), "{junk:?}");
    }
    // The prefix alone, or followed by a space, is a trace line and strict.
    for bad in ["CETRACE", "CETRACE ", "CETRACE\n", "CETRACE x"] {
        assert!(parse_line(bad).is_err(), "{bad:?}");
    }
    // A whole log: valid lines parse, the rest is skipped.
    let mut rng = SplitMix64(7);
    let mut parsed = 0;
    for _ in 0..ROUNDS {
        let lines = valid_lines(&mut rng);
        let log: String = lines
            .iter()
            .flat_map(|l| ["boot: x\n", l.as_str()])
            .collect();
        for l in log.split_inclusive('\n') {
            match parse_line(l).unwrap() {
                Some(r) => {
                    parsed += 1;
                    assert_eq!(reformat(&r), l);
                }
                None => assert!(!l.starts_with("CETRACE")),
            }
        }
    }
    assert_eq!(parsed, ROUNDS * 10);
}
