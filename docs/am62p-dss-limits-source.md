# AM62P DSS: planes, formats, scaling and blending limits (source study)

Date of the study: 2026-11-28. **Nothing in this file is observed.** No device was used and no board identifier appears. Every statement is tagged `read from source` and carries a citation, or is tagged **inferred** where a source does not state it.

## What was read, and what was not

Abbreviations: `D` = `drivers/gpu/drm/tidss/`; `DTS` = `arch/arm64/boot/dts/ti/`; **TDX** = Toradex's TI-derived kernel `toradex_ti-linux-6.6.y` at commit `9764efd3fe8e5f3fa04045919296c102acd7517e` (Makefile 6.6.163); **ML** = mainline tag `v6.12` (`adc218676eef25575469234709c2d87185ca223a`); `SRC` = `docs/am62p-dss-source.md`; `GO` = `docs/am62p-dss-gobit-source.md`; `SHARE` = `docs/am62p-dss-sharing-source.md`.

| Source | Version | How it was read in this item |
| --- | --- | --- |
| Linux `D`tidss_dispc.c (feature tables `dispc_am62p51_feats`, `dispc_am62p52_feats`), `tidss_plane.c`, `tidss_kms.c`, `tidss_crtc.c`, `tidss_drv.c` | TDX `9764efd3fe8e`; ML `v6.12` for the code that is the same | **Not re-read here.** This item had no network access. The rows below are carried from `SRC` and `GO`, which read those files at the commits above and give file and line ranges. Each row names the section of the merged study it comes from. ML `v6.12` has no AM62P table (`SRC`, "Summary of differences"). |
| TI AM62P TRM (DSS chapter) | | **Not read, by this item or by `GO`** (`GO`, "What was read": the TRM page failed to load). No TRM statement about the AM62P is made below. The one TRM text in the repository is the AM67 TRM SPRUJB3 snippets in `GO`, a sibling device. |

A statement tagged `read from source (SRC)` was read from the Linux file by the author of `SRC`; this item checked only that `SRC` says it. If `SRC` is wrong, so is this file. What the DSS silicon can do beyond what the driver programs is **not** covered: a limit the driver enforces is not necessarily a limit of the silicon.

## 1. Instances, video ports, planes

- **read from source (SRC, rows "Compatibles", "DSS instances")**: TDX `D`tidss_drv.c:381-384 lists `ti,am62p51-dss` and `ti,am62p52-dss`; the SoC dtsi (`DTS`k3-am62p-main.dtsi:1186-1206, 1240-1259) has two DSS instances, `dss0` and `dss1`, both `status = "disabled"` in the SoC file.
- **read from source (SRC, row "Video ports per instance")**: each instance has `.num_vps = 2` (`vp1`, `vp2`) and two overlay managers `ovr1`, `ovr2` (`D`tidss_dispc.c:391-438 for DSS0, 440-487 for DSS1). Bus types: DSS0 `{ INTERNAL, DPI }`, DSS1 `{ INTERNAL, INTERNAL }`. Maximum pixel clock: DPI 165000 kHz, INTERNAL 300000 kHz.
- **read from source (SRC, row "Planes per instance")**: each instance has `.num_planes = 2`: `vid_name = { "vid", "vidl1" }`, **`vid_lite = { false, true }`**. So there is one full VID pipe and one VIDL (lite) pipe per instance, four pipes in the SoC. The two tables are otherwise identical (`D`tidss_dispc.c:433-437, 482-486).
- **read from source (SRC, row "Plane creation order")**: `.vid_order = { 1, 0 }`: the first DRM plane created is hw id 1 (`vidl1`), the second is hw id 0 (`vid`) (`D`tidss_dispc.c:437, 486). With one CRTC, the first plane is primary (the lite one) and the second is an overlay (the full one); with two CRTCs both are primaries and no overlay exists (`D`tidss_kms.c:189-203, 225-239). TDX only for the `vid_order` table.
- **read from source (SRC, row "Which planes on which CRTC")**: every plane gets `possible_crtcs = (1 << num_pipes) - 1` (`D`tidss_kms.c:184-185, 195-197, 229-231): a plane can be routed to any CRTC of the instance.
- **read from source (SRC, row "CRTCs")**: one CRTC exists per video port that has a panel or bridge on its OF graph port (`D`tidss_kms.c:126-182, 267), so 0, 1 or 2 per instance depending on the device tree.
- **inferred**: for this engine, "two planes" means the `vid` and `vidl1` pipes of one DSS instance on one video port.

