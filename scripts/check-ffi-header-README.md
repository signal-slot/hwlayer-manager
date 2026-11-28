# check-ffi-header

`bash scripts/check-ffi-header.sh [BASE_REF]` keeps the C header in step with
the FFI crate. It enforces the rule in `CONTRIBUTING.md`: the header moves in
the same commit as `crates/composition-ffi/src/lib.rs` whenever the ABI
surface of that file changes. Exit 0 if every commit passes, 1 if any fails,
2 if the base ref cannot be resolved. Self-test:
`bash scripts/check-ffi-header.test.sh`.

## Usage

```
scripts/check-ffi-header.sh [BASE_REF]
```

The script parses no flags, only one optional positional argument. Run it
inside the git repository (it uses `git` on the current directory).

- `BASE_REF`: the commits checked are those in `BASE_REF..HEAD`, merge
  commits skipped, oldest first. Default `origin/main`. When no argument is
  given and `origin/main` does not exist, it falls back to `main`. An
  explicit `BASE_REF` that does not resolve is an error (no fallback).

## What it compares

The two files, as the script names them:

- `crates/composition-ffi/src/lib.rs` (the Rust FFI source)
- `crates/composition-ffi/include/composition_engine.h` (the C header)

For each commit that touches `lib.rs`, it builds the "ABI surface" of
`lib.rs` at the parent and at the commit. If the two surfaces are equal the
commit passes. If they differ, the commit passes only when it also changes
the header; otherwise it fails. A commit that does not touch `lib.rs` always
passes. For a root commit the parent surface is empty.

The ABI surface is the normalised text of:

- every item introduced by `#[no_mangle]` / `#[unsafe(no_mangle)]` or
  containing `extern "C"`: from the attribute or first line up to and
  including the line with the opening brace or the terminating `;` (so
  signatures spread over several lines are seen, bodies are not);
- every item introduced by `#[repr(C...)]`: the attribute, derives and the
  whole struct/enum/union up to its closing brace at column 0 (or its `;`).

Comment-only lines (`//`, `///`, `//!`) are dropped and whitespace is
collapsed, so edits to comments, function bodies, formatting or unrelated
Rust items pass.

## Output

One line per checked commit, with the abbreviated SHA:

```
ok <sha>
fail <sha>: <reason>
```

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | every commit in the range is `ok` (an empty range is also 0) |
| 1 | at least one `fail` line |
| 2 | the base ref cannot be resolved (`error:` on stderr) |

## Limits

It is a heuristic that guards against forgetting the header; it does not
prove the header is right.

- Any edit inside a `repr(C)` type counts as a surface change.
- A `repr(C)` item whose closing brace is indented is read up to the next
  statement-ending `;`.
- Only the presence of a header change is checked, not its content.

## Test

`bash scripts/check-ffi-header.test.sh` builds a throwaway git repository
under `${TMPDIR:-/tmp}` (removed on exit) with a sample `lib.rs` and header,
makes commits that change or leave alone the ABI surface, and runs the
checker on each. It does not touch the real repository.
