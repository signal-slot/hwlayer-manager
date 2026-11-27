# RK3588 VOP2 per-port plane reach, read from the mainline driver

Written 2026-11-28 for plan #459, item `rk3588-dt-source`. It explains which
DRM plane can reach which video port (CRTC) on the NanoPC-T6 and why. Scaling,
AFBC, rotation and format lists are out of scope here (`docs/rk3588-backend-design.md`
section 4 and the source-reading items cover them). Nothing was run on a board
for this note.

## Tags and what was actually read

Each statement is tagged **read from source** (file:symbol), observed
(with its date, 2026-11-27 or 2026-11-28) or **inferred**.

| Source | Status |
| --- | --- |
| `drivers/gpu/drm/rockchip/rockchip_drm_vop2.c` and `rockchip_vop2_reg.c` (mainline, post-RK3576, i.e. 6.17 or later) | Read from the copies another item (`rk-mainline-source`) had already staged in the shared dependency cache. The staged copy carries no git metadata: **the tag and commit are not recorded**, so they could not be pinned. The task asked for v6.18.y; that match is **unverified**. Line numbers below refer to those copies. |
| `arch/arm64/boot/dts/rockchip/rk3588-base.dtsi`, `rk3588-nanopc-t6*.dts(i)` | **Not read.** This worker could not reach git.kernel.org (clone, `curl` and the web-fetch helper were all refused), and the staged copy holds only the two driver files. Every DT statement below is therefore inferred from the driver plus observations, and marked so. |
| `rockchip_drm_fb.c` / `rockchip_drm_drv.c` (where `normalize_zpos` would be set) | **Not read.** |

Observed data: the plan #457 handoff (2026-11-27, kernel `6.18.55-current-rockchip64`)
and `docs/observed/rk3588-topology.md` (2026-11-28, same kernel).

## How possible_crtcs is built (read from source)

`vop2_create_crtcs()` in `rockchip_drm_vop2.c`:

1. Loop over `nr_vps` (4 on RK3588, `rk3588_vop` in `rockchip_vop2_reg.c`). A
   video port is used only if the VOP node has a remote endpoint on that port
   (`of_graph_get_remote_node(dev->of_node, i, -1)`); otherwise it is skipped
   (`No remote for vp%d`). `nvps` counts the used ports.
2. Primary loop: for each used VP, scan the windows in table order and take the
   first one that is not yet registered, whose `possible_vp_mask` contains the
   VP and whose table `type` is `DRM_PLANE_TYPE_PRIMARY`. It is registered with
   `possible_crtcs = BIT(nvp)`, where `nvp` is the **index among used ports**
   (not the hardware VP id), and becomes `vp->primary_plane`. A VP with no
   candidate fails the probe with `no primary plane for vp`.
3. Overlay loop: every window not registered as primary gets
   `win->type = OVERLAY` and `possible_crtcs` = the OR of `BIT(nvp)` over all
   used VPs whose id is in the window's `possible_vp_mask`.
4. `vp->nlayers = win_size / nvps` for every used VP.

In `rk3588_vop_win_data[]` (`rockchip_vop2_reg.c`) every window has
`possible_vp_mask = BIT(0)|BIT(1)|BIT(2)|BIT(3)`. The table `type` is PRIMARY
for Cluster0 to Cluster3 and OVERLAY for Esmart0 to Esmart3. So the only
per-port restriction in the driver is the primary-plane choice.

## Window to plane to port table

Plane ids and debugfs names are observed (2026-11-28,
`docs/observed/rk3588-topology.md`); the table order is also the driver's
window index (`win_id`, array order). Two used ports, `video_port0` = bit 0 and
`video_port1` = bit 1 (observed 2026-11-28: two CRTCs, ids 81 and 82).

| Window (`.name`) | Plane id | Table `type` | Registered as | possible_crtcs from source | Reason |
| --- | --- | --- | --- | --- | --- |
| Cluster0-win0 | 33 | PRIMARY | Primary of VP0 | `0b01` (VP0 only) | first PRIMARY window in the primary loop for the first used VP |
| Cluster1-win0 | 39 | PRIMARY | Primary of VP1 | `0b10` (VP1 only) | next PRIMARY window, taken by the second used VP |
| Cluster2-win0 | 45 | PRIMARY | Overlay | `0b11` | not picked as primary; overlay loop ORs all used VPs |
| Cluster3-win0 | 51 | PRIMARY | Overlay | `0b11` | same |
| Esmart0-win0 | 57 | OVERLAY | Overlay | `0b11` | same |
| Esmart1-win0 | 63 | OVERLAY | Overlay | `0b11` | same |
| Esmart2-win0 | 69 | OVERLAY | Overlay | `0b11` | same |
| Esmart3-win0 | 75 | OVERLAY | Overlay | `0b11` | same |

All rows: read from source (`vop2_create_crtcs`, `rk3588_vop_win_data[]`); the
plane ids are observed 2026-11-28; the assumption that two ports are used is
observed 2026-11-28 (two CRTCs) and the reason they are used is inferred (both
HDMI outputs have a VOP endpoint in the DT; the DT was not read).

Why a Cluster window is a Primary: the driver, not the hardware, marks the four
Clusters as PRIMARY candidates. Only as many are registered as primary as there
are used ports; the rest are downgraded to Overlay in the overlay loop (read from
source, `win->type = DRM_PLANE_TYPE_OVERLAY`).

## Zpos rule (read from source unless marked)

- Property: `drm_plane_create_zpos_property(&win->base, win->win_id, 0, registered_num_wins - 1)`
  in `vop2_plane_init()`. `registered_num_wins = win_size = 8`, so every plane
  has a mutable zpos range 0..=7, whichever CRTC it sits on, and its initial
  value is its own table index (Cluster0 = 0 ... Esmart3 = 7). It is not
  per-CRTC and not immutable.
