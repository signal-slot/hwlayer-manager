# Engine architecture overview

This is an overview of the engine as the repository has it at the commit that added this file. It follows `SoW.md` §1 (objectives) and §3 (system architecture) and the code. It describes structure only: it makes no hardware or timing claim, and nothing here was measured. Line numbers are those of that commit; they drift, so `grep` the name if one no longer matches. Terms are defined in [`docs/glossary.md`](glossary.md).

## 1. What the engine is

A display composition engine: a caller describes what should be on an output as layers backed by registered buffers, and the engine either maps every layer onto a hardware plane or rejects the frame. There is no fallback composition (`crates/composition-core/src/planner.rs:3-5`). The SoW's picture (§3) is a hardware-independent core, a `DisplayBackend` boundary, and one backend per platform. The repository holds the core, a fake backend, a Linux DRM/KMS backend, a C ABI, and one C backend for a two-plane display driver.

## 2. Crate map

| Path | Role |
|---|---|
| `crates/composition-core` | Scene and capability model, planner, buffer/submission lifecycle, backend trait, engine. `no_std`, no allocation, no dependencies, `#![forbid(unsafe_code)]` (`src/lib.rs:17-18`). |
| `crates/composition-fake` | A scriptable `DisplayBackend` for host tests. |
| `crates/composition-drm` | Linux DRM/KMS backend using atomic modesetting. |
| `crates/composition-ffi` | The C ABI over `Engine`; the only crate that allows `unsafe` (`src/lib.rs:13`). Header: `crates/composition-ffi/include/composition_engine.h`. |
| `crates/composition-ffi-staticlib` | `composition-ffi` as `libcomposition_engine.a` for `no_std` targets such as `thumbv7em-none-eabihf`; adds only the optional panic handler (`src/lib.rs:9-19`). |
| `backends/twoplane` | A backend written in C (`ce_twoplane.c`, `ce_twoplane.h`) for a display driver with two planes; the Rust package only carries its host tests. |

Build and test commands are in [`README.md`](../README.md); the checks CI runs are in [`docs/ci.md`](ci.md).

## 3. Core (`composition-core`)

The module layering is stated at `crates/composition-core/src/lib.rs:6-14`. Paths below are relative to `crates/composition-core/src/`.

### 3.1 Scene model: `types.rs`

- Ids: `OutputId` (`types.rs:5`), `PlaneId` (`:9`), `BufferId` (`:16`), `SubmissionId` (`:20`).
- `Rect` (`:40`), `Size` (`:24`), `PixelFormat` (`:83`, a fourcc), `Modifier` (`:143`), `Transform` (`:163`), `BlendMode` (`:193`).
- `BufferInfo` (`:152`): the immutable metadata of a registered buffer. The core never sees a file descriptor or address (SoW §3.2); the platform imports buffers and hands the core an id.
- `Layer` (`:215`): buffer, source and destination rectangles, z-order, alpha, blend, transform.
- `Frame<'a>` (`:253`): an output and a slice of layers. This is the "scene".

### 3.2 Capability model: `caps.rs`

`Capabilities` (`caps.rs:162`) holds `OutputCaps` (`:151`), `PlaneCaps` (`:75`; kind `PlaneKind` `:12`, formats `FormatModifier` `:24`, `ScaleLimits` `:46`, `Zpos` `:55`, `LayerTraits` `:103`) and `ResourceGroup` (`:139`, resources shared between planes such as scalers). A backend fills it in place after `Capabilities::clear` (`:213`). All tables have fixed maxima in `limits.rs:7-24` (for example `MAX_PLANES`, `MAX_LAYERS`, `MAX_BUFFERS`).

### 3.3 Planner: `planner.rs`

