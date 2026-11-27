#!/usr/bin/env python3
"""format-cut-report: which IN_FORMATS pairs survive the backend's MAX_FORMATS cut.

Run with: python3 -I scripts/format-cut-report.py [--max N] [--quiet] [FILE...]
Standard library only. See scripts/format-cut-report-README.md.

Exit code: 0 nothing dropped, 1 something dropped, 2 parse error.
"""
import re
import sys

MAX_FORMATS = 32  # composition_core::limits::MAX_FORMATS (read-only copy)

PLANE_RE = re.compile(r"^\s*plane\s+(\d+)\b")
# probe: "  IN_FORMATS (45): XR24:0x0 XR24:0x0200000000000001 ..."
PROBE_RE = re.compile(r"^\s*IN_FORMATS\s*\((\d+)\)\s*:(.*)$")
# probe, plane without IN_FORMATS: "  formats (11): linear XR24 AR24 ..."
LINEAR_RE = re.compile(r"^\s*formats\s*\((\d+)\)\s*:\s*linear\b(.*)$")
# props: "      IN_FORMATS: 21 formats, 3 modifiers"
PROPS_HEAD_RE = re.compile(r"^\s*IN_FORMATS\s*:\s*(\d+)\s+formats?\s*,\s*(\d+)\s+modifiers?\b")
# props: "      modifier 0x0000000000000000: XR24 AR24"
PROPS_MOD_RE = re.compile(r"^\s*modifier\s+(0x[0-9a-fA-F]+)\s*:(.*)$")
# IN_FORMATS lines the examples print that are not a listing; skipped:
#   probe --raw property line: "IN_FORMATS: immutable atomic blob, 232 bytes value 9"
#   props property line:       "IN_FORMATS (id 9): immutable blob"
#   unreadable blob:           "IN_FORMATS: unreadable (...)"
#   short blob (props):        "IN_FORMATS blob too short (12 bytes)"
SKIP_RE = re.compile(
    r"^\s*IN_FORMATS\s*(\(id \d+\)|:\s*(im)?mutable\b|:\s*unreadable|blob too short)")
PAIR_RE = re.compile(r"^(\S{4}):(0x[0-9a-fA-F]+)$")


class ParseError(Exception):
    pass


def parse(lines, name):
    """Returns ({plane id: [(fourcc, modifier), ...]}, plane ids in file order)."""
    planes = {}
    order = []
    cur = None
    state = None  # props block being read: {"plane": id, "left": modifier lines to come}

    def err(n, msg):
        raise ParseError("%s:%d: %s" % (name, n, msg))

    def add(n, plane, pairs, how):
        if plane is None:
            err(n, "%s before any 'plane <id>' line" % how)
        if plane in planes:
            err(n, "plane %d lists its formats twice" % plane)
        planes[plane] = pairs
        order.append(plane)

    for n, line in enumerate(lines, 1):
        line = line.rstrip("\n")
        m = PLANE_RE.match(line)
        if m:
            if state and state["left"]:
                err(n, "plane %d: %d modifier line(s) missing" % (state["plane"], state["left"]))
            cur = int(m.group(1))
            state = None
            continue
        m = PROBE_RE.match(line)
        if m:
            pairs = []
            for tok in m.group(2).split():
                p = PAIR_RE.match(tok)
                if not p:
                    err(n, "bad format:modifier token %r" % tok)
                pairs.append((p.group(1), int(p.group(2), 16)))
            if len(pairs) != int(m.group(1)):
                err(n, "IN_FORMATS declares %s pairs, %d listed" % (m.group(1), len(pairs)))
            add(n, cur, pairs, "IN_FORMATS")
            continue
        m = LINEAR_RE.match(line)
        if m:
            toks = m.group(2).split()
            if len(toks) != int(m.group(1)):
                err(n, "formats declares %s entries, %d listed" % (m.group(1), len(toks)))
            add(n, cur, [(t, 0) for t in toks], "formats")
            continue
        m = PROPS_HEAD_RE.match(line)
        if m:
            add(n, cur, [], "IN_FORMATS")
            state = {"plane": cur, "left": int(m.group(2))}
            continue
        m = PROPS_MOD_RE.match(line)
        if m and state and state["left"]:
            mod = int(m.group(1), 16)
            for tok in m.group(2).split():
                if len(tok) != 4:
                    err(n, "bad format code %r" % tok)
                planes[state["plane"]].append((tok, mod))
            state["left"] -= 1
            continue
        if re.match(r"^\s*IN_FORMATS\b", line):
            if SKIP_RE.match(line):
                continue
            err(n, "unrecognised IN_FORMATS line")
    if state and state["left"]:
        raise ParseError("%s: plane %d: %d modifier line(s) missing at end of file"
                         % (name, state["plane"], state["left"]))
    if not planes:
        raise ParseError("%s: no IN_FORMATS or linear formats listing found" % name)
    return planes, order


def cut(pairs, limit):
    """The backend's order: stable sort, linear first, then take(limit)."""
    ordered = sorted(pairs, key=lambda p: p[1] != 0)
    return ordered[:limit], ordered[limit:], ordered


def fmt(p):
    return p[0] if p[1] == 0 else "%s:%#x" % p


def main(argv):
    limit = MAX_FORMATS
    quiet = False
    files = []
    it = iter(argv)
    for a in it:
        if a == "--max":
            try:
                limit = int(next(it))
            except (StopIteration, ValueError):
                print("format-cut-report: --max needs a number", file=sys.stderr)
                return 2
        elif a == "--quiet":
            quiet = True
        elif a in ("-h", "--help"):
            print(__doc__)
            return 0
        else:
            files.append(a)
    try:
        if files:
            texts = []
            for f in files:
                with open(f, encoding="utf-8", errors="replace") as fh:
                    texts.append((f, fh.readlines()))
        else:
            texts = [("<stdin>", sys.stdin.readlines())]
        results = []
        for name, lines in texts:
            planes, order = parse(lines, name)
            results.extend((name, p, planes[p]) for p in order)
    except (ParseError, OSError) as e:
        print("format-cut-report: %s" % e, file=sys.stderr)
        return 2

    dropped_any = False
    for name, plane, pairs in results:
        kept, dropped, ordered = cut(pairs, limit)
        linear_first = any(p[1] == 0 for p in pairs) and ordered == pairs
        if not quiet:
            print("plane %d (%s): total %d" % (plane, name, len(pairs)))
            print("  kept (%d): %s" % (len(kept), " ".join(map(fmt, kept))))
            print("  dropped (%d): %s" % (len(dropped), " ".join(map(fmt, dropped)) or "-"))
        print("FORMAT-CUT plane=%d total=%d kept=%d dropped=%d linear_first=%s"
              % (plane, len(pairs), len(kept), len(dropped), "yes" if linear_first else "no"))
        dropped_any |= bool(dropped)
    return 1 if dropped_any else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
