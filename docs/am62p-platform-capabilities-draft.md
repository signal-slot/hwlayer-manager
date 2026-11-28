# TI AM62P section for platform-capabilities (staging draft)

**This is a staging draft, not part of [platform-capabilities.md](platform-capabilities.md).** Plan 462 is changing that file, so nothing here was pasted into it. The follow-up is a roadmap item: merge the section below into `docs/platform-capabilities.md` (replacing its "TI AM62P: Not researched yet" paragraph) once plan 462 has landed, then delete this file. Written 2026-11-28 from the merged AM62P studies only; no device was used.

**Nothing in this draft is observed.** No AM62P board run has been recorded (see [sow-traceability.md](sow-traceability.md)). Every row that a board run would turn into a measurement says `not measured (pending docs/m3-am62p-report.md)`; that report does not exist yet.

Tags follow `platform-capabilities.md`, with one added:

- **source**: read from driver, device-tree, binding or vendor documentation. Not run. The commit or document is the one the cited study pins.
- **inferred**: a conclusion drawn from the source, not stated by it.
- **not measured (pending docs/m3-am62p-report.md)**: a value or behaviour the board run is meant to show. The cell says what the sources predict, tagged as above.
- **observed** and **vkms** are not used: nothing was run.

Source documents (all in this repository): `DSS` = [am62p-dss-source.md](am62p-dss-source.md), `GO` = [am62p-dss-gobit-source.md](am62p-dss-gobit-source.md), `SHARE` = [am62p-dss-sharing-source.md](am62p-dss-sharing-source.md), `DT` = [am62p-dt-source.md](am62p-dt-source.md), `RPROC` = [am62p-r5f-remoteproc-source.md](am62p-r5f-remoteproc-source.md), `ZEPH` = [am62p-zephyr-upstream-source.md](am62p-zephyr-upstream-source.md), `TORIZON` = [am62p-torizon-boot-source.md](am62p-torizon-boot-source.md), `MATRIX` = [am62p-imx95-matrix.md](am62p-imx95-matrix.md), `TP` = [twoplane-backend.md](twoplane-backend.md), `PLAN` = [m3-am62p-test-plan.md](m3-am62p-test-plan.md).

Caveats carried from the studies, so a reader does not over-trust a row:

