# Upstream Zephyr DPU driver on the i.MX95 M7: scanout and VSYNC/latch path

Source study for SoW §5.1 / WP3 / M2 (framebuffer scanout and VSYNC/latch
reporting from Zephyr on the i.MX95 M7). Written 2026-11-28. **No device was
touched and nothing here is a hardware claim.** Every statement is tagged:

- **read from source**: the cited file and line says it.
- **inferred**: reasoning from the source; no hardware or documentation backs it.

## Sources and how they were read

| What | Pin |
|---|---|
| Zephyr `main` | commit `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (2026-11-27, "tests: benchmark: remove platform_key filter"), https://github.com/zephyrproject-rtos/zephyr/tree/e53873cbd0ef45d63ef9ded1a45d10717e7c71d7 |
| `hal_nxp` | commit `1a8937f3f8dfe5c68f59cc2a0c0c0fccdb096650`, the revision `west.yml` pins for `hal_nxp` at the Zephyr commit above (`west.yml:215-216`), https://github.com/zephyrproject-rtos/hal_nxp/tree/1a8937f3f8dfe5c68f59cc2a0c0c0fccdb096650 |

Method: both trees were read from local checkouts whose `HEAD` is exactly the
two commits above (checked 2026-11-28), so every line number below is exact
for those commits. The `DISP.H` and `DTSI` numbers were re-counted the same
way.

**Which HAL directory is built.** `hal_nxp` has two DPU directories under
`mcux/mcux-sdk-ng/drivers/`: `dpu/` and `dpu_1/`. Zephyr's
`modules/hal_nxp/mcux/mcux-sdk-ng/drivers/drivers.cmake:116` is
`set_variable_ifdef(CONFIG_DISPLAY_SOCIONEXT_DPU CONFIG_MCUX_COMPONENT_driver.dpu_1)`,
and only `dpu_1/CMakeLists.txt:5,8` builds `dpu_1/fsl_dpu.{c,h}` for that
symbol (`dpu/CMakeLists.txt:5` is guarded by `driver.dpu`, which nothing I
found sets for this driver). **Read from source:** the i.MX95 path uses
`dpu_1/`; everything labelled **HAL** below is `dpu_1/` only. `dpu/` defines
none of the `DISPLAY_SEERIS_Type` or unit names the Zephyr driver uses and was
not used for any statement here.

**Which SoC variant.** HAL feature switches select register layouts. For
`MIMX9596` M7, `MIMX9596_cm7_features.h:262` defines
`FSL_FEATURE_DISPLAY_SEERIS_MDR5` and `:264` `..._HAS_LD (0)`; the MDR7 switch
is not defined for it (it is defined for i.MX937/i.MX952 headers). So the
**MDR5 (`#else`)** branches of `dpu_1` apply. `MIMX9596_cm7_COMMON.h:1560`:
`DISPLAY_SEERIS_BASE (0x4B400000u)`. (Both under
`mcux/mcux-sdk-ng/devices/i.MX/i.MX95/MIMX9596/`.) Whether the Verdin iMX95's
SoC is the `MIMX9596` variant is **not examined**; the Zephyr DT is for
`MIMX9596`.

Paths are relative to the Zephyr tree unless stated. Abbreviations:
**DRV** `drivers/display/display_socionext_dpu.c`; **DTSI**
`dts/arm/nxp/imx/nxp_imx95_m7.dtsi`; **DISP.H**
`include/zephyr/drivers/display.h`; **DSI** `drivers/mipi_dsi/dsi_nxp_dwc.c`;
**HAL.c** / **HAL.h**
`modules/hal/nxp/mcux/mcux-sdk-ng/drivers/dpu_1/fsl_dpu.c` / `fsl_dpu.h`.
Other files read: `drivers/display/Kconfig.socionext_dpu`,
`dts/bindings/display/socionext,dpu.yaml`,
`boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts`.

## 1. What the driver is (read from source)