`plan_with` (`planner.rs:318`) runs a bounded depth-first search over layers, bottom to top, and offers each legal assignment to a callback that returns a `Verdict` (`:183`: accept, try next, give up). `plan` (`:302`) is the same with no backend check; `plan_into` (`:333`) is the in-place variant the engine uses. A `Plan` (`:234`) is a list of `Assignment`s (`:212`: layer index to plane, zpos, clipped source and destination), bound to a fingerprint of the frame and to the capability generation; `Plan::matches` (`:270`) checks the first. A failure is a `PlanError` (`:131`) with `RejectReasons` bit flags (`:34`). Buffer metadata comes through the `BufferLookup` trait (`:27`).

### 3.4 Lifecycle: `presentation.rs`

`Presentation` (`presentation.rs:145`) tracks each registered buffer through `BufferState` (`:30`): `Writable`, `Ready`, `Submitted`, `Scanning`, then back to `Writable` once released (SoW §3.1: a successful submit is not proof of display). `submit` (`:360`) records a commit; `latched` (`:403`) turns a hardware latch into `DisplayEvent::Presented` followed by `Released` for each buffer that is no longer referenced. `DisplayEvent` (`:43`) is what callers see: `Presented`, `Dropped`, `Released`, `DeviceLost`, `CapabilitiesChanged`, `LatchTimeout`. `report_overdue` (`:480`) produces the timeout from the time the caller supplies.

### 3.5 Backend contract: `backend.rs`

`DisplayBackend` (`backend.rs:48`) has four methods: `capabilities` (describe usable outputs and planes), `check` (final validation, no change), `commit` (program the frame under a `SubmissionId`), `poll_event` (non-blocking). `BackendEvent` (`:32`) is `Latched`, `DeviceLost` or `CapabilitiesChanged`; `BackendError` (`:17`) has `Rejected`, `Busy`, `UnknownBuffer`, `DeviceLost`, `Platform(i32)`. The trait's documentation fixes the ordering rule for a commit that replaces an unlatched one. This differs in detail from the "illustrative, not final" trait in SoW §3.3: there `validate` returns the plan, here the core plans and the backend only checks.

### 3.6 Engine: `engine.rs`

