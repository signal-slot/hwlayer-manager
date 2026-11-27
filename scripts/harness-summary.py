#!/usr/bin/env python3
"""harness-summary: summarise drm-harness records into the SoW M1 condition-7 numbers.

Reads one or more `drm-harness` record files (the `kind key=value ...` lines
that tools/drm-harness prints, see tools/drm-harness/README.md) and prints,
per scenario and per source (master, kernel-lease, dlm), a markdown table and
a JSON summary of the numbers SoW section 9 M1 acceptance condition 7 asks
for: resolution and refresh, plane assignments per commit, errors, frame loss,
commit-to-latch latency (min/median/p99/max), CPU and memory. A value that no
record carries is printed as `not measured`, never guessed.

  harness-summary.py RECORD_FILE... [--markdown | --json]
  harness-summary.py --self-test

Exit status: 0 when a summary was printed (a FAIL verdict inside the records
is data, not an error for this summarizer), 2 on a usage or input error.
Python 3 standard library only. See scripts/harness-summary-README.md.
"""
import json
import os
import re
import sys

NOT_MEASURED = "not measured"
SCHEMA = 1

# Summary metrics of SoW M1 condition 7, as `<prefix>_<stat>...` fields that
# the harness writes into `move_result` and `rotate_result` (src/stats.rs).
METRICS = ("latency", "interval", "validate", "commit")
RESULT_SUMMARY_KINDS = ("move_result", "rotate_result")
# Kinds that carry one record per scenario run.
SINGLE_RESULT_KINDS = ("move_result", "rotate_result", "combos_result",
                       "revoke_result", "before_revoke", "lease", "lease_check")

VERDICT_RE = re.compile(r"^VERDICT\s+(\S+)\s+(PASS|FAIL)\b\s*(.*)$")


class SummaryError(Exception):
    pass


# ---------------------------------------------------------------- parsing
def raw_tokens(line):
    """Split a record line into tokens, keeping double-quoted values whole.

    Mirrors the quoting of `Record::field` in tools/drm-harness/src/record.rs:
    a value that is empty or contains white space, quotes, backslashes or `=`
    is written quoted with `"` and `\\` escaped (and `\\n`, `\\t` for new
    lines and tabs). Tokens are returned with the quotes still in place."""
    out, cur, i, n = [], [], 0, len(line)
    while i < n:
        c = line[i]
        if c in " \t\r":
            if cur:
                out.append("".join(cur))
                cur = []
            i += 1
        elif c == '"':
            cur.append('"')
            i += 1
            while i < n:
                c = line[i]
                if c == "\\" and i + 1 < n:
                    cur.append(c)
                    cur.append(line[i + 1])
                    i += 2
                elif c == '"':
                    cur.append('"')
                    i += 1
                    break
                else:
                    cur.append(c)
                    i += 1
        else:
            cur.append(c)
            i += 1
    if cur:
        out.append("".join(cur))
    return out


def unquote(value):
    """Undo `record.rs` quoting on one field value."""
    if len(value) >= 2 and value[0] == '"' and value[-1] == '"':
        s, out, i = value[1:-1], [], 0
        while i < len(s):
            if s[i] == "\\" and i + 1 < len(s):
                out.append({"n": "\n", "t": "\t"}.get(s[i + 1], s[i + 1]))
                i += 2
            else:
                out.append(s[i])
                i += 1
        return "".join(out)
    return value


def parse_fields(tokens):
    """Tokens after the kind into a `key -> value` dict (values unquoted)."""
    fields = {}
    for token in tokens[1:]:
        pos = token.find("=")
        if pos < 0:
            continue
        fields[token[:pos]] = unquote(token[pos + 1:])
    return fields


def parse_line(line):
    """One record line into `(kind, fields)`, or None for blank/comment lines.

    `VERDICT <scenario> PASS|FAIL <reason>` becomes kind `verdict`. Lines
    starting with `#` are comments (used by the synthetic test data; the
    harness itself never writes one)."""
    stripped = line.strip()
    if not stripped or stripped.startswith("#"):
        return None
    tokens = raw_tokens(stripped)
    if tokens[0] == "VERDICT":
        match = VERDICT_RE.match(stripped)
        if match:
            return ("verdict", {"scenario": match.group(1),
                                "pass": match.group(2) == "PASS",
                                "reason": match.group(3).strip()})
        return ("verdict", {"scenario": tokens[1] if len(tokens) > 1 else NOT_MEASURED,
                            "pass": False, "reason": stripped})
    return (tokens[0], parse_fields(tokens))


# ---------------------------------------------------------------- numbers
def as_int(value, default=None):
    try:
        return int(value)
    except (TypeError, ValueError):
        return default


def as_float(value):
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def as_id_list(value):
    """`31,32` into [31, 32]; `record.rs list()` writes `-` for an empty list."""
    if value is None or value == "-" or value == "":
        return []
    out = []
    for item in str(value).split(","):
        number = as_int(item)
        if number is None:
            return None
        out.append(number)
    return out


