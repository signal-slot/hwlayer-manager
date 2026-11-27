# WP5 evidence plan: reproducible demo and benchmark evidence

SoW §8 WP5 asks for "minimal documentation, reproducible demo, benchmark evidence, project licensing and contribution guidelines". This page plans the **demo and benchmark evidence** part. It is a plan written 2026-11-28 from the repository at the commit it was committed on. **It claims no result**: no scenario named here was run for this page, every figure is *not measured*, and every threshold is a proposal. Statements about what a script or document contains are *read from source* (the file is named); everything about future runs is *proposal*.

The page holds no addresses, host names, users, serial paths or credentials. `<board>` means the ssh destination from the farm lease.

## 1. Gating conditions

WP5 evidence is collected **after M1** (SoW §9, M1 exit rule: M1 is incomplete without verified real multi-plane composition on i.MX95). Order:

1. **M1 closed first.** The i.MX95 Linux runs of `docs/m1-imx95-test-plan.md` §11 (`wp0-probe`, `m1-master-run`, `m1-lease-run`) are passed, or blocked with a recorded issue (condition 6). Until then no i.MX95 Linux row below is run as WP5 evidence. A WP5 run must not be used to claim M1.
2. **A board's evidence waits for that board's earlier items.** A board whose topology and two-plane run are not recorded (`docs/observed/`, `docs/platform-capabilities.md`) gets no demo row until they are.
3. **M2 rows wait for the feasibility answer.** The Zephyr rows depend on `docs/m2-comparison-plan.md` and the i.MX95 Zephyr feasibility document (another item). If the M7 cannot own the display, the Zephyr "demo" is the blocker report, and it is reported as such, not as a pass (SoW §9, M2).
4. **M3 rows stay with issue #12.** This page plans nothing for the AM62P board run beyond naming what it would need.
5. **A clean tree.** A run is evidence only if `sh scripts/check-all.sh` passed on the commit that was run (the CI of `docs/ci.md`; it shows host tests and builds pass, nothing about hardware).
6. **Board rules** of `docs/imx95-board.md` (B1-B5 in `docs/board-scenarios.md`) hold for every i.MX95 run: write only under `/tmp`, no `fw_setenv`/`saveenv`, never write `mmcblk0boot*`, restore Weston and confirm it is active, remove the copies. The project rule for the AM62P applies to any later AM62P run: put it back on the Torizon boot chain and confirm over ssh.

## 2. How a run is made reproducible

Every run of every board uses the same recipe. None of its parts exist as one runner script today (`docs/board-scenarios.md` §4); the recipe is a procedure over existing tools.

| Element | Rule | Existing tool (read from source) |
| --- | --- | --- |
| Pinned commit | Record the full commit hash of the repository and of `Cargo.lock`; build with `--locked`; the tree is clean | `scripts/check-all.sh`, `scripts/cross-aarch64.sh --locked`, `Cargo.lock` |
| Pinned inputs | Record image and kernel version, and for Zephyr the workspace and SDK pins; for the lease path the drm-lease-manager version | `scripts/imx95-session.sh` writes a `<name>.header` with date, kernel, image; `scripts/build-dlm-aarch64.sh` |
| Build | Host build and the cross build exactly as CI does; the two `thumbv7em` builds are part of the same check | `scripts/check-all.sh [--cross]`, `docs/ci.md` |
| Deploy and run | Deploy to `/tmp`, stop and restore Weston, run with saved header, stdout, stderr; clean up | `scripts/imx95-session.sh` (`deploy`, `display-owner stop/start`, `run`, `cleanup`) |
| Physical check | Judge a picture against an `EXPECT.json`; a picture alone never passes | `scripts/camera-check.py` |
| M7 load | Load from `/tmp`, print state before/after, collect dmesg and log, stop, restore | `scripts/imx95-m7-rproc.sh` (host-tested on a fake sysfs only) |
| Secrets | Scan every log and commit message before committing | `scripts/check-no-board-secrets.sh`, `scripts/check-no-farm-ids.sh` |
| Layout of Zephyr test apps | Lint before use | `scripts/check-zephyr-apps.sh` |
| Result file | First line `SCENARIO <name> PASS\|FAIL\|BLOCKED\|NOT-RUN`, then the evidence lines (convention from `docs/board-scenarios.md`; no script writes it yet) | none (missing) |
| Dated record | One issue per run through `.github/ISSUE_TEMPLATE/board-result.md`: board and carrier, image and kernel, date `YYYY-MM-DD`, repository commit, test program, exact command with addresses removed, verbatim verdict lines, tag, what was restored | the template |
| Tag | Each recorded statement is exactly one of *observed* (with its date), *read from source* (with a pinned URL or file and commit), *inferred*. Anything not measured says "not measured" | `docs/platform-capabilities.md` convention, `CONTRIBUTING.md` |

