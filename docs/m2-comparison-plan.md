# M2 plan: Linux vs Zephyr comparison of the same scene on i.MX95

This is the plan for the comparison that `SoW.md` WP3 ("Compare Linux and Zephyr on the same i.MX95 board using the same scene definition") and milestone M2 (§9) require. It defines the reference scene, the metrics, how each is measured on Linux and on Zephyr, the pass criteria, and which measurements cannot start before the DPU ownership question is settled.

**This file is a plan. It holds no measurement.** Wherever a number would go, it says *not measured*. Numbers that appear below are thresholds and parameters chosen for the criteria, not results. Nothing here is a hardware claim about Zephyr on the i.MX95, and nothing is claimed from another SoC (SoW §5.1, §9 M2).

The plan holds no board addresses, host names, users or serial paths. `<board>` is supplied by the session setup.

## 0. Inputs and status

| Input | Role | Status |
| --- | --- | --- |
| `SoW.md` §5.1, §6, §7, §9 (M2), §10 | requirements | main |
| `docs/m1-imx95-test-plan.md` | Linux scenarios, camera procedure, evidence layout, verdict rules | main |
| `docs/host-metrics.md` | plane-assignment (validate) time on a host, no hardware | main |
| `docs/observed/**` | how a run on another SoC was recorded (STM32MP257F-DK: `RESULT`/`VERDICT` lines, latch interval, late count) | main; format reference only, no number transfers |
| `docs/imx95-zephyr-feasibility.md` | which core owns the DPU, clocks, power domains, display routes, interrupts and scanout memory; how the M7 is loaded | **written by another item; not yet read by this plan.** Called "the feasibility doc" below |
| `tools/drm-harness`, `crates/composition-drm` `twoplane` example | Linux measurement tools | harness scenarios are *planned* in the M1 plan; check its `--help` |
| Zephyr test applications for the i.MX95 M7 (`tests/board/imx95_m7/`) | Zephyr measurement tools | planned, another item |

Tags used in the result tables of the eventual report, as everywhere in this repository: **observed** (dated measurement), **read from source**, **inferred**. Until a run exists, every result cell is *not measured*.

## 1. Ground rules

1. **Same board, same panel, same scene definition.** The Verdin iMX95 on its carrier with the 10.1" LVDS panel. Both sides use the one scene description in §2, and the report states every difference in what each side supports (SoW WP3).
2. **The two sides do not run at the same time unless the feasibility doc says the DPU route can be shared.** The default assumption of this plan is *either/or*: a Linux run, then a Zephyr run, in separate sessions. Whether the M7 can own the display alongside Linux, instead of it, or not at all is the open question (§8).
3. **Scanout first, overlay second** (SoW §5.1). Stage A (one plane) comes first and is the only stage the stock upstream Zephyr driver is expected to allow. Stage B (overlay) runs only after stage A passes and only if the hardware and a driver permit independent planes. Otherwise stage B is replaced by the blocker report (§7).
4. **Never claim parity from another SoC.** Linux numbers from the i.MX95 are compared only with Zephyr numbers from the i.MX95. The AM62P and STM32MP257 records are format references.
5. **Dated, honest tags.** Every number in the report carries the date, the commit under test, the tool version, and a tag. A figure without its source is a failure of the report, as in M1 §9.
6. **Board hygiene** is as in `docs/imx95-board.md`: write only under `/tmp`, install nothing, change no boot setting, restore Weston and confirm it is running at the end of a Linux session.
7. **Flattening is excluded on both sides.** The M1 rule applies unchanged: a picture alone never proves hardware composition. The commit log (plane ids, buffer ids) and the move test decide.

## 2. Reference scene

The scene is a list of layers in the engine's own vocabulary (`composition-core`: source buffer, source rectangle, destination rectangle, z-order, format, plane alpha, blend), so the same description can drive `composition-drm` on Linux and an engine build or direct driver calls on Zephyr. It is defined relative to the panel's native mode, which comes from the Linux probe (condition 2 of M1) and is **not measured** here; `W` and `H` below are that mode's width and height.

