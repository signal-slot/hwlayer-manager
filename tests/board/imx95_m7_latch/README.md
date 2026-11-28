# imx95_m7_latch

Zephyr test application for latch reporting from the i.MX95 Cortex-M7 DPU
(SoW milestone M2, WP3; item M2-04 of the M2 list). It implements the
latch path that [`docs/imx95-m7-latch-measure.md`](../../../docs/imx95-m7-latch-measure.md)
and [`docs/imx95-m7-overlay-design.md`](../../../docs/imx95-m7-overlay-design.md)
§3 and §5 describe:

- a **non-blocking commit** with a **timeout** on the latch,
- **latches counted from the ExtDst0 shadow-load interrupt**,
- the **FrameGen0 frame counter** (`FGTIMESTAMP`) read at each commit and latch,
- **cycle-counter timestamps** (DWT `CYCCNT` and `k_cycle_get_32()`),
- the verdict lines below.

**Status: not run on any board; no latch time measured.** Only host builds
were done (section "Host build results"). Nothing here says that the M7 can
own the DPU, that the interrupt reaches the M7, or what the ExtDst0
shadow-load event means in time. Those are open; see
[`docs/imx95-zephyr-feasibility.md`](../../../docs/imx95-zephyr-feasibility.md)
and [`docs/imx95-m7-dpu-irq.md`](../../../docs/imx95-m7-dpu-irq.md).

## Who owns the DPU

Do not run this image with the DPU option on a board where Linux owns the
DPU, for example the Verdin iMX95 under its Linux BSP. It programs the whole
DPU pipeline and would corrupt Linux's display. Turn the option on only after
the DPU has been shown to belong to the M7 in the boot flow used.

`CONFIG_IMX95_M7_LATCH_ENABLE_DPU` is `n` by default. With it off, the image
contains no DPU code (the HAL source is not even compiled) and `main()` only
prints a refusal and `VERDICT imx95_m7_latch: SKIP DPU access disabled at build
time`. The upstream `socionext,dpu` driver is never built, in either
configuration: the `Kconfig` file sets `DISPLAY_SOCIONEXT_DPU` to `n` and
`main.c` checks that at build time, because the driver connects the same
interrupt line and blocks in `display_write()`.

## How it works (read from source, Zephyr `e53873cbd0ef`, hal_nxp `1a8937f3f8df`)

Upstream Zephyr is not changed. The application programs the DPU itself
through the NXP MCUX DPU HAL (`hal_nxp`, `mcux/mcux-sdk-ng/drivers/dpu_1/fsl_dpu.c`),
which `CMakeLists.txt` compiles into the application unchanged when the option
is on. (Zephyr only builds that HAL for its own driver.)

1. **Set-up** is the same call sequence as the upstream driver's `dpu_init()`
   (`drivers/display/display_socionext_dpu.c`): ConstFrame0 (black) and
   FetchYUV0 (the buffer) into LayerBlend1, into ExtDst0, DomainBlend0 and
   FrameGen0, timings from the devicetree. The sync polarity mapping is copied
   unchanged from the driver. Before the interrupt is enabled, a stale ExtDst0
   shadow-load flag is cleared.
2. **Commit** (`commit()`), with interrupts locked: refuse with `-EBUSY` if a
   commit is pending; start FrameGen0 on the first commit (as the driver does
   on its first write); write the FetchYUV0 address; trigger the ExtDst0
   pipeline shadow load; append a `commit` record with the time stamps and
   return. It does not wait.
3. **Busy rule.** Right after each commit, in the same locked section, a second
   commit is tried and must return `-EBUSY` (no replacement of a pending
   commit, overlay design §3 point 2). This checks the software state machine,
   not the hardware.
4. **Latch** (`latch_isr`, on the `dpu` node's interrupt, NVIC 215 in the
   SoC dtsi): read DWT `CYCCNT` and `k_cycle_get_32()` first, then the
   interrupt group-0 status and `FGTIMESTAMP`. If the ExtDst0 shadow-load bit
   (`kDPU_Group0ExtDst0ShadowLoadInterrupt`, bit 3) is set, clear **only that
   bit**, record `latched` for the pending commit and post it to a message
   queue. A latch bit with no pending commit is counted as unexpected.
5. **Wait** for the latch message with `CONFIG_IMX95_M7_LATCH_TIMEOUT_MS`
   (default 1000 ms). On a latch, the buffer that was on screen is
   `released`. On a timeout the application reads the status register once
   more, records `latch_timeout` and ends the run: a missing interrupt is never
   counted as a latch.
6. After the run: disable the interrupt, stop FrameGen0, then print the log.

