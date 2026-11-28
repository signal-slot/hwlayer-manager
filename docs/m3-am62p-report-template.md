# M3 AM62P report template

**This file holds no observation.** It is the skeleton for
`docs/m3-am62p-report.md` (not yet written). Every `<...>` is a placeholder
that the tool named under it fills in from the board run. Copy the file to
`docs/m3-am62p-report.md`, replace each placeholder, and remove this
paragraph and the "Filled by" lines. The finished report must pass
`scripts/check-m3-report.sh` (structure; see
`scripts/testdata/m3-report/good.md` for a passing example).

In this template the console lines carry a `[trace]` marker before `SCENE`
so that the template itself never matches the checker. In the report, delete
the marker so each line starts with `SCENE N` (inside a fenced block).

Tools: `scripts/am62p-board-info.sh`, `scripts/am62p-state-snapshot.sh`,
`scripts/am62p-scenes.py`, `scripts/am62p-torizon-check.sh` and
`scripts/am62p-r5f-run.sh` are planned (some may not have landed);
`scripts/camera-check.py` exists, see [camera-check-README.md](../scripts/camera-check-README.md).
Keep board addresses, serials, MACs, tty paths and credentials out of the
report.

## Date

Date: <YYYY-MM-DD of the board run>

Filled by: the author, from the run log of `scripts/am62p-r5f-run.sh` (planned).

## Board

Board: <Verdin AM62P module and carrier, revision>

Filled by: `scripts/am62p-board-info.sh` (planned, redacts serials).

## BSP

BSP: <Torizon OS version, image/build id>

Filled by: `scripts/am62p-board-info.sh` (planned).

## Kernel

Kernel: <uname release, build date>

Filled by: `scripts/am62p-board-info.sh` (planned).

## Panel

Panel: <panel or bridge name, mode WxH@Hz, DSS output used>

Filled by: `scripts/am62p-board-info.sh` (planned).

## Firmware image

- Image: <file name, size, build commit of hwlayer-manager>
- Route: <remoteproc: firmware name, core, carve-out>
- Build inputs: <showcase, Zephyr workspace and SDK versions>

Filled by: `scripts/am62p-r5f-run.sh` (planned) and the image build log.

## Console (trace buffer)

Eight SCENE lines and the RESULT line, copied unedited from the R5F
remoteproc trace buffer (the serial console is optional).

```
[trace] SCENE 1 <name> commits=<n> latched=<n> ...
[trace] SCENE 2 <name> commits=<n> latched=<n> ...
[trace] SCENE 3 <name> commits=<n> latched=<n> ...
[trace] SCENE 4 <name> commits=<n> latched=<n> ...
[trace] SCENE 5 <name> commits=<n> latched=<n> ...
[trace] SCENE 6 <name> commits=<n> latched=<n> ...
[trace] SCENE 7 <name> commits=<n> latched=<n> ...
[trace] SCENE 8 <name> commits=<n> latched=<n> ...
[trace] RESULT <PASS|FAIL>
```

Filled by: `scripts/am62p-r5f-run.sh` (planned; reads the trace buffer over ssh).

## Two-plane composition

<which scene shows base plus overlay, plane roles, formats, and whether the
output matches the expectation>

Filled by: `scripts/am62p-scenes.py` (planned; console summary and expected
values per scene), `scripts/scenario-expect.py` where it fits.

## Commits vs latched (GO bit)

| Scene | Commits | Latched | Match |
|-------|---------|---------|-------|
| 1 | <n> | <n> | <yes/no> |
| 2 | <n> | <n> | <yes/no> |
| 3 | <n> | <n> | <yes/no> |
| 4 | <n> | <n> | <yes/no> |
| 5 | <n> | <n> | <yes/no> |
| 6 | <n> | <n> | <yes/no> |
| 7 | <n> | <n> | <yes/no> |
| 8 | <n> | <n> | <yes/no> |

<explanation of any difference between commits and latched counts>

Filled by: `scripts/am62p-scenes.py` (planned).

## Releases

<per scene: planes released, release count, any leaked plane>

Filled by: `scripts/am62p-scenes.py` (planned).

## Refused scenes

<which scenes were refused by the backend, with the reason, and whether the
refusal was expected>

Filled by: `scripts/am62p-scenes.py` (planned).

## Photographs and camera-check

One photograph per scene, with the verdict of `scripts/camera-check.py`.
Replace each placeholder path with the committed photograph (the checker
verifies that linked files exist).

- Photo 1: <photo-scene1.png>
- Photo 2: <photo-scene2.png>
- Photo 3: <photo-scene3.png>
- Photo 4: <photo-scene4.png>
- Photo 5: <photo-scene5.png>
- Photo 6: <photo-scene6.png>
- Photo 7: <photo-scene7.png>
- Photo 8: <photo-scene8.png>

- Scene 1: camera-check <PASS|FAIL|SKIP|BLOCKED> <detail>
- Scene 2: camera-check <PASS|FAIL|SKIP|BLOCKED> <detail>
- Scene 3: camera-check <PASS|FAIL|SKIP|BLOCKED> <detail>
- Scene 4: camera-check <PASS|FAIL|SKIP|BLOCKED> <detail>
- Scene 5: camera-check <PASS|FAIL|SKIP|BLOCKED> <detail>
- Scene 6: camera-check <PASS|FAIL|SKIP|BLOCKED> <detail>
- Scene 7: camera-check <PASS|FAIL|SKIP|BLOCKED> <detail>
- Scene 8: camera-check <PASS|FAIL|SKIP|BLOCKED> <detail>

Filled by: `scripts/camera-check.py` (verdicts), the camera capture of the
board session (photographs).

## Fixes and repeat runs

<what the first run showed, what was changed in `tests/board/am62p_r5f` or
`backends/twoplane`, and the result of the repeat run (a second image needs a
reboot); write "none" if the first run passed>

Filled by: the author.

## Restore and Torizon confirmation

Date: <YYYY-MM-DD of the restore>

- Stopped or unbound before the run: <list: remoteproc state, drivers,
  display services>
- Restored: <list, one entry per item above, with the state after restore>
- Rebooted into the Torizon boot chain: <yes/no>
- Torizon up over ssh: <confirmation text, e.g. os-release name and version,
  no address>

Filled by: `scripts/am62p-state-snapshot.sh` (planned; before/after
compare) and `scripts/am62p-torizon-check.sh` (planned; read-only ssh check).
