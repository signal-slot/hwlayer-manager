#!/usr/bin/env python3
"""Print a camera-check EXPECT JSON for the picture of a drm-harness scenario.

  scenario-expect.py --scenario NAME [--step STEP] [--after SCENARIO]
                     [--width W] [--height H] [--rotation N] [--tolerance N]
                     [--threshold N] [--panel full|auto|corners]
                     [--corners x,y,...]
  scenario-expect.py --list

Scenarios of tools/drm-harness today: combos, two-planes, zorder-alpha, move,
rotate, revoke. Phases without a drawing of their own: stop, modeset, recovery
(a 'changed' compare against the hold picture, no colours). The JSON goes to
stdout; judge a picture with
  camera-check.py PICTURE EXPECT.json [--compare PICTURE2]
Exit status: 0 = ok, 2 = input error. Python 3, standard library only.

Every colour and rectangle below is read from tools/drm-harness/src; the
comments name the function. Rectangles are panel-normalised (u right, v down);
the whole screen is the panel, so the default panel is the full picture. Regions
keep three border widths (3 * 8 px) away from every edge of a drawn rectangle.
"""
import json
import sys

# tools/drm-harness/src/draw.rs, mod colour (0xAARRGGBB -> R, G, B).
BLUE = (0x00, 0x20, 0x80)
RED = (0xE0, 0x10, 0x10)
GREEN = (0x10, 0xC0, 0x10)
WHITE = (0xFF, 0xFF, 0xFF)
BLACK = (0x00, 0x00, 0x00)
GREY = (0x60, 0x60, 0x60)
# RED_HALF_PREMULTIPLIED 0x80700808: alpha 0x80, premultiplied (0x70, 0x08, 0x08).
RED_HALF = (0x70, 0x08, 0x08)

BORDER = 8      # scenarios.rs framed(): Op::Border thickness 8
BAND = 32       # scenarios.rs rotate()/revoke(): band_w = 32.min(width)
DEFAULT_TOL = 40
STEPS = ("a-over-b", "b-over-a", "plane-alpha-50", "pixel-alpha-premultiplied")


class InputError(Exception):
    pass


def mix(a, b, t):
    """t of a over (1 - t) of b, per channel."""
    return tuple(round(t * x + (1 - t) * y) for x, y in zip(a, b))


def two_planes(w, h):
    # scenarios.rs two_planes(): bottom = whole screen, BLUE, WHITE frame; top =
    # (w/2, h/2) at (w/4, h/4), RED, YELLOW frame, above the bottom plane.
    return [("blue bottom plane", [0.06, 0.06, 0.20, 0.20], BLUE),
            ("red top plane", [0.35, 0.35, 0.65, 0.65], RED)]


def zorder_alpha(w, h, step):
    # scenarios.rs zorder_alpha(): a and b are (w/2, h/2); a at (w/8, h/8) is RED
    # with a WHITE frame, b at (3w/8, 3h/8) is GREEN with a WHITE frame (a_alpha
    # has no frame). a covers u, v 0.125..0.625, b 0.375..0.875, the overlap is
    # 0.375..0.625; the screen is black elsewhere. m is three frame widths.
    mu, mv = 3.0 * BORDER / w, 3.0 * BORDER / h
    a_only = [0.125 + mu, 0.125 + mv, 0.625 - mu, 0.375 - mv]   # strip above b
    b_only = [0.625 + mu, 0.375 + mv, 0.875 - mu, 0.875 - mv]   # right of a
    overlap = [0.375 + mu, 0.375 + mv, 0.625 - mu, 0.625 - mv]
    black = ("black outside both", [0.02, 0.02, 0.09, 0.09], BLACK)
    green = ("green b only", b_only, GREEN)
    if step == "a-over-b":          # a z 1, b z 0: overlap is RED
        rows = [("red a only", a_only, RED), green,
                ("overlap is red", overlap, RED)]
    elif step == "b-over-a":        # a z 0, b z 1: overlap is GREEN
        rows = [("red a only", a_only, RED), green,
                ("overlap is green", overlap, GREEN)]
    elif step == "plane-alpha-50":  # a alpha 0x8000 over b
        rows = [("a at 50% over black", a_only, mix(RED, BLACK, .5)), green,
                ("a at 50% over green", overlap, mix(RED, GREEN, .5))]
    else:
        # pixel-alpha-premultiplied: a_alpha's left half (u 0.125..0.375) is
        # opaque RED, its right half (0.375..0.625) RED_HALF premultiplied; the
        # right half over b is RED + (1 - 0.5) * GREEN.
        rows = [("left half opaque red",
                 [0.125 + mu, 0.125 + mv, 0.375 - mu, 0.375 - mv], RED),
                ("right half over black",
                 [0.375 + mu, 0.125 + mv, 0.625 - mu, 0.375 - mv], RED_HALF),
                green,
                ("right half over green", overlap,
                 tuple(round(s + .5 * d) for s, d in zip(RED_HALF, GREEN)))]
    return rows + [black]


