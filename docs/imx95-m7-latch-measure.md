# Measuring scanout latch and VSYNC timing on the i.MX95 M7 under Zephyr

Status: design only. **Nothing here is measured.** No board was used, no
Zephyr image was built or run for this note, and no number below is a result.
Written 2026-11-28.

Every statement carries one of two tags:

- **read from source**: the upstream text named in section 1 says it.
- **inferred**: my reasoning from those sources or from general Cortex-M
  knowledge; it has to be checked on the board before it is relied on.

It supports milestone M2 (SoW §9) and WP3. The scene and metrics of the Linux
versus Zephyr comparison are in [m2-comparison-plan.md](m2-comparison-plan.md);
this note only designs the M7 side of metrics M-1 (frame interval), M-2a
(submit-to-latch) and M-3 (missed frames). Memory and cache rules for the
scanout buffer are in [imx95-m7-fb-memory.md](imx95-m7-fb-memory.md); the
M7 console and BSP facts are in [imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md);
the backend mapping is in [imx95-m7-backend-contract.md](imx95-m7-backend-contract.md).

## 1. Pinned sources

All are upstream Zephyr, repository `zephyrproject-rtos/zephyr`, annotated tag
`v4.5.0` (tag object `d6e5dc6ae358a6bbe05d040e0428e4161f09d006`, commit
`41d10b59730406bbab4ca9f1fbbe9e96aafb4966`), read 2026-11-28 from
`https://raw.githubusercontent.com/zephyrproject-rtos/zephyr/v4.5.0/<path>`.

| Path at v4.5.0 | Used for |
|---|---|
| `drivers/display/display_socionext_dpu.c` | the DPU driver: ISR, shadow load, `display_write` |
| `include/zephyr/drivers/display.h` | the display event-callback API |
| `arch/arm/core/cortex_m/timing.c`, `CMakeLists.txt`, `Kconfig` | DWT based timing functions |
| `drivers/timer/cortex_m_systick.c` | SysTick system timer and its cycle getter |
| `dts/arm/nxp/imx/nxp_imx95_m7.dtsi` | M7 clock, system timer, DPU node, interrupt numbers |

Caveat on the reading: the files were fetched through a page-reading tool that
returns text without line numbers, and for the driver the quotations were
checked by the tool, not by a local checkout. Citations below therefore name
functions and macros, not lines. A pinned local checkout (see
`scripts/` workspace preparation, when it lands) should re-confirm each quoted
identifier before the first board run. **Not verified locally.**

No `docs/trace-format.md` exists in the tree this note was written against
(the proposal is a separate, unmerged item). Section 6 therefore builds on the
event names the Linux harness already uses and says what to align once that
file exists.

## 2. What the stock driver exposes

All **read from source** (`display_socionext_dpu.c`, `display.h`, v4.5.0):

- The driver is `DT_DRV_COMPAT socionext_dpu`. Its API table has
  `set_pixel_format`, `write`, `get_capabilities`, `get_framebuffer`. It has no
  `blanking_on/off`, no `register_event_cb`, no `unregister_event_cb`, and
  `dpu_get_capabilities` does not set `supported_events`.
- `display.h` does define an event API: `DISPLAY_EVENT_LINE_INT`,
  `DISPLAY_EVENT_VSYNC`, `DISPLAY_EVENT_FRAME_DONE`,
  `DISPLAY_EVENT_FIFO_UNDERFLOW`; `struct display_event_data` carries a
  `uint64_t timestamp`; `display_register_event_cb()` returns `-ENOSYS` when
  the driver has no callback or `supported_events` is 0.
- The only DPU interrupt the driver enables is the ExtDst0 shadow-load
  interrupt (`kDPU_Group0ExtDst0ShadowLoadInterrupt`, enabled in `dpu_init`
  through `DPU_EnableInterrupts`). `dpu_isr` reads and clears the pending flags
  and, when that bit is set, does `k_sem_give()`. There is no vertical-blank
  interrupt use anywhere in the file.
- `dpu_write` starts the display lazily on the first call (`DPU_StartDisplay`),
  sets the fetch-unit source address (the caller's buffer if it is full-screen,
  stride-matched and 32-byte aligned, else a copy into a driver framebuffer),
  calls `dpu_trigger_content_stream_shadowload` and then blocks in
  `k_sem_take(..., K_FOREVER)` ("Wait for frame send to complete"). So the call
  is synchronous and cannot be made from an ISR.
- The driver's register block pointer in the ISR and in `DPU_EnableInterrupts`
  is a hard-coded macro (`APP_DPU_DC`), while the other calls use the base from
  the devicetree `reg`. The SoC dtsi puts the DPU node at a different address
  than that macro, as read. **Whether that matters on the Verdin is not known**;
  it is a reason to read the code on the board's tree before trusting any
  interrupt result.
