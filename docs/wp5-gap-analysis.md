# WP5 gap analysis: leased output, Weston-owned output, libliftoff

Desk analysis for SoW §4.2 and WP5. Written 2026-11-28. Nothing here is measured, and nothing was run on a board. Sections 1 to 7 are the 2026-11-28 analysis, kept as written except where marked; section 8 records which gaps later files closed.

## What was read

Both upstream trees were cloned shallow (`--depth 1`) on 2026-11-28 and read in the working tree. The commit is the shallow clone's `origin` head at that time.

| Project | Commit read | Licence file |
| --- | --- | --- |
| libliftoff (`https://gitlab.freedesktop.org/emersion/libliftoff.git`, branch `master`) | `c4226a79b7a52f59bb51789b1eda112658609125`, committed 2026-08-24, "Document why HOTSPOT_{X,Y} properties can't be discarded" | `LICENSE`: MIT, "Copyright (c) 2019 Simon Ser"; README says "MIT" |
| Weston (`https://gitlab.freedesktop.org/wayland/weston.git`, branch `main`) | `c8d00e2a1dbc98f4a8fb6473a4b485ffce409f3a` | `COPYING`: MIT-style, copyright of several authors and companies (only its opening lines were read) |

Files read: libliftoff `alloc.c` (whole), `include/libliftoff.h` (whole), `README.md`, `doc/compositor.md` (first part), `LICENSE`; Weston `libweston/backend-drm/state-propose.c` (searched, and lines 1680-1750 read), `drm-internal.h` (the `drm_output_propose_state_mode` enum), `kms.c` (`drm_pending_state_test`), `include/libweston/backend-drm.h` (`weston_drm_output_api`), `LLM-USAGE.md`, `AGENTS.md`, and a text search of the whole tree. `state-propose.c` was **not** read line by line, and neither were `drm.c`, `modes.c` or the client-side code. Tags with commit refs below carry the abbreviations `lo@c4226a7` (libliftoff) and `we@c8d00e2` (Weston).

Tags:

- **source (this repo)**: read in a file of this repository.
- **source (lo@c4226a7)** / **source (we@c8d00e2)**: read in that tree at that commit.
- **source (recorded)**: from `docs/platform-capabilities.md`, read there on 2026-11-19; not re-read.
- **recalled**: from prior knowledge, not checked against any source. May be wrong or outdated.
- **inferred**: a conclusion from the other rows.

## Correction of a premise

The brief for this plan says Weston can lease connectors to clients through the DRM lease Wayland protocol (`wp_drm_lease_device_v1`) and a `drm-lease` option. **At `we@c8d00e2` there is no such thing.** A case-insensitive whole-word search for `lease`, `leases`, `leasing`, `drm-lease` and `drm_lease` over the whole tree (excluding `.git` and `*.po`) returned nothing, and `libweston/backend-drm/` has no lease source file (source (we@c8d00e2): the file list is `colorops.c, drm-gbm.c, drm.c, fb.c, kms.c, modes.c, state-helpers.c, state-propose.c` and headers). What Weston does have is handling of `non-desktop` connectors (`modes.c: check_non_desktop`, `kms.c: WDRM_CONNECTOR_NON_DESKTOP`; source (we@c8d00e2)), which marks such a head as non-desktop in libweston's head model; that is not leasing.

This holds for upstream `main` only. A vendor or AGL Weston fork, a downstream patch, or the Weston that is on the board image may carry lease support; none of those was read (W5-02).

## 1. This project, as it stands

