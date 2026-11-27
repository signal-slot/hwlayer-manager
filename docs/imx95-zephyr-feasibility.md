# i.MX95 Zephyr feasibility (M2): who owns the display

Draft from source reading, 2026-11-28. Board: Toradex Verdin iMX95 on a Mallow carrier, Toradex 10.1" LVDS panel, Reference Multimedia Image 7.8.0, kernel 6.6.157-7.8.0 (not Torizon). This is the first step of SoW §5.1 ("Zephyr second"), WP3 and M2.

**This draft observes nothing on a board.** Each finding has one of two tags:

- **read from source**: stated by the file named, at the pinned revision.
- **inferred**: concluded from what was read. The source does not say it.

A separate item takes the board snapshots, and a later item merges them into this file. Findings about the NXP EVK, or about NXP's own `imx95-19x19-verdin` files, apply to those files only. They are not claims about the Toradex BSP the board runs.

How the sources were read: the files were fetched over HTTPS from `raw.githubusercontent.com` and GitHub tree pages. `git clone` was not available in this run, and `git ls-remote` was used only to pin revisions. Some long files came back truncated or partly summarised; the rows that depend on them say so. Line numbers are not given; references name the file and the function, node or cfg line. For S7 a full Zephyr checkout at the pinned commit was searched afterwards, to settle the rows that the listings had cut off.

## Sources

| # | Repository | Revision | Files read |
| --- | --- | --- | --- |
| S1 | https://github.com/nxp-imx/imx-sm | tag `rel_imx_sm_2025q3` = commit `af1e37026c6b` (the same commit as tag `lf-6.6.52-2.2.2`) | `configs/mx95evk.cfg`; `configs/mx95evk/config_scmi.h`, `config_lmm.h`, `config_trdc.h` (partly summarised); `configs/mx95alt.cfg`; `configs/other/mx95evkrpmsg.cfg`; `configs/config.dox`; `README.md`; `sm/doc/config.md` (verbatim to about 39k of 53k characters); `sm/rpc/scmi/rpc_scmi.h`, `rpc_scmi.c` |
| S2 | https://github.com/nxp-imx/meta-imx | branch `scarthgap-6.6.52-2.2.2` = commit `474d2b45015f` | `meta-imx-bsp/recipes-bsp/imx-system-manager/imx-system-manager_2025q3.bb`, `imx-system-manager.inc`; `meta-imx-bsp/conf/machine/imx95-19x19-verdin.conf`, `include/imx95-evk.inc`; `meta-imx-bsp/recipes-bsp/imx-mkimage/imx-boot_1.0.bb`, `imx-boot_1.0.bbappend` |
| S3 | https://github.com/nxp-imx/imx-mkimage | tag `lf-6.6.52-2.2.2` = commit `81fca6434be0` (also the head of branch `lf-6.6.52_2.2.1`) | `iMX95/soc.mak` |
| S4 | https://github.com/nxp-imx/imx-atf | tag `lf-6.6.52-2.2.2` = commit `8ec7e38031f8` | `plat/imx/imx95/imx95_m7.c`, `imx95_lmm.c`; `plat/imx/common/imx_sip_svc.c`, `include/imx_sip_svc.h` |
| S5 | https://github.com/nxp-imx/uboot-imx | tag `lf-6.6.52-2.2.2` = commit `ce32a3dfd3e4` | `arch/arm/mach-imx/imx_bootaux.c` |
| S6 | https://github.com/nxp-imx/linux-imx | tag `lf-6.6.52-2.2.2` = commit `5a0a5e71d2bd` | `drivers/remoteproc/imx_rproc.c` (full); `remoteproc_sysfs.c`; `remoteproc_core.c` (truncated, second half summarised); `drivers/firmware/arm_scmi/` (listing); `arch/arm64/boot/dts/freescale/imx95.dtsi` (DPU node cut off mid-node); `imx95-19x19-evk.dts`; `imx95-19x19-verdin.dts`, `-verdin-rpmsg.dts`, `-verdin-panel-cap-touch-10inch-lvds.dts`; `Makefile` |
| S7 | https://github.com/zephyrproject-rtos/zephyr | `main` = commit `e53873cbd0ef` (2026-11-28) | `drivers/display/display_socionext_dpu.c`, `Kconfig.socionext_dpu`, `CMakeLists.txt`; `drivers/mipi_dsi/dsi_nxp_dwc.c`; `dts/arm/nxp/imx/nxp_imx95_m7.dtsi`; `boards/nxp/imx95_evk/` (`board.yml`, `imx95_evk_mimx9596_m7{,_ddr}.{dts,yaml}`, `doc/index.rst`); `soc/nxp/imx/imx9/imx95/m7/soc.c`, `m7/linker.ld`, `Kconfig.defconfig.mimx95.m7`; `samples/subsys/ipc/openamp_rsc_table/boards/imx95_evk_mimx9596_m7.{overlay,conf}`; `boards/toradex/` (listing); `boards/shields/nxp_mx9_dsi_oled/` (overlay, `shield.yml`, doc). The LVDS and shield rows were then checked by searching a full checkout at the same commit |
| S8 | https://git.toradex.com/meta-toradex-nxp.git | branch `scarthgap-7.x.y` = commit `b0b9f0fd9d3e` | **Not read.** The cgit pages return a bot challenge, and clone was not available |
| S9 | https://git.toradex.com/linux-toradex.git | branch `toradex_6.6-2.2.x-imx` = commit `8ad2b7ff6b56` (6.6.161, newer than the board) | **Not read** (same reason) |
| S10 | https://git.toradex.com/device-tree-overlays.git | branch `toradex_6.6-2.2.x-imx` = commit `e9a3ca6fbd6b` | **Not read** (same reason) |

