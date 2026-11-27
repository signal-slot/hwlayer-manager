//! The strict parser refuses malformed lines with a typed error: at least
//! one case per `ErrorKind`.

use composition_trace::{parse_line, ErrorKind, ParseError};

const HDR: &str = "CETRACE v=1 ev=hdr seq=0 src=linux clock=monotonic_ns freq_hz=1000000000 t0=1 at_unit=same build=\"b\" board=\"b\"";

fn error(line: &str) -> ParseError {
    match parse_line(line) {
        Err(e) => e,
        Ok(r) => panic!("{line:?} was accepted as {r:?}"),
    }
}

fn check(kind: ErrorKind, offset: Option<usize>, lines: &[&str]) {
    for line in lines {
        let e = error(line);
        assert_eq!(e.kind, kind, "{line:?}: {e}");
        if let Some(offset) = offset {
            assert_eq!(e.offset, offset, "{line:?}: {e}");
        }
        // The message names the offset.
        assert!(e.to_string().ends_with(&format!("at byte {}", e.offset)));
    }
}

#[test]
fn good_lines_parse() {
    for line in [
        HDR,
        "CETRACE v=1 ev=commit seq=1 t=2 at=3 out=0 result=ok sub=1",
        // Field order after `ev=` is free.
        "CETRACE v=1 ev=commit at=3 sub=1 result=ok out=0 t=2 seq=1",
    ] {
        assert!(parse_line(line).unwrap().is_some(), "{line:?}");
    }
}

#[test]
fn syntax() {
    check(
        ErrorKind::Syntax,
        None,
        &[
            "CETRACE  v=1 ev=device_lost seq=0 t=0 at=0",
            "CETRACE v=1 ev=device_lost  seq=0 t=0 at=0",
            "CETRACE v=1 ev=device_lost seq t=0 at=0",
            "CETRACE v=1 ev=device_lost seq=0 t=0 at=",
            "CETRACE v=1 ev=device_lost Seq=0 t=0 at=0",
            "CETRACE v=1 ev=device_lost =0 seq=0 t=0 at=0",
            "CETRACE v=1 ev=device_lost seq=0 t=0 at=0 junk",
            "CETRACE v=1 ev=device_lost seq=0\nt=0 at=0",
            "CETRACE v=1 ev=device_lost seq=0 t=0 at=0\r",
            "CETRACE v=1 ev=device_lost seq=0 t=0 at=0\n\n",
        ],
    );
    assert_eq!(
        error("CETRACE v=1 ev=hdr seq=0 build=\"open").kind,
        ErrorKind::Syntax
    );
}

#[test]
fn trailing_garbage() {
    check(
        ErrorKind::TrailingGarbage,
        Some(41),
        &["CETRACE v=1 ev=device_lost seq=0 t=0 at=0 "],
    );
    check(
        ErrorKind::TrailingGarbage,
        None,
        &[
            &format!("{HDR} "),
            "CETRACE v=1 ev=hdr seq=0 src=linux clock=monotonic_ns freq_hz=1 t0=1 at_unit=same build=\"b\"x board=\"b\"",
            &format!("{HDR}x"),
        ],
    );
}

#[test]
fn unknown_version() {
    check(
        ErrorKind::UnknownVersion(2),
        Some(10),
        &[
            "CETRACE v=2 ev=device_lost seq=0 t=0 at=0",
            // Checked before anything else on the line.
            "CETRACE v=2 ev=frobnicate whatever=1",
        ],
    );
    check(
        ErrorKind::UnknownVersion(0),
        None,
        &["CETRACE v=0 ev=device_lost seq=0 t=0 at=0"],
    );
}

#[test]
fn unknown_event() {
    check(
        ErrorKind::UnknownEvent,
        Some(15),
        &[
            "CETRACE v=1 ev=frobnicate seq=0 t=0 at=0",
            "CETRACE v=1 ev=Commit seq=0 t=0 at=0",
        ],
    );
}

#[test]
fn unknown_key() {
    check(
        ErrorKind::UnknownKey,
        Some(42),
        &[
            "CETRACE v=1 ev=device_lost seq=0 t=0 at=0 kind=primary",
            "CETRACE v=1 ev=device_lost seq=0 t=0 at=0 fmt=XR24 seq=0",
        ],
    );
}