| Statement | Basis |
| --- | --- |
| The planner maps every layer of a frame to one hardware plane or rejects the frame. No fallback composition, no composition layer | source (this repo): `crates/composition-core/src/planner.rs` module doc |
| Search: bounded depth-first backtracking, layers bottom to top, deterministic, finds *a* legal assignment, no cost ranking. Defaults: 20 000 steps, 32 backend checks (`SearchLimits::default`) | source (this repo): `planner.rs` |
| Each legal assignment is offered to a backend check: `Accept`, `TryNext` or `GiveUp`. On Linux the check is an atomic `TEST_ONLY` commit (with `ALLOW_MODESET` when a modeset is needed) | source (this repo): `planner.rs` `Verdict`, `plan_with`; `crates/composition-drm/src/lib.rs` `check` |
| A refusal names the unmet constraints as a bit set: `OUTPUT`, `FORMAT`, `SCALING`, `SOURCE_SIZE`, `ALPHA`, `BLEND`, `TRANSFORM`, `FULL_OUTPUT`, `Z_ORDER`, `RESOURCE`, `IN_USE` | source (this repo): `planner.rs` `RejectReasons` |
| More layers than attachable planes is refused before any search | source (this repo): `plan_into` |
| Layers reaching beyond the output are clipped exactly or refused (`CannotClip`) | source (this repo): `planner.rs` `clip_to_output` |
| Shared hardware (scalers, fetch units) is a resource group with a capacity; layers consume it by trait (`SCALED_H/V`, `YUV`, `ANY`). The i.MX95 profile fills it from driver source only | source (this repo): `caps.rs` `ResourceGroup`; `profile.rs` `DriverProfile::Imx95Dpu` |
| The backend builds capabilities from what the fd exposes. A lease fd lists only leased objects, so capabilities cover exactly the lease | source (this repo): `composition-drm/src/lib.rs` module doc |
| The backend takes an fd (device node or lease) and does not care how it was obtained. Without `zpos` it assumes primary below overlays below cursor | source (this repo): `lib.rs` module doc |
| Lease revocation, dropped master and device removal (`EACCES`, `ENOENT`, `ENODEV`) become `BackendEvent::DeviceLost`; `EBUSY` becomes `Busy`; `EINVAL`, `ERANGE`, `ENOSPC` become `Rejected` | source (this repo): `lib.rs` `fail` |
| Commit is `NONBLOCK` + `PAGE_FLIP_EVENT`; the output keeps one pending submission | source (this repo): `lib.rs` `commit` |
| The engine tracks buffer state writable → submitted → latched → released and makes dropped frames observable | source (this repo): `presentation.rs`, `engine.rs`; SoW §3.1, §7 |
| `composition-core` is `no_std`, bounded-memory, with a C ABI | source (this repo): `crates/composition-core`, `crates/composition-ffi`; SoW §3.2 |
| `drm-lease-manager` (AGL): default lease per connector with one CRTC and the planes usable only with it; i.MX95 overlays shared between two CRTCs are left out unless listed in the TOML file; `ENOENT` after revocation | source (recorded): `docs/platform-capabilities.md`, AGL gerrit `src/drm-lease-manager` `master` at `68c48758c46d` |
| SoW: DRM leasing is exclusive at object level; a lease does not let the engine modify planes on a CRTC Weston still owns | source (this repo): SoW §2, §4.1, §4.2 |
| The repository is Apache-2.0. At `69706c8` it had no `NOTICE` and no contribution guidelines; those now exist (section 8). A sign-off policy is still absent | source (this repo): `LICENSE`; `NOTICE`; `CONTRIBUTING.md` (no sign-off or DCO text); repository contents at `69706c8` and HEAD |

## 2. What Weston offers

