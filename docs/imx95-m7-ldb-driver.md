# i.MX95 LDB / LVDS driver for Zephyr on the M7: design note (M2-10)

Design note, written 2026-11-28 for `docs/open-issues.md` M2-10. Desk research
only: **no board was accessed, no code was written, nothing was built or run,
and nothing here is a hardware claim.** Every statement carries one of two
tags:

- **read from source**: the named file at the pin in §1 says it. Line numbers
  are approximate (counted by hand on fetched text) unless stated; function
  names are the reliable anchor.
- **inferred**: concluded from what was read, not stated by any source.

A gap is written **NOT READ**. Two dated observations of the board are
cited as such (**observed**, both run 2026-11-27, from
[imx95-zephyr/observed-2026-11-27.txt](imx95-zephyr/observed-2026-11-27.txt) (S1)
and [imx95-zephyr/observed-2026-11-28.txt](imx95-zephyr/observed-2026-11-28.txt) (S2);
the second file's name carries the planner's date, its run date is 2026-11-27);
this note observed nothing itself.

Question: upstream Zephyr has no LVDS/LDB driver, binding or devicetree node
for the i.MX95, yet the Verdin iMX95's panel is LVDS. What would an LDB driver
for the M7 have to program, how would it be described in devicetree and chained
to the `socionext,dpu` node, what would it need from the System Manager (SM),
and what would writing it cost?

What this note does not repeat (read these for the surrounding facts):

| Topic | Note |
|---|---|
| Linux's DT graph, block base addresses, bring-up order, the board's observed display nodes | [imx95-m7-lvds-route.md](imx95-m7-lvds-route.md), [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md) |
| Who may set which display clock under the SM; the LDB PLL path | [imx95-display-clocks.md](imx95-display-clocks.md) |
| Display power domain, what the SM restores after a power cycle | [imx95-display-power.md](imx95-display-power.md) |
| Zephyr's SCMI client and `soc_init()` | [imx95-m7-scmi.md](imx95-m7-scmi.md) |
| The Zephyr DPU driver, its DSI pairing and the LVDS gap | [imx95-m7-dpu-driver.md](imx95-m7-dpu-driver.md) |
| Display interrupt routing to the M7 | [imx95-m7-dpu-irq.md](imx95-m7-dpu-irq.md) |
| NXP MCUXpresso display examples | [imx95-mcux-display.md](imx95-mcux-display.md) |

## 1. Sources and pins

