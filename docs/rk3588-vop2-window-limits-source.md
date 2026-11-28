# RK3588 VOP2: limits that could refuse many simultaneous windows (mainline v6.18)

Written 2026-11-29 (host UTC) for question Q4 of `docs/rk3588-backend-design.md`
§7 (can eight windows be shown at once). No board was touched. Nothing here is a
measurement.

## How this page was made (read this first)

The task was to re-read `drivers/gpu/drm/rockchip/rockchip_drm_vop2.c` and
`rockchip_vop2_reg.c` at tag `v6.18` (the column `docs/rk3588-kernel-compare.md`
uses). In this run every network fetch of the kernel files was refused for
approval, and the project dependency cache holds no copy of them. So the files
were **not re-read today**. This page instead collects the file and line
references that earlier merged docs recorded from the same tag:
`docs/rk3588-vop2-window-assign-source.md`, `docs/rk3588-vop2-limits-source.md`
and `docs/rk3588-kernel-compare.md` (all dated 2026-11-28). Where a row says
"read from source (cited)" the reading was done by that earlier doc and the
line numbers are copied unchanged and not re-verified here. The tag object and
commit pins are in those docs; `docs/rk3588-vop2-limits-source.md` itself notes
that its tag check was not completed. Treat line numbers as good to a few lines
until someone re-fetches the two files.

Tags: **read from source (cited)** = a statement of the named file at `v6.18`,
taken from the named earlier doc; **inferred** = drawn by this page.
`V2C` = `rockchip_drm_vop2.c`, `V2R` = `rockchip_vop2_reg.c`, both at `v6.18`.
The running board kernel is `6.18.55-current-rockchip64` (observed 2026-11-27,
`docs/linux-board-scenarios.md`); `docs/rk3588-kernel-compare.md` found the two
files differ from `v6.18` only in rows 13 and 14 (line shifts, a layer-config
wait change), not in the checks below.

## 1. Paths that can refuse many windows on one CRTC

| # | Path | What it does | Tag |
|---|---|---|---|
| L1 | Per-CRTC layer count, `vop2_crtc_atomic_check` | Counts the planes in the CRTC state; `nplanes > vp->nlayers` returns `-EINVAL`. The primary is counted. It is the only per-port layer cap in the driver | read from source (cited: V2C:1816-1834) |
| L2 | `nlayers` value | `vp->nlayers = win_size / nvps` for each used port. RK3588 has `win_size` 8, so 8 with one port, 4 with two, 2 with three, 2 with four (integer division). `nvps` counts ports that have a device-tree remote endpoint | read from source (cited: V2C:2483; `nvps` V2C:2368-2374; `.win_size` V2R:2524-2529) |
| L3 | Window-to-port reach | Every window row has `possible_vp_mask = BIT(0)..BIT(3)`; `possible_crtcs` of an overlay has one bit per registered CRTC. So the table does not limit which windows one CRTC may use | read from source (cited: V2R:1126-1252; V2C:2417-2445) |
| L4 | Primary per CRTC | Each registered CRTC gets the first unused window of type PRIMARY (a Cluster) at probe; a CRTC with none fails probe. Fewer active ports means extra Clusters become overlays | read from source (cited: V2C:2378-2414) |
| L5 | zpos range | zpos is 0..`registered_num_wins - 1` (0..7) on every plane, not per CRTC. The DRM core normalises zpos per CRTC; equal values are ordered by plane id, not rejected | read from source (cited: V2C:2301-2302; core `drm_atomic_helper.c:1115-1116`, `drm_blend.c:437-446`) |
| L6 | Layer mixer slot | Slot is `normalized_zpos + ofs`, `ofs` the sum of `nlayers` of lower-numbered ports; the overlay code is `rk3568_vop2_setup_overlay`. This is programming, not a check | read from source (cited: V2R:2161-2165, 2457-2461) |
| L7 | Port mux | Per-port layer counts are written to `PORT0..2_MUX` as running totals; port 3 is a fixed value 7 on RK3588. Programming, not a check | read from source (cited: V2R:2123-2152) |
| L8 | Window migration | `OVL_PORT_SEL` is written per window; a window moving between ports or a zpos change waits for the mux and layer config latch. Not a check | read from source (cited: V2R:2170-2260) |

## 2. Per-plane checks that apply to every window in the set

