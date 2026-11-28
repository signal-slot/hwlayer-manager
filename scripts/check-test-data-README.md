# check-test-data

`scripts/check-test-data.sh` lints the dated board data kept under
`crates/composition-drm/tests/data`. That is the default data root; the script
does not read `scripts/testdata`. Bash and standard tools (`find`, `grep`,
`git`); it changes nothing.

## Usage

```
scripts/check-test-data.sh [DATA_ROOT]
```

`DATA_ROOT` is the only argument. Default: `crates/composition-drm/tests/data`
of the repository the script lives in. Any option (an argument starting with
`-`) or more than one argument is a usage error.

Environment: `CHECK_NO_BOARD_SECRETS=PATH` replaces the secrets script (used
by the self-test); the default is `scripts/check-no-board-secrets.sh`.

## What it checks

Each `DATA_ROOT/<soc>/` folder is checked on its own:

1. `README.md` exists in the folder.
2. Every other file (recursively) is named in that `README.md`, by relative
   path or base name, matched as a fixed string.
3. Every data file carries a `YYYY-MM-DD` date in its name, in its first 10
   lines, or on a `README.md` line that names it.
4. No file is over 1 MiB (1048576 bytes).
5. `scripts/check-no-board-secrets.sh` reports nothing for the folder. That
   script takes no paths and scans the added lines of the git diff against
   its base ref, so it is run from the data root and only hits under the
   folder count. A root outside a git work tree skips this check, with a
   note on stderr.

A missing root, or a root with no folders, prints a note and exits 0.

Output is one `FILE: reason` line per problem, for example
`rk3588/props.txt: not named in README.md`. Secret hits are printed as
`FILE: board secret (KIND) at line N`.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | no problems, or nothing to check (missing root, no folders) |
| 1 | at least one problem was printed |
| 2 | usage error (an option or too many arguments), or the script directory or root cannot be entered |

## Test

```
bash scripts/check-test-data.test.sh
```

It builds good and bad data trees under `$TMPDIR` and runs the script with a
stub in place of the secrets script. Cases: a clean tree, a missing root, a
root with no folders, a missing `README.md`, a file not named in the README,
an undated file, a file over 1 MiB, a secret hit, and a bad option (exit 2).
It then runs the script on the current tree with the real secrets script and
requires exit 0. It prints `all tests passed` and exits 0 on success.
