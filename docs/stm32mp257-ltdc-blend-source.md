# STM32MP257 LTDC: zpos, plane alpha and blending, read from source (Q1 baseline)

Date of reading: 2026-11-28. **Nothing here is a board measurement and no picture has been taken.** Every row is tagged **read from source** (stated by the code read) or **inferred** (a conclusion drawn from it, including every predicted colour). The task statement asks for every row to be tagged read from source; rows that go beyond the text of the code are marked inferred instead, so that the tag stays true. This is the baseline for question Q1 (pixel colours of `twoplane`) of [`stm32mp257-session-c-runbook.md`](stm32mp257-session-c-runbook.md), in the style of [`stm32mp257-ltdc-transform-source.md`](stm32mp257-ltdc-transform-source.md) and [`stm32mp257-ltdc-yuv-source.md`](stm32mp257-ltdc-yuv-source.md).

## What was read

| Item | Value | Tag |
| --- | --- | --- |
| Driver | `drivers/gpu/drm/stm/ltdc.c` (2943 lines) of `STMicroelectronics/linux`, branch `v6.6-stm32mp`, commit `548f960c059bc4b9165cb69895fd67551f6061ca`, the commit the other baselines pin. All `ltdc.c:N` below are lines of that file (a copy kept in the dependency cache; it was fetched with `curl` earlier on 2026-11-28 and its line count matches). | read from source |
| DRM core | `drivers/gpu/drm/drm_blend.c` of mainline v6.6, from the `vkms-v6.6` tree in the dependency cache: `drm_plane_create_alpha_property` (`drm_blend.c:219-236`), `drm_plane_create_zpos_property` (`369-390`), `drm_plane_create_zpos_immutable_property` (`408-429`), the zpos paragraph of the file comment (`130-139`). The same file at the ST commit above was **not** fetched (the fetch was refused), so that the ST tree's core is the same as mainline 6.6 is inferred. | read from source (mainline 6.6); inferred for the ST tree |
| Board kernel | The board runs 6.6.129 (per the other baselines); that its `ltdc.c` equals the branch tip is inferred, as in [`stm32mp257-ltdc-source-diff-6.6.md`](stm32mp257-ltdc-source-diff-6.6.md). | inferred |
| Example | `crates/composition-drm/examples/twoplane.rs` and `scripts/alpha-mode-expect.py` (this repository), for what Q1 draws and the arithmetic of the models. | read from source |

## 1. Which planes, and zpos

| Fact | Where | Tag |
| --- | --- | --- |
| Layer count is `LTDC_LCR` clamped to 1..`LTDC_MAX_LAYER`; on `st,stm32mp25-ltdc` a layer `l3` that the firewall reserves is removed from the count, so the driver can expose fewer planes than the hardware has. Layer 0 is the primary plane, layers 1 to `nb_layers - 1` are overlays | `ltdc.c:2415`, `2452-2462`, `2299-2300` | read from source |
| `dynamic_zorder` is `true` only for hardware 4.01.00 and 4.01.01 (`HWVER_40100`, `HWVER_40101`) and `false` for 1.02.00, 1.03.00 and 2.01.01. The MP257 uses the 4.x block (`caps` with `ycbcr_input`, `crc`, `plane_reg_shadow`) | `ltdc.c:2501`, `2527`, `2547`, `2552-2567` | read from source |
| Primary: with `dynamic_zorder`, `drm_plane_create_zpos_property(primary, 0, 0, nb_layers - 1)`, a **mutable** zpos, default 0, range 0 to `nb_layers - 1`; otherwise an immutable zpos of 0 | `ltdc.c:2258-2261` | read from source |
| Overlay `i`: with `dynamic_zorder`, `drm_plane_create_zpos_property(overlay, i, 0, nb_layers - 1)`, mutable, **default `i`**, the same range; otherwise immutable zpos `i`. Every overlay has the same range as the primary: any plane may take any value in the range | `ltdc.c:2305-2308` | read from source |
| So for three layers each plane has a mutable zpos with range 0 to 2 and defaults 0 (primary), 1, 2 (overlays in layer order). That `nb_layers` is 3 on this board is not shown by the source; the recorded property dump is in [`observed/stm32mp257.md`](observed/stm32mp257.md) | | inferred |
| Core meaning of zpos: "planes with lower Z position values are underneath planes with higher Z position values"; two planes with the same value have undefined order | `drm_blend.c:130-139` | read from source (mainline 6.6) |
| The values written to hardware are the **normalised** zpos (`newstate->normalized_zpos`, a gap-free 0..n-1 ranking of the planes on the CRTC, computed by the core), not the raw property | `ltdc.c:1836` | read from source |
| How `normalized_zpos` is computed (sort by the raw zpos, ties broken by plane id) is the core's `drm_atomic_normalize_zpos` (`drm_blend.c:510`), not read here beyond its existence | | inferred |
| Plane indices 34, 42 and 49 are the object ids the board assigned to layers 0, 1 and 2 (the order of creation); the source does not give ids | `ltdc.c:2259`, `2299-2306` | inferred |

