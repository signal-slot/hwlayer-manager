# vc4 fence path on BCM2711 and BCM2712, read from source

Scope: how the vc4 DRM driver treats `IN_FENCE_FD`, implicit sync, the flip
event and `OUT_FENCE_PTR`, for the design in `docs/sync-fences.md`. Host
reading only. Nothing here was run on a board, so it makes no M1 to M3 claim.

## Pinned refs

Read from source (2026-11-28), whole files `vc4_kms.c`, `vc4_plane.c`,
`vc4_crtc.c` under `drivers/gpu/drm/vc4/`. These are the two refs already used
by `docs/vc4-mainline-source.md`.

| Tree | Commit |
| --- | --- |
| raspberrypi/linux rpi-6.18.y | 16ae2649dcfadf9d81445b1177a3191efa3cb82e |
| mainline Linux v6.18 | 7d0a66e4bb9081d75c82ec4957c50034cb0ea449 |

URL form: `https://raw.githubusercontent.com/raspberrypi/linux/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4/<file>`
and `https://raw.githubusercontent.com/torvalds/linux/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/gpu/drm/vc4/<file>`.

Line numbers below are rpi-6.18.y 16ae2649dcfa. Mainline v6.18 has the same
functions at other lines and the same behaviour in every point below.

Caveat on the kernel: the recorded kernels are not the pinned tree. The
BCM2711 topology was recorded on `6.12.47+rpt-rpi-v8` and the BCM2712 one on
`6.18.50+rpt-rpi-2712`. The pin is 6.18, so statements about BCM2711 hold for
the running 6.12 kernel only as inferred.

Not fetched: the DRM core (`drm_atomic_helper.c`, `drm_gem_atomic_helper.c`,
`drm_atomic_uapi.c`). What this note says about them is inferred from the
helper names vc4 calls, tagged below.

## Recorded properties

Observed 2026-11-27 (recorded topology) in
`crates/composition-drm/tests/data/bcm2711/props-2026-11-27.txt`: kernel line
`# kernel: 6.12.47+rpt-rpi-v8`, board line `BCM2711, vc4 driver, HVS5`. Six
CRTCs each list `OUT_FENCE_PTR (id 19): range 0..=18446744073709551615 [atomic] = 0`,
and every plane lists
`IN_FENCE_FD (id 18): signed range -1..=2147483647 [atomic] = 18446744073709551615`.

Observed 2026-11-27 (recorded topology) in
`crates/composition-drm/tests/data/bcm2712/props-2026-11-27.txt`: kernel line
`# kernel: 6.18.50+rpt-rpi-2712`, `Raspberry Pi 5 Model B Rev 1.1 (BCM2712, vc4 driver, HVS6)`.
It carries the same `OUT_FENCE_PTR (id 19)` on its CRTCs and the same
`IN_FENCE_FD (id 18)` on its planes.

These properties are created by the DRM core for any atomic driver. Their
presence says nothing about whether vc4 honours them (inferred).

## Generation mapping

Read from source (2026-11-28): `docs/vc4-mainline-source.md` line 47 maps
BCM2711 to `VC4_GEN_5` and BCM2712 to `VC4_GEN_6_C` / `VC4_GEN_6_D`. The
driver selects plane helpers on that value, see question 2.

## 1. Atomic helper fence wait or its own?

Read from source (2026-11-28): neither `drm_atomic_helper_wait_for_fences` nor
a seqno wait appears in the atomic path. `vc4_kms.c:vc4_atomic_commit_tail`
(rpi 16ae2649dcfa, line 422) is a custom commit tail. Its body calls, in order,
`drm_atomic_helper_commit_modeset_disables`,
`drm_atomic_helper_commit_planes(..., DRM_PLANE_COMMIT_ACTIVE_ONLY)`,
`drm_atomic_helper_commit_modeset_enables`, `drm_atomic_helper_fake_vblank`,
`drm_atomic_helper_commit_hw_done`, `drm_atomic_helper_wait_for_flip_done`
and `drm_atomic_helper_cleanup_planes` (lines 493 to 530). The string
`wait_for_fences` occurs zero times in `vc4_kms.c` in both pinned trees.
`vc4_mode_config_helpers` (line 1158) installs this function as
`.atomic_commit_tail`, and `vc4_mode_funcs` / `vc5_mode_funcs` use plain
`drm_atomic_helper_commit` (lines 1165, 1171).

The only wait on earlier work in the tail is
`vc4_atomic_commit_tail` waiting on `drm_crtc_commit_wait(pending_commit)` per
HVS FIFO (line 467): it orders commits on the same channel and is not a
dma-fence wait.

Inferred: `drm_atomic_helper_wait_for_fences` is called from the default
`drm_atomic_helper_commit_tail` and `..._commit_tail_rpm`, not from
`drm_atomic_helper_commit`. A driver that replaces the tail and does not call
it therefore never waits for `plane_state->fence`. So on vc4 an `IN_FENCE_FD`
is accepted and stored, but the commit tail does not wait for it. The historic
vc4 seqno wait (`vc4_wait_for_seqno`) is not in the three files read here.
Not read: other vc4 files and the core, so "no wait anywhere" needs a
vkms or board check before it is relied on.

Consequence for the engine design, inferred: on vc4 the backend must not treat
a successful commit with `IN_FENCE_FD` as proof that the buffer was ready at
scanout. It has to wait on the fence itself in userspace before the commit, or
mark vc4 as "in-fence not honoured". `docs/sync-fences.md` §3.2 should say
which.

