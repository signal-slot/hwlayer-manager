#!/usr/bin/env python3
"""kms-dump-diff.py - report drift between two KMS dumps of the same kind.

Usage: python3 -I scripts/kms-dump-diff.py [--markdown] [--kind probe|props]
                                           OLD NEW

OLD and NEW are text dumps printed by the composition-drm `probe` example
(with or without --raw) or by the `props` example. Paths come only from the
arguments. Standard library only. See kms-dump-diff-README.md.

Exit status: 0 nothing differs, 1 something differs, 2 usage error.
"""

import re
import sys

USAGE = "usage: kms-dump-diff.py [--markdown] [--kind probe|props] OLD NEW"

INTERESTING_PROPS = ("zpos", "rotation")


def is_interesting(name):
    low = name.lower()
    return low in INTERESTING_PROPS or "scal" in low


def norm_mod(text):
    try:
        return "%#x" % int(text, 16)
    except ValueError:
        return text


def parse_pairs(tokens):
    """FOURCC or FOURCC:0xMOD tokens -> set of 'FOURCC:0xMOD'; 'linear' skipped."""
    pairs = set()
    for tok in tokens:
        if tok == "linear":
            continue
        fmt, sep, mod = tok.rpartition(":")
        if sep and mod.startswith("0x"):
            pairs.add("%s:%s" % (fmt, norm_mod(mod)))
        else:
            pairs.add("%s:0x0" % tok)
    return pairs


class Dump:
    def __init__(self, kind):
        self.kind = kind
        self.planes = {}  # id -> dict
        self.connectors = set()
        self.crtcs = set()
        self.unparsed = []  # (lineno, text)

    def plane(self, pid):
        return self.planes.setdefault(
            pid,
            {
                "type": None,
                "reach": {},  # label -> text
                "zpos": {},  # label -> text
                "props": {},  # name -> spec
                "formats": {},  # label -> set of pairs
            },
        )


def detect_kind(lines):
    for line in lines:
        if re.match(r"^plane \d+: possible_crtcs ", line) or re.match(
            r"^crtc \d+ \(index \d+\)", line
        ):
            return "props"
        if re.match(r"^device \S+, driver \S+ \d+\.\d+\.\d+ \(", line):
            return "props"
    for line in lines:
        if re.match(r"^(output \d+: crtc |plane \d+ \w+: outputs |plane \d+ type )", line):
            return "probe"
        if line.strip() in ("raw", "planes", "connectors", "crtcs"):
            return "probe"
        if re.match(r"^device \S+, driver \S+$", line):
            return "probe"
    return None


RE_PROBE_OUTPUT = re.compile(r"^output \d+: crtc (\d+) ")
RE_PROBE_PLANE = re.compile(
    r"^plane (\d+) (\w+): outputs (\S+), zpos (.+?), plane alpha"
)
RE_PROBE_RAW_PLANE = re.compile(
    r"^plane (\d+) type (\w+) possible crtcs (.+)$"
)
RE_PROBE_FORMATS = re.compile(r"^  formats \(\d+\):\s*(.*)$")
RE_PROBE_IDENT = re.compile(r"^  formats \(\d+\): \(identical to plane (\d+)\)")
RE_PROBE_INFMT = re.compile(r"^  IN_FORMATS \(\d+\):\s*(.*)$")
RE_PROBE_CONN = re.compile(r"^connector (\d+) interface ")
RE_PROBE_CRTC = re.compile(r"^crtc (\d+) (active|inactive)")
RE_PROBE_PROP = re.compile(r"^    (.+?): (mutable|immutable)\b(.*?) value (\d+)$")
RE_PROBE_IGNORE = re.compile(
    r"^(device |raw$|planes$|connectors$|crtcs$|driver: |client caps: |"
    r"driver cap |resources: |  current encoder |  encoder \d+ possible crtcs |"
    r"  modes \(|    \d+x\d+ @ |  attached to |  properties:$|"
    r"  IN_FORMATS: unreadable)"
)

RE_PROPS_CRTC = re.compile(r"^crtc (\d+) \(index \d+\)")
RE_PROPS_CONN = re.compile(r"^connector (\d+) ")
RE_PROPS_PLANE = re.compile(
    r"^plane (\d+): possible_crtcs (\S+) \(crtc indices \[([^\]]*)\]\)"
)
RE_PROPS_PROP = re.compile(r"^    (.+?) \(id \d+\): (.*)$")
RE_PROPS_INFMT_HEAD = re.compile(r"^      IN_FORMATS: \d+ formats, \d+ modifiers$")
RE_PROPS_MOD = re.compile(r"^      modifier (0x[0-9a-fA-F]+):\s*(.*)$")
RE_PROPS_IGNORE = re.compile(
    r"^(device |      IN_FORMATS blob too short|      modifier entry \d+ is truncated)"
)
RE_VALUE_TAIL = re.compile(r" = (-?\d+)(?: \((.*)\))?$")