| Id | Source | Pin | Files read |
|---|---|---|---|
| LX | NXP linux-imx, <https://github.com/nxp-imx/linux-imx> | tag `lf-6.6.52-2.2.2`, annotated tag object `f9a8e33f5cde3e96a57569e9dfa70481fe688943`, commit `5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc` (both resolved with `git ls-remote` on 2026-11-28) | raw files at `https://raw.githubusercontent.com/nxp-imx/linux-imx/5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc/<path>`: `drivers/gpu/drm/bridge/imx/imx95-ldb.c`, `imx-ldb-helper.{c,h}`, `imx95-pixel-link.c`, `imx95-pixel-interleaver.c`; `drivers/phy/freescale/phy-fsl-imx8mp-lvds.c`; `drivers/clk/imx/imx95-blk-ctrl.c`; `Documentation/devicetree/bindings/display/bridge/nxp,imx95-pixel-link.yaml`, `nxp,imx95-pixel-interleaver.yaml`, `fsl,ldb.yaml` |
| MCUX | NXP mcuxsdk-core, <https://github.com/nxp-mcuxpresso/mcuxsdk-core> | commit `19b4cd9c4151edd6165306c82e750b06a9a4c54b` (the pin of [imx95-mcux-display.md](imx95-mcux-display.md), main of 2026-11-23) | `drivers/ldb_1/fsl_ldb.h`, `fsl_ldb.c` at `https://raw.githubusercontent.com/nxp-mcuxpresso/mcuxsdk-core/19b4cd9c4151edd6165306c82e750b06a9a4c54b/drivers/ldb_1/` |
| ZY | Zephyr, <https://github.com/zephyrproject-rtos/zephyr> | `main` commit `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (2026-11-27), as pinned by [imx95-m7-dpu-driver.md](imx95-m7-dpu-driver.md); tag `v4.5.0` = commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966` as pinned by [imx95-m7-scmi.md](imx95-m7-scmi.md) | Not re-read here. Zephyr-side facts below are taken from those two notes and say so |
| HAL | `hal_nxp`, <https://github.com/zephyrproject-rtos/hal_nxp> | commit `1a8937f3f8dfe5c68f59cc2a0c0c0fccdb096650` (the revision `west.yml` pins at ZY) | Not re-read here; its LDB directories are named by [imx95-m7-scmi.md](imx95-m7-scmi.md) |
| SM | NXP imx-sm, <https://github.com/nxp-imx/imx-sm> | tag `rel_imx_sm_2025q3` = commit `af1e37026c6ba19cdca98fd6e91494efd3e7b5ec` | Not re-read here; cited through [imx95-display-clocks.md](imx95-display-clocks.md) and [imx95-display-power.md](imx95-display-power.md) |
| TDX | Toradex linux-toradex, `https://git.toradex.com/linux-toradex.git`, branch `toradex_6.6-2.2.x-imx` | commit `64f85d65c112c45ceb6c7e0566885afc62b1a551` (the board's 6.6.157 kernel, as recorded in [imx95-display-clocks.md](imx95-display-clocks.md)) | **NOT READ** by this note. The Verdin and Mallow DT files at that commit are read in [imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md); the Toradex 10.1" overlays at `https://git.toradex.com/device-tree-overlays.git` commit `e9a3ca6fbd6bd5fab7f00cdb87360be44373cf69` are read in [imx95-verdin-lvds-panel.md](imx95-verdin-lvds-panel.md) |

Limits, stated plainly:

- **Kernel pin.** The board runs 6.6.157-7.8.0. `lf-6.6.52-2.2.2` is the newest
  `lf-6.6.*` release tag in `nxp-imx/linux-imx` (the tags listed on 2026-11-28
  were `lf-6.6.3-1.0.0`, `lf-6.6.3-imx95-er2`, `lf-6.6.23-2.0.0`,
  `lf-6.6.23-imx95-er1`, `lf-6.6.36-2.1.0`, `lf-6.6.52-2.2.0` to `-2.2.2`,
  `lf-6.6.y-imx95-er1`), so no tag matches 6.6.157. That the board's Toradex
  tree carries the same LDB, pixel-link and PHY code is **inferred** (the
  observed board nodes match these drivers' compatibles, see §3, which supports
  but does not prove it).
- **No reference manual.** The i.MX95 reference manual was not read. Register
  and bit meanings below are the names Linux and the MCUX driver give them, not
  a hardware specification. The MCUX SoC header that defines the `LDB_Type`
  register offsets was **NOT READ**.
- **How the files were read.** `git clone` and direct downloads were refused on
  this node; the files were read through a page fetcher at the commits above.
  The fetcher returned the text verbatim, but without line numbers.
- **The HAL copy of the MCUX driver was not compared** with the `mcuxsdk-core`
  one; that they are the same code is **inferred** from the HAL directory name
  `ldb_1`.

## 2. Where the LDB sits (read from source)

The route, from [imx95-m7-lvds-route.md](imx95-m7-lvds-route.md) and
re-checked against the LX bindings and drivers:

```
DPU FrameGen n -> pixel interleaver (channel n, bypass by default)
               -> pixel link (stream n, 1 pixel/clock) -> LDB channel -> LVDS PHY -> panel
```

| Fact | Tag |
|---|---|
| The pixel link "is designed to transport 1 pixel per clock"; ports 0/1 are the inputs from the DPU display streams 0/1, ports 2/3 the outputs. Its only register is `CTRL` at offset 0x8 of the display-mix CSR (its parent syscon); it has no clock and no interrupt | read from source (LX `nxp,imx95-pixel-link.yaml`, `imx95-pixel-link.c`) |
| The pixel interleaver has its own register block (stream registers 0x10000 apart), one bus clock (`DISPAPB` in the binding example), one interrupt (62 on the display irqsteer in the example), and reaches `PIXEL_INTERLEAVER_CTRL` (offset 0x4) in the display-mix CSR through the phandle `nxp,blk-ctrl`. Property `nxp,pixel-interleaving-stream-id` 0 or 1 selects "split 2" mode for that stream; absent selects bypass | read from source (LX `nxp,imx95-pixel-interleaver.yaml`, `imx95-pixel-interleaver.c`) |
| The LDB driver (`fsl,imx95-ldb`) is a child of the LVDS CSR syscon: its regmap is `syscon_node_to_regmap(parent)`, its control register is offset 0x4, and the PHY clock control is offset 0x0 of the same regmap. It has up to two channel children (`reg` 0 or 1); each channel's port 1 endpoint 0 is the next bridge, and each channel takes a PHY named `lvds_phy` | read from source (LX `imx95-ldb.c`: `imx95_ldb_probe`, `ldb_init_helper`, `ldb_find_next_bridge_helper`, `imx95_ldb_get_phy`) |
| LDB inputs and outputs in the Linux driver: it requests the four clocks `ldb_ch0`, `ldb_di0`, `ldb_ch1`, `ldb_di1` (all required) | read from source (LX `imx95_ldb_probe`) |
| The Linux tree has **no** `fsl,imx95-ldb`, no `fsl,imx95-lvds0-phy` and no `fsl,imx95-dispmix-lvds-csr` binding file at the pin: the bridge directory has only `fsl,ldb.yaml` (listing `fsl,imx6sx-ldb`, `fsl,imx8mp-ldb`, `fsl,imx93-ldb`) and `fsl,imx8qxp-ldb.yaml`; the PHY directory has no i.MX95 file; a search of `bindings/soc/imx` found no CSR file. The `bindings/clock/` directory listing was cut off, so a CSR binding there is **NOT READ** | read from source (absence), partly NOT READ |
| Linux's PHY driver is the i.MX8MP one reused: compatibles `fsl,imx8mp-lvds-phy`, `fsl,imx93-lvds-phy`, `fsl,imx95-lvds0-phy`, `fsl,imx95-lvds1-phy`. The two i.MX95 ones differ only in the PHY control register offset: 0x8 (PHY 0) and 0xc (PHY 1); both have the `DISABLE_LVDS` bit | read from source (LX `phy-fsl-imx8mp-lvds.c`, `imx8mp_lvds_phy_devdata`, `imx8mp_lvds_phy_of_match`) |
| The board's DT has `ldb@4` (`fsl,imx95-ldb`) under `syscon@4b0c0000` (`fsl,imx95-dispmix-lvds-csr`) `okay`, `channel@0` `okay`, the pixel link `okay` under `syscon@4b010000`, and a root `panel-lvds-native` node (`compatible = panel-lvds`, with `data-mapping`, `panel-timing`, `width-mm`, `height-mm`, `backlight`) | observed (2026-11-27) |
| The live `panel-lvds-native` has `data-mapping` = `vesa-24` and a `panel-timing` of 1280 x 800 at 68.9 / 71.1 / 73.4 MHz (min/typ/max), porches 23/60/71 (h), 5/7/10 (v), sync lengths 15/40/47 and 6/9/12, hsync and vsync active 0, de-active 1, `pixelclk-active` 1, 217 x 136 mm. No `data-width` property was listed. `ldb@4` `assigned-clock-rates` are 2 986 200 000 and 497 700 000 Hz; `channel@0` `okay`, `phy@8` `reg` = `<0x8 0x4>` | observed (2026-11-27, S2; decoded in its header). They equal the original native Toradex overlay of [imx95-verdin-lvds-panel.md](imx95-verdin-lvds-panel.md) §3b |
| `channel@1` `status` was **not dumped** in either snapshot; only its clock gates being off is observed (S1 `clk_summary`). "Channel 1 disabled" is not observed | observed (gates), NOT READ (status) |
| The carrier: the machine string is "Verdin Development Board", not Mallow, and equals the `model` of `imx95-verdin-wifi-dev.dts` at Toradex commit `64f85d65c112c45ceb6c7e0566885afc62b1a551` (read from source 2026-11-28); "Mallow" is the plan's description, not a board or source fact. NXP's EVK Verdin-10" overlay (`logictechno,lt170410-2whc`) is NXP's adaptation, not this board's tree, and is not used for board facts | observed (model string, 2026-11-27, S2); see [imx95-verdin-lvds-panel.md](imx95-verdin-lvds-panel.md) §2 |

## 3. What an LDB driver has to program

All offsets are within the LVDS CSR (DT base `0x4b0c0000`, size 0x10000) unless
stated. Bit names are Linux's. The MCUX names are given where they differ.

### 3.1 LVDS CSR registers (read from source, LX `imx-ldb-helper.h`, `imx95-ldb.c`, `phy-fsl-imx8mp-lvds.c`, `imx95-blk-ctrl.c`)

| Offset | Bit(s) | Linux name | Meaning in Linux | MCUX name (`fsl_ldb.c`, read from source) |
|---|---|---|---|---|
| 0x0 | 0 | `LVDS_PHY_DIV2` / clock `ldb_phy_div` | PHY clock = `ldbpll` / 2 when set (set in split mode only) | `LVDS_PHY_CLOCK_CONTROL.LVDS_PHY_DIV` (set for dual panel) |
| 0x0 | 1, 2 | clocks `lvds_ch0_gate`, `lvds_ch1_gate` | clock gates; `CLK_GATE_SET_TO_DISABLE`, so a **set** bit gates the clock off | not touched by `LDB_Init` |
| 0x0 | 3, 4 | clocks `lvds_di0_gate`, `lvds_di1_gate` | same, for the pixel (DI) clocks | not touched by `LDB_Init` |
| 0x4 | [1:0] | `LDB_CH0_MODE_EN_*` | 0 off, 1 channel 0 from DI0, 3 channel 0 from DI1 | `PIXEL_MAPPER_CONTROL.CH0_MODE` (1 for DI0, 3 for DI1) |
| 0x4 | [3:2] | `LDB_CH1_MODE_EN_*` | same for channel 1 | `CH1_MODE` |
| 0x4 | 4 | `LDB_SPLIT_MODE_EN` | one image over both channels | `SPLIT_MODE` |
| 0x4 | 5 / 7 | `LDB_DATA_WIDTH_CH0_24` / `CH1_24` | set = 24-bit; clear = 18-bit | `CH0_DATA_WIDTH`, `CH1_DATA_WIDTH` (hard-coded 1 = 24-bit) |
| 0x4 | 6 / 8 | `LDB_BIT_MAP_CH0_JEIDA` / `CH1_JEIDA` | set = JEIDA; clear = SPWG | `CH0_BIT_MAPPING`, `CH1_BIT_MAPPING` (`LVDS_SPWG` 0, `LVDS_JEIDA` 1) |
| 0x4 | 9 / 10 | `LDB_DI0_VS_POL_ACT_LOW` / `DI1_...` | VSYNC active low | `DI0_VSYNC_POLARITY` / `DI1_VSYNC_POLARITY` (always set for the selected DI) |
| 0x4 | 13 / 14 | `LDB_DI0_HS_POL_ACT_LOW` / `DI1_...` | HSYNC active low (defined in `imx95-ldb.c`, not in the helper header) | not used by `LDB_Init` |
| 0x4 | 19 | `LDB_VSYNC_ADJ_EN` | always set by Linux | not used by `LDB_Init` |
| 0x8 (PHY 0), 0xc (PHY 1) | 1 | `DISABLE_LVDS` | cleared at power-on, set at power-off | `LVDSx_CONTROL.LVDS_EN` (the MCUX driver **sets** `LVDS_EN` in its first write and leaves it clear in its last; the polarity relation to `DISABLE_LVDS` is **inferred** from the bit positions the two sources imply, not read) |
| | 2 | `BG_EN` | bandgap enable | `BG_EN` |
| | 3 | `HS_EN` | defined, never set | not seen |
| | 4 | `PRE_EMPH_EN` | pre-emphasis | `PRE_EMPH_EN` |
| | [7:5] | `PRE_EMPH_ADJ` | Linux sets 3 | `PRE_EMPH_ADJ` (3) |
| | [13:11] | `CC_ADJ` | Linux sets 2 | `CC_ADJ` (2) |
| | other fields | `SPARE_IN`, `TEST_*`, `VBG_ADJ`, `SLEW_ADJ`, `CM_ADJ` | defined, not written by Linux (only the three above plus the enables) | not seen |
| | `id` (a bit number from the PHY child's `reg`) | `CH_EN(id)` | channel enable | `CH0_EN` (both ports use it) |

Notes:

- **Where the offsets come from.** The 0x0, 0x4, 0x8 and 0xc offsets and the
  clock-gate bit numbers are read from Linux (`imx95-blk-ctrl.c`,
  `LVDS_PHY_CLK_CTRL`, `ctrl_reg = 0x4`, the PHY `lvds_ctrl` data). The MCUX
  driver uses register names only, so its offsets are **NOT READ**; that its
  `PIXEL_MAPPER_CONTROL`, `LVDS0_CONTROL` and `LVDS1_CONTROL` are the same
  registers as Linux's 0x4, 0x8 and 0xc is **inferred** from the identical
  field set.
- **`CH_EN` bit.** `DISABLE_LVDS` is bit 1 and Linux's `CH_EN(id)` is bit
  `id`, where `id` is a number the PHY driver takes from its device data or
  node (the exact source of `id` was not re-read). The board's PHY child
  `phy@8` has `reg` = `<0x8 0x4>`, i.e. the register offset and size within
  the CSR, **not** a channel number (observed 2026-11-27, S2). So an
  assumption that the PHY child's `reg` must be 0 is unsupported, and the
  `reg` of `phy@8` says nothing about `id`. Which bit is the channel enable
  is unresolved: a driver must fix it from the hardware manual, not from this
  note (**NOT READ**).
- **The two sources differ in what they write.** Linux builds a shadow of the
  whole control word, with polarity from the mode flags, and writes it once at
  enable (`imx95_ldb_bridge_atomic_enable`, `ldb_bridge_enable_helper`). The MCUX
  `LDB_Init` ORs bits into the control register without clearing it first,
  sets the VSYNC polarity bit unconditionally, forces 24-bit width, and waits
  100 ms (`SDK_DelayAtLeastUs(100000, ...)`) between its two PHY control
  writes. Linux waits 15-20 us for the bandgap and 5-10 us after the channel
  enable. Which timing the PHY really needs is **NOT READ**. A Zephyr driver
  should follow Linux's sequence (the one used on this board) and treat the
  MCUX one as a second opinion (**inferred**).

### 3.2 Display-mix CSR registers the LVDS route also needs (read from source, [imx95-m7-lvds-route.md](imx95-m7-lvds-route.md) §3.1, confirmed against LX `imx95-pixel-link.c`, `imx95-pixel-interleaver.c`)

| Block (DT base) | Offset | Register | Fields |
|---|---|---|---|
| `dispmix_csr` (0x4b010000) | 0x0 | `CLOCK_CTRL` / the `disp_engine0_sel`, `disp_engine1_sel` muxes | bits [1:0] / [3:2]; 0 = `videopll1`, 1 = `dsi_pll`, 2 = LDB PLL / 7, 3 = dummy (`imx95-blk-ctrl.c`); the DPU frame generator writes 2 for an LVDS encoder |
| | 0x4 | `PIXEL_INTERLEAVER_CTRL` | 0 = bypass; bit 0 `MODE`; bit 1 `DISP_IN_SEL` |
| | 0x8 | pixel link `CTRL` | `PL_ENABLE(n)` = bit 4n, `PL_VALID(n)` = bit 4n+1. Enable sets `VALID`, then `ENABLE`; disable clears `ENABLE`, then `VALID` (`imx95_pl_bridge_enable`, `imx95_pl_bridge_disable`) |
| pixel interleaver (0x4b0d0000) | 0x0 + 0x10000 n | `CTRL` | bit 9 HSYNC, 10 VSYNC, 11 DE polarity, 12 pixel format, 13 YUV conversion. Linux writes HSYNC and VSYNC polarity (both "active low") and leaves DE active high |
| | 0x20 + 0x10000 n | `SWRST` | bit 1; Linux pulses it for 10-20 us |
| | 0x30 / 0x40 + 0x10000 n | `IE` / `IS` | FIFO overflow bits 0 and 1; write-one-to-clear status |
| | 0x50 + 0x10000 n | `ICTRL` | width [11:0], written only in a split-2 mode (`hdisplay / 2`) |

All of the above is read from source. That a minimal single-channel LVDS
output can leave the interleaver in bypass (never touching its block) is
**inferred** from Linux's default (no `nxp,pixel-interleaving-stream-id` means
`BYPASS`, and `PIXEL_INTERLEAVER_CTRL` is written 0). Linux does reset the
interleaver channel at probe and again in `mode_set` regardless of mode; whether
that reset is required is **NOT READ**.

### 3.3 Data mapping and bit width (read from source)

| Output bus format Linux accepts (`imx95_ldb_bus_output_fmts`) | LDB_CTRL bits set (`ldb_bridge_mode_set_helper`) |
|---|---|
| `MEDIA_BUS_FMT_RGB666_1X7X3_SPWG` (18 bpp) | none: width bit clear, SPWG |
| `MEDIA_BUS_FMT_RGB888_1X7X4_SPWG` (24 bpp) | `DATA_WIDTH_CHn_24` |
| `MEDIA_BUS_FMT_RGB888_1X7X4_JEIDA` (24 bpp) | `DATA_WIDTH_CHn_24` and `BIT_MAP_CHn_JEIDA` |
| `MEDIA_BUS_FMT_FIXED` | none set (the mapping bits stay clear, so 18-bit SPWG); the **input** format is then chosen from the connector's first bus format: 18-bit depth gives `RGB666_1X36_CPADLO`, anything else `RGB888_1X36_CPADLO` |

Notes:

- There is no JEIDA 18-bit combination; JEIDA is 24-bit only in Linux's list
  (read from source). Whether the hardware has one is **NOT READ**.
- The board's panel node has `data-mapping` = `vesa-24` (observed 2026-11-27,
  S2), the 24-bit SPWG-style mapping, so Linux's choice would be
  `RGB888_1X7X4_SPWG` (`DATA_WIDTH_CHn_24`, JEIDA clear) (**inferred**: the
  mapping from `vesa-24` to that format is not read in the Linux panel code).
  No `data-width` property was listed.
