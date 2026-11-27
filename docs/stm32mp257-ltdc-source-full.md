# STM32MP257 LTDC: full source read (ltdc.h, drv.c, DT, binding, mainline)


Date of reading: 2026-11-28. **Nothing here is a board measurement.** Every row is tagged **source** (stated by the code or file read) or **inferred** (a conclusion drawn from it). Behaviour of the board is not claimed anywhere; the board runs ST's 6.6 kernel and has not been touched for this document.

This document completes [`docs/observed/stm32mp257-source.md`](observed/stm32mp257-source.md), which read only `ltdc.c` of ST v6.18-stm32mp and listed the files below as not read. Line numbers for `ltdc.c` in that document are still valid; they are re-used here.

## How it was read, and what failed

**Pinned ST tree (real checkout, 2026-11-28):** `STMicroelectronics/linux`, branch `v6.18-stm32mp`, `git rev-parse HEAD` = `3056405e0479330b0889c2a971452bfd10014b70`. Cloned with `git clone --depth 1 --branch v6.18-stm32mp --filter=blob:none --no-checkout` and read file by file with `git show HEAD:<path>`. Line numbers in the section "Checkout read" below were counted on that output and are exact. Rows above that section were written from the earlier fetch; their numbers were spot-checked against the checkout and held (`ltdc.c` 1004-1024, 1346-1370, 2144-2149, 2484-2608; `drv.c` 35-36, 269-280), except where the section says otherwise.

**Failure record (kept):** earlier on 2026-11-28 the clone was refused by the worker's permission layer ("This command requires approval", no network error) and the files were read through a web-fetch agent (HTTP 200, no line numbers; `raw.githubusercontent.com` answered HTTP 429 for mainline `drv.c`). A first retry the same day was refused twice as well; the ST clone ran only after that exact command was explicitly allowed.

