# Platform capabilities

Status of what is known about each target, and how it was learned. Nothing here has been measured on a board. Rows are tagged:

- **vkms**: observed by running the backend against the kernel's virtual driver.
- **source**: read from driver or tool source. Not run.
- **inferred**: a conclusion drawn from the source, not stated by it.

Desk research was done on 2026-11-19 by reading the sources listed at the end. Key points of the i.MX95 driver (CRTC count, overlay `possible_crtcs`, zpos/alpha/blend properties, upscale-only, linear modifier) were spot-checked against the fetched files; the rest is as reported by that reading and has not been re-checked line by line. Replace every "source" row with a measurement (`cargo run -p composition-drm --example probe`) when a board is available.

## i.MX95, Linux (NXP vendor kernel)

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
- **The fetch-unit pool and the shared scalers are modelled** as resource groups keyed on layer traits (YUV, scaled per axis), in `DriverProfile::Imx95Dpu` of `composition-drm`. The profile is opt-in and encodes the rows above, so it is exactly as unverified as they are. Without it the same frames are still refused, by `TEST_ONLY`, after the planner has tried several equivalent assignments.
- **Not modelled:** the 2048-pixel source limit for scaled planes, the even-coordinate rule for subsampled formats, and units taken by the other display stream. `TEST_ONLY` covers these.
- **Two engines on the two CRTCs are not independent.** They draw from the same fetch units and scalers, so one lessee can make the other's commit fail (inferred).
- **The driver rejects partly off-screen destinations, so clipping has to happen before it.** The planner does that and hands the backend rectangles that lie on the output.

## i.MX95, Zephyr

Tree: `zephyrproject-rtos/zephyr`, `main`.

| Item | Finding | Basis |
| --- | --- | --- |
| Driver | `drivers/display/display_socionext_dpu.c` (`socionext,dpu`), merged May 2026 | source |
| Capability | Single framebuffer: a fixed ConstFrame0 + FetchYUV0 → LayerBlend1 → ExtDst0 pipeline. RGB565, RGB888, ARGB8888. No planes, scaling or alpha | source |
| Core | DPU and DSI nodes exist only for the M7, both disabled by default | source |
| Boards | Neither EVK lists `display` as supported. Only the 19x19 M7 board carries display labels | source |
| A55 vs M7 ownership | Nothing in Linux or Zephyr arbitrates it; it would be System Manager configuration, which was not examined | not found |

Consequence: M2 scanout looks reachable on the M7, most plausibly on the 19x19 EVK. Hardware overlay under Zephyr does not exist in the stock driver and would mean writing a multi-plane DPU driver. Who may own the DPU when Linux also runs is open and is the first thing to settle.

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
