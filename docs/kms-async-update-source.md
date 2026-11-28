# Atomic async (cursor) update path in vc4 HVS5/HVS6 and RK3588 VOP2, from source

Written 2026-11-28 for plan #467, item `kms-async-update-source` (HVS Q6 of
`docs/hvs-backend-design.md` section 7, RK Q9 of `docs/rk3588-backend-design.md` section 7).
Host-only: no board was used and no code was run. **Nothing here is a measured claim.** It is the
prediction that the cursor-move case of `pending_probe` must confirm or refute.

Related, not edited here: [`vc4-replace-pending-source.md`](vc4-replace-pending-source.md) (HVS Q6,
second atomic commit on vc4), [`drm-pending-commit-source.md`](drm-pending-commit-source.md)
(generic helper), [`rk3588-vop2-flip-source.md`](rk3588-vop2-flip-source.md) (VOP2 flip, RK Q9).
There is no file named `vc4-pending-source.md`; the vc4 one is `vc4-replace-pending-source.md`.

## Tags and pins

- **read from source**: read in a depth-1 git fetch of these trees on 2026-11-28 (UTC host):
  - `rpi`: `raspberrypi/linux`, branch `rpi-6.18.y`, commit
    `16ae2649dcfadf9d81445b1177a3191efa3cb82e` (Makefile says 6.18.65). Line numbers are exact.
    https://github.com/raspberrypi/linux/tree/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm
  - `mainline`: `torvalds/linux`, tag `v6.18`, commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449`.
    Line numbers are exact.
    https://github.com/torvalds/linux/tree/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/gpu/drm
  - Paths are relative to `drivers/gpu/drm/`.
- **inferred**: derived from the reads above, or from a tree not read. Called out per claim.
- The boards' running kernels are not compared here (`docs/vc4-rk-kernel-compare.md` does that).
  Armbian's 6.18 patch set for the NanoPC-T6 was not read, so a patch that adds an async path to
  VOP2 there is not excluded (inferred).

## 1. The generic routing (both drivers)

1. Two entry points set `state->legacy_cursor_update = true`, both only when the plane is the
   CRTC's cursor plane: `drm_atomic_helper_update_plane` (the legacy cursor ioctl, via
   `drm_mode_cursor_universal`) and `drm_atomic_helper_disable_plane`.
   (read from source: rpi `drm_atomic_helper.c:3413-3414`, `3451-3452`; mainline lines `3346`,
   `3384`). The legacy cursor ioctl only goes there when `crtc->cursor` exists
   (read from source: rpi `drm_plane.c:1305-1317`). With no cursor plane it falls to the
   `cursor_set` hooks, or returns `-ENXIO` when the CRTC has none
   (read from source: rpi `drm_plane.c:1319-1323`).
2. The atomic ioctl never sets `legacy_cursor_update` (read from source: the only writers in the
   rpi tree are the two lines above and `vc4_kms.c:560`, which writes `false`; `drm_atomic_uapi.c`
   has no write). So **a cursor move sent through `DRM_IOCTL_MODE_ATOMIC` is not an async update**
   for either driver; it is a normal commit (section 4).
3. `drm_atomic_helper_check` ends with
   `if (state->legacy_cursor_update) state->async_update = !drm_atomic_helper_async_check(...)`
   (read from source: rpi `drm_atomic_helper.c:1130-1131`; mainline `1125-1126`). The result of
   the async check is not an error to the caller: any non-zero return only means
   `async_update = false` and the commit proceeds as a normal one.
4. `drm_atomic_helper_async_check` returns non-zero (read from source: rpi
   `drm_atomic_helper.c:2114-2170`; the same text in mainline around `2060-2100`):
   - `-EINVAL` if any CRTC in the state needs a modeset;
   - `-EINVAL` unless exactly one plane is in the state;
   - `-EINVAL` if the plane has no CRTC or the CRTC changes;
   - `-EINVAL` if the plane's helper has no `atomic_async_update`;
   - `-EINVAL` if the new plane state has an in-fence;
   - `-EBUSY` if the plane's previous commit (`old_plane_state->commit`) has not reached
     `hw_done` (`:2157-2163`);
   - otherwise the driver's `atomic_async_check(plane, state, false)` result.
5. `drm_atomic_helper_commit` with `state->async_update` set prepares the planes, calls
   `atomic_async_update` in place, unprepares, and returns 0. It does **not** call
   `drm_atomic_helper_setup_commit`, so no `drm_crtc_commit`, no `flip_done`, no vblank event and
   no `hw_done`/`cleanup_done` is created for it
   (read from source: rpi `drm_atomic_helper.c:2246-2255`; mainline `2172`).
6. The non-async path, for a second commit while the first is pending: `stall_checks` returns
   `-EBUSY` when the CRTC's newest commit has not completed `flip_done` and the new commit is
   nonblocking (read from source: rpi `drm_atomic_helper.c:2380-2393`; mainline `2312-2318`).
   A blocking commit does not return `-EBUSY` there; it waits (read from source: rpi
   `drm_atomic_helper.c:2403-2415` for the cleanup wait, and `drm_atomic_helper_commit` `:2267`
   for the fence wait only on the blocking path). The legacy cursor ioctl commits blocking
   (inferred: `drm_atomic_helper_update_plane` calls `drm_atomic_commit`, whose `nonblock` is
   false; not traced to the `commit` call in `drm_atomic.c`).
7. Difference between the two generic trees: mainline v6.18 has `if (state->legacy_cursor_update)
   { complete_all(&commit->flip_done); continue; }` in `setup_commit` (read from source: mainline
   `drm_atomic_helper.c:2470-2474`) and returns early from the vblank wait for such states
   (`:1814-1819`). The rpi 6.18.65 tree has neither (read from source: no
   `legacy_cursor_update` match in rpi `drm_atomic_helper.c` apart from the lines in item 1 and
   `1130`). Why rpi differs is not known (inferred: a downstream change; its history was not read).
   It matters only for a legacy cursor update that falls back to the non-async path.

## 2. vc4 (HVS5 on BCM2711, HVS6 on BCM2712), rpi-6.18.y 16ae2649

**Does async update exist: yes.** Both plane helper tables, `vc4_plane_helper_funcs` and
`vc5_plane_helper_funcs`, set `atomic_async_check = vc4_plane_atomic_async_check` and
`atomic_async_update = vc4_plane_atomic_async_update`
(read from source: `vc4/vc4_plane.c:2745-2759`; the check at `:2667`, the update at `:2593`).
HVS5 and HVS6 differ only in that the check calls `vc4_plane_mode_set` (gen <= 5) or
`vc6_plane_mode_set` (gen 6) (read from source: `vc4/vc4_plane.c:2677-2680`).
The firmware-KMS driver (`vc4_firmware_kms.c`) has its own pair; it is a different driver and not
the one the boards use (inferred: the boards were recorded with the full KMS driver; see
`docs/observed/` for the driver name).

**Which planes qualify.** Any vc4 plane type: the helper tables are shared by primary, overlay and
cursor planes (read from source: `vc4/vc4_plane.c:2849-2920` creates all three through
`vc4_plane_init`). In practice the legacy-cursor route needs the CRTC's cursor plane, which vc4
creates per CRTC (read from source: `vc4/vc4_plane.c:2986-3000`). Conditions, beyond section 1:
- `vc4_plane_mode_set` must succeed and yield a `hw_dlist` (`-EINVAL` otherwise) (`:2681-2688`).
- The new dlist must have the same length and the same `pos0`, `pos2`, `ptr0` and LBM-size layout
  (`:2690-2695`).
- Only the `pos0`, `pos2`, `ptr0` words (and the LBM word) may differ; any other changed dlist
  word returns `-EINVAL` (`:2697-2710`). A pure cursor move changes `pos0`; a new framebuffer
  changes `ptr0`; a size, format, alpha or scaling change changes other words, so it falls back.
  (Which words each property touches is inferred from the field names; the dlist builder was not
  read here.)
- `vc4_atomic_commit_setup` clears `legacy_cursor_update` for the non-firmware driver
  (read from source: `vc4/vc4_kms.c:554-560`). That runs inside `setup_commit`, after
  `drm_atomic_helper_check` already decided `async_update`, and an async commit never reaches
  `setup_commit`; so it only affects a fallback commit (inferred from the call order in
  `drm_atomic_helper.c:1130` before `:2257`).
- `mode_config.async_page_flip = true` (read from source: `vc4/vc4_kms.c:1215`). That is the
  separate `DRM_MODE_PAGE_FLIP_ASYNC` route: the legacy page-flip ioctl goes to
  `vc5_async_page_flip` (gen > 4) (`vc4/vc4_crtc.c:1114-1127`), and an atomic ioctl with that flag
  asks a non-primary plane's `atomic_async_check(..., true)` for any property other than a pure
  flip (read from source: `drm_atomic_uapi.c:1113-1137`). It does not set `async_update` in
  `drm_atomic_helper_commit` (inferred: no writer, section 1 item 2), so it is not the cursor path
  and not covered further.

**What the second commit returns, per way it is sent:**

| Second commit | Predicted result on vc4 | Tag |
|---|---|---|
| Legacy cursor ioctl move, cursor plane has no unfinished earlier commit, only `pos0` changes | async: returns 0 at once; no event, no `flip_done`; the dlist words are written in place | read from source (section 1 items 3-5; `vc4_plane.c:2593-2665`, `2667-2712`) |
| Same, but the cursor plane's own previous commit has not reached `hw_done` | async check gives `-EBUSY`, which is swallowed; the move becomes a blocking commit and returns 0 after it waited; no `-EBUSY` to the caller | read from source (`drm_atomic_helper.c:2157-2163`, `1131`); blocking return inferred |
| Legacy cursor ioctl, a non-position property or a new fb size changes | same fallback: blocking commit, returns 0 | read from source (`vc4_plane.c:2697-2710`); return inferred |
| Atomic ioctl, nonblocking, cursor plane, flip of another plane on the same CRTC still pending | not async; `stall_checks` returns `-EBUSY` | read from source (`drm_atomic_helper.c:2380-2393`); see also `vc4-replace-pending-source.md` |
| Atomic ioctl, blocking, same | waits for `flip_done` of the previous commit, then returns 0 | inferred from `stall_checks` (no `-EBUSY` when `nonblock` is false) |

**Is a flip event sent for the async update: no.** No `drm_crtc_commit` is made, so nothing
completes `flip_done` and no `DRM_EVENT_FLIP_COMPLETE` is queued for it
(read from source: `drm_atomic_helper.c:2246-2255`). A fallback commit does allocate an internal
event, but a legacy cursor call asked for none (inferred: the legacy path passes no
`DRM_MODE_PAGE_FLIP_EVENT`); whether the internal event is delivered to the file was not traced.

## 3. RK3588 VOP2, mainline v6.18 7d0a66e4

**Does async update exist: no.** `vop2_plane_helper_funcs` has only `atomic_check`,
`atomic_update` and `atomic_disable` (read from source:
`rockchip/rockchip_drm_vop2.c:1404-1408`). There is no `atomic_async_check` or
`atomic_async_update` anywhere in `rockchip_drm_vop2.c` or `rockchip_drm_fb.c` (read from source:
a case-insensitive search for `async` in those two files finds nothing), and
`mode_config.async_page_flip` is never set in the `rockchip/` directory (read from source: no
match for `async_page_flip` there; the only hit in the tree search is in `drm_atomic_uapi.c`).
The older VOP1 driver `rockchip_drm_vop.c` does have an async pair
(read from source: `rockchip/rockchip_drm_vop.c:1073-1152`), but the RK3588 uses VOP2, so it is
not relevant (inferred: `docs/rk3588-vop2-flip-source.md` pins the VOP2 driver).

**Which planes qualify: none.** `drm_atomic_helper_async_check` fails with `-EINVAL` at the
missing `atomic_async_update` (read from source: mainline `drm_atomic_helper.c`, the same check
as rpi `:2138-2143`). Also, VOP2 creates no cursor plane: its window types are
`DRM_PLANE_TYPE_PRIMARY` or `DRM_PLANE_TYPE_OVERLAY` only (read from source:
`rockchip/rockchip_vop2_reg.c:608-650`, `864-946`, `1139-1259`; `rockchip_drm_vop2.c:2422-2428`).
`rockchip_drm_vop2.c` has no `DRM_PLANE_TYPE_CURSOR` and no `crtc->cursor` assignment
(read from source: no match in that file). Hence the legacy cursor ioctl has no cursor plane to
route to and, without `cursor_set` hooks, returns `-ENXIO`
(inferred: the VOP2 CRTC funcs were not searched for `cursor_set`; the rpi
`drm_plane.c:1319-1323` text is taken to equal mainline).

**What the second commit returns:** a "cursor move" on this board has to be an atomic commit on an
overlay plane. The planes use `drm_atomic_helper_update_plane` as `update_plane`
(read from source: `rockchip_drm_vop2.c:1410-1412`); that helper only sets `legacy_cursor_update`
for a CRTC's cursor plane, and there is none, so it is a normal commit. The result follows
`stall_checks`:

| Second commit | Predicted result on VOP2 | Tag |
|---|---|---|
| Atomic ioctl, nonblocking, first commit's `flip_done` not yet complete | `-EBUSY`, no event queued | read from source (`drm_atomic_helper.c` mainline `2312-2318`); the `drm_atomic_helper_commit`/`atomic_commit` hooks are the stock ones (`rockchip_drm_fb.c:28`, `64-65`) |
| Atomic ioctl, blocking | waits, then 0 | inferred |
| Any legacy cursor ioctl | no cursor plane: `-ENXIO` or the `cursor_set` result | inferred (see above) |
| Atomic `DRM_MODE_PAGE_FLIP_ASYNC` | `-EINVAL`: `async_page_flip` is not set | read from source (`drm_atomic_uapi.c:1425` mainline, same check at rpi `:1467-1471`) |

**Is a flip event sent: yes for the commit that is accepted** (the normal path allocates and
sends an event on `flip_done`, see `rk3588-vop2-flip-source.md`); a commit refused with `-EBUSY`
returns before its commit object has any event queued (inferred from `setup_commit` returning at
`stall_checks`, rpi `:2530-2532`).

## 4. Expected cursor-move outcome per driver (what `pending_probe` should see)

Tags: P = prediction from the reading above, not a measurement.

| Driver | Cursor-move route in the probe | Second commit returns | Flip event for the second commit | Tag |
|---|---|---|---|---|
| vc4 HVS5 (BCM2711) | legacy cursor ioctl on the CRTC's cursor plane, `pos0` change only | 0 | none (async, no `flip_done`) | read from source; P |
| vc4 HVS6 (BCM2712) | same | 0 | none | read from source; P |
| vc4, atomic ioctl, nonblocking, first flip pending | atomic commit moving the cursor plane | `-EBUSY` | none queued | read from source; P |
| RK3588 VOP2 | atomic commit moving an overlay plane (no cursor plane exists) | `-EBUSY` while the first flip is pending | none queued | read from source; P |
| RK3588 VOP2 | legacy cursor ioctl | `-ENXIO` (no cursor plane) | none | inferred; P |

A measurement that disagrees means one of these is wrong: the board's kernel differs from the two
pinned trees, a downstream patch exists, or a helper path was read wrongly. The test that
separates the vc4 async case from the fallback: with the cursor plane untouched by the pending
flip, the legacy move returns at once and no event arrives for it; if its previous commit is
unfinished it blocks until the next vblank instead (blocking return inferred).

## 5. What was not read

- `drm_atomic.c` (`drm_atomic_commit`, which sets the `nonblock` argument for the legacy path), the
  vc4 dlist builders for the exact words each property changes, and the VOP2 CRTC funcs for
  cursor hooks. Statements that need them are marked inferred above.
- Armbian's patches on top of v6.18 and the vendor `rockchip-linux` kernel.
- Whether the HVS hardware latches an in-place dlist write at once or at the next scanline: the
  code only writes `hw_dlist` (`vc4_plane.c:2657-2662`); the hardware timing is not a source claim.
