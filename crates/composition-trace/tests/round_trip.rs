//! Format then parse: every `TraceEvent` variant, every `DropReason`, every
//! error code, the header, and the examples of `docs/trace-format.md`.

use composition_core::{
    Assignment, BackendError, BufferId, DropReason, Error, OutputId, PlanError, PlaneId,
    PresentError, Rect, RejectReasons, SubmissionId, TraceEvent,
};
use composition_trace::{
    parse_line, write_event, write_header, Clock, Header, Record, Source, Stamp,
};

fn format(stamp: &Stamp, event: &TraceEvent<'_>) -> String {
    let mut s = String::new();
    write_event(&mut s, stamp, event).unwrap();
    s
}

/// Formats, parses, checks the parsed entry equals the original, and that
/// formatting it again gives the same line.
fn round_trip(stamp: Stamp, event: TraceEvent<'_>) -> String {
    let line = format(&stamp, &event);
    assert!(line.ends_with('\n') && !line[..line.len() - 1].contains('\n'));
    let record = parse_line(&line)
        .unwrap_or_else(|e| panic!("{line:?}: {e}"))
        .unwrap();
    let Record::Event {
        stamp: parsed_stamp,
        event: parsed,
    } = record
    else {
        panic!("{line:?} parsed as a header");
    };
    assert_eq!(parsed_stamp, stamp, "{line:?}");
    assert_eq!(parsed.as_trace_event(), event, "{line:?}");
    assert_eq!(format(&parsed_stamp, &parsed.as_trace_event()), line);
    line
}

fn stamp(seq: u64) -> Stamp {
    Stamp {
        seq,
        t: 1_000_000_000 + seq,
        at: 1_000,
        tev: None,
        dur: None,
    }
}

const ASSIGNMENTS: [Assignment; 2] = [
    Assignment {
        layer: 0,
        plane: PlaneId(31),
        plane_index: 0,
        zpos: 0,
        source: Rect::new(0, 0, 1920, 1200),
        destination: Rect::new(0, 0, 1920, 1200),
    },
    Assignment {
        layer: 1,
        plane: PlaneId(35),
        plane_index: 2,
        zpos: -3,
        source: Rect::new(-4, 7, 256, 256),
        destination: Rect::new(i32::MIN, i32::MAX, u32::MAX, 1),
    },
];

fn backend_errors() -> Vec<BackendError> {
    vec![
        BackendError::Rejected,
        BackendError::Busy,
        BackendError::UnknownBuffer(BufferId(7)),
        BackendError::DeviceLost,
        BackendError::Platform(-22),
        BackendError::Platform(0),
        BackendError::Platform(i32::MIN),
        BackendError::Platform(i32::MAX),
    ]
}

fn engine_errors() -> Vec<Error> {
    let mut all = vec![Error::StalePlan, Error::PlanMismatch];
    all.extend(backend_errors().into_iter().map(Error::Backend));
    let reasons = [
        RejectReasons::NONE,
        RejectReasons::SCALING,
        RejectReasons::OUTPUT | RejectReasons::FORMAT | RejectReasons::IN_USE,
    ];
    all.extend(
        [
            PlanError::UnknownOutput(OutputId(3)),
            PlanError::TooManyLayers { count: 17 },
            PlanError::UnknownBuffer {
                layer: 1,
                buffer: BufferId(99),
            },
            PlanError::InvalidSource { layer: 2 },
            PlanError::InvalidDestination { layer: 3 },
            PlanError::CannotClip { layer: 4 },
            PlanError::BackendRejected { candidates: 32 },
            PlanError::LimitExceeded,
            PlanError::Aborted,
        ]
        .into_iter()
        .chain(
            reasons
                .into_iter()
                .map(|reasons| PlanError::NoPlane { layer: 5, reasons }),
        )
        .map(Error::Plan),
    );
    all.extend(
        [
            PresentError::UnknownBuffer(BufferId(1)),
            PresentError::DuplicateBuffer(BufferId(2)),
            PresentError::BufferTableFull,
            PresentError::BufferInUse(BufferId(3)),
            PresentError::BufferNotReady(BufferId(4)),
            PresentError::BufferNotWritable(BufferId(5)),
            PresentError::Busy(OutputId(1)),
            PresentError::TooManyOutputs,
            PresentError::TooManyBuffers,
            PresentError::EventQueueFull,
            PresentError::UnexpectedLatch {
                output: OutputId(2),
                submission: SubmissionId(u64::MAX),
            },
        ]
        .into_iter()
        .map(Error::Present),
    );
    all
}

