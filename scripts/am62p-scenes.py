#!/usr/bin/env python3
"""The AM62P R5F board test's eight scenes: camera-check EXPECT and console summary.

  am62p-scenes.py expect --scene N|NAME --width W --height H [--tolerance N]
                  [--rotation N] [--panel full|corners] [--corners x,y,...]
  am62p-scenes.py summary CONSOLE|-

expect prints the camera-check.py EXPECT JSON for one scene on stdout.
summary reads a console capture (file or stdin) with the firmware's SCENE and
RESULT lines and prints a Markdown table and the RESULT line.
Exit status: 0 = ok, 1 = summary found a failed scene or RESULT FAIL,
2 = usage or input error (malformed line, missing scene, missing RESULT).
Python 3, standard library only.

The scenes come from tests/board/am62p_r5f/src/scenes.c and scenes.h
(work/am62p-board-test); the line numbers in the comments below are those of
that file at commit 361e31f.
"""
import json
import re
import sys

# scenes.h SCENES_SAFETY_RED 0xFFFF0000 (ARGB8888, opaque), SCENES_CLEAR
# 0x00000000 (premultiplied transparent: shows what is below).
RED = [255, 0, 0]
# scenes.h SCENES_APP_BLUE 0x001F, SCENES_APP_GREEN 0x07E0 (RGB565): a full
# 5 or 6 bit channel, expanded to 8 bits, is 255 (the camera shows about 248).
BLUE = [0, 0, 255]
GREEN = [0, 255, 0]
# The controller's background: black expected (docs/board-test-am62p-r5f.md).
BLACK = [0, 0, 0]

# (name, application plane colour or None for off, safety layer)
# The safety layer is "full" (red all over) or "left" / "right" (a red
# rectangle on an otherwise clear plane). Scenes 6..8 are refused: the panel
# stays as scene 5 left it (scenes.c scene(): no commit is written).
SCENES = {
    # scenes.c:320-322  B: fill_safety(full) red; application plane off
    1: ("safety-only", None, "full"),
    # scenes.c:327-331  A: clear, red rectangle left (drawn() 289-290: SAFETY_A);
    #                   APP_1 blue (drawn() 293: SCENES_APP_BLUE)
    2: ("app-under-safety", BLUE, "left"),
    # scenes.c:334-336  APP_2 green (drawn() 296); the safety layer is still A
    3: ("app-replaced", GREEN, "left"),
    # scenes.c:339-341  B again, begin_write: clear, rectangle right
    #                   (drawn() 286-287: SAFETY_B, again -> SCENES_RECT_RIGHT)
    4: ("safety-flipped", GREEN, "right"),
    # scenes.c:344      layer 0 only (B): the application plane is off, so the
    #                   panel shows the controller background under the rectangle
    5: ("app-off", None, "right"),
    # scenes.c:348-353  refused: nothing written, panel as in scene 5
    6: ("refused-no-safety", None, "right"),
    7: ("refused-blank", None, "right"),
    8: ("refused-foreign-safety", None, "right"),
}
NAMES = {v[0]: k for k, v in SCENES.items()}


class InputError(Exception):
    pass


def rect_frac(side):
    """The rectangle of the safety layer in panel fractions.

    scenes.c:66-74 scenes_rect(): width/3, height/2, y = height/4,
    x = width/12 (left) or width - width/12 - width/3 (right). In fractions
    of the panel (the integer division of the firmware is below the margin
    taken in region()): u 1/12..5/12 (left), 7/12..11/12 (right), v 1/4..3/4.
    """
    if side == "left":
        return (1 / 12, 0.25, 5 / 12, 0.75)
    return (7 / 12, 0.25, 11 / 12, 0.75)


def shrink(r, f=0.25):
    """The inner part of a rectangle: cut f of its size off every side, so
    that camera blur and the firmware's integer rounding stay out."""
    u0, v0, u1, v1 = r
    du, dv = (u1 - u0) * f, (v1 - v0) * f
    return [round(u0 + du, 4), round(v0 + dv, 4),
            round(u1 - du, 4), round(v1 - dv, 4)]


