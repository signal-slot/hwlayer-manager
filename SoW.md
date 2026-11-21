# Statement of Work (SoW)

## Cross-Platform Hardware Composition Engine

- **Document:** `SoW.md`
- **Status:** Draft / implementation baseline
- **Date:** 2026-11-18
- **Primary deployment:** Automotive Grade Linux (AGL)
- **Top-priority SoC:** NXP i.MX95 (first hardware bring-up and acceptance target)
- **Additional targets:** Linux DRM/KMS, Zephyr RTOS, bare-metal and resource-constrained MCUs
- **Implementation language:** Rust (`no_std` core), with C ABI where necessary

## 1. Background and objectives

Develop an independent, reusable **hardware composition engine** that accepts completed rendering buffers from applications and arranges them on display hardware, using the capabilities of each SoC. It must not require an application to render through a particular graphics framework, GPU API, or operating system.

The immediate Linux/AGL integration point is **DRM/KMS**, using AGL's **drm-lease-manager** to obtain display resources when operating as the owner of a leased display. In Zephyr and bare-metal builds, the same composition model is implemented through SoC-specific display-controller drivers.

This is **not** a replacement for Slint, Qt, YakoGL, or Weston. Slint/Qt/YakoGL produce content; this engine decides how already-rendered content is presented, using direct scanout, hardware overlays, and, where explicitly supported, pre-composition.

### Objectives

1. Establish a small, hardware-independent composition API and `no_std` core.
2. Support direct scanout and multiple hardware overlay planes, including z-order, crop, destination rectangles, alpha, pixel formats, and synchronization subject to actual hardware capabilities.
3. Expose capabilities and validate a proposed composition before committing it.
4. Coordinate buffer ownership, presentation, completion, and safe buffer reuse without tearing or premature overwrite.
5. Support a Linux DRM/KMS backend that accepts a DRM lease file descriptor; do not couple core logic to drm-lease-manager or libdlmclient.
6. Prioritize NXP i.MX95 display hardware on Linux/AGL, then implement and validate a Zephyr i.MX95 path where hardware access and driver support permit.
7. Reuse and generalize the existing TI AM62P DSS implementation as a secondary reference and regression target.
8. Keep the architecture extensible to MCU display controllers, optional 2D engines, and other SoCs.
9. Provide a credible path toward upstream integration and use by AGL without maintaining a large downstream Weston fork.

## 2. Scope boundaries and architectural principles

### In scope

- Composition scene/layer model and capability model.
- Hardware resource/plane assignment subject to constraints.
- Frame submission, atomicity semantics, presentation notifications, and buffer lifetime tracking.
- Import/registration of platform-specific buffers via opaque handles.
- Backend adapters for Linux DRM/KMS on i.MX95 first; i.MX95 Zephyr display-controller access next; TI AM62P DSS as a secondary portability adapter.
- Test harnesses, visual validation, trace/metrics, and documented portability requirements.
- Optional safety-layer priority and graceful fallback policy **as a policy mechanism**, not a safety certification claim.

### Out of scope for initial release

- A complete Wayland compositor, window manager, input stack, or application launcher.
- Reimplementation of Slint/Qt rendering or of YakoGL's GLES/EGL APIs.
- A general-purpose GPU renderer or video decoder.
- Guaranteed support for arbitrary transforms, pixel formats, or overlays on every SoC.
- Functional-safety certification, freedom-from-interference certification, or an ASIL claim.
- AGL/Weston upstream acceptance as a guaranteed deliverable.
- Full 2D/GPU pre-composition fallback in the first milestone; specify extension points first.
- Replacing or moving working code from existing repositories before an adapter-based integration is proven.

### Principles

- **Presentation rather than rendering:** use display-controller overlay whenever legal and beneficial.
- **Capability-driven:** never pretend a device can perform an unsupported operation.
- **Explicit ownership:** only the backend that owns the active display resources may commit them.
- **Separation of concerns:** resource acquisition (DRM Lease), composition planning, and hardware submission are distinct layers.
- **Bounded implementation:** static/limited-memory configurations should be feasible; avoid mandatory heap allocation in the core.
- **Correctness first:** a rejected composition is preferable to silently incorrect output.
- **No assumptions about sharing:** DRM leasing is exclusive at the leased DRM-object level; it is not a mechanism for independent processes to drive the same CRTC simultaneously.

