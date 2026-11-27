#!/usr/bin/env python3
"""Turn twoplane EXPECT lines into camera-check EXPECT.json files.

  twoplane-expect.py [TWOPLANE_OUTPUT|-] --out DIR [--size F] [--tolerance N]
                     [--rotation N] [--panel full|corners] [--corners x,y,...]

Reads twoplane stdout (file or stdin) and writes DIR/expect-<phase>.json for
every phase that ran. A phase is skipped when one of its EXPECT lines carries
the token `skipped`, or (for `swapped`) when a `SWAPZ skipped` line is present.
Skipped phases get no file and are listed on stdout.
Exit status: 0 = ok, 2 = input error. Python 3, standard library only.
"""
import json
import os
import re
import sys

EXPECT = re.compile(r'^EXPECT\s+phase=(\S+)\s+"([^"]*)"(.*)$')
FIELD = re.compile(r'\b(x|y|fx|fy|rgb)=(\S+)')


class InputError(Exception):
    pass


def parse(text):
    """Return (phases, skipped): phases maps name -> samples, in order."""
    phases, skipped = {}, set()
    for line in text.splitlines():
        line = line.strip()
        if re.match(r'^SWAPZ skipped\b', line):
            skipped.add("swapped")
            continue
        m = EXPECT.match(line)
        if not m:
            m = re.match(r'^EXPECT\s+phase=(\S+)(.*)$', line)
            if m and "skipped" in m.group(2).split():
                skipped.add(m.group(1))
            continue
        phase, name, rest = m.groups()
        if "skipped" in rest.split():
            skipped.add(phase)
            continue
        f = dict(FIELD.findall(rest))
        try:
            fx, fy = float(f["fx"]), float(f["fy"])
            rgb = f["rgb"]
            if not re.fullmatch(r"#[0-9a-fA-F]{6}", rgb):
                raise ValueError(rgb)
            col = [int(rgb[i:i + 2], 16) for i in (1, 3, 5)]
        except (KeyError, ValueError):
            raise InputError("bad EXPECT line: " + line)
        phases.setdefault(phase, []).append((name, fx, fy, col))
    for p in skipped:
        phases.pop(p, None)
    return phases, skipped


def build(samples, size, tol, rotation, panel):
    h = size / 2
    regions = []
    for name, fx, fy, col in samples:
        u0, v0 = max(0.0, fx - h), max(0.0, fy - h)
        u1, v1 = min(1.0, fx + h), min(1.0, fy + h)
        regions.append({"name": name,
                        "rect": [round(u0, 4), round(v0, 4),
                                 round(u1, 4), round(v1, 4)],
                        "expect": col})
    return {"rotation": rotation, "panel": panel, "tolerance": tol,
            "regions": regions}


def main(argv):
    out = src = None
    size, tol, rotation, mode, corners = 0.02, 40, 0, "full", None
    i = 0
    try:
        while i < len(argv):
            a = argv[i]
            if a in ("--out", "--size", "--tolerance", "--rotation",
                     "--panel", "--corners"):
                i += 1
                v = argv[i]
                if a == "--out":
                    out = v
                elif a == "--size":
                    size = float(v)
                elif a == "--tolerance":
                    tol = float(v)
                elif a == "--rotation":
                    rotation = int(v)
                elif a == "--panel":
                    mode = v
                else:
                    corners = [float(x) for x in v.split(",")]
            elif (a.startswith("--") and a != "-") or src is not None:
                raise InputError("unknown argument " + a)
            else:
                src = a
            i += 1
    except (IndexError, ValueError):
        raise InputError("bad or missing option value")
    if out is None:
        raise InputError("usage: twoplane-expect.py [FILE|-] --out DIR "
                         "[--size F] [--tolerance N] [--rotation N] "
                         "[--panel full|corners] [--corners x,y,x,y,x,y,x,y]")
    if not 0 < size <= 1:
        raise InputError("--size must be in (0, 1]")
    if mode == "full":
        panel = {"corners": [[0, 0], [1, 0], [1, 1], [0, 1]], "unit": "frac"}
    elif mode == "corners":
        if corners is None or len(corners) != 8:
            raise InputError("--panel corners needs --corners "
                             "x,y,x,y,x,y,x,y (TL,TR,BR,BL, fractions)")
        panel = {"corners": [corners[k:k + 2] for k in range(0, 8, 2)],
                 "unit": "frac"}
    else:
        raise InputError("--panel must be full or corners")
    try:
        if src in (None, "-"):
            text = sys.stdin.read()
        else:
            with open(src, encoding="utf-8", errors="replace") as f:
                text = f.read()
    except OSError as e:
        raise InputError("cannot read input: %s" % e)
    phases, skipped = parse(text)
    if not phases:
        raise InputError("no EXPECT lines for a phase that ran")
    try:
        os.makedirs(out, exist_ok=True)
        for phase, samples in phases.items():
            path = os.path.join(out, "expect-%s.json" % phase)
            with open(path, "w") as f:
                json.dump(build(samples, size, tol, rotation, panel), f,
                          indent=1)
                f.write("\n")
            print("wrote %s (%d regions)" % (path, len(samples)))
    except OSError as e:
        raise InputError("cannot write output: %s" % e)
    for p in sorted(skipped):
        print("skipped phase %s" % p)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except InputError as e:
        print("twoplane-expect: %s" % e, file=sys.stderr)
        sys.exit(2)
