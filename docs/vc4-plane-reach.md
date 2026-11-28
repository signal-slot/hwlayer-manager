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

### Before (commit 82a0355)

Date: 2026-11-27 (observed 2026-11-27: the run took place between 20:08Z and 20:17Z). Binaries `probe`, `twoplane` and `drm-harness` were built from a scratch worktree at 82a0355. The board's tty1 showed on the KVM before and after, no display manager existed to restore, and the files copied to the board were removed.

**The detailed notes of this run (plane set, verbatim `twoplane` lines, camera-check verdicts and KVM capture characteristics) were not available to this document.** Only the run's summary survived: it said that camera-check verdicts with exit codes were produced for four phases and that the board was left clean. The four verdicts and the plane set are therefore **not recorded here (not measured in this document)**. A reader needs the original report of the `vc4-rpi5-before` item to fill this in; nothing is invented below.

| | Before |
| --- | --- |
| Kept planes | not recorded |
| Verbatim `EXPECT`/`SWAPZ`/`VERDICT` | not recorded |
| Camera-check verdicts | not recorded (four verdicts existed in the original report) |

### After (post-fix tip e3c6f68)

Date: 2026-11-28 (observed 2026-11-28). Commit: e3c6f68 (observed 2026-11-28). Commands: `probe` and `twoplane --swap-z` on `/dev/dri/card1`. No display manager or compositor held DRM master, so nothing was stopped; the board's `/tmp` was clean afterwards.

Output 0 (1920x1080 @ 60 Hz) kept planes (observed 2026-11-28): 16 planes, all of which reach output 0: primary 83, 14 overlays from 107 through 250, cursor 657. The report gives only the first and last overlay id; the intermediate ids are inferred to follow the 11-id step (not listed individually).

Verbatim lines (observed 2026-11-28):

```
SWAPZ skipped: a plane of the plan has a fixed zpos
EXPECT phase=swapped skipped reason=fixed-zpos
VERDICT twoplane: PASS 1500 frames on two planes, 1500 latched, 2998 released, 0 late, driver vc4 mode 1920x1080@60.000Hz
```

Camera-check (observed 2026-11-28): one KVM screenshot taken mid-run for the normal phase; all 8 regions passed, `VERDICT: PASS`. The swapped phase did not run, so it has no picture. The per-region numbers are in the original report, not here.

Before/after: the plane set before is not recorded (see above), so no comparison of plane sets is possible from this document. The report of the post-fix run itself said the same.

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

### After (post-fix tip)

Date: the notes carry no run date; the item finished on 2026-11-28 (observed 2026-11-28 as the date of the run's completion, the board clock was not recorded). Commit: not recorded (not measured). The board's `/tmp` was clean afterwards and no display manager was running.

**Only the verdict line survived from this run's report**, so the post-fix plane set and the swapped-`EXPECT` line are **not recorded here (not measured in this document)**:

```
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver vc4 mode 1080x1080@60.069Hz
```

(observed 2026-11-28). The plan before the fix was plane 89 for layer 0 and plane 125 for layer 1; a like-for-like run is expected (inferred, not checked) to show the same plan, with the cursor 686 and further overlays kept and the dead primaries dropped. Camera-check: not observed.

## Open points

- The CM4 and RPi4 notes do not give the post-fix commit hash; only the RPi5 does (e3c6f68).
- RPi5 pre-fix notes and RPi4 post-fix notes are incomplete, as stated above: those cells are "not recorded", not zero.
- Planner suggestion carried from the CM4 run: say in the VERDICT line when `--swap-z` was requested but skipped (change belongs in `twoplane.rs`).
- Folding this file into `platform-capabilities.md` and updating X-08 / X-09 in `open-issues.md` is left to the plan's later step; this file edits neither.