**Mainline was not checked out.** `git clone --depth 1 --branch v6.18 --filter=blob:none --no-checkout https://github.com/torvalds/linux <deps>/linux-mainline` was refused ("This command requires approval", twice, 2026-11-28). The mainline column of the comparison below therefore still rests on the earlier web fetch of tag `v6.18` (commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449`, not verified by a local `rev-parse`); its line numbers are counted from fetched text.

**ST 6.6 (`v6.6-stm32mp`)** was not re-read; that section is unchanged from the fetch.

## Trees read

| Tree | Ref | Full commit | Date | Files |
| --- | --- | --- | --- | --- |
| `STMicroelectronics/linux` | branch `v6.18-stm32mp` | `3056405e0479330b0889c2a971452bfd10014b70` (`git rev-parse HEAD` of the local checkout, 2026-11-28) | committed 2026-10-23 | `drivers/gpu/drm/stm/{ltdc.c,ltdc.h,drv.c,Kconfig,Makefile}`, `arch/arm64/boot/dts/st/{stm32mp251.dtsi,stm32mp255.dtsi,stm32mp257f-dk.dts}`, `Documentation/devicetree/bindings/display/st,stm32-ltdc.yaml` |
| `STMicroelectronics/linux` | branch `v6.6-stm32mp` | `548f960c059bc4b9165cb69895fd67551f6061ca` | author 2026-04-30, committer 2026-06-01 | `drivers/gpu/drm/stm/{ltdc.c,ltdc.h,drv.c}` |
| `torvalds/linux` | tag `v6.18` (annotated, tag object `f7b88edb52c8dd01b7e576390d658ae6eef0e134`) | `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` | tagged 2025-11-30 (tagger date; the commit date was not fetched) | `drivers/gpu/drm/stm/{ltdc.c,ltdc.h,drv.c}` |

Read later from the checkout: `stm32mp257.dtsi`, `stm32mp25xf.dtsi` (no `ltdc` or `rotation` text) and `stm32mp257f-dk-resmem.dtsi`. Not read: `stm32mp253.dtsi` and any ST 6.6 device-tree or binding file. The board's kernel is 6.6.129 (OpenSTLinux scarthgap); the `v6.6-stm32mp` tip above is a later commit, so whether it matches 6.6.129 is **inferred**, not known.

## Settled items

| Item | Result | Basis | Where (ST v6.18, commit `3056405e`) |
| --- | --- | --- | --- |
| `LTDC_MAX_LAYER` | **4** (and `LTDC_MAX_FIREWALL` 4) | source | `ltdc.h:14`, `ltdc.h:15`; used to clamp the layer count at `ltdc.c:2423` |
| Layer count on MP25 | Read from `LTDC_LCR` at run time, clamped to 1..4. Not a constant in any file read | source | `ltdc.c:2423` |
| MP25 layer count, expected | 3: the DT has firewall children `l1l2` (RIFSC 119, "l1 & l2 layers") and `l3` (RIFSC 120, "l3 layer") plus `rot` (RIFSC 121); no `l4`. If the `l3` firewall is denied the count is decremented and the layer left out | inferred (firewall structure and `nb_layers--` are source: `ltdc.c:2425-2470`; `stm32mp251.dtsi` ltdc node, no line) | |
| MP25 `ltdc_plat_data` | `stm_drm_plat_data_mp25 = { .pad_max_freq_hz = 150000000 }`, used for both `st,stm32mp251-ltdc` and `st,stm32mp255-ltdc`. Non-MP25 `stm_drm_plat_data` is 90000000 | source | `drv.c:273-275` (struct), `drv.c:269-271`, `drv.c:279-280` (table); `ltdc.h:49-51` (struct) |
| Pad max frequency applied | `caps.pad_max_freq_hz = pdata->pad_max_freq_hz` (150 MHz); the 4.x hardware-version case does not override it (the overrides 65 MHz and 150 MHz belong to the 1.2.0 and 2.1.1 cases). Pixel clocks above it give `MODE_CLOCK_HIGH`, in `ltdc_encoder_mode_valid` (`ltdc.c:2351`) | source (assignment `ltdc.c:2485`; overrides `ltdc.c:2504`, `2550`) | |
| Layers with a scaler | Per layer at run time: `plane_scaling[i]` is true when `LTDC_L1C1R` of layer `i` has `LXCR_C1R_SCA` (bit 31, "SCaling Ability"). **Which layers have it is a hardware register fact, not in any source file read.** The mask array has `LTDC_MAX_LAYER` entries | source (mechanism: `ltdc.c:238`, `ltdc.c:2589-2592`; array `ltdc.h:41`); set of layers open | |
| Scaler limits | Upscale only: with the scaler a source larger than the destination is rejected ("Downscaling is not supported"); a YUV format with any size change is rejected; without the scaler any mismatch is rejected ("Scaling is not supported on layer %d"). **No numeric maximum upscale factor** is in the code: the factor is `((in-1)<<12)/(out-1)` masked to 16 bits, so the code itself does not cap it. Any hardware cap lies in the reference manual (not read) | source (checks `ltdc.c:1585-1600`, arithmetic `ltdc.c:1611-1618` (corrected from 1609 after the checkout read)); numeric bound open, to be measured | |
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
- (Closed: `rotation-memory` is not set in any DT file read, see "Checkout read".)
- Which output path (DSI/ADV7535 HDMI or LVDS panel) is live at a given boot.
- Exact line numbers for mainline and 6.6 `ltdc.c`/`drv.c` (needs a mainline and a 6.6 checkout).

## Checkout read, 2026-11-28 (ST `3056405e`): additions and corrections

All rows are **source** unless marked inferred. Line numbers are counted on the checkout.

| Item | Result | Where |
| --- | --- | --- |
| `LTDC_MAX_LAYER` | 4; `LTDC_MAX_FIREWALL` 4 (confirmed) | `ltdc.h:14-15`; clamp `ltdc.c:2423` |
| `ltdc_plat_data` | One field, `pad_max_freq_hz`. MP25 instance is 150000000 for both `st,stm32mp251-ltdc` and `st,stm32mp255-ltdc`; `st,stm32-ltdc` gets the generic one. **It carries no hardware version and no capability flag.** | `ltdc.h:49-51`; `drv.c:269-280` |
| Framebuffer size cap | `STM_MAX_FB_WIDTH` and `STM_MAX_FB_HEIGHT` are 2048, used as `mode_config.max_width/height`; `normalize_zpos = true` | `drv.c:35-36`, `94-98` |
| `mode_config` funcs | `.fb_create = drm_gem_fb_create`, `.atomic_check = drm_atomic_helper_check`, `.atomic_commit = drm_atomic_helper_commit`; commit tail `drm_atomic_helper_commit_tail_rpm`. No `async_page_flip` | `drv.c:38-46`, `94-98` |
| MP25 hardware version | Not in the DT or the plat data. Read from the `LTDC_IDR` register at probe, then switched on 0x010200, 0x010300, 0x020101, 0x040100, 0x040101; any other value gives `-ENODEV`. See "MP25 hardware version" in `docs/stm32mp257-ltdc-sync-source.md` | `ltdc.c:52-56`, `2484`, `2487-2608`; debug print `2743` |
| Scaler and rotator register access | Scaler registers (`LTDC_L1SISR`, `LTDC_L1SOSR`) are accessible for `st,stm32mp251-ltdc` and `st,stm32mp255-ltdc` inside `case HWVER_40100/40101` of the regmap access tables; rotator registers only for `st,stm32mp255-ltdc` | `ltdc.c:588-632`, `709-752` |
| `plane_scaling[]` | Filled per layer in the 4.x case from bit 31 of `LTDC_L1C1R + i*LAY_OFS`. Which layers have it is a register value, not in any file | `ltdc.c:2585-2592` |
| Scaling checks | On a layer with the ability: YUV with any size change gives `-EINVAL`; source larger than destination gives `-EINVAL` ("Downscaling is not supported"). Without it, any mismatch gives `-EINVAL`. No upper factor check | `ltdc.c:1585-1602` |
| Scaling factor | `((in-1) << 12) / (out-1)`, masked with `0xFFFF`; `0x1000` if `out == 1`. The earlier text cited `ltdc.c:1609`; the function is at `1611-1618`, used at `1999` and `2006`. Inferred: with a 12-bit fraction the factor reaches 0 once `out-1` exceeds `(in-1)*4096`, so the code is meaningless beyond about 4096x; nothing lower is in the code | `ltdc.c:1611-1618`, `1999`, `2006` |
| `LXCR_SCEN` | Set only when the layer can scale and the sizes differ | `ltdc.c:1985-1988` |
| Burst length | `st,burstlen` read for MP251/MP255 only (`mbl/8`, 0 if absent), written to `LTDC_L1BLCR` per layer | `ltdc.c:2656-2664`, `1970-1972` |
| YUV semiplanar/planar | On MP251/MP255 offered on layers with a nonzero index only, when the layer's `YSPA` bit is set | `ltdc.c:2196-2206` |
| `rotation-memory` | **Not set anywhere**: no match in `stm32mp251.dtsi`, `stm32mp255.dtsi`, `stm32mp257.dtsi`, `stm32mp25xf.dtsi`, `stm32mp257f-dk.dts` or `stm32mp257f-dk-resmem.dtsi`. The resmem file reserves `ltdc-sec-layer@fe800000` (8 MiB) and `ltdc-sec-rotation@ff000000` (16 MiB), both `no-map`, but nothing in those files references them. So `crtc_rotation` is cleared on this tree. (Closes the open item below.) | `ltdc.c:2825-2831`; `stm32mp257f-dk-resmem.dtsi:174-182` |
| LTDC node | `ltdc: display-controller@48010000`, `compatible = "st,stm32mp251-ltdc"`; children `l1l2` (`&rifsc 119`), `l3` (`&rifsc 120`), `rot` (`&rifsc 121`); common firewall `&rifsc 80` | `stm32mp251.dtsi:2148-2170` |
| MP255 override | `&ltdc` gets `compatible = "st,stm32mp255-ltdc"` and clocks lcd, bus, syscfg (`ref`), lvds; `dsi@48000000`, `lvds@48060000` | `stm32mp255.dtsi:30-33`, `124`, `138` |
| Board output paths | `&ltdc` is `okay` with `ltdc_ep0_out` to `dsi_in` and `ltdc_ep1_out` to `lvds_in`; DSI goes to the ADV7535 (`hdmi@3d`), LVDS to `panel_lvds`. Which one is live is not shown by the dts | `stm32mp257f-dk.dts:645-663`, `364-383`, `537-575`, `665-684`, `172-200` |
| Binding | `compatible` enum `st,stm32-ltdc`, `st,stm32mp251-ltdc`, `st,stm32mp255-ltdc`; `rotation-memory` (at least two frames); one port with up to 2 endpoints that cannot be active together; `additionalProperties: false`; no `st,burstlen`, no bandwidth property | `st,stm32-ltdc.yaml:14-18`, `95-99`, `106-109`, `163` |

The earlier caveat that the tail of `stm32mp251.dtsi` was unseen is closed: the whole file was searched and no `burstlen`, `rotation` or second `ltdc` text appears.

### Does `Stm32Ltdc` in `profile.rs` agree with the source? (compared 2026-11-28, no edit made)

| `profile.rs` claim | Source | Result |
| --- | --- | --- |
| `max_downscale` 1.0, upscale only | `ltdc.c:1585-1602` rejects a source larger than the destination | matches |
| Primary and first overlay scale, later overlays do not | `LXCR_C1R_SCA` is a register value (`ltdc.c:2585-2592`) | not confirmable from source; stays observed-only |
| Upscale bound 32x; "no numeric cap in the source" | No factor cap in the code; arithmetic at `ltdc.c:1611-1618` | matches in substance |
| Cites `ltdc.c:1609` for the 16-bit mask | The mask is at `ltdc.c:1618` (function `1611-1618`) | **mismatch: citation 9 lines off** |
| Cites `ltdc.c:1585-1600` for "YUV cannot be scaled" | YUV check `ltdc.c:1586-1592`, block ends `1602` | fine |
| Cites `ltdc.c:238` for `LXCR_C1R_SCA` | `ltdc.c:238` | matches |
| 2048x2048 from `STM_MAX_FB_WIDTH`/`HEIGHT` in `drv.c` | `drv.c:35-36` | matches; consistent with leaving `max_source` unset |

No value mismatch. The one mismatch is the `ltdc.c:1609` citation, which should read `1618`.
