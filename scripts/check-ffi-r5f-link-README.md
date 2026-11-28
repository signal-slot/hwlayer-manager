# check-ffi-r5f-link

Host link check for the R5F / ARMv7-R target (M3-05 prep). POSIX sh. It is a
**host check, not a board result**: nothing is run, there is no Zephyr, no board
support and no real start-up code, and no board is touched. It does not close
M3-05 (the link in a real firmware build).

## What it checks

1. Builds `composition-ffi-staticlib` for `armv7r-none-eabihf` with
   `cargo build --locked -p composition-ffi-staticlib --no-default-features
   --features panic-handler --release`, producing `libcomposition_engine.a`
   under `target/armv7r-none-eabihf/release/`.
2. Writes a tiny C `main` (calls `ce_engine_init`, `ce_set_latch_timeout`,
   `ce_advance_time`, `ce_begin_write`, `ce_buffer_state`; includes
   `crates/composition-ffi/include/composition_engine.h`) and a linker script
   with a 1 MiB RAM region, then links it with `-nostdlib`/`-nostartfiles`.
3. Fails if the link leaves undefined symbols (they are listed), if `nm` of the
   image shows any `U` entry, or if `ce_engine_init` is not in the image.

Compiler: `arm-none-eabi-gcc` (`-mcpu=cortex-r5 -mfloat-abi=hard
-mfpu=vfpv3-d16`, linked with `-lgcc`) when present; otherwise
`clang --target=armv7r-none-eabihf` with `ld.lld` or the toolchain's
`rust-lld`. Scratch files go under `$TMPDIR` (default `/tmp`) and are removed on
exit.

## Prerequisites

- `cargo` and the Rust target `armv7r-none-eabihf`. If the target is missing the
  script tries `rustup target add`; if that fails it reports SKIP.
- `arm-none-eabi-gcc`, or `clang` plus `ld.lld` or `rust-lld`.

Missing prerequisites give SKIP, not FAIL.

## Usage

    scripts/check-ffi-r5f-link.sh

No flags or arguments: the script parses none, and the only environment input
is `TMPDIR`. It prints `compiler: <gcc|clang>` and `undefined symbols: none`
on success, and always a final line:

- `PASS`
- `FAIL: <reason>`
- `SKIP: <reason>` (no cargo, target not installed, no compiler, no linker)

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | `PASS` or `SKIP` |
| 1 | `FAIL` (cargo build failed, compile or link failed, undefined symbols, `ce_engine_init` missing) |

## Test

    sh scripts/check-ffi-r5f-link.test.sh

Runs the script and accepts a last line of `PASS` or `SKIP: <reason>` with exit
0 (anything else, or a mismatching exit code, fails). It prints `ok: <last
line>`. A SKIP on a machine without the toolchain is not evidence that the link
works.
