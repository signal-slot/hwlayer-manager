# STM32MP257 LTDC: full source read (ltdc.h, drv.c, DT, binding, mainline)

> **Checkout retry, 2026-11-28 (item source-checkout-retry): failed, nothing re-read.** `git clone --depth 1 --branch v6.18-stm32mp --filter=blob:none --no-checkout https://github.com/STMicroelectronics/linux <deps dir>/stm32-linux` was refused twice by the worker's permission layer, the second time as a single simple command. Exact message: `This command requires approval`. No git or HTTP error was produced (the clone never ran), so no commit was pinned and no row above was changed. The earlier fetch failures stay as recorded. The mainline clone was not tried (same policy). The `Stm32Ltdc` source-read values were not re-checked.

Date of reading: 2026-11-28. **Nothing here is a board measurement.** Every row is tagged **source** (stated by the code or file read) or **inferred** (a conclusion drawn from it). Behaviour of the board is not claimed anywhere; the board runs ST's 6.6 kernel and has not been touched for this document.

This document completes [`docs/observed/stm32mp257-source.md`](observed/stm32mp257-source.md), which read only `ltdc.c` of ST v6.18-stm32mp and listed the files below as not read. Line numbers for `ltdc.c` in that document are still valid; they are re-used here.

## How it was read, and what failed

**A real git checkout was not possible on this worker.** On 2026-11-28 the following were refused by the worker's permission layer ("This command requires approval", no network error): `git clone --depth 1 --branch v6.18-stm32mp --filter=blob:none --no-checkout https://github.com/STMicroelectronics/linux <deps>/stlinux-v6.18`, then `git fetch --depth 1 --filter=blob:none https://github.com/STMicroelectronics/linux v6.18-stm32mp` into an empty `git init` directory (`$CONNECTED_DEPS_DIR/stlinux-v6.18`, now an empty repository with no objects), and `gh api repos/STMicroelectronics/linux/branches/v6.18-stm32mp`. `git ls-remote` worked and confirmed the branch tip below. No HTTP 429/504 was seen on the clone path because it never ran.

**Substitute, used because of that, and not equivalent to a checkout:** the files were read through a web-fetch agent (HTTP GET of `raw.githubusercontent.com/<repo>/<full commit sha>/<path>`; HTTP 200 for every file except one). Consequences, all of which weaken the "line numbers" requirement:

- The fetch tool returns text without line numbers. Where a number is given below, it was either (a) checked by `grep -n` against the earlier single-file copy of the same commit (`ltdc.c`), or (b) counted by the fetch agent from the text it received (`ltdc.h`, `drv.c`, `Kconfig`, `Makefile`; `drv.c` may be off by one after about line 48). Rows without a number say "no line" and give an identifier or node name instead. No number was guessed.
- Whitespace may have been normalised. `stm32mp251.dtsi` (92,783 characters) was seen verbatim only up to its last ~14,000 characters; the claim that nothing display-related is in that tail rests on a model summary and is tagged inferred.
- Mainline `drv.c` at v6.18: `raw.githubusercontent.com` answered **HTTP 429 Too Many Requests**; the same file was then read from `github.com/torvalds/linux/blob/v6.18/drivers/gpu/drm/stm/drv.c?plain=1` (HTTP 200, 273 lines; numbers derived by counting back from 273).
- Whoever wants exact numbers for the DT and binding rows should repeat the read from a checkout; the pinned hashes below make that straightforward.

## Trees read

| Tree | Ref | Full commit | Date | Files |
| --- | --- | --- | --- | --- |
| `STMicroelectronics/linux` | branch `v6.18-stm32mp` | `3056405e0479330b0889c2a971452bfd10014b70` (confirmed by `git ls-remote`, 2026-11-28) | committed 2026-10-23 | `drivers/gpu/drm/stm/{ltdc.c,ltdc.h,drv.c,Kconfig,Makefile}`, `arch/arm64/boot/dts/st/{stm32mp251.dtsi,stm32mp255.dtsi,stm32mp257f-dk.dts}`, `Documentation/devicetree/bindings/display/st,stm32-ltdc.yaml` |
| `STMicroelectronics/linux` | branch `v6.6-stm32mp` | `548f960c059bc4b9165cb69895fd67551f6061ca` | author 2026-04-30, committer 2026-06-01 | `drivers/gpu/drm/stm/{ltdc.c,ltdc.h,drv.c}` |
| `torvalds/linux` | tag `v6.18` (annotated, tag object `f7b88edb52c8dd01b7e576390d658ae6eef0e134`) | `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` | tagged 2025-11-30 (tagger date; the commit date was not fetched) | `drivers/gpu/drm/stm/{ltdc.c,ltdc.h,drv.c}` |

