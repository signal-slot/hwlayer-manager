# Minimum Rust version of the workspace

Date of measurement: 2026-11-28. Commit measured: `3fc4ee6` (tip of the integration branch when the bisect item started). An earlier measurement the same day, at `4812c86`, covered only 1.79.0, 1.88.0 and stable; this document replaces it.

This is a **host measurement** (observed). Host: Linux x86_64. It says nothing about a board, an Arm target at run time, or hardware. The `thumbv7em-none-eabihf` rows only show that the code compiles for that target; nothing was run on it.

Toolchains were installed with `rustup toolchain install <version> --profile minimal --target thumbv7em-none-eabihf` into a separate `RUSTUP_HOME`/`CARGO_HOME`, and each toolchain used its own `CARGO_TARGET_DIR`.

## What is declared

`Cargo.toml` sets `rust-version = "1.83"` for the workspace, and every crate inherits it (read from source, 2026-11-28). Before this measurement the value was `1.82`; the bisect below shows 1.82 is wrong, so it was raised to 1.83. The `msrv` job of `.github/workflows/ci.yml` tests this floor (read from source, 2026-11-28; not yet run on a pull request).

## Result per toolchain

Every command ran with `--locked` on the committed `Cargo.lock` (observed, 2026-11-28). All four commands in the header are `cargo +<toolchain> ...` with the arguments shown. The `rust-version` field was still `1.82` during these runs.

| Toolchain | `cargo build --workspace --locked` | `cargo test --workspace --locked` | `cargo build --locked -p composition-core --target thumbv7em-none-eabihf` | `cargo build --locked -p composition-ffi-staticlib --no-default-features --features panic-handler --target thumbv7em-none-eabihf --release` |
|---|---|---|---|---|
| 1.79.0 | fail | fail | fail | fail |
| 1.80.1 | fail | fail | fail | fail |
| 1.81.0 | fail | fail | fail | fail |
| 1.82.0 | pass | **fail** | pass | pass |
| 1.83.0 | pass | pass | pass | pass |
| 1.84.1 | pass | pass | pass | pass |
| 1.85.1 | pass | pass | pass | pass |
| 1.86.0 | pass | pass | pass | pass |
| 1.87.0 | pass | pass | pass | pass |
| 1.88.0 | pass | pass | pass | pass |

First errors:

- **1.79.0, 1.80.1, 1.81.0**: in all four commands Cargo stops before compiling with `rustc <version> is not supported by the following packages` (declared `rust-version` check, "requires rustc 1.82"). This is not a compiler error; see the `--ignore-rust-version` table for the real ones.
- **1.82.0, `cargo test --workspace --locked`**: three errors `E0369: binary operation == cannot be applied to type std::process::ExitCode` in the unit tests of `composition-drm-harness` (`tools/drm-harness/src/main.rs:672`, `:673`, `:674`, `assert_eq!` on `ExitCode`). The build of the whole workspace passes on 1.82.0. The compiler note says `ExitCode` does not implement `PartialEq` there; 1.83.0 compiles the same code. The 1.83 stabilisation of `PartialEq for ExitCode` is inferred from this pass/fail boundary, not read from the release notes.

## Diagnostic: `--ignore-rust-version` below 1.82 (observed, 2026-11-28)

Run as `cargo +<toolchain> build --locked --ignore-rust-version -p <crate>` per crate, plus `cargo +<toolchain> test --workspace --locked --ignore-rust-version`. Package names are the Cargo names (`drm-harness` is the directory and binary; the package is `composition-drm-harness`, `backends/twoplane` is `composition-twoplane`).

| Crate | 1.79.0 | 1.80.1 | 1.81.0 |
|---|---|---|---|
| composition-core | fail | fail | fail |
| composition-trace | fail | fail | fail |
| composition-fake | fail | fail | fail |
| composition-drm | fail | fail | fail |
| composition-ffi | fail | fail | fail |
| composition-ffi-staticlib | fail | fail | fail |
| composition-twoplane | pass | pass | pass |
| vm-init | pass | pass | pass |
| composition-drm-harness | fail | fail | fail |
| `cargo test --workspace` | fail | fail | fail |