| Statement | Basis |
| --- | --- |
| Weston's DRM backend plans plane use per output in `drm_output_propose_state` with two modes: `DRM_OUTPUT_PROPOSE_STATE_PLANES_ONLY` ("no renderer use, only planes") and `DRM_OUTPUT_PROPOSE_STATE_MIXED` ("mix renderer & planes"), plus a `REUSE` bit ("reuse prior state with new buffers") | source (we@c8d00e2): `drm-internal.h` `enum drm_output_propose_state_mode`; `state-propose.c` |
| The repaint path tries, in order: reuse of the prior state, planes-only (only if `gbm` is present and client-buffer scanout is not disabled), then mixed. Mixed is the fallback, where the renderer draws what planes cannot take. After that `assert(state)` | source (we@c8d00e2): `state-propose.c` lines ~1690-1735 |
| So Weston, like libliftoff, always has a composition fallback and cannot refuse a frame | source (we@c8d00e2) for the fallback chain; inferred for "cannot refuse" |
| Plane assignment walks the paint nodes in z-order and tries each on a plane, counting zpos down from the plane's `zpos_max` and respecting `zpos_min`; it supports underlay planes in mixed mode. It checks for duplicate zpos at the end (`drm_output_check_zpos_plane_states`) | source (we@c8d00e2): `state-propose.c` (matches for `zpos`, `underlay`, `drm_output_check_zpos_plane_states`); the walk itself not read in full |
| Validation is an atomic `TEST_ONLY` commit: `drm_pending_state_test` calls `drm_pending_state_apply_atomic(..., DRM_STATE_TEST_ONLY)` | source (we@c8d00e2): `kms.c` ~line 2242 |
| In `drm_output_try_paint_node_on_plane` the test commit is skipped in planes-only mode (`mode != PLANES_ONLY && drm_pending_state_test(...) != 0`); the whole state is tested once at the end of `drm_output_propose_state` | source (we@c8d00e2): `state-propose.c` lines ~300 and ~1628 |
| Weston does not use libliftoff: no occurrence of `liftoff` anywhere in the tree | source (we@c8d00e2): text search |
| **No DRM lease support** (see "Correction of a premise") | source (we@c8d00e2): text search |
| The one plugin API of the DRM backend, `weston_drm_output_api_v1`, offers output settings (`set_mode`, `set_gbm_format`, `set_seat`, max bpc and similar). The part read has nothing to assign planes, supply a plane state or commit | source (we@c8d00e2): `include/libweston/backend-drm.h` first ~110 lines; the remaining members were not read |
| Weston's contribution policy: DCO sign-off by a human; AI agents must not create or edit issues or merge requests, nor post comments; AI prose is not forwarded upstream unless concise and reviewed by a human; AI-generated code is the human's responsibility | source (we@c8d00e2): `LLM-USAGE.md`, `AGENTS.md` |
| Weston's DRM backend owns the CRTCs, connectors and planes of its outputs and takes DRM master through its launcher | recalled (not read this session) |
| Client-side direct scanout in Weston is driven by `linux-dmabuf` feedback (scanout tranches); the client does not choose planes | recalled |
| `wp_drm_lease_device_v1` (the `drm-lease-v1` staging protocol of wayland-protocols) exists, and some other compositors implement it | recalled; says nothing about Weston |

## 3. Standalone leased-output mode against Weston-owned output mode

| Question | Standalone leased output (SoW §4.2 mode 1) | Weston-owned output (mode 2) | Basis |
| --- | --- | --- | --- |
| Who commits the CRTC | This engine alone, through the lease fd | Weston alone | source (this repo): SoW §4.2; source (we@c8d00e2) for Weston's commit path |
| Plane choice | This planner | `state-propose.c`, greedy z-order walk with renderer fallback | source (we@c8d00e2); source (this repo) |
| Can the frame be refused | Yes, by design (SoW §6) | No: mixed mode falls back to the renderer | source (this repo); source (we@c8d00e2) |
| Where the lease comes from | `drm-lease-manager` / `libdlmclient` (SoW §4.1), or the kernel ioctls from a leaser | Not Weston, at upstream `main`: it has no lease code | source (recorded); source (we@c8d00e2) |
| Weston changes needed | None for a separate output, provided the lease holds the primary and overlays | A patch or a plugin; see below | source (this repo): SoW §4.2; inferred |
| Producers | Send buffers to the engine process (a harness for M1) | Wayland clients; Weston turns surfaces into views | source (this repo): SoW §4.1; recalled for Weston |
| Buffer lifecycle | Engine state machine, with separate latch and release events | `wl_buffer.release` and presentation feedback | inferred; the Wayland side recalled |
| Failure of the owner | Engine or lease fd lost: output follows the lease manager's revocation behaviour | Weston crash takes the output | inferred |
| In M1 | Yes | No (SoW: "later investigation") | source (this repo): SoW §4.2, §9 |

### What the Weston-owned mode would need

The engine cannot commit Weston's CRTC (source (this repo): SoW §4.2). Three routes. None is built, none is required for M1.

1. **Weston as lease source.** Needs lease code Weston upstream does not have (source (we@c8d00e2)): a feature to write, not to configure. It would have to decide which planes a lease holds, which for an i.MX95 DPU with overlays shared between CRTCs means a policy like the one `drm-lease-manager` needs a TOML file for (source (recorded)). The engine itself would need nothing, since it accepts any fd. (inferred)
2. **A plane-placement hint into Weston's planner.** A protocol or internal hook that says "this surface needs its own plane, with these constraints" and reads the assignment back; Weston still commits. The read plugin API has no such hook (source (we@c8d00e2)), so this means a patch to `state-propose.c`. This project's `Plan` is binding by design (SoW §6 item 7), while Weston's mixed mode cannot refuse; the two semantics have to be reconciled (W5-04). (inferred)
3. **Weston calls this engine.** The planner replaces or is consulted by Weston's assignment. `plan_with` already takes a backend check as a closure, so it could run with Weston's own `TEST_ONLY` (source (this repo): `planner.rs`), but `Engine` bundles the commit path, so a planner-only entry point would be new API, in Rust and in the C ABI. Weston is C and the planner has a C ABI, so linking is feasible in principle. (inferred)