def percentile(sorted_samples, percent):
    """Nearest-rank percentile, same definition as `stats.rs::percentile`."""
    if not sorted_samples:
        return None
    n = len(sorted_samples)
    rank = max(1, (percent * n + 99) // 100)  # ceil(percent * n / 100)
    return sorted_samples[rank - 1]


def summarize_samples(samples):
    """Summary of microsecond samples, same definition as `stats.rs::summarize`."""
    if not samples:
        return None
    ordered = sorted(samples)
    return {
        "count": len(ordered),
        "min": ordered[0],
        "median": percentile(ordered, 50),
        "p99": percentile(ordered, 99),
        "max": ordered[-1],
        "mean": sum(ordered) // len(ordered),
    }


def combine_summaries(summaries):
    """Combine per-run summaries of the same metric (README: aggregation rules).

    min/max are exact; median is the nearest-rank median of the run medians;
    p99 is the largest run p99 (an upper bound for the pooled samples); mean
    is the exact count-weighted mean. Returns None when nothing was measured."""
    usable = [s for s in summaries if s and s.get("count", 0) > 0]
    if not usable:
        return None
    count = sum(s["count"] for s in usable)
    medians = [s["median"] for s in usable if s["median"] is not None]
    return {
        "count": count,
        "min": min(s["min"] for s in usable),
        "median": percentile(sorted(medians), 50),
        "p99": max(s["p99"] for s in usable),
        "max": max(s["max"] for s in usable),
        "mean": sum(s["mean"] * s["count"] for s in usable) // count,
    }


# ---------------------------------------------------------------- runs
def new_run(path, harness_meta):
    harness_meta = harness_meta or {}
    return {
        "file": path,
        "harness_version": harness_meta.get("version", NOT_MEASURED),
        "args": harness_meta.get("args", NOT_MEASURED),
        "scenario": None,
        "source": None,
        "session": None,
        "output_records": [],
        "plane_records": [],
        "results": {},
        "assign_records": [],
        "plan_records": [],
        "commit_records": [],
        "frame_records": [],
        "resource_records": [],
        "event_records": [],
        "combo_records": [],
        "progress_records": [],
        "verdicts": [],
    }


def dispatch(run, kind, fields):
    if kind == "verdict":
        run["verdicts"].append(fields)
    elif kind == "output":
        run["output_records"].append(fields)
    elif kind == "plane":
        run["plane_records"].append(fields)
    elif kind == "assign":
        run["assign_records"].append(fields)
    elif kind == "plan":
        run["plan_records"].append(fields)
    elif kind == "commit":
        run["commit_records"].append(fields)
    elif kind == "frame":
        run["frame_records"].append(fields)
    elif kind == "resources":
        run["resource_records"].append(fields)
    elif kind in ("violation", "step_result", "error"):
        run["event_records"].append((kind, fields))
    elif kind == "combo":
        run["combo_records"].append(fields)
    elif kind == "progress":
        run["progress_records"].append(fields)
    elif kind in SINGLE_RESULT_KINDS:
        run["results"][kind] = fields
    # Unknown kinds are ignored on purpose: a newer harness that writes more
    # records must not break this summarizer; the data stays in the log file.


ARGS_SOURCE_RE = re.compile(r"(?:^|\s)--source(?:=|\s+)(\S+)")


def args_source(harness_meta):
    """The `--source` value of the harness command line, or None."""
    args = (harness_meta or {}).get("args")
    match = ARGS_SOURCE_RE.search(args) if isinstance(args, str) else None
    return match.group(1) if match else None


def parse_file(path):
    """A record file into a list of runs (one per scenario).

    The harness writes a scenario's `lease` record (while opening the source)
    BEFORE its `session` record, and a scenario that fails before its session
    writes only `error` and `VERDICT`. So `lease` records are held back and
    attached to the next run, and an `error`/`VERDICT` for another scenario, or
    after the current run's VERDICT, starts a new run."""
    runs, current, harness_meta, pending_lease = [], None, None, None
    try:
        with open(path, encoding="utf-8", errors="replace") as handle:
            lines = handle.read().splitlines()
    except OSError as exc:
        raise SummaryError("cannot read %s: %s" % (path, exc))

    def start(scenario, session=None):
        run = new_run(path, harness_meta)
        run["scenario"] = scenario
        if session is not None:
            run["source"] = session.get("source")
            run["session"] = session
        else:
            run["source"] = args_source(harness_meta)
        nonlocal pending_lease
        if pending_lease is not None:
            run["results"]["lease"] = pending_lease
            pending_lease = None
        runs.append(run)
        return run

    for line in lines:
        parsed = parse_line(line)
        if parsed is None:
            continue
        kind, fields = parsed
        if kind == "harness":
            harness_meta = fields
        elif kind == "lease":
            pending_lease = fields
        elif kind == "session":
            current = start(fields.get("scenario"), fields)
        elif kind in ("error", "verdict") and fields.get("scenario"):
            # A scenario that never opened a session (a refusal before any
            # device work) is an implicit run of its own.
            if (current is None or current["scenario"] != fields["scenario"]
                    or current["verdicts"]):
                current = start(fields["scenario"])
            dispatch(current, kind, fields)
        elif current is not None:
            dispatch(current, kind, fields)
    return runs


# ---------------------------------------------------------------- run facts
def run_verdict(run):
    if not run["verdicts"]:
        return None
    passed = sum(1 for v in run["verdicts"] if v["pass"])
    reasons = [v for v in run["verdicts"] if not v["pass"]]
    return {
        "result": "PASS" if passed == len(run["verdicts"]) else "FAIL",
        "reasons": [v["reason"] for v in reasons],
    }


def run_errors(run):
    errors = []
    for kind, fields in run["event_records"]:
        if kind == "violation":
            errors.append("violation: %s" % fields.get("what", ""))
        elif kind == "step_result":
            errors.append("step refused: %s: %s" % (fields.get("label", "?"),
                                                    fields.get("error", "")))
        elif kind == "error":
            errors.append("error: %s" % fields.get("message", ""))
    verdict = run_verdict(run)
    if verdict:
        for reason in verdict["reasons"]:
            errors.append("FAIL %s: %s" % (run["scenario"], reason))
    combos = run["results"].get("combos_result")
    if combos and (as_int(combos.get("errors"), 0) or 0) > 0:
        errors.append("combos: %s TEST_ONLY error(s)" % combos.get("errors"))
    return errors


def official_summary(run, metric):
    """The harness's own summary fields (`<metric>_count`, `<metric>_min_us`, ...)
    from `move_result` / `rotate_result`, or None."""
    found = []
    for kind in RESULT_SUMMARY_KINDS:
        record = run["results"].get(kind)
        if not record:
            continue
        count = as_int(record.get(metric + "_count"))
        if count is None:
            continue
        stats = {name: as_int(record.get(metric + ("_count" if name == "count"
                                                    else "_%s_us" % name)))
                 for name in ("count", "min", "median", "p99", "max", "mean")}
        stats["count"] = count
        if count > 0 and any(stats[name] is None for name in ("min", "median", "p99", "max")):
            raise SummaryError("%s: %s %s_count is %d but the summary fields are incomplete"
                               % (run["file"], kind, metric, count))
        found.append(stats)
    return combine_summaries(found)


RAW_FIELDS = {"latency": ("latency_us", ("commit_records", "frame_records")),
              "validate": ("validate_us", ("commit_records", "frame_records")),
              "commit": ("commit_us", ("commit_records",))}


def raw_samples(run, metric):
    spec = RAW_FIELDS.get(metric)
    if not spec:
        return None  # `interval` is only measured by the harness itself
    field, lists = spec
    samples = []
    for name in lists:
        for record in run[name]:
            value = as_int(record.get(field))
            if value is not None:
                samples.append(value)
    return summarize_samples(samples)


def run_metric(run, metric):
    """Official harness summary when it has samples, else computed from the
    per-commit / per-frame records of this run (never mixing the two)."""
    official = official_summary(run, metric)
    if official:
        return official
    return raw_samples(run, metric)


FRAME_FIELDS = ("planned", "presented", "dropped", "missed_vblanks", "latch_timeouts")


def run_frames(run):
    """Frame accounting of one run, from the *_result records."""
    frames = {}
    rotate = run["results"].get("rotate_result")
    if rotate:
        frames["planned"] = as_int(rotate.get("frames"))
        for name, key in (("presented", "presented"), ("dropped", "dropped"),
                          ("missed_vblanks", "missed_vblanks"),
                          ("latch_timeouts", "latch_timeouts")):
            frames[name] = as_int(rotate.get(key))
    for kind in ("move_result", "before_revoke"):
        record = run["results"].get(kind)
        if record and "planned" not in frames:
            frames["planned"] = as_int(record.get("frames"))
    return {name: frames.get(name) for name in FRAME_FIELDS}


def run_resources(run):
    return run["resource_records"][-1] if run["resource_records"] else None


def run_output(run):
    """The selected output record (the engine's output the harness ran on)."""
    outputs = run["output_records"]
    if not outputs:
        return None
    for record in outputs:
        if record.get("selected") == "yes":
            return record
    return outputs[0]


def output_resolution(output):
    if not output:
        return None
    width, height = as_int(output.get("width")), as_int(output.get("height"))
    if width is None or height is None:
        return None
    return "%dx%d" % (width, height)


def output_refresh_hz(output):
    if not output:
        return None
    mhz = as_int(output.get("refresh_mhz"))
    if mhz:
        return mhz / 1000.0
    return as_int(output.get("vrefresh"))


def run_lease(run):
    return run["results"].get("lease")


# ---------------------------------------------------------------- groups
def group_of(runs):
    """Runs with the same (scenario, source) merged into one summary."""
    scenario = runs[0]["scenario"]
    source = runs[0]["source"]
    group = {
        "scenario": scenario or NOT_MEASURED,
        "source": source or NOT_MEASURED,
        "runs": len(runs),
        "harness_versions": uniq([r["harness_version"] for r in runs]),
        "profiles": uniq([run_field(r, "profile") for r in runs]),
        "producers": uniq([run_field(r, "producers") for r in runs]),
        "masters": uniq([run_field(r, "master") for r in runs]),
        "devices": uniq([run_field(r, "device") for r in runs]),
        "drivers": uniq([run_field(r, "driver") for r in runs]),
    }
    verdicts = [run_verdict(r) for r in runs]
    known = [v for v in verdicts if v]
    if known:
        group["verdict"] = "PASS" if all(v["result"] == "PASS" for v in known) else "FAIL"
        group["runs_passed"] = sum(1 for v in known if v["result"] == "PASS")
        group["runs_failed"] = sum(1 for v in known if v["result"] == "FAIL")
        if len(known) < len(runs):
            group["runs_without_verdict"] = len(runs) - len(known)
    else:
        group["verdict"] = NOT_MEASURED
    outputs = [run_output(r) for r in runs]
    group["connectors"] = uniq(o.get("connector") if o else None for o in outputs)
    group["modes"] = uniq(o.get("mode") if o else None for o in outputs)
    group["resolutions"] = uniq(output_resolution(o) for o in outputs)
    group["refresh_hz"] = uniq(output_refresh_hz(o) for o in outputs)
    leases = [run_lease(r) for r in runs]
    group["lease_routes"] = uniq(l.get("route") if l else None for l in leases)
    group["lease_lessees"] = [as_int(l.get("lessee")) for l in leases if l and l.get("lessee")]
    group["lease_sockets"] = uniq(l.get("socket") if l else None for l in leases)
    group["metrics"] = {m: combine_summaries([run_metric(r, m) for r in runs]) or NOT_MEASURED
                        for m in METRICS}
    frame_sets = [run_frames(r) for r in runs]
    frames = {}
    for name in FRAME_FIELDS:
        values = [f[name] for f in frame_sets if f[name] is not None]
        frames[name] = sum(values) if values else NOT_MEASURED
    if frames["planned"] != NOT_MEASURED and frames["presented"] != NOT_MEASURED:
        frames["frame_loss"] = frames["planned"] - frames["presented"]
    else:
        frames["frame_loss"] = NOT_MEASURED
    group["frames"] = frames
    resource_sets = [run_resources(r) for r in runs]
    present = [r for r in resource_sets if r]
    if present:
        group["resources"] = {
            "wall_ms": sum(as_int(r.get("wall_ms")) or 0 for r in present),
            "cpu_user_ms": sum(as_int(r.get("cpu_user_ms")) or 0 for r in present),
            "cpu_sys_ms": sum(as_int(r.get("cpu_sys_ms")) or 0 for r in present),
            "cpu_pct_max": max([as_float(r.get("cpu_pct")) for r in present
                                if as_float(r.get("cpu_pct")) is not None] or [NOT_MEASURED]),
            "rss_kib_max": max([as_int(r.get("rss_kib")) for r in present
                                if as_int(r.get("rss_kib")) is not None] or [NOT_MEASURED]),
            "peak_rss_kib_max": max([as_int(r.get("peak_rss_kib")) for r in present
                                     if as_int(r.get("peak_rss_kib")) is not None] or [NOT_MEASURED]),
        }
    else:
        group["resources"] = NOT_MEASURED
    group["errors"] = ["%s: %s" % (os.path.basename(r["file"]), e)
                       for r in runs for e in run_errors(r)]
    group["assignments"] = merge_assignments(runs)
    group["assignment_changes"] = sum(as_int(r["results"].get("rotate_result", {})
                                             .get("assignment_changes"), 0) or 0 for r in runs)
    combo_sets = [r["results"].get("combos_result") for r in runs]
    combo_sets = [c for c in combo_sets if c]
    if combo_sets:
        group["combos"] = {name: sum(as_int(c.get(name), 0) or 0 for c in combo_sets)
                           for name in ("combos", "accepted", "planner_refused",
                                        "test_only_refused", "errors")}
    else:
        group["combos"] = NOT_MEASURED
    group["run_details"] = [run_detail(r) for r in runs]
    return group


def run_field(run, key):
    return run["session"].get(key) if run["session"] else None


def uniq(values):
    out = []
    for value in values:
        if value is not None and value not in out:
            out.append(value)
    return out


FRAMES_STEP = "frames (per frame)"


def merge_assignments(runs):
    """Plane assignments per step, merged across the runs of a group.

    From `assign` records (two-planes, zorder-alpha, rotate's first frame) and
    from `frame` records (move: bottom_plane / top_plane per frame)."""
    steps = {}
    kinds = {}
    for run in runs:
        for record in run["plane_records"]:
            plane = as_int(record.get("id"))
            if plane is not None and record.get("kind"):
                kinds[plane] = record["kind"]
        for record in run["assign_records"]:
            step = record.get("step", "?")
            layer, plane = as_int(record.get("layer")), as_int(record.get("plane"))
            if plane is not None and record.get("kind"):
                kinds[plane] = record["kind"]
            entry = steps.setdefault(step, {"layers": {}, "assign_records": 0, "commits": 0})
            entry["assign_records"] += 1
            if layer is not None and plane is not None:
                entry["layers"].setdefault(layer, set()).add(plane)
        for record in run["commit_records"]:
            step = record.get("step", "?")
            steps.setdefault(step, {"layers": {}, "assign_records": 0, "commits": 0})
            steps[step]["commits"] += 1
        if run["frame_records"]:
            entry = steps.setdefault(FRAMES_STEP,
                                     {"layers": {}, "assign_records": 0, "commits": 0})
            for record in run["frame_records"]:
                entry["commits"] += 1
                for layer, key in ((0, "bottom_plane"), (1, "top_plane")):
                    plane = as_int(record.get(key))
                    if plane is not None:
                        entry["assign_records"] += 1
                        entry["layers"].setdefault(layer, set()).add(plane)
    merged = []
    for step in sorted(steps):
        entry = steps[step]
        layers = {}
        for layer in sorted(entry["layers"]):
            planes = sorted(entry["layers"][layer])
            layers[str(layer)] = {
                "planes": planes,
                "kinds": [kinds.get(plane, "?") for plane in planes],
            }
        merged.append({"step": step, "assign_records": entry["assign_records"],
                       "commits": entry["commits"], "layers": layers})
    return merged


def run_detail(run):
    rotate = run["results"].get("rotate_result", {})
    detail = {
        "file": run["file"],
        "harness_version": run["harness_version"],
        "args": run["args"],
        "scenario": run["scenario"] or NOT_MEASURED,
        "source": run["source"] or NOT_MEASURED,
        "device": run_field(run, "device") or NOT_MEASURED,
        "driver": run_field(run, "driver") or NOT_MEASURED,
        "master": run_field(run, "master") or NOT_MEASURED,
        "profile": run_field(run, "profile") or NOT_MEASURED,
        "producers": run_field(run, "producers") or NOT_MEASURED,
        "verdict": run_verdict(run) or NOT_MEASURED,
        "output": run_output(run) or NOT_MEASURED,
        "planes_visible": len(run["plane_records"]),
        "lease": run_lease(run) or NOT_MEASURED,
        "lease_check": run["results"].get("lease_check", NOT_MEASURED),
        "metrics": {m: run_metric(run, m) or NOT_MEASURED for m in METRICS},
        "frames": {k: v if v is not None else NOT_MEASURED
                   for k, v in run_frames(run).items()},
        "resources": run_resources(run) or NOT_MEASURED,
        "errors": run_errors(run),
        "commits": run["commit_records"],
        "frame_records": run["frame_records"],
        "results": run["results"],
        "combos": run["combo_records"],
        "progress_last": run["progress_records"][-1] if run["progress_records"] else NOT_MEASURED,
    }
    if rotate:
        detail["assignment_changes"] = as_int(rotate.get("assignment_changes"), 0)
    return detail


def summarize_runs(runs):
    groups = {}
    for run in runs:
        key = (run["scenario"] or NOT_MEASURED, run["source"] or NOT_MEASURED)
        groups.setdefault(key, []).append(run)
    ordered = [group_of(runs_list) for _, runs_list in sorted(groups.items())]
    return {
        "tool": "harness-summary.py",
        "schema": SCHEMA,
        "inputs": list(dict.fromkeys(run["file"] for run in runs)),
        "groups": ordered,
    }


# ---------------------------------------------------------------- markdown
def fmt_summary(summary):
    if summary is None or summary == NOT_MEASURED:
        return NOT_MEASURED
    return "%s/%s/%s/%s (n=%s, mean %s)" % (summary["min"], summary["median"],
                                            summary["p99"], summary["max"],
                                            summary["count"], summary["mean"])


def fmt_metric(summary):
    if summary is None or summary == NOT_MEASURED:
        return NOT_MEASURED
    return "%s (n=%s)" % (summary["median"], summary["count"])


def fmt_list(values, sep="/"):
    if not values:
        return NOT_MEASURED
    return sep.join(str(v) for v in values)


def fmt_hz(values):
    if not values:
        return NOT_MEASURED
    return "/".join("%.3f" % v for v in values)


def fmt_resources(resources):
    if resources == NOT_MEASURED:
        return NOT_MEASURED
    return ("wall %s ms, cpu %s+%s ms (max %s%%), RSS %s KiB, peak %s KiB"
            % (resources["wall_ms"], resources["cpu_user_ms"], resources["cpu_sys_ms"],
               resources["cpu_pct_max"], resources["rss_kib_max"],
               resources["peak_rss_kib_max"]))


def fmt_assignment(layers):
    if not layers:
        return NOT_MEASURED
    parts = []
    for layer, info in layers.items():
        planes = ", ".join("%s(%s)" % (p, k) for p, k in zip(info["planes"], info["kinds"]))
        parts.append("layer %s -> %s" % (layer, planes))
    return "; ".join(parts)


def markdown_table(headers, rows):
    lines = ["| " + " | ".join(headers) + " |",
             "|" + "|".join("---" for _ in headers) + "|"]
    for row in rows:
        lines.append("| " + " | ".join(str(cell) for cell in row) + " |")
    return "\n".join(lines)


def render_markdown(summary):
    groups = summary["groups"]
    out = ["# harness-summary", "",
           "Records: " + ", ".join(summary["inputs"]), ""]

    rows = []
    for g in groups:
        rows.append([g["scenario"], g["source"], g["runs"], g["verdict"],
                     fmt_list(g["harness_versions"]), fmt_list(g["producers"]),
                     fmt_list(g["profiles"]), fmt_list(g["masters"]),
                     fmt_list(g["devices"]), fmt_list(g["drivers"]),
                     fmt_list(g["connectors"]), fmt_list(g["modes"]),
                     fmt_list(g["resolutions"]), fmt_hz(g["refresh_hz"]),
                     fmt_list(g["lease_routes"]), fmt_list(g["lease_lessees"])])
    out.append("## Run context")
    out.append("")
    out.append(markdown_table(["scenario", "source", "runs", "verdict", "harness",
                               "producers", "profile", "master", "device", "driver",
                               "connector", "mode", "resolution", "refresh Hz",
                               "lease route", "lessee"], rows))
    out.append("")

    rows = []
    for g in groups:
        frames = g["frames"]
        rows.append([g["scenario"], g["source"], g["runs"],
                     frames["planned"], frames["presented"], frames["dropped"],
                     frames["missed_vblanks"], frames["latch_timeouts"],
                     frames["frame_loss"],
                     fmt_summary(g["metrics"]["latency"]),
                     fmt_metric(g["metrics"]["validate"]),
                     fmt_metric(g["metrics"]["commit"]),
                     fmt_metric(g["metrics"]["interval"]),
                     fmt_resources(g["resources"]), len(g["errors"])])
    out.append("## Measurements (SoW M1 condition 7)")
    out.append("")
    out.append(markdown_table(["scenario", "source", "runs", "frames", "presented",
                               "dropped", "missed vblanks", "latch timeouts",
                               "frame loss", "commit->latch us min/median/p99/max",
                               "validate us", "commit us", "frame interval us",
                               "resources", "errors"], rows))
    out.append("")

    rows = []
    for g in groups:
        for assignment in g["assignments"]:
            rows.append([g["scenario"], g["source"], assignment["step"],
                         assignment["assign_records"],
                         assignment["commits"] if assignment["commits"] else NOT_MEASURED,
                         fmt_assignment(assignment["layers"])])
    out.append("## Plane assignments")
    out.append("")
    out.append(markdown_table(["scenario", "source", "step", "layer->plane records",
                               "commits", "layers -> plane (kind)"], rows))
    if any(g["assignment_changes"] for g in groups):
        out.append("")
        out.append("Assignment changes reported by `rotate_result` (assignments are "
                   "printed for the first frame only): " +
                   "; ".join("%s/%s: %s changes" % (g["scenario"], g["source"],
                                                    g["assignment_changes"])
                             for g in groups if g["assignment_changes"]))
    out.append("")

    out.append("## Errors")
    out.append("")
    any_error = False
    for g in groups:
        if g["errors"]:
            any_error = True
            out.append("- %s / %s:" % (g["scenario"], g["source"]))
            for error in g["errors"]:
                out.append("  - " + error)
    if not any_error:
        out.append("(none)")
    out.append("")
    out.append("Not every SoW M1 condition-7 field is in the harness records: board")
    out.append("revision, BSP/kernel version and the lease stack version belong in the")
    out.append("M1 report text (docs/m1-imx95-report.md), not in this summary.")
    return "\n".join(out)


def render_json(summary):
    return json.dumps(summary, indent=1)


# ---------------------------------------------------------------- self-test
def expected_groups():
    """Hand-computed expected numbers for the synthetic test data.

    The synthetic files and these expectations were written together; the
    numbers were computed by hand with the stats.rs percentile rules, so the
    self-test checks the script against values it did not compute itself."""
    groups = {}

    def group(scenario, source, **kw):
        groups[(scenario, source)] = kw

    def lat(count, mn, med, p99, mx, mean):
        return {"count": count, "min": mn, "median": med, "p99": p99, "max": mx, "mean": mean}

    def frames_missing(**kw):
        frames = {name: NOT_MEASURED for name in FRAME_FIELDS}
        frames["frame_loss"] = NOT_MEASURED
        frames.update(kw)
        return frames

    group("combos", "master",
          verdict="PASS", runs=1,
          metrics={m: NOT_MEASURED for m in METRICS},
          frames=frames_missing(),
          resources=lat_wall(600, 60, 10, 11.7, 3968, 4096),
          errors=1, combos={"combos": 6, "accepted": 2, "planner_refused": 2,
                            "test_only_refused": 1, "errors": 1})

    group("move", "master",
          verdict="PASS", runs=1,
          metrics={"latency": lat(2, 16900, 16900, 17100, 17100, 17000),
                   "interval": lat(1, 16800, 16800, 16800, 16800, 16800),
                   "validate": lat(3, 850, 900, 950, 950, 900),
                   "commit": NOT_MEASURED},
          frames=frames_missing(planned=3),
          resources=lat_wall(1500, 210, 40, 16.7, 4096, 5120),
          errors=0,
          assignments={"frames (per frame)": {0: [86], 1: [143]}})

    group("revoke", "master",
          verdict="FAIL", runs=1,
          metrics={m: NOT_MEASURED for m in METRICS},
          frames=frames_missing(),
          resources=NOT_MEASURED, errors=2, lease_routes=[])

    group("rotate", "kernel-lease",
          verdict="FAIL", runs=2,
          metrics={"latency": lat(10, 15000, 16000, 17000, 17000, 16080),
                   "interval": lat(9, 16100, 16300, 16900, 16900, 16588),
                   "validate": lat(10, 500, 650, 1200, 1200, 735),
                   "commit": lat(10, 300, 380, 500, 500, 390)},
          frames=frames_missing(planned=12, presented=11, dropped=1, missed_vblanks=3,
                                latch_timeouts=0, frame_loss=1),
          resources=lat_wall(4500, 560, 200, 17.1, 4480, 4736),
          errors=2, lease_routes=["kernel"])

    group("two-planes", "master",
          verdict="PASS", runs=1,
          metrics={"latency": lat(1, 25000, 25000, 25000, 25000, 25000),
                   "interval": NOT_MEASURED,
                   "validate": lat(1, 900, 900, 900, 900, 900),
                   "commit": lat(1, 350, 350, 350, 350, 350)},
          frames=frames_missing(),
          resources=lat_wall(900, 90, 20, 12.2, 3840, 4224),
          errors=0,
          assignments={"two-planes": {0: [86], 1: [143]}})

    group("all-two-planes", "kernel-lease",
          verdict="PASS", runs=1,
          metrics={"latency": lat(1, 16500, 16500, 16500, 16500, 16500),
                   "interval": NOT_MEASURED,
                   "validate": lat(1, 800, 800, 800, 800, 800),
                   "commit": lat(1, 300, 300, 300, 300, 300)},
          frames=frames_missing(),
          resources=NOT_MEASURED, errors=0, lease_routes=["kernel"],
          lessees=[11])

    group("all-move", "kernel-lease",
          verdict="FAIL", runs=1,
          metrics={m: NOT_MEASURED for m in METRICS},
          frames=frames_missing(),
          resources=NOT_MEASURED, errors=2, lease_routes=[],
          lessees=[])

    group("all-combos", "kernel-lease",
          verdict="PASS", runs=1,
          metrics={"latency": lat(1, 17000, 17000, 17000, 17000, 17000),
                   "interval": NOT_MEASURED,
                   "validate": lat(1, 900, 900, 900, 900, 900),
                   "commit": lat(1, 400, 400, 400, 400, 400)},
          frames=frames_missing(),
          resources=NOT_MEASURED, errors=0, lease_routes=["kernel"],
          lessees=[13])

    group("zorder-alpha", "dlm",
          verdict="PASS", runs=1,
          metrics={"latency": lat(4, 16100, 16300, 16700, 16700, 16400),
                   "interval": NOT_MEASURED,
                   "validate": lat(4, 700, 750, 850, 850, 775),
                   "commit": lat(4, 300, 320, 360, 360, 330)},
          frames=frames_missing(),
          resources=NOT_MEASURED, errors=0, lease_routes=["drm-lease-manager"],
          steps=4)
    return groups


def lat_wall(wall, user, sysms, pct, rss, peak):
    return {"wall_ms": wall, "cpu_user_ms": user, "cpu_sys_ms": sysms,
            "cpu_pct_max": pct, "rss_kib_max": rss, "peak_rss_kib_max": peak}


def self_test():
    ok = [0]

    def check(cond, what):
        if not cond:
            print("FAIL: " + what)
            raise SystemExit(1)
        ok[0] += 1

    # Tokenizer and quoting, mirroring the record.rs unit tests.
    kind, fields = parse_line('error message="Permission denied (os error 13)"')
    check(kind == "error" and fields["message"] == "Permission denied (os error 13)",
          "quoted value with spaces")
    _, fields = parse_line('x a=1 b="" c="a=b" d="say \\"hi\\"" e="two\\nlines"')
    check(fields == {"a": "1", "b": "", "c": "a=b", "d": 'say "hi"', "e": "two\nlines"},
          "quoting of empty, =, quotes, newline")
    _, fields = parse_line('r n=3 plane=31')
    check(fields == {"n": "3", "plane": "31"}, "plain fields")
    check(as_id_list("31,32") == [31, 32] and as_id_list("-") == [],
          "list fields, `-` is an empty list")
    kind, fields = parse_line("VERDICT rotate FAIL 5 of 6 frames presented; 1 order violations")
    check(kind == "verdict" and fields["pass"] is False and fields["scenario"] == "rotate"
          and fields["reason"] == "5 of 6 frames presented; 1 order violations",
          "VERDICT line with a free-text reason")
    check(parse_line("# synthetic comment") is None, "comment lines are ignored")

    # Percentile and summary definitions, from the stats.rs unit tests.
    hundred = list(range(1, 101))
    check(percentile(hundred, 50) == 50 and percentile(hundred, 99) == 99,
          "percentiles of 1..100")
    check(percentile(list(range(1, 11)), 50) == 5 and percentile(list(range(1, 11)), 99) == 10,
          "percentiles of 1..10 (median lower middle)")
    check(percentile([7], 99) == 7 and percentile([], 50) is None, "single / empty samples")
    check(summarize_samples([30, 10, 20, 1000]) ==
          {"count": 4, "min": 10, "median": 20, "p99": 1000, "max": 1000, "mean": 265},
          "summarize matches stats.rs (mean rounded down)")

    # Summary over the synthetic record files.
    here = os.path.dirname(os.path.abspath(__file__))
    data_dir = os.path.join(here, "testdata", "harness-summary")
    files = sorted(os.path.join(data_dir, name) for name in os.listdir(data_dir)
                   if name.endswith(".log"))
    check(len(files) == 8, "eight synthetic record files in testdata")
    for path in files:
        with open(path, encoding="utf-8") as handle:
            first = handle.readline()
        check(first.startswith("# SYNTHETIC"), "%s is marked SYNTHETIC" % os.path.basename(path))
    runs = []
    for path in files:
        runs.extend(parse_file(path))
    check(len(runs) == 10, "ten runs parsed")
    summary = summarize_runs(runs)
    expected = expected_groups()
    check(len(summary["groups"]) == len(expected), "%d groups" % len(expected))
    for group in summary["groups"]:
        key = (group["scenario"], group["source"])
        want = expected.get(key)
        check(want is not None, "group %s/%s is expected" % key)
        check(group["runs"] == want["runs"], "%s/%s runs" % key)
        check(group["verdict"] == want["verdict"], "%s/%s verdict %s"
              % (key[0], key[1], group["verdict"]))
        check(group["metrics"] == want["metrics"], "%s/%s metrics %r"
              % (key[0], key[1], group["metrics"]))
        frames = dict(group["frames"])
        check(frames == want["frames"], "%s/%s frames %r" % (key[0], key[1], frames))
        check(group["resources"] == want["resources"], "%s/%s resources %r"
              % (key[0], key[1], group["resources"]))
        check(len(group["errors"]) == want["errors"], "%s/%s errors %r"
              % (key[0], key[1], group["errors"]))
        check(group["lease_routes"] == want.get("lease_routes", group["lease_routes"]),
              "%s/%s lease routes" % key)
        if "lessees" in want:
            check(group["lease_lessees"] == want["lessees"], "%s/%s lessee per run %r"
                  % (key[0], key[1], group["lease_lessees"]))
        if "combos" in want:
            check(group["combos"] == want["combos"], "%s/%s combos" % key)
        for step, layers in want.get("assignments", {}).items():
            found = {a["step"]: a for a in group["assignments"]}.get(step)
            check(found is not None, "%s/%s has assignment step %s" % (key[0], key[1], step))
            check({int(l): sorted(i["planes"]) for l, i in found["layers"].items()} == layers,
                  "%s/%s assignment %s planes" % (key[0], key[1], step))
        if "steps" in want:
            check(len(group["assignments"]) == want["steps"],
                  "%s/%s has %d steps" % (key[0], key[1], want["steps"]))

    # A scenario that fails before its session keeps its own error/verdict
    # and does not turn the scenario before it into a FAIL.
    by_key = {(g["scenario"], g["source"]): g for g in summary["groups"]}
    failed = by_key[("all-move", "kernel-lease")]
    check(any("Permission denied" in e for e in failed["errors"]),
          "all-move error lands in all-move")
    check(by_key[("all-two-planes", "kernel-lease")]["verdict"] == "PASS"
          and not by_key[("all-two-planes", "kernel-lease")]["errors"],
          "all-two-planes untouched by the all-move failure")
    for group in summary["groups"]:
        if (group["scenario"], group["source"]) == ("rotate", "kernel-lease"):
            check(group["runs_passed"] == 1 and group["runs_failed"] == 1,
                  "rotate kernel-lease: one PASS run and one FAIL run")
            check(group["assignment_changes"] == 1, "rotate assignment change reported")

    # Markdown: every condition-7 column is printed, `not measured` where absent.
    markdown = render_markdown(summary)
    check("commit->latch us min/median/p99/max" in markdown, "latency column")
    sample = fmt_resources({"wall_ms": 100, "cpu_user_ms": 10, "cpu_sys_ms": 5,
                            "cpu_pct_max": 12.5, "rss_kib_max": 400, "peak_rss_kib_max": 480})
    check("frame loss" in markdown and "RSS 400 KiB" in sample and "peak 480 KiB" in sample,
          "resources columns")
    check("15000/16000/17000/17000 (n=10, mean 16080)" in markdown,
          "combined rotate latency printed")
    check(markdown.count(NOT_MEASURED) >= 30, "absent values printed as `not measured`")
    zorder = [g for g in summary["groups"] if g["scenario"] == "zorder-alpha"][0]
    check("zorder-alpha | dlm" in markdown
          and zorder["resources"] == NOT_MEASURED,
          "zorder-alpha row shows `not measured` CPU/memory")
    for step in ("a-over-b", "b-over-a", "plane-alpha-50", "pixel-alpha-premultiplied"):
        check(step in markdown, "zorder step %s in the assignment table" % step)

    # JSON: parses back, and carries the same numbers.
    as_json = json.loads(render_json(summary))
    check(as_json["schema"] == SCHEMA and len(as_json["groups"]) == 9, "JSON round trip")
    for group in as_json["groups"]:
        key = (group["scenario"], group["source"])
        if key == ("zorder-alpha", "dlm"):
            check(group["resources"] == NOT_MEASURED, "JSON keeps `not measured`")
            check(group["metrics"]["latency"]["median"] == 16300, "JSON latency median")
    print("%d checks" % ok[0])
    print("PASS")
    return 0


# ---------------------------------------------------------------- main
def main(argv):
    args = argv[1:]
    mode = "both"
    files = []
    index = 0
    try:
        while index < len(args):
            arg = args[index]
            if arg == "--self-test":
                return self_test()
            elif arg in ("-h", "--help"):
                print(__doc__)
                return 0
            elif arg == "--markdown":
                mode = "markdown"
            elif arg == "--json":
                mode = "json"
            elif arg.startswith("-"):
                raise SummaryError("unknown option " + arg)
            else:
                files.append(arg)
            index += 1
        if not files:
            raise SummaryError("no record file given")
        runs = []
        for path in files:
            runs.extend(parse_file(path))
    except SummaryError as exc:
        print("error: %s" % (exc or "bad arguments"), file=sys.stderr)
        print(__doc__, file=sys.stderr)
        return 2
    if not runs:
        print("error: no records found in %s" % ", ".join(files), file=sys.stderr)
        return 2
    summary = summarize_runs(runs)
    if mode in ("markdown", "both"):
        print(render_markdown(summary))
    if mode == "both":
        print("\n## JSON\n")
    if mode in ("json", "both"):
        if mode == "both":
            print("```json")
        print(render_json(summary))
        if mode == "both":
            print("```")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
