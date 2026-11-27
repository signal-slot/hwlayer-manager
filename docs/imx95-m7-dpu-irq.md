# i.MX95 DPU interrupts and the Cortex-M7 NVIC (source study)

Scope: how the i.MX95 display controller (DPU) interrupts reach a core, and
what a Zephyr driver on the Cortex-M7 needs for a vblank callback and a latch
(shadow-load) event. This is milestone M2 groundwork (SoW §5.1, WP3).

Nothing in this file is observed on a board. Every finding is tagged
**read from source** or **inferred**. Study date: 2026-11-28. No board
addresses or credentials appear here.

## Sources

| Short name | Source | Pin |
|---|---|---|
| Zephyr | https://github.com/zephyrproject-rtos/zephyr | commit `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (VERSION 4.5.99, main) |
| hal_nxp | https://github.com/zephyrproject-rtos/hal_nxp | commit `1a8937f3f8dfe5c68f59cc2a0c0c0fccdb096650` (the revision `west.yml` of the Zephyr pin names) |
| linux-imx | https://github.com/nxp-imx/linux-imx, branch `lf-6.6.y` | commit `5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc` (tip when read; no `lf-6.6.y_*` tag was found) |
| imx-sm | https://github.com/nxp-imx/imx-sm, branch `master` | commit `eec232878ab7dd460f5a8f3767d39dc7e9412efa` (tags `lf-6.18.37-2.1.0`, `rel_imx_sm_2026q3`) |

Files read:

- Zephyr: `dts/arm/nxp/imx/nxp_imx95_m7.dtsi`,
  `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts`,
  `drivers/display/display_socionext_dpu.c`,
  `drivers/interrupt_controller/intc_nxp_irqsteer.c`,
  `dts/bindings/interrupt-controller/nxp,irqsteer-{intc,master}.yaml`.
- hal_nxp (under `mcux/mcux-sdk-ng/`):
  `devices/i.MX/i.MX95/MIMX9596/MIMX9596_cm7_COMMON.h`,
  `.../MIMX9596_cm7_features.h`, `drivers/dpu_1/fsl_dpu.{h,c}`.
- linux-imx: `arch/arm64/boot/dts/freescale/imx95.dtsi`,
  `imx95-19x19-evk.dts`, `drivers/irqchip/irq-imx-irqsteer.c`,
  `drivers/gpu/drm/imx/dpu95/` (`dpu95.h`, `dpu95-core.c`, `dpu95-crtc.c`).
- imx-sm: `configs/mx95evk.cfg`,
  `devices/MIMX95/configtool/{device,displaymix,m7mix,wakeupmix}.cfg`.

Limits of this study:

- **Not read: the Toradex BSP kernel.** The Toradex kernel tree for BSP 7.8.0
  (6.6.157) could not be fetched. All Linux statements are about NXP
  `lf-6.6.y`, which is the base of the Toradex tree. Whether Toradex changes
  the display nodes is **not known**. The Toradex Verdin iMX95 DT and its
  Mallow/LVDS overlays were not read (only NXP's `imx95-19x19-verdin.dts`,
  which includes the EVK DT and has no display override).
- **Not read: the imx-sm C sources** (TRDC programming, LMM start path,
  `IRQSTEER_M7` definition). Only the config files were read.
- **Not read: Toradex's imx-sm configuration** for the Verdin iMX95. The
  only SM config read is NXP's `mx95evk.cfg`; Toradex's may differ.
- The Linux source was read through a delegated page fetch without line
  numbers, so entries cite node and function names. Bit and index numbers
  marked "derived" were computed from the quoted lists.
- Detail on the DPU driver itself belongs to `docs/imx95-m7-dpu-driver.md`
  and is not repeated here.

## 1. The two-stage interrupt path

Findings:

1. The DPU does not drive core interrupt lines directly. DPU event outputs go
   to the display mix's own IRQSTEER block, which groups its 512 inputs by 64
   and drives one output line per group. — *read from source* (linux-imx
   `imx95.dtsi` `displaymix_irqsteer`: `fsl,num-irqs = <512>`, eight
   `interrupts` entries; `irq-imx-irqsteer.c`: "one output irq for each group
   of 64 inputs").
2. The display-mix IRQSTEER sits at bus address `0x4b0b0000` (64 KiB window in
   Zephyr, 4 KiB in Linux) on both the Linux and the Zephyr/NXP-header view.
   — *read from source* (`imx95.dtsi`; `nxp_imx95_m7.dtsi` `disp_irqsteer`;
   `MIMX9596_cm7_COMMON.h` `DPU_IRQSTEER_BASE`).
3. It is a separate block from the general IRQSTEER (`0x44680000`, NVIC
   224–233, `nxp,irq-offset = <234>`, 640 inputs) that the M7 DT lists for
   other peripherals. — *read from source* (`nxp_imx95_m7.dtsi`;
   `MIMX9596_cm7_features.h` `FSL_FEATURE_IRQSTEER_*`).
4. Register layout (shared IP, from the Linux driver with
   `reg_num = num_irqs/32`): `CHANCTRL` at 0, then `CHnMASK`, `CHnSET`,
   `CHnSTATUS` arrays, then `CHnMINTDIS` and `CHnMASTRSTAT`. Register index is
   reversed (`reg_num - irq/32 - 1`). For the display mix (16 registers)
   that puts MASK at `0x04+4n`, SET at `0x44`, STATUS at `0x84`, MINTDIS at
   `0xC4` and MASTRSTAT at `0xC8` (derived). — *read from source* for the
   macros, *inferred* for the display-mix offsets.
5. `CHANCTRL` must have the channel bit set before an input reaches an output
   ("steer all IRQs into configured channel"). Linux uses `fsl,channel = <0>`
   for the display mix. The Zephyr driver (`intc_nxp_irqsteer.c`) does not
   program `CHANCTRL`; it only writes `MINTDIS` and the masks (and generic
   code at init). Whether the reset value of `CHANCTRL` already selects the
   channel the M7 sees is **not answered** by the sources read. — *read from
   source* (both drivers); the gap is open.

## 2. DPU interrupt sources

### 2.1 Linux view (linux-imx `dpu95`)

- The DPU node lists 86 interrupts in `enum dpu95_irq` order, with
  `interrupt-parent = <&displaymix_irqsteer>`; each entry is an IRQSTEER input
  number. — *read from source* (`imx95.dtsi` `dpu`, `dpu95.h`
  `DPU95_IRQ_COUNT = 86`).
- Input ranges (hwirq = IRQSTEER input, parent = output line group):
  store9 and the command-sequencer error at 448–451 (group 7); ExtDst0 /
  DomainBlend0 / DisEngCFG0 / FrameGen0 at 64–92 (group 1, "stream 0");
  the stream-1 equivalents at 192–212 (group 3) and 213–226 (also group 3,
  FrameGen1 primsync/overflow/underrun/threshold). — *read from source*
  (`imx95.dtsi`) plus the group arithmetic *inferred* (input / 64).
- DSI is input 48 and the pixel interleaver is 62, both group 0. — *read from
  source* (`imx95.dtsi`).
- Linux gives the DPU three register banks for these events:
  `comctrl_irq_reg` at DPU base `+0x1000`, `disp_irq0_reg` at `+0x381000`
  (stream 0) and `disp_irq2_reg` at `+0x3a1000` (stream 1), each with
  `INTERRUPTENABLE(n)` at `0x8+4n`, `INTERRUPTPRESET` at `0x14+4n`,
  `INTERRUPTCLEAR` at `0x20+4n`, `INTERRUPTSTATUS` at `0x2c+4n`. — *read from
  source* (`dpu95.h`, `dpu95-core.c`).
- Linux keeps three Linux IRQ domains, one per bank, and chains the
  IRQSTEER inputs into them. — *read from source* (`dpu95_irq_init`).

### 2.2 What Linux uses for vblank and latch

All *read from source* (`dpu95-crtc.c`):

| Role in Linux | DPU event | Stream 0 input (derived) | Stream 1 input (derived) |
|---|---|---|---|
| Vblank (`drm_crtc_handle_vblank`, page-flip event) | DisEngCFG frame complete (`disengcfg_framecomplete`) | 74 | 202 |
| Latch of the plane/ExtDst state (waited in atomic flush) | ExtDst shadow load (`extdst_shdload`) | 64 | 192 |
| Latch of blending | DomainBlend shadow load | 70 | 198 |
| Latch of display-engine config | DisEngCFG shadow load | 73 | 201 |
| Modeset-off sequencing | DisEngCFG sequence complete | 75 | — |

- The vblank IRQ is enabled/disabled per CRTC (`enable_irq` /
  `disable_irq_nosync`). The handler calls `drm_crtc_handle_vblank` and sends
  the pending page-flip event.
- Shadow-load IRQs are enabled only around a commit: atomic flush enables the
  ExtDst shadow-load IRQ, writes `SYNC_TRIGGER` to the ExtDst
  `PIXENGCFG_TRIGGER`, waits on a completion (1 s timeout), then disables
  the IRQ.
- Linux does **not** use the FrameGen `INT0..3` interrupts for vblank, nor
  ExtDst frame complete; the FrameGen interrupt registers exist but are
  unused for vblank. A code comment says the DisEngCFG frame-complete event
  comes "after ExtDst SHDLD IRQ … about a time duration of an entire vblank".
- For the Linux DT lists, the order of the first 64-range entries is the
  `enum dpu95_irq` order; the numeric index → hwirq table above is derived
  from that list by the study and should be re-checked against the file when
  a driver is written.

### 2.3 What the NXP SDK / Zephyr driver uses

All *read from source* (`fsl_dpu.h`, `fsl_dpu.c`, `display_socionext_dpu.c`):

- The SDK enum `_dpu_interrupt` is three groups of 32 bits read from
  `IRQ_INTERRUPTENABLE[group]` / `IRQ_INTERRUPTSTATUS[group]` at
  `DPU_COMCTRL_OFFSET = 0x0` (`DPU_INT_GROUP_NUM = 3`). Group 0 order:
  Store9 shadow/frame/seq (bits 0–2), ExtDst0 shadow load (bit 3), ExtDst0
  frame complete (bit 4), ExtDst0 seq complete (5), ExtDst4, ExtDst1,
  ExtDst5, DomainBlend0 shadow/frame, DisEngCFG shadow/frame/seq
  (17–19), FrameGen0 INT0–INT3 (20–23), signature units. Group 1 starts with
  local dimming, DomainBlend1, DisEngCFG1, FrameGen1.
- The Zephyr driver (`display_socionext_dpu.c`) enables only
  `kDPU_Group0ExtDst0ShadowLoadInterrupt`. Its ISR reads group-0 status,
  clears what it saw, and gives a semaphore if the ExtDst0 shadow-load bit is
  set; `dpu_display_write` triggers a shadow load and waits on that semaphore.
  That is the only DPU event consumed. There is **no vblank callback and no
  FrameGen/DisEngCFG frame-complete handling** in the driver.

**Layout discrepancy (open).** Linux addresses the DPU events through banks at
`+0x1000`, `+0x381000` and `+0x3a1000` and orders them extdst-first inside the
stream banks; the SDK enum uses a single group-0 bank at offset 0 with
Store9 first. These are two different views of the register file, and which
one the i.MX95 silicon exposes at the offsets used by the SDK is **not
settled** by these sources. A Zephyr bring-up must check the SDK offsets
against the i.MX95 reference manual before relying on the bit positions in
2.3. — *inferred* (comparison of the two sources).

## 3. Linux wiring in DT and driver

All *read from source* (linux-imx `lf-6.6.y`; Toradex tree not read):

- `displaymix_irqsteer` (`fsl,imx-irqsteer`) has eight GIC SPI parents:
  214, 215, 216, 217, 218, 303 (reserved), 303 (reserved), 219.
  `power-domains = <&scmi_devpd IMX95_PD_DISPLAY>`,
  `clocks = <&scmi_clk IMX95_CLK_DISPAPB>` ("ipg"), `fsl,channel = <0>`,
  `#interrupt-cells = <1>`, disabled by default; the EVK DT enables
  `displaymix_irqsteer` and `dpu` and nothing else of the display chain.
- Parent *i* carries inputs `64*i … 64*i+63`; the chained handler reads two
  32-bit status registers and dispatches each set bit.
- The pixel-link / LDB / DSI nodes sit in the same display mix (`dispmix_csr`,
  `lvds_csr`, `mipi_dsi`) and are disabled in the EVK DT. The Toradex Mallow
  LVDS enablement was not read (see limits).

## 4. Do the lines reach the M7 NVIC?

Findings:

1. The M7 NVIC exposes six display-mix IRQSTEER output lines:
   `DISP_IRQSTEER0..4` and `DISP_IRQSTEER7` = NVIC 214, 215, 216, 217, 218,
   219. The numbering skips 5 and 6, matching the two reserved SPI 303
   entries of the Linux node. — *read from source* (`MIMX9596_cm7_COMMON.h`
   `DISP_IRQSTEER0_IRQn`…; Linux `imx95.dtsi`).
2. The M7 and A55 numbers are identical (NVIC 214 ↔ GIC SPI 214, …), so the
   same output group feeds the same index on either core. — *inferred* from
   matching numbers; the numbering rule for the M7 NVIC versus GIC SPI
   (SPI n ↔ NVIC n) is not stated in the sources read.
3. Zephyr's M7 DT defines the display IRQSTEER (`disp_irqsteer`,
   `nxp,irq-offset = <874>`, `nxp,num-irqs = <512>`, `disp_mix` power
   domain) with masters 0,1,2,3,4,7 on NVIC 214,215,216,217,218,219. Both
   the IRQSTEER and its masters are `status = "disabled"`. — *read from
   source* (`nxp_imx95_m7.dtsi`).