## 3. System architecture

```text
Slint / Qt / Video / Camera / Other producer
             |
       Buffer + Layer API
             |
+--------------------------------------+
| Composition Core (Rust, no_std)       |
| - output/layer/scene data model       |
| - capability & resource model         |
| - plane assignment / validation      |
| - transaction and buffer state       |
| - constraints, priority, fallback    |
+--------------------------------------+
             |
   DisplayBackend interface
        /             \
 Linux DRM/KMS       Zephyr / bare metal
 DRM lease FD        i.MX95 DPU / other SoC drivers
 Atomic modeset      MMIO / shadow regs / IRQ
        \             /
         Display controller
```

### 3.1 Core

Core responsibilities:

- Define outputs, layers, source/destination rectangles, clipping, z-order, blend mode and alpha, pixel format/modifier, buffer handles, frame transactions, and display events.
- Maintain an extensible capability description: per-plane format/modifier support, scaling and alignment limits, alpha capabilities, rotation, z-order restrictions, bandwidth/resource conflicts, available outputs, and commit behavior.
- Generate a candidate plane assignment and reject unsupported configurations; allow backend-specific final validation.
- Model and track transitions: **writable → submitted → queued/latched/scanning → released**. A successful submission is *not* proof that the requested frame was displayed or that old buffers are reusable.
- Preserve a distinction between desired state, submitted state, hardware-latched state, and safely released state.
- Allow a caller to request fixed/high-priority layers, and fail clearly if preservation of those layers is impossible.

### 3.2 Platform boundary

The core works with opaque, registered `BufferId`/`BufferHandle` and never directly assumes Linux file descriptors, physical addresses, or MCU bus addresses. Import, allocation, mapping, cache maintenance, DMA synchronization, lifetime reference counting, and access control are delegated to platform-specific code.

The core shall have a bounded-memory mode with explicit maximum numbers of outputs, layers, planes, and in-flight transactions. A C ABI is provided for C/C++ or Zephyr clients while Rust remains the implementation language.

### 3.3 Backend contract (illustrative, not final API)

```rust
pub struct Layer {
    pub buffer: BufferId,
    pub source: Rect,
    pub destination: Rect,
    pub z_order: i32,
    pub alpha: u16,
    pub transform: Transform,
}

pub struct Frame<'a> {
    pub output: OutputId,
    pub layers: &'a [Layer],
}

pub trait DisplayBackend {
    fn capabilities(&self) -> Capabilities;
    fn validate(&mut self, frame: &Frame<'_>) -> Result<Plan, Error>;
    fn commit(&mut self, plan: &Plan) -> Result<SubmissionId, Error>;
    fn poll_event(&mut self) -> Option<DisplayEvent>;
}
```

The final API must additionally define buffer registration/import, immutable buffer metadata, input readiness synchronization, timeout/cancellation, event ordering, device loss, and reconfiguration. `Plan` must bind resource assignment to the validated output configuration and be invalidated when backend resources or capabilities change.

## 4. Linux / AGL backend

### 4.1 DRM Lease integration

- Support a DRM/KMS device or **already obtained DRM lease fd** as an input to the backend.
- Provide a small optional adapter for AGL `drm-lease-manager` / `libdlmclient` resource acquisition.
- Discover which connector, CRTC, planes, and relevant properties are actually accessible through the lease; do not assume all planes on the physical device are available.
- Use DRM Atomic APIs for validation and presentation; support `DRM_MODE_ATOMIC_TEST_ONLY` when available.
- Handle DMA-BUF import, framebuffer creation, format/modifier negotiation, synchronization fences where supported, page-flip/commit events, and loss/revocation of the lease.
- Reuse existing established DRM/KMS or libliftoff concepts where advantageous, avoiding unnecessary duplicate algorithms.

**Important:** A DRM Lease grants exclusive access to leased objects. In the initial AGL deployment model the new engine owns an entire leased output and its usable planes. Independent Slint/Qt processes send rendered buffers to the engine; they do not each commit the same CRTC through different leases.

### 4.2 Weston coexistence

Two separate integration modes shall be evaluated:

1. **Standalone leased-output mode (first priority):** the engine owns the leased output. No Weston internal modification is required for that output, provided a valid lease with the necessary objects is available.
2. **Weston-owned output mode (later investigation):** a Weston integration must use Weston's ownership/commit path. DRM Lease does not by itself allow the new engine to modify planes on a CRTC Weston still owns.

