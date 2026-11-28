# Verdin AM62P device tree: DSS route, display overlays, R5F carve-outs

Written 2026-11-28 for plan #465, item `am62p-dt-gaps`. Source study: no device
was used and nothing here is **observed**. It lists the device-tree facts the M3
board run has to compare with the running board. The tidss and remoteproc
drivers are covered by other notes; this one reads device-tree files only.

Each statement is tagged **source** (file, commit) or **inferred**.

| Short name | Repository, ref, commit | Status |
| --- | --- | --- |
| **TDX 6.6** | `https://git.toradex.com/linux-toradex.git`, branch `toradex_ti-linux-6.6.y`, commit `9764efd3fe8e5f3fa04045919296c102acd7517e` (`Merge tag 'v6.6.163'`, 2026-11-23) | Read from a blobless clone. Paths below are under `arch/arm64/boot/dts/ti/` of this tree. |
| **overlays** | `https://git.toradex.com/device-tree-overlays.git`, branch `toradex_ti-linux-6.6.y`, commit `c93543ff886bd467219464c3cefabe8ed97d9464`, directory `overlays/` | Read from a blobless clone. |

That Torizon OS builds Verdin AM62P from `toradex_ti-linux-6.6.y` is
**inferred**: the branch carries the board files; `toradex_ti-linux-6.18.y`
(`9797b77f3c86abc90e771a13bc1abcce0ee5dbb5`) and `toradex_ti-linux-6.1.y`
(`c5d9d94e5c3a5b9283dbf861886fd761bb4a0e26`) exist and were not read. The
kernel version on the board decides which applies.

Short answer:

* The SoC dtsi has two DSS nodes, both `disabled`. The Verdin `.dtsi` files do
  not enable or wire either; only Toradex overlays do. LVDS overlays use `dss0`
  VP1 through OLDI, DSI overlays use `dss1` VP2 through `dsi0` and a bridge.
* MCU R5F is `okay` in the Verdin dtsi with carve-outs at `0x9b800000`
  (1 MiB DMA pool) and `0x9b900000` (15 MiB).
* TI's `ti,dss-shared-mode*` properties mark DSS resources as remote-core
  owned. Only an SK-board overlay uses them; no Verdin file does.

## (1) The DSS node (source, TDX 6.6, `k3-am62p-main.dtsi`)

```
dss0: dss@30200000 {
	compatible = "ti,am62p51-dss";
	reg = <0x00 0x30200000 0x00 0x1000>, /* common */
	      ... /* vidl1, vid, ovr1, ovr2 */
	      <0x00 0x3020a000 0x00 0x1000>, /* vp1: Used for OLDI */
	      <0x00 0x3020b000 0x00 0x1000>, /* vp2: Used as DPI Out */
	      <0x00 0x30201000 0x00 0x1000>; /* common1 */
	status = "disabled";
```

| Item | `dss0` | `dss1` |
| --- | --- | --- |
| compatible | `ti,am62p51-dss` | `ti,am62p52-dss` |
| base | `0x30200000` | `0x30220000` |
| register blocks | common, vidl1, vid, ovr1, ovr2, vp1, vp2, common1 | same names |
| power domains (k3_pds) | 186 (DSS0), 243, 244 (OLDI0, OLDI1) | 232 (DSS), 244 |
| interrupt (GIC SPI) | 84 | 30 |
| OLDI transmitters | `oldi0_dss0` (companion `oldi1_dss0`), `oldi1_dss0` | `oldi1_dss1` |
| status in SoC dtsi | `disabled` | `disabled` |

Video ports and what they connect to:

* Each instance has one `vidl1` and one `vid` block, two overlay managers
  and two video ports, so two planes per instance (inferred from the register
  list). The reg comments say VP1 is "Used for OLDI" and VP2 "Used as DPI Out"
  (source).
* The port graph (`dss0_ports`, `oldi*_ports`) is empty in the SoC dtsi
  (source); boards and overlays fill it. No Verdin base file does: a
  case-insensitive grep of `k3-am62p-verdin*.dtsi` and `k3-am62p5.dtsi` for
  `dss`, `oldi`, `dsi`, `lvds`, `hdmi`, `panel`, `vout` finds only pin
  comments (`Verdin PWM_3_DSI`, `I2C_3_HDMI`, ...) (source).