- The SoC dtsi has `dpu` (`socionext,dpu`, interrupt 215) and `mipi_dsi`
  disabled by default, and no LVDS node was seen. The display interrupts go
  through an `nxp,irqsteer-intc` node whose master lines map to NVIC lines.

Consequences for the design (**inferred**):

1. The stock driver can report "a shadow load finished" only as a semaphore
   release inside `display_write`. That is M-2a's *end* event, but the M7
   application cannot see it without either timing the return of
   `display_write` (which adds thread wake-up latency) or running a patched
   driver copy with a timestamp in `dpu_isr`.
2. M-1 (interval between `latched` events) needs the interrupt time, so it
   needs the patched copy or a wrapper at the ISR. A patch is a deviation and
   has to be listed with its upstream commit, as m2-comparison-plan.md already
   requires.
3. Whether the ExtDst0 shadow-load interrupt coincides with the start of
   scan-out of the new buffer, with the vertical blanking, or with the end of
   the previous frame is **not established**. It has to be read from the
   reference manual and the fsl_dpu sources, and then decided by the external
   reference in section 4. Until then the event is called `latched` in logs
   only as the name of the harness event, with the hardware meaning
   *unconfirmed*.
4. `DISPLAY_EVENT_FRAME_DONE` is the nearest standard event. If a patched
   driver adopts the event API it should use that and fill
   `display_event_data.timestamp`, but the timestamp's clock is the driver's
   choice; the application must still take its own cycle count.

## 3. Time sources on the Cortex-M7

### 3.1 Candidates

| Source | How it is read | Resolution and wrap | Tag |
|---|---|---|---|
| DWT `CYCCNT` | `CONFIG_TIMING_FUNCTIONS` builds `arch/arm/core/cortex_m/timing.c` when `CONFIG_CORTEX_M_DWT` is on (`CORTEX_M_DWT` defaults to y with `TIMING_FUNCTIONS`); `arch_timing_counter_get()` returns `z_arm_dwt_get_cycles()`. Application API: `timing_init()`, `timing_start()`, `timing_counter_get()`, `timing_cycles_to_ns()` | one core clock cycle; 32-bit | read from source |
| DWT frequency | `z_arm_dwt_freq_get()` uses `SystemCoreClock` if the CMSIS core header is present, else `sys_clock_hw_cycles_per_sec()` with `CONFIG_CORTEX_M_SYSTICK`, else calibrates against `k_cycle_get_32()` | the M7 core clock | read from source |
| `k_cycle_get_32()` | the system timer driver's cycle getter. For SysTick: `cycle_count + elapsed(NULL)` in `cortex_m_systick.c`, built on the generic timer core | SysTick clock; 32-bit | read from source (the generic core was not read; `sys_clock_cycle_get_32` there is **inferred**) |
| System timer of the SoC | the dtsi chooses `zephyr,system-timer = &lptmr1`, a 32768 Hz LPTMR, `status = "disabled"` in the SoC file; a board file enables what it uses | 32768 Hz is one tick per about 30.5 µs | read from source for the node; the board file's choice was not read |
| Dtsi M7 clock | `cpu0` `clock-frequency = <DT_FREQ_M(800)>` | 800 MHz nominal | read from source |

Arithmetic from the nominal clock (**inferred**, plain division): at 800 MHz a
32-bit counter wraps every 2^32 / 800e6 ≈ 5.37 s, and one 60 Hz frame is about
13.3 million cycles. A 32-bit `CYCCNT` is therefore enough for one frame
interval and for any single submit-to-latch span, provided the log stores raw
32-bit values and the host unwraps them (consecutive differences, modulo 2^32).
The 30.5 µs tick of the 32768 Hz timer is too coarse for jitter work on a
16.7 ms frame; it is useful only as a coarse watchdog (section 5).

### 3.2 Choice for the first run (proposed, **inferred**)

