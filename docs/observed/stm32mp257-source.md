# STM32MP257 LTDC: topology read from ST kernel source

Staging file for the later fold into `docs/platform-capabilities.md`. Nothing here was measured on a board. Every row is tagged **source** (stated by the code) or **inferred** (a conclusion drawn from it). Date of reading: 2026-11-28.

## What was read, and how

- Tree: `STMicroelectronics/linux`, branch `v6.18-stm32mp`, commit `3056405e0479330b0889c2a971452bfd10014b70` (committed 2026-10-23, subject "ARM: dts: stm32: move PMIC control to secure world in ST stm32mp15 boards"; the tip itself is unrelated to display).
- Read: `drivers/gpu/drm/stm/ltdc.c` in full (91,082 characters, 2941 lines). A copy fetched from that branch is at `$CONNECTED_DEPS_DIR/stlinux/ltdc.c` (a single file, not a git checkout). The line numbers below were checked against it. The task was later moved to another worker, where that copy is not present; a second attempt on 2026-11-28 to fetch the missing files (raw.githubusercontent.com: HTTP 429; github.com raw: HTTP 504; clone and curl: refused) also failed.
- **Not read** (raw.githubusercontent.com answered HTTP 429, and `git clone`, `git fetch`, `curl` and `gh` into the dependency cache were all refused on this worker): `drivers/gpu/drm/stm/ltdc.h`, `drv.c`, `Kconfig`, `Makefile`, `arch/arm64/boot/dts/st/stm32mp251.dtsi`, `stm32mp255.dtsi`, `stm32mp257f-dk.dts`, `Documentation/devicetree/bindings/display/st,stm32-ltdc.yaml`, and mainline `ltdc.c`.
- References in the tables are function or identifier names in `drivers/gpu/drm/stm/ltdc.c`; line numbers are in the next section. The first read was through a fetch tool; the main statements were then re-checked by `grep` and reading against the downloaded copy.
- Open because of that: the MP25 `ltdc_plat_data` (pad max frequency), `LTDC_MAX_LAYER`, the device-tree nodes and the mainline comparison. The later item should redo this with a real checkout.

## Which code path is MP25

The driver is generic (`st,stm32-ltdc`; MP25 compatibles `st,stm32mp251-ltdc` and `st,stm32mp255-ltdc`). Capabilities come from the hardware version register in `ltdc_get_caps()`; the layer count is read from `LTDC_LCR`. The versions named there are `HWVER_40100` (0x040100) and `HWVER_40101` (0x040101), sharing one `case`. That the MP25 reports one of these was not confirmed (inferred: `ltdc_get_caps` runs the MP25 firewall code for every MP25 compatible and then switches on the version; `default:` returns `-ENODEV`, and only the 4.x case sets `dynamic_zorder`, `plane_rotation` and the per-layer scaler probe). MP251 and MP255 differ: only MP255 has the rotator registers and the LVDS clock.

## Line references in `ltdc.c` (commit above)

`NB_CRTC`/`CRTC_MASK` 47-48; `HWVER_*` 52-56; `FUT_DFT` 264; `ltdc_pix_fmt_a2` 315; `ltdc_drm_fmt_a2` 352; YUV lists 372-384; `ltdc_format_modifiers` 514; register gating `ltdc_readable_reg` ~589-640 and `ltdc_writeable_reg` ~710-760; `clock_tolerance` 781; `ltdc_crtc_atomic_check` 1371; `ltdc_plane_atomic_check` 1565 (scaling rules 1585-1600); `SCALER_FRACTION` 1609; 90/270 rotation path 1744-1765 (`vdisplay > 1366` at 1765); scaler register writes 1985-2010 (`LTDC_L1SHSPR` 2003, `LTDC_L1SVSPR` 2010); `ltdc_plane_create` 2151 (`possible_crtcs` 2155, YUV semi-planar gating 2198-2202, alpha 2236); primary plane and zpos 2260-2269; overlays 2308-2318; `ltdc_encoder_mode_valid` 2328; encoder created as `DRM_MODE_ENCODER_DPI` 2388; `ltdc_get_caps` 2406 (layer count 2423, firewalls 2425-2470, 4.x case 2561-2600); `st,burstlen` 2659; `rotation-memory` 2825; `st,stm32mp25-lvds` 2904.

## CRTC and planes

| Item | Finding | Basis | Where in `ltdc.c` |
| --- | --- | --- | --- |
| CRTCs | 1 (`NB_CRTC 1`). One encoder, type `DRM_MODE_ENCODER_DPI`, bridge attached | source | `NB_CRTC`, `ltdc_encoder_init` |
| Layer count | Read from `LTDC_LCR`, clamped to `1..LTDC_MAX_LAYER`. If access to the `l3` firewall is denied, one fewer and the layer is marked secured. `LTDC_MAX_LAYER` itself is in `ltdc.h`, not read | source (value open) | `ltdc_get_caps` |
| Layer count on MP25 | Probably 3 (child nodes `l1l2` and `l3` are looked up for firewalls). The number is not in what was read | inferred | `ltdc_get_caps` |
| Primary plane | Layer 0 (`DRM_PLANE_TYPE_PRIMARY`) | source | `ltdc_plane_create` |
| Overlay planes | Layers `1 .. nb_layers-1`. No cursor plane | source | `ltdc_plane_create` |
| `possible_crtcs` | `CRTC_MASK = GENMASK(NB_CRTC-1, 0)` = 0x1 for every plane | source | `ltdc_plane_create` |
| Two planes on one output | All planes sit on the one CRTC, so two hardware planes should be possible | inferred | |
| Secured layer | A layer behind a denied firewall is not created, so a board can expose fewer planes than the silicon has | source | `ltdc_get_caps` |

