//! Host tests of `hvs_load_probe`'s parsing (`examples/hvs_load_probe/debugfs.rs`):
//! the vc4 debugfs files against the recorded BCM2711 dumps of 2026-11-28
//! (`tests/data/bcm2711/debugfs-2026-11-28-*.txt`), and the probe's `STEP`
//! and `SUMMARY` lines against synthetic text. No device is needed; nothing
//! here is a hardware result.

#[path = "../examples/hvs_load_probe/debugfs.rs"]
mod debugfs;

use debugfs::{parse_dlists, parse_step, parse_underrun, summary_line, Commit, Step, TestOnly};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/data/bcm2711/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn underrun_fixture_is_zero() {
    // hvs_underrun, then hvs_lbm, in one file: the count is the first line.
    let text = fixture("debugfs-2026-11-28-hvs_underrun_lbm.txt");
    assert_eq!(parse_underrun(&text), Ok(0));
}

#[test]
fn underrun_plain_and_bad() {
    assert_eq!(parse_underrun("17\n"), Ok(17));
    assert_eq!(parse_underrun("  3  \n"), Ok(3));
    assert!(parse_underrun("").is_err());
    assert!(parse_underrun("# only a comment\n").is_err());
    assert!(parse_underrun("LBM Handles:\n").is_err());
    assert!(parse_underrun("-1\n").is_err());
}

#[test]
fn dlists_fixture_channels() {
    let d = parse_dlists(&fixture("debugfs-2026-11-28-hvs_dlists.txt")).unwrap();
    assert_eq!(d.channels.len(), 3);
    let c0 = &d.channels[0];
    assert_eq!((c0.index, c0.enabled), (0, true));
    // dlist: 63 .. 71, nine words, the last one 0x80000000 (end).
    assert_eq!(c0.words(), 9);
    assert_eq!(c0.entries.first(), Some(&(63, 0x4800_d804)));
    assert_eq!(c0.entries.last(), Some(&(71, 0x8000_0000)));
    assert!(c0.ends_with_end_word());
    assert!(!d.channels[1].enabled && !d.channels[2].enabled);
    assert_eq!(d.channels[1].words(), 0);
    assert_eq!(d.total_words(), 9);
    assert_eq!(d.per_channel_text(), "0:9,1:off,2:off");
}

#[test]
fn dlists_agree_with_dlist_allocs_fixture() {
    // hvs_dlist_allocs lists the node [0000003f + 00000009]: start 63, 9
    // words, the same block hvs_dlists prints for channel 0.
    let allocs = fixture("debugfs-2026-11-28-hvs_dlist_allocs.txt");
    assert!(allocs.contains("node [0000003f + 00000009]"));
    let d = parse_dlists(&fixture("debugfs-2026-11-28-hvs_dlists.txt")).unwrap();
    let c0 = &d.channels[0];
    assert_eq!(c0.entries[0].0, 0x3f);
    assert_eq!(c0.words(), 0x9);
}

#[test]
fn dlists_synthetic_and_errors() {
    let text = "HVS chan 0 disabled\nHVS chan 1:\ndlist: 32: 0x00000001\ndlist: 33: 0x80000000\n\
                HVS chan 2:\ndlist: 40: 0x12345678\nsome unknown line\n";
    let d = parse_dlists(text).unwrap();
    assert_eq!(d.per_channel_text(), "0:off,1:2,2:1");
    assert_eq!(d.total_words(), 3);
    assert!(d.channels[1].ends_with_end_word());
    assert!(!d.channels[2].ends_with_end_word());

    assert!(parse_dlists("").is_err());
    assert!(parse_dlists("dlist: 1: 0x0\n").is_err());
    assert!(parse_dlists("HVS chan 0:\ndlist: x: 0x0\n").is_err());
    assert!(parse_dlists("HVS chan 0:\ndlist: 1: 12\n").is_err());
    assert!(parse_dlists("HVS chan X:\n").is_err());
}

