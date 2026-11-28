#!/usr/bin/env python3
"""Plane-type and possible_crtcs arithmetic from `probe --raw` text output.

Usage: plane-count-expect.py [--expect-planes N] [--expect-crtcs N]
                             [--limits FILE] [PROBE_OUTPUT|-]
Exit: 0 ok, 1 an --expect-* mismatch, 2 usage or parse error.
"""
import os
import re
import sys

TYPES = ("primary", "overlay", "cursor", "other", "none")
MAIN = ("primary", "overlay", "cursor")


def die(msg):
    print(f"plane-count-expect: {msg}", file=sys.stderr)
    sys.exit(2)


def parse(text):
    """Returns (crtc ids in listing order, [(plane id, type, set of crtc ids)])."""
    crtcs, planes = [], []
    section = None
    for line in text.splitlines():
        if line in ("crtcs", "planes", "connectors", "raw"):
            section = line
            continue
        m = re.match(r"^crtc (\d+) (active|inactive)\b", line)
        if m and section == "crtcs":
            crtcs.append(int(m.group(1)))
            continue
        m = re.match(
            r"^plane (\d+) type (\w+) possible crtcs (0b[01]+|0x[0-9a-fA-F]+|\d+)"
            r"(?: \(([\d ]*)\))?\s*$",
            line,
        )
        if m and section == "planes":
            kind = m.group(2)
            if kind not in TYPES:
                die(f"unknown plane type {kind!r}: {line.strip()}")
            ids = {int(x) for x in (m.group(4) or "").split()}
            planes.append((int(m.group(1)), kind, int(m.group(3), 0), ids))
    if not crtcs:
        die("no 'crtcs' section found; run probe with --raw")
    if not planes:
        die("no 'planes' section found; run probe with --raw")
    out = []
    for pid, kind, mask, ids in planes:
        if not ids and mask:
            ids = {c for bit, c in enumerate(crtcs) if mask >> bit & 1}
        unknown = ids - set(crtcs)
        if unknown:
            die(f"plane {pid} names unknown crtc(s) {sorted(unknown)}")
        out.append((pid, kind, ids))
    return crtcs, out


def max_planes(limits):
    try:
        with open(limits) as f:
            m = re.search(r"pub const MAX_PLANES: usize = (\d+);", f.read())
    except OSError as e:
        die(f"cannot read {limits}: {e}")
    if not m:
        die(f"MAX_PLANES not found in {limits}")
    return int(m.group(1))


def main(argv):
    here = os.path.dirname(os.path.abspath(__file__))
    limits = os.path.join(here, "..", "crates", "composition-core", "src", "limits.rs")
    exp_planes = exp_crtcs = None
    path = "-"
    args = list(argv)
    while args:
        a = args.pop(0)
        if a in ("--expect-planes", "--expect-crtcs", "--limits"):
            if not args:
                die(f"{a} needs a value")
            v = args.pop(0)
            if a == "--limits":
                limits = v
                continue
            if not v.isdigit():
                die(f"{a} needs a number, got {v!r}")
            if a == "--expect-planes":
                exp_planes = int(v)
            else:
                exp_crtcs = int(v)
        elif a in ("-h", "--help"):
            print(__doc__)
            return 0
        elif a.startswith("-") and a != "-":
            die(f"unknown option {a}")
        else:
            path = a
    try:
        if path == "-":
            text = sys.stdin.read()
        else:
            with open(path) as f:
                text = f.read()
    except OSError as e:
        die(f"cannot read {path}: {e}")

    crtcs, planes = parse(text)
    cap = max_planes(limits)

    print(f"crtcs: {len(crtcs)}")
    print(f"planes: {len(planes)}")
    for t in TYPES:
        n = sum(1 for p in planes if p[1] == t)
        if n or t in MAIN:
            print(f"  {t}: {n}")

    worst = 0
    all_cursor = True
    for c in crtcs:
        mine = [p for p in planes if c in p[2]]
        worst = max(worst, len(mine))
        by = {t: [p[0] for p in mine if p[1] == t] for t in TYPES}
        parts = ", ".join(f"{t} {len(by[t])}" for t in TYPES if by[t] or t in MAIN)
        print(f"crtc {c}: {len(mine)} planes ({parts})")
        for t in TYPES:
            if by[t]:
                print(f"  {t}: {' '.join(map(str, by[t]))}")
        extra = len(by["cursor"]) + len(by["other"]) + len(by["none"])
        only_cursor = not by["other"] and not by["none"]
        if not only_cursor:
            all_cursor = False
        if not extra:
            verdict = "none"
        elif only_cursor:
            verdict = "all cursor"
        else:
            verdict = "not all cursor"
        print(f"  beyond primary+overlays: {extra} ({verdict})")

    shared = [p for p in planes if len(p[2]) > 1]
    print(f"shared planes: {len(shared)}")
    for pid, kind, ids in shared:
        print(f"  plane {pid} {kind}: crtcs {' '.join(map(str, sorted(ids)))}")
    orphans = [p[0] for p in planes if not p[2]]
    if orphans:
        print(f"planes with no possible crtc: {' '.join(map(str, orphans))}")

    print(
        "extra planes are cursors: "
        + ("yes, on every crtc" if all_cursor else "no, some crtc has non-cursor extras")
    )
    hide = len(planes) > cap or worst > cap
    print(
        f"MAX_PLANES {cap}: total {len(planes)}, largest per-crtc {worst}; "
        + ("caps would hide planes" if hide else "caps would not hide planes")
    )

    rc = 0
    if exp_planes is not None and exp_planes != len(planes):
        print(f"MISMATCH: expected {exp_planes} planes, found {len(planes)}")
        rc = 1
    if exp_crtcs is not None and exp_crtcs != len(crtcs):
        print(f"MISMATCH: expected {exp_crtcs} crtcs, found {len(crtcs)}")
        rc = 1
    return rc


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
