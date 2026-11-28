# check-m1-report

Structural lint for an i.MX95 M1 test report, against the report shape that
`SoW.md` section 9 M1 asks for. POSIX `sh`, no dependencies.

It checks that the report is complete and consistent in form. It does not
decide whether M1 passed, and running it says nothing about the M1 status.

```
sh scripts/check-m1-report.sh [report.md]
sh scripts/check-m1-report.sh --self-test
```

The default report is `docs/m1-imx95-report.md` in the repository holding the
script.

## What it checks

The report is read case-insensitively. Bold, backticks and leading bullets,
pipes and quotes are ignored.

- **Verdicts.** A line `Condition N: passed|failed|blocked` for each N from 1
  to 7 (`=` works in place of `:`), and a line
  `Overall M1 verdict: passed|failed|blocked`. Text after the verdict word is
  allowed if it starts with a non-letter.
- **Fields.** A line `<Label>: <value>` with a non-empty value for each of:
  board revision, panel and interface, BSP/kernel version, lease route and
  version, resolution and refresh, plane assignments, errors, frame loss,
  latency, CPU, memory. The value may be a number, text or `not measured`.
- **Date.** At least one date in `YYYY-MM-DD` form.
- **Passed needs evidence.** Only if the overall verdict is `passed`:
  condition 3 must also be `passed`. Its block is the condition 3 line up to
  the next `Condition` line or heading. The block must cite a picture
  (`.png`, `.jpg` or `.jpeg`) that exists, with the path taken relative to the
  repository root or to the report's directory. It must also name plane IDs,
  such as `plane 31`, `planes 31 and 45` or `plane IDs: 31`.

Each problem is printed on standard error as `<report>: <message>`. The script
reports all problems it finds, not just the first.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | No problems found |
| 1 | One or more problems found (or `--self-test` failed) |
| 2 | The report file does not exist |

## Self-test

`--self-test` runs the checks on the fixtures in `scripts/testdata/m1-report/`.
`good.md` must be accepted. `bad.md` must be rejected with the messages for
missing verdicts, the `latency` and `memory` fields, a missing date and
condition 3 being `blocked`. `bad-pictures.md` must be rejected for a missing
picture and missing plane IDs. It prints `self-test: ok` and exits 0, or
prints what failed and exits 1.

## What it does not check

- That any verdict is true, or that any measurement was taken.
- That field values are plausible, or that a number has units.
- That the date is the date of a run.
- That a cited picture shows what the report says, or that the named plane IDs
  exist on the board. Only that the file exists and plane IDs are written.
- Conditions 1, 2 and 4 to 7 beyond the verdict line and the fields above.
- Evidence tags (observed, read from source, inferred); use
  `scripts/check-doc-tags.sh` for those.
- Links in the report; use `scripts/check-doc-links.sh` for those.