## 4. Overlap with libliftoff

| Statement | Basis |
| --- | --- |
| libliftoff is "Lightweight KMS plane library": users create layers, set KMS properties on them, and it picks hardware planes for the layers if possible. Depends on libdrm; requires universal planes and atomic | source (lo@c4226a7): `README.md` |
| API shape: `liftoff_device_create(drm_fd)`, `liftoff_output_create(device, crtc_id)`, `liftoff_plane_create` / `liftoff_device_register_all_planes`, `liftoff_layer_create`, `liftoff_layer_set_property(layer, name, value)` for any plane property except `CRTC_ID` (a zero `FB_ID` disables the layer). `liftoff_output_apply(output, req, flags, options)` appends the plane configuration to a caller-owned `drmModeAtomicReq`; "callers are expected to commit req afterwards" | source (lo@c4226a7): `include/libliftoff.h` |
| It owns the DRM fd (`liftoff_device_create` "takes ownership of the file descriptor") and expects `DRM_CLIENT_CAP_ATOMIC` to be on | source (lo@c4226a7): `libliftoff.h` |
| Algorithm: depth-first search over **planes**, not layers ("for each plane, try to find a layer"), because some drivers want the primary enabled first. The primary is allocated first, then the others from topmost to bottommost. At each node every candidate layer is applied to the plane, an atomic test commit is made, and on success the search recurses to the next plane; a failed test prunes the branch. A plane may also be skipped | source (lo@c4226a7): `alloc.c` header comment, `output_choose_layers` |
| Objective: maximise the number of layers on planes (`score`). Pruning: stop when `best_score >= best_possible_score` (remaining planes and remaining scoring layers). Time bound: a deadline, default 1 ms (`DEFAULT_ALLOC_TIMEOUT_NSEC`), settable through `options->timeout_ns`; no step limit | source (lo@c4226a7): `alloc.c` `calculate_best_possible_score`, `check_deadline` |
| zpos: handled "as a special case": relative zpos of two layers is only preserved when the layers intersect (`layer_intersects`), so non-overlapping layers can be placed on planes in any order | source (lo@c4226a7): `alloc.c` header comment, `check_layer_plane_compatible`, `has_allocated_layer_over`, `has_allocated_plane_under` |
| Composition layer: one per output, only valid on the primary plane. If a composition layer is set, an allocation is refused unless it either composes (some layers lack a plane) or puts every layer on a plane without using composition. A layer under a composited layer that it intersects cannot go to a non-primary plane | source (lo@c4226a7): `alloc.c` `check_alloc_valid`, `check_layer_plane_compatible`, `has_composited_layer_over` |
| Fewer test commits: reuses the previous allocation when nothing material changed (`reuse_previous_alloc`, `layer_needs_realloc`), checking only the framebuffer's size, format and modifier, the alpha extremes (0, 0xFFFF), and whether intersections changed; before each test commit it checks the layer's FB against the plane (`plane_check_layer_fb`); layer priority (`current_priority`, `layer_is_higher_priority`) orders the layers | source (lo@c4226a7): `alloc.c` |
| Before searching, it disables every plane it manages in the request "to make sure not to hit bandwidth limits because too many planes are enabled" | source (lo@c4226a7): `alloc.c` `liftoff_output_apply` |
| Failure of a test commit with `EINVAL`, `ERANGE` or `ENOSPC` means "does not work, try another"; any other error aborts | source (lo@c4226a7): `alloc.c` `output_choose_layers` |
| Candidate planes are recorded per layer so a caller can re-allocate a buffer with a format that the plane accepts (feedback loop, `liftoff_layer_is_candidate_plane`) | source (lo@c4226a7): `libliftoff.h`, `alloc.c` |
| The library mallocs per call (`malloc`, `calloc` in `liftoff_output_apply`), uses `clock_gettime` and linked lists, and logs through its own handler | source (lo@c4226a7): `alloc.c` |
| It has no notion of buffers beyond `FB_ID`, and nothing about latch, release or page-flip events: the caller commits and handles them | source (lo@c4226a7): `libliftoff.h` (no such API) |
| No reasons are returned for a layer that got no plane, only `liftoff_layer_needs_composition()` (bool) and the candidate-plane query; the debug log carries reasons as text | source (lo@c4226a7): `libliftoff.h`, `alloc.c` log calls |
| libliftoff follows Weston's contributing guidelines; used "in production by gamescope on Steam Deck", wlroots integration "underway" (README wording) | source (lo@c4226a7): `README.md` |
| Licence: MIT | source (lo@c4226a7): `LICENSE` |