def parse(text, kind):
    d = Dump(kind)
    ctx = None  # ("plane", id) / ("crtc", id) / ("connector", id)
    lines = text.splitlines()
    for no, raw in enumerate(lines, 1):
        line = raw.rstrip()
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        ok = parse_probe_line(d, line, ctx) if kind == "probe" else parse_props_line(d, line, ctx)
        if ok is False:
            d.unparsed.append((no, line.strip()))
        else:
            ctx = ok if ok is not True else ctx
    # resolve "identical to plane N" format folds
    for pid, label, src in getattr(d, "_idents", []):
        other = d.planes.get(src)
        if other is not None:
            d.plane(pid)["formats"][label] = set(other["formats"].get(label, set()))
    return d


def prop_note(d, ctx, name, spec):
    if ctx and ctx[0] == "plane" and is_interesting(name):
        p = d.plane(ctx[1])
        if name.lower() == "zpos":
            m = re.search(r"range (\S+)", spec)
            p["zpos"]["property"] = m.group(1) if m else spec
        else:
            p["props"][name] = spec


def parse_probe_line(d, line, ctx):
    """Returns the new context, True (parsed, same context) or False (unparsed)."""
    m = RE_PROBE_OUTPUT.match(line)
    if m:
        d.crtcs.add(m.group(1))
        return True
    m = RE_PROBE_PLANE.match(line)
    if m:
        p = d.plane(m.group(1))
        p["type"] = m.group(2).lower()
        p["reach"]["outputs"] = m.group(3)
        p["zpos"]["summary"] = m.group(4)
        return ("plane", m.group(1), "summary")
    m = RE_PROBE_RAW_PLANE.match(line)
    if m:
        p = d.plane(m.group(1))
        p["type"] = m.group(2).lower()
        p["reach"]["possible crtcs"] = m.group(3).strip()
        return ("plane", m.group(1), "raw")
    m = RE_PROBE_CONN.match(line)
    if m:
        d.connectors.add(m.group(1))
        return ("connector", m.group(1))
    m = RE_PROBE_CRTC.match(line)
    if m:
        d.crtcs.add(m.group(1))
        return ("crtc", m.group(1))
    if ctx and ctx[0] == "plane":
        m = RE_PROBE_IDENT.match(line)
        if m:
            if not hasattr(d, "_idents"):
                d._idents = []
            d._idents.append((ctx[1], "summary", m.group(1)))
            return True
        m = RE_PROBE_INFMT.match(line)
        if m:
            d.plane(ctx[1])["formats"]["IN_FORMATS"] = parse_pairs(m.group(1).split())
            return True
        m = RE_PROBE_FORMATS.match(line)
        if m:
            label = "summary" if ctx[2] == "summary" else "linear list"
            d.plane(ctx[1])["formats"][label] = parse_pairs(m.group(1).split())
            return True
    m = RE_PROBE_PROP.match(line)
    if m:
        spec = (m.group(2) + m.group(3)).strip()
        prop_note(d, ctx, m.group(1), spec)
        return True
    if RE_PROBE_IGNORE.match(line):
        return True
    return False


def parse_props_line(d, line, ctx):
    m = RE_PROPS_CRTC.match(line)
    if m:
        d.crtcs.add(m.group(1))
        return ("crtc", m.group(1))
    m = RE_PROPS_PLANE.match(line)
    if m:
        p = d.plane(m.group(1))
        p["reach"]["possible_crtcs"] = "%s [%s]" % (m.group(2), m.group(3).replace(" ", ""))
        return ("plane", m.group(1))
    m = RE_PROPS_CONN.match(line)
    if m:
        d.connectors.add(m.group(1))
        return ("connector", m.group(1))
    m = RE_PROPS_PROP.match(line)
    if m:
        name, rest = m.group(1), m.group(2)
        value = None
        mt = RE_VALUE_TAIL.search(rest)
        if mt:
            value = mt.group(2)
            rest = rest[: mt.start()]
        spec = rest.strip()
        if ctx and ctx[0] == "plane" and name == "type":
            if value:
                d.plane(ctx[1])["type"] = value.lower()
            return True
        prop_note(d, ctx, name, spec)
        return True
    if ctx and ctx[0] == "plane":
        if RE_PROPS_INFMT_HEAD.match(line):
            d.plane(ctx[1])["formats"].setdefault("IN_FORMATS", set())
            return True
        m = RE_PROPS_MOD.match(line)
        if m:
            mod = norm_mod(m.group(1))
            s = d.plane(ctx[1])["formats"].setdefault("IN_FORMATS", set())
            for fmt in m.group(2).split():
                s.add("%s:%s" % (fmt, mod))
            return True
    if RE_PROPS_IGNORE.match(line):
        return True
    return False


def set_diff(label, old, new, out, key):
    for item in sorted(old - new):
        out.append((key, "%s %s removed" % (label, item)))
    for item in sorted(new - old):
        out.append((key, "%s %s added" % (label, item)))


def natural(text):
    return [int(t) if t.isdigit() else t for t in re.split(r"(\d+)", text)]


