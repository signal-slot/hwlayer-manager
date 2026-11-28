# Sharing the AM62P DSS between Linux and a remote core (source study)

Status: source read, written 2026-11-28. **This document claims nothing about hardware.** No board
was used, no code is changed. Nothing below is `observed`.

## Tags

- **source**: read in the public file or page named, on 2026-11-28. It says what the code or
  documentation says, not what a board does.
- **inferred**: a conclusion drawn from source lines, or something a file not read would have to
  confirm.

## Sources

All accessed 2026-11-28. The fetch tool showed no 40-hex commit SHA for any of them. The branch
name is the pin, so a later reader may see newer text.

| Name | Ref | Where |
|------|-----|-------|
| TI kernel | branch `ti-linux-6.12.y` | `https://raw.githubusercontent.com/TexasInstruments/ti-linux-kernel/ti-linux-6.12.y/` |
| SDK doc, DSS7 | Processor SDK Linux AM62PX 12.01 | `https://software-dl.ti.com/processor-sdk-linux/esd/AM62PX/latest/exports/docs/linux/Foundational_Components/Kernel/Kernel_Drivers/Display/DSS7.html` |
| SDK doc, how-to | same | `https://software-dl.ti.com/processor-sdk-linux/esd/AM62PX/latest/exports/docs/linux/How_to_Guides/Target/How_to_enable_display_sharing_between_remotecore_and_Linux.html` |
| SDK doc, demo | same | `https://software-dl.ti.com/processor-sdk-linux/esd/AM62PX/latest/exports/docs/linux/Demo_User_Guides/Display_Cluster_User_Guide.html` |
| MCU+ SDK example | AM62PX 12.01.00 | `https://software-dl.ti.com/mcu-plus-sdk/esd/AM62PX/latest/exports/docs/api_guide_am62px/examples/drivers/dss_display_share.html` |

Files read in the TI kernel: `drivers/gpu/drm/tidss/tidss_drv.c`, `tidss_drv.h`, `tidss_dispc.c`,
`tidss_kms.c`, `Documentation/devicetree/bindings/display/ti/ti,am65x-dss.yaml` and
`arch/arm64/boot/dts/ti/k3-am62p5-sk-dss-shared-mode.dtso`. The raw-file fetch gave no line
numbers, so statements cite file and function.

**Not read (limits):**

- The Toradex tree (`linux-toradex`, the branch Torizon uses). The repository and branch names
  tried returned 404, so there is no Toradex comparison and no Verdin AM62P device tree was read.
  Whether the Toradex kernel has the `ti,dss-shared-mode*` code is unknown.
- `ti-linux-6.6.y`, `ti-linux-6.1.y` and the `09.01.00.008` tag. Also `tidss_crtc.c`,
  `tidss_plane.c`, `tidss_oldi.c` and `tidss_dispc.h`.
- Mainline `torvalds/linux` `tidss_drv.c` (HTTP 429). Only the directory listing was read: it has
  the same file set as TI's tree and no extra shared-mode file.
- The TI e2e forum, the TRM and the TISCI host-ID tables.

## (a) Can Linux tidss leave a VP, overlay or plane to another core?

1. **source**: Yes, in TI's tree, as "shared mode". Only device-tree properties on the DSS node
   select it. `tidss_probe()` in `tidss_drv.c` reads
   `tidss->shared_mode = device_property_read_bool(dev, "ti,dss-shared-mode");`.
2. **source**: The other properties are read in `dispc_update_shared_mode_features()` in
   `tidss_dispc.c`. The SDK DSS7 page writes `-owned-vp`; the code and the overlay write
   `-vp-owned`, which is the spelling to use.

   | Property | Meaning (SDK DSS7 page; the code agrees) |
   |----------|-------------------------------------------|
   | `ti,dss-shared-mode` | enable display sharing |
   | `ti,dss-shared-mode-planes` | planes Linux owns (e.g. `"vid"`) |
   | `ti,dss-shared-mode-vp` | VPs that drive those planes (e.g. `"vp1"`) |
   | `ti,dss-shared-mode-vp-owned` | per VP: 1 = Linux owns the VP (write); 0 = the remote core owns it and Linux only shares it |
   | `ti,dss-shared-mode-common` | register region Linux programs (`common` or `common1`) |
   | `ti,dss-shared-mode-plane-zorder` | z-order of Linux's planes relative to the remote core's |

3. **source**: The driver prunes a copy of the SoC feature table from these properties. The
   overlay manager name comes from the VP name (`vp1` gives `ovr1`). A plane name starting with
   `vidl` is marked `vid_lite`. Common name `common1` selects the `tidss_am62_common1_regs` table.
   `tidss_kms.c` has no shared-mode code and builds CRTCs and planes from the pruned table, so
   Linux sees only its own VPs and planes.
