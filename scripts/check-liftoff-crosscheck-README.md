# check-liftoff-crosscheck.sh

Hand-run checks of the libliftoff cross-check crate `tools/liftoff-crosscheck`.
That crate runs this engine's planner and libliftoff on the same topologies and
frames (design: [`docs/libliftoff-crosscheck.md`](../docs/libliftoff-crosscheck.md);
results: [`docs/liftoff-crosscheck-report.md`](../docs/liftoff-crosscheck-report.md)).
It sits outside the root workspace, so this script is its only gate. It is
**not** part of CI and `scripts/check-all.sh` does not call it (`docs/ci.md` is
unchanged). It touches no board and makes no hardware claim.

## Usage

```
scripts/check-liftoff-crosscheck.sh
```

Run it from anywhere inside the repository; it changes to the repository root
with `git rev-parse --show-toplevel`. It takes **no command-line options or
arguments**; anything passed is ignored. The one switch is an environment
variable:

| Variable | Effect |
| --- | --- |
| `LIFTOFF_XC_STRUCTURE_ONLY` | If set to a non-empty value, skip the three cargo steps and run only the structural checks. The self-test uses it. |

## What it checks

Structural checks (always run):

1. `tools/liftoff-crosscheck/Cargo.toml` exists and has a `[workspace]` table
   that is empty (comments and blank lines allowed before the next table).
2. The root `Cargo.toml` does not list the crate in `members`.
3. No libliftoff source file is tracked by git (`alloc.c`, `plane.c`, `layer.c`,
   `device.c`, `output.c`, `libliftoff.h` anywhere in `git ls-files`).

Cargo steps (skipped with `LIFTOFF_XC_STRUCTURE_ONLY`, or if the manifest is
missing), with `CARGO_TARGET_DIR` set to the crate's own `target/`:

4. `cargo fmt --check`
5. `cargo clippy --all-targets -- -D warnings`
6. `cargo test`

All three use `--manifest-path tools/liftoff-crosscheck/Cargo.toml`. Every
failure prints a `FAIL: ...` line and the script carries on to the next check.

## Inputs

The git repository it is run in, `cargo` with `rustfmt` and `clippy`, and the
crate's own sources. If `tools/liftoff-crosscheck` does not exist it prints
`liftoff-crosscheck: crate not present, skipped` and exits 0. The cargo steps
need whatever the crate's tests need; see the crate's README.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | All checks passed (`liftoff-crosscheck: ok`), or the crate is not present. |
| 1 | At least one check failed (`FAIL: ...` lines), or the repository root could not be found. |

## Test

[`scripts/check-liftoff-crosscheck.test.sh`](check-liftoff-crosscheck.test.sh)
builds throwaway git repositories under `$TMPDIR`, runs the script with
`LIFTOFF_XC_STRUCTURE_ONLY=1` and checks exit code and output for: a good
tree, crate absent, missing or non-empty `[workspace]`, a root-manifest member
entry (and an `exclude` entry, which is fine), and each tracked libliftoff
source name. It never touches the real repository and does not run cargo.

```
bash scripts/check-liftoff-crosscheck.test.sh
```

## Links

- [`docs/libliftoff-crosscheck.md`](../docs/libliftoff-crosscheck.md): design (a proposal).
- [`docs/liftoff-crosscheck-report.md`](../docs/liftoff-crosscheck-report.md): host results of 2026-11-28; no board, no milestone evidence.
