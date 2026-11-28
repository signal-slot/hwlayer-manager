# i.MX8M Plus Route 3: pre-composition and buffer ownership (design note)

Desk design, SoW.md §5.3. **Nothing here was run or observed on a board, nothing is implemented, and nothing here is a hardware or timing claim.** No frame rate appears in this note; where one would matter it is marked *not measured*.

## 0. Status, stated plainly

- Route 3 is **not a hardware overlay**. The Cortex-M7 draws into pixels with software; LCDIF then scans out one plane. The SoW says so (§5.3: "This **is not** equivalent to an additional hardware overlay plane").
- Route 3 is **not an M1 deliverable**, and must not be counted as a hardware-overlay implementation (SoW §5.3). This note does not change that. Its only use is to inform a *future* pre-composition interface and buffer ownership model.
- This note proposes an interface and does not implement it. No file under `crates/` changes.

Tags:

- **read from source**: from this repository's code or docs, or from the SoW text.
- **inferred**: a conclusion drawn from those; the sources do not state it.
- **from the private reference (paraphrased)**: from the private showcase repository. See §1: none were read.

## 1. Reference material: absent on this node

The SoW (§5.3) names three files in the private showcase repository: the firmware's frame-handling source file (`platform/fw_imx8mp/src/app_frame.c`) and two documents under `platform/fw_imx8mp/docs/` (`integration.md`, `step7-60fps.md`).

The environment variable that points at the private showcase checkout was set on the node that wrote this note, but the directory it names did not exist there, so none of the three files could be opened. **No claim in this note is "from the private reference".** Everything about Route 3 below comes from the SoW's one sentence plus general design reasoning, and is tagged *inferred*. In particular the note says nothing about how the existing firmware really lays out its buffers, which synchronisation primitive it uses, or what it measured. Whoever has the checkout should compare §3 and §4 against it and correct them (open question Q1). Nothing was copied, and nothing from that repository is reproduced.

## 2. What Route 3 is (SoW text, plus inference)

**Read from source (SoW §5.3):** on the i.MX8M Plus, the existing Route 3 has the M7 software-blend content into a frame that the A53 produced, before LCDIF scans it out.

Roles that follow from this sentence (**inferred**):

| Role | Core | Does |
| --- | --- | --- |
| Base producer | A53 (Linux) | Renders the rich-UI frame into a buffer. |
| Overlay source | M7 | Holds the content to blend on top (for example a safety or status graphic). It is software-drawn or a static asset. |
| Blender | M7, in software | Reads the A53 frame and the overlay source, writes the blended pixels. Whether into the A53 buffer or into a third one is a design choice, see §4. |
| Scanout | LCDIF | Scans one buffer per CRTC. It sees only the final pixels and cannot tell they were blended. |

Why it is not an overlay (**inferred**): a hardware overlay is composed by the display controller at scanout from two independent buffers, with no CPU on the pixel path and no change to the base buffer. In Route 3 the blend is a CPU write into memory before scanout, so it costs memory bandwidth and CPU time per frame, it has a producer-side deadline, and the base content is changed or copied. For the planner, hardware planes and a CPU blend are different kinds of resources.

The single plane per CRTC of LCDIF is the reason Route 3 exists at all. This note does not restate the LCDIFv3 facts; they are the subject of `docs/imx8mp-lcdif-source.md` (a separate item, which may not have landed when this was written).

## 3. Relation to composition-core today

What exists (**read from source**):

