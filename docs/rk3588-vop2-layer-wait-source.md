# RK3588 VOP2 layer-config wait: what it blocks on and what it does to flip timing

Written 2026-11-28 (host UTC) for plan #467, item `rk-layer-wait-source`
(row 13 of [rk3588-kernel-compare.md](rk3588-kernel-compare.md); RK Q9 of
`docs/rk3588-backend-design.md` section 7). Host-only: no board was used, no
code was run. This page makes no measured claims.

Tags: **read from source** names tree, ref and `file:line`. **inferred** is a
conclusion drawn here from what was read; it is not a hardware fact.

## Sources read

- `v6.18` of torvalds/linux
  (https://github.com/torvalds/linux/tree/v6.18/drivers/gpu/drm/rockchip):
  `rockchip_vop2_reg.c` (`REG`, 2577 lines), `rockchip_drm_vop2.c` (`V2C`,
  2793 lines), `drivers/gpu/drm/drm_atomic_helper.c` (`HELP`).
- `v6.18.55` of gregkh/linux
  (https://github.com/gregkh/linux/tree/v6.18.55/drivers/gpu/drm/rockchip):
  `REG` (2622 lines), `V2C` (2796 lines). This is the stable number of the
  NanoPC-T6's running kernel (`6.18.55-current-rockchip64`, see
  [rk3588-kernel-compare.md](rk3588-kernel-compare.md)); the Armbian patch set
  on top of it was not read, so what the board runs is inferred to match
  `v6.18.55` in these files.
- `diff` of the two `V2C` copies: only the RK3576 / `YUYV10_1X20` output-format
  lines differ (read from source, `V2C:339-355`, v6.18 against v6.18.55). The
  flip path in [rk3588-vop2-flip-source.md](rk3588-vop2-flip-source.md) is
  therefore the same at both refs. Line numbers below: `v6.18` unless a
  `v6.18.55` number is given.

The wait lives in `REG`, not in `V2C`: the RK3588 ops table points
`.setup_overlay` at `rk3568_vop2_setup_overlay` (read from source,
`REG:2457-2460` v6.18, `REG:2502-2505` v6.18.55).

## 1. What the wait blocks on

The function is `rk3568_vop2_wait_for_layer_cfg_done` (read from source,
`REG:2083-2097` v6.18, `REG:2118-2132` v6.18.55). It polls
`RK3568_OVL_LAYER_SEL` (the `OVL_LAYER_SEL` register that is shared by all
video ports, per the source comment at `REG:2241-2255`) with
`readx_poll_timeout_atomic` until the register reads back equal to a given
`cfg` value, with a 50 ms timeout; on timeout it only logs
`wait layer cfg done timeout` (`DRM_DEV_ERROR`) and returns, no error reaches
the commit (read from source, same lines). It is a busy poll; it does not
sleep. `rk3568_vop2_setup_layer_mixer` holds `vop2->ovl_lock` (a mutex) around
it (read from source, `REG:2118` lock, `REG:2270` unlock, v6.18), so it runs in
process context and is not under the `event_lock` spinlock. Whether the
atomic-poll macro leaves interrupts enabled was not read; a mutex holder is
not an interrupt-disabled context (inferred).

It is called from `vop2_crtc_atomic_begin` via `vop2->ops->setup_overlay(vp)`
(read from source, `V2C:1844`, v6.18; `V2C:1847`, v6.18.55), once per commit
that touches the CRTC, as long as the port has at least one window
(`if (!vp->win_mask) return;`, read from source, `REG:2334-2337` v6.18).

### v6.18 (the source pin)

Sequence in `rk3568_vop2_setup_layer_mixer` (read from source, `REG:2154-2269`):

1. `atv_layer_sel` = readback of `OVL_LAYER_SEL` (`REG:2154`);
   `old_layer_sel` = `vop2->old_layer_sel`, the value the driver last wrote
   (`REG:2155`).
2. The new `layer_sel` is built from the CRTC's windows and `zpos`
   (`REG:2156-2239`).
3. `vop2->old_layer_sel = layer_sel` (the **new** value, `REG:2240`).
4. If the port mux changed: write `OVL_PORT_SEL`, `vop2_cfg_done(vp)`, wait
   for the port mux (`REG:2260-2264`).
5. `if (layer_sel != old_layer_sel && atv_layer_sel != old_layer_sel)` wait
   for layer config done, passing `vop2->old_layer_sel` (`REG:2266-2267`).
6. Write `OVL_LAYER_SEL = layer_sel` (`REG:2269`).

Reading the code in order: the guard (step 5) means "the layer map is about to
change and the hardware has not yet taken the value written last time"
(read from source: `atv_layer_sel != old_layer_sel`). But the argument passed
is `vop2->old_layer_sel`, which step 3 has already replaced with the new
`layer_sel`, and the new value is written only in step 6, after the wait.
Inferred: whenever the guard is true, the poll waits for the register to show a
value that has not been written yet, so it can only end by the 50 ms timeout
and the error message, unless the hardware register already equals the new
value. That makes the v6.18 wait, when entered, a fixed delay of up to
50 ms before the commit's `CFG_DONE` write, with a log line. It is not entered
when the layer map is unchanged (`layer_sel == old_layer_sel`) or when the last
write has already taken effect (`atv == old`). Whether the hardware readback
is the active or the shadow value is a hardware fact the source does not
state.

### v6.18.55 (the running kernel's stable number)

The change recorded in row 13 of the compare doc (read from source,
`REG:2292-2303` v6.18.55, against `REG:2266-2267` v6.18):

- The wait moves **before** the `OVL_CTRL` and `OVL_PORT_SEL` writes (step 5
  now precedes step 4).
- It is passed the local `old_layer_sel` (the previously written value), so
  the condition "register equals what was written last time" can be met.
- It is entered only if, in addition to the old guard, another video port has
  a config-done bit set: it reads `RK3568_REG_CFG_DONE`, masks to
  `BIT(nr_vps) - 1`, clears this port's bit, and waits only if any bit is
  left (`/* Changes of other VPs' overlays have not taken effect */`, read from
  source, `REG:2293-2302`).
- The poll delay changes from 0 to 10 microseconds, in both the layer wait and
  the port-mux wait (read from source, `REG:2107` and `REG:2127` v6.18.55;
  `REG:2072` and `REG:2092` v6.18).
- `vop2_find_start_mixer_id_for_vp` now takes the port and derives the start
  mixer id from `vop2->old_layer_sel` instead of counting windows of lower
  ports (read from source, `REG:1864-1888` v6.18.55, `REG:1845-1853` v6.18).
  This is not a wait, but it feeds the same ordering.

So the thing the running kernel's wait blocks on is: the hardware's layer-select
register reaching the value this driver wrote previously, while another port's
config-done bit is still pending. Inferred reading of the comment: that is a
change that another CRTC queued and the hardware has not yet taken at that
port's vsync.

## 2. Can it delay the vblank or flip event?

- The vblank counter and the flip-event decision run in `vop2_isr`
  (read from source, `V2C:2231-2245` v6.18): on frame start the counter
  advances, and a stored event is sent only when this port's config-done bit
  is clear. The ISR does not touch `ovl_lock` or the wait (read from source,
  same lines). Inferred: the wait does not stop vblank counting; the vblank
  timestamps and sequence of the CRTC keep running.
- The event of a commit is stored in `vop2_crtc_atomic_flush` after
  `vop2_cfg_done` (see [rk3588-vop2-flip-source.md](rk3588-vop2-flip-source.md)
  section 1). The wait runs earlier, in `atomic_begin`, so it is on the path
  before this commit's `CFG_DONE` write (read from source: `V2C:1844` begin,
  `V2C:1860` flush, v6.18, order as in `drm_atomic_helper_commit_planes`).
  Inferred: when the wait is entered, it postpones this commit's `CFG_DONE` by
  up to 50 ms, and so postpones its flip event by the same amount plus the
  rest of the frame wait (a CFG_DONE written after the frame-start of frame
  N is taken at frame N+1). At 60 Hz 50 ms is about three frames
  (inferred arithmetic).