## 2. Which pipes can scale

- **read from source (SRC, row "Scaling: which plane")**: only the full `vid` pipe scales. `dispc_plane_check` returns `-EINVAL` for a lite plane when `src_w != crtc_w` or `src_h != crtc_h` (`D`tidss_dispc.c:2169-2206, refusal at :2192-2199), and `dispc_plane_setup` skips `DISPC_VID_SIZE` and the scaler setup for a lite plane (`D`tidss_dispc.c:2296-2302). Same in TDX and ML.
- **read from source (SRC, row "Scaling limits (`vid`)")**: `.upscale_limit = 16`, `.downscale_limit_5tap = 4`, `.downscale_limit_3tap = 2`, `.xinc_max = 32` (`D`tidss_dispc.c:397-413). Input width at most 1280 (RGB) or 2560 (YUV) for the 5-tap filter, at most 2560 (RGB) or 4096 (YUV) for the 3-tap filter; a wider input switches to 3-tap with horizontal decimation. Horizontal downscale beyond `downscale_limit * xinc_max` (128 for 5-tap) is refused (`D`tidss_dispc.c:1866-2028, limits :1919-1954, 1970-1975).
- **read from source (SRC, row "Plane check (DRM level)")**: the DRM-level check passes `min_scale = 0`, `max_scale = INT_MAX`, `can_position = true`, so the core imposes no scale limit (`D`tidss_plane.c:61-101). Source x, y and width must be multiples of the format's `hsub`/`vsub` (matters for YUV only).
- **inferred (GO, section (c))**: scaler registers are plane registers written before the video port's GO; no source read says a scale change needs a different latch.

## 3. Pixel formats per pipe

- **read from source (SRC, row "Formats per plane")**: one list for all planes, 28 formats, built from `dispc_color_formats[]`: ARGB4444, ABGR4444, RGBA4444, RGB565, BGR565, ARGB1555, ABGR1555, ARGB8888, ABGR8888, RGBA8888, BGRA8888, RGB888, BGR888, ARGB2101010, ABGR2101010, XRGB4444, XBGR4444, RGBX4444, XRGB1555, XBGR1555, XRGB8888, XBGR8888, RGBX8888, BGRX8888, XRGB2101010, XBGR2101010, YUYV, UYVY, NV12 (`D`tidss_dispc.c:2084-2128, 3062-3073, 3249-3263; `D`tidss_kms.c:107, 195-197). The list is not reduced for the lite pipe. YUV entries are dropped only for erratum i2000, set for AM65X SR1.0 only. ML's table is identical.
- **read from source (SRC, row "Modifiers")**: `drm_universal_plane_init` is called with no modifiers, so only linear is advertised (`D`tidss_plane.c:214-218).
- **read from source (SRC, row "CSC check")**: a YUV format needs a matching colour-space entry for the plane's encoding and range, else `-EINVAL` (`D`tidss_dispc.c:2180-2189). Properties: BT601 (default) and BT709; full (default) and limited range (`D`tidss_plane.c:195-200, 227-233).
- **inferred**: that the VIDL hardware renders YUV is not shown by the driver (it lets the format through). RGB565 and ARGB8888, the formats of the engine's two-plane scenes (SoW §5.2, as in `docs/am62p-imx95-matrix.md`), are in the list for both pipes as far as the driver goes.

## 4. Maximum sizes

