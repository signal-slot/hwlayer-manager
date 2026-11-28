# WP5-09: keep `DriverProfile::Imx95Dpu` in this project, or offer it

Owner decision brief, written 2026-11-28 from the tree at commit `df507ed`. It recommends and does not decide. Nothing here promises that any upstream project would accept anything (SoW §8 WP5 gives no such promise).

Tags, as in [`platform-capabilities.md`](platform-capabilities.md): **observed** (seen on a board, with a date), **read from source** (a file at a named place, not run), **inferred** (concluded, neither seen nor stated), **not measured**. Every claim below carries one. A tag that cites a file means that file says so; this brief does not re-verify it.

## 1. Question and short answer

Question (open-issues WP5-09, gap W5-09): `DriverProfile::Imx95Dpu` is a per-driver table; KMS has no property for most of what it holds, so there may be no upstream path. Keep it here, or offer it, and in what form?

Short answer (a recommendation, not a decision): **keep it in this project now (option A), and record the data in a form that could be offered later (option B) without writing any libliftoff hook (option C).** Reasons are in section 5. The one thing that would change the recommendation is evidence from the board that does not exist yet (section 6).

## 2. What `DriverProfile` holds, and where each value comes from

**Correction of the brief's pointer.** `DriverProfile` is not in `crates/composition-core`. It is in `crates/composition-drm/src/profile.rs` (read from source 2026-11-28: `pub enum DriverProfile`, `apply(self, caps: &mut Capabilities)`). `composition-core` holds the types it fills in (`PlaneCaps`, `ScaleLimits`, `ResourceGroup`, `LayerTraits` in `crates/composition-core/src/caps.rs`, read from source 2026-11-28). So the profile is a function from "what KMS reported" to "what KMS reported plus driver facts", living in the Linux/DRM backend crate, outside the core.

Variants (read from source 2026-11-28, `profile.rs`): `Generic`, `Imx95Dpu`, `Stm32Ltdc`, `Rk3588Vop2`, `Vc4Hvs5`, `Vc4Hvs6`. The profile is opt-in and chosen explicitly by the caller; it is not detected from the driver name (read from source 2026-11-28; for vc4 the name does not tell HVS5 from HVS6, and `docs/rk3588-vc4-profiles.md` records that the CM4 gave the same counts with both, observed 2026-11-28).

What a profile can set, per `apply` (read from source 2026-11-28): per plane `scaling` (`ScaleLimits`, 16.16 factors), `max_source`, `blend_modes`; and `Capabilities::resource_groups` (`ResourceGroup { consumers: LayerTraits, planes: mask, capacity }`). It never sets formats, zpos, alpha, transforms or plane type; those stay as KMS reported them.

### 2.1 `Generic`

Adds nothing (read from source 2026-11-28; unit test `generic_profile_adds_nothing`).

### 2.2 `Imx95Dpu` (the subject of this brief)

Values as coded (read from source 2026-11-28, `profile.rs` `apply`), with the tag of each as the doc comment and [`platform-capabilities.md`](platform-capabilities.md) (section "i.MX95, Linux, observed on the Verdin iMX95") give it. The board is the Verdin iMX95, BSP 7.8.0, kernel `6.6.157-7.8.0`, SoC revision 2.0.

| Value in the profile | Applied to | Tag | Basis |
| --- | --- | --- | --- |
| `scaling`: `max_upscale` 2048x, `max_downscale` 1x (upscale only) | every non-cursor plane | read from source (kernel 6.6 driver, `imx95-dpu-source-6.6.md`; same in the 6.18 reading); not measured | KMS shows no scaling property: only `SCALING_FILTER` was observed 2026-11-28. Same limits on all planes is read from source |
| `max_source` 8192x8192 | every non-cursor plane | read from source; not measured. It is the limit of an unscaled layer only; the 2048 limit of a scaled layer is not modelled (a scaled source above 2048 passes the planner and `TEST_ONLY` is expected to refuse it) | `platform-capabilities.md` "Differences found" (1) |
| Group `ANY`, capacity 6 over all planes (six fetch units shared by both display streams) | all planes | read from source; not measured | KMS shows six planes per output (observed 2026-11-28); the shared pool is not visible through KMS |
| Group `SCALED` or `YUV`, capacity 4 (the FetchYUV units) | all planes | read from source; not measured. The empty capability of the other two units is inferred from the absence of any assignment | |
| Group `SCALED_H`, capacity 1 (one horizontal scaler) | all planes | read from source; not measured | |
| Group `SCALED_V`, capacity 1 (one vertical scaler) | all planes | rule read from source (vertical scaling refused on revision 1.x); revision 2.0 observed 2026-11-28; "present on this board" is inferred. Unsafe unchanged on a revision 1.x part | |

