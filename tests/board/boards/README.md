# Out-of-tree Zephyr boards

This directory is a Zephyr `BOARD_ROOT`. It holds board definitions that
upstream Zephyr does not have. It is not an application.

| Board target | Directory | What it is |
|---|---|---|
| `verdin_imx95/mimx9596/m7` | [toradex/verdin_imx95/](toradex/verdin_imx95/) | Cortex-M7 of the Toradex Verdin iMX95, running from TCM |

Status (2026-11-28): **host build only, not run on any board.** Nothing in
this directory has been loaded on a Verdin iMX95. Every statement about run
time below is read from source or inferred, not measured.

## Use

Zephyr searches `<BOARD_ROOT>/boards/<vendor>/<board>/`, so the board root
is `tests/board` (the parent of this directory), not this directory:

```sh
cd "$ZEPHYR_WORKSPACE"
west build -p always -b verdin_imx95/mimx9596/m7 -d <build-dir> <repo>/tests/board/imx95_m7 \
    -- -DBOARD_ROOT=<repo>/tests/board -DZEPHYR_SDK_INSTALL_DIR="$ZEPHYR_SDK_INSTALL_DIR"
```

Use a build directory outside `/tmp` and outside the tracked tree (for
example `<repo>/target/...`, which is git-ignored, or `$TMPDIR`).

The artefact for Linux remoteproc is `<build-dir>/zephyr/zephyr.elf`. There
is no `west flash` runner (see `board.cmake`).

## Pinned sources

| Item | Value |
|---|---|
| Zephyr | <https://github.com/zephyrproject-rtos/zephyr>, commit `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (build string `v4.5.0-5218-ge53873cbd0ef`, `VERSION` 4.5.99). The same commit as [../imx95_m7/README.md](../imx95_m7/README.md) |
| Derived from | `boards/nxp/imx95_evk/` at that commit: `board.yml`, `Kconfig.imx95_evk`, `imx95_evk_mimx9596_m7.dts`, `imx95_evk-pinctrl.dtsi` (`lpuart3_default` only), `imx95_evk_mimx9596_m7_defconfig`, `imx95_evk_mimx9596_m7.yaml`, `board.cmake` |
| SoC files used unchanged | `dts/arm/nxp/imx/nxp_imx95_m7.dtsi`, `soc/nxp/imx/imx9/imx95/` |
| hal_nxp | `1a8937f3f8dfe5c68f59cc2a0c0c0fccdb096650` (pinctrl file `dts/nxp/imx/pinctrl/mimx9596avzxn-pinctrl.dtsi`) |
| Zephyr SDK | 1.0.1, `arm-zephyr-eabi`, GCC 14.3.0, binutils 2.43.1 |

Repository notes the choices come from (each pins its own sources):
[imx95-verdin-zephyr-board.md](../../../docs/imx95-verdin-zephyr-board.md)
(what a Verdin board needs),
[imx95-m7-console.md](../../../docs/imx95-m7-console.md) (console UART),
[imx95-verdin-bsp-src.md](../../../docs/imx95-verdin-bsp-src.md) (Toradex
kernel DT, SM logical machines),
[imx95-m7-verdin-bsp.md](../../../docs/imx95-m7-verdin-bsp.md) (M7 memory map).

## `verdin_imx95`: what differs from `imx95_evk/mimx9596/m7`, and why

Tags: **read from source** (pinned file or a repository note that tags it
so), **inferred** (my deduction).