## 2. Plane alpha

| Fact | Where | Tag |
| --- | --- | --- |
| Every plane (primary and overlays) gets the `alpha` property from `ltdc_plane_create` | `ltdc.c:2228` | read from source |
| `drm_plane_create_alpha_property`: range 0 to `DRM_BLEND_ALPHA_OPAQUE` (0xffff), default 0xffff; the state alpha is set to opaque | `drm_blend.c:219-232` | read from source (mainline 6.6) |
| Register: `LTDC_L1CACR` (constant alpha configuration), field `LXCACR_CONSTA = GENMASK(7, 0)`, written with `newstate->alpha >> 8` on each plane update. So the 16-bit property is **truncated to 8 bits**: 0xffff gives 0xff, 32768 (0x8000) gives 0x80, 0 gives 0. Property values in 0x00ff..0x0000 collapse to 0 | `ltdc.c:227`, `1818-1820` | read from source |
| Constant alpha maximum is `CONSTA_MAX = 0xFF`, "1.0" | `ltdc.c:259` | read from source |
| On plane disable the driver writes `LTDC_L1CACR = 0` ("hide any related background color") after clearing the layer enable | `ltdc.c:2073-2077` | read from source |
| Layer register addresses of `LTDC_L1CACR` in the 4.x table: offset `0x120` in `ltdc_layer_regs_a2` (the older tables a0 and a1 use `0x98` and `0x9c`) | `ltdc.c:402`, `436`, `470` | read from source |
| The 8-bit hardware factor is `CONSTA / 255`; this is a datasheet-level fact the source only implies (a "1.0" at 0xFF), so 0x80 is taken as 128/255 = 0.502, not 0.5 | `ltdc.c:259` | inferred |

## 3. Blend mode

| Fact | Where | Tag |
| --- | --- | --- |
| The driver creates **no** `pixel blend mode` property: the only property-creating calls in the plane path are alpha, zpos, rotation, colour encoding/range and scaling (`grep -n 'blend_mode' ltdc.c` finds nothing; `drm_blend.h` is included at `ltdc.c:29` for alpha, zpos and rotation) | `ltdc.c:2228`, `2258-2261`, `2305-2308` | read from source |
| So the core's default applies: userspace that reads "no property" treats the plane as premultiplied. composition-drm reports `blend [premultiplied]` for all three planes and refuses `--blend coverage` before any commit (`NoPlane ... BLEND`), as in [`observed/stm32mp257-kvm.md`](observed/stm32mp257-kvm.md) | `drm_blend.c:130-139` region (the property text); behaviour from the existing record | inferred |
| The blend factors are fixed by the driver, not by a property. Constants: `BF1_PAXCA = 0x600` (pixel alpha x constant alpha), `BF1_CA = 0x400` (constant alpha), `BF2_1PAXCA = 0x007` (1 - pixel alpha x constant alpha), `BF2_1CA = 0x005` (1 - constant alpha). Fields of `LTDC_L1BFCR`: `BF2 = GENMASK(2, 0)`, `BF1 = GENMASK(10, 8)`, `BOR = GENMASK(18, 16)` ("Blending ORder") | `ltdc.c:229-231`, `260-263` | read from source |
| Choice per plane update: default `BF1_PAXCA | BF2_1PAXCA` (formats **with** an alpha channel); `BF1_CA | BF2_1CA` when the framebuffer format has **no** alpha (`fb->format->has_alpha` false, for example XR24) | `ltdc.c:1825-1828` | read from source |
| The `non_alpha_only_l1` quirk (force `PAXCA` on non-primary planes; hide XR24-like formats on overlays) applies to hardware 1.x and 2.x only; it is `false` for 4.x, so overlays on the MP257 may use XR24 and take the `CA` factors | `ltdc.c:1831-1833`, `2173-2175`, `2561` | read from source |
| Meaning: `BF1` multiplies the **layer's colour** by (pixel alpha x constant alpha) and `BF2` multiplies the colour already below it by one minus that: `out = Cs x (Ap x CA) + Cd x (1 - Ap x CA)`. That is **straight (non-premultiplied)** blending, not `out = Cs + Cd x (1 - alpha)`. A premultiplied buffer is therefore darkened twice: once at write time and once at blend time | `ltdc.c:260-263`, `1825-1828` | inferred (from the factor names; the source has no formula) |
| That reading is consistent with the output-CRC measurements the runbook calls session A, Q2 ("the hardware blends straight"); that is a record of a measurement, not a source row | [`stm32mp257-session-c-runbook.md`](stm32mp257-session-c-runbook.md) section 3 | inferred |
| The constant-alpha-only case (`BF1_CA | BF2_1CA`) ignores any pixel alpha, so an XR24 plane at plane alpha `a` blends `Cs x a + Cd x (1 - a)`; this is what a background plane gets | `ltdc.c:1826-1828` | read from source |
| Colour keying is not used: `LXCR_COLKEN` appears only in the definitions of the mask; the plane update never sets it | `ltdc.c:212`, `216`, `1973-1977` | read from source |
| Default colour of a layer is black (`LTDC_L1DCCR = DCCR_DCBLACK`), and the CRTC background colour `LTDC_BCCR` is black (`BCCR_BCBLACK`) | `ltdc.c:176`, `182`, `1822`, `1263` | read from source |

