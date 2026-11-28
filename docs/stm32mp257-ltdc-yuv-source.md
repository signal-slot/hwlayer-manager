# STM32MP257 LTDC: YCbCr-to-RGB conversion, read from source

Date of reading: 2026-11-28. **Nothing here is a board measurement.** Every row is tagged **read from source** (stated by the code) or **inferred** (a conclusion drawn from it). It exists so that a later KVM picture session can judge the NV12 colours on plane 42 against a prediction. It adds to [`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md) and [`stm32mp257-ltdc-source-diff-6.6.md`](stm32mp257-ltdc-source-diff-6.6.md), which cover `ycbcr_input` and the caps but not the coefficients.

## What was read, and how good the line numbers are

| Item | Value | Tag |
| --- | --- | --- |
| Tree | `STMicroelectronics/linux`, branch `v6.6-stm32mp`, tip commit `548f960c059bc4b9165cb69895fd67551f6061ca` (the same commit the 6.6 comparison in `stm32mp257-ltdc-source-diff-6.6.md` pins; its `Makefile` says 6.6.129, the board's version) | read from source |
| Files | `drivers/gpu/drm/stm/ltdc.c`, `drivers/gpu/drm/stm/ltdc.h` | read from source |
| How | `curl` of `raw.githubusercontent.com/STMicroelectronics/linux/v6.6-stm32mp/drivers/gpu/drm/stm/ltdc.c` on 2026-11-28 (the branch name resolved to the tip above at that time; the commit SHA was not re-queried). The file has 2943 lines, the same count the 6.6 comparison reports for `548f960c`. | read from source |
| Line numbers | Exact, counted with `grep -n` / `sed -n` on that file. Re-check with `grep -n 'CYR0R\|ycbcr2rgb\|ltdc_set_ycbcr' ltdc.c`. | read from source |
| `ycbcr_input` per version | `ltdc_get_caps` sets `ycbcr_input = true` and `ycbcr_output = true` for `HWVER_40100` and `HWVER_40101` (`ltdc.c` 2552-2564) and `false` for the older versions (`ltdc.c` 2497, 2523, 2543). | read from source |
| Board kernel | Whether the board kernel (6.6.129) has the same `ltdc.c` as the branch tip is **inferred**, as stated in `stm32mp257-ltdc-source-diff-6.6.md`. | inferred |

## How the driver programs the conversion

| Fact | Detail (6.6, `548f960c`) | Tag | Where |
| --- | --- | --- | --- |
| Registers | `LTDC_L1CYR0R` ("L1 Conversion YCbCr RGB 0") and `LTDC_L1CYR1R` ("L1 Conversion YCbCr RGB 1"), per layer; layer register slots 27 and 28; offsets `0x16c` and `0x170` in the layer table `ltdc_layer_regs_a2` (slots 29 and 30 are the flexible pixel format registers `0x174`, `0x178`). The other two tables (`a0`, `a1`) mark them `0x00 /* not available */`. | read from source | `ltdc.c` 124-125 (macros), 489-491 (a2 offsets), 393-459 (a0/a1) |
| Where the layer offset comes from | `lofs = plane->index * LAY_OFS`, so every plane has its own coefficient pair | read from source | `ltdc.c` 978-1002 |
| Access | The regmap allows `LTDC_L1CYR0R/1R` only for hardware versions `HWVER_40100` and `HWVER_40101` | read from source | `ltdc.c` 609-611 (readable), 727-729 (writeable) |
| Coefficient table | `static const u32 ltdc_ycbcr2rgb_coeffs[DRM_COLOR_ENCODING_MAX][DRM_COLOR_RANGE_MAX][2]`; index `[0]` is written to `CYR0R`, index `[1]` to `CYR1R`. Four entries, see next table. `/* BT2020 not supported */` | read from source | `ltdc.c` 782-800 |
| Programming | `ltdc_set_ycbcr_coeffs(plane)` reads `plane->state->color_encoding` and `color_range` and writes the two table words | read from source | `ltdc.c` 978-1002 |
| Fallback | an encoding that is neither BT.601 nor BT.709: log "color encoding %d not supported, use bt601 by default", use BT.601. A range that is neither limited nor full: log "color range %d not supported, use limited range by default", use limited | read from source | `ltdc.c` 986-996 |
| Mode register | `ltdc_set_ycbcr_config(plane, fourcc)` writes `LTDC_L1PCR` (layer planar configuration): `YCM` (bits 5:4) = `YCM_I` 0 interleaved 4:2:2, `YCM_SP` 1 semi-planar 4:2:0, `YCM_FP` 2 full-planar 4:2:0; `LxPCR_YCEN` (bit 3) "YCbCr-to-RGB Conversion Enable"; `LxPCR_YF` (bit 6) Y first; `LxPCR_CBF` (bit 7) CB first; `LxPCR_OF` (bit 8) odd pixel first; `LxPCR_YREN` (bit 9) "Y Rescale Enable for the color dynamic range" | read from source | `ltdc.c` 243-251 (bits), 932-975 (function) |
| Format to mode | YUYV: `YCM_I`, `YF`, `CBF`. YVYU: `YCM_I`, `YF`. UYVY: `YCM_I`, `CBF`. VYUY: `YCM_I`. NV12: `YCM_SP`, `CBF`. NV21: `YCM_SP`. YUV420 (YU12) and YVU420 (YV12): `YCM_FP` (YV12 swaps the two chroma planes in the address programming instead). Any other format: `DRM_ERROR("Unsupported pixel format")`, return | read from source | `ltdc.c` 940-960 |
| Range handling | After the switch: `if (state->color_range == DRM_COLOR_YCBCR_LIMITED_RANGE) val |= LxPCR_YREN;` then `val |= LxPCR_YCEN;`, then write `LTDC_L1PCR`. So `YREN` (Y rescale) is set exactly when the range is **limited**. Full range clears it. | read from source | `ltdc.c` 969-973 (write follows) |
| When it runs | In `ltdc_plane_update`, inside `if (ldev->caps.ycbcr_input)`: if `fb->format->is_yuv`, `ltdc_set_ycbcr_coeffs(plane)` and `ltdc_set_ycbcr_config(plane, fb->format->format)` run on every commit that has a YUV framebuffer; for an RGB framebuffer `LTDC_L1PCR` is written 0 ("disable ycbcr conversion") | read from source | `ltdc.c` 1872-1873, 1953-1956, else branch 1957-1960 |
| Properties honoured | **Yes, both.** `drm_plane_create_color_properties(plane, BIT(BT601)\|BIT(BT709), BIT(LIMITED_RANGE)\|BIT(FULL_RANGE), DRM_COLOR_YCBCR_BT601, DRM_COLOR_YCBCR_LIMITED_RANGE)` is called when `ycbcr_input` and the layer reports `YIA`, `YSPA` or `YFPA`; the driver reads the plane state values at commit (row above). Nothing is hard-coded except the fallbacks. | read from source | `ltdc.c` 2215-2224 |
| Defaults | **BT.601, limited range** (the `default_encoding`/`default_range` arguments above, and the fallbacks) | read from source | `ltdc.c` 2215-2224, 986-996 |
| Supported choices | BT.601, BT.709; limited, full. No BT.2020. | read from source | `ltdc.c` 2215-2224 |
| Output side (not used for planes) | `if (ldev->caps.ycbcr_output)` in `ltdc_crtc_atomic_enable` selects an RGB-to-YCbCr output converter by CEA VIC (BT.601 for VIC 6, 7, 21, 22, 2, 3, 17, 18, else BT.709) when the bus format is `YUYV8_1X16` or `YVYU8_1X16` (`EDCR_OCYEN`, `EDCR_OCYSEL`, `EDCR_OCYCO`). Not relevant to the DSI/HDMI/LVDS RGB paths of this board. | read from source (relevance: inferred) | `ltdc.c` 1233-1253 and the `EDCR_*` defines 207-209 |
| `ltdc.h` | Only `ycbcr_input` and `ycbcr_output` booleans in `struct ltdc_caps`. No register offsets, no coefficients. | read from source | `ltdc.h` (`struct ltdc_caps`) |

### The coefficient words

Each word packs two 11-bit-ish values (high half and low half); the driver's own comments give the decimal values. The factor is `value / 256` (**inferred**, see next section).

| Encoding, range | `CYR0R` (b_cb / r_cr) | `CYR1R` (g_cb / g_cr) | Tag | Where |
| --- | --- | --- | --- | --- |
| BT.601, limited | `0x02040199` (516 / 409) | `0x006400D0` (100 / 208) | read from source | `ltdc.c` 783-785 |
| BT.601, full | `0x01C60167` (454 / 359) | `0x005800B7` (88 / 183) | read from source | `ltdc.c` 787-789 |
| BT.709, limited | `0x021D01CB` (541 / 459) | `0x00370089` (55 / 137) | read from source | `ltdc.c` 791-793 |
| BT.709, full | `0x01DB0193` (475 / 403) | `0x00300078` (48 / 120) | read from source | `ltdc.c` 795-797 |

### Reading the coefficients (inferred)

The hardware formula is not in the driver; it is in ST's reference manual, which was not read. The values fit the usual equations with a scale of 1/256, which is **inferred**:

| Check | Value | Tag |
| --- | --- | --- |
| BT.601 full: `r_cr` 359/256 | 1.402 (the BT.601 `Cr` to R factor) | inferred |
| BT.601 full: `b_cb` 454/256 | 1.773 (the BT.601 `Cb` to B factor 1.772) | inferred |
| BT.709 full: `r_cr` 403/256 | 1.574 (BT.709 1.5748) | inferred |
| BT.709 full: `b_cb` 475/256 | 1.855 (BT.709 1.8556) | inferred |
| Limited entries: chroma factor = full factor × 255/224 (e.g. 601 `r_cr` 1.402 × 1.138 = 1.596 = 409/256) | the chroma scaling for 224-level chroma is folded into the coefficients | inferred |
| Limited range sets `LxPCR_YREN` | the luma is rescaled by 255/219 = 1.164 by the separate Y-rescale step, since the limited coefficients carry no luma factor | inferred |

So under the model: `Y' = (Y-16)*255/219` for limited (`YREN` set), `Y' = Y` for full; `R = Y' + (r_cr/256)(Cr-128)`; `G = Y' - ((g_cb/256)(Cb-128) + (g_cr/256)(Cr-128))`; `B = Y' + (b_cb/256)(Cb-128)`; clamp to 0..255. Rounding and the internal precision are not known (**inferred**; expect ±1 to ±2 on a picture).

## What the board advertises

| Fact | Value | Tag |
| --- | --- | --- |
| `COLOR_ENCODING` and `COLOR_RANGE` exist on planes 34 and 42 only (property ids 36/37 and 44/45); plane 49 has neither | `docs/observed/stm32mp257.md` "YUV properties" row: "`COLOR_ENCODING` (BT.601, BT.709) and `COLOR_RANGE` on planes 34 and 42 only", **observed 2026-11-27**; modetest listing of the same: "36 COLOR_ENCODING 37 COLOR_RANGE" (plane 34), "44 COLOR_ENCODING 45 COLOR_RANGE" (plane 42), **observed 2026-11-28** (`docs/observed/stm32mp257.md`; `docs/observed/stm32mp257-topology.md` planes table "COLOR_ENCODING, COLOR_RANGE", observed 2026-11-28) | observed 2026-11-27 |
| The encodings listed are BT.601 and BT.709 only; matches the source `BIT(BT601)\|BIT(BT709)` | quoted from the same row | observed 2026-11-27 |
| The **current values** of the two properties on plane 42 were **not recorded**: modetest prints no enum value or current value for them in the recorded listings, and the topology note says probe omits `COLOR_ENCODING/RANGE`. | gap | observed 2026-11-28 (absent from the listings) |
| The board's default value is BT.601 / limited range | follows from the source default and that the driver registers the properties with those defaults; no commit on the board has changed them in any recorded session | inferred |

A picture session should read the two values on plane 42 (`modetest -p`, or `drm_info`) before the NV12 run and state them, and should set them explicitly in the run that it wants to judge, rather than rely on the default.

## Predicted RGB for a few YUV triples

Computed with the model above (script not kept; the arithmetic is the formula in "Reading the coefficients"). The YUV triples are the standard 8-bit ones for each encoding. The prediction is for a flat colour: the same Y, Cb, Cr over a whole region, so chroma subsampling and interpolation do not matter. All rows: **inferred** from source (coefficients **read from source**, the hardware formula and rounding **inferred**).

### Default: BT.601, limited range (`COLOR_ENCODING` BT.601, `COLOR_RANGE` limited)

| Colour | Y, Cb, Cr | Predicted R, G, B | Tag |
| --- | --- | --- | --- |
| black | 16, 128, 128 | 0, 0, 0 | inferred |
| white | 235, 128, 128 | 255, 255, 255 | inferred |
| red | 81, 90, 240 | 255, 0, 0 | inferred |
| green | 145, 54, 34 | 0, 255, 1 | inferred |
| blue | 41, 240, 110 | 0, 0, 255 | inferred |

### BT.709, limited range

| Colour | Y, Cb, Cr | Predicted R, G, B | Tag |
| --- | --- | --- | --- |
| black | 16, 128, 128 | 0, 0, 0 | inferred |
| white | 235, 128, 128 | 255, 255, 255 | inferred |
| red | 63, 102, 240 | 255, 0, 0 | inferred |
| green | 173, 42, 26 | 0, 255, 1 | inferred |
| blue | 32, 240, 118 | 1, 0, 255 | inferred |

### BT.601, full range (`LxPCR_YREN` clear)

| Colour | Y, Cb, Cr | Predicted R, G, B | Tag |
| --- | --- | --- | --- |
| black | 0, 128, 128 | 0, 0, 0 | inferred |
| white | 255, 128, 128 | 255, 255, 255 | inferred |
| red | 76, 85, 255 | 254, 0, 0 | inferred |
| green | 150, 44, 21 | 0, 255, 1 | inferred |
| blue | 29, 255, 107 | 0, 0, 254 | inferred |

### Mismatches a picture would show (inferred)

| If the source file and the plane disagree | Effect | Tag |
| --- | --- | --- |
| Limited-range data on a plane set to full range | Black appears as grey (Y 16 gives 16, not 0) and white as 235; low contrast | inferred |
| Full-range data on a plane set to limited range | Y below 16 and above 235 clips: crushed blacks and clipped whites | inferred |
| BT.601 data read with BT.709 | Reds and greens shift a few tens of levels; neutral greys are unchanged | inferred |

Grey (Cb = Cr = 128) is independent of the encoding and does not test it; use red, green and blue for that. A stricter picture test also needs a camera that does not alter the colours: `scripts/camera-check.py` compares against EXPECT values with its own tolerance, so the ±2 rounding above is inside any sensible tolerance, while a 601/709 mix-up is not.

## For the picture session

1. The plane-42 NV12 prediction is the **BT.601 limited** table unless the session sets the properties otherwise. Both properties are honoured by the driver, so a run may set `COLOR_ENCODING` BT.709 and `COLOR_RANGE` full and compare against the other tables.
2. A flat red, green, blue and black/white NV12 frame cover the table. Grey is not enough.
3. If the picture does not match any of the three tables, suspect the 6.6 kernel on the board differs from `548f960c` (inferred, as in `stm32mp257-ltdc-source-diff-6.6.md`), or the model's Y-rescale or rounding, rather than the coefficients, which are read from source.
