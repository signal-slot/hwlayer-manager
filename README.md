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

### Index by topic

Every remaining file and directory under `docs/`, grouped by topic. Descriptions are the documents' own titles; the evidence tags inside each document apply.

#### i.MX95 M2 (additional)

- [`docs/imx95-m7-runbook.md`](docs/imx95-m7-runbook.md): M7 board runbook (observe and remoteproc start/stop).
- [`docs/imx95-m7-memory-map.md`](docs/imx95-m7-memory-map.md): Cortex-M7 memory map and where a Zephyr scanout buffer can live (source study).
- [`docs/imx95-m7-ldb-driver.md`](docs/imx95-m7-ldb-driver.md): LDB / LVDS driver for Zephyr on the M7 (design note, M2-10).
- [`docs/imx95-dpu-scmi-ids.md`](docs/imx95-dpu-scmi-ids.md): SCMI clock, power-domain and reset IDs of the display pipeline.
- [`docs/imx95-verdin-sm-config.md`](docs/imx95-verdin-sm-config.md): the Verdin System Manager configuration, pinned.
- [`docs/imx95-sm-linux-interfaces.md`](docs/imx95-sm-linux-interfaces.md): what NXP's 6.6 kernel shows userspace about the System Manager.
- [`docs/imx95-verdin-lvds-panel.md`](docs/imx95-verdin-lvds-panel.md): 10.1" LVDS panel overlay, timings and LDB setup (source study).
- [`docs/m2-linux-scene.md`](docs/m2-linux-scene.md): the M2 scene on Linux, the `m2scene` example.
- [`docs/m2-linux-zephyr-comparison.md`](docs/m2-linux-zephyr-comparison.md): how the Linux vs Zephyr comparison will be run.
- [`docs/wp4-difference-matrix.md`](docs/wp4-difference-matrix.md): WP4 draft AM62P vs i.MX95 difference matrix.
- [`docs/m1-imx95/`](docs/m1-imx95/): M1 run material for the i.MX95 (logs, pictures, checks).

#### AM62P

- [`docs/am62p-dss-source.md`](docs/am62p-dss-source.md): display subsystem (DSS), Linux tidss, source reading.
- [`docs/am62p-dt-source.md`](docs/am62p-dt-source.md): Verdin AM62P device tree: DSS, panels and R5F carve-outs, read from source.
- [`docs/am62p-r5f-remoteproc-source.md`](docs/am62p-r5f-remoteproc-source.md): MCU R5F under Linux remoteproc, from source.
- [`docs/am62p-torizon-boot-source.md`](docs/am62p-torizon-boot-source.md): Torizon OS boot chain and R5F firmware loading (source study).
- [`docs/board-test-am62p-r5f.md`](docs/board-test-am62p-r5f.md): board test: the engine on the MCU R5F.
- [`docs/m3-am62p-test-plan.md`](docs/m3-am62p-test-plan.md): M3 test plan, AM62P portability regression.
- [`docs/m3-03-board-test-design.md`](docs/m3-03-board-test-design.md): M3-03 design of the safety-flip and producer-failure board tests.

#### RK3588

- [`docs/rk3588-backend-design.md`](docs/rk3588-backend-design.md): RK3588 (VOP2) backend design.
- [`docs/design/`](docs/design/): per-SoC backend design notes (`hvs-backend.md`, `rk3588-backend.md`, `stm32mp257-backend.md`).
- [`docs/rk3588-dt-source.md`](docs/rk3588-dt-source.md): per-port plane reach, from the mainline driver.
- [`docs/rk3588-mainline-source.md`](docs/rk3588-mainline-source.md): VOP2 in mainline Linux 6.18, from source.
- [`docs/rk3588-vendor-vop2-source.md`](docs/rk3588-vendor-vop2-source.md): Rockchip vendor kernel 6.1 compared with mainline 6.18.
- [`docs/rk3588-vop2-formats-source.md`](docs/rk3588-vop2-formats-source.md): per-window-class formats, modifiers and transforms.
- [`docs/rk3588-vop2-limits-source.md`](docs/rk3588-vop2-limits-source.md): input, line-buffer and scaling limits.
- [`docs/rk3588-vop2-scale-source.md`](docs/rk3588-vop2-scale-source.md): scaling per window class.
- [`docs/rk3588-vc4-profiles.md`](docs/rk3588-vc4-profiles.md): `DriverProfile` variants for RK3588 and Raspberry Pi.
- [`docs/rk3588-vc4-board-procedure.md`](docs/rk3588-vc4-board-procedure.md): board session procedure for NanoPC-T6, Raspberry Pi 5 and reTerminal.

#### BCM27xx / HVS