S8 to S10 are what the board actually runs. The SM configuration, the Verdin/Mallow device tree and the LVDS panel overlay that Toradex ships are therefore **unknown**. No Toradex `imx-sm` fork was found: `github.com/toradex/imx-sm` and `git.toradex.com/imx-sm.git` do not exist (checked with `git ls-remote`). That does not rule out a Toradex patch or cfg file inside S8.

## Ownership model

### How the System Manager partitions the SoC

| Finding | Basis |
| --- | --- |
| The SM (on the M33) splits the SoC into logical machines (LMs) "which have statically configurable access rights to both hardware and RPC API calls" (README). Each cfg file is compiled into the SM firmware: `configtool` turns `<name>.cfg` into the `configs/<name>/config_*.h` headers (`sm/doc/config.md`, `configs/config.dox`) | read from source (S1) |
| A resource line inside an LM section gives that resource to the LM. `OWNER`, `ACCESS` or `READONLY` picks the permission set. API rights are granted with `ALL`, `SET`, `NOTIFY` and `PRIV`, and higher rights imply the lower ones | read from source (S1, cfg layout and `config.md`); that moving a line moves the resource is inferred |
| `boot=` on an LM header gives the boot order (0 or absent means do not boot). `skip=1` only suppresses the error when the boot container holds no image for that LM; it does not defer the boot to another LM. An LM with no `boot=` can be started later with `LMM_Boot()` / `SCMI_LmmBoot()` | read from source (S1, `sm/doc/config.md`) |
| `config.md` gives "Handing a display over from one LM to another" as an example of `SM_SCMI_PERM_EXCLUSIVE`. This is a runtime SCMI permission; it is not known whether it also moves TRDC (bus) access | read from source for the quote; scope unknown |
| Configs under `configs/other` "are delivered as is and not maintained or tested as part of an SM release" | read from source (S1) |
| The NXP LMM protocol is SCMI 0x80 and the CPU protocol 0x82 (`rpc_scmi.h`) | read from source (S1) |

### `mx95evk.cfg`, the config NXP builds for its own Verdin machine

S2 builds `mx95evk` for `imx95-19x19-verdin`. That machine requires `imx95-evk.inc`, which sets `SYSTEM_MANAGER_CONFIG` to `mx95evk` unless an `IMXBOOT_VARIANT` is set, and the Verdin conf does not override it (read from source, S2). **Whether Toradex's BSP builds the same config is not known (S8 not read).**

| LM | Header | CPUs | Display resources |
| --- | --- | --- | --- |
| LM0 | `name="SM", rpc=none, boot=1, did=2` | M33P | `GPV_DISPLAY OWNER`; clocks `CLK_DISPAPB`, `CLK_DISPAXI`, `CLK_DISPOCRAM` under "Modified via PERF protocol" |
| LM1 | `name="M7", rpc=scmi, boot=2, skip=1, did=4` | M7P | **none** |
| LM2 | `name="AP", rpc=scmi, boot=3, skip=1, did=3, default` | A55C0-5, A55P | all of the rest, in the AP-NS (Linux) agent: `DC OWNER`, `DC0`, `DC1` and every `DC_*` sub-block (`DC_DISPENG`, `DC_PIXENGINE`, `DC_FL0/1`, `DC_YUV0-3`, `DC_INT_CTL`, ...), `LVDS`, `MIPI_DSI`, `MIPI_PHY`, `BLK_CTRL_DISPLAYMIX`, `VIDEO_PLL1` (all `OWNER`); API `CLOCK_DISP1PIX/2PIX/3PIX ALL`, `LDB_PLL ALL` |