- **read from source (SRC, row "Mode limits")**: `dispc_vp_mode_valid` refuses `hdisplay` or `vdisplay` above 4096, interlace, odd `hdisplay`, a pixel clock above the bus type's maximum, `hsw` outside 1..256, `hfp`/`hbp` outside 1..4096, `vsw` outside 1..256, `vfp`/`vbp` above 4095 (`D`tidss_dispc.c:1343-1416). DRM `mode_config` limits: min 8x8, max 8096x8096 (`D`tidss_kms.c:255-258).
- **read from source (SRC, row "Plane check (DRM level)")**: a plane partly outside the CRTC is clipped (`can_position = true`; `D`tidss_plane.c:61-101).
- **read from source (SRC, row "Memory bandwidth check")**: if the DT node has `max-memory-bandwidth`, a mode is refused when `1000 * clock * hdisplay * vdisplay * 4 / (htotal * vtotal)` exceeds it (`D`tidss_dispc.c:1418-1431, 3350-3351). Whether any DT sets it: not read.
- **not read from source**: a per-plane maximum framebuffer width or height, pitch alignment, base-address alignment. `SRC` says no pitch or base-address alignment check is in the files read. The TRM would state silicon limits and was not read.

## 5. Z-order and alpha blending

- **read from source (SRC, row "Layer order, zpos")**: `drm_plane_create_zpos_property(plane, num_planes, 0, num_planes - 1)`: mutable, range 0..1 for two planes. `mode_config.normalize_zpos = true`. The CRTC code walks overlay layers `0..num_planes-1`, finds the visible plane with `normalized_zpos == layer` and writes its id and position into `DISPC_OVR_ATTRIBUTES(layer)` (`D`tidss_plane.c:224-225; `D`tidss_kms.c:259; `D`tidss_crtc.c:128-169; `D`tidss_dispc.c:1500-1510, 1550-1558).
- **read from source (SRC, row "Plane position")**: the position is written to the overlay manager: x in bits 17:6 and y in bits 30:19 of `DISPC_OVR_ATTRIBUTES(layer)` for AM62P (`D`tidss_dispc.c:1500-1510, 1527-1539). TDX only.
- **read from source (SRC, row "Global alpha")**: a per-plane `alpha` property, written as `DISPC_VID_GLOBAL_ALPHA = 0xFF & (alpha >> 8)` (`D`tidss_plane.c:235; `D`tidss_dispc.c:2312-2313).
- **read from source (SRC, row "Pixel blend modes")**: the driver offers **Pre-multiplied** and **Coverage** only; `None` is not offered. Pre-multiplied sets `DISPC_VID_ATTRIBUTES` bit 28; any other mode clears it (`D`tidss_plane.c:201-202, 239; `D`tidss_dispc.c:2315-2320).
- **read from source (SRC, row "Default background")**: the video port's default colour is 0, black (`D`tidss_dispc.c:1330-1341, 2948-2953).
- **read from source (SRC, row "Rotation")**: no rotation property exists in any tidss file (`git grep` over `D` returned nothing, TDX).
- **inferred**: the overlay manager is driven with as many layers as planes (2) per video port; the hardware may allow more, the driver does not use them.
- **inferred (SRC)**: whether a format without an alpha channel (XRGB8888, RGB565) is opaque under Coverage is hardware behaviour the driver does not state.

## 6. The overlay manager's per-VP GO bit

