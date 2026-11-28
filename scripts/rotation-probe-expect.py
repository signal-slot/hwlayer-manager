#!/usr/bin/env python3
"""Judge the ROT lines printed by the rotation_probe example, per plane.

  rotation-probe-expect.py [LOG...] [--profile rk3588|hvs5|hvs6]
                           [--cluster ID[,ID...]] [--esmart ID[,ID...]]

Reads saved rotation_probe output (files, or stdin when none is given or for
-). Line form:

  ROT plane=<id> type=<Primary|Overlay|Cursor> value=<name> bits=0x<hex> result=ok
  ROT plane=<id> type=<...> value=<name> bits=0x<hex> result=errno=<n>

Every plane gets a table line of accepted and refused values:

  TABLE plane=42 type=Overlay accepted=rotate-0,reflect-y refused=...

With --profile each plane is compared with the claims read from source
(see rotation-probe-expect-README.md) and gets one line:

  VERDICT plane=42 match
  VERDICT plane=42 mismatch accepted_unexpected=... refused_unexpected=...
  VERDICT plane=42 incomplete missing=...

mismatch wins over incomplete. For rk3588 the window class of a plane is
taken from --cluster / --esmart, else inferred from the table: a plane that
accepted rotate-90 is Cluster, any other plane is Esmart. A plane with no ROT
lines (known from a PLANE line) is incomplete. Unknown lines are ignored.
Final line: VERDICT match|mismatch|incomplete (without --profile the final
line is VERDICT unchecked when every plane is complete).
Exit status: 0 = match (or unchecked and complete), 1 = mismatch or
incomplete, 2 = usage error, unparsable ROT line, or no plane found.
Python 3, standard library only.
"""
import argparse
import re
import sys

VALUES = ("rotate-0", "rotate-90", "rotate-180", "rotate-270", "reflect-x",
          "reflect-y", "rotate-90|reflect-x", "rotate-0|reflect-x|reflect-y")
TYPES = ("Primary", "Overlay", "Cursor")
KV = re.compile(r"(\w+)=(\S+)")
PLANE_LINE = re.compile(r"^PLANE\s+(?:id\s+|plane=)(\d+)\b")

# Expected accepted values, read from source (see the README).
CLUSTER = frozenset(VALUES)
ESMART = frozenset(("rotate-0", "reflect-y"))
VC4 = frozenset(("rotate-0", "rotate-180", "reflect-x", "reflect-y",
                 "rotate-0|reflect-x|reflect-y"))
RANK = {"match": 0, "incomplete": 1, "mismatch": 2}


class InputError(Exception):
    pass


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
    return "\n".join(parts)


def parse(text):
    """Return (planes, types, results, conflicts).

    results[plane] maps value name to None (ok) or an errno int;
    conflicts[plane] is the set of values seen both accepted and refused.
    """
    planes, types, results, conflicts = set(), {}, {}, {}
    for raw in text.splitlines():
        line = raw.strip()
        m = PLANE_LINE.match(line)
        if m:
            planes.add(int(m.group(1)))
            continue
        if not line.startswith("ROT plane="):
            continue
        f = dict(KV.findall(line))
        try:
            plane = int(f["plane"])
            ptype = f["type"]
            value = f["value"]
            int(f["bits"], 16)
            res = f["result"]
        except (KeyError, ValueError):
            raise InputError("bad ROT line: " + line)
        if ptype not in TYPES:
            raise InputError("bad ROT type: " + line)
        # KV splits at the first =, so a refusal reads errno=<n> here.
        if res == "ok":
            outcome = None
        else:
            em = re.fullmatch(r"errno=(\d+)", res)
            if not em or int(em.group(1)) == 0:
                raise InputError("bad ROT result: " + line)
            outcome = int(em.group(1))
        planes.add(plane)
        types.setdefault(plane, ptype)
        seen = results.setdefault(plane, {})
        if value in seen and (seen[value] is None) != (outcome is None):
            conflicts.setdefault(plane, set()).add(value)
        # a refusal overrides an earlier ok; the conflict stays flagged
        if value not in seen or outcome is not None:
            seen[value] = outcome
    return planes, types, results, conflicts


