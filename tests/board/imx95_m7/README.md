# i.MX95 Cortex-M7 hello and read-only DPU probe (Zephyr)

A small Zephyr application for the Cortex-M7 of the i.MX95. It is the first
M2 (WP3) board program: it shows that an upstream-Zephyr image can be built
for the M7 and started by Linux remoteproc, and it can optionally read a few
display controller (DPU) registers without writing any of them.

It uses upstream Zephyr only. It contains no code from any private or
showcase repository.

Status (2026-11-28): **built on the host only. Not run on any board.**
Everything below about run-time behaviour is from source reading, not from
a measurement.

## What the program does

1. It builds a short text report:
   - a banner: `hwlm-imx95-m7: hello from Zephyr <version> on <board target>`;
   - with `CONFIG_HWLM_DPU_PROBE=y` only: the DPU registers listed below.
2. It prints the report on the M7 console UART.
3. With `CONFIG_HWLM_RPMSG=y` (the default for the i.MX95 M7 target), it
   waits until Linux brings up the rpmsg virtio device. Then it announces one
   rpmsg endpoint (default name `rpmsg-tty`) and answers **every** message
   from Linux with the same report.

The endpoint cannot send first. Linux learns the endpoint from the name
service announcement, but the M7 only learns the Linux address when the
first message arrives. So on Linux you write something to the channel and
then read the answer (see "Load procedure").

### DPU probe (off by default)

`CONFIG_HWLM_DPU_PROBE` is `n` in `prj.conf`. When it is enabled:

1. The probe asks the System Manager (SM), over SCMI, whether the DISPLAY
   power domain is on (`POWER_STATE_GET`), and whether the `DISPAPB` clock is
   enabled (`CLOCK_CONFIG_GET`, `CLOCK_RATE_GET`). These calls are queries;
   they change nothing. If a query fails (for example, the M7 agent may not
   see that resource) or the answer is "off", the probe reads no display
   register.
2. Then it reads these registers, each one once (the time stamp twice,
   50 ms apart):

   | Register | Offset | Why |
   |---|---|---|
   | DISPLAYMIX block control `DISPLAY_ENGINES_CLOCK_CONTROL` | `0x4B010000 + 0x0` | display engine clock control (meaning taken from the HAL name only) |
   | `COMCTRL_IPIDENTIFIER` | DPU base + `0x0` | IP identification |
   | `DISENG_FRAMEGEN0_FGENSTS` | DPU base + `0x2B0094` | FrameGen0 enable status |
   | `DISENG_FRAMEGEN0_HTCFG1` | DPU base + `0x2B000C` | horizontal total / active |
   | `DISENG_FRAMEGEN0_VTCFG1` | DPU base + `0x2B0014` | vertical total / active |
   | `DISENG_FRAMEGEN0_FGCHSTAT` | DPU base + `0x2B009C` | channel status |
   | `DISENG_FRAMEGEN0_FGTIMESTAMP` | DPU base + `0x2B0098` | changes while FrameGen0 runs |

   The DPU base (`0x4B400000`) comes from the upstream devicetree node `dpu`.
   A build-time check makes sure it equals the HAL's `DISPLAY_SEERIS_BASE`.
   The offsets come from the HAL structs (`DISPLAY_SEERIS_Type`,
   `DISPLAY_BLK_CTRL_DISPLAYMIX_Type` in hal_nxp), not from hand-written
   numbers. (read from source)
3. Each read runs with data bus faults ignored (`FAULTMASK` set and
   `SCB->CCR.BFHFNMIGN`). If the access is refused with a bus error, the
   probe prints `bus error` and continues instead of crashing. The only
   registers written are the M7's own `FAULTMASK`, `CCR` and `CFSR`.

Limits (inferred, not measured):

- A read can still stall if the display mix is clock-gated in a way the SM
  does not report. Enable the probe only while Linux keeps the display
  running (Weston active).
- Whether the M7 may read the display at all depends on the SM's resource
  assignment for the board. That question belongs to the i.MX95 feasibility
  study, not to this program.

## Board target

There is **no Verdin iMX95 board in upstream Zephyr**. This application uses
the NXP EVK target:

```
imx95_evk/mimx9596/m7
```

