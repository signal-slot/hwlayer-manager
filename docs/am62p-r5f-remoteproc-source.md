# AM62P MCU R5F under Linux remoteproc: what the board run needs, from source

Source study for M3 (WP4, [`open-issues.md`](open-issues.md) M3-01, M3-02). It answers four questions for the board run
of `tests/board/am62p_r5f` through the remoteproc route. **Nothing here is observed on a board**; no device was touched.

## Tags and pins

- **source** (function, file, commit): read in the raw text of the file at the commit below, 2026-11-28. The fetch tool returned
  text without line numbers, so statements name the function instead of `file:line`; a line number would have been a guess.
- **inferred**: derived from facts read, or general kernel knowledge not read in the listed files.

| Tree | Pin |
|---|---|
| TI `ti-linux-kernel`, branch `ti-linux-6.6.y` | `a7758da17c2807e5285d6546b6797aae1d34a7d6` (branch tip, used because the Toradex tree is unreachable) |
| Toradex `linux-toradex`, branch `toradex_ti-linux-6.6.y` | tip `9764efd3fe8e5f3fa04045919296c102acd7517e` (`git ls-remote`). **Not read**: `git.toradex.com` answers with an anti-bot page ("Access Denied") |

The Toradex branch is TI's 6.6 kernel plus Toradex board files (inferred from its name). Whether its remoteproc files differ from
TI's is unchecked. Treat every row as a prediction to compare with `dmesg` and `/sys/class/remoteproc` on the board.
Files read: `drivers/remoteproc/ti_k3_r5_remoteproc.c` (the tree has no `ti_k3_common.c` for this driver; the fetch of the
driver file was cut at about 40 000 of 62 000 characters, so rows from its later part are marked), `remoteproc_sysfs.c`,
`remoteproc_debugfs.c`, `remoteproc_elf_loader.c`, and `remoteproc_core.c` for the functions the sysfs files call.

## 1. Identification, and stop/start per mode

| Statement | Tag |
|---|---|
| The class is `remoteproc`; each core is `/sys/class/remoteproc/remoteprocN`. The attributes are `coredump`, `recovery`, `firmware`, `state`, `name` | source: `rproc_class`, `rproc_dev_attrs`, `remoteproc_sysfs.c`, TI `a7758da1` |
| `name` prints `rproc->name`, which `rproc_alloc` copies from the name passed in | source: `name_show`; `rproc_alloc`, `remoteproc_core.c`, TI `a7758da1` |
| The K3 driver passes `dev_name()` of the core's device as that name, and the DT `firmware-name` as the firmware (`rproc_of_parse_firmware`). So `name` is the core's device name, which for the AM62P MCU R5F is the unit address `79000000.r5f` and **not** a word like "mcu-r5f" | source: `k3_r5_cluster_rproc_init` (read from a model-extracted summary of the cut-off part, so less certain), TI `a7758da1`; the address `79000000` is inferred from the node `mcu_r5fss0_core0` at `0x79000000` in the SoC dtsi |
| `remoteprocN` numbering follows probe order, so N is not fixed: pick the core by `name`, not by number, and cross-check with `firmware` | inferred |
| `state` strings: `offline`, `suspended`, `running`, `crashed`, `deleted`, `attached`, `detached`, `invalid`. `state_store` accepts `start`, `stop`, `detach`; anything else logs `Unrecognised option`, `-EINVAL` | source: `rproc_state_string`, `state_show`, `state_store`, TI `a7758da1` |
| **Remoteproc mode** (core powered off when Linux probes): the driver logs `configured R5F for remoteproc mode`; `start` and `stop` are real: `start` loads the firmware and clears the halt bit through TI-SCI, `stop` asks the firmware to shut down by mailbox (waits up to 5 s), checks WFI and halts the core | source: `k3_r5_rproc_configure_mode` (summary), `k3_r5_rproc_start`, `k3_r5_rproc_stop`, TI `a7758da1` |
| `echo stop` in remoteproc mode: state `running` -> `offline` on success. A firmware that never acks costs 5 s and is then halted anyway. A firmware that acks but is not in WFI within about 2 ms fails: `can't stop rproc: -110`, state stays `running` | source: `k3_r5_rproc_stop`, `is_core_in_wfi`, `rproc_stop`, TI `a7758da1`. Details in section 2 of the first study of this item, if merged |
| **IPC-only mode** (core already running when Linux probes: started by the bootloader or a boot stage): the driver logs `configured R5F for IPC-only mode`, state is `detached`, and `prepare`, `start`, `stop` are NULL | source: `k3_r5_rproc_configure_mode` (summary), TI `a7758da1` |
| `echo stop` in IPC-only mode: from `detached` it is refused (`-EINVAL`, `rproc_shutdown` wants `running` or `attached`); after `echo start` (an attach, no firmware loaded) it passes that check and then fails because `ops->stop` is NULL (`-EINVAL`). Only `detach` works and it leaves the core running. The shell shows `Invalid argument` and `dmesg` stays quiet | source: `state_store`, `rproc_shutdown`, `rproc_stop`, `rproc_detach`, TI `a7758da1` (the NULL `stop` is from the summary of the cut-off part) |
| The mode is decided once at probe from the hardware state, not from sysfs. Which mode the Verdin's R5F is in depends on Torizon's boot chain and is **not known**: read `state` and the `dmesg` line before anything else | inferred |
| `firmware` shows `unknown` while the state is `attached` | source: `firmware_show`, TI `a7758da1` |

