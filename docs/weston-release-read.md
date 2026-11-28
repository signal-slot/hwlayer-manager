# Released Weston: DRM backend plane assignment, read from source (WP5-02, W5-01, W5-03)

Desk read for `docs/wp5-gap-analysis.md` §2, §3 and §6 (W5-01, W5-03).
Written 2026-11-28. Nothing here was run, built or measured, and no board was
touched. This file adds to the gap analysis, which read a shallow `main` at
`c8d00e2` in part; it does not edit it.

## What was read

Both trees were fetched shallow (`--depth 1`) from
`https://gitlab.freedesktop.org/wayland/weston.git` on 2026-11-28 and read in a
checkout of each tag.

| Label | Tag | Commit (tag target) | Commit date | Why this tag |
| --- | --- | --- | --- | --- |
| **16.0.2** | `16.0.2` | `530ae6c769d2fd55ea5157542654810be81a3193` | 2026-10-26 | Highest release tag at `git ls-remote --tags` on 2026-11-28. The `x.0.9x` tags (`15.0.91` to `15.0.93` and so on) are alphas, betas and release candidates, not releases |
| **12.0.5** | `12.0.5` | `3d2cd9978c68c6338e902e9369266dbe8c4766c3` | 2025-04-29 | The upstream base of NXP `weston-imx-12.0.5`. That branch's `meson.build` says `version: '12.0.5'` (`docs/weston-imx-lease.md` row S3), and upstream has a `12.0.5` tag, so no nearer 12.0.x is needed |

What NXP changed on top of upstream 12.0.5 was **not read**. Every 12.0.5 row
below describes upstream 12.0.5, not `weston-imx-12.0.5`.

Files read in both trees:

- In full: `libweston/backend-drm/state-propose.c` and `include/libweston/backend-drm.h`.
- The parts named in the rows below:
  - `libweston/backend-drm/drm.c`: plane and CRTC setup, output enable, launcher and session, backend creation and the API table.
  - `libweston/backend-drm/drm-internal.h`: the mode enum, `drm_plane`, `drm_plane_handle`, `drm_output` and the transform helper.
  - `libweston/backend-drm/state-helpers.c`: plane coordinates.
  - `libweston/backend-drm/fb.c`: `drm_fb_get_from_paint_node` and `drm_fb_compatible_with_plane`.
  - `libweston/compositor.c`: paint-node creation and update, the z-order list and the repaint path.
  - `libweston/launcher-util.c` and `launcher-libseat.c`.
  - `libweston/linux-dmabuf.c`: tranches and per-surface feedback.
  - `include/libweston/libweston.h`: `struct weston_output`, `struct weston_plane`, and a search for plane and plugin symbols.
- A whole-tree word search for lease terms.

Not read: `kms.c` beyond the gap analysis' earlier read, `modes.c`, the
renderers, the shells and the frontend (`frontend/`, `compositor/`).

Source links, pinned to the commit:

