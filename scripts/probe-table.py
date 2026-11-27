#!/usr/bin/env python3
"""Turn `probe --raw` output into markdown tables for docs/platform-capabilities.md.

Usage:
  probe-table.py --date YYYY-MM-DD --kernel VERSION [probe-output.txt]
  probe-table.py --self-test

Reads stdin when no file is given. Every table row ends with the tag
'observed <date>, kernel <kernel>'. Python 3 standard library only.
Format reference: crates/composition-drm/examples/probe.rs (dump_raw).
"""
import argparse
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SAMPLE = os.path.join(HERE, "testdata", "probe-table", "vkms-sample.txt")

PROP_RE = re.compile(
    r"^(?P<name>\S+(?: \S+)*?): (?P<mut>mutable|immutable)(?P<atomic> atomic)?"
    r"(?P<kind>.*?) value (?P<value>-?\d+)$"
)
ROTATION_BITS = [(1, "rotate-0"), (2, "rotate-90"), (4, "rotate-180"),
                 (8, "rotate-270"), (16, "reflect-x"), (32, "reflect-y")]


class ParseError(Exception):
    pass


def parse_prop(line):
    m = PROP_RE.match(line.strip())
    if not m:
        raise ParseError("unrecognised property line: %r" % line)
    kind = m.group("kind").strip()
    p = {"name": m.group("name"), "mutable": m.group("mut") == "mutable",
         "atomic": bool(m.group("atomic")), "kind": kind,
         "value": int(m.group("value")), "range": None, "enum": None}
    r = re.match(r"range (-?\d+)\.\.=(-?\d+)$", kind)
    if r:
        p["range"] = (int(r.group(1)), int(r.group(2)))
    e = re.match(r"enum \[(.*)\]$", kind)
    if e:
        p["enum"] = [x.strip() for x in e.group(1).split(", ")] if e.group(1) else []
    return p


def parse(text):
    """Parse the `raw` section of a probe dump into a dict."""
    out = {"driver": None, "client_caps": None, "caps": [], "connectors": [],
           "crtcs": [], "planes": []}
    lines = [l.rstrip("\n") for l in text.splitlines()
             if l.strip() and not l.lstrip().startswith("#")]
    try:
        start = lines.index("raw")
    except ValueError:
        raise ParseError("no 'raw' line: was the probe run with --raw?")
    section = None
    cur = None
    in_props = False
    in_modes = False
    for line in lines[start + 1:]:
        if line in ("connectors", "crtcs", "planes"):
            section, cur, in_props, in_modes = line, None, False, False
            continue
        if section is None:
            if line.startswith("driver: "):
                m = re.match(r"driver: name (\S+) version (\S+) date (\S+) description (.*)$", line)
                if not m:
                    raise ParseError("bad driver line: %r" % line)
                out["driver"] = dict(zip(("name", "version", "date", "description"), m.groups()))
            elif line.startswith("client caps: "):
                out["client_caps"] = line[len("client caps: "):]
            elif line.startswith("driver cap "):
                k, _, v = line[len("driver cap "):].partition(": ")
                out["caps"].append((k, v))
            continue
        if not line.startswith(" "):
            in_props = in_modes = False
            if section == "connectors":
                m = re.match(r"connector (\d+) interface (\S+) \((\d+)\) state (\S+)$", line)
                if not m:
                    raise ParseError("bad connector line: %r" % line)
                cur = {"id": int(m.group(1)), "interface": m.group(2), "state": m.group(4),
                       "encoder": None, "encoders": [], "modes": [], "props": []}
                out["connectors"].append(cur)
            elif section == "crtcs":
                m = re.match(r"crtc (\d+) (inactive|active (.*))$", line)
                if not m:
                    raise ParseError("bad crtc line: %r" % line)
                cur = {"id": int(m.group(1)), "mode": m.group(3), "props": []}
                out["crtcs"].append(cur)
            else:
                m = re.match(r"plane (\d+) type (\S+) possible crtcs (\S+) \((.*)\)$", line)
                if not m:
                    raise ParseError("bad plane line: %r" % line)
                cur = {"id": int(m.group(1)), "type": m.group(2), "mask": m.group(3),
                       "crtcs": m.group(4), "attached": None, "formats": [],
                       "formats_src": None, "props": []}
                out["planes"].append(cur)
            continue
        if cur is None:
            raise ParseError("indented line before any object: %r" % line)
        s = line.strip()
        if in_props and line.startswith("    "):
            cur["props"].append(parse_prop(s))
            continue
        if in_modes and line.startswith("    "):
            cur["modes"].append(s)
            continue
        in_props = in_modes = False
        if s == "properties:":
            in_props = True
        elif s.startswith("modes ("):
            in_modes = True
        elif s.startswith("current encoder "):
            v = s[len("current encoder "):]
            cur["encoder"] = None if v == "none" else v
        elif s.startswith("encoder "):
            m = re.match(r"encoder (\d+) possible crtcs (\S+) \((.*)\)$", s)
            if not m:
                raise ParseError("bad encoder line: %r" % line)
            cur["encoders"].append((m.group(1), m.group(2), m.group(3)))
        elif s.startswith("attached to "):
            v = s[len("attached to "):]
            cur["attached"] = None if v == "none" else v.replace("crtc ", "")
        elif s.startswith("IN_FORMATS ("):
            cur["formats_src"] = "IN_FORMATS"
            cur["formats"] = parse_pairs(s.partition("): ")[2])
        elif s.startswith("IN_FORMATS:"):
            cur["formats_src"] = "IN_FORMATS " + s.partition(": ")[2]
        elif s.startswith("formats ("):
            cur["formats_src"] = "plane format list (linear only)"
            body = s.partition("): ")[2]
            body = body[len("linear "):] if body.startswith("linear ") else body
            cur["formats"] = [(f, 0) for f in body.split()]
        else:
            raise ParseError("unrecognised line: %r" % line)
    if out["driver"] is None:
        raise ParseError("no driver line in raw section")
    return out


