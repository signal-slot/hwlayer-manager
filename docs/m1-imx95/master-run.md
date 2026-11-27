# M1 master-fd run on the Verdin iMX95 (2026-11-27)

Board session of item `m1-master-run` (SoW §9 M1; test plan `docs/m1-imx95-test-plan.md` §5–§7, §9). `tools/drm-harness` drove `composition-drm` on the LVDS output as DRM master, with the `imx95` profile, and a camera photographed the panel. Everything here was observed on 2026-11-27 between 19:33 and 19:51 UTC (host and board clocks agreed to the second). This page summarises; the logs, pictures and check outputs next to it are the evidence.

The lease route (condition 6) is not part of this run.

## Setup (observed 2026-11-27)

| Item | Value | Source |
| --- | --- | --- |
| Board | Toradex Verdin iMX95 on a Mallow carrier; `/proc/device-tree/model`: "Toradex Verdin iMX95 WB on Verdin Development Board"; SoC i.MX95 revision 2.0 | read on the board, 19:34 UTC |
| Panel | Toradex Capacitive Touch Display 10.1" LVDS on X12; connector `LVDS-1` (id 41), CRTC 39, 1280x800, `refresh_mhz=59993`, pixel clock 71100 kHz | harness `output` record |
| Image | `TDX Wayland with XWayland 7.8.0+build.17 (scarthgap)`, `VERSION_ID=7.8.0-build.17` (Toradex Reference Multimedia Image 7.8.0) | `/etc/os-release` |
| Kernel | `uname -r`: `6.6.157-7.8.0` (`#1 SMP PREEMPT Fri Sep 25 14:06:41 UTC 2026`), driver `imx95-dpu` 1.0.0 | board, harness `session` record |
| Display client | Weston 12.0.4 (`weston.service`, socket-activated by `weston.socket`) and the demo `wayland-app-launch.service` (`Qt5_CinematicExperience --fullscreen`, `Requires=weston.service`) | `systemctl`, `weston --version` |
| Harness | `drm-harness` 0.1.0 at the integration tip `76155cc7acdd`, `scripts/cross-aarch64.sh -p composition-drm-harness --bin drm-harness` (static, `aarch64-unknown-linux-musl`), run from `/tmp` | host build |
| Command line | `/tmp/drm-harness --connector LVDS --profile imx95 [--seconds N] [--frames N] [--buffers K] <scenario>` (exact lines are the first lines of each log) | logs |
| Camera | the farm's camera on the panel; raw 4608x2592 frames, upside down; exposure, focus and white balance locked from about 19:36:30 UTC (shutter 60 ms, gain 2.57) and unlocked at the end | camera tool |

## Weston before and after

- **Before** (19:34:23 UTC): `weston.service` active, the demo client full screen on the panel ([picture](pictures/20261127T193355Z-weston-before-raw.jpg)); `/dev/dri/card0` held by `weston`, `systemd`, `systemd-logind`.
- **Stopped** at 19:34:43 UTC with `systemctl stop weston.service`, the prior state recorded in the board's `/tmp` (the `display-owner` record format of `scripts/imx95-session.sh`). systemd also stopped `wayland-app-launch.service` (it `Requires=` Weston). `weston.socket` stayed listening (systemd warned that it could re-activate Weston); it did not: Weston was inactive before every run.
- **Restored**: `systemctl start weston.service` at 19:47:04 UTC → `active` (new main PID); then `systemctl start wayland-app-launch.service` → `active` at 19:47:35 UTC, the demo back on the panel ([picture](pictures/20261127T194740Z-weston-after-2-raw.jpg)). Last check 19:50:37 UTC: both `active`, same boot (no reboot during the session), board `/tmp` as before the session (every copied file and output removed).

## VERDICT lines (observed 2026-11-27, kernel 6.6.157-7.8.0)

