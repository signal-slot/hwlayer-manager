# BCM2711 (Raspberry Pi CM4, HVS5) recorded board data

Raw dumps from one session on a Seeed reTerminal (Raspberry Pi Compute
Module 4 Rev 1.0, BCM2711, vc4 driver, HVS5, built-in DSI panel 720x1280),
kernel `6.12.47+rpt-rpi-v8`, taken 2026-11-27 on an idle board (no display
manager or compositor running, DSI connector connected, no real commit; the
scale runs used atomic `TEST_ONLY` only). Binaries: composition-drm examples
built from repository commit 963eb8f. Each file starts with a comment header
(date, kernel, board, command, commit). Tests copy values from these files
into the test sources with a file and date comment; docs/observed is not read.

| File | Command |
| --- | --- |
| `props-2026-11-27.txt` | `props /dev/dri/card0` (every property, decoded `IN_FORMATS`) |
| `probe-raw-2026-11-27.txt` | `probe --raw /dev/dri/card0` |
| `scale-probe-2026-11-27.txt` | `scale_probe --plane ID --max-factor 32`, default `--dst` (360x640) |
| `scale-probe-modesize-2026-11-27.txt` | the same with `--dst 720x1280` (the mode size) |

All values below are **observed 2026-11-27** unless marked otherwise.

## Device

- 60 planes, 6 CRTCs (ids 59, 78, 90, 102, 114, 126), 3 connectors. Only the
  DSI connector is connected (CRTC index 2, CRTC 90, 720x1280@60 Hz); both
  HDMI connectors are disconnected.
- Driver caps: universal planes, atomic, addfb2 modifiers, async page flip
  (atomic async page flip: 1), cursor 64x64.

## Per plane class

| Class | Count | type | possible_crtcs | zpos | rotation |
| --- | --- | --- | --- | --- | --- |
| Primary | 6 | Primary | one CRTC each (index 0..5; ids 48, 67, 79, 91, 103, 115) | `0..=0` immutable | bitmask, value 1 (rotate-0) |
| Overlay, shared | 16 | Overlay | `0b111110` (CRTC indices 1..5, ids 127..292 step 11) | `1..=17`, mutable, initial 1..16 | bitmask, value 1 |
| Overlay, CRTC 0 only | 32 | Overlay | `0b1` (CRTC index 0, ids 303..644 step 11) | `1..=17`, mutable, initial 1..32 | bitmask, value 1 |
| Cursor | 6 | Cursor | one CRTC each (ids 655, 666, 677, 688, 699, 710) | `1..=17`, value 17 | bitmask, value 1 |

Notes:

- The connected CRTC (index 2) reaches 1 primary (79), the 16 shared overlays
  (127..292) and 1 cursor (677): 18 planes. The probe keeps 16 of them: the
  primary 79, overlays 127..270 (14) and the cursor 677; overlays 281 and 292
  are cut (`probe-raw-2026-11-27.txt` shows the probe's own listing).
- The 32 overlays of CRTC 0 (HDMI-A-1 side) do not reach the DSI CRTC.
- `zpos` on overlays is a mutable range 1..=17 with a distinct initial value
  per plane; the primary is fixed at 0.
- `rotation` is a bitmask property; the example prints only the current value
  (1 = rotate-0). The supported bits are not decoded by `props` (the `drm`
  crate gives no names for bitmask values), so what rotations the planes
  accept is **not measured** here.
- Other plane properties: `alpha` 0..=65535, `pixel blend mode`
  {None, Pre-multiplied, Coverage} (primary initial None, overlays initial
  Pre-multiplied), `COLOR_ENCODING` (BT.601/709/2020), `COLOR_RANGE`,
  `CHROMA_SITING_H/V`, `SCALING_FILTER` {Default, Nearest Neighbor},
  `IN_FENCE_FD`. The plane has no `SCALING` property that names a limit.

## IN_FORMATS (identical on all 60 planes)

35 formats, 5 modifiers. Modifier values are the Broadcom vendor
(`0x07`) codes, decoded with the `drm_fourcc.h` numbering
(**read from source**: VC4_T_TILED 1, SAND32 2, SAND64 3, SAND128 4,
SAND256 5; the dump itself shows only the numbers):

| Modifier | Decoded | Formats |
| --- | --- | --- |
| `0x0700000000000001` | `DRM_FORMAT_MOD_BROADCOM_VC4_T_TILED` | XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 |
| `0x0700000000000003` | `DRM_FORMAT_MOD_BROADCOM_SAND64` | NV12 NV21 |
| `0x0700000000000004` | `DRM_FORMAT_MOD_BROADCOM_SAND128` | NV12 NV21 P030 |
| `0x0700000000000005` | `DRM_FORMAT_MOD_BROADCOM_SAND256` | NV12 NV21 |
| `0x0000000000000000` | LINEAR | XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12 RX12 RA12 |

The blob is the same on primary, overlay and cursor planes. The linear set
has 34 formats (35 with `P030`, which only SAND128 carries); the probe's
`formats (32)` line is the linear list cut to 32 (it stops before RX12 and
RA12 and omits P030) and shows no modifiers. So on this kernel the blob
advertises T-tiled for the 8 RGB formats and SAND64, SAND128 and SAND256 for
NV12/NV21. Whether `TEST_ONLY` accepts each modifier is **not measured** in
this item (the cm4-sweep item does that).

## Scale limits (TEST_ONLY, XR24 source, XRGB primary below for overlays)

`scale_probe` on primary 79 and overlays 127, 138, 149, 160 and 292, each
with `--max-factor 32`, once with the default destination (360x640, half the
mode) and once with the destination at the mode size (720x1280):

- Upscale, axes `h`, `v` and `both`: every factor tried (1, 1.25, 1.5, 2, 3,
  4, 6, 8, 12, 16, 32) was accepted on every tested plane at both
  destination sizes. No rejection occurred, so the maximum is "32 or more",
  not a measured ceiling (`max_upscale_*=32.0000+`). At the mode size a factor
  of 32 means a source of 22.5x40 pixels.
- Downscale (0.5, 0.75, 0.9 on `h`, `v` and `both`): all accepted, down to the
  smallest factor the tool tries, 0.5. At the mode size that is a 1440x2560
  source. The true downscale ceiling is **not measured** (the tool does not
  try below 0.5).
- Primary and overlay planes behaved identically. No plane class was found
  where scaling differs.
- The cursor plane 677 (CRTC 90) is excluded by `scale_probe` (exit 2, "no
  plane to test"), so its scaling is not measured.
- Only 6 of the 17 CRTC-reachable planes were tested; the others share the
  hardware description and the `IN_FORMATS` blob, but that they scale the same
  is **inferred**.

## Scope

These runs are portability evidence for the HVS5 profile work. They never
count toward M1, M2 or M3.

## Sweeps (observed 2026-11-27)

`kms_sweep` and `scale_probe` from repository commit 0e4f9b8, run as root on
the idle board (kernel 6.12.47+rpt-rpi-v8, no display manager or compositor
active, DSI-1 720x1280, CRTC 90, primary 79, 16 candidate overlays). Files:
`sweep-modifiers-2026-11-27.txt`, `sweep-combos-2026-11-27.txt`,
`sweep-combos-scaled-2026-11-27.txt`, `sweep-replace-2026-11-27.txt`,
`scale-probe-ladder-2026-11-27.txt`; each has a header with date, kernel,
command and commit. All but `replace` are TEST_ONLY.

- **Modifiers** (planes 79, 127, 138; vc4-t-tiled, sand64, sand128, sand256
  tried on every format): of the pairs listed in `IN_FORMATS`, the ones the
  sweep could build passed ADDFB2 and TEST_ONLY (observed, 138 pair/plane
  results): T-tiled (0x0700000000000001) on the eight 32/16-bit RGB formats,
  SAND64/SAND128/SAND256 (0x...03, 0x...04, 0x...05) on NV12 and NV21. Of the
  unlisted pairs the sweep could build (342 pair/plane results), every one was
  refused by ADDFB2 with EINVAL and never reached TEST_ONLY, so the kernel
  rejects unlisted modifiers before the atomic check. Not tested: the sweep
  cannot build RGB8 (RGB888), BGR8 (BGR888) or P030 buffers, so those formats
  show `addfb=unsupported-format` with `test_only=not reached` on all three
  planes (42 pair/plane results). That covers RGB8/LINEAR, BGR8/LINEAR and
  P030/SAND128, which are listed (9 results), and RGB8, BGR8 and P030 with
  the vendor modifiers 0x0700000000000001/03/04/05, which are not listed (33
  results). Whether the kernel accepts them is not measured. The three planes
  gave the same counts (primary and overlays alike). The dumb buffers are
  zero-filled: this says what the kernel accepts, not that the hardware scans
  such content out correctly.
- **Combinations** (`combos --max 16`, 462 requests per run): every request
  was accepted at 1:1, with `--scale-v 0.5` and with `--scale-v 2`, up to all
  16 planes active full-screen (`COMBO_MAX accepted_k=16`). No LBM or
  line-buffer limit showed up in TEST_ONLY at 720x1280 with 16 scaled planes;
  whether HVS5 would scan that out is not measured (observed: accepted;
  hardware behaviour: not measured).
- **Scale ladder** (`scale_probe --max-factor 64`, planes 79 and 127, dst
  360x640): every upscale factor up to 64 on `h`, `v` and `both` and every
  downscale tried (0.5, 0.75, 0.9) was accepted, so the ceiling is "64 or
  more" and the downscale floor is "0.5 or below"; no rejection was found.
- **Replace** (`replace`): the setup commit succeeded with 1 event;
  `first=ok second=EBUSY events=1`, so a second nonblocking flip while the
  first is pending is refused with EBUSY and is not queued (observed). The
  output stays dark after the run.

These runs are portability evidence for the HVS5 profile work. They never
count toward M1, M2 or M3.
