# i.MX95: design of the Linux vs Zephyr comparison of one scene (WP3, M2)

Written 2026-11-28. Design only. Nothing was run on a board, no code was written, and this file holds no measurement and no hardware claim.

`SoW.md` §5.1 and WP3 ask for "Linux and Zephyr on the same i.MX95 board using the same scene definition, with explicit differences in supported features and synchronization". Milestone M2 (§9) is not passed on another SoC's evidence. This file defines what such a comparison is, how each number would be taken, what must be equal for it to be fair, and what stops it today.

Tags, on every statement: **read from source** (a file of this repository, named), **inferred** (reasoning from those files, not checked by running anything), **not measured** (a quantity no run has produced). A table row carries its tag in its last tag column. There is no *observed* row. Pass thresholds below are **inferred** proposals, not results.

Inputs read (all in this repository): [SoW.md](../SoW.md), [board-scenarios.md](board-scenarios.md), [imx95-m7-backend-contract.md](imx95-m7-backend-contract.md), [imx95-twoplane-port.md](imx95-twoplane-port.md), [m2-comparison-plan.md](m2-comparison-plan.md), [m1-imx95-test-plan.md](m1-imx95-test-plan.md), [imx95-sm-display-lm.md](imx95-sm-display-lm.md), [imx95-m7-fb-memory.md](imx95-m7-fb-memory.md), [platform-capabilities.md](platform-capabilities.md), [weston-imx-lease.md](weston-imx-lease.md). A trace-format document and a feasibility document for the i.MX95 Zephyr work were not on main when this was written and are not linked; where this design depends on them it says so.

No board address, host name, user or serial path appears here.

## 1. Relation to the existing M2 plan

[m2-comparison-plan.md](m2-comparison-plan.md) already holds a scene, a metric list and criteria (**read from source**). This file does not replace it. It is the design rationale: why the scene is shaped as it is, what makes the two sides comparable, and how each quantity is defined. Where the two differ, the M2 plan is the operational text, and the difference is a defect of one of them for its owner to fix (**inferred**).

## 2. The comparison scene

The scene is expressed in the engine's vocabulary (source buffer, source and destination rectangle, z-order, format, plane alpha, blend), so one description drives `composition-drm` on Linux and the Zephyr side on the M7 (**read from source**: `SoW.md` §6, §7; [imx95-m7-backend-contract.md](imx95-m7-backend-contract.md) §1).

### 2.1 Stage A: scanout, one layer

| Property | Value | Tag |
| --- | --- | --- |
| Layers | 1: `base`, full panel, z 0 | inferred (proposal) |
| Format | `XRGB8888`; if the stock Zephyr driver cannot do it, the format both sides use is the one it offers (`ARGB8888` or `RGB565`), recorded as a substitution | read from source (the stock driver lists RGB565, RGB888, ARGB8888: backend contract §2) |
| Size | the panel's native mode, taken from the Linux probe, not assumed | inferred; the mode is not measured here |
| Scaling, alpha, rotation | none | inferred (proposal) |
| Buffers | 3 in rotation, each rewritten only after its release event | inferred (proposal), following the M1 `rotate` scenario in the M2 plan |
| Content | flat saturated colour plus a marker strip carrying the low bits of the frame number | inferred (proposal) |

Stage A is the only stage the stock Zephyr driver could carry, because it has one plane over a constant colour (**read from source**: backend contract §2).

### 2.2 Stage B: overlay

| Layer | Format | Size | z | Update | Tag |
| --- | --- | --- | --- | --- | --- |
| `base` | `XRGB8888` | full panel | 0 | buffer fixed after first fill | inferred (proposal) |
| `top` | `ARGB8888` | about one third of the panel in each direction | 1 | destination position changes every frame; buffer not rewritten | inferred (proposal) |

Sub-scenes: opaque, plane alpha one half (only where both sides expose a plane alpha), move, z-swap (only where zpos is mutable). A sub-scene a side cannot express is reported *not applicable* with the reason, never dropped silently (**inferred**).

Fixed buffers with a moving destination are deliberate: if only the position changes, a correct picture can come only from the hardware combining two planes, so flattening cannot fake it (**read from source**: the anti-flattening rule of M1, M2 plan §1 item 7).

More than two layers, scaling, NV12 and rotation are out of scope. Linux-only cases may be run but are labelled *Linux only* (**inferred**).

### 2.3 Update pattern

| Mode | Pattern | Why | Tag |
| --- | --- | --- | --- |
| R | one commit per refresh period | steady-state latency and interval | inferred (proposal) |
| O | submission faster than refresh for a fixed part of the run | forces replacement of pending commits, so `Dropped` is observable | read from source (`SoW.md` §7 requires it); the mode itself is inferred |