Colours follow the M1 camera rules: saturated, flat, well separated, no grey.

### 2.1 Stage A: one plane

| Layer | Plane | Format | Source size | Destination | z | Content |
| --- | --- | --- | --- | --- | --- | --- |
| `base` | the primary plane | `XRGB8888` | `W` x `H` | full panel, no scaling | 0 | flat background colour, with a coloured frame-number marker strip along one edge, see 2.3 |

- Two or three buffers in rotation (as the M1 `rotate` scenario: two as a second run, three as the main run), allocated by the producer, each filled only after its `released` event.
- No scaling, no alpha, no rotation. If the panel's native mode is not accepted by the stock Zephyr driver, the Zephyr run states the mode it did use and the Linux run is repeated at the same mode (the scene stays identical).
- `XRGB8888` is the format the scene asks for. If the stock Zephyr driver exposes only another format, the scene's stage A format becomes that format on **both** sides, and the substitution is recorded.

### 2.2 Stage B: overlay

Stage A plus a second layer:

| Layer | Plane | Format | Source size | Destination | z | Content |
| --- | --- | --- | --- | --- | --- | --- |
| `base` | primary | `XRGB8888` | `W` x `H` | full panel | 0 | as stage A |
| `top` | an overlay plane | `ARGB8888` (premultiplied or straight as the plane reports it) | about `W/3` x `H/3` | same size, offset from the centre, overlapping `base` by at least a quarter of its own area | 1 | flat second colour, marker strip as 2.3 |

Sub-scenes, all on the same two planes (M1 `zorder-alpha` and `move`):

1. **opaque**: `top` over `base`.
2. **alpha**: `top` at a plane alpha of one half, where a plane alpha property exists on both sides. Otherwise *not applicable*, with the reason.
3. **move**: `top` moves by changing its destination position every frame; neither buffer is written after its first fill (anti-flattening, M1 §5.3).
4. **z-swap**: `top` below `base`, where the zpos is mutable. Otherwise *not applicable*.

Layers beyond two, scaling, NV12 and rotation are out of the first comparison. They can be added later as `combos`-style TEST_ONLY cases on Linux only and are then reported as *Linux only*, never as a Zephyr result.

### 2.3 Update rate and frame marking

Two update modes, run for the same number of frames on both sides:

- **Mode R (refresh-locked):** one commit per refresh period, each commit changing the buffer in rotation, as M1 `rotate` "at the refresh rate".
- **Mode O (over-submitted):** submission faster than the refresh for a fixed part of the run, so that dropped frames occur and are observable (SoW §7: dropped or overwritten pending frames are never hidden).

In stage B, mode R uses the `move` scene (position changes, buffers fixed). In stage A, mode R changes the buffer.

Each frame carries its number in two forms, so that the camera, the log and the readback can be matched:

- a header word and counter inside the buffer (as in M1 `rotate`), so a readback shows which frame each buffer holds;
- a marker strip along one panel edge that encodes the low bits of the frame number as a row of saturated cells. Cells are at least one fifth of the panel width apart, in M1's sense, if used by the camera; the strip is for log correlation and is optional for the pictures.

Run length: a number of frames large enough for p95 to mean something. The M1 plan uses at least 1000 frames for latency; this plan uses the same minimum, and the report records the actual count.

## 3. Metrics

The metric names follow M1 §9 and the harness's trace events (`validate`, `commit`, `latched`, `released`, `dropped`) so Linux and Zephyr results share a table. The event vocabulary is SoW §7: *accepted/submitted*, *latched/presented*, *released*.

