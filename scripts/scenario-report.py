#!/usr/bin/env python3
"""board-scenarios.sh logs to one board section of a dated results doc.

usage: scenario-report.py --board NAME [--pictures FILE] [--disclaimer] LOG...
See scripts/scenario-report-README.md. Standard library only.
"""
import argparse
import os
import re
import sys

ROWS = ["A1", "A2", "A3", "A4", "A5", "A6"]
ISO = r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:Z|[+-]\d{2}:\d{2})?"
RE_ISO = re.compile(ISO)
# `[TIME] A1 name RESULT`, as printed by board-scenarios.sh (TIME prefixed by
# the caller).
RE_SUMMARY = re.compile(
    r"^(?:\[?(?P<ts>" + ISO + r")\]?\s+)?(?P<row>A[1-6])\s+(?P<name>[\w-]+)\s+"
    r"(?P<res>PASS|FAIL|BLOCKED|NOT-RUN)\s*$")
# A standalone time line applies to the summary lines after it.
RE_TIMELINE = re.compile(
    r"^(?:#\s*)?(?:host[ -]?time|date|time)\s*[:=]?\s*(?P<ts>" + ISO + r")\s*$",
    re.I)
RE_SCENARIO = re.compile(r"^SCENARIO\s+([\w-]+)\s+(PASS|FAIL|BLOCKED|NOT-RUN)\s*$")
RE_REASON = re.compile(r"^(?:BLOCKED|failed):\s*(?P<r>.+)$")

FILE_EXT = {"log", "txt", "json", "png", "md", "sh", "py", "rs", "toml", "c",
            "h", "csv"}
RE_IPV4 = re.compile(r"\b\d{1,3}(?:\.\d{1,3}){3}(?::\d+)?\b")
RE_MAC = re.compile(r"\b[0-9A-Fa-f]{2}(?::[0-9A-Fa-f]{2}){5}\b")
RE_USERHOST = re.compile(r"\b[\w.-]+@[\w.-]+\b")
RE_URL = re.compile(r"\b[a-z][a-z0-9+.-]*://\S+", re.I)
RE_TTY = re.compile(r"/dev/(?:tty|serial|cu\.)\S*")
RE_DOTTED = re.compile(r"\b[A-Za-z0-9][A-Za-z0-9-]*(?:\.[A-Za-z0-9-]+)+\b")
RE_SERIAL = re.compile(r"\b(?:serial|s/n|sn)\s*[:=]?\s*\S+", re.I)
MASK = "<redacted>"


def _dotted(m):
    last = m.group(0).rsplit(".", 1)[1].lower()
    return m.group(0) if last in FILE_EXT or last.isdigit() else MASK


def sanitize(text):
    """Strip anything that looks like an address, host name, serial or tty."""
    text = RE_URL.sub(MASK, text)
    text = RE_USERHOST.sub(MASK, text)
    text = RE_MAC.sub(MASK, text)
    text = RE_IPV4.sub(MASK, text)
    text = RE_TTY.sub(MASK, text)
    text = RE_SERIAL.sub(MASK, text)
    text = RE_DOTTED.sub(_dotted, text)
    return text.replace("|", "/").replace("\n", " ").strip()


def parse_log(path, rows):
    """Fill rows[row] = dict(res, name, ts, reasons) from one log."""
    cur_ts = None
    scen = {}      # name -> reasons seen under its SCENARIO block
    block = None
    found = []
    with open(path, errors="replace") as fh:
        for line in fh:
            line = line.rstrip("\n")
            m = RE_TIMELINE.match(line.strip())
            if m:
                cur_ts = m.group("ts")
                continue
            m = RE_SCENARIO.match(line)
            if m:
                block = m.group(1)
                scen.setdefault(block, [])
                continue
            m = RE_REASON.match(line)
            if m and block:
                scen[block].append(m.group("r"))
                continue
            m = RE_SUMMARY.match(line.strip())
            if m:
                block = None
                found.append((m.group("row"), {
                    "res": m.group("res"), "name": m.group("name"),
                    "ts": m.group("ts") or cur_ts}))
    for row, r in found:
        r["reasons"] = scen.get(r["name"], [])
        rows[row] = r


def parse_pictures(path):
    """pictures.log lines: ROW PHASE TIME FILE VERDICT... -> {row: [text]}."""
    pics = {}
    with open(path, errors="replace") as fh:
        for line in fh:
            f = line.split(None, 4)
            if len(f) < 5 or not re.fullmatch(r"A[1-6]", f[0]):
                continue
            pics.setdefault(f[0], []).append("picture %s %s" % (f[1], f[4].strip()))
    return pics


def render(board, rows, pics, disclaimer):
    out = []
    if disclaimer:
        out += ["These runs are portability evidence and never count toward "
                "M1, M2 or M3.", ""]
    out += ["## Board: " + board, "",
            "| Row | Verdict | Host time | Tag | Detail |",
            "| --- | --- | --- | --- | --- |"]
    no_time = []
    for row in ROWS:
        r = rows.get(row)
        if r is None:
            verdict, ts, detail = "BLOCKED", "", "not in log"
        else:
            verdict = {"NOT-RUN": "SKIPPED"}.get(r["res"], r["res"])
            ts = r["ts"] or ""
            parts = [r["name"]]
            if r["res"] == "NOT-RUN":
                parts.append("not run")
            parts += r["reasons"]
            detail = "; ".join(sanitize(p) for p in parts)
        if row in pics:
            detail += "; " + "; ".join(sanitize(p) for p in pics[row])
        if not ts:
            no_time.append(row)
            detail += "; host time missing from log"
        out.append("| %s | %s | %s | observed | %s |" % (row, verdict, ts, detail))
    return "\n".join(out) + "\n", no_time


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--board", required=True)
    ap.add_argument("--pictures")
    ap.add_argument("--disclaimer", action="store_true",
                    help="print the portability-evidence disclaimer first")
    ap.add_argument("logs", nargs="+", metavar="LOG")
    a = ap.parse_args(argv)
    if not re.fullmatch(r"[A-Za-z0-9_+-]+", a.board):
        print("scenario-report: --board must be a plain label "
              "(letters, digits, _, + and -)", file=sys.stderr)
        return 2
    for p in a.logs + ([a.pictures] if a.pictures else []):
        if not os.path.isfile(p):
            print("scenario-report: not a file: %s" % p, file=sys.stderr)
            return 2
    rows = {}
    for p in a.logs:
        parse_log(p, rows)
    pics = parse_pictures(a.pictures) if a.pictures else {}
    text, no_time = render(a.board, rows, pics, a.disclaimer)
    sys.stdout.write(text)
    if no_time:
        print("scenario-report: no host time for %s (left empty)"
              % ",".join(no_time), file=sys.stderr)
        return 3
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
