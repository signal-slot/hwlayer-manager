# Synchronization: readiness, fences and release

Status: **design note**. Nothing in this document was run or measured on
hardware. Sections 1 and 2 describe the code as read in the repository at the
time of writing; sections 3 to 7 are proposals and are not implemented.

Why it matters: SoW.md §7 requires "no scanout of a partially rendered/unready
buffer" and separate *accepted/submitted*, *latched/presented* and *released*
events; §4.1 asks the DRM backend to handle "synchronization fences where
supported"; §6 WP2 / M1 condition 5 asks that the three events be separately
observable and that rotation shows no unsafe overwrite in defined tests.

## 1. How readiness is expressed today

### 1.1 Core: a producer-declared state, no fences

- Lifecycle in `crates/composition-core/src/presentation.rs` (module doc, top
  of file): `Writable --finish_write--> Ready --submit--> Submitted
  --latched--> Scanning`, back to idle on a `Released` event.
- `Engine::begin_write` / `Engine::finish_write`
  (`crates/composition-core/src/engine.rs`, around lines 192-200) forward to
  `Presentation::begin_write` / `finish_write`. They are a *promise by the
  caller* that the CPU or device has finished writing. The core cannot
  observe the writes.
- `Presentation::check_submit` (`presentation.rs`, around line 315) refuses a
  frame that uses a buffer still in the writable state with
  `PresentError::BufferNotReady`; the C ABI maps it to `CE_ERR_BUFFER_NOT_READY`
  (`crates/composition-ffi/include/composition_engine.h`, line 66). A buffer
  that is `Submitted` or `Scanning` can only be reused after
  `DisplayEvent::Released`.
- `Layer` (`crates/composition-core/src/types.rs`, around line 213) carries
  no synchronization data: buffer, source, destination, z-order, alpha, blend,
  transform.
- Backend contract (`crates/composition-core/src/backend.rs`):
  `DisplayBackend::commit` success means "the hardware accepted the
  configuration, not that it is on screen"; the only completion event is
  `BackendEvent::Latched { output, submission }`. There is no per-buffer
  event: the engine derives `Presented`, `Dropped` and `Released`
  (`DisplayEvent`, `presentation.rs` lines 43-82) from latches. A buffer is
  released when the latch of the submission that replaced it is recorded
  (`Engine::latched`, `engine.rs` around line 451).
- Platform buffer import is outside the core (`backend.rs` module doc), which
  keeps the core free of file descriptors.

### 1.2 DRM backend: no fence properties

- `crates/composition-drm/src/lib.rs` builds the atomic request in
  `DrmBackend::request` (around line 763). It sets the connector `CRTC_ID`,
  CRTC `MODE_ID` / `ACTIVE` for a modeset and per-plane properties including
  `FB_ID` (property table, around line 495). It does **not** look up or set
  `IN_FENCE_FD` (plane) or `OUT_FENCE_PTR` (CRTC). A search for "fence" in the
  crate finds nothing.
- `check` is an atomic `TEST_ONLY` commit; `commit` uses
  `NONBLOCK | PAGE_FLIP_EVENT` (plus `ALLOW_MODESET` for the first commit),
  around lines 911-945.
- The latch is the DRM page-flip event: `read_events` (around line 716) turns
  `Event::PageFlip` on a CRTC into `BackendEvent::Latched` for the output's
  one pending submission.
- Buffers come from `create_dumb_buffer` (CPU-drawn, `draw_dumb_buffer`) or
  `import_dmabuf` (around line 547), which wraps the descriptors in a
  framebuffer and keeps no reference to any fence or reservation state.
  A DMA-BUF imported from a GPU or camera producer therefore relies on the
  kernel's implicit synchronization (the reservation object attached to the
  dma-buf), if the producer's driver and the display driver both use it. This
  document does not establish whether that holds on the i.MX95 BSP; it is
  unknown.

### 1.3 C ABI

- `composition_engine.h` exposes `ce_begin_write`, `ce_finish_write` (lines
  ~315-317), `ce_present`, the event enum `CE_EVENT_PRESENTED / DROPPED /
  RELEASED / DEVICE_LOST / CAPABILITIES_CHANGED / LATCH_TIMEOUT` and
  `ce_buffer_state`. `ce_layer` (line 124) has no fence member. The C backend
  vtable reports `CE_BACKEND_LATCHED` (line ~209).
- The ABI is size-checked with `ce_abi_check`; adding fields to `ce_layer` or
  `ce_event` changes struct sizes and would be an ABI break (see §5).

### 1.4 Harness and test plan

