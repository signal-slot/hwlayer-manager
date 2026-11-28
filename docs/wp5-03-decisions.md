# WP5-03: owner decision brief for the advisory planner

Date: 2026-11-28. Status: **brief only; nothing is decided here.** Every item ends with `Decision: open (owner)`. Options are listed without ranking.

Sources (**read from source**, repository at `df507ed`, 2026-11-28): [`advisory-planner-design.md`](advisory-planner-design.md) §6 (items 1 to 13 and the closing "further choices" paragraph) and §7; [`open-issues.md`](open-issues.md) WP5-03, WP5-04, WP5-06, WP5-09; `SoW.md` §8 WP5 ("upstreaming proposal and gap analysis; no guarantee of upstream acceptance"); code in [`crates/composition-core/src/advise.rs`](../crates/composition-core/src/advise.rs), [`crates/composition-ffi/src/lib.rs`](../crates/composition-ffi/src/lib.rs) and [`crates/composition-ffi/include/composition_engine.h`](../crates/composition-ffi/include/composition_engine.h).

## 0. Evidence that applies to every item

* **Read from source (2026-11-28):** the Rust form and the stateless C form are implemented, with host tests: `crates/composition-core/tests/advise.rs` (equivalence with `plan`, legality and contiguity of the reduced frame, minimality, static reasons, hidden and clipped layers, backend refusal, budgets, determinism, footprint pin), `crates/composition-fake/tests/advise_fuzz.rs` and `advise_scenes.rs`, FFI tests under `crates/composition-ffi/tests/` and `crates/composition-ffi-staticlib/tests/`, and the C test `tests/c/advise.c`. Test names were read, and this brief did not re-run them.
* **Not measured:** any timing, power or memory figure on a board; any acceptance of the advice by a real compositor; any comparison with libliftoff or Weston (WP5-04 and WP5-06 are open).
* **Read from source:** the "taken" statements are the current behaviour, not owner decisions (`open-issues.md` WP5-03: "the owner has not settled any").
* **Upstreaming, general:** SoW §8 WP5 promises no acceptance, and nothing below promises it. Weston and libliftoff are C projects. The Rust `advise` could be offered only as a C-callable library, an algorithm description or a patch written in C. That is a separate choice from every item here.

Code areas named in the "changes" lines: `advise` and `explain_layer` (Rust, `composition_core::advise`), `ce_advise` and `ce_explain_layer` (C), `include/composition_engine.h`, and `DriverProfile` (per-driver capability tables). `advise` does not read `DriverProfile` itself; it takes the caller's `&Capabilities`.

## 1. One composition buffer or several (design §6.1)

* **Question:** keep a single contiguous run of composed layers, or allow more.
* **Options:** (a) keep one contiguous buffer (current); (b) `k` composition targets; (c) subset search with a step cap, so a promoted layer may lie between two composed ones; (d) a caller hint naming layers that must be composed.
* **Code changes:** (a) none. (b) the compose input and `AdviseOptions` become lists, `Advice` gains per-target run data, and `ce_compose_target`, `ce_advise_options` and `ce_advice` change, so `ce_abi_check_advise` changes (item 7). `DriverProfile` is unchanged. (c) the run search in `advise.rs` is replaced, the contiguity and minimality tests change, and a new step limit is needed. (d) a new field in `Layer`/`ce_layer` or in the options.
* **Upstreaming:** (a) is the simplest to describe. Weston and libliftoff have a renderer fallback in the paths read in `wp5-gap-analysis.md` (partly read). Weston may use several renderer passes (inferred, design §5; not read line by line).
* **Evidence:** read from source: the minimality and termination tests assume (a). Not measured: how often real scenes need (b) or (c); no recorded board scene is evaluated.
* Decision: open (owner)

## 2. Who owns composition (design §6.2)

* **Question:** advise only, or also an optional software compositor.
* **Options:** (a) advise only (current); (b) add a software compositor.
* **Code changes:** (a) none. (b) a new module with pixel handling in `composition-core` (today `no_std` and `forbid(unsafe_code)`), buffer-ownership rules in the engine, and new C entry points.
* **Upstreaming:** (b) duplicates the compositor's renderer and is SoW non-goal territory per the design note. (a) matches the advisory framing.
* **Evidence:** read from source: the design note. Not measured: any cost or benefit of (b).
* Decision: open (owner)

## 3. Greedy by count or by weight (design §6.3)

* **Question:** how to choose which layers are composed.
* **Options:** (a) minimal composed-layer count, ties to the lowest run (current); (b) per-layer priority or weight; (c) a cost model (area, bandwidth).
* **Code changes:** (a) none. (b) `Layer` and `ce_layer` gain a field, so the header and layout checks change. (c) new inputs and a scoring function in `advise.rs`.
* **Upstreaming:** Weston applies its own policy (power, damage, overlay promotion; inferred, design §5), and (b) would let a compositor express it. Not read at line level.
* **Evidence:** read from source: tests for determinism and minimality cover (a) only. Not measured: ranking quality, bandwidth or power effect.
* Decision: open (owner)

