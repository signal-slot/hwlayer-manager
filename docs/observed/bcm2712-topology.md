# BCM2712 (Raspberry Pi 5) display topology

Tags follow `docs/platform-capabilities.md`: **observed** = measured on the board on the
stated date, **inferred** = derived from observed data, not measured directly. Nothing here is
read from source. This file records topology only; no two-plane run has happened yet.

## Setup — observed 2026-11-28

| Item | Value | Tag |
|---|---|---|
| Board | Raspberry Pi 5 Model B Rev 1.1 | observed 2026-11-28 |
| OS / kernel | Debian 13 (trixie), `6.18.50+rpt-rpi-2712 #1 SMP PREEMPT Debian 1:6.18.50-1+rpt1 (2026-09-11) aarch64` | observed 2026-11-28 |
| DRM cards | `card0` = `v3d` (render only, `renderD128`), `card1` = `vc4-drm` (`axi:gpu`, compatible `brcm,bcm2712-vc6`, KMS) | observed 2026-11-28 |
| KMS card used | `/dev/dri/card1`, driver name reported by probe: `vc4` (kernel log: `Initialized vc4 0.0.0 for axi:gpu on minor 1`) | observed 2026-11-28 |
| Connectors | HDMI-A-1 connected, HDMI-A-2 disconnected, Writeback-1/-2 (TXP "mop"/"moplet") | observed 2026-11-28 |
| Active mode | 1920x1080 @ 60.000 Hz, 148.500 MHz, pinned by `video=HDMI-A-1:1920x1080M@60` on the kernel command line | observed 2026-11-28 |
| CRTCs | 4 (ids 59, 78, 94, 106); only CRTC index 2 (id 94) is active, driving HDMI-A-1 | observed 2026-11-28 |
| Encoders | 2 TMDS + 2 VIRTUAL (writeback) | observed 2026-11-28 |
| Planes | 56 in total (universal planes enabled) | observed 2026-11-28 |
| DRM master | Nobody holds it: no display manager or compositor running (lightdm, gdm3, sddm, labwc inactive); only fbcon scans out | observed 2026-11-28 |
| Scanout when idle | fbcon, plane 83, fb 680, 1920x1080 `RG16`, modifier 0, pitch 3840 | observed 2026-11-28 |
| Dropped frames / underruns | `hvs_underrun` debugfs reads `0` | observed 2026-11-28 |

No compositor was running, so none was stopped or restarted; the state after the run is the same
(all four units inactive, no process holding `card1`). The probe binary was copied to `/tmp` on
the board and removed again. Opening `card1` for reading and running probe did not need DRM master.

## Plane layout — observed 2026-11-28 (masks from `kmsprint -l`, counted)

Mask entries are CRTC indices (0..3).

| Group | Count | possible CRTCs | Type | zpos |
|---|---|---|---|---|
| Primary of CRTC 0 / 1 / 2 / 3 | 1 each (ids 48, 67, 83, 95) | its own CRTC only | Primary (`type` = 1) | `zpos` immutable, 0..0 |
| Shared overlays | 16 (ids 107, 118, ... up to 272) | CRTCs 1, 2, 3 | Overlay (`type` = 0) | mutable range 1..17 |
| CRTC-0-only planes | 34 | CRTC 0 only | not individually read | not read |
| One extra plane per CRTC 1/2/3 | 1 each (ids 646, 657, 668) | its own CRTC only | not read; inferred to be the cursor planes | not read |

So the active HDMI-A-1 output (CRTC index 2) can use its primary 83, any of the 16 shared overlays,
and plane 657. Two real overlay planes with independent buffers are therefore available on the
connected output. The 34 CRTC-0-only planes: the kernel table does not say how they are split
between overlay and other types; types were read for planes 48, 83, 107, 239 and 382 only. — inferred

### Per-plane properties — observed 2026-11-28 (via `DRM_IOCTL_MODE_OBJ_GETPROPERTIES`, planes 48, 83, 107, 239, 382)

All five planes expose the same property set:

| Property | Value range / enum |
|---|---|
| `type` | Overlay=0, Primary=1, Cursor=2 (immutable) |
| `IN_FORMATS` | blob, 45 formats, 5 modifiers (below) |
| `alpha` | 0..65535, default 0xffff (plane alpha) |
| `pixel blend mode` | None=2, Pre-multiplied=0, Coverage=1; default Pre-multiplied |
| `rotation` | rotate-0, rotate-180, reflect-x, reflect-y (no 90/270); default rotate-0 |
| `COLOR_ENCODING` | BT.601 / BT.709 / BT.2020, default BT.709 |
| `COLOR_RANGE` | limited / full, default limited |
| `SCALING_FILTER` | Default / Nearest Neighbor |
| `CHROMA_SITING_H`, `CHROMA_SITING_V` | 0..65536 |
| `zpos` | primary 0..0 (immutable); overlays 1..17 (mutable), values at the time of the read: plane 107 = 1, 239 = 13, 382 = 10 |

Note the `rotation` enum values the kernel reports are bit *positions* (0, 2, 4, 5) of the usual
mask bits (0x1, 0x4, 0x10, 0x20), as `kmsprint` shows them. There is no scaling-capability
property in KMS; the HVS scaler is not described by any property. — observed

### IN_FORMATS (identical on all five planes) — observed 2026-11-28