- `tools/drm-harness` (README, `src/session.rs`): in-process producers draw
  dumb buffers with the CPU (`draw_dumb_buffer`, `session.rs` around lines
  517-545: `begin_write`, draw, `finish_write`) *before* the frame is
  submitted. A buffer is redrawn only after the engine reported it
  `Released`. The harness waits for each latch before drawing the next frame
  (README, notes after the scenario table).
- External producers pass DMA-BUFs over a socket (`src/producer.rs`); the
  message set has `Released { buffer }` (producer.rs, message table) and no
  fence message. The producer is told a buffer may be reused only after the
  engine's `Released`.
- `docs/m1-imx95-test-plan.md` §7 (condition 5) tests the order
  submitted → latched → released from a log, no write between submit and
  release, `latched + dropped = submitted`, a checksum readback, and a camera
  picture. It does not mention fences.

### 1.5 What this gives, and what it does not

Given as design reading, not as a measurement:

- **Guaranteed by construction (in the core):** a frame is refused unless
  every buffer was declared finished by the producer; a buffer is not handed
  back before the latch of the frame that replaced it.
- **Not provided:** any check that the producer's writes have *actually*
  completed. For CPU-drawn dumb buffers written before `finish_write`
  returns, completion is program order. For GPU, video-decoder or camera
  producers writing asynchronously, `finish_write` would have to be called
  only after the producer's own completion; the engine has no way to defer
  scanout until an asynchronous write completes, and no way to learn that
  the display is done with a buffer earlier than the replacing latch.
- **Conservative release:** because release is tied to the replacing latch
  and the backend reports one flip event per commit, a buffer's release is
  never early with respect to scanout, but may be later than the earliest
  safe moment (the scanout engine may stop reading a buffer slightly before
  the flip event is delivered to user space; this is not exploited).

## 2. Gaps against the SoW

| Requirement | Today | Gap |
|---|---|---|
| No scanout of a partially rendered buffer (§7) | Producer promise (`finish_write`); the core refuses writable buffers | Asynchronous producers cannot hand over "will be complete at time T"; the engine cannot enforce it |
| Separate submitted / latched / released (§7) | Present (`DisplayEvent`) | None for the CPU-written case; release is tied to the replacing latch only |
| Fences where supported (§4.1) | None: no `IN_FENCE_FD`, no `OUT_FENCE_PTR`, no fence in `Layer` or in the C ABI | Whole feature |
| Zephyr / MCU | No readiness mechanism beyond `finish_write` | A flag or callback is not defined |

## 3. Proposed design (not implemented)

Constraints: the core stays `no_std`, allocation-free and dependency-free
(it already has fixed-capacity types, `fixed.rs`, `limits.rs`) and never
contains a file descriptor type. Fences are therefore **opaque to the core**.

### 3.1 Core: a readiness token, not a fence

Add to the core a small opaque token that names "a thing a backend knows how
to wait for", and a per-layer slot for it:

```text
// Sketch only. Names are illustrative.
pub struct ReadyToken(pub u64);      // 0 / None = already complete

pub struct Layer { ...existing fields..., pub ready: Option<ReadyToken> }
```

- The token is chosen by the producer-side glue and is meaningful only to the
  backend that issued or imports it (like `BufferId` is meaningful only to
  the backend that imported the buffer, `backend.rs` module doc). The core
  copies it into the frame and hands it back to the backend in
  `check` / `commit`; it does not interpret it.
- Because a layer already refers to a buffer, an alternative is to attach
  the token to a buffer submission (`Engine::finish_write_with(buffer,
  token)`) rather than to the `Layer`. Preference: **per-buffer
  "ready with token" at `finish_write` time**, since readiness is a property
  of the content, not of where it is placed; the same buffer in two layers
  or two frames then shares one fence. The core stores one token per
  registered buffer (fixed table, same capacity as the buffer table) and
  gives it to the backend with the frame. This keeps `Layer` and
  `ce_layer` unchanged (relevant to §5). Either way is a design choice for
  the implementer; this note does not decide it beyond the preference.
- State model change: add an optional distinction, *Ready* versus
  *Ready (pending signal)*, so that `Presentation::check_submit` still
  refuses a buffer that is `Writable` but accepts one that is "complete once
  the token signals". `BufferNotReady` keeps its meaning.
- A backend that cannot wait for a token (or the fake backend, by default)
  must return a defined refusal (`BackendError::Rejected`, or a new
  `Unsupported`) for a frame that carries a token, never silently ignore it.
  Ignoring would reproduce the problem this note is about.
