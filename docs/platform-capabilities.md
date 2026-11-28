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

## Broadcom BCM2712 (HVS, Raspberry Pi 5)

Raspberry Pi 5 Model B Rev 1.1, probed and driven with `probe` and `examples/twoplane.rs` over ssh. Records: `docs/observed/bcm2712-topology.md`, `docs/observed/bcm2712.md`; kernel source reading: `docs/observed/vc4-hvs-source.md`. Dates are the local date of the session (the harness `DATE` line printed 2026-11-27 UTC). Nobody held DRM master (no compositor running), so nothing was stopped. None of this is a claim toward M1 to M3.

### BCM2712, observed

| Item | Finding | Basis |
| --- | --- | --- |
| Kernel, driver | `6.18.50+rpt-rpi-2712 #1 SMP PREEMPT Debian 1:6.18.50-1+rpt1 (2026-09-11) aarch64`, Debian 13. DRM `card0` = `v3d` (render only), `card1` = `vc4-drm` (compatible `brcm,bcm2712-vc6`). KMS device `/dev/dri/card1`, driver name `vc4`, version 0.0.0 | observed 2026-11-28 |
| Output, mode | HDMI-A-1 connected (HDMI-A-2 disconnected, two writeback connectors), CRTC 94 (index 2 of 4). 1920x1080 @ 60.000 Hz, 148.5 MHz, pinned by `video=HDMI-A-1:1920x1080M@60` on the kernel command line | observed 2026-11-28 |
| CRTCs, encoders | 4 CRTCs (59, 78, 94, 106); only 94 active. 2 TMDS and 2 virtual (writeback) encoders | observed 2026-11-28 |
| Planes in the kernel | 56 | observed 2026-11-28 |
| Plane-to-CRTC masks (CRTC indices 0..3) | Primaries 48, 67, 83, 95: own CRTC only. 16 shared overlays (107, 118, ... 272): CRTCs 1, 2, 3. 34 planes: CRTC 0 only (types not read). One extra plane per CRTC 1/2/3 (646, 657, 668): own CRTC only, type not read. So the active output reaches primary 83, the 16 shared overlays and plane 657 (18 planes) | observed 2026-11-28 (masks counted from `kmsprint -l`; types of five planes read) |
| Extra planes 646, 657, 668 | Presumed cursor planes | inferred |
| Formats | 45 linear: XR24 AR24 AB24 XB24 RX24 RA24 BX24 BA24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12 RX12 RA12 P010 P012 P016 S010 S012 S016. `IN_FORMATS` identical on the five planes read (48, 83, 107, 239, 382) | observed 2026-11-28 |
| Modifiers | `0x0` linear; `0x0700000000000001` (XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15); `...03` (NV12 NV21); `...04` (NV12 NV21 P030); `...05` (NV12 NV21). Numeric codes read from the blob | observed 2026-11-28 |
| Names of the Broadcom modifier codes | 1 = VC4 T-tiled, 3 / 4 / 5 = SAND-type, matched from memory of `drm_fourcc.h`, no header read on the board | inferred |
| zpos | Primaries immutable 0. Overlays mutable 1..17 (values at read: plane 107 = 1, 239 = 13, 382 = 10) | observed 2026-11-28 |
| Alpha, blend | `alpha` 0..65535, default 0xffff. `pixel blend mode` None=2, Pre-multiplied=0, Coverage=1, default Pre-multiplied | observed 2026-11-28 |
| Rotation | `rotation`: rotate-0, rotate-180, reflect-x, reflect-y (no 90/270); idle value rotate-0. composition-drm reports `transforms 0x0` and does not program it | observed 2026-11-28 |
| Other plane properties | `COLOR_ENCODING` BT.601 / 709 / 2020 (default 709), `COLOR_RANGE`, `SCALING_FILTER` (Default, Nearest Neighbor), `CHROMA_SITING_H/V` | observed 2026-11-28 |
| Scaling | No KMS scaling property; probe prints "scaling not claimed". `TEST_ONLY` accepted a hand-built 2x upscale and a 1/2 downscale of the overlay. No larger factor was tried, so no limit is measured | observed 2026-11-28 |
| What probe shows | 16 of the 56 planes (4 primaries, 12 overlays 107..228); 32 of 45 formats per plane and no modifiers. Cause: the limits `MAX_PLANES` (16) and `MAX_FORMATS` (32) in `composition-core`, read from the constants, not traced in code | observed 2026-11-28 (counts); cause inferred |
| Plane assignment of the two-plane harness | Layer 0 (background, XR24 1920x1080) -> primary 83 zpos 0; layer 1 (overlay, AR24 384x540 at 960,270) -> overlay 107 zpos 1; 3 buffers per layer; `PROFILE Generic`. Debugfs `state` during the run: planes 83 and 107 on `crtc-2`, fbs allocated by `twoplane`; `hvs_dlists` channel 0 held a 1920x1080 entry and a 384x540 entry at x=960, y=270 | observed 2026-11-28 |
| Verdict, default `--frames 600` | `VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver vc4 mode 1920x1080@60.000Hz`. 0 dropped, latch interval mean 16.667 ms, max 16.690 ms; `RELEASE after the run: released 1200 of 1200 buffer uses, 0 still on screen` | observed 2026-11-28 |
| Verdict, `--frames 5400` (KVM capture during the run) | `VERDICT twoplane: PASS 5400 frames on two planes, 5400 latched, 10798 released, 0 late, driver vc4 mode 1920x1080@60.000Hz`. 0 dropped, max interval 16.694 ms | observed 2026-11-28 |
| Verdict, `--swap-z --frames 3600` | `VERDICT twoplane: PASS 3600 frames on two planes, 3600 latched, 7198 released, 0 late, driver vc4 mode 1920x1080@60.000Hz`, after `SWAPZ skipped: a plane of the plan has a fixed zpos`: no swap happened, because the planner puts the background on the primary, whose zpos is immutable. Max interval 16.724 ms | observed 2026-11-28 |
| Verdict, `--frames 120 --unsupported` | `VERDICT twoplane: PASS 120 frames on two planes, 120 latched, 238 released, 0 late, driver vc4 mode 1920x1080@60.000Hz`. Max interval 16.680 ms | observed 2026-11-28 |
| Verdict, `--blend coverage --plane-alpha 32768 --frames 2400` (KVM capture) | `VERDICT twoplane: PASS 2400 frames on two planes, 2400 latched, 4798 released, 0 late, driver vc4 mode 1920x1080@60.000Hz`. Max interval 16.716 ms | observed 2026-11-28 |
| Re-run from commit 20df182 | `probe` output byte-identical; default, `--swap-z --frames 600` and `--frames 120 --unsupported` gave the same plans, verdicts, `SWAPZ` and `UNSUPPORTED` lines; 0 dropped, 0 late; latch interval maxima 16.717 / 16.690 / 16.675 ms. The KVM colour check was not repeated | observed 2026-11-28 |
| Errors, dropped frames | No error line, no `TEST_ONLY` rejection of a planned frame, no device loss; 0 dropped and 0 late in every run. `hvs_underrun` debugfs 0 before and after; no DRM, vc4 or HVS kernel messages | observed 2026-11-28 |
| KVM: picture | The HDMI capture unit on HDMI-A-1 returned a 1920x1080 JPEG. Baseline: the tty1 console (fbcon, plane 83 `RG16`); during the runs: the harness pattern. Screenshots are not committed (the console text shows the board's address) | observed 2026-11-28 |
| KVM: captured colours, default run (premultiplied, plane alpha 0xffff) | Expected -> captured: background #202020 -> #2c2c2c; red #ff0000 -> #dd000f; blue #0000ff -> #1201e6; green uncovered #00ff00 -> #1dff15; overlay opaque over green #ffff00 -> #eafa15; opaque over background #ffff00 -> #eafa15; translucent over green #807f80 -> #7d7d7d; translucent over background #901090 -> #85098b | observed 2026-11-28 |
| KVM: captured colours, coverage blend, plane alpha 0x8000 | Opaque over green #80ff00 -> #84fe15; opaque over background #909010 -> #8a9320; translucent over green #40bf40 -> #4dc947; translucent over background #581858 -> #581a5d; the unblended points as in the default run | observed 2026-11-28 |
| KVM: reading | The yellow block hides the green block (z-order), the translucent half blends grey over green and purple over the background, and with plane alpha 0x8000 the blended colours move to the expected half-way values. Unblended points are off by up to 34 per channel in both captures and blended ones by up to 13, so the error is read as the capture chain (JPEG, HDMI range or matrix) rather than the blending: z-order and alpha correct on the output | inferred from the two captures |
| KVM: staleness | The first one or two screenshots after the picture changes came back stale; the record advises taking three and using the last | observed 2026-11-28 |
| State afterwards | Plane 83 back on fbcon, plane 107 off, the KVM showed the console, no process held `card1`, scratch files on the board removed | observed 2026-11-28 |

### BCM2712, unsupported combinations

From `twoplane /dev/dri/card1 --frames 120 --unsupported`, 2026-11-28; the re-run gave the same lines. The engine's view has 13 planes on the output (primary 83 and overlays 107..228).

| Combination | Capability detection | Planner | Kernel `TEST_ONLY` | Tag |
| --- | --- | --- | --- | --- |
| Format `HWL0` | refuses (no plane lists it) | `NoPlane{layer:1, OUTPUT\|FORMAT}` | not reached (dumb buffers cannot hold it) | observed 2026-11-28 |
| Modifier `AR24:I915_X_TILED` | refuses | `NoPlane{layer:1, OUTPUT\|FORMAT}` | not reached | observed 2026-11-28 |
| 14 layers on 13 planes | refuses | `NoPlane{layer:13, OUTPUT\|IN_USE}` | not reached | observed 2026-11-28 |
| 14 layers in hardware (18 planes reach the CRTC) | not tested | not tested | not tested | inferred that they might fit |
| Scale-up 2x | refuses (no plane declares scaling) | `NoPlane{layer:1, OUTPUT\|SCALING}` | **ok**, the HVS accepts it (hand-built plan, scaling up to 16 claimed) | observed 2026-11-28 |
| Scale-down 1/2 | refuses | `NoPlane{layer:1, OUTPUT\|SCALING}` | **ok** | observed 2026-11-28 |
| Rotate-180 | refuses (`transforms 0x0`) | `NoPlane{layer:1, OUTPUT\|TRANSFORM}` | not reached (composition-drm does not program `rotation`; the kernel offers rotate-180) | observed 2026-11-28 |
| Plane alpha, coverage blend, off-screen, odd size or offset | accepts | plan (off-screen clipped by the planner) | ok | observed 2026-11-28 |

Capability detection and the planner agreed on every line; both were stricter than the kernel for scaling and rotation. No combination was refused by `TEST_ONLY` alone.

### BCM2712, read from source

`raspberrypi/linux` branch `rpi-6.18.y`, commit `16ae2649dcfa`, `drivers/gpu/drm/vc4/`, read 2026-11-28 (`docs/observed/vc4-hvs-source.md`). The board runs `6.18.50+rpt`; which branch it was built from is not known. Some rows there were not re-grepped. Not run.

| Item | Finding | Basis |
| --- | --- | --- |
| Generation | HVS6, `VC4_GEN_6_C` / `VC4_GEN_6_D` (D0 detected by `SCALER6_VERSION == 0x54`) | source |
| CRTCs, channels | 2 pixelvalves on the display path (HDMI0 on channel 0, HDMI1 on channel 1), 3 HVS channels. Writeback blocks `mop` (channel 2) and `moplet` (channel 1); HDMI1 and `moplet` cannot be active together | source |
| Planes | 1 primary and 1 cursor per CRTC, 16 overlays shared by the non-TXP CRTCs, 32 overlays for the writeback CRTC | source |
| zpos | Primary immutable 0, overlays 1..17 mutable, cursor 17, TXP overlays 1..33; planes written into the display list in increasing normalized zpos | source |
| Alpha, blend | Plane alpha written as `alpha >> 4`; blend None, Pre-multiplied, Coverage; stepping D0 changes the encoding, not the behaviour towards userspace | source; "same towards userspace" inferred |
| Formats | The BCM2711 list plus P010, P012, P016, S010, S012, S016 (gen 6 only) | source |
| Modifiers | Advertised: linear, T-tiled, SAND64 / 128 / 256. `vc4_format_mod_supported` rejects T-tiled and SAND64 on gen 6, so usable are linear, SAND128, SAND256 | source |
| Scaling | Per axis NONE / PPF / TPZ; no helper limit (min 1, max `INT_MAX`); driver limit on the TPZ ratio via `SCALER_TPZ0_SCALE_MASK` (field 0x1FFFFF); no source-width check as on gen 5. LBM or UPM allocation can fail a plane's atomic check | source |
| Rotation | 0, 180, reflect-X, reflect-Y; no 90/270 | source |
| Memory | LBM 1024 units; UPM 262144 bytes, 32 handles, one per colour plane; the display-list allocator spans 4064 words on every generation, against the `SCALER6_CXM_SIZE` register (a quirk; intent undetermined) | source; "quirk" inferred |
| Bandwidth | The load tracker is enabled for gen 4 only and gen 6 never applies the computed core clock: no software bandwidth check at commit | source |
| Mode size, COB | `mode_config` max 8192 x 8192; COB line width 3840 | source |
| HDMI pixel clock | 600 MHz for both HDMI blocks, from a summary of `vc4_hdmi.c`, not verified | inferred (unverified) |
| Device tree | Enablement of TXP, DSI, DPI, VEC and HDMI1 on the Pi 5 was not read | not read |

### BCM2712, inferred

- Two hardware planes on one output work, as the harness showed. Whether more (up to 18 reach this CRTC) work together depends on display-list, LBM and UPM space, which only `TEST_ONLY` shows; from the source rows, a load the HVS cannot sustain fails at run time (underrun), not in `TEST_ONLY`. inferred
- `--swap-z` could not swap because the primary's zpos is immutable; two overlays (mutable zpos) could, but the harness always puts the background on the primary. The record proposes a harness change; it is not applied. inferred
- No `DriverProfile` is warranted yet: the observations show only that `TEST_ONLY` accepts 2x and 1/2 scaling, and no limit was measured. inferred

## Broadcom BCM2711 (VC4 HVS, CM4 / reTerminal)

Seeed reTerminal with a Raspberry Pi Compute Module 4 Rev 1.0 (BCM2711, HVS5 / `VC4_GEN_5`), driven over ssh. Records: `docs/observed/bcm2711-cm4-topology.md`, `docs/observed/bcm2711-cm4.md`; source: `docs/observed/vc4-hvs-source.md`. Nobody held DRM master (fbcon only), so nothing was stopped. None of this is a claim toward M1 to M3.

**No KVM exists for this board and nothing captures its DSI panel, so z-order and alpha on the glass are not verified.** The evidence is the harness's event accounting, the kernel's `TEST_ONLY` answers, and the driver's atomic state and display list read from debugfs.

### BCM2711, observed

| Item | Finding | Basis |
| --- | --- | --- |
| Kernel, driver | `6.12.47+rpt-rpi-v8 #1 SMP PREEMPT Debian 1:6.12.47-1+rpt1 (2025-09-16) aarch64`, Raspberry Pi OS trixie, 64-bit userland. `/dev/dri/card0`, `vc4 version 0.0.0 date 0 desc Broadcom VC4 graphics`; `card1` is `v3d` (render only) | observed 2026-11-28 |
| Output, mode | `DSI-1` (built-in panel) is the only connected connector; HDMI-A-1 and HDMI-A-2 disconnected. CRTC 90 = `pixelvalve-1`, HVS channel 0. 720x1280 @ 60.000 Hz (clock 62712 kHz, htotal 780, vtotal 1340), portrait | observed 2026-11-28 |
| CRTCs | 6: `txp` (59), `pixelvalve-0` (78), `pixelvalve-1` (90, active), `pixelvalve-2` (102), `pixelvalve-4` (114), `pixelvalve-3` (126) | observed 2026-11-28 |
| Planes in the kernel's atomic state | 60 (`plane-0`..`plane-59`, ids 48 to 710). By position and `normalized-zpos`: plane-0..5 primaries (zpos 0), plane-6..21 the 16 shared overlays (zpos 1..16), plane-22..53 the 32 writeback overlays, plane-54..59 zpos 17 | observed 2026-11-28 (ids, order, zpos values) |
| Types of plane-54..59 | One per CRTC, matching the source's cursor planes; types not read | inferred |
| Plane-to-CRTC masks | As probe sees them for output 0: primary 79 `0b1`; primaries 48, 67, 91, 103, 115 `0b0` (own, unconnected CRTCs); every reported overlay `0b1`. `0b0` means "not on a connected output", not an empty `possible_crtcs`. The raw `possible_crtcs` was not dumped | observed 2026-11-28 (probe view) |
| What probe shows | 16 of the 60 planes: the 6 primaries and the first 10 overlays (127, 138, ... 226). Usable on output 0 in the engine's view: 11 (primary 79 plus 10 overlays). In the kernel that CRTC has a primary and 16 overlays (and a cursor, inferred). Cause: `MAX_PLANES` = 16 applied after sorting primaries first. Do not read 16 or 11 as the hardware's plane count | observed 2026-11-28 (counts); cause read from this repository's code |
| Formats | Same 32 on every reported plane: XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12. 32 equals `MAX_FORMATS`, so a longer list would have been cut | observed 2026-11-28 (list); that it was cut is inferred |
| Modifiers | Not printed by probe or the harness; every buffer used was linear (`modifier=0x0` in debugfs). The `IN_FORMATS` modifiers were not read | observed 2026-11-28 |
| zpos | Primaries fixed 0; overlays mutable 1..=17 (probe's reading; the raw property range was not dumped) | observed 2026-11-28 |
| Alpha, blend | Plane alpha true and blend none / premultiplied / coverage on every reported plane | observed 2026-11-28 |
| Rotation | debugfs shows `rotation=1` (rotate-0) on all 60 planes; the supported mask was not read. composition-drm reports `transforms 0x0` | observed 2026-11-28 (current value only) |
| Scaling | probe prints "scaling not claimed" (the engine's assumption, not a read property). `TEST_ONLY` accepted a scale-up and a scale-down (below); limits not measured | observed 2026-11-28 (`TEST_ONLY`) |
| HVS state | Core clock 71428000 Hz, channel 0 in use, channels 1 and 2 idle | observed 2026-11-28 |
| Plane assignment of the two-plane harness | Layer 0 (background, XR24 720x1280) -> primary 79 zpos 0; layer 1 (overlay, AR24 144x640 at 360,320) -> overlay 127 zpos 1; 3 buffers per layer; `PROFILE Generic`. Debugfs `state` during the run: planes 79 and 127 on `pixelvalve-1`, fbs allocated by `twoplane`, `normalized-zpos` 0 and 1, `plane_mask=44` (bits 2 and 6). `hvs_dlists` channel 0: two plane entries (720x1280 at 0,0, then 144x640 at 360,320) and the end word | observed 2026-11-28 |
| Verdict, default run | `VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver vc4 mode 720x1280@60.000Hz`. 0 dropped, latch interval mean 16.734 ms, max 16.820 ms; `RELEASE after the run: released 1200 of 1200 buffer uses, 0 still on screen` | observed 2026-11-28 |
| Verdict, `--swap-z` | `VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver vc4 mode 720x1280@60.000Hz`, after `SWAPZ skipped: a plane of the plan has a fixed zpos`; max interval 18.249 ms | observed 2026-11-28 |
| Verdict, `--frames 300 --buffers 2 --blend coverage --plane-alpha 32768` | `VERDICT twoplane: PASS 300 frames on two planes, 300 latched, 598 released, 0 late, driver vc4 mode 720x1280@60.000Hz`. Max interval 16.791 ms | observed 2026-11-28 |
| Verdict, `--frames 0 --unsupported` | `VERDICT twoplane: PASS no frames requested, nothing presented, driver vc4 mode 720x1280@60.000Hz` | observed 2026-11-28 |
| Re-run from commit 112648f | probe output identical; default run 600/600/1198, 0 dropped, 0 late, max interval 16.806 ms; `--swap-z` max 20.420 ms; coverage run max 16.850 ms; the `UNSUPPORTED` block word for word the same | observed 2026-11-28 |
| Errors, dropped frames | Every frame passed validation and the atomic `TEST_ONLY` and was committed; no error line; no new kernel messages; `hvs_underrun` 0 before, between and after all runs (that says nothing about load beyond two planes on an idle scanout) | observed 2026-11-28 |
| KVM | None exists for this board; nothing captures the panel and nobody looked at it. The `EXPECT` lines were not checked | observed 2026-11-28 (the absence) |
| Z-order as programmed | The overlay is above the background in the display list (planes are written in increasing zpos, per the source rows) | inferred from the observed display list and the source |
| State afterwards | Plane 79 back on fbcon, plane 127 off, card0 held by nobody | observed 2026-11-28 |

### BCM2711, unsupported combinations

From `twoplane /dev/dri/card0 --frames 0 --unsupported`, 2026-11-28 (the re-run on 112648f was identical). The engine's view has 11 planes on the output.

| Combination | Capability detection | Planner | Kernel `TEST_ONLY` | Tag |
| --- | --- | --- | --- | --- |
| Format `HWL0` | refuses | `NoPlane{layer:1, OUTPUT\|FORMAT}` | not reached | observed 2026-11-28 |
| Modifier `AR24:I915_X_TILED` | refuses | `NoPlane{layer:1, OUTPUT\|FORMAT}` | not reached | observed 2026-11-28 |
| 12 layers on 11 planes | refuses | `NoPlane{layer:11, OUTPUT\|IN_USE}` | not reached; 11 is the truncated view, the kernel gives this CRTC a primary and 16 overlays | observed 2026-11-28 |
| Scale-up | refuses (no plane declares scaling) | `NoPlane{layer:1, OUTPUT\|SCALING}` | **ok** (hand-built plan, scaling up to 16 claimed) | observed 2026-11-28 |
| Scale-down | refuses | `NoPlane{layer:1, OUTPUT\|SCALING}` | **ok** | observed 2026-11-28 |
| Rotate-180 | refuses (`transforms 0x0`) | `NoPlane{layer:1, OUTPUT\|TRANSFORM}` | not reached (rotation not programmed; the source says vc4 has 0 / 180 / reflect) | observed 2026-11-28 |
| Plane alpha, coverage blend, off-screen, odd geometry | accepts | plan | ok | observed 2026-11-28 |

Capability detection and the planner agreed on every line; both are stricter than the kernel for scaling and rotation. No combination was refused by `TEST_ONLY` alone.

### BCM2711, read from source

Same tree and commit as the BCM2712 source table (`rpi-6.18.y`, `16ae2649dcfa`, read 2026-11-28). The board runs `6.12.47+rpt`, so details such as vertical decimation (new in 6.18) may differ. Not run.

| Item | Finding | Basis |
| --- | --- | --- |
| Generation | HVS5, `VC4_GEN_5`, compatible `brcm,bcm2711-vc5` | source |
| CRTCs, channels | 5 pixelvalves but 3 HVS channels, so at most 3 active CRTCs. pv0: channel 0 only; pv3 (VEC): channel 1 only; pv1, pv2, pv4: any of 0..2; writeback (TXP): channel 2 only. Greedy assignment in `vc4_pv_muxing_atomic_check` | source |
| Planes | 1 primary and 1 cursor per CRTC; 16 overlays shared by all non-TXP CRTCs (one allocation, so a pool, not 16 per CRTC); 32 overlays for the writeback CRTC. The code calls the counts arbitrary; the real limit is display-list space | source; "pool" reading inferred |
| zpos | Primary immutable 0, overlays 1..17 mutable, cursor 17 (on top), TXP overlays 1..33 | source |
| Alpha, blend | Plane alpha as `alpha >> 4`; blend None, Pre-multiplied, Coverage | source |
| Formats | Common list plus P030 and the 10-bit RGB formats (gen 5 and later) | source |
| Modifiers | Advertised: linear, T-tiled, SAND64 / 128 / 256. Per format: the 8 RGB formats XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 take linear or T-tiled; NV12 / NV21 linear or SAND; P030 SAND128 only; others linear only | source |
| Scaling | Per axis NONE / PPF / TPZ; downscale up to 1.5 uses PPF, beyond that TPZ; no helper limit; driver limit on the TPZ ratio via `SCALER_TPZ0_SCALE_MASK` (0x1FFFFF); vertical decimation above 3x (6.18 only); source width up to 8191 on gen 5 | source |
| Memory | LBM 60 x 1K words, 64 handles, needed only for planes with vertical scaling; display list 4064 words shared by all channels, freed lazily | source |
| Rotation | 0, 180, reflect-X, reflect-Y; no 90/270 | source |
| Bandwidth | The load tracker is enabled for gen 4 only: on BCM2711 the load feeds the core clock request and no commit is rejected for bandwidth | source |
| Mode size | `mode_config` max 7680 x 7680; more than one connector per CRTC is `-EINVAL` | source |
| Device tree | Which pixelvalves, TXP and outputs the CM4 / reTerminal tree enables was not read | not read |

### BCM2711, inferred

- Two real planes on one output exist (primary plus any shared overlay), as the harness showed. A shared-pool resource group like `Imx95Dpu`'s would matter only with two active outputs, which this board cannot show (HDMI unplugged); the limiting resource would be display-list words and LBM, visible only to `TEST_ONLY`. inferred
- A z-swap check on this driver needs two overlays because the primary's zpos is immutable; the harness always skips it. inferred
- No `DriverProfile` is warranted yet: only `TEST_ONLY` acceptance of one up and one down factor was observed. inferred

## STM32MP257 (LTDC)

STM32MP257F-DK (STMicroelectronics LTDC), driven over ssh. Records: `docs/observed/stm32mp257-topology.md` (2026-11-28), `docs/observed/stm32mp257.md` (first session 2026-11-27, re-run 2026-11-28), sessions A and B in `docs/stm32mp257-ltdc-session-2026-11-28.md` (the runs are dated 2026-11-27 inside it), session C in `docs/stm32mp257-ltdc-session-c-2026-11-28.md` (the KVM gate and the picture items, none run) and `docs/stm32mp257-ltdc-session-c-outputs-2026-11-28.md` (Q8 other outputs, observed; Q4 scaler layers, from source), and the scenario run in `docs/stm32mp257-board-scenarios.md` (rows A1 to A5, observed 2026-11-28). Source: `docs/observed/stm32mp257-source.md`, `docs/stm32mp257-ltdc-transform-source.md` and `docs/stm32mp257-ltdc-yuv-source.md`. The design with its open questions is `docs/design/stm32mp257-backend.md` (§7). The `Stm32Ltdc` notes in `crates/composition-drm/src/profile.rs` carry the later scaling measurement. The board's clock was not NTP-synchronised, so the `DATE` lines the harness printed on the board do not give the run date. **All of this is portability evidence. None of it counts toward M1, M2 or M3.**

**No picture was captured in any session, including session C: the KVM could not be used (its MCP endpoint failed with HTTP 404 in every session, last tried 2026-11-28), so z-order, alpha and the screen colours are not verified on the physical output.** These items are therefore **not observed**: Q1 pixel colours (default run, both `--swap-z` phases, `--plane-alpha 32768`), Q6 transforms in a picture (planes 34, 42 and 49), NV12 colours on plane 42 (with `COLOR_ENCODING` and `COLOR_RANGE`), and plane-34 scaling. Session C stopped at its gate and ran none of them: no binary was copied and Weston was not stopped. Wherever a row below speaks about those items, it is `read from source` or `inferred`, never observed.

### STM32MP257, observed

| Item | Finding | Basis |
| --- | --- | --- |
| Kernel, driver | `6.6.129 #1 SMP PREEMPT Thu Mar 5 15:03:43 UTC 2026 aarch64`, ST OpenSTLinux Weston image `5.0.17-openstlinux-6.6-yocto-scarthgap-mpu-v26.06.10`. `/dev/dri/card0`, the only card (`48010000.display-controller`); driver `stm` version 1.0.0 date 20170330, "STMicroelectronics SoC DRM" | observed 2026-11-27 |
| Output, mode | `HDMI-A-1` connected, the capture unit as sink. 1920x1080 @ 60.000 Hz, 148.5 MHz, htotal 2200, vtotal 1125, +hsync +vsync. Other modes: 1920x1080 59.94 / 50, 1280x720 60 / 59.94 / 50, 800x600 75, 720x400 70.08 | observed 2026-11-27 (also 2026-11-28) |
| Display client | Weston held the CRTC (XR24 1920x1080 linear on plane 34), planes 42 and 49 off. Weston was stopped for the `twoplane` runs and started again afterwards (`weston-graphical-session` and `seatd-weston` `active`, primary framebuffer allocated by weston) | observed 2026-11-27 |
| CRTCs | 1 (id 41). Properties `VRR_ENABLED`, `GAMMA_LUT`, `GAMMA_LUT_SIZE` 256. No `CTM`, `DEGAMMA_LUT` or background colour | observed 2026-11-27 |
| Planes, masks | 3: 34 Primary, 42 Overlay, 49 Overlay; `possible_crtcs` `0x1` on all. No cursor plane, no writeback connector | observed 2026-11-27 |
| Formats | Plane 34: 21 (AR24 XR24 AB24 XB24 RA24 RX24 BA24 BX24 RG16 BG16 RG24 BG24 AR15 XR15 AR12 XR12 C8 YUYV YVYU UYVY VYUY). Plane 42: those plus NV12 NV21 YU12 YV12 (25). Plane 49: RGB only, 17 (up to C8). Probe prints C8 as `C8??` (not mapped by the engine) | observed 2026-11-27 |
| Modifiers | `IN_FORMATS` lists `LINEAR(0x0)` only, for every format on every plane | observed 2026-11-27 |
| zpos | Mutable range 0..2 on all three planes; defaults 0, 1, 2 | observed 2026-11-27 |
| Alpha, blend | `alpha` 0..65535 (default 65535) on all planes. **No `pixel blend mode` property**; composition-drm reports `blend [premultiplied]` as its default when the property is absent | observed 2026-11-27 |
| Whether LTDC blends premultiplied | The report was the engine's default, not read from hardware; the two-plane run was meant to check it on the KVM, which gave no picture. Sessions A and B then measured it through the output CRC: see the Q2 row below (straight alpha). `crates/composition-drm/src/profile.rs` now sets `BlendMode::Coverage` on the non-cursor planes of `Stm32Ltdc` (read from source) | inferred (unverified on a picture); superseded by the Q2 row |
| Rotation | Plane 34: rotate-0 / 90 / 180 / 270, reflect-x, reflect-y. Planes 42, 49: rotate-0, reflect-x, reflect-y. composition-drm reports and programs no transform (`transforms 0x0`) | observed 2026-11-27 |
| Colour properties | `COLOR_ENCODING` (BT.601, BT.709) and `COLOR_RANGE` on planes 34 and 42 only | observed 2026-11-27 |
| Scaling, harness run | No KMS scaling property. At 2x, `TEST_ONLY` accepted upscale and rejected downscale (all 6 candidates). 2x was the only factor in the harness run, and `TEST_ONLY` the only check | observed 2026-11-27 |
| Scaling, later measurement | Per the `Stm32Ltdc` notes in `profile.rs` (commits 3041769, 99d965d, 7966b89, `scale_probe`; the commit bodies are empty, so this follows `profile.rs`, not a raw record): downscale 0.5, 0.75 and 0.9 rejected (EINVAL) on all three planes; no upscale rejected on planes 34 and 42 up to the factor that shrinks the source to one pixel (1920 horizontally, 1080 vertically and on both axes), so 1080 is used as `max_upscale`; plane 49 accepts only 1.0. Quality, bandwidth and underrun at large factors, and factors needing a sub-pixel source, were not tried | observed 2026-11-27 (per `profile.rs`; `TEST_ONLY` only) |
| Which layers scale | Planes 34 and 42 scale, plane 49 does not. That this holds for layers 0 and 1 of the SoC in general is concluded from the one board | observed 2026-11-27 (this board); general reading inferred |
| Non-full-screen primary | The primary accepted a 384x540 window at 960,270 (second half of `--swap-z`) | observed 2026-11-27 |
| What probe shows | All three planes and one output; formats as raw. Probe omits rotation, colour properties and the CRTC properties | observed 2026-11-28 |
| Plane assignment of the two-plane harness | Layer 0 (background, XR24 1920x1080) -> primary 34 zpos 0; layer 1 (overlay, AR24 384x540 at 960,270) -> overlay 42 zpos 1; plane 49 off; `PROFILE Generic`; 3 buffers per layer. Debugfs state 4 s into the run: plane 34 `normalized-zpos` 0 and plane 42 `normalized-zpos` 1, fbs allocated by `twoplane`, 58 fps user updates each, `plane_mask=3` | observed 2026-11-27 |
| Verdict, default run | `VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver stm mode 1920x1080@60.000Hz`. 0 dropped, latch interval mean 16.667 ms, max 17.446 ms; `RELEASE after the run: released 1200 of 1200 buffer uses, 0 still on screen` | observed 2026-11-27 |
| Verdict, `--swap-z` | `VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver stm mode 1920x1080@60.000Hz`, with `SWAPZ at frame 300: overlay now below the background`; max interval 17.421 ms. The planner swapped by moving layers between planes (overlay onto plane 34, background onto plane 42); each plane's zpos stayed 0 and 1 | observed 2026-11-27 |
| Whether the swap reprogrammed `zpos` | It did not: concluded from the plan lines and the `normalized-zpos` values | inferred |
| Verdict, `--plane-alpha 32768 --frames 120` | `VERDICT twoplane: PASS 120 frames on two planes, 120 latched, 238 released, 0 late, driver stm mode 1920x1080@60.000Hz` (commit only, no picture) | observed 2026-11-27 |
| Verdict, `--blend coverage --frames 60` and `--blend none --frames 60` | `ERROR frame 0: validate: Plan(NoPlane { layer: 1, reasons: BLEND })` and `VERDICT twoplane: FAIL frame 0: validate: Plan(NoPlane { layer: 1, reasons: BLEND }) (1 error(s)), driver stm mode 1920x1080@60.000Hz`. The planner refused a blend mode the caps do not offer; not a driver failure | observed 2026-11-27 |
| Verdict, `--frames 0 --unsupported` | `VERDICT twoplane: PASS no frames requested, nothing presented, driver stm mode 1920x1080@60.000Hz` | observed 2026-11-27 |
| Re-run from commit 112648f | `probe`, default, `--swap-z`, `--plane-alpha 32768 --frames 120`, `--blend coverage`, `--unsupported` and `--unsupported --scaling 8,2` gave the same plans, verdicts and `UNSUPPORTED` lines; 0 dropped, 0 late; latch interval maxima 17.970 ms (default) and 17.411 ms (swap); LTDC error and underrun counters stayed 0 | observed 2026-11-28 |
| Errors, dropped frames | 0 dropped and 0 late in the committed runs; `transfer_error`, `fifo_underrun_error` and `fifo_underrun_warning` 0 during and after; no DRM or LTDC kernel message after the boot line `[drm] Initialized stm 1.0.0 20170330 for 48010000.display-controller on minor 0` | observed 2026-11-27 |
| KVM | No screenshot in sessions A and B, and none in session C (gate: HTTP 404 at 01:37 and 01:51 host UTC, 2026-11-28, so Q1, Q6, NV12 colours and plane-34 scaling did not run). In the first two sessions: the KVM server failed to connect (HTTP 404 from its relay). Z-order, alpha and colours on screen are unverified. The board saw the capture unit as a 1920x1080 sink (EDID name "NanoKVM"), and Weston scanned out an active 1920x1080 XR24 buffer; what was on screen is not known | observed 2026-11-27 and 2026-11-28 (the failure and the sink); picture content unverified |
| Evidence held instead | The kernel's atomic state during the runs, the harness's event accounting, and the `EXPECT` lines for a later capture comparison | observed 2026-11-27 |
| Q2, straight or premultiplied alpha (sessions A and B) | Straight (non-premultiplied) alpha on plane 42 (AR24): `out = Cs*a + Cd*(1-a)`, `a` = pixel alpha x plane alpha, 8-bit with truncated products. Six CRC measurements fit it; the four that can tell the modes apart differ from the premultiplied value by 0x20 to 0x70. Through the output CRC, not a picture. Planes 34 and 49 were not tested | observed 2026-11-27 (CRC); the exact formula and the other planes inferred |
| Q3, scaling quality and underrun | Scaled plane-42 pictures gave a stable CRC that matched none of five nearest-neighbour or bilinear references (the 1:1 case matched all), so the scaler filters with an unknown kernel. `fifo_underrun_warning` grew only while a scaled layer was shown (about 70 to 220 per second at 1.5x to 4x), 0 for 1:1; `fifo_underrun_error` and `transfer_error` stayed 0. The h/v phase question stays open. Each commit with `ALLOW_MODESET` took 746 to 801 ms, cause not determined | observed 2026-11-27 |
| Q5, writing zpos | Crossed zpos on the fixed planes passes `TEST_ONLY` and commits, and the hardware follows it (primary at zpos 2 over an overlay at 0 gives the primary-alone CRC; with two overlays crossed the upper hides the lower). The kernel normalizes over the enabled planes only (2 and 0 became 1 and 0 with plane 49 off) | observed 2026-11-27 (CRC and debugfs) |
| Q6, transforms (CRC and `TEST_ONLY` only) | Overlay 42 accepts rotate-0 with reflect-x, reflect-y or both, and refuses rotate-180/90/270 (EINVAL). Primary 34 commits rotate-180 and the reflections; rotate-90 and rotate-270 pass `TEST_ONLY` and were **not committed** (unsafe, see the inferred list). Primary reflect-x and rotate-180, and overlay reflect-x+reflect-y, gave a CRC that changed from frame to frame; overlay reflect-x alone and every reflect-y run were stable; underrun counters stayed 0. No commit failed after a passing `TEST_ONLY`, so whether such a failure stops the display is not observed. Plane 49 transforms were never tried. **No picture, so what the transforms look like is not observed** | observed 2026-11-27 (CRC, `TEST_ONLY`); picture not observed |
| Q7, underrun under load | With CPU and memory load (load average 3.6 to 7.3) no run raised `fifo_underrun_error` or `transfer_error`, including three planes with a 4x-scaled middle plane and an AR24 top plane. Late frames (3 to 16 per 1800) happened with and without scaling. Buffers are limited by the 128 MiB CMA (`--buffers 16` failed with ENOMEM, 11 per plane worked). `TEST_ONLY` cannot predict the warnings | observed 2026-11-27 |
| Q9, NV12 and C8 | An NV12 overlay on plane 42 at 1:1 over an XR24 primary passes `TEST_ONLY` and commits; its CRC (`0x8d0f`) differs from the primary alone (`0x2fab`), so it is scanned out. NV12 on plane 49 and scaled NV12 on plane 42 are refused (EINVAL). C8 is listed on all three planes and was not tried. **Whether the NV12 colours are right is not observed** (the CRC cannot tell rounding from wrong colours) | observed 2026-11-27 (CRC); colours not observed |
| Q8, other outputs (session C) | One connector `HDMI-A-1` (id 32), one encoder (id 31) with a two-bridge chain (DSI host `dsi@48000000`, then the ADV7535 at `hdmi@3d`), one CRTC (41); no other connector in `/sys/class/drm` or the debugfs state. Device tree: LTDC, DSI and ADV7535 `okay`; `lvds@48060000`, `panel-lvds` and its backlight `disabled`. The LTDC `port` has endpoints for DSI and LVDS, so the LVDS route exists in the tree and is off; it cannot be enabled without a device-tree change. Display modules loaded: `stm_drm`, `dw_mipi_dsi_stm`, `dw_mipi_dsi`, `adv7511`, `drm_dma_helper`, `galcore`; no LVDS or panel module. Read only, Weston left running (`active` twice before and after) | observed 2026-11-28 (host UTC 01:04Z to 01:06Z) |
| Q8, `possible_crtcs` of the encoder | Not shown by sysfs or debugfs; the single CRTC and `encoder_mask=1` imply `0x1`. `modetest -e` was not run | inferred |
| Q4, firewall (RIF) properties | The LTDC node has children `l1l2`, `l3`, `rot` (RIFSC ids 119, 120, 121) and `cmn` (80). Three planes exist, so the `l3` access was granted and the layer count was not reduced | observed 2026-11-28 (properties), observed 2026-11-27 (three planes) |
| Scenario run, A1 to A4 | `scripts/board-scenarios.sh` over `tools/drm-harness` (commit eab4162), hold 30 s: A1 startup PASS (1 commit, 1 latched, planes 34 and 42, also over a kernel lease); A2 steady state PASS (1800 commits, 1800 latched, 0 dropped, 3598 released, 1 missed vblank not judged by the script); A3 stop-restart PASS (SIGTERM gave exit 143 and SIGKILL 137, each restart latched again); A4 lease loss PASS (60 frames, then `DeviceLost` once after 7889 us, buffers released). Log verdicts only | observed 2026-11-28 |
| Scenario run, A5 | **BLOCKED**: the harness printed `connector=HDMIA-1` (the Rust `Debug` name of the interface) while the kernel's sysfs name is `card0-HDMI-A-1`, so the script found no `modes` file. The sink lists four modes, so the row was blocked for the wrong reason. No modeset was attempted and no PASS is claimed. The bug was reported, not fixed, in that run | observed 2026-11-28 |
| Scenario run, A6 | There is no A6 row in that run: the runner had none | observed 2026-11-28 |
| Scenario run, pictures | Every KVM picture verdict of A1 to A5 is not observed; the hook only logged | observed 2026-11-28 |
| Scenario run, state after | Weston and seatd started again, `systemctl is-active` printed `active` twice (and again after ten seconds); `/tmp` files removed | observed 2026-11-28 |

### STM32MP257, unsupported combinations

From `twoplane /dev/dri/card0 --frames 0 --unsupported`, 2026-11-27 (the re-run on 2026-11-28 matched). Three planes on the output.

| Combination | Capability detection | Planner | Kernel `TEST_ONLY` | Tag |
| --- | --- | --- | --- | --- |
| Format `HWL0` | refuses | `NoPlane{layer:1, FORMAT}` | not reached | observed 2026-11-27 |
| Modifier `AR24:I915_X_TILED` | refuses (linear only) | `NoPlane{layer:1, FORMAT}` | not reached | observed 2026-11-27 |
| 4 layers on 3 planes | refuses | `NoPlane{layer:3, IN_USE}` | not reached | observed 2026-11-27 |
| 2x upscale | refuses (no scaling declared); accepts with `--scaling 8,1` or `8,2` | `NoPlane{layer:1, SCALING}` by default; a plan when scaling is claimed | **ok** | observed 2026-11-27 (2x only) |
| 2x downscale | refuses; with `--scaling 8,1` refuses ("declared limits are below a factor of 2"); with `--scaling 8,2` accepts | `NoPlane{layer:1, SCALING}` by default; a plan with `--scaling 8,2` | **rejected** (EINVAL / ERANGE / ENOSPC, 6 candidates) in every case; once the caps claim it, only `TEST_ONLY` catches it | observed 2026-11-27 |
| Rotate-180 | refuses (`transforms 0x0`, though plane 34 lists rotate-180) | `NoPlane{layer:1, TRANSFORM}` | not reached | observed 2026-11-27 |
| Plane alpha 0x8000 | accepts | plan | ok | observed 2026-11-27 |
| Coverage blend | refuses (no blend property) | `NoPlane{layer:1, BLEND}` | not reached | observed 2026-11-27 |
| Off-screen, odd size and offset | accepts | plan (clipped) | ok | observed 2026-11-27 |

### STM32MP257, read from source

`STMicroelectronics/linux` branch `v6.18-stm32mp`, commit `3056405e0479`, `drivers/gpu/drm/stm/ltdc.c` read in full on 2026-11-28 (`docs/observed/stm32mp257-source.md`, which has the line numbers). The board runs `6.6.129`. `ltdc.h`, `drv.c`, the device-tree files, the binding and mainline were not read. Not run.

| Item | Finding | Basis |
| --- | --- | --- |
| Capabilities | Chosen by the hardware version register; the 4.x case (`HWVER_40100` / `40101`) sets `dynamic_zorder`, `plane_rotation`, the scaler probe and the FIFO threshold. That the MP25 reports one of these was not confirmed | source; MP25 mapping inferred |
| CRTC, encoder | 1 CRTC (`NB_CRTC`), one encoder of type `DRM_MODE_ENCODER_DPI` with a bridge | source |
| Layers | Count read from `LTDC_LCR`, clamped to `1..LTDC_MAX_LAYER` (value in `ltdc.h`, not read); a layer behind a denied firewall is not created. Primary is layer 0, the rest are overlays, no cursor plane; `possible_crtcs` 0x1 for all | source |
| Formats | RGB: ARGB8888, XRGB8888, ABGR8888, XBGR8888, RGBA8888, RGBX8888, BGRA8888, BGRX8888, RGB565, BGR565, RGB888, BGR888, ARGB1555, XRGB1555, ARGB4444, XRGB4444, C8. Interleaved YUV where the layer supports it. NV12, NV21, YUV420, YVU420 only on layers with nonzero index on MP251 / MP255 | source |
| Modifiers | `DRM_FORMAT_MOD_LINEAR` only | source |
| zpos | Mutable when `dynamic_zorder`, range `0 .. nb_layers-1`, initial value the layer index | source |
| Alpha, blend | `alpha` on every plane; no call that creates a blend-mode property | source |
| Scaling | Per layer (`LXCR_C1R_SCA`); upscale only ("Downscaling is not supported"); a layer without the scaler rejects any size mismatch; a YUV format with scaling is rejected; the factor has 12 fractional bits masked to 16 bits and no numeric cap in the code. Which layers have the scaler is a register fact, not in the source. The horizontal and vertical phase registers look cross-assigned in `ltdc_plane_update` (unverified, possibly a driver bug) | source; the register reading unverified |
| Rotation | `ROTATE_0`, `REFLECT_X`, `REFLECT_Y` on every layer; 90 / 180 / 270 on the primary only, and only with `crtc_rotation` (needs `GC2R_ROTA` and a `rotation-memory` region in the device tree; rotator registers reachable on MP255 only) | source |
| Rotation constraints | With 90 / 270 no other layer may be used; reserved buffer of at least `hdisplay*vdisplay*2*3` bytes; `vdisplay` above 1366 refused. These refusals sit in `ltdc_plane_update`, after `GCR_LTDCEN` was cleared, so the display stays stopped on that path | source |
| Bandwidth, underrun | No bandwidth accounting at check time; FIFO underrun is found by interrupts and logged ("ltdc fifo underrun: please verify display mode"); `FUT_DFT` 128; burst length from the device tree (`st,burstlen`) | source |
| Pixel clock | `pad_max_freq_hz` from match data (value not read); 5 permille clock tolerance for non-preferred modes | source (limit value open) |
| Which layers scale (Q4, session C) | No file names them: at probe `ltdc_get_caps()` reads `LTDC_L1C1R + i*LAY_OFS` per layer and sets `plane_scaling[i]` from `LXCR_C1R_SCA` (bit 31), the same in `v6.18-stm32mp` and `v6.6-stm32mp`. No per-variant table. MP251 and MP253 share one LTDC node and have no DSI, LVDS or GPU nodes; MP255 and MP257 add them, with the same scaler code. A refused `l3` firewall access removes layer 3 and the CRC but does not change which layers scale. Read from `docs/stm32mp257-ltdc-session-c-outputs-2026-11-28.md`; the fetch was by web agent without line numbers, and that the files are the pinned commits is inferred. The public reference manual was not read | source (`ltdc.c`, device-tree files, binding); identical silicon inferred |
| Both LTDC endpoints at once | The binding says the DSI and LVDS endpoints cannot be active together; on this image only DSI is enabled | source (binding, per `docs/stm32mp257-ltdc-source-full.md`); state observed 2026-11-28 |
| Reflect-x, reflect-y, 180 | All planes use `ltdc_plane_update`. Reflect-x sets `LXCR_HMEN` and starts at the last byte of the first line; reflect-y starts at the last line with a negative pitch; 180 is both and does not use the rotator. Atomic check has no rotation test. Overlays offer only rotate-0, reflect-x and reflect-y (`docs/stm32mp257-ltdc-transform-source.md`, `ltdc.c:1648-2042`, `2311-2314`; 6.6 tip `548f960c`) | source |
| 90 and 270 on the primary | `crtc_rotation` is cleared for a missing `rotation-memory` only after the property was created, so the primary still offers 90/180/270; that explains why session B's 90/270 passed `TEST_ONLY` (`ltdc.c:2263-2276`, `2774`, `2820-2826`). The refusals (buffer too small, `vdisplay` above 1366) are warnings plus a return with the controller off, and `ldev->rot_mem` is used without a null check (`ltdc.c:1738-1784`) | source; the null dereference on this board is inferred |
| Frame-to-frame CRC with reflect-x | The source does not explain it: the driver has no CRC region, programs one reload per commit and treats a reflected layer like any other. The unstable cases all include `HMEN`. A picture or the underrun counters would settle it | source; hypothesis inferred |
| YUV conversion | `LTDC_L1CYR0R` and `CYR1R` per layer, written for every YUV framebuffer from the plane's `color_encoding` and `color_range` (BT.601 and BT.709; limited and full; no BT.2020; defaults BT.601 limited). `LxPCR_YREN` is set exactly for limited range. NV12 uses `YCM_SP`. Scaling with a YUV format is refused. Coefficient words (for example BT.601 limited `0x02040199`, `0x006400D0`) are in `docs/stm32mp257-ltdc-yuv-source.md` (`ltdc.c` 782-800, 932-975, 2215-2224) | source |
| Board kernel against the 6.6 tip | The 6.6 plane path matches the 6.18 rows except compatible strings and minor items (`docs/stm32mp257-ltdc-source-diff-6.6.md`). The board's build date (2026-03-05) precedes the tip commit (2026-06-01), so it is probably not exactly `548f960c` | source (diff), observed 2026-11-27 (board); revision inferred |
| Not read | Mainline `ltdc.c`, the device tree, the `LTDC_MAX_LAYER` value in `ltdc.h`, `drv.c` | not read |

### STM32MP257, inferred

- Two hardware planes on one output exist because all planes share the one CRTC; the harness confirmed it on this board (planes 34 and 42). 90 / 270 rotation would exclude a second plane. inferred
- `TEST_ONLY` covers the scaling rules (in `atomic_check`). The rotation buffer-size and 1366 checks are in `ltdc_plane_update`, past `TEST_ONLY`, so `TEST_ONLY` can pass and the commit then leave the display stopped. Not run on the board. inferred
- Underrun cannot be seen before commit: a combination that passes `TEST_ONLY` can still underrun. inferred
- Predicted transform pictures, from `ltdc.c`: for the eight-bar pattern of `plane_commit`, reflect-x and rotate-180 on the primary and reflect-x on an overlay read black, blue, red, magenta, green, cyan, yellow, white; reflect-y is invisible on vertical bars. These are predictions, not pictures. inferred
- Predicted NV12 colours: with the coefficient model (scale 1/256), BT.601 limited, flat red, green and blue come out near (255,0,0), (0,255,1) and (0,0,255) within about 2 levels. The model and rounding are not in the source, and the current `COLOR_ENCODING` and `COLOR_RANGE` values on plane 42 were never recorded. inferred
- Session C changed no conclusion that depended on a picture. The Q1 prediction from the CRC result (opaque regions pass; translucent regions follow straight alpha, so a premultiplied buffer comes out too dark) is untested on a picture. inferred
- A backend needs one output and no output selection on this image: the encoder is one DSI bridge chain with one connector, and the LVDS route is disabled in the device tree. inferred
- Plane-34 scaling was measured at `TEST_ONLY` only (see the later-measurement row); no scaled picture of it exists. Which layers scale is a register value, so other boards or boot-chain settings are not covered. inferred
- Primary rotate-90 and rotate-270 must not be claimed or committed on this board: they pass `TEST_ONLY`, the refusals sit after the controller is disabled, and a missing `rotation-memory` leaves a null pointer. inferred
- A1 to A4 passing says the harness and the DRM backend behave on the LTDC driver, nothing more: A5 was blocked by a connector-name bug, A6 had no row, and no picture was judged. inferred
- The profile in `profile.rs` (upscale to 1080, no downscale, primary and first overlay) is consistent with the observed rows above; YUV scaling is not modelled and is left to `TEST_ONLY`. inferred

## NXP i.MX8M Plus (LCDIFv3)

Desk study only (SoW §5.3, "a contrasting design"). **No board run was made, so there is no observed subsection and no observed row.** Every row is `read from source` or `inferred`, and each points at `docs/imx8mp-lcdif-source.md`, which has the file and line references and the trees read (mainline Linux at `f4585067150b`; NXP `linux-imx` `lf-6.6.52-2.2.2`, of which only `lcdifv3-plane.c` was read, so vendor CRTC, DT and GPU code are unread). Nothing here is a measured or timing claim.

### i.MX8M Plus, read from source

| Item | Finding | Basis |
| --- | --- | --- |
| Instances, outputs | Three LCDIF nodes in `imx8mp.dtsi`, all disabled in the SoC file: `lcdif1` to MIPI DSI, `lcdif2` to the LDB (LVDS), `lcdif3` to the HDMI block. One endpoint each, so a CRTC drives one fixed output. Mainline binds each node as its own DRM device with one CRTC (`docs/imx8mp-lcdif-source.md` §1) | read from source |
| Planes per CRTC | Mainline registers one primary plane and no cursor or overlay plane; the driver struct has a single `primary` (§2) | read from source |
| Formats | Mainline: RGB565, RGB888, XBGR8888, XRGB1555, XRGB4444, XRGB8888, packed YUYV, YVYU, UYVY, VYUY; no alpha-channel format, no planar YUV. Vendor plane file: adds ARGB8888, ABGR8888, ARGB1555; no YUV (§3, §9) | read from source |
| Modifiers | Linear only, mainline and vendor plane file (§3, §9) | read from source |
| Scaling | Plane check demands no scaling (`DRM_PLANE_NO_SCALING`), mainline and vendor plane file. The vendor plane must also sit at CRTC position (0,0) (§4, §9) | read from source |
| Alpha, blend, zpos | Mainline creates no alpha, blend-mode or zpos property. The vendor plane file creates an immutable zpos of 0 and no alpha property (§4, §9) | read from source |
| Vblank, latch | The new address is written, then `SHADOW_LOAD_EN` is set at flush; the `VS_BLANK` interrupt counts as a vblank only once the hardware has cleared that bit. A DRM vblank event completes the flip. No fence or other frame-done register is used (§5) | read from source |
| Framebuffer memory | GEM DMA helpers, so contiguous buffers; 36-bit DMA mask; 16-bit pitch field; no IOMMU referenced by the LCDIF nodes (§6) | read from source |
| 2D blitter, PXP | The mainline PXP driver matches only i.MX6ULL and i.MX7D, and `imx8mp.dtsi` has no PXP node. The SoC file has a `gpu2d` Vivante node next to `gpu3d`; its blit abilities are not stated there (§7) | read from source |
| Not read | Vendor CRTC code, register layer and DT nodes; `gpu-viv` and g2d; which LCDIFs can run together and share pixel clocks (Open questions) | not read |

### i.MX8M Plus, inferred

- One plane per CRTC with no scaling, alpha or zpos means no overlay assignment is possible. Every layer but one must be composed into the primary buffer. inferred (§2, §4, §8)
- The primary buffer must be contiguous linear memory in one of the listed formats, so whatever composes (CPU, GPU, `gpu2d` or another processor) must write into such memory. inferred (§6, §8)
- Scan-out is latched at the vblank after flush and the old buffer is free only after the flip event; this fits the existing release-on-event model but is read from code, not run. inferred (§5)
- Whether `gpu2d` could blend for composition depends on the vendor tree, which is unread. inferred (open)

### Consequences for this project

With one plane per CRTC and no scaling, alpha or zpos, the planner has nothing to assign to an overlay: all but one layer fall back to composition into the primary frame. Route 3 of SoW §5.3, an M7 software blend into an A53-produced frame before scan-out, is pre-composition. It is not a hardware overlay and not an M1 deliverable. `docs/imx8mp-route3-precomposition.md` is a design note (inferred, not implemented) on the interface and buffer ownership it would need; it makes no timing claim and is not counted as a hardware-overlay row. A host test of the fall-back behaviour, `crates/composition-core/tests/imx8mp_single_plane.rs`, was still in review when this section was written and is not cited as evidence here.

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
- https://github.com/torvalds/linux/tree/f4585067150b85e33f60876149a6e742928da75e/drivers/gpu/drm/mxsfb (`lcdif_drv.c`, `lcdif_kms.c`, `lcdif_regs.h`) and `arch/arm64/boot/dts/freescale/imx8mp.dtsi` at that commit
- https://github.com/nxp-imx/linux-imx/blob/5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc/drivers/gpu/drm/imx/lcdifv3/lcdifv3-plane.c (tag `lf-6.6.52-2.2.2`; the only vendor file read)
