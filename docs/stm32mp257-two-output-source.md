# STM32MP257 LTDC: can DSI and LVDS run at once? Read from source for WP5-07

Date of reading: 2026-11-28. This is **portability evidence only**. It counts toward neither M1, M2 nor M3.
**Nothing here is a board measurement.** The only board facts are quoted from the existing Q8 session file and
carry its tag and date.

## Verdict

**(b) Independent outputs are impossible on this LTDC driver.** The driver creates one CRTC. DSI and LVDS cannot show
different pictures or run at different times on separate CRTCs. This is **read from source**.

**(a) A clone of one CRTC to both outputs is not supported by the driver.** Both drivers set `possible_clones = 0`
("No cloning support"). The pixel clock also has one parent at a time. So (a) does not hold as the code stands.

**(c) is open only for the hardware, not for the driver.** Whether the silicon could clock a DSI and an LVDS output from
one LTDC at the same time is in the reference manual, which was not read. The driver does not use that ability in any
case.

In one line: at most one of the DSI→HDMI route and the LVDS route is active at a time, selected by which connector the
single CRTC is attached to.

## Tags and pins

- **read from source**: stated by a file of ST's public kernel tree. Branch `v6.18-stm32mp` of
  `STMicroelectronics/linux`, tip commit `3056405e0479330b0889c2a971452bfd10014b70` (pinned in
  [`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md)). Pin on GitHub:
  <https://github.com/STMicroelectronics/linux/tree/3056405e0479330b0889c2a971452bfd10014b70/drivers/gpu/drm/stm>.
- **inferred**: a conclusion drawn from source rows. It is not stated by any file.
- **observed 2026-11-28**: only for rows quoted from
  [`stm32mp257-ltdc-session-c-outputs-2026-11-28.md`](stm32mp257-ltdc-session-c-outputs-2026-11-28.md) (Q8). Nothing
  new was observed for this document.

How it was read: a local clone was refused by the permission layer (same as in the earlier source documents), so the
files were read through web-fetch agents, which returned the text but **no line numbers and no commit SHA**. The
references below are therefore function names, not lines. That the fetched text is commit `3056405e` is **inferred**
(the branch name matches; the tip was not re-checked). `lvds.c` was only readable up to about line 1000 of 1308: the
`lvds_probe` function, the bridge ops and the `of_device_id` table were **not read**. The reference manual and the RCC
clock tree were not read.

The board runs kernel 6.6.129 (observed 2026-11-28, Q8 file). Everything below is ST **v6.18**. The v6.6 branch was not
re-read for this question, and its `ltdc.c` differs in the MP25 compatible (`st,stm32mp25-ltdc`, see
[`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md), "ST 6.6 note"). The LVDS clock code below sits
under `st,stm32mp255-ltdc` in v6.18, so whether the board's kernel behaves the same is **inferred**, not known.

## Claims

### How many CRTCs, encoders and bridges

| # | Claim | Tag | Where (ST v6.18, `3056405e`) |
|---|---|---|---|
| 1 | The driver has `#define NB_CRTC 1` and `CRTC_MASK GENMASK(NB_CRTC - 1, 0)`, so mask `0x1`. `ltdc_load()` allocates one `drm_crtc` with `drmm_kzalloc` and calls `ltdc_crtc_init()` once, then `drm_vblank_init(ddev, NB_CRTC)`. | read from source | `drivers/gpu/drm/stm/ltdc.c`, top of file; `ltdc_load()`; `ltdc_crtc_init()` |
| 2 | Every plane is created with `possible_crtcs = CRTC_MASK`, and `ltdc_crtc_init()` adds the overlays to the same CRTC. | read from source | `ltdc_plane_create()`, `ltdc_crtc_init()` |
| 3 | `ltdc_load()` reads the endpoints of **port 0 only**: `of_graph_get_endpoint_count(np)`, then for each index `i` it calls `drm_of_find_panel_or_bridge(np, 0, i, &panel, &bridge)`. `-ENODEV` skips the endpoint, any other error (for example `-EPROBE_DEFER`) stops the probe. A panel is wrapped with `drmm_panel_bridge_add`. | read from source | `ltdc_load()` |
| 4 | Each endpoint that gives a bridge gets its own encoder from `ltdc_encoder_init()`: type `DRM_MODE_ENCODER_DPI`, `possible_crtcs = CRTC_MASK`, **`possible_clones = 0` with the comment "No cloning support"**, then `drm_bridge_attach(encoder, bridge, NULL, 0)`. So N working endpoints give N encoders and still one CRTC. | read from source | `ltdc_encoder_init()` |
| 5 | The LVDS bridge changes its encoder's type to `DRM_MODE_ENCODER_LVDS` in `lvds_attach()`, and sets `encoder->possible_clones = 0` again ("No cloning support"). | read from source | `drivers/gpu/drm/stm/lvds.c`, `lvds_attach()` |
| 6 | `ltdc_crtc_atomic_enable()` finds **one** encoder: the first one in `drm_for_each_encoder` whose `->crtc` is this CRTC (then `break`). It takes that encoder's bridge, bus format and connector. There is no loop over several encoders. | read from source | `ltdc_crtc_atomic_enable()` |
| 7 | The DRM core rejects an atomic state that puts two encoders on one CRTC when they are not in each other's `possible_clones`. With `possible_clones = 0` on both, a DSI and an LVDS connector on the one CRTC would fail the check. This is a DRM core rule that was **not read in this task** (no kernel core file was opened). | inferred | DRM core (`drm_atomic_helper_check_modeset`), not read |
| 8 | With claims 1, 4, 5 and 7: no independent outputs (one CRTC, one timing generator in the driver) and no clone (declined by both drivers). | inferred | |

