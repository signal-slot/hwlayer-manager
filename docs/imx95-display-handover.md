# i.MX95: ways to hand the display from Linux to the M7, against the board rules

Date: 2026-11-28. Scope: SoW §5.1 / §9 M2 (WP3). Source study. **No board was
accessed for this note and nothing was built or run.** Every finding is tagged
**read from source** (content fetched and seen at the pin in §1) or
**inferred** (my reasoning from it). Three tags carry a qualifier:
"read from source (secondary)" is a Toradex community post, not BSP source,
"read from source (other doc)" is a fact taken from a sibling note that
cites its own pin, and "observed 2026-11-27" is a fact seen on the board in
the read-only snapshots [observed-2026-11-27.txt](imx95-zephyr/observed-2026-11-27.txt)
(S1) and [observed-2026-11-28.txt](imx95-zephyr/observed-2026-11-28.txt) (S2),
which this note cites but did not take. No board addresses, host names or credentials appear.

Read first: [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md) (it is
not edited here). This note extends its §4 ("instead of Linux") and replaces an
earlier desk version of this file that rested only on sibling notes. Related:
[imx95-sm-display-lm.md](imx95-sm-display-lm.md),
[imx95-display-power.md](imx95-display-power.md),
[imx95-display-clocks.md](imx95-display-clocks.md),
[imx95-m7-reduced-scope.md](imx95-m7-reduced-scope.md),
[imx95-m7-overlay-design.md](imx95-m7-overlay-design.md),
[imx95-m7-fb-memory.md](imx95-m7-fb-memory.md) and the board rules in
[imx95-board.md](imx95-board.md).

## 1. Sources and pins

| Id | Source | Pin | Read |
|---|---|---|---|
| LX | NXP linux-imx, <https://github.com/nxp-imx/linux-imx> | tag `lf-6.6.52-2.2.2` = commit `5a0a5e71d2bd` (as recorded in the feasibility note, S6; not re-resolved here). On 2026-11-28 the GitHub API showed the tag as an annotated tag object `f9a8e33f5cde3e96a57569e9dfa70481fe688943`; its target commit was not fetched | raw files at the tag: `drivers/gpu/drm/imx/dpu95/dpu95-drv.c` (241 lines), `dpu95-kms.c`, `drivers/gpu/drm/bridge/imx/imx95-ldb.c` (541 lines), `drivers/of/platform.c`, `drivers/of/Kconfig` |
| MK | NXP imx-mkimage, <https://github.com/nxp-imx/imx-mkimage> | tag `lf-6.6.52-2.2.2` = commit `81fca6434be0` (feasibility S3) | `iMX95/soc.mak`, `README` |
| SM | NXP imx-sm, <https://github.com/nxp-imx/imx-sm> | tag `lf-6.6.52-2.2.2`, same commit as `rel_imx_sm_2025q3` = `af1e37026c6ba19cdca98fd6e91494efd3e7b5ec` (the later tag `rel_imx_sm_2026q3` = `eec23287` is the pin of [imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md), not used here; neither is shown to be the board's SM, firmware version 0x23e, observed 2026-11-27) | `README.md`; everything else via the sibling notes |
| TX | Toradex community, <https://community.toradex.com/t/how-to-apply-a-device-tree-overlay/29036> and the Verdin iMX95 threads linked from it | no commit; forum posts, seen 2026-11-28 through a search result summary, **not the page itself** | overlay mechanism of Yocto BSPs (secondary) |
| TK | Toradex kernel `toradex_6.6-2.2.x-imx` = `8ad2b7ff6b5666b25b1b43819c83b9ae3930b8df` | see [imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md) §1 | DT facts only, through that note |

Limits, stated plainly:

- The NXP 6.6.52 tree stands in for the board kernel (Toradex
  6.6.157-7.8.0). That the display drivers are the same is **inferred**.