A run is **reproducible** when someone else can take the recorded commit, the recorded inputs and the command line of the issue, and obtain the same verdict lines or a different dated result with the same recipe. Repeat runs are separate dated issues; an earlier one is never edited into a later one.

Statistics proposal: each benchmark is run `N` times (N set at the first run, then fixed and written into the issue); the issue reports the median, the maximum and the count outside the stated bound, not a single best run. This is a proposal; no `N` is chosen here.

## 3. Metrics that count as benchmark evidence

SoW §10 lists the performance metrics. They are the same on every board, with the definitions of `docs/m2-comparison-plan.md` §3 (M-1 frame interval, M-2a submit-to-latch, M-2b latch-to-light, M-3 missed frames, M-4 CPU load, M-5 plane assignment, M-6 event-order violations, M-7 memory). Tag on each: **observed** only when the board printed it on the stated date, otherwise *not measured*.

| Metric (SoW §10) | Counts as evidence when | Today |
| --- | --- | --- |
| Plane assignment time | A validate-time figure from a board run of the same planner, with the host figure of `docs/host-metrics.md` kept apart (different CPU) | host only; board *not measured* |
| Ready buffer to latch | M-2a from the `commit` and `latched` trace events | *not measured* |
| End-to-end presentation latency | M-2a plus M-2b; M-2b needs an external instrument, and if none exists it is recorded as "not available", never estimated | *not measured* |
| Dropped / late frames | M-3, with the identity `latched + dropped = submitted`, and M-6 equal to zero | *not measured* on a board |
| CPU and memory | M-4 and M-7, core-relative; A55 and M7 figures are never summed or compared as one scale | *not measured* |
| DMA/display bandwidth | Only if a counter is readable on the board; otherwise "not available" | *not measured* |
| Power | Only if an instrument is attached; otherwise "not available" | *not measured* |
| Event order | Zero violations in a buffer-rotation run (`rotate`) | host (fake backend, vkms) only; board *not measured* |

Rules for what is **not** evidence: a host figure presented as a board figure; a figure from one board used for another; a camera picture without log lines; a run on a dirty tree or with an unrecorded commit; a figure compared across A55 and M7 timestamps (`docs/m2-comparison-plan.md` §4.2).

Trace parsing depends on the trace line format, which is not fixed (`docs/open-issues.md` X-03). Until a format and checker exist, metrics are read from the trace output by hand and the issue says so.

## 4. Per board: demo scenario, metrics, gating

"Demo" is the scenario of `docs/board-scenarios.md` that WP5 shows to a reviewer: the one that makes the engine's claim visible (two independently supplied buffers on separate hardware planes, not flattened) and reproducible from one command line. Scenario ids (A1.., Z1.., C1..) are those of that page. Boards the catalogue does not cover get a proposal: Path A's rows applied to their DRM device, since the same harness and `twoplane`/`probe` examples exist; whether each row can be run there is not checked here.