What this means (read from source at the pinned commit, not checked on a
Verdin board):

- **Console UART:** `LPUART3` (`zephyr,console = &lpuart3`, 115200 baud),
  with the EVK pin mux applied through SCMI pinctrl. On another carrier the
  same UART instance may be routed to other pins, or the SM may refuse the
  pin mux request. Check the Verdin M7 debug UART before you expect output.
- **Memory map:** the `m7` variant runs from TCM. Code and read-only data are
  in ITCM (`0x00000000`, 256 KiB). Data and stacks are in DTCM
  (`0x20000000`, 256 KiB). Initialised data is stored in ITCM and copied to
  DTCM at boot (`CONFIG_XIP=y` with `zephyr,flash = &itcm`). Nothing is
  placed in DDR, so the image does not need a DDR carve-out of its own.
- **DDR variant:** `imx95_evk/mimx9596/m7/ddr` places RAM in DDR at
  `0x80000000` (42 MiB). It is not used here: that range is normal Linux
  memory unless the board's Linux devicetree reserves it.
- **rpmsg addresses:** the overlay uses the EVK values from upstream Zephyr's
  `samples/subsys/ipc/openamp_rsc_table/boards/imx95_evk_mimx9596_m7.overlay`:
  mailbox `MU7`, shared memory `0x88000000` (5 MiB), resource table copy at
  `0x88220000`. These must match the remoteproc node and the
  `reserved-memory` regions (vrings, vdev buffer, `rsc-table`) in the Linux
  devicetree of the target board. If they differ, change
  `boards/imx95_evk_mimx9596_m7.overlay`, or build with `CONFIG_HWLM_RPMSG=n`
  for UART only.
- **Resource table:** upstream Zephyr supports the Linux remoteproc resource
  table on this target. `soc/nxp/imx/imx9/imx95/m7/linker.ld` keeps a
  `.resource_table` section when `CONFIG_OPENAMP_RSC_TABLE=y`. The settings
  in `boards/imx95_evk_mimx9596_m7.conf` are the ones of the upstream
  sample, including `CONFIG_OPENAMP_COPY_RSC_TABLE=y` (the M7 copies its
  table to the `rsc-table` region at start). (read from source)
- **Other EVK peripherals:** the overlay disables every EVK peripheral this
  program does not use (ENETC, FlexCAN, FlexSPI, LPI2C, LPSPI, LPUART5, SAI,
  TPM, timers, eDMA, GPIO, ADC, watchdog). Reason: upstream `soc.c` sets the
  FlexCAN clock over SCMI for every enabled FlexCAN node, and drivers of
  enabled nodes would claim pins and clocks that Linux may own.
  `CONFIG_POWER_DOMAIN=n`, and the display (`dpu`, `mipi_dsi`) nodes stay
  disabled as upstream ships them. So this program never powers or clocks the
  display. Note that `CONFIG_MIPI_DSI_NXP_DWC` must stay off: with it,
  upstream `soc.c` powers the DISPLAY and CAMERA mixes and reprograms the
  video PLL.

## Pinned versions

| Item | Value |
|---|---|
| Zephyr | `https://github.com/zephyrproject-rtos/zephyr`, commit `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (`main`, 2026-11-27, build string `v4.5.0-5218-ge53873cbd0ef`, `VERSION` 4.5.99) |
| hal_nxp | `1a8937f3f8dfe5c68f59cc2a0c0c0fccdb096650` (from Zephyr's `west.yml`) |
| cmsis / cmsis_6 | `512cc7e895e8491696b61f7ba8066b4a182569b8` / `1c1840af7a7e757d6e2fec3ddb0e5ce0dfcc93c8` |
| open-amp / libmetal | `6833b0d2cd8dcb0fa51908a5c2e3732e2a82048f` / `e425e3bc3670aada8801886f0ad7c87956aecdee` |
| Zephyr SDK | 1.0.1 (`zephyr/SDK_VERSION`), toolchain `arm-zephyr-eabi`, GCC 14.3.0, binutils 2.43.1 |

## Prepare the workspace

The way Zephyr's Getting Started guide documents it, limited to the modules
this program needs:

```sh
west init -m https://github.com/zephyrproject-rtos/zephyr --mr main "$ZEPHYR_WORKSPACE"
cd "$ZEPHYR_WORKSPACE"
git -C zephyr checkout e53873cbd0ef45d63ef9ded1a45d10717e7c71d7
west config manifest.project-filter -- '-.*,+cmsis,+cmsis_6,+hal_nxp,+libmetal,+open-amp'
west update --narrow -o=--depth=1
west sdk install --version 1.0.1 -d "$ZEPHYR_SDK_INSTALL_DIR" -t arm-zephyr-eabi -H
```

The Python packages come from `zephyr/scripts/requirements-base.txt`.
Install them in a virtual environment, not system-wide.

## Build

From inside the west workspace:

```sh
cd "$ZEPHYR_WORKSPACE"
west build -p always -b imx95_evk/mimx9596/m7 -d <build-dir> <repo>/tests/board/imx95_m7 \
    -- -DZEPHYR_SDK_INSTALL_DIR="$ZEPHYR_SDK_INSTALL_DIR"
