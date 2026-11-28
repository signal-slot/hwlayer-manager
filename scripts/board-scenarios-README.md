# board-scenarios

`scripts/board-scenarios.sh` runs rows A1-A6 of
[`docs/board-scenarios.md`](../docs/board-scenarios.md) (SoW §10) on a Linux
board that has no compositor. It drives the `drm-harness` binary
(`tools/drm-harness`, built for the board) once or twice per row, judges the
harness's records and writes one result file per row. POSIX `sh`.

The runner has been used on other Linux DRM boards; see
[`docs/linux-board-scenarios.md`](../docs/linux-board-scenarios.md) for the
method and the dated results. Those runs are portability evidence and never
count toward M1, M2 or M3. This README records no result.

## Usage

```
scripts/board-scenarios.sh [options] HARNESS
```

`HARNESS` is the path of the drm-harness binary on the machine that runs the
script. It must be executable. The board and how to reach it come from the
lease of the board item; the script itself knows no address, user or host.
It runs where the board's DRM device is, so it is copied there and run there.

| Option | Meaning |
| --- | --- |
| `--out DIR` | Result directory. Default `/tmp/board-scenarios`. |
| `--rows LIST` | Comma-separated rows to run. Default `A1,A2,A3,A4,A5,A6`. Others get `NOT-RUN`. A row other than A1-A6 is a usage error. |
| `--hold SECS` | A2 steady-state hold. Default 10. A2 frames = `SECS * FPS`. |
| `--fps N` | Refresh rate assumed for A2. Default 60. It is assumed, not read from the sink. |
| `--frames N` | A2 frame count; overrides `--hold`/`--fps`. |
| `--seconds N` | How long each picture is held in A1, A3, A4, A5, A6. Default 3. A hold that a picture is taken 20 s or more into needs 25 or more. A5 runs 1 s unless this is given. |
| `--loss NAME` | A6: how the harness loses the display, `master` or `disable`. Default: the harness's (`master`). |
| `--allow-drops N` | Dropped frames A2 may report. Default 0. |
| `--stop-wait N` | Seconds to wait for a signalled run (and for a first commit) before giving up. Default 10. |
| `--trace` | Pass `--trace FILE` to every harness run, run the harness's `trace-check FILE` on it (strict parse) and compare its counts with the harness's own records. A mismatch or parse failure fails the row. The per-row comparisons are listed in the script header. |
| `--device PATH` | Passed to the harness unchanged. |
| `--connector NAME` | Passed to the harness unchanged. |
| `--profile NAME` | Passed to the harness unchanged. |
| `--hook CMD` | Command called as `CMD ROW PHASE [LABEL]` around each hold, so the caller can take a camera or KVM picture (see below). Its failure is only noted. |
| `--` | End of options. |

The numeric options must be numbers. An unknown option, row or loss name, a
missing or non-executable `HARNESS`, or a non-number is a usage error (exit 2).

## What it runs

| Row | Name | Harness run | PASS when |
| --- | --- | --- | --- |
| A1 | startup | `two-planes` with `--source master`, then `--source kernel-lease` | exit 0, `VERDICT PASS`, at least one `commit`, every commit has `latency_us`, a `lease` record for kernel-lease |
| A2 | steady-state | `rotate --frames N --buffers 3` | `VERDICT PASS`, presented == frames, dropped <= `--allow-drops`, releases == 2*(presented-1)+2*dropped and > 0, no order violations |
| A3 | stop-restart | long `two-planes`, SIGTERM once the first commit is seen; separately SIGKILL; then A1's master check again after each | each run exits within `--stop-wait` with an expected status, the restart passes |
| A4 | lease-loss | `revoke --source kernel-lease` | `VERDICT PASS`, `DeviceLost` on the first attempt, `device_lost_events=1`, nothing busy, no outputs or planes after, no commit after `before_revoke` |
| A5 | modeset | `modeset --source master` | `VERDICT PASS`, post-switch commits latched, original mode restored. `VERDICT BLOCKED` (exit 3, connector offers one mode) gives `BLOCKED` |
| A6 | display-recovery | `recovery --source master` | `VERDICT PASS`, `reported=true`, `still_busy=-`, `relatched >= 1`, `crtc_active_end=true`, commits before and after all latched, none of the first run after the loss, a `resources` record |

The script never starts or stops a display manager and never touches DRM,
udev or seat configuration. Stop the compositor yourself before the run and
restore it afterwards. The A5 mode switch and the A6 CRTC off are commits of
the harness on the device it was given, undone before it exits. Only
`--source master` and `kernel-lease` are used.

### Hook phases

`--hook` is called as `CMD ROW PHASE [LABEL]`. `hold-start` is called once
the hold is under way (the run's first `commit` record is seen), so the hook
may sleep and then take its picture; `hold-end` follows the end of the hold.
A1 and A3 pass a label (source, or signal); A3 also has `post-stop-start` /
`post-stop-end` and `restart-start` / `restart-end`; A6 calls the hold phases
twice, before the loss and after the recovery. The header of the script lists
every phase by row.

## Where it writes

Only below `--out` (default `/tmp/board-scenarios`, on the board when run
there). Per row `OUT/ROW/`:

- `result.txt`: first line `SCENARIO <name> PASS|FAIL|BLOCKED|NOT-RUN`, then
  evidence lines, then `failed: ...` lines;
- the raw harness records as `*.log` (with `--trace`, also `*.trace` and
  `*.trace-check`).

Nothing else is written; the script does not write to the repository.

## Output and exit codes

One `ROW name RESULT` line per row on standard output (`A1 startup PASS`).

| Exit | Meaning |
| --- | --- |
| 0 | every selected row PASS |
| 1 | a row FAILed |
| 2 | usage error |
| 3 | no FAIL, but a selected row is BLOCKED or NOT-RUN |

## Test

```
sh scripts/board-scenarios.test.sh
```

[`scripts/board-scenarios.test.sh`](board-scenarios.test.sh) is a host test
against a mocked `drm-harness` (a fake executable printing canned records in
the harness's format, chosen by `MOCK_*` variables). It needs no board and no
DRM device and never runs the script on hardware.

## See also

- [`docs/board-scenarios.md`](../docs/board-scenarios.md): the SoW §10
  scenarios and how each row would be run.
- [`docs/linux-board-scenarios.md`](../docs/linux-board-scenarios.md): the
  runner on other Linux boards, method and dated results.
