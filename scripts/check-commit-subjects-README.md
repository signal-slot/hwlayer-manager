# check-commit-subjects

Lints the commit subjects of a branch against the "Commit messages" section of
`CONTRIBUTING.md`: a component prefix (`engine: ...`, `ffi: ...`, `drm: ...`)
or a plain sentence, with the design explained in the body. POSIX `sh` and
`git`; nothing else. It only reads the repository.

## Usage

```
sh scripts/check-commit-subjects.sh [BASE_REF]
```

`BASE_REF` is any ref `git rev-parse` resolves to a commit (branch, tag, SHA).
The script checks every non-merge commit in `BASE_REF..HEAD`, oldest first.

- Without an argument the base is `origin/main`. If that ref does not exist,
  it falls back to `main`.
- With an explicit argument there is no fallback: an unresolvable ref is an
  error.

Output is one line per commit: `ok <sha>` or `fail <sha>: <reason>` (short
SHA). The first failing check decides the reason.

## What it rejects

Checked in this order; the first match is the reason printed.

| Reason | Example subject (from the test) |
| --- | --- |
| empty subject | (empty) |
| more than 72 characters | `engine: 0123...` of 73 or more characters |
| prefixed subject ends with a period | `drm: fix thing.` |
| subject contains the word `WIP` or `wip` | `engine: WIP scanout` |
| subject starts with `fixup!` | `fixup! engine: x` |
| the diff touches a path under `crates/` and the body is empty | `ffi: change` with no body |

A prefix is a letter followed by letters, digits, `_` or `-`, then `: `
(for example `CI: ...`).

## What it accepts

From the test:

- `engine: add plane assignment`, with a body, touching `crates/`.
- `Plain sentence with a period.` A plain sentence (no prefix) may end in a
  period.
- `docs: no body needed outside crates`, with no body, touching only `docs/`.

Only the `crates/` body rule is enforced. The general request in
`CONTRIBUTING.md` for a body that explains the design is not checked
elsewhere, and neither is the wording of the subject beyond the rules above.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | every commit in the range is `ok` (also when the range is empty) |
| 1 | at least one `fail` line |
| 2 | the base ref cannot be resolved (`error: cannot resolve base ref ...` on stderr), or the temporary file or `git rev-list` failed |

## Test

```
sh scripts/check-commit-subjects.test.sh
```

It builds a throwaway repository under `$TMPDIR` (default `/tmp`) and checks:
three good commits exit 0 with three `ok` lines; each rejection above
(prefixed trailing period, over 72 characters, `WIP`, `fixup!`, `crates/`
without body, empty subject) exits 1 with the matching reason; an unknown ref
exits 2. It prints `pass:`/`FAIL:` per case and `all passed` (exit 0) or
`N failed` (exit 1).
