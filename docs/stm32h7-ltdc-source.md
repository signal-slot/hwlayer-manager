# STM32H7 LTDC and DMA2D: desk evaluation from source

Date of reading: 2026-11-28. This is the first step of SoW §5.4 for the STM32H7 target ("STM32H7 / LTDC and DMA2D"): document the SoC's display topology before any backend is written. It is part of the desk evaluation of the next MCU candidates (SoW WP4 last bullet, `docs/open-issues.md` M3-06). It counts toward none of M1, M2 or M3.

**No board for this target exists on the farm and no device was used. Nothing in this file is a measurement.** Every claim is tagged **read from source** (repository, commit, file and line or symbol) or **inferred** (a conclusion drawn from what was read, or from a name, not read directly). The word for a board measurement is not used anywhere here, because there is none.

## 1. How the sources were read, and how exact the citations are

Read this first: the citations are weaker than in the STM32MP257 documents.

| Item | Fact |
| --- | --- |
| Clones | The first attempt was refused by the worker's permission layer ("This command requires approval"). After the exact `git clone --depth 1` commands were allowed, shallow clones of `zephyrproject-rtos/zephyr`, `zephyrproject-rtos/hal_stm32` and `STMicroelectronics/stm32h7xx-hal-driver` were made, and their `HEAD` equals the pinned commits below. The facts were first fetched over HTTPS at those SHAs, then every Zephyr and HAL line number was **re-checked with `grep -n` on the clones**. `cmsis-device-h7` was **not** cloned and `hal_stm32` was cloned but its H7 files were not read. |
| Zephyr line numbers | Verified with `grep -n` against the checkout at `bc833f46` for the driver, Kconfig, binding and DTS lines cited. Ranges (`a-b`) are the extents counted during the first read; the start lines were re-checked. |
| ST HAL line numbers | Verified with `grep -n` against the checkout at `23147a43`; see the line index after this table. Rows that name only a symbol are in that index. |
| Not read | AN4861 and RM0433 (see section 9), the `st,stm32-dma2d` binding YAML, the `stm32h7xx` CMSIS header bit definitions (`LTDC_SRCR_*`, `LTDC_LxCR_*`), the other H7 boards and the DSI shield definitions. Claims that would need them are **inferred** or listed as open. |
| Zephyr's own HAL copy | The `hal_stm32` module is the copy Zephyr builds against. Only its commit SHA was fetched; its H7 LTDC and DMA2D sources were not read. The ST HAL repository was read instead, and that the two agree on these files is **inferred**. |

### Pinned commits

| Repository | Ref at fetch | Full commit | Commit date | Files read |
| --- | --- | --- | --- | --- |
| `zephyrproject-rtos/zephyr` | `main` | `bc833f469ee653b7755881b8850792a4e14bacbb` | 2026-11-27 | `drivers/display/display_stm32_ltdc.c`, `drivers/display/Kconfig.stm32_ltdc`, `dts/bindings/display/st,stm32-ltdc.yaml`, `dts/arm/st/h7/stm32h7.dtsi`, `stm32h742.dtsi`, `stm32h743.dtsi`, `stm32h745.dtsi`, `stm32h747.dtsi`, `stm32h750.dtsi`, `stm32h7a3.dtsi`, `stm32h7b0.dtsi`, `stm32h7b3.dtsi`, board files of `stm32h747i_disco`, `stm32h7b3i_dk`, `stm32h750b_dk`; directory listings of `drivers/display/`, `drivers/dma/`, `boards/st/` |
| `STMicroelectronics/stm32h7xx-hal-driver` | `master` | `23147a4352c313487d327572298f8853bff8d894` | 2026-11-26 | `Src/stm32h7xx_hal_ltdc.c`, `Inc/stm32h7xx_hal_ltdc.h`, `Inc/stm32h7xx_hal_ltdc_ex.h`, `Src/stm32h7xx_hal_dma2d.c`, `Inc/stm32h7xx_hal_dma2d.h` |
| `STMicroelectronics/cmsis-device-h7` | `master` | `693f8f0a37e7396b234e177f7570b818de0f45bd` | 2026-11-27 | `Include/stm32h743xx.h` (only windows at offsets 0, 50000 and 100000 of a 2.1 MB file) |
| `zephyrproject-rtos/hal_stm32` | `main` | `ba344b95636b154092d139b9ce4640c629c3c05a` | 2026-11-26 | SHA only, no file read |

Below, `zephyr@bc833f46` means the first row, `hal@23147a43` the second, `cmsis@693f8f0a` the third.

### Line index for the ST HAL (`hal@23147a43`, checked with `grep -n`)

| Symbol | File | Line |
| --- | --- | --- |
| `MAX_LAYER 2U` | `Inc/stm32h7xx_hal_ltdc.h` | 45 |
| `LTDC_LAYER_1`, `LTDC_LAYER_2` | same | 261, 262 |
| `LTDC_BLENDING_FACTOR1_CA`, `_PAxCA` | same | 323, 324 |
| `LTDC_BLENDING_FACTOR2_CA`, `_PAxCA` | same | 332, 333 |
| `LTDC_PIXEL_FORMAT_ARGB8888` .. `AL88` | same | 341-348 |
| `LTDC_RELOAD_IMMEDIATE`, `LTDC_RELOAD_VERTICAL_BLANKING` | same | 398, 399 |
| `__HAL_LTDC_RELOAD_IMMEDIATE_CONFIG` | same | 466 |
| `__HAL_LTDC_VERTICAL_BLANKING_RELOAD_CONFIG` | same | 473 |
| `LTDC_LAYER(...)`, `IS_LTDC_LAYER` | same | 646, 649 |
| `IS_LTDC_BLENDING_FACTOR1`, `2`, `IS_LTDC_ALPHA` | same | 669, 671, 681 |
| `HAL_LTDC_Init`, `HAL_LTDC_DeInit` (layer 2 under `#if defined(LTDC_Layer2_BASE)` at 341) | `Src/stm32h7xx_hal_ltdc.c` | 219, 325 |
| `HAL_LTDC_IRQHandler` | same | 627 |
| `HAL_LTDC_ConfigLayer`, its `SRCR = IMR` | same | 827, 856 |
| `ConfigColorKeying` / `EnableColorKeying` / `ConfigCLUT` / `EnableCLUT` | same | 877 / 967 / 915 / 1037 |
| `HAL_LTDC_EnableDither` | same | 1105 |
| `SetWindowSize`, `SetWindowPosition`, `SetPixelFormat`, `SetAlpha`, `SetAddress`, `SetPitch` | same | 1163, 1219, 1270, 1315, 1361, 1410 (IMR writes at 1197, 1249, 1294, 1339, 1384, 1454 and 1460) |
| `HAL_LTDC_ProgramLineEvent` | same | 1479 |
| `HAL_LTDC_ConfigLayer_NoReload` .. `DisableCLUT_NoReload` | same | 1557-2044 |
| `LTDC_SetConfig` | same | 2130 |
| `DMA2D_M2M` .. `DMA2D_M2M_BLEND_BG` | `Inc/stm32h7xx_hal_dma2d.h` | 209-214 |
| `DMA2D_OUTPUT_*`, `DMA2D_INPUT_*` | same | 222-226, 234-245 |
| `DMA2D_*_ALPHA`, `DMA2D_NO_CSS`..`CSS_420`, `DMA2D_CCM_*` | same | 253-255, 302-304, 312-313 |
| `HAL_DMA2D_Start`, `BlendingStart`, `CLUTStartLoad`, `PollForTransfer`, `ConfigLayer`, `ProgramLineEvent`, `ConfigDeadTime` | `Src/stm32h7xx_hal_dma2d.c` | 677, 750, 1016, 1415, 1772, 1936, 2021 |
| "two consecutive transactions on the AHB bus" (dead-time comment) | same | 2016 |