| Piece | Where | What it says |
| --- | --- | --- |
| `Plan` | `crates/composition-core/src/planner.rs:234` | A validated assignment of a frame's layers to hardware planes. Each `Assignment` (`planner.rs:212`) names a layer index, `PlaneId`, plane index, `zpos`, source and destination. Only layers on planes appear. |
| `plan()` / `plan_with()` | `planner.rs:302`, `:318` | Produce a `Plan` or a `PlanError`; they use a `BufferLookup` (`planner.rs:27`) for buffer metadata. They do not compose. |
| `advise()` | `crates/composition-core/src/advise.rs:273` | Advisory only: "Not a `Plan` and not accepted for a commit" (`advise.rs:96`). It already has the idea of layers that are not on a plane: `Placement::Compose` (`advise.rs:51`) and a `ComposeTarget` buffer that **the caller renders into** (`advise.rs:40`), with `Blocker` giving why a layer was not placed (`advise.rs:63`). |
| `DisplayBackend` | `crates/composition-core/src/backend.rs:48` | `capabilities()`, `check()`, `commit()`, `poll_event()`. The backend alone programs the display. `commit` success means accepted, not on screen. The only completion event is `BackendEvent::Latched` (`backend.rs:34`). |
| `Capabilities.replace_pending` | `crates/composition-core/src/caps.rs:172` | Whether a commit may replace one that is not latched yet. |
| Buffer lifecycle | `crates/composition-core/src/presentation.rs:1-20`, `engine.rs:193-198` | `Writable → Ready → Submitted → Scanning`. `begin_write`/`finish_write` are a promise by the caller. A buffer is released when the latch of the submission that replaced it is recorded (`Presentation::latched`, `presentation.rs:403`; `DisplayEvent::Released`, `presentation.rs:59`). |
| Fences | `docs/sync-fences.md` §1, §3 | The core has no fences. Readiness is a producer-declared state. The note proposes an opaque readiness token chosen by the producer-side glue and interpreted only by the backend (§3.1), and an optional earlier release event (§3.1, last bullet). Not implemented. |
| Advisory design | `docs/advisory-planner-design.md`, `docs/architecture.md` | The core plans and advises. Backends and callers act. |

Consequence (**inferred**): composition-core already models "a layer is drawn by the caller into a buffer, not placed on a plane" (`Placement::Compose`). Route 3 is that same idea with a specific *caller*: the M7, in a specific *place*: after the A53's frame exists, before LCDIF reads it. What is missing is a way to say that the composed buffer is produced by a **second agent** with its own ownership and synchronisation, and a backend-level notion of a stage that is not a plane.

## 4. Proposal

All of §4 is **proposal, not implemented**. Everything is **inferred** design, tied to the types above.

### 4.1 How a pre-composition stage appears to the planner

On a single-plane CRTC the planner can place at most one layer, so every other visible layer ends in `Placement::Compose` through `advise()`. Today that target is an ordinary buffer the caller draws into. Proposal:

- Keep `plan()` unchanged: a `Plan` still contains hardware planes only. A pre-composition stage is never an `Assignment`, which keeps "hardware overlay" and "software blend" distinct in the type system and keeps `Plan` honest for the §5.3 rule.
- Add the notion of a **compose executor** to the advice, not to the plan: who draws the `ComposeTarget`. Values: `Caller` (today, implied), and `Precompose` (a backend- or platform-provided stage that the caller does not draw on directly). Which executors a backend offers is described next to its planes in `Capabilities`, as a separate, optional list so that a backend without one is unchanged.
- The layer that stays on LCDIF's one plane is the **composed result**, not an input layer. `advise()` already produces this shape (a compose run plus a plane assignment for the target). Route 3 adds a statement of *where the blend happens*.
- A layer assigned to `Precompose` carries a reason code so reports can show it was not a hardware placement. A test or report that counts hardware overlays must count `Plan` assignments only, never composed layers.

### 4.2 Buffer ownership model

Three buffers per output in the smallest useful scheme (**inferred**):

| Buffer | Producer | Consumer | Owner while being written | Owner while being read |
| --- | --- | --- | --- | --- |
| `A` (A53 frame) | A53 | M7 blender | A53 | M7 (read only) |
| `S` (overlay source) | M7 or an asset loader | M7 blender | M7 | M7 (read only), never shown directly |
| `O` (blended output) | M7 blender | LCDIF (scanout) | M7 | The display backend, then LCDIF |

Rules:

1. Exactly one agent owns a buffer for writing at any time. The core's `Writable/Ready` states express this for the producer side already.
2. `O` is a different buffer from `A`. Blending in place in `A` would have the M7 write a buffer the A53 may reuse and would put a CPU write between the A53's `finish_write` and the latch. A third buffer costs memory and one copy or full-frame pass, and removes that shared write window (*not measured*; the cost depends on the content and is not estimated here).
3. Only `O` is in the `Frame` that is committed. `A` and `S` are never layers of a committed frame; they are inputs of the pre-composition stage. In the core's terms: `A` is released to the A53 when the M7 has finished reading it, which is an event earlier than any latch. The existing rule "release at the replacing latch" does not cover `A`.
4. Handover points:
   - H1: A53 `finish_write(A)`. A is `Ready`, pending signal, with a readiness token (sync-fences §3.1).
   - H2: M7 blend starts. It waits for the H1 token (acquire), then owns the M7 side of `O`.
   - H3: M7 blend ends. `O` becomes `Ready` with a token; the M7 signals "read of A done", which is the release of `A` to its producer.
   - H4: submission. `O` is submitted through `Engine::commit`; the backend `check()`s and `commit()`s.
   - H5: latch. `BackendEvent::Latched` makes `O` scanning and releases the buffer that `O` replaced (the previous output buffer).