- [`docs/hvs-backend-design.md`](docs/hvs-backend-design.md): engine backend for the Raspberry Pi HVS (BCM2711, BCM2712).
- [`docs/bcm2712-dt-source.md`](docs/bcm2712-dt-source.md): BCM2712 and BCM2711 CRTC and plane arithmetic from the device tree and vc4 source.
- [`docs/vc4-mainline-source.md`](docs/vc4-mainline-source.md): HVS5/HVS6 plane code, mainline v6.18 against rpi-6.18.y.
- [`docs/vc4-hvs-dlist-source.md`](docs/vc4-hvs-dlist-source.md): dlist memory and per-plane dlist size.
- [`docs/vc4-hvs-format-source.md`](docs/vc4-hvs-format-source.md): format and modifier acceptance.
- [`docs/vc4-hvs-lbm-upm-source.md`](docs/vc4-hvs-lbm-upm-source.md): LBM and UPM pools, per-plane cost and plane counts.
- [`docs/vc4-hvs-load-source.md`](docs/vc4-hvs-load-source.md): load and core-clock computation.
- [`docs/vc4-hvs-props-source.md`](docs/vc4-hvs-props-source.md): plane properties (zpos, alpha, blend, rotation, colour).
- [`docs/vc4-hvs-scaling-source.md`](docs/vc4-hvs-scaling-source.md): scaling ratios, LBM/UPM sizing, modifier checks and dlist words.

#### STM32MP257

- [`docs/stm32mp257-ltdc-board.md`](docs/stm32mp257-ltdc-board.md): LTDC z-order and alpha through the KVM, and scaling bounds.
- [`docs/stm32mp257-board-scenarios.md`](docs/stm32mp257-board-scenarios.md): board scenarios A1-A5.
- [`docs/stm32mp257-ltdc-session-2026-11-28.md`](docs/stm32mp257-ltdc-session-2026-11-28.md): board sessions A and B, 2026-11-28.
- [`docs/stm32mp257-ltdc-session-c-2026-11-28.md`](docs/stm32mp257-ltdc-session-c-2026-11-28.md): board session C, 2026-11-28.
- [`docs/stm32mp257-ltdc-session-c-outputs-2026-11-28.md`](docs/stm32mp257-ltdc-session-c-outputs-2026-11-28.md): session C, other outputs and scaler layers per variant.
- [`docs/stm32mp257-session-c-runbook.md`](docs/stm32mp257-session-c-runbook.md): session C runbook for the picture sessions.
- [`docs/stm32mp257-ltdc-source-full.md`](docs/stm32mp257-ltdc-source-full.md): full LTDC source read.
- [`docs/stm32mp257-ltdc-source-diff-6.6.md`](docs/stm32mp257-ltdc-source-diff-6.6.md): ST v6.18 source rows against ST's 6.6 kernel.
- [`docs/stm32mp257-ltdc-sync-source.md`](docs/stm32mp257-ltdc-sync-source.md): sync semantics, from source.
- [`docs/stm32mp257-ltdc-transform-source.md`](docs/stm32mp257-ltdc-transform-source.md): reflect and rotation programming, with predicted pictures.
- [`docs/stm32mp257-ltdc-yuv-source.md`](docs/stm32mp257-ltdc-yuv-source.md): YCbCr-to-RGB conversion, from source.

#### i.MX8MP

- [`docs/imx8mp-lcdif-source.md`](docs/imx8mp-lcdif-source.md): LCDIFv3 display path, read from source (SoW §5.3).
- [`docs/imx8mp-route3-precomposition.md`](docs/imx8mp-route3-precomposition.md): Route 3 pre-composition and buffer ownership (design note).
- [`docs/imx8mp-x2-board.md`](docs/imx8mp-x2-board.md): Armadillo-X2, DRM/KMS as the kernel exposes it.

#### Cross-cutting

- [`docs/drm-lease-source.md`](docs/drm-lease-source.md): what a DRM lessee can see and `TEST_ONLY` (mainline 6.18).
- [`docs/drm-pending-commit-source.md`](docs/drm-pending-commit-source.md): pending-commit replacement and flip events in DRM core, vc4 and VOP2.
- [`docs/linux-board-scenarios.md`](docs/linux-board-scenarios.md): Linux board scenarios A1-A5 on boards other than the i.MX95.

#### WP5

- [`docs/wp5-evidence.md`](docs/wp5-evidence.md): evidence checklist, what the SoW asks for and what exists.
- [`docs/wp5-coexistence-survey.md`](docs/wp5-coexistence-survey.md): coexistence survey of three Linux DRM boards.
- [`docs/weston-release-read.md`](docs/weston-release-read.md): released Weston DRM backend plane assignment, from source.
- [`docs/liftoff-crosscheck-report.md`](docs/liftoff-crosscheck-report.md): cross-check of the planner against libliftoff (WP5-04).
- [`docs/host-demo.md`](docs/host-demo.md): host demo.

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
