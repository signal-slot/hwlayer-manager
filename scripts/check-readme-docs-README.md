# check-readme-docs

Checks that `README.md` links every top-level entry of `docs/`, files and
directories alike. POSIX `sh`, no dependencies.

```
sh scripts/check-readme-docs.sh [repo-root]
```

For each entry `docs/<name>` it looks for that string in `README.md`. It
prints `missing docs/<name>` for each entry without a match and exits 1, or
prints `ok` and exits 0. A missing `README.md` or `docs/` is an error: message
on standard error, exit 2. The default root is the repository holding the
script.

The match is a plain substring, so `docs/sub/` or `docs/sub/x.md` counts as a
link to the directory `docs/sub`. It does not check that the link resolves;
`scripts/check-doc-links.sh` does that.

The script is not part of `scripts/check-all.sh`. It can fail on the real
repository while `README.md` lacks links to newer docs.

## Test

```
sh scripts/check-readme-docs.test.sh
```

Builds fake repositories under a `mktemp` directory (in `$TMPDIR`) and covers
all linked, one missing, a directory entry and a missing `README.md`.