| Item | EVK M7 target | `verdin_imx95` M7 target | Why | Tag |
|---|---|---|---|---|
| Targets | A55, A55 SMP, M7, M7 `ddr`, M7 `flash` | M7 (TCM) only | Only the M7 next to Linux is in scope. `ddr` uses `0x80000000` 42 MiB, but Linux on the Verdin reserves only 16 MiB (`m7_reserved`) there; `flash` uses the EVK NOR | read from source (Verdin DT in imx95-verdin-bsp-src.md); scope: inferred |
| Identity | `nxp,imx95_evk`, "NXP i.MX95 EVK board" | `toradex,verdin-imx95`, "Toradex Verdin iMX95 (Cortex-M7)" | own board | inferred |
| SoC part | `MIMX9596AVZXN` | same | the only i.MX95 part with a pinctrl file in hal_nxp at the pin. Whether the Verdin module uses that package is **not checked** | read from source (hal_nxp listing); fit: not checked |
| Memory | `zephyr,flash = &itcm` (`0x0`, 256 KiB), `zephyr,sram = &dtcm` (`0x20000000`, 256 KiB) | same | These are the M7-side TCM addresses that the Linux `imx_rproc` table maps for the M7, so remoteproc can load the ELF without a DDR carve-out | read from source (imx95-verdin-zephyr-board.md §3); match: inferred |
| Console | `lpuart3`, 115200, pads `GPIO_IO14` (TX) / `GPIO_IO15` (RX) | same | NXP's EVK SM config gives `LPUART3` and both pads to the M7 logical machine (LM1), and the Verdin/Mallow Linux DT does not use `lpuart3`. **Caveat:** on the Verdin these pads are UART_2 CTS/RTS (SODIMM 143/141), which Linux on Mallow muxes for `lpuart8`; whether Mallow routes them to a header is unknown; the Toradex SM config was not read. Expect no M7 UART output on Mallow until that is settled; the remoteproc trace buffer is the planned log path (imx95-m7-console.md §6) | read from source (EVK SM cfg, Verdin DT, datasheet, as cited in imx95-m7-console.md); outcome: inferred |
| `lpuart5` | enabled | disabled (SoC default) | EVK-only second UART | inferred |
| System timer | `lptmr1` (32768 Hz, clock `IMX95_CLK_LPTMR1` over SCMI), 1024 ticks/s | Cortex-M SysTick at the 800 MHz core clock, 1000 ticks/s (`zephyr,system-timer` deleted from `chosen`) | SysTick is inside the core and needs no System Manager resource; LPTMR1 needs a clock and ownership that the Verdin SM may not grant | read from source (`Kconfig.defconfig.mimx95.m7`); SM part: inferred |
| SCMI (`mu5`, `scmi_shmem0`) and defconfig | `ARM_SCMI`, `ARM_SCMI_NXP_VENDOR_EXTENSIONS`, `MBOX`, `MBOX_INIT_PRIORITY=0`, `CLOCK_CONTROL`, `SERIAL`, `UART_CONSOLE`, `UART_INTERRUPT_DRIVEN`, `CONSOLE`, `XIP` | same | The LPUART3 clock and pin mux are SCMI requests to the System Manager. That the Verdin SM exposes the same SCMI channel to the M7 is not read | read from source; Verdin SM: not read |
| `mu7` (rpmsg mailbox) | disabled | disabled | Linux on the Verdin uses `mu7` for the M7 remoteproc node; an application that wants rpmsg enables it in its own overlay, as `tests/board/imx95_m7` does for the EVK | read from source (Verdin DT) |
| Ethernet (`emdio`, `enetc_psi0`, `enetc_ptp_clock`), CAN (`flexcan1`, `can_phy1`, `zephyr,canbus`), FlexSPI NOR (`flexspi`, `ext_flash_ctrl`, `zephyr,flash-controller`), `lpi2c2/4/5/6/7` with the PCAL6408/PCAL6416 expanders and the WM8962 codec, `lpspi1`, `sai3`/`sai3_2`, PDM, `tpm2`, `tstmr2`, `edma1/2`, `gpio2..5`, `sar_adc`, `wdog5`, aliases `watchdog0`, `i2s-tx`, `i2s-codec-tx` | enabled | not present / disabled (SoC default) | EVK carrier parts. On the Verdin they are absent or owned by Linux; enabled drivers would probe at boot and ask the SM for pads and clocks the M7 may not own (upstream `soc.c` also sets the FlexCAN clock for each enabled FlexCAN node) | inferred (imx95-verdin-zephyr-board.md §5) |
| Display: `dpu`, `mipi_dsi`, `disp_irqsteer`; labels `zephyr_dpu`, `zephyr_mipi_dsi`, `display_i2c` | disabled; labels present | disabled; **no labels** | Display stays off. In NXP's EVK SM config the DPU, LVDS, DSI and DISPLAYMIX belong to the A55 machine, and Linux drives the panel on this board. Upstream Zephyr has no LVDS/LDB node. Without the labels, display samples and shields do not attach to this board by accident | read from source (imx95-verdin-bsp-src.md §2, imx95-verdin-zephyr-board.md §2) |
| `board.c` | empty `board_init()` | none | it does nothing | read from source |
| `Kconfig`, `Kconfig.defconfig` | NXP common SPSDK/DDR options; CAN/PCA init priorities | none | EVK boot-image and CAN options do not apply | read from source |
| Runner (`board.cmake`) | `spsdk` (EVK `flash.bin`), `jlink` for A55 | none | The M7 image is loaded by Linux remoteproc (`scripts/imx95-m7-rproc.sh`) | read from source |
| Twister (`.yaml`) | `supported`: adc, can, counter, dma, gpio, i2c, eth, ptp, pwm, spi, uart, watchdog | `supported`: uart | only the console is enabled | inferred |
| Resource table | linker hook only (`CONFIG_OPENAMP_RSC_TABLE`) | same, no table in the board | The table is application code; the Verdin `rsc_table` region is `0x88220000`, the same as the EVK overlay in `tests/board/imx95_m7` | read from source |

