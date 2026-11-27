# RK3588 VOP2 in mainline Linux 6.18, read from source

Read 2026-11-28. No board was used and no code was run.

Tags follow `docs/platform-capabilities.md`:

- **read from source**: a quotation or paraphrase of the file and line cited,
  at the pinned ref below.
- **inferred**: every comparison with another document or kernel, and every
  conclusion drawn from more than one row.
- **observed**: not produced here. A row says so only when it repeats a
  dated board fact from `docs/rk3588-backend-design.md` §2, and then it is
  also inferred.

## Pinned refs

- Mainline: tag `v6.18` of torvalds/linux, tag object
  `f7b88edb52c8dd01b7e576390d658ae6eef0e134`:
  https://github.com/torvalds/linux/tree/v6.18/drivers/gpu/drm/rockchip
- Stable: tag `v6.18.55` exists in gregkh/linux (tag object
  `5c70e682251f50354711645fd494be38495f46ec`):
  https://github.com/gregkh/linux/tree/v6.18.55/drivers/gpu/drm/rockchip
- Files (line numbers below are of the `v6.18` copy):
  - `V2C` = `rockchip_drm_vop2.c` (2793 lines)
  - `REG` = `rockchip_vop2_reg.c` (2577 lines)
  - `HDR` = `rockchip_drm_vop2.h` (866 lines)
  - All three under `drivers/gpu/drm/rockchip/`.
- Fetched with a plain HTTPS download of each file at the tag into the
  project dependency cache, outside the repository. The two trees were
  compared with `diff`.

## 1. Did stable v6.18.55 change these files?

Read from source (diff `v6.18` against `v6.18.55`, 2026-11-28):

| File | Result | Tag |
|---|---|---|
| `HDR` | identical | read from source |
| `V2C` | two hunks, `V2C:339` (`vop2_output_rg_swap` also matches `VOP_VERSION_RK3576`) and `V2C:352` (two more `MEDIA_BUS_FMT_*YUV*10_1X20` cases in an output-format helper). Plane check, format, modifier, rotation, zpos and plane-init code unchanged | read from source |
| `REG` | hunks from `REG:1372` on only: a new `vop2_find_win_by_phys_id`, a rewritten `vop2_find_start_mixer_id_for_vp` (mixer start id now found from the old `LAYER_SEL` register instead of counting windows of lower ports), poll delay `0` to `10` at two `readx_poll_timeout` calls, and an extra wait for other ports' `CFG_DONE` before layer-select writes. The window tables (`REG:1121-1268`), format arrays (`REG:58-126`, `REG:198-239`) and `rk3588_vop` (`REG:2520-2538`) are unchanged | read from source |

Inferred: nothing in this document's tables changes between `v6.18` and
`v6.18.55`. The `REG` change touches how windows are stacked across ports
after a layer-select change, which can matter for multi-output ordering but
not for any per-plane limit below.

## 2. Window list and plane types

All rows: read from source, `REG` unless stated.

| Window | `base` | `type` | `possible_vp_mask` | `layer_sel_id` | AXI bus | Lines |
|---|---|---|---|---|---|---|
| Cluster0-win0 | 0x1000 | `PRIMARY` | VP0-3 | 0 | 0 | `REG:1123-1140` |
| Cluster1-win0 | 0x1200 | `PRIMARY` | VP0-3 | 1 | 0 | `REG:1142-1159` |
| Cluster2-win0 | 0x1400 | `PRIMARY` | VP0-3 | 4 | 1 | `REG:1161-1178` |
| Cluster3-win0 | 0x1600 | `PRIMARY` | VP0-3 | 5 | 1 | `REG:1180-1197` |
| Esmart0-win0 | 0x1800 | `OVERLAY` | VP0-3 | 2 | 0 | `REG:1199-1215` |
| Esmart1-win0 | 0x1a00 | `OVERLAY` | VP0-3 | 3 | 0 | `REG:1216-1232` |
| Esmart2-win0 | 0x1c00 | `OVERLAY` | VP0-3 | 6 | 1 | `REG:1233-1249` |
| Esmart3-win0 | 0x1e00 | `OVERLAY` | VP0-3 | 7 | 1 | `REG:1250-1266` |

