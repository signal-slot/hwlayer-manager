#!/usr/bin/env python3
"""Judge a saved zpos_probe log: one verdict per case, then the zpos scope.

  zpos-probe-expect.py [ZPOS_PROBE_OUTPUT|-]

Reads zpos_probe stdout (file or stdin). Prints, for each of the cases
distinct, equal, cross_equal:

  ZPOS <case> accepted | refused(<errno>) | not_run

and then  ZPOS scope=global|per_crtc|undetermined  (the rule is in
zpos-probe-expect-README.md and is an inference). A case is not_run when its
line is absent (cross_equal with one active CRTC) or says result=skipped.
If a case appears twice the last line wins.
Exit status: 0 = ok, 2 = input error (unreadable file, malformed result).
Python 3, standard library only.
"""
import re
import sys

CASES = ("distinct", "equal", "cross_equal")
LINE = re.compile(r"^ZPOS\s+case=(\S+)(.*)$")
RESULT = re.compile(r"\bresult=(\S+)")
ERRNO = re.compile(r"^(E[A-Z0-9]+|errno-?\d+)$")


def parse(text):
    """Return {case: 'accepted' | 'refused(E)' | 'not_run'}."""
    verdicts = {}
    for n, raw in enumerate(text.splitlines(), 1):
        m = LINE.match(raw.strip())
        if not m or m.group(1) not in CASES:
            continue
        r = RESULT.search(m.group(2))
        if not r:
            raise ValueError("line %d: no result= in %r" % (n, raw))
        value = r.group(1)
        if value == "ok":
            verdicts[m.group(1)] = "accepted"
        elif value == "skipped":
            verdicts[m.group(1)] = "not_run"
        elif ERRNO.match(value):
            verdicts[m.group(1)] = "refused(%s)" % value
        else:
            raise ValueError("line %d: unknown result %r" % (n, value))
    return {c: verdicts.get(c, "not_run") for c in CASES}


def scope(v):
    """The inference rule (README, 'Rule')."""
    distinct, equal, cross = (v[c] for c in CASES)
    if distinct != "accepted":
        return "undetermined"
    if not equal.startswith("refused"):
        return "undetermined"
    if cross == "accepted":
        return "per_crtc"
    if cross.startswith("refused"):
        return "global"
    return "undetermined"


def main(argv):
    if len(argv) > 2 or (len(argv) == 2 and argv[1] in ("-h", "--help")):
        print(__doc__, file=sys.stderr)
        return 2
    try:
        if len(argv) == 1 or argv[1] == "-":
            text = sys.stdin.read()
        else:
            with open(argv[1], encoding="utf-8", errors="replace") as f:
                text = f.read()
        v = parse(text)
    except (OSError, ValueError) as e:
        print("zpos-probe-expect: %s" % e, file=sys.stderr)
        return 2
    for c in CASES:
        print("ZPOS %s %s" % (c, v[c]))
    print("ZPOS scope=%s" % scope(v))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
