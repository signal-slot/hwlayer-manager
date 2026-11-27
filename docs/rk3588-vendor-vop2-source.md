# RK3588 VOP2: Rockchip vendor kernel 6.1 compared with mainline 6.18

Read 2026-11-28. No board was used and no code was run.

Tags: **read from source** is a statement of the cited file and function.
**inferred** is a comparison, a conclusion from several rows, or anything that
rests on a model summary instead of verbatim text. Every row is dated
2026-11-28 (the read date) unless it repeats a board fact with its own date.

## Pinned refs

- Vendor: rockchip-linux/kernel, branch `develop-6.1`, commit
  `77168c8d5ab82399f65a80e9f807b50ba37cf483`:
  https://github.com/rockchip-linux/kernel/tree/77168c8d5ab82399f65a80e9f807b50ba37cf483/drivers/gpu/drm/rockchip
- Mainline: tag `v6.18` of torvalds/linux, as pinned and cited line by line in
  `docs/rk3588-mainline-source.md`:
  https://github.com/torvalds/linux/tree/v6.18/drivers/gpu/drm/rockchip
  Local copies of `rockchip_drm_vop2.c`, `rockchip_vop2_reg.c` and
  `rockchip_drm_vop2.h` of `v6.18` were reused from the dependency cache
  (fetch method 1).
- Abbreviations: `VV2` = vendor `rockchip_drm_vop2.c`, `VREG` = vendor
  `rockchip_vop2_reg.c`, `MV2`/`MREG` = the mainline files of the same names.

## How the vendor files were fetched

| Order | Method | Result, 2026-11-28 |
|---|---|---|
| 1 | Existing copies under the dependency cache | Mainline `v6.18` copies existed and were used. No vendor copy existed. |
| 2 | `git -c credential.helper= -c core.askPass= clone --filter=blob:none --no-checkout` of the vendor repo | Not completed: the sandbox held the clone command for approval, which an unattended run cannot give. Counted as failed here. |
| 3 | The web-fetch subagent on `raw.githubusercontent.com/rockchip-linux/kernel/<sha>/<path>` | **Worked** for `VV2` (620,964 characters) and `VREG` (322,833 characters), HTTP 200. |

Limits of method 3, which weaken every vendor row below:

- The fetch tool returns about 39 KB of verbatim text per call plus a model
  summary of the remainder. `VV2` and `VREG` were read in slices at character
  offsets, so vendor locations are given as `file:function`, not line numbers.
- Vendor `vop2_crtc_atomic_begin`, `vop2_crtc_atomic_flush`,
  `vop2_handle_vblank`, the `rk3588_vop` data struct and the video-port table
  were seen only through a summary. Rows that depend on them say so.
- The last ~85 KB of `VV2` was not read.
- Not read at all: `rockchip_drm_vop.h`, `rockchip_vop_reg.h` (the older VOP
  driver; no VOP2 fact below depends on them) and vendor
  `rockchip_drm_vop2.h`. The vendor VOP2 register and plane table file is
  `rockchip_vop2_reg.c`.

## 1. Window list and plane count

Read from source (vendor, 2026-11-28):

- `VREG:rk3588_vop_win_data[]` has 12 windows: Cluster0..3 each with a
  `CLUSTER_MAIN` entry (`win0`) and a `CLUSTER_SUB` entry (`win1`), plus
  Esmart0..3 (`win0`, `WIN_FEATURE_MULTI_AREA`).
- Every entry has `.possible_vp_mask` of VP0..VP3.
- Plane types in the table: Cluster entries `DRM_PLANE_TYPE_OVERLAY`, Esmart
  entries `DRM_PLANE_TYPE_PRIMARY`.
- `rk3588_vop` data: `.nr_vps = 4`, `.nr_mixers = 7`, `.nr_layers = 8`,
  `.nr_dscs = 2`, `.feature = VOP_FEATURE_SPLICE` (summary, not verbatim).
- `VREG:rk3588_vop_video_ports[]` (summary): VP0 `max_output` 7680x4320, VP1
  and VP2 4096x2304, VP3 2048x1536.
- Splice pairs (`.splice_win_id`): Cluster0+1, Cluster2+3, Esmart0+1,
  Esmart2+3.

