//! The `--trace` flag and `trace-check` mode of the built binary, without a
//! display: the sample logs are the synthetic ones of
//! `scripts/testdata/trace-samples`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn harness(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_drm-harness"))
        .args(args)
        .output()
        .expect("run the harness")
}

fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/testdata/trace-samples")
        .join(name)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("drm-harness-trace-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn trace_check_prints_counts_of_a_good_file() {
    let out = harness(&["trace-check", sample("revoke.log").to_str().unwrap()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    for want in [
        "TRACE commit_ok 3\n",
        "TRACE commit_err 1\n",
        "TRACE latched 2\n",
        "TRACE released 3\n",
        "TRACE dropped 1\n",
        "TRACE dropped_device_lost 1\n",
        "TRACE device_lost 1\n",
    ] {
        assert!(stdout.contains(want), "missing {want:?} in\n{stdout}");
    }
    assert!(stdout.lines().all(|l| l.starts_with("TRACE ")));
}

#[test]
fn trace_check_refuses_malformed_files() {
    for bad in [
        "bad-malformed-line.log",
        "bad-unknown-event.log",
        "bad-unknown-version.log",
    ] {
        let out = harness(&["trace-check", sample(bad).to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(1), "{bad}");
        assert!(out.stdout.is_empty(), "{bad}: counted around the bad line");
        assert!(text(&out.stderr).contains("malformed trace line"), "{bad}");
    }
}

#[test]
fn trace_check_refuses_a_file_without_trace_lines_or_missing() {
    let empty = scratch("empty.log");
    std::fs::write(&empty, "VERDICT a PASS ok\n").unwrap();
    let out = harness(&["trace-check", empty.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let out = harness(&["trace-check", "/nonexistent/trace.log"]);
    assert_eq!(out.status.code(), Some(2));
    let out = harness(&["trace-check"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn trace_on_writes_the_header_and_a_trace_record() {
    // No device: the run fails before the engine exists, so the file holds
    // the header only, and it parses.
    let file = scratch("on.log");
    let _ = std::fs::remove_file(&file);
    let out = harness(&[
        "--trace",
        file.to_str().unwrap(),
        "--device",
        "/nonexistent/card",
        "two-planes",
    ]);
    let stdout = text(&out.stdout);
    assert!(stdout.contains("\ntrace lines=1 failed=0\n"), "{stdout}");
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(written.starts_with("CETRACE v=1 ev=hdr seq=0 src=linux clock=monotonic_ns "));
    let out = harness(&["trace-check", file.to_str().unwrap()]);
    assert!(out.status.success());
    assert!(text(&out.stdout).contains("TRACE hdr 1\n"));
}

#[test]
fn trace_off_leaves_the_output_as_it_was() {
    let out = harness(&["--device", "/nonexistent/card", "two-planes"]);
    let stdout = text(&out.stdout);
    assert!(!stdout.contains("trace lines="), "{stdout}");
    assert!(!stdout.contains("CETRACE"));
}

#[test]
fn trace_needs_a_writable_file() {
    let out = harness(&["--trace", "/nonexistent/dir/t.log", "two-planes"]);
    assert_eq!(out.status.code(), Some(2));
}
