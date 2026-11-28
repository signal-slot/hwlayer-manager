# M2 Linux vs Zephyr comparison: how it will be run

Method note, written 2026-11-28 from merged repository files only. It states how WP3 / M2 (`SoW.md`) compares Linux and Zephyr on the same scene on the i.MX95 **once a Zephyr scanout exists**. It holds **no measurement**: every timing, count or load figure below is *not measured*. It is no hardware claim and no claim from another SoC. The fuller plan is [m2-comparison-plan.md](m2-comparison-plan.md); this note is the short operating summary. [m2-comparison-plan.md](m2-comparison-plan.md) is the canonical text; where the two differ, the plan wins and this note is the defect.

Tags: **observed** (dated measurement), **read from source**, **inferred**.

## 1. Status of the precondition

- Zephyr scanout on this board does not exist yet. The DPU is held by Linux, and the gate for starting an M7 image from remoteproc is not met; the M7 run was skipped on 2026-11-28 (observed, [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md) §8).
- Until that changes, no column of §4 can be filled for Zephyr, and the Linux column of stage B waits for the M1 probe results (`m1-imx95-test-plan.md`).

## 2. (a) The scene

Pinned by the host test [`crates/composition-core/tests/m2_scene.rs`](../crates/composition-core/tests/m2_scene.rs) (read from source; a planner test, no board, no timing).

| Item | Pinned value (read from source) |
| --- | --- |
| Output | one, `W` x `H` = 1280 x 800, `refresh_mhz` 60000. The test says this is inferred from the docs for the panel, not measured; the real mode comes from the Linux probe |
| `base` | buffer 1, `W` x `H`, source = destination = full panel, z_order 0, linear. Stage A format `XRGB8888` on Linux; the stock Zephyr driver refuses it (`FORMAT`), so the agreed variant is `ARGB8888` on both sides |
| `top` (stage B) | buffer 2, `W/3` x `H/3` = 426 x 266, `ARGB8888`, `BlendMode::Premultiplied`, z_order 1, destination at (500, 300), moved over (0,0), (300,100) and the far corner in the move test |
| Plane alpha | `0x8000` of `0xFFFF` (one half) |
| Linux topology | six planes (primary + 5 overlays), zpos 0..5, plane alpha, three blend modes, linear, upscale only, no rotation, the four resource groups of the DPU profile |
| Zephyr topology (stock driver) | one primary plane, `RGB565` / `RGB888` / `ARGB8888`, no scaling, no alpha, fixed zpos, full output only |
| Expected plans | Linux: base on plane 1, `top` on a distinct plane with higher zpos; the assignment is unchanged by alpha and by move. Stock Zephyr: stage A plans on plane 1; stage B, alpha and any second layer are refused (`NoPlane`), because the engine never composes in software |
| Out of scope | downscale and rotation (refused on Linux, `SCALING` / `TRANSFORM`); not run on either side |

The test is the shared definition: either side checks that it plans the same assignment before it is timed. Stage B on Zephyr is *not applicable* until a driver with more than one plane exists (inferred; see [imx95-m7-overlay-design.md](imx95-m7-overlay-design.md)).

## 3. Recording on each side

### 3.1 (b) Linux, A55, DRM backend

- Backend: `crates/composition-drm` (read from source). A commit with `PAGE_FLIP_EVENT` yields `LatchInfo { output, submission, sequence, timestamp }`; `sequence` is the CRTC vblank counter and `timestamp` is the kernel vblank time on the DRM clock, `CLOCK_MONOTONIC` (read from source, `lib.rs`).
- Drivers: `tools/drm-harness` scenarios `two-planes`, `zorder-alpha`, `move`, `rotate` (and `combos`, which shows nothing); the `twoplane` example, whose last line is `VERDICT twoplane: PASS|FAIL <reason>`; `plane_commit` for single commits (read from source). Their `--help` wins over this text.
- Available stamps: submit return (process clock, same `CLOCK_MONOTONIC`), flip-event vblank time, vblank sequence, `released` and `dropped` events, latch timeout (read from source, [trace-format.md](trace-format.md) §3.4). Latch-to-light (pixels on the panel) is not in any DRM event (inferred).
- Board rules of [imx95-board.md](imx95-board.md) apply: files under `/tmp` only, Weston restored and confirmed active after a master-fd run.

