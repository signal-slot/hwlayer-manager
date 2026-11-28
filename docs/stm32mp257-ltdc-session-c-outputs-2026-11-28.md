# STM32MP257F-DK (LTDC): session C, other outputs (Q8) and scaler layers per variant (Q4), 2026-11-28

Q8 and Q4 of the open questions in
[`design/stm32mp257-backend.md`](design/stm32mp257-backend.md) §7. The picture
items of session C (Q1, Q6, NV12, plane 34 scaling) are in a separate file.
Earlier sessions: [`stm32mp257-ltdc-session-2026-11-28.md`](stm32mp257-ltdc-session-2026-11-28.md).
Background reading: [`observed/stm32mp257-topology.md`](observed/stm32mp257-topology.md),
[`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md),
[`stm32mp257-ltdc-source-diff-6.6.md`](stm32mp257-ltdc-source-diff-6.6.md).

This is **portability evidence only**. It counts toward neither M1, M2 nor M3.

## Tags, clock and conditions

- **observed 2026-11-28**: seen on the board over ssh, read only. The host UTC
  time of the Q8 reads was 2026-11-28T01:04Z to 2026-11-28T01:06Z (`date -u` on
  the host). The board clock was 2026-11-27T15:11Z at the same moment, about
  10 h behind and not NTP-synchronised; board times are not used.
- **read from source**: stated by a file of ST's public kernel tree, read on
  2026-11-28 through a web-fetch agent (no local checkout: the clone was
  refused by the permission layer). The agent reported function-level
  positions and file lengths, not line numbers, and no commit SHA for either
  branch. The pins are therefore the branch names with the file lengths seen
  (`ltdc.c`: 2941 lines on `v6.18-stm32mp`, 2943 on `v6.6-stm32mp`, which match
  the files in `stm32mp257-ltdc-source-full.md`), and the tip commits recorded
  there (`v6.18-stm32mp` `3056405e0479330b0889c2a971452bfd10014b70`,
  `v6.6-stm32mp` `548f960c059bc4b9165cb69895fd67551f6061ca`). That the fetched
  files are those commits is **inferred**.
- **inferred**: a conclusion drawn from the above.
- Board kernel: `uname -r` = `6.6.129`.
- Read only, Weston left running: `weston-graphical-session` and
  `seatd-weston` were `active` before and after the reads (checked 2026-11-28
  around 01:04Z and 01:05Z). No DRM-master program was run, nothing was
  copied to the board, no module was loaded, no file was changed.

## Q8: other outputs

### DRM objects

| Item | Result | Tag |
|---|---|---|
| Connectors | One: `HDMI-A-1` (id 32), `status` connected, `enabled`, modes 1920x1080 (several). `/sys/class/drm` holds only `card0` and `card0-HDMI-A-1` | observed 2026-11-28 |
| Encoders | One (id 31). `bridge_chains` in debugfs shows it with a two-bridge chain: bridge 0 type 0 = the DSI host `dsi@48000000` (`st,stm32mp25-dsi`), bridge 1 type 11 (HDMI-A) = the ADV7535 at `hdmi@3d` (`adi,adv7535`) on I2C | observed 2026-11-28 |
| CRTCs, `possible_crtcs` | One CRTC (id 41); in the atomic state every plane has `crtc-0` available; connector mask 1, encoder mask 1, plane mask 1. The `possible_crtcs` of the encoder is not shown by sysfs or by `state`; the single CRTC and `encoder_mask=1` imply `0x1` | observed 2026-11-28 (masks), inferred (encoder `possible_crtcs`) |
| State | Plane 34 on `crtc-0` showing a 1920x1080 XR24 buffer from Weston; planes 42 and 49 idle. Mode 1920x1080 at 60 Hz, 148500 kHz. `transfer_error=0`, `fifo_underrun_error=0`, `fifo_underrun_warning=0` | observed 2026-11-28 |
| DRM clients | Only `seatd` holds master (Weston runs under it) | observed 2026-11-28 |
| Other connectors, encoders | None: no LVDS, DSI or DPI connector exists in `/sys/class/drm` or in the debugfs `state` | observed 2026-11-28 |

The tidy `possible_crtcs` per encoder would need `modetest -e`. That was not
run: it is not necessary, and the rule was to stay read only without
DRM-master programs.

### Device tree (`/proc/device-tree`)

| Node | `compatible` | `status` | Tag |
|---|---|---|---|
| `display-controller@48010000` (LTDC) | `st,stm32mp25-ltdc` | okay | observed 2026-11-28 |
| `dsi@48000000` | `st,stm32mp25-dsi` | okay | observed 2026-11-28 |
| `i2c@40130000/hdmi@3d` | `adi,adv7535` | okay | observed 2026-11-28 |
| `lvds@48060000` | `st,stm32mp25-lvds` | **disabled** | observed 2026-11-28 |
| `panel-lvds` | `edt,etml0700z9ndha`, `panel-lvds` | **disabled** | observed 2026-11-28 |
| `panel-lvds-backlight` | `pwm-backlight` | **disabled** | observed 2026-11-28 |
| `connector` with `hdmi-pwr-supply` | HDMI-A connector node | present | observed 2026-11-28 |

LTDC details, observed 2026-11-28: `port` has two endpoints, `endpoint@0`
(remote phandle `0x50`, the DSI input `dsi_in`) and `endpoint@1` (remote
`0x51`, the LVDS input `lvds_in`). The labels `ltdc_ep0_out`, `ltdc_ep1_out`,
`dsi_in`, `dsi_out1`, `lvds_in`, `lvds_out0`, `lvds_panel_in`,
`hdmi_connector_in` exist in `__symbols__`. The LTDC node has three firewall
children `l1l2`, `l3`, `rot`, each with `access-controllers` and
`access-controller-names`, and the node itself has `access-controller-names`
`cmn`. The reserved-memory nodes `ltdc-sec-layer@fe800000` and
`ltdc-sec-rotation@ff000000` exist (the `ltdc_sec_layer` and `ltdc_sec_rotation`
labels). The LTDC node has an `st,syscon` property, and no `rotation-memory`
property under that name was seen in the listing (the secured rotation memory
is a reserved-memory node instead; whether the driver finds it was not
checked here).

### Modules for display (`lsmod`, read only)

Loaded, observed 2026-11-28: `stm_drm`, `dw_mipi_dsi_stm`, `dw_mipi_dsi`,
`adv7511` (the ADV7535 driver, with `cec`), `drm_dma_helper`, and `galcore`
(the Vivante GPU, 5 users). No `lvds`, panel or backlight module appears in the
list. `dw_mipi_dsi_stm` has the platform driver directory `stm32-display-dsi`
and the device `48000000.dsi` is bound; `48010000.display-controller` is the
LTDC device. No `48060000` LVDS device is among the platform devices.

### Is there an LVDS or DSI route, and is it enabled?

| Question | Answer | Tag |
|---|---|---|
| DSI route | **Exists and is live**: LTDC, DSI host `dsi@48000000`, ADV7535, the HDMI-A connector. The `bridge_chains` file shows exactly this chain, and `HDMI-A-1` is connected at 1920x1080 | observed 2026-11-28 |
| LVDS route | **Exists in the device tree and is off**: the LTDC `endpoint@1` points at `lvds_in`, but `lvds@48060000`, `panel-lvds` and its backlight are `disabled`, no LVDS device is bound and no connector appears. It cannot be enabled without a device-tree change, which this task does not make | observed 2026-11-28 |
| Both at once | The binding says the two LTDC endpoints "cannot be activated simultaneously"; on this image only DSI is enabled | read from source (binding, per `stm32mp257-ltdc-source-full.md`), observed 2026-11-28 (the state) |
| Other outputs (bare DPI) | None: no connector besides `HDMI-A-1` | observed 2026-11-28 |
| Implication for the backend | The LTDC encoder is seen by DRM as one encoder with a DSI bridge chain and one connector, so the backend needs one output and no output selection on this image | inferred |

The board's `st,stm32mp25-*` compatibles (one MP25 string for LTDC, DSI and
LVDS) are the 6.6 style; ST's 6.18 tree uses `st,stm32mp251-ltdc` and
`st,stm32mp255-ltdc`. This agrees with `stm32mp257-ltdc-source-diff-6.6.md`
rows 72 and 27.

## Q4: which layers have a scaler on other STM32MP25 variants

Sources: `STMicroelectronics/linux`, branches `v6.18-stm32mp` and
`v6.6-stm32mp`, files `drivers/gpu/drm/stm/ltdc.c`,
`arch/arm64/boot/dts/st/stm32mp251.dtsi`, `stm32mp253.dtsi`, `stm32mp255.dtsi`,
`stm32mp257.dtsi`, `stm32mp25xc.dtsi`, `stm32mp25xf.dtsi`, `stm32mp257f-dk.dts`,
and `Documentation/devicetree/bindings/display/st,stm32-ltdc.yaml` (pins under
"Tags, clock and conditions").

| Finding | Result | Tag |
|---|---|---|
| How the driver learns which layer has a scaler | At probe, in `ltdc_get_caps()` inside `case HWVER_40100: case HWVER_40101:`, it reads `LTDC_L1C1R + i * LAY_OFS` of each layer and sets `plane_scaling[i]` from bit 31, `LXCR_C1R_SCA` ("SCaling Ability"). The same code is in `v6.18-stm32mp` and `v6.6-stm32mp` | read from source |
| A static per-variant or per-layer table | **None.** No table, property or comment names the layers that scale. The only comment naming layers is about YUV semi-planar/planar formats ("Soc MP25 doesn't support YUV semiplanar and planar pixel formats on layer 1"), not scaling | read from source |
| Which layers have the scaler | So **not stated by any source file**: it is a hardware register value. On the board: plane 42 (layer 2) scales, plane 49 (layer 3) does not; plane 34 (layer 1) not yet probed (session C pictures) | open in source; observed 2026-11-27 for 42/49 (earlier file) |
| MP251, MP253 | Same LTDC node (`stm32mp251.dtsi`, compatible `st,stm32mp251-ltdc`); `stm32mp253.dtsi` only includes `stm32mp251.dtsi` and adds a second CPU, CAN and Ethernet, nothing for the display. **No DSI, LVDS or GPU nodes** in these files. The scaler code path and registers are the same (`ltdc_readable_reg` allows the scaler registers for `st,stm32mp251-ltdc` and `st,stm32mp255-ltdc`) | read from source |
| MP255, MP257 | `stm32mp255.dtsi` overrides the LTDC node with `st,stm32mp255-ltdc` and four clocks (`lcd`, `bus`, `ref`, `lvds`), and adds `dsi`, `lvds` (both `disabled` by default) and `gpu`. `stm32mp257.dtsi` only adds the Ethernet switch. Same scaler code | read from source |
| `xc` and `xf` suffix files | Contain only the crypto nodes, nothing for the display | read from source |
| Any per-variant difference in scaler layers | None in the driver or device tree. All variants share the one LTDC IP node, the same `HWVER_4010x` case and the same register read, so the layers with `SCA` set would be the same if the IP is the same | inferred (identical software path is read from source; identical silicon is not stated by any file) |
| Public reference manual | **Not read**: no public MP25 reference manual or datasheet page was fetched in this session, so the layer-by-layer scaler capability from ST's documents remains unchecked | open |
| Scaling limits | Upscale only; no downscale; YUV never scaled; a layer without the scaler refuses any size mismatch ("Scaling is not supported on layer %d"); no numeric cap in the code | read from source (as in `stm32mp257-ltdc-source-full.md`) |
| Firewall or RIF: layer count | The LTDC has firewall children `l1l2` (RIFSC 119), `l3` (RIFSC 120), `rot` (RIFSC 121) and `cmn` (RIFSC 80). In `ltdc_get_caps()`, when `stm32_firewall_grant_access_by_id` is refused for `l3`, `nb_layers` is decremented (layer 3 is not exposed) and `caps.crc` is cleared. A refusal of `cmn`, `l1l2` or `rot` fails the probe | read from source |
| Firewall or RIF: scaler | **Does not change which layers have a scaler.** The scaler loop runs over `nb_layers`, so a secured layer 3 is simply not scanned and gets no plane; `plane_scaling[]` still comes from `LXCR_C1R_SCA` alone. Rotation: with no usable `rotation-memory`, `ltdc_load()` clears `crtc_rotation` | read from source |
| Firewall on this board | The three children carry RIFSC ids `0x77`, `0x78`, `0x79` (119, 120, 121) and the node `0x50` (80) as in the source. Three planes (34, 42, 49) exist, so the `l3` access was granted and `nb_layers` was not reduced | observed 2026-11-28 (properties), observed 2026-11-27 (three planes) |
| Other variants' firewall settings | The device-tree files contain the same `access-controllers` for every variant; whether a given boot-chain configuration denies `l3` is a runtime property of its RIF settings, not of the variant | inferred |

## Summary table

| Question | Answer | Tag |
|---|---|---|
| Q8 connectors and encoders | One connector `HDMI-A-1` (connected, 1920x1080), one encoder (a DSI host and ADV7535 bridge chain), one CRTC; nothing else | observed 2026-11-28 |
| Q8 `possible_crtcs` | One CRTC, so `0x1` on every plane and the encoder (encoder value from masks) | observed 2026-11-28, inferred (encoder) |
| Q8 device-tree display nodes | LTDC, DSI and ADV7535 `okay`; LVDS, `panel-lvds` and its backlight `disabled`; LTDC `port` has endpoints for DSI and LVDS | observed 2026-11-28 |
| Q8 display modules | `stm_drm`, `dw_mipi_dsi_stm`, `dw_mipi_dsi`, `adv7511`, `drm_dma_helper`, `galcore`; no LVDS or panel module | observed 2026-11-28 |
| Q8 DSI route | Exists and is live (to HDMI via ADV7535) | observed 2026-11-28 |
| Q8 LVDS route | In the device tree, **disabled**, not bound; not usable without a device-tree change | observed 2026-11-28 |
| Q4 which layers scale | Not given by any source file: read from `LXCR_C1R_SCA` of each layer at probe. No per-variant table or layer comment | read from source |
| Q4 MP251/MP253 vs MP255/MP257 | Same driver path and same LTDC IP node. MP251/MP253 have no DSI, LVDS or GPU nodes; MP255/MP257 add them. No scaler difference in source | read from source; inferred (same silicon) |
| Q4 firewall or RIF | Can remove layer 3 (and the CRC) when `l3` is denied; does not change which layers have the scaler | read from source |
| Q4 public reference manual | Not read, the answer from ST's documents is open | open |

Kept out on purpose: board addresses, host names, IPs, serial numbers and
credentials.
