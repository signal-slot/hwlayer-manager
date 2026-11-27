# BCM2711 (Raspberry Pi CM4, reTerminal) DRM topology

All rows tagged **observed 2026-11-28** unless marked otherwise. Topology only: the two-plane
harness run (`twoplane`, unsupported combinations, swap-z and alpha variants) is in
[`bcm2711-cm4.md`](bcm2711-cm4.md), which also corrects the plane counts and the scaling row below.
`crates/composition-drm/examples/props.rs` did not exist when this page was written, so there is no
raw KMS property dump (see "Discrepancies").

> **Plane counts here are probe's cut view, not the kernel's.** composition-drm keeps only
> `MAX_PLANES` = 16 planes, primaries first. The kernel's atomic state has 60 planes, and the DSI
> CRTC can use 16 shared overlays plus its primary (and a cursor, inferred). See "Topology" in
> [`bcm2711-cm4.md`](bcm2711-cm4.md). Do not fold the 16 / 10 / 11 counts below into
> `docs/platform-capabilities.md` as the hardware's plane count.

## Setup

| Item | Value | Tag |
|---|---|---|
| Board | Seeed reTerminal, Raspberry Pi Compute Module 4 Rev 1.0 (BCM2711) | observed 2026-11-28 |
| OS / userland | Raspberry Pi OS trixie, aarch64 (64-bit, `uname -m` = aarch64) | observed 2026-11-28 |
| Kernel | `Linux 6.12.47+rpt-rpi-v8 #1 SMP PREEMPT Debian 1:6.12.47-1+rpt1 (2025-09-16) aarch64` | observed 2026-11-28 |
| DRM driver | `vc4` (`vc4-drm` platform driver, card0); card1 / renderD128 is `v3d` (render only) | observed 2026-11-28 |
| Primary DRM device | `/dev/dri/card0` | observed 2026-11-28 |
| Connected connector | `DSI-1` (built-in DSI panel); `HDMI-A-1`, `HDMI-A-2` disconnected; `Writeback-1` status unknown | observed 2026-11-28 |
| Active mode | `720x1280` @ 60 Hz (clock 62712 kHz, htotal 780, vtotal 1340), portrait | observed 2026-11-28 |
| CRTC driving DSI-1 | CRTC object 90 = `pixelvalve-1`, HVS channel 0 in use, fbcon on plane 79 (RG16, 720x1280) | observed 2026-11-28 |
| DRM master | Nobody but the kernel fbcon: no compositor/desktop running (the slint-launcher service is stopped and disabled by the owner). Nothing was stopped or restarted for this run. | observed 2026-11-28 |
| Display capture | **No KVM exists for this board and nothing captures the panel.** No picture-based check (z-order, alpha) is possible here; only state from KMS / debugfs. | observed 2026-11-28 |
| Build | `cargo build --release -p composition-drm --example probe` run on the board itself (cargo present, native aarch64), source piped over ssh into a scratch dir under `/tmp`, removed afterwards | observed 2026-11-28 |
| `modetest` | not installed; not used | observed 2026-11-28 |

## Topology summary (from probe and `/sys/kernel/debug/dri/0/state`)

| Item | Value | Tag |
|---|---|---|
| CRTCs | 6: `txp` (59, writeback), `pixelvalve-0` (78), `pixelvalve-1` (90), `pixelvalve-2` (102), `pixelvalve-4` (114), `pixelvalve-3` (126) | observed 2026-11-28 |
| Active CRTC | only `pixelvalve-1` (enable=1, plane_mask=0x4, connector DSI-1) | observed 2026-11-28 |
| Planes probe reports | 16: 6 `Primary` (ids 48, 67, 79, 91, 103, 115 = debugfs plane-0..5), 10 `Overlay` (ids 127 … 226). This is probe's view cut to `MAX_PLANES`; the kernel exposes 60 planes (`bcm2711-cm4.md`, "Topology") | observed 2026-11-28 (probe's cut view) |
| Planes usable on the DSI CRTC, as probe sees them | probe mask `0b1` for output 0: primary 79 plus the ten overlays probe kept (127, 138, 149, 160, 171, 182, 193, 204, 215, 226) = 11 planes. Primaries 48, 67, 91, 103, 115 belong to other CRTCs (mask `0b0`). The kernel gives this CRTC 16 shared overlays plus the primary (and a cursor, inferred); 11 is the engine's cut view (`bcm2711-cm4.md`, "Topology") | observed 2026-11-28 (probe's cut view) |
| zpos | primaries: fixed 0; overlays: range 1..=17 (as probe derives it) | observed 2026-11-28 |
| Plane alpha | true on all 16 planes probe reports | observed 2026-11-28 |
| Pixel blend modes | `none`, `premultiplied`, `coverage` on all 16 planes probe reports | observed 2026-11-28 |
| Formats | identical 32-entry list on every plane (below), includes AR24/AB24 (alpha), NV12/YUV planar, XR30/AR30 | observed 2026-11-28 |
| Modifiers | not reported by probe (it prints no modifier column) | observed 2026-11-28 |
| Scaling | probe prints "scaling not claimed" for every plane. That is the engine's conservative assumption, not a read property. The kernel accepted scale-up and scale-down in a later `TEST_ONLY` run (`bcm2711-cm4.md`, "Unsupported combinations") | inferred (probe); observed 2026-11-28 (`TEST_ONLY`, in `bcm2711-cm4.md`) |
| Rotation | debugfs state shows `rotation=1` (bit 0 = rotate-0) on all planes, i.e. a rotation property exists; its supported mask was not read (no props dump) | observed 2026-11-28 (current value only) |
| HVS state | core clock 71428000 Hz, channel 0 in use (load 71428000), channel 1 idle | observed 2026-11-28 |
| Underruns | `hvs_underrun` debugfs counter read 0 | observed 2026-11-28 |

