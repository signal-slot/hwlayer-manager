# composition-drm: from commit to Latched, and a second commit while one is pending

Status: source read, written 2026-11-28 from the tree at commit `82126a216d28`. **No hardware result
is claimed.** Nothing was run on a board. This note describes this repository's own DRM backend so
that the `pending_probe` board runs (HVS Q6 in [`hvs-backend-design.md`](hvs-backend-design.md) §7,
RK Q9 in [`rk3588-backend-design.md`](rk3588-backend-design.md) §7) can be compared with what the
backend would do with each kernel answer.

## Tags

- **read from source**: read in this repository at the commit above, in the file and function
  named. The line says what the code does, not what any kernel or board does.
- **inferred**: a conclusion drawn from those lines, or a statement about kernel behaviour that
  this note did not read. A line without a tag is inferred.
- No line here carries a hardware-run tag: this note copies nothing from `docs/observed/`.

## 1. The types involved

- Read from source (`crates/composition-core/src/backend.rs`, trait `DisplayBackend`): the engine's
  backend trait has `check` (a trial, "on Linux an atomic `TEST_ONLY` commit"), `commit(submission,
  frame, plan)` ("success means the hardware accepted the configuration, not that it is on
  screen") and `poll_event` ("must not block"). The backend later reports
  `BackendEvent::Latched { output, submission }`.
- Read from source (`backend.rs`, `enum BackendError`): `Rejected`, `Busy` ("a previous commit has
  not been latched and cannot be replaced"), `UnknownBuffer`, `DeviceLost`, `Platform(i32)`.
- Read from source (`crates/composition-drm/src/lib.rs`, `impl DisplayBackend for DrmBackend`):
  the DRM backend implements `check`, `commit` and `poll_event` (`lib.rs:1303`, `:1331`, `:1381`).
- Read from source (`crates/composition-core/src/caps.rs:172`, `Capabilities::replace_pending`):
  "whether a new commit may replace one that has not been latched yet. If `false`, committing while
  a submission is pending fails as busy." The default is `false` (`caps.rs:208`).

## 2. Where a commit is issued

- **`DrmBackend::check`** (`lib.rs:1303`, read from source): builds the request with `request()`,
  commits it with `AtomicCommitFlags::TEST_ONLY`, adding `ALLOW_MODESET` only when the output has
  not had its first commit yet. If that fails with a "refused" errno (`EINVAL`, `ERANGE`,
  `ENOSPC`) and no modeset was in it, the same request is tried again with
  `TEST_ONLY | ALLOW_MODESET`; if that passes, `DrmFailure::NeedsModeset` is logged and the result
  is still `BackendError::Rejected`. Other errors go through `fail()` (section 5).
- **`DrmBackend::commit`** (`lib.rs:1331`, read from source): the function that issues the real
  commit. `request()` yields the request and whether it contains a modeset, which is
  `!output.modeset_done` (`lib.rs:1166`). The flags are:
  - first commit of an output: `ALLOW_MODESET | PAGE_FLIP_EVENT`, **blocking** (the comment at
    `lib.rs:1342` gives the reason: a modeset may take several frame times and a nonblocking one
    can fail with `EBUSY` while the kernel finishes an earlier commit);
  - every later commit: `NONBLOCK | PAGE_FLIP_EVENT` (`lib.rs:1353`).

  It never sets `TEST_ONLY` on this path. `PAGE_FLIP_EVENT` is always set, so every accepted commit
  is expected to produce one page-flip event (inferred: DRM core sends one event per accepted
  commit that asked for it; the kernel was not read here).
- After the kernel accepts, `commit` records the book-keeping (read from source,
  `lib.rs:1359-1377`): `plane.crtc` is updated for each plane, and for the output
  `modeset_done = true`, `pending = Some(submission)`, `pending_since = Some(Instant::now())`,
  `overdue = false`. `pending` holds **one** submission id per output; it is overwritten, not
  queued.
- The caller is `Engine::commit` (`crates/composition-core/src/engine.rs:295`, `commit_untraced`),
  read from source: it calls `self.backend.commit(submission, frame, plan)?` at `engine.rs:327`,
  after `check_submit` (section 4).

## 3. How the event becomes Latched

All in `DrmBackend::read_events` (`lib.rs:993`), reached only from `poll_event` (`lib.rs:1381`)
(read from source):

1. `poll_event` calls `read_events()` unless the device is lost, then pops one queued event from
   `self.events`. It never blocks: `read_events` polls the descriptor with a zero timeout
   (`poll_device(Some(Duration::ZERO))`, `lib.rs:996`).
2. `HUP`, `ERR` or `NVAL` on the descriptor mark the device lost (`mark_lost`, which clears every
   `pending` and queues `BackendEvent::DeviceLost`). Nothing readable ends the loop.
3. `self.card.receive_events()` yields DRM events. Only `Event::PageFlip` is used; other events
   are skipped (`lib.rs:1029`). Inferred from the flags in section 2 (`PAGE_FLIP_EVENT` only):
   no vblank events are requested by this backend.
4. The event's CRTC is matched with an output (`o.crtc == flip.crtc`); no match is skipped. Then
   `output.pending.take()`: if the output has **no pending submission the event is ignored**
   (`lib.rs:1035`).
5. Otherwise `pending_since` and `overdue` are cleared; a `LatchInfo { output, submission,
   sequence: flip.frame, timestamp: flip.duration }` is stored as `last_latch`, pushed to the
   `latches` log (last 64), and **`BackendEvent::Latched { output, submission }` is pushed to
   `self.events`** (`lib.rs:1051`). This is the function that produces Latched.
6. `check_overdue()` then logs `DrmFailure::LatchTimeout` once for each pending submission older
   than the latch timeout (default `DEFAULT_LATCH_TIMEOUT`, `lib.rs:187`) after `probe_device()`;
   the submission **stays pending** (doc comment at `lib.rs:158`).

The event carries no submission tag in this code: the backend uses the output's single `pending`
value as the submission the event belongs to (read from source, steps 4 and 5).

The engine side (read from source, `engine.rs`): `Engine::poll_event` (`:401`) and
`collect_backend_events` (`:424`) call `backend.poll_event()`; `apply_backend_event` (`:436`)
passes `Latched` to `Engine::latched` (`:451`), which calls `Presentation::latched`
(`presentation.rs:403`): the output's `pending` must equal the submission, else
`PresentError::UnexpectedLatch`; the pending submission becomes `current`,
`DisplayEvent::Presented` is emitted and the buffers of the previous frame are released.
`Engine::commit_untraced` also drains `backend.poll_event()` right after `backend.commit`
(`engine.rs:340-367`), so a latch that arrived while the commit was written is recorded before the
new submission, and a latch of the new submission itself is held back and recorded after it
(`own_latch`).

## 4. A second commit before the event arrives

Which of the two paths happens depends on `replace_pending`.

**`replace_pending == false` (the default, and every profile today).**
- Read from source: no profile sets the flag. `crates/composition-drm/src/profile/vc4.rs:74` says
  `replace_pending` "stays as reported (unknown: inferred)"; a grep for `replace_pending` in
  `crates/composition-drm/src` finds only that comment, so no code in the DRM crate assigns it and
  the value is the engine default `false`.
- Read from source (`Engine::commit_untraced`, `engine.rs:310-325`): `replace =
  caps.replace_pending`; the engine first drains the backend's already-queued events
  (`collect_backend_events`), so a latch that has arrived is recorded; then
  `Presentation::check_submit(output, buffers, false)` returns `PresentError::Busy(output)` if the
  output's slot still has a pending submission (`presentation.rs:330-333`). `Engine::commit`
  returns `Error::Present(PresentError::Busy)`.
- Inferred from that order: **the kernel is not asked at all**. `backend.commit` (`engine.rs:327`)
  comes after `check_submit` (`engine.rs:324`) and is never reached: no atomic ioctl, no errno.
  The result is Busy from the engine, with the first submission untouched and still pending.

**`replace_pending == true`** (a caller or profile would have to set it; none does today). Read
from source:
- `check_submit(..., true)` passes, `backend.commit` runs, and `DrmBackend::commit` issues another
  `NONBLOCK | PAGE_FLIP_EVENT` commit (section 2). The backend does **not** check its own
  `output.pending` first and does not read events first.
- If the kernel accepts, `output.pending` is overwritten with the new submission and
  `pending_since` restarted (`lib.rs:1374-1375`). `Presentation::submit(..., true)` replaces the
  pending one, emits `DisplayEvent::Dropped` for it and releases its buffers that nothing else
  uses (`presentation.rs:388-395`).
- If the kernel refuses with `EBUSY`, see section 5: `Presentation` is not changed (the `?` at
  `engine.rs:327` returns before `submit`), so the first submission stays pending.

Inferred consequences of the backend overwriting one `pending` slot, if the kernel accepts both
commits and sends one event each:
- The **first event** (A's, by kernel order) finds `pending == B` and produces `Latched` for B.
  The **second event** (B's) finds `pending == None` and is ignored (`lib.rs:1035`). So B is
  reported latched one flip earlier than the kernel's own B event, and A is never reported
  latched. That matches the trait contract that a replaced submission must not be reported as
  latched (`backend.rs`, doc of `commit`), but B gets its Latched from A's event. What the display
  shows at that moment is for the board run to say.
- If the kernel instead coalesces (A dropped without an event), there is one event, for B, and the
  mapping is exact.

## 5. What `EBUSY` becomes

Read from source (`DrmBackend::fail`, `lib.rs:1094`, and `class_of`, `lib.rs:1446`):
- `class_of(EBUSY) == ErrnoClass::Busy`.
- In `fail`, the `Busy` arm logs `DrmFailure::Refused { operation, errno, primary_unused }`
  (`operation` is `Operation::Commit` from `commit`, `Operation::Check` from `check`) and returns
  **`BackendError::Busy`**. The raw errno stays in the failure log (`take_failures`,
  `last_failure`).
- The device is not marked lost, `output.pending` is not touched (`commit` returns before
  `lib.rs:1359`), and `Engine::commit` returns `Error::Backend(Busy)` through
  `From<BackendError> for Error` (`engine.rs:42`).
- For comparison: `EINVAL`, `ERANGE`, `ENOSPC` give `Rejected`; `EACCES`, `EPERM` give
  `Platform(errno)` (logged `NotMaster`) unless the CRTCs are gone, which is `DeviceLost`;
  `ENOENT` gives `Platform` unless the CRTCs are gone, and `ENODEV` gives `DeviceLost`; anything
  else gives `Platform(errno)`.
- So the engine's Busy has two sources: `Error::Present(PresentError::Busy)` (bookkeeping, no
  kernel call) and `Error::Backend(BackendError::Busy)` (kernel `EBUSY`), told apart by the error
  variant.

## 6. How the `replace_pending` claim is consumed

- Read from source: the claim lives in `Capabilities::replace_pending` (`caps.rs:172`); the only
  reader in the engine is `Engine::commit_untraced` (`engine.rs:310`), which passes it to
  `check_submit` and `submit`. `DriverProfile::apply` (`profile.rs:155`) takes
  `&mut Capabilities` and could set it, and `DrmBackend::capabilities` calls
  `self.options.profile.apply(caps)` (`lib.rs:1300`). No profile does, so `Generic`,
  `Rk3588Vop2`, `Vc4Hvs5` and `Vc4Hvs6` all leave it `false`.
- Inferred: the claim decides **where** a second commit is stopped, not whether the kernel would
  accept it. With `false` the engine never asks the kernel, so only `pending_probe --replace`
  (which commits through `Card` directly, not through `DrmBackend`) can answer it.
- Inferred: a profile may set `true` only if the kernel accepted B in the probe **and** the
  event-order consequence in section 4 is acceptable, or `DrmBackend` is changed to key events to
  submissions. That is a separate decision, not made here.

## 7. What the board sessions should compare

The kernel-side predictions are in [`vc4-replace-pending-source.md`](vc4-replace-pending-source.md)
(vc4 HVS5/HVS6) and [`drm-pending-commit-source.md`](drm-pending-commit-source.md) (DRM core, vc4
and VOP2). A VOP2 flip note, `docs/rk3588-vop2-flip-source.md`, was not in the tree when this note
was written, so it is not linked; read it too if it is present when the session runs.

`pending_probe` (`crates/composition-drm/examples/pending_probe.rs`) talks to the kernel through
`Card` with the same flag set as `DrmBackend::commit`, `NONBLOCK | PAGE_FLIP_EVENT`
(`pending_probe.rs:945`), and a `TEST_ONLY | NONBLOCK` check (`pending_probe.rs:996`). It does not
use `DrmBackend`, `Engine` or `replace_pending`. Its output lines map to backend states as follows
(inferred, from the line formats in `pending_probe.rs`):

| `pending_probe` output | Corresponding backend state |
|---|---|
| `SETUP commit=ok`, `SETUP events=1` | before a measured commit: `output.pending == None`, last submission latched |
| `FLIP n=.. commit=ok seq=.. ts_us=..` | one `DrmBackend::commit` accepted, then `read_events` takes the event: `Latched`, with `LatchInfo.sequence` = `seq` and `timestamp` = `ts_us` |
| `SUMMARY flips: submitted=N events=N missed=0` | every accepted commit became `Latched`; `missed>0` is a commit whose pending would sit until `LatchTimeout` is logged |
| `A commit=ok` | `output.pending = Some(A)` |
| `B_TEST_ONLY commit=ok` / `commit=EBUSY` | what `DrmBackend::check` would return for B while A is pending: `Ok` / `BackendError::Busy` |
| `B commit=EBUSY`, `EVENTS arrived=1 expected=1 a_event=yes b_event=n/a`, `VERDICT replace_pending: refused errno=EBUSY` | with `replace_pending=true` the backend would return `BackendError::Busy` and keep A pending; A's event gives `Latched` for A. With `false`, as today, the engine never reaches the kernel and reports `PresentError::Busy` |
| `B commit=ok`, `EVENTS arrived=2 expected=2 a_event=yes b_event=yes`, `VERDICT replace_pending: accepted` | the kernel kept both: with `replace_pending=true` the backend overwrites `pending` with B, the first event gives `Latched` for B and the second is ignored (section 4). `Dropped` for A comes from `Presentation`, not the kernel |
| `B commit=ok`, `EVENTS arrived=1 expected=2 a_event=yes b_event=no` | the kernel took B and sent one event (coalesced, or B's event missing). The backend would report B `Latched` from it and nothing stays pending; this probe cannot tell a coalesce from a lost event |
| `B commit=ok`, `arrived=0` | no event within `--timeout-ms`: the backend would keep `pending` and log `LatchTimeout` after the latch timeout |
| `PLANE_FB ... shows=A` / `shows=B` | the committed `FB_ID` after the events: `A` means the kernel kept A as current state (B refused), `B` means B was taken. It says nothing about the scanout; judge that by camera |
| `VERDICT replace_pending: unknown (A was refused ...)` | A itself failed (`fail()` would give `Rejected`, `Busy` or `Platform`); no statement about replacing |

The comparison for a session: for each `--replace` run, put the `VERDICT` and `EVENTS` lines next
to the row above and say which backend state was reached. If the kernel answers `accepted` with
two events, record that the backend's one-slot `pending` maps A's event to B (section 4), because
that, not the errno, decides whether a profile can claim `replace_pending == true`. A virtual
result (vkms) is never a hardware result.