- **read from source (SRC, row "GO bit"; GO, section (a))**: the GO bit is bit 5 of `DISPC_VP_CONTROL`, one per video port. `dispc_vp_go_busy()` reads it; `dispc_vp_go()` warns if it is set and then writes 1 (`D`tidss_dispc.c:1279-1288 in TDX; ML :1155-1164).
- **read from source (GO, section (a), AM67 TRM SPRUJB3 snippet, a sibling device)**: "The DSS0_VP_CONTROL[5] GOBIT bit enables the hardware to use the new configuration, for all shadow registers associated with each VP output." Shadow registers are tied to a video port, statically (timing) or dynamically (video registers). The AM62P driver uses the same bit position.
- **read from source (SRC, row "GO-bit use in a commit"; GO, section (a))**: `tidss_crtc_atomic_flush` writes plane and overlay registers, then calls `dispc_vp_go()` once per VP under the event lock (`D`tidss_crtc.c:171-218). There is no per-plane commit bit: all plane and overlay writes of a commit are latched by the one GO of their VP.
- **read from source (GO, sections (a) and (c))**: new settings are taken at the vertical front porch and GO clears at the same time, before the vblank interrupt (`D`tidss_crtc.c, `tidss_crtc_finish_page_flip`). A flush while GO still reads 1 is dropped with a `WARN_ON` (`D`tidss_crtc.c:193-195).
- **read from source (SRC, row "Shared mode")**: for a VP Linux does not own, the Linux driver does no GO write and no VP or overlay register access (`D`tidss_crtc.c:220-256, 303-322, 369-388). `SHARE` covers the split with a remote core.
- **inferred**: two planes on one VP are latched together by one GO; planes on the two VPs of an instance are latched separately. The AM62P TRM was not read to confirm either.

## Summary table, in the form of `docs/am62p-imx95-matrix.md` (AM62P cells only)

Every cell is `read from source` in the sense of this file (TDX `9764efd3fe8e`, via `SRC` and `GO`); none is observed. Rows the matrix has and this study cannot fill say "not recorded".

| Item | AM62P |
| --- | --- |
| Display controller | DSS, Linux driver `tidss`; compatibles `ti,am62p51-dss`, `ti,am62p52-dss` (TDX only; none in ML v6.12). 2 instances, 2 video ports each (source, `SRC`) |
| CRTCs / outputs | One CRTC per video port that has a panel or bridge in the DT: 0, 1 or 2 per instance (source, `SRC`) |
| Planes (count, type) | 2 per instance: `vid` (full VID) and `vidl1` (VIDL, lite). With one CRTC the lite one is primary and the VID one an overlay; with two CRTCs both are primary (source, `SRC`) |
| Plane → CRTC | Every plane can go on any CRTC of the instance (source, `SRC`) |
| Formats | One list for both planes, 28 formats incl. RGB565, ARGB8888, XRGB8888, YUYV, UYVY, NV12; linear only (source, `SRC`). Whether VIDL renders YUV: inferred |
| Scaling | Only the full `vid` plane scales; the lite plane refuses a size change. Up 16x, down 4x (5-tap) or 2x (3-tap), `xinc_max` 32; input width 1280/2560 (5-tap), 2560/4096 (3-tap) (source, `SRC`). Silicon limits: not recorded (TRM not read) |
| Blending / alpha | Per-plane alpha (8 bits used); blend modes Pre-multiplied and Coverage, no None (source, `SRC`) |
| Z-order | Mutable zpos 0..1 (source, `SRC`) |
| Rotation | No property (source, `SRC`) |
| Maximum size | Mode up to 4096x4096, no interlace, even width; DRM limit 8096x8096; pixel clock 165 MHz (DPI) or 300 MHz (internal). Per-plane size, pitch, alignment: not recorded (source, `SRC`) |
| Commit / latch model (hardware) | Shadowed plane and overlay registers latched by one GO bit (`DISPC_VP_CONTROL` bit 5) per video port, taken at the front porch, GO clears itself; Linux drops a flush while GO is set (source, `GO`; the clear is AM67 TRM text, AM62P TRM not read) |
| Constraints on a commit | A position or zpos change pulls all planes of the CRTC into the state; memory-bandwidth check if `max-memory-bandwidth` is set (source, `SRC`) |

## Not covered

The AM62P TRM chapter, per-plane silicon limits, the TI MCU+ SDK DSS driver, DSI/OLDI behaviour, the 6.18 branch, and everything observed. When the TRM can be read, check each AM67-TRM and driver-enforced statement above against it.
