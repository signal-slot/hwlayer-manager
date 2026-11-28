# Verdin AM62P device tree: DSS, panels and R5F carve-outs, read from source

Written 2026-11-28 for plan #465, item `am62p-dt-source`. Source study only:
no device was used, and nothing here is **observed**. It is meant to tell the
M3 board run which device-tree facts to check on the board. The tidss driver is
studied in `docs/am62p-dss-source.md` and the R5F remoteproc driver in
`docs/am62p-r5f-remoteproc-source.md`; this note covers device-tree files only
and does not repeat them.

**This study is incomplete.** The Toradex kernel tree could not be read
(see the table), and the DSS node, the Verdin display wiring and the Toradex
display overlays were not obtained from any source. Sections (a), (b) and (d)
say so and give only what can be stated safely. Section (c) is read from
mainline. Do not treat a gap here as "the tree has no such node".

## Tags and what was actually read

Each statement is tagged **source** (file, repository, commit or tag) or
**inferred**. Nothing is observed.

| Source | Repository, ref, commit | Status |
| --- | --- | --- |
| Toradex kernel tree, branch `toradex_ti-linux-6.6.y` | `https://git.toradex.com/linux-toradex.git`, tip `9764efd3fe8e5f3fa04045919296c102acd7517e` (from `git ls-remote`, 2026-11-28). The branch `toradex_ti-linux-6.18.y` (tip `9797b77f3c86abc90e771a13bc1abcce0ee5dbb5`) also exists. | **Not read.** `git ls-remote` worked; a clone was refused in this session, and the cgit web interface answered with an anti-bot "Access Denied" page. The GitHub mirror path `toradex/linux-toradex` returned 404. Which of the two TI-based branches Torizon OS uses for Verdin AM62P is **inferred** (6.6 is the older Torizon 7 line, 6.18 the newer one); the sources read here do not say. |
| Toradex overlays, `device-tree-overlays` | `https://git.toradex.com/device-tree-overlays.git`, branch heads from `git ls-remote`: `toradex_ti-linux-5.10.y_bringup` `28a84e584ce575689110c12fcd517ab4a957dd36`, `toradex_ti-linux-6.1.y` `68f55f88d66c6f24a805bd15aca3c8892be9b5ba`, `toradex_ti-linux-6.6.y` `c93543ff886bd467219464c3cefabe8ed97d9464` | **Not read** (same cgit block). The GitHub copies `toradex/device-tree-overlays` and `toradex/device-trees` are archives (default branch `toradex_5.4.y` / `toradex_5.4-2.3.x-imx`, 48 and 153 commits) with i.MX overlays only; no file name there contains `am62p`. The `device-trees` README says the repository is deprecated in favour of a successor, and the overlays README says it is deprecated in favour of `device-trees` (page text, read 2026-11-28). |
| Mainline Linux `arch/arm64/boot/dts/ti/` | `https://github.com/torvalds/linux.git` tag `v6.18`, commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` (the tag object is `f7b88edb52c8dd01b7e576390d658ae6eef0e134`) | **Partly read** through GitHub web views, which cut files off at about 1000 lines or were rate limited. Read in full: `k3-am62p-ti-ipc-firmware.dtsi`, `k3-am62p-verdin-dev.dtsi`, `k3-am62p-verdin-wifi.dtsi`, `k3-am62p5-verdin-nonwifi-dev.dts`, `arch/arm64/boot/dts/ti/Makefile`, the tail of `k3-am62p-j722s-common-mcu.dtsi` and `-wakeup.dtsi`. Read in part (first ~1000 lines): `k3-am62p-j722s-common-main.dtsi` (1102 lines) and `k3-am62p-verdin.dtsi` (1418 lines). **Not read:** their tails, and the carrier files `-dahlia`, `-ivy`, `-mallow`, `-yavia`. |
| Mainline checkout in the dependency cache (`linux-mainline`) | Version `7.4-rc4` per its `Makefile` | Sparse: it has no `arch/arm64/boot/dts/ti`. Not used. |

Licences of what is quoted: `k3-am62p-ti-ipc-firmware.dtsi` carries
`SPDX-License-Identifier: GPL-2.0-only OR MIT` (TI); `k3-am62p5-verdin-nonwifi-dev.dts`
carries `GPL-2.0-or-later OR MIT` (Toradex). The SPDX line of the other files
was not seen. Quotes are short excerpts, not whole files.

Consequence for the question "what does Torizon run": the Verdin AM62P files
exist in mainline from at least v6.18, but Toradex ships its own TI-based
kernel branch, and its device tree may differ from mainline. Everything in (c)
is therefore **source for mainline v6.18** and **inferred** for the Toradex
tree.

## Board files that exist (source, mainline v6.18)

From `arch/arm64/boot/dts/ti/Makefile` (tag `v6.18`, commit
`7d0a66e4bb9081d75c82ec4957c50034cb0ea449`): the Verdin AM62P boards are
`k3-am62p5-verdin-{nonwifi,wifi}-{dahlia,dev,ivy,mallow,yavia}.dtb`. There is
no `.dtbo`/`.dtso` entry for Verdin AM62P in that Makefile (source). The
AM625 Verdin has display overlays in the same directory (`k3-am625-verdin-dsi-to-hdmi.dtso`,
`...-dsi-to-lvds-panel-cap-touch-10inch.dtso`, `...-panel-cap-touch-10inch-lvds.dtso`,
`...-panel-cap-touch-7inch-dsi.dtso`, a GitHub directory listing of the same tag),
but those are for the AM625 SoM, not AM62P (source for existence; inferred that
the AM62P ones will have a similar naming).

`k3-am62p5-verdin-nonwifi-dev.dts` (same tag) is 22 lines and includes, in this
order, `k3-am62p5.dtsi`, `k3-am62p-verdin.dtsi`, `k3-am62p-verdin-nonwifi.dtsi`
and `k3-am62p-verdin-dev.dtsi`; its compatible list is
`toradex,verdin-am62p-nonwifi-dev`, `toradex,verdin-am62p-nonwifi`,
`toradex,verdin-am62p`, `ti,am62p5` (source). It does **not** include
`k3-am62p-ti-ipc-firmware.dtsi` (source for the `.dts`; the include list of the
unread tail of `k3-am62p-verdin.dtsi` is unknown).

## (a) DSS node, video ports, planes — **not obtained**

* Mainline v6.18, `k3-am62p.dtsi`: the `cbass_main` ranges contain a window
  commented `/* DSS */` at `0x30200000`, size `0x10000`, and one commented
  `/* VPU */` at `0x30210000` (source, the agent that read the file reported
  these two lines from the GitHub view).
* The `dss:` node itself (compatible, reg, clocks, power-domains, ports and
  remote endpoints, OLDI nodes) was **not read**. It is not in the first ~1000
  lines of `k3-am62p-j722s-common-main.dtsi`, and the file's last 100 lines
  were not available. `k3-am62p-main.dtsi` (82 lines, read in full) holds only
  USB, OC-SRAM, DMSS interrupt, audio clock, GPIO and SDHCI overrides and **no**
  display node (source).
* A hand decode of the file through the GitHub contents API found no `dss`
  or OLDI node either, but that decode was partial and is not relied on here.
* Inferred (from the driver study, not from a dts): the AM62P display subsystem
  is expected to be reachable at `0x30200000`, and the number of planes and
  video ports follows the tidss compatible; see `docs/am62p-dss-source.md`.
  Which ports are wired to OLDI, DPI or a DSI bridge on Verdin is **unknown
  from the sources read**.

What the board run should do: read `/proc/device-tree` or `dtc -I fs` on the
board to fill this section; it is a board task, not source work.

## (b) Toradex display overlays for Verdin AM62P — **not obtained**

The overlay files, their panels or bridges, timing source, VP use and the
default overlay were not read (the Toradex repositories were blocked, see the
table). What is known:

* Mainline v6.18 has no Verdin AM62P display overlay (Makefile, source).
* The AM625 Verdin overlay names listed above suggest the Toradex interfaces
  (DSI to HDMI, DSI to LVDS, 10-inch and 7-inch capacitive-touch panels,
  Raspberry Pi display); whether the AM62P tree has the same set is
  **inferred**, not read.
* Pin comments in `k3-am62p-verdin.dtsi` (first ~1000 lines, tag `v6.18`)
  show the SoM routes `Verdin PWM_3_DSI`, `DSI_1_INT#`, `DSI_1_BKL_EN#`,
  `I2C_2_DSI` (`main_i2c1`) and `I2C_3_HDMI` (`mcu_i2c0`). They show that the
  module offers a DSI connector with I2C and backlight lines, and an HDMI
  bridge I2C line (source). No VOUT0 (DPI) pin group was found in the part read.
