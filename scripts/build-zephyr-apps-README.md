# build-zephyr-apps.sh

West-builds every Zephyr app of the repository, one after the other, from a
Zephyr workspace. POSIX `sh`, no other dependency than `west`. It only builds:
it loads nothing onto a board and runs nothing, so a successful run is a
compile-and-link result, not a hardware result.

    scripts/build-zephyr-apps.sh --dry-run
    scripts/build-zephyr-apps.sh [--keep-going] [--build-root DIR] [ROOT]

## Which apps, for which boards

An app is a directory `tests/board/*` or `tests/zephyr/*` that holds both
`CMakeLists.txt` and `prj.conf`; other directories are skipped. Its board is
the first `-b <board>` of a `west build` line in the app's own `README.md`
(for example `imx95_evk/mimx9596/m7` or `mps2/an386`, including a board
qualifier such as `/ddr`). The script has no board list of its own: an app
whose README has no such line is a failure (`fail <name>: no 'west build -b
<board>' line in README.md`). So the boards follow the READMEs; the board of
each app is in `tests/board/README.md` and in the app's README. Apps that are
added later are picked up without a change to the script.

Each app is built with

    west build -p always -b <board> -d <build-root>/<name> <app>

run inside `ZEPHYR_WORKSPACE`. `<name>` is the path below `tests/` with `/`
replaced by `_` (`board_<app>`, `zephyr_<app>`). `-p always` makes every build
a pristine one.

## Inputs

| input | meaning |
|---|---|
| `ZEPHYR_WORKSPACE` | a west workspace with the pinned upstream Zephyr; must be a directory |
| `ZEPHYR_SDK_INSTALL_DIR` | the Zephyr SDK; must be set (west reads it) |
| `west` | on `PATH` |

To get a workspace and an SDK, use `scripts/prepare-zephyr.sh`; how they are
pinned and laid out is in [`docs/zephyr-workspace.md`](../docs/zephyr-workspace.md).
Give the values as placeholders of your own, for example
`ZEPHYR_WORKSPACE=<workspace> ZEPHYR_SDK_INSTALL_DIR=<sdk>`. `--dry-run` needs
none of them.

## Options

| option | effect |
|---|---|
| `--dry-run` | print the `west build` command of each app, run nothing, need no environment |
| `--build-root DIR` | root of the per-app build directories |
| `--keep-going` | build the remaining apps after a failure (default: stop at the first) |
| `ROOT` | repository root (default: the one holding the script) |
| `-h`, `--help` | print the header of the script |

## Where output goes

Build directories are `<build-root>/<name>`. Default `<build-root>`:
`$TMPDIR/zephyr-apps` when `TMPDIR` is set to something outside `/tmp` and
`/var/tmp`, otherwise `<ROOT>/target/zephyr-apps`. The default is never under
`/tmp`. Progress goes to stdout (`build <name> (<board>)`, and at the end
`built N, failed M`); failures go to stderr.

## Exit codes

| code | meaning |
|---|---|
| 0 | every app built (with `--dry-run`: every app has a board line) |
| 1 | a build failed, or an app has no board line |
| 2 | missing environment (`ZEPHYR_WORKSPACE`, `ZEPHYR_SDK_INSTALL_DIR`, `west`, or workspace not a directory), unknown option, bad `--build-root` or `ROOT` |

## CI

CI does not build board programs: `docs/ci.md` describes the checks, and none
of them runs `west`. The board Zephyr apps are built only when someone runs
this script (or `west` by hand) with a workspace. The lint of the app layout
is a separate script, `scripts/check-zephyr-apps.sh`.

## Test

`scripts/build-zephyr-apps.test.sh` runs the script against fake apps and a
fake `west`; it needs no Zephyr and builds nothing real. It checks the dry-run
output and board parsing, the exit-2 cases, the build-in-workspace call,
stop versus `--keep-going`, a missing board line, and that the default build
root is not `/tmp`.
