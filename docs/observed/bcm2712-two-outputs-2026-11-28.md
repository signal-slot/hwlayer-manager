# BCM2712 (Raspberry Pi 5): two active HDMI outputs (HVS Q7, Pi 5 half), not run: one sink

Date: 2026-11-28. Rows tagged **observed 2026-11-28** were read on the board over ssh and through the
NanoKVM HDMI capture on 2026-11-28 (host UTC 10:27 to 10:29). The aim was the Pi 5 half of
`docs/design/hvs-backend.md` §11 Q7: drive HDMI-A-1 and HDMI-A-2 at the same time with the engine, and
record overlay sharing between the CRTCs, channel pinning (HDMI1 against moplet) and the dlist budget.
**The run was not possible with this setup: only HDMI-A-1 has a sink.** Nothing was started on the board
and nothing was changed. This is portability evidence only and counts toward no milestone.

Evidence: `bcm2712-two-outputs-2026-11-28/hdmi-a-1-kvm-capture.png` (the NanoKVM capture of HDMI-A-1,
reduced from 1920x1080 to 480x270; the console text in the top left, which shows the board's network
addresses, is blacked out).

## Sinks and capture

| Item | Value | Tag |
|---|---|---|
| Kernel, card | `6.18.50+rpt-rpi-2712`; `card1` = vc4 (display), debugfs `/sys/kernel/debug/dri/1/` | observed 2026-11-28 |
| HDMI-A-1 | sysfs `status` connected, `enabled` enabled, 20 modes, EDID 256 bytes, first mode 1920x1080 (the NanoKVM's EDID, as in `bcm2712.md`) | observed 2026-11-28 |
| **HDMI-A-2** | **disconnected, disabled, 0 modes, no EDID** | observed 2026-11-28 |
| Writeback-1, Writeback-2 | connected (virtual, TXP "mop" and "moplet"), disabled, 34 modes, first 4096x2160 | observed 2026-11-28 |
| Kernel command line | only `video=HDMI-A-1:1920x1080M@60`; no force-enable of HDMI-A-2 | observed 2026-11-28 |
| What the KVM captures | HDMI-A-1: the screenshot at 10:27Z shows the tty1 login prompt (fbcon) at 1920x1080, which matches the only active CRTC below | observed 2026-11-28 |
| Same state in earlier records | `bcm2712-topology.md` and `bcm2712.md` also saw HDMI-A-2 disconnected | observed 2026-11-28 (those docs) |

## One-output baseline (read-only)

| Item | Value | Tag |
|---|---|---|
| CRTCs | 59 `mop`, 78 `moplet` (writeback), 94 `crtc-2`, 106 `crtc-3`. Only CRTC 94 is active: HDMI-A-1, 1920x1080@60 (148.5 MHz), `plane_mask=4`; CRTCs 59, 78, 106 disabled | observed 2026-11-28 |
| Planes | 26 planes (`plane-0` … `plane-25`, ids 48 … 338); only plane 83 (`plane-2`) is in use, on `crtc-2`, fb 680, 1920x1080 (fbcon) | observed 2026-11-28 |
| `hvs_dlists` | `HVS chan 0`: one display list (entries 74 … 83, one plane, ending in the end marker). `HVS chan 1 disabled`, `HVS chan 2 disabled` | observed 2026-11-28 |
| `hvs_dlist_allocs` | three nodes allocated, `[0x20 + 0xb]`, `[0x2b + 0xb]`, `[0x4a + 0xa]`; no stale nodes | observed 2026-11-28 |
| `hvs_underrun` | 0 | observed 2026-11-28 |
| DRM master | nobody (`fuser -v /dev/dri/*` empty); `lightdm` inactive; default target `multi-user.target` | observed 2026-11-28 |
| Kernel log | one earlier line `vc4-drm axi:gpu: [drm] *ERROR* Failed to allocate UPM entry: -28` at uptime 25558 s, before this session; which run caused it is not known from this session | observed 2026-11-28 (line); cause not established |

## What could not be recorded

| Q7 point | Status | Tag |
|---|---|---|
| Overlay sharing between two active CRTCs | not measured: no second active HDMI CRTC | observed 2026-11-28 (precondition missing) |
| Channel pinning (HDMI1 against moplet) | not measured: HDMI1 (HDMI-A-2) cannot be enabled without a sink; only channel 0 is in use | observed 2026-11-28 (precondition missing) |
| Planes each output gets | not measured with two outputs; with one output the single active CRTC 94 is the only output | observed 2026-11-28 |
| dlist budget with both active | not measured; the one-output baseline is the table above | observed 2026-11-28 |
| KVM judge (`twoplane-expect.py --rotation 0`, `camera-check.py`) | not run: no engine run took place, so there is no engine picture to judge | observed 2026-11-28 |

To answer Q7 on the Pi 5, a second sink is needed on HDMI-A-2 (a second capture device or any monitor).
Forcing the connector on without a sink (`video=HDMI-A-2:...e` or the sysfs `status` file) would change
DRM configuration on a farm node, which this session must not do, so it was not tried (inferred: such a
forced output would drive the pipeline and could give the dlist and channel data, but no picture).

## Verdict lines

```text
second sink present: no (HDMI-A-1 connected to the NanoKVM, EDID 256 bytes; HDMI-A-2 disconnected, no EDID)
KVM captures: HDMI-A-1 (tty1 fbcon at 1920x1080)
Q7 on the Pi 5: not answerable with this setup (one sink)
route used: none (no engine process, no drm-lease-manager started)
board restored: yes (nothing was changed; no files written to the board)
```

## State after

| Item | State | Tag |
|---|---|---|
| Processes | none started; `fuser -v /dev/dri/*` empty | observed 2026-11-28 |
| fbcon | still on HDMI-A-1 (CRTC 94, plane 83) | observed 2026-11-28 |
| Board files | none written under `/tmp` or elsewhere | observed 2026-11-28 |