Not read: `stm32mp253.dtsi`, `stm32mp257.dtsi`, `stm32mp25xf.dtsi` and `stm32mp257f-dk-resmem.dtsi` (included by the board file; the likeliest home of a `rotation-memory` reserved region), and any ST 6.6 device-tree or binding file. The board's kernel is 6.6.129 (OpenSTLinux scarthgap); the `v6.6-stm32mp` tip above is a later commit, so whether it matches 6.6.129 is **inferred**, not known.

## Settled items

| Item | Result | Basis | Where (ST v6.18, commit `3056405e`) |
| --- | --- | --- | --- |
| `LTDC_MAX_LAYER` | **4** (and `LTDC_MAX_FIREWALL` 4) | source | `ltdc.h:14`, `ltdc.h:15`; used to clamp the layer count at `ltdc.c:2423` |
| Layer count on MP25 | Read from `LTDC_LCR` at run time, clamped to 1..4. Not a constant in any file read | source | `ltdc.c:2423` |
| MP25 layer count, expected | 3: the DT has firewall children `l1l2` (RIFSC 119, "l1 & l2 layers") and `l3` (RIFSC 120, "l3 layer") plus `rot` (RIFSC 121); no `l4`. If the `l3` firewall is denied the count is decremented and the layer left out | inferred (firewall structure and `nb_layers--` are source: `ltdc.c:2425-2470`; `stm32mp251.dtsi` ltdc node, no line) | |
| MP25 `ltdc_plat_data` | `stm_drm_plat_data_mp25 = { .pad_max_freq_hz = 150000000 }`, used for both `st,stm32mp251-ltdc` and `st,stm32mp255-ltdc`. Non-MP25 `stm_drm_plat_data` is 90000000 | source | `drv.c:273-275` (struct), `drv.c:269-271`, `drv.c:279-280` (table); `ltdc.h:49-51` (struct) |
| Pad max frequency applied | `caps.pad_max_freq_hz = pdata->pad_max_freq_hz` (150 MHz); the 4.x hardware-version case does not override it (the overrides 65 MHz and 150 MHz belong to the 1.2.0 and 2.1.1 cases). Pixel clocks above it give `MODE_CLOCK_HIGH`, in `ltdc_encoder_mode_valid` (`ltdc.c:2351`) | source (assignment `ltdc.c:2485`; overrides `ltdc.c:2504`, `2550`) | |
| Layers with a scaler | Per layer at run time: `plane_scaling[i]` is true when `LTDC_L1C1R` of layer `i` has `LXCR_C1R_SCA` (bit 31, "SCaling Ability"). **Which layers have it is a hardware register fact, not in any source file read.** The mask array has `LTDC_MAX_LAYER` entries | source (mechanism: `ltdc.c:238`, `ltdc.c:2589-2592`; array `ltdc.h:41`); set of layers open | |
| Scaler limits | Upscale only: with the scaler a source larger than the destination is rejected ("Downscaling is not supported"); a YUV format with any size change is rejected; without the scaler any mismatch is rejected ("Scaling is not supported on layer %d"). **No numeric maximum upscale factor** is in the code: the factor is `((in-1)<<12)/(out-1)` masked to 16 bits, so the code itself does not cap it. Any hardware cap lies in the reference manual (not read) | source (checks `ltdc.c:1585-1600`, arithmetic `ltdc.c:1609`); numeric bound open, to be measured | |
| zpos | MP25 (4.x case): mutable `zpos`, range `0..nb_layers-1`, initial value = layer index; `normalized_zpos` is written into `LTDC_L1BFCR` (bit field at shift 16). DRM core `normalize_zpos = true` (drv.c) | source | `ltdc.c:2267` (primary), `ltdc.c:2314` (overlays), `ltdc.c:1842`; `drv.c` (`mode_config.normalize_zpos`), no line |
| Alpha | `drm_plane_create_alpha_property` on every plane. No `blend_mode` property anywhere in `ltdc.c` | source | `ltdc.c:2236` |
| Memory-bandwidth property | None in the binding (`additionalProperties: false`, no bandwidth or "max-bandwidth" property) and none in the two dtsi and the board dts read. The nearest knob is `st,burstlen`, read by the driver for MP251/MP255 (`max_burst_length = mbl/8`, 0 if absent) and written to `LTDC_L1BLCR` per layer. It is **not in the binding** and **not set** in `stm32mp251.dtsi`, `stm32mp255.dtsi` or `stm32mp257f-dk.dts`, so on this tree it is 0 | source (driver `ltdc.c:2656-2664`, `1970-1972`; binding and dts: absence in files read); tail of `stm32mp251.dtsi` unseen verbatim: inferred | |
| Max framebuffer size | `STM_MAX_FB_WIDTH 2048`, `STM_MAX_FB_HEIGHT 2048` ("same as width to handle orientation") in `drv.c`. This is the one place a size limit exists; it limits the DRM framebuffer, so it bounds source size, not the scaled destination | source (no line; the same definitions are in mainline v6.18 and in ST 6.6) | |

