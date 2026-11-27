# Raspberry Pi 5 (BCM2712, HVS6) recorded data

Raw dumps taken on 2026-11-27 (UTC) on a Raspberry Pi 5 Model B Rev 1.1,
kernel `6.18.50+rpt-rpi-2712`, vc4 DRM driver (the second DRM card; the first
is v3d, a render node only), HDMI output 1920x1080@60, **no display manager
or compositor running** (`display-manager`, `lightdm`, `gdm`, `sddm` all
inactive, no labwc/wayfire/Xorg process found).
The binaries were built from repository commit 963eb8f. Every file starts with
a `#` comment header (date, kernel, board, command, commit). The dumps carry
no EDID bytes, only the blob id and size; nothing had to be stripped.

| File | Command |
| --- | --- |
| `props-2026-11-27.txt` | `props <vc4 card>` (every property of every CRTC, connector and plane, IN_FORMATS decoded per modifier) |
| `probe-raw-2026-11-27.txt` | `probe --raw <vc4 card>` |
| `scale-probe-2026-11-27.txt` | `scale_probe <vc4 card> --max-factor 32` (default dst: half the output, 960x540) |
| `scale-probe-modesize-2026-11-27.txt` | `scale_probe <vc4 card> --max-factor 32 --dst 1920x1080` (dst = mode size) |

The data is a test fixture source; code never reads it at run time.

## Topology (observed 2026-11-27)

56 planes, 4 CRTCs (indices 0..3; only index 2, CRTC 94, drives the connected
HDMI output). Plane ids in the dumps are of that boot.

| Class | Count | type | possible_crtcs | zpos | rotation | Measured scaling |
| --- | --- | --- | --- | --- | --- | --- |
| Primary | 4 (one per CRTC; ids 48, 67, 83, 95) | Primary | one CRTC each (0b1, 0b10, 0b100, 0b1000) | immutable 0..=0 (fixed 0) | `rotate-0` only | up 32x+ , down to 0.5 (plane 83), observed 2026-11-27 |
| Overlay, shared | 16 (ids 107..=272 step 11) | Overlay | 0b1110 (CRTCs 1, 2, 3) | mutable 1..=17, initial values 1..=16 | `rotate-0` only | up 32x+, down to 0.5, observed 2026-11-27 |
| Overlay, CRTC 0 only | 32 (ids 283..=624) | Overlay | 0b1 (CRTC 0) | mutable 1..=17, initial values 1..=32 | `rotate-0` only | not tested (not on the connected CRTC), observed 2026-11-27 for properties only |
| Cursor | 4 (ids 635, 646, 657, 668) | Cursor | one CRTC each | mutable 1..=17, initial 17 | `rotate-0` only | not tested: `scale_probe` excludes cursor planes (`--plane` on a cursor plane fails with "no plane to test") |

Planes reaching the connected CRTC 94: primary 83, the 16 shared overlays and
cursor 657, 18 in all (scale_probe tests the 17 non-cursor ones).

Every plane exposes the same property set: `alpha`, `pixel blend mode`
(None / Pre-multiplied / Coverage), `rotation` (bitmask value 1 = rotate-0
only, no rotation or reflection on any plane), `COLOR_ENCODING`,
`COLOR_RANGE`, `CHROMA_SITING_H/V`, `SCALING_FILTER` (Default, Nearest
Neighbor), `zpos`, `IN_FORMATS`. The cursor plane has `pixel blend mode`
Pre-multiplied as its value; every other plane has None. Cursor planes are not
fixed-format (same IN_FORMATS as the other planes).

## IN_FORMATS (observed 2026-11-27)

Identical on every plane (all 56): 45 formats, 5 modifiers. The `props` dump
decodes them per modifier. Modifier values are `fourcc_mod_code(BROADCOM, n)`
(vendor 0x07):

| Modifier | Decoded | Formats |
| --- | --- | --- |
| `0x0000000000000000` | linear | XR24 AR24 AB24 XB24 RX24 RA24 BX24 BA24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12 RX12 RA12 P010 P012 P016 S010 S012 S016 |
| `0x0700000000000001` | VC4_T_TILED (T-tiled) | XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 |
| `0x0700000000000003` | SAND64 | NV12 NV21 |
| `0x0700000000000004` | SAND128 | NV12 NV21 P030 |
| `0x0700000000000005` | SAND256 | NV12 NV21 |

So in the IN_FORMATS blob read on this kernel, T-tiled covers the packed RGB
formats listed and SAND64, SAND128 and SAND256 are all present for NV12/NV21
(SAND128 also P030). The blob only says what the driver advertises; whether
TEST_ONLY accepts each modifier is not measured here (that is the
`rpi5-sweep` item).

## Scaling (observed 2026-11-27)

`scale_probe`, TEST_ONLY only, primary on 83 plus the plane under test on
CRTC 94, XR24 buffers, factors 1 .. 32 (ladder) both axes and each axis alone,
downscale 0.5, 0.75, 0.9 on each axis and on both. Resolution 256/65536.

* Default dst (960x540): all 17 planes (primary 83 and the 16 shared
  overlays) accepted every upscale factor up to the largest tried, 32.0
  (source 30x16.875), and every downscale tried, down to 0.5 (source
  1920x1080 into 960x540), on h, v and both. No rejection anywhere
  (`grep -c rejected` = 0). The RESULT lines read
  `max_upscale_h/v/both=32.0000+ downscale=accepted min_downscale=0.5000`
  for all 17 planes.
* dst = mode size (1920x1080): the same results; the source for factor 0.5 is
  3840x2160 and was accepted on every plane, including the primary.
* The numbers are lower bounds from the tool's ladder: upscale is bounded by
  `--max-factor 32` and downscale by the smallest factor the tool tries
  (0.5). The true HVS6 limits (the design doc's open question Q1) are not
  found by this run; a follow-up with a larger `--max-factor` and a smaller
  downscale ladder is needed to see a rejection.
* No plane class differed: primary, overlay behave alike. The cursor plane was
  not measurable with this tool.
* Display state: the connected CRTC was already active at 1920x1080@60 with
  the console framebuffer; TEST_ONLY never changes it.