| # | Metric | Definition |
| --- | --- | --- |
| M-1 | Frame interval | the time between consecutive `latched` events. Reported as the median and the maximum, and the count of intervals outside one refresh period ± 20 % (M1 §7.1), with dropped frames listed separately |
| M-2 | Latch-to-present latency | two parts, kept apart. (a) **Submit-to-latch:** time from the `commit` call returning to the `latched` event. (b) **Latch-to-light:** time from the latch event to the pixels on the panel. (a) is software-observable; (b) is not, see §4.3 |
| M-3 | Missed frames | `submitted` minus `latched` (the `dropped` count) and the number of late latches (interval above 1.5 refresh periods). The identity `latched + dropped = submitted` must hold, as in M1 §7.1 |
| M-4 | CPU load | the share of the core's time spent in the submission and event path, over the run, with the frame-generation cost reported separately. Core-relative: A55 load and M7 load are not summed or compared as one scale |
| M-5 | Plane assignment | the plane id (or on Zephyr the DPU layer/stream index), its type, format, and z-order for every layer of every commit; the validate (plan) time where a validate step exists |
| M-6 | Event order violations | zero are required (SoW §7, M1 §7.1) |
| M-7 | Memory | peak resident memory of the producer on Linux; static footprint (text, data, bss) and the scanout buffer size on Zephyr. Informational, not compared as a pass criterion |

Every cell in the comparison table of the report is one of: a dated measurement, *not measured*, *not measurable* (with the reason), or *not applicable* (with the reason).

Results table, to be filled by the runs (this is the template, not data):

| Metric | Linux stage A | Zephyr stage A | Linux stage B | Zephyr stage B |
| --- | --- | --- | --- | --- |
| M-1 frame interval, median / max | not measured | not measured | not measured | not measured |
| M-1 outliers | not measured | not measured | not measured | not measured |
| M-2a submit-to-latch, median / p95 / max | not measured | not measured | not measured | not measured |
| M-2b latch-to-light | not measured | not measured | not measured | not measured |
| M-3 dropped / late | not measured | not measured | not measured | not measured |
| M-4 CPU | not measured | not measured | not measured | not measured |
| M-5 plane assignment | not measured | not measured | not measured | not measured |
| M-6 order violations | not measured | not measured | not measured | not measured |
| M-7 memory | not measured | not measured | not measured | not measured |

## 4. Measurement methods

### 4.1 Linux (A55, DRM/KMS)

Tools: `tools/drm-harness` with the scenarios of the M1 plan (`two-planes`, `zorder-alpha`, `move`, `rotate`), and `crates/composition-drm` examples. The harness's own `--help` wins over this text. If a needed flag is missing, the report says so; this plan does not add one.

