# am62p-scenes

Helper for the AM62P R5F board test (`tests/board/am62p_r5f`, branch
`work/am62p-board-test`; see `docs/board-test-am62p-r5f.md` there). The test
runs eight scenes, prints one `SCENE` line for each and ends with a `RESULT`
line. This script does two things with them. Python 3, standard library only.
It does not need the branch merged: the scenes' colours and geometry are
written into the script, with the `scenes.c` line they come from (commit
`361e31f`). If `scenes.c` changes, change the script.

```
python3 scripts/am62p-scenes.py expect --scene N|NAME --width W --height H
        [--tolerance N] [--rotation N] [--panel full|corners] [--corners x,y,...]
python3 scripts/am62p-scenes.py summary CONSOLE|-
```

Exit status: 0 = ok, 1 = `summary` found a failed scene or `RESULT FAIL`,
2 = usage or input error.

## expect

Prints the `scripts/camera-check.py` EXPECT JSON of one scene (number 1..8 or
name). Use it with a photograph of that scene:

```
python3 scripts/am62p-scenes.py expect --scene 2 --width 1280 --height 800 > expect-2.json
python3 scripts/camera-check.py scene-2.jpg expect-2.json
```

| # | Name | Application plane | Safety plane (red) |
| --- | --- | --- | --- |
| 1 | `safety-only` | off (background) | whole panel |
| 2 | `app-under-safety` | blue `(0,0,255)` | rectangle left |
| 3 | `app-replaced` | green `(0,255,0)` | rectangle left |
| 4 | `safety-flipped` | green | rectangle right |
| 5 | `app-off` | off, black expected | rectangle right |
| 6, 7, 8 | the refused scenes | as scene 5 (nothing was written) | as scene 5 |

The rectangle is a third of the width and half the height, centred vertically,
`width/12` from the left or right edge (`scenes_rect()`). Each EXPECT has
regions in the red rectangle, in the opposite rectangle position, and above
and below the rectangle; they are cut well inside the areas so that blur and
the firmware's integer division stay out. In scene 1 they are in the middle and
at both sides. `--width` and `--height` only have to describe a panel big
enough for the scenes: the regions are fractions of the panel.

Options:

- `--tolerance` (default 60): camera-check's RGB distance. Loose, because a
  camera does not return sRGB-exact colours.
- `--rotation` (default 180): the board camera's picture is upside down. Use 0
  for a KVM picture.
- `--panel full` (default): the picture's own corners after rotation.
  `--panel corners --corners TLx,TLy,TRx,TRy,BRx,BRy,BLx,BLy` gives the panel's
  corners in the rotated picture, as fractions of it. A panel with a black
  background cannot be found automatically, so a camera picture with a dark
  surround needs `corners`.

The background of scenes 1 and 5 to 8 is expected black. If the controller's
background is not black, the board shows it: change the colour in the JSON and
record it in the report.

## summary

Reads a console capture (the remoteproc trace buffer, or the UART; `-` for
standard input) and prints a Markdown table, one row for each scene: status
(expected), commits, latched, delta (expected), presented, released
(expected), verdict. Then the `RESULT` line. Text before `SCENE` or `RESULT`
on a line and CR line ends are ignored, other lines too.

The verdict of a row is `ok` only if the firmware's own verdict is `ok` and
its numbers agree: status as expected, latched equal to commits, delta as
expected, and for a scene that was presented, `presented yes` and releases as
expected (a refused scene must print `-`). A
`SCENE n name: commits changed while held` line adds a note and makes the
row `FAIL`.

Exit 2, with a message and no table, for a malformed `SCENE` or `RESULT` line, a
scene that is not one of the eight, the same scene twice, a missing scene, or
a missing `RESULT` line. Exit 1 for a table with a `FAIL` row or `RESULT FAIL`.

## Test

```
sh scripts/am62p-scenes.test.sh
```

Draws a synthetic picture of each scene from the firmware's integer geometry
(upside down, with a colour error), checks it against the tool's EXPECT with
`camera-check.py` (including 1280×800), checks that a wrong picture fails, and
runs `summary` on `testdata/am62p-scenes/host-run.log` (the host run's console
of the board test doc, on a 96×48 fake panel) and on variants of it: prefixed
lines, a failed scene, a missing scene, malformed lines and so on.