## Other facts from `ltdc.h`, `drv.c`, Kconfig, Makefile

| Item | Finding | Basis | Where |
| --- | --- | --- | --- |
| `struct ltdc_caps` | Fields: `hw_version`, `nb_layers`, `layer_ofs`, `layer_regs`, `conf_regs`, `bus_width`, `pix_fmt_hw`, `pix_fmt_drm`, `pix_fmt_nb`, `pix_fmt_flex`, `non_alpha_only_l1`, `pad_max_freq_hz`, `nb_irq`, `ycbcr_input`, `ycbcr_output`, `plane_reg_shadow`, `crc`, `dynamic_zorder`, `plane_rotation`, `crtc_rotation`, `fifo_threshold`, `plane_scaling[LTDC_MAX_LAYER]` | source | `ltdc.h:19-42` |
| `struct ltdc_device` | Holds `max_burst_length`, `rot_mem` (reserved memory for the rotator), `firewall[LTDC_MAX_FIREWALL]`, `plane_enabled[LTDC_MAX_LAYER]`, clocks `pixel_clk`, `ltdc_clk`, `lvds_clk`, `bus_clk` | source | `ltdc.h:53-77` |
| MP25 compatibles | `st,stm32mp251-ltdc`, `st,stm32mp255-ltdc` (both map to the MP25 plat data), plus `st,stm32-ltdc` | source | `drv.c:278-280` |
| Display variants | MP251: LTDC (the DT base compatible). MP255: adds the `lvds` clock and `ref` (syscfg) clock and the DSI/LVDS nodes. The MP257F-DK DTS enables the DSI path and the LVDS path | source | `stm32mp255.dtsi` `&ltdc`, board `&ltdc`, no lines |
| Kconfig | `DRM_STM` (depends on `DRM && (ARCH_STM32 || COMPILE_TEST)`, `COMMON_CLK`; selects `DRM_GEM_DMA_HELPER`, `DRM_PANEL_BRIDGE`), `DRM_STM_DSI` (selects `DRM_DW_MIPI_DSI`), `DRM_STM_LVDS` | source | `Kconfig:2-35` |
| Makefile | `stm-drm-y := drv.o ltdc.o`; `dw_mipi_dsi-stm.o` for DSI; `lvds.o` for LVDS | source | `Makefile:2-10` |

## Device tree and binding (ST v6.18, commit `3056405e`; no line numbers, see above)