def parse_pairs(body):
    pairs = []
    for tok in body.split():
        fmt, _, mod = tok.partition(":")
        pairs.append((fmt, int(mod, 16) if mod else 0))
    return pairs


def cell(x):
    return str(x).replace("|", "\\|").replace("\n", " ")


def table(header, rows):
    out = ["| " + " | ".join(header) + " |", "|" + "|".join("---" for _ in header) + "|"]
    out += ["| " + " | ".join(cell(c) for c in r) + " |" for r in rows]
    return "\n".join(out)


def prop(obj, name):
    for p in obj["props"]:
        if p["name"] == name:
            return p
    return None


def describe_zpos(p):
    if p is None:
        return "none"
    lo, hi = p["range"] or (p["value"], p["value"])
    if not p["mutable"] or lo == hi:
        return "fixed %d (immutable)" % p["value"] if not p["mutable"] else "fixed %d" % lo
    return "%d..=%d (mutable, current %d)" % (lo, hi, p["value"])


def describe_alpha(p):
    if p is None:
        return "none"
    r = p["range"]
    return "%s%s, current %d" % (("%d..=%d" % r) if r else p["kind"],
                                 "" if p["mutable"] else " immutable", p["value"])


def describe_enum(p):
    if p is None:
        return "none"
    cur = p["value"]
    names = []
    for e in p["enum"] or []:
        n, _, v = e.rpartition("=")
        names.append(n + ("*" if v.isdigit() and int(v) == cur else ""))
    return "[" + ", ".join(names) + "] (* current)"


def describe_rotation(p):
    if p is None:
        return "none"
    bits = [n for b, n in ROTATION_BITS if p["value"] & b]
    return "present, current " + "+".join(bits or [str(p["value"])]) + (
        "" if p["enum"] else "; supported set not listed by probe")


def describe_scaling(plane):
    parts = []
    f = prop(plane, "SCALING_FILTER")
    if f:
        parts.append("SCALING_FILTER " + describe_enum(f))
    if not parts:
        return "no scaling property (KMS has no scale-range property; limits not observable here)"
    return "; ".join(parts) + " (scale limits not observable)"


def mode_summary(modes, limit=6):
    shown = modes[:limit]
    more = len(modes) - len(shown)
    return "; ".join(shown) + ("; +%d more" % more if more > 0 else "") if modes else "none"


