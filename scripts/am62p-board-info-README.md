# am62p-board-info.sh

Read-only collector for the M3 AM62P report (`docs/m3-am62p-report.md`). Run it
on the board (Torizon OS), e.g. `ssh <board> sh -s < scripts/am62p-board-info.sh`,
and paste the markdown table into the report's board/BSP/kernel/panel section.

    sh scripts/am62p-board-info.sh [--root DIR]

It reads only: `/etc/os-release`, kernel release and version
(`/proc/sys/kernel/osrelease` and `version`, the data behind `uname -r` / `-v`),
`/proc/device-tree/model` and `compatible`, `/sys/class/drm/card*-*` (status,
first modes, monitor name from the EDID), `/sys/class/remoteproc/*` (name, state,
firmware) and, for an R5F instance, the symlink `/lib/firmware/<firmware>`. It
writes nothing.

Never printed: `/proc/device-tree/serial-number` (not read), the EDID serial
descriptor (not decoded), the host name, machine-id. Every value is also filtered
for MAC and IP addresses, 32-digit hex ids and `serial ...` tokens
(`<redacted>`). Still read the output before committing it.

`--root DIR` makes all paths relative to DIR, so a fake tree can be used.
Test: `sh scripts/am62p-board-info.test.sh` (fixture in `$TMPDIR`, fake values).