All rows read from source (S1, `mx95evk.cfg`). The generated `config_scmi.h` agrees: the AP-NS agent has `.clkPerms[DEV_SM_CLK_DISP1PIX]`, `[DEV_SM_CLK_LDBPLL]`, `[DEV_SM_CLK_VIDEOPLL1]`, `.pdPerms[DEV_SM_PD_DISPLAY]` and `.perfPerms[DEV_SM_PERF_DISP]` = `SM_SCMI_PERM_ALL`. The M7 agent has only `.pdPerms[DEV_SM_PD_M7]` and no display permission (read from source).

The M7 LM also holds: TCM (`M7MIX DATA` ranges), `DDR EXEC` from 0x80000000 to 0x89FFFFFF, `LPUART3`, `MU5_A`, `MU7_B`, `MU8_B`, `IRQSTEER_M7`, timers and `WDOG5`. Its SCMI mailbox line is `MAILBOX type=mu, mu=9, ...` (read from source). `MU7` is the pair Linux's `cm7` node uses (S6), and its A side is in AP-NS (read from source).

Nowhere in the cfg does display interrupt routing appear (the display `irqsteer`, or `PD_DISPLAY` as a resource line). `PD_DISPLAY` shows up only as an SCMI permission in the generated header (read from source). The display irqsteer probably sits under `BLK_CTRL_DISPLAYMIX` or a `DC_*` TRDC region, but that was not checked (`config_trdc.h` holds only raw register writes) (inferred).

### The other two mx95 configs

| Config | Display owner | M7 control from the AP | Basis |
| --- | --- | --- | --- |
| `configs/other/mx95evkrpmsg.cfg` | AP-NS of LM2, the same list as mx95evk; LM1 "M7" has none | AP-S: `PD_M7 ALL`, `CPU_M7P ALL`, `LMM_1 ALL`; AP-NS: `LMM_1 ALL`, `PD_M7 ALL` | read from source (S1) |
| `configs/mx95alt.cfg` ("for MCUXpresso testing") | One LM, `LM1 name="Boot"`, holds M7P and all A55 cores and every display resource. Three SCMI agents (M7, AP-S, AP-NS) with the same API rights (`dup=0`) | not separate: same LM | read from source (S1) |

**No NXP config gives the display to an M7 LM that runs next to Linux.** `mx95alt` is the only one where the M7 can reach the display, and there the M7 and A55 share a single LM with no isolation between them (read from source; "no isolation" inferred from the one shared DID).

### Who owns what, by resource (mx95evk, Linux and M7 side by side)

| Resource | Owner under mx95evk | Basis |
| --- | --- | --- |
| DPU registers (`DC`, `DC0`, `DC1`, `DC_*`) | LM2 (AP-NS, Linux) | read from source (S1) |
| LVDS (LDB + PHYs), MIPI DSI, MIPI PHY, DISPLAYMIX blk-ctrl | LM2 | read from source (S1) |
| Pixel clocks DISP1-3PIX, LDB PLL, VIDEO PLL1 | LM2 (SCMI `ALL`) | read from source (S1) |
| DISP AXI/APB/OCRAM clocks, display GPV | LM0 (SM), changed via PERF; AP-NS has `perfPerms[DEV_SM_PERF_DISP]` | read from source (S1) |
| Display power domain `PD_DISPLAY` | AP-NS SCMI permission `ALL`; M7 none | read from source (S1) |
| DSI power domain | Linux DT puts `mipi_dsi` in `IMX95_PD_CAMERA`, not `PD_DISPLAY`; Zephyr's `soc.c` powers both | read from source (S6, S7) |
| Display interrupts | Linux: DPU interrupts go through `displaymix_irqsteer` (interrupt-controller@4b0b0000, `PD_DISPLAY`, GIC SPIs 214-219 and 303). Zephyr: `disp_irqsteer` at the same address, master1 → NVIC 215 | read from source (S6, S7). Whether M7 NVIC 215 and A55 GIC SPIs are outputs of the same irqsteer whose channel masks both cores would program is inferred |
| Scanout memory | Linux scans out of CMA (`alloc-ranges` 0x80000000 + 0x7F000000, S6 EVK/Verdin DT). The M7 LM has `DDR EXEC` over 0x80000000-0x89FFFFFF, which overlaps the start of that CMA range and the vring carveouts at 0x88000000 | read from source (S1, S6). Which DID the DPU uses as a bus master, and so which DDR it may read, is set by the TRDC setup and was not read (unknown) |

