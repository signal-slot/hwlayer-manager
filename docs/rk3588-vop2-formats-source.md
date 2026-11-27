# RK3588 VOP2 per-window-class formats, modifiers and transforms, read from mainline 6.18

Written 2026-11-28 for plan #459, item `vop2-formats-source`. No board was used
and no code was run. It fills the format, modifier, transform and property gaps
of `docs/rk3588-backend-design.md` section 4 and compares them with the recorded
NanoPC-T6 data.

## Pinned source and how it was read

- Commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` of torvalds/linux (the
  task names it as Linux v6.18), directory `drivers/gpu/drm/rockchip/`:
  https://github.com/torvalds/linux/tree/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/gpu/drm/rockchip
- `REG` = `rockchip_vop2_reg.c`, `V2C` = `rockchip_drm_vop2.c`.

What this worker could and could not do (all stated, nothing hidden):

- No copy of the kernel was in the dependency cache. The two files were first
  read through the web-fetch helper and then downloaded as raw files from
  `raw.githubusercontent.com` at the commit above (2026-11-28). The downloaded
  copies have 2577 lines (`REG`) and 2793 lines (`V2C`), and every `file:line`
  below was checked against them with `grep -n`/`sed -n`.
- That the pinned commit is Linux v6.18 is taken from the task, not verified
  here. The line numbers agree with `docs/rk3588-mainline-source.md`, which read
  the `v6.18` tag copy.
- Not read: `drm_plane.c` / `drm_framebuffer.c` in DRM core (how the IN_FORMATS
  blob is built and how `format_mod_supported` gates framebuffer creation).
  Statements that depend on it are **inferred**.

Tags: **read from source**, **inferred**, and board facts quoted from recordings
(each with its date, 2026-11-27 or 2026-11-28).

## Window classes in the RK3588 table

Read from source (`REG:1121-1264`, `rk3588_vop_win_data[]`): eight windows,
four Cluster (`Cluster0-win0` .. `Cluster3-win0`, `WIN_FEATURE_AFBDC |
WIN_FEATURE_CLUSTER`, type `PRIMARY`) and four Esmart (`Esmart0-win0` ..
`Esmart3-win0`, no feature flags, type `OVERLAY`). All eight carry
`possible_vp_mask = BIT(0)|BIT(1)|BIT(2)|BIT(3)`. Per-port reach is
`docs/rk3588-dt-source.md`'s subject and is not repeated.

## Format lists

| Class | Array (`REG`) | Formats | Count | Tag |
|---|---|---|---|---|
| Cluster0-3 | `formats_cluster` (`REG:58-73`) | XRGB2101010, XBGR2101010, XRGB8888, ARGB8888, XBGR8888, ABGR8888, RGB888, BGR888, RGB565, BGR565, YUV420_8BIT, YUV420_10BIT, YUYV, Y210. The source comments say the last four are "non-Linear mode only". | 14 | read from source |
| Esmart0-3 | `formats_esmart` (`REG:106-128`) | XRGB8888, ARGB8888, XBGR8888, ABGR8888, RGB888, BGR888, RGB565, BGR565, NV12, NV21, NV16, NV61, NV20, NV24, NV42, NV30, NV15, YVYU, VYUY, YUYV, UYVY. No 10-bit RGB. | 21 | read from source |

Other format arrays in `REG` (`formats_rk3576_cluster`, `formats_rk356x_esmart`,
`formats_rk3576_esmart`, `formats_smart`) are not referenced by the RK3588 window
table (read from source, `REG` window entries name only `formats_cluster` and
`formats_esmart`).

## Modifier lists

| Class | Array (`REG`) | Content | Tag |
|---|---|---|---|
| Cluster0-3 | `format_modifiers_afbc` (`REG:203-238`) | Nine Arm AFBC modifiers, all `AFBC_FORMAT_MOD_BLOCK_SIZE_16x16`, then `DRM_FORMAT_MOD_INVALID` as terminator. **No `DRM_FORMAT_MOD_LINEAR` and no 32x8 block size.** | read from source |
| Esmart0-3 | `format_modifiers` (`REG:198-201`) | `DRM_FORMAT_MOD_LINEAR`, then `DRM_FORMAT_MOD_INVALID`. | read from source |

The nine Cluster modifiers in array order (read from source, `REG:203-238`),
with their numeric value computed from the Arm AFBC encoding (flag bits
YTR 0x10, SPLIT 0x20, SPARSE 0x40, CBR 0x80, block 16x16 = 1, vendor Arm =
0x08 << 56; the numeric values are **inferred** from that encoding):

| # | Flags (besides 16x16) | Value |
|---|---|---|
| 1 | none | `0x0800000000000001` |
| 2 | SPARSE | `0x0800000000000041` |
| 3 | YTR | `0x0800000000000011` |
| 4 | CBR | `0x0800000000000081` |
| 5 | YTR, SPARSE | `0x0800000000000051` |
| 6 | CBR, SPARSE | `0x08000000000000c1` |
| 7 | YTR, CBR | `0x0800000000000091` |
| 8 | YTR, CBR, SPARSE | `0x08000000000000d1` |
| 9 | YTR, SPARSE, SPLIT | `0x0800000000000071` |

The source comment at `REG:232` reads "SPLIT mandates SPARSE, RGB modes
mandates YTR", which is why SPLIT appears only once and with YTR and SPARSE.
Block size, YTR, SPLIT, SPARSE and CBR are therefore all advertised; **32x8 is
not**, although `vop2_plane_atomic_update` has a branch for it (see below).

## Rotations and reflections

Read from source (`REG:1131-1132`, `1150-1151`, `1169-1170`, `1188-1189` for
Cluster; `REG:1207`, `1224`, `1241`, `1258` for Esmart):

| Class | `supported_rotations` | Resulting KMS values | Needs AFBC? |
|---|---|---|---|
| Cluster0-3 | `ROTATE_90 \| ROTATE_270 \| REFLECT_X \| REFLECT_Y` | rotate-0, 90, 180 (both reflects), 270, and the reflect bits | no check in code (below) |
| Esmart0-3 | `REFLECT_Y` | rotate-0 and reflect-Y only; no 90, 180, 270, no reflect-X | no AFBC exists on Esmart |

The property is created only if the window has bits and with `ROTATE_0 |
supported_rotations` (`V2C:2295-2298`). 180 degrees is available on Cluster as
both reflect bits (inferred from the bit set; the kernel's rotation helpers
treat 180 as REFLECT_X|REFLECT_Y).

Where rotation is applied (read from source):

- Linear buffers: the Y mirror moves the start line (`V2C:1185`, `1200`).
- AFBC buffers: the AFBC rotate and mirror registers (`VOP2_WIN_AFBC_ROTATE_90`,
  `..._270`, `VOP2_WIN_XMIRROR`) are written only inside the `if (afbc_en)` block
  (`V2C:1275`, `1350-1351`); the AFBC transform offset is computed by
  `vop2_afbc_transform_offset` (`V2C:436-503`).
- 90/270 degrees: `vop2_get_cluster_lb_mode` returns 2 (`V2C:516-527`) and
  `vop2_half_block_enable` returns false (`V2C:424-431`).
- A stride not aligned to 64 pixels with mirror or rotation produces only a
  `drm_dbg_kms` message, not an error (`V2C:1293-1296`).

**No rotation or reflection is tied to the modifier anywhere in the check
path** (read from source: `vop2_plane_atomic_check` `V2C:992-1061` and
`rockchip_vop2_mod_supported` `V2C:383-422` contain no rotation test). The
"only for AFBC buffers" rule in the design doc is therefore not enforced by
this code (inferred conclusion from the absence of such a test).

## Per-class zpos, alpha and blend properties

Read from source, all in `vop2_plane_init` (zpos `V2C:2301-2302`, rotation `V2C:2295-2298`; called from
`vop2_create_crtcs`):

- zpos: `drm_plane_create_zpos_property(&win->base, win->win_id, 0,
  vop2->registered_num_wins - 1)`: initial value is the window index in the
  table, range 0 .. (window count - 1), i.e. 0..7 on RK3588. `win_id` is the
  table index (`vop2_win_init`, `V2C:2564`).
- alpha: `drm_plane_create_alpha_property`.
- blend mode: `drm_plane_create_blend_mode_property` with `PIXEL_NONE |
  PREMULTI | COVERAGE`.
- No colour-encoding or colour-range property: `drm_plane_create_color_properties`
  is never called in `V2C`.
- None of these calls branches on Cluster versus Esmart or on a feature flag.
  The only per-class property difference is the rotation property above.

## How the plane check rejects a format, modifier or transform

Read from source (function, then lines of the tag copy):

`vop2_plane_atomic_check` (`V2C:992-1061`):

1. `min_scale = FRAC_16_16(1, 8)`, `max_scale = FRAC_16_16(8, 1)` for **every**
   window (`V2C:1004-1005`); `drm_atomic_helper_check_plane_state(..., true,
   true)` (`V2C:1020-1022`).
2. Invisible plane: return 0.
3. `vop2_convert_format(fb->format->format)`; a negative result is returned as
   the error (`V2C:1029-1031`). Format support at this stage is the hardware
   code mapping, not the per-class `formats[]` list (that list is enforced
   earlier by DRM core at framebuffer and plane-state level, **inferred**).
4. Source or destination smaller than 4x4: message, plane made invisible, return
   0 (`V2C:1033-1040`).
5. Source larger than `max_input` (4096x4320, `REG:2525`): `-EINVAL`
   (`V2C:1042-1051`).
6. YUV format with odd source x: `-EINVAL` (`V2C:1054-1059`).

There is **no** modifier test and **no** rotation test in this function.

`rockchip_vop2_mod_supported` (`V2C:383-422`: invalid `V2C:389`, RK3568
cluster `V2C:392-400`, RK3588 10-bit `V2C:402-408`, linear `V2C:410`, AFBC list
`V2C:413-419`, convert `V2C:421`; the `format_mod_supported` callback in
`vop2_plane_funcs` is `V2C:1417`; helpers `rockchip_afbc` and `vop2_convert_afbc_format`):

1. `DRM_FORMAT_MOD_INVALID` -> false.
2. On `VOP_VERSION_RK3568` a Cluster window rejects `DRM_FORMAT_MOD_LINEAR`
   ("Cluster window only supports format with afbc"). This branch is not
   taken on RK3588.
3. On `VOP_VERSION_RK3588`, `XRGB2101010` / `XBGR2101010` with a non-AFBC
   modifier -> false ("Only support 32 bpp format with afbc").
4. `DRM_FORMAT_MOD_LINEAR` -> true.
5. A modifier not in the plane's `modifiers[]` (`rockchip_afbc` is false for
   LINEAR and for any value missing from the list) -> false ("Unsupported
   format modifier").
6. Otherwise true iff `vop2_convert_afbc_format(format) >= 0`. That function maps
   the 2101010, 8888, RGB888/BGR888, RGB565/BGR565, YUV420_8BIT, YUV420_10BIT,
   YVYU/YUYV/VYUY/UYVY and Y210 formats to AFBC format codes, and everything
   else to `VOP2_AFBC_FMT_INVALID`. Every Cluster format in `formats_cluster`
   maps, so all 14 formats pass for every listed AFBC modifier (inferred by
   matching the two lists).

Atomic update details that matter to a planner (read from source,
`vop2_plane_atomic_update`, AFBC block from `V2C:1275`):

- `afbc_en = rockchip_afbc(plane, fb->modifier)`; block width 32 if the
  modifier has `AFBC_FORMAT_MOD_BLOCK_SIZE_32x8`, else 16 (a dead branch for
  the advertised list); YTR sets bit 4 of the AFBC format; SPLIT sets
  `VOP2_WIN_AFBC_BLOCK_SPLIT_EN`; **SPARSE is not referenced** by the update
  code, so it changes only which modifier the buffer carries.
- AFBC source width is rounded down to a multiple of 4 (`V2C:1245-1249`).
- Cluster windows return early from `vop2_setup_scale` before filter-mode writes
  (`V2C:619-620`); the scale limits themselves are in `docs/rk3588-mainline-source.md`
  section 4 and are out of scope here.

Two inferred consequences:

- A rotate or reflect request on an Esmart plane other than REFLECT_Y fails
  because the property does not offer the bit (atomic ioctl property validation
  in DRM core, **inferred**; not a check in this driver).
- A linear buffer on a Cluster window: `rockchip_vop2_mod_supported` returns
  true for LINEAR on RK3588 (step 4), but the modifier is not in the Cluster
  modifier array and so not in IN_FORMATS. Whether DRM core lets such a
  framebuffer be created depends on code not read here (**inferred, open**).
  The board observation below says linear scanout works on `Cluster0-win0`.

## Agreements and differences with `docs/rk3588-backend-design.md` section 4

| Design doc §4 row | This reading | Verdict |
|---|---|---|
| Esmart `transforms`: reflect Y only | `REG:1207` etc.: `REFLECT_Y` only | agrees |
| Cluster `transforms`: rotate 90/270 and reflect X/Y | `REG:1131-1132` etc.: same four bits | agrees |
| "only for AFBC buffers; none for linear and none for tiled" | no check in `vop2_plane_atomic_check` or `rockchip_vop2_mod_supported`; the AFBC-only rule is the vendor 6.1 note | **differs** for mainline 6.18 (inferred: the rule does not exist in this code) |
| Cluster format set: AFBC combinations + linear (no linear YUV, no linear 10-bit RGB) | `format_modifiers_afbc` has no LINEAR (`REG:203-238`); `mod_supported` allows LINEAR for 8888/RGB/YUV (step 4) | **differs**: the advertised Cluster list has no linear pair; the "no linear 10-bit RGB" rule is source-true (step 3) |
| Esmart format set: linear only (no 10-bit RGB) | `formats_esmart` 21 formats, `format_modifiers` = LINEAR | agrees |
| `scaling`: Cluster 4x, Esmart 8x; kernel check 1/8..8 | 4 and 8 are table fields (`REG:1136-1137`, `1212-1213`), kernel check is 1/8..8 for all (`V2C:1004-1005`) | agrees on the figures (use is covered in `docs/rk3588-mainline-source.md` section 4) |
| `max_source` 4096 x 7680 | `max_input` is `{ 4096, 4320 }` (`REG:2525`), the only size compared with the source (`V2C:1042-1051`) | **differs**: 4320, not 7680, on this tree |
| Layer budget: 8 windows, inferred | 8 table entries, zpos range 0..7 | agrees (the budget being reachable is still unmeasured) |
| Port capability VP0 7680x4320, VP1/VP2 4096x2304, VP3 2048x1536 | `rk3588_vop_video_ports[]` `max_output`: VP0-VP2 `{4096, 2304}`, VP3 `{2048, 1536}` (`REG:1057-1090`) | **differs**: VP0 is 4096x2304 here (inferred: the design doc quotes the vendor tree) |
| Sub-window pairing, 2048 width | each Cluster has one window `win0` in the mainline table; no `win1` entries | **differs**: no sub-windows in mainline 6.18 |

## Agreements and differences with the recorded IN_FORMATS (2026-11-27)

Recorded 2026-11-27, kernel 6.18.55-current-rockchip64
(`crates/composition-drm/tests/data/rk3588/props-2026-11-27.txt`, and
`docs/observed/rk3588-topology.md`, 2026-11-28). The observed data are the
reference for the board; the following compares them with the source text.

| Item | Source (this doc) | Observed | Verdict |
|---|---|---|---|
| Cluster formats | 14, list above | 14: XR30 XB30 XR24 AR24 XB24 AB24 RG24 BG24 RG16 BG16 YU08 YU10 YUYV Y210 (observed 2026-11-27) | agrees, same order |
| Cluster modifiers | 9 AFBC, order 01, 41, 11, 81, 51, c1, 91, d1, 71 (computed) | 9 modifiers in that exact order, no `0x0` (observed 2026-11-27) | agrees, which settles the flag decoding for the three the earlier note called inferred |
| Cluster pairs | 14 x 9 = 126 | 126 (observed 2026-11-27) | agrees |
| Esmart | 21 formats, LINEAR only | 21 formats, 1 modifier `0x0` (observed 2026-11-27) | agrees |
| zpos | range 0..(8-1) | `zpos` range 0..=7 on every plane (observed 2026-11-27) | agrees |
| alpha, blend | alpha and three blend modes on every plane | `alpha` 0..=65535; blend {None, Pre-multiplied, Coverage} on every plane (observed 2026-11-27) | agrees |
| Rotation | Cluster four bits, Esmart `REFLECT_Y` | only the current value (`rotate-0`) was recorded, not the bitmask (observed 2026-11-27) | **not measured**; the source figures stay **read from source** |
| Linear on Cluster | not in the modifier array | debugfs showed the console framebuffer scanning out of `Cluster0-win0` with modifier `0x0` (observed 2026-11-28) | **differs** from the advertised list; consistent with `mod_supported` allowing LINEAR (inferred) |
| Plane type | Cluster `PRIMARY`, Esmart `OVERLAY` in the table | planes 33 and 39 Primary, 45 and 51 Overlay among the Clusters (observed 2026-11-28) | **differs**: the driver turns Cluster2/3 into overlays when it assigns primaries (read in `docs/rk3588-dt-source.md`) |

## What a profile can take from this

All inferred from the tables above:

- Cluster: transforms = rotate 90/270, reflect X/Y; Esmart: reflect Y. No per
  modifier split is needed because the driver has none.
- The AFBC-only transform rule is a vendor-kernel rule, not a 6.18 one. A
  profile that wants it must treat it as a conservative choice, not as a kernel
  fact.
- Format sets per class are exactly the two arrays above; the Cluster set is
  AFBC-only as advertised, with 10-bit RGB AFBC-only even if linear is accepted
  elsewhere.
- `max_source` 4096x4320 for both classes on this tree.
- Open: whether a linear buffer is created and scanned out on Cluster for
  non-10-bit formats on a modifier-aware client, and the real rotation bitmask.
  Both need a raw property dump or a TEST_ONLY run (plan items `t6-sweep`,
  `t6-verify`).
