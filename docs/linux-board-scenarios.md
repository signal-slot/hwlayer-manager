# Linux board scenarios: A1-A5 on boards other than the i.MX95

Written 2026-11-28. It holds the method and the results of the first runs. The NanoPC-T6 was run on 2026-11-27 (script and harness at 195b458) and the Raspberry Pi 5 on 2026-11-28 (at a49d6ae); both are filled in. The script changed between the two runs, so the method section below, written for 195b458, does not describe every option and row the Raspberry Pi 5 run used (A5 now runs the harness's `modeset` scenario, A3 and A4 have more hook phases).

## Purpose and scope

[`board-scenarios.md`](board-scenarios.md) designs the SoW §10 hardware integration scenarios A1-A6 for the i.MX95 Linux path. This page records the first runs of rows A1-A5 on two other Linux DRM boards, using the repository's runner [`scripts/board-scenarios.sh`](../scripts/board-scenarios.sh) over the `tools/drm-harness` binary:

- Raspberry Pi 5 (BCM2712, `vc4` DRM driver);
- FriendlyElec NanoPC-T6 (RK3588).

Both run with no compositor: the display manager, if any, is stopped for the run and restored afterwards by whoever runs it (the script never does either). Only the `master` and `kernel-lease` sources are used.

## Portability evidence only

These runs are portability evidence. **They never count toward M1, M2 or M3**: M1 is the i.MX95 Linux run (plan #453), M2 the i.MX95 Zephyr comparison (plan #455), and M3 the AM62P run (issue #12). A PASS here says that the harness and the DRM backend behave on another driver; it does not satisfy any acceptance condition of those milestones, and it is not to be cited as such.

Tags used on this page, as in [`platform-capabilities.md`](platform-capabilities.md):

- **observed**: read from a real board run, and written with its date (as in 2026-12-01) and the kernel. A tag without a date is a mistake.
- **read from source**: taken from a file of this repository (the file is named).
- **inferred**: a conclusion drawn from the above, not stated by it.
- **not measured**: nothing was run; the cell is a placeholder.

The method below is read from source (`scripts/board-scenarios.sh`, `scripts/board-scenarios.test.sh`, `scripts/camera-check-README.md`). The result tables are observed, with the date and kernel in each row.

## How the runs are done

### Command line (read from source: `scripts/board-scenarios.sh`)

```
scripts/board-scenarios.sh [options] HARNESS
```

`HARNESS` is the path of the `drm-harness` binary built for the board. Options:

| Option | Meaning |
| --- | --- |
| `--out DIR` | result directory, default `/tmp/board-scenarios`; the only place the script writes to |
| `--rows LIST` | comma-separated rows, default `A1,A2,A3,A4,A5`; unselected rows get `NOT-RUN` |
| `--hold SECS` | A2 steady-state hold, default 10 seconds; with `--fps` (default 60, assumed, not read from the sink) it gives `rotate --frames HOLD*FPS --buffers 3` |
| `--frames N` | A2 frame count; overrides `--hold`/`--fps` |
| `--seconds N` | how long each picture is held in A1, A3 and A5, default 3 |
| `--allow-drops N` | dropped frames A2 may report, default 0 |
| `--stop-wait N` | seconds to wait for a signalled run to exit, default 10 |
| `--device PATH`, `--connector NAME`, `--profile NAME` | passed to the harness unchanged |
| `--hook CMD` | called as `CMD ROW PHASE` (`hold-start` or `hold-end`) around each hold, so the caller can take a KVM or camera picture; a failing hook is only noted |

Exit status: 0 every selected row PASS, 1 a row FAILed, 2 usage error, 3 no FAIL but a selected row is BLOCKED or NOT-RUN. One `ROW name RESULT` line per row goes to stdout.

### Result files

Each row writes `OUT/ROW/result.txt`, whose first line is `SCENARIO <name> PASS|FAIL|BLOCKED|NOT-RUN`, followed by the evidence lines and `failed:` lines for each broken check. The raw harness records are kept beside it as `OUT/ROW/*.log`. A picture alone never passes a row: the log checks below decide, and the picture is additional evidence.

### What each row checks (read from source: `scripts/board-scenarios.sh`)

| Row | Name | Harness run | Pass rule |
| --- | --- | --- | --- |
| A1 | startup | `two-planes --source master`, then `--source kernel-lease` | exit 0 and `VERDICT two-planes PASS`; at least one `commit` record; every `commit` record carries `latency_us` (counted as *latched*, so latched must equal commits); for kernel-lease, a `lease` record |
| A2 | steady state | `rotate --source master --frames HOLD*FPS --buffers 3` | `VERDICT rotate PASS`; `presented` equals the frame count; `dropped` at most `--allow-drops`; `order_violations` 0; no `violation` record; `releases` greater than 0 and equal to `2*(presented-1) + 2*dropped` (the *released* count from the harness's own `rotate_result`) |
| A3 | stop/restart | `two-planes --source master --seconds 600`, signalled once the first `commit` is seen: SIGTERM, then separately SIGKILL | each run exits within `--stop-wait` (SIGTERM: status 0 or 143; SIGKILL: 137); after each, A1's master check passes again (exit, VERDICT, commits equal latched) |
| A4 | lease loss | `revoke --source kernel-lease --frames 60 --buffers 3` | `VERDICT revoke PASS`; `revoke_result first_attempt` is a `DeviceLost`; `device_lost_events=1`; `still_busy` is `-` (no buffer busy); `outputs_after=0` and `planes_after=0`; no `commit`, `frame` or `step_result` record after `before_revoke` |
| A5 | modeset | `two-planes --source master --seconds 1` to find the selected connector; modes read from `${DRM_SYSFS:-/sys/class/drm}/card*-CONNECTOR/modes` | never PASS: BLOCKED when the sink offers one mode, or no modes file is readable; BLOCKED with "harness has no modeset scenario" when it offers more, because the harness has no mode option |

The commit/latched/released counts are taken from the harness's current output, not from the trace format of `crates/composition-core`, which is not fixed (X-03 in [`open-issues.md`](open-issues.md)). The A2 release formula and the `FPS` of 60 are the script's assumptions.

Behaviour of the script itself is tested on the host by `scripts/board-scenarios.test.sh` with a mocked harness (read from source); that test says nothing about a board.

### Board rules for these runs

- Stop any display manager or compositor for the run and restore it afterwards; say in the results whether it was restored. The script does not do this.
- Change no DRM, udev or seat configuration; use only `--source master` and `--source kernel-lease`.
- Write only under a temporary directory on the board and remove it afterwards.
- No board address, host name, IP, serial number or credential goes into this file or into a result directory that is committed. Run `scripts/check-no-board-secrets.sh` before committing.

## How KVM pictures are judged

Read from source: `scripts/camera-check-README.md`. The KVM capture (or camera) picture of the panel taken during a hold is judged by `scripts/camera-check.py`:

```
python3 scripts/camera-check.py PICTURE EXPECT.json [--compare PICTURE2] [--rotation N] [--json]
```

Exit 0 is `VERDICT: PASS`, 1 `VERDICT: FAIL`, 2 a usage or input error. The picture is rotated by `rotation` (default 180 for the board camera; a KVM capture is normally upright, so pass `--rotation 0` or set it in the file), the panel is found (`auto`, or explicit corners, which are needed for a black panel background), and each region in `EXPECT.json` is measured against an expected colour within a tolerance. With `--compare` the same regions are measured in two pictures and asserted `changed` or `same`.

Pictures are taken through the script's `--hook` (read from source: `scripts/board-scenarios.sh` at 99f8748, its usage text). The hook is called as `CMD ROW PHASE [LABEL]`, and `hold-start` comes once the first commit of the run is seen, so the hook may sleep and then take its picture. A1, A2, A4 and A5 have `hold-start`/`hold-end`; A3 has `hold-start`/`hold-end` before the signal, `post-stop-start`/`post-stop-end` while the screen is held after the process has exited (`--pause`, default `--seconds`), and `restart-start`/`restart-end` for the restarted run. The NanoPC-T6 run was made with the script at 195b458, which had hooks in A2 and A3 only (A3 with its two calls 0.1 s apart), no hook in A1, A4 and A5, and no `post-stop` phase; its table below stays true for that script. The expected regions come from the plane layout and colours printed by the harness's `two-planes` run. A picture verdict is recorded next to the log verdict of its row; the two are never merged, and a row whose logs fail is FAIL whatever the picture shows. A picture that cannot be judged (no capture, panel not found) is recorded as such, not as PASS.

## Raspberry Pi 5 (BCM2712, vc4)

Board facts (observed 2026-11-28, kernel 6.18.50+rpt-rpi-2712): model Raspberry Pi 5 Model B Rev 1.1, Raspberry Pi OS Lite (Debian 13); DRM card1 is the KMS device, driver `vc4` (`vc4-drm`); card0 is `v3d` (render only, no connectors), so every run passed `--device /dev/dri/card1`. Connector HDMI-A-1 connected at 1920x1080, 60 Hz (148500 kHz), CRTC 94; HDMI-A-2 disconnected; the sink offers 20 modes (the harness's `connector_modes count=20`, the same 20 lines in sysfs). The two planes of every two-planes, rotate, revoke and modeset run were 83 (primary) and 107 (overlay). Compositor state (observed 2026-11-28): display-manager, gdm, lightdm, sddm and greetd all inactive before and after, and no process held the DRM device, so nothing was stopped or restored; the screen shows the tty1 text console when no harness runs. The harness ran as an ordinary user in the `video` group (no sudo).

Run (observed 2026-11-28): harness built at commit a49d6ae (`scripts/cross-aarch64.sh -p composition-drm-harness --bin drm-harness`, static aarch64 musl), the same commit as `scripts/board-scenarios.sh`. Command: `board-scenarios.sh --rows A1,A2,A3,A4,A5 --hold 30 --seconds 25 --device /dev/dri/card1 --hook <marker hook>`, where the hook only logs the phase and, at each `*-start` phase, sleeps 15 s so that the KVM picture can be taken by hand during the hold. Three identical runs: 08:23-08:28, 08:28-08:33 and 08:33:38-08:38:23 UTC. All three gave the same rows and the same counts. The pictures come from the third run; in the first two, no pictures were taken because of an operator timing mistake, not a script fault. Script exit status 0 each time (A6 not selected, `NOT-RUN`). A fourth run with the same options but no hook (09:20:03-09:24:46 UTC, no pictures) gave the same rows, counts and VERDICT lines, with exit status 0 and dmesg still at 589 lines.

| Row | Log result | Hold | Counts (commits / latched / released) | KVM picture verdict (camera-check) | Date and kernel |
| --- | --- | --- | --- | --- | --- |
| A1 startup | PASS | 25 s per source (`--seconds 25`) | master and kernel-lease each: 1 / 1 / not counted; the kernel-lease run has its `lease` record (17 planes leased: primary 83 and overlays 107-272; see bug 6); `VERDICT two-planes PASS 2 buffers on planes 83,107 (bottom,top)` twice | PASS (08:33:55, master hold; 08:34:23, kernel-lease hold). The picture at 08:34:19 FAIL: stale, still shows the text console between the two runs | observed 2026-11-28, 6.18.50+rpt-rpi-2712 |
| A2 steady state | PASS | 30 s (1800 frames at the assumed 60 fps, 3 buffers) | presented 1800, dropped 0, order_violations 0; released 3598 (= 2*(1800-1)); `VERDICT rotate PASS 1800 frames, 3 buffers per plane, 3598 releases in order, 0 dropped, 0 missed vblanks` | PASS (08:35:15 and 08:35:18, `rotate` expectations); moving bar compared 08:35:15 with 08:35:18: PASS, changed by 310.1 / 310.3 | observed 2026-11-28, 6.18.50+rpt-rpi-2712 |
| A3 stop/restart | PASS | 25 s before the signal, 25 s after the exit, 25 s per restart | SIGTERM run: exit 0 after 1 commit, its own line `VERDICT two-planes PASS ...; stopped by SIGTERM or SIGINT during the hold, buffers released`; SIGKILL run: exit 137 after 1 commit; each restart 1 / 1 / not counted with `VERDICT two-planes PASS 2 buffers on planes 83,107 (bottom,top)` | Held pattern PASS (08:35:40 TERM hold; 08:36:48 and 08:37:00 KILL hold); after the stop the text console is back: stop pictures compared with the held pattern PASS, changed by 95.3 / 180.7 (TERM 08:36:02; KILL 08:37:05 and 08:37:20); restart PASS (08:36:05 after TERM; 08:37:41 after KILL) | observed 2026-11-28, 6.18.50+rpt-rpi-2712 |
| A4 lease loss | PASS | 26 s run (`wall_ms=26005`): 60 frames, then the revoke, then the 25 s `hold step=revoke` | 60 frames before the revoke; no commit-like record after `before_revoke`; `first_attempt` is `Err(Backend(DeviceLost))` after 1278 us; `device_lost_events=1`; `still_busy=-`; `outputs_after=0`; `planes_after=0`; buffers 3 and 6 released; `VERDICT revoke PASS 60 frames on planes 83,107, then revoked: DeviceLost once, buffers 3,6 released` | PASS (08:38:00): the last frame before the revoke stays on screen (bars frozen; `revoke` expectations) | observed 2026-11-28, 6.18.50+rpt-rpi-2712 |
| A5 modeset | PASS | 1 s on the original mode, 25 s on the second mode | before the switch 61 / 61; after the switch 1251 / 1251; released not counted; switch commit 55110 us (third run); `VERDICT modeset PASS 1920x1080@60 -> 1920x1080@50: 61 commits latched before, 1251 after the switch; 1920x1080@60 restored` | PASS for the pattern (08:38:16, `two-planes` expectations). The picture cannot show the switch itself, because the second mode has the same size (bug 7) | observed 2026-11-28, 6.18.50+rpt-rpi-2712 |

Notes:

- **The A5 row passes here.** At a49d6ae the harness has a `modeset` scenario. It chose the first other mode that passed TEST_ONLY, `1920x1080@50` (mode index 3), and restored `1920x1080@60` afterwards. On the NanoPC-T6, A5 was BLOCKED at 195b458, before that scenario existed.
- **Picture timing.** The KVM picture runs about 20-25 s behind the HDMI output (inferred from the console and pattern changes against the hook's marks), so each picture was credited to a run by what it shows and when. The judged pictures and their times (UTC) are the ones in the table. Before the run (08:22:32) and after it (08:38:48) the text console showed, and both pictures FAIL the `two-planes` expectations, as expected.
- **Camera-check input.** EXPECT files from `scripts/scenario-expect.py --scenario two-planes|rotate|revoke|stop` (rotation 0, the full frame as the panel). Measured colours: blue (20, 40, 126), red (196, 7, 29), distances 21.6 and 32.2 within tolerance 40. The A2 bar comparison used a hand-made EXPECT with two thin regions at the two bar positions (x 0.407-0.418 and 0.866-0.877, y 0.03-0.2), each asserted `changed`. The A4 and A5 picture expectations are those of the pattern left on screen (`revoke`, `two-planes`). `scenario-expect.py` has no `modeset` colours.
- **Board state.** dmesg had 589 lines before and after the three runs. The last line, a `vc4-drm` "Failed to allocate UPM entry: -28", is from about 50 minutes before the first run and was not repeated. No DRM, udev or seat change was made. Only `--source master` and `--source kernel-lease` were used. The board's temporary directory was removed afterwards.
- **Earlier run.** An A1-A5 run on this board on 2026-11-27 (harness at 195b458) named bugs H1-H3 and S1, S2. Its report was lost, so those bugs could not be checked again here. This run's own findings are bugs 6-9 below.
- **Released counts.** "Not counted" means the script's A1, A3 and A5 checks compare commits with latched only; it counts released in A2 (and A4 names the released buffers).

## NanoPC-T6 (RK3588)

Board facts (observed 2026-11-27, kernel 6.18.55-current-rockchip64): model FriendlyElec NanoPC-T6; DRM card0 is `rockchip-drm`; connector HDMI-A-1 connected at 1920x1080, 60 Hz; no display manager (display-manager, gdm, lightdm and sddm all inactive before and after), so nothing was stopped or restored; no process held the DRM device. Harness built at commit 195b458 (`scripts/cross-aarch64.sh -p composition-drm-harness --bin drm-harness`), the commit of the script run. Command: `board-scenarios.sh --hold 30 --seconds 15 --hook <timestamp logger>`; run 20:21-20:36 UTC. The two planes of the runs were 57 and 63.

Plane facts (observed 2026-11-27, same kernel): 8 planes, each with zpos 0..7 and none able to scale; Cluster planes take AFBC, Esmart planes take linear and NV12-class formats; planes 39 and 69 cannot reach video_port0; the kernel lease holds primary 33 and overlays 45, 51, 57, 63, 69, 75.

| Row | Log result | Hold | Counts (commits / latched / released) | KVM picture verdict (camera-check) | Date and kernel |
| --- | --- | --- | --- | --- | --- |
| A1 startup | PASS | 15 s per source (`--seconds 15`) | master and kernel-lease each: 1 / 1 / not counted; the kernel-lease run has its `lease` record; `VERDICT two-planes PASS 2 buffers on planes 57,63 (bottom,top)` | PASS (picture 20:23:03). Two earlier pictures (20:22:41, 20:22:48) FAIL: stale, still showing the console. No hook; taken by hand | observed 2026-11-27, 6.18.55-current-rockchip64 |
| A2 steady state | PASS | 30 s (1800 frames at the assumed 60 fps, 3 buffers) | presented 1800, dropped 0, order_violations 0; released 3598 (= 2*(1800-1)); `VERDICT rotate PASS 1800 frames, 3 buffers per plane, 3598 releases in order, 0 dropped, 0 missed vblanks` | PASS (4 pictures, 20:23:23 to 20:23:37); moving bars compared 20:23:33 with 20:23:37: PASS, changed by 102.7 / 103.0 | observed 2026-11-27, 6.18.55-current-rockchip64 |
| A3 stop/restart | PASS | until the signal, sent after the first commit (a 600 s run is requested) | SIGTERM run: exit 143 after 1 commit; SIGKILL run: exit 137 after 1 commit; each restart 1 / 1 / not counted | PASS for 4 pictures (20:23:40, 20:23:44 stale, 20:23:51 restart after SIGTERM, 20:24:09 restart after SIGKILL). None shows the screen just after a signal (bug 2) | observed 2026-11-27, 6.18.55-current-rockchip64 |
| A4 lease loss | PASS | about 1 s (1063 ms, bug 5) | no commit-like record after `before_revoke`; `first_attempt` is `Err(Backend(DeviceLost))`; `device_lost_events=1`; `still_busy=-`; `outputs_after=0`; `planes_after=0`; buffers 3 and 6 released | not applicable (no hook call; too short to picture) | observed 2026-11-27, 6.18.55-current-rockchip64 |
| A5 modeset | BLOCKED | not run | none | not applicable | observed 2026-11-27, 6.18.55-current-rockchip64 |

Notes:

- **The A5 reason is wrong.** The script printed "BLOCKED: no readable modes file for connector HDMIA-1", but the sink offers 7 mode names (observed 2026-11-27). This is harness bug 1. With it fixed, A5 would still be BLOCKED, because the harness has no modeset scenario.
- **Picture timing.** The KVM picture runs about 10-20 s behind the HDMI output, so each picture was credited to a run by what it shows, not by when it was taken. Take a picture at least 20 s into a hold. Rows A3 and A4 are too short to picture until the script holds the screen longer or has more hooks.
- **Before and after the run** the text console showed. Pictures at 20:21:52 and 20:34:33 FAIL camera-check, as expected.
- **Camera-check input.** Expected colours come from the harness's colour constants, with the full frame as panel corners, because the KVM captures HDMI directly. The `--rotation` value used was not recorded in the run report; the camera-check README says a KVM capture is upright (rotation 0), and this page claims no more.
- **Board state.** No kernel log lines appeared during the run (dmesg had 1279 lines before and after). No DRM, udev or seat change was made, and the board's temporary files were removed.
- **Released counts.** "Not counted" means the script's A1 and A3 checks compare commits with latched only; it counts released in A2.

## Harness bugs (listed, not fixed)

Observed on the NanoPC-T6 on 2026-11-27, kernel 6.18.55-current-rockchip64, harness and script at commit 195b458. They are bugs of `tools/drm-harness` or `scripts/board-scenarios.sh`, not board faults, and are for the owners of that code. This page does not fix them.

1. **A5 cannot find the connector's modes.** The harness prints the connector as `HDMIA-1`, but sysfs names it `card0-HDMI-A-1`, so `row_a5` in `scripts/board-scenarios.sh` never finds the modes file and gives a wrong BLOCKED reason. Reproduce: run `drm-harness --source master --seconds 1 two-planes`, take the line with `selected=yes`, and compare with `ls /sys/class/drm/`.
2. **The A3 hook does not cover the screen after the stop.** Its two calls are about 0.1 s apart and the restart begins at once, so no picture can show the screen after the signal.
3. **No picture hook in A1, A4 or A5.** Pictures of A1 had to be taken by hand.

   Bugs 1-3 were fixed in the script afterwards, by 9a780bc (sysfs connector names, picture hooks in A1/A3/A4/A5) and 99f8748 (`--pause` for the A3 hold after the signal). They are kept here as observed at 195b458.
4. **The harness has no SIGTERM handler.** It exits 143 without writing `resources` or `VERDICT`, so A3 does not test a clean shutdown.
5. **`revoke` ignores the `--seconds` the script passes.** The run took 1063 ms with `--seconds 15`, so the screen after the revoke is never held.

Observed on the Raspberry Pi 5 on 2026-11-28, kernel 6.18.50+rpt-rpi-2712, harness and script at commit a49d6ae. None of them failed a row; they are for the owners of that code, and this page does not fix them.

6. **The kernel lease holds a plane the engine never uses** (harness). The in-process lessor leases the primary and all 16 overlays of the CRTC (17 planes). The engine keeps `MAX_PLANES` = 16, so `lease_check` prints `leased_not_seen=272`, but the run still passes. Reproduce: `drm-harness --device /dev/dri/card1 --source kernel-lease --seconds 1 two-planes`, then read the `lease` and `lease_check` lines.
7. **The modeset scenario can choose a mode of the same size** (harness). The second mode is the first other mode that passes TEST_ONLY. On this sink that is `1920x1080@50` next to `1920x1080@60`, so neither a KVM capture nor a camera can tell the two modes apart. The A5 picture only shows that the pattern survives the switch.
8. **A3 does not check the stopped run's own result** (script). `stop_run` in `scripts/board-scenarios.sh` checks only the exit status (TERM: 0 or 143). At a49d6ae the harness handles SIGTERM: it exits 0 and writes `VERDICT two-planes PASS ...; stopped by SIGTERM or SIGINT during the hold, buffers released` and a `resources` record. The script never reads them, so a TERM run that exits 0 with a failing VERDICT would still pass A3.
9. **A1 records no evidence of the lease** (script). `row_a1` writes a `failed:` line when the kernel-lease log has no `lease` record, but nothing when it has one. So `A1/result.txt` does not show that a lease was made.

On the Raspberry Pi 5 at a49d6ae, NanoPC-T6 bugs 4 and 5 did not recur. The SIGTERM run exited 0 with the VERDICT above. The revoke run held its 25 s (`hold step=revoke seconds=25`, `wall_ms=26005`). Bugs 1-3 do not apply to the script at a49d6ae: A5 drives the harness's `modeset` scenario and reads the connector name from it, and A1, A3 (post-stop), A4 and A5 have hook calls. Bugs H1-H3 and S1, S2 of the lost 2026-11-27 Raspberry Pi 5 report stay undescribed: the report is lost, and this run cannot tell which of them, if any, are among items 6-9.

## Differences from docs/board-scenarios.md

- **A6 (display recovery) is not covered.** The runner has no row for it, and no result is claimed.
- **Boards.** `board-scenarios.md` Path A is the Verdin iMX95 with Weston and an LVDS panel; this page is the RPi5 and the NanoPC-T6. Paths B (Zephyr on the M7) and C (AM62P) are not touched.
- **Runner exists.** `board-scenarios.md` says no runner or result-file writer exists; here the script writes `result.txt` per row in the same `SCENARIO <name> PASS|FAIL|BLOCKED|NOT-RUN` form (read from source).
- **Bounds.** A2 uses a short stated hold (default 10 seconds, not the design's 10 minutes) and zero allowed drops; no memory or CPU bound is checked.
- **A3** signals a `two-planes` harness run; it does not use external producers or a dead-producer case.
- **A4** uses `--source kernel-lease` only; the `dlm` source is not run, and the one-second bound on reporting the loss is not checked.
- **A5** can only be BLOCKED: the harness has no mode-change scenario, so the page says whether the sink offers a second mode and never passes the row.
- **Camera.** The design judges with a camera on the i.MX95 panel; here pictures come from a KVM capture, and only for the rows with a hold hook.
- **Milestones.** The i.MX95 rows feed M1; these rows never feed M1, M2 or M3.
