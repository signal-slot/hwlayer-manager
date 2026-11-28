# mode-test-expect

Judges the log of a [`mode_test`](../crates/composition-drm/examples/mode_test.rs)
run (a TEST_ONLY modeset of each connector mode) and turns it into one line
per mode plus a mechanical `EXPECT` verdict — the checker behind HVS Q10, the
BCM2712 HDMI mode limits in
[`docs/hvs-backend-design.md`](../docs/hvs-backend-design.md) §7. Every
output and exit code described below is read from source
(`mode_test.rs` and its `lines` unit test). Python 3, standard library only.

```sh
cargo run -p composition-drm --example mode_test > mode.log 2>&1
python3 -I scripts/mode-test-expect.py mode.log
python3 -I scripts/mode-test-expect.py mode.log --expect-min-clock 594000
```

Capture the run with `2>&1` as above: `mode_test` reports a missing DRM
master on stderr, and the checker can only name that reason in a combined
log. One log per run — a log with two runs in it is an input error.

## Input

The script reads a saved `mode_test` log (file or `-` for stdin) and judges
only these lines; `DATE`, `KERNEL`, `DEVICE`, `CONNECTOR` and `LISTED` lines
are skipped:

```
mode WxH@R clock=KHZ test=ok
mode WxH@R clock=KHZ test=refused errno=E
mode_test: tested=N ok=M highest_ok_clock=KHZ
mode_test: DRM master is unavailable: ...
```

## Output

```
MODE name=WxH@R clock=KHZ result=ok|E
MODE highest_ok_clock=KHZ
MODE refused=N
EXPECT ok|fail          (only with --expect-min-clock)
```

One `MODE` line per mode line, in log order; `result` is `ok` or the numeric
errno as `mode_test` printed it (22 = EINVAL, 28 = ENOSPC).
`highest_ok_clock` is the largest pixel clock in kHz of a passing mode (0 if
none passed, as `mode_test` counts it) and `refused` the count of refused
mode lines; the script recomputes all three and exits 2 when the
`mode_test:` summary line disagrees with the mode lines, is malformed, or
appears twice.

With `--expect-min-clock KHZ` a completed run gets a final `EXPECT ok` when
`highest_ok_clock >= KHZ`, `EXPECT fail` otherwise (KHZ = 0 always passes).

A log without a summary line is an unfinished run — e.g. exit 2 from
`mode_test` because it could not take DRM master. The script then prints the
counts the log shows so far, adds

```
MODE incomplete reason=drm-master|no-summary
```

(`drm-master` when the log holds the "DRM master is unavailable" error,
otherwise the run died before its summary; the `mode_test:` error line is
echoed on stderr), prints **no** `EXPECT` line, and exits 1: the expectation
is unanswered, not failed. An empty log is unfinished too.

## Exit status

| exit | meaning |
|---|---|
| 0 | run completed, and the expectation met if one was given |
| 1 | `EXPECT fail`, or the run did not complete (no summary line) |
| 2 | usage or input error (bad option, unreadable file, malformed or mixed log) |

Self-test: `bash scripts/mode-test-expect.test.sh` (synthetic logs, no board
data).
