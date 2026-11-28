# i.MX95 display pipeline: SCMI clock, power-domain and reset IDs

Source study, dated 2026-11-28. Scope: SoW §5.1 / §9 M2 (WP3). No board was
accessed for this note and nothing was built or run. Findings carry one of
three tags: **read from source**, **inferred**, or **observed**. The only
observed values are quoted from the dated board snapshots
`imx95-zephyr/observed-2026-11-27.txt` and `observed-2026-11-28.txt` and are
collected in §10 (reconciled 2026-11-28); everything else is read from source
or inferred.

This note is the ID table the other i.MX95 notes refer to. It does not repeat
their analysis:

- Clock tree, mux and PLL behaviour, and the SM clock permission model:
  [imx95-display-clocks.md](imx95-display-clocks.md).
- Power-domain and reset behaviour in the SM, and what Linux does to them:
  [imx95-display-power.md](imx95-display-power.md).
- The Zephyr SCMI client and what `soc_init()` calls:
  [imx95-m7-scmi.md](imx95-m7-scmi.md).
- What Linux shows userspace about the SM:
  [imx95-sm-linux-interfaces.md](imx95-sm-linux-interfaces.md).
- DPU and LDB driver behaviour in the 6.6 kernel:
  [imx95-dpu-source-6.6.md](imx95-dpu-source-6.6.md).
- Overall feasibility: [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md).

The Verdin-specific SM resource assignment is out of scope here. Where a
permission is quoted, it is NXP's `mx95evk` reference configuration, not the
Verdin's (that the Verdin grants the same is **inferred** and unchecked).

## 1. Sources and pins

| Id | Source | Pin (as resolved 2026-11-28) | Files read |
|---|---|---|---|
| SM | NXP imx-sm, <https://github.com/nxp-imx/imx-sm/tree/af1e37026c6ba19cdca98fd6e91494efd3e7b5ec> | tag `rel_imx_sm_2025q3` = commit `af1e37026c6ba19cdca98fd6e91494efd3e7b5ec`. The annotated tag `lf-6.6.52-2.2.2` (tag object `5c94700d6a548ee508a5aa4a821c69ecd5ecc52c`) is recorded by the other notes as pointing to the same commit; it was not re-dereferenced here | `devices/MIMX95/sm/dev_sm_clock.h`, `dev_sm_power.h`, `dev_sm_reset.h`; `configs/mx95evk/config_scmi.h` |
| LX | NXP linux-imx, <https://github.com/nxp-imx/linux-imx/tree/lf-6.6.52-2.2.2> | tag `lf-6.6.52-2.2.2` (tag object `f9a8e33f5cde3e96a57569e9dfa70481fe688943`; the commit `5a0a5e71d2bd` is the one recorded in the feasibility note and was not re-resolved here) | `include/dt-bindings/clock/fsl,imx95-clock.h`, `include/dt-bindings/power/fsl,imx95-power.h`, `arch/arm64/boot/dts/freescale/imx95.dtsi` |
| ZP | upstream Zephyr, <https://github.com/zephyrproject-rtos/zephyr/tree/v4.5.0> | tag `v4.5.0` (commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966`, as recorded in [imx95-m7-scmi.md](imx95-m7-scmi.md)) | `include/zephyr/dt-bindings/clock/imx95_clock.h`, `include/zephyr/dt-bindings/power/imx95_power.h`, `dts/arm/nxp/imx/nxp_imx95_m7.dtsi` |

How the files were read and what that limits:

- Read on 2026-11-28 as raw text over HTTPS through a page fetcher at the pins
  above. No clone was available on this node. Quoted identifiers and values are
  verbatim. **Line numbers were counted by hand on the fetched text** and may
  be off by a line or two. The Linux display nodes are cited by node label, not
  by line, because they sit deep in `imx95.dtsi`.
- **The BSP match is not established.** The board runs Toradex Reference
  Multimedia 7.8.0 with kernel 6.6.157-7.8.0. NXP's `linux-imx` has no tag
  containing `7.8` or `6.6.157` (its `lf-6.6` tags end at `lf-6.6.52-2.2.2`,
  listed through the GitHub tag API on 2026-11-28), and which imx-sm commit
  Toradex's SM was built from is not known. `lf-6.6.52-2.2.2` and
  `rel_imx_sm_2025q3` are therefore **stand-ins** (the same ones the other
  i.MX95 notes use). That the IDs below equal the board's is **inferred**. The
  IDs are `#define`s of a fixed SoC numbering that the Linux, SM and Zephyr
  headers share, so a mismatch is unlikely, but a newer SM may add clocks
  (see §7).