Read from source (mainline, `docs/rk3588-mainline-source.md` §2):

- `MREG:rk3588_vop_win_data[]` has 8 entries (Cluster0..3 `PRIMARY`, Esmart0..3
  `OVERLAY`); there is no `win1` entry and no splice code. Masks are VP0..VP3
  for all. VP0..2 `max_output` 4096x2304, VP3 2048x1536.

Inferred:

- Plane count: 12 vendor table windows against 8 mainline. Whether the vendor
  tree registers all 12 as planes depends on the DT `support-multi-area` flag;
  `docs/observed/rk3588-source.md` says sub windows get no plane without it.
  This read did not re-verify that gate. The vendor default would be 8 planes,
  12 with the flag.
- VP assignment: both tables give every window reach to every VP. The vendor
  Primary candidates are the Esmart windows; mainline binds Cluster *n* as the
  Primary of the *n*-th active port. Per-CRTC reach on mainline is therefore
  narrower than the table, which fits the NanoPC-T6 fact that planes 39 and 69
  cannot reach `video_port0` (observed 2026-11-27, kernel
  6.18.55-current-rockchip64).
- VP0 7680x4320 output exists only in the vendor tables. It is a port limit,
  not a plane limit.

## 2. Scaling

| Item | Vendor 6.1 | Mainline 6.18 |
|---|---|---|
| Hardware factors in the table | Cluster 4x up and down, Esmart 8x up and down (`VREG:rk3588_vop_win_data[]`, `max_upscale_factor`, `max_downscale_factor`) | same values (`MREG`, `docs/rk3588-mainline-source.md` §4) |
| Enforced by the atomic check | `VV2:vop2_plane_atomic_check`: `min_scale = FRAC_16_16(1, 8)`, `max_scale = FRAC_16_16(8, 1)` when `win->regs->scl`, else `DRM_PLANE_NO_SCALING`, passed to `drm_atomic_helper_check_plane_state`. The 4x Cluster figure was not seen enforced. | `MV2:vop2_plane_atomic_check`: same 1/8..8 for all windows, no per-window branch; table factors unused |
| Minimum size | source or destination under 4x4: plane made invisible, return 0 | same |
| Filters | `VOP2_SCALE_UP_BIC` horizontal up, bilinear otherwise, same on all entries | Cluster writes no filter registers; Esmart does |
| Down-scale quirks | `VV2:vop2_setup_scale`: `ygt4`/`ygt2` vertical shortcuts; RK3568-only even-`dst_w` rule | Esmart odd-`dst_w` and `% 16 == 1` crop rules apply without a version test |
| Splice scale check | `VV2:vop2_cluter_splice_scale_check`: sources over 4096 wide scaling down need a centred destination and a horizontal factor under 1.2 | none |

All rows read from source, 2026-11-28 (vendor from verbatim slices).

Inferred:

- The enforced range is the same on both trees: 1/8..8 wherever a scaler
  exists. The vendor splice check is the only extra scale limit and applies to
  8K only.
- Neither tree checks 4x on Cluster, so `TEST_ONLY` cannot be expected to
  refuse a 5x..8x Cluster scale; only a picture settles it. This agrees with
  `docs/rk3588-mainline-source.md` §4.

## 3. Rotation and AFBC transforms

| Item | Vendor 6.1 | Mainline 6.18 |
|---|---|---|
| Cluster main window rotations | `ROTATE_90`, `ROTATE_270`, `REFLECT_X`, `REFLECT_Y` (`VREG:rk3588_vop_win_data[]`) | same four bits |
| Cluster sub window rotations | `REFLECT_X`, `REFLECT_Y` only | no sub window |
| Esmart rotations | `REFLECT_Y` only | `REFLECT_Y` only |
| Rotation tied to buffer type | `VV2:vop2_plane_atomic_check`: a non-AFBC Cluster plane refuses `REFLECT_X`, `ROTATE_90`, `ROTATE_270` ("Unsupported linear rotation"); a tiled Cluster plane refuses all mirror and rotate; 90 and 270 together refused | no such check (`docs/rk3588-mainline-source.md` §5) |
| Other refusals | Cluster sub window: main and sub must share the modifier and `act_w + xoffset % block_w <= 2048`; splice mode refuses 90/270 and `REFLECT_X` (`VV2:vop2_plane_splice_check`) | none |
| AFBC programming | `VV2:vop2_win_atomic_update`: AFBC mirror and rotate bits set; stride must be 64-pixel aligned with xmirror or rotation; `vop2_afbc_transform_offset` per-case offsets; half-block mode off for 90/270 (`vop2_afbc_half_block_enable`); `vop2_get_cluster_lb_mode` returns 2 for 90/270 | same helpers, without the stride message |