#[test]
fn unexpected_key() {
    check(
        ErrorKind::UnexpectedKey("out"),
        Some(42),
        &["CETRACE v=1 ev=device_lost seq=0 t=0 at=0 out=0"],
    );
    check(
        ErrorKind::UnexpectedKey("tev"),
        None,
        &["CETRACE v=1 ev=commit seq=1 t=2 at=3 out=0 result=ok sub=1 tev=4"],
    );
    check(
        ErrorKind::UnexpectedKey("dur"),
        None,
        &["CETRACE v=1 ev=latched seq=1 t=2 at=3 out=0 sub=1 dur=4"],
    );
    check(
        ErrorKind::UnexpectedKey("sub"),
        None,
        &["CETRACE v=1 ev=commit seq=1 t=2 at=3 out=0 result=backend.busy sub=1"],
    );
    check(
        ErrorKind::UnexpectedKey("n"),
        None,
        &["CETRACE v=1 ev=validate seq=1 t=2 at=3 out=0 layers=1 n=0 assign=- result=stale_plan"],
    );
    check(
        ErrorKind::UnexpectedKey("t"),
        None,
        &[&format!("{HDR} t=0")],
    );
    check(
        ErrorKind::UnexpectedKey("buf"),
        None,
        &["CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=0 assign=- result=busy buf=1"],
    );
}

#[test]
fn duplicate_key() {
    check(
        ErrorKind::DuplicateKey("seq"),
        Some(42),
        &["CETRACE v=1 ev=device_lost seq=0 t=0 at=0 seq=0"],
    );
    check(
        ErrorKind::DuplicateKey("v"),
        None,
        &["CETRACE v=1 ev=device_lost seq=0 t=0 at=0 v=1"],
    );
    check(
        ErrorKind::DuplicateKey("ev"),
        None,
        &["CETRACE v=1 ev=device_lost seq=0 t=0 at=0 ev=device_lost"],
    );
}

#[test]
fn missing_field() {
    check(
        ErrorKind::MissingField("v"),
        None,
        &["CETRACE", "CETRACE ev=hdr v=1"],
    );
    check(
        ErrorKind::MissingField("ev"),
        None,
        &["CETRACE v=1", "CETRACE v=1 seq=0 ev=hdr"],
    );
    let line = "CETRACE v=1 ev=device_lost seq=0 at=0";
    check(ErrorKind::MissingField("t"), Some(line.len()), &[line]);
    check(
        ErrorKind::MissingField("sub"),
        None,
        &[
            "CETRACE v=1 ev=commit seq=1 t=2 at=3 out=0 result=ok",
            "CETRACE v=1 ev=released seq=1 t=2 at=3 buf=0",
        ],
    );
    check(
        ErrorKind::MissingField("reason"),
        None,
        &["CETRACE v=1 ev=dropped seq=1 t=2 at=3 out=0 sub=1"],
    );
    check(
        ErrorKind::MissingField("assign"),
        None,
        &["CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=0 result=ok"],
    );
    check(
        ErrorKind::MissingField("reasons"),
        None,
        &["CETRACE v=1 ev=validate seq=1 t=2 at=3 out=0 layers=1 result=plan.no_plane layer=0"],
    );
    check(
        ErrorKind::MissingField("err_sub"),
        None,
        &["CETRACE v=1 ev=commit seq=1 t=2 at=3 out=0 result=present.unexpected_latch err_out=0"],
    );
    check(
        ErrorKind::MissingField("board"),
        None,
        &["CETRACE v=1 ev=hdr seq=0 src=linux clock=monotonic_ns freq_hz=1 t0=1 at_unit=same build=\"b\""],
    );
}

#[test]
fn bad_number() {
    check(
        ErrorKind::BadNumber("seq"),
        Some(31),
        &[
            "CETRACE v=1 ev=device_lost seq=01 t=0 at=0",
            "CETRACE v=1 ev=device_lost seq=-1 t=0 at=0",
            "CETRACE v=1 ev=device_lost seq=1x t=0 at=0",
            "CETRACE v=1 ev=device_lost seq=18446744073709551616 t=0 at=0",
        ],
    );
    check(
        ErrorKind::BadNumber("v"),
        Some(10),
        &["CETRACE v=one ev=hdr"],
    );
    check(
        ErrorKind::BadNumber("out"),
        None,
        &["CETRACE v=1 ev=latched seq=1 t=2 at=3 out=4294967296 sub=1"],
    );
    check(
        ErrorKind::BadNumber("errno"),
        None,
        &[
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=0 assign=- result=platform errno=-0",
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=0 assign=- result=platform errno=2147483648",
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=0 assign=- result=platform errno=--1",
        ],
    );
    check(
        ErrorKind::BadNumber("reasons"),
        None,
        &[
            "CETRACE v=1 ev=validate seq=1 t=2 at=3 out=0 layers=1 result=plan.no_plane layer=0 reasons=4",
            "CETRACE v=1 ev=validate seq=1 t=2 at=3 out=0 layers=1 result=plan.no_plane layer=0 reasons=0x",
            "CETRACE v=1 ev=validate seq=1 t=2 at=3 out=0 layers=1 result=plan.no_plane layer=0 reasons=0x04",
            "CETRACE v=1 ev=validate seq=1 t=2 at=3 out=0 layers=1 result=plan.no_plane layer=0 reasons=0xA",
            "CETRACE v=1 ev=validate seq=1 t=2 at=3 out=0 layers=1 result=plan.no_plane layer=0 reasons=0x100000000",
        ],
    );
    check(
        ErrorKind::BadNumber("freq_hz"),
        None,
        &["CETRACE v=1 ev=hdr seq=0 src=linux clock=monotonic_ns freq_hz=1e9 t0=1 at_unit=same build=\"b\" board=\"b\""],
    );
}

