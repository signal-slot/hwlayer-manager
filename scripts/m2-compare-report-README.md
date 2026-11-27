# m2-compare-report.py

Builds the WP3 Linux vs Zephyr comparison table (metrics M-1 to M-7 of
`docs/m2-comparison-plan.md` §3) from two run logs. Python 3, standard library
only. Exit 0 ok, 2 input error (unreadable file, missing or malformed date,
a log whose `side=` is the wrong one).

    python3 -I scripts/m2-compare-report.py linux.log zephyr.log
    python3 -I scripts/m2-compare-report.py --json linux.log zephyr.log

Output: a Markdown table with columns metric, Linux, Zephyr, difference
(Zephyr minus Linux), under a `measured on <date>` heading taken from the
`RUN date=` lines (`Linux <d>, Zephyr <d>` when the dates differ). A metric
missing from one log, or with an empty `value=`, prints `not measured`.

## Log format

The docs leave the summary format open (`docs/trace-format.md` is a proposal
for per-event lines, not a summary), so this script defines a simple one. Lines
starting with `#` and unknown lines are ignored. Values may contain spaces; a
field runs to the next ` key=`. Quote a name with spaces.

| line | fields |
|---|---|
| `RUN` (required, once or more) | `date=YYYY-MM-DD`, `side=linux\|zephyr`, optional `stage`, `commit`, `tool` |
| `METRIC` | `id=` (e.g. `M-1.median`), `name=`, optional `unit=`, `value=` |

Rules: a numeric `value` with the same `unit` on both sides gives a signed
difference; different units print `units differ`; non-numeric values (for
example `not measurable (reason)`) are shown as written and the difference is
`n/a`. Rows keep the Linux log's order, then Zephyr-only metrics. Values are
not interpreted: the script makes no pass or verdict statement.

Self-test: `scripts/m2-compare-report.test.sh`. Fixtures in
`scripts/testdata/m2-compare-report/` are synthetic (invented numbers), not
board data, and make no hardware claim.
