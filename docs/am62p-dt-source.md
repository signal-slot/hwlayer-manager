# Verdin AM62P device tree: DSS, panels and R5F carve-outs, read from source

Written 2026-11-28 for plan #465, item `am62p-dt-source`. Source study: no
device was used and nothing here is **observed**. It tells the M3 board run
which device-tree facts to check on the board. The tidss driver is studied in
`docs/am62p-dss-source.md` and the R5F remoteproc driver in
`docs/am62p-r5f-remoteproc-source.md`; this note covers device-tree files only.

Short answer:

* The Toradex kernel tree describes **two** DSS instances (`dss0`, `dss1`),
  both `disabled` in the SoC dtsi, and the Verdin AM62P files do **not** enable
  or wire either of them. Display wiring on Verdin comes from Toradex overlays
  (section b): LVDS panels use `dss0` VP1 through OLDI, every DSI overlay uses
  `dss1` VP2 through `dsi0` and a bridge. No overlay uses DPI.
* The Toradex Verdin dtsi enables both R5F clusters and carves out memory for
  them: MCU R5F `0x9b800000` (1 MiB DMA pool) and `0x9b900000` (15 MiB),
  wakeup R5F `0x9c800000` and `0x9c900000`.
* TI's tree has a DSS "shared mode" (`ti,dss-shared-mode*`) that marks a video
  port and planes as owned by a remote core. It exists for the AM62P SK board
  as an overlay; nothing in the Verdin files uses it.

## Tags and sources

Each statement is tagged **source** (file, repository, commit) or **inferred**.