## Host build result

Built 2026-11-28 on an x86_64 Linux host with the pins above:

```sh
west build -p always -b verdin_imx95/mimx9596/m7 -d <repo>/target/zephyr-verdin-imx95-m7 \
    <repo>/tests/board/imx95_m7 \
    -- -DBOARD_ROOT=<repo>/tests/board -DZEPHYR_SDK_INSTALL_DIR="$ZEPHYR_SDK_INSTALL_DIR"
```

Result: builds, no compiler or devicetree warnings. CMake reports
`Board: verdin_imx95, qualifiers: mimx9596/m7` and takes the dts from this
directory. Memory report as printed by the link:

```
Memory region         Used Size  Region Size  %age Used
           FLASH:       26724 B       256 KB     10.19%
             RAM:        8324 B       256 KB      3.18%
            ITCM:           0 B       256 KB      0.00%
        IDT_LIST:           0 B        32 KB      0.00%
```

`FLASH` is the ITCM image (code, read-only data, initialised-data copy) and
`RAM` is DTCM, as for the EVK target. The `ITCM` row is the separate
`itcm` memory-region section, which this program does not use.

Configuration of that build (from the generated `.config`):
`CONFIG_CORTEX_M_SYSTICK=y`, `CONFIG_SYS_CLOCK_HW_CYCLES_PER_SEC=800000000`,
no OpenAMP (the application's rpmsg settings are in
`tests/board/imx95_m7/boards/imx95_evk_mimx9596_m7.conf`, which applies only
to the EVK target), so `CONFIG_HWLM_RPMSG` is off and the program prints its
report on the console UART only. `CONFIG_HWLM_DPU_PROBE` is off. Of the
peripherals the EVK board enables, only `lpuart3` is enabled here; the SCMI
transport nodes come from the SoC dtsi unchanged.

For comparison, `tests/board/imx95_m7/README.md` records 27,180 B ITCM and
8,364 B DTCM for the EVK target with `CONFIG_HWLM_RPMSG=n`.

## Open points

- The Toradex System Manager configuration for the Verdin iMX95: which
  LPUART, pads and SCMI channel the M7 logical machine owns. Until it is read,
  the console choice is the EVK's.
- Whether the Verdin module uses the `MIMX9596AVZXN` package.
- rpmsg for this board: `tests/board/imx95_m7` has no
  `boards/verdin_imx95_mimx9596_m7.{conf,overlay}` yet.
- Nothing here has run on a board.