- Release: add an optional backend-reported event
  `BackendEvent::Released { output, submission }` or per-buffer variant
  (`Released { buffer }`) for backends that know earlier than the replacing
  latch, plus a release token handed back to the producer. Without it the
  existing rule applies (release at the replacing latch). The engine must
  never release a buffer before the *earlier* of the two events is
  confirmed, and must never release it twice.

### 3.2 Linux DRM backend

- **Acquire (input) fence.** Backend-side `import_dmabuf` (or a new
  `set_acquire_fence(buffer, OwnedFd)`) stores a `sync_file` fd with the
  buffer. At commit, for each plane in the plan, if the buffer has an
  acquire fence, set plane property `IN_FENCE_FD` to the fd. The kernel
  takes its own reference; the fd may be closed after the ioctl. Create
  the property lookup the same way as `FB_ID` in `discover`
  (`require`/`find` in `lib.rs`); the property is optional, and its absence
  is the capability bit "acquire fences supported" in `Capabilities`
  (design: a new flag in the capabilities, which would be an ABI change, §5).
  If a frame carries a fence and the plane lacks `IN_FENCE_FD`, `check`
  returns `Rejected`.
- Using `TEST_ONLY` with `IN_FENCE_FD` set is allowed by the DRM interface,
  but the kernel consumes the fd semantics at commit; the backend must pass a
  duplicate for the test so the real commit still has the fence. (Design; not
  verified against this kernel.)
- **Release / out fence.** Optional: set the CRTC property `OUT_FENCE_PTR`
  to a user address; the kernel returns a `sync_file` that signals when the
  commit has completed (at the flip, per the kernel documentation); on the
  CRTC this is a per-commit fence, not a per-buffer one. The first use is as a second completion source next to the page-flip
  event, which is redundant for scanout; it matters for exporting a release
  fd to an external producer (§3.4). Page-flip events remain the primary
  latch; `OUT_FENCE_PTR` is not needed for M1.
- **Failure behaviour:** a fence that signals with an error must lead to the
  frame being refused or the buffer not shown; not displaying is preferable
  to displaying garbage. This is DRM atomic behaviour for the commit and is
  to be confirmed, not assumed.
- **Non-blocking:** with `NONBLOCK`, an unsignalled `IN_FENCE_FD` defers the
  commit inside the kernel; the page-flip event (the latch) is then
  delayed, so `latch_timeout` (`Engine::set_latch_timeout`) must be
  larger than the longest expected producer completion, or the engine
  reports `LatchTimeout` spuriously. A design caveat for the timeout
  documentation.

### 3.3 Zephyr / MCU

The no-fd equivalent is the token from §3.1 plus a backend-defined wait:

- **Readiness flag:** the token is an index into a small table owned by the
  backend; the producer (or its DMA completion ISR) sets "complete". The
  backend's `commit` queues the frame and programs the display controller
  only when the flag is set, or the backend polls the flag in `poll_event`.
- **Callback:** a function pointer registered with the backend, called from
  `poll_event` to ask "is token N complete?". No blocking in `poll_event`
  (`backend.rs` requires it not to block).
- Programming the display controller (the actual commit) stays under the
  single GO/commit owner invariant (SoW §6 WP, AM62P operational
  invariants); the flag changes only *when* the backend commits, not who
  commits.
- Nothing is specified here for cache maintenance; the core stays out of
  it (SoW §4, DMA synchronization delegated to platform code).

### 3.4 C ABI

All of this would extend `crates/composition-ffi/include/composition_engine.h`
(not changed by this note):

- **Do not grow `ce_layer` or `ce_event` in place.** Prefer new functions:
  `ce_finish_write_fence(engine, buffer, int64_t token)` (the token for the
  Linux backend is the fd number as a `int`, owned by the caller until the
  call returns, then duplicated by the backend; on Zephyr an opaque index),
  and `ce_set_release_callback` or an event kind `CE_EVENT_RELEASED` that
  already exists. Existing C callers keep working.
- **Capabilities:** a flag (e.g. `ce_capabilities.acquire_fences`) appended at
  the end of the struct; this changes `sizeof`, which `ce_abi_check` catches
  by design. The ABI version should be bumped.
- **Ownership:** a fd passed in is owned by the engine on success, by the
  caller on failure. Stated in the header comment when implemented.
- **Release export:** an optional `ce_buffer_release_fd(engine, buffer, int *fd)`
  returning a `sync_file` that signals at release, for external producers;
  not for M1.
- The Rust FFI wrapper must reject a negative or invalid descriptor with a
  defined status, and return `CE_ERR_*` rather than ignore a fence the
  backend cannot honour (§3.1).

## 4. Minimum for M1 versus later

M1 (SoW §9, conditions 3 and 5) needs:

1. The existing three-event model, with producers that finish writing before
   `finish_write` and `Released` only after the replacing latch. This is
   implemented today (§1.1). The tests in `docs/m1-imx95-test-plan.md` §7
   exercise it.
2. A **written statement** in the M1 report that fences are not implemented
   and which producers it covers (§6).

Explicitly not needed for M1: `IN_FENCE_FD`, `OUT_FENCE_PTR`, any C ABI
change, any Zephyr readiness mechanism, GPU/video producers.

Later, in this order (each separable):

1. Core readiness token and "unsupported fence is refused" rule (§3.1); fake
   backend support so the state machine is tested on the host without
   hardware, including adversarial orders (fence never signals, signals
   late, signals with error).
2. DRM `IN_FENCE_FD` with vkms (if vkms implements it; unverified) and on the
   board. Needed for WP5 and for any GPU/camera/decoder producer; may be
   required earlier if M1 is later widened to dma-bufs written by hardware.
3. C ABI additions (§3.4).
4. Release fds / `OUT_FENCE_PTR` for external producers.
5. Zephyr flag/callback with the M2 backend.

## 5. Compatibility and risk notes (design)

- The core and the DRM backend both treat "the device went away" as a state,
  not an error code; a fence error should map the same way only when the
  device is gone, otherwise to a refused submission.
- `IN_FENCE_FD` is per plane; `OUT_FENCE_PTR` is per CRTC. A buffer fence
  maps naturally to the first; a release for a single buffer does not map to
  the second. This is why release stays event-based for now.
- Whether the i.MX95 display driver of the board's kernel exposes
  `IN_FENCE_FD` is **not known**; it is a generic atomic-helper property on
  planes of drivers using the atomic helpers, but whether it appears must be
  read from the probe output (WP0, `docs/platform-capabilities.md`), not
  assumed.
- Implicit sync on imported dma-bufs is a separate question from explicit
  fences; both need an answer for GPU producers and neither is answered here.

## 6. Wording advice for the M1 report, condition 5

If fences are not implemented when the report is written, the report must say
so in the condition 5 entry, not leave it to be inferred. Suggested text, to
be adjusted to what the evidence shows; a clause is only to be used if the
run backs it:

> **Condition 5 — submission, latch and release.** `submitted`, `latched`
> and `released` are separate events in the engine and in the harness log.
> Synchronization fences (`IN_FENCE_FD` / `OUT_FENCE_PTR`) are **not
> implemented** in this milestone: `composition-drm` sets neither property.
> Readiness is expressed by the producer: the harness draws each buffer
> completely with the CPU (dumb buffers) and only then calls `finish_write`,
> before the frame containing it is submitted; the engine refuses a frame
> that uses a buffer not declared finished. A buffer is reused only after the
> engine's `Released` event, which is raised only after the page-flip event
> of the commit that replaced it. The tests show [results: number of
> frames, ordering violations, `latched + dropped = submitted`, dropped
> count, checksum readback]. This establishes the lifecycle for CPU-written
> buffers. It does **not** show that a buffer written asynchronously by a
> GPU, decoder or camera is complete at scanout: no such producer was used,
> and no acquire-fence or implicit-synchronization behaviour was tested.
> The external-producer scenario passes dma-bufs that were completely drawn
> before they were sent. Latency is measured from the commit call to the
> engine's `Presented` event, which includes wake-up time.

Advice:

- Verdict wording: **passed for CPU-written buffers**, with the limitation
  stated in the same paragraph; do not write "synchronization verified" or
  "fences supported". If any ordering violation or unexplained dropped frame
  occurred, report it with its count; do not omit a zero.
- Do not claim that release is "as early as possible"; it is conservative
  (§1.5).
- Do not cite the vkms results as evidence for i.MX95; vkms timing and
  fence behaviour differ from the DPU.
- Add the fence gap to the list of open issues (M1 follow-up, WP5), with a
  pointer to this document.
- If a later run does implement `IN_FENCE_FD`, the entry needs a separate
  sentence with the property's presence in the probe output, the test that
  held a fence unsignalled and showed the previous frame stayed on screen,
  and the kernel's behaviour for an errored fence.

## 7. Open questions (for whoever implements)

- Does the board's display driver list `IN_FENCE_FD` on its planes? (probe
  output, WP0.)
- Do the vkms version in CI and the board kernel honour `IN_FENCE_FD` with
  `NONBLOCK` and `PAGE_FLIP_EVENT`, and when is the flip event delivered
  relative to fence signal?
- Token or per-layer fence (§3.1): decide before the ABI is frozen.
- Whether `LatchTimeout` should be suspended while a frame is waiting on an
  acquire fence (a frame waiting on a fence is not late).