- The Toradex kernel DT (`linux-toradex`, the board's 6.6.157 kernel) was not
  re-read. Its display clocks are cited in
  [imx95-dpu-source-6.6.md](imx95-dpu-source-6.6.md) and agree with LX.
- The SM headers give the numeric values only in comments (`/*!< 80: DISP1PIX
  root */`); the `CLOCK_SRC_*`, `PWR_MIX_SLICE_IDX_*` and `RST_LINE_*` constants
  behind them live in files not read here (`fsl_clock.h`, `fsl_power.h`,
  `fsl_reset.h`). The values below are the comment values, which the Linux
  and Zephyr headers' numeric defines confirm for clocks and power.

## 2. Clocks

SCMI clock IDs are the SM's `DEV_SM_CLK_*` numbers. Linux
(`IMX95_CLK_*`, `IMX95_CCM_NUM_CLK_SRC` = 41) and Zephyr
(`IMX95_CLK_*`, same base) use the same numbers (read from source: SM
`dev_sm_clock.h`, LX `fsl,imx95-clock.h`, ZP `imx95_clock.h`).

### 2.1 SCMI clocks (all read from source unless stated)

| Name | Value in SM | Value in kernel DT binding | Value in Zephyr binding | Used by (Linux 6.6 driver / DT consumer) | Referenced by upstream Zephyr M7 side? |
|---|---|---|---|---|---|
| `DISP1PIX` | 80 (`dev_sm_clock.h:149`) | `IMX95_CCM_NUM_CLK_SRC + 39` = 80 (`fsl,imx95-clock.h:91`) | `+ 39` = 80 (`imx95_clock.h:92`) | `dpu` node `clocks` "pix"; `mipi_dsi` "pix". In `dpu95-framegen.c` it is rate-set and enabled only for a DSI encoder (see [imx95-display-clocks.md](imx95-display-clocks.md) §3.2) | Yes: `soc_init()` sets its parent to VIDEOPLL1 and its rate, only under `CONFIG_MIPI_DSI_NXP_DWC`. Not in the `dpu` node |
| `DISP2PIX` | 81 (`:150`) | `+ 40` = 81 (`:92`) | `+ 40` = 81 (`:93`) | no node in `imx95.dtsi` names it | defined only; no reference |
| `DISP3PIX` | 82 (`:151`) | `+ 41` = 82 (`:93`) | `+ 41` = 82 (`:94`) | no node names it | defined only |
| `DISPAPB` | 75 (`:144`) | `+ 34` = 75 (`:86`) | `+ 34` = 75 (`:87`) | `dispmix_csr`, `lvds_csr`, `ldb0_phy`, `ldb1_phy`, `displaymix_irqsteer`, `pixel_interleaver`, `dpu` ("apb"). `dispmix_csr` also has it in `assigned-clocks`/`-parents`/`-rates` (parent `SYSPLL1_PFD1_DIV2`, 133333333 Hz) | defined only; the `dpu` node has no `clocks` |
| `DISPAXI` | 76 (`:145`) | `+ 35` = 76 (`:87`) | `+ 35` = 76 (`:88`) | `dpu` ("axi"); `dispmix_csr` `assigned-clocks` (parent `SYSPLL1_PFD1`, 400000000 Hz) (read from source, LX). **Open:** the board's `clk_summary` shows `dispaxi` at 800000000 Hz, not 400000000 (observed 2026-11-27, §10) | defined only |
| `DISPOCRAM` | 78 (`:147`) | `+ 37` = 78 (`:89`) | `+ 37` = 78 (`:90`) | `dpu` ("ocram"); `dispmix_csr` `assigned-clocks` (parent `SYSPLL1_PFD1`, 400000000 Hz) | defined only |
| (ID 77) | `DEV_SM_CLK_RESERVED77` (`:146`) | `IMX95_CLK_DISPDP` = `+ 36` (`:88`) | `IMX95_CLK_DISPDP` (`:89`) | not named by any display node read | defined only. **Name differs from the SM**, which reserves it |
| (ID 79) | `DEV_SM_CLK_RESERVED79` (`:148`) | `IMX95_CLK_DISPUSB31` = `+ 38` (`:90`) | `IMX95_CLK_DISPUSB31` (`:91`) | not display | defined only |
| `LDBPLL_VCO` | 37 (`:105`) | 37 (`:45`) | 37 (`:46`) | `dpu` "ldb_vco" (`dpu95-framegen.c` enables it for LVDS, see the clocks note); NXP's 10" LVDS panel DT file `assigned-clocks` on `&ldb`, 2986200000 Hz | defined only; no `ldb` node, no code |
| `LDBPLL` | 38 (`:106`) | 38 (`:46`) | 38 (`:47`) | parent of the top-level fixed-factor clock `ldb_pll_div7` (label `ldb_pll_pixel`, `clock-div = <7>`), which the `dpu` node takes as "ldb"; also the `ldb_phy_div` parent in `imx95-blk-ctrl.c`. Panel DT file: 497700000 Hz | defined only |
| `VIDEOPLL1_VCO` | 18 (`:86`) | 18 (`:26`) | 18 (`:27`) | DSI only (`mipi_dsi` parent chain) | Yes: `soc_init()` rate-set and enable, DSI configuration only |
| `VIDEOPLL1` | 19 (`:87`) | 19 (`:27`) | 19 (`:28`) | DSI only: parent of `MIPIPHYPLLBYPASS` and of the `disp_engineN_sel` mux input "videopll1" | Yes: same as above |
| `MIPIPHYCFG` | 66 (`:135`) | `+ 25` = 66 (`:77`) | `+ 25` = 66 (`:78`) | `mipi_dsi` "cfg" (parent 24M) | the M7 `mipi_dsi` node uses the literal `<&scmi_clk 0>` instead (an upstream defect, see [imx95-m7-scmi.md](imx95-m7-scmi.md)) |
| `MIPIPHYPLLBYPASS` | 67 (`:136`) | `+ 26` = 67 (`:78`) | `+ 26` = 67 (`:79`) | `mipi_dsi` "bypass" (parent `VIDEOPLL1`) | Yes: `soc_init()`, DSI only |
| `MIPIPHYPLLREF` | 68 (`:137`) | `+ 27` = 68 (`:79`) | `+ 27` = 68 (`:80`) | `mipi_dsi` "ref" (parent 24M) | defined only |
| `MIPITESTBYTE` | 69 (`:138`) | `+ 28` = 69 (`:80`) | `+ 28` = 69 (`:81`) | not named by a display node | Yes: `soc_init()`, DSI only |
| `CAMAPB` | 62 (`:131`) | `+ 21` = 62 (`:73`) | `+ 21` = 62 (`:74`) | `mipi_dsi` "pclk"; `display_master_csr` | defined only |

The two numbers `IMX95_CCM_NUM_CLK_SRC + 36` and `+ 38` (IDs 77 and 79) are the
only display IDs whose **names disagree** between the SM and the two DT
bindings. A client that asked for `DISPDP` or `DISPUSB31` over SCMI would be
asking for an SM-reserved clock (inferred from the SM reserved names; whether
the SM answers `NOT_FOUND` was not read for these IDs). The tables above list
the Linux and Zephyr names against the SM's.

The SM's last clock is `DEV_SM_CLK_GPU_CGC` = 174 (`dev_sm_clock.h:255-256`), so
the SM has 175 clocks; the header gives no `DEV_SM_NUM_CLOCK` value, only a
sum of counts (`:59-60`) (read from source; 175 is computed, inferred).

### 2.2 Linux-only IDs (not SCMI)

| ID | Value | What it is | Tag |
|---|---|---|---|
| `IMX95_CLK_LDBPLLDIV7` | 175 (`fsl,imx95-clock.h:187`) | Linux-side name outside the SM range (the SM has 175 clocks, IDs 0-174). The `imx95.dtsi` uses a DT fixed-factor clock `ldb_pll_div7` instead of it | read from source |
| `IMX95_CLK_DISPMIX_ENG0_SEL`, `_ENG1_SEL` | 0, 1 (`:191-192`) | clock-controller indices of the `dispmix_csr` provider (`fsl,imx95-dispmix-csr`, register block `0x4b010000`): the DPU engine source muxes | read from source |
| `IMX95_CLK_DISPMIX_LVDS_PHY_DIV` | 0 (`:195`) | `lvds_csr` provider (`fsl,imx95-dispmix-lvds-csr`, `0x4b0c0000`): LVDS PHY divider | read from source |
| `IMX95_CLK_DISPMIX_LVDS_CH0_GATE`, `_CH1_GATE` | 1, 2 (`:196-197`) | `lvds_csr`: LDB channel serial-clock gates ("ldb_ch0/1" of the `ldb` node) | read from source |
| `IMX95_CLK_DISPMIX_PIX_DI0_GATE`, `_DI1_GATE` | 3, 4 (`:198-199`) | `lvds_csr`: LDB pixel-input gates ("ldb_di0/1" of the `ldb` node) | read from source |

These are **bits in display-mix registers written over MMIO, not SCMI clocks**
(the `ldb` node's `clocks` are `<&lvds_csr ...>`, not `<&scmi_clk ...>`, read
from source in `imx95.dtsi`). The Zephyr binding has none of them and the Zephyr
M7 dtsi has no `ldb` or `lvds` token (read from source: ZP
`nxp_imx95_m7.dtsi`, no match for either string). So the **LVDS gates and the
PHY divider are not reachable by any SCMI request**. They need MMIO access to
`lvds_csr` and `dispmix_csr` (inferred; access is TRDC/resource assignment, see
[imx95-sm-display-lm.md](imx95-sm-display-lm.md)).

### 2.3 What the SM says about who may use them (NXP `mx95evk`, read from source)

`configs/mx95evk/config_scmi.h`, agent 0 "M7" (:57-63), agent 1 "AP-S"
(:150-156) and agent 2 "AP-NS" (:220-226). Only AP-NS has display entries. All
line numbers in this section were checked on 2026-11-28 with `grep -n` on a
downloaded copy of the file at the pin (read from source):

| Clock | AP-NS permission | Lines | M7 agent |
|---|---|---|---|
| `DISP1PIX`, `DISP2PIX`, `DISP3PIX` | `SM_SCMI_PERM_ALL` | :244-246 | none |
| `LDBPLL`, `LDBPLL_VCO` | `SM_SCMI_PERM_ALL` | :264-265 | none |
| `VIDEOPLL1`, `VIDEOPLL1_VCO` | `SM_SCMI_PERM_ALL` | :308-309 | none |
| `MIPIPHYCFG`, `MIPIPHYPLLBYPASS`, `MIPIPHYPLLREF`, `MIPITESTBYTE` | `SM_SCMI_PERM_ALL` | :287-290 | none |
| `DISPAPB`, `DISPAXI`, `DISPOCRAM` | no `clkPerms` entry for any agent (full file re-read 2026-11-28; no match for these three names) | none | none |

So under `mx95evk` **no SCMI agent, AP-NS included, holds a clock permission on
`DISPAPB`/`AXI`/`OCRAM`** (read from source). The other notes record that they
belong to LM0 and are "Modified via PERF protocol" in `mx95evk.cfg`
([imx95-display-clocks.md](imx95-display-clocks.md) §3.2) (read from source in
that note). AP-NS holds `perfPerms[DEV_SM_PERF_CAM]` and
`perfPerms[DEV_SM_PERF_DISP]` = `SM_SCMI_PERM_ALL` (`config_scmi.h:460-461`).
The SCMI perf domain `IMX95_PERF_DISP` is 12 (LX `fsl,imx95-power.h:45`), but
no display node in `imx95.dtsi` names a `scmi_perf` power domain (read from
source, nodes listed in §5). The file has no reset-permission field (read from
source).

The M7 agent has no clock permission for any display clock under `mx95evk`
(read from source). The permission levels and what each SCMI message needs are
in [imx95-display-clocks.md](imx95-display-clocks.md) §4; they are not repeated.

## 3. Power domains

| Name | Value in SM | Value in kernel DT | Value in Zephyr | Used by (Linux 6.6) | Referenced by upstream Zephyr M7 side? |
|---|---|---|---|---|---|
| `DISPLAY` | `DEV_SM_PD_DISPLAY` = 13 (`dev_sm_power.h:84`) | `IMX95_PD_DISPLAY` = 13 (`fsl,imx95-power.h:22`) | `IMX95_PD_DISPLAY` = 13 (`imx95_power.h:23`) | `dpu`, `dispmix_csr`, `lvds_csr`, `ldb0_phy`, `ldb1_phy`, `displaymix_irqsteer`: all `power-domains = <&scmi_devpd IMX95_PD_DISPLAY>` (read from source). `pixel_interleaver` and `ldb` have no `power-domains` property; `ldb` sits inside `lvds_csr` | Yes, two ways: `soc_init()` powers it (DSI configuration only), and the dtsi node `disp_mix` (`power-domain@d`, reg `0xd`) which only the disabled `disp_irqsteer` names. The enabled-by-overlay `dpu` node has no `power-domains` |
| `CAMERA` | `DEV_SM_PD_CAMERA` = 3 (`:74`) | `IMX95_PD_CAMERA` = 3 (`:12`) | `IMX95_PD_CAMERA` = 3 (`:13`) | `mipi_dsi` and `display_master_csr` (DSI path, and the pixel-link mux) | Yes: `soc_init()`, DSI configuration only |

- **There is no separate LVDS, LDB, LVDS-PHY or MIPI power domain.** The SM list
  runs ANA 0 to WAKEUP 22 (`dev_sm_power.h:71-93`) with no such entry, and the
  Linux and Zephyr bindings carry the same 23 names (read from source). The LDB
  and the LVDS PHYs are powered by `DISPLAY` (13). So an LVDS scanout needs
  exactly one SCMI power domain, `DISPLAY` (inferred from the node list).
- The Linux binding also defines SCMI **performance** domain IDs
  (`IMX95_PERF_*`, `fsl,imx95-power.h:33-53`), of which `IMX95_PERF_DISP` = 12
  and `IMX95_PERF_CAM` = 11 relate to display and camera. The Zephyr binding has
  no `IMX95_PERF_*` (read from source), and no display node read here uses a
  perf domain. A scanout does not need them (inferred).
- The numeric values in the SM header are in comments; they equal the Linux
  and Zephyr numeric defines (read from source), so SM, kernel DT and Zephyr
  agree.
- Who may switch `DISPLAY`: under `mx95evk`, only AP-NS has `pdPerms` for
  `DEV_SM_PD_DISPLAY` and `DEV_SM_PD_CAMERA`, `SM_SCMI_PERM_ALL`
  (`config_scmi.h:452` CAMERA, `:453` DISPLAY). The M7 agent has no power-domain permission for
  either (read from source). `POWER_STATE_GET` needs no permission, `SET`
  needs at least `SM_SCMI_PERM_SET`; see
  [imx95-display-power.md](imx95-display-power.md) §4.

## 4. Resets

| Name | Value in SM | Value in kernel DT | Value in Zephyr | Used by | Referenced by upstream Zephyr M7 side? |
|---|---|---|---|---|---|
| `DISP0_RESETN` | `DEV_SM_RST_DISP0_RESETN` = 20 (`dev_sm_reset.h:84`) | none | none | no consumer | no |
| `DISP1_RESETN` | `DEV_SM_RST_DISP1_RESETN` = 21 (`:85`) | none | none | no consumer | no |
| `LVDS_RESETN` | `DEV_SM_RST_LVDS_RESETN` = 22 (`:86`) | none | none | no consumer | no |
| `DISPLAYMIX` | `DEV_SM_RST_DISPLAYMIX` = 45 (`:109`) | none | none | no consumer | no |
| `CAMERAMIX` | `DEV_SM_RST_CAMERAMIX` = 34 (`:98`) | none | none | no consumer | no |

All rows read from source. The SM defines the five reset lines, with the only
reset type `DEV_SM_RST_COLD` = 0 (`:128`). But:

- **Neither kernel DT nor Zephyr DT uses SCMI resets for the display.** In
  `imx95.dtsi` (LX) the `scmi` node has protocols 0x11, 0x12, 0x13, 0x14, 0x15,
  0x19, 0x81 and 0x84 only, with **no reset protocol (0x16) node**. None of
  `dpu`, `ldb`, `ldb0_phy`, `ldb1_phy`, `dispmix_csr`, `lvds_csr`,
  `display_master_csr`, `displaymix_irqsteer`, `pixel_interleaver` or
  `mipi_dsi` has a `resets` property, and the file has no `IMX95_RESET`
  reference. In the Zephyr dtsi (ZP) the `scmi` node has protocols 0x11, 0x12,
  0x14, 0x19 and 0x82, no reset protocol, and the only `reset` text in the file
  is a pinmux label.
- The `mx95evk` `config_scmi.h` has no reset-permission entry for any agent
  (read from source; the fetch searched for `DEV_SM_RST_` and found none), so no
  agent could assert these resets over SCMI.
- Linux therefore brings the DPU and LDB out of reset only through the
  power-up of `DISPLAY` (13) and its own register writes (for example the LDB
  control register rewrite on runtime resume), as
  [imx95-display-power.md](imx95-display-power.md) §5-§6 records (read from
  source in that note).

## 5. Linux node to ID map

For one view of what Linux's LVDS route names, from LX `imx95.dtsi` (read from
source; nodes by label):