Conclusion (inferred): under `mx95evk` and `mx95evkrpmsg`, the A55/Linux LM owns the DPU, the LVDS and DSI routes, their clocks and the display power domain. The M7 LM has no access. If the board's SM follows either config, the M7 cannot drive the display, and a DPU register read from the M7 should be refused by the TRDC (a bus fault). The fault is inferred from the ownership model; the TRDC programming itself was not read.

## How the M7 is started

| Path | Finding | Basis |
| --- | --- | --- |
| Boot container | `imx-mkimage` `flash_all` packs `-m7 $(M7_IMG) 0 $(M7_TCM_ADDR) $(M7_TCM_ADDR_ALIAS)` (`M7_IMG ?= m7_image.bin`); `flash_a55` has no `-m7`. `flash_lpboot_sm_all` = `flash_all`, `flash_lpboot_sm_a55` = `flash_a55` | read from source (S3) |
| NXP Verdin machine | `IMXBOOT_TARGETS_BASENAME = "flash"` gives `flash_all flash_a55` for SD/eMMC, and `flash_all` becomes `imx-boot`. `M4_DEFAULT_IMAGE_MX95 = "imx95-verdin-evk_m7_TCM_power_mode_switch.bin"` is copied to `m7_image.bin`. Four `imx95-verdin-evk_m7_TCM_*.bin` demos are added to `IMAGE_BOOT_FILES` | read from source (S2) |
| SM at reset | With `boot=2, skip=1`, the SM boots LM1 at reset if the container holds an M7 image and ignores its absence otherwise | read from source (S1 `config.md`) + inferred for this board |
| Consequence | On NXP's Verdin image the M7 is already running the power_mode_switch demo when Linux starts. Whether the Toradex image does the same is unknown (S8 not read) | inferred |
| U-Boot `bootaux` | `imx_bootaux.c` has no i.MX95, SCMI or LMM code; its body is `#ifndef CONFIG_IMX8` and handles i.MX8M/older SRC. An i.MX95 `bootaux` path, if one exists, is elsewhere and was not found | read from source (S5) |
| Linux `imx_rproc` | `fsl,imx95-cm7` uses `.method = IMX_RPROC_SMC`: start is `arm_smccc_smc(IMX_SIP_RPROC, IMX_SIP_RPROC_START, rproc->bootaddr, ...)`, stop is `IMX_SIP_RPROC_STOP`, and probe asks `IMX_SIP_RPROC_STARTED`. The 6.6 driver has no SCMI LMM or CPU code and no `fsl,lmm-id`/`fsl,cpu-id`. `drivers/firmware/arm_scmi/` has only `imx-sm-bbm.c` and `imx-sm-misc.c`, and `imx95.dtsi` declares no LMM or CPU protocol node | read from source (S6) |
| TF-A | SiP `0xC2000005` (`IMX_SIP_SRC`) → `imx_src_handler()` in `imx95_m7.c`. START is `scmi_core_set_reset_addr(..., x2, IMX9_SCMI_CPU_M7P, SCMI_CPU_VEC_FLAGS_BOOT)` then `scmi_core_start(..., IMX9_SCMI_CPU_M7P)`. STOP is `scmi_core_stop` then `scmi_base_reset_agent_config(agent "M7")`. STARTED returns 1 for run mode START or SLEEP, and the return code of `scmi_core_info_get` is not checked. These are the SCMI **CPU** protocol (0x82). A separate SiP `0xC200000F` does LMM boot/shutdown (`imx95_lmm.c`), but `imx_rproc` 6.6 does not call it | read from source (S4) |
| Who asks the SM | TF-A runs in EL3, so it talks to the SM as the secure AP agent (AP-S of LM2) | inferred |
| Already running | If STARTED says the core runs, `imx_rproc_detect_mode()` sets `RPROC_DETACHED`; Linux then attaches (mailbox only) and loads nothing. `imx_rproc_detach()` returns `-EOPNOTSUPP` for this method, so `echo detach` fails | read from source (S6) |
| Boot address | For `fsl,imx95-cm7`, `imx_rproc_get_boot_addr()` returns the address of the ELF section named `.interrupts` | read from source (S6) |
| Firmware load | `rproc_alloc(dev, "imx-rproc", ..., NULL, ...)`, so there is no DT firmware name; the default is `rproc-imx-rproc-fw` | `rproc_alloc` call read from source; the default name inferred from a summarised `remoteproc_core.c` |
| | Sysfs `firmware` store calls `rproc_set_firmware()`, which only works in `RPROC_OFFLINE`. `state` takes `start`, `stop` and `detach` | read from source (S6) |
| | `rproc_boot()` loads with `request_firmware()`, so the `firmware_class` search path applies | `request_firmware` from a summarised `remoteproc_core.c`; the search path inferred |
| | ELF segments go through `imx_rproc_da_to_va()`. The i.MX95 table maps TCM code da `0x0` → `0x203C0000` (256 KiB), TCM sys da `0x20000000` → `0x20400000` (256 KiB) and DDR `0x80000000` 1:1 (`0x50000000`) | read from source (S6) |
| Resource table | Optional ("No resource table in elf" is only logged). A `memory-region` named `rsc-table` is used as the loaded table (1 KiB) | read from source (S6) |

