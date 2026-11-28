# WP5 upstreaming proposal: libliftoff, Weston and AGL meta-agl-drm-lease

Date: 2026-11-28. Desk work. No board was touched for this page, nothing was run or measured for it, and nothing was posted upstream.

This is the "upstreaming proposal" deliverable of SoW §8 WP5. The gap analysis it rests on is [`wp5-gap-analysis.md`](wp5-gap-analysis.md). The page says what this project could offer to two upstream targets, in what form, under which licence, and what the owner must still decide:

1. libliftoff;
2. Weston, and AGL's `meta-agl-drm-lease` layer with `drm-lease-manager`.

**No upstream acceptance is promised.** SoW §8 WP5 says the deliverable comes with "no guarantee of upstream acceptance". Nothing here has been shown to an upstream maintainer.

## Tags

Every factual statement carries one of these:

- **observed &lt;date&gt;**: seen on a board on that date (for example 2026-11-27), as recorded in the named document. This page adds no observation.
- **read from source (&lt;date&gt;)**: read in a named file at a named commit on that date. "(this repo)" means this repository at the commit this page was written on.
- **recorded (&lt;doc&gt;)**: written by an earlier item in the named document, and not re-read here.
- **inferred**: a conclusion from the tagged rows.
- **host only**: run on a development host (host tests, the cross-check harness, host metrics). Not a hardware result.
- **not measured**: no document records a figure.

Proposals and owner questions carry no tag. They are not facts.

## 1. What could be offered

| # | Item | Where it is | State |
| --- | --- | --- | --- |
| O1 | Stateless advisory planner, Rust: `composition_core::advise` (`advise`, `explain_layer`, `Advice`, `Blocker`, `AdviceError`, `AdviseOptions`). For a frame it says which layers go on which planes, which contiguous run the caller must compose into one buffer, and why. It never commits, keeps no state between calls and is deterministic | [`crates/composition-core/src/advise.rs`](../crates/composition-core/src/advise.rs) | read from source (2026-11-28, this repo); host tests only ([`advisory-planner-design.md`](advisory-planner-design.md) status paragraph) |
| O2 | The same as a C ABI: `ce_advise`, `ce_explain_layer` and `ce_abi_check_advise` (macro `CE_ABI_CHECK_ADVISE()`). Takes the caller's `ce_capabilities`, a `ce_buffer_entry` table and an optional `check` callback, for example a DRM `TEST_ONLY` commit. Nothing allocates | [`composition_engine.h`](../crates/composition-ffi/include/composition_engine.h), [`crates/composition-ffi/src/lib.rs`](../crates/composition-ffi/src/lib.rs) | read from source (2026-11-28, this repo); a C caller linked against the static library on the host only. No board and no compositor has called it (advisory-planner-design §7) |
| O3 | `DriverProfile` capability data: shared hardware (fetch units, scalers) as resource groups with a capacity. `DriverProfile::Imx95Dpu` fills them for the i.MX95 DPU | `crates/composition-drm/src/profile.rs`; [`platform-capabilities.md`](platform-capabilities.md) | Topology it assumes: observed 2026-11-28 ([`platform-capabilities.md`](platform-capabilities.md)). Resource-group values: read from source (driver 6.6.157-7.8.0), **not measured** (KMS does not show them) |
| O4 | The standalone leased-output route (SoW §4.2 mode 1): the engine takes a lease fd from AGL `drm-lease-manager`, discovers exactly the leased objects and owns that output. No Weston change | `crates/composition-drm`, `tools/drm-harness`, [`dlm-aarch64.md`](dlm-aarch64.md) | observed 2026-11-27 on the Verdin iMX95 with `drm-lease-manager` `68c48758c46d` and planes 31 and 51 in the TOML file ([`m1-imx95-report.md`](m1-imx95-report.md), Condition 6) |
| O5 | Ideas and findings, not code: typed refusal reasons per layer, resource groups, the cross-check findings (lead L-1, the zpos restacking question) | [`wp5-gap-analysis.md`](wp5-gap-analysis.md) §4; [`liftoff-crosscheck-report.md`](liftoff-crosscheck-report.md) | host only |

