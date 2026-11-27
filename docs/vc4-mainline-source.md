# vc4 HVS5/HVS6 plane code: mainline v6.18 against rpi-6.18.y

Date of reading: 2026-11-28. Item vc4-mainline-source-r3 of plan #459.
Companion to `docs/hvs-backend-design.md` (§4, §5, §7); nothing in that
document is changed here.

## Sources and how they were fetched

| Tree | Commit | Files |
|---|---|---|
| mainline Linux v6.18 | 7d0a66e4bb9081d75c82ec4957c50034cb0ea449 | `drivers/gpu/drm/vc4/{vc4_plane.c,vc4_hvs.c,vc4_crtc.c,vc4_kms.c,vc4_drv.h,vc4_regs.h}` |
| raspberrypi/linux rpi-6.18.y | 16ae2649dcfadf9d81445b1177a3191efa3cb82e | the same six files |

URL pattern (read through the web-fetch subagent, method 3 of the item):
`https://raw.githubusercontent.com/torvalds/linux/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/gpu/drm/vc4/<file>`
and
`https://raw.githubusercontent.com/raspberrypi/linux/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4/<file>`.

Fetch order and outcome (2026-11-28):

1. Copies under the node's dependency cache: failed. `linux-v6.18` holds
   only three Rockchip VOP2 files, `rpi-linux` has an empty `raw/`
   directory, `vc4-src/` is empty, and `linux-6.18-drm` holds four DRM core
   files. No vc4 file was present, so no commit could be checked.
2. `git -c credential.helper= -c core.askPass= clone --filter=blob:none
   --no-checkout --depth 1`: not run. The harness refused the command
   (approval required), and the item forbids changing node git or
   credential configuration.
3. Web-fetch subagent on the raw URLs: worked.
   - rpi-6.18.y: all six files read in full (chunked with offsets).
   - mainline: all six files read in full. The first request for
     `vc4_hvs.c` returned HTTP 429 and gave verbatim text only up to about
     character 39,300 of 50,400; a second pass with an offset on
     2026-11-28 read the tail (`vc4_hvs_cob_init`, `vc4_hvs_bind`)
     verbatim, for both trees.

Limits of this reading. The fetch tool returns no line numbers, so
references give the exact function and file and no line. Text came through
a fetch tool that may truncate or paraphrase; both subagents separated
verbatim text from summaries, and only the verbatim parts are tagged
"read from source". There was no compiler or board run. Nothing here is a
board measurement.

Tags: **read from source** (2026-11-28, verbatim text of the commit named),
**inferred** (2026-11-28, my conclusion from that text).

Generation names: HVS5 is `VC4_GEN_5` (BCM2711). HVS6 is `VC4_GEN_6_C` or
`VC4_GEN_6_D` (BCM2712, versions 0x53 and 0x54 of `SCALER6_VERSION` in
`vc4_regs.h`).

## 1. Summary of differences

| Topic | mainline v6.18 | rpi-6.18.y |
|---|---|---|
| vertical line skipping on TPZ downscale | absent | `vc4_plane_set_vdownsample` |
| TPZ horizontal ratio limit | no check found | `vc4_plane_setup_clipping_and_scaling`: `src_w/crtc_w > FIELD_MAX(SCALER_TPZ0_SCALE_MASK)` gives -EINVAL |
| TPZ for source <= 1 px | not present | present in `vc4_get_scaling_mode` |
| HVS6-only formats (P010, P012, P016, S010, S012, S016) | none | `hvs_formats[]` with `.hvs6_only` |
| overlay planes | 16, `possible_crtcs` all CRTCs | 16 on non-TXP CRTCs, plus 32 on the TXP CRTC |
| `vc6_plane_mode_set` modifiers | linear, SAND128, SAND256 | same |
| `vc4_format_mod_supported` | no generation check; HVS6 still advertises T-tiled and SAND64 | HVS6 returns false for T-tiled and SAND64 |
| HVS6 UPM sizing for SAND | raster formula only | SAND128 stride terms added |
| `vc4_crtc_handle_page_flip` underrun unmask | `if (vc4->gen < VC4_GEN_6_C)` | `if (0 && ...)`, dead code |

