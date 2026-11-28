# Board test: the engine on the Verdin AM62P's MCU R5F

`tests/board/am62p_r5f/` is a Zephyr application for the MCU Cortex-R5F of a Toradex Verdin AM62P. It drives the board's display through the engine's C API (`composition_engine.h`) and the two-plane backend (`backends/twoplane/ce_twoplane.c`, compiled from this repository). It uses no Slint and none of the demo's application code.

**Status (2026-11-28): the image builds from this repository; not run on a board.** The binding of `struct ce_twoplane_hw` to the demo's DSS driver (`src/hw_am62p.c`) is written, and `prj.conf` is configured for the remoteproc route. The build was repeated from this repository on 2026-11-28 and exits 0 with `zephyr.elf` (see [What was checked (2026-11-28, image build)](#what-was-checked-2026-11-28-image-build)). Nothing has run on a board. The scene sequence, the backend and the engine run on the host behind a fake panel. See [What was checked](#what-was-checked-2026-11-27).

## Layout

| File | What it is |
| --- | --- |
| `src/scenes.c`, `src/scenes.h` | The test: the scene sequence and what the firmware judges by itself. Uses only `<stdbool.h>`, `<stddef.h>`, `<stdint.h>`, the engine and the backend. The system and the driver come through `struct scenes_env`. |
| `src/main.c` | Zephyr: the port (`irq_lock`, `k_sem_give` at every vsync, `device_is_ready`), the display device (`DT_CHOSEN(zephyr_display)`), the two application buffers, `printk` as the console. After the sequence it idles with the last scene on the panel. |
| `src/hw_am62p.c` | `struct ce_twoplane_hw` bound to the demo module's driver (`include/mcui/am62p_dss.h`). Written; see the status above. |
| `CMakeLists.txt` | Builds the engine with Cargo and compiles the backend from `backends/twoplane`. |
| `prj.conf` | Configuration for the remoteproc route. |
| `host/fake_panel.c` | The same sequence on the host, behind a fake driver and a fake panel (see below). |

### The engine: why the static library

The application links `libcomposition_engine.a` of `composition-ffi-staticlib`. It is built for `armv7r-none-eabihf` without `std` and with `panic-handler`, the same build as the `thumbv7em` build in CI. The firmware has no Rust of its own, so the static library is the simpler route. It needs only stable Rust and the target's prebuilt `core` (`rustup target add armv7r-none-eabihf`): no nightly, no `rust-src`, and no Rust crate or Cargo workspace in the application. The demo builds `composition-ffi` as an rlib into its own Rust UI crate, because it already has Rust code (Slint). That reason does not apply here. The target is hard-float, so `prj.conf` sets `CONFIG_FPU=y` and `CONFIG_FP_HARDABI=y`. If the demo's R5F configuration uses the soft-float ABI, use `armv7r-none-eabi` in `CMakeLists.txt` instead.

## The scenes

The colours are flat. The safety plane uses ARGB8888, premultiplied: red, or fully transparent. The application plane uses RGB565: blue or green. The rectangle is a third of the panel's width and half its height, centred vertically, at 1/12 of the width from the left or right edge. Each scene is held for `TEST_HOLD_MS` (5000 ms by default) after its line is printed.

| # | Name | Frame | Firmware expects | The photograph should show |
| --- | --- | --- | --- | --- |
| 1 | `safety-only` | safety B, red all over | `CE_OK`, 1 commit (flip), presented | red everywhere |
| 2 | `app-under-safety` | safety A (clear, red rectangle left) over app 1 (blue) | `CE_OK`, 3 commits (buffer, enable, flip), presented, 1 release | blue, red rectangle on the left, on top |
| 3 | `app-replaced` | app 2 (green, frame number 2) | `CE_OK`, 1 commit, presented, 1 release | green, the red rectangle still on the left |
| 4 | `safety-flipped` | safety B again (clear, rectangle right) | `CE_OK`, 1 commit (flip), presented, 1 release | green, red rectangle on the right |
| 5 | `app-off` | safety B alone | `CE_OK`, 1 commit (plane off), presented, 1 release | the controller's background (black expected), red rectangle on the right |
| 6 | `refused-no-safety` | app 1 alone | `CE_ERR_BACKEND_REJECTED` (-16), 0 commits | unchanged from 5 |
| 7 | `refused-blank` | no layer | `CE_ERR_BACKEND_REJECTED`, 0 commits | unchanged from 5 |
| 8 | `refused-foreign-safety` | an ARGB8888 buffer that is not one of the driver's two | `CE_ERR_BACKEND_REJECTED`, 0 commits | unchanged from 5 |