## 2. Loading a new firmware

| Statement | Tag |
|---|---|
| The file name is the `firmware` attribute. `echo name > firmware` calls `rproc_set_firmware`; it is refused (`can't change firmware while running`, an error return) unless the state is `offline`. A trailing newline from `echo` is stripped by the core | source: `firmware_store`; `rproc_set_firmware`, `remoteproc_core.c`, TI `a7758da1` (the newline strip is inferred; the sysfs attribute is also made read-only if `sysfs_read_only` is set, `rproc_is_visible`) |
| Order for a new image: stop (state `offline`), write `firmware`, write `start`. `start` runs `rproc_boot`, which fetches the file with `request_firmware` and boots | source: `state_store`, `rproc_boot`, `rproc_fw_boot`, TI `a7758da1` (the first two read, the last two inferred from the first study) |
| The name is looked up by the generic firmware loader, which is outside the listed files. Its default search path is `/lib/firmware/updates/<uname -r>`, `/lib/firmware/updates`, `/lib/firmware/<uname -r>`, `/lib/firmware`. The module parameter `firmware_class.path` (kernel command line) adds a first, custom directory; read it with `cat /sys/module/firmware_class/parameters/path` (empty means unset) | inferred (general kernel knowledge; `drivers/base/firmware_loader` is not among the files to read). Verify with the read-only `cat` on the board |
| `/lib/firmware` is usually read-only on Torizon (an OSTree deployment). If the write fails, the custom path above or a bind mount is needed. This is the board item's decision | inferred |
| The ELF check requires a valid ELF magic, ELF32 or ELF64, host endianness and at least one program header (`No loadable segments`); `e_machine` is not checked. A truncated header returns `-EINVAL` | source: `rproc_elf_sanity_check`, `remoteproc_elf_loader.c`, TI `a7758da1` |
| Each `PT_LOAD` segment with non-zero `memsz` is copied to the device address `p_paddr`. It must lie in a region the driver knows (TCM, SRAM, the carveout from the DT); otherwise `bad phdr da 0x... mem 0x...`, `-EINVAL`. `filesz > memsz` or a segment past the file end also fail | source: `rproc_elf_load_segments`, `rproc_da_to_va` (`k3_r5_rproc_da_to_va`), TI `a7758da1` |
| A section named exactly `.resource_table` is **required**: without it `rproc_elf_load_rsc_table` returns `-EINVAL` and the boot stops before any load. The table must have version 1, zero reserved bytes and fit in the file | source: `find_table`, `rproc_elf_load_rsc_table`, TI `a7758da1` |
| A trace entry (`fw_rsc_trace`, type `RSC_TRACE`) is **not required** for booting, but without one no `trace0` exists. `rproc_handle_trace` makes one file per entry, named `trace<index in table>`, from `da` and `len` of the entry; unknown resource types are skipped | source: `rproc_handle_trace`, `rproc_handle_resources`, `remoteproc_core.c`, TI `a7758da1` |
| The entry point is `e_entry` | source: `rproc_elf_get_boot_addr`, `k3_r5_rproc_start`, TI `a7758da1` |

