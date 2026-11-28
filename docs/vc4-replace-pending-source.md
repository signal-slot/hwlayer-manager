# vc4: a second commit while the first is pending (HVS question 6), from source

Status: source read, written 2026-11-28. **No hardware result is claimed.** Nothing was run on a
board. This is the prediction that the `pending_probe` runs in the `rpi5-hvs6-session` (HVS6,
BCM2712) and the `cm4-hvs5-session` (HVS5, BCM2711) must confirm or refute.

It answers question 6 of [`hvs-backend-design.md`](hvs-backend-design.md) §7 (`replace_pending`:
can a commit that has not latched be replaced, and what is reported).

## Tags and pins

- **read from source**: read in the tree below, with file:line.
  - Kernel `rpi-6.12.y`: tree `raspberrypi/linux`, branch `rpi-6.12.y`, commit
    `8f5f2e15840330091c7116b2ef9454108d4bc423` (shallow checkout of `drivers/gpu/drm` and
    `include/drm`, read 2026-11-28). Line numbers below are from this checkout and are exact.
  - Kernel `rpi-6.18.y`: tree `raspberrypi/linux`, branch `rpi-6.18.y`, commit
    `16ae2649dcfadf9d81445b1177a3191efa3cb82e`. The local checkout of this commit had no files,
    so the `vc4_*.c` files were read through a web fetch of
    `https://raw.githubusercontent.com/raspberrypi/linux/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4/vc4_kms.c`
    (and `vc4_crtc.c`, `vc4_hvs.c`, `vc4_plane.c` at the same commit). The line numbers given for
    it are approximate (about 3 lines either way). `vc4_plane.c` was only partly readable
    there: its async check and update are **inferred** to match 6.12.
  - `drm_atomic_helper.c` at 6.18 was not read in full; the helper statements for 6.18 are
    **inferred** from 6.12 (generic DRM core code, same stall and async checks in both
    lineages).
- **inferred**: a conclusion of this note. A line without a tag is inferred.
- Board facts (the boards' kernel versions) are in
  [`observed/bcm2712-topology.md`](observed/bcm2712-topology.md) and are not repeated here.
  Whether the boards run these exact kernels is HVS question 7 and is not decided here.

## 1. How the engine models `replace_pending`

All of this is read from the repository on 2026-11-28 (inferred as far as it concerns the HVS).

- `Capabilities::replace_pending` (`crates/composition-core/src/caps.rs:172`): "whether a new
  commit may replace one that has not been latched yet. If `false`, committing while a submission
  is pending fails as busy."
- `Presentation::check_submit` (`crates/composition-core/src/presentation.rs:331`): a slot with a
  pending submission and `replace_pending == false` gives `PresentError::Busy(output)`.
  With `replace_pending == true` the pending submission is replaced and reported as
  `DisplayEvent::Dropped` (`presentation.rs:357-359`).
- `Engine::commit` (`crates/composition-core/src/engine.rs:310`) takes the flag from
  `self.caps.replace_pending` and calls `check_submit` before asking the backend to commit.
- `crates/composition-drm/src/profile/vc4.rs:74` leaves the flag "as reported (unknown:
  inferred)": no profile claims `true`; the default is `false` (`caps.rs:208`).

So the engine's question is: when userspace commits again before the first commit's flip event,
does the kernel **accept** the second commit (then `true`, with a Dropped for the first), or
refuse it (then `false`, the engine reports Busy)?

## 2. What the kernel does (read from source)

### 2.1 The second nonblocking atomic commit is refused with -EBUSY

`rpi-6.12.y` `8f5f2e1`, `drivers/gpu/drm/`:

- `vc4_kms.c:1133-1141`: both the vc4 and the vc5 mode-config funcs use
  `.atomic_commit = drm_atomic_helper_commit`; the commit hooks are
  `.atomic_commit_setup = vc4_atomic_commit_setup` and
  `.atomic_commit_tail = vc4_atomic_commit_tail` (`vc4_kms.c:1128-1129`). So the generic
  helper's nonblocking machinery applies unchanged.
