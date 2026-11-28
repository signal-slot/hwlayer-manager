# i.MX95 display route from DPU to the LVDS panel: source study for an M7 driver

Desk research, 2026-11-28, corrected 2026-11-28 against the two dated board snapshots under `docs/imx95-zephyr/`. Sections 1-3, 5 and 6 are source reads and inference only. Section 4 holds the board facts, tagged **observed** with the run date (2026-11-27 for both snapshots, see [observed-2026-11-27.txt](imx95-zephyr/observed-2026-11-27.txt) and [observed-2026-11-28.txt](imx95-zephyr/observed-2026-11-28.txt)). Tags: **observed** = read on the board in a named snapshot; **read from source** = read in the named file at the pinned ref; **inferred** = concluded from what was read, not stated by a source. A gap is written **NOT READ**. This note covers the route after the DPU (pixel interleaver, pixel link, LDB, LVDS PHY, display-mix CSRs, clocks, panel). The DPU core driver is out of scope (see `docs/imx95-dpu-source-6.6.md`).

## 1. Sources and pins

| Ref | URL | Pin | Notes |
|---|---|---|---|
| NXP Linux | `https://github.com/nxp-imx/linux-imx` | tag `lf-6.6.52-2.2.2`, commit `5a0a5e71d2bd` (as recorded in `docs/imx95-dpu-source-6.6.md`; this study could not re-resolve the SHA: the GitHub API answered 403) | Files read via `https://raw.githubusercontent.com/nxp-imx/linux-imx/lf-6.6.52-2.2.2/<path>` |
| Upstream Zephyr | `https://github.com/zephyrproject-rtos/zephyr` | this study read `main` unpinned: newest commit seen `2c9beecdd026` (merge of PR #99224, 2026-05-19), head SHA **not obtained** (API 403). The canonical pin of this repository is tag `v4.5.0` (commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966`, the tag `scripts/prepare-zephyr.sh` fetches; see [imx95-m7-scmi.md](imx95-m7-scmi.md)); section 5 was **not re-read** at that tag | Files via `https://raw.githubusercontent.com/zephyrproject-rtos/zephyr/main/<path>` |
| Toradex kernel / DT | `https://git.toradex.com/linux-toradex.git`, branch `toradex_6.6-2.2.x-imx` | commit `64f85d65c112c45ceb6c7e0566885afc62b1a551` (the `SRCREV_machine` of BSP 7.8.0, kernel 6.6.157, pinned in [imx95-m7-memory-map.md](imx95-m7-memory-map.md)). **This study did not read it**: `github.com/toradex/linux-toradex` and `github.com/toradex/meta-toradex-nxp` answered 404 and cgit is behind a bot challenge (2026-11-28). The Verdin and Mallow DT files at that commit are read by [imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md) | No Verdin iMX95 or Mallow file was read here |
| Toradex overlays | `https://git.toradex.com/device-tree-overlays.git`, branch `toradex_6.6-2.2.x-imx` | commit `e9a3ca6fbd6bd5fab7f00cdb87360be44373cf69` | Read by [imx95-verdin-lvds-panel.md](imx95-verdin-lvds-panel.md) (section 1 there); the panel facts of section 4 come from there and from the snapshots |

The board kernel is NXP 6.6.157-7.8.0 (Toradex). That it shares the `lf-6.6.52-2.2.x` drivers below is **inferred** (see the same caveat in `docs/imx95-dpu-source-6.6.md`). The Toradex deltas to these files are not read.

## 2. The route (read from source)

`imx95.dtsi` at the pin, DT graph: DPU `port@0/1` -> `pixel_interleaver` channel@0/1 -> `display_pixel_link` `port@0/1`; pixel-link `port@2` endpoint@1 -> `ldb` channel@0, `port@3` endpoint@1 -> channel@1 (endpoint@0 of those ports goes to MIPI DSI); `ldb` channel@N `phys` = `ldbN_phy1` (`phy-names = "lvds_phy"`), then the panel on channel `port@1`.

Blocks and DT base addresses (`arch/arm64/boot/dts/freescale/imx95.dtsi`):

| Block | Node | Compatible | Base | Status in `imx95.dtsi` |
|---|---|---|---|---|
| DPU | `dpu: display-controller@4b400000` | `nxp,imx95-dpu` | 0x4b400000, 0x400000 | disabled |
| Display-mix block control | `dispmix_csr: syscon@4b010000` | `fsl,imx95-dispmix-csr` | 0x4b010000, 0x10000 | (no status = enabled) |
| Pixel link | child `display_pixel_link: bridge@8` of `dispmix_csr` | `nxp,imx95-dc-pixel-link` | dispmix_csr + 0x8 | disabled |
| Pixel interleaver | `bridge@4b0d0000` | `nxp,imx95-pixel-interleaver` | 0x4b0d0000, 0x20000 | disabled |
| LVDS CSR | `lvds_csr: syscon@4b0c0000` | `fsl,imx95-dispmix-lvds-csr` | 0x4b0c0000, 0x10000 | (no status = enabled) |
| LDB | child `ldb: ldb@4` of `lvds_csr` | `fsl,imx95-ldb` | lvds_csr + 0x4 | disabled (channels disabled) |
| LVDS PHY 0 / 1 | children `phy@8` / `phy@c` of `lvds_csr` | `fsl,imx95-lvds0-phy` / `lvds1-phy` | lvds_csr + 0x8 / + 0xc | disabled |
| Display irqsteer | `interrupt-controller@4b0b0000` | `fsl,imx-irqsteer` | 0x4b0b0000, 0x1000 | disabled |

Power domain of all of these is `IMX95_PD_DISPLAY` via `scmi_devpd` (the DPU, dispmix_csr, lvds_csr, PHYs, irqsteer); the pixel interleaver node has no `power-domains` although its binding requires one. Which of these the **board** DT enables is **NOT READ** (needs the Toradex tree).

A panel route therefore needs `ldb` plus its channel, the PHY, the pixel link, the interleaver and the DPU enabled, since the `imx95.dtsi` defaults are disabled. That this set is exactly what an LVDS output needs is **inferred** from the graph above.

## 3. What Linux does to light LVDS (read from source)

Files: `drivers/gpu/drm/bridge/imx/imx95-ldb.c`, `imx-ldb-helper.{c,h}`, `imx95-pixel-link.c`, `imx95-pixel-interleaver.c`; `drivers/phy/freescale/phy-fsl-imx8mp-lvds.c`; `drivers/clk/imx/imx95-blk-ctrl.c`; `drivers/gpu/drm/imx/dpu95/dpu95-{framegen,core,crtc}.c`. All at the NXP pin. There is no LDB or pixel-link code in `dpu95/`.

### 3.1 Registers (read from source)

All offsets are from the DT base of the named syscon.

**lvds_csr (0x4b0c0000)**

| Off | Register | Fields written |
|---|---|---|
| 0x0 | `LVDS_PHY_CLK_CTRL` / clock gates | bit0 `ldb_phy_div` (divide by 1 or 2; the LDB driver also sets bit0 as `LVDS_PHY_DIV2` in split mode); bit1 `lvds_ch0_gate`; bit2 `lvds_ch1_gate`; bit3 `lvds_di0_gate`; bit4 `lvds_di1_gate`. Gates use `CLK_GATE_SET_TO_DISABLE` (a set bit gates the clock off) |
| 0x4 | `LDB_CTRL` | [1:0] CH0 mode (1 = to DI0, 3 = to DI1); [3:2] CH1 mode (same encoding shifted); 4 split mode; 5 CH0 24-bit; 6 CH0 JEIDA map; 7 CH1 24-bit; 8 CH1 JEIDA; 9 DI0 VS active-low; 10 DI1 VS active-low; 13 DI0 HS active-low; 14 DI1 HS active-low; 19 `VSYNC_ADJ_EN` |
| 0x8 | LVDS0 PHY control | 1 `DISABLE_LVDS`; 2 `BG_EN`; 3 `HS_EN` (defined, never set); 4 `PRE_EMPH_EN`; [7:5] `PRE_EMPH_ADJ`; [13:11] `CC_ADJ`; bit `id` `CH_EN` |
| 0xc | LVDS1 PHY control | same layout |

18-bit output sets neither width nor JEIDA bit (`RGB666_1X7X3_SPWG`); 24-bit SPWG sets the width bit; 24-bit JEIDA sets width and JEIDA bits. Sync polarity bits follow the mode flags (NVSYNC/NHSYNC set the active-low bit); `VSYNC_ADJ_EN` is always set.

**dispmix_csr (0x4b010000)**

| Off | Register | Use |
|---|---|---|
| 0x0 | `CLOCK_CTRL` (also the `disp_engine0/1_sel` clock muxes, bits [1:0] and [3:2]) | per frame generator `n`: value 0 = CCM, 1 = DSI PLL, 2 = LDB PLL /7 (`LVDS_PLL_7`); the DPU writes 2 for an LVDS encoder |
| 0x4 | `PIXEL_INTERLEAVER_CTRL` | 0 = bypass; bit0 `MODE` = stream-0 split-2; `MODE`+bit1 `DISP_IN_SEL` = stream-1 split-2 |
| 0x8 | pixel link `CTRL` | per link `n`: `PL_ENABLE(n)` = bit 4n, `PL_VALID(n)` = bit 4n+1 |
| 0x1c | `QOS_SETTING` | DPU writes panic/ARQOS = 3 (mask 0x70 / 0x7) |

**Pixel interleaver own block (0x4b0d0000), stream n at stride 0x10000**: `CTRL` 0x0 (bit 9 HSYNC pol, bit 10 VSYNC pol, bit 11 DE pol, bit 12 pixel format, bit 13 YUV conv), `SWRST` 0x20 (bit1), `IE` 0x30, `IS` 0x40 (`FOVF` bit n), `ICTRL` 0x50 (width [11:0]). Interrupt: line 62 on the display irqsteer.

### 3.2 Clocks (read from source)

- SCMI clock IDs (`include/dt-bindings/clock/fsl,imx95-clock.h`): `IMX95_CLK_LDBPLL_VCO` 37, `IMX95_CLK_LDBPLL` 38, `DISPAPB` 75, `DISPAXI` 76, `DISPOCRAM` 78, `DISP1PIX` 80, `CAMAPB` 62. Zephyr's `imx95_clock.h` carries the same names (section 5).
- `dispmix_csr` assigns `DISPAXI` and `DISPOCRAM` = 400 MHz (parent `SYSPLL1_PFD1`) and `DISPAPB` = 133.333 MHz (parent `SYSPLL1_PFD1_DIV2`) through `assigned-clocks` (read from source, NXP pin). On the board `dispocram` is 400 MHz and `dispapb` 133.333 MHz but **`dispaxi` is 800 MHz**, not 400 (**observed** 2026-11-27, `clk_summary` in [observed-2026-11-27.txt](imx95-zephyr/observed-2026-11-27.txt)); why differs is **NOT READ** (the Toradex tree or the SM may set it). Do not use 400 MHz for `DISPAXI` as a board fact.
- `ldb_pll_div7` is a DT `fixed-factor-clock` (1/7) of `IMX95_CLK_LDBPLL`. It is the parent of `lvds_di0/1_gate`; `ldb_phy_div` is a 1-bit power-of-two divider of `ldbpll` and parents `lvds_ch0/1_gate`. All four gates set `CLK_SET_RATE_PARENT`.
- The LDB driver (`imx95_ldb_bridge_mode_set`) does `clk_set_rate(ldb_di<N>, mode_clock*1000)`. Propagated up, this makes LDBPLL = 7 x pixel clock (the PLL is an SCMI-owned clock). `ldb_vco` is only enabled, never rate-set, in the files read.
- `mode_valid`: clock > 330 MHz rejected; a single channel rejects > 165 MHz. `DPU95_FRAMEGEN_MAX_CLOCK` is **NOT READ**.
- The PHY driver uses the `apb` clock (`DISPAPB`) around each register access. The pixel interleaver and the pixel link use `DISPAPB` too.
- The DPU enables `ldb` (`ldb_pll_div7`) then `ldb_vco`, in that order read as `clk_prepare_enable(clk_ldb_vco)` then `clk_prepare_enable(clk_ldb)`; disable reverses.

### 3.3 Bring-up order for one LVDS channel (read from source; the sequence is the Linux one, not a hardware requirement)

`mode_set`: resume the device (runtime-resume writes `LDB_CTRL` = 0), `phy_init` (writes PHY control = `CC_ADJ(2) | PRE_EMPH_EN | PRE_EMPH_ADJ(3)`), set the DI clock rate, build `ldb_ctrl` shadow (VSYNC_ADJ_EN, polarity, width/JEIDA, split).
`atomic_enable`: enable the ch and di gates; set CH mode (to DI0 for channel 0; to DI1 for channel 1 when not split); `phy_power_on` (set `BG_EN`, clear `DISABLE_LVDS`, wait 15-20 us if the bandgap was off, then set `CH_EN`, wait 5-10 us); last, write the whole `LDB_CTRL` shadow.
Elsewhere in the chain: the pixel interleaver (reset, polarity, optional split) and the pixel link (`VALID`, then `ENABLE`; disable reverses) are enabled by the DRM bridge chain; the DPU frame generator sets `CLOCK_CTRL` and enables the clocks.
Dual-link: split mode sets `LVDS_PHY_DIV2`, powers both PHYs; the even/odd pixel order is rejected.

### 3.4 Not touched by the LVDS route (read from source)

`display_master_csr` (0x4ad10000, with its `Pixel_link_sel` mux at +0x4) and `display_stream_csr` serve the DSI path in the DT (`mipi_dsi` references them). The LVDS driver code read does not use them. Whether the hardware needs them idle for LVDS is **inferred** as no, and untested. `imx95-mipi-dsi.c` was not read.

## 4. Toradex Verdin iMX95 / 10.1" panel on the board

What is **observed** comes from the two snapshots (both run 2026-11-27, board over ssh, read-only). The overlay files are **read from source** in [imx95-verdin-lvds-panel.md](imx95-verdin-lvds-panel.md).

| Fact | Tag | Where |
|---|---|---|
| One LDB channel in use: `ldb@4` (`fsl,imx95-ldb`) under `syscon@4b0c0000`, `channel@0` `okay`, `phys` the PHY at `phy@8`, `phy-names` `lvds_phy`; single link. `phy@8` `reg` = `<0x8 0x4>`, `status` `okay` | observed 2026-11-27 | S2 |
| `channel@1`: its `status` was **never dumped** in either snapshot. Only its clock gates (`lvds_ch1_gate`, `lvds_di1_gate`) being off is observed. "Channel 1 disabled" is therefore **not observed** | observed 2026-11-27 (gates), NOT READ (status) | S1 `clk_summary`, S2 |
| Root node `panel-lvds-native` (`panel-lvds`), `data-mapping` `vesa-24`, 217 x 136 mm; `panel-timing`: 1280 x 800, clock 68.9 / 71.1 / 73.4 MHz (min/typ/max), hfront/hback porch 23/60/71, hsync 15/40/47, vfront/vback 5/7/10, vsync 6/9/12, hsync and vsync active 0, de-active 1, pixelclk-active 1 | observed 2026-11-27 | S2 (hex decoded in its header) |
| These values equal the original native overlay `verdin-imx95_panel-cap-touch-10inch-lvds_overlay.dts` and differ from the V2 overlay (64.5 MHz, `pixelclk-active` 0) | observed (board values) and read from source (overlay) | S2 and the panel note |
| `ldb@4` `assigned-clock-rates` 2 986 200 000 and 497 700 000 Hz; `clk_summary`: `ldbpll_vco` 2 986.2 MHz, `ldbpll` 497.7 MHz, `ldb_pll_div7` 71.1 MHz, `lvds_ch0_gate` and `lvds_di0_gate` on | observed 2026-11-27 | S1, S2 |
| Backlight `backlight-lvds-native` (`pwm-backlight`): PWM period 6 666 667 ns, polarity flag 1; brightness levels 0 45 63 88 119 158 203 255 (decoded from hex), default level 4. The enable GPIO and the PWM controller phandle were not decoded | observed 2026-11-27 | S2 |
| Machine string `Toradex Verdin iMX95 WB on Verdin Development Board`, SoC revision 2.0 | observed 2026-11-27 | S2 |
| **Carrier is a Mallow** | **not observed**: the board's own model string names the Verdin Development Board, not Mallow. The Mallow naming comes from the plan's description of the hardware. The Toradex Mallow DTS names the model "... on Mallow Board" ([imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md) section 3.1), a different string, so the board's DT is not that file. The board's model string equals `model` in `arch/arm64/boot/dts/freescale/imx95-verdin-wifi-dev.dts` (blob `83fb2c3d6dbd7ceb26f17c08baaba65f20bb0f9e`) and `imx95-verdin-dev.dtsi` (blob `13efa69ebf59b1ad3149b96f4d5754a675feab44`) at commit `64f85d65c112c45ceb6c7e0566885afc62b1a551` (**read from source**, 2026-11-28), which enables no display node and has `&tpm5` ("Verdin PWM_2") okay | see panel note section 2 |
| The DPU stream that feeds the LDB channel (stream 0 or 1) | read from source only (the `imx95.dtsi` graph in section 2); never observed | - |

S1 = [observed-2026-11-27.txt](imx95-zephyr/observed-2026-11-27.txt), S2 = [observed-2026-11-28.txt](imx95-zephyr/observed-2026-11-28.txt). Still **NOT READ**: the board-level `status` of the DPU, pixel link and interleaver nodes beyond what the panel note lists from the overlays, and the reserved memory and Cortex-M7 nodes (other notes).

## 5. Upstream Zephyr (read from source, pin in section 1)

- `dts/arm/nxp/imx/nxp_imx95_m7.dtsi`: has `disp_mix` (`arm,scmi-power-domain`, reg 0xd), `mipi_dsi@4acf0000` (`nxp,mipi-dsi-dwc`), `disp_irqsteer@4b0b0000` (`nxp,irqsteer-intc`, `nxp,irq-offset = <874>`, 512 irqs, children `disp_irqsteer_master0..4,7` on NVIC 214..219), `dpu@4b400000` (`socionext,dpu`, NVIC 215), SCMI clock (`protocol@14`), power (`protocol@11`), pinctrl (`protocol@19`) nodes, TPM1-6 (`nxp,kinetis-tpm`, PWM) and GPIO. All display nodes are `disabled`. Also: the `mipi_dsi` clock is a bare `<&scmi_clk 0>`; the `dpu` node has no `clocks` or `power-domains`.
- **Absent**: any LDB, LVDS CSR, LVDS PHY, pixel interleaver, pixel link, display-mix (`dispmix_csr`) or display-master CSR node. A search of this file and the bindings listing found none.
- `dts/bindings/display/`: `socionext,dpu.yaml` ("for DSI panels"), `nxp,dcif.yaml` and the other NXP LCD controllers; none for LDB/LVDS/pixel link. The listing was truncated at about 100 entries, so later names are **NOT READ**.
- `drivers/display/display_socionext_dpu.c`: a hard-coded DPU base (`0x4B780000`), timings from a `display-timings` child; it mentions no LVDS, LDB, pixel link, clock, power domain or backlight. Panel drivers present are DSI or SPI ICs plus DSI bridges; no LVDS panel or `panel-lvds`/`simple-panel` driver was seen (the `.c` listing was truncated, so this is not exhaustive).
- Clocks: `include/zephyr/dt-bindings/clock/imx95_clock.h` defines `IMX95_CLK_LDBPLL_VCO` (37), `IMX95_CLK_LDBPLL` (38), `DISPAPB/DISPAXI/DISPOCRAM`, `DISP1PIX..DISP3PIX` and the MIPI PHY clocks. It has **no** LDB gate or CSR-provided clock (the LVDS CSR gates are not SCMI clocks; see 3.2).
- SCMI: `drivers/firmware/scmi/` has clock, power, pinctrl, reset and NXP CPU protocol files. Whether `clock_control` can set the LDB PLL rate through SCMI was **NOT READ** (see `docs/imx95-m7-scmi.md` for that item).
- Boards: `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts` only aliases `&dpu` and `&mipi_dsi`; it does not enable them and has no LVDS, backlight or panel. No Verdin or Toradex board was seen under `boards/nxp`; `boards/toradex/` was **NOT READ**.
- PR #99224 ("Support DPU on i.MX95", merged 2026-05-19) is DSI-only. A search for `lvds OR ldb` pull requests found no i.MX95 LVDS work (0 open); the issue search was inconclusive.

Summary (read from source): upstream Zephyr has the DPU, the display irqsteer, the display power domain and the LDB PLL clock IDs, but nothing for the LVDS output stage.

## 6. What an M7 driver would have to own (inferred)

Derived from sections 3 and 5; none of it was run.

**Registers** (all in the display-mix; the M7 would need them mapped, and each access depends on the System Manager permission model, see `docs/imx95-m7-scmi.md` and the SM item):
1. `lvds_csr` 0x4b0c0000: +0x0 clock gates and PHY divider, +0x4 `LDB_CTRL`, +0x8 (and +0xc for channel 1) PHY control.
2. `dispmix_csr` 0x4b010000: +0x0 engine clock source (LDB PLL /7 = 2), +0x4 interleaver mode, +0x8 pixel-link valid/enable, +0x1c QoS.
3. Pixel interleaver 0x4b0d0000: reset, polarity, interrupt enable (needed only if the interleaver is not bypassed; the Linux default with no property is bypass).
4. Display irqsteer 0x4b0b0000, for the interleaver and DPU interrupts (the interrupt path itself is another item).

**Clocks and power** (SCMI-owned, by inference from the Linux DT using `scmi_clk`/`scmi_devpd`): `IMX95_PD_DISPLAY`; `DISPAPB`, `DISPAXI`, `DISPOCRAM`; `LDBPLL_VCO` and `LDBPLL` at 7 x pixel clock (71.1 MHz on this board, 497.7 MHz PLL, observed 2026-11-27, section 4); the CSR-local gates (lvds_csr +0x0) are plain register bits, not SCMI clocks, so an M7 driver would clear them itself.

**Panel side** (sized by the 2026-11-27 observation, section 4): `vesa-24` mapping, sync polarity active low, backlight PWM (upstream has TPM PWM) and enable GPIO, touch not needed.

**Gaps in upstream Zephyr to fill for this route**: an LDB/PHY/pixel-link/interleaver driver or a minimal register-poking board layer; display-mix DT nodes and bindings; an LVDS panel or a fixed-timing description (the DPU driver already takes `display-timings`); a Verdin board or overlay; an LDB PLL rate path (clock IDs exist; setting the rate was not checked).

**Open risks to check before any M7 claim** (inferred):
- The Linux LDB code writes PHY and `LDB_CTRL` with full-register writes; the PHY and the clock divider share `lvds_csr` +0x0 bit0 semantics, so two owners (Linux and the M7) cannot both write them safely.
- Whether the M7 may reach `lvds_csr`, `dispmix_csr` and the display clocks at all, and whether the display power domain can be shared with Linux, is a System Manager question not answered here.
- With Linux using the same display route and the same panel, the route is a single instance: M7 and Linux alternate ownership, they do not run it concurrently (inferred from one LDB channel set and one DPU).
