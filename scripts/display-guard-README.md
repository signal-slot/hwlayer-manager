# display-guard

Stop a display manager or compositor only for a board run, then show it
active again. It serves the board rule that Weston (or another display client
stopped to take DRM master) is started again and confirmed active before a
task ends; see `docs/imx95-board.md` and `CONTRIBUTING.md`. Run it on the
board; POSIX sh, needs `systemctl`.

    sh display-guard.sh [--wait N] detect
    sh display-guard.sh [--wait N] stop STATEFILE
    sh display-guard.sh [--wait N] restore STATEFILE

## Options and commands

- `--wait N`, `--wait=N`: seconds `restore` waits for each unit to become
  active after starting it. Whole number, default 15. It must come before the
  command; a missing or non-numeric value is a usage error (exit 2).
- `detect`: print each active display unit, one per line. Candidates are a
  fixed list: display-manager, gdm, gdm3, lightdm, sddm, lxdm, xdm, greetd,
  weston, cage, cog, sway, plasma-kiosk and kiosk (all `.service`). A unit
  counts when `systemctl is-active` says `active`.
- `stop STATEFILE`: stop each active unit. STATEFILE is named by the caller
  (under `/tmp` on the board). Each unit is written to STATEFILE before it is
  stopped, so a half-failed stop is still restored. Prints
  `DISPLAY-GUARD stopped <unit>` or `DISPLAY-GUARD stop-failed <unit>` per
  unit, or `DISPLAY-GUARD none` (with an empty STATEFILE) when nothing was
  active.
- `restore STATEFILE`: `systemctl start` every recorded unit, then wait up to
  `--wait` seconds for each to be active. Prints
  `DISPLAY-GUARD restored <unit> active` or
  `DISPLAY-GUARD restore-failed <unit> <state>` per unit. An empty STATEFILE
  prints `DISPLAY-GUARD none`. A missing STATEFILE prints
  `DISPLAY-GUARD no-state-file`.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | `detect` finished; `stop` stopped everything or found nothing; `restore` found every unit active, or an empty STATEFILE |
| 1 | `stop`: a stop command failed; `restore`: a unit was not active after the wait |
| 2 | usage error (bad or missing `--wait`, missing STATEFILE argument, unknown command, STATEFILE not writable) or `restore` with a missing STATEFILE |

A missing state file is exit 2, not success: nothing can be shown to be active
again.

## What it does and never does

On the board it runs only `systemctl is-active`, `stop` and `start`, on units
that were already active. It never enables, disables, masks or edits units,
and never touches DRM, udev or seat configuration. It does not decide that
restoring is enough: the board task still confirms the display is active
before it ends, and treats exit 1 or 2 from `restore` as a failure to report.

## Test

`sh scripts/display-guard.test.sh` runs the script on the host against a mock
`systemctl` on `PATH`. It needs no systemd and no board.
