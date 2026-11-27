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

Apache License 2.0. See [`LICENSE`](LICENSE) for the terms, [`NOTICE`](NOTICE)
and [`docs/third-party-licences.md`](docs/third-party-licences.md).

## Crate map

| Path | What it is |
| --- | --- |
| `crates/composition-core` | `no_std` data model, plane planner and presentation state machine; the backend contract. No `unsafe`. |
| `crates/composition-fake` | Host-only fake display backend (shadow registers, test-controlled vsync) for deterministic tests of the core. |
| `crates/composition-drm` | Linux DRM/KMS atomic backend; reports the capabilities of whatever descriptor it is given. |
| `crates/composition-ffi` | C ABI of the engine, with backends implemented in C; header at `crates/composition-ffi/include/composition_engine.h`. The one crate that needs `unsafe`. |
| `crates/composition-ffi-staticlib` | The same C ABI built as the static library `libcomposition_engine.a` for C programs. |
| `backends/twoplane` | Backend for a display driver with two planes on one output, written in C (`ce_twoplane.c`); the crate holds only its host tests. |
| `tools/drm-harness` | Producer harness that drives `composition-drm` on a real DRM output; see its [`README`](tools/drm-harness/README.md). |
| `tools/vm-init` | PID 1 of the throwaway VM for the vkms tests. |

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
hardware. `scripts/check-all.sh` runs the CI steps locally.

## Scripts

- `scripts/cross-aarch64.sh`: cross-builds the DRM backend, its `probe` example and the harness for aarch64 Linux.
- `scripts/build-dlm-aarch64.sh`: build recipe for AGL drm-lease-manager for aarch64 (no binary produced yet, see [`docs/dlm-aarch64.md`](docs/dlm-aarch64.md)).
- `scripts/vkms-vm-test.sh`, `scripts/vkms-harness-test.sh`: boot a kernel in QEMU and run the vkms tests, and the harness on vkms.
- `scripts/camera-check.py`: host checker for camera pictures of the two-plane test patterns ([`scripts/camera-check-README.md`](scripts/camera-check-README.md)).
- `scripts/probe-table.py`: table from probe output ([`scripts/probe-table-README.md`](scripts/probe-table-README.md)).
- `scripts/imx95-session.sh`: host-side helper for short sessions on the Verdin iMX95 ([`scripts/imx95-session-README.md`](scripts/imx95-session-README.md)).
- `scripts/imx95-m7-rproc.sh`: load, stop and restore for the i.MX95 M7 through remoteproc (host test `scripts/imx95-m7-rproc.test.sh`).
- `scripts/prepare-zephyr.sh`: pinned Zephyr workspace and SDK preparation ([`docs/zephyr-workspace.md`](docs/zephyr-workspace.md)).
- Checks: `scripts/check-all.sh`, `scripts/check-doc-links.sh` (`--anchors` also checks fragments), `scripts/check-ffi-header.sh`, `scripts/check-zephyr-apps.sh`, `scripts/check-no-board-secrets.sh`, `scripts/check-no-farm-ids.sh`, `scripts/check-m1-report.sh`. Several have a `.test.sh` beside them.

## Zephyr apps

Board-test applications for SoW milestone M2, kept in the repository:

- [`tests/board/imx95_m7`](tests/board/imx95_m7/README.md): i.MX95 Cortex-M7 hello and read-only DPU probe (upstream Zephyr).
- [`tests/board/imx95_m7_engine`](tests/board/imx95_m7_engine/README.md): the engine's C ABI on the M7, presenting fixed scenes to a backend that describes the stock Zephyr DPU driver.
- [`tests/board/imx95_m7_scanout`](tests/board/imx95_m7_scanout/README.md): framebuffer scanout from the i.MX95 Cortex-M7 through Zephyr's display API, with per-frame timing hooks. Host builds only; its README records that nothing has been run on hardware and no timing is measured.

QEMU apps, not hardware results (they count towards no milestone):

