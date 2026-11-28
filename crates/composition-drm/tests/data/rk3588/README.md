# NanoPC-T6 (RK3588, VOP2) recorded KMS data

Board: FriendlyElec NanoPC-T6, driver `rockchip`, kernel
`6.18.55-current-rockchip64`, HDMI-A-1 connected at 1920x1080@60, CRTC 81
(video_port0). The examples ran as master with no display manager or
compositor active (observed 2026-11-27). Each file starts with a `#` comment
header (date, kernel, board, exact command, repository commit the binaries were
built from). Nothing was removed from the dumps: they hold no EDID bytes (only
blob ids and sizes) and no serial-like value.

Code never reads these files at runtime; tests copy the data they need into the
test file with the file name and date in a comment.

| File | Command |
| --- | --- |
| `props-2026-11-27.txt` | `props /dev/dri/card0`: every property of CRTCs, connectors, planes, with the full IN_FORMATS list per plane (format and modifier). |
| `probe-raw-2026-11-27.txt` | `probe --raw /dev/dri/card0`: the probe summary (32-pair cut) followed by the raw dump with the complete IN_FORMATS pair list (126 or 21 pairs). |
| `scale-probe-2026-11-27.txt` | `scale_probe /dev/dri/card0 --max-factor 32`: default destination 960x540, TEST_ONLY scale ladder up and down. |
| `scale-probe-modesize-2026-11-27.txt` | the same with `--dst 1920x1080` (the mode size, for downscale). |

## Per plane (all observed 2026-11-27)

Ids are from the kernel in the dump. "Cluster" and "Esmart" are VOP2 window
names; KMS does not name them. They are recognised only from the plane
properties below (see Recognition).

| Plane | type | Class | Reaches CRTC 81 | IN_FORMATS pairs | AFBC | zpos | Upscale max (RESULT) | Downscale |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 33 | Primary | Cluster | yes (0b01) | 126 (14 formats x 9 modifiers) | yes | 0..=7 (current 0) | 8.0 (h, v, both) | 0.5 accepted |
| 39 | Primary | Cluster | no (0b10, CRTC 82 only) | 126 (14 x 9) | yes | 0..=7 (current 1) | not probed (cannot reach CRTC 81) | not probed |
| 45 | Overlay | Cluster | yes (0b11) | 126 (14 x 9) | yes | 0..=7 (current 2) | 8.0 | 0.5 accepted |
| 51 | Overlay | Cluster | yes (0b11) | 126 (14 x 9) | yes | 0..=7 (current 3) | 8.0 | 0.5 accepted |
| 57 | Overlay | Esmart | yes (0b11) | 21 (21 formats x 1 modifier) | no | 0..=7 (current 0) | 8.0 | 0.5 accepted |
| 63 | Overlay | Esmart | yes (0b11) | 21 (21 x 1) | no | 0..=7 (current 1) | 8.0 | 0.5 accepted |
| 69 | Overlay | Esmart | yes (0b11) | 21 (21 x 1) | no | 0..=7 (current 6) | 8.0 | 0.5 accepted |
| 75 | Overlay | Esmart | yes (0b11) | 21 (21 x 1) | no | 0..=7 (current 7) | 8.0 | 0.5 accepted |

All eight planes: 8 planes in total, `zpos` is a mutable range 0..=7 on every
plane, `alpha` 0..=65535, `pixel blend mode` {None=2, Pre-multiplied=0,
Coverage=1}, and `rotation` is mutable with current value 1 (rotate-0). The
`props` and `probe --raw` output shows only the current value of `rotation`,
not the set of supported bits, so the supported rotation values are not
recorded here (not measured).

### Cluster planes (33, 39, 45, 51)

* 14 formats: XR30 XB30 XR24 AR24 XB24 AB24 RG24 BG24 RG16 BG16 YU08 YU10 YUYV Y210.
* 9 modifiers, all `DRM_FORMAT_MOD_ARM_AFBC` (vendor 0x08): `0x0800000000000001`,
  `...41`, `...11`, `...81`, `...51`, `...c1`, `...91`, `...d1`, `...71`; the
  same 14 formats under each. No `0x0` (linear) pair is listed: every pair on
  a Cluster plane carries an AFBC modifier (observed 2026-11-27).
* The probe's default view stops at 32 pairs (2 modifiers and a part of a
  third); the full list is 126 pairs, 14 x 9 (observed 2026-11-27).

### Esmart planes (57, 63, 69, 75)

