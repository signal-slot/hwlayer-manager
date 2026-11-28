#!/usr/bin/env python3
"""One verdict line per plane from the text output of the props example.

  plane-props-expect.py [FILE|-]      (default: stdin)

Input: the output of crates/composition-drm/examples/props.rs. Its plane
sections start with "plane N: possible_crtcs ..." and list properties as
"    name (id I): kind [flags] = value (decoded)". The probe example prints no
properties, so its output has no plane sections here and is rejected.

Per plane the verdict names the plane id and type, the rotation bits decoded
with the standard DRM_MODE_ROTATE_*/REFLECT_* positions, and whether zpos,
alpha, "pixel blend mode", COLOR_ENCODING, COLOR_RANGE and SCALING_FILTER exist.
Bits are named the way composition-core's Transform names them:
rotate-0 Normal, rotate-90 Rotate90, rotate-180 Rotate180, rotate-270 Rotate270,
reflect-x FlipH, reflect-y FlipV.

Limit: props prints the CURRENT value of rotation (the drm crate does not return
the bitmask's value names), not the set the plane supports. The decoded bits
are therefore what is set now; a plane showing only rotate-0 may still support
more. Use a TEST_ONLY probe for the supported set.

Exit status: 0 = ok, 2 = input error. Python 3, standard library only.
"""
import re
import sys

ROTATION = [
    (0, "rotate-0", "Normal"),
    (1, "rotate-90", "Rotate90"),
    (2, "rotate-180", "Rotate180"),
    (3, "rotate-270", "Rotate270"),
    (4, "reflect-x", "FlipH"),
    (5, "reflect-y", "FlipV"),
]
PLANE = re.compile(r"^plane (\d+): possible_crtcs ")
PROP = re.compile(r"^    (.+?) \(id (\d+)\): (.*?)(?: \[([a-z,]*)\])? = (-?\d+)(?: \((.*)\))?$")
OTHER_SECTION = re.compile(r"^(crtc|connector) \d+")


class InputError(Exception):
    pass


def parse(text):
    planes = []
    cur = None
    for n, line in enumerate(text.splitlines(), 1):
        m = PLANE.match(line)
        if m:
            cur = {"id": int(m.group(1)), "props": {}}
            planes.append(cur)
            continue
        if OTHER_SECTION.match(line) or line.startswith("device "):
            cur = None
            continue
        if cur is None or not line.startswith("    ") or line.startswith("      "):
            continue
        m = PROP.match(line)
        if not m:
            raise InputError("line %d: cannot parse property: %r" % (n, line))
        name, _id, kind, flags, value, extra = m.groups()
        cur["props"][name] = {
            "kind": kind,
            "flags": (flags or "").split(","),
            "value": int(value),
            "extra": extra,
        }
    if not planes:
        raise InputError("no 'plane N: possible_crtcs' section found (is this props output?)")
    return planes


def plane_type(p):
    t = p["props"].get("type")
    if t is None:
        return "type unknown"
    if t["extra"]:
        return t["extra"].lower()
    return "type=%d" % t["value"]


def rotation(p):
    r = p["props"].get("rotation")
    if r is None:
        return "no rotation property"
    bits = [(nm, tn) for b, nm, tn in ROTATION if r["value"] & (1 << b)]
    if r["value"] >> 6:
        bits.append(("unknown-bits=%#x" % (r["value"] >> 6 << 6), "?"))
    if not bits:
        return "rotation=0 (no bit set)"
    return "rotation=%d [%s]" % (r["value"], ", ".join("%s(%s)" % b for b in bits))


def zpos(p):
    z = p["props"].get("zpos")
    if z is None:
        return "no zpos"
    m = re.match(r"range (\d+)\.\.=(\d+)$", z["kind"])
    rng = "%s..=%s" % (m.group(1), m.group(2)) if m else z["kind"]
    imm = "immutable" if "immutable" in z["flags"] else "mutable"
    return "zpos %s %s value %d" % (rng, imm, z["value"])


def present(p, name, label=None):
    return (label or name) + (" yes" if name in p["props"] else " no")


def verdict(p):
    parts = [
        "plane %d %s" % (p["id"], plane_type(p)),
        rotation(p),
        zpos(p),
        present(p, "alpha"),
        present(p, "pixel blend mode", "blend-mode"),
        present(p, "COLOR_ENCODING", "color-encoding"),
        present(p, "COLOR_RANGE", "color-range"),
        present(p, "SCALING_FILTER", "scaling-filter"),
    ]
    return "; ".join(parts)


def main(argv):
    if len(argv) > 2 or (len(argv) == 2 and argv[1] in ("-h", "--help")):
        print(__doc__)
        return 2
    try:
        if len(argv) == 2 and argv[1] != "-":
            with open(argv[1], encoding="utf-8", errors="replace") as f:
                text = f.read()
        else:
            text = sys.stdin.read()
        planes = parse(text)
    except (InputError, OSError) as e:
        print("plane-props-expect: %s" % e, file=sys.stderr)
        return 2
    for p in planes:
        print(verdict(p))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
