# WP5 gap analysis: leased output, Weston-owned output, libliftoff

Desk analysis for SoW §4.2 and WP5. Written 2026-11-28. Nothing here is measured.

## Read this first: what was and was not read

The task asked for citations of the Weston and libliftoff commits that were read. **None were read.** In this session `git clone`, `curl` and WebFetch against gitlab.freedesktop.org were all refused by the sandbox's permission layer, and no copy of either project is on the machine. Reading them is therefore the first open issue (W5-01), and every upstream statement below is tagged **recalled**, not **source**.

Tags:

- **source**: read in a file of this repository, or in a source already recorded in `docs/platform-capabilities.md` (cited by file and function). Checked in this session for this repository; for drm-lease-manager, as recorded there on 2026-11-19, not re-read.
- **recalled**: from the author's prior knowledge of Weston or libliftoff. Not checked against any commit. It may be wrong in detail, outdated, or wrong about which release has a feature. Do not quote it in an upstreaming proposal before W5-01 is done.
- **inferred**: a conclusion drawn from the other rows.

No Weston or libliftoff commit hash is given, deliberately: a hash written down without having been read would look like a citation and be none.

## 1. This project, as it stands

| Statement | Basis |
| --- | --- |
| The planner maps every layer of a frame to one hardware plane or rejects the frame. There is no fallback composition and no "composition layer" | source: `crates/composition-core/src/planner.rs` module doc |
| Search: bounded depth-first backtracking, layers bottom to top, deterministic, finds *a* legal assignment, no cost ranking. Limits: 20 000 steps, 32 backend checks (`SearchLimits::default`) | source: `planner.rs` |
| Each legal assignment is offered to a backend check, which can answer `Accept`, `TryNext` or `GiveUp`. On Linux the check is an atomic commit with `TEST_ONLY` (plus `ALLOW_MODESET` when a modeset is needed) | source: `planner.rs` `Verdict`, `plan_with`; `crates/composition-drm/src/lib.rs` `check` |
| A refusal names the unmet constraint as a bit set (`OUTPUT`, `FORMAT`, `SCALING`, `SOURCE_SIZE`, `ALPHA`, `BLEND`, `TRANSFORM`, `FULL_OUTPUT`, `Z_ORDER`, `RESOURCE`, `IN_USE`) | source: `planner.rs` `RejectReasons` |
| Layers reaching beyond the output are clipped exactly, or refused (`CannotClip`) | source: `planner.rs` `clip_to_output` |
| Shared hardware (scalers, fetch units) is modelled as resource groups with capacity; consumers are layer traits (`SCALED_H/V`, `YUV`, `ANY`). The i.MX95 profile fills them from driver source only | source: `caps.rs` `ResourceGroup`; `profile.rs` `DriverProfile::Imx95Dpu` (marked "read from source, not measured") |
| A frame is validated against *capabilities*, which the backend builds from what the fd exposes. A lease fd lists only leased objects, so capabilities cover only the lease | source: `composition-drm/src/lib.rs` module doc |
| The backend takes an fd (device node or lease); how it was obtained is not its concern. Without `zpos` it assumes primary below overlays below cursor | source: `lib.rs` module doc, `DrmBackend::from_fd` |
| Lease revocation and lost master show as `EACCES`/`ENOENT`/`ENODEV`, mapped to `BackendEvent::DeviceLost`; `EBUSY` to `Busy`; `EINVAL`/`ERANGE`/`ENOSPC` to `Rejected` | source: `lib.rs` `fail` |
| Commit is `NONBLOCK | PAGE_FLIP_EVENT`; the output keeps one pending submission | source: `lib.rs` `commit` |
| Beyond planning the engine tracks buffer state writable → submitted → latched → released, with dropped frames observable (SoW §3.1, §7) | source: `crates/composition-core/src/presentation.rs`, `engine.rs`; SoW §3.1 |
| `composition-core` is `no_std`, bounded memory, with a C ABI | source: `crates/composition-core`, `crates/composition-ffi`; SoW §3.2 |
| Lease sets and revocation behaviour of AGL `drm-lease-manager` (default lease, shared overlays left out on i.MX95, TOML plane list, `ENOENT` after revocation) | source: `docs/platform-capabilities.md`, section "AGL drm-lease-manager" (AGL gerrit `src/drm-lease-manager` `master` at `68c48758c46d`, read 2026-11-19, not re-read) |
| SoW's own statement: DRM leasing is exclusive at object level and is not a way for independent processes to drive one CRTC; a Weston-owned CRTC cannot be modified through a lease | source: SoW §2 principles, §4.1, §4.2 |

