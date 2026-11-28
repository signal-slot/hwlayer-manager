# imx95-observe-summary.py

Summarises a dated `imx95-observe.sh` snapshot into a short Markdown
ownership summary: which remoteproc instances exist, which display, rproc
and reserved-memory device-tree nodes are present, the SCMI/SM version lines
from dmesg, and the display-related clock and power-domain entries.

## Usage

```
imx95-observe-summary.py [FILE]
```

- `FILE` (optional): path to a snapshot text file. Without it the script
  reads the snapshot from standard input.
- Any option (an argument starting with `-`) or more than one argument
  prints the usage text to stderr and exits 2. There are no flags.

### Exit status

| code | meaning |
|---|---|
| 0 | summary written |
| 1 | input unreadable, or not an `imx95-observe.sh` snapshot |
| 2 | usage error |

## Input

The text produced by `scripts/imx95-observe.sh`: a first line starting with
`# imx95-observe`, then sections introduced by `### ` headings (`date`,
`uname -r`, `remoteproc`, `device tree`, `dmesg`, `debugfs clk`,
`debugfs pm_genpd`). The `remoteproc` and `device tree` sections are
required; the others are optional. Path lines (starting with `/`) followed
by indented `key: value` lines form the records.

## Output

Markdown on stdout, with these headings in order:

- `remoteproc`: table of instance, name, state, firmware
- `display nodes`, `rproc nodes`, `reserved-memory nodes`: tables of node,
  compatible, status (a missing status is shown as `okay (default)`)
- `SCMI / SM version`: dmesg lines that mention SCMI or SM and a version,
  otherwise `not observed`
- `clock entries (dpu/ldb/lvds/disp)` and `genpd entries (dpu/ldb/lvds/disp)`:
  matching lines, or `none`, or `not readable`

An empty table is written as `none`.

## Example

```
scripts/imx95-observe-summary.py /path/to/snapshot.txt > /path/to/summary.md
scripts/imx95-observe-summary.py < /path/to/snapshot.txt
```

## Caveat

The output is a reading of a dated snapshot, not a new hardware observation.
It adds no data the snapshot did not contain; when quoting it, keep the
snapshot's date and tag it as read from that snapshot.
The script is host-only and never contacts a board.