| Metric | Method |
| --- | --- |
| M-1 | page-flip event timestamps from DRM events (`DRM_EVENT_FLIP_COMPLETE`, with the CRTC sequence number and the event's timestamp, using the monotonic clock). Interval between consecutive events. The measured refresh for the report's header is the median interval, as in M1 §9 |
| M-2a | harness `commit` call return timestamp to the page-flip event timestamp, both on the monotonic clock. The event timestamp is the kernel's vblank time, not the time the user process read it. Records the median, p95 and maximum |
| M-2b | not available from DRM events; see §4.3 |
| M-3 | harness counters `submitted`, `latched`, `dropped`, and late latches computed from M-1. The checker over the trace (M1 `rotate`) lists ordering violations |
| M-4 | process CPU time over wall time (`/proc/<pid>/stat` or `getrusage`) during the run, with frame-generation cost reported separately; the board's load average as context only. Weston state is recorded; the CPU load is that of the harness, with Weston stopped for a master-fd run as in M1 and restored afterwards |
| M-5 | the `commit` trace lines (layer, plane id, plane type, format, modifier, `zpos`); `validate` duration from the trace |
| M-6 | the trace checker |
| M-7 | `VmHWM` of the harness; the buffers' total size |

Linux runs are done twice: with DRM master after stopping Weston (as the M1 `m1-master-run`), and, if the M1 lease route works, through the lease. Which was used is stated in each result. Weston is restored and confirmed active at the end of every session.

For the Linux side the report also names the kernel, BSP, driver, and the number of planes that reach the LVDS CRTC from the M1 probe. If M1 has not produced these, the Linux column of stage B is *not measured* and the comparison does not start (§8).

### 4.2 Zephyr (M7)

Tools: upstream Zephyr only, as test applications under `tests/board/imx95_m7/` (a scanout application with latch timing hooks and, later, an engine-linked application; both are planned items). Whether the display driver is the stock `socionext,dpu` driver or a patched copy is stated in the report with its upstream commit; a patch is a deviation and is listed as such.

| Metric | Method |
| --- | --- |
| M-1 | the DPU's frame-complete / vertical-blank interrupt: in the handler, read the free-running cycle counter (the Cortex-M7 DWT `CYCCNT`, or a Zephyr cycle-counter driver or the system timer through `k_cycle_get_32()`; the application states which and its clock frequency, which is read from the running system, not assumed). Convert to time by the counter frequency, and take consecutive differences. This is the Zephyr equivalent of the page-flip event |
| M-2a | cycle counter when the display driver call that submits the buffer returns, to the cycle counter in the DPU interrupt that reports the buffer as taken. Which register or event means "latched" (shadow-load, frame-complete, or a stream status bit) is read from the DPU driver and reference manual and recorded as **read from source**; if the driver reports no latch event, M-2a is *not measurable* and the report says what the stock driver does expose |
| M-2b | not available from software; see §4.3 |
| M-3 | counters in the interrupt handler and in the submit path: `submitted`, `latched`, `dropped`, late latches from M-1. A missing interrupt is not counted as a latch (SoW §7); a watchdog timeout of one second without an event is an error |
| M-4 | a Zephyr thread analyzer or a measured idle-loop count against a calibrated reference loop; or the cycle counter accumulated in the interrupt and submit paths divided by the run's elapsed cycles. The method is chosen once and used for both stages. It measures the M7 only |
| M-5 | the application logs the DPU layer, stream and format programmed for each commit, from the driver's own configuration, not from the picture |
| M-6 | the same trace checker as Linux, run on the Zephyr log. The Zephyr application writes trace lines in the same format as the harness (`validate`, `commit`, `latched`, `released`, `dropped`) so the checker is shared |
| M-7 | the linker map (`text`, `data`, `bss`) and the framebuffer size |

How the trace leaves the M7 (a UART, or rpmsg through remoteproc, or a shared-memory log read from Linux) is part of the board bring-up and is decided by the feasibility doc and the M7 BSP study. The timestamps are cycle counts; they are converted on the host, so the M7 does no formatting in the measured path. The log buffer is filled in the measured path and written out after the run, so the console does not disturb the timing. If the console is used during the run, the report says so and M-4 is marked accordingly.

Timestamps from the two sides are never subtracted from each other: the M7 and A55 clocks are different counters. Comparison is of intervals and counts, not of absolute times.

### 4.3 Camera observation

The camera (target `imx95-camera`) follows the procedure in M1 §2 (180 degree rotation, saturated flat colours, steady light, three pictures per state, regions sampled by median, every picture kept).

It gives **qualitative** evidence on both sides, identically:

- the picture shows the expected plane layout, z-order and (where available) alpha response (M1 criteria, applied to Zephyr stage B as well);
- the move test's picture pair, to show that the stationary layer is untouched;
- no tearing or pattern mixing, with the frame marker strip.

It does **not** provide M-2b or M-1. Its frame rate and exposure are those of the camera tool, which are read from the resource description at the time of the run, not assumed here. If the tool cannot resolve one refresh period (as the M1 plan expects), the report says "latch-to-light: not measurable with this camera" and does not estimate it. A faster instrument (a photodiode on the panel, a high-speed camera) would be an addition and has to be named as such; none is assumed.

A camera picture is never used to establish plane assignment (the log decides), and it is never used to compare timing between Linux and Zephyr.

## 5. Pass criteria

Each stage and each side gets exactly one of **passed**, **failed**, **blocked**, with the evidence, using the M1 verdict style.

### 5.1 Stage A: one-plane scanout (the Zephyr M2 precondition)

Zephyr passes stage A when all hold, in a run of the stated length:

1. **Ownership is stated.** The report names which core owns the DPU, clocks, power domains, display route, interrupts and scanout memory in the boot flow used, from the feasibility doc, with its tags.
2. **Scanout is correct.** The camera shows the base colour at full panel, with the frame marker strip, with no tearing or partial frame, in at least three pictures; the framebuffer content readback matches what the application filled.
3. **Latch reporting works.** Every submission ends as exactly one `latched` or `dropped` (`latched + dropped = submitted`); latch events come from the DPU interrupt, not from a timer; M-6 is zero.
4. **No unsafe reuse.** No buffer is written between `submitted` and `released`; a buffer is not released before the flip that replaced it is reported (M1 §7.1 criteria, on the Zephyr trace).
5. **Timing is plausible.** The latch interval is within one refresh period ± 20 % except for listed outliers and dropped frames, with the same interval statistic as Linux. This is a plausibility criterion, not a performance target; a larger spread is reported, not failed, if it is explained, but a run with no events within one second fails.
6. **Hygiene.** The M7 is stopped afterwards, every changed setting is restored, Linux is back in its original state.

Linux passes stage A on the same criteria 2 to 5, using the M1 harness, plus the M1 plan's checks for the report's header items.

### 5.2 Stage B: overlay (the Zephyr M2 acceptance)

Runs only after stage A has passed on that side. Zephyr passes when:

1. The hardware, the driver and the resource assignment give a second independently supplied plane (the feasibility doc and the driver study say so, with tags). The stock driver is not assumed to.
2. The M1 condition 3 criteria hold on the Zephyr commit log and pictures: two distinct plane/layer identifiers, separate buffers, z-order as set, overlap showing the top layer, alpha response where the plane has alpha, the `move` test passing (no buffer writes after the first fill, stationary layer unchanged).
3. Unsupported configurations return explicit errors (SoW WP3), as in `combos` style cases: at least one refusal of a combination the hardware cannot do, with the reason, none committed.
4. The stage A event criteria (3 to 5 above) hold with two layers.
5. M-5 is recorded; the comparison table has every cell filled by a dated measurement or an explicit reason.

**Blocked, not failed:** if firmware, secure resource assignment or the driver prevents the access, the Zephyr result is **blocked**, with the reproducible investigation report and the reduced-scope bring-up that does run (SoW §9 M2, WP3). A flattened single framebuffer on one plane does not pass stage B (M1 rule, SoW §9). A pass on stage A is not described as an overlay pass.

### 5.3 The comparison itself

The comparison passes (as a deliverable, not as a performance contest) when:

- both sides ran the **same scene** (§2) at the same mode, and any difference in format, mode, buffer count or features is listed;
- the table in §3 has no empty cell: each is a dated measurement or one of *not measurable* / *not applicable* / *blocked*, with the reason;
- the differences in supported features and synchronization are listed explicitly: for example whether each side has a vblank interrupt, a latch event, plane alpha, mutable `zpos`, and which release semantics the driver gives (SoW WP3);
- no statement says one side is "faster" or "better" unless the measured intervals and counts support it for this scene on this board, and with the date.

There is no latency or CPU threshold the Zephyr side must beat. Linux numbers and Zephyr numbers are reported side by side; a threshold would be invented without data (SoW §6: no cost model before data).

## 6. Order of work

1. Linux stage A and B baselines come from the M1 runs; they are re-run here with the exact scene of §2 only if M1's scenes differ (`two-planes` and `move` are the same layout in spirit; the frame marker strip and the mode R/O definition are what this plan adds). The Linux harness may need a scene file or flags to match; that is a harness item, not part of this plan.
2. Ownership (§8) is settled in the feasibility doc.
3. Zephyr stage A: stock driver, one plane.
4. Linux and Zephyr stage A comparison table.
5. Zephyr stage B or the blocker report.
6. Stage B comparison, or the stage A comparison with the blocker as the stage B column.

## 7. Blocker report (replaces stage B)

If stage B cannot run, the deliverable replacing it contains: the stage reached, the exact condition that blocks (resource assignment, firmware, driver gap, interrupt not routed to the M7, the M7 not able to reach the scanout memory, and so on), the command or source line that shows it, the versions, and the reduced-scope bring-up that does run, with its own log. It does not claim the M2 acceptance (SoW §9) and does not substitute another SoC.

## 8. What cannot be measured until DPU ownership is settled

`docs/imx95-zephyr-feasibility.md` is the document that settles it. Until it states, for the boot flow used on this board, who owns the DPU and its resources and whether the M7 can be given the display route (alone or alongside Linux), these are **impossible** (not merely not yet done):

| Measurement | Why it waits |
| --- | --- |
| Any Zephyr result on the display (M-1 to M-6 on the Zephyr side, stage A and B) | it is not known that the M7 can reach the DPU registers, clocks, power domains, interrupt or scanout memory at all; the upstream devicetree only shows the DPU node for the M7, disabled by default, which is not evidence of an independent display route |
| The choice of the Zephyr time base and M-2a definition | depends on which interrupt the M7 receives and whether the driver exposes a latch event |
| Running Linux and Zephyr at the same time on one display (any "side by side" run) | depends on whether the route can be shared; the default of this plan is separate sessions |
| The same-mode condition (§2) | the mode the M7 can drive depends on who owns the LVDS or DSI clocks and the panel enable |
| Any CPU load comparison with Linux stopped or running | depends on whether Linux can keep running (Weston) while the M7 owns the display |
| Zephyr stage B | additionally requires hardware and a driver giving a second plane (not assumed from the devicetree) |
| Loading the M7 image and reading its log | gated in the feasibility doc by whether the M7 can be started without writing the rootfs, the boot environment or the eMMC boot partitions |

These **can** proceed before ownership is settled, because they need no M7 display access:

- the Linux stage A and B runs (they are the M1 runs and depend only on M1);
- host builds of the Zephyr test applications and the trace checker, and the host-side checks of the trace format against the fake backend;
- the reading of the DPU driver sources for the latch and interrupt semantics (tagged *read from source*, not as a result).

If the feasibility doc says the M7 cannot own the display on this boot flow, the Zephyr columns become **blocked** with that document as the evidence, and what remains is the blocker report (§7), the reduced-scope bring-up the doc allows, and the Linux columns.

## 9. Evidence layout

One directory per session, named by date and side (for example `m2/<date>-linux`, `m2/<date>-zephyr`), as the M1 plan §11: raw logs, pictures used and discarded, the command lines as typed, versions (kernel or Zephyr revision, toolchain, driver commit, board revision as read from the board, no serial numbers), the checker output, and a short note of anything unexpected. A log is never edited by hand. The report is `docs/m2-imx95-report.md` (to be written with the runs), with the §3 table filled.

## 10. Open points for later items

- The Linux harness needs the scene's frame marker strip and the mode R/O definition; whether that is a flag or a new scenario is for the harness owners.
- The Zephyr trace format must match the harness's trace lines for the shared checker; the Zephyr application items should take their format from the harness at the time of writing.
- The cycle counter's clock and the DPU's pixel clock are not assumed; both are read from the running system and recorded.
- A photodiode or high-speed camera for latch-to-light is an optional addition; none is in scope of the first comparison.