Further rows, read from source:

- The header comment says the VOP has 4 Cluster and 4 Esmart windows, that
  every Cluster can work as one 4K window or split into two, and that all
  Cluster windows support AFBC decoding (`REG:1092-1096`).
- The table has exactly 8 entries; there is no `win1` sub-window entry. The
  driver registers one plane per entry: `vop2->registered_num_wins =
  vop2_data->win_size` (`V2C:2576`), `win_size = ARRAY_SIZE(rk3588_vop_win_data)`
  (`REG:2529`).
- Plane type is not taken from the table as-is. `vop2_create_crtcs`
  (`V2C:2332`) registers, for each video port that has a DT remote
  endpoint (`V2C:2374`), the first window whose table `type` is `PRIMARY`
  and which can reach that port as that port's primary plane, with
  `possible_crtcs = BIT(nvp)` (`V2C:2377-2415`, `V2C:2400`). Every
  window that is not a primary is then forced to `DRM_PLANE_TYPE_OVERLAY`
  (`V2C:2428`) and gets `possible_crtcs` set for every active port its
  `possible_vp_mask` allows (`V2C:2418-2446`, `V2C:2439`).
- Window-to-port masks are `BIT(0)|BIT(1)|BIT(2)|BIT(3)` for all 8 windows
  (`REG:1126`, `1145`, `1164`, `1183`, `1201`, `1218`, `1236`, `1252`). The
  table has no per-port narrowing and no `plane-mask` code in the files read
  (`grep` of `plane-mask` finds nothing in `V2C`/`REG`/`HDR`).
- Video ports (`REG:1057-1090`): VP0 offset `0xc00`, VP1 `0xd00`, VP2
  `0xe00`, each `max_output` 4096x2304 and 10-bit output; VP3 offset `0xf00`,
  `max_output` 2048x1536, no 10-bit flag. `nr_vps = 4` (`REG:2524`).

Inferred:

- With *n* ports having endpoints, Cluster0 .. Cluster(*n*-1) become the
  Primary planes of ports in order, each reachable from its own CRTC only;
  the remaining Cluster windows and all Esmart windows are Overlay and reach
  every active CRTC. With two active ports this gives "Cluster0 and Cluster1
  Primary, six Overlay", as `docs/rk3588-backend-design.md` §2 records
  (observed 2026-11-28 on a NanoPC-T6, kernel 6.18.55). With three active
  ports Cluster0, 1 and 2 are Primary and bound to one CRTC each. This is a
  plausible explanation of the NanoPC-T6 handoff fact that planes 39 and 69
  cannot reach `video_port0` (observed 2026-11-27, kernel
  6.18.55-current-rockchip64, per the plan handoff): they would be the
  Cluster1 and Cluster2 primaries. That identification is not established.
- Versus the vendor 6.1 recording in `docs/rk3588-backend-design.md` §2:
  there Esmart windows are named Primary candidates and Cluster sub-windows
  can add planes with DT `support-multi-area`; mainline 6.18 has neither.
  The mainline plane count is fixed at 8.

## 3. Formats and modifiers per window class

Read from source:

| Class | `.formats` | `.format_modifiers` | Lines |
|---|---|---|---|
| Cluster (all 4) | `formats_cluster`, 14 entries | `format_modifiers_afbc` | `REG:1127-1129` (and equivalents per Cluster) |
| Esmart (all 4) | `formats_esmart`, 21 entries | `format_modifiers` | `REG:1202-1204` (and equivalents per Esmart) |

- `formats_cluster` (`REG:58-73`): `XRGB2101010`, `XBGR2101010`, `XRGB8888`,
  `ARGB8888`, `XBGR8888`, `ABGR8888`, `RGB888`, `BGR888`, `RGB565`,
  `BGR565`, then four entries commented "non-Linear mode only":
  `YUV420_8BIT`, `YUV420_10BIT`, `YUYV`, `Y210`. There is no `NV12`, no
  `ARGB2101010` and no `ABGR2101010` in the RK3588 Cluster list (the RK3576
  list, `REG:82-104`, has both).