| Node | SCMI clocks | Block-control clocks | SCMI power domain |
|---|---|---|---|
| `dpu` (`nxp,imx95-dpu`, `0x4b400000`) | `DISP1PIX` "pix", `DISPAPB` "apb", `DISPAXI` "axi", `DISPOCRAM` "ocram", `LDBPLL_VCO` "ldb_vco" | `ldb_pll_pixel` ("ldb"): top-level fixed-factor `÷7` of `LDBPLL` | `DISPLAY` |
| `dispmix_csr` (`0x4b010000`) | `DISPAPB`; `assigned-clocks` `DISPAXI`, `DISPOCRAM`, `DISPAPB` | provider of `IMX95_CLK_DISPMIX_ENG0/1_SEL` | `DISPLAY` |
| `lvds_csr` (`0x4b0c0000`) | `DISPAPB` | provider of `LVDS_PHY_DIV`, `LVDS_CH0/1_GATE`, `PIX_DI0/1_GATE` | `DISPLAY` |
| `ldb` (`fsl,imx95-ldb`, inside `lvds_csr`, reg `0x4`) | none | consumes `PIX_DI0/1_GATE` ("ldb_di0/1") and `LVDS_CH0/1_GATE` ("ldb_ch0/1") | none (inherits) |
| `ldb0_phy`, `ldb1_phy` (inside `lvds_csr`, regs `0x8`, `0xc`) | `DISPAPB` "apb" | none | `DISPLAY` |
| `displaymix_irqsteer` (`0x4b0b0000`) | `DISPAPB` "ipg" | none | `DISPLAY` |
| `pixel_interleaver` (`0x4b0d0000`) | `DISPAPB` | none | none |
| `mipi_dsi` (`0x4acf0000`, DSI only) | `MIPIPHYCFG`, `MIPIPHYPLLBYPASS`, `MIPIPHYPLLREF`, `CAMAPB`, `DISP1PIX`; assigned parents 24M, `VIDEOPLL1`, 24M | none | `CAMERA` |
| `display_master_csr` (`0x4ad10000`, DSI path) | `CAMAPB` | none | `CAMERA` |

