# Armadillo-X2 (i.MX8M Plus): DRM/KMS as the kernel exposes it

**Scope: evidence for SoW §5.3 ("i.MX8M Plus as a contrasting design")
only. It counts toward none of M1, M2 or M3.** This is a read-only probe of
the Atmark Techno Armadillo-X2 over ssh, run on 2026-11-28 (UTC, host clock).
No commit, `TEST_ONLY` or otherwise, was made. Nothing on the board was
changed, and nothing was stopped. The raw scrubbed dumps are in
[`observed/imx8mp-x2-2026-11-28/`](observed/imx8mp-x2-2026-11-28/README.md).

Tags: **observed 2026-11-28** = read on the board in this session.
**inferred** = reasoned from names or addresses, not read directly.
**not measured** = not looked at in this session.

## Summary

| Fact | Value | Tag |
| --- | --- | --- |
| SoC | i.MX8M Plus (`fsl,imx8mp`), revision 1.1 | observed 2026-11-28 |
| Display driver | `imx-drm` 1.0.0 ("i.MX DRM graphics") on card1. Its CRTCs come from the vendor `imx-lcdifv3` / `imx-lcdifv3-crtc` drivers | observed 2026-11-28 |
| CRTCs | 2: id 34 (index 0) is LCDIF2 → LVDS, id 37 (index 1) is LCDIF3 → HDMI | observed 2026-11-28 (see the mapping section for the inferred steps) |
| Connectors | 2: HDMI-A-1 connected (1920x1080@60 active), LVDS-1 disconnected (forced off). No DSI connector | observed 2026-11-28 |
| Planes | 2, one **primary** per CRTC. **No overlay, no cursor** | observed 2026-11-28 |
| zpos / alpha / blend / rotation / scaling props | zpos immutable `0..=0`. No `alpha`, `pixel blend mode`, `rotation`, `SCALING_FILTER` or `FB_DAMAGE_CLIPS` property | observed 2026-11-28 |
| Modifiers | `DRM_CAP_ADDFB2_MODIFIERS` = 0. No `IN_FORMATS` property | observed 2026-11-28 |
| GPU | own card0 (`galcore`, DRM name `vivante`) and renderD128. KMS queries on card0 fail with `EOPNOTSUPP` | observed 2026-11-28 |
| Display manager | none, before and after. No process held `/dev/dri/*` | observed 2026-11-28 |

## SoC identity, kernel and BSP

All observed 2026-11-28 ([`soc.txt`](observed/imx8mp-x2-2026-11-28/soc.txt)):

- `/proc/device-tree/compatible`: `atmark,yakushima-es1 fsl,imx8mp`.
  Model: `Atmark-Techno Armadillo-X2 Board`.
- `/sys/devices/soc0`: `soc_id` i.MX8MP, `revision` 1.1, `family` Freescale
  i.MX.
- Kernel: `uname -r` `6.12.105-0-at`, `uname -v` `#1-Alpine SMP PREEMPT Mon
  Aug 24 07:48:35 UTC 2026`, aarch64. The `-at` suffix and the `imx-drm` /
  `imx-lcdifv3` driver names mean this is Atmark's vendor kernel with NXP's
  downstream display stack, not mainline's `imx-lcdif` (inferred).
- BSP: Armadillo Base OS, `/etc/atmark-release` `3.24-at.3`. Distribution:
  Alpine Linux `3.24.1` (`/etc/os-release`, `/etc/alpine-release`).

## DRM devices and drivers

Observed 2026-11-28:

| Node | by-path | Device | Driver (sysfs) | DRM name |
| --- | --- | --- | --- | --- |
| card0 | `platform-40000000.mix_gpu_ml-card` | `40000000.mix_gpu_ml` | `galcore` | `vivante` 1.0.0 (dmesg) |
| renderD128 | `platform-40000000.mix_gpu_ml-render` | `40000000.mix_gpu_ml` | `galcore` | (not measured) |
| card1 | `platform-display-subsystem-card` | `display-subsystem` (`fsl,imx-display-subsystem`) | `imx-drm` | `imx-drm` 1.0.0 |

- The GPU (Galcore `6.4.11.p4.1190909` in dmesg. That it is a GC7000UL is
  not measured) has its own card,
  and that card is not a KMS device. `probe` and `props` on card0 both
  exit 1 with `Not supported (os error 95)`
  ([`probe-card0.txt`](observed/imx8mp-x2-2026-11-28/probe-card0.txt),
  [`props-card0.txt`](observed/imx8mp-x2-2026-11-28/props-card0.txt)): observed
  2026-11-28. Which ioctl returned `EOPNOTSUPP`: not measured.
