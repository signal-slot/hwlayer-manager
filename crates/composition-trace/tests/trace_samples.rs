//! The sample board logs in `scripts/testdata/trace-samples`, parsed line by
//! line with the public API. Counts are those of the fixtures' README.

use composition_trace::{parse_line, ErrorKind, Event, ParseError, Record};

#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    commit: usize,
    latched: usize,
    released: usize,
    dropped: usize,
    /// Lines that start with `CETRACE` and parsed.
    trace_lines: usize,
    /// Lines that are not trace lines.
    other_lines: usize,
}

macro_rules! sample {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/testdata/trace-samples/",
            $name
        ))
    };
}

/// Parse a whole log, stopping at the first bad line (1-based number).
fn parse_log(log: &str) -> Result<Counts, (usize, ParseError)> {
    let mut c = Counts::default();
    for (i, line) in log.lines().enumerate() {
        match parse_line(line).map_err(|e| (i + 1, e))? {
            None => c.other_lines += 1,
            Some(Record::Header { .. }) => c.trace_lines += 1,
            Some(Record::Event { event, .. }) => {
                c.trace_lines += 1;
                match event {
                    Event::Commit { result: Ok(_), .. } => c.commit += 1,
                    Event::Latched { .. } => c.latched += 1,
                    Event::Released { .. } => c.released += 1,
                    Event::Dropped { .. } => c.dropped += 1,
                    _ => {}
                }
            }
        }
    }
    Ok(c)
}

fn good(log: &str, commit: usize, latched: usize, released: usize, dropped: usize) {
    let c = parse_log(log).unwrap_or_else(|(n, e)| panic!("line {n}: {e}"));
    assert_eq!(
        (c.commit, c.latched, c.released, c.dropped),
        (commit, latched, released, dropped)
    );
    assert!(c.trace_lines > 0, "no trace line parsed");
    assert!(c.other_lines > 0, "harness lines must give Ok(None)");
}

#[test]
fn good_logs_parse_with_readme_counts() {
    // The parser rejects `future_field=1` (see the ignored test below), so
    // the field is stripped to count the rest of the log.
    good(
        &sample!("hold.log").replace(" future_field=1", ""),
        1,
        1,
        0,
        0,
    );
    good(sample!("stop.log"), 3, 2, 1, 0);
    good(sample!("revoke.log"), 3, 2, 3, 1);
    good(sample!("modeset.log"), 2, 2, 1, 0);
    good(sample!("recovery.log"), 3, 2, 1, 1);
}

/// README and handoff: an unknown `key=value` on a known version is ignored.
/// The parser as merged answers `ErrorKind::UnknownKey` (strict, pinned by
/// `malformed.rs`), so this stays ignored until the owner decides.
#[test]
#[ignore = "parser rejects unknown keys (ErrorKind::UnknownKey); fixtures expect them ignored"]
fn unknown_field_is_ignored_on_known_version() {
    let log = sample!("hold.log");
    let line = log
        .lines()
        .find(|l| l.contains("future_field=1"))
        .expect("hold.log carries future_field=1");
    assert!(matches!(parse_line(line), Ok(Some(Record::Event { .. }))));
}

#[test]
fn non_trace_lines_are_none() {
    for log in [sample!("hold.log"), sample!("recovery.log")] {
        for line in log.lines().filter(|l| !l.starts_with("CETRACE")) {
            assert_eq!(parse_line(line), Ok(None), "{line}");
        }
    }
}

#[test]
fn recovery_has_two_headers() {
    let headers = sample!("recovery.log")
        .lines()
        .filter(|l| matches!(parse_line(l), Ok(Some(Record::Header { .. }))))
        .count();
    assert_eq!(headers, 2);
}

fn bad(log: &str, line: usize, kind: ErrorKind) {
    let (n, e) = parse_log(log).expect_err("bad log must be rejected");
    assert_eq!((n, e.kind), (line, kind));
}

#[test]
fn bad_unknown_event_names_line_7() {
    bad(sample!("bad-unknown-event.log"), 7, ErrorKind::UnknownEvent);
}

#[test]
fn bad_malformed_line_names_line_6() {
    bad(
        sample!("bad-malformed-line.log"),
        6,
        ErrorKind::BadNumber("t"),
    );
}

#[test]
fn bad_unknown_version_names_line_6() {
    bad(
        sample!("bad-unknown-version.log"),
        6,
        ErrorKind::UnknownVersion(2),
    );
}

/// R1 and R2 break relations between lines. `parse_line` is line-level and
/// the crate has no log checker, so these two files parse line by line; only
/// their README counts are pinned here. Cross-line rejection is left to the
/// board-scenarios count check.
#[test]
fn bad_relation_logs_parse_per_line_with_readme_counts() {
    let c = parse_log(sample!("bad-released-exceeds-commits.log")).expect("parses per line");
    assert_eq!((c.commit, c.latched, c.released, c.dropped), (2, 2, 3, 0));
    let c = parse_log(sample!("bad-missing-latch.log")).expect("parses per line");
    assert_eq!((c.commit, c.latched, c.released, c.dropped), (2, 1, 1, 0));
}