* `k3-am62p-verdin-dev.dtsi` (read in full) enables `&epwm0` (PWM_3_DSI),
  `&main_i2c1` (I2C_2_DSI) and `&mcu_i2c0` (I2C_3_HDMI) and has no `&dss`,
  `&dsi`, `&oldi` or panel nodes (source).
* Inferred: at v6.18 the Verdin AM62P mainline tree does not itself describe a
  panel; display support, if present on Torizon, comes from the Toradex branch
  and overlays. Which overlay is enabled by default is unknown.

## (c) R5F remoteproc nodes, carve-outs, mailboxes

### MCU R5F and wakeup R5F (source, mainline v6.18)

`k3-am62p-j722s-common-mcu.dtsi` (tag `v6.18`, commit
`7d0a66e4bb9081d75c82ec4957c50034cb0ea449`), lines 187 to 210:

```
mcu_r5fss0: r5fss@79000000 {
	compatible = "ti,am62-r5fss";
	ranges = <0x79000000 0x00 0x79000000 0x8000>,
		 <0x79020000 0x00 0x79020000 0x8000>;
	power-domains = <&k3_pds 7 TI_SCI_PD_EXCLUSIVE>;
	status = "disabled";
	mcu_r5fss0_core0: r5f@79000000 {
		compatible = "ti,am62-r5f";
		reg-names = "atcm", "btcm";
		ti,sci-dev-id = <9>;
		ti,sci-proc-ids = <0x03 0xff>;
		firmware-name = "am62p-mcu-r5f0_0-fw";
		ti,atcm-enable = <0>;
		ti,btcm-enable = <1>;
		ti,loczrama = <0>;
		status = "disabled";
```