### Pixel clock and the DSI/LVDS selection

| # | Claim | Tag | Where |
|---|---|---|---|
| 9 | On `st,stm32mp255-ltdc`, `ltdc_get_clk()` takes `pixel_clk` = clock `"ref"` (the `<&syscfg>` entry), `ltdc_clk` = `"lcd"` (`CK_KER_LTDC`) and, only if an LVDS node is available, `lvds_clk` = `"lvds"` (the `<&lvds>` clock). It looks the node up with `of_find_compatible_node(.., "st,stm32mp25-lvds")` and `of_device_is_available`. On other compatibles `pixel_clk` is `"lcd"`. | read from source | `ltdc_get_clk()` |
| 10 | In `ltdc_crtc_atomic_enable()` the driver calls `clk_set_parent(ldev->pixel_clk, ldev->lvds_clk)` if the one encoder on the CRTC has type `DRM_MODE_ENCODER_LVDS`, else `clk_set_parent(ldev->pixel_clk, ldev->ltdc_clk)`. Then it calls `clk_set_rate(ldev->pixel_clk, mode->clock * 1000)`. `pixel_clk` has **one** parent at a time, so the LTDC is clocked from the LVDS PHY clock **or** from the kernel LTDC clock, never from both. | read from source | `ltdc_crtc_atomic_enable()` |
| 11 | `ltdc_crtc_atomic_disable()` sets the parent of `pixel_clk` back to `ltdc_clk`. `ltdc_load()` does the same for `st,stm32mp255-ltdc` when the LTDC was not started by the bootloader. | read from source | `ltdc_crtc_atomic_disable()`, `ltdc_load()` |
| 12 | `ltdc_encoder_mode_valid()` rounds the rate on `lvds_clk` for an LVDS encoder and on `ltdc_clk` otherwise, and rejects a result above `pad_max_freq_hz` (150 MHz on MP25) with `MODE_CLOCK_HIGH`. It accepts the preferred mode, otherwise the rounded rate must be within the `clock_tolerance` (default 5 per mille). | read from source | `ltdc_encoder_mode_valid()`, `clock_tolerance` |
| 13 | The LVDS driver registers its own clock `clk_pix_lvds` (parent `ck_ker_lvdsphy`) and offers it as a clock provider (`#clock-cells = <0>`). `lvds_pixel_clk_set_rate()` only stores the rate; the PHY PLL runs at 7 times the pixel clock (halved per link in dual link). The PLL is programmed from the stored rate in `lvds_pll_config()` and enabled in `lvds_pixel_clk_enable()`. | read from source | `lvds.c`, `lvds_pixel_clk_register()`, `lvds_pixel_clk_set_rate()`, `lvds_pll_config()` |
| 14 | The DSI node takes three clocks `pclk`, `ref` (`CK_KER_DSIPHY`) and `px_clk`, and `px_clk` is `CK_KER_LTDC`, the **same** clock the LTDC calls `lcd`. The DSI driver registers a clock too (`ck_dsi_phy` or `txbyteclk`, depending on the DSI IP version) and sets its PLL from `mode->clock`, bits per pixel and lanes. | read from source | `stm32mp255.dtsi` `dsi` node; `dw_mipi_dsi-stm.c`, clock registration and `get_lane_mbps` |
| 15 | The pixel path into the DSI host is the `px_clk` = `CK_KER_LTDC` clock (claim 14), and `pixel_clk` is reparented to the LVDS clock when LVDS is selected (claim 10). So with LVDS selected, the LTDC pixels are timed by the LVDS clock, while the DSI input still sees `CK_KER_LTDC`. Whether that gives a usable DSI picture was not read from any file (it needs the clock tree in the RCC and the reference manual). | inferred | claims 10 and 14 |
| 16 | No `syscon`, `syscfg` register write, regmap mux or "output select" register appears in `ltdc.c`, and none in the part of `lvds.c` or `dw_mipi_dsi-stm.c` that was read (about the first 1000 lines of each). The only selector the LTDC driver uses is the clock parent above. The `st,syscon` property seen on the board's LTDC node (observed 2026-11-28, Q8 file) is not read by `ltdc.c`; its use elsewhere was not searched. | read from source (`ltdc.c`; partial for the other two) | files named |
| 17 | Both pixel-clock paths are configured by one `clk_set_rate(pixel_clk, ...)`. The driver never sets a rate on `lvds_clk` or `ltdc_clk` itself. | read from source | `ltdc_crtc_atomic_enable()` |

