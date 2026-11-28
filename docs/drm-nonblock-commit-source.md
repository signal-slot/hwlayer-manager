# DRM atomic helper: a second nonblocking commit while one is pending, from source

Status: source read, written 2026-11-28. **No hardware result is claimed.** Nothing was run on a
board. It closes the gap left by [`rk3588-vop2-flip-source.md`](rk3588-vop2-flip-source.md) and
complements [`vc4-replace-pending-source.md`](vc4-replace-pending-source.md): both stop at the
generic helper, and this note reads it.

## Tags and pins

- **read from source**: read in Linux tag `v6.18`, file `drivers/gpu/drm/drm_atomic_helper.c`,
  fetched from
  `https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/plain/drivers/gpu/drm/drm_atomic_helper.c?h=v6.18`
  (read 2026-11-28 UTC). Line numbers below are exact for that file.
- **inferred**: a conclusion of this note. A line without a tag is inferred.
- **read from source**: also `drivers/gpu/drm/drm_atomic_uapi.c` and `include/drm/drm_atomic.h`
  at `v6.18`, same tag and host (paths `drivers/gpu/drm/drm_atomic_uapi.c` and
  `include/drm/drm_atomic.h` with `?h=v6.18`), read 2026-11-28 UTC.
- **Not read**: `drivers/gpu/drm/drm_atomic.c` at `v6.18` (the fetch was refused). Nothing below
  is claimed from it. What it decides is listed under "Open".

## What a nonblocking second commit gets: -EBUSY

All references are in `drm_atomic_helper.c` at `v6.18`.

- **read from source**: `drm_atomic_helper_commit()` (line 2166) passes its `nonblock` argument to
  `drm_atomic_helper_setup_commit()` (call at line 2183, definition at 2432) and returns its error
  at once, before `drm_atomic_helper_swap_state()`. The kernel-doc of `setup_commit` says
  "-EBUSY when userspace schedules nonblocking commits too fast" (line 2429).
- **read from source**: for each CRTC in the new state, `setup_commit` calls
  `stall_checks(crtc, nonblock)` (line 2456; function at 2296). `stall_checks` reads the first
  entry of `crtc->commit_list` (loop at line 2305), the newest earlier commit. It returns
  **-EBUSY** (line 2318) when `try_wait_for_completion(&commit->flip_done)` is false (line 2307)
  **and** `nonblock` is true (line 2312). `flip_done` is the completion bound to that commit's
  vblank event (`new_crtc_state->event->base.completion = &commit->flip_done`, line 2485). So the
  condition is: an earlier commit on the same CRTC whose flip event has not been sent yet.
- **read from source**: two more -EBUSY sites with the same test (`nonblock` and the old state's
  commit `flip_done` not complete), keyed on the object in the new state:
  - connector: lines 2500 to 2506;
  - plane: lines 2522 to 2528, on `old_plane_state->commit`.

  **inferred**: a nonblocking commit that touches a plane or connector still owned by a pending
  commit is refused even when its CRTC differs, since these two loops do not compare CRTCs.
- **read from source**: the CRTC loop is `for_each_oldnew_crtc_in_state` (line 2447), so
  `stall_checks` runs for every CRTC present in the new state.
- **read from source**: the refusal is logged by `drm_dbg_atomic` as "busy with a previous commit"
  (lines 2314 to 2316, 2503, 2525), not as an error.
- **read from source**: in `drm_atomic_uapi.c`, `drm_mode_atomic_ioctl()` (line 1384) calls
  `drm_atomic_nonblocking_commit(state)` when `DRM_MODE_ATOMIC_NONBLOCK` is set (lines 1539 to
  1540), `drm_atomic_commit(state)` otherwise (line 1542), and `drm_atomic_check_only()` for
  `DRM_MODE_ATOMIC_TEST_ONLY` (lines 1537 to 1538). The only error it retries is -EDEADLK (line
  1548); any other `ret`, -EBUSY included, is returned at line 1560.
- **inferred**: so the -EBUSY from `stall_checks` reaches userspace unchanged, provided
  `drm_atomic_nonblocking_commit()` (declared at `drm_atomic.h` line 845; body in `drm_atomic.c`,
  not read) hands the driver's `atomic_commit` result back as is.
- **inferred**: the refusal happens before the state swap, so the pending commit is neither
  replaced nor cancelled. It keeps its event and completes normally. This is the answer for a
  driver that uses `drm_atomic_helper_commit`; a driver with its own `atomic_commit` hook (see the
  vc4 note) can differ.

## What a blocking second commit does

- **read from source**: with `nonblock` false, `stall_checks` does not return -EBUSY for the
  first entry (the `nonblock` test at line 2312 fails), and the connector and plane tests (lines
  2500, 2522) are skipped too.