| File | 16.0.2 | 12.0.5 |
| --- | --- | --- |
| `state-propose.c` | <https://gitlab.freedesktop.org/wayland/weston/-/blob/530ae6c769d2fd55ea5157542654810be81a3193/libweston/backend-drm/state-propose.c> | <https://gitlab.freedesktop.org/wayland/weston/-/blob/3d2cd9978c68c6338e902e9369266dbe8c4766c3/libweston/backend-drm/state-propose.c> |
| `drm.c` | <https://gitlab.freedesktop.org/wayland/weston/-/blob/530ae6c769d2fd55ea5157542654810be81a3193/libweston/backend-drm/drm.c> | <https://gitlab.freedesktop.org/wayland/weston/-/blob/3d2cd9978c68c6338e902e9369266dbe8c4766c3/libweston/backend-drm/drm.c> |
| `backend-drm.h` | <https://gitlab.freedesktop.org/wayland/weston/-/blob/530ae6c769d2fd55ea5157542654810be81a3193/include/libweston/backend-drm.h> | <https://gitlab.freedesktop.org/wayland/weston/-/blob/3d2cd9978c68c6338e902e9369266dbe8c4766c3/include/libweston/backend-drm.h> |
| `libweston.h` | <https://gitlab.freedesktop.org/wayland/weston/-/blob/530ae6c769d2fd55ea5157542654810be81a3193/include/libweston/libweston.h> | <https://gitlab.freedesktop.org/wayland/weston/-/blob/3d2cd9978c68c6338e902e9369266dbe8c4766c3/include/libweston/libweston.h> |
| `compositor.c` | <https://gitlab.freedesktop.org/wayland/weston/-/blob/530ae6c769d2fd55ea5157542654810be81a3193/libweston/compositor.c> | <https://gitlab.freedesktop.org/wayland/weston/-/blob/3d2cd9978c68c6338e902e9369266dbe8c4766c3/libweston/compositor.c> |
| `linux-dmabuf.c` | <https://gitlab.freedesktop.org/wayland/weston/-/blob/530ae6c769d2fd55ea5157542654810be81a3193/libweston/linux-dmabuf.c> | <https://gitlab.freedesktop.org/wayland/weston/-/blob/3d2cd9978c68c6338e902e9369266dbe8c4766c3/libweston/linux-dmabuf.c> |

Tags used below:

- **read from source (16.0.2)** / **read from source (12.0.5)**: read on
  2026-11-28 at the file:line given, in the tree with that label. Paths are
  relative to the tree root; `backend-drm/` stands for `libweston/backend-drm/`.
- **read from source (this repo)**: read in this repository at
  `3fc4ee65cc34873512867b6a45696789d11514d4` on 2026-11-28.
- **inferred**: a conclusion from the rows it names, not read anywhere.

## 1. Answers

- **Routes of `docs/wp5-gap-analysis.md` §3.** Neither release supports any of
  the three routes without a change to Weston (inferred, from sections 2 to 6):
  1. *Weston as lease source:* no. Neither tree has any lease code (row R1).
  2. *Plane-placement hint into Weston's planner:* no. No protocol, plugin API
     or callback lets a client or module ask for a plane or read the
     assignment back (rows A1 to A5). linux-dmabuf feedback flows the other
     way: Weston tells a client which formats might scan out (rows D1 to D4).
  3. *Weston calls this engine:* no. Plane assignment is a function the
     DRM backend installs in `weston_output::assign_planes`, built on private
     backend state (rows A3, A4).
- **The one route that needs no Weston change** is the standalone leased
  output of SoW §4.2 mode 1. The lease comes from `drm-lease-manager`, not from
  Weston, and Weston is simply not the owner of the leased objects. This read
  changes nothing there (inferred, rows R1 and P10).
- **A hook for `composition_core::advise`:** there is none in either release
  (rows A1 to A5). If it were consulted, it would have to be inside the DRM
  backend. The likely place is `drm_assign_planes` or `drm_output_propose_state`
  in `state-propose.c`, so it is a patch to Weston, not a plugin (inferred;
  section 7).
- **Can Weston refuse a frame:** no, in either release. If the plane states
  fail, it falls back to the renderer and then asserts that a state exists
  (rows P1, P2). This confirms the gap analysis §2 row "cannot refuse" for both
  releases.

## 2. Lease support

| # | Finding | Tag |
| --- | --- | --- |
| R1 | A case-insensitive whole-word search for `lease`, `leases`, `leasing`, `leased`, `drm-lease` and `drm_lease` over both trees (`*.po` excluded) returns **no hits**. Neither release has lease code, a lease option, or the `drm-lease-v1` protocol | read from source (16.0.2), read from source (12.0.5): whole-tree search |

## 3. Plane assignment: modes and fallback