The "Where" column of the section 2 to 5 rows names symbols; use this index for the line.

## 2. (a) LTDC: layers, formats, alpha, blending, window

| Claim | Basis | Where |
| --- | --- | --- |
| The HAL knows exactly two layers: `MAX_LAYER` is 2, `LTDC_LAYER_1` is 0 and `LTDC_LAYER_2` is 1, `IS_LTDC_LAYER(x)` is `x < MAX_LAYER`, and `LTDC_HandleTypeDef` holds `LayerCfg[MAX_LAYER]`. Layer 2 is only touched under `#if defined(LTDC_Layer2_BASE)`, so some parts may have one layer | read from source | hal@23147a43 `Inc/stm32h7xx_hal_ltdc.h` symbols `MAX_LAYER`, `IS_LTDC_LAYER`; `Src/stm32h7xx_hal_ltdc.c` symbol `HAL_LTDC_DeInit` |
| Register map: `LTDC_Layer1_BASE` is `LTDC_BASE + 0x84` and `LTDC_Layer2_BASE` is `LTDC_BASE + 0x104`; layer stride 0x80 (the HAL macro `LTDC_LAYER` adds `0x84 + 0x80*layer`) | read from source | cmsis@693f8f0a `Include/stm32h743xx.h` symbols `LTDC_Layer1_BASE`, `LTDC_Layer2_BASE`; hal@23147a43 symbol `LTDC_LAYER` |
| The layer register block is exactly `CR, WHPCR, WVPCR, CKCR, PFCR, CACR, DCCR, BFCR, CFBAR, CFBLR, CFBLNR, CLUTWR` (plus reserved words). There is **no scaler register** and **no YUV register** (no `PCR`, no `YCEN`) | read from source | cmsis@693f8f0a `Include/stm32h743xx.h` symbol `LTDC_Layer_TypeDef` (H743 header only; other H7 headers not read) |
| The global register block has `SSCR, BPCR, AWCR, TWCR, GCR, SRCR, BCCR, IER, ISR, ICR, LIPCR, CPSR, CDSR`; `SRCR` at 0x24, `GCR` at 0x18, `LIPCR` at 0x40 | read from source | cmsis@693f8f0a symbol `LTDC_TypeDef` |
| Pixel formats the HAL accepts: ARGB8888 (0), RGB888 (1), RGB565 (2), ARGB1555 (3), ARGB4444 (4), L8 (5), AL44 (6), AL88 (7). All RGB or luminance, none YUV | read from source | hal@23147a43 `Inc/stm32h7xx_hal_ltdc.h` symbols `LTDC_PIXEL_FORMAT_*` |
| No YUV path in the H7 HAL: no `LTDC_YUV2RGBCONVERTOR*`, no `ConfigLayerFlexYUVCoPlanar`, no FlexYUV types, no `YCEN`; `stm32h7xx_hal_ltdc_ex.h` holds only the two DSI struct-copy helpers. The H743 layer struct has no `PCR` | read from source | hal@23147a43 headers; cmsis@693f8f0a symbol `LTDC_Layer_TypeDef` |
| So the LTDC of the H743 (and, by the same HAL, probably every H7 part) has **no YUV converter**. Parts other than H743 were not checked in the CMSIS headers; the HAL has one source for all | inferred | |
| Blending factors: `BLENDING_FACTOR1` is `CA` (0x400) or `PAxCA` (0x600); `BLENDING_FACTOR2` is `CA` (0x5) or `PAxCA` (0x7). The assert macros accept only those two values each | read from source | hal@23147a43 `Inc/stm32h7xx_hal_ltdc.h` symbols `LTDC_BLENDING_FACTOR1_CA`, `_PAxCA`, `LTDC_BLENDING_FACTOR2_CA`, `_PAxCA`, `IS_LTDC_BLENDING_FACTOR1`, `IS_LTDC_BLENDING_FACTOR2` |
| Reading the names: layer colour = F1 x layer colour + F2 x colour below, with F1 = constant alpha or (pixel alpha x constant alpha) and F2 = 1 - F1. This is the standard LTDC blend; the equation itself is in RM0433, which was not read | inferred | |
| Layer config carries constant alpha `Alpha` (0..255, asserted by `IS_LTDC_ALPHA`), default alpha `Alpha0` (also asserted), and a per-layer window `WindowX0/X1/Y0/Y1` plus `ImageWidth/ImageHeight` and a framebuffer address | read from source | hal@23147a43 `Inc/stm32h7xx_hal_ltdc.h` symbol `LTDC_LayerCfgTypeDef`; `Src/stm32h7xx_hal_ltdc.c` symbol `HAL_LTDC_ConfigLayer` |
| Layer window and source are the same size: `LTDC_SetConfig` writes `CFBLR = (ImageWidth * bytes) << 16 \| ((WindowX1 - WindowX0) * bytes + 7)` and `CFBLNR`. No source-rectangle-versus-destination-rectangle pair exists, so **no scaling**. A source smaller than the display is a smaller window on the display, not a scaled one | read from source (registers); no scaling: inferred from their absence | hal@23147a43 `Src/stm32h7xx_hal_ltdc.c` symbol `LTDC_SetConfig` |
| `HAL_LTDC_SetPitch` changes the line pitch (`CFBLR`) separately from the window, so a window can be cut from a larger framebuffer | read from source | hal@23147a43 symbol `HAL_LTDC_SetPitch` |
| `HAL_LTDC_SetWindowSize` and `SetWindowPosition` move or resize a window at run time | read from source | hal@23147a43 symbols of those names |
| Fixed stacking order: layer 2 over layer 1 over the background colour (`BCCR`; `Backcolor` is in the config). There is no z-order register in the layer block | read from source (no register); "layer 2 on top" is the RM's usual statement, not read | cmsis@693f8f0a symbols `LTDC_TypeDef`, `LTDC_Layer_TypeDef`; order: inferred |
| CLUT: `HAL_LTDC_ConfigCLUT` writes `CLUTWR` entries as `(index << 24) \| RGB`; `EnableCLUT`/`DisableCLUT` set and clear `LTDC_LxCR_CLUTEN`. Used with L8 and AL44/AL88 | read from source | hal@23147a43 symbols `HAL_LTDC_ConfigCLUT`, `HAL_LTDC_EnableCLUT` |
| Colour keying per layer: `HAL_LTDC_ConfigColorKeying` writes `CKCR`; `EnableColorKeying` sets `LTDC_LxCR_COLKEN` | read from source | hal@23147a43 symbols of those names |
| Dither is a global bit: `HAL_LTDC_EnableDither` sets `LTDC_GCR_DEN` | read from source | hal@23147a43 symbol `HAL_LTDC_EnableDither` |
| Pad bandwidth and maximum resolution are not in the HAL. Zephyr's DT builds a framebuffer of `width x height x bytes` from DT properties and caps `set_timing` at the DT size | read from source (Zephyr); the hardware limits: not read | zephyr@bc833f46 `drivers/display/display_stm32_ltdc.c:891-905` |