(abridged by omitting `#address-cells`, `#size-cells`, `reg`, `ti,sci`,
`resets` lines). ATCM and BTCM are 32 KiB each (`0x8000`) at `0x79000000` and
`0x79020000`. Both the cluster and the core are `disabled` here (source).

`k3-am62p-j722s-common-wakeup.dtsi`, same tag, lines 118 to 141:
`wkup_r5fss0: r5fss@78000000`, same compatibles, ATCM `0x78000000` and BTCM
`0x78100000` (32 KiB each), `ti,sci-dev-id = <121>`, `ti,sci-proc-ids = <0x01 0xff>`,
`firmware-name = "am62-wkup-r5f0_0-fw"`, `ti,atcm-enable = <1>`, `ti,loczrama = <1>`,
`status = "disabled"` (source). The wakeup core is the one the TI documentation
calls the device-manager R5F; that mapping is **inferred**, the dts does not say it.

### Mailboxes (source)

`k3-am62p-j722s-common-main.dtsi` (same tag) defines four nodes with
`compatible = "ti,am64-mailbox"`, all `status = "disabled"`, `ti,mbox-num-users = <4>`,
`ti,mbox-num-fifos = <16>`:

| Label | Address | GIC SPI |
| --- | --- | --- |
| `mailbox0_cluster0` | `0x29000000` | 76 |
| `mailbox0_cluster1` | `0x29010000` | 77 |
| `mailbox0_cluster2` | `0x29020000` | 108 |
| `mailbox0_cluster3` | `0x29030000` | 109 |

### Carve-outs and mailbox wiring in the TI IPC include (source)

`k3-am62p-ti-ipc-firmware.dtsi` (tag `v6.18`, SPDX `GPL-2.0-only OR MIT`, 60
lines) enables the R5Fs and defines:

```
mcu_r5fss0_core0_dma_memory_region: memory@9b800000 {  /* reg 0x9b800000, size 0x100000 */
mcu_r5fss0_core0_memory_region:     memory@9b900000 {  /* reg 0x9b900000, size 0xf00000  */
```

both `compatible = "shared-dma-pool"`, `no-map`. The MCU core gets
`mboxes = <&mailbox0_cluster1 &mbox_mcu_r5_0>` with `ti,mbox-rx = <0 0 0>`,
`ti,mbox-tx = <1 0 0>`, and `memory-region = <dma region>, <memory region>`
(DMA first). The wakeup core gets `mailbox0_cluster0`/`mbox_r5_0` with the same
rx/tx numbers and the `wkup_r5fss0_core0_*` regions.

`k3-am62p-verdin.dtsi` (first ~1000 lines, same tag) has, in its
`reserved-memory` node: `tfa@9e780000` (`0x9e780000`, `0x80000`, no-map),
`optee@9e800000` (`0x9e800000`, `0x01800000`, no-map), and
`wkup_r5fss0_core0_dma_memory_region` `memory@9c800000` (`0x9c800000`, `0x100000`)
and `wkup_r5fss0_core0_memory_region` `memory@9c900000` (`0x9c900000`, `0x01e00000`),
both `shared-dma-pool`, `no-map` (source).

Facts that follow:

* Mainline v6.18 gives the **wakeup** R5F carve-outs in the Verdin dtsi and the
  **MCU** R5F carve-outs only in the TI IPC include (source).
* The Verdin board `.dts` read does not include that IPC dtsi, so in that
  mainline board file the MCU R5F is likely to stay `disabled` and without a
  memory region (**inferred**; the tail of `k3-am62p-verdin.dtsi` is unread).
  The MCU R5F carve-out at `0x9b800000`/`0x9b900000` is then not available from
  mainline alone. Whether the Toradex branch or a Torizon overlay adds it is
  **not known**; this is the first thing to check on the board
  (`docs/board-test-am62p-r5f.md` and `docs/remoteproc-bringup.md`).
* The two MCU regions are contiguous (`0x9b800000` to `0x9c800000`), the
  wakeup regions likewise (`0x9c800000` to `0x9e700000`), and the OP-TEE/TF-A
  regions start at `0x9e780000` (**inferred** from the arithmetic of the
  quoted values). The sizes (1 MiB DMA pool, 15 MiB for the MCU image;
  1 MiB, 30 MiB for wakeup) bound what a firmware image can place there.
* Firmware names: the MCU core asks Linux for `am62p-mcu-r5f0_0-fw` and the
  wakeup core for `am62-wkup-r5f0_0-fw` (source). Whether the board test
  overrides the name through sysfs is a driver matter; see
  `docs/am62p-r5f-remoteproc-source.md`.

## (d) DSS planes or ports owned by a remote core — **not found**

Nothing in the files read marks a plane or video port as owned by a remote
core. No `ti,dss-shared`-style property appears in the nodes quoted above, but
the DSS node was not read, so this is **absence of evidence, not evidence of
absence** (inferred). The only remote-core object seen is the R5F cluster and
mailbox wiring above. The remote DSS sharing support, if any, is a driver and
firmware matter (`docs/am62p-dss-source.md`), and for the board test the
shared state is held by the R5F image itself (`docs/board-test-am62p-r5f.md`).

## What the M3 board run should confirm (all open)

1. Which dts the running kernel was built from (Toradex branch and commit),
   to close the "not read" row above.
2. The `dss` node, its ports and the enabled overlay (a) and (b).
3. Whether the MCU R5F has `memory-region` entries and which addresses,
   compared with `0x9b800000` and `0x9b900000` (c).
4. Whether `mcu_r5fss0` and `mailbox0_cluster1` are `okay` (c).

## Follow-ups for the planner

Re-read the DSS node and the Toradex overlays when a clone of
`linux-toradex` (`toradex_ti-linux-6.6.y` or `-6.18.y`) is possible from a node
that is not blocked by the anti-bot page; then this note's sections (a), (b),
(d) can be completed and the "inferred" lines replaced.
