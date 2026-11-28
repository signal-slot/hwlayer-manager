# Changelog

All notable changes to this repository are recorded here, in the style of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). No tag or release
exists yet, so everything is under **Unreleased**. Dates are commit dates on
`main` (2026-11-21 to 2026-11-28), taken from `git log`.

Hardware results appear only as the linked documents record them, with their
date and tag (**observed** = measured on the board, **source** = read from
source, **inferred** = concluded). Host tests and cross-builds are not hardware
results. No milestone (M1, M2, M3) is met by anything listed here; see the
Status section of `README.md`.

## How to update this file

Every PR that changes behaviour, the C ABI, a script or a document adds one
line to **Unreleased**, under Added, Changed or Documentation, prefixed with the
crate or area (`composition-core`, `composition-fake`, `composition-drm`,
`composition-ffi`, `composition-ffi-staticlib`, `backends/twoplane`, `scripts`,
`docs`). Write the line from the commit subject, in one line. A hardware result
gets its date and tag and links the document that records it; do not state
anything the document does not. Never put board addresses, host names, users or
credentials here. Pure refactors with no visible effect need no line. At a
release, rename **Unreleased** to the version and date and open a new empty one.

## [Unreleased]

### Added

- composition-core: `no_std` portable core (planner, presentation state machine, capability description) with no dependencies (2026-11-21).
- composition-core: structured traces of engine events (2026-11-28).
- composition-core: a plan made stale by events queued before the commit is refused (2026-11-28).
- composition-core: a commit the backend accepted is no longer failed by a spurious latch (2026-11-28).
- composition-core: the latch a backend already reported before the replacing frame is recorded, and the latch of the submission being committed is held back until it is recorded (2026-11-21).
- composition-core: tests: planner soundness and completeness properties, fixed-point properties, capability description edge cases, model-based tests of the presentation state machine (2026-11-28).
- composition-fake: fake backend, with SoW §10 lifecycle scenario tests, adversarial event-order tests, seeded fuzz tests for malformed frame descriptions, and planner table tests (2026-11-21 to 2026-11-28).
- composition-fake: `plan_time` example that measures plane assignment time on the host (2026-11-28; host figures only, `docs/host-metrics.md`).
- composition-drm: DRM/KMS backend and `DriverProfile`, with edge-case tests of `DriverProfile::apply` (2026-11-21, 2026-11-28).
- composition-drm: `probe` example, with `--raw` to dump the raw KMS view and the prefer-shadow and atomic async page flip capabilities (2026-11-28).
- composition-drm: `twoplane` producer harness example; it reports TEST_ONLY rejections that ran into the search limit (2026-11-28).
- composition-drm-harness (`tools/drm-harness`): two-plane producer harness on a DRM master fd, with kernel lease, drm-lease-manager client and external producer modes (2026-11-28).
- composition-ffi: C ABI over the engine, with malformed-input tests (seeded generator) and an adversarial event-ordering test (2026-11-21, 2026-11-28).
- composition-ffi-staticlib: static library for C programs, split out of `composition-ffi`, which is now an rlib (2026-11-21).
- backends/twoplane: the two-plane C backend taken from the AM62P demo firmware, free of Zephyr (port table instead), host-tested, with adversarial event-ordering host tests (2026-11-26, 2026-11-28).
- scripts: `cross-aarch64.sh`, a static aarch64 Linux cross-build of the DRM backend, probe and harness (2026-11-28).
- scripts: `build-dlm-aarch64.sh`, which builds AGL drm-lease-manager for aarch64 with a self-bootstrapped sysroot (2026-11-28).
- scripts: `check-no-board-secrets.sh` and `check-no-farm-ids.sh`, which scan files and commit messages for board secrets and farm identifiers (2026-11-28).
- scripts: `imx95-session.sh` for short Verdin iMX95 board sessions, and `imx95-m7-rproc.sh`, a host-tested M7 remoteproc load/stop/restore script that has not been run on a board (2026-11-28).
- scripts: `camera-check.py`, a host checker for camera pictures of the two-plane patterns (2026-11-28).
- scripts: `vkms-vm-test.sh`, a QEMU kernel boot for the vkms tests, not part of CI.
- scripts: `check-doc-links.sh` (relative links in the Markdown files), `check-ffi-header.sh` (the C header moves with `composition-ffi` in the same commit), `check-zephyr-apps.sh` (layout of the Zephyr board-test apps) and `check-all.sh` (runs the CI checks of `docs/ci.md` locally), each with a host test script where one exists (2026-11-28).
- scripts: `check-m1-report.sh`, a structural lint of an M1 report against SoW §9 (the report itself is not in the repository) (2026-11-28).
- scripts: `prepare-zephyr.sh`, a pinned Zephyr workspace and SDK preparation script, described in `docs/zephyr-workspace.md` (2026-11-28).
- scripts: `probe-table.py`, which turns `probe --raw` output into `docs/platform-capabilities.md` tables (`scripts/probe-table-README.md`) (2026-11-28).
- tests/board: `imx95_m7_scanout`, an upstream-Zephyr i.MX95 M7 framebuffer scanout app that prints write timing; host builds only, not run on a board (2026-11-28; `tests/board/imx95_m7_scanout/README.md`).
- docs: `SECURITY.md` (private reporting, supported versions, scope and the FFI unsafe boundary), `NOTICE` and `docs/third-party-licences.md` (third-party licence record) (2026-11-28).
- docs: `.github/pull_request_template.md` and the issue template `.github/ISSUE_TEMPLATE/board-result.md` for reporting a dated board result (2026-11-28).
- docs: CI workflow (`.github/workflows/ci.yml`) running fmt, clippy, tests, the `thumbv7em-none-eabihf` builds and the aarch64 cross-build on pull requests and on `main` (2026-11-27, 2026-11-28; `docs/ci.md`).
- docs: `README.md` and `CONTRIBUTING.md` (2026-11-28).
- composition-core: `advise`, a stateless advisory planner entry point: same input, same advice, bounded work, per-layer `Blocker` reasons; host tests in `crates/composition-core/tests/advise.rs` and `crates/composition-fake/tests/`; design note `docs/advisory-planner-design.md` (status and §6 decisions, for the owner to revisit; the C form is not implemented) (2026-11-28; host build only).
- composition-core: host test pinning the plan of the M2 comparison scene (`tests/m2_scene.rs`) (2026-11-28).
- composition-fake: planner tests on the recorded BCM2711 HVS5, BCM2712 HVS6, RK3588 VOP2 and STM32MP257 LTDC topologies (including outputs sharing a pool and per-port plane reach), a seeded soak test of malformed frames under adversarial event order, and the MAX_PLANES cut (2026-11-28; host tests of recorded data, not board runs).
- composition-drm: `DriverProfile::Stm32Ltdc` (upscale bound from the STM32MP257F-DK TEST_ONLY measurement of 2026-11-28), `DriverProfile::Rk3588Vop2` (window class, scaling and source limits from the NanoPC-T6 observation of 2026-11-27 and mainline v6.18 source) and `Vc4Hvs5`/`Vc4Hvs6` (no-op skeletons) (2026-11-28; `docs/rk3588-vc4-profiles.md`).
- composition-drm: examples `props` (raw KMS plane, CRTC and connector properties), `kms_sweep` (plane combinations, modifiers, pending-commit replace), `scale_probe` (TEST_ONLY scaling bounds per plane), `writeback` (two-plane z-order and alpha check) and `plane_commit` (zpos, rotation and NV12 commits) (2026-11-28).
- composition-drm: tests on recorded probe data (loader, Generic planner on RK3588 and vc4 topologies, modifier, scaling, rotation, blend and zpos choices, format and plane cuts, shared planes), and a deterministic fuzz of `DriverProfile::apply` and the planner (2026-11-28; the data are dated records under `crates/composition-drm/tests/data/`).
- composition-drm: the backend keeps the planes that reach a probed output before truncating to the plane limit (2026-11-28).
- liftoff-crosscheck (`tools/liftoff-crosscheck`): a crate that compares the planner with libliftoff `c4226a7` on recorded topologies: xcheck reader, probe converter, fake-kernel TEST_ONLY model, C driver and fake libdrm, frame generator and sweep, calibration and section 7 frames; run by hand with `scripts/check-liftoff-crosscheck.sh` (2026-11-28; design in `docs/libliftoff-crosscheck.md`).
- backends/twoplane: i.MX95 DPU port tables as an additive module (`src/imx95.rs`), and Arm link files and a start-up stub for Cortex-M7 and Cortex-R5 compile-and-link checks (2026-11-28; host and compile checks only).
- scripts: `check-arm-link.sh`, which compiles and links the twoplane backend for Cortex-M7 and Cortex-R5 with GCC and clang on the host; CI gains an `arm-backend` job running the compile (2026-11-28).
- scripts: `build-zephyr-apps.sh`, which west-builds every Zephyr app in the repository, with a mocked host test (2026-11-28).
- scripts: board-data helpers, each with a host test: `check-test-data.sh` (dated board data folders), `check-fixture-provenance.sh` (recorded fixtures in test files), `check-profile-tags.sh` (evidence tags in `DriverProfile` docs), `scrub-kms-dump.py`, `kms-dump-diff.py`, `decode-modifiers.py`, `format-cut-report.py`, `sweep-summary.py`, `sweep-to-fixture.py`, `profile-check-expect.py`, `twoplane-expect.py`, `alpha-mode-expect.py`, `harness-summary.py` and `board-run-bundle.sh` (2026-11-28; see the `*-README.md` next to each).
- scripts: `board-scenarios.sh`, which runs SoW §10 rows A1-A5 with the drm harness (2026-11-28; `docs/linux-board-scenarios.md`).
- scripts: `check-all.sh` also runs the script self-tests and the fixture-provenance lint (2026-11-28).
- tests/board: `imx95_m7`, an upstream-Zephyr i.MX95 M7 hello and read-only DPU probe app (host build only, not run on a board), and `imx95_m7_engine`, which links the engine static library (2026-11-28).
- tests/zephyr: `display_dummy`, the engine over Zephyr's dummy display driver in QEMU (not hardware) (2026-11-28).
- composition-drm (`tests/data`): dated read-only KMS dumps and TEST_ONLY sweeps recorded from the BCM2711 (CM4), BCM2712 (Raspberry Pi 5) and RK3588 (NanoPC-T6) boards, each with a README giving date and source (2026-11-27, 2026-11-28).
- docs: `docs/testing.md`, `docs/architecture.md`, `docs/msrv.md` (host measurement of 2026-11-28 of the minimum Rust version against the declared `rust-version`; says nothing about a board) (2026-11-28).
- scripts: `host-demo.sh`, a host-only reproducible demo (builds, fake-backend scene tests, `plan_time`, the two thumbv7em builds), with `host-demo.test.sh` and `docs/host-demo.md`; a host run, not a hardware result (2026-11-28).
- scripts: host tests `prepare-zephyr.test.sh` (dry run and argument handling) and `imx95-session.test.sh` (fake ssh) (2026-11-28).
- scripts: `imx95-observe.sh`, a read-only board snapshot script (remoteproc, device tree, dmesg, clk and genpd), and `imx95-observe-summary.py`, which turns a snapshot into a short ownership table; each with a host test; `imx95-m7-log.sh`, which captures the M7 rpmsg or trace log under `/tmp`, with a host test (2026-11-28; the host tests are not board results).
- scripts: `trace-to-metrics.py` converts `docs/trace-format.md` event logs into `m2-compare-report.py` METRIC lines, with an optional `--refresh-us N` from which the outlier and late-latch counts are derived; `RUN` lines with `alpha=` are variant runs and are skipped, not reported as the scene (`scripts/trace-to-metrics-README.md`; 2026-11-28, host only).
- scripts: `m2-compare-report.py`, which builds the WP3 Linux vs Zephyr comparison table from run logs, and `check-m2-acceptance.sh`, which runs the M2 acceptance checks mechanically; host tests `m2-pipeline.test.sh` and `m2scene-trace.test.sh` run trace logs through `trace-to-metrics.py` and the report (2026-11-28; host only, no measurement).
- scripts: `check-trace-log.sh`, which checks trace log headers against `docs/trace-format.md` (version, `# date:`, not-a-hardware-result comment), with a host test (2026-11-28).
- scripts: `check-feasibility-doc.sh` (tags, dates and pinned sources of `docs/imx95-zephyr-feasibility.md`) and `check-doc-tags.sh` (evidence tags in hardware docs), each with a host test (2026-11-28).
- composition-drm: `m2scene` example, the WP3 comparison scene on Linux, emitting `docs/trace-format.md` events; usage in `docs/m2-linux-scene.md` (2026-11-28; no board run recorded here).
- tests/board: `README.md`, an index of the board test apps (2026-11-28).