- The input side: the LDB asks the DPU/pixel link for `RGB888_1X36_CPADLO`
  (24-bit) or `RGB666_1X36_CPADLO` (18-bit); the pixel link accepts
  `RGB888_1X36_CPADLO` or `FIXED` outputs and demands `RGB888_1X24` on its
  input (`imx95_pl_bridge_atomic_get_input_bus_fmts`); the interleaver likewise
  requires `RGB888_1X24`. That the DPU therefore always emits 24-bit RGB on the
  link and an 18-bit panel only changes the LDB width bit is **inferred**.
  How the 18-bit case truncates the bits (the DPU, the LDB or the PHY) is **NOT
  READ**.
- A Zephyr driver has no media-bus-format negotiation (inferred, nothing of the
  kind is in the Zephyr display API). It would take mapping and width from DT
  (§5).

### 3.4 Channels and dual-channel modes (read from source)

| Mode | What Linux does | Tag |
|---|---|---|
| Single link, channel 0 | `CH0_MODE` = 1 (from DI0); PHY 0 powered; clock gates `lvds_ch0`, `lvds_di0`; `LVDS_PHY_DIV2` clear | read from source (`imx95_ldb_bridge_atomic_enable`) |
| Single link, channel 1 | `CH1_MODE` = 3 (from DI1); PHY 1 powered; clock gates `lvds_ch1`, `lvds_di1` | read from source |
| Dual link ("split") | needs both channels `okay`; `SPLIT_MODE` set; `CH0_MODE` = 1 and `CH1_MODE` = 1 (**both from DI0**); both PHYs initialised and powered; `LVDS_PHY_DIV2` set; width and mapping bits set for both channels; both channels' clock gates enabled | read from source |
| Dual link pixel order | `drm_of_lvds_get_dual_link_pixel_order` on the two channels' port 1; `DRM_LVDS_DUAL_LINK_EVEN_ODD_PIXELS` is rejected with `-EINVAL`, only odd-even is accepted | read from source (`imx95_ldb_probe`) |
| Limits | `mode_valid` rejects a pixel clock above 330 MHz, and above 165 MHz on a single link | read from source (`imx95_ldb_bridge_mode_valid`) |
| MCUX | `LDB_Init(base, diIndex, dualpanelIndex, datamap)` configures **both** channels identically (`CH0_MODE` and `CH1_MODE` both set from `diIndex`), 24-bit only, with split mode when `dualpanelIndex` is 1 | read from source (`fsl_ldb.c`) |

