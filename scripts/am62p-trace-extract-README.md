# am62p-trace-extract

Turns a raw remoteproc `trace0` dump of the AM62P R5F board test
(`tests/board/am62p_r5f`) into the run's own console lines. The trace buffer is
a fixed-size ring, so a dump can start mid-line, hold NUL padding, and hold an
older run's lines after a wrap. Python 3, standard library only. It does not
touch a board: read the dump over ssh first (see
`docs/board-test-am62p-r5f.md` and `docs/am62p-r5f-remoteproc-source.md`).

```
python3 scripts/am62p-trace-extract.py [DUMP|-] > console.txt
python3 scripts/am62p-scenes.py summary console.txt
```

With no argument, or `-`, it reads stdin.

## What it does

1. Drops NUL bytes, then drops lines that are not valid UTF-8 and empty lines.
   A first line cut in the middle stays; it is not part of a run and falls away
   in step 2.
2. Finds the start of the last run: the last `BEGIN hwlayer-manager board test
   am62p_r5f` line (`scenes.c`, `scenes_run()`). If no banner is left (the
   ring overwrote it), the last `SCENE 1 <name>:` line.
3. Takes the lines from there to the first `RESULT PASS|FAIL` line after it.
   Lines after that `RESULT` (an older run's wrapped tail) are dropped. Lines
   in between that are not `SCENE` or `RESULT` lines are kept in order.
4. Checks that `RESULT` is present and that a `SCENE n <name>: status ...`
   line exists for each of 1..8 (the `SCENE ... commits changed while held`
   line does not count).

The newest run decides: if a later run has started and has no `RESULT`, the
output is "incomplete", not the earlier complete run. This avoids reporting
stale results. Lines before the banner (`board test: output enable ...`) are
not part of the run and are not printed.

## Exit status

| Status | Meaning |
| --- | --- |
| 0 | a complete run was found and printed |
| 1 | no run, or the last run is incomplete: reason on stderr (`no RESULT line`, `missing SCENE 7`) |
| 2 | usage error, or the file cannot be read |

Exit 0 only says the run is complete. `RESULT FAIL` still exits 0; the verdict
is for `am62p-scenes.py summary`, which exits 1 on a failed scene.

## Test

`sh scripts/am62p-trace-extract.test.sh` runs it on the synthetic fixtures in
`scripts/testdata/am62p-trace/` (clean, wrapped, NUL padding, two runs,
incomplete) and checks that `am62p-scenes.py summary` accepts the output. The
fixtures are not board output; `generate.py` there rebuilds them.

## Limits

A wrap that cuts the newest run's banner and its `SCENE 1` line makes it
incomplete (`missing SCENE 1`). If an older run's banner survives after the
newest run's `RESULT`, the older banner counts as the newest run. Neither
happens with one run per boot, the case the board session uses.
