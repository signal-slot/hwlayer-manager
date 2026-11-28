# NXP weston-imx 12.0.5 against upstream 12.0.5, read from source (WP5-02 remainder, W5-01, W5-03)

Desk read that closes the open part of `docs/weston-release-read.md` §9
("nothing about NXP's `weston-imx-12.0.5`"). Written 2026-11-28. Nothing here
was built, run or measured, and no board was touched. It adds to the release
read and to `docs/weston-imx-lease.md`; it edits neither.

## What was read

The NXP tree was cloned on 2026-11-28 from `https://github.com/nxp-imx/weston-imx.git`
(`--depth 300 --branch weston-imx-12.0.5`). The upstream base commit is inside
that history, so the comparison is a plain `git diff` in one clone.

| Label | Ref | Commit | Commit date | Note |
| --- | --- | --- | --- | --- |
| **NXP 12.0.5** | branch `weston-imx-12.0.5` (no tag of that name; branch tip on 2026-11-28) | `a29bbd0f65e68b9beda47a94144bd9b2801c42cb` | 2025-09-23 (author date of the tip commit: 2025-08-30) | `meson.build` says `version: '12.0.5'` |
| **Upstream 12.0.5** | tag `12.0.5` | `3d2cd9978c68c6338e902e9369266dbe8c4766c3` | 2025-04-29 | The same commit `docs/weston-release-read.md` pins. It is an ancestor of the NXP tip: `git merge-base` returns it, so NXP is upstream 12.0.5 plus 79 commits (`git rev-list --count`) |

Other NXP branches seen with `git ls-remote` on 2026-11-28 and **not read**:
`weston-imx-12.0.3` (`b0d890915ee191ceb10e79ea9a830281f5c6258f`),
`weston-imx-12.0.4` (`de7f8df4b4275e9bb345cacab6962d1a490d9eab`),
`weston-imx-11.0.1`. There is no branch for a Weston newer than 12.0.x in that
listing, and no tag for 12.0.5.

Source links (pinned to the commit, tag 12.0.5 style as in the release read):

| File | NXP 12.0.5 |
| --- | --- |
| `state-propose.c` | <https://github.com/nxp-imx/weston-imx/blob/a29bbd0f65e68b9beda47a94144bd9b2801c42cb/libweston/backend-drm/state-propose.c> |
| `drm.c` | <https://github.com/nxp-imx/weston-imx/blob/a29bbd0f65e68b9beda47a94144bd9b2801c42cb/libweston/backend-drm/drm.c> |
| `kms.c` | <https://github.com/nxp-imx/weston-imx/blob/a29bbd0f65e68b9beda47a94144bd9b2801c42cb/libweston/backend-drm/kms.c> |
| `fb.c` | <https://github.com/nxp-imx/weston-imx/blob/a29bbd0f65e68b9beda47a94144bd9b2801c42cb/libweston/backend-drm/fb.c> |
| `backend-drm.h` | <https://github.com/nxp-imx/weston-imx/blob/a29bbd0f65e68b9beda47a94144bd9b2801c42cb/include/libweston/backend-drm.h> |

What was compared (`git diff 3d2cd99..a29bbd0`, 37 files, +3857 −216 lines):

- In full: `libweston/backend-drm/state-propose.c`, `state-helpers.c`, `fb.c`,
  `kms.c`, `drm.c`, `drm-internal.h`, `include/libweston/backend-drm.h`,
  `libweston/backend.h`, and the compositor-side diffs in `compositor.c`,
  `libweston-internal.h`, `linux-dmabuf.h`.
- Only skimmed by symbol: `renderer-gl/gl-renderer.c`, `linux-dmabuf.c`,
  `drm-gbm.c`, `compositor/main.c`, `weston.ini.in`.
- Not read: the body of `libweston/renderer-g2d/g2d-renderer.c` (2510 new
  lines) beyond its existence and its interface use from the DRM backend.
- `libweston/backend-drm/modes.c` is **not changed** by NXP (empty diff).

Tags used below:

- **read from source (2026-11-28)**: read on that date at the file:line given,
  in the NXP tree above (paths relative to the tree root; `backend-drm/` is
  `libweston/backend-drm/`). "Diff" means the line is added or changed against
  upstream 12.0.5.
