# VC4 HVS5 / HVS6: scaling ratios, LBM/UPM sizing, modifier checks and dlist words, read from source

Dated 2026-11-28. Closes the open points of `docs/hvs-backend-design.md` §7 Q1 (scaling limit), §5 (IN_FORMATS blob against the source) and §3 (LBM/UPM capacity without per-layer sizing). Nothing here was run on a board.

## Provenance

- Tree: `raspberrypi/linux`, branch `rpi-6.18.y`, commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e`, directory `drivers/gpu/drm/vc4/`.
  Link: <https://github.com/raspberrypi/linux/tree/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4>
- Files read in full at that commit: `vc4_plane.c` (3004 lines), `vc4_hvs.c`, `vc4_regs.h`, `vc4_drv.h`. They were fetched as raw files at the SHA, so every line number below is exact for the pin. The DRM core (`drivers/gpu/drm/*.c`) is not part of the fetch; statements about it are tagged inferred.
- Names: gen 5 = `VC4_GEN_5` = BCM2711 = HVS5. Gen 6 = `VC4_GEN_6_C` / `VC4_GEN_6_D` = BCM2712 = HVS6. Gen 4 is shown only where a mask differs.
- Tags: **read from source** rows carry `file:line` at the pin. **inferred** rows (worked numbers, conclusions) are derived from those rows and say so.
- Which kernel a board runs is not known from this file. All statements are about the pin.
- Fixed-point note: `src_w`, `src_h` are 16.16 fixed point (`state->src` after `drm_atomic_helper_check_plane_state`); `crtc_w`, `crtc_h` are plain pixels. `src_w / crtc_w` is therefore a 16.16 ratio.

## 1. Scaling mode and ratio encoding

### 1.1 Mode choice

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| Per-axis mode | NONE if `dst << 16 == src`. TPZ if the source is ≤ 1 pixel. PPF if `3 * dst >= 2 * (src >> 16)` (downscale ≤ 1.5×, or any upscale). TPZ otherwise | read from source | `vc4_plane.c:329-342` |
| DRM helper limits | Called with `min_scale = 1`, `max_scale = INT_MAX`, so the helper adds no ratio limit of its own | read from source | `vc4_plane.c:593-594` |
| Fractional start | An axis that is NONE but has a fractional `src_x` / `src_y` is turned into PPF | read from source | `vc4_plane.c:618-621` |
| YUV forces | Any multi-plane format sets `is_yuv`; chroma `x_scaling[1]` and `y_scaling[1]` are forced to PPF when NONE. So NV12 always has a vertical scaler on chroma and therefore needs LBM even at 1:1 | read from source | `vc4_plane.c:635`, `vc4_plane.c:658-665` |
| TPZ upscale of YUV | A TPZ upscale on a YUV format or with a fractional start is switched to PPF (the 1-pixel TPZ upscale is wrong for those) | read from source | `vc4_plane.c:677-685` |
| YUV444 | Gen 5: luma x forced PPF when NONE (`is_yuv444_unity`). Gen 6: luma x and y both forced PPF when NONE | read from source | `vc4_plane.c:1489-1495` (gen 5), `vc4_plane.c:2049-2068` (gen 6) |

### 1.2 TPZ (trapezoid) encoding, gen 5 and gen 6 identical

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| Fields | Word 0: `SCALE` bits 28:8 (21 bits), `IPHASE` 7:0 (the driver writes 0). Word 1: `RECIP` bits 15:0. Bit 31 (`VERT_RECALC`) exists on gen 5 only and is not used | read from source | `vc4_regs.h:1332-1338`, `vc4_plane.c:721-724` |
| Downscale | `scale = src / dst` (16.16 ratio), `recip = ~0 / scale` (the code writes `~0`, an `int`, so this is 0xFFFFFFFF / scale, truncated into 16 bits by the field) | read from source | `vc4_plane.c:707-709` |
| Upscale | `scale = (1 << 16) + 1`, `recip = (1 << 16) - 1`, independent of the ratio. So a TPZ upscale has no ratio field to saturate | read from source | `vc4_plane.c:710-713` |
| Field maximum | `FIELD_MAX(SCALER_TPZ0_SCALE_MASK)` = 2^21 − 1 = 0x1FFFFF = 2097151. As a ratio: 2097151 / 65536 = **31.99998×** | read from source (mask), inferred (ratio) | `vc4_regs.h:1333` |
| Horizontal refusal | `-EINVAL` when an axis is TPZ and `src_w[i] / crtc_w > 0x1FFFFF`, for luma (`i = 0`) and chroma (`i = 1`). Formula: refused when `src_w_pixels / crtc_w > 31.99998` | read from source | `vc4_plane.c:686-690` |
| Vertical refusal | In `vc4_plane_set_vdownsample`: `-EINVAL` when a TPZ axis has `src_h[i] / vds / crtc_h > 0x1FFFFF` for `i` in 0..1. Refused when `src_h_pixels / (vds * crtc_h) > 31.99998` | read from source | `vc4_plane.c:1446-1452` |
| Applies to both gens | The same `SCALER_TPZ0_SCALE_MASK` is used by `vc4_write_tpz` for gen 5 and gen 6 (the comment says the BCM2712 only lacks bit 31) | read from source | `vc4_plane.c:696-722` |
| Field overflow | `VC4_SET_FIELD` only `WARN_ON`s when the value does not fit, then `FIELD_PREP` truncates. The checks above make that unreachable for the TPZ scale | read from source | `vc4_regs.h:14-18` |

### 1.3 PPF (polyphase filter) encoding, gen 5 and gen 6 identical

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| Fields | `SCALE` bits 24:8 (17 bits), `IPHASE` 6:0, `NOINTERP` bit 31 (nearest neighbour), `AGC` bit 30 (always set). Kernel word: offset bits 13:0 | read from source | `vc4_regs.h:1343-1355` |
| Value | `scale = src / dst` as 16.16, luma channel `scale &= ~1` (loses 1 bit). Phase starts at half a pixel less chroma siting, clamped to 63 | read from source | `vc4_plane.c:730-790` |
| Field maximum | `FIELD_MAX(SCALER_PPF_SCALE_MASK)` = 2^17 − 1 = 0x1FFFF → ratio 1.99998× | read from source (mask), inferred (ratio) | `vc4_regs.h:1348` |
| Driver check | **None.** There is no refusal on the PPF scale. It does not need one: mode choice picks PPF only when `3 * dst >= 2 * (src >> 16)`, i.e. a downscale ≤ 1.5, so `scale ≤ 0x18000` (1.5 × 65536 = 98304) < 0x1FFFF. Upscale gives `scale < 0x10000` | read from source (no check, `vc4_write_ppf`); the bound is inferred | `vc4_plane.c:337-341`, `vc4_plane.c:730-790` |
| Chroma exception | A YUV chroma axis with `src[1]` half the luma source can be PPF at a luma ratio up to 3.0 (chroma ratio ≤ 1.5). Chroma `scale` is then ≤ 0x18000 as well | inferred from `vc4_plane.c:640-650` (chroma `src_w[1] = src_w[0] / hsub`) | `vc4_plane.c:640-651` |
| Upscale precision | For luma `scale &= ~1`, so `scale` reaches 0 when `src/dst` in 16.16 is below 2, i.e. an upscale beyond 32768×. That is beyond the dst field (§1.5), so unreachable | inferred | `vc4_plane.c:748` |

### 1.4 Vertical decimation (line skipping), a third factor on vertical TPZ downscale

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| Trigger | Only vertical TPZ with `src_h / crtc_h > 3.0` (the test is `> (3 << 16)` in 16.16) | read from source | `vc4_plane.c:1429-1431` |
| Wanted skip | `vds = ((src_h[0] / (crtc_h * 3)) >> 16) + 1`, so about ratio / 3, rounded up | read from source | `vc4_plane.c:1431` |
| Cap by layout | SAND (64, 128, 256): `max_tile_skip`. Linear: `max_pitch / pitches[0]`. Anything else (T-tiled): 1. Multi-plane formats also `min(max, (src_h[1] / crtc_h) >> 16)`. Then `vds = clamp(max, 1, vds)` | read from source | `vc4_plane.c:1433-1443` |
| Gen 5 inputs | `max_pitch` = `FIELD_MAX(SCALER_SRC_PITCH_MASK)` = 0xFFFF = 65535; `max_tile_skip` = `FIELD_MAX(SCALER_TILE_SKIP_0_MASK) + 1` = 7 + 1 = 8 | read from source | `vc4_plane.c:1504-1505`, `vc4_regs.h:1358`, `vc4_regs.h:1362` |
| Gen 6 inputs | `max_pitch` = `FIELD_MAX(SCALER6_PTR2_PITCH_MASK)` = 0x1FFFF = 131071 (bits 16:0); `max_tile_skip` = `FIELD_MAX(SCALER6_PTR2_TILE_LSKIP_MASK) + 1` = 7 + 1 = 8 (bits 31:29) | read from source | `vc4_plane.c:2087-2088`, `vc4_regs.h:1438`, `vc4_regs.h:1441` |
| Hardware-side use | Source height and pitch are divided / multiplied by `vds` in the dlist words (`height / vdownsample`, `pitch * vdownsample`, tile line skip `vdownsample − 1`). T-tiled is forced to `vdownsample = 1` | read from source | `vc4_plane.c:1556`, `vc4_plane.c:1776`, `vc4_plane.c:1859`, `vc4_plane.c:2292`, `vc4_plane.c:2329` |

### 1.5 Resulting maxima (inferred from 1.2 to 1.4)

The vertical refusal is `ratio_v / vds > 31.99998`, where `vds` is the clamped skip. When the cap `M` is larger than the wanted skip, `ratio_v / vds` is about 3, so no refusal; the limit is reached when the cap binds: **`ratio_v ≤ 31.99998 × M`**.

| Case | M | Largest accepted vertical TPZ downscale | Basis |
| --- | --- | --- | --- |
| Gen 5, T-tiled RGB | 1 | **31.99998×** | inferred |
| Gen 5, SAND (any) | 8 | **255.99984×** (8 × 31.99998) | inferred |
| Gen 5, linear, 1920 × 4-byte pitch 7680 | 65535 / 7680 = 8 | **255.99984×** | inferred |
| Gen 5, linear, pitch 3840 (e.g. RGB565 1920) | 17 | **543.99966×** | inferred |
| Gen 6, SAND128 / SAND256 | 8 | **255.99984×** | inferred |
| Gen 6, linear, pitch 7680 | 131071 / 7680 = 17 | **543.99966×** | inferred |
| Gen 6, linear, pitch 3840 | 34 | **1087.9993×** | inferred |
| Horizontal TPZ downscale, both gens, any layout | n/a | **31.99998×** | inferred (`vc4_plane.c:686-690`) |
| Downscale by PPF, both gens | n/a | 1.5× by mode choice (field could hold 1.99998×) | inferred |
| Upscale, both gens | n/a | no ratio field limit. Limited by the destination rectangle: dst width ≤ 8192 on gen 6 (`SCALER6_POS1_SCL_WIDTH` bits 12:0 holds `crtc_w − 1`) and `crtc_w` ≤ 8191 on gen 5 (`SCALER5_POS1_SCL_WIDTH` bits 12:0); heights have 13 bits on both | inferred from `vc4_regs.h:1247-1251`, `vc4_regs.h:1414-1415` |

Cross-checks: the vertical ratio is also bounded by the source height field, 13 bits (`SCALER5_POS2_HEIGHT_MASK` 28:16, `SCALER6_POS2_SRC_LINES_MASK` 28:16, `vc4_regs.h:1268`, `vc4_regs.h:1417`), so a source is at most 8191 / 8192 lines. With `crtc_h` ≥ 1, ratios above the table are not reachable on real sources except for the T-tiled case.

Conclusion for the planner (inferred): a conservative **per-axis downscale limit of 31.99×** is exact for horizontal and for vertical T-tiled; vertical linear and SAND allow far more, but only through the line-skip path, which is lossy by design. Upscale has no ratio limit in the driver. TEST_ONLY on a board can only tighten these numbers (for example the hardware may fail underrun-wise earlier).

### 1.6 Width and rectangle limits that gate scaling

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| Gen 5 source width | `-EINVAL` when the fetched span (doubled for YUV444) exceeds `FIELD_MAX(SCALER5_POS2_WIDTH_MASK)` = 8191 (gen 4: 4095) | read from source | `vc4_plane.c:1725-1731`, `vc4_regs.h:1271` |
| Gen 6 source width | **No equivalent check** in `vc6_plane_mode_set`; width − 1 is written into `SCALER6_POS2_SRC_WIDTH` (bits 12:0, 8191 max) with only the `WARN_ON` in `VC4_SET_FIELD`. Over-wide sources truncate silently on gen 6 | read from source (absence of a check: the `FIELD_MAX` grep over the function finds only the vdownsample call at `vc4_plane.c:2087-2088`); consequence inferred | `vc4_plane.c:2245-2246`, `vc4_plane.c:2289-2296`, `vc4_regs.h:1418` |
| Chroma narrow source | A multi-plane source narrower than one chroma sample (`src_w >> 16 < hsub` or likewise for height) is refused. The comment says on BCM2712 it corrupts other planes | read from source | `vc4_plane.c:627-633` |

## 2. LBM (line buffer memory)

### 2.1 Capacity and allocation

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| Capacity | Gen 4: 48 K words. **Gen 5: 60 K words = 61440** (4 × 12-bit pixels per word). **Gen 6: 1024** (set as a constant: the comment says a register read is not possible under KUnit, and the code does not read `SCALER6_LBM_SIZE`) | read from source | `vc4_hvs.c:1693-1711` |
| Allocator | `drm_mm` over `[0, lbm_size)`, plus an IDA of handles 1..`VC4_NUM_LBM_HANDLES` = 64 | read from source | `vc4_hvs.c:1717-1718`, `vc4_drv.h:304` |
| Unit | A size is in words on gen 5 and in lines (units of 128 / hsub components) on gen 6; `vc4_lbm_size` returns one or the other by generation | read from source | `vc4_plane.c:850-863` |
| Alignment | Gen 5: size aligned to 64 words. Gen 4: 32. Gen 6: none (the comment says the unit is words already) | read from source | `vc4_plane.c:1105-1113` |
| Reuse | A plane keeps its handle when the new size equals the old handle's size, otherwise it drops it and allocates anew. Two planes never share an LBM allocation | read from source | `vc4_plane.c:1123-1168` |
| Failure | Handle exhaustion (`Out of lbm_handles`) or `drm_mm_insert_node_generic` failure returns the error from `vc4_plane_atomic_check`, so a TEST_ONLY commit fails | read from source | `vc4_plane.c:1146-1165`, `vc4_plane.c:2486-2488` |
| Need | Zero when both `y_scaling[0]` and `y_scaling[1]` are NONE. Horizontal scaling alone needs no LBM | read from source | `vc4_plane.c:806-808`, `vc4_plane.c:855-857` |
| dlist slot | An extra dlist word is reserved for the LBM base whenever either y axis scales | read from source | `vc4_plane.c:1902-1905` (gen 5), `vc4_plane.c:2349-2357` (gen 6) |

### 2.2 Gen 5 sizing: `__vc4_lbm_size` (`vc4_plane.c:802-848`)

Let `P` = `crtc_w` if horizontal luma is TPZ, otherwise the fetched source span `vc4_src_span(src_x, src_w)` (`vc4_plane.c:797-800`, `vc4_plane.c:818-823`).

```
RGB, vertical luma TPZ : L = P * 8          (vc4_plane.c:829)
RGB, vertical luma PPF : L = P * 16         (vc4_plane.c:832)
any YUV                : L = P * 16         (vc4_plane.c:839)
L = roundup(L, 128)                         (vc4_plane.c:843)
words = L / 4                               (vc4_plane.c:846)
words = ALIGN(words, 64)                    (vc4_plane.c:1111)
```

### 2.3 Gen 6 sizing: `vc4_lbm_channel_size` / `__vc6_lbm_size` (`vc4_plane.c:868-956`)

```
width = (hsub>1 && channel==1) ? src_w                   (vc4_plane.c:906-907)
      : (x_scaling[channel]==TPZ) ? crtc_w                (vc4_plane.c:908-909)
      : min(src_w, crtc_w)                                (vc4_plane.c:911)
width *= 2  for YUV444 / YVU444                          (vc4_plane.c:912-913)
width  = round_up(ceil(width / hsub), 4)                 (vc4_plane.c:914)
wpc    = 4 if y_scaling[channel]==PPF, 2 if TPZ, else 0 (vc4_plane.c:853-866)
comps  = 0 if y NONE; YUV: channel ? 2 : 1;
         RGB: 4 if y TPZ; 4 if has_alpha and plane alpha opaque; else 3
                                                         (vc4_plane.c:868-886)
words  = width * wpc * comps                             (vc4_plane.c:924)
lines  = ceil(words / (128 / hsub))                      (vc4_plane.c:926)
size   = YUV ? max(lines(ch0), lines(ch1)) : lines(ch0)  (vc4_plane.c:930-937)
```

The result is the allocation size in units of the 1024-unit LBM.

### 2.4 Worked numbers, 1920 pixels wide (inferred from 2.2 and 2.3)

Assumptions: source width 1920 pixels, destination width 1920 (no horizontal scaling) so `P = 1920` and `min(src_w, crtc_w) = 1920`; vertical scaling present. Alpha property opaque (the default).

| Layer | Gen 5 words (of 61440) | Layers in 61440 | Gen 6 units (of 1024) | Layers in 1024 |
| --- | --- | --- | --- | --- |
| ARGB8888, vertical PPF | 1920 × 16 = 30720 → roundup 128 = 30720 → / 4 = 7680 → ALIGN 64 = **7680** | 8 | width 1920, wpc 4, comps 4 (alpha, opaque) → 30720 words → ceil(30720 / 128) = **240** | 4 (960) |
| XRGB8888, vertical PPF | **7680** (gen 5 does not look at alpha) | 8 | comps 3 → 23040 → **180** | 5 (900) |
| ARGB8888, vertical TPZ | 1920 × 8 = 15360 → / 4 = **3840** | 16 | wpc 2, comps 4 → 15360 → **120** | 8 (960) |
| ARGB8888 vertical PPF, 960-wide output with horizontal TPZ | `P = crtc_w = 960`: 960 × 16 = 15360 → **3840** (if the vertical mode is PPF; with vertical TPZ `P × 8` = 7680 → **1920**) | 16 (32) | width 960 (TPZ) → 960 × 4 × 4 = 15360 → **120** | 8 |
| NV12, 1:1 | y chroma forced PPF so LBM is needed: 1920 × 16 = 30720 → **7680** | 8 | ch0: y NONE → 0; ch1: width = src_w 1920 → ceil(1920 / 2) = 960, wpc 4, comps 2 → 7680 words → ceil(7680 / 64) = **120** | 8 (960) |
| NV12, vertical PPF scaled | **7680** | 8 | ch0: width 1920 → 960, wpc 4, comps 1 → 3840 → 60; ch1: 120; max = **120** | 8 |

Conclusions (inferred): gen 5 can hold about 8 full-width PPF-scaled layers of any format, 16 if vertical TPZ and RGB. Gen 6 is much tighter: **4 ARGB, 5 XRGB or 8 NV12 or TPZ layers** of 1920 width. NV12 always occupies LBM, even unscaled, on both generations. Both gens are also bounded by 64 LBM handles. Gen 6 cannot hold a single 8191-wide PPF ARGB layer: 8191 × 4 × 4 / 128 = 1024 lines exactly, a full LBM (inferred from the formula, rounding 8191 up to 8192 gives 1024).

## 3. UPM (gen 6 only)

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| Capacity | `drm_mm` of `1024 * HVS_UBM_WORD_SIZE` = 1024 × 256 = **262144 bytes**. The code comment says the real size could be read from a register but is not | read from source | `vc4_hvs.c:1721-1730`, `vc4_drv.h:397` |
| Handles | `VC4_NUM_UPM_HANDLES` = 32; one handle **per colour plane** of each plane's framebuffer (ARGB: 1, NV12: 2). Handle id minus 1 goes into a 5-bit field | read from source | `vc4_drv.h:317`, `vc4_plane.c:1203-1231`, `vc4_regs.h:1422` |
| Buffer lines | Fixed: `upm_buffer_lines = SCALER6_PTR0_UPM_BUFF_SIZE_2_LINES` (= 0), so `buffer_lines = 2 << 0` = 2. The 4, 8 and 16 line settings exist in `vc4_regs.h` but are never chosen | read from source | `vc4_plane.c:1201`, `vc4_plane.c:980`, `vc4_regs.h:1423-1427` |
| Size per colour plane | `stride = fb->pitches[plane]`; for SAND128 `ALIGN(fb->width, 128)` (P030: `ALIGN(width, 96) * 4 / 3`); doubled for the luma of YUV444. `words_per_line = (stride + 62) / 32`, `fetch_region = words_per_line * 32`, `size = ALIGN(fetch_region * buffer_lines, 256)` | read from source | `vc4_plane.c:960-984` |
| Alignment | Insertion alignment 256 bytes (`HVS_UBM_WORD_SIZE`). The base written to the dlist is `start / 256` in 13 bits | read from source | `vc4_plane.c:1236-1240`, `vc4_plane.c:1257-1258`, `vc4_regs.h:1421` |
| Reuse and failure | Same handle-reuse rule as LBM. `Out of upm_handles` or `drm_mm` failure returns an error from plane check; the allocation happens for every enabled plane regardless of scaling | read from source | `vc4_plane.c:1203-1250`, `vc4_plane.c:2490-2494` |
| Padding comment | "TODO: This only works for raster formats, and is sub-optimal for buffers with a stride aligned on 32 bytes" | read from source | `vc4_plane.c:975-977` |

Worked numbers (inferred from the rows above):

| Layer | Per colour plane | Per layer | Layers by memory (262144) | Layers by handles (32) |
| --- | --- | --- | --- | --- |
| ARGB8888 1920 × any, linear pitch 7680 | `(7680 + 62) / 32` = 241 → 7712 × 2 = 15424 → ALIGN 256 = 15616 | 15616 | 16 (16 × 15616 = 249856; a 17th gives 265472) | 32 |
| NV12 1920, linear, both pitches 1920 | `(1920 + 62) / 32` = 61 → 1952 × 2 = 3904 → ALIGN 256 = 4096 each | 8192 (2 handles) | 32 | 16 |
| XRGB8888 3840 wide linear, pitch 15360 | 481 → 15392 × 2 = 30784 → ALIGN 256 = 30976 | 30976 | 8 | 32 |

Conclusion (inferred): on gen 6 every enabled plane, scaled or not, consumes UPM. About 16 full-HD ARGB layers or 16 NV12 layers (handle bound) fit; planes at 4K are bound by UPM bytes at about 8. This resource group is independent of the LBM group.

## 4. `format_mod_supported` and the IN_FORMATS blob

### 4.1 What the driver accepts

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| Advertised modifier array | T_TILED, SAND128 (`COL_HEIGHT(0)`), SAND64 (`COL_HEIGHT(0)`), SAND256 (`COL_HEIGHT(0)`), LINEAR, then `MOD_INVALID`. The same array is passed on every generation, to every plane | read from source | `vc4_plane.c:2858-2865`, `vc4_plane.c:2877-2880` |
| Format list | `hvs_formats[]` filtered: `hvs5_only` entries dropped below gen 5, `hvs6_only` dropped below gen 6C. The `hvs5_only` entries are P030 and the 10-bit RGB formats, so they stay on gen 6; `hvs6_only` are the P01x / S01x formats | read from source | `vc4_plane.c:41`, `vc4_plane.c:193-217`, `vc4_plane.c:283-313`, `vc4_plane.c:2867-2875` |
| Hook | `.format_mod_supported = vc4_format_mod_supported` in `vc4_plane_funcs` | read from source | `vc4_plane.c:2845` |
| Gen 6 early reject | `gen >= VC4_GEN_6_C` and (modifier equals `T_TILED`, or the base modifier is SAND64) returns false for every format. Comment: "vc6_plane_mode_set() only handles linear, SAND128 and SAND256" | read from source | `vc4_plane.c:2767-2771` |
| RGB group | XRGB8888, ARGB8888, ABGR8888, XBGR8888, RGB565, BGR565, ARGB1555, XRGB1555: LINEAR and T_TILED | read from source | `vc4_plane.c:2773-2790` |
| NV12 / NV21 | LINEAR, SAND64, SAND128, SAND256 | read from source | `vc4_plane.c:2791-2799` |
| P030 | SAND128 only | read from source | `vc4_plane.c:2800-2806` |
| Everything else | LINEAR only, including the 10-bit RGB formats (the case list names the `RGBX1010102` family, but the advertised formats are the `XRGB2101010` family; they fall through the `default:` to LINEAR only either way), the 4444, 332, YUV420/422 3-plane, NV16/NV61, P010/P012/P016 and S010/S012/S016 | read from source | `vc4_plane.c:2807-2833` |
| Base modifier | The check compares `fourcc_mod_broadcom_mod(modifier)`, which strips the `COL_HEIGHT` parameter. So any `SAND*_COL_HEIGHT(n)` passes the support check for NV12 / NV21 / P030 | read from source | `vc4_plane.c:2776`, `vc4_plane.c:2791-2806` |

Result per generation (inferred from the rows):

| Format group | Gen 5 (BCM2711) | Gen 6 (BCM2712) |
| --- | --- | --- |
| XRGB8888, ARGB8888, ABGR8888, XBGR8888, RGB565, BGR565, ARGB1555, XRGB1555 | LINEAR, T_TILED | LINEAR |
| NV12, NV21 | LINEAR, SAND64, SAND128, SAND256 | LINEAR, SAND128, SAND256 |
| P030 | SAND128 | SAND128 |
| All other formats | LINEAR | LINEAR |

### 4.2 Where the blob comes from

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| vc4's part | vc4 builds no blob. It passes `formats[]` and `modifiers[]` to `drmm_universal_plane_alloc`, and the `format_mod_supported` hook in the plane funcs | read from source | `vc4_plane.c:2877-2880`, `vc4_plane.c:2845` |
| Core's part | The DRM core builds `IN_FORMATS` from `formats[]` × `modifiers[]` and, when the funcs have a `format_mod_supported` hook, keeps a (format, modifier) pair only if the hook returns true. This code is in the DRM core (`drm_mode_config.c`), not in the vc4 directory, so it was not read at the pin | inferred (core not fetched) | — |
| Expected blob at the pin | If the core behaves as said, the gen-6 blob has no T_TILED or SAND64 entries, and the gen-5 blob has the pairs in the tables of 4.1 | inferred | — |

### 4.3 Listed but rejected: explanations

The 2026-11-28 observation recorded in `docs/observed/bcm2712-topology.md` lists the Broadcom T-tiled code for XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 on the Pi 5, which the pin's `format_mod_supported` rejects on gen 6. Which explanation applies cannot be decided from source alone. Candidates (all inferred):

1. **The Pi 5 kernel is not the pin.** The gen-6 early reject (`vc4_plane.c:2767-2771`) was added at some point; an older `rpi-6.x.y` kernel (or a distribution kernel) without it would list T-tiled and SAND64 in the blob. The check is cheap: `uname -r` and the distribution's kernel source. This is the likeliest.
2. **The blob is built without the hook.** If the core only applies the hook in some paths, the blob would be the plain product of formats and modifiers. Then the blob would also list SAND on RGB formats and T-tiled on NV12, which the recorded blob does not (it lists T-tiled on exactly the RGB group of 4.1), so this explanation fits worse.
3. **Per-plane mismatch.** `format_mod_supported` is generation-wide, not per plane, so no plane should differ.

Pairs accepted by `format_mod_supported` but refused by `mode_set` (so a TEST_ONLY failure is still possible after a blob hit), all read from source:

| Case | Where |
| --- | --- |
| SAND with `COL_HEIGHT` parameter above 0xFFFF (`SCALER_TILE_HEIGHT_MASK` = bits 15:0) | gen 5 `vc4_plane.c:1630-1634`, `vc4_regs.h:1364`; gen 6 `vc4_plane.c:2140-2144` |
| P030 with SAND256 or SAND64 is rejected by `format_mod_supported` already (SAND128 only) | `vc4_plane.c:2800-2806` |
| Gen 6 SAND64 / T_TILED reach `vc6_plane_mode_set` only if userspace skips the support check; the `default:` returns `-EINVAL` | `vc4_plane.c:2236-2239` |
| Gen 6 SAND for a non-P030 format with a modifier other than SAND128 / SAND256 | `vc4_plane.c:2162-2164` |
| Gen 5 SAND for a non-P030 format with a modifier other than SAND64 / 128 / 256 | `vc4_plane.c:1656-1658` |

Conclusion for the profile (inferred): for the backend, treat the **generation, selected explicitly**, as the source of truth for gen 6: LINEAR, SAND128, SAND256 only. A blob that lists T-tiled or SAND64 on a Pi 5 should be recorded together with the kernel version, and TEST_ONLY of a T-tiled XRGB layer (`hvs-backend-design.md` §6) settles it per kernel.

## 5. Display-list words per plane

The dlist for a CRTC is the sum of its planes' `dlist_count` plus one end word, allocated from a `drm_mm` over words (`vc4_hvs.c:1162-1172`, `vc4_hvs.c:644-646`).

### 5.1 Memory

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| Gen 5 | `(SCALER_DLIST_SIZE >> 2) - 32` = 4096 − 32 = **4064 words** (first 32 reserved for the bootloader) | read from source | `vc4_hvs.c:1641-1642`, `vc4_regs.h:549`, `vc4_regs.h:1079` |
| Gen 6 | First init uses `HVS_READ(SCALER6_CXM_SIZE)` (4096 under KUnit). A second, unconditional `drm_mm_init` of `(SCALER_DLIST_SIZE >> 2) - 32` = 4064 words follows | read from source | `vc4_hvs.c:1647-1672`, `vc4_hvs.c:1682-1684` |
| Effect on gen 6 | The second init replaces the first, so the usable allocator range is 4064 words regardless of the register (no `drm_mm_takedown` between them) | inferred | `vc4_hvs.c:1670`, `vc4_hvs.c:1682` |
| Per-plane limit | `SCALER_CTL0_SIZE` (gen 5) and `SCALER6_CTL0_NEXT` (gen 6) are 6 bits (29:24), so one plane's dlist holds up to 63 words | read from source | `vc4_regs.h:1140`, `vc4_regs.h:1384`, `vc4_plane.c:1950`, `vc4_plane.c:2414` |
| Growth | The per-plane array doubles from 4 words and `dlist_count` counts the words used | read from source | `vc4_plane.c:469-485` |

### 5.2 Words per plane, by structure

Gen 5, `vc4_plane_mode_set` (`vc4_plane.c:1783-1950`):

| Part | Words | Reference |
| --- | --- | --- |
| Control word 0, position word 0, control word 2 | 3 | `vc4_plane.c:1783-1815` |
| Position word 1 (scaled size) | 1 if not `is_unity` | `vc4_plane.c:1819-1825` |
| Position word 2, position word 3 (context) | 2 | `vc4_plane.c:1827-1836` |
| Pointer words and pointer context words | 2 × `num_planes` | `vc4_plane.c:1846-1854` |
| Pitch words | `num_planes` | `vc4_plane.c:1857-1873` |
| Colour-space words | 3 if YUV | `vc4_plane.c:1876-1891` |
| LBM base slot | 1 if a y axis scales | `vc4_plane.c:1902-1905` |
| Scaling parameters per channel | H-PPF 1, V-PPF 2, H-TPZ 2, V-TPZ 3 (parameters, [reciprocal], context). YUV writes channel 1 then channel 0 | `vc4_plane.c:1003-1030` |
| PPF kernel pointers | 4 if any axis of either channel is PPF | `vc4_plane.c:1920-1948` |

Gen 6, `vc6_plane_mode_set` (`vc4_plane.c:2257-2414`):

| Part | Words | Reference |
| --- | --- | --- |
| Control word 0, position word 0, control word 2 | 3 | `vc4_plane.c:2257-2280` |
| Position word 1 | 1 if not `is_unity` | `vc4_plane.c:2282-2287` |
| Position word 2, position word 3 | 2 | `vc4_plane.c:2290-2298` |
| Per colour plane: pointer words 0, 1, 2 | 3 × `num_planes` | `vc4_plane.c:2303-2334` |
| Palette and trans words | 0 (marked TODO, not written) | `vc4_plane.c:2336-2345` |
| LBM base slot | 1 if a y axis scales | `vc4_plane.c:2349-2357` |
| Scaling parameters | as gen 5 | `vc4_plane.c:1003-1030`, `vc4_plane.c:2359-2373` |
| PPF kernel pointers | 4 if any PPF | `vc4_plane.c:2380-2408` |
| End word | 1 (`SCALER6_CTL0_END`) | `vc4_plane.c:2411` |
| No colour-space words (CSC mode is inside control word 2) | 0 | `vc4_plane.c:2275-2279` |

### 5.3 Worked words per plane (inferred from 5.2)

| Plane | Gen 5 | Gen 6 |
| --- | --- | --- |
| ARGB8888, unity | 3 + 2 + 2 + 1 = **8** | 3 + 2 + 3 + 1 (end) = **9** |
| ARGB8888, scaled both axes PPF | 3 + 1 + 2 + 2 + 1 + 1 (lbm) + 1 (H) + 2 (V) + 4 = **17** | 3 + 1 + 2 + 3 + 1 + 1 + 2 + 4 + 1 = **18** |
| ARGB8888, scaled both axes TPZ | 3 + 1 + 2 + 2 + 1 + 1 (lbm) + 2 (H) + 3 (V) = **15** | 3 + 1 + 2 + 3 + 1 + 2 + 3 + 1 = **16** |
| NV12, unity (chroma forced PPF) | 3 + 2 + 4 + 2 + 3 (csc) + 1 (lbm) + 1 (H ch1) + 2 (V ch1) + 4 (kernels) = **22** | 3 + 2 + 6 + 1 (lbm) + 3 + 4 + 1 = **20** |

Conclusion (inferred): 4064 words fit about 508 unity ARGB planes on gen 5 (4064 / 8), 239 scaled ARGB planes (4064 / 17) or 184 unity NV12 planes (4064 / 22), far above the 16 overlays the driver exposes. The binding resources per commit are therefore LBM units (gen 6), UPM bytes and handles (gen 6) and the 64 LBM handles, not dlist words. The dlist is shared by all CRTCs and stale entries are only freed by the frame-counter work, so back-to-back commits can fail on dlist space earlier than the static figure suggests (the retry path at `vc4_hvs.c:660-672` frees stale entries once).

## 6. What this changes for `docs/hvs-backend-design.md` (inferred)

- §7 Q1: the scaling field is a 16.16 ratio with 21 bits for TPZ (max 31.99998×) and 17 bits for PPF (max 1.99998×, never reached). The driver refuses a horizontal TPZ ratio above 31.99998× and a vertical one above 31.99998 × `vds`, with `vds` from the line-skip cap of §1.4.
- §5 blob versus source: the source rejects T-tiled and SAND64 on gen 6 in `format_mod_supported`; the blob at the pin should therefore not list them. A Pi 5 blob that does points at a different kernel; record `uname -r` with the blob.
- §3 capacities: gen 5 LBM 60 K words (about 8 full-width scaled layers), gen 6 LBM 1024 units (4 ARGB or 8 NV12 layers of 1920), gen 6 UPM 262144 bytes and 32 handles (16 full-HD ARGB or NV12 layers).
- Not covered here (needs a board or `vc4_kms.c`, `vc4_crtc.c`, not part of this read): bandwidth, `replace_pending`, and whether the hardware underruns before the driver refuses.
