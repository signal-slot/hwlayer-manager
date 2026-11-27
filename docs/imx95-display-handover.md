# i.MX95: ways to hand the display from Linux to the M7, against the board rules

Date of this note: 2026-11-28. Scope: SoW §5.1 / §9 M2 (WP3). This is a
desk design note. **Nothing here was observed, tried, built or run, and no
board was accessed.** Every finding is tagged **read from source** (taken from
the repository's other docs, which cite their own pins) or **inferred** (my
reasoning from them). No board addresses, host names or credentials appear.

Inputs, none changed by this item: `docs/imx95-board.md` (the board rules),
`docs/imx95-sm-display-lm.md`, `docs/imx95-m7-fb-memory.md`,
`docs/imx95-m7-verdin-bsp.md`, `docs/imx95-m7-overlay-design.md`,
`docs/imx95-dpu-source-6.6.md`, `docs/imx95-m7-scmi.md`,
`docs/weston-imx-lease.md`.

## 1. The rules and the starting state

Rules (read from source, `docs/imx95-board.md`, "Board rules"):

- R1 write only under `/tmp`; do not modify the root filesystem;
- R2 install no packages (nothing into `/usr`, `/etc`);
- R3 no `fw_setenv`, no `saveenv` (the U-Boot environment stays as it is);
- R4 never write `mmcblk0boot0` or `mmcblk0boot1`;
- R5 Weston owns the display and holds DRM master; if stopped it must be
  restored and `systemctl is-active` must confirm it. The item brief adds that
  Weston must stay or be restored.

Starting state (read from source, `docs/imx95-dpu-source-6.6.md`,
`docs/imx95-sm-display-lm.md`, `docs/imx95-m7-fb-memory.md`):

- In NXP's reference `mx95evk` SM configuration the AP-NS agent (Linux) owns
  the DPU (`DC*`), `LVDS`, `VIDEO_PLL1`, `BLK_CTRL_DISPLAYMIX` and the display
  pixel clocks. The M7 logical machine has no display entry. That the Verdin's
  shipped SM configuration matches is **inferred**; the Toradex configuration
  was not read there.
- The kernel tree has no display node for Verdin/Mallow; the panel comes from
  an overlay applied outside the tree, not read (read from source: the
  absence; that the boot loader applies it is inferred).
- The board's Linux DT has a `remoteproc-cm7` node (`fsl,imx95-cm7`) and
  reserved regions for the M7; no M7 image was installed (read from source:
  the earlier snapshot cited by `docs/imx95-m7-fb-memory.md`; not re-observed
  here).

## 2. The constraint behind every option (inferred)

Two things must hold for the M7 to scan out:

1. **Linux must stop using the DPU path**, or two masters program the same
   registers and clocks.
2. **The SM must let the M7 logical machine reach the DPU, LVDS, their clocks
   and power domain.** In the reference configuration these belong to AP-NS
   with `OWNER` rights (read from source, `docs/imx95-sm-display-lm.md` §2).
   SCMI clock/power requests and TRDC register access are enforced by the SM
   and TRDC, not by Linux. So (inferred) *no Linux-side action can give the M7
   the right to touch these blocks*; Linux can only release its own use. The
   grant lives in the SM configuration, in the boot container.

Options that change only Linux (overlay, command line, unbind) can at best
*free* the display. None *grants* it. Zephyr's SCMI client can request
clocks and power domains only for resources its LM may use (read from source,
`docs/imx95-m7-scmi.md`; the denial behaviour is inferred).

## 3. The mechanisms

"Reboot" = does the change survive a reboot. "Reversible" = can a session
undo it itself.

| # | Mechanism | Requires | Reboot | Reversible | Rules | Verdict |
|---|---|---|---|---|---|---|
| A | Different SM logical-machine configuration | New imx-sm build, new boot container, written to the boot medium | Yes | Only by rewriting the container; a bad image can leave the board unbootable | Breaks R4 (or R3 via a boot-flow change) and R1 | **Forbidden** |
| B | DT overlay disabling the Linux display nodes | Boot-loader overlay (file or U-Boot variable), or a runtime configfs overlay | Boot-loader form: yes. Runtime form: no | Boot-loader form: by editing back. Runtime: by removal | Boot-loader form breaks R1/R3 | **Forbidden** (persistent); **not available** (runtime) |
| C | Unbind the DRM driver at runtime | sysfs `unbind` writes, after stopping Weston | No | In principle (rebind) | No file written; R5 at its limit | **Risky to Weston**; insufficient alone |
| D | Kernel command-line change | Edit of the U-Boot environment or boot configuration | Yes | Only by writing it back | Breaks R3 (and R1) | **Forbidden** |
| E | Sharing: Linux keeps the DPU, the M7 renders into a buffer Linux scans out | M7 image loaded via remoteproc from `/tmp`; a shared memory region; a Linux consumer under `/tmp` | No | Yes (stop the core, remove files) | Fits R1 to R5 if Weston is untouched | **Allowed**; does not answer the M2 scanout question |

### A. Different SM logical-machine configuration

- Requires (read from source, `docs/imx95-sm-display-lm.md` §3, §5, §6): move
  every `DC*`, `LVDS`, `VIDEO_PLL1`, `BLK_CTRL_DISPLAYMIX` line and the display
  clock entries from AP-NS to LM1 (the M7); build with `make cfg` and
  `make config=<name> all`; assemble a container with imx-mkimage; program it
  to the boot device. The Linux display nodes must also be disabled or the
  first register access faults (inferred).
- Reboot: yes, the SM runs from the boot container (read from source, same
  note).
- Reversible: only with a stored stock container and a recovery path
  independent of eMMC boot. None exists on the shared board (read from
  source, same note §6).
