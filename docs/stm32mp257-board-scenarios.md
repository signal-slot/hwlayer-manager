# STM32MP257F-DK board scenarios A1-A5: observed 2026-11-28

One board session on the STM32MP257F-DK (ST LTDC, `stm` DRM driver, HDMI sink at 1920x1080), observed 2026-11-28 over ssh only, running rows A1-A5 of [`board-scenarios.md`](board-scenarios.md) with the repository's runner [`scripts/board-scenarios.sh`](../scripts/board-scenarios.sh) over the `tools/drm-harness` binary. The method is the one in [`linux-board-scenarios.md`](linux-board-scenarios.md) (read only); this page holds the results for this board until they are folded into `board-scenarios.md` and `platform-capabilities.md`.

## Portability evidence only

These runs are portability evidence. **They never count toward M1, M2 or M3**: M1 is the i.MX95 Linux run, M2 the i.MX95 Zephyr comparison, M3 the AM62P run. A PASS here says that the harness and the DRM backend behave on the LTDC driver; it satisfies no acceptance condition of those milestones.

Tags as in [`platform-capabilities.md`](platform-capabilities.md): **observed** (from this board session, with its date, here 2026-11-28), **read from source** (a repository file is named), **inferred** (a conclusion drawn from those).

## Session

| Item | Value | Tag |
| --- | --- | --- |
| Date | 2026-11-28 (build host's local date; in UTC the run was late on 2026-11-27). The board's clock is behind the host, as already noted in `docs/observed/stm32mp257.md`, so its file times say nothing about when the run happened | observed 2026-11-28 |
| Kernel | `uname -r` printed `6.6.129` | observed 2026-11-28 |
| Image | ST OpenSTLinux Starter Package v26.06.10, Weston image (unchanged since the earlier runs) | observed 2026-11-28 |
| DRM | `/dev/dri/card0`, driver `stm`; one connector, selected output 1920x1080 at 60 Hz (148500 kHz); planes 34 (primary), 42 and 49 (overlays), zpos 0..2, no scaling | observed 2026-11-28 |
| Harness | `tools/drm-harness` (package `composition-drm-harness`) from commit eab4162, built with `scripts/cross-aarch64.sh -p composition-drm-harness --bin drm-harness` (static aarch64 musl). Only the binary and `scripts/board-scenarios.sh` were copied to `/tmp` on the board; nothing was installed | observed 2026-11-28 |
| Weston before | `systemctl is-active weston-graphical-session seatd-weston` printed `active` twice; both were then stopped (`inactive` twice) | observed 2026-11-28 |
| Weston after | `/tmp` files removed, both units started again; `systemctl is-active weston-graphical-session seatd-weston` printed `active` twice, and again ten seconds later; the demo launcher's lock file was back in `/tmp` | observed 2026-11-28 |
| Configuration | no DRM, udev or seat configuration was changed; no serial console, reset, flashing or boot-chain change | observed 2026-11-28 |

Command line on the board (A2 hold chosen: **30 seconds**, so 1800 frames at the script's assumed 60 fps; the sink's mode is 60 Hz):

```
sh /tmp/board-scenarios.sh --out /tmp/board-scenarios --hold 30 --hook "logger -t bscen-hook" /tmp/drm-harness
```

The hook only marked the hold phases (`logger`); it took no picture. The runner's exit status was **3** (no FAIL, a row BLOCKED).

Summary lines printed by the script (verbatim):

```
A1 startup PASS
A2 steady-state PASS
A3 stop-restart PASS
A4 lease-loss PASS
A5 modeset BLOCKED
```

## Results

First lines of each `result.txt` (verbatim):

```
SCENARIO startup PASS
SCENARIO steady-state PASS
SCENARIO stop-restart PASS
SCENARIO lease-loss PASS
SCENARIO modeset BLOCKED
```

| Row | Log result | Counts (commits / latched / released) | KVM picture verdict | Tag |
| --- | --- | --- | --- | --- |
| A1 startup | `SCENARIO startup PASS` | master: 1 / 1 / -; kernel-lease: 1 / 1 / - (two-planes holds one frame and counts no release) | not observed | observed 2026-11-28 |
| A2 steady state | `SCENARIO steady-state PASS` | 1800 commits (`commit_count=1800`), 1800 latched (`presented=1800`, 0 dropped), 3598 released (= 2*(1800-1)) | not observed | observed 2026-11-28 |
| A3 stop/restart | `SCENARIO stop-restart PASS` | TERM run 1 / 1 / -, restart 1 / 1 / -; KILL run 1 / 1 / -, restart 1 / 1 / - | not observed | observed 2026-11-28 |
| A4 lease loss | `SCENARIO lease-loss PASS` | 60 frames before the revoke; 0 commit-like records after it; released=3,6 on the revoke (`releases=0` in `before_revoke`) | not observed (no hook call) | observed 2026-11-28 |
| A5 modeset | `SCENARIO modeset BLOCKED` | 0 / 0 / - (BLOCKED because no modes file was found, see bug 1) | not observed (no hook call) | observed 2026-11-28 |

**KVM pictures: not observed for every row.** The KVM's MCP server failed to connect for this session, so no capture was taken and `scripts/camera-check.py` judged nothing. `scripts/twoplane-expect.py` had nothing to convert either: the harness prints no `EXPECT` lines, only a free-text `step ... expect="blue full screen with white edge, red rectangle with yellow edge on top in the middle"` label (observed 2026-11-28). The verdicts above are log verdicts only.

### Evidence lines per row (verbatim from `result.txt` and the logs)

A1 (observed 2026-11-28):

```
VERDICT two-planes PASS 2 buffers on planes 34,42 (bottom,top)
commits=1 latched=1
```

for both sources; the kernel-lease log has `lease route=kernel ... connector=32 ... crtc=41 primary=34 overlays=42,49 objects=5` and `lease_check leased_planes=34,42,49 engine_planes=34,42,49 not_leased=- leased_not_seen=- outputs=1`. The layout: plane 34 full screen 1920x1080, plane 42 a 960x540 rectangle at 480,270.

A2 (observed 2026-11-28):

```
hold=30s fps=60 (assumed) frames=1800 buffers=3 allow_drops=0
VERDICT rotate PASS 1800 frames, 3 buffers per plane, 3598 releases in order, 0 dropped, 1 missed vblanks
frames=1800 presented=1800 dropped=0 releases=3598 order_violations=0
```

From `rotate_result`: latch latency median 13072 us, p99 13813 us, max 22501 us; latch interval median 16666 us, max 25713 us; `latch_timeouts=0`. The one missed vblank is not checked by the script (it only checks `dropped`).

A3 (observed 2026-11-28):

```
TERM: exit=143 after commits=1
VERDICT two-planes PASS 2 buffers on planes 34,42 (bottom,top)
commits=1 latched=1
KILL: exit=137 after commits=1
VERDICT two-planes PASS 2 buffers on planes 34,42 (bottom,top)
commits=1 latched=1
```

Both signalled runs had reached `hold step=two-planes seconds=600` after their one commit and exited within the 10-second `--stop-wait`; the restart after each passed A1's master check.

A4 (observed 2026-11-28):

```
VERDICT revoke PASS 60 frames on planes 34,42, then revoked: DeviceLost once, buffers 3,6 released
lease_revoked=1 first_attempt=Err(Backend(DeviceLost)) device_lost_events=1 still_busy=- outputs_after=0 planes_after=0
commit-like records after before_revoke=0
```

The first commit attempt after the revoke failed with `DeviceLost` after 7889 us; the second with `UnknownOutput` (the output was gone).

A5 (observed 2026-11-28):

```
connector=HDMIA-1 selected mode=1920x1080
BLOCKED: no readable modes file for connector HDMIA-1
```

Read separately over ssh before the run, the sysfs connector directory is `card0-HDMI-A-1`, and its `modes` file lists 1920x1080, 1280x720, 800x600 and 720x400 (with repeats). So the sink offers **more than one mode**; the row is BLOCKED, but for the wrong reason (bug 1). Had the file been found, the script would still have printed BLOCKED ("harness has no modeset scenario") (read from source: `scripts/board-scenarios.sh`). No modeset was attempted; no PASS is claimed.

## A3 and A5 looked at closely (first run on hardware)

The vkms dry run had never exercised these two rows.

- **A3 signal handling works as the script defines it** (observed 2026-11-28): the script waited for the first `commit` record, signalled, and the run exited within the 10-second `--stop-wait`. SIGTERM gave exit 143, the default action, not 0: the harness has no SIGTERM handler, so it tears nothing down itself and the kernel drops DRM master when the file closes (inferred). The script accepts 143 (read from source), and the restart reached a latched commit on the same planes both times, so the kernel freed the display. The design's "after SIGTERM the process exits cleanly" (`board-scenarios.md` A3) is therefore not shown: the process was killed by the signal.
- **A5's sysfs read does not match on this board**: bug 1 below.

## Bugs found (reported, not fixed)

Found 2026-11-28 on kernel 6.6.129. Not fixed here: `scripts/board-scenarios.sh`, `tools/drm-harness/**` and `crates/composition-drm/**` are held by other items.

1. **Connector name does not match the kernel's name, so A5 never finds the `modes` file** (observed 2026-11-28). The harness prints `connector=HDMIA-1`; the kernel's sysfs entry is `card0-HDMI-A-1`. The harness forms the name with `format!("{:?}-{}", info.interface(), info.interface_id())` (read from source: `tools/drm-harness/src/session.rs`, `connector_name`), so it prints the Rust `Debug` name of the interface (`HDMIA`), not the kernel name (`HDMI-A`). `row_a5` globs `card*-$conn/modes` and finds nothing. The host test `scripts/board-scenarios.test.sh` mocks `connector=HDMI-A-1`, so it misses this (read from source). The same name also shows up in the lease record (`connector_name=HDMIA-1`). Not tested here: whether `--connector HDMI-A-1`, the example in the harness's help, matches on such a board (inferred: it would not match, if the name is compared as a prefix).
2. **The script header says it prints `ROW name RESULT` lines, but it prints `A1 startup PASS`** (no `ROW` word) (observed 2026-11-28; read from source: `finish` in `scripts/board-scenarios.sh` prints `"$1 $2 $st"`). A caller that greps for `^ROW ` finds nothing.
3. **A2 ignores `missed_vblanks`** (read from source; observed 2026-11-28: `missed_vblanks=1` with a PASS). This may be intended, since the row bounds only `dropped`, but the design's "late frames above the stated bound" is not checked.

## What this session does not show

- No picture judgement: every KVM verdict is not observed.
- No mode change (A5), and no A6 (display recovery). The runner has no row for A6.
- No `dlm` source. The design's one-second bound for reporting the loss in A4 was not checked as a bound: the loss came 7889 us after the revoke, but the script does not judge timing.
- A2 is a 30-second hold, not the design's long run; memory and CPU bounds are not checked.