The `dpu`, `ldb` and `ldb` channel nodes are `status = "disabled"` in the SoC
dtsi (a board file enables them). The Linux DPU interrupts come through
`displaymix_irqsteer` (read from source), the interrupt side of which is
[imx95-m7-dpu-irq.md](imx95-m7-dpu-irq.md).

## 6. What upstream Zephyr's M7 side references

From ZP v4.5.0 `nxp_imx95_m7.dtsi` and the study in
[imx95-m7-scmi.md](imx95-m7-scmi.md) (read from source):

- SCMI protocols named: power 0x11 (`scmi_devpd`), system 0x12, clock 0x14
  (`scmi_clk`), pinctrl 0x19, NXP CPU 0x82. **No reset, perf or vendor
  display protocol.**
- The `dpu` node (`socionext,dpu`, `0x4b400000`, interrupt 215) has **no
  `clocks` and no `power-domains`**. The M7 DPU driver itself makes no SCMI
  call.
- The `mipi_dsi` node has `clocks = <&scmi_clk 0>` (literal 0, not an ID of
  the binding).
- `disp_mix` is the SCMI power domain `DISPLAY` (13); only `disp_irqsteer`
  names it.
- `soc_init()` references these SCMI IDs, only under `CONFIG_MIPI_DSI_NXP_DWC`:
  power domains 13 and 3; clocks 18, 19, 80, 67 and 69.
