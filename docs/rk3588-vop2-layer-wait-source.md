# VOP2 layer-config wait and flip timing, read from mainline 6.18 and 6.18.55 (RK Q9)

Written 2026-11-28 (host UTC) for plan #467, item `rk-layer-wait-source`.
Host-only: no board was used, no code was run, and this page makes no
measured claim. It follows up row 13 of `docs/rk3588-kernel-compare.md` and
builds on `docs/rk3588-vop2-flip-source.md`; neither is edited.

Tags: **read from source** names tree, tag and file:line; **inferred** is a
conclusion drawn by this page.

## Sources

- `v6.18` of torvalds/linux (commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449`,
  https://github.com/torvalds/linux/tree/v6.18/drivers/gpu/drm/rockchip).
- `v6.18.55` of gregkh/linux (stand-in for the NanoPC-T6's
  `6.18.55-current-rockchip64`; the tag's commit id was not resolved, only the
  tag was fetched),
  https://github.com/gregkh/linux/tree/v6.18.55/drivers/gpu/drm/rockchip.
- Only `rockchip_drm_vop2.c` and `rockchip_vop2_reg.c` were downloaded (raw
  files, no clone) into the dependency cache and compared with `diff`.
  `drm_atomic_helper.c` was not read, so everything about the generic commit
  tail and `-EBUSY` is inferred. Armbian patches are not covered (item
  `rk-armbian-vop2-read`).
- File aliases: `REG` = `rockchip_vop2_reg.c`, `V2C` = `rockchip_drm_vop2.c`.
  `A` = `v6.18` line numbers, `B` = `v6.18.55` line numbers.

## 1. What the code is

All in `rk3568_vop2_setup_layer_mixer` (`REG` A:2098, B:2133), which RK3588
uses through `rk3568_vop2_setup_overlay` (`REG` A:2316, B:2361).

- It is called from `vop2_crtc_atomic_begin` (`V2C` A:1838-1844, B:1841-1847),
  i.e. once per CRTC that is part of a commit, for every commit, not only for
  modesets. It returns early only when the CRTC has no planes
  (`if (!vp->win_mask) return;`). Read from source.
- It holds `vop2->ovl_lock` (a mutex) across the whole function: lock at B:2154,
  unlock at B:2315. Read from source (v6.18.55).
- `RK3568_OVL_LAYER_SEL` and `RK3568_OVL_PORT_SEL` are shared by all video
  ports, and a change takes effect on one video port's vsync (the source's own
  comment, A:2242-2254, B:2278-2291). Read from source.
- `layer_sel` is rebuilt from `old_layer_sel` and each plane's
  `normalized_zpos` plus the port's layer offset (B:2202). So `layer_sel`
  differs from the previous value only if the window-to-layer binding changes
  (zpos change, a window moved between layers or ports). A commit that only
  changes a framebuffer or coordinates leaves it equal. Read from source for
  the computation; "a plain flip leaves it equal" is inferred.
- The wait is `rk3568_vop2_wait_for_layer_cfg_done` (A:2083-2096,
  B:2118-2131): `readx_poll_timeout_atomic` on `RK3568_OVL_LAYER_SEL` until the
  value read equals `cfg`, with a 50 ms timeout, then
  `DRM_DEV_ERROR("wait layer cfg done timeout ...")`. It does not return an
  error to the commit; the commit goes on. Read from source.

## 2. What the wait blocks on

It blocks the CPU thread running `atomic_begin` for this CRTC (a busy poll of
a register, atomic variant, mutex still held) until the **active**
`LAYER_SEL` register reads equal to a target value, or 50 ms pass. The active
register changes only when the hardware takes a previously written layer
selection at a video port's config-done/vsync (comment at B:2278-2291).
Read from source for the poll; "the hardware latches it at a port vsync" is the
source comment's claim, not a register-manual fact (inferred).

### v6.18 (A)

- Condition: `layer_sel != old_layer_sel && atv_layer_sel != old_layer_sel`
  (A:2266), where `atv_layer_sel` is the active register read at the top and
  `old_layer_sel` is the local copy of the previously programmed value.
- Target: `vop2->old_layer_sel` (A:2267). But A:2240 has already assigned
  `vop2->old_layer_sel = layer_sel`, the **new** value, before the wait. Read
  from source.
- Position: after the `OVL_CTRL` and port-mux writes, before the
  `LAYER_SEL` write at A:2269.
- Inferred consequence: when the condition is true (the previous selection has
  not been taken yet, and a new one is being set), the poll waits for the
  register to show the new value, which this function has not yet written.
  Unless something else makes it so, it runs the full 50 ms and logs the
  timeout. When the previous selection has already latched
  (`atv == old`) there is no wait at all.
- Poll delay 0 (A:2093), a pure spin.

### v6.18.55 (B)

- Condition: `layer_sel != old_layer_sel && atv_layer_sel != old_layer_sel`
  (B:2293) and additionally a read of `RK3568_REG_CFG_DONE` masked to the
  video-port bits, minus this port's own bit, must be non-zero (B:2294-2302):
  that is, **another** port has a config-done pending. Only then is the wait
  called. Read from source.
- Target: the local `old_layer_sel` (B:2301), i.e. the previously programmed
  selection; the wait ends when the hardware has taken it.
- Position: before `OVL_CTRL`, the port-mux write and the `LAYER_SEL` write
  (B:2304-2314).
- Poll delay 10 us (B:2126, and the port-mux wait at B:2107).
- The `vop2_find_start_mixer_id_for_vp` change (`REG` B:1864) is called from
  `vop2_setup_alpha` (B:1973), which `rk3568_vop2_setup_overlay` runs after the
  layer mixer setup (B:2383-2384). It computes alpha mixer ids from
  `vop2->old_layer_sel`, no wait. Read from source. It is not part of the wait.

The compare doc's row 13 says the wait "was unconditional, after" in v6.18. The
source agrees on "conditional now" but in v6.18 the wait was already guarded by
the `layer_sel` and `atv_layer_sel` test, not unconditional, and it sat before
the `LAYER_SEL` write, after `OVL_CTRL`. What moved is its position relative
to the `OVL_CTRL` and port-mux writes, its target value, and the extra
other-port test. Read from source (both trees).

## 3. Can it delay the vblank / flip event?

- The flip event is sent from the frame-start interrupt once this port's bit in
  `RK3568_REG_CFG_DONE` is clear (`docs/rk3588-vop2-flip-source.md` section 2;
  same code in `vop2_isr` at B:2205 onward). The wait is not on that path: it
  neither sends nor withholds the event. Read from source.
- The wait runs in `atomic_begin`, before `vop2_crtc_atomic_flush` calls
  `vop2_cfg_done` (`V2C` B:1850-1863). So a long wait pushes the config-done
  write later. If it is pushed past the next frame-start interrupt, the event
  goes out one frame later (the ISR sees the bit still set, or `vp->event` not
  yet stored). Read from source for the order; the one-frame consequence is
  inferred.
- When the wait actually runs, then:
  - v6.18: up to 50 ms (about three frames at 60 Hz), likely the full time
    (section 2); inferred.
  - v6.18.55: only if another port has a pending config-done and the layer
    selection changes; wait ends when the previous selection latched, normally
    within about a frame; inferred.
- Commits that do not change `layer_sel` (fb or geometry updates, same zpos)
  never reach the wait in either tree (the condition is false). Inferred from
  the computation in section 1.
- Single-output case: with one active port, other-port `CFG_DONE` bits are
  zero, so v6.18.55 never waits (B:2294-2302). v6.18 can still wait, if the
  previous change has not latched. Inferred.

## 4. Does it change when a second nonblocking commit gets -EBUSY?

- The wait does not return or set any error and the VOP2 driver has no
  `-EBUSY` of its own (`docs/rk3588-vop2-flip-source.md` section 4). Read from
  source.
- A nonblocking commit does its work in the commit tail, after the ioctl has
  returned (inferred from the generic helper, not read). The `-EBUSY` for a
  second commit comes from the helper checking the first commit's `flip_done`
  at the second ioctl (also inferred, not read here).
- So the wait changes `-EBUSY` only indirectly: it stretches the time between
  the first commit's ioctl return and its `flip_done` (which is signalled with
  the event). A second commit issued in that longer window gets `-EBUSY`
  where it might not with no wait. Inferred.
- Scope: only commits that change `layer_sel`. A flip-only overlay commit
  should see the same `-EBUSY` window as any other commit, about one frame.
  Inferred.

## 5. What `t6-commit-session` should expect

All of these are expectations from reading, to be compared with the
measurement; none is a measurement.

1. **Flip-only overlay commit** (same plane, same zpos, new fb, or the first
   commit of a second window at an unchanged binding): no wait in either tree.
   Expect one flip event per commit, arriving about one frame after the
   commit, as in `docs/rk3588-vop2-flip-source.md`. Inferred.
2. **Commit that enables an overlay or changes its zpos on a single active
   port**, if the board runs the v6.18.55 behaviour: no wait (no other port
   pending). Expect the same one-frame latency. If the board shows 2-3 frames
   of latency, or a kernel log line `wait layer cfg done timeout`, that points
   to the v6.18 behaviour or to another port being active. Inferred.
3. **Commit on port A while port B has a pending commit and the layer binding
   changes**: v6.18.55 waits for the previous selection to latch, typically up
   to about a frame; the event may be delayed by about one extra frame.
   Inferred.
4. **Second nonblocking commit before the first flip event**: expect `-EBUSY`
   as in the flip doc. The window may be longer than one frame for a commit
   that changed `layer_sel` (cases 2 and 3). Inferred.
5. **Cheap check from the board session**: note whether the kernel log shows
   the timeout string above after the run (reading the log is read-only, but it
   is a board action for that session, not this item). A timeout line means the
   wait ran its full 50 ms. Inferred from the `DRM_DEV_ERROR` at B:2128 / A:2094.
6. **Which tree the board runs**: the board's kernel is `6.18.55`, so the
   v6.18.55 rows are the expected ones. The Armbian patch set was not read, so
   whether it changes these functions is unknown.

## 6. Open

- `drm_atomic_helper_setup_commit` and the commit tail were not read; every
  `-EBUSY` and commit-tail ordering claim stays inferred.
- Which of the A or B behaviours the NanoPC-T6 actually has depends on the
  Armbian tree; the board session cannot tell by timing alone from the
  v6.18-no-wait cases.
- The RK3576 path (`rk3576_vop2_setup_layer_mixer`, B:2388) has no such wait
  in the lines read; not relevant to RK3588.
