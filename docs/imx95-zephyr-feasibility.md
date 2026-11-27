# i.MX95 Zephyr (SoW M2): ownership and feasibility on the Verdin iMX95

Scope: SoW §5.1 step 1 ("Zephyr second"), WP3 and milestone M2. Board:
Verdin iMX95 on a Mallow carrier, 10.1" LVDS panel, Toradex Reference
Multimedia Image 7.8.0, kernel 6.6.157-7.8.0 (not Torizon).

Tags: **observed** (on the board, dated), **read from source** (pinned below,
read 2026-11-28), **inferred**, **not found**. No Zephyr image was built or
run for this document, so nothing here is a measurement of Zephyr. No parity
is claimed from any other SoC.

## Sources

| Id | Source | Pin |
| --- | --- | --- |
| S1 | `nxp-imx/imx-sm`, `configs/mx95evk.cfg` (stock EVK SM configuration) | cloned 2026-11-28 at `eec232878ab7dd460f5a8f3767d39dc7e9412efa` (tag `rel_imx_sm_2026q3`). Lines: LM1 `M7` 343, M7 start/stop 354-367, `LMM_2 ALL` 392, `IRQSTEER_M7` 402, LM2 `AP` 443, `LMM_1 NOTIFY` 597, `DC` 625, `LVDS` 798, `MIPI_DSI` 801 |
| S2 | `zephyrproject-rtos/zephyr`: `dts/arm/nxp/imx/nxp_imx95_m7.dtsi`, `drivers/display/display_socionext_dpu.c`, `boards/nxp/imx95_evk/` | read on `main`; tip listed then `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (files may be a commit or two newer) |
| S3 | `docs/imx95-zephyr/observed-2026-11-27.txt` | this repository, observed 2026-11-27 |
| S4 | `docs/imx95-dpu-source-6.6.md` | this repository, Linux DPU driver notes |

URLs: `https://github.com/nxp-imx/imx-sm`, `https://github.com/zephyrproject-rtos/zephyr`.

**Not found:** Toradex's Verdin iMX95 SM configuration and its 6.6 BSP device
tree (the GitHub paths tried returned 404). What Toradex ships is therefore
known from S3 only. All SM statements from source describe the stock EVK
config (S1), not the Verdin. Not examined: U-Boot `bootaux`, the boot
container contents and imx-mkimage on this board.

## Ownership model

Read from source (S1, stock `mx95evk.cfg`):

- Logical machines: LM0 `SM` (M33), LM1 `M7`, LM2 `AP` (A55, default).
- The AP non-secure environment owns `BLK_CTRL_DISPLAYMIX`, the `DC*`
  display-controller resources, `LVDS`, `MIPI_DSI`, `MIPI_PHY`, `VIDEO_PLL1`,
  `CLOCK_DISP1PIX` and `LDB_PLL`. LM0 owns `GPV_DISPLAY` and the
  `CLK_DISPAPB`/`CLK_DISPAXI`/`CLK_DISPOCRAM` clocks.
- LM1 (M7) lists none of the DC, LVDS, DSI or DISPLAYMIX resources. It has the
  M7 core, `PD_M7`, `LPUART3`, `MU5_A`/`MU7_B`/`MU8_B`, `LPIT1`, `LPTMR1/2`,
  `LPTPM1`, `WDOG5`, two GPIO pins and DDR `0x80000000–0x89FFFFFF`.
- LM1 may manage LM2 (`LMM_2 ALL`); the AP has only `LMM_1 NOTIFY`, so the
  stock config does not let Linux start or stop the M7 through SCMI LMM.

Inferred: on the stock config the DPU/LVDS route belongs to the A55. The M7
could own it only with a changed SM configuration, i.e. a different boot
container, which writes boot media and is outside the board rules. A DPU
resource has one owner in S1, and nothing in S1 splits the display complex
between two cores, so "alongside Linux" is not supported by the evidence;
"instead of Linux" on the display means Linux not using the DPU.

Observed 2026-11-27 (S3): Linux holds the pipeline. `imx95-dpu` is bound to
`4b400000.display-controller` (`[drm] Initialized imx95-dpu`), LDB channel 0
and the pixel-link bridge are `okay`, the DSI node is `disabled`. The LDB PLL
runs at 2986.2 MHz with `lvds_ch0`/`lvds_di0` on; `dispapb`, `dispaxi` and
`dispocram` are enabled by the DPU; the `disp` and `display` power domains
are `on`, the DPU `active`. Weston was active before and after. SCMI is
v2.1 `NXP:IMX`, firmware version `0x23e`, SM MISC 1.1, BBM 1.0. The DT lists
SCMI protocols 0x11, 0x12, 0x13, 0x14, 0x15, 0x19, 0x81, 0x84.

## How M7 firmware is loaded on this board

