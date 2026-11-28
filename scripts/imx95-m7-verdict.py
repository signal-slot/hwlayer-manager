#!/usr/bin/env python3
"""Final verdict line from a captured i.MX95 M7 log (reduced-scope bring-up).

usage: python3 -I imx95-m7-verdict.py --candidate A|B [--expect FILE] [LOG]
       (LOG may be a file or stdin; no board is contacted)

Reads the `M7P:` (candidate A, register probe) or `M7E:` (candidate B, engine
plan) lines of a log from scripts/imx95-m7-log.sh or a console capture, and
compares them with the expected set (--expect FILE). Expect file lines, '#'
comments allowed:
  A:  reg <name> [<value-hex>]      value optional (any value accepted)
  B:  plan <output> <plane> <buffer> <x> <y> <w> <h>
      (the host plan; compared as a set of lines)
Prints one line: `M7-VERDICT: pass|fail|incomplete <detail>`.
exit: 0 pass, 1 fail (mismatch), 2 incomplete (missing lines), 64 usage.
Without --expect, A checks the log's own consistency only; B cannot show
plan equality and is reported incomplete (plan-unchecked).
"""
import re
import sys

LINE = re.compile(r"\b(M7[PE]):[ \t]*(.*?)\s*$")


def usage():
    sys.stderr.write(__doc__.split("\n\n")[0] + "\n")
    sys.exit(64)


def read_lines(path):
    try:
        if path is None:
            return sys.stdin.read().splitlines()
        with open(path, encoding="utf-8", errors="replace") as f:
            return f.read().splitlines()
    except OSError as e:
        sys.stderr.write("imx95-m7-verdict: %s\n" % e.strerror)
        sys.exit(64)


def tagged(lines, tag):
    out = []
    for ln in lines:
        m = LINE.search(ln)
        if m and m.group(1) == tag:
            out.append(" ".join(m.group(2).split()))
    return out


def read_expect(path, kind):
    exp = []
    for ln in read_lines(path):
        ln = ln.split("#", 1)[0].strip()
        f = ln.split()
        if f and f[0] == kind and len(f) > 1:
            exp.append(" ".join(f))
    return exp


def emit(status, code, detail):
    print("M7-VERDICT: %s %s" % (status, detail))
    sys.exit(code)


def judge_a(lines, expect):
    regs, summary, bad, missing = {}, [], [], []
    for t in tagged(lines, "M7P"):
        f = t.split()
        if f[:1] == ["reg"] and len(f) >= 3:
            if f[1] in regs and regs[f[1]] != f[2:]:
                bad.append("reg:%s:conflict" % f[1])
            regs[f[1]] = f[2:]
        elif f[:2] == ["verdict", "A"] and len(f) >= 3:
            summary.append(f[2:])
    if not regs and not summary:
        emit("incomplete", 2, "A missing=all (no M7P lines)")
    faults = [n for n, v in regs.items() if v[0] == "FAULT"]
    for e in expect:
        f = e.split()
        name = f[1]
        if name not in regs:
            missing.append("reg:" + name)
        elif len(f) > 2 and regs[name] != f[2:]:
            bad.append("reg:%s:got=%s,want=%s" % (name, "_".join(regs[name]), "_".join(f[2:])))
    if any(s != summary[0] for s in summary):
        bad.append("verdict:conflict")
    if summary:
        s = summary[-1]
        if s[0] == "READABLE" and faults:
            bad.append("verdict:READABLE-but-faults=%s" % ",".join(sorted(faults)))
        elif s[0] == "BLOCKED":
            if len(s) < 2 or not s[1].isdigit() or int(s[1]) != len(faults):
                bad.append("verdict:BLOCKED-count=%s,faults=%d" % (s[1] if len(s) > 1 else "?", len(faults)))
        elif s[0] == "HUNG":
            missing.append("summary:HUNG")
        elif s[0] != "READABLE":
            bad.append("verdict:unknown=" + s[0])
    else:
        missing.append("summary:verdict-A")
    if bad:
        emit("fail", 1, "A mismatch=" + ",".join(bad))
    if missing:
        emit("incomplete", 2, "A missing=" + ",".join(missing))
    emit("pass", 0, "A %s regs=%d faults=%d" % (summary[-1][0].lower(), len(regs), len(faults)))


def judge_b(lines, expect, have_expect):
    t = tagged(lines, "M7E")
    if not t:
        emit("incomplete", 2, "B missing=all (no M7E lines)")
    start = [x for x in t if x.startswith("engine start ok")]
    plans = [x for x in t if x.startswith("plan ")]
    backend = [x for x in t if x.startswith("backend=fake")]
    fails = [x for x in t if x.startswith("verdict B FAIL")]
    missing, bad = [], []
    if fails:
        bad.append("m7-reported:" + fails[-1][len("verdict B FAIL"):].strip().replace(" ", "_"))
    if not start:
        missing.append("engine-start")
    if not plans:
        missing.append("plan")
    if not backend:
        missing.append("backend=fake")
    if have_expect:
        for e in expect:
            if e not in plans:
                (bad if plans else missing).append("plan:" + e.replace(" ", "_"))
        for p in plans:
            if p not in expect:
                bad.append("unexpected:" + p.replace(" ", "_"))
    elif plans:
        missing.append("plan-unchecked")
    if bad:
        emit("fail", 1, "B mismatch=" + ",".join(bad))
    if missing:
        emit("incomplete", 2, "B missing=" + ",".join(missing))
    emit("pass", 0, "B plan-equal=yes lines=%d" % len(plans))


def main(argv):
    cand = exp = log = None
    i = 0
    while i < len(argv):
        a = argv[i]
        if a in ("--candidate", "--expect") and i + 1 < len(argv):
            if a == "--candidate":
                cand = argv[i + 1]
            else:
                exp = argv[i + 1]
            i += 2
        elif a.startswith("-") or log is not None:
            usage()
        else:
            log = a
            i += 1
    if cand not in ("A", "B"):
        usage()
    lines = read_lines(log)
    expect = read_expect(exp, "reg" if cand == "A" else "plan") if exp else []
    if cand == "A":
        judge_a(lines, expect)
    else:
        judge_b(lines, expect, exp is not None)


main(sys.argv[1:])
