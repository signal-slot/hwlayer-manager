# Platform capabilities

Status of what is known about each target, and how it was learned. The only board measurement so far is the i.MX95 Linux topology captured on 2026-11-28 (section "i.MX95, Linux, observed on the Verdin iMX95"); everything else is desk research or vkms. Rows are tagged:

- **observed**: read from a real board with the repository's `probe` (date and kernel given per section). This is the KMS view the kernel exposes; it says nothing about what a commit accepts.
- **vkms**: observed by running the backend against the kernel's virtual driver.
- **source**: read from driver or tool source. Not run.
- **inferred**: a conclusion drawn from the source, not stated by it.

Desk research was done on 2026-11-19 by reading the sources listed at the end. Key points of the i.MX95 driver (CRTC count, overlay `possible_crtcs`, zpos/alpha/blend properties, upscale-only, linear modifier) were spot-checked against the fetched files; the rest is as reported by that reading and has not been re-checked line by line. For the i.MX95 topology, the observed section below now takes precedence over the source rows; the source rows that a probe cannot see (fetch units, scalers, limits checked in `atomic_check`) are still unmeasured.

## i.MX95, Linux, observed on the Verdin iMX95

Captured 2026-11-28 with `probe --raw /dev/dri/card0` (cross-built with `scripts/cross-aarch64.sh -p composition-drm --example probe`, run from `/tmp`, removed afterwards). Raw output: [`imx95-probe-2026-11-28.txt`](imx95-probe-2026-11-28.txt).

- Board: Toradex Verdin iMX95 (`/proc/device-tree/model`: "Toradex Verdin iMX95 WB on Verdin Development Board"; the carrier in use is a Mallow), SoC i.MX95 revision 2.0 (`/sys/devices/soc0`). Panel: Toradex Capacitive Touch Display 10.1" LVDS on X12.
- BSP: Toradex Reference Multimedia Image 7.8.0 (`/etc/os-release`: "TDX Wayland with XWayland", `7.8.0+build.17 (scarthgap)`). Kernel: `6.6.157-7.8.0`. Driver: `imx95-dpu` 1.0.0 (20230213).
- Display client: Weston (`weston.service`) was running and holding `/dev/dri/card0` during the capture. It was not stopped (the probe needs no DRM master) and was still active with the same PID afterwards. Property values marked "Weston state" are what Weston had programmed, not driver defaults.

| Item | Finding | Basis |
| --- | --- | --- |
| Device | `/dev/dri/card0` (+ `renderD128`), one DRM device, driver `imx95-dpu` | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Client caps | Universal planes and atomic accepted | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Driver caps | `ADDFB2_MODIFIERS` 1, `DUMB_BUFFER` 1, `DUMB_PREFER_SHADOW` 0, cursor 64x64, `ASYNC_PAGE_FLIP` 0; `ATOMIC_ASYNC_PAGE_FLIP` not known to this kernel (`EINVAL`) | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Resources | 2 CRTCs (39, 50), 1 encoder (40), 1 connector (41) | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Connector | 41, type LVDS (`card0-LVDS-1`), connected, encoder 40 → CRTC 39 only. Properties: EDID, DPMS, link-status, non-desktop, TILE, CRTC_ID | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Mode | One mode, 1280x800 @ 59.993 Hz; CRTC 39 active with it (Weston state). CRTC 50 inactive, no connector can reach it on this board | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| CRTC properties | ACTIVE, MODE_ID, OUT_FENCE_PTR, VRR_ENABLED. No GAMMA_LUT, CTM or DEGAMMA_LUT | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Planes | 7: primary 31 (CRTC 39 only), primary 42 (CRTC 50 only), overlays 51, 59, 67, 75, 83 (both CRTCs, `possible_crtcs` 0b11). No cursor plane, although the cursor-size cap reports 64x64 | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Usable on the LVDS output | Primary 31 + 5 overlays = 6 planes on CRTC 39 | observed 2026-11-28, kernel 6.6.157-7.8.0 (that all six can be active at once is source, not tested) |
| Weston's use | Only primary 31 attached (FB 97, full screen 1280x800). All overlays unattached | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Formats / IN_FORMATS | Every plane: AR24 XR24 AB24 XB24 RA24 RX24 BA24 BX24 RG16 YUYV UYVY NV12 NV21 (13), each with modifier 0x0 (linear) only. IN_FORMATS blob 104 bytes per plane | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| zpos | Mutable range 0..5 on every plane (not immutable). Values: overlays 1, primary 42 0 (match the source defaults); primary 31 is 5 (Weston state) | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Plane alpha | `alpha` mutable 0..65535 on every plane, value 65535 | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Blend modes | `pixel blend mode` enum None=2, Pre-multiplied=0, Coverage=1 on every plane, value Pre-multiplied | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Rotation | No `rotation` property on any plane | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Scaling-related | `SCALING_FILTER` enum Default, Nearest Neighbor on every plane. No property states scaling limits; the probe reports "scaling not claimed" | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Color properties | `COLOR_ENCODING` BT.601 / BT.709 (value BT.709), `COLOR_RANGE` limited / full (value limited) on every plane | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Fences | `IN_FENCE_FD` on every plane, `OUT_FENCE_PTR` on both CRTCs | observed 2026-11-28, kernel 6.6.157-7.8.0 |
| Upscale only, one shared HScaler/VScaler, 2048 scaled source limit, fetch-unit pool, off-screen reject, BT.709 full-range reject | Not visible as properties | source (see below and `imx95-dpu-source-6.6.md`); to be tested by `TEST_ONLY` |
| Vertical scaling on this SoC | Revision 2.0 is not rev 1.x, so by the 6.6 reading the driver uses VScaler4 (`use_vs4 = true`) | inferred from the observed revision and the source reading |

