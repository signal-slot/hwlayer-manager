# Stateful `Engine::advise`: design note (roadmap after WP5-03)

Date of this note: 2026-11-28. Status: **proposal, no code**. Tags: **read from source (2026-11-28)** for what the repository does today (tree at `df507ed`); **inferred** for everything proposed; **not measured** for cost. Nothing here was run on a board and no upstream acceptance is promised.

Companion of [advisory-planner-design.md](advisory-planner-design.md) (not edited by this note). Its §6 decision 6 says the engine form is "**not decided**"; this note sketches it so the owner can decide.

## 1. Current behaviour (read from source, 2026-11-28)

* `composition_core::advise` ([`advise.rs`](../crates/composition-core/src/advise.rs)) is a free function over `&Frame`, `&Capabilities`, a `BufferLookup`, `&AdviseOptions` and a check closure `FnMut(&Plan, Option<usize>) -> Verdict`. Its doc says it "keeps no state between calls", and "the same input always gives the same advice". It tries the frame as it is, then every contiguous run, shortest first, lower runs first, and stops at the first accepted candidate.
* `Engine<B, T>` ([`engine.rs`](../crates/composition-core/src/engine.rs)) holds `backend`, `caps`, `generation`, `presentation` (buffer states, latches), `limits`, `latch_timeout`, an `unexpected` queue and a trace. Its public planning calls are `validate`, `commit` and `present`; there is no `advise` on it. `validate` calls `planner::plan_into` with a closure over `backend.check`, mapping `BackendError::Rejected` to `Verdict::TryNext` and other errors to `GiveUp`.
* The C side has `ce_advise` (stateless; the caller passes `ce_capabilities`, a `ce_buffer_entry` table, `ce_advise_options` and a `check` callback) and `ce_abi_check_advise` with `CE_ABI_CHECK_ADVISE()`, so `ce_abi_check` and `CE_ENGINE_SIZE` (28672 bytes in the header read) did not change. No `ce_engine_advise` exists.
* `composition-core` is `#![no_std]`, `#![forbid(unsafe_code)]`, has no dependencies and no allocation; sizes are fixed (`MAX_OUTPUTS` 4, `MAX_PLANES` 16, `MAX_LAYERS` 16, `MAX_BUFFERS` 64). `composition-ffi` holds the `unsafe`.

What a caller cannot do today: ask the engine (with its own backend, capabilities, buffer table and search limits) for advice; keep the last advice stable across frames; learn from a `TEST_ONLY` commit that failed.

## 2. What state could help (inferred)

Per output, the engine would remember:

1. **Previous assignment**: for each layer slot, its `Placement` last time (plane index or compose or hidden) and the identity it was keyed by (`BufferId`, `z_order`).
2. **Hysteresis**: a per-layer counter of consecutive frames in which the stateless answer differed from the held one.
3. **Failed test-commit feedback**: a small set of assignments, or single layer-to-plane pairings, that the backend refused, with the generation they were refused in.