| Modifier | Formats |
|---|---|
| `0x0000000000000000` (linear) | XR24 AR24 AB24 XB24 RX24 RA24 BX24 BA24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12 RX12 RA12 P010 P012 P016 S010 S012 S016 |
| `0x0700000000000001` (Broadcom vendor, code 1 = VC4 T-tiled) | XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 |
| `0x0700000000000003` (Broadcom, code 3 = SAND128) | NV12 NV21 |
| `0x0700000000000004` (Broadcom, code 4) | NV12 NV21 P030 |
| `0x0700000000000005` (Broadcom, code 5) | NV12 NV21 |

The modifier codes were read as numbers; matching the codes to SAND128/SAND256/SAND30 names is
from memory of `drm_fourcc.h`, not read from a header on the board. — inferred

## Raw probe output — observed 2026-11-28

`probe /dev/dri/card1`, built from `crates/composition-drm/examples/probe.rs` at commit
93f3db6 for `aarch64-unknown-linux-musl` (static, linked with `rust-lld`), run on the board.
Formats lines are identical for every plane and are reproduced once; the first form
repeats for each listed plane.

```
device /dev/dri/card1, driver vc4
output 0: crtc 94 1920x1080 @ 60.000 Hz
plane 48 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 67 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 83 Primary: outputs 0b1, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 95 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 107 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 118 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 129 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 140 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 151 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 162 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 173 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 184 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 195 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 206 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 217 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 228 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  (after each plane line:)
  formats (32): XR24 AR24 AB24 XB24 RX24 RA24 BX24 BA24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12
```

## Discrepancies between probe's derived view and the raw properties — observed 2026-11-28

1. **Plane list truncated to 16.** The kernel exposes 56 planes; probe lists 16 (4 primaries and the
   first 12 overlays by id). `MAX_PLANES` in `crates/composition-core/src/limits.rs` is 16, so
   the engine's view drops 40 planes, including the last four of the 16 shared overlays
   (239, 250, 261, 272) and the extra per-CRTC plane 657. Two-plane use is unaffected; a backend
   that wants every plane of the BCM2712 HVS cannot represent it with `MAX_PLANES` = 16.
   (cause of the cut: inferred from the matching constant, not traced in code.)
2. **Formats: 32 listed versus 45 in the kernel.** probe prints 32 fourccs, exactly the first 32 of
   the linear-modifier list in `IN_FORMATS` (XR24 ... AR12). It stops there because
   `MAX_FORMATS` in `crates/composition-core/src/limits.rs` is 32 (observed value; lib.rs says
   format/modifier pairs beyond the limit are dropped, linear first). The 13 dropped linear
   formats are XB12 AB12 BX12 BA12 RX12 RA12 P010 P012 P016 S010 S012 S016 (and P030 under a
   Broadcom modifier). probe shows no modifiers at all, so the four Broadcom modifiers above are
   not visible in its view.
3. **Rotation not shown.** probe prints nothing about `rotation`; raw property supports
   rotate-0/180 and reflect-x/y only.
4. **Scaling "not claimed".** probe reports "scaling not claimed" because KMS has no scaling
   property; the HVS can scale (SRC_* and CRTC_* sizes differ), which was not tested here.
   Capability for it stays unverified.
5. **Outputs.** probe shows one output (the connected CRTC 94). Planes bound only to other CRTCs
   appear with mask `0b0`; this is correct for the engine's output list but hides CRTC 0..3 layout.
6. **Zpos.** probe's "zpos fixed 0" for primaries and `1..=17` for overlays match the raw
   `zpos` ranges (0..0 and 1..17). Overlay zpos is mutable. No discrepancy.
7. **Alpha / blend.** probe's `plane alpha true` and `[none, premultiplied, coverage]` match the raw
   properties; the default blend mode is Pre-multiplied, not None (probe does not show the default).

## KVM baseline — observed 2026-11-28

The NanoKVM Lite capturing HDMI-A-1 returned a picture: 1920x1080, one screenshot, 49,813 bytes
(JPEG). It shows the text console (tty1, fbcon) on a dark grey background: the `Debian GNU/Linux 13
... tty1` login banner, the network address line, a login prompt with two kernel boot-stage
messages, and a blinking cursor. This matches the `RG16` fbcon scanout on plane 83 read from
debugfs. The screenshot is not committed (it contains the board's address); no z-order or alpha
check was done, since no overlay was set. The KVM therefore gives a usable picture for the later
two-plane check.

## Other observations — observed 2026-11-28

- Kernel log: `vc4-drm axi:gpu: bound 107c580000.hvs (ops vc4_hvs_ops [vc4])`, HDMI 0/1, the
  mop/moplet writeback units and two pixelvalves are bound; HVS has an IOMMU
  (`bcm2712_iommu_*`).
- debugfs `/sys/kernel/debug/dri/1/` has `hvs_dlists`, `hvs_dlist_allocs`, `hvs_lbm`,
  `hvs_upm`, `hvs_regs`, `hvs_underrun`, `crtc-N`, `state`; `state` shows every plane with
  `normalized-zpos` and the active fb. Readable with sudo.
- `modetest` is not installed on the board; `kmsprint` (libkms++) was used, and a short Python
  ioctl reader for plane properties and `IN_FORMATS` (not part of the repository).
- `props` example (`crates/composition-drm/examples/props.rs`) does not exist in this snapshot, so
  it was not run.
- Build: the probe example builds for `aarch64-unknown-linux-musl` with
  `--config target.aarch64-unknown-linux-musl.linker="rust-lld"` and
  `rustflags=["-C","linker-flavor=ld.lld"]`; the host `cc` (x86 binutils) cannot link it.