- [`tests/zephyr/qemu_engine`](tests/zephyr/qemu_engine/README.md): the engine's C ABI under Zephyr in QEMU.
- [`tests/zephyr/display_dummy`](tests/zephyr/display_dummy/README.md): the engine over Zephyr's dummy display under QEMU.

Writing and running one: [`docs/board-tests.md`](docs/board-tests.md).

## Documentation

- [`SoW.md`](SoW.md): statement of work: objectives, architecture, work packages, milestones.
- [`CONTRIBUTING.md`](CONTRIBUTING.md), [`SECURITY.md`](SECURITY.md), [`CHANGELOG.md`](CHANGELOG.md), [`NOTICE`](NOTICE), [`docs/third-party-licences.md`](docs/third-party-licences.md), [`LICENSE`](LICENSE).
- [`.github/pull_request_template.md`](.github/pull_request_template.md) and the [board-result issue template](.github/ISSUE_TEMPLATE/board-result.md).

### Overview and traceability

- [`docs/glossary.md`](docs/glossary.md): terms of the engine, DRM, Zephyr and the i.MX95.
- [`docs/sow-traceability.md`](docs/sow-traceability.md): SoW deliverables and milestone criteria mapped to repository evidence.
- [`docs/architecture.md`](docs/architecture.md): engine architecture overview.
- [`docs/testing.md`](docs/testing.md): test map from SoW §10 to the tests in this repository.
- [`docs/msrv.md`](docs/msrv.md): minimum Rust version of the workspace.
- [`docs/trace-format.md`](docs/trace-format.md): trace line format for engine events (proposal, X-03).
- [`docs/open-issues.md`](docs/open-issues.md): open issues for M2, M3, WP5 and cross-cutting.
- [`docs/platform-capabilities.md`](docs/platform-capabilities.md): what is known about each target and how (vkms / source / inferred), plus the planned tools.

### i.MX95 Zephyr (M2) studies

