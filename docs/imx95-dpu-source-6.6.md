# i.MX95 display driver, kernel 6.6.157-7.8.0: source reading

Source reading, 2026-11-28. **Nothing here is observed on a board.** Every row is tagged `read from source (6.6.157-7.8.0, 64f85d65c112)` unless it says otherwise; **inferred** marks a conclusion the source does not state itself (for example DRM-core behaviour that is not in the driver files).

## Which kernel

- The board reports `6.6.157-7.8.0`.
- Layer `meta-toradex-nxp` (`git://git.toradex.com/meta-toradex-nxp.git`), branch `scarthgap-7.x.y`, commit `962f3217b3e4` ("linux-toradex: update to stable v6.6.157"), file `recipes-kernel/linux/linux-toradex_6.6-2.2.x.bb`: `LINUX_VERSION = "6.6.157"`, `SRCBRANCH = "toradex_6.6-2.2.x-imx"`, `SRCREV_machine = "64f85d65c112c45ceb6c7e0566885afc62b1a551"`, kernel repository `git://git.toradex.com/linux-toradex.git`.
- Kernel commit read: **`64f85d65c112c45ceb6c7e0566885afc62b1a551`**, fetched shallowly (`git fetch --depth 1`, 2026-11-28). The branch head `8ad2b7ff6b56` (6.6.161) was not read.
- That the board's BSP 7.8.0 is this recipe state is **inferred** from the version string `6.6.157`; no release tag was seen.
- Only the files named below were extracted (to the machine's dependency cache, not into this repository).
- Not read at this commit: the Verdin iMX95 / Mallow **panel overlay**. The kernel tree has no display node for Verdin/Mallow (see "Device tree").

Paths: `P` = `drivers/gpu/drm/imx/dpu95/`; `DTS` = `arch/arm64/boot/dts/freescale/`. Line numbers are those of the files at the commit above.

## Table

Tag column: `src` = `read from source (6.6.157-7.8.0, 64f85d65c112)`; `src+inf` = the same, with the stated part inferred from DRM core.

| Item | Finding at 6.6.157-7.8.0 | Where | Tag |
| --- | --- | --- | --- |
| CRTC count | 2. `#define DPU95_CRTCS 2`; the init loop runs it and `drm_vblank_init(drm, DPU95_CRTCS)`. Both CRTCs are always created; a CRTC whose bridge node is disabled gets **no encoder and no connector** (it returns 0 and skips them) | `P`dpu95-drv.h:17-21; `P`dpu95-kms.c:604-614, 629; `P`dpu95-kms.c:531-533 | src |
| Encoders / connectors | One encoder per CRTC (`DRM_MODE_ENCODER_NONE`, `possible_crtcs` = that CRTC) and a bridge connector (`drm_bridge_connector_init`), only when the CRTC's remote node is available | `P`dpu95-kms.c:543-575 | src |
| Plane count and types | 7: `DPU95_PRIMARYS` = 2 (one `DRM_PLANE_TYPE_PRIMARY` per CRTC) + `DPU95_OVERLAYS` = 5 (`DRM_PLANE_TYPE_OVERLAY`). No cursor plane | `P`dpu95-drv.h:19-20; `P`dpu95-crtc.c:677-685; `P`dpu95-kms.c:616-627 | src |
| `possible_crtcs` | Primary: created with `possible_crtcs = 0`, then tied to its CRTC by `drm_crtc_init_with_planes()`, so each primary serves only its own CRTC. Overlays: `possible_crtcs = dpu_drm->crtc_mask` (both CRTCs). One CRTC therefore sees its primary plus 5 overlays = 6 planes | `P`dpu95-crtc.c:679-680, 689-690; `P`dpu95-kms.c:619-621 | src (primary mask: inferred from DRM core) |
| Hardware behind planes | `struct dpu95_plane_res`: `fl[2]`, `fy[4]`, `lb[6]`, in one `dpu95_plane_grp` that **both CRTCs share**, with one `hs` and one `vs`, per-stream `cf[2]`, `ed[2]`. Six hardware planes in total (`DPU95_HW_PLANES`). Unit ids: FetchLayer 0,1; FetchYUV 0-3; FetchEco 0,1,2,9; LayerBlend 1-6 | `P`dpu95.h:739-758; `P`dpu95-drv.h:21; `P`dpu95-core.c:149, 182, 199, 234 | src |
| Fetch unit order | The group's list is built `fy3..fy0`, then `fl1, fl0`, each added at the list head (`list_add`), so the first-fit scan visits **FetchLayer0, FetchLayer1, FetchYUV0..3** | `P`dpu95-core.c:1001-1007; `P`dpu95-fetchunit.c:413; scan `P`dpu95-kms.c:213-246 | src |
| Fetch unit capabilities | FetchYUV: FETCHECO, HSCALER, VSCALER4, VSCALER9, PACKED_YUV422. FetchYUV on silicon rev 1.x: the same **without VSCALER4**. FetchLayer sets no `cap_mask` (allocated with `devm_kzalloc`, never assigned in the file), so its mask is 0: **it can take only an unscaled RGB layer** | `P`dpu95-fetchyuv.c:16-25, 236-237; `P`dpu95-fetchlayer.c:99 (no `cap_mask` in the file); caps `P`dpu95.h:72-76 | src (FetchLayer mask 0: inferred from absence of any assignment) |
| Layer to fetch-unit rule | Per layer: needs FETCHECO if `num_planes > 1`; HSCALER if `src_w != dst_w`; VSCALER4 if `src_h != dst_h`; PACKED_YUV422 for packed 4:2:2 (YUYV, UYVY). A unit qualifies only if `(needs & caps) == needs`. No match: `-EINVAL` | `P`dpu95-kms.c:178-183, 201-209, 223-224, 248-253 | src |
| Stage and blend assignment | Planes of a CRTC are sorted by `normalized_zpos`. Stream 0 uses `lb[i]`, stream 1 uses `lb[DPU95_HW_PLANES - (n - i)]` (from the top); the first plane's primary input is the stream's ConstFrame, later ones the previous LayerBlend. The top plane drives ExtDst | `P`dpu95-kms.c:36-67, 257-273; `P`dpu95-plane.c:564-581 | src |
| Formats | One list for all planes (13): ARGB8888, XRGB8888, ABGR8888, XBGR8888, RGBA8888, RGBX8888, BGRA8888, BGRX8888, RGB565, YUYV, UYVY, NV12, NV21 | `P`dpu95-plane.c:29-44; `drm_universal_plane_init` :602-606 | src |
| Modifiers | Linear only: `format_mod_supported` returns `modifier == DRM_FORMAT_MOD_LINEAR`; the modifier list passed to `drm_universal_plane_init` is NULL | `P`dpu95-plane.c:107-112, 121, 602-606 | src (IN_FORMATS content for NULL list: inferred from DRM core) |
| zpos | `drm_plane_create_zpos_property(plane, zpos, 0, DPU95_HW_PLANES - 1)`: **mutable, range 0..5**; reset defaults primary 0, overlay 1; `drm_atomic_normalize_zpos()` in `atomic_check` | `P`dpu95-plane.c:46-54, 72, 612-614; `P`dpu95-kms.c:459 | src |
| Plane alpha | `drm_plane_create_alpha_property()` (written to the fetch unit's constant alpha and to the LayerBlend) | `P`dpu95-plane.c:618; `P`dpu95-fetchunit.c:232-233 | src (default value 0xffff: inferred from DRM core) |
| Pixel blend modes | None (`PIXEL_NONE`), Pre-multiplied, Coverage; `PIXEL_NONE` uses the constant alpha and ignores the pixel alpha; Premultiplied and Coverage use pixel alpha too if the format has alpha. Default mode: Pre-multiplied (inferred) | `P`dpu95-plane.c:622-626; `P`dpu95-fetchunit.c:214-234; `P`dpu95-layerblend.c:228-241 | src (default: inferred) |
| Bottom plane is opaque | The plane with `normalized_zpos == 0` is blended with ZERO primary and constant alpha forced to `DRM_BLEND_ALPHA_OPAQUE`: its alpha and blend mode are **ignored**; it replaces the black ConstFrame | `P`dpu95-layerblend.c:219-227 | src |
| Color properties | Color encoding (BT601, BT709; default BT709) and range (limited, full; default limited) on every plane | `P`dpu95-plane.c:629-635, 73-74 | src |
| Scaling limits | `drm_atomic_helper_check_plane_state(..., min_scale = (1<<16)/2048, max_scale = DRM_PLANE_NO_SCALING, can_position = true, can_update_disabled = false)`: **upscale up to 2048x per axis, no downscale** | `P`dpu95-plane.c:364-367 | src+inf (meaning of min/max scale: DRM core) |
| Source size | Without scaling: source width and height each at most 8192. With scaling: each at most 2048. "Without scaling" is judged `src_w == dst_w || src_h == dst_h`, so a layer scaled on **one** axis only gets the 8192 limit. Pitch at most `0x10000` | `P`dpu95-plane.c:25-27, 178-204 (condition :185), 292, 321 | src |
| Source alignment | For 4:2:0 / 4:2:2 formats (`hsub == 2` / `vsub == 2`): source width, x (and height, y for vsub) must be even. Base address alignment per bpp (32 bpp: 4 bytes; 16 bpp: 8 bytes in the second check); pitch alignment 4 (32 bpp) / 2 (16 bpp); chroma plane address 8-aligned, pitch even | `P`dpu95-plane.c:206-236, 250-333 | src |
| Scalers: sharing | One HScaler and one VScaler (`hs`, `vs`) in the plane group, shared by both CRTCs: a second user gets `-EINVAL`. A scaler already bound to the other stream is not migrated (`-EINVAL`) | `P`dpu95-kms.c:69-133; group `P`dpu95.h:749-758; `hs`/`vs` fetched at `P`dpu95-crtc.c:673-675 | src |
| Silicon rev 1.0 / 1.1 | `soc_device_match` on `i.MX95` rev `1.0` and `1.1` leaves `use_vs4 = false`: no VScaler4 unit is initialised, `plane_grp->vs` stays NULL, FetchYUV loses `USE_VSCALER4`; **a layer with `src_h != dst_h` then finds no fetch unit and is refused**. Horizontal scaling still works. The board's SoC revision is not known from the source | `P`dpu95-core.c:35-39, 1084-1087, 876-882, 925-931; `P`dpu95-crtc.c:673-675; `P`dpu95-fetchyuv.c:22-25, 236-237; `P`dpu95-kms.c:206-207, 223-224 | src |
| Scaling filter | Property with `DEFAULT` and `NEAREST_NEIGHBOR`; applied to both scalers; scale mode always upscale | `P`dpu95-plane.c:639-641, 530-553 | src |
| Rotation | No rotation property created | `P`dpu95-plane.c (no `drm_plane_create_rotation_property` call; creation block :602-641) | src |
| Is the primary plane mandatory? | **No such check.** The only per-CRTC rule is: an active CRTC must have at least one plane (`-EINVAL`). Planes of a CRTC are ordered by zpos and the lowest-zpos plane takes the ConstFrame stage, whichever type it is. An overlay-only frame is therefore not refused by this driver's source; whether the core or userspace refuses it is not decided here | `P`dpu95-crtc.c:246-250; `P`dpu95-kms.c:36-55, 257-271; `P`dpu95-plane.c:335-404 | src (conclusion that overlay-only passes: inferred; **must be confirmed by TEST_ONLY on the board**) |
| Plane not on a CRTC | A plane with a framebuffer must have a CRTC (`-EINVAL`); a plane with no framebuffer is accepted | `P`dpu95-plane.c:346-357 | src |
| Off-screen destination | Destination rectangle must lie inside the CRTC's adjusted mode (`x1 < 0`, `y1 < 0`, `x2 > hdisplay`, `y2 > vdisplay` refused). `can_position` is true; no clipping | `P`dpu95-plane.c:155-166, 364-374 | src |
| Other refusals | Interlaced framebuffers; BT709 + full range on YUV | `P`dpu95-plane.c:168-176, 238-248 | src |
| Modeset-dependent behaviour | A CRTC changing to active is forced into a modeset. CRTCs absent from the state are pulled in ("prone to put"); if the state does not allow a modeset they keep their current fetch units. A fetch unit bound to the other stream is skipped (no hot migration), so moving layers between CRTCs may fail without a modeset | `P`dpu95-crtc.c:231-244; `P`dpu95-kms.c:190-199, 291, 420-444, 226-229 | src |
| Bandwidth check | None found in `dpu95-plane.c`, `dpu95-crtc.c`, `dpu95-kms.c` | files above | src (absence in these files) |
| Mode limits | CRTC `mode_valid`: `crtc_clock > 350000` kHz is `MODE_CLOCK_HIGH`. LDB bridge `mode_valid`: `clock > 330000` is refused; `> 165000` on a single-link channel is refused; if the next bridge has detect and EDID, the DI clock must round exactly. `mode_config` min 60x60, max 8192x8192 | `P`dpu95.h:70; `P`dpu95-crtc.c:146-153; `drivers/gpu/drm/bridge/imx/imx95-ldb.c`:317-351; `P`dpu95-kms.c:597-600 | src |
| Page-flip event timing | The flip event is queued in `atomic_flush` and sent from the `DISENGCFG` frame-complete interrupt handler, together with `drm_crtc_handle_vblank()`; `atomic_flush` waits (1 s timeout, error is logged, not returned) for the ExtDst shadow-load. Vblank timestamps use `drm_crtc_vblank_helper_get_vblank_timestamp` with a driver `get_scanout_position` | `P`dpu95-crtc.c:78-96, 119-130, 324-408, 473-511 | src |
| Clocks, power, DMA | Clocks `pix`, `apb`, `axi`, `ocram`, `ldb`, `ldb_vco`; power domain `IMX95_PD_DISPLAY`; QoS written to the blk-ctrl; 40-bit DMA mask | `P`dpu95-core.c:1064-1082, 902-915, 1093; `imx95.dtsi`:2759-2767 | src |

## Device tree

| Item | Finding | Where | Tag |
| --- | --- | --- | --- |
| Display chain in the SoC dtsi | `dpu: display-controller@4b400000` (`nxp,imx95-dpu`, **disabled**) with ports 0 and 1 to `pixel_interleaver` channels 0 and 1, then `display_pixel_link` (bridge@8), then `ldb` (`fsl,imx95-ldb`, **disabled**, `channel@0` and `channel@1`, each with its own LVDS PHY), plus the MIPI-DSI branch. `ldb_pll_pixel` is the fixed divide-by-7 clock feeding the DPU's `ldb` clock | `DTS`imx95.dtsi:2647-2790 (dpu, status :2768), :2409-2462, :2499-2552 (ldb, status :2509), :2589-2645, :345 | src |
| CRTC to LDB channel | Stream 0 to pixel interleaver 0 to pixel link 0 to LDB channel 0 and LVDS PHY 0; stream 1 to channel 1. The LDB bridge also supports dual-link/split mode (both channels on one panel) | `DTS`imx95.dtsi:2774-2788, 2609-2641, 2444-2462, 2519-2546; `imx95-ldb.c`:36-60, 436-470 | src |
| Verdin iMX95 / Mallow | `imx95-verdin.dtsi`, `imx95-verdin-mallow.dtsi`, and the `imx95-verdin-{wifi,nonwifi}-mallow.dts` contain **no** reference to `&dpu`, `&ldb`, `&display_pixel_link`, `lvds` or `pixel_interleaver`. In this kernel tree the display is not enabled for the Mallow; the board's panel configuration must come from an overlay outside this tree (Toradex device-tree overlays, applied by the boot loader). That overlay was **not read** | `git grep` over these files at the commit: no match | src (absence); overlay source: not obtained |
| NXP-derived Verdin panel dts | The tree has `imx95-19x19-verdin-panel-cap-touch-10inch-lvds.dts` (includes `imx95-19x19-verdin.dts`): panel `logictechno,lt170410-2whc` on LDB `channel@0` port 1, `assigned-clock-rates = <2986200000>, <497700000>` for `LDBPLL_VCO`/`LDBPLL`, `pixel_interleaver` channel 0, PWM backlight. This is NXP's 19x19 EVK adaptation, **not** the Mallow carrier; it shows the shape (LDB channel 0, stream 0) but is not proof of the board's configuration | `DTS`imx95-19x19-verdin-panel-cap-touch-10inch-lvds.dts:6-77 (panel :9-19, LDB :40-62, clocks :43-45) | src |
| Panel timing (`logictechno,lt170410-2whc`) | `panel-simple`: 1280x800, pixel clock 68.9 / 71.1 / 73.4 MHz (min/typ/max), hfp 23/60/71, hbp 23/60/71, hsync 15/40/47, vfp 5/7/10, vbp 5/7/10, vsync 6/9/12, HSYNC and VSYNC low, DE high; 8 bpc, 217x136 mm, LVDS, bus format `RGB888_1X7X4_SPWG`. Typical totals 1440x823 give 71.1 MHz / (1440 x 823) = **60.0 Hz**; `497.7 MHz / 7 = 71.1 MHz` matches the overlay's PLL rate. That the board's Toradex 10.1" panel is this panel is **inferred** (same overlay name, no overlay read) | `drivers/gpu/drm/panel/panel-simple.c`:2731-2756, compatible :4471-4472 | src (panel identity: inferred) |
| LVDS electrical limits | A single-link channel refuses modes above 165 MHz, so this 71 MHz panel is a single-link, single-channel case | `imx95-ldb.c`:330-334 | src |

## Differences from `DriverProfile::Imx95Dpu`

`crates/composition-drm/src/profile.rs` (read from `lf-6.18.y`) compared with the table above. `profile.rs` is **not edited here**; `profile-check` decides after the board observation.

| # | Profile (`profile.rs`) | At 6.6.157-7.8.0 | Verdict |
| --- | --- | --- | --- |
| 1 | `ANY` group, capacity 6, over all planes of the output | 6 hardware planes shared by both CRTCs (`DPU95_HW_PLANES`); one CRTC sees primary + 5 overlays = 6 | same |
| 2 | `SCALED ∪ YUV` group, capacity 4 | Only FetchYUV (4) has the HSCALER/VSCALER/FETCHECO/PACKED_YUV422 caps; FetchLayer's mask is 0 | same |
| 3 | `SCALED_H` capacity 1, `SCALED_V` capacity 1 | One HScaler, one VScaler in the shared group | same, but see 4 and 9 |
| 4 | Vertical scaler available (capacity 1) always | On SoC rev 1.0 / 1.1 there is no VScaler4: any vertical scaling is refused (`USE_VSCALER4` missing). Capacity for `SCALED_V` would be 0 there | **differs on rev 1.x** (depends on the board's SoC revision, unknown) |
| 5 | `max_source` 8192x8192 for every non-cursor plane | 8192 only unscaled; with scaling 2048 per axis. A layer scaled on one axis only is judged "unscaled" and gets 8192 (`src_w == dst_w || src_h == dst_h`) | **differs**: scaled layers are limited to 2048 |
| 6 | `max_upscale = 2048 << 16`, `max_downscale = 1 << 16` | `min_scale = 1/2048`, `max_scale = NO_SCALING`: upscale up to 2048x, no downscale | same (an earlier version of this note called the 2048 a source-width limit; the source shows it is the upscale ratio *and* a separate source-size limit) |
| 7 | Doc comment: "at most six planes are active" over both streams | `lb[6]` is shared; stream 0 takes blends from the bottom, stream 1 from the top | same |
| 8 | Doc comment: units are shared with the other stream, invisible to the planner | Confirmed: fetch units, LayerBlends, HScaler and VScaler are in one group | same |
| 9 | Not modelled: no scaler/fetch binding to a stream | A unit bound to the other stream is not migrated; a CRTC not in the state keeps its units when no modeset is allowed | not modelled; can cause refusals the planner does not predict |
| 10 | Not modelled: bottom-plane opacity | zpos-0 plane ignores alpha and blend mode (always opaque) | not modelled (matters for expected alpha pictures) |
| 11 | Not modelled: placement rules | Destination must be fully inside the mode; even source width/height/x/y for 4:2:0 and 4:2:2; address and pitch alignment; pitch <= 0x10000; BT709 full range refused; interlaced refused | not modelled |
| 12 | Not modelled: mode limits | CRTC 350 MHz, LDB 330 MHz (165 MHz single link) | not modelled; the 71 MHz panel is far below |
| 13 | Planner refuses an overlay-only frame (planner/profile premise, see drm-sem-fixes) | No primary-plane check in the driver source; the lowest-zpos plane takes the ConstFrame stage | **not supported by the source**; decide by the board's TEST_ONLY |
| 14 | `profile.rs` doc comment cites `lf-6.18.y` | The board kernel is 6.6.157; the structure (counts, caps, sharing, scaling) matches in every item above except 4, 5 and the unmodelled rules | note to update after the board run |

Not covered by the profile and not in the table because KMS reports it: formats (13, one list), modifiers (linear), zpos (mutable 0..5), alpha, blend modes, scaling-filter and color properties.

## What this does not establish

- The SoC revision, the actual device tree and overlay of the board, whether the 10.1" panel is `lt170410-2whc`, and which connector and CRTC appear are **unobserved**. The probe on the board is the authority (`wp0-probe`).
- Behaviour claims (for example overlay-only acceptance) are conclusions from source, to be confirmed with `TEST_ONLY` on the board.
