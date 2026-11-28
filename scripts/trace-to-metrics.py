#!/usr/bin/env python3
"""Convert a trace log (docs/trace-format.md, CETRACE v=1) into the RUN and
METRIC lines read by m2-compare-report.py. Standard library only.

    python3 -I scripts/trace-to-metrics.py [--stage S] [--refresh-us N] LOG > run.txt

Exit 0 ok, 2 input error (unreadable file, no date comment, no hdr line,
unknown version, malformed CETRACE line). See trace-to-metrics-README.md.
"""
import re
import sys

WRAP = 1 << 32
NS = 10**9


class Bad(Exception):
    pass


def parse_line(text, n):
    toks = text.split()
    if len(toks) < 3 or not toks[1].startswith("v="):
        raise Bad("line %d: malformed CETRACE line" % n)
    if toks[1] != "v=1":
        raise Bad("line %d: unknown version %s" % (n, toks[1][2:]))
    f = {}
    for t in re.findall(r'[a-z][a-z0-9_]*=(?:"[^"\n]*"|\S*)', text):
        k, _, v = t.partition("=")
        f.setdefault(k, v.strip('"'))
    if "ev" not in f:
        raise Bad("line %d: no ev=" % n)
    for k in ("seq", "t", "at", "tev", "dur", "out", "buf"):
        if k in f and not f[k].isdigit():
            raise Bad("line %d: %s=%s is not an unsigned integer" % (n, k, f[k]))
    if f["ev"] != "hdr" and ("seq" not in f or "t" not in f):
        raise Bad("line %d: ev=%s without seq= and t=" % (n, f["ev"]))
    return f


def read(path):
    date = None
    hdr = None
    events = []
    try:
        fh = open(path, encoding="utf-8", errors="replace")
    except OSError as e:
        raise Bad("cannot read %s: %s" % (path, e.strerror))
    with fh:
        for n, line in enumerate(fh, 1):
            line = line.rstrip("\n")
            m = re.match(r"#\s*date\s*[:=]\s*(\d{4}-\d{2}-\d{2})\s*$", line)
            if m:
                if date and date != m.group(1):
                    raise Bad("line %d: two different dates" % n)
                date = m.group(1)
                continue
            if not line.startswith("CETRACE"):
                continue
            f = parse_line(line, n)
            if f["ev"] == "hdr":
                if hdr:
                    raise Bad("line %d: second hdr line" % n)
                hdr = f
            else:
                events.append(f)
    if not date:
        raise Bad("no '# date: YYYY-MM-DD' comment line in the log")
    if not hdr:
        raise Bad("no CETRACE ev=hdr line")
    if hdr.get("src") not in ("linux", "zephyr"):
        raise Bad("hdr src= must be linux or zephyr")
    return date, hdr, events


