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
   `"panel": "full"` (or `--panel full` on the command line, which overrides the
   file) takes the picture's own corners after rotation, the same as explicit corners
   `[[0, 0], [w-1, 0], [w-1, h-1], [0, h-1]]`. It is meant for KVM (HDMI capture)
   pictures, which show the whole screen edge to edge with no surround, and
   rotation 0 (`"rotation": 0` or `--rotation 0`; the default is 180 for the board
   camera). `--panel auto` forces auto detection.
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

## Using a KVM capture instead of a camera

Status: this section is a procedure, not a result. As of this writing no KVM
frame from the STM32MP257F-DK has been checked with `camera-check.py`; the
capture there did not work and the script ran only its self-test
(see [`docs/observed/stm32mp257-kvm.md`](../docs/observed/stm32mp257-kvm.md)).
Nothing below says a KVM frame passed or failed.

On boards with a NanoKVM (HDMI capture) the picture of the physical output is a
frame of the HDMI signal, not a photograph of a panel. It shows the screen edge
to edge, with no perspective, no surround, no camera blur and no room light. It
is scaled to the KVM's capture size and may carry compression artifacts.

**Passing a frame.** Save the frame as a file and pass it like a camera picture:
`python3 scripts/camera-check.py frame.png expect.json --panel full --rotation 0`.
PNG and PPM load without Pillow. A JPEG frame (the usual KVM stream format)
needs Pillow, or convert it to PNG first. If the KVM serves only a stream, save
one frame per phase; the file names go into the report with the verdict. Take
the frame while the harness holds the pattern, and note the host UTC time.

**Options that suit a direct capture.**

- `--panel full` (or `"panel": "full"` in the file): the frame's own corners.
  Do not use `auto` on a full frame. It needs a dark surround and a lit
  background, and a KVM frame has neither.
- `--rotation 0` (or `"rotation": 0`): the default of 180 is for the board
  camera that sees the panel upside down. A KVM frame is upright.
- Regions are fractions of the frame, so the capture size does not matter: the
  same `rect` values work at any scale. Scaling and compression blur plane
  edges, so keep regions well inside one plane's area, away from edges, and
  on solid areas (for example the middle of a quadrant), not on thin lines or
  text. The tighter edge margin of a direct capture allows regions closer to
  the edge than for a camera, but not on it.
- Tolerance: the capture has no room light or camera colour response, so
  solid colours should land near the commanded value. Compression and a
  possible limited-range conversion in the KVM still shift them. Start from
  the camera tolerance (60), look at the distance column of a first run on a
  known solid pattern, and only then tighten. Do not tighten before a
  calibration frame of the same KVM exists. Record the measured colours of
  that frame; do not assume them.
- For `--compare` (move tests) the KVM does not move, so the same-corner rule
  holds trivially. A `changed` threshold of 60 or more is still a good choice
  while compression noise is unmeasured.

**What a capture cannot show.**

- The panel itself. The sink is the KVM, so panel timing, backlight, the
  panel's own colour response, DSI or LVDS signalling and anything after the
  HDMI encoder are not covered. A KVM frame tells what the display pipeline
  sent to its HDMI output, not what a panel would show.
- Which connector it came from, if the board has more than one output. State
  in the log which connector the harness drove.
- Exact pixel values. Scaling and compression make the picture a check of
  colour regions and layout, not a bit-exact readback.
- That one plane per layer was used. As above, a picture alone cannot exclude
  a flattened buffer.

**The picture-and-log rule stays.** A KVM frame is the visual half of the
evidence only. Keep the harness log (plane assignment, commit status, latched
or presented counts) with each frame, as in
[`docs/m3-am62p-test-plan.md`](../docs/m3-am62p-test-plan.md) section 3. A pass
needs a log that shows the intended planes and a verdict `PASS` for the frame; a
picture without the log, or a log without the picture, is not a pass.

**Open points (the script is not changed here).**

- The script has no crop option. A KVM frame that includes a border or an
  overlay outside the screen area (for example a letterboxed capture, or a
  capture size with a different aspect ratio than the mode) would need
  explicit corners, which works today in pixels or `frac`. Whether a KVM frame
  needs that is not known until one is captured.
- If the KVM scales a mode to its own capture size, region fractions still hold
  but the scaling filter is unmeasured.
- The script does not report the frame size or the colour range assumption.
  If a KVM turns out to give limited-range RGB, an optional range
  conversion would be a code change; it is not needed or proposed until a
  calibration frame shows the offset.
- Self-test coverage of `--panel full` exists on synthetic frames only; it is not
  evidence about a real KVM.

## Notes

- Without Pillow the PNG decoder is pure Python and slow: a full-resolution camera
  PNG can take tens of seconds. Install Pillow or downscale first.
- The self-test runs every JSON example of this file on synthetic pictures of the
  layouts described, with a black panel background.

- Colours are 8-bit RGB, distances are Euclidean in RGB (max about 441).
- `--json` prints the full result for the report.
- The first line of output for each region is the evidence to paste into
  `docs/m1-imx95-report.md`, together with the picture file names.
