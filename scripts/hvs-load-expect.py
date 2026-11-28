#!/usr/bin/env python3
"""Judge the STEP lines printed by the hvs_load_probe example.

  hvs-load-expect.py [LOG...]

Reads hvs_load_probe output (files, or stdin when no file is given or for
'-'). Every STEP line is one rung of the load ladder:

  STEP n=3 layers=2 size=360x640 src=720x1280 scale=0.5000 test_only=ok
       commit=ok latched=118/120 late=1 missing=2 underrun_before=0
       underrun_after=3 dlist_words=21 dlist=0:21,1:off,2:off

(one line in the log). A step drops when test_only=ok and at least one of:
underrun_after > underrun_before, late > 0, missing > 0, commit=err:<errno>.
The judge prints one DROP line per such step, then

  FOUND first_drop step=<n> layers=<N> size=<WxH> src=<WxH> scale=<f> reasons=<...>

for the first one, or NOT_FOUND when no step dropped. It also prints the
first TEST_ONLY rejection (FIRST_REJECT) and whether the underrun counter
was readable at all (UNDERRUN unavailable when every step says na: then a
NOT_FOUND rests on flip counts only).

This judges text. It never touches a device, and a log from a host or vkms
run is no hardware result.

Exit status: 0 = judged (FOUND or NOT_FOUND), 2 = usage or input error
(unreadable file, malformed STEP line, no STEP line, duplicate step number,
or no 'VERDICT hvs_load_probe: DONE' line, i.e. an incomplete run).
Python 3, standard library only.
"""
import argparse
import re
import sys

KEYS = (
    "n", "layers", "size", "src", "scale", "test_only", "commit", "latched",
    "late", "missing", "underrun_before", "underrun_after", "dlist_words", "dlist",
)
SIZE = re.compile(r"^(\d+)x(\d+)$")
FRACTION = re.compile(r"^(\d+)/(\d+)$")
TEST_ONLY = re.compile(r"^(ok|err:-?\d+|skipped:\S+)$")
COMMIT = re.compile(r"^(ok|not-run|err:-?\d+)$")


class InputError(Exception):
    pass


def count(value, key, line_no, allow_na=False):
    if allow_na and value == "na":
        return None
    if not value.isdigit():
        raise InputError("line %d: bad %s=%s" % (line_no, key, value))
    return int(value)


def parse_step(line, line_no):
    fields = {}
    for token in line.split()[1:]:
        if "=" not in token:
            raise InputError("line %d: token without '=': %s" % (line_no, token))
        key, value = token.split("=", 1)
        if key in fields:
            raise InputError("line %d: %s= given twice" % (line_no, key))
        fields[key] = value
    for key in KEYS:
        if key not in fields:
            raise InputError("line %d: STEP without %s=" % (line_no, key))
    for key in ("size", "src"):
        if not SIZE.match(fields[key]):
            raise InputError("line %d: bad %s=%s" % (line_no, key, fields[key]))
    if not TEST_ONLY.match(fields["test_only"]):
        raise InputError("line %d: bad test_only=%s" % (line_no, fields["test_only"]))
    if not COMMIT.match(fields["commit"]):
        raise InputError("line %d: bad commit=%s" % (line_no, fields["commit"]))
    latched = FRACTION.match(fields["latched"])
    if not latched:
        raise InputError("line %d: bad latched=%s" % (line_no, fields["latched"]))
    try:
        scale = float(fields["scale"])
    except ValueError:
        raise InputError("line %d: bad scale=%s" % (line_no, fields["scale"]))
    return {
        "n": count(fields["n"], "n", line_no),
        "layers": count(fields["layers"], "layers", line_no),
        "size": fields["size"],
        "src": fields["src"],
        "scale": scale,
        "scale_text": fields["scale"],
        "test_only": fields["test_only"],
        "commit": fields["commit"],
        "latched": (int(latched.group(1)), int(latched.group(2))),
        "late": count(fields["late"], "late", line_no),
        "missing": count(fields["missing"], "missing", line_no),
        "u0": count(fields["underrun_before"], "underrun_before", line_no, True),
        "u1": count(fields["underrun_after"], "underrun_after", line_no, True),
        "dlist_words": count(fields["dlist_words"], "dlist_words", line_no, True),
    }