4. **source**: There is no Kconfig option and no module parameter for it. The device tree is the
   only switch. `tidss_drv.h` has `shared_mode`, `shared_mode_owned_vps[]` and
   `shared_mode_own_oldi` ("Linux needs to configure OLDI in shared mode").
5. **source**: The AM62P compatibles in the OF match table are `ti,am62p51-dss` and
   `ti,am62p52-dss`. There is no `ti,am62p-dss`.
6. **source**: The binding `ti,am65x-dss.yaml` in `ti-linux-6.12.y` does not list the
   `ti,dss-shared-mode*` properties, although the schema has `additionalProperties: false`. The
   SDK page points to the binding at the `09.01.00.008` tag, which could not be read.
   **inferred**: the properties are TI-vendor and a schema check would flag them. Not checked.
7. **Versions.** **source**: present in `ti-linux-6.12.y`. **source**: the SDK docs point to the
   `09.01.00.008` tag for the overlay. **inferred**: the feature exists from SDK 9.1 (kernel 6.6
   era). **inferred**: mainline does not have it (the listing shows no extra file), but
   `tidss_drv.c` was not read. Toradex: unknown (see Sources).
8. **source**: TI ships an overlay for the AM62P SK board, `k3-am62p5-sk-dss-shared-mode.dtso`,
   for the TI firmware `dss_display_share.wkup-r5f0_0.release.strip.out`. Its `&dss0` node sets
   `ti,dss-shared-mode`, `ti,dss-shared-mode-vp = "vp1"`, `ti,dss-shared-mode-vp-owned = <0>`,
   `ti,dss-shared-mode-common = "common1"`, `ti,dss-shared-mode-planes = "vid"` and
   `ti,dss-shared-mode-plane-zorder = <0>`. It adds an `interrupts` entry (`GIC_SPI 85`) and a
   128 MiB `no-map` reserved region `rtos-framebuffer-memory@93500000`.
9. **source**: Shared mode needs no user-space change: "the underlying kernel framework takes care
   of the display sharing functionality" (DSS7 page).
10. **source**: TI's granularity: a pipeline (VID or VIDL) belongs to one core. A VP has one write
    owner, and other cores get read-only access. Overlay managers follow the VP's owner. Each
    core uses its own register region (`common` or `common1`), and only one core holds the global
    configuration region. Two layouts are documented: (1) Linux owns one plane (VID) on a VP the
    remote core owns; (2) Linux owns a whole VP and pipeline and the remote core another
    (`vp-owned = <1>`).

## (b) What the remote core owns and what stays with Linux

For TI's layout 1 (the MCU+ `dss_display_share` example and the shared-mode overlay):

| Resource | Remote core | Linux | Tag |
|----------|-------------|-------|-----|
| DSS instance | DSS0 | DSS0 (`&dss0`) | source |
| Pipeline | VIDL | VID | source (the example "expects linux to use VID pipeline") |
| Video port and overlay | VP1 and OVR1, owned | VP1 shared, not owned | source (overlay `vp-owned = <0>`) |
| Register region | `common` (global configuration) | `common1` | source (how-to table; `dispc_owns_global_common_in_shared_mode()`) |
| Z-order | VIDL above VID | VID below | source; the two TI pages give different numbers (see below) |
| Global init, `fck`, soft reset | yes | no: with `common1` Linux skips `fck`, `max-memory-bandwidth` and `dispc_init_hw()` | source (`dispc_init()`) |
| Power domain | firmware side | tidss skips PM-domain attach and `pm_runtime_enable`; `tidss_runtime_get()` returns early ("No PM in display sharing mode") | source |
| OLDI and panel | the example configures the OLDI LVDS panel on VP1 | only if `shared_mode_own_oldi` | source |
| Framebuffer memory | reserved region, firewalled so that only the DSS and the remote core reach it | does not map it (`no-map`) | source |

- Z-order numbers: the how-to page says Linux 0 / remote 1; the MCU+ page says VID 1 / VIDL 2.
  **inferred**: the how-to numbers are a relative order and the example's are hardware values.
- **source**: the remote core must run firmware "which programs the display hardware by supporting
  this resource paritioning scheme" (DSS7 page; the typo is TI's). Linux does no global
  initialisation in this mode. **inferred**: the remote core must therefore bring up clocks, the
  global region, VP1 and OVR1 itself.
- **source**: TI's example runs on the **Device Manager R5** (`wkup-r5fss0-0`, FreeRTOS). The SBL
  must start it; CCS cannot load it. It shows a splash and tell-tale frames before Linux boots.
  A firewall function gives the RTOS core read, write, cache and debug access to the DSS regions
  and gives the Linux core read-only access.
- **source**: the Display Cluster demo says the R5 keeps displaying if the A53 kernel crashes
  (`echo c > /proc/sysrq-trigger`).
