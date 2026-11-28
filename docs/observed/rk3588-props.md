# RK3588 (Rockchip VOP2) raw KMS properties and scaling limits, NanoPC-T6

Tag convention as in `docs/platform-capabilities.md`. Every row is
**observed 2026-11-28** on the board unless it says otherwise. It complements
`docs/observed/rk3588-topology.md`; rows that contradict that file are marked
**CONTRADICTS topology**.

Method: `props` and `scale_probe` examples of `composition-drm`, built for
`aarch64-unknown-linux-musl` (rust-lld), copied to `/tmp` on the board, run as
root against the primary device, then removed. `props` only reads properties.
`scale_probe` only issues atomic `TEST_ONLY | ALLOW_MODESET` requests, so
**every scaling result below is a TEST_ONLY result**: no frame was committed
and nothing was shown. The scaling probe's buffers are dumb buffers, so every
framebuffer in those tests is XR24 without a modifier (the kernel treats it as
linear).

## System

| Item | Value | Tag |
|---|---|---|
| Kernel | `6.18.55-current-rockchip64` (Armbian), aarch64 | observed 2026-11-28 |
| Primary DRM device | `/dev/dri/card0`, driver `rockchip` 1.0.0 ("RockChip Soc DRM") | observed 2026-11-28 |
| Mode | 1920x1080@60 on connector HDMIA-1 (HDMIA-2 disconnected), CRTC 81 | observed 2026-11-28 |
| DRM master | none held: only kernel fbcon on `Cluster0-win0`. Nothing was stopped; the fbcon framebuffer (fb 90, XR24, modifier 0x0 on plane 33) was still in place after the run, `/tmp` cleaned. | observed 2026-11-28 |

## CRTCs

Both CRTCs expose only `ACTIVE`, `MODE_ID`, `OUT_FENCE_PTR`, `GAMMA_LUT`,
`GAMMA_LUT_SIZE` (1024, immutable) and `VRR_ENABLED` (bool, 0). No
VOP2-specific CRTC property beyond those. (observed 2026-11-28)

## Per-plane properties

All eight planes carry the standard `CRTC_*`, `SRC_*`, `FB_ID`, `CRTC_ID`,
`IN_FENCE_FD`, `IN_FORMATS`, `alpha`, `pixel blend mode`, `rotation`, `type`
and `zpos` properties and **no other property**: no VOP2-specific plane
property (no color encoding/range, no scaling-filter, no AFBC toggle) exists on
this kernel. (observed 2026-11-28)

| Plane | Name (debugfs) | type | possible_crtcs | zpos (reset value, range) | alpha | pixel blend mode (reset) | rotation prop value | Tag |
|---|---|---|---|---|---|---|---|---|
| 33 | Cluster0-win0 | Primary | `0b01` (CRTC 0) | 0, 0..=7, mutable | 0..=65535 (=65535) | none/premult/coverage (premult) | 1 (rotate-0) | observed 2026-11-28 |
| 39 | Cluster1-win0 | Primary | **`0b10` (CRTC 1 only)** | 1, 0..=7, mutable | same | same (premult) | 1 | observed 2026-11-28 |
| 45 | Cluster2-win0 | Overlay | `0b11` | 2, 0..=7, mutable | same | same (premult) | 1 | observed 2026-11-28 |
| 51 | Cluster3-win0 | Overlay | `0b11` | 3, 0..=7, mutable | same | same (premult) | 1 | observed 2026-11-28 |
| 57 | Esmart0-win0 | Overlay | `0b11` | 0, 0..=7, mutable | same | same (**none**) | 1 | observed 2026-11-28 |
| 63 | Esmart1-win0 | Overlay | `0b11` | 1, 0..=7, mutable | same | same (**none**) | 1 | observed 2026-11-28 |
| 69 | Esmart2-win0 | Overlay | `0b11` | 6, 0..=7, mutable | same | same (premult) | 1 | observed 2026-11-28 |
| 75 | Esmart3-win0 | Overlay | `0b11` | 7, 0..=7, mutable | same | same (premult) | 1 | observed 2026-11-28 |

Notes:

- `possible_crtcs`: plane 39 (Cluster1) reaches `video_port1` only, so it
  cannot drive the connected output on CRTC 81. The topology note had
  "outputs 0b0 ... inferred"; the raw mask now confirms it. The seven other
  planes: Cluster0 on CRTC 0 only, all Overlay planes on both CRTCs.
  **Refines topology** (inference confirmed, with the mask).
- zpos: `range 0..=7` with no `immutable` flag on every plane, so zpos is
  writable (whether the kernel accepts every permutation was not exercised).
  The values above are the live ones at dump time. The topology note recorded
  the debugfs reset state as the plane index (Esmart0..3 = 4..7); the live
  property values of Esmart0, Esmart1 and Esmart2 are 0, 1 and 6, only
  Esmart3 (7) matches. **CONTRADICTS topology** (Esmart0, 1, 2 zpos).
- pixel blend mode: enum `{None=2, Pre-multiplied=0, Coverage=1}` on every
  plane; the *default* is None on Esmart0 and Esmart1 and Pre-multiplied on
  the others.
- rotation: the property type is `bitmask`; the `props` tool prints only the
  current value (1 = rotate-0 on all planes) and the drm crate does not expose
  the supported-bit list, so **the supported rotation/reflect bitmask is NOT
  observed**. This remains unobserved (debugfs `rotation=1` as in the topology
  note is likewise only the current value).
- IN_FENCE_FD is present (`-1`, i.e. no fence) on every plane.

## IN_FORMATS

