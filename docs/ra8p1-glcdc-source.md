# Renesas RA8P1 / RA8D1 GLCDC display path, read from source (SoW WP4, §5.4, M3-06)

Date: 2026-11-28. **No claim here comes from a board run.** No device was used,
and no RA8P1 or RA8D1 board exists on the farm. Every row is tagged
`read from source` (the cited repository, commit and file say it) or
`inferred` (a conclusion drawn from what was read). Every row names the SoC it
is about: **RA8P1**, **RA8D1**, or **both** (same file, no per-SoC branch seen).
Third-party code is cited by path and never copied. This is the first step of
SoW §5.4 ("evaluate the next MCU candidate"): the display topology, before any
backend exists.

## Trees and how they were read

| Tree | Commit | What was read |
|---|---|---|
| Zephyr (`zephyrproject-rtos/zephyr`), shallow clone of the default branch on 2026-11-28 | `bc833f469ee653b7755881b8850792a4e14bacbb` | `drivers/display/display_renesas_ra.c`, `drivers/display/Kconfig.renesas_ra`, `dts/bindings/display/renesas,ra-glcdc.yaml`, `dts/arm/renesas/ra/ra8/r7fa8d1xh.dtsi`, `r7ka8p1kflcac.dtsi`, `ra8x1.dtsi`, `ra8x2.dtsi`, `boards/renesas/ek_ra8d1/*`, `boards/renesas/ek_ra8p1/*`, `drivers/mipi_dsi/dsi_renesas_ra.c` |
| `zephyrproject-rtos/hal_renesas`, shallow clone of the default branch on 2026-11-28 | `31e99f1c15d817d87448b981287e8ebbbf754f76` | `drivers/ra/fsp/src/r_glcdc/r_glcdc.c` (first part only), `drivers/ra/fsp/inc/api/r_display_api.h`, `drivers/ra/fsp/inc/instances/r_glcdc.h`, `drivers/ra/fsp/src/bsp/mcu/ra8p1/bsp_feature.h`, `.../ra8d1/bsp_feature.h`, directory listings of `src/r_drw`, `src/bsp/mcu`, `src/bsp/cmsis/Device/RENESAS/Include` |
| `hal_renesas` as pinned by Zephyr's `west.yml` at the Zephyr commit above (`modules/hal/renesas`) | `f3d847daccb3fb94980e1222f05049082592dc29` | **not read**; every `hal_renesas` claim below is from HEAD `31e99f1c…`, and a difference to the pinned revision is unchecked |
| Renesas hardware manuals (RA8P1, RA8D1 user's manual, GLCDC chapter), app notes | none | **not read**: access to the Renesas site was only granted after the first draft, and no PDF was fetched in this pass. Every claim that would need a manual is `inferred` |
| `renesas/fsp` | none | not cloned; `hal_renesas` carries the FSP sources Zephyr builds |

Method limits, stated once because they bound every line citation:

- Both repositories were cloned shallow (`git clone --depth 1`) at the commits
  above after the first draft of this file, and line numbers were checked
  against the clones for `display_renesas_ra.c`, `r_glcdc.c`
  (2108 lines), `r_display_api.h` (453 lines), the two SoC `.dtsi` files and
  the two `bsp_feature.h` files. A line written plain (`:123`) was checked in
  the clone. A line written with `~` came from the first-pass read of a file
  that was not re-opened, and is good to about ±2 lines: this covers the
  board files, the Kconfig, the binding YAML, the DSI driver and
  the `r_glcdc.h` / `r_display_api.h` ranges not listed in the greps.
- Only part of the 6.9-million-character register header
  `R7KA8P1KF_core0.h` was opened; no register-offset claim rests on it.
- The Renesas manuals were not read (see the table above), so no claim about
  silicon behaviour is read from source; those are `inferred`.
- The repository tree used for the 2D-engine listing is the `hal_renesas`
  clone; "no D2 sources" means a name search of that clone found only
  `src/r_drw/r_drw_base.c` (a `d2_` match) and nothing else.

Paths below are relative to the repository root of the tree named in the row's
"Tree" cell: **Z** = Zephyr `bc833f46…`, **H** = `hal_renesas` `31e99f1c…`
(under `drivers/ra/fsp/`).

## 1. Instances and the SoC display blocks

| Fact | SoC | Where | Tag |
|---|---|---|---|
| One GLCDC node, `compatible = "renesas,ra-glcdc"`, label `lcdif`, at `0x40342000`, size `0x1454`, clock `lcdclk MSTPC 4`, `status = "disabled"` | RA8P1 | Z `dts/arm/renesas/ra/ra8/r7ka8p1kflcac.dtsi:82-87` | read from source |
| One GLCDC node, same compatible, label `lcdif`, at `0x40342000`, size `0x1454`, clock `lcdclk MSTPC 4`, `interrupts = <71 1>`, name `line`, disabled | RA8D1 | Z `r7fa8d1xh.dtsi:25-32` | read from source |
| A MIPI DSI host node `renesas,ra-mipi-dsi` at `0x40346000`, size `0x2000`, six interrupts named `sq0 sq1 vm rcv ferr ppi`, disabled | RA8D1 | Z `r7fa8d1xh.dtsi:34-43` | read from source |
| The same DSI node at the same address, clock `lcdclk MSTPC 10`, disabled; the SoC file has no interrupts for it | RA8P1 | Z `r7ka8p1kflcac.dtsi:89-96` | read from source |
| RA8P1 line and DSI interrupts are added in the board file instead: `lcdif` `<30 1>` name `line`; `mipi_dsi` `<24 12>…<29 12>` | RA8P1 | Z `boards/renesas/ek_ra8p1/ek_ra8p1_cm85.dts` (interrupt block, line not counted) | read from source |
| Neither the GLCDC nor the DSI node is enabled by any board file read; they stay `disabled` until an application overlay or shield enables them | both | Z `ek_ra8d1.dts`, `ek_ra8p1_cm85.dts` (no `okay` on `&lcdif`/`&mipi_dsi`) | read from source |
| Exactly one GLCDC instance, so one display pipeline; no second controller node exists in either SoC tree | both | Z `r7fa8d1xh.dtsi`, `r7ka8p1kflcac.dtsi`, `ra8x1.dtsi`, `ra8x2.dtsi` (no second glcdc) | inferred (absence of a node, not a manual statement) |
| RA8P1 has two cores (`_cm33`, `_cm85` SoC variants) and IPC mailboxes `mbox0`/`mbox1`; the board file used for the display is `_cm85` | RA8P1 | Z `r7ka8p1kflcac.dtsi` mailbox nodes; `ek_ra8p1_cm85.dts` | read from source |
| RA8P1 also lists a CSI capture block (`BSP_FEATURE_MIPI_CSI_IS_AVAILABLE` 1 vs RA8D1 0); DSI and PHY are 1 on both | RA8P1, RA8D1 | H `src/bsp/mcu/ra8p1/bsp_feature.h`, `ra8d1/bsp_feature.h` (macros reported by a summary of the first part of each file) | inferred |
| Both SoCs define `BSP_FEATURE_BSP_HAS_GRAPHICS_DOMAIN (1UL)`; neither `bsp_feature.h` has a GLCDC feature macro, so FSP has no per-SoC GLCDC branch | both | H `src/bsp/mcu/ra8p1/bsp_feature.h`, `ra8d1/bsp_feature.h` | inferred (only the start of each file was read verbatim) |
| FSP builds one `r_glcdc.c` for both SoCs, so the driver code path is identical; whether the silicon differs (layer count, maximum size, clock) needs the manuals | both | H `src/r_glcdc/` holds only `r_glcdc.c` | read from source (shared file); the silicon claim is inferred and open |

## 2. GLCDC layers, background, order (a)

| Fact | SoC | Where | Tag |
|---|---|---|---|
| The API has two graphics layers: `DISPLAY_FRAME_LAYER_1` and `DISPLAY_FRAME_LAYER_2`; the config holds `input[2]` and `layer[2]` | both (shared API) | H `inc/api/r_display_api.h` `display_frame_layer_t` `:50-51`; `display_cfg_t:310-326` | read from source |
| Each layer has a background colour (`bg_color`), a coordinate rectangle, a fade control and a fade speed | both | H `r_display_api.h` `display_layer_t` `:297` (end of struct) | read from source |
| A separate background plane exists as a register block `R_GLCDC->BG` (`EN`, `MON`) beside the two `GR[n]` blocks | both | H `r_glcdc.c:487`, `:553`, `:616` | read from source |
| Blend modes are private to `r_glcdc.c`, not in the public API: transparent (1), non-transparent (2), on-lower-layer (3); the public layer struct has no blend field | both | H `r_glcdc.c:159-161`, used at `:668`, `:673`, `:1449`, `:1551` | read from source |
| Layer order is fixed by hardware as layer 1 below layer 2 on the background; the API has no z-order setter | both | H `r_display_api.h` (no order field in `display_layer_t`) | inferred |
| The layer coordinate is a rectangle with a start offset; the FSP limits are horizontal start 5-2028, vertical start 2-2029, active width 16-2039, height 16-2043, total per line 23-2047, per frame 19-2047 | both | H `r_glcdc.c` (limit macros near the top, not re-read) | inferred |
| A layer rectangle is a window of the layer's own buffer shown at that position: there is no output-size field, so no scaling | both | H `r_display_api.h` `display_input_cfg_t` (`hsize`, `vsize`, `hstride`, no destination size) | read from source |
| `hsize` and `vsize` of the input buffer equal the shown size; the layer can show a line-descending (flipped) or line-repeat (vertical line doubling) buffer via `line_descending_enable`, `lines_repeat_enable`, `lines_repeat_times` | both | H `r_display_api.h` `display_input_cfg_t` `:269` (end of struct) | read from source |
| The contract consequence: two plus background, fixed order, no scaling, no per-layer zpos choice | both | rows above | inferred |

## 3. Pixel formats, per-layer alpha, fade, chroma key, CLUT (a)

| Fact | SoC | Where | Tag |
|---|---|---|---|
| Input formats: ARGB8888, RGB888 (32-bit), RGB565, ARGB1555, ARGB4444, CLUT8, CLUT4, CLUT1 | both | H `r_display_api.h` `display_in_format_t` `:76-84` | read from source |
| Output formats: RGB888 24-bit, RGB666 18-bit, RGB565 16-bit, 8-bit serial | both | H `r_display_api.h` `display_out_format_t` `:91-94` | read from source |
| No YUV, no planar or semi-planar input format is listed | both | H `r_display_api.h` `:76-84` | read from source |
| Per-pixel alpha is available only in the ARGB input formats; there is no per-layer constant-alpha field in `display_layer_t` | both | H `r_display_api.h` `:297` (end of struct) and `:76-84` | read from source |
| A fade control gives a layer a timed alpha ramp: `NONE`, `FADEIN`, `FADEOUT`, with a status `NOT_UNDERWAY`, `FADING_UNDERWAY`, `PENDING`, driven by a frame counter; this is the only constant-alpha path in the API | both | H `r_display_api.h` `:128-130` (control enum; status enum not re-checked); `r_glcdc.c` (`g_fade_pending`) | read from source (enums); inferred (frame-counter detail) |
| Chroma key: types `display_color_keying_t`, `display_colorkeying_cfg_t` (source colour, destination colour, enable) and `display_colorkeying_layer_t` (one per layer) exist, and `colorKeySet` is in the API table | both | H `r_display_api.h` ~142-146, ~243-254 | read from source |
| `R_GLCDC_ColorKeySet` just returns `FSP_ERR_UNSUPPORTED`: chroma key exists as API types but is not implemented in FSP | both | H `r_glcdc.c:838-846` | read from source |
| CLUT: 256 entries per CLUT (`GLCDC_PRV_CLUT_ENTRY_SIZE`); `ClutUpdate` refuses while an edit is latched (`FSP_ERR_INVALID_UPDATE_TIMING`) and `start >= 256` or `size > 256` gives `FSP_ERR_INVALID_CLUT_ACCESS`; two CLUT planes per layer exist in `glcdc_clut_plane_t` | both | H `r_glcdc.c:39`, `:764-766`, `:793-820`; `r_glcdc.h` `glcdc_clut_plane_t` | read from source; that an update flips a select bit is inferred |
| Brightness 0-1023 (default 512), contrast 0-255 (default 128), gamma with 16 elements per colour, dithering (truncate, round-off, 2x2 pattern), all gated by a build option `GLCDC_CFG_COLOR_CORRECTION_ENABLE` | both | H `r_display_api.h` ~195-233; `r_glcdc.h` extended config | read from source (types); inferred (ranges and build option) |

## 4. The 2D helper: DRW / D/AVE 2D (b)

| Fact | SoC | Where | Tag |
|---|---|---|---|
| FSP ships a `src/r_drw/` directory with `r_drw_base.c`, `r_drw_base.h`, `r_drw_memory.c`; no `r_drw.h` in `inc/instances`, no `d2_*` sources and no `rm_*` D2 wrapper found in the listings read | both | H listing of `drivers/ra/fsp/src/r_drw`, `inc/instances` | read from source (listings); a search of the whole `hal_renesas` clone for `d2_` found only `src/r_drw/r_drw_base.c` |
| Zephyr has a `drw` node (`drw@40344000`, RA8D1 base file) with no interrupts or clocks and `status = "disabled"` | RA8D1 | Z `dts/arm/renesas/ra/ra8/ra8x1.dtsi` (drw node) | read from source |
| Zephyr has a `drw` node at `drw@40444000`, size `0xd4`, disabled; a different address from RA8D1 | RA8P1 | Z `ra8x2.dtsi` (end of the soc node) | read from source |
| The boards add the interrupt: RA8D1 `<90 12>` name `drw`; RA8P1 `<31 12>` | RA8D1, RA8P1 | Z `ek_ra8d1.dts` (`&drw`); `ek_ra8p1_cm85.dts` (`&drw`) | read from source |
| `display_renesas_ra.c` has no DRW or D2 call, so the display driver never uses the 2D engine | both | Z `drivers/display/display_renesas_ra.c` (whole file) | read from source (reported absent by the reader, not grepped) |
| No Zephyr 2D-engine API consumer was found in the files read; a D/AVE 2D port would use the FSP D2 library, which is not in `hal_renesas` as far as the listings show | both | listings above | inferred |
| As a helper for the model: a 2D engine can compose or convert into one framebuffer, which gives a "fall back to the 2D engine" path when the hardware has fewer layers than the scene | both | n/a | inferred |

## 5. Latch semantics: register update, line detect, VSYNC (c)

| Fact | SoC | Where | Tag |
|---|---|---|---|
| `R_GLCDC_Start` (`:531`) sets `BG.EN_b.VEN = 1` (`:553`), then enables the vertical-position detect `SYSCNT.DTCTEN_b.VPOSDTC = 1` (`:557`) | both | H `r_glcdc.c:531-557` | read from source |
| `R_GLCDC_LayerChange` and `R_GLCDC_BufferChange` return `FSP_ERR_INVALID_UPDATE_TIMING` while `GR[layer].VEN_b.PVEN` or `BG.EN_b.VEN` is still set, then write the layer registers and set `PVEN = 1` | both | H `r_glcdc.c:616-617`, `:624` (LayerChange); `:659-660`, `:685` (BufferChange) | read from source |
| That the hardware applies the written values at the next VSYNC and clears `PVEN` is implied by the refusal and by the FSP doc comments; the register-level statement is in the manual | both | H `r_glcdc.c:602-690` | inferred |
| Meaning of "latched" in this IP, as the code shows it: software writes shadow-style layer registers, sets a per-layer valid bit (`PVEN`), the hardware applies them at the next frame boundary and clears the bit; a second write before that is refused, not queued | both | H `r_glcdc.c` (rows above) | inferred |
| Colour-correction updates use a separate latch: `R_GLCDC_ColorCorrection` checks `OUT.VLATCH_b.VEN` and `BG.EN_b.VEN` and sets the former; `R_GLCDC_Close` refuses while `BG.MON_b.VEN` is set | both | H `r_glcdc.c:726-734`, `:487` | read from source |
| A further function (`:906`) refuses while either layer's `PVEN`, `OUT.VLATCH_b.VEN` or `BG.EN_b.VEN` is set | both | H `r_glcdc.c:924-927` | read from source |
| Public API: `bufferChange` and `layerChange` per layer; no "commit all layers atomically" call, so two layers change at the next VSYNC only if both `PVEN` bits are set in the same frame | both | H `r_display_api.h` `display_api_t` | read from source (API list); atomicity is inferred |
| Events: `DISPLAY_EVENT_GR1_UNDERFLOW` (1), `GR2_UNDERFLOW` (2), `LINE_DETECTION` (3), `FRAME_END` (4) | both | H `r_display_api.h` `display_event_t` `:65-68` | read from source |
| The line-detect line is programmed at open time as back porch plus display cycles plus `GLCDC_PRV_BG_PLANE_HSYNC_POS_MIN`, into `GR[1].CLUTINT_b.LINE` | both | H `r_glcdc.c:422-424` | read from source |
| So the `LINE_DETECTION` event sits near the end of the active area, not at a pure VSYNC pulse; the exact position needs the macro value and the manual | both | H `r_glcdc.c:422-424` | inferred |
| The VPOS detect and interrupt enables are written in `r_glcdc_interrupt_enable` (`DTCTEN = 6`, `INTEN`); the comment there says VPOS is enabled in `R_GLCDC_Start` | both | H `r_glcdc.c:149`, `:2000-2001` | read from source |
| The config carries one line-detect IRQ and two underflow IRQs, each with a priority | both | H `r_display_api.h` `display_cfg_t:310-326` | read from source |
| Zephyr connects only the line-detect IRQ (`line`): `R_ICU->IELSR` mapped to `EVENT_GLCDC_LINE_DETECT`, `IRQ_CONNECT`, `irq_enable` | both | Z `display_renesas_ra.c:404-414` | read from source |
| Zephyr turns the underflow IRQs off (`BSP_IRQ_DISABLED`) and has no underflow handler | both | Z `display_renesas_ra.c:482-485` | read from source |
| The FSP callback gives a semaphore on `DISPLAY_EVENT_LINE_DETECTION`; `write()` waits on it with `K_FOREVER` after a buffer change | both | Z `display_renesas_ra.c:51-60`, `:141-145` | read from source |
| The GLCDC has no atomic multi-plane commit comparable to DRM's: per-layer valid bits, one VSYNC-aligned apply, and an event only at line detect | both | rows above | inferred |

## 6. Framebuffer memory, alignment, cache, DMA (d)

| Fact | SoC | Where | Tag |
|---|---|---|---|
| `R_GLCDC_BufferChange` returns `FSP_ERR_INVALID_ALIGNMENT` unless the base address is a multiple of 64 (`GLCDC_PRV_ADDRESS_ALIGNMENT_64B`) | both | H `r_glcdc.c` | read from source |
| The open-time check rejects a layer base address, and a stride in bytes, that is not a multiple of 64 (`FSP_ERR_INVALID_ALIGNMENT`); `:1489-1492` rounds the horizontal read size up to 64 | both | H `r_glcdc.c:1003-1008`, `:1489-1492` | read from source |
| Zephyr's `set_pixel_format` rounds the stride up to a 64-bit (8-byte) multiple, not 64 bytes | both | Z `display_renesas_ra.c:265-268` | read from source |
| Zephyr's framebuffer array is declared `__aligned(8)`, not 64, while FSP's open check (`r_glcdc.c:1003`) and `BufferChange` (`:655`) require 64; whether they pass depends on where the linker puts the array | both | Z `display_renesas_ra.c:436` | read from source; the mismatch conclusion is inferred |
| The framebuffer size is computed at build time from DT `width`, `height` and `input-pixel-format`, times `CONFIG_RENESAS_RA_GLCDC_FB_NUM` (0 to 2, default 2) | both | Z `display_renesas_ra.c` `:418-437`; `Kconfig.renesas_ra` ~16-19 | read from source |
| The framebuffer can go to an external RAM region through the DT `ext-ram` phandle; without it the default RAM `zephyr,sram` holds it | both | Z `renesas,ra-glcdc.yaml` ~128; `display_renesas_ra.c:421-423` | read from source |
| `CONFIG_RENESAS_RA_GLCDC_FRAME_BUFFER_SECTION` (string, example `.sdram`) exists in Kconfig but the driver file read does not reference it; the DT `ext-ram` path is what is used | both | Z `Kconfig.renesas_ra` ~26; `display_renesas_ra.c` | read from source |
| RA8P1 on-chip RAM nodes: `sram0` at `0x22000000` (1 MB) and `sram1` at `0x22100000` (640 KB), plus 128 KB ITCM and DTCM | RA8P1 | Z `r7ka8p1kflcac.dtsi:24-38` | read from source |
| A 1280x800 RGB565 buffer is about 2.0 MB and ARGB8888 about 4.1 MB, which do not fit `sram0` twice over; double-buffered UI therefore points at external RAM | RA8P1 | arithmetic over the sizes above | inferred |
| External RAM on the EK boards: an SDRAM node at `0x68000000`, 64 MB; RA8D1 16-bit bus, RA8P1 32-bit bus | RA8D1, RA8P1 | Z `ek_ra8d1.dts` (`sdram1`), `ek_ra8p1.dtsi` (`sdram1`) | read from source |
| OSPI flash windows: `0x90000000` on both EK boards (S28HL512T on RA8D1, MX25LW512 on RA8P1); SoC files map OSPI0 at `0x80000000` (512 MB) | RA8D1, RA8P1 | Z `ek_ra8d1.dts` (`&ospi0`), `ek_ra8p1.dtsi` (`&ospi0`), `ra8x1.dtsi`, `ra8x2.dtsi` | read from source |
| Cache: Zephyr's display driver has no `sys_cache_*` or D-cache call, so a caller's full-screen buffer goes to the GLCDC without a clean | both | Z `display_renesas_ra.c` (no cache call found by the reader; not grepped) | read from source (absence reported); the consequence is inferred |
| The Cortex-M85 has a data cache; with the framebuffer in cacheable RAM the CPU's writes may sit in the cache while the GLCDC bus master reads memory, so a backend needs a clean before the layer valid bit, or a non-cacheable region | RA8P1 (and RA8D1, M85 too) | n/a | inferred (M85 cache from the core, not from a read file; the manuals were unreachable) |
| Memory-bandwidth and bus-arbitration settings exist in the FSP config (`bus arbitration` enum in `r_glcdc.h`) | both | H `inc/instances/r_glcdc.h` | read from source (enum exists); effect inferred |

## 7. Zephyr `display_renesas_ra` driver (e)

| Fact | SoC | Where | Tag |
|---|---|---|---|
| Driver compat `renesas_ra_glcdc`; it uses only layer 1 (`input[0]`); layer 2 has `p_base = NULL` | both | Z `display_renesas_ra.c:7`, `:454-466` | read from source |
| Fade is `DISPLAY_FADE_CONTROL_NONE`; brightness, contrast and dithering are off at open | both | Z `display_renesas_ra.c:454-478` | read from source |
| `get_capabilities`: formats `RGB_888`, `ARGB_8888`, `RGB_565`; orientation normal only; no `set_orientation` | both | Z `display_renesas_ra.c:200-213` | read from source |
| `RGB_888` is stored as 32 bits per pixel (`DISPLAY_IN_FORMAT_32BITS_RGB888`) | both | Z `display_renesas_ra.c:236-239` | read from source |
| `set_pixel_format` is refused while the display is running (`-EWOULDBLOCK`) and when the framebuffer is too small | both | Z `display_renesas_ra.c:215-280` (`:230-233`, `:257`) | read from source |
| `write()` on a full-screen rectangle at (0,0) hands the caller's buffer to `R_GLCDC_BufferChange(..., LAYER_1)` with no copy; a partial write needs an internal buffer (`-ENOTSUP` when `FB_NUM` is 0), copied row by row | both | Z `display_renesas_ra.c:62-148` | read from source |
| The first `write()` calls `R_GLCDC_Start`; init does not start the display | both | Z `display_renesas_ra.c:131-139`, `:362-402` | read from source |
| `blanking_on` / `blanking_off` only toggle the backlight GPIO and return `-ENOTSUP` without it; the GLCDC keeps running | both | Z `display_renesas_ra.c:172-198` | read from source |
| `set_brightness` scales 0-255 to 0-1023 and refuses 0; `set_contrast` refuses 0; colour correction goes through `R_GLCDC_ColorCorrection` | both | Z `display_renesas_ra.c:289-340` | read from source |
| `get_framebuffer` returns the front buffer; the API table has no layer-count, no per-layer or blend call | both | Z `display_renesas_ra.c:343-360` | read from source |
| Zephyr's display API is one framebuffer: the driver exposes one layer, no layer API, no alpha or position control; layers 2, fade, chroma key and CLUT are reachable only through FSP calls the driver does not make | both | rows above | inferred |
| DT binding `renesas,ra-glcdc`: required `reg`, `clocks`, `interrupts`, `interrupt-names`, `input-pixel-format`, `output-pixel-format`; optional `pinctrl-0` (not needed with DSI), `backlight-gpios`, `output-pin-hsync/vsync/de` (`TCON_PIN_NONE`, `_0` to `_3`), `output-clock-divisor` (1-9, 12, 16, 24, 32), `output-endian`, `output-color-oder` (spelling as in the file), `output-data-signal-polarity`, `output-signal-sync-edge`, `def-back-color-*`, `ext-ram` | both | Z `dts/bindings/display/renesas,ra-glcdc.yaml` ~6-128 | read from source |
| The binding has no per-layer node and no layer-2 properties | both | same file | read from source |
| `CONFIG_RENESAS_RA_GLCDC` defaults to `y` when a `renesas,ra-glcdc` node is enabled and selects `USE_RA_FSP_DISPLAY` | both | Z `drivers/display/Kconfig.renesas_ra` ~6-10 | read from source |
| The driver builds when `CONFIG_RENESAS_RA_GLCDC` is set (`drivers/display/CMakeLists.txt`) | both | Z `drivers/display/CMakeLists.txt` (~59) | read from source |
| The driver passes `phy_layer = NULL` in the GLCDC extended config, even for DSI; the DSI host is a separate Zephyr driver | both | Z `display_renesas_ra.c:438-445` | read from source |
| MIPI DSI host driver `dsi_renesas_ra.c` (`renesas_ra_mipi_dsi`): `attach` and `transfer` only, no `detach`; video mode only; RGB888 and RGB565; DCS and generic writes; reads unsupported (`-ENOTSUP`); PHY PLL integer limits 20-180 for RA8D1 and 40-375 for RA8P1, other series `#error` | RA8D1, RA8P1 | Z `drivers/mipi_dsi/dsi_renesas_ra.c` ~7, ~44-95 | read from source |
| DSI binding `renesas,ra-mipi-dsi`: PLL (`pll-div`, `pll-out-div`, `pll-mul-int`, `pll-mul-frac`), `lp-divisor`, `timing`, `video-mode-delay`, `ulps-wakeup-period`, a `phys-timing` child | RA8D1, RA8P1 | Z `dts/bindings/mipi-dsi/renesas,ra-mipi-dsi.yaml` | read from source |
| Of the DSI IRQs only `sq0` is connected in Zephyr; the other five numbers go to FSP through the extended config | both | Z `dsi_renesas_ra.c` | read from source |

## 8. Boards and panels (f)

| Fact | SoC | Where | Tag |
|---|---|---|---|
| Zephyr has `boards/renesas/ek_ra8d1` and `boards/renesas/ek_ra8p1`; the latter has `_cm33` and `_cm85` variants | RA8D1, RA8P1 | Z `boards/renesas/` listing | read from source |
| Other RA8 boards exist (`aik_ra8d1`, `cpkcor_ra8d1b`, `ek_ra8d2`, `ek_ra8m1`, `ek_ra8m2`, `ek_ra8t2`, `fpb_ra8e1`, `mck_ra8t1`, `mck_ra8t2`); not read | other RA8 | Z `boards/renesas/` listing | read from source (names only) |
| `ek_ra8d1.dts` has no `zephyr,display` chosen; its defconfig has only GPIO, serial and console options; its pinctrl file has no GLCDC or LCD groups (it has `sdram_default`, `ospi0_default`, `ceu_default`) | RA8D1 | Z `ek_ra8d1.dts`, `ek_ra8d1_defconfig`, `ek_ra8d1-pinctrl.dtsi` | read from source |
| `ek_ra8d1` defines a MIPI connector (backlight, reset, interrupt, `iic1`) and a parallel-graphics expansion header on the same pins, and `alias mipi-dsi = &mipi_dsi` | RA8D1 | Z `ek_ra8d1.dts` | read from source |
| `ek_ra8d1` clocks: `lcdclk` from `pll` /2; `pll2` mul 80 (400 MHz `pll2p`) | RA8D1 | Z `ek_ra8d1.dts` (`&lcdclk`, `&pll2`) | read from source |
| `ek_ra8p1` clocks: `lcdclk` from `pll2r` /2 (480 MHz source, so 240 MHz); a gpio-hog `mipi_dphy_gpio` drives `ioport1` pin 8 low | RA8P1 | Z `ek_ra8p1.dtsi` | read from source |
| `ek_ra8p1` defines the same kind of MIPI connector (different pins) and `alias mipi-dsi`; no `zephyr,display`, no panel node, no enabled `&lcdif` | RA8P1 | Z `ek_ra8p1.dtsi`, `ek_ra8p1_cm85.dts` | read from source |
| The `ek_ra8p1` pinctrl file and defconfigs were not read | RA8P1 | n/a | open |
| One RA8 shield was found: `ek_ra8d1_rtk7eka6m3b00001bu` (overlay about 19 lines): a `renesas,ra-parallel-graphics-header` GPIO map (IIC_SCL, IIC_SDA, DISP_RST, DISP_INT, DISP_BLEN); no panel node, no GLCDC configuration | RA8D1 | Z `boards/shields/ek_ra8d1_rtk7eka6m3b00001bu/*.overlay` | read from source |
| The shield listing was cut off after `mikroe_a…`, so a MIPI panel shield for RA8 may exist in the unread part | RA8D1, RA8P1 | n/a | open |
| As the tree stands, a working display on either EK board needs an application overlay that enables `&lcdif` (and `&mipi_dsi` for DSI), names a panel, sets timings and the pixel format, and chooses `zephyr,display`; no in-tree sample config was found | both | rows above | inferred |
| Which Renesas display kits RA8P1 ships with (panel, resolution) is not in the files read | RA8P1 | n/a | open |

## 9. What this means for the layer model (inferred)

All rows below are `inferred` from sections 2-8.

- Planes: 2 graphics layers over a background colour; fixed order; ARGB per-pixel alpha, fade ramp, chroma key, 256-entry CLUT; no scaling, no YUV. So a UI layer plus a second UI or "priority" layer maps; a video plane with YUV or scaling does not.
- Latch: per-layer valid bit, applied at the next VSYNC, refused (not queued) while pending; atomicity across both layers is a software discipline.
- Zephyr uses one layer and one framebuffer; any two-layer backend would call FSP (`R_GLCDC_*`) directly, so it is a new C layer, not a Zephyr display driver change.
- The 2D engine is a copy/compose helper with no Zephyr display-driver use.
- RA8P1 adds a second core and a larger SoC, but FSP uses the same GLCDC code path as RA8D1; RA8P1 silicon limits are unconfirmed.

## 10. Summary of pinned sources

- Zephyr `zephyrproject-rtos/zephyr` @ `bc833f469ee653b7755881b8850792a4e14bacbb`.
- `zephyrproject-rtos/hal_renesas` @ `31e99f1c15d817d87448b981287e8ebbbf754f76` (HEAD read); Zephyr's own pin is `f3d847daccb3fb94980e1222f05049082592dc29`, not read.
- Renesas manuals: not reachable.

## Open questions

1. Re-check the rows still marked `~` (board files, binding YAML, Kconfig, DSI driver, part of `r_display_api.h`) against the checkout; the rest was checked.
2. Read the same `hal_renesas` files at Zephyr's pin `f3d847dac…` and diff against `31e99f1c…`.
3. Fetch the RA8P1 and RA8D1 user's manual GLCDC chapters: layer count and sizes per SoC, whether `PVEN` clears at VSYNC or at line detect, what "valid" means for each register block, maximum pixel clock and dot clock of RA8P1, bus bandwidth.
4. D-cache behaviour on the Cortex-M85: does FSP place framebuffers in a non-cacheable region, and where does the clean belong in a backend? (needs the manual and the FSP BSP linker files `bsp_linker.c`)
5. Is the FSP D/AVE 2D (`d2_*`) library shipped anywhere in `hal_renesas`, and what is the Zephyr consumer, if any?
6. The rest of `r_glcdc.c` (fade handling, the limit macros, the interrupt service routines) was not read line by line; rows on them are `inferred`.
7. The Zephyr framebuffer is `__aligned(8)` while FSP wants 64-byte alignment: how does the build satisfy it?
8. Which board would a farm need: an EK-RA8P1 (with MIPI display kit and debug probe) is the matching board; its display kit and pinctrl in Zephyr were not read.
9. `ek_ra8p1` pinctrl, defconfigs and any display shield for RA8 that the shield listing cut off.
10. Does either SoC run both GLCDC layers and DSI at once at the panel's pixel clock, and what is the DSI PHY clock budget? (manual)
