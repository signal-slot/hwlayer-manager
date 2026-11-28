# Upstream Zephyr support for the AM62P MCU R5F (source study)

Date: 2026-11-28. No device was used and nothing was built. The showcase
(`MCUI_SHOWCASE_DIR`) was not read.

## What was read

- **Zephyr `v4.5.0`**, the version pinned by `docs/zephyr-workspace.md` and
  `scripts/prepare-zephyr.sh`. The tag is annotated (tag object
  `d6e5dc6ae358a6bbe05d040e0428e4161f09d006`, tagger date 2026-10-28). It points to
  commit **`41d10b59730406bbab4ca9f1fbbe9e96aafb4966`**.
- `main` of `zephyrproject-rtos/zephyr` on 2026-11-27 was
  `bc833f469ee653b7755881b8850792a4e14bacbb` ("MAINTAINERS: Remove my from TI K3
  Platforms"). Only that commit was looked at. Nothing below is a statement about `main`.
- Toradex's public repositories, by GitHub's organisation page and API (see (a)).
- The board test: `docs/board-test-am62p-r5f.md`, `tests/board/am62p_r5f/prj.conf` and
  `src/hw_am62p.c` on branch `work/am62p-board-test` (tip `361e31f`).

How the files were read: first through a web fetch of raw files and GitHub's contents
API at the tag, then the trace-entry, resource-table linking and display-driver
statements were checked in a local shallow checkout of `v4.5.0` (kept outside this
repository). Statements resting on a truncated or summarised listing say so.

Tags: **source** = read in a file at commit `41d10b59` (path given, relative to the
Zephyr tree); **inferred** = concluded from sources, not read.

## (a) Boards and SoC entries

| Statement | Tag |
|---|---|
| There is **no AM62P SoC** and no AM62P board upstream: `soc/ti/k3/` has `am62lx`, `am64x`, `am62x`, `j721e`, `j722s` and `common`; `soc/ti/k3/soc.yml` lists no `am62p` series; `dts/arm64/ti/` and `dts/arm/ti/` have no `am62p` file. | source: `soc/ti/k3/`, `soc/ti/k3/soc.yml`, `dts/arm/ti/`, `dts/arm64/ti/` |
| There is **no Verdin AM62P board**. `boards/toradex/` has `colibri_imx7d`, `verdin_am62`, `verdin_imx8mm`, `verdin_imx8mp`. | source: `boards/toradex/` |
| `boards/toradex/verdin_am62` is the **AM62 (AM6234) M4F** only: one variant, `verdin_am62_am6234_m4`. Its doc says Zephyr is ported to the M4F core and never mentions R5F. | source: `boards/toradex/verdin_am62/` |
| The **J722S** SoC (the AM67A family; the AM62P is a sibling SoC) is present with exactly two cpu clusters, both Cortex-R5F: `main_r5f0_0` and `mcu_r5f0_0`. It has no A53 cluster. | source: `soc/ti/k3/soc.yml`, `soc/ti/k3/j722s/Kconfig.soc` |
| The only board on J722S is `boards/beagle/beagley_ai` (BeagleY-AI), with qualifiers `beagley_ai/j722s/mcu_r5f0_0` (MCU domain R5F, `SOC_J722S_MCU_R5F0_0`) and `beagley_ai/j722s/main_r5f0_0` (MAIN domain R5F, `SOC_J722S_MAIN_R5F0_0`). `boards/ti/` has no J722S or AM67 board. | source: `boards/beagle/beagley_ai/board.yml`, `boards/ti/` |
| The qualifier shape `<board>/<soc>/mcu_r5f0_0` is the same shape as the board test's `verdin_am62p/am62p54/mcu_r5f0_0`. The SoC name differs (`j722s` vs `am62p54`). | source (both names) |
| Whether the AM62P's MCU R5F is close enough to J722S's to reuse `j722s_mcu_r5.dtsi` is not answered by upstream. It is the nearest upstream starting point. | inferred |
| Toradex: no public Zephyr repository for the Verdin AM62P was found. The organisation page for query `zephyr` showed 0 repositories. The API listing was truncated after about 19 repositories, so absence is **not proven**. | source (GitHub organisation page, 2026-11-28); the rest inferred |

### SoC entry `j722s` (R5F) at `v4.5.0`

- `soc/ti/k3/j722s/Kconfig`: `SOC_SERIES_J722S_R5` selects `ARM`, `CPU_CORTEX_R5`,
  `CPU_HAS_ARM_MPU`, `VFP_SP_D16`, `ARM_CUSTOM_INTERRUPT_CONTROLLER`, `TI_VIM`,
  `UART_NS16550_ACCESS_WORD_ONLY` (if `UART_NS16550`) and `SOC_EARLY_INIT_HOOK`. **source**
- `soc/ti/k3/j722s/Kconfig.defconfig`: `KERNEL_ENTRY` `_vector_table`, `NUM_IRQS` 512,
  `BUILD_OUTPUT_BIN` n, `UART_NS16550_VARIANT_NS16750` when `SERIAL`. **source**
- `soc/ti/k3/common/cortex_r/` has `arm_mpu_regions.c`, `boot.S`, `linker.ld`, `soc.c`,
  `soc.h`; contents not read. **source** (listing)
- `VFP_SP_D16` means the SoC entry selects single-precision VFP with 16 registers.
  The board test's `prj.conf` sets `CONFIG_FPU=y` and `CONFIG_FP_HARDABI=y`; whether the
  two fit the engine library's `armv7r-none-eabihf` (vfpv3-d16 in the doc's compile
  flags) was not checked. **inferred**

