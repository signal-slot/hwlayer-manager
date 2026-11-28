# am62p-dss-regs

Host parser for a read-only snapshot of the AM62P display subsystem (DSS) state.
The board session saves the text next to each photograph; this script turns it
into a table per video port (VP) and per plane. It parses saved text only. It
never reads `/dev/mem` or any device and writes nothing.

```
python3 -I scripts/am62p-dss-regs.py [--json] [FILE]     # FILE or stdin
sh scripts/am62p-dss-regs.test.sh                        # self-test
```

Output is a Markdown table (default) or JSON (`--json`). Exit status: 0 parsed,
2 unknown format or usage error (nothing on stdout).

## Input formats

Detected from the content.

### `drm-state`: `/sys/kernel/debug/dri/N/state`

The DRM atomic state print of the DRM core (`drm_atomic_helper_plane_print_state`
and `drm_atomic_helper_crtc_print_state`, `drivers/gpu/drm/drm_atomic.c` and
`drm_atomic_helper.c`). Per plane: CRTC, framebuffer id, fourcc, `crtc-pos`
(size and position), `normalized-zpos` (hex), an `alpha=` line if the kernel
prints one (the core print does not, so the column is usually `n/a`). Per CRTC:
`enable`, `active`, mode name, `plane_mask`. A plane is *enabled* when it has a
CRTC and a framebuffer. The GO bit is not part of this file (`n/a`).

### `regdump`: register values by name

Line format, one register per line, `#` starts a comment:

```
VP<n> <REG> = 0x<value>            e.g. VP0 CONTROL = 0x00000021
VID<n> <REG> = 0x<value>           e.g. VID1 POSITION = 0x00320064
OVR<n>.L<k> ATTRIBUTES = 0x<value> overlay layer k (= zorder k) of overlay n
```

Registers read: `VP CONTROL`, `VP SIZE_SCREEN`; `VID ATTRIBUTES`, `POSITION`,
`SIZE`, `GLOBAL_ALPHA`; `OVR ATTRIBUTES`. The output has the VP enable, GO bit
and size, and per plane the enable, format code, size, position, zorder
(layer index in the overlay), VP, layer enable and global alpha. The format
code is shown raw (`0x27`); no fourcc names are guessed.

**Status of this format, read before relying on it.** This is the capture
format *this script defines*. I found no tidss debugfs register dump that I
could confirm: the sources could not be read in the session that wrote the
script (see below), so which kernel offers a register dump, under which file
name and in which layout, is open. Until it is confirmed, the register
dump is whatever the session converts into the lines above; `drm-state` needs
no conversion and is the format to capture first.

## Register fields

Bit positions used by the `regdump` parser (all in `scripts/am62p-dss-regs.py`,
constants at the top):

| register | field | bits | source |
|---|---|---|---|
| `VP CONTROL` | ENABLE | 0 | tidss `dispc_vp_enable` / `dispc_vp_prepare` |
| `VP CONTROL` | GOBIT | 5 | tidss `dispc_vp_go` |
| `VP SIZE_SCREEN` | width-1, height-1 | 11:0, 27:16 | tidss `dispc_vp_prepare` |
| `VID ATTRIBUTES` | ENABLE | 0 | tidss `dispc_plane_enable` |
| `VID ATTRIBUTES` | FORMAT | 6:1 | tidss `dispc_plane_setup` |
| `VID GLOBAL_ALPHA` | alpha | 7:0 | tidss `dispc_plane_setup` |
| `VID POSITION` | x, y | 13:0, 29:16 | tidss `dispc_plane_setup` |
| `VID SIZE` | width-1, height-1 | 13:0, 29:16 | tidss `dispc_plane_setup` |
| `OVR ATTRIBUTES(layer)` | ENABLE | 0 | tidss `dispc_ovr_set_plane` |
| `OVR ATTRIBUTES(layer)` | plane id | 4:1 | tidss `dispc_ovr_set_plane` |
| `OVR ATTRIBUTES(layer)` | video port (CHANNELIN) | 13:10 | tidss `dispc_ovr_set_plane` |

Sources, public: Linux `drivers/gpu/drm/tidss/tidss_dispc.c` (the functions
named above) and `tidss_dispc_regs.h` (field masks), and the AM62Px TRM DSS
chapter (TI document SPRUJ83). The script uses field positions only; it
contains no register address offsets, because the text format names the
registers and a dump taken through the kernel does not need them.

Commits identified for the two files (latest commit touching each, from the
GitHub API on 2026-11-28):

- `tidss_dispc_regs.h`: `7287177e5abe2472c03f1c78099352c64d1b86fc`
- `tidss_dispc.c`: `4dd10fbbeb9c33709edc89916e19c03c780a0751`

**Not yet checked against those commits.** The field positions in the table
were written from knowledge of the driver, not re-read from the two files at
these commits: raw.githubusercontent.com answered HTTP 429 and no kernel tree
was readable in the session. Whoever next has the sources must compare each
row with `tidss_dispc.c` and the TRM and correct the constants; the self-test
checks the parser against the fixtures, not the fields against the hardware.
The OVR plane-id and video-port positions are the least certain rows.

## Fixtures

`scripts/testdata/am62p-dss-regs/` holds **synthetic** inputs, written by hand
from the formats above. None comes from a board. They contain no addresses,
host names or serial numbers.

- `regdump.txt`: VP0 enabled with GO set, VP1 disabled, three planes (the third
  disabled), one overlay.
- `drm-state.txt`: three planes (the third without CRTC and framebuffer) and one CRTC.
- `unknown.txt`: not a DSS dump; the script must exit 2.
