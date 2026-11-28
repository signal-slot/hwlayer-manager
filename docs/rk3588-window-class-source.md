# RK3588 VOP2: Cluster vs Esmart through KMS, and win1 / multi-area (Q3, Q8)

Written 2026-11-28 for plan #467, item `rk-window-class-source-b`. It answers
Q3 and Q8 of `docs/rk3588-backend-design.md` section 7. No board was used and
no code was run.

## Source and how it was read

- Mainline Linux `v6.18`, shallow clone, checked-out commit
  `7d0a66e4bb9081d75c82ec4957c50034cb0ea449`:
  https://github.com/torvalds/linux/tree/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/gpu/drm/rockchip
  (the tag object is `f7b88edb52c8dd01b7e576390d658ae6eef0e134`).
- Files: `rockchip_drm_vop2.c`, `rockchip_vop2_reg.c`, `rockchip_drm_vop2.h`.
  Line numbers below refer to that commit. Every source claim is tagged
  *read from source (v6.18)*.
- The vendor (Rockchip BSP) tree was not read.
- Board data comes from `docs/observed/rk3588-props.md` and
  `docs/observed/rk3588-topology.md` (both observed 2026-11-28). No
  `docs/rk3588-vop2-session-*.md` exists yet on this tree.

## Q3: how does KMS tell a Cluster window from an Esmart window?

**Answered.** A program can tell them apart from the KMS properties alone. The
most reliable signal is the modifier list in `IN_FORMATS`. The plane name is
not visible in KMS.

| Signal | Cluster (Cluster0..3) | Esmart (Esmart0..3) | Source (read from source, v6.18) |
|---|---|---|---|
| `IN_FORMATS` modifiers | 9 Arm AFBC modifiers (16x16 block, with sparse/YTR/CBR/split combinations), **no `LINEAR`** | `LINEAR` only | `format_modifiers_afbc` `rockchip_vop2_reg.c:203-234`; `format_modifiers` `:198-201`; assigned at `:1128` and `:1204` etc. |
| `IN_FORMATS` formats | 14 formats incl. `XRGB2101010`/`XBGR2101010`, `YUV420_8BIT`, `YUV420_10BIT`, `YUYV`, `Y210` | 21 formats: RGB plus NV12/NV21/NV16/NV61/NV20/NV24/NV42/NV30/NV15 and packed YUYV/YVYU/UYVY/VYUY; no 10-bit RGB | `formats_cluster` `:58-73`; `formats_esmart` `:106-127` |
| `rotation` property | `ROTATE_0/90/270` and `REFLECT_X/Y` | `ROTATE_0` and `REFLECT_Y` only | `.supported_rotations` in `rk3588_vop_win_data` `:1131-1132` (Cluster) and `:1207` (Esmart); property created in `vop2_plane_init` `rockchip_drm_vop2.c:2295-2298` |
| `zpos` | range `0..registered_num_wins-1` (0..7), default = window index | same range | `rockchip_drm_vop2.c:2301-2302`, `registered_num_wins` set at `:2576` |
| `possible_crtcs` | all connected video ports (see below) | all connected video ports | `.possible_vp_mask = BIT(0..3)` for all eight windows, `rockchip_vop2_reg.c:1126-1259`; computed at `rockchip_drm_vop2.c:2428-2441` |
| Plane type | `PRIMARY` in the table for all four, **but** the type is assigned per connected CRTC, so it does not identify the class | `OVERLAY` | table `.type` fields; assignment loops at `rockchip_drm_vop2.c:2378-2415` and `:2417-2445` |
| Plane name | internal only: `Cluster0-win0` etc. passed as the `name` argument of `drm_universal_plane_init` (`:2283-2287`). It is visible in debugfs; a program using only the KMS ioctls cannot rely on it | same | `:2287`, table `.name` fields |
| Scaling properties | none: VOP2 creates no scaling property; the limits (`max_upscale_factor`/`max_downscale_factor` = 4 for Cluster, 8 for Esmart) live only in the driver | | `:1136-1137`, `:1209-1210`; `vop2_plane_init` creates only rotation, alpha, blend mode and zpos (`:2295-2302`) |

Why the plane type does not identify the class: in `vop2_create_crtcs` the
driver gives each video port that has a connected remote (`vp->crtc.port`) the
first window whose table type is `PRIMARY` and whose `possible_vp_mask` matches
(`rockchip_drm_vop2.c:2378-2415`). All four Cluster windows are `PRIMARY` in the
table, but only as many become `PRIMARY` as there are connected video ports.
Every window not chosen is forced to `OVERLAY` (`:2428`). `win->type` is
initialised from the table at `:2562`. Plane type therefore says "primary for
CRTC n", not "Cluster".