- It delays only the commit that enters it. A commit that does not change the
  layer map (same windows, same `zpos` values, only a new framebuffer or
  changed source/destination rectangles) skips it at both refs, since
  `layer_sel == old_layer_sel` (read from source, the guard above; whether a
  given overlay commit changes `layer_sel` depends on the window-to-layer
  mapping that `vop2->old_layer_sel` already holds, inferred).

## 3. Does it change when a second nonblocking commit gets -EBUSY?

The `-EBUSY` is decided in the ioctl path by the generic helper, before any
driver code runs. `drm_atomic_helper_setup_commit` returns `-EBUSY` for a
nonblocking commit when the previous commit of the same CRTC has not completed
`flip_done` (read from source, `HELP:2296-2318` `stall_checks`, v6.18) or when
an old plane state's commit has not completed `flip_done`
(`HELP:2521-2528`, v6.18). `flip_done` is completed with the DRM event
(`drm_crtc_send_vblank_event`, see the flip doc). VOP2 adds no `-EBUSY` of its
own (read from source, flip doc section 4).

So the wait changes the answer only through duration (inferred): a first
commit that enters the wait reaches `CFG_DONE` and its flip event later, so
`flip_done` completes later, so the interval in which a second nonblocking
commit on the same CRTC (or on the same plane) gets `-EBUSY` is longer by the
wait. The wait never turns an `-EBUSY` into success or the reverse by itself.
The first commit's own `-EBUSY`-free return is also unaffected: a nonblocking
commit's ioctl returns before the commit tail, where `atomic_begin` runs
(read from source, `drm_atomic_helper_commit_tail_rpm` calls
`drm_atomic_helper_commit_planes`, `HELP:1934-1952`, v6.18, and the
Rockchip tail is that function per the flip doc); a **blocking** commit's
ioctl would include the wait (inferred).

