# STM32MP257F-DK (LTDC) display topology

Tags: `observed 2026-11-28` = read from the live board in this session.
`inferred` = derived by me from observed rows. Nothing here is "read from
source". Board identity (addresses, names, serials) is deliberately omitted.

## System

| Item | Value | Tag |
|---|---|---|
| Kernel | `Linux 6.6.129 #1 SMP PREEMPT Thu Mar 5 15:03:43 UTC 2026 aarch64` (ST OpenSTLinux) | observed 2026-11-28 |
| DRM device | `/dev/dri/card0`, only one card | observed 2026-11-28 |
| Driver | `stm32-display` platform device (DT `display-controller@48010000`); DRM driver name `stm` | observed 2026-11-28 |
| Connector | `HDMI-A-1` (id 32), connected, encoder 31; the only connector in `/sys/class/drm` | observed 2026-11-28 |
| Sink | EDID name "NanoKVM" (the KVM capture unit), 480x270 mm | observed 2026-11-28 |
| Active mode | 1920x1080 @ 60.000 Hz, pixel clock 148500 kHz, htotal 2200, vtotal 1125, phsync/pvsync | observed 2026-11-28 |
| Other modes | 1920x1080 59.94/50, 1280x720 60/59.94/50, 800x600 75, 720x400 70.08 | observed 2026-11-28 |
| Display client | Weston (`weston-graphical-session`) owns the CRTC, scanning out an XR24 1920x1080 linear buffer on plane 34 | observed 2026-11-28 |
| CRTCs | 1 (id 41), possible_crtcs mask `0x1` on every plane | observed 2026-11-28 |
| CRTC properties | `VRR_ENABLED` (0..1), `GAMMA_LUT`, `GAMMA_LUT_SIZE`=256; no `ACTIVE`/`MODE_ID` lines shown by modetest | observed 2026-11-28 |

## Planes (LTDC layers)

| id | type | formats | extra | zpos | alpha | rotation |
|---|---|---|---|---|---|---|
| 34 | Primary (layer 0) | 21: AR24 XR24 AB24 XB24 RA24 RX24 BA24 BX24 RG16 BG16 RG24 BG24 AR15 XR15 AR12 XR12 C8 YUYV YVYU UYVY VYUY | COLOR_ENCODING, COLOR_RANGE | 0..2 range, default 0 | 0..65535, default 65535 | rotate-0/90/180/270, reflect-x, reflect-y |
| 42 | Overlay (layer 1) | the same 21 plus NV12 NV21 YU12 YV12 (25) | COLOR_ENCODING, COLOR_RANGE | 0..2, default 1 | 0..65535 | rotate-0, reflect-x, reflect-y only |
| 49 | Overlay (layer 2) | 17: the RGB formats above plus C8, no YUV | none | 0..2, default 2 | 0..65535 | rotate-0, reflect-x, reflect-y only |

All rows: observed 2026-11-28 (modetest `-p`).

- Modifiers: every format is `LINEAR(0x0)` only, per the decoded `IN_FORMATS` blob. Observed.
- `zpos` is a mutable range 0..2 on all three planes (not immutable). Observed.
- There is **no `pixel blend mode` property** and **no scaling property** on
  any plane. Observed. Whether the hardware scales is not shown by the
  properties; the debugfs state shows `src-pos` equal to the CRTC size only.
- Debugfs state before the run: plane 34 bound to CRTC 41, planes 42 and 49
  unbound, `normalized-zpos` 0/1/2, `user_updates=0fps`. Observed.
- Plane 42 is the only plane with YUV 4:2:0 (NV12, NV21, YU12, YV12). Observed.

## Probe's derived view vs the raw properties

Raw `probe` output, run unstripped over the board's glibc from a
cross-built aarch64 binary (Weston kept running; probe needed no DRM master):

```
device /dev/dri/card0, driver stm
output 0: crtc 41 1920x1080 @ 60.000 Hz
plane 34 Primary: outputs 0b1, zpos 0..=2, plane alpha true, blend [premultiplied], scaling not claimed
  formats (21): AR24 XR24 AB24 XB24 RA24 RX24 BA24 BX24 RG16 BG16 RG24 BG24 AR15 XR15 AR12 XR12 C8?? YUYV YVYU UYVY VYUY
plane 42 Overlay: outputs 0b1, zpos 0..=2, plane alpha true, blend [premultiplied], scaling not claimed
  formats (25): AR24 XR24 AB24 XB24 RA24 RX24 BA24 BX24 RG16 BG16 RG24 BG24 AR15 XR15 AR12 XR12 C8?? YUYV YVYU UYVY VYUY NV12 NV21 YU12 YV12
plane 49 Overlay: outputs 0b1, zpos 0..=2, plane alpha true, blend [premultiplied], scaling not claimed
  formats (17): AR24 XR24 AB24 XB24 RA24 RX24 BA24 BX24 RG16 BG16 RG24 BG24 AR15 XR15 AR12 XR12 C8?? 
```
(The trailing space on the last line is not significant.)

Discrepancies and notes (observed 2026-11-28 unless marked):

1. `blend [premultiplied]` is **not a property of the hardware**: the planes
   have no `pixel blend mode`; `crates/composition-drm/src/lib.rs` fills in
   Premultiplied when the property is absent ("KMS blends as premultiplied").
   Whether the LTDC blends premultiplied (vs straight alpha) is therefore
   *inferred*, not observed; the two-plane run must check it on the KVM.
2. `C8??` : probe prints `??` for C8 (palette format, not mapped by the
   engine); the raw plane advertises C8 on all three planes.
3. Probe omits the sink identity, rotation, `COLOR_ENCODING/RANGE`, and the
   CRTC `VRR_ENABLED`/`GAMMA_LUT` properties. Rotation (90/270 on plane 34
   only) is not shown by probe at all.
4. `scaling not claimed` is the engine's default; the properties offer no
   scaling control. Observed absence, hardware capability unknown.
5. `alpha true` matches the raw `alpha` property (0..65535) on all planes.
6. `plane_mask=1` in the debugfs CRTC state: only plane 34 is attached,
   although all three can reach CRTC 41 (mask `0x1`).

## Build used (not in the repository)

`probe` was cross-built for `aarch64-unknown-linux-gnu` with `rust-lld`
against copies of the board's own `libc.so.6`, `libm.so.6`, `libgcc_s.so.1`
(read from the board, empty stubs for libutil/librt/libpthread/libdl) and a
hand-written `_start` object, because the host has no aarch64 gcc and the
musl rust-std install failed. The binary was copied to `/tmp`, run, and
removed. `props.rs` does not exist in this snapshot, so raw properties come
from `modetest -M stm -p/-c` (read-only) and the debugfs `state` file.

## KVM baseline

The KVM MCP server (`kvm_kvm-stm32mp257f-dk`) failed to connect in this
session: `HTTP 404` on its `p2p` MCP endpoint. Its credentials live on another
node, so the capture could not be reached another way. **No screenshot was
taken; whether the KVM shows a picture is unverified.** What is known:

- observed 2026-11-28: the board's HDMI connector reports connected and the
  sink EDID is named "NanoKVM", so the board detects the KVM as a
  1920x1080 sink.
- observed 2026-11-28: Weston scans out a 1920x1080 XR24 buffer on the CRTC
  with `active=1`, so a picture is being sent; its content was not seen.

## State left

Weston was never stopped (probe and modetest read without master);
`weston-graphical-session` was `active` after the run. `/tmp/probe` removed.