| Short name | Repository, ref, commit | Status |
| --- | --- | --- |
| **TDX 6.6** | `https://git.toradex.com/linux-toradex.git`, branch `toradex_ti-linux-6.6.y`, commit `9764efd3fe8e5f3fa04045919296c102acd7517e` (`Merge tag 'v6.6.163'`, 2026-11-23) | **Read** from a blobless clone (`git show`, `git grep`), 2026-11-28. All "source" tags below without another name mean this tree. |
| other Toradex branches | same repository; `toradex_ti-linux-6.18.y` at `9797b77f3c86abc90e771a13bc1abcce0ee5dbb5`, `toradex_ti-linux-6.1.y` at `c5d9d94e5c3a5b9283dbf861886fd761bb4a0e26` (from `git ls-remote`) | **Not read.** That Torizon OS uses `toradex_ti-linux-6.6.y` for Verdin AM62P is **inferred** (the branch carries the Verdin AM62P board files; the 6.18 branch may supersede it in newer Torizon releases). |
| **overlays** | `https://git.toradex.com/device-tree-overlays.git`, branch `toradex_ti-linux-6.6.y`, commit `c93543ff886bd467219464c3cefabe8ed97d9464` (same as `git ls-remote` tip) | **Read** from a blobless clone, 2026-11-28, directory `overlays/`. The GitHub archives `toradex/device-tree-overlays` and `toradex/device-trees` hold only 5.4-era i.MX overlays and were not used. |
| **mainline v6.18** | `https://github.com/torvalds/linux.git`, tag `v6.18`, commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` | Partly read via GitHub web views; used only for the comparison in section (c). |

Licences of quoted excerpts: `k3-am62p-main.dtsi`, `k3-am62p-mcu.dtsi` and
`k3-am62p-wakeup.dtsi` carry `GPL-2.0-only OR MIT` (TI); the Verdin board
`.dts` and the SK overlays carry `GPL-2.0-or-later OR MIT`. Quotes are short
excerpts, not whole files. All paths are under `arch/arm64/boot/dts/ti/` of
TDX 6.6 unless stated.

## Files that exist (source, TDX 6.6)

Verdin AM62P boards (`Makefile`): `k3-am62p5-verdin-{nonwifi,wifi}-{dahlia,dev,ivy,mallow,yavia}.dtb`
(the tree also has `-zinnia` `.dts` files, not in the Makefile lines read).
`k3-am62p5-verdin-nonwifi-dev.dts` includes `k3-am62p5.dtsi`, `k3-am62p-verdin.dtsi`,
`k3-am62p-verdin-nonwifi.dtsi`, `k3-am62p-verdin-dev.dtsi`; compatible
`toradex,verdin-am62p-nonwifi-dev`, `toradex,verdin-am62p-nonwifi`,
`toradex,verdin-am62p`, `ti,am62p5`. `k3-am62p.dtsi` includes the SoC
`-main`, `-mcu`, `-wakeup`, `-thermal` files. **There is no Verdin AM62P
`.dtso` in the tree**; the `.dtso` files named `k3-am62p5-sk-*` are for TI's SK
board (source).

## (a) DSS nodes, ports and planes (source, TDX 6.6, `k3-am62p-main.dtsi`)

The tree has two DSS instances. Excerpt of `dss0` (lines 1186 onward, trimmed):

```
dss0: dss@30200000 {
	compatible = "ti,am62p51-dss";
	reg = <0x00 0x30200000 0x00 0x1000>, /* common */
	      <0x00 0x30202000 0x00 0x1000>, /* vidl1 */
	      <0x00 0x30206000 0x00 0x1000>, /* vid */
	      <0x00 0x30207000 0x00 0x1000>, /* ovr1 */
	      <0x00 0x30208000 0x00 0x1000>, /* ovr2 */
	      <0x00 0x3020a000 0x00 0x1000>, /* vp1: Used for OLDI */
	      <0x00 0x3020b000 0x00 0x1000>, /* vp2: Used as DPI Out */
	      <0x00 0x30201000 0x00 0x1000>; /* common1 */
	reg-names = "common", "vidl1", "vid", "ovr1", "ovr2", "vp1", "vp2", "common1";
	power-domains = <&k3_pds 186 ...>, <&k3_pds 243 ...>, <&k3_pds 244 ...>;
	clocks = <&k3_clks 186 6>, <&dss0_vp1_clk>, <&k3_clks 186 2>;
	clock-names = "fck", "vp1", "vp2";
	interrupts = <GIC_SPI 84 IRQ_TYPE_LEVEL_HIGH>;
	status = "disabled";
```

| Item | `dss0` | `dss1` |
| --- | --- | --- |
| compatible | `ti,am62p51-dss` | `ti,am62p52-dss` |
| base | `0x30200000` | `0x30220000` |
| register blocks | common, vidl1, vid, ovr1, ovr2, vp1, vp2, common1 | same names, offsets `+0x20000` |
| power domains (k3_pds ids) | 186 (DSS0), 243 (OLDI0), 244 (OLDI1) | 232 (DSS), 244 (OLDI1) |
| clocks | `fck` = clk 186/6, `vp1` = `dss0_vp1_clk`, `vp2` = clk 186/2 | `fck` = clk 232/8, `vp1` = `dss1_vp1_clk`, `vp2` = clk 232/4 |
| interrupt (GIC SPI) | 84 | 30 |
| OLDI transmitters | `oldi0_dss0` (reg 0, `ti,companion-oldi = <&oldi1_dss0>`), `oldi1_dss0` (reg 1, `ti,secondary-oldi`) | `oldi1_dss1` (reg 0) |
| status in the SoC dtsi | `disabled` | `disabled` |

Further facts (source):

* Planes: each instance lists **one** `vidl1` and **one** `vid` register block,
  so the binding implies two planes per instance (one VIDL, one VID), two
  overlay managers (`ovr1`, `ovr2`) and two video ports (`vp1`, `vp2`). The
  `common1` block is the second register space used by shared mode.
* The reg comments say `vp1` is "Used for OLDI" and `vp2` "Used as DPI Out".
  `dss0_vp1_clk` and `dss1_vp1_clk` are `fixed-factor-clock` dividers by 7 of
  clk 186/0 and clk 232/0 (`k3-am62p.dtsi`).
* A single OLDI-IO control syscon `dss_oldi_io_ctrl` at offset `0x8600`
  (size `0x200`) is referenced through `ti,oldi-io-ctrl` by all OLDI nodes.
* `dsi0: dsi@30500000` (`ti,j721e-dsi`, wrapper at `0x30270000`) with D-PHY
  `dphy_tx0` (`ti,j721e-dphy`, `0x301c0000`) exist, both `disabled`. The SoC
  `cbass_main` ranges map `DSS`, `DSS1`, `DSI Wrapper` and `DSI Config`
  (`k3-am62p.dtsi`).
* The port graph (`dss0_ports`, `oldi*_ports`) is empty in the SoC dtsi; the
  boards fill it. Exemplar from TI's own SK board (source, `k3-am62p5-sk.dts`
  and `*.dtso`): `dss0` VP2 (`port@1`) goes to an HDMI transmitter
  (`sii9022`), `dss0` VP1 (`port@0`, two endpoints) goes to OLDI0 and OLDI1
  for a dual-link LVDS panel overlay, and `dss1` VP2 goes to `dsi0` and a
  Toshiba `tc358762` bridge for the RPi 7-inch DSI panel overlay. The SK
  LVDS overlay notes that OLDI TX 0 sends the odd pixels and TX 1 the even
  ones, a fixed configuration.
* Inferred from the above: for a Verdin AM62P the likely routes are VP1 to
  OLDI/LVDS, VP2 to DPI (parallel RGB or an HDMI bridge on the carrier) and
  `dss1` VP2 to DSI, but **no Verdin file in the tree sets any of it** (section b).

## (b) Display overlays for Verdin AM62P (source, overlays repository)

Source for this section: `device-tree-overlays`, branch `toradex_ti-linux-6.6.y`,
commit `c93543ff886bd467219464c3cefabe8ed97d9464`, directory `overlays/`,
files `verdin-am62p_*_overlay.dts` (SPDX line of the files read:
`GPL-2.0-or-later OR MIT`). Each has `compatible = "toradex,verdin-am62p"`.
The base dtb (TDX 6.6, section a) enables no display, so these overlays are
what turns `dss0` or `dss1` on.

The tree has no base-dtb display wiring (source, TDX 6.6): a case-insensitive
grep of `k3-am62p-verdin*.dtsi` and `k3-am62p5.dtsi` for `dss`, `oldi`, `dsi`,
`lvds`, `hdmi`, `panel`, `vout` finds only pin comments (`Verdin PWM_3_DSI`,
`I2C_2_DSI`, `I2C_3_HDMI`, `DSI_1_INT#`, `DSI_1_BLK_EN#`) and VOUT0 pads used
as GPIO or UART functions.

| Overlay file (`verdin-am62p_...`) | Panel or bridge | Timing source | DSS and VP | Path |
| --- | --- | --- | --- | --- |
| `panel-cap-touch-10inch-lvds` (single-channel LVDS, SoM connector) | `panel-lvds`, `data-mapping = "vesa-24"`, `pwm-backlight`, Atmel maXTouch at 0x4a | `panel-timing` in the overlay: clock 68.9 / 71.1 / 73.4 MHz (min/typ/max), 1280 x 800 | `&dss0` okay, comment "VP1: Output to OLDI" (`dss0_ports` port@0, two endpoints) | VP1 to `oldi0_dss0` and `oldi1_dss0` to the panel. `oldi1_dss0` is enabled too, with a comment calling it a hack ("pretend we are in clone mode") |
| `mezzanine_panel-cap-touch-10inch-lvds` | same panel data, backlight from `verdin-am62p_mezzanine_backlight.dtsi` | same: 68.9 / 71.1 / 73.4 MHz, 1280 x 800 | `&dss0`, VP1 to OLDI0 and OLDI1 | same hack for OLDI1 |
| `mezzanine_panel-lvds-dual-channel-1080p` | LG LP156WF1 dual-channel LVDS, `panel-lvds`, `data-mapping = "jeida-24"`, `dual-lvds-odd-pixels` / `dual-lvds-even-pixels` | 138.5 MHz, 1920 x 1080 | `&dss0`, VP1 to OLDI0 (odd) and OLDI1 (even) | true dual link |
| `dsi-to-hdmi` | Lontium `lt8912b` bridge (I2C 0x48) and an `hdmi-connector` | not in the overlay (taken from the sink by the bridge; **inferred**) | `&dss1` okay; `dss1_ports` port@1 (`dss1_dpi1_out`, VP2) to `dsi0` | `dss1` VP2 to `dsi0` to `lt8912b` to HDMI |
| `dsi-to-lvds_panel-cap-touch-10inch-lvds` | TI `sn65dsi84` bridge (I2C 0x2c) to `panel-lvds`, `pwm-backlight`, maXTouch | `panel-timing`: 68.9 / 71.1 / 73.4 MHz | `&dss1`, VP2 to `dsi0` | `dss1` VP2 to `dsi0` to `sn65dsi84` to LVDS panel |
| `panel-cap-touch-10inch-dsi` | `ti,sn65dsi83` on the panel board to the `panel-lvds` of `display-rvt101hvdwc00b_overlay.dtsi`, ILI2132 touch | 68.9 / 71.1 / 73.4 MHz, 1280 x 800 (that dtsi) | `&dss1`, VP2 to `dsi0` | same as above |
| `panel-cap-touch-7inch-dsi` | `ti,sn65dsi83` to the panel of `display-rvt70hslnwca0_overlay.dtsi`, ILI2132 touch | 40.8 / 51.2 / 67.2 MHz, 1024 x 600 | `&dss1`, VP2 to `dsi0` | same |
| the `-lvds-v2` variants (`panel-cap-touch-10inch-lvds-v2`, `mezzanine_panel-cap-touch-10inch-lvds-v2`, `dsi-to-lvds_..-lvds-v2`) | **not read** (listed in the Makefile) | | | |

Findings (source unless marked):

* **No overlay uses the DPI output** (`dss0` VP2 or a parallel panel): none of
  the files read references a DPI endpoint on `dss0` port@1; the SK board's
  HDMI route (section a) has no Verdin counterpart in this repository.
* LVDS panels use `dss0` VP1 (OLDI); every DSI route uses `dss1` VP2. A board
  with an LVDS overlay and one with a DSI overlay therefore drive **different
  DSS instances**. Whether both families can be applied together is not
  stated (inferred: nothing in the files read forbids it, none was seen doing it).
* **Which overlay Torizon enables by default is not stated** in the
  repository: its `Makefile` only builds all `.dtbo` files (lines 52 to 66),
  and the `overlays/` directory holds no list or README. Inferred: the image
  runs with **no display overlay** until one is selected on the device; this
  must be read on the board.
* AM62P `.dtbo` list in the `Makefile`: dsi-to-hdmi, dsi-to-lvds (10 inch, v1
  and v2), mezzanine LVDS (10 inch v1 and v2, dual-channel 1080p), panel
  7 inch DSI, panel 10 inch DSI, panel 10 inch LVDS (v1 and v2), plus
  non-display overlays (nau8822, ov5640 and ov5640-27mhz, spidev, uart4-mcu).

For the M3 report the panel and the active overlay still have to be taken from
the running board; this table is what to compare with.

## (c) R5F remoteproc nodes, carve-outs, mailboxes

### Cores (source, TDX 6.6)

| | MCU R5F | Wakeup R5F |
| --- | --- | --- |
| file | `k3-am62p-mcu.dtsi` | `k3-am62p-wakeup.dtsi` |
| cluster | `mcu_r5fss0: r5fss@79000000`, `ti,am62-r5fss`, power domain `k3_pds 7` | `wkup_r5fss0: r5fss@78000000`, `ti,am62-r5fss`, `k3_pds 119` |
| core | `mcu_r5fss0_core0: r5f@79000000`, `ti,am62-r5f` | `wkup_r5fss0_core0: r5f@78000000`, `ti,am62-r5f` |
| TCM | atcm `0x79000000`, btcm `0x79020000`, 32 KiB each | atcm `0x78000000`, btcm `0x78100000`, 32 KiB each |
| SCI | dev-id 9, proc-ids `0x03 0xff`, reset `k3_reset 9 1` | dev-id 121, proc-ids `0x01 0xff`, reset `k3_reset 121 1` |
| `firmware-name` | `am62p-mcu-r5f0_0-fw` | `am62-wkup-r5f0_0-fw` |
| TCM enable | `ti,atcm-enable = <0>`, `ti,btcm-enable = <1>`, `ti,loczrama = <0>` | `<1>`, `<1>`, `ti,loczrama = <1>` |
| other | `sram = <&mcu_ram>` (`0x79100000`, 512 KiB), `wakeup-source` | none |
| default status | `disabled` on the cluster, none on the core | `disabled` on the cluster |

The wakeup core is the one TI calls the device-manager R5F; this mapping is
**inferred** (the dts does not say it). The SK overlay's firmware label
`dss_display_share.wkup-r5f0_0.release.strip.out` supports it (source).

### Verdin overrides (source, TDX 6.6, `k3-am62p-verdin.dtsi`, lines 152-192 and 872-905)

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

Also present: `tfa@9e780000` (`0x80000`, no-map) and `optee@9e800000`
(`0x01800000`, no-map). In the same file `&mailbox0_cluster0` and
`&mailbox0_cluster1` are `okay`; `mbox_mcu_r5_0` has `ti,mbox-rx = <0 0 0>`,
`ti,mbox-tx = <1 0 0>`; `&wkup_r5fss0`, `&mcu_r5fss0` and both cores are
`okay`. The MCU core gets `mboxes = <&mailbox0_cluster1 &mbox_mcu_r5_0>` and
`memory-region = <dma region>, <memory region>` (DMA pool first). The wakeup
core gets only `memory-region` here, **no `mboxes`** (source: the override has
no such line). Mailbox nodes themselves (`ti,am64-mailbox`, cluster 0 to 3 at
`0x29000000` + `0x10000` steps, 4 users, 16 FIFOs) come from the SoC dtsi
(source for cluster addresses at mainline v6.18; not re-read in TDX 6.6).

Facts that follow:

* **The MCU R5F is enabled with its carve-out in the Toradex Verdin dtsi.**
  The board image's code and data must fit `0x9b900000` to `0x9c800000`
  (15 MiB), with the resource table and rpmsg/vring in or next to the DMA
  pool at `0x9b800000` (1 MiB) (**inferred**: placement follows the usual
  `shared-dma-pool` use by `ti_k3_r5_remoteproc`; see the remoteproc study).
* MCU regions end at `0x9c800000` and the wakeup regions start there
  (`0x9c800000` to `0x9e700000`); TF-A starts at `0x9e780000` (**inferred** by
  arithmetic from the quoted values).
* Mainline v6.18 differs: there the MCU carve-outs live in
  `k3-am62p-ti-ipc-firmware.dtsi`, which the mainline Verdin `.dts` does not
  include, so the MCU core stays disabled in mainline (source for the
  includes, **inferred** for the effect). TDX 6.6 does not have that split.
* Because both cores are `okay` and have firmware names, Linux can start
  either through remoteproc; whether the Torizon boot chain already loads
  firmware into the wakeup core is a boot-chain question for
  `am62p-torizon-bsp-source`, not answered by device-tree files.

## (d) DSS planes or ports owned by a remote core (source)

TI's tree has a **DSS shared mode**. Binding (TDX 6.6,
`Documentation/devicetree/bindings/display/ti/ti,am65x-dss.yaml`) properties:

* `ti,dss-shared-mode` (boolean): "TI DSS7 supports sharing of display between
  multiple hosts"; the host with `common` has exclusive access to the global
  registers, the other uses `common1`; planes can be partitioned between
  Linux and a remote core.
* `ti,dss-shared-mode-planes` (`vidl` or `vid`): the plane owned by Linux.
* `ti,dss-shared-mode-vp` (`vp1` or `vp2`): the video port; shared between
  Linux and a remote core, with the remote core holding write access when
  Linux does not own it.
* `ti,dss-shared-mode-common` (`common` or `common1`): the register region
  owned by Linux.
* `ti,dss-shared-mode-vp-owned` (0 or 1): whether Linux has write access to the
  port.
* `ti,dss-shared-mode-plane-zorder` (0 or 1): z-order of Linux's planes; when
  Linux has no write access it is "just informational".

The only user in the tree is the SK overlay `k3-am62p5-sk-dss-shared-mode.dtso`
(compatible with Device Manager firmware `dss_display_share.wkup-r5f0_0...`):

```
&dss0 {
	ti,dss-shared-mode;
	ti,dss-shared-mode-vp = "vp1";
	ti,dss-shared-mode-vp-owned = <0>;
	ti,dss-shared-mode-common = "common1";
	ti,dss-shared-mode-planes = "vid";
	ti,dss-shared-mode-plane-zorder = <0>;
	interrupts = <GIC_SPI 85 IRQ_TYPE_LEVEL_HIGH>;
};
&reserved_memory {
	rtos_framebuffer_memory_region: rtos-framebuffer-memory@93500000 {
		reg = <0x00 0x93500000 0x00 0x08000000>;   /* no-map, 128 MiB */
```

