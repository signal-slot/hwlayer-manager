# RK3588 VOP2 source rows against the NanoPC-T6's running kernel (RK Q2)

Written 2026-11-28 (host UTC) on the host. No board was logged in to. This
page answers `docs/rk3588-backend-design.md` §7 Q2.

Tags: **read from source** means a statement of a file at a named ref;
**inferred** means a comparison or a mapping drawn by this page;
**observed** is used only for a dated board fact copied from an existing doc.

## How the refs were read

`drivers/gpu/drm/rockchip/rockchip_drm_vop2.c`, `rockchip_vop2_reg.c` and
`rockchip_drm_vop2.h` were downloaded over HTTPS at tags `v6.18` and
`v6.18.55` (no clone, no other files; the project dependency cache) and
compared with `diff -r` on 2026-11-28. The diff has five hunks in `V2C`
and `REG` (rows 13 and 14) and none in `HDR`; everything outside them is
byte-identical. That makes every "same" verdict below read from source
(both refs named). The vendor `develop-6.1` @
`77168c8d5ab82399f65a80e9f807b50ba37cf483` was not re-fetched: the vendor
column repeats the cited docs, and the vendor-versus-mainline verdicts are
inferred from them. Line numbers are `v6.18`; in `v6.18.55` they shift by
+3 in `V2C` after line 339 and by +19 to +45 in `REG` after line 1371
(for example `rk3588_vop` is at `REG:2565` there, `rockchip_vop2_mod_supported`
at `V2C:386`).

## Which kernel is running

- Version string: `6.18.55-current-rockchip64 #1 SMP PREEMPT`, aarch64
  (observed 2026-11-28, `docs/observed/rk3588-topology.md`; also observed
  2026-11-27 in `docs/linux-board-scenarios.md`).
- The docs state it is an Armbian build (`docs/observed/rk3588-topology.md`,
  `docs/rk3588-vc4-profiles.md`). No doc states the tree it is built from.
- Mapping (inferred): `6.18.55` is a stable release number of mainline
  Linux 6.18; Armbian's `current` branch for `rockchip64` follows mainline
  stable and adds its own patch set. So the running tree is **mainline
  v6.18.55 plus Armbian patches**. It is not the Rockchip vendor 6.1 tree
  (`rockchip-linux/kernel` `develop-6.1`), whose version numbers are 6.1.x.
  Whether Armbian patches touch `drivers/gpu/drm/rockchip/` is unknown here.
- Refs named below:
  - **cited mainline ref**: tag `v6.18` of torvalds/linux (tag object
    `f7b88edb52c8dd01b7e576390d658ae6eef0e134`),
    https://github.com/torvalds/linux/tree/v6.18/drivers/gpu/drm/rockchip
  - **running ref**: tag `v6.18.55` of gregkh/linux (tag object
    `5c70e682251f50354711645fd494be38495f46ec`), the closest public
    stand-in for the Armbian tree,
    https://github.com/gregkh/linux/tree/v6.18.55/drivers/gpu/drm/rockchip
  - **cited vendor ref**: `rockchip-linux/kernel` `develop-6.1` at
    `77168c8d5ab82399f65a80e9f807b50ba37cf483`
    (`docs/rk3588-vendor-vop2-source.md`, `docs/observed/rk3588-source.md`).

Files: `V2C` = `rockchip_drm_vop2.c`, `REG` = `rockchip_vop2_reg.c`,
`HDR` = `rockchip_drm_vop2.h`, all under `drivers/gpu/drm/rockchip/`. Line
numbers are those of `v6.18`, as cited in `docs/rk3588-mainline-source.md`.

## Rows

"Cited" is the value at the cited mainline ref `v6.18`. "Running" is the
value at running ref `v6.18.55`. "Vendor 6.1" is the cited vendor value
(`develop-6.1` @ `77168c8d5ab82399f65a80e9f807b50ba37cf483`), shown because Q2
asks about that tree. The verdict is cited against running, then cited
against vendor. Cited-against-running verdicts are read from source at both refs (see above); cited-against-vendor verdicts are inferred from the cited vendor docs.
The question of Armbian patches applies to every row and is not repeated.

