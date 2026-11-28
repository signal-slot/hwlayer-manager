# vc4 plane reach: before and after X-08 / X-09

What `probe` and `twoplane --swap-z` showed on three vc4 boards before and after two fixes: X-09 (`DrmBackend` kept the first `MAX_PLANES` = 16 planes sorted by kind, not by whether they reach a probed output) and X-08 (`twoplane --swap-z` printed the `EXPECT phase=swapped` block before finding that the swap cannot run).

**These runs are not M1, M2 or M3 evidence.** They are portability checks of two defect fixes on Raspberry Pi boards. No result here counts toward M1 (i.MX95), M2 or M3 (AM62P). They used DRM master only (no lease, no scenario script).

Tags: **observed** (always with a date such as 2026-11-27) is something a board printed during a run listed here; **read from source** is stated by repository code; **inferred** is a conclusion drawn from the observed lines; **not measured** was not done. Every run was done by an agent whose written report was handed to this document. Where that report is missing or incomplete the section says so instead of filling in values.

Pre-fix commit for all three boards: `82a0355` (observed 2026-11-27 for CM4 and RPi5, 2026-11-28 for the RPi4 notes; see each section). Post-fix: the integration tip after the X-08 fix (`twoplane.rs`) and the X-09 fix (`crates/composition-drm/src/lib.rs`) had merged.

## What the fixes do

- X-09: read from source, `crates/composition-drm/src/lib.rs`: the plane list is truncated to `MAX_PLANES` (16); the fix orders planes that can reach a probed output ahead of the others before the truncation.
- X-08: read from source, `crates/composition-drm/examples/twoplane.rs`: when the swap will not run, the swapped `EXPECT` block is replaced by one marked line (`EXPECT phase=swapped skipped reason=fixed-zpos`).
- On vc4 every primary plane has a fixed zpos (observed 2026-11-27 on the CM4, `probe`: "fixed 0"), so inferred: `--swap-z` never runs on a vc4 board.

## CM4 (Raspberry Pi Compute Module 4, reTerminal)

Board facts, observed 2026-11-27: model "Raspberry Pi Compute Module 4 Rev 1.0", kernel 6.12.47+rpt-rpi-v8, driver vc4, device `/dev/dri/card0`. 3 connectors, 6 CRTCs, 4 encoders, 60 planes. Only connector 47 (DSI) is connected: CRTC 90, one mode 720x1280 @ 60.000 Hz. HDMI connectors 33 and 42 are disconnected. No display manager or compositor was running before or after (observed 2026-11-27); nothing was stopped. No KVM or camera on this board.

Plane inventory (observed 2026-11-27):

| Planes | Type | Possible CRTCs |
| --- | --- | --- |
| 48, 67, 79, 91, 103, 115 | primary | 59, 78, 90, 102, 114, 126 (one each) |
| 127 to 292 step 11 (16 planes) | overlay | 78, 90, 102, 114, 126 |
| 303 to 644 step 11 (32 planes) | overlay | 59 only |
| 655, 666, 677, 688, 699, 710 | cursor | 59, 78, 90, 102, 114, 126 (one each) |

### Before (commit 82a0355)

Date: 2026-11-27, board session 20:11Z to 20:12Z, UTC. Binaries cross-built for `aarch64-unknown-linux-musl` from a scratch worktree at 82a0355 and run as an unprivileged user in the `video` group. Commands (observed 2026-11-27):

```
probe /dev/dri/card0
probe --raw /dev/dri/card0
twoplane /dev/dri/card0 --swap-z
```

Output 0 (CRTC 90) kept planes (observed 2026-11-27):

| Output 0 | Kept planes |
| --- | --- |
| Primary | 48, 67, 91, 103, 115 (do not reach output 0, mask `0b0`); 79 (reaches it) |
| Overlays | 127, 138, 149, 160, 171, 182, 193, 204, 215, 226 (10, all reach it) |
| Cursor | none |
| Reachable but dropped | overlays 237, 248, 259, 270, 281, 292 and cursor 677 |

Observed 2026-11-27: 16 kept, of which 5 are primaries that reach no output and 11 reach output 0.

Verbatim `twoplane --swap-z` lines (observed 2026-11-27):