### Can Linux start a different M7 image at runtime?

The Linux side needs no persistent change (inferred):

1. `/sys/module/firmware_class/parameters/path` is a runtime parameter, so it can point at a directory under `/tmp`. This was not read in S6: it is the standard firmware loader, inferred.
2. `/sys/class/remoteproc/remoteprocN/firmware` is set to the file name, then `state` to `start`.

None of this writes the rootfs, the U-Boot environment or the eMMC boot partitions. Whether the start then works depends on the SM:

- Under `mx95evk`, AP-S has no `CPU_M7P` permission, and AP-NS has only `LMM_1 NOTIFY` (`.lmmPerms[1] = SM_SCMI_PERM_NOTIFY`, no `cpuPerms` for M7P). TF-A's `scmi_core_set_reset_addr`/`scmi_core_start` would be refused, and so would `scmi_core_stop` (inferred from S1 and S4; the SM's permission check in the CPU protocol handler was not read).
- Under `mx95evkrpmsg`, AP-S has `CPU_M7P ALL`, so the same calls are allowed (inferred, same caveat).
- `mx95evkrpmsg` sits under `configs/other`, and the S2 recipe has no variant for it. S2's NXP Verdin image builds `mx95evk` and boots the M7 from the container. Under that config, Linux can only attach to the running M7; it can neither stop it nor load a new image (inferred).

## What Linux holds

The Toradex Verdin/Mallow device tree and its LVDS overlay (S9, S10) were not read. The rows below are NXP's own `imx95-19x19-verdin*` files (S6). They show what NXP's Verdin build claims; they are not the board's tree.