What the profile assumes about the topology and takes from KMS rather than setting (observed 2026-11-28 on the board, `platform-capabilities.md`): six planes per output (primary plus five overlays), mutable zpos, plane alpha, three blend modes, linear modifier only, no rotation, no cursor plane on this board. Not modelled, left to `TEST_ONLY` (read from source 2026-11-28, doc comment): the 2048 scaled-source limit, even source coordinates for subsampled formats, off-screen destinations, BT.709 full range on YCbCr, the bottom plane forced opaque, and units already bound to the other display stream.

The profile was **checked against the board capture and the driver source on 2026-11-28**, and no value changed except documentation (observed and read from source, `platform-capabilities.md`). That check compared the profile with the KMS capture and the kernel source. A run of the `profile_check` example, which compares the planner with `TEST_ONLY` frame by frame, on the i.MX95 is not recorded anywhere I found: `crates/composition-drm/tests/data/` has `rk3588`, `bcm2711` and `bcm2712` and no i.MX95 directory (read from source 2026-11-28), and `docs/m1-imx95-report.md` calls the resource groups "source-tagged, not measured" (read from source 2026-11-28). So the open-issues wording "until `profile-check` runs on the board" matches the repository: **the `TEST_ONLY` run of `profile_check` on the i.MX95 is not run**. The words "checked (`profile-check`)" in `platform-capabilities.md` mean the capture-and-source comparison, which is easy to misread as the run; this brief does not edit that file (non-goal), so the owner may want the wording clarified.

### 2.3 The other profiles, for comparison

