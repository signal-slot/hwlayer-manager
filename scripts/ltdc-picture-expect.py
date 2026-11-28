#!/usr/bin/env python3
"""Predict camera-check EXPECT.json files for STM32MP257 LTDC pictures.

  ltdc-picture-expect.py --mode WxH --plane X,Y,WxH [--src WxH]
        [--transform T] [--nv12 | --yuv Y,Cb,Cr]
        [--encoding bt601|bt709] [--range limited|full]
        [--size F] [--tolerance N] [--rotation N] [--name NAME] [--out DIR]

Everything written is a PREDICTION (inferred / read from source), never an
observation: see ltdc-picture-expect-README.md.

Three kinds of expectation:
  (default)      plane_commit `pattern=quadrants` in an RGB buffer: one region
                 at the centre of each buffer quadrant after the transform.
  --nv12         the same quadrants in an NV12 buffer. plane_commit converts
                 the quadrant colours to BT.601 limited Y,Cb,Cr; the LTDC then
                 converts back with --encoding/--range (default bt601 limited).
  --yuv Y,Cb,Cr one flat region at the plane centre: the colour the LTDC model
                 gives for that Y,Cb,Cr under --encoding/--range.
--transform is `0`, `90`, `180`, `270`, `reflect-x`, `reflect-y` joined with
`+` (as plane_commit's rotation=). With --out DIR the file is
DIR/expect-NAME.json (NAME default `ltdc`); without it the JSON goes to stdout.
The panel is the whole picture (KVM capture), camera rotation default 0.
Exit status: 0 = ok, 2 = input error. Python 3, standard library only.
"""
import json
import os
import re
import sys

# plane_commit.rs QUADRANTS (tl, tr, bl, br) and QUADRANT_NAMES.
QUADRANTS = [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 255)]
NAMES = ["tl", "tr", "bl", "br"]

# ltdc.c ltdc_ycbcr2rgb_coeffs [encoding][range]: (CYR0R, CYR1R) words, read
# from source. CYR0R = b_cb<<16 | r_cr, CYR1R = g_cb<<16 | g_cr, all /256.
COEFFS = {
    ("bt601", "limited"): (0x02040199, 0x006400D0),
    ("bt601", "full"): (0x01C60167, 0x005800B7),
    ("bt709", "limited"): (0x021D01CB, 0x00370089),
    ("bt709", "full"): (0x01DB0193, 0x00300078),
}


class InputError(Exception):
    pass


def yuv_of(rgb):
    """plane_commit.rs `yuv`: integer BT.601 limited-range (Y, Cb, Cr)."""
    r, g, b = rgb
    y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16
    cb = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128
    cr = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128
    return y, cb, cr


def yuv_to_rgb(y, cb, cr, enc, rng):
    """The model of docs/stm32mp257-ltdc-yuv-source.md (inferred): luma
    rescaled by 255/219 when limited (YREN), chroma factors from the
    coefficient words, clamp, round to nearest. +-2 of rounding is expected."""
    w0, w1 = COEFFS[(enc, rng)]
    b_cb, r_cr = (w0 >> 16) / 256.0, (w0 & 0xFFFF) / 256.0
    g_cb, g_cr = (w1 >> 16) / 256.0, (w1 & 0xFFFF) / 256.0
    yy = (y - 16) * 255.0 / 219.0 if rng == "limited" else float(y)
    r = yy + r_cr * (cr - 128)
    g = yy - (g_cb * (cb - 128) + g_cr * (cr - 128))
    b = yy + b_cb * (cb - 128)
    return [max(0, min(255, int(v + 0.5))) for v in (r, g, b)]


def parse_wh(text, what):
    m = re.fullmatch(r"(\d+)x(\d+)", text)
    if not m or int(m.group(1)) < 1 or int(m.group(2)) < 1:
        raise InputError("bad %s %r: use WxH" % (what, text))
    return int(m.group(1)), int(m.group(2))


def parse_plane(text):
    m = re.fullmatch(r"(-?\d+),(-?\d+),(\d+)x(\d+)", text)
    if not m or int(m.group(3)) < 1 or int(m.group(4)) < 1:
        raise InputError("bad --plane %r: use X,Y,WxH" % text)
    return tuple(int(g) for g in m.groups())


