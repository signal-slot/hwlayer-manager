#!/bin/sh
# Check that scripts/am62p-scenes.py still matches the AM62P board test's
# scenes (tests/board/am62p_r5f/src/scenes.c and scenes.h).
#
#   sh scripts/check-am62p-scenes-sync.sh [ROOT]
#
# ROOT is a tree with tests/board/am62p_r5f/src/ and scripts/am62p-scenes.py
# (default: the repository this script is in). Nothing is changed.
# Per scene it prints "SYNC N ok" or "SYNC N differs: ...", then "PASS" or
# "FAIL: <reason>". Exit: 0 PASS, 1 FAIL, 2 usage error.
#
# From scenes.c it reads, by text matching: the scene() calls (number, name,
# status, layer count), the full_layer() assignments to l[0] / l[1], the draws
# in drawn() (which buffer gets which colour, red all over or a rectangle on
# which side) and the colour values in scenes.h. A refused scene (status not
# CE_OK) leaves the panel as the scene before it. From am62p-scenes.py it takes
# the names and the `expect` output (red region kind, colour under the clear
# plane) for scenes 1..8.

usage() {
	echo "usage: sh scripts/check-am62p-scenes-sync.sh [ROOT]" >&2
	exit 2
}

case $# in
0) root=$(cd "$(dirname "$0")/.." && pwd) ;;
1)
	case $1 in
	-*) usage ;;
	esac
	root=$1
	;;
*) usage ;;
esac
if [ ! -d "$root" ]; then
	echo "check-am62p-scenes-sync: not a directory: $root" >&2
	exit 2
fi
command -v python3 >/dev/null 2>&1 || {
	echo "check-am62p-scenes-sync: python3 not found" >&2
	exit 2
}

python3 -I - "$root" <<'PYEOF'
import importlib.util
import json
import os
import re
import subprocess
import sys

sys.dont_write_bytecode = True
root = sys.argv[1]
src = os.path.join(root, "tests", "board", "am62p_r5f", "src")
tool = os.path.join(root, "scripts", "am62p-scenes.py")


def finish(code, msg, lines=()):
    for l in lines:
        print(l)
    print(msg)
    sys.exit(code)


