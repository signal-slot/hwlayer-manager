//! Compiles `tests/c/event_order.c` against the header of `composition-ffi`
//! and the static library, and runs it: adversarial event ordering and
//! malformed arguments as a C caller sees them.
//!
//! `tests/c/event_order.c` runs every case that is expected to pass when
//! started without arguments, and one named case when given its name. Each
//! `#[ignore]`d test below runs one case that records a bug of the engine.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

/// Builds the static library and returns its path, as Cargo reports it.
///
/// Cargo builds only the rlib of this crate for its tests, so the static
/// library is built by a nested cargo, in the same profile. Its path comes
/// from Cargo's JSON messages: the layout of the target and build
/// directories is not a stable interface (Cargo 1.100 changed it).
fn build_static_library(manifest: &Path) -> PathBuf {
    let mut build = Command::new(env!("CARGO"));
    build
        .args(["build", "--lib", "-p", "composition-ffi-staticlib"])
        .arg("--message-format=json-render-diagnostics")
        .current_dir(manifest);
    if !cfg!(debug_assertions) {
        build.arg("--release");
    }
    let output = build.output().expect("cannot run cargo");
    assert!(
        output.status.success(),
        "cargo build of the static library failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
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

/// Builds the C program (once per call) and runs it with `args`.
fn run_event_order(args: &[&str]) -> Output {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let library = build_static_library(&manifest);

    let program = Path::new(env!("CARGO_TARGET_TMPDIR")).join("composition_ffi_event_order");
    let compiler = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let output = Command::new(&compiler)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-pedantic"])
        .arg("-I")
        .arg(manifest.join("../composition-ffi/include"))
        .arg(manifest.join("tests/c/event_order.c"))
        .arg(&library)
        // What the Rust standard library needs on Linux.
        .args(["-lpthread", "-ldl", "-lm"])
        .arg("-o")
        .arg(&program)
        .output()
        .unwrap_or_else(|e| panic!("cannot run the C compiler `{compiler}`: {e}"));
    assert!(
        output.status.success(),
        "C compilation failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Command::new(&program).args(args).output().unwrap()
}

fn assert_passes(args: &[&str]) {
    let run = run_event_order(args);
    assert!(
        run.status.success(),
        "C program failed ({:?}):\n{}{}",
        run.status,
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn c_caller_survives_adversarial_event_order() {
    assert_passes(&[]);
}
