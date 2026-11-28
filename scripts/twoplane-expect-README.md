# twoplane-expect

Turns the `EXPECT` lines printed by the `twoplane` and `plane_commit`
examples of `crates/composition-drm` into `scripts/camera-check.py` EXPECT
JSON files, one per phase that ran. `twoplane` drives the two-plane scene of
the `backends/twoplane` backend (`docs/twoplane-backend.md`); this script only
reads what it printed and never links the backend or touches a device. Python
3, standard library only.

**The output is computed from the text it is given. It is not a board result**:
it states the colours a picture should show, and says nothing about whether any
board showed them. A board result exists only once `camera-check.py` has judged
a real capture against these files and the verdict is recorded with a date.

```
python3 -I scripts/twoplane-expect.py [FILE|-] --out DIR [--size F] [--tolerance N]
        [--rotation N] [--panel full|corners] [--corners x,y,x,y,x,y,x,y]
sh scripts/twoplane-expect.test.sh
```

## Arguments

| argument | default | meaning |
|---|---|---|
| `FILE` or `-` (positional, at most one) | stdin | the example's stdout; `-` or no value reads stdin. A second positional or an unknown `--option` is an input error. |
| `--out DIR` | none, **required** | directory for the JSON files (created if missing) |
| `--size F` | `0.02` | side of each sampled square as a fraction of the picture, in (0, 1]; the rectangle is the sample point +/- F/2, clipped to 0..1 |
| `--tolerance N` | `40` | per-region colour tolerance written into the JSON (a number) |
| `--rotation N` | `0` | `rotation` written into the JSON (integer; camera-check's own default for a camera is 180) |
| `--panel full\|corners` | `full` | `full`: the whole picture is the panel (a KVM capture). `corners`: use `--corners` (a camera) |
| `--corners x,y,x,y,x,y,x,y` | none | eight fractions, TL, TR, BR, BL; required with `--panel corners`, ignored with `full` |

## Input

Lines of the form

```
EXPECT phase=<phase> "<region name>" x=<px> y=<px> fx=<frac> fy=<frac> rgb=#rrggbb
```

- `<phase>` is any non-empty string of `[a-z0-9_-]` (`twoplane`: `normal`,
  `swapped`; `plane_commit`: `commit`). Another `phase=` spelling is an input
  error. `x` and `y` are read but not used; `fx` and `fy` place the region.
- A phase is skipped when one of its `EXPECT` lines carries the token
  `skipped` (for example `EXPECT phase=commit skipped reason=...`), or, for
  `swapped`, when a `SWAPZ skipped` line is present. A skipped phase gets no file.
- Every other line (`GEOMETRY ...` and so on) is ignored. A malformed
  `EXPECT` line (missing field, `rgb` not `#` plus six hex digits) is an input error.

## Output

`DIR/expect-<phase>.json` per phase, in the order the phases appear, with
`rotation`, `panel` (`{"corners": [[x,y]x4], "unit": "frac"}`), `tolerance`
and `regions` (`name`, `rect` as `[u0, v0, u1, v1]` fractions rounded to four
places, `expect` as `[r, g, b]`). On stdout one `wrote PATH (N regions)` line
per file, then one `skipped phase NAME` line per skipped phase (sorted).
Errors go to stderr as `twoplane-expect: ...`.

```
python3 -I scripts/twoplane-expect.py twoplane.txt --out expect --panel full
python3 -I scripts/camera-check.py capture.png expect/expect-normal.json
```

## Exit codes

| code | meaning |
|---|---|
| 0 | at least one phase file written |
| 2 | input error: no `--out`, bad or missing option value, `--size` out of range, `--panel corners` without eight corners, unknown `--panel`, unreadable input or output path, bad phase name, malformed `EXPECT` line, or no `EXPECT` lines for a phase that ran (including when every phase is skipped) |

## Test

`sh scripts/twoplane-expect.test.sh` builds canned `EXPECT` text under a
`mktemp` directory in `$TMPDIR` and checks: normal only, normal plus swapped,
both skip forms, the `commit` phase and a skipped one, mixed phases, stdin,
the JSON content, every exit-2 case it names (no `--out`, missing input, no
`EXPECT`, bad rgb, only skipped, bad phase name), and a round trip through
`camera-check.py` with a painted picture (PASS) and a wrong colour (FAIL). The
pictures are synthetic: the test is a host check of the converter, not a
board measurement. It needs no device.