- card1 is the only display card. Its index changes if the GPU module loads
  later, so a tool should select it by driver name, or by the by-path link
  `platform-display-subsystem-card`, not by `card1` (inferred).
- Driver caps on card1 (observed 2026-11-28): `ADDFB2_MODIFIERS` 0,
  `DUMB_BUFFER` 1, `DUMB_PREFER_SHADOW` 0, cursor 64x64, `ASYNC_PAGE_FLIP` 0,
  atomic async page flip 0. The universal-planes and atomic client caps
  were both accepted.
- The fbdev emulation `fb0: imx-drmdrmfb` is the current scanout owner
  (dmesg; the kernel command line also carries `console=tty1`): observed
  2026-11-28.

## CRTCs and which LCDIF each one is

| CRTC id (index) | State | Component device | Parent DT node | DT compatible | LCDIF | Encoder → connector |
| --- | --- | --- | --- | --- | --- | --- |
| 34 (0) | inactive, no mode | `imx-lcdifv3-crtc.0` | `32e90000.lcd-controller` | `fsl,imx8mp-lcdif2` | LCDIF2 | encoder 38 (possible_crtcs 0b1) → LVDS-1 (connector 39) |
| 37 (1) | active, 1920x1080@60 | `imx-lcdifv3-crtc.1` | `32fc6000.lcd-controller` | `fsl,imx8mp-lcdif1` | LCDIF3 | encoder 40 (possible_crtcs 0b10) → HDMI-A-1 (connector 41) |

