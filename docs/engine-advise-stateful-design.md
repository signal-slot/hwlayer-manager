# Design note: stateful `Engine::advise` (roadmap after WP5-03)

Status (2026-11-28): **proposal. No code, no header change.** This note covers one roadmap item: an `advise` that lives in the `Engine` and keeps state between frames. It does not edit [`advisory-planner-design.md`](advisory-planner-design.md) (WP5-03), which stays the reference for the stateless form.

Tags: **read from source (2026-11-28)** is what the tree at commit `aca3066` does today. **Inferred** is reasoning from that, and covers all of the proposal. Nothing here is observed on hardware, nothing is measured, and no upstream acceptance is implied. Sizes and costs in §3 are estimates until a `size_of` test and a host timing exist (**not measured**).

## 1. Why state at all (inferred)

A compositor calls the advisor once per repaint. Two things hurt when the advisor has no memory:

1. **Flapping.** A small change in one frame (a view moves, a client swaps its buffer format for one frame, a surface fades) can change which contiguous run is composed. The plane set then changes from frame to frame, and each change can cost a modeset-like reconfiguration or a visible glitch on some hardware. The stateless search is deterministic per input, but it is a function of the current frame only, so it has no reason to stay where it was.
2. **Repeated refused TEST_ONLY.** If the backend refuses an assignment, the stateless search re-discovers that on every frame and spends the same `backend_checks` budget (default 32) on it each time.

This note proposes memory for three things: the previous assignment, hysteresis against flapping, and failed-commit feedback. Whether any of it helps a real compositor is **not measured**; there is no board or compositor data. The first test that could say so is the host replay of §6.

## 2. Current behaviour (read from source, 2026-11-28)

* `composition_core::advise` is a free function: `advise(frame, caps, buffers, options, accept) -> Result<Advice, AdviceError>` with `accept: FnMut(&Plan, Option<usize>) -> Verdict` ([`crates/composition-core/src/advise.rs:273`](../crates/composition-core/src/advise.rs)). Its doc comment says "Nothing is remembered between calls, and the same input always gives the same advice."
* It tries the frame as it is, then every contiguous run in order of increasing length, lower runs first, and stops at the first the planner and `accept` take (module comment, `advise.rs:9-17`). `MAX_CANDIDATES = 1 + MAX_LAYERS*(MAX_LAYERS+1)/2` (`advise.rs:33`).
* `AdviseOptions { limits: SearchLimits, max_candidates, compose: Option<ComposeTarget> }` (`advise.rs:206`). `SearchLimits { steps, backend_checks }` (`planner.rs:194`). `steps` bounds each candidate; `backend_checks` bounds the whole call.
* `Advice` is `Copy` and fixed-size: `FixedVec<LayerAdvice, MAX_LAYERS>`, `Option<Assignment>`, `compose_run: Option<(u8, u8)>`, two counters (`advise.rs:99-106`). Constants are in `limits.rs`: `MAX_OUTPUTS` 4, `MAX_PLANES` 16, `MAX_LAYERS` 16, `MAX_BUFFERS` 64.
* There is **no `Engine::advise`**. `Engine` holds `generation: u32` (`engine.rs:55`), which `commit` and `Plan` use to reject a stale plan (`engine.rs:296`, `:319`, bumped at `:531`). `Engine::validate` is the planner plus `DisplayBackend::check`, and "Nothing on the display changes" (`engine.rs:207-210`).
* `Layer` has no identity across frames; it names a buffer, and decision 3 of the design note kept `Layer` unchanged. `Plan` keeps a private frame `fingerprint` (`planner.rs:237`, `:275`).
* C: `ce_advise`, `ce_explain_layer`, `ce_abi_check_advise` and `CE_ERR_NO_COMPOSE_PLANE` exist ([`composition_engine.h`](../crates/composition-ffi/include/composition_engine.h), lines 62-63 and 386-460). The engine storage is `CE_ENGINE_SIZE` 28672 bytes, aligned to `CE_ENGINE_ALIGN` 8, and a test in `lib.rs` (about line 1359) requires the real size to be at most that and more than half of it. The header has no version macro; compatibility is checked by size functions (`ce_abi_check`, `ce_abi_check_advise`).
* `composition-core` has no dependencies, no allocation, `forbid(unsafe_code)`, and builds for `thumbv7em-none-eabihf`. Unsafe code lives only in `composition-ffi`.

## 3. Proposed state (inferred)

