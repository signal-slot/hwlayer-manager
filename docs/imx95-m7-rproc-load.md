# i.MX95 M7: how Linux `imx_rproc` loads an ELF, checked against Zephyr's M7 layout

Source study, 2026-11-28. Board in scope: Toradex Verdin iMX95 on Mallow,
Reference Multimedia Image 7.8.0, kernel 6.6.157-7.8.0. Feeds the gate of the
M2 item that loads a Zephyr image on the M7 through remoteproc (`m7-run`).

**No board was accessed, nothing was built, and nothing here claims that a load
works on hardware.** Every finding carries one tag:

- **read from source**: the pinned file says so, at the line given.
- **inferred**: my deduction from what was read; no source states it.

The only board facts used are quoted from the read-only snapshot
[`imx95-zephyr/observed-2026-11-27.txt`](imx95-zephyr/observed-2026-11-27.txt)
and keep its tag (observed 2026-11-27).

Already covered elsewhere, linked rather than repeated:

- Who may start and stop the M7 under the System Manager (TF-A SiP →
  SCMI CPU protocol, `mx95evk` vs `mx95evkrpmsg`), and the remoteproc gate:
  [`imx95-zephyr-feasibility.md`](imx95-zephyr-feasibility.md#how-the-m7-is-started)
  and its [gate section](imx95-zephyr-feasibility.md#gate-can-an-m7-image-be-started-from-linux-remoteproc-with-files-only-under-tmp).
- The Verdin reserved-memory map, the `remoteproc-cm7` node, Toradex's own
  remoteproc instructions and user reports:
  [`imx95-m7-verdin-bsp.md`](imx95-m7-verdin-bsp.md) sections 2 and 3.
- Whether the display could go to the M7's logical machine:
  [`imx95-sm-display-lm.md`](imx95-sm-display-lm.md).
- Where a framebuffer may live relative to Linux's carve-outs, and the
  `…/m7/ddr` vs CMA overlap: [`imx95-m7-fb-memory.md`](imx95-m7-fb-memory.md)
  section 7.
- The host-tested load/stop/restore script:
  [`../scripts/imx95-m7-rproc.sh`](../scripts/imx95-m7-rproc.sh).

## Sources

| # | What | Pin | How read |
| --- | --- | --- | --- |
| L1 | NXP `linux-imx`, `drivers/remoteproc/imx_rproc.c`, `imx_rproc.h` | tag `lf-6.6.52-2.2.2` (tag object `f9a8e33f5cde3e96a57569e9dfa70481fe688943`, commit `5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc`, also the head of branch `lf-6.6.y` on 2026-11-28); https://github.com/nxp-imx/linux-imx/blob/lf-6.6.52-2.2.2/drivers/remoteproc/imx_rproc.c | raw file over HTTPS through a fetch helper |
| L2 | Mainline Linux `drivers/remoteproc/remoteproc_elf_loader.c`, `remoteproc_core.c`, `remoteproc_sysfs.c`, `drivers/base/firmware_loader/main.c` | tag `v6.6`; https://github.com/torvalds/linux/tree/v6.6/drivers/remoteproc | same |
| L3 | Mainline Linux `drivers/remoteproc/imx_rproc.c` (contrast only) | tag `v7.3`; https://github.com/torvalds/linux/blob/v7.3/drivers/remoteproc/imx_rproc.c | same |
| Z1 | Zephyr | tag `v4.5.0` = commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966`; https://github.com/zephyrproject-rtos/zephyr/tree/v4.5.0 | local checkout at that commit; line numbers exact |
| Z2 | Zephyr west modules `open-amp` / `libmetal` | revisions `01032a8a7bdfdd541686f5dfa3671e602b9fcbff` / `77237e9748212796a8f93a33689ce935fb9c81c2` (Z1 `west.yml:361-363` / `west.yml:305-307`) | pins only; module code not read |

**Which kernel.** The board runs Toradex's `toradex_6.6-2.2.x-imx` tree, which
could not be read (see
[`imx95-zephyr-feasibility.md` S9](imx95-zephyr-feasibility.md#sources)). L1
is the newest NXP `lf-6.6` release tag; it is the same revision as S6 of the
feasibility doc. I found no NXP tag named after 6.6.157 or 7.8.0. That the BSP
driver equals L1 is **inferred**; it is consistent with two observed facts: the
remoteproc is called `imx-rproc` (L1 passes that fixed name to `rproc_alloc`;
L3 uses the DT node name, which would be `remoteproc-cm7`) and dmesg shows no
"Using CPU/LMM Protocol OPS" line (L3 prints one at probe).

**Line numbers for L1-L3** were counted by the fetch helper from raw text
without numbering; they may be off by a few lines. The function names are the
reliable anchor. Z1 line numbers come from the local checkout and are exact.

## (a) Linux `imx_rproc` on the i.MX95 (L1, `lf-6.6.52-2.2.2`)

### dcfg, method and address translation table

| Finding | File:line (L1) | Tag |
| --- | --- | --- |
| `fsl,imx95-cm7` → `imx_rproc_cfg_imx95_m7` | `imx_rproc.c:1334` | read from source |
| The dcfg has only `.att`, `.att_size` and `.method = IMX_RPROC_SMC`; there is no `cpuid`, `lmid` or `ops` field in this version | `imx_rproc.c:387-391`, `imx_rproc.h:29-39` | read from source |
| Methods are `NONE`, `MMIO`, `SMC`, `SCU_API`; no SCMI/LMM method exists in L1 | `imx_rproc.h:19-27` | read from source |
| ATT: da `0x00000000` → sys `0x203C0000`, 256 KiB, `ATT_OWN \| ATT_IOMEM` (TCM code); da `0x20000000` → sys `0x20400000`, 256 KiB, `ATT_OWN \| ATT_IOMEM` (TCM sys); da `0x80000000` → `0x80000000`, `0x50000000`, flags 0 (DDR) | `imx_rproc.c:130-140` | read from source |
| `ATT_OWN` = bit 31 ("can be mapped at probe"), `ATT_IOMEM` = bit 30 | `imx_rproc.c:90-96` | read from source |

The table is the same in L3 (`imx_rproc.c:136-146`, read from source) and in
the v6 patch quoted in
[`imx95-m7-verdin-bsp.md` 2.1](imx95-m7-verdin-bsp.md#21-address-translation-table-in-the-linux-driver).

### Which addresses a segment may use

| Finding | File:line (L1) | Tag |
| --- | --- | --- |
| `imx_rproc_da_to_sys()` accepts `da` only if `da >= att->da && da + len < att->da + att->size`: the end test is **strict**, so a segment that ends exactly at the end of a window is rejected ("Translation failed") | `imx_rproc.c:485-519`, test at `:506` | read from source |
| `imx_rproc_da_to_va()` then needs the system address inside one of `priv->mem[]`, again with a strict end test | `imx_rproc.c:521-552`, test at `:539-540` | read from source |
| `imx_rproc_addr_init()` fills `priv->mem[]` with the `ATT_OWN` entries (the two TCM windows, `devm_ioremap`) and then with every `memory-region` of the node whose name does not start with `vdev` (`devm_ioremap_wc`) | `imx_rproc.c:765-844`, skip at `:812` | read from source |
| The DDR ATT entry has no `ATT_OWN`, so DDR is not mapped by the ATT; a DDR address resolves only if a non-`vdev` memory-region covers it | `imx_rproc.c:777-778`, `:798-841` | read from source |
| On the board those memory-regions would be `memory@80000000` (16 MiB, `no-map`) and `rsc-table@88220000` (4 KiB); the `vdev*` ones are skipped | snapshot (b) reserved-memory | observed 2026-11-27 (names of the regions in the node's list inferred: the snapshot has only phandle numbers) |
| So the windows an ELF segment may load into on this board are: M7 `0x00000000-0x0003FFFF` (ITCM), `0x20000000-0x2003FFFF` (DTCM), `0x80000000-0x80FFFFFF` (`m7_reserved`), and the 4 KiB at `0x88220000`; each minus its last byte because of the strict test | from the rows above | inferred |
| Toradex user report "bad phdr da 0x90000000" ([verdin-bsp §3](imx95-m7-verdin-bsp.md#3-resource-table-and-rpmsg-vrings-versus-the-dt)) fits this: `0x90000000` passes the DDR ATT entry but lies in no mapped region | | inferred |

### Start, stop and detection

| Finding | File:line (L1) | Tag |
| --- | --- | --- |
| Start: `imx_rproc_xtr_mbox_init()`, then for SMC `clk_prepare_enable(priv->clk_audio)` and `arm_smccc_smc(IMX_SIP_RPROC, IMX_SIP_RPROC_START, rproc->bootaddr, …)`; `fsl,startup-delay-ms` is slept after it | `imx_rproc.c:393-438`, SMC at `:416-423` | read from source |
| Stop: `arm_smccc_smc(IMX_SIP_RPROC, IMX_SIP_RPROC_STOP, …)`; `res.a1` set means "Not in wfi, force stopped" | `imx_rproc.c:440-483`, SMC at `:463-469` | read from source |
| `IMX_SIP_RPROC` = `0xC2000005`, `START` 0, `STARTED` 1, `STOP` 2 | `imx_rproc.c:71-74` | read from source |
| Probe asks `IMX_SIP_RPROC_STARTED`; non-zero → `RPROC_DETACHED` (Linux attaches, loads nothing) | `imx_rproc.c:1034-1053` | read from source |
| What TF-A does with the SiP call (SCMI CPU protocol `set_reset_addr`/`start`/`stop` on M7P, as the secure AP agent) and which SM configs allow it | [feasibility, "How the M7 is started"](imx95-zephyr-feasibility.md#how-the-m7-is-started) | read from source there (imx-atf), not re-read |
| `fsl,startup-delay-ms` is `0x32` (50 ms) and `power-domains` is `<0x5c 0x11>` on the board's `remoteproc-cm7` node | snapshot (b2) | observed 2026-11-27 |
| Domain `0x11` = 17 = `IMX95_PD_M7` in Zephyr's ID table (Z1 `include/zephyr/dt-bindings/power/imx95_power.h:27`); that Linux uses the same SM domain number, so the node holds the M7 power domain on while the device is bound (runtime PM "unsupported" in the snapshot), is | | inferred |

### Boot address

| Finding | File:line | Tag |
| --- | --- | --- |
| L1's `imx_rproc_get_boot_addr()`: for `fsl,imx95-cm7` (and `imx93-cm33`) it returns the `sh_addr` of the ELF **section named `.interrupts`**, and **0** if there is none; other SoCs use the ELF entry | L1 `imx_rproc.c:711-747` | read from source |
| That value is `rproc->bootaddr`, passed as `x2` of the START SiP call | L1 `imx_rproc.c:420`; L2 `remoteproc_core.c:1405` | read from source |
| For contrast, L3 uses `rproc_elf_get_boot_addr` (ELF `e_entry`) masked with `GENMASK_U32(31, 16)` | L3 `imx_rproc.c:377-391`, `:1516`, `:780` | read from source |

### Resource table

| Finding | File:line | Tag |
| --- | --- | --- |
| `imx_rproc_parse_fw()` calls `rproc_elf_load_rsc_table()` and, if that fails, only logs "No resource table in elf" and **returns 0**: a table-less ELF is accepted | L1 `imx_rproc.c:633-642` | read from source |
| The generic `rproc_elf_load_rsc_table()` returns `-EINVAL` when there is no `.resource_table` section, and `rproc_fw_boot()` aborts on any `parse_fw` error; this is why the i.MX override matters | L2 `remoteproc_elf_loader.c:336-338`, `remoteproc_core.c:1408-1410` | read from source |
| `imx_rproc_addr_init()` remembers the memory-region whose node name is exactly `rsc-table` as `priv->rsc_table`; `rsc_table` with an underscore is not recognised | L1 `imx_rproc.c:837-838` | read from source |
| `find_loaded_rsc_table` returns `priv->rsc_table` whenever it is set (no check that the ELF had a table), else the ELF's `.resource_table` translated by `sh_addr` | L1 `imx_rproc.c:700-709`; L2 `remoteproc_elf_loader.c:372-394` | read from source |
| `get_loaded_rsc_table` (attach path) returns `priv->rsc_table` with size `SZ_1K` | L1 `imx_rproc.c:688-698` | read from source |
| `imx_rproc_prepare()` adds every memory-region except nodes named `vdev0buffer` and `rsc-table` as a carveout (`da` = physical base) | L1 `imx_rproc.c:583-631` | read from source |
| The board's DT has a node named `rsc-table` (`rsc-table@88220000`, 4 KiB) and a buffer node named `vdevbuffer`, not `vdev0buffer` | snapshot (b) | observed 2026-11-27 |
| Hence on this board `priv->rsc_table` is set, and at start Linux copies the ELF's cached table (zero bytes if the ELF has none) into `0x88220000`; a table-less hello is unaffected, and `vdevbuffer` is registered as an ordinary carveout | from the rows above and the generic `rproc_start()` copy | inferred |

### Firmware name and search path

| Finding | File:line | Tag |
| --- | --- | --- |
| `rproc_boot()` loads the image with `request_firmware(&firmware_p, rproc->firmware, dev)` | L2 `remoteproc_core.c:1947` | read from source |
| Search order: `fw_path_para` (the `firmware_class.path` module parameter, 256 bytes, mode 0644, so writable at run time through `/sys/module/firmware_class/parameters/path`), then `/lib/firmware/updates/<release>`, `/lib/firmware/updates`, `/lib/firmware/<release>`, `/lib/firmware`; an empty custom path is skipped | L2 `firmware_loader/main.c:470-485`, `:513-531` | read from source |
| Writing `firmware` in sysfs calls `rproc_set_firmware()`, which fails with `-EBUSY` unless the state is `RPROC_OFFLINE` | L2 `remoteproc_sysfs.c:149-159`, `remoteproc_core.c:2184-2188` | read from source |
| `state` accepts `start` (`rproc_boot`), `stop` (`rproc_shutdown`) and `detach` | L2 `remoteproc_sysfs.c:189-209` | read from source |
| Mainline v6.12 additionally rejects firmware names containing a `..` component; whether 6.6.157 has that is unknown, so the name should be a plain file name anyway | L2 at `v6.12`, `_request_firmware()` | read from source (v6.12); 6.6.157 inferred |
| On the board: `remoteproc1` is `imx-rproc`, state `offline`, firmware `rproc-imx-rproc-fw`, `firmware_class` path empty, no M7 image in `/lib/firmware` | snapshot (a), (e) | observed 2026-11-27 |

### ELF checks and segment loading

| Finding | File:line (L2) | Tag |
| --- | --- | --- |
| `rproc_elf_sanity_check()` checks magic, class (32 or 64), endianness (LSB on this host) and that there are program headers; **no `e_machine` check** | `remoteproc_elf_loader.c:37-114` | read from source |
| `rproc_elf_load_segments()` takes the device address from **`p_paddr`** (not `p_vaddr`), skips non-`PT_LOAD` and `memsz == 0` headers, calls `rproc_da_to_va(da, memsz)`, fails with "bad phdr da" if that is NULL, copies `filesz` bytes (`memcpy_toio` for TCM) and zeroes `memsz - filesz` | `remoteproc_elf_loader.c:160-244`, `p_paddr` at `:176`, error at `:212-218` | read from source |

## (b) Zephyr v4.5.0 M7 layout (Z1, exact lines)

| Finding | File:line | Tag |
| --- | --- | --- |
| `itcm@0`: `0x00000000`, 256 KiB; `dtcm@20000000`: `0x20000000`, 256 KiB. The SoC dtsi has **no OCRAM node** for the M7 | `dts/arm/nxp/imx/nxp_imx95_m7.dtsi:134-143` | read from source |
| `imx95_evk/mimx9596/m7`: `zephyr,flash = &itcm`, `zephyr,sram = &dtcm` | `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts:23-26` | read from source |
| `…/m7` defconfig `CONFIG_XIP=y`, `CONFIG_ARM_SCMI=y` | `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7_defconfig:12`, `:15` | read from source |
| `…/m7/ddr`: deletes `zephyr,flash`, `zephyr,sram = &ddr`, `ddr: memory@80000000` = `DT_SIZE_M(42)`; defconfig `CONFIG_XIP=n`; yaml `ram: 43008` | `imx95_evk_mimx9596_m7_ddr.dts:14-22`, `_ddr_defconfig:23`, `_ddr.yaml:11` | read from source |
| `…/m7/flash`: `zephyr,flash` = the external NOR, code in `slot0_partition` | `imx95_evk_mimx9596_m7_flash.dts:14-17` | read from source |
| `CONFIG_FLASH_SIZE` / `CONFIG_FLASH_BASE_ADDRESS` default to the `zephyr,flash` node's size and address | `soc/nxp/imx/imx9/imx95/Kconfig.defconfig.mimx95.m7:10-14` | read from source |
| Cortex-M linker: with XIP the ROM region is `FLASH`, else `RAM`; `ROM_ADDR` = `CONFIG_FLASH_BASE_ADDRESS + CONFIG_FLASH_LOAD_OFFSET`, or `RAM_ADDR` when `!XIP && FLASH_SIZE == 0`; `RAM` = the `zephyr,sram` node | `include/zephyr/arch/arm/cortex_m/scripts/linker.ld:22-27`, `:51-55`, `:64-65`, `:92-104` | read from source |
| `ENTRY(CONFIG_KERNEL_ENTRY)`, default `__start` | `linker.ld:107`; `Kconfig.zephyr:242-244` | read from source |
| The first ROM output section is `rom_start`, which receives the vector table (`exc_vector_table` subsection, symbol `_vector_table`); **no section is named `.interrupts`** | `linker.ld:128-142`; `arch/arm/core/CMakeLists.txt:51-56` (`zephyr_linker_sources(ROM_START … vector_table.ld)`); `arch/arm/core/cortex_m/vector_table.S:33` | read from source (absence of `.interrupts`: searched in the linker script and vector table only) |
| The SoC linker script adds `.resource_table` to the ROM region only with `CONFIG_OPENAMP_RSC_TABLE` | `soc/nxp/imx/imx9/imx95/m7/linker.ld:11-16` | read from source |
| `CONFIG_OPENAMP_RSC_TABLE` defaults off and `imply OPENAMP`; `OPENAMP` depends on the `open-amp` module and selects `LIBMETAL` | `subsys/ipc/open-amp/Kconfig:8-14`; `modules/Kconfig.open-amp:7-12` | read from source |
| The generic table (`resource_table.c`) has a vdev entry only if `CONFIG_OPENAMP_RSC_TABLE_NUM_RPMSG_BUFF > 0` and a trace entry (`RSC_TRACE`, `ram_console_buf`, `CONFIG_RAM_CONSOLE_BUFFER_SIZE`, name `Zephyr_log`) only with `CONFIG_RAM_CONSOLE` | `subsys/ipc/open-amp/resource_table.h:36-44`, `:88-97`; `resource_table.c:40-53` | read from source |
| Upstream's own i.MX95 M7 OpenAMP overlay puts the table at `0x88220000` (`zephyr,ipc_rsc_table`, `0x100` bytes) and shared memory at `0x88000000` (5 MiB), uses `mu7`, and sets `CONFIG_OPENAMP_COPY_RSC_TABLE=y` (the M7 copies its table there at run time) | `samples/subsys/ipc/openamp_rsc_table/boards/imx95_evk_mimx9596_m7.overlay:8-37`; `.conf:10`; `resource_table.c:32-36`, `:47-49` | read from source |
| `prepare-zephyr.sh` pins only `cmsis`, `cmsis_6` and `hal_nxp` ([`zephyr-workspace.md`](zephyr-workspace.md#pins-2026-11-28)), so a build with `CONFIG_OPENAMP_RSC_TABLE` would also need `open-amp` and `libmetal` fetched | | read from source (pins) + inferred (need) |

### Does the default ELF fall inside the windows?

| Target | Load addresses (`p_paddr`) | Boot address under L1 | Verdict | Tag |
| --- | --- | --- | --- | --- |
| `imx95_evk/mimx9596/m7` (TCM) | text/rodata and the load image of `.data` in ITCM from `0x0`; `.bss`/`.noinit` in DTCM from `0x20000000`. Both inside the ATT TCM windows, mapped at probe | no `.interrupts` → 0, which is where the vector table sits (`rom_start` at `ROM_ADDR` = 0) | **Loadable** if each PT_LOAD stays at least one byte short of its window's end | inferred |
| `imx95_evk/mimx9596/m7/ddr` | everything from `0x80000000` (ROM = RAM because `FLASH_SIZE` is 0 once `zephyr,flash` is deleted). Segments load if their `memsz` fits in the 16 MiB `memory@80000000` region; Zephyr nonetheless believes it owns 42 MiB, 26 MiB of which Linux may use ([fb-memory §7](imx95-m7-fb-memory.md#7-interaction-with-linuxs-reserved-memory)) | 0, but the vector table is at `0x80000000`: the M7 would start from whatever is in ITCM | **Not loadable as is** (wrong reset vector; memory overlap) | inferred |
| `imx95_evk/mimx9596/m7/flash` | NOR window, outside every ATT entry → "Translation failed" / "bad phdr da" | | **Not loadable** | inferred |

Resource table: the default build has **none** (`CONFIG_OPENAMP_RSC_TABLE`
off, read from source above). Under L1 that is accepted (read from source
above). A table-less hello therefore needs no rpmsg, no vrings and no
`open-amp` module (inferred).

## (c) Is a Kconfig or devicetree overlay needed to make it loadable?

1. **For `imx95_evk/mimx9596/m7` (TCM): no overlay is needed for Linux to
   accept and start the ELF** (inferred from (a) and (b)). Use this target.
2. **`…/m7/ddr` would need two changes** (inferred): a DT overlay shrinking
   `ddr` to fit the 16 MiB `memory@80000000` carve-out, and a section named
   `.interrupts` at the vector table's address (or a kernel that uses `e_entry`
   as L3 does). Not recommended for the first run.
3. **Optional, for a log without the UART** (inferred): `CONFIG_RAM_CONSOLE=y`
   plus `CONFIG_OPENAMP_RSC_TABLE=y` with
   `CONFIG_OPENAMP_RSC_TABLE_NUM_RPMSG_BUFF=0` gives a table with only a trace
   entry pointing at `ram_console_buf` in DTCM. Linux exposes trace entries as
   `trace0` under the remoteproc debugfs directory, and the DTCM address
   translates through the ATT. Costs: the `open-amp`/`libmetal` modules in the
   workspace, and on this board Linux copies the table to `rsc-table@88220000`
   at start (harmless for a trace-only table, inferred). The rproc trace path
   in `remoteproc_core.c` was not read for this note.
4. **Not needed for loading, but needed for the image to run correctly on a
   Verdin** (inferred, outside this note): the EVK DTS enables peripherals
   (ENETC, FlexCAN, …) and SoC init issues SCMI calls; whether the Verdin SM
   grants them is the subject of [`imx95-m7-scmi.md`](imx95-m7-scmi.md) and
   [`imx95-m7-verdin-bsp.md` §5](imx95-m7-verdin-bsp.md#5-what-an-out-of-tree-zephyr-board-or-overlay-for-verdin-would-need).
   A minimal app should disable what it does not use.

rpmsg (a vdev entry) is out of scope here. One caveat for whoever designs it
(inferred): with `CONFIG_OPENAMP_COPY_RSC_TABLE` the M7 writes its static table
to `0x88220000` after Linux has written its copy there at start, so the two
sides must agree on which copy carries the vring addresses.

## What m7-run must check on the board

All read-only unless marked; the order matters. Items 1-5 decide whether to
write anything at all.

1. **The kernel driver is L1's SMC variant.** dmesg has
   `imx-rproc is available` and no "Using CPU/LMM Protocol OPS" line. If an
   LMM line appears, the BSP driver is L3-like: the boot address becomes
   `e_entry & 0xFFFF0000` and start/stop go through SCMI LMM/CPU from Linux;
   re-read this note's boot-address and resource-table rows before going on.
2. **State is `offline`** in `/sys/class/remoteproc/remoteproc*/state` for the
   one whose `name` is `imx-rproc` (do not assume the index). If `attached` or
   `running`, stop here and record it; the feasibility gate forbids stopping a
   core someone else started.
3. **The node's memory-regions are the ones assumed**: resolve the seven
   `memory-region` phandles of `/proc/device-tree/remoteproc-cm7` against the
   `phandle` property of each `reserved-memory` child, and confirm
   `memory@80000000` (16 MiB) and `rsc-table@88220000` are among them.
4. **The ELF fits** (on the host, before copying): `readelf -lS zephyr.elf`
   shows only PT_LOAD headers with `p_paddr + p_memsz < 0x00040000` (ITCM) or
   `0x20000000 <= p_paddr` and `p_paddr + p_memsz < 0x20040000` (DTCM), no
   section named `.interrupts`, `rom_start` at address 0, ELF32 little-endian.
   Any other address fails with "bad phdr da" before the core is touched.
5. **TCM write risk.** Linux writes ITCM/DTCM through `0x203C0000` /
   `0x20400000` with `memcpy_toio` before the SiP START call. A Verdin user
   saw a data abort on such a write from U-Boot, with an SM TRDC denial logged
   ([verdin-bsp §3](imx95-m7-verdin-bsp.md#3-resource-table-and-rpmsg-vrings-versus-the-dt));
   whether AP-NS may write the M7 TCM under the Toradex SM config is unknown.
   If it may not, the write is an external abort in kernel context and can take
   Linux down (inferred). Do this step only when a reset or power-cycle path is
   available to restore the board, and with the serial console open if
   possible.
6. **Firmware path, then start** (writes, all under `/tmp` and sysfs, restored
   afterwards; the script [`imx95-m7-rproc.sh`](../scripts/imx95-m7-rproc.sh)
   does this): record `firmware_class.path` (observed empty) and the
   `firmware` name (observed `rproc-imx-rproc-fw`); set the path to the `/tmp`
   directory; write a plain file name (no `/`, no `..`) to `firmware`;
   `echo start`. Record dmesg: expect "No resource table in elf" for a
   table-less image, then either success or "Failed to enable remote core!"
   (the SiP START refused by the SM, see the feasibility gate).
7. **Evidence the M7 runs**: the M7 console UART (LPUART3 by default, Verdin
   pins unknown, see [verdin-bsp §1](imx95-m7-verdin-bsp.md#1-console-uart-of-the-m7)),
   or `trace0` in the remoteproc debugfs directory if the image carries the
   trace-only table of (c)3. A `state` of `running` alone only proves the SiP
   call returned 0.
8. **Stop and restore**: `echo stop` (expect possibly "Not in wfi, force
   stopped"), confirm `offline`, write back the original `firmware` name and
   the original `firmware_class.path`, read both back. Confirm Weston is still
   active (it should not have been touched).

## Notes for other documents

- [`imx95-m7-fb-memory.md`](imx95-m7-fb-memory.md#7-interaction-with-linuxs-reserved-memory)
  section 7 gives `rsc-table@88220000` as 1 MiB. The snapshot's `reg` is
  `00000000 88220000 00000000 00001000`, i.e. **4 KiB** (observed 2026-11-27).
  That file is not changed here.
- [`imx95-m7-verdin-bsp.md`](imx95-m7-verdin-bsp.md) §3 and §5 say Zephyr has
  no resource table for i.MX95. Z1 does have an i.MX95 M7 board overlay in
  `samples/subsys/ipc/openamp_rsc_table/boards/` and a `.resource_table`
  section in the SoC linker script (read from source, rows above); what it
  lacks is a table in the board's default build.
