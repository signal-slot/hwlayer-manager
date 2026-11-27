# Writing and running a board-test Zephyr app

How to add a board test to this repository and how to run it on a board
without breaking the board for the next person. Board verification here is a
Zephyr application (or a small program) kept in the repository, as
[`CONTRIBUTING.md`](../CONTRIBUTING.md) requires. Nothing in this page is a
hardware result; results are reported as described under "Reporting a result".

## Where an app lives and what it contains

An app is a directory `tests/board/<name>/` (or `tests/zephyr/<name>/` for
apps that run in QEMU or `native_sim`). `scripts/check-zephyr-apps.sh` lints
every directory below those two parents:

```sh
sh scripts/check-zephyr-apps.sh
```

It prints `ok <dir>` or `fail <dir>: <reason>` per app and exits 1 on any
failure. Each app needs:

- `CMakeLists.txt`
- `prj.conf`
- `src/` with at least one `.c`, `.cpp`, `.cc`, `.S` or `.rs` file
- `README.md` that contains a `west build` command

The script also fails an app whose files contain an IPv4 address, the string
`/dev/tty` or the word `password`. Keep those out of sources, devicetree
overlays and the README. Optional pieces such as `Kconfig`, `boards/` overlays
and extra `.conf` files are not checked.

The README of an app should say, in this order: what the app proves, what
board it is meant for, the exact `west build` command, how to run it, and the
status. Until the app has been run on the board the status says "not run on
hardware" and every timing or scanout claim says "not measured". Use the
evidence tags `observed` / `read from source` / `inferred` with a date, as in
the other hardware docs.

An app at HEAD that shows the layout: `tests/board/imx95_m7_scanout/`
(upstream Zephyr only; a `Kconfig` option guards the part that touches the
DPU; host builds only so far).

If an app can damage state it does not own (it programs a display controller
that Linux may own, for example), make that opt-in through a Kconfig option
that defaults to off, and say in the README when it is safe to turn on.

## Environment names

Builds use three environment variables. This repository never records their
values; set them in your shell.

| Variable | Meaning |
| --- | --- |
| `ZEPHYR_WORKSPACE` | A west workspace (`<ws>/zephyr`, `<ws>/modules/...`). Upstream Zephyr only. |
| `ZEPHYR_SDK_INSTALL_DIR` | A Zephyr SDK with the `arm-zephyr-eabi` toolchain. |
| `MCUI_SHOWCASE_DIR` | Your checkout of the private showcase module that supplies the AM62P board support. |

`scripts/prepare-zephyr.sh` builds the first two at a pinned Zephyr tag and
prints the export lines; [`zephyr-workspace.md`](zephyr-workspace.md)
describes what it fetches and how to build an app from the workspace. The
i.MX95 apps use upstream Zephyr only and need no `MCUI_SHOWCASE_DIR`.

`MCUI_SHOWCASE_DIR` matters only for AM62P apps. The AM62P board support
(board definition, display driver, DSS register header) comes from a private
module that is not part of this repository. Point the variable at your own
checkout of it; never copy any of its files into this repository. If an app
cannot build without it, the app's README says so and names the variable, and
the app is not part of any CI.

A build looks like this (use the README of the app for the real board name and
options):

```sh
cd "$ZEPHYR_WORKSPACE"
west build -p -b <board> <repo>/tests/board/<name> -d <build dir outside the repo and outside /tmp>
```

Keep build directories out of the repository and out of `/tmp`, which is
memory on some machines.

## Linking the engine

An app that exercises the engine links the C ABI static library, not Rust
sources. The library is the one CI already builds for the bare-metal target
(see [`ci.md`](ci.md)):

```sh
cargo build --locked -p composition-ffi-staticlib \
  --no-default-features --features panic-handler \
  --target thumbv7em-none-eabihf --release
```

- Output: `target/thumbv7em-none-eabihf/release/libcomposition_engine.a`
  (the name is set in `crates/composition-ffi-staticlib/Cargo.toml`).
- Header: `include/composition_engine.h`, which moves in the same commit as
  `crates/composition-ffi/src/lib.rs`.
- `--no-default-features` drops `std`, so the library is `no_std`. The
  `panic-handler` feature supplies a panic handler that halts; use it when the
  final program contains no other Rust code. Without it the program must
  provide its own panic handler.
- `thumbv7em-none-eabihf` is the Cortex-M4/M7 hard-float target, which fits
  the i.MX95 M7. It needs `rustup target add thumbv7em-none-eabihf`. Other
  cores need other targets and are not covered by CI.

The app's `CMakeLists.txt` adds the include directory and links the `.a`
(for example as an imported static library). Pass the library path in with a
CMake variable or environment variable instead of writing an absolute path
into the file. At HEAD no app in `tests/board/` links the library; add the
exact CMake lines to the README of the first one that does.

## Board rules

The full rules for the i.MX95 board are in
[`imx95-board.md`](imx95-board.md); read it before you touch that board, and
do not edit it as part of an app change. In short, and for every board:

- Write only under `/tmp` on the board. Do not modify the root filesystem.
- Install no packages.
- Do not change the U-Boot environment (no `fw_setenv`, no `saveenv`).
- Never write the eMMC boot areas (`mmcblk0boot0`, `mmcblk0boot1`).
- If you stop Weston to take the display, restore it before you leave and
  confirm it is active again. If you cannot restore it, say so in the report.
  Do not stop Weston for a test that does not need the display.
- If a test boots a board into anything other than its normal boot chain, put
  the board back on that chain and confirm over ssh before you finish.
- Keep leases short: take the board, run, restore, release. Build on the host
  first and do not hold the board while you edit; other runs may need it.
- Files, commits and reports contain no board addresses, host names, users,
  serial numbers, UART or device paths, or credentials. Write `<board>` or the
  resource name. `scripts/check-no-board-secrets.sh` scans for them, and
  `scripts/check-zephyr-apps.sh` checks a subset inside the app directories.

## Reporting a result

Open an issue from the template `.github/ISSUE_TEMPLATE/board-result.md`
(label `board-result`). Fill in:

- board, carrier, image and kernel version, and the **date of the measurement**
  (YYYY-MM-DD);
- the repository commit and the path of the test under `tests/board` or
  `tools`;
- the exact command with board addresses and paths removed, and the verdict
  lines copied verbatim;
- the tag: `observed`, `read from source` or `inferred`. Anything not measured
  is written "not measured";
- what was changed on the board and how it was restored and confirmed.

A superseded result is not edited: add a new dated result.

## What does not count as a hardware result

Host unit tests, CI runs, `native_sim` or QEMU runs, vkms runs, and a Zephyr
image that only builds are **never** hardware results. They show that code
compiles or that logic holds against a fake. Label them as such ("host test",
"QEMU, not hardware") and do not use them to meet a milestone that names a
board. A result from one SoC also says nothing about another: do not claim
parity from a different board.

## Checklist before opening the PR

1. `sh scripts/check-zephyr-apps.sh` reports `ok` for your app.
2. `sh scripts/check-doc-links.sh` and `sh scripts/check-no-board-secrets.sh`
   pass.
3. The app's README has the `west build` command and an honest status.
4. The PR goes to `main` and passes the CI in [`ci.md`](ci.md), as
   [`CONTRIBUTING.md`](../CONTRIBUTING.md) describes.
