# BCM2712 (Raspberry Pi 5): recheck after the plane-truncation fix, writeback against KVM

Recheck of `docs/observed/bcm2712.md` after `drm-plane-truncate` (planes are filtered by CRTC
before `MAX_PLANES` is applied). Tags: **observed** = measured on the board on the stated date,
**inferred** = derived from observed data. Nothing here is read from source. Not a claim toward
M1 to M3.

## Setup — observed 2026-11-28

| Item | Value |
|---|---|
| Board | Raspberry Pi 5 Model B Rev 1.1, Raspberry Pi OS Lite arm64 (Debian 13) |
| Kernel | `6.18.50+rpt-rpi-2712 #1 SMP PREEMPT Debian 1:6.18.50-1+rpt1 (2026-09-11) aarch64` |
| Driver | `/dev/dri/card1`, `vc4` ("Broadcom VC4 graphics"); `card0` is `v3d` |
| Mode | HDMI-A-1 on CRTC 94, 1920x1080 @ 60.000 Hz (pinned on the kernel command line) |
| Writeback connectors | Writeback-1 (CRTC 59, "mop"), Writeback-2 (CRTC 78, "moplet"); formats RG24 BG24 XR24 XB24 AR24 AB24 RX24 BX24 RA24 BA24; TXP mode 4096x2160 @ 59.999 Hz |
| DRM master holder | none (no desktop, no compositor; only `getty@tty1` and `serial-getty@ttyAMA10`); nothing was stopped, so nothing had to be restored except the fbcon scanout (below) |
| Build | `probe`, `twoplane`, `writeback` at commit b58bfab, `cargo build --release --target aarch64-unknown-linux-musl` with `rust-lld`, target directory outside the repository; copied to `/tmp` on the board, run with `sudo`, removed afterwards; nothing installed |
| Kernel log during the runs | no DRM, vc4 or HVS message (the 25 lines matching drm/vc4/hvs are all boot-time, 0.6 s to 41 s) |
| `hvs_underrun` | 0 before, 0 after |

## (1) Probe: planes the engine sees — observed 2026-11-28

`sudo /tmp/probe /dev/dri/card1` printed `output 0: crtc 94 1920x1080 @ 60.000 Hz` and 16 planes
(the kernel has 56). Plane ids, in order:

```
83 Primary (outputs 0b1, zpos fixed 0)
107 118 129 140 151 162 173 184 195 206 217 228 239 250   Overlay (outputs 0b1, zpos 1..=17)
657 Cursor (outputs 0b1, zpos 1..=17)
```

All 16 have mask `0b1`, plane alpha, blend [none, premultiplied, coverage], scaling not claimed,
32 formats.

| | Earlier record (`bcm2712.md`) | Now |
|---|---|---|
| Planes in the engine's view | 16 | 16 |
| Primaries of unused CRTCs (48, 67, 95) | 3, mask `0b0`, useless | gone |
| Planes usable on HDMI-A-1 | 13 (1 primary + 12 overlays 107 to 228) | 16 (1 primary + 14 overlays 107 to 250 + cursor 657) |
| Overlays 239, 250 | cut | visible |
| Overlays 261, 272 | cut | still cut |

The fix works as intended: every plane in the list now reaches the output. Two of the four
formerly cut shared overlays (239, 250) are back. The other two (261, 272) are still cut because
the cursor plane 657 takes a slot and the list still ends at 16. — observed 2026-11-28; the
cause of the 261/272 cut is inferred from the count.

## (2) Two-plane run — observed 2026-11-28

Plan, identical to the earlier record: layer 0 (background) on plane 83 Primary zpos 0, layer 1
(overlay) on plane 107 Overlay zpos 1. The planner takes the first plane that fits, so the
default run does not touch a formerly cut overlay, and `twoplane` has no option to choose a
plane; running on 239/250 (or 261/272) needs a code change, which this item does not make. Not
run. — observed (the plan), inferred (the reason)

Runs (RESULT lines, verbatim):

```
RESULT frames=600 committed=600 latched=600 released=1198 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=16.666 latch_interval_max_ms=16.680 elapsed_s=9.997
RESULT frames=1800 committed=1800 latched=1800 released=3598 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=16.667 latch_interval_max_ms=16.704 elapsed_s=29.987
```

Both ended with `RELEASE after the run: released N of N buffer uses, 0 still on screen`
(1200 of 1200, 3600 of 3600).

VERDICT lines (verbatim):

```
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver vc4 mode 1920x1080@60.000Hz
VERDICT twoplane: PASS 1800 frames on two planes, 1800 latched, 3598 released, 0 late, driver vc4 mode 1920x1080@60.000Hz
```

`--unsupported` (on the 600-frame run), what changed from the earlier record:

| Combination | Earlier | Now |
|---|---|---|
| 14 layers | caps refuses "14 layers for 13 planes"; planner `NoPlane{layer:13,OUTPUT\|IN_USE}` | caps refuses "17 layers for 16 planes"; planner `TooManyLayers{count:17}` (the harness now builds 17 layers: one more than the planes) |
| format, modifier | `NoPlane{OUTPUT\|FORMAT}` | `NoPlane{FORMAT}` (the OUTPUT reason no longer appears: every listed plane reaches the output) |
| scale-up, scale-down | planner `NoPlane{OUTPUT\|SCALING}`, TEST_ONLY ok | planner `NoPlane{SCALING}`, TEST_ONLY ok |
| rotation | planner `NoPlane{OUTPUT\|TRANSFORM}`, not reached | planner `NoPlane{TRANSFORM}`, not reached |
| plane-alpha, coverage, off-screen, odd | accept, plan, ok | the same |

— observed 2026-11-28. The layer count differs because the harness sizes the combination from
the plane count; the two lines are not the same test.

### KVM colour check — observed 2026-11-28

Screenshots taken during the 1800-frame run (three taken; the first two were stale and still
showed the fbcon console, the third showed the pattern, as the caveat in `bcm2712.md` says).
Median of an 11x11 box at each `EXPECT` point (scratch script outside the repository;
screenshots are not committed: the console text in them shows the board's address).

| Point | Expected | KVM | Max channel diff |
|---|---|---|---|
| background | #202020 | #2c2c2c | 12 |
| red block | #ff0000 | #dd000f | 34 |
| blue block | #0000ff | #1201e6 | 25 |
| green block uncovered | #00ff00 | #1dff15 | 29 |
| overlay opaque over green | #ffff00 | #eafa15 | 21 |
| overlay opaque over background | #ffff00 | #eafa15 | 21 |
| overlay translucent over green | #807f80 | #7d7d7d | 3 |
| overlay translucent over background | #901090 | #85098b | 11 |

The values equal the earlier record's, point for point. The overlay hides the green block, and
the translucent half blends to grey over green and to purple over the background: z-order and
alpha are correct on the physical output. The up-to-34 offsets on unblended colours are the
capture's (JPEG, HDMI range, colour matrix). — observed (values), inferred (the offset's cause)

## (3) Writeback — observed 2026-11-28

`sudo /tmp/writeback /dev/dri/card1` (exit status 1), verbatim:

```
KERNEL 6.18.50+rpt-rpi-2712
DEVICE /dev/dri/card1
DRIVER vc4
WRITEBACK connector 61 Writeback-1 crtcs [59] formats [RG24,BG24,XR24,XB24,AR24,AB24,RX24,BX24,RA24,BA24]
WRITEBACK connector 80 Writeback-2 crtcs [78] formats [RG24,BG24,XR24,XB24,AR24,AB24,RX24,BX24,RA24,BA24]
MODE 4096x2160@59.999Hz output 1 crtc 59 writeback Writeback-1
PLANE id 48 Primary zpos Fixed(0) plane-alpha true blend-mask 0x7
PLANE id 635 Cursor zpos Range { min: 1, max: 17 } plane-alpha true blend-mask 0x7
VERDICT writeback: FAIL no overlay plane with a linear 32-bit RGB format, driver vc4 mode 4096x2160@59.999Hz via Writeback-1
```

There is no `CHECK` line: the run stopped before any frame was composed, so **no writeback pixel
value exists on this board and the comparison with the KVM samples cannot be made**.

Why: the example takes the first output a writeback connector can capture, which is the TXP CRTC
59 ("mop", CRTC index 0, probed as engine output 1). The planes that reach that CRTC are only
primary 48 and cursor 635. The shared overlays reach CRTC indices 1, 2 and 3 only (plane masks
in `docs/observed/bcm2712-topology.md`), so the capture path of Writeback-1 has no overlay and a
two-plane check cannot be set up there. Writeback-2 (CRTC 78, index 1) should be reachable by the
shared overlays, but the example never tries it: it stops at the first match, and choosing the
output needs a code change. — observed (the output above), inferred (the Writeback-2 remark,
not tried)

How well writeback stands in for a KVM on this board: not at all, as shipped. Two reasons, both
observed: the TXP output the example chooses carries no overlay, and the writeback mode
(4096x2160) is a different CRTC from the one on the HDMI cable (1920x1080), so even a passing
run would show the composition of another CRTC, not the picture on HDMI-A-1. The KVM check
above remains the only physical evidence of z-order and alpha on this board. The comparison of
writeback pixels with KVM samples stays open; see the handoff in the report.

## State after the session — observed 2026-11-28

- debugfs `state`: plane 83 on `crtc-2` with fb 680 allocated by `[fbcon]`, format RG16; plane
  107 has no CRTC.
- KVM: three screenshots after the last run; the third showed the tty1 console (the first two
  were stale harness frames, per the caveat).
- `getty@tty1` and `serial-getty@ttyAMA10` active; no desktop or compositor was running before
  or after.
- `/tmp` on the board holds only the two systemd-private directories it held before.
- `hvs_underrun` reads 0. `config.txt`, the boot loader, boot media and packages untouched.
