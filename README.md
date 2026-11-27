# hwlayer-manager

A cross-platform **hardware composition engine**. Applications hand it
completed rendering buffers; the engine decides how to present them on the
display hardware: direct scanout, hardware overlay planes and, where a backend
explicitly supports it, pre-composition. It is not a toolkit or a compositor
(not a replacement for Slint, Qt, YakoGL or Weston); it only arranges content
that others have already rendered. The full scope, architecture and milestones
are in [`SoW.md`](SoW.md) (§1 objectives, §3 architecture).

The design is a `no_std`, allocation-free Rust planning core (scene and
capability model, plane assignment and validation, buffer and presentation
state machine) behind a display-backend interface. Backends are a Linux
DRM/KMS atomic backend that takes a file descriptor (a DRM lease descriptor
from AGL drm-lease-manager works the same as a device node), and
Zephyr/MCU-side backends written against a C ABI.

## Licence

Apache License 2.0. See [`LICENSE`](LICENSE) for the terms.

## Crate map

| Path | What it is |
| --- | --- |
| `crates/composition-core` | `no_std` data model, plane planner and presentation state machine; the backend contract. No `unsafe`. |
| `crates/composition-fake` | Host-only fake display backend (shadow registers, test-controlled vsync) for deterministic tests of the core. |
| `crates/composition-drm` | Linux DRM/KMS atomic backend; reports the capabilities of whatever descriptor it is given. |
| `crates/composition-ffi` | C ABI of the engine, with backends implemented in C; header at `crates/composition-ffi/include/composition_engine.h`. The one crate that needs `unsafe`. |
| `crates/composition-ffi-staticlib` | The same C ABI built as the static library `libcomposition_engine.a` for C programs. |
| `backends/twoplane` | Backend for a display driver with two planes on one output, written in C (`ce_twoplane.c`); the crate holds only its host tests. |

Other workspace members: `tools/drm-harness` (producer harness that drives
`composition-drm` on a real DRM output; see its
[`README`](tools/drm-harness/README.md)) and `tools/vm-init` (PID 1 of the
throwaway VM for the vkms tests).

## Build and test (host)

