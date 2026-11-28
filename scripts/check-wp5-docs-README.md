# check-wp5-docs

Checks the WP5 documents `docs/wp5-proposal.md` and `docs/wp5-demo.md`.
POSIX `sh`, no dependencies.

```
sh scripts/check-wp5-docs.sh [repo-root]
```

A file that does not exist prints `skip <file>` and is not a failure, because
the docs are written by parallel work. For each file that exists:

1. a `YYYY-MM-DD` date occurs in the first 10 lines;
2. at least one evidence tag occurs: `observed <date>`, `read from source`,
   `inferred`, `not measured` or `host only` (case-insensitive). The tag words
   follow `scripts/check-doc-tags.sh`, which checks the per-line date rules of
   `observed`; this script does not repeat that;
3. `docs/wp5-proposal.md` has no line with `will be accepted`, `guaranteed` or
   `accepted upstream`, unless the same line also has the word `not` or `no`;
4. `docs/wp5-demo.md` has a heading containing `Walk-through`.

One line per failure (`<file>[:line]: <reason>`), then exit 1; otherwise `ok`
and exit 0. A missing repo root is an error: message on standard error, exit 2.
The default root is the repository holding the script.

The line check is a keyword match, not a reading of the text. It does not
replace review of the proposal.

The script is not part of `scripts/check-all.sh` or CI.

## Test

```
sh scripts/check-wp5-docs.test.sh
```

Builds fake repositories under a `mktemp` directory (in `$TMPDIR`) and covers
pass, each failure, the skip and the missing root.
