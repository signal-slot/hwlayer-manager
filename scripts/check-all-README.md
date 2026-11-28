# check-all.sh

Runs the checks of the pull-request CI locally, in the order of `.github/workflows/ci.yml` (documented in [`docs/ci.md`](../docs/ci.md)), so a change can be checked before it is pushed. It installs nothing.

```sh
sh scripts/check-all.sh                  # the CI steps, stop at the first failure
sh scripts/check-all.sh --keep-going     # run every step, report all failures
sh scripts/check-all.sh --cross          # also the aarch64 cross-build steps
sh scripts/check-all.sh --lint           # also the repository's lint scripts
sh scripts/check-all.sh --help           # print the usage comment of the script
```

Flags can be combined. It changes into the repository root first, so it can be started from any directory.

## Flags

The script parses exactly these arguments (on the tip this README was written from):

| Flag | Effect |
| --- | --- |
| `--keep-going` | Run every step even after a failure. Without it the script prints the summary and exits at the first failing step. |
| `--cross` | Also run the three aarch64 cross-build steps of `ci.yml`, through `scripts/cross-aarch64.sh` (see [`cross-aarch64-README.md`](cross-aarch64-README.md)): the `composition-drm` library, its `probe` example, and the `drm-harness` binary. That script adds the `aarch64-unknown-linux-musl` rustup target on first use, or falls back to a private rustup cache. |
| `--lint` | After the CI steps, also run the repository's lint scripts and the script self-tests listed in the script. These are not CI steps; without `--lint` nothing changes. |
| `-h`, `--help` | Print the usage comment at the top of the script and exit 0. |

Any other argument prints `unknown argument: <arg>` to stderr and exits 2.

## What each mode runs

### Default (no flag)

Before anything runs, the script checks that the `thumbv7em-none-eabihf` rustup target is installed. If it is not, it prints `FAIL  target ... is not installed (nothing was run)` with the `rustup target add` command and exits 1.

Then, in order:

1. `cargo fmt --all --check`
2. `cargo clippy --locked --workspace --all-targets -- -D warnings`
3. `cargo test --locked --workspace`
4. build `composition-core` for `thumbv7em-none-eabihf`
5. build `composition-ffi-staticlib` for `thumbv7em-none-eabihf` without default features, with `panic-handler`, in release
6. the script self-tests and the provenance lint that run on every invocation: the `scripts/*.test.sh` tests named in the first loop of the script, and `scripts/check-fixture-provenance.sh`. A script that is not on the tree is skipped with a `SKIP` line.

Steps 1 to 5 are the steps of `ci.yml`. Step 6 is not in CI (see below). A few tests and lints are deliberately left out of step 6; the comment in the script says why for each.

### `--cross`

Adds the aarch64 cross-build steps after step 5. They compile and link only; nothing is run. The first run needs network access if the private-rustup fallback of `cross-aarch64.sh` is taken.

### `--lint`

Runs after everything else, as additional steps:

- the lint scripts listed in the script (documentation links, Zephyr app layout, the FFI header rule, evidence tags, board secrets, commit subjects, the feasibility doc), each as `lint <name>`;
- the script host tests listed in the script (`scripts/*.test.sh`), run with `sh`;
- the script host tests written for bash, run with `bash`; a test that needs `python3` or `cargo` is skipped with a message when the tool is missing.

Three of the lints take a base ref and are given `origin/main`; they are skipped with a message when `origin/main` does not resolve in the clone. A lint or test script that does not exist on the tree is skipped with a `SKIP` line, not failed. The exact lists are in the script; read them there, since they change as scripts are added.

## Output

Each step prints `==> name: command`, then `PASS  name` or `FAIL  name`. The run ends with `summary: N passed, M failed`, followed by the names of the failed steps. `SKIP` lines are not counted as passed or failed.

## Relation to the CI gate

[`docs/ci.md`](../docs/ci.md) describes the CI that gates every pull request to `main`. The first five steps here (plus `--cross`) are the CI steps; the script header says its step list must follow `ci.yml` and be changed in the same commit as it. A pass here is not the CI result: CI runs on a clean runner with the latest stable Rust, and its other job (`arm-backend`) is not run by this script. The extra self-tests and `--lint` scripts are local conveniences and do not gate a merge. Like CI, a pass says nothing about hardware.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Every step that ran passed (skipped items do not count as failures), or `--help` was given. |
| 1 | A step failed, or the `thumbv7em-none-eabihf` target is not installed. |
| 2 | Unknown argument. |