| Profile | What it adds | Tags (from the profile docs, read from source 2026-11-28) |
| --- | --- | --- |
| `Stm32Ltdc` | straight-alpha blend (`Coverage`) on non-cursor planes; scaling (upscale 1080x, no downscale) on the primary and first overlay only | alpha and scaling limits observed 2026-11-27 by `TEST_ONLY`; "layers 0 and 1 scale" inferred from one board; YUV not scalable is read from source and not modelled |
| `Rk3588Vop2` | scaling 8x up and 8x down, `max_source` 4096x4320, at most 4 planes per frame (one group over all planes) | 8x up and 4-plane limit observed 2026-11-27 (`TEST_ONLY`); downscale below 2x not measured; `max_source` read from source and not measured; the 4-plane cause (video-port count) is inferred |
| `Vc4Hvs5`, `Vc4Hvs6` | scaling (32x up, 2x down), `max_source`, LBM group (and UPM group on HVS6); HVS6 drops T-tiled and SAND64 modifiers | scaling ladder observed 2026-11-27 (accepted at the tool's ends, so a lower bound); `max_source` and group capacities read from source or inferred; neither board runs the pinned kernel |

Observation that matters here (inferred): only `Imx95Dpu` and `Vc4*` use shared-resource groups for hardware that is on one chip's shared pools; the scaling and `max_source` fields are what every profile except `Generic` sets. A profile is therefore a small, regular piece of data (a handful of numbers per plane plus up to four groups) wrapped in a Rust function. That is what makes option B (data) possible.

## 3. What could map to generic KMS properties, and what cannot

From [`libliftoff-crosscheck.md`](libliftoff-crosscheck.md) section 4 (field mapping, read from source 2026-11-28 against libliftoff `c4226a7`) and `caps.rs`:

| Capability | Generic KMS carrier | Maps? | Tag |
| --- | --- | --- | --- |
| Plane type (primary/overlay/cursor) | `type` property | yes, direct; the profile does not touch it | read from source 2026-11-28 |
| Formats and modifiers | `IN_FORMATS` blob | yes, direct (the 32-pair cap of `MAX_FORMATS` truncates long lists, observed 2026-11-28 in the RK3588 dumps) | observed 2026-11-27 (truncation, per `libliftoff-crosscheck.md`); read from source (`MAX_FORMATS`) |
| zpos | `zpos` property | yes for the value and the range; libliftoff reads only the current value and never the range | read from source 2026-11-28 |
| Plane alpha, blend mode, rotation | `alpha`, `pixel blend mode`, `rotation` | yes, indirectly. The profile sets only `blend_modes` (STM32 case), where KMS reported a wrong or missing value | read from source 2026-11-28; the STM32 correction observed 2026-11-27 |
| Scaling limits | none | **no**: KMS has no scaling-limit property (only `SCALING_FILTER` was observed on the i.MX95, 2026-11-28) | observed 2026-11-28; read from source |
| `max_source` | none (the kernel answers `-ERANGE`/`EINVAL` at commit) | **no** | read from source 2026-11-28 |
| Resource groups (fetch-unit pool, shared scalers, LBM, UPM, per-CRTC layer budget) | none | **no**: they exist only inside the kernel's atomic check, and libliftoff finds them only through failing test commits | read from source 2026-11-28 (libliftoff `c4226a7`, `alloc.c`) |

Result (inferred): **all the `Imx95Dpu` values are in the "no" rows.** The only generic parts of the i.MX95 picture (six planes, zpos range, alpha, blend modes, formats, no rotation) are already read from KMS and are not in the profile. So no part of the profile "could be replaced by a KMS property", and there is nothing in `Imx95Dpu` that an upstream generic-KMS consumer could read from properties instead. A generic carrier could only exist if the kernel gained properties for scaling and shared-resource limits; that would be a kernel proposal outside the SoW and is not assessed here (not measured, not read).

## 4. Options

Each option says what it needs. None is a promise of acceptance anywhere. Costs are inferred unless a source is named.

### A. Keep in this project

The profile stays in `composition-drm`, selected explicitly, documented with tags. Needs: nothing new. Keeps the existing tests (`profile.rs` unit tests, `profile_check`, the cross-check harness that applies `profile imx95dpu` through `DriverProfile::Imx95Dpu.apply`, read from source 2026-11-28 in `liftoff-crosscheck-report.md`). Cost: a consumer outside this project, for example Weston, never sees the data. Risk (inferred): the table is tied to one kernel (`6.6.157-7.8.0`) and one SoC revision; it must be maintained here by someone with the board.

### B. Offer as data

Publish the numbers (per-plane scaling and `max_source`, the four groups, and the conditions: kernel, SoC revision) as a data file or table with its evidence tags, with no Rust and no `no_std` dependency. A consumer may read it or ignore it. Needs: (1) a data format decided by the owner (not designed; not measured); (2) a statement of which kernel and revision each value was read for, since two of the values (the vertical scaler, the 2048 scaled limit) depend on them (read from source 2026-11-28); (3) the profile-check run (section 6) before values are labelled other than "read from source"; (4) a licence decision (Apache-2.0 vs MIT is an owner item of the proposal, see `docs/wp5-gap-analysis.md` W5-07 and the sibling brief on WP5-03, not decided here); (5) a human who is willing to maintain it. Without such a human the data goes stale (inferred). Who would consume it is **not established**: no source read says libliftoff or Weston has a place for it.

### C. Offer as a libliftoff hook

libliftoff has no per-driver model, and learns limits only by test commits (read from source 2026-11-28, `c4226a7`). A hook would be new libliftoff API, for example a callback or table that tells the allocator a plane's scaling limits or a shared-unit budget, so it can skip test commits that must fail. Needs: an API design in C, agreement from libliftoff's maintainers that a per-driver table belongs in a generic library (not known; not asked), human-authored patches under libliftoff's contribution rules (`libliftoff-crosscheck.md` records that it follows Weston's guidelines, read from source 2026-11-28; the AI-contribution rules are being read by a sibling item and are not repeated here), MIT-compatible code, and a benefit. **The benefit is not measured**: libliftoff's own test commit already refuses these frames, so a hook would at most save test commits (inferred), and no timing or commit-count comparison exists. The hook also needs data per driver, which is option B's data, so C includes B.

