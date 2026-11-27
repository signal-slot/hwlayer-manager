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
| `B/sm/hal_config.h` (read whole): `SM_CLOCK 1`, `SM_POWER 1`, `SM_PINCTRL 1`, `CCM_CLOCK 0`, `CCMSRCGPC_POWER 0`, `IOMUXC_PINCTRL 0`. `B/nonsm/hal_config.h` is the inverse (M7 drives CCM/GPC/IOMUXC directly). Board `prj.conf` says "sm option2 now is for IMX95 CM7" | read from source (EX); `sm/hal_config.h` re-read verbatim from raw.githubusercontent.com at the pin on 2026-11-28 | `B/sm/hal_config.h`, `B/nonsm/hal_config.h` |
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
| M7 MPU in `board.c`: region 1 Device memory `0x80000000`/512 MB, region 14 non-cacheable `0x81000000`/1 MB (PDM), region 15 `0xC0000000`/256 MB (audio). Region 1 (base attribute) is device-like TEX=2/C=0/B=0; the "NonCacheable" label is backed by MPU region 10, and region 9 makes the first 64 MB cacheable under `USE_SDRAM` (see Line citations, corrections) | read from source (EX) | `B/board.c` |
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
6. **Memory.** The NXP M7 board code maps the DDR window with a
   device-like base attribute and conditional cacheable/non-cacheable regions
   (`board.c:521-580`), and the DPU examples use fixed DDR addresses or a
   non-cacheable linker section. This is a hint for the M7 cache note
   (`m7-fb-cache`), not a measurement.

## Line citations (re-read of 2026-11-28)

Files were re-fetched whole from `raw.githubusercontent.com` at the EX/CORE
pins above, and the lines below were taken from those copies (nothing was
run or built). Line numbers refer to the raw file at the pin. The earlier
"summary" rows for `display_support.c`, `dpu_fetch_decode.c` and
`fsl_dpu.h` are now re-read; corrections are listed after the table.

| Entry point | File (at pin) | Lines | Tag |
|---|---|---|---|
| `DPU_IRQHandler` extern; `DISP_IRQSTEER1/3/7_DriverIRQHandler` each call it | `B/display_support.c` | 43; 121-123, 127-129, 134-136 | read from source |
| `BOARD_InitDpuInterrupt`: `CHN_MASK[13]=0x10249`, `[9]=0x10249`, `[1]=0x7`, `EnableIRQ` of IRQSTEER1/3/7; "duplicate to be sent to Cortex-M" comment | `B/display_support.c` | 140-153; comment 110; called at 313 | read from source |
| HAL clock calls: VIDEOPLL1 / pixel root / MIPI PHY roots; LDB PLL | `B/display_support.c` | 275-285; 303-306 | read from source |
| Direct register writes: pixel link, combo PHY, `DISPLAY_ENGINES_CLOCK_CONTROL` | `B/display_support.c` | 160-164; 445-448; 465-469 | read from source |
| `SM_Platform_Init`, display and camera MIX power via `HAL_PowerSetState`, `BOARD_ConfigMPU`, root clocks, `BOARD_PrepareDisplay` | `B/driver_examples/dpu/figure_display/cm7/hardware_init.c` | 65; 46-72; 74; 98-101; 103 | read from source |
| `SM_CLOCK 1`, `CCM_CLOCK 0`, `SM_PINCTRL 1`, `SM_POWER 1`, `IOMUXC_PINCTRL 0` | `B/sm/hal_config.h` | 7-12 | read from source |
| `SM_PLATFORM_LMID_A55` (the only LM reference) | `B/cm7/app_srtm.c` | 275 | read from source |
| Init order: `DPU_Init` 578, `DPU_PreparePathConfig` 580, `DPU_InitDisplayTiming` 594, `DPU_SetDisplayConfig` 613, `DPU_TriggerDisplayShadowLoad` 615, `APP_InitPixelLink` 624, `APP_StartDisplayInterface` 626, `DPU_StartDisplay` 628; `main` 658 | `EX/driver_examples/dpu/fetch_decode/dpu_fetch_decode.c` | 578-628; 658 | read from source |
| Latch: `isShadowLoadPending` 150, `APP_TriggerStreamShadowLoad` 341-345, flag cleared 352, `DPU_EnableInterrupts` 550-551, `DPU_IRQHandler` 641-651 | same | as listed | read from source |
| Non-cacheable framebuffer array | same | 172 (`AT_NONCACHEABLE_SECTION_ALIGN`) | read from source |
| `DPU_DI_LVDS 0`, `DPU_DI_MIPI 1`, panel ids, default `DPU_EXAMPLE_DI`/`DEMO_PANEL`; display index 1 for LVDS else 0; DPU bases `0x4B430000` (blit), `0x4B780000` (display 0), `0x4B7A0000` (display 1) | `B/display_support.h` | 12-17, 21, 25; 37-42; 47-53 | read from source |
| Stream interrupt macros (ExtDst0/1/4/5, Display0/1, Store9 shadow load and frame complete) | `B/display_support.h` | 161-192 | read from source |
| `DPU_Init` decl 838; `DPU_EnableInterrupts` 880; `DPU_TriggerPipelineShadowLoad` 952; `DPU_GetInterruptsPendingFlags` 1415; `DPU_ClearInterruptsPendingFlags` 1435; `DPU_InitDisplayTiming` 1464; `DPU_TriggerDisplayShadowLoad` 1527 | `CORE/drivers/dpu_1/fsl_dpu.h` | as listed | read from source |
| Interrupt enums: Store9 frame complete 326, ExtDst0 shadow load 328, DomainBlend0 shadow load/frame complete 340-341, DiSengcfg shadow load0 343, FrameGen0 Int0-3 346-349, PrimSyncOn 383 | `CORE/drivers/dpu_1/fsl_dpu.h` | as listed | read from source |
| `DPU_Init` definition | `CORE/drivers/dpu_1/fsl_dpu.c` (4918 lines) | 1074 | read from source (only definition lines grepped; body not read) |
| No `DPU_IRQHandler` definition in `fsl_dpu.c`; it is the example's own function (`dpu_fetch_decode.c:641`) | `CORE/drivers/dpu_1/fsl_dpu.c` | none found | read from source (absence) |
| Verdin `prj.conf` driver list (`dpu_1` line 16, `ldb_1` 10, `mipi_dsi_imx` 4) | `B/driver_examples/dpu/prj.conf` | 1-17 | read from source |
| J-Link flow: "Launch the debugger" 32, `loadbin` at `0x80000000` 33, per-panel addresses 38-44 | `.../figure_display/example_board_readme.md` | 32-44 | read from source |
| `rsc_table.c` TCM `0x2001f000` (NS) comment | `B/rsc_table.c` | 97 | read from source |
| M7 MPU (ARMv7-M path): region 1 `0x80000000`/512 MB TEX=2, C=0, B=0 | `B/board.c` | 521-522 | read from source |
| Region 9 `0x80000000`/64 MB, cacheable (write-through or write-back), **only under `USE_SDRAM`** | `B/board.c` | 554-564 | read from source |
| Region 10 non-cacheable shareable for the `NonCacheable` linker section | `B/board.c` | 578-580 | read from source |
| Regions 14 (`0x81000000`, 1 MB) and 15 (`0xC0000000`, 256 MB), TEX=1 S=1, non-cacheable | `B/board.c` | 595-601 | read from source |
| ARMv8-M MPU path (attribute indexes, region 0 `0x80000000-0xDFFFFFFF`) | `B/board.c` | 640-699 | read from source |