`Engine<B, T>` (`engine.rs:52`) owns one backend, the capabilities it reported, a generation counter, a `Presentation`, search limits, an optional latch timeout and a `Trace` sink. Its calls: `register_buffer` / `begin_write` / `finish_write` (`:183-198`), `validate` (`:210`), `commit` (`:281`), `present` (`:383`, validate then commit), `poll_event` (`:401`), `advance_time` (`:173`, the caller's clock; the core reads no clock). `Error` (`:17`) wraps `PlanError`, `PresentError` and `BackendError`, plus `StalePlan` and `PlanMismatch` for a plan that no longer fits.

### 3.7 Trace: `trace.rs`

`Trace` (`trace.rs:106`) receives `TraceEvent`s (`:38`: `Check`, `Validate`, `Commit`, `Latched`, `Released` and others). `NoTrace` (`:122`) is the default and costs nothing to prepare because `enabled()` is `false`. Timestamps are the value last given to `advance_time`.

## 4. Data flow

```text
producer
  | register_buffer(BufferId, BufferInfo); begin_write / finish_write
  v
Frame { output, layers }                     types.rs:253
  | Engine::validate / present               engine.rs:210, :383
  v
planner::plan_into                           planner.rs:333
  |  each legal candidate assignment list
  |  --> DisplayBackend::check(frame, plan)  (Rejected => next candidate)
  v
Plan { assignments, fingerprint, generation }          planner.rs:234
  | Engine::commit                           engine.rs:281
  |   stale generation => StalePlan; frame differs => PlanMismatch
  v
DisplayBackend::commit(SubmissionId, frame, plan)
  | (success = accepted by the driver, not shown)
  v
backend queues BackendEvent::Latched { output, submission }
  | Engine::poll_event                       engine.rs:401
  v
Presentation::latched                        presentation.rs:403
  v
DisplayEvent::Presented, then Released per freed buffer
```

Points that follow from the code:

- `Engine::commit` first takes the events the backend queued; a lost device fails the commit, and a capability change makes the plan stale (doc comment at `engine.rs:275-280`).
- A `Latched` event for a submission that is not pending is an `Error`, not a silent drop (`engine.rs:401-420`).
- Trace entries are written at `validate` (`engine.rs:220`), each `check` (`:246`) and `commit` (`:284`), and at latch and release, with the time last set by `advance_time`.
- The DRM backend also keeps the kernel's vblank sequence and timestamp of each latch on its own side channel (below); the core's events carry no timestamp.

For buffer readiness and release beyond this lifecycle, see [`docs/sync-fences.md`](sync-fences.md).

## 5. Backends

### 5.1 Fake: `composition-fake`

`FakeBackend` (`crates/composition-fake/src/lib.rs:44`) implements `DisplayBackend` (`:173`) in memory. A test drives it: `vsync` (`:108`) latches the pending commit, `vsync_in_next_commit` (`:90`) and `vsync_after_next_commit` (`:96`) place a latch relative to a commit, `fail_next_commits` (`:83`), `set_check_hook` (`:78`, a `CheckHook`, `:41`), `lose_device` (`:122`), `inject_event` (`:141`), and inspection with `scanout` (`:146`) and `pending` (`:151`). It has no timing of its own.

### 5.2 DRM: `composition-drm`

Paths in this section are in `crates/composition-drm/src/`. `DrmBackend` (`lib.rs:282`) is built on a file descriptor (`open` `:386`, `from_fd` `:398`), so a DRM lease descriptor works like a device node and the reported capabilities are exactly what the descriptor exposes (module comment, `lib.rs:1-8`). `capabilities` (`:1289`) lists outputs from the CRTCs and planes from the plane properties; `DriverProfile` (`profile.rs:16`) can add a vendor's resource limits on top, chosen explicitly through `DrmOptions` (`lib.rs:76`). `check` (`:1307`) is an atomic `TEST_ONLY` commit; `commit` (`:1335`) is an atomic commit requesting a page-flip event (`:1355-1357`). `poll_event` (`:1385`) reads the DRM fd (`read_events` `:997`); a page-flip event becomes a `LatchInfo` (`:172`, with the CRTC's vblank sequence and timestamp) kept for `take_latches` (`:898`), and a `BackendEvent::Latched` (`:1044-1055`). Raw errno and failure reasons that `BackendError` cannot carry are kept as `DrmFailure` (`:100`, `take_failures` `:886`). Buffers come in through `import_dmabuf` (`:674`) or `create_dumb_buffer` (`:738`). The crate is `#![forbid(unsafe_code)]` (`:27`). Leases and Weston coexistence are `SoW.md` §4 topics, not covered here.

### 5.3 C ABI: `composition-ffi`

The header is `crates/composition-ffi/include/composition_engine.h`; `crates/composition-ffi/src/lib.rs` is its implementation, and the two change in the same commit (checked by `scripts/check-ffi-header.sh`). The C caller supplies a `ce_backend_ops` table (`lib.rs:221`, header `:229`) with `capabilities`, `check`, `commit` and `poll_event` callbacks; `CBackend` (`lib.rs:254`) adapts it to `DisplayBackend` (`:438`). `ce_engine` (`:264`) wraps `Engine<CBackend>` in caller-provided storage, so there is no allocation. Entry points: `ce_abi_check` (`:609`), `ce_engine_init` (`:635`), `ce_register_buffer` (`:675`), `ce_unregister_buffer` (`:700`), `ce_begin_write` (`:714`), `ce_finish_write` (`:728`), `ce_buffer_state` (`:742`), `ce_present` (`:764`), `ce_poll_event` (`:841`), `ce_set_latch_timeout` (`:889`), `ce_advance_time` (`:906`). Results are a `ce_status` (`:31`); `ce_error_info` (`:245`) carries details. Events come back as `ce_event` (`:235`), mirroring `DisplayEvent`. The C API exposes `ce_present` only: there is no separate validate and commit call.

