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

## Q1: IN_FORMATS order and the effect of `MAX_FORMATS` = 32 (added 2026-11-28, item `rk-formats-source`)

This section answers RK3588 design question Q1 of `docs/rk3588-backend-design.md`
section 7 (does the 32-pair cap hide AFBC pairs on Cluster planes, and is the
order linear first). **It is the expectation, not a measurement.** The
measurement is the board session `t6-topology-session`
(`docs/rk3588-vop2-session-*.md`), which reads the raw IN_FORMATS blob; until it
lands nothing here is a hardware result.

Sources, all read 2026-11-28:

- Linux commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` (v6.18),
  `drivers/gpu/drm/rockchip/rockchip_vop2_reg.c` (`REG`) and
  `rockchip_drm_vop2.c` (`V2C`): the local copies named in the first section.
- `drivers/gpu/drm/drm_plane.c` of the same commit (`DP`):
  https://github.com/torvalds/linux/blob/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/gpu/drm/drm_plane.c
  Honest limit: this worker could not download it (network refused), so `DP`
  was read through the web-fetch helper, which returned the code text without
  line numbers. The `DP` line numbers below were counted by that helper and are
  good to about 2 lines; the code shape (quoted briefly) is exact.
- composition at this repository's checkout: `crates/composition-drm/src/lib.rs`
  and `crates/composition-core/src/limits.rs`.

### How the kernel orders IN_FORMATS (read from source, `DP`)

- `drm_universal_plane_init` copies the driver's `formats[]` and `modifiers[]`
  verbatim (`DP:451-454`); nothing is sorted or de-duplicated. The modifier
  count is the number of entries before the `DRM_FORMAT_MOD_INVALID` terminator
  (`DP:410-414`); the terminator is not stored and never appears in the blob.
  With no modifier array, the plane gets a single implicit `LINEAR`
  (`DP:372-374`, `415-418`).
- `create_in_format_blob` (`DP:197-260`) writes the format array once
  (`memcpy`, `DP:240`) and then one entry per modifier, in `plane->modifiers[]`
  order: an outer loop over modifiers, an inner loop over formats setting bit
  `j` of a 64-bit mask when `format_mod_supported(plane, format[j], modifier[i])`
  holds (`DP:242-257`). The entry `offset` is always 0 and a plane may have at
  most 64 formats (`DP:386-387`).
- So the blob is **modifier-major in the driver's array order**; the kernel
  never promises linear first. The order of a plane is the order of its two
  arrays in `REG`.

### How composition reads and caps it (read from source, this checkout)

- `parse_in_formats` (`crates/composition-drm/src/lib.rs:336-372`) walks the
  modifier entries in blob order and, inside each, the set mask bits in
  ascending order (`lib.rs:351-368`): the same modifier-major order, one
  `format x modifier` pair per set bit. It does not drop or reorder anything.
- The caller sorts the pairs with `pairs.sort_by_key(|p| p.modifier !=
  Modifier::LINEAR)` (`lib.rs:532`). A stable sort on that key moves linear
  pairs to the front and leaves the rest in blob order (inferred from the
  standard library's documented stable `sort_by_key`). Then
  `.take(MAX_FORMATS)` keeps the first 32 (`lib.rs:533-535`);
  `MAX_FORMATS = 32` is `crates/composition-core/src/limits.rs:15`. The cut is
  documented at `lib.rs:18`.
- Pairs past 32 are not recorded anywhere: they cannot be offered to a layer,
  and the planner never sees them (inferred from the loop above).

### Counts per window class (RK3588 table, `REG:1121-1264`)

| Window class | Format array | Formats | Modifiers | Pairs IN_FORMATS advertises | vs 32 |
|---|---|---|---|---|---|
| Cluster0-3 (4 planes) | `formats_cluster` `REG:58-73` | 14 | 9 AFBC, no linear (`REG:203-238`) | 14 x 9 = 126 | 94 past the cap |
| Esmart0-3 (4 planes) | `formats_esmart` `REG:106-128` | 21 | 1 (`LINEAR`, `REG:198-201`) | 21 x 1 = 21 | fits, 11 spare |
| Smart (RK3568 only) | `formats_smart` `REG:187-196` | 8 | 1 (`LINEAR`) | 8 | not on RK3588: the RK3588 table names no Smart window |

Why every Cluster format is advertised under every modifier (read from source,
`V2C:383-421`, then inferred by matching the lists): `format_mod_supported`
rejects only `MOD_INVALID`, the 10-bit RGB formats without AFBC (RK3588 branch,
`V2C:402-408`), and AFBC modifiers for formats that
`vop2_convert_afbc_format` (`V2C:220-255`) cannot map. All 14 `formats_cluster`
entries map, and the 9 modifiers are all in the plane's own list, so each of the
9 mask bits is set for all 14 formats: 126. The Esmart plane's one `LINEAR`
modifier is accepted for every format (`V2C:411-412`), so 21. These counts
agree with the recorded plane dump (126 and 21, observed 2026-11-27; see the
agreement table above).

### Which pairs fall past the cap (inferred from the two reads above)

Cluster has no linear modifier, so the stable sort changes nothing and the first
32 pairs are modifier-major in `REG:203-238` order:

| Pairs | Kept? | Content |
|---|---|---|
| 1-14 | kept | modifier 1 (16x16, no flags, `0x0800000000000001`) with all 14 formats |
| 15-28 | kept | modifier 2 (SPARSE, `...41`) with all 14 formats |
| 29-32 | kept | modifier 3 (YTR, `...11`) with only its first 4 formats: XRGB2101010, XBGR2101010, XRGB8888, ARGB8888 |
| 33-126 | **cut (94 pairs)** | modifier 3 with the other 10 formats (XBGR8888 .. Y210); modifiers 4-9 (CBR `...81`, YTR+SPARSE `...51`, CBR+SPARSE `...c1`, YTR+CBR `...91`, YTR+CBR+SPARSE `...d1`, YTR+SPARSE+SPLIT `...71`) with all formats |

Consequences (inferred):

- **Yes, `MAX_FORMATS` = 32 hides AFBC pairs on Cluster planes**: 94 of 126, and
  six of the nine modifiers never appear at all. Every format still appears
  under at least the first two modifiers, so no *format* is lost, only modifier
  variants (the 10 formats that lose their YTR pair keep plain and SPARSE).
- The comment at `REG:232` says RGB AFBC modes mandate YTR. If a producer's
  RGB AFBC buffer carries a YTR modifier, only 4 formats of it are visible to
  composition; for the others the planner would see the plane as unable to take
  that buffer (a lost choice, never a wrong claim). Which modifier a given
  producer emits is not known from this read.
- **Linear first** holds on RK3588 only by construction, not by a kernel
  guarantee: Cluster has no linear pair, Esmart has nothing else, and
  composition's own sort puts linear first in general (`lib.rs:532`). On a plane
  with both, the kernel order would be driver-array order and the sort would
  fix it.
- Esmart planes lose nothing (21 <= 32).
- The recorded probe already showed 32 kept pairs on Cluster, observed 2026-11-27 (as cited in `crates/composition-drm/src/profile/rk3588.rs`); that
  matches the arithmetic here.

Raising `MAX_FORMATS` (to at least 126 to keep everything on RK3588) is a
composition-core decision recorded as a finding; this item changes no code. What
the board session must confirm: the raw blob holds 9 modifier entries in the
order of the nine-row table above, each with 14 bits set; no `0x0` entry on
Cluster; one entry with 21 bits on Esmart; and the offsets are 0.
