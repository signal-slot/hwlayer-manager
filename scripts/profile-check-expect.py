#!/usr/bin/env python3
"""Judge the CHECK ... agree= lines printed by the profile_check example.

  profile-check-expect.py [OUTPUT|-] [--allow VALUE[,VALUE...]]
                          [--allow-disagree N] [--markdown]

Reads profile_check output (file or stdin). Only lines starting with `CHECK `
are judged, others are skipped. A CHECK line is
  CHECK <name> planner=.. test_only=.. agree=<yes|no|planner-stricter|planner-looser> via=.. candidates=N [note=..]
agree=yes is agreement; no, planner-stricter and planner-looser are
disagreements. --allow names disagreement values that are tolerated (for
example planner-stricter); --allow-disagree N tolerates N further ones.
Prints a count per agree value, each disallowed disagreement, and one
`VERDICT: PASS` or `VERDICT: FAIL` line.
Exit status: 0 = PASS, 1 = FAIL, 2 = usage or input error.
Python 3, standard library only.
"""
import argparse
import re
import sys

VALUES = ("yes", "no", "planner-stricter", "planner-looser")
FIELD = re.compile(r"(\w+)=")


def parse_line(line):
    """Return (name, fields) for a CHECK line, else None."""
    m = re.match(r"CHECK\s+(\S+)\s*(.*)$", line.rstrip())
    if not m:
        return None
    rest = m.group(2)
    # Values may hold spaces (test_only=not reached): a value runs to the next key=.
    marks = [x for x in FIELD.finditer(rest)
             if x.start() == 0 or rest[x.start() - 1] == " "]
    fields = {}
    for i, x in enumerate(marks):
        end = marks[i + 1].start() if i + 1 < len(marks) else len(rest)
        fields[x.group(1)] = rest[x.end():end].strip()
    if "agree" not in fields:
        return None
    return m.group(1), fields


def parse(text):
    return [r for r in map(parse_line, text.splitlines()) if r]


def cell(s):
    return str(s).replace("|", "\\|")


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("input", nargs="?", default="-")
    ap.add_argument("--allow", default="", metavar="VALUES")
    ap.add_argument("--allow-disagree", type=int, default=0, metavar="N")
    ap.add_argument("--markdown", action="store_true")
    try:
        a = ap.parse_args(argv)
    except SystemExit as e:
        return 2 if e.code else 0
    allowed = {v for v in a.allow.split(",") if v}
    if not allowed <= set(VALUES[1:]) or a.allow_disagree < 0:
        print("profile-check-expect: bad --allow/--allow-disagree (values: %s)"
              % ", ".join(VALUES[1:]), file=sys.stderr)
        return 2
    try:
        if a.input == "-":
            text = sys.stdin.read()
        else:
            with open(a.input, encoding="utf-8", errors="replace") as fh:
                text = fh.read()
    except OSError as e:
        print("profile-check-expect: %s" % e, file=sys.stderr)
        return 2

    rows = parse(text)
    counts = {v: 0 for v in VALUES}
    bad = []
    for name, f in rows:
        v = f["agree"]
        counts[v] = counts.get(v, 0) + 1
        if v != "yes" and v not in allowed:
            bad.append((name, f))
    reasons = []
    if not rows:
        reasons.append("no CHECK lines")
    if len(bad) > a.allow_disagree:
        reasons.append("%d disallowed disagreement(s) (allowed %d)"
                       % (len(bad), a.allow_disagree))

    if a.markdown:
        print("| agree | count |")
        print("|---|---|")
        for v, n in counts.items():
            print("| %s | %d |" % (cell(v), n))
        if bad:
            print()
            print("| check | agree | planner | test_only | note |")
            print("|---|---|---|---|---|")
            for name, f in bad:
                cols = (name, f["agree"], f.get("planner", ""),
                        f.get("test_only", ""), f.get("note", ""))
                print("| %s |" % " | ".join(cell(x) for x in cols))
        print()
    else:
        print("total=%d %s" % (len(rows), " ".join("%s=%d" % kv for kv in counts.items())))
        for name, f in bad:
            print("DISAGREE %s agree=%s planner=%s test_only=%s" % (
                name, f["agree"], f.get("planner", ""), f.get("test_only", "")))
    if reasons:
        print("VERDICT: FAIL (%s)" % "; ".join(reasons))
        return 1
    print("VERDICT: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
