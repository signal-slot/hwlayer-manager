#!/usr/bin/env python3
"""Build the WP3 Linux vs Zephyr comparison table from two run logs.

Usage:
  m2-compare-report.py [--json] LINUX_LOG ZEPHYR_LOG

Each log is key=value lines (format in m2-compare-report-README.md):
  RUN date=YYYY-MM-DD side=linux|zephyr [stage=A|B] [commit=...] [tool=...]
  METRIC id=M-1.median name="frame interval median" unit=ms value=16.67
Lines starting with '#' and unknown lines are ignored. Output is a Markdown
table: metric, Linux, Zephyr, difference (Zephyr minus Linux). A metric absent
from a log, or with an empty value, prints 'not measured'. The table header
carries 'measured on <date>' from the logs' RUN date lines.
Exit 0 ok, 2 input error (unreadable file, missing/bad date, wrong side).
Python 3 standard library only.
"""
import argparse
import json
import re
import sys

KV_RE = re.compile(r'(\w+)=("[^"]*"|.*?)(?= \w+=|$)')
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
NUM_RE = re.compile(r"^[-+]?\d+(\.\d+)?$")
NOT_MEASURED = "not measured"


class InputError(Exception):
    pass


def parse_fields(rest):
    out = {}
    for k, v in KV_RE.findall(rest.strip()):
        out[k] = v[1:-1] if len(v) >= 2 and v[0] == '"' and v[-1] == '"' else v.strip()
    return out


def read_log(path, side):
    try:
        with open(path, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except OSError as e:
        raise InputError("cannot read %s: %s" % (path, e))
    dates, metrics, order = [], {}, []
    for line in lines:
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        kind, _, rest = line.partition(" ")
        if kind == "RUN":
            f = parse_fields(rest)
            if f.get("side", side) != side:
                raise InputError("%s: RUN side=%s, expected %s" % (path, f["side"], side))
            d = f.get("date", "")
            if not DATE_RE.match(d):
                raise InputError("%s: RUN line without a valid date=YYYY-MM-DD" % path)
            if d not in dates:
                dates.append(d)
        elif kind == "METRIC":
            f = parse_fields(rest)
            if "id" not in f:
                raise InputError("%s: METRIC line without id=" % path)
            if f["id"] not in metrics:
                order.append(f["id"])
            metrics[f["id"]] = f
    if not dates:
        raise InputError("%s: no RUN line with a date" % path)
    return {"dates": dates, "metrics": metrics, "order": order}


def cell(m):
    if m is None or not m.get("value", "").strip():
        return NOT_MEASURED
    v = m["value"].strip()
    return v + " " + m["unit"] if m.get("unit") and NUM_RE.match(v) else v


def diff(l, z):
    if l is None or z is None:
        return NOT_MEASURED
    lv, zv = l.get("value", "").strip(), z.get("value", "").strip()
    if not NUM_RE.match(lv) or not NUM_RE.match(zv):
        return NOT_MEASURED if NOT_MEASURED in (cell(l), cell(z)) else "n/a"
    if l.get("unit", "") != z.get("unit", ""):
        return "units differ"
    d = float(zv) - float(lv)
    s = ("%+.4f" % d).rstrip("0").rstrip(".")
    return s + (" " + l["unit"] if l.get("unit") else "")


def build(lin, zep):
    ids = list(lin["order"]) + [i for i in zep["order"] if i not in lin["metrics"]]
    rows = []
    for i in ids:
        l, z = lin["metrics"].get(i), zep["metrics"].get(i)
        name = (l or z).get("name") or i
        rows.append({"id": i, "name": name, "linux": cell(l), "zephyr": cell(z), "diff": diff(l, z)})
    return rows


def render(lin, zep, rows):
    ld, zd = ", ".join(lin["dates"]), ", ".join(zep["dates"])
    when = ld if ld == zd else "Linux %s, Zephyr %s" % (ld, zd)
    out = ["## Linux vs Zephyr, same scene (measured on %s)" % when, ""]
    out += ["| Metric | Linux | Zephyr | Difference (Zephyr - Linux) |", "| --- | --- | --- | --- |"]
    for r in rows:
        out.append("| %s %s | %s | %s | %s |" % (r["id"], r["name"], r["linux"], r["zephyr"], r["diff"]))
    return "\n".join(out) + "\n"


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--json", action="store_true")
    ap.add_argument("linux")
    ap.add_argument("zephyr")
    a = ap.parse_args(argv)
    try:
        lin, zep = read_log(a.linux, "linux"), read_log(a.zephyr, "zephyr")
    except InputError as e:
        print("error: %s" % e, file=sys.stderr)
        return 2
    rows = build(lin, zep)
    if a.json:
        print(json.dumps({"linux_dates": lin["dates"], "zephyr_dates": zep["dates"], "rows": rows}, indent=1))
    else:
        sys.stdout.write(render(lin, zep, rows))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
