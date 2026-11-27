# Pending-commit replacement and flip events in DRM core, vc4 and VOP2

Status: source read, written 2026-11-28. **This document claims nothing about hardware.** No board
was run, no code is changed. The board sweep items (`kms-sweep`, `t6-sweep`, `rpi5-sweep`,
`cm4-sweep`) check the answers below on hardware.

It answers two open questions from the backend designs:

- HVS Q6 in [`hvs-backend-design.md`](hvs-backend-design.md) §7: may a not-yet-latched commit be
  replaced, and what does the backend report?
- RK Q9 in [`rk3588-backend-design.md`](rk3588-backend-design.md) §7: overlay-plane flip
  behaviour and latch reporting (page-flip event per CRTC).

## Tags

- **read from source**: read in the file and function named, at the commit named in "Sources".
  The line says what the code does, not what the hardware does.
- **inferred**: a conclusion drawn from source lines, or a kernel file that was not read.
- Hardware results, which would need a date: this document copies none from `docs/observed/`, so
  no line below carries that tag.

## Sources

| Tree | Ref | Commit | Files read |
|------|-----|--------|------------|
| mainline Linux | tag `v6.18` ("Linux 6.18", 2025-11-30) | `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` | `drivers/gpu/drm/drm_atomic_helper.c`, `drm_atomic_uapi.c`, `drm_atomic.c`, `drm_vblank.c`, `drm_file.c`, `rockchip/rockchip_drm_vop2.c`, `rockchip_drm_vop2.h`, `rockchip_drm_fb.c`, `rockchip_drm_drv.c` |
| Raspberry Pi kernel | branch `rpi-6.18.y` | `16ae2649dcfadf9d81445b1177a3191efa3cb82e` | `drivers/gpu/drm/vc4/vc4_kms.c`, `vc4_crtc.c`, `vc4_hvs.c`, `vc4_plane.c` |

URLs, each file at its commit:

- `https://raw.githubusercontent.com/torvalds/linux/7d0a66e4bb9081d75c82ec4957c50034cb0ea449/drivers/gpu/drm/`
- `https://raw.githubusercontent.com/raspberrypi/linux/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4/`

How the files were obtained (an honest limit): `git.kernel.org` and `git clone` were not reachable
from the node that wrote this, so there is no clone under the dependency cache for these two
commits. Each file was fetched as raw text at the pinned commit; files longer than the fetch tool's
page size were read in several offset fetches and the code was quoted from the raw text, not from
the tool's summaries. The Rockchip file `rockchip_drm_vop2.c` was also read from the copy in the
dependency cache (`linux-v6.18/rockchip_drm_vop2.c`, labelled v6.18, no git history); its
`vop2_crtc_atomic_flush`, `vop2_isr` and `vop2_crtc_atomic_disable` matched the pinned fetch.
Mainline `vc4` was **not** read here (item `vc4-mainline-source` compares it with `rpi-6.18.y`);
every vc4 line below is for `rpi-6.18.y` only. `drm_atomic_helper_wait_for_vblanks` and
`drm_mode_cursor_universal` were not read.

## 1. DRM core

### 1.1 Can a second commit replace one that has not latched?

**No, not in the helper path.** The commit queue is per CRTC and strictly ordered.

- `drm_atomic_helper_setup_commit` gives every CRTC in the state a new `drm_crtc_commit` and calls
  `stall_checks(crtc, nonblock)`. (read from source: `drm_atomic_helper.c:
  drm_atomic_helper_setup_commit`, v6.18 commit above)
- `stall_checks` looks at the head of `crtc->commit_list` (the newest commit, added by
  `drm_atomic_helper_swap_state`). If that commit's `flip_done` has not completed and the new
  commit is nonblocking, it returns **`-EBUSY`** ("busy with a previous commit"). (read from
  source: `drm_atomic_helper.c: stall_checks`)