Two full-frame ARGB8888 buffers are filled once (two colours, a white square at
a different place in each), cleaned from the D-cache once, and never written
again. Commits alternate between them; one commit is issued per latch.

`FGTIMESTAMP` has no HAL accessor. The application reads FrameGen0 + `0x98`
(the offset of `FGTIMESTAMP` in the HAL's private `DPU_FRAMEGEN_Type`) and
decodes the frame index as bits 31..14 (18 bits, wraps after `0x3ffff`) and
the line index as bits 13..0. This matches the NXP Linux `dpu95` driver (read
from source 2026-11-28, `nxp-imx/linux-imx` commit `5a0a5e71d2bd`:
`dpu95-framegen.c` defines `FGTIMESTAMP 0x98`, `FRAMEINDEX_SHIFT 14`,
`LINEINDEX_MASK 0x3fff`; `dpu95.h` defines
`DPU95_FRAMEGEN_MAX_FRAME_INDEX 0x3ffff`). The raw value is printed too
(`fgts=`), so the host can decode it again. That the M7 view of the register
behaves the same is not checked on hardware.

Open points the code does not settle (inferred, from the source studies):

- The interrupt registers are reached at the devicetree `reg` of `dpu`
  plus `CONFIG_IMX95_M7_LATCH_IRQ_BANK_OFFSET` (default `0x0`, the devicetree
  base, as the feasibility doc asks). The upstream driver uses a hard-coded
  base `0x380000` higher; set the option to `0x380000` to try that. Which is
  right is not known.
- Nothing here enables the display IRQSTEER or its channel. If the M7 never
  gets the interrupt, the timeout path reads the status register and gives
  `BLOCKED` instead of `FAIL` (see the verdicts).
- No clock, power domain, pixel link, LDB or panel is set up, as in the
  upstream driver.

## Output

All output after the run, so the console does not disturb the measured path.
Records are kept in a RAM ring of `CONFIG_IMX95_M7_LATCH_RING` entries (a
build-time check makes it hold three records per commit). Lines follow the
`CETRACE v=1` proposal in [`docs/trace-format.md`](../../../docs/trace-format.md),
with extra fields that the format allows (a parser ignores unknown fields):

```
CETRACE v=1 ev=hdr seq=0 src=zephyr clock=cyc32 freq_hz=<k_cycle Hz> t0=<k_cycle> at_unit=same dwt_hz=<DWT Hz> mode=640x480 timeout_ms=1000 irq_bank_off=0x0 build="..." board="..."
CETRACE v=1 ev=commit seq=<n> t=<k_cycle> at=<same> out=0 result=ok sub=<id> buf=<0|1> dwt=<CYCCNT> fgts=0x<raw> fi=<frame index> line=<line index>
CETRACE v=1 ev=latched seq=<n> t=<k_cycle in ISR> at=<same> out=0 sub=<id> tev=<same as t> buf=<0|1> dwt=<CYCCNT in ISR> fgts=0x<raw> fi=<n> line=<n> st=0x<group-0 status>
CETRACE v=1 ev=released seq=<n> t=<k_cycle> at=<same> buf=<0|1> sub=<id of the submission whose buffer it was> dwt=<CYCCNT>
CETRACE v=1 ev=latch_timeout seq=<n> t=... at=... out=0 sub=<id> buf=<0|1> dwt=... fgts=0x... fi=... line=... st=0x<status at timeout>
RESULT imx95_m7_latch: frames=<n> committed=<n> latched=<n> released=<n> dropped=0 timeouts=<n> busy_refused=<n> isr=<n> isr_other=<n> unexpected_latch=<n> queue_full=<n> fi_step1=<n> fi_step_other=<n> records=<n> overrun=<n>
VERDICT imx95_m7_latch: PASS|FAIL|BLOCKED|SKIP <reason>
```

- Both clock frequencies in `hdr` are read from the running system
  (`sys_clock_hw_cycles_per_sec()` and `timing_freq_get()`), never assumed.
  In the host build the board's system clock is configured at
  `CONFIG_SYS_CLOCK_HW_CYCLES_PER_SEC=32768` (observed 2026-11-28 in the
  generated `.config`), so `t`/`tev` are coarse (one count is about
  30.5 µs, inferred by division); `dwt` is the fine counter. Both are raw
  32-bit counts that the host unwraps and converts. The application prints no
  time.
- `fi_step1` / `fi_step_other` count consecutive latches whose frame index
  advanced by exactly one / by any other amount (modulo 2^18). They are counts,
  not timing.
- `dropped` is always 0: a pending commit is never replaced.

Verdicts (latch reporting only; no timing is judged):

| Verdict | Condition |
| --- | --- |
| `PASS <n> commits, <n> latched by the ExtDst0 shadow-load interrupt, <n> released, 0 timeouts` | every commit latched through the interrupt, every busy probe refused, no unexpected or lost latch, no record lost |
| `BLOCKED sub=<id> ExtDst0 shadow-load flag set but no interrupt within <ms> ms (interrupt routing)` | the DPU latched, but the interrupt did not reach the M7 |
| `FAIL sub=<id> no ExtDst0 shadow load within <ms> ms` | timeout and the flag is clear |
| `FAIL a commit was accepted while one was pending` | busy rule broken |
| `FAIL <n> latch interrupts without a pending commit, <n> lost` | latch with nothing pending, or queue overflow |
| `FAIL event log incomplete, <n> records lost` | ring overrun |
| `FAIL committed=<n> latched=<n> of <n>` | counts do not add up |
| `SKIP DPU access disabled at build time` | default build |

A bus fault after `imx95_m7_latch: programming the DPU` and before the
`run start` line would mean the M7 may not access the DPU registers.

Kconfig options (only with the DPU option on): `IMX95_M7_LATCH_FRAMES`
(default 600), `IMX95_M7_LATCH_TIMEOUT_MS` (default 1000),
`IMX95_M7_LATCH_RING` (default 4096), `IMX95_M7_LATCH_IRQ_BANK_OFFSET`
(default `0x0`).

## Overlay

`boards/imx95_evk_mimx9596_m7_ddr.overlay` is the same as in
`tests/board/imx95_m7_scanout`: it enables the `dpu` node and gives it
`width`, `height`, `pixel-format` (ARGB8888) and a `display-timings` child.
The timing is a placeholder 640x480@60 (VESA), not the timing of the Verdin
panel or of any EVK panel. The DDR variant is used because the buffers
(2.4 MB) do not fit the TCM. The board target is the NXP EVK one; there is no
Zephyr board for the Verdin iMX95 M7 in this repository yet.

## Build

```sh
cd "$ZEPHYR_WORKSPACE"
west build -p always -b imx95_evk/mimx9596/m7/ddr <repo>/tests/board/imx95_m7_latch -d <build dir>
# DPU enabled
west build -p always -b imx95_evk/mimx9596/m7/ddr <repo>/tests/board/imx95_m7_latch -d <build dir> -- \
    -DCONFIG_IMX95_M7_LATCH_ENABLE_DPU=y
```

Use a build directory outside the repository and outside `/tmp`. Requires a
west workspace with `hal_nxp` and the Zephyr SDK
([`docs/zephyr-workspace.md`](../../../docs/zephyr-workspace.md)).

## Host build results (observed, 2026-11-28)

Built on an x86_64 Linux host against Zephyr `main` at commit
`e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (4.5.99, the pin of
`tests/board/imx95_m7_scanout`) with `hal_nxp` at
`1a8937f3f8dfe5c68f59cc2a0c0c0fccdb096650`, Zephyr SDK 1.0.1
(`arm-zephyr-eabi` GCC 14.3.0, default `zephyr` toolchain variant), with the
two commands above:

| Configuration | Result |
| --- | --- |
| default | links, no compiler warnings; RAM 37936 B of 42 MB; no DPU code in the image (no `fsl_dpu.c`, no `display_socionext_dpu.c`) |
| `CONFIG_IMX95_M7_LATCH_ENABLE_DPU=y` | links, no compiler warnings; RAM 2593960 B of 42 MB (mostly the two buffers and the ring); `fsl_dpu.c` is in the application library, `display_socionext_dpu.c` is not built, and the ISR table entry 215 points at `latch_isr` |

Rebuilt on a second x86_64 Linux host on 2026-11-28 against the workspace that
`scripts/prepare-zephyr.sh` creates (Zephyr tag `v4.5.0`, `hal_nxp` at
`a8b437ef366ff9a6f9bee3377dc189c4d2623f5b`), same SDK and compiler, same two
commands:

| Configuration | Result (`v4.5.0`) |
| --- | --- |
| default | links, no compiler warnings; RAM 37928 B of 42 MB |
| `CONFIG_IMX95_M7_LATCH_ENABLE_DPU=y` | links, no compiler warnings; RAM 2593952 B of 42 MB; `fsl_dpu.c` is built into the application library |

The `v4.5.0` builds were repeated on a third x86_64 Linux host on 2026-11-28
with the same workspace pins, SDK and commands. Both configurations linked
with no compiler warnings, and the RAM figures were the same as in the table.

These are host builds only. The image was not loaded on any board; no latch,
interval or latency was measured.