- `drm_atomic_helper.c:2049` (`drm_atomic_helper_commit`): `drm_atomic_helper_setup_commit(state,
  nonblock)` runs first for every non-async commit.
- `drm_atomic_helper.c:2322` calls `stall_checks()` for each CRTC in the state.
  `stall_checks` (`drm_atomic_helper.c:2162-2209`): if the newest commit on the CRTC has not
  completed its `flip_done` and the new commit is nonblocking, it returns `-EBUSY`
  (`drm_atomic_helper.c:2173-2184`, message "busy with a previous commit"). The comment at
  `:2174-2175` says "Userspace is not allowed to get ahead of the previous commit with
  nonblocking ones."
- The same refusal exists per plane (`drm_atomic_helper.c:2382-2389`) and per connector
  (`drm_atomic_helper.c:2360-2367`): a nonblocking commit that touches a plane or connector whose
  previous commit has not completed `flip_done` returns `-EBUSY`.
- `flip_done` completes when the CRTC's vblank event is sent: `setup_commit` ties the event to
  `commit->flip_done` (`drm_atomic_helper.c:2345`), and vc4 sends it from
  `vc4_crtc_handle_page_flip` (`vc4_crtc.c:832-858`, the send at `:853`) in the vblank interrupt
  (`vc4_crtc_handle_vblank`, `vc4_crtc.c:870-881`) once the hardware's active display list equals
  the one this commit installed (`vc4_crtc.c:851-852`: `current_dlist == vc4_crtc->current_dlist`).
  The latter is stored by `vc4_hvs_update_dlist` (`vc4_hvs.c:1177-1204`; the event is stored at
  `:1192`, `current_dlist` at `:1202`).

Result: **a second nonblocking atomic commit that touches the same CRTC (or a plane or connector
of it) before the first commit's flip event is refused at the ioctl with `-EBUSY`.** Nothing is
queued and nothing is dropped; the first commit still completes and sends its event.

### 2.2 A blocking commit waits, it does not replace

- `drm_atomic_helper.c:2178` returns `-EBUSY` only `if (!completed && nonblock)`. A blocking
  (no `DRM_MODE_ATOMIC_NONBLOCK`) commit goes on.
- vc4 adds its own serialisation per HVS channel: `vc4_atomic_commit_setup` stores
  `fifo_state[channel].pending_commit = drm_crtc_commit_get(crtc_state->commit)` for every CRTC in
  the state whose channel is in use (`vc4_kms.c:536-550`), and `vc4_atomic_commit_tail` begins by
  waiting on the previous commit of each in-use channel with `drm_crtc_commit_wait`
  (`vc4_kms.c:422-439`).
- So a blocking second commit is delayed until the first has latched and then applied in order.
  Both frames are shown, and each commit's flip event is delivered. That is not
  `replace_pending`: nothing is replaced.

### 2.3 The in-place async paths do not apply to an atomic overlay commit

- The core decides async update only for the legacy cursor path: `drm_atomic_helper.c:1117-1118`
  (`if (state->legacy_cursor_update) state->async_update = !drm_atomic_helper_async_check(...)`).
  `legacy_cursor_update` is set only in `drm_atomic_helper_update_plane` /
  `drm_atomic_helper_disable_plane` when the plane is the CRTC's cursor plane
  (`drm_atomic_helper.c:3205-3206`, `:3243-3244`). The atomic ioctl does not set it (it is not
  assigned in `drm_atomic_uapi.c`). So an atomic commit never takes the in-place path.
