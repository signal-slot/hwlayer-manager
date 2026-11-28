#!/usr/bin/env python3
"""Judge a mode_test log: the TEST_ONLY modeset of each connector mode.

  mode-test-expect.py [OUTPUT|-] [--expect-min-clock KHZ]

Reads a saved mode_test log (file or stdin; the run's stdout and stderr may
be mixed) and judges the lines crates/composition-drm/examples/mode_test.rs
prints:

  mode WxH@R clock=KHZ test=ok
  mode WxH@R clock=KHZ test=refused errno=E
  mode_test: tested=N ok=M highest_ok_clock=KHZ
  mode_test: DRM master is unavailable: ...

One line per mode line, in log order (E is the numeric errno, so 22 is
EINVAL):

  MODE name=WxH@R clock=KHZ result=ok|E

Then MODE highest_ok_clock=KHZ (largest pixel clock of a passing mode in
kHz, 0 if none passed) and MODE refused=N (count of refused mode lines).
The summary line must agree with the mode lines; DATE, KERNEL, DEVICE,
CONNECTOR and LISTED lines are skipped, so logs of several runs cannot be
mixed (the script reports exit 2 when it sees two summary lines).

A log without a summary line is an unfinished run: the two MODE result lines
then describe only what the log shows so far, one extra line says why,

  MODE incomplete reason=drm-master|no-summary

(`drm-master` when the log holds mode_test's "DRM master is unavailable"
error, exit 2 there means the example could not take DRM master), and no
EXPECT line is printed even with --expect-min-clock. With
--expect-min-clock KHZ a completed run prints a final `EXPECT ok` when
highest_ok_clock >= KHZ and `EXPECT fail` otherwise.
Exit status: 0 = completed run, expectation met if given, 1 = EXPECT fail or
unfinished run, 2 = usage or input error. Python 3, standard library only.
"""
import argparse
import re
import sys

MODE_OK = re.compile(r"mode (\d+x\d+@\d+) clock=(\d+) test=ok$")
MODE_REFUSED = re.compile(r"mode (\d+x\d+@\d+) clock=(\d+) test=refused errno=(-?\d+)$")
SUMMARY = re.compile(r"mode_test: tested=(\d+) ok=(\d+) highest_ok_clock=(\d+)$")
MASTER = "mode_test: DRM master is unavailable"


class InputError(Exception):
    pass


def parse(text):
    """Return (rows, errors, summaries, master) for a mode_test log."""
    rows, errors, summaries, master = [], [], [], False
    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith("mode "):
            m = MODE_OK.match(line) or MODE_REFUSED.match(line)
            if not m:
                raise InputError("bad mode line: " + line)
            groups = m.groups()
            name, clock = groups[0], int(groups[1])
            result = "ok" if len(groups) == 2 else groups[2]
            rows.append((name, clock, result))
        elif line.startswith("mode_test:"):
            m = SUMMARY.match(line)
            if m:
                summaries.append(tuple(int(g) for g in m.groups()))
            elif line.startswith(MASTER):
                master = True
            elif line.startswith("mode_test: tested="):
                raise InputError("bad summary line: " + line)
            else:
                errors.append(line)
    return rows, errors, summaries, master


def main(argv):
    ap = argparse.ArgumentParser(
        prog="mode-test-expect.py",
        description="Judge a mode_test TEST_ONLY modeset log.")
    ap.add_argument("input", nargs="?", default="-", metavar="OUTPUT",
                    help="mode_test log (file or - for stdin)")
    ap.add_argument("--expect-min-clock", type=int, metavar="KHZ",
                    help="EXPECT ok only if highest_ok_clock >= KHZ")
    try:
        a = ap.parse_args(argv)
    except SystemExit as e:
        return 2 if e.code else 0
    if a.expect_min_clock is not None and a.expect_min_clock < 0:
        print("mode-test-expect: --expect-min-clock must be >= 0",
              file=sys.stderr)
        return 2
    try:
        if a.input == "-":
            text = sys.stdin.read()
        else:
            with open(a.input, encoding="utf-8", errors="replace") as fh:
                text = fh.read()
    except OSError as e:
        raise InputError("cannot read input: %s" % e)

    rows, errors, summaries, master = parse(text)
    if len(summaries) > 1:
        raise InputError("%d summary lines: the log mixes runs"
                         % len(summaries))
    summary = summaries[0] if summaries else None
    if summary is not None and (master or errors):
        raise InputError("the log holds an error line and a summary line")

    tested = len(rows)
    ok = sum(1 for _, _, result in rows if result == "ok")
    highest = max((clock for _, clock, result in rows if result == "ok"),
                  default=0)
    refused = tested - ok
    if summary is not None and summary != (tested, ok, highest):
        raise InputError(
            "summary tested=%d ok=%d highest_ok_clock=%d disagrees with the "
            "%d mode lines" % (summary[0], summary[1], summary[2], tested))

    for name, clock, result in rows:
        print("MODE name=%s clock=%d result=%s" % (name, clock, result))
    print("MODE highest_ok_clock=%d" % highest)
    print("MODE refused=%d" % refused)

    if summary is None:
        reason = "drm-master" if master else "no-summary"
        print("MODE incomplete reason=%s" % reason)
        for line in errors:
            print("mode-test-expect: %s" % line, file=sys.stderr)
        return 1
    if a.expect_min_clock is not None:
        met = highest >= a.expect_min_clock
        print("EXPECT %s" % ("ok" if met else "fail"))
        return 0 if met else 1
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except InputError as e:
        print("mode-test-expect: %s" % e, file=sys.stderr)
        sys.exit(2)