Corrections found by this re-read:

- **`kDPU_Group0Display0ShadowLoadInterrupt` is not defined in
  `fsl_dpu.h` at the pin** (no match for `Display0ShadowLoad`). It is only
  *used* as a macro value in `display_support.h:169` (`Display1` at 183). Its
  definition is in a header not read (probably the device header in
  `mcux-devices-imx`, **inferred**). The "VSYNC/latch signals" row above is
  correct about use, but its "CORE enum" source applies only to the other
  names. Not read: where `Display0ShadowLoad` is defined.
- **DDR cacheability is conditional.** The earlier sentence that the DDR
  window at `0x80000000` is Device was too strong: region 1 is the base
  attribute (`board.c:521-522`, TEX=2 C=0 B=0; its architectural meaning as
  device-like is **inferred**), but region 9 (cacheable, higher number wins)
  overrides the first 64 MB when `USE_SDRAM` is defined (`board.c:554-564`),
  and region 10 marks the `NonCacheable` section. Whether `USE_SDRAM` is
  defined for the Verdin DPU build was **not read**. Point 6 above is read in
  this light.

Still not read: `fsl_dpu.c` bodies, `fsl_ldb.c`, `fsl_mipi_dsi.c`,
`fsl_dpu_irqsteer.h`, HAL and `SM_Platform_Init` sources, the device header
that defines `Display0ShadowLoad`, `B/nonsm`, `B/ca55`, `B/cm33`.

## Per-example summary of points (2) and (3)

All of this restates rows above (inferred only where stated).

| Example / layer | Resources it assumes the M7 owns | Alongside Linux? | Frame-done / VSYNC report |
|---|---|---|---|
| Verdin `driver_examples/dpu/*` (`figure_display` etc.) | Both DPU display streams, DISPLAY and CAMERA MIX power, display root clocks, IRQSTEER (read from source, EX); SM LM requirement not found | Standalone, J-Link start (read from source, EX); no Linux sharing shown (inferred from absence) | ShadowLoad interrupts via IRQSTEER (read from source, EX) |
| `fetch_decode` (generic) | Same, via `APP_DPU`; DPR/PRG prefetch optional | Standalone (inferred) | `kDPU_Group0ExtDst0ShadowLoadInterrupt` / `Display0ShadowLoad`, pending flag cleared in handler (read from source, EX) |
| `drivers/dpu_1`, `ldb_1`, `mipi_dsi_imx` | Take a base pointer; no ownership logic (inferred; `fsl_dpu.c`, `fsl_ldb.c`, `fsl_mipi_dsi.c` not read) | n/a | Interrupt enum only; no VBLANK abstraction read |
| `drivers/dcif*` | Unknown for i.MX95 | not read | `kDCIF_Vsync*` (read from source, CORE, not tied to i.MX95) |
| `rpmsg_lite_pingpong_rtos_linux` (not display) | Boot-image started M7, shared memory | With Linux, rpmsg only (read from source, EX; LPD5 EVK paths, not Verdin) | none |

## Not done / open

- No clone: summary rows above should be re-verified from a checkout.
- Not read: `fsl_dpu.c`, `fsl_ldb.c`, `fsl_mipi_dsi.c`, `fsl_dpu_irqsteer.h`,
  `sm_platform.h` and the HAL sources, `B/nonsm`, `B/ca55`, `B/cm33`,
  `project_segments` beyond Kconfig, the `isi` example, the release notes,
  `mcux-devices-imx` MIMX9596 contents.
- No SDK tag matched to BSP 6.6.157-7.8.0; no Toradex-specific M7 display
  port beyond the NXP `imx95verdinevk` board was found.
- The SM configuration/LM the examples assume: **not found.**