- **The binding defines but no code uses** `LDBPLL_VCO` 37, `LDBPLL` 38,
  `DISPAPB` 75, `DISPAXI` 76, `DISPOCRAM` 78, `DISP2PIX` 81, `DISP3PIX` 82,
  `MIPIPHYCFG` 66, `MIPIPHYPLLREF` 68 and the SM-reserved IDs 77, 79.
- No LVDS or LDB node, clock ID or reset anywhere in the Zephyr dtsi or
  binding.

## 7. Differences between the three sources

All read from source unless marked:

1. **Names at IDs 77 and 79.** SM: reserved. Linux and Zephyr: `DISPDP`,
   `DISPUSB31` (§2.1).
2. **Number of clocks.** The SM goes to ID 174. Linux adds `LDBPLLDIV7` = 175, a
   Linux-only (non-SCMI) ID, with `DUMMY` 176 and `END` 177. Zephyr's file
   ends at `XSPISLVROOT`, `IMX95_CCM_NUM_CLK_SRC + 122`, and has none of the
   Linux-only IDs.
3. **Linux-only block-control clocks** (`IMX95_CLK_DISPMIX_*`) are a separate
   clock-controller index space starting at 0, not SCMI IDs (§2.2).
4. **No reset binding** exists in either DT, although the SM defines reset
   domains 20-22 and 45.