## 2. What Weston already offers

| Statement | Basis |
| --- | --- |
| Weston's DRM backend owns the CRTCs, connectors and planes of its outputs and is the only committer for them. It gets master through its launcher (logind/seatd/direct) | recalled (`libweston/backend-drm/`, `drm.c`, `launcher-*`) |
| Plane assignment is done in `libweston/backend-drm/state-propose.c`. Per output it proposes a state in one of several modes (planes only, mixed renderer-and-planes, renderer only), tries to put each `weston_view` on a plane, lets the GL or Pixman renderer composite the rest into the primary plane's buffer, and validates with an atomic `TEST_ONLY` commit | recalled |
| The assignment is Weston's own greedy walk over views in z-order, not a search over plane/layer permutations. It uses `zpos` ranges where the kernel gives them | recalled |
| Weston has a build and a runtime path where a connector is offered to a Wayland client as a DRM lease through `wp_drm_lease_device_v1` (the staging protocol `drm-lease-v1`, wayland-protocols). The client receives a lease fd. The offered connector is withheld from Weston's own use while leased. Which connectors are offered is configured per output (a `drm-lease` setting) and typically targets non-desktop connectors | recalled; the exact option name, meson switch and minimum versions are not known and must be read (W5-02) |
| What the lease contains (connector, one CRTC, which planes) is decided by Weston's code, not by the client | recalled; the AGL manager's rule, by contrast, is recorded in section 1 |
| The protocol is about leasing a whole connector. Nothing in it lets a client place a layer on a plane of a CRTC Weston keeps | recalled; consistent with SoW §4.2 (source) |
| Direct scanout of client buffers by Weston is driven by `linux-dmabuf` feedback (scanout tranches) and Weston's view-to-plane policy. The client does not choose planes | recalled |
| Weston has plugin APIs for its DRM backend (output-level settings such as mode and format). None known that lets an outside party assign planes or submit a commit | recalled; verify (W5-03) |
| Licence: MIT | recalled (`COPYING`) |

## 3. Standalone leased-output mode against Weston-owned output mode

| Question | Standalone leased output (SoW §4.2 mode 1) | Weston-owned output (mode 2) | Basis |
| --- | --- | --- | --- |
| Who commits the CRTC | This engine, alone, through the lease fd | Weston, alone | source: SoW §4.2; inferred for the Weston side from the rows above |
| Who sees the planes | Only what the lease exposes; the engine plans over those | All planes; Weston plans, and the engine would only influence | source: `lib.rs`; recalled for Weston |
| Weston changes needed | None for the output, if a lease with primary and overlays exists. If Weston itself provides the lease, its policy for the plane set must include overlays | A protocol or plugin. See below | source: SoW §4.2; inferred |
| Producer path | Producers send buffers to the engine process (a harness for M1) | Producers are Wayland clients; Weston turns their surfaces into views | source: SoW §4.1; recalled |
| Plane choice | The engine's planner | Weston's `state-propose.c` | inferred |
| Buffer lifecycle | The engine's state machine, with latch and release as separate events | Wayland `wl_buffer.release` and presentation-time feedback | inferred; `wl_buffer.release` and `wp_presentation` recalled |
| Failure case | Engine or lease fd lost: output is dark or falls back to whatever the lease manager does on revocation | Weston crash takes the output with it | inferred |
| Fits M1 | Yes; this is what M1 tests | No; SoW calls it later investigation | source: SoW §4.2, §9 |

### What the Weston-owned mode would need

The engine cannot commit Weston's CRTC (source: SoW §4.2). Three ways to put this engine's work into that mode, none of them built and none required by M1:

