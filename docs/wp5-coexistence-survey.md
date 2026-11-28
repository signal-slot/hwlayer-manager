# WP5 coexistence survey of three Linux DRM boards

Date: 2026-11-28.

This is a **survey, not the coexistence trial**. It only reads the state of three idle boards over ssh. Nothing was leased, no mode was set, no service or compositor was started or stopped, nothing was flashed or rebooted, and no serial or KVM was used; DRM, udev and seat configuration were not touched. Every board was left as found. The survey **never counts toward M1, M2 or M3**. The trial itself (a leased output while Weston owns another connector, W5-10 in `docs/wp5-gap-analysis.md` §6 and WP5-07 in `docs/open-issues.md`) is left to a later plan.

Commands used on each board (read-only): `uname -r`; `ls /dev/dri`; per connector `status`, `enabled`, `modes` and the byte size of `edid` under `/sys/class/drm`; the bound driver of each card from sysfs; `systemctl list-units --type=service --state=running` filtered for display managers and compositors; `systemctl get-default`; `loginctl list-sessions`; `fuser -v /dev/dri/*`; a count of `/proc/*/fd` links to dri. `modetest` is not installed on any of the three boards (observed 2026-11-28), so the connector data comes from sysfs only; the repository's probe example was not looked for or run.

## Observed rows

All rows: observed 2026-11-28.

| Item | NanoPC-T6 (RK3588) | Raspberry Pi 5 | reTerminal (CM4) |
| --- | --- | --- | --- |
| Kernel (observed 2026-11-28) | 6.18.55-current-rockchip64 | 6.18.50+rpt-rpi-2712 | 6.12.47+rpt-rpi-v8 |
| `/dev/dri` (observed 2026-11-28) | card0, card1, renderD128 | card0, card1, renderD128 | card0, card1, renderD128 |
| Display card and driver (observed 2026-11-28) | card0 = rockchip-drm | card1 = vc4-drm | card0 = vc4-drm |
| Render-only card (observed 2026-11-28) | card1 = panthor | card0 = v3d | card1 = v3d |
| Connectors (observed 2026-11-28) | HDMI-A-1 connected, enabled, 19 modes, EDID 256 bytes, first mode 1920x1080; HDMI-A-2 disconnected, disabled, no modes | HDMI-A-1 connected, enabled, 20 modes, EDID 256 bytes, first mode 1920x1080; HDMI-A-2 disconnected, disabled; Writeback-1 and Writeback-2 report connected, disabled | DSI-1 connected, enabled, 1 mode 720x1280, no EDID; HDMI-A-1 and HDMI-A-2 disconnected, disabled; Writeback-1 reports connected, disabled |
| Display manager or compositor service running (observed 2026-11-28) | none; the filter for weston, gdm, sddm, lightdm, labwc, wayfire, cage, kms, x11, xorg matched nothing; default target graphical.target | none; default target multi-user.target | none; default target multi-user.target |
| Process holding `/dev/dri` (observed 2026-11-28) | none (`fuser` empty, no dri file descriptors in `/proc`) | none (same checks) | none (same checks) |
| Owner of the display (observed 2026-11-28) | no userspace compositor; a login getty on tty1 (logind shows a seat0 session on tty1) | no userspace compositor; a login getty on tty1 | no userspace compositor; nothing holds the display |
| Second output physically connected (observed 2026-11-28) | no: one HDMI sink; HDMI-A-2 empty | no: one HDMI sink; HDMI-A-2 empty | no: only the built-in DSI panel; both HDMI empty |

The Writeback connectors report `connected` because they are virtual (memory-to-memory capture), not a physical sink. They are not counted as second outputs (inferred from their name and mode lists).

## Verdict per board

None of the three boards runs Weston or any other compositor now (observed 2026-11-28), so the "while Weston owns another connector" half of the scenario would first need Weston (or another compositor) started on a board. This survey did not do that; a later plan must decide it.

- **NanoPC-T6 (RK3588)**: possible only after a second sink is attached to HDMI-A-2 (inferred). It has two HDMI connectors and one is connected, so the hardware allows two outputs; today there is one sink, so it is blocked by hardware as it stands. Needed: a second HDMI sink on HDMI-A-2, and a compositor started on card0 for the other connector.
- **Raspberry Pi 5**: possible only after a second sink is attached to HDMI-A-2 (inferred). Same shape as the NanoPC-T6: two HDMI connectors, one connected, no compositor running. Needed: a second HDMI sink, and a compositor on card1 (vc4-drm).
- **reTerminal (CM4)**: blocked by hardware as it stands (inferred). One connected sink (the DSI panel); both HDMI connectors exist in the driver but have nothing attached. A second output would need a sink on an HDMI connector; whether the carrier exposes a usable HDMI port was not checked (inferred only from the driver listing the connectors).

Whether a DRM lease of one connector can be taken from vc4-drm or rockchip-drm while another process owns the other connector was not tested here; that is the trial's question.