| Date (UTC) | Log | VERDICT line, as printed |
| --- | --- | --- |
| 2026-11-27 19:34:50 | [combos.log](logs/combos.log) | `VERDICT combos PASS 16 accepted, 4 refused by the planner, 0 refused by TEST_ONLY, nothing committed` |
| 2026-11-27 19:35:02–19:36:02 | [two-planes.log](logs/two-planes.log) | `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)` |
| 2026-11-27 19:36:14–19:36:44 | [calib-two-planes.log](logs/calib-two-planes.log) | `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)` (pattern shown while the camera was locked) |
| 2026-11-27 19:38:40–19:40:10 | [two-planes-cam.log](logs/two-planes-cam.log) | `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)` (the camera run) |
| 2026-11-27 19:39:51 | [zorder-alpha-attempt0-busy.log](logs/zorder-alpha-attempt0-busy.log) | `VERDICT zorder-alpha FAIL not DRM master (Resource busy (os error 16)); ...` — operator error, see below |
| 2026-11-27 19:40:11–19:42:51 | [zorder-alpha.log](logs/zorder-alpha.log) | `VERDICT zorder-alpha PASS steps a-over-b,b-over-a,plane-alpha-50,pixel-alpha-premultiplied` |
| 2026-11-27 19:42:52–19:43:53 | [move.log](logs/move.log) | `VERDICT move PASS 3600 frames, bottom buffer 1 unchanged on plane 31, top on its own plane` |
| 2026-11-27 19:44:22–19:45:12 | [rotate-b3.log](logs/rotate-b3.log) | `VERDICT rotate PASS 3000 frames, 3 buffers per plane, 5998 releases in order, 0 dropped, 0 missed vblanks` |
| 2026-11-27 19:45:16 | [rotate-b2.log](logs/rotate-b2.log) | none: `drm-harness: --buffers: must be between 3 and 16`, exit status 2 — the 2-buffer run of the plan is not possible with this harness |
| 2026-11-27 19:45:25–19:46:31 | [all.log](logs/all.log) | `VERDICT combos PASS ...`, `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)`, `VERDICT zorder-alpha PASS steps a-over-b,b-over-a,plane-alpha-50,pixel-alpha-premultiplied`, `VERDICT move PASS 240 frames, bottom buffer 1 unchanged on plane 31, top on its own plane`, `VERDICT rotate PASS 600 frames, 3 buffers per plane, 1198 releases in order, 0 dropped, 0 missed vblanks`, `SUMMARY passed=5 failed=0` |

Each log starts with `# start_utc=... uname_r=6.6.157-7.8.0` and `# command=...` and ends with `# end_utc=... exit_status=N`, written by a wrapper on the board around the harness's own output; the harness lines are unedited.

**Operator error (19:39:51 UTC).** I started `zorder-alpha` while the 90 s hold of `two-planes-cam` was still running. The second process could not become DRM master and stopped at once with `EBUSY`, committed nothing, and the running hold was not disturbed. The scenario was rerun after the hold ended. The log is kept as evidence that a second master is refused cleanly.

## Plane assignments from the commits (observed 2026-11-27)

Copied from the `assign` records; every commit was preceded by `Engine::validate` (atomic `TEST_ONLY`), whose time is the `validate_us` of the same `commit` record.

| Scenario / step | Layer → buffer → plane (kind, zpos) | Other properties |
| --- | --- | --- |
| two-planes | 0 → buf 1 → **31** (primary, 0), full screen blue; 1 → buf 2 → **51** (overlay, 1), 640x400 red at 320,200 | both opaque XR24, alpha 0xffff |
| zorder-alpha a-over-b | red buf 1 → **51** (overlay, 1); green buf 2 → **31** (primary, 0) | opaque |
| zorder-alpha b-over-a | red buf 1 → **31** (primary, 0); green buf 2 → **51** (overlay, 1) | opaque |
| zorder-alpha plane-alpha-50 | red buf 1 → **51** (overlay, 1) alpha 0x8000; green buf 2 → **31** (primary, 0) | plane alpha |
| zorder-alpha pixel-alpha-premultiplied | red/half-alpha buf 3 (AR24) → **51** (overlay, 1) blend Premultiplied; green buf 2 → **31** (primary, 0) | pixel alpha |
| move (3600 frames) | bottom buf 1 → **31** (primary, 0) in all 3600 `frame` records; top buf 2 → **51** (overlay, 1) in all 3600 | only the top's destination changes (`top_x` 0 → 960) |
| rotate | layer 0 → **31** (primary); layer 1 → **51** (overlay); `assignment_changes=0` | 3 buffers per plane |

