# i.MX95 Cortex-M7 under Zephyr: scanout buffer memory, cache and MPU rules

Design note for the M2 scanout app (SoW §5.1 "Zephyr second", WP3). Written
2026-11-28, host only: no board was accessed, nothing was built, and
**nothing was measured**. This note gives no timing figure; every cost below is
qualitative and is to be measured later under `docs/m2-comparison-plan.md`.

## 1. Sources and tags

Every row is tagged one of:

- **read from source (`<ref>`)**: read in a checkout this session.
- **observed (date of the snapshot, here 2026-11-27)**: taken from a dated snapshot already in this repository.
- **inferred**: reasoning from the above or from general Arm knowledge that was
  not re-read here. Public Arm/NXP documents were not fetched in this session
  (no network access was granted to this task), so Arm/NXP statements are
  tagged inferred and must be re-checked against the cited document before
  they are relied on.

Pinned Zephyr source (the checkout in the machine's dependency cache, read
2026-11-28; the requested fresh shallow clone into `$TMPDIR` was refused by
the sandbox, so the existing checkout was read instead):

- Repository: `https://github.com/zephyrproject-rtos/zephyr`
- Tag: `v4.5.0` (annotated tag object `d6e5dc6ae358a6bbe05d040e0428e4161f09d006`)
- Commit: `41d10b59730406bbab4ca9f1fbbe9e96aafb4966` (`VERSION` reads 4.5.0)

Below, `<ref>` is `zephyr v4.5.0 (41d10b59)`. Paths are relative to the Zephyr
root. Repository evidence used: `crates/composition-core/src/presentation.rs`
(read only) and `docs/imx95-zephyr/observed-2026-11-27.txt` (observed on the
Verdin iMX95, 2026-11-27).

Not read: the NXP i.MX95 Reference Manual, the Arm Cortex-M7 TRM, the Arm
ARMv7-M Architecture Reference Manual, CMSIS `core_cm7.h`, and the Toradex
kernel device tree source (only its observed runtime form).

## 2. What the engine needs from memory

