# RK3588 VOP2 topology, read from source

Staging file for plan #454 (item `src-rk3588`). A later item moves it into `docs/platform-capabilities.md` and deletes `docs/observed/`. Nothing here was run on a board. Rows are tagged:

- **source**: read from the file and line given. Line numbers are from the pinned commit below.
- **inferred**: a conclusion drawn from the source, not stated by it.

Read on 2026-11-28.

## Pinned trees

| Tree | Branch | Commit | Read? |
| --- | --- | --- | --- |
| `rockchip-linux/kernel` (vendor) | `develop-6.1` | `77168c8d5ab82399f65a80e9f807b50ba37cf483` | Yes. Only `drivers/gpu/drm/rockchip/` was checked out. The kernel version number was not read. |
| `torvalds/linux` (mainline) | not fetched | none | **No.** The clone was refused by the sandbox. The mainline section at the end is unverified. |

Paths below are in `drivers/gpu/drm/rockchip/`. `V2` is `rockchip_drm_vop2.c`, `REG` is `rockchip_vop2_reg.c`.

The vendor driver is one driver for several SoCs. `rk3588_vop` (REG:7495) is selected by `rockchip,rk3588-vop` (REG:7546). RK3588 is not treated as "VOP3" (`is_vop3()`, V2:1370-1376).

## Video ports (CRTCs)

| Item | Finding | Basis |
| --- | --- | --- |
| Count | 4 video ports, VP0 to VP3 (`nr_vps = 4`, REG:7499; table REG:3034) | source |
| Mixers and layers | 7 mixers, 8 layers (`nr_mixers`, `nr_layers`, REG:7500-7501) | source |
| VP0 | Max output 7680x4320, dclk max 2.4 GHz, 10-bit output, HDR10, DoVi. Can be spliced with VP1 (`splice_vp_id = 1`) | source (REG:3036-3052) |
| VP1 | Max output 4096x2304, dclk max 600 MHz, 10-bit | source (REG:3055-3067) |
| VP2 | Max output 4096x2304, dclk max 600 MHz, 10-bit | source (REG:3068-3081) |
| VP3 | Max output 2048x1536, dclk max 200 MHz, no 10-bit | source (REG:3083-3092) |
| Splice mode | Used when `hdisplay > 4096` (`VOP2_MAX_VP_OUTPUT_WIDTH`). Each plane then also takes its splice window (Cluster0 with Cluster1, Cluster2 with Cluster3, Esmart0 with Esmart1, Esmart2 with Esmart3), and the maximum input width doubles | source (V2:175, 7683-7695; `splice_win_id` in REG:6035-6410) |
| Which ports become CRTCs | If the device tree (or bootloader) gives any port a non-zero `rockchip,plane-mask`, ports with a zero mask are skipped. If none do, all ports are tried | source (V2:18512-18535, 18568-18575, 19869-19870) |

## Windows and DRM planes

Window table: `rk3588_vop_win_data[]`, REG:6035-6410.

| Item | Finding | Basis |
| --- | --- | --- |
| Hardware windows | 4 Cluster (Cluster0 to 3), each with a main window ("-win0") and a sub window ("-win1"), and 4 Esmart (Esmart0 to 3). There are no Smart windows on RK3588 | source (REG:6035-6410) |
| Window to plane | One DRM plane per window entry, via `drm_universal_plane_init()` in `vop2_plane_init()`. The entries are Cluster0-3 win0, Cluster0-3 win1 and Esmart0-3 win0 | source (V2:18007-18053) |
| Cluster sub windows | Not registered unless the DT sets `support-multi-area`: with it unset, `WIN_FEATURE_CLUSTER_SUB` windows return `-EACCES` and get no plane. So the default plane count is 8 (4 Cluster + 4 Esmart); with the flag it is 12 | source (V2:18027-18030, 19693); the counts are inferred |
| Esmart multi-area | Esmart windows carry `WIN_FEATURE_MULTI_AREA` and `area = rk3568_area_data` (3 extra areas). This reading did not show whether those areas become separate DRM planes | source (REG:6275-6300, 3866-3870); not resolved |
| Cluster sub-window rule | A sub window follows its main window. Both must use the same modifier. With the main window off, the sub window is forced invisible. Source width plus the x offset (mod 16) is limited to 2048 in this mode | source (V2:6916-6962) |
| Plane type | Primary candidates are the four Esmart windows. Cluster windows are Overlay. One primary is chosen per registered CRTC: `primary_plane_phy_id` from the DT if set, else the first free Primary-type window that can attach. Unused Primary windows are demoted to Overlay. A cursor plane exists only if the DT names a `cursor-win-id` | source (REG:6275-6410; V2:18600-18700, 18785-18800) |
| Default plane-to-VP table | `rk3588_vp_plane_mask`, both the "one display" and "two display" policies: first display gets Cluster0, Esmart0, Cluster2, Esmart2 (primary Esmart0); second gets Cluster1, Esmart1, Cluster3, Esmart3 (primary Esmart1); third and fourth are empty. The DT overrides it | source (REG:7172-7215) |
| Planes per CRTC at default | 4 on each of 2 CRTCs (1 primary + 3 overlay) | inferred from the row above |