The z-order swap is done by the planner as a swap of the **plane assignment** (the lower layer always goes to primary 31 at zpos 0, the upper to overlay 51 at zpos 1), not by reprogramming `zpos` on fixed planes. The test plan allows this ("otherwise swap the buffers' plane assignment and say so"); the buffers themselves were not redrawn between the steps (same buffer ids 1 and 2).

## Measurements (observed 2026-11-27)

| Item | Value | Source |
| --- | --- | --- |
| Refresh | mode 59.993 Hz (period 16 668 µs); measured latch interval median 16 668 µs, min 16 388, p99 16 892, max 16 972 over 2998 intervals (`rotate`, 3000 frames) | `rotate_result` in rotate-b3.log |
| Frame loss | `rotate` 3000 frames: presented 3000, dropped 0, missed vblanks 0, latch timeouts 0, order violations 0, releases 5998 (= 2 × 3000 − 2 still on screen). `all`/rotate 600 frames: same, 1198 releases. `move` 3600 frames: 3600 `frame` records, interval max 16 943 µs | rotate-b3.log, all.log, move.log |
| Latency, submit → latched | `rotate`: median 11 168 µs, p99 14 490, max 15 020, min 9 624 (2999 frames, first frame excluded). `move`: median 16 478 µs, p99 16 633, max 16 765 (3599 frames). Includes the harness's wake-up time (no kernel vblank timestamp is read); `move` commits right after a latch, so its latency is about one period by construction | `rotate_result`, `move_result` |
| validate (TEST_ONLY) time | `rotate`: median 142 µs, p99 310, max 387; `combos`: 45–163 µs per case | rotate-b3.log, combos.log |
| commit call time | `rotate`: median 133 µs, p99 280, max 16 824 (the first, blocking modeset commit); first commits of the other scenarios 1.3–9.1 ms (modeset) | logs |
| CPU | `rotate` 3000 frames over 50.0 s: user 2.44 s + system 14.75 s = 34.4 % of one core, almost all of it the harness redrawing a 1280x800 and a 640x400 dumb buffer every frame (system time: writes to the dumb buffer mapping). `move` (no redraw): 4.7 %. `two-planes`/`zorder-alpha` holds: ≤ 0.2 %. Board load average 0.36 after the runs | `resources` records, `/proc/loadavg` |
| Memory | harness peak RSS (`VmHWM`): 1 236 KiB (`rotate`), 812–932 KiB otherwise. Buffers: `rotate` holds 3 × 4 096 000 B + 3 × 1 024 000 B = 15.4 MB of dumb buffers. CMA: `CmaFree` 1 070 488 kB before the session (Weston running), 1 098 836 kB after the runs (Weston stopped), of `CmaTotal` 1 114 112 kB; CMA was not sampled during a run | `resources` records, `/proc/meminfo` |
| Errors | no non-zero errno in any commit; the only errors are the operator's `EBUSY` above and the refused `--buffers 2`. Kernel log: `dmesg` before and after the session identical (empty diff) | logs, dmesg diff on the board |

`combos` (no commit): 20 cases, 16 accepted by the planner and by `TEST_ONLY` (1 to 6 layers on planes 31, 51, 59, 67, 75, 83; plane alpha; ARGB with the three blend modes; 2x upscale; NV12 and NV12 2x upscale; partly off-screen), 4 refused by the planner before `TEST_ONLY` (7 layers: `NoPlane ... OUTPUT | IN_USE`; 2x downscale: `SCALING`; 90° rotation: `TRANSFORM`; unknown format `HWLX`: `FORMAT`). No case was refused by `TEST_ONLY`, so the "forced past the planner" comparison of the test plan (§6.1) was not possible: the harness has no `--skip-planner`. The `off-screen` case is accepted with only the background layer assigned (the off-screen layer is dropped by the planner).

## Camera pictures and checks (observed 2026-11-27)