def regions(scene):
    name, app, safety = SCENES[scene]
    under = app if app is not None else BLACK
    out = []
    if safety == "full":
        out.append(("red all over, centre", shrink((0, 0, 1, 1), 0.3), RED))
        out.append(("red all over, left", shrink((0, 0, 0.3, 1), 0.2), RED))
        out.append(("red all over, right", shrink((0.7, 0, 1, 1), 0.2), RED))
        return out
    mine, other = rect_frac(safety), rect_frac("left" if safety == "right" else "right")
    out.append(("red rectangle " + safety, shrink(mine), RED))
    out.append(("%s under the clear plane" % ("application" if app else "background"),
               shrink(other), under))
    # Above and below the rectangle, across the whole width (v 0..1/4, 3/4..1).
    out.append(("above the rectangle", shrink((0.05, 0.0, 0.95, 0.25), 0.2), under))
    out.append(("below the rectangle", shrink((0.05, 0.75, 0.95, 1.0), 0.2), under))
    return out


def expect(scene, tolerance, rotation, mode, corners):
    if mode == "full":
        panel = {"corners": [[0, 0], [1, 0], [1, 1], [0, 1]], "unit": "frac"}
    else:
        if corners is None or len(corners) != 8:
            raise InputError("--panel corners needs --corners "
                             "x,y,x,y,x,y,x,y (TL,TR,BR,BL, fractions of the "
                             "rotated picture)")
        panel = {"corners": [corners[k:k + 2] for k in range(0, 8, 2)],
                 "unit": "frac"}
    return {"rotation": rotation, "panel": panel, "tolerance": tolerance,
            "regions": [{"name": n, "rect": r, "expect": c}
                        for n, r, c in regions(scene)]}


def scene_number(s):
    if s in NAMES:
        return NAMES[s]
    try:
        n = int(s)
    except ValueError:
        n = 0
    if n not in SCENES:
        raise InputError("unknown scene %r (1..8 or one of %s)"
                         % (s, ", ".join(SCENES[k][0] for k in sorted(SCENES))))
    return n


def cmd_expect(argv):
    opts = {"--scene": None, "--width": None, "--height": None,
            "--tolerance": "60", "--rotation": "180", "--panel": "full",
            "--corners": None}
    i = 0
    while i < len(argv):
        a = argv[i]
        if a not in opts:
            raise InputError("unknown argument " + a)
        i += 1
        if i >= len(argv):
            raise InputError("missing value for " + a)
        opts[a] = argv[i]
        i += 1
    for k in ("--scene", "--width", "--height"):
        if opts[k] is None:
            raise InputError("usage: am62p-scenes.py expect --scene N|NAME "
                             "--width W --height H [--tolerance N] "
                             "[--rotation N] [--panel full|corners] [--corners x,y,...]")
    try:
        w, h = int(opts["--width"]), int(opts["--height"])
        tol = float(opts["--tolerance"])
        tol = int(tol) if tol == int(tol) else tol
        rot = int(opts["--rotation"])
        corners = (None if opts["--corners"] is None
                   else [float(x) for x in opts["--corners"].split(",")])
    except ValueError:
        raise InputError("bad number in an option")
    if w <= 0 or h <= 0:
        raise InputError("--width and --height must be positive")
    if tol <= 0:
        raise InputError("--tolerance must be positive")
    if rot not in (0, 90, 180, 270):
        raise InputError("--rotation must be 0, 90, 180 or 270")
    if opts["--panel"] not in ("full", "corners"):
        raise InputError("--panel must be full or corners")
    # The panel size is only checked, not needed: the regions are fractions.
    # The firmware's rectangle (scenes.c scenes_rect) must fit the panel.
    if w < 12 or h < 4:
        raise InputError("panel too small for the scenes")
    d = expect(scene_number(opts["--scene"]), tol, rot, opts["--panel"], corners)
    json.dump(d, sys.stdout, indent=1)
    sys.stdout.write("\n")
    return 0


