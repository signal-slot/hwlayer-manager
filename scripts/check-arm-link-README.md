# check-arm-link

`scripts/check-arm-link.sh [OUT_DIR]` compiles the twoplane C backend
(`backends/twoplane/ce_twoplane.c`) for Cortex-M7 and Cortex-R5, links each
object with the engine static library into a minimal freestanding image, and
checks the images with `nm`.

**This is a host link check, not a board result.** Nothing is run, on a
device or in an emulator; it says only that the objects compile and link and
that the symbols resolve.

## Prerequisites

- `cargo` and `rustup` (the script runs `rustup target add` for each target,
  ignoring failure, and builds with `--locked`). Rust targets used:
  `thumbv7em-none-eabihf` (Cortex-M7) and `armv7r-none-eabihf` (Cortex-R5).
- One C toolchain:
  - `arm-none-eabi-gcc` (with `arm-none-eabi-nm`), when it is on `PATH`; or
  - `clang` with a lld: `ld.lld` on `PATH`, else `rust-lld` from the Rust
    sysroot (`rustc --print sysroot`), plus `llvm-nm` or `nm`.
- Inputs in the repository: `backends/twoplane/arm/{start.c,main.c,cortex-m7.ld,cortex-r5.ld}`
  and `crates/composition-ffi/include`.

## Usage

```
scripts/check-arm-link.sh [OUT_DIR]
```

- `OUT_DIR` (optional, first argument): where per-target output goes. Default
  `target/arm-link` under the repository root. Each target gets a
  subdirectory (`cortex-m7`, `cortex-r5`) holding `ce_twoplane.o`, `start.o`,
  `main.o`, `image.elf`, `image.map` and `image.nm`.
- Environment `CC_KIND`: `gcc` or `clang` forces the toolchain. Unset, the
  script picks `gcc` if `arm-none-eabi-gcc` is on `PATH`, else `clang`.
  `NM` overrides the `nm` tool in the clang case.

The script parses no other options.

## What it links and checks

Per target it:

1. builds `composition-ffi-staticlib` (`--no-default-features --features
   panic-handler`, release) for the Rust target;
2. compiles `ce_twoplane.c` with `-Wall -Wextra -Werror` (`-ffreestanding`;
   the clang build also uses `-nostdinc`), and `start.c` / `main.c` without
   loop-to-memset rewriting;
3. prints the undefined symbols of the backend object;
4. links `start.o`, `main.o`, `ce_twoplane.o` and the library with
   `-nostdlib`, `--gc-sections` and the target's linker script (gcc adds
   `-lgcc`), writing a map file;
5. checks the image:
   - if the object calls `__aeabi_memclr4`, it must be defined in the image
     (`T`, `t`, `W` or `w`) and the map lines naming it are shown; otherwise
     every non-`ce_` symbol the object calls (for example `memset` from GCC)
     must be defined in the image;
   - the image must have no undefined symbol.

## Exit codes

- `0`: both targets pass.
- `1`: a check failed (a required symbol is missing from an image, or an
  image has undefined symbols); the script still runs both targets. A compile,
  link or cargo failure stops it at once with a non-zero status (`set -e`).
- `2`: toolchain problem: `CC_KIND` is neither `gcc` nor `clang`, or the
  clang case finds no `ld.lld` or `rust-lld`.

## Test

`bash scripts/check-arm-link.test.sh` runs the script from a copy in a fake
repository with stub compilers, linkers, `nm`, `cargo` and `rustup` on a
restricted `PATH`, so no ARM toolchain is needed and nothing is built. It
covers the gcc and clang paths, the `__aeabi_memclr4` and `memset` cases,
weak definitions, undefined symbols, compile failure, `CC_KIND` validation,
a missing linker (exit 2) and the Rust-sysroot `rust-lld` fallback. It prints
`pass`/`FAIL` per case and exits non-zero if any failed.