### D. Drop

Remove `Imx95Dpu` and rely on `TEST_ONLY` alone. Needs: accepting that the planner cannot refuse over-subscribed scalers or fetch units before the kernel does, so the backend check carries every such refusal. The planner's own contract allows this (the backend check has the last word; read from source 2026-11-28, `caps.rs` doc of `ResourceGroup`). Cost (inferred): more `TEST_ONLY` commits during search on the i.MX95; how many is **not measured**. It would also remove the only i.MX95 content of the cross-check's `full` model (read from source 2026-11-28) and the profile's tests. It reverses work whose point was the shared-resource model for M1; not recommended without a measurement that shows the groups bring no benefit.

## 5. Recommendation

1. **Choose A now.** It is the only option that needs no one else's agreement and no evidence that is missing. It also keeps SoW §4.1's story true: this project's planner models shared units that generic KMS cannot show, and libliftoff learns them only by failing commits (read from source 2026-11-28). It does not say this is an advantage; that is not measured.
2. **Prepare B, do not start C.** Writing the data out with tags costs little and keeps the option open. C depends on libliftoff maintainers' view and on a benefit nobody has measured; starting it first would invert the order the evidence allows.
3. **Do not drop (D)** unless the board run in section 6 shows `TEST_ONLY` alone gives the same planning at acceptable cost.
4. **Do not claim a path to upstream** in the WP5 proposal for this table. Say instead: the table is project-local data with an offered-as-data form, whether anyone wants it is open (inferred, consistent with W5-09 of `wp5-gap-analysis.md`).

What the owner still has to decide: A alone or A plus B; if B, the data format, the licence and who maintains it; whether to ask libliftoff's maintainers anything about C (asking is outward-facing and is the owner's to do).

## 6. Missing evidence

| Missing | Why it matters | Status |
| --- | --- | --- |
| `profile_check --profile imx95` on the Verdin iMX95, with the log kept under `crates/composition-drm/tests/data/` | The groups, the 2048 scaled limit and the vertical scaler are read from source only. The run tells over-strict cases from profile errors (`m1-imx95-test-plan.md` §3 wording, read from source 2026-11-28) | **not run** |
| `TEST_ONLY` refusal of a scaled source above 2048 on the board | Confirms or refutes the unmodelled limit | **not measured** |
| Behaviour with both display streams in use | The groups are modelled for one output; the Verdin has one panel, so units bound to the other stream cannot be seen | **not measured**; blocked by hardware (inferred) |
| A second i.MX95 board or revision 1.x part | The profile is specific to kernel `6.6.157-7.8.0`, revision 2.0; its portability is unknown | **not measured** |
| The kernel 6.18 reading against a 6.18 board | The groups are the same in both readings (read from source 2026-11-28) but no 6.18 run exists | **not measured** |
| Whether libliftoff, Weston or anyone would use the data | Decides whether B or C has a consumer | **not known**; nothing asked |
| A planning-cost comparison with and without the groups | The only basis for D or for a claim of benefit in C | **not measured** |

The cross-check of the profile against libliftoff runs on a synthetic and a recorded stream, not on board `TEST_ONLY`; its report states that no result is a hardware claim (read from source 2026-11-28, `liftoff-crosscheck-report.md`), so it cannot fill these rows.

## 7. What this brief does not claim

- It does not claim the profile is correct for the hardware; its values are read from source and not measured, except the topology items tagged observed above.
- It does not claim a benefit of the profile over `TEST_ONLY` alone or over libliftoff: not measured.
- It does not claim any upstream interest in the data, or acceptance of any contribution.
- It does not decide Apache-2.0 vs MIT, the DCO sign-off, or any `advisory-planner-design.md` §6 item; those are the owner's, in separate briefs.
