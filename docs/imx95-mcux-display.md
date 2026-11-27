# NXP MCUXpresso SDK display examples for the i.MX95 M7 (source study)

Read on 2026-11-28. No board was accessed. Nothing here was measured.

Evidence for SoW §5.1 (M2): does NXP's own non-Zephyr M7 software drive the
i.MX95 DPU and the LVDS/MIPI-DSI outputs, and with which System Manager (SM)
setup and boot flow?

## How this was read, and its limits

- `git clone` and `git ls-remote` were refused on the node that wrote this
  (credential helper error), so **nothing was cloned**. All files were read
  through a web-fetch agent on the GitHub web UI and
  `raw.githubusercontent.com`, pinned to the commits below.
- Fetched pages were partly condensed by a model. Rows say **verbatim** where
  the fetched text was quoted as-is and **summary** where only a condensed
  reading was available. Summary rows should be re-read from a clone before
  anything is built on them.
- Tag legend: **read from source (`<repo>@<commit>`)** or **inferred**.
  A file that was not opened is listed as *not read*, never guessed.

## Pinned sources

| Id | Repository | Pin | URL |
|---|---|---|---|
| EX | nxp-mcuxpresso/mcuxsdk-examples | `cc8974f843f496ac0ec959c55164766fd0d489bd` (main, 2026-11-23, merge into `tmp_release/26.12.00-pvw2`) | https://github.com/nxp-mcuxpresso/mcuxsdk-examples/tree/cc8974f843f496ac0ec959c55164766fd0d489bd |
| CORE | nxp-mcuxpresso/mcuxsdk-core | `19b4cd9c4151edd6165306c82e750b06a9a4c54b` (main, 2026-11-23) | https://github.com/nxp-mcuxpresso/mcuxsdk-core/tree/19b4cd9c4151edd6165306c82e750b06a9a4c54b |
| MAN | nxp-mcuxpresso/mcuxsdk-manifests | `56f645e06bbc360920da66e964a4c449d0d2c390` = tag `v26.12.00-pvw2_2` (2026-11-24) | https://github.com/nxp-mcuxpresso/mcuxsdk-manifests/tree/56f645e06bbc360920da66e964a4c449d0d2c390 |
| DEV | nxp-mcuxpresso/mcux-devices-imx | `55088d0cc88e2dd0c71bb7208aaf8439915560f5` (pinned by MAN `submanifests/devices/i.MX.yml`) | https://github.com/nxp-mcuxpresso/mcux-devices-imx/tree/55088d0cc88e2dd0c71bb7208aaf8439915560f5 |
| REL | mcuxsdk-manifests tag `v25.09.00` (2025-09-19) release page | tag `v25.09.00` | https://github.com/nxp-mcuxpresso/mcuxsdk-manifests/releases/tag/v25.09.00 |