def clock_ns(hdr, events):
    """Per event: (t_ns, tev_ns or None) relative to t0, or None if the hdr
    does not allow a conversion."""
    try:
        freq = int(hdr["freq_hz"])
        t0 = int(hdr["t0"])
    except (KeyError, ValueError):
        return None
    if freq <= 0:
        return None
    wrap = hdr.get("clock") == "cyc32"
    out = []
    prev_raw = t0
    prev_u = t0
    for e in events:
        t = int(e["t"])
        if wrap:
            tu = prev_u + ((t - prev_raw) % WRAP)
            prev_raw, prev_u = t, tu
        else:
            tu = t
        tev = None
        if "tev" in e:
            v = int(e["tev"])
            if wrap:
                d = ((t - v + WRAP // 2) % WRAP) - WRAP // 2
                v = tu - d
            tev = (v - t0) * NS // freq
        out.append(((tu - t0) * NS // freq, tev))
    return out


def ms(ns):
    sign = "-" if ns < 0 else ""
    ns = abs(ns)
    return "%s%d.%06d" % (sign, ns // 10**6, ns % 10**6)


def median(v):
    v = sorted(v)
    n = len(v)
    return v[n // 2] if n % 2 else (v[n // 2 - 1] + v[n // 2]) // 2


def p95(v):
    v = sorted(v)
    return v[max(0, -(-95 * len(v) // 100) - 1)]  # nearest rank


def derive(hdr, events, refresh_ns=None):
    metrics = []

    def add(id_, name, value, unit=""):
        metrics.append((id_, name, unit, value))

    times = clock_ns(hdr, events)
    commit_at = {}
    ended = set()
    latch_at = {}
    violations = 0
    late = None
    submitted = latched = dropped = timeouts = 0
    first_assign = None
    prev_seq = None
    for i, e in enumerate(events):
        seq = int(e["seq"])
        if prev_seq is not None and seq != prev_seq + 1:
            violations += 1
        prev_seq = seq
        ev = e["ev"]
        key = (e.get("out"), e.get("sub"))
        if ev == "commit" and e.get("result") == "ok":
            submitted += 1
            if key in commit_at:
                violations += 1
            commit_at[key] = i
        elif ev == "validate" and e.get("result") == "ok" and first_assign is None \
                and e.get("assign"):
            first_assign = e["assign"]
        elif ev in ("latched", "dropped"):
            if key not in commit_at or key in ended:
                violations += 1
            ended.add(key)
            if ev == "latched":
                latched += 1
                latch_at[key] = i
            else:
                dropped += 1
        elif ev == "latch_timeout":
            timeouts += 1
    pending = len([k for k in commit_at if k not in ended])

    if times:
        by_out = {}
        for key, i in sorted(latch_at.items(), key=lambda kv: kv[1]):
            t, tev = times[i]
            by_out.setdefault(key[0], []).append(tev if tev is not None else t)
        gaps = [b - a for v in by_out.values() for a, b in zip(v, v[1:])]
        if gaps:
            add("M-1.median", "frame interval median", ms(median(gaps)), "ms")
            add("M-1.max", "frame interval max", ms(max(gaps)), "ms")
            if refresh_ns:
                add("M-1.outliers", "frame interval outliers", str(
                    sum(1 for g in gaps if 2 * abs(g - refresh_ns) > refresh_ns)),
                    "intervals")
                late = sum(1 for g in gaps if 2 * g > 3 * refresh_ns)
        lat = []
        for key, i in latch_at.items():
            if key not in commit_at:
                continue
            t, tev = times[i]
            lat.append((tev if tev is not None else t) - times[commit_at[key]][0])
        if lat:
            add("M-2a.median", "commit-to-latch median", ms(median(lat)), "ms")
            add("M-2a.p95", "commit-to-latch p95", ms(p95(lat)), "ms")
            add("M-2a.max", "commit-to-latch max", ms(max(lat)), "ms")
    if submitted:
        add("M-3.submitted", "submitted frames", str(submitted), "frames")
        add("M-3.latched", "latched frames", str(latched), "frames")
        add("M-3.dropped", "dropped frames", str(dropped), "frames")
        add("M-3.timeouts", "latch timeouts", str(timeouts), "events")
        add("M-3.pending", "pending at end of log", str(pending), "frames")
        if late is not None:
            add("M-3.late", "late latches", str(late), "intervals")
    if first_assign:
        parts = []
        for a in first_assign.split(";"):
            f = a.split(":")
            if len(f) >= 4:
                parts.append("layer %s plane %s zpos %s" % (f[0], f[1], f[3]))
        if parts:
            add("M-5.plane", "plane assignment (first validate ok)", ", ".join(parts))
    if events:
        add("M-6.violations", "event order violations", str(violations), "events")
    return metrics


def main(argv):
    stage = None
    refresh_ns = None
    args = argv[1:]
    while len(args) >= 2 and args[0] in ("--stage", "--refresh-us"):
        if args[0] == "--stage":
            stage = args[1]
        else:
            if not args[1].isdigit() or int(args[1]) == 0:
                sys.stderr.write("trace-to-metrics: --refresh-us needs a "
                                 "positive integer\n")
                return 2
            refresh_ns = int(args[1]) * 1000
        args = args[2:]
    if len(args) != 1:
        sys.stderr.write("usage: trace-to-metrics.py [--stage S] "
                         "[--refresh-us N] LOG\n")
        return 2
    try:
        date, hdr, events = read(args[0])
    except Bad as e:
        sys.stderr.write("trace-to-metrics: %s\n" % e)
        return 2
    run = "RUN date=%s side=%s" % (date, hdr["src"])
    if stage:
        run += " stage=%s" % stage
    print(run + " tool=trace-to-metrics")
    for id_, name, unit, value in derive(hdr, events, refresh_ns):
        print('METRIC id=%s name="%s"%s value=%s'
              % (id_, name, " unit=" + unit if unit else "", value))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
