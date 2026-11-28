# check-armv7r-build

Host build check: the `no_std` crates build for `armv7r-none-eabihf`, the
Rust target of the Cortex-R5 (ARMv7-R) MCU R5F of the AM62P. POSIX sh.

This is a host build check, **not a board result**. Nothing is linked, no C is
compiled and nothing runs on a board. The link of the static library into C
is a different script, `scripts/check-ffi-r5f-link.sh`.

CI does not run this script. `docs/ci.md` does not mention it, and
`.github/workflows/ci.yml` installs the `armv7r-none-eabihf` target only for
`scripts/check-arm-link.sh`. Read both again before relying on this.

## Prerequisites

- `rustup` and `cargo` on `PATH`.
- The Rust target `armv7r-none-eabihf` installed
  (`rustup target add armv7r-none-eabihf`). The script never installs
  anything. It looks for the exact name in `rustup target list --installed`,
  so `armv7r-none-eabi` does not count.

## Usage

    scripts/check-armv7r-build.sh [--strict]

| Option     | Effect |
|------------|--------|
| (none)     | A missing target prints `ARMV7R SKIP: target not installed` and exits 0. |
| `--strict` | A missing target prints `ARMV7R FAIL: target not installed (...; --strict)` to stderr and exits 1. |

It takes at most one argument, and `--strict` is the only one it accepts.
Anything else prints a usage line to stderr and exits 2.

## What it builds

From the repository root, in this order, stopping at the first failure:

    cargo build -p composition-core --target armv7r-none-eabihf
    cargo build -p composition-twoplane --target armv7r-none-eabihf

`composition-twoplane` is `backends/twoplane`. It is `#![no_std]` and its
`lib.rs` is empty, because the backend itself is C, but it is built too.
On success it prints
`ARMV7R PASS: composition-core composition-twoplane build for armv7r-none-eabihf`.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Both packages built, or the target is missing without `--strict` (skip). |
| 1 | A build failed (`ARMV7R FAIL: <pkg> does not build ...`), or the target is missing under `--strict`. |
| 2 | Bad usage. |

A skip exits 0, so a plain run on a machine without the target proves
nothing. Use `--strict` where the result must count.

## Test

    sh scripts/check-armv7r-build.test.sh

Runs the script with stub `cargo` and `rustup` on `PATH`: skip (including a
similar target name), strict skip, pass, build failure and bad usage. Nothing
is built or installed.
