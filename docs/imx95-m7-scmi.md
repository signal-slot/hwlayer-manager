# i.MX95 M7 under upstream Zephyr: SCMI reach for display clocks and power domains

Source study, 2026-11-28. No board was accessed. Every row is tagged
**read from source** or **inferred**; nothing here is **observed**.

## Question and short answer

Can upstream Zephyr on the i.MX95 M7 ask the System Manager (SM) to enable the
display power domain and set the DPU and LVDS pixel clocks?

- **Power domain and clocks, DSI path: yes, but only by SoC code, not by the
  display drivers.** With `CONFIG_MIPI_DSI_NXP_DWC` set, `soc_init()` powers
  DISPLAYMIX and CAMERAMIX over SCMI and programs VIDEOPLL1 and DISP1PIX plus
  two MIPI clocks over SCMI (read from source, `soc.c:223-361`). That path is
  written for the EVK's MIPI-DSI panel.
- **The DPU driver itself never calls SCMI.** `display_socionext_dpu.c` has no
  clock, power-domain or SCMI reference, and the `dpu` DT node has no `clocks`
  or `power-domains` (read from source). It assumes the SoC init (or the SM
  defaults) already left the display domain and clocks on.
- **LVDS: no.** The M7 dtsi has no LVDS/LDB node, no LDB clock ID in the
  binding (only `IMX95_CLK_LDBPLL`/`_VCO` PLL IDs) and no code that sets an LDB
  pixel clock (read from source). hal_nxp ships `ldb` driver sources, but the
  Zephyr tree here does not use them (read from source, see "LVDS").
- Whether any of these calls *succeed* depends on what the SM grants the M7's
  logical machine (LM). That is not in any file read here (inferred, below).

The board in the task is named `mimx95_evk/mimx9596/m7`; in v4.5.0 it is
`imx95_evk/mimx9596/m7` (`boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts`).
There is no Verdin iMX95 board in upstream Zephyr.

## Pinned sources (2026-11-28)

| Source | Pin |
|---|---|
| Zephyr `https://github.com/zephyrproject-rtos/zephyr` | tag `v4.5.0` (`VERSION` 4.5.0), tag object `d6e5dc6ae358a6bbe05d040e0428e4161f09d006`, commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966` |
| hal_nxp `https://github.com/zephyrproject-rtos/hal_nxp` | `a8b437ef366ff9a6f9bee3377dc189c4d2623f5b`, the revision `west.yml:215-216` of that tag pins |

Whether v4.5.0 is the newest release could not be checked: `git ls-remote`
failed on this node (credential-helper error). The checkout used is the
existing workspace under the project dependency cache, not a fresh clone.
Paths below are relative to the Zephyr tree unless prefixed `hal_nxp:`.

## SCMI protocols implemented (all read from source)

| Protocol | ID | Client code | Version constant | Used by M7 DT |
|---|---|---|---|---|
| Base | 0x10 | `drivers/firmware/scmi/base.c` | `base.h:26` 0x20001 | implicit, channel 0 |
| Power domain | 0x11 | `power.c` (`scmi_power_state_get/set`, `power.c:28,67`) | `power.h:29` 0x30001 | `scmi_devpd` `nxp_imx95_m7.dtsi:83-88` |
| System power | 0x12 | `system.c`, `reboot.c` | `system.h:32` 0x20001 | `dtsi:89-93` |
| Clock | 0x14 | `clk.c` (rate get/set, parent get/set, config get/set, attributes, `get_permissions`, `clk.c:82-397`) | `clk.h:44` 0x30000 | `scmi_clk` `dtsi:94-99` |
| Pin control | 0x19 | `pinctrl.c`, `drivers/pinctrl/pinctrl_imx_scmi.c` | `pinctrl.h:60` 0x10000 | `scmi_iomuxc` `dtsi:100-107` |
| NXP CPU domain (vendor) | 0x82 | `nxp/cpu.c` (sleep mode, LPM, IRQ mask, reset vector, info) | `nxp/cpu.h:50` 0x10000 | `dtsi:109-113`, needs `CONFIG_ARM_SCMI_NXP_VENDOR_EXTENSIONS` |