- Compatible `socionext,dpu`; driver is `DRV`, Kconfig `DISPLAY_SOCIONEXT_DPU`,
  default `y` when a node is enabled (`Kconfig.socionext_dpu:4-9`). Binding
  says "socionext DPU display controller for DSI panels", requires `reg` and
  `interrupts` and includes `lcd-controller.yaml` (`socionext,dpu.yaml:4-15`).
- It is a thin wrapper over the NXP MCUX DPU HAL (`#include <fsl_dpu.h>`,
  `DRV:14`); every register access is a HAL call. The driver itself contains no
  register offsets.
- Copyright header is 2026 NXP (`DRV:2`): the driver is new and has no
  history of use that this study can see.
- DTSI node: `dpu: dpu@4b400000` (`DTSI:1093`), `reg = <0x4b400000 0x1000>`
  (`:1095`), `height = <0>`, `width = <0>`, `interrupts = <215 0>` (`:1098`),
  `status = "disabled"`. `width`/`height` are zero: a board must override them
  and add a `display-timings` child (`DRV:365-370` read `display-timings`).
- The only board that mentions it is `imx95_evk_mimx9596_m7.dts`, which only
  creates the labels `zephyr_dpu`, `zephyr_mipi_dsi`, `display_i2c`
  (`:302-306`); no board sets `status = "okay"` on the DPU, DSI or display
  irqsteer. No `samples/drivers/display/boards/` overlay for i.MX95 exists.
  A search of `tests/` and `doc/` was not done, so "no test enables it" is
  **not established**.
- No i.MX LDB/LVDS driver or binding was found by file name in
  `drivers/display/`, `drivers/mipi_dsi/`, `dts/bindings/display/`,
  `dts/bindings/mipi-dsi/`. The only display output path wired to the DPU is
  the DSI host below. **Inferred:** a LVDS panel (the Verdin 10.1" panel in the
  SoW) has no upstream Zephyr output path today.

## 2. Pipeline programming (read from source)

`dpu_init` (`DRV:209-330`) builds a fixed pipeline. Order and calls:

1. Enable the IRQ in the NVIC (`irq_config_func`, `DRV:215`, defined by
   `IRQ_CONNECT`/`irq_enable` at `DRV:348-353`).
2. Derive pixel format from DT `pixel-format` (RGB565/RGB888/ARGB8888 only,
   `DRV:23-24`, `218-231`); stride = width x bytes rounded up to 32
   (`DRV:21`, `190-192`).
3. `DPU_Init` then `DPU_PreparePathConfig` (`DRV:256-257`).
   - `DPU_Init` (`HAL.c:1074-1095`) makes **no clock call**. For MDR5 it only
     ORs `DOMAINMASK_ENABLE` into the Store9, ExtDst0/4/1/5 domain-mask
     registers (`HAL.c:1089-1093`). Its comment says the related root clocks
     "were enabled in dpu_board.c" (`HAL.c:1076-1077`), a file I did not
     look for. The only clock-related code in `HAL.c` is an empty
     `#if !FSL_SDK_DISABLE_DRIVER_CLOCK_CONTROL` block (`HAL.c:983-985`).
   - `DPU_PreparePathConfig` (`HAL.c:1139-1175`) disables shadow loading on
     the pipelines Store9, ExtDst0/1/4/5 (`:1141-1149`), sets every listed
     pixel-engine unit's source to 0: FetchDecode9, Hscaler9/4, Vscaler9/4,
     Rop9, BlitBlend9, Store9, ExtDst0/4/1/5, FetchRot9, FetchYuv0/1/3,
     LayerBlend1-5 and, for MDR5, LayerBlend6 and FetchYuv2
     (`:1152-1161`), then ORs the sticky shadow-load-request bit into
     FetchLayer0/1 (`:1172-1173`). The list contains **no ConstFrame, no
     FetchWarp and no LayerBlend0**.
