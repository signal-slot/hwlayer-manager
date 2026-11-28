# check-no-farm-ids

Fails if a tracked file contains a farm identifier: a board address, host
name, serial/UART path, ssh target, MAC address or credential. It backs the
rule that no board addresses or credentials appear in files or commits.

## Usage

    scripts/check-no-farm-ids.sh

Run it from anywhere inside the repository; it changes to the top level
itself. It takes no options and no arguments, and reads no environment
variables. It scans tracked files only (`git grep`), so an untracked file is
not checked until it is added.

## What it rejects

Each category is reported under its own heading, as `file:line:text`.

| Category | Rejected |
| --- | --- |
| IPv4 address | four dot-separated numbers, not part of a longer dotted number |
| `.ts.net` name | any host name ending in the tailnet suffix (case-insensitive) |
| serial/UART path | a USB or ACM serial device node, or a path below the serial by-id or by-path directories |
| user@host ssh target | an `ssh` command line naming a login at a host, or a bare login at a dotted host name |
| MAC address | six colon-separated hex pairs |
| credential | a password, passwd or token name followed by `=` or `:` and a literal value (case-insensitive) |

Not rejected, so no false alarms: loopback and unspecified addresses, the
RFC 5737 documentation ranges, board kernel/BSP version numbers that look like
IPs, documentation and noreply mail addresses, the commit-trailer address, a
systemd unit instance name, the public upstream mailing-list address, the
public git remote login, Rust parameters typed as references, and placeholder
credential values (none, null, true, false, redacted, changeme and similar).
Allowed text is blanked out of the line and the category is tested again on
the rest, so an allowed string never hides another identifier on the same
line.

Skipped lines and files:

- a line carrying the marker `secrets-ok` (same marker as
  `check-no-board-secrets.sh`; use sparingly, on deliberate fixtures);
- fixtures: a table in the script lists exact (file, text) pairs, such as the
  sample values in other scripts' tests; the same text in any other file is
  still reported;
- never scanned: the script itself, `Cargo.lock` and `scripts/testdata/`.

To allow something new, add a narrow entry with a reason to the allowlist or
fixture table in the script; do not widen an existing one.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | clean; prints `check-no-farm-ids: clean` |
| 1 | at least one match remains; every category with hits is printed |

## Test

    bash scripts/check-no-farm-ids.test.sh

Each case builds a throwaway git repository under `$TMPDIR` with a copy of the
script and one file, so the real repository is untouched. It covers every
category, the allowlist (including not hiding a second identifier), the
marker, fixtures per file and per string, the lockfile and `scripts/testdata/`
exclusions, tracked-only scanning and running from a subdirectory. Fake values
are assembled from pieces so the test file itself stays clean. It exits 0 when
every case prints `ok`.