- Rules: the container goes to the eMMC boot area or needs a boot-flow
  change: R4, or R3, plus R1. **Forbidden.**
- It is the only mechanism that actually grants the M7 the display (inferred),
  so it is a dedicated-board path.

### B. Device-tree overlay disabling the Linux display nodes

- Requires (inferred): DPU, pixel link, LDB, panel and backlight nodes set to
  `status = "disabled"`. Two forms:
  - *Boot-loader overlay.* The panel configuration already comes from outside
    the kernel tree (read from source, `docs/imx95-dpu-source-6.6.md`).
    Changing it means editing a file on the boot partition or a U-Boot
    variable (inferred). Survives a reboot, reversible by editing back, breaks
    R1/R3. **Forbidden.**
  - *Runtime overlay* via the kernel's overlay configfs. Nothing in the
    repository shows this kernel has it. Treated as **not available**
    (inferred). Even if present, a node already probed keeps its driver when
    later marked disabled (inferred), so it reduces to C.
- Either form frees Linux only; the SM still withholds the display from the
  M7 (§2).
- Weston: with the display nodes gone it has no output, and restoring it
  means undoing the overlay (inferred), so a persistent form also collides
  with R5.

### C. Unbind the DRM driver at runtime (risky to the shared board's Weston)

- Requires (inferred): stop Weston (R5 permits it if restored), then write
  the DPU platform device name to the driver's sysfs `unbind` file. The device
  names are not recorded in the repository for this board.
- Reboot: no, the driver binds again at boot (inferred).
- Reversible: in principle by `bind` plus a Weston restart. **Risky**
  (inferred):
  - unbinding a DRM driver with live framebuffers and an active CRTC can hang
    or oops if anything still holds the device; a kernel fault takes the whole
    shared board down and blocks every other plan;
  - the DPU, pixel link and LDB are separate drivers with probe-order links;
    a partial rebind can leave the bridge or backlight in a state Weston
    cannot start on, with only a reboot to repair it;
  - runtime PM may power the display domain down, so a rebind starts cold.
- Rules: no file written, so R1 to R4 are untouched. R5 is the problem: Weston
  must be stopped first, and a failed rebind leaves it unrestored.
- Helps the M7? No (§2): the SM still denies it the DPU. Inferred.
- Verdict: **risky to Weston**, not recommended on the shared board, and not
  for any item that does not own the display. Not tried here.

### D. Kernel command-line change

- Requires (inferred): for example `video=<connector>:d` or a driver
  blacklist. The command line comes from the U-Boot environment or the boot
  configuration, which R3 and R1 forbid writing. Reading `/proc/cmdline` is
  fine.
- Reboot: yes. Reversible: only by writing the value back.
- Rules: breaks R3 and R1. **Forbidden.**
- Like B and C it frees Linux only (§2), and removing Weston's output collides
  with R5.

### E. Sharing: the M7 renders, Linux scans out

- Requires (inferred):
  - the M7 firmware started through `remoteproc` from a file under `/tmp`.
    State and search-path changes are sysfs/module-parameter writes, not
    filesystem writes; this is my reading of R1 and the item doing it must
    state it;
  - a buffer both sides reach. The DT already reserves M7 regions (a 16 MiB
    `no-map` region, vring and `vdevbuffer` regions, a resource-table region;
    read from source, `docs/imx95-m7-fb-memory.md`). Which phandle names which
    region was not resolved, so whether one can serve as a frame buffer is
    **inferred and open**. The M7 side must keep it out of the cache (rule
    read from source in the same note);
  - a Linux consumer: a Weston client needs no DRM master, so Weston is
    untouched. A DRM client would need master, so the Weston stop of C. Only
    the first fits the rules. How a `/tmp` program reads the region
    (`/dev/mem`, UIO) is unknown on this kernel (inferred, open).
- Reboot: no. Reversible: yes.
- Rules: fits R1 to R5 with Weston running. **Allowed.**
- What it proves (inferred): rproc loading, M7 rendering and a shared buffer,
  which supports a reduced-scope M2 bring-up. It does **not** prove
  "framebuffer scanout and VSYNC/latch reporting from Zephyr" (SoW §5.1, WP3):
  the DPU stays Linux's and the M7 never programs a latch. It must not be
  reported as an M2 pass, and any M7 timing is of its own render loop, not of
  the display.
- The vblank would be Linux's DRM event; the M7 would learn of it only through
  an extra signal such as rpmsg or a shared counter (inferred).
  `docs/imx95-m7-overlay-design.md` describes the other ownership models.

## 4. Result

Allowed on the shared board against R1 to R5 (inferred):

- **E, sharing**, with Weston untouched.
- **C, runtime unbind**, within the letter of the rules but **risky to the
  shared board's Weston** and useless to the M7 without A. Treat as not
  allowed for items that do not own the display.

Forbidden on the shared board: **A** (R4, R3, R1), **B** persistent form
(R1/R3), **D** (R3, R1).

Consequence for M2 (inferred): true M7 scanout on the DPU needs A together
with B or D, which means a dedicated, recoverable board or a board-owner
action outside the rules. On the shared board only E can run. The blocker
report is therefore: the grant of the display to the M7 is an SM
boot-container change, and the board rules forbid it; E is the reduced-scope
bring-up. This rests on the sources above; reading the Toradex SM
configuration would turn "matches the NXP reference" from inferred into read
from source.

## 5. Open points

- Read the Toradex SM configuration (owned by the Verdin BSP read item).
- Resolve which reserved region each `memory-region` phandle of the rproc node
  names, and whether any can be a shared frame buffer.
- Find out whether the running kernel offers overlay configfs or `/dev/mem`
  reads; both unknown here.
- Ask the board owner whether a dedicated, recoverable board exists for A.