The engine form of the advisory planner (`Engine::advise`, `ce_engine_advise`) does not exist (read from source (2026-11-28, this repo)). It is not offered.

## 2. Licences

| Project | Licence | Basis |
| --- | --- | --- |
| This repository | Apache-2.0. `LICENSE` names no copyright holder; `NOTICE` uses the placeholder "The hwlayer-manager contributors" | read from source (2026-11-28, this repo): `LICENSE`, `NOTICE`, `Cargo.toml`; [`third-party-licences.md`](third-party-licences.md) §1 |
| libliftoff | MIT, "Copyright (c) 2019 Simon Ser" | recorded ([`third-party-licences.md`](third-party-licences.md) §3.4, read 2026-11-28 at `c4226a7`). The README at that commit ends with "## License" and "MIT": read from source (2026-11-28) |
| Weston | MIT, X.org wording of the Expat licence (`COPYING`) | recorded ([`third-party-licences.md`](third-party-licences.md) §3.5, read 2026-11-28 at `c8d00e2`) |
| drm-lease-manager and libdlmclient | Apache-2.0; `Copyright 2020-2021 IGEL Co., Ltd.` in the source headers | recorded ([`third-party-licences.md`](third-party-licences.md) §3.1, read 2026-11-28 through the Gerrit REST API at `68c48758c46d`) |
| AGL `meta-agl`, layer `meta-agl-drm-lease` | not read | nobody has read the layer's licence file. The AGL Weston patches in that layer are recorded in [`weston-imx-lease.md`](weston-imx-lease.md) row S7 |

What follows for an offer. All of it is **inferred, not legal advice**:

- **To libliftoff or Weston as code:** both are MIT. Code from this repository can go into them only if its copyright holders offer it under MIT. The repository names no holder yet, so first the owner must name one.
- **Apache-2.0 patent grant:** it does not travel with code that is offered under MIT.
- **Dual licence:** offering O1/O2 as "Apache-2.0 OR MIT" would allow both uses. That needs the consent of every copyright holder of the files concerned.
- **As a separate library:** an Apache-2.0 library linked into an MIT program does not conflict. The combined work carries both notices. No relicensing is needed.
- **To `drm-lease-manager` or AGL:** the licence of `drm-lease-manager` is the same as this repository's. The `meta-agl-drm-lease` licence is not read, so no statement is made.
- **Ideas and data (O5):** findings written up in an issue are not code. They need no licence decision. A copy of the cross-check frame files would still be Apache-2.0 files of this repository.

## 3. Upstream contribution rules

### 3.1 Weston

Read from source (2026-11-28): a shallow clone of `https://gitlab.freedesktop.org/wayland/weston.git`, branch `main`, made on 2026-11-28. Its head is `c8d00e2a1dbc98f4a8fb6473a4b485ffce409f3a`, committed 2026-11-27, the same commit that the gap analysis read. Files read in full: `CONTRIBUTING.md`, `LLM-USAGE.md` and `AGENTS.md`. Tag `16.0.2` was not compared. In the quotes, Weston's relative links are shown as code so that they do not point into this repository.

**DCO and sign-off.** `CONTRIBUTING.md`, section "Formatting and separating commits":