- **inferred**: a conclusion from the rows it names, not read anywhere.
- **not measured**: nothing was run, so no behaviour or figure is claimed.

## 1. Answers

- **Does NXP's fork have DRM lease support?** No. A case-insensitive
  whole-word search for `lease`, `leases`, `leasing`, `leased`, `drm-lease` and
  `drm_lease` over the whole tree (`.git` and `*.po` excluded) returns **no
  hits**, and a substring search for `drm.?lease`, `CREATE_LEASE` and
  `wp_drm` returns none (row L1). The protocol directory has no
  `drm-lease-v1`. This agrees with upstream 12.0.5 (release read row R1) and
  with `docs/weston-imx-lease.md` row L4 (read from source (2026-11-28)).
- **Does NXP change plane assignment?** Yes, in the DRM backend itself.
  `drm_output_propose_state` and `drm_output_find_plane_for_view` are modified
  to allow overlay planes below the primary plane ("underlay"), with a "through
  hole" punched in the renderer image, and the plane-busy check now spans all
  outputs of a repaint (rows P1 to P5). This is a different change from the
  underlay code of upstream 16.0.2 (release read row P6), though it solves the
  same problem.
- **Is there a hook for an advisory planner?** No new one. The public header
  diff adds only config fields (`use_g2d`, `enable_overlay_view`,
  `shell_width`, `shell_height`; row H1). `weston_drm_output_api` is not
  touched. `assign_planes` is still the only entry point and the plane-state
  types are still private (row H2). Release read §7 therefore stands for the
  NXP fork: the place to consult `composition_core::advise` is inside
  `drm_output_propose_state`, as a patch (inferred, rows P1 to P6, H1, H2).
- **No i.MX DPU or plane-hardware model.** There is no DPU, DCSS or LCDIF
  symbol in the tree (row D1). NXP's plane support is generic KMS plus Vivante
  modifier and Amphion tile handling (row D2), and G2D is a renderer, not a
  plane (row G1). Whether an i.MX95 plane takes a given buffer is still decided
  by the kernel's `TEST_ONLY` (inferred from rows P1 to P6; not measured).

## 2. Lease support

| # | Finding | Tag |
| --- | --- | --- |
| L1 | Whole-word, case-insensitive search for `lease`, `leases`, `leasing`, `leased`, `drm-lease`, `drm_lease` over every text file (`.git` and `*.po` excluded): **0 files**. Substring search for `drm.?lease`, `CREATE_LEASE`, `wp_drm`: 0 hits. No `DRM_IOCTL_MODE_CREATE_LEASE` and no `drmModeCreateLease` call | read from source (2026-11-28) |
| L2 | The only master-related call in `libweston/` is `drmSetMaster` at `backend-drm/kms.c:1548`, **a diff line**: before an atomic commit it checks `drmGetMagic`/`drmAuthMagic` and, if that fails, calls `drmSetMaster` again ("drm master was set by systemd in PM test"). Upstream 12.0.5 has no `drmSetMaster` in `libweston/` (release read row M1). See the consequence in §4 | read from source (2026-11-28) |
| L3 | The launcher is unchanged in kind (libseat and logind). The diff adds a retry loop in `launcher-libseat.c` and `launcher-logind.c`, and ignores udev `gone` events for the main DRM device. None concerns leases | read from source (2026-11-28) |

## 3. Plane assignment changes (backend-drm)

