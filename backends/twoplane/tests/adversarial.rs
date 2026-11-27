//! Adversarial event-ordering host tests of the two-plane backend
//! (`ce_twoplane.c`) behind a fake driver.
//!
//! Where `twoplane.rs` runs `tests/c/twoplane_test.c` (the honest
//! orderings), this test compiles and runs `tests/c/adversarial_test.c`:
//! latch and release orderings driven out of their expected sequence — a
//! vsync with nothing pending, duplicated latch reports, missing latches
//! (including the engine's latch timeout), a pending submission replaced
//! before its latch and latched anyway, out-of-band commits the engine was
//! never told about, a device loss injected through the backend's
//! `poll_event` followed by a stale latch, drivers that refuse writes the
//! backend does not check, and buffer rotations over many frames with
//! idle vsyncs, early polls and replacements thrown in. Each scenario
//! asserts the documented behaviour: no buffer released twice, no release
//! before the latch or drop it follows, no use after release, and
//! inconsistencies reported as a negative status from `ce_poll_event`,
//! never as a crash.
//!
//! Every scenario runs twice in Rust: inside `all_scenarios` (the C
//! program run with no arguments) and as its own test that runs the
//! single scenario by name, so a failing ordering is visible on its own.
//! A scenario whose failure is a recorded engine or backend bug is marked
//! `known_bug` in the C table (so `all_scenarios` stays green) and its
//! single-scenario test carries `#[ignore]` with a comment naming the bug;
//! running it with `cargo test -- --ignored` then shows the bug still
//! present, and dropping `#[ignore]` after a fix is what keeps the fix
//! honest.
//!
//! Bugs found here and since fixed, kept as regression tests (see the scenario comments in `tests/c/adversarial_test.c`):
//! - `flip_error_swallowed`: `ce_twoplane.c` discards `hw->safety_flip`'s
//!   return value; a refused flip is recorded as done, the frame reports
//!   PRESENTED, and the other safety buffer becomes unreachable.
//! - `enable_error_swallowed`: same for `hw->enable_app_plane`; a refused
//!   enable is recorded as on, the frame reports PRESENTED, and no later
//!   frame retries the enable.

use std::path::PathBuf;
use std::process::{Command, Output};

const FLAGS: [&str; 5] = ["-std=c11", "-Wall", "-Wextra", "-Werror", "-pedantic"];

fn backend_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn engine_include() -> PathBuf {
    backend_dir().join("../../crates/composition-ffi/include")
}

/// Where this test writes the programs and objects it builds.
fn scratch_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
}

/// The JSON strings of a list that starts right after its `[`.
fn json_strings(list: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut chars = list.chars();
    while let Some(c) = chars.next() {
        match c {
            ']' => break,
            '"' => {
                let mut string = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => string.push('\n'),
                            Some('t') => string.push('\t'),
                            Some('r') => string.push('\r'),
                            Some('u') => {
                                let hex: String = chars.by_ref().take(4).collect();
                                let code = u32::from_str_radix(&hex, 16).unwrap();
                                string.push(char::from_u32(code).unwrap());
                            }
                            Some(other) => string.push(other),
                            None => break,
                        },
                        _ => string.push(c),
                    }
                }
                strings.push(string);
            }
            _ => {}
        }
    }
    strings
}

/// Builds the static library of the engine and returns its path, as Cargo
/// reports it.
///
/// Cargo builds only the rlibs of the workspace for tests, so the static
/// library is built by a nested cargo, in the same profile. Its path comes
/// from Cargo's JSON messages: the layout of the target and build
/// directories is not a stable interface (Cargo 1.100 changed it).
fn build_static_library() -> PathBuf {
    let mut build = Command::new(env!("CARGO"));
    build
        .args(["build", "--lib", "-p", "composition-ffi-staticlib"])
        .arg("--message-format=json-render-diagnostics")
        .current_dir(backend_dir());
    if !cfg!(debug_assertions) {
        build.arg("--release");
    }
    let output = run(&mut build, "cargo build of the static library");
    let messages = String::from_utf8_lossy(&output.stdout);
    let library = messages
        .lines()
        .filter(|line| line.contains("\"reason\":\"compiler-artifact\""))
        .filter_map(|line| line.split_once("\"filenames\":[").map(|(_, rest)| rest))
        .flat_map(json_strings)
        .map(PathBuf::from)
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "libcomposition_engine.a")
        });
    let library =
        library.unwrap_or_else(|| panic!("cargo reported no libcomposition_engine.a:\n{messages}"));
    assert!(
        library.exists(),
        "static library not found at {}",
        library.display()
    );
    library
}

fn compiler() -> String {
    std::env::var("CC").unwrap_or_else(|_| "cc".into())
}