Why: the stateless search is deterministic, but a small change in a frame (a layer's clip, a buffer swap to a format only some planes take) can move a layer between a plane and the composition run from one frame to the next. That "flapping" costs a re-render of the composition buffer and a plane reprogramming each time. Whether it occurs in practice on any board is **not measured**; the benefit is a hypothesis.

## 3. Proposed Rust API (inferred)

State lives in the `Engine`, as fixed arrays, no allocation:

```rust
/// Kept per output; Copy; no references.
struct AdviseState {
    output: OutputId,
    valid: bool,
    generation: u32,                       // caps generation it was made under
    layer_count: u8,
    slots: [SlotMemory; MAX_LAYERS],       // key + last Placement
    held_compose: Option<(u8, u8)>,        // last composed run
    pending: [u8; MAX_LAYERS],             // frames the alternative has won
    refused: FixedVec<Refusal, MAX_REFUSALS>, // ring, oldest dropped
}
struct Refusal { generation: u32, layer_key: u32, plane_index: u8, in_run: bool, age: u16 }

pub struct StickyOptions {
    pub hold_frames: u8,       // 0 = no hysteresis
    pub refusal_frames: u16,   // 0 = refusals are not remembered
}

impl<B: DisplayBackend, T: Trace> Engine<B, T> {
    pub fn advise(
        &mut self,
        frame: &Frame<'_>,
        options: &AdviseOptions,
        sticky: &StickyOptions,
    ) -> Result<Advice, Error>;
    /// Forget everything for one output (or all if None).
    pub fn advise_reset(&mut self, output: Option<OutputId>);
}
```

`Engine::advise` supplies `self.caps`, `self.presentation` as the `BufferLookup` and a closure over `backend.check` (as `plan_frame` does, `Rejected` to `TryNext`, other errors to `GiveUp`), then post-processes. The stateless free function stays as it is and remains the kernel; hysteresis is a wrapper around it, not a change to its search.

Sizes: `AdviseState` is bounded by `MAX_OUTPUTS * (MAX_LAYERS * a few bytes + MAX_REFUSALS * 12)`. With `MAX_REFUSALS` 16 that is a few hundred bytes per output (**inferred** estimate, **not measured**; a `size_of` test would pin it). `CE_ENGINE_SIZE` grows by that amount, which is an ABI change (§5).

### 3.1 Algorithm sketch (inferred)

1. If `caps.generation` differs from the state's, or the layer count or any key changed in a way that cannot be matched, drop the state (cold start = stateless answer).
2. Compute the stateless advice `A` (with refusals applied, step 4).
3. Hysteresis. Let `H` be the previous advice re-applied to this frame (same plane for the same layer key, same composed run). If `H` is still **legal** (the planner accepts it via the same check, one extra candidate) and differs from `A`, keep `H` while `pending < hold_frames`, increment `pending`; otherwise switch to `A` and reset. One rule must hold: a held advice is never worse in correctness, only possibly in composed-layer count. Whether a *larger* composed run for a few frames is acceptable is exactly owner decision D3 below.
4. Refusal feedback. The engine already sees `Rejected` from its own backend while advising, but those results are lost after the call. With state, an assignment refused in this generation is recorded and tried last (or skipped) for `refusal_frames` frames, saving repeat `TEST_ONLY` commits (each is a kernel round trip; the saving is **not measured**). A refusal made by a different check than the backend's (caller closure in the stateless form) would have to be reported by the caller: `Engine::advise_feedback(output, &Advice, Refusal reason)` is an alternative to the engine owning the check.
5. Update the state from the returned advice.

Determinism: the result becomes a function of the input *and* the state; the §6 decision 8 property "same input gives the same advice" holds only for the same history. Tests must say so (§6).

### 3.2 Constraints kept

* `no_std`, no dependency, no allocation: arrays and `FixedVec` only (as `Advice` does).
* `forbid(unsafe_code)` stays in `composition-core`; nothing new needs `unsafe` there.
* thumbv7em builds: the new code uses no `u64` division, no float, no recursion; the same two builds as the existing CI must pass, plus a check that `size_of::<Engine<..>>()` stays inside the C storage constant.
* Bounded time: the held-advice re-check is at most one extra candidate and one extra `accept` call per frame, counted in `backend_checks`.

## 4. C ABI counterpart (inferred)

```c
typedef struct ce_sticky_options {
	uint8_t  hold_frames;      /* 0: no hysteresis */
	uint16_t refusal_frames;   /* 0: refusals not remembered */
} ce_sticky_options;

/* Like ce_advise, but caps, buffers, backend check and limits are the engine's. */
ce_status ce_engine_advise(ce_engine *engine, const ce_frame *frame,
			   const ce_advise_options *options,
			   const ce_sticky_options *sticky,
			   ce_advice *advice, ce_error_info *info);
void ce_engine_advise_reset(ce_engine *engine, uint32_t output, int all);
```

Reuses `ce_advise_options`, `ce_advice`, `ce_layer_advice` unchanged; `ce_advise` itself is unchanged.

### 4.1 Versioning note

* `CE_ENGINE_SIZE` must grow (the engine holds the state). Existing callers that pass storage sized by the old header would get `NULL` from `ce_engine_init` (documented: too small); callers rebuilt against the new header are fine. That is a source-compatible, binary-incompatible change for storage size.
* Prefer the pattern of decision 7: a new check, `ce_abi_check_engine_advise(sticky_options)` with `CE_ABI_CHECK_ENGINE_ADVISE()`, plus the existing `ce_abi_check` that already compares `CE_ENGINE_SIZE`. Whether to add a `CE_ENGINE_ABI_VERSION` integer is an owner choice; the header has none today (**read from source**: no `VERSION` macro found in the header on 2026-11-28).
* `scripts/check-ffi-header.sh` must be extended for the new names; the header and `lib.rs` change in one commit, as for `ce_advise`.
* Adding functions does not break existing C callers; changing `ce_engine`'s size does only for static storage declared with a hard-coded number.

## 5. Tests it would need (inferred; none exist)

Host tests in `crates/composition-core/tests/` and `crates/composition-ffi/tests/`:

1. **Cold start equals stateless**: first `Engine::advise` on a frame equals `advise` with the same inputs (a fixed set of frames over the existing profiles).
2. **Same history, same advice**: replay a recorded frame sequence twice on two engines; advice sequences are equal (determinism with state).
3. **Hysteresis holds**: a layer that the stateless planner flips between plane and compose on alternate frames stays put for `hold_frames`, then moves; `hold_frames = 0` equals stateless on the whole sequence.
4. **Held advice illegal**: if the previous assignment no longer passes the planner or the check (format change, plane gone), the answer is the stateless one immediately, never a held illegal one.
5. **Generation change** (`probe`, caps changed): the state is dropped; first answer equals stateless.
6. **Failed test-commit**: a mock backend that rejects one assignment; the next frame does not re-offer it within `refusal_frames` (count `check` calls) and offers it again afterwards; a later generation clears it.
7. **Fatal backend error** (`DeviceLost`): the call returns the error and leaves the state unchanged.
8. **Layer-set changes**: layer added, removed, reordered, keys renamed: no stale slot is reused for another layer; the buffer-id renaming property of decision 8 still holds for cold starts.
9. **Bounds**: more than `MAX_REFUSALS` refusals drops the oldest; `MAX_LAYERS` layers; per-call `backend_checks` is honoured including the held re-check.
10. **`advise_reset`** for one output leaves the others; with `None` clears all.
11. **Size and layout**: `size_of::<AdviseState>()` pinned; `size_of::<Engine<..>>() <= CE_ENGINE_SIZE`; thumbv7em builds.
12. **FFI**: `ce_engine_advise` equals the Rust call; null and invalid arguments as for `ce_advise`; `ce_abi_check_engine_advise` accepts the mirror sizes and rejects a wrong one; a C caller linked to the static library runs a three-frame sequence.
13. **Header lint**: `scripts/check-ffi-header.sh` and `scripts/check-doc-tags.sh`/links pass on the changed header and docs.

## 6. Which §6 owner decisions it depends on

From [advisory-planner-design.md §6](advisory-planner-design.md#6-decisions) (the owner has settled none; the "taken" choices are the implementer's):

* **Decision 6 (stateless only or also engine form)**: this note *is* that decision. It cannot start before it is made.
* **Decision 7 (ABI versioning)**: sets how §4.1 is done (second check function vs. extra arguments with a version bump). The engine form changes `CE_ENGINE_SIZE`, so the owner should decide here whether a version integer is introduced.
* **Decision 8 (advice stability)**: stateful advice is deterministic only per history; the owner must accept the weaker statement.
* **Decision 3 (greedy by count or weight)**: hysteresis deliberately accepts a non-minimal composed-layer count for a few frames; it needs the owner to accept that count is not always minimal when sticky options are on (off by default keeps decision 3 intact).
* **Decision 9 (relation to `Plan`)**: unchanged; `Engine::advise` still returns no `Plan` and `commit` still does not take an `Advice`. A held advice is re-derived through `validate`/`present` as today.
* **Decision 1 (one composition buffer)** and **decision 2 (advise only)**: unchanged by this note; a different composition model would change the state's shape (`held_compose`).
* **Decision 11 (capability source)**: the engine form takes capabilities from the engine, which resolves it for engine callers only.
* **Decisions 4, 5, 10, 12, 13** are not touched.

New decisions this note adds for the owner (inferred): **D-a** whether refusal feedback is owned by the engine's backend check or reported by the caller; **D-b** default of `hold_frames` (proposal: 0, i.e. stateless behaviour until asked); **D-c** the value of `MAX_REFUSALS`.

## 7. What this note does not claim

* No code, header or test changed; nothing was built for this note beyond reading the sources.
* Flapping, the benefit of hysteresis and of remembered refusals are **not measured**; there is no board or host data in this repository showing either.
* No claim that libliftoff or Weston do this; their handling was not read for this note.
* No upstream acceptance; no hardware.