## 4. Per-layer input errors (design §6.4)

* **Question:** which bad inputs fail the whole call.
* **Options:** (a) current: `UnknownBuffer`, `InvalidSource`, `InvalidDestination`, `UnknownOutput` and `TooManyLayers` are whole-call errors, and an unclippable layer becomes `Compose` with `Blocker::Clip`; (b) mark per layer and continue; (c) fail on clip too.
* **Code changes:** (a) none. (b) `AdviceError` shrinks, `Blocker` grows, and the `ce_status` mapping in `describe` and `ce_advise` changes. (c) `Blocker::Clip` is removed and an error added.
* **Upstreaming:** a compositor usually wants a result per view, which (b) gives, but it weakens "invalid input is a caller bug".
* **Evidence:** read from source: tests `input_errors_are_whole_call_errors`, `hidden_and_clipped_layers`, `clipping_agrees_with_the_planner` (5000 cases per the design note). Not measured: behaviour with real callers.
* Decision: open (owner)

## 5. New `RejectReasons` bits (design §6.5)

* **Question:** extend the reject-reason bitset or keep a separate enum.
* **Options:** (a) separate `Blocker` enum (current); (b) new `RejectReasons` and `CE_REJECT_*` bits.
* **Code changes:** (a) none. (b) `RejectReasons` in `composition-core`, the `CE_REJECT_*` constants in the header, every backend mapping that translates reasons, and tests that enumerate the bits.
* **Upstreaming:** (a) leaves `plan`/`commit` unchanged, so an offer is smaller. (b) touches the API existing backends use.
* **Evidence:** read from source: the design note says only `static_reasons` became `pub(crate)` in `planner.rs`. Not measured: no consumer feedback.
* Decision: open (owner)

## 6. Stateless C form only, or also the engine form (design §6.6)

* **Question:** add `Engine::advise` and `ce_engine_advise`.
* **Options:** (a) stateless only (current); (b) add the engine form; (c) engine form only, which would remove the merged `ce_advise`.
* **Code changes:** (a) none. (b) a new method in the engine crate, a new C function and tests. Design §5 test 9 (no side effects: buffer states, events and `generation` unchanged, `backend.commit` never called) must be written. (c) removes `ce_advise`, `ce_explain_layer` and their tests.
* **Upstreaming:** a compositor without an engine needs the stateless form (design §4). The engine form serves only callers that already use `Engine::present`. A separate design note for the stateful form is another item of this plan.
* **Evidence:** read from source: tests of the stateless form exist. Not measured, and not existing: test 9 of design §5, and any engine form. No board has called either form.
* Decision: open (owner)

## 7. Versioning the ABI (design §6.7)

* **Question:** how the advise structs are version-checked.
* **Options:** (a) second check function `ce_abi_check_advise` (current); (b) extra arguments to `ce_abi_check` with a version bump; (c) a version field in each struct.
* **Code changes:** (a) none. (b) the `ce_abi_check` signature, the `CE_ABI_CHECK()` macro, `CE_ENGINE_SIZE` handling, and all C callers and tests. (c) new fields in every advise struct and the header layout checks.
* **Upstreaming:** (a) keeps existing integrations source-compatible; (b) breaks callers of `ce_abi_check`. For a C project the header is the only visible surface.
* **Evidence:** read from source: `abi_check_advise_accepts_the_mirror_sizes` in `crates/composition-ffi/tests/advise.rs`, and the C test against the header. Not measured: no external consumer of the header appears in the repository evidence.
* Decision: open (owner)

## 8. Advice stability (design §6.8)

* **Question:** same input gives same advice, or may vary.
* **Options:** (a) deterministic (current); (b) allow variation, for example hysteresis across frames.
* **Code changes:** (a) none. (b) needs state between calls, so it interacts with item 6, and the determinism test (also under renamed buffer ids) is dropped or replaced.
* **Upstreaming:** a stateless deterministic function is easier to test in a compositor's CI (inferred); (b) depends on item 6 and the compositor's frame policy.
* **Evidence:** read from source: `advice_is_deterministic`. Not measured: plane churn over real frame sequences.
* Decision: open (owner)

## 9. Relation to `Plan` (design §6.9)

* **Question:** may an `Advice` become a `Plan` or be accepted by `commit`.
* **Options:** (a) no conversion (current); (b) a conversion helper; (c) `commit` accepts an `Advice`.
* **Code changes:** (a) none. (b) a function in `advise.rs` and a rule for the composed run, since the synthetic layer needs a buffer the caller rendered. (c) `Engine::commit` and `Engine::validate` change, which touches the committed path, and the header gains a function.
* **Upstreaming:** a compositor commits through its own path (design §5 step 6), so (b) and (c) matter only to engine users.
* **Evidence:** read from source: the reduced-frame tests check that `plan` accepts the reduced frame with the returned assignments. Not measured: real commits.
* Decision: open (owner)

