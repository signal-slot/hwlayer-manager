# imx95-session.sh

Host-side helper for short sessions on the Verdin iMX95 board (Toradex
Reference Multimedia Image, Weston owns the display). It deploys the probe
and `drm-harness` to `/tmp` on the board, takes the display from Weston and
gives it back, runs commands and saves their output, and removes what it
copied.

The ssh destination comes only from the environment, taken from the farm
lease. It is never written to a file by the script, and the script holds no
address, host name or user:

```sh
export IMX95_SSH='<destination from the lease>'     # required (not for --dry-run)
export IMX95_SSH_OPTS='-i <key> ...'                # optional extra ssh/scp options
export IMX95_DISPLAY_UNIT=weston.service            # optional; default: autodetect
```

```
scripts/imx95-session.sh [--dry-run] [--out DIR] <subcommand>
```

`--dry-run` prints the planned commands (`DRY ...`, the remote script is shown
quoted) and runs nothing, builds nothing, needs no `IMX95_SSH`.
`--out DIR` (default `./board-logs`) is where `run` saves its files.

## Subcommands

| Subcommand | What it does |
|---|---|
| `deploy` | `scripts/cross-aarch64.sh -p composition-drm --example probe` and `-p composition-drm-harness --bin drm-harness`, then `scp` to `/tmp/probe` and `/tmp/drm-harness`, `chmod 755`. The paths are recorded in `/tmp/imx95-session.files` on the board. Both builds run first; a build failure touches nothing on the board. |
| `display-owner status` | Prints the Weston unit, whether it is active, and whether a stop of this script is pending. |
| `display-owner stop` | Finds the unit (`IMX95_DISPLAY_UNIT`, else `weston.service`, else the first `weston*.service` systemd knows). Records `unit=` and `was=<is-active output>` in `/tmp/imx95-session.display`, then stops it. If that record already exists (an earlier stop was never undone) it is kept, so "was active" is not overwritten by the now-inactive state. Lists holders of `/dev/dri/card*` if `fuser` exists. |
| `display-owner start` | Reads the record. Starts the unit **only if it was active** before the stop, waits up to 30 s until `systemctl is-active` says active, prints the result, removes the record. Exits 1 if the unit did not come back. Without a record it does nothing. |
| `run <name> -- <cmd...>` | Runs the command on the board (words are single-quoted, so spaces and quotes survive). Saves `<out>/<name>.header` (UTC date, `uname -r`, `ID`, `VERSION_ID`, `PRETTY_NAME`, `BUILD_ID` from `/etc/os-release`, the command, then exit status and end time), `<name>.stdout`, `<name>.stderr`. Returns the command's exit status. `<name>` is `[A-Za-z0-9._-]+`. |
| `cleanup` | Removes only the files listed in `/tmp/imx95-session.files` (directly under `/tmp`), then the list. If a display stop is still pending it says so and leaves the record. Output files you made yourself on the board are not removed. |

## Using it across invocations

A session is normally several commands, so `display-owner stop` leaves Weston
stopped when it exits, and the state lives on the board, not in the shell:

```sh
export IMX95_SSH=...                       # from the lease
S=scripts/imx95-session.sh
$S deploy
$S display-owner stop
$S --out m1/2026-11-28-a run two-planes -- /tmp/drm-harness --connector LVDS --profile imx95 two-planes
$S --out m1/2026-11-28-a run combos     -- /tmp/drm-harness --connector LVDS --profile imx95 combos
$S display-owner start                     # always, even after a failure
$S cleanup
```

Always finish with `display-owner start`; check that it printed
`... is active again` (the test plan requires Weston running at the end of a
session). If the shell or the ssh link dies in between, the record on the
board survives: just run `display-owner start` again.

### Restore on exit

For a single invocation that must not leave the display stopped, use `--then`:

```sh
$S display-owner stop --then ./my-session-script.sh
```

It stops Weston, runs the local command, and an `EXIT` trap runs
`display-owner start` on any exit of the script (command failure, Ctrl-C,
`set -e`), whatever the command returned. A failed restore is reported as a
warning; the board's record stays, so a later `display-owner start` retries.
Plain `display-owner stop` (no `--then`) arms no trap, since that would undo
the stop immediately.

## Safety

- Writes only under `/tmp` on the board: `probe`, `drm-harness`,
  `imx95-session.files`, `imx95-session.display`.
- Installs nothing, never touches the U-Boot environment, never touches
  `mmcblk0boot*`. `run` additionally refuses a command line that mentions
  `fw_setenv`, `saveenv`, `mmcblk0boot`, `opkg`, `dnf`, `apt`/`apt-get` or
  `pip install`. This is a coarse guard, not a sandbox; the rules in
  `docs/imx95-board.md` still apply to what you type.
- `/tmp` on the board is memory; run `cleanup` at the end.

## Verification state

Written without a board. Checks to run on the host: `bash -n
scripts/imx95-session.sh`, `shellcheck scripts/imx95-session.sh` if installed,
and `--dry-run` of each subcommand. The remote scripts have not been run on
a board.
