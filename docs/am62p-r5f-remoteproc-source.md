# AM62P MCU R5F under Linux remoteproc: trace buffer, stop and memory, from source

Source study for M3 (WP4, [`open-issues.md`](open-issues.md) M3-01, M3-02). The AM62P board test
(`tests/board/am62p_r5f`, `docs/board-test-am62p-r5f.md` on the owner's branch `work/am62p-board-test`) runs on the MCU
Cortex-R5F through Linux remoteproc and reads its console from the trace buffer. That document says a second stop of the
remote processor fails, so a second image needs a reboot. This page explains from source why a stop fails or is refused,
how the resource table's trace entry becomes `trace0`, and which memory an R5F image may use.

**Nothing here is observed on a board.** No device was touched. Every fact below carries a tag.

## Tags

- **read from source** (file, function, commit): read in the text of the file at the commit pinned below, 2026-11-28.
  Code was fetched as raw text, so no line numbers are given; functions are named instead.
- **inferred**: derived from the facts read, or from a comment, not read as code.
- **not read**: stated so that nobody mistakes a gap for a finding (see [What was not read](#what-was-not-read)).

## Source pins

| Tree | Pin | Used for |
|---|---|---|
| TI `ti-linux-kernel`, branch `ti-linux-6.6.y` | tip `a7758da17c2807e5285d6546b6797aae1d34a7d6` (branch tip as listed by `git ls-remote` on 2026-11-28; the individual file commits could not be listed, see below) | the K3 R5F driver, `remoteproc_{core,sysfs,elf_loader,debugfs}.c`, `remoteproc_internal.h`, the AM62P device tree, the binding. Raw URL form: `https://raw.githubusercontent.com/TexasInstruments/ti-linux-kernel/a7758da17c2807e5285d6546b6797aae1d34a7d6/<path>` |
| Mainline, tag `v6.6` | commit `ffc253263a1375a65fa6c9f62a893e9767fbebfa` (tag object `5260836abb7056beed3f3f0d0e4262c11f36f0d0`) | `remoteproc_core.c`, `remoteproc_debugfs.c`, `remoteproc_sysfs.c`, `remoteproc_internal.h`, `include/linux/remoteproc.h`. Raw URL form: `https://raw.githubusercontent.com/torvalds/linux/ffc253263a1375a65fa6c9f62a893e9767fbebfa/<path>` |
| Mainline, tag `v6.18` | commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` (tag object `f7b88edb52c8dd01b7e576390d658ae6eef0e134`) | file names of the Verdin AM62P device tree, commit lists only. The driver and the AM62P nodes at this tag were not read |
| Toradex `linux-toradex`, branch `toradex_ti-linux-6.6.y` | tip `9764efd3fe8e5f3fa04045919296c102acd7517e` (`git ls-remote`, 2026-11-28) | **not read**: the host answers fetches with an anti-bot page. The branch name is the Verdin AM62P BSP's kernel line (inferred from the branch name, not from the board) |

The Toradex BSP kernel is a TI kernel with Toradex's board files on top (inferred from the branch name
`toradex_ti-linux-6.6.y`). Everything about the driver below therefore describes the TI 6.6 tree, **not** the Toradex tree
that runs on the board. Whether the two drivers are identical must be checked on the board (`/sys/class/remoteproc`,
`dmesg`) or by reading the Toradex tree.

## 1. The driver and its modes (TI 6.6)

| Fact | Tag |
|---|---|
| Driver `k3_r5_rproc`, compatibles `ti,am654-r5fss`, `ti,j721e-r5fss`, `ti,j7200-r5fss`, `ti,am64-r5fss`, `ti,am62-r5fss`, `ti,j721s2-r5fss`; AM62P uses `ti,am62-r5fss` with `am62_soc_data` (`is_single_core = true`, `tcm_ecc_autoinit = true`) | read from source: `drivers/remoteproc/ti_k3_r5_remoteproc.c`, `k3_r5_of_match`, `am62_soc_data`, TI `a7758da1` |
| Cluster mode defaults to `CLUSTER_MODE_SINGLECORE` when `ti,cluster-mode` is absent and `is_single_core` is set; one R5F child node is required (`num_cores != 1` fails the probe) | read from source: `k3_r5_probe`, TI `a7758da1` |
| The AM62P SoC dtsi does not set `ti,cluster-mode` on either R5F subsystem | read from source: `arch/arm64/boot/dts/ti/k3-am62p-mcu.dtsi`, `k3-am62p-wakeup.dtsi`, TI `a7758da1` |
| `k3_r5_cluster_rproc_init` allocates one rproc per core with `devm_rproc_alloc`, `firmware-name` from DT, `has_iommu = false`, `recovery_disabled = true`; it never sets `auto_boot` | read from source: `k3_r5_cluster_rproc_init`, TI `a7758da1` |
| `rproc_alloc` sets `auto_boot = true` and the initial state `RPROC_OFFLINE` | read from source: `remoteproc_core.c:rproc_alloc`, mainline `v6.6` `ffc25326` |
| `rproc_add` calls `rproc_trigger_auto_boot` when `auto_boot` is set. In state `RPROC_DETACHED` that calls `rproc_boot` (an attach). Otherwise it calls `request_firmware_nowait` for `rproc->firmware` and boots from the callback | read from source: `remoteproc_core.c:rproc_add`, `rproc_trigger_auto_boot`, `rproc_auto_boot_callback`, `v6.6` `ffc25326`; the TI-tree core is identical in the respects checked (boot, stop, shutdown, trace), TI `a7758da1` |
| Consequence: if a file named like the DT `firmware-name` exists on the firmware search path when the driver probes, the R5F boots at probe without anyone writing `start` | inferred from the two rows above |
| Two modes are decided per core in `k3_r5_rproc_configure_mode`. **IPC-only** if the TI-SCI device is on (`c_state`), the local reset is deasserted and the core is not halted: state becomes `RPROC_DETACHED`, and the ops `prepare`, `unprepare`, `start`, `stop` are set to NULL, `attach` and `detach` (both no-ops) and `get_loaded_rsc_table` are installed. **Remoteproc mode** if the device is off (`!c_state`). Anything else is `mismatched mode ... ` and `-EINVAL` | read from source: `k3_r5_rproc_configure_mode`, TI `a7758da1` |
| The mode decision is made once, at driver probe | read from source: `k3_r5_cluster_rproc_init` calls `k3_r5_rproc_configure_mode`, TI `a7758da1` |
| In IPC-only mode the resource table is taken from the base of DDR memory region #1 (`kproc->rmem[0]`), at most 256 bytes. The code comment calls this "design-by-contract" so that any bootloader can start the core | read from source: `k3_r5_get_loaded_rsc_table`, TI `a7758da1` |
| In remoteproc mode the driver sets up TCM and boot configuration through TI-SCI (`ti_sci_proc_set_config`, `ti_sci_proc_set_control`, `ti_sci_proc_get_status`), starts the core by clearing the halt bit, and stops it by setting the halt bit | read from source: `k3_r5_rproc_configure`, `k3_r5_rproc_start`, `k3_r5_rproc_stop`, `k3_r5_core_run`, `k3_r5_core_halt`, TI `a7758da1` |
| Mainline's newest commits to `ti_k3_r5_remoteproc.c` up to 2025-08-24 move shared code into a common K3 driver (subjects "Refactor ... into common driver", 2025-05-13, first `80e8a868c90ed03228510a669449c8e4999436fd` .. `6fdad99c94d721182100071be02873f2b41b6490`) and include "remoteproc: k3-r5: Fix IPC-only mode detection" `a8631f6d6344d976096b1efafdb2fbb3111bd790` (2024-06-21) | read from source: commit subjects and dates from the GitHub commit list for the path, mainline at `v6.18` `7d0a66e4`. Only the first ten SHAs were read verbatim (`2433961962be8b8ff32058763339b818128e8ff0` .. `80e8a868c90ed03228510a669449c8e4999436fd`); the rest came from a summary of the list and are less certain |
| Whether `a8631f6d` or any of the refactoring commits is in the TI 6.6 tree or in the Toradex tree | not read |

## 2. Why a stop is refused or fails

Entry point: writing `stop` to `/sys/class/remoteproc/remoteprocN/state`. Source for the table: `remoteproc_sysfs.c:state_store`,
`remoteproc_core.c:rproc_shutdown`, `rproc_stop`, `rproc_boot`, `rproc_detach`, TI `a7758da1` (mainline `v6.6` `ffc25326` identical in
these functions).

### The path

1. `state_store` accepts exactly `start`, `stop`, `detach`; anything else logs `Unrecognised option` and returns `-EINVAL`.
   (read from source)
2. `stop` calls `rproc_shutdown`. It returns `-EINVAL`, without a log line, unless the state is `RPROC_RUNNING` or
   `RPROC_ATTACHED`. (read from source)
3. `rproc_shutdown` then decrements `rproc->power`; if the count is still above zero it returns 0 and does nothing.
   Otherwise it calls `rproc_stop`. If that fails it increments the count again and returns the error; the state stays as it was.
   (read from source)
4. `rproc_stop` returns `-EINVAL` at once when `ops->stop` is NULL. Otherwise it calls, in this order:
   `rproc_stop_subdevices`, `rproc_reset_rsc_table_on_stop` (which copies the resource table out of the firmware's memory into
   `cached_table`; an error is logged as `can't reset resource table`), then `ops->stop`. A non-zero return is logged as
   `can't stop rproc: <err>`. Only on success does the state become `RPROC_OFFLINE`. (read from source)
5. After `rproc_stop` succeeded, `rproc_shutdown` runs `rproc_resource_cleanup` (this removes the `traceN` files, the carveouts
   and the rvdevs), `rproc_unprepare_device` (the return value is not checked) and frees the cached table. (read from source)

### The ways it fails

| # | Situation | Result | Tag |
|---|---|---|---|
| A | The core is in IPC-only mode: state `detached`. `stop` is refused by step 2 (`-EINVAL`, silent). `start` attaches (`rproc_boot` -> `rproc_attach`; no firmware is loaded). In state `attached` `stop` passes step 2 and then fails in step 4 because `ops->stop` is NULL (`-EINVAL`, silent). Only `detach` works (`rproc_detach`: needs `RPROC_ATTACHED`, calls the no-op `k3_r5_rproc_detach`) and it leaves the core running; after a detach the state is `detached` again (inferred: the body of `__rproc_detach` and `rproc_attach` were not read) | `stop` can never power the core off in this mode | read from source: `k3_r5_rproc_configure_mode`, `state_store`, `rproc_shutdown`, `rproc_stop`, `rproc_detach`, TI `a7758da1` |
| B | The core is `offline` (already stopped, or never started). A second `stop` is refused by step 2 with `-EINVAL` (shell: `Invalid argument`), and `dmesg` shows nothing | the benign form of "a second stop fails" | read from source: `rproc_shutdown`, mainline `v6.6` `ffc25326` |
| C | Remoteproc mode, split/single-core path: `k3_r5_rproc_stop` sends `RP_MBOX_SHUTDOWN` through the mailbox and waits 5000 ms for `shut_comp`. The firmware must answer `RP_MBOX_SHUTDOWN_ACK` (an rpmsg-style firmware does; whether a Zephyr image without IPC does is a property of the image). **If no ack arrives the code only logs `timeout waiting for rproc completion event` and goes on to halt the core: the stop then succeeds** (after the 5 s) | no failure from a silent firmware | read from source: `k3_r5_rproc_stop`, `k3_r5_rproc_mbox_callback`, TI `a7758da1` |
| D | Same path, but an ack arrived: the driver polls `is_core_in_wfi` every 200 us for at most 2000 us (`readx_poll_timeout`). If the R5F is not in WFI in that time, `ret` is `-ETIMEDOUT`, the function returns before halting, and the log shows `can't stop rproc: -110`. The state stays `running`. `is_core_in_wfi` also returns 0 when `ti_sci_proc_get_status` fails | a firmware that acks and then keeps running (a display loop, a busy thread) is not stopped | read from source: `k3_r5_rproc_stop`, `is_core_in_wfi`, TI `a7758da1` |
| E | `mbox_send_message` of the shutdown returns an error (it returns a negative value, e.g. a full queue; mailbox tx is non-blocking and the client has no `tx_done`): `PM mbox_send_message failed: <err>`, the stop fails before anything is halted | the error is the mailbox core's | read from source: `k3_r5_rproc_stop`, `k3_r5_rproc_request_mbox` (`tx_block = false`, `tx_done = NULL`), TI `a7758da1` |
| F | The state is `RPROC_SUSPENDED`: `k3_r5_rproc_stop` logs `We can't stop in suspended state!!!!`, frees the mailbox channel and **returns 0** | the mailbox is gone after this; a later start has no channel | read from source: `k3_r5_rproc_stop`, TI `a7758da1` |
| G | `k3_r5_rproc_unprepare` fails (reset assert): `rproc_shutdown` ignores the value, the stop still reports success | hidden failure | read from source: `rproc_shutdown`, `k3_r5_rproc_unprepare`, TI `a7758da1` |
| H | The driver is unbound and bound again (or the module is reloaded) after a stop. The core is then halted with its local reset asserted, but its module power stays on: `k3_r5_rproc_unprepare` uses `k3_r5_split_reset` for every mode except lockstep and single-CPU (so also single-core), and `k3_r5_split_reset` only asserts the local reset, with no `put_device`. `k3_r5_rproc_configure_mode` finds `c_state` true but the core halted or in reset, which is neither branch: `mismatched mode: local_reset = ..., module_reset = ..., core_state = ...`, `-EINVAL`, probe fails | no `remoteprocN` for the core until reboot | inferred from `k3_r5_rproc_unprepare`, `k3_r5_split_reset`, `k3_r5_rproc_configure_mode`, TI `a7758da1`; the module-power claim assumes `ti_sci` keeps the device on, not read |

`recovery_disabled` is true and `RP_MBOX_CRASH` is only logged (`K3 R5F rproc %s crashed`), so a crashed firmware is not
restarted by the driver (read from source: `k3_r5_cluster_rproc_init`, `k3_r5_rproc_mbox_callback`, TI `a7758da1`).

### What this says about "a second stop fails"

The board document's statement is a board result that this page cannot confirm. The source offers these candidates, in the
order in which the first run's `dmesg` and `state` would tell them apart:

- B, if the second `stop` came while the state was already `offline`: nothing is wrong (inferred).
- D, if `dmesg` shows `can't stop rproc: -110` and `state` still reads `running`: the image acked the shutdown and kept running (inferred from D).
- A, if `state` read `detached` or `attached` from the start: the R5F was started by something before Linux (inferred from A; this needs the state at boot).
- H, if the failure is at the next probe and not at the stop (inferred).
- The shutdown message and the echo request sent at probe stay in the mailbox FIFO until a firmware reads them (inferred from `k3_r5_rproc_request_mbox`, which says the echo reply "will wait in the mailbox fifo until the remote processor is booted"). A Zephyr image that reads the mailbox would see them. Whether the board test's image does is not in this study.

The first board run should record `state`, `dmesg | grep remoteproc` and the mailbox log lines around every start and stop, so that
the rows A to H can be told apart (inferred; this is a request to the board item, not a finding).

## 3. From the resource table's trace entry to `trace0`

| Step | What the code does | Tag |
|---|---|---|
| 1 | The ELF must have a section named exactly `.resource_table`. `find_table` rejects a version other than 1 and non-zero reserved bytes, or a table that does not fit the file. Without the section `rproc_elf_load_rsc_table` returns `-EINVAL` and the boot fails (`rproc_fw_boot` stops before loading) | read from source: `remoteproc_elf_loader.c:find_table`, `rproc_elf_load_rsc_table`, `remoteproc_core.c:rproc_fw_boot`, TI `a7758da1` |
| 2 | The table is copied to `cached_table`; `rproc_handle_resources` walks its offsets and calls the handler per type: carveout, devmem, trace, vdev. Unknown types are skipped with `unsupported resource`. A handler error aborts the boot | read from source: `rproc_elf_load_rsc_table`, `rproc_handle_resources`, `rproc_loading_handlers`, mainline `v6.6` `ffc25326` |
| 3 | `rproc_handle_trace` rejects a truncated entry and non-zero `reserved`, stores `da` and `len` from the `fw_rsc_trace` entry, names the file `trace%d` with `rproc->num_traces` (so `trace0`, `trace1`, in table order) and creates it with `rproc_create_trace_file`, mode 0400, in the rproc's debugfs directory | read from source: `rproc_handle_trace`, `rproc_create_trace_file`, TI `a7758da1` |
| 4 | The directory is `<debugfs>/remoteproc/remoteprocN/` (parent named `KBUILD_MODNAME`, directory `dev_name`), next to `name`, `recovery`, `crash`, `resource_table`, `carveout_memories`, `coredump`. There is no `state` file in debugfs: `state` and `firmware` are sysfs attributes under `/sys/class/remoteproc/remoteprocN/` | read from source: `rproc_create_debug_dir`, `remoteproc_sysfs.c` (`state`, `firmware`), TI `a7758da1` |
| 5 | The files are created during the boot (`rproc_fw_boot` -> `rproc_handle_resources`), before the core runs, and removed by `rproc_resource_cleanup` when the rproc stops. After a stop `trace0` is gone | read from source: `rproc_fw_boot`, `rproc_resource_cleanup`, `rproc_shutdown`, TI `a7758da1` |
| 6 | A read calls `rproc_da_to_va(rproc, trace->da, trace->len)` every time. If that is NULL the reader returns the text `Trace traceN not available`. Otherwise the length is `strnlen(va, trace->len)`: **the output ends at the first NUL byte**, so the buffer must hold text from its first byte, and a ring buffer that has a zero byte in front of its text reads as empty | read from source: `remoteproc_debugfs.c:rproc_trace_read`, TI `a7758da1` |
| 7 | `k3_r5_rproc_da_to_va` resolves a device address only in: ATCM/BTCM (R5 view and SoC view), the `sram` regions, and the static DDR regions (memory-region index 1 and up). Hence the trace buffer's `da` must lie in one of those. It must not be in memory-region 0 (the vring DMA pool), which is not in `rmem[]` | read from source: `k3_r5_rproc_da_to_va`, `k3_r5_reserved_mem_init`, TI `a7758da1` |
| 8 | The resource table's own `da` is looked up the same way. If it is not covered, `rproc_find_loaded_rsc_table` returns NULL and the cached table is not copied into the image's memory; the boot goes on | read from source: `rproc_elf_find_loaded_rsc_table`, `rproc_start`, TI `a7758da1` |
| 9 | Reading `trace0` needs debugfs mounted (`rproc_init_debugfs` creates the parent only if `debugfs_initialized()`) and root: mode 0400 | read from source: `rproc_init_debugfs`, `rproc_create_trace_file`, TI `a7758da1` |

The board branch's `prj.conf` puts the console in a RAM buffer "that the SoC's resource table offers to Linux as the remote
processor's trace buffer" (read from the branch `work/am62p-board-test`, tip `361e31f`, `tests/board/am62p_r5f/prj.conf`). Whether
the Zephyr RAM console's format ends in a NUL right after the last character, and whether it wraps, is not in this study
(not read). By row 6 a wrapped or zero-padded buffer is shown truncated at its first zero byte.

## 4. What memory the image may use

TI 6.6, SoC dtsi plus the AM62P SK board file (the SK file is the only reserved-memory source read; Verdin's is not, see
[What was not read](#what-was-not-read)).

### How Linux loads and checks it

| Fact | Tag |
|---|---|
| `rproc_elf_load_segments` takes the load address of every `PT_LOAD` segment with `memsz > 0` from `p_paddr`, asks `rproc_da_to_va` for `memsz` bytes and fails with `bad phdr da 0x... mem 0x...` (`-EINVAL`) if the range is not inside one covered region; it copies `filesz` bytes and zeroes the rest up to `memsz` | read from source: `rproc_elf_load_segments`, TI `a7758da1` |
| The ELF header check accepts ELF32 or ELF64, requires host endianness, requires at least one program header, and does **not** check `e_machine` | read from source: `rproc_elf_sanity_check`, TI `a7758da1` |
| The boot address is `e_entry`; `k3_r5_rproc_start` passes it to `ti_sci_proc_set_config`. `k3_r5_rproc_configure` had set the boot vector to 0 and cleared `TEINIT` (ARM mode) before | read from source: `rproc_elf_get_boot_addr`, `k3_r5_rproc_start`, `k3_r5_rproc_configure`, TI `a7758da1` |
| Memory outside the covered regions is never touched by the loader. A `PT_LOAD` segment there fails the boot; a section there that is not in any `PT_LOAD` (for example `NOBITS` that the linker did not put into a segment) is simply not initialised, and the memory is whatever Linux uses it for | inferred from the loader and `da_to_va` rows |
| The R5F has no MMU; a region address translator maps 32-bit R5F addresses to 64-bit bus addresses, only programmable by the R5F; the driver does not support it, so regions are limited to 32-bit bus addresses and device address equals bus address | read from source: comment in `k3_r5_reserved_mem_init`, TI `a7758da1` |

### The regions (AM62P SoC dtsi and SK board file, TI `a7758da1`)

| Region | Address | Size | Notes | Tag |
|---|---|---|---|---|
| MCU R5F TCM, ATCM | SoC bus `0x79000000` | 0x8000 | `ti,atcm-enable = <0>` in the SoC dtsi: the TCM is **disabled** for the core, although Linux can still write to it through the bus | read from source: `k3-am62p-mcu.dtsi`, `mcu_r5fss0_core0` |
| MCU R5F TCM, BTCM | SoC bus `0x79020000` | 0x8000 | `ti,btcm-enable = <1>`; `ti,loczrama = <0>` puts BTCM at R5F address 0 and ATCM at `0x41010000` (`K3_R5_TCM_DEV_ADDR`) | read from source: `k3-am62p-mcu.dtsi`, `k3_r5_core_of_get_internal_memories`, `k3_r5_rproc_configure` |
| MCU SRAM | `0x79100000` | 0x80000 (512 KiB) | `mcu_ram`, node `mmio-sram`, referenced by `sram = <&mcu_ram>`; device address equals bus address | read from source: `k3-am62p-mcu.dtsi`, `k3_r5_core_of_get_sram_memories` |
| Region 0: vring DMA pool | `0x9b800000` | 0x100000 | `mcu_r5fss0_core0_dma_memory_region`, `no-map`, used for `dma` allocations, **not** loadable | read from source: `k3-am62p5-sk.dts`, `k3_r5_reserved_mem_init` |
| Region 1: firmware carveout | `0x9b900000` | 0xf00000 (15 MiB) | `mcu_r5fss0_core0_memory_region`, `no-map`, device address = physical address, `ioremap_wc`; image sections and the trace buffer may be placed here | read from source: `k3-am62p5-sk.dts`, `k3_r5_reserved_mem_init` |
| `mailbox0_cluster1` / `mbox_mcu_r5_0` | `0x29010000` | | MCU core's mailbox: `ti,mbox-rx = <0 0 0>`, `ti,mbox-tx = <1 0 0>`, bound by `mboxes = <&mailbox0_cluster1 &mbox_mcu_r5_0>` | read from source: `k3-am62p-main.dtsi`, `k3-am62p5-sk.dts` |
| `rtos_ipc_memory_region` | `0x9b500000` | 0x300000 | `no-map`, shared-dma-pool, not referenced by the MCU core's `memory-region` | read from source: `k3-am62p5-sk.dts` |
| WKUP R5F (DM) regions | `0x9c800000` (0x100000), `0x9c900000` (0x1e00000) | | `wkup_r5fss0_core0_*`, other core | read from source: `k3-am62p5-sk.dts` |
| OP-TEE, TF-A | `0x9e800000` (0x1800000), `0x9e780000` (0x80000) | | `no-map` | read from source: `k3-am62p5-sk.dts` |

- The driver requires at least two `memory-region` entries (`device needs at least two memory regions to be defined`) and the
  binding allows two to eight. Index 0 is for vrings, indices 1 and up are static carveouts. (read from source:
  `k3_r5_reserved_mem_init`; `Documentation/devicetree/bindings/remoteproc/ti,k3-r5f-rproc.yaml`, TI `a7758da1`)
- The Zephyr test's two application buffers are 2 MB each in `.bss` (board document, branch tip `361e31f`). Together with code, data
  and stack that fits the 15 MiB carveout of the SK layout but **not** TCM or MCU SRAM. Whether the Verdin carveout has the SK's size is not
  known (inferred; not read). A linker script that places `.bss` anywhere else would put it into memory Linux owns.
- The DSS reads the application buffers by bus address, so the buffers need an address the display controller can reach; the
  R5F's address equals the bus address in the carveout (inferred from the device-address rule above).
- The SoC dtsi's `dss0` has `status = "disabled"` and no `memory-region`. The SK board file enables it and gives it no carveout, so
  Linux owns the display controller there (read from source: `k3-am62p-main.dtsi`, `k3-am62p5-sk.dts`, TI `a7758da1`). What
  the Verdin file does is not read.

### Which R5F runs the Device Manager

| Fact | Tag |
|---|---|
| The SK board file's comment on `wkup_uart0` says it "is used by DM firmware"; `main_uart1` is "used by TIFS firmware". The WKUP R5F node `wkup_r5fss0_core0` (`0x78000000`, firmware name `am62-wkup-r5f0_0-fw`) is enabled in the SK file | read from source: `k3-am62p5-sk.dts`, `k3-am62p-wakeup.dtsi`, TI `a7758da1` |
| The MCU R5F (`0x79000000`, `ti,sci-proc-ids = <0x03 0xff>`, firmware name `am62p-mcu-r5f0_0-fw`) is a separate core and has `mcu_gpio0` and `mcu_gpio_intr` marked "reserved for mcu firmware usage" | read from source: `k3-am62p-mcu.dtsi`, `k3-am62p5-sk.dts`, TI `a7758da1` |
| So the DM firmware shares the WKUP R5F, not the MCU R5F, and the board test does not take the DM's core | inferred from the two rows; the AM62x binding text says the single AM62 R5F "runs Device Manager firmware" (read from source: `ti,k3-r5f-rproc.yaml`, TI `a7758da1`), which describes the AM62x layout and is not contradicted by the AM62P files |
| The same SoC dtsi names the firmware `am62p-mcu-r5f0_0-fw`. TI documentation on IPC-only mode, early boot and the firmware layout was not read (web access to TI's documentation site was refused) | not read |

## What was not read

| Gap | Why it matters |
|---|---|
| The Toradex tree `toradex_ti-linux-6.6.y` at `9764efd3fe8e5f3fa04045919296c102acd7517e` | It is the kernel the board runs (inferred). Its driver, its reserved memory for the R5F and whether the Verdin file enables `mcu_r5fss0` at all are unknown. The host serves an anti-bot page |
| Mainline `v6.18` Verdin AM62P device tree (`k3-am62p-verdin.dtsi`, `k3-am62p-ti-ipc-firmware.dtsi`, `k3-am62p5-verdin-*-dev.dts`) | Only the file names and the top of two `.dts` files were read: models `Toradex Verdin AM62P WB on Verdin Development Board` and `Toradex Verdin AM62P on Verdin Development Board`, compatible `toradex,verdin-am62p`, `ti,am62p5`; they include `k3-am62p5.dtsi`, `k3-am62p-verdin.dtsi` and the wifi or nonwifi and `-dev` files. No R5F node is in those two files. The files that hold the nodes were rate-limited (HTTP 429) |
| Mainline `v6.18` and mainline `v6.6` `ti_k3_r5_remoteproc.c` | Whether mainline has the TI tree's shutdown handshake (`RP_MBOX_SHUTDOWN`, `shut_comp`, the WFI poll) is unknown. The commit-subject list through 2024-06 has no subject that mentions stop, shutdown, wfi or suspend (inferred: those are TI-tree additions) |
| Per-commit history of the driver in the TI tree and in the Toradex tree | The GitHub API refused the list (HTTP 500). The commit that introduced the shutdown mailbox handshake is therefore not named |
| `drivers/base/firmware_loader` (the `firmware_class.path` search path), `rproc_attach`, `__rproc_detach`, `ti_sci` device ops | The firmware search path rules and the exact attach/detach bodies are not read here |
| The Zephyr side: `ram_console` format, the resource table the board image links, its linker script and whether it handles the mailbox | These decide rows C, D, F and the trace buffer's format |
| TI documentation (IPC-only, early boot, DM sharing, memory map) | Web access to the documentation site was refused |

## Source of a statement versus a request

Every row above is tagged. The following are requests for later items, not findings:

- Record `state` before the first start, the `dmesg` lines of `remoteproc` and `k3_r5_rproc` around start and stop, the content of
  `carveout_memories` and `resource_table` in debugfs while the image runs, and `lsmod`/kernel version.
- Read the Verdin R5F nodes from the Toradex tree or from the board's `/proc/device-tree` and compare them with section 4.
- Check that the Zephyr image's link map places every `PT_LOAD` segment inside TCM, MCU SRAM or the firmware carveout, and the trace buffer in the carveout.