| # | Finding | Tag |
| --- | --- | --- |
| P1 | **16.0.2 modes.** `enum drm_output_propose_state_mode` has five modes:<br>- `INVALID`<br>- `MIXED` ("mix renderer & planes")<br>- `RENDERER_AND_CURSOR` (renderer plus the cursor plane)<br>- `RENDERER_ONLY`<br>- `PLANES_ONLY` ("no renderer use, only planes")<br>It also has a `REUSE` bit (128) that reuses the prior state with new buffers (`backend-drm/drm-internal.h:376-382`). `drm_assign_planes` (`state-propose.c:1646`) tries, in order:<br>1. reuse of the prior state, unless `disable_drm_state_reuse` is set (`:1664-1672`);<br>2. planes-only, then mixed (`:1674-1686`). This step is skipped if `disable_client_buffer_scanout`, `disable_planes`, a virtual output or no GBM;<br>3. renderer-only if `disable_planes`, else renderer-and-cursor (`:1694-1706`);<br>4. the same again without a writeback screenshot (`:1710-1718`).<br>Then `assert(state)` (`:1721`) | read from source (16.0.2) |
| P2 | **12.0.5 modes.** The enum is local to `state-propose.c:46-50` and has three modes: `MIXED`, `RENDERER_ONLY` ("only assign to renderer & cursor") and `PLANES_ONLY`. There is no reuse bit and no separate renderer-and-cursor mode. `drm_assign_planes` (`:963`) tries planes-only, then mixed (`:981-991`), then renderer-only (`:999-1004`), then renderer-only without writeback (`:1008-1014`). Then `assert(state)` (`:1017`) | read from source (12.0.5) |
| P3 | **Order of the walk.** The walk goes over `weston_output::paint_node_z_order_list` top to bottom. Each node gets a zpos below the last one placed: `MIN(current_lowest_zpos - 1, plane->zpos_max)`. A plane whose `zpos_min` is at or above the current lowest zpos is skipped (16.0.2 `state-propose.c:921-943`; 12.0.5 `:595-629`). The first plane that takes the node wins (`break`). There is no backtracking over earlier nodes and no cost ranking | read from source (16.0.2), read from source (12.0.5) |
| P4 | **When the kernel is asked.** In mixed mode every provisional placement is checked with an atomic `TEST_ONLY` (`drm_pending_state_test`), with the last renderer FB on the primary plane as a stand-in. In planes-only mode the per-node test is skipped ("we just hope it'll work"). In both modes the whole state is tested once at the end (16.0.2 `:290-296`, `:1617-1621`; 12.0.5 `:128-134`, `:936-941`). Mixed mode needs a previous renderer FB of the current mode size, else that mode is not proposed (16.0.2 `:1349-1369`; 12.0.5 `:706-731`) | read from source (16.0.2), read from source (12.0.5) |
| P5 | **Renderer region.** A node that intersects the region already given to the renderer cannot go on an overlay (12.0.5 `:846-855`). In 16.0.2 it may still go on an *underlay* if the output has one and the node is fully opaque; otherwise it is marked `OCCLUDED_BY_RENDERER` (`:1496-1522`). In planes-only mode, a node that would need the renderer fails the whole mode, and the walk falls through to the next mode (16.0.2 `:1576-1580`; 12.0.5 `:891-896`) | read from source (16.0.2), read from source (12.0.5) |
| P6 | **Underlays (16.0.2 only).** At output enable, an overlay plane whose `zpos_min` is below the primary's `zpos_min` is tagged `UNDERLAY_ONLY` or `BOTH`, and the output gets `has_underlay` (`backend-drm/drm.c:2689-2705`). It is turned off again if the primary needs straight alpha and the renderer cannot produce it (`:2713-2722`). A node placed on an underlay sets `need_hole`, and the renderer punches a transparent hole for it (`state-propose.c:956-961`; `compositor.c:494-501`). 12.0.5 refuses any plane whose `zpos_max` is at or below the primary's zpos in mixed mode (`state-propose.c:613-624`) | read from source (16.0.2), read from source (12.0.5) |
| P7 | **Solid colour (16.0.2 only).** In planes-only mode, opaque solid-colour nodes are lowered into a background region. If the CRTC has `BACKGROUND_COLOR`, it is programmed from them; otherwise this works only for black (`state-propose.c:994-1110`, `:1455-1460`). 12.0.5 sends every solid buffer to the renderer (`:832-837`) | read from source (16.0.2), read from source (12.0.5) |
| P8 | **Reuse (16.0.2 only).** The prior state is reused when only buffers changed (`paint_node_changes & ~BUFFER_DIRTY` is empty), the prior mode was neither renderer-only nor renderer-and-cursor, and `planes_enabled` did not change. Reuse skips the test commit until reuse has failed more than `DRM_MAX_REUSE_FAILURES` (10) times (`state-propose.c:1129-1284`; `drm-internal.h:87`) | read from source (16.0.2) |
| P9 | **Result.** Nodes with a plane state are moved to that plane. All others go to the output's primary (renderer) plane. Nodes on a non-cursor plane get `ZERO_COPY` presentation feedback (16.0.2 `:1729-1792`; 12.0.5 `:1021-1087`) | read from source (16.0.2), read from source (12.0.5) |
| P10 | **Plane availability.** A plane is available to an output if it has no pending request, is not active on another output, and lists the output's CRTC in `possible_crtcs` (16.0.2 `drm.c:360-379`; 12.0.5 `drm.c:196-215`).<br>- **16.0.2** builds a per-output list of `drm_plane_handle`s: every plane that the CRTC may use, except the scanout and cursor planes (`drm.c:2630-2725`; `drm-internal.h:536-544`). A plane shared by two CRTCs thus appears on both outputs, but it is used by whichever output takes it first in a repaint cycle.<br>- **12.0.5** walks `device->plane_list` with the same availability test (`state-propose.c:506-512`, `:577`). | read from source (16.0.2), read from source (12.0.5) |