- **not found**: the TISCI host IDs and `ti,sci-dev-id` values for the remote core's DSS clock and
  power requests. No page read lists them.
- **inferred**: TI's documented remote core is the DM R5, not the MCU R5F that the repository's
  board test targets. No TI page read describes the MCU R5F as the display owner. An MCU R5F that
  needs DSS clocks or firewall access would need its own TISCI host assignment and firewall entry,
  which TI's example puts in SBL or DM code. Not verified.

## (c) What a test that drives the DSS from the MCU R5F must stop, unbind or configure

Everything here is **inferred** from (a) and (b) unless a source tag is given. No TI page read
describes an MCU R5F test.

There are two routes.

1. **Shared-mode route (TI's way).** Boot Linux with the shared-mode overlay, or an equivalent for
   the Verdin. Whether the Toradex tidss accepts the properties is unknown. Linux then owns only
   `common1` and VID, and the remote core owns VP1, OVR1 and the global setup. The test needs:
   - the overlay and a panel overlay in the boot chain;
   - the framebuffer region reserved `no-map`;
   - firmware that does the global init before or without Linux (**source**: Linux does none).

   Restore: boot the stock device tree again. **inferred**: this needs a reboot, because the
   property is read at probe.
2. **Take-over route (stock tidss owns the whole DSS).** The test must first make Linux stop
   touching the DSS:
   - The display stack above tidss: stop the compositor (for example Weston) and anything holding
     `/dev/dri/card*`, so that no atomic commit reaches tidss.
   - fbcon: unbind the virtual console or disable it so it does not redraw. DRM fbdev emulation
     keeps a framebuffer on the CRTC until tidss is unbound.
   - The tidss platform driver: unbind it from the DSS device through sysfs. DRM then tears down
     the CRTCs and runtime PM may power the DSS domain down, so the R5F must do the clock, power
     domain, VP and OLDI setup itself afterwards.
   - The panel, backlight and regulators may stay under Linux. Whether the R5F needs them stopped
     depends on the panel route and was not read.

   Restore: re-bind tidss, restart the compositor, re-bind the virtual console. The DSS is left
   in the R5F's state, and the driver's probe runs `dispc_init_hw()` (**source**: the global init
   in `dispc_init()`), but no source read shows that a re-bind alone recovers. A reboot is the
   safe restore.
3. **Access first.** The R5F must be allowed to reach the DSS registers and the framebuffer memory
   at all. If a firewall or TISCI assignment denies the MCU R5F, unbinding Linux changes nothing.
   This is the main unknown for the board session. The device-tree and remoteproc studies of the
   plan should answer it; they are not linked here because they are not in the tree yet.

## (d) Risks if both cores program the DSS

- **source**: TI protects by partition, not by arbitration: separate register regions (`common`,
  `common1`), one write owner per VP, a pipeline that belongs to one core, and a firewall that
  makes the other core read-only. No page read says what happens if both cores write the same
  region. e2e was not read.
- **inferred**: two writers to the same VP, overlay-manager or pipeline registers leave an
  undefined mix of settings. Expected failure modes:
  - **Clobbered configuration.** The last write wins per register. A Linux commit can change the
    VP timing, the z-order or a pipeline address that the R5F set.
  - **Shared latch.** The shadow-register GO bit is per VP (not read in the TRM here). If both
    cores set GO on one VP, one core's commit can latch the other's half-written registers, and
    the commit count no longer matches the frames latched.
  - **Power and clock.** Linux runtime PM can gate the DSS power domain or `fck` while the R5F
    uses it. TI's shared mode avoids this by making tidss skip PM and global init (**source**).
  - **Interrupts.** Both cores can see the VP vsync or underflow interrupt, and one can clear
    status the other waits for. The overlay's extra `interrupts` entry shows the DSS interrupt is
    routed to Linux (**source**). The ack race is **inferred**.
  - **Memory.** A framebuffer in memory Linux also maps can be changed by the kernel. This is why
    TI uses a `no-map` region (**source**).
  - **Teardown.** Linux unbind or shutdown can disable the VP or OLDI under a running R5F. Not
    read in the tidss disable paths.
  - **Crash isolation.** In shared mode the remote core keeps the panel alive when Linux crashes
    (**source**, demo page). Without shared mode, a Linux crash during an R5F run proves nothing.
- **inferred**: the only safe configurations are TI's partitioned shared mode, or Linux fully
  detached from the DSS during the R5F run.

## Open questions for the board session

These are not answered by the sources above.

- Does the Toradex kernel for Torizon on the Verdin AM62P carry `ti,dss-shared-mode`?
- Can the MCU R5F reach the DSS registers and the framebuffer memory (firewall, TISCI host)?
- Does a tidss re-bind after an R5F run recover without a reboot?
