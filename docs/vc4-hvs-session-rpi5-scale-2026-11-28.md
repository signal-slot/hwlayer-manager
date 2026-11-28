# Raspberry Pi 5 (BCM2712, HVS6) session A: scale limits, CRTC/plane arithmetic, listed HDMI modes

Answers HVS Q1 (scale limits), Q5 (CRTC/plane arithmetic) and the listed-modes part of Q10 of
[`hvs-backend-design.md`](hvs-backend-design.md) section 7 for HVS6. Tags: "observed" = measured
on the board on 2026-11-28 (host UTC), **read from source** = from the linked source study,
**inferred** = derived here. Procedure:
[`rk3588-vc4-board-procedure.md`](rk3588-vc4-board-procedure.md). Everything here is portability
evidence and never counts toward M1, M2 or M3. The page holds no address, host name or serial.

## Setup, observed 2026-11-28

| Item | Value |
|---|---|
| Board | Raspberry Pi 5 Model B, Raspberry Pi OS Lite arm64 |
| Kernel (`uname -r`) | `6.18.50+rpt-rpi-2712` |
| Device | `card1` = `vc4` ("Broadcom VC4 graphics"); `card0` = v3d (probe: "Not supported") |
| DRM access | DRM master, taken by each example itself (no lease) |
| Display manager before | `display-manager`, `gdm`, `lightdm`, `sddm`, `weston`: all `inactive`; no process held the card |
| Display manager after | the same five units: all `inactive` (nothing was stopped, so nothing had to be restored) |
| Build | `probe`, `props`, `scale_probe`, `kms_sweep` cross-built with `scripts/cross-aarch64.sh` (aarch64 musl), copied to a directory under `/tmp` on the board, which was removed at the end (`ls` afterwards: no such directory) |
| Mode in use | HDMI-A-1 on CRTC 94, 1920x1080 @ 60 Hz |
| Changes to the display | none: `scale_probe` and `kms_sweep combos` use `TEST_ONLY` requests only (read from source, example headers); no example committed |

## Q1: scale limits (`scale_probe`, TEST_ONLY), observed 2026-11-28

Two runs, XR24 buffers, each against the 17 non-cursor planes that can drive CRTC 94 (primary 83
and overlays 107, 118, 129, 140, 151, 162, 173, 184, 195, 206, 217, 228, 239, 250, 261, 272):

1. `scale_probe card1 --max-factor 32` (destination 960x540, the default).
2. `scale_probe card1 --dst 1920x1080 --max-factor 4096`: the upscale ladder runs until the source
   is one pixel.

Result lines (all 17 planes gave the same line apart from the plane id and kind):

```
RESULT plane=83  kind=Primary format=XR24 max_upscale_h=32.0000+   max_upscale_v=32.0000+   max_upscale_both=32.0000+   downscale=accepted min_downscale=0.5000
RESULT plane=107 kind=Overlay format=XR24 max_upscale_h=32.0000+   max_upscale_v=32.0000+   max_upscale_both=32.0000+   downscale=accepted min_downscale=0.5000
VERDICT scale_probe: OK 17 plane(s) probed at dst 960x540, resolution 256/65536
RESULT plane=83  kind=Primary format=XR24 max_upscale_h=1920.0000+ max_upscale_v=1080.0000+ max_upscale_both=1080.0000+ downscale=accepted min_downscale=0.5000
RESULT plane=107 kind=Overlay format=XR24 max_upscale_h=1920.0000+ max_upscale_v=1080.0000+ max_upscale_both=1080.0000+ downscale=accepted min_downscale=0.5000
NOTE plane=107 axis=both no rejection up to 1080.0000 (limited by the mode size: the source is one pixel)
VERDICT scale_probe: OK 17 plane(s) probed at dst 1920x1080, resolution 256/65536
```