- **read from source**: `stall_checks` then takes the second entry of the list (`i == 1`, lines
  2320 to 2321). It waits for that older commit's `cleanup_done` with
  `wait_for_completion_interruptible_timeout(..., 10*HZ)` (lines 2335 to 2336), logs an error on
  timeout (line 2338), and returns the negative errno if a signal interrupts it (line 2343). This
  wait also applies to a nonblocking commit that gets past line 2318, that is, one whose newest
  earlier commit already has `flip_done` complete.
- **read from source**: a blocking commit runs `commit_tail()` in the caller (the `else` at line
  2232; `queue_work` at line 2231 is the nonblocking branch). `commit_tail()` (line 1955) calls
  `drm_atomic_helper_wait_for_dependencies()` (line 1981; function at 2558, loops at lines 2569,
  2577 and 2585 over the old CRTC, connector and plane commits). It waits until the earlier
  commit is both committed to hardware and has sent its flip event. So a blocking second commit
  **waits** for the pending one to latch and does not fail with -EBUSY.
- **read from source**: a blocking commit also waits for the new framebuffers' fences before the
  state swap (lines 2193 to 2194); a nonblocking one defers that wait to `commit_tail()` (line
  1979).

## Legacy cursor updates

- **read from source**: inside this file `legacy_cursor_update` is set only in
  `drm_atomic_helper_update_plane()` (line 3308; lines 3345 to 3346, when the plane is the CRTC's
  cursor) and in `drm_atomic_helper_disable_plane()` (line 3365; lines 3383 to 3384). Both then
  call `drm_atomic_commit()`. These are the helpers behind the legacy cursor ioctls.
- **read from source**: the flag changes three things:
  - `setup_commit` gives the commit no event and completes `flip_done` at once (lines 2471 to
    2472), so a later nonblocking commit never stalls on a cursor update;
  - `drm_atomic_helper_check()` sets `state->async_update =
    !drm_atomic_helper_async_check(dev, state)` (lines 1125 to 1126). The check needs exactly
    one plane (lines 2048 to 2049), no modeset (line 2042) and no CRTC change (line 2056), and
    `drm_atomic_helper_commit()` then calls `drm_atomic_helper_async_commit()` (line 2113) and
    returns at line 2180, without calling `setup_commit`;
  - `drm_atomic_helper_wait_for_flip_done()` returns early (lines 1818 to 1819).
- **read from source**: on that fast path the one -EBUSY is in `drm_atomic_helper_async_check()`
  (line 2088): an earlier commit on the plane whose `hw_done` is not complete. It tests `hw_done`,
  not `flip_done`. The kernel-doc says an error from it only means the update is not done
  asynchronously and "the commit should be treated like a normal synchronous commit" (lines 2025
  to 2027). With `async_update` false, `drm_atomic_helper_commit()` goes on to `setup_commit`
  (line 2183). So on the legacy cursor path an async-check -EBUSY is not returned to the caller by
  this file.
- **read from source**: on that fallback `stall_checks` is still called (line 2456) before the
  legacy test (line 2471), and the plane loop (line 2522) still applies when `nonblock` is true.
  Whether `drm_atomic_commit()` passes `nonblock` true or false is in `drm_atomic.c` (not read).
- **read from source**: `drm_atomic_uapi.c` at `v6.18` contains no assignment to
  `legacy_cursor_update` or `async_update`. A cursor-only update through the **atomic** ioctl
  with `DRM_MODE_ATOMIC_NONBLOCK` therefore does not take the legacy path: `drm_mode_atomic_ioctl()`
  (lines 1539 to 1540) calls `drm_atomic_nonblocking_commit()` and the state has both flags false
  (`drm_atomic.h` lines 444 and 449 declare them as per-state bits). The header calls the legacy flag
  "thoroughly broken" and says "new users of this flag are not allowed" (`drm_atomic.h` lines 436
  to 443). `DRM_MODE_PAGE_FLIP_ASYNC` only calls `set_async_flip()` (line 1535; function at 1373), not
  the `async_update` hint.
- **inferred**: that cursor commit takes the normal `setup_commit` path with `nonblock` true and
  gets the -EBUSY rules above (CRTC, plane and connector tests), so a cursor-only nonblocking
  atomic commit while a flip is pending on its CRTC is refused like any other.

## Open (not answered here)

- What `drm_atomic_commit()` and `drm_atomic_nonblocking_commit()` in `drm_atomic.c` pass as
  `nonblock` to the driver's `atomic_commit`, and whether they return its result unchanged.
- Which target-board drivers use `drm_atomic_helper_commit` unmodified: see the two linked notes.
  Only a `pending_probe` run on a board can say what a board returns.