```
EXPECT phase=swapped "background" x=36 y=896 fx=0.0500 fy=0.7000 rgb=#202020
EXPECT phase=swapped "red block" x=144 y=480 fx=0.2000 fy=0.3750 rgb=#ff0000
EXPECT phase=swapped "blue block" x=576 y=480 fx=0.8000 fy=0.3750 rgb=#0000ff
EXPECT phase=swapped "green block uncovered" x=324 y=480 fx=0.4500 fy=0.3750 rgb=#00ff00
EXPECT phase=swapped "overlay opaque over green" x=396 y=480 fx=0.5500 fy=0.3750 rgb=#00ff00
EXPECT phase=swapped "overlay opaque over background" x=468 y=480 fx=0.6500 fy=0.3750 rgb=#202020
EXPECT phase=swapped "overlay translucent over green" x=396 y=720 fx=0.5500 fy=0.5625 rgb=#00ff00
EXPECT phase=swapped "overlay translucent over background" x=468 y=800 fx=0.6500 fy=0.6250 rgb=#202020
PLAN from frame 0:
  layer 0 (background) -> plane 79 Primary zpos 0 src 0,0 720x1280 dst 0,0 720x1280
  layer 1 (overlay) -> plane 127 Overlay zpos 1 src 0,0 144x640 dst 360,320 144x640
SWAPZ skipped: a plane of the plan has a fixed zpos
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver vc4 mode 720x1280@60.000Hz
```

The eight `EXPECT phase=normal` lines were printed before these (observed 2026-11-27; the CM4 report has them, they are not repeated here). Run totals (observed 2026-11-27): `RESULT frames=600 committed=600 latched=600 released=1198 dropped=0 late=0`. X-08 reproduced (observed 2026-11-27): the swapped block comes before the `SWAPZ skipped` line. Camera-check verdict: not observed (no KVM or camera on this board).

### After (post-fix tip)

Date: 2026-11-27, 20:21Z to 20:23Z, UTC (observed 2026-11-27). Commit: the integration tip after both fixes; the notes give no hash for this run (not measured). Same three commands, same board, nothing to stop, board left as found.