* 21 formats: XR24 AR24 XB24 AB24 RG24 BG24 RG16 BG16 NV12 NV21 NV16 NV61 NV20
  NV24 NV42 NV30 NV15 YVYU VYUY YUYV UYVY, one modifier `0x0` (linear), no AFBC.
* The probe prints NV20 and NV30 as `0x3032564e` / `0x3033564e` in scale_probe
  (fourcc names it has no table for); the props/probe dumps name them.
  `pixel blend mode` default is None (value 2) on planes 57 and 63, and
  Pre-multiplied (0) on 69 and 75 (observed 2026-11-27).

### Recognition (Q3)

Cluster versus Esmart is recognised through KMS as (all observed 2026-11-27):

* Cluster: the IN_FORMATS blob carries AFBC modifiers (vendor ARM, 0x08), 14
  formats, no YUV 4:2:0/4:2:2 semi-planar (NV*) formats; both Primary planes
  are Cluster.
* Esmart: IN_FORMATS is a single linear modifier with 21 formats including NV*;
  all four are Overlay.
* The plane id order (33, 39, 45, 51 versus 57, 63, 69, 75) and the type
  property alone do not tell the class: two of the Cluster planes are also
  Overlay. `zpos` ranges and scaling limits are identical for both classes.

## Scaling (scale_probe RESULT lines, observed 2026-11-27)

* Every probed plane (33, 45, 51 Cluster; 57, 63, 69, 75 Esmart): upscale
  accepted up to 8x and rejected from 8.0039 (errno 34, ERANGE) on h, v and
  both axes, at both destinations (960x540 and 1920x1080), format XR24 on every
  plane. 8x is the common limit, the ladder shows no difference between
  Cluster and Esmart.
* Downscale: 0.9, 0.75 and 0.5 accepted on h, v and both on every plane.
  scale_probe's lowest downscale step is 0.5, so the limit below 0.5 is not
  measured here.
* Plane 39 is not probed: it cannot reach CRTC 81.
* One format per plane (XR24) was used for the ladder; YUV formats and AFBC
  modifiers were not (linear buffers only).

## Differences from the earlier handoff (observed 2026-11-27)

* The earlier note says planes 39 and 69 cannot reach video_port0. In this
  master run plane 69 lists possible_crtcs 0b11 and was probed on CRTC 81;
  plane 39 is 0b10 only. The earlier run was under the kernel lease, which may
  hide plane 69 from the lease's CRTC; this was not tested here.
* The earlier note says no plane scaled. In this run every probed plane
  scaled (up to 8x, down to 0.5), so the "scaling not claimed" line in the
  probe summary is the planner's own default, not a kernel statement.

## Sweeps (observed 2026-11-27)

Same board, kernel and state as above (master, no display manager or
compositor; only getty units ran, and they still do). Binaries built from
repository commit 0e4f9b80805117d97bcd15d6cc3e60ee062fef03 with
`scripts/cross-aarch64.sh`; each file's header has the exact command. All runs
are TEST_ONLY, nothing was committed.

| File | Content |
| --- | --- |
| `sweep-combos-2026-11-27.txt` | `kms_sweep combos --max 8`: full-screen 1920x1080 linear layers, 1:1. |
| `sweep-combos-scaled-2026-11-27.txt` | the same with `--scale-v 2`, `--scale-h 2`, `--scale-h 0.5` (three runs). |
| `sweep-modifiers-2026-11-27.txt` | `kms_sweep modifiers` plus `--modifier linear`, `afbc-16x16`, `sand128` on every format. |
| `scale-probe-classes-2026-11-27.txt` | `scale_probe --plane 33` (Cluster) and `--plane 57` (Esmart), `--max-factor 64`, dst 960x540 and 1920x1080. |

### Combinations (Q4)

* The candidates for CRTC 81 are 7 planes (33, 45, 51, 57, 63, 69, 75);
  plane 39 does not reach it. So 8 simultaneous windows on one video port are
  not reachable with this CRTC; the sweep went to k=7.
* `COMBO_MAX accepted_k=4`. Every one of the 20 four-plane sets tried (the
  primary 33 plus 3 others, Cluster and Esmart mixed) was accepted; every one
  of the 15 five-plane, 6 six-plane and 1 seven-plane sets tried returned
  EINVAL. Counts per k: 1/1, 6/6, 15/15, 20/20 accepted, then 0/15, 0/6, 0/1.
  These are all the subsets: with 7 candidates and the primary fixed there are
  C(6,k-1) sets per k, and none was left out.
* Why 5 fail is not known from this run (the sweep reports only EINVAL). The
  layers were full-screen at 1:1, so a bandwidth or per-port window limit
  both fit; this is inferred, not observed. Four windows per video port is
  not in docs/rk3588-backend-design.md's table.
