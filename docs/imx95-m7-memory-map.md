# i.MX95 Cortex-M7 memory map and where a Zephyr scanout buffer can live (source study)

Study date: 2026-11-28. Method: source reading only. No board was accessed and
nothing was run, built or measured; **nothing in this file is a hardware
observation.**

Scope: SoW §5.1 / WP3 (M2). Which memory the Cortex-M7 sees, which of it Linux
leaves to the M7 on the Verdin iMX95, whether the display controller (DPU) can
fetch from it, and what a 1280x800 scanout buffer needs.

Every finding is tagged one of:

- **read from source**: the text was read at the pin given in
  [Sources](#sources). Where a row says "via `<doc>`", the pinned read was
  done by an earlier item and is recorded in that repository document; this
  study did not re-read that source.
- **inferred**: my deduction. No source states it.
- **not found**: I looked and no source I could read answers it.

Related, not edited here: [`imx95-zephyr-feasibility.md`](imx95-zephyr-feasibility.md),
[`imx95-dpu-source-6.6.md`](imx95-dpu-source-6.6.md),
[`imx95-m7-fb-memory.md`](imx95-m7-fb-memory.md) (cache and MPU rules),
[`imx95-m7-verdin-bsp.md`](imx95-m7-verdin-bsp.md),
[`imx95-verdin-bsp-src.md`](imx95-verdin-bsp-src.md),
[`imx95-m7-rproc-load.md`](imx95-m7-rproc-load.md).

## Sources

| Id | Source | Pin |
|---|---|---|
| Z | Zephyr, <https://github.com/zephyrproject-rtos/zephyr> | tag `v4.5.0` (the tag is pinned by [`zephyr-workspace.md`](zephyr-workspace.md)), commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966` (`git ls-remote` peel of the tag, 2026-11-28); files fetched as `https://raw.githubusercontent.com/zephyrproject-rtos/zephyr/v4.5.0/<path>` on 2026-11-28 |
| L | NXP Linux, <https://github.com/nxp-imx/linux-imx> | tag `lf-6.6.52-2.2.2` = branch `lf-6.6.y` tip at commit `5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc` (tag peel and branch tip both this commit, `git ls-remote` 2026-11-28; committer date 2025-11-26); files fetched as `https://raw.githubusercontent.com/nxp-imx/linux-imx/5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc/<path>` on 2026-11-28 |
| S | NXP System Manager, <https://github.com/nxp-imx/imx-sm> | tag `rel_imx_sm_2026q3`, commit `eec232878ab7dd460f5a8f3767d39dc7e9412efa`; files fetched as `https://raw.githubusercontent.com/nxp-imx/imx-sm/eec232878ab7dd460f5a8f3767d39dc7e9412efa/<path>` on 2026-11-28 |
| K | Toradex kernel of BSP 7.8.0, `https://git.toradex.com/linux-toradex.git`, branch `toradex_6.6-2.2.x-imx` | commit `64f85d65c112c45ceb6c7e0566885afc62b1a551` (committer date 2026-09-25): the `SRCREV_machine` of BSP 7.8.0 (see the chain below). Read 2026-11-28 by item memmap-reread with `git fetch --depth 1 --filter=blob:none` of that commit plus a fetch of the DT blobs by id, then `git show`. Files: `arch/arm64/boot/dts/freescale/imx95-verdin.dtsi` (blob `c25f8e4c7792b3fc0d7d94e8c7afac4e34050f9a`), `imx95-verdin-mallow.dtsi` (`4ba4ab29e25965c0150dc61c7e4e1cb1c3d5a260`), `imx95-verdin-wifi-mallow.dts` (`2049b2b07f538f2900f418df434cf8a65f6f8256`), `imx95-verdin-nonwifi-mallow.dts` (`98fca21d4bf092b5b68d669ca54a826196e71ca3`), `imx95-verdin-wifi.dtsi` (`35a9c7f41e3e842763d58b538c6924cc391be081`), `imx95-verdin-nonwifi.dtsi` (`044ec8bcf8cd1a5b84a48b8abf3dda1a712981a9`) |
| T | Toradex kernel, `linux-toradex`, branch `toradex_6.6-2.2.x-imx` | commit `8ad2b7ff6b5666b25b1b43819c83b9ae3930b8df` (branch head on 2026-11-28, committer date 2026-11-03, read by the earlier item recorded in [`imx95-verdin-bsp-src.md`](imx95-verdin-bsp-src.md#1-pins)). `git diff 64f85d65c112c45ceb6c7e0566885afc62b1a551 8ad2b7ff6b5666b25b1b43819c83b9ae3930b8df` over the six Verdin/Mallow DT files of K and `drivers/remoteproc/imx_rproc.c` is empty (2026-11-28), so T and K have identical copies of those files |
| M | NXP MCUXpresso SDK examples, <https://github.com/nxp-mcuxpresso/mcuxsdk-examples> | commit `cc8974f843f496ac0ec959c55164766fd0d489bd` (2026-11-23), read via [`imx95-mcux-display.md`](imx95-mcux-display.md#pinned-sources) (pins EX, CORE) |
| R | Toradex BSP release manifest, `https://git.toradex.com/toradex-manifest.git` | tag `7.8.0` = commit `d989ca3a34d834e0829b1ee2ef92822ec32b046e` (`git ls-remote` and `git fetch --depth 1` of the tag, 2026-11-28); files `tdxref/default.xml`, `bsp/pinned-tdx.xml` |
| N | Toradex NXP layer, `https://git.toradex.com/meta-toradex-nxp.git` | commit `e0f9a9c3a84fe7f26538092f272db56c3e4770c1`, the revision R pins (fetched by id, 2026-11-28); files `recipes-kernel/linux/linux-toradex_6.6-2.2.x.bb`, `conf/machine/verdin-imx95.conf` |

**How K is tied to BSP 7.8.0** (item memmap-reread, 2026-11-28; every step
read from source):

| Step | What was read | Result |
|---|---|---|
| 1 | `linux-toradex` refs (`git ls-remote`) | no Toradex release tags for BSP 7.x (only mainline `vX.Y[-rcN]` and old image tags), so 7.8.0 is pinned only through the layer recipe |
| 2 | R `tdxref/default.xml` (includes `bsp/pinned-tdx.xml`) and R `bsp/pinned-tdx.xml` | `<project name="meta-toradex-nxp.git" ... revision="e0f9a9c3a84fe7f26538092f272db56c3e4770c1" upstream="scarthgap-7.x.y"/>` |
| 3 | N `conf/machine/verdin-imx95.conf:23` and `:27-39` | `PREFERRED_PROVIDER_virtual/kernel = "linux-toradex"`; the DTB list includes `imx95-verdin-wifi-mallow.dtb` and `imx95-verdin-nonwifi-mallow.dtb` |
| 4 | N `recipes-kernel/linux/linux-toradex_6.6-2.2.x.bb:36-40` | `COMPATIBLE_MACHINE = "(mx8-nxp-bsp\|mx9-nxp-bsp)"`, `LINUX_VERSION = "6.6.157"`, `SRCBRANCH = "toradex_6.6-2.2.x-imx"`, `SRCREV_machine = "64f85d65c112c45ceb6c7e0566885afc62b1a551"` |
| 5 | K `imx95-verdin-{wifi,nonwifi}-mallow.dts` | each includes only `imx95-verdin.dtsi`, `imx95-verdin-{wifi,nonwifi}.dtsi` and `imx95-verdin-mallow.dtsi`; none of the four Mallow/wifi files mentions `reserved-memory`, `memory`, `cm7`, `remoteproc`, `mu7`, `vdev` or `rsc`, so the carveouts come from `imx95-verdin.dtsi` alone |

That `LINUX_VERSION` 6.6.157 at this `SRCREV` is the running
`6.6.157-7.8.0` kernel is **inferred** (version string plus release tag; the
board's own DT was not read by this item). The NXP RM for the i.MX95 is not
publicly fetchable; "reference-manual facts" below are only those that Z, L,
S and M quote or encode. Nothing was taken from the RM itself.

An earlier attempt on the same day (recorded in this file's history) could
not get past step 1 because the agent sandbox refused `git clone`/`curl`, the
cgit web pages return an anti-bot page, and there is no GitHub mirror of
`toradex-manifest`, `linux-toradex` or `meta-toradex-nxp` (HTTP 404). Git
fetches over HTTPS from the Toradex host work.

## 1. The Cortex-M7 view (Zephyr and the SoC description)

### 1.1 Memories and windows in upstream Zephyr

All rows: file at `v4.5.0`, line numbers counted from line 1 of the raw file
(the dtsi lines after 143 are approximate).

| Finding | Source | Tag |
|---|---|---|
| ITCM: `itcm@0`, `reg = <0x00000000 DT_SIZE_K(256)>`, `zephyr,memory-region = "ITCM"` | Z `dts/arm/nxp/imx/nxp_imx95_m7.dtsi:134-138` | read from source |
| DTCM: `dtcm@20000000`, `reg = <0x20000000 DT_SIZE_K(256)>` | same file `:140-143` | read from source |
| No OCRAM node, no `dma-ranges`, no LVDS node, and no other `zephyr,memory-region` in that dtsi | same file (searched) | read from source (absence of nodes, not of hardware) |
| FlexSPI XIP window: `<0x28000000 DT_SIZE_M(128)>` | same file (~161) | read from source |
| MPU node is bare: `arm,armv7m-mpu` at `0xe000ed90` | same file `:33-36` | read from source |
| Board `imx95_evk/mimx9596/m7`: `zephyr,flash = &itcm`, `zephyr,sram = &dtcm`, console `lpuart3` | Z `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts:23-30` | read from source |
| Board `imx95_evk/mimx9596/m7/ddr`: `ddr: memory@80000000`, `reg = <0x80000000 DT_SIZE_M(42)>`, `zephyr,sram = &ddr`, `/delete-property/ zephyr,flash`; defconfig `CONFIG_XIP=n`; yaml `ram: 43008` (KiB) | Z `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7_ddr.dts:11-23`, `..._ddr.yaml`, `..._ddr_defconfig` | read from source |
| SoC linker script is 17 lines: it includes the generic Cortex-M script and only adds `.resource_table` (with `CONFIG_OPENAMP_RSC_TABLE`). It defines no memory region, no OCRAM and no non-cacheable section | Z `soc/nxp/imx/imx9/imx95/m7/linker.ld:7-17` | read from source |
| `soc/nxp/imx/imx9/imx95/m7/` holds only `CMakeLists.txt`, `linker.ld`, `soc.c`, `soc.h`: no MPU region table. `soc_early_init_hook()` enables the D- and I-cache under `CONFIG_CACHE_MANAGEMENT` | Z same directory, `soc.c` | read from source |
| DPU node `dpu@4b400000` in the M7 dtsi: `compatible = "socionext,dpu"`, `reg = <0x4b400000 0x1000>`, `interrupts = <215 0>`, `status = "disabled"`; `disp_irqsteer@4b0b0000` (`nxp,irq-offset = <874>`, 512 IRQs) | Z `nxp_imx95_m7.dtsi` (~983, ~1054-1061) | read from source |
| MU5 (SCMI mailbox) `0x44610000`, SCMI shared memory `memory@44611000` `0x80` bytes; MU7 `mu7@42440000`, disabled | Z `nxp_imx95_m7.dtsi:63-71`, ~869-876 | read from source |
| The framebuffer of the stock DPU driver is a `static uint8_t __aligned(32) dpu_frame_buffer_N[CONFIG_DPU_FB_NUM * height * ROUND_UP(width * 4, 32)]`, so it lands in `.bss`: DTCM on the TCM board, the DDR window on the `m7/ddr` board. There is no devicetree or Kconfig knob to choose its region. `CONFIG_DPU_FB_NUM` is `int`, default 2, range 0..2 | Z `drivers/display/display_socionext_dpu.c` (~339-345, ~382), `drivers/display/Kconfig.socionext_dpu` | read from source |
| `dpu_write()` hands the DPU a raw CPU pointer cast to `uint32_t` (`DPU_SetFetchUnitSrcBufferAddr(..., (uint32_t)(uintptr_t)data->active_fb)`), with no translation; with the buffer 32-byte aligned, full-screen, at x=y=0 and matching pitch it uses the caller's buffer without a copy, otherwise it copies into its own static buffers. It does no cache maintenance | Z `drivers/display/display_socionext_dpu.c` (~86-87, ~124-127) | read from source |
| The driver's ISR uses a hard-coded register base `0x4B780000` (`APP_DPU_DC`), not the devicetree address | same file (~17) | read from source |

### 1.2 The SoC-level windows the M7 and Linux agree on

| Finding | Source | Tag |
|---|---|---|
| The Linux remoteproc driver maps the M7 into the A55 view with an address translation table: M7 `0x00000000` (ITCM code) to system `0x203C0000`, 256 KiB, `ATT_OWN \| ATT_IOMEM`; M7 `0x20000000` (DTCM system) to system `0x20400000`, 256 KiB, `ATT_OWN \| ATT_IOMEM`; M7 `0x80000000` (DDR) to `0x80000000`, size `0x50000000`, flags 0 | L `drivers/remoteproc/imx_rproc.c:130-140` (`imx_rproc_att_imx95_m7`); the same table via [`imx95-m7-rproc-load.md`](imx95-m7-rproc-load.md) | read from source |
| The System Manager (SM) M7 logical machine (LM1) is granted, on the NXP EVK config: `M7MIX DATA 0x020380000-0x02047FFFF` (1 MiB window containing both TCM aliases), `M7MIX DATA 0x04A060000-0x04A09FFFF`, and `DDR EXEC 0x080000000-0x089FFFFFF` (160 MiB) | S `configs/mx95evk.cfg` (~426-429) | read from source (EVK config; the Verdin SM config was not read here) |
| DDR as the M7 reaches it starts at `0x80000000` and is identity-mapped by Linux's table above, so a DDR address means the same thing to the M7 and the A55 | L `imx_rproc.c:139` | read from source (table) and inferred (the "same thing" reading) |
| OCRAM in the SoC description: `sram1: sram@204c0000` `0x18000` (96 KiB, `mmio-sram`) and `ocram_c@4af80000` `0x18000` (96 KiB, `no-map`, used by the camera ISP firmware). The SM EVK config lists `OCRAM 0x0204C0000, size=96K` for the A55 non-secure environment, `OCRAM_C 0x001000000-0x001017FFF`, and `OCRAM DATA 0x020480000, size=352K` for LM0 | L `arch/arm64/boot/dts/freescale/imx95.dtsi:373-375, 2124-2126`; S `configs/mx95evk.cfg` (~81, ~539, ~1020-1025) | read from source |
| None of the OCRAM entries appears in the M7 LM1 grants or in Zephyr's M7 devicetree | S `mx95evk.cfg` (LM1 block); Z `nxp_imx95_m7.dtsi` | read from source (absence) |
| The NXP SDK's M7 board linker script (for the NXP EVK, not Verdin): code `0x00001000` (252 KiB) and data `0x20004000` (104 KiB) in TCM, `m_ncache` at `0x80000000` for 128 MiB receiving the `NonCacheable` sections, resource table in DRAM at `0x88220000` | M `_boards/imx95verdinevk/driver_examples/dpu/linker/MIMX9596_cm7_ram.ld`, via [`imx95-mcux-display.md`](imx95-mcux-display.md#framebuffer-memory) | read from source (via that doc) |

So, to the M7: **ITCM 256 KiB at 0x0, DTCM 256 KiB at 0x20000000, DDR from
0x80000000** (read from source). Zephyr's SoC files define no OCRAM; whether
the M7 can reach the OCRAM at `0x204C0000` is **not found** (it is not in the
M7's SM grants).

## 2. What the Verdin iMX95 kernel DT gives the M7

Source K, the BSP 7.8.0 kernel (see [Sources](#sources)), read 2026-11-28:
`imx95-verdin.dtsi` (blob `c25f8e4c7792b3fc0d7d94e8c7afac4e34050f9a`), its
`reserved-memory` block (`:151-217`), the `cm7: remoteproc-cm7` node
(`:44-54`) and `&mu7` (`:569-571`). The Mallow board files add nothing to
these (Sources, step 5). The same content was read earlier at source T and
re-stated in
[`imx95-verdin-bsp-src.md`](imx95-verdin-bsp-src.md#31-verdin-and-mallow-files);
T and K have identical copies of the file. The NXP lf-6.6 EVK dts (L `arch/arm64/boot/dts/freescale/imx95-19x19-evk.dts:52-105`,
`:260-270`) is the comparison: it has the same vdev carveouts but **no**
`m7_reserved` region.

**Confirmed for BSP 7.8.0 at the kernel source** (2026-11-28): every row
below was read at K, the 7.8.0 `SRCREV`. It is not a board observation; the
running DT can still differ if U-Boot or an overlay changes it (see the
`rsc-table` row under "Further facts").

| Region (Verdin DT, K line) | Address | Size | Flags | Tag |
|---|---|---|---|---|
| `m7_reserved: memory@80000000` (`:170-173`) | `0x80000000` | `0x1000000` (16 MiB) | `no-map` | read from source (K); the EVK dts has none (read from source, L) |
| `vdev0vring0` (`:187-190`) | `0x88000000` | `0x8000` (32 KiB) | `no-map` | read from source (K; same address in L EVK dts `:75`) |
| `vdev0vring1` (`:192-195`) | `0x88008000` | `0x8000` | `no-map` | read from source (K; L `:80`) |
| `vdev1vring0` (`:197-200`) | `0x88010000` | `0x8000` | `no-map` | read from source (K; L `:85`) |
| `vdev1vring1` (`:202-205`) | `0x88018000` | `0x8000` | `no-map` | read from source (K; L `:90`) |
| `vdevbuffer` (`:207-211`) | `0x88020000` | `0x100000` (1 MiB), `compatible = "shared-dma-pool"` | `no-map` | read from source (K; L `:100-104`) |
| `rsc_table: rsc-table@88220000` (`:182-185`) | `0x88220000` | `0x1000` (4 KiB) | `no-map` | read from source (K; L `:95`) |
| `linux_cma: linux,cma` (`:162-168`) | `alloc-ranges = <0 0x80000000 0 0x7F000000>`, i.e. `0x80000000`..`0xFEFFFFFF` | `0x3c000000` (960 MiB) | `reusable`, `shared-dma-pool`, `linux,cma-default` | read from source (K; L EVK dts `:62-67` has the same size) |
| `ele_reserved: memory@9c300000` (`:156-160`) | `0x9c300000` | `0x100000` (1 MiB), `shared-dma-pool` | `no-map` | read from source (K); used by `&ele_fw2`, not by the M7 |
| `npu_dma: npu-dma` (`:175-180`) | dynamic (no `reg`, no `alloc-ranges`) | `0x8000000` (128 MiB), alignment 1 MiB | `reusable`, `shared-dma-pool` | read from source (K); used by `&neutron`, not by the M7 |
| `vpu_boot: vpu_boot@a0000000` (`:213-216`) | `0xa0000000` | `0x100000` (1 MiB) | `no-map` | read from source (K); not referenced by the `cm7` node |

Further facts:

| Finding | Tag |
|---|---|
| The `cm7: remoteproc-cm7` node (`compatible = "fsl,imx95-cm7"`, `power-domains = <&scmi_devpd IMX95_PD_M7>`, `mbox-names = "tx", "rx", "rxdb"`) lists `memory-region = <&vdevbuffer>, <&vdev0vring0>, <&vdev0vring1>, <&vdev1vring0>, <&vdev1vring1>, <&rsc_table>, <&m7_reserved>`, mailboxes `&mu7` 0/1/3, `fsl,startup-delay-ms = <50>`; `&mu7` is `status = "okay"` | read from source (K `:44-54`, `:569-571`) |
| `linux_cma`'s allocation range includes `0x80000000`, but `m7_reserved` and the vdev nodes are `no-map` carveouts, so Linux neither maps nor allocates them | read from source (`no-map`, `alloc-ranges`); the consequence is inferred |
| The `0x80000000-0x8FFFFFFF` bytes outside those nodes (the rest of the first 256 MiB; a Toradex patch summary says the first 256 MB of SDRAM is "reserved for Cortex-M7 firmware", summary text only) are **not** carved out in K, so they sit inside `linux_cma`'s range. That includes the 1 MiB gap `0x88120000-0x8821FFFF` between `vdevbuffer`'s end and `rsc_table` | read from source (node list of K); the consequence and the gap arithmetic are inferred; the 256 MB remark was not read in full |
| Linux `imx_rproc` loads ELF segments only into addresses it can map: the TCM windows, and memory-regions whose node name does not start with `vdev` (`m7_reserved`, `rsc_table`). A DDR address outside those fails with "bad phdr da" | read from source ([`imx95-m7-rproc-load.md`](imx95-m7-rproc-load.md): L `imx_rproc.c:765-844`; `drivers/remoteproc/imx_rproc.c` is identical at T and K, 2026-11-28); Toradex user report of `bad phdr da 0x90000000` is consistent (inferred) |
| This repository's earlier snapshot of the live board and the source disagree on the `rsc-table` size (1 MiB versus 4 KiB; see [`imx95-m7-fb-memory.md`](imx95-m7-fb-memory.md#7-interaction-with-linuxs-reserved-memory) and [`imx95-m7-rproc-load.md`](imx95-m7-rproc-load.md)). K, the 7.8.0 source, has 4 KiB, so if the snapshot's 1 MiB holds, something after the kernel build (U-Boot fixup, overlay or a different DTB) changes the running DT | read from source (K size); the explanation is inferred; not resolved here (a board matter) |

Reading (**inferred**): the only DDR the Verdin DT gives the M7 for code and
data is the 16 MiB `m7_reserved` at `0x80000000`, plus the vdev carveouts at
`0x88000000-0x8811FFFF` and the `rsc_table` at `0x88220000-0x88220FFF`, which
are reserved for rpmsg. Zephyr's `m7/ddr` board
claims 42 MiB at `0x80000000` (section 1.1); the 26 MiB beyond `m7_reserved`
lies in `linux_cma`'s range, so that board's window is too large for this DT.

## 3. Can the DPU fetch from each region?

What the sources say about the DPU as a bus master:

| Finding | Source | Tag |
|---|---|---|
| The fetch unit's base address is a 64-bit register pair, `BASEADDRESS` (low 32 bits) and `BASEADDRESSMSB` (high bits); the Linux driver writes both with `lower_32_bits`/`upper_32_bits` | L `drivers/gpu/drm/imx/dpu95/dpu95-fetchunit.h:43-46`, `dpu95-fetchunit.c:174-179` | read from source |
| The Linux DPU driver sets a 40-bit DMA mask | L `drivers/gpu/drm/imx/dpu95/dpu95-core.c:1093` (`dma_set_mask_and_coherent(dev, DMA_BIT_MASK(40))`, line counted back from the file's end) | read from source |
| The Linux driver takes fetch addresses from DRM GEM DMA objects (`drm_fb_dma_get_gem_obj`, `dma_addr`), i.e. CMA/DDR by default | L `dpu95-plane.c` (`drm_plane_state_to_baseaddr`) | read from source |
| The DPU node has no `iommus`, no `memory-region` and no `dma-ranges` (other IPs in the same dtsi do have `iommus = <&smmu ..>`), so the DPU sees system addresses unchanged | L `imx95.dtsi:2607-2750` | read from source (absence) and inferred (reading) |
| The Zephyr DPU driver takes only a 32-bit address (`(uint32_t)(uintptr_t)`), so a Zephyr scanout buffer must have a physical address below 4 GiB | Z `display_socionext_dpu.c` (~124-127) | read from source; the limit is inferred |
| The DPU has its own on-chip memory clock `DISPOCRAM` (`clock-names = "ocram"`); the blit command sequencer uses an OCRAM_D address register (`CMDSEQ_OCRAM_D_ADDR`) | L `imx95.dtsi:2719-2725`, `dpu95-blit.c` (~64) | read from source; that this is not general frame memory is inferred |
| In the SM, the display controller blocks carry `DFMT1` and `MDAC_*` entries, `DC` carries `MBC_D0` entries, and `GPV_DISPLAY` is owned by LM0. No file I read states which masters or address windows the DISPLAYMIX NoC can reach | S `devices/MIMX95/configtool/displaymix.cfg` (~39-66); `configs/mx95evk.cfg` (~236) | read from source; "nothing found" for the NoC reach |
| The A55 non-secure environment's DDR grants in the EVK config are `0x088000000-0x089FFFFFF` and `0x08E000000-0x87FFFFFFF`; the M7's are `0x080000000-0x089FFFFFF`. If the DPU's bus transactions carry the domain of the machine that owns it, then the DPU would be limited to its owner's grants | S `configs/mx95evk.cfg` (~1020-1025, ~429) | read from source (grants); the "DPU inherits its owner's domain" reading is inferred (from `DFMT1`), not stated by a source |
| The A55 grants contain no `M7MIX`/TCM window, and `m7mix.cfg` has `M7MIX did=2-12, perm=none` by default | S `configs/mx95evk.cfg`; `devices/MIMX95/configtool/m7mix.cfg` (~50) | read from source; the consequence for the DPU is inferred |
| The NXP SDK's DPU display examples for the i.MX95 M7 place the framebuffer in DDR (fixed addresses starting at `0x80000000`, or a non-cacheable linker section in DDR), never in TCM | M `_boards/imx95verdinevk/driver_examples/dpu/...`, via [`imx95-mcux-display.md`](imx95-mcux-display.md#framebuffer-memory) | read from source (via that doc) |

Verdict by region (the answer to "can the DPU fetch from it"):

| Region | DPU fetch | Basis | Tag |
|---|---|---|---|
| DDR, in `m7_reserved` (`0x80000000`, 16 MiB) | **expected yes** | 40-bit/64-bit fetch address; same view as the CPU; the M7 is granted this window in the EVK config and the SDK examples scan out of DDR from `0x80000000`. Whether the DPU transactions are permitted there depends on whose domain they carry (EVK: A55 grants do not list `0x80000000-0x87FFFFFF`) | inferred; the Verdin SM config is the open check (item sm-config-read3) |
| DDR, in the rest of `0x80000000-0x8FFFFFFF` or Linux CMA | do not use | Linux may allocate it | inferred |
| M7 DTCM/ITCM (M7 `0x0`/`0x20000000`, system `0x203C0000`/`0x20400000`) | **not found** | no source states a DISPLAYMIX-to-M7MIX path; the A55 grants lack M7MIX; the Zephyr driver never uses a TCM address for the fetch unit. The M7's own TCM address (`0x20000000`) is also not a system address the DPU can use at all (it is the M7-local alias; the system alias is `0x20400000`) | inferred |
| OCRAM `0x204C0000` (96 KiB) | **not found** | granted to the A55 in the EVK config; not in the M7's grants; too small anyway (section 4) | read from source (grants) and inferred |
| OCRAM_C `0x4AF80000` (96 KiB) | not usable | owned by the camera ISP firmware carveout in the Linux DT; too small | read from source (L `imx95.dtsi:2124-2133`); consequence inferred |

## 4. Frame sizes against each region

Panel: 1280x800 (the Toradex 10.1" LVDS panel; see
[`imx95-m7-lvds-route.md`](imx95-m7-lvds-route.md) and
[`imx95-verdin-lvds-panel.md`](imx95-verdin-lvds-panel.md)). Arithmetic
(**inferred**: the sizes follow from the pixel count; the pitch alignment of 32
bytes is the Zephyr driver's `DPU_FB_PITCH_ALIGN`, read from source in Z
`display_socionext_dpu.c` ~21, and both pitches below are already multiples of
32, so no padding is added).

1280 x 800 = 1,024,000 pixels.

| Format | Bytes per pixel | Pitch | One frame | Double-buffered |
|---|---|---|---|---|
| ARGB8888 | 4 | 5120 B | 4,096,000 B = 4000 KiB = 3.906 MiB | 8,192,000 B = 8000 KiB = 7.813 MiB |
| RGB565 | 2 | 2560 B | 2,048,000 B = 2000 KiB = 1.953 MiB | 4,096,000 B = 4000 KiB = 3.906 MiB |

Region capacities and fit (**inferred**: arithmetic against the capacities of
sections 1 and 2):

| Region | Capacity | ARGB8888 x1 | ARGB8888 x2 | RGB565 x1 | RGB565 x2 |
|---|---|---|---|---|---|
| DTCM | 256 KiB (262,144 B) | no (15.6x too big) | no | no (7.8x) | no |
| ITCM | 256 KiB | no | no | no | no |
| OCRAM `0x204C0000` | 96 KiB (98,304 B) | no | no | no | no |
| OCRAM_C | 96 KiB | no | no | no | no |
| M7MIX system window | 1 MiB | no | no | no (1.953 MiB) | no |
| `vdevbuffer` | 1 MiB | no | no | no | no |
| `m7_reserved` | 16 MiB (16,777,216 B) | yes | yes (7.813 MiB; 8.187 MiB left for code, data, stack) | yes | yes (12.1 MiB left) |
| Zephyr `m7/ddr` window | 42 MiB | yes | yes | yes | yes, but 26 MiB of it is outside `m7_reserved` (section 2) |

The stock driver sizes its own buffers as `CONFIG_DPU_FB_NUM` (default 2)
copies at 4 bytes per pixel whatever the pixel format (read from source, Z
`display_socionext_dpu.c` ~339-343): 8,192,000 B for this panel in its default
form, on top of any buffer the application owns (inferred, arithmetic).

Only DDR holds a 1280x800 frame. No TCM or OCRAM does, so the question in
section 3 about TCM and OCRAM reach does not decide the choice.

## 5. Which region a Zephyr scanout buffer should use

**Recommendation (inferred): the 16 MiB `m7_reserved` carveout at
`0x80000000`, both frames in ARGB8888 (7.813 MiB), with the M7 image kept in
ITCM/DTCM.** Reasons, each from the sections above:

1. It is the only DDR the Verdin DT reserves for the M7 and keeps from Linux
   (`no-map`, section 2), so neither Linux nor its CMA touches it.
2. It is a region the Linux loader can write if ever needed (a memory-region
   not named `vdev*`), and it is inside the SM's M7 DDR grant (section 1.2).
3. It fits two ARGB8888 frames with room to spare (section 4).
4. The Zephyr DPU driver passes the CPU address straight to the fetch unit, so
   no address translation is needed (section 1.1).

The image stays in TCM (the `imx95_evk/mimx9596/m7` layout) because Linux can
load TCM windows without any other mapping (section 1.2) and because the stock
`m7/ddr` board's 42 MiB window crosses `m7_reserved`'s end (section 2). The
alternative (image and buffers both in `m7_reserved`, the `m7/ddr` layout with
the window cut to 16 MiB) works too but leaves 8.187 MiB for everything else
and gives the loader a DDR ELF to place.

What that needs (all **inferred**; none of it was tried or built):

| Change | Where | Why |
|---|---|---|
| A board overlay (or the out-of-tree board of item verdin-zephyr-board-def) adding a node such as `fb_mem: memory@80000000` with `compatible = "zephyr,memory-region"`, `reg = <0x80000000 DT_SIZE_M(16)>` (or the framebuffer share only), `zephyr,memory-region = "FRAMEBUF"` and, to avoid dirty cache lines, `zephyr,memory-attr = <DT_MEM_ARM(ATTR_MPU_RAM_NOCACHE)>` | board DTS | the mechanism exists upstream for the ITCM node and as a display precedent ([`imx95-m7-fb-memory.md`](imx95-m7-fb-memory.md#4-what-zephyr-does-for-the-m7s-mpu-and-cache)); whether a second `zephyr,memory-region` over DDR links cleanly was not tried |
| A linker placement of the application's two frame arrays into `FRAMEBUF` (a named section, for example with `Z_GENERIC_SECTION`/an `.overlay`-driven linker region, aligned to 32 bytes) | application and CMake, not the SoC linker script, which has no such region | the stock driver places its buffers in `.bss`; there is no DT knob |
| `CONFIG_DPU_FB_NUM=0` so the stock driver allocates none, and the application hands the driver its own full-screen, x=0, y=0, 32-byte-aligned, matching-pitch buffers (the zero-copy path, section 1.1). If the build rejects a zero-length array, the alternative is a small out-of-tree driver change | application `prj.conf` | otherwise the driver's static arrays (8,192,000 B) stay in DTCM, which cannot hold them (section 4). Kconfig allows `0`; the zero-length array compiling is unverified |
| Either the nocache attribute above, or `sys_cache_data_flush_range()` of the frame before each commit | application | the stock driver does no cache maintenance (section 1.1); the costs are not measured ([`imx95-m7-fb-memory.md`](imx95-m7-fb-memory.md#5-flush-before-commit-versus-a-nocache-region)) |
| MPU: the generic v7-M code programs the chosen `zephyr,sram` and flash regions, and rounds sizes up to a power of two. A 16 MiB region at `0x80000000` is already a power of two, so it fits without rounding; a 7.813 MiB sub-range would round up to 8 MiB at an 8 MiB-aligned base, which `0x80000000` satisfies | board / MPU | per [`imx95-m7-fb-memory.md`](imx95-m7-fb-memory.md#4-what-zephyr-does-for-the-m7s-mpu-and-cache) (read from source there) plus arithmetic |
| Match the Zephyr `reg` to the Linux carveout. If the Verdin SM config or the live DT differs from section 2 (neither was read here; section 2 is the 7.8.0 kernel source), change the address and size here, and in the DT of the board that Linux runs | board DTS and the Linux DT | the carveout is the contract with Linux |
| A DPU-side check before any claim: that the DPU transactions may read `0x80000000-0x80FFFFFF` under the Verdin SM config. The EVK excerpt (section 3) suggests the A55-owned DPU may not; the answer decides whether Linux must hand the DPU to the M7's logical machine first | SM config (item sm-config-read3) | otherwise the fetch can fault or read zeros |

## 6. What this study does not establish

- That the running board's DT matches source K. Section 2 is confirmed at
  the 7.8.0 kernel source (2026-11-28), not on the board; the earlier board
  snapshot's 1 MiB `rsc-table` suggests a later change (**inferred**).
- Whether the DPU may bus-master into `m7_reserved` under the Verdin SM
  configuration, and whether it can reach any M7MIX address (**not found**).
- Any figure from the i.MX95 reference manual itself (not public here).
- That a `zephyr,memory-region` over DDR, `CONFIG_DPU_FB_NUM=0` and the
  linker placement build and run (**inferred**, nothing built).
- Any timing, cache cost or board behaviour: **not measured**.
