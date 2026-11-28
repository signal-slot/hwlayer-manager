# i.MX95 Zephyr feasibility (M2): who owns the display

Source reading 2026-11-28, merged with a read-only board snapshot taken 2026-11-27. Board: Toradex Verdin iMX95 on a Mallow carrier, Toradex 10.1" LVDS panel, Reference Multimedia Image 7.8.0, kernel 6.6.157-7.8.0 (not Torizon). This is the first step of SoW §5.1 ("Zephyr second"), WP3 and M2.

Each finding has one of three tags:

- **observed (date)**: seen on the board over ssh, read-only, in `docs/imx95-zephyr/observed-2026-11-27.txt` (S11).
- **read from source**: stated by the file named, at the pinned revision.
- **inferred**: concluded from what was read or observed. Neither says it.

Where the source and the observation disagree, the row says so. No finding here is a claim about another SoC. Findings about the NXP EVK, or about NXP's own `imx95-19x19-verdin` files, apply to those files only. They are not claims about the Toradex BSP the board runs.

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

| S11 | Board snapshot, `docs/imx95-zephyr/observed-2026-11-27.txt` | taken 2026-11-27 (board clock 18:49 UTC), Reference Multimedia Image 7.8.0 build 17, kernel 6.6.157-7.8.0 | remoteproc sysfs, `/proc/device-tree`, dmesg, debugfs `clk_summary` and `pm_genpd`, `/lib/firmware`. Not read: `/proc/interrupts`, SoC revision, the SM cfg, any M7 console |

S8 to S10 are what the board actually runs. The SM configuration, the Verdin/Mallow device tree and the LVDS panel overlay that Toradex ships are therefore **unknown** in this document's own reading. The later read in [imx95-verdin-sm-config.md](imx95-verdin-sm-config.md) (2026-11-28) supersedes the earlier statement that no Toradex `imx-sm` fork exists: the fork is `imx-sm-toradex` (`git.toradex.com/imx-sm-toradex.git`, pinned `SRCREV` `9e5555333eb27c75dc670945443a13dfc2760f16`, read from source there). The earlier `git ls-remote` check covered only the names `github.com/toradex/imx-sm` and `git.toradex.com/imx-sm.git`. The fork's `configs/verdin-imx95.cfg` itself has still not been read.

## 1. Ownership model

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

S2 builds `mx95evk` for `imx95-19x19-verdin`. That machine requires `imx95-evk.inc`, which sets `SYSTEM_MANAGER_CONFIG` to `mx95evk` unless an `IMXBOOT_VARIANT` is set, and the Verdin conf does not override it (read from source, S2). **Toradex's BSP does not build the same config:** its layer sets `SYSTEM_MANAGER_CONFIG = "verdin-imx95"` and builds the `imx-sm-toradex` fork (read from source, [imx95-verdin-sm-config.md](imx95-verdin-sm-config.md) §2.1). The `mx95evk` tables below are therefore NXP's, not the board's.

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

### Observed on the board (2026-11-27)

| Finding | Basis |
| --- | --- |
| Linux holds the DPU (`nxp,imx95-dpu`, `okay`, `imx95-dpu` DRM device on minor 0, fb0), the LDB (`fsl,imx95-ldb`, channel 0 `okay`), the pixel link and the DISPLAYMIX CSRs. Weston was active before and after the snapshot | observed (2026-11-27) |
| Linux scans out and runs on the display power domain (`display` `on`, DPU `active`), and the DISP AXI (800 MHz), APB (133 MHz) and OCRAM (400 MHz) clocks are enabled | observed (2026-11-27) |
| The AP agent sees SCMI protocols 0x11-0x15, 0x19, 0x81, 0x84 only. This agrees with NXP's DT. It says nothing about the M7 agent's rights | observed (2026-11-27) |
| SM identity: SCMI v2.1, vendor `NXP:IMX`, firmware version 0x23e. The version does not name a cfg. That the board's cfg is `mx95evk` is **not** shown | observed (2026-11-27) |
| **Agreement with the source:** Linux holds every display resource that `mx95evk` gives to LM2. **Not tested:** that the M7 LM lacks them, since no M7 agent was reachable | observed (2026-11-27) + read from source |