5. Release of `A` and of `S` are driven by the M7's completion, not by `Latched`. Release of `O` is driven by `Latched` of its replacement, as in the core today.

### 4.3 Fences and events at each handover

Using `docs/sync-fences.md` terms (a readiness token that is opaque to the core, and a release that may be earlier than the replacing latch):

| Handover | Needed | Mechanism in the proposed model |
| --- | --- | --- |
| H1 (A53 → M7) | Acquire: M7 must not read `A` until the A53 finished writing | Readiness token for `A` (sync-fences §3.1), signalled by A53-side glue after `finish_write`. The M7 side of the glue waits for it. Which transport (shared-memory flag, mailbox, RPMsg) is open (Q2). |
| H3 (M7 → A53) | Release of `A`: the A53 may rewrite it once the M7 has read it | A release notification to the producer: the "release token" of sync-fences §3.1, last bullet, issued at M7 completion, not at a latch. |
| H3 → H4 (M7 → backend) | Acquire: LCDIF must not scan `O` before the blend ended | `O` is `Ready` with a readiness token that the display backend checks in `check`/`commit`. A backend that cannot wait on a token must refuse (`BackendError::Rejected`), as sync-fences §3.1 requires, never ignore it. |
| H5 (latch) | `O` is shown; previous `O` free | `BackendEvent::Latched` as today; `Presented`, then `Released` for the buffer replaced. No change. |
| Producer stops | The A53 stops producing | The M7 must be able to re-blend into a new `O` from the last `A`, or hold the last `O`. This is the same property that SoW §5.2 asks of the AM62P path ("resilience when the rich-UI producer stops"); here it is a requirement on the stage, not tested. |

Count of `O` buffers: if the M7 may start blending the next frame while the previous `O` is pending or scanning, at least two `O` buffers are needed (one scanning, one being written), three if a pending latch must not be replaced (`replace_pending`, `caps.rs:172`). Which one LCDIF supports is a fact for the LCDIF study, not assumed here.

### 4.4 Tearing and latch hazards

LCDIF scans one plane and takes a new framebuffer address at a vertical-blank boundary (**inferred** here from the "single plane, latch at vblank" description in the task; the register-level facts belong to `docs/imx8mp-lcdif-source.md`). Hazards:

| Hazard | Cause | Mitigation in the model |
| --- | --- | --- |
| Tearing | The M7 writes into the buffer LCDIF is scanning. | The M7 only writes `O` buffers that are neither `Submitted` nor `Scanning` (the core's states). It never blends in place into a scanning buffer. |
| Torn read of `A` | The A53 rewrites `A` while the M7 reads it. | `A` is released to the A53 only at H3. The A53 uses a second `A` meanwhile (double buffering on the A53 side). |
| Late blend | The blend ends after the latch point for frame N, so frame N repeats and the frame after shows both changes. | Not a corruption; the previous `O` stays. Reported as a repeated frame through the core's `Dropped`/`Presented` ordering, not by the stage. No timing claim is made about how often it happens (*not measured*). |
| Early commit | `O` is committed before the blend ended. | Forbidden by the H3→H4 token. The engine's `BufferNotReady` (`presentation.rs`, `check_submit`) is the failure mode for a buffer still `Writable`. |
| Overlay content changes mid-blend | `S` is updated while read. | `S` has its own owner (the M7) and is double-buffered by that owner if it changes at frame rate. |
| Cache coherence between A53 and M7 | The two cores see memory through different caches. | Out of the scope of this note. A buffer-flush/invalidate step belongs to the handover (Q3). |

### 4.5 Interface sketch (proposal, not implemented)

Names are illustrative. Nothing below is in `crates/`.

```text
// PROPOSAL, NOT IMPLEMENTED. composition-core would stay no_std, dependency-free.

/// Who draws the composition target of an advice.
pub enum ComposeExecutor { Caller, Precompose }

/// Optional capability of a backend or platform glue: a stage that
/// composes layers into a target before the display reads it.
pub struct PrecomposeCaps {
    pub executor_present: bool,
    pub max_inputs: u8,            // layers it can blend into the target
    pub formats: &'static [PixelFormat],
    pub output_buffers: u8,        // minimum count of target buffers
}

/// Stage contract. Not part of DisplayBackend: a plane backend is unchanged.
pub trait PrecomposeStage {
    fn caps(&mut self) -> PrecomposeCaps;
    /// Start blending `inputs` of `frame` into `target`. Does not block.
    /// `acquire` is the readiness token of the base buffer; None = ready.
    fn begin(&mut self, job: JobId, frame: &Frame<'_>, inputs: &[usize],
             target: BufferId, acquire: Option<ReadyToken>) -> Result<(), StageError>;
    /// Next event, if any. Must not block.
    fn poll_event(&mut self) -> Option<StageEvent>;
}

pub enum StageEvent {
    /// Reads of the inputs are done: input buffers may be released.
    InputsReleased { job: JobId },
    /// `target` is complete and may be committed; carries a readiness token.
    TargetReady { job: JobId, ready: ReadyToken },
    Failed { job: JobId, error: StageError },
}

pub enum StageError { Rejected, Busy, DeviceLost, Platform(i32) }
```

Prose:

- `PrecomposeStage` is deliberately **not** an extension of `DisplayBackend`: the display backend still programs the display and reports only `Latched`. A stage is a separate agent, usually on another core, that reports its own events.
- The caller (or the engine, in a later step) runs: `advise()` → for the `Compose` run, `PrecomposeStage::begin` → on `InputsReleased` release the inputs to their producers → on `TargetReady` mark the target `Ready` with its token → `Engine::commit` of a frame that holds the target as the single plane layer.
- `ReadyToken` is the opaque token of `docs/sync-fences.md` §3.1 (also **proposal, not implemented**). If that token is not adopted, a bool `ready` flag in a backend-defined table is a weaker alternative (sync-fences §3.3).
- A `Precompose` result must never be reported as a hardware overlay: `Plan` is unchanged and a layer composed by the stage has no `Assignment`.

### 4.6 Fit with the existing contract

- `DisplayBackend::check` and `commit` see an ordinary single-layer frame whose buffer is `O`. The LCDIF backend needs no knowledge of the M7 (**inferred**).
- The release semantics for `O` stay those of `Presentation::latched`. The new events apply only to the inputs `A` and `S`.
- `advise()` stays advisory. The stage contract does not make composed layers part of a `Plan`.

## 5. What Route 3 tells us, and what it does not

Tells us (**inferred**):

- A pipeline with two producers on different cores needs an explicit **input-release** event that is earlier than any display latch.
- A design where the display sees one buffer needs the blended output to be a separate, distinctly owned buffer, so that the "write while scanning" and "write while the producer reuses" windows are closed by ownership rather than by timing.
- The core's `Placement::Compose` already leaves room for this. The missing parts are who draws, and the early input release.

Does not tell us:

- Nothing about hardware overlay: LCDIF is not shown to have, or to lack, an overlay by this note. Route 3 gives no hardware-overlay capability and no M1 evidence.
- Nothing about timing: no frame rate, no latency, no CPU or bandwidth cost was measured or estimated. Any "60 fps" or similar figure from the reference material is *not measured here* and is not repeated.
- Nothing about correctness on the real board: the buffer flow in §4 was not tried.
- Nothing about the existing firmware's actual choices (§1).

## 6. Open questions

- **Q1.** Compare §3 and §4 with the private reference files when they can be opened. Are the roles, the third output buffer and the release order the same as the existing firmware's? Correct this note where they differ.
- **Q2.** Transport for tokens and events between A53 and M7 (shared-memory flags, mailbox, RPMsg). Not decided here.
- **Q3.** Cache maintenance and memory placement: which memory both cores can map, and which agent flushes or invalidates at H1 and H3.
- **Q4.** Does the blend write into a third buffer (`O`) every frame, or only the changed region? Depends on the overlay's size, not studied.
- **Q5.** Where does the stage's capability live: next to `Capabilities` of the display backend, or in a platform glue object outside the backend? This note leans to the second but does not decide.
- **Q6.** Does the engine run the stage itself, or only the caller? Running it in the engine would add the stage events to `DisplayEvent` handling.
- **Q7.** How is a `Precompose` placement shown in traces (`trace.rs`) so a report cannot mistake it for a hardware overlay?
- **Q8.** LCDIFv3 latch and buffer-count facts (`replace_pending`, minimum number of `O` buffers) from `docs/imx8mp-lcdif-source.md`, once it exists.
- **Q9.** Producer stops: should the stage keep re-blending the last `A`, or hold the last `O`?