| # | Row | file:function | Cited (`v6.18`) | Running (`v6.18.55`) | Vendor 6.1 | Verdict |
|---|---|---|---|---|---|---|
| 1 | Window list: 8 windows, no `win1` | `REG` `rk3588_vop_win_data` 1121-1268; `V2C:2576` | 4 Cluster + 4 Esmart, one plane each | window tables identical at both refs | Cluster `win1` sub-windows with DT `support-multi-area`; 8 planes, 12 with multi-area (not re-verified there) | same (read from source); different from vendor (inferred) |
| 2 | Plane type per window | `V2C` `vop2_create_crtcs` 2332-2446 | first PRIMARY window per active port is Primary, rest Overlay (`V2C:2428`) | `V2C` hunks only at 339 and 352; function identical | Esmart windows named Primary candidates | same (read from source); different from vendor (inferred) |
| 3 | Window-to-port masks | `REG` window entries (`REG:1126` etc.) | `BIT(0..3)` on all 8 windows | `REG` hunks start at 1372; table unchanged | default masks, DT `plane-mask` overrides | same (read from source); different from vendor (inferred) |
| 4 | Format list, Cluster | `REG` `formats_cluster` 58-73 | 14 entries, no NV12 | arrays `REG:58-126` unchanged | AFBC plus linear | same (read from source); different from vendor (inferred) |
| 5 | Format list, Esmart | `REG` `formats_esmart` 106-126 | 21 entries, linear only | unchanged | linear only | same (read from source); same as vendor (inferred) |
| 6 | Modifier tables | `REG` `format_modifiers_afbc` 203-239, `format_modifiers` 198-201 | Cluster: nine AFBC 16x16, no LINEAR; Esmart: LINEAR | `REG` hunks start at 1372 | Cluster AFBC combinations plus linear | same (read from source); different from vendor (inferred) |
| 7 | Modifier acceptance | `V2C` `rockchip_vop2_mod_supported` 383-422 | 10-bit RGB32 needs AFBC on RK3588 | not in the `V2C` hunks | not read | same (read from source); unknown for vendor |
| 8 | zpos range | `V2C:2301-2302` | 0..7, initial `win_id`, same for every window | not in the `V2C` hunks | not read | same (read from source); unknown for vendor |
| 9 | Scale check | `V2C` `vop2_plane_atomic_check` 992-1061 (1004-1005) | 1/8..8 on every window; `max_upscale_factor`/`max_downscale_factor` (4 Cluster, 8 Esmart, `REG:1136` etc.) not used | unchanged | hardware 4x Cluster, 8x Esmart; check 1/8..8 | same (read from source): the check is 1/8..8 and the 4x/8x fields are not used (`grep` of both `V2C` copies finds none); vendor unknown |
| 10 | Max source size | `REG` `rk3588_vop` 2525; `V2C:1042-1051` | 4096x4320 | `rk3588_vop` (`REG:2520-2538`) unchanged | 4096x7680, Cluster sub 2048 | same (read from source); different from vendor (inferred) |
| 11 | Rotation per window | `REG:1131-1132`, `REG:1207` | Cluster 90/270 plus X/Y reflect; Esmart `REFLECT_Y`; no buffer check | table and update path identical | Cluster rotate only for AFBC buffers | same (read from source); different from vendor (inferred) |
| 12 | Per-CRTC layer budget | `V2C:2476-2484`, `V2C:1816-1835` | `nlayers = 8 / active ports`, `-EINVAL` above | not in the `V2C` hunks | `nr_layers = 8`, 7 mixers | same (read from source); different from vendor (inferred) |
| 13 | Layer-select and mixer order across ports | `REG` from line 1372, `vop2_find_start_mixer_id_for_vp` | `v6.18` code | **changed** in `v6.18.55`: new `vop2_find_win_by_phys_id`, start id read from `LAYER_SEL`, poll delay 0 to 10, the layer-config wait moves before the layer-select write and runs only when another port has a pending `CFG_DONE` (it was unconditional, after) | not read | different (stable backport). Read from source at both refs |
| 14 | Output-format helper | `V2C:339`, `V2C:352` | RK3576 not matched in rg-swap; two YUV10 bus formats absent | RK3576 matched; two more `MEDIA_BUS_FMT_*YUV*10_1X20` cases | not read | different (stable). No effect on RK3588 plane limits (inferred) |
| 15 | Flip and vblank handling | `V2C` `vop2_crtc_atomic_begin` (`V2C:1844`) and the flip path | not cited in detail by the RK docs; Q9 is a board question | not in the `V2C` hunks; the `CFG_DONE` wait of row 13 is on the same path | not read | unknown for flip semantics (not read in detail); `vop2_crtc_atomic_begin` itself is identical at both refs |

## Answer to Q2

- **Which kernel**: the board runs `6.18.55-current-rockchip64`, an Armbian
  kernel. Inferred: mainline v6.18.55 plus Armbian patches, not vendor 6.1.
- **Which source rows apply**: the mainline v6.18 rows of
  `docs/rk3588-mainline-source.md`, not the vendor 6.1 rows of
  `docs/rk3588-backend-design.md` §2/§4. Against the vendor tree the rows
  that differ are sub-windows (1), plane type (2), masks (3), Cluster
  formats and modifiers (4, 6), source size (10), rotation (11) and layer
  budget (12). Profile values must be tagged per kernel: mainline 6.18 for
  this board.
- **v6.18 against v6.18.55** (read from source): the plane-limit rows (1-12) are the same.
  Only row 13 (layer-select ordering across ports) and row 14 (output
  formats) changed. Row 13 and the unknown row 15 matter for multi-output
  ordering and flip timing, and should be kept in mind when Q9 is measured.
- **Still open**: whether the Armbian patch set changes
  `drivers/gpu/drm/rockchip/` (unknown), and the vendor rows marked unknown.
  The vendor column rests on the cited docs, not on a fresh read of
  `develop-6.1`.
