# Verdin iMX95 BSP sources: pinned reads

Date of reads: 2026-11-28. Nothing here was observed on a board.

Tags: **read from source** (content fetched and seen, pinned below),
**inferred** (my deduction), **not read** (no content obtained). Section 3 is
the Toradex kernel device-tree read; section 3.3 lists what is still unread.

## 1. Pins

| Tree | Ref | Commit | How resolved |
|---|---|---|---|
| NXP imx-sm, <https://github.com/nxp-imx/imx-sm> | tag `rel_imx_sm_2026q3` | `eec232878ab7dd460f5a8f3767d39dc7e9412efa` | `git ls-remote --tags`, 2026-11-28 (the tag exists) |
| Toradex kernel, <https://git.toradex.com/linux-toradex.git> | branch `toradex_6.6-2.2.x-imx` | `8ad2b7ff6b5666b25b1b43819c83b9ae3930b8df` | `git ls-remote --heads`, 2026-11-28 |

The Toradex SHA is the branch head on the resolution date, not the SRCREV of
BSP 7.8.0 (that recipe in `meta-toradex-nxp` was not read), so it identifies
the tree but is **inferred** to be near, not equal to, the running kernel.

## 2. SM logical-machine assignment (read from source)

Source: `configs/mx95evk.cfg` at imx-sm `rel_imx_sm_2026q3`
(`eec232878ab7dd460f5a8f3767d39dc7e9412efa`), fetched over HTTPS from
`https://raw.githubusercontent.com/nxp-imx/imx-sm/rel_imx_sm_2026q3/configs/mx95evk.cfg`
(HTTP 200). This is the NXP **EVK** config; no Verdin-specific `.cfg` was
found, so applying it to the Verdin is **inferred**. Line numbers were counted
by hand from line 1 on the fetched text; re-check with `sed -n` before citing.

| Lines | Content | Meaning |
|---|---|---|
| 115 | `LM0 name="SM", rpc=none, boot=1, did=2, safe=feenv` | SM's own machine |
| 343 | `LM1 name="M7", rpc=scmi, boot=2, skip=1, did=4, safe=seenv` | M7 logical machine |
| 443 | `LM2 name="AP", rpc=scmi, boot=3, skip=1, did=3, default` | A55 machine |
| 168-170, 236 | `CLK_DISPAPB`, `CLK_DISPAXI`, `CLK_DISPOCRAM` `ALL`; `GPV_DISPLAY OWNER` | in LM0 (SM) |
| 584, 596, 608 | `CLOCK_DISP1PIX ALL`, `LDB_PLL ALL`, `BLK_CTRL_DISPLAYMIX OWNER` | in LM2, A55 non-secure |
| 625-641 | `DC OWNER, test`, `DC0`, `DC1`, `DC_2DBLIT` ... `DC_YUV3` `OWNER` | in LM2: the display controller and its sub-blocks |
| 798, 801, 802 | `LVDS OWNER`, `MIPI_DSI OWNER`, `MIPI_PHY OWNER` | in LM2 |
| 343-437 (LM1) | no DPU, LVDS, DSI, DISPLAY or PIXEL entry; owns `LPUART3` (l. 407), `PIN_GPIO_IO14/15` (422-423), `M7MIX` data windows (427-428), `DDR` exec `0x080000000-0x089FFFFFF` (429) | M7 owns no display resource |

Reading: in the EVK config the whole display pipeline belongs to the A55
machine and the M7 has none of it. A Zephyr DPU driver on the M7 would need an
SM config that moves those resources (**inferred**; follows from the table, no
source states it). This agrees with
[imx95-sm-display-lm.md](imx95-sm-display-lm.md) and
[imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md).

## 3. Toradex kernel device trees (read from source)

Tree: `linux-toradex` branch `toradex_6.6-2.2.x-imx` at `8ad2b7ff` (section 1),
read 2026-11-28 from a blobless `git clone --depth 1` of
`https://git.toradex.com/linux-toradex.git` plus `git show HEAD:<path>`.
Paths are under `arch/arm64/boot/dts/freescale/`; line numbers are from the
file as shown by `git show`.

### 3.1 Verdin and Mallow files

