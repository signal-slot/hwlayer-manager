# Mapping i.MX95 M7 DPU events onto the trace format (M2-07)

Design note, written 2026-11-29. **Nothing in this note is measured.** No
board was used, no Zephyr image was built or run for it, and no timestamp,
interval or count below is a result. It is no hardware claim and no claim
from another SoC.

Tags: **read from source** (a repository file named in the text, read on
2026-11-29 at the integration tip) and **inferred** (my reasoning from
those files; to be checked on the board). There is no **observed**
statement here. Event names are those of [trace-format.md](trace-format.md)
(`ev=` values of its §3.2) unless marked **proposed addition**.

Files read and not changed: [trace-format.md](trace-format.md),
[imx95-m7-latch-measure.md](imx95-m7-latch-measure.md),
[imx95-m7-dpu-irq.md](imx95-m7-dpu-irq.md),
[m2-comparison-plan.md](m2-comparison-plan.md),
[m2-linux-zephyr-comparison.md](m2-linux-zephyr-comparison.md),
[m2-linux-scene.md](m2-linux-scene.md), the Linux scene example
`crates/composition-drm/examples/m2scene.rs`, `scripts/trace-to-metrics.py`,
`tests/board/imx95_m7_scanout/src/main.c` and
`tests/board/imx95_m7_engine/src/main.c`. The in-review scene app
`tests/board/imx95_m7_scene/` is **not on the tip** (the directory does not
exist), so it was not read; where this note says what that app should emit
it is **inferred** and has to be re-checked against it when it lands.

## 1. What the M7 apps emit today (read from source)

| App | Emits | CETRACE lines? | Timestamps |
| --- | --- | --- | --- |
| `imx95_m7_scanout` | CSV `frame,write_start_cyc,write_return_cyc,write_us,return_to_return_us` from `display_write()` (`main.c`, `printk`) | no | `k_cycle_get_32()` before and after `display_write`, which blocks on the ExtDst0 shadow-load interrupt and returns after it |
| `imx95_m7_engine` | scene verdict text, `plan:` and `event:` lines, `RESULT: PASS\|FAIL`; its backend reports the latch at once and programs nothing | no | none |

So no M7 app writes `CETRACE` lines yet; `trace-format.md`'s header says the
same of every producer. Every M7 row of §2 is therefore a **proposal** for
what a producer should write, not a description of existing output. The
engine app shows the engine side of the M7 can plan (`ce_present`,
`ce_poll_event` with `presented` / `dropped` / `released` events); the
scanout app shows the hardware side can only offer "write returned" as a
latch signal with the stock driver (read from source; see also
[imx95-m7-latch-measure.md](imx95-m7-latch-measure.md) §2).

## 2. Event by event

The Linux column is what `m2scene.rs` writes (read from source, its module
doc and `Entry` code) and what [m2-comparison-plan.md](m2-comparison-plan.md)
§4.1 names. "Clock" is the `hdr` `clock=` value
([trace-format.md](trace-format.md) §3.5).

### 2.1 Engine-side events (no hardware involved)

| M7 event | `ev=` and fields | Clock for `t` | Linux equivalent | Like for like? |
| --- | --- | --- | --- | --- |
| Engine plan / advise result, accepted | `validate` with `out`, `layers`, `result=ok`, `n`, `assign=`; optional `dur` | `cyc32` (`k_cycle_get_32()`) or `cyc64` | the same line from `m2scene` (`dur` in ns) | Assignment: yes. The `assign=` list is the planner's output on each side's own capability table; equal lists mean equal plans (the scene is pinned by `m2_scene.rs`). `dur`: no, see §3.1 |
| Engine plan refused | `validate` with `result=plan.no_plane layer=N reasons=0x..` or another `plan.*` code | as above | the same code on Linux (for example `reasons=0x10`, alpha, on vkms per m2-linux-scene.md) | yes as a statement of refusal; the *reason bits* differ by topology and are the point of the comparison (stock Zephyr refuses stage B) |
| Planner candidate tried | `check` with `out`, `n`, `assign=`, `result=ok\|backend.*` | as above | `m2scene` leaves `check` out (m2-linux-scene.md) | not compared; Linux writes none |
| Resource assignment used by a hardware commit | the last `validate ok` before the `commit ok` of that output (trace-format.md §7, **inferred** there) | n/a | same rule | yes, same inference on both sides |

### 2.2 Commit and latch events (hardware involved)

