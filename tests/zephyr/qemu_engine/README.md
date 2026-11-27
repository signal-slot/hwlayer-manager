# qemu_engine: the engine's C ABI under Zephyr, in QEMU

A Zephyr application that links `libcomposition_engine.a`
(`crates/composition-ffi-staticlib`, built `no_std` with `panic-handler` for
`thumbv7em-none-eabihf`) and drives it through
`crates/composition-ffi/include/composition_engine.h`.

The display backend is a fake in `src/main.c`: one 800x480 output, a primary
plane (XRGB8888, full screen only) and an overlay plane (ARGB8888) above it.
A commit writes shadow registers. A `k_timer` interrupt every 16 ms stands in
for vertical blank: it latches the shadow registers and the backend reports
`CE_BACKEND_LATCHED` from `poll_event`. Nothing touches display hardware.

**This is a QEMU run, not a hardware result.** It shows that the static
library links into a Zephyr image with the hard-float ABI and that the C ABI
works there: plan, commit, latch, release. It says nothing about any display
controller, about the i.MX95 M7, or about timing. The `uptime_ms` values come
from QEMU with `-icount` and are not measurements.

## What it does

1. `CE_ABI_CHECK()`, then `ce_engine_init` in a static, aligned buffer of
   `CE_ENGINE_SIZE` bytes.
2. Registers three buffers: background A and B (800x480 XRGB8888) and an OSD
   (320x80 ARGB8888), and marks them written.
3. Frame 1: background A plus the OSD in the lower right corner. Waits for
   `CE_EVENT_PRESENTED`. Checks that the fake controller latched A on the
   primary plane and the OSD on the overlay plane at the planned position.