Meaning (inferred from the binding text): Linux owns the `vid` plane through
`common1`, has read-only access to `vp1`, and a remote core (the wakeup R5F
running the shared-display firmware) drives `vp1`; the remote core's
framebuffer pool is the 128 MiB region at `0x93500000`. Nothing in the Verdin
files uses any `ti,dss-shared-mode*` property or this region (source:
`grep` of the Verdin files above). So on Verdin, Linux owns the whole DSS
unless a Toradex overlay says otherwise (**inferred**). The Verdin overlays
listed in section (b) were read for display wiring only: their descriptions
there (`&dss0`/`&dss1` okay, ports, panels) mention no remote core, but a
`grep` for `ti,dss-shared-mode`, `r5f` or `reserved-memory` over
`verdin-am62p_*` was **not run** (no network access in the follow-up session),
so "no Toradex overlay marks DSS as remote-owned" is **inferred**, not checked.
The three `-lvds-v2` overlays remain unread.

For the M3 test: the MCU R5F in `docs/board-test-am62p-r5f.md` scenes writes
DSS planes through its own driver while Linux's tidss is stopped or unbound;
it does not use the shared mode (inferred from that doc's route; this note
does not re-read it).

## What the M3 board run should confirm

1. The running dtb's source: kernel build and branch, to tie it to TDX 6.6 or
   another branch.
2. Which overlay is active, and the resulting `dss0`/`dss1` status, ports and
   panel; compare with the table in section (b).
3. That `mcu_r5fss0` and its `memory-region` exist as in (c), and the sizes the
   remoteproc core reports match 1 MiB and 15 MiB.
4. Whether the wakeup R5F is running firmware (boot-chain question).

## For the planner

Not read: the three `-lvds-v2` overlays and the files outside `overlays/`
(for example how Torizon picks the active overlay). The second would come from
the Torizon boot-chain study.
