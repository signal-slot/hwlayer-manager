# check-wp5-claims.sh

Lints the WP5 support docs for advantage or acceptance claims without an
evidence tag. POSIX sh plus awk; no dependencies.

    sh scripts/check-wp5-claims.sh [file ...]
    sh scripts/check-wp5-claims.test.sh

## Rule

A line is flagged when it contains (case-insensitive) `faster`,
`lower latency`, `less power`, `will be accepted`, `upstream will` or
`guarantees`, unless the same line or the same paragraph (text between blank
lines) contains `not measured`, `observed`, `measured`, `inferred` or
`read from source`. The word `unmeasured` does not count as a tag.

Exit status: 0 clean, 1 findings (printed as `file:line: untagged claim: ...`),
2 missing file.

## Default files

`docs/wp5-gap-analysis.md`, `docs/wp5-evidence-plan.md`,
`docs/advisory-planner-design.md`, `docs/libliftoff-crosscheck.md`,
`docs/liftoff-crosscheck-report.md`, and every other `docs/wp5-*.md` except
`wp5-proposal.md` and `wp5-demo.md` (those are covered by
`check-wp5-docs.sh`).

## Limits

- Paragraph-level tagging is lenient: one tag excuses every claim in the
  paragraph. It finds missing tags, not wrong ones.
- Code fences and tables are not treated specially (a table row is one line).
- Not part of `check-all.sh` or CI.

## Result of a run on the current docs

Run 2026-11-28 on the tree at commit df507ed (default file list, which
included `wp5-coexistence-survey.md` and `wp5-evidence.md` as other
`wp5-*.md` files): exit 0, no lines flagged. Findings: none. The docs were
not edited. Docs added by other plans after this commit were not checked.

`check-wp5-claims.test.sh` passes (pass, fail and missing-file cases using
`scripts/testdata/check-wp5-claims/`).
