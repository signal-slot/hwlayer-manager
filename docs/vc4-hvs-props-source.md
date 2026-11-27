# VC4 HVS5 / HVS6: plane properties (zpos, alpha, blend, rotation, colour) read from source

Dated 2026-11-28. Feeds the `Vc4Hvs5` / `Vc4Hvs6` profile tables of `docs/hvs-backend-design.md` §4. Nothing here was run on a board.

## Provenance

- Tree: `raspberrypi/linux`, branch `rpi-6.18.y`, commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e`, directory `drivers/gpu/drm/vc4/`.
  Link: <https://github.com/raspberrypi/linux/tree/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4>
- Files read at that commit: `vc4_plane.c`, `vc4_hvs.c`, `vc4_regs.h`, `vc4_drv.h` (raw copies from the dependency cache, line numbers exact), and for `vc4_crtc.c` and `vc4_kms.c` only excerpts returned by the web-fetch route. The DRM core (`drm_blend.c`, `drm_atomic_helper.c`, `drm_plane.c`) was not fetched.
- Line numbers: `vc4_plane.c` and `vc4_hvs.c` are exact. `vc4_crtc.c` and `vc4_kms.c` numbers were counted by hand by the fetching agent from excerpts (quoted text exact, line numbers possibly off by a few); they are marked `~`.
- Names: HVS5 = `VC4_GEN_5` = BCM2711. HVS6 = `VC4_GEN_6_C` (BCM2712 C0) and `VC4_GEN_6_D` (BCM2712 D0). Gen 4 (BCM2835-7) is shown only where it explains the shared code.
- Tags: **read from source** rows carry `file:line` at the pin. **inferred** rows are derived from those rows or rest on DRM core behaviour that was not read. **observed 2026-11-28** rows are from `docs/observed/bcm2711-cm4-topology.md`, `docs/observed/bcm2711-cm4.md`, `docs/observed/bcm2712-topology.md` and `docs/observed/bcm2712.md`.
- Which kernel a board runs is not known from this file. All statements are about the pin.

## 1. Property creation (`vc4_plane_init`)

Every plane of every generation goes through one function; no property depends on `vc4->gen`. Only the format list differs (`hvs6_only` formats are dropped below gen 6).

| Property | Created as | Basis | Reference |
| --- | --- | --- | --- |
| alpha | `drm_plane_create_alpha_property`: 0..0xffff, default opaque (DRM core default) | read from source (call), inferred (range and default are core behaviour) | `vc4_plane.c:2891` |
| pixel blend mode | `drm_plane_create_blend_mode_property` with `PIXEL_NONE`, `PREMULTI`, `COVERAGE`. Default is the core default, pre-multiplied | read from source (mask), inferred (default) | `vc4_plane.c:2892-2895` |
| rotation | `drm_plane_create_rotation_property`, default `ROTATE_0`, supported `ROTATE_0 \| ROTATE_180 \| REFLECT_X \| REFLECT_Y`. No 90 / 270 | read from source | `vc4_plane.c:2896-2900` |
| COLOR_ENCODING, COLOR_RANGE | `drm_plane_create_color_properties`: encodings BT.601, BT.709, BT.2020; ranges limited, full; default BT.709 limited | read from source | `vc4_plane.c:2902-2909` |
| SCALING_FILTER | default, nearest neighbour | read from source | `vc4_plane.c:2911-2913` |
| CHROMA_SITING_H / _V | `drm_plane_create_chroma_siting_properties(plane, 0, 0x8000)` | read from source | `vc4_plane.c:2916` |
| IN_FORMATS | formats from `hvs_formats` (`hvs6_only` skipped on gen < 6_C), modifiers `T_TILED`, `SAND128`, `SAND64`, `SAND256`, `LINEAR` (each SAND with column height 0) | read from source | `vc4_plane.c:2858-2881`, `vc4_plane.c:2867-2875` |
| zpos, primary | `drm_plane_create_zpos_immutable_property(plane, 0)` when `type == PRIMARY` | read from source | `vc4_plane.c:2918-2919` |
| Colour-management properties | none on planes (no `drm_plane_create_color_pipeline_property`, no per-plane LUT / CTM) | inferred (absence in the file) | `vc4_plane.c:2848-2922` |

## 2. zpos

| Item | Gen 5 (BCM2711) | Gen 6 (BCM2712) | Basis | Reference |
| --- | --- | --- | --- | --- |
| Primary plane | immutable 0, range 0..0 | same | read from source; observed 2026-11-28 | `vc4_plane.c:2918-2919`; `docs/observed/bcm2712-topology.md` (plane table) |
| Overlay planes (shared pool) | 16 planes, mutable, range 1..17, initial value `i + 1` | same | read from source; observed 2026-11-28 | `vc4_plane.c:2924`, `vc4_plane.c:2956-2968` |
| Overlay planes (TXP / writeback CRTC) | 32 planes, mutable, range 1..33, initial `i + 1`, reach only the TXP CRTC | same code | read from source | `vc4_plane.c:2925`, `vc4_plane.c:2971-2983` |
| Cursor plane (one per CRTC, including the TXP CRTC) | mutable, range 1..17, initial 17 | same | read from source | `vc4_plane.c:2986-3000` |
| Shared overlay reach | `possible_crtcs` = all CRTCs except the one that `feeds_txp` | same | read from source | `vc4_plane.c:2935-2944`, `vc4_plane.c:2958-2959` |
| Plane count that follows | 6 primaries + 16 + 32 + 6 cursors = 60 | 4 + 16 + 32 + 4 = 56 | inferred (arithmetic on the rows above) | rows above |
| Observed plane count | 60, zpos values 0 / 1..16 / 1..32 / 17 | 56; primaries 0..0, shared overlays 1..17 mutable | observed 2026-11-28 | `docs/observed/bcm2711-cm4.md` (kernel plane table), `docs/observed/bcm2712-topology.md` (plane layout) |
| Who calls the plane factory | `vc4_crtc_init` creates the primary with `vc4_plane_init(drm, DRM_PLANE_TYPE_PRIMARY, 0)`; the call to `vc4_plane_create_additional_planes` is **not** in `vc4_crtc.c` and its call site was not read | read from source | `vc4_crtc.c:~1482` |
| Normalisation | `mode_config.normalize_zpos = true`, so the core assigns each plane a unique `normalized_zpos` per CRTC (0..n-1, ties broken by plane id). The 17 / 33 range is therefore a bound on the property, not on how many planes a CRTC can show | read from source (flag), inferred (core algorithm not read) | `vc4_kms.c:~1216` |
| Where it is applied | `vc4_hvs_atomic_flush` writes dlists by increasing `normalized_zpos`: it loops `zpos = 0, 1, ...` and, for each CRTC plane with that normalized value, appends its dlist words, until a pass finds none. Lowest value is first in the dlist, so zpos is the blend order, bottom first | read from source | `vc4_hvs.c:1299`, `vc4_hvs.c:1321-1349` |
| Zpos change forces a dlist rewrite | `crtc_state->zpos_changed = true` in `vc4_crtc_atomic_check` | read from source | `vc4_crtc.c:~798` |
| Async update copies zpos | `vc4_plane_atomic_async_update` copies `zpos` and `normalized_zpos` into the live state | read from source | `vc4_plane.c:2616-2617` |
| Cursor as top | Comment: "zpos fallback is that planes are rendered by plane ID order, and that then puts the cursor on top", so the cursor planes are created last | read from source | `vc4_plane.c:2986-2990` |

Consequences for the profile (inferred): the primary cannot be moved, so a layer below the primary is impossible and a harness that swaps z needs two overlays (`SWAPZ skipped` in the observed runs). Zpos behaves the same on HVS5 and HVS6; `Zpos::Range { 1, 17 }` for shared overlays and `Fixed(0)` for primaries match both source and the recorded probes.

## 3. Background fill and the first plane

| Item | Gen 5 | Gen 6 C0 | Gen 6 D0 | Basis | Reference |
| --- | --- | --- | --- | --- | --- |
| CRTC background colour property | `drm_crtc_attach_background_color_property(crtc)` on every CRTC (a CRTC property, not a plane property) | same | same | read from source | `vc4_crtc.c:~1444` |
| Colour written | 8-bit R, G, B of `crtc->state->background_color` into `SCALER_DISPBKGNDX`, with the FILL bit always set ("run with a lower core clock when background fill is enabled on gen 5 so leave it enabled always") | 8-bit R, G, B, A into `SCALER6_DISPX_BGND` | 12-bit G, B into `SCALER6D_DISPX_BGND0`, 12-bit R, A into `SCALER6D_DISPX_BGND1` | read from source | `vc4_hvs.c:1386-1398` (gen 5), `vc4_hvs.c:1359-1371` (6_C), `vc4_hvs.c:1372-1383` (6_D) |
| Fill enable | always on | `hvs->bg_fill[channel] = enable_bg_fill` | same | read from source | `vc4_hvs.c:1384` |
| `enable_bg_fill` rule | not used (fill always on) | taken from the **first plane in dlist order**: its `needs_bg_fill` | same | read from source | `vc4_hvs.c:1331-1341` |
| `needs_bg_fill` per plane | `fb->format->has_alpha \|\| !covers_screen \|\| alpha != opaque`, where `covers_screen` = plane at 0,0 with `crtc_w x crtc_h` equal to the mode's `hdisplay x vdisplay` | same, in `vc6_plane_mode_set` | same | read from source | `vc4_plane.c:1953-1961` (gen 5), `vc4_plane.c:2417-2427` (gen 6) |
| Rationale in source | "it suffices to consider the first active plane since we set `needs_bg_fill` such that either the first plane already needs it or all planes on top blend from the first or a lower plane" | same | same | read from source | `vc4_hvs.c:1331-1339` |

Inferred: a full-screen opaque XRGB primary lets HVS6 skip the background fill, which saves bandwidth; an ARGB or partial primary turns it on. Nothing in the engine's plane tables depends on this; the cost shows in the load computation (`docs/vc4-hvs-load-source.md`). The generic `background_color` CRTC property is out of scope for plane assignment.

## 4. Alpha and blend

### 4.1 What userspace may set

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| Plane alpha | 16-bit `alpha`; the hardware field gets `alpha >> 4` on gen 5 and gen 6 (12 bits) and `alpha >> 8` on gen 4 (8 bits) | read from source | `vc4_plane.c:1811-1812` (gen 5), `vc4_plane.c:2279` (gen 6), `vc4_plane.c:1757` (gen 4) |
| Blend modes | `PIXEL_NONE`, `PREMULTI`, `COVERAGE` on every plane, every generation | read from source | `vc4_plane.c:2892-2895` |
| Observed | `alpha` 0..65535 default 0xffff on every plane; blend None=2, Pre-multiplied=0, Coverage=1, default Pre-multiplied | observed 2026-11-28 | `docs/observed/bcm2712-topology.md` (property table); `docs/observed/bcm2711-cm4.md` (probe, "plane alpha true blend [none,premultiplied,coverage]") |
| Alpha without per-pixel alpha | A format without an alpha channel (XRGB, YUV) ignores `pixel blend mode` and uses the fixed `alpha` value only | read from source | `vc4_plane.c:1343-1345` (gen 4), `vc4_plane.c:1374-1376` (gen 5 / 6 C0), `vc4_plane.c:1393-1395` (6 D0) |
| Core-level refusals | None in `vc4_plane.c`: no plane checks `alpha` against `blend_mode` or format. | read from source (absence in file) | `vc4_plane.c:2452-2494` |

### 4.2 Hardware encoding per generation

| Case | Gen 5 (BCM2711) | Gen 6 C0 (BCM2712) | Gen 6 D0 (BCM2712) | Basis | Reference |
| --- | --- | --- | --- | --- | --- |
| Format has no alpha, or `PIXEL_NONE` | `ALPHA_MODE_FIXED` | `ALPHA_MODE_FIXED` | no CTL2 bits (0); `CTL0` alpha mask = `ALPHA_MASK_FIXED` | read from source | `vc4_plane.c:1374-1381`, `vc4_plane.c:1391-1395`, `vc4_plane.c:1406-1410` |
| `PREMULTI` with alpha | `ALPHA_MODE_PIPELINE` plus `ALPHA_PREMULT` | same | `CTL2` `ALPHA_PREMULT`; mask none | read from source | `vc4_plane.c:1383-1386`, `vc4_plane.c:1393-1395`, `vc4_plane.c:1412` |
| `COVERAGE` with alpha | `ALPHA_MODE_PIPELINE` (no premult bit) | same | no `ALPHA_PREMULT`, mask none | read from source | `vc4_plane.c:1387-1389`, `vc4_plane.c:1393-1395` |
| `ALPHA_MIX` (multiply plane alpha into pixel alpha) | `alpha != opaque && has_alpha` (no blend-mode term) | `alpha != opaque && has_alpha && blend_mode != PIXEL_NONE` | same as 6 C0 | read from source | `vc4_plane.c:1737-1738` (gen 5), `vc4_plane.c:2252-2254` (gen 6) |
| Plane alpha with `PIXEL_NONE` on an alpha format | Mode is FIXED, but `ALPHA_MIX` is still set when `alpha != opaque` (the gen 5 expression has no blend-mode term) | `ALPHA_MIX` is not set: the gen 6 expression tests `blend_mode != PIXEL_NONE` | same as 6 C0 | inferred (rows read together; the hardware effect of the mix under FIXED mode was not read) | `vc4_plane.c:1374-1381`, `vc4_plane.c:1737-1738`, `vc4_plane.c:2252-2254` |
| `default:` of the blend switch on gen 5 / 6 C0 | falls into `PREMULTI` handling | same | n/a | read from source | `vc4_plane.c:1382-1383` |
| Cost of alpha in LBM sizing | `vc4_lbm_components` counts 4 LBM components instead of 3 for a non-YUV plane whose format has alpha and whose plane alpha is opaque (YUV: 1 or 2; TPZ vertical scaling: 4) | same | same | read from source | `vc4_plane.c:866-886` |

Comparison with `docs/hvs-backend-design.md` §3 and `docs/observed/`: the design's "plane alpha, blend modes none / pre-multiplied / coverage, identical on every plane" matches the source (§4.1) and the probes on both SoCs. The design's "stepping C0 vs D0 differ in alpha-mask and fixed-alpha encoding, same toward userspace" is confirmed: the only differences are the CTL0/CTL2 bits in the table above, none is visible in properties. — read from source

## 5. Rotation and reflection

### 5.1 Property and where it is read

| Item | Gen 5 | Gen 6 | Basis | Reference |
| --- | --- | --- | --- | --- |
| Supported mask | `ROTATE_0 \| ROTATE_180 \| REFLECT_X \| REFLECT_Y` | same | read from source | `vc4_plane.c:2896-2900` |
| Observed mask | kernel enum rotate-0, rotate-180, reflect-x, reflect-y, idle value rotate-0; composition-drm reported `transforms 0x0` because it does not read the property | same on BCM2712 | observed 2026-11-28 | `docs/observed/bcm2712-topology.md`; `docs/observed/bcm2711-cm4.md` (probe line "transforms 0x0", debugfs `rotation=1`) |
| How mode_set reads it | `drm_rotation_simplify(state->rotation, ROTATE_0 \| REFLECT_X \| REFLECT_Y)`. This rewrites a requested `ROTATE_180` as `REFLECT_X \| REFLECT_Y` (the simplify helper lives in DRM core, not read) | same | read from source (call), inferred (helper semantics) | `vc4_plane.c:1527-1530` (gen 5), `vc4_plane.c:2106-2109` (gen 6) |
| ROTATE_90 / 270 | Not creatable: not in the supported mask, so the core rejects the property value before the driver sees it | same | read from source (mask), inferred (core rejection) | `vc4_plane.c:2896-2900` |
| Driver-side refusal | There is no `-EINVAL` on rotation in `vc4_plane_mode_set`, `vc6_plane_mode_set` or `vc4_plane_atomic_check`. No modifier or format combination is rejected because of rotation | read from source (absence) | `vc4_plane.c:1467-1790` (mode_set), `vc4_plane.c:2028-2440` (vc6), `vc4_plane.c:2452-2494` |
| Cursor and primary | The same rotation mask is created for primary, overlay and cursor planes | read from source | `vc4_plane.c:2896-2900` |

### 5.2 How flips are programmed, per modifier

`src_y` is moved to the last line when `REFLECT_Y` is set, before the modifier switch, so the base-pointer arithmetic of each modifier starts from the bottom row.

| Modifier | Gen 5 | Gen 6 | Basis | Reference |
| --- | --- | --- | --- | --- |
| Common | `src_y += height - 1` if `REFLECT_Y` | same | read from source | `vc4_plane.c:1532-1534` (gen 5), `vc4_plane.c:2111-2113` (gen 6) |
| Linear | `VFLIP` bit in dlist POS0 word (`SCALER5_POS0_VFLIP`), `HFLIP` in POS0 (`SCALER5_POS0_HFLIP`) | `HFLIP` in POS0 (`SCALER6_POS0_HFLIP`), `VFLIP` in pointer word 0 (`SCALER6_PTR0_VFLIP`) | read from source | `vc4_plane.c:1799-1804` (gen 5), `vc4_plane.c:2271` (gen 6 HFLIP), `vc4_plane.c:2314` (gen 6 VFLIP) |
| T-tiled | Extra tile handling: with `REFLECT_Y`, `y_off = tile_h_mask - y_off` and `SCALER_PITCH0_TILE_LINE_DIR` is set so the lines after the first are fetched in descending order. The comment admits "don't know why we need `y_off = max_y_off - y_off`, but it's definitely required" | **Not supported at all** (see §5.3) | read from source | `vc4_plane.c:1554`, `vc4_plane.c:1586-1597` |
| SAND64 / SAND128 / SAND256 | The SAND branch builds pitch words from the (already shifted) `src_y`, with no flip-specific word; the generic POS0 VFLIP / HFLIP bits still apply. No code treats flipped SAND specially | gen 6 SAND128 / SAND256: same, generic POS0 / PTR0 flip bits only, nothing SAND-specific | read from source | `vc4_plane.c:1624-1716` (gen 5), `vc4_plane.c:2132-2230` (gen 6) |
| Whether flipped SAND works in hardware | Unknown. The source neither forbids nor handles it. — inferred (needs a board `TEST_ONLY` and a picture) | same | inferred | rows above |

### 5.3 Which formats or modifiers forbid what

There is no rotation-specific rule. The only restrictions are the modifier / format ones from `vc4_format_mod_supported`, which apply regardless of rotation:

| Rule | Gen 5 | Gen 6 | Basis | Reference |
| --- | --- | --- | --- | --- |
| T-tiled and SAND64 | accepted (RGB formats for T-tiled; NV12, NV21 for SAND64) | `vc4_format_mod_supported` returns false for `T_TILED` and `SAND64` on gen >= 6_C; `vc6_plane_mode_set` handles only linear, SAND128 and SAND256 and returns `-EINVAL` ("Unsupported FB tiling flag") for the rest | read from source | `vc4_plane.c:2767-2771`, `vc4_plane.c:2233-2239` |
| T-tiled formats | XRGB8888, ARGB8888, ABGR8888, XBGR8888, RGB565, BGR565, ARGB1555, XRGB1555 (linear or T-tiled) | n/a | read from source | `vc4_plane.c:2773-2789` |
| NV12 / NV21 | linear, SAND64, SAND128, SAND256 | linear, SAND128, SAND256 (SAND64 filtered above) | read from source | `vc4_plane.c:2790-2800` |
| P030 | SAND128 only | SAND128 only | read from source | `vc4_plane.c:2801-2807` |
| Everything else (10-bit RGB, 4444, RGB332, YUV422 / 420 3-plane, NV16 / NV61, P010 / P012 / P016, S010 / S012 / S016) | linear only | linear only | read from source | `vc4_plane.c:2808-2836` |
| Raster-scan note | Gen 6: "TODO: This only covers Raster Scan Order planes" next to the pointer words | n/a | read from source | `vc4_plane.c:2300-2302` |

Comparison with the recorded IN_FORMATS (`docs/observed/bcm2712-topology.md`, "IN_FORMATS", plane 83 and four others): the BCM2712 blob lists linear for all 44 formats and the four non-linear modifiers for their subsets. Whether the blob contains `T_TILED` or `SAND64` on BCM2712, which `vc4_format_mod_supported` forbids on gen 6, is a question for the format-acceptance item (`vc4-formats-source`), not for this file.

Consequence for the profile (inferred): the profile may advertise `transforms = ROTATE_0 | ROTATE_180 | REFLECT_X | REFLECT_Y` on every plane of both generations, with no per-modifier mask. A flipped T-tiled or SAND buffer on gen 5, and a flipped SAND buffer on gen 6, are not refused by the driver; whether the picture is right is **unmeasured**, so a conservative profile would refuse reflection on SAND until a board `TEST_ONLY` plus picture says otherwise.

## 6. Colour encoding, range and chroma siting

| Item | Gen 5 | Gen 6 C0 | Gen 6 D0 | Basis | Reference |
| --- | --- | --- | --- | --- | --- |
| Properties | `COLOR_ENCODING` BT.601 / BT.709 / BT.2020, default BT.709; `COLOR_RANGE` limited / full, default limited | same | same | read from source; observed 2026-11-28 on BCM2712 | `vc4_plane.c:2902-2909`; `docs/observed/bcm2712-topology.md` |
| When used | Only when the format is YUV (`vc4_state->is_yuv`); RGB formats ignore both | same | same | read from source | `vc4_plane.c:1876-1888` (gen 5), `vc4_plane.c:1975-2008` (gen 6) |
| Programming | Three 32-bit words copied from `colorspace_coeffs[range][encoding]` into the dlist | CSC-enable plus a preloaded matrix index `encoding + 3 * range` in `BRCM_CFC_CONTROL` (0 = 601 limited ... 5 = 2020 full) | same index, `SCALER6D_CTL2_*` bits | read from source | `vc4_plane.c:1296-1330` (table), `vc4_plane.c:1886-1893` (gen 5 words), `vc4_plane.c:1998-2005` (gen 6) |
| Out-of-range value | Falls back to BT.601 / limited (gen 5: `>= MAX`; gen 6: `> BT2020` or `> FULL`) | same | same | read from source | `vc4_plane.c:1881-1884`, `vc4_plane.c:1993-1996` |
| Chroma siting | Property exists, range 0..0x8000 (H cosited, V interstitial in the driver's comment) | same | same | read from source | `vc4_plane.c:2915-2916` |
| Observed | `COLOR_ENCODING`, `COLOR_RANGE`, `SCALING_FILTER`, `CHROMA_SITING_H/V` (0..65536) on the five BCM2712 planes read | | | observed 2026-11-28 | `docs/observed/bcm2712-topology.md` (property table) |

The `Capabilities` model has no field for colour encoding or range, so the profile table carries nothing for these; they only matter when a layer is YUV. — inferred

## 7. Comparison with the recorded data and the design

| Statement in `docs/hvs-backend-design.md` | Source | Result |
| --- | --- | --- |
| §3: primary immutable zpos 0 | `vc4_plane.c:2918-2919` | agrees (read from source; observed 2026-11-28) |
| §3: overlay zpos 1..17 mutable, initial `i + 1` | `vc4_plane.c:2956-2968` | agrees |
| §3: cursor zpos range 1..17, initial 17 | `vc4_plane.c:2996-2999` | agrees. The design says the BCM2712 probe did not type the extra planes (646, 657, 668); the source predicts them as cursor planes (`vc4_plane.c:2991-3000`), but a board read of `type` is still outstanding |
| §3: alpha, blend modes identical on every plane | `vc4_plane.c:2891-2895` | agrees |
| §3: rotation 0 / 180 / reflect X / Y, no 90 / 270 | `vc4_plane.c:2896-2900` | agrees. The design's phrase "the mask must exclude 90/270" is already satisfied by the driver |
| §5: stepping C0 vs D0 "same toward userspace" | `vc4_plane.c:1362-1412`, `vc4_hvs.c:1359-1383` | agrees: the differences are register encodings only |
| Recorded plane counts 60 (BCM2711) and 56 (BCM2712) | `vc4_plane.c:2924-3000`, `vc4_crtc.c:~1482` | agree with 6 + 16 + 32 + 6 and 4 + 16 + 32 + 4 (inferred) |
| `docs/observed/bcm2712-topology.md` "CRTC-0-only planes: 34" plus 3 extra plus 4 primaries plus 16 shared = 57, but the kernel table says 56 | `vc4_plane.c:2971-3000` | The source gives 32 TXP overlays on the TXP CRTC and one cursor on each of 4 CRTCs. Reading CRTC 0 as the TXP CRTC, the CRTC-0-only group is 32 + 1 cursor = 33, not 34, which makes 56. The recorded "34" is one too many. — inferred; a board re-count is not part of this item |
| Plane reach: shared overlays on CRTCs 1, 2, 3 and not CRTC 0 | `vc4_plane.c:2935-2944` | agrees if CRTC 0 is the `feeds_txp` CRTC (inferred; which CRTC feeds the TXP was not read) |

## 8. What the profile can take from this

| Field | Value | Basis |
| --- | --- | --- |
| zpos, primary | `Fixed(0)` | read from source; observed 2026-11-28 |
| zpos, shared overlay | `Range { 1, 17 }`, mutable, both generations | read from source; observed 2026-11-28 |
| plane alpha | yes, every plane | read from source; observed 2026-11-28 |
| blend modes | none, pre-multiplied, coverage, every plane | read from source; observed 2026-11-28 |
| transforms | `ROTATE_0 \| ROTATE_180 \| REFLECT_X \| REFLECT_Y`, every plane, both generations | read from source |
| transform on SAND / T-tiled | not refused by the driver; correctness unmeasured; refuse in a conservative profile | inferred |
| T-tiled, SAND64 on HVS6 | refused by `vc4_format_mod_supported` and `vc6_plane_mode_set` | read from source |
| colour encoding / range | not modelled | inferred |

## Open points (not closed by source)

- Whether flipped SAND or T-tiled buffers produce a correct picture (needs `TEST_ONLY` and a camera on a board).
- The DRM core pieces assumed in this file: default alpha and blend values, `drm_rotation_simplify` semantics, how property values outside the created mask are rejected, `drm_atomic_normalize_zpos` ordering. They are standard but were not read here.
- Where `vc4_plane_create_additional_planes` is called and which CRTC feeds the TXP.
- The exact line numbers marked `~` for `vc4_crtc.c` and `vc4_kms.c`.
