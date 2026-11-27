# Composition engine on the i.MX95 Cortex-M7 (Zephyr)

A Zephyr application that links the engine's C ABI, `libcomposition_engine.a` from `composition-ffi-staticlib` built for `thumbv7em-none-eabihf`, through `crates/composition-ffi/include/composition_engine.h`. It presents five fixed scenes to a backend that describes the stock Zephyr DPU driver, and prints each plan (or the reason there is none) on the console.

The backend matches the "i.MX95, Zephyr" section of `docs/platform-capabilities.md` (read from source, not measured): one output, one primary framebuffer plane, linear RGB565 / RGB888 / ARGB8888, opaque only, no plane alpha, no scaling, no rotation, a fixed zpos. The plane is marked full-output-only because the driver exposes a single framebuffer for the whole panel (inferred). The output is 1280x800, the size of the Verdin iMX95 setup's 10.1" LVDS panel. That is an assumption of this test: nothing reads the panel.

**No display access.** The backend's `check` accepts what the planner proposes, and its `commit` only prints the assignments and reports the latch at once. The program uses the console UART and nothing else. It is a test of the engine on the M7 toolchain and memory budget, not of scanout.

Upstream Zephyr only. Nothing here comes from the showcase.

## Target

`imx95_evk/mimx9596/m7`, the upstream i.MX95 EVK M7 target (TCM, XIP). There is no upstream Verdin iMX95 board. Loading on the Verdin (remoteproc, console UART, memory map) is not covered here; see `docs/imx95-zephyr-feasibility.md` once it exists.

## Zephyr version

Built with upstream `zephyrproject-rtos/zephyr` `main` at `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (2026-11-27; the build reports `v4.5.0-5218-ge53873cbd0ef`, Zephyr 4.5.99), with Zephyr SDK 1.0.1 (`arm-zephyr-eabi`, GCC 14.3.0) and the modules `hal_nxp`, `cmsis`, `cmsis_6`, `libmetal` and `open-amp` from that commit's `west.yml`.

## Build

Prerequisites: a Zephyr workspace (`west init` / `west update`, or at least `zephyr` plus the `hal_nxp` and `cmsis_6` modules), the Zephyr SDK with `arm-zephyr-eabi`, and Rust with the `thumbv7em-none-eabihf` target:

```sh
rustup target add thumbv7em-none-eabihf
```

### One step (CMake runs cargo)

From the repository root (`target/` is git-ignored; with `ZEPHYR_BASE` set, `west` does not need to run inside the workspace):

```sh
west build -b imx95_evk/mimx9596/m7 -d target/zephyr/imx95_m7_engine tests/board/imx95_m7_engine
```

The build runs, from the repository root and with `CARGO_TARGET_DIR` inside the build directory:

```sh
cargo build --locked -p composition-ffi-staticlib --no-default-features --features panic-handler --target thumbv7em-none-eabihf --release
```

and links the resulting `libcomposition_engine.a`. Cargo runs on every build and does nothing if nothing changed.

### Two steps (prebuilt library)

```sh
cargo build --locked -p composition-ffi-staticlib --no-default-features --features panic-handler --target thumbv7em-none-eabihf --release
west build -b imx95_evk/mimx9596/m7 -d target/zephyr/imx95_m7_engine tests/board/imx95_m7_engine -- \
  -DCOMPOSITION_ENGINE_LIB=$PWD/target/thumbv7em-none-eabihf/release/libcomposition_engine.a
```

### Configuration notes

- `CONFIG_FPU` and `CONFIG_FP_HARDABI`: the Rust target uses the hard-float calling convention, so Zephyr has to as well, or the link fails on mismatched VFP attributes. The upstream M7 defconfig does not enable the FPU itself.
- `CONFIG_MAIN_STACK_SIZE=16384`: the planner runs on `main`'s stack. The size is a margin, not a measurement.
- The engine needs `CE_ENGINE_SIZE` (28672) bytes of statically allocated storage.

### Emulation

`mps2/an386` (Cortex-M4F, QEMU) can run the same program, because `thumbv7em-none-eabihf` code runs on an M4 with a single-precision FPU:

```sh
west build -b mps2/an386 -d target/zephyr/imx95_m7_engine_an386 tests/board/imx95_m7_engine -t run
```

That is emulation of a different core, not the i.MX95. QEMU comes with the Zephyr SDK host tools (`setup.sh -h`). `mps2/an385` (Cortex-M3) cannot run thumbv7em code. `mps2/an500` (Cortex-M7) declares no FPU and is not a QEMU target in upstream Zephyr.

## Expected output

The lines below come from compiling `src/main.c` for the host against stand-in Zephyr headers (`printk` as `printf`) and the hosted build of the same library (2026-11-28). The banner names the board target on Zephyr. The plan lines do not depend on the target.

```
hwlayer-manager composition engine on imx95_evk/mimx9596/m7
capabilities: 1 output 1280x800, 1 primary framebuffer plane (RGB565/RGB888/ARGB8888, linear, opaque, no scaling, full output only)
scene: XRGB8888 primary (1 layer(s))
  not planned: CE_ERR_NO_PLANE (-15), layer 0
    reasons 0x2: format
  -> as expected
scene: primary only (1 layer(s))
  plan: submission 1, 1 assignment(s) for 1 layer(s)
    layer 0 (buffer 4) -> plane 0 (index 0) zpos 0 src 0,0 1280x800 dst 0,0 1280x800
  event: presented submission 1 buffer 0
  -> as expected
scene: primary + overlay (2 layer(s))
  not planned: CE_ERR_NO_PLANE (-15), layer 1
    reasons 0x400: in-use
  -> as expected
scene: scaled window (1 layer(s))
  not planned: CE_ERR_NO_PLANE (-15), layer 0
    reasons 0x84: scaling full-output
  -> as expected
scene: primary again (1 layer(s))
  plan: submission 2, 1 assignment(s) for 1 layer(s)
    layer 0 (buffer 4) -> plane 0 (index 0) zpos 0 src 0,0 1280x800 dst 0,0 1280x800
  event: presented submission 2 buffer 0
  -> as expected
checks 2, commits 2
RESULT: PASS
```

What the scenes show:

1. An XRGB8888 layer is refused: the plane lists no XRGB8888 (`format`).
2. A full-screen ARGB8888 layer gets the only plane.
3. Primary plus one overlay window: the overlay finds the only plane taken (`in-use`). With the stock driver, a second layer has to be composed in software before the engine sees the frame. That is the M2 overlay gap.
4. A window scaled from 640x360 to 1280x720 is refused (`scaling`, and `full-output` because it does not cover the panel).
5. The primary again: still plannable. There is no `released` event because the same buffer stays on screen.

Zephyr's boot banner (`*** Booting Zephyr OS ... ***`) comes before these lines.

## Status

As of 2026-11-28:

- Done: the one-step `west build -p always -b imx95_evk/mimx9596/m7` above, with the Zephyr commit and SDK named in "Zephyr version". CMake ran cargo and linked the library with no warnings. `zephyr.elf`: 64864 B text, 888 B data, 48537 B bss. In the linker's region map, FLASH used 65756 B of 256 KiB and RAM 49424 B of 256 KiB.
- Done: `west build -p always -b mps2/an386` of the same application. It links too: 56080 B text, 180 B data, 50330 B bss.
- Done: `src/main.c` compiled without warnings for the host (gcc, `-Wall -Wextra`) and ran with the output above, against the hosted library.
- Not done: running it under QEMU on `mps2/an386`. The SDK host tools, which bring QEMU, were not installed on the build machine. So the expected output above comes from the host run, not from emulation.
- Not done: any run on hardware.
