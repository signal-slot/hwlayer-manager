# VC4 HVS5 / HVS6: LBM and UPM pools, per-plane cost and plane counts, read from rpi-6.18.y

Dated 2026-11-28. Closes the "LBM/UPM resource groups" rows of `docs/hvs-backend-design.md` §4 with exact pool sizes, the per-plane size formulas, which planes need the pools, the failure errno and how many 1920-wide layers fit. Nothing here was run on a board.

## Provenance

- Tree: `raspberrypi/linux`, branch `rpi-6.18.y`, commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e`, directory `drivers/gpu/drm/vc4/`.
  Link: <https://github.com/raspberrypi/linux/tree/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4>
- Fetch route: no usable copy was in `$CONNECTED_DEPS_DIR` (the `rpi-linux` directory there was empty), a `git` clone was refused on this node, so the web-fetch subagent read the raw files at the SHA: `vc4_plane.c`, `vc4_hvs.c`, `vc4_drv.h`, `vc4_regs.h`. The text of the functions quoted below came back verbatim.
- Line numbers: the fetch tool returns no line numbers. For `vc4_plane.c` they were counted by hand from line 1 and cross-checked against the line numbers of the sibling doc `docs/vc4-hvs-scaling-source.md`, which pins the same commit; they can be off by one or two lines. For `vc4_hvs.c`, `vc4_drv.h`, `vc4_regs.h` the numbers are taken from that sibling doc (`vc4_hvs.c:1693-1711`, `1717-1730`; `vc4_drv.h:304`, `317`, `397`; `vc4_regs.h:1421-1427`), not recounted here. Function names are given everywhere so a citation can be found by name.
- Names: gen 4 = `VC4_GEN_4` = BCM2835 family; gen 5 = `VC4_GEN_5` = BCM2711 = HVS5; gen 6 = `VC4_GEN_6_C` / `VC4_GEN_6_D` = BCM2712 = HVS6.
- Tags: **read from source** rows cite the pin. **inferred** rows are derived from those rows (arithmetic, or the generic kernel library, which was not read here).
- Related docs: `docs/vc4-hvs-scaling-source.md` (§2 LBM, §3 UPM cover the same functions; this doc adds the errno, the format and alpha dependence of the component count, the need matrix and the per-generation counts), `docs/vc4-hvs-load-source.md` (load and clock model; it states that `hvs_load` ignores LBM/UPM stalls). `docs/vc4-hvs-dlist-source.md` does not exist at this commit of the repository, so dlist words are not related here.

## 1. Pool sizes per generation

| Generation | LBM pool | LBM unit | LBM handles | UPM pool | UPM handles | Basis | Reference |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Gen 4 | `48 * SZ_1K` = 49152 | word of 2 × 12-bit pixels | 64 | none | none | read from source | `__vc4_hvs_alloc`, `vc4_hvs.c:1693-1711`; `VC4_NUM_LBM_HANDLES` `vc4_drv.h:304` |
| Gen 5 (BCM2711) | `60 * SZ_1K` = **61440** | word of 4 × 12-bit pixels | 64 | none | none | read from source | same |
| Gen 6 (BCM2712) | **1024** (constant; `SCALER6_LBM_SIZE` is only listed in debugfs, never read) | one "line" of 128 / `hsub` components (see 2.3) | 64 | `1024 * HVS_UBM_WORD_SIZE` = 1024 × 256 = **262144 bytes** (`SCALER6_UBM_SIZE` is not read either) | 32 | read from source | same; `HVS_UBM_WORD_SIZE` = 256 `vc4_drv.h:397`; `VC4_NUM_UPM_HANDLES` `vc4_drv.h:317` |

- Both pools are `drm_mm` ranges starting at 0, plus an IDA of handles 1..N (`drm_mm_init(&hvs->lbm_mm, 0, lbm_size)`, `ida_init(&hvs->lbm_handles)`; for gen 6 `ida_init(&hvs->upm_handles)` and `drm_mm_init(&hvs->upm_mm, 0, 1024 * HVS_UBM_WORD_SIZE)`). The UPM pool exists only for `vc4->gen >= VC4_GEN_6_C`. read from source (`__vc4_hvs_alloc`).
- Gen 6_C and 6_D use the same sizes; the pool is global to the HVS, shared by all three channels (one `struct vc4_hvs`, one `lbm_mm`). read from source (`struct vc4_hvs`, `vc4_drv.h`).
- The code comment in `__vc4_hvs_alloc` says the gen 6 size is a constant because a register read is impossible under KUnit; the real hardware size is therefore not confirmed by the driver. Board value: not measured. inferred.

## 2. LBM: who needs it and how much

### 2.1 Which planes need LBM

The gate is one test in `vc4_lbm_size` (`vc4_plane.c:942-956`): the size is **0 when both `y_scaling[0]` and `y_scaling[1]` are `VC4_SCALING_NONE`**; `vc4_plane_allocate_lbm` returns 0 on size 0 (`vc4_plane.c:1102-1104`) and the dlist slot is reserved only when a y axis scales (gen 5 `vc4_plane.c:1902-1906`, gen 6 `vc4_plane.c:2353-2357`). read from source.

| Plane | LBM? | Why | Reference |
| --- | --- | --- | --- |
| RGB, unscaled | no | both y modes NONE | `vc4_plane.c:948-950` |
| RGB, horizontal scale only | no | LBM is a vertical-scaler resource | `vc4_plane.c:809`, `948-950` |
| RGB, vertical scale (PPF or TPZ) | yes | | `vc4_plane.c:1102` |
| RGB, unscaled but fractional `src_y` | yes | a fractional start promotes `y_scaling[0]` NONE → PPF | `vc4_plane.c:620-621` |
| Any multi-plane YUV (NV12, YUV420, YUV422, YUV444) | **yes, even at 1:1** | `y_scaling[1]` NONE is forced to PPF | `vc4_plane.c:626-665` |
| YUV444 on gen 6 | yes | `y_scaling[0]` is also forced to PPF | `vc4_plane.c:2049-2072` |
| YUV444 on gen 5 | yes (via chroma) | only `x_scaling[0]` forced, chroma y forced as above | `vc4_plane.c:1489-1495` |
| Zero-size source or destination (off-screen) | no | `atomic_check` returns before the allocators | `vc4_plane.c:2481-2483` |
| Disabled plane | releases | `vc6_plane_free_upm` + `vc4_plane_free_lbm` | `vc4_plane.c:2462-2472` |

Gen 6 has an outer gate `!is_unity || fb->format->is_yuv` (`vc4_plane.c:2348`) but the allocation still depends on the y modes; the design doc's wording "scaled *or* YUV (`!is_unity || is_yuv`)" (`docs/hvs-backend-design.md` §4 line 137) is true of the dlist words and too wide for the LBM: horizontal-only RGB scaling takes no LBM. read from source; comparison inferred.

### 2.2 Gen 5 size: `__vc4_lbm_size` (`vc4_plane.c:802-849`)

Words (of 61440), with `P` = pixels per line:

- `P = crtc_w` when `x_scaling[0]` is TPZ, otherwise the source span `vc4_src_span(src_x, src_w[0])` = whole pixels covered by the 16.16 start and size (`vc4_plane.c:796-800`, `821-825`).
- RGB (not `is_yuv`): `lbm = P * 8` when `y_scaling[0]` is TPZ, else `P * 16` (the comment says "in special cases, this multiplier might be 12"). YUV: `P * 16` (`vc4_plane.c:827-840`).
- `roundup(lbm, 128)` on gen 5 (64 on gen 4), then `/ 4` on gen 5 (`/ 2` on gen 4) (`vc4_plane.c:843-846`); `vc4_plane_allocate_lbm` then aligns to 64 on gen 5 and 32 on gen 4 (`vc4_plane.c:1110-1113`).
- Format and alpha do not matter beyond RGB vs YUV. read from source.

### 2.3 Gen 6 size: `vc4_lbm_channel_size` (`vc4_plane.c:868-940`)

Units are lines (the code comment at `vc4_plane.c:1106-1109` says the gen 6 size is already in the allocator's units and is not aligned). Per channel `c` (0 = luma or the RGB plane, 1 = chroma):

1. `src_w = vc4_src_span(src_x, src_w[0])` (`vc4_plane.c:897`).
2. `width`: chroma of a subsampled format (`hsub > 1`, channel 1) → `src_w`; else TPZ on the x axis of that channel → `crtc_w`; else `min(src_w, crtc_w)` (`vc4_plane.c:905-910`). YUV444 doubles it (`vc4_plane.c:912-913`). Then `width = round_up(DIV_ROUND_UP(width, hsub), 4)` (`vc4_plane.c:914`).
3. Words per component by the channel's y mode: PPF 4, TPZ 2, NONE 0 → channel contributes 0 (`vc4_lbm_words_per_component`, `vc4_plane.c:851-866`).
4. Components (`vc4_lbm_components`, `vc4_plane.c:868-887`): y NONE → 0; YUV → 1 for channel 0, 2 for channel 1; RGB with y TPZ → 4; RGB with an alpha channel and plane alpha `DRM_BLEND_ALPHA_OPAQUE` (the default) → 4; RGB otherwise (no alpha channel, or plane alpha below opaque) → 3.
5. `words = width * wpc * components`; `lines = DIV_ROUND_UP(words, 128 / hsub)` (`vc4_plane.c:924-928`).
6. Plane size: for YUV `max(channel 0, channel 1)`, else channel 0 (`__vc6_lbm_size`, `vc4_plane.c:931-940`).

So on gen 6 the cost depends on the format (alpha or not, planar or not), the plane alpha property and the TPZ/PPF choice; on gen 5 it does not. read from source.

## 3. UPM (gen 6 only): who needs it and how much

- Every enabled gen 6 plane with a non-empty source and destination allocates UPM, **scaled or not, RGB or YUV**; there is no gate (`vc4_plane_atomic_check`, `vc4_plane.c:2489-2493`). One handle and one `drm_mm` node **per colour plane** of the framebuffer (`vc6_plane_allocate_upm`, loop over `info->num_planes`, `vc4_plane.c:1189-1266`). read from source.
- Size of one colour plane (`vc6_upm_size`, `vc4_plane.c:958-984`): `stride = fb->pitches[plane]`; for SAND128 `ALIGN(fb->width, 128)` (P030: `ALIGN(width, 96) * 4 / 3`); luma of YUV444/YVU444 doubled; `words_per_line = (stride + 62) / 32`; `fetch_region = words_per_line * 32`; `buffer_lines = 2 << upm_buffer_lines` and `upm_buffer_lines` is fixed at `SCALER6_PTR0_UPM_BUFF_SIZE_2_LINES` = 0 (`vc4_plane.c:1201`, `vc4_regs.h:1423-1427`), so 2 lines; `size = ALIGN(fetch_region * 2, 256)`. The size therefore depends on the **stride (the framebuffer pitch)**, not on the plane size or scaling. The in-source TODO says it only fits raster formats. read from source.
- The pool is bytes, the alignment 256 (`drm_mm_insert_node_generic(&hvs->upm_mm, ..., upm_size, HVS_UBM_WORD_SIZE, 0, 0)`, `vc4_plane.c:1241-1244`); the base in the dlist is `start / 256` (`vc4_plane.c:1257`). read from source.
- Handle and size reuse: if the plane already holds a handle of the same size it keeps it, otherwise it drops (refcount) and allocates anew (`vc4_plane.c:1212-1253`); same for LBM (`vc4_plane.c:1124-1165`). No two planes share an allocation. read from source.

## 4. How the pools fail

| Case | Message | Return | Reference |
| --- | --- | --- | --- |
| LBM handles exhausted (> 64) | `drm_err` "Out of lbm_handles" | the negative value of `ida_alloc_range` | `vc4_plane.c:1138-1144` |
| LBM `drm_mm_insert_node_generic` fails | `drm_err` "Failed to allocate LBM entry: %d"; refcount reset, handle freed | `ret` from `drm_mm` | `vc4_plane.c:1151-1164` |
| LBM offset slot missing | `WARN_ON` | `-EINVAL` | `vc4_plane.c:1118-1119` |
| UPM handles exhausted (> 32) | `drm_dbg` (not `drm_err`) "Out of upm_handles" | the negative value of `ida_alloc_range` | `vc4_plane.c:1227-1233` |
| UPM `drm_mm_insert_node_generic` fails | `drm_err` "Failed to allocate UPM entry: %d"; cleanup as for LBM | `ret` from `drm_mm` | `vc4_plane.c:1240-1252` |
| UPM size 0 | none | `-EINVAL` | `vc4_plane.c:1209-1211` |

- `vc4_plane_atomic_check` returns either error **unchanged**, LBM first, then UPM (`vc4_plane.c:2485-2493`), so a `DRM_MODE_ATOMIC_TEST_ONLY` commit fails (no separate "pool" check exists). If the UPM step fails after the LBM step, the LBM allocation of that plane state stays until the state is destroyed (reported by the fetch of `vc4_plane.c:418-450`, not recounted). read from source.
- The errno itself: `ida_alloc_range` returns `-ENOSPC` when the range is full and `drm_mm_insert_node_generic` returns `-ENOSPC` when no hole fits, from the generic kernel library (not in the fetched files); the ioctl then reports it to userspace. So the expected errno of an exhausted pool is **`-ENOSPC`** (`-ENOMEM` only on an allocation failure). inferred; the board sweep items should record the observed value.
- The same `-ENOSPC` is indistinguishable from the other ways to fail a TEST_ONLY, so a backend can learn only "this combination does not fit", not which pool. inferred.
- Handle exhaustion needs the sum of handles of all committed and tested planes: a plane keeps its handle across commits and TEST_ONLY states allocate from the same global pool until destroyed (the allocation runs in `atomic_check`, `vc4_plane.c:2485-2493`). inferred from the call site; DRM state lifetime not read.

## 5. How many 1920-wide layers fit (inferred)

Assumptions, all of them from the formulas above: source and destination 1920 pixels wide (so `P = min(src_w, crtc_w) = 1920`), integer source start, vertical scaling present, plane alpha at the default opaque, tightly packed pitch (1920 × 4 = 7680 bytes for ARGB; 1920 for each NV12 plane), nothing else holding the pools. A taller or wider source with a 1920-wide destination (a downscale) costs the same width (`min`); an upscale from a narrower source costs the source width.

### 5.1 LBM

| Layer | Gen 5 words | Layers in 61440 | Gen 6 lines | Layers in 1024 |
| --- | --- | --- | --- | --- |
| ARGB8888, vertical PPF | 1920 × 16 = 30720 → 7680 | **8** | 1920 × 4 × 4 = 30720 / 128 = **240** | **4** (960) |
| XRGB8888, vertical PPF | 7680 | **8** | 1920 × 4 × 3 = 23040 / 128 = **180** | **5** (900) |
| ARGB8888, vertical TPZ | 1920 × 8 = 15360 → 3840 | **16** | 1920 × 2 × 4 = 15360 / 128 = **120** | **8** (960) |
| ARGB8888, plane alpha below opaque, vertical PPF | 7680 | 8 | comps 3: **180** | 5 |
| NV12, 1:1 | 1920 × 16 → 7680 | **8** | ch 0: 0; ch 1: width 960, wpc 4, comps 2 = 7680 / 64 = **120** | **8** (960) |
| NV12, vertical PPF | 7680 | **8** | ch 0: 960 × 4 × 1 = 3840 / 64 = 60; ch 1: 120; max **120** | **8** |
| NV12, vertical TPZ both channels | 7680 (gen 5 does not distinguish for YUV) | 8 | ch 1: 960 × 2 × 2 = 3840 / 64 = **60** | **17** (1020) |

- The handle limit (64) is not the binding limit in any row; the words or lines are. The 17-layer row is the only one above 16, and it needs a vertical TPZ (a downscale beyond 1.5×, `vc4_plane.c:329-342`, see the sibling doc); NV12 upscales use PPF.
- A 1920-wide upscaled or unscaled-but-YUV layer is the common case in a backend; take **8** (gen 5) and **4 for ARGB / 5 for XRGB / 8 for NV12** (gen 6) as the planning figures.
- Gen 6 cannot hold a single full-width 8191 PPF ARGB layer with room to spare: 8192 × 4 × 4 / 128 = 1024 lines, the whole pool (inferred from the formula).
- Gen 4 (not a target, for completeness): 1920 ARGB PPF = 30720 / 2 = 15360 words of 49152 → 3 layers.

### 5.2 UPM (gen 6)

| Layer | Per colour plane | Per layer | By bytes (262144) | By handles (32) | Fits |
| --- | --- | --- | --- | --- | --- |
| ARGB8888, pitch 7680 | (7680 + 62) / 32 = 241 → 7712 × 2 = 15424 → ALIGN 256 = 15616 | 15616 (1 handle) | 16 (249856; a 17th is 265472) | 32 | **16** |
| NV12, both pitches 1920 | (1920 + 62) / 32 = 61 → 1952 × 2 = 3904 → ALIGN 256 = 4096 | 8192 (2 handles) | 32 | 16 | **16** |
| XRGB8888 3840 wide, pitch 15360 | 481 → 15392 × 2 = 30784 → 30976 | 30976 | 8 | 32 | **8** |

A padded pitch (alignment of the allocator) raises the figure per layer slightly and can lower these counts. Unscaled ARGB layers take UPM too, so the UPM bound applies to every layer, the LBM bound only to scaled and YUV layers.

### 5.3 Combined (gen 6, mixed use)

The LBM and UPM pools are independent; a layer must fit both. For 1920-wide layers:

| Mix | LBM limit | UPM limit | Binding |
| --- | --- | --- | --- |
| Scaled ARGB (PPF) only | 4 | 16 | LBM, 4 |
| Scaled XRGB (PPF) only | 5 | 16 | LBM, 5 |
| NV12 only | 8 | 16 | LBM, 8 |
| Unscaled ARGB only | no LBM | 16 | UPM, 16 |
| 1 scaled ARGB (240) + 3 NV12 (360) + unscaled ARGB | 600 of 1024 | 15616 + 3 × 8192 + 15616 = 55808 of 262144 | none |

inferred; gen 5 has no UPM, so only the first LBM column applies there (8 PPF layers of any format, 16 TPZ RGB layers).

## 6. Relation to `docs/hvs-backend-design.md` §4 (read-only)

- Row "LBM, HVS5: 64 handles, 60 K words": the handle count is not binding for 1920-wide layers (the words are, at 8 PPF layers); a unit cost per layer is a safe lower bound only if sized by the width: a `SCALED_V` group with capacity 8 is sound for full-width layers but refuses cases that fit (narrow or TPZ layers). The authoritative check stays `TEST_ONLY`. inferred.
- Row "LBM and UPM, HVS6": the design's note that an NV12 layer needs two UPM handles while a group charges one unit per layer is right (`vc6_plane_allocate_upm` loops over `num_planes`, `vc4_plane.c:1203`); with the byte pool binding only for wide ARGB, a UPM group of 16 layers is sound for NV12 and for 1920-wide ARGB, and the LBM group for HVS6 must be weighted (ARGB 240, XRGB 180, NV12 120 per 1920 width), not a flat per-layer unit. A flat capacity of 4 (ARGB worst case) is sound but wasteful for NV12 (8 fit). inferred.
- Row 137 (HVS6 LBM "scaled or YUV"): see 2.1, horizontal-only RGB scaling takes no LBM.
- `docs/vc4-hvs-load-source.md` says `hvs_load` ignores LBM/UPM stalls: these pools are a hard resource (a failed `atomic_check`), not a load figure; they are the only limits on gen 6 that `TEST_ONLY` reports reliably, whereas bandwidth is not testable.

## 7. Open points

- Board values: the gen 6 LBM pool is a driver constant, and whether the hardware really has 1024 lines was not measured. The errno of an exhausted pool (`-ENOSPC` expected) was not observed. The board sweep items can stack 5 scaled 1920-wide ARGB layers on BCM2712 (expected failure) and 9 scaled 1920-wide layers on BCM2711 (expected failure) with `TEST_ONLY` and record the errno; those would turn the figures of §5 from inferred into observed.
- The copy of the source for this doc was not available as a local checkout; line numbers of `vc4_plane.c` should be confirmed with `sed -n` against a checkout when one is available.