Conclusion (inferred): under `mx95evk` and `mx95evkrpmsg`, the A55/Linux LM owns the DPU, the LVDS and DSI routes, their clocks and the display power domain. The M7 LM has no access. If the board's SM follows either config, the M7 cannot drive the display, and a DPU register read from the M7 should be refused by the TRDC (a bus fault). The fault is inferred from the ownership model; the TRDC programming itself was not read.

## 2. How M7 firmware is loaded on this board

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

### Observed on the board (2026-11-27)

| Finding | Basis |
| --- | --- |
| `remoteproc1` is `imx-rproc`, state `offline`, firmware `rproc-imx-rproc-fw` (the default name, as read). `remoteproc0` is the NPU. A `remoteproc` debugfs directory exists for both | observed (2026-11-27) |
| **Disagrees with the draft's inference** that the M7 would already run a container demo and Linux would attach: the state is `offline`, not detached/attached. So `IMX_SIP_RPROC_STARTED` did not report a running core. That the Toradex container holds no M7 image is inferred | observed (2026-11-27); inferred |
| `/sys/module/firmware_class/parameters/path` exists and is empty. Writability was not tested | observed (2026-11-27) |
| `/lib/firmware` has no M7, `cm7` or `imx*` file (only wifi, Mali, NXP, Marvell files) | observed (2026-11-27) |
| The kernel logged `imx-rproc is available` at 2.5 s. No attach or start line followed | observed (2026-11-27) |
| The SCMI node has no 0x80 (LMM) or 0x82 (CPU) protocol, as in S6. The start path therefore goes through TF-A, not through a Linux SCMI call | observed (2026-11-27) for the nodes; inferred for the path |
| The `m7` power domain is `on` and the `remoteproc-cm7` device's runtime state reads `unsupported`. The `m7` clock reads 800 MHz with an enable count of 0 | observed (2026-11-27) |

### Can Linux start a different M7 image at runtime?

The Linux side needs no persistent change (inferred):

1. `/sys/module/firmware_class/parameters/path` is a runtime parameter, so it can point at a directory under `/tmp`. This was not read in S6: it is the standard firmware loader, inferred.
2. `/sys/class/remoteproc/remoteprocN/firmware` is set to the file name, then `state` to `start`.

None of this writes the rootfs, the U-Boot environment or the eMMC boot partitions. Whether the start then works depends on the SM:

- Under `mx95evk`, AP-S has no `CPU_M7P` permission, and AP-NS has only `LMM_1 NOTIFY` (`.lmmPerms[1] = SM_SCMI_PERM_NOTIFY`, no `cpuPerms` for M7P). TF-A's `scmi_core_set_reset_addr`/`scmi_core_start` would be refused, and so would `scmi_core_stop` (inferred from S1 and S4; the SM's permission check in the CPU protocol handler was not read).
- Under `mx95evkrpmsg`, AP-S has `CPU_M7P ALL`, so the same calls are allowed (inferred, same caveat).
- `mx95evkrpmsg` sits under `configs/other`, and the S2 recipe has no variant for it. S2's NXP Verdin image builds `mx95evk` and boots the M7 from the container. Under that config, Linux can only attach to the running M7; it can neither stop it nor load a new image (inferred).

## 3. What Linux holds

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

### Observed on the board (2026-11-27)