### Memory layout of `beagley_ai/j722s/mcu_r5f0_0` and `main_r5f0_0`

R5F-side addresses. TCM from `dts/arm/ti/j722s_mcu_r5.dtsi`; DDR nodes from the board
`.dts` files.

| Region | MCU R5F | MAIN R5F | Note |
|---|---|---|---|
| `atcm` | `0x0`, 32 KB, `zephyr,memory-region = "ATCM"` | same | `j722s_mcu_r5.dtsi`, `j722s_main_r5.dtsi` |
| `btcm` | `0x41010000`, 32 KB, `"BTCM"` | same | same files |
| `ddr0` (mmio-sram, IPC shared memory, `zephyr,ipc_shm`) | `0xa1000000`, 1 MB | `0xa2000000`, 1 MB | `beagley_ai_j722s_*_r5f0_0.dts` |
| `rsc_table` (`zephyr,memory-region = "RSC_TABLE"`) | `0xa1100000`, 1 MB | `0xa2100000`, 1 MB | same |
| `ddr1` (`zephyr,sram`) | `0xa1200000`, 14 MB | `0xa2200000`, 14 MB | same |

All of this is **source**. The board doc gives the A53's view of the TCMs (MCU R5F ATCM
`0x79000000`, BTCM `0x79020000`; MAIN R5F `0x78400000`, `0x78500000`) and says the DDR
region is 16 MB at `0xa1000000` (MCU) and `0xa2000000` (MAIN): `boards/beagle/beagley_ai/doc/index.rst`.
**source**

These DDR addresses belong to BeagleY-AI's Linux carve-outs (4 GB board). A Verdin AM62P
carve-out is a device-tree fact of the Toradex BSP, not of upstream. **inferred**

### Resource table and remoteproc

- Config: `CONFIG_OPENAMP_RSC_TABLE=y`, commented as "Enable OpenAMP resource table
  needed for remoteproc booting". It is set in `beagley_ai_j722s_mcu_r5f0_0_defconfig`
  and the MAIN twin (identical file), and in the defconfigs of `sk_am62` (M4) and
  `verdin_am62` (M4). **source**: `boards/beagle/beagley_ai/`, `boards/ti/sk_am62/`,
  `boards/toradex/verdin_am62/`