def move(w, h):
    # scenarios.rs move_top(): bottom = whole screen, Op::Checker 64 px cells
    # GREY / BLACK, WHITE frame; top = (w/4, h/4) YELLOW with RED frame, centred
    # vertically (v 0.375..0.625) and moving left to right during the run. Only
    # the checker outside that band is static; its mean is (GREY + BLACK) / 2.
    # The scenario has no hold: the picture is taken during the run, so use a
    # --frames value that makes it last.
    mean = mix(GREY, BLACK, .5)
    return [("checkerboard above the moving plane", [0.06, 0.06, 0.94, 0.30], mean),
            ("checkerboard below the moving plane", [0.06, 0.70, 0.94, 0.94], mean)]


def bars(w, h):
    # scenarios.rs rotate() / revoke(): bottom = whole screen BLUE, top = (w/2,
    # h/2) at (w/4, h/4) RED; each frame a BAND px wide WHITE bar, full height of
    # its buffer, at x = (n * 8) % (width - BAND + 1). The regions are wide so
    # that one bar moves the mean by at most BAND / region width of the way to
    # white; revoke freezes the last frame, which has the same colours.
    return [("blue bottom plane", [0.02, 0.03, 0.24, 0.22], BLUE),
            ("red top plane", [0.30, 0.30, 0.70, 0.70], RED)]


def bar_tolerance(w):
    """Worst case: a bar inside the narrowest region (0.22 of the width)."""
    # distance BLUE -> WHITE is sqrt(255^2 + 223^2 + 127^2), about 363.
    return max(DEFAULT_TOL, round(363 * BAND / (0.22 * w)) + 20)


COLOURED = ("two-planes", "zorder-alpha", "move", "rotate", "revoke")
SCENARIOS = ("combos",) + COLOURED + ("stop", "modeset", "recovery")


def pictures(name, step, w, h):
    if name == "two-planes":
        return two_planes(w, h)
    if name == "zorder-alpha":
        return zorder_alpha(w, h, step or STEPS[0])
    if name == "move":
        return move(w, h)
    return bars(w, h)


def probe_regions():
    """Measure-only regions for compares without a drawing: centre and corners."""
    return [("centre", [0.45, 0.45, 0.55, 0.55]),
            ("top left", [0.08, 0.08, 0.18, 0.18]),
            ("top right", [0.82, 0.08, 0.92, 0.18]),
            ("bottom left", [0.08, 0.82, 0.18, 0.92]),
            ("bottom right", [0.82, 0.82, 0.92, 0.92])]


