//! Compiles `tests/c/smoke.c` against the header of `composition-ffi` and the
//! static library and
//! runs it, so that the hand-written header is exercised by a real C
//! compiler.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn c_program_drives_the_engine() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // target/<profile>/deps/<this test> -> target/<profile>
    let profile = std::env::current_exe()
        .unwrap()
        .parent()
        .and_then(|deps| deps.parent())
        .unwrap()
        .to_path_buf();
    // Cargo builds only the rlib of this crate for its tests: build the static
    // library itself, into the same target directory and profile.
    let mut build = Command::new(env!("CARGO"));
    build
        .args(["build", "--lib", "-p", "composition-ffi-staticlib"])
        .current_dir(&manifest);
    if profile.file_name().is_some_and(|name| name == "release") {
        build.arg("--release");
    }
    let built = build.status().expect("cannot run cargo");
    assert!(built.success(), "cargo build of the static library failed");
    let library = profile.join("libcomposition_engine.a");
    assert!(
        library.exists(),
        "static library not found at {}",
        library.display()
    );

    let program = profile.join("composition_ffi_smoke");
    let compiler = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let output = Command::new(&compiler)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-pedantic"])
        .arg("-I")
        .arg(manifest.join("../composition-ffi/include"))
        .arg(manifest.join("tests/c/smoke.c"))
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

    let run = Command::new(&program).output().unwrap();
    assert!(
        run.status.success(),
        "C program failed:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}