## 3. (b) Shadow reload: immediate, vertical blanking, and what "latched" means

| Claim | Basis | Where |
| --- | --- | --- |
| The layer registers are shadowed. Writes to them take effect only on a **reload request** written to `SRCR`: `LTDC_SRCR_IMR` (immediate) or `LTDC_SRCR_VBR` (vertical blanking). The HAL names them `LTDC_RELOAD_IMMEDIATE` and `LTDC_RELOAD_VERTICAL_BLANKING` | read from source (names and the two bits); the shadow model itself: inferred from this structure | hal@23147a43 `Inc/stm32h7xx_hal_ltdc.h` symbols `LTDC_RELOAD_IMMEDIATE`, `LTDC_RELOAD_VERTICAL_BLANKING` |
| Macros: `__HAL_LTDC_RELOAD_IMMEDIATE_CONFIG` does `SRCR \|= LTDC_SRCR_IMR`; `__HAL_LTDC_VERTICAL_BLANKING_RELOAD_CONFIG` does `SRCR \|= LTDC_SRCR_VBR` | read from source | hal@23147a43 `Inc/stm32h7xx_hal_ltdc.h`, those symbols |
| `HAL_LTDC_ConfigLayer`, `SetWindowSize`, `SetWindowPosition`, `SetPixelFormat`, `SetAlpha`, `SetAddress`, `ConfigColorKeying`, `EnableColorKeying`, `DisableColorKeying`, `EnableCLUT`, `DisableCLUT` each end with `SRCR = LTDC_SRCR_IMR`, so the HAL setters apply **immediately**. `SetPitch` does `SRCR \|= IMR` twice | read from source | hal@23147a43 `Src/stm32h7xx_hal_ltdc.c`, those symbols |
| Every setter has a `_NoReload` twin that skips the `SRCR` write, so a caller can stage several changes and issue one reload | read from source | hal@23147a43 symbols `HAL_LTDC_*_NoReload` |
| `HAL_LTDC_Reload(hltdc, ReloadType)` enables `LTDC_IT_RR` and then writes `SRCR = ReloadType`. The only HAL use of the vertical-blanking macro is in `HAL_LTDC_DeInit`. There is no `HAL_LTDC_ReloadEx` | read from source | hal@23147a43 symbols `HAL_LTDC_Reload`, `HAL_LTDC_DeInit` |
| Interrupts: `ISR/IER` carry `LIF/LIE` (line), `RRIF/RRIE` (register reload), `FUIF/FUIE` (FIFO underrun) and `TERRIF/TERRIE` (transfer error). `HAL_LTDC_IRQHandler` handles them in the order TE, FU, LI, RR, disables the source and clears the flag each time. The line interrupt is therefore **one-shot in the HAL**; `HAL_LTDC_Init` enables only TE and FU. The reload event reaches the application as the weak `HAL_LTDC_ReloadEventCallback` | read from source | hal@23147a43 symbols `HAL_LTDC_IRQHandler`, `HAL_LTDC_Init`, `HAL_LTDC_ReloadEventCallback` |
| Line interrupt: `HAL_LTDC_ProgramLineEvent(Line)` asserts `Line <= 0x7FF`, disables LI, writes `LIPCR`, re-enables LI | read from source | hal@23147a43 symbol `HAL_LTDC_ProgramLineEvent` |
| Current position: `CPSR` holds the current line in its low 16 bits (used by Zephyr as `CPSR & 0xFFFF`) | read from source | zephyr@bc833f46 `drivers/display/display_stm32_ltdc.c:100` |
| What "latched" means here: a value written to a shadowed layer register is not used for scan-out until the reload happens. For IMR that is as soon as the hardware takes the request; for VBR it is at the start of the next vertical blanking. The reload-done flag `RRIF` marks the moment the shadow values were taken over, so the application knows the old framebuffer is free. The wording of when exactly IMR takes effect (mid-frame, per line) and that RRIF is set in both cases are in RM0433, not read | inferred | |
| Contrast with the MP257 LTDC: there the driver uses per-layer reload registers (`LTDC_L1RCR`) in newer IP and one global `SRCR_VBR` in older IP, and never IMR for updates (section 8) | read from source (MP257 docs) | [`stm32mp257-ltdc-sync-source.md`](stm32mp257-ltdc-sync-source.md) section 1 |
| A global reload covers **both** layers at once (one `SRCR` for the whole block), so a two-layer update can land atomically at VBR if both layers are staged with `_NoReload` first | inferred (one `SRCR` register, no per-layer reload register in the layer block) | cmsis@693f8f0a symbols `LTDC_TypeDef`, `LTDC_Layer_TypeDef` |