- Primary: DWT `CYCCNT`, read in the ISR as the very first statement
  (`timing_counter_get()` or a direct read of the counter; if a direct read is
  used, enabling `DWT->CTRL.CYCCNTENA` and unlocking via the lock access
  register is the application's job; the timing API does it).
- Secondary, for cross-check: `k_cycle_get_32()` read at the same points in
  the same handler. Two counters that disagree beyond their nominal ratio
  point at a wrong clock assumption.
- The clock frequency is **read from the running system** (the SCMI clock
  rate the M7 observes, or `SystemCoreClock`, and the board's own report),
  logged in the run header, and never taken from the 800 MHz dtsi figure. The
  dtsi value is only a nominal default. Neither the real M7 clock on this
  board nor the SCMI rate is known yet: **not measured**.
- Whether `CYCCNT` keeps counting while the core waits in `WFI` or is held by
  a debugger is **not established** here (it depends on the SoC's clock gating
  of the trace/debug block). The application should avoid sleep during a run,
  or the first board run should check it by reading the counter across a
  `k_busy_wait` and a `k_msleep`. Treat as **inferred**, to be tested.

## 4. Checking the timestamps against an external reference

The cycle counter only says when the M7 *noticed* the interrupt, in M7 clock
cycles. Three independent checks are possible; each answers a different
question. None has been run.

### 4.1 GPIO toggle on the M7 (primary)

- In the ISR, right after the timestamp read, toggle a GPIO the M7 owns.
  Which pin is usable on the Verdin carrier, and whether the System Manager
  gives it to the M7, is **not known**. The NXP EVK configuration gives the M7
  two GPIO pins and LPUART3 (**read from source**, recorded in
  [imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md)); that is an EVK statement,
  not a Verdin one. The pin choice belongs to the board bring-up, from the
  lease description, not from this file.
- A logic analyser on that pin gives the ISR-entry time on an independent
  clock. Comparing the analyser's edge spacing with the CYCCNT differences
  checks the cycle counter's *frequency* (the ratio over many frames) and
  the ISR's own jitter. It does **not** check the hardware meaning of the
  event.
- A camera cannot resolve the ISR time. It can only show which frame the
  panel displayed when a second GPIO (for example an LED driven from the
  same toggle) changed, with the camera's own frame period as resolution. The
  resolution is read from the camera tool when used, not assumed; if it
  cannot resolve a refresh period the report says "not measurable with this
  camera", as m2-comparison-plan.md already states.
- The display itself provides the other half of the check: the panel's
  timing signals. If the LVDS or DSI timing is reachable by a probe, edges of
  the data-enable or the first line against the GPIO edge show the offset of
  the event from the true vertical blank. Whether that is reachable on this
  carrier is **not known**.

### 4.2 Linux-side DRM vblank timestamps (only when both run)

- On Linux the harness uses the kernel's vblank timestamp in the page-flip
  event as the `latched` time (m2-comparison-plan.md, M-1 and M-2a).
- Comparing Linux and M7 timestamps needs a common clock. Both cores run
  different clocks in different software; subtracting their raw counters is
  meaningless. A shared hardware counter that both can read may exist on the
  SoC (**inferred**, not read from any source in this tree). Without one, the
  comparison is statistical: interval distributions side by side, not
  per-frame offsets.
- The DPU and its LVDS route are owned by one side at a time unless the System
  Manager configuration says otherwise; the ownership study is in
  [imx95-sm-display-lm.md](imx95-sm-display-lm.md). If that holds (**inferred**
  from those documents, not a finding of this note), Linux and the M7 never
  scan out on the same DPU in the same boot, and the Linux vblank times come
  from a separate run of the same scene. **Both running against the same
  panel is therefore not assumed.** A per-frame cross-check between the two
  stacks is possible only in a configuration that does not exist yet.
- Linux can still serve as a *reference for the panel's refresh period*: the
  mode's nominal period and the measured vblank interval on a separate run
  give the figure the M7's interval is compared against. **Not measured.**

### 4.3 What each check can and cannot claim

| Claim | GPIO + analyser | Camera | Linux vblank (separate run) |
|---|---|---|---|
| CYCCNT frequency is right | yes (over many frames) | no | no |
| ISR entry jitter | yes | no | no |
| Event is the true vertical blank | only with a panel-side probe | coarsely, by frame | no |
| Latch-to-light latency (M-2b) | no | at camera resolution only | no |
| Refresh period of the mode | yes | coarsely | yes |

## 5. Expected jitter and error sources

All **inferred**; none is measured. Each has a way to bound it.

| Source | Effect | How to bound it |
|---|---|---|
| Interrupt latency: other IRQs, the NVIC priority of the DPU line (the DPU `interrupts` cell is priority 0 in the dtsi, read from source), `irq_lock()` regions, tail-chaining | delay between the DPU event and the first instruction of the ISR; one-sided (always late) | read CYCCNT first in the ISR; measure the same path with a timer interrupt of known period |
| Cache and DDR/OCRAM bus contention on the ISR's own fetch | variable ISR entry time on a cached-M7 | keep the handler and vector in TCM if the board has it, and state where the code runs |
| Reading `display_write`'s return instead of the ISR | adds semaphore wake-up and scheduler latency, tens of microseconds or more | take the timestamp in the ISR (patched driver copy), and log both so the difference is itself a result |
| Clock domains: the DPU pixel clock (derived from a PLL, set through SCMI) and the M7 core clock are not the same oscillator path | slow relative drift of frame interval versus CYCCNT time; not noise, a scale error | compare the analyser's frame spacing with the CYCCNT spacing over thousands of frames |
| Wrong frequency assumption | every time conversion scales | read the rate at run time, log it, cross-check with the second counter and the analyser |
| 32-bit wrap | a spurious interval | host unwraps modulo 2^32; the application logs at least one entry per wrap period (5.37 s at the nominal clock) |
| Shadow-load semantic mismatch | a consistent offset (event is not the vertical blank) | the panel-side probe or the camera; stated as an offset *not measured* until then |
| Frame skipped or merged by the DPU | an interval of two periods, a `dropped` event | count it, never average it away; a missing interrupt is not a latch (SoW §7), as in m2-comparison-plan.md |
| Logging in the measured path | adds cycles between events | store raw counts in a RAM ring and write them out after the run (section 6) |
| Cache maintenance on the buffer (see [imx95-m7-fb-memory.md](imx95-m7-fb-memory.md)) | extends the submit side of M-2a | timestamp *after* the clean and say so |
| SysTick or LPTMR tick interrupts firing during the span | small extra delay on either edge | note the tick rate; keep the measured ISR at higher priority than the tick if the board allows |
| Temperature and PLL spread | small slow drift of the pixel clock | record the date and, if available, the board temperature; do not generalise |

A watchdog is separate from the measurement: if no frame event arrives for one
second, the run fails (m2-comparison-plan.md §M-3). The coarse 32768 Hz timer
or `k_uptime_get()` is enough for that.

## 6. Log format

### 6.1 Alignment with the trace format

`docs/trace-format.md` does not exist in the tree this was written from, so
nothing here is read from it. The Linux harness's trace events are `validate`,
`commit`, `latched`, `released` and `dropped`, and the M2 plan requires the
Zephyr application to emit the same names so one checker reads both
(m2-comparison-plan.md, M-6). **Read from source** (that plan and the
harness's scenario and session code). When the trace-format proposal merges,
this section is to be replaced by a pointer to it; the field set below is a
provisional shape, not a format.

### 6.2 Provisional line shape

The M7 does not format text in the measured path. It writes fixed-size binary
records into a RAM ring buffer, then prints text after the run; the host
converts counts to time (m2-comparison-plan.md says the same).

Record, **proposed (inferred)**: `seq` (u32), `event` (u8: validate, commit,
latched, released, dropped, plus `isr` for a raw interrupt entry), `buf` (u8
buffer index), `cyc` (u32 DWT count), `kcyc` (u32 `k_cycle_get_32()`), `flags`
(u8: bit for "patched-driver ISR stamp" versus "stamp after `display_write`
return").

Text after the run, one record per line, key=value, no board address, host name
or device path:

```
# run=<date> clk_hz=<read at run time> clk2_hz=<read at run time> src=dwt,kcyc ring=<n> driver=<upstream commit; patched yes/no>
ev seq=17 event=commit buf=1 cyc=123456789 kcyc=123456 flags=0
ev seq=18 event=latched buf=1 cyc=135789123 kcyc=135789 flags=1
# end wraps=<n> overrun=<n> events=<n>
```

The two `<read at run time>` values are not known now (**not measured**). The
header states which clock each count is in; a log without it is invalid. The
host script derives M-1 (differences of `latched`), M-2a (`latched` minus the
matching `commit`), and M-3 (counts) and prints them with a date.

### 6.3 Transport

The log leaves the M7 after the run by the M7 console UART, or a shared
buffer read from Linux, or rpmsg; the choice is made in the bring-up
([imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md) and the rpmsg design note,
when merged). Writing to the console during the run changes the timing and the
run must then be marked so (M-4 in the comparison plan).

## 7. What this note does not establish

- No latch, VSYNC or interval figure of the i.MX95. All **not measured**.
- That the M7 can own the DPU on the Verdin at all; see
  [imx95-sm-display-lm.md](imx95-sm-display-lm.md) and the feasibility work.
  Until the ownership is shown, everything in sections 3 to 6 is a plan.
- That the ExtDst0 shadow-load interrupt is the latch or vertical blank in
  the hardware sense (section 2, point 3).
- The M7's real clock, the counter behaviour in sleep, the availability of a
  GPIO, or a shared Linux/M7 time base.
- Any parity with another SoC: the STM32MP257F-DK latch records
  ([observed/](observed/)) are format references only; no number transfers.

## 8. Items this suggests (for the planner, not done here)

1. Re-read the five pinned files in a local checkout and add line numbers.
2. Decide patched-driver ISR stamp versus event API, and record the patch as
   a deviation with the upstream commit.
3. Read the i.MX95 reference manual for the ExtDst0 shadow-load and vertical
   blank semantics, and the hard-coded base address question.
4. Choose the GPIO and the logic-analyser setup from the lease description.
5. Check whether a Linux/M7 shared counter exists, from source.
6. Align section 6 with `docs/trace-format.md` when it lands.
