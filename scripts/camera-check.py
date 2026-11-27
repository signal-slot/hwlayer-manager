#!/usr/bin/env python3
"""camera-check: check a camera picture of a display panel against expected colours.

Used for SoW M1 condition 3 (two hardware planes visible at once, z-order, alpha).
Standard library only; Pillow is used for decoding when installed (any format),
otherwise only 8-bit PNG and binary PPM (P6) can be read. No network access.

  camera-check.py PICTURE EXPECT.json [--compare PICTURE2] [--rotation N] [--panel full|auto] [--json]
  camera-check.py --self-test

Exit status: 0 = VERDICT PASS, 1 = VERDICT FAIL, 2 = usage or input error.
See scripts/camera-check-README.md.
"""
import copy
import json
import math
import os
import re
import struct
import sys
import zlib

SAMPLES = 16  # sample grid per region side


class CheckError(Exception):
    pass


# ---------------------------------------------------------------- images
class Img:
    """8-bit RGB image, rows top to bottom, data = bytes of r,g,b,..."""

    def __init__(self, w, h, data):
        if len(data) != w * h * 3:
            raise CheckError("image data size mismatch")
        self.w, self.h, self.data = w, h, bytes(data)

    def px(self, x, y):
        i = (y * self.w + x) * 3
        d = self.data
        return (d[i], d[i + 1], d[i + 2])

    def rotated(self, deg):
        """Rotate clockwise by deg (0, 90, 180, 270)."""
        deg %= 360
        d, w, h = self.data, self.w, self.h
        if deg == 0:
            return self
        if deg == 180:
            b = bytearray(len(d))
            b[0::3], b[1::3], b[2::3] = d[-3::-3], d[-2::-3], d[-1::-3]
            return Img(w, h, b)
        if deg in (90, 270):
            out = bytearray()
            xs = range(w) if deg == 90 else range(w - 1, -1, -1)
            for x in xs:
                # 90 cw: new row x is old column x read bottom to top;
                # 270 cw: new row is old column w-1-i read top to bottom
                col = [d[c + 3 * x::3 * w] for c in range(3)]
                if deg == 90:
                    col = [c[::-1] for c in col]
                row = bytearray(3 * h)
                row[0::3], row[1::3], row[2::3] = col
                out += row
            return Img(h, w, out)
        raise CheckError("rotation must be 0, 90, 180 or 270")


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def decode_png(raw):
    if raw[:8] != b"\x89PNG\r\n\x1a\n":
        raise CheckError("not a PNG")
    pos, idat, hdr = 8, b"", None
    while pos + 8 <= len(raw):
        n, typ = struct.unpack(">I4s", raw[pos:pos + 8])
        body = raw[pos + 8:pos + 8 + n]
        pos += 12 + n
        if typ == b"IHDR":
            hdr = struct.unpack(">IIBBBBB", body)
        elif typ == b"IDAT":
            idat += body
        elif typ == b"IEND":
            break
    if hdr is None:
        raise CheckError("PNG without IHDR")
    w, h, depth, ctype, _, _, interlace = hdr
    if depth != 8 or interlace or ctype not in (0, 2, 4, 6):
        raise CheckError("stdlib PNG reader handles only 8-bit, non-interlaced, "
                         "gray/RGB(A); install Pillow for other files")
    ch = {0: 1, 2: 3, 4: 2, 6: 4}[ctype]
    stride = w * ch
    data = zlib.decompress(idat)
    if len(data) < h * (stride + 1):
        raise CheckError("truncated PNG")
    prev = bytearray(stride)
    out = bytearray()
    p = 0
    for _ in range(h):
        f = data[p]
        line = bytearray(data[p + 1:p + 1 + stride])
        p += stride + 1
        for i in range(stride):
            a = line[i - ch] if i >= ch else 0
            b = prev[i]
            c = prev[i - ch] if i >= ch else 0
            if f == 1:
                line[i] = (line[i] + a) & 255
            elif f == 2:
                line[i] = (line[i] + b) & 255
            elif f == 3:
                line[i] = (line[i] + ((a + b) >> 1)) & 255
            elif f == 4:
                line[i] = (line[i] + _paeth(a, b, c)) & 255
            elif f != 0:
                raise CheckError("bad PNG filter")
        prev = line
        if ch in (3, 4):
            for x in range(w):
                out += line[x * ch:x * ch + 3]
        else:
            for x in range(w):
                out += bytes((line[x * ch],) * 3)
    return Img(w, h, out)