4. `DPU_InitPipeline(kDPU_PipelineExtDst0)` (`DRV:260`): `HAL.c:1274-1285`
   clears the pipeline `STATIC` POWERDOWN bit and sets `SHDEN`
   (`DPU_UNIT_SHDEN_MASK`), i.e. powered, **shadow load enabled**. It writes
   no clock divider; the "single mode" wording is only in the doc comment.
5. `DPU_InitExtDst(kDPU_ExtDst0, source = LayerBlend1)` (`DRV:263`):
   `HAL.c:2163-2177` sets `STATICCONTROL = KICKMODE | SHDEN` ("Set KICK_MODE to
   external", `:2171-2172`) and the unit source (`DPU_SetUnitSrc`,
   `HAL.c:1431`). External kick means ExtDst0 is started by the FrameGen kick
   in step 10 (**inferred** from the comment and from the kick registers
   below).
6. `DPU_InitLayerBlend(kDPU_LayerBlend1, sources = ConstFrame0 + FetchYUV0)`,
   default blend config, enabled (`DRV:266-270`; `HAL.c:1547`). ConstFrame0 is
   the *primary*, FetchYUV0 the *secondary*; ConstFrame0 is black, alpha 0
   (`DRV:275-280`). The source codes are `kDPU_UnitSrcConstFrame0 = 12`
   (`HAL.h:497`), `kDPU_UnitSrcLayerBlend1 = 20` (`HAL.h:501`) and, for MDR5,
   `kDPU_UnitSrcFetchYUV0 = 29` (`HAL.h:522`; the MDR7 value 28 is at
   `HAL.h:510`).
7. `DPU_InitDomainBlend(kDPU_DomainBlend0)` (`DRV:273`): `HAL.c:2531-2544`
   sets software shadow-token generation and software shadow load, shadow
   enabled, and `BLENDCONTROL = 0x10`.
8. FetchYUV0 configured for the full frame, one sub-layer (sub-layer 0): bits
   per pixel, RGB pixel format (`kDPU_PixelFormatRGB565/888/ARGB8888`), stride,
   buffer width/height, `baseAddr = fb[0]`, zero offset, buffer enabled
   (`DRV:282-297`; `DPU_InitFetchUnit` `HAL.c:1873`). "YUV" is the HAL unit
   name; it is programmed with an RGB format here.
9. Trigger the first shadow load (`DRV:300`), enable the shadow-load interrupt
   (`DRV:302`), then `DPU_TriggerDisplayDbShadowLoad(kDPU_DomainBlend0)`
   (`DRV:305`). The function is declared at `HAL.h:1556` and defined at
   `HAL.c:2555-2564`: it writes `CONTROLTRIGGER = ShdTokGen` on the DomainBlend
   unit, i.e. it generates the DomainBlend shadow-load token. (The older
   `dpu/` directory is not the one built; see above.)