* `dsi0: dsi@30500000` (`ti,j721e-dsi`) and D-PHY `dphy_tx0` (`ti,j721e-dphy`,
  `0x301c0000`) exist, both `disabled` (source).
* Connections that exist on this board come from the overlays in (2):
  `dss0` VP1 to `oldi0_dss0`/`oldi1_dss0` to LVDS panel; `dss1` VP2 to `dsi0`
  to a bridge. No overlay read uses the DPI output of VP2 on `dss0` (source).
* TI's own SK board (`k3-am62p5-sk.dts`) routes `dss0` VP2 to an HDMI
  transmitter; that has no Verdin counterpart (source, for reference only).

## (2) Verdin AM62P display overlays (source, overlays)

Files `overlays/verdin-am62p_*_overlay.dts`, each `compatible =
"toradex,verdin-am62p"`.

| Overlay (`verdin-am62p_...`) | Panel or bridge | DSS and VP |
| --- | --- | --- |
| `panel-cap-touch-10inch-lvds` | `panel-lvds` 1280 x 800, `vesa-24`, maXTouch; timing 68.9/71.1/73.4 MHz | `&dss0`, "VP1: Output to OLDI" (`dss0_ports` port@0, two endpoints); `oldi1_dss0` is enabled too, commented as a hack ("pretend we are in clone mode") |
| `mezzanine_panel-cap-touch-10inch-lvds` | same panel data, mezzanine backlight | `&dss0`, VP1 to OLDI0 and OLDI1 |
| `mezzanine_panel-lvds-dual-channel-1080p` | LG LP156WF1, `jeida-24`, odd/even pixel links, 138.5 MHz, 1920 x 1080 | `&dss0`, VP1 to OLDI0 (odd) and OLDI1 (even) |
| `dsi-to-hdmi` | Lontium `lt8912b` (I2C 0x48), `hdmi-connector` | `&dss1`, port@1 (VP2) to `dsi0` |
| `dsi-to-lvds_panel-cap-touch-10inch-lvds` | TI `sn65dsi84` (I2C 0x2c) to `panel-lvds` 1280 x 800 | `&dss1`, VP2 to `dsi0` |
| `panel-cap-touch-10inch-dsi` | `ti,sn65dsi83` to `panel-lvds` 1280 x 800 (via `display-rvt101hvdwc00b_overlay.dtsi`) | `&dss1`, VP2 to `dsi0` |
| `panel-cap-touch-7inch-dsi` | `ti,sn65dsi83`, 1024 x 600, 40.8/51.2/67.2 MHz (via `display-rvt70hslnwca0_overlay.dtsi`) | `&dss1`, VP2 to `dsi0` |
| three `-lvds-v2` variants (`panel-cap-touch-10inch-lvds-v2`, `mezzanine_panel-cap-touch-10inch-lvds-v2`, `dsi-to-lvds_..-lvds-v2`) | **not read** | **not read**; inferred to match the v1 file of the same name |

* LVDS overlays and DSI overlays drive different DSS instances (source).
* Which overlay Torizon enables by default is not stated in the repository
  (the `Makefile` only builds every `.dtbo`; `overlays/` has no list). Inferred:
  none until one is selected on the device; read it on the board.

## (3) The MCU R5F node (source, TDX 6.6)

`k3-am62p-mcu.dtsi`: cluster `mcu_r5fss0: r5fss@79000000`, `ti,am62-r5fss`,
power domain `k3_pds 7`; core `mcu_r5fss0_core0: r5f@79000000`, compatible
`ti,am62-r5f`; atcm `0x79000000`, btcm `0x79020000` (32 KiB each); SCI dev-id
9, proc-ids `0x03 0xff`; `firmware-name = "am62p-mcu-r5f0_0-fw"`;
`ti,atcm-enable = <0>`, `ti,btcm-enable = <1>`, `ti,loczrama = <0>`;
`sram = <&mcu_ram>` (`0x79100000`, 512 KiB). Cluster default `disabled`.

`k3-am62p-verdin.dtsi` (about lines 152-192 and 872-905) overrides it:

```
reserved-memory {
	mcu_r5fss0_core0_dma_memory_region: memory@9b800000 {   /* shared-dma-pool, no-map */
		reg = <0x00 0x9b800000 0x00 0x100000>;
	mcu_r5fss0_core0_memory_region: memory@9b900000 {
		reg = <0x00 0x9b900000 0x00 0xf00000>;
	wkup_r5fss0_core0_dma_memory_region: memory@9c800000 {
		reg = <0x00 0x9c800000 0x00 0x100000>;
	wkup_r5fss0_core0_memory_region: memory@9c900000 {
		reg = <0x00 0x9c900000 0x00 0x01e00000>;
```

* `&mcu_r5fss0` and its core are `okay`; `memory-region = <dma region>,
  <memory region>` (DMA pool first); `mboxes = <&mailbox0_cluster1
  &mbox_mcu_r5_0>`, with `mbox_mcu_r5_0` `ti,mbox-rx = <0 0 0>`,
  `ti,mbox-tx = <1 0 0>`; `&mailbox0_cluster0` and `&mailbox0_cluster1` are
  `okay` (source).
* Sizes: DMA pool 1 MiB at `0x9b800000`, firmware region 15 MiB at
  `0x9b900000`, ending at `0x9c800000` where the wakeup R5F regions start
  (arithmetic on the quoted values). Also `tfa@9e780000` (`0x80000`) and
  `optee@9e800000` (`0x01800000`), both `no-map` (source).
* The wakeup core (`k3-am62p-wakeup.dtsi`, `r5fss@78000000`,
  `firmware-name = "am62-wkup-r5f0_0-fw"`) is `okay` with `memory-region` only
  and no `mboxes` line (source). That it is TI's device-manager R5F is
  **inferred**.
* The mailbox node definitions (`ti,am64-mailbox` clusters) were **not re-read**
  in TDX 6.6; their addresses are not given here.
* An image for the MCU R5F must fit the 15 MiB region; resource table and
  vrings sit in or next to the 1 MiB pool (**inferred** from the usual
  `ti_k3_r5_remoteproc` use of `shared-dma-pool`).

## (4) Is any DSS resource marked as owned by a remote core?

* **Binding (source, TDX 6.6, `Documentation/devicetree/bindings/display/ti/ti,am65x-dss.yaml`):**
  `ti,dss-shared-mode` (display shared between hosts; the host with `common`
  has exclusive access to global registers, the other uses `common1`),
  `ti,dss-shared-mode-planes` (`vidl` or `vid`, owned by Linux),
  `ti,dss-shared-mode-vp` (`vp1`/`vp2`), `ti,dss-shared-mode-common`,
  `ti,dss-shared-mode-vp-owned` (0 or 1: Linux write access to the port) and
  `ti,dss-shared-mode-plane-zorder`.
* **Only user in the tree (source):** the SK overlay
  `k3-am62p5-sk-dss-shared-mode.dtso`:

```
&dss0 {
	ti,dss-shared-mode;
	ti,dss-shared-mode-vp = "vp1";
	ti,dss-shared-mode-vp-owned = <0>;
	ti,dss-shared-mode-common = "common1";
	ti,dss-shared-mode-planes = "vid";
```

  with a 128 MiB `rtos_framebuffer_memory_region` at `0x93500000`. Meaning
  (inferred from the binding text): Linux keeps plane `vid` through `common1`
  and read-only `vp1`; a remote core drives `vp1`.
* **Verdin:** no `ti,dss-shared-mode*` property and no framebuffer region in
  the Verdin files or the display overlays read (source, `grep`). Answer: **no
  node marks DSS resources as remote-core owned** for Verdin AM62P; Linux owns
  the whole DSS unless the running board says otherwise (inferred; `-lvds-v2`
  overlays unread).

## For the board run

1. Kernel build and branch of the running dtb (ties it to TDX 6.6 or another).
2. Active overlay, resulting `dss0`/`dss1` status, ports and panel; compare
   with (2).
3. `mcu_r5fss0` and `memory-region` sizes reported by remoteproc: 1 MiB and
   15 MiB as in (3).

## Not done, for the planner

* The three `-lvds-v2` overlays and the TDX 6.6 mailbox nodes were not read:
  the farm's policy denied access to the cloned trees in this session and the
  web view of `git.toradex.com` answers with a bot challenge.
* Content comes from the earlier item's result (commit
  `22c05ddae25a9763193f1402c7582d3ed5de1da1`), condensed. If that doc lands
  first, take its version and drop this one.