### 3.2 (c) Zephyr, M7

- Signals: the ExtDst0 shadow-load interrupt is the stock driver's only latch point; it carries no frame number or time stamp. A FrameGen/vblank interrupt and a counter are needed and do not exist in the stock driver (read from source, [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md) §5).
- Time stamps: a free-running M7 cycle counter read in the ISR, converted with the clock frequency read from the running system, not assumed (inferred, [imx95-m7-latch-measure.md](imx95-m7-latch-measure.md) §3). An optional GPIO toggle on a logic analyser checks the counter independently, if the SM gives the M7 a pin (not known).
- Log path: lines in the trace format of [trace-format.md](trace-format.md), stored in a ring and read through the remoteproc trace buffer (`RAM_CONSOLE` + `RSC_TRACE`, debugfs `trace0`), or a UART if one is owned (read from source and inferred, [imx95-m7-console.md](imx95-m7-console.md) §3.1). Ring overrun is reported in the run's `end` line and a run with overrun is not a comparison result.
- Reduced scope: if the M7 cannot own the DPU, only candidates A (register-ownership probe) and B (engine on the M7, plan over the log) of [imx95-m7-reduced-scope.md](imx95-m7-reduced-scope.md) can run. They give planning and ownership evidence and **no latch or vblank timing**, so §5 applies: that is a blocker report, not a comparison.

## 4. (d) Metrics

Every cell is *not measured*. Units and recording (the definitions are M-1..M-4 of [m2-comparison-plan.md](m2-comparison-plan.md) §3; M-5 plane assignment, M-6 event order violations and M-7 memory are in the plan and are not repeated here).

| Metric (plan id and name) | Unit | Linux (A55) | Zephyr (M7) | Value |
| --- | --- | --- | --- | --- |
| M-1 frame interval | ms (interval median, maximum; outliers are intervals outside one refresh period ± 20 %) | differences of consecutive flip-event vblank times | differences of consecutive cycle-counter reads in the latch ISR, converted by the read clock | not measured |
| M-2a submit-to-latch latency (M-2b latch-to-light is not measurable by software or by the frame camera) | ms (median, p95, maximum) | commit-return time to the flip-event vblank time, same clock | commit-call return count to the latch-ISR count, same counter | not measured |
| M-3 missed frames | count; late latches (interval above 1.5 refresh periods) by count | harness counters `submitted`, `latched`, `dropped`; identity `latched + dropped = submitted` | the same counters from the application's own log lines | not measured |
| M-4 CPU load | percent of that core's time in the submit and event path, run-average | process CPU time over wall time, frame generation reported apart | busy cycles in the path over total cycles from the counter, or idle-thread accounting; the method used is stated | not measured |

Rules: A55 and M7 CPU loads are never added or ranked on one scale (inferred). Each result states date, commit under test, tool version, mode (Mode R refresh-locked, Mode O over-submitted), frame count (the plan sets at least 1000 for latency, [plan](m2-comparison-plan.md) §2.3) and whether the display driver is stock or patched.

## 5. (e) What counts as a comparison

Counts (all of them required):

- Both columns come from the **same board, same panel, same scene definition** of §2, each a dated measurement, with the numbers from an i.MX95 only.
- Both sides planned the scene as `m2_scene.rs` pins it, and every difference in what each side supports is listed (for stage A: the `ARGB8888` substitution).
- The Zephyr latch time stamp comes from a real display interrupt, not from a timer, a sleep or a simulated display.
- No dropped or overwritten frame is hidden, and ordering violations are zero on both sides.
- Each side's stage meets the pass criteria of [m2-comparison-plan.md](m2-comparison-plan.md) §5 (stage A: §5.1; stage B: §5.2; the comparison: §5.3); this note does not restate them.

Does not count:

- Any run on another SoC, or on QEMU, `native_sim` or the dummy display driver: host builds of the Zephyr apps show that they compile and run, not display timing.
- The reduced-scope candidates alone (§3.2), or a Zephyr run that only proves the M7 starts.
- Linux numbers from a different mode, panel or kernel than the Zephyr run, or from M1 runs on a different date without being named.
- A picture without the commit log, and any figure without date, source and tag.
- Stage B on Zephyr, unless a driver with independent planes is used; otherwise stage B is a blocker report, listed *not applicable* with the reason.
