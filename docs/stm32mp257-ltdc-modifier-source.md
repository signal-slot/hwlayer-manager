# STM32MP257 LTDC: framebuffer modifiers, IN_FORMATS and dma-buf import, read from source

Date of reading: 2026-11-29. **Nothing here is a board measurement and no board was accessed.** Every row is tagged **read from source** (stated by the code I read) or **inferred** (a conclusion, or DRM core behaviour I did not read in this session). The galcore (Vivante) side is **not read**: it is out of tree.

## What was read

| Item | Value | Tag |
| --- | --- | --- |
| Tree | `STMicroelectronics/linux`, branch `v6.6-stm32mp`, commit `548f960c059bc4b9165cb69895fd67551f6061ca`, the commit the other STM32MP257 source notes pin (`stm32mp257-ltdc-source-full.md`) | read from source |
| Files | `drivers/gpu/drm/stm/ltdc.c` (2943 lines), `drivers/gpu/drm/stm/drv.c` (317 lines) | read from source |
| How | Local copies of both files in the project's dependency cache (saved as `st66-ltdc.c` and `st66-drv.c`). The 2943-line count matches the `ltdc.c` count in `stm32mp257-ltdc-yuv-source.md`. Both were then re-fetched from `raw.githubusercontent.com/STMicroelectronics/linux/548f960c059bc4b9165cb69895fd67551f6061ca/drivers/gpu/drm/stm/` (by SHA) and are byte-identical to the copies, so the numbers are exact at that commit. Line numbers below are exact on those copies and use the upstream names `ltdc.c` and `drv.c`. | read from source |
| Not read | DRM core and the GEM DMA helpers (`drm_gem_dma_helper.c`, `drm_gem_framebuffer_helper.c`, `drm_prime.c`, `drm_framebuffer.c`, `drm_plane.c`): no kernel tree was available and nothing could be fetched. Their behaviour below is written from my knowledge of mainline and tagged **inferred**. Re-check against the 6.6.129 tree before relying on a row. | not read |
| Not read | galcore (Vivante GPU kernel driver, out of tree), the Vivante user-space GL/EGL stack, and any ST gbm/EGL glue | not read |

## 1. Modifiers the planes advertise

| Claim | Detail | Tag | Where |
| --- | --- | --- | --- |
| One modifier table for all planes | `static const u64 ltdc_format_modifiers[] = { DRM_FORMAT_MOD_LINEAR, DRM_FORMAT_MOD_INVALID };` (INVALID is only the terminator) | read from source | `ltdc.c` 517-520 |
| Every plane uses it | `ltdc_plane_create` sets `modifiers = ltdc_format_modifiers` and passes it to `drmm_universal_plane_alloc`, for primary and overlays alike, with no per-plane or per-format variation | read from source | `ltdc.c` 2156, 2210-2211 |
| No `format_mod_supported` callback | `ltdc_plane_funcs` has only `update_plane`, `disable_plane`, `reset`, the duplicate/destroy state helpers and `atomic_print_state`. There is no `.format_mod_supported`. | read from source | `ltdc.c` 2129-2136 |
| So the only modifier is LINEAR | With no callback, the core checks a (format, modifier) pair against the `modifiers` array given at plane creation, so only LINEAR passes. | inferred (core not read) | n/a |
| `IN_FORMATS` is the cross product | The blob the core builds from the format list and the one-entry modifier list: every format of the plane with LINEAR (0) only. Userspace sees no other modifier, and no vendor modifier (no Vivante tiling, no AFBC). | inferred (core not read) | n/a |
| Formats differ per plane, modifiers do not | RGB formats come from `caps.pix_fmt_drm`; on parts with `non_alpha_only_l1` the XR24/RX24-like formats are dropped on non-primary planes; YCbCr packed formats are added when the layer reports `YIA`; semi-planar and planar only when `YSPA`/`YFPA` and not layer 0 on MP25 | read from source | `ltdc.c` 2169-2203 (RGB, 2169-2180; YCbCr 2183-2203) |
| `fb_create` is the generic GEM one | `.fb_create = drm_gem_fb_create`: no driver check of the modifier at framebuffer creation | read from source | `drv.c` 36-40 |
| A non-LINEAR modifier is refused at AddFB2 | The core's framebuffer creation checks the modifier against the plane format lists before calling `fb_create`, and fails with `-EINVAL`. | inferred (core not read) | n/a |
| The atomic check does not look at the modifier | `ltdc_plane_atomic_check` tests only the YUV flag and the source/destination sizes (scaling rules). `ltdc_plane_update` reads pitch and address and never `fb->modifier`. | read from source | `ltdc.c` 1566-1604 (check); update 1845-1938 (address, pitch) |

