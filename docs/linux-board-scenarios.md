# Linux board scenarios: A1-A5 on boards other than the i.MX95

Skeleton written 2026-11-28. It holds the method and empty result tables. **No hardware result is recorded here yet**: every result cell says "not measured", and a later change fills them in.

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

This page itself is a skeleton: the method below is read from source (`scripts/board-scenarios.sh`, `scripts/board-scenarios.test.sh`, `scripts/camera-check-README.md`) and the tables are empty.

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

Board facts (kernel, driver, connector, mode, compositor state): not measured.

| Row | Log result | Counts (commits / latched / released) | KVM picture verdict | Date and kernel |
| --- | --- | --- | --- | --- |
| A1 startup | not measured | not measured | not measured (no hook; by hand) | not measured |
| A2 steady state | not measured | not measured | not measured | not measured |
| A3 stop/restart | not measured | not measured | not measured | not measured |
| A4 lease loss | not measured | not measured | not applicable (no hook call) | not measured |
| A5 modeset | not measured | not measured | not applicable | not measured |

Notes: none yet.

## NanoPC-T6 (RK3588)

Board facts (kernel, driver, connector, mode, compositor state): not measured.

| Row | Log result | Counts (commits / latched / released) | KVM picture verdict | Date and kernel |
| --- | --- | --- | --- | --- |
| A1 startup | not measured | not measured | not measured (no hook; by hand) | not measured |
| A2 steady state | not measured | not measured | not measured | not measured |
| A3 stop/restart | not measured | not measured | not measured | not measured |
| A4 lease loss | not measured | not measured | not applicable (no hook call) | not measured |
| A5 modeset | not measured | not measured | not applicable | not measured |

Notes: none yet.

## Harness bugs (listed, not fixed)

None listed yet. Defects found in `tools/drm-harness` or `scripts/board-scenarios.sh` during the runs are listed here with the date and kernel they were observed on, and are reported to the owner of the code. They are not fixed from this page.

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