- The `rsc_table` memory node exists in the dts, marked `"RSC_TABLE"`. **source**
- There is **no flash runner** for the BeagleY-AI R5Fs (no `board.cmake`). The doc loads
  them from Linux on the A53: copy `zephyr.elf` to `/lib/firmware/`, then
  `echo stop` to `/dev/remoteproc/am67a-{main,mcu}-r5f0_0/state`, `echo zephyr.elf` to
  `.../firmware`, `echo start` to `.../state`. **source**: `boards/beagle/beagley_ai/doc/index.rst`
- The doc's console is a UART (UART1 on header pins 8 and 10) and it states "Future
  versions will also support a console over RPmsg". **source** (same file). There is no
  trace-buffer console documented upstream for these boards. **inferred** (absent from the doc)
- The resource table **has a trace entry** when `CONFIG_RAM_CONSOLE=y`: in
  `subsys/ipc/open-amp/resource_table.h`, `struct fw_resource_table` gets a
  `struct fw_rsc_trace cm_trace` ("rpmsg trace entry") pointing at `ram_console_buf`
  with size `CONFIG_RAM_CONSOLE_BUFFER_SIZE`; `resource_table.c` declares
  `extern char ram_console_buf[]`. This is the console route the board test's `prj.conf`
  uses. **source** (checkout of `v4.5.0`)
- The resource table is linked in section `.resource_table` for `SOC_SERIES_J722S_R5`
  too: `soc/ti/k3/common/CMakeLists.txt` (the branch for `AM64X_R5 OR J721E_R5 OR
  J722S_R5`). **source**

## (b) Drivers for the MCU R5F

| Function | Upstream at `v4.5.0` | Tag |
|---|---|---|
| Interrupt controller (VIM) | Present. Driver `drivers/interrupt_controller/intc_vim.c`; `Kconfig.vim` defines `CONFIG_TI_VIM` (`default y`, `depends on DT_HAS_TI_VIM_ENABLED`, inside `if CPU_CORTEX_R5`). Binding compatible `ti,vim`; node `interrupt-controller@7ff0000` (reg `0x07ff0000 0x2800`, `#interrupt-cells = <4>`: IRQ/FIQ, number, type, priority) in `j722s_mcu_r5.dtsi`; the MAIN R5F's VIM is at `0x2fff0000`. | source |
| UART | The generic `ns16550` driver, not a TI driver. MCU UART `mcu_uart0` at `0x04a00000`, VIM IRQ 217, disabled by default (`j722s_mcu.dtsi`); MAIN `uart0`..`uart6` at `0x02800000 + n*0x10000`, IRQ 210..216 (`j722s_main.dtsi`). The SoC selects `UART_NS16550_ACCESS_WORD_ONLY` and the NS16750 variant. | source |
| Mailbox | `ti,omap-mailbox`: `drivers/mbox/mbox_ti_omap.c`, `CONFIG_MBOX_TI_OMAP_MAILBOX`. MCU R5F node `mbox1` at `0x29010000`, VIM IRQ 241, `usr-id = <2>`; MAIN R5F `mbox3` at `0x29030000`, IRQ 116, `usr-id = <3>`. Also present: `ti,secure-proxy` (`mbox_ti_secproxy.c`, `CONFIG_MBOX_TI_SECURE_PROXY`); no J722S node uses it. | source |
| IPC / OpenAMP | Boards use `ipc0` (`zephyr,mbox-ipm`) on the mailbox with `zephyr,ipc_shm` = `ddr0` (1 MB) and `CONFIG_OPENAMP_RSC_TABLE=y`. The `samples/subsys/ipc/` directory has `ipc_service`, `openamp`, `openamp_rsc_table`, `rpmsg_service`. The IPC-service RPMsg backend symbols and the samples' platform lists were **not read** (the fetches failed). | source for the nodes and the sample directories; the rest not read |
| System timer | `ti,am654-timer`, node `systick_timer` at `0x04800000` (MCU R5F, IRQ 28) and `0x02470000` (MAIN R5F, IRQ 35), chosen as `zephyr,system-timer`. Driver `drivers/timer/ti_dmtimer.c`, `CONFIG_TI_DM_TIMER` (`default y`, selects `TICKLESS_CAPABLE`). `drivers/timer/ti_rti_timer.c` also exists; not opened. | source |
| Watchdog | `ti,j7-rti-wdt` node `mcu_rti0` at `0x04880000`, enabled by the MCU R5F board. | source |
| GPIO | `ti,davinci-gpio`: `mcu_gpio0` at `0x04201010` (24 pins, disabled); MAIN banks at `0x600010` and others, disabled. | source |
| Pin control | `ti,k3-pinctrl` node `pinctrl@f4000` in `j722s_main.dtsi`. | source |
| I2C | No I2C node in any of the four J722S dtsi files. (`ti,omap-i2c` exists for the AM62 M4 in `am62x_m4.dtsi`.) The board test sets `CONFIG_I2C=y`. | source |
| **Display (DSS)** | **None.** `drivers/display/` has no file or Kconfig whose name contains `dss`, `tidss`, `dispc`, `am62`, `am65` or `j722`. `dts/bindings/mipi-dsi/` (complete listing) has no `ti,` entry. `dts/bindings/display/` shows no TI entry in its visible part. The four J722S dtsi files contain no `dss`, `dispc` or `dsi` node. File contents of `drivers/display/` were not searched, and part of the listing was summarised, so "no TI DSS driver" rests on names. | source (names); contents inferred |

