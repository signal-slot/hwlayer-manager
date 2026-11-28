# BCM2712 HDMI clock limits and the mode refusal path from the vc4 source (HVS Q10)

Answers the source half of question 10 of [`hvs-backend-design.md`](hvs-backend-design.md) §7:
what the HDMI limits of the Raspberry Pi 5 (BCM2712) are. The measured answer stays with the RPi5
`mode_test` session. No board was used for this document.

Source: raspberrypi/linux, branch `rpi-6.18.y`, commit
`16ae2649dcfadf9d81445b1177a3191efa3cb82e`, files `drivers/gpu/drm/vc4/vc4_hdmi.c` and
`drivers/gpu/drm/vc4/vc4_hvs.c`.

Tags: **read from source** (commit, file, function) · **inferred**. No row is observed.

**Caveat on access and line numbers.** The shared checkout in the dependency cache held only an
empty `drivers/gpu/drm/vc4` directory and an empty git repository without a remote. Adding a remote
for the documented shallow sparse fetch, and a direct download with `curl`, were both refused. The
two files were then read as raw text of the pinned commit through a web fetch, which returns text
without line numbers. Every row therefore names the file and function, not a `file:line`; find each
with `grep -n` on the function or variant name. Not read: `vc4_hdmi.h` (the definition of
`struct vc4_hdmi_variant`), the DRM core helper `drm_hdmi_connector_mode_valid`, and the bytes of
`vc4_hdmi.c` at the boundary between two fetch chunks (about three lines).

## 1. Maximum clock per port (a)

| # | Statement | Where (rpi-6.18.y 16ae2649) | Tag |
|---|---|---|---|
| H1 | `bcm2712_hdmi0_variant` has `.max_pixel_clock = 600000000` (600 MHz), `.encoder_type = VC4_ENCODER_TYPE_HDMI0`, `.card_name = "vc4-hdmi-0"`, `.registers = vc6_hdmi_hdmi0_fields`, `.phy_init = vc6_hdmi_phy_init`, `.supports_hdr = true` | `vc4_hdmi.c`, `bcm2712_hdmi0_variant` | read from source |
| H2 | `bcm2712_hdmi1_variant` is the same as H1 except the encoder type (HDMI1), names (`hdmi1_regs`, `vc4-hdmi-1`) and the register table (`vc6_hdmi_hdmi1_fields`). Its `.max_pixel_clock` is also `600000000`. So both BCM2712 ports carry the same per-variant limit | `vc4_hdmi.c`, `bcm2712_hdmi1_variant` | read from source |
| H3 | `HDMI_14_MAX_TMDS_CLK` is `(340 * 1000 * 1000)` | `vc4_hdmi.c`, macro near the top | read from source |
| H4 | For comparison, BCM2711: `bcm2711_hdmi0_variant.max_pixel_clock` = 600000000, `bcm2711_hdmi1_variant.max_pixel_clock` = `HDMI_14_MAX_TMDS_CLK`; `bcm2835_variant` = 162000000. So on the CM4/Pi 4 only HDMI0 may go above 340 MHz; on BCM2712 both may | `vc4_hdmi.c`, the variant tables | read from source |
| H5 | The field name is `max_pixel_clock`, but the value is compared with a TMDS character rate (H6), not with `mode->clock` alone. For 8 bpc RGB the two are equal; at higher bpc or in YCbCr 4:2:0 the compared rate differs | `vc4_hdmi.c`, `vc4_hdmi_connector_clock_valid`; rate computation is in the DRM core | inferred (the core helper was not read) |
| H6 | `vc4_hdmi_bind` sets `vc4_hdmi->scdc_enabled = true` when `variant->max_pixel_clock > HDMI_14_MAX_TMDS_CLK`, so both BCM2712 ports enable SCDC handling | `vc4_hdmi.c`, `vc4_hdmi_bind` | read from source |

## 2. The refusing function (b)

