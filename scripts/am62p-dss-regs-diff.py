#!/usr/bin/env python3
"""Diff two saved AM62P DSS snapshots (before/after a scene).

Run: python3 -I scripts/am62p-dss-regs-diff.py [--json] BEFORE AFTER
Both files are read with the parser in am62p-dss-regs.py (loaded from this
script's own directory) and must have the same format (drm-state or regdump).
Prints the plane and video-port fields that changed. Exit status: 0 done
(with or without changes), 2 unreadable file, unknown or mixed format, usage.
Host tool: parses text only, makes no hardware claim.
"""
import importlib.util
import json
import os
import sys

_here = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location(
    "am62p_dss_regs", os.path.join(_here, "am62p-dss-regs.py"))
regs = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(regs)

PLANE_FIELDS = {
    "drm-state": ["enabled", "crtc", "fb", "format", "size", "position",
                  "zorder", "alpha"],
    "regdump": ["enabled", "format_code", "size", "position", "zorder", "vp",
                "layer_enabled", "alpha"],
}
VP_FIELDS = {
    "drm-state": ["enable", "active", "mode", "plane_mask", "go"],
    "regdump": ["enable", "go", "size"],
}


def load(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        lines = f.read().splitlines()
    fmt = regs.detect(lines)
    if fmt is None:
        raise ValueError("%s: unknown input format" % path)
    if fmt == "drm-state":
        return regs.parse_drm_state(lines)
    return regs.parse_regdump(lines)


def key(row, fmt):
    return row["object_id"] if fmt == "drm-state" else row["id"]


def label(row, fmt):
    return row["name"] if fmt == "drm-state" else str(row["id"])


def diff_rows(a_rows, b_rows, fmt, fields):
    a = dict((key(r, fmt), r) for r in a_rows)
    b = dict((key(r, fmt), r) for r in b_rows)
    out = []
    for k in sorted(set(a) | set(b)):
        ra, rb = a.get(k), b.get(k)
        ent = {"id": k, "name": label(ra or rb, fmt), "fields": {}}
        if ra is None or rb is None:
            ent["presence"] = "added" if ra is None else "removed"
        for f in fields:
            va = None if ra is None else ra.get(f)
            vb = None if rb is None else rb.get(f)
            if va != vb:
                ent["fields"][f] = {"before": va, "after": vb}
        if ent["fields"] or "presence" in ent:
            out.append(ent)
    return out


def diff(before, after):
    fmt = before["format"]
    return {
        "format": fmt,
        "planes": diff_rows(before["planes"], after["planes"], fmt,
                            PLANE_FIELDS[fmt]),
        "vps": diff_rows(before["vps"], after["vps"], fmt, VP_FIELDS[fmt]),
    }


def show(name, v):
    # the parser's cell() prints every int pair as WxH; a position is X,Y
    if name == "position" and v is not None:
        return "%d,%d" % tuple(v)
    return regs.cell(v)


def markdown(d):
    out = ["Source format: " + d["format"], ""]
    for title, noun, rows in (("Planes", "plane", d["planes"]),
                              ("Video ports", "vp", d["vps"])):
        out += ["### %s changed" % title, ""]
        lines = []
        for e in rows:
            if "presence" in e:
                lines.append("| %s | (%s) | | |" % (e["name"], e["presence"]))
            for f, v in e["fields"].items():
                lines.append("| %s | %s | %s | %s |" % (
                    e["name"], f, show(f, v["before"]), show(f, v["after"])))
        if lines:
            out += ["| %s | field | before | after |" % noun,
                    "|---|---|---|---|"] + lines
        else:
            out.append("(no change)")
        out.append("")
    return "\n".join(out)


def main(argv):
    args = argv[1:]
    as_json = "--json" in args
    args = [a for a in args if a != "--json"]
    if len(args) != 2 or any(a.startswith("-") for a in args):
        sys.stderr.write("usage: am62p-dss-regs-diff.py [--json] BEFORE AFTER\n")
        return 2
    try:
        before, after = load(args[0]), load(args[1])
    except (OSError, ValueError) as e:
        sys.stderr.write("am62p-dss-regs-diff: %s\n" % e)
        return 2
    if before["format"] != after["format"]:
        sys.stderr.write("am62p-dss-regs-diff: formats differ (%s, %s)\n"
                         % (before["format"], after["format"]))
        return 2
    d = diff(before, after)
    print(json.dumps(d, indent=2, sort_keys=True) if as_json else markdown(d))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
