# i.MX RT1170 display path (eLCDIF, LCDIFv2, PXP), read from source (SoW §5.4)

Date: 2026-11-28. **No claim here comes from a board run.** No device was used
and none exists on the farm for this target. Every row is tagged
`read from source` (the cited file and line say it) or `inferred` (a conclusion
drawn from what was read, or a statement that needs a document that was not
available). Rows say what the code does, not what a running RT1170 does.
This is the first step of SoW §5.4 ("NXP i.MX RT1170 / LCDIFv2", subject to
board availability; open-issues M3-06): the SoC's display topology, before any
backend is written. Style follows [imx8mp-lcdif-source.md](imx8mp-lcdif-source.md)
and [am62p-dss-source.md](am62p-dss-source.md).

## Trees

| Tree | Ref | Commit | Status |
|---|---|---|---|
| `zephyrproject-rtos/zephyr` (shallow, depth 1) | `main` at clone time, no tag | `bc833f469ee653b7755881b8850792a4e14bacbb` | read: `drivers/display/display_mcux_elcdif.c`, `display_mcux_lcdifv2.c`, `Kconfig.mcux_elcdif`, `Kconfig.mcux_lcdifv2`, `dts/bindings/display/nxp,imx-lcdifv2.yaml`, `dts/arm/nxp/imxrt/imxrt11xx/*.dtsi`, `boards/nxp/mimxrt1170_evk/*`, `boards/shields/{rk055hdmipi4m,rk055hdmipi4ma0,rk043fn02h_ct,g1120b0mipi}`, `drivers/dma/dma_mcux_pxp.c`, `include/zephyr/drivers/display.h`, `soc/nxp/imxrt/imxrt11xx/soc.c` |
| `zephyrproject-rtos/hal_nxp` (shallow, depth 1) | `master` at clone time, no tag | `eaffbf4ea1c272988a8a4cbe5c001ad45e0965fd` | read: `mcux/mcux-sdk-ng/drivers/{lcdifv2,elcdif,pxp}/*.h`, `mcux/mcux-sdk-ng/devices/RT/RT1170/MIMXRT1176/MIMXRT1176_cm7_features.h` |

In the tables, `Z:` means the Zephyr commit above and `H:` means the hal_nxp
commit above; paths are relative to the repository root, except that `H:` paths
drop the prefix `mcux/mcux-sdk-ng/`. `nxp-mcuxpresso/mcux-sdk` was not cloned:
hal_nxp carries the same driver sources, and one tree is enough for a desk
study.

**Not available.** The i.MX RT1170 Reference Manual (`IMXRT1170RM`) and the
related application notes were not fetched: the one attempt to reach the NXP
document host was refused by the session, and NXP's manuals are in any case
behind a login. So no register-level fact (shadow-load timing, layer stacking
order, scaler presence, bus masters, cache attributes of OCRAM) is read from a
manual. Where a claim needs it, it is tagged `inferred` and appears again in
"Open questions". The HAL headers' own doc comments are used as the nearest
source and are cited as such.

## 1. Controllers and routing

| Fact | Where | Tag |
|---|---|---|
| The RT1176 SoC tree has two display controllers: `lcdif` (`compatible = "nxp,imx-elcdif"`, base 0x40804000, IRQ 54) and `lcdifv2` (`"nxp,imx-lcdifv2"`, base 0x40808000, IRQ 55), both `status = "disabled"` in the SoC file | Z: `dts/arm/nxp/imxrt/imxrt11xx/nxp_rt11xx.dtsi:1046-1060` | read from source |
| The `lcdif` node links the PXP through `nxp,pxp = <&pxp>`; the PXP node is `"nxp,pxp"` at 0x40814000, IRQ 57, `#dma-cells = <0>` | Z: `nxp_rt11xx.dtsi:1050`, `:1099-1104` | read from source |
| The RT1175 variant deletes `lcdif`, `lcdifv2`, `pxp`, `mipi_dsi`: "the whole DISPLAY power domain is absent" | Z: `dts/arm/nxp/imxrt/imxrt11xx/nxp_rt1175_cm7.dtsi:11-20` | read from source |
| The MIMXRT1170-EVK (cm7) selects one controller at a time: `zephyr_lcdif: &lcdif {}` with the comment "Change to lcdifv2 to switch controller" | Z: `boards/nxp/mimxrt1170_evk/mimxrt1170_evk_mimxrt1176_cm7.dts:86-87` | read from source |
| For the MIPI DSI output the SoC code sets the video mux bit by which driver is enabled: eLCDIF clears `MIPI_DSI_SEL`, LCDIFv2 sets it | Z: `soc/nxp/imxrt/imxrt11xx/soc.c:778-783` | read from source |
| So the two controllers are alternatives for one DSI output in Zephyr's configuration; whether both can drive different outputs at once is a hardware question | `soc.c:778-783` | inferred (open: needs the RM video-mux chapter) |
| Each controller gets its own root clock programmed from the panel's `clock-frequency` | Z: `soc.c:629-652` | read from source |