## 2. Does `prepare_fb` do implicit sync?

Read from source (2026-11-28): depends on generation.

- `vc4_plane.c:vc4_prepare_fb` (line 2715) calls
  `drm_gem_plane_helper_prepare_fb(plane, state)` then `vc4_bo_inc_usecnt`.
  It is wired as `.prepare_fb` only in `vc4_plane_helper_funcs` (line 2745),
  which is used when `vc4->gen < VC4_GEN_5` (`vc4_plane_init`, line 2848, helper choice at line 2887).
- `vc5_plane_helper_funcs` (line 2754) has no `.prepare_fb` and no
  `.cleanup_fb`. It is used for `gen >= VC4_GEN_5`, so both BCM2711 and
  BCM2712. Same in mainline v6.18 (`vc4_plane.c` lines 2418 and 2427).

So on the two Raspberry Pi boards of this repository the driver installs no
`prepare_fb` and, read from the file, no implicit-sync fence is attached to
the plane state by vc4 code.

Inferred: `drm_gem_plane_helper_prepare_fb` copies the GEM object's
`dma_resv` write fence into `state->fence` unless userspace already set an
explicit one, and the core also needs a `prepare_fb` for that. Without it,
neither explicit nor implicit fences are collected. `drm_gem_fb_create` is the
BCM2711/BCM2712 `fb_create` (`vc5_mode_funcs`, line 1172 of `vc4_kms.c`), which does not
change this.

## 3. Does an in-fence delay the flip event?

Read from source (2026-11-28), for the atomic path: the flip event is sent
from `vc4_crtc.c:vc4_crtc_handle_page_flip` (line 844), when the HVS display
list in use (`current_dlist`) equals the one this commit wrote, and from
`vc4_crtc_send_vblank` (line 625) for the disabled-CRTC case. Neither looks at
a fence.

Inferred from question 1: since the tail never waits on the in-fence, the
commit writes its display list on time, the event comes at the next vblank,
and an unsignalled in-fence does not delay it. The fence is simply not
honoured, so the event can report a flip that scanned out an unready buffer.
This is the opposite of what SoW §7 asks and what `docs/sync-fences.md`
assumes.

`OUT_FENCE_PTR` (inferred, core): the core creates the out-fence per CRTC and
signals it when the CRTC's event is sent, so on vc4 it signals from
`vc4_crtc_handle_page_flip`. It reports "latched", not "released", which
matches the separate-events wish of SoW §7 only partly.

## 4. NONBLOCK and async page flips

Read from source (2026-11-28):

- `vc4_kms.c:vc4_atomic_commit_setup` (line 545) sets
  `state->legacy_cursor_update = false`, so the core's cursor fast path that
  would complete `commit->flip_done` early is off unless `firmware_kms`.
  NONBLOCK commits take the normal path: the tail runs on a workqueue, and
  commits on one FIFO serialise through `pending_commit`.
- `vc4_kms_load` (line 1175) sets `dev->mode_config.async_page_flip = true` (line 1215).
  `vc4_crtc.c:vc4_page_flip` (line 1114) routes `DRM_MODE_PAGE_FLIP_ASYNC` to
  `vc4_async_page_flip` (gen 4) or `vc5_async_page_flip` (gen 5 and up), else
  `drm_atomic_helper_page_flip`. Both async paths end in
  `vc4_async_page_flip_common` and `vc4_async_set_fence_cb` (line 986).
- `vc4_async_set_fence_cb` does implicit sync by hand: it reads
  `dma_resv_get_singleton(dma_bo->base.resv, DMA_RESV_USAGE_READ, &fence)` and
  completes the flip from a `dma_fence_add_callback`, or at once if there is no
  fence or it is signalled. This applies to the legacy flip ioctl with
  ASYNC on BCM2711 and BCM2712 too, so async legacy flips do wait for the
  buffer's resv fences, but they do not look at `IN_FENCE_FD`: that property
  exists only on atomic.
- Atomic async updates use `vc4_plane_atomic_async_check` (line 2667) and
  `vc4_plane_atomic_async_update` (line 2593). The check returns `-EINVAL`
  unless only `pos0`, `pos2` and `ptr0` words of the display list (and the
  LBM word) changed, so any other change falls back to a sync update.
  Nothing in either function reads `state->fence`.
- `vc4_async_page_flip_common` calls `WARN_ON(drm_crtc_vblank_get(crtc) != 0)`
  and the comment at line 1040 calls the whole async path a hand-made
  prepare_fb/cleanup_fb awaiting generic async page flip.

Inferred: the engine must not combine `IN_FENCE_FD` with the async flag and
expect a wait. Use of async flips would need the resv wait or a userspace
wait.

## What this means for the repository

All inferred unless a line above says read from source.

1. In-fences on BCM2711 and BCM2712 are accepted by the property table but
   not waited for by vc4 code in the atomic path. A backend that sets
   `IN_FENCE_FD` there must still wait in userspace (`poll` on the sync file)
   before the commit.
2. Whether `OUT_FENCE_PTR` signals at the event or earlier needs the core
   source, which this note did not read.
3. A check that would settle questions 1 and 3 on real hardware belongs to a
   board plan, not to this host reading. Mark any claim made from this file as
   read from source, never as a hardware result.