- `formats_esmart` (`REG:106-126`): `XRGB8888`, `ARGB8888`, `XBGR8888`,
  `ABGR8888`, `RGB888`, `BGR888`, `RGB565`, `BGR565`, then linear YUV:
  `NV12`, `NV21`, `NV16`, `NV61`, `NV20`, `NV24`, `NV42`, `NV30`, `NV15`,
  `YVYU`, `VYUY`, `YUYV`, `UYVY`. No 10-bit RGB.
- `format_modifiers` (`REG:198-201`): `DRM_FORMAT_MOD_LINEAR`, then
  `DRM_FORMAT_MOD_INVALID`.
- `format_modifiers_afbc` (`REG:203-239`): nine Arm AFBC modifiers, all
  `BLOCK_SIZE_16x16`: plain; `SPARSE`; `YTR`; `CBR`; `YTR|SPARSE`;
  `CBR|SPARSE`; `YTR|CBR`; `YTR|CBR|SPARSE`; `YTR|SPARSE|SPLIT` (comment:
  "SPLIT mandates SPARSE, RGB modes mandates YTR"). It contains **no**
  `DRM_FORMAT_MOD_LINEAR`. The 32x8 variants are in
  `format_modifiers_rk3576_afbc` (`REG:241`), used by RK3576 windows
  (`REG:861`, `REG:878`) and not by RK3588.