The camera sees the panel upside down and under strong perspective; the upper-left part of the panel (in panel coordinates) is outside the frame, and a cable crosses the lower-left corner. `scripts/camera-check.py` maps panel coordinates bilinearly from four corners, which does not fit this perspective (a first try put its regions up to 5 % of the panel off). So each picture was first rectified to panel pixels with [`rectify.py`](rectify.py): rotate by 180°, then a projective warp defined by the outer corners of the red layer of `two-planes` (panel rectangle 320,200–960,600, known from the commit), measured once on the first locked picture ([`quad.json`](quad.json)). The camera did not move and was locked for all pictures from 19:39 to 19:47 UTC. `camera-check.py` then runs on the rectified picture with the whole image as the panel and rotation 0. Expectation files: [`checks/`](checks/). All outputs: [`checks/results.txt`](checks/results.txt) (`ALL: PASS`); rerun with `python3 docs/m1-imx95/run_checks.py` (needs Pillow).

Colours: the expected colours are the hue classes as this camera renders them (red ≈ (240,80,0), green ≈ (90,220,0), blue ≈ (0,70,200), tolerance 70; red–green and red–blue distances are about 200–300). The panel shows a strong brightness fall-off to the right and a yellowish hot spot on the left half seen from the camera's angle, visible in every picture. The 50 % blend is expected at the mean of the two opaque overlap colours measured in states a and b, (185,169,0), tolerance 50 (test plan: 25 % of the range per channel).

| State | Pictures (raw / rectified) | What the check shows |
| --- | --- | --- |
| two-planes | [1](pictures/20261127T193910Z-two-planes-1-raw.jpg) / [rect](pictures/20261127T193910Z-two-planes-1-rect.jpg), [2](pictures/20261127T193916Z-two-planes-2-rect.jpg), [3](pictures/20261127T193920Z-two-planes-3-rect.jpg) | both planes at once: blue primary (0,81,216) around, red overlay (252,88,0) on top in the middle with no blue in it (B ≤ 1); 3/3 PASS |
| a-over-b | [1](pictures/20261127T194022Z-a-over-b-1-raw.jpg) / [rect](pictures/20261127T194022Z-a-over-b-1-rect.jpg), [2](pictures/20261127T194027Z-a-over-b-2-rect.jpg), [3](pictures/20261127T194032Z-a-over-b-3-rect.jpg) | overlap red (253,93,1) = top plane 51; 3/3 PASS |
| b-over-a | [1](pictures/20261127T194102Z-b-over-a-1-raw.jpg) / [rect](pictures/20261127T194102Z-b-over-a-1-rect.jpg), [2](pictures/20261127T194106Z-b-over-a-2-rect.jpg), [3](pictures/20261127T194112Z-b-over-a-3-rect.jpg) | overlap green (118,245,0) after the swap; 3/3 PASS |
| plane-alpha-50 | [1](pictures/20261127T194141Z-plane-alpha-50-1-raw.jpg) / [rect](pictures/20261127T194141Z-plane-alpha-50-1-rect.jpg), [2](pictures/20261127T194147Z-plane-alpha-50-2-rect.jpg), [3](pictures/20261127T194153Z-plane-alpha-50-3-rect.jpg) | overlap (201,169,1), 16 from the expected blend, ~100 from either opaque colour; red over black at half brightness (124,27,3) against (228,56,1) opaque; 3/3 PASS |
| pixel-alpha-premultiplied | [1](pictures/20261127T194225Z-pixel-alpha-1-raw.jpg) / [rect](pictures/20261127T194225Z-pixel-alpha-1-rect.jpg), [2](pictures/20261127T194230Z-pixel-alpha-2-rect.jpg), [3](pictures/20261127T194236Z-pixel-alpha-3-rect.jpg) | left half opaque red (254,99,1), right half at 50 % over black (123,26,3) and over green (201,169,1), same as plane alpha 50 %; 3/3 PASS |
| move | [1](pictures/20261127T194303Z-move-1-rect.jpg) (top at x ≈ 110), [2](pictures/20261127T194339Z-move-2-rect.jpg) (x ≈ 690), [3](pictures/20261127T194357Z-move-3-rect.jpg) (x ≈ 960); raw: [1](pictures/20261127T194303Z-move-1-raw.jpg), [2](pictures/20261127T194339Z-move-2-raw.jpg), [3](pictures/20261127T194357Z-move-3-raw.jpg) | the checkerboard on primary 31 is identical between pictures (distances 0.3–1.3 against threshold 30), the yellow overlay is at three positions (changed regions 204–254); 2/2 comparisons PASS |
| rotate | [raw](pictures/20261127T194433Z-rotate-b3-1-raw.jpg) / [rect](pictures/20261127T194433Z-rotate-b3-1-rect.jpg) | judged by eye only: each plane shows its own moving bar (blurred by the 60 ms exposure), no pattern of one layer in the other, no tear line; no `camera-check` expectation |
| after exit | [raw](pictures/20261127T194641Z-after-exit-1-raw.jpg) / [rect](pictures/20261127T194641Z-after-exit-1-rect.jpg) | panel black once the harness (DRM master) has exited |