Format list per plane (XR24 = XRGB8888, AR24 = ARGB8888, AB24 = ABGR8888, XB24 = XBGR8888, RG16 = RGB565, …):
`XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12`

## Raw probe output

`probe /dev/dri/card0`, exit code 0 (formats lines are as printed):

```text
device /dev/dri/card0, driver vc4
output 0: crtc 90 720x1280 @ 60.000 Hz
plane 48 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 67 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 79 Primary: outputs 0b1, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 91 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 103 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 115 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 127 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 138 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 149 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 160 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 171 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 182 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 193 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 204 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 215 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
plane 226 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
```

Each plane is followed by `  formats (32): …` with the list above (all 16 identical; omitted here
for brevity). The 16 planes are probe's cut view (see the note at the top).

## Debugfs state (read-only, `/sys/kernel/debug/dri/0/state`)

Abridged:

```text
crtc[90]: pixelvalve-1  enable=1 active=1 plane_mask=4 connector_mask=4 encoder_mask=4
    mode: "720x1280": 60 62712 720 730 750 780 1280 1290 1310 1340 0x48 0x0
plane[79]: plane-2  crtc=pixelvalve-1 fb=722 (allocated by fbcon) format=RG16 modifier=0x0 size=720x1280
    crtc-pos=720x1280+0+0 src-pos=720.0x1280.0+0+0 rotation=1 normalized-zpos=0
connector[47]: DSI-1  crtc=pixelvalve-1
all other crtcs/planes/connectors: crtc=(null), fb=0
```

## Discrepancies and caveats

- probe lists only connected outputs, so only the DSI CRTC (pixelvalve-1) appears as "output 0". The
  other five CRTCs exist in debugfs but are not in probe's view (HDMI unplugged; txp is writeback).
  The primary planes of those CRTCs show mask `0b0`, which means "not on a connected output", not
  "no CRTC": do not read it as a possible_crtcs of zero.
- All six primaries are typed `Primary` although only plane 79 can reach the DSI CRTC. On this
  vc4 each CRTC has its own primary plane; the overlays are shared and all reach the DSI CRTC.
  probe shows ten of them; the kernel has 16 shared overlays (plus 32 TXP overlays and six
  cursors), see `bcm2711-cm4.md`, "Topology".
- Overlay zpos range 1..=17 is probe's derived reading of the zpos property range; the raw
  min/max/immutable flags were not dumped (no props example in this snapshot), so this is not
  cross-checked against the raw property.
- Scaling: probe's "not claimed" is an assumption. The later `TEST_ONLY` run accepted both
  scale-up and scale-down (`bcm2711-cm4.md`, "Unsupported combinations"); limits were not probed.
- Rotation: only the current value (`rotation=1`) was seen; supported rotate/reflect bits unread.
- `hvs_underrun` read 0 only on an idle fbcon-only scanout; it says nothing about two-plane load.
- Panel is portrait 720x1280; any harness frame sizes must be chosen for that orientation.
- No KVM / capture: nothing visual was verified.

## Run hygiene

Files placed only under `/tmp` on the board (source, `target/`), removed at the end. Cargo's registry
cache in the board user's home (`~/.cargo`) was populated by the build (crates fetched for the build);
it was left in place. No packages installed, no boot or display configuration changed; no compositor
was running before or after (`slint-launcher` and `lightdm` inactive, slint-launcher disabled, both
checked over ssh before and after).
