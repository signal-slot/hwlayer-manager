# Verdin iMX95 / Mallow 10.1" LVDS panel: source study (blocked, values NOT READ)

Desk research, 2026-11-28 (open issue M2-10). **Nothing was checked on the board.** No board was accessed for this note.

Tags: **read from source** = read in the named file at the pinned commit; **inferred** = concluded, not stated by a source; **secondhand** = taken from another note in this repository, not re-read here. A value that could not be read is written **NOT READ**. No panel value below is invented.

## 1. Result

**The overlay, the panel part number and the timings were not obtained.** The Toradex overlay source could not be read from this machine (section 3). Every value requested by M2-10 is therefore **NOT READ**. The table in section 2 keeps the slots so that whoever reads the overlay can fill them in with a pin.

## 2. Values requested (all NOT READ)

| Item | Value | Tag |
|---|---|---|
| Overlay file name (Verdin iMX95 + Mallow, 10.1" LVDS) | NOT READ | none |
| Files the overlay includes | NOT READ | none |
| Panel compatible / part number | NOT READ | none |
| `clock-frequency` (pixel clock) | NOT READ | none |
| `hactive`, `vactive` | NOT READ | none |
| `hfront-porch`, `hback-porch`, `hsync-len` | NOT READ | none |
| `vfront-porch`, `vback-porch`, `vsync-len` | NOT READ | none |
| `hsync-active`, `vsync-active`, `de-active`, `pixelclk-active` | NOT READ | none |
| `data-mapping` (JEIDA/SPWG), `data-width`, bus format | NOT READ | none |
| Single or dual channel | NOT READ | none |
| LDB / pixel-link / PHY / interleaver / DPU nodes enabled | NOT READ | none |
| Backlight (`pwm-backlight`, PWM instance, enable GPIO) | NOT READ | none |

Second-hand hint only (not a value to rely on): `docs/imx95-verdin-zephyr-board.md` section "Facts about the display" records "one LDB channel, single link, 1280x800 at about 71.1 MHz" as the shape of the 10.1" overlay **in NXP's tree**, and tags that this is the Verdin panel as *inferred*. **secondhand**; it is not Toradex's overlay and was not re-read here. `docs/imx95-m7-lvds-route.md` section 4 states the Verdin side was NOT READ, and `docs/imx95-verdin-bsp-src.md` records, **read from source**, that the Verdin and Mallow dtsi/dts files enable no `&dpu`, `&ldb`, panel, backlight or display node, so the panel comes from an overlay or other file not yet read.

## 3. Sources tried (2026-11-28)

| Source | Result |
|---|---|
| `https://git.toradex.com/device-tree-overlays.git`, `git ls-remote` | **read from source** (ref list only): branch `toradex_6.6-2.2.x-imx` exists at `e9a3ca6fbd6bd5fab7f00cdb87360be44373cf69`; `toradex_6.6-2.1.x-imx` at `abd3972097c6ec1d4be9dd7445b6b2bad88e45c9`; `toradex_6.6-2.0.x-imx` at `3beb3252a492dd9cffff294361b7232d1aaa8350`; `master` at `77baaf2cc7bfd8ae704b8d9e06915e6a9d205279`. File contents were **not read**: `git clone` and `git fetch` were refused by the agent's permission layer in this run, so no tree was obtained |
| `https://git.toradex.com/cgit/device-tree-overlays.git/tree/?h=toradex_6.6-2.2.x-imx` | HTTP 200 with an anti-bot block page instead of repository content |
| `https://github.com/toradex/device-tree-overlays` | Reachable. The repository is archived and deprecated; its only branch is `toradex_5.4.y` (last updated 2021-01-22), no 6.6 branch. It predates i.MX95 and was not opened |
| `https://github.com/toradex/device-trees` | Per `docs/imx95-m7-lvds-route.md` only 5.4-era branches |
| `https://github.com/nxp-imx/linux-imx`, branch `lf-6.6.y`, `arch/arm64/boot/dts/freescale/` | Directory listing truncated before the `imx95*` names; the `Makefile` returned HTTP 503. No NXP Verdin or LVDS file was read |

Which branch matches the board (Reference Multimedia Image 7.8.0, kernel 6.6.157-7.8.0) is **inferred** to be `toradex_6.6-2.2.x-imx`; the pin is not confirmed by any file read.

## 4. How to close this

1. On a machine where `git clone` is allowed, run `git clone --branch toradex_6.6-2.2.x-imx https://git.toradex.com/device-tree-overlays.git`, record `git rev-parse HEAD`, and search for Verdin iMX95 overlays (names with `imx95` and `lvds`, or the 10.1" capacitive-touch LVDS display) and the files they include.
2. Read the same pin of `linux-toradex` (`https://git.toradex.com/cgit/linux-toradex.git`, branch `toradex_6.6-2.2.x-imx`) for `imx95-verdin*.dts*` and the panel and `ldb` nodes it references, or NXP's `lf-6.6.y` if Toradex's is unreadable.
3. Fill section 2 with values tagged **read from source** and the commit in each source row.
4. Optionally, the live device tree on the board gives an **observed** cross-check (separate item `imx95-observe2`); that was not done here.

## 5. Not done

No board access, no other documents edited, no code. `docs/open-issues.md` M2-10 is unchanged: the panel part number, timings and pixel clock remain open.