## 3. Where the trace appears, and whether it survives

| Statement | Tag |
|---|---|
| The file is `<debugfs>/remoteproc/remoteprocN/trace0` (`remoteproc` is `KBUILD_MODNAME`, the directory is `dev_name`; the same directory has `name`, `recovery`, `crash`, `resource_table`, `carveout_memories`, `coredump`). Mode 0400: read as root | source: `rproc_init_debugfs`, `rproc_create_debug_dir`, `rproc_create_trace_file`, `remoteproc_debugfs.c`, TI `a7758da1` |
| debugfs must be mounted: `mount -t debugfs none /sys/kernel/debug` if `/sys/kernel/debug/remoteproc` is absent. The `remoteproc` directory exists only if `debugfs_initialized()` was true at init | source: `rproc_init_debugfs`; the mount command is inferred |
| A read takes the live buffer: `rproc_da_to_va(trace->da, trace->len)`; NULL gives the text `Trace traceN not available`, else the length is `strnlen(va, len)`: the text ends at the first NUL byte | source: `rproc_trace_read`, TI `a7758da1` |
| `rproc_handle_trace` runs during the boot, so `trace0` exists from `start` on, also while the firmware runs | source: `rproc_handle_trace`, `rproc_fw_boot`, TI `a7758da1` |
| **The trace does not survive a stop**: `rproc_shutdown` calls `rproc_resource_cleanup`, which removes every `traceN` file and frees its entry. Read `trace0` **before** `stop`, and copy it out | source: `rproc_resource_cleanup`, `rproc_shutdown`, `rproc_remove_trace_file`, TI `a7758da1` |
| **There is no `last_trace` file** in this tree: none of the four files mentions one. After a crash the file stays (the core is `crashed`, resources are not cleaned up until a stop or recovery), but `recovery_disabled` is set, so the driver does not restart the core; read it then | source: `remoteproc_debugfs.c` (no `last_trace`); `k3_r5_cluster_rproc_init` `recovery_disabled = true` from the first study; the post-crash survival is inferred |
| The buffer lives in the firmware's memory, so it also survives the stop in RAM unless the memory is reused; but Linux has no file for it after the stop. Reading it again needs `/dev/mem` or a restart of the same image | inferred |

## 4. What to restore afterwards

| Statement | Tag |
|---|---|
| Before touching anything, record `name`, `state`, `firmware`, the `dmesg` lines `configured R5F for ...-only mode`, and the content of `firmware_class.path`. This is the "original" to restore | inferred (request to the board item) |
| Restore means: stop, write the original name back to `firmware`, and (only if the core was `running` or `attached` at the start) `start`. `firmware` can be written only while `offline` | source: `firmware_store`, `rproc_set_firmware`, `state_store`, TI `a7758da1` |
| If the core was in IPC-only mode (`detached` at the start), Linux cannot restore it: `stop` and `start` of a different image changes nothing the driver can undo, and the original image is not loaded by Linux at all | inferred from section 1 |
| Whatever was stopped or unbound elsewhere (see [`remoteproc-bringup.md`](remoteproc-bringup.md)) is restored in the reverse order | inferred |
| A reboot restores everything in any case: sysfs `firmware` is a RAM string initialised from the DT `firmware-name`; the core's halt state, the carveout contents, debugfs and the mode decision are all rebuilt at the next probe, and the R5F is brought up again by the boot chain. The rebooted board must be Torizon's own chain, confirmed over ssh (owner rule, 2026-11-25) | source: `rproc_alloc` (`rproc->firmware`), `k3_r5_cluster_rproc_init` (DT name), `k3_r5_rproc_configure_mode` (decided at probe), TI `a7758da1`; the boot-chain part is inferred |
| An unbind and bind of the driver after a stop probably fails (the core is halted, its power stays on, `mismatched mode`): do not unbind as a restore step; reboot | inferred from the first study, section 2 row H; unverified |

## Not read

- The Toradex tree itself (anti-bot page); the Toradex device tree for the R5F; `drivers/base/firmware_loader`; TI documents.
- The second half of `ti_k3_r5_remoteproc.c` and of `remoteproc_core.c` as verbatim text; rows from it are marked.
- Line numbers: not available from the fetch tool. Function names stand in for them.