Also in the tree but not named in the M7 dtsi: reset domain (`reset.c`). Perf,
sensor, voltage and power-cap IDs exist only as constants
(`include/zephyr/drivers/firmware/scmi/util.h:439-448`). No NXP vendor protocol
for display or LDB exists in `drivers/firmware/scmi/nxp/` (only `cpu.c`,
`shmem.c`).

Error mapping: SM status `DENIED` becomes `-EACCES`, `NOT_FOUND` `-ENOENT`,
`NOT_SUPPORTED` `-EOPNOTSUPP` (`core.c:16-27`).

## Transport: MU doorbell plus shared memory (read from source)

| Item | Value | Citation |
|---|---|---|
| SCMI node | compatible `arm,scmi`, `shmem = <&scmi_shmem0>`, `mboxes = <&mu5 0>`, `mbox-names = "tx"` | `dts/arm/nxp/imx/nxp_imx95_m7.dtsi:74-79` |
| Shared memory | `arm,scmi-shmem` at `0x44611000`, size `0x80` | `dtsi:67-70` |
| MU | `mu5` = `nxp,mbox-imx-mu` at `0x44610000`, IRQ 205 | `dtsi:869-874` |
| Second MU | `mu7` at `0x42440000`, IRQ 207, `status = "disabled"` (not used for SCMI) | `dtsi:876-882` |
| Mailbox transport | message written to shmem, then `mbox_send_dt()` rings the MU general-interrupt doorbell; reply arrives by MU callback | `drivers/firmware/scmi/mailbox.c:34-57,13-32,133-135` |
| MU doorbell | channel to GenInt trigger mask | `drivers/mbox/mbox_nxp_imx_mu.c:34-39,60-70` |
| NXP shmem vendor hook | optional CRC32 over the message (`res1[0]==3`) | `drivers/firmware/scmi/nxp/shmem.c:17-50` |
| Init order | `CONFIG_MBOX_INIT_PRIORITY=0`; SCMI protocols at `PRE_KERNEL_1`; `soc_init` at `PRE_KERNEL_2` | `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7_defconfig:11-16`, `drivers/firmware/scmi/mailbox.c:133`, `soc.c:500` |
| Reply wait | semaphore timeout default 500 us (`ARM_SCMI_CHAN_SEM_TIMEOUT_USEC`), polling optional | `drivers/firmware/scmi/Kconfig` |

One channel (MU5 ch 0, shmem `0x44611000`) serves all protocols; there are no
per-protocol channels in this DT. The addresses are upstream's EVK defaults
for the M7 agent. That the SM of the Toradex image assigns the same MU
instance and shared-memory window to the M7 LM is **inferred** and unchecked:
on a Verdin image, SCMI would not reach the SM at all if the SM configuration
differs (inferred).

## IDs the display nodes name

| Node (M7 dtsi) | `clocks` | `power-domains` | Tag |
|---|---|---|---|
| `dpu` (`socionext,dpu`, `0x4b400000`, IRQ 215, disabled) `dtsi:1054-1061` | none | none | read from source |
| `mipi_dsi` (`nxp,mipi-dsi-dwc`, `0x4acf0000`, disabled) `dtsi:1063-1073` | `<&scmi_clk 0>` | none | read from source |
| `disp_irqsteer` (`0x4b0b0000`, disabled) `dtsi:983-991` | none | `<&disp_mix>` | read from source |
| `disp_mix` power-domain node `dtsi:116-122` | n/a | SCMI domain `0xd` = 13 = `IMX95_PD_DISPLAY` (`dt-bindings/power/imx95_power.h:23`) | read from source |
| LVDS / LDB | no node | no node | read from source |

Board file: `imx95_evk_mimx9596_m7.dts:302-304` only attaches the labels
`zephyr_dpu: &dpu {}` and `zephyr_mipi_dsi: &mipi_dsi {}`; it does not set
`status = "okay"`, width/height or display timings. Display is therefore off in
the board's default M7 build; an application overlay must enable both nodes
and supply timings (read from source).

