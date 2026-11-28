# Verdin iMX95 10.1" LVDS panel: overlay, timings and LDB setup (source study)

Desk research, 2026-11-28 (open issue M2-10). No board was accessed for the source reads in sections 1-5. Section 3b and the corrections of sections 2 and 6 compare them with two dated board snapshots, [observed-2026-11-27.txt](imx95-zephyr/observed-2026-11-27.txt) (S1) and [observed-2026-11-28.txt](imx95-zephyr/observed-2026-11-28.txt) (S2), both run on the board 2026-11-27 (the second file's name carries the planner's date).

Tags: **observed** = read on the board in S1 or S2, run date given; **read from source** = read in the named file at the pinned commit; **inferred** = concluded, not stated by a source; **secondhand** = taken from another note in this repository. **NOT READ** marks what was not obtained.

## 1. Source pin

| Item | Value |
|---|---|
| Repository | `https://git.toradex.com/device-tree-overlays.git` |
| Branch | `toradex_6.6-2.2.x-imx` |
| Commit read | `e9a3ca6fbd6bd5fab7f00cdb87360be44373cf69` (shallow clone, `HEAD` of the branch on 2026-11-28; the same value `git ls-remote` returned) |
| Files | all under `overlays/` of that commit |

That this branch matches the board (Reference Multimedia Image 7.8.0, kernel 6.6.157-7.8.0) is **inferred** from the 6.6 branch name; no file read ties the two together. The GitHub mirror `toradex/device-tree-overlays` is archived with a 5.4 branch only and was not used. Other branch heads (`ls-remote`, 2026-11-28): `toradex_6.6-2.1.x-imx` `abd3972097c6ec1d4be9dd7445b6b2bad88e45c9`, `toradex_6.6-2.0.x-imx` `3beb3252a492dd9cffff294361b7232d1aaa8350`.

**Toradex kernel pin.** `https://git.toradex.com/linux-toradex.git`, branch `toradex_6.6-2.2.x-imx`, commit `64f85d65c112c45ceb6c7e0566885afc62b1a551` (`SRCREV_machine` of BSP 7.8.0, 6.6.157; see [imx95-m7-memory-map.md](imx95-m7-memory-map.md)). Its `arch/arm64/boot/dts/freescale/imx95-verdin-wifi-mallow.dts` (blob `2049b2b07f538f2900f418df434cf8a65f6f8256`) and `imx95-verdin-mallow.dtsi` (blob `4ba4ab29e25965c0150dc61c7e4e1cb1c3d5a260`) were read by [imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md) (**secondhand** here). The Verdin Development Board files at that commit were read for this note on 2026-11-28 by a blobless `git fetch --depth 1` of the commit from `git://git.toradex.com/linux-toradex.git` (section 2).

**NOT READ:** the Verdin iMX95 base DTS in `linux-toradex` (the overlays reference its labels `ldb`, `ldb0_phy`, `dpu`, `pixel_interleaver`, `display_pixel_link`, `displaymix_irqsteer`, `tpm5`, `lpi2c3`, `scmi_clk`) and the NXP DTS. The base `status` of those nodes is therefore unconfirmed; `docs/imx95-verdin-bsp-src.md` records (**secondhand**) that the Verdin and Mallow dts enable no display node.

## 2. Verdin iMX95 10.1" LVDS overlays at the pin

Every file below has `compatible = "toradex,verdin-imx95"` where it sets one, and a header pointing to the Toradex "Capacitive Touch Display 10.1 inch LVDS" pages. All are **read from source**.

| Overlay file | Route | Role |
|---|---|---|
| `verdin-imx95_panel-cap-touch-10inch-lvds_overlay.dts` | native LDB, standard LVDS connector | original display revision (`panel-lvds`) |
| `verdin-imx95_panel-cap-touch-10inch-lvds-v2_overlay.dts` | native LDB, standard LVDS connector | V2 display revision (named panel) |
| `verdin-imx95_mezzanine_panel-cap-touch-10inch-lvds_overlay.dts` | native LDB, mezzanine connector | original revision, other GPIOs |
| `verdin-imx95_mezzanine_panel-cap-touch-10inch-lvds-v2_overlay.dts` | native LDB, mezzanine connector | V2 revision |
| `verdin-imx95_dsi-to-lvds_panel-cap-touch-10inch-lvds_overlay.dts` | MIPI DSI, SN65DSI84 bridge adapter | not the LDB route |
| `verdin-imx95_dsi-to-lvds-v2_panel-cap-touch-10inch-lvds_overlay.dts` | DSI to LVDS, V2 | not the LDB route (**NOT READ** beyond its name) |