Board directory used below, `B` =
`EX/_boards/imx95verdinevk`. There is **no `evkmimx95` directory**; the i.MX95
boards in EX are `imx95verdinevk`, `imx95lpd5evk19`, `imx95lp4xevk15`,
`frdmimx95`, `frdmimx952`, `imx952evk`
(https://github.com/nxp-mcuxpresso/mcuxsdk-examples/tree/cc8974f843f496ac0ec959c55164766fd0d489bd/_boards).
Each of the first three has `ca55`, `cm33` and `cm7` subdirectories.

**BSP match.** No mapping from an SDK tag to NXP BSP 6.6.157-7.8.0 was found
in any page read (read from source, MAN tags pages). The Verdin board entry
appears as "IMX95VERDINEVK", quality "Early Access Release (EAR)" in REL
(summary). `B/ChangeLog_board.md` lists only "[25.06.00] Initial version"
(summary). The SDK that matches the Toradex 7.8.0 image therefore **was not
established**; the SDK read here is newer than anything the board's Linux
image is known to pair with. Release notes
(`https://mcuxpresso.nxp.com/mcuxsdk/25.09.00/html/boards/i.MX/imx95verdinevk/releaseNotes/rnindex.html`)
were *not read*.

## Answer in one paragraph

Yes, NXP ships M7 bare-metal (non-Zephyr) DPU examples for the Verdin iMX95
board, and they drive the DPU with MIPI-DSI or LVDS (LDB) output from the
Cortex-M7, with fixed DDR framebuffers and shadow-load interrupts through
IRQSTEER (read from source, EX and CORE). They get clocks, power and pin
control from the SM through a HAL (`sm/hal_config.h`: `SM_CLOCK=1`,
`SM_POWER=1`, `SM_PINCTRL=1`). They are started by a J-Link debugger, not by
Linux. **No document was found that names an SM configuration or logical
machine they assume**, so that part is open (see below).

## Rows

### Which examples exist

| Finding | Tag | Source |
|---|---|---|
| Verdin M7 DPU examples: `driver_examples/dpu/{fetch_yuv, figure_display, rop, warp, linker}` with `cm7/` subfolders; `reconfig.cmake` replaces the default `MIMX9596xxxxN_cm7_ram.ld` with a board linker script | read from source (EX) | `B/driver_examples/dpu` |
| Shared DPU examples `blitblend, character, fetch_decode, fetch_layer, palette, rop, signature, warp` plus `display_interface.c` | read from source (EX) | `EX/driver_examples/dpu` |
| No lvgl, fbdev, pxp or stand-alone LDB/DSI example in the Verdin board tree; LDB and DSI appear only as options inside `display_support` | read from source (EX) | `B/demo_apps` (only `hello_world`, `power_mode_switch`, `sai_low_power_audio`) |
| Generic `display_examples/` (fbdev_freertos, mipi_dsi_compliant_test, ...) exist at EX root; whether any is enabled for i.MX95 was *not read* | read from source (EX), enablement not read | `EX/display_examples` |
| `figure_display` has only `cm7/{app.h, hardware_init.c, pin_mux.*}`, a readme and `reconfig.cmake` in the board tree; its main source was **not found**. `fetch_decode/dpu_fetch_decode.c` was read as the nearest generic example, but that `figure_display` builds it is **not verified** | read from source (EX) / inferred | `B/driver_examples/dpu/figure_display`, `EX/driver_examples/dpu/fetch_decode/dpu_fetch_decode.c` |
| Panels in `display_support.h`: `DEMO_PANEL_MX9_DSI_OLED` (13, default, RM692C9 1080x2340), `MIPI2HDMI` (14, ADV7535 1920x1080), `CAP_TOUCH_LVDS` (16, 1280x800), `CAP_TOUCH_DSI` (17, 1280x800 via SN65DSI83). Default interface `DPU_DI_MIPI`; with `DPU_DI_LVDS` display index is 1 and the output is LDB | read from source (EX) | `B/display_support.h` |
| The Toradex 10.1" LVDS panel of this project is **not** one of the listed panels; `CAP_TOUCH_LVDS` is the closest, with panel identity not read | inferred | `B/display_support.h` |

### Drivers and DPU units

| Finding | Tag | Source |
|---|---|---|
| Verdin `prj.conf` enables `driver.dpu_1`, `driver.ldb_1`, `driver.mipi_dsi_imx`, `driver.lpi2c`, `driver.rgpio`, expanders and panel drivers | read from source (EX) | `B/driver_examples/dpu/prj.conf` |
| `drivers/dpu_1/fsl_dpu.h` (Copyright 2023-2026): `DPU_Init(DISPLAY_SEERIS_Type *)`; units FetchDecode9, FetchRot9, FetchEco0-2, FetchLayer0-1, FetchYuv0-3, H/V scalers, LayerBlend1-6, BlitBlend9, Rop9, ConstFrame0/1/4/5, ExtDst0/1/4/5, Store9, DomainBlend0/1, FrameGen0/1, Sig0-2. Unit list cross-checked against the verbatim enum; other lines are summary | read from source (CORE) | `CORE/drivers/dpu_1/fsl_dpu.h` |
| `drivers/dpu/` is the older Iris-MVPL variant (Copyright 2017-2020, `IRIS_MVPL_Type`); not the one the Verdin `prj.conf` selects | read from source (CORE) | `CORE/drivers/dpu/fsl_dpu.h` |
| `fsl_dpu.c` (either variant), `fsl_ldb.[ch]`, `fsl_mipi_dsi.[ch]`, `fsl_dpu_irqsteer.h` exist but were **not read** | read from source (CORE), not read | `CORE/drivers/{dpu_1,ldb_1,mipi_dsi_imx,dpu_irqsteer}` |
| `dpu_fetch_decode.c` init order: `DPU_Init`, `DPU_PreparePathConfig`, `DPU_InitDisplayTiming`, `DPU_SetDisplayConfig` (`kDPU_DisplayPrimOnTop`), `DPU_TriggerDisplayShadowLoad`, `APP_InitPixelLink`, `APP_StartDisplayInterface`, `DPU_StartDisplay` | read from source (EX) | `EX/driver_examples/dpu/fetch_decode/dpu_fetch_decode.c` |
| Units used there: content stream FetchDecode0 + VScaler4/HScaler4 + ConstFrame0 + LayerBlend0 + an ExtDst; safety stream FetchDecode1 + VScaler5/HScaler5 + ConstFrame4 + LayerBlend2 + an ExtDst. DPR/PRG prefetch only if `APP_DPU_USE_PREFETCH` | read from source (EX) | same file |
| A separate display-controller IP driver `drivers/dcif` / `dcif_1` (DCIF, two or four layers, `kDCIF_Vsync*` interrupts, shadow load on next VSYNC) exists in CORE; **no evidence it is used for i.MX95**, not tied to MIMX9596 in anything read | read from source (CORE), applicability not verified | `CORE/drivers/dcif/fsl_dcif.h` |

### Clocks, power, SM

| Finding | Tag | Source |
|---|---|---|
| Clock, power and pinctrl go through a HAL, not driver-level `CLOCK_*` calls: `HAL_ClockSetPllClk`, `HAL_ClockEnable`, `HAL_ClockSetRootClk`, `HAL_ClockGetIpFreq`, `HAL_PowerSetState`, `SM_Platform_Init` | read from source (EX) | `B/display_support.c`, `B/driver_examples/dpu/figure_display/cm7/hardware_init.c` |
| `B/sm/hal_config.h` (read whole): `SM_CLOCK 1`, `SM_POWER 1`, `SM_PINCTRL 1`, `CCM_CLOCK 0`, `CCMSRCGPC_POWER 0`, `IOMUXC_PINCTRL 0`. `B/nonsm/hal_config.h` is the inverse (M7 drives CCM/GPC/IOMUXC directly). Board `prj.conf` says "sm option2 now is for IMX95 CM7" | read from source (EX) | `B/sm/hal_config.h`, `B/nonsm/hal_config.h` |
| So the display build for the SM flow requests clocks and power from the SM over SCMI via that HAL; that the HAL uses SCMI is **inferred** from `scmi.h`/`scmi_cpu.h` includes and the `SM_*` naming. `sm_platform.h`, `hal_clock`, `hal_power`, agent id, MU base and channel were **not found** (not in CORE `components/`, which 404s; probably another manifest repo, not checked) | inferred; sources not read | `B/cm7/app_srtm.c` includes |
| Display bring-up order: `SM_Platform_Init`, display MIX power on, camera MIX power on (DSI/CSI PHY registers live there), `BOARD_ConfigMPU`, pins, `BOARD_BootClockRUN`, console, root clocks, `BOARD_PrepareDisplay` | read from source (EX) | `hardware_init.c` |
| Display root clocks requested: `dispapb` from SYSPLL1 DFS1/2 (div 3, comment 133 MHz), `dispaxi` from SYSPLL1 DFS2 (800 MHz), `dispocram` (div 2, 333 MHz), pixel clock `disp1pix` from VIDEOPLL1 (OLED: 4008 MHz VCO, 446.33 MHz, div 3), LDB path `ldbpllctl` 2986.2 MHz / `ldbpll` 497.7 MHz (the board file; the LPD5 board uses 4158 / 1039.5 MHz for the IT6263 bridge) | read from source (EX) | `B/display_support.c`, `hardware_init.c`, `imx95lpd5evk19/display_support.c` |
| Direct register writes by the app: `DISPLAY__BLK_CTRL_DISPLAYMIX->PIXEL_LINK_CTRL`, `DISPLAY_ENGINES_CLOCK_CONTROL` (LDBPLL/7 select), `CAMERA__DSI_MASTER_CSR`, `CAMERA__DSI_OR_CSI_PHY_CSR` combo PHY | read from source (EX) | `B/display_support.c` |
| **SM configuration / logical machine the examples assume: not found.** No file read names an SM config (e.g. an EVK or Verdin config), an LM id for the M7, or says that the DPU/LDB resources must be granted to the M7's LM. The only LM reference read is `SM_PLATFORM_LMID_A55` in `app_srtm.c`, used to detect A55 suspend/reset notifications | read from source (EX); absence | `B/cm7/app_srtm.c` |
| It is therefore **inferred** that the examples need an SM image whose M7 LM may use the DISPLAY and CAMERA MIX power, the display clock roots and the DPU/IRQSTEER (output "display 0 B ... duplicate sent to Cortex-M"), and that this is satisfied on NXP EVKs by an SM config that is not part of EX. Whether Toradex's SM config does the same is **not known** | inferred | — |

### Interrupts, VSYNC and latch

| Finding | Tag | Source |
|---|---|---|
| DPU interrupt reaches the M7 through IRQSTEER: handlers `DISP_IRQSTEER1/3/7_DriverIRQHandler` each call `DPU_IRQHandler()`; `BOARD_InitDpuInterrupt()` writes `DPU_IRQSTEER->CHN_MASK[13] = 0x10249` (display 0), `[9]` (display 1), `[1] = 0x7` (blit) and `EnableIRQ`s them. Comment: "INT_OUT [2] - display 0 B (a duplicate to be sent to Cortex-M core)" | read from source (EX) | `B/display_support.c` |
| VSYNC/latch signals: `kDPU_Group0ExtDst0ShadowLoadInterrupt` (content stream), `kDPU_Group0Display0ShadowLoadInterrupt` (display stream), `kDPU_Group0Store9FrameCompleteInterrupt` (blit complete); `dpu_1` header also has `DomainBlend0ShadowLoad/FrameComplete`, `DiSengcfgShadowLoad0` and FrameGen0 Int0..3 and `PrimSyncOn` | read from source (EX header macros; CORE enum) | `B/display_support.h`, `CORE/drivers/dpu_1/fsl_dpu.h` |
| Latch handling in the example: `APP_TriggerStreamShadowLoad` sets `isShadowLoadPending` and calls `DPU_TriggerPipelineShadowLoad`; `DPU_IRQHandler` reads `DPU_GetInterruptsPendingFlags`, the example clears the pending flag and `DPU_ClearInterruptsPendingFlags`; main loop updates a stream only when no shadow load is pending; interrupts enabled with `DPU_EnableInterrupts(APP_DPU, 0/1, mask)` | read from source (EX) | `dpu_fetch_decode.c` |
| DPU register bases used: `0x4B780000` (stream 0), `0x4B7A0000` (stream 1), blit `0x4B430000` | read from source (EX) | `B/display_support.h` |

### Framebuffer memory

| Finding | Tag | Source |
|---|---|---|
| Board linker script: M7 code `0x00001000` (252 KiB) and data `0x20004000` (104 KiB) in TCM; `m_ncache` at `0x80000000`, length 128 MiB, receives `NonCacheable` sections; resource table in DRAM at `0x88220000` | read from source (EX) | `B/driver_examples/dpu/linker/MIMX9596_cm7_ram.ld` |
| `figure_display` readme loads image binaries by J-Link `loadbin` at `0x80000000` and then a per-panel address (`0x809a3f80` OLED, `0x807e9000` MIPI2HDMI, `0x803e8000` both CAP_TOUCH panels): fixed DDR framebuffers | read from source (EX) | `.../figure_display/example_board_readme.md` |
| `dpu_fetch_decode.c` instead uses a static `AT_NONCACHEABLE_SECTION_ALIGN(uint32_t s_frameBuffers[streams][sublayers][2][H][W], 32)` ARGB8888 array, double-buffered per stream, i.e. a non-cacheable DDR placement by linker section | read from source (EX) | `dpu_fetch_decode.c` |
| M7 MPU in `board.c`: region 1 Device memory `0x80000000`/512 MB, region 14 non-cacheable `0x81000000`/1 MB (PDM), region 15 `0xC0000000`/256 MB (audio). Note the DDR window at `0x80000000` is set to Device, so the "NonCacheable" label is backed by MPU attributes, not by a separate cache setting | read from source (EX) | `B/board.c` |
| The DPU driver does not allocate: callers pass `baseAddr`/`strideBytes`; 32 bpp needs 4-byte alignment | read from source (CORE, summary) | `fsl_dpu.h` |
| DDR is initialised by the "ddr oei image" before the M7 touches it (comment in `rsc_table.c`) | read from source (EX) | `B/rsc_table.c` |

### Boot flow

| Finding | Tag | Source |
|---|---|---|
| Display examples are started from a **J-Link debugger** ("Launch the debugger in your IDE"); no U-Boot `bootaux` or Linux remoteproc text anywhere in the DPU examples' readmes | read from source (EX) | `.../figure_display/example_board_readme.md` |
| The only Linux-coexistence example read, `rpmsg_lite_pingpong_rtos_linux/remote`, builds with `-DSCMI_LMM_POWER_CHANGE_PROCESSED=1`, includes `rsc_table.c`, `app_srtm.c` and **imx95lpd5evk19** (not Verdin) paths; its readme says "Make flash.bin and burn it to imx95 ... Boot to linux", i.e. the M7 is started by the boot image, shared memory base `0xb8000000` | read from source (EX) | `EX/multicore_examples/rpmsg_lite_pingpong_rtos_linux/remote` |
| `rsc_table.c` is copied to TCM (`0x2001f000` NS) and to DRAM "for early M Core boot case" | read from source (EX) | `B/rsc_table.c` |
| How the Toradex image starts an M7 (U-Boot `bootaux`, Linux `imx_rproc` over SCMI LMM) is **not covered by NXP's SDK text read** and belongs to the sibling source studies | not found | — |

## What this tells us about Zephyr on the Verdin

1. **Hardware plausibility (read from source, EX/CORE).** The M7 can
   program the DPU, IRQSTEER, DSI/LDB and the display clocks and MIX power on
   this SoC family, with a fixed-address DDR framebuffer and a ShadowLoad
   interrupt as the latch signal. The DPU register bases, the IRQSTEER mask
   values and the unit set are a concrete checklist to compare against
   Zephyr's `display_socionext_dpu.c` and its devicetree.
2. **It does not show the Verdin's actual SM configuration allows it.** The
   examples carry no SM/LM requirement, and the Toradex Reference Multimedia
   image, with Linux (A55) owning the display and Weston running, is a
   different configuration from the J-Link debug flow assumed here. DPU/LDB
   ownership on the real board stays **open** and must come from the SM
   config study and the read-only board snapshot.
3. **It gives no parity claim.** These are NXP's drivers (`fsl_dpu`, HAL over
   SM), not Zephyr's. Passing here would say nothing about Zephyr's DPU
   driver, and Zephyr's upstream DPU/DSI nodes remain as recorded in
   `docs/platform-capabilities.md`.