Clock IDs the SoC code uses for display (`include/zephyr/dt-bindings/clock/imx95_clock.h`):
`IMX95_CLK_VIDEOPLL1_VCO` 18 (`:27`), `IMX95_CLK_VIDEOPLL1` 19 (`:28`),
`IMX95_CLK_DISP1PIX` 41+39 = 80 (`:92`), `IMX95_CLK_MIPIPHYPLLBYPASS` 41+26 = 67
(`:79`), `IMX95_CLK_MIPITESTBYTE` 41+28 = 69 (`:81`). Also defined and unused by
any M7 display code: `DISPAPB/AXI/DP/OCRAM` (`:87-90`), `DISP2PIX`, `DISP3PIX`
(`:93-94`), `LDBPLL_VCO` 37, `LDBPLL` 38 (`:46-47`). `IMX95_CLK_MIPIPHYCFG`
(`:78`) is what the DSI driver is supposed to be given; the node uses literal
`0` instead (see next section). (All read from source.)

## Does the M7 path call SCMI for display, or assume "on"?

| Step | What the code does | Tag |
|---|---|---|
| Power DISPLAYMIX | `soc_init()` sets `IMX95_PD_DISPLAY` ON via `scmi_power_state_set`, then polls `scmi_power_state_get` until ON, aborting `soc_init` on error. Only `#if defined(CONFIG_MIPI_DSI_NXP_DWC)` (`soc.c:223,236-250`) | read from source |
| Power CAMERAMIX | same for `IMX95_PD_CAMERA` (13 is display, 3 is camera) because the DSI pixel-link CSR sits in CAMERAMIX (`soc.c:252-266,270-271`) | read from source |
| Pixel-link registers | direct MMIO writes to `DISPLAY__BLK_CTRL_DISPLAYMIX->PIXEL_LINK_CTRL`, `CAMERA__DSI_MASTER_CSR`, `DPU_IRQSTEER->CHN_MASK[13]` (`soc.c:268-274`); these need the domains on and MMIO access to the blocks | read from source |
| VIDEOPLL1 | rate set to 4008 MHz VCO then 446.333 MHz, each followed by `scmi_clock_config_set` enable (`soc.c:276-312`) | read from source |
| DISP1PIX | parent set to VIDEOPLL1, rate 148.444 MHz (`soc.c:314-328`); no explicit enable call | read from source |
| MIPI PHY clocks | MIPIPHYPLLBYPASS and MIPITESTBYTE: parent VIDEOPLL1, rate 446.333 MHz (`soc.c:330-360`) | read from source |
| DSI driver init | `dwc_mipi_dsi_init` calls `clock_control_set_rate(scmi_clk, 0, &dphy_ref_frequency)` (`drivers/mipi_dsi/dsi_nxp_dwc.c:224-232`). The SCMI clock driver turns `rate` into `cfg.rate[0] = (uintptr_t)rate` (`clock_control_arm_scmi.c:110`), so the value passed is the pointer, not the frequency, and clock ID 0 is not defined in the binding (first ID is 1). Looks like an upstream defect; I did not test it | read from source, interpretation inferred |
| DPU driver | `dpu_init` configures the IRQ, formats and framebuffers and calls `DPU_Init`/`DPU_PreparePathConfig` (`display_socionext_dpu.c:209-257`); no clock/power/SCMI call anywhere in the file | read from source |
| `dpu` DT node | no `clocks`, no `power-domains`, so even generic PM-domain handling cannot power it | read from source |
| LVDS | nothing: no node, no driver, no clock set; hal_nxp has `mcux/mcux-sdk-ng/drivers/ldb`, `ldb_1`, `ldb_combo_phy` sources but no Zephyr driver or SoC code references them | read from source |
| Clock on/off helper | `scmi_clock_on_off` queries clock attributes; if the clock has restrictions and state control is **not** allowed it returns 0 without doing anything (`clock_control_arm_scmi.c:50-60`). A denied enable is silent success | read from source |
| `disp_mix` as a PM domain | `power_domain_arm_scmi.c:21-63` would issue SCMI power set on resume/suspend, but only `disp_irqsteer` (disabled) names `disp_mix` and `scmi_pd` needs `PM_DEVICE`; nothing display-related that is enabled uses it | read from source |
| Wiring is board-specific | the whole DSI block is guarded by the Kconfig symbol, which depends on a DT node being enabled (`drivers/mipi_dsi/Kconfig.mcux:47-52`); with no overlay enabling `mipi_dsi`, none of these SCMI calls run | read from source |

