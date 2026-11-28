# i.MX95 M7 board runbook (observe and remoteproc start/stop)

Written 2026-11-28.

**This runbook is not a hardware result.** No step in it has been run on a
board by the item that wrote it. The two scripts were only exercised on the
host against fake trees (`scripts/imx95-observe.test.sh`,
`scripts/imx95-m7-rproc.test.sh`). Nothing here may be quoted as an observed
board fact until a board item has run it and recorded the output in a dated
file as described in "Where the outputs go".

Evidence tags used below: *read from source* means read from the two scripts
in this repository; *inferred* means derived from them without running them.
There are no *observed* statements in this file.

Board rules come from [imx95-board.md](imx95-board.md); this file does not
replace them. The summary that matters here: write only under `/tmp`, install
no packages, no `fw_setenv`/`saveenv`, never write the eMMC boot partitions,
never stop Weston, never commit on the display, and put no addresses, host
names, users or serial paths in files or commits.

## What the two scripts are

| Script | Role | Writes on the board |
|---|---|---|
| `scripts/imx95-observe.sh` | read-only snapshot: remoteproc, device tree, dmesg, clk and genpd debugfs | nothing (read from source) |
| `scripts/imx95-m7-rproc.sh` | `start IMAGE` / `stop` of the M7 through remoteproc, with restore | the remoteproc `firmware` and `state` files, the `firmware_class` path parameter, and files under `/tmp/imx95-m7-rproc` (read from source) |

Both are POSIX `sh`. Context for the M7 load path is in
[imx95-m7-rproc-load.md](imx95-m7-rproc-load.md); the feasibility verdict is in
[imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md). Run the rproc
script only if those documents allow starting the M7 without touching the
rootfs, U-Boot environment or boot partitions.

## 0. Preconditions

- The board item holds the board through its lease. Take the login details
  from the lease; this runbook and your files never contain them. Below, `the
  lease` stands for that ssh access.
- No other item holds the board for an exclusive capability.
- The M7 ELF was built on the host (see [board-tests.md](board-tests.md)) and
  is a regular file on the host.
- The M7 remoteproc must be `offline`. The script refuses otherwise and
  changes nothing (exit 4).

## 1. Copy scripts and image to /tmp

Use the lease's ssh/scp access, one file at a time, destination `/tmp` only:

- `scripts/imx95-observe.sh` to `/tmp/imx95-observe.sh`
- `scripts/imx95-m7-rproc.sh` to `/tmp/imx95-m7-rproc.sh`
- the M7 ELF to `/tmp/<name>.elf`

The rproc script accepts the image and its state directory only under `/tmp`
(exit 2 otherwise), so do not choose another directory. The observe script
needs no copy: `ssh <lease> sh -s < scripts/imx95-observe.sh` also works and
leaves nothing on the board. Copy it only if you want it for the "after" run
without a second host-side pipe.

## 2. Snapshot before

Run `imx95-observe.sh` on the board and save its stdout **on the host**.

It prints these section headers (read from source): `### date`,
`### uname -r`, `### remoteproc`, `### device tree`,
`### dmesg (SM/SCMI/rproc/DPU)`, and two `### debugfs` sections (clk summary
and genpd summary). A section whose source cannot be read prints
`not readable`; with no remoteproc devices the `### remoteproc` section prints
`none`. The script redacts address-like values, the hostname and the serial
number before printing. Still run the secrets check (below) before committing
anything derived from it.

Check in the "before" output that a remoteproc entry for the M7 exists, its
`state:` is `offline`, and note its `firmware:` value.

## 3. Start

Run `sh /tmp/imx95-m7-rproc.sh start /tmp/<name>.elf` over the lease.

Lines printed on success (read from source):

- `recorded: rproc=... state=offline firmware=... firmware_class.path=...`
  the original values, also saved to the state file under
  `/tmp/imx95-m7-rproc`;
- `start: running firmware=<name>`;
- a final `state=running firmware=<name> firmware_class.path=...` line.

Failures print `FAIL <check>: ...` on stderr. Exit codes: 2 usage or path
outside `/tmp`, 3 no M7 remoteproc found, 4 core not offline, 5 a previous
start was never stopped (run `stop` first). If the core does not reach
`running`, the script calls `stop` itself to roll back and exits 1.