def parse_ids(text, opt):
    try:
        return {int(x) for x in text.split(",") if x}
    except ValueError:
        raise InputError("bad plane list for %s: %s" % (opt, text))


def expected_for(profile, plane, res, cluster, esmart):
    """Return (expected accepted set, class label) for one plane."""
    if profile in ("hvs5", "hvs6"):
        return VC4, profile
    if plane in cluster:
        return CLUSTER, "cluster"
    if plane in esmart:
        return ESMART, "esmart"
    if "rotate-90" in res and res["rotate-90"] is None:
        return CLUSTER, "cluster"
    return ESMART, "esmart"


def main(argv):
    ap = argparse.ArgumentParser(
        prog="rotation-probe-expect.py",
        description="Judge rotation_probe ROT lines per plane.")
    ap.add_argument("input", nargs="*", help="rotation_probe output files")
    ap.add_argument("--profile", choices=("rk3588", "hvs5", "hvs6"))
    ap.add_argument("--cluster", default="", metavar="IDS",
                    help="rk3588: plane ids known to be Cluster windows")
    ap.add_argument("--esmart", default="", metavar="IDS",
                    help="rk3588: plane ids known to be Esmart windows")
    try:
        args = ap.parse_args(argv)
    except SystemExit as e:
        return 2 if e.code else 0
    try:
        cluster = parse_ids(args.cluster, "--cluster")
        esmart = parse_ids(args.esmart, "--esmart")
        planes, types, results, conflicts = parse(read_all(args.input))
    except InputError as e:
        print("rotation-probe-expect: %s" % e, file=sys.stderr)
        return 2
    if not planes:
        print("rotation-probe-expect: no ROT or PLANE lines", file=sys.stderr)
        return 2

    overall = "match"
    for plane in sorted(planes):
        res = results.get(plane, {})
        known = [v for v in VALUES if v in res]
        acc = [v for v in known if res[v] is None]
        ref = ["%s(errno=%d)" % (v, res[v]) for v in known
               if res[v] is not None]
        print("TABLE plane=%d type=%s accepted=%s refused=%s"
              % (plane, types.get(plane, "unknown"), ",".join(acc) or "none",
                 ",".join(ref) or "none"))
        missing = [v for v in VALUES if v not in res]
        if args.profile is None:
            verdict = "incomplete" if missing else "match"
            if missing:
                print("VERDICT plane=%d incomplete missing=%s"
                      % (plane, ",".join(missing)))
            overall = max(overall, verdict, key=RANK.get)
            continue
        want, label = expected_for(args.profile, plane, res, cluster, esmart)
        bad_acc = [v for v in known if res[v] is None and v not in want]
        bad_ref = [v for v in known if res[v] is not None and v in want]
        conf = sorted(conflicts.get(plane, ()))
        extra = ""
        if bad_acc or bad_ref or conf:
            verdict = "mismatch"
            if bad_acc:
                extra += " accepted_unexpected=" + ",".join(bad_acc)
            if bad_ref:
                extra += " refused_unexpected=" + ",".join(bad_ref)
            if conf:
                extra += " conflicting=" + ",".join(conf)
        elif missing:
            verdict = "incomplete"
            extra = " missing=" + ",".join(missing)
        else:
            verdict = "match"
        print("VERDICT plane=%d %s%s class=%s" % (plane, verdict, extra, label))
        overall = max(overall, verdict, key=RANK.get)

    if args.profile is None:
        print("VERDICT unchecked" if overall == "match"
              else "VERDICT incomplete")
    else:
        print("VERDICT %s" % overall)
    return 0 if overall == "match" else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
