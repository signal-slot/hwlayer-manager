# reTerminal (CM4, BCM2711, HVS5): scale limits, session A

Dated 2026-11-28 (host UTC). Answers HVS5 part of [`hvs-backend-design.md`](hvs-backend-design.md) §7 Q1 (ScaleLimits). The run is portability evidence and never counts toward M1, M2 or M3. Replace-pending (Q6) is not part of this session.

## Setup

| Item | Value | Tag |
|---|---|---|
| Board | Seeed reTerminal, Raspberry Pi CM4 (BCM2711, HVS5), no camera or KVM | observed 2026-11-28 |
| Kernel (`uname -r`) | `6.12.47+rpt-rpi-v8` | observed 2026-11-28 |
| DRM driver | `vc4` (card0), `v3d` render only (card1) | observed 2026-11-28 |
| Tools | existing `probe` and `scale_probe` examples of `composition-drm` at commit a0eaa56, cross-built static aarch64 musl with `scripts/cross-aarch64.sh`; copied to a session directory under `/tmp` on the board | observed 2026-11-28 |
| DRM access | master (no lease). The tool opened the card directly; nothing else held it | observed 2026-11-28 |
| Display owner before | `display-manager`, `gdm`, `lightdm`, `sddm`, `weston`: all `inactive`; `fuser` on `/dev/dri/card*` showed no holder. Nothing was stopped, so `display-guard.sh` was not needed | observed 2026-11-28 |
| Display owner after | same five units `inactive`, no holder | observed 2026-11-28 |
| Cleanup | the `/tmp` session directory was removed; `ls` reported it missing | observed 2026-11-28 |
| Method | `scale_probe /dev/dri/card0 --max-factor 32` (all planes), then `--plane 91` and `--plane 127` with `--max-factor 4096`; atomic `TEST_ONLY | ALLOW_MODESET` only, no real commit | observed 2026-11-28 |

## Planes seen

`scale_probe` reported mode 1920x1080@60 on CRTC 102 for the connected connector it chose (a second connected output besides the built-in 720x1280 DSI panel was present on this run; `probe` listed two outputs, CRTC 102 at 640x480@75 and CRTC 90 at 720x1280@60). The kernel list had 6 primaries (48, 67, 79, 91, 103, 115), 48 overlays (127 ... 644, step 11) and 6 cursors (655 ... 710), 60 in total. Of these, 91 (primary) and 127 ... 292 (16 overlays) can drive CRTC 102 and 688 (cursor) too (observed 2026-11-28, `scale_probe` PLANE lines). `probe` itself cuts the list to 16 planes (`MAX_PLANES`) and shows 32 formats per plane, while `scale_probe` lists 35 formats incl. `0x30333050` (P030), `RX12`, `RA12` (observed 2026-11-28; this is the `MAX_FORMATS` cut, a composition-core decision, not changed here). Every plane showed the same format list.

## Result: per plane type

Format XR24 (XRGB8888) linear dumb buffers, destination fixed at 960x540, other planes off.

| Plane type | Planes probed | Largest accepted upscale | Smallest accepted downscale tried | Tag |
|---|---|---|---|---|
| Primary | 91 (the only primary for CRTC 102) | no rejection up to 64x on both axes; h and v separately up to the one-pixel source (960x and 540x); ladder ended at the mode size, not at a rejection | 0.5 (also 0.75, 0.9) accepted on h, v and both | observed 2026-11-28 |
| Overlay | all 16 for the CRTC (127, 138, 149, 160, 171, 182, 193, 204, 215, 226, 237, 248, 259, 270, 281, 292) | `--max-factor 32` run: all 16 `32.0000+` on h, v and both. `--plane 127` with `--max-factor 4096`: no rejection up to one-pixel source (960x h, 540x v and both) | 0.5 (also 0.75, 0.9) accepted on h, v and both on all 16 | observed 2026-11-28 |
| Cursor | not probed (`scale_probe` skips cursor planes) | not measured | not measured | observed 2026-11-28 (tool behaviour) |

Raw evidence (lines as printed by `scale_probe`, kernel `6.12.47+rpt-rpi-v8`):

```text
VERDICT scale_probe: OK 17 plane(s) probed at dst 960x540, resolution 256/65536
RESULT plane=91 kind=Primary format=XR24 max_upscale_h=32.0000+ max_upscale_v=32.0000+ max_upscale_both=32.0000+ downscale=accepted min_downscale=0.5000
RESULT plane=127 kind=Overlay format=XR24 max_upscale_h=32.0000+ max_upscale_v=32.0000+ max_upscale_both=32.0000+ downscale=accepted min_downscale=0.5000
... same line for planes 138, 149, 160, 171, 182, 193, 204, 215, 226, 237, 248, 259, 270, 281, 292 ...
SCALE plane=127 axis=both src=15.0000x8.4375 dst=960x540 factor=64.0000 test_only=accepted errno=0
SCALE plane=127 axis=both src=1920.0000x1080.0000 dst=960x540 factor=0.5000 test_only=accepted errno=0
RESULT plane=127 kind=Overlay format=XR24 max_upscale_h=960.0000+ max_upscale_v=540.0000+ max_upscale_both=540.0000+ downscale=accepted min_downscale=0.5000
RESULT plane=91 kind=Primary format=XR24 max_upscale_h=960.0000+ max_upscale_v=540.0000+ max_upscale_both=540.0000+ downscale=accepted min_downscale=0.5000
```

## Answer to HVS5 Q1

- **Upscale**: no limit was found. TEST_ONLY accepted every ratio up to the mode-size bound (a one-pixel source, 540x vertical, 960x horizontal at this destination) on Primary and Overlay planes alike (observed 2026-11-28). The profile need not claim a finite upscale cap below 32x for HVS5; a conservative `ScaleLimits` of at least 32x up is measured. This matches the source read: upscale uses the polyphase or one-pixel TPZ path with no ratio field to saturate ([`vc4-hvs-scaling-source.md`](vc4-hvs-scaling-source.md) §1.2, read from source).
- **Downscale**: 0.5 (source twice the destination) was accepted on both axes on every plane type probed; 0.75 and 0.9 too. `scale_probe` tests no ratio below 0.5, so the **smallest accepted downscale was not found**: it is at most 0.5 (observed 2026-11-28). The source read predicts a TPZ refusal only beyond about 32x downscale on an axis, and vertical line skipping above 3x (read from source, [`vc4-hvs-scaling-source.md`](vc4-hvs-scaling-source.md) §1.2, §1.4); this run did not test it. Measuring it needs a downscale ladder, which the existing example does not offer (the example may not be changed in this item), so the lower bound stays **inferred ~1/31.99** (read from source) and open as a measurement.
- Primary and Overlay planes behaved the same; cursor planes were not measured.
- Plane count and list agree with the observed topology page ([`observed/bcm2711-cm4-topology.md`](observed/bcm2711-cm4-topology.md)) as to kernel and driver; the 60-plane total here is the kernel's, larger than probe's cut view (observed 2026-11-28).

## Limits of this run

- Only XRGB8888 was tested; YUV formats need chroma scalers and may differ (inferred from source, `vc4-hvs-scaling-source.md` §1.1).
- Only one destination size (960x540) was used; HVS scaling can also depend on line-buffer capacity at larger destinations (inferred from `vc4-hvs-scaling-source.md` LBM section), not measured.
- Not tested: total scaler budget with several scaled planes at once (inferred open).
