# check-m2-acceptance.sh

Runs the M2 / WP5 acceptance checks c1-c8 that can be decided mechanically
and prints one line per check. Bash, plus `git`, `grep` and `awk`; `cargo`
only with `--cargo`. It changes nothing in the tree. It is a host check: it
does not touch a board and says nothing about hardware results.

    scripts/check-m2-acceptance.sh [--cargo] [BASE_REF]
    scripts/check-m2-acceptance.sh --cargo origin/main

## Arguments

| argument | meaning |
|---|---|
| `--cargo` | also run c6 (fmt, clippy, tests, the two thumbv7em builds); without it c6 is SKIP |
| `BASE_REF` | git ref the diff checks (c2, c7) compare against; default `origin/main` |
| `M2_ACCEPT_ROOT` (environment) | tree to check instead of the repository the script lives in; the self-test uses it to point the script at throwaway repos |

Any other argument starting with `-` prints a usage line and exits 2.

If `BASE_REF` does not resolve, or resolves to the same commit as `HEAD`,
there is no diff to look at: c2 still checks that the pointer exists and
reports SKIP for the diff part, and c7 is SKIP. Pass a ref that is behind
your work (for example the commit before your branch) to get both.

## Output and exit codes

One line per check: `PASS cN`, `FAIL cN: reason` or `SKIP cN: reason`.

| exit | meaning |
|---|---|
| 0 | no check failed (SKIP is not a failure) |
| 1 | at least one `FAIL` |
| 2 | bad option, or the root directory cannot be entered |

A SKIP is not a pass. Read the SKIP lines before treating a clean exit as
acceptance.

## The checks

| check | what it requires |
|---|---|
| c1 | `docs/imx95-zephyr-feasibility.md` exists, contains the tags `observed`, `read from source` and `inferred`, and at least one `YYYY-MM-DD` date. If `scripts/check-feasibility-doc.sh` exists it must also pass (its first output lines are shown on failure). |
| c2 | `docs/platform-capabilities.md` mentions `imx95-zephyr-feasibility.md`. With a usable base ref, every changed hunk of that file against `BASE_REF` must lie inside the `i.MX95, Zephyr` section (heading to the next heading of the same or higher level). Changes by other plans elsewhere in the file fail this check. |
| c3 | `tests/board/imx95_m7` has `CMakeLists.txt`, `prj.conf` and a non-empty `src/`. It checks that the app exists, not that it builds or runs. |
| c4 | the feasibility doc mentions `remoteproc`. This is the only mechanical part of the remoteproc outcome: it does not tell whether the M7 was loaded, only that the doc discusses it. |
| c5 | `README.md` and `CONTRIBUTING.md` exist; CONTRIBUTING contains `forbid(unsafe_code)` and `composition_engine.h`; README contains `SoW.md` and `imx95-zephyr-feasibility.md`. |
| c6 | only with `--cargo`: `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, then the `thumbv7em-none-eabihf` builds of `composition-core` and of `composition-ffi-staticlib` (`--no-default-features --features panic-handler --release`). The failing steps are listed. The cargo output goes to stderr. |
| c7 | the diff against `BASE_REF` of `README.md`, `CONTRIBUTING.md`, `docs/imx95-zephyr-feasibility.md`, `docs/imx95-zephyr` and `tests/board` must have no line matching (case-insensitive) a dotted four-number address, a tty device path, the word for a login secret, or a user-at-host string. The count and the first hit are shown. The grep reads the whole diff, so context and removed lines also match; it is deliberately blunt. |
| c8 | always SKIP: "board-side, see the board item reports". |

## What it cannot decide

- **c8** is the board result: it needs the dated reports of the board items
  (the remoteproc load and its log, the observations), which a script on the
  host cannot judge. Read those reports and their `observed` tags yourself.
- c1 checks that the tags and a date are present, not that each claim carries
  the right tag or that the date belongs to the claim.
- c3 and c4 are existence and keyword checks. They do not show that the app
  builds (see `scripts/check-zephyr-apps.sh` and
  `scripts/build-zephyr-apps.sh`) or that the M7 ran.
- c7 is a pattern grep over a fixed list of paths, not a full secret scan
  (for that see `scripts/check-no-board-secrets.sh`).
- c2 and c7 depend on the base ref you pass; a wrong base gives a wrong answer.

## Self-test

    bash scripts/check-m2-acceptance.test.sh

It builds throwaway git repos under `$TMPDIR` and runs the script against
them with `M2_ACCEPT_ROOT`: PASS and FAIL for c1, c2, c5 and c7, and the SKIP
for a missing base ref. It does not cover c3, c4 or c6, and uses no board.