def build(args):
    name = args["scenario"]
    w, h = args["width"], args["height"]
    tol, thr = args["tolerance"], args["threshold"]
    exp = {"rotation": args["rotation"], "panel": args["panel"]}
    if name in COLOURED:
        if tol is None:
            tol = bar_tolerance(w) if name in ("rotate", "revoke") else DEFAULT_TOL
        exp["tolerance"] = tol
        exp["regions"] = [{"name": n, "rect": [round(v, 4) for v in r],
                           "expect": list(c)}
                          for n, r, c in pictures(name, args["step"], w, h)]
        return exp
    if tol is not None:
        exp["tolerance"] = tol
    if name == "combos":
        # scenarios.rs combos(): only validate / TEST_ONLY, nothing is committed,
        # so a picture taken during the run equals the one taken before it.
        rows = probe_regions()
        want = "same"
        thr = 30 if thr is None else thr
    else:
        # stop (after the harness exits), modeset and recovery (scenarios the
        # harness does not have yet): the regions of the hold picture must differ
        # from the picture taken after the event. Colours are not stated.
        base = args["after"]
        if base in COLOURED:
            rows = [(n, r) for n, r, _ in pictures(base, args["step"], w, h)]
        else:
            rows = probe_regions()
        want = "changed"
        thr = 60 if thr is None else thr
    exp["regions"] = [{"name": n, "rect": [round(v, 4) for v in r]}
                      for n, r in rows]
    exp["compare"] = [{"region": n, "expect": want, "threshold": thr}
                      for n, _ in rows]
    return exp


def parse(argv):
    a = {"scenario": None, "step": None, "after": "two-planes", "width": 1920,
         "height": 1080, "rotation": 0, "tolerance": None, "threshold": None,
         "panel": "full", "corners": None}
    keys = {"--scenario": ("scenario", str), "--step": ("step", str),
            "--after": ("after", str), "--width": ("width", int),
            "--height": ("height", int), "--rotation": ("rotation", int),
            "--tolerance": ("tolerance", float),
            "--threshold": ("threshold", float), "--panel": ("panel", str),
            "--corners": ("corners", str)}
    i = 0
    try:
        while i < len(argv):
            k = argv[i]
            if k == "--list":
                return None
            if k not in keys:
                raise InputError("unknown argument " + k)
            i += 1
            dest, conv = keys[k]
            a[dest] = conv(argv[i])
            i += 1
    except (IndexError, ValueError):
        raise InputError("bad or missing option value")
    if a["scenario"] not in SCENARIOS:
        raise InputError("usage: scenario-expect.py --scenario NAME [--step S] "
                         "[--after SCENARIO] [--width W] [--height H] "
                         "[--rotation N] [--tolerance N] [--threshold N] "
                         "[--panel full|auto|corners] [--corners x,y,...] | "
                         "--list; scenarios: " + " ".join(SCENARIOS))
    if a["step"] is not None and a["step"] not in STEPS:
        raise InputError("--step must be one of " + ", ".join(STEPS))
    if a["after"] not in ("combos",) + COLOURED:
        raise InputError("--after must name a harness scenario")
    if a["width"] < 64 or a["height"] < 64:
        raise InputError("--width and --height must be at least 64")
    if a["rotation"] not in (0, 90, 180, 270):
        raise InputError("--rotation must be 0, 90, 180 or 270")
    if a["panel"] == "full":
        a["panel"] = {"corners": [[0, 0], [1, 0], [1, 1], [0, 1]], "unit": "frac"}
    elif a["panel"] == "corners":
        try:
            c = [float(x) for x in (a["corners"] or "").split(",")]
        except ValueError:
            c = []
        if len(c) != 8:
            raise InputError("--panel corners needs --corners "
                             "x,y,x,y,x,y,x,y (TL,TR,BR,BL, fractions)")
        a["panel"] = {"corners": [c[k:k + 2] for k in range(0, 8, 2)],
                      "unit": "frac"}
    elif a["panel"] != "auto":
        raise InputError("--panel must be full, auto or corners")
    return a


def main(argv):
    args = parse(argv)
    if args is None:
        for s in SCENARIOS:
            print(s)
        return 0
    json.dump(build(args), sys.stdout, indent=1)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except InputError as e:
        print("scenario-expect: %s" % e, file=sys.stderr)
        sys.exit(2)