def reasons(step):
    """Why a step drops; empty when it does not (or TEST_ONLY did not pass)."""
    if step["test_only"] != "ok":
        return []
    why = []
    if step["u0"] is not None and step["u1"] is not None and step["u1"] > step["u0"]:
        why.append("underrun:+%d" % (step["u1"] - step["u0"]))
    if step["late"] > 0:
        why.append("late:%d" % step["late"])
    if step["missing"] > 0:
        why.append("missing:%d" % step["missing"])
    if step["commit"].startswith("err:"):
        why.append("commit:" + step["commit"])
    return why


def where(step):
    return "step=%d layers=%d size=%s src=%s scale=%s" % (
        step["n"], step["layers"], step["size"], step["src"], step["scale_text"])


def read_lines(paths):
    if not paths:
        paths = ["-"]
    lines = []
    for path in paths:
        try:
            if path == "-":
                lines.extend(sys.stdin.read().splitlines())
            else:
                with open(path, encoding="utf-8", errors="replace") as f:
                    lines.extend(f.read().splitlines())
        except OSError as e:
            raise InputError("cannot read %s: %s" % (path, e))
    return lines


def judge(lines):
    steps = []
    seen = set()
    done = False
    summary = None
    for line_no, raw in enumerate(lines, 1):
        line = raw.strip()
        if line.startswith("STEP "):
            step = parse_step(line, line_no)
            if step["n"] in seen:
                raise InputError("line %d: step %d given twice" % (line_no, step["n"]))
            seen.add(step["n"])
            steps.append(step)
        elif line.startswith("SUMMARY "):
            summary = dict(t.split("=", 1) for t in line.split()[1:] if "=" in t)
        elif line.startswith("VERDICT hvs_load_probe: DONE"):
            done = True
    if not steps:
        raise InputError("no STEP line")
    if not done:
        raise InputError("no 'VERDICT hvs_load_probe: DONE' line: the run is incomplete")

    out = []
    ok = sum(1 for s in steps if s["test_only"] == "ok")
    err = sum(1 for s in steps if s["test_only"].startswith("err:"))
    skipped = len(steps) - ok - err
    drops = [(s, reasons(s)) for s in steps if reasons(s)]
    out.append("STEPS total=%d test_only_ok=%d test_only_err=%d skipped=%d drop_steps=%d"
               % (len(steps), ok, err, skipped, len(drops)))
    if all(s["u0"] is None or s["u1"] is None for s in steps):
        out.append("UNDERRUN unavailable (no step has both counts; drops rest on flips only)")
    words = [s["dlist_words"] for s in steps if s["dlist_words"] is not None]
    if words:
        out.append("DLIST max_words=%d" % max(words))
    reject = next((s for s in steps if s["test_only"].startswith("err:")), None)
    if reject:
        out.append("FIRST_REJECT %s errno=%s" % (where(reject), reject["test_only"][4:]))
    else:
        out.append("FIRST_REJECT none")
    for step, why in drops:
        out.append("DROP %s reasons=%s" % (where(step), ",".join(why)))
    first = drops[0] if drops else None
    if summary is not None:
        probe_first = summary.get("first_drop")
        mine = str(first[0]["n"]) if first else "none"
        if probe_first is not None and probe_first != mine:
            out.append("NOTE probe SUMMARY first_drop=%s, judged %s" % (probe_first, mine))
    if first:
        out.append("FOUND first_drop %s reasons=%s" % (where(first[0]), ",".join(first[1])))
    else:
        out.append("NOT_FOUND")
    return out


def main():
    parser = argparse.ArgumentParser(
        description="Judge hvs_load_probe STEP lines: first step that drops while "
                    "TEST_ONLY passes.")
    parser.add_argument("logs", nargs="*", help="probe logs (default stdin, '-' for stdin)")
    args = parser.parse_args()
    try:
        out = judge(read_lines(args.logs))
    except InputError as e:
        print("ERROR %s" % e)
        return 2
    print("\n".join(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
