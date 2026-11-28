# RK3588 VOP2 zpos session, NanoPC-T6 (2026-11-28)

Answers part of Q6 of [`rk3588-backend-design.md`](rk3588-backend-design.md) section 7 (is `zpos` global or per CRTC; what happens on equal `zpos`), using the `zpos_probe` example (`crates/composition-drm/examples/zpos_probe.rs`) and [`rk-hvs-session-checklist.md`](rk-hvs-session-checklist.md). The earlier topology session is [`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md).

**Portability evidence.** Never counts toward M1, M2 or M3. Every request was `TEST_ONLY | ALLOW_MODESET`; nothing was committed.

## Setup

| Item | Value | Tag |
|---|---|---|
| Kernel | `6.18.55-current-rockchip64`, driver `rockchip` 1.0.0 | observed 2026-11-28 12:30Z |
| Device | `/dev/dri/card0` | observed 2026-11-28 12:30Z |
| Outputs | HDMI-A-1 connected (CRTC index 0, 1920x1080@60); HDMI-A-2 disconnected (sysfs status) | observed 2026-11-28 12:30Z |
| Display manager | `display-manager`, `gdm`, `lightdm`, `sddm`: `inactive` before; `display-manager` `inactive` afterwards. Nothing was stopped, so `display-guard.sh` was not needed | observed 2026-11-28 12:30Z |
| Binary | `zpos_probe` cross-built with `scripts/cross-aarch64.sh` (static aarch64 musl), copied to the board's `/tmp`, removed afterwards with its output file | observed 2026-11-28 12:30Z |
| Judging | `scripts/zpos-probe-expect.py` is not on the tip; judged by hand | observed 2026-11-28 |

## Raw log (full output, exit status 0)

```text
DATE 2026-11-28T12:30:35Z
KERNEL 6.18.55-current-rockchip64
DEVICE /dev/dri/card0
DRIVER rockchip version 1.0.0
CRTC index 0 id 81 connector 84 mode 1920x1080@60Hz
PLANE id 33 Primary crtcs 0 zpos 0..7
PLANE id 39 Primary crtcs none zpos 0..7
PLANE id 45 Overlay crtcs 0 zpos 0..7
PLANE id 51 Overlay crtcs 0 zpos 0..7
PLANE id 57 Overlay crtcs 0 zpos 0..7
PLANE id 63 Overlay crtcs 0 zpos 0..7
PLANE id 69 Overlay crtcs 0 zpos 0..7
PLANE id 75 Overlay crtcs 0 zpos 0..7
ZPOS case=distinct planes=33,45 zpos=0,1 result=ok
ZPOS case=equal planes=33,45 zpos=0,0 result=ok
VERDICT zpos_probe: DONE
```

## Per case

| Case | Planes | zpos | Result | errno | Tag |
|---|---|---|---|---|---|
| distinct | 33, 45 | 0, 1 | ok | none | observed 2026-11-28 12:30Z |
| equal | 33, 45 | 0, 0 | ok (TEST_ONLY accepted) | none | observed 2026-11-28 12:30Z |
| cross_equal | n/a | n/a | not testable: no line printed | n/a | observed 2026-11-28 12:30Z |

`cross_equal` needs a second connected connector with its own CRTC. Only HDMI-A-1 is connected and one CRTC was used (the tool printed a single `CRTC` line); HDMI-A-2 is disconnected. Cabling and configuration were not changed. Each plane reports `zpos 0..7`, the same range of eight values on all eight planes (observed 2026-11-28 12:30Z).

## Q6 status: partly answered

- Equal `zpos` on two planes of one CRTC: the kernel's TEST_ONLY check accepted it (observed 2026-11-28 12:30Z). This says the request is not rejected; it does not say what stacking order the hardware produces. No real commit was made, so the visible result of equal values is unmeasured (inferred).
- Global versus per-CRTC `zpos`: still open. The `zpos` range is identical (0..7) on every plane, which fits either reading; only a second active CRTC can decide it (inferred).
- Next step: rerun `zpos_probe` where a second display is attached for `cross_equal`; a picture-judged real commit of equal `zpos` belongs to the commit session.