| M7 event | `ev=` and fields | Clock | Linux equivalent | Like for like? |
| --- | --- | --- | --- | --- |
| Commit submitted (the backend `commit` returned `CE_OK`; for the stock driver, `display_write()` was entered) | `commit` with `out`, `result=ok sub=<id>` or `result=backend.*`; optional `dur` | `cyc32`; `t` stamped when the sink runs, after the call returns (trace-format.md §4) | `commit` line; `t` = `CLOCK_MONOTONIC` ns after the atomic commit returns | Same *position* (the return of the submit call). Different cost content: a non-blocking atomic commit versus a driver that may block; see §3.2 |
| Shadow-register load finished (ExtDst0 shadow-load interrupt, the stock driver's only enabled event) | `latched` with `out`, `sub`, and `tev` = cycle counter read **first** in the ISR (**proposed**: the stock `dpu_isr` has no timestamp or callback, so this needs the patched driver copy or a wrapper at the ISR, latch-measure §2 and §3.2) | `cyc32` | `latched` with `tev` = the page-flip event's vblank time (m2scene `Entry`, plan §4.1 M-1) | **Not like for like** as hardware events, see §3.3 |
| `display_write()` returned (stock driver, no patch) | `latched` with **no** `tev`; the sink's `t` is then the time the thread woke up, and `trace-to-metrics.py` falls back to `t` when `tev` is absent (read from source, `scripts/trace-to-metrics.py` lines for `latch_at`) | `cyc32` | none | No. It adds semaphore wake-up and scheduling delay (latch-measure §5); a run in this form must say so in a comment line and is a weaker result |
| Frame complete / vblank (DisEngCFG frame complete, the event Linux uses for vblank; **needs unmasking and ISR support the stock driver lacks**, dpu-irq §2.2, §6) | **proposed addition**: no `ev=` name exists for a raw vblank. Options in §4 | `cyc32` | Linux counts it as the CRTC vblank and carries its counter in `vseq` on `latched` | Linux's `latched` time **is** the vblank time (inferred from `tev` being the vblank timestamp); the M7's `latched` is the shadow load. Different events, see §3.3 |
| Frame replaced before latch | `dropped` with `out`, `sub`, `reason=replaced` | `cyc32` | same, produced by the engine on both sides (core `Dropped`, trace-format.md §2.1) | yes (the engine decides, not the hardware), provided the M7 backend reports `replace_pending` the same way; the engine app sets `replace_pending = 0` (read from source), so replacement is a different code path there |
| Buffer free again | `released` with `buf`, `sub` | `cyc32` | same | yes, engine-defined; its time is the sink's `t`, not a hardware time |
| Latch did not arrive in the watchdog time | `latch_timeout` with `out`, `sub` | `cyc32` or `uptime_ticks` | same | yes as a count; the timeout *length* is each side's own setting and must be recorded |
| Device lost | `device_lost`, `dropped reason=device_lost`, `released sub=-` | as above | same | not expected on either side in the scene |

Header line (**proposed** values for the M7, inferred from
trace-format.md §3.5): `src=zephyr`, `clock=cyc32` (or `cyc64`),
`freq_hz` **read from the running system** (never the 800 MHz of the
devicetree), `t0` the counter at the header, `at_unit=same` if the app
passes the platform stamp to `advance_time`, `board=` free text without any
address, host or device path. A real log also needs the `# date:` line of
trace-format.md §5.1, written by whoever collects it.

## 3. Which comparisons are like-for-like

All of this section is **inferred** from the files named; nothing is
measured.

### 3.1 Like for like

- **Counts and ordering** (M-3, M-6): `commit ok`, `latched`, `dropped`,
  `latch_timeout`, `released` come from the same engine on both sides, so
  `latched + dropped + pending = commit ok` and the ordering rules of
  trace-format.md §8 apply to both logs unchanged. The M7's `latched` may be
  a different hardware event (§3.3) but is the same *engine* event.
- **Plan and assignment** (M-5): the `assign=` list and the `plan.*` refusal
  codes, given the capability tables each side declares. Stage A is
  comparable only after the format substitution of m2-linux-zephyr-comparison
  §2 (`ARGB8888` on both sides); stage B is refused on stock Zephyr and is
  *not applicable* there, not a slower run.
- **Intervals inside one log**, expressed in time by the log's own
  `freq_hz` (M-1 on each side separately). The plan's rule stands: intervals
  and counts are compared, absolute times never are, because the Linux
  `monotonic_ns` and the M7 cycle counter have no common origin
  (trace-format.md §5, m2-comparison-plan.md §4.2).

### 3.2 Not like for like (state the difference, do not rank)

- **`commit dur` and commit-to-latch latency (M-2a).** Linux submits a
  non-blocking atomic commit and later receives a page-flip event; the stock
  Zephyr driver's `display_write` blocks until the shadow load is done. A
  `dur` or submit-to-latch value from the stock driver includes the wait
  itself, so it measures a different thing than the Linux number even with
  the same field names. Only a patched, non-blocking submit with the latch in
  the ISR (a deviation to be recorded with its upstream commit,
  latch-measure §2) makes `commit` to `latched tev` the same *kind* of span.
- **`latched` hardware meaning.** Linux: the DRM page-flip event time, the
  kernel's vblank timestamp. M7: the ExtDst0 shadow-load interrupt, which
  Linux's own source orders *before* DisEngCFG frame complete "about a time
  duration of an entire vblank" (dpu-irq §2.2). Whether the shadow-load
  interrupt coincides with a vblank is not established (latch-measure §2,
  point 3). Until it is, an M7 `latched` is "shadow load done", and M-1 and
  M-2a differ from Linux by an unknown, possibly constant, offset. Constant
  offsets do not change interval statistics (M-1); they do change M-2a.
- **Timestamp origin.** Linux `tev` is a kernel timestamp taken at the
  vblank (interrupt) and passed through a DRM event; the M7 `tev` is a
  counter read at ISR entry, late by the interrupt latency (latch-measure §5).
  The Linux timestamp is closer to the hardware event than the M7 one by
  construction.
- **Clock resolution and wrap.** `monotonic_ns` has 1 ns units; `cyc32`
  wraps (about 5.4 s at the nominal 800 MHz, latch-measure §3.1, an
  arithmetic on a nominal figure); the host unwraps. The conversion is
  integer on the host for both (trace-format.md §5).
- **`dur` of `validate`.** Linux measures it with `CLOCK_MONOTONIC` around the
  call on an A55; the M7 would measure it with the cycle counter on an M7.
  Different cores, caches and clocks; plan time is not comparable between
  them (m2-comparison-plan.md M-4 says the same of CPU load).
- **M-2b latch-to-light**: not in any trace on either side.
- **CPU load and memory** (M-4, M-7): not trace events.

### 3.3 The one decision this note asks for

Which M7 event is written as `latched`: the shadow load (the only event the
stock driver enables) or the frame complete. Recommendation (**inferred**):
write the shadow-load event as `latched`, because it is the engine's
"submission is now what the hardware will show" and it is what exists, and
add the frame-complete as a separate, clearly named record (§4) once a
driver can raise it. Whatever is chosen is stated in the log's comments and
in the report, so no reader takes the M7 `latched` for a vblank.

## 4. Fields and events not in trace-format.md (proposed additions)

`m2scene.rs` already writes two things the format does not define, and
`trace-format.md` §3.1 says its crate's parser rejects a key it does not
list (D2), so the M7 side must not simply copy them (read from source):

| Item | Where it exists | Status |
| --- | --- | --- |
| `vseq=<n>` on `latched` (CRTC vblank counter) | `m2scene.rs` | **proposed addition**. M7 equivalent: a frame counter incremented in a frame-complete ISR, if one exists; absent otherwise |
| `vbl_ns=` instead of `tev=` and `clock=rel_ns` | `m2scene.rs`, when its monotonic time is unreadable | **proposed addition**; the M7 does not need it |
| a raw vblank record, e.g. `ev=vblank out= seq= t= tev= vseq=` | nowhere | **proposed addition**. Without it the frame-complete interrupt has no line; with the stock driver there is none to write anyway |
| `ev=isr_overrun` or a ring-overrun marker | nowhere | **proposed addition**; today a ring overrun would show as a `seq` gap (trace-format.md §3.3), which the checker reports. Latch-measure §6.2 wants `overrun=` in a final `end` comment, which is a `#`-style comment line, not an event |

`trace-to-metrics.py` reads only `ev`, `seq`, `t`, `at`, `tev`, `dur`, `out`,
`sub`, `buf`, `assign` and tolerates other keys (it parses `key=value`
tokens and ignores the rest), so `vseq` does not break it, but the strict
crate parser would refuse it (read from source, both). Whether to extend the
format or to drop `vseq` from the Linux example is for the owner of
`trace-format.md` and the example; this note does not change either.

## 5. Consumption by the comparison scripts

Read from source, `scripts/trace-to-metrics.py`: M-1 is the differences of
`tev` (else `t`) of consecutive `latched` per output; M-2a is `tev` (else
`t`) of `latched` minus `t` of the matching `commit`; M-3 counts `commit ok`,
`latched`, `dropped`, `latch_timeout` and pending. Consequences
(**inferred**):

1. An M7 log without `tev` still converts, but its M-1 and M-2a then use
   the sink's `t` and carry the wake-up delay of §2.2. The log should say so
   in a comment line; the script does not.
2. The `hdr` `freq_hz` is what converts the M7's counts; a wrong or assumed
   value scales every M7 figure, so it must be read at run time.
3. The `# date:` line and the "not a hardware result" line decide whether
   the output counts as a board result (trace-format.md §5.1); a QEMU,
   `native_sim` or dummy-display log is never one
   (m2-linux-zephyr-comparison.md §5).

## 6. Not measured, and what is left

Not measured: every timestamp, interval, latency and count named above, on
either side; the M7 clock frequency; the interrupt latency; the offset
between shadow load and vblank; whether the M7 can own the DPU and receive
these interrupts at all (the ownership questions of
[imx95-m7-dpu-irq.md](imx95-m7-dpu-irq.md) §5 and §7). The Zephyr columns of
the comparison stay blocked until those are settled; this note only fixes how
the lines would be written once they are.

Left for other items:

1. Check §2 against `tests/board/imx95_m7_scene/` once it merges.
2. Decide the §3.3 question and the §4 additions with the owners of
   `trace-format.md` and `m2scene.rs`.
3. A patched DPU driver with an ISR timestamp (or an event-API
   `DISPLAY_EVENT_FRAME_DONE` with `timestamp`), recorded as a deviation.
4. A GPIO or analyser cross-check of the counter frequency
   (latch-measure §4), if the board gives the M7 a pin.