#[test]
fn every_variant_round_trips() {
    let out = OutputId(0);
    let sub = SubmissionId(42);
    let mut lines = Vec::new();

    for assignments in [&ASSIGNMENTS[..], &ASSIGNMENTS[..1], &[]] {
        lines.push(round_trip(
            stamp(1),
            TraceEvent::Check {
                output: out,
                assignments,
                result: Ok(()),
            },
        ));
        lines.push(round_trip(
            stamp(2),
            TraceEvent::Validate {
                output: out,
                layers: assignments.len(),
                result: Ok(assignments),
            },
        ));
    }
    for error in backend_errors() {
        lines.push(round_trip(
            stamp(3),
            TraceEvent::Check {
                output: out,
                assignments: &ASSIGNMENTS,
                result: Err(error),
            },
        ));
    }
    for error in engine_errors() {
        lines.push(round_trip(
            stamp(4),
            TraceEvent::Validate {
                output: out,
                layers: 3,
                result: Err(error),
            },
        ));
        lines.push(round_trip(
            Stamp {
                dur: Some(80_000),
                ..stamp(5)
            },
            TraceEvent::Commit {
                output: out,
                result: Err(error),
            },
        ));
    }
    lines.push(round_trip(
        Stamp {
            dur: Some(0),
            ..stamp(6)
        },
        TraceEvent::Commit {
            output: OutputId(u32::MAX),
            result: Ok(sub),
        },
    ));
    for tev in [None, Some(1_016_667_000)] {
        lines.push(round_trip(
            Stamp { tev, ..stamp(7) },
            TraceEvent::Latched {
                output: out,
                submission: sub,
            },
        ));
    }
    for submission in [Some(sub), None] {
        lines.push(round_trip(
            stamp(8),
            TraceEvent::Released {
                buffer: BufferId(u32::MAX),
                submission,
            },
        ));
    }
    for reason in [DropReason::Replaced, DropReason::DeviceLost] {
        lines.push(round_trip(
            stamp(9),
            TraceEvent::Dropped {
                output: out,
                submission: sub,
                reason,
            },
        ));
    }
    lines.push(round_trip(
        stamp(10),
        TraceEvent::LatchTimeout {
            output: out,
            submission: sub,
        },
    ));
    lines.push(round_trip(stamp(11), TraceEvent::DeviceLost));
    lines.push(round_trip(
        Stamp {
            seq: u64::MAX,
            t: u64::MAX,
            at: u64::MAX,
            tev: None,
            dur: None,
        },
        TraceEvent::CapabilitiesChanged,
    ));

    // Spot checks of the exact text.
    assert!(lines.contains(
        &"CETRACE v=1 ev=released seq=8 t=1000000008 at=1000 buf=4294967295 sub=-\n".to_string()
    ));
    assert!(lines.contains(
        &"CETRACE v=1 ev=check seq=1 t=1000000001 at=1000 out=0 n=0 assign=- result=ok\n"
            .to_string()
    ));
    assert!(lines.contains(
        &"CETRACE v=1 ev=commit seq=5 t=1000000005 at=1000 out=0 result=present.unexpected_latch err_out=2 err_sub=18446744073709551615 dur=80000\n"
            .to_string()
    ));
    assert!(lines
        .iter()
        .any(|l| l.contains("result=plan.no_plane layer=5 reasons=0x0 ")));
    assert!(lines
        .iter()
        .any(|l| l.contains("result=plan.no_plane layer=5 reasons=0x403 ")));
}