4. Frame 2: background B plus the OSD. Expects `PRESENTED`, then
   `RELEASED` for A (A is released at B's latch, not before).
5. Checks that A may be written again and B, on screen, may not
   (`CE_ERR_BUFFER_IN_USE`).
6. Frame 3: back to A. Expects `PRESENTED` and `RELEASED` for B.
7. Unregisters B (idle) and fails to unregister A (on screen).

A latch timeout of 200 ms is set with `ce_set_latch_timeout`, and
`k_uptime_get()` is fed to `ce_advance_time`, so a missing vertical blank
would show up as `latch_timeout`, not as a hang.

## Output

One line per step:

```
ce: <what> key=value ...
```

`<what>` is `start`, `abi_check`, `buffers`, `commit` (one line per plane
assignment, printed by the backend), `present`, `event` (`kind=presented`,
`released`, `dropped`, `latch_timeout`, ...), `reuse`, `summary` and `stack`
(bytes of the 32 KiB main stack never used). The last line is `PASS`, or
`FAIL <line> <condition>` followed by `FAIL`.

The run ends with a semihosting `SYS_EXIT` (`CONFIG_SEMIHOST=y`): QEMU exits
with status 0 on `PASS` and 1 on `FAIL`, so `west build -t run` succeeds or
fails with the test.

## Board and float ABI

The Rust library is built for `thumbv7em-none-eabihf`: ARMv7E-M with the
FPv4-SP-D16 FPU and the hard-float calling convention (float arguments in FPU
registers). The Zephyr image must use the same convention.

Read from source, Zephyr v4.4.2:

- `mps2/an500` is the Cortex-M7 board, but it cannot be used here.
  `soc/arm/mps2/Kconfig` gives `SOC_AN500` no FPU (`CPU_HAS_VFP` or
  `CPU_HAS_FPU` is not selected), so `CONFIG_FPU` and `CONFIG_FP_HARDABI`
  cannot be set. `boards/arm/mps2/board.cmake` lists only `armfvp` (the Arm
  FVP, not QEMU) as its emulator, and `boards/arm/mps2/Kconfig` does not
  select `QEMU_TARGET` for it.
- Zephyr v4.4.2 has no QEMU board with a Cortex-M7. The Cortex-M CPUs its
  boards pass to QEMU (`QEMU_CPU_TYPE` in `board.cmake`) are the M0
  (`qemu_cortex_m0`), M3 (`qemu_cortex_m3`, `mps2/an385`), M4
  (`mps2/an386`), M33 (`mps2/an521`) and M55 (`mps3`). The M33 and M55 are
  ARMv8-M, which `thumbv7em` does not target.
- `mps2/an386` is a Cortex-M4 with an FPU (`SOC_AN386` selects
  `CPU_HAS_VFP`) and runs in QEMU (`-machine mps2-an386 -cpu cortex-m4`).
  The Cortex-M4 is ARMv7E-M like the Cortex-M7, and FPv4-SP-D16 is the FPU
  that `thumbv7em-none-eabihf` targets. It is the nearest board that matches,
  and this app uses it.

What the Cortex-M4 does not cover: anything specific to the Cortex-M7
(caches, TCM, the M7's double-precision FPU) and anything of the i.MX95.

`prj.conf` sets `CONFIG_FPU=y` and `CONFIG_FP_HARDABI=y`. The linker enforces
the match. Observed 2026-11-28 (GNU Arm Embedded build): the same app built
with `-DCONFIG_FP_SOFTABI=y`
does not link; `ld` reports, for every object of the library,
`libcomposition_engine.a(...) uses VFP register arguments, zephyr/zephyr_pre0.elf does not`.

## Build and run

Requirements:

- A west workspace of upstream Zephyr v4.4.2 (commit
  `dccb09599635bdff17633fa7e9dab014b91dce90`). Only the `cmsis_6` module
  is needed for this board:
  ```
  west init -m https://github.com/zephyrproject-rtos/zephyr --mr v4.4.2 <workspace>
  cd <workspace>
  west update cmsis_6
  ```
  plus the Python packages of `zephyr/scripts/requirements-base.txt`.
- The Zephyr SDK named in `zephyr/SDK_VERSION` (1.0.1 for v4.4.2) with the
  `arm-zephyr-eabi` toolchain and its host tools (they provide
  `qemu-system-arm`). `west sdk install --version 1.0.1 -t arm-zephyr-eabi`
  installs it.
- Rust with the `thumbv7em-none-eabihf` target
  (`rustup target add thumbv7em-none-eabihf`).

The build runs cargo itself:

```
cargo build --locked -p composition-ffi-staticlib --no-default-features \
    --features panic-handler --target thumbv7em-none-eabihf --release \
    --target-dir <build-dir>/cargo
```

so the result does not depend on `CARGO_TARGET_DIR`. To link a library built
elsewhere with the same options, pass
`-DCOMPOSITION_ENGINE_LIB=/path/to/libcomposition_engine.a`.

From the west workspace:

```
west build -p always -b mps2/an386 -d <build-dir> <repo>/tests/zephyr/qemu_engine \
    -- -DZEPHYR_SDK_INSTALL_DIR=<zephyr-sdk-1.0.1>
west build -d <build-dir> -t run
```

The app also builds with the GNU Arm Embedded toolchain instead of the SDK
(`-- -DZEPHYR_TOOLCHAIN_VARIANT=gnuarmemb -DGNUARMEMB_TOOLCHAIN_PATH=<prefix>`);
`west build -t run` then still needs a `qemu-system-arm`. Both ways gave the
same `PASS` on 2026-11-28.

CI does not build or run this app.

## Log

QEMU run, not a hardware result. 2026-11-28 (19:05 UTC on 2026-11-27).
Repository at `107c5c9` plus this directory. Zephyr v4.4.2, Zephyr SDK 1.0.1
(`arm-zephyr-eabi`, QEMU from its host tools), board `mps2/an386`,
`rustc 1.98.0-nightly (4c9d2bfe4 2026-07-01)`. Command:
`west build -d <build-dir> -t run`.

```
-- west build: running target run
[0/7] cargo build composition-ffi-staticlib for thumbv7em-none-eabihf
    Finished `release` profile [optimized] target(s) in 0.02s
[1/2] To exit from QEMU enter: 'CTRL+a, x'[QEMU] CPU: cortex-m4
qemu-system-arm: warning: nic lan9118.0 has no peer
*** Booting Zephyr OS build v4.4.2 ***
ce: start board=mps2/an386
ce: abi_check ok engine_size=28672
ce: buffers registered count=3
ce: commit submission=1 layer=0 buffer=1 plane=10 zpos=0 src=0,0,800x480 dst=0,0,800x480
ce: commit submission=1 layer=1 buffer=3 plane=11 zpos=1 src=0,0,320x80 dst=464,384,320x80
ce: present submission=1 status=0 layers=2
ce: event kind=presented output=0 submission=1 buffer=0 uptime_ms=20
ce: commit submission=2 layer=0 buffer=2 plane=10 zpos=0 src=0,0,800x480 dst=0,0,800x480
ce: commit submission=2 layer=1 buffer=3 plane=11 zpos=1 src=0,0,320x80 dst=464,384,320x80
ce: present submission=2 status=0 layers=2
ce: event kind=presented output=0 submission=2 buffer=0 uptime_ms=40
ce: event kind=released output=0 submission=0 buffer=1 uptime_ms=40
ce: reuse buffer=1 ok
ce: commit submission=3 layer=0 buffer=1 plane=10 zpos=0 src=0,0,800x480 dst=0,0,800x480
ce: commit submission=3 layer=1 buffer=3 plane=11 zpos=1 src=0,0,320x80 dst=464,384,320x80
ce: present submission=3 status=0 layers=2
ce: event kind=presented output=0 submission=3 buffer=0 uptime_ms=60
ce: event kind=released output=0 submission=0 buffer=2 uptime_ms=60
ce: summary commits=3 vblanks=3
ce: stack size=32768 unused=26344
PASS
```

`west build -t run` returned 0. Image size with the SDK toolchain: 59688 B
flash, 67048 B RAM (of which 28672 B is the engine storage and 32768 B the
main stack).

Also observed 2026-11-28 (GNU Arm Embedded build, the same QEMU): with one
check changed on purpose to fail, the run
printed `FAIL`, QEMU exited with status 1, and `west build -t run` failed
(`command exited with status 1`). The change was reverted.

Re-run, QEMU run, not a hardware result. 2026-11-28, on a second host, with
the same repository state and commands but Zephyr v4.5.0 (Zephyr SDK 1.0.1,
which is what v4.5.0's `zephyr/SDK_VERSION` names) and
`rustc 1.99.0 (b940084d7 2026-09-28)`. It printed the same lines as above,
apart from the banner (`*** Booting Zephyr OS build v4.5.0 ***`), the
`uptime_ms` values (40, 60, 80) and `unused=26352`, and ended in `PASS`.
`west build -t run` returned 0. Image size: 60452 B flash, 67136 B RAM.