`presentation.rs` defines Writable → Ready → Submitted → Scanning, with
`Presented` and `Released` events after a latch. For a buffer the DPU fetches
from system memory this means (inferred, from that file's state definitions):

| Transition | Memory obligation on the M7 |
|---|---|
| Writable → Ready (`finish_write`) | all CPU writes to the buffer are visible to the DPU's bus master, i.e. D-cache lines for the buffer are clean (or the buffer is never cached) |
| Ready → Submitted | the DPU register write that selects this address; must come after the clean, with a barrier between |
| Submitted → Scanning (latch) | the DPU shadow-load event reported by the driver |
| Scanning → Released | only after a later latch replaced it; the CPU may then write it again |

## 3. Regions usable for a framebuffer

| Region | Fact | Tag |
|---|---|---|
| M7 ITCM | 256 KiB at M7 address 0x0, `zephyr,memory-region = "ITCM"` and the default `zephyr,flash` of the TCM board | read from source (`<ref>`: `dts/arm/nxp/imx/nxp_imx95_m7.dtsi`, `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts`) |
| M7 DTCM | 256 KiB at 0x20000000, the default `zephyr,sram` of the TCM board | read from source (same files) |
| DDR window | the `…/m7/ddr` board sets `zephyr,sram` to a `memory@80000000` node of 42 MiB and deletes `zephyr,flash`; the board doc lists it as the DDR variant | read from source (`<ref>`: `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7_ddr.dts`, `…/doc/index.rst`) |
| OCRAM | no OCRAM node exists in the i.MX95 M7 devicetree of `<ref>` (searched for `ocram`, none) | read from source (`<ref>`: `nxp_imx95_m7.dtsi`) — absence of a node, not absence of the hardware |
| OCRAM address, size, DPU reach | not found in any source read | not found |
| DPU bus master reaching M7 TCM | **not found.** Zephyr's DPU driver never takes a TCM address for the fetch unit; the board's only large-buffer variant is DDR. No NXP document was read that states whether the display NoC masters can address the M7 TCM aliases | not found |
| TCM capacity | a 256 KiB DTCM cannot hold a panel-sized 32-bit buffer (the driver sizes its buffers for 4 bytes per pixel per `DPU_FRAMEBUFFER_DECL`; a panel of order 1280x800 needs about 4 MB per buffer) | inferred (arithmetic; panel size is the Toradex panel's, see `docs/imx95-board.md`, not re-checked here) |

Consequence (inferred): the framebuffer is in DDR. The stock `imx95_evk/mimx9596/m7`
TCM board cannot hold it; the M2 app needs the DDR variant or its own board
overlay. If TCM reach is ever wanted, it must first be proven on the device;
until then **a buffer must not be placed in TCM**, as the task's rule states.

The driver's own framebuffers are a `static` array in the image's normal RAM
(`__aligned(32)`, `CONFIG_DPU_FB_NUM` copies, default 2), so on the DDR board
they live wherever the linker puts `.bss` inside the 42 MiB window
(read from source, `<ref>`: `drivers/display/display_socionext_dpu.c`,
`drivers/display/Kconfig.socionext_dpu`). They are cacheable (section 4).

## 4. What Zephyr does for the M7's MPU and cache

| Item | Fact | Tag |
|---|---|---|
| Cache support | `CPU_HAS_ICACHE`, `CPU_HAS_DCACHE`, `CPU_HAS_ARM_MPU`, `ARM_MPU` are selected for `SOC_MIMX9596_M7`; `CACHE_MANAGEMENT` defaults to y | read from source (`<ref>`: `soc/nxp/imx/imx9/imx95/Kconfig`, `Kconfig.defconfig.mimx95.m7`) |
| Cache enabled at boot | `soc_early_init_hook()` calls `sys_cache_data_enable()` and `sys_cache_instr_enable()` | read from source (`<ref>`: `soc/nxp/imx/imx9/imx95/m7/soc.c`) |
| Cache backend | `ARCH_CACHE` (the core's own SCB controller) is the default type; its range operations are CMSIS `SCB_CleanDCache_by_Addr`, `SCB_InvalidateDCache_by_Addr`, `SCB_CleanInvalidateDCache_by_Addr`; `arch_dcache_flush_all` is `SCB_CleanDCache`; `arch_dcache_invd_all` returns `-ENOTSUP` | read from source (`<ref>`: `arch/Kconfig`, `arch/arm/core/cortex_m/cache.c`) |
| Line size | `DCACHE_LINE_SIZE` defaults to 32 unless the CPU node has `d-cache-line-size`; the i.MX95 M7 node has none | read from source (`<ref>`: `arch/Kconfig`, `nxp_imx95_m7.dtsi`) |
| MPU region set | the SoC provides no MPU region table; the generic `arch/arm/core/mpu/arm_mpu_regions.c` gives: region 0 flash (only with `CONFIG_XIP`), region 1 the chosen `zephyr,sram`, both from the chosen node | read from source (`<ref>`: `arch/arm/core/mpu/arm_mpu_regions.c`; no `mpu_regions` file under `soc/nxp/imx/imx9/imx95/`) |
| SRAM attribute | `REGION_RAM_ATTR` = Normal, outer+inner write-back, write/read allocate, non-shareable, read/write for privileged, XN only if `CONFIG_XIP`; `CONFIG_ARM_MPU_SRAM_WRITE_THROUGH` switches it to write-through | read from source (`<ref>`: `include/zephyr/arch/arm/mpu/arm_mpu_v7m.h`, `arch/arm/core/mpu/Kconfig`) |
| Region size on v7-M | the size is rounded up to a power of two (`arm_mpu_v7_internal.h`), and the SRAM size macro selects the next step up. So the 42 MiB DDR window becomes a 64 MiB region at 0x80000000, **cacheable write-back, and larger than the DT node** | read from source (`<ref>`: `arch/arm/core/mpu/arm_mpu_v7_internal.h`, `arm_mpu_mem_cfg.h`) for the rounding; the 64 MiB figure is inferred from it |
| Nocache via DT | with `CONFIG_MEM_ATTR` (default y with `ARM_MPU`), a DT node with `zephyr,memory-attr = <DT_MEM_ARM(ATTR_MPU_RAM_NOCACHE)>` is programmed as `REGION_RAM_NOCACHE_ATTR` (Normal, non-cacheable, XN, privileged RW). A DT region must not also carry `DT_MEM_CACHEABLE` | read from source (`<ref>`: `arch/arm/core/mpu/arm_mpu.c`, `arm_mpu_v7m.h`, `subsys/mem_mgmt/Kconfig`) |
| Precedent | `boards/shields/rk055hdmipi4ma0/boards/mimxrt700_evk_mimxrt798s_cm33_cpu0.overlay` marks a PSRAM region the display DMA reads as `ATTR_MPU_RAM_NOCACHE` ("keep it out of the CPU cache"). A different SoC; it shows the mechanism only, no parity is claimed | read from source (`<ref>`) |
| Nocache via linker | `CONFIG_NOCACHE_MEMORY` (needs `ARCH_HAS_NOCACHE_MEMORY_SUPPORT`, which Cortex-M gets with MPU + D-cache) adds a `nocache` section (`__nocache` attribute, `_nocache_ram_start/size` symbols); the MPU init cleans-and-invalidates the D-cache before reprogramming when it is on | read from source (`<ref>`: `arch/Kconfig`, `arch/arm/core/Kconfig`, `include/zephyr/linker/section_tags.h`, `arch/arm/core/mpu/arm_core_mpu.c`, `arm_mpu.c`) |
| Linker-section size | the `__nocache` section is inside the image's RAM region; as the image's `.bss` is already in the 64 MiB cacheable region, the later-programmed nocache region only takes effect because the higher-numbered MPU region wins on overlap | inferred (Arm ARMv7-M overlap rule, not re-read; region order per `arm_mpu.c` lines near 533 reads static regions first, DT regions after) |
| `zephyr,memory-region` | `nxp,imx-itcm` already uses `compatible = "zephyr,memory-region"`; the same compatible creates a named linker region that `zephyr,memory-attr` can attribute. Whether a DDR sub-window can be done this way on the DDR board without a board overlay edit was not tried | read from source (`<ref>`: `nxp_imx95_m7.dtsi`, `dts/bindings/base/zephyr,memory-region.yaml`) for the compatible; the DDR use is inferred |
| Stock DPU driver | **performs no cache maintenance.** `drivers/display/display_socionext_dpu.c` contains no `sys_cache_*` call (searched `cache`: none). `dpu_write()` either adopts the caller's buffer pointer (when 32-byte aligned, full-frame, same pitch) or `memcpy`s into its static buffer, then writes the fetch address and triggers the shadow load, and waits on a semaphore given from the shadow-load interrupt | read from source (`<ref>`: `drivers/display/display_socionext_dpu.c`) |

The last row is the core finding: with the stock driver on the write-back
SRAM region, the DPU can fetch stale DDR contents, unless a caller flushes the
buffer first. Nothing in the driver or the board files does this for it (read);
whether it already works through some other mechanism on hardware is not known
(not measured, not run).

## 5. Flush before commit versus a nocache region

| | `sys_cache_data_flush_range` before commit | nocache region (DT `zephyr,memory-attr` or `__nocache`) |
|---|---|---|
| Mechanism | clean by address over the buffer's lines (`SCB_CleanDCache_by_Addr`), then a barrier (CMSIS issues DSB/ISB inside; not re-read) | MPU attribute Normal non-cacheable; writes go to memory, never held as dirty lines |
| Where cost is paid | once per frame, proportional to the lines actually dirty plus the loop over the range; none if the producer did not write | on every CPU access to the buffer (reads and writes bypass the cache) |
| Cost shape | scales with range size and dirty fraction. A whole-cache clean (`sys_cache_data_flush_all`/`SCB_CleanDCache`) is bounded by the cache size, not the buffer size, and is the cheaper form when the buffer is larger than the cache (inferred) | no per-frame cost; large CPU-side drawing and read-modify-write (blending, partial redraw) pay uncached access cost (inferred) |
| Fit | full-frame produce-then-flip (render in cacheable memory or the buffer, flush once, commit); also keeps the buffer cacheable for CPU reads | write-mostly producers and the minimal bring-up, where correctness matters more than CPU cost |
| Risk | forgetting a flush, a range that is not line-aligned (a clean of a neighbour's partial line is harmless, since clean never discards data; an *invalidate* would be unsafe), a write after the flush | alignment and power-of-two region rules of v7-M MPU, XN attribute, region count; an oversize or mis-sized region also changes neighbouring data |
| Measured cost | **not measured** | **not measured** |

Neither cost is quantified here, and no figure is claimed. M2's comparison
plan should measure both on the board, not assume either.

## 6. What "latched" must mean for release when the CPU may hold dirty lines

All of this is inferred from the engine's state model and the driver source.

1. **Clean precedes submit.** At `finish_write`/`submit` every line of the
   buffer that the CPU wrote must be written back to DDR (flush by range, then
   barrier) before the DPU address register write. After that the CPU must not
   write the buffer until `Released`.
2. **"Latched" is the DPU shadow-load event**, not the register write: the
   stock driver's only completion signal is `kDPU_Group0ExtDst0ShadowLoadInterrupt`
   from the ISR (read from source). Until then the old buffer is still being
   fetched, so it must not be released; after it, the old buffer is no longer
   fetched (inferred from the shadow-load model; the DPU register manual was
   not read).
3. **Why dirty lines matter at release.** A released buffer returns to the
   producer. If a dirty line of it were still in the cache, a later eviction
   could write stale data over memory the producer has since rewritten, or,
   if the buffer is released wrongly early, over memory the DPU is scanning.
   Rule 1 (no CPU writes between clean and `Released`) leaves no dirty lines at
   release, so **no invalidate is needed** for a buffer only the CPU writes and
   the DPU reads. An invalidate-by-range is needed only if a device writes the
   buffer (a DPU write-back path, capture); that is not part of M2 scanout.
4. **Speculative reads** by the M7 cannot dirty a line, so they do not break
   rule 3; they can make a cached line stale if something else writes memory,
   which again only matters for device-written buffers.
5. **Dropped submissions.** A submission replaced before latch (`Dropped`) was
   already cleaned; its buffers return to `Ready`/`Writable` without further
   cache work, but the producer must again clean after rewriting.
6. The `Released` event must be emitted after the latch event, not at ISR entry
   of an unrelated interrupt; the stock ISR gives one semaphore for any
   shadow-load flag (read from source), which is enough for a single plane.

## 7. Interaction with Linux's reserved memory

The Toradex Linux on the same board (observed 2026-11-27, from
`docs/imx95-zephyr/observed-2026-11-27.txt`, a read-only snapshot):

- `reserved-memory/memory@80000000`: 0x1000000 (16 MiB), `no-map`, at the bottom
  of DDR (observed 2026-11-27; the Toradex source names it `m7_reserved`, read
  from source in [`imx95-m7-memory-map.md`](imx95-m7-memory-map.md)).
- `vdev0vring0@88000000`, `vdev0vring1@88008000`, `vdev1vring0@88010000`,
  `vdev1vring1@88018000` (each 0x8000, 32 KiB), `vdevbuffer@88020000`
  (0x100000, 1 MiB, `shared-dma-pool`) and `rsc-table@88220000` (0x1000,
  **4 KiB**), all `no-map` (observed 2026-11-27, `reg` bytes in the snapshot).
- `memory@9c300000` (0x100000, 1 MiB, `shared-dma-pool`, `no-map`),
  `optee_core@8c000000` (0x2000000, 32 MiB, `no-map`),
  `optee_shm@8e000000` (0x200000, 2 MiB, `no-map`) and `vpu_boot@a0000000`
  (0x100000, 1 MiB, `no-map`) (observed 2026-11-27); `npu-dma` is a reusable
  `shared-dma-pool` of 0x8000000 (128 MiB, no fixed address).
- `linux,cma`: 0x3c000000 (960 MiB) `shared-dma-pool`, `reusable`,
  `alloc-ranges` 0x80000000 + 0x7f00000000 (observed 2026-11-27).
- The `remoteproc-cm7` node (`fsl,imx95-cm7`) has seven `memory-region`
  phandles, 0xa0 to 0xa6 (observed 2026-11-27). The snapshot has no phandle
  values for the reserved-memory nodes, so the mapping is not observed; the
  Toradex source lists, in order, `vdevbuffer`, `vdev0vring0`, `vdev0vring1`,
  `vdev1vring0`, `vdev1vring1`, `rsc_table`, `m7_reserved` (read from source,
  [`imx95-m7-memory-map.md`](imx95-m7-memory-map.md)); that the board's
  phandles follow that order is inferred.
- `remoteproc-cm7` was reported `unsupported` for runtime PM and no
  `/lib/firmware` M7 image existed on that board (observed 2026-11-27).

Inferred consequences (nothing below was read from the kernel DT source):

- The `…/m7/ddr` board links the Zephyr image into 42 MiB starting at
  0x80000000. Linux has a 16 MiB `no-map` region there, so the first 16 MiB
  is the `m7_reserved` carve-out the BSP source assigns to the M7's
  `memory-region` (observed 2026-11-27 for the 16 MiB `no-map` node; its role
  is read from source; that it is the code area the loader uses is inferred). The
  remaining 26 MiB of the Zephyr window lies where Linux's CMA may hand out
  pages. **A Zephyr image or framebuffer there can corrupt Linux, or be
  corrupted by it,** unless the memory map is changed to fit the BSP's
  carve-out or a new carve-out is added. Both need the Linux DT, which this
  task cannot change.
- The M7 MPU's 64 MiB cacheable region (section 4) is larger than the window
  Zephyr believes it owns. The MPU does not enforce system-level ownership;
  whether the system fabric lets the M7 touch Linux memory at all is a System
  Manager/resource-domain question that was not examined here.
- A framebuffer must therefore sit **inside a carve-out Linux does not map or
  allocate** (a `no-map` region), at an address the M7 and the DPU can both
  reach. Where the DPU master can address, and whether the DPU belongs to the M7
  at all, is the ownership question of `docs/imx95-zephyr/README.md` and the
  other items of plan #455; this note does not settle it.
- Linux's DPU driver also uses DDR for its own scanout; the two must never use
  the same display at once (see the ownership item).

## 8. Recommendation for the M2 scanout app

All inferred from the above; to be confirmed on the board before being called
true.

1. **Use DDR for the framebuffer**, never TCM, until a document or a probe
   shows the DPU can fetch from it. State the TCM reach as "not found" in the
   M2 report.
2. **Place the buffers in a `no-map` carve-out agreed with the Linux side**,
   with a sub-window the app sizes itself: buffer base aligned to 32 bytes
   (the driver's pitch alignment and the line size) and, if it is to be mapped
   nocache, to its own power-of-two size and alignment for the v7-M MPU.
   Do not reuse the default 42 MiB window at 0x80000000 without the check in
   section 7.
3. **First bring-up: nocache buffer region** (`zephyr,memory-attr =
   <DT_MEM_ARM(ATTR_MPU_RAM_NOCACHE)>` on the buffer node). It makes
   "latched" the only synchronisation problem and removes cache maintenance as a
   variable while scanout and VSYNC are proven.
4. **Then, for the comparison and anything CPU-heavy: cacheable buffers with an
   explicit `sys_cache_data_flush_range()` of each buffer before every commit**,
   wrapped in one function that the app's backend calls at Ready → Submitted,
   with the buffer 32-byte aligned and sized in whole lines. Do not rely on the
   stock driver for this: it does no maintenance (section 4), so the app (or an
   out-of-tree driver patch) must.
5. **Make both modes a build option** and record in the M2 report which was used
   for each measurement; compare them by measurement only (not measured here).
6. **Release only on the shadow-load event**, after which the producer may
   rewrite; do not invalidate for CPU-written buffers.
7. If the stock `dpu_write()` is used, pass a full-frame, 32-byte-aligned buffer
   with the panel's pitch so the driver adopts the pointer and does not `memcpy`;
   `CONFIG_DPU_FB_NUM=0` avoids a 2x framebuffer array in `.bss`. Its
   `CONFIG_DPU_FB_NUM` help text recommends exactly this (read from source).
8. Set `ARM_MPU_CM7_UNMAPPED_REGION` only after the buffer and DPU registers are
   mapped (it adds a catch-all no-access region; read from source,
   `arch/arm/core/mpu/Kconfig`). It is an option, not a requirement.

## 9. Open items

| Item | Why open |
|---|---|
| DPU master reach of M7 TCM and OCRAM | not found in anything read; needs the i.MX95 reference manual or a board probe |
| Which carve-out the Toradex BSP gives the M7 | observed (2026-11-27 snapshot) only as phandle numbers; needs the kernel DT source or `dtc` |
| System Manager rights of the M7 over DPU, clocks, power domains and memory | outside this note; see the ownership items of plan #455 |
| Cost of flush vs nocache | not measured; to be done under `docs/m2-comparison-plan.md` |
| Whether stock DPU output works with a cacheable buffer, uncleaned | not run; the source suggests it can show stale data |
