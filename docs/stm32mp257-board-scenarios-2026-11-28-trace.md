# STM32MP257F-DK board scenarios A1-A6 with `--trace`: observed 2026-11-28 (second run)

Second board session on the STM32MP257F-DK (ST LTDC, `stm` DRM driver, HDMI sink at 1920x1080) over ssh only, rows A1-A6 of [`board-scenarios.md`](board-scenarios.md) with [`scripts/board-scenarios.sh`](../scripts/board-scenarios.sh) over the `tools/drm-harness` binary, now with `--trace`. It follows [`stm32mp257-board-scenarios.md`](stm32mp257-board-scenarios.md) (A1-A5, A5 BLOCKED by the connector name, no A6).

Portability evidence only: it never counts toward M1, M2 or M3. Tags: **observed** (this session), **read from source**, **inferred**.

## Session

| Item | Value | Tag |
| --- | --- | --- |
| Host UTC times | copy 10:45:16-10:45:19; units stopped 10:45:19; run 10:45:23-10:46:32 (board clock 00:19:08-00:20:17, behind the host); extra A6 run 10:46:40-10:46:47; cleanup and units started 10:46:56; second `is-active` check 10:47:03 + 11 s (10:47:15) | observed 2026-11-28 |
| Kernel | `uname -r` printed `6.6.129` | observed 2026-11-28 |
| Commit | harness built from `e4fc84a` (main tip) with `scripts/cross-aarch64.sh -p composition-drm-harness --bin drm-harness` (static aarch64 musl) | observed 2026-11-28 |
| Copied to the board | the binary and `scripts/board-scenarios.sh`, into one directory under `/tmp` | observed 2026-11-28 |
| Weston before | `systemctl is-active weston-graphical-session seatd-weston` printed `active` and `active`; both stopped (`inactive`, `inactive`) | observed 2026-11-28 |
| Weston after | both units started; `is-active` printed `active` and `active`, and again `active` and `active` 11 s later | observed 2026-11-28 |
| `/tmp` | the directory was removed; `/tmp` holds only what it held before (demo launcher lock and system entries) | observed 2026-11-28 |
| Configuration | no reboot, flashing, boot-media, U-Boot, DRM, udev or seat change, no module load, no console or reset | observed 2026-11-28 |
| KVM | the newest rehearsal in [`observed/stm32mp257-kvm.md`](observed/stm32mp257-kvm.md) (2026-11-28 10:00 UTC, fourth) says **capture fails**, and the KVM's MCP server again failed to connect at the start of this session. Hook: `logger -t bscen-hook` only | observed 2026-11-28 |

Command line on the board (A2 hold 30 s = 1800 frames at the assumed 60 fps):

```
sh ./board-scenarios.sh --out OUT --hold 30 --trace --hook "logger -t bscen-hook" ./drm-harness
```

Exit status **1** (A6 FAILed). Summary lines (verbatim):

```
A1 startup PASS
A2 steady-state PASS
A3 stop-restart PASS
A4 lease-loss PASS
A5 modeset PASS
A6 display-recovery FAIL
```

First lines of each `result.txt` (verbatim):

```
SCENARIO startup PASS
SCENARIO steady-state PASS
SCENARIO stop-restart PASS
SCENARIO lease-loss PASS
SCENARIO modeset PASS
SCENARIO display-recovery FAIL
```

## Results per row

KVM picture verdict: **not observed** for every row (capture fails).

| Row | Result line | Counts (commits / latched / released) | Picture | Tag |
| --- | --- | --- | --- | --- |
| A1 startup | `SCENARIO startup PASS` | master 1 / 1 / 0; kernel-lease 1 / 1 / 0 | not observed | observed 2026-11-28 |
| A2 steady state | `SCENARIO steady-state PASS` | 1800 / 1800 / 3598, 0 dropped, 1 missed vblank (not judged by the script) | not observed | observed 2026-11-28 |
| A3 stop/restart | `SCENARIO stop-restart PASS` | TERM 1 / 1 / 0 (exit **0**), restart 1 / 1 / 0; KILL exit 137 with commit 0 in the trace (see below), restart 1 / 1 / 0 | not observed | observed 2026-11-28 |
| A4 lease loss | `SCENARIO lease-loss PASS` | 60 / 60 / 120, `device_lost_events=1`, 0 commit-like records after `before_revoke` | not observed | observed 2026-11-28 |
| A5 modeset | `SCENARIO modeset PASS` | 111 / 111 / 218 | not observed | observed 2026-11-28 |
| A6 recovery | `SCENARIO display-recovery FAIL` | 359 commits (358 ok, 1 error) / 358 / 712 | not observed | observed 2026-11-28 |

### trace-check TRACE counts per run (X-03 on non-NXP hardware)

Every trace file parsed strictly (`trace lines=N failed=0` in each log, `trace-check` exit 0). The script's cross-check of the trace counts against the harness's own records raised no mismatch in any run; A6 failed for its own reason below (observed 2026-11-28).