A full-fledged Wayland producer protocol or Weston backend patch is **not** required for Milestone 1; an elementary producer harness suffices.

## 5. Zephyr / MCU backend

### 5.1 First-priority NXP i.MX95 backend

The **first supported SoC and the hardware acceptance baseline is NXP i.MX95**, not AM62P. Prioritize a board with a working display and accessible Linux DRM/KMS pipeline. Initial board candidate: an i.MX95 EVK (exact board revision, panel/bridge and Linux BSP to be recorded in WP0).

**Linux / AGL first:** inspect the installed kernel's actual i.MX95 Display Controller / DPU driver and DRM topology, enumerate usable connectors/CRTCs/planes and verify atomic operations. Use `drm-lease-manager` for the leased-output deployment when supported. Build a real multi-plane composition demo, not merely a single flattened framebuffer.

**Zephyr second, on the same SoC:** determine which i.MX95 core owns DPU, clocks, power domains, display routes, interrupts and scanout memory under the chosen boot flow. Zephyr board descriptions list `socionext,dpu`, but **the existence of a devicetree node is not evidence that multi-plane hardware composition is implemented or that an independently owned display route is available**. First establish display ownership, framebuffer scanout and vblank/latch semantics; then add overlay composition. If required vendor firmware, secure resource assignments or drivers are unavailable, record the blocker and retain the Linux i.MX95 milestone rather than claiming Zephyr parity.

The i.MX95 backend shall characterize at minimum: number of actual usable planes per output, formats/modifiers, scaling/alpha/z-order constraints, DPU resource conflicts, synchronization and fault behavior. Do **not** copy i.MX8QXP/i.MX8QM DPU capabilities to i.MX95 without measurement or current i.MX95-specific documentation.

Relevant references: NXP i.MX95 Linux BSP documentation and source, Zephyr i.MX95 EVK board documentation, and upstream i.MX95 DPU/DSI/LVDS DRM patch series. Exact kernel/Zephyr revisions are part of the recorded baseline.

### 5.2 Secondary TI AM62P DSS adapter

Use the existing `signal-slot/slint-zephyr-yakogl-mixed-criticality-ui` implementation as a reference, particularly:

- `include/mcui/am62p_dss.h`
- `drivers/display/display_am62p_dss.c`
- `platform/fw/src/display_hw.c`
- `docs/display-sharing.md`

The existing AM62P path displays an A53-generated application frame (RGB565) beneath an R5F-owned safety plane (ARGB8888) using DSS hardware overlay. Preserve its operational invariants: latched-vsync acknowledgement, safe double/triple buffering, single ownership of GO/commit, ordered safety/background transitions, and resilience when the rich-UI producer stops.

Initially wrap the existing driver. Avoid an invasive rewrite until the shared API is validated in real hardware.

### 5.3 i.MX8M Plus as a contrasting design

Existing Route 3 uses an M7 software blend into an A53-produced frame before LCDIF scanout. This **is not** equivalent to an additional hardware overlay plane. Use it to inform the future pre-composition interface and buffer ownership model, but do not count it as a hardware-overlay implementation or a Milestone 1 deliverable.

Reference material:

- `platform/fw_imx8mp/src/app_frame.c`
- `platform/fw_imx8mp/docs/integration.md`
- `platform/fw_imx8mp/docs/step7-60fps.md`

### 5.4 Additional MCU/SoC targets (subsequent phases)

Candidate backends, dependent on board availability and hardware verification:

- NXP i.MX RT1170 / LCDIFv2
- Renesas RA8P1 / GLCDC and DRW
- STM32H7 / LTDC and DMA2D
- Broadcom Raspberry Pi 4/5 / HVS
- Rockchip RK3588 / VOP2

Implement a backend only after documenting the SoC's actual display topology, plane/formats/scaling/alpha restrictions, framebuffer memory accessibility, synchronization semantics, and testable board configuration.

## 6. Composition planning and validation

The planning algorithm shall:

1. Accept a frame with zero or more ordered layers and their requirements.
2. Determine which layers can be scanned out directly or assigned to available hardware planes.
3. Account for plane-type constraints, shared scalers, alignment, pixel formats, modifiers, color handling, alpha/premultiplication, rotation, clipping, bandwidth, and resource conflicts.
4. Prefer a fully hardware-composed frame when possible; do not promise globally optimal energy or bandwidth use without measurements.
5. Run backend-specific final validation (e.g., atomic TEST_ONLY on Linux).
6. Return an explicit error/report listing unmet constraints when no legal assignment exists.
7. Preserve protected or priority layers when requested, otherwise reject the frame rather than silently weakening the priority policy.

The first planner can use bounded exhaustive search or backtracking for small plane/layer counts. Benchmark and optimize only after correctness and hardware validity are established. Avoid inventing a single universal cost model before data is available.

## 7. Buffer, synchronization, and failure semantics

Required behaviors:

- No scanout of a partially rendered/unready buffer.
- No reuse of a displayed or pending buffer before the backend reports it safe.
- Distinguish **accepted/submitted**, **latched/presented**, and **released** events.
- Make dropped/overwritten pending frames observable; never imply every submission reaches the screen.
- Handle VSYNC, missing interrupts, atomic commit failure, hotplug, modeset, lease revocation, stale producers, and backend reset.
- Specify how color format, alpha premultiplication, pitch, and memory layout are validated.
- On MCU platforms, explicitly document memory protection, cache clean/invalidate, DMA visibility, and shared-memory producer trust boundaries.
- Provide a deterministic fallback policy on producer failure; do not assume the display can stay operational when the underlying controller or its owner crashes.

For safety-related use cases, separate the engine's requested priority from *enforced ownership/protection* on the actual SoC. Demonstration of an R5F safety plane surviving an A53 application crash is evidence of a particular configuration, not a general safety guarantee.

## 8. Work packages and deliverables

### WP0 — Baseline and requirements

- Inspect existing display paths and identify reusable invariants and hardware assumptions.
- Document the i.MX95 DRM display topology and Zephyr/DPU feasibility first; document AM62P DSS as a comparison platform.
- Record representative scenarios: two independent UI layers; video + UI; application + persistent priority/safety layer; unsupported geometry; producer crash/restart.

**Deliverables:** architecture note; platform capability matrix; test scenarios; initial API proposal.

### WP1 — `no_std` composition core

- Implement output/layer/buffer schema, fixed-capacity planning, resource constraints, and presentation state machine.
- Introduce backend interface and error taxonomy.
- Implement deterministic host-only tests and a fake backend that simulates delayed VSYNC, failed commits, and resource contention.

**Deliverables:** Rust library, C-facing headers (where required), API docs, unit tests, test vectors.

### WP2 — i.MX95 Linux DRM/KMS and AGL lease mode

- Accept a DRM lease fd obtained via drm-lease-manager/libdlmclient adapter.
- Discover leased resources and import/register framebuffers.
- Implement TEST_ONLY validation, atomic submission, event handling, and safe buffer release.
- On i.MX95, demonstrate at least two independently backed hardware planes using a minimal producer harness and actual DRM resources. If the selected BSP cannot expose those planes through a lease, investigate the driver/lease limitation explicitly and do not substitute flattened output as proof.

**Deliverables:** i.MX95 Linux backend and capability report, AGL integration example, automated validation where possible, captured logs and measured results.

### WP3 — i.MX95 Zephyr display integration

- Establish the actual i.MX95 core/display ownership and boot/power/clock configuration; prove framebuffer scanout and VSYNC/latch reporting first.
- Implement the shared composition contract for i.MX95 DPU where driver access and hardware permits. Demonstrate two-plane overlay only after hardware capability is verified, with explicit errors for unsupported configurations.
- Compare Linux and Zephyr on the same i.MX95 board using the same scene definition, with explicit differences in supported features and synchronization.
- If low-level access or driver support blocks Zephyr hardware overlay, deliver an evidence-based gap report and a runnable reduced-scope bring-up; do not quietly change M1 to a different SoC.

**Deliverables:** i.MX95 Zephyr adapter or documented blocker, firmware example, hardware test report and traces.

### WP4 — Secondary AM62P integration and portability

- Adapt the existing working AM62P R5F DSS path to the common model without regressing its safety-layer behavior.
- Validate two-plane composition, latch-aware buffer reuse, priority-layer transitions and producer restart/failure behavior.
- Contrast the AM62P pipeline with i.MX95 and document which constraints are truly generic.
- Evaluate the next MCU candidate (i.MX RT1170 or RA8P1), subject to board availability; implementation is optional until i.MX95 is demonstrated.