No overlay file mentions Mallow. **Carrier: Mallow is not observed.** The board's `/proc/device-tree/model` reads `Toradex Verdin iMX95 WB on Verdin Development Board` (**observed** 2026-11-27, S2 `### (c) soc0`, `machine:`). The Mallow DTS at the Toradex pin carries a different model string, "Toradex Verdin iMX95 WB on Mallow Board" (**read from source**, via [imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md) section 3.1, commit `64f85d65c112c45ceb6c7e0566885afc62b1a551`, file `imx95-verdin-wifi-mallow.dts`). So the board runs a Development Board DT, not that file. **Read from source** (2026-11-28): `arch/arm64/boot/dts/freescale/imx95-verdin-wifi-dev.dts` (blob `83fb2c3d6dbd7ceb26f17c08baaba65f20bb0f9e`) and `imx95-verdin-dev.dtsi` (blob `13efa69ebf59b1ad3149b96f4d5754a675feab44`) at commit `64f85d65c112c45ceb6c7e0566885afc62b1a551`: `imx95-verdin-wifi-dev.dts` sets `model = "Toradex Verdin iMX95 WB on Verdin Development Board"`, exactly the observed string, and includes `imx95-verdin.dtsi`, `imx95-verdin-wifi.dtsi` and `imx95-verdin-dev.dtsi`. The `-dev.dtsi` has no `ldb`, `dpu`, panel or backlight node; it enables `&tpm5` (comment "Verdin PWM_2"), the PWM_2 the overlays use. That the physical carrier is a Mallow is the plan's description of the hardware, not a board or source fact; no read file ties the board to Mallow, and the Mallow files are not what the board's DT matches. Which overlay is applied is not shown by the snapshots (no overlay setting was read). Which display revision is attached is settled by the board values in section 3b, not by a file name. The "original" and "V2" labels are the file names' own, read as display revisions (**inferred**).

## 3. Panel and timings (all read from source)

Two timing sets exist. Values are `min typ max` where three numbers are given, otherwise a single value.

| Field | Original: `..._panel-cap-touch-10inch-lvds_overlay.dts` (native, lines 36-59) | V2: `..._lvds-v2_overlay.dts` (native, lines 32-53) |
|---|---|---|
| Node / compatible | `panel-lvds-native`, `panel-lvds` (generic, no part number) | `panel-lvds-native`, `optologic,scx1001511ggc49` then `panel-lvds` |
| Part number | none stated in the file | `scx1001511ggc49` (compatible string; vendor prefix `optologic`) |
| `clock-frequency` | 68 900 000 / 71 100 000 / 73 400 000 Hz | 64 500 000 Hz |
| `hactive` / `vactive` | 1280 / 800 | 1280 / 800 |
| `hfront-porch` | 23 / 60 / 71 | 20 |
| `hback-porch` | 23 / 60 / 71 | 20 |
| `hsync-len` | 15 / 40 / 47 | 2 |
| `vfront-porch` | 5 / 7 / 10 | 5 |
| `vback-porch` | 5 / 7 / 10 | 5 |
| `vsync-len` | 6 / 9 / 12 | 3 |
| `hsync-active` / `vsync-active` | 0 / 0 (active low) | 0 / 0 (active low) |
| `de-active` | 1 (high) | 1 (high) |
| `pixelclk-active` | 1 (positive edge, per the file comment) | 0 (negative edge) |
| `data-mapping` | `vesa-24` | `vesa-24` |
| `height-mm` / `width-mm` | 136 / 217 | 136 / 217 |

The mezzanine variants carry the same timing blocks as the native ones with the same values (original: identical min/typ/max set at lines 41-55; V2: 64 500 000 Hz, 20/20/2 and 5/5/3, `pixelclk-active` 0).

Derived, **inferred** arithmetic from the typ values (not stated in the files): original total 1280+60+40+60 = 1440 by 800+7+9+7 = 823 pixels, 71.1 MHz gives about 60.0 Hz; V2 total 1280+20+2+20 = 1322 by 800+5+3+5 = 813, 64.5 MHz gives about 60.0 Hz. The DT-bindings type of the compatible `scx1001511ggc49` and any datasheet were **NOT READ**.

Bus format: `data-mapping = "vesa-24"` is 24 bits per pixel, VESA/SPWG-style 8-bit-per-colour mapping (**read from source**: the string; the 8 bits per colour is the `panel-lvds` binding meaning, **inferred**, binding not read). Neither file sets `data-width`.

## 3b. The board's values against the overlays (observed 2026-11-27)