| Run | lines | check | validate | commit (ok / err) | latched | released | dropped | device_lost |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A1 master | 5 | 1 | 1 | 1 (1 / 0) | 1 | 0 | 0 | 0 |
| A1 kernel-lease | 5 | 1 | 1 | 1 (1 / 0) | 1 | 0 | 0 | 0 |
| A2 rotate | 10799 | 1800 | 1800 | 1800 (1800 / 0) | 1800 | 3598 | 0 | 0 |
| A3 TERM | 5 | 1 | 1 | 1 (1 / 0) | 1 | 0 | 0 | 0 |
| A3 restart after TERM | 5 | 1 | 1 | 1 (1 / 0) | 1 | 0 | 0 | 0 |
| A3 KILL | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| A3 restart after KILL | 5 | 1 | 1 | 1 (1 / 0) | 1 | 0 | 0 | 0 |
| A4 revoke | 365 | 61 | 62 | 60 (60 / 0) | 60 | 120 | 0 | 1 |
| A5 modeset | 663 | 111 | 111 | 111 (111 / 0) | 111 | 218 | 0 | 0 |
| A6 recovery | 2150 | 360 | 360 | 359 (358 / 1) | 358 | 712 | 0 | 0 |

`dropped_replaced`, `dropped_device_lost`, `latch_timeout` and `caps_changed` were 0 in every run. The A3 KILL file holds the header line only: the SIGKILLed process had not flushed its commit (the script only parses it, by design). The trace layer therefore parses and agrees with the harness's records on the ST LTDC driver (observed 2026-11-28); no comparison with NXP counts is made here.

## A5 modeset

`SCENARIO modeset PASS` (observed 2026-11-28):

```
connector=HDMI-A-1 current=1920x1080@60 modes listed=10
VERDICT modeset PASS 1920x1080@60 -> 1920x1080@50: 61 commits latched before, 50 after the switch; 1920x1080@60 restored
from=1920x1080@60 to=1920x1080@50 restored=1920x1080@60
```

The switch took 61863 us, the restore 69340 us; `crtc_mode` read back each mode. The first other mode that passed TEST_ONLY was 1920x1080 at 50 Hz, not a different size. The sink was not looked at (no picture).

## A6 recovery: FAIL, a new defect

Observed 2026-11-28, `--loss master` (the script's default):

```
loss=master made=ok crtc_active_after_loss=false
first=error:validate:Backend(Platform(13)) first_ms=0 second=error:commit:Backend(Rejected) master_back=ok
reported=true still_busy=5,6 relatched=177 crtc_active_end=true
recovery-before: commits=181 latched=181
recovery-after: commits=177 latched=177
VERDICT recovery FAIL buffers 5,6 still submitted or scanning after the loss was reported: the engine did not release them
```

An extra A6-only run with `--loss disable` (10:46:40 UTC) failed the same way:

```
loss=disable made=ok crtc_active_after_loss=false
first=error:commit:Backend(Rejected) first_ms=0 second=- master_back=-
reported=true still_busy=5,6 relatched=177 crtc_active_end=true
VERDICT recovery FAIL buffers 5,6 still submitted or scanning after the loss was reported: the engine did not release them
```

Findings:

- The loss is made, reported (`reported=true`) and recovered from: a new run relatched 177 of 177 commits with the CRTC active at the end. Only the check that the engine holds no buffer fails: `buffers_after_loss` shows buffers 5 and 6 in state `Scanning` (the two buffers on screen) after the loss, in both loss modes (observed).
- With `--loss master` the first attempt after the loss failed at `validate` with errno 13 (`NotMaster`), the second at `commit` with errno 22 (`Refused`) after the master was returned.
- Defect or too strict a check is not decided here: the engine keeps the on-screen buffers as `Scanning` when a commit is refused while the CRTC is off. Whether the engine should release them on `NotMaster` / `Refused`, or the scenario should accept them, needs a read of `crates/composition-drm` (inferred; not read). Whether this is specific to LTDC is not known: there is no A6 run on another board in this record.
- Not fixed: `tools/drm-harness/**`, `scripts/board-scenarios*.sh` and `crates/composition-drm/**` are held by other items.

## Connector-name bug: gone

The harness now prints `connector=HDMI-A-1` (earlier `HDMIA-1`), in the `output` record and in the A5 result; `row_a5` found the sysfs `modes` file and ran (observed 2026-11-28). Bug 1 of the first run is fixed on main for this board. Bug 2 of the first run (header says `ROW name RESULT`) was not looked at again; the summary lines still print `A1 startup PASS` without the word `ROW`.

## Other changes against the first run

- A3 TERM now exits **0** (earlier 143): the harness handles SIGTERM on current main (observed 2026-11-28).
- A5 and A6 exist and ran.

## What this run does not show

- No picture judgement (KVM capture fails; see above).
- A2 is a 30-second hold, not the design's long run; no memory bound.
- A6 passed in neither loss mode; the recovery claim rests on `reported=true`, `relatched` and `crtc_active_end=true` only.
- No `dlm` source.