The board uses channel 0 only (**observed**, 2026-11-27: `channel@0` `okay`,
`lvds_ch0_gate` and `lvds_di0_gate` enabled, channel-1 gates off).

**Which DPU display index feeds the LDB** is not settled: the Linux graph sends
DPU display stream 0 through interleaver channel 0 and pixel link port 0 to LDB
channel 0 with `CH0_MODE` = DI0 (read from source, LX and
[imx95-m7-lvds-route.md](imx95-m7-lvds-route.md)). The MCUX example selects
display index 1 (base `0x4B7A0000`) when its interface is LVDS and 0
(`0x4B780000`) for MIPI ([imx95-mcux-display.md](imx95-mcux-display.md), read
from source), while the Zephyr DPU driver hard-codes `0x4B780000`
([imx95-m7-dpu-driver.md](imx95-m7-dpu-driver.md), read from source). Whether
these are two valid routes, or one of them is a different numbering, is **NOT
READ** (inferred: the LDB `DI` number is the LDB's own input index and need not
equal the DPU display number). An M2 item must settle this before choosing the
Zephyr DPU's display stream (§8, item L1).

### 3.5 Pixel clock and LDB clock ratio (read from source, with the observation)

| Clock | Relation | Tag |
|---|---|---|
| `ldb_pll_div7` (LDB pixel clock, the DPU frame generator's source when `CLOCK_CTRL` = 2) | `LDBPLL` / 7 | read from source ([imx95-display-clocks.md](imx95-display-clocks.md) §2.3: a DT `fixed-factor-clock`) |
| `lvds_diN_gate` | gates `ldb_pll_div7`, so the LDB DI clock equals the pixel clock | read from source (`imx95-blk-ctrl.c`) |
| `ldb_phy_div` -> `lvds_chN_gate` (PHY serial clock) | `LDBPLL` / 1 (bit 0 clear, single link) or `LDBPLL` / 2 (bit 0 set, split mode), so the PHY clock is 7 x the pixel clock for a single link and 3.5 x for a split link | read from source (divider flags, `imx95_ldb_bridge_atomic_enable`) for the divider; the 7x / 3.5x ratios are **inferred** arithmetic |
| Linux's request | `clk_set_rate(clk_di, mode_clock * 1000)` in `mode_set`, return value ignored. On the board the PLL rates are set in the DT (`assigned-clock-rates` 2986.2 MHz VCO, 497.7 MHz `ldbpll`, observed 2026-11-27, S2), and per [imx95-display-clocks.md](imx95-display-clocks.md) §3.2 the request does not reach the SCMI PLL | read from source; the last clause is **inferred** there |
| Board | `ldbpll_vco` 2986.2 MHz, `ldbpll` 497.7 MHz, `ldb_pll_div7` 71.1 MHz enabled; 497.7 / 7 = 71.1 | observed (2026-11-27); the ratio is inferred arithmetic |

So an M7 driver must make `LDBPLL` = 7 x the panel's pixel clock, and the
pixel clock must come from the panel's `panel-timing`. The VCO/PLL pair that
gives an exact 7x for a given panel clock is a fractional-PLL question (the
SM's `FRACTPLL` code, [imx95-display-clocks.md](imx95-display-clocks.md) §4) and
is **NOT READ** here; the only evidence is the board's own pair (2986.2 / 497.7
MHz, VCO / 6; observed 2026-11-27, S1 and S2), which yields 71.1 MHz, the
panel's typical pixel clock.

## 4. What the driver needs from the SM (the M7 agent)

Summary, all from the cited notes. The M7 agent is `SCMI_AGENT0`, LM1, in NXP's
`mx95evk` configuration. The Verdin's SM configuration is **NOT READ** by this
note; every permission row below is NXP's reference, and that the Verdin image
grants the same is **inferred**.

### 4.1 SCMI clocks (IDs from [imx95-display-clocks.md](imx95-display-clocks.md) §2.1, read from source)

| Clock | ID | Needed for | M7 permission under `mx95evk` | Note |
|---|---|---|---|---|
| `LDBPLL_VCO` | 37 | PLL parent; Linux enables it (and `ldb_pll_div7`) when the frame generator clocks start | none (the LM1 agent has no display clock line) | read from source |
| `LDBPLL` | 38 | PLL; the 7 x pixel clock | none; AP-NS (Linux) holds `LDB_PLL ALL`; usable only while `PD_DISPLAY` is on | read from source |
| `DISPAPB` | 75 | register access to the CSRs, PHY and interleaver (Linux enables it around each PHY access) | none | read from source |
| `DISPAXI`, `DISPOCRAM` | 76, 78 | DPU fetch; not an LDB need | none | read from source |
| `DISP1PIX`, `VIDEOPLL1` | 80, 19 | **not** in the LVDS path (DSI only) | none | read from source |

The LVDS CSR gates (`ldb_phy_div`, `lvds_ch0/1_gate`, `lvds_di0/1_gate`) and the
`disp_engine0/1_sel` muxes are **register bits, not SCMI clocks**
([imx95-display-clocks.md](imx95-display-clocks.md) §2.3, read from source). A
Zephyr LDB driver therefore writes them itself, and the SM does not mediate
them; access is a TRDC question (**inferred**, same note). Zephyr's
`imx95_clock.h` has no ID for them (read from source,
[imx95-m7-lvds-route.md](imx95-m7-lvds-route.md) §5).

Zephyr-side cautions for the three SCMI clock calls (read from source,
[imx95-m7-scmi.md](imx95-m7-scmi.md)):

- `scmi_clock_on_off` returns success without acting when a clock has
  restrictions and state control is not allowed, so a denied `LDBPLL` enable
  would look like success (**read from source**). A driver must read the clock
  state back, or use `clock_control_get_rate`, after enabling.
- The SCMI `clock_control_set_rate` path passes the `rate` argument as a
  pointer-sized value (`cfg.rate[0] = (uintptr_t)rate`), and the one existing
  caller (DSI) appears to pass a pointer where the number is expected
  (**read from source**; the interpretation as a defect is that note's
  **inference**, untested). The LDB driver must pass the form the SCMI driver
  expects, which has to be checked against the pinned `clock_control_arm_scmi.c`
  before the first call.
- Zephyr's `soc_init()` sets `VIDEOPLL1`, `DISP1PIX` and the MIPI PHY clocks only
  under `CONFIG_MIPI_DSI_NXP_DWC` (read from source). An LVDS-only image would
  not enable that option, so none of the SCMI calls would run unless the LDB
  code (or a new `soc_init()` branch) made them.

### 4.2 Power

- One domain: `IMX95_PD_DISPLAY` = 13 (SM `DEV_SM_PD_DISPLAY`). The LVDS CSR,
  the PHYs, the display-mix CSR and the DPU all sit on it in Linux's DT. There
  is no separate LVDS or PHY domain (read from source,
  [imx95-display-power.md](imx95-display-power.md) §2).
- `PD_CAMERA` (3) is needed only for the DSI branch (the display-master CSR and
  the DSI block are on it). Whether an LVDS-only output needs it is **inferred**
  as no ([imx95-display-power.md](imx95-display-power.md) §2).
- Zephyr's `soc_init()` already powers `PD_DISPLAY` through
  `scmi_power_state_set`, but only under the DSI Kconfig symbol, and it then
  polls without a bound, so a domain that stays off hangs boot (read from
  source, [imx95-m7-scmi.md](imx95-m7-scmi.md)). The `dpu` node has no
  `power-domains` (read from source). A new LDB node should carry
  `power-domains = <&disp_mix>` (the Zephyr M7 dtsi already has the `disp_mix`
  SCMI power-domain node, read from source) and should be the thing that
  triggers power-on, so LVDS does not depend on DSI configuration (**inferred**
  design choice).
- After any `PD_DISPLAY` power cycle all CSR, interleaver and DPU registers are
  lost; the SM restores only TRDC D, its board list and the LDB PLL context
  ([imx95-display-power.md](imx95-display-power.md) §3, read from source). The
  driver must keep its configuration and re-apply it, and it gets no
  notification (inferred from the same note).
- Whether the M7 may switch `PD_DISPLAY` at all, and whether Linux's use of it
  is compatible with the M7 powering it, is the SM question of
  [imx95-sm-display-lm.md](imx95-sm-display-lm.md); it is **not answered** here
  and gates the whole design.

### 4.3 What is not an SCMI matter

- **MMIO access** to `0x4b0c0000`, `0x4b010000`, `0x4b0d0000`: TRDC rights for
  the M7 domain (**inferred**, [imx95-display-clocks.md](imx95-display-clocks.md)
  §2.3). Without them the first register write faults. Nothing about the
  Verdin's TRDC set-up was read.
- **Interrupts**: the LDB and the pixel link have no interrupt (read from
  source). The pixel interleaver has one (62 on the display irqsteer, FIFO
  overflow only). Routing it to the M7 is the irqsteer question of
  [imx95-m7-dpu-irq.md](imx95-m7-dpu-irq.md); an LDB driver can leave it unused
  (**inferred**: it is a diagnostic, not part of scanout).
- **Backlight and panel power**: not part of the LDB. The board has a
  `pwm-backlight` node with a 6 666 667 ns PWM period and eight brightness
  levels (**observed**, 2026-11-27, S2; its PWM controller phandle and enable
  GPIO were not decoded). Zephyr has TPM PWM nodes in the M7 dtsi (read from
  source, [imx95-m7-lvds-route.md](imx95-m7-lvds-route.md) §5). The Toradex
  overlay uses `&tpm5` channel 0 ([imx95-verdin-lvds-panel.md](imx95-verdin-lvds-panel.md)
  §5, read from source); that the board's backlight uses it is **inferred**
  (period and polarity match), and which carrier pins it reaches is **NOT READ**.

## 5. Zephyr devicetree binding and how it chains to the DPU

### 5.1 Current state (read from source, [imx95-m7-dpu-driver.md](imx95-m7-dpu-driver.md), [imx95-m7-lvds-route.md](imx95-m7-lvds-route.md))

- `socionext,dpu` has `reg`, an interrupt list, and takes timings from a
  `display-timings` child; its description says "for DSI panels". The driver
  hard-codes the DPU base and calls no clock, power or SCMI function.
- The DSI host driver `nxp,mipi-dsi-dwc` is wired by a phandle property
  `nxp,dc = <&dpu>` and reads width, height and porches from the **DPU node's**
  `display-timings` (`dsi_nxp_dwc.c`). It is the only output path the DPU
  driver has. There is no Zephyr abstraction for "a bridge between the DPU and a
  panel": the chaining is a driver-private phandle.
- No LDB, LVDS CSR, PHY, pixel-link or interleaver node, binding or driver
  exists in upstream Zephyr (read from source, ZY, per the two notes above).
- The HAL does carry NXP's LDB driver directories (`ldb`, `ldb_1`,
  `ldb_combo_phy`), but no Zephyr driver or SoC code references them (read from
  source, [imx95-m7-scmi.md](imx95-m7-scmi.md)). That the Zephyr HAL build could
  compile `ldb_1` the way it selects `dpu_1` for the DPU (a
  `set_variable_ifdef` line in `drivers.cmake`) is **inferred** from that
  pattern ([imx95-m7-dpu-driver.md](imx95-m7-dpu-driver.md)).