Compared with the source rows below: CRTC count, plane count and types, `possible_crtcs`, formats, linear-only modifiers, zpos range and defaults, alpha, the three blend modes and the absence of rotation all match. New against the 6.18 reading: the `COLOR_ENCODING`/`COLOR_RANGE` and `SCALING_FILTER` properties (the 6.6 reading lists them). Board-specific: one LVDS connector, wired to CRTC 39 only; the second CRTC and primary 42 are unusable here, but the overlays are still shared with CRTC 50, so a default drm-lease-manager lease of the LVDS connector would contain primary 31 only (inferred from the lease-manager rows below).

## i.MX95, Linux (NXP vendor kernel), read from source

Tree: `nxp-imx/linux-imx`, branch `lf-6.18.y`, commit `6e694eda40aa`. Driver: `drivers/gpu/drm/imx/dpu95/` (`imx95-dpu`). Other branches were not read.

| Item | Finding | Basis |
| --- | --- | --- |
| CRTCs | 2 (display stream 0 and 1), one encoder and bridge connector each | source |
| Planes | 7: 2 primary (one per CRTC), 5 overlay, no cursor | source |
| Plane → CRTC | Primary: its own CRTC only. Overlays: both CRTCs | source (primary mask is inferred from DRM core behaviour) |
| Hardware behind planes | 6 fetch units (2 FetchLayer, 4 FetchYUV) and 6 LayerBlends, allocated per commit. At most 6 planes active across both CRTCs | source |
| Formats | Same list on every plane: ARGB/XRGB/ABGR/XBGR/RGBA/RGBX/BGRA/BGRX8888, RGB565, YUYV, UYVY, NV12, NV21 | source |
| Modifiers | Linear only | source |
| YUV | Needs a FetchYUV unit (4 exist). BT.709 full range is rejected | source |
| Scaling | Upscale only. One horizontal and one vertical scaler shared by all planes on both CRTCs. A scaled plane needs a FetchYUV unit | source |
| Source size | Up to 8192 unscaled, 2048 scaled | source |
| Plane alpha | Yes | source |
| Blend modes | None, Pre-multiplied, Coverage. The bottom plane (normalized zpos 0) is forced opaque | source |
| zpos | Mutable, 0..5 | source |
| Rotation | No property | source |
| Off-screen destination | Rejected, no clipping | source |
| Active CRTC with no plane | Rejected. The primary need not be full-screen | source |
| Cross-CRTC migration | A fetch unit, scaler or LayerBlend freed on one CRTC cannot be taken by the other in the same commit | source |
| Bandwidth check | None found | source |
| Clocks and power | From SCMI (System Manager) | source |

### Consequences for this project

- **M1 must use the vendor kernel.** The i.MX95 DPU is not in mainline (checked at v7.4-rc3). The upstream series extends the `dc` driver, was still unmerged with an open structural disagreement in June 2026, and mainline `dc` planes are XRGB8888, primary only. An upstream-based BSP would expose no overlays. (The last point is inferred; not all patches of the series were read.)
- **Two hardware planes on one output look achievable**: a primary plus up to 5 overlays, with mutable zpos, plane alpha and all three blend modes. The backend already handles a mutable `zpos` range.
- **The fetch-unit pool and the shared scalers are modelled** as resource groups keyed on layer traits (YUV, scaled per axis), in `DriverProfile::Imx95Dpu` of `composition-drm`. The profile is opt-in. It was checked on 2026-11-28 (`profile-check`) against the board capture above and against the 6.6.157-7.8.0 driver source (`imx95-dpu-source-6.6.md`, kernel commit `64f85d65c112`). Result: the topology it assumes (six planes per output, mutable zpos, alpha, three blend modes, linear only, no rotation, no cursor plane on this board) is **observed**; the resource groups (6 fetch units, 4 of them FetchYUV for YCbCr/scaled, one HScaler, one VScaler, upscale only up to 2048x) are **source** for 6.6 as for 6.18 and are still **not measured**, because KMS does not show them. The vertical scaler being present is **inferred** (revision 2.0 is not 1.x). Do not use the profile unchanged on a revision 1.x part, where vertical scaling is refused.
- **Differences found and what was done:** (1) `max_source` 8192 is the limit of an unscaled layer only; scaled layers are limited to 2048 per axis in the 6.6 source (as in the 6.18 reading). One `max_source` cannot express both, so the profile keeps 8192 and the 2048 scaled limit is **not modelled** (the doc comment now says so; `TEST_ONLY` refuses it). (2) The doc comment cited only 6.18 and "unmeasured"; it now tags every constraint observed / source (commit) / inferred. (3) No profile value changed otherwise: the other constants match both sources.
- **Not modelled:** the 2048-pixel source limit for scaled planes, the even-coordinate rule for subsampled formats, the bottom plane being forced opaque (alpha and blend mode ignored at normalized zpos 0, source), BT.709 full range on YCbCr, and units taken by the other display stream (none is usable on this board, which has one connector). `TEST_ONLY` covers these.
- **Two engines on the two CRTCs are not independent.** They draw from the same fetch units and scalers, so one lessee can make the other's commit fail (inferred).
- **The driver rejects partly off-screen destinations, so clipping has to happen before it.** The planner does that and hands the backend rectangles that lie on the output.