| Item | Finding | Basis | Where |
| --- | --- | --- | --- |
| LTDC node | `ltdc: display-controller@48010000`, `compatible = "st,stm32mp251-ltdc"`, `reg = <0x48010000 0x400>`, two GIC SPI interrupts (158, 159), clocks `lcd` = `CK_KER_LTDC` and `bus` = `CK_BUS_LTDC`, reset `LTDC_R`, power domain `d1_pd`, `status = "disabled"`, common firewall `<&rifsc 80>` ("cmn") | source | `stm32mp251.dtsi`, inside `rifsc: bus@42080000` |
| Child nodes | `l1l2` (`<&rifsc 119>`), `l3` (`<&rifsc 120>`), `rot` (`<&rifsc 121>`): each carries only `access-controllers` and `access-controller-names` | source | `stm32mp251.dtsi`; binding |
| MP255 override | `&ltdc { compatible = "st,stm32mp255-ltdc"; clocks = lcd, bus, <&syscfg> ("ref"), <&lvds> ("lvds") }`. `dsi: dsi@48000000` (`st,stm32mp255-dsi`) and `lvds: lvds@48060000` (`st,stm32mp255-lvds`, `st,stm32mp25-lvds`, RIFSC 84), both disabled by default and neither with an `interrupts` property | source | `stm32mp255.dtsi` |
| Board dts: output paths | `&ltdc` `status = "okay"` with **two** endpoints: `ltdc_ep0_out` to `&dsi_in` (DSI, to ADV7535 on I2C2 at 0x3d, to the HDMI-A connector) and `ltdc_ep1_out` to `&lvds_in` (LVDS to `panel_lvds`, EDT ETML0700Z9NDHA, 1024x600, 54 MHz, vesa-24). The binding says the two endpoints "cannot be activated simultaneously" | source (nodes); which path is live on the board is not shown by the dts alone: open | `stm32mp257f-dk.dts`, `&ltdc`, `&dsi`, `&lvds`, `&i2c2` |
| `rotation-memory` | Binding property (phandle array, "at least two frames"). **Not set** in `stm32mp251.dtsi`, `stm32mp255.dtsi` or `stm32mp257f-dk.dts`; it may be in `stm32mp257f-dk-resmem.dtsi`, not read. Without it the driver clears `crtc_rotation` (`ltdc.c:2825`), so 90/270 rotation is not available on a tree that does not set it | source (driver, binding); presence in resmem file open | `ltdc.c:2825`; binding |
| `st,burstlen` | See the memory-bandwidth row: read by the driver (`ltdc.c:2659`), absent from binding and from the three DT files | source | |
| Binding compatibles and clocks | `enum: st,stm32-ltdc, st,stm32mp251-ltdc, st,stm32mp255-ltdc`; `clock-names` `lcd, bus, ref, lvds`; MP251 exactly 2 clocks, MP255 at least 4; required: `compatible reg interrupts clocks clock-names resets port` | source | `Documentation/devicetree/bindings/display/st,stm32-ltdc.yaml` |
| Output port | One `port`, "up to 2 endpoints": external DPI RGB (gpios) or the internal DPI input of the MIPI DSI host | source | binding |
| HDMI and LVDS path | HDMI is reached only via LTDC -> DSI -> ADV7535 -> `hdmi-connector`; the panel via LTDC -> LVDS. `ltdc_encoder_init` always creates a `DRM_MODE_ENCODER_DPI` encoder; the DSI/LVDS bridges are attached behind it | source (DT graph); the `DRM_MODE_ENCODER_DPI` fact is from `ltdc.c:2388` | board dts |

## ST v6.18 vs mainline (v6.18)

Mainline `drivers/gpu/drm/stm/ltdc.c` at tag `v6.18` has no MP25-specific plane code: its `ltdc_get_caps` handles only the hardware versions 1.2.0, 1.3.0, 2.1.1 and 4.0.0/4.0.1 (4.x sets `dynamic_zorder`, `plane_rotation`, `fifo_threshold`, flexible pixel format), and `ltdc.h` has no scaling, firewall or rotator fields.