- [`docs/imx95-zephyr-feasibility.md`](docs/imx95-zephyr-feasibility.md): who owns the display, from source reading.
- [`docs/imx95-zephyr/README.md`](docs/imx95-zephyr/README.md): evidence directory; holds a dated read-only board snapshot, [`observed-2026-11-27.txt`](docs/imx95-zephyr/observed-2026-11-27.txt).
- [`docs/imx95-sm-display-lm.md`](docs/imx95-sm-display-lm.md): a System Manager configuration giving the display to the M7 (inferred).
- [`docs/imx95-m7-verdin-bsp.md`](docs/imx95-m7-verdin-bsp.md): Verdin iMX95 M7 BSP facts (source study).
- [`docs/imx95-m7-scmi.md`](docs/imx95-m7-scmi.md): SCMI reach for display clocks and power domains under Zephyr.
- [`docs/imx95-m7-lvds-route.md`](docs/imx95-m7-lvds-route.md): DPU to LVDS panel route (source study).
- [`docs/imx95-m7-fb-memory.md`](docs/imx95-m7-fb-memory.md): scanout buffer memory, cache and MPU rules on the M7.
- [`docs/imx95-m7-rpmsg-log.md`](docs/imx95-m7-rpmsg-log.md): taking the Zephyr log over rpmsg / trace buffer (design).
- [`docs/imx95-m7-overlay-design.md`](docs/imx95-m7-overlay-design.md): two-plane DPU overlay under Zephyr (design).
- [`docs/imx95-m7-backend-contract.md`](docs/imx95-m7-backend-contract.md): mapping the engine backend contract onto Zephyr.
- [`docs/imx95-twoplane-port.md`](docs/imx95-twoplane-port.md): filling the two-plane backend's tables for the i.MX95 DPU (design).
- [`docs/imx95-m7-dpu-driver.md`](docs/imx95-m7-dpu-driver.md): upstream Zephyr DPU driver, scanout and VSYNC/latch path.
- [`docs/imx95-m7-dpu-irq.md`](docs/imx95-m7-dpu-irq.md): DPU interrupts and the M7 NVIC.
- [`docs/imx95-m7-rproc-load.md`](docs/imx95-m7-rproc-load.md): how Linux `imx_rproc` loads an M7 ELF.
- [`docs/imx95-m7-console.md`](docs/imx95-m7-console.md): the M7 console UART and how to read its log.
- [`docs/imx95-m7-latch-measure.md`](docs/imx95-m7-latch-measure.md): measuring latch and VSYNC timing on the M7 (design).
- [`docs/imx95-m7-reduced-scope.md`](docs/imx95-m7-reduced-scope.md): reduced-scope bring-up if the M7 cannot own the DPU.
- [`docs/imx95-display-clocks.md`](docs/imx95-display-clocks.md), [`docs/imx95-display-power.md`](docs/imx95-display-power.md): display clocks, power domains and resets under the System Manager.
- [`docs/imx95-display-handover.md`](docs/imx95-display-handover.md): ways to hand the display from Linux to the M7, against the board rules.
- [`docs/imx95-verdin-bsp-src.md`](docs/imx95-verdin-bsp-src.md), [`docs/imx95-verdin-zephyr-board.md`](docs/imx95-verdin-zephyr-board.md): Verdin BSP sources and what a Zephyr board would need.
- [`docs/imx95-mcux-display.md`](docs/imx95-mcux-display.md): NXP MCUXpresso display examples (source study).
- [`docs/imx95-linux-zephyr-compare.md`](docs/imx95-linux-zephyr-compare.md): design of the Linux vs Zephyr comparison.
- [`docs/m2-comparison-plan.md`](docs/m2-comparison-plan.md): Linux vs Zephyr comparison of the same scene.
- [`docs/am62p-imx95-matrix.md`](docs/am62p-imx95-matrix.md): AM62P vs i.MX95 difference matrix (WP4, from recorded docs).
- [`docs/zephyr-workspace.md`](docs/zephyr-workspace.md): Zephyr workspace and SDK for board applications.

### WP5 design notes

- [`docs/wp5-gap-analysis.md`](docs/wp5-gap-analysis.md): leased vs Weston-owned output and libliftoff (SoW §4.2).
- [`docs/weston-imx-lease.md`](docs/weston-imx-lease.md): Weston on the i.MX95 image and DRM leases.
- [`docs/advisory-planner-design.md`](docs/advisory-planner-design.md): planner-only / advisory entry point.
- [`docs/libliftoff-crosscheck.md`](docs/libliftoff-crosscheck.md): cross-checking the planner against libliftoff.
- [`docs/sync-fences.md`](docs/sync-fences.md): readiness, fences and release.
- [`docs/twoplane-backend.md`](docs/twoplane-backend.md): the C two-plane backend and what a board test still needs.

### CI and scripts

- [`docs/wp5-evidence-plan.md`](docs/wp5-evidence-plan.md): plan for the reproducible demo and benchmark evidence (not yet produced).
- [`docs/ci.md`](docs/ci.md): what CI runs and what a green run does and does not show.
- [`docs/host-metrics.md`](docs/host-metrics.md): plane-assignment cost of the core on a development host (no board involved).
- [`docs/dlm-aarch64.md`](docs/dlm-aarch64.md): pinned sources and build script for drm-lease-manager on aarch64.

### Board docs

