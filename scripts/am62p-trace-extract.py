#!/usr/bin/env python3
"""Extract the last complete AM62P R5F board-test run from a raw trace0 dump.

Usage: am62p-trace-extract.py [DUMP|-]      (default: stdin)

See am62p-trace-extract-README.md. Exit 0: complete run printed; 1: the run is
incomplete (reason on stderr); 2: usage or input error.
"""
import re
import sys

SCENES = 8
BANNER = re.compile(r"\bBEGIN hwlayer-manager board test am62p_r5f\b")
SCENE = re.compile(r"\bSCENE (\d+) [a-z0-9-]+: status ")
SCENE1 = re.compile(r"\bSCENE 1 [a-z0-9-]+:")
RESULT = re.compile(r"\bRESULT (PASS|FAIL)\b")


def lines_of(data):
    out = []
    for raw in data.replace(b"\0", b"").split(b"\n"):
        try:
            text = raw.decode("utf-8")
        except UnicodeDecodeError:
            continue
        text = "".join(c for c in text if c == "\t" or c >= " ").rstrip()
        if text:
            out.append(text)
    return out


def main(argv):
    if len(argv) > 1 or (argv and argv[0].startswith("-") and argv[0] != "-"):
        print("usage: am62p-trace-extract.py [DUMP|-]", file=sys.stderr)
        return 2
    try:
        if not argv or argv[0] == "-":
            data = sys.stdin.buffer.read()
        else:
            with open(argv[0], "rb") as f:
                data = f.read()
    except OSError as e:
        print("am62p-trace-extract: %s" % e, file=sys.stderr)
        return 2
    lines = lines_of(data)
    starts = [i for i, l in enumerate(lines) if BANNER.search(l)]
    if not starts:
        starts = [i for i, l in enumerate(lines) if SCENE1.search(l)]
    if not starts:
        print("no run found: no banner and no SCENE 1 line", file=sys.stderr)
        return 1
    run = lines[starts[-1]:]
    end = next((i for i, l in enumerate(run) if RESULT.search(l)), None)
    body = run if end is None else run[:end + 1]
    have = set()
    for l in body:
        m = SCENE.search(l)
        if m:
            have.add(int(m.group(1)))
    problems = []
    if end is None:
        problems.append("no RESULT line")
    missing = [n for n in range(1, SCENES + 1) if n not in have]
    if missing:
        problems.append("missing SCENE " + ", ".join(map(str, missing)))
    if problems:
        print("incomplete run: " + "; ".join(problems), file=sys.stderr)
        return 1
    print("\n".join(body))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