Conclusion for modifiers: **only LINEAR is taken**, by construction of the plane tables. This is a statement about the driver. Whether the 6.6.129 board image has patches that change it is inferred as unlikely (same as the other notes on this tree).

## 2. Memory model: drm_gem_dma, contiguous only

| Claim | Detail | Tag | Where |
| --- | --- | --- | --- |
| The driver uses the GEM DMA helpers | `drv.c` includes `drm/drm_gem_dma_helper.h`; the driver struct uses `DRM_GEM_DMA_DRIVER_OPS_WITH_DUMB_CREATE(stm_gem_dma_dumb_create)` and `DEFINE_DRM_GEM_DMA_FOPS` | read from source | `drv.c` 24, 61-73 (ops 73) |
| `fb_create` is `drm_gem_fb_create`, so a framebuffer's GEM objects are expected to be `drm_gem_dma` objects | `drm_gem_fb_create` only wraps GEM handles; it does not copy or remap | read from source (assignment) / inferred (what the helper does) | `drv.c` 37 |
| The scan-out address is a single physical/DMA address | `paddr = (u32)drm_fb_dma_get_gem_addr(fb, newstate, 0)`, and the same for planes 1 and 2 of YUV formats. The layer registers take one base address per plane and a pitch; the code has no scatter-gather path. | read from source | `ltdc.c` 1845 (plane 0), 1878, 1890-1891, 1908-1909 (chroma planes); pitch 1861-1863, 1936-1938 |
| The address is truncated to 32 bits | `(u32)` cast on every address | read from source | `ltdc.c` 1845, 1878, 1890-1891, 1908-1909 |
| The device is limited to 32-bit DMA | `dma_set_coherent_mask(dev, DMA_BIT_MASK(32))` and `dma_set_max_seg_size(dev, DMA_BIT_MASK(32))`, with the comment "Configure the DMA segment size to make sure we get contiguous & coherent memory" | read from source | `drv.c` 226-241 (comment 226, mask 227, seg size 233) |
| Maximum framebuffer size | `STM_MAX_FB_WIDTH` 2048 and `STM_MAX_FB_HEIGHT` 2048, set as `mode_config.max_width/max_height`, so AddFB2 of a larger buffer fails | read from source | `drv.c` 33-34, 94-95 |
| Dumb buffers | `stm_gem_dma_dumb_create` rounds the pitch up to 128 bytes and the height to 4, then calls `drm_gem_dma_dumb_create_internal` | read from source | `drv.c` 46-60 |
| No pitch or offset restriction on imported framebuffers in `atomic_check` | The check has no pitch, alignment or offset test. Whether the IP imposes a pitch alignment is not stated here; `ltdc_plane_update` uses `fb->pitches[]` directly. | read from source | `ltdc.c` 1566-1604, 1861-1863 |

## 3. dma-buf import from another device

| Claim | Detail | Tag | Where |
| --- | --- | --- | --- |
| Import goes through the GEM DMA prime hook | `DRM_GEM_DMA_DRIVER_OPS_WITH_DUMB_CREATE` expands to the dumb-create hook plus `.gem_prime_import_sg_table = drm_gem_dma_prime_import_sg_table`. The driver sets no `gem_prime_import` of its own and no `DRIVER_` feature other than `MODESET`, `GEM`, `ATOMIC`. | read from source (no own hook, features) / inferred (macro expansion) | `drv.c` 64-73 (features 65, macro 73) |
| Import path | `PRIME_FD_TO_HANDLE` runs the default `drm_gem_prime_import`, attaches the dma-buf to the LTDC device, maps it to a `sg_table`, and hands the table to `drm_gem_dma_prime_import_sg_table` | inferred (core not read) | n/a |
| A contiguous buffer is accepted | The helper checks the mapped table is contiguous in the device's DMA address space (`drm_prime_get_contiguous_size(sgt) < dma_buf size` is the failure test) and stores `dma_addr = sg_dma_address(sgt->sgl)`, with `vaddr` unset. | inferred (core not read) | n/a |
| A non-contiguous buffer is refused | If the mapping has more than one DMA segment, the helper returns `-EINVAL` ("scatter/gather table must be contiguous" in a debug message) and PRIME_FD_TO_HANDLE fails. The driver has no fallback (no bounce copy, no per-page scanout). | inferred (core not read) | n/a |
| One case that can make a scattered buffer look contiguous | If the LTDC sat behind an IOMMU, the mapping could merge pages into one DMA range. Whether the MP25 LTDC has an IOMMU in this tree is not read in this note; the device-tree files were not read. | inferred / not read | n/a |
| Cache coherency | The coherent mask in `drv.c` 227 is for the device; whether an imported GPU buffer is CPU-cache-clean when scanned out depends on the exporter's map/sync behaviour | inferred | n/a |

