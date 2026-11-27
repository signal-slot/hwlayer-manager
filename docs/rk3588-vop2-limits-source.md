# RK3588 VOP2 input, line-buffer and scaling limits (mainline v6.18 source)

Written 2026-11-28 for the `max_source` and scaling rows of
`docs/rk3588-backend-design.md` §4. Source reading only: no board was touched.

## Source pin

- Tree: `torvalds/linux`, tag `v6.18`, commit
  `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` (read from source: the commit
  and tag object `f7b88edb52c8dd01b7e576390d658ae6eef0e134` are the pins that
  `docs/stm32mp257-ltdc-source-full.md` recorded on 2026-11-28 for the same
  tag; this task could not re-resolve the tag, see "How the files were
  obtained").
- Files read, all under `drivers/gpu/drm/rockchip/`: `rockchip_drm_vop2.c`
  (2793 lines), `rockchip_vop2_reg.c` (2577 lines), `rockchip_drm_vop2.h`
  (866 lines). Line numbers below are counted on these copies.
- Nothing else was read. In particular the device tree (which VPs exist, per
  plane reach) is out of scope here.

### How the files were obtained

The three files were already present, as per-file copies, in the project's
dependency cache (`linux-v6.18/` there) and were reused. The copies carry no
git metadata of their own, and network commands were not permitted in this
run, so the tag was **not** re-resolved and the bytes were **not** compared
with a fresh download. Consequence (inferred, 2026-11-28): the copies are
v6.18 only on the strength of the cache directory name and of their content
(they contain RK3576 tables, which fits v6.18 and not older trees). Re-check
with a raw per-file fetch of the three paths at the pinned commit before
relying on a single line number.

## Tags

Each statement is tagged `read from source` or `inferred`, with the date
2026-11-28 (the date of this reading; it is the same for all rows).

## 1. Sizes: max input and max output

| Item | RK3588 value | Where | Tag |
|---|---|---|---|
| Max plane input (source rect) | 4096 x 4320, one SoC-wide value for every window class | `rockchip_vop2_reg.c` `rk3588_vop` (`.max_input = { 4096, 4320 }`, line 2525) | read from source, 2026-11-28 |
| Where it is enforced | `drm_rect_width(src) >> 16 > max_input.width` or height `>` max returns `-EINVAL` | `rockchip_drm_vop2.c` `vop2_plane_atomic_check`, 1042-1050 | read from source, 2026-11-28 |
| Per-class input limit | None in code: the check does not look at the window class, the format or the modifier | same function, 992-1062 | read from source, 2026-11-28 |
| Max output (SoC-wide) | 4096 x 4320 | `rk3588_vop`, line 2526 | read from source, 2026-11-28 |
| Max output per video port | VP0, VP1, VP2: 4096 x 2304 (10-bit output); VP3: 2048 x 1536 (no 10-bit) | `rk3588_vop_video_ports[]`, 1057-1090 | read from source, 2026-11-28 |
| `max_output` used by the plane code | Not used: the field is only in the data tables and no `.c` line reads it (grep on `max_output` in `rockchip_drm_vop2.c` finds nothing) | `rockchip_drm_vop2.c` whole file | read from source, 2026-11-28 |
| Minimum size | source and destination rect at least 4x4, else the plane is set invisible and `atomic_check` returns 0 (no error) | `vop2_plane_atomic_check`, 1032-1040 | read from source, 2026-11-28 |
| YUV source origin | odd `src.x1` with a YUV format returns `-EINVAL` | `vop2_plane_atomic_check`, 1052-1059 | read from source, 2026-11-28 |

**Difference to the design doc.** `docs/rk3588-backend-design.md` §4 gives
`max_source` as 4096 x 7680 (Cluster sub-window 2048 x 7680). v6.18 says
4096 x **4320** for RK3588 and has no Cluster sub-window split: every
Cluster is registered as a single `ClusterN-win0` (`rockchip_vop2_reg.c`
1121-1203; the comment above it says "Every cluster can work as 4K win or
split into two win", but only `win0` rows exist). The 7680 height therefore
does not come from this tree (read from source, 2026-11-28). It may come
from the vendor kernel, which is a different tree (inferred, 2026-11-28).
The safe `max_source` for a mainline kernel is 4096 x 4320 (inferred,
2026-11-28).

## 2. Line-buffer modes and what they limit

| Class | Statement | Where | Tag |
|---|---|---|---|
| Cluster | The window has a 2048 x 16 line buffer; it runs as 2048 x 16 (full) or 4096 x 8 (half) | comment above `vop2_get_cluster_lb_mode`, `rockchip_drm_vop2.c` 514-518 | read from source, 2026-11-28 |
| Cluster | `CLUSTER_LB_MODE` register, 4 bits | `rockchip_vop2_reg.c` 315 (RK3568 table, which RK3588 uses: `rk3588_vop.cluster_reg = rk3568_vop_cluster_regs`) | read from source, 2026-11-28 |
| Cluster | Documented modes: 0 = half mode for input width 2048..4096; 1 = half mode for "2 * 2048 plane mode" (two sub-windows); 2 = half mode for 90/270 rotation | comment, 514-520 | read from source, 2026-11-28 |
| Cluster | The driver picks only 2 (rotate 90 or 270) or 0 (everything else). Mode 1 is never written, which fits there being no `win1` | `vop2_get_cluster_lb_mode`, 520-528; write at 1397-1399 | read from source, 2026-11-28 |
| Cluster | Width is not checked against 2048 in `atomic_check`: a 4096-wide Cluster source is handled by mode 0 and is accepted; the line-buffer mode is hardware state, not a reject reason | 992-1062, 520-528 | read from source, 2026-11-28 |
| Cluster | With 90/270 rotation the half-block flag is off (`vop2_half_block_enable` returns false), so the 16-line tile is used; whether a 4096-wide source still fits then is not stated in the source | `vop2_half_block_enable`, 428-434 | read from source for the flag; the fit is inferred as unknown, 2026-11-28 |
| Esmart | No line-buffer mode register is written for Esmart: `VOP2_WIN_CLUSTER_LB_MODE` is `{ .reg = 0xffffffff }` (unused) in the smart table. The line-buffer size of Esmart is not stated anywhere in the three files | `rockchip_vop2_reg.c` 396 | read from source, 2026-11-28 |
| Esmart | So the 4096 input width of Esmart is the SoC-wide `max_input`; no source statement ties it to a line buffer | `rk3588_vop` | inferred, 2026-11-28 |

## 3. Scaling

### Scale-factor range in `atomic_check`

| Statement | Where | Tag |
|---|---|---|
| `min_scale = 1/8`, `max_scale = 8` (16.16) are local constants of `vop2_plane_atomic_check` and are passed to `drm_atomic_helper_check_plane_state(pstate, cstate, min_scale, max_scale, true, true)`. They do not depend on the window class, the format or the modifier | `rockchip_drm_vop2.c` 1000-1001, 1020-1022 | read from source, 2026-11-28 |
| The check therefore accepts 1/8x to 8x for every window, in both axes, with no class-specific gate. A plane is not refused for "scaling not supported" by this driver | 992-1062 | read from source, 2026-11-28 |
| `win->data->max_upscale_factor` and `max_downscale_factor` exist (`rockchip_drm_vop2.h` 200-201) and are filled in the tables, but **no line of `rockchip_drm_vop2.c` or the header reads them** (grep finds only the two declarations) | `.h` 200-201; table rows below | read from source, 2026-11-28 |
| This version does not disable scaling for any class: there is no branch on `WIN_FEATURE_CLUSTER` or `WIN_FEATURE_AFBDC` that clears scaling or narrows the range in `atomic_check` | 992-1062 | read from source, 2026-11-28 |

### Table values per class

| Class (RK3588) | `max_upscale_factor` / `max_downscale_factor` in the table | Used by code? | Where | Tag |
|---|---|---|---|---|
| Cluster0..3 (`ClusterN-win0`) | 4 / 4 | No | `rockchip_vop2_reg.c` 1121-1203 | read from source, 2026-11-28 |
| Esmart0..3 (`EsmartN-win0`) | 8 / 8 | No | `rockchip_vop2_reg.c` 1204-1275 | read from source, 2026-11-28 |

### What the scaler hardware path does

| Item | Cluster | Esmart | Where | Tag |
|---|---|---|---|---|
| Scale-factor registers | `SCL_FACTOR_YRGB` X and Y only; the chroma factor fields are unused (`0xffffffff`) | YRGB X/Y and CBCR X/Y | `rockchip_vop2_reg.c` 304-305, 338-339 (cluster); 376-379 (smart) | read from source, 2026-11-28 |
| Factor formula | `vop2_scale_factor`: shift 12 when down-scaling, 16 when up-scaling; `(src-1)<<shift / (dst-1)`, clamped to 16 bits; returns `U16_MAX` for `dst < 2` | same | `rockchip_drm_vop2.c` 530-554 | read from source, 2026-11-28 |
| Vertical pre-decimation | `gt2` when `src_h >= 2*dst_h`, `gt4` when `src_h >= 4*dst_h` (source height is halved or quartered before the scaler). This is what lets down-scaling go past the table's 4 or 8 | same | `vop2_setup_scale`, 573-581 | read from source, 2026-11-28 |
| Filters | Cluster: bicubic for horizontal up-scale, others bilinear; the filter registers are written only for non-Cluster windows (`if (vop2_cluster_window(win)) return;`) | Esmart: nearest/bilinear/bicubic up, nearest/bilinear/average down. The driver picks bicubic (H up), bilinear (V up), bilinear (down, both) | `rockchip_vop2_reg.c` comment 1099-1117 (as printed above `rk3588_vop_win_data`); `vop2_setup_scale` 586-623 | read from source, 2026-11-28 |
| Chroma scaling | none (`return` for Cluster before the CBCR block) | separate CBCR factor, chroma size from `hsub`/`vsub` | `vop2_setup_scale`, 623-657 | read from source, 2026-11-28 |
| Even destination width | Cluster (`WIN_FEATURE_AFBDC`) is exempt | non-AFBDC (Esmart): when scaling down horizontally with odd `dst_w`, `dst_w` is incremented | `vop2_setup_scale`, 599-606 | read from source, 2026-11-28 |
| Source width workaround | Cluster exempt | non-AFBDC: `actual_w > dsp_w && (actual_w & 0xf) == 1` makes `actual_w` one smaller ("esmart can't support scale down when actual_w % 16 == 1") | `vop2_plane_atomic_update`, 1230-1243 | read from source, 2026-11-28 |

### Resulting limits to use for the profile

| Class | Accepted by `atomic_check` | Hardware figure from the table | Statement | Tag |
|---|---|---|---|---|
| Cluster | 1/8 .. 8 | 4 / 4 | the kernel will not refuse more than 4x, so a planner that trusts the table is the tighter and safe one | inferred, 2026-11-28 |
| Esmart | 1/8 .. 8 | 8 / 8 | the table figure equals the check | read from source, 2026-11-28 |

Down-scaling beyond the table value is possible in the register path through
`gt2`/`gt4` (up to 4x of the vertical axis before the scaler) but the source
does not say that it is supported by the scaler; do not rely on it
(inferred, 2026-11-28).

## 4. AFBC width and alignment limits

| Statement | Where | Tag |
|---|---|---|
| AFBC is offered only on windows whose table row has `format_modifiers = format_modifiers_afbc`: the four Cluster windows. Esmart rows use `format_modifiers` (linear and invalid only) | `rockchip_vop2_reg.c` 1121-1275; modifier lists 203-238 | read from source, 2026-11-28 |
| RK3588 Cluster modifiers are 16x16 superblock AFBC with combinations of SPARSE, YTR, CBR and SPLIT (the 32x8 list is "used from rk3576") | `format_modifiers_afbc`, 203-238; `format_modifiers_rk3576_afbc`, 240-280 | read from source, 2026-11-28 |
| Width alignment: for an AFBC plane `actual_w` is rounded **down** to a multiple of 4 ("not 4 pixel aligned"). It is silent: no error, the visible width shrinks | `vop2_plane_atomic_update`, 1245-1249 | read from source, 2026-11-28 |
| Superblock width is 16 (or 32 for the 32x8 modifier); the tile count is `ALIGN(actual_w, block_w) / block_w` and `pic_vir_width` is in pixels. No check against `max_input` beyond the generic one | 1275-1300 | read from source, 2026-11-28 |
| No AFBC-only maximum width or height is defined in the three files; the AFBC picture size uses the same `act_info` as linear | 1343 | read from source, 2026-11-28 |
| AFBC header pointer must be the zero offset of the framebuffer: `src.x1`/`src.y1` offsets are not added to the address for AFBC (they go into `PIC_OFFSET`) | 1176-1186, 1345 | read from source, 2026-11-28 |
| `mod_supported`: with RK3588 and `XRGB2101010`/`XBGR2101010` the modifier must be AFBC (linear is refused). Any other modifier must be in the plane's list and its format must convert to an AFBC format (`vop2_convert_afbc_format` >= 0). `DRM_FORMAT_MOD_INVALID` is refused | `rockchip_vop2_mod_supported`, 381-422 | read from source, 2026-11-28 |
| Rotation on AFBC: the Cluster rotation mask is 90, 270, reflect X, reflect Y; Esmart is reflect Y only; reflect Y is handled in the buffer address only when not AFBC | `rk3588_vop_win_data` `.supported_rotations`; `vop2_plane_atomic_update` 1197-1201 | read from source, 2026-11-28 |

AFBC and scaling: no source line forbids scaling an AFBC Cluster plane, and
the scaler of a Cluster is used on the AFBC-decoded data (the `AFBCD` decode
comment at 1308 says all YUV is decoded to YUV444). That scaling works
together with AFBC is not shown by the source (inferred, 2026-11-28).

## 5. Comparison with the NanoPC-T6 observation of 2026-11-27

The hand-off of plan #457 (`scen-t6`) recorded, on kernel
`6.18.55-current-rockchip64` (observed 2026-11-27): 8 planes, zpos 0..7 on
every plane, no plane scales, AFBC only on Cluster planes. This document did
not re-observe that; the statements below only compare it with the source.

| Observation | What v6.18 source says | Agreement | Tag |
|---|---|---|---|
| 8 planes | 4 Cluster + 4 Esmart rows in `rk3588_vop_win_data` | agrees | inferred, 2026-11-28 |
| zpos 0..7 on every plane | `drm_plane_create_zpos_property(..., win->win_id, 0, registered_num_wins - 1)` gives min 0, max 7 for 8 windows; the initial value is `win_id`. A mutable zpos | agrees on the range | inferred, 2026-11-28 |
| AFBC only on Cluster | modifier lists: AFBC on Cluster rows only | agrees | inferred, 2026-11-28 |
| No plane scales | The driver has no scaling property of its own. DRM exposes no "can scale" attribute, so "no plane scales" can only be a result of how the probe derived it. The source accepts 1/8x..8x on every plane. Source and observation disagree about capability unless the observation means "no scaling property exists" | disagrees about capability; agrees about the absence of a property | inferred, 2026-11-28 |
| Planes 39 and 69 cannot reach `video_port0` | In the source `possible_vp_mask` is `BIT(0)..BIT(3)` for all eight windows, so the driver table would let every window reach every VP. `possible_crtcs` is then built only from VPs with a `crtc.port` (device-tree dependent) and the primary plane choice assigns Cluster windows to VPs in order (`rockchip_drm_vop2.c` 2378-2445). A plane that cannot reach VP0 is therefore a device-tree and primary-plane assignment effect, not a table limit | the table does not explain it | inferred, 2026-11-28 |

Reading for the profile (inferred, 2026-11-28): the probe's "cannot scale"
for every plane is a probe-side default, not a hardware or driver limit.
A `DriverProfile::Rk3588Vop2` should carry scaling per class (Cluster 4x,
Esmart 8x) and should be validated with TEST_ONLY runs; those are the sweep
items of the same plan and are not part of this document.

## 6. Hand-over: what changes in `docs/rk3588-backend-design.md` §4

For the owner of that doc (this item does not edit it):

- `max_source`: 4096 x 4320 per mainline v6.18, not 4096 x 7680; no
  per-class difference in code (read from source, 2026-11-28).
- Scaling: the check allows 1/8..8 on every window; tables say Cluster 4x,
  Esmart 8x, unused by the code; use the table values (inferred, 2026-11-28).
- No scaler `ResourceGroup` is suggested by these files either: each window
  writes its own scale registers (read from source, 2026-11-28).
- AFBC: width rounded down to a multiple of 4, no error (read from source,
  2026-11-28).