4. In the same DT, `mipi_dsi` uses the display IRQSTEER (master 0, input 48)
   but `dpu` declares `interrupts = <215 0>` with no `interrupt-parent`, so it
   is a direct NVIC line. That is output group 1 (inputs 64–127, "stream 0"
   of Linux), not a level-1 line of its own. — *read from source*; the
   implication that the DPU's event inputs must be unmasked in the display
   IRQSTEER (and that the IRQSTEER must be enabled) for NVIC 215 to ever fire
   is *inferred*: nothing in the DT or the driver enables
   `disp_irqsteer` today, and the DPU driver never touches it.
5. Neither EVK board for the M7 enables `disp_irqsteer`; the 9596 board file
   only enables `zephyr_dpu` and `zephyr_mipi_dsi`. — *read from source*
   (`imx95_evk_mimx9596_m7.dts`).
6. With only groups 1 (215) bound, the M7 sees stream-0 events (ExtDst0 /
   DomainBlend0 / DisEngCFG0 / FrameGen0, inputs 64–92 on the Linux numbering)
   and nothing else. The stream-0 vblank (DisEngCFG0 frame complete, input 74)
   and ExtDst0 shadow load (input 64) are both in group 1. — *inferred*
   (Linux input numbers + 64-per-group).
7. A single NVIC line per group means level-2 demultiplexing: the ISR (or
   Zephyr's multi-level IRQ layer through `intc_nxp_irqsteer.c`) reads the
   IRQSTEER status to find the input. Zephyr's multi-level scheme uses
   `nxp,irq-offset` as the second-level base and ties each master to its
   parent line. — *read from source* (`intc_nxp_irqsteer.c`).

## 5. Does the System Manager route or grant them?

- **No per-LM interrupt routing was found** in the SM config files: no `irq`,
  `NVIC` or `GIC` assignment keyword; the only related resources are `GIC`
  (SM `ACCESS`, `AP-NS` `OWNER`), `GPC` (SM owner) and `IRQSTEER_M7` and
  `MSGINTR1/2` (M7 `OWNER`). `IRQSTEER_M7`'s defining file was not found.
  — *read from source* (`mx95evk.cfg`).
- **Access to the blocks is what the SM controls.** Each display resource in
  `displaymix.cfg` (`DC`, `DC0`, `DC1`, `LVDS`, `BLK_CTRL_DISPLAYMIX`,
  `TRDC_D`, `GPV_DISPLAY`, …) is tied to `PD_DISPLAY` and to TRDC MBC slots;
  each logical machine has a TRDC domain id (SM 2, AP 3, M7 4) and
  `OWNER`/`ACCESS` permissions decide who may touch the resource.
  — *read from source*.
- **In NXP's EVK config the M7 LM owns none of these.** LM1 (`M7`) lists
  `M7P`, `CAN_FD1`, `FSB`, `IRQSTEER_M7`, `LPIT1`, `LPTMR1/2`, `LPTPM1`,
  `LPUART3`, `MSGINTR1/2`, `MU5_A`, `MU7_B`, `MU8_B`, `MU_ELE5`, `PD_M7`,
  `SYSCTR_RD`, `TSTMR2`, `V2X_SHE1`, `WDOG5`; none of `DC*`, `LVDS`,
  `MIPI_DSI`, `BLK_CTRL_DISPLAYMIX` or a display clock. The non-secure A55
  LM (`AP-NS`) owns the whole DC/LVDS/MIPI set and `CLOCK_DISP1PIX`,
  `LDB_PLL`; LM0 (SM) owns `GPV_DISPLAY`, `TRDC_D` and the DISPAPB / DISPAXI /
  DISPOCRAM clocks. — *read from source* (`mx95evk.cfg`).
- **No `DISP_IRQSTEER*` resource appeared** in the files read, and the
  `PD_DISPLAY` power-domain assignment per LM was not visible in the portion
  read (the tail of `mx95evk.cfg` was truncated). So whether the display
  IRQSTEER registers (`0x4b0b0000`) are gated per LM, and by which resource,
  is **not answered**.
- Consequences (*inferred*):
  1. Under an EVK-like SM config the M7 could not read the DPU event
     registers, program the display IRQSTEER, or enable the display clocks
     and power domain, so the NVIC lines would never be usable even though
     the NVIC inputs physically exist on the M7.
  2. Giving the M7 the display would be an SM configuration change (an
     imx-sm config rebuilt into the boot container), not a Zephyr change.
     Whether Toradex's SM config for the Verdin iMX95 differs from the EVK
     one is **not known**; no board was observed for this study.
  3. The Zephyr M7 DT's `disp_mix` power domain (`scmi-power-domain`, id
     `0xd`) means the M7 would request the display power domain over SCMI
     itself; the SM must permit that for LM1.