### Changed

- composition-core: the capabilities refresh and the device-lost handling are kept off the stack of `commit` and `poll_event`; the engine and the present's plan are built in place, not on the stack (2026-11-21 to 2026-11-23).
- composition-ffi: capabilities are described in place, and the C engine is built in its storage (2026-11-22).
- composition-ffi, composition-ffi-staticlib: `composition-ffi` is an rlib; the static library is `composition-ffi-staticlib` (2026-11-21).
- composition-drm: `Imx95Dpu` profile apply is idempotent (2026-11-28).
- composition-core: `check_submit` reservation comment says the replaced and the new submission can both need room (2026-11-21).
- repository: the two-plane backend repository was merged into this one (2026-11-26); stray scratch gitlinks `ml-src` and `rk-src` were removed (2026-11-28).
- composition-core: the planner refuses a huge flipped destination or gigantic source instead of overflowing in `clip_to_output`, and a `Zpos::Range` with `min > max` describes no usable position; each pinned by a regression test (2026-11-28).
- backends/twoplane: a refused enable or safety-flip write returns `CE_ERR_BACKEND_REJECTED` and leaves state unchanged, so a later frame retries (2026-11-28).
- composition-drm: the `twoplane` example prints the swapped EXPECT block only when the swap runs; `Stm32Ltdc` `max_upscale` is 1080 from the measured ceiling (2026-11-28).
- scripts: `cross-aarch64.sh` gains a private rustup and rust-lld fallback and a file check of its outputs; the secret scans exit 0 on the tip and still flag real secrets (2026-11-28).

