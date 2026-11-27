# Advisory planner: design note (WP5-03)

Status: **proposal, not implemented.** Nothing in this note exists in `crates/`, `include/` or `backends/`. Every signature below is a sketch to be argued about, not an API. No code or header was changed to write it. Written 2026-11-28 from the tree at commit `107c5c9`.

Tags: **read from source** (a file and line in this tree), **inferred** (reasoning from what was read), **proposal**. Nothing here is observed on hardware and no timing is claimed.

Scope: gap WP5-03 in `docs/open-issues.md`: `Plan` is binding, while Weston and libliftoff always fall back to composition; there is no planner-only entry point and no advisory form. The design must stay `no_std`, bounded-memory and dependency-free (SoW §3.2; `composition-core` has no dependencies).

## 1. What a caller can do today without committing

All of it is **read from source**, `crates/composition-core/src/` unless noted.

| Entry point | What it does | Commits? | Limits for an advisory use |
| --- | --- | --- | --- |
| `planner::plan` (`planner.rs:302`) | Plans a `Frame` against `&Capabilities` and a `BufferLookup`, default `SearchLimits`, no backend check (`Verdict::Accept` always). Public, free function, no `Engine` or backend needed | No | Stateless and usable by a compositor. Rust only: the C ABI exposes nothing like it |
| `planner::plan_with` (`planner.rs:318`) | Same, with a caller closure `accept(&Plan) -> Verdict` for each legal assignment (`Accept`, `TryNext`, `GiveUp`) and caller `SearchLimits` | No (the closure may do a TEST_ONLY) | Same |
| `Engine::validate` (`engine.rs:210`) | `plan_frame` (`engine.rs:231`): the planner plus `DisplayBackend::check` (`backend.rs`, "on Linux an atomic `TEST_ONLY` commit") as the closure. Returns a `Plan`. Needs `&mut Engine`, registered buffers (`Engine::register_buffer`, `engine.rs:183`) and a backend | No ("Nothing on the display changes", `engine.rs:207`) | All-or-nothing, see below |
| `Plan::assignments` / `Plan::matches` (`planner.rs:265`, `:270`) | Read a plan; test it against a frame by fingerprint | No | |
| `Engine::capabilities` (`engine.rs:152`) | The `Capabilities` the engine plans against (outputs, planes, formats, scaling, zpos, resource groups) | No | Lets a caller see the hardware, not decide on it |
| `Engine::set_search_limits` (`engine.rs:157`) | Bounds the search: `steps` 20 000, `backend_checks` 32 by default (`planner.rs:201`) | No | |
| C ABI | `ce_present` (`crates/composition-ffi/src/lib.rs:764`; `composition_engine.h:326`) plans **and commits** in one call (`engine.0.present`). There is no `ce_validate`, no `ce_plan`, and `Engine::validate` is not exported (the function list in `composition_engine.h:295-342` is `ce_abi_check`, `ce_engine_init`, buffer calls, `ce_present`, `ce_poll_event`, `ce_set_latch_timeout`, `ce_advance_time`) | Yes | A C caller cannot ask "would this work?" without presenting it |

What is not possible today, all **read from source**:

1. **No partial answer.** `plan_into` (`planner.rs:333`) either returns every visible layer on a plane or an error. `PlanError::NoPlane { layer, reasons }` (`planner.rs:164`) names one layer, "the layer the search could not get past" (`search.order[search.deepest]`, `planner.rs:471-474`), and the reasons gathered at that depth only (`note`, `planner.rs:675`). The reasons of the other layers are not reported, and that layer is not necessarily the one a compositor should drop.
2. **No composition.** `planner.rs:5` says "There is no fallback composition". The planner has no notion of a layer that is drawn by the client or compositor into another buffer, and no way to say "this plane is taken by that composed result".
3. **Per-layer static reasons are private.** `static_reasons` (`planner.rs:586`) computes the exact per-plane, per-layer `RejectReasons` (`compat`, `planner.rs:661`) and then keeps them only inside `Search`. No function returns them.
4. **Per-layer failures are errors, not advice.** `UnknownBuffer`, `InvalidSource`, `InvalidDestination` and `CannotClip` (`planner.rs:391-416`) abort the whole plan. `CannotClip` is the one a compositor would want as "compose this layer": a rotated layer partly off the output (`clip_to_output`, `planner.rs:482-546`).
5. **`Engine::validate` needs buffers registered.** The `BufferLookup` is the engine's `Presentation` (`engine.rs:241`), filled by `register_buffer`. A compositor that creates and drops buffers every frame would have to register each one, with the table bounded by `MAX_BUFFERS = 64` (`limits.rs`). `plan`/`plan_with` take any `BufferLookup`, so this is a Rust-only escape hatch.
6. **The C error report is lossy.** `ce_error_info` (`composition_engine.h:245`) carries one layer index and one `reasons` word, which `describe` fills only for `CE_ERR_NO_PLANE` (`lib.rs:520-545`).
7. **Binding handles.** A `Plan` is only valid for the frame and capability generation it came from (`Plan::matches`, `Error::StalePlan`/`PlanMismatch`, `engine.rs:296-300`). That is the right rule for commit and is kept; the advisory result must not be accepted by `commit`.

