# am62p-state-snapshot

Read-only snapshot and compare of the Verdin AM62P state, for the M3 restore
check (owner rule: after a session that stops or unbinds things, restore them
and reboot into the Torizon boot chain). Run on the board; POSIX sh.

    sh am62p-state-snapshot.sh snapshot FILE
    sh am62p-state-snapshot.sh compare BEFORE AFTER   # prints SAME, or a diff and exits 1

Records, sorted: remoteproc instances (name, state, firmware); devices bound
to display and remoteproc drivers under `/sys/bus/platform/drivers`
(unit addresses dropped); the active state of units matching docker, weston,
portainer, display, tidss, wayland or cog; running container names (or
`docker-unreachable`); DRM connector status; `ostree=` from `/proc/cmdline`
and ID/VERSION_ID from `/etc/os-release`. No serials, addresses or host names.

Options: `--root DIR` reads `/sys`, `/proc`, `/etc` below DIR;
`STATE_SYSTEMCTL` and `STATE_DOCKER` replace the `systemctl` and `docker`
commands (tests). Self-test: `sh scripts/am62p-state-snapshot.test.sh`.

The board item takes a snapshot before the session (before stopping or
unbinding anything) and compares it with a snapshot taken after the Torizon
reboot. A difference in `ostree=` alone may be a legitimate slot change;
judge it, do not hide it.