Run length: at least 1000 frames per mode, the minimum the M1 plan uses for a p95 (**read from source**: M2 plan §2.3). The actual count is recorded with each result.

## 3. What is measured

Names follow the engine events *accepted/submitted*, *latched/presented*, *released* (**read from source**: `SoW.md` §7; [board-scenarios.md](board-scenarios.md), "Common material").

| Quantity | Definition | Tag of the definition | Value |
| --- | --- | --- | --- |
| Plan result | per commit: the plane (Linux: DRM plane id and type; Zephyr: DPU layer or stream index), format, z, and whether the plan was accepted or rejected with its reason | inferred | not measured |
| Plan (validate) time | time of the planning and check call. On Zephyr `check` is a software function only, with no test-only hardware path | read from source (backend contract §3.2) | not measured |
| Commit latency | time from the commit call returning to the latch event | inferred | not measured |
| Latch-to-light | latch event to pixels on the panel | inferred; not software-observable | not measured |
| Frame interval | time between consecutive latch events; outliers are intervals outside one refresh period ± 20 % | inferred (thresholds taken from M1, M2 plan §3) | not measured |
| Missed frames | `submitted − latched` (drops) and late latches (interval above 1.5 refresh periods); `latched + dropped = submitted` must hold | read from source for the identity (M2 plan §3) | not measured |
| Event-order violations | must be zero | read from source (`SoW.md` §7) | not measured |
| CPU cost | share of the executing core's time in submit and event handling; frame generation reported separately | inferred | not measured |
| Memory | Linux producer peak resident size; Zephyr static footprint and scanout buffer size; informational | inferred | not measured |

## 4. How each quantity is taken

### 4.1 Linux, A55, through `composition-drm`

| Quantity | Method | Tag |
| --- | --- | --- |
| Plan result | the `commit` trace lines of the harness (layer, plane, format, zpos); the planner is `composition-drm` | read from source (board-scenarios.md, tools listing); a parsable shared trace format is not on main |
| Commit latency, frame interval | DRM page-flip completion events carry a kernel timestamp and a CRTC sequence number; the interval is between consecutive events; commit latency is the event timestamp minus the monotonic time at which the commit call returned | inferred (event semantics of the DRM API; not checked on this board) |
| Missed frames | harness counters `submitted`, `latched`, `dropped`, and late counts computed from the intervals | read from source (M2 plan §4.1); whether the current harness revision reports them is to be checked with its `--help` |
| CPU cost | process CPU time over wall time during the run, with Weston stopped for a DRM-master run and restored afterwards | inferred |
| Display ownership | Linux holds DRM master after stopping Weston, or holds a lease if one is available | read from source (board-scenarios.md §1; the lease question is in [weston-imx-lease.md](weston-imx-lease.md)) |

### 4.2 Zephyr, M7

| Quantity | Method | Tag |
| --- | --- | --- |
| Plan result | the backend's `capabilities`, `check` and `commit` logged with the layer index and format; for one plane `check` is a software check | read from source (backend contract §3) |
| Latch event | a DPU interrupt counted in an ISR. The stock driver enables only the ExtDst0 shadow-load interrupt and exposes no event callback, so a driver extension is needed before a latch is observable | read from source (backend contract §2, §3.4) |
| Commit latency, frame interval | a cycle counter read in the ISR and at commit return, converted with the counter frequency read from the running system and not assumed. Printed after the run, not inside the ISR | inferred |
| Is the shadow-load interrupt a frame boundary? | open: it may fire at trigger time instead of at the next frame start. Until settled, the Zephyr "latch" is that interrupt and the result says so | read from source as open ([imx95-twoplane-port.md](imx95-twoplane-port.md), "Latch source") |
| Missed frames | counters of commits and of latch interrupts; a latch timeout supplied through the engine's time input is reported once | read from source (backend contract §4) |
| CPU cost | share of cycles in the submit path and ISR, from the cycle counter; core-relative | inferred |
| Memory | linker map and the buffer table | inferred |
| Log transport | a console UART or an rpmsg channel; not fixed here | not decided |

### 4.3 Latch-to-light on both sides

Neither side sees pixels light up. A frame camera cannot resolve an interval shorter than a refresh period (**inferred**), so latch-to-light is reported *not measurable*, and the comparison uses software-observable latch times only (**inferred**).

## 5. What must be equal for the comparison to be fair

