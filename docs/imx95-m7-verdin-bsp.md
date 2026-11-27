# Verdin iMX95 Cortex-M7: BSP facts for a Zephyr image (source study)

Study date: 2026-11-28. Method: source reading only, no device access.
This file makes no hardware claim. Nothing here was run or observed on a board.

Tags used on every finding:

- **read from source**: a source I read says so; the source is pinned in
  [Sources](#sources). "excerpt" means I saw it only in a search-result summary.
- **inferred**: my deduction from sources; no source says it directly.
- **not found**: I looked and could not read it (see
  [What was not found](#what-was-not-found)).

## Scope and limits of this study

The target is Verdin iMX95 on a Mallow carrier, Toradex Reference Multimedia
Image 7.8.0, NXP kernel 6.6.157-7.8.0. Upstream Zephyr has no Verdin iMX95
board, only the NXP i.MX95 EVK M7 target.

The Toradex BSP kernel tree (`linux-toradex`, branch `toradex_6.6-2.2.x-imx`
per a Toradex staff post) could not be read: github.com/toradex/linux-toradex
returned 404, git.toradex.com cgit answered with an anti-bot block, and
`git ls-remote` against GitHub failed on a credential-helper error. The
Verdin facts below therefore come from Toradex-authored **upstream Linux and
U-Boot patches** for `imx95-verdin.dtsi` (read on lkml.iu.edu and
lists.infradead.org, except where marked excerpt) and from **Toradex developer
pages and forum threads** (read 2026-11-28). A statement about the
6.6.157-7.8.0 BSP tree itself is tagged *inferred*. The imx-sm and Zephyr
facts were read through a page-fetching helper and are EVK facts, not Verdin
facts.

## 1. Console UART of the M7

| Finding | Tag |
|---|---|
| The Zephyr i.MX95 EVK M7 target uses `zephyr,console = &lpuart3` and `zephyr,shell-uart = &lpuart3`, 115200 baud. `lpuart3` is `serial@42570000`, interrupt 64 (`<64 3>`). | read from source (Zephyr v4.5.0, `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts`, `dts/arm/nxp/imx/nxp_imx95_m7.dtsi`) |
| In NXP's `configs/mx95evk.cfg` the System Manager gives the M7 logical machine (`LM1 name="M7"`) `LPUART3 OWNER`, plus `PIN_GPIO_IO14 OWNER` and `PIN_GPIO_IO15 OWNER`. The AP-NS (non-secure A55) block owns `LPUART8` and LPUART1/4/5/6/7. | read from source (imx-sm `rel_imx_sm_2026q3`, `configs/mx95evk.cfg`) |
| An NXP engineer's forum answer says LPUART3 is assigned to the M7 in the SCMI (SM) config, and using it from Linux requires removing `LPUART3 OWNER` from the M7 resources in `configs/mx95evk.cfg` and adding `PERLPI_LPUART3 ALL` to the A55 resources. The thread's DT diff is for `imx95-19x19-evk.dts`; an EVK statement. | read from source (excerpt; page returned 403) |
| Variscite's DART-MX95 documentation says LPUART3 is the Cortex-M7 debug console when M7 support is enabled. Different SoM; it shows only that LPUART3 is the NXP-default M7 console. | read from source (excerpt) |
| The Verdin DT aliases are `serial0..4` = lpuart7, lpuart8, lpuart1, lpuart2, lpuart6 (Bluetooth). **lpuart3 is not used by Linux in the Verdin DT.** Verdin `UART_1` = lpuart7, `UART_2` = lpuart8, `UART_3` = lpuart1 (Linux console), `UART_4` = lpuart2. | read from source (Toradex U-Boot patch v2 `imx95-verdin.dtsi`; Verdin Mallow Linux patch v3 5/6) |
| Pads behind those UARTs: UART_1 on GPIO_IO08..11, UART_2 on GPIO_IO12/13, UART_3 on UART1_RXD/TXD, UART_4 on UART2_RXD/TXD. | read from source (summary of the truncated half of the U-Boot patch page; not verbatim) |
| The EVK config's M7 pads GPIO_IO14/GPIO_IO15 are not among the pads Linux uses for UART_1/UART_2 (GPIO_IO08..13). | inferred |
| Toradex's "How to load and run firmware on alternative cores (NXP)" page (updated 2026-11-11) says the alternative-core serial console is on the `ttyUSB2` port of the USB debug connector and U-Boot on `ttyUSB3`. The text is shared by the i.MX 95/93/8MP/8MM tabs; the i.MX 95 tab's pinout/connection details did not survive extraction. It names no LPUART and no Mallow header. | read from source |
| Therefore the M7 console is **not** one of Verdin `UART_1..UART_4` (LPUART 7, 8, 1, 2) if the Verdin SM config keeps the NXP EVK default. | inferred |
| Which Verdin or Mallow header/pins the M7 console reaches (SODIMM numbers, Mallow X-number): **not found**. Whether Toradex's SM config gives GPIO_IO14/15 to the M7 as the EVK config does, and whether those pads reach any SODIMM pin or Mallow header, is not in any source I could read. | not found |
| The Verdin EVK datasheet (Toradex) is quoted in a search excerpt as connecting an FTDI port to "SoC UART3 (Cortex-M7 console)". That is the Verdin Evaluation Kit, not Mallow. | read from source (excerpt) |
| One forum answer lists "UART_3: X1 pins 147, 149; UART_4: X1 pins 151, 153". It concerns the Ivy carrier and looks AI-generated; not used as evidence. | not relied on |

Conclusion for a board port: the console UART candidate is **LPUART3**
(inferred from the EVK/NXP default and Variscite; no Verdin source names the
LPUART). Its pin names and header on Verdin and Mallow must be confirmed from
the Verdin iMX95 datasheet pinout, the Mallow schematic and the SM config in
the BSP's imx-boot, none of which this study read.

## 2. M7 memory map that remoteproc expects

### 2.1 Address translation table in the Linux driver

`imx_rproc` for i.MX95 (compatible `fsl,imx95-cm7`, patch v6 of 2026-01, read
verbatim):

```c
static const struct imx_rproc_att imx_rproc_att_imx95_m7[] = {
	/* TCM CODE NON-SECURE */
	{ 0x00000000, 0x203C0000, 0x00040000, ATT_OWN | ATT_IOMEM },
	/* TCM SYS NON-SECURE*/
	{ 0x20000000, 0x20400000, 0x00040000, ATT_OWN | ATT_IOMEM },
	/* DDR */
	{ 0x80000000, 0x80000000, 0x50000000, 0 },
};
/* .ops = &imx_rproc_ops_sm_lmm, .cpuid = 1, .lmid = 1 ("Must align with System Manager Firmware") */
```

| M7 view | System address | Size | Meaning | Tag |
|---|---|---|---|---|
| 0x00000000 | 0x203C0000 | 256 KiB | TCM code (ITCM), non-secure | read from source |
| 0x20000000 | 0x20400000 | 256 KiB | TCM system (DTCM), non-secure | read from source |
| 0x80000000 | 0x80000000 | 0x50000000 | DDR, identity mapped, not driver-owned | read from source |

CPU ID 1 and LM ID 1 match `LM1 name="M7"` / `CPU_M7P` in `mx95evk.cfg`. I
did not read the merged `drivers/remoteproc/imx_rproc.c` or the 6.6 BSP copy:
that the BSP table is identical is *inferred*.

### 2.2 Zephyr EVK M7 target

| Item | Value | Tag |
|---|---|---|
| ITCM | `itcm@0`, 0x00000000, 256 KiB, `zephyr,flash` for the base target | read from source (Zephyr v4.5.0 dtsi) |
| DTCM | `dtcm@20000000`, 0x20000000, 256 KiB, `zephyr,sram` for the base target | read from source |
| DDR variant | `ddr: memory@80000000`, `DT_SIZE_M(42)`, `zephyr,sram = &ddr`; flash variant uses NOR at 0x28000000 (128 MiB window) | read from source |
| Board yaml/defconfig | `ram: 256`, `flash: 256`; `CONFIG_XIP=y`, `CONFIG_MBOX=y`, `CONFIG_ARM_SCMI=y` | read from source |
| SCMI | `scmi_shmem0` at 0x44611000 (0x80), mailbox `mu5` at 0x44610000 | read from source |

A Zephyr image linked for ITCM at 0 and DTCM at 0x20000000 matches the
remoteproc table above for M7-side addresses and sizes. *inferred*.

### 2.3 imx-sm memory grants for the M7 LM (EVK config)

From `configs/mx95evk.cfg` (`rel_imx_sm_2026q3`), M7 LM:

- `M7MIX DATA, begin=0x020380000, end=0x02047FFFF`
- `M7MIX DATA, begin=0x04A060000, end=0x04A09FFFF`
- `DDR EXEC, begin=0x080000000, end=0x089FFFFFF` (160 MiB)

The DDR window overlaps the AP-NS DDR entry `0x088000000..0x089FFFFFF`. All
*read from source*, EVK only. The Verdin SM configuration was **not found**:
NXP imx-sm `configs/` lists mx95alt, mx95evk, mx95frdm, mx952alt, mx952evk,
mx937alt, mx937frdm, mx94alt, mx94evk, other, simu, with no Verdin or Toradex
entry. Toradex ships its own SM inside imx-boot; whether its M7 grants equal
the EVK ones is unknown.

### 2.4 Verdin reserved DDR (Linux DT)

Toradex's `imx95-verdin.dtsi` as posted for U-Boot (patch 2/3, v2, April
2026, read) has the same remoteproc and reserved-memory layout as the upstream
EVK DT and the Toradex SMARC patch (section 3). A Toradex staff post says the
Verdin iMX95 RPMsg setup is in the base DT `imx95-verdin.dtsi` on branch
`toradex_6.6-2.2.x-imx`. The U-Boot cover letter says "the first DDR region is
reserved for Cortex-M7 firmware"; a summary of that series' `verdin-imx95.h`
says the first 256 MB of SDRAM is reserved for M7 firmware (summary text, not
verbatim). Whether the 6.6.157-7.8.0 BSP DT is identical to these patches is
*inferred*, not verified.

## 3. Resource table and rpmsg vrings versus the DT

Reserved-memory nodes (all `no-map`), identical in the Verdin dtsi (U-Boot
patch v2), the SMARC patch v1 (2026-03-03) and the upstream 19x19 EVK patch v3
(2026-07-24). *read from source*:

| Region | Address | Size |
|---|---|---|
| `m7_reserved` (`memory@80000000`) | 0x80000000 | 0x1000000 (16 MiB) |
| `vdev0vring0` | 0x88000000 | 0x8000 (32 KiB) |
| `vdev0vring1` | 0x88008000 | 0x8000 |
| `vdev1vring0` | 0x88010000 | 0x8000 |
| `vdev1vring1` | 0x88018000 | 0x8000 |
| `vdevbuffer` (`vdev0buffer` in EVK v3), `shared-dma-pool` | 0x88020000 | 0x100000 (1 MiB) |
| `rsc_table` | 0x88220000 | 0x1000 (4 KiB) |

The `remoteproc-cm7` node (compatible `fsl,imx95-cm7`, label `cm7`):
`mbox-names = "tx", "rx", "rxdb"`, `mboxes = <&mu7 0 1 &mu7 1 1 &mu7 3 1>`,
`memory-region = <&vdevbuffer>, <&vdev0vring0>, <&vdev0vring1>,
<&vdev1vring0>, <&vdev1vring1>, <&rsc_table>, <&m7_reserved>`, and `&mu7`
set to `okay`. *read from source*.

Review history (read): on the SMARC v1 patch Frank Li noted that the driver
hardcodes the rsc-table address and asked for `memory-region-names` support to
remove the "hidden ABI"; the author said v1 did not work because of a
last-minute rename and sent a v2 with renamed nodes. The v2 diff could not be
fetched; it was reported applied as commit
`f0e54db969cd2af80d065ee45827ff24317d507a` (search excerpt). A May 2026
binding thread (excerpt) says imx95-verdin takes an extra carveout named
`m7_reserved`. Final node names in the BSP 6.6 tree: **not found**.

How it lines up (*inferred* from these sources and the Linux remoteproc
convention):

1. `imx_rproc` writes ITCM/DTCM through the 0x203C0000/0x20400000 aliases and
   reads the resource table from the rsc_table carveout, whose address the
   driver hardcodes. A Zephyr image's `.resource_table` section must therefore
   sit at 0x88220000 (DDR is identity-mapped).
2. The table's vdev entry must name vrings inside 0x88000000..0x8801FFFF and
   the 1 MiB buffer pool at 0x88020000 so both sides agree.
3. Zephyr has no resource table for i.MX95 (see section 5), so a table would
   be new code. The Toradex remoteproc page's example log shows `no dtb
   rsrc-table` for a hello_world ELF that booted, so a table may not be
   mandatory; that log was a generic example, not stated to be i.MX95
   (*read from source*).
4. Kick mailboxes: the Linux node uses `mu7`; the EVK imx-sm config gives the
   M7 `MU7_B`, `MU5_A`, `MU8_B`; Zephyr keeps `mu7` disabled and uses `mu5` for
   SCMI. Pairing Linux's MU7 side with the M7's MU7_B is *inferred*.

Toradex's own statements (developer.toradex.com and community, read
2026-11-28):

- RemoteProc is enabled by default on i.MX 95; no overlay is needed. The
  firmware must be `*.elf`; the default search path is `/lib/firmware`, which
  can be changed through `/sys/module/firmware_class/parameters/path`.
  `*.bin` files go through U-Boot.
- The Cortex-M shared-memory guide's i.MX 95 example overlay (a plain shared
  buffer, not the rproc layout) reserves 1 MiB at 0x8FF00000 and Toradex does
  not recommend it for production.
- Toradex staff: "We don't have specific documentation on shared memory for
  the Verdin iMX95". On Verdin, `remoteproc-cm7` appeared as `remoteproc1`
  (remoteproc0 is the Neutron NPU). A V1.0B user saw `deferred probe pending`
  on 7.6.0/7.6.1 until kernel 6.6.138-7.6.1 (reported 2026-05-28); staff
  confirmed rproc on Torizon 7.7.0 and said M7 rproc may be missing on A1
  silicon.
- Staff sequence for SMARC iMX95 (Torizon): set the `firmware_class` path to
  `/tmp/`, `echo stop`, write the ELF name into `firmware`, `echo start` on
  `remoteproc1`. A user's ELF linked at 0x90000000 failed (`bad phdr da
  0x90000000`) and one linked at 0x80000000 caused an SError panic on SMARC
  with a DT patch applied (user reports, not staff-confirmed).
- U-Boot path on Toradex's page (Cortex-M7 `hello_world_cm7.bin`):
  `cp.b ${loadaddr} 0x203c0000 ${cm_isize}; dcache flush; bootaux 0x20000000`,
  size 0x20000. A user on a V1.0A Verdin hit a data abort writing 0x203C0000
  with the System Manager monitor logging a TRDC domain 3 non-secure write
  denial; the TRDC explanation is the user's and an AI assistant's hypothesis,
  not a staff statement.

## 4. Difference from the EVK target

| Aspect | Zephyr/NXP EVK | Verdin iMX95 (this study) |
|---|---|---|
| Console | LPUART3 (`serial@42570000`) | probably LPUART3 (inferred); Toradex says the alt-core console is `ttyUSB2` of the USB debug port; pins **not found** |
| M7 SM config | `mx95evk.cfg` LM1 | Toradex SM config **not found**; no Verdin entry in NXP imx-sm |
| Boot of M7 | NXP EVK flow (U-Boot `bootaux`/imx-mkimage) | Toradex documents U-Boot `bootaux` (copy to 0x203c0000) and Linux remoteproc (ELF, on by default) |
| Resource table | none in Zephyr for i.MX95 | `rsc_table` carveout at 0x88220000 in the Verdin dtsi (U-Boot patch) |
| Memory | ITCM 0 / DTCM 0x20000000 (256 KiB each), optional DDR at 0x80000000 | same M7-side map expected; Linux loads via 0x203C0000/0x20400000; `m7_reserved` 16 MiB at 0x80000000 |
| Board support | `boards/nxp/imx95_evk` | none upstream |
| Display | `dpu`/`mipi_dsi` labels exist but are disabled, M7-only | EVK SM config gives DC, LVDS, DSI and DISPLAYMIX to AP-NS, not the M7; Verdin unknown |

All rows are *read from source* or *inferred* as noted; the Verdin column is
mostly *not found*.

## 5. What an out-of-tree Zephyr board or overlay for Verdin would need

All *inferred* from sections 1-4; nothing was built or tried.

1. A board definition (out of tree, e.g. `boards/<vendor>/verdin_imx95/`)
   reusing the SoC `nxp_imx95_m7.dtsi`. Chosen nodes: `zephyr,console` and
   `zephyr,shell-uart` on the LPUART the Verdin SM config actually gives the M7
   (LPUART3 unless proven otherwise), ITCM as flash and DTCM as sram.
2. Pin control for that LPUART: confirm the M7 LM owns the pads and route
   them in the board DT. Pad names come from the Verdin datasheet and Mallow
   schematic (not read).
3. A resource table section, linked at the rsc_table address of the Linux DT
   (0x88220000), with vring and buffer entries matching the DT. Zephyr has none
   for i.MX95 (v4.5.0: no rsc_table in the board dir or dtsi, no resource-table
   header under `include/zephyr/ipc/`, no `imx95_evk` overlay in the OpenAMP
   sample), so this is new code; a table-less ELF may boot (section 3, item 3).
4. A firmware ELF whose load segments sit in ITCM/DTCM (the driver writes them
   via 0x203C0000/0x20400000) or inside DDR 0x80000000 (the 16 MiB
   `m7_reserved` carveout, no-map). Zephyr's EVK DDR variant is 42 MiB at
   0x80000000, larger than that carveout, so a DDR variant needs a smaller
   `ddr` node. Toradex user reports of ELFs at 0x80000000/0x90000000 failing
   (section 3) are a warning, not a diagnosis.
5. Confirmation from the Verdin SM configuration actually shipped in the image
   that LM1 (M7) has CPU_M7P and PD_M7 start/stop rights and which peripherals
   it owns. If the display controller is to be used from the M7 the same
   configuration would have to grant `DC`, `LVDS`, `BLK_CTRL_DISPLAYMIX` and
   their clocks to the M7 (the EVK config gives them to AP-NS); that belongs
   to the display feasibility work.
6. An MU choice: SCMI uses `mu5` at 0x44610000 with shared memory at
   0x44611000 in Zephyr; any rpmsg needs the MU the Linux DT mailbox names
   (`mu7`).

## What was not found

- Verdin iMX95 SM (imx-sm) configuration: not in NXP imx-sm `configs/` at the
  pinned tag; Toradex's copy (inside its imx-boot) was not read.
- Toradex `linux-toradex` BSP DT (branch `toradex_6.6-2.2.x-imx`) and a head
  commit SHA: GitHub 404, cgit anti-bot block, `git ls-remote` failed. The BSP
  DT is represented only by Toradex-authored U-Boot/Linux patches.
- Toradex `meta-toradex-*` recipes, U-Boot tree and any MCUXpresso SDK
  `verdin_imx95` board: GitHub 404 for the two repos tried; no evidence for or
  against an SDK board. The Toradex i.MX95 Zephyr blog
  (https://www.toradex.com/blog/imx95-zephyr-asymmetric-multiprocessing)
  returned HTTP 403 to the fetch helper on 2026-11-28 and was not read.
- M7 console LPUART on Verdin and its pad names and SODIMM/Mallow header pins
  (the i.MX 95 tab of the Toradex load-firmware page did not extract).
- The SMARC v2 diff and the NXP community LPUART3 thread (403; used only from
  a search excerpt).

## Sources

Commit or tag where one was identified.

- NXP imx-sm, tag `rel_imx_sm_2026q3`, commit
  `eec232878ab7dd460f5a8f3767d39dc7e9412efa`:
  https://github.com/nxp-imx/imx-sm/blob/rel_imx_sm_2026q3/configs/mx95evk.cfg
- Zephyr, tag `v4.5.0`, commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966`:
  https://github.com/zephyrproject-rtos/zephyr/tree/v4.5.0/boards/nxp/imx95_evk
  and
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/dts/arm/nxp/imx/nxp_imx95_m7.dtsi
- Linux, Toradex SMARC i.MX95 Cortex-M7 patch v1 (read) and replies:
  https://lkml.iu.edu/2603.0/04066.html ,
  https://lkml.iu.edu/2603.0/04485.html , https://lkml.iu.edu/2603.0/04509.html ;
  v2, reported applied as commit `f0e54db969cd2af80d065ee45827ff24317d507a`
  (excerpt only): https://lkml.org/lkml/2026/3/3/1959
- Linux, imx_rproc i.MX95 support, v6 (read):
  https://lists.infradead.org/pipermail/linux-arm-kernel/2026-January/1093240.html
- Linux, i.MX95 19x19 EVK CM7 and memory nodes, v3 (read):
  https://lkml.iu.edu/2607.3/00122.html
- Linux, Verdin iMX95 Mallow carrier, v3 5/6 (read):
  https://lkml.iu.edu/2603.3/03999.html
- U-Boot, Toradex Verdin iMX95 v2, `imx95-verdin.dtsi` (first half read, rest
  summarised): https://ratatoskr.run/u-boot/2026/04/10318271/t
- Linux, `memory-region-names` binding thread mentioning imx95-verdin
  `m7_reserved` (excerpt):
  https://lore.kernel.ime.usp.br/linux-arm-kernel/ahOwO7LBXCYFIngw@shlinux89/t/
- NXP community, LPUART3 in Verdin imx95 thread (excerpt, 403):
  https://community.nxp.com/t5/i-MX-Processors/LPUART3-support-in-verdin-imx95/td-p/2159671
- Toradex community (read 2026-11-28):
  https://community.toradex.com/t/verdin-imx95-hmp-overlay-enable-rpmsg-remoteproc/30375 ,
  https://community.toradex.com/t/known-workflow-to-load-and-run-cortex-m7-firmware-on-verdin-i-mx95-som/30309 ,
  https://community.toradex.com/t/load-binary-to-cortex-m7-on-smarc-imx95/30075 ;
  console-port thread (excerpt):
  https://community.toradex.com/t/change-console-uart-port-for-verdin-imx95/30464
- Toradex developer pages (read 2026-11-28; no commit, dated by "last updated"):
  https://developer.toradex.com/software/hmp/hmp-nxp/cortex-m-shared-memory-guide/ ,
  https://developer.toradex.com/software/hmp/hmp-nxp/how-to-load-firmwares-nxp/ (2026-11-11),
  https://developer.toradex.com/software/hmp/hmp-nxp/how-to-use-remoteproc/ (2026-11-11)
- Variscite DART-MX95 UART page (excerpt):
  https://dev.variscite.com/dart-mx95/debian/mx9-uart/
- Verdin i.MX 95 EVK datasheet (excerpt):
  https://docs.toradex.com/116068-datasheet-toradex-verdin-i.mx-95-evk.pdf

Mailing-list, forum and developer-page URLs are not commit-pinned; the
mailing-list pages are fixed text, the one pinned kernel commit is the SMARC
v2 commit above, and no BSP-tree commit could be pinned. Re-verify every row
marked excerpt against its primary page before using it in a port.
