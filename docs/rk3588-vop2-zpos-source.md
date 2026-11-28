# RK3588 VOP2 zpos: per-CRTC or global, and equal values (source read)

Read 2026-11-28. No board was used and no code was run. This answers the
source half of Q6 in `docs/rk3588-backend-design.md` §7 ("is zpos global or
per CRTC; what happens on equal zpos"), which `docs/rk3588-mainline-source.md`
§7 left open.

Tags: **read from source** (a quotation or close paraphrase of the cited
line), **inferred** (reasoning from more than one row), **observed** (a dated
board fact; none is produced here).

## Pinned refs and an honest limit

- Mainline `v6.18`, commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449`:
  https://github.com/torvalds/linux/tree/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/gpu/drm
- Stable `v6.18.55` (tag object `5c70e682251f50354711645fd494be38495f46ec`):
  https://github.com/gregkh/linux/tree/v6.18.55/drivers/gpu/drm/rockchip
- Files: `BLEND` = `drivers/gpu/drm/drm_blend.c`, `HLP` =
  `drivers/gpu/drm/drm_atomic_helper.c`, `FB` =
  `drivers/gpu/drm/rockchip/rockchip_drm_fb.c`, `V2C` =
  `rockchip/rockchip_drm_vop2.c`, `REG` = `rockchip/rockchip_vop2_reg.c`.
- How read: `git clone` and shell downloads were not available on this node, so
  each file was fetched as raw text at the pinned commit by a page-reading
  tool that returns text without line numbers. **Line numbers are therefore
  hand-counted** from the fetched text. `BLEND` and `FB` numbers are exact to
  the counter's care; `REG` numbers are approximate (about two lines,
  counted back from the end of the file) and are marked `~`. Where I have no
  trustworthy number I give the function name. `V2C` numbers are copied from
  `docs/rk3588-mainline-source.md` (its `V2C` numbering), not recounted.
- `v6.18.55` was **not** re-read file by file here. For it I rely on the diff
  recorded in `docs/rk3588-mainline-source.md` §1 (read from source,
  2026-11-28): `BLEND`, `HLP` and `FB` are outside that diff; `V2C` changed
  only at two output-format hunks; `REG` changed from `REG:1372` on (below).

## 1. Is normalisation per CRTC? Yes

All rows read from source at `v6.18` unless stated.

| Fact | Where |
|---|---|
| The core calls `drm_atomic_normalize_zpos()` from `drm_atomic_helper_check()` only when `dev->mode_config.normalize_zpos` is true, after `drm_atomic_helper_check_modeset()` and before `drm_atomic_helper_check_planes()` | `HLP`, function `drm_atomic_helper_check` (no line number) |
| Rockchip sets that flag: `dev->mode_config.normalize_zpos = true;` last statement of `rockchip_drm_mode_config_init` | `FB:84` (hand-counted) |
| Rockchip's `atomic_check` is the plain `drm_atomic_helper_check` | `FB:62-66` |
| `rockchip_drm_vop2.c` does not call `drm_atomic_normalize_zpos` and does not sort by zpos; it only creates the property and prints `normalized_zpos` in a debugfs dump | `V2C:2301-2302` (create), function `vop2_plane_state_dump` |
| The static worker `drm_atomic_helper_crtc_normalize_zpos(crtc, crtc_state)` collects only planes in `crtc_state->plane_mask` (`drm_for_each_plane_mask(plane, dev, crtc_state->plane_mask)`), sorts them, and writes `normalized_zpos = i` for `i = 0..n-1` | `BLEND:470`, `BLEND:482-490` |
| The exported `drm_atomic_normalize_zpos` runs that worker for each CRTC in the state whose `plane_mask` changed or whose `zpos_changed` is set; a plane changing zpos sets `zpos_changed` on its own CRTC only | `BLEND:525-543` |
| Kerneldoc: "For every CRTC this function checks new states of all planes assigned to it ... `normalized_zpos` is then filled with unique values from 0 to number of active planes in crtc minus one" | `BLEND:506-511` |

Conclusion, inferred from the rows: the **user-visible `zpos` value is a
global-looking property** (one range 0..7 on every plane, `V2C:2301-2302`),
but **the kernel's ordering is per CRTC**: a plane's `normalized_zpos` is its
rank among the planes of its own CRTC only. A plane with `zpos` 7 on CRTC A
does not compete with planes on CRTC B.

## 2. Tie-break on equal zpos: plane object id, lowest at the bottom

Read from source:

- Comparator `drm_atomic_state_zpos_cmp`: if `sa->zpos != sb->zpos` return
  `sa->zpos - sb->zpos`, else return `sa->plane->base.id - sb->plane->base.id`
  (`BLEND:437-446`).
- Kerneldoc: "Planes are compared first by their zpos values, then by plane id
  (if zpos is equal). The plane with lowest zpos value is at the bottom"
  (`BLEND:507-509`).
- The DRM overview says that without normalisation "two planes with the same Z
  position value have undefined ordering" (`BLEND:141-142`); the helper is what
  makes it defined.

Inferred:

- With `normalize_zpos` true (it is, on Rockchip), equal zpos is **resolved
  deterministically** before the driver sees it: the plane with the lower DRM
  object id sits lower. No error is raised for equal values, so a `TEST_ONLY`
  commit with equal zpos on one CRTC is not rejected by the zpos code.
- The object id order is the id order of the plane objects, which follows
  plane registration order. Per `V2C` the planes are registered in window-table
  order (Cluster0-3, Esmart0-3, with the per-CRTC primaries registered
  first), so the tie-break is not necessarily the same as the initial zpos
  (`win_id`) order. The board session reads the ids.

## 3. Layer-select registers are shared between video ports

Read from source, `REG` (`~` = approximate), function
`rk3568_vop2_setup_layer_mixer`, used for RK3588 through
`rk3588_vop_ops.setup_overlay = rk3568_vop2_setup_overlay` (`REG:~2459-2463`):

- A comment in the function says `RK3568_OVL_LAYER_SEL` and
  `RK3568_OVL_PORT_SEL` "are shared by all Video Ports, and the configuration
  take effect by one Video Port's vsync" (`REG:~2244-2257`). There is one
  `RK3568_OVL_CTRL`, one `RK3568_OVL_PORT_SEL`, one `RK3568_OVL_LAYER_SEL`
  for the whole VOP (`REG:~2121`, `~2156`, `~2263-2271`); only a
  `YUV_MODE(vp->id)` bit in `OVL_CTRL` is per port (`REG:~2125-2128`).
- Each window's `layer_sel_id` is the same value in all four VP slots on RK3588
  (`REG:1123-1266`, e.g. Cluster0 `{0,0,0,0}`, Esmart3 `{7,7,7,7}`), unlike
  RK3568 (`{3,3,3,0xf}`) and RK3576 (per VP, with its own per-VP
  `RK3576_OVL_LAYER_SEL(vp->id)`, `REG:~2345-2371`).
- Which port a window feeds is a field in the shared `OVL_PORT_SEL`, written
  with `vp->id` per window in a `switch (win->data->phys_id)`
  (`REG:~2189-2228`).

## 4. How the per-VP layer count and `port_mux` are derived

Read from source:

- `vp->nlayers = vop2_data->win_size / nvps` for each port with an endpoint,
  at the end of `vop2_create_crtcs` (`V2C:2476-2484`, expression `V2C:2483`);
  comment: layers are distributed equally because changing the count at run
  time is very hard. `nvps` counts ports with a DT endpoint. `win_size` is
  `ARRAY_SIZE(rk3588_vop_win_data)` = 8 (`REG:~2531`).
- `vop2_crtc_atomic_check` returns `-EINVAL` when the number of planes in
  the CRTC state exceeds `vp->nlayers`; it counts planes with
  `drm_atomic_crtc_state_for_each_plane` and never reads zpos
  (`V2C:1816-1835`, test `V2C:1832`).
- Port mux, `rk3568_vop2_setup_layer_mixer`: `PORT0_MUX = vp0->nlayers - 1`,
  `PORT1_MUX = vp0->nlayers + vp1->nlayers - 1`,
  `PORT2_MUX = vp0 + vp1 + vp2 nlayers - 1`; a port with `nlayers == 0` gets
  the value 8; `PORT3_MUX` is the fixed value 7 on RK3588 with the comment
  "Fixed value for rk3588" (`REG:~2134-2154`).
- Layer slot of a plane: `ofs = sum of nlayers of the lower-numbered ports`
  (`REG:~2160-2162`); `layer_id = (u8)(plane->state->normalized_zpos + ofs)`
  (`REG:~2168`). The slot is then written into `OVL_LAYER_SEL` together with
  the window's `layer_sel_id[vp->id]` (`REG:~2231-2239`); the previous window
  of the slot is cleared the same way.
- Write order: `OVL_CTRL`, `OVL_PORT_SEL`, `vop2_cfg_done(vp)`, a wait until
  the port-mux register reads back the written value, then `OVL_LAYER_SEL`
  (`REG:~2258-2271`; wait helpers `REG:~2060-2090`).

Stable `v6.18.55` difference (read from source in
`docs/rk3588-mainline-source.md` §1, not re-read here): the mixer start id used
for alpha is now derived from the old `LAYER_SEL` register instead of counting
windows of lower ports, and an extra wait for the other ports' `CFG_DONE`
precedes layer-select writes (`REG:1372` onward).

Inferred:

- Because the slot is `normalized_zpos + ofs` and `normalized_zpos <
  nplanes <= nlayers`, a CRTC cannot spill into another CRTC's slot range. The
  split is **static**: the second CRTC starts at slot `vp0->nlayers`, whether
  or not CRTC 0 uses its slots. With two active ports each CRTC gets 4 slots
  (8 / 2); with one, 8; with three, 2 each (integer division), so the
  planner's 8-window figure is reachable on one CRTC only.
- Which ports count in `ofs` is "lower-numbered ports" by id, and `nlayers` is 0
  for a port with no endpoint; an inactive port in between therefore
  contributes nothing. The two outputs' ids on the T6 decide the offsets.
- Layer registers being shared means a commit on one CRTC that changes the
  slot map can be applied at another port's vsync, and can delay or be delayed
  by it (the `v6.18.55` change adds a wait for it). This is a latency or
  stall risk, not a correctness limit on zpos.

## 5. Is a `TEST_ONLY` with equal zpos accepted? Inferred: yes

Inferred from §1, §2 and §4:

1. `drm_atomic_helper_check` runs normalisation before `check_planes`, and
   `TEST_ONLY` runs the same `atomic_check` (drivers are not skipped).
2. Normalisation makes ranks unique, so the VOP2 code never sees a tie.
3. `vop2_crtc_atomic_check` looks only at the plane count, not at zpos.

So the equal value is not a reason to fail. The only zpos-related failure is
count based (`nplanes > nlayers`), and zpos values larger than the range
(above `registered_num_wins - 1` = 7) fail in the property setter, not in
`atomic_check`.

## 6. Predictions the board session can confirm or refute

Each is inferred; "R" means it would refute the source reading.

1. Set equal `zpos` on two planes of one CRTC and run `TEST_ONLY`: accepted
   (refuted if `-EINVAL`/`-ERANGE`).
2. In a real or debugfs-visible state, the plane with the lower object id has
   the lower `normalized-zpos` when zpos is equal (debugfs prints
   `normalized-zpos=` per plane, `drm_atomic_plane_print_state` in
   `drm_atomic.c`). Swapping which plane has the lower id is not possible, so
   check both pairs.
3. The `normalized-zpos` values of planes on a CRTC are `0..n-1` with no gap,
   whatever zpos values userspace wrote (for example zpos 7 and 7 give 0 and 1;
   zpos 3 and 6 give 0 and 1).
4. Two CRTCs may both hold a plane with the same `zpos`, and both
   `normalized-zpos` can be 0 at once; a plane with zpos 7 on CRTC B has
   `normalized-zpos` 0 if it is alone.
5. A CRTC with more planes than `nlayers` fails `TEST_ONLY` regardless of
   zpos: with two active CRTCs the fifth plane on one CRTC fails, with one
   active CRTC the ninth does not exist. (This is the same check as Q4.)
6. The `zpos` of a window reads `win_id`-ordered by default (0..7) on every
   plane, the same on both CRTCs, supporting §1's "global-looking property".
7. Raising a plane to zpos 7 on CRTC A does not change the `normalized-zpos`
   of any plane on CRTC B, and does not set `zpos_changed` there (visible as no
   plane of B pulled into the atomic state in debug logs, if enabled).
8. A refutation of 1 or 2 would mean the board's kernel is not `v6.18`-like
   in `drm_blend.c`; the board kernel is `6.18.55-current-rockchip64`
   (observed 2026-11-27 in `docs/rk3588-backend-design.md` §2), an Armbian
   tree, which `docs/rk3588-mainline-source.md` marks as not read.

## 7. What remains open after this read

- Real hardware order of overlapping equal-zpos planes (what is visible), as
  opposed to the normalised rank: needs a KVM picture, Q9's session.
- `v6.18.55` line numbers and the Armbian tree: not read here.
- Behaviour when two CRTCs commit the shared layer registers at the same time
  (a stall or a wrong stack): not decidable from source.