- Observed 2026-11-28: the two component devices and their parents (sysfs),
  the parents' driver `imx-lcdifv3`, the bind order (`bound
  imx-lcdifv3-crtc.0`, then `.1`, in dmesg), each encoder's possible_crtcs,
  and the DT links in dmesg (the LDB's `lvds-channel@0` ↔ `32e90000`, `hdmi@32fd8000`
  ↔ `32fc6000`).
- Inferred: the CRTC index follows the component bind order (index 0 =
  `.0`, index 1 = `.1`). The encoder masks agree with the DT links, which
  supports this.
- Inferred: the names LCDIF1/2/3 come from the unit addresses (32e80000,
  32e90000, and 32fc6000 in the HDMI block). The DT compatibles do not follow
  that numbering: the LCDIF3 node carries `fsl,imx8mp-lcdif1`.
- The third LCDIF, `32e80000.lcd-controller` (`fsl,imx8mp-lcdif1`, the MIPI
  DSI path), has DT status `disabled` and no driver bound. The DSI host
  `mipi_dsi@32e60000` is `disabled` too: observed 2026-11-28.
- **Correction to the plan's premise:** on this kernel, both bound LCDIFs
  (LCDIF2 and LCDIF3) are driven by the same `imx-lcdifv3` driver. None of
  them uses an mxsfb-type driver (observed 2026-11-28). Whether LCDIF1/2 are
  a different IP revision from LCDIF3 in hardware is not measured.

## Connectors

Observed 2026-11-28 ([`probe-card1.txt`](observed/imx8mp-x2-2026-11-28/probe-card1.txt),
[`props-card1.txt`](observed/imx8mp-x2-2026-11-28/props-card1.txt)):

| Connector | Type | State | Current CRTC | Modes | Properties |
| --- | --- | --- | --- | --- | --- |
| 41 HDMI-A-1 | HDMIA | connected, enabled, DPMS On, link Good | 37 | 15 (1920x1080@60 ×2, @59.94, @50; 1280x1024@75/60; 1280x720@60 ×2, @59.94, @50; 1024x768@60; 800x600@60.3; 640x480@60, @59.94 ×2) | EDID (256 bytes, contents not dumped), DPMS, link-status, non-desktop, TILE, CRTC_ID, `max bpc` 8..=16 |
| 39 LVDS-1 | LVDS | disconnected, disabled, 0 modes | none | 0 | EDID (empty), DPMS, link-status, non-desktop, TILE, CRTC_ID |

- The active mode is 1920x1080@60 on CRTC 37: observed 2026-11-28. The
  HDMI sink is the NanoKVM Lite, so the mode list comes from its EDID
  (inferred from the farm's wiring). The board devicetree on main is written for 1920x1200 (see
  the X2 notes in `driver/tools/kexec-zephyr/README.md`). This probe only
  records what Linux drives.
- LVDS-1 is off because the kernel command line has `video=LVDS-1:d`, and
  dmesg prints `[drm] forcing LVDS-1 connector off`: observed 2026-11-28.
  Whether an LVDS panel is attached: not measured.
- DSI: no connector. A DSI connector would also need the disabled DSI host
  above: observed 2026-11-28.

## Planes per CRTC

Observed 2026-11-28. card1 exposes exactly two planes.

| Plane | Type | possible_crtcs | Attached | zpos | Formats (7) | Modifiers |
| --- | --- | --- | --- | --- | --- | --- |
| 32 | primary | 0b1 (CRTC 34, LCDIF2/LVDS) | none | immutable range `0..=0`, value 0 | XR24 AR24 RG16 XB24 AB24 AR15 XR15 | no `IN_FORMATS`. `ADDFB2_MODIFIERS` = 0 |
| 35 | primary | 0b10 (CRTC 37, LCDIF3/HDMI) | CRTC 37, FB 44, 1920x1080 at (0,0), SRC 1920x1080 | immutable range `0..=0`, value 0 | XR24 AR24 RG16 XB24 AB24 AR15 XR15 | no `IN_FORMATS`. `ADDFB2_MODIFIERS` = 0 |

Each plane exposes exactly these properties: `type`, `FB_ID`, `IN_FENCE_FD`,
`CRTC_ID`, `CRTC_X/Y/W/H`, `SRC_X/Y/W/H` and `zpos` (observed 2026-11-28).
That means:

- **Overlay planes: none on either CRTC.** Each CRTC has one primary
  plane, and nothing else (observed 2026-11-28). Whether the LCDIFv3 can
  scan out a second layer that the driver does not expose: not measured.
  Item x2-testonly checks what `TEST_ONLY` accepts.
- **Cursor plane: none**, even though `DRM_CAP_CURSOR_WIDTH/HEIGHT` reports
  64: observed 2026-11-28. That 64 is the DRM core's fallback when a driver
  sets no cursor size, not a cursor plane (inferred).
- **zpos:** present, immutable, range `0..=0` (observed 2026-11-28).
- **Plane alpha (`alpha`):** absent (observed 2026-11-28).
- **Pixel blend mode (`pixel blend mode`):** absent (observed 2026-11-28).
  `probe` prints `blend [premultiplied]`. That is composition-drm's default
  when the property is absent, not something the kernel reported (inferred
  from the example's output path).
- **Rotation (`rotation`):** absent (observed 2026-11-28).
- **Scaling:** no `SCALING_FILTER`, no `FB_DAMAGE_CLIPS` and no scaler
  property (observed 2026-11-28). `probe` prints `scaling not claimed`.
  Whether the kernel accepts `SRC_W/H` ≠ `CRTC_W/H`: not measured
  (x2-testonly, `scale_probe`).
- **Color encoding / range (`COLOR_ENCODING`, `COLOR_RANGE`):** absent, and
  no YUV format is listed (observed 2026-11-28).
- The CRTCs expose only `ACTIVE`, `MODE_ID`, `OUT_FENCE_PTR` and
  `VRR_ENABLED`. They have no `GAMMA_LUT`, `DEGAMMA_LUT`, `CTM` or
  background-colour property (observed 2026-11-28).

What composition-drm derives (`probe` header, observed 2026-11-28): one
output (CRTC 37, 1920x1080 @ 60.000 Hz), and two primary planes with zpos
fixed 0, plane alpha false, blend [premultiplied], scaling not claimed and
the same seven formats. Plane 32 has outputs `0b0`, because its CRTC drives
no connector.

## Display-manager state

- Before (observed 2026-11-28): no compositor, X server, seatd or other
  display process. No process had `/dev/dri/*` open. OpenRC default runlevel:
  `armadillo-twin-agentd`, `avahi-daemon`, `sshd`, `abos-web`, `buttond`,
  `chronyd`, `podman-atmark`, `reset_bootcount`, `zramswap`, `local`. Podman
  containers `slint-home-automation` and `slint-launcher` were both
  `Exited`. The HDMI picture is the fbdev console.
- `scripts/display-guard.sh` was not used and nothing was stopped, because
  the examples need no DRM master and nothing held the card.
- After (observed 2026-11-28): the same default-runlevel services started,
  still no DRM fd holder, HDMI-A-1 still `connected` and LVDS-1 still
  `disconnected`. The binaries and dumps were removed from `/tmp` on the
  board.

## Not measured

- Anything that needs a commit: `TEST_ONLY` plane acceptance, overlay use,
  scaling, and format/modifier acceptance at commit time. These belong to
  item x2-testonly.
- The screen picture: no KVM was leased for this item.
- The EDID contents (only its size, 256 bytes, was read).
- Which ioctl fails on card0, and the GPU's render-node capabilities.
- Whether an LVDS panel is physically attached, and the LVDS-1 modes with
  `video=LVDS-1:d` removed (that would be a boot-chain change).
- The LCDIFv3 hardware's own layer count and blending, independent of what
  the driver exposes. Driver source reading belongs to plan 462 /
  x2-lcdif-profile.
- debugfs (`/sys/kernel/debug/dri` is not mounted).
