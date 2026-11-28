# BCM2711 (Raspberry Pi CM4, reTerminal): recheck after the plane-truncation fix, and writeback

Everything tagged **observed 2026-11-28** was measured on the board over ssh on 2026-11-28 (the
harness printed `DATE 2026-11-28T01:41:19Z`). Binaries were built on the host from commit ea5b16f
(`probe`, `twoplane`, `writeback` of `crates/composition-drm/examples`, release,
`aarch64-unknown-linux-musl`, linked with `rust-lld`), copied to `/tmp` on the board and removed
afterwards. `scripts/cross-aarch64.sh` was not used. This file does not change
`docs/observed/bcm2711-cm4.md`; it records what differs from it.

> **Writeback is not the panel.** The `writeback` example reads back the HVS composition of the
> **TXP CRTC** through the Writeback-1 connector. It is not the DSI-1 panel's physical output, and
> nothing captures the panel (no KVM). Z-order and alpha **on the glass remain not verified**.

## Setup

| Item | Value | Tag |
|---|---|---|
| Kernel | `6.12.47+rpt-rpi-v8 #1 SMP PREEMPT Debian 1:6.12.47-1+rpt1 (2025-09-16)` | observed 2026-11-28 |
| Driver | `vc4` on `/dev/dri/card0` | observed 2026-11-28 |
| Panel mode | DSI-1, CRTC 90, 720x1280 @ 60.000 Hz | observed 2026-11-28 |
| Writeback mode | TXP CRTC 59, 4096x2160 @ 59.999 Hz, connector `Writeback-1`, formats RG24 BG24 XR24 XB24 AR24 AB24 RX24 BX24 RA24 BA24 | observed 2026-11-28 |
| DRM master | Nobody held card0 before or after (`fuser` empty); `slint-launcher.service` and `lightdm.service` inactive before and after; fbcon scans out plane 79 (`allocated by = [fbcon]`, RG16) before and after. Nothing was stopped, so nothing needed restoring; the state is the same as before the runs | observed 2026-11-28 |
| Underruns | `hvs_underrun` 0 after the runs | observed 2026-11-28 |

## 1. probe: planes the engine sees

| Item | Before (`bcm2711-cm4.md`, commit before drm-plane-truncate) | Now | Tag |
|---|---|---|---|
| Planes reported | 16: 6 primaries (5 of them unreachable, mask `0b0`) + 10 overlays | **16** on one output: primary 79, **14 overlays** (127 138 149 160 171 182 193 204 215 226 **237 248 259 270**), cursor 677 | observed 2026-11-28 |
| Shared overlays | 10 of 16 | 14 of 16 (the four formerly dropped 237 248 259 270 now seen; two more, 281 and 292, are not) | observed 2026-11-28 |
| Unreachable primaries | 5 reported | none reported | observed 2026-11-28 |
| Cursor | dropped | reported (677, zpos 1..=17) | observed 2026-11-28 |
| Per plane | outputs `0b1`, overlay zpos 1..=17, primary fixed 0, plane alpha, blend none/premultiplied/coverage, the same 32 formats | unchanged | observed 2026-11-28 |

The harness's `PLANE` lines agree: 16 entries. The count is still capped at the engine's 16 planes, but the cap now falls on this CRTC's own planes instead of on unreachable primaries.

## 2. twoplane

Default run (`twoplane /dev/dri/card0`), plan from frame 0: layer 0 on plane 79 (Primary), layer 1
on plane 127 (Overlay).

```text
RESULT frames=600 committed=600 latched=600 released=1198 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=16.730 latch_interval_max_ms=16.866 elapsed_s=10.026
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver vc4 mode 720x1280@60.000Hz
```

observed 2026-11-28. The harness has no option to choose the planes (the planner picks the lowest
index that fits), so a run **on a formerly dropped shared overlay (237..270) was not possible**
without a code change; four of them (237..270) are only shown to be visible to the engine (section 1).

`--frames 0 --unsupported`: capability detection, planner and `TEST_ONLY` answered as before
(format, modifier, layer count, scale up/down, rotation refused by caps and planner; scaling
accepted by `TEST_ONLY` on a hand-built plan; plane alpha, coverage blend, off-screen and odd
sizes accepted), with 16 planes counted for the layer-count case (`17 layers for 16 planes`).

```text
VERDICT twoplane: PASS no frames requested, nothing presented, driver vc4 mode 720x1280@60.000Hz
```

observed 2026-11-28.

## 3. writeback

```text
KERNEL 6.12.47+rpt-rpi-v8
DEVICE /dev/dri/card0
DRIVER vc4
WRITEBACK connector 65 Writeback-1 crtcs [59] formats [RG24,BG24,XR24,XB24,AR24,AB24,RX24,BX24,RA24,BA24]
MODE 4096x2160@59.999Hz output 1 crtc 59 writeback Writeback-1
PLANE id 48 Primary zpos Fixed(0) plane-alpha true blend-mask 0x7
PLANE id 655 Cursor zpos Range { min: 1, max: 17 } plane-alpha true blend-mask 0x7
VERDICT writeback: FAIL no overlay plane with a linear 32-bit RGB format, driver vc4 mode 4096x2160@59.999Hz via Writeback-1
```

observed 2026-11-28. **No CHECK line was printed: the run stopped before composing anything, so the
z-order and alpha were not checked by writeback on this board.**

Cause, read from the output: for the TXP CRTC (output 1) the engine sees only primary 48 and cursor
655. The 32 TXP overlays are not among the planes the backend keeps. This matches the caveat of the
plane-truncation fix: the cap of 16 planes is applied after ranking planes by "reaches a probed
output", not per CRTC, so the planes of the DSI CRTC and the primaries take the slots before the
TXP's own overlays. Which of the ranked slots the TXP overlays lose is inferred from the plane list,
not from a source read of the filter. This is a finding for the planner of work, not a change made
here.

## Summary

- probe: 16 planes now (1 primary, 14 of the 16 shared overlays, 1 cursor) against 16 before, of which 5 were unreachable primaries. Observed.
- twoplane: PASS on the panel CRTC. Observed.
- writeback: **FAIL** (no overlay on the TXP CRTC after the plane cap). Observed. Not a panel result.
- z-order and alpha on the DSI panel: still not verified.
