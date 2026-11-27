//! `LineTrace` attached to an engine with the fake backend: every line it
//! writes parses, `seq` has no gap, and the counts a board checker would
//! compare add up.

use core::fmt;

use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, Engine, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, PlaneCaps, PlaneId, PlaneKind, Rect, Size, SubmissionId,
    Trace, TraceEvent, Zpos,
};
use composition_fake::FakeBackend;
use composition_trace::{parse_line, Clock, Event, Header, LineTrace, Record, Source};

const OUT: OutputId = OutputId(0);
const FULL: Rect = Rect::new(0, 0, 640, 480);
const BUFFERS: [BufferId; 3] = [BufferId(1), BufferId(2), BufferId(3)];

fn caps() -> Capabilities {
    let mut caps = Capabilities {
        replace_pending: true,
        ..Capabilities::default()
    };
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(640, 480),
            refresh_mhz: 60_000,
        })
        .unwrap();
    let mut plane = PlaneCaps {
        id: PlaneId(31),
        kind: PlaneKind::Primary,
        outputs: 1,
        zpos: Zpos::Fixed(0),
        blend_modes: BlendMode::Opaque.mask(),
        ..PlaneCaps::default()
    };
    plane
        .formats
        .push(FormatModifier::linear(PixelFormat::XRGB8888))
        .unwrap();
    caps.planes.push(plane).unwrap();
    caps
}

const HEADER: Header<'static> = Header {
    src: Source::Linux,
    clock: Clock::MonotonicNs,
    freq_hz: 1_000_000_000,
    t0: 0,
    at_unit: "same",
    build: "test",
    board: "fake",
};

#[test]
fn engine_trace_lines_parse_and_count() {
    let mut now = 0u64;
    let mut sink = LineTrace::new(String::new(), move || {
        now += 10;
        now
    });
    sink.header(&HEADER).unwrap();

    let mut engine = Engine::new(FakeBackend::new(caps())).with_trace(&mut sink);
    for b in BUFFERS {
        let info = BufferInfo {
            size: Size::new(640, 480),
            format: PixelFormat::XRGB8888,
            modifier: Modifier::LINEAR,
        };
        engine.register_buffer(b, info).unwrap();
        engine.finish_write(b).unwrap();
    }
    let present = |engine: &mut Engine<FakeBackend, &mut LineTrace<String, _>>, n: usize| {
        let layers = [Layer::new(BUFFERS[n % 3], FULL, FULL, 0)];
        engine.present(&Frame {
            output: OUT,
            layers: &layers,
        })
    };
    for n in 0..4 {
        engine.advance_time(16 * (n as u64 + 1));
        present(&mut engine, n).unwrap();
        engine.backend_mut().vsync();
        while engine.poll_event().unwrap().is_some() {}
    }
    // A replaced pending submission, a refused frame and a device loss.
    present(&mut engine, 4).unwrap();
    present(&mut engine, 5).unwrap();
    let unknown = [Layer::new(BufferId(99), FULL, FULL, 0)];
    assert!(engine
        .present(&Frame {
            output: OUT,
            layers: &unknown,
        })
        .is_err());
    engine.backend_mut().lose_device();
    while engine.poll_event().unwrap().is_some() {}
    drop(engine);

    assert_eq!(sink.failed(), 0);
    let log = sink.into_inner();
    let mut counts = std::collections::BTreeMap::<&str, u64>::new();
    let mut commits_ok = 0;
    for (seq, line) in log.lines().enumerate() {
        let record = parse_line(line)
            .unwrap_or_else(|e| panic!("{line:?}: {e}"))
            .unwrap();
        let name = match record {
            Record::Header { seq: s, header } => {
                assert_eq!((s, header), (0, HEADER));
                "hdr"
            }
            Record::Event { stamp, event } => {
                assert_eq!(stamp.seq, seq as u64, "{line}");
                assert_eq!(stamp.t, 10 * seq as u64, "{line}");
                match event {
                    Event::Check { .. } => "check",
                    Event::Validate { .. } => "validate",
                    Event::Commit { result, .. } => {
                        commits_ok += u64::from(result.is_ok());
                        "commit"
                    }
                    Event::Latched { .. } => "latched",
                    Event::Released { .. } => "released",
                    Event::Dropped { .. } => "dropped",
                    Event::LatchTimeout { .. } => "latch_timeout",
                    Event::DeviceLost => "device_lost",
                    Event::CapabilitiesChanged => "caps_changed",
                }
            }
        };
        *counts.entry(name).or_default() += 1;
    }
    // Six submissions: four latched, one replaced, one lost with the device.
    assert_eq!(commits_ok, 6);
    assert_eq!(counts["latched"], 4);
    assert_eq!(counts["dropped"], 2);
    assert_eq!(counts["latched"] + counts["dropped"], commits_ok);
    assert_eq!(counts["device_lost"], 1);
    assert_eq!(counts["validate"], 7);
    assert!(log.contains(" reason=replaced\n") && log.contains(" reason=device_lost\n"));
    assert!(log.contains(" sub=-\n"));
}

/// A writer that refuses everything.
struct Full;

impl fmt::Write for Full {
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Err(fmt::Error)
    }
}

#[test]
fn failed_writes_are_counted_and_leave_a_seq_gap() {
    let mut sink = LineTrace::new(Full, || 0);
    assert!(sink.header(&HEADER).is_err());
    sink.record(0, TraceEvent::DeviceLost);
    sink.record(
        0,
        TraceEvent::Latched {
            output: OUT,
            submission: SubmissionId(1),
        },
    );
    assert_eq!((sink.seq(), sink.failed()), (3, 3));
}
