# Verdin iMX95 Cortex-M7: BSP facts for a Zephyr image (source study)

Study date: 2026-11-28. Method: source reading only, no device access.
This file makes no hardware claim. Nothing here was run or observed on a board.

Tags used on every finding:

- **read from source**: a source I read (page, patch, config, DT) says so; the
  source is pinned in [Sources](#sources).
- **inferred**: my deduction from sources; no source says it directly.
- **not found**: I looked and could not read it (the evidence is in
  [What was not found](#what-was-not-found)).

## Scope and limits of this study

The target is Verdin iMX95 on a Mallow carrier, Toradex Reference Multimedia
Image 7.8.0, NXP kernel 6.6.157-7.8.0. Upstream Zephyr has no Verdin iMX95
board, only the NXP i.MX95 EVK M7 target.

The Toradex BSP kernel tree (`linux-toradex`), the Toradex U-Boot tree, the
`meta-toradex-*` layers and the developer.toradex.com pages could not be read
directly in this session. Direct git access to GitHub was refused, and page
fetches of the Toradex sites were refused or returned 404. The Verdin facts
below therefore come from **upstream Linux mailing-list patches** (Toradex
authored) and **forum/search excerpts**, not from the 6.6.157-7.8.0 BSP tree.
Where a statement is about the BSP kernel itself it is tagged *inferred*.
The imx-sm and Zephyr facts were read through a page-fetching helper and
are second-hand relative to this file's author; they are EVK facts, not Verdin
facts.

## 1. Console UART of the M7

| Finding | Tag |
|---|---|
| The Zephyr i.MX95 EVK M7 target uses `zephyr,console = &lpuart3` and `zephyr,shell-uart = &lpuart3`, 115200 baud. `lpuart3` is `serial@42570000`, interrupt 64 (`<64 3>`). | read from source (Zephyr v4.5.0, `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts`, `dts/arm/nxp/imx/nxp_imx95_m7.dtsi`) |
| In NXP's `configs/mx95evk.cfg` the System Manager gives the M7 logical machine (`LM1 name="M7"`) `LPUART3 OWNER`, plus `PIN_GPIO_IO14 OWNER` and `PIN_GPIO_IO15 OWNER`. The AP-NS (non-secure A55) block owns `LPUART8` and LPUART1/4/5/6/7. | read from source (imx-sm `rel_imx_sm_2026q3`, `configs/mx95evk.cfg`) |
| An NXP engineer's forum answer on a Verdin-labelled 19x19 EVK thread says LPUART3 is assigned to the M7 in the SCMI (SM) config, and that using it from Linux requires removing `LPUART3 OWNER` from the M7 resources in `configs/mx95evk.cfg` and adding `PERLPI_LPUART3 ALL` to the A55 resources. The thread's DT diff is for `imx95-19x19-evk.dts`, so this is an EVK statement. | read from source (NXP community thread, see Sources) |
| Variscite's DART-MX95 documentation says LPUART3 is the Cortex-M7 debug console when M7 support is enabled. Different SoM; it shows only that LPUART3 is the NXP-default M7 console. | read from source (Variscite pages, see Sources) |
| Linux on the Verdin iMX95 uses LPUART1 as the Linux console (Verdin `UART_3`, SODIMM 147/149) and LPUART2 as `UART_4` (SODIMM 151/153). Verdin `UART_1` and `UART_2` are on LPUART7 and LPUART8. | read from source (Toradex U-Boot patch series for Verdin iMX95, excerpt via search; upstream Verdin Mallow DT patch labels lpuart1 "UART_3", lpuart2 "UART_4") |
| Therefore the M7 console is **not** one of the four Verdin UART_1..UART_4 instances (1, 2, 7, 8) if the Verdin SM config keeps the NXP EVK default (LPUART3 to the M7). | inferred |
| Which Verdin or Mallow header/pins LPUART3 reaches (pad names, SODIMM numbers, X-number on Mallow): **not found**. The EVK config lists `PIN_GPIO_IO14` and `PIN_GPIO_IO15` for the M7 LM; whether Toradex's SM config does the same, and whether those pads are routed to any SODIMM pin or Mallow header, is not in any source I could read. | not found |
| The Verdin EVK datasheet (Toradex, 0524) is quoted in a search excerpt as connecting an FTDI port to "SoC UART3 (Cortex-M7 console)". That is the Verdin Evaluation Kit, not Mallow, and I could not read the page to see the header. | read from source (excerpt only) |
| One forum answer lists "UART_3: X1 pins 147, 149; UART_4: X1 pins 151, 153". It concerns the Ivy carrier and looks AI-generated. It is not used as evidence. | read from source, low confidence, not relied on |

Conclusion for a board port: the console UART candidate is **LPUART3**
(inferred from the EVK/NXP default and Variscite). Its pin names on Verdin
and Mallow must be confirmed from the Verdin iMX95 datasheet pinout and the
Mallow schematic, which this study did not read.

## 2. M7 memory map that remoteproc expects

### 2.1 Address translation table in the Linux driver

`imx_rproc` support for i.MX95 (compatible `fsl,imx95-cm7`; patch series v1..v6,
last seen v6 2026-01) carries the table `imx_rproc_att_imx95_m7`:

| M7 view (device address) | System address | Size | Meaning | Flags | Tag |
|---|---|---|---|---|---|
| 0x00000000 | 0x203C0000 | 256 KiB (0x40000) | TCM code, non-secure | `ATT_OWN \| ATT_IOMEM` | read from source |
| 0x20000000 | 0x20400000 | 256 KiB (0x40000) | TCM system, non-secure | `ATT_OWN \| ATT_IOMEM` | read from source |
| 0x80000000 | 0x80000000 | 0x50000000 (1.25 GiB) | DDR, identity mapped | not `ATT_OWN` | read from source |

The same patch sets CPU ID 1 and LM ID 1 "to match the System Manager
firmware". That matches `LM1 name="M7"` and `CPU_M7P` in `mx95evk.cfg`.
Source: LKML thread "remoteproc: imx_rproc: Add support for i.MX95"
(`lore`/`lkml.iu.edu` v3..v6). I did not read the merged
`drivers/remoteproc/imx_rproc.c` or the 6.6 BSP copy of it: whether the BSP
table is identical is *inferred*, not verified.

### 2.2 Zephyr EVK M7 target

| Item | Value | Tag |
|---|---|---|
| ITCM | `itcm@0`, 0x00000000, 256 KiB, `zephyr,flash` for the base target | read from source (Zephyr v4.5.0 dtsi) |
| DTCM | `dtcm@20000000`, 0x20000000, 256 KiB, `zephyr,sram` for the base target | read from source |
| DDR variant | `ddr: memory@80000000`, size `DT_SIZE_M(42)`, `zephyr,sram = &ddr`; flash variant uses the NOR at 0x28000000 (128 MiB window) | read from source |
| Board yaml | `ram: 256`, `flash: 256`; `CONFIG_XIP=y`, `CONFIG_MBOX=y`, `CONFIG_ARM_SCMI=y` | read from source |
| SCMI | `scmi_shmem0` at 0x44611000 (0x80), mailbox `mu5` at 0x44610000 | read from source |

The M7 sees ITCM at 0 and DTCM at 0x20000000; Linux sees them at 0x203C0000 and
0x20400000. A Zephyr image linked for ITCM/DTCM matches the remoteproc table
above for sizes and the M7-side addresses. *inferred*.

### 2.3 imx-sm memory grants for the M7 LM (EVK config)

From `configs/mx95evk.cfg` (`rel_imx_sm_2026q3`), M7 LM:

- `M7MIX DATA, begin=0x020380000, end=0x02047FFFF`
- `M7MIX DATA, begin=0x04A060000, end=0x04A09FFFF`
- `DDR EXEC, begin=0x080000000, end=0x089FFFFFF` (160 MiB)

The DDR window overlaps the AP-NS DDR entry `0x088000000..0x089FFFFFF`. All
*read from source*, EVK only. The Verdin SM configuration was **not found**
(the `configs/` directory of NXP imx-sm lists mx95alt, mx95evk, mx95frdm,
mx952alt, mx952evk, mx937alt, mx937frdm, mx94alt, mx94evk, other, simu; no
Verdin or Toradex entry). Toradex ships its own `imx-boot`/SM container;
whether its M7 grants equal the EVK ones is unknown.

### 2.4 Verdin / SMARC reserved DDR (Linux DT)

Toradex's upstream patch "arm64: dts: freescale: imx95-toradex-smarc: Support
Cortex M7" (Emanuele Ghidoli, 2026-03-03; v2 applied by Frank Li as commit
`f0e54db969cd2af80d065ee45827ff24317d507a`) reserves, per a search excerpt
of the diff:

| Node | Address | Size |
|---|---|---|
| `m7_reserved` | 0x80000000 | 16 MiB, `no-map` |

This is the **SMARC** module, not Verdin; the patch's `m7_reserved` is
mentioned in a later binding thread as an extra carveout that imx95-verdin
also takes (see 3). *read from source (excerpt)*. Verdin's exact node values
in the BSP DT: **not found**.

## 3. Resource table and rpmsg vrings versus the DT

Reserved-memory layout used by the upstream i.MX95 EVK and Toradex SMARC patches
(search excerpts of the patch hunks; addresses agree across Peng Fan's EVK
series and Toradex's SMARC series):

| Region | Address | Size | Note |
|---|---|---|---|
| `vdev0vring0` | 0x88000000 | 32 KiB in the SMARC excerpt (8 KiB in the EVK excerpt, the two excerpts disagree) | rpmsg vring |
| `vdev0vring1` | 0x88008000 (SMARC) / 0x88010000 (EVK excerpt) | same | rpmsg vring |
| `vdev1vring0` | 0x88010000 (SMARC) / 0x88018000 (EVK) | same | second vdev |
| `vdev1vring1` | 0x88018000 (SMARC) | same | second vdev |
| `vdevbuffer` (later renamed `vdev0buffer`) | 0x88020000 | 1 MiB, `shared-dma-pool` | rpmsg buffers |
| `rsc_table` | 0x88220000 | 4 KiB | M7 resource table |
| `m7_reserved` | 0x80000000 | 16 MiB, `no-map` | firmware image area |

Tags: *read from source* for the existence of each region and for the
rsc_table and vdevbuffer addresses; the vring sizes and the two sets of vring
offsets differ between the two excerpts I saw, so **the exact vring layout
is not established** here. I could not read the full hunk.

How it lines up (*inferred* from those sources plus the Linux remoteproc
convention):

1. The Linux `remoteproc-cm7` node (compatible `fsl,imx95-cm7`) lists in
   `memory-region` the vrings, `vdevbuffer`, `rsc_table` and `m7_reserved`.
2. The `imx_rproc` driver hardcodes the rsc_table address (a reviewer on the
   binding thread said so and proposed `memory-region-names` to remove the
   "hidden ABI"). The M7 firmware's linker script must therefore place its
   `.resource_table` section exactly at 0x88220000 (M7 view; DDR is identity
   mapped, so the same address).
3. The table's vring entries must carry the `vdev0vring0/1` addresses and the
   `vdevbuffer` carveout so that Linux and the M7 agree. In the NXP
   MCUXpresso/SDK convention the vdev entry holds vring device addresses
   inside 0x88000000 and a carveout entry for the buffer pool.
4. A Zephyr image using `ipc_static_vrings`/OpenAMP would need a hand-written
   `struct resource_table` placed in that section. Zephyr has none for i.MX95
   (see 4).

The mailbox for rpmsg kicks is MU7 on the Linux side per the EVK patch
description ("inter-processor communication via the MU7 mailbox"); the EVK
imx-sm config gives the M7 `MU7_B`, `MU5_A`, `MU8_B`. *read from source*.
Zephyr's dtsi keeps `mu7` disabled and uses `mu5` for SCMI. The pairing
(Linux MU7 side A to M7 MU7_B) is *inferred*.

A forum thread (Toradex community, "Verdin imx95 HMP Overlay - Enable RPMSG &
RemoteProc") reports a user's overlay with the `remoteproc-cm7` node staying
in deferred probe ("platform remoteproc-cm7: deferred probe pending"). A
Toradex staff post says Toradex has no specific shared-memory documentation
for Verdin iMX95 yet. Both *read from source* via excerpts; unverified beyond
that. Toradex's generic Cortex-M shared memory guide uses 0x8FF00000 in DDR
for an i.MX8M example and warns to adjust it per module.

The Toradex BSP 6.6.157 DT for Verdin: **not found**. Whether it keeps the
same names or carries the HMP overlay instead is unknown.

## 4. Difference from the EVK target

| Aspect | Zephyr/NXP EVK | Verdin iMX95 (this study) |
|---|---|---|
| Console | LPUART3 (`serial@42570000`) | probably LPUART3 (inferred); pins **not found** |
| M7 SM config | `mx95evk.cfg` LM1 | Toradex SM config **not found**; no Verdin entry in NXP imx-sm |
| Boot of M7 | NXP EVK flow (U-Boot `bootaux`/imx-mkimage) | Linux remoteproc via SCMI LMM is the documented Toradex path (Toradex says to use remoteproc via sysfs, per search excerpt) |
| Resource table | none in Zephyr for i.MX95 | rsc_table at 0x88220000 in the Linux DT patches |
| Memory | ITCM 0 / DTCM 0x20000000 (256 KiB each), optional DDR at 0x80000000 | same M7-side map expected; Linux loads via 0x203C0000/0x20400000 |
| Board support | `boards/nxp/imx95_evk` | none upstream |
| Display | `dpu`/`mipi_dsi` labels exist but are disabled, M7-only | EVK SM config gives DC, LVDS, DSI and DISPLAYMIX to AP-NS, not the M7; Verdin unknown |

All rows are *read from source* or *inferred* as noted; the Verdin column is
mostly *not found*.

## 5. What an out-of-tree Zephyr board or overlay for Verdin would need

All of this is *inferred* from sections 1-4; nothing was built or tried.

1. A board definition (out of tree, e.g. `boards/<vendor>/verdin_imx95/`)
   reusing the SoC `nxp_imx95_m7.dtsi`. Chosen nodes: `zephyr,console` and
   `zephyr,shell-uart` on the LPUART the Verdin SM config actually gives the
   M7 (LPUART3 unless proven otherwise), ITCM as flash and DTCM as sram.
2. Pin control for that LPUART: confirm that the M7 LM owns the pads and
   route them in the board DT. The pad names come from the Verdin datasheet
   and Mallow schematic (not read).
3. A resource table section in the image, linked at the rsc_table address of
   the Linux DT (0x88220000 in the patches seen), with vring and buffer
   entries matching the BSP DT. Zephyr has none for i.MX95, so this is new
   code (or a minimal table with no vdev if rpmsg is not wanted; remoteproc
   can still boot an ELF without one only if the driver tolerates a missing
   table, not verified).
4. A firmware ELF built for ITCM/DTCM load addresses: the Linux driver writes
   them through 0x203C0000 / 0x20400000, so a DDR-only variant would need the
   `m7_reserved` carveout (0x80000000, 16 MiB) and a matching Zephyr `ddr`
   memory node (Zephyr's EVK variant uses 42 MiB at 0x80000000, larger than
   that carveout).
5. Confirmation, from the Verdin SM configuration actually shipped in the
   image, that LM1 (M7) has CPU_M7P and PD_M7 start/stop rights and which
   peripherals it owns. If the display controller is to be used from the M7 the
   same configuration would have to grant `DC`, `LVDS`, `BLK_CTRL_DISPLAYMIX`
   and their clocks to the M7 (the EVK config gives them to AP-NS), which is a
   question for the display feasibility work, not this file.
6. An MU choice: SCMI uses `mu5` at 0x44610000 with shared memory at
   0x44611000 in Zephyr; any rpmsg needs the MU the Linux DT mailbox names.

## What was not found

- Verdin iMX95 SM (imx-sm) configuration: not in NXP imx-sm `configs/` at the
  pinned tag; Toradex's copy is not read.
- Toradex `linux-toradex` BSP DT for Verdin (display, rproc, reserved memory);
  `github.com/toradex/linux-toradex` returned 404 from the fetch helper and
  `git ls-remote` was refused here.
- Toradex `meta-toradex-*` recipes, U-Boot tree and MCUXpresso SDK
  `verdin_imx95` board: `github.com/toradex/meta-toradex-nxp` and
  `github.com/toradex/u-boot-toradex` returned 404; the developer.toradex.com
  pages were refused. No evidence either way that a Verdin SDK board exists.
- M7 console pad names and carrier header on Verdin/Mallow.
- The full reserved-memory hunk (vring sizes) and the final BSP rproc firmware
  name.

## Sources

Commit or tag given where one was identified. "excerpt" means I saw the
content only in a search-result summary and not the page itself.

- NXP imx-sm, tag `rel_imx_sm_2026q3`, commit
  `eec232878ab7dd460f5a8f3767d39dc7e9412efa`:
  https://github.com/nxp-imx/imx-sm/blob/rel_imx_sm_2026q3/configs/mx95evk.cfg
- Zephyr, tag `v4.5.0`, commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966`:
  https://github.com/zephyrproject-rtos/zephyr/tree/v4.5.0/boards/nxp/imx95_evk
  and
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/dts/arm/nxp/imx/nxp_imx95_m7.dtsi
- Linux, Toradex SMARC i.MX95 Cortex-M7 patch, applied as commit
  `f0e54db969cd2af80d065ee45827ff24317d507a` (excerpt):
  https://lkml.org/lkml/2026/3/3/1959 and
  https://lkml.iu.edu/2603.0/04066.html (v1)
- Linux, imx_rproc i.MX95 support (table `imx_rproc_att_imx95_m7`), v6 (excerpt):
  https://lists.infradead.org/pipermail/linux-arm-kernel/2026-January/1093240.html
  (earlier: https://lists.infradead.org/pipermail/linux-arm-kernel/2025-November/1082877.html)
- Linux, i.MX95 19x19 EVK CM7 and memory nodes, v3 (excerpt):
  https://lkml.iu.edu/2607.3/00122.html
- Linux, `memory-region-names` binding thread mentioning imx95-verdin
  `m7_reserved` (excerpt):
  https://lore.kernel.ime.usp.br/linux-arm-kernel/ahOwO7LBXCYFIngw@shlinux89/t/
- Linux, Verdin iMX95 series including Mallow (console labels, excerpt):
  https://lkml.iu.edu/2603.3/03999.html
- U-Boot, Toradex Verdin iMX95 support, UART mapping (excerpt):
  https://ratatoskr.run/u-boot/2026/04/10318271/t
- NXP community, LPUART3 in Verdin imx95 thread (excerpt):
  https://community.nxp.com/t5/i-MX-Processors/LPUART3-support-in-verdin-imx95/td-p/2159671
- Toradex community: https://community.toradex.com/t/verdin-imx95-hmp-overlay-enable-rpmsg-remoteproc/30375
  and https://community.toradex.com/t/change-console-uart-port-for-verdin-imx95/30464 (excerpts)
- Toradex Cortex-M shared memory guide (excerpt):
  https://developer.toradex.com/software/hmp/hmp-nxp/cortex-m-shared-memory-guide/
- Variscite DART-MX95 UART page (excerpt):
  https://dev.variscite.com/dart-mx95/debian/mx9-uart/
- Verdin i.MX 95 EVK datasheet (excerpt):
  https://docs.toradex.com/116068-datasheet-toradex-verdin-i.mx-95-evk.pdf

Mailing-list and forum URLs are not commit-pinned; the patch Message-IDs
resolve to fixed text, and the one pinned kernel commit is the SMARC M7 patch
above. Re-verify every excerpt-sourced row against the primary page before
using it in a port.