- A blocking commit does not fail there. It queues behind the previous one:
  `commit_tail` runs `drm_atomic_helper_wait_for_dependencies`, which waits for `hw_done` and
  `flip_done` of the old commit of each CRTC, plane and connector (`drm_crtc_commit_wait`; its own
  comment reads "Currently no support for overwriting flips, hence stall for previous one to
  execute completely"). (read from source: `drm_atomic_helper.c:
  drm_atomic_helper_wait_for_dependencies`; `drm_atomic.c: drm_crtc_commit_wait`)
- The same `-EBUSY` is returned from `setup_commit` when a **plane** or **connector** in the new
  state still has an old commit whose `flip_done` is incomplete. So a commit that touches only an
  overlay plane is refused while any earlier commit of that plane's CRTC is unflipped. (read from
  source: `drm_atomic_helper.c: drm_atomic_helper_setup_commit`, the connector and plane loops)
- Nonblocking does not mean "never sleeps": `stall_checks` also waits (interruptible, 10 s
  timeout) for `cleanup_done` of the **second** previous commit, so up to two commits can be in
  flight. (read from source: `drm_atomic_helper.c: stall_checks`)
- `-ERESTARTSYS`/`-EINTR` can come from that wait or from the modeset lock acquire; `-EDEADLK` is
  retried inside the ioctl. (inferred: the wait is `wait_for_completion_interruptible_timeout`;
  the retry is `drm_mode_atomic_ioctl`'s `retry:` label, read from source)

Consequence: with the DRM helpers, "replace a pending commit" is not a state the kernel offers
through the atomic ioctl. A driver would have to supply its own `atomic_commit` hook; neither vc4
nor Rockchip does (sections 2 and 3).

### 1.2 What the flags do

- `DRM_MODE_ATOMIC_TEST_ONLY` runs `drm_atomic_check_only` and nothing else; `prepare_signaling`
  returns first without creating events or fences. Combined with `DRM_MODE_PAGE_FLIP_EVENT` the
  ioctl returns `-EINVAL`. (read from source: `drm_atomic_uapi.c: drm_mode_atomic_ioctl`,
  `prepare_signaling`)
- `DRM_MODE_ATOMIC_NONBLOCK` only selects `drm_atomic_nonblocking_commit`, which calls
  `config->funcs->atomic_commit(dev, state, true)`; the helper then returns after
  `queue_work(system_unbound_wq, ...)`. `DRM_MODE_ATOMIC_ALLOW_MODESET` only sets
  `state->allow_modeset`. (read from source: `drm_atomic_uapi.c: drm_mode_atomic_ioctl`;
  `drm_atomic.c: drm_atomic_nonblocking_commit`; `drm_atomic_helper.c: drm_atomic_helper_commit`)
- `DRM_MODE_PAGE_FLIP_EVENT`: `prepare_signaling` creates one `DRM_EVENT_FLIP_COMPLETE` event for
  **each CRTC in the state**. If no CRTC is in the state it returns `-EINVAL` ("need at least one
  CRTC"). If a CRTC with an event is inactive before and after, `drm_atomic_crtc_check` returns
  `-EINVAL` ("requesting event but off"). (read from source: `drm_atomic_uapi.c:
  prepare_signaling`; `drm_atomic.c: drm_atomic_crtc_check`)
- Without `PAGE_FLIP_EVENT`, `setup_commit` still allocates an internal event for every active
  CRTC so that `flip_done` can complete; it has no file and is freed on send. (read from source:
  `drm_atomic_helper.c: drm_atomic_helper_setup_commit`; `drm_file.c: drm_send_event_helper`)

### 1.3 Which CRTCs are in the state (overlay-only commits)

Setting any property of a plane that currently has a CRTC calls `drm_atomic_get_plane_state`,
which also calls `drm_atomic_get_crtc_state` for `plane_state->crtc`. So an update of an
overlay's `FB_ID` or `CRTC_X` with an unchanged `CRTC_ID` puts that plane's CRTC in the state; the
CRTC then gets an event, `atomic_begin` and `atomic_flush`. (read from source: `drm_atomic.c:
drm_atomic_get_plane_state`; `drm_atomic_uapi.c: drm_atomic_set_property`, plane branch;
`drm_atomic_helper.c: drm_atomic_helper_commit_planes`)

An overlay that is **not** yet on a CRTC and gets only `FB_ID` adds no CRTC; it fails with "FB set
but no CRTC". Turning an overlay on needs `CRTC_ID` set in the same commit, which adds the CRTC
through `drm_atomic_set_crtc_for_plane`. (read from source: `drm_atomic.c: drm_atomic_plane_check`;
`drm_atomic_uapi.c: drm_atomic_set_crtc_for_plane`)

### 1.4 When is the event sent?

- The event completes `commit->flip_done` in the same call that queues it to the file:
  `drm_send_event_helper` runs `complete_all(e->completion)` and then moves the event to the
  file's list. So a client that has read the event can immediately commit again without `-EBUSY`
  from the previous commit's `flip_done`. (read from source: `drm_file.c: drm_send_event_helper`;
  the conclusion about the client is inferred)
- Who calls send is the driver. The core offers two ways: `drm_crtc_arm_vblank_event` (sent at the
  next vblank, its sequence is the current count plus one) and `drm_crtc_send_vblank_event` (sent
  now, with the sequence and timestamp of the most recent vblank). The kernel doc of the arm
  function says it is only safe if the hardware commits at exactly the vblank that follows the
  arm, such as with a "go" bit, and that otherwise drivers must send from their interrupt handler
  and exclude the race with the hardware commit. (read from source: `drm_vblank.c:
  drm_crtc_arm_vblank_event`, `drm_crtc_send_vblank_event`, `send_vblank_event`)
- The sequence and timestamp in the event come from `drm_vblank_count_and_time` at send time, not
  from the moment of the hardware latch. (read from source: `drm_vblank.c:
  drm_crtc_send_vblank_event`)
- `drm_atomic_helper_fake_vblank` sends the event itself only for CRTCs with `no_vblank`. (read
  from source: `drm_atomic_helper.c: drm_atomic_helper_fake_vblank`)
- `drm_atomic_helper_commit_hw_done` carries `WARN_ON(new_crtc_state->event)`: the driver must
  have consumed the event by then, either sent or stored. (read from source:
  `drm_atomic_helper.c: drm_atomic_helper_commit_hw_done`)

### 1.5 Async paths (not used by the engine)

`drm_atomic_helper_commit` takes a shortcut when `state->async_update` is set: it calls the
plane's `atomic_async_update` and returns without `setup_commit`, so no event and no `flip_done`.
`async_update` is set in `drm_atomic_helper_check` only when `state->legacy_cursor_update` is set
and `drm_atomic_helper_async_check` passes (one plane, no CRTC change, no modeset, `-EBUSY` if the
plane's previous commit has not reached `hw_done`). (read from source: `drm_atomic_helper.c:
drm_atomic_helper_commit`, `drm_atomic_helper_async_check`)
`DRM_MODE_PAGE_FLIP_ASYNC` in the atomic ioctl needs `mode_config.async_page_flip`, and for a
non-primary plane asks the driver's `atomic_async_check`. (read from source: `drm_atomic_uapi.c:
drm_mode_atomic_ioctl`, `drm_atomic_set_property`)
The engine's DRM backend never passes `PAGE_FLIP_ASYNC` and does not use the legacy cursor
ioctls; it commits with `ALLOW_MODESET | PAGE_FLIP_EVENT` for the first commit of an output and
`NONBLOCK | PAGE_FLIP_EVENT` after. (read from source: this repository,
`crates/composition-drm/src/lib.rs`, the commit method, at commit `2e458d2`)

## 2. vc4 (`rpi-6.18.y`, HVS5 on BCM2711, HVS6 on BCM2712)

### 2.1 Replacement and errno

- Both mode-config tables use `.atomic_commit = drm_atomic_helper_commit`; there is no vc4
  specific nonblocking path. (read from source: `vc4_kms.c: vc4_mode_funcs`, `vc5_mode_funcs`)
  So section 1.1 applies as it is: nonblocking commit on a CRTC whose newest commit has not
  completed `flip_done` returns **`-EBUSY`**; a blocking one waits.
- vc4 adds a second, per-HVS-channel wait: `vc4_atomic_commit_setup` stores the new commit as
  `fifo_state[channel].pending_commit` for each CRTC in the state whose channel is in use, and
  `vc4_atomic_commit_tail` first calls `drm_crtc_commit_wait` on the old state's `pending_commit`
  of every in-use channel (hw_done and flip_done) and logs "Timed out waiting for commit" on
  failure. The commit therefore queues behind the previous commit **on every channel in use**, not
  only its own. (read from source: `vc4_kms.c: vc4_atomic_commit_setup`,
  `vc4_atomic_commit_tail`)
- The tail then runs `drm_atomic_helper_commit_planes(..., DRM_PLANE_COMMIT_ACTIVE_ONLY)`,
  `drm_atomic_helper_fake_vblank`, `drm_atomic_helper_commit_hw_done` and
  `drm_atomic_helper_wait_for_flip_done`, so a commit's work item ends only after its event has
  been sent. (read from source: `vc4_kms.c: vc4_atomic_commit_tail`)
- `vc4_atomic_commit_setup` forces `state->legacy_cursor_update = false` unless
  `firmware_kms`, "to prevent `drm_atomic_helper_setup_commit()` from auto-completing
  `commit->flip_done`": vc4 does not let cursor updates run unsynchronised past the queue. (read
  from source: `vc4_kms.c: vc4_atomic_commit_setup`)
- `dev->mode_config.async_page_flip = true`, and the plane helper tables carry
  `atomic_async_check`/`atomic_async_update`. The async check passes only when, apart from the
  `pos0`, `pos2`, `ptr0` (and LBM offset) dlist words, the new plane dlist equals the old one and
  its length, offsets and LBM size are unchanged; the update then writes those three words into the
  live hardware dlist. This is the path of an async page flip or of a legacy cursor update; it
  has no event and no `flip_done`. (read from source: `vc4_kms.c: vc4_kms_load`;
  `vc4_plane.c: vc4_plane_atomic_async_check`, `vc4_plane_atomic_async_update`)
- vc4 frees a replaced dlist only after the HVS has moved on: `vc4_hvs_mark_dlist_entry_stale`
  puts it on `stale_dlist_entries` until the channel's frame counter passes `target_frame_count`
  and the EOF interrupt sweeps it. The dlist is double-buffered by allocation, not by a latch
  shadow the driver can overwrite. (read from source: `vc4_hvs.c: vc4_hvs_mark_dlist_entry_stale`,
  `vc4_hvs_schedule_dlist_sweep`)

Answer for vc4: a second commit cannot replace one that has not latched. A nonblocking one fails
with `-EBUSY` (core, `stall_checks`); a blocking one waits for the first to flip. (read from
source; the sentence is a conclusion, so inferred as a summary)

### 2.2 Event timing relative to the hardware latch

- `vc4_hvs_atomic_flush` rewrites the whole dlist of the CRTC from all of its planes in zpos
  order, and, if the CRTC was and stays active, calls `vc4_hvs_install_dlist` (writes the new
  dlist start to `SCALER_DISPLISTX(channel)`; HVS6: `SCALER6_DISPX_LPTRS`) and
  `vc4_hvs_update_dlist`. (read from source: `vc4_hvs.c: vc4_hvs_atomic_flush`,
  `vc4_hvs_install_dlist`)
- `vc4_hvs_update_dlist` takes a vblank reference, moves `crtc->state->event` into
  `vc4_crtc->event` and records `vc4_crtc->current_dlist = <new dlist start>`. It does not send
  the event. (read from source: `vc4_hvs.c: vc4_hvs_update_dlist`)
- The event is sent from the pixelvalve interrupt `PV_INT_VFP_START` (start of vertical front
  porch): `vc4_crtc_irq_handler` -> `vc4_crtc_handle_vblank` -> `vc4_crtc_handle_page_flip`. That
  function reads the **active** dlist pointer (`SCALER_DISPLACTX(chan)`; HVS6:
  `SCALER6_DISPX_DL` field `LACT`) and sends the event only if it equals `vc4_crtc->current_dlist`
  (or the CRTC feeds the transposer); otherwise the event stays and the next vblank tries again.
  (read from source: `vc4_crtc.c: vc4_crtc_irq_handler`, `vc4_crtc_handle_vblank`,
  `vc4_crtc_handle_page_flip`)
- So the event is **never sent before the hardware reports the new dlist as active**, and it is
  sent at the next front-porch interrupt after that, not at the latch instant. The scan-out of
  the new buffers may have started up to a frame earlier than the event's timestamp. (read from
  source for the condition; the up-to-one-frame lag is inferred from the interrupt being at
  front-porch start and the register being a polled status)
- Where in the frame the HVS switches to a new dlist start is a hardware property that this
  source does not state. (inferred: not in the files read)
- If the CRTC is disabled in the same commit `vc4_crtc_atomic_disable` sends the event itself
  through `vc4_crtc_send_vblank`. (read from source: `vc4_crtc.c: vc4_crtc_atomic_disable`,
  `vc4_crtc_send_vblank`)
- `vc4_crtc` holds exactly one `event` slot. That is safe only because the core queue of 2.1
  stops a second commit from reaching `atomic_flush` before the first event was sent.
  (read from source for the single slot; the safety argument is inferred)

### 2.3 Does an overlay-only commit get an event?

**Yes.** An overlay-only commit puts its CRTC in the state (1.3). `vc4_hvs_atomic_flush` is the
CRTC's `atomic_flush` for all generations (HVS5 and HVS6 share it), it rewrites and installs the
full dlist, and arms the event as in 2.2. The commit completes `flip_done` when that event is
sent. (read from source: `vc4_crtc.c: vc4_crtc_helper_funcs`; `vc4_hvs.c: vc4_hvs_atomic_flush`)
A commit that leaves the dlist byte-identical still installs the same start pointer, so the
active pointer already matches and the next VFP_START sends the event. (inferred)

## 3. Rockchip VOP2 (mainline `v6.18`, RK3588)

### 3.1 Replacement and errno

- The mode config is generic: `.atomic_commit = drm_atomic_helper_commit`, `.atomic_check =
  drm_atomic_helper_check`, `.atomic_commit_tail = drm_atomic_helper_commit_tail_rpm`. There is no
  `atomic_commit_setup` and no driver specific queue. (read from source:
  `rockchip_drm_fb.c: rockchip_drm_mode_config_funcs`, `rockchip_mode_config_helpers`; the
  function name `rockchip_drm_mode_config_init` is called from `rockchip_drm_drv.c:
  rockchip_drm_bind`)
- `vop2_plane_helper_funcs` has `atomic_check`, `atomic_update`, `atomic_disable` only, and the
  file has no `atomic_async` and no `async_page_flip`. `vop2_crtc_funcs.page_flip` is
  `drm_atomic_helper_page_flip`. So there is no async or unsynchronised path at all. (read from
  source: `rockchip_drm_vop2.c: vop2_plane_helper_funcs`, `vop2_crtc_funcs`; the absence was
  checked over the whole file)
- Therefore section 1.1 applies as it is: a nonblocking commit on a CRTC whose newest commit has
  not completed `flip_done` returns **`-EBUSY`**; a blocking one waits.
  (read from source for the pieces; summary inferred)
- `vop2_crtc_atomic_check` has no event or `active` logic; it only rejects a CRTC with more
  planes than `vp->nlayers` with `-EINVAL`. (read from source: `rockchip_drm_vop2.c:
  vop2_crtc_atomic_check`)

### 3.2 Event timing relative to the latch

- `vop2_plane_atomic_update` and `vop2_plane_atomic_disable` write only window registers; neither
  calls `vop2_cfg_done`. (read from source: `rockchip_drm_vop2.c: vop2_plane_atomic_update`,
  `vop2_plane_atomic_disable`)
- `vop2_crtc_atomic_begin` calls `ops->setup_overlay(vp)`. `vop2_crtc_atomic_flush` calls
  `vop2_post_config(crtc)`, then `vop2_cfg_done(vp)`, and **after** that, under `event_lock`,
  takes a vblank reference (`drm_crtc_vblank_get`) and moves `crtc->state->event` into `vp->event`.
  It does not send the event. (read from source: `rockchip_drm_vop2.c: vop2_crtc_atomic_begin`,
  `vop2_crtc_atomic_flush`)
- `vop2_cfg_done` sets `GLB_CFG_DONE_EN` (bit 15 of `RK3568_REG_CFG_DONE`, offset 0) and the VP's
  own bit with its write-mask bit (`BIT(vp->id) | BIT(vp->id) << 16`). The header comment says
  `GLB_CFG_DONE_EN` has no write-mask bit. (read from source: `rockchip_drm_vop2.h: vop2_cfg_done`)
- The event is sent from the ISR: `vop2_isr` (RK3576 and later: `rk3576_vp_isr`) handles
  `VP_INT_FS_FIELD` by `drm_crtc_handle_vblank`, then, under `event_lock`, if `vp->event` is set
  it reads `RK3568_REG_CFG_DONE` and sends the event with `drm_crtc_send_vblank_event` and drops
  the vblank reference **only if `BIT(vp->id)` is clear**. If the bit is still set, the event
  stays for the next `FS_FIELD`. (read from source: `rockchip_drm_vop2.c: vop2_isr`,
  `rk3576_vp_isr`)
- So the event is sent at the first frame-start interrupt at which the VP's `CFG_DONE` bit has
  cleared. That the hardware clears the bit when it has taken the shadowed configuration into the
  live registers, which is what "latched" means, is not stated by this source. (inferred from the
  driver testing the bit; the register semantics are in the SoC manual, which was not read)
- The vblank reference is held from `atomic_flush` to the event send, so `FS_FIELD` is enabled
  for exactly the commits that wait for an event (`vop2_crtc_enable_vblank` enables
  `VP_INT_FS_FIELD`). (read from source: `rockchip_drm_vop2.c: vop2_crtc_enable_vblank`)
- The `commit_tail_rpm` order is: modeset disables, modeset enables, `commit_planes(ACTIVE_ONLY)`,
  `fake_vblank`, `commit_hw_done`, `wait_for_vblanks`, `cleanup_planes`. Unlike vc4 it does not
  call `wait_for_flip_done`; what `wait_for_vblanks` waits for was not read. (read from source:
  `drm_atomic_helper.c: drm_atomic_helper_commit_tail_rpm`)
- `vop2_crtc_atomic_disable` sends a pending `crtc->state->event` itself when the CRTC becomes
  inactive. (read from source: `rockchip_drm_vop2.c: vop2_crtc_atomic_disable`)
- `vp->event` is a single slot, like vc4's. (read from source: `vop2_crtc_atomic_flush`; safe by
  the same core queue argument, inferred)

### 3.3 Does an overlay-only commit get an event?

**Yes.** Overlay-only commits put the CRTC in the state (1.3), `vop2_crtc_atomic_flush` is called
for every CRTC in the state and always writes `CFG_DONE` and arms `vp->event`, whatever planes
changed. (read from source: `rockchip_drm_vop2.c: vop2_crtc_atomic_flush`;
`drm_atomic_helper.c: drm_atomic_helper_commit_planes`; `ACTIVE_ONLY` is set by `commit_tail_rpm`
and skips CRTCs that are not active, which cannot carry an event, see 1.2)
That is the answer to RK Q9's overlay question for the VP the overlay is on: one per-CRTC event
per commit, for the Cluster and the Esmart planes alike (the code has no plane-type branch).
Per-VP reach of a plane (a plane that cannot reach a VP) is not a source question here; see the
device-tree item `rk3588-dt-source`.

## 4. Mapping onto the engine contract

What the engine requires (read from source: this repository at `2e458d2`):

- `Capabilities::replace_pending` in `crates/composition-core/src/caps.rs`: "Whether a new commit
  may replace one that has not been latched yet. If `false`, committing while a submission is
  pending fails as busy." Default `false`.
- `Engine::commit_untraced` in `crates/composition-core/src/engine.rs` reads the flag and calls
  `check_submit(output, buffers, replace)`; with `false` and a pending submission on the output
  it fails before the backend is asked. With `true` the old submission is dropped (a `Dropped`
  event) unless the backend reports it latched first.
- `DisplayBackend::commit` in `backend.rs`: success means the hardware accepted the configuration,
  not that it is on screen; `BackendEvent::Latched { output, submission }` follows once it latched;
  a replaced submission must never be reported latched, except when the hardware latched it
  before the replacing commit took it over.
- The DRM backend turns `EBUSY` into `BackendError::Busy` and a page-flip event for a CRTC into
  `Latched` for that output's single pending submission, with the event's sequence and timestamp
  (`crates/composition-drm/src/lib.rs`, the event reader and `fail`).

### 4.1 `replace_pending` per profile

| Profile | `replace_pending` | Tag | Why |
|---------|-------------------|-----|-----|
| `DriverProfile::Generic` (any DRM driver) | `false` | read from source (core queue); the value is inferred | The core helper queue returns `-EBUSY` or stalls rather than replacing (1.1). `false` is what the planner needs when the driver is unknown. |
| `DriverProfile::Rk3588Vop2` | `false` | read from source (`rockchip_drm_fb.c`, `rockchip_drm_vop2.c` at `v6.18`); the value is inferred | Generic helper commit, no async path, one `vp->event` slot (3.1, 3.2). |
| `DriverProfile::Vc4Hvs5` | `false` | read from source (`vc4_kms.c`, `vc4_crtc.c`, `vc4_hvs.c` at `rpi-6.18.y`); the value is inferred | Generic helper commit plus the per-channel `pending_commit` wait, one `vc4_crtc->event` slot (2.1, 2.2). |
| `DriverProfile::Vc4Hvs6` | `false` | read from source (same files); the value is inferred | Same code as HVS5 for these functions; the only difference read is the active-dlist register (`SCALER6_DISPX_DL.LACT`), which does not change the answer. |

`true` is wrong for all four: with `true` the engine would commit while the kernel's previous
commit is still queued, and the kernel would answer `-EBUSY` (nonblocking), which the engine then
sees as `Busy` after it already counted the old submission as replaced. The profiles must not
set it. (inferred)

Not a profile value, but a note for the engine's retry: with `false`, the engine's own check
refuses a second commit while an output has a pending submission, so the kernel's `-EBUSY` is only
reached when the kernel's view lags the engine's. It cannot lag after the event: the event
completes `flip_done` in the call that queues it (1.4). It can still occur for a CRTC that shares
a commit with another one, because the plane and connector loops of `setup_commit` and vc4's
per-channel `pending_commit` look at other CRTCs' commits (2.1). (inferred)

### 4.2 `Latched` per profile

| Profile | When `Latched` is correct to report | Tag |
|---------|-------------------------------------|-----|
| `Generic` | On the page-flip event of the CRTC. The core does not promise that the event is at the latch; only that the driver arms or sends it (1.4). | read from source (core); conclusion inferred |
| `Rk3588Vop2` | On the page-flip event. It is sent at the first `FS_FIELD` at which the VP's `CFG_DONE` bit is clear, so at or after the hardware took the configuration, never before. | read from source (3.2); "at or after the latch" is inferred |
| `Vc4Hvs5`, `Vc4Hvs6` | On the page-flip event. It is sent at the first front-porch interrupt at which the active dlist register equals the new dlist, so at or after the HVS used the new dlist, never before; the new buffers may have been scanned out up to one frame before the event's timestamp. | read from source (2.2); the one-frame lag is inferred |

For all of them the engine's rule "never report a replaced submission as latched" is satisfied
trivially, because nothing is replaced. An overlay-only commit gets its own event on all three
(2.3, 3.3), so the engine does not need a "no event, treat as latched at commit" special case for
overlay-only frames. (read from source; the engine consequence is inferred)

### 4.3 What the board items should check

These are the source's claims; the sweeps decide them. (inferred: planning, not a result)

1. Second `NONBLOCK` commit issued before the first event was read: expect `-EBUSY` on all
   three, also with only an overlay plane in the second commit.
2. A blocking second commit in the same situation: expect it to return after the first flipped
   (no error).
3. Overlay-only commit with `PAGE_FLIP_EVENT`: expect one event for the CRTC.
4. Event sequence and timestamp against the vblank count: expect at or after the commit's
   vblank, for vc4 possibly one frame after the first scanned-out frame of the new buffer.
5. `TEST_ONLY | PAGE_FLIP_EVENT`: expect `-EINVAL` from the core before any driver code runs.

## 5. Not covered

- Mainline `vc4` (against `rpi-6.18.y`): `vc4-mainline-source`.
- The RK3588 vendor kernel (`rockchip-linux/kernel`, which has its own `rockchip_drm_vop2.c`
  with different commit code): `vop2-vendor-source`.
- `drm_atomic_helper_wait_for_vblanks` (behaviour of the `commit_tail_rpm` wait) and
  `drm_mode_cursor_universal`.
- Other VOP2 SoCs: the file is the same for RK3568, RK3576 and RK3588; only RK3588 is a target.
- Anything about hardware: underrun, bandwidth, and when in a frame the hardware latches.
