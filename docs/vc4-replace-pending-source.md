# vc4: a second commit while the first is pending (HVS Q6), read from source

Status: source read, written 2026-11-28. **No hardware result is claimed.** Nothing here was run on a
board. This answers, from the driver and the DRM core, question 6 of
[`hvs-backend-design.md`](hvs-backend-design.md) §7: may a commit that has not latched be replaced,
and what is reported. The measurement is the `pending_probe` run in the `rpi5-hvs6-session` (HVS6,
BCM2712) and the `cm4-hvs5-session` (HVS5, BCM2711); this file only states what those runs should
find.

## Tags

- **read from source**: `raspberrypi/linux`, with a file and line. Two trees were read on 2026-11-28:
  - `rpi-6.18.y` at commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e` (written "6.18" below);
  - branch `rpi-6.12.y`, fetched by branch name, whose tip a moment later was
    `8f5f2e15840330091c7116b2ef9454108d4bc423` (written "6.12" below). The line numbers of 6.12 are
    for the file as fetched, which is that tip unless the branch moved between the two requests.
- **inferred**: a conclusion drawn from those lines, not itself a line of source.
- Paths: `drm/` is `drivers/gpu/drm/`; `vc4/` is `drivers/gpu/drm/vc4/`.
- Which kernel each board runs is a separate question (HVS Q7, not answered here); this file does not
  say that a board runs either tree.

## 1. How the engine models it

All read from this repository on 2026-11-28 (a statement about the engine's contract, not the HVS):

- `Capabilities::replace_pending` is read once per commit at `crates/composition-core/src/engine.rs:310`
  and passed to `Presentation::check_submit` / `submit`.
- With the flag false and a submission still pending on that output, the core refuses before the
  backend is asked: `PresentError::Busy` (`crates/composition-core/src/presentation.rs:331`).
- With the flag true, the pending submission is replaced and reported as `DisplayEvent::Dropped`
  (`presentation.rs:357-366`, doc comment).
- The vc4 profile does not decide the flag; it "stays as reported (unknown: inferred)"
  (`crates/composition-drm/src/profile/vc4.rs:74`).

So the question for the board is exactly: does the kernel accept a second nonblocking commit on the
same CRTC between the first commit's return and its flip event?

## 2. Second nonblocking atomic commit before the first flip event

**Prediction: the kernel refuses it with `-EBUSY`, on both kernels and both generations. It is not
queued and not replaced.** (inferred from the lines below)

- `drm_atomic_helper_commit` calls `drm_atomic_helper_setup_commit(state, nonblock)` for every
  non-async-update commit (read from source: 6.18 `drm/drm_atomic_helper.c:2257`; 6.12 same function,
  `drm/drm_atomic_helper.c` near 2060, read by name).
- `setup_commit` calls `stall_checks(crtc, nonblock)` per CRTC. For the newest commit on the CRTC's
  `commit_list`, if its `flip_done` has not completed and the new commit is nonblocking, it returns
  `-EBUSY` ("busy with a previous commit"). Read from source: 6.18 `drm/drm_atomic_helper.c:2370-2395`;
  6.12 `drm/drm_atomic_helper.c:2162-2184` (the `-EBUSY` at 2184).
- The same refusal exists per connector and per plane, if the old state's commit has not completed
  `flip_done` (read from source: 6.18 `drm/drm_atomic_helper.c:2569-2574` and `2591-2596`; 6.12
  `2366` and `2388` by the `-EBUSY` lines). A second commit that touches only other planes of the
  same CRTC is still refused by the CRTC check (inferred: `stall_checks` is keyed on the CRTC, and a
  plane-only commit pulls its CRTC into the state).
- The driver does not override it. `vc4_atomic_commit_setup` (read from source: 6.18
  `vc4/vc4_kms.c:545-583`; 6.12 `vc4/vc4_kms.c:515`) runs after the helper checks, forces
  `legacy_cursor_update = false`, and records the commit as the channel's `pending_commit`; it
  returns no error of its own.
- A **blocking** commit is not refused: `stall_checks` only returns `-EBUSY` when `nonblock` is set.
  `vc4_atomic_commit_tail` waits for the previous commit of every in-use HVS channel with
  `drm_crtc_commit_wait` (read from source: 6.18 `vc4/vc4_kms.c:455-473`). (inferred: it returns
  after the first flip event, with no replacement.)
- `TEST_ONLY` does not go through `setup_commit` (inferred: the uapi calls `drm_atomic_check_only`,
  read from source: 6.18 `drm/drm_atomic_uapi.c:1579` region, by the `DRM_MODE_ATOMIC_TEST_ONLY`
  branch following line 1576). A `TEST_ONLY` while a commit is pending should therefore succeed.

What is reported: the ioctl returns `-1` with `errno = EBUSY` (inferred; the helper returns `-EBUSY`
and the atomic ioctl passes it up). The first commit is unaffected and still produces its event
(inferred).

## 3. When the first commit's flip event arrives

This fixes what "latched" means for the probe.

- On a CRTC with a new state event, `vc4_hvs_update_dlist` stores it as `vc4_crtc->event` and takes a
  vblank reference (read from source: 6.18 `vc4/vc4_hvs.c:1218-1228`; 6.12 `vc4/vc4_hvs.c:1191`).
- The event is sent from the vblank interrupt, only when the HVS channel's active display list equals
  the one the commit wrote: `vc4_crtc->current_dlist == current_dlist` (read from source: 6.18
  `vc4/vc4_crtc.c:863`; 6.12 `vc4/vc4_crtc.c:852`). The register read differs by generation: HVS5
  reads `SCALER_DISPLACTX(chan)`, HVS6 reads `SCALER6_DISPX_DL_LACT` (read from source: 6.18
  `vc4/vc4_crtc.c:857-861`). `vc4_crtc_handle_vblank` calls the page-flip handler on every vblank
  (read from source: 6.18 `vc4/vc4_crtc.c:882-893`).
- So the flip event is the latch, one or more vblanks after the commit returns. If the display list
  has not become active at a vblank, the event waits for the next one (inferred). If the CRTC
  stops, the CRTC-disable path sends the event (read from source: 6.18 `vc4/vc4_crtc.c:625-636`,
  `664`).
- The commit's `flip_done` completes when that event is delivered (read from source: 6.18
  `drm/drm_atomic_helper.c:2553`, `new_crtc_state->event->base.completion = &commit->flip_done`).
  That completion is what `stall_checks` tests, which closes the loop: the refusal in section 2
  lasts exactly until the flip event of the first commit.

## 4. Legacy cursor and async plane updates: replaced in place

**Prediction: a legacy cursor-plane update (`DRM_IOCTL_MODE_CURSOR` / `MODE_CURSOR2`, and the
legacy `SETPLANE` helper path when the plane is the CRTC's cursor) is applied in place to the live
display list when only position and framebuffer address change, with no flip event.** (inferred)

- The legacy plane helpers set `legacy_cursor_update = true` when the plane is `crtc->cursor` (read
  from source: 6.18 `drm/drm_atomic_helper.c:3414` and `3452`; 6.12 `3206` and `3244`). The atomic
  ioctl does not set it (inferred: no other assignment found in `drm_atomic_uapi.c`).
- In `drm_atomic_helper_check`, if `legacy_cursor_update`, `state->async_update` is set to
  `!drm_atomic_helper_async_check()` (read from source: 6.18 `drm/drm_atomic_helper.c:1130-1131`;
  6.12 `1118`). `drm_atomic_helper_commit` then skips `setup_commit` and calls
  `drm_atomic_helper_async_commit` (read from source: 6.18 `drm/drm_atomic_helper.c:2246-2251`). So
  `stall_checks` is not run and no commit object is created: there is no `-EBUSY` from section 2
  and no flip event (inferred).
- `drm_atomic_helper_async_check` refuses with `-EBUSY` while an earlier commit that modifies the
  plane has not reached `hw_done` (read from source: 6.18 `drm/drm_atomic_helper.c:2158-2162`; 6.12
  `1949-1950`). The ioctl then continues on the normal path (inferred: `async_update` stays false),
  which for a legacy (blocking) call waits instead of failing.
- The vc4 plane hook accepts the update only if the display-list layout is unchanged and every word
  except pos0, pos2, ptr0 (and the LBM word) is equal; otherwise `-EINVAL` and the normal path (read
  from source: 6.18 `vc4/vc4_plane.c:2667-2715`; 6.12 `vc4/vc4_plane.c:2561-2590`). The update
  writes those three words into the hardware display list (`writel`) (read from source: 6.18
  `vc4/vc4_plane.c:2593-2665`). Gen 5 uses `vc4_plane_mode_set`, gen 6 `vc6_plane_mode_set` for the
  check (read from source: 6.18 `vc4/vc4_plane.c:2677-2680`).
- Hook table: both the primary/overlay and the cursor `drm_plane_helper_funcs` list
  `atomic_async_check` / `atomic_async_update` (read from source: 6.18 `vc4/vc4_plane.c:2750-2758`;
  6.12 `2644-2652`). A cursor plane exists on each CRTC (read from source: 6.18 `vc4/vc4_plane.c:2991`).
- Consequence for the engine (inferred): in-place replacement exists only for the cursor plane via
  legacy calls and only for position or buffer-address changes. It is not a way to replace a pending
  multi-plane atomic commit, and the engine's atomic commits never take this path.

## 5. DRM_MODE_PAGE_FLIP_ASYNC

**Prediction: it exists on both kernels, for gen 4, 5 and 6, and is not what the engine uses.**

- `dev->mode_config.async_page_flip = true` (read from source: 6.18 `vc4/vc4_kms.c:1215`; 6.12
  `vc4/vc4_kms.c:1184`). The generic legacy ioctl accepts the flag only if that is set (read from
  source: 6.18 `drm/drm_plane.c:1408`; 6.12 `drm/drm_plane.c:1381`), and so does the atomic ioctl
  (read from source: 6.18 `drm/drm_atomic_uapi.c:1466-1473`; 6.12 `1443`).
- Legacy page flip with the flag: `vc4_page_flip` calls `vc5_async_page_flip` for gen above 4 and
  `vc4_async_page_flip` for gen 4 (read from source: 6.18 `vc4/vc4_crtc.c:1120-1127`; 6.12
  `vc4/vc4_crtc.c:1111`). Both end in `vc4_async_page_flip_common`, which waits for the buffer's
  fence, then changes the plane's framebuffer address and sends the event
  (`vc4_async_page_flip_complete`, read from source: 6.18 `vc4/vc4_crtc.c:923-938`). It takes a
  vblank reference, and the event is sent when the fence signals, not at a vblank (inferred from
  `vc4_crtc.c:934`, `1050`). It does not go through `stall_checks` (inferred: it bypasses
  `drm_atomic_helper_page_flip`, line 1129 is the only helper call).
- Atomic with the flag: the core restricts the changeable properties to the primary plane's FB_ID,
  IN_FENCE_FD and FB_DAMAGE_CLIPS; anything else is rejected (read from source: 6.12
  `drm/drm_atomic_uapi.c:1106-1110`, same pattern at 6.18 `1113`). (inferred: so an overlay
  cannot be moved or replaced this way.)
- Whether the flag is accepted for a given plane by a kernel that runs on a board is a board
  question; `pending_probe` need only record the errno.

## 6. Expected `replace_pending`

| | HVS5 (BCM2711) | HVS6 (BCM2712) |
|---|---|---|
| second nonblocking atomic commit before flip event, kernel 6.12 | `-EBUSY` | `-EBUSY` |
| same, kernel 6.18 | `-EBUSY` | `-EBUSY` |
| blocking commit in that window | waits, no replace | waits, no replace |
| `TEST_ONLY` in that window | accepted | accepted |
| legacy cursor update in that window | in place if hw_done reached, else normal path | same |
| `DRM_MODE_PAGE_FLIP_ASYNC` legacy flip | accepted | accepted |
| **expected `Capabilities::replace_pending`** | **false** | **false** |

All rows are inferred from the source lines above; the refusal is DRM-core code shared by every
generation, so the generations differ only in how the latch is detected (section 3), not in the
result. With `false` the engine itself returns `PresentError::Busy` before the kernel is asked, so
the kernel's `-EBUSY` should never be seen by a correct engine (inferred). This matches the
hvs-backend-design note that the commit-ordering row suggests `false`.

## 7. What the board runs should confirm or refute

For each of `rpi5-hvs6-session` and `cm4-hvs5-session`, the `pending_probe` runs are the measurement.
A row of this file is confirmed or refuted by:

1. Nonblocking commit A returns; nonblocking commit B on the same CRTC is submitted before A's event:
   expect `EBUSY`. Anything else (success with A's event never arriving, or two events) refutes
   section 2 and would make `replace_pending = true` worth measuring further.
2. After A's event, B succeeds, and A's flip count is one.
3. A `TEST_ONLY` commit between A and its event returns success.
4. A legacy cursor move between A and its event: record the result and whether any event arrives.
5. The kernel version string, so the result is matched to the 6.12 or 6.18 column (HVS Q7).

Until those runs exist, HVS Q6 is **not answered**: this file gives the source's prediction, `false`
for both, and no hardware result.

## 8. Not covered

- Whether a commit that has not latched can be dropped by the kernel on a CRTC disable or modeset
  (the disable path sends the event; what the engine would call that is the backend's decision).
- HVS channel assignment and FIFO sharing between CRTCs. `vc4_atomic_commit_tail` waits on the
  previous commit of each in-use channel (read from source: 6.18 `vc4/vc4_kms.c:455-473`), which
  could delay a commit on one CRTC behind another CRTC that shares a channel's state; this was not
  traced further (inferred only).
- Firmware KMS (`vc4->firmware_kms`) takes other branches in `vc4_kms.c`; not read.