All rows are **read from source** (2026-11-28).

## 2. Scaling paths and limits

### Shared code (HVS5 and HVS6)

- read from source (2026-11-28): `vc4_get_scaling_mode` in `vc4_plane.c`
  returns NONE when the size is unchanged, PPF when `3 * dst >= 2 * (src >> 16)`
  and TPZ otherwise. Upscales and downscales up to 1.5x use PPF, steeper
  downscales use TPZ. There is no generation switch in either tree.
- read from source (2026-11-28), rpi only: `vc4_get_scaling_mode` returns TPZ
  when `src <= (1 << 16)` ("Source rectangle <= 1 pixel can use TPZ for
  resize/upscale"). Mainline has no such branch.
- read from source (2026-11-28): `vc4_plane_setup_clipping_and_scaling` calls
  `drm_atomic_helper_check_plane_state(state, crtc_state, 1, INT_MAX, true,
  true)` in both trees, so the DRM helper sets no ratio limit.
- read from source (2026-11-28): both trees force chroma channels with a
  NONE mode to PPF for multi-plane (YUV) formats.
- read from source (2026-11-28), rpi only: a fractional `src_x`/`src_y` with
  mode NONE is promoted to PPF, TPZ upscaling for YUV or a fractional start
  is forced to PPF, and the horizontal TPZ ratio is rejected above
  `FIELD_MAX(SCALER_TPZ0_SCALE_MASK)`. In `vc4_regs.h` that mask is
  `VC4_MASK(28, 8)` (21 bits, 16.16), so the cap is just under 32x.
- read from source (2026-11-28), rpi only: `vc4_plane_set_vdownsample` skips
  source lines when the vertical mode is TPZ and `src_h/crtc_h > (3 << 16)`:
  `vds = ((src_h[0] / (crtc_h * 3)) >> 16) + 1`. The factor is clamped to
  `max_tile_skip` for SAND, `max_pitch / pitches[0]` for linear, and 1 for
  other modifiers (T-tiled). It returns -EINVAL if
  `src_h[i] / vds / crtc_h > FIELD_MAX(SCALER_TPZ0_SCALE_MASK)`.
- read from source (2026-11-28): `vc4_write_tpz` and `vc4_write_ppf` carry
  `WARN_ON_ONCE(vc4->gen > VC4_GEN_6_D)`. `vc4_write_tpz` notes that BCM2712
  lacks BIT(31). In `vc4_regs.h`: `SCALER_TPZ0_VERT_RECALC` BIT(31),
  `SCALER_TPZ0_SCALE_MASK` bits 28:8, `SCALER_PPF_SCALE_MASK` bits 24:8.
- inferred (2026-11-28): with no vertical skipping, mainline accepts any
  vertical TPZ ratio at the plane check and relies on the register field
  (`VC4_SET_FIELD` has `WARN_ON(!FIELD_FIT(...))`), so an extreme ratio
  would warn instead of returning -EINVAL. The rpi tree turns the same case
  into a clean -EINVAL. A TEST_ONLY sweep (plan item rpi5-sweep) is the way to
  confirm; this document makes no board claim.
- read from source (2026-11-28): `vc4_plane_calc_load` uses
  `hvs_load_shift = 1` when any axis is scaled and 2 otherwise, and
  multiplies the memory-bus load by `DIV_ROUND_UP(src_h >> 16, crtc_h)`.

### HVS5 only

- read from source (2026-11-28): `vc4_plane_mode_set` in `vc4_plane.c` checks
  the source width against `FIELD_MAX(SCALER5_POS2_WIDTH_MASK)`
  (`VC4_MASK(12, 0)`, so 8191), in the rpi tree. Mainline: not reported in
  the portions read.
- read from source (2026-11-28): LBM size in `__vc4_lbm_size`: zero without
  vertical scaling; `pix_per_line` is `crtc_w` for horizontal TPZ, else the
  source span; `* 8` for non-YUV with vertical TPZ, `* 16` otherwise;
  `roundup(.., 128)` then `/ 4` on HVS5. `vc4_plane_allocate_lbm` then
  applies `ALIGN(lbm_size, 64)` for `VC4_GEN_5`.
- read from source (2026-11-28): both trees do the YUV444 case in
  `vc4_plane_mode_set` by setting `x_scaling[0] = VC4_SCALING_PPF`.

### HVS6 only

- read from source (2026-11-28): `vc4_lbm_size` dispatches to `__vc6_lbm_size`
  for `gen >= VC4_GEN_6_C`. `vc4_lbm_channel_size` uses
  `round_up(width / hsub, 4)` (rpi: `DIV_ROUND_UP`), words per component 4
  for PPF and 2 for TPZ (`vc4_lbm_words_per_component`), components 1 or 2
  for YUV, 4 with alpha, otherwise 3 (`vc4_lbm_components`), and
  `lines = DIV_ROUND_UP(words, 128 / hsub)`. No extra alignment ("BCM2712
  doesn't need to be aligned").
- read from source (2026-11-28): in mainline `vc4_lbm_channel_size` the loop
  `for (i = 0; i < 2; i++)` tests `y_scaling[channel]` instead of
  `y_scaling[i]`, so `channels_scaled` is 0 or 2 and the
  `channels_scaled == 1` halving never runs. Not checked in the rpi tree.
- read from source (2026-11-28): `vc6_plane_mode_set` forces YUV444 luma to
  the scaler ("GEN6 requires the scaler for the luma channel to be enabled
  for YUV444") and runs the scaling block under
  `!is_unity || fb->format->is_yuv`. It has no explicit width range check;
  fields are `width - 1` in 13-bit masks (`SCALER6_POS2_SRC_WIDTH_MASK`,
  bits 12:0).
- inferred (2026-11-28): the 13-bit HVS6 field gives the same 8191 maximum
  width as HVS5, which matches `max_source 8191` in
  `docs/hvs-backend-design.md` §4.

## 3. Formats and modifiers

### Common to both generations

- read from source (2026-11-28): `vc4_plane_init` advertises the same
  modifier list for every generation: T_TILED, SAND128, SAND64, SAND256,
  LINEAR, then INVALID.
- read from source (2026-11-28): `vc4_format_mod_supported` has no generation
  check in mainline. Per format: XRGB8888, ARGB8888, ABGR8888, XBGR8888,
  RGB565, BGR565, ARGB1555, XRGB1555: linear or T-tiled. NV12 and NV21:
  linear, SAND64, SAND128, SAND256. P030: SAND128 only. Everything else
  (4444, RGB332, YUV422, YUV420, NV16, NV61, the 10-10-10-2 formats): linear
  only. The case list names RGBX1010102 style fourccs while the table has
  XRGB2101010 style ones, so the 10-10-10-2 formats reach the default branch
  either way.
- read from source (2026-11-28): the format filter in `vc4_plane_init` is
  `!hvs5_only || gen >= VC4_GEN_5`, so HVS5 and HVS6 both get P030 and
  XRGB2101010, ARGB2101010, ABGR2101010, XBGR2101010.
- read from source (2026-11-28): rotation is 0, 180, REFLECT_X and REFLECT_Y;
  blend modes none, premultiplied, coverage; encodings BT601, BT709, BT2020.
- read from source (2026-11-28): HVS5 and HVS6 use `drm_gem_fb_create`; the
  T-tiled `vc4_fb_create` fallback in `vc4_kms.c` is `VC4_GEN_4` only.

### HVS5 (`vc4_plane_mode_set`)

- read from source (2026-11-28): handles linear, T-tiled (4 KB tiles, tile
  width shift 6 for 16 bpp, else 5), SAND64 (`SCALER_CTL0_TILING_64B`),
  SAND128 (`_128B`), SAND256 (`_256B_OR_T`). Non-P030 SAND uses
  `HVS_PIXEL_FORMAT_H264`, P030 uses `HVS_PIXEL_FORMAT_YCBCR_10BIT`.
  `param > SCALER_TILE_HEIGHT_MASK` returns -EINVAL. Default returns -EINVAL.

### HVS6 (`vc6_plane_mode_set`)

- read from source (2026-11-28): handles linear, SAND128
  (`SCALER6_CTL0_ADDR_MODE_128B`) and SAND256 (`_256B`). The default case
  returns -EINVAL ("Unsupported FB tiling flag"). Non-P030 SAND uses
  `HVS_PIXEL_FORMAT_YCBCR_YUV420_2PLANE`. P030 uses `YCBCR_10BIT`.
- read from source (2026-11-28): in mainline the plane still advertises
  T-tiled and SAND64 on HVS6, and `vc4_format_mod_supported` says yes, so
  an atomic check with those modifiers fails in `vc6_plane_mode_set`, not
  at the IN_FORMATS level. In rpi-6.18.y `vc4_format_mod_supported` returns
  false for T-tiled and SAND64 on HVS6 ("vc6_plane_mode_set() only handles
  linear, SAND128 and SAND256").
- inferred (2026-11-28): this is the likely source of the conflict between
  an IN_FORMATS blob and the source that `docs/hvs-backend-design.md` asks to
  settle. A mainline kernel on a Pi 5 would list T-tiled and SAND64 pairs
  that TEST_ONLY then rejects; an rpi kernel would not list them. A
  TEST_ONLY run per generation (plan item rpi5-sweep) decides it.
- read from source (2026-11-28), rpi only: HVS6-only formats P010, P012,
  P016, S010, S012, S016 are flagged `.hvs6_only` in `hvs_formats[]`.
  Mainline `vc4_plane_init` has no `hvs6_only` field.
- read from source (2026-11-28): HVS6 uses the `pixel_order_hvs5` column for
  `SCALER6_CTL0_ORDERRGBA` in both trees (`vc6_plane_mode_set`).

## 4. zpos and plane count

Identical for HVS5 and HVS6 (no generation gating) unless noted.

- read from source (2026-11-28): `vc4_crtc_init` in `vc4_crtc.c` creates one
  primary plane per CRTC with `vc4_plane_init(drm, DRM_PLANE_TYPE_PRIMARY,
  0)`. A primary plane gets `drm_plane_create_zpos_immutable_property(plane,
  0)`.
- read from source (2026-11-28): `vc4_plane_create_additional_planes` has
  `#define VC4_NUM_OVERLAY_PLANES 16`. Overlay `i` gets
  `drm_plane_create_zpos_property(plane, i + 1, 1, VC4_NUM_OVERLAY_PLANES +
  1)`, i.e. initial `i+1`, range 1 to 17. Each CRTC gets one cursor plane
  with zpos initial 17, range 1 to 17.
- read from source (2026-11-28): mainline overlays use `possible_crtcs =
  GENMASK(num_crtc - 1, 0)`. rpi-6.18.y uses the non-TXP CRTC mask and
  additionally creates 32 overlay planes (`VC4_NUM_TXP_OVERLAY_PLANES`,
  zpos 1 to 33) for the TXP CRTC. The comment there names the 16 KB HVS
  space and the 28-byte minimum plane as the limit.
- read from source (2026-11-28): `vc4_kms_load` sets `normalize_zpos = true`,
  and `vc4_hvs_atomic_flush` writes planes in normalized-zpos order.
- read from source (2026-11-28): `HVS_NUM_CHANNELS` is 3 (`vc4_drv.h`).
- HVS5 CRTCs (`vc4_crtc.c`): pv0 channels {0}, output 0; pv1 {0,1,2},
  output 3; pv2 {0,1,2}, output 4, HDMI0; pv3 {1}, output 1; pv4 {0,1,2},
  output 5, HDMI1. read from source (2026-11-28).
- HVS6 CRTCs: bcm2712_pv0 channel {0}, output 0, HDMI0; bcm2712_pv1
  channel {1}, output 1, HDMI1. read from source (2026-11-28).
- inferred (2026-11-28): with the planes shared by all non-TXP CRTCs, a
  BCM2711 with two HDMI CRTCs sees the same 16 overlays from both. This
  matches the recorded per-output reach noted in
  `docs/hvs-backend-design.md`; only a board can confirm.
- read from source (2026-11-28): `vc4_pv_muxing_atomic_check` (`vc4_kms.c`)
  assigns each newly enabled CRTC `ffs(unassigned_channels &
  hvs_available_channels) - 1` and returns -EINVAL if none matches. HVS6
  muxing commit (`vc6_hvs_pv_muxing_commit`) only acts for channel 1.
- read from source (2026-11-28): max framebuffer size set in `vc4_kms_load`:
  HVS6 8192x8192, HVS5 7680x7680.

## 5. LBM, UPM and dlist allocation

- read from source (2026-11-28): `__vc4_hvs_alloc` (`vc4_hvs.c`) sizes `lbm_mm`
  as 60 K words on HVS5 and a hard-coded 1024 on HVS6 (the
  `SCALER6_LBM_SIZE` register is not read), both trees.
- read from source (2026-11-28): HVS6 UPM pool is `drm_mm_init(&hvs->upm_mm,
  0, 1024 * HVS_UBM_WORD_SIZE)` (256 KiB, `HVS_UBM_WORD_SIZE` 256). Handles:
  `VC4_NUM_UPM_HANDLES` 32, `VC4_NUM_LBM_HANDLES` 64 (rpi `vc4_drv.h`;
  mainline `vc4_drv.h` shows 32 for UPM).
- read from source (2026-11-28): `vc6_upm_size` (`vc4_plane.c`):
  `words_per_line = (stride + 62) / 32`, `fetch_region_size = words_per_line
  * 32`, `buffer_lines = 2 << upm_buffer_lines`, size aligned to 256.
  `vc6_plane_allocate_upm` always selects
  `SCALER6_PTR0_UPM_BUFF_SIZE_2_LINES` and carries the note "This only works
  for raster formats". The rpi tree adds a SAND128 stride term
  (`ALIGN(width, 128)`, P030 `ALIGN(width, 96) * 4 / 3`, doubled for plane 0
  of YUV444); the mainline text read shows only the raster stride.
- read from source (2026-11-28): UPM handle fields in `vc4_regs.h`:
  `SCALER6_PTR0_UPM_BASE_MASK` bits 28:16 (start / 256),
  `SCALER6_PTR0_UPM_HANDLE_MASK` bits 14:10 (handle - 1),
  `SCALER6_PTR0_UPM_BUFF_SIZE_MASK` bits 9:8.
- read from source (2026-11-28): `vc4_plane_atomic_check` allocates LBM on every
  generation and UPM additionally on `gen >= VC4_GEN_6_C`, and frees UPM when
  a plane is disabled.
- read from source (2026-11-28): dlist memory in `__vc4_hvs_alloc`. HVS5:
  `dlist_start = HVS_BOOTLOADER_DLIST_END` (32), size
  `(SCALER_DLIST_SIZE >> 2) - 32` words, `SCALER_DLIST_SIZE` 0x4000.
  HVS6: size `HVS_READ(SCALER6_CXM_SIZE)`. `hvs->dlist_mem_size` is set from that size. Mainline
  calls `drm_mm_init(&hvs->dlist_mm, ...)` once. rpi-6.18.y calls it twice:
  the same per-generation call, then a second unconditional
  `drm_mm_init(&hvs->dlist_mm, HVS_BOOTLOADER_DLIST_END,
  (SCALER_DLIST_SIZE >> 2) - HVS_BOOTLOADER_DLIST_END)` right after
  `INIT_WORK(&hvs->free_dlist_work, ...)`. Confirmed by a second verbatim
  read (2026-11-28). inferred (2026-11-28): on HVS6 the second call
  replaces the `SCALER6_CXM_SIZE` size with the 4064-word gen 4/5 size, while
  `dlist_mem_size` keeps the CXM value. The dlist budget belongs to plan item
  vc4-dlist-source.
- read from source (2026-11-28): `vc4_hvs_atomic_check` sums
  `vc4_plane_dlist_size` over the CRTC's planes, adds 1 for
  `SCALER_CTL0_END` and allocates with `drm_mm_insert_node`. In rpi, stale
  entries are queued by `vc4_hvs_mark_dlist_entry_stale` and freed from a
  worker once the 6-bit frame counter passes; mainline has no
  `vc4_hvs_alloc_dlist_entry` in the text read.
- read from source (2026-11-28), both trees identical: `vc4_hvs_cob_init`
  gives HVS5 `VC5_COB_SIZE` 44416 with 4096-pixel lines (channel 2 and 1: 3
  lines each, channel 0 the rest) and HVS6 `VC6_COB_LINE_WIDTH` 3840 with
  `VC6_COB_NUM_LINES` 4 (channel 2 base 0 top 3840; channel 1 base 3856
  top 19216; channel 0 base 19232 top 34592, derived from the code).
- read from source (2026-11-28): `vc4_hvs_bind` requests the HVS interrupt
  only for `gen < VC4_GEN_6_C` in mainline. rpi-6.18.y additionally requests
  one `chN-eof` interrupt per channel on HVS6 (handler
  `vc6_hvs_eof_irq_handler`, `IRQF_NO_AUTOEN`).
- read from source (2026-11-28): rpi uploads two filter kernels (Mitchell
  and nearest-neighbour), mainline one.

## 6. Present in only one tree

All read from source (2026-11-28) unless tagged.

Only in rpi-6.18.y: `vc4_plane_set_vdownsample`; the <= 1 px TPZ rule; the
TPZ ratio -EINVAL; HVS6-only formats P010/P012/P016/S010/S012/S016; 32 TXP
overlay planes and the non-TXP `possible_crtcs` mask; generation filter in
`vc4_format_mod_supported` for HVS6; SAND-aware UPM sizing; the dlist
stale-entry worker with per-channel EOF interrupts; `if (0 && ...)` around
the underrun unmask in `vc4_crtc_handle_page_flip`.

Only in mainline: `vc4_crtc_handle_page_flip` still unmasks underrun for
`gen < VC4_GEN_6_C`; `vc4_hvs_irq_handler`, `vc4_hvs_mask_underrun` and
`vc4_hvs_unmask_underrun` all `WARN_ON(vc4->gen > VC4_GEN_5)`, so HVS6 has no
underrun IRQ path in the text read.

Identical in both: load tracker enabled only for `VC4_GEN_4`
(`vc4_kms_load`), so neither HVS5 nor HVS6 rejects on load; the core clock is
computed everywhere but applied only for `VC4_GEN_5 && !firmware_kms`
(`vc4_atomic_commit_tail`), clamped to 500 MHz minimum on a modeset; each
channel waits on the old state's `pending_commit` with `drm_crtc_commit_wait`
in `vc4_atomic_commit_tail` (the "Timed out waiting for commit" message).
Those load and clock details are the subject of plan item vc4-load-source.

## 7. What this means for the profiles

All inferred (2026-11-28):

- `Vc4Hvs5` and `Vc4Hvs6` may share one scaling table (mode selection is
  generation-independent) and differ in modifier support and format list.
- Modifier priority on HVS6 should drop T-tiled and SAND64 if a TEST_ONLY
  run on a mainline-kernel board rejects them; on an rpi kernel they never
  appear in IN_FORMATS.
- The 16-overlay plane pool is shared across CRTCs in both trees; the
  rpi tree's extra 32 TXP overlays only matter for a TXP CRTC.
- A TPZ scaling range cannot be taken from the source alone on mainline.
  Use the smallest range a TEST_ONLY run accepted, as the plan says.

## 8. Not covered

- The `if (0 && ...)` underrun unmask in rpi `vc4_crtc.c` was read once; a
  search of both `vc4_hvs.c` files found no such code there.
- Kernel configuration, device tree and firmware-KMS (`vc4->firmware_kms`)
  paths beyond the early return in `vc4_pv_muxing_atomic_check`.
