# scenario-expect

Writes the `camera-check.py` EXPECT JSON for the picture a `tools/drm-harness`
scenario leaves on the screen, for the Linux board rows A1-A6 of
`docs/board-scenarios.md` (driven by `scripts/board-scenarios.sh`). Python 3,
standard library only, same style as `twoplane-expect.py`.

```
python3 scripts/scenario-expect.py --scenario NAME [--step STEP] [--after SCENARIO]
        [--phase before|after] [--modeset-compare changed|drawing]
        [--width W] [--height H] [--rotation N] [--tolerance N] [--threshold N]
        [--panel full|auto|corners] [--corners x,y,x,y,x,y,x,y]
python3 scripts/scenario-expect.py --list
sh scripts/scenario-expect.test.sh
```

The JSON is printed to stdout. Exit status 0 = ok, 2 = input error.

## Scenarios

Colours and rectangles are read from `tools/drm-harness/src/draw.rs` (`colour`)
and `scenarios.rs`; the comments in the script name the function for each.

| `--scenario` | picture at hold time | regions |
|---|---|---|
| `two-planes` | `two_planes()`: BLUE full screen with white frame; RED (w/2 x h/2) with yellow frame in the middle | blue corner, red centre |
| `zorder-alpha` | `zorder_alpha()`, four held steps, picked with `--step` (default `a-over-b`): `a-over-b`, `b-over-a`, `plane-alpha-50`, `pixel-alpha-premultiplied` | a only, b only, overlap, black outside |
| `move` | `move_top()`: GREY/BLACK checkerboard, yellow rectangle moving across the middle band | the checker above and below the band (mean grey 48) |
| `rotate` | `rotate()`: BLUE background, RED rectangle, a 32 px white bar sweeping | wide blue and red regions |
| `revoke` | `revoke()`: as `rotate`, frozen at the last frame after the revoke | same as `rotate` |
| `combos` | `combos()`: only TEST_ONLY, nothing is committed | `compare` **same** against the picture before the run |
| `stop` | after the harness stopped: no pattern | regions of `--after` (default `two-planes`), `compare` **changed** against the hold picture |
| `modeset` | `modeset()`: `modeset-before` and `modeset-after` both draw the two-planes picture, on the original and on the new mode | `--modeset-compare changed` (default): as `stop`, `compare` **changed** against the picture before the switch, no colours. `drawing`: the `two-planes` colours, no compare |
| `recovery` | `recovery()`: `recovery-before` draws the two-planes picture (`TWO_PLANES`); the new run after the loss draws `RECOVERED`: GREEN full screen, YELLOW rectangle with BLUE frame, same layout. `--phase before` or `after` (default) picks the picture | colour regions: blue/red (before) or green/yellow (after) |

`move`, `rotate` and `revoke` have no `--seconds` hold: the picture is taken
during the run, so give them a `--frames` large enough to last (the bar and the
rectangle move, so only static areas are judged; the tolerance for the bar
scenarios grows as `--width` shrinks). `--width`/`--height` (default 1920x1080)
only move the margins, which keep three frame widths (24 px) from every edge.

The `changed` entries use threshold 60 (`--threshold`); each listed region must
differ, so after the stop the screen must not show the pattern's colours there.
For `modeset`, `changed` assumes the picture differs between the modes. A sink
that scales both modes to the full screen shows the same picture twice, so use
`--modeset-compare drawing`: the two-planes colours must be seen in the picture
taken in each mode (judge both pictures with the same file, no `--compare`).
`--after` only applies to `stop`, `--phase` only to `recovery` and
`--modeset-compare` only to `modeset`.

## Using it

The default `--rotation` is 0 (KVM pictures; camera-check's own default for a
camera is 180) and the default panel is the whole picture, as a KVM shows the
screen only. Use `--panel corners` for a camera, or `auto` if the panel
background is lit; patterns with a black background need explicit corners.

```
python3 scripts/scenario-expect.py --scenario two-planes > e.json
python3 scripts/camera-check.py hold.png e.json
python3 scripts/scenario-expect.py --scenario stop > s.json
python3 scripts/camera-check.py hold.png s.json --compare stopped.png
```

Colours are the colours the harness writes; the display and capture chain may
shift them, so check the distance column and record the measured colour if a
board needs a wider `--tolerance`. Plane alpha and pixel alpha rows depend on
the driver's blend formula (see `camera-check-README.md`).