| # | Finding | Tag |
| --- | --- | --- |
| P1 | **Underlay in mixed mode.** Upstream 12.0.5 refuses any overlay plane whose `zpos_max` is not above the primary's zpos in mixed mode. NXP **removes that check** (`backend-drm/state-propose.c`, near `:617`, diff). Instead, when a view is placed on a plane whose `zpos` is below the scanout (primary) plane's, `pnode->need_through_hole` is set (`state-propose.c:638-647`, diff) | read from source (2026-11-28) |
| P2 | **Two zpos cursors.** `drm_output_propose_state` keeps `current_lowest_zpos_overlay` and `current_lowest_zpos_underlay` (`state-propose.c:684-687`, `:751`, `:787-789`, diff). The underlay cursor starts at the primary's zpos. A view that intersects the "bottom region" (renderer region plus underlay region) is no longer forced to the renderer: it is tried on the underlay cursor (`:864-875`, diff). Upstream 12.0.5 forced the renderer in that case (release read row P5) | read from source (2026-11-28) |
| P3 | **Bottom region.** `renderer_region` is renamed `bottom_region` and also receives the area of views placed below the primary (`state-propose.c:679`, `:760`, `:918-930`, diff) | read from source (2026-11-28) |
| P4 | **Plane-busy check across outputs.** `drm_output_check_plane_has_view_assigned` takes the whole `drm_pending_state` and walks every output state in it, so a plane taken by another output's state in the same repaint is not offered again (`state-propose.c:67-81`, `:451`, `:539`, `:586`, diff; commit `d355a5a` "fix drm plane repeated picked when do state propose"). Upstream 12.0.5 checks only the current output's state. Relevant to any shared-plane scenario (release read row P10) | read from source (2026-11-28) |
| P5 | **Hole bookkeeping.** `need_through_hole` is added to `struct weston_paint_node` (`libweston/libweston-internal.h:558`, initialised at `compositor.c:199`, diff), reset when a view falls back to the renderer (`state-propose.c:1098`), and consumed by the GL renderer (`renderer-gl/gl-renderer.c:1327`) and by `drm-gbm.c:324`. This is a hole in the renderer output, like 16.0.2's `need_hole`, with a different name and a different implementation | read from source (2026-11-28) |
| P6 | **Unchanged.** The mode order and fallback of `drm_assign_planes` (planes-only, mixed, renderer-only, renderer-only without writeback, then `assert`), the top-to-bottom walk with no backtracking, the mixed-mode `TEST_ONLY` per placement, and the rule that the primary gets `zpos_min` all stay as in upstream 12.0.5 (release read rows P2 to P4). No reuse mode, no solid-colour background, no `COLOR_ENCODING`/`COLOR_RANGE` handling is added | read from source (2026-11-28); the "unchanged" part is read from the diff, which has no hunk there |
| P7 | **Opaque region.** `compositor.c` (`weston_view_update_transform_*`, diff) drops the shortcut that treated a whole opaque surface's bounding box as its opaque region and uses `surface->opaque` instead. This feeds the occlusion region and so plane eligibility | read from source (2026-11-28); effect on plane choice inferred |
| P8 | **Scaling coordinates.** `drm_plane_state_coords_for_paint_node` (`backend-drm/state-helpers.c`, diff) applies a second scale when `buffer.scale` differs from the output's `current_scale`. This changes the destination rectangle proposed to the kernel for scaled views | read from source (2026-11-28) |
| P9 | **Mixed-mode primary size.** The check that the previous renderer FB matches the mode size now accepts GBM-aligned sizes (`ALIGNTO`, 64 for Vivante super-tiled modifiers) (`state-propose.c:725-731`, `drm.c:411-433`, diff). The primary plane's source rectangle becomes the mode size rather than the FB size (`drm.c:457-462`, diff) | read from source (2026-11-28) |

## 4. Buffers, formats and other backend changes