def parse_transform(text):
    """Return (angle, reflect_x, reflect_y); angle in 0/90/180/270."""
    angle, rx, ry, seen = 0, False, False, set()
    angles = ("0", "90", "180", "270")
    for tok in text.split("+"):
        if tok in seen:
            raise InputError("repeated transform %r" % tok)
        if tok in angles:
            if any(t in angles for t in seen):
                raise InputError("two angles in --transform %r" % text)
            angle = int(tok)
        elif tok == "reflect-x":
            rx = True
        elif tok == "reflect-y":
            ry = True
        else:
            raise InputError("bad transform %r: use 0, 90, 180, 270, "
                             "reflect-x, reflect-y joined with +" % tok)
        seen.add(tok)
    return angle, rx, ry


def quadrant_pos(quadrant, plane, transform):
    """Centre of buffer quadrant on the screen, in pixels: the same mapping
    as plane_commit.rs quadrant_screen_pos (turn counter-clockwise by the
    angle, then reflect), in quarters of the dst size."""
    angle, rx, ry = transform
    a, b = 1 + 2 * (quadrant & 1), 1 + 2 * (quadrant >> 1)
    if angle == 90:
        a, b = b, 4 - a
    elif angle == 180:
        a, b = 4 - a, 4 - b
    elif angle == 270:
        a, b = 4 - b, a
    if rx:
        a = 4 - a
    if ry:
        b = 4 - b
    x, y, w, h = plane
    return x + a * w / 4.0, y + b * h / 4.0


def build(args):
    mw, mh = args["mode"]
    px, py, pw, ph = args["plane"]
    if px < 0 or py < 0 or px + pw > mw or py + ph > mh:
        raise InputError("--plane must lie inside the mode %dx%d" % (mw, mh))
    half = args["size"] / 2

    def region(name, cx, cy, col):
        u, v = cx / mw, cy / mh
        return {"name": name,
                "rect": [round(max(0.0, u - half), 4),
                         round(max(0.0, v - half), 4),
                         round(min(1.0, u + half), 4),
                         round(min(1.0, v + half), 4)],
                "expect": col}

    regions = []
    desc = []
    if args["yuv"] is not None:
        y, cb, cr = args["yuv"]
        col = yuv_to_rgb(y, cb, cr, args["encoding"], args["range"])
        regions.append(region("flat Y=%d Cb=%d Cr=%d" % (y, cb, cr),
                              px + pw / 2.0, py + ph / 2.0, col))
        desc.append("flat YCbCr %d,%d,%d as %s %s"
                    % (y, cb, cr, args["encoding"], args["range"]))
    else:
        t = args["transform"]
        for q in range(4):
            col = list(QUADRANTS[q])
            if args["nv12"]:
                col = yuv_to_rgb(*yuv_of(QUADRANTS[q]), args["encoding"],
                                 args["range"])
            cx, cy = quadrant_pos(q, args["plane"], t)
            regions.append(region("buffer %s at %d,%d" % (NAMES[q], cx, cy),
                                  cx, cy, col))
        desc.append("quadrants%s, transform %s" % (
            " in NV12 as %s %s" % (args["encoding"], args["range"])
            if args["nv12"] else "", args["transform_text"]))
    note = ("PREDICTION (inferred / read from source), not an observation: "
            + "; ".join(desc))
    if args["src"] and args["src"] != (pw, ph):
        note += "; scaled %dx%d -> %dx%d" % (args["src"] + (pw, ph))
    return {"prediction": note, "rotation": args["rotation"],
            "panel": {"corners": [[0, 0], [1, 0], [1, 1], [0, 1]],
                      "unit": "frac"},
            "tolerance": args["tolerance"], "regions": regions}


USAGE = ("usage: ltdc-picture-expect.py --mode WxH --plane X,Y,WxH "
         "[--src WxH] [--transform T] [--nv12 | --yuv Y,Cb,Cr] "
         "[--encoding bt601|bt709] [--range limited|full] [--size F] "
         "[--tolerance N] [--rotation N] [--name NAME] [--out DIR]")