All rows read from source, 2026-11-28.

Inferred:

- The vendor rule "non-AFBC Cluster rejects X-mirror and rotation" that
  `docs/rk3588-backend-design.md` §4 records is confirmed for the vendor tree.
  On mainline it cannot be reached through a modifier-aware client because the
  Cluster modifier list has no linear (section 4). The transform set per class
  is the same on both trees: Cluster rotate and reflect, Esmart `REFLECT_Y`.
  Sub-window and splice limits are vendor-only.
- Mainline has no stride check, so its `TEST_ONLY` may pass an AFBC rotation
  that the vendor tree refuses for stride alignment.

## 4. Formats and modifiers

| Item | Vendor 6.1 | Mainline 6.18 |
|---|---|---|
| Cluster format list | `VREG:formats_for_cluster[]`, 14 entries: XRGB2101010, XBGR2101010, XRGB8888, ARGB8888, XBGR8888, ABGR8888, RGB888, BGR888, RGB565, BGR565, then YUV420_8BIT, YUV420_10BIT, YUYV, Y210 ("non-linear mode only") | `MREG:formats_cluster`, same 14 |
| Esmart format list | `VREG:formats_for_esmart[]`, 21 entries: XRGB8888, ARGB8888, XBGR8888, ABGR8888, RGB888, BGR888, RGB565, BGR565, NV12, NV21, NV16, NV61, NV24, NV42, NV15, NV20, NV30, YVYU, VYUY, YUYV, UYVY; no 10-bit RGB | `MREG:formats_esmart`, same 21 |
| Cluster modifiers | `format_modifiers_afbc[]`: AFBC 16x16 variants plus linear | `format_modifiers_afbc`: nine AFBC 16x16 variants, no linear |
| Esmart modifiers | `format_modifiers[]`: linear | same |
| Per-pair rule | `VV2:rockchip_vop2_mod_supported`: invalid modifier refused; on RK3588 XRGB2101010 and XBGR2101010 need AFBC; Y210 needs AFBC or RFBC; linear otherwise passes; Cluster-must-be-AFBC applies to RK3568 only | same logic (`MV2:rockchip_vop2_mod_supported`) |
| Linear YUV | `VV2:vop2_linear_yuv_format_check` rounds the source offset up to 2, 4 or 8 pixels with a `DRM_WARN` (a rewrite, not a refusal); linear YUV on Cluster refused | odd-x YUV source refused (`-EINVAL`) |

All rows read from source, 2026-11-28. The vendor "plus linear" on Cluster
comes from the fetch tool's description of `format_modifiers_afbc[]` and so
rests on a summary.

Inferred:

- The format lists are identical; the modifier lists differ by one entry
  (linear on Cluster). A profile built from the vendor tree over-promises
  linear on Cluster for a mainline kernel.

## 5. Maximum source size

| Item | Vendor 6.1 | Mainline 6.18 |
|---|---|---|
| Table value | `VREG` per window: Cluster main `{ 4096, 7680 }`, Cluster sub `{ 2048, 7680 }`, Esmart `{ 4096, 7680 }` | `MREG:rk3588_vop`: `.max_input = { 4096, 4320 }` for every window |
| Check | `VV2:vop2_plane_atomic_check`: source over the window's `input_width_prop`/`input_height_prop` maximum is `-EINVAL` ("Invalid source"); in splice mode (`mode->hdisplay > 4096`) the width limit is doubled | source over `max_input` is `-EINVAL` |
| `max_output` | not used in the plane check | not used in the plane check |

All rows read from source, 2026-11-28.

Inferred: the vendor limit is 4096 wide by 7680 high (8192 wide when
spliced); mainline is 4096x4320. For the NanoPC-T6 (mainline-based kernel) a
`max_source` of 4096x4320 is right; 4096x7680 matches only the vendor tree.

