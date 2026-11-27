# SoW traceability: deliverables and milestone criteria to repository evidence

Written 2026-11-28 and updated 2026-11-28 to the repository at commit `a88ae19`. It covers every deliverable named in `SoW.md` §8 (WP0 to WP5) and every acceptance condition of §9 (M1 to M3), and lists the files that address each one. It adds no result of its own. Every status comes from what another document of this repository records, and that document is named in the row. Where this file and the named document disagree, the named document is right.

Status values:

- **observed &lt;date&gt;**: a document of this repository records a board observation made on that date.
- **host only**: the item exists in the repository and was checked on a development host only, by build, host tests, desk reading or vkms (a virtual kernel driver, not hardware). No board evidence exists.
- **planned**: files, a test plan or an assigned plan item exist, but the result does not.
- **not started**: nothing in the repository addresses it yet.

At this commit one document records a board observation of the i.MX95: `docs/imx95-zephyr/observed-2026-11-27.txt`, a read-only snapshot (remoteproc, device tree, SM/SCMI dmesg, clocks, power domains) of the Verdin iMX95. It says what Linux holds today. It is not a scanout, VSYNC, latch or composition result, so it meets no milestone condition. No document records an AM62P board observation. The other dated board observations are on other SoCs (`docs/observed/`), which are outside the milestones (`docs/open-issues.md` X-07).

**M2 work from plan #455 is partly on `main` at this commit**: the source studies and design notes listed in the WP3 and M2 rows, `docs/imx95-zephyr-feasibility.md` (a draft from source reading; the rows below say what it does not yet settle) and the scanout application `tests/board/imx95_m7_scanout/` (host build only, never run on hardware). Not at this commit: `tests/board/imx95_m7/`, `tests/board/imx95_m7_engine/` and `docs/imx95-m7-dpu-driver.md`. Rows that name those paths say "not at this commit" and stay **planned**. M2 is open: no document records a scanout, VSYNC or latch result from the M7.

## Work package deliverables (SoW §8)

