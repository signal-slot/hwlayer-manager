#!/usr/bin/env python3
"""Judge a saved pending_probe log: flip/latch counts and replace-before-flip.

  pending-probe-expect.py [LOG|-] [--expect-replace accepted|ebusy]
                          [--expect-flips N]

Reads pending_probe stdout (file or stdin) and prints, for what the log holds:

  PENDING commits=<n> flips=<n> latched=<n> missed=<n>     (--flips run)
  PENDING replace result=<accepted|ebusy|other errno|unknown>   (--replace run)
  PENDING truncated=yes ...                                (log ends early)
  EXPECT ok|fail ...                                       (with an --expect-*)

commits = commits the kernel accepted, flips = page-flip events that came
(both from the SUMMARY line), latched = FLIP lines whose commit was accepted
and carried an event, missed = commits without an event. A log without the
SUMMARY (flips) or the VERDICT (replace) line is truncated: the counts come
from the FLIP / A,B lines and the verdict is "fail" when an expectation is
given. The replace result follows the real commit B ("EBUSY" prints `ebusy`,
any other errno prints as read, "A was refused" prints `unknown`).
--expect-flips N needs commits=N, flips=N and missed=0.
Exit status: 0 = ok (or no expectation), 1 = EXPECT fail or truncated,
2 = input error (no pending_probe lines, bad arguments).
Python 3, standard library only.
"""
import re
import sys

FLIP = re.compile(r'^FLIP n=(\d+)\s.*?\bcommit=(\S+)\s.*?\bseq=(\S+)')
SUMMARY = re.compile(
    r'^SUMMARY flips: submitted=(\d+) events=(\d+) missed=(\d+)')
CHECK = re.compile(r'^(A|B) commit=(\S+)')
VERDICT = re.compile(r'^VERDICT replace_pending: (.*)$')
KNOWN = re.compile(r'^(DATE|KERNEL|DEVICE|DRIVER|SETUP|FLIP|SUMMARY|VERDICT|'
                   r'EVENT|A |B |B_TEST_ONLY|PLANE_FB|RESTORE)')


class InputError(Exception):
    pass


def parse(text):
    r = {"flips": [], "summary": None, "ab": {}, "verdict": None,
         "seen": False}
    for line in text.splitlines():
        line = line.strip()
        if KNOWN.match(line):
            r["seen"] = True
        m = FLIP.match(line)
        if m:
            r["flips"].append((m.group(2), m.group(3)))
            continue
        m = SUMMARY.match(line)
        if m:
            r["summary"] = tuple(int(g) for g in m.groups())
            continue
        m = CHECK.match(line)
        if m:
            r["ab"][m.group(1)] = m.group(2)
            continue
        m = VERDICT.match(line)
        if m:
            r["verdict"] = m.group(1).strip()
    return r


def errno_result(e):
    return "ebusy" if e == "EBUSY" else e


def replace_result(r):
    """Return (result, truncated)."""
    v = r["verdict"]
    if v is not None:
        if v == "accepted":
            return "accepted", False
        m = re.match(r'refused errno=(\S+)', v)
        if m:
            return errno_result(m.group(1)), False
        return "unknown", False
    a, b = r["ab"].get("A"), r["ab"].get("B")
    if a is None or b is None:
        return None, True
    if a != "ok":
        return "unknown", True
    return ("accepted" if b == "ok" else errno_result(b)), True


def judge(r, expect_replace, expect_flips):
    out, bad = [], []
    if r["summary"] or r["flips"]:
        ok_flips = [f for f in r["flips"] if f[0] == "ok"]
        latched = sum(1 for _, s in ok_flips if s != "n/a")
        trunc = r["summary"] is None
        if trunc:
            commits, flips = len(ok_flips), latched
            missed = commits - flips
        else:
            commits, flips, missed = r["summary"]
        out.append("PENDING commits=%d flips=%d latched=%d missed=%d"
                   % (commits, flips, latched, missed))
        if trunc:
            out.append("PENDING truncated=yes (no SUMMARY line)")
            bad.append("log truncated")
        if expect_flips is not None and not (
                commits == flips == expect_flips and missed == 0):
            bad.append("flips: want %d, got commits=%d flips=%d missed=%d"
                       % (expect_flips, commits, flips, missed))
    elif expect_flips is not None:
        bad.append("no flips data")
    res, trunc = replace_result(r)
    if res is not None:
        out.append("PENDING replace result=" + res)
    if res is not None and trunc:
        out.append("PENDING truncated=yes (no VERDICT line)")
        bad.append("log truncated")
    if expect_replace is not None:
        if res is None:
            bad.append("no replace data")
        elif res != expect_replace:
            bad.append("replace: want %s, got %s" % (expect_replace, res))
    if expect_replace is not None or expect_flips is not None:
        out.append("EXPECT fail " + "; ".join(bad) if bad else "EXPECT ok")
    return out, bad


def main(argv):
    path, er, ef = "-", None, None
    args = list(argv)
    try:
        while args:
            a = args.pop(0)
            if a == "--expect-replace":
                er = args.pop(0)
                if er not in ("accepted", "ebusy"):
                    raise InputError("--expect-replace: accepted or ebusy")
            elif a == "--expect-flips":
                ef = int(args.pop(0))
            elif a.startswith("--"):
                raise InputError("unknown option " + a)
            else:
                path = a
    except (IndexError, ValueError):
        sys.stderr.write("bad arguments\n")
        return 2
    except InputError as e:
        sys.stderr.write(str(e) + "\n")
        return 2
    try:
        if path == "-":
            text = sys.stdin.read()
        else:
            with open(path) as f:
                text = f.read()
    except OSError as e:
        sys.stderr.write("cannot read input: %s\n" % e)
        return 2
    r = parse(text)
    if not r["seen"]:
        sys.stderr.write("no pending_probe lines in input\n")
        return 2
    out, bad = judge(r, er, ef)
    print("\n".join(out))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
