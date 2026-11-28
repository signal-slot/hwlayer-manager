#!/usr/bin/env python3
"""Judge the SCALE test lines and RESULT summaries printed by scale_probe.

  scale-probe-expect.py [LOG...] [--profile-up F] [--profile-down F]

Reads scale_probe output (files, or stdin when no file is given or for '-').
For every tested (plane, axis) group it prints the largest accepted upscale
factor and the smallest accepted downscale factor:

  SCALE plane=42 axis=h up_max=2.5000 down_min=0.7500

A factor is dst over src, the same 16.16-fixed-point value scale_probe
prints. Axes are h, v and both, the axes scale_probe varies. A group whose
accepted factors are not one interval - a rejected factor between two
accepted factors, or the same factor accepted once and rejected once (two
runs concatenated) - is also flagged:

  SCALE plane=42 axis=h non_monotonic

With --profile-up F (a max_upscale claim) and --profile-down F (a
max_downscale claim: source over destination, so 2 means factor 0.5000),
every judged axis prints an EXPECT line ending in ok, fail or unproven:

  EXPECT plane=42 axis=h up_max=2.5000 profile_up=2.0000 ok

ok: the measurements reach the claim and nothing was refused inside it.
fail: the kernel refused a factor the claim says is allowed.
unproven: the log never tested up to the claim and refused nothing, so the
claim is not established (a claim of 32 proved only up to 16, for example).
RESULT lines are cross-checked against the judged SCALE lines; a mismatch
prints a NOTE line and does not change the verdict. A plane with a RESULT
line but no SCALE line is judged from the RESULT fields alone; its down_min
is then the RESULT min_downscale, which the probe takes over all axes.
Exit status: 0 = PASS, 1 = FAIL, 2 = usage or input error.
Python 3, standard library only.
"""
import argparse
import re
import sys

AXES = ("h", "v", "both")
KV = re.compile(r"(\w+)=(\S+)")
EPS = 1e-9
RESULT_MAX_KEYS = ("max_upscale_h", "max_upscale_v", "max_upscale_both")
WORSE = {"ok": 0, "unproven": 1, "fail": 2}


class InputError(Exception):
    pass


def fmt(v):
    """Format a factor like scale_probe does, or 'none'."""
    return "none" if v is None else "%.4f" % v


def parse_number(text):
    """Parse a scale_probe factor text: none, 8.0000 or 8.0000+."""
    if text == "none":
        return None, False
    plus = text.endswith("+")
    try:
        return float(text[:-1] if plus else text), plus
    except ValueError:
        raise InputError("bad number: " + text)


def dst_key(text):
    """Sort key for a dst=WxH token."""
    w, _, h = text.partition("x")
    try:
        return (int(w), int(h))
    except ValueError:
        raise InputError("bad dst: " + text)


def worse(a, b):
    """The worse of two expect verdicts."""
    return a if WORSE[a] >= WORSE[b] else b


def read_all(paths):
    parts = []
    for p in paths or ["-"]:
        try:
            if p == "-":
                parts.append(sys.stdin.read())
            else:
                with open(p, encoding="utf-8", errors="replace") as fh:
                    parts.append(fh.read())
        except OSError as e:
            raise InputError("cannot read input: %s" % e)
    return "".join(parts)


def parse(text):
    """Return (groups, results).

    groups maps (plane, axis, dst) to {factor: set of outcomes}, one outcome
    per SCALE test line; results maps plane to its RESULT line fields.
    """
    groups, results = {}, {}
    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith("SCALE plane="):
            f = dict(KV.findall(line))
            try:
                key = (int(f["plane"]), f["axis"], f["dst"])
                factor = float(f["factor"])
                outcome = f["test_only"] == "accepted"
            except (KeyError, ValueError):
                raise InputError("bad SCALE line: " + line)
            if key[1] not in AXES:
                raise InputError("bad SCALE axis: " + line)
            groups.setdefault(key, {}).setdefault(factor, set()).add(outcome)
        elif line.startswith("RESULT plane="):
            f = dict(KV.findall(line))
            try:
                plane = int(f["plane"])
            except (KeyError, ValueError):
                raise InputError("bad RESULT line: " + line)
            missing = [k for k in RESULT_MAX_KEYS + ("min_downscale",)
                       if k not in f]
            if missing:
                raise InputError("RESULT line lacks %s: %s"
                                 % (",".join(missing), line))
            if f["min_downscale"].endswith("+"):
                raise InputError("bad RESULT line: " + line)
            for k in RESULT_MAX_KEYS + ("min_downscale",):
                parse_number(f[k])
            results[plane] = f
    return groups, results


