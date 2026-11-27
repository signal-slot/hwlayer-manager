#!/usr/bin/env python3
"""Turn scale_probe, kms_sweep and profile_check lines into markdown tables.

Usage:
  sweep-summary.py [--json] [FILE ...]

Reads stdin when no file is given (or for '-'). Lines starting with '#' and
unknown lines are ignored. Line kinds (key=value fields, see the module docs
of crates/composition-drm/examples/{scale_probe,kms_sweep,profile_check}.rs):
  RESULT, COMBO, COMBO_MAX, MODIFIER, REPLACE, CHECK, AGREEMENT.
Exit 0 ok, 2 input error. Python 3 standard library only.
"""
import argparse
import json
import re
import sys

KV_RE = re.compile(r"(\w+)=(.*?)(?= \w+=|$)")
KINDS = ("RESULT", "COMBO_MAX", "COMBO", "MODIFIER", "REPLACE", "CHECK", "AGREEMENT")


class InputError(Exception):
    pass


def parse_line(line):
    line = line.strip()
    if not line or line.startswith("#"):
        return None
    head, _, rest = line.partition(" ")
    if head not in KINDS:
        return None
    rest = rest.strip()
    rec = {"line": head}
    if head == "CHECK":
        # CHECK <name> planner=...: the name is positional
        name, _, rest = rest.partition(" ")
        rec["name"] = name
    for k, v in KV_RE.findall(rest):
        rec[k] = v.strip()
    return rec


def read_all(paths):
    recs = []
    for p in paths or ["-"]:
        try:
            if p == "-":
                text = sys.stdin.read()
            else:
                with open(p, encoding="utf-8") as f:
                    text = f.read()
        except (OSError, UnicodeDecodeError) as e:
            raise InputError(str(e))
        for line in text.splitlines():
            r = parse_line(line)
            if r:
                recs.append(r)
    return recs


def num_key(s):
    m = re.match(r"-?\d+", s or "")
    return (0, int(m.group())) if m else (1, 0)


def modifier_flag(r):
    listed, t = r.get("listed"), r.get("test_only", "")
    if listed == "yes" and t not in ("ok", "not reached"):
        return "listed but refused"
    if listed == "no" and t == "ok":
        return "unlisted but accepted"
    return ""


def check_rank(r):
    return {"planner-looser": 0, "planner-stricter": 1, "no": 2}.get(r.get("agree"), 3)


def build(recs):
    def by(n):
        return [r for r in recs if r["line"] == n]

    combos = sorted(by("COMBO"), key=lambda r: (num_key(r.get("k")), r.get("planes", ""), r.get("scale", "")))
    mods = []
    for r in by("MODIFIER"):
        r = dict(r)
        r["flag"] = modifier_flag(r)
        mods.append(r)
    checks = sorted(by("CHECK"), key=lambda r: (check_rank(r), r.get("name", "")))
    return {
        "scale": sorted(by("RESULT"), key=lambda r: num_key(r.get("plane"))),
        "combos": combos,
        "combo_max": by("COMBO_MAX"),
        "modifiers": sorted(mods, key=lambda r: (num_key(r.get("plane")), r.get("format", ""), r.get("modifier", ""))),
        "replace": by("REPLACE"),
        "checks": checks,
        "agreement": by("AGREEMENT"),
    }


def esc(v):
    return str(v).replace("|", "\\|")


def table(title, cols, rows):
    if not rows:
        return ""
    out = ["### " + title, "", "| " + " | ".join(h for h, _ in cols) + " |",
           "|" + "|".join("---" for _ in cols) + "|"]
    for r in rows:
        out.append("| " + " | ".join(esc(r.get(k, "")) for _, k in cols) + " |")
    return "\n".join(out) + "\n"


def render(d):
    parts = [
        table("Scale limits per plane",
              [("plane", "plane"), ("kind", "kind"), ("format", "format"),
               ("max upscale h", "max_upscale_h"), ("max upscale v", "max_upscale_v"),
               ("max upscale both", "max_upscale_both"), ("downscale", "downscale"),
               ("min downscale", "min_downscale")], d["scale"]),
        table("Combination results per k",
              [("k", "k"), ("planes", "planes"), ("scale", "scale"), ("result", "result")], d["combos"]),
        table("Accepted combination maximum", [("accepted k", "accepted_k")], d["combo_max"]),
        table("Modifier acceptance",
              [("plane", "plane"), ("format", "format"), ("modifier", "modifier"),
               ("listed", "listed"), ("addfb", "addfb"), ("test_only", "test_only"),
               ("flag", "flag")], d["modifiers"]),
        table("Pending-commit replacement",
              [("second commit", "second"), ("events", "events")], d["replace"]),
        table("Planner vs TEST_ONLY",
              [("check", "name"), ("planner", "planner"), ("test_only", "test_only"),
               ("agree", "agree")], d["checks"]),
        table("Agreement totals",
              [("total", "total"), ("agree", "agree"), ("stricter", "stricter"),
               ("looser", "looser")], d["agreement"]),
    ]
    return "\n".join(p for p in parts if p) or "No sweep lines found.\n"


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--json", action="store_true", help="print JSON instead of markdown")
    ap.add_argument("files", nargs="*")
    args = ap.parse_args(argv)
    try:
        recs = read_all(args.files)
    except InputError as e:
        print("sweep-summary: %s" % e, file=sys.stderr)
        return 2
    d = build(recs)
    if args.json:
        print(json.dumps(d, indent=2, sort_keys=True))
    else:
        sys.stdout.write(render(d))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