5. **Newer SM releases.** The SM tag list on 2026-11-28 runs through
   `rel_imx_sm_2026q3` (commit `eec232878ab7dd460f5a8f3767d39dc7e9412efa`). The
   clocks note records that a compare of the pin with `master` lists a new
   board-level clock file for the EVK, which may change how LDB and display
   rates are set; the Toradex SM's revision is unknown. The ID values in §2-§4
   were read only at the pin (inferred: unchanged in newer releases, since the
   Linux binding is stable ABI, but not checked).

## 8. SCMI requests a Zephyr M7 LVDS scanout would have to make

Everything in this section is **inferred** from §2-§6 and the permission
tables in [imx95-display-clocks.md](imx95-display-clocks.md) §4-§5 and
[imx95-display-power.md](imx95-display-power.md) §4. It assumes the M7 owns the
LVDS route (the Verdin-specific assignment is another note's subject). The
upstream Zephyr code makes **none** of the LVDS requests today (§6), so each
row is new code (an SoC-init step or an LDB driver, see
[imx95-m7-ldb-driver.md](imx95-m7-ldb-driver.md)).

| # | SCMI request | IDs | Why | Needs the M7 agent's LM to hold access? |
|---|---|---|---|---|
| 1 | Base: protocol discovery, agent attributes | n/a | find power and clock protocols | The M7 needs a working channel and visibility of protocols 0x11 and 0x14 (inferred); not a per-resource permission |
| 2 | `POWER_STATE_SET` on, then `POWER_STATE_GET` until on | `DISPLAY` = 13 | `ldb`, both LVDS PHYs, `dispmix_csr`, `lvds_csr`, irqsteer and the DPU are all inside this one domain | **SET: yes**, `pdPerms[13]` at least `SM_SCMI_PERM_SET`. **GET: no**, any agent may read the state |
| 3 | `CLOCK_RATE_SET` | `LDBPLL_VCO` = 37 (the VCO, to the panel's multiple), then `LDBPLL` = 38 (the output divider) | the LVDS serial clock and, through `÷7`, the pixel clock both come from `LDBPLL` (Linux's panel DT file asks for 2986200000 and 497700000 Hz for a 71.1 MHz pixel clock, an NXP EVK file rather than the board's overlay) | **Yes**: `EXCLUSIVE` (`ALL`) on both. Only AP-NS holds it under `mx95evk`. Two writers on one PLL are not arbitrated |
| 4 | `CLOCK_CONFIG_SET` enable | `LDBPLL` = 38 and `LDBPLL_VCO` = 37 (Linux enables both: `dpu95-framegen.c` enables `ldb_vco`, then the `ldb` clock whose parent is `LDBPLL`) | gate the PLL outputs on | **Yes**: at least `SET` (aggregated with Linux's vote, so safe to share) |
| 5 | `CLOCK_CONFIG_SET` enable, and possibly rate set | `DISPAPB` = 75, `DISPAXI` = 76, `DISPOCRAM` = 78 | the DPU runs on these; Linux's runtime PM enables them and `dispmix_csr` assigns 400 / 400 / 133.33 MHz | **Yes if the M7 must enable them**: at least `SET`; `EXCLUSIVE` for any rate. Under `mx95evk` no agent holds a permission on them (§2.3). On the Verdin board all three were enabled while Linux ran (observed 2026-11-27, §10), held by the DPU's own votes, so whether they would be on without Linux is not known |
| 6 | Reads: `CLOCK_RATE_GET`, `CLOCK_CONFIG_GET`, `CLOCK_PARENT_GET`, `CLOCK_ATTRIBUTES`, `CLOCK_DESCRIBE_RATES`, `CLOCK_GET_PERMISSIONS` | the IDs above | diagnostics, and to read back the PLL rate Linux set | **No**: the SM checks no permission for these (read from source in the clocks note) |
| 7 | none | `DISP1PIX` = 80, `DISP2PIX` = 81, `DISP3PIX` = 82, `VIDEOPLL1(_VCO)` = 19 / 18, `MIPIPHY*` / `MIPITESTBYTE` = 66-69 | DSI only. For LVDS the DPU pixel clock comes from `LDBPLL` through `÷7` and the engine mux, not from `DISP1PIX` (read from source in the clocks note, with the caveat that Linux writes the mux bits directly) | n/a: do not call them. In particular **do not run upstream's DSI `soc_init()` sequence**, which sets `VIDEOPLL1` and `DISP1PIX` and powers `CAMERA` (3) |
| 8 | none | any SCMI reset | no SCMI reset is used by Linux or Zephyr for the display (§4) | n/a |
| 9 | none | `CAMERA` = 3 power domain | DSI path only. Whether an LVDS-only DC still needs it was left open by the power note (inferred) | If the M7 must power it, it would need `pdPerms[3]` at least `SET` |

Not SCMI, but needed for the same scanout (inferred): MMIO writes to
`lvds_csr` (`LVDS_PHY_DIV`, `LVDS_CH0/1_GATE`, `PIX_DI0/1_GATE`), to `dispmix_csr`
(`ENG0/1_SEL`, `CLOCK_CTRL`, pixel-link control) and to the LDB and PHY
registers. Those need TRDC access to these blocks, not an SCMI permission
([imx95-display-clocks.md](imx95-display-clocks.md) §2.3).

### Which of these need the agent's logical machine to have access

Under the `mx95evk` reference configuration the M7 agent holds **none** of
rows 2 (set), 3, 4 and 5 (read from source: only AP-NS has display
permissions). If the M7 ran the above with that configuration:

- `POWER_STATE_SET` (13) returns `SM_ERR_DENIED` (read from source), which
  upstream Zephyr maps to `-EACCES`.
- `CLOCK_RATE_SET` on 37 and 38 returns `SM_ERR_DENIED` (read from source),
  also `-EACCES`.
- `CLOCK_CONFIG_SET` enable returns `SM_ERR_DENIED` at the SM, but Zephyr's
  `scmi_clock_on_off` queries the clock's attributes first, and if the clock
  is restricted and state control is not allowed it **returns 0 without sending
  the request**, so the denial is silent (read from source in
  [imx95-m7-scmi.md](imx95-m7-scmi.md)). A scanout app must read the
  permissions (row 6) and check them, not trust the return value
  (inferred).
- Only the read requests (row 6) and `POWER_STATE_GET` work with no grant.

The minimum grant for an M7 **alongside** Linux is `SET` on `PD_DISPLAY`
and on the clocks it must keep on, with Linux staying the sole rate writer
(aggregated votes are safe, rate writes are not, per the clocks note). An M7
that **replaces** Linux as the display owner also needs `EXCLUSIVE` on
`LDBPLL_VCO` and `LDBPLL`. Which of these the Verdin's SM actually grants is the
subject of another note and is **not established here**.

## 9. Not done

- The Toradex kernel DT and the Mallow LVDS overlay were not re-read here.
- The numeric `PWR_MIX_SLICE_IDX_*`, `CLOCK_SRC_*` and `RST_LINE_*` constants
  were not read; values come from the SM header comments and the DT
  bindings.
- Whether `DISPAPB`/`DISPAXI`/`DISPOCRAM` are on without Linux was not read
  (their absence from `config_scmi.h` is recorded in §2.3; they were on with
  Linux running, §10).
- The Verdin `verdin-imx95.cfg` and the Toradex kernel DT were not read in this
  reconciliation. The LX `imx95.dtsi` nodes (`dispmix_csr` :2350, `dpu` :2604
  with clocks :2712-2723, `ldb` :2456, `lvds_csr` :2447, `displaymix_irqsteer`
  :2426, `pixel_interleaver` :2546) were re-read on 2026-11-28 and agree with
  §2 and §5 (read from source).
- The SM's answer to a request on the reserved IDs 77 and 79 was not read.
- Nothing was built, run or measured on a board. No timing claim is made.
  Section 10 only quotes the dated snapshots.

## 10. Board observations compared with the IDs above

Reconciliation added 2026-11-28. Source: the two read-only snapshots
`imx95-zephyr/observed-2026-11-27.txt` and `observed-2026-11-28.txt` of the
Verdin iMX95 on its Mallow carrier with the LVDS panel, Linux 6.6.157-7.8.0
(not the NXP stand-in tag of §1). Both runs were on 2026-11-27. The snapshots
hold decoded DT cells and `clk_summary` / `pm_genpd` text, not SCMI messages.

| # | What the snapshot shows | Compared with this note | Tag |
|---|---|---|---|
| 1 | The `dpu` node `clocks` cells name, in order, clock-controller cells `0x50, 0x4b, 0x4c, 0x4e`, then the `ldb` fixed-factor clock, then cell `0x25`, with `clock-names` `pix apb axi ocram ldb ldb_vco` | 0x50 = 80 `DISP1PIX`, 0x4b = 75 `DISPAPB`, 0x4c = 76 `DISPAXI`, 0x4e = 78 `DISPOCRAM`, 0x25 = 37 `LDBPLL_VCO`: **agrees** with §2.1 and §5 | observed 2026-11-27 |
| 2 | `dpu` and `dispmix_csr` `power-domains` cell `0xd` | 13 = `DISPLAY`: **agrees** with §3 | observed 2026-11-27 |
| 3 | `clk_summary`: `ldbpll_vco` 2986200000 Hz, `ldbpll` 497700000 Hz, `ldb_pll_div7` 71100000 Hz; `ldb@4` `assigned-clock-rates` 2986200000 / 497700000 | the rates quoted from NXP's panel file in §2.1 and §8 row 3: **agree** (here as the board's own values). The kernel (AP-NS) set them, so AP-NS holds rate rights on these two PLLs on this board, as `mx95evk` gives it (§2.3) | observed 2026-11-27 |
| 4 | `clk_summary`: `dispapb` 133333333 Hz, `dispocram` 400000000 Hz, `dispaxi` **800000000** Hz, all enabled, consumer the DPU | `DISPAPB` and `DISPOCRAM` agree with the `dispmix_csr` `assigned-clock-rates` of §2.1. **`DISPAXI` disagrees:** LX says 400000000 Hz, the board shows 800000000 Hz | observed 2026-11-27 |
| 5 | `clk_summary`: `disp1pix` 24000000 Hz, enable count 0 | `DISP1PIX` is not driven for LVDS (§2.1, §8 row 7): **agrees** | observed 2026-11-27 |
| 6 | `pm_genpd`: domain `display` on, with the DPU `active`; `camera` on | consistent with §3 (one display domain, no LVDS domain) | observed 2026-11-27 |
| 7 | `firmware/scmi` has protocol nodes 0x11, 0x12, 0x13, 0x14, 0x15, 0x19, 0x81, 0x84; dmesg: SCMI protocol v2.1 `NXP:IMX`, firmware version 0x23e, "Initialized 13 performance domains" | node list: same as §4's LX list, no reset (0x16) node, **agrees**. 13 perf domains = IDs 0-12, so `IMX95_PERF_DISP` = 12 (§2.3) is the last one: **consistent** | observed 2026-11-27 |

Open values (not settled; no more research done in this pass):

- **`DISPAXI` rate.** Candidate A: 400000000 Hz, from `dispmix_csr`
  `assigned-clock-rates` = `<400000000>, <400000000>, <133333333>` for
  `DISPAXI`, `DISPOCRAM`, `DISPAPB` (LX `imx95.dtsi:2358-2364` at
  `lf-6.6.52-2.2.2`, re-read 2026-11-28; read from source). Candidate B: 800000000 Hz,
  the board's `clk_summary` (observed 2026-11-27). Possible causes are the
  Toradex kernel DT, or the SM re-setting the rate; neither was read. For the
  board, B is the value that was in effect.
- **Verdin rights.** Every permission in §2.3 and §3 is NXP's `mx95evk`
  (read from source). Whether the Verdin SM config matches is **open**:
  candidate A, same as `mx95evk` (inferred; observation 3 above fits for AP-NS
  rate rights); candidate B, anything else. The Verdin `verdin-imx95.cfg` was
  not read here (see [imx95-verdin-sm-config.md](imx95-verdin-sm-config.md)).
  The M7 agent's rights on the Verdin are not observed by these snapshots.
- **Firmware version.** The SM reports 0x23e (observed 2026-11-27). Which
  imx-sm commit that is was not established, so the stand-in pin of §1 stays a
  stand-in.
