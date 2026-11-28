# am62p-photo-manifest

Turns the photographs of the AM62P board test and the saved
`scripts/camera-check.py` outputs into the per-scene lines of the M3 report
(`docs/m3-am62p-report.md`), in the form `scripts/check-m3-report.sh` accepts.
Python 3, standard library only, safe under `python3 -I`. It never runs the
camera or `camera-check.py`, never copies files and never reads EXIF or other
picture metadata (it only hashes the file bytes).

```
python3 -I scripts/am62p-photo-manifest.py --photos DIR --verdicts DIR
        [--link-prefix PREFIX] [--require-pass]
```

## Input naming

| Directory | Files for scene N (1..8) |
| --- | --- |
| `--photos` | `sceneN.jpg`, `sceneN.jpeg` or `sceneN.png` (exactly one) |
| `--verdicts` | `sceneN.txt`: saved stdout of `camera-check.py`; its last `VERDICT: PASS` or `VERDICT: FAIL` line counts. Or `sceneN.json`: saved `camera-check.py --json` output; its `pass` field counts. Not both. |

Save the verdict with e.g.
`python3 scripts/camera-check.py scene2.jpg expect-2.json > verdicts/scene2.txt`
(`am62p-scenes.py expect` makes the EXPECT file). If `camera-check.py` fails
with an input error it prints no `VERDICT` line, so the scene counts as
missing.

## Output

A markdown section on stdout, one line per scene:

```
## Photographs and camera-check

- Scene 2: camera-check PASS - [scene2.jpg](photos/scene2.jpg) sha256 <64 hex>
```

`--link-prefix` is the path from the report to the photograph directory
(default: the `--photos` value). It must be relative, so that no host path
enters the report; the linked file must exist there for `check-m3-report.sh`.
The tool does not copy photographs: put them where the prefix says.

A `FAIL` verdict is recorded as `FAIL` (the report is allowed to say so);
`--require-pass` turns it into an error.

## Exit status

0 ok. 1 a photograph or verdict is missing or ambiguous (also a `FAIL` with
`--require-pass`); the reasons go to stderr and nothing to stdout. 2 usage
error or absolute prefix.

## Test

```
sh scripts/am62p-photo-manifest.test.sh
```

Uses the 8 fixture photographs (8x8 PNG, no metadata, checked with
`check-photo-metadata.py`) and verdicts in `testdata/am62p-photo-manifest/`,
puts the output into a copy of `testdata/m3-report/good.md` (its photograph
section replaced) and runs `check-m3-report.sh` on it. It also covers missing
photo or verdict, a verdict file without `VERDICT` line, JSON verdicts, `.jpg`
names, two photographs of one scene, `--require-pass` and an absolute prefix.