def judge(outcomes):
    """Judge one (plane, axis, dst) group of tested factors."""
    accepted = sorted(f for f, o in outcomes.items() if True in o)
    rejected = sorted(f for f, o in outcomes.items() if False in o)
    conflict = any(o == {True, False} for o in outcomes.values())
    lo, hi = (accepted[0], accepted[-1]) if accepted else (None, None)
    # scale_probe assumes the accepted factors form one interval starting at
    # 1 (its module docs say so); a rejected factor strictly inside the
    # accepted range breaks that interval.
    gap = lo is not None and any(lo - EPS < r < hi + EPS for r in rejected)
    return {
        "up": max((f for f in accepted if f >= 1 - EPS), default=None),
        "down": min((f for f in accepted if f < 1 - EPS), default=None),
        "r_up": min((f for f in rejected if f >= 1 - EPS), default=None),
        "r_down": max((f for f in rejected if f < 1 - EPS), default=None),
        "non_monotonic": conflict or gap,
    }


def check_up(claim, accepted_max, refused_min):
    """Judge a max_upscale claim against one axis group."""
    if accepted_max is not None and claim <= accepted_max + EPS:
        return "ok", None
    if refused_min is not None and claim >= refused_min - EPS:
        return "fail", "up_refused_at=%s" % fmt(refused_min)
    return "unproven", "up_unproven"


def check_down(claim, accepted_min, refused_max):
    """Judge a max_downscale claim against one axis group.

    claim is source over destination, so the claimed factor is 1 / claim.
    """
    floor = 1.0 / claim
    if accepted_min is not None and floor >= accepted_min - EPS:
        return "ok", None
    if refused_max is not None and floor <= refused_max + EPS:
        return "fail", "down_refused_at=%s" % fmt(refused_max)
    return "unproven", "down_unproven"


