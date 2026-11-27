# scenario-report

`python3 scripts/scenario-report.py --board NAME [--pictures FILE] [--disclaimer] LOG...`
turns the output of `scripts/board-scenarios.sh` into one board section of a
dated Linux board results doc, in the layout `scripts/check-board-results.sh`
requires (`scripts/check-board-results-README.md`). Python 3, standard library
only. Self-test: `sh scripts/scenario-report.test.sh`.

## Input

- `LOG...`: one or more captures of `board-scenarios.sh` stdout (lines
  `A1 startup PASS`). If a log also contains the `result.txt` text
  (`SCENARIO name RESULT`, then evidence and `failed:` / `BLOCKED:` lines),
  the `failed:` and `BLOCKED:` reasons go into Detail. A later log, or a later
  line, replaces an earlier one for the same row.
- **Host time.** `board-scenarios.sh` prints none, so the caller supplies it,
  either as a prefix (`2026-11-28T10:05:00Z A3 stop-restart PASS`) or as a
  line `host time: 2026-11-28T10:00:00Z` that covers the rows after it. The
  format is ISO 8601 `YYYY-MM-DDThh:mm:ss[Z|+hh:mm]`.
- `--pictures FILE`: `picture-hook.sh`'s `pictures.log`
  (`ROW PHASE TIME FILE VERDICT`). Each line adds `picture PHASE VERDICT` to
  that row's Detail. The file path is dropped, and the picture time is never
  used as the row's host time.

## Output

Markdown on stdout: `## Board: NAME` and a table
`| Row | Verdict | Host time | Tag | Detail |` with rows A1-A6.

- Tag is always `observed` (the row comes from a run's log).
- Verdict: `PASS`, `FAIL`, `BLOCKED`; the script's `NOT-RUN` becomes
  `SKIPPED` (Detail says `not run`).
- A row missing from every log is `BLOCKED`, Detail `not in log`.
- No host time is ever invented. If the log gives none for a row, the cell is
  left empty, Detail ends `host time missing from log`, and the exit status
  is 3 (the checker then also flags the row). Fill the time in by hand from
  your own record, or re-run with times.
- `--disclaimer` first prints the line the checker requires (`portability
  evidence ... never count toward M1, M2 or M3`). Print it once per doc; with
  several boards, give it to the first only.

`NAME` is a plain label (letters, digits, `_`, `+`, `-`): no address or host
name.

## Redaction

Copied text (names, reasons, picture verdicts) is scrubbed: IPv4 and MAC
addresses, URLs, `user@host`, dotted host names (names ending in a file
extension such as `rotate.log` are kept), `serial ...` values and
`/dev/tty*`, `/dev/serial*` paths become `<redacted>`. `|` becomes `/`. This
is a heuristic: still run `scripts/check-no-board-secrets.sh` on the doc.

## Exit status

0 clean, 2 usage error (bad `--board`, missing file), 3 some row has no host
time. *The time sources, the SKIPPED mapping and the redaction rules are
choices by the implementer; the owner may revisit them.*