- The Toradex U-Boot, `meta-toradex-bsp-common` and `meta-toradex-nxp` were
  **not read**: GitHub copies returned 404 and the cgit pages of
  `git.toradex.com` answered with a bot challenge. (The kernel tree on that
  host *was* read with `git`, [imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md)
  §3.) Where §4 says how Toradex applies overlays, the source is community text
  only.
- No Toradex imx-sm fork or Verdin SM configuration (`verdin-imx95`) was read;
  see [imx95-verdin-sm-config.md](imx95-verdin-sm-config.md) §1 and
  [imx95-sm-display-lm.md](imx95-sm-display-lm.md) §1. Statements about the
  Verdin SM use NXP's `mx95evk` as a stand-in (**inferred**).
- Line numbers of LX files were counted from the fetched text; they may be off
  by one or two.

## 2. The rules, and the constraint behind every mechanism

Rules (read from source (other doc), [imx95-board.md](imx95-board.md) "Board
rules"):

- R1 write only under `/tmp`; do not modify the root filesystem;
- R2 install no packages;
- R3 no `fw_setenv`, no `saveenv`;
- R4 never write `mmcblk0boot0` or `mmcblk0boot1`;
- R5 Weston owns the display and holds DRM master; if it is stopped it must
  be restored and `systemctl is-active` must confirm it.

What "the M7 owns the display" needs (inferred from
[imx95-display-power.md](imx95-display-power.md) §4 to §7 and
[imx95-sm-display-lm.md](imx95-sm-display-lm.md) §2):

1. **A grant.** In NXP's reference SM config, `DC*`, `LVDS`,
   `BLK_CTRL_DISPLAYMIX`, the display clocks and the `PD_DISPLAY` permission
   belong to the A55 (AP-NS) logical machine, and the M7 logical machine lists
   none of them (read from source (other doc)). The grant is enforced by the SM
   and TRDC, so no Linux action can create it (inferred).
2. **Linux letting go.** Otherwise two masters program one DPU and its clocks.
3. **A power vote the M7 can rely on.** If Linux votes `PD_DISPLAY` off, the
   mix loses power; the M7 cannot switch the domain under stock permissions
   (read from source (other doc), display-power §4, §7).

So mechanisms (b), (c) and the command line can only do item 2. Only (a) can do
item 1. (d) avoids all three by not programming the DPU at all.

## 3. (a) SM logical-machine configuration change

**How it is built (read from source).**

- imx-sm builds with `make config=<name> all`, and the result
  `build/<name>/m33_image.bin` is copied to the `iMX95` directory of imx-mkimage
  (SM `README.md` lines 80-84). `README.md` lines 90-91: "Customers will usually
  create their own config that partitions resources between the AP and M7".
  The README does not mention `make cfg`; the `make cfg` step and the Yocto
  recipe are in [imx95-sm-display-lm.md](imx95-sm-display-lm.md) §1 (read from
  source (other doc)).
- imx-mkimage `iMX95/soc.mak` takes the SM only as the prebuilt binary
  `MCU_IMG ?= m33_image.bin` (line 59). It does not reference any `.cfg`
  (MK). The configuration is therefore baked into the SM binary (read from
  source).
- `soc.mak` assembles `flash.bin`: `flash_a55` (SM plus A55 container) and
  `flash_all`, which also packs `-m7 $(M7_IMG) 0 $(M7_TCM_ADDR) $(M7_TCM_ADDR_ALIAS)`
  (`M7_IMG ?= m7_image.bin`, line 60). The SM is run from this container at
  reset (read from source).

**How it is deployed (inferred).** `flash.bin` is written to the boot device.
On a Toradex eMMC module that is the boot area. Where the Toradex image keeps
it was not read; that it is in `mmcblk0boot0`/`boot1` is the usual layout and
the feasibility note (§4, item 2) assumes it. Nothing in the sources lets a
running Linux load a different SM: `soc.mak` has no runtime path, and the SM
runs before Linux.

| Question | Answer | Tag |
|---|---|---|
| Could the M7 then program the DPU? | Yes. This is the only mechanism that moves `DC*`, `LVDS`, `BLK_CTRL_DISPLAYMIX`, the clocks and a `PD_DISPLAY` permission to the M7 logical machine ([imx95-sm-display-lm.md](imx95-sm-display-lm.md) §3 gives the cfg design). Linux's display nodes must also be disabled or it faults on the first access | inferred |
| Survives reboot? | Yes, the container is read at every boot | read from source (SM runs from the container; `soc.mak`) |
| Reversible by a session? | Only with a stored stock container and a recovery path that does not depend on the eMMC boot area. None is recorded for the shared board | inferred |
| Allowed by the board rules? | **No.** Writing the container needs the eMMC boot partitions (R4) or a change of boot device and U-Boot environment (R3). It is not a write under `/tmp` (R1) | inferred (deployment target); rules read from source (other doc) |

Verdict: **forbidden on the shared board; needs the board owner**. A route
that avoids writing the eMMC (a one-off RAM boot of a container through the
SoC's serial-download or an SD card selected by the boot switch) would need
physical access and tooling that none of the sources describe; it is a
question for the owner, not something this note can assert (inferred).

## 4. (b) Device-tree overlay disabling the dc/ldb/panel nodes

**What Toradex does (read from source (secondary)).** Community answers say
that on Yocto BSPs overlays are enabled by editing `overlays.txt` on the boot
partition, with `.dtbo` files in an `overlays/` directory next to it (the
partition is mounted under `/run/media/`), and that U-Boot's `fdt_overlays`
variable is not the standard route for Toradex modules. A Verdin iMX95 thread
shows `fdt_overlays=...` in use there, so the exact mechanism on the board's
image is **not settled**. The board's live DT already contains a
`panel-lvds-native` node with a `panel-lvds` and a backlight (recorded
as seen on 2026-11-27 by another item,
[imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md) §3), and
Toradex's overlay source is the repository `device-tree-overlays`
([imx95-verdin-lvds-panel.md](imx95-verdin-lvds-panel.md) §1). So the panel
reaches Linux as an overlay applied by the boot loader (inferred).

**Files written by that route (inferred from the above).** `overlays.txt` and
a `.dtbo` on the first partition of the eMMC user area (not `mmcblk0boot*`).
The base DT file name could also be set through the U-Boot variable `fdtfile`
(community post), which is a U-Boot environment write.

**What the overlay would say.** The kernel's own nodes are `status = "disabled"`
in the SoC dtsi ([imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md) §3.2) and
the panel overlay turns on `dpu`, `display_pixel_link`, `ldb`, `ldb0_phy`,
`pixel_interleaver` and the panel (read from source (other doc):
[imx95-verdin-lvds-panel.md](imx95-verdin-lvds-panel.md) §2). On the board the
corresponding nodes are `display-controller@4b400000`, `ldb@4` with
`channel@0`, `phy@8`, the pixel-link `bridge@8` and `panel-lvds-native`
(observed 2026-11-27, S1 and S2; the node names differ from the labels). A disabling
overlay would set `status = "disabled"` on `&dpu`, `&displaymix_irqsteer`,
`&pixel_interleaver`, `&display_pixel_link`, `&ldb`, `&ldb0_phy`, the panel and
the backlight, or simply leave out the panel overlay (inferred; the feasibility
note §4 lists the same nodes).

**Runtime form.** The NXP 6.6.52 tree has `CONFIG_OF_OVERLAY` (`drivers/of/Kconfig`
lines 90-99, no `depends on` other than `OF`) but **no `OF_CONFIGFS` entry** in
that Kconfig, so it has no configfs interface to load an overlay at runtime
(read from source). A Toradex patch could add one; the board's kernel config
was not read (inferred absent). If an overlay were applied at runtime, the
platform notifier handles a node whose status changes to disabled:
`of_platform_notify` case `OF_RECONFIG_CHANGE_REMOVE` calls
`of_platform_device_destroy` (`drivers/of/platform.c` :792-808, :642-664), and
that unregisters the platform device (:655-656), which unbinds the driver
(read from source). So a runtime disabling overlay would behave like (c)
below (inferred).

| Question | Answer | Tag |
|---|---|---|
| Could the M7 then program the DPU? | **No, not by itself.** It frees Linux's side only. The SM still withholds the display and the `PD_DISPLAY` vote (§2). With the Linux display nodes disabled, the DPU is no longer a genpd user, so Linux votes the domain **off** at `late_initcall_sync` unless `pd_ignore_unused` is set ([imx95-display-power.md](imx95-display-power.md) §6), which makes things worse for an M7 that has no vote | inferred, resting on read-from-source rows of display-power |
| Survives reboot? | Boot-partition form: yes. Runtime form: no. Also not available on the NXP tree (no configfs) | read from source (no `OF_CONFIGFS`); inferred (the rest) |
| Allowed by the board rules? | **Boot-partition form: no.** It writes files outside `/tmp` (R1) and may need `fdtfile` (R3). **Runtime form: not available** on the NXP tree; if the board kernel had it, the write would go to configfs rather than the filesystem, which is the same reading of R1 that the remoteproc `firmware` and `state` files need, and it still stops Weston's output, so R5 applies | inferred |

Verdict: **forbidden (persistent form); not available (runtime form)**. Needs
the board owner, and is useful only together with (a).

## 5. (c) Runtime unbind of the display drivers through sysfs

**What the 6.6.52 drivers do (read from source, line numbers from
`dpu95-drv.c` and `imx95-ldb.c` at LX).**

DPU, platform driver `nxp,imx95-dpu` (`dpu95-drv.c` :220-236):

- `.remove = dpu95_remove` (:108-119) runs `drm_dev_unregister`,
  `drm_atomic_helper_shutdown`, `dpu95_unload` (blit engine unload and
  `drm_kms_helper_poll_fini`, :62-66, and `dpu95-kms.c`), then
  `pm_runtime_disable`. There is **no `.shutdown`** and no component framework
  code in the file.
- `probe` calls `pm_runtime_enable` (:87) and `drm_fbdev_dma_setup` (:97);
  it never takes a runtime-PM reference of its own.
- `dpu95_runtime_suspend` (:121-139) turns off the blit engine and the ocram,
  apb and axi clocks. `dpu95_runtime_resume` (:141-188) re-enables them and
  re-runs QoS, interrupt and submodule hardware init (:169-179).
- `drm_atomic_helper_shutdown` disables every active CRTC, which runs the
  bridge `atomic_disable` below (that the helper does so is DRM core
  behaviour, **inferred**; the core was not read).

LDB, platform driver `fsl,imx95-ldb` (`imx95-ldb.c` :522-537):

- `imx95_ldb_remove` (:489-499) calls `ldb_remove_bridge_helper` and
  `pm_runtime_disable`. No `.shutdown`.
- `atomic_disable` (:185-229) powers the PHY off, gates `ldb_ch`/`ldb_di`
  clocks and calls `pm_runtime_put` (:226); `mode_set` took the reference with
  `pm_runtime_resume_and_get` (:85). `runtime_resume` writes 0 to the LDB
  control register (:511-512); `runtime_suspend` does nothing (:501-504).
- The LDB registers its bridge from `probe` (:484), so the DPU's DRM device
  holds a bridge whose driver can be unbound independently (inferred: unbinding
  `ldb` first leaves the DPU's encoder chain dangling; the safe order is DPU
  first).

**Clocks and power domains on unbind.** From the above, plus
[imx95-display-power.md](imx95-display-power.md) §6 (read from source (other
doc)):

- The SCMI clocks the drivers hold (`ldb_ch`, `ldb_di`, DPU axi/apb/ocram) are
  released through `clk_disable_unprepare` in `atomic_disable` and
  `runtime_suspend`. No `remove` function unprepares a clock directly (read
  from source for these two files).
- Power: `scmi_pm_domain` registers a genpd for `PD_DISPLAY`; genpd powers a
  domain off when every attached device is runtime-suspended
  (display-power §6). `pm_runtime_disable` in `remove` does not itself drop a
  vote (inferred from the code above). Whether the vote drops on unbind depends
  on the device's runtime state at that moment and on the driver core's
  genpd detach, **neither of which was read** (inferred; open). The board's
  `pm_genpd_summary` of 2026-11-27 showed `display` on with only the DPU
  attached ([imx95-display-power.md](imx95-display-power.md) §6, observation
  cited there), so unbinding the DPU leaves nothing else on that domain
  (inferred).
- The mix power state is the SM's, not the driver's: an OFF vote from Linux
  becomes an SM power-down that loses DPU, BLK_CTRL, LVDS CSR and irqsteer
  state (display-power §3). The drivers do not save any of it on unbind.
- `drm_dev_unregister` with Weston still holding DRM master and the fbdev
  console (`fb0`) is the hazard; a client with an open device node survives the
  unregister and may block or fault (inferred; the DRM core was not read).

| Question | Answer | Tag |
|---|---|---|
| Could the M7 then program the DPU? | **Only if the SM already allows it**, which stock permissions do not (§2). After unbind the DPU is idle and its clocks are gated by Linux. An M7 that may not set `PD_DISPLAY` or the clocks cannot re-enable them, and if Linux's genpd vote ends up OFF the mix loses power under it. Under a modified SM (a) it would be a clean handover step | inferred |
| Survives reboot? | No. The driver binds again at the next boot | inferred (nothing in the sources disables autobind) |
| Allowed by the board rules? | **Within the letter of R1 to R4**: sysfs `unbind` writes no file. **R5 is the issue**: Weston must be stopped first (it holds DRM master), and the rebind plus Weston restart must succeed and be confirmed. A failed rebind leaves the shared board without a display until a reboot, and a kernel fault would block every other plan on it | inferred |

Verdict: **allowed by the rules, but risky and insufficient**. It is not a
mechanism the M7 can use under stock SM permissions, and an item that does not
own the display should not attempt it on the shared board. Even for the owner
it should be tried only with the Weston restore path ready.

## 6. (d) Sharing: Linux keeps the DPU, the M7 only reads it

**What is available (read from source).**

- The Toradex kernel DT has the `remoteproc-cm7` node (`fsl,imx95-cm7`),
  mailbox unit `mu7`, reserved regions for vrings, `vdevbuffer`, a resource
  table and a 16 MiB `no-map` M7 region ([imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md)
  §3.1). The board shows the same node and regions, with `remoteproc1`
  `offline` (observed 2026-11-27, S1, S2).
- `imx_rproc` starts the core through the SiP call to TF-A, which uses the SM's
  CPU protocol (feasibility §2, S4/S6; NXP 6.6.52 source, see
  [imx95-sm-linux-interfaces.md](imx95-sm-linux-interfaces.md) §2). The board
  binds no SCMI LMM or CPU driver (observed 2026-11-27, S2); the Toradex
  `imx_rproc.c` is unread. Linux can set the firmware name and
  state through sysfs (feasibility §2). Whether TF-A's request is permitted
  depends on the SM's CPU permissions (inferred; under `mx95evk` it is
  probably refused, feasibility §2).
- The SM TRDC settings decide whether the M7 can even **read** DPU registers.
  Under `mx95evk` the M7 has no DPU entry, so a register read is expected to be
  refused with a bus fault (feasibility §1, inferred there).

**Two sub-forms (inferred).**

- *d1, M7 reads display registers.* Needs a TRDC/resource entry for the M7: the
  same SM change as (a), only with read-only rights (`READONLY` exists in the
  cfg syntax, feasibility §1). Not available without (a).
- *d2, the M7 renders into shared memory and Linux scans out.* Needs only an
  M7 image started through remoteproc from `/tmp` (if the SM permits the start),
  a reserved region both sides reach, and a Linux consumer. The region's
  suitability as a frame buffer is open ([imx95-m7-fb-memory.md](imx95-m7-fb-memory.md)).
  Weston is untouched if the consumer is a Wayland client.

| Question | d1 | d2 |
|---|---|---|
| M7 programs the DPU? | **No.** It may at most read registers | **No.** The DPU stays Linux's |
| Survives reboot? | Needs (a): yes | No state outside `/tmp` and the remoteproc session |
| Allowed by the rules? | Needs (a): no | **Yes**, writes only under `/tmp`; the sysfs writes for remoteproc follow the same reading of R1 as above and must be stated by the item that does them; Weston stays running (R5) |

Tags: all inferred, except the DT and remoteproc facts above.

Verdict: **d2 is allowed, d1 is not available without (a)**. Neither proves
framebuffer scanout or VSYNC/latch reporting **from Zephyr** (SoW §5.1, WP3),
and neither may be reported as an M2 pass. What d2 can show is M7 rendering and
a shared-memory path; any M7 timing would be that of its own loop, not of the
display ([imx95-m7-reduced-scope.md](imx95-m7-reduced-scope.md) holds the
reduced-scope bring-up spec).

## 7. Summary

| | Mechanism | M7 can program the DPU | Survives reboot | Board rules | Verdict |
|---|---|---|---|---|---|
| a | SM cfg change + new container | yes (the only one that grants it) | yes | breaks R4 (or R3), R1 | forbidden; owner only |
| b | DT overlay, boot partition | no (frees Linux only; may power the mix off) | yes | breaks R1 (and R3 if `fdtfile`) | forbidden |
| b' | DT overlay, runtime | no | no | no configfs in the NXP tree; R5 | not available |
| c | sysfs unbind of DPU and LDB | no under stock SM; yes only after (a) | no | letter of R1-R4 kept; R5 at risk | allowed but risky; not for non-owners |
| d | sharing (d2: M7 renders, Linux scans out) | no | no | fits R1-R5, Weston untouched | allowed; not an M2 scanout proof |

All verdict cells are **inferred** from the read-from-source rows above.

**What a reduced-scope M2 run can use without breaking the rules:** (d2) only,
with Weston running. It yields M7 load, execution and a shared buffer, and it
must be reported as such (inferred). A (c) test is within the letter of the
rules but should be left to whoever owns the display for that session, with
Weston stopped and restored and confirmed.

**What needs the board owner's agreement:** (a) in every form, since it
cannot be done under R1/R3/R4 and the grant lives there; (b) in its persistent
form, as a companion of (a); and any variant of (c) that is meant to let the M7
take over, since it needs (a) first. Questions for the owner: is there a
dedicated board whose boot container may be replaced, and a recovery path that
does not rely on the eMMC boot area?

## 8. Open points

- Toradex's U-Boot, `meta-toradex-bsp-common` and BSP 7.8.0 recipes: how the
  image applies overlays and where the container lives. Not read.
- The Verdin SM configuration `verdin-imx95` (see
  [imx95-verdin-sm-config.md](imx95-verdin-sm-config.md) and
  [imx95-sm-display-lm.md](imx95-sm-display-lm.md)). Not read.
- Driver-core and genpd detach behaviour on unbind, and DRM core behaviour with
  an open master: not read.
- Whether the board kernel has an overlay configfs or a `/dev/mem`-like path
  to the shared region: the kernel config was not read.
- Line numbers of the LX citations should be re-checked with `grep -n` on a
  checkout at the pin.
