#!/usr/bin/env python3
"""Turn scale_probe / kms_sweep lines into Rust fixture constants.

Usage: python3 -I scripts/sweep-to-fixture.py --source PATH --date YYYY-MM-DD [FILE...]
Reads stdin when no FILE is given. Exit 0 ok, 1 unparsable line(s), 2 usage.
"""
import re
import sys

KV_RE = re.compile(r"(\w+)=(.*?)(?= \w+=|$)")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
INT_RE = re.compile(r"^\d+$")
HEX_RE = re.compile(r"^(0[xX][0-9a-fA-F]+|\d+)$")
RATIO_RE = re.compile(r"^(none|\d+(\.\d+)?x?)$")

S = "&'static str"

# Line keyword -> (struct, const, [(field, rust type, validator)]).
# The field name is also the key= name in the line.
KINDS = {
    "RESULT": ("ScaleResult", "SCALE_RESULTS", [
        ("plane", "u32", "int"),
        ("kind", S, "str"),
        ("format", S, "str"),
        ("max_upscale_h", S, "ratio"),
        ("max_upscale_v", S, "ratio"),
        ("max_upscale_both", S, "ratio"),
        ("downscale", S, "str"),
        ("min_downscale", S, "ratio"),
    ]),
    "COMBO": ("Combo", "COMBOS", [
        ("k", "u32", "int"),
        ("planes", "&'static [u32]", "planes"),
        ("scale", S, "ratio"),
        ("result", S, "str"),
    ]),
    "MODIFIER": ("ModifierResult", "MODIFIERS", [
        ("plane", "u32", "int"),
        ("format", S, "str"),
        ("modifier", "u64", "mod"),
        ("listed", "bool", "yesno"),
        ("addfb", S, "str"),
        ("test_only", S, "str"),
    ]),
    "REPLACE": ("Replace", "REPLACES", [
        ("second", S, "str"),
        ("events", "u32", "int"),
    ]),
}


def rs_str(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def conv(kind, v):
    if kind == "int":
        if not INT_RE.match(v):
            raise ValueError("not an integer: " + v)
        return v
    if kind == "mod":
        if not HEX_RE.match(v):
            raise ValueError("bad modifier: " + v)
        return v
    if kind == "yesno":
        if v not in ("yes", "no"):
            raise ValueError("expected yes/no: " + v)
        return "true" if v == "yes" else "false"
    if kind == "planes":
        parts = v.split(",")
        if not all(INT_RE.match(p) for p in parts):
            raise ValueError("bad plane list: " + v)
        return "&[" + ", ".join(parts) + "]"
    if kind == "ratio":
        if not RATIO_RE.match(v):
            raise ValueError("bad ratio: " + v)
        return rs_str(v)
    if not v or not v.isprintable():
        raise ValueError("empty or unprintable value")
    return rs_str(v)


def parse_line(line):
    """Return (keyword, [rust values]) or None for ignored lines."""
    if not line.strip() or line.startswith("#"):
        return None
    head, _, rest = line.partition(" ")
    if head not in KINDS:
        return None
    kv = dict(KV_RE.findall(rest))
    out = []
    for fname, _t, vk in KINDS[head][2]:
        if fname not in kv:
            raise ValueError("missing " + fname + "=")
        out.append(conv(vk, kv[fname].strip()))
    return head, out


def render(source, date, recs):
    o = ["// source: %s, observed %s" % (source, date)]
    for head, (sname, cname, fields) in KINDS.items():
        rows = recs[head]
        if not rows:
            continue
        o += ["", "#[allow(dead_code)]", "#[derive(Clone, Copy, Debug, PartialEq)]",
              "struct %s {" % sname]
        o += ["    %s: %s," % (f, t) for f, t, _v in fields]
        o += ["}", "", "#[allow(dead_code)]"]
        # rustfmt hugs the brackets round a lone multi-line element.
        hug = len(rows) == 1
        o.append("const %s: &[%s] = &[%s" % (cname, sname, sname + " {" if hug else ""))
        for vals in rows:
            pad = "" if hug else "    "
            if not hug:
                o.append("    %s {" % sname)
            o += [pad + "    %s: %s," % (f[0], v) for f, v in zip(fields, vals)]
            o.append(pad + ("}" if hug else "},"))
        o.append("];")
        if hug:
            o[-2:] = ["}];"]
    return "\n".join(o) + "\n"


def usage(msg):
    sys.stderr.write("sweep-to-fixture: %s\n" % msg)
    sys.stderr.write("usage: sweep-to-fixture.py --source PATH --date YYYY-MM-DD [FILE...]\n")
    return 2


def main(argv):
    source = date = None
    files = []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a in ("--source", "--date"):
            if i + 1 >= len(argv):
                return usage(a + " needs a value")
            if a == "--source":
                source = argv[i + 1]
            else:
                date = argv[i + 1]
            i += 2
        elif a.startswith("-") and a != "-":
            return usage("unknown option " + a)
        else:
            files.append(a)
            i += 1
    if not source or not date:
        return usage("--source and --date are required")
    if not DATE_RE.match(date):
        return usage("--date must be YYYY-MM-DD")
    if "\n" in source or "\r" in source:
        return usage("--source must be one line")
    texts = []
    try:
        if not files or files == ["-"]:
            texts.append(("<stdin>", sys.stdin.read()))
        else:
            for p in files:
                with open(p, encoding="utf-8") as f:
                    texts.append((p, f.read()))
    except (OSError, UnicodeDecodeError) as e:
        return usage("cannot read input: %s" % e)
    recs = {k: [] for k in KINDS}
    bad = 0
    for name, text in texts:
        for n, line in enumerate(text.splitlines(), 1):
            try:
                r = parse_line(line)
            except ValueError as e:
                sys.stderr.write("%s:%d: cannot parse (%s): %s\n" % (name, n, e, line))
                bad += 1
                continue
            if r:
                recs[r[0]].append(r[1])
    if bad:
        return 1
    if not any(recs.values()):
        sys.stderr.write("sweep-to-fixture: no recognised lines\n")
        return 1
    sys.stdout.write(render(source, date, recs))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
