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

## Sweeps (observed 2026-11-27)

Run 2026-11-27 (UTC, board clock) on the same board and kernel
`6.18.50+rpt-rpi-2712` as above, vc4 card, 1920x1080@60 on CRTC 94, as root
(DRM master), **no display manager or compositor running** (all of
`display-manager`, `gdm`, `sddm`, `lightdm`, `weston` inactive before and after;
only the tty1 getty and the serial getty). Binaries `kms_sweep` and
`scale_probe` built from repository commit 0e4f9b8. Every request is
`TEST_ONLY | ALLOW_MODESET` except `replace`, which makes real commits.
Buffers are zero-filled dumb buffers: an accepted modifier means the kernel
check passed, not that the hardware would scan such content correctly.

| File | Command |
| --- | --- |
| `sweep-modifiers-2026-11-27.txt` | `kms_sweep modifiers --plane P --modifier vc4-t-tiled --modifier sand64 --modifier sand128 --modifier sand256` for P = 83 (primary), 107, 118 (overlays) |
| `sweep-combos-2026-11-27.txt` | `kms_sweep combos --max 16` |
| `sweep-combos-scaled-2026-11-27.txt` | `kms_sweep combos --max 16 --scale-v 0.5` and `--scale-v 2` (every layer, primary included) |
| `sweep-replace-2026-11-27.txt` | `kms_sweep replace` |
| `scale-probe-ladder-2026-11-27.txt` | `scale_probe --plane P --max-factor 64` for P = 83 and 107 |

### Modifiers (observed 2026-11-27)

Result per modifier, identical on the primary (83) and both overlays (107, 118).
Modifier values: `0x0700000000000001` T-tiled, `...03` SAND64, `...04` SAND128,
`...05` SAND256.

| Modifier | IN_FORMATS lists it for | ADDFB2 | TEST_ONLY |
| --- | --- | --- | --- |
| linear (0) | 39 of 45 formats per plane (the other 5 per plane: `addfb=unsupported-format`, sweep does not know the layout) | ok | **accepted**, all 39 |
| T-tiled | 8 packed-RGB formats per plane (24 pairs over 3 planes) | ok | **EINVAL, all 24** |
| SAND64 | NV12, NV21 | ok | **EINVAL, all 6** |
| SAND128 | NV12, NV21 (+1 format per plane the sweep cannot build) | ok | **accepted**, NV12 and NV21 on each plane (6 of 6) |
| SAND256 | NV12, NV21 | ok | **accepted**, NV12 and NV21 on each plane (6 of 6) |

Pairs the blob does not list (`listed=no`; every `--modifier` on every other
format, 111 per modifier over 3 planes) fail at ADDFB2 with EINVAL, so TEST_ONLY
is not reached for them. Conclusion: on BCM2712 / HVS6 the IN_FORMATS blob
lists T-tiled and SAND64, but TEST_ONLY rejects both; only SAND128 and SAND256
(NV12/NV21) pass beyond linear. This settles the conflict of
`docs/hvs-backend-design.md` §5 in favour of the source (HVS6 rejects
T-tiled and SAND64), and the blob over-reports.

### Combinations (observed 2026-11-27)

Candidates: primary 83 plus 16 shared overlays, 107..272 step 11 (17 planes,
cursor excluded). 33 sets per k (the first plus 32 spread), full-screen linear
XRGB/ARGB, distinct ascending zpos.

| Run | `COMBO_MAX` | per k |
| --- | --- | --- |
| plain (1:1) | `accepted_k=16` | k = 1..16 every set `ok` (all 17 planes not tried: `--max 16`) |
| `--scale-v 0.5` | `accepted_k=8` | k <= 8 `ok`; k = 9..16 every set `ENOSPC` |
| `--scale-v 2` | `accepted_k=4` | k <= 4 `ok`; k = 5..16 every set `ENOSPC` |

The ENOSPC rejections coincide with `vc4-drm ... Failed to allocate LBM entry:
-28` lines in the kernel log (read after the run): vertically scaled layers
each need line-buffer memory, so the budget is about 8 layers at 2:1 downscale
and 4 at 2x upscale on a full-screen 1920x1080 destination (every set of the
same k gave the same result, so this is a count budget at this size, not a
per-plane property). The ENOSPC boundary depends on source and destination
sizes; it is one data point, not the HVS6 limit.

### Pending commit (observed 2026-11-27)

`SETUP commit=ok events=1`, then `REPLACE first=ok second=EBUSY events=1`: a
non-blocking page flip was accepted, and a second non-blocking flip committed
at once, before the first flip's event, failed with EBUSY. A pending commit
cannot be replaced on HVS6 / this kernel (design doc §7 Q6); one event
arrived, for the accepted commit. After the run the sweep's framebuffers are
removed and, per the tool's NOTE, the output stays dark until the next client
commits; the screen was not looked at afterwards (no display manager exists
to restart).

### Scale ladder (observed 2026-11-27)

`scale_probe --max-factor 64` at dst 960x540: primary 83 and overlay 107 both
`max_upscale_h/v/both=64.0000+` (no rejection up to 64x; the source shrinks to
15x8.4 pixels), `downscale=accepted min_downscale=0.5000`. Still lower
bounds (ladder limit 64x, smallest downscale tried 0.5); the real HVS6 limits
(Q1) were not reached. The 8/4 layer limits above show that the
combination budget, not the per-plane ratio, is what TEST_ONLY rejects.