## 4. What happens to a view that is YUV, scaled, constrained or shares hardware

| # | Case | 16.0.2 | 12.0.5 | Tag |
| --- | --- | --- | --- | --- |
| V1 | **Format and modifier** | The FB gets a `plane_mask` of every non-cursor plane whose format list holds its format and modifier (`fb.c:676-711`, `:815-829`). If no plane matches, it fails with `FB_FORMAT_INCOMPATIBLE`. Candidates are then limited to that mask (`state-propose.c:812-814`) | Same idea (`fb.c:770-774`; `state-propose.c:520-527`) | read from source (16.0.2), read from source (12.0.5) |
| V2 | **YUV** | Format support is as in V1. For a YUV FB, the plane must support the colour encoding and range of the surface's colour representation, through `COLOR_ENCODING` and `COLOR_RANGE`. A plane without the property takes only the default (`state-propose.c:235-286`). The values are written to the plane state | No YUV-specific check, and no `COLOR_ENCODING` or `COLOR_RANGE` anywhere in `libweston/backend-drm/` (search). Any surface colour transform, or a non-identity pipeline, forces the renderer (`state-propose.c:839-844`) | read from source (16.0.2), read from source (12.0.5) |
| V3 | **Scaling and cropping** | No userspace check. Source and destination are computed from the paint node (`buffer_source_*`, `output_dest`), and the kernel's `TEST_ONLY` decides (`state-helpers.c:234-271`). Only the cursor path refuses scaling or cropping (`state-propose.c:357-366`) | Same; only the cursor refuses (`state-propose.c:234-243`) | read from source (16.0.2), read from source (12.0.5) |
| V4 | **Rotation and transform** | The node needs a "simple" transform (a 90° multiple) that maps to the plane's `rotation` property. Otherwise it gets `INCOMPATIBLE_TRANSFORM` (`drm-internal.h:850-862`; `state-propose.c:798-810`) | Same (`drm-internal.h:779`; `state-propose.c:506-518`, `:105-109`) | read from source (16.0.2), read from source (12.0.5) |
| V5 | **Plane alpha, blend, fences, colour pipeline** | A plane is skipped when:<br>- the node is not opaque and the plane has no alpha range;<br>- the node has a colour transform and the plane has no colour pipeline;<br>- the node has an acquire fence and the plane has no `IN_FENCE_FD`<br>(`state-propose.c:660-707`). The blend mode is chosen from `NONE`, `PREMULT` and `COVERAGE` (`:128-167`) | Alpha and fence checks only (`state-propose.c:588-611`). An acquire fence on a plane without `IN_FENCE_FD` ends the search for that view (`return NULL`, `:606-611`) | read from source (16.0.2), read from source (12.0.5) |
| V6 | **Shared hardware (scalers, fetch units; this project's resource groups)** | No model. The only knowledge is the per-plane availability of row P10 and the kernel's answer to `TEST_ONLY` (row P4). A refused test in mixed mode tries the next candidate plane for that node, then the renderer | Same | read from source (16.0.2), read from source (12.0.5); "no model" is inferred from the absence of any such structure in the files read |
| V7 | **On more than one output** | No such check was found in `drm_output_propose_state`. Paint nodes are per output (`compositor.c:516-569`) | Forced to the renderer: "on multiple outputs" (`state-propose.c:808-814`) | read from source (16.0.2), read from source (12.0.5) |
| V8 | **SHM buffers** | Cursor plane only: ARGB8888 and no larger than the cursor (`state-propose.c:581-624`, `:760-787`) | Same (`state-propose.c:470-498`) | read from source (16.0.2), read from source (12.0.5) |
| V9 | **Content protection** | A protected surface on an unsecured output is censored and kept on the renderer (`state-propose.c:1524-1529`; `compositor.c:367-382`, `:433-437`) | Forced to the renderer (`state-propose.c:858-866`) | read from source (16.0.2), read from source (12.0.5) |

Every refusal is recorded as a bit set, `try_view_on_plane_failure_reasons`,
on the paint node. It is used for debug logging and for dma-buf feedback
(section 6). It is not exported through any API (inferred; no such accessor
appears in `libweston.h` or `backend-drm.h`, rows A1 and A2).

## 5. DRM master, launcher, devices, outputs and planes

| # | Finding | Tag |
| --- | --- | --- |
| M1 | **No `drmSetMaster` in libweston.** Neither tree calls `drmSetMaster`, `drmDropMaster` or `drmIsMaster` anywhere in `libweston/` (search). The device is opened through the launcher, `weston_launcher_open(launcher, filename, O_RDWR)` (16.0.2 `backend-drm/drm.c:4375`). Master comes with the seat session, through libseat (or logind) | read from source (16.0.2), read from source (12.0.5) for the search; "master comes with the session" is inferred |
| M2 | **Launchers.** 16.0.2 has libseat only (`launcher-util.c:40`; `launcher-libseat.c:126` `libseat_open_device`, `:249` `libseat_open_seat`). 12.0.5 has libseat and, if built with it, a direct logind launcher (`launcher-util.c:39-47`). If no launcher connects, the DRM backend fails: "your system should either provide the logind D-Bus API, or use seatd" (16.0.2 `drm.c:4705-4711`; 12.0.5 `drm.c:3753`) | read from source (16.0.2), read from source (12.0.5) |
| M3 | **Session switch.** When the session goes inactive, input is disabled, the compositor goes offscreen and repaints are cancelled. When it comes back, device recovery is requested (16.0.2 `drm.c:4297-4329`; 12.0.5 `drm.c:3235`) | read from source (16.0.2); 12.0.5 located, not compared line by line |
| M4 | **Device choice.** One main KMS device: the configured `specific_device`, else a boot-VGA PCI device on the seat, else the first `card*` with CRTCs, connectors and encoders (16.0.2 `drm.c:4361-4496`, `:4722-4729`). `additional_devices` adds scanout-only devices (`:4620-4648`) | read from source (16.0.2) |
| M5 | **Planes.** All planes of the device are created at start-up with type, `zpos` and alpha ranges, formats and modifiers, and (16.0.2) colour pipelines. They are kept sorted by `zpos_max` (16.0.2 `drm.c:1444-1540`, `:1654-1684`; 12.0.5 `drm.c:1013`, `:1199` `create_sprites`). If the driver exposes no `zpos`, fake values are computed (16.0.2 `drm.c:135-211`, `:4605-4606`) | read from source (16.0.2); 12.0.5 located only |
| M6 | **Outputs.** At enable, the output reserves a CRTC, takes a free primary plane as scanout and an optional cursor plane (`drm_output_find_special_plane`), and installs `drm_assign_planes` as `output->base.assign_planes` (16.0.2 `drm.c:2893-2966`, `:1549-1598`; 12.0.5 `drm.c:2119-2165`, `:1949-1981`). Weston owns these CRTCs, connectors and planes for as long as the output is enabled (inferred) | read from source (16.0.2), read from source (12.0.5) |

## 6. Client buffers to paint nodes, and linux-dmabuf feedback

| # | Finding | Tag |
| --- | --- | --- |
| N1 | **Paint nodes, 16.0.2.** A paint node exists per (view, output) pair. It is created on demand while the output's z-order list is rebuilt from the compositor's view list, which is built from the layers (`compositor.c:3582-3596`, `:3681-3719`, `:3721-3752`).<br>Each repaint runs:<br>1. `paint_node_update_early` (`:318-410`, called at `:3994`). It updates the matrices, the "simple transform" and alpha, marks solid and censored nodes, and calls `renderer->attach` when the buffer is dirty.<br>2. `output_update_visibility` (`:3997`).<br>3. `output_assign_planes` (`:3857-3881`, called at `:3999`). It always calls the backend's `assign_planes`, even with `disable_planes`.<br>4. `paint_node_update_late` (`:446-514`, called at `:4003`).<br>5. `output->repaint` (`:4008`) | read from source (16.0.2) |
| N2 | **Paint nodes, 12.0.5.** Paint nodes are rebuilt with the view list at every repaint (`compositor.c:2982-3011`, called at `:3057`). `paint_node_update` only updates matrices (`:149-168`). `assign_planes` is skipped when `disable_planes` is set; all views then go to the compositor-wide primary plane (`:3078-3090`). In 12.0.5 the primary plane is per compositor (`ec->primary_plane`), in 16.0.2 per output (`output->primary_plane`, `libweston.h:428`) | read from source (12.0.5), read from source (16.0.2) |
| D1 | **Scanout formats.** At backend start, the backend computes the union of formats and modifiers over all non-cursor planes, intersected with the renderer's dma-buf formats, and stores their indices in the feedback format table (16.0.2 `drm.c:2772-2830`, `:4834-4850`; 12.0.5 `drm.c:2021`, `:3907`). These are device-wide, not per plane or per output (inferred from the union) | read from source (16.0.2), read from source (12.0.5) |
| D2 | **Per-surface feedback.** A surface that asks for feedback starts with one renderer tranche (`linux-dmabuf.c:882-905` in 16.0.2; `:869-893` in 12.0.5). A scanout tranche uses the scanout indices (16.0.2 `:450-455`; 12.0.5 `:440-445`) | read from source (16.0.2), read from source (12.0.5) |
| D3 | **Scanout tranche logic.** After each assignment, `dmabuf_feedback_maybe_update` reads the paint node's failure reasons (16.0.2 `state-propose.c:469-579`, called at `:1735-1737`; 12.0.5 `:338-433`, called at `:1032-1035`).<br>- *Remove the scanout tranche* when re-allocating cannot help. In 12.0.5 that is `FORCE_RENDERER` or `NO_PLANES_AVAILABLE`. 16.0.2 adds: no planes, transform, content protection, buffer type or size, alpha, no GBM, colour transform, solid, occluded by the renderer, output colour effect.<br>- *Add it* when re-allocating might help (add-FB, format, modifier, GBM import or handle failures), or when there was no failure.<br>A change is sent only after it has held for 2 s (`MAX_TIME_SECONDS`) | read from source (16.0.2), read from source (12.0.5) |
| D4 | **Direction of the protocol.** The tranches tell a client which format and modifier pairs might scan out. Nothing in this protocol path lets the client ask for a plane, a zpos or an output, or tells it which plane it got. Presentation feedback carries only `ZERO_COPY` (row P9) | read from source (16.0.2), read from source (12.0.5) for the code paths; "nothing lets the client ask" is inferred from them |
| D5 | linux-dmabuf is advertised at version 5 in 16.0.2 and version 4 in 12.0.5 when a default feedback exists, else version 3 (16.0.2 `linux-dmabuf.c:1104`; 12.0.5 `:1088`) | read from source (16.0.2), read from source (12.0.5) |

## 7. Plugin points and where `advise` could go

| # | Finding | Tag |
| --- | --- | --- |
| A1 | **`weston_drm_output_api`** (`"weston_drm_output_api_v1"`) has exactly five members in both releases, at the same lines (`include/libweston/backend-drm.h:57-107`):<br>- `set_mode` (`:63`)<br>- `set_gbm_format` (`:73`)<br>- `set_seat` (`:79`)<br>- `set_max_bpc` (`:95`)<br>- `set_content_type` (`:105`)<br>The backend registers the same five (16.0.2 `drm.c:4650-4656`; 12.0.5 `drm.c:3692-3698`). None concerns planes | read from source (16.0.2), read from source (12.0.5) |
| A2 | **The other DRM APIs.** `weston_drm_virtual_output_api` (`"..._v2"`, `backend-drm.h:119-184` in 16.0.2) has `create_output`, `set_gbm_format`, `set_submit_frame_cb`, `get_fence_sync_fd`, `buffer_released` and `finish_frame`, for a renderer-only virtual output; virtual outputs never get planes (`drm.c:365-366`). `weston_drm_backend_config` (version 6 in both) holds start-up options. 16.0.2 adds `offload_blend_to_output` and `disable_drm_state_reuse` (`backend-drm.h:262-271`); 12.0.5 ends at `additional_devices`. None assigns planes or exposes plane state | read from source (16.0.2), read from source (12.0.5) |
| A3 | **The only plane-assignment entry point** is the function pointer `void (*assign_planes)(struct weston_output *)` in the public `struct weston_output` (16.0.2 `libweston.h:547`; 12.0.5 `libweston.h:538`). The DRM backend sets it at output enable (row M6), and the core calls it each repaint (row N1). `struct weston_plane` holds only a compositor pointer in 16.0.2 (`libweston.h:381-383`) | read from source (16.0.2), read from source (12.0.5) |
| A4 | Building a DRM plane state needs `drm_output_state`, `drm_plane_state`, `drm_pending_state` and `drm_plane`. These are declared in `backend-drm/drm-internal.h`, which is private to the backend and not under `include/libweston/`. The public headers offer no call to propose, test or commit a plane state | read from source (16.0.2), read from source (12.0.5) for where the types are declared; "not reachable from a module" is inferred |
| A5 | Searching `libweston.h` for plane and plugin symbols turned up only:<br>- `assign_planes`<br>- `disable_planes` (an integer counter; 16.0.2 `weston_output_disable_planes_incr`, `compositor.c:11360`)<br>- `paint_node_z_order_list`<br>- `plugin_api_list` (the generic `weston_plugin_api_register` and `get` registry)<br>- `weston_module_init`<br>No signal or callback runs before or after plane assignment with the candidate set | read from source (16.0.2), read from source (12.0.5) |

**Answer to the hook question (inferred, from A1 to A5 and section 3).**

There is no hook in either release where `composition_core::advise` could be
consulted for plane assignment.

A module could overwrite `output->assign_planes` after enable. It would then
replace the DRM backend's planner without access to the backend's private plane
state, so in practice it is a fork of the backend.

The place for a patch is `drm_assign_planes` or `drm_output_propose_state` in
`libweston/backend-drm/state-propose.c` (16.0.2 `:1286`, `:1646`; 12.0.5
`:667`, `:963`). The patch would:

1. Build a `ce_frame` from `paint_node_z_order_list`.
2. Call `ce_advise` (C ABI, `crates/composition-ffi/include/composition_engine.h:450`;
   read from source (this repo)), with a `check` callback that wraps
   `drm_pending_state_test`.
3. Use the result to pre-place nodes on planes and to send the "compose" nodes
   to the renderer in mixed mode.

**Mismatches such a patch would have to handle (inferred):**

- `advise` composes one contiguous run, at the run's own position in the stack
  (`crates/composition-core/src/advise.rs` module doc, lines 9-17; read from
  source (this repo)).
- Weston's renderer buffer sits on the primary plane at its lowest zpos, below
  every overlay (rows P4, P6).
  - 12.0.5 cannot place any plane below it.
  - 16.0.2 can, but only fully opaque nodes on underlay planes with a hole
    punched (P5, P6).
- So a run that `advise` composes in the middle of the stack has no Weston
  counterpart in 12.0.5. In 16.0.2 it has one only when the layers below the
  run are opaque and underlay planes exist.
- Weston also cannot refuse a frame (P1, P2), while `advise` may report layers
  as composed for `Budget` or `Backend` reasons. The patch would have to fall
  back to Weston's own walk rather than fail. This is the semantic gap W5-04
  names.

## 8. Differences between 12.0.5 and 16.0.2 that matter here

| Topic | 12.0.5 | 16.0.2 | Rows |
| --- | --- | --- | --- |
| Modes | planes-only → mixed → renderer-only | reuse → planes-only → mixed → renderer-and-cursor or renderer-only | P1, P2 |
| Planes below the renderer | Refused in mixed mode | Underlays with hole punching, for fully opaque nodes | P5, P6 |
| Solid colour | Renderer | Background region or CRTC `BACKGROUND_COLOR` in planes-only mode | P7 |
| YUV encoding and range | Not set; any colour transform forces the renderer | `COLOR_ENCODING` and `COLOR_RANGE` checked and set; colour pipelines may offload a transform | V2, V5 |
| Multi-output views | Renderer | No such check found | V7 |
| Planes per output | Device plane list, filtered by availability | Per-output `drm_plane_handle` list | P10 |
| Launchers | libseat, logind | libseat | M2 |
| Paint nodes | Rebuilt per repaint; compositor-wide primary plane | Kept, dirty-tracked; per-output primary plane; `assign_planes` always called | N1, N2 |
| dma-buf feedback | Scanout tranche removed only for "forced renderer" or "no planes" | Many more removal reasons | D3 |
| Plane-assignment hook | None | None | A1 to A5 |
| Lease code | None | None | R1 |

## 9. What this read does not claim

- Nothing about NXP's `weston-imx-12.0.5` beyond its stated upstream base.
  NXP's patches (for example `imxg2d`, `docs/weston-imx-lease.md` row L4) may
  change plane assignment and were not read.
- Nothing measured. Whether a given i.MX95, AM62P or RK3588 plane takes a
  given YUV, scaled or rotated buffer is decided by the kernel's `TEST_ONLY` in
  both releases (row V3). Only a board can answer it.
- Line numbers are for the two commits above. Weston `main` has moved since
  `c8d00e2`, and the gap analysis' `main` rows were not re-read here.
- No Weston code is copied into this repository. The quoted fragments are
  short identifiers, a few-word comment or a log string.