## 4. (c) DMA2D as a 2D helper

DMA2D ("Chrom-ART") is a separate DMA with its own pixel pipeline. It writes to memory; it does not scan out. It produces framebuffers that the LTDC then shows.

| Claim | Basis | Where |
| --- | --- | --- |
| Modes: `DMA2D_M2M` (copy), `DMA2D_M2M_PFC` (copy with pixel format conversion), `DMA2D_M2M_BLEND` (blend foreground and background into the output), `DMA2D_R2M` (register to memory, a solid fill), `DMA2D_M2M_BLEND_FG` and `DMA2D_M2M_BLEND_BG` (blend with a fixed colour in one of the two inputs) | read from source | hal@23147a43 `Inc/stm32h7xx_hal_dma2d.h` symbols `DMA2D_M2M`, `DMA2D_M2M_PFC`, `DMA2D_M2M_BLEND`, `DMA2D_R2M`, `DMA2D_M2M_BLEND_FG`, `DMA2D_M2M_BLEND_BG` |
| Input colour modes: ARGB8888, RGB888, RGB565, ARGB1555, ARGB4444, L8, AL44, AL88, L4, A8, A4 and **YCbCr** with chroma sub-sampling `NO_CSS`, `CSS_422`, `CSS_420` (applied to the foreground layer only). Output modes: ARGB8888, RGB888, RGB565, ARGB1555, ARGB4444 | read from source | hal@23147a43 `Inc/stm32h7xx_hal_dma2d.h` symbols `DMA2D_INPUT_*`, `DMA2D_*_CSS`, `DMA2D_OUTPUT_*` |
| Alpha handling on a layer: `DMA2D_NO_MODIF_ALPHA`, `DMA2D_REPLACE_ALPHA`, `DMA2D_COMBINE_ALPHA`, plus `DMA2D_REGULAR_ALPHA`/`DMA2D_INVERTED_ALPHA`, red-blue swap and byte swap options | read from source | hal@23147a43 symbols of those names |
| CLUT for L8/AL44/AL88/L4 inputs, loaded by `HAL_DMA2D_CLUTStartLoad`, formats `DMA2D_CCM_ARGB8888` and `DMA2D_CCM_RGB888` | read from source | hal@23147a43 symbols `HAL_DMA2D_CLUTStartLoad`, `DMA2D_CCM_*` |
| Calls: `HAL_DMA2D_Start` (copy, PFC, fill), `HAL_DMA2D_BlendingStart` (two sources and a destination), `HAL_DMA2D_PollForTransfer` (waits for `TC`, and for `CTC` if a CLUT load is pending), `HAL_DMA2D_ConfigLayer` (layer 0 background, layer 1 foreground), `HAL_DMA2D_ProgramLineEvent` (watermark). The transfer size is `NLR = Height \| Width << PL_Pos`; output address `OMAR` | read from source | hal@23147a43 `Src/stm32h7xx_hal_dma2d.c`, those symbols |
| The HAL has dead-time controls (`HAL_DMA2D_ConfigDeadTime`, register `AMTCR`) and describes the dead time as the minimum number of cycles between two transactions "on the AHB bus", which lets the CPU and the LTDC get memory bandwidth between DMA2D bursts | read from source (comment); the bandwidth purpose: inferred | hal@23147a43 symbols `HAL_DMA2D_ConfigDeadTime`, `HAL_DMA2D_EnableDeadTime` |
| DMA2D base: `DMA2D_BASE` is `D1_AHB1PERIPH_BASE + 0x1000`; the Zephyr DT node is `dma2d: dma@52001000`, compatible `st,stm32-dma2d`, interrupt 90, disabled by default | read from source | cmsis@693f8f0a symbol `DMA2D_BASE`; zephyr@bc833f46 `dts/arm/st/h7/stm32h7.dtsi:1028` |
| **Zephyr has no DMA2D driver.** `drivers/display/` has `display_stm32_ltdc.c` as its only STM32 file and `drivers/dma/` has no `dma2d` file; `drivers/dma/dma_mcux_pxp.c` is the NXP analogue. The node is enabled by the board `stm32h7b3i_dk` (`&dma2d { status = "okay"; }`) but nothing binds to it. The code search that would confirm "no other mention in the tree" was refused by the API (HTTP 401); the middle of each directory listing was read through a summary, so this is a listing-based finding | read from source (listings, board DTS); absence in the rest of the tree: inferred | zephyr@bc833f46 directory listings `drivers/display/`, `drivers/dma/`; `boards/st/stm32h7b3i_dk/stm32h7b3i_dk.dts:342-344` |
| DMA2D is a real 2D helper for a composition backend: a **GPU-less composer** for the case where the two LTDC layers are not enough (a third UI layer or a layer needing rotation-free format conversion). It blends into a framebuffer in memory, which costs memory bandwidth and a frame of latency, and gives no tearing control of its own | inferred | |
| DMA2D cannot scale or rotate (no such mode in the mode list) | read from source (list of modes); the absence of scaling in RM0433: inferred | hal@23147a43 `Inc/stm32h7xx_hal_dma2d.h` symbol `DMA2D_Mode` |

## 5. (d) Framebuffer memory, cache and DMA visibility