1. **Weston as lease provider.** Weston leases a connector with the planes the engine needs to this engine. This is the standalone mode with Weston as the lease source; the output leaves Weston while leased. Needs: Weston's lease policy to include overlays and to be configurable by plane (open question), and the engine to work with a lease from the Wayland protocol (it accepts any fd). (inferred)
2. **A plane-placement hint into Weston's planner.** A protocol or compositor-internal API by which a client or integration marks a surface as "needs its own plane, with these constraints" and reads back the assignment. Weston still commits. Needs: a Weston patch to `state-propose.c`, or a Weston plugin if one can be hooked in there, and a decision whether this engine's `Plan` is advisory (Weston may refuse) or binding. This project's `Plan` is binding by design (SoW §6 item 7: reject rather than weaken), which does not map onto an advisory hint without change. (inferred)
3. **Weston calls this engine.** The engine is linked into Weston as its plane allocator (see section 4) and Weston supplies buffer metadata and commits the result itself. Needs the engine to expose the planner without the committing backend. `plan_with` already takes a backend check as a closure, so the planner can run with Weston's own `TEST_ONLY`; the `Engine` type bundles the commit path, so a planner-only entry point would be new API. (inferred from `planner.rs`)

All three need Weston code or Weston-side configuration. This is the gap SoW §4.2 expects.

## 4. Overlap with libliftoff