## i.MX95, Zephyr

See [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md): ownership model, how the M7 is loaded, the display path and the blockers.

## AGL drm-lease-manager

Tree: AGL gerrit `src/drm-lease-manager`, `master` at `68c48758c46d`.

| Item | Finding | Basis |
| --- | --- | --- |
| Default lease | One per connector: the connector, one CRTC, and every plane usable only with that CRTC. No filtering by plane type | source |
| Planes shared between CRTCs | Left out unless listed in the configuration | source |
| Configuration | TOML, default `/etc/drm-lease-manager.toml`: per connector `planes = [id, ...]`. Present from the marlin release | source |
| Universal planes | Since June 2024 (ricefish onward) the manager only enumerates overlays unless started with `-u`; primary and cursor are then added by the kernel implicitly | source; the implicit add is inferred |
| Bad configuration | A parse error silently falls back to the default leases | source (configuration guide) |
| Option parsing | On ricefish and later `-c` appears to take no argument and `-u` to require one; use `--config <path>` | inferred from `main.c`, not tested |
| Client API | `dlm_get_lease(name)`, `dlm_lease_fd()`, `dlm_release_lease()` | source |
| Revocation | On client release or disconnect, and with `-t` when another client asks for the same lease. Calls then fail with `ENOENT` | source (README) |

### Consequences for this project

- On i.MX95 the overlays are shared between both CRTCs, so **a default lease would contain the primary only**. The overlays must be listed by plane ID in the TOML file. Plane IDs are board- and driver-specific.
- Because a broken configuration falls back silently, check the lease at runtime. `DrmBackend` reports exactly the planes the lease fd exposes, so the `probe` example run on a lease fd shows it.
- `ENOENT` after revocation is what the backend already maps to `DeviceLost`.

## vkms (kernel 7.2, virtual)

Observed with `probe` and the tests in `crates/composition-drm/tests/vkms.rs`.

| Item | Finding |
| --- | --- |
| Topology | 1 CRTC, 1024x768. 1 primary, 8 overlays, 1 cursor (with `enable_overlay=1 enable_cursor=1`) |
| Properties | No `zpos`, `alpha` or `pixel blend mode` property. Stacking follows plane order; blending is premultiplied |
| Formats | More than 32 per plane; the report is truncated at `MAX_FORMATS` |
| Lease | A lease with the primary and one overlay composes two planes; planes outside the lease are invisible; revocation is reported |

vkms is a software model. It validates the backend's use of the KMS API and the lifecycle, and says nothing about any SoC.

## TI AM62P

Not researched yet. The existing implementation in `signal-slot/slint-zephyr-yakogl-mixed-criticality-ui` is the reference.

## Sources

- https://github.com/nxp-imx/linux-imx/tree/6e694eda40aabd6671fca82b5416dbd76beeebb9/drivers/gpu/drm/imx/dpu95
- https://github.com/nxp-imx/linux-imx/blob/6e694eda40aabd6671fca82b5416dbd76beeebb9/arch/arm64/boot/dts/freescale/imx95.dtsi
- https://github.com/torvalds/linux/tree/master/drivers/gpu/drm/imx/dc
- https://www.mail-archive.com/dri-devel@lists.freedesktop.org/msg570400.html
- https://mid.mail-archive.com/dri-devel@lists.freedesktop.org/msg612534.html
- https://github.com/zephyrproject-rtos/zephyr/blob/main/drivers/display/display_socionext_dpu.c
- https://github.com/zephyrproject-rtos/zephyr/blob/main/dts/arm/nxp/imx/nxp_imx95_m7.dtsi
- https://docs.zephyrproject.org/latest/boards/nxp/imx95_evk_15x15/doc/index.html
- https://gerrit.automotivelinux.org/gerrit/projects/src%2Fdrm-lease-manager/branches/
- `drm-lease-manager/lease-manager.c`, `drm-lease-manager/main.c`, `libdlmclient/dlmclient.h`, `README.md`, `doc/configuration-guide.md` on that branch