#[test]
fn bad_value() {
    check(
        ErrorKind::BadValue("result"),
        None,
        &[
            "CETRACE v=1 ev=commit seq=1 t=2 at=3 out=0 result=fine",
            "CETRACE v=1 ev=commit seq=1 t=2 at=3 out=0 result=busy",
            "CETRACE v=1 ev=commit seq=1 t=2 at=3 out=0 result=plan.stale_plan",
            "CETRACE v=1 ev=commit seq=1 t=2 at=3 out=0 result=backend.backend.busy",
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=0 assign=- result=backend.busy",
        ],
    );
    check(
        ErrorKind::BadValue("reason"),
        None,
        &["CETRACE v=1 ev=dropped seq=1 t=2 at=3 out=0 sub=1 reason=late"],
    );
    check(
        ErrorKind::BadValue("src"),
        None,
        &["CETRACE v=1 ev=hdr seq=0 src=qnx clock=monotonic_ns freq_hz=1 t0=1 at_unit=same build=\"b\" board=\"b\""],
    );
    check(
        ErrorKind::BadValue("clock"),
        None,
        &["CETRACE v=1 ev=hdr seq=0 src=linux clock=realtime freq_hz=1 t0=1 at_unit=same build=\"b\" board=\"b\""],
    );
    check(
        ErrorKind::BadValue("freq_hz"),
        None,
        &["CETRACE v=1 ev=hdr seq=0 src=linux clock=monotonic_ns freq_hz=0 t0=1 at_unit=same build=\"b\" board=\"b\""],
    );
    check(
        ErrorKind::BadValue("build"),
        None,
        &[
            "CETRACE v=1 ev=hdr seq=0 src=linux clock=monotonic_ns freq_hz=1 t0=1 at_unit=same build=b board=\"b\"",
            "CETRACE v=1 ev=hdr seq=0 src=linux clock=monotonic_ns freq_hz=1 t0=1 at_unit=same build=\"tab\there\" board=\"b\"",
        ],
    );
    check(
        ErrorKind::BadValue("at_unit"),
        None,
        &["CETRACE v=1 ev=hdr seq=0 src=linux clock=monotonic_ns freq_hz=1 t0=1 at_unit=\"ms\" build=\"b\" board=\"b\""],
    );
    check(
        ErrorKind::BadValue("seq"),
        None,
        &[
            "CETRACE v=1 ev=device_lost seq=\"0\" t=0 at=0",
            "CETRACE v=1 ev=device_lost seq=0\"x t=0 at=0",
            "CETRACE v=1 ev=device_lost seq=caf\u{e9} t=0 at=0",
            // `+` is outside the word character set.
            "CETRACE v=1 ev=device_lost seq=+1 t=0 at=0",
        ],
    );
    check(
        ErrorKind::BadValue("assign"),
        None,
        &[
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=1 assign=0:31:0:0:0,0,1,1 result=ok",
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=1 assign=0:31:0:0:0,0,1,1:0,0,1,1:9 result=ok",
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=1 assign=0:31:0:0:0,0,1:0,0,1,1 result=ok",
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=1 assign=0:31:0:0:0,0,1,-1:0,0,1,1 result=ok",
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=2 assign=0:31:0:0:0,0,1,1:0,0,1,1; result=ok",
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=0 assign=-- result=ok",
        ],
    );
    // Bits the core does not define (bit 11 and up).
    check(
        ErrorKind::BadValue("reasons"),
        None,
        &["CETRACE v=1 ev=validate seq=1 t=2 at=3 out=0 layers=1 result=plan.no_plane layer=0 reasons=0x800"],
    );
}

#[test]
fn count_mismatch() {
    check(
        ErrorKind::CountMismatch,
        Some(44),
        &[
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=2 assign=0:31:0:0:0,0,1,1:0,0,1,1 result=ok",
            "CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=1 assign=- result=ok",
        ],
    );
}

#[test]
fn too_many_assignments() {
    let item = "0:31:0:0:0,0,1,1:0,0,1,1";
    let list = vec![item; 17].join(";");
    let line = format!("CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=17 assign={list} result=ok");
    check(ErrorKind::TooManyAssignments, None, &[&line]);
    let list = vec![item; 16].join(";");
    let line = format!("CETRACE v=1 ev=check seq=1 t=2 at=3 out=0 n=16 assign={list} result=ok");
    assert!(parse_line(&line).unwrap().is_some());
}
