#!/usr/bin/env python3
"""Rectify a camera picture of the Verdin iMX95 panel into panel pixels.

    python3 rectify.py PICTURE OUT.jpg QUAD_JSON

The camera sees the panel upside down (180 degrees) and under strong
perspective, with part of the panel outside the frame, so the bilinear corner
mapping of scripts/camera-check.py does not fit it. This script rotates the
picture by 180 degrees, then applies a projective warp so that OUT is the
panel at 1280x800 (panel pixel coordinates; parts outside the camera frame
are black). camera-check then runs on OUT with corners at the image corners
and rotation 0.

QUAD_JSON holds {"frame_size": [w, h], "quad": [[x, y] x4], "panel_rect":
[x0, y0, x1, y1]}: the four corners TL TR BR BL, in the 180-degree rotated
camera picture of size frame_size (scaled to the input's size), of a
rectangle whose position on the panel is known from the commit log
(panel_rect, panel pixels). It was measured once on the two-planes picture
(red layer at 320,200 640x400) with the camera locked, see master-run.md.
Needs Pillow.
"""
import json
import sys

from PIL import Image

W, H = 1280, 800


def solve(src, dst):
    """Projective map src -> dst (four point pairs), as 8 coefficients."""
    a = []
    for (x, y), (u, v) in zip(src, dst):
        a.append([x, y, 1, 0, 0, 0, -u * x, -u * y, u])
        a.append([0, 0, 0, x, y, 1, -v * x, -v * y, v])
    n = 8
    for i in range(n):
        p = max(range(i, n), key=lambda r: abs(a[r][i]))
        a[i], a[p] = a[p], a[i]
        for r in range(n):
            if r != i:
                k = a[r][i] / a[i][i]
                a[r] = [x - k * y for x, y in zip(a[r], a[i])]
    return [a[i][n] / a[i][i] for i in range(n)]


def main(argv):
    if len(argv) != 4:
        print(__doc__, file=sys.stderr)
        return 2
    spec = json.load(open(argv[3]))
    x0, y0, x1, y1 = spec["panel_rect"]
    panel = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    im = Image.open(argv[1]).convert("RGB").rotate(180)
    # The quad is in pixels of a frame of frame_size; scale it to this picture
    # (the committed *-raw.jpg copies are downscaled).
    fw, fh = spec.get("frame_size", im.size)
    sx, sy = im.size[0] / fw, im.size[1] / fh
    quad = [(x * sx, y * sy) for x, y in spec["quad"]]
    # PIL wants the map from output (panel) pixels to input pixels.
    coeffs = solve(panel, quad)
    out = im.transform((W, H), Image.Transform.PERSPECTIVE, coeffs,
                       Image.Resampling.BILINEAR)
    out.save(argv[2], quality=88)
    print(f"rectified {argv[1]} -> {argv[2]} ({W}x{H})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
