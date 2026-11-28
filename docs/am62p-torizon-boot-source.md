# Verdin AM62P: Torizon OS boot chain and R5F firmware loading (source study)

Desk study for SoW §9 M3 (AM62P portability, WP4). It is read from public
sources only. No device was used and nothing here is `observed`.

How to read the tags. Every statement ends with one of:

- **source** — taken from a public page or repository named next to it, accessed
  2026-11-28. The pages were reached through a web search whose result
  summaries were read. Where the full page was not opened, the tag says
  `(search summary)`.
- **inferred** — reasoned from a source or from general Linux/U-Boot knowledge,
  not stated by a source. The board run must check it.

Limits of this study: the Toradex layers `meta-toradex-ti` and
`meta-toradex-torizon` could not be cloned or browsed in this session, so no
recipe or commit was read. Statements about them are `inferred` or rest on the
Toradex developer pages and community threads listed in "Sources". Community
threads (some answers there come from Toradex's AI assistant) are weaker than
official docs and are marked as such. Re-read the layers at a pinned commit
before relying on any `inferred` row (see "Open points").

## (a) Torizon boot chain on the Verdin AM62P

### a.1 Stages, cores, files

| Stage | File | Runs on | Tag |
|---|---|---|---|
| Boot ROM loads the first stage from the device chosen by the boot-mode pins, into on-chip SRAM (DDR is not up yet) | — | boot ROM | source: U-Boot AM62Px docs, https://docs.u-boot.org/en/latest/board/ti/am62px_sk.html (search summary) |
| First stage: U-Boot R5 SPL packaged by binman | `tiboot3.bin` (Toradex name for the AM62P: `tiboot3-am62px-hs-fs-verdin.bin`) | R5F (the boot-time R5F; on AM62P this is the wakeup/device-manager R5F) | source for file name and R5 build: https://developer.toradex.com/linux-bsp/os-development/build-u-boot-and-linux-kernel-from-source-code/build-u-boot/build-u-boot-for-ti-am62p-modules (search summary); source for R5 SPL: U-Boot AM62Px docs above; the "wakeup R5F" identity is inferred |
| Device Manager (DM, TISCI server: power, clocks, interrupts, DMA) | DM firmware, built into the boot images through binman (`ti-dm` entry, firmware directory passed as `BINMAN_INDIRS`) | R5F; the DM R5F runs the DM task and may run other non-DM code beside it | source: TI E2E FAQ and U-Boot docs via search (search summary); which image carries it on this module is inferred |
| Second stage: TF-A (`bl31`), OP-TEE (`bl32`), A53 SPL and A53 DTBs | `tispl.bin` | A53 (Cortex-A53 cluster) | source: U-Boot AM62Px docs (search summary) |
| U-Boot proper | `u-boot.img` | A53 | source: U-Boot AM62Px docs (search summary) |
| Kernel and device tree | from the OSTree deployment, see a.3 | A53 | source: see a.3 |

Chain order: ROM, R5 SPL (`tiboot3.bin`), TF-A, OP-TEE, A53 SPL, U-Boot, kernel
(**source**: U-Boot AM62Px docs, https://docs.u-boot.org/en/latest/board/ti/am62px_sk.html,
search summary). The AM62P bootflow is stated there to be the same as the other
AM62-family SoCs (**source**, same page).

Toradex builds the AM62P bootloader in two parts: an R5 build giving
`tiboot3-am62px-hs-fs-verdin.bin`, and an A72-labelled build giving `tispl.bin`
and `u-boot.img`, from the `toradex_ti-u-boot-2024.04` branch with the TI Linux
firmware fetched (**source**: Toradex build page above, search summary). The
"A72" label in that page is a Toradex build-target name; the AM62P application
cores are Cortex-A53, so treat the label as inferred to be a naming quirk.

### a.2 Where the files live

- The Verdin AM62 U-Boot doc writes the files to the **eMMC boot partition**
  (`mmc dev 0 1`) at fixed offsets: `tiboot3` at block 0, `tispl.bin` at
  0x400 (1024) and `u-boot.img` at 5120 blocks (**source**: U-Boot Verdin AM62
  doc, https://docs.u-boot.org/en/latest/board/toradex/verdin-am62.html, and a
  Toradex-related DFU config, search summary). The same offsets for the AM62P
  module are **inferred** from the AM62 numbers.
- A community report writes the three files with `dd` to the first eMMC boot
  device at seeks 0, 1024 and 5120 (**source**: TI E2E thread
  https://e2e.ti.com/support/processors-group/processors/f/processors-forum/1427329/am62p-q1-how-to-boot-from-emmc,
  search summary; community report, not an official procedure).
- So the bootloader sits in the eMMC **hardware boot areas** (`mmcblk*boot0` and
  `boot1` in Linux), not on the user-data partitions where the OSTree rootfs
  lives (**inferred**).
- Which boot area is active (boot0 or boot1) is chosen by the eMMC `BOOT_PARTITION_ENABLE`
  setting. Which one this module uses by default is **not known**; the first
  read-only probe should record it (`inferred`, open point).

### a.3 How the kernel and device tree are chosen

- Torizon is an OSTree-based OS. U-Boot runs a boot script (`boot.scr`) that
  reads the device-tree name and path from `/boot/loader/uEnv.txt`
  (**source**: Toradex community thread
  https://community.toradex.com/t/custom-torizon-u-boot-env/20672 and
  https://community.toradex.com/t/select-different-devicetree-at-startup/19957,
  search summary; community posts).
- The default `uEnv.txt` ships in the OS under `/usr/lib/ostree-boot/uEnv.txt`
  (**source**: community thread, same search; community post). The recipe that
  generates the OSTree/extlinux-style boot files is listed in the OpenEmbedded
  layer index as `ostree-uenv-extlinux` (**source**:
  https://layers.openembedded.org/layerindex/recipe/484819/, search summary).
- The base device tree comes from the `fdtfile` default in the
  `meta-toradex-torizon` `uEnv.txt` template, and TorizonCore Builder changes it
  by writing the new file into the boot script (**source**: community threads,
  search summary). The AM62P default name is **inferred** to be the module's
  `k3-am62p5-verdin-*` tree with the carrier-board suffix; read it from the board
  before relying on it.
- Device-tree overlays are enabled by `overlays.txt` (an `fdt_overlays=` line)
  in the OSTree deployment, with the `.dtbo` files beside it (**source**:
  Toradex developer page https://developer.toradex.com/knowledge-base/device-tree-overlays
  and https://developer.toradex.com/torizon/os-customization/torizoncore-builder-tool-commands-manual/,
  search summary). An OS update can reset `overlays.txt` (**source**:
  https://community.toradex.com/t/t-cloud-update-overwrites-overlays-txt/28978;
  community post).
- The U-Boot variable `skip_fdt_overlays=1` bypasses overlay loading (**source**:
  community answer, search summary). This project does not set it (see c.2).
- The kernel image is taken from the booted OSTree deployment under
  `/boot/ostree/...` (**inferred** from the OSTree layout; the layout is not read
  in this session).
- The remote-core overlay `verdin-am62_hmp_overlay.dtbo` reserves memory for the
  M4 and R5 cores on the AM62 (**source**: community thread
  https://community.toradex.com/t/verdin-am62-support-for-ipc/25454, search
  summary; community post). Its AM62P equivalent and whether it is enabled by
  default are **not known**. This overlap with the MCU R5F carve-outs belongs to
  the device-tree study (item am62p-dt-source), not here.

## (b) R5F firmware under Torizon

### b.1 Which firmware files the remoteproc nodes use

- TI's K3 remoteproc driver loads firmware by the name given in the device tree
  (`firmware-name`) from `/lib/firmware`. For the AM62 M4F the file is
  `am62-mcu-m4f0_0-fw` placed in `/lib/firmware` (**source**: Zephyr Verdin AM62
  doc, https://docs.zephyrproject.org/latest/boards/toradex/verdin_am62/doc/index.html,
  search summary; Toradex how-to
  https://developer.toradex.com/software/hmp/hmp-ti/how-to-load-firmwares-ti,
  search summary).
- For the AM62P the matching names follow the same pattern and are
  **inferred**: `am62p-mcu-r5f0_0-fw` (MCU R5F) and `am62p-wkup-r5f0_0-fw`
  (wakeup R5F). Confirm against the `firmware-name` properties and the
  `remoteproc` sysfs `firmware` attribute on the board; the remoteproc driver
  study (item am62p-rproc-source) owns the driver side.
- In Linux the firmware files are usually symlinks into `/lib/firmware/ti-ipc/...`
  (**inferred** from TI SDK images; not confirmed for Torizon).

### b.2 Who boots the R5F

- The **wakeup / DM R5F** is booted early by the boot chain (the SPL starts the
  DM firmware); Linux's R5F driver does not boot it and instead late-attaches to
  the running firmware, and the DM R5F cannot be stopped at run time (**source**:
  TI AM62x IPC docs https://software-dl.ti.com/processor-sdk-linux/esd/AM62X/09_02_01_10/exports/docs/linux/Foundational_Components_IPC62x.html,
  search summary). That page is for the AM62 (non-P); the same on AM62P is
  **inferred**.
- Whether the **MCU R5F** is autobooted by U-Boot or Linux on the Verdin AM62P
  Torizon image is **not stated by any source found**. The Toradex how-to starts
  the M4F by writing `start` to the remoteproc `state` file (**source**: Toradex
  how-to above), so a Linux-started model is the default expectation
  (**inferred**). The board probe must read `state` and `firmware` of each
  remoteproc node before any load.

### b.3 Loading a user firmware from /tmp without writing the rootfs

- Torizon's root filesystem is read-only; a Toradex forum answer says to copy the
  firmware to a writable place such as `/tmp` and give that path (**source**:
  https://community.toradex.com/t/verdin-am62-remoteproc-bad-phdr/25489/2,
  search summary; the answer came from Toradex's AI assistant, weak source).
- **Route 1, remoteproc `firmware` attribute** (**inferred** from the Linux
  remoteproc ABI, https://docs.kernel.org/staging/remoteproc.html, not opened
  in this session): the kernel resolves the string written to
  `/sys/class/remoteproc/remoteprocN/firmware` through the firmware loader, so
  it is a name relative to the firmware search path, not an absolute path. The
  attribute can only be written while the core is stopped (`offline`).
- **Route 2, firmware search path** (**inferred** from
  https://docs.kernel.org/driver-api/firmware/fw_search_path.html, not opened
  in this session): the loader tries `/lib/firmware/updates/<kver>`,
  `/lib/firmware/updates`, `/lib/firmware/<kver>`, `/lib/firmware`, preceded by
  the custom path set by `firmware_class.path=` on the kernel command line or by
  writing `/sys/module/firmware_class/parameters/path` at run time. Pointing that
  parameter at a directory under `/tmp` and putting the image there lets a name
  such as `am62p-mcu-r5f0_0-fw` resolve there with no rootfs write.
- Combination (**inferred**): copy the image to a directory under `/tmp`, set
  the custom path to that directory, write the file name to the `firmware`
  attribute, then write `start` to `state`. After the run, write `stop`, put the
  attribute and the custom path back to the values read before, and remove the
  files.
- Caution (**inferred**): the custom path is global to the kernel, so other
  firmware lookups see it while it is set. It is a run-time setting only and is
  gone after a reboot. Restore it explicitly anyway.
- The remote core load failure `bad phdr` is a known symptom of an ELF whose
  program headers fall outside the declared memory (**source**: community thread
  https://community.toradex.com/t/verdin-am62-remoteproc-bad-phdr/25489, search
  summary; cause **inferred**).

## (c) What "back on the Torizon boot chain" means

- After a board test that loaded a different R5F image through remoteproc, the
  image lived only in RAM and in `/tmp` (b.3). The boot chain on the eMMC boot
  areas and the OSTree deployment were not changed (**inferred**, provided the
  run used only the b.3 route).
- The restore is therefore: undo the run-time changes made (stop the core, put
  back the `firmware` attribute and the custom firmware path, restart anything
  the run stopped, unbind or re-bind what it unbound), then a **plain reboot**
  (`reboot`). The boot ROM, `tiboot3.bin`, `tispl.bin` and `u-boot.img` then run
  again from the same eMMC boot area with the same U-Boot environment, and U-Boot
  boots the same OSTree deployment (**inferred**).
- A power cycle is only needed if a plain reboot does not bring the board back
  to the DM R5F running (**inferred**, since the DM R5F is not stopped by
  Linux, see b.2).

### c.2 What this project forbids

This project's rules forbid, during board work:

- changing the U-Boot environment (no `fw_setenv`, no `setenv`+`saveenv`, no
  `skip_fdt_overlays`, no edits to `uEnv.txt`, `overlays.txt` or `boot.scr`);
- writing `mmcblk0boot0` or `mmcblk0boot1` (no `dd` of `tiboot3.bin`,
  `tispl.bin` or `u-boot.img`, and no change of `force_ro`);
- running `update_uboot`.

So a different boot chain is **never** installed by a session here. "Back on the
Torizon chain" means only the plain reboot in (c) under an unchanged environment
and unchanged boot areas. If the board does not come back after a reboot and
a power cycle, the session reports it and stops; recovery by hand is the owner's
(rule: Verdin AM62P, 2026-11-25).

## (d) Read-only checks that Torizon is up

All of these only read. Run them over ssh with the access the farm supplies; no
address or credential goes into a file.

| Check | Command | Expect | Tag |
|---|---|---|---|
| OS identity | `cat /etc/os-release` | `ID=torizon` (or `torizon-rt`/similar), `NAME`, `VERSION_ID`, `VARIANT_ID`; record them | inferred (Torizon os-release fields, not read) |
| Kernel | `uname -r` | a TI-based `6.x` kernel string; record it | inferred |
| Booted deployment | `ostree admin status` | the first entry carries `*` (booted); record checksum and, if shown, the `(pending)`/`(rollback)` marks | inferred from the OSTree CLI; source for OSTree use in Torizon: https://developer.toradex.com/torizon/os-customization/torizoncore-builder-tool-commands-manual/ (search summary) |
| Booted deployment, alternative if `ostree` needs root | `cat /proc/cmdline` | contains `ostree=/ostree/boot.N/...`, naming the booted deployment | inferred |
| System state | `systemctl is-system-running` | `running` (or `degraded`, record failed units with `systemctl --failed`) | inferred |
| Remoteproc state files | `for d in /sys/class/remoteproc/remoteproc*; do echo "$d $(cat $d/name) $(cat $d/state) $(cat $d/firmware)"; done` | each core `running` or `offline`; the DM R5F is `attached`/`running`, never `offline`, after a good boot | inferred (state values from the remoteproc ABI) |
| Firmware path left clean | `cat /sys/module/firmware_class/parameters/path` | empty (the b.3 setting is reverted) | inferred |
| Uptime since reboot | `uptime -s` | later than the end of the board run | inferred |

A restore is confirmed when the five first rows show Torizon with the same
deployment checksum as before the run and `is-system-running` reports
`running`. The same deployment is compared with the probe's earlier reading
(item am62p-probe). If `ostree admin status` shows a different booted checksum
than before, report it; do not roll back (rollback writes the boot loader
entries, which this project does not do).

## Open points for the next session

1. Pin `meta-toradex-ti` and `meta-toradex-torizon` at a commit and read:
   `conf/machine/verdin-am62p.conf`, the U-Boot recipe, the `uEnv.txt` template,
   the K3 firmware recipe. Then upgrade the `inferred` rows above.
2. Read the AM62P remoteproc nodes on the board (read-only) and fill b.1/b.2.
3. Read the active eMMC boot partition number (read-only `mmc` sysfs entries)
   for a.2, without writing.
4. Confirm that the custom firmware path route and the `firmware` attribute route
   work on the Torizon kernel; the driver study owns the stop/start rules.

## Sources (accessed 2026-11-28; search summaries)

- U-Boot, AM62Px platforms: https://docs.u-boot.org/en/latest/board/ti/am62px_sk.html
- U-Boot, Verdin AM62 module: https://docs.u-boot.org/en/latest/board/toradex/verdin-am62.html
- Toradex, build U-Boot for AM62P modules: https://developer.toradex.com/linux-bsp/os-development/build-u-boot-and-linux-kernel-from-source-code/build-u-boot/build-u-boot-for-ti-am62p-modules
- Toradex, load firmware into alternative cores (TI): https://developer.toradex.com/software/hmp/hmp-ti/how-to-load-firmwares-ti
- Toradex, device tree overlays: https://developer.toradex.com/knowledge-base/device-tree-overlays
- Toradex, TorizonCore Builder manual: https://developer.toradex.com/torizon/os-customization/torizoncore-builder-tool-commands-manual/
- OpenEmbedded layer index, `ostree-uenv-extlinux`: https://layers.openembedded.org/layerindex/recipe/484819/
- Zephyr Verdin AM62 board doc: https://docs.zephyrproject.org/latest/boards/toradex/verdin_am62/doc/index.html
- TI AM62x IPC: https://software-dl.ti.com/processor-sdk-linux/esd/AM62X/09_02_01_10/exports/docs/linux/Foundational_Components_IPC62x.html
- TI E2E, AM62P eMMC boot: https://e2e.ti.com/support/processors-group/processors/f/processors-forum/1427329/am62p-q1-how-to-boot-from-emmc
- Toradex community threads cited inline (community.toradex.com, topics 20672, 19957, 28978, 25454, 25489)
- Linux kernel docs (not opened; inferred rows only): https://docs.kernel.org/staging/remoteproc.html, https://docs.kernel.org/driver-api/firmware/fw_search_path.html
- Repositories not read: `toradex/meta-toradex-ti`, `toradex/meta-toradex-torizon` (no commit pinned)