| # | Path | Refuses | Tag |
|---|---|---|---|
| P1 | Scale range, `vop2_plane_atomic_check` | `min_scale` 1/8, `max_scale` 8, same for every class; the table's per-class `max_upscale_factor`/`max_downscale_factor` (4 Cluster, 8 Esmart) are not read by `V2C` | read from source (cited: V2C:1000-1022; table V2R:1121-1275) |
| P2 | Source size | Source width or height above `max_input` (4096 x 4320 on RK3588) gives `-EINVAL`; no per-class value | read from source (cited: V2C:1042-1053; V2R:2525) |
| P3 | Minimum size | Source or destination edge under 4 px makes the plane invisible and returns 0, so it does not refuse | read from source (cited: V2C:1032-1041) |
| P4 | YUV origin | Odd source x with a YUV format gives `-EINVAL` | read from source (cited: V2C:1052-1059) |
| P5 | Format | Unsupported fourcc returns the negative result of `vop2_convert_format` | read from source (cited: V2C:1030-1032) |
| P6 | Modifier and format pair | Checked at framebuffer creation (`rockchip_vop2_mod_supported`), not in atomic check: `DRM_FORMAT_MOD_INVALID` refused; 10-bit RGB needs AFBC on RK3588; other non-linear modifiers must be in the plane's list and convert to an AFBC format | read from source (cited: V2C:380-422) |
| P7 | AFBC alignment | Width is aligned down to a multiple of 4 silently; no error | read from source (cited: V2C:1245-1249) |
| P8 | Rotation | Per-window `supported_rotations` (Cluster: 90, 270, reflect X, reflect Y; Esmart: reflect Y). Refused by the core's property range, not by this driver | read from source (cited: V2R:1131-1132, 1207); the core refusal is inferred |
| P9 | Gamma | `vop2_crtc_atomic_check_gamma` can refuse a CRTC; it concerns the gamma LUT, not windows | read from source (cited: V2C:1789, 1825) |

## 3. Limits looked for and not found

Read from source (cited: absence reported by `docs/rk3588-vop2-window-assign-source.md`
R11 and `docs/rk3588-vop2-limits-source.md` §1-2):

- No bandwidth or memory-bus check (a search for `bandwidth` found nothing).
- No line-buffer check in atomic check. The Cluster line-buffer mode
  (`vop2_get_cluster_lb_mode`, V2C:514-528) is chosen at update time (2 for
  rotate 90/270, else 0) and is hardware state, not a reject reason. No
  line-buffer mode is written for Esmart (V2R:396).
- No cap on how many Cluster, Esmart or AFBC windows are used.
- `max_output` is data only; no line in `V2C` reads it.

## 4. What this implies for a TEST_ONLY sweep (inferred)

- inferred: the first refusal as windows are added on one CRTC is L1. With one
  port exposed it should come after eight planes (so none); with two exposed
  CRTCs, `nlayers` is 4 and the fifth plane on one CRTC returns `-EINVAL`. The
  recorded board exposes two CRTCs (`docs/observed/rk3588-topology.md`), so
  four per CRTC is the expectation, if the running kernel matches `v6.18` here.
- inferred: whether an idle but exposed CRTC lowers `nlayers` depends on
  whether its port has a device-tree endpoint (L2). The sweep cannot tell this
  from the source alone.
- inferred: since no bandwidth or line-buffer check exists (section 3), a
  sweep that passes at any count says nothing about underflow at scan-out; only
  a real commit and a picture can.
- inferred: a failure below the layer cap would come from a per-plane rule
  (P1 to P6), for example a scaled plane or an AFBC buffer, not from the count.
- inferred: if the first refusal is below four per CRTC, or the board accepts
  more than `nlayers`, the running Armbian tree differs from `v6.18` in these
  functions (`docs/rk3588-armbian-vop2-patches.md` is the item reading the
  Armbian patch set).

## 5. Open

- The two files were not re-fetched in this run; every line number is cited from
  earlier docs (see the first section). Re-verify before quoting one.
- Whether the DRM core refuses a CRTC state with no primary plane before
  `vop2_crtc_atomic_check` runs is not read; the earlier doc tags it inferred.
- Whether a commit may migrate a window to another CRTC and use it in the same
  atomic commit is not decided by either file.
- How many ports the recorded board's device tree gives an endpoint (so `nvps`)
  is not read here; `docs/rk3588-dt-source.md` and the board sweep are the
  places that decide it.
- Anything about the running Armbian tree beyond the `v6.18` to `v6.18.55`
  comparison is not known from source.