A different second commit: one from another CRTC is not stalled by
`stall_checks` (it is per CRTC); in the v6.18.55 code its `atomic_begin` is the
case that can enter the wait, because the first CRTC's `CFG_DONE` bit is the
"other port pending" (inferred).

## 4. What t6-commit-session should expect

Setting: one overlay plane on one CRTC, `pending_probe` commits, second commit
issued before the flip event; the other outputs idle. The running kernel is
inferred to carry the `v6.18.55` code.

| Point | Expectation | Tag |
| --- | --- | --- |
| Lone overlay commit on an otherwise idle second port | The v6.18.55 wait needs another port's `CFG_DONE` bit set; none is set when the other CRTCs are not committing. It is not entered, so the flip event comes one frame-start after the `CFG_DONE` write, as in the flip doc. | read from source (`REG:2293-2302` v6.18.55); consequence inferred |
| First commit that enables the overlay (layer map may change) | Same: no wait without a pending `CFG_DONE` on another port. | inferred |
| Flip/latch counts | Equal to the number of accepted commits; the wait changes no count, only timing. | inferred |
| Second commit before the flip event | `-EBUSY` from the helper (`HELP:2296-2318`), independent of the wait; its window is the time to `flip_done`. | read from source (`HELP:2296-2318`, v6.18) |
| Flip latency outlier | A flip delayed by roughly 10 to 50 ms beyond a frame, or a kernel log line `wait layer cfg done timeout` or `wait port_mux done timeout`, would point at this wait. This needs two outputs committing near each other with a changed layer map. | inferred |
| If the session ran the v6.18 code instead | A commit whose layer map changes while the last write is not yet taken would spin to the 50 ms timeout and log `wait layer cfg done timeout` even with one CRTC. The same lone overlay case is unaffected when `atv_layer_sel == old_layer_sel`, which is the normal state between latched commits. | inferred from the argument-order reading in section 1 |

Recommendation for the board session (not a claim): record the kernel log for
the two message strings above next to each flip run, and run any multi-output
commits as a separate, labelled case so a delay can be attributed to this
wait. The compare doc's row 15 (flip semantics identical at both refs in
`vop2_crtc_atomic_begin`) is consistent with this page: the commit path is the
same, only the optional wait differs.

## 5. Not covered

- The Armbian patch set for `drivers/gpu/drm/rockchip/` (unread).
- The vendor 6.1 tree (not read).
- Whether the hardware `OVL_LAYER_SEL` readback shows the shadow or the active
  value, and when the hardware takes it (a register-manual fact).
- Any measurement: the numbers 10 and 50 are source constants, not timings
  seen on a board.
