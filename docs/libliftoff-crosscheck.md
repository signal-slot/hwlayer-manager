# Cross-check of the planner against libliftoff: design (WP5-04)

**Status: proposal. Nothing here is implemented and nothing was measured.** No harness exists, no frame was run through libliftoff, and no board was used. Written 2026-11-28. Closes nothing in `docs/open-issues.md` by itself; it describes what would close WP5-04 ("No cross-check of the planner against libliftoff on a shared topology").

## 0. What was read, and what was not

| Source | Pin | How it was read |
| --- | --- | --- |
| libliftoff (`https://gitlab.freedesktop.org/emersion/libliftoff.git`, MIT) | `c4226a79b7a52f59bb51789b1eda112658609125` (`c4226a7`), committed 2026-08-24, as pinned in `docs/wp5-gap-analysis.md` section "What was read" | **Not re-read for this note.** The shallow clone into `$TMPDIR` was refused in this session (the shell asked for approval nobody could give), and `gitlab.freedesktop.org` answers automated page fetches with an anti-bot page. Every libliftoff statement below is therefore either quoted from the rows `docs/wp5-gap-analysis.md` section 4 tagged *source (lo@c4226a7)* (read 2026-11-28 there: `alloc.c` whole, `include/libliftoff.h` whole, `README.md`, `doc/compositor.md` first part, `LICENSE`), or marked **recalled** and listed in section 9 as to be checked first. |
| `crates/composition-core/src/{caps,planner,types}.rs` | this repository at `107c5c9` | read 2026-11-28 |
| `crates/composition-drm/src/profile.rs` (`Imx95Dpu`) | same | read 2026-11-28 |
| `docs/observed/*.md` | same | file list read 2026-11-28; the RK3588 topology (`rk3588-topology.md`) read in part. `docs/observed/` is owned by plan #454 and was not changed. |

Tags: **source (this repo)**, **source (lo@c4226a7)** (via the gap analysis), **recalled** (prior knowledge, unchecked), **proposal**, **inferred**. A "dated observation" appears only for what the observed documents record, and they say so themselves.

## 1. Purpose and non-purpose

Purpose: find disagreements between two planners on the same hardware description and the same frames, as leads for a planner bug, a missing constraint, or a documented difference of contract. It supports SoW §4.1's claim that this engine is not a duplicate of libliftoff (WP5-06), but it does **not** measure speed and does not prove either planner correct: both are judged against a model of the kernel (section 6), not the kernel.

Non-purpose: no hardware claim, no M1/M2/M3 evidence, no CI gate, no benchmark.

## 2. What libliftoff needs from KMS

From `docs/wp5-gap-analysis.md` section 4 (source (lo@c4226a7)):

- A DRM fd that it owns, with `DRM_CLIENT_CAP_ATOMIC` on; universal planes and atomic are required (`README.md`).
- Output: a CRTC id. Planes: registered individually or `liftoff_device_register_all_planes`.
- Per layer: arbitrary KMS plane properties by name through `liftoff_layer_set_property`, except `CRTC_ID`; `FB_ID` zero disables the layer.
- Validation: an atomic `TEST_ONLY` commit made at every search node; `EINVAL`, `ERANGE`, `ENOSPC` mean "try another", any other error aborts.
- Before the search it disables all managed planes in the request; it keeps a per-plane FB check (`plane_check_layer_fb`) before each test commit.

Recalled, not re-read (check in section 9): at device and plane setup it calls libdrm to list planes (`drmModeGetPlaneResources`, `drmModeGetPlane`: id, `possible_crtcs`), reads each plane's properties (`drmModeObjectGetProperties`, `drmModeGetProperty`) and uses by name at least `type`, `zpos` (value, range, immutability) and `IN_FORMATS` (a blob of format/modifier pairs, for `plane_check_layer_fb`). It does not itself know scaling limits, rotation support, alpha support, blend modes or shared-unit limits: those reach the kernel through the test commit.

Consequence (inferred): to run libliftoff without a GPU, the harness must present a fake libdrm. The only KMS knowledge libliftoff has of a plane is what that fake exposes as properties, and the only hardware limits it can learn are the ones the fake's test commit enforces.

## 3. What the capability dumps contain