Everything is a fixed array. No allocation, no new dependency, no unsafe code in core.

```rust
/// What the engine remembers of the last advice for one output. Proposal.
#[derive(Clone, Copy)]
pub struct AdviseMemory {
    output: OutputId,
    /// `Engine::generation` and the capability version the memory belongs to.
    /// A mismatch clears everything (hotplug, mode change, probe).
    epoch: u32,
    /// Frame counter of the last call, from the caller or the engine clock.
    last_tick: u64,
    layers: FixedVec<RememberedLayer, MAX_LAYERS>,
    compose_run: Option<(u8, u8)>,
    /// Plane of the composition target.
    compose_plane: Option<u8>,
    /// Frames in a row for which a run with fewer composed layers was legal.
    better_streak: u16,
    refused: FixedVec<Refusal, MAX_REFUSALS>, // MAX_REFUSALS = 8
    last_advised: Option<u64>, // signature of the last returned advice
}

#[derive(Clone, Copy)]
struct RememberedLayer {
    key: u64,                 // caller's layer identity, see below
    plane_index: Option<u8>,  // None: composed or hidden
}

#[derive(Clone, Copy)]
struct Refusal {
    signature: u64, // FNV-1a over (layer key, plane_index, zpos, source, destination) per assignment
    until_tick: u64,
}
```

`Engine` holds `[Option<AdviseMemory>; MAX_OUTPUTS]`.

**Size (inferred, not measured).** Per output about 16 x 16 B for layers, 8 x 16 B for refusals and a few dozen bytes more: roughly 0.4 KiB, so 1.6 KiB for four outputs. `CE_ENGINE_SIZE` has to cover it. Whether the current 28 672 B has room is unknown until `size_of::<Engine<CBackend>>()` is printed; the existing "more than half" test shows the block is already sized to fit tightly. If it does not fit, `CE_ENGINE_SIZE` grows, which is an ABI change (§5). A `size_of` assertion test pins the result.

### 3.1 Layer identity

Across frames the advisor must know that "layer 2 now" is "layer 2 before". Buffer ids are a poor key: a swapchain rotates them every frame. `Layer` is kept unchanged (decision 3), so identity is a **side input**: the caller passes a key per layer, in the order of `frame.layers`.

* Rust: `keys: &[u64]` in `StatefulOptions`, same length as `frame.layers`, or empty. Empty or a length mismatch means "no identity": the layers are matched by index and stacking position, which is weaker and tagged as such in the result (`Advice::identity_used()`).
* C: `const uint64_t *layer_keys` and a count, may be `NULL`.
* A Weston-like caller would use something stable per view; which one is **inferred** and unread (see `docs/wp5-gap-analysis.md` for what has been read of Weston).

### 3.2 Hysteresis (the previous assignment)

The stateless search is the engine; the stateful form changes only the **order and acceptance** of candidates. The planner is not modified.