### 5.2 Proposed shape (inferred design; nothing exists)

Follow the DSI precedent: an output driver that points back at the DPU with
`nxp,dc` and borrows its `display-timings`. No change to `socionext,dpu` is
needed for that direction. This gives three small nodes:

| Node | Compatible (proposed) | Properties (proposed) | Source of the design |
|---|---|---|---|
| Display-mix CSR | `nxp,imx95-dispmix-csr` (or a plain `reg` on the LDB node, see below) | `reg = <0x4b010000 0x10000>`, `power-domains = <&disp_mix>` | Linux's `fsl,imx95-dispmix-csr` syscon (read from source); Zephyr has no regmap, so the register block is mapped directly (inferred) |
| LVDS CSR + LDB | `nxp,imx95-ldb` | `reg = <0x4b0c0000 0x10000>`; `nxp,dc = <&dpu>`; `nxp,dispmix-csr = <&dispmix_csr>`; `clocks` and `clock-names` for `ldbpll-vco`, `ldbpll`, `apb`; `power-domains = <&disp_mix>`; a child-binding per channel | Linux `fsl,imx95-ldb` and its syscon parent (read from source); merging the CSR and the LDB into one node because the LDB regs are CSR offsets 0x0-0xc and Zephyr has no syscon parent pattern (inferred) |
| Channel (child) | `reg` 0 or 1; `data-mapping` enum `spwg`, `jeida`; `data-width` enum 18, 24; `dual-link` boolean (channel 0 only, implies channel 1); `phy-ctrl-offset` or per-channel fixed 0x8 / 0xc; optional `phy-cc-adj`, `phy-pre-emph-adj` defaulting to Linux's 2 and 3 | Linux `imx95_ldb_probe`, `imx95_ldb_get_phy`, `ldb_bridge_mode_set_helper` (read from source); property names are mine (inferred) |
| Pixel link and interleaver | folded into the LDB driver as register pokes, or a second small node `nxp,imx95-pixel-link` with `reg-offset = <0x8>` | Linux keeps them as separate bridges because DRM wants a bridge chain; Zephyr's chain is a phandle, so separate nodes only add boilerplate (inferred) |
| Panel | the DPU's own `display-timings` child, plus new properties on the **LDB channel** for polarity (VS/HS active low) taken from the timing's `hsync-active` / `vsync-active` | Zephyr's DPU driver already reads `display-timings` (read from source); that the same node carries the polarity flags is **inferred** and must be checked against the DPU binding |
| Backlight | a `pwms` property and a GPIO on the board overlay, driven by the application | the panel needs a backlight outside the LDB (inferred) |

