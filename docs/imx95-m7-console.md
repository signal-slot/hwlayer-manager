# i.MX95 Cortex-M7 console on Verdin iMX95 / Mallow: which UART, and how to read the log

Study date: 2026-11-28. Method: source reading only. No board was accessed.
This file makes no hardware claim: nothing here was run or seen on a board.

Every finding carries one of these tags:

- **read from source**: a pinned source (see [Sources](#sources)) says so.
- **inferred**: my deduction from those sources. No source says it directly.
- **not found**: I looked and could not read it.

Source IDs such as S1 refer to the [Sources](#sources) list. This file does
not repeat SoC register or memory addresses. Look them up in the pinned files.
It complements [imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md) section 1,
which already found that LPUART3 is the EVK M7 console and that Linux on
Verdin does not use LPUART3. Two things here are new: the pad conflict on
Verdin (section 2.3) and the System Manager's use of LPUART2 (section 2.4).

## 1. (a) The console of upstream Zephyr's `imx95_evk` M7 target

| Finding | Tag |
|---|---|
| `imx95_evk_mimx9596_m7.dts` chooses `zephyr,console = &lpuart3` and `zephyr,shell-uart = &lpuart3`. It has no `zephyr,ram-console`. | read from source (S1) |
| `&lpuart3` is `status = "okay"`, `current-speed = <115200>`, `pinctrl-0 = <&lpuart3_default>`. The board also enables `&lpuart5` (115200, `lpuart5_default`). | read from source (S1) |
| Pinmux `lpuart3_default` (in `imx95_evk-pinctrl.dtsi`) is `&iomuxc_gpio_io15_lpuart_rx_lpuart3_rx` and `&iomuxc_gpio_io14_lpuart_tx_lpuart3_tx`, with `bias-pull-up`, `slew-rate = "slightly_fast"` and `drive-strength = "x4"`. So TX is pad GPIO_IO14 and RX is pad GPIO_IO15. | read from source (S2) |
| In the SoC dtsi, `lpuart3` (`serial@...`, `compatible = "nxp,imx-lpuart", "nxp,lpuart"`) is `disabled` by default. Its clock is `<&scmi_clk IMX95_CLK_LPUART3>`, so the clock goes through the SM's SCMI. The dtsi defines lpuart1 to lpuart8 for the M7. All are disabled by default. | read from source (S3) |
| The SM logical machine that must own it is LM1 `name="M7"` in NXP's `mx95evk.cfg`. That LM lists `LPUART3 OWNER, test` and `PIN_GPIO_IO14 OWNER` / `PIN_GPIO_IO15 OWNER`. The AP-NS pin list goes from `PIN_GPIO_IO13` to `PIN_GPIO_IO16`, so IO14/IO15 belong to the M7 alone. | read from source (S4) |
| On i.MX95 the M7's pinctrl and clock requests go to the SM over SCMI, so the SM refuses them unless the M7's LM owns the LPUART and both pads. | inferred (S3 uses SCMI clock handles; S4 grants the pads per LM) |

## 2. (b) UARTs on Verdin iMX95 with the Mallow carrier

### 2.1 What Linux claims (mainline DT written by Toradex)

| Finding | Tag |
|---|---|
| `imx95-verdin.dtsi` aliases are `serial0` = lpuart7, `serial1` = lpuart8, `serial2` = lpuart1, `serial3` = lpuart2, `serial4` = lpuart6. `stdout-path = "serial2:115200n8"`. **The Linux console is lpuart1 = Verdin UART_3.** | read from source (S5) |
| Verdin names and pads: UART_1 = lpuart7 (GPIO_IO08..11, with RTS/CTS); UART_2 = lpuart8 (TX/RX on GPIO_IO12/13, CTS on GPIO_IO14, RTS on GPIO_IO15, in separate `pinctrl_uart8_cts` / `pinctrl_uart8_rts` groups); UART_3 = lpuart1 (pads UART1_RXD/TXD); UART_4 = lpuart2 (pads UART2_RXD/TXD); lpuart6 = on-module Bluetooth on Wi-Fi SKUs. | read from source (S5) |
| SODIMM pins: UART_3 RX/TX are 147/149, UART_4 RX/TX are 151/153, and lpuart8 CTS/RTS (pads GPIO_IO14/IO15) are 143/141. | read from source (S5, pin comments) |
| `imx95-verdin.dtsi` sets no `status` on these nodes. `imx95-verdin-mallow.dtsi` sets `status = "okay"` on **lpuart1, lpuart2, lpuart7 and lpuart8**. So on Mallow, Linux claims all four Verdin UARTs, UART_4 included. | read from source (S6) |
| `lpuart3` does not appear in `imx95-verdin.dtsi` or `imx95-verdin-mallow.dtsi`. | read from source (S5, S6) |
| The Toradex BSP tree (`linux-toradex`, the 6.6.157-7.8.0 kernel) has the same UART layout as mainline. | inferred. The tree could not be read (section 5) |

### 2.2 What the Verdin standard and Mallow give a debugger

| Finding | Tag |
|---|---|
| Toradex's Verdin UART table: UART_1 and UART_2 are general purpose; UART_3 is "debug (console) for the main OS (Cortex-A)"; UART_4 is "debug for the real-time OS (Cortex-M) or general-purpose". | read from source (S7) |
| Toradex's serial-console page, Mallow tab: "Mallow Carrier Board features two debug UARTs (UART3 and UART4). However, the Debugging UART Header (X11) is not assembled by default." | read from source (S8) |
| Mallow product page: debug UART signal levels are 1.8 V. It recommends an external 1.8 V USB-TTL cable and warns that a USB-UART converter must be compatible with that level. | read from source (S9) |
| Neither Toradex page names an on-board USB-to-UART chip for Mallow. The FT4232 bridge that carries UART_3/UART_4 to USB is described for Dahlia and the Verdin Development Board, not Mallow. | read from source (S7, S8). That Mallow has no on-board bridge is inferred |
| Toradex's alternative-core page says the U-Boot console is on the fourth port and the alternative-core console on the third port of the carrier's USB debug interface. That wording fits the four-port FT4232 boards: the highest index is the Linux console (UART_3), so the one below it is UART_4. Mallow is not named. | read from source (S10). The UART_4 mapping is inferred |
| On Mallow, the only place to read the M7 log over a wire is the X11 header, with UART_3 or UART_4 and an external 1.8 V cable. X11 is not fitted by default. Where UART_1/UART_2 go on Mallow was not found. | inferred (S8, S9). Mallow datasheet and schematic not read (section 5) |

### 2.3 The EVK console pads collide with Verdin UART_2

| Finding | Tag |
|---|---|
| The EVK M7 console's pads (GPIO_IO14 TX, GPIO_IO15 RX, section 1) are the pads Verdin uses for UART_2 CTS/RTS (SODIMM 143/141), and Mallow's Linux DT claims them through lpuart8. | read from source (S2, S5, S6) |
| So the unchanged EVK image would put the M7 console on Verdin UART_2's flow-control pins. Those pins reach a Mallow header only if Mallow routes UART_2 RTS/CTS, which I could not check. Linux would also mux them for lpuart8, unless Toradex's SM gives the pads to the M7. In that case Linux's pinctrl request for them would fail. | inferred |
| Mainline commit `13d4890ca951` "arm64: dts: freescale: imx95-verdin: Split UART_2 pinctrl group" moved CTS/RTS into their own groups. That makes it possible to drop them from Linux, but no source I read says this was done for the M7. | read from source (S5 commit). The purpose is not known |

### 2.4 LPUART2 (UART_4) is the SM debug UART in NXP's EVK configuration

| Finding | Tag |
|---|---|
| `mx95evk.cfg` sets `BOARD DEBUG_UART_INSTANCE=2` and `DEBUG_UART_BAUDRATE=115200`. It lists `LPUART2 OWNER`, `PIN_UART2_RXD OWNER` and `PIN_UART2_TXD OWNER` under LM0 `name="SM"`. | read from source (S4) |
| In imx-sm `boards/mcimx95evk/board.c`, `BOARD_UART` is `BOARD_DEBUG_UART_INSTANCE` when built with `INC_LIBC`, and the clock table is indexed by instance number (2 = LPUART2). So the EVK SM's own console and monitor run on LPUART2. | read from source (S11) |
| Linux on Mallow enables lpuart2 (section 2.1), so Toradex's SM configuration must give LPUART2 to the A55 side, not keep it for the SM as the EVK does. Otherwise Linux could not get its SCMI clock. What the SM then uses for its own console is unknown. | inferred |
| Toradex's SM configuration (imx-sm config inside its imx-boot): **not found**. NXP imx-sm has no Verdin or Toradex board or config at the pinned tag. github.com/toradex/imx-sm returned 404, and git.toradex.com (`meta-toradex-nxp`, `linux-toradex`) turned the fetcher away with an anti-bot page. | not found |

### 2.5 Answer to (b)

| Question | Answer | Tag |
|---|---|---|
| Linux console | Verdin UART_3 = LPUART1, reachable on Mallow's X11 debug header (not fitted by default) | read from source (S5, S6, S8) |
| UARTs Linux holds on Mallow | LPUART1, LPUART2, LPUART7, LPUART8 (Verdin UART_3, _4, _1, _2), plus LPUART6 for Bluetooth on Wi-Fi SKUs | read from source (S5, S6) |
| UARTs free for the M7 without changing Linux | LPUART3, LPUART4, LPUART5 are unclaimed by the Verdin/Mallow DT. Of their usual pads, only GPIO_IO14/15 (LPUART3, EVK pinmux) are known to reach a SODIMM pin, and Linux muxes those for UART_2 RTS/CTS | read from source for the DT side. Pad reach is inferred |
| Toradex's intended M7 debug UART | Verdin UART_4 = LPUART2, on X11 with UART_3. On Mallow, Linux enables it, so using it from the M7 means taking it away from Linux (DT overlay) and an SM that grants it to LM1 | read from source (S7, S8) for the intent. The conflict is inferred |
| Which LPUART the shipped SM gives the M7 | not found | not found |

## 3. (c) Fallbacks when no UART is reachable

### 3.1 remoteproc trace buffer (one way, M7 to Linux, no rpmsg needed)

| Finding | Tag |
|---|---|
| Zephyr's resource table is in `subsys/ipc/open-amp/` (`resource_table.c`, `resource_table.h`). The table is placed in the linker section `.resource_table`. `rsc_table_get()` returns it, or with `CONFIG_OPENAMP_COPY_RSC_TABLE` copies it to the `zephyr,ipc_rsc_table` chosen node. | read from source (S12) |
| The trace entry is compiled in only `#if defined(CONFIG_RAM_CONSOLE)`: `{ RSC_TRACE, (uint32_t)ram_console_buf, CONFIG_RAM_CONSOLE_BUFFER_SIZE, 0, "Zephyr_log" }`. | read from source (S12) |
| `CONFIG_OPENAMP_RSC_TABLE` (`subsys/ipc/open-amp/Kconfig`, `imply OPENAMP`) enables the table. The vdev entry is present only when `CONFIG_OPENAMP_RSC_TABLE_NUM_RPMSG_BUFF > 0` (default 0). So a trace-only table needs no vrings and no mailbox. | read from source (S12) |
| `CONFIG_RAM_CONSOLE` (`drivers/console/Kconfig`, inside `if CONSOLE`) hooks printk and stdout into `ram_console_buf[CONFIG_RAM_CONSOLE_BUFFER_SIZE]` (default 1024). It is a ring that wraps, with one byte kept as NUL. With a `zephyr,ram-console` chosen node the buffer goes in that region (`RAM_CONSOLE_BUFFER_SECTION`). Without one it is an ordinary array in RAM. | read from source (S13) |
| Linux: `rproc_handle_trace()` registers each `RSC_TRACE` entry as debugfs file `trace<N>` (the first is `trace0`) under the rproc's debugfs directory. A read translates `da` with `rproc_da_to_va()` and returns up to the first NUL. `struct fw_rsc_trace` is `{ u32 da; u32 len; u32 reserved; u8 name[32]; }`, and `reserved` must be 0. | read from source (S14) |
| Minimum Zephyr Kconfig for this path: `CONFIG_CONSOLE=y`, `CONFIG_RAM_CONSOLE=y`, `CONFIG_RAM_CONSOLE_BUFFER_SIZE=<n>`, `CONFIG_OPENAMP_RSC_TABLE=y` (pulls in `CONFIG_OPENAMP`), and `CONFIG_UART_CONSOLE=n` if no UART is owned. | inferred from S12, S13 |
| `ram_console_buf` is an M7 address. If it sits in DTCM, Linux reaches it only through the i.MX95 rproc address-translation table, which maps M7 TCM addresses to the system aliases. `CONFIG_OPENAMP_WITH_DCACHE` or an uncached placement may be needed so Linux sees what the M7 wrote. | inferred. The ATT is recorded in [imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md) section 2.1; cache rules are in [imx95-m7-fb-memory.md](imx95-m7-fb-memory.md) |
| Whether NXP's 6.6 `imx_rproc` reads the table from the ELF's `.resource_table` section or from the `rsc_table` carveout, and so whether a trace entry in the ELF is registered at all, is the subject of item rproc-load-src. | not decided here |
| Reading the trace needs debugfs mounted and root. It does not touch the display, Weston, any UART or any persistent storage. | inferred (S14: the file mode is 0400) |

### 3.2 rpmsg tty (two way, needs vrings and the MU7 mailbox)

| Finding | Tag |
|---|---|
| Zephyr sample `samples/subsys/ipc/openamp_rsc_table` creates endpoints `"rpmsg-client-sample"` and `"rpmsg-tty"`. Its `prj.conf` sets `CONFIG_OPENAMP=y`, `CONFIG_OPENAMP_RSC_TABLE=y`, `CONFIG_OPENAMP_RSC_TABLE_NUM_RPMSG_BUFF=8`, `CONFIG_IPM=y`, `CONFIG_SHELL=y` and `CONFIG_SHELL_BACKEND_RPMSG=y`. | read from source (S15) |
| The sample has `boards/imx95_evk_mimx9596_m7.{conf,overlay}`. The conf adds `CONFIG_ARM_SCMI=y`, `CONFIG_OPENAMP_WITH_DCACHE=y`, `CONFIG_OPENAMP_COPY_RSC_TABLE=y`, `CONFIG_UART_CONSOLE=y` and `CONFIG_MBOX_INIT_PRIORITY=0`. The overlay sets chosen `zephyr,ipc_shm`, `zephyr,ipc_rsc_table` and `zephyr,ipc` (a `zephyr,mbox-ipm` on `&mu7`), and enables `&mu7`. The shared-memory and resource-table regions it uses are the ones the Verdin Linux DT reserves (`vdev*` and `rsc_table`). `tests.yaml` does not list any i.MX9 platform, so CI does not build it for the i.MX95. | read from source (S15). The match with the Verdin DT is read from source (S5) |
| Zephyr also has `CONFIG_UART_RPMSG` (`drivers/serial/uart_rpmsg.c`, compatible `rpmsg-uart`, `depends on RPMSG_SERVICE`). It registers the hard-coded endpoint name `"rpmsg-tty"` and implements only poll in/out. No log backend named for rpmsg exists in `subsys/logging/backends`. | read from source (S16) |
| Linux mainline `rpmsg_tty` (`CONFIG_RPMSG_TTY`) binds the channel name `"rpmsg-tty"` and creates a `ttyRPMSG` tty. | read from source (S14) |
| NXP's lf-6.6.y kernel has a separate `imx_rpmsg_tty` (`CONFIG_IMX_RPMSG_TTY`, modules only, `default m`). It matches `"rpmsg-virtual-tty-channel-1"`, `"rpmsg-virtual-tty-channel"` and `"rpmsg-openamp-demo-channel"`, and does **not** match `"rpmsg-tty"`. | read from source (S17) |
| On the 6.6.157-7.8.0 BSP, Zephyr's `"rpmsg-tty"` endpoint binds only if that kernel has `CONFIG_RPMSG_TTY` built. Otherwise the Zephyr endpoint needs to be renamed to an `imx_rpmsg_tty` name. Whether the BSP enables `RPMSG_TTY` was not checked. | inferred. The kernel config was not read |
| rpmsg is a second step after the trace buffer. It needs the vdev entry, the vrings and the buffer pool in the Linux carveouts, `mu7` on the M7 side and on the Linux side (`cm7` mailboxes), and a working kick in both directions. Any of these can fail where a trace-only table would not. | inferred |

## 4. Summary of risk per log channel

| Channel | Needs from SM / Linux | Disturbs Linux? | Tag |
|---|---|---|---|
| LPUART3 on GPIO_IO14/15 (EVK image as is) | SM grants LPUART3 and both pads to LM1. Reaching the pins on Mallow is not known | Collides with Linux's lpuart8 RTS/CTS pinmux | inferred |
| LPUART2 = Verdin UART_4 on X11 | SM grants LPUART2 and the UART2 pads to LM1. Linux DT overlay disabling lpuart2. X11 fitted plus a 1.8 V cable | Needs a DT change, which is a boot-config write the board rules forbid | inferred |
| remoteproc trace (`trace0`) | Only an rproc load path that registers the ELF's resource table | No: read-only debugfs | inferred |
| rpmsg tty | Same as trace, plus vdev, vrings, MU7 and a matching Linux tty driver | Loads or binds a tty driver. No boot-config write | inferred |

## 5. What was not found

- Toradex's SM configuration for Verdin iMX95 (imx-boot / imx-sm fork). Without it, which LPUART and pads LM1 owns on this board is unknown.
- The Verdin iMX95 datasheet and the Mallow datasheet and schematic (documents on docs.toradex.com, not fetched in this study). So these are unknown: whether GPIO_IO14/15 reach a Mallow header, where UART_1/UART_2 go on Mallow, and whether Mallow has any on-board USB-UART bridge.
- The `linux-toradex` 6.6 BSP DT and kernel config (git.toradex.com anti-bot page). So the BSP's `RPMSG_TTY` / `IMX_RPMSG_TTY` settings are unknown, and the BSP UART layout is assumed equal to mainline.
- Mainline `imx95.dtsi` default UART status and `imx95-verdin-nonwifi-mallow.dts` (rate-limited fetch). The Mallow dtsi sets `okay` explicitly, so the answer does not depend on them.

## 6. Recommendation for item m7-run

1. **Use the remoteproc trace buffer as the primary log channel.** Build the M7 hello with `CONFIG_RAM_CONSOLE=y`, a small `CONFIG_RAM_CONSOLE_BUFFER_SIZE` (for example 1024 to 4096), `CONFIG_OPENAMP_RSC_TABLE=y`, `CONFIG_OPENAMP_RSC_TABLE_NUM_RPMSG_BUFF=0` and `CONFIG_UART_CONSOLE=n`. After `start`, read `trace0` in the rproc's debugfs directory, and record whether it exists at all. It needs no UART ownership, no DT change and no wiring, and touches nothing the board rules protect. *inferred*
2. **Gate on rproc-load-src.** If that study finds that NXP's 6.6 `imx_rproc` on i.MX95 ignores the ELF's resource table, or registers it only from the `rsc_table` carveout, also set `CONFIG_OPENAMP_COPY_RSC_TABLE=y` with a `zephyr,ipc_rsc_table` chosen node on the Verdin `rsc_table` carveout, as the upstream sample's i.MX95 overlay does. *inferred*
3. **Do not count on a UART.** Keep `lpuart3` in the build only if the SM is shown to grant it. Even then, its EVK pads are Verdin UART_2 RTS/CTS, which Linux muxes on Mallow. Using UART_4 needs a Linux DT change, which is not allowed on this board. Record "no M7 UART on Mallow without SM and DT changes" as the expected outcome, not as a failure. *inferred*
4. **Treat no trace as evidence, not proof.** If `trace0` is missing or empty, the cause may be resource-table handling, cache visibility or the M7 not running. Record the remoteproc `state` and the kernel log lines before guessing. rpmsg tty is the next step only after the trace works, and only after checking whether the BSP kernel has `RPMSG_TTY` (endpoint `"rpmsg-tty"`) or only `IMX_RPMSG_TTY` (needs an NXP channel name). *inferred*
5. **Optional, read-only:** while holding the serial capability, note what the X11/USB debug interface of this setup shows during boot. An SM banner there would answer section 2.4's open question about the SM debug UART. *inferred*

## Sources

All read 2026-11-28.

- S1. Zephyr tag `v4.5.0`, commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966`:
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts
- S2. Same tag:
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/boards/nxp/imx95_evk/imx95_evk-pinctrl.dtsi
- S3. Same tag:
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/dts/arm/nxp/imx/nxp_imx95_m7.dtsi
- S4. NXP imx-sm tag `rel_imx_sm_2026q3`, commit `eec232878ab7dd460f5a8f3767d39dc7e9412efa`:
  https://github.com/nxp-imx/imx-sm/blob/rel_imx_sm_2026q3/configs/mx95evk.cfg
- S5. Linux mainline, master at commit `822ba3a89c7896b26e4beb70e2ebffb881924dfe` (2026-11-27). The last commit touching the file is `13d4890ca951d8aa6b43e1f03d5f44fe5451d6ec`:
  https://github.com/torvalds/linux/blob/822ba3a89c7896b26e4beb70e2ebffb881924dfe/arch/arm64/boot/dts/freescale/imx95-verdin.dtsi
- S6. Same commit:
  https://github.com/torvalds/linux/blob/822ba3a89c7896b26e4beb70e2ebffb881924dfe/arch/arm64/boot/dts/freescale/imx95-verdin-mallow.dtsi
  and `imx95-verdin-wifi-mallow.dts` in the same directory (includes only)
- S7. Toradex, "UART (Linux)", BSP 7.x.y, last updated 2025-09-02:
  https://developer.toradex.com/linux-bsp/application-development/peripheral-access/uart-linux/
- S8. Toradex, "Configuring Serial Port Debug Console (Linux/U-Boot)", last updated 2026-04-27 (first part read verbatim):
  https://developer.toradex.com/software/development-resources/configuring-serial-port-debug-console-linuxu-boot/
- S9. Toradex, Mallow Carrier Board product page, last updated 2026-11-26:
  https://developer.toradex.com/hardware/verdin-som-family/carrier-boards/mallow-carrier-board/
- S10. Toradex, "How to load and run firmware on alternative cores (NXP)", last updated 2026-11-11:
  https://developer.toradex.com/software/hmp/hmp-nxp/how-to-load-firmwares-nxp/
- S11. NXP imx-sm, same tag as S4:
  https://github.com/nxp-imx/imx-sm/blob/rel_imx_sm_2026q3/boards/mcimx95evk/board.c
- S12. Zephyr, same tag as S1:
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/subsys/ipc/open-amp/resource_table.c ,
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/subsys/ipc/open-amp/resource_table.h ,
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/subsys/ipc/open-amp/Kconfig
- S13. Zephyr, same tag as S1:
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/drivers/console/Kconfig ,
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/drivers/console/ram_console.c
- S14. Linux mainline, same commit as S5:
  https://github.com/torvalds/linux/blob/822ba3a89c7896b26e4beb70e2ebffb881924dfe/drivers/remoteproc/remoteproc_core.c ,
  https://github.com/torvalds/linux/blob/822ba3a89c7896b26e4beb70e2ebffb881924dfe/drivers/remoteproc/remoteproc_debugfs.c ,
  https://github.com/torvalds/linux/blob/822ba3a89c7896b26e4beb70e2ebffb881924dfe/include/linux/rsc_table.h ,
  https://github.com/torvalds/linux/blob/822ba3a89c7896b26e4beb70e2ebffb881924dfe/drivers/tty/rpmsg_tty.c
- S15. Zephyr, same tag as S1:
  https://github.com/zephyrproject-rtos/zephyr/tree/v4.5.0/samples/subsys/ipc/openamp_rsc_table
  (`prj.conf`, `tests.yaml`, `src/main_remote.c`, `boards/imx95_evk_mimx9596_m7.conf`, `boards/imx95_evk_mimx9596_m7.overlay`)
- S16. Zephyr, same tag as S1:
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/drivers/serial/Kconfig.rpmsg ,
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/drivers/serial/uart_rpmsg.c ,
  https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/dts/bindings/serial/rpmsg-uart.yaml ,
  https://github.com/zephyrproject-rtos/zephyr/tree/v4.5.0/subsys/logging/backends
- S17. NXP linux-imx branch `lf-6.6.y`, tip commit `5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc`:
  https://github.com/nxp-imx/linux-imx/blob/5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc/drivers/rpmsg/imx_rpmsg_tty.c ,
  https://github.com/nxp-imx/linux-imx/blob/5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc/drivers/rpmsg/Kconfig

The Toradex developer pages have no commit. They are dated by their "last
updated" line. The files in S1 to S6 and S11 to S17 were read through a
page-fetching helper at the branch or tag named. The S5, S6, S14 and S17 URLs
pin the commit that the GitHub API reported as tip on 2026-11-28. Those
commits are not the 6.6.157-7.8.0 BSP tree.