#[test]
fn header_round_trips() {
    for (src, clock) in [
        (Source::Linux, Clock::MonotonicNs),
        (Source::Zephyr, Clock::Cyc32),
        (Source::Zephyr, Clock::Cyc64),
        (Source::Zephyr, Clock::UptimeTicks),
    ] {
        let header = Header {
            src,
            clock,
            freq_hz: 800_000_000,
            t0: 12_000,
            at_unit: "ms",
            build: "abc123 dirty (x=1)",
            board: "",
        };
        let mut line = String::new();
        write_header(&mut line, 0, &header).unwrap();
        assert_eq!(
            parse_line(&line).unwrap(),
            Some(Record::Header { seq: 0, header })
        );
    }
}

#[test]
fn formatter_refuses_what_the_parser_would() {
    let good = Header {
        src: Source::Linux,
        clock: Clock::MonotonicNs,
        freq_hz: 1_000_000_000,
        t0: 0,
        at_unit: "same",
        build: "b",
        board: "b",
    };
    for bad in [
        Header { freq_hz: 0, ..good },
        Header {
            at_unit: "",
            ..good
        },
        Header {
            at_unit: "two words",
            ..good
        },
        Header {
            build: "say \"hi\"",
            ..good
        },
        Header {
            board: "tab\there",
            ..good
        },
        Header {
            board: "caf\u{e9}",
            ..good
        },
    ] {
        let mut s = String::new();
        assert!(write_header(&mut s, 0, &bad).is_err(), "{bad:?}");
        assert!(s.is_empty());
    }

    let mut s = String::new();
    let tev = Stamp {
        tev: Some(1),
        ..stamp(1)
    };
    assert!(write_event(&mut s, &tev, &TraceEvent::DeviceLost).is_err());
    let dur = Stamp {
        dur: Some(1),
        ..stamp(1)
    };
    let latched = TraceEvent::Latched {
        output: OutputId(0),
        submission: SubmissionId(1),
    };
    assert!(write_event(&mut s, &dur, &latched).is_err());
    assert!(s.is_empty());
}

/// The lines of section 6 of the format document.
fn doc_examples() -> Vec<&'static str> {
    let doc = include_str!("../../../docs/trace-format.md");
    let start = doc.find("\n## 6.").expect("section 6");
    let end = start + doc[start..].find("\n## 7.").expect("section 7");
    doc[start..end]
        .lines()
        .filter(|l| l.starts_with("CETRACE"))
        .collect()
}

#[test]
fn every_example_of_the_document_parses_and_formats_back() {
    let examples = doc_examples();
    assert_eq!(examples.len(), 20, "{examples:#?}");
    for line in examples {
        let record = parse_line(line)
            .unwrap_or_else(|e| panic!("{line:?}: {e}"))
            .unwrap();
        let mut again = String::new();
        match record {
            Record::Header { seq, header } => write_header(&mut again, seq, &header).unwrap(),
            Record::Event { stamp, event } => {
                write_event(&mut again, &stamp, &event.as_trace_event()).unwrap()
            }
        }
        assert_eq!(again, format!("{line}\n"));
    }
}

#[test]
fn line_endings_and_foreign_lines() {
    let line = "CETRACE v=1 ev=device_lost seq=12 t=9500 at=9500";
    let expected = parse_line(line).unwrap().unwrap();
    assert_eq!(parse_line(&format!("{line}\n")).unwrap(), Some(expected));
    assert_eq!(parse_line(&format!("{line}\r\n")).unwrap(), Some(expected));
    for other in [
        "",
        "[    1.234] CETRACE v=1 ev=device_lost seq=0 t=0 at=0",
        "RESULT ok",
        "CETRACEX v=1",
        "cetrace v=1 ev=device_lost seq=0 t=0 at=0",
    ] {
        assert_eq!(parse_line(other).unwrap(), None, "{other:?}");
    }
}
