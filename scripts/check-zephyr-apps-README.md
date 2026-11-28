# check-zephyr-apps.sh

Lints the layout of the Zephyr board-test apps. POSIX `sh`, no dependencies.
It reads files only; it does not build anything (building is
`scripts/build-zephyr-apps.sh`). Layout and how to write an app:
[tests/board/README.md](../tests/board/README.md).

    sh scripts/check-zephyr-apps.sh [repo-root]

`repo-root` defaults to the parent of `scripts/`. Every directory directly
under `<root>/tests/board/` and `<root>/tests/zephyr/` is one app; a missing
parent directory is skipped.

## What it checks per app

| check | failure text |
|---|---|
| `CMakeLists.txt`, `prj.conf` and `README.md` exist | `missing <file>` |
| `src/` exists | `missing src/` |
| `src/` holds at least one `*.c`, `*.cpp`, `*.cc`, `*.S` or `*.rs` file | `src/ has no source file` |
| `README.md` contains the text `west build` | `README.md has no west build command` |
| no IPv4 address anywhere in the app (version-like numbers such as `1.2.3` do not match) | `contains an IPv4 address` |
| no `/dev/tty...` path; `/dev/ttyRPMSG*` (the Linux rpmsg node) is allowed | `contains /dev/tty` |
| no `password`, case-insensitive | `contains password` |

The last three scan every text file under the app and keep board addresses,
serial paths and credentials out of the repository (see
[CONTRIBUTING.md](../CONTRIBUTING.md)).

## Output and exit codes

One line per app: `ok <app dir>`, or `fail <app dir>: <reason>; <reason>`
listing every failed check. All apps are checked even after a failure.
No apps at all prints nothing.

| code | meaning |
|---|---|
| 0 | every app passed (or there are none) |
| 1 | at least one app failed |

## Self-test

`scripts/check-zephyr-apps.test.sh` builds fixture apps in a temporary
directory and checks one case per rule above, plus empty trees, apps in both
parent directories, a mixed good/bad tree, the `/dev/ttyRPMSG` exception and
the version-number case. It prints `pass <name>` or `FAIL <name>` and exits
non-zero if any case fails. It runs on the host only.
