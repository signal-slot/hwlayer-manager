# i.MX95 M7: taking the Zephyr log over rpmsg / trace buffer (design note)

Status: design only, written 2026-11-28. No board was accessed for this
note and no code was written. Nothing here is a hardware result.

Purpose: a later board run starts a Zephyr image on the i.MX95 Cortex-M7
through Linux remoteproc (`imx_rproc`) on the Verdin iMX95 (Reference
Multimedia Image 7.8.0, NXP kernel 6.6.157-7.8.0) and must capture its log.
This note compares the M7 UART with two rpmsg-era alternatives and says what
the image and the Linux side would need.

Tags used for every finding:

- **read from source**: I read the cited file in the cited checkout in this
  session.
- **recorded observation**: copied from the dated, read-only board snapshot
  already in the repository
  ([`imx95-zephyr/observed-2026-11-27.txt`](imx95-zephyr/observed-2026-11-27.txt),
  taken 2026-11-27); not re-observed here.
- **inferred**: derived from the above or from general remoteproc/OpenAMP
  behaviour that I did **not** read in the 6.6.157-7.8.0 tree. Inferred items
  must be confirmed on the board before they are relied on.

## 0. Sources and what could not be read

| Source | Pin | Used for |
|---|---|---|
| Zephyr, `https://github.com/zephyrproject-rtos/zephyr` | commit `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (tree `VERSION` 4.5.99; `git describe --tags` in the local workspace checkout gives `v4.5.0-5218-ge53873cbd0e`, i.e. 5218 commits after tag `v4.5.0`; read 2026-11-28) | `samples/subsys/ipc/openamp_rsc_table/**`, `subsys/ipc/open-amp/{resource_table.c,resource_table.h,Kconfig}`, `drivers/console/Kconfig`, `subsys/shell/backends/Kconfig.backends`, `boards/nxp/imx95_evk/**` |
| Repository snapshot `docs/imx95-zephyr/observed-2026-11-27.txt` | this repository, commit `4e7b888` | device-tree and remoteproc state of the Verdin iMX95 |
| Repository `scripts/imx95-m7-rproc.sh` | this repository, commit `4e7b888` | load, stop and restore procedure |

Not read (no network access from the writing node, 2026-11-28): the Toradex /
NXP Linux 6.6.157-7.8.0 tree (`drivers/remoteproc/imx_rproc.c`,
`remoteproc_core.c`, `drivers/rpmsg/*`, `drivers/tty/rpmsg_tty.c`, the
defconfig) and the Zephyr commit's presence on the upstream remote. Every
Linux-side statement below is therefore **inferred** and carries the
standard-kernel behaviour I expect; a follow-up must pin the kernel tree
(URL plus commit or tag) and re-tag them. The Zephyr commit is known only
from the local checkout; whether it is an ancestor of upstream `main` was not
verified.

## 1. Why consider rpmsg / trace instead of the UART

- **inferred** The Verdin iMX95 M7 console UART is probably not the UART
  Linux uses; whether it is routed to a header on the Mallow carrier is the
  concern of the BSP study item (`verdin-m7-bsp`), not this note. A log path
  that needs no extra wire lets the board run use the ssh session alone.
- **inferred** The remoteproc trace buffer and rpmsg both reach the same ssh
  session that starts the core, so the run can interleave start, log and stop
  with one clock.
- The UART stays the fallback and the only log if the core crashes before the
  resource table is parsed, because the table is read by Linux *before* the
  core starts (inferred, standard remoteproc order: `rproc_fw_boot` parses
  the table, then boots).

Recommendation (**inferred**): enable both. The trace buffer
(Section 2) as the primary, zero-protocol, post-mortem log; rpmsg
(Sections 3-4) only if a live or bidirectional channel is wanted.

## 2. Resource table and trace buffer

### What upstream Zephyr already provides (read from source)

`subsys/ipc/open-amp/resource_table.h` and `resource_table.c` at the pinned
commit define a Linux-compatible table in a section named `.resource_table`:

- header `ver = 1` followed by an offset array;
- **vdev entry** (when `CONFIG_OPENAMP_RSC_TABLE_NUM_RPMSG_BUFF > 0`): one
  `RSC_VDEV` of id `VIRTIO_ID_RPMSG`, feature bit 1 (rpmsg name-service
  announce), two vrings, `da = -1` ("allocated by Master processor",
  i.e. Linux fills them), alignment `CONFIG_OPENAMP_VRING_ALIGNMENT`
  (default 16), vring notify ids `CONFIG_OPENAMP_RSC_TABLE_IPM_RX_ID`
  (default 0, host to remote) and `..._IPM_TX_ID` (default 1, remote to
  host), each with `NUM_RPMSG_BUFF` entries;
- **trace entry** (when `CONFIG_RAM_CONSOLE` is set): `RSC_TRACE` named
  `"Zephyr_log"`, device address = the RAM console buffer, length
  `CONFIG_RAM_CONSOLE_BUFFER_SIZE`. With `CONFIG_RAM_CONSOLE_BUFFER_SECTION`
  the address is the `reg` of the `zephyr,ram-console` chosen node and a
  `BUILD_ASSERT` requires it to fit in 32 bits; otherwise it is the address of
  the `ram_console_buf` symbol (default 1024 bytes, wraps).
- `CONFIG_OPENAMP_COPY_RSC_TABLE` copies the table to the `zephyr,ipc_rsc_table`
  node; the Kconfig help says it is **not needed when remoteproc loads the
  firmware and controls placement of `.resource_table`**.

The sample's board file
`samples/subsys/ipc/openamp_rsc_table/boards/imx95_evk_mimx9596_m7.{conf,overlay}`
enables `CONFIG_OPENAMP_RSC_TABLE`, `CONFIG_OPENAMP_COPY_RSC_TABLE`,
`CONFIG_OPENAMP_RSC_TABLE_NUM_RPMSG_BUFF=8`, `CONFIG_SHELL_BACKEND_RPMSG`
and `CONFIG_LOG_MODE_MINIMAL`; it does **not** enable `CONFIG_RAM_CONSOLE`,
so the sample as shipped has no trace entry (read from source).

### What Linux then does (inferred)

Linux `rproc_handle_trace()` (mainline `remoteproc_core.c`; not read in the
7.8.0 tree) turns each `RSC_TRACE` entry into a debugfs file
`/sys/kernel/debug/remoteproc/remoteproc<N>/trace<k>`; the first trace entry
is `trace0`. It translates the entry's device address through the
carveout/memory-region map (`rproc_da_to_va`); if the address is not inside a
region the driver can map, the entry is rejected and no `trace0` appears.
The read is a plain copy of `len` bytes from the buffer, with no read pointer:
a wrapped Zephyr RAM console shows its raw ring, start in the middle. The
index `<N>` is dynamic: the snapshot has the M7 as `imx-rproc`
(**recorded observation**, 2026-11-27, `remoteproc1`, with a `neutron-rproc`
at `remoteproc0`), so select it by its `name` file, never by number.

### Design decisions (inferred)

1. Put the RAM console in a region Linux maps. The snapshot shows the
   `remoteproc-cm7` node with seven `memory-region` phandles
   (**recorded observation**); reserved-memory children visible are
   `vdev0vring0` (size 0x8000), `vdev0vring1` (0x8000), `vdev1vring0`
   (0x8000), `vdev1vring1` (0x8000), `vdevbuffer` (0x100000) and
   `rsc-table` (0x1000), plus a `memory@80000000` of 0x01000000 bytes.
   Which phandle is which, and which regions are in the M7's list, was not
   resolved in the snapshot. Safest placement (inferred): a
   `zephyr,ram-console` node inside the `vdevbuffer` region above the
   rpmsg buffers, or in the region the ELF itself loads into, once the
   `verdin-m7-bsp` study fixes the M7 memory map.
2. Keep the buffer small (1-4 KiB) and use `CONFIG_RAM_CONSOLE_BUFFER_SECTION`
   so the address is a constant; it avoids relying on the symbol address
   matching the Linux view.
3. Cache: the M7 buffer lives in memory Linux reads through a non-cached or
   `no-map` mapping (the snapshot shows `no-map` on the vring and
   `rsc-table` regions, **recorded observation**). The Zephyr side must
   write the buffer through a non-cacheable MPU region or clean it
   explicitly; the sample sets `CONFIG_OPENAMP_WITH_DCACHE=y` for the shared
   memory (read from source). Inferred: the RAM console needs the same.
4. A trace entry is independent of the vdev entry. The M7 can start with the
   table containing only the trace entry
   (`CONFIG_OPENAMP_RSC_TABLE_NUM_RPMSG_BUFF=0`), which removes the MU,
   vrings and rpmsg driver from the first run's risk.

## 3. Zephyr rpmsg support for the M7 (read from source)

- Board `imx95_evk/mimx9596/m7` exists (variants plain, `ddr`, `flash`
  under `boards/nxp/imx95_evk/`); the sample overlay targets the plain M7
  board, not a Verdin board. A Verdin M7 board definition is not upstream
  (inferred from the directory listing at the pinned commit: only the EVK is
  present); the overlay and conf would be reused as an application overlay on
  the EVK board, as in the `m7-app` item.
- Sample `samples/subsys/ipc/openamp_rsc_table` is the remote side of the
  Linux rpmsg client sample and the Linux `rpmsg_tty` driver; its README
  says so and shows `cat /dev/ttyRPMSG0` and `echo ... >/dev/ttyRPMSG0`.
  `CONFIG_SHELL_BACKEND_RPMSG` (in `subsys/shell/backends/Kconfig.backends`)
  also puts the Zephyr shell on an rpmsg endpoint.
- The overlay (read from source):
  - `zephyr,ipc_shm = &shmem` with `shmem` = `mmio-sram` at `0x88000000`,
    size `0x500000`;
  - `zephyr,ipc_rsc_table = &rsc_table` = `mmio-sram` at `0x88220000`,
    size `0x100`;
  - `zephyr,ipc = &mailbox0`, a `zephyr,mbox-ipm` with
    `mboxes = <&mu7 1>, <&mu7 1>` ("tx", "rx"), `&mu7` enabled;
  - `CONFIG_ARM_SCMI=y`, `CONFIG_CLOCK_CONTROL=y`, `CONFIG_MBOX_INIT_PRIORITY=0`.
- The addresses match the snapshot's reserved-memory (vdev0 vring0 at
  `0x88000000`, rsc-table at `0x88220000`; **recorded observation**), so
  the sample was written for the same layout the Toradex DT reserves
  (inferred: both follow NXP's imx95 remoteproc layout). The `rsc_table`
  size of 0x100 in the overlay is smaller than the DT's 0x1000; a build
  assertion (`BUILD_ASSERT(sizeof(struct fw_resource_table) <= RSC_TABLE_SIZE)`
  when `COPY_RSC_TABLE` is on) guards it (read from source).
- Open point: `CONFIG_OPENAMP_RSC_TABLE_IPM_RX_ID`/`TX_ID` default to 0/1,
  which are also the vring ids Linux kicks; the DT mailbox indices (below)
  must agree. Not verifiable without the 6.6.157 `imx_rproc.c`.

## 4. Linux side

### Module / driver availability (not read; inferred)

- `imx_rproc` is evidently present (**recorded observation**: `imx-rproc is
  available` in dmesg, a `remoteproc-cm7` platform device with compatible
  `fsl,imx95-cm7`).
- Whether `CONFIG_RPMSG_CHAR`, `CONFIG_RPMSG_TTY`, `CONFIG_RPMSG_CTRL`,
  `CONFIG_RPMSG_VIRTIO`, `CONFIG_RPMSG_NS` and `CONFIG_DEBUG_FS` are
  built in, modular or off in the 7.8.0 image is **unknown**. A `rpmsg-lifecycle`
  node (`nxp,rpmsg-lifecycle`) is present (**recorded observation**), which
  suggests NXP's rpmsg stack is built; that is not proof of
  `rpmsg_char`/`rpmsg_tty`. The board run should read `/proc/config.gz`
  (if present), `lsmod` and `ls /dev/rpmsg* /dev/ttyRPMSG*` read-only
  first. If neither driver exists, rpmsg is out and the trace buffer (needing
  only `imx_rproc` and debugfs) is the log path.
- `trace0` additionally needs debugfs mounted at `/sys/kernel/debug`
  (inferred; unverified on 7.8.0).

### Which driver to prefer (inferred)

- **`rpmsg_tty`**: Zephyr's sample announces a `rpmsg-tty` channel and Linux
  creates `/dev/ttyRPMSGn`; plain `cat` reads the log. Best for a one-way log
  plus occasional commands. Requires `CONFIG_RPMSG_TTY`.
- **`rpmsg_char`** (`/dev/rpmsg_ctrlN`, `/dev/rpmsgN`): needs a small program
  to create endpoints; more flexible, more work. Not recommended for the
  first run.
- Zephyr's name-service announcement selects the Linux driver: the channel
  name decides which driver binds (inferred, standard rpmsg bus behaviour).

### Reserved-memory and mailbox nodes (recorded observation unless noted)

- Reserved-memory children (sizes in bytes): `vdev0vring0@88000000` 0x8000,
  `vdev0vring1@88008000` 0x8000, `vdev1vring0@88010000` 0x8000,
  `vdev1vring1@88018000` 0x8000, `vdevbuffer@88020000` 0x100000
  (`shared-dma-pool`, `no-map`), `rsc-table@88220000` 0x1000 (`no-map`).
  Two vdev pairs are reserved; the Zephyr resource table defines one vdev,
  so the second pair stays unused.
- `remoteproc-cm7` mailboxes: `mbox-names = "tx", "rx", "rxdb"`, all on one
  MU instance with indices 1, 1 and 1 and types 0, 1 and 3. This matches
  the sample overlay's `<&mu7 1>` for both directions (read from source),
  and the `rxdb` doorbell is what a vring kick from the M7 arrives on
  (inferred).
- Nothing in the Linux DT needs to change for either path if the table
  addresses fit the existing reserved regions; the whole recipe is
  therefore expected to be doable without a DT change (inferred; the
  snapshot shows the nodes enabled, state `offline`).

## 5. Minimal recipe (write only under /tmp; nothing is run here)

Every step is for the later board item (`m7-run`); it assumes the host-built
ELF with the resource table described in Section 2 exists and no other item
holds the board.

1. Read-only checks (no writes): note `name`, `state`, `firmware` of the M7
   remoteproc (found by `name`), the value of
   `/sys/module/firmware_class/parameters/path`, whether debugfs is mounted,
   which of `/dev/rpmsg*`, `/dev/ttyRPMSG*` exist, `/proc/config.gz`
   contents for the options in Section 4, and that Weston is active.
2. Copy the ELF to a new directory under `/tmp` on the board.
3. Use `scripts/imx95-m7-rproc.sh` (it writes only the remoteproc
   `firmware`, `state` and the `firmware_class` path parameter, and restores
   them; see its header) to point the firmware search path at the `/tmp`
   directory, set the firmware name, and start the core.
4. Log capture, in this order of preference:
   a. `cat` the debugfs `trace0` file for the M7 remoteproc, repeatedly, with
      a host-side timestamp per read;
   b. if `rpmsg_tty` exists: `cat` the new `/dev/ttyRPMSGn` into a file
      under `/tmp` in the background;
   c. M7 UART, if wired, as the cross-check (separate item).
5. Stop the core through the script; confirm state `offline`.

### Restore list (inferred; mirrors what the script restores)

- remoteproc `state`: `offline` (as found);
- remoteproc `firmware`: the original name (the snapshot shows
  `rproc-imx-rproc-fw`, **recorded observation**);
- `firmware_class` `path`: the original value (empty in the snapshot);
- kill the background `cat`, delete the `/tmp` directory and capture files
  after copying them to the host;
- if a kernel module (for example `rpmsg_tty`) had to be loaded with
  `modprobe`, `rmmod` it; loading a module writes nothing to the rootfs but
  changes runtime state, so list it in the report. If the module is not
  already available, stop: installing it would break the "no packages / write
  only under /tmp" rule;
- confirm Weston is still active and that the M7 `state` and the firmware
  name read back as at the start. Never start or stop Weston.

## 6. Risks and open items

1. Linux-side facts (Section 4 driver set, `trace0` behaviour, vring and
   mailbox index matching in 6.6.157-7.8.0) are not read from source; pin
   the Toradex / NXP kernel tree and re-tag them.
2. The Zephyr overlay is for the EVK memory map; the Verdin M7 memory map and
   which regions the `remoteproc-cm7` node lists must be matched in
   `verdin-m7-bsp` and `m7-app`. If the RAM-console region is not mapped by
   `imx_rproc`, `trace0` will not appear (Section 2), but the core itself
   can still start.
3. A first run should use the trace-only table (vdev entries off) to
   separate "core boots" from "MU and vrings work", and only then enable
   rpmsg.
4. `CONFIG_LOG_MODE_MINIMAL` in the sample drops formatted log output; a
   run that wants the engine's trace lines (`trace-format`) needs deferred or
   immediate logging with `CONFIG_LOG_BACKEND_RAM`-style or
   `RAM_CONSOLE` output, chosen in the app, not in this note.
5. Timing claims from log timestamps are not measured here; this path adds
   an unmeasured delay, so scanout latency must not be measured through it.
