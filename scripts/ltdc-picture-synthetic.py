#!/usr/bin/env python3
"""Draw a synthetic panel picture (binary PPM) of plane_commit's `quadrants`
pattern for ltdc-picture-synthetic.test.sh. Standard library only.

  ltdc-picture-synthetic.py OUT.ppm --mode WxH --plane X,Y,WxH [--src WxH]
        [--transform T] [--nv12] [--encoding bt601|bt709]
        [--range limited|full]

The picture is drawn pixel by pixel (buffer turned counter-clockwise, then
reflected, then nearest-neighbour scaled onto the plane, black elsewhere),
independently of the position arithmetic in ltdc-picture-expect.py.
Synthetic pictures are test fixtures, never hardware observations.
"""
import sys

QUAD = [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 255)]
K = {"bt601": (1.402, 0.344136, 0.714136, 1.772),
     "bt709": (1.5748, 0.187324, 0.468124, 1.8556)}


def yuv_limited_601(c):
    r, g, b = c
    return (16 + 0.257 * r + 0.504 * g + 0.098 * b,
            128 - 0.148 * r - 0.291 * g + 0.439 * b,
            128 + 0.439 * r - 0.368 * g - 0.071 * b)


def to_rgb(y, cb, cr, enc, rng):
    rcr, gcb, gcr, bcb = K[enc]
    yy = (y - 16) * 255.0 / 219.0 if rng == "limited" else y
    v = (yy + rcr * (cr - 128), yy - gcb * (cb - 128) - gcr * (cr - 128),
         yy + bcb * (cb - 128))
    return tuple(max(0, min(255, int(round(x)))) for x in v)


def buffer_image(w, h, nv12, enc, rng):
    cols = list(QUAD)
    if nv12:
        cols = [to_rgb(*yuv_limited_601(c), enc, rng) for c in QUAD]
    return [[cols[(y >= h // 2) * 2 + (x >= w // 2)] for x in range(w)]
            for y in range(h)]


def ccw(img):
    h, w = len(img), len(img[0])
    return [[img[y][w - 1 - x] for y in range(h)] for x in range(w)]


def transform(img, text):
    toks = text.split("+")
    for t in toks:
        if t in ("90", "180", "270"):
            for _ in range(int(t) // 90):
                img = ccw(img)
    if "reflect-x" in toks:
        img = [row[::-1] for row in img]
    if "reflect-y" in toks:
        img = img[::-1]
    return img


def wh(s):
    a, b = s.split("x")
    return int(a), int(b)


def main(argv):
    out, opt, i = argv[0], {}, 1
    while i < len(argv):
        if argv[i] == "--nv12":
            opt["nv12"] = True
            i += 1
        else:
            opt[argv[i][2:]] = argv[i + 1]
            i += 2
    mw, mh = wh(opt["mode"])
    p = opt["plane"].split(",")
    px, py = int(p[0]), int(p[1])
    pw, ph = wh(p[2])
    sw, sh = wh(opt["src"]) if "src" in opt else (pw, ph)
    img = buffer_image(sw, sh, opt.get("nv12", False),
                       opt.get("encoding", "bt601"),
                       opt.get("range", "limited"))
    img = transform(img, opt.get("transform", "0"))
    ih, iw = len(img), len(img[0])
    rows = []
    for y in range(mh):
        row = bytearray(3 * mw)
        if py <= y < py + ph:
            for x in range(px, px + pw):
                c = img[(y - py) * ih // ph][(x - px) * iw // pw]
                row[3 * x:3 * x + 3] = bytes(c)
        rows.append(bytes(row))
    with open(out, "wb") as f:
        f.write(b"P6\n%d %d\n255\n" % (mw, mh) + b"".join(rows))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
