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

## TEST_ONLY results

**Scope: SoW §5.3 evidence only. It counts toward none of M1, M2 or M3.
A `TEST_ONLY` acceptance shows what the kernel's atomic check allows. It is
not proof of on-screen composition.** This is item x2-testonly of plan 463,
run on 2026-11-28 (UTC, host and board clock agree to within a minute,
09:24-09:28Z). `kms_sweep`, `scale_probe` and `plane_commit` were built with
`scripts/cross-aarch64.sh -p composition-drm --example kms_sweep --example
scale_probe --example plane_commit` at commit bbe4967 (unchanged examples),
copied to `/tmp` on the board, run as root over ssh on `/dev/dri/card1`, and
removed afterwards. Raw outputs went through `scripts/scrub-kms-dump.py`
(0 replacements in each) and were read by hand.

Tags: **observed 2026-11-28** = printed on the board, or seen on the KVM, in
this session. **inferred** = reasoned, not read. **not measured** = not
tried.

### Answer: overlay on the LCDIFv3 CRTC

**No overlay plane is accepted on CRTC 37 (LCDIF3 → HDMI), because none
exists. Only the primary plane 35 is accepted, and only as a full-screen,
unscaled layer at (0,0)** (observed 2026-11-28):

- `kms_sweep combos` finds one candidate plane for CRTC 37 (`CANDIDATES 35
  primary 35`). k=1 with the primary is accepted and no k=2 set exists to
  try. `COMBO_MAX accepted_k=1`.
- The only other plane, 32, is the primary of CRTC 34. A request that puts
  it on CRTC 37 is refused by `plane_commit` before any ioctl (`plane 32 is
  not a plane of the CRTC`), and `scale_probe --plane 32` finds no plane to
  test. Both follow from `possible_crtcs` 0b1 (x2-probe).
- Whether the LCDIFv3 hardware could scan out a second layer that the
  driver does not expose: not measured.

### Printed RESULT / VERDICT lines

Observed 2026-11-28, copied from the files linked below:

```text
COMBO k=1 set=0 planes=35 scale=1.0000x1.0000 src=1920.0000x1080.0000 dst=1920x1080 result=ok
COMBO_MAX accepted_k=1
VERDICT kms_sweep: DONE combos
VERDICT kms_sweep: DONE modifiers
VERDICT kms_sweep: FAIL no connected connector with modes                    (--connector 39, LVDS-1)
RESULT plane=35 kind=Primary format=XR24 max_upscale_h=none max_upscale_v=none max_upscale_both=none downscale=rejected min_downscale=none
VERDICT scale_probe: FAIL some plane rejects factor 1 or the baseline: 1 plane(s) probed at dst 960x540, resolution 256/65536
RESULT plane=35 kind=Primary format=XR24 max_upscale_h=1.0000 max_upscale_v=1.0000 max_upscale_both=1.0000 downscale=rejected min_downscale=none
VERDICT scale_probe: OK 1 plane(s) probed at dst 1920x1080, resolution 256/65536
VERDICT scale_probe: FAIL no plane to test                                    (--plane 32)
VERDICT plane_commit: DONE                                                    (real commit, quadrants)
VERDICT: PASS                                                                 (camera-check on the KVM picture)
```

### Per CRTC

| CRTC | `kms_sweep` | `scale_probe` | Tag |
| --- | --- | --- | --- |
| 37 (LCDIF3, HDMI-A-1 connected, 1920x1080@60) | combos: k=1 ok, no k=2. modifiers: every format `addfb=EINVAL` (see Formats) | primary 35: 1:1 only at full screen, see Scaling | observed 2026-11-28 |
| 34 (LCDIF2, LVDS-1 disconnected, forced off by `video=LVDS-1:d`) | `--connector 39`: `VERDICT kms_sweep: FAIL no connected connector with modes`, exit 2. Nothing was tested | not runnable: the examples need a connected connector with a mode, and plane 32 cannot drive CRTC 37 | observed 2026-11-28. Enabling LVDS would be a boot-chain change and was not done |

### Position and size of the primary

`plane_commit --test-only-only` on plane 35, no modeset (the running mode
was kept). Observed 2026-11-28:

| Layer (`src` → `dst`) | Other property | `TEST_ONLY` |
| --- | --- | --- |
| XR24 1920x1080 → 0,0 1920x1080 | none | ok |
| AR24 1920x1080 → 0,0 1920x1080 | none | ok |
| XR24 1820x980 → 100,100 1820x980 (inside the screen, offset) | none | EINVAL |
| XR24 1920x1080 → 100,100 1920x1080 (offset, runs off the screen) | none | EINVAL |
| XR24 960x540 → 0,0 960x540 (smaller, at the origin) | none | EINVAL |
| XR24 1920x1080 → 0,0 1920x1080 | `zpos=0` (the value it already has) | EINVAL |
| XR24 1920x1080 → 0,0 1920x1080 | `zpos=1` | EINVAL |

- The primary is accepted only when it covers the whole mode from (0,0).
  A primary smaller than the mode at the origin is refused too, so the
  source's "origin only" rule is not the whole limit on this kernel. The
  proposal's `full_output_only = true` matches what the board accepts and is
  not too strict (observed 2026-11-28 for the results; the comparison with
  the proposal is inferred).
- Writing the immutable `zpos` is refused even with its current value 0
  (observed 2026-11-28). That is the DRM core's rule for immutable
  properties, not an LCDIF limit (inferred).

### Scaling bounds

`scale_probe` on plane 35, XR24, resolution 256/65536, every test
`ATOMIC_TEST_ONLY | ALLOW_MODESET`. Observed 2026-11-28:

| Destination | Largest accepted upscale (h / v / both) | 16.16 | First rejected | Downscale 0.5 / 0.75 / 0.9 |
| --- | --- | --- | --- | --- |
| 960x540 at (0,0) (default) | none: factor 1 itself rejected on h, v and both, errno 22 (EINVAL) | none | 1.0 | all rejected, errno 34 (ERANGE) |
| 1920x1080 at (0,0) (`--dst 1920x1080`) | 1.0 on h, v and both | 0x00010000 (65536) | 1.25 on the ladder, then the search narrowed to 1.0039 (65792), rejected on h, v and both with errno 34 | all rejected, errno 34, on h, v and both |

- The primary does no scaling at all. Any `SRC` ≠ `CRTC` size is refused
  with `ERANGE` (observed 2026-11-28). `ERANGE` is the errno the DRM core's
  plane-state helper returns for a scale outside the driver's limits, which
  agrees with `DRM_PLANE_NO_SCALING` in the vendor source read for the
  proposal below (inferred).
- The 960x540 run gives `EINVAL` before any scaling is tried, which is the
  full-screen rule above (observed 2026-11-28).

### Formats and modifiers

