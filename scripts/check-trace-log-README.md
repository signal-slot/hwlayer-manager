# check-trace-log.sh

Checks trace logs against the header rules of
[`docs/trace-format.md`](../docs/trace-format.md) (section 3.1 version,
section 5.1 `# date:` line and the not-a-hardware-result comment). POSIX
`sh` and `awk`, no other dependencies. Only the shape is checked, as the
doc does; no date is validated as a real calendar date.

    sh scripts/check-trace-log.sh run1.log run2.log

One result per log on stdout: `ok`, or `fail <file>:<line>: <reason>` (the
first failure of that log only). Warnings go to stderr.

## Exit codes

| code | meaning |
|---|---|
| 0 | every log passed (warnings do not change this) |
| 1 | at least one log failed; every log is still reported |
| 2 | usage error: no arguments, an argument starting with `-`, or an unreadable file (nothing is checked) |

## Rules

A log passes when all of these hold. Carriage returns at line end are ignored.

- **CETRACE line.** At least one line starting with `CETRACE`; otherwise
  `no CETRACE line` (reported at line 1, so an empty file fails).
- **Version.** Every `CETRACE` line has `v=1` as its first field. Another
  or missing version fails with `unsupported or missing version`.
- **Date.** A comment line `# date: YYYY-MM-DD` (`:` or `=`, white space
  optional, nothing after the date but white space). Missing, or a line
  that is not of this shape, fails with `no "# date: YYYY-MM-DD" comment
  line`. The line may repeat; a copy with a different date fails with
  `date … differs from …`, naming the first line.
- **Host and dry runs.** If a comment line mentions `host`, `vkms` or
  `dry run` (also `dry-run`, `dryrun`), a comment line containing `not a
  hardware result` must also exist; otherwise the log fails, at the
  marking line. Matching ignores case.

## Errors and warnings

Errors are the failures above and set exit 1. There is one warning, which
leaves the result `ok` and exit 0: the log is neither marked as a
host/dry/vkms run nor says `not a hardware result`, and it has no hardware
evidence comment (`hardware evidence` or `board:` / `board=`). It is
printed as `warn <file>: …` on stderr, so a log of unknown origin is
flagged without being rejected.

The script does not prove a log came from hardware: a board log passes on
the header text alone.

## Test

`scripts/check-trace-log.test.sh` covers each rule, the exit codes and the
warning. Its fixtures in `scripts/testdata/check-trace-log/` are synthetic,
not board data.
