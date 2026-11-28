# am62p-console-compare

Compares the console of the AM62P R5F board test (`tests/board/am62p_r5f/`) with the host reference, the console of `board_scenes_behind_a_fake_panel` in `backends/twoplane/tests/twoplane.rs`. Both run `src/scenes.c`, so the lines have one format. Host tool only. It claims no board result: it says whether two files agree.

```sh
cargo test -p composition-twoplane board_scenes -- --nocapture > host.txt
python3 -I scripts/am62p-console-compare.py board.txt host.txt
```

Exit 0 on `COMPARE PASS`, 1 on `COMPARE FAIL`, 2 on a usage or read error.

## What is read

Only lines that contain `SCENE <n> <name>:` or `RESULT PASS|FAIL`. Whatever precedes that text on the line is dropped (a `[trace] ` prefix, a timestamp). Every other line (`BEGIN`, `PANEL`, cargo output) is ignored. The formats come from `scenes.c`:

```
SCENE <n> <name>: status <s> (expected <s>), commits <c> latched <l> (+<d>, expected +<d>), presented yes|NO|-, released <r> (expected <r>): ok|FAIL
SCENE <n> <name>: commits changed while held: <a> -> <b>: FAIL
RESULT PASS (8 scenes, vsyncs <v>)
RESULT FAIL (<k> checks failed, vsyncs <v>)
RESULT FAIL (setup)
```

If a line repeats (a console holding two runs), the last one wins.

## Output

One line per scene: `SCENE <n>: MATCH`, `SCENE <n>: DIFF <field> board=<x> host=<y>`, or `SCENE <n>: MISSING missing on board|host`. The "commits changed while held" line is its own entry, `SCENE <n> held`, so it shows as MISSING on the side that did not print it. `RESULT` is compared the same way. The last line is `COMPARE PASS` or `COMPARE FAIL`.

## What fails and what does not

Failing: a difference in the scene name, status, expected values, `+d`, expected `+d`, `presented`, `released`, the `ok`/`FAIL` verdict, or the RESULT verdict and detail; any MISSING entry; an input with no SCENE or RESULT line.

Reported but not failing (`DIFF soft (not a failure)`): the absolute `commits` and `latched`, and `vsyncs` in RESULT. The reason comes from `docs/board-test-am62p-r5f.md`: the host fake driver only simulates the latch at frame start, and the board latches at real VSYNC. The absolute counts also depend on what the driver did before the first scene, and the vsync count on timing. What is comparable is the per-scene change (`+d`) and the firmware's own judgement that latched equals commits, which is part of the `ok` verdict. A latch disagreement on the board therefore shows up as a hard DIFF on the verdict.