- `drm_atomic_helper_async_check` (`drm_atomic_helper.c:1895-1963`) refuses: a state with a
  modeset (`:1906-1909`), more than one plane (`:1915`, "only single plane async updates are
  supported"), a CRTC change (`:1921-1927`), a fence (`:1937-1942`), and a plane whose previous
  commit has not reached `hw_done` (`:1949-1955`, `-EBUSY`, "inflight previous commit preventing
  async commit"). A refusal means the commit goes through the normal path, i.e. section 2.1/2.2.
- vc4 additionally forces the normal path when it can: `vc4_atomic_commit_setup` sets
  `state->legacy_cursor_update = false` unless firmware KMS is in use (`vc4_kms.c:524-530`), "to
  prevent `drm_atomic_helper_setup_commit()` from auto-completing `commit->flip_done`". This is on
  the non-async path only; the async decision was already taken in the check
  (`drm_atomic_helper.c:1117`), before `setup_commit`.
- vc4's plane hooks do implement the in-place update: `.atomic_async_check` and
  `.atomic_async_update` are set for both plane helper funcs (`vc4_plane.c:2639-2653`).
  `vc4_plane_atomic_async_check` (`vc4_plane.c:2561-2607`) allows the change only if the display
  list has the same length and offsets and differs only in the `pos0`, `pos2` and `ptr0` words
  (and the LBM word): position and framebuffer address, not size, format, scale or alpha;
  otherwise `-EINVAL` (comment at `:2591-2593`: "if anything else has changed, fallback to a
  sync update"). `vc4_plane_atomic_async_update` (`vc4_plane.c:2487-2559`) writes those three
  words straight into the hardware display list (`:2551-2556`). No flip event is produced and
  nothing is pending: `drm_atomic_helper_commit` returns right after
  `drm_atomic_helper_async_commit` (`drm_atomic_helper.c:2038-2047`), without `setup_commit`.
- In practice this path is the **legacy cursor** ioctl (a cursor-plane move or image change); it
  replaces in place, at once, and reports nothing. An overlay plane is not reached by it from the
  atomic ioctl.

### 2.4 DRM_MODE_PAGE_FLIP_ASYNC exists on these kernels

- `vc4_kms.c:1184` (6.12) sets `dev->mode_config.async_page_flip = true` unconditionally; in the
  6.18 fetch it is also unconditional (`vc4_kms.c`, about line 1215).
- Atomic ioctl: the flag is accepted when `async_page_flip` is set
  (`drm_atomic_uapi.c:1443-1451`; otherwise `-EINVAL` at `:1444-1448`). With it, only the primary
  plane's `FB_ID`, `IN_FENCE_FD` and `FB_DAMAGE_CLIPS` may change; any other property must keep
  its value (`drm_atomic_uapi.c:1106-1112`), and a CRTC or connector property must keep its value
  (`:1084-1089`, `:1062-1067`). `drm_atomic_helper.c:3656` copies the flag into
  `crtc_state->async_flip` for the legacy flip path. `vc4/*.c` (6.12) does not read
  `crtc_state->async_flip`, so for an atomic commit the flag does not by itself change what vc4
  does: **inferred**, from the absence of a use in the driver. It is not a way to replace a
  pending commit and is not used by composition-drm.
- Legacy page-flip ioctl: `vc4_page_flip` (`vc4_crtc.c:1105-1122`) with the flag calls
  `vc5_async_page_flip` for gen 5 and 6 (`vc4_crtc.c:1115-1116`, which is
  `vc4_async_page_flip_common`, `:1097-1103`). That sets the new fb in the plane state, waits for
  the buffer's dma-resv read fence, then writes the plane address (`vc4_plane_async_set_fb`,
  called at `vc4_crtc.c:921`) and sends the event **immediately, not at vblank**
  (`vc4_crtc.c:922-928`). It takes no `flip_done` and queues nothing behind a pending atomic
  commit (it does not go through `setup_commit`), so it bypasses the -EBUSY check. It also races
  with a pending commit's display list: **inferred**, not tested. `vc4_async_page_flip` (the
  VC4_GEN_4 variant) returns `-ENODEV` on gen 5 and 6 (`vc4_crtc.c:1072-1073`).

## 3. Per kernel and per HVS

The commit path is the same code for HVS5 and HVS6. The gen switch is in three places only:
the core-clock handling (gen 5 only, `vc4_kms.c:441-455`, `:496-512`), the PV muxing commit
(`vc4_kms.c:462-481`), and the active-display-list read that decides when the event is sent
(`vc4_crtc.c:845-849`: `SCALER_DISPLACTX(chan)` below gen 6, `SCALER6_DISPX_DL(chan)` field
`LACT` from gen 6). None of them changes the stall, EBUSY or serialisation behaviour.

| Kernel | HVS | 2nd nonblocking atomic commit before the flip event | 2nd blocking atomic commit | Legacy cursor move | Legacy async page flip | `replace_pending` the source predicts |
| --- | --- | --- | --- | --- | --- | --- |
| `rpi-6.12.y` `8f5f2e1` (read from source) | HVS5 (BCM2711, gen 5) | `-EBUSY` at the ioctl; first commit unaffected | waits for the first to latch, then both are shown, two events | in place, no event, only if no commit is in flight on the plane and only `pos0/pos2/ptr0` change; else normal path | accepted, event at once | `false` |
| `rpi-6.12.y` `8f5f2e1` (read from source) | HVS6 (BCM2712, gen 6) | the same | the same | the same | the same | `false` |
| `rpi-6.18.y` `16ae2649` (read from source for vc4, approximate lines; helper and plane async check inferred from 6.12) | HVS5 | the same (inferred for the helper) | the same | the same | the same | `false` |
| `rpi-6.18.y` `16ae2649` (as above) | HVS6 | the same (inferred for the helper) | the same | the same | the same | `false` |

If the second commit is refused, the engine's `Capabilities::replace_pending == false` is the
matching value: `Engine::commit` returns Busy before the backend is asked, and no `Dropped` event
exists. A backend that would set `true` would have to turn `-EBUSY` into a replacement itself,
for example by waiting for the flip event; that is a different behaviour from "replaced while
pending" and this source does not support it.

What the backend sees: the `-EBUSY` comes back from the commit ioctl, not as a later event
(inferred: the check runs in `drm_atomic_helper_setup_commit`, which runs inside
`drm_atomic_helper_commit`, `drm_atomic_helper.c:2049-2051`). The first commit's event arrives at
the next vblank after the hardware switched to its display list; on HVS6 that comparison reads
the `LACT` field (`vc4_crtc.c:845-849`), which is the part of the prediction that differs from
HVS5 and is the least certain (inferred).

## 4. What the `pending_probe` must show

The measurement is the `pending_probe` run in the `rpi5-hvs6-session` (HVS6) and in the
`cm4-hvs5-session` (HVS5), both items of plan #467; the tool is the `pending-probe-tool` item.
Neither result exists when this note is written. The prediction to confirm or refute:

1. Commit A nonblocking with an event, then commit B nonblocking on the same CRTC before A's
   event: **B returns `-EBUSY`**; A's event still arrives once. If B is accepted instead, this
   note is wrong for that kernel; check that the board's kernel is the one read here (HVS
   question 7) before changing the model.
2. Same with B blocking: B returns after A latched; two events, in order.
3. Legacy cursor move before A's event: if a cursor plane is present, expected to wait or fall
   back to the normal path, not to replace A (the in-place path needs no commit in flight,
   `drm_atomic_helper.c:1949-1955`).
4. Counts to record per run: the errno of B, flip events received, time from B to return.

If all four match, `replace_pending` stays `false` for HVS5 and HVS6 and
`crates/composition-drm/src/profile/vc4.rs` needs no change (its comment already says "as
reported"); the design question can be marked answered by source plus measurement.

## 5. Not covered

- Firmware KMS (`vc4-fkms-v3d`, `firmware_kms` in the code above): out of scope as in the design.
- The TXP/writeback path (`feeds_txp`, `vc4_crtc.c:852`): not used by composition-drm.
- Any timing claim. The source gives ordering, not latency.
