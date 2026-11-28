# check-profile-tags

`scripts/check-profile-tags.sh` lints the evidence tags in the doc comments of
the `DriverProfile` source files. POSIX `sh` plus `awk`. It reads files only
and changes nothing. It is not run by CI.

## What it checks

It looks at every doc-comment bullet: a line starting with `///` or `//!`
followed by `- `. The bullet includes its continuation doc lines, up to the
next bullet, a blank doc line (`///` alone) or a non-doc line. Per bullet:

1. It must contain one of the words `observed`, `source` or `inferred`
   (case-insensitive, whole word; `sources` does not count).
2. If it uses the word `observed`, it must also contain a `YYYY-MM-DD` date
   anywhere in the bullet.

Source pins (URLs, commits) are not checked here. Doc comments that are not
bullets are ignored.

## Usage

```
sh scripts/check-profile-tags.sh [FILE...]
```

There are no flags. Any first argument starting with `-` is a usage error.

With no arguments it checks `crates/composition-drm/src/profile.rs` and
`crates/composition-drm/src/profile/*.rs`, whichever exist. With none present
it exits 2. With arguments it checks exactly those files, in order, and every
one must be a regular file (checked before any file is linted).

Findings are printed as `file:line: reason`, where `line` is the bullet's first
line:

```
f.rs:3: bullet has no evidence tag (observed / source / inferred)
f.rs:3: observed without a YYYY-MM-DD date
```

## Exit codes

| code | meaning |
|---|---|
| 0 | no findings |
| 1 | at least one finding (all files are still checked) |
| 2 | usage error: option argument, no default files, or an argument that is not a file |

## Self-test

```
sh scripts/check-profile-tags.test.sh
```

It runs the script on the fixtures in `scripts/testdata/check-profile-tags/`
(`good.rs`, `bad-notag.rs`, `bad-nodate.rs`). It checks: a clean file gives 0;
a missing tag and a missing date give 1 with the expected `file:line` message;
several files in one run; an option argument and a missing file give 2; and the
two bad fixtures give exactly two finding lines together. It exits non-zero if
any case fails. `scripts/check-all.sh` runs this self-test; per a comment in
that script it leaves the lint itself out, because the current `profile.rs`
had observed bullets without a date when that comment was written.

## Difference from `check-doc-tags.sh`

| | `check-profile-tags.sh` | `check-doc-tags.sh` |
|---|---|---|
| Target | Rust doc-comment bullets (`///`, `//!`) | Markdown hardware docs, default `docs/imx95-*.md` |
| Unit checked | each bullet | each file, and each line using `observed` as a tag |
| Tags | `observed`, `source`, `inferred` as plain words | `observed`, `read from source`, `inferred` |
| Tag required | on every bullet | at least once per file |
| Date for `observed` | in the same bullet | on the line or the nearest earlier line of the same heading section |
| Source pins | not checked | github.com / git.toradex.com / raw URLs need a 40-hex SHA or a tag within two lines |

Both exit 0 / 1 / 2 the same way and print `file:line: reason`.
