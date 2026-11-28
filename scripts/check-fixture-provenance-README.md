# check-fixture-provenance

`scripts/check-fixture-provenance.sh` lints recorded board data in the
repository's Rust test code (plan criterion c4). Recorded board data is copied
into the tests, or into `crates/*/tests/data/`, and the test says where it came
from and when. Tests never read `docs/` at run time. POSIX `sh` and `awk`; no
other dependencies. It only reads files.

## Provenance it requires (read from the script)

Two checks, applied to each file:

- **runtime-read.** `include_str!`, `include_bytes!`, `read_to_string`,
  `File::open` or `fs::read` whose argument (on that line or the next two)
  mentions `docs/platform-capabilities.md` or the retired notes directory under
  `docs/`. Comment lines are ignored. Fix: copy the data into the test, with
  source and date.
- **provenance.** A file that mentions a recorded board (`nanopc`, `rk3588`,
  `bcm2711`, `bcm2712`, `cm4`, `rpi5`, case-insensitive) and embeds data (a raw
  string literal, a `const`/`static` of type `&str` or `&[..]`, or an
  `include_str!`/`include_bytes!`) needs one comment block (consecutive `//`
  lines) that names a source, either `tests/data/PATH` or
  `docs/platform-capabilities.md`, and a `YYYY-MM-DD` date. The source and the
  date must be in the same block. The retired notes directory is not a source.
  A comment line `provenance: synthetic` declares that the file holds no
  recorded board data (a board name used only as a string); it silences this
  check for the file.

## Usage

```
scripts/check-fixture-provenance.sh [FILE.rs ...]
```

With no arguments it checks `crates/*/tests/**/*.rs`,
`crates/*/examples/*.rs` and `tools/**/*.rs` (skipping `target/`), found from
the repository root whatever the working directory. With arguments it checks
exactly those files. Output is `file:line: kind: message` per finding, or
`check-fixture-provenance: clean` when there are none.

Flags: there are none. The script parses no options. Any first argument
starting with `-` (including `-h`) prints the usage line to stderr and exits 2.

## What passes and fails

From `scripts/check-fixture-provenance.test.sh` and its fixtures in
`scripts/testdata/check-fixture-provenance/`:

| Case | Result |
| --- | --- |
| `good.rs`, `good-nodata.rs`, `good-other.rs`, `good-caps.rs`, `good-synthetic.rs` | clean |
| `bad-nosource.rs`: board data, no source in the comment | `provenance` at line 1 |
| `bad-nodate.rs`: source named, no date | `provenance` at line 1 |
| `bad-split.rs`: source and date in separate comment blocks | `provenance` at line 1 |
| `bad-include.rs`: `include_str!` of a recorded-data doc | `runtime-read` at line 2 |
| a comment citing the retired notes directory as the source | `provenance` |
| `include_str!` of a file in the retired notes directory | `runtime-read` |
| `good.rs` + `bad-nodate.rs` | the bad file is reported, rc 1 |
| `good.rs` + `bad-nodate.rs` + `bad-include.rs` | exactly two finding lines |

The check is a heuristic on text. It cannot tell whether the named source
really holds the data or whether the date is true.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | no findings (`check-fixture-provenance: clean`) |
| 1 | at least one finding |
| 2 | usage: a first argument starting with `-`, an argument that is not a file (`not a file`), or the file list could not be built |

## Test

```
sh scripts/check-fixture-provenance.test.sh
```

Runs the cases above, plus two temporary files in `$TMPDIR` for the retired
notes directory (its path is assembled in the test so no file names it). Prints
`ok NAME` or `FAIL NAME`; exit 0 when all pass.

## Relation to CONTRIBUTING.md

`CONTRIBUTING.md` ("Measurements and hardware claims") says a timing or
hardware claim is a dated measurement taken on the named hardware, or is marked
*not measured*. This script applies that rule to test fixtures: recorded board
data must carry a date and a named source, so a reader can tell when and where
it was observed. It checks that a date and a source are present; it does not
check that they are correct, and it does not check claims in documents. A
fixture that is not recorded data must say `provenance: synthetic` rather than
be left undated.
