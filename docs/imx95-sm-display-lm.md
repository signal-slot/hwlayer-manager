# i.MX95: a System Manager configuration that gives the display to the M7

Date of this note: 2026-11-28. Scope: SoW §5.1 / §9 M2 (WP3). This is a
design note. Nothing here was built, flashed or run, and no board was
accessed. It does not repeat the full ownership survey of
`docs/imx95-zephyr-feasibility.md`; it only cites the stock assignment that
the design changes.

Tags used in the tables: **read from source (repo@commit)** or **inferred**.

## 1. Sources and their limits

| Source | Pin | Tag |
|---|---|---|
| NXP meta-imx, recipe `meta-imx-bsp/recipes-bsp/imx-system-manager/imx-system-manager_2025q3.bb`, branch `scarthgap-6.6.52-2.2.2` (<https://github.com/nxp-imx/meta-imx/tree/scarthgap-6.6.52-2.2.2>); sets `SRCREV` for imx-sm and builds with `make cfg` (`CONFIG=${SYSTEM_MANAGER_CONFIG}`, `mx95evk` by default) | meta-imx `rel_imx_6.6.52_2.2.2` = `e1ec1a20d573e3913f4ad90fa36546ed2bc87715` | read from source (meta-imx@e1ec1a20) |
| NXP imx-sm (<https://github.com/nxp-imx/imx-sm>) | `af1e37026c6ba19cdca98fd6e91494efd3e7b5ec` (`master`, untagged, the `SRCREV` of that recipe) | read from source (imx-sm@af1e3702) |
| Toradex fork of imx-sm | `https://github.com/toradex/imx-sm` and `https://github.com/toradex/meta-toradex-nxp` both returned HTTP 404; the org's visible repos had no imx-sm fork. Toradex's BSP layers live on git.toradex.com, which was **not** read. | not found |

Caveats, stated plainly:

- **The pin is the nearest NXP release, not the proven pin for the board's
  image.** NXP publishes no `lf-6.6.157-*` release; the board runs Toradex
  Reference Multimedia 7.8.0 (kernel 6.6.157-7.8.0), whose imx-sm `SRCREV`
  and Verdin SM configuration were not read. Toradex may ship a different
  commit or a Verdin-specific `.cfg`. Every row below is therefore about
  **NXP's `mx95evk` configuration**, which is the stock reference, and not
  about what runs on the Verdin. Whether the Verdin's SM matches it is
  **inferred only** until the Toradex layer is read (item imx95-src owns
  that survey).
- **Line numbers are exact as of 2026-11-28** and were taken from a
  tarball of imx-sm@af1e3702 (`curl -sSL
  https://codeload.github.com/nxp-imx/imx-sm/tar.gz/af1e37026c6ba19cdca98fd6e91494efd3e7b5ec`,
  extracted into an empty directory; read from source, imx-sm@af1e3702). The
  commit is on `master` and untagged, so the commit is the pin; no tag was
  found to cite. An earlier version of this note had only approximate line
  numbers (the first attempt could not clone or fetch); they were corrected
  in the pinning pass of 2026-11-28, which also fixed the points marked
  "corrected" below.
- imx-mkimage was **not** read (the tag URL `rel_imx_6.6.52_2.2.2` returned
  404 for `iMX95/soc.mak`); §6 relies only on imx-sm's README.

## 2. Stock assignment that the design changes

All in `configs/mx95evk.cfg` unless noted (read from source, imx-sm@af1e3702).

| Item | Stock owner | Line (imx-sm@af1e3702) |
|---|---|---|
| LM0 `SM` (rpc=none, did=2), LM1 `M7` (rpc=scmi, boot=2, did=4), LM2 `AP` (boot=3, did=3, default); `SCMI_AGENT0 "M7"`, `SCMI_AGENT1 "AP-S"`, `SCMI_AGENT2 "AP-NS"` | 106, 330, 430; 358, 483, 552 |
| `CLK_DISPAPB`, `CLK_DISPAXI`, `CLK_DISPOCRAM` | LM0 `SM`, `ALL` | 159–161 |
| `GPV_DISPLAY` | LM0 `SM`, `OWNER` | 227 |
| `CLOCK_DISP1PIX`, `CLOCK_DISP2PIX`, `CLOCK_DISP3PIX`, `LDB_PLL` (SCMI clock API entries) | AP-NS agent (`SCMI_AGENT2 "AP-NS"`), `ALL` | 572–574, 586 |
| `BLK_CTRL_DISPLAYMIX`, `DC`(`, test`), `DC0`, `DC1`, `DC_2DBLIT`, `DC_BLITINT`, `DC_CMDSEQ`, `DC_DISPENG`, `DC_DISPENG_INT`, `DC_FL0`, `DC_FL1`, `DC_INT_CTL`, `DC_PIXENGINE`, `DC_XPC`, `DC_YUV0..3`, `LVDS`, `MIPI_DSI`, `MIPI_PHY`, `VIDEO_PLL1` | AP-NS agent, `OWNER` | 599, 616–632 (`DC` at 616 is `OWNER, test`), 790, 793–794, 878 |
| M7 LM resources: `M7P`, `CAN_FD1`, `IRQSTEER_M7`, `LPUART3`, `MSGINTR1/2`, `MU5_A`, `MU7_B`, `MU8_B`, `MU_ELE5`, `WDOG5`, pins `PIN_GPIO_IO14/15`; **no** display, pixel clock, LVDS or DSI entry | LM1 `M7` | 386–410 (`M7P` 386, `CAN_FD1` 387, `IRQSTEER_M7` 389, `LPUART3` 394, `MSGINTR1/2` 395–396, `MU5_A` 397, `MU7_B` 398, `MU8_B` 399, `MU_ELE5` 400, `WDOG5` 405, pins 409–410) |
| M7 memory: `M7MIX` data `0x0203_80000–0x0204_7FFFF` and `0x04A0_60000–0x04A0_9FFFF`; `DDR EXEC 0x8000_0000–0x89FF_FFFF` (corrected: the earlier note mis-grouped the hex digits) | LM1 | 414–416 |
| A55 non-secure DDR: `0x8800_0000–0x89FF_FFFF` and `0x8E00_0000–0x87FFF_FFFF` (as in the file, end address `0x87FFFFFFF`) | AP-NS | 1017–1018 |
| Display power domain `PD_DISPLAY`, perf domain `PERF_DISP` | Not an LM resource: referenced by each DC/LVDS resource in `devices/MIMX95/configtool/displaymix.cfg` (lines 39–66), so owning those resources is what pulls the power domain in (corrected: `DC` also lists `PD_CAMERA`, line 39, and `MIPI_DSI`/`MIPI_PHY` are defined in `cameramix.cfg` lines 56–57 with `PD_CAMERA`, not `PD_DISPLAY`); `PERF_DISP` is not in AP-NS's API list | — |
| TRDC: `DC` has `MBC_D0=…`/`MDAC_D*` entries and `DFMT1`; `LVDS` has `MBC_D0=0.12,1.12,0.16,1.16`; `DC0` `MBC_D0=0.13,1.13`; `DC1` `0.14,1.14,0.48,1.48` | `displaymix.cfg` | 39–41 (`DC`), 49 (`DC0`), 50–51 (`DC1`), 64–65 (`LVDS`) |

Note an overlap in the stock file: the DDR range `0x8800_0000–0x89FF_FFFF` is
listed for both the M7 (`0x8000_0000–0x89FF_FFFF`) and AP-NS. This is read
from the file as quoted; the overlap is confirmed from source (LM1 `DDR EXEC` at line 416, AP-NS
`DDR EXEC` at line 1017, imx-sm@af1e3702); whether the SM tolerates it or
the TRDC resolves it was not checked (**not verified**).

Cfg syntax (read from `configs/configtool.pl`, same commit): `OWNER` = full
TRDC permission plus full SCMI API permission; `ALL` on an API line = full
SCMI access, not a TRDC permission; `ACCESS` = same permissions without a
master domain id; `PD_*`, `PERF_*`, `CLK_*` tokens become `DEV_SM_PD_*`,
`DEV_SM_PERF_*`, `DEV_SM_CLK_*` constants. The generated headers are
`configs/mx95evk/config_*.h` (`config_lmm.h`, `config_scmi.h`, `config_trdc.h`,
`config_dev.h`, …).

Definitions located in the 2026-11-28 pass (read from source, imx-sm@af1e3702):
`MIPI_DSI` (`cameramix.cfg:56`) and `MIPI_PHY` (`cameramix.cfg:57`, with
`CLK_MIPIPHYCFG`), `LDB_PLL` (`ccmsrcgpcmix.cfg:63`, `CLK_LDBPLL`,
`CLK_LDBPLL_VCO`), `VIDEO_PLL1` (`ccmsrcgpcmix.cfg:64`), `CLOCK_DISP1PIX`
(`ccmsrcgpcmix.cfg:48`, `CLK_DISP1PIX`) and `IRQSTEER_M7` (`aonmix.cfg:163`,
`PD_AON`), all under `devices/MIMX95/configtool/`. Still no
`CLK_LDB*` or `PIXEL_LINK` entry appears in `mx95evk.cfg`. The LVDS bridge
clock chain is therefore **not fully traced**.

## 3. Proposed configuration (inferred design)

Goal: the M7 owns the whole DISPLAYMIX scan-out path and its memory; Linux
keeps everything else. Resource names below are the ones that exist in
`mx95evk.cfg`. Which of them a Verdin SM config actually uses is inferred.

The `.cfg` format puts a resource line in one LM section; moving it means
deleting it from AP-NS and adding it to LM1. Illustrative diff against
`configs/mx95evk.cfg` (a Verdin config would be a copy of the Toradex file
with the same edit; **not applied, not built, not validated by `make cfg`**):

```diff
--- a/configs/mx95evk.cfg
+++ b/configs/mx95evk.cfg
@@ LM1 "M7" (SCMI_AGENT0 "M7") API block @@
+CLOCK_DISP1PIX      ALL
+LDB_PLL             ALL
+PERF_DISP           ALL            # only if the M7 must set display perf level
@@ LM1 "M7" resources @@
+BLK_CTRL_DISPLAYMIX OWNER
+DC                  OWNER
+DC0                 OWNER
+DC1                 OWNER
+DC_2DBLIT           OWNER
+DC_BLITINT          OWNER
+DC_CMDSEQ           OWNER
+DC_DISPENG          OWNER
+DC_DISPENG_INT      OWNER
+DC_FL0              OWNER
+DC_FL1              OWNER
+DC_INT_CTL          OWNER
+DC_PIXENGINE        OWNER
+DC_XPC              OWNER
+DC_YUV0             OWNER
+DC_YUV1             OWNER
+DC_YUV2             OWNER
+DC_YUV3             OWNER
+LVDS                OWNER
+VIDEO_PLL1          OWNER
@@ LM1 "M7" memory (scanout buffers, DATA, inside the existing M7 DDR window) @@
+DDR                 DATA, begin=0x088000000, end=0x089FFFFFF
@@ AP-NS agent API block @@
-CLOCK_DISP1PIX      ALL
-LDB_PLL             ALL
@@ AP-NS resources @@
-BLK_CTRL_DISPLAYMIX OWNER
-DC OWNER, test
-DC0 .. DC1, DC_2DBLIT .. DC_YUV3 (all the DC_* lines)
-LVDS                OWNER
-VIDEO_PLL1          OWNER
```

Reasoning, all **inferred** from the cfg semantics above:

- The DC block's power domain `PD_DISPLAY` follows the resources: because
  every `DC*`/`LVDS`/`BLK_CTRL_DISPLAYMIX` entry in `displaymix.cfg` names
  `PD_DISPLAY`, moving them to the M7 LM makes the M7 LM the user of that
  domain. If both LMs listed resources that depend on `PD_DISPLAY` the domain
  would be shared and a Linux power-off could remove it under the M7; hence
  the move must be complete (all DC and LVDS entries leave AP-NS).
  **Read from source (imx-sm@af1e3702, 2026-11-28), qualifying this:** `DC`
  also lists `PD_CAMERA` (`displaymix.cfg:39`), and `MIPI_DSI`/`MIPI_PHY`/
  `MIPI_CSI*` stay with AP-NS with `PD_CAMERA` (`cameramix.cfg:56–59`), so
  `PD_CAMERA` would be used by both LMs after the move. The design as drawn
  therefore does not give the M7 sole use of every power domain; whether the
  SM tolerates shared use of `PD_CAMERA` was not checked.
- Keep `MIPI_DSI`/`MIPI_PHY` with AP-NS unless the panel path used by the M7
  is DSI. The target panel here is LVDS, so the diff leaves them in place.
- `CLK_DISPAPB/AXI/OCRAM` stay with LM0 (`ALL`): the SM sets them for any LM.
- `IRQSTEER_M7` is already owned by the M7 LM; it is the route a DC interrupt
  would take to the M7 NVIC. Which IRQSTEER channel the DC vblank uses was
  **not read** and the DT/RM mapping must be confirmed.
- Scanout memory: the example reuses part of the AP-NS window that is
  inside the M7's `EXEC` DDR range; this is only a placeholder. A real design
  needs a carve-out that is **absent from the A55 non-secure DDR list and
  from Linux's DT**, with a matching `reserved-memory` entry and TRDC/MRC
  permission for the DC master domains (`DFMT1`, `did`) — **not designed in
  detail**.
- Moving `LVDS` also requires the LVDS pin/pad ownership and the panel's
  backlight/enable GPIOs to stay reachable by whoever drives them (not
  surveyed).

## 4. What Linux would lose (inferred)

Linux (AP-NS) would lose every resource removed above, so the SCMI/TRDC
access to those blocks is denied. The kernel DT nodes that use them must be
disabled, or probing will fault or hang on the first register access:

| Linux function (kernel 6.6 NXP/Toradex tree, **not read in this item**) | Action |
|---|---|
| Display controller (DPU/`DC`) node and its display-engine / pixel-engine children | `status = "disabled"` |
| LVDS display bridge node (`ldb`/`LVDS`) and the DISPLAYMIX block-control / pixel-link (`dispmix_csr`/`mediamix`-style) node | `status = "disabled"` |
| The panel node and its `panel-lvds` / backlight (`pwm-backlight`) that hang off the bridge | disabled, or kept only if the backlight is still Linux-owned |
| DRM/KMS and Weston | no `/dev/dri/card*` for the panel; Weston and the HWLayer DRM backend have nothing to drive on this board (the M1 Linux measurement is no longer possible in this configuration) |
| `CLOCK_DISP*PIX`, `LDB_PLL`, `VIDEO_PLL1` clock consumers in the DT | remove or disable; the SCMI clock protocol would reject them |

Exact node names depend on the Toradex DT and were not read here. The
consequence for the roadmap: this design makes **Zephyr instead of Linux**
own the display, not alongside it. A Linux-and-Zephyr split of DC resources
is not expressible at resource granularity in this cfg format (the DC
sub-blocks are separate resources, but the display engine is one pipeline),
and was not investigated further.

## 5. How the SM image is built (read from source, imx-sm@af1e3702)

1. `make cfg` runs `configs/configtool.pl -i ./configs/$(CONFIG).cfg -o
   ./configs/$(CONFIG)` (rule in `sm/makefiles/configtool.mak:35`) to generate
   `configs/<CONFIG>/config_*.h` and `config.mak`.
2. `make config=<CONFIG> all` (README "Compiling an SM Image", `README.md:74–84`) produces
   `build/<CONFIG>/m33_image.bin`. A new cfg needs its own `configs/<name>.cfg`
   and a board directory under `boards/`.
3. The README says to copy `m33_image.bin` into the `iMX95` directory of
   imx-mkimage, build a boot container, and "program it to the boot device".
   The Yocto recipe's `flash_all`/`flash_a55` targets build that container;
   how `soc.mak` embeds the SM binary and OEI files was **not read**
   (404 on the pinned tag).

## 6. Installing it: blocked on the shared board

The SM runs from the boot container (the `flash.bin` image holding SM, OEI,
ATF and U-Boot). Replacing it needs one of:

- a write of the new container to the eMMC boot area, or
- a change to the boot flow (e.g. a different U-Boot/boot-media selection).

**Blocked here.** `docs/imx95-board.md` forbids `mmcblk0boot0`/`mmcblk0boot1`
writes, `fw_setenv`/`saveenv` and root-filesystem changes on the shared farm
Verdin iMX95 (read only for this item; the rule lines are 56–59). A bad SM
image can also leave the board unbootable, which on a shared board blocks
every other plan. This item did not touch the board and did not build or flash
an image.

What a dedicated board would need (inferred, none of it exists on this farm):

- a board that is not shared, with a recovery path independent of eMMC boot
  (serial download mode / USB boot, or an SD/USB boot selection with a known
  good container) and power-cycle control;
- the Toradex SM sources and the exact configuration of the shipped image, to
  base the edit on the Verdin file instead of `mx95evk`;
- imx-mkimage at the matching tag and the ATF/OEI/U-Boot binaries of the same
  BSP to assemble a full container;
- a Linux DT with the display nodes of §4 disabled and a matching M7 image
  with a DPU driver, loaded by U-Boot `bootaux` or a rproc firmware path;
- a fallback copy of the stock container on the same board.

## 7. Open points for the planner

- Read the Toradex 7.8.0 layers (git.toradex.com) for the real imx-sm
  `SRCREV` and any Verdin cfg; this note's pin is the NXP reference.
- Trace the LVDS clock chain (the `CLK_LDB*` names behind `LDB_PLL`).
- The no-reconfiguration alternative, the DC owned by AP-NS and driven from
  the M7 over RPMsg/shared memory, needs no SM change and is the remaining
  path on the shared board; it is not evaluated here.
