# trace-to-metrics.py

Converts one engine trace log in the `CETRACE v=1` format of
`docs/trace-format.md` into the `RUN` and `METRIC` lines that
`scripts/m2-compare-report.py` reads (see `m2-compare-report-README.md`).
Python 3, standard library only. Exit 0 ok, 2 input error.

    python3 -I scripts/trace-to-metrics.py [--stage S] [--refresh-us N] LOG > linux.txt
    python3 -I scripts/trace-to-metrics.py [--stage S] LOG > zephyr.txt
    python3 -I scripts/m2-compare-report.py linux.txt zephyr.txt

## Input

Lines not starting with `CETRACE` are ignored, except one comment line
`# date: YYYY-MM-DD` (or `# date=...`) that the producer of the log must add:
the trace format carries no calendar date, so the script takes it from the
log and never invents or defaults one. No date comment (or two different
ones) is exit 2. Also exit 2: no `ev=hdr` line, `src=` not `linux`/`zephyr`,
an unknown `v=`, a non-numeric `seq`/`t`/`tev`/`out`, a second `hdr`,
an unreadable file. The `RUN side=` comes from `hdr src=`; `--stage` adds
`stage=`.

## Time conversion

`ns = (t - t0) * 1e9 / freq_hz` in integer arithmetic, using `hdr` `t0` and
`freq_hz`. For `clock=cyc32` the counter is unwrapped (one wrap of 2^32
assumed between consecutive lines; `tev` is placed relative to the `t` of its
own line). For `latched`, `tev` is used when present, else `t`. If the `hdr`
has no usable `freq_hz`/`t0`, no time metric is emitted. Times are printed in
ms with six decimals (the unit of `unit=ms`).

## Metrics emitted (ids of `docs/m2-comparison-plan.md` §3)

| id | Derivation |
|---|---|
| `M-1.median`, `M-1.max` | differences of consecutive latch times, per output, pooled. Needs two latches. |
| `M-2a.median`, `.p95`, `.max` | `commit result=ok` line time `t` to the latch time of the same `(out, sub)`. p95 is nearest-rank. |
| `M-3.submitted`, `.latched`, `.dropped`, `.timeouts`, `.pending` | counts of `commit ok`, `latched`, `dropped`, `latch_timeout`; pending = committed with neither latched nor dropped. |
| `M-5.plane` | layer, plane and zpos of the first `validate result=ok` with `assign=`. |
| `M-6.violations` | `seq` gaps or repeats, a `latched`/`dropped` for a `(out, sub)` never committed or already ended, a repeated commit id. |

A metric that cannot be derived is left out, so `m2-compare-report.py` shows
`not measured`: M-2b (latch-to-light), M-4 (CPU load), M-7 (memory) never
come from a trace; M-1/M-2a are absent without latches. M-1's outlier count
and the late-latch count need a refresh period the log does not carry and are
not derived unless `--refresh-us` is given (below). The script makes no pass or verdict statement and the numbers are
only as good as the log: M-2a on Zephyr is only valid for a log whose `tev`
comes from a real display interrupt (`docs/m2-linux-zephyr-comparison.md`).

## Optional `--refresh-us N`

`N` is the refresh period in microseconds, a positive integer (anything else
is exit 2). The caller supplies it; the log does not carry it. Without the
flag the output is unchanged. With it, and only when the log has at least two
latches on one output, two more lines are derived from the M-1 interval list:

| id | Derivation |
|---|---|
| `M-1.outliers` (`unit=intervals`) | latch intervals deviating from `N` by more than half a period: `abs(interval - N) > N/2` (too short or too long). |
| `M-3.late` (`unit=intervals`) | latch intervals above 1.5 periods (`docs/m2-comparison-plan.md` M-3). Printed after the other M-3 lines. |

Integer arithmetic in ns (`N * 1000`). The counts describe the log against the
stated `N`; whether `N` is the real panel refresh is the caller's claim and is
not checked here.

## Test

`scripts/trace-to-metrics.test.sh`. The fixtures in
`scripts/testdata/trace-to-metrics/` are **synthetic** (invented numbers, dates
and `build="synthetic"`), not board data, and make no hardware claim.
