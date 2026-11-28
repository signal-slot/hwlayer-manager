# How an R5F producer fails and what Linux sees (source study for M3-03)

Status: source read, written 2026-11-28. **This document claims nothing about hardware.** No board was used, no
code is changed, the private showcase was not read. Nothing below is `observed`.

It serves [`open-issues.md`](open-issues.md) M3-03 (safety-layer and producer-failure behaviour under real R5F/DSS
concurrency). Context: [`m3-03-board-test-design.md`](m3-03-board-test-design.md) (scenes 9 to 12),
[`am62p-dss-sharing-source.md`](am62p-dss-sharing-source.md), [`am62p-dss-gobit-source.md`](am62p-dss-gobit-source.md),
[`am62p-r5f-remoteproc-source.md`](am62p-r5f-remoteproc-source.md).

## Tags

- **source**: read in the file and function named, at the pin in the next section, 2026-11-28.
- **inferred**: a conclusion drawn from source lines, or a statement that a file not read would have to confirm.
- **board-only**: the sources do not answer it; it is listed again in the last section.

## Source pins

| Tree | Pin | Read how |
|---|---|---|
| Zephyr `v4.5.0` | commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966` (the tag's target, as in `packed-refs` of the local checkout; the version `docs/zephyr-workspace.md` pins) | read directly from a local checkout, paths relative to its root |
| TI `ti-linux-kernel`, branch `ti-linux-6.6.y` | tip `a7758da17c2807e5285d6546b6797aae1d34a7d6` (the pin of the remoteproc study) | `raw.githubusercontent.com` text at that commit, fetched through a helper agent that returned the code verbatim; no line numbers, functions are named. The helper's own summaries of two functions were wrong once and were replaced by the verbatim text, so the text, not the summary, is what was used |

**Not read (limits).** The Toradex BSP kernel that runs on the board (`toradex_ti-linux-6.6.y`; the host refuses
fetches, see the remoteproc study). Everything about Linux below is the TI 6.6 tree, **not** the board's kernel. The
AM62P TRM, the TISCI documentation, the Linux `rti_wdt.c` watchdog driver, the AM62P device tree (the carve-outs and
the MCU R5F's RTI node), Zephyr `mcu_r5f0_0` support for the AM62P (there is none upstream, see
[`am62p-zephyr-upstream-source.md`](am62p-zephyr-upstream-source.md)), and the board test's own `hw_am62p.c` startup,
linker script and resource table (the table comes from the showcase and is not read).

## What the board test image is, for this study

- **source**: `tests/board/am62p_r5f/prj.conf` sets `CONFIG_WATCHDOG=n`, `CONFIG_PRINTK=y`, `CONFIG_RAM_CONSOLE=y`,
  `CONFIG_SERIAL=n`. It does not set `CONFIG_RESET_ON_FATAL_ERROR`, `CONFIG_TASK_WDT` or `CONFIG_LOG`.
- **source**: `tests/board/am62p_r5f/src/main.c`: `main()` runs the scenes and ends in `for (;;) { k_sleep(K_FOREVER); }`.
  The scenes run on the main thread; there is no second producer thread and no mailbox code (grep for
  `mbox`/`mailbox` in `src/` finds none).
- **inferred**: so every failure below is a fault of the one thread that also presents, or of the core as a whole. A
  producer that stalls inside a scene is the same thread as the presenter. Scenes 9 to 12 (the design) would add a
  producer thread; that is not built yet.

## 1. A fatal fault in Zephyr on a Cortex-R5

Path for a data abort, prefetch abort or undefined instruction:

| Step | Statement | Tag |
|---|---|---|
| 1 | The vectors `z_arm_data_abort`, `z_arm_prefetch_abort`, `z_arm_undef_instruction` save a frame on the abort/undefined-mode stack (not the thread stack) and call `z_arm_fault_data`, `z_arm_fault_prefetch`, `z_arm_fault_undef_instruction`. The file comment says so: an arbitrary thread stack overflow cannot cause exception handling to fail | source: `arch/arm/core/cortex_a_r/exc.S` (header comment, `exception_entry`) |
| 2 | Each handler prints `***** DATA ABORT *****` (or `PREFETCH ABORT`, `UNDEFINED INSTRUCTION ABORT`) through `EXCEPTION_DUMP`, then calls `z_arm_fatal_error`. The comments say "All data aborts are treated as fatal for now", likewise prefetch and undefined. The one recoverable data-abort case is under `CONFIG_USERSPACE` (`memory_fault_recoverable`) and under `CONFIG_GDBSTUB` | source: `arch/arm/core/cortex_a_r/fault.c` |
| 3 | The fault status text (`Synchronous External Abort @ 0x...`, and so on) is printed only with `CONFIG_FAULT_DUMP == 2` (`FAULT_DUMP_VERBOSE`) | source: `fault.c` (`dump_fault`) |
| 4 | `z_arm_fatal_error` dumps the exception frame under `CONFIG_EXCEPTION_DEBUG` (default `y`, depends on `PRINTK \|\| LOG`), then calls `z_fatal_error` | source: `arch/arm/core/fatal.c`, `arch/Kconfig` |
| 5 | `z_fatal_error` locks interrupts (`arch_irq_lock`), calls `printk_panic()`, prints `>>> ZEPHYR FATAL ERROR %d: %s on CPU %d`, the current thread's name, then calls `coredump` and `k_sys_fatal_error_handler` | source: `kernel/fatal.c` |
| 6 | Without logging, `EXCEPTION_DUMP` is `printk_panic(); ...; printk(...)`. So with `CONFIG_PRINTK=y` and the RAM console the fatal text goes to the same RAM console as the `SCENE`/`RESULT` lines | source: `include/zephyr/arch/exception.h`, `drivers/console/ram_console.c` (`__printk_hook_install`) |
| 7 | The default `k_sys_fatal_error_handler` (weak) prints `Halting system` and calls `arch_system_halt`, or, with `CONFIG_RESET_ON_FATAL_ERROR`, `Resetting system` and `sys_reboot(SYS_REBOOT_WARM)` | source: `kernel/fatal.c`, `lib/os/Kconfig` |
| 8 | The weak `arch_system_halt` calls `arch_irq_lock()` and then spins in `for (;;)`. It is **not** a wait-for-interrupt: the core keeps executing the loop | source: `kernel/fatal.c`; inferred: not WFI because the loop body is empty |
| 9 | If an application's own `k_sys_fatal_error_handler` returns, `z_fatal_error` unlocks interrupts and calls `k_thread_abort` on the faulting thread: the thread dies and the rest of the system (timers, interrupt handlers, other threads) keeps running. The comment says "a policy decision was made not to hang the system" | source: `kernel/fatal.c` (end of `z_fatal_error`) |
| 10 | The board test image defines no `k_sys_fatal_error_handler` (grep of `tests/board/am62p_r5f` for `fatal` finds nothing) | source: that grep. So the default applies: halt in a spin loop |

Consequences:

- **inferred**: after a fatal fault the R5F stops running the engine and never returns to `main`. Interrupts are
  masked, so the vsync callback (the backend's GO-bit latch counting) stops too. The last text in the RAM console
  is the fault report, if the console buffer is still readable (section 5).
- **inferred**: a fault that corrupts the console buffer, or one inside `printk` itself, can leave no text. Zephyr
  says as much in `z_fatal_error` (`printk_panic` is used because the print lock "is not trustworthy").
- **source**: Zephyr's `arch_system_halt` default does not tell Linux anything. The crash message to Linux,
  `RP_MBOX_CRASH`, is not sent by this code (no mailbox use in the files read). Nothing in the Cortex-A/R fault
  path posts to a mailbox.

## 2. A hung thread, and a stalled producer

- **source**: Zephyr's scheduler and timer interrupt are independent of one thread. A thread that loops forever
  without yielding stops lower- or equal-priority threads (the main thread in the board test is a preemptible
  thread; other threads exist only if the app creates them). A thread that blocks (`k_sleep`, a mutex, a
  semaphore wait) leaves the core to idle via `arch_cpu_idle` (`arch/arm/core/cortex_a_r/cpu_idle.c`).
- **inferred**: a *stalled* producer, one that simply stops calling `ce_present` (design scene 9 (c)), is not a
  fault at all. The core keeps running, the backend's vsync callback keeps running, and the engine reaches
  `CE_EVENT_LATCH_TIMEOUT` only if the app calls `ce_advance_time` (the engine has no clock;
  [`m3-03-board-test-design.md`](m3-03-board-test-design.md)).
- **inferred**: a producer that spins inside the same thread as the presenter (the board test as it is) also stops
  the presenter, so no new commit reaches the DSS. A thread-context busy loop does not starve the vsync callback,
  which is an interrupt handler (the design doc says so; the handler is in `backends/twoplane`, not read again here);
  a loop with interrupts masked, or in interrupt context, would.
- **source**: the image has no detection of a hung or stalled thread today: `CONFIG_WATCHDOG=n`, no `CONFIG_TASK_WDT`.
  `subsys/task_wdt/task_wdt.c` lets each thread register a channel with a timeout and a callback; with
  `CONFIG_TASK_WDT_HW_FALLBACK` a hardware watchdog is fed from the task-watchdog timer. A thread that stops
  feeding its channel gets its callback called, from a kernel timer callback
  (`task_wdt_trigger`, `channels[channel_id].callback(...)`). The callback's own code decides what follows. The
  part of the file after the callback (the default action when the callback is NULL) was not read: **not read**.
- **source**: the K3 RTI watchdog driver `drivers/watchdog/wdt_ti_rti.c` exists upstream (`ti,j7-rti-wdt`,
  `WDT_TI_RTI`, "Copyright 2025 Ayush Singh BeagleBoard.org Foundation"). It offers two modes: `WDT_FLAG_RESET_SOC` writes
  `RTIWWDRX_RESET` and needs the node property `reset-capable`; the other writes `RTIWWDRX_NMI` and needs an
  `interrupts` entry. `wdt_ti_rti_timeout` returns `-ENOTSUP` if the requested mode is not declared in the
  device tree. The driver's ISR feeds the watchdog again after calling the callback (`wdt_ti_rti_isr`). Window sizes are
  6 steps from 100% to 3.125%.
- **source**: the only DT node of this driver for an R5F in the tree read is `mcu_rti0` at `0x04880000` in
  `dts/arm/ti/j722s_mcu.dtsi`, `status = "disabled"`, no `reset-capable`, no interrupts. J722S is a **sibling** of
  the AM62P; the AM62P MCU RTI is **not** read. **board-only / not read**: which RTI instance the AM62P MCU R5F
  has, whether a reset reaches the R5F or the whole SoC, and whether TI System Firmware or Linux already owns that
  RTI. `WDT_FLAG_RESET_SOC` in the driver name says "SoC"; what the RTI reset resets on the AM62P is not in the
  files read.

## 3. A reboot of the core

Two different things are meant by "reboot of the core"; the sources separate them.

**3a. Zephyr resets itself (`sys_reboot`).**

- **source**: `lib/os/reboot.c`: `sys_reboot` locks interrupts, disables caches (`CONFIG_ARCH_CACHE`), disables the
  system timer if supported, then calls `sys_arch_reboot(type)`, and if that returns prints
  `Failed to reboot: spinning endlessly...` and loops in `k_cpu_idle`.
- **source**: `arch/arm/core/cortex_a_r/reboot.c` defines `sys_arch_reboot` as `__weak` with an empty body (the
  argument is unused). A `grep` of `soc/ti`, `boards`, `arch/arm` and `drivers` for `sys_arch_reboot` in the
  Zephyr tree found no TI K3 definition (`soc/ti/k3/common/cortex_r/soc.c` has none).
- **inferred**: with the K3 Cortex-R SoC code in this tree, `CONFIG_RESET_ON_FATAL_ERROR=y` or a direct
  `sys_reboot()` ends in the `Failed to reboot: spinning endlessly` message and an idle loop, not in a reset,
  unless the (not yet existing) AM62P port defines `sys_arch_reboot`. Whether a K3 reset can be requested from
  the R5F itself (a reset-request through TISCI or an RTI) is **not read**.

**3b. Linux restarts the core (remoteproc).**

- **source**: `k3_r5_rproc_start` (TI `a7758da1`) programs the boot address through `ti_sci_proc_set_config`
  and runs the core with `k3_r5_core_run`. `rproc_start` (core) first calls `rproc_load_segments` and so
  `rproc_elf_load_segments`, which copies each `PT_LOAD` segment into the memory resolved by `rproc_da_to_va` and
  zero-fills `memsz - filesz` of each segment.
- **inferred**: a restart through Linux therefore rewrites the image's own segments (and zeroes its `.bss`-like
  tail). Memory outside the image's `PT_LOAD` segments is not written by these two functions. Whether the display
  buffers lie inside a segment of the image (and so are zeroed) or outside is a property of the board test's linker
  script and is **not read**: **board-only**.
- **source**: `rproc_alloc_carveout` uses `dma_alloc_coherent` for a carveout in the generic path. The K3
  driver replaces the carveouts with its static memory-region handling (see
  [`am62p-r5f-remoteproc-source.md`](am62p-r5f-remoteproc-source.md) section 3, `k3_r5_reserved_mem_init`). Whether
  the generic allocator path zeroes the carveout that the board uses was **not established** here.
- **source**: `rproc_resource_cleanup` (called from `rproc_shutdown`) removes the `traceN` debugfs entries,
  frees carveouts through their release callbacks and unmaps. After a stop the trace file is gone; a new start
  creates it again (consistent with the remoteproc study).

## 4. What Linux sees (TI 6.6 tree)

| Event | What Linux does or shows | Tag |
|---|---|---|
| The firmware sends `RP_MBOX_CRASH` | `k3_r5_rproc_mbox_callback` logs `K3 R5F rproc <name> crashed` with `dev_err` and does nothing else. The comment: "remoteproc detected an exception, but error recovery is not supported. So, just log this for now" | source: `ti_k3_r5_remoteproc.c`, `k3_r5_rproc_mbox_callback` |
| Any crash notification | The K3 driver never calls `rproc_report_crash`: the file has no such call and no hit for `watchdog`, `WDT` or `wdt` | source: whole-file read for those words, `ti_k3_r5_remoteproc.c` |
| If `rproc_report_crash` were called | It logs `crash detected in <name>: type <watchdog / fatal error / mmufault>`, queues `rproc_crash_handler_work`; that sets the state to `RPROC_CRASHED` (state string `crashed`) and, if `recovery_disabled` is false, calls `rproc_trigger_recovery` | source: `remoteproc_core.c`, `rproc_report_crash`, `rproc_crash_handler_work`, `rproc_crash_names` |
| Recovery flag | `recovery_disabled = true` for every K3 R5F (`/* error recovery is not supported at present */`) | source: `k3_r5_cluster_rproc_init` |
| Hand-made crash | debugfs file `crash` (mode 0200) calls `rproc_report_crash(rproc, type)` with a number the writer gives. This is the one way to reach the crashed state in this tree without a driver change | source: `remoteproc_debugfs.c`, `rproc_crash_write`. **inferred**: it marks the Linux state only; it does not touch the core |
| Recovery by hand | `recovery` (sysfs and debugfs) accepts `enabled`, `disabled`, `recover`. `recover` runs `rproc_trigger_recovery` without changing the flag. `enabled` sets the flag false and also triggers recovery | source: `remoteproc_sysfs.c`, `recovery_store` |
| What recovery does | `rproc_boot_recovery`: `rproc_stop(rproc, true)` (so `k3_r5_rproc_stop`), a coredump (`rproc->ops->coredump`, default `rproc_coredump`, since the K3 ops table has no `.coredump`), `request_firmware`, then `rproc_start`. A K3 stop with a silent firmware logs the 5 s timeout and halts the core anyway ([`am62p-r5f-remoteproc-source.md`](am62p-r5f-remoteproc-source.md) section 1, row C) | source: `rproc_trigger_recovery`, `rproc_boot_recovery`, `k3_r5_rproc_stop`. **inferred**: reaching the stop requires state `crashed` first: `rproc_trigger_recovery` returns unless the state is `RPROC_CRASHED` |
| Stop of a crashed state | `rproc_shutdown` (the sysfs `stop` path) refuses with `-EINVAL` unless the state is `RPROC_RUNNING` or `RPROC_ATTACHED`; a `crashed` rproc cannot be stopped by `echo stop` | source: `rproc_shutdown`. **inferred**: so a crashed state is left only by `recovery` |
| Kernel panic | `rproc_panic_handler` calls `ops->panic` for running rprocs; the K3 ops table has no `.panic` | source: `rproc_panic_handler`, `k3_r5_rproc_ops` |
| A core that halted, hung or faulted without sending anything | Linux receives nothing. The state stays `running`, `recovery` stays `disabled`, `dmesg` shows nothing new. No driver code polls the core's liveness (no heartbeat, no watchdog use in the K3 driver or the core file) | **inferred** from the absence of any such code in `ti_k3_r5_remoteproc.c` and `remoteproc_core.c` as read; the "read for these words" is the evidence |
| `state` stays `running` after a halt | A stop request then goes to `k3_r5_rproc_stop`: shutdown message, 5 s wait, `is_core_in_wfi` poll (200 us step, 2000 us total) only if an ack came. A fault-halted core spins in `arch_system_halt`, so it is **not** in WFI; but it also sends no ack. With no ack the code logs the timeout and halts the core (success) | source for the driver; **inferred** for the combination with `arch_system_halt` (rows 8 of section 1 and C in the remoteproc study) |
| What the trace shows | `trace0` read: `rproc_trace_read` returns the buffer text up to the first NUL, every time it is read, straight from the carve-out through `rproc_da_to_va`. After a fatal fault the text printed by `EXCEPTION_DUMP` is visible if it was written in the buffer. The RAM console writes `pos = (pos + 1) % (SIZE - 1)`: it wraps (the old text is overwritten from the start of the buffer), and the last byte stays NUL | source: `remoteproc_debugfs.c`, `drivers/console/ram_console.c`. **inferred**: after a wrap, the text shown starts mid-stream, and the newest text overwrites the oldest from byte 0 |
| Trace content across a core reboot | `rproc_resource_cleanup` removes the file at stop; the RAM console variable is initialised by the new run (`ram_console_init`) but the buffer is not cleared in the code read, so old text beyond the new `pos` can remain after a restart | source for the init code (`ram_console.c`: no memset); **inferred** for the leftover. The ELF load zero-fills only the segments' `memsz`; whether the console buffer is inside a segment is **not read**, see 3b |

## 5. What the DSS keeps scanning out

All of this is **inferred** from the GO-bit and sharing studies, because no source read describes the DSS's
behaviour when the one who programs it stops. Nothing here is a hardware result.

- **source** ([`am62p-dss-gobit-source.md`](am62p-dss-gobit-source.md)): a plane's registers are shadowed; a new
  configuration takes effect only when the GO bit of the plane's video port is set and the hardware copies shadow
  to work registers at the frame start, after which GO reads 0 (AM67 TRM snippet, a sibling device).
- **inferred**: with no further GO, the work registers keep the last copied configuration. The display subsystem
  then keeps fetching the plane's buffer address from that configuration every frame. The panel shows the content
  of that buffer **as it is in memory at each fetch**, not a frozen copy. So the answer to "last latched frame or
  not" is: *the last latched configuration, read again from memory each frame*. If nothing writes the buffer, the
  picture is the last latched frame; if something writes it (Linux, a restarted image, a DMA), the panel shows that.
- **inferred**: a fault of the R5F therefore does not stop scanout by itself. The DSS is not a part of the R5F
  and the sources read give no reset link between the R5F and the DSS registers. What does stop or change
  it: the video port being disabled (by whoever owns it), a power or clock action by System Firmware through a
  TISCI request (not read), or buffer content changing.
- **inferred**: an *incomplete* update is the risky case: a fault between two plane-register writes of one
  commit, before the GO write, leaves shadow registers written but not latched; they do not reach the work
  registers until a later GO from whoever programs the DSS next. Whether the hardware latches the shadow set on its
  own is not claimed by any source read (the gobit study: "No source read answers this" for writes around GO).
- **inferred**: the safety layer's purpose is that the safety plane stays visible. The R5F's death does not remove a
  plane that was enabled and latched; it also cannot flip to the safety plane after the death, because the flip is
  an R5F write plus a GO. So with the backend as designed, a producer failure **before** the core dies (a stall)
  can be answered by a flip; a **core** failure cannot be answered by the R5F. Linux tidss in shared mode does
  not own those registers ([`am62p-dss-sharing-source.md`](am62p-dss-sharing-source.md), `ti,dss-shared-mode-vp-owned`)
  and the Verdin kernel's use of shared mode is unknown.
- **source** (TI `a7758da1`): halting the core by `k3_r5_core_halt` and unpreparing it (`k3_r5_split_reset`) act on
  the R5F cluster through TISCI and the module reset. No function read touches the DSS. **inferred**: so a stop
  by Linux also leaves the DSS configuration as the R5F left it.
- **source**: a restart through Linux (section 3b) rewrites the image's segments in memory but does not
  touch the DSS registers before the new core runs. **inferred**: the new image's first `ce_*` call may
  find DSS registers in the previous run's state; whether `hw_am62p.c` initialises them every time is in
  the board test code, not read in this study.

## 6. Summary table of the four failure modes

| Mode | What the core does | What Linux sees (TI 6.6) | What the DSS scans | Tag |
|---|---|---|---|---|
| Fatal fault (data/prefetch/undef abort) | Prints the dump to `printk` (RAM console), locks interrupts, spins in `arch_system_halt` (image has no handler of its own). No mailbox message | `state` `running`, no log line, `trace0` ends in the fault text | last latched configuration, memory read each frame | source for the core side; inferred for the rest |
| Hung thread (busy loop in thread context) | Interrupts and timer keep running; other threads of lower priority starve; no new `ce_present` | nothing; `state` `running` | last latched configuration; the vsync callback keeps counting `vsyncs` | inferred |
| Stalled producer (stops committing) | Core healthy, idle in WFI via `arch_cpu_idle`; engine latch timeout only if the app advances time | nothing; trace shows whatever the app prints | last latched configuration | inferred |
| Core reboot | `sys_reboot` has no K3 backend in the tree read (ends in a spin message). A Linux-driven restart reloads the image (section 3b) | `stop`/`start` log lines (`stopped remote processor`, `remote processor ... is now up`); trace file removed and recreated | no change from the restart itself; content changes if the new image writes the buffer or the DSS | source for the Linux and reboot code; inferred for the DSS |

## What only the board can answer

1. Whether the Zephyr image in the board test, after a forced fault (for example a store to an unmapped address), writes
   the fault text to `trace0` and whether it stays readable. Whether the board's resource table and the linker script
   put the RAM console where `rproc_da_to_va` can find it after the fault.
2. Whether the panel keeps showing the last latched frame after the R5F spins in `arch_system_halt`, and for how
   long (also under `echo stop`, a Linux-driven restart, and a full image reload).
3. What Linux prints and what `state` reads after each of the four modes, and whether the Toradex kernel differs
   from TI `a7758da1` (`recovery_disabled`, the `RP_MBOX_CRASH` handling, `ti_sci_proc` calls).
4. Whether the DSS shows tearing, SYNC LOST or an underflow when the R5F dies between two plane-register writes
   (needs the DSS interrupt status; the existing register-dump tool can read it).
5. Which RTI watchdog the AM62P MCU R5F has, whether Linux or System Firmware already owns it, what its reset
   resets, and whether a Zephyr image can use `ti,j7-rti-wdt` on it without a device-tree change.
6. Whether a K3 R5F can reset itself (`sys_arch_reboot`), and what it takes (a TISCI request, the RTI, or none).
7. Whether the display buffers and the console buffer lie inside a `PT_LOAD` segment of the image (zeroed on every
   Linux load) or in carved-out memory that is left untouched.
8. Whether a stop after a fatal fault takes the 5 s timeout path and succeeds, as the source suggests, or fails.
9. Whether the DSS registers keep the state of an earlier run when the next image starts, and what `hw_am62p.c`
   does at start (it was not read here).

## Next steps for the board items

- **inferred**: scene or manual steps that force a fault must run last in a session: a core that spun in
  `arch_system_halt` cannot be recovered by Linux except by a stop (about 5 s) and, per the remoteproc study, a
  second start needs a reboot.
- **inferred**: before forcing a fault, set `CONFIG_FAULT_DUMP=2` and `CONFIG_EXCEPTION_DEBUG=y` in the image to get
  the fault status in the trace, and keep the console buffer small so it does not wrap.
- Nothing in this document changes code or the repository's other documents.