```

To build the DPU probe in, add `-DCONFIG_HWLM_DPU_PROBE=y` after `--`. To
build without rpmsg (UART only), add `-DCONFIG_HWLM_RPMSG=n`.

The artefact for remoteproc is `<build-dir>/zephyr/zephyr.elf`.

Host build results (2026-11-28, pinned versions above):

| Configuration | Result | ITCM (code, rodata, data image) | DTCM (data, bss, stacks) |
|---|---|---|---|
| default (rpmsg on, DPU probe off) | builds, no warnings | 35,012 B of 256 KiB | 8,868 B of 256 KiB |
| `CONFIG_HWLM_DPU_PROBE=y` | builds, no warnings | 36,072 B of 256 KiB | 8,868 B of 256 KiB |
| `CONFIG_HWLM_RPMSG=n` (UART only) | builds, no warnings | 27,180 B of 256 KiB | 8,364 B of 256 KiB |

In the default ELF, all `PT_LOAD` segments have physical addresses in ITCM
(`0x0`–`0x88c8`) or DTCM (`0x200003d0`, bss only). `.resource_table` is at
`0x0000886c` (88 bytes).

CI does not build this program.

## Load procedure (Linux remoteproc)

Not run yet. This is the intended procedure, and it is load-only: nothing
here writes the rootfs, the U-Boot environment or an eMMC boot partition.
Run it as root on the board. Use the remoteproc instance whose `name` is the
i.MX M7 (`imx-rproc`). Before you start, confirm that the M7 is not already
running firmware that the board needs.

```sh
RP=/sys/class/remoteproc/remoteprocN        # the imx-rproc instance
cat $RP/name $RP/state $RP/firmware           # state must be "offline"
OLD_FW=$(cat $RP/firmware)
OLD_PATH=$(cat /sys/module/firmware_class/parameters/path)

mkdir -p /tmp/hwlm-fw
cp zephyr.elf /tmp/hwlm-fw/hwlm_imx95_m7.elf
echo -n /tmp/hwlm-fw > /sys/module/firmware_class/parameters/path
echo -n hwlm_imx95_m7.elf > $RP/firmware
echo start > $RP/state

# Output: the M7 console UART, and (rpmsg) the kernel log and the tty:
dmesg | tail
ls /dev/ttyRPMSG*                             # needs the Linux rpmsg_tty driver
cat /dev/ttyRPMSG0 & echo hello > /dev/ttyRPMSG0

# Restore everything:
echo stop > $RP/state
echo -n "$OLD_FW" > $RP/firmware
echo -n "$OLD_PATH" > /sys/module/firmware_class/parameters/path
rm -r /tmp/hwlm-fw
```

Notes:

- `firmware` can only be changed while the state is `offline`.
- If Linux has no `rpmsg_tty` driver, build with
  `-DCONFIG_HWLM_RPMSG_EPT_NAME=\"rpmsg-client-sample\"`. The Linux rpmsg
  client sample sends first and prints every reply in the kernel log.
- The vendor kernel's remoteproc may expect other mailbox or memory
  addresses than the EVK values above. Compare with the board's
  `/proc/device-tree` before loading.