- [`docs/imx95-board.md`](docs/imx95-board.md): cross-building for and running on the Toradex Verdin iMX95 (Reference Multimedia Image, kernel 6.6), with placeholders instead of addresses.
- [`docs/imx95-dpu-source-6.6.md`](docs/imx95-dpu-source-6.6.md): desk reading of the i.MX95 display driver in the board's 6.6.157-7.8.0 kernel.
- [`docs/imx95-probe-2026-11-28.txt`](docs/imx95-probe-2026-11-28.txt): recorded probe output.
- [`docs/m1-imx95-test-plan.md`](docs/m1-imx95-test-plan.md): WP0 test plan mapping each M1 acceptance condition to scenarios and evidence.
- [`docs/board-scenarios.md`](docs/board-scenarios.md): SoW §10 board scenarios, design only.
- Per-SoC topology notes in [`docs/observed/`](docs/observed/):
  - [`bcm2711-cm4.md`](docs/observed/bcm2711-cm4.md), [`bcm2711-cm4-topology.md`](docs/observed/bcm2711-cm4-topology.md): BCM2711 (CM4 / reTerminal).
  - [`bcm2712.md`](docs/observed/bcm2712.md), [`bcm2712-topology.md`](docs/observed/bcm2712-topology.md): BCM2712 (Raspberry Pi 5).
  - [`vc4-hvs-source.md`](docs/observed/vc4-hvs-source.md): VC4 / HVS read from Raspberry Pi kernel source.
  - [`rk3588-source.md`](docs/observed/rk3588-source.md), [`rk3588-topology.md`](docs/observed/rk3588-topology.md): RK3588 VOP2.
  - [`stm32mp257.md`](docs/observed/stm32mp257.md), [`stm32mp257-source.md`](docs/observed/stm32mp257-source.md), [`stm32mp257-topology.md`](docs/observed/stm32mp257-topology.md): STM32MP257F-DK.

## Status

Taken from the documents above; nothing is claimed beyond them. Tags: observed
means measured on a board on the stated date, source means read from source,
inferred means concluded from either.

- **M1 (i.MX95 Linux DRM / AGL):** the report `docs/m1-imx95-report.md` (written 2026-11-28; hardware runs observed 2026-11-27 and 2026-11-28 on a Toradex Verdin iMX95 on a Mallow carrier) gives the overall verdict *passed*: real multi-plane composition through `composition-drm` on the board, a kernel lease and AGL drm-lease-manager `68c48758c46d` run from `/tmp`. Conditions 4 and 7 are passed with stated gaps, and the report notes that whether this board counts as "equivalent" to the SoW's EVK is the requester's call. Weston was stopped during every run; no Weston lease was demonstrated (`docs/wp5-gap-analysis.md`). The test plan is `docs/m1-imx95-test-plan.md`; the report lists what was not measured.
- **M2 (i.MX95 Zephyr):** not met. The repository holds source studies and designs (listed above), a dated read-only board snapshot (observed 2026-11-27, `docs/imx95-zephyr/`) and a host-built scanout app that has not run on hardware. `docs/imx95-zephyr-feasibility.md` is a draft from source reading: its ownership conclusion, that the A55/Linux side owns the display under the NXP EVK System Manager configurations, is inferred, and the configuration Toradex ships was not read. No scanout, VSYNC or latch result from Zephyr exists, and nothing is measured on hardware (`docs/open-issues.md` M2-01 to M2-07).
- **M3 (AM62P regression):** not met. The C two-plane backend, taken from the AM62P demo firmware, passes host tests against a fake driver; that its latch counter follows real hardware and the timing of latches are not tested (`docs/twoplane-backend.md`). No AM62P run is recorded here.
- **WP1 (`no_std` core):** the core builds for `thumbv7em-none-eabihf` and has host tests in CI; host costs are in `docs/host-metrics.md`. These are host results only.
- **WP2 / WP3 / WP4:** see M1, M2 and M3 above. Other SoCs have dated observations of their display topology (STM32MP257F-DK, RK3588, BCM2712, BCM2711 on 2026-11-28; the STM32MP257F-DK two-plane run on 2026-11-27, re-run 2026-11-28, in `docs/observed/`). These are not milestone results. The AM62P vs i.MX95 matrix is built from recorded docs only (`docs/am62p-imx95-matrix.md`).
- **WP5:** gap analysis, design notes, licence record, contribution and security documents exist; they are desk research and documentation. Demo and benchmark evidence are not produced (`docs/sow-traceability.md`).