| Topic | ST v6.18-stm32mp (`3056405e`) | Mainline v6.18 (`7d0a66e4`) | Basis |
| --- | --- | --- | --- |
| `LTDC_MAX_LAYER` | 4 (`ltdc.h:14`) | 4 (`#define` at `ltdc.h:36`, counted) | source |
| MP25 compatibles in `drv.c` | `st,stm32mp251-ltdc`, `st,stm32mp255-ltdc`, plat data 150 MHz | Both compatibles present in `drv.c`, same 150 MHz plat data (`drv.c:242-249`, counted from the end) | source |
| MP25 code in `ltdc.c` | Firewalls (`l1l2`, `l3`, `rot`), rotator, scaler, LVDS clock, `st,burstlen`, `rotation-memory` | Only the `bus` clock for MP251/MP255 in `ltdc_load` and an optional `lvds` clock; no firewall, rotator, scaler, burst length | source |
| Scaling | `plane_scaling[]` per layer; upscale allowed on layers with `LXCR_C1R_SCA`; scaler registers programmed (`ltdc.c:1985`) | Unconditional rejection: "Scaling is not supported" whenever `src` and `dst` sizes differ; `LXCR_C1R_SCA` defined but never used | source |
| zpos | Mutable on 4.x: `drm_plane_create_zpos_property(.., 0, nb_layers-1)` | Same code | source |
| Alpha | `drm_plane_create_alpha_property`, no blend-mode property | Same | source |
| Rotation | Plane: `ROTATE_0 | REFLECT_X | REFLECT_Y`; primary also 90/180/270 when `crtc_rotation` (needs `rotation-memory`) | Plane rotation only (`ROTATE_0 | REFLECT_X | REFLECT_Y`); no `crtc_rotation` | source |
| `ltdc_encoder_mode_valid` | Present; checks `pad_max_freq_hz` for the encoder | Not present; the pad frequency check is in `ltdc_crtc_mode_valid` (`result > pad_max_freq_hz` gives `MODE_CLOCK_HIGH`) | source |
| YUV semi-planar / planar | Layers with nonzero index on MP251/MP255 only, per layer capability bits | Gated by `ycbcr_input` and the layer's `YSPA`/`YFPA` bits, no MP25 special case | source |
| Consequence | A mainline 6.18 kernel on the MP257F-DK would at best expose the LTDC planes without scaling, rotation memory, firewall handling or the RIFSC firewall accesses ST's tree requests; this is inferred from the absence of the code | | inferred |

## ST 6.6 note (`v6.6-stm32mp`, commit `548f960c`)

The board's kernel is 6.6.129, so this branch is the closest available reading. Differences that matter (all from the fetched `ltdc.h`, `drv.c`, `ltdc.c`; no line numbers):

| Topic | ST v6.6-stm32mp | vs ST v6.18 | Basis |
| --- | --- | --- | --- |
| Compatibles | `st,stm32mp21-ltdc` and `st,stm32mp25-ltdc` (single MP25 compatible), plus `st,stm32-ltdc`. **`st,stm32mp251-ltdc` and `st,stm32mp255-ltdc` do not appear** | v6.18 split MP25 into MP251/MP255 and dropped MP21 from the table | source |
| Plat data | `stm_drm_plat_data` 90 MHz; `stm_drm_plat_data_mp21` 150 MHz; `stm_drm_plat_data_mp25` 150 MHz | MP25 value unchanged at 150 MHz | source |
| `LTDC_MAX_LAYER`, caps, firewalls | 4; `plane_scaling[LTDC_MAX_LAYER]`, `crtc_rotation`, `conf_regs`, `max_burst_length`, `rot_mem`, firewall `l1l2`/`l3`/`rot` all present | Same | source |
| Scaling check in `ltdc_plane_atomic_check` | Same logic: with `plane_scaling[plane->index]` upscale only and no scaling on YUV; otherwise any mismatch rejected | Same | source |
| zpos / alpha | Same code: mutable zpos `0..nb_layers-1` on 4.x, alpha property on every plane, no blend-mode | Same | source |
| 4.x caps | `crtc_rotation` follows `GC2R_ROTA`, cleared when `rotation-memory` is missing; `plane_scaling[i]` from `LXCR_C1R_SCA` per layer | Same | source |
| Burst length | `LTDC_L1BLCR` written with `max_burst_length` on mp21 and mp25 | Same behaviour, gated on MP251/MP255 | source |
| Rotator / scaler register access gating | Guarded by `st,stm32mp25-ltdc` (so, with one MP25 compatible, no MP251/MP255 split such as "only MP255 has the rotator") | v6.18 limits rotator registers to MP255 | source (per fetch summary); the exact lines were not seen |

Inferred: with a single `st,stm32mp25-ltdc` compatible on the 6.6 kernel the 6.18-only distinction between MP251 and MP255 does not exist there; scaling, zpos and alpha rules the board shows should then be those of the table above. No claim about the board follows from this.

## What remains open (needs a board or the reference manual)

- Which of the 3 (inferred) layers carry `LXCR_C1R_SCA`: a hardware register, to be read from the board probe (item scale-bounds) or the RM.
- The numeric upscale bound: not in code.
- Whether `rotation-memory` is defined in `stm32mp257f-dk-resmem.dtsi`.
- Which output path (DSI/ADV7535 HDMI or LVDS panel) is live at a given boot.
- Exact line numbers for the DT and binding rows and for mainline and 6.6 `ltdc.c`, once a real checkout is possible.