def encode_png(img):
    rows = b"".join(b"\x00" + img.data[y * img.w * 3:(y + 1) * img.w * 3]
                    for y in range(img.h))

    def chunk(t, b):
        return (struct.pack(">I", len(b)) + t + b +
                struct.pack(">I", zlib.crc32(t + b) & 0xFFFFFFFF))
    return (b"\x89PNG\r\n\x1a\n" +
            chunk(b"IHDR", struct.pack(">IIBBBBB", img.w, img.h, 8, 2, 0, 0, 0)) +
            chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def decode_ppm(raw):
    toks, pos = [], 0
    while len(toks) < 4:
        while raw[pos:pos + 1].isspace():
            pos += 1
        if raw[pos:pos + 1] == b"#":
            while raw[pos:pos + 1] not in (b"\n", b""):
                pos += 1
            continue
        s = pos
        while raw[pos:pos + 1] and not raw[pos:pos + 1].isspace():
            pos += 1
        toks.append(raw[s:pos])
    pos += 1
    if toks[0] != b"P6" or int(toks[3]) != 255:
        raise CheckError("only 8-bit binary PPM (P6) is supported")
    w, h = int(toks[1]), int(toks[2])
    return Img(w, h, raw[pos:pos + w * h * 3])


def load_image(path):
    try:
        from PIL import Image  # optional
    except ImportError:
        Image = None
    if Image is not None:
        try:
            im = Image.open(path).convert("RGB")
            return Img(im.width, im.height, im.tobytes())
        except OSError as e:
            raise CheckError("cannot read %s: %s" % (path, e))
    try:
        with open(path, "rb") as f:
            raw = f.read()
    except OSError as e:
        raise CheckError("cannot read %s: %s" % (path, e))
    if raw[:8] == b"\x89PNG\r\n\x1a\n":
        return decode_png(raw)
    if raw[:2] == b"P6":
        return decode_ppm(raw)
    raise CheckError("%s: Pillow is not installed and the file is not PNG/PPM; "
                     "install Pillow (python3 -m pip install pillow) or convert "
                     "the picture to PNG" % path)


# ---------------------------------------------------------------- geometry
def auto_corners(img):
    """Corners (TL, TR, BR, BL) of the bright area: pixels above the midpoint
    between darkest and brightest luminance; extreme points along the diagonals."""
    step = max(1, max(img.w, img.h) // 400)
    pts = []
    lo, hi = 1e9, -1.0
    for y in range(0, img.h, step):
        for x in range(0, img.w, step):
            r, g, b = img.px(x, y)
            lum = 0.299 * r + 0.587 * g + 0.114 * b
            pts.append((x, y, lum))
            lo, hi = min(lo, lum), max(hi, lum)
    if hi - lo < 20:
        raise CheckError("auto panel detection: picture has no bright area")
    thr = (lo + hi) / 2
    br = [(x, y) for x, y, lum in pts if lum >= thr]
    tl = min(br, key=lambda p: p[0] + p[1])
    brc = max(br, key=lambda p: p[0] + p[1])
    tr = max(br, key=lambda p: p[0] - p[1])
    bl = min(br, key=lambda p: p[0] - p[1])
    return [tl, tr, brc, bl]


def panel_corners(img, spec):
    if spec in (None, "auto"):
        return auto_corners(img)
    if spec == "full":  # KVM picture: the screen edge to edge
        return [(0, 0), (img.w - 1, 0), (img.w - 1, img.h - 1), (0, img.h - 1)]
    if isinstance(spec, dict):
        pts = spec.get("corners")
        unit = spec.get("unit", "px")
    else:
        pts, unit = spec, "px"
    if not isinstance(pts, list) or len(pts) != 4:
        raise CheckError("panel needs 4 corners [TL, TR, BR, BL], \"auto\" or \"full\"")
    out = []
    for p in pts:
        x, y = float(p[0]), float(p[1])
        if unit == "frac":
            x, y = x * (img.w - 1), y * (img.h - 1)
        elif unit != "px":
            raise CheckError("panel unit must be px or frac")
        out.append((x, y))
    return out


def map_point(c, u, v):
    """Bilinear map of panel-normalised (u right, v down) into the picture."""
    (x0, y0), (x1, y1), (x2, y2), (x3, y3) = c
    tx, ty = x0 + (x1 - x0) * u, y0 + (y1 - y0) * u
    bx, by = x3 + (x2 - x3) * u, y3 + (y2 - y3) * u
    return tx + (bx - tx) * v, ty + (by - ty) * v


def mean_colour(img, corners, rect):
    u0, v0, u1, v1 = rect
    if not (0 <= u0 < u1 <= 1 and 0 <= v0 < v1 <= 1):
        raise CheckError("region rect must satisfy 0<=u0<u1<=1, 0<=v0<v1<=1: %r" % (rect,))
    acc, n = [0, 0, 0], 0
    for j in range(SAMPLES):
        v = v0 + (v1 - v0) * (j + 0.5) / SAMPLES
        for i in range(SAMPLES):
            u = u0 + (u1 - u0) * (i + 0.5) / SAMPLES
            x, y = map_point(corners, u, v)
            xi, yi = int(round(x)), int(round(y))
            if not (0 <= xi < img.w and 0 <= yi < img.h):
                raise CheckError("region lies outside the picture (check panel corners)")
            p = img.px(xi, yi)
            acc[0] += p[0]
            acc[1] += p[1]
            acc[2] += p[2]
            n += 1
    return tuple(a / n for a in acc)


def dist(a, b):
    return math.sqrt(sum((p - q) ** 2 for p, q in zip(a, b)))


# ---------------------------------------------------------------- checks
def prepare(img, exp, rotation, corners=None):
    rot = exp.get("rotation", 180) if rotation is None else rotation
    img = img.rotated(int(rot))
    return img, corners or panel_corners(img, exp.get("panel", "auto"))


def measure(img, corners, regions):
    return {r["name"]: mean_colour(img, corners, r["rect"]) for r in regions}


def check_regions(regions, means, default_tol):
    rows = []
    for r in regions:
        if "expect" not in r:
            continue
        m = means[r["name"]]
        tol = float(r.get("tolerance", default_tol))
        d = dist(m, r["expect"])
        rows.append(dict(name=r["name"], measured=m, expected=r["expect"],
                         distance=d, tolerance=tol, ok=d <= tol))
    return rows


def run(exp, img_a, img_b=None, rotation=None):
    regions = exp.get("regions", [])
    names = [r["name"] for r in regions]
    if len(set(names)) != len(names):
        raise CheckError("duplicate region names")
    tol = float(exp.get("tolerance", 40))
    a, ca = prepare(img_a, exp, rotation)
    ma = measure(a, ca, regions)
    results = {"regions": check_regions(regions, ma, tol)}
    if img_b is not None:
        b, _ = prepare(img_b, exp, rotation, corners=ca)
        cb = ca
        mb = measure(b, cb, regions)
        rb = exp.get("regions_after")
        if rb:
            mb_after = {r["name"]: mean_colour(b, cb, r["rect"]) for r in rb}
            results["regions_after"] = check_regions(rb, mb_after, tol)
        cmp_rows = []
        for c in exp.get("compare", []):
            nm = c["region"]
            if nm not in ma:
                raise CheckError("compare refers to unknown region %r" % nm)
            d = dist(ma[nm], mb[nm])
            thr = float(c.get("threshold", 30))
            want = c.get("expect", "changed")
            if want not in ("changed", "same"):
                raise CheckError("compare expect must be changed or same")
            ok = d >= thr if want == "changed" else d < thr
            cmp_rows.append(dict(name=nm, expect=want, distance=d,
                                 threshold=thr, ok=ok, a=ma[nm], b=mb[nm]))
        results["compare"] = cmp_rows
    elif exp.get("compare"):
        raise CheckError("expectation has \"compare\" but no second picture was given")
    rows = results["regions"] + results.get("regions_after", []) + results.get("compare", [])
    if not rows:
        raise CheckError("expectation file defines nothing to check")
    results["pass"] = all(r["ok"] for r in rows)
    return results


def fmt(c):
    return "(%5.1f,%5.1f,%5.1f)" % tuple(c)


def report(res):
    lines = []
    for key in ("regions", "regions_after"):
        for r in res.get(key, []):
            lines.append("%-5s %s%-16s measured %s expected %s distance %6.1f tol %5.1f" % (
                "PASS" if r["ok"] else "FAIL", "after:" if key == "regions_after" else "",
                r["name"], fmt(r["measured"]), fmt(r["expected"]), r["distance"], r["tolerance"]))
    for r in res.get("compare", []):
        lines.append("%-5s compare %-16s want %-7s distance %6.1f threshold %5.1f  A %s B %s" % (
            "PASS" if r["ok"] else "FAIL", r["name"], r["expect"], r["distance"],
            r["threshold"], fmt(r["a"]), fmt(r["b"])))
    lines.append("VERDICT: " + ("PASS" if res["pass"] else "FAIL"))
    return "\n".join(lines)


# ---------------------------------------------------------------- self-test
def _synth(panel, bg=(10, 10, 10), margin=(30, 20), rot180=True, pbg=(60, 60, 60)):
    """panel: list of (u0,v0,u1,v1,colour), later entries on top, drawn on an
    upright 200x100 panel inside a dark picture; rotated by 180 if asked."""
    pw, ph = 200, 100
    w, h = pw + 2 * margin[0], ph + 2 * margin[1]
    px = [[bg] * w for _ in range(h)]
    for y in range(ph):
        for x in range(pw):
            col = pbg  # panel background
            for u0, v0, u1, v1, c in panel:
                if u0 * pw <= x < u1 * pw and v0 * ph <= y < v1 * ph:
                    col = c
            px[y + margin[1]][x + margin[0]] = col
    img = Img(w, h, bytes(v for row in px for p in row for v in p))
    return img.rotated(180) if rot180 else img


def check_readme_examples(check):
    """Run every JSON block of the README on synthetic pictures of the layouts
    it describes, with a black panel background."""
    here = os.path.dirname(os.path.abspath(__file__))
    with open(os.path.join(here, "camera-check-README.md")) as f:
        blocks = re.findall(r"```json\n(.*?)```", f.read(), re.S)
    check(len(blocks) == 4, "README has four JSON examples")
    ex = [json.loads(b) for b in blocks]
    RED, BLUE = (220, 20, 20), (20, 20, 220)
    A = (0.10, 0.30, 0.60, 0.90, RED)
    B = (0.40, 0.10, 0.90, 0.70, BLUE)
    blk = dict(pbg=(0, 0, 0))
    check(run(ex[0], _synth([A, B], **blk))["pass"], "README example 1 (B on top)")
    check(not run(ex[0], _synth([B, A], **blk))["pass"], "README example 1 rejects swapped")
    check(run(ex[1], _synth([B, A], **blk))["pass"], "README example 2 (A on top)")
    check(not run(ex[1], _synth([A, B], **blk))["pass"], "README example 2 rejects B on top")
    Bh = (0.40, 0.10, 0.90, 0.70, (10, 10, 110))
    mix = (0.40, 0.30, 0.60, 0.70, (120, 20, 120))
    check(run(ex[2], _synth([A, Bh, mix], **blk))["pass"], "README example 3 (50% alpha)")
    check(not run(ex[2], _synth([A, B], **blk))["pass"], "README example 3 rejects opaque")
    Bm = (0.45, 0.10, 0.95, 0.70, BLUE)
    res = run(ex[3], _synth([A, B], **blk), _synth([A, Bm], **blk))
    check(res["pass"] and len(res["compare"]) == 2, "README example 4 (move test)")
    check(not run(ex[3], _synth([A, B], **blk), _synth([A, B], **blk))["pass"],
          "README example 4 rejects unchanged picture")


def self_test():
    n_ok = [0]

    def check(cond, what):
        if not cond:
            print("FAIL: " + what)
            raise SystemExit(1)
        n_ok[0] += 1

    RED, BLUE, GREEN = (220, 20, 20), (20, 20, 220), (20, 200, 20)
    A = (0.1, 0.3, 0.6, 0.9, RED)    # lower-left
    B = (0.4, 0.1, 0.9, 0.7, BLUE)   # upper-right, overlaps A
    corners = {"corners": [[30, 20], [229, 20], [229, 119], [30, 119]]}
    regions = [
        {"name": "A_only", "rect": [0.15, 0.75, 0.3, 0.85], "expect": RED},
        {"name": "B_only", "rect": [0.7, 0.15, 0.85, 0.3], "expect": BLUE},
        {"name": "overlap", "rect": [0.45, 0.35, 0.55, 0.65], "expect": BLUE},
    ]
    exp = {"rotation": 180, "tolerance": 30, "panel": "auto", "regions": regions}
    pic = _synth([A, B])
    res = run(exp, pic)
    check(res["pass"], "B-on-top picture passes with rotation 180 / auto panel")
    check(all(r["distance"] < 5 for r in res["regions"]), "measured colours close")

    # explicit corners are in the picture after rotation: the panel is centred,
    # so the same box is valid for the 180-degree-rotated picture
    exp_c = dict(exp, panel=corners)
    check(run(exp_c, pic)["pass"], "explicit corners")
    check(not run(exp_c, pic, rotation=0)["pass"], "wrong rotation fails")
    check(run(exp_c, _synth([A, B], rot180=False), rotation=0)["pass"],
          "unrotated picture with rotation 0")

    swapped = _synth([B, A])  # A on top
    res = run(exp, swapped)
    ov = [r for r in res["regions"] if r["name"] == "overlap"][0]
    check(not res["pass"] and not ov["ok"], "swapped z-order detected in overlap")
    exp_sw = copy.deepcopy(exp)
    exp_sw["regions"][2]["expect"] = RED
    check(run(exp_sw, swapped)["pass"], "swapped expectation passes on swapped picture")

    # 50% alpha: blue over red, the overlap shows the mean of both
    mix = tuple((a + b) // 2 for a, b in zip(RED, BLUE))
    half = _synth([A, B, (0.4, 0.3, 0.6, 0.7, mix)])
    exp_a = copy.deepcopy(exp)
    exp_a["regions"][2]["expect"] = mix
    exp_a["regions"][2]["tolerance"] = 15
    check(run(exp_a, half)["pass"], "50% alpha blend colour")
    check(not run(exp_a, pic)["pass"], "opaque overlap is not a 50% blend")

    # two-picture compare: a strip at the right edge changes, A stays
    moved = _synth([A, B, (0.9, 0.1, 0.95, 0.7, GREEN)])
    exp_m = {
        "rotation": 180, "panel": "auto", "tolerance": 30,
        "regions": [{"name": "A_only", "rect": [0.15, 0.75, 0.3, 0.85]},
                    {"name": "B_edge", "rect": [0.91, 0.2, 0.94, 0.6]}],
        "compare": [{"region": "A_only", "expect": "same"},
                    {"region": "B_edge", "expect": "changed"}],
    }
    res = run(exp_m, pic, moved)
    check(res["pass"] and len(res["compare"]) == 2, "move test: B_edge changes, A same")
    exp_m2 = copy.deepcopy(exp_m)
    exp_m2["compare"][0]["expect"] = "changed"
    check(not run(exp_m2, pic, moved)["pass"], "move test fails when A expected changed")
    check(not run(exp_m, pic, pic)["pass"], "identical pictures: changed region fails")

    # auto detection is made on picture A only: B is measured with A's corners
    shifted = _synth([A, (0.0, 0.0, 1.0, 1.0, (250, 250, 250))])
    res = run(exp_m, pic, shifted)
    check(res["compare"][1]["b"][0] > 200, "compare reuses corners of picture A")

    # KVM picture: whole screen edge to edge, no dark surround, rotation 0
    full_pic = _synth([A, B], margin=(0, 0), rot180=False)
    exp_f = {"rotation": 0, "tolerance": 30, "panel": "full", "regions": regions}
    check(run(exp_f, full_pic)["pass"], "full-frame picture passes with panel full")
    exp_fw = copy.deepcopy(exp_f)
    exp_fw["regions"][0]["expect"] = GREEN
    check(not run(exp_fw, full_pic)["pass"], "full-frame picture with wrong colour fails")
    check(panel_corners(full_pic, "full") == panel_corners(full_pic, [[0, 0], [199, 0], [199, 99], [0, 99]]),
          "panel full equals explicit edge corners")

    check_readme_examples(check)

    back = decode_png(encode_png(pic))
    check(back.data == pic.data and back.w == pic.w, "PNG encode/decode round trip")
    check(pic.rotated(90).rotated(270).data == pic.data, "rotate 90 then 270 is identity")
    check(pic.rotated(180).rotated(180).data == pic.data, "rotate 180 twice is identity")
    check(pic.rotated(90).w == pic.h, "rotate 90 swaps dimensions")
    txt = report(run(exp, pic))
    check(txt.rstrip().endswith("VERDICT: PASS") and "overlap" in txt, "report text")
    try:
        run({"regions": []}, pic)
        check(False, "empty expectation must be an error")
    except CheckError:
        check(True, "empty expectation rejected")
    print("%d checks" % n_ok[0])
    print("PASS")


# ---------------------------------------------------------------- main
def main(argv):
    args = argv[1:]
    if "--self-test" in args:
        self_test()
        return 0
    pos, compare, rotation, as_json, panel = [], None, None, False, None
    it = iter(args)
    try:
        for a in it:
            if a == "--compare":
                compare = next(it)
            elif a == "--rotation":
                rotation = int(next(it))
            elif a == "--panel":
                panel = next(it)
                if panel not in ("auto", "full"):
                    raise CheckError("--panel must be auto or full")
            elif a == "--json":
                as_json = True
            elif a in ("-h", "--help"):
                print(__doc__)
                return 0
            elif a.startswith("-"):
                raise CheckError("unknown option " + a)
            else:
                pos.append(a)
        if len(pos) != 2:
            raise CheckError("usage: camera-check.py PICTURE EXPECT.json "
                             "[--compare PICTURE2] [--rotation N] [--panel full|auto] [--json] | --self-test")
        try:
            with open(pos[1]) as f:
                exp = json.load(f)
        except (OSError, ValueError) as e:
            raise CheckError("cannot read expectation %s: %s" % (pos[1], e))
        if panel:
            exp["panel"] = panel
        for r in exp.get("regions", []) + exp.get("regions_after", []):
            if "name" not in r or "rect" not in r:
                raise CheckError("each region needs name and rect")
        res = run(exp, load_image(pos[0]), load_image(compare) if compare else None, rotation)
    except (CheckError, StopIteration, ValueError) as e:
        print("error: %s" % (e or "bad arguments"), file=sys.stderr)
        return 2
    print(json.dumps(res, indent=1) if as_json else report(res))
    return 0 if res["pass"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