| Board (where recorded) | Demo scenario | Benchmark scenarios and metrics | Reproducibility specifics | Gate |
| --- | --- | --- | --- | --- |
| **i.MX95 Linux, Verdin iMX95 / Mallow / LVDS** (`docs/imx95-board.md`, `docs/m1-imx95-test-plan.md`, `docs/platform-capabilities.md`) | **A1** `two-planes` on a DRM master fd, then through the lease, with the `move` camera check | A2 steady state (`move`, `rotate`): M-1, M-2a, M-3, M-4, M-5, M-6, M-7; A4 `revoke` lease-loss timing; A3, A5, A6 as functional rows | session script, harness `--log`, `EXPECT.json` for the panel corners, `check-all.sh --cross` for the aarch64 binaries | M1 closed (§1.1); Weston-lease support is a separate recorded question (`docs/weston-imx-lease.md`); the lease half stays "blocked" if M1 recorded it so |
| **i.MX95 Zephyr, Cortex-M7** (`docs/m2-comparison-plan.md`, `docs/imx95-m7-*.md`) | **Z1** Stage A one-plane scanout, only if the feasibility answer allows the M7 to own the display; otherwise the blocker report | Z2: M-1..M-3 from cycle-count traces converted on the host, M-4 core-relative, M-7 static footprint and buffer size; the Linux-vs-Zephyr comparison table of the M2 plan, each cell *not measured* until filled | `imx95-m7-rproc.sh`, an image built from a recorded workspace pin, log capture; M7 and A55 timestamps never subtracted | M1 closed; the M2 gate of §1.3; the script is unrun on a real remoteproc |
| **TI AM62P, R5F** (`docs/twoplane-backend.md`, `docs/am62p-imx95-matrix.md`) | none planned here; the two-plane regression is **issue #12** (M3) | C1-C6 belong to issue #12 | a Torizon restore is part of any run | M1 and the M3 owner; the difference matrix needs both boards' dated results |
| **RK3588 VOP2, NanoPC-T6** (`docs/observed/rk3588-topology.md`, `docs/observed/rk3588-source.md`) | proposal: **A1** `two-planes` on the HDMI connector; no RK3588 engine backend exists yet, so the demo would use the DRM backend | A2 `move`/`rotate` metrics as above | recorded kernel build; the topology doc's session date | M1 closed; topology documented as a staging note, not folded into a final capability row by this page |
| **BCM2712 HVS, Raspberry Pi 5** (`docs/observed/bcm2712.md`, `docs/observed/bcm2712-topology.md`) | proposal: **A1** | A2 as above | as for RK3588 | M1 closed |
| **BCM2711 HVS, CM4 / reTerminal** (`docs/observed/bcm2711-cm4.md`) | proposal: **A1** | A2 as above | the doc records a re-run on a named commit; WP5 repeats it on the pinned WP5 commit | M1 closed |
| **STM32MP257F-DK, LTDC** (`docs/observed/stm32mp257.md`) | proposal: **A1** (the staging note records a `twoplane` run) | A2 as above | as for RK3588 | M1 closed |
| **vkms (virtual, host)** (`crates/composition-drm/tests/vkms.rs`, `scripts/vkms-vm-test.sh`) | not a board; a host-only regression, not a WP5 hardware demo | `plan_time` (`docs/host-metrics.md`); lease revocation and buffer-release order tests | `scripts/vkms-harness-test.sh`, `scripts/vkms-vm-test.sh` | none; never reported as hardware |

Boards other than the i.MX95 appear in the SoW only as portability checks; running them for WP5 is optional and follows the M1 result, not the reverse. A board that is busy with another task is not taken by this plan: evidence runs request the board through the farm lease like any other task.

## 5. The demo itself

The WP5 demo is one repeatable command sequence per board, written into the result issue, not a showcase application. For the i.MX95 Linux demo it is A1: deploy, stop Weston, run `two-planes`, take two camera pictures with the expected regions, restore Weston. It shows two distinct plane ids and distinct buffer ids in the log and an `EXPECT.json` verdict; both are required. A recorded video or screenshot may accompany an issue but is never the evidence.

Demo output that would hide a limitation is a failure of the plan: a blocked row is published as blocked, with the reproducible limitation (SoW §9, M1 condition 6; M2).

## 6. What is missing before this plan can run

- A scenario runner and result writer (`docs/board-scenarios.md` §4, `docs/open-issues.md` X-02).
- A trace line format and checker (X-03), so M-1..M-6 are computed rather than read by eye.
- A chosen `N` and bounds for drops, latency and memory, set after a first run shows the cost.
- A recorded way to measure latch-to-light, or a statement that none is available (`docs/m2-comparison-plan.md` §4.3).
- The M1 report (`docs/m1-imx95-report.md`) once M1 is run; this page does not link it because it does not exist yet.
- Results for the non-i.MX95 boards under the pinned WP5 commit; the staging notes in `docs/observed/` are earlier, dated, and not WP5 evidence.

## 7. Related files

`SoW.md` (§8 WP5, §9, §10), `docs/board-scenarios.md`, `docs/m1-imx95-test-plan.md`, `docs/m2-comparison-plan.md`, `docs/host-metrics.md`, `docs/ci.md`, `docs/open-issues.md`, `docs/wp5-gap-analysis.md`, `docs/sow-traceability.md`, `docs/platform-capabilities.md`, `docs/imx95-board.md`, `.github/ISSUE_TEMPLATE/board-result.md`.