fn run(command: &mut Command, what: &str) -> Output {
    let output = command
        .output()
        .unwrap_or_else(|e| panic!("cannot run {what}: {e}"));
    assert!(
        output.status.success(),
        "{what} failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Compiles `ce_twoplane.c` with `tests/c/adversarial_test.c` and the
/// static library of the engine. `tag` keeps the output file distinct per
/// test, so the tests can run in parallel.
fn build_program(tag: &str) -> PathBuf {
    let library = build_static_library();
    let program = scratch_dir().join(format!("composition_twoplane_adversarial_{tag}"));
    run(
        Command::new(compiler())
            .args(FLAGS)
            .arg("-I")
            .arg(engine_include())
            .arg("-I")
            .arg(backend_dir())
            .arg(backend_dir().join("ce_twoplane.c"))
            .arg(backend_dir().join("tests/c/adversarial_test.c"))
            .arg(&library)
            // What the Rust standard library needs on Linux.
            .args(["-lpthread", "-ldl", "-lm"])
            .arg("-o")
            .arg(&program),
        "the C compiler",
    );
    program
}

/// Runs the program with `args` and asserts it exits 0, printing its
/// output on failure.
fn run_scenario(program: &PathBuf, args: &[&str]) {
    let output = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("cannot run the C test: {e}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the C test {} failed ({}):\n{stdout}{stderr}",
        if args.is_empty() {
            String::new()
        } else {
            format!("scenario {}", args.join(" "))
        },
        output.status,
    );
    print!("{stdout}");
}

/// All scenarios that are not known bugs: the C program run with no
/// arguments.
#[test]
fn all_scenarios() {
    let program = build_program("all");
    run_scenario(&program, &[]);
}

#[test]
fn vsync_idle() {
    let program = build_program("vsync_idle");
    run_scenario(&program, &["vsync_idle"]);
}

#[test]
fn duplicate_latch() {
    let program = build_program("duplicate_latch");
    run_scenario(&program, &["duplicate_latch"]);
}

#[test]
fn latch_between_presents() {
    let program = build_program("latch_between_presents");
    run_scenario(&program, &["latch_between_presents"]);
}

#[test]
fn latch_during_plan_check() {
    let program = build_program("latch_during_plan_check");
    run_scenario(&program, &["latch_during_plan_check"]);
}

#[test]
fn replace_pending() {
    let program = build_program("replace_pending");
    run_scenario(&program, &["replace_pending"]);
}

#[test]
fn replace_latch_during_writes() {
    let program = build_program("replace_latch_during_writes");
    run_scenario(&program, &["replace_latch_during_writes"]);
}

#[test]
fn own_latch_deferred_isr() {
    let program = build_program("own_latch_deferred_isr");
    run_scenario(&program, &["own_latch_deferred_isr"]);
}

#[test]
fn latch_timeout_cycle() {
    let program = build_program("latch_timeout_cycle");
    run_scenario(&program, &["latch_timeout_cycle"]);
}

#[test]
fn poll_during_pending() {
    let program = build_program("poll_during_pending");
    run_scenario(&program, &["poll_during_pending"]);
}

#[test]
fn unknown_submission_latch() {
    let program = build_program("unknown_submission_latch");
    run_scenario(&program, &["unknown_submission_latch"]);
}

#[test]
fn latch_after_device_lost() {
    let program = build_program("latch_after_device_lost");
    run_scenario(&program, &["latch_after_device_lost"]);
}

#[test]
fn event_queue_full() {
    let program = build_program("event_queue_full");
    run_scenario(&program, &["event_queue_full"]);
}

#[test]
fn capabilities_changed() {
    let program = build_program("capabilities_changed");
    run_scenario(&program, &["capabilities_changed"]);
}

/// Bug: `ce_twoplane.c` discards `hw->safety_flip`'s return value, so a
/// driver that refuses the flip is accepted, the frame is reported
/// PRESENTED, and the second safety buffer becomes unreachable. The
/// scenario asserts that a refused write is reported (`ce_present` fails,
/// nothing is consumed). Fixed: the backend returns the refusal; this is
/// the regression test.
#[test]
fn flip_error_swallowed() {
    let program = build_program("flip_error_swallowed");
    run_scenario(&program, &["flip_error_swallowed"]);
}

/// Bug: `ce_twoplane.c` discards `hw->enable_app_plane`'s return value, so
/// a refused enable is recorded as on, the frame is reported PRESENTED,
/// and the application plane is never re-enabled. The scenario asserts
/// that the refusal is reported. Fixed: the backend returns the refusal;
/// this is the regression test.
#[test]
fn enable_error_swallowed() {
    let program = build_program("enable_error_swallowed");
    run_scenario(&program, &["enable_error_swallowed"]);
}

#[test]
fn rotation_64() {
    let program = build_program("rotation_64");
    run_scenario(&program, &["rotation_64"]);
}

#[test]
fn wrap_rotation() {
    let program = build_program("wrap_rotation");
    run_scenario(&program, &["wrap_rotation"]);
}