Conclusion for the question: on the M7, **display power and clocks are
requested through SCMI by `soc_init()` for the DSI configuration**. The DPU
driver and the LVDS route assume the domains and clocks are already on. A
Verdin LVDS panel path would need new SoC-init code (LDB clock/parent/rate,
LDB and DISPLAYMIX blk-ctrl register writes) that upstream does not have. This
is a gap in upstream, not something this study found working.

## What the SM must grant the M7's logical machine (inferred)

Nothing in the Zephyr tree states these; they follow from the SCMI calls above
and from how the SM filters requests per agent. They are untested and the
Toradex SM configuration was not read in this item (see the SM items of the
plan).

1. **A channel.** An SCMI agent for the M7 LM wired to MU5 channel 0 and a
   shared-memory window at `0x44611000` (size 0x80), reachable from the M7 MPU
   map. Otherwise `scmi_protocol` init fails at boot.
2. **Protocol visibility.** Base, power (0x11), clock (0x14) and, for
   `soc_init`'s CPU call, the NXP CPU protocol (0x82). Pin control (0x19) only
   if pinctrl is used. The CPU-domain call (`soc.c:369-377`) comes after the
   display block, so a missing 0x82 would fail `soc_init` only after the
   display clocks were already programmed.
3. **Power-domain permission on DISPLAY (13) and CAMERA (3).** The M7 LM needs
   power-state set/get rights on both, or `soc_init` returns `-EACCES` and
   stops. If Linux (A55 LM) already powers them, a second ON request from the
   M7 is harmless only if the SM allows it; OFF must not be requested from the
   M7 while Linux uses the display.
4. **Clock permissions** on VIDEOPLL1_VCO (18), VIDEOPLL1 (19), DISP1PIX (80),
   MIPIPHYPLLBYPASS (67), MIPITESTBYTE (69), plus parent-set and rate-set
   rights. Shared PLLs mean Linux and the M7 would fight over rates: the SM
   normally lets only an agent with exclusive ownership change them, so a
   split-ownership display (Linux keeps a plane, M7 the other) is not
   expressible by SCMI alone (inferred). For an LVDS panel the equivalent list
   is the LDB pixel clock and `LDBPLL(_VCO)` (38, 37) plus the DISP pixel
   clock; their exact IDs on the Toradex SM were not read.
5. **MMIO access to the display blocks.** SCMI does not grant register access.
   The resource-protection configuration (RDC/XRDC-style domain assignment in
   the SM's board configuration) must give the M7's domain access to the DPU
   (`0x4b400000`), DISPLAYMIX blk-ctrl, DPU IRQSTEER (`0x4b0b0000`), and the
   scanout memory, and route the DPU interrupt (NVIC 215 via the M7 DT) to the
   M7. Without it the first DPU register access faults the M7.
6. **Linux must not own the same resources.** If the A55 LM's DPU/DISPLAYMIX
   and clocks are not released, the SM will deny the M7 (`SCMI_DENIED` =
   `-EACCES`; for clock enable the Zephyr helper then silently returns 0, see
   above).

Items 1 to 6 are the same conclusion the plan's ownership items are meant to
confirm from the SM sources; treat them as hypotheses until checked there.

## Gaps and risks for the Verdin iMX95 (inferred)

- No upstream Verdin board: `imx95_evk` DT values (MU, shmem, IRQs) are the EVK
  defaults and must be reconciled with the Toradex SM configuration.
- The upstream display SCMI path targets MIPI-DSI; the Verdin/Mallow panel is
  LVDS, so VIDEOPLL1/DSI settings do not apply and no LDB support exists.
- `dwc_mipi_dsi_init` passes a pointer as the rate and uses clock ID 0; do not
  rely on it for anything.
- `scmi_power_state_set` is followed by an unbounded poll loop in `soc_init`
  (`soc.c:245-250`): a power domain that stays off hangs boot rather than
  failing.

## Not done

- No board access, no Toradex SM configuration or `imx-sm` source read here.
- Nothing built or run; the SCMI calls were not exercised.
- Linux-side SCMI use of the same clock and power IDs was not compared.
