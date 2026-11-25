//! Compiles the two-plane backend (`ce_twoplane.c`) with the system C
//! compiler: once with `tests/c/twoplane_test.c`, a fake driver and the
//! static library of the engine, and runs it; once freestanding, without a C
//! library, to show that it needs nothing a bare-metal firmware lacks.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FLAGS: [&str; 5] = ["-std=c11", "-Wall", "-Wextra", "-Werror", "-pedantic"];

fn backend_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn engine_include() -> PathBuf {
    backend_dir().join("../../crates/composition-ffi/include")
}

/// target/<profile>, from target/<profile>/deps/<this test>.
fn profile_dir() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .and_then(|deps| deps.parent())
        .unwrap()
        .to_path_buf()
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

#[test]
fn backend_behind_a_fake_driver() {
    let profile = profile_dir();
    // Cargo builds only the rlibs of the workspace for tests: build the
    // static library itself, into the same target directory and profile.
    let mut build = Command::new(env!("CARGO"));
    build
        .args(["build", "--lib", "-p", "composition-ffi-staticlib"])
        .current_dir(backend_dir());
    if profile.file_name().is_some_and(|name| name == "release") {
        build.arg("--release");
    }
    run(&mut build, "cargo build of the static library");
    let library = profile.join("libcomposition_engine.a");
    assert!(
        library.exists(),
        "static library not found at {}",
        library.display()
    );

    let program = profile.join("composition_twoplane_test");
    run(
        Command::new(compiler())
            .args(FLAGS)
            .arg("-I")
            .arg(engine_include())
            .arg("-I")
            .arg(backend_dir())
            .arg(backend_dir().join("ce_twoplane.c"))
            .arg(backend_dir().join("tests/c/twoplane_test.c"))
            .arg(&library)
            // What the Rust standard library needs on Linux.
            .args(["-lpthread", "-ldl", "-lm"])
            .arg("-o")
            .arg(&program),
        "the C compiler",
    );
    let output = run(&mut Command::new(&program), "the C test");
    print!("{}", String::from_utf8_lossy(&output.stdout));
}

/// The compiler's own headers (stddef.h, stdbool.h, stdint.h), which
/// `-nostdinc` drops along with the C library's.
fn compiler_include_dir() -> PathBuf {
    let ask = |arg: &str| {
        let output = run(Command::new(compiler()).arg(arg), "the C compiler");
        PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
    };
    // GCC answers this; Clang may answer with the bare name.
    let dir = ask("-print-file-name=include");
    if dir.is_absolute() && dir.join("stddef.h").exists() {
        return dir;
    }
    let dir = ask("-print-resource-dir").join("include");
    assert!(
        dir.join("stddef.h").exists(),
        "cannot find the C compiler's own headers"
    );
    dir
}

#[test]
fn backend_compiles_freestanding() {
    let sources = backend_dir();
    // What the sources include themselves: the three headers a freestanding
    // C implementation has, and the engine's header.
    let allowed = [
        "<stdbool.h>",
        "<stddef.h>",
        "<stdint.h>",
        "<composition_engine.h>",
        "\"ce_twoplane.h\"",
    ];
    let engine_header = engine_include().join("composition_engine.h");
    for file in [
        sources.join("ce_twoplane.h"),
        sources.join("ce_twoplane.c"),
        engine_header,
    ] {
        let text = std::fs::read_to_string(&file).unwrap();
        for line in text.lines() {
            if let Some(name) = line.trim().strip_prefix("#include") {
                assert!(
                    allowed.contains(&name.trim()),
                    "{} includes {}",
                    file.display(),
                    name.trim()
                );
            }
        }
    }

    let system = compiler_include_dir();
    let freestanding = |command: &mut Command| {
        command
            .args(FLAGS)
            .args(["-ffreestanding", "-nostdinc", "-O2"])
            .arg("-isystem")
            .arg(&system)
            .arg("-I")
            .arg(engine_include())
            .arg("-I")
            .arg(&sources)
            .arg("-c")
            .arg(sources.join("ce_twoplane.c"));
    };

    // Every header the compile reads is the backend's, the engine's, or the
    // compiler's own: none of a C library.
    let mut deps = Command::new(compiler());
    freestanding(&mut deps);
    let output = run(deps.arg("-M"), "the C compiler (-M)");
    let roots: Vec<PathBuf> = [&sources, &engine_include(), &system]
        .iter()
        .map(|dir| dir.canonicalize().unwrap())
        .collect();
    let rule = String::from_utf8_lossy(&output.stdout).replace("\\\n", " ");
    let mut system_headers = BTreeSet::new();
    for dep in rule.split_whitespace().skip(1) {
        let path = Path::new(dep).canonicalize().unwrap();
        let root = roots.iter().find(|root| path.starts_with(root));
        assert!(root.is_some(), "the backend reads {}", path.display());
        if path.starts_with(&roots[2]) {
            system_headers.insert(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    for header in ["stdbool.h", "stddef.h", "stdint.h"] {
        assert!(system_headers.contains(header), "{header} not used");
    }

    let object = profile_dir().join("ce_twoplane_freestanding.o");
    let mut compile = Command::new(compiler());
    freestanding(&mut compile);
    run(compile.arg("-o").arg(&object), "the freestanding C compile");

    // The backend reaches the driver and the system through its tables only,
    // so it needs no symbol from elsewhere, except those the compiler may
    // call for structure copies, which a freestanding program provides.
    let nm = std::env::var("NM").unwrap_or_else(|_| "nm".into());
    let output = run(Command::new(&nm).arg("-u").arg(&object), "nm");
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let symbol = line.split_whitespace().last().unwrap_or("");
        assert!(
            ["memcpy", "memset", "memmove", "memcmp"].contains(&symbol),
            "the freestanding backend needs `{symbol}`"
        );
    }
}