- `kms_sweep modifiers` reported `addfb=EINVAL test_only=not reached` for
  all seven formats (XR24, AR24, RG16, XB24, AB24, AR15, XR15) with the
  linear modifier, `listed=no` (observed 2026-11-28). The example always
  sets `DRM_MODE_FB_MODIFIERS` when it passes a modifier, also for linear,
  and the driver reports `DRM_CAP_ADDFB2_MODIFIERS` = 0. So these rows show
  that ADDFB2 with the modifier flag is refused, not that the formats are
  (inferred from the example's source and the cap). This is a limitation of
  `kms_sweep` on a driver without modifier support. A mode that adds a
  framebuffer without the flag would test the formats. It is not added
  here, since `crates/composition-drm` is out of this item's scope.
- Without the modifier flag (`plane_commit`), XR24 and AR24 full-screen
  were accepted by `TEST_ONLY` (observed 2026-11-28). RG16, XB24, AB24,
  AR15 and XR15: not measured, because `plane_commit` only offers XR24, AR24
  and NV12. NV12: not measured, since it is not in the plane's format list.

### Real commit and picture

One real commit was made, because the output could be driven safely: no
display manager or DRM client was running, the CRTC was already running
1920x1080 (so `plane_commit` made no modeset), and the KVM showed the HDMI
output. Observed 2026-11-28
([`sweep-plane-commit-crtc37.txt`](observed/imx8mp-x2-2026-11-28/sweep-plane-commit-crtc37.txt)):

- `plane_commit --hold-ms 30000 --layer
  plane=35,format=XR24,src=1920x1080,dst=0,0,1920x1080,pattern=quadrants`:
  `TEST_ONLY ok`, `COMMIT ok`, `STATE plane=35 fb=47 zpos=0`, `CRTC
  active=1`, `RECOVER ok`, `RESTORE ok`, `VERDICT plane_commit: DONE`.
- KVM picture during the hold
  ([`sweep-picture-quadrants.jpg`](observed/imx8mp-x2-2026-11-28/sweep-picture-quadrants.jpg),
  42 KB, metadata stripped): judged by `scripts/camera-check.py` against
  [`sweep-picture-expect.json`](observed/imx8mp-x2-2026-11-28/sweep-picture-expect.json)
  (`panel full`, rotation 0, tolerance 60): all four quadrants PASS
  (distances 30.8-37.2), `VERDICT: PASS`
  ([`sweep-picture-verdict.txt`](observed/imx8mp-x2-2026-11-28/sweep-picture-verdict.txt)).
  This shows one primary plane on screen. It is not a composition test,
  since there is only one plane.
- `scripts/picture-hook.sh` was not run: the KVM capture here is an MCP
  tool and not the shell command the hook's `PICTURE_CAPTURE` needs, and
  running the hook with a copy command was refused in this session.
  camera-check (the hook's judge) was run directly instead (observed
  2026-11-28).
- The KVM picture lagged the board by several seconds. One capture 4 s
  into the hold still showed the console, and one about 12 s after
  `RESTORE ok` still showed the quadrants (observed 2026-11-28, times
  approximate). About 23 s after the hold, the KVM showed the fbdev console
  again (observed 2026-11-28).
- Pitch change with or without a modeset (proposal item 5), and whether the
  AR24 alpha channel changes the picture: not measured.

### Display-manager state

- Before (observed 2026-11-28): no process had `/dev/dri/*` open. The OpenRC
  default runlevel was as x2-probe recorded (10 services started, none of
  them a display manager or compositor). No podman container was running.
  The HDMI picture was the fbdev console. The board has no `systemctl`, so
  `scripts/display-guard.sh detect` found nothing, and `stop` printed
  `DISPLAY-GUARD none` and wrote an empty state file.
- After (observed 2026-11-28): `display-guard.sh restore` printed
  `DISPLAY-GUARD none` and exited 0. No process had `/dev/dri/*` open. The
  same 10 default-runlevel services were started. HDMI-A-1 was `connected`
  and LVDS-1 `disconnected`, and the KVM showed the fbdev console. The
  binaries and outputs were removed from `/tmp` on the board. Nothing was
  stopped, so there was nothing to restore.

### Files

All in [`observed/imx8mp-x2-2026-11-28/`](observed/imx8mp-x2-2026-11-28/README.md),
observed 2026-11-28:

| File | What |
| --- | --- |
| [`sweep-crtc37.txt`](observed/imx8mp-x2-2026-11-28/sweep-crtc37.txt) | `kms_sweep combos` and `kms_sweep modifiers` on CRTC 37 |
| [`sweep-crtc34.txt`](observed/imx8mp-x2-2026-11-28/sweep-crtc34.txt) | `kms_sweep combos --connector 39`: refused, no connected connector |
| [`sweep-scale-crtc37.txt`](observed/imx8mp-x2-2026-11-28/sweep-scale-crtc37.txt) | `scale_probe`: default destination, `--dst 1920x1080`, `--plane 32` |
| [`sweep-summary-crtc37.md`](observed/imx8mp-x2-2026-11-28/sweep-summary-crtc37.md) | `scripts/sweep-summary.py` output for the two files above, with reading notes |
| [`sweep-plane-commit-testonly-crtc37.txt`](observed/imx8mp-x2-2026-11-28/sweep-plane-commit-testonly-crtc37.txt) | the `plane_commit --test-only-only` position, size, format and zpos cases |
| [`sweep-plane-commit-crtc37.txt`](observed/imx8mp-x2-2026-11-28/sweep-plane-commit-crtc37.txt) | the one real commit |
| [`sweep-picture-quadrants.jpg`](observed/imx8mp-x2-2026-11-28/sweep-picture-quadrants.jpg), [`sweep-picture-expect.json`](observed/imx8mp-x2-2026-11-28/sweep-picture-expect.json), [`sweep-picture-verdict.txt`](observed/imx8mp-x2-2026-11-28/sweep-picture-verdict.txt) | KVM picture, camera-check regions, verdict |

## Proposed DriverProfile

Design proposal only. Nothing here is implemented, and `crates/composition-drm`
is not edited (plans 457 and drm-plane-truncate are changing it). It
extends the sections above and does not replace them. Tags:
**observed 2026-11-28** = a fact in this note's own dumps
([`observed/imx8mp-x2-2026-11-28/`](observed/imx8mp-x2-2026-11-28/README.md)).
**read from source** = read in a kernel tree, as recorded by
[`imx8mp-lcdif-source.md`](imx8mp-lcdif-source.md). **inferred** = concluded,
neither seen nor stated. The `TEST_ONLY` session of item x2-testonly had not
merged when this was written, so every `TEST_ONLY` expectation below is open.

### Is an existing variant enough?

No variant matches this board by name or by intent. `DriverProfile` has
`Generic`, `Imx95Dpu`, `Stm32Ltdc`, `Rk3588Vop2`, `Vc4Hvs5` and `Vc4Hvs6`
(`crates/composition-drm/src/profile.rs`, read at this branch's base commit).
`Imx95Dpu` is a different IP with six planes and shared scalers, so it must
not be reused (inferred). Variants are chosen explicitly by the caller, not by
driver name (the `Stm32Ltdc` doc says so).

`Generic` alone is nearly right, which is the reason for a small variant and
not a large one. `Generic` only reflects what KMS reports. On this board that
is one primary plane per CRTC, zpos 0, no scaling claim, seven formats, plane
alpha false (observed 2026-11-28). A plane with `scaling: None` is planned
1:1 (the `PlaneCaps::scaling` doc in `composition-core`), which matches the
source below. What `Generic` cannot say is the position limit below.

### Proposed variant

| Item | Proposal | Tag |
| --- | --- | --- |
| Name | `DriverProfile::Imx8mpLcdifv3`. It names LCDIFv3 and not "imx-lcdif", because the board's driver is the vendor `imx-lcdifv3`, not mainline's | observed 2026-11-28 for the driver; the name is inferred |
| Driver string it matches | none by itself. The DRM name is `imx-drm` (card1), and the older i.MX6 IPU driver also reports `imx-drm`, so the name is ambiguous. The caller selects the variant. A guard can also check the DT compatible `fsl,imx8mp` (`soc.txt`) | observed 2026-11-28 for the name and the compatible; the i.MX6 IPU overlap is inferred |
| Sysfs hint for a tool | the CRTC's component device is `imx-lcdifv3-crtc.N`, with a parent bound to `imx-lcdifv3` | observed 2026-11-28 |
| Planes per CRTC | one primary, no overlay, no cursor. The variant adds nothing: it takes what KMS reports | observed 2026-11-28 (KMS); read from source (vendor `lcdifv3-plane.c`: only the primary type is handled, overlay is a "TODO") |
| Formats | the seven KMS formats XR24, AR24, RG16, XB24, AB24, AR15, XR15, linear only. No YUV, no modifier | observed 2026-11-28 (list, no `IN_FORMATS`); read from source (the same seven, `lcdifv3-plane.c:22-30` at `lf-6.6.52-2.2.2`, linear only) |
| Scaling | none: `scaling: None` on every plane, set explicitly and not left to the default | observed 2026-11-28 (no scaling property); read from source (`DRM_PLANE_NO_SCALING` both ways, `lcdifv3-plane.c:~62-65`) |
| zpos | fixed 0, immutable. Nothing to set | observed 2026-11-28 |
| Plane alpha | `plane_alpha: false` (KMS already says so) | observed 2026-11-28 |
| Blend modes | left as KMS reports (premultiplied is only composition-drm's default for an absent property). With one plane at zpos 0 nothing lies below it, so whether the `AR24` alpha is used by the scanout is not visible to the planner | observed 2026-11-28 for the absent property; the effect of the alpha channel is not measured |
| Position limit | the primary must sit at CRTC (0,0): a non-zero `crtc_x` or `crtc_y` is rejected, and so is an invisible plane. Proposal: set `full_output_only = true` on the primary. That is stricter than the source (origin, not full size), so it errs on the safe side | read from source (`lcdifv3-plane.c:~55-57`, `~67-68`); choosing the stricter flag is inferred |
| Pitch change | a change of framebuffer pitch forces a full modeset, because the pitch registers cannot change while the controller runs. The planner cannot express this | read from source (`lcdifv3-plane.c:~70-77`); how it shows under `TEST_ONLY` is not measured |
| `ResourceGroup` | none. No shared scaler, fetch unit or blender is exposed or was read. No "one plane per CRTC" group either, since the plane list already has one plane per CRTC | inferred |
| Cross-CRTC limits | not modelled. Whether LCDIF2 and LCDIF3 can run together and share pixel clocks was not worked out | not measured |

Effect for a caller: a frame with one full-output layer gets the primary
plane. A frame with two or more layers has no plane for the extra ones, and
the planner must fall back to composition into the primary's buffer. That is
the case that `docs/imx8mp-route3-precomposition.md` addresses (inferred).

**Version caveat.** The board runs Atmark's `6.12.105-0-at` (observed
2026-11-28). The only vendor source read is NXP `linux-imx` `lf-6.6.52-2.2.2`
(commit `5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc`), and only its plane
file. These are two different trees (6.6 against 6.12), so the "read from
source" rows describe a nearby vendor tree, not the board's kernel. The seven
formats and the single primary plane agree with the board (observed
2026-11-28), which is some reason to trust the rest of the plane file and no
more (inferred). The position limit and the pitch rule were not observed. The
Atmark kernel's own source was not read.

### What `TEST_ONLY` on the board should confirm

All open: item x2-testonly had not merged. When it does, its `kms_sweep` and
`scale_probe` results settle rows 1 and 2; read the rest against them.

1. A second layer on a second plane is impossible, because no second plane
   exists. A primary-only commit is accepted (`kms_sweep`).
2. `SRC` ≠ `CRTC` size on the primary is refused (expected from
   `DRM_PLANE_NO_SCALING`; `scale_probe`).
3. A primary at a non-zero `CRTC_X/Y`, and a primary smaller than the mode
   at (0,0), are each accepted or refused. This decides whether
   `full_output_only` is right or too strict. It needs a small case beyond
   the two named examples.
4. `AR24` and `AB24` buffers are accepted on the primary. Whether the alpha
   channel changes the picture needs the KVM picture and is not a
   `TEST_ONLY` fact.
5. A framebuffer with a different pitch is accepted with or refused without
   the modeset flag.

### Implementation

Not started. It waits until `crates/composition-drm` is free. The planned
change is a `DriverProfile::Imx8mpLcdifv3` arm that sets `scaling: None` and
`full_output_only` on every non-cursor plane, a unit test on a fake
one-primary capability set, and a doc comment tagged like the variants above.

## Proposed open-issues row

For the owner of `docs/open-issues.md` to paste in (plan 461 is changing
that file, so it is not edited here). `X-10` is a placeholder: renumber it.
The columns are that file's: ID, Issue, Evidence, What would close it.

| ID | Issue | Evidence | What would close it |
| --- | --- | --- | --- |
| X-10 | The i.MX8M Plus (Armadillo-X2, SoW §5.3) has no `DriverProfile`. Its display driver `imx-drm` (vendor `imx-lcdifv3`) exposes one primary plane per CRTC, no overlay, no scaling, zpos fixed 0 (observed 2026-11-28), and `imx-drm` is also the i.MX6 IPU driver's name, so selection by name is ambiguous. `Generic` cannot express the primary's position limit, which is read from a nearby vendor tree (`lf-6.6.52-2.2.2`), not the board's 6.12 kernel. Not verified by any commit on the board | `docs/imx8mp-x2-board.md` ("Proposed DriverProfile"); `docs/observed/imx8mp-x2-2026-11-28/`; `docs/imx8mp-lcdif-source.md` | After `crates/composition-drm` is free (plans 457 and drm-plane-truncate): add `DriverProfile::Imx8mpLcdifv3` with a host test. Run `TEST_ONLY` on the X2 (item x2-testonly) for the position, pitch and scaling cases, and correct the profile where the board disagrees. Not closed: implementation has not started |

Implementation waits until `crates/composition-drm` is free. The row is a
proposal, not a filed issue.