```sh
cargo build
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

`cargo test --workspace` also compiles and runs the C tests with the system
`cc`. The vkms tests are ignored by default.

### Bare-metal (thumbv7em) builds

CI also runs these two (needs `rustup target add thumbv7em-none-eabihf`):

```sh
cargo build -p composition-core --target thumbv7em-none-eabihf
cargo build -p composition-ffi-staticlib --no-default-features --features panic-handler --target thumbv7em-none-eabihf --release
```

CI is described in [`docs/ci.md`](docs/ci.md). A green run says nothing about
hardware.

## Scripts and tools

- `scripts/cross-aarch64.sh`: cross-builds the DRM backend, its `probe` example and the harness for aarch64 Linux.
- `scripts/build-dlm-aarch64.sh`: build recipe for AGL drm-lease-manager for aarch64 (no binary produced yet, see `docs/dlm-aarch64.md`).
- `scripts/vkms-vm-test.sh`: boots a kernel in QEMU and runs the vkms tests.
- `scripts/camera-check.py`: host checker for camera pictures of the two-plane test patterns ([`scripts/camera-check-README.md`](scripts/camera-check-README.md)).
- `tools/drm-harness`: board-side producer harness (above). `tools/vm-init`: VM init for the vkms tests.

## Documentation

- [`SoW.md`](SoW.md): statement of work: objectives, architecture, work packages, milestones.
- [`docs/ci.md`](docs/ci.md): what CI runs and what a green run does and does not show.
- [`docs/dlm-aarch64.md`](docs/dlm-aarch64.md): pinned sources and build script for drm-lease-manager on aarch64.
- [`docs/host-metrics.md`](docs/host-metrics.md): plane-assignment cost of the core on a development host (no board involved).
- [`docs/imx95-board.md`](docs/imx95-board.md): cross-building for and running on the Toradex Verdin iMX95 (Reference Multimedia Image, kernel 6.6), with placeholders instead of addresses.
- [`docs/imx95-dpu-source-6.6.md`](docs/imx95-dpu-source-6.6.md): desk reading of the i.MX95 display driver in the board's 6.6.157-7.8.0 kernel.
- [`docs/m1-imx95-test-plan.md`](docs/m1-imx95-test-plan.md): WP0 test plan mapping each M1 acceptance condition to scenarios and evidence.
- [`docs/platform-capabilities.md`](docs/platform-capabilities.md): what is known about each target and how (vkms / source / inferred), plus the planned tools.
- [`docs/twoplane-backend.md`](docs/twoplane-backend.md): the C two-plane backend, what its host tests cover, and what a board test still needs.
- [`docs/wp5-gap-analysis.md`](docs/wp5-gap-analysis.md): desk analysis of leased vs Weston-owned output and libliftoff (SoW §4.2, WP5).
- [`docs/observed/`](docs/observed/): per-SoC topology notes:
  - [`bcm2711-cm4-topology.md`](docs/observed/bcm2711-cm4-topology.md): BCM2711 (CM4 / reTerminal) DRM topology.
  - [`bcm2712-topology.md`](docs/observed/bcm2712-topology.md): BCM2712 (Raspberry Pi 5) display topology.
  - [`rk3588-source.md`](docs/observed/rk3588-source.md): RK3588 VOP2 topology read from source.
  - [`rk3588-topology.md`](docs/observed/rk3588-topology.md): RK3588 DRM topology on the NanoPC-T6.
  - [`stm32mp257-source.md`](docs/observed/stm32mp257-source.md): STM32MP257 LTDC topology read from ST kernel source.
  - [`stm32mp257-topology.md`](docs/observed/stm32mp257-topology.md): STM32MP257F-DK display topology (probe and raw properties).
  - [`stm32mp257.md`](docs/observed/stm32mp257.md): STM32MP257F-DK topology and two-plane run.
  - [`vc4-hvs-source.md`](docs/observed/vc4-hvs-source.md): VC4 / HVS topology read from Raspberry Pi kernel source.

## Status

Taken from the documents above; nothing is claimed beyond them. Tags: observed
means measured on a board on the stated date, source means read from source,
inferred means concluded from either.

- **M1 (i.MX95 Linux DRM / AGL):** not met. For i.MX95 the docs record desk research only (driver topology read from source, `docs/platform-capabilities.md`, `docs/imx95-dpu-source-6.6.md`) and a test plan (`docs/m1-imx95-test-plan.md`); the plan's results file, `docs/m1-imx95-report.md`, is not present. No multi-plane run on i.MX95 is recorded, and the AGL leased-output demonstration is not done. The aarch64 build recipes exist but the drm-lease-manager binary has not been produced (`docs/dlm-aarch64.md`).
- **M2 (i.MX95 Zephyr):** not started in the docs. The i.MX95 Zephyr row is source reading only; who owns the DPU, A55 or M7, is recorded as unexamined (`docs/platform-capabilities.md`). Not measured on hardware.
- **M3 (AM62P regression):** not met. The C two-plane backend, taken from the AM62P demo firmware, passes host tests against a fake driver; that its latch counter follows real hardware and the timing of latches are not tested (`docs/twoplane-backend.md`). No AM62P run is recorded here.
- **WP1 (`no_std` core):** the core builds for `thumbv7em-none-eabihf` and has host tests in CI; host costs are in `docs/host-metrics.md`. These are host results only.
- **WP2 / WP3 / WP4:** see M1, M2 and M3 above. Other SoCs have dated observations of their display topology (STM32MP257F-DK, RK3588, BCM2712, BCM2711 on 2026-11-28; the STM32MP257F-DK two-plane run on 2026-11-27, re-run 2026-11-28, in `docs/observed/`). These are not milestone results.
- **WP5:** gap analysis of leased and Weston-owned output is desk research only (`docs/wp5-gap-analysis.md`); this README and the licence are the minimal documentation so far. Demo and benchmark evidence are not produced.