> Your commit messages must also include a Signed-off-by line with your name
> (or pseudonym) and email address which indicates that you agree to the
> [Developer's Certificate of Origin 1.1]`(DCO-1.1.txt)`.

> **Agreeing to DCO 1.1 is mandatory.** Patches without a Signed-off-by cannot
> be accepted, but using a pseudonym is fine as long as the email address is
> yours personally.

**AI contributions.** `CONTRIBUTING.md`, section "LLM-Policy", points to `LLM-USAGE.md` ("Please read it before opening a MR if you want to use any LLM or AI tools"). `LLM-USAGE.md` says, under "Upstream interaction rules":

> AI agents must not use any API, CLI, or web UI automation to:
>
> - create, edit, or close issues ("work items"),
> - post comments on merge requests, issues, or commits, nor
> - open or update merge requests.
>
> Prose written by an AI agent must not be forwarded upstream unless it is
> concise, and fully reviewed and copyedited by a human first.

and under "Signed-off-by and Developer Certificate of Origin":

> AI agents MUST NOT add Signed-off-by tags.
>
> Only humans can legally certify the [Developer Certificate of
> Origin]`(DCO-1.1.txt)`.

and under "Responsibility":

> The human contributor is solely responsible for all code committed to Weston
> repositories.

`AGENTS.md` adds:

> - Do not write documentation, commit messages, merge-request text, or code
>   comments for the human unless they explicitly ask you to draft a rough version
>   for review. The human must write the final commit message and any code
>   comments they want to keep.

**Licence and review.** `CONTRIBUTING.md`, section "Licensing": "New source code files should specify the MIT Expat license in their boilerplate". Section "Review" lists, among others: "The code license is the same MIT licence the project generally uses."; "Stable ABI or API additions must be justified by actual use cases, not only by speculation."; "The code does not depend on API or ABI which has no working free open source implementation.". Section "Increasing requirements" limits new or newer dependencies for the "DRM-backend" and the other foundational components.

These quotes agree with the gap analysis' summary ([`wp5-gap-analysis.md`](wp5-gap-analysis.md) §2). Before any contact, a human should read the files again at the then-current `main`.

### 3.2 libliftoff

Read from source (2026-11-28): `README.md` at `c4226a79b7a52f59bb51789b1eda112658609125`, in the libliftoff clone in the project's dependency directory (its `HEAD` is that commit). The GitLab API gave the same commit as the head of `master` on 2026-11-28 (committed 2026-08-24). Under "## Contributing" the README says:

> Report bugs and send pull requests on [GitLab][gitlab].
>
> We use the Wayland/Weston style and contribution guidelines, see [Weston's contributing document][weston-contributing].

The `weston-contributing` link points at Weston's `CONTRIBUTING.md` on branch `master`. The working tree at that commit has no `CONTRIBUTING.md`, `AGENTS.md` or `LLM-USAGE.md` of its own (read from source (2026-11-28): file list). So libliftoff's own rules are Weston's, and section 3.1 applies, including the DCO (inferred: the README names "contribution guidelines" and does not say whether `LLM-USAGE.md` is meant too). libliftoff has no separate statement on AI contributions.

### 3.3 AGL (`meta-agl`, `drm-lease-manager`)

The AGL contribution rules (Gerrit, sign-off) were not read. The AGL Weston lease patches carry `Upstream-Status: Pending` (recorded ([`weston-imx-lease.md`](weston-imx-lease.md) row S8, read from source at `meta-agl` tag `unagi_20.92.0` on 2026-11-28)).

### 3.4 What this means for this repository

- Most commits of this repository are written with an AI coding assistant: 486 of the 622 commits reachable from this page's parent commit name it as co-author in a trailer (read from source (2026-11-28, this repo): `git log`).
- Under Weston's rules (section 3.1), no such commit can be sent to Weston or libliftoff as it is. A human would have to write or take over the change, review it and add their own `Signed-off-by`. A human would also have to open any issue or merge request. Under `AGENTS.md`, a human would have to write the final commit message and merge-request text too. Text drafted from this page counts as AI prose and would need full human review and copyediting (inferred from the quotes).
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) has no sign-off or DCO text (read from source (2026-11-28, this repo)). So this repository's history gives no DCO record to point at.
- This page posts nothing and contacts nobody.

## 4. Target 1: libliftoff

### 4.1 What would be offered

| Offer | Why it could matter to libliftoff | Evidence |
| --- | --- | --- |
| Typed refusal reasons per layer, as an idea (O5) | libliftoff returns only `liftoff_layer_needs_composition()` and a candidate-plane query. Its reasons are debug-log text | read from source at `c4226a7` (recorded, [`wp5-gap-analysis.md`](wp5-gap-analysis.md) §4). Usefulness to libliftoff users: **not measured** |
| Resource groups (shared scalers and fetch units) as an idea, with the i.MX95 DPU as the example (O3, O5) | libliftoff learns about shared hardware only from the kernel's test commit | recorded (gap analysis §4). Whether it saves test commits: **not measured** |
| Cross-check findings, as data (O5) | Lead L-1: libliftoff skips a property the plane lacks when the layer's value is 0, so an ARGB layer marked opaque lands on a plane without `pixel blend mode`. Also the zpos restacking difference on the observed i.MX95 plane set | host only, 2026-11-28 ([`liftoff-crosscheck-report.md`](liftoff-crosscheck-report.md), "Lead L-1" and "Step (f)"). Whether either matters on a kernel: **not measured** |
| The advisory planner (O1/O2) as a separate library, **not** as a patch to `alloc.c` | It answers the same question as `liftoff_output_apply` (planes plus a composed remainder), with a bounded, allocation-free search | read from source (2026-11-28, this repo). Same answers as libliftoff on hardware: **not measured** |

Not offered: the engine's buffer lifecycle, the `Plan` commit path or the `no_std` targets. libliftoff has no buffer or presentation state at all (recorded, gap analysis §4). So these have no place in it (inferred).

### 4.2 Form

In order. Each step waits for the one before it.

1. **An issue, written by a human:** typed refusal reasons and resource groups, with the cross-check findings as data. No code.
2. **Only if maintainers ask:** a small patch by a human author under MIT. This needs the licence decision D1 (section 7).
3. **Otherwise:** keep O1/O2 here as a separate library that a compositor may call next to, or instead of, libliftoff. This needs no upstream step.

Not proposed: a libliftoff-compatible front over the C ABI. It is large, and the contract difference blocks it (recorded, gap analysis §4 option 4).

### 4.3 Licence

- **Ideas:** no licence decision.
- **Code into libliftoff:** MIT, which needs D1 and D2.
- **Separate library:** stays Apache-2.0. The combined work carries both notices (inferred).

### 4.4 Interfaces or patches libliftoff would need

None for the separate-library form. For refusal reasons in libliftoff itself, a new public query next to `liftoff_layer_needs_composition()` (inferred). No such patch is drafted.

## 5. Target 2: Weston, and AGL meta-agl-drm-lease with drm-lease-manager

### 5.1 What upstream has today

| Fact | Tag |
| --- | --- |
| Upstream Weston has no lease code at `main` `c8d00e2`, nor at tags `16.0.2` and `12.0.5` | recorded ([`wp5-gap-analysis.md`](wp5-gap-analysis.md), [`weston-release-read.md`](weston-release-read.md) row R1; read from source 2026-11-28) |
| NXP `weston-imx` has no lease code at `weston-imx-12.0.5`, `14.0.2` and `15.0.1` | recorded ([`weston-imx-lease.md`](weston-imx-lease.md) rows S3-S5; read from source 2026-11-28) |
| The board image's Weston reports version 12.0.4 | observed 2026-11-27 ([`m1-imx95-report.md`](m1-imx95-report.md), "Condition 7 fields") |
| Weston 12.0.5 and 16.0.2 have no hook where `advise` could be consulted. The only plane-assignment entry point is `weston_output::assign_planes`, built on private backend state. Neither can refuse a frame | recorded ([`weston-release-read.md`](weston-release-read.md) §1, §7; read from source 2026-11-28) |
| AGL `meta-agl-drm-lease` patches Weston 13.0.x so that it becomes a lease **client** of `drm-lease-manager` (`--drm-lease=<name>`). Status `Upstream-Status: Pending`. Its README says it is tested on Renesas R-Car Gen3 | recorded ([`weston-imx-lease.md`](weston-imx-lease.md) rows S7-S9; read from source at `meta-agl` tag `unagi_20.92.0` on 2026-11-28) |
| `drm-lease-manager` gives a default lease per connector with one CRTC and the planes usable only with it. On i.MX95 the overlays are shared by both CRTCs, so they must be listed in the TOML file | recorded ([`platform-capabilities.md`](platform-capabilities.md) "AGL drm-lease-manager"; read from source at `68c48758c46d`) |
| With `planes = [31, 51]` the lessee saw exactly planes 31 and 51, and two-plane, `rotate` and external-producer runs passed. Weston was stopped during every run | observed 2026-11-27 ([`m1-imx95-report.md`](m1-imx95-report.md), Condition 6) |
| Weston owning one connector while the engine holds a lease on another: no evidence. The Verdin setup has one panel | recorded ([`m1-imx95-report.md`](m1-imx95-report.md); open issue WP5-07). Coexistence: **not measured** |

### 5.2 What would be offered

| Offer | Recipient | Form | Evidence behind it |
| --- | --- | --- | --- |
| The standalone leased output as a use of `drm-lease-manager` with no Weston change (O4) | AGL (`meta-agl-drm-lease` README or the lease-manager documentation) | A report from a non-R-Car platform: an i.MX95 needs an explicit plane list because its overlays are shared. Text only | observed 2026-11-27 (M1 Condition 6). Platforms other than i.MX95: **not measured** |
| A recipe that builds this engine (or the harness) as a `drm-lease-manager` client | `meta-agl-drm-lease` | First an issue or mail by a human. A recipe only if AGL wants one. No recipe is written; whether the layer can build Rust is not read | none; proposal |
| A plane-assignment hook in Weston's DRM backend that consults `ce_advise` | Weston | An RFC issue by a human, with the mismatches of [`weston-release-read.md`](weston-release-read.md) §7 listed. Not a patch | read from source (2026-11-28): no hook exists. That Weston gains anything from it: **not measured** |
| Refusal reasons as a debug aid for Weston's per-view trial and error | Weston | Idea in the same RFC | recorded (advisory-planner-design §5, inferred there). Value: **not measured** |
| Nothing for Weston itself in mode 1 | — | — | SoW §4.2 mode 1; observed 2026-11-27 that the lease route ran with no Weston fork (M1 Condition 6) |

The Weston hook would have to handle these mismatches. All of them are recorded ([`weston-release-read.md`](weston-release-read.md) §7, inferred there):

- `advise` composes one contiguous run at its own place in the stack. Weston's renderer buffer sits on the primary plane at the lowest zpos.
- 12.0.5 cannot place any plane below the renderer buffer. 16.0.2 can, but only with opaque underlays.
- Weston must never refuse a frame, so a hook would have to fall back to Weston's own walk whenever `advise` reports `Budget` or `Backend` blockers.

### 5.3 Licence

- **Weston patch:** MIT, which needs D1 and D2, plus the human sign-off of section 3.1.
- **`ce_advise` as an external library called from Weston:** no licence conflict as such (inferred). Weston's review list asks for "the same MIT licence the project generally uses", and its "Increasing requirements" rules limit new dependencies of the DRM backend (read from source (2026-11-28), §3.1). So an external Apache-2.0 library in the DRM backend is unlikely to pass review unless it is offered under MIT, or is optional (inferred). Weston has not said either way about this case.
- **AGL recipe or documentation:** the layer's own licence, which is not read.
- **Running `drm-lease-manager` next to this engine:** two Apache-2.0 programs, no question (inferred).

### 5.4 Interfaces or patches needed

- **Mode 1 (standalone leased output):** none in Weston. In `drm-lease-manager`, none: the plane list is configuration. Observed 2026-11-27 on one board.
- **Mode 2 (Weston-owned output):** a patch to `libweston/backend-drm/state-propose.c` (`drm_assign_planes` or `drm_output_propose_state`). It would build a `ce_frame` from `paint_node_z_order_list`, call `ce_advise` with a `check` that wraps `drm_pending_state_test`, and fall back to the existing walk. This is recorded ([`weston-release-read.md`](weston-release-read.md) §7, inferred there). No such patch exists.
- **Weston as lease source:** a feature to write, not to configure (recorded, [`weston-imx-lease.md`](weston-imx-lease.md) §1). Not proposed.

## 6. Why not reuse libliftoff (SoW §4.1)

SoW §4.1 asks to "reuse existing established DRM/KMS or libliftoff concepts where advantageous, avoiding unnecessary duplicate algorithms". The argument below uses only what the repository records. Each point says what backs it. Where nothing backs it, it says **not measured**.

### 6.1 Points backed by source or by a recorded run

1. **Different contract.** libliftoff returns the best partial allocation and the caller composites the rest. The engine's planner puts every layer on a plane or refuses the frame (SoW §6). Tag: recorded (gap analysis §4, read from source at `c4226a7` and in this repo). The advisory planner (O1) adds the libliftoff-like question as a separate call and leaves `Plan` binding (read from source (2026-11-28, this repo)).
2. **Memory model.** libliftoff calls `malloc` and `calloc` in every `liftoff_output_apply` and uses `clock_gettime` and linked lists. `composition-core` is `no_std`, has no dependencies and allocates nothing, so it builds for `thumbv7em-none-eabihf`. Tag: recorded (gap analysis §4) and read from source (2026-11-28, this repo). This is what blocks libliftoff on the MCU targets of SoW §5 (inferred).
3. **Buffer lifecycle.** libliftoff has no notion of buffers beyond `FB_ID`, and no latch or release events (recorded, gap analysis §4). The engine tracks them. On the Verdin iMX95, `rotate` with 3 buffers per plane ran 3000 frames with 0 dropped, 0 missed vblanks and 5998 releases in order (observed 2026-11-27, [`m1-imx95-report.md`](m1-imx95-report.md), Condition 5). This shows that the lifecycle works on one board. It is not a comparison with libliftoff, which has no such part.
4. **Named refusals.** On the board, `combos` accepted 16 frames and refused 4 in the planner with named reasons: 7 layers (`OUTPUT | IN_USE`), 2x downscale (`SCALING`), 90° rotation (`TRANSFORM`), unknown format (`FORMAT`). 0 frames were refused by `TEST_ONLY`. Tag: observed 2026-11-27 (M1 report, Condition 4). libliftoff gives no reason through its API (recorded, gap analysis §4).
5. **Shared hardware modelled before the kernel is asked.** `DriverProfile::Imx95Dpu` holds resource groups that libliftoff does not model. The values are read from source and **not measured** on the board (M1 report, Condition 2).
6. **Host cross-check, 2026-11-28 (host only).** On a fake kernel model with libliftoff `c4226a7`, the two planners agree on the `fill`, `zperm`, `fmt`, `tie` and `alpha0` families of every swept input. Every difference falls into an expected class, or is lead L-1. With the full model, no input has a liftoff-only frame. Tag: host only ([`liftoff-crosscheck-report.md`](liftoff-crosscheck-report.md), "Summary per input and model setting" and "Triage of every difference"). So, on these inputs, the engine does not repeat libliftoff's answers by accident, and it does not miss frames that libliftoff takes once shared hardware is modelled. This holds within the harness only (inferred).

### 6.2 Points that count the other way

Also from the record, and part of an honest answer:

- **Search limit.** On `search-effort` `eight-argb-two-nv12-late` the engine stopped with `LimitExceeded` under its default limits. libliftoff gave an answer on the same frame. Tag: host only (cross-check report, "Triage", S7-7).
- **zpos on disjoint overlays.** libliftoff keeps relative zpos only between intersecting layers, so on `calibration/zpos-disjoint-inverted` it placed a frame that the engine refused with `Z_ORDER`. Tag: host only (cross-check report, S7-2).
- **The i.MX95 core-only frames rest on an untested assumption.** On the observed i.MX95 plane set the engine accepted 156 (structural) and 177 (full) frames that libliftoff refused. All of them depend on the engine restacking plane zpos. Whether the i.MX95 driver allows that in one atomic commit is not shown. Tag: host only, with input observed 2026-11-28 (cross-check report, "Step (f)"). Until a board answers it, those frames are **not** an advantage.
- **Reuse between frames.** libliftoff reuses the previous allocation when nothing material changed. The engine's planner does not (recorded, gap analysis §4).
- **More general for KMS, and in production.** libliftoff takes any KMS property by name, and its README says it is used in production by gamescope on Steam Deck devices (read from source (2026-11-28), libliftoff `README.md` at `c4226a7`).

### 6.3 Figures, and what they do not show

| Figure | Value as recorded | Tag | What it does not show |
| --- | --- | --- | --- |
| Planner time on a development host (`Engine::validate` with the fake backend, which accepts everything) | Found plans about 140 ns (1 layer, 2 planes) to about 770 ns (4 layers, 16 planes) median; 2.9 µs for 16 `plain` layers on 16 planes; `LimitExceeded` after about 60 µs median at the worst case | host only, 2026-11-28 ([`host-metrics.md`](host-metrics.md)) | Anything about a board. It is not compared with any board figure here |
| `Engine::validate` on the Verdin iMX95, `TEST_ONLY` included | median 135-142 µs, p99 208-310 µs | observed 2026-11-27 ([`m1-imx95-report.md`](m1-imx95-report.md), "Condition 7 fields") | How much of it is the planner and how much the kernel. Any comparison with libliftoff |
| Submit to latched, `rotate`, master fd | median 11 168 µs, p99 14 490, max 15 020 | observed 2026-11-27 (same) | Input-to-photon latency (not measured) |
| Harness CPU, `rotate` | 34.4 % of one core, almost all of it the harness redrawing both buffers | observed 2026-11-27 (same) | Planner cost alone |
| libliftoff planning time, on host or board | — | **not measured** | — |

The two time rows are not compared with each other. One is a host CPU with a fake backend, the other a board with a kernel test commit.

### 6.4 Claimed advantages without a measurement

Each of these may be argued. None is recorded, so each is **not measured**:

- fewer `TEST_ONLY` commits than libliftoff's per-node test: **not measured**;
- lower planning latency than libliftoff on any target: **not measured**;
- lower CPU, memory or power in a compositor: **not measured**;
- more frames shown on planes on real i.MX95 hardware: **not measured** (the host model only; see 6.2);
- a smaller footprint on an MCU than any alternative: **not measured** (`size_of::<Advice>()` is pinned by a host test, not measured on a board);
- a compositor accepting `advise`'s output: **not measured** (advisory-planner-design §7).

### 6.5 Conclusion

The engine does not reuse libliftoff for three reasons: it has a different contract (refuse or place everything), it is `no_std` with bounded memory, and it owns the buffer lifecycle. All three are in the source of both projects (inferred, from 6.1 points 1-3). It does reuse libliftoff's concepts where the evidence supports that: the advisory form (O1) answers libliftoff's question. The cross-check uses libliftoff as a reference. Intersection-aware zpos and allocation reuse are open ideas to take over (recorded, gap analysis §4 option 3). No performance advantage over libliftoff is claimed.

## 7. Owner decisions

None of these is settled. The proposal cannot go upstream before D1-D3 and D9 are settled.

| # | Decision | Options | Blocks |
| --- | --- | --- | --- |
| D1 | Licence of anything offered as code | Keep Apache-2.0 (separate library only); MIT for the offered files; "Apache-2.0 OR MIT" for `composition-core` and `composition-ffi` | §4.2 step 2, §5.3 Weston patch |
| D2 | Copyright holder | Name the holder in `LICENSE`/`NOTICE` (today a placeholder) | D1; any relicensing |
| D3 | Sign-off policy | Adopt a DCO with human `Signed-off-by` in [`../CONTRIBUTING.md`](../CONTRIBUTING.md); decide who the human author and reviewer of upstream changes is, given that this repository's commits are AI-assisted and Weston's rules (§3.1) bar AI agents from opening issues or merge requests and from adding `Signed-off-by` | every upstream contact |
| D4 | Whether `DriverProfile::Imx95Dpu` is offered at all (open issue WP5-09) | Project-local only; offered as data in an issue; not offered. Its resource-group values are read from source and not measured | §4.1 second row |
| D5 | The advisory-planner-design §6 items that the implementation took, which the owner may revisit (as in open issue WP5-03) | See the list below | what O1/O2 look like when offered |
| D6 | The items left open in advisory-planner-design §6 (as in WP5-03) | See the list below | a Weston hook (§5.4), any engine-side offer |
| D7 | Lead L-1: keep the strict opaque-blend rule? | Keep it (the report proposes no core change); relax it to match libliftoff and the kernel | the L-1 issue text (§4.1) |
| D8 | Whether to rely on zpos restacking on i.MX95 | Wait for a board answer; restrict the profile to the current zpos | any claim based on "Step (f)" (§6.2) |
| D9 | Which target first, and whether to contact either | AGL report on the lease route (has board evidence); libliftoff issue (host data only); Weston RFC (no evidence of benefit) | all of §4 and §5 |

**D5: §6 items taken by the implementation**, which the owner may revisit (as in open issue WP5-03, [`advisory-planner-design.md`](advisory-planner-design.md) §6):

- one contiguous composition buffer (§6.1);
- advise only, no software compositor (§6.2);
- the fewest composed layers, ties to the lowest stacking run, no per-layer priority (§6.3);
- `UnknownBuffer`, `InvalidSource`, `InvalidDestination`, `UnknownOutput` and `TooManyLayers` are whole-call errors; a layer that cannot be clipped becomes `Placement::Compose` with `Blocker::Clip` (§6.4);
- a separate `Blocker` enum, with no new `RejectReasons` or `CE_REJECT_*` bits (§6.5);
- deterministic advice (§6.8);
- no conversion of `Advice` into `Plan` (§6.9);
- buffers need not be `Ready` (§6.10);
- the caller's `&Capabilities` as input (§6.11);
- function `advise` in module `advise`, no separate crate (§6.12);
- ABI versioning through a second check function, `ce_abi_check_advise`, with `ce_abi_check` and `CE_ENGINE_SIZE` unchanged (§6.7);
- a refused frame without a composition target is `Ok`, with every layer composed (§6.13).

**D6: §6 items left open** (as in WP5-03):

- the stateful engine form (`Engine::advise`, `ce_engine_advise`), not decided (§6.6);
- conversion of `Advice` to `Plan`;
- more than one composition buffer (`k` targets or a subset search);
- a smaller capability input form, not decided (§6.11);
- the libliftoff cross-check (WP5-04). The host sweep exists ([`liftoff-crosscheck-report.md`](liftoff-crosscheck-report.md)). A board-backed comparison does not.

## 8. What this proposal does not claim

- No upstream acceptance, and no contact with any upstream. SoW §8 WP5 gives no guarantee.
- No measured advantage over libliftoff or Weston (§6.4).
- Weston's contribution files are quoted at `main` `c8d00e2` only. Tag `16.0.2` and later `main` commits were not compared (§3.1).
- Nothing about the Weston on a board beyond the recorded `12.0.4` version string. Lease support in the image's Weston is open issue WP5-01, and NXP's `weston-imx-12.0.5` changes were not read.
- No coexistence result (WP5-07). No clean-checkout demo (WP5-08).
- Fit statements on licences are inferred, not legal advice.

## 9. Sources

In this repository:

- [`../SoW.md`](../SoW.md) §4.1, §4.2, §8 WP5
- [`wp5-gap-analysis.md`](wp5-gap-analysis.md), [`wp5-evidence-plan.md`](wp5-evidence-plan.md), [`wp5-evidence.md`](wp5-evidence.md)
- [`advisory-planner-design.md`](advisory-planner-design.md)
- [`weston-imx-lease.md`](weston-imx-lease.md), [`weston-release-read.md`](weston-release-read.md)
- [`libliftoff-crosscheck.md`](libliftoff-crosscheck.md), [`liftoff-crosscheck-report.md`](liftoff-crosscheck-report.md)
- [`third-party-licences.md`](third-party-licences.md), [`dlm-aarch64.md`](dlm-aarch64.md)
- [`m1-imx95-report.md`](m1-imx95-report.md), [`host-metrics.md`](host-metrics.md), [`platform-capabilities.md`](platform-capabilities.md)
- [`open-issues.md`](open-issues.md) (WP5-01 to WP5-09)
- [`../NOTICE`](../NOTICE), [`../CONTRIBUTING.md`](../CONTRIBUTING.md), [`../LICENSE`](../LICENSE)

Upstream:

- libliftoff `README.md` at `c4226a79b7a52f59bb51789b1eda112658609125`, read 2026-11-28.
- Weston `CONTRIBUTING.md`, `LLM-USAGE.md` and `AGENTS.md` at `c8d00e2a1dbc98f4a8fb6473a4b485ffce409f3a` (`main` on 2026-11-28), read 2026-11-28.