def render(data, date, kernel):
    tag = "observed %s, kernel %s" % (date, kernel)
    out = []
    d = data["driver"]
    out.append("#### Device (%s)\n" % tag)
    out.append(table(["Item", "Value", "Tag"], [
        ["driver", "%s %s (date %s): %s" % (d["name"], d["version"], d["date"], d["description"]), tag],
        ["client caps", data["client_caps"] or "not reported", tag]]
        + [["cap " + k, v, tag] for k, v in data["caps"]]))
    out.append("\n#### CRTCs\n")
    out.append(table(["CRTC", "Index", "State / mode", "Active", "VRR", "GAMMA/DEGAMMA/CTM", "Tag"], [
        [c["id"], i, c["mode"] or "inactive", describe_val(prop(c, "ACTIVE")),
         describe_val(prop(c, "VRR_ENABLED")),
         ", ".join(n for n in ("GAMMA_LUT", "DEGAMMA_LUT", "CTM") if prop(c, n)) or "none",
         tag] for i, c in enumerate(data["crtcs"])]))
    out.append("\n#### Connectors and modes\n")
    out.append(table(["Connector", "Interface", "State", "Encoders (possible CRTCs)",
                      "Current encoder", "Modes", "Tag"], [
        [c["id"], c["interface"], c["state"],
         "; ".join("%s (%s)" % (e[0], e[2] or e[1]) for e in c["encoders"]) or "none",
         c["encoder"] or "none", "%d: %s" % (len(c["modes"]), mode_summary(c["modes"])), tag]
        for c in data["connectors"]]))
    out.append("\n#### Planes\n")
    out.append(table(["Plane", "Type", "possible_crtcs", "Attached", "zpos", "alpha",
                      "pixel blend mode", "rotation", "Scaling", "Tag"], [
        [p["id"], p["type"], "%s (%s)" % (p["mask"], p["crtcs"]), p["attached"] or "none",
         describe_zpos(prop(p, "zpos")), describe_alpha(prop(p, "alpha")),
         describe_enum(prop(p, "pixel blend mode")), describe_rotation(prop(p, "rotation")),
         describe_scaling(p), tag] for p in data["planes"]]))
    out.append("\n#### Plane formats and modifiers\n")
    rows = []
    for p in data["planes"]:
        groups = {}
        for f, m in p["formats"]:
            groups.setdefault(m, []).append(f)
        if not groups:
            rows.append([p["id"], p["type"], p["formats_src"] or "none", "-", "-", tag])
        for m, fs in groups.items():
            rows.append([p["id"], p["type"], p["formats_src"], "0x%x" % m,
                         "%d: %s" % (len(fs), " ".join(fs)), tag])
    out.append(table(["Plane", "Type", "Source", "Modifier", "Formats", "Tag"], rows))
    return "\n".join(out) + "\n"


def describe_val(p):
    return "none" if p is None else str(p["value"])


def self_test():
    with open(SAMPLE, encoding="utf-8") as f:
        text = f.read()
    assert "NOT A BOARD CAPTURE" in text, "sample must be marked"
    data = parse(text)
    assert data["driver"]["name"] == "vkms"
    assert len(data["crtcs"]) == 1 and len(data["connectors"]) == 1 and len(data["planes"]) == 3
    assert data["connectors"][0]["modes"] == ["1024x768 @ 60.000 Hz", "800x600 @ 60.317 Hz"]
    assert data["connectors"][0]["encoders"] == [("33", "0b1", "36")]
    p0, p1, p2 = data["planes"]
    assert [p0["type"], p1["type"], p2["type"]] == ["primary", "overlay", "cursor"]
    assert describe_zpos(prop(p0, "zpos")) == "fixed 0 (immutable)"
    assert describe_zpos(prop(p1, "zpos")).startswith("1..=2 (mutable")
    assert prop(p0, "alpha")["range"] == (0, 65535)
    assert len(p0["formats"]) == 4 and ("XR24", 1) in p0["formats"]
    assert p2["formats"] == [("AR24", 0)]
    assert prop(p0, "IN_FENCE_FD")["range"] == (-1, 2147483647)
    assert prop(p0, "pixel blend mode")["enum"][1] == "Pre-multiplied=1"
    md = render(data, "2000-01-01", "0.0.0-test")
    tag = "observed 2000-01-01, kernel 0.0.0-test"
    for section in ("#### CRTCs", "#### Connectors and modes", "#### Planes"):
        assert section in md, section
    for table_row_start, n in (("| 31 |", 2), ("| 33 |", 2), ("| 35 |", 2), ("| 36 |", 1), ("| 34 |", 1)):
        rows = [l for l in md.splitlines() if l.startswith(table_row_start)]
        assert len(rows) >= n, (table_row_start, len(rows))
    body_rows = [l for l in md.splitlines() if l.startswith("| ") and not l.startswith("| ---")
                 and not l.rstrip().endswith("| Tag |")]
    assert body_rows and all(l.rstrip().endswith(tag + " |") for l in body_rows), "tag missing"
    for bad in ("raw\nbogus", "no raw here"):
        try:
            parse(bad)
        except ParseError:
            pass
        else:
            raise AssertionError("should have failed: %r" % bad)
    print("probe-table self-test ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--date", help="capture date for the 'observed' tag (YYYY-MM-DD)")
    ap.add_argument("--kernel", help="kernel version string for the tag")
    ap.add_argument("input", nargs="?", help="probe --raw output (default stdin)")
    a = ap.parse_args()
    if a.self_test:
        self_test()
        return 0
    if not a.date or not a.kernel:
        ap.error("--date and --kernel are required")
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", a.date):
        ap.error("--date must be YYYY-MM-DD")
    if a.input:
        with open(a.input, encoding="utf-8", errors="replace") as f:
            text = f.read()
    else:
        text = sys.stdin.read()
    try:
        data = parse(text)
    except ParseError as e:
        print("probe-table: %s" % e, file=sys.stderr)
        return 1
    sys.stdout.write(render(data, a.date, a.kernel))
    return 0


if __name__ == "__main__":
    sys.exit(main())