## 4. What this means for buffers from the GPU

All rows below are **inferred**: they combine the LTDC rows above with the engine's model; the galcore side is **not read**.

| Statement | Tag |
| --- | --- |
| A GPU buffer is scannable by the LTDC plane only if it is LINEAR (modifier 0 or no modifier), in one of the formats that plane advertises, within 2048x2048, in memory that maps to a single DMA segment below 4 GiB. | inferred |
| Whether galcore exports render targets as contiguous: **not read**. A Vivante driver may export contiguous or CMA-backed memory (it often can), but I did not read the galcore export path, so a GPU render buffer imported into the LTDC may succeed or fail with `-EINVAL`. This must be tested on the board, not assumed. | not read |
| Whether the Vivante user space can allocate LINEAR render targets with the required pitch and format (gbm/EGL surfaces with `GBM_BO_USE_SCANOUT` or an explicit modifier list) is **not read**. If it picks a tiled or super-tiled layout, the buffer has a non-LINEAR modifier and the LTDC refuses it; the compositor must then copy or blit into a LINEAR buffer. | not read |
| The planner must not offer a non-LINEAR buffer to an LTDC plane. Since the LTDC lists only LINEAR, the (format, modifier) match in the planner will exclude such buffers if `IN_FORMATS` is parsed correctly. A tiled GPU buffer falls to GPU composition (a copy into a LINEAR scan-out buffer), not to a plane. | inferred |
| Memory that is contiguous on the exporter's device may not be contiguous on the LTDC's; a failure shows up at import, not at the atomic check. The engine should treat a `prime_fd_to_buffer` or `AddFB2` error on an LTDC output as "this buffer cannot be scanned out here" and fall back, not as a fatal error. | inferred |
| The LTDC cannot read from a buffer above 4 GiB (32-bit cast and 32-bit mask). On MP25 boards with more than 4 GiB of RAM, the buffer must sit in the low 4 GiB. | inferred |

## 5. `crates/composition-drm/src/profile.rs` (`Stm32Ltdc`) against this

I did not edit it. Compared by reading.

| Point | What the code does | Matches? | Tag |
| --- | --- | --- | --- |
| Profile touches modifiers | `DriverProfile::Stm32Ltdc` (profile.rs 189-213) sets `blend_modes` to coverage on non-cursor planes and `scaling` on the primary and the first overlay. It reads and sets no format or modifier state. | Neutral: it makes no modifier claim, so none can be wrong | read from source |
| Where modifiers come from | In `lib.rs`, `IN_FORMATS` is parsed by `parse_in_formats` (lib.rs 335-370) and fills `caps.formats`, with LINEAR pairs first (sort at lib.rs 532). If the property is missing, every plane format is paired with LINEAR (lib.rs 525-531). | Yes: this gives LINEAR-only for the LTDC, exactly what section 1 says, whether the kernel provides the blob or not | read from source (code) / inferred (that the blob has only LINEAR) |
| Import path | `import_dmabuf` (lib.rs 670-728) passes the modifier only if the device reports `AddFB2Modifiers`; for a non-LINEAR buffer without that capability it refuses (lib.rs 680-682). Otherwise it relies on the kernel to reject an unsupported modifier. | Compatible: the host does not pre-check against `IN_FORMATS`; the planner must (section 4) | read from source |
| Contiguity | No check; the kernel decides at `prime_fd_to_buffer` (lib.rs 698). An error there is returned, with the handles closed (lib.rs 720-722). | Compatible; the host cannot know it earlier. Whether the caller falls back on error is a question for the engine, not read here | read from source / inferred |
| Size limit 2048x2048 | The profile has no entry for it | Gap: a 4K client buffer would fail at AddFB2 with the kernel's message, not at the planner. Not a modifier issue; noted for the planner owner | inferred |
| 32-bit address | Not modelled | Gap, only matters for buffers above 4 GiB; not likely on the DK, whose RAM size was not read here | inferred |

Verdict: **the modifier handling matches** the driver, because the profile defers to `IN_FORMATS` and the LTDC's `IN_FORMATS` is LINEAR only. The two gaps (2048 limit, 32-bit address) are not modifier claims and are left for a different note.

## 6. What a board check would settle

Nothing here is run. Suggested checks, to be done by a board item, not by this note:

1. `modetest -p` (or `drm_info`) on the board: confirm `IN_FORMATS` lists only modifier 0 on planes 34, 42 and 49. Expected from section 1.
2. AddFB2 with a non-zero modifier: expect `-EINVAL`.
3. Import a GPU-allocated buffer (galcore) via `PRIME_FD_TO_HANDLE`: record success or `-EINVAL`. This is the one row no source reading here can answer.