| Planes | Formats | Modifiers | Tag |
|---|---|---|---|
| Cluster0-3 (33, 39, 45, 51), all identical | 14: XR30 XB30 XR24 AR24 XB24 AB24 RG24 BG24 RG16 BG16 YU08 YU10 YUYV Y210 | 9, all Arm AFBC (vendor 0x08): `0x0800000000000001`, `...041`, `...011`, `...081`, `...051`, `...0c1`, `...091`, `...0d1`, `...071`. Every one of the 14 formats is listed under every one of the 9 modifiers. **`DRM_FORMAT_MOD_LINEAR` (0x0) is NOT in the list.** | observed 2026-11-28 |
| Esmart0-3 (57, 63, 69, 75), all identical | 21: XR24 AR24 XB24 AB24 RG24 BG24 RG16 BG16 NV12 NV21 NV16 NV61 NV20 NV24 NV42 NV30 NV15 YVYU VYUY YUYV UYVY | 1: linear (0x0) only | observed 2026-11-28 |

- Complete lists, not truncated: the `props` dump reads the whole IN_FORMATS
  blob (probe's 32-entry cut-off is gone). Cluster2 and Cluster3 carry the
  same AFBC-only set as Cluster0/1. The AFBC flag bits behind each modifier
  value are not decoded here (inferred from the encoding, not observed).
- The "modifiers 0x0 linear support of Cluster2 and Cluster3" question left by
  the topology note: **neither plane lists linear in IN_FORMATS**. Yet
  `scale_probe`'s TEST_ONLY requests with a modifier-less (linear) XR24 dumb
  framebuffer were accepted on every Cluster plane (see below), and fbcon's
  modifier-0x0 XR24 buffer is live on Cluster0. So the kernel accepts
  modifier-less legacy framebuffers on Cluster planes although IN_FORMATS does
  not advertise linear. A client that follows IN_FORMATS strictly (explicit
  modifier) cannot place a linear buffer on a Cluster plane; a legacy
  `ADDFB` client can. **CONTRADICTS topology** finding 1 ("complete modifier
  data needs the raw dump"; and its inference that linear is accepted on
  Cluster0 means IN_FORMATS lists it, which it does not).
- Format-name notes: `NV20`/`NV30` print as unnamed fourcc (`0x3032564e`,
  `0x3033564e`) in scale_probe's list; props names them.

## Scaling limits (TEST_ONLY)

Setup of `scale_probe /dev/dri/card0 --max-factor 64`: output 1920x1080@60 on
CRTC 81, destination fixed 960x540 at 0,0, XR24 source, overlay tested over a
full-screen opaque primary with all other planes disabled, primary tested
alone. Binary search resolution 256/65536 (about 0.004). Date: 2026-11-28
(probe's own stamp 2026-11-28T01:29:19Z), kernel 6.18.55-current-rockchip64.

| Plane | Type | Max upscale h / v / both | Downscale accepted | Smallest factor tried | Tag |
|---|---|---|---|---|---|
| 33 Cluster0 | Primary | 8.0 / 8.0 / 8.0 | yes (h, v, both) | 0.5 | observed 2026-11-28, TEST_ONLY |
| 45 Cluster2 | Overlay | 8.0 / 8.0 / 8.0 | yes | 0.5 | observed 2026-11-28, TEST_ONLY |
| 51 Cluster3 | Overlay | 8.0 / 8.0 / 8.0 | yes | 0.5 | observed 2026-11-28, TEST_ONLY |
| 57 Esmart0 | Overlay | 8.0 / 8.0 / 8.0 | yes | 0.5 | observed 2026-11-28, TEST_ONLY |
| 63 Esmart1 | Overlay | 8.0 / 8.0 / 8.0 | yes | 0.5 | observed 2026-11-28, TEST_ONLY |
| 69 Esmart2 | Overlay | 8.0 / 8.0 / 8.0 | yes | 0.5 | observed 2026-11-28, TEST_ONLY |
| 75 Esmart3 | Overlay | 8.0 / 8.0 / 8.0 | yes | 0.5 | observed 2026-11-28, TEST_ONLY |
| 39 Cluster1 | Primary | not tested (cannot drive CRTC 81; `possible_on_crtc false`) | - | - | observed 2026-11-28 |

- Upscale: 8x accepted, the next ladder step (12x) rejected (the
  tool prints errno=34, i.e. ERANGE, for every rejection), and the bisection to 8.0039 was rejected on every
  axis. So the ceiling lies between 8.0000 and 8.0039: 8x exactly, the same
  on all seven tested planes and on both axes, independent of Cluster vs
  Esmart and of Primary vs Overlay.
- Downscale: 0.9, 0.75 and 0.5 accepted on h, v and both on every plane.
  0.5 is the smallest factor the probe tries; **no downscale limit was
  found**, and the tool's `min_downscale=0.5` is a "nothing smaller tried"
  figure, not a limit. A limit near 0.25 (4x reduction) is plausible but
  unobserved.
- Only XR24 linear sources were tested. AFBC buffers, YUV formats and
  non-zero source offsets were not scaled, so the table says nothing about
  format-dependent limits (inference: none confirmed, none excluded).
- The output was 1920x1080 only; limits that depend on the CRTC dimensions
  or the clock were not explored.

## Not observed

- Supported rotation/reflect bitmask of any plane (props prints the value
  only).
- Downscale limit below 0.5, scaling of AFBC or YUV sources, any non-TEST_ONLY
  scaling behaviour (what a real commit shows).
- Kernel log during the run was not watched on the serial console; the
  kernel log tail after the run showed only journald messages (no DRM errors),
  read over ssh.
