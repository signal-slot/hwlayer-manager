# Atomic async (cursor) update in vc4 HVS5/HVS6 and VOP2, from source

Status: source read, written 2026-11-28. **No hardware result is claimed.** Nothing was run on a
board. This is the prediction that the cursor-move case of `pending_probe` must confirm or refute
on the RPi5 (HVS6), the reTerminal CM4 (HVS5) and the NanoPC-T6 (VOP2). It bears on question 6 of
[`hvs-backend-design.md`](hvs-backend-design.md) §7 (`replace_pending`) and question 9 of
[`rk3588-backend-design.md`](rk3588-backend-design.md) §7 (flip and latch).

Related source reads (not edited here):
[`drm-pending-commit-source.md`](drm-pending-commit-source.md) (generic helper, second nonblocking
commit), [`vc4-replace-pending-source.md`](vc4-replace-pending-source.md) (vc4, HVS Q6) and
[`rk3588-vop2-flip-source.md`](rk3588-vop2-flip-source.md) (VOP2 flip). A `vc4-pending-source.md`
does not exist on this tip, so it is not linked.

## Tags and pins

- **read from source**: read in one of the trees below, with file:line. Line numbers are exact
  (files were fetched whole by raw URL on 2026-11-28 and read locally).
  - `rpi`: `raspberrypi/linux`, branch `rpi-6.18.y`, commit
    `16ae2649dcfadf9d81445b1177a3191efa3cb82e`, files under `drivers/gpu/drm/`.
    Fetched from
    `https://raw.githubusercontent.com/raspberrypi/linux/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4/vc4_plane.c`
    and the same path for `vc4_kms.c`, `vc4_crtc.c`, `drm_atomic_helper.c`, `drm_atomic_uapi.c`,
    `drm_plane.c`.
  - `v6.18`: `torvalds/linux`, tag `v6.18`, files under `drivers/gpu/drm/`
    (`https://raw.githubusercontent.com/torvalds/linux/v6.18/drivers/gpu/drm/rockchip/rockchip_drm_vop2.c`,
    and the same tag for `rockchip_drm_fb.c`, `drm_atomic_helper.c`, `drm_atomic_uapi.c`,
    `drm_plane.c`).
- **inferred**: a conclusion of this note. A line without a tag is inferred.
- The boards' running kernels are not compared here (HVS Q7, RK Q2 are other items).

## 1. The generic mechanism (read from source)

All of this is common DRM core code; the two trees differ only where marked.