4. **Clock/power reach is the main dependency.** The examples depend on an
   SM-mediated HAL for display PLLs, root clocks and MIX power. A Zephyr M7
   would need an equivalent SCMI client; whether upstream Zephyr has one for
   this board is for the `zephyr-scmi-m7` study.
5. **Coexistence is unproven.** Every DPU example claims both display
   engines and the whole display MIX; nothing read shows the M7 sharing the
   DPU with Linux. The M7 pingpong example shows only rpmsg coexistence.
6. **Memory.** The DDR window is mapped as Device memory by the NXP M7
   board code, and the DPU examples use fixed DDR addresses or a
   non-cacheable linker section. This is a hint for the M7 cache note
   (`m7-fb-cache`), not a measurement.

## Not done / open

- No clone: summary rows above should be re-verified from a checkout.
- Not read: `fsl_dpu.c`, `fsl_ldb.c`, `fsl_mipi_dsi.c`, `fsl_dpu_irqsteer.h`,
  `sm_platform.h` and the HAL sources, `B/nonsm`, `B/ca55`, `B/cm33`,
  `project_segments` beyond Kconfig, the `isi` example, the release notes,
  `mcux-devices-imx` MIMX9596 contents.
- No SDK tag matched to BSP 6.6.157-7.8.0; no Toradex-specific M7 display
  port beyond the NXP `imx95verdinevk` board was found.
- The SM configuration/LM the examples assume: **not found.**