# scenes.c scene(): the format of the SCENE line (env->log call, line 190).
SCENE = re.compile(
    r"\bSCENE (\d+) ([a-z0-9-]+): status (-?\d+) \(expected (-?\d+)\), "
    r"commits (\d+) latched (\d+) \(\+(\d+), expected \+(\d+)\), "
    r"presented (yes|NO|-), released (\d+) \(expected (\d+)\): (ok|FAIL)\s*$")
# scenes.c:201 scene(): the hold check, and RESULT lines (scenes_run()).
HELD = re.compile(r"\bSCENE (\d+) ([a-z0-9-]+): commits changed while held: "
                  r"(\d+) -> (\d+): FAIL\s*$")
RESULT = re.compile(r"\bRESULT (PASS|FAIL) \((.*)\)\s*$")


def parse(text):
    scenes, held, result = {}, {}, None
    for no, line in enumerate(text.splitlines(), 1):
        if re.search(r"\bSCENE \d", line):
            m = SCENE.search(line)
            h = HELD.search(line)
            if m:
                n = int(m.group(1))
                if n not in SCENES or SCENES[n][0] != m.group(2):
                    raise InputError("line %d: unknown scene: %s" % (no, line.strip()))
                if n in scenes:
                    raise InputError("line %d: scene %d twice" % (no, n))
                scenes[n] = m.groups()[2:]
            elif h:
                n = int(h.group(1))
                if n not in SCENES or SCENES[n][0] != h.group(2):
                    raise InputError("line %d: unknown scene: %s" % (no, line.strip()))
                held[n] = (h.group(3), h.group(4))
            else:
                raise InputError("line %d: malformed SCENE line: %s" % (no, line.strip()))
        elif re.search(r"\bRESULT\b", line):
            m = RESULT.search(line)
            if not m or result is not None:
                raise InputError("line %d: malformed or second RESULT line: %s"
                                 % (no, line.strip()))
            result = m.group(1), m.group(2)
    missing = [n for n in SCENES if n not in scenes]
    if missing:
        raise InputError("missing scene%s %s" % ("s" if len(missing) > 1 else "",
                                                 ", ".join(map(str, missing))))
    if result is None:
        raise InputError("no RESULT line")
    return scenes, held, result


def cmd_summary(argv):
    if len(argv) != 1:
        raise InputError("usage: am62p-scenes.py summary CONSOLE|-")
    try:
        if argv[0] == "-":
            text = sys.stdin.read()
        else:
            with open(argv[0], encoding="utf-8", errors="replace") as f:
                text = f.read()
    except OSError as e:
        raise InputError("cannot read input: %s" % e)
    scenes, held, result = parse(text)
    print("| scene | status (expected) | commits | latched | delta (expected) "
          "| presented | released (expected) | verdict |")
    print("| --- | --- | --- | --- | --- | --- | --- | --- |")
    failed = 0
    for n in sorted(SCENES):
        st, ex, c, lt, d, ed, pr, rel, erel, word = scenes[n]
        # The line's own verdict, and what its numbers say.
        good = (word == "ok" and st == ex and lt == c and d == ed
                and n not in held
                and (pr == "-" if st != "0" else pr == "yes" and rel == erel))
        failed += not good
        verdict = "ok" if good else "FAIL"
        if n in held:
            verdict += " (commits changed while held: %s -> %s)" % held[n]
        print("| %d %s | %s (%s) | %s | %s | +%s (+%s) | %s | %s (%s) | %s |"
              % (n, SCENES[n][0], st, ex, c, lt, d, ed, pr, rel, erel, verdict))
    print()
    print("RESULT %s (%s)" % result)
    return 1 if failed or result[0] != "PASS" else 0


def main(argv):
    if not argv or argv[0] not in ("expect", "summary"):
        raise InputError("usage: am62p-scenes.py expect|summary ...")
    return (cmd_expect if argv[0] == "expect" else cmd_summary)(argv[1:])


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except InputError as e:
        print("am62p-scenes: %s" % e, file=sys.stderr)
        sys.exit(2)
