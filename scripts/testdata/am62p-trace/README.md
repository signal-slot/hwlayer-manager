# am62p-trace fixtures

All files here are **synthetic**: generated for
`scripts/am62p-trace-extract.test.sh` in the line format of
`tests/board/am62p_r5f/src/scenes.c`. They are not board output and carry no
board identifiers. Dated 2026-11-28.

| File | What it models |
| --- | --- |
| `clean.txt` | one complete run with two lines before the banner |
| `wrapped.txt` | ring wrap: starts mid-line, a run, then an older run's tail |
| `nul.txt` | NUL padding, NULs after newlines, an undecodable line |
| `two-runs.txt` | an earlier failed run, then a passing run |
| `incomplete.txt` | scenes but no RESULT line |
| `incomplete-scene.txt` | RESULT present, scene 7 missing |
