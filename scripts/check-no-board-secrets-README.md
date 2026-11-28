# check-no-board-secrets.sh

Fails when a branch adds board addresses, host names, serial or UART paths,
MAC addresses or credentials to a file or a commit message. It enforces the
project rule in `CONTRIBUTING.md` ("Board access and secrets"): such values
come from the environment at run time and stay out of the repository.
POSIX `sh`; needs only `git`, `grep`, `sed` and `mktemp`. It reads the
repository and changes nothing.

## Usage

    scripts/check-no-board-secrets.sh [BASE_REF]
    scripts/check-no-board-secrets.sh --c7 BASE_REF [PATH...]

| Form | Meaning |
|---|---|
| (no argument) | Base is the first of `origin/main`, `main`, `refs/remotes/origin/HEAD` that resolves to a commit; the choice is printed to stderr. If none resolves, a **full scan** of `HEAD` runs: every line of every tracked file counts as added and every commit of `git rev-list HEAD` is scanned. |
| `BASE_REF` | Use this ref as given. A ref that does not resolve is exit 2. Any second argument is a usage error (exit 2). |
| `--c7 BASE_REF [PATH...]` | Opt-in mode that reproduces acceptance check c7 (below). `BASE_REF` is required (missing is exit 2, unknown ref is exit 2). Default `PATH`s: `README.md`, `CONTRIBUTING.md`, `docs/imx95-zephyr-feasibility.md`, `docs/imx95-zephyr`, `tests/board`. |

These are all the options the script parses; there are no environment
variables except `TMPDIR` (location of the scratch directory, default the
current directory).

## What is scanned

- Normal mode: the **added** lines of `git diff BASE_REF...HEAD` (three-dot;
  two-dot against the empty tree in full-scan mode) and every line of the
  messages of `git log BASE_REF..HEAD`. Existing content is not scanned.
- `--c7` mode: the added lines of `git diff BASE_REF -- PATH...` only (no
  commit messages).

## What it matches

Normal mode reports each hit as `kind: file:line: text` (commit messages
as `commit:<short sha>:<line>`). Kinds:

| Kind | Matches |
|---|---|
| `ipv4` | a dotted quad of four numbers |
| `tty` | a `/dev/tty...` device path or a path under the `/dev/serial/by-id` directory |
| `mac` | six pairs of hex digits joined by `:` or by `-` (one separator throughout) |
| `user@host` | a user name, an `@`, then a host name that starts with a letter |
| `ssh-key` | a PEM `BEGIN ... PRIVATE KEY` header line |
| `secret` | `password`, `passwd`, `token`, `secret` or `api_key` (any case), then `=`, then a value |

`--c7` mode uses one case-insensitive pattern: a dotted quad, `/dev/tty`,
the word `password`, or an `@` followed by a name and a dot. It prints
`fail <file>: <line>` per hit, or `ok` when clean.

### What it allows

Each rule is narrow; the test checks that the nearest real secret is still
flagged.

- `ipv4`: lines containing the unspecified or loopback address; any
  dotted quad in `Cargo.lock` or under a `vendor/` directory (dependency
  versions look like addresses).
- `user@host`: the public git hosting clone user, `noreply` addresses and
  `Co-Authored-By` / `Signed-off-by` lines; GitHub Actions `uses:` step lines
  and `actions/NAME@vN`; display modes such as `1920x1080@60` or `WxH@Hz`;
  the literal placeholders `user@host`, `repo@commit`, `repo@pin`, `a@b.c` and
  the reserved example domains; `name@commit` pins (hex only), device-tree
  unit addresses, `@N`, and systemd instance names; exact public mailing-list
  and archive domains; kernel mailing-list archive URLs with an `@` in the
  message id. The pin, unit-address and mailing-list rules do **not** apply
  to a line that mentions `ssh`, `scp`, `sftp` or `rsync`, so an ssh target
  stays flagged.
- `tty`: the `/dev/ttyRPMSG` rpmsg node is not a UART.
- `secret`: values starting with `$`, `<` or `{` (placeholders).
- Any line containing the marker `secrets-ok` (use sparingly, for documented
  non-board examples only).
- Not scanned at all: the script, its test and `scripts/check-no-farm-ids.sh`
  (they spell out the patterns), and the directory `scripts/testdata/`
  (fake fixture data). A small `FIXTURES` table in the script allows named
  fake sample values in named test files of other scripts, per file and per
  text only.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | clean (`clean (<range>)` on stdout, or `ok` in `--c7` mode) |
| 1 | at least one hit; hits are printed to stderr (stdout in `--c7` mode) |
| 2 | usage error, unknown base ref, or scratch file could not be created |

## Host test

    scripts/check-no-board-secrets.test.sh

Builds throwaway git repositories under `TMPDIR` and never touches the real
repository. The fake secrets are assembled from pieces so the test file stays
clean. It covers every kind above, each allowed shape and its nearest flagged
lookalike, commit-message scanning, `file:line` reporting, unknown base ref,
the no-argument base choice, full scan, the `--c7` mode (hits, usage errors,
path limit), and `scripts/check-no-farm-ids.sh`. It prints `ok` or `FAIL`
per case and exits non-zero if any case failed.

## Relation to the rules

- **`CONTRIBUTING.md`**, "Board access and secrets": the rule this script
  checks, and the script is listed under the local checks there. No board
  address, host name, serial or UART path, serial number or credential goes
  into a file or a commit.
- **Acceptance rule c7**: "no address-like, tty, password or user@host strings
  in the diff since BASE_REF", as run by `scripts/check-m2-acceptance.sh`
  over the M2 documents and board apps. `--c7` reproduces that grep exactly,
  so a hit here is a hit there. It is stricter than the default mode (it has
  no allowlist), so a default-mode pass does not imply a c7 pass.