Why not a Linux-style `panel-lvds` node: the generic `panel-lvds` binding is a
Linux DT convention; its allowed `data-mapping` strings (`vesa-24`, `jeida-18`,
`jeida-24`) were **NOT READ** at the LX pin. The observed board node has the
property with the value `vesa-24` (observed, 2026-11-27, S2), so a Zephyr port of the
Verdin overlay could translate it, but that translation table is unwritten.

### 5.3 Chaining and enable order (inferred)

Linux enables the DPU frame generator's clocks, the pixel link, the LDB and the
PHY through the DRM bridge ordering, which was **not re-read** here. A Zephyr
driver has no such ordering, so it must choose one. A conservative order that
follows each block's own Linux sequence (read from source in §3):

1. Power `PD_DISPLAY`; enable `DISPAPB`; clear nothing else.
2. LDB: write `LDB_CTRL` = 0, program PHY control (`CC_ADJ` 2, `PRE_EMPH_EN`,
   `PRE_EMPH_ADJ` 3), set the PLL rate (SCMI), clear the clock gates for the
   chosen channel and DI.
3. `dispmix_csr` `CLOCK_CTRL`: select LDB PLL / 7 for the DPU stream; interleaver
   in bypass.
4. DPU: the existing Zephyr DPU init and timing programming, with the display
   timing and polarity from the DT.