## Per-plane properties

| Item | Finding | Basis | Where |
| --- | --- | --- | --- |
| RGB formats (4.x list) | ARGB8888, XRGB8888, ABGR8888, XBGR8888, RGBA8888, RGBX8888, BGRA8888, BGRX8888, RGB565, BGR565, RGB888, BGR888, ARGB1555, XRGB1555, ARGB4444, XRGB4444, C8 | source | `ltdc_drm_fmt_a2` |
| Hardware encodings | Fixed codes for ARGB8888, ABGR8888, RGBA8888, BGRA8888, RGB565, BGR565, RGB888. The rest (BGR888, ARGB1555, ARGB4444, C8) go through the flexible pixel format (`PF_FLEXIBLE`, `LTDC_L1FPF0R`/`1R`) | source | `ltdc_pix_fmt_a2`, `ltdc_set_flexible_pixel_format` |
| RGB list per layer | Same on every layer: `non_alpha_only_l1` is false in the 4.x case, so X-formats stay on non-primary layers | source | `ltdc_get_caps`, `ltdc_plane_create` |
| YUV, interleaved | YUYV, YVYU, UYVY, VYUY, when the layer reports `LXCR_C1R_YIA` | source | `ltdc_drm_fmt_ycbcr_cp` |
| YUV, semi-planar and planar | NV12, NV21, YUV420, YVU420, added only for a layer with nonzero index on MP251/MP255. Comment: "Soc MP25 doesn't support YUV semiplanar and planar pixel formats on layer 1". So the primary has interleaved YUV only | source | `ltdc_drm_fmt_ycbcr_sp`/`_fp`, `ltdc_plane_create` |
| YUV encoding | BT.601 and BT.709, limited and full range, as plane properties. `/* BT2020 not supported */`; an unsupported value logs an error and falls back to BT.601 limited | source | `ltdc_ycbcr2rgb_coeffs`, `ltdc_set_ycbcr_config` |
| YUV with scaling | On a scaling-capable layer, a YUV format with source size different from destination is rejected ("Scaling is not supported with yuv pixel format") | source | `ltdc_plane_atomic_check` |
| Modifiers | `DRM_FORMAT_MOD_LINEAR` only | source | `ltdc_format_modifiers` |
| zpos | Mutable when `dynamic_zorder` (true in the 4.x case), range `0 .. nb_layers-1`, initial value is the layer index. Immutable otherwise | source | `ltdc_plane_create`, `ltdc_get_caps` |
| Plane alpha | `drm_plane_create_alpha_property` on every plane | source | `ltdc_plane_create` |
| Pixel blend mode | No property: `drm_plane_create_blend_mode_property` is not called in the file | source | whole file |
| Scaling, which layers | Per layer: `plane_scaling[i]` is set when `LTDC_L1C1R` has `LXCR_C1R_SCA` (BIT 31). Which layers have it is not in the source | source (set open) | `ltdc_get_caps` |
| Scaling, limits | Upscale only ("Downscaling is not supported"). On a layer without the scaler any size mismatch is rejected ("Scaling is not supported on layer %d"). Scaler registers are reachable only on the MP251/MP255 compatibles | source | `ltdc_plane_atomic_check`, `ltdc_readable_reg` |
| Scaler arithmetic | 12 fractional bits, factor `((in-1)<<12)/(out-1)` masked to 16 bits. In `ltdc_plane_update` the horizontal and vertical phase registers look cross-assigned (`LTDC_L1SHSPR` gets the vertical factor). Unverified, possibly a driver bug | source (reading unverified) | `calculateScalingFactor`, `ltdc_plane_update` |
| Rotation, every layer | `ROTATE_0 | REFLECT_X | REFLECT_Y` when `plane_rotation` (true in the 4.x case) | source | `ltdc_plane_create` |
| Rotation, primary only | 90/180/270 added only if `plane_rotation && crtc_rotation`. 180 is `REFLECT_X | REFLECT_Y`; 90 and 270 use the separate rotator block | source | `ltdc_plane_create` |
| `crtc_rotation` | True only if `GC2R_ROTA` is set in `LTDC_GC2R`, and cleared if the device tree has no `rotation-memory` reserved region. Rotator registers (`LTDC_RB0AR`, `RB1AR`, `RBPR`, `RIFCR`) are reachable on `st,stm32mp255-ltdc` only, not MP251 | source | `ltdc_get_caps`, `ltdc_load`, `ltdc_readable_reg` |
| Rotator constraints | 90/270 only on the primary layer. With rotation enabled other layers are refused ("Rotation enabled on primary plane, do not use another layer"). Needs a reserved buffer of at least `hdisplay*vdisplay*2*3` bytes. `vdisplay` above 1366 is refused. Buffer addresses are cast to `u32`. In the code read, the buffer-size and 1366 refusals come after `GCR_LTDCEN` was cleared, so the display stays stopped on that path | source | `ltdc_plane_update` |
| Rotation and two planes | 90/270 excludes a second plane. 0, 180 and reflection do not | inferred | |
| Size limits | No width or height cap in the capabilities or in `atomic_check`. Hardware limits are in the reference manual, not read | source (absence); limit open | `ltdc_plane_atomic_check` |
| Colour management | CRTC gamma size 256 (`CLUT_SIZE`); `C8` uses the layer CLUT | source | `CLUT_SIZE`, `ltdc_crtc_init` |

