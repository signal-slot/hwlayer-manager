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