| # | Finding | Tag |
| --- | --- | --- |
| H1 | **Public header.** The diff to `include/libweston/backend-drm.h` adds `use_g2d` (under `ENABLE_IMXG2D`), `enable_overlay_view`, `shell_width`, `shell_height`, and an `argb8888` mention in a comment. `include/libweston/libweston.h` adds `WESTON_RENDERER_G2D = 4`. No new plugin API, no change to `weston_drm_output_api` | read from source (2026-11-28) |
| H2 | **Plane state is still private.** `drm_plane_state`, `drm_output_state` and `drm_pending_state` stay in `backend-drm/drm-internal.h`, which NXP extends (`dtrc_meta`, `use_g2d`, `enable_overlay_view`, `shell_width`/`shell_height`, three dumb buffers) but does not publish. Nothing under `include/libweston/` lets a module propose, test or read a plane state | read from source (2026-11-28) |
| H3 | **Backend hooks added to the core.** `struct weston_backend` gains `get_supported_formats` and `import_dmabuf` (`libweston/backend.h`, diff). `weston_compositor_import_dmabuf` tries the backend first (`compositor.c:8797-8798`, diff). The DRM backend answers true only for NV15 (or NV12_10LE40) on an overlay plane (`drm.c` `drm_import_dmabuf`, diff). These are format-import hooks, not plane-assignment hooks | read from source (2026-11-28) |
| H4 | **`enable_overlay_view`.** The option `enable-overlay-view` (`weston.ini`, read at `compositor/main.c:3017`; default 0) makes the backend create a flag file `enable-overlay-view` in `XDG_RUNTIME_DIR` (`drm.c:4220-4232`) and delete it on destroy (`:3294-3304`), and only then import WL buffers as overlay candidates (`fb.c:819-826`, diff). With the option off, `RENDERER_OPAQUE` wl_buffers do not get a GBM bo here. The file's consumer is not in this tree (searched; not found) | read from source (2026-11-28); consumer not found |
| H5 | **dma-buf additions.** `linux-dmabuf.c` adds a `dtrc_meta` (DTRC, Vivante compression metadata) request and per-buffer GEM-handle caching (`fb.c`, `linux-dmabuf.h`, diff). `kms.c` writes a `dtrc_table_ofs` plane property for overlay planes when the FB has a non-linear modifier (`kms.c:1380-1385`, diff). The format table falls back to `DRM_FORMAT_MOD_LINEAR` instead of `DRM_FORMAT_MOD_INVALID` where a plane lists no modifiers (`kms.c:595`, diff) | read from source (2026-11-28) |
| H6 | **Atomic commit changes.** The primary plane gets an `IN_FENCE_FD` from the GBM surface or from G2D (`kms.c:1183-1200`, `:1357-1360`, diff). HDR10 metadata, via a `zwp_hdr10_metadata_v1` global, is written to the connector blob (`drm.c` and `kms.c`, diff). HDCP updates and HDR changes now add `DRM_MODE_ATOMIC_ALLOW_MODESET` | read from source (2026-11-28) |
| H7 | **Legacy path asserts removed.** `drm_output_apply_state_legacy` drops the asserts that the primary's source and destination equal the mode size (`kms.c:843-851`, diff) | read from source (2026-11-28) |

## 5. i.MX display hardware and G2D