| Claim | Basis | Where |
| --- | --- | --- |
| Zephyr places the framebuffer in static `frame_buffer_<n>`, aligned to 32 bytes ("cache line width for optimal cache flushing"), size `FB_NUM x width x height x bytes_per_pixel`. With DT property `ext-sdram` the array goes into the linker region named after that node (`FRAME_BUFFER_SECTION`); without it, in normal RAM ("internal RAM") | read from source | zephyr@bc833f46 `drivers/display/display_stm32_ltdc.c:957-962`, `972-984`; binding `dts/bindings/display/st,stm32-ltdc.yaml:29-33` |
| Optionally the framebuffer is taken from Zephyr's shared multi-heap (`CONFIG_STM32_LTDC_FB_USE_SHARED_MULTI_HEAP`), with attribute cacheable (0), non-cacheable (1) or external (2) and alignment 32 | read from source | zephyr@bc833f46 `drivers/display/Kconfig.stm32_ltdc:63-80`; `display_stm32_ltdc.c:797-810` |
| The driver does CPU cache maintenance itself: `sys_cache_data_flush_range` after writing a full frame (`write`), after each row of a partial write and after the back-buffer copy. Kconfig selects `CACHE_MANAGEMENT if DCACHE` | read from source | zephyr@bc833f46 `display_stm32_ltdc.c:319-335`, `395-402`, `433-434`; `Kconfig.stm32_ltdc:11` |
| Consequence: with a cacheable framebuffer on the Cortex-M7 the CPU data cache must be cleaned before the LTDC (a bus master that does not snoop the cache) reads it. That the LTDC DMA does not snoop the M7 cache is a general property of the SoC | inferred | |
| FMC bank 1 is switched off by default: `STM32_LTDC_DISABLE_FMC_BANK1` (default y on H7 and F7) clears `MBKEN` and `MTYP[1:0]` of `FMC_Bank1_R->BTCR[0]` "to prevent speculative read accesses", citing AN4861 section 4.6 "Special recommendations for Cortex-M7". The reason: the M7 may speculatively read from an unused FMC region and stall the LTDC. AN4861 itself was not read | read from source (code, Kconfig text); the mechanism: inferred | zephyr@bc833f46 `Kconfig.stm32_ltdc:82-88`; `display_stm32_ltdc.c:788-795` |
| External SDRAM over FMC: the boards describe it as a `zephyr,memory-region`, `mmio-sram`, in FMC SDRAM bank 2 at `0xd0000000`, with attribute `ATTR_MPU_RAM`, which is how Zephyr sets the MPU region. Sizes: 16 MiB on `stm32h7b3i_dk`, 8 MiB (of a 128 Mbit part, "only half is available by HW design") on `stm32h750b_dk`, 32 MiB on `stm32h747i_disco` | read from source | zephyr@bc833f46 `boards/st/stm32h7b3i_dk/stm32h7b3i_dk.dts:96-102`; `boards/st/stm32h750b_dk/stm32h750b_dk-common.dtsi:59-66`; `boards/st/stm32h747i_disco/stm32h747i_disco_stm32h747xx_m7.dts:61-67` |
| `stm32h747i_disco` also declares a 64 MiB `ext_memory` at `0x90000000` (octal flash window) with `ATTR_MPU_IO` and a comment that `ATTR_MPU_EXTMEM` "causes a MPU FAULT" | read from source | zephyr@bc833f46 `boards/st/stm32h747i_disco/stm32h747i_disco_stm32h747xx_m7.dts:53-59` |
| Internal framebuffer in AXI SRAM or other SRAM: the driver allows it ("If not defined, internal RAM will be used"), but which SRAM bank the linker chooses is not set by the driver. That a 480x272 RGB565 frame (about 255 KiB) fits in the 512 KiB AXI SRAM is arithmetic; the memory map of the part was not read | read from source (the option); sizes: inferred | zephyr@bc833f46 binding lines 29-33 |
| Which bus the LTDC masters (AXI to the SRAM and to the FMC) and the peak bandwidth are in the RM; not read. FIFO underrun is reported by `LTDC_FLAG_FU` and enabled by `HAL_LTDC_Init`, so the hardware signals starvation | read from source (flag); bandwidth: not read | hal@23147a43 symbol `HAL_LTDC_Init` |
| DMA2D to a cacheable framebuffer needs the mirror maintenance: clean the source before, invalidate the destination (or place the buffers in a non-cacheable MPU region) after. Not done by Zephyr, since there is no DMA2D driver | inferred | |

## 6. (e) Zephyr's `display_stm32_ltdc`

All references are `zephyr@bc833f46 drivers/display/display_stm32_ltdc.c` (lines checked with `grep -n` on the clone) unless noted.