Output 0 (CRTC 90) kept planes (observed 2026-11-27, from the run's report: output 0 can reach 18 planes, keeps 16, and overlays 281 and 292 are dropped by the cap):

| Output 0 | Kept planes |
| --- | --- |
| Primary | 79 |
| Overlays | 127 to 270 step 11 (14) |
| Cursor | 677 |
| Reachable but dropped | overlays 281 and 292 |

The overlay range and the cursor id are inferred: the report names only the two dropped overlays and the total of 16, and the inventory above gives the ids. No unreachable primary is kept (inferred from 1 + 14 + 1 = 16).

Verbatim lines (observed 2026-11-27):

```
EXPECT phase=swapped skipped reason=fixed-zpos
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver vc4 mode 720x1280@60.000Hz
```

The full post-fix `SWAPZ` and `PLAN` lines are not in the notes this document was built from (not measured here). No unmarked swapped `EXPECT` block was reported (observed 2026-11-27). The VERDICT line does not say that `--swap-z` was asked for but skipped (observed 2026-11-27); a change to say so is proposed for `twoplane.rs`, not made here. Losing overlays 281 and 292 to the 16-plane cap is a `composition-core` decision, not a defect of the fix. Camera-check verdict: not observed.

### After, re-run with full lines (commit 0ba62fc)

Date: 2026-11-28, 07:46Z UTC (observed 2026-11-28, board clock). Commit: `0ba62fc34774b893e13357e7dd6d2f600065ae9a`, the integration tip, built for `aarch64-unknown-linux-musl` on the host with `scripts/cross-aarch64.sh` and copied to the board's `/tmp`, run as an unprivileged user (observed 2026-11-28). No display manager or compositor was running; nothing was stopped; `/tmp` was cleaned afterwards and the board left as found (observed 2026-11-28). Commands: `probe /dev/dri/card0` and `twoplane /dev/dri/card0 --swap-z`.

Output order differed from the 2026-11-27 runs (observed 2026-11-28): the HDMI connector now reads as connected (the KVM is attached to it) and is output 0 (CRTC 102, 640x480 @ 75.000 Hz, the console mode); the DSI panel is output 1 (CRTC 90, 720x1280 @ 60.000 Hz). The planes in the table above are unchanged.

Kept planes (observed 2026-11-28, from the `probe` plane list, 16 planes of 60):

| | Kept planes |
| --- | --- |
| Primary | 79 (reaches output 1 only, mask `0b10`), 91 (reaches output 0 only, mask `0b1`) |
| Overlays | 127, 138, 149, 160, 171, 182, 193, 204, 215, 226, 237, 248 (12, mask `0b11`) |
| Cursor | 677 (output 1 only), 688 (output 0 only) |

Every kept plane reaches at least one of the two probed outputs and no other plane does (observed 2026-11-28); no overlay that reaches an output is dropped with only two outputs probed (inferred: 2 + 12 + 2 = 16).

Verbatim lines of `twoplane --swap-z` (observed 2026-11-28):

```
PLAN from frame 0:
  layer 0 (background) -> plane 91 Primary zpos 0 src 0,0 640x480 dst 0,0 640x480
  layer 1 (overlay) -> plane 127 Overlay zpos 1 src 0,0 128x240 dst 320,120 128x240
SWAPZ skipped: a plane of the plan has a fixed zpos
EXPECT phase=swapped skipped reason=fixed-zpos
RESULT frames=600 committed=600 latched=600 released=1198 dropped=0 late=0 period_ms=13.333 latch_interval_mean_ms=13.330 latch_interval_max_ms=13.443 elapsed_s=7.993
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver vc4 mode 640x480@75.000Hz
```

No unmarked swapped `EXPECT` block was printed (observed 2026-11-28); the `EXPECT phase=swapped skipped reason=fixed-zpos` line comes after the eight `EXPECT phase=normal` lines. A second run with `--frames 3000` printed the same `PLAN`, `SWAPZ` and swapped-`EXPECT` lines and `VERDICT twoplane: PASS 3000 frames on two planes, 3000 latched, 5998 released, 0 late, driver vc4 mode 640x480@75.000Hz` (observed 2026-11-28). The `VERDICT` line still does not say that `--swap-z` was skipped (observed 2026-11-28).

Camera-check verdict: not observed. The KVM listed for this board captures the HDMI output, not the DSI panel. `twoplane` used the HDMI console mode 640x480 @ 75 Hz and has no option to choose another mode, and the KVM screenshot tool returned "failed to capture screenshot" twice while the 3000-frame run was drawing (observed 2026-11-28). `scripts/camera-check.py` was therefore not run.

### CM4 before/after

| | Before 82a0355 | After |
| --- | --- | --- |
| Primary kept | 6 (79 reaches; 5 dead) | 1 (79) |
| Overlays kept | 10 | 14 |
| Cursor kept | none | 677 |
| Swapped EXPECT | 8 unmarked lines, then SWAPZ skipped | `EXPECT phase=swapped skipped reason=fixed-zpos` |
| Verdict | PASS 600 latched | PASS 600 latched |

## RPi5 (BCM2712)

Both runs used a KVM for pictures. The DRM device is `/dev/dri/card1`: `card0` is the v3d GPU and `probe` on it fails with "Not supported" (observed 2026-11-28, post-fix run).

Board facts, observed 2026-11-28 (re-run, board clock 09:02Z to 09:12Z UTC): model "Raspberry Pi 5 Model B Rev 1.1", kernel 6.18.50+rpt-rpi-2712, `card1` bound to `vc4-drm`, `card0` to `v3d`. `probe --raw /dev/dri/card1`: 2 connectors, 4 CRTCs, 4 encoders, 56 planes. Connector 35 (HDMI-A, the KVM) is connected on CRTC 94, 1920x1080 @ 60.000 Hz; connector 44 is disconnected. No display manager or compositor ran before, during or after (observed 2026-11-28: `display-manager`, `lightdm`, `gdm`, `sddm` inactive; no weston, labwc, wayfire or Xorg process), so nothing was stopped or restarted. The tty1 login prompt showed on the KVM before the runs and again after each run.

Plane inventory (observed 2026-11-28, `probe --raw`):

| Planes | Type | Possible CRTCs |
| --- | --- | --- |
| 48, 67, 83, 95 | primary | 59, 78, 94, 106 (one each) |
| 107 to 272 step 11 (16 planes) | overlay | 78, 94, 106 |
| 283 to 624 step 11 (32 planes) | overlay | 59 only |
| 635, 646, 657, 668 | cursor | 59, 78, 94, 106 (one each) |

So 18 planes can reach output 0 (CRTC 94): primary 83, the 16 overlays 107 to 272, cursor 657 (inferred from the table).

### Re-run 2026-11-28: builds and method

Both builds were made on the host with `scripts/cross-aarch64.sh -p composition-drm --example probe --example twoplane` (static `aarch64-unknown-linux-musl`), copied to the board's `/tmp` and run as the unprivileged board user with DRM master (no lease). The board's `/tmp` was removed afterwards and matched its listing from before the run (observed 2026-11-28).

- Pre-fix: commit `82a0355b995ca147095fabf3ea24419600ed5561` (a git worktree of that commit, built with `--manifest-path`).
- Post-fix: commit `63b29bfb410b255e3d24cf494be97d76d92cc9de`, the integration tip of 2026-11-28, which holds the X-08 fix, the X-09 fix, the `KEPT` line of `probe` and the `swapz=` field of the `VERDICT` line.

Commands per build: `probe /dev/dri/card1`, then `twoplane /dev/dri/card1 --swap-z --frames 2400` (swap point frame 1200), then a plain `twoplane /dev/dri/card1 --frames 1200`, because the swapped phase was skipped.

KVM pictures: the KVM screenshot tool returned frames that were up to about a minute old after it had been idle (observed 2026-11-28: it still showed the test pattern more than 60 s after `twoplane` had exited, then the console). Each capture used here was therefore tied to its run by two checks: the KVM showed the console just before the run started and again after it ended, and the frame number decoded from the picture (8-bit counter plus bar position, unique below frame 29952) rises within the run. The decoded frame numbers are given with each verdict. A first pair of runs at 09:04Z to 09:06Z is not used: its decoded frames showed that the captures labelled "post-fix" came from the pre-fix run (observed 2026-11-28).

EXPECT.json files were made with `scripts/twoplane-expect.py OUTPUT --out DIR --panel full --rotation 0` from each run's own output, and every picture was checked with `scripts/camera-check.py PICTURE DIR/expect-normal.json`. For both `--swap-z` runs `twoplane-expect.py` wrote only `expect-normal.json` and printed `skipped phase swapped` (observed 2026-11-28): for the pre-fix output it takes the skip from the `SWAPZ skipped` line, because the unmarked swapped block alone does not say so.

### Before (commit 82a0355)

Run of 2026-11-28 (observed 2026-11-28; `twoplane` printed `DATE 2026-11-28T09:08:55Z`). An earlier pre-fix run on 2026-11-27 (20:08Z to 20:17Z) also took place, but its notes were lost; the values below are from the 2026-11-28 re-run.

Output 0 (CRTC 94) kept planes, observed 2026-11-28 from the `probe` plane list (16 planes, the `MAX_PLANES` cap):

| Output 0 (CRTC 94) | Kept planes, before |
| --- | --- |
| Primary | 48, 67, 95 (mask `0b0`, dead); 83 (reaches) |
| Overlays | 107, 118, 129, 140, 151, 162, 173, 184, 195, 206, 217, 228 (12, all reach) |
| Cursor | none |
| Dropped though reachable | overlays 239, 250, 261, 272 and cursor 657 |

Observed 2026-11-28: 13 of the 16 kept planes reach the output, 3 are dead primaries. `twoplane` listed the same 13 reachable planes as `PLANE index 2 id 83` to `PLANE index 15 id 228`.

Verbatim lines of `twoplane --swap-z --frames 2400` (observed 2026-11-28), in printed order; the swapped block comes before the plan and the skip:

```
EXPECT phase=normal "background" x=96 y=756 fx=0.0500 fy=0.7000 rgb=#202020
EXPECT phase=normal "red block" x=384 y=405 fx=0.2000 fy=0.3750 rgb=#ff0000
EXPECT phase=normal "blue block" x=1536 y=405 fx=0.8000 fy=0.3750 rgb=#0000ff
EXPECT phase=normal "green block uncovered" x=864 y=405 fx=0.4500 fy=0.3750 rgb=#00ff00
EXPECT phase=normal "overlay opaque over green" x=1056 y=405 fx=0.5500 fy=0.3750 rgb=#ffff00
EXPECT phase=normal "overlay opaque over background" x=1248 y=405 fx=0.6500 fy=0.3750 rgb=#ffff00
EXPECT phase=normal "overlay translucent over green" x=1056 y=607 fx=0.5500 fy=0.5620 rgb=#807f80
EXPECT phase=normal "overlay translucent over background" x=1248 y=675 fx=0.6500 fy=0.6250 rgb=#901090
EXPECT phase=swapped "background" x=96 y=756 fx=0.0500 fy=0.7000 rgb=#202020
EXPECT phase=swapped "red block" x=384 y=405 fx=0.2000 fy=0.3750 rgb=#ff0000
EXPECT phase=swapped "blue block" x=1536 y=405 fx=0.8000 fy=0.3750 rgb=#0000ff
EXPECT phase=swapped "green block uncovered" x=864 y=405 fx=0.4500 fy=0.3750 rgb=#00ff00
EXPECT phase=swapped "overlay opaque over green" x=1056 y=405 fx=0.5500 fy=0.3750 rgb=#00ff00
EXPECT phase=swapped "overlay opaque over background" x=1248 y=405 fx=0.6500 fy=0.3750 rgb=#202020
EXPECT phase=swapped "overlay translucent over green" x=1056 y=607 fx=0.5500 fy=0.5620 rgb=#00ff00
EXPECT phase=swapped "overlay translucent over background" x=1248 y=675 fx=0.6500 fy=0.6250 rgb=#202020
PLAN from frame 0:
  layer 0 (background) -> plane 83 Primary zpos 0 src 0,0 1920x1080 dst 0,0 1920x1080
  layer 1 (overlay) -> plane 107 Overlay zpos 1 src 0,0 384x540 dst 960,270 384x540
SWAPZ skipped: a plane of the plan has a fixed zpos
RESULT frames=2400 committed=2400 latched=2400 released=4798 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=16.667 latch_interval_max_ms=17.382 elapsed_s=39.998
RELEASE after the run: released 4800 of 4800 buffer uses, 0 still on screen
VERDICT twoplane: PASS 2400 frames on two planes, 2400 latched, 4798 released, 0 late, driver vc4 mode 1920x1080@60.000Hz
```

Plain `twoplane --frames 1200` (observed 2026-11-28, `DATE 2026-11-28T09:11:23Z`): the same `PLAN` lines, no swapped `EXPECT` lines, and

```
RESULT frames=1200 committed=1200 latched=1200 released=2398 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=16.667 latch_interval_max_ms=16.700 elapsed_s=19.988
VERDICT twoplane: PASS 1200 frames on two planes, 1200 latched, 2398 released, 0 late, driver vc4 mode 1920x1080@60.000Hz
```

Camera-check verdicts, before (observed 2026-11-28; KVM pictures 1920x1080, `expect-normal.json` of the same run):

| Picture | Decoded frame | Verdict line | Exit |
| --- | --- | --- | --- |
| `--swap-z` run, first half (normal phase) | 408 | `VERDICT: PASS` (8 of 8 regions) | 0 |
| `--swap-z` run, second half (swap skipped, normal picture) | 2252 | `VERDICT: PASS` (8 of 8 regions) | 0 |
| plain run | 245 | `VERDICT: PASS` (8 of 8 regions) | 0 |

The largest region distance was 39.0 of tolerance 40 (red block, measured `(219, 1, 15)`), observed 2026-11-28 in all three pictures.

What the unmarked swapped block would have done (observed 2026-11-28): the same pre-fix output with its `SWAPZ` line removed gives `twoplane-expect.py ... --phase swapped` a swapped file (8 regions, exit 0). Checking the second-half picture (frame 2252) against it gives `VERDICT: FAIL`, exit 1: background and the three blocks pass, and all four overlay regions fail (`overlay opaque over green` distance 235.9, `overlay opaque over background` 298.9, `overlay translucent over green` 218.9, `overlay translucent over background` 149.1). The screen was right; the expectations described a phase that never ran. That is X-08.

### After (commit 63b29bf)

Run of 2026-11-28 (observed 2026-11-28; `twoplane` printed `DATE 2026-11-28T09:09:51Z`).

`probe` printed (observed 2026-11-28):

```
KEPT output=0 crtc=94 primary=83 overlay=107,118,129,140,151,162,173,184,195,206,217,228,239,250 cursor=657 dropped_reachable=261,272
```

| Output 0 (CRTC 94) | Kept planes, after |
| --- | --- |
| Primary | 83 (reaches) |
| Overlays | 107, 118, 129, 140, 151, 162, 173, 184, 195, 206, 217, 228, 239, 250 (14, all reach) |
| Cursor | 657 (reaches) |
| Dropped though reachable | overlays 261, 272 |

Observed 2026-11-28: all 16 kept planes reach output 0, and `twoplane` listed the same 16 as `PLANE index 0 id 83` to `PLANE index 15 id 657`. Two reachable overlays are still dropped because 18 planes reach the output and the cap is 16 (inferred). This matches the earlier post-fix run at commit e3c6f68 (observed 2026-11-28), which reported primary 83, 14 overlays 107 to 250 and cursor 657, one KVM picture of the normal phase with `VERDICT: PASS` (8 of 8 regions), and these lines:

```
SWAPZ skipped: a plane of the plan has a fixed zpos
EXPECT phase=swapped skipped reason=fixed-zpos
VERDICT twoplane: PASS 1500 frames on two planes, 1500 latched, 2998 released, 0 late, driver vc4 mode 1920x1080@60.000Hz
```

That run was made before the KVM's stale frames were known, so its picture was not tied to its run by frame number (inferred).

Verbatim lines of `twoplane --swap-z --frames 2400` (observed 2026-11-28), in printed order:

```
PLAN from frame 0:
  layer 0 (background) -> plane 83 Primary zpos 0 src 0,0 1920x1080 dst 0,0 1920x1080
  layer 1 (overlay) -> plane 107 Overlay zpos 1 src 0,0 384x540 dst 960,270 384x540
SWAPZ skipped: a plane of the plan has a fixed zpos
EXPECT phase=normal "background" x=96 y=756 fx=0.0500 fy=0.7000 rgb=#202020
EXPECT phase=normal "red block" x=384 y=405 fx=0.2000 fy=0.3750 rgb=#ff0000
EXPECT phase=normal "blue block" x=1536 y=405 fx=0.8000 fy=0.3750 rgb=#0000ff
EXPECT phase=normal "green block uncovered" x=864 y=405 fx=0.4500 fy=0.3750 rgb=#00ff00
EXPECT phase=normal "overlay opaque over green" x=1056 y=405 fx=0.5500 fy=0.3750 rgb=#ffff00
EXPECT phase=normal "overlay opaque over background" x=1248 y=405 fx=0.6500 fy=0.3750 rgb=#ffff00
EXPECT phase=normal "overlay translucent over green" x=1056 y=607 fx=0.5500 fy=0.5620 rgb=#807f80
EXPECT phase=normal "overlay translucent over background" x=1248 y=675 fx=0.6500 fy=0.6250 rgb=#901090
EXPECT phase=swapped skipped reason=fixed-zpos
RESULT frames=2400 committed=2400 latched=2400 released=4798 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=16.667 latch_interval_max_ms=16.706 elapsed_s=39.987
RELEASE after the run: released 4800 of 4800 buffer uses, 0 still on screen
VERDICT twoplane: PASS 2400 frames on two planes, 2400 latched, 4798 released, 0 late, driver vc4 mode 1920x1080@60.000Hz, swapz=skipped(fixed-zpos)
```

Plain `twoplane --frames 1200` (observed 2026-11-28, `DATE 2026-11-28T09:11:51Z`): the same `PLAN` lines, and

```
RESULT frames=1200 committed=1200 latched=1200 released=2398 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=16.667 latch_interval_max_ms=16.692 elapsed_s=19.991
VERDICT twoplane: PASS 1200 frames on two planes, 1200 latched, 2398 released, 0 late, driver vc4 mode 1920x1080@60.000Hz, swapz=off
```

Camera-check verdicts, after (observed 2026-11-28; same method):

| Picture | Decoded frame | Verdict line | Exit |
| --- | --- | --- | --- |
| `--swap-z` run, first half (normal phase) | 746 | `VERDICT: PASS` (8 of 8 regions) | 0 |
| `--swap-z` run, second half (swap skipped, normal picture) | 1544 | `VERDICT: PASS` (8 of 8 regions) | 0 |
| plain run | 246 | `VERDICT: PASS` (8 of 8 regions) | 0 |

The swapped phase did not run, so it has no picture and no expectation file (observed 2026-11-28).

### RPi5 before/after

| | Before 82a0355 | After 63b29bf |
| --- | --- | --- |
| Kept planes reaching output 0 | 13 of 16 | 16 of 16 |
| Primary kept | 4 (83 reaches; 48, 67, 95 dead) | 1 (83) |
| Overlays kept | 12 (107 to 228) | 14 (107 to 250) |
| Cursor kept | none | 657 |
| Reachable but dropped | 239, 250, 261, 272, cursor 657 | 261, 272 |
| `PLAN` | plane 83 + plane 107 | plane 83 + plane 107 (same) |
| Swapped `EXPECT` | 8 unmarked lines before `PLAN`, then `SWAPZ skipped` | `EXPECT phase=swapped skipped reason=fixed-zpos` after `SWAPZ skipped` |
| `VERDICT` | PASS 2400 latched, 4798 released, 0 late | PASS 2400 latched, 4798 released, 0 late, `swapz=skipped(fixed-zpos)` |
| Camera-check (3 pictures each) | PASS, PASS, PASS (exit 0) | PASS, PASS, PASS (exit 0) |

All values observed 2026-11-28. Inferred from these lines:

- X-09 is fixed on this board: the dead primaries 48, 67 and 95 are no longer kept, and the cursor 657 and overlays 239 and 250 take their slots. The plan `twoplane` makes is unchanged (planes 83 and 107), so the fix does not change what this test draws; it changes which planes are available to a scene with more layers.
- X-08 is fixed on this board: the post-fix output has no unmarked swapped block, and the `VERDICT` line says the swap was skipped. Before the fix, a check built from the `EXPECT` lines alone fails on a correct screen (FAIL, exit 1, above).
- Camera-check verdicts are the same before and after, as expected: both builds draw the same two-plane picture.

## RPi4 (BCM2711)

The DRM device is `/dev/dri/card1` (vc4 KMS); `card0` is v3d and cannot do KMS (observed 2026-11-27). Kernel 6.12.62+rpt-rpi-v8 (observed 2026-11-27). No KVM or camera: camera-check verdict not observed, before and after. No display manager or compositor ran (observed 2026-11-27: `display-manager`, `lightdm`, `gdm`, `sddm` inactive, no weston, labwc, wayfire or Xorg process).

Device (observed 2026-11-27): 60 planes (6 primary, 48 overlay, 6 cursor), 6 CRTCs, 2 HDMI connectors, 3 encoders. One output: connector 33 (HDMI-A) connected on CRTC 100, 1080x1080 @ 60.069 Hz. Connector 42 is disconnected.

### Before (commit 82a0355)

Date: 2026-11-27, board clock 22:25:09Z (observed 2026-11-27). Built with `scripts/cross-aarch64.sh -p composition-drm --example probe --example twoplane`. Commands (observed 2026-11-27):

```
probe /dev/dri/card1
probe --raw /dev/dri/card1
twoplane /dev/dri/card1 --swap-z --output 0
```

| Output 0 (CRTC 100) | Kept planes (observed 2026-11-27) |
| --- | --- |
| Primary | 46, 65, 77, 101, 113 (mask `0b0`, dead); 89 (reaches) |
| Overlays | 125, 136, 147, 158, 169, 180, 191, 202, 213, 224 (10, all reach) |
| Cursor | none |
| Dropped though reachable | 38 overlays, and cursor 686 |

Observed 2026-11-27: 11 of the 16 kept planes reach the output, 5 are dead primaries.

Verbatim lines (observed 2026-11-27), swapped block printed before the skip:

```
EXPECT phase=swapped "background" x=54 y=756 fx=0.0500 fy=0.7000 rgb=#202020
PLAN from frame 0:
  layer 0 (background) -> plane 89 Primary zpos 0 src 0,0 1080x1080 dst 0,0 1080x1080
  layer 1 (overlay) -> plane 125 Overlay zpos 1 src 0,0 216x540 dst 540,270 216x540
SWAPZ skipped: a plane of the plan has a fixed zpos
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver vc4 mode 1080x1080@60.069Hz
```

The first line is one of eight swapped lines; the other seven (and the eight `normal` lines) are in the original report. The four overlay lines read `rgb=#00ff00` / `#202020` / `#00ff00` / `#202020` (opaque over green, opaque over background, translucent over green, translucent over background). `RESULT frames=600 committed=600 latched=600 released=1198 dropped=0 late=0` (observed 2026-11-27). Camera-check: not observed.

### After (post-fix tip, commit 62086e2)

Date: 2026-11-28, board clock 08:40:25Z to 08:40:35Z (observed 2026-11-28). Commit 62086e2 (observed 2026-11-28, the integration tip the binaries were built from, `scripts/cross-aarch64.sh -p composition-drm --example probe --example twoplane`). Kernel 6.12.62+rpt-rpi-v8 (observed 2026-11-28). The binaries were copied to the board's `/tmp`, run as an unprivileged user in the `video` group and removed afterwards; `/tmp` held no files of the run afterwards (observed 2026-11-28). No display manager or compositor ran before or after (observed 2026-11-28: `display-manager`, `lightdm`, `gdm`, `sddm` inactive, no weston, labwc, wayfire, cage or Xorg process), so nothing was stopped or restored. Commands (observed 2026-11-28):

```
probe /dev/dri/card1
twoplane /dev/dri/card1 --swap-z --output 0
```

Output 0 (CRTC 100) kept planes (observed 2026-11-28, from the `KEPT` line of `probe`):

| Output 0 (CRTC 100) | Kept planes |
| --- | --- |
| Primary | 89 |
| Overlays | 125, 136, 147, 158, 169, 180, 191, 202, 213, 224, 235, 246, 257, 268 (14, all reach) |
| Cursor | 686 |
| Reachable but dropped | overlays 279, 290 |

Verbatim `probe` line (observed 2026-11-28):

```
KEPT output=0 crtc=100 primary=89 overlay=125,136,147,158,169,180,191,202,213,224,235,246,257,268 cursor=686 dropped_reachable=279,290
```

Verbatim `twoplane` lines (observed 2026-11-28):

```
PLAN from frame 0:
  layer 0 (background) -> plane 89 Primary zpos 0 src 0,0 1080x1080 dst 0,0 1080x1080
  layer 1 (overlay) -> plane 125 Overlay zpos 1 src 0,0 216x540 dst 540,270 216x540
SWAPZ skipped: a plane of the plan has a fixed zpos
EXPECT phase=swapped skipped reason=fixed-zpos
RESULT frames=600 committed=600 latched=600 released=1198 dropped=0 late=0 period_ms=16.648 latch_interval_mean_ms=16.646 latch_interval_max_ms=16.822 elapsed_s=9.986
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver vc4 mode 1080x1080@60.069Hz, swapz=skipped(fixed-zpos)
```

Before/after (observed 2026-11-27 before, 2026-11-28 after):

| | Before (82a0355) | After (62086e2) |
| --- | --- | --- |
| Kept planes | 16 | 16 |
| Primaries kept | 6 (5 dead, 1 reaches) | 1 (reaches) |
| Overlays kept | 10 (all reach) | 14 (all reach) |
| Cursor kept | none | 686 |
| Kept planes that reach output 0 | 11 of 16 | 16 of 16 |
| Plan | layer 0 on 89, layer 1 on 125 | layer 0 on 89, layer 1 on 125 (same) |
| Swapped EXPECT | 8 lines printed, then the swap was skipped | one line, `skipped reason=fixed-zpos` |
| VERDICT | PASS, 600 latched, 1198 released, 0 late | PASS, 600 latched, 1198 released, 0 late, `swapz=skipped(fixed-zpos)` |

The earlier statement that the plan would stay the same, with cursor 686 kept and the dead primaries dropped, is confirmed by this run (observed 2026-11-28). Dropped though reachable: before, 38 overlays and cursor 686 (observed 2026-11-27); after, 2 overlays, 279 and 290 (observed 2026-11-28). An earlier post-fix run of this board (observed 2026-11-28, no commit recorded) had the same verdict except for the `swapz=skipped` suffix. Camera-check: not observed (no KVM or camera on this board).


## Open points

- Post-fix commits: CM4 0ba62fc, RPi4 62086e2, RPi5 e3c6f68.
- The RPi5 pre-fix values come from the 2026-11-28 re-run; the notes of the 2026-11-27 run were lost (observed 2026-11-28).
- Planner suggestion carried from the CM4 run: say in the VERDICT line when `--swap-z` was requested but skipped (change belongs in `twoplane.rs`).
- Folding this file into `platform-capabilities.md` and updating X-08 / X-09 in `open-issues.md` is left to the plan's later step; this file edits neither.