`possible_crtcs` is also not a class signal. Primary planes get exactly one bit
(`BIT(nvp)`, `:2400`). Overlay planes get one bit per connected video port
(`:2430-2441`, `nvp` counts only ports with a remote). The bit numbers are
CRTC indexes in registration order, not VOP2 video-port ids.

**Rule for a profile (inferred from the table above):** a plane is a Cluster
window if its `IN_FORMATS` has any AFBC modifier (vendor byte `0x08`) and no
`LINEAR`, or if its `rotation` property offers `ROTATE_90`/`ROTATE_270`. It is
an Esmart window if `IN_FORMATS` lists only `LINEAR` and `rotation` offers at
most `REFLECT_Y`. Inferred: the two tests agree on every window in the
`rk3588_vop_win_data` table.

**Board check (observed 2026-11-28):** `docs/observed/rk3588-props.md` records
Cluster0..3 with 14 formats under 9 AFBC modifiers and no `LINEAR`, and
Esmart0..3 with 21 formats and `LINEAR` only. This matches the source. The
board recorded the types Cluster0 Primary (CRTC 0 only), Cluster1 Primary
(CRTC 1 only), Cluster2/3 Overlay, Esmart Overlay (both CRTCs). That matches the
assignment loops above for two connected video ports.

Not settled: the raw `rotation` bitmask on the board was recorded only as the
current value (`1`), not the property's supported set
(`docs/observed/rk3588-props.md`). See "Still open".

## Q8: do Cluster win1 and the Esmart multi-area sub-windows become KMS planes?

**Answered: no, in mainline v6.18 each Cluster and each Esmart is one KMS
plane.**

- The window table has exactly eight entries for RK3588, named
  `Cluster0-win0` .. `Cluster3-win0` and `Esmart0-win0` .. `Esmart3-win0`
  (`rockchip_vop2_reg.c:1121-1269`). There is no `win1` entry and no
  multi-area entry. *read from source (v6.18).*
- `vop2_win_init` creates one `struct vop2_win` per table entry and sets
  `registered_num_wins = win_size` (`rockchip_drm_vop2.c:2551-2578`). The plane
  loops iterate `registered_num_wins` (`:2380`, `:2418`), so no extra plane can
  appear. *read from source (v6.18).*
- The header only mentions `win1` as a register name (`WIN1_EMPTY_INTR`,
  `rockchip_drm_vop2.h:358`). The driver comment at `rockchip_vop2_reg.c:834-836`
  lists "Cluster0 win1" and "Cluster1 win1" AXI ids, but that is the RK3568
  hardware description; no RK3588 table entry uses them. The only other
  `win1` hit is a comment at `rockchip_drm_vop2.c:1871`, which is unrelated to
  planes. *read from source (v6.18).*
- Result: the RK3588 exposes 8 planes (4 Cluster + 4 Esmart), and
  `zpos` runs 0..7, because `drm_plane_create_zpos_property(..., 0,
  registered_num_wins - 1)` (`rockchip_drm_vop2.c:2301`).

**Board check (observed 2026-11-28):** the board shows 8 planes, zpos range
0..=7, names `Cluster0-win0`..`Esmart3-win0` in debugfs
(`docs/observed/rk3588-topology.md`, `docs/observed/rk3588-props.md`). This
matches the source: no win1 or multi-area planes exist.

## Still open

- **Rotation set on the board.** The source says Cluster offers
  `ROTATE_90/270` and `REFLECT_X/Y`, and Esmart offers `REFLECT_Y` only. The
  board recording has only the current rotation value. To settle: dump the
  `rotation` property's enum list per plane with the `props` example.
  Owned by `t6-topology-session`.
- **Whether the window table is the same on the board's kernel.** This doc
  reads v6.18; the board's running kernel is the subject of `rk-kernel-compare`
  (Q2). If the board runs a different tree, the table could differ.
- **Esmart default blend mode.** The board shows `none` as default blend on
  Esmart0 and Esmart1 and `premultiplied` elsewhere. This doc did not find the
  source of the difference; it was not needed for Q3 or Q8.
- **Effective scale limits.** The 4x / 8x table values are read, but whether
  `ATOMIC_TEST_ONLY` enforces exactly those ratios was not traced here. That
  belongs to `rk-scale-transform-source`.