| Topic | Claim | Basis | Lines |
| --- | --- | --- | --- |
| Layers used | **One.** `stm32_ltdc_start` configures layer 1 ("only one layer is used") and `__HAL_LTDC_LAYER_DISABLE(…, LTDC_LAYER_2)` ("since it not used"). `LayerCfg[0]` is the only static layer config; there is no second layer in the DT binding | read from source | 699-715, 1020-1041 |
| Window | Layer 1 window from DT `window0-x0/x1/y0/y1`, defaulting to `0..width` and `0..height`; the window follows `set_timing` | read from source | 1021-1026, 914-919; binding 61-75 |
| Alpha and blend | Constant alpha 255, `Alpha0` 0, both blend factors `PAxCA`, set statically | read from source | 1028-1031 |
| Background | `def-back-color-red/green/blue`, default 0xFF each | read from source | 1013-1018; binding 49-59 |
| Pixel formats | Build-time choice among ARGB8888, RGB888, RGB565 (default), L8 (`CONFIG_STM32_LTDC_*`). Run-time `set_pixel_format` accepts L8, RGB565, RGB888 and ARGB8888 and calls `HAL_LTDC_SetPixelFormat(…, 0)` (layer index 0), otherwise `-ENOTSUP`. ARGB1555, ARGB4444, AL44 and AL88 are not offered | read from source | 32-46, 230-284; `Kconfig.stm32_ltdc:21-51` |
| YUV in the driver | A YUYV path exists (`stm32_ltdc_set_yuv`, `PIXEL_FORMAT_YUYV`, flex-YUV config, BT.601, reduced range) behind `#if defined(LTDC_YUV2RGBCONVERTOR_BT601_REDUCED_RANGE)`, citing an erratum "ES0620, section 2.7.1" for planar 4:2:0. The H7 HAL has no such symbol, so on H7 `STM32_LTDC_HAS_YUV` is undefined and `set_yuv` returns `-ENOTSUP`. The path is for another STM32 series | read from source (driver, HAL header); applies to H7: inferred from the symbol's absence in `hal@23147a43` | 48-51, 186-228, 237-249; hal `Inc/stm32h7xx_hal_ltdc.h` |
| Double buffering | `CONFIG_STM32_LTDC_FB_NUM` 0, 1 or 2 (default 1): 0 = application owns the buffer and writes full frames; 1 = one internal buffer; 2 = two internal buffers | read from source | `Kconfig.stm32_ltdc:53-61` |
| How a swap is done | `display_write` of a full-screen rectangle takes the caller's buffer as the new front buffer (after a cache flush); `stm32_ltdc_sync_frame` stores it in `pend_buf`, clears and enables the line interrupt, and **blocks** on a semaphore (`K_FOREVER`) until the ISR has swapped. A partial write with `FB_NUM == 2` copies the front buffer into the back one, overlays the rectangle, flushes and swaps; with 1 buffer it writes in place (visible tearing) | read from source | 347-363, 395-409, 411-439 |
| **Reload mode** | The ISR, on the line interrupt at line 0, writes the new address straight into `LTDC_LAYER(…, LTDC_LAYER_1)->CFBAR` and then calls `__HAL_LTDC_RELOAD_IMMEDIATE_CONFIG`, i.e. **`SRCR_IMR`**. It does not use VBR. It relies on the interrupt being raised at the frame start to approximate a vblank-aligned update | read from source (code); the timing relation of line 0: inferred (RM not read) | 97-146, esp. 130-141 |
| Event model | `display_register_event_cb` supports `DISPLAY_EVENT_VSYNC` and `DISPLAY_EVENT_LINE_INT` from ISR context only (`in_isr` must be true, otherwise `-ENOSYS`; one callback at a time, `-EBUSY`). VSYNC is the line interrupt programmed to line 0 ("VSync can only be detected by LTDC line interrupt"). If the callback returns true for VSYNC (event handled), the driver does **not** swap; the application then owns the swap | read from source | 97-179, 557-607; capabilities 316 |
| Fixed behaviours | `set_orientation` accepts only `DISPLAY_ORIENTATION_NORMAL`; `get_capabilities` reports `screen_info = 0`; `read` copies from the front buffer; `get_framebuffer` returns the front buffer | read from source | 286-296, 298-317, 444-479 |
| Blanking | `blanking_on/off` switch only the backlight GPIO `bl-ctrl-gpios` and, if a `display-controller` phandle (a DSI panel device) is given, forward to its `display_blanking_*`. They do **not** touch the LTDC layer. With neither, `-ENOSYS` | read from source | 481-555 |
| Panel enable | `disp-on-gpios` and the backlight are set after layer start; `suspend` turns the panel off, disables the LTDC and its clock | read from source | 664-723, 822-865 |
| Timing | Taken from the DT child `display-timings` (`zephyr,panel-timing`) at init, or later through `set_timing`; without it the LTDC is started by `display_set_timing()`. `set_timing` rejects a mode larger than the DT `width x height` with `-ENOMEM` and rebuilds layer 1 | read from source | 812-819, 891-927 |
| Output format | Only `PANEL_PIXEL_FORMAT_RGB_888` is accepted at the LTDC output (compile error otherwise) | read from source | 986-989 |
| Interrupts | The driver connects the first interrupt only (`DT_INST_IRQN`); the `ltdc_er` interrupt (`interrupt-names = "ltdc", "ltdc_er"`) is not connected, so the HAL error path is not used. FIFO underrun and transfer error are therefore not reported by this driver | read from source (driver, DTS); consequence: inferred | 995-1003; `stm32h742.dtsi:65-73` |
| Instance | The macro hard-codes instance 0 in `DT_INST_PROP(0, pixel_format)`, `RESET_DT_SPEC_INST_GET(0)` and the `ext_sdram` section: one LTDC | read from source | 957, 987, 1081 |
| DT binding | `st,stm32-ltdc`: `disp-on-gpios`, `bl-ctrl-gpios`, `ext-sdram` (phandle), `clocks`, `resets`, `interrupts` (required), `pinctrl-0`, `def-back-color-*`, `window0-*`, `display-controller`; includes `lcd-controller.yaml` (`width`, `height`, `pixel-format`, from the include; not read) | read from source | binding 12-81 |
| SoC nodes | `ltdc: display-controller@50001000`, `reg = <0x50001000 0x200>`, interrupts 88 and 89, clocks `APB3` bit 3, reset `APB3` bit 3, in `stm32h742.dtsi:65-73` (inherited by H743, H750), `stm32h745.dtsi:69-77` (H747) and `stm32h7a3.dtsi:107-115` (H7B0, H7B3) and `stm32h723.dtsi:113` (found on re-check, node body not read). `stm32h7.dtsi` has no `ltdc` node. Only `stm32h747.dtsi` has a DSI host (`st,stm32-mipi-dsi` at `0x50000000`) | read from source | those files |
| DMA2D state in Zephyr | See section 4: node present, no driver, no use in the LTDC driver | read from source | section 4 |
| What the driver does **not** expose | Layer 2, per-layer alpha at run time, run-time blending factors, window change after init other than via timing, CLUT, colour keying, z-order, any scaling, reload on VBR, the reload-done interrupt, FIFO-underrun reporting, any YUV on H7 | inferred from the driver reading above (absence of code) | |

The Zephyr driver exposes the LTDC as a **single-plane framebuffer display** (`display_write` plus optional double buffering). A composition backend over this driver would see one plane; using layer 2 or blend control would need either direct HAL calls beside the driver or a new driver, since `LayerCfg[0]` and layer 1 are hard-wired in it (inferred).

## 7. (f) Boards in Zephyr

All `zephyr@bc833f46`; board DTS lines for the `ltdc`, `sdram2`, `dma2d` and display nodes were re-checked with `grep -n`; the longer ranges are as first counted.