- Observed 2026-11-27: `remoteproc1` is `imx-rproc` (DT node `remoteproc-cm7`,
  `fsl,imx95-cm7`), state `offline`, firmware name `rproc-imx-rproc-fw`;
  `remoteproc0` is the Neutron NPU. The `firmware_class` search path is empty
  and `/lib/firmware` holds no M7 image. `remoteproc-cm7` has seven
  `memory-region` entries, mailboxes `tx rx rxdb`, a power domain and
  `fsl,startup-delay-ms = 50`. Reserved memory has vring/buffer regions at
  `0x88000000..0x88030000` and a resource table at `0x88220000`. The `m7`
  genpd is on; the `m7` clock reads 800 MHz with enable count 0.
- Inferred: the kernel registered `imx-rproc`, so the Toradex stack expects
  Linux to start the M7. Whether `start` succeeds on this SM is **not
  established**; no start was tried.
- Read from source (S2): Zephyr's i.MX95 EVK documentation starts the M7 from
  a `flash.bin` container (SPSDK plus SM) and mentions remoteproc only for the
  A55 build. It does not describe loading the M7 variants from Linux and says
  nothing about display.

## Zephyr side

Read from source (S2):

- `dpu@4b400000` (`socionext,dpu`, `status = "disabled"`) and
  `mipi-dsi@4acf0000` (`nxp,mipi-dsi-dwc`, disabled) exist only in the M7
  dtsi; the EVK M7 board dts adds labels only. There is **no LVDS/LDB node**,
  and this board's panel is LVDS (observed: LDB channel 0, DSI disabled).
  Scanout to this panel needs LDB and DISPLAYMIX CSR programming upstream
  Zephyr does not have (inferred from the missing nodes).
- The driver offers `set_pixel_format`, `write`, `get_capabilities`,
  `get_framebuffer`; RGB565, RGB888, ARGB8888; one fixed pipeline; 32-byte
  pitch alignment.
- Vblank/latch: the ISR clears pending flags and gives a semaphore on the
  ExtDst0 shadow-load interrupt; `write()` blocks on it. The latch is
  `DPU_TriggerPipelineShadowLoad` after the fetch-unit source address is set;
  scanout starts lazily on the first `write()`. There is no vblank callback or
  timestamp API.
- Board variants: `imx95_evk/mimx9596/m7` (ITCM/DTCM, console `lpuart3`) and
  `.../m7/ddr` (42 MB at `0x80000000`).

Inferred: a framebuffer in DTCM may not be reachable by the DPU, so scanout
memory would have to be DDR granted to the M7. The observed Linux reserved
memory includes a `no-map` entry at `0x80000000` and a pool at `0x9c300000`;
which DDR range is free for the M7 is **not established**.

## Summary

| Item | Stock EVK SM (source) | This board (observed 2026-11-27) |
| --- | --- | --- |
| DPU, DISPLAYMIX | AP (A55) | bound to Linux `imx95-dpu` |
| LVDS/LDB, `VIDEO_PLL1` | AP | LDB ch0 on, PLL 2986.2 MHz |
| MIPI DSI | AP | node disabled |
| Display clocks/power | AP and SM | enabled / `display` on |
| Display interrupts | not in LM1 | Linux |
| Scanout memory | M7 DDR `0x80000000–0x89FFFFFF` | Linux reservations in the range, split unknown |
| M7 start | AP has no start right | `imx-rproc` offline, start not tried |

The left column is the stock EVK and may differ on the Verdin.

## Blocked

1. Ownership: the stock SM gives the display complex to Linux (read from source).
2. LVDS: no LDB support in upstream Zephyr (read from source).
3. Vblank: no callback API in the driver (read from source).
4. Scanout memory placement unresolved (inferred).
5. M7 start from Linux: denied by the stock config, untested on this board.
6. Overlay: the stock driver has one framebuffer; a multi-plane DPU driver
   would be new work and comes only after scanout.

## Gate for the M7 image

The plan builds an M7 image only if the sources show the M7 can be started from
Linux remoteproc without writing the rootfs, U-Boot env or eMMC boot
partitions. They do not: the stock config denies it and the Toradex config was
not found. No image was built and no board session was held for this item.

## Proposed M2 items

1. Start-permission probe (board): upstream `imx95_evk/mimx9596/m7` hello in
   `tests/board/imx95_m7/`, loaded from `/tmp` through remoteproc with a
   `firmware_class` path under `/tmp`, UART log, stop, restore
   (`scripts/imx95-m7-rproc.sh`). Its result gates the rest.
2. Ownership evidence: obtain the Verdin SM configuration or query the running
   SM over SCMI LMM, and replace the left column of the table with it.
3. Reduced scope if the display stays with Linux: the M7 runs a timer-tick and
   rpmsg task so `docs/m2-comparison-plan.md` can use an M7 timer; no scanout
   claim.
4. Scanout only if the DPU can go to the M7: LDB support, DDR scanout buffer,
   then vblank/latch reporting from the shadow-load interrupt.
5. Overlay after item 4.