While the core runs, capture its log by the route the image supports (UART or
rpmsg; see [imx95-m7-console.md](imx95-m7-console.md) and
[imx95-m7-rpmsg-log.md](imx95-m7-rpmsg-log.md)). Take `dmesg` lines for rproc
from the "after" snapshot. Keep the run short.

## 4. Stop and restore

Run `sh /tmp/imx95-m7-rproc.sh stop`. It stops the core (if `start` did),
restores the firmware name and the `firmware_class` path from the state file,
checks the display service and prints one line per check (read from source):

```
OK core-offline
OK firmware-restored
OK firmware-class-path-restored
OK weston-active
```

Each is `FAIL <check>` instead on failure, and the exit status is 1. Only when
all four are `OK` does the script delete its state file and work directory
under `/tmp`; on any `FAIL` they are kept so `stop` can be repeated.

## 5. Snapshot after, and compare

Run `imx95-observe.sh` again and save it next to the first. On the host, diff
the two. Expected (inferred, not yet observed): the `### remoteproc` entry is
`offline` with the original `firmware:` value again, and the debugfs and
device-tree sections are unchanged except for counters. The `### dmesg`
section will show the rproc start and stop lines of the run; that is the
evidence of the run, not a regression.

## Which lines prove c8

The plan's check c8 asks for four things after an M7 run. Use these lines,
all from the `stop` run unless noted:

| c8 claim | Line to quote | Strength |
|---|---|---|
| M7 stopped | `OK core-offline` from `stop`, and `state: offline` in the after-snapshot `### remoteproc` | script verdict plus independent snapshot |
| firmware path restored | `OK firmware-class-path-restored` and `OK firmware-restored`, and the after-snapshot `firmware:` equal to the before-snapshot one | script verdict plus snapshot |
| Weston active | `OK weston-active` | script verdict only (it asks `systemctl is-active weston`); additionally run the same query by hand once and quote its output |
| nothing written outside /tmp | no script emits a verdict line for this | see below |

The last row has **no verdict line**. What exists is a design property (read
from source): the scripts accept the image and state directory only under
`/tmp`, and otherwise write only the three sysfs/module parameters that
`stop` restores. The board item must say that in its report as the basis, and
must not claim a measurement it did not make. If a stronger statement is
needed, list `/tmp` before and after by hand and report that listing.

`imx95-observe.sh` has no pass/fail verdict lines at all, only the section
headers above; do not describe its output as passing.

A `FAIL` in any of the four `stop` lines means c8 is not met; report it
as is.

## If the board stops answering

There is no remote power control for this board. If `ssh` hangs, drops, or the
`stop` command cannot be run or its output is lost:

1. Do not retry in a loop, do not power-cycle, do not attempt other routes
   into the board.
2. Report exactly what ran and when, the last output received, and that the
   state of the M7, the firmware name, the `firmware_class` path and Weston
   is unknown.
3. Stop the item there. Recovery by a person is the only way back, and the
   report should say so.

A single retry of `stop` after a short pause is fine if the connection comes
back; its `OK` lines are then the proof.

## Where the outputs go

Raw outputs are committed only after the checks below and with a date in the
file name or heading (the format is YYYY-MM-DD, for example 2026-11-28).

| Output | Destination | Tag |
|---|---|---|
| before and after snapshots | a dated file under `docs/imx95-zephyr/` (for example `observed-<date>.txt`, as the existing observation items do), not into [imx95-board.md](imx95-board.md) | observed, with date |
| `start` and `stop` output, dmesg lines of the run, the M7 log | the same dated file, as a separate section | observed, with date |
| conclusions (what the M7 could or could not do) | [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md) with a date and a pointer to the dated file | observed or inferred, as it is |
| anything about display ownership or latch timing | [imx95-linux-zephyr-compare.md](imx95-linux-zephyr-compare.md) or [m2-linux-zephyr-comparison.md](m2-linux-zephyr-comparison.md) only if a comparison was actually run | observed, with date |
| new open questions | [open-issues.md](open-issues.md) | inferred |

Before committing, run `scripts/check-no-board-secrets.sh` and
`scripts/check-doc-links.sh`; both must report nothing. Item owners of the
files above (running items) must be respected: add to a file only if your
item owns it.