## 4. How layer order and alpha reach the registers

All of this is in `ltdc_plane_update` (the function of the transform baseline, called from `ltdc_plane_atomic_update` and the enable path), once per plane per commit.

| Step | What the code does | Where | Tag |
| --- | --- | --- | --- |
| Pixel format | `LTDC_L1PFCR` (or the flexible format) | `ltdc.c:1805-1816` | read from source |
| Constant alpha | `LTDC_L1CACR + lofs` = `alpha >> 8` | `ltdc.c:1818-1820` | read from source |
| Default colour | `LTDC_L1DCCR + lofs` = black | `ltdc.c:1822` | read from source |
| Blend factors and order | `val` = `BF1`/`BF2` as in section 3; with `dynamic_zorder`, `val |= normalized_zpos << 16` and `LTDC_L1BFCR + lofs` is written under `BF2 | BF1 | BOR`; without it only `BF2 | BF1` are written and the order stays at the hardware default (layer index) | `ltdc.c:1825-1842` | read from source |
| `BOR` is the zpos, with no inversion or offset: layer register block `lofs` (layer `n`) gets `BOR = normalized_zpos` of **that** plane. So moving a plane's zpos changes its `BOR`, not which register block it uses | `ltdc.c:1836-1839` | read from source |
| Whether a larger `BOR` is blended later (on top) is defined by the controller, not by the driver source. The field name "Blending ORder" and the straight mapping of DRM's "higher value is on top" suggest it, but the source does not state it | | inferred |
| Reload | One `LTDC_L1RCR + lofs = LXRCR_VBR` per updated layer, with `LXRCR_IMR | VBR | GRMSK` as the mask: vertical-blanking reload of that layer's shadow registers (not an immediate or centralised reload) | `ltdc.c:2012-2015` | read from source |
| Atomicity across layers | Both planes of a swap are written in the same atomic commit, before the next vertical blank, so both reloads land at that blank; the source has no cross-layer lock | `ltdc.c:2012-2015` | inferred |
| Disable | Layer disabled and its `CACR` set to 0 | `ltdc.c:2073-2077` | read from source |
| Atomic check | `ltdc_plane_atomic_check` has no test on alpha, zpos or blend (transform baseline, diff row 35); the core checks the property ranges and `drm_atomic_normalize_zpos` | `ltdc.c:1566-1605` | read from source |

## 5. What twoplane draws

From `twoplane.rs` and `alpha-mode-expect.py` defaults: a 1920x1080 background on plane layer 0 (colour `#202020`, three blocks red `#ff0000`, green `#00ff00`, blue `#0000ff`, a moving bar), and a translucent-test overlay of `w/5 x h/2` at `(w/2, h/4)`. The overlay's upper half is opaque yellow `#ffff00`; its lower half is magenta `#ff00ff` at pixel alpha 0x80, **stored premultiplied** (`#800080`) because the planes report no blend property, as in [`observed/stm32mp257-kvm.md`](observed/stm32mp257-kvm.md). With `--swap-z`, at frame 300 the two layers exchange planes and zpos in one atomic commit, if every plane's zpos is a range (`swap_plan` in `twoplane.rs`, which holds here by section 1).

| Fact | Tag |
| --- | --- |
| Constants `BACKGROUND = #202020`, `PIXEL_ALPHA = 0x80`, `MAGENTA`, `YELLOW`, blocks, and the `over` equations (`Opaque`, `Premultiplied`, `Coverage`) | read from source (`twoplane.rs:116-124`, `350-380`) |
| The Q1 arithmetic below uses the **straight** equation of section 3 with the 8-bit constant alpha of section 2, the buffers twoplane writes (premultiplied), and rounding to nearest. The hardware's rounding or truncation is not in the driver, so the last digit of a colour may differ by one | inferred |

## 6. Predicted Q1 picture per command

