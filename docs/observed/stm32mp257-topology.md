# STM32MP257F-DK (LTDC) display topology

Tags: `observed 2026-11-28` = read from the live board in this session.
`inferred` = derived from observed rows. `read from source` = stated by a
source file; it appears only in the "Session C additions (2026-11-28)" section
at the end, copied from the session C outputs file. Every row outside that
section is observed or inferred, with no read-from-source claim. Board identity
(addresses, names, serials) is deliberately omitted.

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

The two-plane harness run (`twoplane`, unsupported combinations, swap-z and
alpha variants) is in [`stm32mp257.md`](stm32mp257.md).

## State left

Weston was never stopped (probe and modetest read without master);
`weston-graphical-session` was `active` after the run. `/tmp/probe` removed.

## Session C additions (2026-11-28)

Copied from [`../stm32mp257-ltdc-session-c-outputs-2026-11-28.md`](../stm32mp257-ltdc-session-c-outputs-2026-11-28.md)
(Q8 other outputs, Q4 scaler layers), with the tags it gives. Observed rows
were read over ssh, read only, Weston running; the host UTC time of the reads
was 2026-11-28T01:04Z to 2026-11-28T01:06Z. Read-from-source rows come from ST's
public kernel tree, branches `v6.18-stm32mp` and `v6.6-stm32mp`, via a
web-fetch agent; that the fetched files are the tip commits recorded in the
outputs file is itself inferred.

### Q8: encoder chain and routes

| Item | Value | Tag |
|---|---|---|
| Encoders | One (id 31). `bridge_chains` shows a two-bridge chain: the DSI host `dsi@48000000` (`st,stm32mp25-dsi`), then the ADV7535 at `hdmi@3d` (`adi,adv7535`) on I2C, type HDMI-A, ending at `HDMI-A-1` | observed 2026-11-28 |
| `possible_crtcs` | One CRTC (id 41); every plane has `crtc-0` available; connector, encoder and plane masks are 1. The encoder's `possible_crtcs` is not shown by sysfs or `state`; `0x1` follows from the single CRTC and `encoder_mask=1` | observed 2026-11-28 (masks), inferred (encoder `possible_crtcs`) |
| Other connectors, encoders | None: no LVDS, DSI or DPI connector in `/sys/class/drm` or the debugfs `state` | observed 2026-11-28 |
| DRM clients | Only `seatd` holds master (Weston runs under it) | observed 2026-11-28 |
| Error counters | `transfer_error=0`, `fifo_underrun_error=0`, `fifo_underrun_warning=0` | observed 2026-11-28 |
| DSI route | LTDC, DSI host, ADV7535, HDMI-A connector: exists and is live | observed 2026-11-28 |
| LVDS route | LTDC `endpoint@1` points at `lvds_in`, but `lvds@48060000`, `panel-lvds` and its backlight are `disabled`, no LVDS device is bound, no connector appears; not usable without a device-tree change | observed 2026-11-28 |
| Both at once | The binding says the two LTDC endpoints "cannot be activated simultaneously"; on this image only DSI is enabled | read from source (binding), observed 2026-11-28 (the state) |
| Implication for the backend | One encoder with a DSI bridge chain and one connector: one output, no output selection on this image | inferred |

### Q8: device tree (`/proc/device-tree`)

| Node | `compatible` | `status` | Tag |
|---|---|---|---|
| `display-controller@48010000` (LTDC) | `st,stm32mp25-ltdc` | okay | observed 2026-11-28 |
| `dsi@48000000` | `st,stm32mp25-dsi` | okay | observed 2026-11-28 |
| `i2c@40130000/hdmi@3d` | `adi,adv7535` | okay | observed 2026-11-28 |
| `lvds@48060000` | `st,stm32mp25-lvds` | **disabled** (LVDS) | observed 2026-11-28 |
| `panel-lvds` | `edt,etml0700z9ndha`, `panel-lvds` | **disabled** | observed 2026-11-28 |
| `panel-lvds-backlight` | `pwm-backlight` | **disabled** | observed 2026-11-28 |
| `connector` with `hdmi-pwr-supply` | HDMI-A connector node | present | observed 2026-11-28 |

LTDC details, observed 2026-11-28: `port` has `endpoint@0` (remote phandle
`0x50`, the DSI input `dsi_in`) and `endpoint@1` (remote `0x51`, the LVDS input
`lvds_in`). The firewall children `l1l2`, `l3`, `rot` carry RIFSC ids `0x77`,
`0x78`, `0x79` (119, 120, 121) and the node `0x50` (80). The reserved-memory
nodes `ltdc-sec-layer@fe800000` and `ltdc-sec-rotation@ff000000` exist. The
LTDC node has an `st,syscon` property; no `rotation-memory` property was seen
in the listing, and whether the driver finds the secured rotation memory was
not checked.

### Q8: display modules (`lsmod`, read only)

Loaded, observed 2026-11-28: `stm_drm`, `dw_mipi_dsi_stm`, `dw_mipi_dsi`,
`adv7511` (with `cec`), `drm_dma_helper`, `galcore`. No LVDS, panel or backlight
module appears. The device `48000000.dsi` is bound; no `48060000` LVDS device
is among the platform devices.

### Q4: scaler layers per variant

| Finding | Result | Tag |
|---|---|---|
| How the driver learns which layer scales | At probe, `ltdc_get_caps()` (`case HWVER_40100: case HWVER_40101:`) reads `LTDC_L1C1R + i * LAY_OFS` per layer and sets `plane_scaling[i]` from bit 31, `LXCR_C1R_SCA`. Same code on both branches | read from source |
| A static per-variant or per-layer table | None. The only layer comment is about YUV semi-planar/planar formats on layer 1, not scaling | read from source |
| Which layers scale | Not stated by any source file; a hardware register value. On the board plane 42 (layer 2) scales and plane 49 (layer 3) does not (earlier file); plane 34 not yet probed | open in source; observed 2026-11-27 for 42/49 |
| MP251, MP253 | Same LTDC node; no DSI, LVDS or GPU nodes; same scaler registers and code path | read from source |
| MP255, MP257 | `stm32mp255.dtsi` adds `dsi`, `lvds` (both `disabled` by default) and `gpu`; same scaler code | read from source |
| Per-variant scaler difference | None in the driver or device tree; identical silicon is not stated by any file | inferred |
| Public reference manual | Not read; ST's layer-by-layer scaler capability is unchecked | open |
| Scaling limits | Upscale only; YUV never scaled; a layer without the scaler refuses size mismatch; no numeric cap in the code | read from source |
| Firewall or RIF: layers | When access to `l3` is refused, `nb_layers` is decremented (layer 3 not exposed) and `caps.crc` is cleared; refusal of `cmn`, `l1l2` or `rot` fails the probe | read from source |
| Firewall or RIF: scaler | Does not change which layers scale; with no usable `rotation-memory`, `ltdc_load()` clears `crtc_rotation` | read from source |
| Firewall on this board | Three planes exist, so `l3` access was granted and `nb_layers` was not reduced | observed 2026-11-28 (properties), observed 2026-11-27 (three planes) |
| Other variants' firewall settings | Same `access-controllers` in every variant's device tree; whether a boot chain denies `l3` is a runtime RIF property | inferred |