def main(argv):
    ap = argparse.ArgumentParser(
        prog="scale-probe-expect.py",
        description="Judge scale_probe SCALE/RESULT lines against "
                    "ScaleLimits profile claims.")
    ap.add_argument("input", nargs="*",
                    help="scale_probe output files (default stdin)")
    ap.add_argument("--profile-up", type=float, metavar="F",
                    help="claimed max_upscale factor (>= 1)")
    ap.add_argument("--profile-down", type=float, metavar="F",
                    help="claimed max_downscale factor, source over "
                         "destination (>= 1)")
    try:
        args = ap.parse_args(argv)
    except SystemExit as e:
        return 2 if e.code else 0
    for name in ("profile_up", "profile_down"):
        v = getattr(args, name)
        if v is not None and v < 1:
            print("scale-probe-expect: --%s must be at least 1"
                  % name.replace("_", "-"), file=sys.stderr)
            return 2

    try:
        groups, results = parse(read_all(args.input))
    except InputError as e:
        print("scale-probe-expect: %s" % e, file=sys.stderr)
        return 2

    counts = {"ok": 0, "fail": 0, "unproven": 0}

    def emit_expect(fields, parts):
        """Print one EXPECT line and count its verdict."""
        verdict = "ok"
        reasons = []
        for v, r in parts:
            verdict = worse(verdict, v)
            if r:
                reasons.append(r)
        line = "EXPECT " + " ".join(fields) + " " + verdict
        if reasons:
            line += " reason=" + ";".join(reasons)
        print(line)
        counts[verdict] += 1

    planes = sorted(set(k[0] for k in groups) | set(results))
    judged = nonmono = notes = 0

    def judged_from_result(plane, axis, res):
        """Judge one axis from RESULT fields when the log has no test lines."""
        up_text = res["max_upscale_" + axis]
        down_text = res["min_downscale"]
        print("SCALE plane=%d axis=%s up_max=%s down_min=%s"
              % (plane, axis, up_text, down_text))
        parts = []
        fields = ["plane=%d" % plane, "axis=%s" % axis]
        if args.profile_up is not None:
            b, _ = parse_number(up_text)
            parts.append(check_up(args.profile_up, b, None))
            fields += ["up_max=%s" % up_text,
                       "profile_up=%s" % fmt(args.profile_up)]
        if args.profile_down is not None:
            m, _ = parse_number(down_text)
            parts.append(check_down(args.profile_down, m, None))
            fields += ["down_min=%s" % down_text,
                       "profile_down=%s" % fmt(args.profile_down)]
        if parts:
            emit_expect(fields, parts)

    for plane in planes:
        res = results.get(plane)
        dsts = sorted({k[2] for k in groups if k[0] == plane}, key=dst_key)
        multi = len(dsts) > 1
        plane_notes = []
        measured_downs = []
        for axis in AXES:
            keys = sorted((k for k in groups
                           if k[0] == plane and k[1] == axis),
                          key=lambda k: dst_key(k[2]))
            if not keys:
                if res is not None:
                    judged_from_result(plane, axis, res)
                    judged += 1
                continue
            for key in keys:
                j = judge(groups[key])
                tag = (" dst=%s" % key[2]) if multi else ""
                print("SCALE plane=%d axis=%s%s up_max=%s down_min=%s"
                      % (plane, axis, tag, fmt(j["up"]), fmt(j["down"])))
                judged += 1
                if j["non_monotonic"]:
                    print("SCALE plane=%d axis=%s%s non_monotonic"
                          % (plane, axis, tag))
                    nonmono += 1
                if args.profile_up is not None or args.profile_down is not None:
                    fields = ["plane=%d" % plane, "axis=%s" % axis]
                    if multi:
                        fields.append("dst=%s" % key[2])
                    parts = []
                    if args.profile_up is not None:
                        parts.append(check_up(args.profile_up, j["up"], j["r_up"]))
                        fields += ["up_max=%s" % fmt(j["up"]),
                                   "profile_up=%s" % fmt(args.profile_up)]
                    if args.profile_down is not None:
                        parts.append(check_down(args.profile_down, j["down"], j["r_down"]))
                        fields += ["down_min=%s" % fmt(j["down"]),
                                   "profile_down=%s" % fmt(args.profile_down)]
                    emit_expect(fields, parts)
                if j["down"] is not None:
                    measured_downs.append(j["down"])
                # Cross-check one axis against the RESULT field of the same
                # axis. Only sound when the log is a single run (one dst),
                # because RESULT lines carry no dst.
                if res is None or multi:
                    continue
                res_text = res["max_upscale_" + axis]
                rval, _ = parse_number(res_text)
                note = "NOTE plane=%d axis=%s result_max_upscale=%s" % (
                    plane, axis, res_text)
                if rval is None:
                    if j["up"] is not None:
                        plane_notes.append(note + " measured_up_max=%s"
                                           % fmt(j["up"]))
                elif j["up"] is None or abs(j["up"] - rval) > EPS:
                    plane_notes.append(note + " measured_up_max=%s"
                                       % fmt(j["up"]))
                elif j["r_up"] is not None and j["r_up"] <= rval + EPS:
                    plane_notes.append(note + " measured_refused_at=%s"
                                       % fmt(j["r_up"]))
        # Cross-check the plane-level min_downscale: the probe's own value is
        # the smallest accepted downscale factor over all axes.
        if (res is not None and not multi
                and any(k[0] == plane for k in groups)):
            rmin, _ = parse_number(res["min_downscale"])
            mdown = min(measured_downs) if measured_downs else None
            if rmin is None and mdown is not None:
                plane_notes.append("NOTE plane=%d result_min_downscale=none "
                                   "measured_down_min=%s" % (plane, fmt(mdown)))
            elif rmin is not None and (
                    mdown is None or abs(rmin - mdown) > EPS):
                plane_notes.append(
                    "NOTE plane=%d result_min_downscale=%s "
                    "measured_down_min=%s" % (plane, fmt(rmin), fmt(mdown)))
        notes += len(plane_notes)
        for line in plane_notes:
            print(line)

    expect_total = sum(counts.values())
    print("SUMMARY planes=%d judged=%d expect=%d ok=%d fail=%d "
          "unproven=%d non_monotonic=%d notes=%d"
          % (len(planes), judged, expect_total,
             counts["ok"], counts["fail"], counts["unproven"],
             nonmono, notes))

    reasons = []
    if nonmono:
        reasons.append("%d non_monotonic axis group(s)" % nonmono)
    if counts["fail"]:
        reasons.append("%d expect fail" % counts["fail"])
    if counts["unproven"]:
        reasons.append("%d expect unproven" % counts["unproven"])
    if judged == 0:
        reasons.append("no SCALE or RESULT lines")
    if reasons:
        print("VERDICT: FAIL (%s)" % "; ".join(reasons))
        return 1
    print("VERDICT: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
