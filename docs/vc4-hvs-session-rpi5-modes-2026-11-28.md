# Raspberry Pi 5 (BCM2712, HVS6): TEST_ONLY modeset of each listed HDMI mode

Short board session answering the "TEST_ONLY modeset of each listed mode" part that
[`vc4-hvs-session-rpi5-scale-2026-11-28.md`](vc4-hvs-session-rpi5-scale-2026-11-28.md) left open for
HVS question 10 of [`hvs-backend-design.md`](hvs-backend-design.md) section 7 ("HDMI limits on BCM2712
(600 MHz) are unverified in the source record"). Everything here is portability evidence and never
counts toward M1, M2 or M3. The page holds no address, host name or serial. Procedure:
[`rk-hvs-session-checklist.md`](rk-hvs-session-checklist.md).

Tags: **observed** on 2026-11-28 = measured on the board (host UTC, from the tool's own `DATE`
line); **read from source** = from the named file; **inferred** = derived here.

## Setup, observed 2026-11-28

| Item | Value |
|---|---|
| Board | Raspberry Pi 5 Model B (BCM2712, HVS6), Raspberry Pi OS Lite arm64 (observed 2026-11-28) |
| Kernel (`uname -r`) | `6.18.50+rpt-rpi-2712` (observed 2026-11-28) |
| DRM device | `/dev/dri/card0` and `/dev/dri/card1` exist; the read-only `probe` printed `device /dev/dri/card1, driver vc4`, so card1 is the display card (card0 is v3d and carries no connector) (observed 2026-11-28) |
| Tool | `crates/composition-drm/examples/mode_test.rs` at tip 4095094, cross-built on the host with `scripts/cross-aarch64.sh --locked -p composition-drm --example mode_test --example probe`; the aarch64-unknown-linux-musl build printed `ELF 64-bit LSB executable, ARM aarch64, ... statically linked` (observed 2026-11-28, build host); copied to the board's `/tmp`, removed afterwards (observed 2026-11-28) |
| Connector | connector 35 on CRTC 94 (the one already driving 1920x1080@60), 20 modes from the attached sink's EDID (observed 2026-11-28) |
| DRM source | master, taken by `mode_test` itself and released at exit (read from source, `mode_test.rs`); the tool would print "DRM master is unavailable" and exit 2 if another client held it, which did not happen: both runs ended with exit status 0 (observed 2026-11-28) |
| Display manager before | `display-manager`, `gdm`, `lightdm`, `sddm`, `weston`: all five `inactive`; no process held the card (observed 2026-11-28) |
| Display manager after | the same five units: all five still `inactive` — none was running, so `scripts/display-guard.sh` was not needed and nothing had to be restored (observed 2026-11-28) |
| Display after | `HDMI-A-1` status `connected` with its mode list unchanged, first entry 1920x1080; nothing was committed, so the board was left as found (observed 2026-11-28, sysfs read) |
| Cleanup | `/tmp/mode_test` and `/tmp/probe` removed; `ls -l /tmp` afterwards shows only the two `systemd-private-*` directories that were already there (observed 2026-11-28) |
| Judging | `scripts/mode-test-expect.py` is not in the tree at this tip, so the logs were judged by hand (observed 2026-11-28) |

What `mode_test` does (read from source, its doc comment): for each mode it builds one atomic
request with a new `MODE_ID` blob, `ACTIVE=1` and the CRTC's primary plane on a dumb XR24 buffer
of the mode size, and submits it with `ATOMIC_TEST_ONLY | ALLOW_MODESET`. Nothing is ever
committed; a refusal is printed as `test=refused errno=E`.

## Runs, observed 2026-11-28

1. 14:16:05Z: `mode_test --device /dev/dri/card1` — every listed mode, all 20. Exit status 0.
2. 14:16:09Z: `mode_test --device /dev/dri/card1 --min-clock 100000` — only the six modes whose
   pixel clock is at least 100 MHz (rows 0 to 3, 5 and 6 below). Exit status 0.

An earlier run of this same session on the same board, recorded at 13:58Z on 2026-11-28, printed
the same two summary lines; the 14:16Z pair reproduced the full output line for line, so each row
below was observed twice on 2026-11-28.

## Per-mode results, observed 2026-11-28

The `#` column is the order the connector lists the modes (the `LISTED` lines); `TEST_ONLY` is the
result of that mode's `ATOMIC_TEST_ONLY | ALLOW_MODESET` request. No mode was refused.

| # | Mode | Pixel clock (kHz) | htotal/vtotal | vrefresh | TEST_ONLY |
|---|---|---|---|---|---|
| 0 | 1920x1080 (preferred) | 148500 | 2200/1125 | 60 | accepted (`test=ok`) |
| 1 | 1920x1080 | 148500 | 2200/1125 | 60 | accepted |
| 2 | 1920x1080 | 148352 | 2200/1125 | 60 | accepted |
| 3 | 1920x1080 | 148500 | 2640/1125 | 50 | accepted |
| 4 | 1920x1080i (`flags=0x15`) | 74250 | 2640/1125 | 50 | accepted |
| 5 | 1280x1024 | 135000 | 1688/1066 | 75 | accepted |
| 6 | 1280x1024 | 108000 | 1688/1066 | 60 | accepted |
| 7 | 1280x720 | 74250 | 1650/750 | 60 | accepted |
| 8 | 1280x720 | 74250 | 1650/750 | 60 | accepted |
| 9 | 1280x720 | 74176 | 1650/750 | 60 | accepted |
| 10 | 1280x720 | 74250 | 1980/750 | 50 | accepted |
| 11 | 1024x768 | 78750 | 1312/800 | 75 | accepted |
| 12 | 1024x768 | 65000 | 1344/806 | 60 | accepted |
| 13 | 800x600 | 49500 | 1056/625 | 75 | accepted |
| 14 | 800x600 | 40000 | 1056/628 | 60 | accepted |
| 15 | 640x480 | 31500 | 840/500 | 75 | accepted |
| 16 | 640x480 | 25200 | 800/525 | 60 | accepted |
| 17 | 640x480 | 25175 | 800/525 | 60 | accepted |
| 18 | 640x480 | 25175 | 800/525 | 60 | accepted |
| 19 | 720x400 | 28320 | 900/449 | 70 | accepted |

Summary lines (verbatim, observed 2026-11-28):

```text
mode_test: tested=20 ok=20 highest_ok_clock=148500        # run 1
mode_test: tested=6  ok=6  highest_ok_clock=148500        # run 2
```

By hand (no judging script on the tip): 20 tested, 20 accepted, no errno, highest accepted clock
148500 kHz. Run 2 confirms the six modes at or above 100 MHz individually and reaches the same
highest clock. Note on names: `mode_test` prints each mode line as `WxH@R` from the mode size and
the rounded refresh (read from source), so the interlaced row 4 appears in the log as
`1920x1080@50 clock=74250`; its `LISTED` line carries `flags=0x15`.

## Raw log excerpt (run 1 in full, run 2's mode lines; observed 2026-11-28)

```text
DATE 2026-11-28T14:16:05Z
KERNEL 6.18.50+rpt-rpi-2712
DEVICE /dev/dri/card1
CONNECTOR 35 crtc 94 modes 20
LISTED 1920x1080 clock=148500 htotal=2200 vtotal=1125 vrefresh=60 flags=0x5 preferred
LISTED 1920x1080 clock=148500 htotal=2200 vtotal=1125 vrefresh=60 flags=0x5
LISTED 1920x1080 clock=148352 htotal=2200 vtotal=1125 vrefresh=60 flags=0x5
LISTED 1920x1080 clock=148500 htotal=2640 vtotal=1125 vrefresh=50 flags=0x5
LISTED 1920x1080i clock=74250 htotal=2640 vtotal=1125 vrefresh=50 flags=0x15
LISTED 1280x1024 clock=135000 htotal=1688 vtotal=1066 vrefresh=75 flags=0x5
LISTED 1280x1024 clock=108000 htotal=1688 vtotal=1066 vrefresh=60 flags=0x5
LISTED 1280x720 clock=74250 htotal=1650 vtotal=750 vrefresh=60 flags=0x5
LISTED 1280x720 clock=74250 htotal=1650 vtotal=750 vrefresh=60 flags=0x5
LISTED 1280x720 clock=74176 htotal=1650 vtotal=750 vrefresh=60 flags=0x5
LISTED 1280x720 clock=74250 htotal=1980 vtotal=750 vrefresh=50 flags=0x5
LISTED 1024x768 clock=78750 htotal=1312 vtotal=800 vrefresh=75 flags=0x5
LISTED 1024x768 clock=65000 htotal=1344 vtotal=806 vrefresh=60 flags=0xa
LISTED 800x600 clock=49500 htotal=1056 vtotal=625 vrefresh=75 flags=0x5
LISTED 800x600 clock=40000 htotal=1056 vtotal=628 vrefresh=60 flags=0x5
LISTED 640x480 clock=31500 htotal=840 vtotal=500 vrefresh=75 flags=0xa
LISTED 640x480 clock=25200 htotal=800 vtotal=525 vrefresh=60 flags=0xa
LISTED 640x480 clock=25175 htotal=800 vtotal=525 vrefresh=60 flags=0xa
LISTED 640x480 clock=25175 htotal=800 vtotal=525 vrefresh=60 flags=0xa
LISTED 720x400 clock=28320 htotal=900 vtotal=449 vrefresh=70 flags=0x6
mode 1920x1080@60 clock=148500 test=ok
mode 1920x1080@60 clock=148500 test=ok
mode 1920x1080@60 clock=148352 test=ok
mode 1920x1080@50 clock=148500 test=ok
mode 1920x1080@50 clock=74250 test=ok
mode 1280x1024@75 clock=135000 test=ok
mode 1280x1024@60 clock=108000 test=ok
mode 1280x720@60 clock=74250 test=ok
mode 1280x720@60 clock=74250 test=ok
mode 1280x720@60 clock=74176 test=ok
mode 1280x720@50 clock=74250 test=ok
mode 1024x768@75 clock=78750 test=ok
mode 1024x768@60 clock=65000 test=ok
mode 800x600@75 clock=49500 test=ok
mode 800x600@60 clock=40000 test=ok
mode 640x480@75 clock=31500 test=ok
mode 640x480@60 clock=25200 test=ok
mode 640x480@60 clock=25175 test=ok
mode 640x480@60 clock=25175 test=ok
mode 720x400@70 clock=28320 test=ok
mode_test: tested=20 ok=20 highest_ok_clock=148500
```

Run 2 (`--min-clock 100000`, 14:16:09Z) printed the same `DATE` to `CONNECTOR` header and all 20
`LISTED` lines; its `mode` lines and summary were:

```text
mode 1920x1080@60 clock=148500 test=ok
mode 1920x1080@60 clock=148500 test=ok
mode 1920x1080@60 clock=148352 test=ok
mode 1920x1080@50 clock=148500 test=ok
mode 1280x1024@75 clock=135000 test=ok
mode 1280x1024@60 clock=108000 test=ok
mode_test: tested=6 ok=6 highest_ok_clock=148500
```

## Q10 status: still open, with its "listed modes" half now settled

- Settled by this session (observed 2026-11-28): the kernel accepts every mode the attached sink
  offers — all 20 on connector 35 / CRTC 94 — under a `TEST_ONLY | ALLOW_MODESET` request, up to
  148500 kHz, including the interlaced 1080i@50. There is no driver refusal anywhere in the sink's
  list.
- Not settled: every tested mode comes from the sink's EDID, and none is above 148.5 MHz. No 4K
  mode, no mode beyond the EDID and no other output was tested (HDMI-A-2 is disconnected), so this
  run cannot show where the BCM2712 HDMI path stops, and the 600 MHz figure in section 7 stays
  unverified here (inferred).
- The source-side limit study (`docs/bcm2712-hdmi-limits-source.md`) was not in the tree at the
  writing of this doc (observed 2026-11-28), so no source figure is cited. A source figure plus a
  refusal (or an acceptance) at or above it would close Q10; a `TEST_ONLY` run against a 4K-capable
  sink could supply the hardware half.
- The 4096x2160@59.999 figure in [`observed/bcm2712-recheck.md`](observed/bcm2712-recheck.md) is
  the TXP writeback connector, not HDMI (observed 2026-11-28 there).

## Not done here

- No real commit: `mode_test` only ever sends `TEST_ONLY` requests (read from source); nothing was
  displayed, changed or photographed, and no KVM picture was taken because the display never moved.
- No mode outside the sink's EDID: no custom modeline was added, and no 4K mode was ever offered
  or requested.
- The other HDMI output (disconnected) was not tested; no connector, cable or configuration was
  touched.
- No `replace_pending`, downscale or real-modeset runs: those belong to the later `rpi5-hvs6-session`.
- No `mode-test-expect.py` judgement: the script is not in the tree at this tip; the verdict above
  is by hand.