1. **Previous first.** If the memory has a run `R_prev` and the layer keys still match, try `R_prev` first (a single candidate, one planner search plus a check).
2. **Demote at once.** If `R_prev` is no longer legal (planner or check refuses), fall back to the stateless order and take the first legal candidate. Legality is never traded for stability.
3. **Promote late.** If `R_prev` is legal, also find the stateless best candidate `R_best`. If `R_best` composes fewer layers than `R_prev`, count `better_streak += 1`; return `R_prev` until `better_streak >= hold_frames`, then switch and reset. If `R_best` composes the same number or more, keep `R_prev` (sticky) and reset the streak.
4. **Different run, same size.** Keep `R_prev`. The stateless tie-break (lowest run) is what flaps when a layer appears below the run.
5. **Dwell on the clock.** `hold_frames` is in frames, counted by calls for one output. A time-based form (`hold_ns` using the engine's `advance_time` clock, `engine.rs:173`) is the alternative; frames are chosen because the engine clock is a test hook and caller-driven.

What this does **not** do (inferred): it does not make plane choice inside a fixed run sticky. The planner picks planes deterministically for a given frame, but if a layer is added the plane of other layers can still move. Stickiness there needs a preference input to `plan_with` (a hint of plane per layer), which is a change to `planner.rs` and is out of this note; flag it as open question Q4.

The cost: `R_best` still has to be computed each frame to know if a better run exists, so the stateful call costs at least the stateless search plus one candidate. The step and check budgets of `AdviseOptions` stay as they are. **Not measured.**

### 3.3 Failed test-commit feedback

Two kinds, both bounded.

* **Inside the call.** Today `Verdict::TryNext` from `accept` means "this assignment does not work". The stateful form hashes each refused candidate into `refused` with `until_tick = tick + refusal_ttl_frames`. On later frames, a candidate whose signature is in the table is skipped without calling `accept`. This saves `backend_checks`. The signature includes the layer keys and geometry, so a changed frame produces a different signature and is tried again. The ring overwrites its oldest entry when full.
* **After the call.** A test commit that passed can still fail at the real commit, or a later frame can fail where the same assignment passed. The compositor tells the engine: `report_outcome(output, outcome)` with `Outcome::CommitFailed` (mark `last_advised` refused), or `Outcome::Committed` (nothing, optional). A `CommitFailed` also resets `better_streak` and forces step 2 of §3.2 on the next call.

Why a TTL and not a permanent ban (inferred): TEST_ONLY failures can depend on things the signature does not see (memory bandwidth, another output's state, a modifier that works only with other buffers). A permanent ban could hide a legal assignment forever. Clearing on `epoch` change, on TTL, and on a changed signature are the three exits. How long the TTL should be has no data; it is a caller option with a default that is not decided (Q3).

`Advice` gets a new reading: `Blocker::Backend` already exists; a skipped candidate is reported as `Blocker::Backend` too, with a flag `from_memory` so a log can tell "refused now" from "refused earlier". Whether to add the flag or a new `Blocker` variant is open (Q5; the stateless design chose no new `RejectReasons` bits, decision 5).

## 4. Proposed Rust API (inferred)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatefulOptions<'a> {
    pub advise: AdviseOptions,
    /// One key per layer of the frame, or empty.
    pub layer_keys: &'a [u64],
    /// Frames a better (smaller) composed run must be legal in a row before
    /// the advice switches to it. 0 = switch at once, still demote at once.
    pub hold_frames: u16,
    /// Frames a refused candidate is skipped. 0 = remember nothing.
    pub refusal_ttl_frames: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome { Committed, CommitFailed }

impl<B: DisplayBackend, T: Trace> Engine<B, T> {
    /// `advise` with the engine's capabilities, registered buffers, limits
    /// and `backend.check`, plus the memory of earlier calls. Changes nothing
    /// on the display and never calls `backend.commit`.
    pub fn advise(&mut self, frame: &Frame<'_>, options: &StatefulOptions<'_>)
        -> Result<Advice, Error>;

    /// Feedback about the last advice for `output`.
    pub fn report_outcome(&mut self, output: OutputId, outcome: Outcome) -> Result<(), Error>;

    /// Forget the memory of one output (or all with `None`).
    pub fn reset_advise_memory(&mut self, output: Option<OutputId>);
}
```

* `&mut self` because the memory is updated. It does not touch buffer states, the event queue or `generation` (§6, test 1).
* The stateless `advise` is unchanged and remains the base. `Engine::advise` calls an internal function that takes `&mut AdviseMemory` and shares the candidate loop; the loop moves into a helper used by both, so the stateless path is the stateful path with an empty memory and `hold_frames = 0`. That identity is a test (§6, test 2).
* `AdviseMemory` fields are private; the type is exposed only if a caller-owned memory is wanted (Q1).
* `hold_frames == 0 && refusal_ttl_frames == 0` must give exactly the stateless advice.
* Name clash: the design note's earlier sketch used `Engine::advise(&mut self, frame, compose)`; this form replaces it with `options`. The existing stateless `advise` keeps its name in the `composition_core::advise` module; the method is not ambiguous, but docs must say which is meant.

## 5. Proposed C ABI (inferred)

Additive only. No existing struct changes size or layout, no existing function changes its signature.

```c
typedef struct ce_stateful_options {
	ce_advise_options advise;          /* as ce_advise */
	const uint64_t *layer_keys;        /* NULL, or layer_count keys */
	uint32_t layer_key_count;          /* must equal layer_count if non-NULL */
	uint16_t hold_frames;              /* 0: switch to a better run at once */
	uint16_t refusal_ttl_frames;       /* 0: remember no refusals */
} ce_stateful_options;

#define CE_OUTCOME_COMMITTED     0
#define CE_OUTCOME_COMMIT_FAILED 1

ce_status ce_engine_advise(ce_engine *engine, const ce_frame *frame,
			   const ce_stateful_options *options,
			   ce_advice *advice, ce_error_info *info);
ce_status ce_engine_report_outcome(ce_engine *engine, uint32_t output, uint32_t outcome);
void      ce_engine_reset_advise_memory(ce_engine *engine, int32_t output /* -1: all */);

bool ce_abi_check_advise_state(size_t stateful_options);
#define CE_ABI_CHECK_ADVISE_STATE() ce_abi_check_advise_state(sizeof(ce_stateful_options))
```

* `ce_advice` is reused. The `from_memory` flag (Q5) would need a bit in an existing field or a new field; a new field changes `sizeof(ce_advice)` and therefore `CE_ABI_CHECK_ADVISE()`. To stay additive the first version should avoid it, and report skipped candidates as `CE_BLOCKER_BACKEND` only.
* The `check` callback is the engine's backend `ops.check`, so `ce_engine_advise` takes none. (The stateless `ce_advise` takes one because it has no engine.)
* Engine-held state grows `ce_engine`. If `CE_ENGINE_SIZE` must grow, that is **not** additive: an old program that gives 28 672 B to `ce_engine_init` gets `NULL` back (`ce_engine_init` rejects `size < size_of::<ce_engine>()`, `lib.rs:761-766`). That is a safe failure, not memory corruption, but it breaks the program. `ce_abi_check` already compares `CE_ENGINE_SIZE` (`lib.rs:726`), so a rebuilt caller finds out at start-up.

### 5.1 Versioning note for the header (inferred)

The header has no version macro today. Options, in order of preference:

1. **Keep the size-check style.** New function `ce_abi_check_advise_state` for the new struct, as `ce_abi_check_advise` did for the stateless form (decision 7 of the design note). No macro. If `CE_ENGINE_SIZE` changes, the existing `ce_abi_check` already fails for an old header against a new library.
2. **Add `CE_ABI_VERSION`** as a plain integer macro, bumped when `CE_ENGINE_SIZE` or an existing struct changes, plus `uint32_t ce_abi_version(void)`. This is a new convention and the owner should decide it (Q6).
3. **Do not ship the engine form in a C header release that already has `ce_advise` users without telling them**: `scripts/check-ffi-header.sh` and the header/`lib.rs` rule (they change in one commit) apply; the header comment should say which functions need the larger engine block.

Which of `CE_ENGINE_SIZE` growth or a separate caller-owned memory block avoids the break is a real choice: a **caller-owned block** (`ce_advise_memory`, storage given to the call, like the engine storage) leaves `CE_ENGINE_SIZE` alone but moves the lifetime to the caller and is no longer "Engine-held". The roadmap item says Engine-held, so the main proposal is Engine-held; the caller-owned form is the fallback if growth is unwanted (Q1).

## 6. Tests it would need (inferred; host only)

1. **No side effects.** `Engine::advise` leaves buffer states, the event queue, pending submissions and `generation` unchanged, and never calls `backend.commit` (counting fake backend). This is test 9 of the stateless note, which does not exist yet.
2. **Equivalence at zero.** With empty memory, or `hold_frames = 0` and `refusal_ttl_frames = 0`, `Engine::advise` equals `composition_core::advise` for the same inputs. Random frames from the existing fake topologies and fuzz seeds (`crates/composition-fake`).
3. **No flapping.** A replay of a frame sequence in which one layer toggles legality for one frame: stateless advice changes the run, stateful with `hold_frames = N` does not until the streak reaches `N`. Count run changes over the sequence; assert stateful is not more than stateless.
4. **Demote at once.** When `R_prev` becomes illegal (planner or check), the very next call leaves it, whatever `hold_frames`. Legality of every returned advice is checked as in tests 2-3 of the stateless note (build the reduced frame, `plan` accepts it).
5. **Promote late, exactly.** With `hold_frames = N`, the switch happens on the N-th consecutive better frame, not before; a worse frame in between resets the streak.
6. **Refusal memory.** A fake `check` that refuses signature S: `accept` is called once for S within the TTL, then not again; after the TTL, it is called again; a frame that changes geometry or keys is not skipped. The call count never exceeds `backend_checks`.
7. **Feedback.** `report_outcome(CommitFailed)` marks `last_advised` refused and the next call differs; `Committed` changes nothing; a report with no prior advice, or for an unknown output, returns an error and changes nothing.
8. **Epoch.** A capability change or `probe` (the `generation` bump) clears the memory: the next call equals the stateless one.
9. **Identity.** Renamed buffer ids with the same keys keep the memory; no keys falls back to index matching and `identity_used()` says so; a key count mismatch is an error, not a silent fallback (decision to confirm, Q2).
10. **Bounds.** Refusal ring overflow overwrites the oldest and never panics; `n = MAX_LAYERS`, four outputs, full rings: no allocation, no panic, terminates.
11. **Determinism.** The same call sequence gives the same advice sequence (replay twice).
12. **Fuzz.** Extend `crates/composition-fake/tests/advise_fuzz.rs` to random sequences with random outcome reports; invariants: every advice is legal, memory size is constant, no panic.
13. **ABI.** In the style of `crates/composition-ffi/tests/advise.rs`: `ce_abi_check_advise_state`, `NULL` keys, key count mismatch, `layer_count > CE_MAX_LAYERS`, bad `output`, a corrupt outcome value, `*advice` untouched on error; and a C file compiled against the header in the staticlib test (`crates/composition-ffi-staticlib/tests/c/`).
14. **Size.** `size_of::<AdviseMemory>()` and `size_of::<Engine<CBackend>>()` assertions; the existing "at most and more than half of `CE_ENGINE_SIZE`" test still passes, or `CE_ENGINE_SIZE` is changed together with the header.
15. **Builds and checks.** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, and both `thumbv7em` builds (`composition-core` with the new module; the FFI staticlib link), as in `docs/ci.md`; `scripts/check-ffi-header.sh`.

Not testable here: that a compositor sees fewer glitches or lower cost. That needs a compositor trial on hardware (roadmap) and is **not measured**.

## 7. Dependence on the §6 owner decisions of `advisory-planner-design.md` (inferred)

| §6 item | Taken so far | Effect on this proposal |
| --- | --- | --- |
| 1 One contiguity buffer or several | one contiguous run | Hysteresis is defined on one run `[lo, hi]`. A `k`-target or subset model needs a different memory shape and a new streak rule. **Blocks the design if changed.** |
| 3 Greedy by count or weight | minimal count, lowest run on ties | §3.2 rule 3 ("fewer composed layers is better") and rule 4 depend on it. A weight model changes what "better" means. **Depends.** |
| 4 Per-layer input errors | whole-call errors | Key count mismatch (§3.1) follows the same style; consistent, but Q2. |
| 5 New `RejectReasons` bits | none, `Blocker` enum | A `from_memory` indication must not add `RejectReasons` bits; Q5. |
| 6 Engine form or stateless only | stateless only; engine form **not decided** | **This note is that decision.** The owner must accept an engine form before any code. |
| 7 Versioning the ABI | `ce_abi_check_advise` | §5.1 option 1 extends it; option 2 reopens it. |
| 8 Advice stability | deterministic per input | Stateful advice is deterministic per **call sequence**, not per input. Item 8 must be restated, not just kept. **Depends.** |
| 9 Relation to `Plan` | no conversion | Unchanged; `Engine::advise` returns no `Plan`. |
| 10 Buffer state checks | metadata only | Unchanged. |
| 11 Capability source for non-engine caller | caller's `&Capabilities` | The engine form uses the engine's own; a non-engine caller has no memory unless the caller-owned form of Q1 is chosen. |
| 2, 12, 13 | advise only; place; no-target case | Not affected. |

## 8. Open questions for the owner

* **Q1.** Memory inside the engine (grows `CE_ENGINE_SIZE` if there is no headroom) or in a caller-owned block (no growth, but not engine-held and the caller handles the lifetime).
* **Q2.** Layer identity as a side array of keys, or a field in `Layer` (reverses decision 3 and changes `ce_layer`). And what a missing or mismatched key array does: error or index matching.
* **Q3.** Defaults and units: `hold_frames`, `refusal_ttl_frames`, frames or time. No data supports any value.
* **Q4.** Whether plane stickiness inside a run is wanted. It needs a preference input in the planner.
* **Q5.** How a refusal from memory is reported: same `Blocker::Backend`, a flag, or a new variant.
* **Q6.** Header versioning: size-check functions only, or an `CE_ABI_VERSION` macro.
* **Q7.** Whether the engine's `advise` replaces `Engine::validate` for compositors, or both stay (this note keeps both).

## 9. What this note does not claim

* Nothing is implemented, and nothing was run; the current-behaviour section is a reading of the source at `aca3066`.
* No size, timing, power or glitch figure is claimed. The estimates in §3 are not measured.
* No claim that Weston or libliftoff has the same problem, or that they would take this; neither was read for this note. No upstream acceptance is implied.
* No hardware: no board or compositor has used the stateless form either (see the design note, §7).