The live `panel-lvds-native` node in S2 has: clock 68 900 000 / 71 100 000 / 73 400 000 Hz, 1280 x 800, h porches 23/60/71, hsync-len 15/40/47, v porches 5/7/10, vsync-len 6/9/12, hsync and vsync active 0, de-active 1, `pixelclk-active` 1, `data-mapping` `vesa-24`, 217 x 136 mm, `compatible` `panel-lvds` only (**observed** 2026-11-27, S2; hex decoded in its header). Every one equals the **original** overlay column of section 3 and differs from V2 (64.5 MHz, `pixelclk-active` 0, compatible `optologic,scx1001511ggc49`). `ldb@4` `assigned-clock-rates` are 2 986 200 000 and 497 700 000 Hz (observed, S2), the original overlay's pair of section 4. The backlight node has PWM period 6 666 667 ns with polarity flag 1 (inverted), as the original native overlay of section 5, and brightness levels 0 45 63 88 119 158 203 255, default 4 (observed). The match to the **native** (not mezzanine) original overlay is by node names (`panel-lvds-native`, `backlight-lvds-native`) (inferred); the backlight enable GPIO was not decoded, so native versus mezzanine is not confirmed by it. That the overlay file itself is what the board applied is **inferred** from the equality; no overlay setting was read.

## 4. LDB and pixel-link setup (read from source)

Single LVDS channel (channel 0 only, single link, not dual). Both native overlays enable these nodes:

| Node | Setting |
|---|---|
| `&dpu` | `status = "okay"` |
| `&display_pixel_link` | `okay` |
| `&displaymix_irqsteer` | `okay` |
| `&pixel_interleaver` | `okay`, `channel@0` `okay` |
| `&ldb` | `okay`, `channel@0` `okay`, `port@1` endpoint `lvds0_out` (original) or `lvds_out` (V2) to the panel |
| `&ldb0_phy` | `okay` |

LDB PLL via the SCMI clock provider (`assigned-clocks` `IMX95_CLK_LDBPLL_VCO` and `IMX95_CLK_LDBPLL`), each overlay with its comment "LDBPLL_VCO = 6 * 7 * pixelclk, LDBPLL = 7 * pixelclk":

| Overlay | `assigned-clock-rates` | Pixel clock they imply (**inferred**, rate / 7) |
|---|---|---|
| original (native and mezzanine) | 2 986 200 000 and 497 700 000 Hz | 71.1 MHz |
| V2 (native and mezzanine) | 2 709 000 000 and 451 500 000 Hz | 64.5 MHz |

Graph: DPU, `pixel_interleaver` channel 0, `display_pixel_link`, `ldb` channel 0, panel (the panel node has `port@0` in the original native overlay, a single `port` in the others). The DPU and pixel-link wiring inside the base dtsi is **NOT READ** here (see `docs/imx95-m7-lvds-route.md`, **secondhand**).

## 5. Backlight, PWM and touch (read from source)

| Item | Original native | V2 native | Original / V2 mezzanine |
|---|---|---|---|
| Backlight | `pwm-backlight` | `pwm-backlight` | `pwm-backlight` |
| PWM | `&tpm5` channel 0, period 6 666 667 ns, inverted | `&tpm5` channel 0, 6 666 667 ns, normal polarity | `&tpm5` channel 0, 6 666 667 ns; inverted (original), normal (V2) |
| PWM label | Verdin PWM_2 (SODIMM 16) | Verdin PWM_2 (SODIMM 16) | Verdin PWM_2 |
| Enable GPIO | `&gpio5` 4, active high (I2S_2_D_OUT) | `&gpio5` 4, active high | `&gpio5` 12 (GPIO_4), active high |
| Brightness levels | 0 45 63 88 119 158 203 255, default level 4 | same | same |
| Touch | Atmel maXTouch at 0x4a on `&lpi2c3` | Ilitek ILI251x at 0x41 on `&lpi2c3` | as native |

`&tpm5` is enabled by the original overlays; the V2 overlay's enabling of `&tpm5` was not found by the search done (**NOT READ**, may come from the base DT). The board connector names in the comments are Verdin signal names, not addresses of this board.

## 6. What this means for M2-10 (inferred)

- The Zephyr route needs one LDB channel, single link, `vesa-24`, with the LDB PLL set to 7 x the pixel clock (71.1 MHz for the original display, 64.5 MHz for V2). The board carries the **original** set: 71.1 MHz typ, observed 2026-11-27 (section 3b); the V2 set is **not** what this board shows. An M7 driver should still take the timings from a devicetree `display-timings` node and must not hard-code one set, since another panel revision would differ.
- The timings are confirmed for this board by the live device tree of 2026-11-27 (section 3b), not by this note's source reads.
- Previously recorded "about 71.1 MHz, 1280x800" in `docs/imx95-verdin-zephyr-board.md` matches the typ value of the original overlay and the live panel node (observed 2026-11-27).

## 7. Not done

No board access for the source reads; section 3b compares them with S1/S2 and edits nothing on the board. No code. `linux-toradex` and NXP `lf-6.6.y` DTS files were not read by this note; the Verdin Development Board DTS was read afterwards (section 2). `docs/open-issues.md` M2-10 is unchanged.
