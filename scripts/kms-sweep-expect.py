#!/usr/bin/env python3
"""Judge a saved kms_sweep TEST_ONLY combination run (python3, stdlib only).

Reads the COMBO / COMBO_MAX lines that
crates/composition-drm/examples/kms_sweep.rs prints and prints

    SWEEP windows=<n> pass=<k> fail=<m>      one per window count
    SWEEP max_simultaneous=<n>               largest count with a pass
    EXPECT ok|fail                           only with --expect-min N

Exit status: 0 ok, 1 EXPECT fail or COMBO_MAX disagreement, 2 input error.
"""
import re
import sys

COMBO = re.compile(
    r"^COMBO k=(\d+) set=(\d+) planes=(\d+(?:,\d+)*) scale=\S+ src=\S+ dst=\S+ result=(\S+)$"
)
MAXLINE = re.compile(r"^COMBO_MAX accepted_k=(\d+)$")
USAGE = "usage: kms-sweep-expect.py LOG [--expect-min N]"


def die(msg):
    print("kms-sweep-expect: " + msg, file=sys.stderr)
    sys.exit(2)


def parse(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            lines = f.read().splitlines()
    except OSError as e:
        die("cannot read %s: %s" % (path, e))
    counts = {}
    reported = None
    for no, raw in enumerate(lines, 1):
        line = raw.strip()
        if line.startswith("COMBO_MAX"):
            m = MAXLINE.match(line)
            if not m:
                die("line %d: malformed COMBO_MAX: %s" % (no, line))
            reported = int(m.group(1))
        elif line.startswith("COMBO "):
            m = COMBO.match(line)
            if not m:
                die("line %d: malformed COMBO: %s" % (no, line))
            k = int(m.group(1))
            listed = len(m.group(3).split(","))
            if listed != k:
                die("line %d: k=%d but %d planes listed" % (no, k, listed))
            ok = m.group(4) == "ok"
            p, f = counts.get(k, (0, 0))
            counts[k] = (p + ok, f + (not ok))
    if not counts:
        die("no COMBO lines in %s" % path)
    return counts, reported


def main(argv):
    if len(argv) < 1 or argv[0].startswith("-"):
        die(USAGE)
    path = argv[0]
    expect = None
    rest = argv[1:]
    if rest:
        if len(rest) != 2 or rest[0] != "--expect-min" or not rest[1].isdigit():
            die(USAGE)
        expect = int(rest[1])
    counts, reported = parse(path)
    best = 0
    for k in sorted(counts):
        p, f = counts[k]
        print("SWEEP windows=%d pass=%d fail=%d" % (k, p, f))
        if p:
            best = max(best, k)
    print("SWEEP max_simultaneous=%d" % best)
    rc = 0
    if reported is not None and reported > best:
        print("SWEEP note: COMBO_MAX accepted_k=%d exceeds the COMBO lines" % reported)
        rc = 1
    if expect is not None:
        good = best >= expect
        print("EXPECT " + ("ok" if good else "fail"))
        if not good:
            rc = 1
    return rc


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
