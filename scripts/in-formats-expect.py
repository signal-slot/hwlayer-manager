#!/usr/bin/env python3
"""Judge raw IN_FORMATS per plane from the text of the probe or props example.

Reads crates/composition-drm/examples/probe.rs output ("plane N type ...",
"  IN_FORMATS (n): FOURCC:0xmod ...") or props.rs output ("plane N: ...",
"IN_FORMATS: F formats, M modifiers", "modifier 0x...: FOURCC ..."), from
files or stdin, and prints one verdict line per plane:

  plane <id> pairs <n> over-max <yes|no> (max <M>) linear-first <yes|no|absent> afbc-beyond-max <k>

Exit 0 when every plane parsed, 2 on input it cannot parse (message on stderr).
Standard library only.
"""
import os
import re
import sys

ARM_VENDOR = 0x08  # DRM_FORMAT_MOD_VENDOR_ARM; AFBC modifiers are ARM ones.


def max_formats(override=None):
    """MAX_FORMATS: from crates/composition-core/src/limits.rs unless given."""
    if override is not None:
        return override
    here = os.path.dirname(os.path.abspath(__file__))
    path = os.path.join(here, "..", "crates", "composition-core", "src", "limits.rs")
    try:
        with open(path) as fh:
            text = fh.read()
    except OSError as e:
        raise ValueError("cannot read composition-core limits.rs (%s); use --max-formats" % e)
    m = re.search(r"pub const MAX_FORMATS:\s*usize\s*=\s*(\d+)\s*;", text)
    if not m:
        raise ValueError("MAX_FORMATS not found in %s" % path)
    return int(m.group(1))


def is_afbc(mod):
    return (mod >> 56) == ARM_VENDOR


def parse(lines):
    """Returns [(plane_id, modifiers or None)], one modifier per pair, in order."""
    planes = []
    state = {"cur": None}  # [id, pairs|None, expected_pairs, expected_mods, seen_mods]
    head = re.compile(r"^plane (\d+)(?: type |:)")

    def close():
        cur = state["cur"]
        if cur is None:
            return
        pid, pairs, ecount, emods, seen = cur
        if emods is not None and emods != seen:
            raise ValueError("plane %s: IN_FORMATS announces %d modifiers, %d lines" % (pid, emods, seen))
        if ecount is not None and pairs is not None and ecount != len(pairs):
            raise ValueError("plane %s: IN_FORMATS announces %d pairs, found %d" % (pid, ecount, len(pairs)))
        planes.append((pid, pairs))

    for n, raw in enumerate(lines, 1):
        line = raw.rstrip("\n")
        m = head.match(line)
        if m:
            close()
            state["cur"] = [int(m.group(1)), None, None, None, 0]
            continue
        cur = state["cur"]
        if cur is None:
            continue
        s = line.strip()
        if s.startswith("IN_FORMATS: unreadable") or s.startswith("IN_FORMATS blob too short"):
            raise ValueError("line %d: plane %d: %s" % (n, cur[0], s))
        m = re.match(r"^IN_FORMATS \((\d+)\):(.*)$", s)  # probe
        if m:
            pairs = []
            for tok in m.group(2).split():
                t = re.fullmatch(r"(\S{4}):0x([0-9a-fA-F]+)", tok)
                if not t:
                    raise ValueError("line %d: bad pair %r" % (n, tok))
                pairs.append(int(t.group(2), 16))
            cur[1], cur[2] = pairs, int(m.group(1))
            continue
        m = re.match(r"^formats \((\d+)\): linear(.*)$", s)  # probe, no IN_FORMATS
        if m:
            cur[1] = [0] * len(m.group(2).split())
            cur[2] = int(m.group(1))
            continue
        m = re.match(r"^IN_FORMATS: (\d+) formats, (\d+) modifiers$", s)  # props
        if m:
            cur[1], cur[3], cur[4] = [], int(m.group(2)), 0
            continue
        m = re.match(r"^modifier 0x([0-9a-fA-F]+):(.*)$", s)  # props
        if m and cur[3] is not None:
            cur[1].extend([int(m.group(1), 16)] * len(m.group(2).split()))
            cur[4] += 1
            continue
        if s.startswith("modifier entry") and "truncated" in s:
            raise ValueError("line %d: plane %d: %s" % (n, cur[0], s))
    close()
    if not planes:
        raise ValueError("no plane lines found")
    return planes


def verdict(pid, mods, limit):
    if mods is None:
        return "plane %d pairs none (no IN_FORMATS) over-max no (max %d) linear-first absent afbc-beyond-max 0" % (pid, limit)
    lin = next((i for i, x in enumerate(mods) if x == 0), None)
    vend = next((i for i, x in enumerate(mods) if x != 0), None)
    if lin is None:
        lf = "absent"
    elif vend is None or lin < vend:
        lf = "yes"
    else:
        lf = "no"
    afbc = sum(1 for x in mods[limit:] if is_afbc(x))
    return "plane %d pairs %d over-max %s (max %d) linear-first %s afbc-beyond-max %d" % (
        pid, len(mods), "yes" if len(mods) > limit else "no", limit, lf, afbc)


def main(argv):
    limit = None
    files = []
    it = iter(argv)
    for a in it:
        if a == "--max-formats":
            try:
                limit = int(next(it))
            except (StopIteration, ValueError):
                print("in-formats-expect: --max-formats needs a number", file=sys.stderr)
                return 2
        elif a in ("-h", "--help"):
            print(__doc__)
            return 0
        else:
            files.append(a)
    try:
        limit = max_formats(limit)
        if files:
            lines = []
            for f in files:
                with open(f) as fh:
                    lines.extend(fh.readlines())
        else:
            lines = sys.stdin.readlines()
        planes = parse(lines)
    except (ValueError, OSError, UnicodeDecodeError) as e:
        print("in-formats-expect: %s" % e, file=sys.stderr)
        return 2
    for pid, mods in planes:
        print(verdict(pid, mods, limit))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