Every scene also checks that the latched count equals the commit count after the frame was presented, and that the commit count did not change while the scene was held. A presented frame must be reported by `ce_poll_event` within 1000 ms. Each scene prints one line:

```
SCENE <n> <name>: status <s> (expected <s>), commits <c> latched <l> (+<d>, expected +<d>), presented yes|NO|-, released <r> (expected <r>): ok|FAIL
```

The sequence ends with `RESULT PASS (...)` or `RESULT FAIL (...)`. Only the photographs can judge what the panel shows.

## Build

It needs these environment values:

- `MCUI_SHOWCASE_DIR`: the demo checkout, a Zephyr module. It provides the board target `verdin_am62p/am62p54/mcu_r5f0_0` and the DSS driver.
- `YAKOGL_DIR`: yakogl, the board `verdin_am62p` and the AM62P SoC. If it is unset, clone yakogl's `main` outside this repository.
- `ZEPHYR_WORKSPACE`: the west workspace at the Zephyr revision the demo pins.
- `ZEPHYR_SDK_INSTALL_DIR`: the Zephyr SDK. `$ZEPHYR_SDK_INSTALL_DIR/gnu/arm-zephyr-eabi` must exist.
- Rust: `rustup target add armv7r-none-eabihf`.

The workspace's own manifest repository also defines `verdin_am62p`, so west's automatic module list fails with "Duplicated board qualifiers detected". Pass the modules explicitly and leave out the manifest repository and `modules/lib/slint`:

```sh
cd "$ZEPHYR_WORKSPACE"   # west must run inside the workspace
export ZEPHYR_TOOLCHAIN_VARIANT=zephyr
M=$ZEPHYR_WORKSPACE/modules
west build -p always -b verdin_am62p/am62p54/mcu_r5f0_0 \
  -d <build directory> <this repository>/tests/board/am62p_r5f -- \
  -DZEPHYR_MODULES="$M/hal/cmsis;$M/hal/cmsis_6;$M/hal/ti;$M/hal/libmetal;$M/lib/open-amp;$M/lib/picolibc;$YAKOGL_DIR/driver;$YAKOGL_DIR;$MCUI_SHOWCASE_DIR"
```

Use `$MCUI_SHOWCASE_DIR/scripts/env.sh` and the demo's own build as the reference for the module list and the order. Add `lib/zcbor` or `fs/fatfs` if the board or the driver needs them. Optional: `-DTEST_HOLD_MS=<ms>`, and `-DTEST_APP_MAX_WIDTH=<px> -DTEST_APP_MAX_HEIGHT=<px>`. The default of 1280×800 is the largest panel that the two application buffers in `.bss` fit, 2 MB each. A larger panel ends the run with `SETUP: panel ... needs ...` and `RESULT FAIL (setup)`.

CMake runs `cargo build --locked --release -p composition-ffi-staticlib --no-default-features --features panic-handler --target armv7r-none-eabihf` in this repository, with the target directory inside the build directory.

None of this has been run. Before the first build:

1. Write `src/hw_am62p.c` from `$MCUI_SHOWCASE_DIR/include/mcui/am62p_dss.h`.
2. Compare `prj.conf` with `$MCUI_SHOWCASE_DIR/platform/fw/*.conf`: the display, the DSI bridge, the remoteproc resource table and the RAM console.
3. Check that the R5F's addresses of `.bss` are the addresses the DSS reads. The test passes `app_mem`'s address as the plane's address.

## Run

Use the remoteproc route of `$MCUI_SHOWCASE_DIR/docs/remoteproc-bringup.md`. Install the image as the R5F's firmware and stop or unbind what that document lists before the R5F takes the display. Start the remote processor and read the console from its trace buffer. Photograph each scene: the lines are printed when a scene starts, then the scene holds for `TEST_HOLD_MS`. Then restore what was stopped or unbound and reboot into Torizon. A second stop of the remote processor fails, so a second image needs a reboot. This route leaves the boot chain alone.

## The host run