10. FrameGen/display timing: `DPU_InitDisplayTiming(base, 0, ...)` with
    `vfp - 1` and `vbp + 1` (`DRV:316-318`), then `DPU_SetDisplayConfig` with
    `kDPU_DisplayOnlyPrim` and `primAreaStartX/Y = 1` (`DRV:320-325`), then
    `DPU_TriggerDisplayShadowLoad(base, 0)` (`DRV:327`).
    - `DPU_InitDisplayTiming` (`HAL.c:2303-2359`) sets `FGSTCTRL` shadow
      enable, the dither polarity register, `HTCFG1/2`, `VTCFG1/2`, and
      `PKICKCONFIG`/`SKICKCONFIG` (`:2352-2358`, comment "Kick signal set to
      start of last vertical blanking line"). Both kick registers get row
      `vtotal + 1` (with `vtotal = height + vfp + vbp + vsw - 1`, `:2320-2323`)
      and column `width + hfp + hbp + hsw`, enable bit set. **In `dpu_1` there
      is no TCON or signal-mixer programming** (no `TCON`/`SMX` text in
      `HAL.c`/`HAL.h`); how the pixel stream leaves the DPU is not in this HAL
      file (**not examined**).
    - `DPU_TriggerDisplayShadowLoad` (`HAL.c:2501-2508`) writes FrameGen
      `FGSLR = ShdTokGen`; its doc comment says the shadow registers load "at
      the begining of next frame".
11. The display is **not started in init**; `DPU_StartDisplay`
    (`HAL.c:2466-2473`, `FGENABLE = FgEn`) runs on the first `display_write`
    (`DRV:55-56`, `118-122`).

Pipeline, as the driver wires it (**read from source**):

```
ConstFrame0 (black) ─┐
                     ├─ LayerBlend1 ── ExtDst0 ── DomainBlend0 ── Display0/FrameGen0 ── (DSI)
FetchYUV0 (fb) ──────┘
```

## 3. Scanout: where the framebuffer lives (read from source)

- Driver framebuffers are a static `__aligned(32)` array in the image,
  `CONFIG_DPU_FB_NUM * height * ROUND_UP(width*4, 32)` bytes, sized for 4 Bpp
  (`DRV:339-345`). `DPU_FB_NUM` is 0..2, default 2 (`Kconfig.socionext_dpu:13-24`).
  The array is in M7 `.bss`; the DPU is given its address as a 32-bit value
  (`(uint32_t)(uintptr_t)`, `DRV:125-126`, `294`). Which memory that is (TCM,
  DDR) depends on the board linker setup, which this study did not read
  (**not examined**). **Inferred:** the DPU master must be able to reach that
  address, and the M7's TCM aliases differ from the system view, so a DDR
  region assigned to the M7 is the likely requirement. Nothing in the driver
  checks or documents this.
- Zero-copy path: if `x==0 && y==0`, the descriptor equals the panel size, the
  source stride equals the driver stride and the buffer is 32-byte aligned,
  then `active_fb = buf` (the caller's buffer is scanned out directly,
  `DRV:82-87`). Otherwise the driver copies into one of its own framebuffers
  (`DRV:88-116`), and returns `-ENOTSUP` if `DPU_FB_NUM == 0` (`DRV:90-92`).
- With `DPU_FB_NUM == 0` the DPU scans out `fb_ptr` regardless; the Kconfig
  help warns it shows "a dump of system RAM" until the first write
  (`Kconfig.socionext_dpu:26-31`).
- The driver does **no cache maintenance** (no cache-clean call appears in
  `DRV`). **Inferred:** with D-cache enabled on the M7, software-rendered
  pixels may not reach memory before the DPU fetches them; whether the M7
  cache is on for that memory depends on board config, not read here.
- Only a full-frame address swap exists: `get_framebuffer` returns the active
  buffer (`DRV:152-157`); there is no `set_framebuffer` in the display API
  (`display_get_framebuffer` only, DISP.H).

## 4. Latch: how a new address takes effect (read from source)

`dpu_write` ends with (`DRV:124-134`):

1. `DPU_SetFetchUnitSrcBufferAddr(FetchYUV0, sublayer 0, addr)`:
   `HAL.c:2518-2523` writes the sub-layer `BASEADDRESS0` register.
2. `DPU_TriggerPipelineShadowLoad(kDPU_PipelineExtDst0)`: `HAL.c:1311-1318`
   writes the pipeline `TRIGGER` register with `SYNC_TRIGGER`.
3. `k_sem_take(&data->sem, K_FOREVER)`: blocks until the ISR gives the
   semaphore.

So: the registers written in step 1 are shadowed (shadow load was enabled by
`DPU_InitPipeline`, section 2 step 4); the `SYNC_TRIGGER` bit in step 2 asks
the DPU to copy the shadow set to active. **Inferred:** the copy happens at
the next point the kick (section 2 step 10: last line of vertical blanking,
last column) causes ExtDst0 to start a frame, not immediately; this is what
makes the flip tear-free, but the exact timing is not documented in anything
read here. The kick row is `vtotal + 1` (`HAL.c:2353`) and the column is the
total line width (`:2354`), so the latch point is at the end of the frame's
last blanking line; the TRIGGER-to-latch latency is therefore up to one frame,
**inferred**.

Other shadow triggers used: `DPU_TriggerDisplayShadowLoad` (FrameGen `FGSLR`,
`DRV:327`) and the DomainBlend token (`DRV:305`, section 2 step 9). Init-time
only.

Buffer-reuse hazard (**inferred**): the semaphore is taken after the trigger
but the previous buffer is only safe to reuse once the shadow load happened,
which the semaphore does mark; with two driver buffers (`next_idx`,
`DRV:129-132`) the copy at `DRV:93-101` into the "next" buffer relies on that.
Zero-copy callers get no signal about which of their buffers is on screen
except that `display_write` has returned.

## 5. Interrupts and VSYNC/frame-done exposure

### What the driver does (read from source)

- One IRQ, from DT `interrupts = <215 0>` (`DTSI:1098`, `DRV:350-352`;
  priority from DT).
- `dpu_isr` reads pending flags of interrupt **group 0**, clears them, and
  gives the semaphore if `kDPU_Group0ExtDst0ShadowLoadInterrupt` is set
  (`DRV:197-207`). It records nothing else: no timestamp, counter or callback.
- The only DPU interrupt enabled is that ExtDst0 shadow-load flag
  (`DPU_EnableInterrupts(APP_DPU_DC, 0, ...)`, `DRV:302`;
  `kDPU_Group0ExtDst0ShadowLoadInterrupt = 1<<3`, `HAL.h:328`, common to
  both variants). ExtDst0 FrameComplete (`HAL.h:329`), DomainBlend0
  FrameComplete (`HAL.h:341`), DiSengcfg FrameComplete0 and the FrameGen0
  Int0-3 flags are not enabled.
- The interrupt registers live in `DPU_COMCTRL_Type` (`HAL.c:19-30`):
  `IRQ_LOCKUNLOCK` at offset 0x1000, `IRQ_INTERRUPTENABLE[3]`,
  `PRESET[3]`, `CLEAR[3]`, `STATUS[3]`. The HAL helpers address them at
  `base + DPU_COMCTRL_OFFSET` (`HAL.c:1199`, `:2232`, `:2257`) and
  `DPU_COMCTRL_OFFSET` is `0x0000U` in the layout used here (`HAL.h:140`).
  So they sit at `base + 0x1000 ...`.
- **Base-address discrepancy.** `dpu_isr` and the enable call use a hard-coded
  `APP_DPU_DC` of `0x4B780000` (`DRV:17`, `202-203`, `302`), while every other
  call uses the DT `reg` base `0x4b400000` (`DRV:360`), which equals the HAL's
  `DISPLAY_SEERIS_BASE` for MIMX9596 M7 (`MIMX9596_cm7_COMMON.h:1560`). The
  two differ by `0x380000`. The MDR5 unit offsets in `dpu_1` end at about
  `0x371020` (`DPU_DITHER1_CONFIG_OFFSET`, `HAL.h:206`), so the interrupt
  registers the ISR touches would be at `0x4B781000...`, past the block the
  HAL describes for this SoC. `0x380000` does equal the MDR7 Dither1 offset
  (`HAL.h:185`), which looks like coincidence. **Unresolved:** nothing read
  explains it. **Inferred:** either `0x4B780000` is another alias of the same
  block that this study cannot confirm (no reference manual was read), or it is
  a latent bug (a leftover from a different board) that would leave the ISR
  never seeing a flag and the first `display_write` blocked forever. It must
  be settled on hardware or with the reference manual, not from these files.
- The DT IRQ 215 is `&nvic 215`, which `DTSI` lists as
  `disp_irqsteer_master1` (`DTSI:1042`); that irqsteer (`:1022`) and its
  masters and `mipi_dsi` (`:1010`) are `status = "disabled"`.
  **Inferred:** the DPU IRQ is only delivered if a board enables the display
  irqsteer and the steer channel is routed; the driver does not do that.
- The `disp_irqsteer` node carries `power-domains = <&disp_mix>` (`DTSI:1027`)
  and `disp_mix` is an SCMI power-domain, `reg = <0xd>` (`DTSI:120`).

### Interrupt flags that exist for the i.MX95 (read from source)

`enum _dpu_interrupt` in `HAL.h` holds all groups in one enum, with two
variants of group 0 (selected by `FSL_FEATURE_DISPLAY_SEERIS_MDR7`,
`HAL.h:342`/`:411`). That is why `kDPU_Group0FrameGen0Int0Interrupt` is
defined twice: bit 20 for MDR7 (`HAL.h:346`) and bit 21 for MDR5 (`HAL.h:416`),
and likewise `Int1-3`, `DiSengcfg*0` (MDR5 bits 18-20, `HAL.h:413-415`) and
`DomainBlend0SeqComplete` (MDR5 bit 17, `HAL.h:412`). For the MIMX9596 M7
(MDR5) the group-0 candidates are therefore:

| Event | Flag | Bit | Source |
|---|---|---|---|
| ExtDst0 shadow load (**latch**, used) | `kDPU_Group0ExtDst0ShadowLoadInterrupt` | 3 | `HAL.h:328` |
| ExtDst0 frame complete | `kDPU_Group0ExtDst0FrameCompleteInterrupt` | 4 | `HAL.h:329` |
| DomainBlend0 shadow load / frame complete | `...DomainBlend0ShadowLoad/FrameCompleteInterrupt` | 15 / 16 | `HAL.h:340-341` |
| DiSengcfg frame complete 0 | `kDPU_Group0DiSengcfgFrameComplete0Interrupt` | 19 | `HAL.h:414` |
| FrameGen0 Int0-3 | `kDPU_Group0FrameGen0Int0..3Interrupt` | 21-24 | `HAL.h:416-419` |

Group 1 has FrameGen0 primary-sync on/off (`HAL.h:383-384`) and the
overflow/under-run flags (`:385-389`). There is **no** enumerator named
`...Display0FrameComplete...` and none named VSYNC. `dpu_1` has no function
that programs the FrameGen interrupt-config register: `INTCONFIG[4]` exists
only as a struct member (`HAL.c:276`) and is never referenced, so enabling
FrameGen Int0-3 on a given line needs a direct register write or a HAL patch
(**inferred** from the absence of any use; what value selects "per frame" is
**not examined**).

### What the display API offers (read from source)

The brief said the display API has no vblank callback. At this commit that is
only half right:

- `struct display_capabilities.supported_events` (DISP.H:394) and
  `enum display_event` with `DISPLAY_EVENT_LINE_INT`, `DISPLAY_EVENT_VSYNC`,
  `DISPLAY_EVENT_FRAME_DONE`, `DISPLAY_EVENT_FIFO_UNDERFLOW` (`:461-469`);
  `struct display_event_data` carries a `timestamp`, a line, and a
  `buffer_id` (`:444-456`); callback type `display_event_cb_t` (`:497`),
  `display_register_event_cb()`/`display_unregister_event_cb()`
  (`:1023-1071`) which return `-ENOSYS` when `supported_events` is 0 and
  `-EINVAL` for an unsupported mask; driver ops `register_event_cb`,
  `unregister_event_cb` (`:675-679`).
- The word "vblank" does not occur in the header. There is no
  `set_framebuffer`/page-flip call; a flip is `display_write`.
- The DPU driver implements none of it: `dpu_api` has only
  `set_pixel_format`, `write`, `get_capabilities`, `get_framebuffer`
  (`DRV:332-337`); `dpu_get_capabilities` does not set `supported_events`
  (`DRV:139-150`), and `display_get_capabilities` zeroes the struct first
  (DISP.H:891). Applications therefore see **no** VSYNC or frame-done
  event from this driver; the only completion signal is `display_write`
  returning after the shadow-load interrupt.

## 6. Clocks, power domains, resets (read from source)

- The DPU node has **no** `clocks`, `power-domains`, `resets` or
  `pinctrl` properties (`DTSI:1093-1100`), and the driver calls no
  clock-control, power-domain or reset API (`DRV` has no such include beyond
  `gpio.h`, which it does not use).
- The HAL does **not** enable any clock either: `DPU_Init` only sets domain
  masks (section 2 step 3, `HAL.c:1074-1095`) and `HAL.c` has no
  `CLOCK_EnableClock` call. So in this driver **no software enables the DPU
  or pixel clocks**; `HAL.c:1076-1077` says that is done in a board file this
  study did not find (**not established** whether anything in Zephyr does it).
- No code in `DRV` or `DSI` programs the display **pixel clock** or a
  display PLL. **Inferred:** the pixel clock has to be set up by something
  else (SM defaults, a board init, or the application), and no upstream
  Zephyr sample demonstrates it.
- SCMI services the M7 DTS declares (`DTSI:73-114`): SCMI over MU5
  (`mu5`, `DTSI:894-896`), shared memory at `0x44611000` (`:69`), protocols
  power (0x11), system (0x12), clock (0x14), pinctrl (0x19), and NXP CPU
  (0x82). Power domains: `disp_mix` (0xd) and `netc_mix` (0x12).
- `mipi_dsi` (`DTSI:1010-1020`) uses `clocks = <&scmi_clk 0>` (`:1017`),
  `nxp,dc = <&dpu>` and `interrupt-parent = <&disp_irqsteer_master0>`
  (`:1015`), interrupt 48. `DSI` uses the clock only for
  `clock_control_set_rate` of the PHY configuration clock (`DSI:224-233`),
  and under `CONFIG_SOC_MIMX9596_M7` writes directly to
  `CAMERA__DSI_OR_CSI_PHY_CSR`, `CAMERA__DSI_MASTER_CSR` and
  `DISPLAY__BLK_CTRL_DISPLAYMIX->PIXEL_LINK_CTRL` (`DSI:108-120`).
  **Inferred:** those blk-ctrl writes presuppose that the System Manager
  grants the M7 access to DISPLAYMIX and CAMERAMIX; that is the ownership
  question of the feasibility work, not answered here.
- The `DSI` file shows no reset or power-domain calls.

## 7. DSI pairing, LVDS gap (read from source / inferred)

- `dsi_nxp_dwc` takes width, height and porches from the DPU node's
  `display-timings` child via `nxp,dc` (`DSI:235-259`). It is a DSI **host**
  and attaches a DSI peripheral; it is the only output the DPU driver is
  paired with.
- Panel/bridge drivers present for DSI (e.g. ADV7535, HX8394, ILI98xx,
  RM67162) are named in the listing, not read here.
- No LDB/LVDS driver/binding was found (section 1). **Inferred:** the
  Mallow-carrier LVDS panel path through the DISPLAYMIX LDB is not covered by
  upstream Zephyr; a reduced-scope bring-up would need either a DSI-to-LVDS
  bridge path (not part of this board, **not examined**) or an out-of-tree LDB
  driver.

## 8. What is missing

### (a) Reporting a VSYNC/latch timestamp

All **inferred** unless noted:

1. The framework hook exists (`supported_events`, `register_event_cb`,
   `display_event_data.timestamp`, section 5) but the DPU driver does not
   implement it (read from source, section 5). A driver change is needed:
   fill `supported_events`, implement `register_event_cb`/`unregister_event_cb`.
2. The ISR must take the timestamp itself (e.g. `k_cycle_get_32()`) at the top
   of `dpu_isr`; today it only gives a semaphore (`DRV:204-206`), so
   scheduling latency of the `display_write` caller is added to any
   timestamp taken after `k_sem_take`.
3. Which hardware event to report is a choice the driver does not make:
   ExtDst0 shadow-load (the **latch**, already enabled), or one of ExtDst0 /
   DomainBlend0 FrameComplete, DiSengcfg FrameComplete0 or FrameGen0 Int0-3
   (**frame boundary**, none enabled; flags and bits in the section 5 table,
   MDR5 numbering for the MIMX9596 M7). Which of them is aligned to the
   panel's VSYNC is not stated in the files read. To report both latch and
   frame boundary, both interrupt sources must be enabled and the ISR must
   tell them apart; FrameGen Int0-3 additionally need a direct write to the
   unused `INTCONFIG` registers (section 5).
4. Today's single `k_sem` (limit 1, `DRV:244`) with `K_FOREVER`
   (`DRV:134`) makes `display_write` synchronous. An engine that submits a
   frame and later learns when it was latched needs the event callback or a
   non-blocking submit; neither exists.
5. Resolve the `APP_DPU_DC` base discrepancy (section 5) before trusting the
   ISR to see any flag.
6. Display irqsteer, its master and routing must be enabled by the board
   (`status = "disabled"` in `DTSI`), and the SM must allow the M7 to take the
   interrupt (**not examined**).
7. Possible stale-semaphore race: init triggers shadow loads (`DRV:300`,
   `305`, `327`) and enables the interrupt (`DRV:302`) before the display runs
   (`DRV:118-122`). If any init-time load later raises the ExtDst0 flag, the
   first `display_write` could return before its own latch. Purely **inferred**
   from the call order.

### (b) A second plane

All **inferred** from the code, except where cited:

1. The pipeline is fixed: ConstFrame0 + FetchYUV0 into LayerBlend1 into
   ExtDst0 (`DRV:263-267`). A second plane needs another fetch unit (the HAL
   has FetchYuv1/2/3 and LayerBlend1-6 in `DPU_PreparePathConfig`,
   `HAL.c:1152-1161`) and the LayerBlend chain re-sourced; the driver never
   calls `DPU_InitFetchUnit` or `DPU_SetUnitSrc` for any other unit.
2. The display API has no plane concept: `display_write` takes one
   rectangle and one buffer for the whole panel, and the zero-copy path
   requires full panel size (`DRV:82-87`). Overlay would need either a
   driver-private extension API or a second `socionext,dpu` instance per
   plane, which the binding does not model (one node = one controller).
3. Per-plane format, position, alpha and blend mode live in HAL calls the
   driver does not expose (it passes `DPU_LayerBlendGetDefaultConfig`
   unchanged, `DRV:268-269`). No z-order, alpha or scaling is reachable
   through the display API.
4. A coherent two-plane update needs both planes' addresses written and one
   shadow trigger (one `TRIGGER` write latches the pipeline, section 4). The
   driver already has the single trigger point; the missing piece is exposing
   a second address write ahead of it.
5. Pixel format: the driver only offers RGB565/888/ARGB8888 (`DRV:23-24`);
   the NV12/YUYV formats in the display API are unsupported.

## 9. Pointers for the next items (inferred, not requirements)

- A reduced-scope M2 scanout test can use this driver's public path
  (`display_write` with a panel-sized, 32-byte-aligned buffer) only after
  board-level pieces the driver does not provide: DT overrides for
  `width/height/display-timings/pixel-format`, enabling `dpu`, display
  irqsteer, `mipi_dsi` (or an out-of-tree LVDS path), an M7-reachable
  framebuffer region, a pixel clock source, enabling the DPU clocks (the HAL
  does not), and SM permission for all of them.
- Latch timing can be measured from the ExtDst0 shadow-load IRQ with a
  timestamp hook in the ISR (a driver patch or a downstream copy), after the
  `APP_DPU_DC` question is settled; a frame-boundary event needs one of the
  extra interrupt sources in section 8(a)3.
- Do not claim parity with the i.MX95 Linux DPU path or with other SoCs from
  this study; no register behaviour here was confirmed.