| Board | LTDC use | Panel and memory | Where |
| --- | --- | --- | --- |
| `stm32h7b3i_dk` (SoC `stm32h7b3xxq`, "STM32H7B3I Discovery kit") | `zephyr,display = &ltdc`; RGB parallel pins; `disp-on-gpios` PA2, `bl-ctrl-gpios` PA1; `ext-sdram = <&sdram2>`; **480x272**, `PANEL_PIXEL_FORMAT_RGB_888`; timings hsync 1, vsync 10, hbp 43, vbp 12, hfp 8, vfp 4; PLL3R gives a 9 MHz pixel clock (DTS comment). `&dma2d` enabled | The board doc says "4.3" (480x272 pixels) TFT colour LCD module including a capacitive touch panel with RGB interface"; it lists "Chrom-ART graphical hardware Accelerator (DMA2D)" and a 128 Mbit SDRAM. The DTS has 16 MiB at `0xd0000000`. The panel's part number is **not** stated in the DTS or the doc | `boards/st/stm32h7b3i_dk/stm32h7b3i_dk.dts:23, 96-102, 125-134, 196-232, 244-279, 342-344`; `doc/index.rst` |
| `stm32h750b_dk` (SoC `stm32h750xx`, "STM32H750B Discovery Kit", variant `ext_flash_app`) | `zephyr,display = &ltdc`; `disp-on-gpios` PD7 (no backlight GPIO); `ext-sdram = <&sdram2>`; `clocks` adds `PLL3_R` as the LTDC kernel clock; **480x272**, RGB888; timings as above but `de-active = 1`. DMA2D is **not** enabled | Doc: "RGB interface LCD with capacitive touch panel", no size and no part number; 8 MiB SDRAM window at `0xd0000000`, FMC SDRAM on bank 1 with CAS 3 | `boards/st/stm32h750b_dk/stm32h750b_dk-common.dtsi:17, 59-66, 83-122, 134-141, 211-247` |
| `stm32h747i_disco` (SoC `stm32h747xx`, M7 core file) | The M7 DTS has **no `&ltdc` node and no `zephyr,display`**. It only provides aliases `zephyr_lcd_controller: &ltdc` and `zephyr_mipi_dsi: &mipi_dsi` for display shields; the DSI LCD connector (`st,dsi-lcd-qsh-030-connector`) maps `LCD_BL_CTRL` (PJ12), `DSI_TE` (PJ2) and `DSI_RESET` (PG3). 32 MiB SDRAM at `0xd0000000`, 64 MiB at `0x90000000` | Doc: a 4" capacitive-touch LCD with a MIPI DSI interface; display is used with shield `st_b_lcd40_dsi1_mb1166` (OTM8009A controller) or `st_b_lcd40_dsi1_mb1166_a09` (NT35510 controller) on connector CN15. The shields were not read | `boards/st/stm32h747i_disco/stm32h747i_disco_stm32h747xx_m7.dts:53-67, 300-304`; `stm32h747i_disco.dtsi:88-104`; `doc/index.rst` |
| Other H7 boards present in `boards/st/` | `stm32h735g_disco`, `stm32h745i_disco`, `stm32h757i_eval`, `stm32h7p5i_dk`, `stm32h7s78_dk` exist per the directory listing; **none was read**. `nucleo_h7*` boards have no LCD. (`stm32h7p5i_dk`/`h7s78_dk` are H7R/S parts and may use a different LTDC, not checked) | The names come from a summarised listing, so their presence is less certain than the other rows | `boards/st/` listing |

The panel part numbers RK043FN48H, RK070ER9427 or similar are named by ST's BSP but do **not** appear in what was read; the 480x272 numbers are the only panel facts that are sourced.

Inferred: for a first hardware evaluation the candidate boards are `stm32h7b3i_dk` or `stm32h750b_dk`, which are the two with a DT-enabled parallel RGB LTDC panel and SDRAM in Zephyr. The `stm32h747i_disco` needs a DSI shield and has no board-level LTDC config.

## 8. (g) Cross-reference with the STM32MP257 LTDC docs