| Node | Board | Compared with S6 |
| --- | --- | --- |
| DPU | `display-controller@4b400000`, `nxp,imx95-dpu`, `okay`; power domain index 13 (display); clocks `pix apb axi ocram ldb ldb_vco`; a long `interrupts` list (the line was cut) | agrees |
| LDB | `syscon@4b0c0000` (`fsl,imx95-dispmix-lvds-csr`) with `ldb@4` (`fsl,imx95-ldb`) `okay`, `channel@0` `okay`, `channel@1` present | agrees: channel 0 |
| Pixel link | `bridge@8` in `syscon@4b010000`, `nxp,imx95-dc-pixel-link`, `okay` | agrees |
| MIPI DSI | `dsi@4acf0000` `disabled` | agrees |
| Panel | `panel-lvds-native`, `compatible = panel-lvds`, with `panel-timing`, `data-mapping`, `width-mm`, `height-mm`; backlight `pwm-backlight` (`backlight-lvds-native`) | **Disagrees with S6**: NXP's Verdin file names `logictechno,lt170410-2whc`. The board uses the generic `panel-lvds` with its own timing. Whether the panel is a Toradex 10.1" part is not stated in these nodes |
| Root nodes | `remoteproc-cm7` and `rpmsg-lifecycle` (`nxp,rpmsg-lifecycle`) are at the DT root, not under `/soc` | Where S6 puts `cm7` was not recorded in this document, so no comparison is made. `rpmsg-lifecycle` has no counterpart in the files read |
| M7 rproc | `fsl,imx95-cm7`; `mboxes` three entries (`tx`, `rx`, `rxdb`) on one mailbox phandle; **`power-domains` present**; `memory-region` seven phandles; `fsl,startup-delay-ms` = 50 (0x32). No `firmware-name` | S6 as recorded above lists six regions and no power domain; the board has a power domain and seven regions, so the board differs from that record |
| Reserved memory | `vdev0vring0` 0x88000000, `vdev0vring1` 0x88008000, `vdev1vring0` 0x88010000, `vdev1vring1` 0x88018000 (0x8000 each), `vdevbuffer` 0x88020000 (1 MiB, `shared-dma-pool`), `rsc-table` 0x88220000 (4 KiB), all `no-map`. Also `memory@80000000` (16 MiB, `no-map`), `memory@9c300000` (1 MiB, shared-dma-pool), `vpu_boot` 0xa0000000, `optee_core` 0x8c000000 (32 MiB), `optee_shm` 0x8e000000, `npu-dma`, and `linux,cma` 0x3c000000 bytes in 0x80000000 + 0x7F000000 | agrees with S6 for the vdev and CMA entries. The `memory@80000000` region resembles the `m7@80000000` of the `-rpmsg` variant; that it is the seventh `memory-region` entry is inferred (phandles were not mapped) |
| SCMI | `arm,scmi`, protocols 0x11, 0x12, 0x13, 0x14, 0x15, 0x19, 0x81, 0x84; four mailbox entries; optee also present | agrees |
| Clocks | `ldbpll_vco` 2986.2 MHz and `ldbpll` 497.7 MHz enabled; `ldb_pll_div7` 71.1 MHz enabled (the DPU's `ldb` clock); `lvds_ch0_gate` and `lvds_di0_gate` enabled, channel 1 gates off; `disp1pix` 24 MHz with enable count 0 | observed (2026-11-27); the 71.1 MHz LDB clock is read as the pixel clock actually driving channel 0 (inferred). The LDB rates equal those in S6's 10" file |
| Power domains | `disp` on; `display` on (DPU `active`); `camera` on; `m7` on | observed (2026-11-27) |
| DRM | `[drm] Initialized imx95-dpu 1.0.0 ... on minor 0`, then `fb0: imx95-dpudrmfb`. One dmesg line: `imx95-ldb ... Failed to create device link (0x180) with bridge@8` (the display still came up) | observed (2026-11-27) |

The board's Toradex DT sources (S9, S10) were not read. These rows are the board's own tree and replace the earlier "unknown" for the items listed. The panel's part number and timing values remain open.

## 4. Can the M7 own the DPU/LVDS route, alongside Linux or instead of it?

### Alongside Linux

Not without Linux giving it up (inferred).

- The SM can split resources only by its cfg lines (`DC0`, `DC1` and the `DC_*` sub-blocks are separate entries). The DPU's fetch units, LayerBlends and scalers, however, are one pool shared by both streams, and Linux's `dpu95` driver manages that pool across both CRTCs (`docs/imx95-dpu-source-6.6.md`).
- Giving one stream (`DC1` plus its FrameGen) to the M7 while Linux keeps `DC0` would need a cfg that no NXP release provides. It would also need a `dpu95` driver that leaves part of the pixel engine alone, which the 6.6 driver does not do.
- The `SM_SCMI_PERM_EXCLUSIVE` "display hand-over" mention in `config.md` covers SCMI permissions. It is not known to cover register (TRDC) access.

Observed (2026-11-27) in support: one DPU, one `ldb` with both channels in the same node, one pixel link and one display power domain are all held by one Linux driver instance (`imx95-dpu`, minor 0). The board's panel sits on channel 0 only, so a split between the two streams would leave channel 1 free, but nothing observed shows it could be given to another core.

### Instead of Linux

Possible in principle (inferred). It would take every one of these:

1. **SM cfg change:** move `DC`, `DC0`, `DC1`, all `DC_*`, `LVDS`, `BLK_CTRL_DISPLAYMIX`, `VIDEO_PLL1` and the display irqsteer region (location not confirmed) to LM1. Grant the M7 agent `CLOCK_DISP1PIX`, `LDB_PLL`, `PD_DISPLAY` and `PERF_DISP`. Give AP-S `CPU_M7P` (and `PD_M7`) if Linux should still be able to restart the M7. Rebuild the SM firmware.
2. **New boot container:** the SM firmware lives in the `imx-boot` container (S2, S3). A new SM means writing the boot container, which on this board is in the eMMC boot partitions. **The board rules forbid that.**
3. **Linux DT overlay:** disable `&dpu`, `&displaymix_irqsteer`, `&pixel_interleaver`, `&display_pixel_link`, `&ldb`, `&ldb0_phy`, the panel and the backlight in Linux. Otherwise Linux probes hardware it no longer owns. Loading such an overlay at boot needs a U-Boot env or `/boot` change, which the board rules also forbid.
4. **Zephyr driver work:** upstream Zephyr has no i.MX95 LDB/LVDS driver (next section). The Verdin's panel is LVDS.

What the observation adds (2026-11-27): the M7's clock and power-domain entries exist on the board, and an `imx-rproc` device exists, so Linux is wired for an M7 that the Toradex image does not start. That is still no sign that the SM gives the M7 any display resource.

Alternative: a single-LM config in the style of `mx95alt` (M7 and A55 in one LM) also lets the M7 reach the display, but with no isolation from Linux. It needs the same new SM firmware and boot container (inferred).

## 5. Scanout and vblank/latch path the Zephyr driver would use

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

Observed (2026-11-27): on this board the DPU is not available to a Zephyr driver. It is held by Linux with the display powered, and its interrupts are routed through Linux's tree. Which irqsteer lines reach the M7 was not observed (`/proc/interrupts` was not read), and the DPU register map was not read at all.

For the engine's needs (SoW §5.1: scanout and vblank/latch first):

- The stock driver gives one plane and a blocking "latched" point with no timestamp or frame counter.
- A Zephyr backend would need a non-blocking commit, a latch event with a frame number, and a timeout. It would get these from the ExtDst shadow-load interrupt plus a FrameGen counter or interrupt. The Linux DT names `framegen*` interrupts in the same irqsteer, which shows they exist.
- It would also need an LDB driver for this panel, and overlay support (more fetch units and LayerBlends).

All four points are inferred.

## 6. What is blocked, with evidence

| # | Blocker | Evidence | Basis |
| --- | --- | --- | --- |
| B1 | The board's SM configuration is unknown, so it is unknown whether the M7 may use the display or may be started and stopped from Linux | The only SM facts observed are `SCMI Protocol v2.1 'NXP:IMX' Firmware version 0x23e`, MISC 1.1, BBM 1.0. No cfg name or build banner is printed. The Toradex sources (S8) were not readable | observed (2026-11-27) for the version; read from source for S8 |
| B2 | Linux owns and uses the display now | `dpu` and `ldb` channel 0 `okay`, `imx95-dpu` bound with fb0, display power domain on and `active` for the DPU, display AXI/APB/OCRAM clocks and the LDB PLL enabled, Weston active | observed (2026-11-27) |
| B3 | Neither NXP's cfg nor any cfg read here gives the display to an M7 LM that runs beside Linux | `mx95evk`: LM2 (Linux) has every display resource; `mx95alt` is the only cfg where the M7 reaches it and it shares one LM with the A55 cores | read from source (S1) |
| B4 | Moving the display needs an SM cfg rebuild, a new boot container and a Linux DT change. Each is a write to the eMMC boot area, the U-Boot environment or `/boot` | `docs/imx95-board.md` forbids these writes | read from source (S1-S3) for the artefacts; the rule is the board's |
| B5 | Zephyr has no LVDS/LDB driver; its only DPU route is DSI | no `ldb`/`lvds` driver, binding or node in S7. The board's panel is `panel-lvds` on LDB channel 0; its DSI node is `disabled` | read from source (S7); observed (2026-11-27) for the board |
| B6 | The stock Zephyr DPU driver is one plane with a blocking commit, no timestamp or frame counter, and a hard-coded DPU base in the ISR | see section 5 | read from source (S7) |
| B7 | No Zephyr board or devicetree for the Verdin iMX95 exists upstream; the M7 console UART on Mallow is not known | `boards/toradex/` has no iMX95 board | read from source (S7); UART unknown |
| B8 | Whether `imx_rproc` can start an M7 image at all is not shown | state is `offline`; no evidence of `CPU_M7P` for the AP agent (section 8) | observed (2026-11-27); evidence absent |

### Open questions: settled by the observation, and still open

| Question from the draft | Answer | Basis |
| --- | --- | --- |
| Is there an imx rproc, and in which state? | Yes, `remoteproc1`, name `imx-rproc`, state `offline`, firmware `rproc-imx-rproc-fw`. `remoteproc0` is the NPU (`neutron-rproc`) | observed (2026-11-27) |
| Is there an `imx95-cm7` node? | Yes, `remoteproc-cm7` at the DT root, `fsl,imx95-cm7`, with a power domain; see section 3 | observed (2026-11-27) |
| `m7@80000000` carveout? | Not by that name; a `memory@80000000` 16 MiB `no-map` region exists | observed (2026-11-27) |
| SCMI 0x80/0x82 protocols for Linux? | No. Protocols 0x11, 0x12, 0x13, 0x14, 0x15, 0x19, 0x81, 0x84 | observed (2026-11-27) |
| Status of `dpu`, `ldb`, pixel link, `mipi_dsi`; panel | `dpu`, `ldb`, `channel@0`, pixel link `okay`; DSI `disabled`; panel `panel-lvds` | observed (2026-11-27) |
| SM/SCMI version | SCMI v2.1, vendor `NXP:IMX`, firmware 0x23e | observed (2026-11-27) |
| Display clocks and power domains | On under Linux (section 3) | observed (2026-11-27) |
| Is the M7 already running a container demo? | Not reported: `offline`. That the container holds no M7 image is inferred | observed (2026-11-27); inferred |
| Is `firmware_class` path writable? | Only its existence and empty value were read; writability was not tested | observed (2026-11-27) for the value; open for writability |
| Which SM config does the board run, and does AP-S have `CPU_M7P`? | **Open** | |
| Which DID the DPU masters under, and are TRDC rules in place? | **Open** | |
| Do M7 NVIC 215 and the A55 GIC SPIs come from the same irqsteer? `/proc/interrupts` was not read | **Open** | |
| SoC revision | **Open** (not read) | |
| Panel part number and timing values; pixel clock needed | **Open** (the panel node has `panel-timing`, values not captured) | |
| M7 UART on Mallow | **Open** | |

## 7. Proposed M2 items

Related design notes in this repository: `docs/imx95-sm-display-lm.md`, `docs/imx95-m7-fb-memory.md`, `docs/imx95-m7-overlay-design.md`, `docs/imx95-m7-backend-contract.md`, `docs/imx95-m7-scmi.md`, `docs/imx95-m7-verdin-bsp.md`, `docs/m2-comparison-plan.md`. They are not re-checked here.

1. **Settle ownership.** Read S8 to S10 where `git clone` from git.toradex.com works, to find `SYSTEM_MANAGER_CONFIG`, whether the container holds an M7 image and whether the cfg gives AP-S `CPU_M7P`. Optionally read the SM boot banner on the console of the board, with a power cycle only when no other item holds it. Decides B1 and B8.
2. **Ownership probe from the M7**, only if the gate (section 8) turns to met: build a TCM image, read DPU and display blk-ctrl registers behind a fault handler, query SCMI `PD_DISPLAY` as the M7 agent. Expected under `mx95evk`: refused or fault (inferred).
3. **Scanout.** Two routes, depending on 2 and on a board the project may reflash:
   - If the M7 can reach the DPU: scanout from the M7 on a DSI panel through the stock driver (the `nxp_mx9_dsi_oled` shield route on the EVK). This is a different panel from the Verdin's LVDS one.
   - On the Verdin's LVDS panel: write an LDB driver for Zephyr, a new SM cfg, a boot container and a Linux DT with the display disabled. All of it needs a reflashable board.
4. **VSYNC/latch reporting.** Make the commit non-blocking with a timeout; report the ExtDst0 shadow-load latch with a FrameGen frame counter and a timestamp; read the DPU base from the devicetree. Check the result against the Linux flip-done on the same scene.
5. **Overlay or blocker report.** Only after scanout and latch work. A second fetch unit and a LayerBlend need a multi-plane driver. If 2 shows Linux ownership and no reflashable board is granted, the deliverable is this document's section 6 as the blocker report plus the reduced-scope bring-up (an M7 that runs without the display).
6. **Linux vs Zephyr comparison of the same scene** (latch time, frame count, missed latches) with dated measurements, as `docs/m2-comparison-plan.md` sets out. Measurements on a different SoC are not a substitute.

## 8. Gate: can an M7 image be started from Linux remoteproc with files only under /tmp?

Gate: not met. The core is `offline` (observed 2026-11-27), but nothing shows that the System Manager lets the Linux side start or stop `CPU_M7P`. The Toradex SM configuration was not read, and NXP's default `mx95evk` refuses it (read from source).

The decision rule from the draft: yes only if the M7 remoteproc reports `offline` and there is evidence that the SM config grants the M7 start/stop right. Check by check:

| Check | Result | Basis |
| --- | --- | --- |
| (a) a `remoteproc` named `imx-rproc` | Met: `remoteproc1` | observed (2026-11-27) |
| (b) state `offline` rather than `attached`/`running` | Met | observed (2026-11-27) |
| (c) evidence that the SM grants `CPU_M7P` to the agent TF-A uses | **Not met.** Only the SM version was observed; no cfg name or permission table. The Linux SCMI node has no 0x82 protocol, and does not need one, because `imx_rproc` reaches the CPU protocol through TF-A | observed (2026-11-27) for the version; read from source (S1, S4, S6) for the path |
| Linux side writes nothing persistent | Plausible: the `firmware_class` path (read as empty) and the remoteproc `firmware`/`state` attributes are runtime settings. Writability of the path was not tested | observed (2026-11-27) for the path value; inferred for the rest |
| No M7 image installed | `/lib/firmware` has no M7 or `imx*` file, so a file would have to come from a search path under `/tmp` | observed (2026-11-27) |

What the offline state means for risk (inferred): a refused `start` leaves the core as it was. Nothing is running that would have to be stopped, so the draft's hazard of stopping an attached core does not apply. The remaining risk is a `start` that succeeds with a bad image, followed by a refused `stop`: the M7 would then run until the next reset, and the farm has no remote power control for this board.

What turns the answer into met: evidence for (c) from item 1 of section 7, or a decision by the planner that a single `start` attempt of a harmless image on the offline core is itself the evidence. That decision is not taken here.

Even if met, only an M7 hello and an ownership probe follow. It does not put a Zephyr image on the display. The power domain `m7` shows `on` and the `m7` clock 800 MHz with an enable count of 0 (observed 2026-11-27). Whether the core itself is held in reset was not observed.

### M7 run (skipped 2026-11-28)

Not run. The gate above reads "not met": check (c), evidence that the System Manager lets the Linux side start and stop `CPU_M7P`, is still open, and no planner decision has accepted a single `start` attempt as that evidence. The m7-run item therefore left the board untouched: no ssh session, no copy to `/tmp`, no write to `firmware_class.path` or to the remoteproc `firmware`/`state` attributes, and no build of `tests/board/imx95_m7` for the run. The values last observed (2026-11-27, section 2 and the table above) still stand: `remoteproc1` (`imx-rproc`) `offline`, `firmware_class.path` empty. Nothing was changed, so no restore check was needed.

To run it later: close check (c) (item 1 of section 7), or record a planner decision that accepts one `start` attempt on the offline core as the evidence. Then follow "Load procedure (Linux remoteproc)" in [tests/board/imx95_m7/README.md](../tests/board/imx95_m7/README.md).

### m7-run2 outcome: remoteproc load skipped (2026-11-28)

Not run; nothing was loaded. The item was gated on a "Gate verdict for m7-run2" section that the sm-config-read item was to add to [imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md). That section does not exist at the integration tip this item started from. sm-config-read could not read the Toradex Verdin SM configuration (its section 3.3 lists the Toradex-shipped SM config and the BSP 7.8.0 SRCREVs as still unread), and its handoff says to treat the gate as no-go. Check (c) of section 8, evidence that the System Manager lets the Linux side start and stop `CPU_M7P`, therefore stays open, and no planner decision accepts a single `start` attempt as that evidence.

What was done on the board: one read-only ssh look, 2026-11-28 06:59 UTC (observed). No file was copied, and no sysfs attribute or other setting was written.

```
$ uname -r
6.6.157-7.8.0
$ systemctl is-active weston
active
/sys/class/remoteproc/remoteproc0 name=neutron-rproc state=offline fw=rproc-neutron-rproc-fw
/sys/class/remoteproc/remoteproc1 name=imx-rproc state=offline fw=rproc-imx-rproc-fw
/sys/module/firmware_class/parameters/path = (empty)
```

New against the 2026-11-27 observation (observed): `remoteproc1` (`imx-rproc`) has the default firmware name `rproc-imx-rproc-fw`, and there is a second remoteproc, `remoteproc0` (`neutron-rproc`, the NPU core, inferred from the name), also `offline` with its default name. A loader must select the M7 core by `name`, not by index.

Post-run checks (observed in the same session; nothing was changed, so nothing had to be restored): `weston` is `active`; `remoteproc1` is `offline`; `firmware_class.path` is empty, as before; no files were left under `/tmp`, because none were written. `tests/board/imx95_m7` was not built for this item, and no Verdin overlay was added.

To run it later: the same condition as in "M7 run (skipped 2026-11-28)" above applies. Either a re-run of sm-config-read records a go verdict from the Toradex SM config, or the planner records that one `start` attempt on the offline core is accepted as the evidence.