## 6. Pending-commit behaviour

Read from source (2026-11-28):

- Vendor `VV2:vop2_cfg_done` writes the per-VP done bit
  (`RK3568_VOP2_GLB_CFG_DONE_EN | BIT(vp->id) | (BIT(vp->id) << 16)` on
  RK3588). `vop2_vp_done_bit_status` treats a cleared done bit as "current
  frame took effect"; `vop2_wait_for_fs_by_done_bit_status` polls with a
  100 ms timeout; port mux and layer config waits are 50 ms.
- Vendor `vop2_crtc_atomic_flush` calls `vop2_cfg_update`, `vop2_cfg_done`,
  then `vop2_wait_for_irq_handler` (up to 10 ms) and stores `vp->event`
  (summary, not verbatim). Vendor `vop2_handle_vblank` sends the event with
  `drm_crtc_send_vblank_event` (summary).
- Mainline `MV2:vop2_crtc_atomic_flush` runs `vop2_post_config`,
  `vop2_cfg_done`, then under `event_lock` stores `vp->event` after
  `drm_crtc_vblank_get`. `MV2:vop2_isr` on `VP_INT_FS_FIELD` sends the event
  only when the VP's `RK3568_REG_CFG_DONE` bit is clear, then drops the
  vblank reference.

Inferred:

- Both trees deliver the event at the first frame start after the done bit
  clears: one `vp->event` slot per VP, not a queue. A second non-blocking
  commit issued before that event is refused by the DRM core's
  pending-commit handling, not by driver code; neither tree was seen to
  implement replacement. The core semantics are the subject of
  `docs/drm-pending-commit-source.md` (another item); a board run decides.
- The vendor flush adds a bounded wait (up to 10 ms) that mainline lacks.

## 7. What explains "no plane scales" on the NanoPC-T6

Observed (2026-11-27, kernel 6.18.55-current-rockchip64): the probe prints
"scaling not claimed" for all 8 planes (`docs/observed/rk3588-topology.md`),
and the plan #457 handoff records that no plane scales.

Inferred (2026-11-28):

- The probe does not measure scaling. DRM has no scale property; scaling is a
  `SRC_*` versus `CRTC_*` rectangle difference. "Scaling not claimed" is most
  likely the probe printing what it does not test, not a hardware limit.
  `docs/observed/rk3588-topology.md` says the same ("not reported by probe").
- Source gives no refusal. Mainline `vop2_plane_atomic_check` accepts 1/8..8
  on every window, as the vendor tree does, so a 2x scale would pass
  `TEST_ONLY` on a kernel based on either.
- A refusal from combined effects stays possible: the per-CRTC cap of
  `8 / active ports` planes (`MV2:vop2_crtc_atomic_check`) rejects commits
  with too many planes whatever their scale.
- The Armbian `current-rockchip64` kernel is mainline-based but may carry
  patches; not checked.
- The TEST_ONLY scale sweep of the plan's board items settles it.

## 8. Summary against `docs/rk3588-backend-design.md` §4

All inferred, 2026-11-28:

| §4 row | Vendor 6.1 | Mainline 6.18 |
|---|---|---|
| scaling | check 1/8..8, hardware 4x Cluster / 8x Esmart | check 1/8..8 only |
| `max_source` | 4096x7680 | 4096x4320 |
| transforms | Cluster rotate and reflect, AFBC buffers only; Esmart `REFLECT_Y` | no buffer check |
| format set | Cluster AFBC and linear; Esmart linear | Cluster AFBC only; Esmart linear |
| sub windows | `win1`, 2048 wide, shares modifier, reflect only | none |
| plane count | 8 default, 12 with multi-area (not re-verified) | 8 |
| layer budget | `nr_layers = 8`, 7 mixers | `8 / active ports` per CRTC |

For the profile: use the mainline column (the board runs a mainline-based
kernel); keep the vendor column only if a vendor-kernel board is added.

## Not covered

- Vendor line numbers; vendor rows rest on slices and summaries (above).
- Vendor `rockchip_drm_vop.h`, `rockchip_vop_reg.h`, `rockchip_drm_vop2.h`.
- Hardware behaviour. Nothing here says a given scale or modifier works.