### 5.4 Static library: `composition-ffi-staticlib`

A `no_std` crate with no code of its own beyond `extern crate composition_ffi` (`src/lib.rs:13`); it builds to `libcomposition_engine.a`. With the `panic-handler` feature it defines a `#[panic_handler]` that loops (`:15-19`), for C programs with no other Rust code. A program that already links Rust code depends on `composition-ffi` instead, so there is one copy of `core` and one panic handler. The two thumbv7em builds are in [`README.md`](../README.md).

### 5.5 Two-plane backend: `backends/twoplane`

A backend in C against the C ABI, for a driver with two planes on one output (application plane below, safety plane on top). It reaches the driver through `struct ce_twoplane_hw` (`ce_twoplane.h:67`) and the system through `struct ce_twoplane_port` (`:88`); its state is `struct ce_twoplane` (`:110`), initialised by `ce_twoplane_init` (`:137`). Its `ce_backend_ops` table is `ce_twoplane_ops` (`ce_twoplane.c:247`), built from `twoplane_check` (`:114`), `twoplane_commit` (`:154`), `twoplane_poll_event` (`:227`) and the vsync callback `vsync_cb` (`:50`). Its tests, what they do not cover, and the port-table contract are in [`docs/twoplane-backend.md`](twoplane-backend.md).

### 5.6 Planned, not implemented: i.MX95 M7 backend (milestone M2)

No i.MX95 M7 backend exists in this repository, and nothing about it was measured on hardware. This section only places the planned work against the crates above. It comes from [`docs/imx95-m7-backend-contract.md`](imx95-m7-backend-contract.md), [`docs/imx95-twoplane-port.md`](imx95-twoplane-port.md), [`docs/twoplane-backend.md`](twoplane-backend.md) and [`docs/imx95-zephyr-feasibility.md`](imx95-zephyr-feasibility.md).

- **Core: unchanged.** *Inferred.* `composition-core` stays hardware-independent. The M7 backend would implement the same four operations of `DisplayBackend` (§3.5), through the C callbacks `capabilities`, `check`, `commit` and `poll_event`. The contract document maps each operation onto Zephyr's display API and records which parts need a driver extension.
- **ABI and library: reused.** *Inferred.* A Zephyr application would link `libcomposition_engine.a` from `composition-ffi-staticlib` (§5.4) and give `composition-ffi` (§5.3) a `ce_backend_ops` table. The backend would be C or Rust firmware code outside the core, like `backends/twoplane`.
- **`backends/twoplane`: partly reusable.** *Read from source* (contract doc §5): its `ce_twoplane_port` (interrupt lock, wake, buffer lookup, device-ready) maps onto Zephyr calls. Its capabilities, `check` and `commit` do not carry over as they are: they assume two planes and a mandatory safety layer, while the stock Zephyr DPU driver has one plane. A single-plane backend would need its own versions. *Inferred:* the port document (a design, written on paper) proposes filling the hardware and port tables for the DPU, and the pending/latched protocol could be shared; that would edit `backends/twoplane` and is a later item.
- **Open prerequisite.** *Read from source:* whether the M7 may own the DPU and display route at all, under the board's System Manager configuration, is the question of the feasibility document. The contract document assumes a favourable answer and does not claim it.
- **Status.** No M7 backend code, no backend host test and no board result exist for this target. Milestone status is in [`README.md`](../README.md).

## 6. Where to look next

- Terms: [`docs/glossary.md`](glossary.md).
- Two-plane backend, tests and limits: [`docs/twoplane-backend.md`](twoplane-backend.md).
- Readiness, fences and release (design note): [`docs/sync-fences.md`](sync-fences.md).
- Requirements and milestones: [`SoW.md`](../SoW.md); the repository's status against them is in [`README.md`](../README.md).