| Statement | Basis |
| --- | --- |
| libliftoff is a small C library by Simon Ser (emersion) that allocates KMS planes to layers. The caller creates an output per CRTC, planes from the plane resources, and layers described as sets of KMS properties (`FB_ID`, `CRTC_X`, `CRTC_W`, `SRC_*`, `zpos`, `alpha`, ...). The library chooses a plane per layer, and writes the properties into a `drmModeAtomicReq` the caller commits | recalled |
| It validates candidates with atomic `TEST_ONLY` commits and tries to keep their number small, with heuristics (priorities, reusing the previous allocation, checking a plane's format list before a test commit). A layer it cannot place is left to the caller as a "composition layer" (the caller renders the remainder into one buffer) | recalled |
| Consequence of the last row: libliftoff's central feature is the fallback this project forbids. It assumes the caller has a renderer; this engine has none | inferred |
| It works on KMS property sets and `drmModeAtomicReq`; it has no notion of buffer ownership, latch, release or events. The caller commits and handles page-flip events | recalled |
| Written in C99 with libdrm and `malloc`. No bounded-memory mode | recalled |
| Licence: MIT | recalled (`LICENSE`) |
| Users include wlroots, gamescope and others | recalled; irrelevant to the decision, listed only because it bears on how a contribution would be received (inferred) |

### Where the two overlap, and where they do not

| Topic | libliftoff | This project | Reuse, contribute, or leave |
| --- | --- | --- | --- |
| Layer-to-plane search with `TEST_ONLY` | yes | yes (backtracking, verdict per candidate) | Overlap. Two implementations of the same job (inferred). Reuse is blocked by language and by `no_std` (inferred); SoW §4.1 asks to avoid "unnecessary duplicate algorithms", so the justification has to be written down: the bounded `no_std` core must run on MCUs where there is no libdrm (source: SoW §3.2, §5) |
| Fallback composition | yes, via caller | no, by design | Not shared |
| Typed reasons for refusal | partial at best (recalled: it logs) | `RejectReasons` per layer | Could be contributed as an idea, not as code (inferred) |
| Shared resources (scalers, fetch units) | the kernel's `TEST_ONLY` is the only authority (recalled) | resource groups, profile per driver | Possibly of interest to libliftoff users, but a per-driver profile is not generic KMS; speculative (inferred) |
| Clipping to the output | not known | exact clipping or refusal | Unknown (needs W5-01) |
| Buffer and presentation state machine | none | the core of the engine | Not shared; no counterpart |
| Bounded memory, `no_std`, C ABI | no | yes | Not shared |
| Property-level API for arbitrary KMS properties | yes | no: typed layer fields and a fixed capability model | libliftoff is more general for KMS; this project more general across non-KMS targets (inferred) |

### Options, in the order of cost

1. **Keep separate and document why** (cheapest; consistent with section 4.1 provided the reason is written down). No upstream work.
2. **Use libliftoff's allocation as a cross-check in tests**: run both on the same capability dump and compare the sets of frames accepted, to find planner bugs (inferred). Only needs a dev dependency on a host with libdrm; best fed with the observed i.MX95 topology from `docs/platform-capabilities.md` once WP0 has it.
3. **A libliftoff-compatible front** over `composition-drm`: expose a liftoff-like property-set API from the C ABI so a compositor that already uses libliftoff can swap this engine in (inferred; large, and a decision on `Plan` binding versus advisory is needed first).
4. **Contribute ideas upstream**: refusal reasons, resource groups for shared units. Only worth it if the i.MX95 measurements show libliftoff's heuristics getting the DPU wrong, which nothing yet shows (inferred).

## 5. Licensing

| Statement | Basis |
| --- | --- |
| This repository is Apache-2.0 | source: `LICENSE` |
| Weston and libliftoff are MIT | recalled; confirm in W5-01 |
| MIT code can be included in an Apache-2.0 work, with the MIT notice and copyright kept; that is the usual reading | inferred; not legal advice |
| Contributing this project's code to an MIT project means offering it under MIT. The copyright holders can do that for their own code; Apache-2.0's patent grant then does not travel with it. Code from third parties in this repository would need their consent | inferred |
| Linking: an Apache-2.0 library linked into an MIT program has no conflict as such; the combined binary carries both notices | inferred |
| No contribution guidelines, `NOTICE` file or DCO/CLA decision exist in the repository yet | source: repository contents at `69706c8`; WP5 asks for them |
| AGL components have their own licences (drm-lease-manager's was not recorded) | not recorded; W5-07 |

## 6. Open issues for WP5

Numbered for the tracker; none are filed (filing is outside this item).

| ID | Issue | Why it matters |
| --- | --- | --- |
| W5-01 | **Read Weston `state-propose.c` and libliftoff `alloc.c`, `include/libliftoff.h`, `LICENSE` at pinned commits; replace every "recalled" row above with "source" and the hash.** Not done in this item because upstream was unreachable | Everything in sections 2, 4 and 5 rests on memory until then |
| W5-02 | Weston DRM lease: find the exact option, meson switch, minimum Weston/wayland-protocols/libdrm versions, and which objects it puts in a lease. Check whether planes beyond the primary can be included, and what the lease looks like on i.MX95 (overlays shared between two CRTCs, see `docs/platform-capabilities.md`) | Decides whether mode 1 of section 3 can use Weston as the lease source. Board-side Weston lease support is covered by the plan item `lease-feasibility` |
| W5-03 | Weston plugin or protocol surface: is there any hook at plane assignment where an external party could constrain or read the result? | Decides feasibility of option 2 of section 3 |
| W5-04 | Decide whether `Plan` stays binding or gets an advisory form, and whether a planner-only entry point (no committing backend) is exposed, in Rust and in the C ABI | Needed for section 3 options 2 and 3 and for the libliftoff-compatible front |
| W5-05 | Cross-check the planner against libliftoff on shared capability dumps (dev-only), with the observed i.MX95 topology after WP0 | Cheapest evidence for or against duplication |
| W5-06 | Write the SoW §4.1 justification for not reusing libliftoff (language, `no_std`, no composition fallback, buffer lifecycle) into the upstreaming proposal, once W5-01 confirms the facts | SoW §4.1 requires it |
| W5-07 | Licensing and contribution set: decide Apache-2.0 versus MIT for any upstream offer, add `NOTICE`, contribution guidelines and a sign-off policy; record the licence of drm-lease-manager and libdlmclient | WP5 deliverable; SoW §8 |
| W5-08 | Record how `DriverProfile::Imx95Dpu` (shared fetch units, scalers) would reach upstream, if at all: a per-driver table has no equivalent in generic KMS, so it may stay project-local | Avoids promising an upstream path that does not exist |
| W5-09 | Coexistence evidence: run the leased-output mode on the board while Weston owns another connector, if the board can show two outputs, and record that the unleased output is unaffected | Needs a second output; the Verdin iMX95 setup in this plan has one panel. May be blocked by hardware (inferred); plan items `m1-lease-run` and `lease-feasibility` give the first evidence |
| W5-10 | Reproducible demo and benchmark evidence for the proposal: depends on M1 results (`docs/m1-imx95-report.md`), none exist yet | WP5 deliverable |

## 7. What this analysis does not claim

- No statement that Weston or libliftoff cannot do something is established here; section 2 and 4 rows say what the author recalls.
- No claim of measured advantage of this planner over either. There is no measurement.
- No claim that an upstream proposal would be accepted (SoW WP5 gives none).
