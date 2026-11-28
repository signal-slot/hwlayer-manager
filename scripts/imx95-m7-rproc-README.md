# imx95-m7-rproc.sh

Board-side helper that loads an ELF onto the i.MX95 Cortex-M7 through Linux
remoteproc, lets it run, stops it, and puts back every setting it changed.
POSIX `sh`, no packages; it needs only the shell, `cat`, `cp`, `mktemp`,
`sed`, `dmesg` and `systemctl`. It runs on the board as root. How a board item
uses it is in [`docs/imx95-m7-runbook.md`](../docs/imx95-m7-runbook.md). This
README is not a board result: the host test uses a fake sysfs tree.

## Board rules it follows

From [`docs/imx95-board.md`](../docs/imx95-board.md):

- Write only under `/tmp`. The ELF, the work directory and the state
  directory must be under `/tmp` (no `..`). The only other files it writes are
  the remoteproc `firmware` and `state` files and the `firmware_class` `path`
  parameter in sysfs.
- No `fw_setenv` or `saveenv`: it never touches the U-Boot environment.
- Never `mmcblk0boot0` or `mmcblk0boot1`: it never opens a block device.
- Weston must be active afterwards. The `stop` subcommand checks this and
  fails if it is not. The script never stops or starts Weston itself.

## Two ways to run it

One call, selecting the core by index or by name:

    scripts/imx95-m7-rproc.sh [--force-stop] [--timeout SECS] [--run-secs SECS] \
        [--log FILE] ELF REMOTEPROC

`ELF` is a file under `/tmp`. `REMOTEPROC` is an index (`3`) or the contents of
the remoteproc `name` file. One call does everything: record, load, start,
wait for `running`, sleep `--run-secs`, print the `dmesg` lines added since the
start (`dmesg: ...`) and the captured log (`log: ...`), then stop and restore.

Two steps, for when the core must keep running between calls:

    scripts/imx95-m7-rproc.sh start /tmp/<image>.elf
    # ... read the console or rpmsg log, run the measurement ...
    scripts/imx95-m7-rproc.sh stop

`start` finds the M7 itself: the first remoteproc whose `name` matches a
pattern in `M7_NAME_GLOB`.

## Options (one-call form)

| option | meaning |
|---|---|
| `--force-stop` | if the core is not `offline`, record its state and stop it first. Without it the script refuses and changes nothing. |
| `--timeout SECS` | wait for `running` and `offline` (default 10) |
| `--run-secs SECS` | let the core run before collecting (default 3) |
| `--log FILE` | rpmsg tty or log file to `cat` in the background while the core runs; shown as `log:` lines |

## What it changes and how it restores

It first records three values: the remoteproc `state`, its `firmware` name and
`/sys/module/firmware_class/parameters/path`. Then it:

1. (`--force-stop` only) writes `stop` to `state` and waits for `offline`.
2. Points the `firmware_class` search path at the directory of the ELF
   (one-call form), or at a copy of the image in a state directory under
   `/tmp` (`start`).
3. Writes the image file name to `firmware`.
4. Writes `start` to `state` and waits for `running`.

Restore (an `EXIT` trap in the one-call form; the `stop` subcommand
otherwise): write `stop` if the core is not `offline` and wait for it; write
the original `firmware` name and path back and read them again to check. The
trap also runs on failure, timeout, Ctrl-C, `TERM` and `HUP`. A core that was
running before a `--force-stop` is left `offline`, and the script says so. In
the two-step form, a failed `start` rolls back by calling `stop`.

The two-step form keeps the recorded values in `<state dir>/state` (default
`/tmp/imx95-m7-rproc`) and refuses a second `start` until `stop` has removed
it. `stop` prints `OK` or `FAIL` for `core-offline`, `firmware-restored`,
`firmware-class-path-restored` and `weston-active`, and removes the state
directory only if all four are `OK`.

## Exit codes

One-call form: 0 ok and restored; 1 failure (restore still attempted); 2 usage
(bad option, ELF not under `/tmp`, ELF missing, unknown remoteproc); 3 refused,
core not `offline`; 4 restore failed.

`start` and `stop`: 0 ok; 1 failure (including any `FAIL` verdict from `stop`);
2 usage or path outside `/tmp`; 3 no M7 remoteproc found; 4 core not
`offline`; 5 a previous `start` was not stopped.

## Environment

Test hooks, not for board use: `ROOT` (one-call form: prefix for `/sys`),
`SYSFS_ROOT` (`start`/`stop`, default `/sys`), `RPROC_FW_ROOT` (allowed prefix,
default `/tmp`), `SYSTEMCTL`, `DMESG`, `RPROC_POLL` and `RPROC_POLLS_PER_SEC`
(poll interval and polls per second). Usable on the board: `M7_NAME_GLOB`
(space-separated name patterns, default `*m7* *M7* *imx-rproc*`),
`M7_STATE_DIR` (must stay under `/tmp`) and `M7_TIMEOUT` (default 10).

## Self-test

    sh scripts/imx95-m7-rproc.test.sh

No board and no root. It builds a fake sysfs tree in a temporary directory and
runs a background loop that turns writes to the fake `state` file into
`running`, `crashed` or `offline`. It checks:

- a normal run by index and by name, with the `dmesg` and log capture;
- a core that crashes on start, and a start that never completes;
- refusal when the core is running, and `--force-stop`;
- rejection of a path outside the allowed root, `..`, a missing ELF and an
  unknown remoteproc, leaving the tree unchanged;
- the `start`/`stop` form: a Weston-down failure, no M7 found, a core already
  running, a double `start`, `stop` without `start`, an image or state
  directory outside the allowed root, and a crash rolled back.

It prints `ok` or `FAIL` per check, then `all passed`, or the number of
failures and exit 1. It generates its fixtures; none are board data.

In notes about runs use placeholders such as `<board>`. Do not put real
addresses, host names, users or serial paths in files or commits
(`scripts/check-no-board-secrets.sh`).