| # | Statement | Where | Tag |
|---|---|---|---|
| R1 | The clock check is `vc4_hdmi_connector_clock_valid(connector, mode, clock)`, registered as `.tmds_char_rate_valid` in `vc4_hdmi_hdmi_connector_funcs`. It returns the status `MODE_CLOCK_HIGH` (not an errno) in three cases: (1) `clock > variant->max_pixel_clock`; (2) `!hvs->vc5_hdmi_enable_hdmi_20 && clock > HDMI_14_MAX_TMDS_CLK`; (3) `!hvs->vc5_hdmi_enable_4096by2160 && mode->hdisplay > 3840 && mode->vdisplay >= 2160 && drm_mode_vrefresh(mode) >= 50`. Otherwise `MODE_OK` | `vc4_hdmi.c`, `vc4_hdmi_connector_clock_valid`, `vc4_hdmi_hdmi_connector_funcs` | read from source |
| R2 | The connector helper sets `.mode_valid = drm_hdmi_connector_mode_valid`, the DRM core HDMI state helper, which calls `tmds_char_rate_valid`. So a too-fast mode is dropped from the connector's mode list (`MODE_CLOCK_HIGH` marks it bad in the probe) | `vc4_hdmi.c`, `vc4_hdmi_connector_helper_funcs` | read from source (the helper itself) / inferred (that it calls R1 and prunes the mode) |
| R3 | `vc4_hdmi_encoder_atomic_check` has no maximum-clock comparison. It returns `-EINVAL` only for odd horizontal timings: when `unsupported_odd_h_timings`, or `unsupported_int_odd_h_timings` with `DRM_MODE_FLAG_INTERLACE`, is set, and `hdisplay`, `hsync_start`, `hsync_end` or `htotal` is odd. BCM2712 has `unsupported_odd_h_timings = false` and `unsupported_int_odd_h_timings = true`, so only interlaced modes with odd horizontal timings are refused there | `vc4_hdmi.c`, `vc4_hdmi_encoder_atomic_check`, the BCM2712 variants | read from source |
| R4 | `vc4_hdmi_encoder_mode_valid` has no clock check. It returns `MODE_H_ILLEGAL` for odd horizontal timings only when `unsupported_odd_h_timings` is set; BCM2712 clears it, so the function returns `MODE_OK` for BCM2712 | `vc4_hdmi.c`, `vc4_hdmi_encoder_mode_valid` | read from source |
| R5 | A mode refused by R1 or R2's helper through the mode list is also what a TEST_ONLY atomic commit with a user-supplied mode sees: the DRM core runs `mode_valid` over the connector during `drm_atomic_helper_check_modeset`, so the commit returns `-EINVAL` | DRM core, not read | inferred |

## 3. 4K60 and firmware gating (c)

| # | Statement | Where | Tag |
|---|---|---|---|
| G1 | `hvs->vc5_hdmi_enable_hdmi_20` and `hvs->vc5_hdmi_enable_4096by2160` are set in `vc4_hvs_bind` when `vc4->gen >= VC4_GEN_5` (BCM2711 and BCM2712). The driver reads `rpi_firmware_clk_get_max_rate(firmware, RPI_FIRMWARE_CORE_CLK_ID)`: `max_rate >= 550000000` sets `vc5_hdmi_enable_hdmi_20`, `max_rate >= 600000000` sets `vc5_hdmi_enable_4096by2160`. The value is the firmware's maximum core clock, not a DT property | `vc4_hvs.c`, `vc4_hvs_bind` | read from source |
| G2 | `vc4_hvs_bind` returns `-EINVAL` without a firmware node and `-EPROBE_DEFER` while the firmware is not ready, so the gating depends on the firmware mailbox being present | `vc4_hvs.c`, `vc4_hvs_bind` | read from source |
| G3 | `vc4_hdmi_connector_get_modes` warns "Please change your config.txt file to add hdmi_enable_4kp60" when `vc5_hdmi_enable_hdmi_20` is false and a mode needs scrambling. The code never reads a config.txt key itself | `vc4_hdmi.c`, `vc4_hdmi_connector_get_modes` | read from source |
| G4 | Therefore clocks above 340 MHz on BCM2712 depend on the firmware's maximum core clock being at least 550 MHz, and 4096x2160 at 50 Hz or more on at least 600 MHz. That `hdmi_enable_4kp60` in config.txt raises that firmware maximum is not shown by this code | `vc4_hvs.c`, `vc4_hdmi.c` | inferred |
| G5 | The source gives no value of the firmware core-clock maximum on a Pi 5, and says nothing about whether the stock firmware reports at least 600 MHz | — | not determined from source |

## 4. What the RPi5 `mode_test` session should see

Predictions only; none is a measurement.

- **Boundary by the TMDS rate.** If the firmware reports a core-clock maximum of at least
  600 MHz (G1), then modes up to 600 MHz at 8 bpc RGB are accepted on both HDMI0 and HDMI1,
  including 3840x2160 at 60 Hz (594 MHz) and 4096x2160 at 60 Hz. Modes with a TMDS rate above
  600 MHz are refused (H1, R1 case 1).
- **If the firmware maximum is between 550 and 600 MHz:** 3840x2160 at 60 Hz is accepted, 4096x2160
  at 50 Hz or more is refused (R1 case 3).
- **If it is below 550 MHz:** every mode above 340 MHz is refused, so 3840x2160 at 60 Hz is out
  and 3840x2160 at 30 Hz stays (R1 case 2). The HDMI0 and HDMI1 boundary is the same, unlike on
  BCM2711 (H4).
- **Refusal form.** The kernel status is `MODE_CLOCK_HIGH` (R1): the mode would be missing from the
  connector's list. A TEST_ONLY commit of such a mode by hand is predicted to fail with `-EINVAL`
  (R5, inferred).
- **Interlaced modes with odd horizontal timings** are refused by the encoder `atomic_check` with
  `-EINVAL` (R3). Other modes are not refused by it.
- **Check to run first:** note the largest accepted TMDS rate; if it is at most 340 MHz the
  firmware core-clock maximum (G1) is the cause, not the port. Record the core clock maximum next
  to it if the session can read it without changing configuration.