### Device tree

| # | Claim | Tag | Where |
|---|---|---|---|
| 18 | The binding says the LTDC has one `port` with up to two endpoints: external DPI RGB, or the internal DPI input of the MIPI DSI host. "**Note: These 2 endpoints cannot be activated simultaneously.**" The text names the DPI/DSI pair. The LVDS case is not named, but the LVDS input is wired to the second endpoint (`ltdc_ep1_out`) in the board file and in the LVDS binding example. | read from source | `Documentation/devicetree/bindings/display/st,stm32-ltdc.yaml`, `port`; `st,stm32mp25-lvds.yaml`, example |
| 19 | `stm32mp255.dtsi` has `&ltdc` with `compatible = "st,stm32mp255-ltdc"` and clocks `lcd`, `bus`, `ref` (`<&syscfg>`) and `lvds` (`<&lvds>`). `dsi@48000000` and `lvds@48060000` are `status = "disabled"` there. No `assigned-clocks` or `assigned-clock-parents` appear in the file. | read from source | `stm32mp255.dtsi` |
| 20 | The ST board file `stm32mp257f-dk.dts` has `&ltdc` `status = "okay"` with `ltdc_ep0_out` (endpoint 0, to `dsi_in`) and `ltdc_ep1_out` (endpoint 1, to `lvds_in`), `&dsi` `okay`, `&lvds` `okay` with `port@1` to the panel, and the panel `panel_lvds` (EDT ETML0700Z9NDHA, 1024x600, 54 MHz). So ST's own board file enables both endpoints, although the binding says they cannot be active together. | read from source | `stm32mp257f-dk.dts`, `&ltdc`, `&dsi`, `&lvds`, `panel_lvds` (the file was decoded by hand from a base64 fetch, so quote marks are exact only for the keys named) |
| 21 | Per claims 3 and 9: an endpoint whose remote node is disabled gives `-ENODEV` from `drm_of_find_panel_or_bridge` and is skipped, so no encoder is created for it. With the LVDS node disabled, `lvds_clk` is also not requested. | inferred (from claims 3 and 9; the `-ENODEV` result is the code's own comment, the DRM helper itself was not read) | `ltdc_load()`, `ltdc_get_clk()` |
| 22 | The LVDS binding lists "Single-Display or Double-Display (with the same content duplicated on both)" and "Single-Link or Dual-Link". These are the two LVDS PHYs of the **LVDS block**, with output `port@1` and the optional `port@2` ("sink for odd or even pixels"). They are not a DSI plus LVDS clone. `lvds.c` has link types for single primary, single secondary and two dual-link pixel orders; the code that sets `link_type` from the ports was not read. | read from source (binding, link-type enum); `link_type` assignment not read | `st,stm32mp25-lvds.yaml`; `lvds.c`, `enum lvds_link_type` |

### Constraints on the LVDS route

| # | Claim | Tag | Where |
|---|---|---|---|
| 23 | The LVDS connector's `atomic_check` returns `-EINVAL` unless the CRTC mode has the panel's `hdisplay` and `vdisplay`, and then copies the panel mode into `adjusted_mode`. So LVDS runs only the panel mode, 1024x600 at 54 MHz here (panel timing in the board file). | read from source | `lvds.c`, `lvds_connector_atomic_check()`; `panel_lvds` `panel-timing` |
| 24 | The LVDS encoder type matters only for the clock parent: `ltdc_crtc_atomic_enable()` also calls `pinctrl_pm_select_default_state` for `DRM_MODE_ENCODER_DPI` and the sleep state for others, so neither DSI nor LVDS uses the LTDC pads. | read from source | `ltdc_crtc_atomic_enable()` |

## What the board showed (quoted, not new)

From the [Q8 file](stm32mp257-ltdc-session-c-outputs-2026-11-28.md), **observed 2026-11-28** (host UTC 01:04Z to 01:06Z,
read only):

- One CRTC (id 41), one encoder (id 31) with a DSI host plus ADV7535 bridge chain, one connector `HDMI-A-1`. This matches
  claims 1 and 4 for a tree with one live endpoint.
- `lvds@48060000`, `panel-lvds` and `panel-lvds-backlight` are `disabled`. `dsi@48000000` and the ADV7535 are `okay`.
- The LTDC `port` has two endpoints, `endpoint@0` to the DSI input and `endpoint@1` to the LVDS input. The LVDS endpoint
  is wired but its remote node is disabled, which fits claim 21 (inferred).
- The kernel is 6.6.129, the LTDC `compatible` is `st,stm32mp25-ltdc`, and the LVDS `compatible` is `st,stm32mp25-lvds`.

## Conditions, if someone wanted to try

1. **Device-tree change.** The LVDS route needs `lvds`, `panel-lvds` and its backlight `okay`. These are `disabled` on the
   board (observed 2026-11-28). The LTDC endpoints already exist. That is a change to the device tree on the boot media,
   and it is **outside the device rules** of this plan.
2. **Which kernel code.** The clock code in claims 9 to 11 is under `st,stm32mp255-ltdc` in v6.18. The board uses
   `st,stm32mp25-ltdc` on 6.6.129. The v6.6 code for this was not read, so the behaviour on the board is **inferred
   only**.
3. **Clock.** One pixel clock (claim 10), one parent at a time. For LVDS the PHY PLL runs at 7 times the pixel clock
   (claim 13), so the rate on this route is whatever `lvds_pixel_clk_round_rate` gives. For DSI it is `CK_KER_LTDC`
   (claim 14). Both are limited to 150 MHz at the pad (claim 12). Whether the two can sit on one PLL tree at once is
   in the RCC and the reference manual, not read.
4. **Same mode.** A clone, if the driver allowed it, would need one mode on both outputs. LVDS accepts only the panel
   mode, 1024x600 at 54 MHz (claim 23). The HDMI side would have to accept that mode from the ADV7535. That is
   **inferred**; it was not read. The observed HDMI mode is 1920x1080 at 148.5 MHz (Q8 file).
5. **Driver change.** For (a), `possible_clones` would have to be non-zero on both encoders, `ltdc_crtc_atomic_enable()`
   would have to handle two encoders, and the clock parent would need a rule for two sinks. That is a kernel change, not a
   configuration. It is a statement of what the source lacks. It is not a recommendation.

## Consequence for WP5-07

- **Source answer:** two displays at once, as independent outputs, are not possible on this LTDC driver (one CRTC). A
  clone is declined by the drivers. The expected behaviour with both routes enabled is: two connectors exist, but only
  one can be active on the single CRTC. Switching between them goes through a modeset, and the pixel clock parent
  changes with it (claims 10 and 11). That is **inferred**; no board has shown it.
- **A board session needs the owner.** To see anything beyond Q8 the LVDS route must exist, and that needs a device-tree
  change on the boot media. A device-tree change is outside the device rules (no change to boot media, U-Boot, DRM, udev
  or seat configuration, no reboot, no flashing). Without it there is one connector, and nothing to test for two
  outputs. Q8 already answered that.
- **If the owner enables the LVDS route**, the session should be small and should record, in this order:
  1. `modetest -c` and `-e`: how many connectors and encoders, and the `possible_clones` of each encoder (source says 0);
  2. an atomic `TEST_ONLY` commit with both connectors on CRTC 41 (expected to fail, claim 7);
  3. each connector alone: the LVDS panel at 1024x600, the HDMI at 1920x1080;
  4. a switch from one to the other, with the host UTC time for each step and a check that `fifo_underrun_error` stays 0.
  The harness connector names are in [`stm32mp257-board-scenarios.md`](stm32mp257-board-scenarios.md). Results would go
  into a new dated session file, not into this one.
- **Not needed:** a clone or an independent-output test without the owner change. The source does not support a clone,
  so a failing `TEST_ONLY` would be the expected result and would not change the verdict.
- **What this does not settle:** the hardware question in (c) (reference manual), and the v6.6 kernel code.