fn step(n: u32) -> Step {
    Step {
        n,
        layers: 2,
        size: (360, 640),
        src: (720, 1280),
        scale: 0.5,
        test_only: TestOnly::Ok,
        commit: Commit::Ok,
        latched: (120, 120),
        late: 0,
        missing: 0,
        underrun_before: Some(0),
        underrun_after: Some(0),
        dlist_words: Some(21),
        dlist: "0:21,1:off,2:off".into(),
    }
}

#[test]
fn step_line_round_trips() {
    let s = step(4);
    let line = s.line();
    assert_eq!(
        line,
        "STEP n=4 layers=2 size=360x640 src=720x1280 scale=0.5000 test_only=ok commit=ok \
         latched=120/120 late=0 missing=0 underrun_before=0 underrun_after=0 dlist_words=21 \
         dlist=0:21,1:off,2:off"
    );
    assert_eq!(parse_step(&line).unwrap(), s);

    let mut other = step(5);
    other.test_only = TestOnly::Err(22);
    other.commit = Commit::NotRun;
    other.latched = (0, 0);
    other.underrun_before = None;
    other.underrun_after = None;
    other.dlist_words = None;
    other.dlist = "na".into();
    let line = other.line();
    assert!(line.contains("test_only=err:22 commit=not-run"));
    assert!(line.contains("underrun_before=na underrun_after=na dlist_words=na dlist=na"));
    assert_eq!(parse_step(&line).unwrap(), other);

    let mut skipped = step(6);
    skipped.test_only = TestOnly::Skipped("alloc:12".into());
    skipped.commit = Commit::Err(16);
    assert_eq!(parse_step(&skipped.line()).unwrap(), skipped);
}

#[test]
fn step_parse_rejects_bad_lines() {
    let good = step(1).line();
    assert!(parse_step("SUMMARY steps=1").is_err());
    assert!(parse_step(&good.replace("late=0", "late=x")).is_err());
    assert!(parse_step(&good.replace("latched=120/120", "latched=120")).is_err());
    assert!(parse_step(&good.replace("test_only=ok", "test_only=maybe")).is_err());
    assert!(parse_step(&good.replace("commit=ok", "commit=perhaps")).is_err());
    assert!(parse_step(&good.replace(" missing=0", "")).is_err());
    assert!(parse_step(&good.replace("size=360x640", "size=360")).is_err());
    assert!(parse_step(&format!("{good} stray")).is_err());
}

#[test]
fn dropped_needs_test_only_ok() {
    assert!(!step(1).dropped());
    let mut underrun = step(2);
    underrun.underrun_after = Some(3);
    assert!(underrun.dropped());
    let mut late = step(3);
    late.late = 1;
    assert!(late.dropped());
    let mut missing = step(4);
    missing.missing = 2;
    assert!(missing.dropped());
    let mut refused = step(5);
    refused.commit = Commit::Err(22);
    assert!(refused.dropped());
    // A rejected step never counts, whatever its counters say.
    let mut rejected = step(6);
    rejected.test_only = TestOnly::Err(22);
    rejected.underrun_after = Some(9);
    assert!(!rejected.dropped());
    // No debugfs: underrun cannot show a drop.
    let mut na = step(7);
    na.underrun_before = None;
    na.underrun_after = Some(9);
    assert!(!na.dropped());
}

#[test]
fn summary_counts_and_first_drop() {
    let mut steps = vec![step(1), step(2), step(3), step(4)];
    steps[1].underrun_after = Some(2);
    steps[1].underrun_before = Some(0);
    steps[2].underrun_before = Some(2);
    steps[2].underrun_after = Some(2);
    steps[2].test_only = TestOnly::Err(28);
    steps[3].underrun_before = Some(2);
    steps[3].underrun_after = Some(5);
    steps[3].test_only = TestOnly::Skipped("alloc:12".into());
    assert_eq!(
        summary_line(&steps),
        "SUMMARY steps=4 test_only_ok=2 test_only_err=1 skipped=1 drop_steps=1 first_drop=2 \
         first_reject=3 underrun_delta=5"
    );
    assert!(summary_line(&[step(1)]).contains("first_drop=none first_reject=none underrun_delta=0"));
    let mut na = step(1);
    na.underrun_before = None;
    na.underrun_after = None;
    assert!(summary_line(&[na]).ends_with("underrun_delta=na"));
}