`cargo test --workspace` includes `board_scenes_behind_a_fake_panel` in `backends/twoplane/tests/twoplane.rs`, which is also part of CI. It compiles `src/scenes.c`, the backend and `host/fake_panel.c` with the host's C compiler and links them with the engine. Then it runs the sequence on a 96×48 fake panel. At every frame start the fake driver latches its shadow registers. At every hold the fake panel composes the active planes at a corner and at the middle of each rectangle position, and compares them with the "should show" column above. This checks the sequence and its own judgement. It cannot check the driver, the timing or the caches.

## What was checked (2026-11-28, image build)

- **Command:** the one under [Build](#build), run as written with `ZEPHYR_TOOLCHAIN_VARIANT=zephyr` from inside `$ZEPHYR_WORKSPACE`, a build directory under `$TMPDIR`, and the module list exactly as given. `YAKOGL_DIR` was a read-only checkout of yakogl `main` beside the demo (a new clone was not possible in that session). No file of the demo or of yakogl was written or copied.
- **Result:** success. `west build` exited 0 (157 of 157 steps) and `zephyr/zephyr.elf` exists. No file of `tests/board/am62p_r5f` needed a fix.
- **Versions:** Zephyr 4.5.0 (the workspace's `VERSION`; revision 41d10b597304), Zephyr SDK 1.0.1 with `gnu/arm-zephyr-eabi`, `ZEPHYR_TOOLCHAIN_VARIANT=zephyr`, stable rustc 1.99.0 (2026-09-28) with the `armv7r-none-eabihf` target, west in the workspace's Python virtual environment.
- **Image size** (from the ELF section headers, in bytes; `arm-zephyr-eabi-size` itself was not run): text 80412 (code 0xeec8 + vector/boot + rodata + ISR tables, 0x376c of rodata), data 192, bss 4093336 + noinit 4936256 + RAM_CONSOLE 65536. The linker's own report: RAM 9142656 B of 12 MB (72.66%), ATCM 288 B of 32 KB, RSC_TABLE 68 B of 64 KB, RAM_CONSOLE 64 KB of 64 KB. The `.bss` is dominated by the two application buffers (2 MB each for the default 1280×800 limit) plus the `noinit` area.
- **Link (M3-05 evidence):** the final link of `zephyr.elf` succeeded with no unresolved symbol. That includes the engine's static library `libcomposition_engine.a` built for `armv7r-none-eabihf`, `ce_twoplane.c` and `scenes.c`, and `__aeabi_memclr4`, which the engine needs. Which object supplied `__aeabi_memclr4` (the library's weak copy from compiler_builtins or the C library) was not looked at; the map file was not inspected.
- **Not checked:** anything on a board; the image's behaviour; the host test and CI checks are in the report of the same change.

## What was checked (2026-11-27)

- **Host run:** `cargo test -p composition-twoplane board_scenes`. All eight scenes were `ok`, with commit and latched counts 1, 4, 5, 6, 7, 7, 7, 7, and the result was `RESULT PASS (8 scenes, vsyncs 61)`. Every `PANEL` line matched the expected colours.
- **Engine library for the R5F:** `cargo build --locked -p composition-ffi-staticlib --no-default-features --features panic-handler --target armv7r-none-eabihf --release` built with stable rustc 1.99.0. Only `rustup target add armv7r-none-eabihf` was needed. The library defines every engine symbol the test calls. It also defines `__aeabi_memclr4` and `memcpy` as weak symbols, from compiler_builtins.
- **C for the R5F:** `src/scenes.c` and `backends/twoplane/ce_twoplane.c` compiled without warnings with clang 23. The flags were `--target=armv7r-none-eabihf -mcpu=cortex-r5 -mfpu=vfpv3-d16 -mfloat-abi=hard -std=c11 -Wall -Wextra -Werror -pedantic -ffreestanding -nostdinc -O2`, with clang's own headers. The objects need nothing beyond the engine, the backend and `__aeabi_memclr4`. They were not linked: there was no Arm linker that could be run.
- **Not checked:**
  - The Zephyr build from this repository's `main`: not run (the owner's commit `361e31f` reports that the image builds; that is not a result of this repository's checks).
  - `arm-zephyr-eabi-gcc`: present in the SDK, but not run.
  - Anything on the board or the panel: no scene was run there, so there are no photographs and no console lines from hardware.