5. Pixel link: `VALID` then `ENABLE`.
6. LDB: set channel mode and polarity/width/mapping bits, power the PHY (set
   `BG_EN`, clear `DISABLE_LVDS`, wait about 15-20 us, set `CH_EN`, wait about
   5-10 us), then write `LDB_CTRL` once.
7. Backlight on.

Whether the hardware tolerates this order (versus Linux's, where the DPU stream
starts after the LDB is enabled) is **not known**; it is the first thing a bring-up
has to try (§8, item L6). The chaining direction (LDB `nxp,dc` -> DPU) means the
DPU driver would need a hook, or the LDB driver would be initialised at
`POST_KERNEL` before the DPU (init priority), because the DPU's `dpu_init`
starts scanout (read from source, [imx95-m7-dpu-driver.md](imx95-m7-dpu-driver.md))
and Zephyr's display API has no separate "enable output" call besides
`display_blanking_off`. Either an `nxp,dc` back-reference in the DPU driver, a
board hook, or init priorities solves it; which is least intrusive upstream is
**NOT decided** here.

## 6. What the design does not settle

| Gap | Why it matters |
|---|---|
| SM permissions of the Verdin M7 LM for `PD_DISPLAY`, `LDBPLL*`, TRDC on the CSRs | Gates every item; the gated chain (`sm-config-read` -> `m7-run2`) answers it. Without it this note is a paper design |
| Panel part, timing, `data-mapping`, pixel clock | Captured (observed 2026-11-27, S2): `vesa-24`, 71.1 MHz typ, polarity active low. Open: the panel part number (the live compatible is the generic `panel-lvds`) and whether another panel revision (V2, 64.5 MHz) must be supported |
| LDB `DI` index versus DPU display index (§3.4) | Decides whether the Zephyr DPU's hard-coded display 0 can feed the LDB |
| PHY channel-enable bit (§3.1) | A wrong bit would disable or collide with `DISABLE_LVDS`. The PHY child `reg` (`<0x8 0x4>`, observed) is an offset, not a channel number, so it does not settle the bit |
| Order of enable, and whether the DPU may start before the LDB (§5.3) | Bring-up risk |
| TRDC rights and whether Linux and the M7 can alternate the route | See [imx95-display-handover.md](imx95-display-handover.md); the CSRs hold one set of control registers, and both owners would write the same words (inferred) |
| Reference manual | Nothing here is checked against the hardware specification |

