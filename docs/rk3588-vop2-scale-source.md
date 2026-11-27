# RK3588 VOP2 scaling per window class, read from mainline Linux 6.18

Read 2026-11-28. No board was used and no code was run.

Tags: **read from source** is a quotation or paraphrase of the file and line
cited at the pin below. **inferred** is every conclusion drawn from more than
one row, from outside the cited files, or from a board fact. **observed** is
used only for the NanoPC-T6 fact repeated from the plan #457 scen-t6 handoff
(observed 2026-11-27, kernel 6.18.55-current-rockchip64); this document
produces none.

This document narrows `docs/rk3588-mainline-source.md` section 4 to the
per-class question. The line-buffer and source-width limits are not repeated
here: `docs/rk3588-vop2-limits-source.md` is meant to cover them but is not in
this tree yet, so that item's output is the place to look.

## Pin

- Mainline tag `v6.18` of torvalds/linux (tag object
  `f7b88edb52c8dd01b7e576390d658ae6eef0e134`):
  https://github.com/torvalds/linux/tree/v6.18/drivers/gpu/drm/rockchip
- The plan names commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449`. The copies
  read here are the `v6.18` tag files from the project dependency cache, as
  for `docs/rk3588-mainline-source.md`. Inferred: they are the same tree as
  that commit; this was not checked against the commit id.
- `V2C` = `rockchip_drm_vop2.c`, `REG` = `rockchip_vop2_reg.c`,
  `HDR` = `rockchip_drm_vop2.h`, all under `drivers/gpu/drm/rockchip/`.
  `drm_atomic_helper_check_plane_state` lives in `drivers/gpu/drm/` and was
  not in the cache; what is said of it is inferred.

## 1. What the kernel enforces: one rule for both classes

Read from source:

- `vop2_plane_atomic_check` starts at `V2C:992`. It sets
  `min_scale = FRAC_16_16(1, 8)` and `max_scale = FRAC_16_16(8, 1)`
  (`V2C:1004-1005`) and passes them to
  `drm_atomic_helper_check_plane_state(pstate, cstate, min_scale, max_scale,
  true, true)` (`V2C:1020-1022`). No branch looks at the window, so Cluster
  and Esmart get the same bounds.
- The per-window fields `max_upscale_factor` and `max_downscale_factor`
  are declared at `HDR:200-201`. The RK3588 table sets 4 and 4 on Cluster0-3
  (`REG:1136-1137`, `1156-1157`, `1175-1176`, `1194-1195`) and 8 and 8 on
  Esmart0-3 (`REG:1212-1213`, `1229-1230`, `1246-1247`, `1263-1264`).
  A search of `V2C` finds no reader of either field, so the numbers are
  never compared with a request.
- `max_input` of the RK3588 is 4096x4320 (`REG:2525`) and is compared with the
  source rectangle for every window alike (`V2C:1042-1051`, `-EINVAL`).
  That limit does not depend on whether the plane scales.
- A source or destination below 4x4 makes the plane invisible with an error
  message and returns 0 (`V2C:1033-1040`).

Inferred (the helper is not in the cache): in the helper, `min_scale` and
`max_scale` are source-over-destination ratios in 16.16. `max_scale` 8 allows
a down-scale by 8 (source 8 times the destination); `min_scale` 1/8 allows an
up-scale by 8. A rectangle outside them returns `-ERANGE` from the check.
So the kernel limit on both classes is up 8x and down 8x, per axis.

## 2. What the hardware tables say per class

Read from source:

| | Cluster0-3 | Esmart0-3 |
|---|---|---|
| `max_upscale_factor` / `max_downscale_factor` | 4 / 4 (`REG:1136-1137` and the three after it) | 8 / 8 (`REG:1212-1213` and the three after it) |
| Used by the driver | no | no |
| `feature` | `WIN_FEATURE_AFBDC \| WIN_FEATURE_CLUSTER` (`REG:1140`, `1159`, `1178`, `1197`) | none (no `.feature` line in `REG:1199-1266`) |
| Scale registers | Y/RGB factor X/Y, hor/ver mode, GT2/GT4, `BIC_COE_SEL` in the cluster control words (`REG:304-310`) | the same plus a separate CBCR factor and per-axis filter-mode fields (`REG:376-390`) |
| Filter (comment) | bicubic for horizontal scale-up, bilinear otherwise (`REG:1101`) | nearest, bilinear or bicubic up; nearest, bilinear or average down (`REG:1103-1104`) |
| Filter registers written | none: `vop2_setup_scale` returns at `V2C:619-620` | `YRGB_HSCL/VSCL_FILTER_MODE` at `V2C:622-623`, plus the CBCR pair for YUV (`V2C:653-654`) |

The Cluster `.reg = 0xffffffff` entries for CBCR scale, CBCR modes and filter
modes (`REG:338-345`) mark those fields as absent on the Cluster register map.

Inferred: the 4x and 8x numbers are the hardware design limits, kept in the
table for a future check. On 6.18 a Cluster window asked for 5x to 8x passes
the atomic check, so only the hardware or a `TEST_ONLY` plus a picture can say
what such a request does. The safe planner figure for Cluster stays 4x
(inferred), and 8x for Esmart.

## 3. Scale-factor and vertical-skip arithmetic

Read from source:

- `vop2_scale_factor(src, dst)` (`V2C:530-558`): equal sizes give 0;
  `dst < 2` gives `U16_MAX`; `src < 2` gives 0. Otherwise the shift is 12
  for a down-scale and 16 for an up-scale, `src` and `dst` are each reduced
  by 1, and the factor is `DIV_ROUND_UP(src << shift, dst) - 1` clamped to
  `U16_MAX`. The factor register is 16 bits wide on both classes
  (`REG:304-305`, `REG:376-377`).
- Vertical down-scale: if `src_h >= 4 * dst_h`, `GT4` is set and `src_h` is
  divided by 4; else if `src_h >= 2 * dst_h`, `GT2` is set and it is halved
  (`V2C:575-581`). The factor is then computed from the reduced height, and
  `GT2`/`GT4` are written for the Y/RGB channel (`V2C:613-614`). Cluster has
  these fields too (`REG:309-310`), Esmart has them at `REG:389-390`.
- Horizontal and vertical mode come from `scl_get_scl_mode` (`V2C:583-584`).
  The function is not defined in the three cached files.

Inferred: with the 1/8..8 check, a vertical down-scale of up to 8x reduces by
`GT4` or `GT2` first, so the 12-bit-shift factor stays inside 16 bits.
Horizontal 8x down gives about `8 * 4096 = 32768`, and an 8x up gives about
`65536 / 8`, both inside 16 bits. The `dst < 2` case that returns `U16_MAX`
is excluded by the 4x4 minimum (`V2C:1033-1040`). No overflow path was found
for either class within 1/8..8.

## 4. Source-width rules that apply only when scaling

The plain line-buffer width limits are out of scope (see the top). The rules
below fire only on a scaled plane, or differ by class.

Read from source:

- Esmart only (`!WIN_FEATURE_AFBDC`), scaled down in width, source width
  `& 0xf == 1`: the source width is reduced by 1 (`V2C:1233-1243`, comment:
  "esmart can't support scale down when actual_w % 16 == 1").
- Esmart only, horizontal down-scale with odd destination width: `dst_w` is
  incremented for the factor computation (`V2C:598-606`).
- AFBC source (Cluster): width rounded down to a multiple of 4
  (`V2C:1245-1249`). This applies with or without scaling.
- A destination running past the mode is clipped, `dsp_w`/`dsp_h` floored at
  4 and the source rescaled by the same ratio (`V2C:1211-1233`), so a
  scaled plane at a screen edge changes its programmed ratio.
- With 90/270 rotation the source width and height swap before the scale
  factors are computed (`V2C:1363-1367`), and a Cluster window uses
  `CLUSTER_LB_MODE` 2 (`vop2_get_cluster_lb_mode`, `V2C:520-528`; written at
  `V2C:1399`).

Inferred: the first two rules are why a layer whose source width is `16n+1`
and scales down is better placed on a Cluster plane when it can go there.
Neither rule returns an error, so the planner needs them only to explain a
rounded result, not to refuse a layer.

## 5. YUV chroma scaling

Read from source:

- Chroma scale is programmed only for Esmart. After the Cluster early return
  (`V2C:619-620`), `if (info->is_yuv)` computes `cbcr_src_w = src_w / hsub`
  and `cbcr_src_h = src_h / vsub` (`V2C:625-628`), re-runs the GT2/GT4 test
  on the chroma height (`V2C:630-638`), derives its own horizontal and
  vertical modes (`V2C:640-641`), and writes the CBCR factor, GT bits, modes
  and filter modes (`V2C:643-654`).
- The CBCR scale fields are absent for Cluster (`REG:338-345`).
- Chroma and luma share the destination size, so each chroma factor is
  computed from a source reduced by the subsampling in that axis.
- YUV with odd source x is refused in the plane check (`V2C:1054-1059`),
  for both classes.
- Cluster lists YUV formats as non-linear only: `YUV420_8BIT`,
  `YUV420_10BIT`, `YUYV` (`REG:69-71`, comment "non-Linear mode only").
  Esmart lists linear NV12, NV21, NV16, NV61, NV24, NV42, NV15, NV20, NV30,
  YVYU and YUYV (`REG:106-126`).

Inferred: a scaled YUV layer belongs on Esmart. On Cluster the CBCR scaler
does not exist in the register map, so a YUV AFBC layer scaled on a Cluster
plane is programmed with luma factors only; whether the hardware derives the
chroma scale on its own is not answered by source. The kernel does not
refuse it.

## 6. How this squares with "no plane scales" on the NanoPC-T6

Observed (2026-11-27, kernel 6.18.55-current-rockchip64, plan #457 scen-t6
handoff): 8 planes, zpos 0..7 on every plane, no plane exposes scaling, AFBC
only on Cluster planes, planes 39 and 69 cannot reach `video_port0`.

Read from source: DRM has no per-plane "can scale" property and VOP2 adds
none. The plane check takes scale from the `SRC_*` and `CRTC_*` rectangles
(`V2C:1020-1022`).

Inferred, in order of confidence:

1. "No plane exposes scaling" is a property-listing fact, not a hardware
   fact. Nothing a client reads can show 4x or 8x: the driver's 4 and 8 are
   neither exported nor enforced (section 2).
2. A refused scaled `TEST_ONLY` commit inside 1/8..8 has no cause in the
   scaling path of either class. If one is seen on the board, look at the
   other rules of section 1: below 4x4, above 4096x4320, odd-x YUV, or
   per-CRTC plane reach and count.
3. The planner treats every plane as unable to scale unless
   `DrmOptions::scaling` is set globally (`docs/rk3588-backend-design.md`).
   That default matches the observation and not the kernel. A profile table of
   Cluster up 4 / down 4 and Esmart up 8 / down 8 is the conservative
   reading; a `TEST_ONLY` sweep at growing ratios (plan item t6-sweep) is the
   only way to measure the 5x to 8x Cluster range.