| WP | Deliverable | Evidence in the repository | Status |
| --- | --- | --- | --- |
| WP0 | Architecture note | `SoW.md` §3 (architecture as proposed); module docs of `crates/composition-core/src/lib.rs`, `planner.rs`, `presentation.rs`, `engine.rs`; `crates/composition-drm/src/lib.rs` module doc. No separate architecture note exists | host only |
| WP0 | Platform capability matrix | `docs/platform-capabilities.md` (i.MX95 Linux and Zephyr, drm-lease-manager, vkms; AM62P "Not researched yet"); `docs/imx95-dpu-source-6.6.md` (i.MX95 kernel 6.6 driver, "Nothing here is observed on a board"); `crates/composition-drm/src/profile.rs` (`DriverProfile::Imx95Dpu`, from source); `docs/observed/` (BCM2711, BCM2712, RK3588, STM32MP257, VC4/HVS) | i.MX95 and AM62P: host only (source). Other SoCs, outside the milestones: observed 2026-11-27 (STM32MP257 two-plane run, `docs/observed/stm32mp257.md`) and observed 2026-11-28 (topology of BCM2711, BCM2712, RK3588, STM32MP257) |
| WP0 | Test scenarios | `docs/m1-imx95-test-plan.md` (M1 conditions to scenarios; §10 maps the five WP0 scenarios: two UI layers and unsupported geometry covered, video + UI, priority layer and producer crash/restart "Later"); `tools/drm-harness/src/scenarios.rs`; host scenario tests in `crates/composition-fake/tests/` | planned (board scenarios); host only (fake-backend scenarios) |
| WP0 | Initial API proposal | `SoW.md` §3.3; `crates/composition-core/src/backend.rs`, `types.rs`, `caps.rs`; `crates/composition-ffi/include/composition_engine.h` | host only |
| WP1 | Rust library | `crates/composition-core` (`no_std`, `#![forbid(unsafe_code)]`), `crates/composition-fake`, `crates/composition-ffi`, `crates/composition-ffi-staticlib`; CI builds the core and the static library for `thumbv7em-none-eabihf` (`docs/ci.md`) | host only |
| WP1 | C-facing headers | `crates/composition-ffi/include/composition_engine.h`; `backends/twoplane/ce_twoplane.h`; C tests in `crates/composition-ffi-staticlib/tests/c/` and `backends/twoplane/tests/c/` | host only |
| WP1 | API docs | Rustdoc module and item docs in the crates above; `docs/twoplane-backend.md`; `README.md`; `docs/glossary.md` | host only |
| WP1 | Unit tests | `crates/composition-core/tests/` (property tests: caps, fixed point, planner, presentation model); `crates/composition-fake/tests/` (engine, event order, lifecycle, malformed frames, rotation, trace); `crates/composition-ffi/tests/`, `crates/composition-ffi-staticlib/tests/`, `backends/twoplane/tests/`; run by `cargo test --workspace` in CI (`docs/ci.md`) | host only |
| WP1 | Test vectors | Seeded fuzz and adversarial cases: `crates/composition-fake/tests/malformed.rs`, `crates/composition-ffi/tests/malformed.rs`, `backends/twoplane/tests/adversarial.rs`, `crates/composition-ffi-staticlib/tests/c/event_order.c`. The fake backend (`crates/composition-fake/src/lib.rs`) simulates delayed VSYNC, failed commits, injected events and device loss, as WP1 asks | host only |
| WP2 | i.MX95 Linux backend | `crates/composition-drm` (accepts any fd, device node or lease; TEST_ONLY check, atomic commit, events); `crates/composition-drm/tests/vkms.rs` and `scripts/vkms-vm-test.sh` (vkms, outside CI, `docs/ci.md`); aarch64 cross-build in CI (compiles and links only) | host only (vkms); no i.MX95 run recorded |
| WP2 | Capability report | `crates/composition-drm/examples/probe.rs`; `docs/m1-imx95-test-plan.md` §4; the report `docs/m1-imx95-report.md` does not exist at this commit (`docs/open-issues.md`) | planned |
| WP2 | AGL integration example | `tools/drm-harness` (`--lease dlm`, `lessor`); `docs/dlm-aarch64.md` and `scripts/build-dlm-aarch64.sh` ("no binary has been produced", 2026-11-28); `docs/m1-imx95-test-plan.md` §8 | planned |
| WP2 | Automated validation where possible | `.github/workflows/ci.yml`, `docs/ci.md` (host tests, `thumbv7em` builds, aarch64 cross-build; "builds and runs nothing on a board"); `scripts/vkms-vm-test.sh`; `scripts/camera-check.py`, `scripts/camera-check-README.md` | host only |
| WP2 | Captured logs and measured results | `docs/m1-imx95-test-plan.md` §9, §11 (evidence directory and order of runs); `tools/drm-harness` record format (`tools/drm-harness/README.md` "Output"); `docs/host-metrics.md` (host plane-assignment time, "Nothing here is a hardware result") | planned (board); host only (`docs/host-metrics.md`) |
| WP3 | i.MX95 Zephyr adapter or documented blocker | `docs/platform-capabilities.md` "i.MX95, Zephyr" (source; ownership "not found"); `docs/open-issues.md` M2-01 to M2-07; `docs/imx95-zephyr-feasibility.md` (draft, source reading; the Toradex kernel and SM sources were not read); `docs/imx95-zephyr/README.md` and `docs/imx95-zephyr/observed-2026-11-27.txt` (Linux-side snapshot); source studies `docs/imx95-sm-display-lm.md` (SM logical-machine design, inferred, cannot be applied here), `docs/imx95-m7-verdin-bsp.md`, `docs/imx95-m7-scmi.md`, `docs/imx95-m7-lvds-route.md`, `docs/imx95-m7-fb-memory.md`, `docs/imx95-m7-rpmsg-log.md`; design `docs/imx95-m7-overlay-design.md`, `docs/imx95-m7-backend-contract.md`, `docs/imx95-twoplane-port.md`; `docs/zephyr-workspace.md`, `scripts/prepare-zephyr.sh`, `scripts/imx95-m7-rproc.sh` (host-tested, never run on a board). `docs/imx95-m7-dpu-driver.md` is not at this commit | host only (source and design); observed 2026-11-27 (Linux-side snapshot only); the adapter or the blocker report is not concluded |
| WP3 | Firmware example | `tests/board/imx95_m7_scanout/` (scanout app with latch timing hooks; host build only, "nothing here has been run on hardware"); `scripts/check-zephyr-apps.sh` (layout lint of these apps); `backends/twoplane` (C backend taken from the AM62P demo, host-tested). `tests/board/imx95_m7/` and `tests/board/imx95_m7_engine/` are not at this commit | host only (scanout app build); planned (the others) |
| WP3 | Hardware test report and traces | None. `docs/open-issues.md` M2-04 (no VSYNC/latch under Zephyr verified) | not started |
| WP3 | Linux vs Zephyr comparison on the same board and scene (WP3 bullet 3) | `docs/m2-comparison-plan.md` (plan only, "holds no measurement"); `docs/board-scenarios.md`; `docs/open-issues.md` M2-07 | planned |
| WP4 | AM62P adapter | `backends/twoplane` (`ce_twoplane.h`, `ce_twoplane.c`, `src/lib.rs`), `docs/twoplane-backend.md` (host tests; compiled by hand for `thumbv7em-none-eabihf` and `armv7r-none-eabihf` on 2026-11-26, never linked or run) | host only |
| WP4 | Regression report | Board run is issue #12 (`docs/open-issues.md` M3-01); no report in the repository | planned |
| WP4 | Cross-SoC portability report | `docs/am62p-imx95-matrix.md` (from recorded docs, no new finding); `docs/open-issues.md` M3-04 | host only (desk); no board data for the AM62P column |
| WP4 | Optional third-target prototype | None. `docs/open-issues.md` M3-06 ("not started"; optional until i.MX95 is demonstrated) | not started |
| WP5 | Gap analysis | `docs/wp5-gap-analysis.md` (desk analysis of libliftoff `c4226a7` and Weston `c8d00e2`, "Nothing here is measured, and nothing was run on a board"; open issues W5-01 to W5-11); `docs/open-issues.md` WP5-01 to WP5-09 | host only (desk) |
| WP5 | Upstreaming proposal | None. `docs/wp5-gap-analysis.md` W5-08 (justification still to be written) and §4 "Options"; design notes `docs/advisory-planner-design.md`, `docs/libliftoff-crosscheck.md`, `docs/weston-imx-lease.md` (desk, nothing run) | not started (proposal); host only (design notes) |
| WP5 | Minimal documentation | `README.md` (what the engine is, crate map, host build and test, links to `SoW.md` and `docs/`); `docs/glossary.md`; `CHANGELOG.md` (history, all under Unreleased); `docs/ci.md`; `docs/zephyr-workspace.md`; `docs/board-scenarios.md`; this file; the `docs/` files above. `scripts/check-doc-links.sh` checks the relative links of the Markdown files | host only |
| WP5 | Reproducible demo | None from a clean checkout. Cross-build recipes in `docs/imx95-board.md`, `docs/dlm-aarch64.md`, `scripts/cross-aarch64.sh`; depends on `docs/m1-imx95-report.md` (`docs/wp5-gap-analysis.md` W5-11, `docs/open-issues.md` WP5-08) | not started |
| WP5 | Benchmark evidence | `docs/host-metrics.md`, `crates/composition-fake/examples/plan_time.rs` (host plane-assignment time, 2026-11-28); board figures depend on M1 (`docs/open-issues.md` X-04, WP5-06) | host only (host metrics); not started (board) |
| WP5 | Project licensing | `LICENSE` (Apache-2.0); `NOTICE`; `docs/third-party-licences.md` (licences of dependencies and named build inputs; fit with Apache-2.0 is "inferred, not legal advice"; `docs/wp5-gap-analysis.md` §5, W5-07). Completeness against `Cargo.lock` is still being checked by other plan #455 items | host only (desk record) |
| WP5 | Contribution guidelines | `CONTRIBUTING.md`; `SECURITY.md` (reporting policy, FFI unsafe boundary); `.github/pull_request_template.md`; `.github/ISSUE_TEMPLATE/board-result.md` (dated board result report); `scripts/check-all.sh`, `scripts/check-ffi-header.sh`, `scripts/check-no-board-secrets.sh`, `scripts/check-no-farm-ids.sh`; `docs/wp5-gap-analysis.md` W5-07 (sign-off policy fitting Weston's DCO and AI rules still to decide) | host only (documents and scripts); sign-off policy not decided |

## Milestone acceptance criteria (SoW §9)

### M1: i.MX95 Linux DRM / AGL hardware composition

`docs/m1-imx95-test-plan.md` maps each condition to scenarios and verdict rules (§12). Its results go into `docs/m1-imx95-report.md`, which does not exist at this commit, so no condition has a board verdict.

| # | Condition (short) | Evidence in the repository | Status |
| --- | --- | --- | --- |
| M1-1 | Core builds `no_std`; host tests and Linux i.MX95 builds pass | `crates/composition-core` (`#![no_std]`); `.github/workflows/ci.yml`, `docs/ci.md` (`thumbv7em-none-eabihf` builds; aarch64 cross-build of `composition-drm`, `probe` and `drm-harness`, compile and link only); `scripts/cross-aarch64.sh`; test plan §3 | host only |
| M1-2 | i.MX95 topology and plane restrictions captured from the target board and driver | `docs/platform-capabilities.md` and `docs/imx95-dpu-source-6.6.md` (source only); `crates/composition-drm/examples/probe.rs`; `crates/composition-drm/src/profile.rs`; test plan §4 | planned |
| M1-3 | Two distinct hardware planes concurrently, verified z-order and alpha; no flattened output | `tools/drm-harness` (scenarios `two-planes`, `zorder-alpha`, `move`); `scripts/camera-check.py`; test plan §5; vkms lease composes two planes (`docs/platform-capabilities.md` "vkms", virtual) | planned (board); host only (vkms) |
| M1-4 | Supported and unsupported combinations told apart by capability detection, planner validation and TEST_ONLY | `crates/composition-core/src/planner.rs`, `caps.rs`; `crates/composition-drm/src/lib.rs` (`check`), `profile.rs`; `crates/composition-core/tests/planner_props.rs`; `crates/composition-drm/tests/profile.rs`, `vkms.rs`; harness scenario `combos` (test plan §6) | host only (planner, vkms); planned (board) |
| M1-5 | Submission, latch/presentation and release separately observable; rotation without unsafe overwrite | `crates/composition-core/src/presentation.rs`, `engine.rs`, `trace.rs`; `crates/composition-fake/tests/rotation.rs`, `event_order.rs`, `trace.rs`, `lifecycle.rs`; harness scenario `rotate` (test plan §7); `docs/open-issues.md` X-03 | host only (fake backend); planned (board) |
| M1-6 | AGL leased output without a Weston fork, or a recorded blocking limitation | `tools/drm-harness` (`--lease kernel-lease`, `--lease dlm`, `lessor`, scenario `revoke`); `docs/dlm-aarch64.md`, `scripts/build-dlm-aarch64.sh` (no binary produced, 2026-11-28); vkms lease and revocation (`docs/platform-capabilities.md` "vkms"); test plan §8; `docs/wp5-gap-analysis.md` (upstream Weston has no lease code) | planned (board); host only (vkms) |
| M1-7 | Test report with EVK revision, panel, BSP/kernel, lease-manager version, resolution, plane assignments, errors, frame loss, latency, CPU, memory | Test plan §9 (field list), §11; `docs/imx95-board.md` (board and image named, no measurement); `docs/host-metrics.md` (host only); `docs/m1-imx95-report.md` absent | planned |

Exit rule (M1 incomplete without verified real multi-plane composition on i.MX95): not met at this commit, since M1-3 has no board result.

### M2: i.MX95 Zephyr composition

`docs/open-issues.md` (written from `20df182`) records M2 as "not started, not verified on hardware". Plan #455 has since merged the source studies, design notes and the scanout application listed below. No M2 condition has a board result, so **M2 is open**.

| # | Condition (short) | Evidence in the repository | Status |
| --- | --- | --- | --- |
| M2-a | Scanout first: working target display | `docs/platform-capabilities.md` "i.MX95, Zephyr" (stock `socionext,dpu` driver, single framebuffer, source); `docs/open-issues.md` M2-02, M2-03; `tests/board/imx95_m7_scanout/` (host build only, not run); `docs/imx95-m7-verdin-bsp.md`, `docs/imx95-m7-fb-memory.md`, `docs/imx95-m7-lvds-route.md`, `docs/imx95-m7-scmi.md` (source studies); `docs/imx95-m7-dpu-driver.md` not at this commit | host only (source, host build); no scanout observed |
| M2-b | Explicit ownership model | `docs/platform-capabilities.md` ("A55 vs M7 ownership ... not found"); `docs/open-issues.md` M2-01; `docs/imx95-zephyr-feasibility.md` (draft; ownership from the NXP EVK sources, Toradex sources not read); `docs/imx95-sm-display-lm.md`; `docs/imx95-zephyr/observed-2026-11-27.txt` (what Linux holds today); `tests/board/imx95_m7/` not at this commit | host only (source); observed 2026-11-27 (Linux-side snapshot, not an ownership proof) |
| M2-c | Buffer and synchronisation semantics consistent with M1 | `crates/composition-core` (shared presentation state machine); `backends/twoplane`, `docs/twoplane-backend.md` (i.MX95 latch note is a design statement, not a measurement); `docs/imx95-m7-backend-contract.md`, `docs/imx95-twoplane-port.md` (design); `tests/board/imx95_m7_scanout/` (latch timing hooks, not run); `tests/board/imx95_m7_engine/` not at this commit; `docs/open-issues.md` M2-04, M2-05 | host only (core, design); planned (Zephyr) |
| M2-d | Hardware overlay on i.MX95 if supported | `docs/imx95-m7-overlay-design.md` (design only, comes after scanout); `docs/open-issues.md` M2-03 (stock driver has no planes) | planned (design only) |
| M2-e | Documented limitations for unsupported properties | `docs/platform-capabilities.md` "i.MX95, Zephyr"; `docs/open-issues.md` M2-03, M2-06; `docs/imx95-zephyr-feasibility.md` (draft) | planned |
| M2-f | If BSP or firmware blocks access: blocked with a reproducible investigation report, never passed on another SoC | `docs/open-issues.md` "Blocks"; `docs/imx95-zephyr-feasibility.md` (draft; not concluded); `docs/imx95-sm-display-lm.md` (why an SM change cannot be applied here, inferred) | planned |
| M2-g | Linux vs Zephyr comparison of the same scene (WP3; roadmap) | `docs/m2-comparison-plan.md` (plan, no measurement); `tools/drm-harness` scenarios (Linux side); `docs/open-issues.md` M2-07 | planned |

### M3: AM62P portability regression

`docs/open-issues.md` records `backends/twoplane` as "host-tested only"; the board run is issue #12.

| # | Condition (short) | Evidence in the repository | Status |
| --- | --- | --- | --- |
| M3-a | Two-plane composition | `backends/twoplane`, `backends/twoplane/tests/twoplane.rs`, `tests/c/twoplane_test.c` (fake driver); `docs/twoplane-backend.md`; issue #12 (`docs/open-issues.md` M3-01, M3-02) | host only; planned (board, issue #12) |
| M3-b | Correct VSYNC latch tracking | `backends/twoplane/tests/adversarial.rs`, `tests/c/adversarial_test.c` (frame start at any point of the writes, with or without the vsync interrupt); `docs/twoplane-backend.md` "What the host test cannot show" | host only; planned (board, issue #12) |
| M3-c | Safe frame release | `backends/twoplane/tests/`; `crates/composition-fake/tests/rotation.rs`, `lifecycle.rs`; `docs/open-issues.md` M3-03 | host only; planned (board, issue #12) |
| M3-d | No visible intermediate-state regression (safety layer, producer failure) | `docs/twoplane-backend.md`; `docs/open-issues.md` M3-03 (fake driver only; real concurrency untested); `scripts/camera-check.py` (not yet used on AM62P, X-05) | host only; planned (board, issue #12) |
| M3-e | Recorded difference matrix versus i.MX95 | `docs/am62p-imx95-matrix.md`; `docs/platform-capabilities.md` "TI AM62P" ("Not researched yet"); `docs/open-issues.md` M3-04 | planned |

## Keeping this file current

Change a status only when the document named in its row records the new result, and name that document. A board result is written as **observed &lt;date&gt;** with the date the document gives. CI results, vkms runs and desk readings never raise a row above **host only** (`docs/ci.md`).