| # | Finding | Tag |
| --- | --- | --- |
| D1 | A case-insensitive search for `dpu`, `dcss`, `lcdif` over the tree finds only unrelated hits (the RDP backend's `rdputil`, `rdpUpdate`). **No i.MX display-controller (DPU/DCSS/LCDIF) symbol exists**, and no board, SoC or compatible string selects behaviour. The fork keys its changes to features (`ENABLE_IMXG2D`, modifiers, `enable-overlay-view`), not to a controller | read from source (2026-11-28) |
| D2 | Hardware-specific code in the DRM backend is limited to buffer formats: `DRM_FORMAT_MOD_VIVANTE_SUPER_TILED(_FC)` alignment (`fb.c:208-230`, `:376-395`), `DRM_FORMAT_MOD_AMPHION_TILED` AddFB sizing (`fb.c:222-229`), NV15/NV12_10LE40 import (row H3) and DTRC (row H5) | read from source (2026-11-28) |
| G1 | **G2D renderer.** `libweston/renderer-g2d/` is new (`g2d-renderer.c` +2510 lines, `g2d-renderer.h`, `meson.build`). The meson option `renderer-g2d` defaults to `true` (`meson_options.txt`, diff). `drm_output_render_g2d` draws into dumb buffers on the primary plane. G2D is therefore a software-side substitute for the GL renderer on the primary plane; it is not a plane and not an overlay path. The fork's commit log (79 commits, `git log 3d2cd99..a29bbd0`) has 27 subjects that mention `g2d-render` | read from source (2026-11-28); the body of `g2d-renderer.c` was not read |
| G2 | The tree's `SCR.txt` describes the base as `Version: 12.0.4.imx`, `Outgoing License: MIT`, `Release Location: ... -b lf-6.6.23-2.0.0`, origin NXP (MIT) and Weston (MIT). That file is stale against `meson.build` (12.0.5). Licence of the fork: MIT, like upstream (`LICENSE`, renamed from `COPYING` in commit `cf41259`) | read from source (2026-11-28) |

## 6. What this means

**For WP5-01 (`docs/weston-imx-lease.md`).**

- The lease question for the NXP fork is answered from source: it has none
  (row L1). If the Verdin iMX95 image runs this fork, Weston itself cannot hand
  a lease to anyone (inferred). This read does not say which Weston is on the
  image; that is the board observation of plan item `imx95-weston-version`,
  and the version string there, not this file, decides whether the fork applies.
- The one lease-relevant difference is row L2. NXP's atomic path calls
  `drmSetMaster` when the device fd is not authenticated. If Weston and
  `drm-lease-manager` share a DRM device, that call is a place where Weston
  may try to retake master that another process holds. Whether it matters for a
  lease holder is **not measured** and not read (the lease is created by a
  master, so who is master at the moment of the call is what counts). It
  belongs on the board checklist of WP5-07/WP5-01, not in a conclusion.

**For WP5-02 and the gap analysis (W5-01, W5-03).**

- Release read §1 holds for the fork: no lease route, no plane-hint route, no
  hook (rows L1, H1, H2). Route 3 of the gap analysis ("Weston calls this
  engine") is still a patch to the DRM backend, now to a backend that already
  carries about 100 lines of NXP changes to the same function
  (`drm_output_propose_state`, rows P1 to P5). A patch for `advise` would have
  to be rebased on those changes if it targeted the NXP fork, and could not be
  offered upstream as is (inferred).
- The mismatch named in release read §7 for 12.0.5 (a run composed in the
  middle of the stack has no Weston counterpart because nothing can sit below
  the renderer buffer) is **weaker in the fork**: the fork allows overlay
  planes below the primary, with a through hole in the renderer image (rows P1,
  P2, P5). The condition is that such planes exist (an overlay with `zpos`
  below the primary's) and that the views below the run are opaque enough to
  need the hole only (inferred; whether the iMX95 DPU exposes such a plane is
  **not measured**, see `docs/imx95-board.md`).
- Weston still cannot refuse a frame (row P6): the fallback chain and the final
  `assert` are as in upstream 12.0.5. The W5-04 semantic gap stands.

**For the advisory planner (`docs/advisory-planner-design.md`).**

- Nothing in the fork gives a data source or a call site that the design does
  not already assume. There is no DPU resource model to compare against
  `DriverProfile::Imx95Dpu`; NXP encodes no shared-scaler or fetch-unit limit
  (row D1; same finding as the release read row V6). The profile data stays
  this project's own, and any cross-check against Weston on i.MX is a
  kernel `TEST_ONLY` comparison on the board (inferred; not measured).
- The underlay change (rows P1 to P5) is the closest Weston-side counterpart to
  `advise`'s "compose a contiguous run at its own position". It is a useful
  reference when WP5-04 compares what Weston and `advise` do with the same
  stack, but it works per view, top to bottom, with no cost ranking (row P6),
  so it is not a model of the budget reasons `advise` reports (inferred).

**For the upstreaming proposal.** Offering anything to Weston means a patch
against upstream `main`, not the NXP fork. This read finds nothing in the fork
that NXP offered upstream or that upstream could reuse, and it does not say
whether NXP would accept or carry such a patch. No acceptance is implied
(inferred).

## 7. What this read does not claim

- Nothing about the Weston actually installed on any board. Version, package
  and lease build flags on the Verdin iMX95 image are the board observation
  and may be a different branch or a newer NXP release.
- Nothing about NXP branches other than `weston-imx-12.0.5` at the commit
  above. `weston-imx-12.0.3` and `12.0.4` were listed, not read.
- Nothing measured: no build, no run, no timing, no check that a plane takes a
  buffer on i.MX95.
- The body of the G2D renderer (2510 lines) and the GL renderer diff were not
  read line by line; claims about them are limited to the symbols named.
- Line numbers are for `a29bbd0f65e68b9beda47a94144bd9b2801c42cb`. Where a row
  says "near" the line was located by the diff hunk, not recounted.
- No Weston or NXP code is copied into this repository. The quoted fragments
  are short identifiers or a few-word comment.