### Where the two overlap, and where they do not

| Topic | libliftoff | This project | Reuse, contribute, or leave |
| --- | --- | --- | --- |
| Search | DFS over planes, maximise the number of placed layers, 1 ms deadline, test commit at every node | DFS over layers (bottom to top), first legal assignment, step and backend-check limits, test commit per complete assignment | Same job, different shape. This project issues `TEST_ONLY` only for assignments that passed the static checks, which may or may not mean fewer commits than libliftoff's per-node tests; unmeasured (inferred) |
| What "success" is | Best partial allocation; the remainder is composited by the caller | All layers on planes, or an error | Different contract (inferred). This is the deciding difference |
| zpos | Relative order preserved only between intersecting layers | `Z_ORDER` constraint over all layers placed so far (no intersection test in the planner's reject reasons) | Possible improvement for this planner: allowing non-overlapping layers to share or swap order would accept more frames. Not verified against `planner.rs` beyond its reject-reason names (inferred) |
| Reasons for refusal | Not returned (log text) | Typed bit set per refused layer | Contribution candidate as an idea (inferred) |
| Shared resources (scalers, fetch units) | Only the kernel's test commit knows | Resource groups plus a per-driver profile, and then the test commit | Not in libliftoff; a per-driver table is not generic KMS (inferred) |
| Allocation reuse between frames | Yes (`reuse_previous_alloc`) | Not seen in the planner read | Idea to take over if benchmarks show planning cost matters; SoW says do not optimise before correctness (source (this repo): SoW §6) |
| Clipping to the output | Not seen in `alloc.c` | Exact clipping or refusal | Unknown for the rest of the library |
| Composition layer | Central | Excluded | Not shared |
| Buffer and presentation state machine | None | Core of the engine | Not shared |
| Memory model | `malloc` per apply | Bounded, `no_std`, MCU-capable | Not shared; blocks reuse on MCU targets (source (this repo): SoW §3.2, §5) |
| API level | Arbitrary KMS properties by name | Typed layer fields and capability model, valid across KMS and non-KMS targets | libliftoff is more general for KMS; this project for targets without KMS (inferred) |

### Options, in order of cost

1. **Keep separate and document why** (SoW §4.1 asks to avoid "unnecessary duplicate algorithms"; the answer is the contract difference, `no_std`/bounded memory and the buffer lifecycle, all source-backed above). No upstream work.
2. **Cross-check in tests.** Run libliftoff and this planner on the same topology and layer sets, host-side only, and compare which frames each accepts with no composition layer; a difference is a lead for a planner bug or a missing constraint. Needs a dev-only dependency on libdrm and libliftoff, and a fake-KMS test harness for libliftoff's test commit (inferred). Best done with the i.MX95 topology once WP0 captured it.
3. **Adopt ideas**: intersection-aware zpos, allocation reuse (inferred).
4. **A libliftoff-compatible front** over the C ABI so a compositor that uses libliftoff can swap this engine in. Large, and blocked by the contract difference (inferred).
5. **Contribute ideas upstream**: typed refusal reasons, resource groups. Weston's policy (section 2) means any such contribution is human-authored. Only worth it if board measurements show libliftoff's heuristics failing on the DPU, which nothing shows yet (inferred).

## 5. Licensing

| Statement | Basis |
| --- | --- |
| This repository is Apache-2.0 | source (this repo): `LICENSE` |
| libliftoff is MIT (Simon Ser); Weston is MIT-style (`COPYING`) | source (lo@c4226a7), source (we@c8d00e2) |
| Weston requires a DCO sign-off from a human; libliftoff follows Weston's contributing guidelines, so the same is expected there | source (we@c8d00e2): `LLM-USAGE.md`; source (lo@c4226a7): `README.md` |
| MIT code can be included in an Apache-2.0 work with its notice kept; Apache-2.0 code cannot be copied into an MIT project without the copyright holders offering it under MIT | inferred; not legal advice |
| Apache-2.0's patent grant does not travel with code offered under MIT | inferred |
| An Apache-2.0 library linked into an MIT program has no conflict as such; the combined work carries both notices | inferred |
| `drm-lease-manager` and `libdlmclient` licences were not recorded | not read; W5-07 |

## 6. Open issues for WP5

Numbered for the tracker; none are filed (filing is outside this item).

| ID | Issue | Why it matters |
| --- | --- | --- |
| W5-01 | Finish the Weston read. Not read: `state-propose.c` in full (what it does with a layer that is YUV, scaled, or needs a resource group), `drm.c` (DRM master and launcher), how client buffers become paint nodes, remaining members of `weston_drm_output_api`. Pin tags of released Weston (shallow `main` was read, not a release) | The Weston rows are as good as the part read |
| W5-02 | Establish which Weston is on the board and whether it, or the AGL/Toradex Weston, carries lease support. Upstream `main` has none (source (we@c8d00e2)). Check `lease-feasibility` results when they exist, and AGL's Weston recipes (not read) | Decides whether "Weston as lease source" is a configuration or a patch. The brief assumed a lease protocol; upstream disproves it |
| W5-03 | Read the rest of `include/libweston/backend-drm.h` and `libweston.h` for any plane-assignment hook, and `linux-dmabuf` feedback | Decides route 2 of section 3 |
| W5-04 | Decide whether `Plan` stays binding or gets an advisory form, and whether a planner-only entry point (no committing backend) is exposed, in Rust and in the C ABI | Needed for routes 2 and 3 and the libliftoff-compatible front; Weston's planner cannot refuse |
| W5-05 | Cross-check the planner against libliftoff on shared capability dumps (dev-only), with the observed i.MX95 topology after WP0 | Cheapest evidence for or against duplication |
| W5-06 | Review the planner's `Z_ORDER` rule against libliftoff's intersection-aware zpos, and decide whether the stricter order refuses frames the hardware can show | A possible planner improvement; arises from section 4 |
| W5-07 | Licensing and contribution set: decide Apache-2.0 versus MIT for any upstream offer, add `NOTICE`, contribution guidelines and a sign-off policy that fits Weston's DCO and AI rules; record the licences of `drm-lease-manager` and `libdlmclient` | WP5 deliverable (SoW §8) |
| W5-08 | Write the SoW §4.1 justification for not reusing libliftoff into the upstreaming proposal, using the contract difference and memory model (section 4) | SoW §4.1 requires it |
| W5-09 | Decide how `DriverProfile::Imx95Dpu` could reach upstream, if at all: a per-driver table has no counterpart in generic KMS, so it may stay project-local | Avoids promising an upstream path that does not exist |
| W5-10 | Coexistence evidence: leased output while Weston owns another connector, if the board can show two outputs. The Verdin iMX95 setup in this plan has one panel; may be blocked by hardware (inferred) | WP5 and SoW §4.2 evidence |
| W5-11 | Reproducible demo and benchmark evidence for the proposal; depends on `docs/m1-imx95-report.md`, which does not exist yet | WP5 deliverable |

## 7. What this analysis does not claim

- No claim of measured advantage of this planner over libliftoff or Weston. There is no measurement.
- Rows tagged source cover only the files and lines named. The Weston read in particular is partial (W5-01).
- No claim that an upstream proposal would be accepted (SoW WP5 gives none).

## 8. Status of the gaps at HEAD

Updated 2026-11-28. A gap is marked closed only where a file at HEAD answers it; the file is named. Dates are the commit dates from `git log` of the commit that added the file, and all of them are 2026-11-27. Closed means the file exists and covers the gap's wording; it does not mean its content is verified beyond what the file itself says.

| Gap | Status | Closing file (added, commit) | What stays open |
| --- | --- | --- | --- |
| W5-01 / WP5-01: which Weston the image ships, and whether it has lease code | Closed as a source read | `docs/weston-imx-lease.md` (2026-11-27, `58a8712`) | That the Toradex 7.8.0 image uses NXP `weston-imx` is inferred, not established (the file's section 5). `docs/weston-release-read.md` (2026-11-28) read upstream `12.0.5`, the base the `weston-imx-12.0.5` branch states; NXP's own changes on top were not read |
| W5-02 / WP5-02 and W5-03: the full Weston read at a released tag | Closed as a source read, in part | `docs/weston-release-read.md` (2026-11-28, `946ee05`) | Read from source at upstream tags `16.0.2` (`530ae6c`) and `12.0.5` (`3d2cd99`): neither supports a gap-analysis §3 route without a Weston change, and neither has a hook for `composition_core::advise`. Open: NXP's `weston-imx-12.0.5` changes were not read; `kms.c`, `modes.c`, renderers and shells were not read; nothing was run or measured on a board |
| W5-04 / WP5-03: advisory or planner-only entry point | Design only | `docs/advisory-planner-design.md` (2026-11-27, `84dd94c`) | Open: it is a proposal. Nothing is implemented in `crates/` or `include/`, and its decisions are still open |
| W5-05 / WP5-04: cross-check against libliftoff | Design only | `docs/libliftoff-crosscheck.md` (2026-11-27, `4437b16`) | Open: no harness exists and nothing was run |
| W5-07 / WP5-05: `NOTICE` | Closed in part | `NOTICE` (2026-11-27, `21b1653`) | Copyright holder, below |
| W5-07 / WP5-05: licences of `drm-lease-manager` and `libdlmclient` | Closed | `docs/third-party-licences.md` (2026-11-27, `21b1653`), section 3.1 | The file lists items it did not read |
| W5-07 / WP5-05: contribution guidelines | Closed | `CONTRIBUTING.md` (2026-11-27, `107c5c9`), with `.github/pull_request_template.md` (`c86231d`) | Sign-off policy: `CONTRIBUTING.md` has no sign-off or DCO text, so it is open |
| WP5 minimal documentation: README | Closed | `README.md` (2026-11-27, `4f99c33`) | |
| WP5 minimal documentation: terms | Closed | `docs/glossary.md` (2026-11-27, `fa5546b`) | |
| WP5 minimal documentation: change record | Closed | `CHANGELOG.md` (2026-11-27, `039d02f`) | |
| WP5: reporting security issues | Closed | `SECURITY.md` (2026-11-27, `38ba15a`) | |
| WP5: reporting dated board results | Closed | `.github/ISSUE_TEMPLATE/board-result.md` (2026-11-27, `22f18ff`) | |
| Repeatable checks: links, FFI header, board secrets, Zephyr app layout, all CI checks | Closed | `scripts/check-doc-links.sh` (`920e6f1`), `scripts/check-ffi-header.sh` (`2a06164`), `scripts/check-no-board-secrets.sh` (`582b919`), `scripts/check-zephyr-apps.sh` (`a119a09`), `scripts/check-all.sh` (`3686047`); all 2026-11-27 | |

Still open, with no file at HEAD closing them:

- **W5-02 / WP5-02 and W5-03: what the released-tag read left out.** The source read is in `docs/weston-release-read.md`. Still open: the NXP `weston-imx-12.0.5` patches, the files that read skipped, and any board evidence of how the shipped Weston places planes.
- **W5-06 / WP5-06 and W5-08: the SoW §4.1 justification for not reusing libliftoff.** Still a desk argument; `docs/libliftoff-crosscheck.md` only proposes the cross-check.
- **W5-07: the copyright holder.** `LICENSE` names none, and `NOTICE` uses the placeholder "The hwlayer-manager contributors" until the owner decides (`docs/third-party-licences.md`, section 1). The choice of Apache-2.0 or MIT for any upstream offer is not recorded as decided either.
- **W5-07: the sign-off policy.**
- **W5-09: the upstream path of `DriverProfile::Imx95Dpu`.**
- **W5-10 / WP5-07: coexistence evidence.**
- **W5-11 / WP5-08: reproducible demo and benchmark evidence.** `README.md` says these are not produced, and `docs/m1-imx95-report.md` is not present at HEAD.

The numbered rows in sections 1 to 7 keep their original wording; section 8 is the current status.
