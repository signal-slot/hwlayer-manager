# profile-check-expect

Judges the planner-vs-TEST_ONLY agreement lines printed by the
`profile_check` example, so a board run gets a mechanical verdict.
Python 3, standard library only.

```sh
cargo run -p composition-drm --example profile_check ... | python3 -I scripts/profile-check-expect.py -
python3 -I scripts/profile-check-expect.py run.log --expect-profile rk3588-vop2
python3 -I scripts/profile-check-expect.py run.log --markdown
```

Options:

- `--allow-disagree N` tolerates up to N DISAGREE lines (default 0).
- `--expect-profile NAME` fails when no line carries `profile=NAME`.
- `--markdown` prints a table (and a table of the DISAGREE lines) instead of
  the plain summary.

## Line format

```
AGREE profile=rk3588-vop2 frame=two-plane planner=accept kernel=ok
DISAGREE profile=vc4-hvs6 frame=sand128 planner=accept errno=22 note="a b"
```

A judged line starts at column 0 with `AGREE` or `DISAGREE`, followed by
space-separated `key=value` tokens. Expected keys: `profile`, `frame`,
`planner`, and `kernel` or `errno`. Values may be quoted; unknown keys are
kept and shown for DISAGREE lines. Any other line is skipped, so mixed logs
work. A line without `profile=` counts under profile `?`.

## Verdict and exit status

The summary ends with one `VERDICT: PASS` or `VERDICT: FAIL (reasons)` line.

| exit | meaning |
|---|---|
| 0 | PASS |
| 1 | FAIL: more DISAGREE lines than allowed, no AGREE/DISAGREE line, or the expected profile is missing |
| 2 | usage or input error (bad option, unreadable file) |

Self-test: `bash scripts/profile-check-expect.test.sh` (fixtures in
`scripts/testdata/profile-check-expect/`).