| Condition | Why | Tag |
| --- | --- | --- |
| Same board, panel, cabling and panel mode | the refresh period sets every interval | inferred |
| Same scene file, frame count and buffer count; same format, or the common substitute | formats change fetch cost and bandwidth | inferred |
| Same buffer contents per frame number | keeps content-dependent effects out | inferred |
| Frames generated before the timed region, or generation cost reported separately | the two cores differ in speed; generation must not be charged to composition | inferred |
| Timestamps taken at the same logical points on both sides: commit return and latch event | otherwise the two latencies are different quantities | inferred |
| CPU cost reported per core, never summed or normalised across A55 and M7 | different cores, clocks and caches | inferred |
| Scanout-memory cacheability stated on both sides; on the M7 the caches are cleaned before commit | stale lines would show an old frame | read from source for the need (backend contract §3.3; [imx95-m7-fb-memory.md](imx95-m7-fb-memory.md)); values not measured |
| Display ownership stated per run (Linux master, lease, or M7) | the two sides do not drive the DPU at once unless an ownership study allows it | inferred |
| Each side states what it cannot express (planes, alpha, blend, event type) | `SoW.md` WP3 asks for explicit differences | read from source (`SoW.md` §8 WP3) |
| Numbers come only from the i.MX95; AM62P and STM32MP257 records are format references | `SoW.md` §5.1 forbids claiming parity from another SoC | read from source |
| Each result carries its date, the commit under test, tool versions and a tag | a number without its source is a failed report | read from source (M2 plan §1) |

Unequal by construction, to be listed in any report: Linux has an event interface with vblank timestamps and a test-only commit, and the stock Zephyr driver has neither (**inferred** from backend contract §2 and the DRM API). Linux uses the vendor driver's multi-plane pipeline and its per-commit resource allocation, while the stock Zephyr driver offers one plane (**read from source**: [platform-capabilities.md](platform-capabilities.md), i.MX95 Linux section; backend contract §2). Stage B on Zephyr needs a multi-plane driver that is not upstream (**read from source**: imx95-twoplane-port.md, "What is generic and what is SoC-specific").

## 6. Pass and verdict rules (proposed)

All **inferred**; none is a result.

- Stage A passes on a side if the scene shows, the identity `latched + dropped = submitted` holds, there are zero order violations, and every cell is filled or labelled *not measured*, *not measurable* or *not applicable* with the reason.
- Stage B passes on a side only if the move sub-scene shows the moving layer with fixed buffers and the commit log names two distinct planes.
- The comparison is complete only when both sides have a passing stage, or one side has a blocker report in place of stage B (`SoW.md` §8 WP3, §9 M2: **read from source**).
- A side without a verdict does not take one from the other side's result.

## 7. Blockers that keep the comparison from running today

| Blocker | Basis | Tag |
| --- | --- | --- |
| The DPU, its clocks and its power domain belong to Linux under the board's boot flow: Linux runs the vendor DRM driver and Weston holds the display. Only the stock System Manager configuration of NXP's reference board was read; the Verdin's configuration was not | [imx95-sm-display-lm.md](imx95-sm-display-lm.md) §1 and the board rules quoted in board-scenarios.md | read from source; the Verdin SM configuration is not read |
| Giving the display to the M7 needs a changed System Manager logical-machine configuration, which cannot be applied here | imx95-sm-display-lm.md (scope) | read from source |
| Linux and the M7 cannot be assumed to share the DPU: both would use the same shadow-load registers and clocks | none read; reasoning only | inferred |
| The stock Zephyr DPU driver has one plane, a blocking write, no event callback, no clock or power-domain call, and a hard-coded register block that differs from the devicetree address; the DPU and DSI nodes are disabled by default | backend contract §2; imx95-twoplane-port.md | read from source |
| The stock SoC description has a DSI node but no LVDS node, and the Verdin panel is LVDS | backend contract §7 | read from source |
| The meaning of the ExtDst0 shadow-load interrupt, the cache attributes of scanout memory, and the DPU's view of buffer addresses are open | backend contract §7; imx95-twoplane-port.md | read from source as open |
| The M1 Linux side exists only as plans and probe records; no Linux run of this scene exists | M2 plan §4.1; [m1-imx95-test-plan.md](m1-imx95-test-plan.md) | read from source |
| A parsable trace format shared by both sides is not on main | board-scenarios.md, "What is common to all rows and missing" | read from source |
| The Zephyr tools (single-plane scanout app with latch hooks, engine-linked app) are planned items | backend contract §6 | read from source |

Consequence (**inferred**): today only the Linux side of stage A could run in principle, once the M1 tools exist. No Zephyr cell can be filled. Every value in §3 is *not measured*.

## 8. Order of work to unblock (proposal, inferred)

1. Settle ownership: read the Verdin's System Manager configuration and decide whether the M7 can own the DPU at all.
2. Fix a shared trace line format for both sides.
3. Extend the Zephyr driver for a latch event and a non-blocking present (backend contract §6, steps 2 and 3), then run a single-plane scanout app.
4. Run stage A on both sides in separate sessions at the same panel mode.
5. Study the DPU units beyond the stock pipeline, then attempt stage B, or write the blocker report in its place.