def read(p):
    try:
        with open(p, encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError as e:
        finish(1, "FAIL: cannot read %s: %s" % (p, e.strerror))


h = read(os.path.join(src, "scenes.h"))
c = read(os.path.join(src, "scenes.c"))


def define(name):
    m = re.search(r"^#define\s+%s\s+(0[xX][0-9A-Fa-f]+|\d+)u?\b" % name, h, re.M)
    if not m:
        finish(1, "FAIL: scenes.h has no #define %s" % name)
    return int(m.group(1), 0)


def rgb565(v):
    r, g, b = (v >> 11) & 31, (v >> 5) & 63, v & 31
    return [round(r * 255 / 31), round(g * 255 / 63), round(b * 255 / 31)]


def argb(v):
    return [(v >> 16) & 255, (v >> 8) & 255, v & 255]


red = argb(define("SCENES_SAFETY_RED"))
colour = {"SCENES_APP_BLUE": rgb565(define("SCENES_APP_BLUE")),
          "SCENES_APP_GREEN": rgb565(define("SCENES_APP_GREEN"))}
side_of = {"SCENES_RECT_LEFT": "left", "SCENES_RECT_RIGHT": "right"}
if define("SCENES_CLEAR") >> 24 != 0:
    finish(1, "FAIL: SCENES_CLEAR is not transparent")

# drawn(): what each buffer id is filled with.
m = re.search(r"\bdrawn\s*\([^)]*\)\s*\{(.*?)\n\}", c, re.S)
if not m:
    finish(1, "FAIL: scenes.c has no drawn() function")
body = m.group(1)
fills = {}  # id -> ("safety", full_expr, side) or ("app", colour name)
cur = []
for tok in re.finditer(r"case\s+(\w+)\s*:|default\s*:|"
                       r"fill_safety\s*\(\s*env\s*,\s*id\s*,\s*([^,]+?)\s*,\s*(\w+)\s*\)|"
                       r"fill_app\s*\(\s*env\s*,\s*id\s*,\s*(\w+)\s*\)", body):
    if tok.group(0).startswith("case"):
        cur = [tok.group(1)]
    elif tok.group(0).startswith("default"):
        cur = ["default"]
    elif tok.group(2) is not None:
        for i in cur:
            fills[i] = ("safety", tok.group(2), tok.group(3))
    else:
        for i in cur:
            fills[i] = ("app", tok.group(4))

# scenes_run(): the sequence of draws, layer assignments and scene() calls.
m = re.search(r"\bscenes_run\s*\([^)]*\)\s*\{(.*)\n\}", c, re.S)
if not m:
    finish(1, "FAIL: scenes.c has no scenes_run() function")
run = m.group(1)
ev = re.compile(
    r"drawn\s*\(\s*env\s*,\s*(\w+)\s*,\s*(true|false)\s*\)|"
    r"\bl\s*\[\s*(\d)\s*\]\s*=\s*full_layer\s*\(\s*(\w+)\s*,|"
    r"\bscene\s*\(\s*env\s*,\s*(\d+)\s*,\s*\"([^\"]*)\"\s*,\s*l\s*,\s*(\d+)\s*,\s*(\w+)\s*,")
content = {}  # buffer id -> ("full",) or ("rect", side) or ("app", colour)
layers = {}
panel = None
found = {}
extra = []
for t in ev.finditer(run):
    if t.group(1):
        bid, again = t.group(1), t.group(2) == "true"
        f = fills.get(bid) or fills.get("default")
        if f is None:
            continue
        if f[0] == "safety":
            expr = f[1].replace(" ", "")
            full = {"true": True, "false": False,
                    "!again": not again, "again": again}.get(expr)
            if full is None:
                content[bid] = ("unknown expression " + f[1],)
            elif full:
                content[bid] = ("full",)
            else:
                content[bid] = ("rect", side_of.get(f[2], "?" + f[2]))
        else:
            content[bid] = ("app", colour.get(f[1]))
    elif t.group(3):
        layers[int(t.group(3))] = t.group(4)
    else:
        n, name, count, status = int(t.group(5)), t.group(6), int(t.group(7)), t.group(8)
        if n in found:
            extra.append("scene %d twice" % n)
        if status == "CE_OK":
            s = content.get(layers.get(0), ("none",))
            app = None
            if count >= 2 and layers.get(1) in content and content[layers[1]][0] == "app":
                app = content[layers[1]][1]
            panel = (s, app)
        found[n] = (name, panel)

# The tool.
spec = importlib.util.spec_from_file_location("am62p_scenes_tool", tool)
try:
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    table = mod.SCENES
except Exception as e:
    finish(1, "FAIL: cannot load scripts/am62p-scenes.py: %s" % e)


def expected(n):
    r = subprocess.run([sys.executable, "-I", tool, "expect", "--scene", str(n),
                        "--width", "800", "--height", "480"],
                       capture_output=True, text=True)
    if r.returncode != 0:
        return None, "expect failed: " + r.stderr.strip()
    try:
        regs = json.loads(r.stdout)["regions"]
    except (ValueError, KeyError):
        return None, "expect printed no JSON regions"
    kind, app = None, None
    for g in regs:
        nm = g["name"]
        if nm.startswith("red all over"):
            kind = ("full",)
        elif nm.startswith("red rectangle "):
            kind = ("rect", nm.split()[-1])
        elif nm.startswith("application under"):
            app = g["expect"]
        elif nm.startswith("background under"):
            app = None
    for g in regs:
        if g["name"].startswith("red") and g["expect"] != red:
            return None, "red region colour %s, scenes.h says %s" % (g["expect"], red)
    return (kind, app), None


out = []
bad = 0
for n in range(1, 9):
    diffs = []
    fw = found.get(n)
    exp, err = expected(n)
    if err:
        diffs.append(err)
    if fw is None:
        diffs.append("not found in scenes.c")
    elif exp is not None:
        name, st = fw
        if n not in table:
            diffs.append("not in am62p-scenes.py")
        elif table[n][0] != name:
            diffs.append("name: scenes.c %r, am62p-scenes.py %r" % (name, table[n][0]))
        if st is None:
            diffs.append("scenes.c: no panel state before this scene")
        else:
            (s, app) = st
            if s != exp[0]:
                diffs.append("safety layer: scenes.c %s, am62p-scenes.py %s"
                             % (" ".join(map(str, s)), " ".join(map(str, exp[0]))))
            if exp[0] == ("full",):
                if app is not None:
                    diffs.append("application plane: scenes.c %s, am62p-scenes.py off" % app)
            elif app != exp[1]:
                diffs.append("application plane: scenes.c %s, am62p-scenes.py %s"
                             % ("off" if app is None else app,
                                "off" if exp[1] is None else exp[1]))
    bad += bool(diffs)
    out.append("SYNC %d %s" % (n, "differs: " + "; ".join(diffs) if diffs else "ok"))
for n in sorted(set(found) - set(range(1, 9))):
    extra.append("scenes.c has scene %d, am62p-scenes.py does not" % n)
if bad or extra:
    why = ["%d of 8 scenes differ" % bad] if bad else []
    finish(1, "FAIL: " + "; ".join(why + extra), out)
finish(0, "PASS", out)
PYEOF
exit $?