* Scaled runs gave the identical result: COMBO_MAX accepted_k=4 for
  `--scale-v 2`, `--scale-h 2` and `--scale-h 0.5`. Every plane class
  accepted a 2x upscale (Cluster and Esmart together, all 20 four-plane
  sets) and a 0.5 horizontal downscale.

### Modifiers

* No `listed=yes` pair with a successful ADDFB2 was refused by TEST_ONLY:
  400 listed pairs `addfb=ok test_only=ok`. The listed pairs that did not
  reach TEST_ONLY are `addfb=unsupported-format` (YU08, YU10 on Cluster, 18
  pairs per Cluster plane; 2 per Esmart plane): the sweep cannot lay out
  those formats, which says nothing about the kernel.
* Unlisted pairs the kernel accepted: linear (0x0) on Cluster planes 33, 45,
  51 for XR24, AR24, XB24, AB24, RG24, BG24, RG16, BG16, YUYV, Y210 (10 per
  plane, `listed=no addfb=ok test_only=ok`), although the Cluster IN_FORMATS
  blob lists only AFBC modifiers. These MODIFIER lines disagree with the
  listed IN_FORMATS (the sweep reports linear as unlisted for Cluster).
  Linear XR30 and XB30 on Cluster failed at ADDFB2 (EINVAL), probably because
  the sweep's linear buffer layout for them differs; not investigated.
* AFBC on Esmart (0x0800000000000001, unlisted): ADDFB2 accepts it for 9
  formats (XR24, AR24, XB24, AB24, RG24, BG24, RG16, BG16, YUYV) but
  TEST_ONLY returns EINVAL, consistent with "AFBC only on Cluster". The other
  formats fail already at ADDFB2. SAND128 (0x0700000000000004) fails at
  ADDFB2 (EINVAL) on every Esmart format it was tried on (all non-
  `unsupported-format` ones) and on Cluster.
* Linear Esmart pairs (21 formats, one modifier) were all accepted,
  including NV12 linear. NV12 is not in the Cluster plane's format list, so
  "NV12 linear on Cluster" was not testable; Cluster linear YUYV and Y210 are
  accepted (unlisted, above).

### Scaling per class (scale_probe, RESULT lines)

| Plane | Class | max_upscale h / v / both | downscale |
| --- | --- | --- | --- |
| 33 | Cluster | 8.0 / 8.0 / 8.0 (dst 960x540 and 1920x1080) | accepted down to 0.5 |
| 57 | Esmart | 8.0 / 8.0 / 8.0 (dst 960x540 and 1920x1080) | accepted down to 0.5 |

`--max-factor 64` did not raise the upscale limit: the 8.0 reported by RESULT is the same for both classes and both destination sizes, so the earlier
`--max-factor 32` data is unchanged. No difference between Cluster and Esmart
scaling showed in these runs (VOP2 source describes different scalers per
class; that is not seen here for the ratios tested).

## Profile check 2026-11-28 (t6-verify)

Observed 2026-11-28, kernel 6.18.55-current-rockchip64, repository commit
2d689bac9da2f51b0247e4aff5feef42bb511efb, DRM master (display manager, gdm,
lightdm and sddm inactive before and after). Portability evidence only; it
counts toward none of M1, M2 or M3. Files: `verify-probe-2026-11-28.txt`,
`verify-profile-check-2026-11-28.txt`, `verify-twoplane-2026-11-28.txt`,
`verify-camera-2026-11-28.txt`.

* `profile_check --profile generic`: AGREEMENT total=21 agree=12 stricter=6
  looser=0. The 6 stricter cases are scale-0.5 .. scale-8: Generic refuses
  because the plane claims no scaling, TEST_ONLY accepts them (forced caps).
* `profile_check --profile rk3588`: AGREEMENT total=21 agree=18 stricter=0
  looser=0; no planner-looser case. The 3 `agree=no` frames were not compared:
  rotate-90 and reflect-y (composition-drm does not program rotation) and
  wide-source (dumb buffer of that width refused with EINVAL at creation).
* With the Rk3588Vop2 profile the probe shows scaling up 8.00 down 8.00 and max
  source 4096x4320 on every plane.
* `profile_check --show 20` and `twoplane --frames 600` (generic) both failed
  with "no primary plane with a linear 32-bit RGB format": primary plane 33
  lists only AFBC modifiers. Nothing was displayed, so no KVM picture was
  judged and camera-check was not run (see `verify-camera-2026-11-28.txt`).
