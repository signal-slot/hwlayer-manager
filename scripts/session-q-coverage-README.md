# session-q-coverage.sh

Checks that a session doc covers named §7 questions with evidence tags.

    scripts/session-q-coverage.sh <doc.md> Q1 Q4 Q9 ...

For each question the script looks for lines naming it as a whole word
(`Q4`, not `Q41`). The scope is the table row if the line is a table row,
otherwise the heading section holding the line (down to the next heading of
the same or higher level). The scope must carry one of the tags that
`scripts/check-doc-tags.sh` accepts: `observed` (the scope must also hold a
YYYY-MM-DD date), `read from source`, or `inferred`.

Output, one line per question: `COVERED Qn <observed|read-from-source|inferred>`
or `MISSING Qn` (with `(named, no tag)` when named but untagged). Exit 1 if
any is MISSING, 2 on usage errors, else 0. Not wired into CI.

Test: `bash scripts/session-q-coverage.test.sh`.
