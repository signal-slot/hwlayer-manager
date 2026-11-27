# Linux board scenarios: A1-A5 on boards other than the i.MX95

Written 2026-11-28. It holds the method and the results of the first runs. The NanoPC-T6 was run on 2026-11-27 and is filled in. The Raspberry Pi 5 rows are still "not measured" on this page: see the reason in that section.

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

Pictures are taken through the script's `--hook` at `hold-start`/`hold-end` of A2 and A3 only (A1, A4 and A5 have no hook call, so a picture of A1 has to be taken by hand during its `--seconds` hold). The expected regions come from the plane layout and colours printed by the harness's `two-planes` run. A picture verdict is recorded next to the log verdict of its row; the two are never merged, and a row whose logs fail is FAIL whatever the picture shows. A picture that cannot be judged (no capture, panel not found) is recorded as such, not as PASS.

## Raspberry Pi 5 (BCM2712, vc4)

Board facts (kernel, driver, connector, mode): not measured on this page. Compositor state (observed 2026-11-27): no display manager was running before or after the run, so none was stopped or restored.

| Row | Log result | Counts (commits / latched / released) | KVM picture verdict | Date and kernel |
| --- | --- | --- | --- | --- |
| A1 startup | not measured | not measured | not measured (no hook; by hand) | not measured |
| A2 steady state | not measured | not measured | not measured | not measured |
| A3 stop/restart | not measured | not measured | not measured | not measured |
| A4 lease loss | not measured | not measured | not applicable (no hook call) | not measured |
| A5 modeset | not measured | not measured | not applicable | not measured |

Notes: an A1-A5 run on this board was made on 2026-11-27 with the harness built at commit 195b458, and its report names five bugs, H1-H3 (harness) and S1, S2 (script). The text of that report (SCENARIO lines, counts, camera-check verdicts) was not available when this page was written, so every cell stays "not measured" and those bugs are not described below. A later change fills this section from the report. The bug labels are the run report's own.

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
- **Camera-check input.** Expected colours come from the harness's colour constants, with the full frame as panel corners, because the KVM captures HDMI directly. The `--rotation` value used is not in the run report, so it is not confirmed here.
- **Board state.** No kernel log lines appeared during the run (dmesg had 1279 lines before and after). No DRM, udev or seat change was made, and the board's temporary files were removed.
- **Released counts.** "Not counted" means the script's A1 and A3 checks compare commits with latched only; it counts released in A2.

## Harness bugs (listed, not fixed)

Observed on the NanoPC-T6 on 2026-11-27, kernel 6.18.55-current-rockchip64, harness and script at commit 195b458. They are bugs of `tools/drm-harness` or `scripts/board-scenarios.sh`, not board faults, and are for the owners of that code. This page does not fix them.

1. **A5 cannot find the connector's modes.** The harness prints the connector as `HDMIA-1`, but sysfs names it `card0-HDMI-A-1`, so `row_a5` in `scripts/board-scenarios.sh` never finds the modes file and gives a wrong BLOCKED reason. Reproduce: run `drm-harness --source master --seconds 1 two-planes`, take the line with `selected=yes`, and compare with `ls /sys/class/drm/`.
2. **The A3 hook does not cover the screen after the stop.** Its two calls are about 0.1 s apart and the restart begins at once, so no picture can show the screen after the signal.
3. **No picture hook in A1, A4 or A5.** Pictures of A1 had to be taken by hand.
4. **The harness has no SIGTERM handler.** It exits 143 without writing `resources` or `VERDICT`, so A3 does not test a clean shutdown.
5. **`revoke` ignores the `--seconds` the script passes.** The run took 1063 ms with `--seconds 15`, so the screen after the revoke is never held.

Raspberry Pi 5 run (2026-11-27): that run reported five bugs, H1-H3 and S1, S2, whose text was not available when this page was written. They are not listed here, and some may duplicate the items above.

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
