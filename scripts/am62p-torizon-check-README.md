# am62p-torizon-check

`am62p-torizon-check.sh` checks over ssh, read-only, that a Verdin AM62P is
running Torizon. Run it at the end of a board session to confirm the board is
back on its Torizon boot chain, and put the output in
`docs/m3-am62p-report.md`.

```sh
AM62P_SSH_DEST=<destination from the lease> sh scripts/am62p-torizon-check.sh
sh scripts/am62p-torizon-check.sh <destination>
```

The destination comes from the lease (argument or `AM62P_SSH_DEST`); it is
never stored in a file and never printed. `SSH` overrides the ssh command
(default `ssh`; the script adds `-o BatchMode=yes -o ConnectTimeout=10`).

One ssh connection runs only read-only commands: `date -u`, the ID, NAME,
VERSION_ID, PRETTY_NAME and VARIANT fields of `/etc/os-release`, `uname -r`,
`uptime`, `ostree admin status` (if present), `systemctl is-system-running`
and the name, state and firmware of each `/sys/class/remoteproc/remoteproc*`.
It writes nothing on the board and needs no root.

Output is `KEY value` lines (`DATE`, `OS_*`, `KERNEL`, `UPTIME`, `OSTREE`,
`SYSTEMD`, `REMOTEPROC`), then exactly one verdict line:

- `TORIZON UP`: os-release names Torizon and systemd reports `running` or
  `degraded`. Exit status 0.
- `TORIZON NOT-UP <reason>`: anything else (ssh failure, other OS, system
  still starting, incomplete answer). Exit status 1; 2 for a usage error.

The remote commands print no host name; IPv4 addresses and lines mentioning
a serial number are removed from the output.

## Test

```sh
sh scripts/am62p-torizon-check.test.sh
```

The test uses a fake ssh and covers the UP and degraded cases, a non-Torizon
os-release, a failing ssh, a missing destination, scrubbing, and that the
destination never appears in the output. `scripts/check-all.sh` has an
explicit list of script tests and does not pick this one up; run it by hand.