| Node | NXP Verdin (S6) | Basis |
| --- | --- | --- |
| Base | `imx95-19x19-verdin.dts` is `#include "imx95-19x19-evk.dts"` plus Verdin changes; it has no display or reserved-memory nodes of its own | read from source |
| DPU, display irqsteer | `&dpu` and `&displaymix_irqsteer` `okay` (from the EVK dts) | read from source |
| LVDS route (10" panel file) | `imx95-19x19-verdin-panel-cap-touch-10inch-lvds.dts`: `&display_pixel_link`, `&ldb` (LDBPLL rates `<2986200000>, <497700000>`), `channel@0` → `lvds0_out` → `lvds0_panel` (`compatible = "logictechno,lt170410-2whc"`), `&ldb0_phy`, `&pixel_interleaver` channel 0, `pwm-backlight` on `&tpm6 2`, all `okay`. No `&dpu` change | read from source |
| MIPI DSI | Not enabled by the base or the LVDS file; a separate `-panel-cap-touch-10inch-dsi` overlay exists | read from source |
| M7 rproc | `cm7: imx95-cm7`, `compatible = "fsl,imx95-cm7"`, `mboxes` = `&mu7` (tx, rx, rxdb), `memory-region = <&vdevbuffer>, <&vdev0vring0>, <&vdev0vring1>, <&vdev1vring0>, <&vdev1vring1>, <&rsc_table>`, `fsl,startup-delay-ms = <50>`, `status = "okay"`; no `firmware-name`, no `fsl,auto-boot` | read from source |
| Reserved memory | `vdev0vring0/1` at 0x88000000/0x88008000, `vdev1vring0/1` at 0x88010000/0x88018000 (0x8000 each), `vdevbuffer` 0x88020000 (1 MiB, `shared-dma-pool`), `rsc-table` 0x88220000 (4 KiB), all `no-map`. CMA 0x3c000000 bytes in 0x80000000 + 0x7F000000. No `m7@80000000` carveout in the base | read from source |
| `-rpmsg` variant | Adds `m7@80000000` (16 MiB, `no-map`), `micfil-rpmsg`, `audio-rpmsg`, rpmsg audio and I2C bus nodes; disables SAI3, MICFIL, eDMA1/2, LPUART5. The cm7 node is unchanged | read from source |
| `vdevbuffer` name | The driver skips only a region named `vdev0buffer` when it builds carveouts, so `vdevbuffer` is also registered as an ordinary carveout | inferred from S6 code, not run |
| SCMI | `arm,scmi` on `mu2`, protocols 0x11 power, 0x12 system, 0x13 perf, 0x14 clock, 0x15 sensor, 0x19 pinctrl, 0x81 BBM, 0x84 misc. No 0x80 LMM, no 0x82 CPU | read from source (S6 `imx95.dtsi`) |

**Is the Toradex 10.1" panel the same `logictechno,lt170410-2whc` on LDB channel 0?** Unknown. It is only NXP's Verdin file, S10 was not read, and the board observation should read it from `/proc/device-tree`.

## Can the M7 own the DPU/LVDS route?

### Alongside Linux

Not without Linux giving it up (inferred).

- The SM can split resources only by its cfg lines (`DC0`, `DC1` and the `DC_*` sub-blocks are separate entries). The DPU's fetch units, LayerBlends and scalers, however, are one pool shared by both streams, and Linux's `dpu95` driver manages that pool across both CRTCs (`docs/imx95-dpu-source-6.6.md`).
- Giving one stream (`DC1` plus its FrameGen) to the M7 while Linux keeps `DC0` would need a cfg that no NXP release provides. It would also need a `dpu95` driver that leaves part of the pixel engine alone, which the 6.6 driver does not do.
- The `SM_SCMI_PERM_EXCLUSIVE` "display hand-over" mention in `config.md` covers SCMI permissions. It is not known to cover register (TRDC) access.

### Instead of Linux

Possible in principle (inferred). It would take every one of these:

1. **SM cfg change:** move `DC`, `DC0`, `DC1`, all `DC_*`, `LVDS`, `BLK_CTRL_DISPLAYMIX`, `VIDEO_PLL1` and the display irqsteer region (location not confirmed) to LM1. Grant the M7 agent `CLOCK_DISP1PIX`, `LDB_PLL`, `PD_DISPLAY` and `PERF_DISP`. Give AP-S `CPU_M7P` (and `PD_M7`) if Linux should still be able to restart the M7. Rebuild the SM firmware.
2. **New boot container:** the SM firmware lives in the `imx-boot` container (S2, S3). A new SM means writing the boot container, which on this board is in the eMMC boot partitions. **The board rules forbid that.**
3. **Linux DT overlay:** disable `&dpu`, `&displaymix_irqsteer`, `&pixel_interleaver`, `&display_pixel_link`, `&ldb`, `&ldb0_phy`, the panel and the backlight in Linux. Otherwise Linux probes hardware it no longer owns. Loading such an overlay at boot needs a U-Boot env or `/boot` change, which the board rules also forbid.
4. **Zephyr driver work:** upstream Zephyr has no i.MX95 LDB/LVDS driver (next section). The Verdin's panel is LVDS.

Alternative: a single-LM config in the style of `mx95alt` (M7 and A55 in one LM) also lets the M7 reach the display, but with no isolation from Linux. It needs the same new SM firmware and boot container (inferred).

## Scanout and vblank/latch path in upstream Zephyr

| Item | Finding | Basis |
| --- | --- | --- |
| Driver | `display_socionext_dpu.c` (`socionext,dpu`): `dpu_init()` builds ConstFrame0 + FetchYUV0 → LayerBlend1 → ExtDst0, DomainBlend0, `displayMode = kDPU_DisplayOnlyPrim`. Up to 2 driver framebuffers (`DPU_FB_NUM`, default 2) | read from source (S7) |
| Scanout | `dpu_write()` sets the FetchYUV0 buffer address (`DPU_SetFetchUnitSrcBufferAddr`), calls `DPU_TriggerPipelineShadowLoad(..., kDPU_PipelineExtDst0)`, starts the display on the first write (`DPU_StartDisplay(config->base, 0)`), then blocks in `k_sem_take(&data->sem, K_FOREVER)` | read from source (S7) |
| Interrupt | Only `kDPU_Group0ExtDst0ShadowLoadInterrupt` is enabled. `dpu_isr` gives the semaphore. Nothing handles FrameGen, VSYNC or frame-done. The ISR reads status through a hard-coded `APP_DPU_DC` = `0x4B780000`, not the DT `reg` (`0x4b400000`, size `0x1000`) | read from source (S7) |
| Latch semantics | `dpu_write()` returns 0 once the ExtDst0 shadow load fires. It gives no sequence number, timestamp or error, and it hangs forever if the interrupt never arrives. The shadow load stands for "the new address has been latched for the next frame"; that this matches a KMS flip-done is inferred, not stated | read from source; meaning inferred |
| Stop / errors | No stop, blank or PM path. No return codes are checked; the driver makes no SCMI calls | read from source (S7) |
| Clocks and power | In `soc/.../m7/soc.c` `soc_init()`, only `#if defined(CONFIG_MIPI_DSI_NXP_DWC)`: SCMI power-on of `IMX95_PD_DISPLAY` and `IMX95_PD_CAMERA` (unbounded poll on `scmi_power_state_get`), `DPU_IRQSTEER->CHN_MASK[13] = 0x10249U`, pixel-link setup, VIDEO PLL1 (4008000000/446333333) and `DISP1PIX` = 148444444 from VIDEOPLL1 | read from source (S7) |
| Route | DPU → pixel link → MIPI DSI (`dsi_nxp_dwc.c`, `nxp,mipi-dsi-dwc`, blocking transfers, no IRQ). No LVDS/LDB driver, binding or DT node in Zephyr: a case-insensitive search for `ldb`/`lvds` under `drivers/`, `dts/bindings/` and `dts/arm/nxp/imx/` finds only unrelated substrings and Renesas display drivers | read from source (S7, searched in a full checkout at the pinned commit) |
| SCMI from the M7 | `arm,scmi` on `mu5` (`mailbox@44610000`, shmem `0x44611000`), protocols power, system, clock, pinctrl and `nxp,scmi-cpu` (0x82); no reset and no LMM. The dtsi does not say which SM agent it is | read from source (S7). That `mu5` is the M7 agent's channel matches `MU5_A OWNER` in LM1 (S1) and is inferred |
| Under mx95evk | The M7 agent has no `PD_DISPLAY` or display clock permission, so `soc_init()` would fail at the display power-on (it `return ret`s). The driver itself would fault on DPU register access | inferred from S1 + S7 |
| Boards | `imx95_evk/mimx9596/m7` (TCM), `/m7/ddr`, `/m7/flash`. Neither m7 yaml lists `display`. The m7 board DT only adds labels `zephyr_dpu`, `zephyr_mipi_dsi` and `display_i2c`. The `nxp_mx9_dsi_oled` shield (NXP MX9-DSI-OLED, `raydium,rm692c9` MIPI DSI panel, an `adi,adp5585` GPIO expander) sets `&zephyr_dpu` and `&zephyr_mipi_dsi` to `okay` and documents `imx95_evk/mimx9596/m7`. It is a DSI panel on the EVK, not this board's LVDS panel. Console `lpuart3`. No Toradex Verdin iMX95 board (`boards/toradex/` has `colibri_imx7d`, `verdin_am62`, `verdin_imx8mm`, `verdin_imx8mp`) | read from source (S7) |
| Loading | The EVK doc loads the M7 through a boot container (`flash.bin` via SPSDK, or `zephyr.bin` renamed to `m7_image.bin`). Remoteproc is documented only for the A55 Zephyr target | read from source (S7) |
| Remoteproc fit | TCM build: code at ITCM `0x0` and data at DTCM `0x20000000`, both inside `imx_rproc`'s i.MX95 table. A `.resource_table` section exists only with `CONFIG_OPENAMP_RSC_TABLE`. `openamp_rsc_table` has an `imx95_evk_mimx9596_m7` overlay (`mu7`, shmem `0x88000000`, `rsc_table` `0x88220000`) that matches the Linux carveouts | read from source (S6, S7) |
| Boot address | Zephyr's vector table section is not named `.interrupts`, so what `imx_rproc_get_boot_addr()` returns for a Zephyr ELF is unknown. For a TCM build whose vectors sit at `0x0` the result may still be right | inferred |

For the engine's needs (SoW §5.1: scanout and vblank/latch first):

- The stock driver gives one plane and a blocking "latched" point with no timestamp or frame counter.
- A Zephyr backend would need a non-blocking commit, a latch event with a frame number, and a timeout. It would get these from the ExtDst shadow-load interrupt plus a FrameGen counter or interrupt. The Linux DT names `framegen*` interrupts in the same irqsteer, which shows they exist.
- It would also need an LDB driver for this panel, and overlay support (more fetch units and LayerBlends).

All four points are inferred.

## Gate: can an M7 image be started from Linux remoteproc with files only under /tmp?

**Answer: unknown. Under NXP's default SM config it is expected to be no.**

Reasons:

1. **The Linux side does not block it (inferred).** The firmware search path and the remoteproc `firmware`/`state` attributes are runtime sysfs settings. They write nothing to the rootfs, the U-Boot env or the eMMC boot partitions.
2. **The SM decides (inferred from S1 + S4).** `imx_rproc` (6.6) → TF-A → SCMI CPU `set_reset_addr`/`start`/`stop` on M7P, as the AP-S agent:
   - **No** under `mx95evk`, the config NXP builds for its Verdin machine: AP-S has no `CPU_M7P` right.
   - **Yes** under `mx95evkrpmsg`: `CPU_M7P ALL`.
   - **Unknown** for Toradex, whose SM config was not read (S8).
3. **The M7 may already be running (inferred from S2, S3, S6).** NXP's Verdin build packs an M7 demo into `flash_all`, and the SM boots it. Linux then attaches (`RPROC_DETACHED` → attached). To load a new image it must first `stop`. Stop is the same SCMI CPU call, and `detach` is unsupported.
4. **Stopping is the risky step (inferred).** If `stop` succeeds and `start` is then refused or the image is bad, the M7 stays down until the board is reset. The farm has no remote power control for this board, so the original state could not be restored remotely.
5. **The rproc node may not exist (unknown).** Whether the Toradex DT has an enabled `fsl,imx95-cm7` node is unknown (S9 not read). Without it there is no remoteproc device at all.

What would turn the answer into yes, all from read-only observation:

- (a) A `remoteproc` device whose `name` matches the imx rproc.
- (b) `state` = `offline`, not `attached`/`running`. Then a failed `start` leaves the core as it was, which is harmless.
- (c) Ideally, evidence of the SM config: the SM build banner on its debug UART or an SCMI misc config-info. Without that, case (b) is the only safe one to try.

If `state` is `attached` or `running`, the M7 run should **not** stop the core. It should record the blocker instead.

## Open questions for the board observation

1. `/sys/class/remoteproc/*/{name,state,firmware}`: is there an imx rproc, and is it `offline`, `attached` or `running`?
2. `/proc/device-tree`:
   - Is there an `imx95-cm7` node, and with what status, `mboxes` and `memory-region`?
   - Which `reserved-memory` children exist, and is there an `m7@80000000`?
   - The `scmi` protocol children: any 0x80/0x82?
   - The status of `dpu`, `ldb` (and which channel), `pixel_interleaver`, `display_pixel_link`, `mipi_dsi` and `displaymix_irqsteer`.
   - The panel `compatible`.
3. dmesg: the SCMI version/vendor lines, the imx_rproc probe ("attach"/detached), and dpu95/ldb probe messages. Does the kernel print an SM firmware build or config name (`imx-sm-misc`)?
4. `/sys/module/firmware_class/parameters/path`: does it exist and is it writable? Read it only; do not change it.
5. Debugfs `clk_summary`: the parents and rates of `disp1pix`, `ldbpll`, `videopll1`, `dispaxi` and `dispapb`, if the clocks are visible through SCMI. Debugfs `pm_genpd_summary`: is the display domain on, and is there an M7 domain?
6. `/proc/interrupts`: the display irqsteer and dpu95 lines. Are they owned by Linux and counting?
7. The SoC revision (`/sys/devices/soc0/revision`). It also matters for the VScaler4 question in `docs/imx95-dpu-source-6.6.md`.

## Proposed M2 items

1. **Read the Toradex sources** (needs `git clone` from git.toradex.com):
   - S8: the `verdin-imx95` machine conf, the SM recipe, `SYSTEM_MANAGER_CONFIG`, the imx-boot targets and whether an M7 image is in the container.
   - S9 and S10: the Verdin/Mallow DT and the 10.1" LVDS overlay.
   - Pin `SRCREV`s at the commit that matches 7.8.0.
2. **Board observation:** the open questions above. Read only.
3. **Gate decision:** decide the gate from items 1 and 2 by the rules above. Only an `offline` rproc and a config that grants `CPU_M7P` makes it yes.
4. **M7 hello via remoteproc:** only if the gate is yes. Build a TCM image, optionally with a resource table. Log over the M7 UART, if one is reachable on Mallow (the Zephyr EVK uses `lpuart3`; the Verdin pin-out is unknown), or over rpmsg. Restore the firmware path afterwards.
5. **Ownership probe from the M7:** only after item 4. Read DPU or display blk-ctrl registers from the M7 behind a bus-fault handler, and query SCMI `PD_DISPLAY` state as the M7 agent. Expected under mx95evk: fault or denied (inferred). This turns the ownership model into an observed fact without touching Linux's display.
6. **Blocker report:** if items 3-5 confirm Linux ownership, write the evidence-based blocker report SoW §5.1/M2 asks for. Scanout from the M7 then needs a new SM config, a new boot container and a Linux DT change, all outside the board rules. In its place, propose the reduced-scope bring-up the plan allows: the M7 hello/probe, plus scanout on a board the project may reflash.
7. **If a reflashable board is granted:** a custom SM cfg giving the display to LM1, a Linux DT with the display disabled, an LDB driver for Zephyr, and a Zephyr adapter that reports latch (ExtDst shadow load plus a FrameGen counter). Overlay comes only after that.