DSI hosts that exist upstream are for other vendors: `espressif,esp-mipi-dsi`,
`nxp,imx-mipi-dsi`, `nxp,mipi-dsi-2l`, `nxp,mipi-dsi-dwc`, `renesas,ra-mipi-dsi`,
`renesas,rz-mipi-dsi`, `st,stm32-mipi-dsi`, `st,stm32u5-mipi-dsi`. **source**:
`dts/bindings/mipi-dsi/`. Bridge and panel drivers exist in `drivers/display/`
(for example `display_adv7535.c`, `display_waveshare_dsi2dpi.c`, `display_ili9881c_dsi.c`),
but they need a MIPI-DSI host driver, and none exists for the AM62P. **source** (names), **inferred** (the dependency)

## (c) What upstream lacks that the board test takes from `MCUI_SHOWCASE_DIR`

`docs/board-test-am62p-r5f.md` says `MCUI_SHOWCASE_DIR` provides the board target
`verdin_am62p/am62p54/mcu_r5f0_0` and the DSS driver, and that yakogl provides the board
`verdin_am62p` and the AM62P SoC; it also lists `modules/hal/ti` and
`modules/hal/libmetal` and `lib/open-amp` as modules. `src/hw_am62p.c` includes
`<mcui/am62p_dss.h>`. Against upstream at `41d10b59`, these are absent:

1. **The AM62P SoC** (`am62p54`, `mcu_r5f0_0`) and **the Verdin AM62P board**: neither
   exists upstream (see (a)). The nearest upstream parts are J722S's `mcu_r5f0_0` SoC
   entry and `verdin_am62` (M4F, other SoC). **source** (absence)
2. **A DSS driver.** `src/hw_am62p.c` calls `am62p_dss_set_app_buffer`,
   `am62p_dss_enable_app_plane`, `am62p_dss_safety_flip`, `am62p_dss_safety_buffer`,
   `am62p_dss_safety_front`, `am62p_dss_safety_premultiplied`, `am62p_dss_commit_count`,
   `am62p_dss_latched_count` and `am62p_dss_set_vsync_callback`, and the formats
   `AM62P_DSS_FMT_RGB565`, `_XRGB8888`, `_ARGB8888`. Upstream has nothing behind any
   of them (no DSS driver, no vsync interrupt hook for it, no commit/latched counters).
   **source** (absence; call list from the branch)
3. **A MIPI-DSI host driver and the bridge/panel binding for the Verdin DSI output**
   (the doc names "the DSI host, the bridge and panel" as coming with the board).
   No TI DSI host binding upstream. **source** (absence)
