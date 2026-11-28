# RK3588 VOP2 flip, vblank event and pending-commit semantics, read from mainline 6.18

Written 2026-11-28 for plan #467, item `rk-flip-source-b` (RK Q9 of
`docs/rk3588-backend-design.md` section 7). Host-only: no board was used, no
code was run, and nothing here is a hardware claim. Everything below is what
the source says; where the source does not settle a point it says so.

## Pinned source

- Tag `v6.18`, commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` of
  torvalds/linux
  (https://github.com/torvalds/linux/tree/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/gpu/drm/rockchip).
  Fetched with `git fetch --depth 1 origin tag v6.18` into a sparse checkout of
  `drivers/gpu/drm/rockchip` and `include/drm` (the server ignored the blob
  filter, so the fetch was depth-1 but not blobless; still no full clone).
- Because only those two directories were checked out, the generic atomic
  helper code (`drivers/gpu/drm/drm_atomic_helper.c`) was **not** read. Claims
  about it are tagged `inferred`, with the header text that supports them.
- The NanoPC-T6 kernel recorded in `docs/observed/rk3588-*.md` is
  `6.18.55-current-rockchip64` (Armbian). That is a 6.18 stable tree, not the
  exact v6.18 tag; a difference in these functions between v6.18 and 6.18.55
  was not checked (inferred: stable releases rarely touch this path).
- Vendor `rockchip-linux/kernel` was not read for this item. See the last
  section.

## 1. What an overlay-plane nonblocking commit does in the driver

The mode config uses the stock helper tail:
`rockchip_mode_config_helpers.atomic_commit_tail = drm_atomic_helper_commit_tail_rpm`
(read from source (rockchip_drm_fb.c:rockchip_mode_config_helpers, v6.18)).
`mode_config_funcs.atomic_commit = drm_atomic_helper_commit` (read from source
(rockchip_drm_fb.c:rockchip_drm_mode_config_funcs, v6.18)). There is no
driver-specific commit queue, work item or lock around commits.

For a commit that only changes an overlay plane on an active CRTC:

1. **Plane update.** `vop2_plane_atomic_update` programs the window registers
   from the new plane state (read from source
   (rockchip_drm_vop2.c:vop2_plane_atomic_update, v6.18)). It does not start
   the flip by itself.
2. **CRTC begin.** `vop2_crtc_atomic_begin` calls `vop2->ops->setup_overlay(vp)`,
   which recomputes the layer/zpos selection (read from source
   (rockchip_drm_vop2.c:vop2_crtc_atomic_begin, v6.18)).
3. **CRTC flush.** `vop2_crtc_atomic_flush` does, in this order (read from
   source (rockchip_drm_vop2.c:vop2_crtc_atomic_flush, v6.18)):
   - gamma LUT update only if colour management changed and no modeset,
   - `vop2_post_config(crtc)`,
   - `vop2_cfg_done(vp)`,
   - under `event_lock`: if `crtc->state->event` is set, take a vblank
     reference with `drm_crtc_vblank_get`, store the event in `vp->event`, and
     clear `crtc->state->event`.
4. **Event delivery.** The event is not sent from the commit path. It is sent
   from the interrupt handler (next section).

Ordering note: the event is stored in `vp->event` *after* `vop2_cfg_done`. The
handler's test (below) reads the config-done register under the same
`event_lock`, so a frame-start interrupt that fires between the two steps
finds `vp->event == NULL` and does nothing; the event waits for the next
frame-start interrupt (inferred from the lock and ordering; the source does not
comment on it).

## 2. When the vblank / flip event is sent

Per video port, the interrupt handler on a `VP_INT_FS_FIELD` (frame-start /
field) interrupt does (read from source
(rockchip_drm_vop2.c:vop2_isr, and rk3576_vp_isr for RK3576, v6.18)):

1. `drm_crtc_handle_vblank(crtc)`: the vblank counter advances on every
   frame-start interrupt, whether or not a commit is pending.
2. Under `event_lock`, only if `vp->event` is set: read `RK3568_REG_CFG_DONE`
   and test `BIT(vp->id)`. If the bit is **clear**,
   `drm_crtc_send_vblank_event(crtc, vp->event)`, `vp->event = NULL`,
   `drm_crtc_vblank_put(crtc)`. If the bit is still **set**, the event stays
   pending and nothing is sent this frame.

So the flip event is sent on the first frame-start interrupt at which the
hardware has cleared this port's config-done bit. The `VP_INT_FS_FIELD`
interrupt is enabled by `vop2_crtc_enable_vblank` (read from source
(rockchip_drm_vop2.c:vop2_crtc_enable_vblank, v6.18)); the commit's
`drm_crtc_vblank_get` keeps it enabled until the event is sent.

The event timestamp and sequence come from `drm_crtc_send_vblank_event`, i.e.
from the vblank counter/timestamp of that frame-start interrupt (inferred from
the generic helper's behaviour; `drm_vblank.c` was not read).

The disabled-CRTC case differs: `vop2_crtc_atomic_disable` sends
`crtc->state->event` immediately when the CRTC is not active (read from source
(rockchip_drm_vop2.c:vop2_crtc_atomic_disable, v6.18, the `event && !active`
branch near line 983)). That does not apply to an overlay commit on an active
CRTC.

## 3. What "latched" means: the config-done bit

`vop2_cfg_done` sets three things in `RK3568_REG_CFG_DONE` with one
`regmap_set_bits`: `GLB_CFG_DONE_EN` (bit 15), and `BIT(vp->id)` together with
its write-mask bit `BIT(vp->id) << 16` (read from source
(rockchip_drm_vop2.h:vop2_cfg_done, v6.18)). The header comment states that the
write mask is needed from RK3588 on, and that `GLB_CFG_DONE_EN` has no mask bit.

Meaning in the source's own terms:

- Window, overlay and port registers are written through the register map
  first; writing the per-port config-done bit hands that set to the hardware
  (read from source: the bit is written only after all register writes in
  `vop2_crtc_atomic_flush`).
- The ISR treats "bit still set" as "hardware has not yet taken the new
  configuration" and "bit clear" as "taken" (read from source
  (rockchip_drm_vop2.c:vop2_isr, v6.18)). The driver never clears the bit
  itself; only the hardware does (inferred from the absence of any clearing
  write in the driver; the register manual was not read).
- When exactly the hardware clears it (at the next frame start, or at vblank
  only) is a hardware fact the source does not state. Inferred: it is cleared
  around the next frame boundary, since the driver tests it in the frame-start
  interrupt. This needs a board run (item `t6-commit-session`) to confirm.

For `hwlayer-manager`: "Latched" for VOP2 can therefore be mapped to "the flip
event for this commit was delivered", which is the first frame-start interrupt
after the bit cleared. There is no earlier, driver-visible latch notification
(inferred: no other event or completion is raised on bit clear).

## 4. A second nonblocking commit before the first one's flip event

The answer comes from the generic helper, not from VOP2. The driver contains
no handling of it: no `-EBUSY` return, no pending-commit check and no
replacement logic anywhere in `drivers/gpu/drm/rockchip/rockchip_drm_vop2.c`
(read from source (rockchip_drm_vop2.c, whole file searched for `EBUSY`,
`nonblock`, `commit`, v6.18: no such handling; only `event` handling in the
functions cited above)).

What `include/drm/drm_atomic.h` documents (read from source
(include/drm/drm_atomic.h:struct drm_crtc_commit, v6.18)): `flip_done` "will be
signaled when the hardware has flipped to the new set of buffers", at the same
time as the DRM event is sent, via `drm_crtc_send_vblank_event()`; `hw_done` is
signalled once all register changes are written. The header points at
`drm_atomic_helper_setup_commit()` as the implementation.

The behaviour of that function, as the kernel's uAPI describes it: a
nonblocking commit that touches a CRTC, plane or connector whose previous
commit has not yet signalled `flip_done` is **rejected with `-EBUSY`** rather
than queued or replaced (inferred: from the helper's documented contract and
my recollection of `drm_atomic_helper_setup_commit()`; the function lives in
`drivers/gpu/drm/drm_atomic_helper.c`, outside the checked-out directories, and
was not read). The helper table entry `drm_atomic_helper_page_flip` /
`.page_flip` (read from source (rockchip_drm_vop2.c:vop2_crtc_funcs, v6.18))
and the `drm_crtc.h` text "if a page flip operation is already pending the
callback should return -EBUSY" (read from source
(include/drm/drm_crtc.h:struct drm_crtc_funcs.page_flip, v6.18)) are consistent
with that, but concern the legacy page-flip path.

Summary of what a second commit gets:

| Question | Answer | Tag |
| --- | --- | --- |
| Does VOP2 queue or replace the first commit in the driver? | No driver code does either. | read from source (rockchip_drm_vop2.c, v6.18) |
| Is a second nonblocking commit on the same CRTC before the first flip event rejected? | Expected `-EBUSY` from `drm_atomic_helper_commit` / `setup_commit`. Not settled by the files read. | inferred |
| Is it queued or replaced instead? | Expected no. Not settled by the files read. | inferred |
| Does it matter that only an overlay plane changed? | Inferred no: the helper tracks per-CRTC, per-plane and per-connector commits, and the commit holds the CRTC's state because the plane state change pulls the CRTC into the commit. | inferred |

Stated plainly: **the VOP2 source read here does not settle the
second-commit answer; the generic atomic helper does, and it was not part of
the checkout.** The `-EBUSY` answer is inferred. It should be confirmed either
by reading `drm_atomic_helper_setup_commit()` at the same pin (adding
`drivers/gpu/drm/drm_atomic_helper.c` to the sparse checkout) or, better, by
the board run with `pending_probe` (items `pending-probe-tool` and
`t6-commit-session`).

One more driver-side consequence, read from source: `vop2_crtc_atomic_flush`
overwrites `vp->event` unconditionally when `crtc->state->event` is set
(rockchip_drm_vop2.c:vop2_crtc_atomic_flush, v6.18). If two commits with events
ever reached flush with the first event still stored, the first event would be
lost and its vblank reference leaked. The helper's `-EBUSY` (inferred above) is
what prevents that; the driver does not guard it itself.

A blocking (non-nonblock) commit would instead wait for the earlier commit
(inferred from the helper design in `drm_atomic.h`: "wait for flip_done" in the
commit thread diagram, read from source
(include/drm/drm_atomic.h:struct drm_crtc_commit comment, v6.18)).

## 5. Vendor rockchip kernel

Not read. `docs/rk3588-vendor-vop2-source.md` pins `rockchip-linux/kernel`
`develop-6.1` at `77168c8d5ab82399f65a80e9f807b50ba37cf483` but does not cover
flip semantics, and the vendor driver (`rockchip_drm_vop2.c` there) has its own
commit and event path that this item did not fetch. Status: open. The T6 runs a
mainline-derived 6.18 kernel, so the vendor answer does not bear on that board.

## 6. Answers for RK Q9 and what remains for the board

| RK Q9 point | Source answer | Left for the board |
| --- | --- | --- |
| Flip event sent when? | On the first `VP_INT_FS_FIELD` interrupt where the port's config-done bit is clear. read from source (rockchip_drm_vop2.c:vop2_isr, v6.18) | Count flip events and their sequence numbers against commits. |
| "Latched" | Config-done bit cleared by hardware; the driver reports it only through the flip event. read from source (rockchip_drm_vop2.h:vop2_cfg_done, v6.18); clearing time inferred | Whether the event arrives one frame after the commit. |
| Second commit before the event | `-EBUSY` expected from the generic helper; not settled by the VOP2 files. inferred | Run `pending_probe`: record the errno of the second commit. |
| Overlay vs primary plane | No difference in the flush path; the event is per CRTC. read from source (rockchip_drm_vop2.c:vop2_crtc_atomic_flush, v6.18) | Nothing. |

No hardware claim is made by this document.
