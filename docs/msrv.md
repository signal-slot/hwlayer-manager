# Minimum Rust version of the workspace

Date of measurement: 2026-11-28. Commit measured: `4812c86` (tip of the integration branch when the item started).

This is a **host measurement** (observed). Host: Linux x86_64 (Gentoo, kernel 7.2.13). It says nothing about a board, an Arm target at run time, or hardware. The `thumbv7em-none-eabihf` rows only show that the code compiles for that target; nothing was run on it.

## What is declared

`Cargo.toml` sets `rust-version = "1.82"` for the workspace, and every crate inherits it (read from source, 2026-11-28). `docs/ci.md` records that CI runs only the latest stable and does not test this floor. So the field exists but is untested.

## Result per toolchain

Every command ran with `--locked` on the committed `Cargo.lock`, one separate target directory per toolchain.

| Toolchain | `cargo build --workspace --locked` | `cargo test --workspace --locked` | `cargo build --locked -p composition-core --target thumbv7em-none-eabihf` | `cargo build --locked -p composition-ffi-staticlib --no-default-features --features panic-handler --target thumbv7em-none-eabihf --release` |
|---|---|---|---|---|
| 1.79.0 | fail | fail | fail | fail |
| 1.88.0 | pass | pass | pass | pass |
| stable (1.99.0) | pass | pass | pass | pass |

First error for 1.79.0, identical shape in all four commands: Cargo stops before compiling anything with `rustc 1.79.0 is not supported by the following packages`. The packages named are `composition-core`, `-fake`, `-ffi`, `-ffi-staticlib`, `-drm`, `-drm-harness`, `-twoplane` and `vm-init`, each "requires rustc 1.82". The two thumbv7em commands name only `composition-core`, `composition-ffi` and `composition-ffi-staticlib`. This is the declared `rust-version` check. It is not a compiler error, so it does not show which language or library feature needs a newer compiler.

### Diagnostic: 1.79.0 with `--ignore-rust-version` (observed, 2026-11-28)

To see real compiler errors, `cargo +1.79 build --locked --ignore-rust-version -p <crate>` was run per crate (only these four crates, not the whole workspace, and no test or thumbv7em runs):

| Crate | Result | First error |
|---|---|---|
| composition-core | pass | none |
| composition-drm | pass | none |
| composition-fake | fail | E0599: `Option::is_none_or` (`crates/composition-fake/src/lib.rs:186`) |
| composition-ffi | fail | E0658: `&raw mut` raw-address-of syntax (`crates/composition-ffi/src/lib.rs:655`, `:662`) |

Read from the Rust release notes (not measured here): both `Option::is_none_or` and `&raw` syntax were stabilised in Rust 1.82. This matches the declared 1.82 for those two crates. composition-core and composition-drm compile on 1.79 as far as this build shows. Not tried on 1.79: composition-ffi-staticlib, composition-twoplane, drm-harness, vm-init, and the tests.

The other toolchains in `rustup toolchain list` (1.95.0, 1.97.0, 1.98.0, nightly, esp, ferrocene) were not measured; they are above the passing 1.88.

## Resulting MSRV

The narrowest range shown: the workspace **builds and tests on 1.88.0 and later, and is rejected on 1.79.0**. The true floor lies in 1.80 to 1.88. The declared 1.82 is consistent with the diagnostic below (two crates use features stabilised in 1.82) but no 1.80 to 1.87 toolchain was run, so it is not confirmed.

Not done:

- **Bisect.** No toolchain between 1.79 and 1.88 is installed. `rustup toolchain install 1.82 --profile minimal --target thumbv7em-none-eabihf` was refused on this node (not approved), so 1.80 to 1.87 were not tried.
- **Full compiler check below 1.82.** The `--ignore-rust-version` diagnostic covered four crates only; the rest and the tests were not run.
- **Dependency floors.** Locked dependencies (for example `syn`, `rustix`, `libc`) were not checked for their own `rust-version`.

To finish the bisect, install 1.80 to 1.87 and run the same four commands. Add `--ignore-rust-version` to the 1.79 and 1.82 runs to see compiler errors.

## Proposal (not applied)

- Keep `rust-version = "1.82"` in `Cargo.toml` until the bisect shows otherwise. Raise or lower it to the measured floor.
- Add an `msrv` job to `.github/workflows/ci.yml`, next to `checks`:
  - install the toolchain named by the `rust-version` field, with `--target thumbv7em-none-eabihf`;
  - run `cargo build --workspace --locked`, `cargo test --workspace --locked`, and the two thumbv7em builds above;
  - skip fmt and clippy there, since their output depends on the toolchain.
- Update the sentence in `docs/ci.md` that says CI does not test the minimum Rust version when the job lands.

None of these files were changed by this measurement.