- Use: the layer mixer code reads `plane->state->normalized_zpos` (not the raw
  property) in `rk3568_vop2_setup_layer_mixer()` and the alpha setup in
  `rockchip_vop2_reg.c`. The layer slot for a window is
  `normalized_zpos + ofs`, where `ofs` is the sum of `nlayers` of the lower-id
  VPs. So after normalisation the planes on one CRTC form a dense 0..n-1
  sequence, and a VP's slots start after the previous VP's.
- Normalisation itself is done by the DRM atomic core when the driver sets
  `normalize_zpos`; that flag lives in a file that was not read, so this step is
  **inferred**. Consequence (inferred): userspace may use any distinct values in
  0..=7 on a CRTC; only the order counts, and equal values are rejected by the
  core.
- Per-CRTC layer budget: `vop2_crtc_atomic_check()` returns `-EINVAL` when the
  number of planes on the CRTC exceeds `vp->nlayers = win_size / nvps`. With two
  used ports that is 8 / 2 = **4 planes per CRTC**, with one used port 8
  (read from source; the "two ports" input is observed 2026-11-28). This bounds
  the "up to 8 active windows" TEST_ONLY sweep: expect rejection above 4
  planes on one CRTC (inferred, to be checked by the sweep items).

## Match against the observations

| Observation | Source says | Match |
| --- | --- | --- |
| 8 planes (observed 2026-11-27 and 2026-11-28) | `win_size = ARRAY_SIZE(rk3588_vop_win_data)` = 8; no sub-windows in the mainline table | yes |
| zpos 0..7 on every plane (observed 2026-11-27) | range `0..registered_num_wins - 1` on every plane | yes |
| Reset zpos is the plane index (observed 2026-11-28, `rk3588-topology.md`) | initial value `win->win_id` | yes |
| Planes 33 and 39 Primary, 45..75 Overlay (observed 2026-11-28) | Cluster0, Cluster1 registered as primary; Cluster2/3 demoted; Esmart overlay | yes |
| Plane 39 cannot reach video_port0 (observed 2026-11-27) | Cluster1 is the primary of the second used port: `possible_crtcs = BIT(1)` | **yes, follows from source** |
| Plane 69 cannot reach video_port0 (stated in the plan #457 handoff) | Esmart2 is an overlay with `0b11` | **no, does not follow from source** |
| Lease holds primary 33 plus overlays 45/51/57/63/69/75 (observed 2026-11-27) | planes 45..75 are all `0b11` | yes (and the lease holds 69, so 69 reaches the port) |
| Probe mask `0b0` only for plane 39 (observed 2026-11-28, `rk3588-topology.md`) | only Cluster1 lacks bit 0 | yes |

Planes 39 and 69: the unreachability of 39 follows from the source. The
unreachability of 69 does not, and two observations disagree with it: the
2026-11-28 probe lists 69 with `outputs 0b1`, and the kernel lease for the
output includes 69. The most likely explanation is an error in the handoff
sentence (inferred); a board that reports 69 with mask `0b0` would need a kernel
that differs from the staged driver (for example a vendor patch), which is not
observed. The planner should trust the probe masks, and the profile fixture
should follow the recorded 2026-11-28 masks (only 39 unreachable).

## Which ports exist on the NanoPC-T6 (inferred)

Not read from the DT. Observed 2026-11-28: two CRTCs registered (`video_port0`,
`video_port1`), connectors `HDMI-A-1` connected and `HDMI-A-2` disconnected,
and no connector bound to `video_port1`. Inferred: the board enables both HDMI
transmitters, each with an endpoint on a VOP port, so `nvps = 2`; a port needs
only the endpoint, not a connected sink, to count (the driver checks the remote
node, not the connector state). The two used ports are the hardware ports 0 and
1 (inferred from the CRTC names; `vp->id` is used for the name,
`crtc init ... "video_port%d"`).

## Consequences for two-output use (design section 5, Q7)

All inferred from the source above unless tagged.

- Plane 39 (Cluster1) is the only plane that can be on `video_port1` as primary
  and the only one that cannot be on `video_port0`. A planner that gives
  `video_port0` a lease needs no Cluster1.
- `video_port1`'s own primary (39) is reachable only by it; the six overlays can
  go to either port. So a per-VP assignment has fixed anchors (33 -> VP0, 39 ->
  VP1) and six movable planes (45, 51, 57, 63, 69, 75).
- The budget is per CRTC: 4 planes each with two used ports, so at most 3
  overlays beside the primary on each output, and the two outputs together
  cannot exceed 8. The six movable overlays cannot all sit on one output when
  both ports are used.
- A single-lease setup (kernel lease 33 plus six overlays, observed 2026-11-27)
  hands the planner 7 planes for one CRTC, which exceeds the 4-plane CRTC check
  above; a commit with more than 4 of them enabled is expected to fail
  (inferred). The planner must either cap at `nlayers` or learn it from a
  TEST_ONLY probe (Q4).
- Moving a window from one VP to the other re-programs the shared layer and
  port select registers (`old_layer_sel`, `old_port_sel`), and the code comment
  warns that the change takes effect with one VP's vsync. A second lease
  committing independently on the other port can therefore interact
  (read from source, `rk3568_vop2_setup_layer_mixer`; the interaction itself is
  inferred). Q7 stays open until a two-lease board run (not part of this item).

## Open after this note

- Pin the staged driver to a tag and commit, and read the DT files and the
  `normalize_zpos` setting, from a machine that can reach git.kernel.org.
- Confirm the 4-plane per-CRTC limit and the 69 question on the board
  (TEST_ONLY sweep; probe masks).