## Clock, bandwidth and bus constraints

| Item | Finding | Basis | Where |
| --- | --- | --- | --- |
| Bandwidth check | None at check time: no FIFO or bus accounting. `ltdc_crtc_atomic_check` only forces `mode_changed` when `active_changed` | source | `ltdc_plane_atomic_check`, `ltdc_crtc_atomic_check` |
| FIFO underrun | Found at run time by interrupts (`IER_FUWIE`, `IER_FUEIE`, `IER_TERRIE`, plus `IER_FURIE` with the rotator). Logged "ltdc fifo underrun: please verify display mode"; counters `fifo_err`, `fifo_warn`, `transfer_err`, `fifo_rot`. A bus-starving combination passes TEST_ONLY | source (TEST_ONLY part inferred) | `ltdc_irq*` |
| FIFO threshold | `FUT_DFT = 128`, written to `LTDC_FUT` on CRTC enable when `fifo_threshold` (true in the 4.x case) | source | `ltdc_crtc_atomic_enable` |
| Burst length | MP251/MP255 read `st,burstlen` from the device tree (`max_burst_length = mbl/8`, 0 if absent) and write it per layer to `LTDC_L1BLCR`. A board-set bus-load knob | source | `ltdc_get_caps` |
| Bus width | `8 << ((gc2r & GC2R_BW) >> 4)` | source | `ltdc_get_caps` |
| Pixel clock limit | `pad_max_freq_hz` from the match data (value not read, `drv.c`). Above it: `MODE_CLOCK_HIGH` | source (value open) | `ltdc_encoder_mode_valid` |
| Clock tolerance | Module parameter `clock_tolerance = 5` permille. The rounded clock must lie within it, except for the preferred mode, which skips the tolerance but not the pad limit | source | `ltdc_encoder_mode_valid` |
| Clocks | MP251/MP255: `bus`. MP255 only: `lvds` (when an `st,stm32mp25-lvds` node is available), `ref` as pixel clock, `lcd` as LTDC clock. Pixel clock is reparented to the LVDS clock for an LVDS encoder; rate set to `mode->clock * 1000` | source | `ltdc_get_clk`, `ltdc_crtc_atomic_enable` |
| Encoder type vs LVDS | `ltdc_encoder_init` always creates `DRM_MODE_ENCODER_DPI`, yet `ltdc_encoder_mode_valid` tests for `DRM_MODE_ENCODER_LVDS`. Which route the board uses is open | inferred | |
| Memory bandwidth property | Not seen in `ltdc.c`. Binding and device tree not read | open | |

## What this means for the harness and planner

| Item | Finding | Basis |
| --- | --- | --- |
| Capability detection | Plane properties (`zpos` range, `alpha`, rotation mask) reflect silicon and firewall. There is no blend-mode property, so blend cannot be selected | source |
| Planner | Scaling is per layer and up-only. Which layer has a scaler is not visible from properties, so a downscale or a scaled YUV frame fails in the driver rather than by capability | inferred |
| TEST_ONLY | Catches the scaling rules (in `atomic_check`). The 90/270 buffer-size and 1366 checks are in `ltdc_plane_update`, past TEST_ONLY, so TEST_ONLY can pass and the commit then leaves the display stopped | inferred |
| Underrun | Cannot be seen before commit. The board session should read the kernel log for `fifo underrun` | inferred |

## Mainline difference

Not checked: mainline `drivers/gpu/drm/stm/ltdc.c` could not be fetched. Open for the later item: MP25 compatibles and the 4.x caps in mainline, the rotator and `rotation-memory`, `st,burstlen`, `dynamic_zorder`, `plane_scaling`. Do not carry any mainline claim into `platform-capabilities.md` until it has been read.

## Open for the board session

- Real layer count, per-layer scaler presence, `zpos` range and `possible_crtcs` from the probe, replacing the inferred rows.
- Whether the output is LVDS, DSI or HDMI through a bridge, and its mode.
- Kernel log for `fifo underrun` with two planes and a scaled layer active.