## 6. What a Zephyr driver would need

*inferred* from the sources above unless marked.

For a **latch event** (shadow load done):

1. Ownership: SM config grants the M7 the display set (§5), and the
   Linux side leaves the DPU unclaimed (the two cannot share a stream).
2. Power and clocks over SCMI (`disp_mix` power domain, `DISPAPB`/`DISPAXI`/
   `DISPOCRAM`/pixel clocks) before any register access.
3. Enable `disp_irqsteer` and the master(s) in the M7 DT, with the DPU
   `interrupt-parent` set so the DPU's inputs go through the level-2 layer
   instead of the bare `interrupts = <215 0>`. Unmask the needed inputs in
   IRQSTEER (and make sure `CHANCTRL` selects the channel the M7 is wired to;
   see §1 finding 5).
4. Unmask the ExtDst0 shadow-load event in the DPU enable register and clear
   it in the ISR. The existing driver already does this with the SDK
   interface (group 0 bit 3), subject to the layout check in §2.3.
5. Deliver the event to the application: replace the existing semaphore
   with a callback (or keep the semaphore and add a callback hook) that
   records a timestamp from a free-running M7 timer.

For a **vblank callback**:

6. Also unmask the frame-complete event Linux uses for vblank (DisEngCFG
   frame complete, or FrameGen INT0..3 whose meaning is not documented in the
   sources read; Linux deliberately uses DisEngCFG frame complete).
7. In its ISR, read the status for that bit, clear it, count the frame, and
   call the application callback. Order check from Linux's comment: shadow
   load fires, and DisEngCFG frame complete follows about one full vblank
   later, so a driver can use shadow load as "latch" and frame complete as
   "vblank".
8. Keep ISRs short; both bits live in the same level-2 group, so one NVIC
   entry (215 for stream 0) serves both and the ISR must dispatch on status.

For reporting honestly, any measurement taken this way must record which of
the two events was timestamped; no hardware result is claimed by this file.

## 7. Open items for the M2 list

1. Obtain the Toradex 6.6.157 tree and the Toradex imx-sm config, and
   re-check §3 and §5 against them.
2. Resolve the register-layout discrepancy of §2.3 against the i.MX95
   reference manual.
3. Find which SM resource (if any) gates the display IRQSTEER and the
   `PD_DISPLAY` assignment per LM; read `sm/` C sources for TRDC set-up.
4. Check `CHANCTRL` reset value and which channel the M7 is wired to.
5. Check whether the M7 NVIC index for display IRQSTEER outputs equals the GIC
   SPI number from the reference manual, rather than from naming alone.