### Documentation

- docs: `platform-capabilities.md` and the sources under `docs/observed/`: topologies read from source for VC4/HVS (BCM2711, BCM2712, rpi-6.18.y), RK3588 VOP2 (vendor kernel) and STM32MP257 LTDC (ST kernel, partial), each tagged **source** (2026-11-28).
- docs: observed display topology on 2026-11-28 for BCM2712 (Raspberry Pi 5), BCM2711 (CM4, reTerminal) and RK3588 (NanoPC-T6), tagged **observed**; two-plane runs recorded for BCM2711 (CM4), BCM2712 and STM32MP257F-DK (STM32MP257F-DK observed 2026-11-27, re-run 2026-11-28). Physical output was not verified on the BCM2711 and BCM2712 boards (no KVM) (`docs/observed/`).
- docs: i.MX95 DPU driver read from NXP lf-6.6.52-2.2.2 and compared with the 6.18 reading, tagged **source** (`docs/imx95-dpu-source-6.6.md`).
- docs: M1 test plan for i.MX95 Linux DRM (`docs/m1-imx95-test-plan.md`); the plan's results are not recorded.
- docs: M2 Linux vs Zephyr comparison plan (`docs/m2-comparison-plan.md`); the mapping of the engine backend contract onto Zephyr for the i.MX95 M7 (`docs/imx95-m7-backend-contract.md`).
- docs: read-only snapshot of the Verdin iMX95 remoteproc, device tree, SCMI and clock state, **observed** 2026-11-27 (board UTC date), in `docs/imx95-zephyr/`.
- docs: i.MX95 board rules (`docs/imx95-board.md`); documented aarch64 cross-build and the dlm build record (`docs/dlm-aarch64.md`: built on the host 2026-11-28, nothing run on a board).
- docs: WP5 gap analysis of leased output vs Weston-owned output and libliftoff overlap, from read sources (`docs/wp5-gap-analysis.md`).
- docs: AM62P vs i.MX95 difference matrix from the recorded docs (`docs/am62p-imx95-matrix.md`); SoW traceability (`docs/sow-traceability.md`); open issues after M1 (`docs/open-issues.md`).
- docs: design note on fences and readiness across core, DRM backend and C ABI (`docs/sync-fences.md`); host metrics (`docs/host-metrics.md`, host only); the two-plane backend and what a board test still needs, including the by-hand Arm compiles of 2026-11-26 (`docs/twoplane-backend.md`).
- docs: i.MX95 Zephyr feasibility, a source-based draft (`docs/imx95-zephyr-feasibility.md`); an inferred design of an SM logical-machine configuration giving the display to the M7, with its imx-sm citations pinned (`docs/imx95-sm-display-lm.md`); a source study of the DPU-to-LVDS route for an M7 driver (`docs/imx95-m7-lvds-route.md`).
- docs: i.MX95 M7 source studies and design notes: BSP facts (`docs/imx95-m7-verdin-bsp.md`), SCMI reach for display clocks and power domains at Zephyr v4.5.0 (`docs/imx95-m7-scmi.md`), framebuffer memory, cache and MPU rules (`docs/imx95-m7-fb-memory.md`), a two-plane overlay design (`docs/imx95-m7-overlay-design.md`), a log channel over remoteproc trace and rpmsg (`docs/imx95-m7-rpmsg-log.md`) and filling the twoplane tables for the DPU (`docs/imx95-twoplane-port.md`); these are design and source studies, not hardware results.
- docs: Weston on the i.MX95 image and DRM leases, a source read (`docs/weston-imx-lease.md`); design notes for a planner-only advisory entry point (`docs/advisory-planner-design.md`) and for cross-checking the planner against libliftoff (`docs/libliftoff-crosscheck.md`), both proposals.
- docs: catalogue of SoW §10 board scenarios, design only (`docs/board-scenarios.md`); glossary of engine, DRM, Zephyr and i.MX95 terms (`docs/glossary.md`).
- docs: M1 test report for the Verdin iMX95 with a verdict per acceptance condition (`docs/m1-imx95-report.md`): it records an overall M1 verdict of passed with stated gaps, from the master-fd and lease runs observed 2026-11-27 and the probe of 2026-11-28 (`docs/m1-imx95/master-run.md`, `docs/m1-imx95/lease-run.md`). The board is a Verdin iMX95 on a Mallow carrier, not an EVK; whether it counts as equivalent is the requester's call.
- docs: i.MX95 Zephyr feasibility finished with the board snapshot; the remoteproc gate decided and the M7 run recorded as skipped by the gate, so no M7 result exists (`docs/imx95-zephyr-feasibility.md`). Pinned reads of the imx-sm and Toradex kernel nodes (`docs/imx95-verdin-bsp-src.md`); open issues updated (`docs/open-issues.md`).
- docs: i.MX95 M7 source studies, tagged **source**: console UART (`docs/imx95-m7-console.md`), remoteproc ELF load path (`docs/imx95-m7-rproc-load.md`), upstream Zephyr DPU driver scanout, latch and VSYNC path (`docs/imx95-m7-dpu-driver.md`), a design for measuring latch timing (`docs/imx95-m7-latch-measure.md`), what a Zephyr board for the Verdin M7 would need (`docs/imx95-verdin-zephyr-board.md`), display clocks and power domains under the System Manager (`docs/imx95-display-clocks.md`, `docs/imx95-display-power.md`), and NXP MCUXpresso display examples (`docs/imx95-mcux-display.md`, line citations pinned where they could be fetched).
- docs: STM32MP257 LTDC read from the ST kernel and compared with mainline v6.18 and ST's 6.6.129 (`docs/stm32mp257-ltdc-source-full.md`, `docs/stm32mp257-ltdc-sync-source.md`, `docs/stm32mp257-ltdc-source-diff-6.6.md`), per-plane upscale bounds and scenarios A1-A5 observed on the STM32MP257F-DK (2026-11-28; `docs/stm32mp257-ltdc-board.md`, `docs/stm32mp257-board-scenarios.md`); KVM capture attempts gave no picture (`docs/observed/stm32mp257-kvm.md`).
- docs: RK3588 VOP2 read from source, tagged **source**: mainline 6.18, the vendor kernel and the device tree (`docs/rk3588-mainline-source.md`, `docs/rk3588-vendor-vop2-source.md`, `docs/rk3588-dt-source.md`, `docs/rk3588-vop2-limits-source.md`, `docs/rk3588-vop2-formats-source.md`, `docs/rk3588-vop2-scale-source.md`).
- docs: Raspberry Pi vc4 HVS5/HVS6 read from source, tagged **source**: mainline v6.18 against rpi-6.18.y, dlist memory, load and core clock, LBM/UPM pools, scaling, formats and modifiers, plane properties (`docs/vc4-*-source.md`, `docs/bcm2712-dt-source.md`).
- docs: `docs/drm-lease-source.md` (what a DRM lessee sees and TEST_ONLY, mainline v6.18) and `docs/drm-pending-commit-source.md` (pending-commit replacement and flip events in DRM core, vc4 and VOP2), both read from source.
- docs: design notes for backends for the RK3588 VOP2, the Raspberry Pi HVS and the STM32MP257 LTDC (`docs/design/`), and a draft of RK3588 and vc4 profiles (`docs/rk3588-vc4-profiles.md`); proposals, not hardware results.
- docs: Linux board scenarios A1-A5 for the Raspberry Pi 5 and NanoPC-T6, with results where run and a reason for each row not measured (`docs/linux-board-scenarios.md`); board session procedure for the NanoPC-T6, Raspberry Pi 5 and reTerminal (`docs/rk3588-vc4-board-procedure.md`).
- docs: `docs/platform-capabilities.md` gains BCM2712, BCM2711 and STM32MP257 sections from the board records; `docs/wp5-gap-analysis.md`, `docs/twoplane-backend.md`, `docs/third-party-licences.md` (verified against `Cargo.lock`) and `docs/ci.md` updated; README links the new i.MX95 Zephyr, WP5 and board-test docs (2026-11-28).
- docs: `docs/trace-format.md` requires a `# date:` comment line in real trace logs (section 5.1); `docs/wp5-evidence.md`, an evidence checklist per milestone for the WP5 demo and benchmark; `docs/m2-linux-zephyr-comparison.md`, the method of the WP3 comparison, with no measurement (2026-11-28).
- docs: `docs/imx95-m7-runbook.md`, how a board item uses `imx95-observe.sh` and `imx95-m7-rproc.sh`; `docs/imx95-verdin-sm-config.md` (the Verdin System Manager configuration, pinned) and `docs/imx95-sm-linux-interfaces.md` (what the 6.6 kernel shows userspace about the System Manager), both read from source; the dated snapshots are under `docs/imx95-zephyr/` (2026-11-28).