`docs/observed/` holds topology documents for RK3588, BCM2711, BCM2712 and STM32MP257 (and source notes). **It holds no i.MX95 topology yet** (the M1 WP0 capture belongs to plan #453). The RK3588 document is the model: per-plane id, DRM type, `possible_crtcs` as the probe's `outputs` mask, zpos range, alpha flag, blend list, formats with modifiers, and the raw `probe` output. Facts that matter for the harness:

- The `probe` prints at most 32 formats per plane, and `composition-core` stores at most `MAX_FORMATS = 32` per plane (source (this repo): `limits.rs`). Dumps of planes with more format/modifier pairs are **truncated** (observed, per `rk3588-topology.md`: the Cluster planes' lists end mid-modifier-group). Any cross-check over a truncated dump compares against a plane that accepts fewer formats than the real one.
- The probe reports scaling as "not claimed" for DRM planes; KMS has no scaling-limit property. Scaling limits come only from a `DriverProfile`.
- Rotation: the dump says the rotation bitmask itself was not read for RK3588. The core capability `transforms` is therefore absent from some dumps.
- Until a real i.MX95 dump exists, the i.MX95 input is **synthetic**: the six-plane stream of the `profile.rs` unit test (ids 40..45, primary plus five overlays, zpos 0..=5, ARGB8888 and NV12 linear) plus `DriverProfile::Imx95Dpu`. Synthetic input must be labelled so (section 5), and a finding on it is a finding about that description, not about the board.

## 4. Field mapping

`PlaneCaps` and friends from `crates/composition-core/src/caps.rs`; the KMS side is what the fake libdrm would expose.

| Core field | libliftoff / KMS side | Maps? |
| --- | --- | --- |
| `PlaneCaps::id` | plane object id | direct |
| `kind` Primary / Overlay | `type` property (primary 1, overlay 0) | direct |
| `kind` Cursor | `type` = cursor (2). How libliftoff treats a cursor plane in allocation is not re-read | direct in the dump, behaviour **unknown**; keep cursor planes out of the first version |
| `outputs` (mask over output indices) | `possible_crtcs` bit per CRTC | direct; core's bits index `Capabilities::outputs`, KMS bits index CRTCs in device order. The dump must carry both consistently |
| `formats` (format + modifier) | `IN_FORMATS` blob | direct. Truncation caveat of section 3 |
| `zpos` Range{min,max} | mutable `zpos` property with range | direct |
| `zpos` Fixed(n) | immutable `zpos` property | direct in value; libliftoff's handling of an immutable zpos is recalled, not re-read |
| `plane_alpha`, `blend_modes`, `transforms` | properties `alpha`, `pixel blend mode`, `rotation` on the plane | **indirect**: libliftoff never interprets them in the part read; the fake test commit must enforce them from the dump. If the property is absent, setting it fails the commit or the layer cannot use the plane (recalled) |
| `scaling` ScaleLimits (16.16) | no KMS property | **does not map**: only the fake test commit can refuse; libliftoff has no model |
| `max_source` | no KMS property (kernel `-ERANGE`/`EINVAL` at commit) | **does not map**, same |
| `full_output_only` | no property | **does not map**, same |
| `ResourceGroup` (consumers, planes, capacity) | none | **does not map**: shared scalers and fetch-unit pools exist only inside a kernel's atomic check. libliftoff finds them by test commits that fail |
| `replace_pending` | none (a commit/flip concern) | out of scope for planning; ignore |
| `OutputCaps` size, refresh | CRTC mode | direct for size; refresh unused by either planner for assignment |
| `Layer::buffer` → `BufferInfo` size/format/modifier | `FB_ID` plus the framebuffer's format and modifier | direct; the fake needs a framebuffer table (id → size, format, modifier) |
| `Layer::source` (whole pixels) | `SRC_X/Y/W/H` in 16.16 | direct (shift by 16) |
| `Layer::destination` | `CRTC_X/Y/W/H` (signed x/y, unsigned w/h) | direct |
| `Layer::z_order` (ties: later in frame on top) | layer `zpos` property | direct after a rule: harness sets the layer zpos to the rank of the stable sort, so ties resolve the way core resolves them. This is an assumption of the harness (**proposal**) |
| `Layer::alpha` (0..=0xFFFF) | `alpha` | direct |
| `Layer::blend` Opaque / Premultiplied / Coverage | `pixel blend mode` enum `None` / `Pre-multiplied` / `Coverage` | direct; enum values are defined by the fake's property |
| `Layer::transform` (rot90..270, flips) | `rotation` bitmask | direct (DRM bits) |
| Composition layer | `liftoff_layer_set_composition_layer` | not mapped; not set (section 6) |

Where core is richer than KMS (resource groups, scaling, source limit, full-output rule), the fake test commit must be given the same knowledge or the comparison measures "libliftoff does not know" rather than anything about the planners (section 6).

## 5. Input format: `xcheck` v1 (proposal)

One text file describes a topology and any number of frames. Line based, `#` comments, whitespace separated, `key=value` fields; no external parser is needed on either side (core stays dependency free, and the C side stays a few dozen lines of `sscanf`). Ids are decimal; masks and modifiers are `0x` hex; formats are DRM fourcc strings. Rectangles are `x,y,w,h`.

```
xcheck 1
origin <observed|source|synthetic> <YYYY-MM-DD> <free text, no addresses or hostnames>
profile <generic|imx95dpu>                 # which DriverProfile the core side applies
output <id> <w> <h> <refresh_mhz>          # repeated; order = core output index
plane <id> kind=<primary|overlay|cursor> outputs=<mask over output lines>
      zpos=<fixed:N|N..M> alpha=<0|1> blend=<opaque,premul,coverage|->
      transforms=<normal,rot90,rot180,rot270,flip_h,flip_v|->
      scaling=<none|up:<hex16.16>/down:<hex16.16>> max_source=<WxH|none> full_output=<0|1>
  format <fourcc> <modifier>               # one per accepted pair, in dump order
group consumers=<any,scaled_h,scaled_v,yuv>[,...] planes=<plane id>[,...] capacity=<n>
buffer <id> <w> <h> <fourcc> <modifier>
frame <name> output=<id>
  layer buffer=<id> src=<x,y,w,h> dst=<x,y,w,h> z=<i32> alpha=<0..65535>
        blend=<opaque|premul|coverage> transform=<normal|rot90|...>
end
```

(The `plane` and `layer` records are one line each in a file; they are wrapped above for reading.)

Rules:

- `origin` is mandatory. An `observed` file names the observed document it was derived from and that document's date; a `synthetic` file says what it imitates. The harness copies `origin` to every report line, so a result cannot lose its basis.
- Groups name planes by id; the core side converts them to index masks. The Rust reader refuses a file the engine's limits cannot hold (more than `MAX_PLANES`, `MAX_FORMATS`, `MAX_LAYERS` ...) instead of truncating silently.
- `profile imx95dpu` means: the core side calls `DriverProfile::Imx95Dpu.apply`. The file may also carry explicit `scaling`, `max_source` and `group` lines; the harness refuses a file that does both, so the constraints have one origin.
- The test-commit model (section 6) is a function of this same file. No hidden state.
- A converter from the `probe` text output to `xcheck` (without `scaling`, `group`, `max_source`) is the way real dumps enter; the constraints are then added by hand with their source named in `origin`.
- Frames are plain data; large sweeps are generated by the harness from axes (layer count, formats, scaled or not, overlapping or not, z permutation) and written back out as `xcheck` so any finding is replayable from a file.

Not part of the file: file locations of any device node, hostnames, credentials, board addresses (`docs/imx95-board.md` rules). Plane and connector ids are fine; they are in the observed documents already.

## 6. The comparison

For every frame in a file:

1. **Core side:** build `Capabilities` from the file; run `composition_core::planner::plan` with a backend check that accepts (so the result is the planner's own static verdict), and again with the test-commit model as the backend check (so `TEST_ONLY` is modelled the same way). Record `Ok(plan)` with assignments, or the `PlanError`, with `RejectReasons` for `NoPlane`.
2. **libliftoff side:** one device on the fake libdrm built from the same file, one output, one layer per frame layer with the properties of section 4, no composition layer, `timeout_ns` raised so the 1 ms default deadline is not reached (and the harness records whether the deadline was reached). Run `liftoff_output_apply`; call `liftoff_layer_needs_composition` per layer and `liftoff_layer_get_plane`.
3. **Accepted** means: core returns `Ok`; libliftoff leaves **every** layer on a plane. A partial libliftoff allocation counts as "rejected" for this comparison, because the two contracts differ (gap analysis section 4: "best partial allocation" against "all layers on planes, or an error"). The number of layers libliftoff did place is kept as extra data.
4. **Assignment validity, not equality.** Several assignments can be legal. Each side's assignment is replayed through the test-commit model and must pass. Equal assignments are not required and not reported as differences.

**The test-commit model** is the fake kernel: a pure function of the file that accepts a set of plane states or refuses it. It checks, from the file only: the plane can reach the output; the framebuffer's format and modifier are in the plane's list; zpos is within range or equal to the fixed value, and zpos values in use are unique; alpha, blend mode and rotation are among those the plane lists; scaling and source size are within limits; `full_output` planes cover the output; every group's capacity holds. Its rules must be written **independently of the planner's code** (a separate small module in the harness, not calls into `planner.rs`), otherwise agreement is circular. Two model settings:

- `model=structural`: everything above except groups, scaling, source limit, full-output (what generic KMS properties can express).
- `model=full`: everything, including the constraints from `profile` or explicit lines.

Core is run with `profile generic` against `structural` and with the file's profile against `full`. That separates "a planner difference" from "libliftoff cannot know what the properties do not say".

Outcome classes per frame: `both-accept`, `both-reject`, `core-only`, `liftoff-only`, `invalid-assignment(core|liftoff)`, `core-error(<PlanError>)`, `skipped(<reason>)`. Frames core refuses before planning for reasons that are not about planes (`UnknownBuffer`, `InvalidSource`, `InvalidDestination`, `CannotClip`) are `skipped`: the comparison starts from a frame core accepts as well formed.

## 7. Expected kinds of difference

All **inferred**; each is a prediction from the gap analysis and the sources named, to be confirmed or refuted by a run.

| Kind | Direction | Why |
| --- | --- | --- |
| Contract | libliftoff places some layers, core errors | Partial allocation against all-or-nothing (gap analysis section 4). Handled by the "every layer on a plane" rule; reported, not a bug |
| zpos and intersection | `liftoff-only` | libliftoff preserves relative zpos only between layers that intersect (`layer_intersects`); core's `Z_ORDER` constraint covers all layers placed. Frames of non-overlapping layers that need out-of-order planes are accepted by libliftoff and refused by core. Possible improvement for core, not verified against `planner.rs` beyond its reject-reason names |
| Search effort | `liftoff-only` or `core-error(LimitExceeded / BackendRejected)` | Core is bounded by `SearchLimits` (20 000 steps, 32 backend checks); libliftoff by a deadline. Rare on 6 or 8 planes; shows up with many layers on many planes |
| Search determinism | libliftoff flips between runs | 1 ms wall-clock deadline (`DEFAULT_ALLOC_TIMEOUT_NSEC`). Mitigated by a long timeout; any remaining flip is reported as `unstable` |
| Truncated format list | `core-only` or `both-reject` | Dumps cut at 32 formats per plane (section 3); applies equally to both sides but makes the dump differ from the hardware |
| Knowledge gap, scaling/groups | `core-only` rejects, libliftoff accepts, under `model=structural` | Core with `profile` knows limits libliftoff cannot learn from properties. Expected: under `model=full` both should refuse, and a remaining difference is a lead |
| Knowledge gap, other direction | `liftoff-only` under `model=full` | If libliftoff refuses what core accepts, the fake test commit refused it: the model has a rule core lacks (or the planner has a missing constraint). A lead |
| Clipping | core accepts, libliftoff sees different coordinates | Core clips a layer reaching beyond the output and programs the visible part (`Assignment::source/destination`); libliftoff is given the original coordinates unless the harness feeds it the clipped ones. The harness should feed libliftoff the *unclipped* layer and let the model clip, or restrict first versions to frames inside the output (**proposal**: restrict) |
| zpos tie rule | either | Core: equal `z_order` stacks in frame order. Harness maps to distinct zpos ranks; if the mapping were wrong the differences would be artefacts. A first sanity test on tie frames |
| Cursor planes | undefined | Not exercised in v1 |
| Composition layer | n/a | Not used; libliftoff's central concept is excluded by design |
| Format/modifier absent from dump | both reject | Same input, same result |
| Planes visible per output | either | One output per run in v1; both planners see all planes whose mask includes it |

Differences found are triaged as *expected* (a row above), *artefact* (harness bug, fixed), or *lead* (written up with the frame file, never summarised without it).

## 8. Where the harness would live (proposal; nothing is changed now)

Constraints: `composition-core` stays dependency free and `#![forbid(unsafe_code)]`; CI (`docs/ci.md`: fmt, clippy, tests, the two `thumbv7em-none-eabihf` builds, the aarch64 cross-build) stays as it is; this note changes no `Cargo.toml`.

- **A new directory outside the workspace**, `tools/liftoff-crosscheck/`, with its own `Cargo.toml` that carries an empty `[workspace]` table (so Cargo does not look for the root manifest) and a path dependency on `composition-core`. The root `Cargo.toml` lists its members explicitly, so `cargo fmt --all`, `cargo clippy --workspace`, `cargo test --workspace`, and the cross-builds never see the new package, and `Cargo.lock` does not change. (When the project later wants it inside the workspace, that is one line in the root manifest and a decision for that day.)
- **Rust part:** the `xcheck` reader, the core-side run, the test-commit model, the comparison and the report. Safe Rust only; it needs no FFI. It runs the C part as a child process and reads its output.
- **C part:** a small program, one `.c` file plus a fake-libdrm file, that reads the same `xcheck` file, builds the libdrm functions from section 2 on top of the file's data (with `drmModeAtomicCommit` answering `TEST_ONLY` from the model), links libliftoff and prints one result line per frame. The model must be the same rules as the Rust model; the cleanest is for the Rust side to answer the test commits, over a pipe, from its own model, so the rules exist once. **Open design choice**; the alternative is writing the model twice and cross-testing the two copies, which costs more and gives a second, independent check.
- **libliftoff source:** not vendored and not a submodule. The harness's script takes the path of a checkout at the pinned commit (`c4226a7`) and builds it with `cc`/`meson` into the scratch directory; it refuses another commit unless told. This avoids shipping MIT source in this Apache-2.0 repository (if ever vendored, `NOTICE` and the MIT text would be needed; see WP5-05). libdrm is needed for headers only if the fake provides the symbols itself (**recalled**: libliftoff depends on libdrm's atomic API and property structs).
- **Not in CI.** The harness runs by hand or by a script (`scripts/` entry later), needs a C compiler and a libliftoff checkout, and prints its report as text that may be committed under `docs/` with a date. Its rules about hardware claims are those of the repository: output is labelled with the `origin` of its input, and a synthetic topology produces no hardware statement.
- **Repository rules honoured:** no board address, hostname or credential in input files or reports; the harness never touches a device.

## 9. Work that follows, and what to check first

Checks to make when a libliftoff checkout at `c4226a7` is available (the first step of any implementation):

1. Which libdrm functions libliftoff calls at device and plane setup, and which property names it reads (section 2, "recalled").
2. Whether libliftoff's own test suite already contains a libdrm mock that can be reused or copied as a pattern (recalled: it does, with a layer-and-plane builder for tests; not read). If so, the fake libdrm of section 8 is smaller than described here.
3. How `zpos`, `alpha`, `rotation` and immutable properties are treated in `plane.c` and `alloc.c` (section 4, `unknown` and `recalled` rows).
4. How a cursor plane and a plane without `zpos` are treated.
5. The behaviour of `timeout_ns` and the return of `liftoff_output_apply` when no composition layer is set and a layer cannot be placed.

Order of work (proposal): (a) `xcheck` reader and converter from probe text, with unit tests against the committed observed documents; (b) the model and the core side alone, with its own tests; (c) the fake libdrm and the C part; (d) a sweep on the synthetic i.MX95 stream and on the RK3588 topology; (e) the report, with each difference triaged as in section 7; (f) when the M1 WP0 i.MX95 dump lands, a run with `origin observed`.

Open decisions for the project owner: whether the Rust side answers the test commits (one model) or the model is written twice; whether a harness may later join the workspace; whether libliftoff may ever be vendored.

Non-claims: no result of this design is known. Whether the two planners agree, how often, and on which frames is **not measured**.