## 7. What was captured and what is still open

Captured read-only from the live DT on 2026-11-27 (S2): the `panel-timing` node,
`data-mapping`, the PHY child `reg` (`<0x8 0x4>`) and the PHY compatible
(`fsl,imx95-lvds0-phy` at `phy@8`, per the node names; the compatible text was
dumped in S2). Still **not captured**: the `status` of `channel@1`, the
`data-width` (no such property was listed), the DPU stream feeding the LDB (read
from source only), and the backlight enable GPIO and PWM controller. These
close the gaps in §3.1, §3.3 and §3.5 except for those items.

## 8. Item list with estimates (inferred)

Estimates are engineering guesses, in ideal working days for one engineer who
already knows Zephyr drivers, and **not measured** (no work of this kind was
timed in this repository). They exclude board time, review rounds and the SM
permission work. They are meant for sizing, not for commitment.

| Id | Item | Depends on | Estimate | Output |
|---|---|---|---|---|
| L0 | Capture the board's `panel-timing`, `data-mapping`, PHY child `reg` (read-only) | none | 0.5 d | done for the listed fields: S2, observed 2026-11-27; the open items of §7 remain |
| L1 | Read the reference manual's LDB / display-mix CSR chapters; settle the PHY channel-enable bit, the DI-versus-display index (§3.4), HS/VS polarity bits and the 18-bit truncation | RM access (**not** known to be available) | 1-2 d | a register table with tags |
| L2 | Binding YAML (`nxp,imx95-ldb` with channel child-binding, optional `nxp,imx95-dispmix-csr`) and `disabled` nodes in the M7 dtsi | none | 1 d | binding + dtsi change, host-buildable |
| L3 | LDB register layer: control word builder (mode, width, JEIDA/SPWG, polarity, split), PHY sequence, clock-gate bits; unit-tested against the Linux words for the three mappings with a mock MMIO | L1, L2 | 2 d | driver core + a host test |
| L4 | Pixel link and `dispmix_csr` `CLOCK_CTRL` / interleaver-bypass writes | L1, L2 | 0.5-1 d | register code |
| L5 | SCMI clock and power use: set `LDBPLL_VCO` / `LDBPLL` for 7 x pixel clock, enable with read-back (the silent-success case), `PD_DISPLAY` via the node's `power-domains`; fix or avoid the `set_rate` argument form | SM verdict; [imx95-m7-scmi.md](imx95-m7-scmi.md) | 1-2 d | clock/power glue |
| L6 | Chain with the DPU: `nxp,dc`, polarity from `display-timings`, init order (§5.3) | L3-L5 | 1-2 d | integration; the least certain item |
| L7 | Panel and backlight: Verdin overlay (`display-timings`, mapping, width, PWM backlight via TPM) | L0 | 1-2 d | board overlay |
| L8 | Dual-channel (split) mode: both channels, PHY divide by 2, 2x data path | L3 | 1-2 d | optional; the board uses channel 0 only, so not needed for M2 |
| L9 | Test app under `tests/board/` that enables the LDB and the DPU, reports each register read-back, builds on the host at pinned upstream Zephyr | L2-L7 | 1 d | board-test app |
| L10 | Bring-up on the board: run L9 through remoteproc, find the enable order that works, capture the dated result | the gated `m7-run2`, SM verdict, board lease | not estimated (depends on the SM verdict and board access) | an observed result or a blocker report |
| L11 | Upstreaming (binding review, style, a Verdin board in Zephyr) | L2-L9 | not estimated (review-bound) | optional |

Estimated total for L0 to L9 excluding L8: about 8-13 working days, **inferred**,
before any board time. L10 is the real risk: nothing in this note shows the M7
can reach any of these registers or clocks on the Verdin (§4, §6).

Where the code would live is also open: an out-of-tree Zephyr module in this
repository (keeps the "upstream Zephyr only for tests" rule if the module is
selected only by the board-test app), or an upstream pull request. This note
makes no choice (**NOT decided**).

## 9. Reading list for whoever implements this

| Need | Where |
|---|---|
| The Linux sequence to mirror | LX `imx95-ldb.c` (`imx95_ldb_bridge_mode_set`, `imx95_ldb_bridge_atomic_enable`, `imx95_ldb_bridge_atomic_disable`), `imx-ldb-helper.c`, `phy-fsl-imx8mp-lvds.c` (`imx8mp_lvds_phy_init`, `imx8mp_lvds_phy_power_on`) |
| NXP's non-Linux version | MCUX `fsl_ldb.c` (`LDB_Init`), with the 100 ms delay and `|=` caveats of §3.1 |
| Clock gates and muxes | LX `imx95-blk-ctrl.c` (`lvds_clk_dev_data`, `dispmix_csr_clk_dev_data`) |
| The DSI precedent for chaining | `dsi_nxp_dwc.c` and the `nxp,dc` property ([imx95-m7-dpu-driver.md](imx95-m7-dpu-driver.md) §7) |
| Who may do what | [imx95-sm-display-lm.md](imx95-sm-display-lm.md), [imx95-display-clocks.md](imx95-display-clocks.md) §4, [imx95-display-power.md](imx95-display-power.md) |
