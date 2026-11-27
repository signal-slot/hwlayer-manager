# Verdin iMX95 Cortex-M7 under Zephyr: what a board would need (source study)

Study date: 2026-11-28. Method: reading upstream Zephyr source and the
repository's existing i.MX95 notes. No device was accessed, nothing was built,
and no board file exists. This file makes no hardware claim.

Tags on every finding:

- **read from source**: I read it in the pinned upstream file (see
  [Sources](#sources)), or in a repository note that itself tags it as read
  from source (named in the row).
- **inferred**: my deduction; no source says it directly.
- **not read**: a source that would settle it was not read.

Upstream Zephyr has no Verdin iMX95 board. The only i.MX95 M7 target is NXP's
EVK, `imx95_evk/mimx9596/m7`. The Verdin side comes from
[imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md) (Toradex-authored upstream
patches and Toradex pages, not the BSP tree) and the other `docs/imx95-*.md`
notes. `docs/imx95-verdin-bsp-src.md` does not exist on this branch; anything
that would need the Toradex BSP device tree or SM configuration is marked
*not read*.

## 1. The EVK M7 target, as upstream ships it

All rows: **read from source**, Zephyr tag `v4.5.0`.

| Item | Finding | File |
|---|---|---|
| Board identity | `board.yml`: board `imx95_evk`, SoC `mimx9596`, variants `smp` (A55), `ddr` (M7), `flash` (M7). Test identifier `imx95_evk/mimx9596/m7`, `ram: 256`, `flash: 256` (KiB), toolchains `zephyr`, `gnuarmemb` | `boards/nxp/imx95_evk/board.yml`, `imx95_evk_mimx9596_m7.yaml` |
| Memory used by the base target | `zephyr,flash = &itcm`, `zephyr,sram = &dtcm`: ITCM `0x00000000` 256 KiB (compatible `zephyr,memory-region`, `nxp,imx-itcm`), DTCM `0x20000000` 256 KiB (`nxp,imx-dtcm`) | `imx95_evk_mimx9596_m7.dts`; `dts/arm/nxp/imx/nxp_imx95_m7.dtsi` |
| Console | `zephyr,console` and `zephyr,shell-uart` = `&lpuart3`, 115200 baud, `lpuart3` is `serial@42570000`, interrupt `<64 3>`, clock `IMX95_CLK_LPUART3` | board dts; SoC dtsi |
| Console pads | `lpuart3_default`: `GPIO_IO15` as LPUART3 RX and `GPIO_IO14` as LPUART3 TX, pull-up | `imx95_evk-pinctrl.dtsi` |
| Other chosen / aliases | `zephyr,flash-controller = &ext_flash_ctrl`, `zephyr,canbus = &flexcan1`, `watchdog0 = &wdog5`, `i2s-tx`/`i2s-codec-tx = &sai3_2`; system timer `&lptmr1` (SoC dtsi) | board dts; SoC dtsi |
| Peripherals the board enables | emdio, `enetc_psi0` (RTL8211F PHY), `flexcan1` (TJA1463 via a PCAL6416 expander hog on LPI2C6), FlexSPI NOR (MT35XU01GBBA, 1 Gbit, mapped partitions), LPI2C2/4/5/6/7, LPSPI1, LPUART3, LPUART5, SAI3, PDM pinmux, TPM2, LPTMR1, TSTMR2, eDMA1/2, GPIO2..5, SAR ADC, WDOG5. Two I2C devices (PCAL6408 and a WM8962 codec on LPI2C4) | board dts |
| Display labels | `zephyr_dpu: &dpu {}`, `zephyr_mipi_dsi: &mipi_dsi {}`, `display_i2c: &lpi2c2 {}`. Nothing is enabled, no timings, no `zephyr,display` | board dts |
| Defconfig | `CONFIG_CLOCK_CONTROL`, `SERIAL`, `UART_CONSOLE`, `UART_INTERRUPT_DRIVEN`, `CONSOLE`, `XIP`, `MBOX` (`MBOX_INIT_PRIORITY=0`), `ARM_SCMI`, `ARM_SCMI_NXP_VENDOR_EXTENSIONS` | `imx95_evk_mimx9596_m7_defconfig` |
| Flash/run method | `board.cmake` has only the `spsdk` runner (builds a container `flash.bin`) and, for the A55 SoC, `jlink`. Nothing for remoteproc or `bootaux` | `board.cmake` |
| Documented M7 load path | The board doc describes only an i.MX container `flash.bin` (System Manager, ELE+V2X firmware, TCM OEI, M7 image) booted from SD/eMMC or loaded over USB download mode. It does not mention U-Boot `bootaux` or Linux remoteproc for the M7 (the A55 section lists remoteproc) | `boards/nxp/imx95_evk/doc/index.rst` |
| SCMI | Under `/firmware/scmi`: shared memory `scmi_shmem0` at `0x44611000` (size `0x80`), mailbox `mu5` (`0x44610000`, interrupt 205) channel `tx`; protocols 0x11 power, 0x12 system, 0x14 clock, 0x19 pinctrl, 0x82 `nxp,scmi-cpu` | SoC dtsi |
| Mailbox units | `mu5` enabled (SCMI). `mu7` at `0x42440000`, interrupt 207, `status = "disabled"` | SoC dtsi |
| SoC Kconfig | `SOC_MIMX9596_M7` selects Cortex-M7, FPU (double), I and D caches, MPU, `SOC_EARLY_INIT_HOOK`. `CACHE_MANAGEMENT` defaults on; `soc_early_init_hook()` enables the data and instruction caches | `soc/nxp/imx/imx9/imx95/Kconfig`, `Kconfig.defconfig.mimx95.m7`, `m7/soc.c` |
| Interrupt model | Multi-level interrupts: `NUM_IRQS` 250, one 2nd-level aggregator at offset 234 (IRQSTEER); the DISPLAYMIX IRQSTEER masters hang off NVIC lines 214..219 | `Kconfig.defconfig.mimx95.m7`; SoC dtsi |
| Linker | `SOC_LINKER_SCRIPT` is `m7/linker.ld`: the Cortex-M script plus a `.resource_table` output section (`KEEP(*(.resource_table*))`, in the ROMABLE region) **only if** `CONFIG_OPENAMP_RSC_TABLE`. There is no memory-region file of its own; regions come from the chosen nodes | `soc/nxp/imx/imx9/imx95/m7/{CMakeLists.txt,linker.ld}` |
| DDR and flash variants | `board.yml` lists `ddr` and `flash` variants and the board doc builds `imx95_evk/mimx9596/m7/ddr`. The variant dts files were **not read** by me; [imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md) section 2.2 records (as read from source, v4.5.0) a DDR node at `0x80000000` of 42 MiB and a NOR window at `0x28000000` | not read here; secondary note |

Correction of one claim in the earlier note: it says Zephyr has "no resource
table for i.MX95". What v4.5.0 has is the **linker hook** for a
`.resource_table` section behind `CONFIG_OPENAMP_RSC_TABLE`; the table
contents are still application code. *(read from source; the earlier note's
"no table in the board directory" is consistent with that.)*

## 2. Display-related pieces in the SoC dtsi (what a board could enable)

All **read from source**, `nxp_imx95_m7.dtsi` and `m7/soc.c`, `v4.5.0`.

| Node / code | Finding |
|---|---|
| `dpu: dpu@4b400000` | compatible `socionext,dpu`, `height = <0>`, `width = <0>`, register window `0x1000`, interrupt `<215 0>` (NVIC), **no clocks, no power-domains, no pinctrl property**, `status = "disabled"` |
| `mipi_dsi: mipi-dsi@4acf0000` | compatible `nxp,mipi-dsi-dwc`, `nxp,dc = <&dpu>`, interrupt via `disp_irqsteer_master0` line 48, `clocks = <&scmi_clk 0>`, disabled |
| `disp_irqsteer` | `0x4b0b0000`, offset 874, 512 interrupts, `power-domains = <&disp_mix>`, masters 0..4 and 7 on NVIC 214..219, all disabled |
| `disp_mix` | `/power-domains/power-domain@d`, compatible `arm,scmi-power-domain`, SCMI power-domain id 0xd |
| LVDS / LDB | **no node in the dtsi**. In the listings I read, `dts/bindings/mipi-dsi` has only `nxp,imx-mipi-dsi`, `nxp,mipi-dsi-2l`, `nxp,mipi-dsi-dwc` among NXP files; `dts/bindings/display` has `nxp,dcnano-lcdif`, `nxp,imx-elcdif`, `nxp,imx-lcdifv2`, `nxp,imx-lcdifv3` and `socionext,dpu`; `display/panel` has only `zephyr,panel-color-palette` and `zephyr,panel-timing`. The `display` listing was cut off partway through the `s` entries, so a binding named after a later prefix is not excluded |
| `soc_init()` display code | Built only `#if defined(CONFIG_MIPI_DSI_NXP_DWC)`: powers up DISPLAYMIX and CAMERAMIX through SCMI power protocol (`IMX95_PD_DISPLAY`, `IMX95_PD_CAMERA`), writes `DPU_IRQSTEER->CHN_MASK[13]`, programs the DSI pixel-link select and the display-blk-ctrl pixel-link enable, then sets VIDEOPLL1 and the `DISP1PIX`, `MIPIPHYPLLBYPASS`, `MIPITESTBYTE` clocks through SCMI. **Nothing equivalent exists for LVDS** (no LDB clock, no LDB register setup) |

Reading across with [imx95-m7-backend-contract.md](imx95-m7-backend-contract.md):
the `socionext,dpu` driver itself calls no clock, power or SCMI function, so
the board (or application) has to bring the DISPLAYMIX domain and pixel clock
up before the first DPU register access. *inferred*, combining the dtsi above
with that note's reading of the driver.

## 3. What differs for a Verdin iMX95 on a Mallow carrier

Source for the Verdin column: [imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md)
unless named otherwise. Tag per row.

| Aspect | EVK M7 target | Verdin iMX95 on Mallow | Tag |
|---|---|---|---|
| Board identity | `compatible = "nxp,imx95_evk"`, model "NXP i.MX95 EVK board" | needs its own model/compatible | inferred |
| Console UART instance | LPUART3, pads `GPIO_IO14`/`GPIO_IO15` | LPUART3 is **not** used by Linux in the Verdin DT (Linux uses LPUART1, 2, 6, 7, 8). The alt-core console is on a USB-debug-port channel per Toradex. So LPUART3 is the likely M7 console, but no Verdin source names it | inferred; Linux's use of LPUART1/2/6/7/8: read from source (Toradex U-Boot and Linux patches) |
| Console pins and header | EVK pinctrl above | which SODIMM/Mallow pins carry the M7 console, and whether `GPIO_IO14/15` reach any: **not found** by that note; Verdin datasheet and Mallow schematic **not read** | not read |
| Whether the Verdin SM grants LPUART3 and its pads to the M7 | EVK config grants `LPUART3`, `PIN_GPIO_IO14`, `PIN_GPIO_IO15` to LM1 (M7) | Toradex SM configuration **not read** | not read |
| Memory: M7 code and data | ITCM `0x0`, DTCM `0x20000000`, 256 KiB each | Linux's `imx_rproc` table maps the M7 view `0x0` to `0x203C0000` and `0x20000000` to `0x20400000` (256 KiB each) and DDR `0x80000000` size `0x50000000`. The base target's ITCM/DTCM layout matches those M7-side addresses | read from source (Linux patch) for the table; match: inferred |
| DDR carve-out | EVK DDR variant: 42 MiB at `0x80000000` (secondary note) | Linux Verdin DT reserves `m7_reserved` `0x80000000` 16 MiB `no-map`. A DDR variant must shrink to fit. The BSP 6.6 tree's final node names: not read | read from source (Toradex U-Boot patch v2; Linux patches); fit: inferred |
| Resource table | none in the board; linker hook only | Linux reserves `rsc_table` at `0x88220000`, 4 KiB; the driver hardcodes that address; vring 0/1 for vdev0 and vdev1 at `0x88000000..0x8801FFFF`, 1 MiB buffer pool at `0x88020000`. A Toradex page example shows an ELF booting with `no dtb rsrc-table`, so a table may be optional | read from source (patches); optional: read from source, generic example |
| Mailbox / rpmsg | `mu7` present but disabled, `mu5` for SCMI | Linux `remoteproc-cm7` uses `mu7` with `tx`, `rx`, `rxdb` channels; the M7 side would enable Zephyr's `mu7` and keep `mu5` for SCMI. Pairing is inferred. The imx-sm EVK config gives the M7 `MU7_B`, `MU5_A`, `MU8_B` | read from source (EVK config; Linux node); pairing: inferred |
| Boot of the M7 | `flash.bin` via SPSDK, SD/eMMC or USB | Toradex documents U-Boot `bootaux` from `0x203c0000` and Linux remoteproc with an ELF (default path `/lib/firmware`, settable via `firmware_class`). Neither is a `west flash` runner | read from source (Toradex pages) |
| SCMI to the SM | `mu5` + `scmi_shmem0` at `0x44611000` | works only if the Verdin SM exposes the same SCMI channel to the M7 LM. Not read | not read |
| Display nodes enabled | none | see section 4 | |

## 4. DPU, LDB and panel nodes a Verdin board would enable

Facts about the display on this board come from
[imx95-dpu-source-6.6.md](imx95-dpu-source-6.6.md) (Linux 6.6 tree reading)
and [imx95-sm-display-lm.md](imx95-sm-display-lm.md) (SM configuration, an
NXP-EVK reading).

1. **Route.** On Linux the display chain is DPU, pixel interleaver, display
   pixel link, LDB (LVDS display bridge) with one channel per LVDS PHY, panel.
   The Toradex panel is LVDS, not MIPI-DSI. The 10.1" overlay shape in NXP's
   tree is one LDB channel, single link, 1280x800 at about 71.1 MHz.
   *read from source* (those notes); that the board's panel is that panel:
   *inferred* by those notes.
2. **Zephyr has no LDB path.** The dtsi has `dpu` and `mipi_dsi`, the SoC init
   code programs only the DSI pixel link, and I found no LVDS/LDB binding in
   the directories I listed (section 2). A Verdin LVDS board therefore needs
   new code beyond a devicetree: LDB register setup and its PLL clock, the
   pixel-link/interleaver routing, and the DPU's own clock and power
   handling. *inferred* from section 2.
3. **Nodes a board dts would enable or add** (all *inferred*):
   `&dpu` with real `width`/`height` and a clock and power-domain it does not
   have today; `&disp_irqsteer` with the masters that carry the DPU's frame
   interrupt; the `disp_mix` power domain through SCMI; a node for the LDB
   and its channel (new binding); a panel timing node (the stock
   `zephyr,panel-timing` binding exists); a backlight/enable GPIO or PWM
   (Linux uses a PWM backlight in the NXP panel dts, Mallow wiring **not
   read**); `chosen { zephyr,display = &dpu; }`.
4. **Ownership blocks all of this.** In NXP's EVK System Manager
   configuration the display controller, LVDS, DSI and DISPLAYMIX belong to
   the A55 logical machine, not the M7 ([imx95-sm-display-lm.md](imx95-sm-display-lm.md)).
   Whether the Verdin SM matches is **not read**. A Linux-booted Verdin
   already uses the display through the DPU driver and Weston, so by the
   board rules in [imx95-board.md](imx95-board.md) the M7 cannot take the
   display from a running Linux without changing the boot flow or the SM
   configuration. The ways to try that are for the display-handover design,
   not this file.

## 5. Overlay on `imx95_evk/mimx9596/m7`, or an out-of-tree board?

Recommendation: **overlay for the first application, an out-of-tree board
before any display work.** Both are *inferred*; nothing was built.

An overlay (`boards/imx95_evk_mimx9596_m7.overlay` in the application) can
work for a hello/probe image because devicetree lets an overlay override
`chosen`, aliases and `status`:

- It can keep `zephyr,console = &lpuart3` or point it at another LPUART, and
  set `zephyr,flash = &itcm`, `zephyr,sram = &dtcm` (already so), and enable
  `&mu7`, `&dpu` and the DSI/IRQSTEER nodes.
- Zephyr already links the image at ITCM/DTCM, which is what remoteproc
  expects (section 3).

Why it stops being enough:

1. The board dts enables a large EVK-only set: `flexcan1` with a PHY whose
   enable/standby pins are on an I2C expander, two I2C expanders with
   `gpio-hog` nodes, a codec, Ethernet with a PHY, FlexSPI NOR with
   partitions, SAI3 and WDOG5. Those drivers probe at boot and use pads,
   clocks and peripherals the Verdin SM may not give the M7. An overlay would
   have to switch each off with `status = "disabled"` and re-point
   `zephyr,canbus`, `zephyr,flash-controller` and the `watchdog0`, `i2s-tx`
   aliases. That is possible but fragile, and a missed node can stop boot in
   an SCMI or I2C call. *inferred.*
2. `compatible`, model and the SPSDK `board.cmake` runner describe the EVK;
   `west flash` would try to build an EVK container. A Verdin board needs its
   own `board.cmake` with no runner or a remoteproc helper script.
   *inferred.*
3. Display work needs board-level code the EVK board does not have: the LVDS
   route, the panel node and the init sequence in section 2. The SoC-level
   `soc_init()` display code is a `CONFIG_MIPI_DSI_NXP_DWC` block in the SoC
   directory, which an application cannot change; LVDS bring-up would live in
   the application or an out-of-tree module. *inferred.*

The `imx95_evk` board itself cannot be edited (non-goal: no upstream change),
so the out-of-tree board reuses `nxp_imx95_m7.dtsi` and the SoC support, and
the application layer carries board files under `tests/board/`.

## 6. Proposed file list (not created)

For the work item that owns `tests/board/imx95_m7/` (running at the time of
writing). Paths are proposals; the application directory name is theirs.

```
tests/board/imx95_m7/
  CMakeLists.txt                       # sets BOARD_ROOT to the directory below
  prj.conf                             # CONSOLE, SERIAL, optional DISPLAY
  src/main.c                           # hello, then a DPU register/ownership probe
  boards/imx95_evk_mimx9596_m7.overlay # stage 1: overlay on the EVK target
  zephyr/module.yml                    # declares board_root for stage 2
  boards/nxp_like/verdin_imx95/        # stage 2: out-of-tree board
    board.yml                          # board verdin_imx95, soc mimx9596, cpucluster m7
    Kconfig.verdin_imx95               # BOARD_VERDIN_IMX95 selects SOC_MIMX9596_M7
    verdin_imx95_mimx9596_m7.dts       # includes nxp/imx/nxp_imx95_m7.dtsi
    verdin_imx95_mimx9596_m7-pinctrl.dtsi  # console pads, from the Verdin datasheet
    verdin_imx95_mimx9596_m7_defconfig # as the EVK defconfig, minus unused drivers
    verdin_imx95_mimx9596_m7.yaml      # twister metadata (ram 256, flash 256)
    board.cmake                        # no runner; build the ELF for remoteproc
    rsc_table.c                        # .resource_table at the Linux rsc_table address
    memory_map.md                      # the carve-out table of section 3, kept in step with Linux
```

Contents to decide before writing them:

- `board.yml` and the Kconfig layout follow the hardware-model-v2 pattern used
  by `boards/nxp/imx95_evk` (read from source: `board.yml` with socs and
  variants).
- `verdin_*.dts` console: LPUART3 unless the Verdin SM and Mallow wiring are
  read first (section 3). Disable or omit every EVK-only peripheral.
- `rsc_table.c`: sets `CONFIG_OPENAMP_RSC_TABLE` so the SoC linker emits the
  section; the table's vring and buffer addresses are copied from section 3.
  Whether a table is needed at all is open (section 3). *inferred.*
- Anything display-related (`&dpu`, LDB, panel) goes in a separate overlay
  file so the console-only board still builds without the display route.

## 7. Open points

| Point | State |
|---|---|
| Verdin SM configuration: which LM owns LPUART3, its pads, the DPU, LVDS, DISPLAYMIX, and whether the M7 LM can start/stop | not read |
| Toradex BSP kernel DT and `imx-boot` contents | not read |
| M7 console pins on Verdin / Mallow | not read |
| EVK `m7/ddr` and `m7/flash` variant dts | not read here |
| Mallow LVDS panel wiring, backlight, enable GPIO | not read |
| Zephyr `dts/bindings/display` entries after `solomon` | not read (listing cut off) |
| Linux-side confirmation on the running board | out of scope (no board access) |

## Sources

Zephyr, <https://github.com/zephyrproject-rtos/zephyr>, tag `v4.5.0`, commit
`41d10b59730406bbab4ca9f1fbbe9e96aafb4966`. The commit is the one the release
page for the tag links; I did not open the commit page to confirm the tag
object. Files read over HTTPS from
`https://raw.githubusercontent.com/zephyrproject-rtos/zephyr/v4.5.0/` (read
2026-11-28):

- `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts`
- `boards/nxp/imx95_evk/imx95_evk-pinctrl.dtsi`
- `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7_defconfig`
- `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.yaml`
- `boards/nxp/imx95_evk/board.cmake`, `board.yml`, `doc/index.rst`
- `dts/arm/nxp/imx/nxp_imx95_m7.dtsi`
- `soc/nxp/imx/imx9/imx95/{Kconfig,Kconfig.soc,Kconfig.defconfig.mimx95.m7,CMakeLists.txt}`
- `soc/nxp/imx/imx9/imx95/m7/{CMakeLists.txt,linker.ld,soc.c,soc.h}`
- directory listings of `dts/bindings/mipi-dsi`, `dts/bindings/display` (cut
  off) and `dts/bindings/display/panel`

The files were read through a page-fetching helper, because `git clone` fails
on farm nodes; the text is verbatim as returned, not checked against a local
checkout.

Repository notes used (each pins its own sources):
[imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md),
[imx95-dpu-source-6.6.md](imx95-dpu-source-6.6.md),
[imx95-sm-display-lm.md](imx95-sm-display-lm.md),
[imx95-m7-backend-contract.md](imx95-m7-backend-contract.md),
[imx95-board.md](imx95-board.md).
Other notes this file relies on for context, not for facts:
[imx95-m7-scmi.md](imx95-m7-scmi.md),
[imx95-m7-fb-memory.md](imx95-m7-fb-memory.md).
