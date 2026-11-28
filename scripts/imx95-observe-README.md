# imx95-observe.sh

Read-only snapshot of an i.MX95 board (the Verdin iMX95 of the M2 work). POSIX
`sh`, runs ON the board and prints one text report to stdout. It changes
nothing: no file is written, no service is touched, no device node is opened.
Its exit status is that of the final `sed` filter, not a verdict on the board.

    ssh <board> sh -s < scripts/imx95-observe.sh > snapshot.txt

`<board>` is a placeholder for whatever ssh login the board's lease gives you;
do not put a real address, host name, user or serial path in a file or commit.
Dated snapshots go under `docs/imx95-zephyr/` (see
[docs/imx95-zephyr/README.md](../docs/imx95-zephyr/README.md)), named
`observed-<UTC date>.txt`; the date is the board's `date -u`, which the report
prints itself.

## What the report contains

The first line is `# imx95-observe: read-only snapshot`; sections start with
`### `.

| section | source | what is kept |
|---|---|---|
| `date`, `uname -r` | `date -u`, `uname -r` | UTC time and kernel release |
| `remoteproc` | `/sys/class/remoteproc/*` | per device: `name`, `state`, `firmware`; `none` if there is no device |
| `device tree` | `/proc/device-tree` | nodes whose path or `compatible` matches `display`, `dpu`, `ldb`, `lvds`, `dsi`, `remoteproc`, `imx_rproc`, `reserved-memory` or `scmi`; each with `compatible`, `status` and `reg` (hex, 32-bit cells) |
| `dmesg (SM/SCMI/rproc/DPU)` | `dmesg` | lines matching `sm` (as a word), `scmi`, `rproc` or `dpu` |
| `debugfs clk/clk_summary` | `/sys/kernel/debug/clk` | lines matching `disp`, `dpu`, `ldb`, `lvds`, `dsi` or `pix` |
| `debugfs pm_genpd/pm_genpd_summary` | `/sys/kernel/debug/pm_genpd` | same filter |

Anything that cannot be read (no permission for `dmesg` or debugfs, debugfs not
mounted, no device tree) is reported as `not readable`; the script carries on.
Run it as a user that can read these if you need them.

## Redaction

Before anything is printed, IPv4 addresses, MAC addresses, `serial number: …`
style values, the host name and the board serial number
(`/proc/device-tree/serial-number`) are replaced by `<redacted>`. This is a
pattern filter, not a guarantee: read the snapshot before committing it.

## Summary table

Pipe a snapshot through the summariser for a short Markdown ownership table:

    python3 -I scripts/imx95-observe-summary.py snapshot.txt
    ssh <board> sh -s < scripts/imx95-observe.sh | python3 -I scripts/imx95-observe-summary.py

It exits 1 when the input does not start with the `# imx95-observe` line. What
the snapshot shows is **observed** on the date it prints; the summary adds no
new evidence.

## Test

    bash scripts/imx95-observe.test.sh

The test runs the script on a fake `/proc` and `/sys` tree under `$TMPDIR`; the
script reads its roots from `PROC_ROOT` (default `/proc`), `SYSFS_ROOT`
(default `/sys`) and `DMESG_CMD` (default `dmesg`) for that purpose. It checks
that the sections above appear, that unrelated nodes and lines are left out,
that the fake address, MAC, host name and serial number are redacted, that
unreadable sources are reported, and that the script writes no file. The fake
values are synthetic, not board data. The test does not touch a board.

## Board rules

Keep the session short; the board may be in use by another run. Never stop
Weston or change a setting. See [docs/imx95-board.md](../docs/imx95-board.md)
and [docs/imx95-m7-runbook.md](../docs/imx95-m7-runbook.md) for the rules and
for how a board item combines this script with the M7 helper scripts.