- Per-(format, modifier) acceptance is `rockchip_vop2_mod_supported`
  (`V2C:383-422`):
  - `DRM_FORMAT_MOD_INVALID` is refused (`V2C:389`).
  - The "Cluster window only supports format with afbc" refusal
    (`V2C:393-398`) applies only when `version == VOP_VERSION_RK3568`.
  - `XRGB2101010` and `XBGR2101010` on `VOP_VERSION_RK3588` need an AFBC
    modifier; linear is refused (`V2C:402-408`, message "Only support 32 bpp
    format with afbc").
  - Linear otherwise passes (`V2C:411-412`); any other modifier must be in
    the plane's list (`V2C:414-419`) and the format must have an AFBC
    format code (`V2C:421`, `vop2_convert_afbc_format`, `V2C:220-254`).
  - AFBC format codes exist for RGB 8/10-bit, `RGB888`, `RGB565`,
    `YUV420_8BIT`, `YUV420_10BIT`, the four packed 4:2:2 formats and `Y210`
    (`V2C:220-254`). So `NV12`-type formats have no AFBC code.
- A format in `formats_cluster` marked "non-Linear mode only" is not
  excluded from linear by code in `rockchip_vop2_mod_supported`. What keeps
  linear off the Cluster list is that `format_modifiers_afbc` has no
  `LINEAR` entry (`REG:203-239`).

Inferred:

- The Cluster `IN_FORMATS` blob therefore lists AFBC pairs only (the 10
  RGB8/565/888 formats and the YUV variants with the nine modifiers, minus
  pairs `mod_supported` refuses); Esmart lists linear pairs only. This
  matches the observed shape in `docs/rk3588-backend-design.md` §2 ("Cluster
  planes list AFBC modifiers plus YUV formats; Esmart planes list linear
  only (21 formats)", observed 2026-11-28). The 21 Esmart count equals
  `formats_esmart` here. A full per-plane list is Q1 of the design note and
  needs the board, not source.
- Vendor 6.1, per `docs/rk3588-backend-design.md` §2/§4, lists linear for
  Cluster plus AFBC ("AFBC combinations + linear (no linear YUV, no linear
  10-bit RGB)"). The mainline 6.18 Cluster list has no linear at all. A
  profile written from the vendor figures would over-promise linear on
  Cluster.

## 4. Scaling

Read from source:

- `vop2_plane_atomic_check` (`V2C:992`) sets
  `min_scale = FRAC_16_16(1, 8)` and `max_scale = FRAC_16_16(8, 1)`
  (`V2C:1004-1005`) and calls `drm_atomic_helper_check_plane_state(pstate,
  cstate, min_scale, max_scale, true, true)` (`V2C:1020-1022`). The same
  bounds apply to every window; there is no per-window branch.
- The per-window table fields `max_upscale_factor` and `max_downscale_factor`
  exist (`HDR:200-201`) and are set to 4 on all four Cluster windows
  (`REG:1136-1137`, `1156-1157`, `1175-1176`, `1194-1195`) and 8 on all four
  Esmart windows (`REG:1212-1213`, `1229-1230`, `1246-1247`, `1263-1264`).
  `grep` finds **no use** of either field in `V2C`; they are never
  compared with the requested scale in this driver.
- Other refusals or clamps in the plane check and update:
  - source or destination smaller than 4x4: error message and the plane is
    made invisible, return 0 (`V2C:1033-1040`);
  - source wider or higher than `max_input` 4096x4320 (`REG:2525`):
    `-EINVAL` (`V2C:1042-1051`);
  - YUV format with odd source x: `-EINVAL` (`V2C:1054-1059`);
  - `format` with no hardware code: error (`V2C:1029-1031`).
- Update-time quirks that change the programmed rectangle but do not refuse
  it: Esmart (`!WIN_FEATURE_AFBDC`) with source width `% 16 == 1` and scaling
  down gets 1 pixel cropped (`V2C:1234-1243`); Esmart with an odd destination
  width in down-scale gets `dst_w++` (`V2C:598-606`); an AFBC source width
  not a multiple of 4 is rounded down (`V2C:1245-1249`); a destination that
  runs past the mode is clipped (`V2C:1211-1233`).
- Filtering: for Cluster windows `vop2_setup_scale` returns before writing any
  filter-mode register (`V2C:619-620`); the header comment says Cluster uses
  bicubic for horizontal scale-up and bilinear otherwise (`REG:1101`);
  Esmart additionally writes filter-mode registers (`V2C:621-654`).
  Vertical down-scale of 2x or 4x or more is done with the `GT2`/`GT4` bits
  (`V2C:575-582`, `V2C:614-615`).

Why scaling might be refused on 6.18 (inferred, from the rows above):

- The kernel itself refuses a plane only outside 1/8..8, below 4x4, above
  4096x4320, or with odd-x YUV. A Cluster window scaled 6x passes
  `drm_atomic_helper_check_plane_state`; the hardware figure of 4x is not
  enforced here, so whether a 5x..8x Cluster scale works is a hardware
  question that only `TEST_ONLY` plus a picture can answer, and `TEST_ONLY`
  alone would not refuse it.
- Source gives no reason for a `TEST_ONLY` scaling refusal inside 1/8..8 on
  any window. A "no plane scales" observation on the NanoPC-T6 (plan #457
  handoff, 2026-11-27, kernel 6.18.55) is therefore not explained by this
  code. The more likely reasons are that it was a property-visibility check
  (DRM has no scale property; scaling is `SRC_*` versus `CRTC_*`), the
  4-plane per-CRTC limit in section 8, or the plane/CRTC reach in section 2.
  Not settled.
- Versus `docs/rk3588-backend-design.md` §2/§4 (vendor 6.1): the vendor
  figures "Cluster 4x, Esmart 8x; the atomic check allows 1/8..8" are the
  same two numbers. In mainline the 4x/8x are table fields that nothing
  reads, and 1/8..8 is the only enforced check (inferred; the vendor
  statement itself is the design note's, not re-read here).

## 5. Rotation and reflection

Read from source:

- Property creation: if the window has `supported_rotations`, create the
  property with `ROTATE_0` plus those bits (`V2C:2295-2298`). A window with
  no bits gets no rotation property.
- Cluster windows: `ROTATE_90 | ROTATE_270 | REFLECT_X | REFLECT_Y`
  (`REG:1131-1132`, `1150-1151`, `1169-1170`, `1188-1189`). `ROTATE_180`
  is `ROTATE_0` plus both reflect bits and so is available.
- Esmart windows: `REFLECT_Y` only (`REG:1207`, `1224`, `1241`, `1258`). No
  `REFLECT_X`, no 90/270, so no 180.
- Rotation is applied in the update path: Y mirror moves the start line for
  linear buffers (`V2C:1185`, `V2C:1200`); the AFBC rotate and
  mirror registers are written only when AFBC is on (`V2C:1350-1351`, inside
  the `if (afbc_en)` block that starts at `V2C:1275`); AFBC rotation uses a
  transform offset (`V2C:436-503`); 90/270 sets the Cluster line-buffer mode
  to 2 (`V2C:516-527`) and turns off the AFBC half-block mode
  (`V2C:424-431`).
- No check in `vop2_plane_atomic_check` (`V2C:992-1061`) or in
  `rockchip_vop2_mod_supported` ties rotation to the buffer modifier.

Inferred:

- The vendor 6.1 note in `docs/rk3588-backend-design.md` §2/§4, "non-AFBC
  Cluster rejects REFLECT_X and ROTATE_90/270", has no counterpart in this
  mainline code. On 6.18 a linear buffer cannot reach a Cluster plane on
  RK3588 anyway (section 3: no `LINEAR` in the list), so the case cannot
  arise through a modifier-aware client. Where AFBC rotation on Cluster is
  accepted by `TEST_ONLY`, the source shows the AFBC registers being set;
  correctness of the picture is not shown by source.
- The profile can use `transforms` = rotate and reflect on Cluster and
  `REFLECT_Y` on Esmart with no per-modifier split, because the only
  modifiers Cluster accepts are AFBC.

## 6. Input and output sizes

Read from source:

- `rk3588_vop`: `.max_input = { 4096, 4320 }`, `.max_output = { 4096, 4320 }`
  (`REG:2525-2526`). `max_input` is the only size compared with the source
  rectangle (`V2C:1042-1051`). `max_output` of the SoC data is not used in
  `V2C` plane code (`grep`: only the `max_input` use above; per-port
  `max_output` is in `REG:1057-1090` and is not compared in the plane check).
- No separate Cluster or Esmart size limit, and no code for a sub-window
  limit, in this driver.
- Minimum size: 4x4 for source and destination (`V2C:1033-1039`).
- A Cluster window line buffer is 2048x16 in full mode or 4096x8 in half mode
  (comment at `V2C:512-519`); `vop2_get_cluster_lb_mode` returns mode 2 for
  90/270 and 0 otherwise (`V2C:516-527`). Source above 2048 wide on a
  Cluster window uses the 4096x8 mode per that comment; nothing refuses it.

Inferred:

- Vendor 6.1 per `docs/rk3588-backend-design.md` §2/§4: "4096x7680 (Cluster
  sub: 2048x7680), width doubles in splice mode". Mainline 6.18 enforces
  4096x4320 for every window and has no splice or multi-area code in the
  three files. A `max_source` of 4096x4320 is the value the 6.18 source
  supports; 4096x7680 would be wrong for it.
- Port limits differ from the design note's table too (VP0 7680x4320 there;
  here VP0 4096x2304, VP3 2048x1536, `REG:1057-1090`). These are per-port
  output limits, not plane limits.

## 7. zpos

Read from source:

- `drm_plane_create_zpos_property(&win->base, win->win_id, 0,
  vop2->registered_num_wins - 1)` (`V2C:2301-2302`): a mutable property with
  initial value `win_id` and range 0..7 for RK3588.
- `win_id` is the table index (`V2C:2564`), so the initial zpos order is
  Cluster0-3, then Esmart0-3.
- The range is the same for every window, whatever CRTC it ends up on. The
  hardware layer order is rebuilt from the normalised zpos in the
  overlay setup (`ops->setup_overlay`, called from `vop2_crtc_atomic_begin`,
  `V2C:1844`).

Inferred: this matches the "zpos 0..7 on every plane" observation
(NanoPC-T6, 2026-11-27, kernel 6.18.55). Open question Q6 of the design note
(zpos competition per CRTC) is not answered by source.

## 8. What limits the number of active windows

Read from source:

- `vp->nlayers = vop2_data->win_size / nvps` for each port with an
  endpoint (`V2C:2476-2484`, expression at `V2C:2483`), with a comment that
  layers are distributed equally because changing the layer count of a port
  at run time is very hard (`V2C:2476-2478`). `nvps` is the number of ports
  with a DT endpoint (`V2C:2342`, `V2C:2374`). For `win_size = 8` that is 8
  for one active port, 4 for two, 2 for three (integer division), 2 for
  four.
- `vop2_crtc_atomic_check` counts the planes attached to the CRTC state and
  returns `-EINVAL` when `nplanes > vp->nlayers` (`V2C:1816-1835`, test at
  `V2C:1832`). This is in `atomic_check`, so it applies to `TEST_ONLY`.
- This count includes the Primary plane.
- No other cap on simultaneously active windows is in the three files read.
  The "8 layers and 7 mixers" and AXI-bus remarks in the design note are
  hardware facts of the vendor tree; this driver has `axi_bus_id` and read-id
  fields in the table (`REG:1133-1135` etc.) but no check against them.

Inferred:

- With two active ports (one connector on the NanoPC-T6 session records two
  CRTCs registered), each CRTC accepts at most 4 planes including its
  Primary. The planner's 8-window budget in `docs/rk3588-backend-design.md`
  §4 ("at most 8 simultaneously active windows") is only reachable with a
  single active port; with two or more it is 4 or fewer per output. This is
  the quantity the TEST_ONLY sweep of design note Q4 should measure, and
  the kernel lease fact (Primary 33 plus overlays 45, 51, 57, 63, 69, 75;
  plan handoff, 2026-11-27) is consistent with at most a 7-overlay set being
  usable by one output only if that output is the sole active port.
  The sweep decides, source does not.
- The count is per CRTC, so a plane used on another CRTC does not reduce
  this count, but the planner cannot see that other CRTC (design note §5).

## 9. Port and plane-mask rules

Read from source:

- `possible_crtcs` is a bitmask over *active* ports in order (`nvp`
  increments only for ports with an endpoint, `V2C:2377-2415`,
  `V2C:2431-2441`), not over hardware port numbers. A Primary has exactly
  one bit (`V2C:2400`); an Overlay has one bit per active port its table mask
  allows (`V2C:2439`). Since all masks are 0xf, every Overlay reaches all
  active CRTCs.
- A port with no DT endpoint gets no CRTC and does not count in `nvp`
  (`V2C:2364-2373`, `V2C:2382`). If a port has an endpoint but no
  Primary window can be bound, probe fails with `-ENOENT` (`V2C:2413-2414`).
- No `rockchip,plane-mask` or `disable-win-move` property is parsed in the
  three files (`grep` finds neither).

Inferred:

- The vendor DT plane-mask restriction in `docs/rk3588-backend-design.md`
  §2 does not exist in mainline 6.18; reach is decided only by the
  Primary-per-port rule above and by which ports have endpoints in the board
  DT.
- `PlaneCaps.outputs` for the generic backend already comes from
  `possible_crtcs`, so a profile needs no extra reach table.

## 10. Summary against the design note's §4 rows

All inferred (comparison of this document with
`docs/rk3588-backend-design.md` §4, vendor 6.1):

| §4 row | Vendor 6.1 figure (design note) | Mainline 6.18 source (this note) |
|---|---|---|
| scaling | Cluster 4x, Esmart 8x; check 1/8..8 | Check 1/8..8 on all windows (`V2C:1004-1005`); the 4x/8x fields exist (`REG:1136-1137`, `REG:1212-1213`) but are unused |
| `max_source` | 4096x7680 | 4096x4320 (`REG:2525`) |
| transforms | Cluster: only AFBC buffers; Esmart: `REFLECT_Y` | Cluster: rotate and reflect with no buffer check in code (`REG:1131-1132`); Esmart: `REFLECT_Y` (`REG:1207`) |
| format set | Cluster: AFBC + linear (no linear YUV, no linear 10-bit RGB) | Cluster: AFBC only (`REG:203-239`, no `LINEAR`); Esmart: linear only, 21 formats (`REG:106-126`) |
| sub-window pairing | follows main, 2048 wide | none: 8 windows, no `win1` (`REG:1121-1268`) |
| layer budget | at most 8 | at most `8 / active ports` per CRTC (`V2C:2483`, `V2C:1832`) |
| window-to-port | default per-port masks, DT overrides | all windows reach all ports (`REG:1126` etc.); Primary per port is Cluster *n* |

## What this note does not cover

- Whether the kernel on the board (`6.18.55-current-rockchip64`, an Armbian
  build) carries downstream patches to these files. Only the mainline tags
  were read, and `v6.18.55` is a stable tag, not the Armbian tree.
- Hardware behaviour past what the source checks. Nothing here says a
  5x Cluster scale, a given AFBC modifier or a given plane count works on
  the board.
- Any fixture or code change.