## possible_crtcs

| Item | Finding | Basis |
| --- | --- | --- |
| Hardware capability | Every window lists all four ports in `possible_vp_mask` | source (REG:6035-6410) |
| Mask given to DRM, normal | `vop2_win_get_possible_crtcs()` maps the window's VP mask onto the CRTCs actually registered (bit i is the i-th enabled VP), so every plane is possible on every registered CRTC | source (V2:18149-18167) |
| Mask given to DRM with `disable-win-move` | Primary: only its own CRTC. Overlay: only the CRTC whose `plane_mask` holds the window | source (V2:18187, 18670, 18821-18834; DT flag V2:19695) |
| PLANE_MASK property | Because RK3588 is not VOP3, each CRTC gets a `PLANE_MASK` property listing its windows, for userspace that moves windows between CRTCs | source (V2:18751-18756) |
| Moving a plane at commit time | `possible_crtcs` says "any", but whether a given window can be used on a given CRTC at commit time was not traced. TEST_ONLY should be the arbiter | inferred |

## Formats and modifiers

| Item | Finding | Basis |
| --- | --- | --- |
| Cluster formats | XRGB2101010, XBGR2101010, XRGB8888, ARGB8888, XBGR8888, ABGR8888, RGB888, BGR888, RGB565, BGR565, YUV420_8BIT, YUV420_10BIT, YUYV, Y210. The code comments say the YUV ones are non-linear only | source (REG:33-46) |
| Esmart formats | XRGB8888, ARGB8888, XBGR8888, ABGR8888, RGB888, BGR888, RGB565, BGR565, NV12, NV21, NV16, NV61, NV24, NV42, NV15, NV20, NV30, YVYU, VYUY, YUYV, UYVY. No 10-bit RGB | source (REG:108-129) |
| Cluster modifiers | AFBC 16x16 alone and in combinations of SPARSE, YTR, CBR and SPLIT (SPLIT needs SPARSE; per the source comments the RGB modes need YTR), then LINEAR, then INVALID | source (REG:254-288) |
| Esmart modifiers | LINEAR and INVALID only | source (REG:249-252) |
| AFBC can be turned off | `disable_afbc_win` stops AFBC windows from registering. `disable_afbc_mask` strips the modifiers from chosen windows | source (V2:18017-18021, 18036-18041) |
| Linear rules on RK3588 | Linear XRGB2101010 and XBGR2101010 are rejected on any window. Linear YUV on a Cluster window is rejected | source (V2:7799-7810) |
| Tiled | The check refuses AFBC and tiled together. None of the RK3588 modifier lists above contain a tiled modifier | source (V2:7779-7782); the second part is inferred from the lists |

## zpos, alpha, blend

| Item | Finding | Basis |
| --- | --- | --- |
| zpos property | `drm_plane_create_zpos_property(plane, win->win_id, 0, registered_num_wins - 1)`: mutable, range 0 to registered windows minus 1, initial value from the window's id | source (V2:18080) |
| zpos handling | The DRM core normalises zpos (`normalize_zpos = true`). The driver reads `pstate->zpos` in the plane check and sorts windows by it to set mixer order | source (`rockchip_drm_fb.c:423`; V2:7732, 12864, 15473) |
| Initial zpos | Primary: the CRTC index (lowest). Overlays: CRTC index plus overlay index. Cursor: highest | source (V2:18700-18701, 18815-18817) |
| Plane alpha | `alpha` property, applied as `alpha >> 8` | source (V2:18078, 7733) |
| Pixel blend mode | None, Pre-multiplied and Coverage offered | source (V2:18010-18011, 18079) |
| Alpha chain | Global alpha, per-pixel alpha, or both are chosen from the format and the alpha value. A global alpha of 0xff counts as off | source (V2:13567-13623) |
| Immutable zpos? | No. The property comes from the DRM helper (mutable) and nothing in this path makes it immutable | source |

## Scaling

| Item | Finding | Basis |
| --- | --- | --- |
| Hardware factors | Cluster: up and down 4x. Esmart: up and down 8x | source (REG:6035-6410, `max_upscale_factor`, `max_downscale_factor`) |
| What the atomic check enforces | 1/8 to 8 whenever the window has scaler registers, otherwise no scaling. This is looser than the Cluster figure of 4 | source (V2:7637-7662); the mismatch is inferred |
| Exposed to userspace | Immutable range properties `SCALE_RATE`, `INPUT_WIDTH/HEIGHT`, `OUTPUT_WIDTH/HEIGHT` | source (V2:18107-18140) |
| Minimum size | Source and destination must be at least 4x4. Smaller makes the plane invisible, not an error | source (V2:7740-7749) |
| Esmart scale-down limit | A comment in the programming path says Esmart cannot scale down when the source width mod 16 is 1 or the destination width is odd | source (V2:8460-8461) |

