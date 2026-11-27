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
- docs: CI workflow (`.github/workflows/ci.yml`) running fmt, clippy, tests, the `thumbv7em-none-eabihf` builds and the aarch64 cross-build on pull requests and on `main` (2026-11-27, 2026-11-28; `docs/ci.md`).
- docs: `README.md` and `CONTRIBUTING.md` (2026-11-28).

### Changed

- composition-core: the capabilities refresh and the device-lost handling are kept off the stack of `commit` and `poll_event`; the engine and the present's plan are built in place, not on the stack (2026-11-21 to 2026-11-23).
- composition-ffi: capabilities are described in place, and the C engine is built in its storage (2026-11-22).
- composition-ffi, composition-ffi-staticlib: `composition-ffi` is an rlib; the static library is `composition-ffi-staticlib` (2026-11-21).
- composition-drm: `Imx95Dpu` profile apply is idempotent (2026-11-28).
- composition-core: `check_submit` reservation comment says the replaced and the new submission can both need room (2026-11-21).
- repository: the two-plane backend repository was merged into this one (2026-11-26); stray scratch gitlinks `ml-src` and `rk-src` were removed (2026-11-28).

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
