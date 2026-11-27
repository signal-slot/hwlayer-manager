# check-board-results

`sh scripts/check-board-results.sh FILE...` checks the structure of a dated
Linux board results doc (rows A1-A6 of `docs/board-scenarios.md`). Exit 0 if
clean, 1 on findings (one `file:line: reason` per problem), 2 on usage errors.
Self-test: `sh scripts/check-board-results.test.sh` (fixtures in
`scripts/testdata/board-results/`).

## Required layout

*Layout chosen by the implementer; the owner may revisit it.*

1. **Disclaimer.** One line (case-insensitive) containing both
   `portability evidence` and `never count toward M1, M2 or M3`, e.g.
   `These runs are portability evidence and never count toward M1, M2 or M3.`
2. **Board sections.** One level-2 heading `## Board: NAME` per board, at
   least one. The section runs to the next heading of any level.
3. **Rows.** Inside a board section, one table row per scenario with the
   columns `| Row | Verdict | Host time | Tag | Detail |`. Rows A1, A2, A3,
   A4, A5 and A6 must each appear exactly once per board. Table lines whose
   first cell is not `A<number>` (header, separator) are ignored; `A7` and
   up are reported as unknown.
   - **Verdict**: exactly `PASS`, `FAIL`, `BLOCKED` or `SKIPPED`. BLOCKED and
     SKIPPED rows put the reason in Detail.
   - **Host time**: ISO 8601 date and time, `YYYY-MM-DDThh:mm:ss` optionally
     followed by `Z` or `+hh:mm`/`-hh:mm`. Required on every row, including
     BLOCKED and SKIPPED (the time the verdict was decided).
   - **Tag**: exactly `observed`, `read from source` or `inferred`
     (case-insensitive), the tags `scripts/check-doc-tags.sh` knows.
   - **Detail**: free text, not checked.

The checker does not check the tag/date pairing (that is
`check-doc-tags.sh`), board names, or that the host time matches the
verdict; run both checkers on a results doc.
