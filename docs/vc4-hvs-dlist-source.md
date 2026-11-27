# vc4 HVS5/HVS6 dlist memory and per-plane dlist size (rpi-6.18.y)

Answers the plane-budget part of the open questions in `docs/hvs-backend-design.md` §7 from the
kernel source: how big the dlist memory is, how many dlist words one plane uses, and how many
planes fit. Nothing here is observed on a board. Every statement is tagged **read from source**
(with `file:function`) or **inferred**. Tags carry the date 2026-11-28, the day the source was read.

## 1. Source read

- Tree: `raspberrypi/linux`, branch `rpi-6.18.y`, commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e`,
  directory `drivers/gpu/drm/vc4/`.
  Link: <https://github.com/raspberrypi/linux/tree/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4>
  (read from source, 2026-11-28).
- Files read: `vc4_hvs.c`, `vc4_plane.c`, `vc4_drv.h`, `vc4_regs.h` (read from source, 2026-11-28).
- How the files were obtained (read from source, 2026-11-28): this session could not reach the
  network, so it read the four raw files that the sibling items `vc4-scaling-source` and
  `vc4-load-source` had already downloaded into the project dependency cache
  (`rpi-linux/raw/`, saved 2026-11-28). Those items pin the same commit
  `16ae2649dcfadf9d81445b1177a3191efa3cb82e` (branch tip they resolved on 2026-11-28). This item did
  not re-hash the files, so the pin is inherited, not re-verified (inferred).
- Line numbers are not given; references are `file:function`.
- `enum vc4_gen`: `VC4_GEN_5` is BCM2711 (HVS5); `VC4_GEN_6_C` and `VC4_GEN_6_D` are BCM2712
  (HVS6). The enum is read from source (`vc4_drv.h`); the SoC mapping is inferred from the design doc.

## 2. Dlist memory

| Fact | Value | Where | Tag |
|---|---|---|---|
| HVS5 dlist window | `SCALER_DLIST_SIZE` = 0x4000 bytes = 4096 words, at register offset `SCALER5_DLIST_START` (0x4000) | `vc4_regs.h`, `vc4_hvs.c:vc4_hvs_bind` (`hvs->dlist = hvs->regs + SCALER5_DLIST_START` for gen >= 5) | read from source, 2026-11-28 |
| Reserved for the bootloader | `HVS_BOOTLOADER_DLIST_END` = 32 words ("just 128b out of our 16K") | `vc4_regs.h`, `vc4_hvs.c:__vc4_hvs_alloc` | read from source, 2026-11-28 |
| HVS5 allocator range | `drm_mm_init(&hvs->dlist_mm, 32, 4096 - 32)`: 4064 words | `vc4_hvs.c:__vc4_hvs_alloc` | read from source, 2026-11-28 |
| HVS6 size read from hardware | `dlist_size = HVS_READ(SCALER6_CXM_SIZE)` (4096 under kunit). It sets `hvs->dlist_mem_size`, which only the debugfs dump (`vc6_hvs_debugfs_dlist`) uses as a loop bound | `vc4_hvs.c:__vc4_hvs_alloc` | read from source, 2026-11-28 |
| HVS6 allocator range | The same function calls `drm_mm_init(&hvs->dlist_mm, ...)` a second time, after the generation switch and for every generation, with `HVS_BOOTLOADER_DLIST_END` and `(SCALER_DLIST_SIZE >> 2) - HVS_BOOTLOADER_DLIST_END`. The second call is the one that stays in effect, so the HVS6 allocator also spans 32..4096 = 4064 words, whatever `SCALER6_CXM_SIZE` says | `vc4_hvs.c:__vc4_hvs_alloc` | read from source, 2026-11-28; "second call wins" is inferred |
| Value of `SCALER6_CXM_SIZE` on BCM2712 | Not in the source. If it were smaller than 4096 the allocator would hand out words past the real memory | n/a | inferred |
| Permanent filter kernels | Two kernels (`mitchell_netravali_filter`, `nearest_neighbour_filter`), each `VC4_KERNEL_DWORDS` = 11 words, allocated from `dlist_mm` at bind and kept for the driver's life: 22 words. `vc4_hvs_bind` runs this for every generation, so HVS6 loses them too | `vc4_hvs.c:vc4_hvs_upload_linear_kernel`, `vc4_hvs_bind` | read from source, 2026-11-28 |
| Words left for frames | 4064 - 22 = 4042 on both generations | derived | inferred |

### Split between channels

- There is **no per-channel split**. One `struct drm_mm dlist_mm` serves all channels
  (`vc4_hvs.c:__vc4_hvs_alloc`), guarded by `hvs->mm_lock`. `vc4_hvs_alloc_dlist_entry` allocates
  one contiguous block per CRTC commit with `drm_mm_insert_node` and records the channel only in
  `alloc->channel` (read from source, 2026-11-28).
- `HVS_NUM_CHANNELS` = 3 (`vc4_drv.h`), for HVS5 and HVS6 alike (read from source, 2026-11-28).
- A CRTC's block holds the dlists of all its planes plus one end word:
  `vc4_hvs.c:vc4_hvs_atomic_check` sums `vc4_plane_dlist_size()` over the CRTC's planes and does
  `dlist_count++` ("Account for SCALER_CTL0_END"). `vc4_hvs_atomic_flush` writes the planes in
  `normalized_zpos` order and then `SCALER_CTL0_END`, and `WARN_ON_ONCE`s if the written length is not
  the block size (read from source, 2026-11-28).
- Blocks are not freed at once. `vc4_hvs_mark_dlist_entry_stale` puts a programmed block on
  `stale_dlist_entries`, and `vc4_hvs_dlist_free_work` frees it after the frame counter moves on.
  If an allocation fails, `vc4_hvs_alloc_dlist_entry` frees all stale entries and retries once
  (read from source, 2026-11-28).
- Consequence: during a page flip a CRTC holds its old block and its new block at the same time.
  The steady-state need is about 2 x the largest frame per CRTC (inferred).

## 3. Dlist words per plane

The count is `vc4_state->dlist_count`, which `vc4_plane_dlist_size` returns. It is built by
`vc4_plane.c:vc4_plane_mode_set` on HVS5 and `vc4_plane.c:vc6_plane_mode_set` on HVS6
(read from source, 2026-11-28). `N` is the number of colour planes of the framebuffer format
(`fb->format->num_planes`).

### 3.1 Words

| Part | HVS5 (`vc4_plane_mode_set`, gen 5 branch) | HVS6 (`vc6_plane_mode_set`) | Tag |
|---|---|---|---|
| CTL0, POS0, CTL2 | 3 | 3 | read from source |
| POS1 (scaled size), only if not unity | 1 | 1 | read from source |
| POS2 (source size) | 1 | 1 | read from source |
| POS3 (context, `0xc0c0c0c0`) | 1 | 1 | read from source |
| Pointers | per colour plane: pointer + pointer context + pitch = 3 words (`N` pointers, `N` contexts, `N` pitches) | per colour plane: PTR0, PTR1, PTR2 = 3 words | read from source |
| Colour-space words | 3 if the format is YUV (`ccm[0..2]`) | 0 (the CSC mode is a field in CTL2, `vc6_plane_get_csc_mode`) | read from source |
| LBM base address | 1 word if any vertical scaling (`y_scaling[0]` or `[1]` not NONE): `lbm_offset` slot | 1 word, same condition | read from source |
| UPM entry | none. HVS5 has no UPM | none as a separate word: the UPM base, handle and buffer-line fields are OR-ed into PTR0 by `vc6_plane_allocate_upm` | read from source |
| Scaling parameters | per channel (channel 1 first for planar YUV, then channel 0), see 3.2 | same | read from source |
| PPF kernel pointers | 4 words, once, if any channel uses PPF | 4 words, once, if any channel uses PPF | read from source |
| Own end word | 0 (the CRTC block has one end word) | 1 (`SCALER6_CTL0_END` per plane, then the CRTC block adds one more) | read from source; the double end on HVS6 is inferred from both `vc6_plane_mode_set` and `vc4_hvs_atomic_check` counting an end word |

Fixed part for a plane with `N` colour planes: unity header = 5 words (6 if not unity). HVS5 total
before extras = header + 3N (+3 if YUV). HVS6 total before extras = header + 3N + 1.

The scaling block is entered on HVS5 when any scaling mode is not NONE; on HVS6 when
`!is_unity || fb->format->is_yuv` and any mode is not NONE (`vc4_plane_mode_set`,
`vc6_plane_mode_set`; read from source, 2026-11-28).

### 3.2 Scaling parameter words per channel

From `vc4_plane.c:vc4_write_scaling_parameters`, `vc4_write_ppf`, `vc4_write_tpz` (read from source,
2026-11-28):

| Axis and mode | Words |
|---|---|
| Horizontal PPF | 1 |
| Vertical PPF | 2 (parameter + context) |
| Horizontal TPZ | 2 (scale + reciprocal) |
| Vertical TPZ | 3 (scale + reciprocal + context) |
| Mode NONE | 0 |

Mode choice, `vc4_get_scaling_mode`: NONE if source equals destination, TPZ if the source is at most
one pixel, PPF if `3 * dst >= 2 * src` (downscale by 1.5 or less, or any upscale), else TPZ
(read from source, 2026-11-28). `vc4_plane_setup_clipping_and_scaling` forces PPF on the chroma channel
in both axes for every planar YUV format, even at ratio 1:1; it also forces PPF for a fractional
source start, and PPF for TPZ upscales of YUV (read from source, 2026-11-28). YUV444 forces PPF on
luma too: horizontal on both generations (`is_yuv444_unity`), vertical as well on HVS6
(`vc6_plane_mode_set`) (read from source, 2026-11-28).

### 3.3 Results per plane class

All numbers are sums of the rules above. They are derived, so inferred; the rules behind them are
read from source. Date 2026-11-28.

| Class | HVS5 words | HVS6 words | Working (HVS5 / HVS6) |
|---|---|---|---|
| Linear RGB, unscaled, integer source position | **8** | **9** | 5 + 3 / 5 + 3 + 1 |
| Linear RGB, scaled, smallest (H-TPZ only) | 11 | 12 | 6 + 3 + 2 / 6 + 3 + 1 + 2 |
| Linear RGB, scaled, H-PPF only (also a fractional source start) | 14 | 15 | 6 + 3 + 1 + 4 / 6 + 3 + 1 + 1 + 4 |
| Linear RGB, scaled, H-TPZ and V-TPZ | 15 | 16 | 6 + 3 + 1 LBM + 2 + 3 / +1 end |
| Linear RGB, scaled, H-PPF and V-PPF | 17 | 18 | 6 + 3 + 1 LBM + 1 + 2 + 4 / +1 end |
| Linear RGB, scaled, largest (H-PPF + V-TPZ, or H-TPZ + V-PPF, with kernels) | 18 | 19 | 6 + 3 + 1 LBM + 8 / +1 end |
| NV12-type YUV (N = 2), unscaled | **22** | **20** | HVS5: 5 + 6 + 3 CSC + 1 LBM + 3 chroma (H-PPF + V-PPF) + 4 kernels. HVS6: 5 + 6 + 1 end + 1 LBM + 3 + 4 |
| NV12-type YUV, scaled, largest (luma TPZ and TPZ, chroma PPF + TPZ mix, kernels) | 29 | 27 | HVS5: 6 + 6 + 3 + 1 + 13. HVS6: 6 + 6 + 1 + 1 + 13 |
| YUV444 planar (N = 3), unscaled | **27** | **27** | HVS5: 6 + 9 + 3 + 1 + 3 chroma + 1 luma H-PPF + 4. HVS6: 6 + 9 + 1 + 1 + 3 + 3 + 4 |
| YUV 4:2:0 three-plane (N = 3), largest | 32 | 30 | 6 + 9 + 3 + 1 + 13 / 6 + 9 + 1 + 1 + 13 |
| SAND (SAND64/128/256 NV12 or P030, N = 2) | same as NV12-type YUV: 22..29 | same as NV12-type YUV: 20..27 | see below |

In the table, 13 is the largest parameter total over both channels: one channel at H-TPZ + V-TPZ
(2 + 3 = 5), the other at H-PPF + V-TPZ (1 + 3 = 4) or H-TPZ + V-PPF (2 + 2 = 4), plus 4 kernel
words (inferred).

SAND: the tiling modifier only changes the `tiling` field in CTL0 and the pitch word value (the
tile-skip field on HVS5, `pitch[i]` instead of `fb->pitches[i] * vdownsample` for SAND128/SAND256 on
HVS6). No branch for SAND adds or removes a `vc4_dlist_write` (`vc4_plane_mode_set` and
`vc6_plane_mode_set`, tiling switches; read from source, 2026-11-28). SAND formats are planar YUV
and so also carry the forced chroma scaling, so their count equals the NV12 row (inferred).
Supported SAND modifiers differ: HVS5 handles SAND64, SAND128 and SAND256; `vc6_plane_mode_set` only
raster, SAND128 and SAND256, and `vc4_plane.c:vc4_format_mod_supported` rejects SAND64 and T-tiled on HVS6
(read from source, 2026-11-28).

A plane that is off-screen or has a zero-size source writes no words (`dlist_initialized` set with
`dlist_count` left 0; read from source, 2026-11-28). The CRTC block still adds its own end word.

## 4. Other per-plane resources

The dlist is not the only shared pool, and for HVS6 it is not the first one to run out
(all read from source, 2026-11-28):

| Resource | Size | Used by | Where |
|---|---|---|---|
| LBM handles | `VC4_NUM_LBM_HANDLES` = 64 (`ida_alloc_range` 1..64) | planes with vertical scaling | `vc4_drv.h`, `vc4_plane.c:vc4_plane_allocate_lbm` |
| LBM memory | HVS5: 60K words (`lbm_size = 60 * SZ_1K`). HVS6: `lbm_size = 1024` in `__vc4_hvs_alloc` | same | `vc4_hvs.c:__vc4_hvs_alloc` |
| UPM handles | `VC4_NUM_UPM_HANDLES` = 32 (`ida_alloc_range` 1..32), HVS6 only, one per colour plane | every enabled plane on HVS6 | `vc4_drv.h`, `vc4_plane.c:vc6_plane_allocate_upm` |
| UPM memory | 1024 x `HVS_UBM_WORD_SIZE` (256) = 262144 bytes, HVS6 only | same | `vc4_hvs.c:__vc4_hvs_alloc`, `vc4_plane.c:vc6_upm_size` |

Whether HVS6's `lbm_size = 1024` is the real BCM2712 LBM size is not shown by the source: the
comment says it is a plausible value for kunit, and `SCALER6_LBM_SIZE` exists as a register
(`vc4_regs.h`). Its use at runtime for gen 6 is unclear in the lines read (inferred).

## 5. How many planes fit

Inputs: 4042 usable words (section 2), per-plane words (section 3.3), CRTC end word +1 per commit,
a transient doubling for stale blocks, `HVS_NUM_CHANNELS` = 3 (section 2). All figures are derived
(inferred, 2026-11-28).

| Case | HVS5 | HVS6 |
|---|---|---|
| Words for one CRTC with the engine's `MAX_LAYERS` = 16 unscaled RGB planes | 16 x 8 + 1 = 129 | 16 x 9 + 1 = 145 |
| Words for 16 largest-case YUV planes (3-plane worst case) | 16 x 32 + 1 = 513 | 16 x 30 + 1 = 481 |
| One CRTC, all words, only dlist limit, unscaled RGB | (4042 - 1) / 8 = 505 planes | (4042 - 1) / 9 = 449 planes |
| One CRTC, all words, YUV444 | (4042 - 1) / 27 = 149 planes | 149 planes |
| Three CRTCs each at 16 largest-case YUV, with the old block still stale | 3 x 2 x 513 = 3078 words, under 4042 | 3 x 2 x 481 = 2886 words, under 4042 |
| Planes the kernel creates (design doc §3: 1 primary, 1 cursor, 16 overlays shared by the non-writeback CRTCs, 32 for the writeback CRTC) | Even if every one of 60 planes were on screen as the largest YUV plane, 60 x 32 + 3 = 1923 words fits once | 60 x 30 + 3 = 1803 words fits once |

Per channel: there is no per-channel quota, so one channel could in theory use all 4042 words
(inferred). The per-channel figure that follows from the engine's 16-layer cap is 129..513 words on
HVS5 and 145..481 on HVS6.

### Conclusions

1. **The dlist does not bind** on either generation for a frame the engine can plan (at most 16
   layers per output): the worst plausible frame uses about 513 of 4042 words (13 %), and three
   CRTCs with stale blocks about 3078 (76 %) (inferred). This agrees with the design doc §4 row
   "Dlist words" (about 321 words with 7 to 20 words per layer). Corrections to that row, from this
   read: per-plane size is 8 to 18 words for RGB and 20 to 32 words for YUV and SAND, not 7 to 20,
   and 22 words of the 4064 are taken by the permanent filter kernels (inferred from the source).
2. **On HVS6, UPM handles bind before dlist words.** One handle per colour plane and 32 handles
   in all: at most 32 linear RGB planes or 16 NV12 or SAND planes (10 three-plane formats such as
   YUV444, at 3 handles each) may be enabled at the same time across all CRTCs, far below what the dlist
   allows. `vc6_plane_allocate_upm` fails with the "Out of upm_handles" debug message, and
   `vc4_plane_atomic_check` returns the error (read from source; the cross-plane arithmetic is
   inferred). Handles are shared only through the refcount copied by `vc4_plane_duplicate_state`
   when a plane's state is duplicated with a matching size; two different planes do not share
   one (inferred).
3. **LBM handles bind for scaled planes**: at most 64 planes with vertical scaling (both
   generations), and HVS5 LBM memory of 60K words (inferred from the sizes in `__vc4_lbm_size`;
   for HVS5 a 1920-pixel-wide RGB plane with vertical PPF needs `1920 * 16 = 30720` bytes rounded to
   128, divided by 4 = 7680 words, so about 8 such planes fit in 61440 words; inferred).
4. **Suggested planner budget** (inferred): treat the dlist as a check-time limit
   (`4042` words, 22 permanent) and the HVS6 UPM handle count as a plane-count limit (32 colour
   planes across all outputs). The LBM word budget is a scaled-plane limit on HVS5. The design doc's
   proposal to leave the dlist to `check` is consistent with this read.

## 6. What this does not settle

- The real `SCALER6_CXM_SIZE` value of a BCM2712 and the real BCM2712 LBM size: the source gives
  only a register name and kunit defaults. A board read of `hvs_dlists` and `hvs_dlist_allocs`
  (debugfs) would give them. That belongs to the board items, not to this one (inferred).
- Whether the doubled `drm_mm_init` call is intended. It looks like a merge leftover (inferred);
  the effect on HVS6 is only that the allocator ignores `SCALER6_CXM_SIZE`.
- Nothing was run. Everything here comes from reading source, and no statement is an observation of
  a board.