**Deliverables:** AM62P adapter and regression report; cross-SoC portability report; optional third-target prototype.

### WP5 — AGL upstream-readiness evaluation

- Compare the standalone leased-output use case with Weston-owned output requirements.
- Identify overlap with existing Weston/libliftoff machinery and document interfaces or patches needed for contribution.
- Prepare minimal documentation, reproducible demo, benchmark evidence, project licensing and contribution guidelines.

**Deliverables:** upstreaming proposal and gap analysis; no guarantee of upstream acceptance.

## 9. Priority milestones and acceptance criteria

### M1 (highest priority): i.MX95 Linux DRM / AGL hardware composition

**Goal:** On an i.MX95 development board, drive at least two independently supplied buffers through real display-controller hardware planes, using the shared API and a leased DRM output when the chosen BSP/lease configuration exposes the necessary DRM objects.

**Acceptance conditions:**

1. The common core builds in `no_std` mode; host tests and Linux i.MX95 builds pass.
2. The actual i.MX95 display-controller topology and plane restrictions are captured from the target board and driver, not inferred from other i.MX generations.
3. At least two distinct hardware planes display concurrently, with verified z-order and applicable alpha behavior. A flattened/composited single framebuffer does **not** count.
4. Supported and unsupported layer combinations are differentiated by capability detection, planner validation, and DRM atomic TEST_ONLY where available.
5. Submission, hardware latch/presentation, and buffer release are separately observable; repeated buffer rotation shows no unsafe overwrite in defined tests.
6. The AGL leased-output demonstration works without a Weston fork, or a reproducible kernel/BSP/lease-manager limitation is recorded as a blocking issue rather than hidden.
7. The test report identifies EVK revision, display interface/panel, BSP/kernel commit, lease-manager version, resolution and refresh, plane assignments, observed errors, frame loss, latency, CPU and memory usage.

**Exit rule:** M1 is incomplete without verified real multi-plane composition on i.MX95.

### M2: i.MX95 Zephyr composition

**Goal:** Reuse M1's layer/transaction model on i.MX95 under Zephyr; first establish scanout, then hardware overlay if the required driver/resource access exists.

**Acceptance conditions:** working target display and explicit ownership model; consistent buffer and synchronization semantics; demonstrated hardware overlay on i.MX95 if supported; documented limitations for any unsupported property. If the BSP/firmware prevents the required access, M2 remains blocked, with a reproducible investigation report, and is not declared passed based on a different SoC.

### M3: AM62P portability regression

**Goal:** Integrate the existing AM62P Zephyr/R5F DSS composition as a second, contrasting hardware backend; preserve its safety-layer/producer-failure behavior and validate common API portability.

**Acceptance conditions:** two-plane composition, correct VSYNC latch tracking, safe frame release, no visible intermediate-state regression, and a recorded difference matrix versus i.MX95.

## 10. Quality, CI and instrumentation

- Unit tests for planner conflicts, scaling limits, format mismatch, z-order, missing resources, and event order.
- Fake-backend property/fuzz tests for malformed frame descriptions and adversarial event ordering.
- Hardware integration tests at startup, steady state, app stop/restart, lease loss, modeset, and display recovery.
- Structured traces for `validate`, `commit`, `latched`, `released`, dropped frames, and resource assignments.
- Optional image comparisons or camera-based verification for physical output (not just framebuffer hashes).
- Performance metrics: plane assignment time; time from ready buffer to latch; end-to-end presentation latency; dropped/late frames; CPU and memory consumption; DMA/display bandwidth when measurable; power where instruments are available.