1. A commit takes the async path only if `state->legacy_cursor_update` is set. The only setters are
   `drm_atomic_helper_update_plane` (when `plane == crtc->cursor`) and
   `drm_atomic_helper_disable_plane` (when the plane is its CRTC's cursor)
   (read from source: v6.18 `drm_atomic_helper.c:3346`, `:3384`; rpi `drm_atomic_helper.c:3414`,
   `:3452`). Nothing else sets it: the atomic ioctl code (`drm_atomic_uapi.c`) has no reference to
   `legacy_cursor_update` in either tree (read from source: grep of both files, no match).
2. The legacy cursor ioctl reaches that helper only if the CRTC has a universal cursor plane:
   `drm_mode_cursor_common` calls `drm_mode_cursor_universal` when `crtc->cursor` is set
   (read from source: v6.18 `drm_plane.c:1305-1315`; rpi `drm_plane.c:1305-1315`), which ends in
   `__setplane_atomic` (`drm_plane.c:1256`) and so `plane->funcs->update_plane`. If `crtc->cursor`
   is NULL it falls back to `crtc->funcs->cursor_set`/`cursor_move`, and returns `-ENXIO` when a
   `DRM_MODE_CURSOR_BO` request finds neither (`drm_plane.c:1319-1322`) or `-EFAULT` when a
   `DRM_MODE_CURSOR_MOVE` request finds no `cursor_move` (`:1332-1338`) (read from source, v6.18).
3. `drm_atomic_helper_check` sets `state->async_update = !drm_atomic_helper_async_check()` when
   `legacy_cursor_update` is set (read from source: v6.18 `:1125-1126`; rpi `:1130-1131`).
   `drm_atomic_helper_async_check` fails (not an error to the user: the commit just becomes a normal
   one) when:
   - a CRTC needs a modeset (`-EINVAL`);
   - the state holds other than exactly one plane (`-EINVAL`, v6.18 `:2048-2054`);
   - the plane has no CRTC or changes CRTC (`-EINVAL`, `:2056-2061`);
   - the plane's helper has no `atomic_async_update` (`-EINVAL`, `:2064-2069`);
   - the plane state carries a fence (`-EINVAL`, `:2071-2076`);
   - the plane's previous commit has not reached `hw_done` (`-EBUSY`, `:2082-2090`);
   - the driver's `atomic_async_check(plane, state, false)` returns non-zero.
   (read from source: v6.18 `drm_atomic_helper.c:2029-2098`; rpi `:2103-2175`, same conditions;
   the `-EBUSY` is at rpi `:2158-2162`.)
4. `drm_atomic_helper_commit` with `async_update` set prepares the planes, calls the driver's
   `atomic_async_update`, unprepares, and **returns 0**. It never calls
   `drm_atomic_helper_setup_commit`, so no `drm_crtc_commit`, no `flip_done` and no pageflip event
   is created for it (read from source: v6.18 `:2172-2179`; rpi `:2246-2255`).
5. If the async check fails, the commit is a normal one. A legacy cursor ioctl commits through
   `drm_atomic_commit` blocking, so `stall_checks` does not return `-EBUSY` (that needs
   `nonblock`, read from source: v6.18 `:2312-2318`; rpi `:2386-2392`); it waits instead. The
   ioctl returns 0 after the commit, or an error from check/prepare.
6. Mainline v6.18 has an extra rule that rpi-6.18.y lacks: with `legacy_cursor_update` set,
   `drm_atomic_helper_setup_commit` completes `flip_done` at once ("Legacy cursor updates are fully
   unsynced") and `drm_atomic_helper_wait_for_vblanks` returns early (read from source: v6.18
   `:2471-2474` and `:1818-1819`). In rpi `drm_atomic_helper.c` the only mentions of
   `legacy_cursor_update` are `:1130`, `:3414`, `:3452` (read from source: grep), so a
   synchronous fallback of a cursor update still waits for its vblank there (inferred from the
   missing code).
7. A cursor commit sent through the **atomic ioctl** (`DRM_IOCTL_MODE_ATOMIC`), which is what the
   engine's DRM backend and `pending_probe` use, never has `legacy_cursor_update`, so it never takes
   the async path, in either driver (inferred from item 1). It is an ordinary commit: with
   `NONBLOCK` while the CRTC's previous commit has not completed `flip_done`, `stall_checks`
   returns `-EBUSY` (read from source: v6.18 `:2312-2318`, called at `:2456`; rpi `:2386-2392`,
   called at `:2530`). An event is sent only if the request asks for one (`PAGE_FLIP_EVENT`).
   The plane is added to the CRTC's state, so the CRTC's commit list is checked even for a
   cursor-only request (inferred).

The separate `DRM_MODE_PAGE_FLIP_ASYNC` flag is a different path: it needs
`mode_config.async_page_flip` and, for a non-primary plane, the driver's `atomic_async_check(...,
true)` (read from source: v6.18 `drm_atomic_uapi.c:1095-1101` and `:1424-1431`). It is not the
cursor path and the engine does not use it.

## 2. vc4 HVS5 (BCM2711) and HVS6 (BCM2712), `rpi-6.18.y` 16ae2649

- **Async update exists: yes.** `vc4_plane_atomic_async_check` and `vc4_plane_atomic_async_update`
  are in both plane helper tables, `vc4_plane_helper_funcs` and `vc5_plane_helper_funcs`, so HVS5
  and HVS6 alike (read from source: rpi `vc4_plane.c:2593`, `:2667`, `:2750-2751`, `:2757-2758`).
  The check branches on generation: `vc4_plane_mode_set` for gen <= 5 (HVS5), `vc6_plane_mode_set`
  for HVS6 (`:2677-2680`), so both generations run the same checks below (read from source).
- **Which planes qualify.** Only a plane that is `crtc->cursor`: `vc4_plane.c:2991-2994` creates one
  `DRM_PLANE_TYPE_CURSOR` plane per CRTC and assigns it (`crtc->cursor = cursor_plane`)
  (read from source). Overlay and primary planes are not `crtc->cursor`, so a legacy
  `SETPLANE`/`SETCRTC` on them never sets `legacy_cursor_update` (inferred from §1 item 1). The
  cursor plane uses `drm_atomic_helper_update_plane`/`disable_plane` as `update_plane`/
  `disable_plane` (`vc4_plane.c:2840-2841`, read from source).
- **Driver conditions** (`vc4_plane_atomic_async_check`, read from source, `:2667-2720`):
  - the new state must build a valid dlist (`vc4_plane_mode_set`/`vc6_plane_mode_set` succeed and
    `hw_dlist` exists, else `-EINVAL`);
  - dlist length, the `pos0`, `pos2` and `ptr0` offsets and the line-buffer size must equal the old
    state's (else `-EINVAL`);
  - every dlist word other than `pos0`, `pos2`, `ptr0` (and the LBM word) must be unchanged
    (else `-EINVAL`).
  So a move or a framebuffer change at an unchanged size, format, scaling and blend qualifies.
  Enabling a disabled cursor, or changing its size, format, alpha or scaling, does not and falls
  back to the synchronous path (inferred from the conditions; the code comment says the same,
  `:2697-2698`).
- **Pending flip on the same plane.** `hw_done` of the plane's previous commit must have
  completed, else the generic check returns `-EBUSY` and the cursor commit becomes synchronous
  (read from source: rpi `drm_atomic_helper.c:2158-2162`). `hw_done` is signalled in
  `vc4_atomic_commit_tail` (`vc4_kms.c:526`, read from source). A flip pending on a *different*
  plane (for example the primary) does not hold this back, because the check looks only at the
  cursor plane's own `old_plane_state->commit` (inferred).
- **What the second commit returns.**
  - Legacy cursor ioctl, async-capable: **0**, immediately, hardware dlist words written in place
    (`vc4_plane.c:2655-2662`, read from source).
  - Legacy cursor ioctl, async refused: the commit is synchronous and blocking: **0** after it
    completes, not `-EBUSY` (inferred from §1 item 5).
  - Atomic ioctl, `NONBLOCK`, previous commit on the CRTC not yet `flip_done`: **`-EBUSY`**
    (§1 item 7). Atomic ioctl, blocking: waits, then 0.
- **Flip event.** None for the async path (§1 item 4). None for the legacy synchronous fallback
  that reaches userspace either: the legacy cursor ioctl takes no event argument, and the internal
  event allocated by `setup_commit` is not queued to a file (inferred). An atomic commit sends an
  event only if it requests one.
- **`vc4_atomic_commit_setup` clears `legacy_cursor_update`** for non-firmware KMS
  (`vc4_kms.c:554-560`, read from source). It is reached only from `setup_commit`, which an async
  commit never calls (§1 item 4), so it affects only the synchronous fallback: there
  `flip_done` is not auto-completed (inferred). In rpi `drm_atomic_helper.c` that auto-completion
  code is absent anyway (§1 item 6), so the comment there is stale (inferred).
- vc4 also sets `mode_config.async_page_flip = true` (`vc4_kms.c:1215`, read from source) and its
  `atomic_async_check` accepts `flip == true`; that concerns `PAGE_FLIP_ASYNC` only.

**Expected cursor-move outcome, vc4 (HVS5 and HVS6).** (inferred from the reads above)

| how the move is sent | result |
|---|---|
| legacy `MODE_CURSOR` MOVE, cursor already shown, same size/format | 0, applied in place, no event |
| legacy `MODE_CURSOR`, cursor plane's previous commit not at `hw_done` | 0 after a blocking sync commit, no event |
| atomic ioctl, `NONBLOCK`, flip on the same CRTC not yet latched | `-EBUSY` |
| atomic ioctl, `NONBLOCK`, previous commit done | 0; event only if requested |

## 3. Rockchip VOP2 (RK3588), mainline `v6.18`

- **Async update exists: no.** `vop2_plane_helper_funcs` has only `atomic_check`, `atomic_update`
  and `atomic_disable` (read from source: v6.18 `rockchip_drm_vop2.c:1404-1408`); no
  `atomic_async_check` or `atomic_async_update` appears anywhere in the file (grep, no match). The
  generic check therefore fails with `-EINVAL` ("driver does not support async updates",
  `drm_atomic_helper.c:2064-2069`) and `async_update` stays false (read from source).
- **Which planes qualify: none.** Also, VOP2 has no cursor plane: its plane types are PRIMARY and
  OVERLAY only (`rockchip_drm_vop2.c:2389-2428`) and each CRTC is created with `cursor = NULL`
  (`drm_crtc_init_with_planes(drm, &vp->crtc, plane, NULL, ...)`, `:2457`); `vop2_crtc_funcs`
  (`:2144`) has no `cursor_set` or `cursor_move` (the file has no "cursor" string) (read from
  source). So `crtc->cursor` is NULL and the legacy cursor ioctl never reaches the helper
  (§1 item 2).
- **What the legacy cursor ioctl returns:** `-ENXIO` for a request with `DRM_MODE_CURSOR_BO`, and
  `-EFAULT` for a MOVE-only request (§1 item 2; inferred for the combination with this driver's
  tables). No flip event, nothing committed.
- **Atomic ioctl on an overlay plane used as a cursor.** Ordinary commit through
  `drm_atomic_helper_commit` (`rockchip_drm_fb.c:63-66`, `.atomic_commit = drm_atomic_helper_commit`,
  read from source), so `setup_commit` and `stall_checks` apply: **`-EBUSY`** with `NONBLOCK` while
  the CRTC's previous commit has not completed `flip_done` (generic, §1 item 7); 0 otherwise. A
  flip event is sent only on request, and `flip_done` completes with the VOP2 vblank/flip event as
  described in [`rk3588-vop2-flip-source.md`](rk3588-vop2-flip-source.md) (inferred).
- `mode_config.async_page_flip` is not set anywhere in the Rockchip files read, so
  `PAGE_FLIP_ASYNC` is refused with `-EINVAL` (`drm_atomic_uapi.c:1424-1428`; read from source,
  grep of `async_page_flip` over the fetched files finds only the core files).

**Expected cursor-move outcome, VOP2.** (inferred from the reads above)

| how the move is sent | result |
|---|---|
| legacy `MODE_CURSOR` (BO or MOVE) | `-ENXIO` / `-EFAULT`, nothing committed |
| atomic ioctl, overlay plane, `NONBLOCK`, flip on the same video port not yet latched | `-EBUSY` |
| atomic ioctl, `NONBLOCK`, previous commit done | 0; event only if requested |

## 4. What this means for `pending_probe`'s cursor-move case

(inferred)

- The probe sends through the atomic ioctl. On all three drivers a cursor-move commit sent before
  the flip is a normal commit and is expected to report **`-EBUSY`** when sent `NONBLOCK`, the same
  as a second ordinary overlay commit. The async path is **not** exercised by an atomic-ioctl
  cursor commit on any of them, so the probe's cursor case cannot show an "accepted while pending"
  result unless it uses the **legacy** `DRM_IOCTL_MODE_CURSOR`.
- A legacy-ioctl variant is the only way to see the async path: it is expected to return 0 at once
  with no event on vc4 (cursor already shown, same size), and `-ENXIO`/`-EFAULT` on VOP2. If the
  probe's cursor case is changed to use it, the expected values are those in the two tables above.
- A vc4 result of 0 for the atomic cursor commit while the flip is pending would contradict §1
  item 7 and would point at a different kernel than the one read here.