All rows are **inferred** from sections 1 to 5. "Under" colours are those the overlay covers: green block `#00ff00`, or the background `#202020`.

### Q1a: `twoplane /dev/dri/card0 --frames 600 --swap-z`

Plane 34 (layer 0) carries the XR24 background, `CA = 0xff`, factors `CA`/`1-CA`, `BOR = 0`; plane 42 (layer 1) carries the AR24 overlay, `CA = 0xff`, factors `PAxCA`/`1-PAxCA`, `BOR = 1`; plane 49 is not used (its layer stays off).

| Phase | Region | Predicted colour | Why | Tag |
| --- | --- | --- | --- | --- |
| Normal (about 2 s) | Background, red, green and blue blocks, moving bar | as drawn | Layer 0 has `BF1 = CA = 1.0`, `BF2 = 0`: it replaces the black `BCCR` | inferred |
| Normal | Overlay upper half over green or background | `#ffff00` | Opaque pixel alpha 0xff times `CA` 0xff is 1.0 | inferred |
| Normal | Overlay lower half over green | `#407f40` (twoplane's premultiplied EXPECT is `#807f80`) | `0.502 x #800080 + 0.498 x #00ff00`: red 64, green 127, blue 64 | inferred |
| Normal | Overlay lower half over background | `#501050` (EXPECT `#901090`) | `0.502 x #800080 + 0.498 x #202020`: red 80, green 16, blue 80 | inferred |
| Swapped (about 7 s) | Everything | background, blocks and bar; **no overlay visible** | Plane 42 now carries the XR24 full-screen background at the higher zpos 1 (`BOR = 1`, `CA = 0xff`, `BF2 = 0`); it covers plane 34, which carries the AR24 overlay at `BOR = 0`. The picture equals the background alone | inferred |
| Swapped | Counter squares in the overlay's upper right | not visible | They are in the hidden overlay buffer | inferred |

What would show a different reading: if the overlay stays visible after 5 s, `BOR` orders the layers the other way round (a larger `BOR` blended first) or the swap did not reach the registers; if the lower half is `#807f80` the controller blends premultiplied, against the factor names.

### Q1b: `twoplane /dev/dri/card0 --frames 600 --plane-alpha 32768`

The overlay's alpha property is 32768, so `CACR = 0x80` on plane 42 only (the background keeps 0xff). `PAxCA` is `0x80/255 x pixel alpha/255`.

| Region | Predicted colour | Computation | Tag |
| --- | --- | --- | --- |
| Overlay upper half over green | `#80ff00` | `0.502 x #ffff00 + 0.498 x #00ff00` | inferred |
| Overlay upper half over background | `#8f8f0f` or `#909010` | `0.502 x 255 + 0.498 x 32` is about 144 (0x8f or 0x90, by the hardware's rounding), blue `0.498 x 32` about 16 | inferred |
| Overlay lower half over green | `#20bf20` (twoplane EXPECT `#40bf40`) | factor `0.502 x 0.502 = 0.252`: `0.252 x #800080 + 0.748 x #00ff00` is red 32, green 191, blue 32 | inferred |
| Overlay lower half over background | `#381838` (EXPECT `#581858`) | `0.252 x #800080 + 0.748 x #202020`: red 56, green 24, blue 56 | inferred |
| Unrelated regions, bar | as drawn | The background layer is unchanged | inferred |

These match the "predicted verdicts under straight alpha" table of the runbook section 3 (derived there from CRC measurements); this document gets the same colours from the factors of `ltdc.c:1825-1828` independently. The distances of the translucent regions from EXPECT are about 91 (Q1a) and about 45 (Q1b), so at tolerance 40 `camera-check.py` should fail Q1a's translucent regions clearly and Q1b's marginally (inferred, as in the runbook).

### What the source does not settle

| Question | Why open | Tag |
| --- | --- | --- |
| Direction of `BOR` (is higher blended last?) | The driver writes the value; the controller defines it. Q1a's swapped phase decides it | inferred |
| Rounding of the 8-bit blend | Hardware property, not in the driver | inferred |
| That the board sets `nb_layers` to 3 and plane 49 is layer 2 | Depends on `LTDC_LCR` and the firewall at run time | inferred |
| Plane 49's behaviour | Same registers and code at its own `lofs`; no source difference from plane 42 (section 4 has no layer-index condition other than the 1.x/2.x quirk) | inferred |

## Not claimed

- No picture, CRC or register value was observed for this document.
- Nothing here changes `crates/composition-drm/src/profile.rs`. The profile claims `blend [premultiplied]` for the LTDC planes because of the missing property; section 3 says the hardware factors are straight, so a profile claim of "premultiplied" is a statement about the KMS interface, not about the hardware arithmetic. Whether to change it belongs to the picture session.