4. **The Verdin AM62P memory plan on the R5F side**: DDR carve-outs for the firmware,
   the resource table and the two display buffers, and the cache/MPU attributes of
   the buffers the DSS reads. Upstream's J722S values are BeagleY-AI's. The doc already
   says "check that the R5F's addresses of `.bss` are the addresses the DSS reads".
   **inferred**
5. **A trace-buffer console is not a gap**: `CONFIG_RAM_CONSOLE` produces the trace
   entry in the resource table (see (a)). **source**
6. **Known to be present upstream and therefore not a gap**: the R5 architecture
   support (`CPU_CORTEX_R5`), VIM, ns16550, the mailbox, the J722S timer, OpenAMP
   resource table Kconfig, and `lib/open-amp`/`libmetal` as west modules (the
   modules are in the doc's list). **source** for the drivers; the module remark is the
   doc's statement, not read upstream.

Not established: that the AM62P differs from J722S in the MCU R5F's peripheral map
(VIM base, timers, mailbox IRQs). A TRM check is needed. **inferred**

## (d) What this means for M3-02 and M3-05

(`docs/open-issues.md`: M3-02 "no firmware, build files or board configuration in this
repository use the backend"; M3-05 "the link in a real firmware build".)

- **Could the firmware build depend on upstream Zephyr only?** Not today. The image
  needs a board, an SoC entry and a DSS driver; upstream has none of the three for the
  AM62P (a, c). **inferred** from the absences above.
- **What upstream does give**, and what would not need to be written: the R5 core, VIM,
  ns16550 UART, the timer, the OMAP mailbox, OpenAMP resource-table support and
  remoteproc loading as documented for BeagleY-AI. **source**
- **What would have to be written** for an upstream-only build (**inferred**):
  1. SoC and board files for `am62p54` / `verdin_am62p`: SoC series Kconfig and
     defconfig (start from `soc/ti/k3/j722s/`), `soc.yml`, a dtsi for the MCU R5F (start
     from `j722s_mcu_r5.dtsi`), a board dts with the carve-outs of the Verdin BSP's
     device tree. Not a Toradex file: none is public (a).
  2. A DSS driver with the interface `struct ce_twoplane_hw` needs: two planes (safety
     ARGB8888 with a front/back flip, application RGB565/XRGB8888), a GO-bit commit
     counted per commit, a latched counter advanced at the frame start, a vsync
     callback. This is the register-level work of the DSS, covered by
     the GO-bit study (plan items `am62p-dss-gobit-source`, `am62p-dss-sharing-source`).
  3. A DSI host and bridge/panel setup for the Verdin's display output, or a
     decision to use whatever output the Linux side leaves configured (what is stopped or
     unbound is in `docs/remoteproc-bringup.md`).
- **Smaller route for M3-05**: the link question (the backend plus engine in a
  firmware image) does not need the DSS driver. It needs a firmware that links the two.
  An upstream-only link check could use `beagley_ai/j722s/mcu_r5f0_0`, which is
  in the pinned tree, with `backends/twoplane` and the engine's static library for
  `armv7r-none-eabihf`, and a stub `struct ce_twoplane_hw` (the host test's fake driver is
  of this kind). It proves the toolchain and ABI fit, not the AM62P. **inferred**; not tried
  (this item builds nothing).
- Licence and placement: the pin is `v4.5.0`; any new SoC/board files would be Apache-2.0
  Zephyr files, which fits keeping them in this repository as a Zephyr module
  rather than upstreaming. **inferred**

## Open questions (not answered by what was read)

1. The AM62P MCU R5F's register map against J722S's, from the TRM.
2. (Answered, see (a): trace entry exists with `CONFIG_RAM_CONSOLE`.) Whether the entry's
   address is one Linux can map on the AM62P is a Torizon/kernel question.
3. The IPC-service RPMsg backend symbols and sample platform lists.
4. `main` after `41d10b59`: nothing was read beyond its head commit.
5. The complete list of Toradex repositories.