- The kernel rows read the Toradex tree `toradex_ti-linux-6.6.y` at `9764efd3fe8e` (Linux 6.6.163) and mainline `v6.12` (`DSS`). That the board runs this kernel is **inferred**; `uname -r` on the board must be compared with `6.6.163`.
- `SHARE` and `RPROC` say they could not read the Toradex tree (they read TI's `ti-linux-6.12.y` and `ti-linux-6.6.y` at `a7758da1`); `DSS` and `DT` did read it. The driver rows below from `SHARE` and `RPROC` therefore describe TI's trees. The two sets agree on the shared-mode properties, but the agreement was not cross-checked line by line.
- The AM62P TRM was not reached by `GO`; its register statements rest on the TI AM67 TRM (a sibling device) and on Linux. Rows from `GO` say so.
- `MATRIX` still lists many AM62P cells as "not recorded"; those cells are being filled by another item. This draft does not edit it.

## TI AM62P, Linux (Toradex BSP kernel), read from source

Tree: `git.toradex.com/linux-toradex.git`, branch `toradex_ti-linux-6.6.y`, commit `9764efd3fe8e`, `drivers/gpu/drm/tidss/` (`tidss`), compared with mainline `v6.12` (`adc218676eef`), which has no AM62P support in tidss. Board BSP, kernel and panel to be taken from the running board.

| Item | Finding | Source doc | Basis |
| --- | --- | --- | --- |
| Driver, compatibles | `tidss`; `ti,am62p51-dss` (DSS0) and `ti,am62p52-dss` (DSS1). Mainline `v6.12` has neither | `DSS` | source |
| DSS instances | 2 in the SoC dtsi (`dss0` at `0x30200000`, `dss1` at `0x30220000`), both `disabled` there | `DSS`, `DT` | source |
| Video ports | 2 per instance (`vp1`, `vp2`). DSS0: vp1 feeds the OLDI TX block, vp2 is parallel DPI. DSS1: both internal (DSI path). Max pixel clock 165 MHz (DPI), 300 MHz (internal) | `DSS` | source; DSS1 port wiring inferred |
| Planes | 2 per instance: `vid` (full VID) and `vidl1` (VIDL, lite). With one connected output the first-created plane is the primary and it is the lite plane (`vid_order = {1, 0}`); the full VID plane is the overlay. With two connected outputs both are primaries and no overlay exists | `DSS` | source (order and counts); roles inferred from DRM core |
| Plane count seen on the board | The count, types and `possible_crtcs` the running kernel exposes | `DSS` (to confirm), `PLAN` | not measured (pending docs/m3-am62p-report.md); expected 2 per active instance |
| CRTCs | One per video port with a panel or bridge on its OF graph port, so 0 to 2 per instance depending on the device tree | `DSS` | source |
| Plane to CRTC | Every plane has `possible_crtcs` for all CRTCs, primaries included | `DSS` | source |
| Formats | One list on every plane, 28: ARGB/ABGR/RGBA/BGRA/XRGB/XBGR/RGBX/BGRX 8888, 4444 and 1555 variants, 2101010 variants, RGB565, BGR565, RGB888, BGR888, YUYV, UYVY, NV12. Not reduced for the lite plane. Whether the VIDL hardware renders YUV is not stated | `DSS` | source; lite-plane YUV inferred |
| Modifiers | None advertised (modifiers argument NULL), so linear only | `DSS` | source; `IN_FORMATS` content inferred |
| Scaling, which plane | Only the full `vid` plane scales. A lite plane with source size different from destination size gets `-EINVAL` | `DSS` | source |
| Scaling limits (`vid`) | Up 16x per axis; down 4x (5-tap) or 2x (3-tap) with horizontal decimation up to `xinc_max` 32, so refused beyond 128x (5-tap). 5-tap input width up to 1280 (RGB) or 2560 (YUV), 3-tap up to 2560 or 4096. YUV input counts as scaled | `DSS` | source |
| Scaling, measured | Whether `TEST_ONLY` accepts and the panel shows a scaled overlay | `DSS` ("to confirm") | not measured (pending docs/m3-am62p-report.md) |
| DRM-level plane check | No DRM scale limit (min 0, max `INT_MAX`); partly off-screen planes are clipped; source x, y and width must be multiples of the chroma subsampling. No pitch or address alignment check in the files read | `DSS` | source; clipping meaning inferred |
| Global alpha | `alpha` property, written as 8 bits (`alpha >> 8`) | `DSS` | source |
| Blend modes | Pre-multiplied and Coverage only; **`None` is not offered**. Pre-multiplied sets `DISPC_VID_ATTRIBUTES` bit 28, every other mode clears it | `DSS` | source; defaults, and XRGB8888 under Coverage, inferred |
| Blend modes, measured | Which blend the panel shows for the safety plane (ARGB8888) | `DSS`, `TP` | not measured (pending docs/m3-am62p-report.md) |
| zpos | Mutable, range 0..1 (two planes); initial value is the creation index, so the lite plane starts at 0. `normalize_zpos` is on; the CRTC walks overlay layers in normalized zpos order | `DSS` | source; initial-value rule inferred |
| Plane position | Written to the overlay manager (`DISPC_OVR_ATTRIBUTES(layer)`: x bits 17:6, y bits 30:19), not to the plane | `DSS` | source |
| Rotation | No rotation property in tidss | `DSS` | source (absence by grep) |
| Colour properties | `COLOR_ENCODING` BT.601 (default) and BT.709, `COLOR_RANGE` full (default) and limited, on every plane; a YUV format needs a matching CSC entry | `DSS` | source |
| Primary plane required | tidss has no rule; a CRTC with no visible primary is not refused by the driver. DRM core and userspace conventions not read | `DSS` | source; overlay-only commit inferred, to confirm by `TEST_ONLY` |
| Mode limits | `hdisplay`/`vdisplay` at most 4096, interlace refused, odd `hdisplay` refused, pixel clock by bus type; `mode_config` max 8096x8096 | `DSS` | source |
| Bandwidth check | Only if the DT node has `max-memory-bandwidth` (4 bytes per pixel assumed). Not seen in the Verdin files; no DSS node enabled there | `DSS` | source |
| Gamma, CTM, background | Per-VP 256-entry gamma, CTM supported; default VP background black | `DSS` | source |
| Clocks, power | DSS0: `fck`, `vp1`, `vp2`; power domains 186, 243, 244 (`k3_pds`); DSS1: 232, 244. Linux PM, not SCMI | `DSS`, `DT` | source |
| Underflow, sync lost | Plane FIFO underflow is logged (TDX only); VP sync lost is a ratelimited error | `DSS` | source |
| Sync lost on layer disable | Silicon advisory i2097: disabling a layer connected to the overlay may cause a sync lost, dependent on GO-bit timing; the next frame is corrupt or blank, the design recovers from the next GO. Workaround: move the layer off-screen before disabling | `GO` | source (AM62P errata SPRZ574B); relevance to the backend inferred |
| Sync lost on layer disable, on the board | Whether scenes that remove the application plane raise `SYNC_LOST` | `GO`, `PLAN` | not measured (pending docs/m3-am62p-report.md) |

## TI AM62P, GO bit and latch, read from source

The register facts come from the AM67 TRM (a sibling, via search snippets) and Linux; the AM62P TRM was not read (`GO`).

| Item | Finding | Source doc | Basis |
| --- | --- | --- | --- |
| GO bit | Bit 5 of `DISPC_VP_CONTROL`, one per video port. Software writes 1 to validate the shadow configuration; hardware clears it when the shadow-to-work copy has happened | `GO`, `DSS` | source (AM67 TRM text for the clear; Linux for bit and position) |
| What GO latches | The video-port and overlay registers and the plane (video) registers routed to that port. Plane enable is a plain shadowed register. No per-plane commit bit | `GO` | source; the full shadowed-register list for the AM62P inferred |
| When the copy happens | At the vertical front porch, before the vblank interrupt, not at the VSYNC pulse | `GO`, `DSS` | source (Linux driver comment) |
| How Linux decides "latched" | The vblank handler returns without sending the page-flip event while GO still reads 1; the event goes out at the first vblank after GO cleared. The driver keeps no count | `GO`, `DSS` | source |
| Commit while GO is busy | `tidss_crtc_atomic_flush` warns and returns when GO still reads 1: the update is dropped by that path | `GO`, `DSS` | source; "a second commit in one frame can be lost" inferred |
| GO written while already 1 | No source read says what the hardware does. Likely a no-op that merges both commits into one copy | `GO` | inferred; no source answers it |
| Plane writes while GO is pending | They land in the shadow set; if the copy comes mid-update the configuration can be torn across two frames. tidss avoids it by never writing planes while GO reads 1 | `GO` | inferred |
| VP interrupts | tidss maps raw VP bits: frame done 0, vsync even 1, vsync odd 2, sync lost 4. The AM67 TRM ties the GO clear to the VSYNC interrupt entry | `GO` | source for tidss; AM62P bit numbering inferred |
| Backend mapping | `commit_count` = one GO write per commit, `latched_count` = one increment per observed GO 1 to 0, seen by reading GO or in the VSYNC handler | `GO`, `TP` | inferred |
| Latched count against commits | After each scene settles, `commit_count` equals `latched_count`; a difference points to a missed copy or a merged GO | `GO`, `PLAN` | not measured (pending docs/m3-am62p-report.md); expectation inferred |
| GO read after the write, in the VSYNC handler | Expected 1 right after the write and 0 in the handler for a lone commit | `GO` | not measured (pending docs/m3-am62p-report.md) |
| VSYNC interrupts between a commit and its latch | Expected at most one if GO is set before the VFP, one more after | `GO` | not measured (pending docs/m3-am62p-report.md); expectation inferred |
| Two commits under one GO | Whether the latch counter credits both | `GO` | not measured (pending docs/m3-am62p-report.md); needs a scene made for it |
| FRAMEDONE against VSYNC | Which one the latch follows on the AM62P | `GO` | not measured (pending docs/m3-am62p-report.md) |

## TI AM62P, sharing the DSS between Linux and a remote core, read from source

Read in TI's `ti-linux-6.12.y`, SDK 9.1 binding and the SDK 12.01 pages (`SHARE`), and in the Toradex tree (`DSS`, `DT`).

| Item | Finding | Source doc | Basis |
| --- | --- | --- | --- |
| Shared mode | Device-tree switch only (no Kconfig or module parameter): `ti,dss-shared-mode` with `-planes`, `-vp`, `-vp-owned`, `-common`, `-plane-zorder`. The driver prunes its feature table to what Linux owns and uses the shared-VP helpers for a VP Linux does not own (no VP, overlay or GO access) | `DSS`, `SHARE` | source |
| Where it exists | Toradex 6.6 tree (`DSS`) and TI 6.12 (`SHARE`); SDK 9.1 binding defines it. Mainline `tidss_drv.c` has no `shared_mode` | `DSS`, `SHARE` | source; mainline `tidss_dispc.c` not read |
| Spelling | `ti,dss-shared-mode-vp-owned` in code and overlay; the SDK page writes `-owned-vp` | `SHARE` | source |
| Use in the Verdin files | None. No Verdin `.dts`, `.dtsi` or overlay read uses any `ti,dss-shared-mode*` property; Linux owns the whole DSS unless an unread Toradex overlay says otherwise | `DSS`, `DT` | source (absence); overlays partly unread, so conclusion inferred |
| TI's layout | Remote core owns VIDL, VP1 and OVR1 and the global `common` bank; Linux owns VID through `common1`; Linux skips global init, `fck` and runtime PM. Framebuffer in a `no-map` reserved region (SK overlay: 128 MiB at `0x93500000`) | `SHARE`, `DSS` | source |
| TI's remote core | The Device Manager (wakeup) R5F running FreeRTOS, started by the SBL. No TI page read describes the **MCU** R5F as display owner | `SHARE` | source; MCU R5F suitability inferred |
| Access of the MCU R5F | TISCI host IDs, firewall entries and clock/power requests for an MCU R5F driving the DSS were not found in any page read | `SHARE` | not found; to be shown by the board run |
| Take-over route (what the test uses) | Stop whatever holds `/dev/dri/card*`, unbind fbcon and tidss; the R5F then sets up clocks, power domain, VP and OLDI itself. A re-bind alone is not shown to recover; a reboot is the safe restore | `SHARE` | inferred |
| Two writers on one VP | TI protects by partition, not arbitration. Expected failures: clobbered configuration, shared GO latch, runtime-PM gating, interrupt-status races, teardown under a running core | `SHARE` | inferred |
| Crash isolation | TI's demo says the R5 keeps the panel alive when the A53 kernel crashes, in shared mode only. Without shared mode a Linux crash during an R5F run proves nothing | `SHARE` | source (demo page); the caveat inferred |
| Whether the R5F reaches DSS registers and the buffers | Result of the run | `SHARE`, `PLAN` | not measured (pending docs/m3-am62p-report.md) |
| Whether a tidss re-bind recovers without a reboot | Result of the run | `SHARE` | not measured (pending docs/m3-am62p-report.md) |

## TI AM62P, Verdin device tree, read from source

Toradex tree `toradex_ti-linux-6.6.y` `9764efd3fe8e` and overlays repository `c93543ff886b` (`DT`). Which kernel and overlay the board actually runs is not known.

| Item | Finding | Source doc | Basis |
| --- | --- | --- | --- |
| Base display wiring | The Verdin dtsi files enable and wire neither `dss0` nor `dss1`; only DSI-named pins and GPIOs appear | `DT`, `DSS` | source (absence by grep) |
| LVDS panels | Toradex overlays route `dss0` VP1 through OLDI0 and OLDI1 (single-channel 10.1 inch 1280x800 at 68.9 / 71.1 / 73.4 MHz; dual-channel 1080p at 138.5 MHz) | `DT` | source |
| DSI routes | Every DSI overlay uses `dss1` VP2 through `dsi0` and a bridge (`lt8912b` to HDMI, `sn65dsi84` or `sn65dsi83` to LVDS panels, 7 inch 1024x600 and 10 inch 1280x800) | `DT` | source |
| DPI | No overlay read uses the DPI output | `DT` | source (absence) |
| Active overlay on Torizon | Not stated in the repository; inferred to be none until one is selected | `DT` | inferred |
| Active overlay, panel, DSS node status on the board | Taken from the running board | `DT` ("what the run should confirm"), `PLAN` | not measured (pending docs/m3-am62p-report.md) |
| R5F nodes | MCU R5F `r5f@79000000` (`ti,am62-r5f`, TCM 32 KiB + 32 KiB, `sram` 512 KiB at `0x79100000`, firmware name `am62p-mcu-r5f0_0-fw`); wakeup R5F `r5f@78000000`, firmware name `am62-wkup-r5f0_0-fw`. Both are enabled in the Verdin dtsi | `DT` | source |
| Carve-outs | MCU R5F: DMA pool `0x9b800000` (1 MiB) and memory `0x9b900000` (15 MiB). Wakeup R5F: `0x9c800000` and `0x9c900000`. TF-A at `0x9e780000` by arithmetic | `DT` | source; TF-A address inferred |
| Mailbox | The MCU R5F has `mboxes`; the wakeup R5F override has none | `DT` | source |
| Wakeup R5F is the Device Manager | The dts does not say it; the SK overlay's firmware label supports it | `DT` | inferred |
| Carve-out sizes reported by remoteproc | Match 1 MiB and 15 MiB | `DT` | not measured (pending docs/m3-am62p-report.md) |

## TI AM62P, R5F under remoteproc and Torizon, read from source

Driver `ti_k3_r5_remoteproc.c` in TI `ti-linux-6.6.y` (`a7758da1`); the Toradex kernel's driver was not read (`RPROC`). Torizon layers `meta-toradex-ti` and `meta-toradex-torizon` were not read (`TORIZON`).

| Item | Finding | Source doc | Basis |
| --- | --- | --- | --- |
| Modes | AM62P uses `ti,am62-r5fss`, single-core cluster mode by default. Per core: **IPC-only** (core already running, state `detached`; no `start` or `stop` ops) or **remoteproc mode** (core off). Decided once at probe | `RPROC` | source |
| Boot at probe | If a file named like the DT `firmware-name` exists on the firmware path at probe, the core boots without a `start` write | `RPROC` | inferred from the code |
| Who boots which R5F | The wakeup (Device Manager) R5F is booted by the boot chain and cannot be stopped; the MCU R5F's boot is not stated by any source found (default expectation: started from Linux through `state`) | `TORIZON`, `RPROC` | source for the DM R5F (TI AM62x docs, search summary; AM62P by inference); MCU R5F inferred |
| State of each remoteproc node at boot | `state` and `firmware` of each node | `TORIZON`, `RPROC` | not measured (pending docs/m3-am62p-report.md) |
| Stop | `stop` needs state `running`; remoteproc mode sends a shutdown mailbox message and waits 5 s. No ack: logs a timeout and halts anyway. Ack but the core not in WFI within 2 ms: `can't stop rproc: -110`, state stays `running` (a display loop is not stopped). Second `stop` on an offline core: `-EINVAL`, silent | `RPROC` | source |
| Why "a second image needs a reboot" | Candidates: an ack-and-keep-running image (row above), IPC-only state, or a re-probe after unbind (`mismatched mode`). Not decided by source | `RPROC` | inferred; not measured (pending docs/m3-am62p-report.md) |
| Crash handling | `recovery_disabled` is true; a crash message is only logged, the core is not restarted | `RPROC` | source |
| Trace buffer | The resource table needs a section `.resource_table`; a trace entry becomes `trace0` in `<debugfs>/remoteproc/remoteprocN/`, mode 0400, created at boot and removed at stop. The read ends at the **first NUL byte**; the address must lie in TCM, `sram` or a static DDR region (not the vring pool). Needs debugfs and root | `RPROC` | source |
| Trace buffer format | Whether Zephyr's RAM console ends in a NUL after the last character and whether it wraps | `RPROC` | not read; not measured (pending docs/m3-am62p-report.md) |
| Firmware path | `/lib/firmware` by `firmware-name`; AM62P file names by analogy with the AM62 M4F | `TORIZON` | inferred; to be read from the `firmware` attribute |
| Boot chain | Torizon boot chain, Torizon OS files and what "back on the Torizon boot chain" means: see the study | `TORIZON` | source (search summaries) and inferred, as the study tags each row |
| Torizon OS, BSP and kernel versions | `uname -r`, `/etc/os-release`, `/proc/device-tree/model` | `TORIZON`, `DSS` | not measured (pending docs/m3-am62p-report.md) |

## TI AM62P, Zephyr on the MCU R5F, read from source

Zephyr `v4.5.0` (`41d10b597304`), the version this repository pins (`ZEPH`).

| Item | Finding | Source doc | Basis |
| --- | --- | --- | --- |
| SoC and board upstream | No AM62P SoC and no Verdin AM62P board. `verdin_am62` is the AM62 M4F only. Nearest part: J722S with `mcu_r5f0_0` (BeagleY-AI board) | `ZEPH` | source |
| Reuse of J722S | Whether the AM62P MCU R5F's peripheral map equals J722S's (VIM base, timers, mailbox IRQs) is open | `ZEPH` | inferred; needs the TRM |
| Drivers present | R5 architecture, VIM (`ti,vim`), `ns16550` UART, `ti,omap-mailbox`, `ti,am654-timer`, RTI watchdog node, GPIO, pinctrl, OpenAMP resource-table Kconfig | `ZEPH` | source (J722S nodes) |
| Display driver | None for the TI DSS and no TI MIPI-DSI host. Rests on file and binding names, not on a content search | `ZEPH` | source (names); contents inferred |
| Console route | `CONFIG_RAM_CONSOLE=y` makes a trace entry in the resource table, pointing at `ram_console_buf` | `ZEPH` | source |
| What the board test takes from the showcase | The SoC `am62p54`, the board `verdin_am62p`, the DSS driver (`am62p_dss_*` calls) and the DSI/bridge setup. None exists upstream | `ZEPH` | source (absence) |
| Memory | Upstream's carve-outs are BeagleY-AI's; the Verdin plan comes from the BSP device tree. Cache and MPU attributes of the DSS-read buffers are not recorded | `ZEPH`, `DT` | inferred |
| FPU build options | `VFP_SP_D16` in the SoC entry against the test's `CONFIG_FPU` and hard-float setting: fit with the engine library not checked | `ZEPH` | inferred |
| An upstream-only build | Not possible today: needs SoC, board and DSS driver. A link-only check could use `beagley_ai/j722s/mcu_r5f0_0` with a stub driver; it would prove toolchain and ABI, not the AM62P | `ZEPH` | inferred; not tried |

## TI AM62P, the two-plane backend, read from source

| Item | Finding | Source doc | Basis |
| --- | --- | --- | --- |
| Backend | `backends/twoplane` (`ce_twoplane.h`, `ce_twoplane.c`), taken from the AM62P firmware; tested on the host against a fake driver. `latched_count` follows the GO bit | `TP`, `MATRIX` | source (this repository's code and doc); no board run |
| Compile for Arm | Compiled by hand once for `armv7r-none-eabihf` (Cortex-R5) with clang 23, objects not linked or run | `TP`, `MATRIX` | source (the doc's statement); not a board result |
| Two-plane composition on the glass | Application plane beneath the safety plane, as the engine composes it | `PLAN`, `TP` | not measured (pending docs/m3-am62p-report.md) |
| Releases, refused scenes | Buffer releases per scene and the scenes the engine refuses | `PLAN` | not measured (pending docs/m3-am62p-report.md) |
| Photographs of the eight scenes and their camera checks | Whether the panel shows what each scene expects | `PLAN` | not measured (pending docs/m3-am62p-report.md) |

## TI AM62P, inferred

Each point is drawn from the rows above and is not asserted by a source.

- The two-plane composition fits the hardware: two planes per DSS instance ordered by an overlay manager, per-plane global alpha, pre-multiplied blending (`DSS`). Under Linux with one output the primary is the lite plane, which cannot scale, so scaled content must go on the overlay (`DSS`).
- A commit is one GO write after the plane writes, and one `latched_count` increment per GO 1 to 0 transition (`GO`). If the hardware merges two commits under one GO, a backend waiting for `latched >= commit` waits for the next commit (`GO`).
- The only safe layouts are TI's partitioned shared mode or Linux fully detached from the DSS during the run; this project's route is the second (`SHARE`).
- The Verdin image probably runs with no display overlay until one is selected, so the display route has to be read on the board (`DT`).

## What a reader should not conclude from this section

- Nothing about the AM62P was measured. A row tagged **source** is what a file says, not what the board does.
- The kernel read may not be the kernel on the board.
- Single-DSS-instance facts (two planes, zpos 0..1) do not say which instance or video port the Verdin's panel uses.
