# scale-probe-expect

Judges the `SCALE` test lines and `RESULT` summaries printed by the
`scale_probe` example, and compares them with the `ScaleLimits` a profile
claims. Python 3, standard library only. The input line formats below are
read from source: [scale_probe.rs](../crates/composition-drm/examples/scale_probe.rs).

```sh
python3 -I scripts/scale-probe-expect.py scale.log
cargo run -p composition-drm --example scale_probe -- --max-factor 32 | \
    python3 -I scripts/scale-probe-expect.py - --profile-up 32 --profile-down 2
```

Options:

- `--profile-up F`: the profile's claimed `max_upscale` factor (F >= 1).
- `--profile-down F`: the profile's claimed `max_downscale` factor, source
  over destination, so `2` means factor 0.5000 (F >= 1).
- Several files are read as one log; with no file, or `-`, stdin is read.

## What it judges

Each tested axis group (plane, axis `h` / `v` / `both`, and one `--dst` size)
gets one judged line with the largest accepted upscale factor and the
smallest accepted downscale factor:

```
SCALE plane=42 axis=h up_max=2.5000 down_min=0.7500
```

- `up_max` / `down_min` are factors of destination over source, printed as
  `scale_probe` prints factors (`0.5000`), or `none` when nothing on that
  side was accepted. A group judged only from a `RESULT` line shows the
  probe's own text (a trailing `+` means "no rejection seen up to here").
- A group whose accepted factors are not one interval - a rejected factor
  sits between two accepted ones, or one factor was accepted once and
  rejected in another run - is flagged:

  ```
  SCALE plane=42 axis=h non_monotonic
  ```

- With profile claims, each judged axis also gets an `EXPECT` line ending in
  `ok`, `fail` or `unproven`:

  ```
  EXPECT plane=42 axis=h up_max=2.5000 profile_up=2.0000 down_min=0.7500 profile_down=2.0000 ok
  EXPECT plane=42 axis=both up_max=8.0000 profile_up=32.0000 unproven reason=up_unproven
  ```

  `ok`: the measurements reach the claim and nothing was refused inside it.
  `fail`: the kernel refused a factor the claim says is allowed
  (`reason=up_refused_at=<factor>` or `reason=down_refused_at=<factor>`).
  `unproven`: the log never tested up to the claim and refused nothing, so
  the claim is not established (`reason=up_unproven`, `reason=down_unproven`).
  `unproven` is kept apart from `fail` on purpose: "tested 2x only" is not
  "the kernel refused 32x". A profile claim above the tested range therefore
  asks for a longer probe (`scale_probe --max-factor <claim>`), not a fix.

- `RESULT` lines are cross-checked against the judged `SCALE` lines of the
  same log; a mismatch prints a `NOTE` line (for example
  `NOTE plane=42 axis=h result_max_upscale=3.0000 measured_up_max=2.5000`)
  and does not change the verdict. A plane with a `RESULT` line but no
  `SCALE` lines is judged from the `RESULT` fields alone; its `down_min` is
  then the probe's plane-wide `min_downscale` on every axis.
- If one plane was probed at two different `--dst` sizes, each group is
  judged apart and its lines carry a `dst=WxH` token. Other probe lines
  (`DATE`, `PLANE`, `NOTE`, `SKIP`, ...) are ignored.

## Verdict and exit status

The summary counts the judged lines, then one verdict line:

```
SUMMARY planes=1 judged=3 expect=3 ok=1 fail=2 unproven=0 non_monotonic=0 notes=0
VERDICT: PASS | FAIL (reasons)
```

| exit | meaning |
|---|---|
| 0 | PASS |
| 1 | FAIL: a `non_monotonic` flag, an `expect fail`, an `expect unproven`, or no judged line |
| 2 | usage or input error (bad option or value, unreadable file, malformed `SCALE` / `RESULT` line) |

Self-test: `bash scripts/scale-probe-expect.test.sh` (synthetic logs inline,
no board data).
