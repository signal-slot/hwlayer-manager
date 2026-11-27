# harness-summary.py

A host-side script that turns `drm-harness` record files into the numbers
SoW section 9 M1 acceptance condition 7 asks for. It reads one or more log
files written by `tools/drm-harness` (for example
`/tmp/drm-harness ... all | tee /tmp/harness.log` on the board, or a vkms
run), merges the runs, and prints a markdown table and a JSON summary per
scenario and per source (`master`, `kernel-lease`, `dlm`).

It changes nothing and measures nothing itself. Python 3 standard library
only; run it on the host, not on the board.

## Usage

```sh
python3 scripts/harness-summary.py RECORD_FILE...            # markdown + JSON
python3 scripts/harness-summary.py RECORD_FILE... --markdown # markdown only
python3 scripts/harness-summary.py RECORD_FILE... --json     # JSON only
python3 scripts/harness-summary.py --self-test               # built-in checks
```

Exit status: 0 when a summary was printed — a `FAIL` verdict inside the
records is data, not a script error. 2 on a usage or file-read error.

## What it reads

Only the documented one-line record format of `tools/drm-harness`
(`kind key=value ...`, quoted values, `VERDICT <scenario> PASS|FAIL <reason>`;
see `tools/drm-harness/README.md`, `src/record.rs` and `src/stats.rs`). A run
starts at each `session` record; every later record belongs to that run until
the next `session`. Runs are grouped by `(scenario, source)` across all input
files, so two files of the same scenario and source merge into one group with
`runs: 2`.

Fields used, per condition-7 number:

| Number | From |
|--------|------|
| resolution, refresh | `output` record of the selected connector (`width`, `height`, `mode`, `refresh_mhz`) |
| plane assignment | `assign` / `plan` records per commit step; `frame` records for `move` (bottom/top plane per frame); `rotate_result.planes` |
| errors | `violation`, `error`, `step_result` records, `combos_result.errors`, `FAIL` verdict reasons |
| frame loss | `rotate_result` (`frames`, `presented`, `dropped`, `missed_vblanks`, `latch_timeouts`); frame loss = frames − presented |
| commit→latch latency | `move_result` / `rotate_result` summary fields (`latency_min_us` ...); otherwise computed from `commit` / `frame` records' `latency_us` |
| validate / commit / interval time | same summary fields (`validate_*`, `commit_*`, `interval_*`) |
| CPU, memory | `resources` record (`wall_ms`, `cpu_user_ms`, `cpu_sys_ms`, `cpu_pct`, `rss_kib`, `peak_rss_kib`) |
| lease route | `lease` record (`route=kernel` or `route=drm-lease-manager`, `lessee`) |
| verdict | `VERDICT` line (PASS only if every run of the group passed) |

Record kinds it does not know are ignored, so a harness that writes new
kinds still summarizes fine (the new data stays in the log file).

## Rules the summary follows

- **`not measured`, never guessed.** A number with no record behind it is
  printed as `not measured` in both the markdown tables and the JSON.
- **Latency prefers the harness's own numbers.** The `move_result` /
  `rotate_result` summaries already exclude the warm-up frame
  (`warmup_frames=1`), so they are used as-is when present. They are used
  instead of — never mixed with — the per-frame `latency_us` values.
- **Merging runs of a group:** `count` sums; `min`/`max` are exact; `median`
  is the median of the run medians; `p99` is the largest run p99 (an upper
  bound for the pooled samples); `mean` is the count-weighted mean. Percentiles
  follow `src/stats.rs`: nearest-rank, median = lower middle, mean rounded
  down.
- **Resources:** `wall_ms`, `cpu_user_ms`, `cpu_sys_ms` are summed over runs
  (the harness measures only its own process; `--producers external` producer
  CPU is not measured by the harness); `cpu_pct`, `rss_kib` and
  `peak_rss_kib` take the largest run value. A group that ran with both
  producer modes lists both in the `producers` column and merges their numbers.
- **Frames:** counts come from `rotate_result`; `frame loss = frames −
  presented`. `move` and `combos` report no presented count, so their frame
  columns stay `not measured`; a `move` that dropped a frame shows it in
  `errors` (the harness stops with a `submission N was dropped` error and a
  FAIL verdict).

## Coverage of M1 condition 7

Board revision, BSP/kernel version and the lease stack version are **not in
the harness records**. They belong in the M1 report text
(`docs/m1-imx95-report.md`), together with this script's output. Panel and
interface come from the `connector` column; the lease route from `lease`.

## Self-test and synthetic test data

`python3 scripts/harness-summary.py --self-test` checks the percentile and
summary definitions against the unit tests of `src/stats.rs`, the record
tokenizer against `src/record.rs`, and the merged numbers against
hand-computed expectations over the files in
`scripts/testdata/harness-summary/`.

Those `.log` files are **SYNTHETIC**: hand-made to the documented format, not
board measurements. Each file says so in its header. Never quote their
numbers as evidence of a board run.
