#!/usr/bin/env python3
"""Compare the AM62P R5F board console with the host reference console.

Usage: python3 -I am62p-console-compare.py BOARD.txt HOST.txt
See am62p-console-compare-README.md. Host tool only.
"""
import re
import sys

SCENE = re.compile(
    r"SCENE (\d+) (\S+): status (-?\d+) \(expected (-?\d+)\), "
    r"commits (\d+) latched (\d+) \(\+(\d+), expected \+(\d+)\), "
    r"presented (yes|NO|-), released (\d+) \(expected (\d+)\): (ok|FAIL)\s*$")
HELD = re.compile(
    r"SCENE (\d+) (\S+): commits changed while held: (\d+) -> (\d+): FAIL\s*$")
RESULT = re.compile(r"RESULT (PASS|FAIL) \((.*?)\)\s*$")
FIND = re.compile(r"(SCENE \d+ \S+:.*|RESULT (?:PASS|FAIL).*)$")

SCENE_FIELDS = ["name", "status", "exp_status", "commits", "latched", "delta",
                "exp_delta", "presented", "released", "exp_released", "verdict"]
# Measured on the board, simulated on the host: reported, not failed.
SOFT = {"commits", "latched", "vsyncs"}


def read(path):
    """Return {key: fields} for the SCENE and RESULT lines of a file."""
    out = {}
    with open(path, encoding="utf-8", errors="replace") as f:
        for raw in f:
            m = FIND.search(raw.rstrip("\r\n"))
            if not m:
                continue
            line = m.group(1)
            if line.startswith("SCENE"):
                s = SCENE.match(line)
                if s:
                    out[("scene", int(s.group(1)))] = dict(
                        zip(SCENE_FIELDS, s.groups()[1:]))
                    continue
                h = HELD.match(line)
                if h:
                    out[("held", int(h.group(1)))] = {
                        "name": h.group(2), "before": h.group(3),
                        "after": h.group(4)}
                    continue
                out[("bad", line)] = {"line": line}
            else:
                r = RESULT.match(line)
                if not r:
                    out[("bad", line)] = {"line": line}
                    continue
                detail = r.group(2)
                v = re.search(r",? ?vsyncs (\d+)", detail)
                rest = re.sub(r",? ?vsyncs \d+", "", detail).strip()
                out[("result", 0)] = {"verdict": r.group(1), "detail": rest,
                                      "vsyncs": v.group(1) if v else "-"}
    return out


def sort_key(key):
    order = {"scene": 0, "held": 1, "result": 2, "bad": 3}
    return (order[key[0]], str(key[1]).zfill(6))


def compare(a, b):
    """Yield (label, state, text); state in MATCH, DIFF, MISSING."""
    for key in sorted(set(a) | set(b), key=sort_key):
        kind, n = key
        label = {"scene": "SCENE %s" % n, "held": "SCENE %s held" % n,
                 "result": "RESULT", "bad": "UNPARSED"}[kind]
        x, y = a.get(key), b.get(key)
        if x is None or y is None:
            side = "board" if x is None else "host"
            yield label, "MISSING", "missing on %s" % side
            continue
        hard, soft = [], []
        for field in x:
            if x[field] != y.get(field):
                d = "%s board=%s host=%s" % (field, x[field], y.get(field))
                (soft if field in SOFT else hard).append(d)
        if hard:
            yield label, "DIFF", "; ".join(hard + soft)
        elif soft:
            yield label, "DIFF", "soft (not a failure): " + "; ".join(soft)
        else:
            yield label, "MATCH", ""


def main(argv):
    if len(argv) != 3:
        sys.stderr.write(__doc__)
        return 2
    try:
        board, host = read(argv[1]), read(argv[2])
    except OSError as e:
        sys.stderr.write("cannot read input: %s\n" % e)
        return 2
    ok = bool(board) and bool(host)
    for label, state, text in compare(board, host):
        print(("%s: %s %s" % (label, state, text)).rstrip())
        if state == "MISSING" or (state == "DIFF" and
                                  not text.startswith("soft")):
            ok = False
    for name, d in (("board", board), ("host", host)):
        if not d:
            print("no SCENE or RESULT lines in the %s console" % name)
    print("COMPARE PASS" if ok else "COMPARE FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