def diff(old, new):
    """Returns a list of (group, id, text) sorted for output."""
    items = []
    for pid in sorted(set(old.planes) - set(new.planes), key=int):
        items.append((0, int(pid), 0, "plane %s removed" % pid))
    for pid in sorted(set(new.planes) - set(old.planes), key=int):
        items.append((0, int(pid), 1, "plane %s added" % pid))
    for pid in sorted(set(old.planes) & set(new.planes), key=int):
        a, b = old.planes[pid], new.planes[pid]
        rows = []
        if a["type"] != b["type"]:
            rows.append("type %s -> %s" % (a["type"], b["type"]))
        for label in sorted(set(a["reach"]) | set(b["reach"])):
            x, y = a["reach"].get(label), b["reach"].get(label)
            if x != y:
                rows.append("%s %s -> %s" % (label, x, y))
        for label in sorted(set(a["zpos"]) | set(b["zpos"])):
            x, y = a["zpos"].get(label), b["zpos"].get(label)
            if x != y:
                rows.append("zpos %s (%s) %s -> %s" % ("range", label, x, y))
        for name in sorted(set(a["props"]) | set(b["props"])):
            x, y = a["props"].get(name), b["props"].get(name)
            if name.lower() == "rotation":
                what = "rotation"
            else:
                what = "scaling property %s" % name
            if x is None:
                rows.append("%s added (%s)" % (what, y))
            elif y is None:
                rows.append("%s removed" % what)
            elif x != y:
                rows.append("%s: %s -> %s" % (what, x, y))
        for label in sorted(set(a["formats"]) | set(b["formats"])):
            x = a["formats"].get(label, set())
            y = b["formats"].get(label, set())
            tag = "format" if label == "IN_FORMATS" else "format (%s)" % label
            rows_f = []
            set_diff(tag, x, y, rows_f, 0)
            rows.extend(r[1] for r in rows_f)
        for row in rows:
            items.append((1, int(pid), 2, "plane %s: %s" % (pid, row)))
    for label, o, n, group in (
        ("connector", old.connectors, new.connectors, 2),
        ("crtc", old.crtcs, new.crtcs, 3),
    ):
        for cid in sorted(o - n, key=int):
            items.append((group, int(cid), 0, "%s %s removed" % (label, cid)))
        for cid in sorted(n - o, key=int):
            items.append((group, int(cid), 1, "%s %s added" % (label, cid)))
    # plane-level add/remove first, then per-plane details; row text keeps its order
    items.sort(key=lambda t: (t[0] if t[0] != 1 else 0.5, t[1], t[2]))
    return items


def read(path):
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError as e:
        print("kms-dump-diff: %s: %s" % (path, e.strerror or e), file=sys.stderr)
        sys.exit(2)


def usage_error(msg):
    print("kms-dump-diff: %s" % msg, file=sys.stderr)
    print(USAGE, file=sys.stderr)
    sys.exit(2)


def main(argv):
    markdown = False
    kind = None
    paths = []
    it = iter(argv)
    for a in it:
        if a == "--markdown":
            markdown = True
        elif a == "--kind":
            kind = next(it, None)
            if kind not in ("probe", "props"):
                usage_error("--kind needs probe or props")
        elif a.startswith("--kind="):
            kind = a.split("=", 1)[1]
            if kind not in ("probe", "props"):
                usage_error("--kind needs probe or props")
        elif a in ("-h", "--help"):
            print(USAGE)
            return 0
        elif a.startswith("-") and a != "-":
            usage_error("unknown argument %s" % a)
        else:
            paths.append(a)
    if len(paths) != 2:
        usage_error("need exactly two dump files")
    texts = [read(p) for p in paths]
    if kind is None:
        kinds = [detect_kind(t.splitlines()) for t in texts]
        for p, k in zip(paths, kinds):
            if k is None:
                usage_error("cannot tell the dump kind of %s (use --kind)" % p)
        if kinds[0] != kinds[1]:
            usage_error("dumps are of different kinds (%s vs %s)" % tuple(kinds))
        kind = kinds[0]
    old, new = parse(texts[0], kind), parse(texts[1], kind)
    items = diff(old, new)
    unparsed = len(old.unparsed) + len(new.unparsed)
    lines = [t[3] for t in items]

    if markdown:
        print("# kms-dump-diff (%s)" % kind)
        print()
        print("- old: `%s`" % paths[0])
        print("- new: `%s`" % paths[1])
        print()
        if lines:
            for l in lines:
                print("- %s" % l)
        else:
            print("No differences.")
        if unparsed:
            print()
            print("unparsed: %d" % unparsed)
            for tag, dump in (("old", old), ("new", new)):
                for no, text in dump.unparsed:
                    print("- %s:%d: `%s`" % (tag, no, text))
    else:
        for l in lines:
            print(l)
        if not lines:
            print("no differences")
        if unparsed:
            print("unparsed: %d" % unparsed)
            for tag, dump in (("old", old), ("new", new)):
                for no, text in dump.unparsed:
                    print("  %s:%d: %s" % (tag, no, text))
    return 1 if lines else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
