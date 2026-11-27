# camera-check

Host checker for camera pictures of a panel showing the two-plane test patterns
(SoW M1 condition 3: two hardware planes visible at once, z-order, alpha).
Python 3, standard library only. Pillow is used to decode if installed (any
format); without it only 8-bit PNG and binary PPM (P6) load, and anything else
stops with an error telling you to install Pillow or convert to PNG. No network.

```
python3 scripts/camera-check.py PICTURE EXPECT.json [--compare PICTURE2] [--rotation N] [--json]
python3 scripts/camera-check.py --self-test
```

Exit status: 0 = `VERDICT: PASS`, 1 = `VERDICT: FAIL`, 2 = usage or input error.
The self-test builds synthetic pictures in memory (including the 180-degree
rotation) and prints `PASS`.

## What it does

1. Rotates the picture clockwise by `rotation` (default 180, the board camera's
   picture is upside down). `--rotation` overrides the file.
2. Finds the panel: `"panel": "auto"` (default) takes the bright area, i.e. pixels
   above the midpoint of the darkest and brightest luminance, and uses its extreme
   points along the diagonals as TL, TR, BR, BL. Or give the corners explicitly.
   **Corners are in the rotated (upright) picture**, order TL, TR, BR, BL, in pixels
   (`"unit": "px"`, default) or as fractions 0..1 of the picture (`"unit": "frac"`).
   Auto detection needs a dark surround and a panel whose *background is lit*
   (brighter than the midpoint of darkest and brightest pixel). With a black panel
   background, or patterns with black or dark areas at the panel edge, it finds the
   plane union instead of the panel: use explicit corners. All examples below do,
   with pixel corners of the 260x140 synthetic picture the self-test uses; replace
   them by the panel corners read from your picture (after rotation).
3. For each region (`rect` = `[u0, v0, u1, v1]` in panel-normalised coordinates,
   u right, v down, 0..1) it averages a 16x16 grid of samples and prints the mean
   colour, the Euclidean RGB distance to `expect` and PASS/FAIL (`tolerance`,
   default 40 from the top level, overridable per region).
4. With `--compare PICTURE2` the same regions are measured in both pictures and
   each `compare` entry asserts `"expect": "changed"` (distance >= `threshold`) or
   `"same"` (distance < `threshold`), default threshold 30. Optional
   `regions_after` are checked against expected colours in the second picture.
   The camera must not move between the two pictures; the second picture is always
   measured with the corners found in the first (auto detection runs on the first
   picture only).

Regions without `expect` are only measured, for use in `compare`. Keep regions
inside one plane's area, away from edges (camera blur, rotation errors).
Use colours measured from a calibration picture of the same camera if possible;
the camera's colour response is not sRGB-exact, so choose saturated, well-separated
colours and a generous tolerance, and check the distance column for margin.

## Example: two overlapping solid planes, top colour in the overlap

Plane A (red, bottom plane) covers u 0.10..0.60, v 0.30..0.90. Plane B (blue, top
plane) covers u 0.40..0.90, v 0.10..0.70. The overlap is u 0.40..0.60, v 0.30..0.70
and must show blue.

```json
{
  "rotation": 180,
  "panel": {"corners": [[30, 20], [229, 20], [229, 119], [30, 119]]},
  "tolerance": 60,
  "regions": [
    {"name": "A_only",  "rect": [0.15, 0.75, 0.30, 0.85], "expect": [220, 20, 20]},
    {"name": "B_only",  "rect": [0.70, 0.15, 0.85, 0.30], "expect": [20, 20, 220]},
    {"name": "overlap", "rect": [0.45, 0.35, 0.55, 0.65], "expect": [20, 20, 220]},
    {"name": "outside", "rect": [0.02, 0.02, 0.08, 0.08], "expect": [0, 0, 0]}
  ]
}
```

## Example: swapped z-order

Same layout with A above B: the overlap must now be red. Only that expectation
changes.

```json
{
  "rotation": 180,
  "panel": {"corners": [[30, 20], [229, 20], [229, 119], [30, 119]]},
  "tolerance": 60,
  "regions": [
    {"name": "A_only",  "rect": [0.15, 0.75, 0.30, 0.85], "expect": [220, 20, 20]},
    {"name": "B_only",  "rect": [0.70, 0.15, 0.85, 0.30], "expect": [20, 20, 220]},
    {"name": "overlap", "rect": [0.45, 0.35, 0.55, 0.65], "expect": [220, 20, 20]},
    {"name": "outside", "rect": [0.02, 0.02, 0.08, 0.08], "expect": [0, 0, 0]}
  ]
}
```

## Example: 50% plane alpha

B (blue) has plane alpha 50% over A (red). With premultiplied or straight alpha
the overlap is `0.5*B + 0.5*A` = (120, 20, 120). Over the bare background B alone
is `0.5*B` = (10, 10, 110). Both depend on the driver's blend mode, so a failure
here may mean a different blend formula (record the measured colour).
The overlap tolerance of 50 separates a 50% blend from either opaque colour
(distance about 140 from each).

```json
{
  "rotation": 180,
  "panel": {"corners": [[30, 20], [229, 20], [229, 119], [30, 119]]},
  "tolerance": 60,
  "regions": [
    {"name": "A_only",  "rect": [0.15, 0.75, 0.30, 0.85], "expect": [220, 20, 20]},
    {"name": "B_only",  "rect": [0.70, 0.15, 0.85, 0.30], "expect": [10, 10, 110]},
    {"name": "overlap", "rect": [0.45, 0.35, 0.55, 0.65], "expect": [120, 20, 120], "tolerance": 50}
  ]
}
```

## Example: move test (two pictures)

Move one plane between pictures while the other plane's buffer is unchanged.
Here B moved right by 0.05 of the panel width; the strip B uncovers/covers at
u 0.91..0.94 must change and the A-only region must not.

```json
{
  "rotation": 180,
  "panel": {"corners": [[30, 20], [229, 20], [229, 119], [30, 119]]},
  "regions": [
    {"name": "A_only",  "rect": [0.15, 0.75, 0.30, 0.85]},
    {"name": "B_edge",  "rect": [0.91, 0.20, 0.94, 0.60]}
  ],
  "compare": [
    {"region": "A_only", "expect": "same",    "threshold": 30},
    {"region": "B_edge", "expect": "changed", "threshold": 60}
  ]
}
```

```
python3 scripts/camera-check.py before.png move.json --compare after.png
```

Note that this only shows what is on the panel. That a single flattened framebuffer
was not used is shown by the commit's plane assignment and by the producers
updating only one buffer; the picture is the visual half of that evidence.
Take the two pictures with the same camera position and exposure.

## Notes

- Without Pillow the PNG decoder is pure Python and slow: a full-resolution camera
  PNG can take tens of seconds. Install Pillow or downscale first.
- The self-test runs every JSON example of this file on synthetic pictures of the
  layouts described, with a black panel background.

- Colours are 8-bit RGB, distances are Euclidean in RGB (max about 441).
- `--json` prints the full result for the report.
- The first line of output for each region is the evidence to paste into
  `docs/m1-imx95-report.md`, together with the picture file names.