The MP257 documents were read and not edited: [`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md), [`stm32mp257-ltdc-sync-source.md`](stm32mp257-ltdc-sync-source.md), [`stm32mp257-ltdc-transform-source.md`](stm32mp257-ltdc-transform-source.md), [`stm32mp257-ltdc-yuv-source.md`](stm32mp257-ltdc-yuv-source.md) and [`design/stm32mp257-backend.md`](design/stm32mp257-backend.md). Their pinned trees are ST `STMicroelectronics/linux` at `3056405e0479330b0889c2a971452bfd10014b70` (v6.18-stm32mp) and `548f960c059bc4b9165cb69895fd67551f6061ca` (v6.6-stm32mp).

### Same IP lineage

| Point | Basis |
| --- | --- |
| Both are the ST "LTDC" (LCD-TFT display controller): Zephyr names it `st,stm32-ltdc`, the Linux driver (`drivers/gpu/drm/stm/ltdc.c`) is the same driver family for the STM32F4, F7, H7 and MP1/MP2 lines. The Linux driver's oldest hardware versions are 0x010200, 0x010300 and 0x020101, with `plane_reg_shadow` false; 0x040100/0x040101 set it true. That the H7 LTDC is one of the earlier versions is **not stated** in any file read, and which of them is unknown | read from source (Linux driver version list, MP257 docs); the H7 version: open |
| Register names match: `SRCR`, `IMR`/`VBR` reload bits, `LIPCR` and `LIF` line interrupt, `CPSR` current position, `BCCR` background, per-layer `CFBAR`, `CFBLR`, `CFBLNR`, `CACR`, `DCCR`, `BFCR`, `CKCR`, `PFCR`, `CLUTWR`, `LXCR_C1R_SCA` only on MP25. The H7 register names are from the CMSIS struct, the MP257 names from `ltdc.c` | read from source |
| Same blend structure: per-layer constant alpha, pixel alpha, and a two-factor blend selected through `BFCR` (MP257 writes `normalized_zpos` into `LTDC_L1BFCR` at shift 16, `docs/stm32mp257-ltdc-source-full.md`, zpos row; H7 writes `BlendingFactor1/2` into `BFCR`) | read from source / inferred that the same register carries both |
| Same line-interrupt completion model: MP257 completes the flip event in the line interrupt `ISR_LIF` and does not test `ISR_RRIF` (sync doc section 1); Zephyr on H7 swaps in the line interrupt and does not use `RRIF` either | read from source |
| Same shared frame buffer path: DMA fetch of an RGB (or L8) framebuffer from memory, no memory-to-memory processing inside the LTDC | inferred |

### What differs

| Topic | STM32H7 (this doc) | STM32MP257 (existing docs) |
| --- | --- | --- |
| Layer count | **2**, fixed (`MAX_LAYER` 2) | up to 4 (`LTDC_MAX_LAYER`); expected 3 on MP25, read from `LTDC_LCR` at run time; layers behind a firewall can be absent |
| Pixel formats | RGB/L: ARGB8888, RGB888, RGB565, ARGB1555, ARGB4444, L8, AL44, AL88 | RGB plus **YUV**: packed YUYV/YVYU/UYVY/VYUY, semi-planar NV12/NV21, planar YU12/YV12, with `LTDC_L1PCR` and coefficient registers `CYR0R/CYR1R`, BT.601/BT.709, limited/full (yuv doc) |
| YUV | none (no `PCR`, no `YCEN`); DMA2D can read YCbCr but writes RGB | on layers with a nonzero index, per-layer capability bits `YIA`, `YSPA`, `YFPA` |
| Scaling | **none** (no source/destination size pair) | upscale only, on layers whose `LXCR_C1R_SCA` bit is set (`ltdc.c:1585-1618` in the ST tree); downscale rejected |
| Rotation and mirror | none in the registers read | per-layer reflect-x (`HMEN`), reflect-y by negative pitch, 90/270 by a separate rotator with reserved memory (`rotation-memory`, not set on the reference tree) (transform doc) |
| z-order | **fixed** (layer 2 over layer 1) | mutable `zpos`, `0..nb_layers-1`, in `BFCR` |
| Reload | global `SRCR`: `IMR` or `VBR`; HAL setters use IMR; Zephyr uses IMR from the line-0 interrupt | per-layer reload `LTDC_L1RCR` with `LXRCR_VBR` on the 4.x IP (older IP: global `SRCR_VBR`); IMR only on disable |
| Reload-done interrupt | `RRIF/RRIE` exists and the HAL exposes `HAL_LTDC_ReloadEventCallback` | defined in `ltdc.c` but the IRQ thread does not test it |
| Software stack | Zephyr `display` API, one layer, no DRM, no atomic commit, no TEST_ONLY | Linux DRM atomic with planes, zpos, alpha, `TEST_ONLY` |
| Clock and burst | pixel clock from PLL3R on the boards read; no burst-length register in the layer block | `st,burstlen`, `LTDC_L1BLCR`, pad max 150 MHz (MP25) |
| Firewall, secure layers | none | RIFSC firewalls on `l1l2`, `l3` and `rot` |
| Memory | M7 data cache, AXI SRAM or FMC SDRAM, MPU regions, no IOMMU | Linux DMA, CMA, DDR |
| 2D helper | **DMA2D** (blend, PFC, fill) next to the LTDC | no DMA2D; the MP25 relies on its own layers and the rotator |

Reading across for the hwlayer-manager model (inferred): H7 is the *small, old end* of the LTDC family: two layers, fixed order, no scaling, no YUV, and a single global shadow-reload register. The MP257 backend's profile fields for scaling (`max_downscale` 1.0, upscale only) and `zpos` map to "no scaling" and "fixed order" here, but the MP257 YUV and rotation rows have no H7 counterpart.

## 9. Sources that could not be fetched

| Document | Result |
| --- | --- |
| ST AN4861, "LCD-TFT display controller (LTDC) on STM32 MCUs" (PDF on `st.com`) | **Not reachable.** First attempt: "Permission to use WebFetch has been denied". After the `st.com` domain was allowed, two further attempts each ended with `timeout of 60000ms exceeded`; not retried with variants. Its section 4.6 is cited in Zephyr's Kconfig (`Kconfig.stm32_ltdc:88`), which is the only knowledge of it here. |
| ST RM0433 (STM32H742/H743/H753/H750 reference manual) | **Not reachable**, same denial and then the same timeouts (a very large PDF). The LTDC and DMA2D chapters are unread. |
| ST product pages, data sheets | Not attempted. |

Every hardware statement that rests on the RM (maximum resolution, exact IMR timing, bus the LTDC masters, blend equation, whether `RRIF` is set for IMR, position of line 0 in the frame) is therefore **inferred** or open.

## 10. Open questions

1. **Shadow reload timing.** Does an `SRCR_IMR` request take effect mid-frame or at the next line? Zephyr uses IMR from the line-0 interrupt, so a tear-free swap depends on this. RM0433 and AN4861 need to be read.
2. **Where line 0 sits.** `LIPCR = 0` fires at which point of the frame (start of vertical sync, start of back porch, or start of the active area)? `CPSR & 0xFFFF == 0` is the only test the driver uses.
3. **VBR on H7.** Is a VBR-based reload strictly better for a two-layer atomic update? A backend would stage both layers with the `_NoReload` setters and issue one `SRCR_VBR`, then wait for `RRIF`. No Zephyr code does this.
4. **Hardware version of the H7 LTDC** (the Linux `LTDC_IDR` numbering) and whether H7 parts other than H743 have the same layer block and one or two layers (`LTDC_Layer2_BASE` guard).
5. **Layer 2 and Zephyr.** Zephyr hard-wires layer 1. Is the right shape for a backend a new driver, direct `hal_stm32` calls beside the display driver, or a Zephyr change? Zephyr's `LayerCfg[0]` and the `LTDC_LAYER_1` constant in the ISR would need generalising.
6. **DMA2D.** No driver exists in Zephyr. Is a DMA2D helper in scope, how would its use be modelled in `Capabilities` (a compositing resource outside the planes), and what is its bandwidth effect on LTDC underrun (`AMTCR` dead time)?
7. **Cache and MPU.** Which MPU attribute should the framebuffer region have (write-through, non-cacheable, or cacheable plus explicit clean), and how do clean and invalidate interact with a DMA2D destination? `ATTR_MPU_RAM` is what the boards use; its cacheability was not read.
8. **Memory placement.** AXI SRAM versus SDRAM for 480x272 and larger: bandwidth limits and FMC speculative-read behaviour (AN4861 section 4.6) need the RM and a measurement.
9. **Unread Zephyr boards** (`stm32h735g_disco`, `stm32h745i_disco`, `stm32h757i_eval`, `stm32h7p5i_dk`, `stm32h7s78_dk`) and the DSI shields: not read. `stm32h7p5i_dk` and `stm32h7s78_dk` may carry a different LTDC.
10. **Citations.** Zephyr and HAL line numbers were checked against clones. Still unchecked: `cmsis-device-h7` (not cloned), the `hal_stm32` module copy, and board DTS ranges that were not re-grepped.
11. **Panel part numbers** for the 480x272 RGB panels on `stm32h7b3i_dk` and `stm32h750b_dk` (the DTS and docs read do not give them).
12. **Board for a hardware step.** No board is on the farm. If the owner adds an STM32H7 board, `stm32h7b3i_dk` (has LTDC panel, SDRAM and an enabled DMA2D in Zephyr) is the closest match to what was read; this is an input to the owner's decision, not a decision.
