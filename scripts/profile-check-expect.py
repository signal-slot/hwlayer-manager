#!/usr/bin/env python3
"""Judge profile_check AGREE/DISAGREE lines.

  profile-check-expect.py [OUTPUT|-] [--allow-disagree N] [--expect-profile NAME]
                          [--markdown]

Reads profile_check output (file or stdin). Lines that start with neither
AGREE nor DISAGREE are skipped. Each judged line is `AGREE|DISAGREE` followed
by key=value tokens (profile=, frame=, planner=, kernel= or errno=, ...);
values may be quoted, unknown keys are kept and shown.
Prints a per-profile summary and one `VERDICT: PASS` or `VERDICT: FAIL` line.
Exit status: 0 = PASS, 1 = FAIL, 2 = usage or input error.
Python 3, standard library only.
"""
import argparse
import re
import shlex
import sys

KEYS = ("profile", "frame", "planner", "kernel", "errno")


def parse_line(line):
    """Return (word, fields) for an AGREE/DISAGREE line, else None."""
    m = re.match(r"(AGREE|DISAGREE)(?:\s+(.*))?$", line.rstrip())
    if not m:
        return None
    fields = {}
    rest = m.group(2) or ""
    try:
        toks = shlex.split(rest)
    except ValueError:
        toks = rest.split()
    for t in toks:
        k, eq, v = t.partition("=")
        if eq and k:
            fields[k] = v.strip("\"'")
    return m.group(1), fields


def parse(text):
    return [r for r in map(parse_line, text.splitlines()) if r]


def summarize(rows):
    per = {}
    for word, f in rows:
        c = per.setdefault(f.get("profile", "?"), [0, 0])
        c[0 if word == "AGREE" else 1] += 1
    return per


def cell(s):
    return str(s).replace("|", "\\|")


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("input", nargs="?", default="-")
    ap.add_argument("--allow-disagree", type=int, default=0, metavar="N")
    ap.add_argument("--expect-profile", metavar="NAME")
    ap.add_argument("--markdown", action="store_true")
    try:
        a = ap.parse_args(argv)
    except SystemExit as e:
        return 2 if e.code else 0
    if a.allow_disagree < 0:
        print("profile-check-expect: --allow-disagree must be >= 0", file=sys.stderr)
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
    per = summarize(rows)
    agree = sum(c[0] for c in per.values())
    dis = sum(c[1] for c in per.values())
    reasons = []
    if not rows:
        reasons.append("no AGREE/DISAGREE lines")
    if dis > a.allow_disagree:
        reasons.append("%d DISAGREE (allowed %d)" % (dis, a.allow_disagree))
    if a.expect_profile and a.expect_profile not in per:
        reasons.append("no line for profile %s" % a.expect_profile)

    bad = [f for w, f in rows if w == "DISAGREE"]
    if a.markdown:
        print("| profile | AGREE | DISAGREE |")
        print("|---|---|---|")
        for p in sorted(per):
            print("| %s | %d | %d |" % (cell(p), per[p][0], per[p][1]))
        if bad:
            print()
            print("| profile | frame | planner | kernel/errno | other |")
            print("|---|---|---|---|---|")
            for f in bad:
                ke = f.get("kernel") or ("errno=" + f["errno"] if "errno" in f else "")
                other = " ".join("%s=%s" % (k, v) for k, v in f.items() if k not in KEYS)
                cols = (f.get("profile", "?"), f.get("frame", ""), f.get("planner", ""), ke, other)
                print("| %s |" % " | ".join(cell(x) for x in cols))
        print()
    else:
        for p in sorted(per):
            print("profile=%s agree=%d disagree=%d" % (p, per[p][0], per[p][1]))
        for f in bad:
            print("DISAGREE " + " ".join("%s=%s" % kv for kv in f.items()))
        print("total agree=%d disagree=%d" % (agree, dis))
    if reasons:
        print("VERDICT: FAIL (%s)" % "; ".join(reasons))
        return 1
    print("VERDICT: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