## 10. Buffer state checks (design §6.10)

* **Question:** must buffers be `Ready`.
* **Options:** (a) not required; only `BufferLookup` metadata is read (current); (b) require `Ready`.
* **Code changes:** (a) none. (b) a state check and a new error in `advise`. The stateless C form has no buffer states, so (b) applies only together with item 6 (b).
* **Upstreaming:** a compositor has no engine buffer lifecycle, and (a) fits it.
* **Evidence:** read from source: the design note and tests that build `BufferLookup` tables without engine state. Not measured: nothing.
* Decision: open (owner)

## 11. Capability source for a non-engine caller (design §6.11)

* **Question:** what a caller without an engine passes as capabilities.
* **Options:** (a) the caller's `&Capabilities` / `ce_capabilities` (current); (b) a smaller input form, for example plane lists with formats and zpos ranges; (c) a loader from KMS property dumps.
* **Code changes:** (a) none. (b) a new type, a conversion and a separate API; `DriverProfile` data is consumed only if a conversion is added. (c) a KMS-property parser, outside the `no_std` core or behind a feature.
* **Upstreaming:** the most visible item for a compositor author, since Weston and libliftoff keep KMS property data in their own structures (`wp5-gap-analysis.md`, partly read). The integration effort saved is not measured.
* **Evidence:** read from source: `ce_capabilities` mirrors the backend `ops.capabilities` table (design §4). `DriverProfile::Imx95Dpu` values are from source until `profile-check` (`open-issues.md` WP5-09). Not measured: any compositor integration attempt.
* Decision: open (owner)

## 12. Name and place (design §6.12)

* **Question:** where the function lives.
* **Options:** (a) `composition_core::advise::advise` (current); (b) `advisory.rs` as in the first sketch; (c) a separate crate.
* **Code changes:** (a) none. (b) a rename. (c) a new crate, workspace and CI changes (the two `thumbv7em` builds, `docs/ci.md`) and a second public API surface.
* **Upstreaming:** a separate crate would package an advisory-only offer more easily, at the cost of exposing or duplicating shared types. Not evaluated.
* **Evidence:** read from source: `advise.rs` holds a copy of the planner's private `clip_to_output`, checked by a test. Not measured: nothing.
* Decision: open (owner)

## 13. Frame refused entirely, no composition target (design §6.13)

* **Question:** result when `compose: None` and the frame does not fit.
* **Options:** (a) `Ok` advice with every shown layer `Placement::Compose` and `compose()` `None` (current); (b) an error such as `NoComposePlane`; (c) a new `Blocker`.
* **Code changes:** (a) none. (b) an `AdviceError` variant and a `ce_status` mapping. `CE_ERR_NO_COMPOSE_PLANE` (-19) already exists for "composition enabled and not even the target fits", so (b) reuses it or adds a code, and the header comment changes. (c) `Blocker` and its C constants change.
* **Upstreaming:** in (a) a caller must read `compose().is_none()` together with `Compose`. An explicit error is harder to misread. Not evaluated against a compositor.
* **Evidence:** read from source: `refused_frame_without_target_is_ok_and_all_composed`. Not measured: nothing.
* Decision: open (owner)

## 14. Choices the proposal left open (design §6, last paragraph)

* **Question:** keep these implementation choices: `accept` is `FnMut(&Plan, Option<usize>) -> Verdict`; steps bounded per candidate, not shared; a candidate cut by its step limit does not end the search; `Blocker::Static` is the intersection over planes reaching the output; the duplicated `clip_to_output`.
* **Options:** (a) keep; (b) change individually. The design note gives no further alternatives.
* **Code changes:** (a) none. (b) per choice: the callback type (`advise` signature, `ce_advise` `check` semantics, header), the budget model (`AdviseOptions`, `ce_advise_options`, `ce_abi_check_advise`), a shared `clip_to_output` (a change to `planner.rs`).
* **Upstreaming:** the callback and budgets define how a compositor's TEST_ONLY commit is called, and a reviewer would read them first (inferred). Not discussed with any upstream.
* **Evidence:** read from source: `backend_check_count_is_bounded`, `tiny_budgets_are_honoured`, `terminates_at_the_maxima`, the 5000-case clip test. Not measured: nothing on hardware.
* Decision: open (owner)

## 15. Related decisions outside design §6

Named in `open-issues.md`, listed so the owner sees them together, not expanded here.

* **WP5-05:** Apache-2.0 vs MIT for any offer, and the sign-off policy (Weston requires a human DCO sign-off and restricts AI-authored contributions). Other items of the plan cover the licence note and the contribution rules.
* **WP5-09:** whether `DriverProfile::Imx95Dpu` is project-local or offered. Evidence: values from source until `profile-check`; not measured on a board here.
* **WP5-04 and WP5-06:** the libliftoff cross-check and benchmarks. Not measured, and items 1, 3 and 11 depend on them for evidence.

Decision: open (owner)