First real compiler error, identical for every failing crate: `advise.rs:449` in `composition-core` (`crates/composition-core/src/advise.rs:449:32`), `E0599: no method named is_none_or found for enum Option` on 1.79.0 and 1.80.1, and `E0658: use of unstable library feature 'is_none_or'` on 1.81.0. Every crate that depends on `composition-core` fails there first, so errors in later crates are masked on these toolchains. `composition-twoplane` and `vm-init` do not depend on `composition-core` and compile on all three.

Masked errors, read from source (2026-11-28, not compiled): `crates/composition-fake/src/lib.rs:186` also uses `Option::is_none_or`, and `crates/composition-ffi/src/lib.rs:655` and `:662` use `&raw mut`. The earlier measurement at `4812c86`, before `advise.rs` used `is_none_or`, reported E0599 for `composition-fake` and E0658 for `composition-ffi` on 1.79.0, and `composition-core` and `composition-drm` passing there; they fail now only because of `advise.rs:449`. `Option::is_none_or` and `&raw` syntax are stable from 1.82 (read from the Rust release notes).

## Dependency floors (read from source, 2026-11-28)

`rust-version` of every registry package in `Cargo.lock`, read from `cargo +1.88.0 metadata --format-version 1 --locked`. The 20 registry packages (of 29 packages) declare:

| rust-version | Packages |
|---|---|
| 1.71 | bytemuck_derive 1.12.1, proc-macro2 1.0.107, quote 1.0.47, syn 3.0.10, unicode-ident 1.0.27, windows-link 0.2.1, windows-sys 0.61.2 |
| 1.70 | drm 0.15.0, drm-ffi 0.9.1, drm-sys 0.8.1 |
| 1.65 | libc 0.2.191, rustix 1.1.6 |
| 1.63 | linux-raw-sys 0.9.4, linux-raw-sys 0.12.1 |
| 1.56 | bitflags 2.13.2, errno 0.3.15 |
| 1.26 | signal-hook-registry 1.4.8 |
| not declared | bytemuck 1.25.2, drm-fourcc 2.2.0, signal-hook 0.3.18 |

The highest dependency floor is **1.71** (seven packages, among them `syn`, `proc-macro2`, `quote`). It is below the workspace floor, so no dependency raises the measured floor. Packages without a declared `rust-version` were not measured separately; the 1.83 builds above pass with them.

## Resulting MSRV

**Measured floor: Rust 1.83.0** (observed, 2026-11-28, host Linux x86_64, commit `3fc4ee6`). The workspace builds, tests, and builds for `thumbv7em-none-eabihf` on 1.83.0 through 1.88.0 (six toolchains run at this commit: 1.83.0, 1.84.1, 1.85.1, 1.86.0, 1.87.0, 1.88.0). On 1.82.0 the builds pass but `cargo test --workspace` fails, so for "builds and tests" the floor is 1.83. On 1.81.0 and below the workspace does not compile because `composition-core` uses `Option::is_none_or`, stable from 1.82.

Compile-only floor: 1.82.0 (workspace build and both thumbv7em builds pass). Test floor: 1.83.0. The declared `rust-version` follows the test floor, so `rust-version` in `Cargo.toml` was changed from `1.82` to `1.83` with this measurement.

Not done:

- **Per-crate error list past `composition-core`** on 1.79 to 1.81: the first error in `advise.rs` masks later ones in `composition-fake` and `composition-ffi`, which are listed above from source only.
- **Stable 1.99.0 and newer** were not re-run with this commit; the earlier run at `4812c86` passed on stable. The project checks on stable are run in the item's acceptance step.
- **CI.** The `msrv` job in `.github/workflows/ci.yml` installs 1.83.0 and runs these builds; it was added 2026-11-28 and has not yet run on a pull request (see `docs/ci.md`).