## 2. (a) eLCDIF versus LCDIFv2: layers, formats, alpha, scaling, order

### 2.1 eLCDIF (`nxp,imx-elcdif`)

| Fact | Where | Tag |
|---|---|---|
| The HAL exposes one frame buffer: only `ELCDIF_SetNextBufferAddr` (writes `NEXT_BUF`) selects pixels; no layer index exists in the API | H: `drivers/elcdif/fsl_elcdif.h:404-409` | read from source |
| The SoC feature header says `FSL_FEATURE_LCDIF_HAS_NO_AS` (no alpha surface) for the RT1176 | H: `devices/RT/RT1170/MIMXRT1176/MIMXRT1176_cm7_features.h:620` | read from source |
| RGB-mode pixel formats in the HAL: RAW8, RGB565, RGB666 (unpacked), XRGB8888, RGB888 | H: `fsl_elcdif.h:156-161` | read from source |
| Zephyr's eLCDIF driver advertises only RGB565, XRGB8888 (alpha byte unused), RGB888 | Z: `drivers/display/display_mcux_elcdif.c:34-35` | read from source |
| No alpha, blend, scaling or ordering control exists on this controller in the HAL or driver | `fsl_elcdif.h`, `display_mcux_elcdif.c` | inferred (the absence is read; the hardware's absence of them needs the RM) |
| Summary: eLCDIF is a single-buffer scan-out engine: 1 layer, opaque, no scaler | all rows above | inferred |

### 2.2 LCDIFv2 (`nxp,imx-lcdifv2`)

| Fact | Where | Tag |
|---|---|---|
| The RT1176 has 8 LCDIFv2 layers and 2 layer colour-space converters (`FSL_FEATURE_LCDIFV2_LAYER_COUNT (8)`, `..._LAYER_CSC_COUNT (2)`), 2 interrupt domains, and a CLUT RAM at offset 0x2000 | H: `MIMXRT1176_cm7_features.h:629-635` | read from source |
| The HAL has per-layer calls: size, offset, buffer config, buffer address, enable, background colour, blend config, colour-space mode | H: `drivers/lcdifv2/fsl_lcdifv2.h:462`, `:476`, `:489`, `:500`, `:512`, `:549`, `:561`, `:572` | read from source |
| Layer pixel formats: LUT index 1/2/4/8 bpp, RGB565, ARGB1555, ARGB4444, YUV 4:2:2 packed (UYVY, VYUY, YUYV, YVYU), RGB888 (24-bit packed), ARGB8888, ABGR8888 | H: `fsl_lcdifv2.h:148-167` | read from source |
| No planar or semi-planar YUV format (NV12 and the like) is listed | H: `fsl_lcdifv2.h:148-167` | read from source |
| Per-layer alpha mode: disable, override (use global alpha), embedded (use pixel alpha), Porter-Duff | H: `fsl_lcdifv2.h:180-185` | read from source |
| The blend config holds an 8-bit `globalAlpha` and, for Porter-Duff, an alpha mode, colour mode, global-alpha mode (scaled alpha among them) and factor mode | H: `fsl_lcdifv2.h:187-245` | read from source |
| Porter-Duff operators named in the HAL: Src, Atop, Over, In, Out, Dst, DstAtop, DstOver, DstIn, DstOut, Xor, Clear, with `SrcLayer` and `DestLayer` roles | H: `fsl_lcdifv2.h:247-269` | read from source |
| Each layer has a position (offset) and a size, so layers are rectangles on the panel, not full-screen | H: `fsl_lcdifv2.h:462-480` | read from source |
| No scaler call exists in the HAL: layer size is the source size (no separate destination size), and no filter, scale or decimation control is declared | H: `fsl_lcdifv2.h` (whole file; the only match for "scal" is the Porter-Duff `kLCDIFV2_PD_ScaledAlpha` alpha mode at `:213`) | read from source |
| So LCDIFv2 does no per-layer scaling | above | inferred (RM not read) |
| The HAL header offers no explicit z-order control: layer index is the only ordering handle; which index is on top is not stated in the header | H: `fsl_lcdifv2.h` | read from source (absence); which index is on top: inferred, open |
| Colour keying: none declared in this HAL header | H: `fsl_lcdifv2.h` | read from source (absence) |

Mapping to the SoW WP0 scenes (all **inferred**): LCDIFv2's 8 rectangle layers
with per-layer global or pixel alpha could host "two UI layers" and "UI +
priority layer" without a 2D copy. "Video + UI" is limited to packed YUV 4:2:2
video (no NV12). "Unsupported geometry" cases are those that need scaling:
the planner would have to refuse them or route them through the PXP.

## 3. (b) PXP as a 2D helper

| Fact | Where | Tag |
|---|---|---|
| The PXP is a memory-to-memory engine, not part of the scan-out path: it is a separate node (`nxp,pxp`, `#dma-cells = <0>`) and Zephyr drives it as a DMA device | Z: `nxp_rt11xx.dtsi:1099-1104`; `drivers/dma/dma_mcux_pxp.c:23`, `:182` | read from source |
| Rotation 0/90/180/270 with an option to rotate the output buffer or the process surface; flips: horizontal, vertical, both | H: `drivers/pxp/fsl_pxp.h:276-296`, `:1736` | read from source |
| Rotating the process surface "cannot be used together with flip, scale, or decimation" | H: `fsl_pxp.h:285-287` | read from source |
| Scaling of the process surface is available | H: `fsl_pxp.h:1587` (`PXP_SetProcessSurfaceScaler`) | read from source |
| Alpha-surface blending, Porter-Duff, overlay colour key, and one CSC (CSC1); the RT1176 features header says `HAS_NO_CSC2`, `HAS_NO_LUT`, no dither | H: `fsl_pxp.h:1417`, `:1443`, `:1917`, `:2104`; `MIMXRT1176_cm7_features.h:973-987` | read from source |
| Input formats include ARGB8888, ARGB1555/4444, RGB565, packed and semi-planar YUV 4:2:0/4:2:2; outputs include ARGB8888, RGB888, RGB565 and YUV | H: `fsl_pxp.h:311-327`, `:348-375`, `:401-412` | read from source |
| Zephyr uses the PXP only for rotation and flip, on formats RGB565, RGB888 and ARGB8888; the DMA driver configures the process surface, the output buffer, position, `PXP_SetRotateConfig`, and starts it | Z: `dma_mcux_pxp.c:70-125`, `:149-182` | read from source |
| The eLCDIF driver calls the PXP in `display_write` only when `CONFIG_MCUX_ELCDIF_PXP` is on and the input is a full-size frame; otherwise it logs "PXP rotation/flip will not work correctly unless a full sized framebuffer is provided" | Z: `display_mcux_elcdif.c:155-224`; `Kconfig.mcux_elcdif:46-56` | read from source |
| The PXP is blocking from Zephyr's side: `display_write` waits on a semaphore given by the PXP completion callback | Z: `display_mcux_elcdif.c:219` | read from source |
| The PXP therefore substitutes for the scaling and rotation that neither controller has, at the cost of a memory pass and an extra buffer; it cannot sit between a layer and the panel | rows above | inferred |
| The LCDIFv2 driver does not use the PXP at all | Z: `display_mcux_lcdifv2.c` (no PXP include) | read from source |

## 4. (c) Shadow registers, latch, and VSYNC semantics

| Fact | Where | Tag |
|---|---|---|
| LCDIFv2 layer configuration is shadowed: writes go to shadow registers; `LCDIFV2_TriggerLayerShadowLoad` sets `SHADOW_LOAD_EN` in `CTRLDESCL5` of that layer. The HAL comment says the shadowed registers "are updated to the active control registers on VSYNC of next frame" | H: `fsl_lcdifv2.h:524-538` | read from source |
| The latch is per layer (the trigger takes a `layerIndex`), not per frame across layers | H: `fsl_lcdifv2.h:535-538` | read from source |
| The CLUT upload takes a `useShadowLoad` flag: with it, the data goes through the shadow and needs the trigger; without it, "loaded to hardware directly" | H: `fsl_lcdifv2.h:429-443` | read from source |
| LCDIFv2 interrupts: vertical blanking ("start of vertical blanking period"), VSYNC edge, output underrun, and per layer FIFO-empty, DMA-done, DMA-error, over 2 domains | H: `fsl_lcdifv2.h:80-108`; features `:631` | read from source |
| Zephyr's `display_write` on LCDIFv2 sets the buffer address, enables the vertical-blanking interrupt, enables the display, triggers the shadow load, and then blocks on a semaphore | Z: `display_mcux_lcdifv2.c:116-134` | read from source |
| The ISR clears the status, and on vertical blanking disables that interrupt and gives the semaphore; so Zephyr treats "shadow loaded / frame handed to scan-out" as "the vertical-blanking interrupt fired once after the trigger" | Z: `display_mcux_lcdifv2.c:226-243` | read from source |
| "Latched" for this controller therefore means: the shadow-load request was taken at a frame boundary, after which the new address is active. The driver's own proof is only the blanking interrupt, not a read-back of an active-register or a flag | `display_mcux_lcdifv2.c:130-134`; `fsl_lcdifv2.h:524-535` | inferred |
| Whether several layers' triggers apply atomically at one VSYNC (a per-layer trigger could in principle land layers on different frames) is not stated by the header | `fsl_lcdifv2.h:535` | inferred (open: needs the RM) |
| eLCDIF has no shadow or layer trigger: the driver writes `NEXT_BUF`; the ISR on `CurFrameDone` checks `CUR_BUF` equals the active buffer before giving the semaphore, so "latched" means the engine has started scanning the queued buffer | Z: `display_mcux_elcdif.c:226-227`, `:412-427` | read from source |
| eLCDIF interrupts: frame done, VSYNC edge, TX FIFO underflow; surfaced as display events `FRAME_DONE`, `VSYNC`, `FIFO_UNDERFLOW` | H: `fsl_elcdif.h:121-124`; Z: `display_mcux_elcdif.c:346-348`, `:428-445` | read from source |
| The LCDIFv2 driver registers no event callback and reports no supported events: its API struct has no `register_event_cb` | Z: `display_mcux_lcdifv2.c:294-301` | read from source |

## 5. (d) Framebuffer memory, alignment, cache, DMA visibility

| Fact | Where | Tag |
|---|---|---|
| LCDIFv2 layer buffer addresses must be 64-byte aligned (`LCDIFV2_FB_ALIGN 64U`; the comment says "Layer address shall be 64-byte aligned") | Z: `display_mcux_lcdifv2.c:30-31`, `:177` | read from source |
| eLCDIF next-buffer address must be double-word aligned (HAL assert on bits 2:1); Zephyr's driver also allocates its own buffers 64-byte aligned | H: `fsl_elcdif.h:404-409`; Z: `display_mcux_elcdif.c:304` | read from source |
| Driver-owned frame buffers come from a `k_heap` sized by `CONFIG_MCUX_LCDIFV2_FB_NUM * CONFIG_MCUX_LCDIFV2_FB_SIZE + 512`; default FB_NUM is 1 (range 0-2) for LCDIFv2, 2 for eLCDIF; default FB_SIZE 3686400 (1280x720x4) | Z: `display_mcux_lcdifv2.c:25`; `Kconfig.mcux_lcdifv2:14-35`; `Kconfig.mcux_elcdif:16-37` | read from source |
| With 0 driver buffers the application hands `display_write` a full-panel buffer and the driver points the layer at it directly; partial updates need driver buffers | Z: `display_mcux_lcdifv2.c:73-79`, `:81-83`; `Kconfig.mcux_lcdifv2:19-25` | read from source |
| Both drivers clean the D-cache by range over the active frame buffer before pointing the controller at it, when `CONFIG_HAS_MCUX_CACHE` is set | Z: `display_mcux_lcdifv2.c:111-113`; `display_mcux_elcdif.c:151-153` | read from source |
| The RT11xx SoC selects `HAS_MCUX_CACHE` and, for the Cortex-M7, an I-cache and D-cache | Z: `soc/nxp/imxrt/imxrt11xx/Kconfig:10-14` | read from source |
| The cm7 EVK's default `zephyr,sram` is `sdram0`, an external SEMC SDRAM node of 64 MiB at 0x80000000; the board enables `NXP_IMX_EXTERNAL_SDRAM` and external memory config data for the M7 | Z: `mimxrt1170_evk_mimxrt1176_cm7.dts:24`; `mimxrt1170_evk.dtsi:363-367`; `Kconfig.defconfig:9-13` | read from source |
| The EVK file warns that enabling the SEMC driver would software-reset the controller and clear the SDRAM set-up the boot ROM programmed while the kernel runs from SDRAM | Z: `mimxrt1170_evk.dtsi:350-359` | read from source |
| Consequently, with the stock board configuration, a driver framebuffer from `k_heap` lands in whatever region the heap's section maps to; with SDRAM as the default RAM that is the cached SDRAM, which is why the D-cache clean is needed | rows above | inferred |
| The OCRAM region at 0x20200000 exists in the board tree (`/soc/ocram@20200000` is named in the defconfig); no display code in these trees places a frame buffer there explicitly | Z: `Kconfig.defconfig:30` | read from source (absence of explicit placement: inferred) |
| The driver only cleans the cache (CPU writes to memory); it never invalidates, which is correct for a scan-out-only buffer. After a PXP pass, the eLCDIF path cleans before the PXP and does not invalidate afterwards, because the CPU does not read the output | Z: `display_mcux_elcdif.c:151-224` | inferred |
| Cache attributes of OCRAM and SDRAM (MPU regions, write-back or write-through, TCM non-cacheable), bus bandwidth between SDRAM and the controller, and whether the SEMC can sustain several full-screen layers | not read | inferred (open; needs the RM and the board MPU table) |

## 6. (e) What Zephyr exposes today

| Fact | Where | Tag |
|---|---|---|
| Three NXP display drivers for this SoC family exist: `display_mcux_elcdif.c`, `display_mcux_lcdifv2.c` (plus `display_mcux_lcdifv3.c`, `display_mcux_dcnano_lcdif.c`, `display_nxp_dcif.c` for other SoCs) | Z: `drivers/display/` listing; `Kconfig.mcux_lcdifv2:4-10` | read from source |
| **An LCDIFv2 driver does exist**, with copyright "2026 NXP" and compatible `nxp,imx-lcdifv2`; it is new and small (352 lines) | Z: `display_mcux_lcdifv2.c:2`, `:7` | read from source |
| It drives layer 0 only: `LCDIFV2_LAYER_INDEX 0U`; init sets layer size to the panel size and offset (0,0) and enables that one layer | Z: `display_mcux_lcdifv2.c:27-28`, `:276-289` | read from source |
| It never calls the blend, colour-space, background, or any other layer's functions, so alpha is not configured (layer 0 uses whatever reset state gives) and layers 1-7 stay unused | Z: `display_mcux_lcdifv2.c` (whole file) | read from source |
| Formats exposed: RGB565, ARGB8888, RGB888 | Z: `display_mcux_lcdifv2.c:33-34`, `:191-197` | read from source |
| `set_pixel_format` frees and re-allocates the driver buffers, then sets the layer stride and format | Z: `display_mcux_lcdifv2.c:157-202` | read from source |
| `blanking_on/off` toggle the whole display output (`LCDIFV2_EnableDisplay`), not a layer and not a backlight | Z: `display_mcux_lcdifv2.c:139-155` | read from source |
| `set_orientation` only accepts NORMAL; `get_capabilities` reports the panel size, supported and current format | Z: `display_mcux_lcdifv2.c:204-224` | read from source |
| The eLCDIF driver additionally provides `get_framebuffer`, `register_event_cb`, `unregister_event_cb`; its blanking toggles the backlight GPIO and returns `-ENOSYS` without one | Z: `display_mcux_elcdif.c:261-283`, `:508-517` | read from source |
| The display API surface is one buffer per device: mandatory `write`; optional `blanking_*`, `read`, `clear`, `get_framebuffer`, brightness, contrast, `get_capabilities`, `set_pixel_format`, `set_orientation`, timing, event callbacks. There is no layer, plane, alpha, z-order, or atomic-commit call | Z: `include/zephyr/drivers/display.h:560-660` (struct at `:619`) | read from source |
| The API has `PIXEL_FORMAT_ARGB_8888`, `XRGB_8888`, `MONO01`, `L_8`, `AL_88` bits; no YUV format | Z: `display.h:77-160` | read from source |
| DT binding `nxp,imx-lcdifv2` includes `lcd-controller.yaml` and `pinctrl-device.yaml`, with `reg`, `interrupts` required and an optional `backlight-gpios`; `width`, `height`, `pixel-format` and a `display-timings` child come from the included binding and are read by the driver | Z: `dts/bindings/display/nxp,imx-lcdifv2.yaml`; `display_mcux_lcdifv2.c:311-332`, `:340` | read from source |
| Consequence for the contract: through Zephyr's display API the RT1170 offers exactly one layer on either controller; the 8 LCDIFv2 layers, alpha and Porter-Duff are reachable only by writing to the HAL directly (a new driver or a bypass) | rows above | inferred |

## 7. (f) Board configurations

| Fact | Where | Tag |
|---|---|---|
| `mimxrt1170_evk` has a cm7 target (`mimxrt1170_evk_mimxrt1176_cm7`, plus `_A`/`_B` overlays and an `enet_qos` variant) and a cm4 target | Z: `boards/nxp/mimxrt1170_evk/` listing | read from source |
| Both controllers get the same pinctrl group `pinmux_lcdif` | Z: `mimxrt1170_evk.dtsi:223-231`; `mimxrt1170_evk-pinctrl.dtsi:259` | read from source |
| The PXP is `status = "okay"` on the board | Z: `mimxrt1170_evk.dtsi:341-343` | read from source |
| The default controller is eLCDIF through the label `zephyr_lcdif`; shields refer to that label | Z: `mimxrt1170_evk_mimxrt1176_cm7.dts:86-87` | read from source |
| `rk055hdmipi4ma0` (5.5" 720x1280 MIPI DSI panel, Himax HX8394, GT911 touch): board files exist for `mimxrt1170_evk_mimxrt1176_cm7` (overlay, conf, defconfig); panel 720x1280, RGB565 default, 24-bit bus | Z: `boards/shields/rk055hdmipi4ma0/boards/`; `rk055hdmipi4ma0.overlay:7-84` | read from source |
| The cm7 overlay for it raises the DSI PHY clock to 792 MHz; its conf sets `CONFIG_LV_Z_VDB_SIZE=20`, and the defconfig selects LVGL partial-refresh rendering | Z: `rk055hdmipi4ma0/boards/mimxrt1170_evk_mimxrt1176_cm7.overlay:7-14`, `.conf`, `.defconfig` | read from source |
| `rk055hdmipi4m` (same panel size, Raydium RM68200): its board directory has no `mimxrt1170_evk` file (only `mimxrt595_evk`); its overlay points `zephyr,display` at `&lcdif`; its doc page still lists `mimxrt1170_evk//cm7` as a build target | Z: `boards/shields/rk055hdmipi4m/boards/`; `rk055hdmipi4m.overlay:11`; `doc/index.rst:61` | read from source |
| `rk043fn02h_ct` (4.3" 480x272 parallel RGB panel, FT5336 touch): `zephyr,display = &zephyr_lcdif`, RGB565, 16-bit bus; the doc lists `mimxrt1060_evk` as its board | Z: `boards/shields/rk043fn02h_ct/rk043fn02h_ct.overlay:11`, `:27-31`; `doc/index.rst:98` | read from source |
| `g1120b0mipi` (400x392 round MIPI panel, RM67162; its overlay makes the DSI panel node the display) has a cm7 `_A` and `_B` overlay for the EVK | Z: `boards/shields/g1120b0mipi/g1120b0mipi.overlay:9`, `:45-53`; `boards/mimxrt1170_evk_mimxrt1176_cm7_A.overlay` | read from source |
| The panel `width`/`height`/`pixel-format` come from the LCDIF node in the shield overlay, so the controller node's size is the panel's size | Z: `rk055hdmipi4ma0.overlay:35-38` | read from source |
| The board's parallel RGB output is wired to the MIPI DSI bridge for the MIPI panels; a LVDS or HDMI route is not configured by these shields | shield overlays | inferred |
| A board to evaluate would be the MIMXRT1170-EVK (or EVKB) with an RK055 MIPI panel on the MIPI connector; no such board is on this farm | rows above | inferred |

## Topology summary (for the later items)

All of the following are `inferred` from the rows above unless the row says so.

- eLCDIF: 1 layer, no alpha, no scaler, formats RGB565/RGB888/XRGB8888; latch = next-buffer address at frame done.
- LCDIFv2: 8 layers, rectangles with offset and size; formats include ARGB8888, ARGB4444, ARGB1555, RGB565, RGB888, packed YUV 4:2:2, CLUT index; no NV12.
- Alpha: per-layer disable, override (global), embedded, Porter-Duff (LCDIFv2 only).
- Scaling: none in either controller (RM unread); the PXP can scale and rotate in memory.
- Order: layer index (which index is on top is open).
- Latch: LCDIFv2 per-layer shadow load, applied at the next frame's VSYNC; blanking interrupt is Zephyr's completion signal.
- PXP: rotate, flip, scale, alpha blend, Porter-Duff, one CSC; memory to memory only.
- Memory: 64-byte aligned buffers; D-cache clean before scan-out; default RAM is external SDRAM on the EVK.
- Zephyr: eLCDIF driver mature (events, PXP rotate); LCDIFv2 driver exists but drives layer 0 only, no blend, no events.
- Board: MIMXRT1170-EVK cm7 plus RK055 MIPI shields; the RT1175 has no display block.

## Open questions

1. **Layer stacking order** of the 8 LCDIFv2 layers, and whether a higher index is on top: needs the RM.
2. **Atomicity** of per-layer shadow loads: can all layers' triggers be applied on one VSYNC, or can a pair of layers land on different frames? Needs the RM, then a board.
3. **Scaling**: confirm that LCDIFv2 has no scaler and no per-layer destination size distinct from the source size.
4. **Memory**: MPU attributes of OCRAM and SDRAM on the cm7, SEMC bandwidth for full-screen layers, whether a layer buffer in OCRAM needs no cache clean.
5. **Both controllers at once**: whether eLCDIF and LCDIFv2 can drive different outputs together (the video-mux rows suggest a shared DSI route only).
6. **Zephyr**: whether upstream will extend `display_mcux_lcdifv2.c` beyond layer 0; the contract would need a new bypass or driver otherwise.
7. **Documents**: the i.MX RT1170 Reference Manual and the LCDIFv2/PXP application notes were not reachable; this doc should be re-read against them before any backend is written.