## Rotation and reflection

| Item | Finding | Basis |
| --- | --- | --- |
| Property | Created only if the window has `supported_rotations`: ROTATE_0 plus the supported set | source (V2:18075-18077) |
| Cluster main windows | ROTATE_90, ROTATE_270, REFLECT_X, REFLECT_Y | source (REG:6035-6410) |
| Cluster sub windows | REFLECT_X, REFLECT_Y | source |
| Esmart | REFLECT_Y only | source |
| Restrictions | ROTATE_90 with ROTATE_270 is rejected. A Cluster window without AFBC rejects REFLECT_X, ROTATE_90 and ROTATE_270. A Cluster window with a tiled format rejects all reflection and rotation | source (V2:7699-7703, 7813-7826) |

## Size limits

| Item | Finding | Basis |
| --- | --- | --- |
| Cluster main window | Input and output up to 4096 x 7680 | source (REG:6035-6410) |
| Cluster sub window | Input and output up to 2048 x 7680 | source |
| Esmart | Input and output up to 4096 x 7680 | source |
| Splice | The input width limit doubles when the CRTC is in splice mode | source (V2:7695) |

## Shared resources and bandwidth

| Item | Finding | Basis |
| --- | --- | --- |
| Shared hardware | A Cluster main and sub window share one cluster (the same AFBC decoder) and must agree on modifier. Cluster0-3 and the Esmart group have separate power domains | source (V2:6916-6962; REG:5806-5864) |
| AXI buses | Cluster0, Cluster1, Esmart0, Esmart1 use AXI 0. Cluster2, Cluster3, Esmart2, Esmart3 use AXI 1 | source (`axi_id`, REG:6035-6410) |
| Bandwidth | `vop2_crtc_bandwidth()` computes line and frame bandwidth (halved for AFBC) and hands it to DDR frequency scaling. I found no path that rejects a commit because of it | source (V2:10603-10760); "does not reject" is inferred, since I did not read its callers |
| Layer mixer | Window to layer assignment comes from `layer_sel_id` and the zpos-sorted list at commit | source (V2:14483, 15473-15483) |
| Simultaneous windows | 8 layers and 7 mixers bound the windows that can be active at once. Whether all 12 windows can be on together was not determined | inferred |

## Mainline differences (not verified)

The mainline `rockchip_drm_vop2.c` and `rockchip_vop2_reg.c` were **not fetched**, so there is no file or line to cite and no row here carries a source tag. The points below are recalled, not read, and must be checked against a real checkout before use:

- Mainline has a RK3588 VOP2 with four Cluster and four Esmart windows, as the vendor tree does.
- Mainline is expected to expose zpos, alpha and blend mode, with less rotation support than the vendor driver.
- Mainline is not expected to carry the vendor-only features seen above: `PLANE_MASK`, `disable-win-move`, `support-multi-area`, DDR bandwidth reporting and the Rockchip tiled modifiers.

Which kernel a board runs decides which of the two applies. The board item should record the kernel version and the plane list from `probe`, then correct these rows.

## Sources

- `rockchip-linux/kernel` `develop-6.1` @ `77168c8d5ab82399f65a80e9f807b50ba37cf483`
  - `drivers/gpu/drm/rockchip/rockchip_vop2_reg.c`: formats 33-46, 108-129; modifiers 249-288; area data 3866-3870; video ports 3034-3095; power domains 5806-5864; windows 6035-6410; plane mask table 7172-7215; `rk3588_vop` 7495-7530; compatible 7546.
  - `drivers/gpu/drm/rockchip/rockchip_drm_vop2.c`: `VOP2_MAX_VP_OUTPUT_WIDTH` 175; `is_vop3` 1370; cluster two-window check 6916-6962; plane atomic check 7637-7830; AFBC/tiled/rotation checks 7699-7830; scale-down comment 8460; bandwidth 10603-10760; zpos sort 12864; alpha 13567-13623; mixer 14483, 15473; `vop2_get_max_output_width` 17899; `vop2_plane_init` 18007-18147; possible CRTCs 18149-18167; CRTC and plane creation 18505-18835; DT properties 19693-19695, 19869-19870.
  - `drivers/gpu/drm/rockchip/rockchip_drm_fb.c`: `normalize_zpos` 423.
- Mainline `drivers/gpu/drm/rockchip/rockchip_drm_vop2.c` and `rockchip_vop2_reg.c`: not read.