| File:line | Content | Tag |
|---|---|---|
| `imx95-verdin-wifi-mallow.dts` | includes `imx95-verdin.dtsi`, `imx95-verdin-wifi.dtsi`, `imx95-verdin-mallow.dtsi`; model "Toradex Verdin iMX95 WB on Mallow Board" | read from source |
| `imx95-verdin.dtsi:44-53` | `cm7: remoteproc-cm7`, `compatible = "fsl,imx95-cm7"`, `power-domains = <&scmi_devpd IMX95_PD_M7>`, mailboxes `&mu7 0 1`, `&mu7 1 1`, `&mu7 3 1` (tx, rx, rxdb), `memory-region` = vdevbuffer, vdev0vring0/1, vdev1vring0/1, rsc_table, m7_reserved, `fsl,startup-delay-ms = <50>` | read from source |
| `imx95-verdin.dtsi` (reserved-memory block) | `m7_reserved` `0x80000000` size `0x1000000` no-map; vdev0vring0 `0x88000000`, vdev0vring1 `0x88008000`, vdev1vring0 `0x88010000`, vdev1vring1 `0x88018000` (each `0x8000`); vdevbuffer `0x88020000` size `0x100000`; rsc_table `0x88220000` size `0x1000`; `linux_cma` size `0x3c000000` in `0x80000000`..`0xFF000000` | read from source |
| `imx95-verdin.dtsi` (`&mu7`) | `status = "okay"` | read from source |
| `imx95-verdin.dtsi` chosen | `stdout-path = "serial2:115200n8"`; aliases serial0..4 = lpuart7, lpuart8, lpuart1, lpuart2, lpuart6 | read from source |
| `imx95-verdin-mallow.dtsi` | enables lpuart1 (Linux console), lpuart2, lpuart7, lpuart8, `lpi2c3` ("I2C_2_DSI"), `tpm6` ("PWM_3_DSI") | read from source |
| the five files above | no `&dpu`, `&ldb`, `&mipi_dsi`, panel, backlight, display or LVDS node is enabled or defined in the Verdin or Mallow dtsi/dts. Panel support, if any, comes from a separate overlay not in these files | read from source |
| `lpuart3` | not enabled in the Verdin or Mallow files read; free of a Linux user there | read from source (absence); that it is therefore the M7 console is **inferred** |

### 3.2 SoC dtsi `imx95.dtsi` (display pipeline Linux would claim)

| Line | Content | Tag |
|---|---|---|
| 2647-2768 | `dpu: display-controller@4b400000`, `compatible = "nxp,imx95-dpu"`, `reg = <0x0 0x4b400000 0x0 0x400000>`, interrupts routed through `displaymix_irqsteer`, `power-domains = <&scmi_devpd IMX95_PD_DISPLAY>`, clocks DISP1PIX, DISPAPB, DISPAXI, DISPOCRAM, ldb, ldb_vco, `status = "disabled"` | read from source |
| 2653-2758 (names) | interrupt names include `extdst0_shdload`, `extdst0_framecomplete`, `framegen0_int0..3`, `framegen0_primsync_on/off`, `framegen0_underrun0_on/off` | read from source |
| 2469-2488 | `displaymix_irqsteer@4b0b0000`, `fsl,imx-irqsteer`, parent GIC SPI 214-219 (plus two reserved 303), 512 irqs, `status = "disabled"` | read from source |
| 2393-2407 | `dispmix_csr@4b010000`; assigned clocks DISPAXI/DISPOCRAM 400 MHz, DISPAPB 133.333 MHz | read from source |
| 2409-2412 | `display_pixel_link` (`nxp,imx95-dc-pixel-link`), `status = "disabled"`; endpoints to pixel interleavers, `mipi_dsi`, `ldb` channels | read from source |
| 2490-2509 | `lvds_csr@4b0c0000`, `ldb@4` `fsl,imx95-ldb`, `status = "disabled"` | read from source |
| 2183-2203 | `mipi_dsi@4acf0000`, `nxp,imx95-mipi-dsi`, uses `IMX95_PD_CAMERA` power domain | read from source |

So at this branch head Linux's DPU, pixel link, LDB and DSI nodes are all
`disabled` in the SoC dtsi and nothing in the Verdin/Mallow files read turns
them on. Whether the running 7.8.0 image enables them (for example by an
overlay or a different tree) is **not** shown here; the live DT of the board
is the question for [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md).
Which core would take the DPU interrupts (an NVIC mapping for the
`displaymix_irqsteer` outputs) is **inferred**, not read.

### 3.3 Fetch record and what is still unread (2026-11-28)

| URL or command | Outcome |
|---|---|
| `git ls-remote --heads https://git.toradex.com/linux-toradex.git` | read: branch list; source of the SHA in section 1 |
| `https://git.toradex.com/cgit/linux-toradex.git/tree/...` through a web fetcher | HTTP 200 but an anti-bot ("Anubis") page, no listing; not used |
| `git clone --filter=blob:none --depth 1 --no-checkout --branch toradex_6.6-2.2.x-imx https://git.toradex.com/linux-toradex.git` | first attempt refused by the tool sandbox before running; second attempt (after approval) succeeded; HEAD = `8ad2b7ff` |
| `git ls-remote https://github.com/toradex/linux-toradex.git` | "Repository not found" (a GitHub copy under that name is **inferred** absent; a rename or privacy change would look the same) |
| `raw.githubusercontent.com/torvalds/linux/v6.18/.../imx95-verdin.dtsi`, `imx95-toradex-smarc.dtsi` | HTTP 404 at that tag; not used |

Still unread: the `meta-toradex-nxp` recipe and SRCREV for BSP 7.8.0; any
Toradex-shipped SM config for the Verdin; imx-mkimage; the display overlay for
the 10.1" LVDS panel (it is not in the files above).

## 4. What this adds to the earlier studies

- Pinned imx-sm tag and Toradex branch SHAs (section 1).
- Line-cited LM ownership for display resources at `rel_imx_sm_2026q3`
  (section 2).
- Line-cited Verdin cm7 rproc, reserved-memory, console and display-node
  state from the Toradex tree (section 3). The Verdin facts in [imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md) stay
  sourced from Toradex-authored upstream patches and forum posts.