| Plane type | Planes | Largest accepted upscale | Smallest rejected upscale | Downscale tried (accepted) | Smallest rejected downscale |
|---|---|---|---|---|---|
| Primary | 83 (the only primary on CRTC 94) | 1920x horizontal, 1080x vertical and both: the one-pixel source (the ladder's end, not a rejection) | none | 0.5, 0.75, 0.9 on h, v and both | none found |
| Overlay | 107 to 272 (16 planes) | the same, plane by plane | none | 0.5, 0.75, 0.9 on h, v and both | none found |

Findings:

- **No upscale ratio was rejected on any plane.** 1, 1.25, 1.5, 2, 3, 4, 6, 8, 12, 16, 32 and the
  one-pixel-source factor were accepted (observed 2026-11-28). The Q1 value "only 2x up tried" is
  therefore not a limit: the kernel accepted 1920x on one axis. This agrees with the source study,
  which finds no ratio field limit for upscale (read from source,
  [`vc4-hvs-scaling-source.md`](vc4-hvs-scaling-source.md) section 1).
- **The smallest rejected ratio is not found for downscale.** `scale_probe` fixes the downscale
  ladder at 0.5, 0.75 and 0.9 (read from source, `crates/composition-drm/examples/scale_probe.rs`
  header), and all were accepted, so the downscale limit lies below 0.5 and cannot be reached with the
  existing examples. The source study predicts 31.99x per axis for horizontal TPZ downscale
  (inferred, same document, section 1); that is **not confirmed** by this session. A later session
  needs a probe with a downscale ladder (a new example is outside this item).
- Only XR24 was tested; YUV and tiled-modifier planes may differ (the source study notes a
  YUV and a vertical-linear path, read from source), and are not measured here.
- The result is identical on the primary plane and on all overlays (inferred from 17 identical
  `RESULT` lines: no per-plane difference on HVS6).
- Inferred for `ScaleLimits`: a profile claim of "upscale unlimited, downscale at least 2x" is
  supported by TEST_ONLY on this kernel; "downscale limit" stays open.

## Q5: CRTCs, planes, `possible_crtcs`, types, observed 2026-11-28

`props card1` and `probe --raw card1`: 4 CRTCs, 56 planes, 2 connectors, 4 encoders. CRTC index
is the position in the resource list (`possible_crtcs` bit n).

| CRTC id | Index | State | Role |
|---|---|---|---|
| 59 | 0 | off | feeds a writeback connector ("mop") (observed 2026-11-28, same listing; the connector name is from [`observed/bcm2712-recheck.md`](observed/bcm2712-recheck.md)) |
| 78 | 1 | off | feeds a writeback connector ("moplet"), same source |
| 94 | 2 | on, 1920x1080@60 | HDMI-A-1 (encoder possible_crtcs `0b100`) |
| 106 | 3 | off | HDMI-A-2 (disconnected, encoder possible_crtcs `0b1000`) |

| Plane ids | Count | Type | `possible_crtcs` | zpos |
|---|---|---|---|---|
| 48, 67, 83, 95 | 4 | Primary | one each: `0b1`, `0b10`, `0b100`, `0b1000` | fixed 0 |
| 107, 118, ..., 272 (step 11) | 16 | Overlay | `0b1110` (CRTCs 1, 2, 3) | range 1..=17, current values 1 to 16 |
| 283, 294, ..., 624 (step 11) | 32 | Overlay | `0b1` (CRTC 0 only) | range 1..=17, current values 1 to 32 |
| 635, 646, 657, 668 | 4 | **Cursor** | one each: `0b1`, `0b10`, `0b100`, `0b1000` | 17 |

4 + 16 + 32 + 4 = 56.

- **The extra per-CRTC planes are not cursors.** The four cursor planes are one per CRTC. The
  extra planes are 32 overlays bound to CRTC 0 only and 16 overlays shared by CRTCs 1 to 3
  (observed 2026-11-28).
- For the HDMI output (CRTC 94): 1 primary + 16 overlays = 17 candidates (plus 1 cursor, which
  `scale_probe` and `kms_sweep` skip). The 32 CRTC-0 overlays cannot drive it
  (`possible_on_crtc false` in both examples).
- Every plane lists the same `IN_FORMATS`: 45 formats, 5 modifiers: `0x0700000000000001`
  (T-tiled) with XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15; `0x0700000000000004` with NV12 NV21
  P030; `0x0700000000000003` and `0x0700000000000005` with NV12 NV21; linear (0) with
  the long RGB, YUV and 12-bit list that starts XR24 AR24 AB24 XB24 RX24 (the tool's line was cut
  there). The tiled modifiers are printed before linear in every blob. Rotation is a bitmask with value
  `rotate-0` on every plane; scaling filter, alpha, pixel blend mode, color encoding and range
  exist on every plane (observed 2026-11-28).
- `kms_sweep combos` (TEST_ONLY, 1920x1080 XR24 full-screen planes at 1:1, primary 83 always
  included, k = number of active planes, up to 33 sets per k):

```
COMBO k=16 set=15 planes=83,118,129,...,272 scale=1.0000x1.0000 src=1920.0000x1080.0000 dst=1920x1080 result=ok
COMBO k=17 set=0 planes=83,107,118,...,272 scale=1.0000x1.0000 src=1920.0000x1080.0000 dst=1920x1080 result=ENOSPC
COMBO_MAX accepted_k=16
VERDICT kms_sweep: DONE combos
```

  463 requests with k = 1 to 16 were accepted (every set tried); the only request at k = 17 (all
  17 candidates) was rejected with `ENOSPC` (observed 2026-11-28). So **16 simultaneous full-screen
  XR24 planes per CRTC are accepted and the 17th is not**; the rejection is an out-of-space error,
  which fits dlist memory or similar allocator exhaustion (inferred; the dlist study is
  [`vc4-hvs-dlist-source.md`](vc4-hvs-dlist-source.md)). It is not a plane-count limit of the
  hardware lists (all 17 exist) and it depends on plane size, format and scale: smaller planes
  may fit (not measured). The engine's `MAX_PLANES` question is a composition-core decision and
  is only recorded here.

## Q10 (listed modes part): HDMI-A-1, observed 2026-11-28

`probe` lists 20 modes on HDMI-A-1 (HDMI-A-2: disconnected, 0 modes). The pixel clocks come from
the system tool `kmsprint -m` (read only, already installed on the board; not an example of this
repository), because `probe` prints only width, height and refresh. Timing columns are
hdisplay/hfp/hsync/hbp and vdisplay/vfp/vsync/vbp.

| # | Mode | Pixel clock (MHz) |
|---|---|---|
| 0 | 1920x1080@60.00 (preferred) | 148.500 |
| 1 | 1920x1080@60.00 | 148.500 |
| 2 | 1920x1080@59.94 | 148.352 |
| 3 | 1920x1080@50.00 | 148.500 |
| 4 | 1920x1080i@50.00 | 74.250 |
| 5 | 1280x1024@75.02 | 135.000 |
| 6 | 1280x1024@60.02 | 108.000 |
| 7 | 1280x720@60.00 | 74.250 |
| 8 | 1280x720@60.00 | 74.250 |
| 9 | 1280x720@59.94 | 74.176 |
| 10 | 1280x720@50.00 | 74.250 |
| 11 | 1024x768@75.03 | 78.750 |
| 12 | 1024x768@60.00 | 65.000 |
| 13 | 800x600@75.00 | 49.500 |
| 14 | 800x600@60.32 | 40.000 |
| 15 | 640x480@75.00 | 31.500 |
| 16 | 640x480@60.00 | 25.200 |
| 17 | 640x480@59.94 | 25.175 |
| 18 | 640x480@59.94 | 25.175 |
| 19 | 720x400@70.08 | 28.320 |

- The list is what the attached sink's EDID offers (the connector has `EDID`, 256 bytes); the
  highest mode is 1920x1080@60, 148.5 MHz (observed 2026-11-28). It says nothing about the
  BCM2712 limit: **the sink cannot show a 4K or 4Kp60 mode, so the BCM2712 HDMI limit is not
  answered here** (inferred). The limit that the TXP writeback reports, 4096x2160@59.999, is from
  [`observed/bcm2712-recheck.md`](observed/bcm2712-recheck.md) (observed 2026-11-28 there), and
  is the writeback, not HDMI.
- Kernel: `6.18.50+rpt-rpi-2712` (observed 2026-11-28).

**Left for the later session (rpi5-hvs6-session):** the TEST_ONLY modeset of each listed mode
(whether the kernel accepts each mode on CRTC 94), the replace-pending question (Q6), and any
mode outside the sink's EDID. This session did not run any of them.

## Not done here

No claim about `replace_pending` (Q6), no TEST_ONLY modeset of the modes, no real commit, no
YUV or modifier scale test, no downscale below 0.5.