**Anti-flattening evidence** (test plan §1): (1) every commit puts the two layers on two distinct planes from the probe's list, 31 (primary) and 51 (overlay), with separately allocated buffers (distinct buffer ids; dumb buffers allocated one by one); (2) in `move`, primary 31 kept buffer 1 for all 3600 commits, buffer 1 was never released, and its checksum was `ad2ffa4dee576f25` before and after, while only the overlay's destination changed; the pictures show the overlay sliding over an unchanged checkerboard; (3) `combos` shows planner refusals that match the probe's plane list (7 layers on 6 planes). A refusal by `TEST_ONLY` itself was not obtained (see `combos` above).

**Control run** (`--flatten`): not run; the harness has no flatten mode.

**Expectations changed after a failure** (kept, not hidden): two yellow expectations in the move checks were guesses written before measuring and failed: (250,245,120) at the panel's left, where the camera saturates the yellow layer to near white (254,255,190) ([output](checks/move-1-2-first-attempt.txt)), and (230,230,0) at the dim right edge, where it measured (180,171,0) ([output](checks/move-2-3-first-attempt.txt)). Both were reset to the camera's yellow at those places. The comparisons that carry the move evidence (changed / same) passed unchanged in both attempts.

**Pictures not used for the verdicts**: the three `two-planes` pictures taken before the camera was locked (`*-unlocked-two-planes-*-raw.jpg`, framing shifted between them under autofocus) and the picture of Weston before the session, which carries the camera tool's own detection outline.

## Against the test plan (observed 2026-11-27)

- **Condition 3** (§5): log criteria met (two distinct planes of the probe's list, both on CRTC 39, distinct buffers, `TEST_ONLY` before every commit), camera confirms both planes, both stacking orders, plane alpha and pixel alpha, and the move test passes with an unchanged checksum. Z-order is swapped by plane assignment, not by `zpos` on fixed planes. The flatten control was not run.
- **Condition 4** (§6): 16 accepted, 4 refused with named reasons, no crash. Planner and kernel agreed on every case that reached `TEST_ONLY`; there is no forced-past-planner case, so no planner/kernel disagreement could be found.
- **Condition 5** (§7): zero order violations over 3000 + 600 frames with 3 buffers per plane, `latched + dropped = submitted`, intervals within ±20 % of the period (max deviation +1.8 %). Not done: the 2-buffer run (harness minimum is 3), a readback of every buffer's frame number at the end (the harness checks ordering, not the content), CMA during the run.
- **Condition 7** items measured here: board, panel, image, kernel, resolution and refresh, plane assignments, errors, frame loss, latency, CPU, memory (above).

## Files

- `logs/`: harness output of every run, unedited apart from the wrapper's `#` lines.
- `pictures/`: every picture taken, as `<UTC time saved>-<state>-<n>-raw.jpg` (camera frame downscaled 4x, as the camera sees it: upside down) and `-rect.jpg` (rectified to 1280x800 panel pixels from the full-size frame).
- `checks/`: `camera-check.py` expectations, the two failed first attempts, `results.txt`.
- `rectify.py`, `quad.json`, `run_checks.py`: the picture pipeline above.