def parse_args(argv):
    a = {"mode": None, "plane": None, "src": None, "transform_text": "0",
         "nv12": False, "yuv": None, "encoding": None, "range": None,
         "size": 0.02, "tolerance": 40.0, "rotation": 0, "name": "ltdc",
         "out": None}
    takes = {"--mode", "--plane", "--src", "--transform", "--yuv",
             "--encoding", "--range", "--size", "--tolerance", "--rotation",
             "--name", "--out"}
    i = 0
    try:
        while i < len(argv):
            o = argv[i]
            if o == "--nv12":
                a["nv12"] = True
            elif o in takes:
                i += 1
                v = argv[i]
                k = o[2:]
                if k in ("mode", "src"):
                    a[k] = parse_wh(v, o)
                elif k == "plane":
                    a[k] = parse_plane(v)
                elif k == "transform":
                    a["transform_text"] = v
                elif k == "yuv":
                    t = [int(x) for x in v.split(",")]
                    if len(t) != 3 or any(not 0 <= x <= 255 for x in t):
                        raise ValueError(v)
                    a[k] = tuple(t)
                elif k in ("size", "tolerance"):
                    a[k] = float(v)
                elif k == "rotation":
                    a[k] = int(v)
                else:
                    a[k] = v
            else:
                raise InputError("unknown argument " + o)
            i += 1
    except IndexError:
        raise InputError("missing value for " + argv[-1])
    except ValueError:
        raise InputError("bad value for " + argv[i - 1])
    if a["mode"] is None or a["plane"] is None:
        raise InputError(USAGE)
    if a["encoding"] not in (None, "bt601", "bt709"):
        raise InputError("--encoding must be bt601 or bt709")
    if a["range"] not in (None, "limited", "full"):
        raise InputError("--range must be limited or full")
    yuv_mode = a["nv12"] or a["yuv"] is not None
    if not yuv_mode and (a["encoding"] or a["range"]):
        raise InputError("--encoding/--range apply to --nv12 or --yuv only")
    a["encoding"] = a["encoding"] or "bt601"
    a["range"] = a["range"] or "limited"
    if not 0 < a["size"] <= 1:
        raise InputError("--size must be in (0, 1]")
    if a["rotation"] not in (0, 90, 180, 270):
        raise InputError("--rotation must be 0, 90, 180 or 270")
    if not re.fullmatch(r"[a-z0-9_-]+", a["name"]):
        raise InputError("bad --name: use [a-z0-9_-]")
    if a["nv12"] and a["yuv"] is not None:
        raise InputError("--nv12 and --yuv exclude each other")
    if a["yuv"] is not None and a["transform_text"] != "0":
        raise InputError("--transform applies to quadrants, not to --yuv")
    if a["nv12"]:
        for what, wh in (("--plane", a["plane"][2:]), ("--src", a["src"])):
            if wh and (wh[0] % 2 or wh[1] % 2):
                raise InputError("NV12 needs even sizes (%s)" % what)
    a["transform"] = parse_transform(a["transform_text"])
    return a


def main(argv):
    a = parse_args(argv)
    if a["transform"][0] in (90, 270):
        print("ltdc-picture-expect: note: 90/270 are not offered on overlay "
              "planes, and a committed 90/270 on this board is predicted to "
              "reach a null pointer (inferred); do not commit it",
              file=sys.stderr)
    exp = build(a)
    text = json.dumps(exp, indent=1) + "\n"
    if a["out"] is None:
        sys.stdout.write(text)
        return 0
    try:
        os.makedirs(a["out"], exist_ok=True)
        path = os.path.join(a["out"], "expect-%s.json" % a["name"])
        with open(path, "w") as f:
            f.write(text)
    except OSError as e:
        raise InputError("cannot write output: %s" % e)
    print("wrote %s (%d regions, prediction)" % (path, len(exp["regions"])))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except InputError as e:
        print("ltdc-picture-expect: %s" % e, file=sys.stderr)
        sys.exit(2)