## 2. What the advisory API must answer (proposal)

The question a compositor asks is not "may I present this?" but "which of my layers can go on planes, which must I draw into a composition buffer, and why?". For each input layer the answer is one of:

* **Plane**: assignment with plane id, `zpos`, clipped source and destination (the existing `Assignment`, `planner.rs:212`).
* **Compose**: not on a plane; with the reasons.
* **Hidden**: entirely off the output; no plane, no composition needed (the planner already drops these, `planner.rs:264`).

and, for the composed layers collectively, **where the composed result sits in the stack and which plane it needs**.

### 2.1 Model of composition (the central design choice)

The engine does not render. The advisory API therefore must not pretend to; it plans the frame the compositor *would present after composing*:

* The caller names the composition result by a `ComposeTarget { format, modifier }` it will render into (in Weston terms the renderer's output buffer; **inferred**, see §4).
* The composed layers are replaced by **one synthetic layer**: full output, that format, opaque or premultiplied-alpha as the caller states, at the stacking position of the composed run.
* One synthetic layer means one composition buffer. So the composed layers must be **contiguous in stacking order**: a promoted layer between two composed layers would need two composition buffers (**inferred**; a two-target variant is an open decision, §6).
* The search is therefore over a contiguous run `[lo, hi]` of the stacking-sorted layers: everything below `lo` and above `hi` is offered to planes, the run is replaced by the synthetic layer. With `n <= MAX_LAYERS = 16` there are at most `1 + n(n+1)/2 = 137` candidate runs (including the empty run, "everything on planes"). Candidates are tried in order of increasing run length, ties by lower position, so the result is deterministic and promotes as many layers as possible. It does **not** rank by cost or power; neither does the planner (`planner.rs:15-16`).
* Each candidate is one call of the existing search (`plan_with` semantics) on the reduced frame, with a shared step budget, so total work is bounded (§3.3).

This is deliberately simple. It is a proposal that gives a compositor a usable answer and a bounded runtime; it is weaker than a search over arbitrary subsets and says so.

## 3. Proposed Rust API

All **proposal**, a new module `crates/composition-core/src/advisory.rs`, `#![forbid(unsafe_code)]`, no dependencies, no allocation, only the existing `FixedVec`/`limits` constants.

### 3.1 Types

```rust
/// How the composed layers are rendered by the caller. Proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComposeTarget {
    pub format: PixelFormat,
    pub modifier: Modifier,
    /// Blend of the composed result against what is below it.
    pub blend: BlendMode,
}

/// Where one input layer ends up. Proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// On a plane; the assignment is as in `Plan`.
    Plane(Assignment),
    /// Drawn by the caller into the composition buffer.
    Compose,
    /// Entirely off the output: neither on a plane nor composed.
    Hidden,
}

/// Why a layer is not on a plane. Proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocker {
    /// The layer is on a plane (or hidden).
    None,
    /// No plane can take this layer on its own: `reasons` is what holds on
    /// every plane (the intersection of the per-plane static reasons).
    Static,
    /// Some plane can take it alone but not together with the others
    /// (`IN_USE`, `Z_ORDER`, `RESOURCE`), or only below/above a composed run
    /// that was needed for another layer.
    Combination,
    /// The backend check (`TEST_ONLY`) refused every assignment that held it.
    Backend,
    /// The layer cannot be clipped exactly (`PlanError::CannotClip`).
    Clip,
    /// The step budget ended before this layer was decided.
    Budget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerAdvice {
    pub placement: Placement,
    pub blocker: Blocker,
    /// `RejectReasons` common to all planes; empty unless `Blocker::Static`
    /// or `Backend`-narrowed.
    pub reasons: RejectReasons,
}

/// The advisory result. Proposal.
#[derive(Clone, Copy, Debug)]
pub struct Advice {
    output: OutputId,
    /// One entry per input layer, in input order.
    layers: FixedVec<LayerAdvice, MAX_LAYERS>,
    /// Assignment of the synthetic composition layer, if any layer is
    /// composed. Its `layer` field is `usize::MAX`.
    compose: Option<Assignment>,
    /// Stacking positions the composed run covers, `[low, high]` over the
    /// stacking-sorted layers.
    compose_run: Option<(u8, u8)>,
    steps: u32,
    backend_checks: u32,
}
impl Advice {
    pub fn layers(&self) -> &[LayerAdvice];
    pub fn compose(&self) -> Option<&Assignment>;
    /// Number of layers placed on planes (the composition target excluded).
    pub fn promoted(&self) -> usize;
    /// True if every layer is on a plane and nothing needs composing.
    pub fn is_direct(&self) -> bool;
}

/// Errors of the whole call (caller or hardware, not a per-layer outcome).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdviceError {
    UnknownOutput(OutputId),
    TooManyLayers { count: usize },
    UnknownBuffer { layer: usize, buffer: BufferId },
    InvalidSource { layer: usize },
    InvalidDestination { layer: usize },
    /// Even a frame of the composition layer alone has no plane (no plane
    /// takes the target format on this output): a configuration problem.
    NoComposePlane { reasons: RejectReasons },
    /// The step or backend-check budget ended before any candidate, the
    /// full-composition one included, was decided.
    LimitExceeded,
    /// The backend check gave up (`Verdict::GiveUp`).
    Aborted,
}

/// Budgets. Proposal: `SearchLimits` plus a cap on candidate runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdviseOptions {
    pub limits: SearchLimits,
    pub max_candidates: u32,
    /// `None` forbids composition: the call then only answers "all on
    /// planes?" and reports per-layer blockers.
    pub compose: Option<ComposeTarget>,
}
```

Per-layer problems that today abort (`CannotClip`) become `Placement::Compose` with `Blocker::Clip`. `UnknownBuffer`, `InvalidSource`, `InvalidDestination` stay errors: they are caller bugs, not hardware limits (open decision, §6).

### 3.2 Entry points

```rust
/// Planner-only: no engine, no backend, no buffer table. Proposal.
pub fn advise<L: BufferLookup>(
    frame: &Frame<'_>,
    caps: &Capabilities,
    buffers: &L,
    options: &AdviseOptions,
    accept: impl FnMut(&Plan) -> Verdict,
) -> Result<Advice, AdviceError>;

/// Per-layer, per-plane static reasons only; no search. Proposal.
/// `out[plane_index]` is what stops `layer` on that plane taken alone
/// (`planner::static_reasons`, today private).
pub fn explain_layer(
    layer: &Layer,
    info: &BufferInfo,
    caps: &Capabilities,
    output: OutputId,
    out: &mut [RejectReasons; MAX_PLANES],
) -> Result<(), AdviceError>;

impl<B: DisplayBackend, T: Trace> Engine<B, T> {
    /// `advise` with the engine's capabilities, registered buffers, search
    /// limits and `backend.check` as `accept`. Changes nothing. Proposal.
    pub fn advise(&mut self, frame: &Frame<'_>, compose: Option<ComposeTarget>)
        -> Result<Advice, Error>;
}
```

* `advise` is the stateless form: a compositor with its own capability snapshot and its own buffer metadata calls it with a `BufferLookup` over its own table, which is what `plan` already allows (`planner.rs:27`). No backend is needed; `accept` defaults to `|_| Verdict::Accept` for a pure planner answer or runs a TEST_ONLY.
* `explain_layer` needs `static_reasons` made `pub(crate)`-reachable (an edit to `planner.rs`, not done here).
* The advice has **no** handle to `Engine::commit`: `Advice` is not a `Plan`. The engine entry point returns reasons and assignments only. To present, the caller renders the composition buffer, builds the real frame (composed layers replaced by the target layer) and calls `Engine::present`, which plans again and may differ if the hardware state moved. That is intended; it keeps `Plan`'s binding meaning (§1 item 7).
* Existing paths do not change: `plan`, `plan_with`, `validate`, `commit`, `present` keep their behaviour.

### 3.3 Bounded memory and time (inferred)

* Memory: `Advice` is `MAX_LAYERS` x (an `Assignment` of about 40 bytes plus a few words) = on the order of 1 KiB on a 32-bit target; `explain_layer` out-array is `MAX_PLANES x 4` = 64 bytes; the search reuses `plan_into`'s fixed tables. The figures are estimates to be replaced by a `size_of` assertion test (§5); the ABI check pattern `CE_ENGINE_SIZE` (`lib.rs:85`) shows how.
* Time: at most `1 + n(n+1)/2` candidate runs, each bounded by the shared `SearchLimits.steps`/`backend_checks`; `max_candidates` stops earlier. When the budget ends, the best candidate found so far is returned with `Blocker::Budget` on undecided layers, or `AdviceError::LimitExceeded` if nothing was found. No measurement of the cost exists; none is claimed.
* No allocation, no recursion beyond `Search::descend`, which is already bounded by `MAX_LAYERS`.

## 4. Proposed C ABI

All **proposal**; declarations as they would appear in `crates/composition-ffi/include/composition_engine.h` with matching `#[repr(C)]` structs and `#[no_mangle]` functions in `crates/composition-ffi/src/lib.rs`, which must change in the same commit (the rule that header and `lib.rs` move together). Existing structs and functions are unchanged. `CE_ENGINE_SIZE` does not change for the stateless form. New structs would be added to `ce_abi_check` (a new `ce_abi_check2`, or extra arguments with a version bump; open decision, §6).

```c
/* CE_PLACE_* */
enum {
	CE_PLACE_PLANE = 0,   /* on a plane: see assignment */
	CE_PLACE_COMPOSE = 1, /* drawn by the caller into the composition buffer */
	CE_PLACE_HIDDEN = 2,  /* entirely off the output */
};

/* CE_BLOCKER_* */
enum {
	CE_BLOCKER_NONE = 0,
	CE_BLOCKER_STATIC = 1,      /* no plane can take it alone: see reasons */
	CE_BLOCKER_COMBINATION = 2, /* fits a plane alone, not with the others */
	CE_BLOCKER_BACKEND = 3,     /* the backend check refused it */
	CE_BLOCKER_CLIP = 4,        /* cannot be clipped exactly */
	CE_BLOCKER_BUDGET = 5,      /* search budget ended first */
};

typedef struct ce_compose_target {
	uint64_t modifier;
	uint32_t format;
	uint8_t blend;      /* CE_BLEND_* */
	uint8_t enabled;    /* 0: composition is not allowed */
} ce_compose_target;

typedef struct ce_advise_options {
	uint32_t max_steps;      /* 0: library default (SearchLimits) */
	uint32_t max_backend_checks; /* 0: library default */
	uint32_t max_candidates; /* 0: library default */
	ce_compose_target compose;
} ce_advise_options;

typedef struct ce_layer_advice {
	ce_assignment assignment; /* valid if placement == CE_PLACE_PLANE */
	uint32_t reasons;         /* CE_REJECT_*, see CE_BLOCKER_STATIC */
	uint8_t placement;        /* CE_PLACE_* */
	uint8_t blocker;          /* CE_BLOCKER_* */
} ce_layer_advice;

typedef struct ce_advice {
	ce_layer_advice layers[CE_MAX_LAYERS]; /* [0, layer_count) of the frame */
	ce_assignment compose;   /* valid if has_compose */
	uint32_t layer_count;
	uint32_t compose_low;    /* stacking range composed, if has_compose */
	uint32_t compose_high;
	uint8_t has_compose;
} ce_advice;

/* Buffer metadata for the stateless form; the caller's own table. */
typedef struct ce_buffer_entry {
	uint32_t buffer;
	ce_buffer_info info;
} ce_buffer_entry;

/*
 * Stateless planner-only advice: no engine, no backend, no registration.
 * Plans `frame` against `caps` with the metadata in `buffers`; if `check` is
 * not NULL it is called like ce_backend_ops.check for each legal assignment
 * (e.g. a DRM TEST_ONLY commit). Writes *advice; on error returns a
 * negative status and fills *info. Nothing is committed and nothing is
 * remembered between calls.
 */
ce_status ce_advise(const ce_capabilities *caps,
		    const ce_buffer_entry *buffers, size_t buffer_count,
		    const ce_frame *frame, const ce_advise_options *options,
		    int32_t (*check)(void *ctx, const ce_frame *frame,
				     const ce_assignment *assignments, size_t count),
		    void *ctx, ce_advice *advice, ce_error_info *info);

/*
 * The same through an engine: its capabilities, registered buffers and
 * backend `check`; never `commit`. Changes nothing.
 */
ce_status ce_engine_advise(ce_engine *engine, const ce_frame *frame,
			   const ce_advise_options *options, ce_advice *advice,
			   ce_error_info *info);

/*
 * Per-plane static reasons for one layer; reasons[i] belongs to
 * caps->planes[i]. No search.
 */
ce_status ce_explain_layer(const ce_capabilities *caps, uint32_t output,
			   const ce_layer *layer, const ce_buffer_info *info,
			   uint32_t reasons[CE_MAX_PLANES], ce_error_info *info_out);
```

Notes (**proposal**):

* `ce_advise` takes `ce_capabilities` from the caller. A compositor that has no engine builds it from its own KMS properties (the same table a backend's `ops.capabilities` fills, `lib.rs:221`). A new status `CE_ERR_NO_COMPOSE_PLANE` is needed; the rest map onto existing `CE_ERR_*` through `describe` (`lib.rs:520`).
* The `check` callback matches the signature of `ce_backend_ops.check` (`lib.rs:222-224`); the `ctx` pointer is the caller's. Unsafe code stays confined to `composition-ffi`, as today; `composition-core` remains `forbid(unsafe_code)`.
* The stateless form is the one Weston-like callers need: no storage block (`CE_ENGINE_SIZE` = 28 672), no buffer lifecycle (`register_buffer`, `finish_write`) and no event queue.

## 5. How a compositor such as Weston would use it (inferred)

Weston's DRM backend plans plane use per output in `drm_output_propose_state` with modes `PLANES_ONLY` and `MIXED` and always has a renderer fallback; libliftoff does likewise (`docs/wp5-gap-analysis.md` section 2, source read at `we@c8d00e2`). That file also records that `state-propose.c` was **not** read line by line and the Weston release the boards run was not read, so the mapping below is a sketch of intent, not a claim that Weston can host it (W5-02 and the planned Weston reads cover that).

1. At output (re)configuration the compositor builds `ce_capabilities` from its KMS plane properties, or asks the engine (`Engine::capabilities`).
2. Per repaint it describes its views as `ce_layer`s in stacking order, with the buffer metadata of each client buffer in a `ce_buffer_entry` table of its own.
3. It calls `ce_advise` (or the Rust `advise`) with `check` pointing at its existing TEST_ONLY commit, and `compose` set to the format of its renderer's output buffer.
4. `ce_advice.layers[i].placement` says per view: scan out on plane P with `zpos`, or paint in the renderer. `compose_low..compose_high` says which views the renderer paints into one buffer, and `compose` says which plane shows that buffer (usually the primary).
5. `blocker`/`reasons` replace Weston's per-view trial and error with an explanation; they can feed a debug log ("view 3: FORMAT, SCALING on every plane") or a compositor's decision to try a different client buffer format.
6. The compositor then renders, builds its own atomic state, and commits through its own path. The engine never commits in this flow. If the compositor wants the engine's lifecycle and latch events, it switches to `Engine::present` with the composed frame, which plans afresh.

Inferred limits: Weston can also underlay or reorder planes, use several renderer passes, and applies policy (power, damage, overlay surfaces it will not promote). None of that is modelled; the advice is a legal assignment, not Weston's preferred one.

### Tests the design needs (**proposal**; none exist)

Host tests, no board. The fake backend (`crates/composition-fake`) and `vkms` are the existing fixtures.

1. **Equivalence with the planner.** For random frames on fake topologies: if `plan` succeeds, `advise` returns `is_direct()` with identical assignments; if `plan` fails with `NoPlane`, `advise` does not return `is_direct()`.
2. **Composition is legal.** For every advice, build the reduced frame (promoted layers plus the synthetic layer) and check `plan` accepts it with the returned assignments; check every composed layer has a non-`None` blocker unless the budget ended.
3. **Contiguity.** No `Plane` placement lies between two `Compose` placements in stacking order.
4. **Minimality (within the model).** No shorter contiguous run is legal (brute force over runs, small `n`), for fixtures from `docs/platform-capabilities.md` and the AM62P/i.MX95 shapes.
5. **Reasons.** `Blocker::Static` reasons equal the intersection of per-plane `static_reasons` computed independently; `explain_layer` agrees with `plan` (a layer whose row is all-empty for some plane is not `Static`).
6. **Hidden and clipped layers.** Off-output layers get `Hidden` and no plane; a rotated layer partly off-output gets `Compose` with `Blocker::Clip` where `plan` returns `CannotClip`.
7. **Backend refusal.** A fake `check` that refuses a plane set yields `Blocker::Backend` or a different run; `GiveUp` yields `AdviceError::Aborted`; the closure is called no more than `backend_checks` times.
8. **Bounds.** Step and candidate limits are honoured; a tiny budget returns `LimitExceeded` or `Blocker::Budget`, never a hang; the loop terminates for `n = MAX_LAYERS` and `MAX_PLANES` planes.
9. **No side effects.** `Engine::advise` leaves the buffer states, pending submissions, events and `generation` unchanged and never calls `backend.commit` (counting fake backend).
10. **ABI.** C ABI tests in the style of `crates/composition-ffi/tests`: struct sizes via `ce_abi_check`, null and short arguments, `layer_count > CE_MAX_LAYERS`, a corrupt `placement` never written, every status mapped; a C file compiled against the header in the same commit.
11. **Fuzz.** Extend the seeded malformed-frame fuzz of `crates/composition-fake` to `advise` with the invariants of 2 and 3.
12. **no_std build.** `composition-core` with the new module still builds for `thumbv7em-none-eabihf` (the CI job in `docs/ci.md`), and a `size_of::<Advice>()` assertion pins the footprint.
13. **Cross-check (later).** The recorded-topology comparison with libliftoff (WP5-04) would use `advise` as its input; that note is separate.

Not testable here: that a real compositor accepts the advice, and any timing or power effect. Nothing is measured; none of this is a hardware result.

## 6. Open decisions

1. **One composition buffer or several.** The contiguity model (§2.1) is simple and bounded but cannot express "promoted layer between two composed ones". Weston and libliftoff may do better. Alternatives: allow `k` composition targets; or a subset search with a step cap; or a user-supplied hint of which layers must be composed.
2. **Who owns composition.** Whether the engine should only advise, or also offer an optional software compositor (SoW non-goal territory; SoW sections to be re-read before any answer). This note assumes advise-only.
3. **Greedy by count or by weight.** Minimal composed-layer count is the proposed objective. Compositors usually want to promote by size, update rate or video; an optional per-layer priority or "must be on a plane" / "never promote" flag in `Layer` would change `Layer` and `ce_layer` (an ABI change to existing structs, which this proposal otherwise avoids).
4. **Per-layer input errors**: keep `UnknownBuffer`/`InvalidSource`/`InvalidDestination` as whole-call errors (proposed) or report them as `Compose` with a blocker.
5. **New `RejectReasons` bits.** `Blocker::Clip` and `Combination` are a separate enum here so as not to touch the existing 11 bits (`planner.rs:38-60`, mirrored by `CE_REJECT_*`). One could instead add bits, simpler for C but a change to a published word.
6. **Stateless C form only, or also the engine form.** The engine form (`ce_engine_advise`) reuses registration and backend check but needs the 28 KiB engine (`CE_ENGINE_SIZE`) and buffer lifecycle a compositor may not want. Proposed: ship both, stateless first.
7. **Versioning the ABI.** New structs need `ce_abi_check` coverage: a second check function, extra parameters (breaks callers) or a version constant. Not decided.
8. **Advice stability.** Whether the same frame and capabilities must always give the same advice (the planner is deterministic, `planner.rs:15`); proposed yes, which constrains future search changes.
9. **Relation to `Plan`.** Whether an `Advice` with `is_direct()` may be convertible into a `Plan` for `commit` (proposed no, §3.2) or whether `Engine::validate` should simply gain the partial result.
10. **Buffer state checks.** `Engine::validate` does not require buffers to be `Ready`; whether `advise` should is open (a compositor typically advises before its clients have finished drawing).
11. **Capability source for a non-engine caller.** `ce_capabilities` is about 9 KiB (a comment in `lib.rs:254-258`, **read from source**); a caller that only wants advice may not want to build it. A smaller input form (a sequence of planes) would be a separate API.
12. **Name and place.** `advise` vs `plan_advisory`; a module in `composition-core` (proposed) vs a separate crate. A separate crate would keep `composition-core` unchanged but would need its own `no_std` CI job.

## 7. What this note does not claim

* Nothing here is implemented, tested or measured.
* No claim about what Weston or libliftoff do beyond what `docs/wp5-gap-analysis.md` records (Weston `main` at `c8d00e2`, libliftoff `c4226a7`, partly read), and none about the Weston on a board image.
* No upstream acceptance, and no claim that an advisory API reaches parity with either planner; that needs WP5-04 and hardware.
* No hardware: not tied to M1, M2 or M3.