## 11. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Lease lacks required CRTC/planes or drivers restrict leased objects | Probe actual leased resources; report unsupported topology; use a different lease configuration/board |
| Linux DRM property behavior differs across drivers | TEST_ONLY, capability probing, backend quirks and hardware CI |
| Shared scalers/bandwidth cause unexpected assignment failures | Model resource conflicts; preserve evidence from failed atomic tests |
| i.MX95 DPU/DRM upstream-vs-vendor differences obstruct plane/lease behavior | Pin BSP/kernel revision; inspect actual topology and patch status; reproduce limitations before choosing workarounds |
| i.MX95 Zephyr board support does not cover required composition mode | Verify ownership/driver availability in WP0; gate M2 on real scanout, then actual overlays; report blockers |
| Existing AM62P behavior regresses during extraction | Adapter-first strategy; retain baseline demos and regression tests |
| Presentation events are mistaken for buffer release | Explicit separate lifecycle events and state machine |
| Cache/DMA/physical memory handling differs on MCUs | Backend-specific buffer import and explicit memory coherency contracts |
| Safety-layer priority is confused with functional-safety certification | Document limits and platform-enforced isolation separately |
| Scope expands into new Wayland compositor or GPU renderer | Keep them out of M1; publish integration/extension interfaces only |
| Existing Weston/libliftoff capabilities are reimplemented unnecessarily | Compare before implementation; reuse rather than replace where feasible |

## 12. Repository structure (proposal)

```text
composition-engine/
  Cargo.toml
  crates/
    composition-core/         # no_std data model, planner, state machine
    composition-backend-api/  # backend contracts
    composition-drm/          # Linux DRM/KMS + atomic
    composition-dlmclient/    # optional DRM lease acquisition adapter
    composition-imx95/        # i.MX95 display-controller Zephyr adapter
    composition-am62p/        # secondary AM62P Zephyr DSS adapter
    composition-ffi/          # stable/versioned C-facing integration
  examples/
    imx95-drm-leased-output/
    imx95-zephyr/
    am62p-zephyr/
    host-simulator/
  tests/
    scenarios/
    planner/
    presentation/
  docs/
    architecture.md
    platform-capabilities.md
    buffer-lifecycle.md
    agl-integration.md
    validation.md
```

Repository and package names above are placeholders; naming and licensing require a separate decision. Existing code remains in its original repository until migration is justified by tests.

## 13. Dependencies and assumptions

- Primary hardware access: an NXP i.MX95 EVK or equivalent i.MX95 board with working display, documented board revision, and Linux BSP is required for M1.
- Zephyr i.MX95 display ownership, peripheral access and driver availability must be verified; M2 has an explicit feasibility gate.
- An AM62P setup remains a secondary reference for M3, not a prerequisite for M1.
- Linux target exposes usable atomic DRM resources and a valid lease path; this must be established in WP0, not assumed.
- Existing mixed-criticality UI code can be used as a reference and adapter integration target.
- Platform-specific firmware, bootloader, driver, and SoC permissions remain external dependencies.
- Additional MCU boards and 2D engines are optional until i.MX95 M1 (and the M2 feasibility decision) is complete.
- The work is organized by technical milestones; cost, staffing, contractual dates, and commercial deliverables are not specified in this draft.

## 14. References

- i.MX95 upstream DPU/DSI/LVDS series: https://www.mail-archive.com/dri-devel@lists.freedesktop.org/msg570400.html
- Zephyr i.MX95 EVK board documentation: https://docs.zephyrproject.org/latest/boards/nxp/imx95_evk_15x15/doc/index.html
- AGL DRM Lease Manager and `libdlmclient` documentation/source (verify branch/version during WP0).
- Linux DRM/KMS atomic modesetting and leasing interfaces; DMA-BUF and synchronization interfaces.
- Weston/libweston DRM backend and libliftoff, for integration and reuse assessment.
- Signal Slot existing source:
  - https://github.com/signal-slot/slint-zephyr-yakogl-mixed-criticality-ui
  - https://github.com/signal-slot/slint-zephyr-yakogl-mixed-criticality-ui/blob/main/include/mcui/am62p_dss.h
  - https://github.com/signal-slot/slint-zephyr-yakogl-mixed-criticality-ui/blob/main/docs/display-sharing.md
  - https://github.com/signal-slot/slint-zephyr-yakogl-mixed-criticality-ui/blob/main/platform/fw_imx8mp/docs/integration.md
  - https://github.com/signal-slot/yakogl

---

**Working definition of success:** first, a small standalone `no_std`-capable composition core driving real hardware overlays on an **i.MX95 Linux/AGL DRM leased output**; next, a verified i.MX95 Zephyr display backend (or clearly reported hardware/firmware blocker), and then an AM62P adapter demonstrating cross-SoC portability. Explicit synchronization, ownership and capability constraints are required throughout.
