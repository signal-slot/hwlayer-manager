# composition-core API notes for the cross-check core side (xc-core)

Dated 2026-11-28. Read from `crates/composition-core/src/**` at commit `d028587` (read-only). Design: `docs/libliftoff-crosscheck.md` (sections 4 to 6). These notes make no hardware claim: the inputs of the cross-check are recorded or synthetic topologies, and the core side only runs the planner on them.

All paths below are relative to `crates/composition-core/src/`. Types are re-exported at the crate root (`lib.rs:24-43`); `plan`, `plan_with` and `BufferLookup` are used through `composition_core::planner`.

## 1. Crate properties that shape the xc-core code

- `#![no_std]`, `#![forbid(unsafe_code)]` (`lib.rs:17-18`), no dependencies. The cross-check crate is a normal `std` crate and may implement core's traits (`BufferLookup`) on its own types.
- All tables are fixed-capacity (`fixed.rs:12`). Limits (`limits.rs`): `MAX_OUTPUTS = 4` (`:7`), `MAX_PLANES = 16` (`:11`), `MAX_LAYERS = 16` (`:13`), `MAX_FORMATS = 32` (`:15`), `MAX_RESOURCE_GROUPS = 8` (`:19`). `FixedVec::push` returns `Err(CapacityError)` when full (`fixed.rs:45`), so the builder must turn that into a refusal, never truncate (design section 5).
- `Capabilities` is `Copy` and about 9 KiB (`backend.rs:56`). Build it in place (`Capabilities::default()`, then `push`).

## 2. Building `Capabilities`

| Item | Where | Notes |
| --- | --- | --- |
| `Capabilities { outputs, planes, resource_groups, replace_pending }` | `caps.rs:162` | `Default` is derived; `Capabilities::EMPTY` `caps.rs:178`; `clear()` `caps.rs:213`; `output_index(OutputId)` `caps.rs:221` |
| `OutputCaps { id: OutputId, size: Size, refresh_mhz: u32 }` | `caps.rs:151` | refresh in **millihertz**, 0 = unknown |
| `PlaneCaps { id, kind, outputs, formats, scaling, max_source, plane_alpha, blend_modes, transforms, zpos, full_output_only }` | `caps.rs:75-99` (fields `:77-98`) | `Default` derived: kind Primary, `zpos` `Fixed(0)` |
| `PlaneKind::{Primary, Overlay, Cursor}` | `caps.rs:12` | |
| `FormatModifier { format, modifier }`, `FormatModifier::linear` | `caps.rs:24`, `:33` | |
| `PixelFormat(u32)`, `PixelFormat::fourcc(&[u8; 4])` | `types.rs:83`, `:87` | little-endian pack, the DRM fourcc value; named constants `types.rs:97-117` |
| `Modifier(u64)`, `Modifier::LINEAR` (0) | `types.rs:143`, `:147` | |
| `ScaleLimits { max_upscale, max_downscale }` (16.16, `0x1_0000` = 1) | `caps.rs:46` | `None` on the plane = sizes must match |
| `max_source: Option<Size>`, `Size::new(w, h)` | `caps.rs:88`, `types.rs:33` | |
| `plane_alpha: bool`, `blend_modes: u8`, `transforms: u8` | `caps.rs:90`, `:92`, `:94` | masks of `BlendMode::mask()` (`types.rs:205`) and `Transform::mask()` (`types.rs:181`); bit = `1 << (variant as u8)` in declaration order |
| `Zpos::Fixed(i32)` / `Zpos::Range { min, max }` | `caps.rs:55` | |
| `ResourceGroup { consumers: LayerTraits, planes: u32, capacity: u8 }` | `caps.rs:139` | `planes` is a mask over **plane indices** of `Capabilities::planes`, not ids |
| `LayerTraits(pub u8)`: `ANY`, `SCALED_H`, `SCALED_V`, `YUV`, `SCALED`, `union`, `intersects` | `caps.rs:103-131` | `consumers=any,scaled_h,...` of the file = `union` of these |
| `FixedVec::new`, `push`, `from_slice`, `as_slice`, `as_mut_slice`, `Deref<Target=[T]>` | `fixed.rs:19`, `:45`, `:36`, `:78`, `:83`, `:99` | no `DerefMut`; mutate through `as_mut_slice()` |

`PlaneCaps::outputs` is a mask over the **index** in `Capabilities::outputs` (`caps.rs:80-82`; used `planner.rs:430-436`), so output lines must be pushed in file order and the file mask used as is.

## 3. Describing a frame

| Item | Where | Notes |
| --- | --- | --- |
| `Frame<'a> { output: OutputId, layers: &'a [Layer] }` | `types.rs:253` | borrowed slice; stacking by `z_order`, not slice order. An empty frame plans to nothing (test `planner.rs:867`) |
| `Layer { buffer, source, destination, z_order, alpha, blend, transform }`, `Layer::new(buffer, src, dst, z)` | `types.rs:215`, `:236` | `new` gives alpha `0xFFFF` (`ALPHA_OPAQUE`, `types.rs:211`), `Opaque`, `Normal`; use struct update for the rest |
| `Rect { x: i32, y: i32, width: u32, height: u32 }`, `Rect::new(x, y, w, h)` | `types.rs:40`, `:53` | argument order matches the file's `x,y,w,h` |
| `BlendMode::{Opaque, Premultiplied, Coverage}` | `types.rs:193` | |
| `Transform::{Normal, Rotate90, Rotate180, Rotate270, FlipH, FlipV}` | `types.rs:163` | |
| `BufferLookup::buffer_info(&self, BufferId) -> Option<BufferInfo>` | `planner.rs:27` | the only way buffers reach the planner. Implement it on a struct over the file's `buffer` lines, as the planner's own test does (`planner.rs:802`) |
| `BufferInfo { size, format, modifier }` | `types.rs:152` | |

## 4. The planner entry points and result

- `plan(&Frame, &Capabilities, &impl BufferLookup) -> Result<Plan, PlanError>`: default `SearchLimits`, backend check that accepts everything (`planner.rs:302`). This is the "static verdict" run of design section 6, step 1.
- `plan_with(&Frame, &Capabilities, &L, SearchLimits, accept: impl FnMut(&Plan) -> Verdict) -> Result<Plan, PlanError>` (`planner.rs:318`). Use this for the run with the test-commit model as backend check. The closure runs once per complete legal assignment and answers `Verdict::{Accept, TryNext, GiveUp}` (`planner.rs:183`). `plan_into` is `pub(crate)` (`planner.rs:333`): not usable.
- The closure sees only `&Plan`, not the frame: capture the frame and the buffer table in it (M12). Count its calls to get the number of backend checks.
- `SearchLimits { steps, backend_checks }`, default 20 000 and 32 (`planner.rs:194`).
- `Plan` (`planner.rs:234`): `output()` `:258`, `assignments() -> &[Assignment]` bottom to top `:265`, `matches(&Frame)` `:270`. Fields are private and there is no public constructor.
- `Assignment { layer, plane, plane_index, zpos, source, destination }` (`planner.rs:212`): `layer` = index in the frame; `plane` = `PlaneId`; `plane_index` = index in `caps.planes`; `zpos` = the value core would program (the plane's own value for `Fixed`); `source` / `destination` = clipped, whole pixels. A layer entirely off the output has no assignment.
- `PlanError` (`planner.rs:131`): `UnknownOutput(OutputId)`, `TooManyLayers{count}`, `UnknownBuffer{layer,buffer}`, `InvalidSource{layer}`, `InvalidDestination{layer}`, `CannotClip{layer}`, `NoPlane{layer, reasons}`, `BackendRejected{candidates}`, `LimitExceeded`, `Aborted`. It is `Copy + PartialEq`.
- `RejectReasons` (`planner.rs:34`): `OUTPUT`, `FORMAT`, `SCALING`, `SOURCE_SIZE`, `ALPHA`, `BLEND`, `TRANSFORM`, `FULL_OUTPUT`, `Z_ORDER`, `RESOURCE`, `IN_USE`; `bits()`, `contains()`, `is_empty()`, and a `Debug` that prints the names (`planner.rs:110`).
- Skipped classes of design section 6 map to `UnknownBuffer`, `InvalidSource`, `InvalidDestination`, `CannotClip` (raised in the per-layer validation, `planner.rs:391-416`, before any plane search), plus `TooManyLayers` (`:345`) and `UnknownOutput`.

### Fallback and composition to the primary plane

There is **none** in the planner. The module doc says: "There is no fallback composition: a frame either maps completely onto hardware planes or is rejected" (`planner.rs:3-5`). There is no composition layer, no "layer goes to the GPU" assignment and no partial plan. Failure is `Err(NoPlane{..})`, `BackendRejected` or `LimitExceeded`. A caller wanting fallback composes itself and plans a smaller frame. Consequences:

- libliftoff's composition layer has no counterpart; design section 4 already excludes it. The rule "libliftoff must leave every layer on a plane" (design section 6, step 3) is exactly core's contract.
- A libliftoff partial allocation has no core-side shape; the "layers placed" count stays extra data.
- `PlaneKind` is not read by `planner.rs` (grep: no use in the file outside tests); only `composition-drm`'s profile code reads it. Core places a single layer on any plane and does not require the bottom layer on the primary plane. libliftoff treats the primary plane specially (design section 4; not re-read here). Record this as a candidate difference class for the triage, not an artefact.

## 5. How the planner decides

Orientation for xc-core and for reading results. The model (xc-model) must be written independently and must not call into these.

- Layers are clipped to the output first; fully off-output layers are dropped; rotations that need clipping give `CannotClip` (`planner.rs:482-545`, used `:401-416`).
- Stacking order: stable insertion sort by `z_order`; ties keep frame order, later on top (`planner.rs:419-428`).
- `static_reasons` per layer and plane (`planner.rs:586-645`): output mask bit (`:590-596`); `(format, modifier)` pair in the plane's list (`:597`); a scaled layer needs `ScaleLimits` within the limits per axis, using the rotation-swapped source (`:604-621`, `axis_within` `:576`, `oriented_source` `:568`); `max_source` compares the clipped source width and height with the limit, unrotated (`:623-627`); `alpha != 0xFFFF` needs `plane_alpha` (`:629`); blend is free when the layer is `Opaque` and the buffer format is known opaque (RGB565, XRGB8888, XBGR8888, known YUV; `types.rs:133`), otherwise the mode bit must be set (`:633-636`); a non-`Normal` transform needs its bit (`:637`); `full_output_only` needs the clipped destination to be exactly the output (`:640`, `Rect::covers` `types.rs:76`).
- `descend` (`planner.rs:717`): depth first from the bottom layer. Per plane: static reasons, plane unused, zpos, group budget. zpos: `Fixed(z)` must exceed the zpos of the layer below; `Range` takes `max(min, below + 1)` if `<= max`; the first layer on a `Range` plane gets `min` (`planner.rs:743-758`). Core never reads a current zpos.
- Resource groups: a layer takes one unit in each group where its plane is a member and `consumers` intersects the layer traits (`planner.rs:685-715`). Traits (`traits_of`, `planner.rs:553`): `ANY` always; `SCALED_H` / `SCALED_V` when the oriented source differs from the destination on that axis; `YUV` when the format is one of six known fourccs (`types.rs:120`: NV12, NV21, NV16, YUYV, UYVY, YU12). A dump with any other YUV fourcc does not consume a `yuv` group in core (M11).
- Early out: more visible layers than planes attachable to the output gives `NoPlane{reasons: IN_USE (| OUTPUT)}` without searching (`planner.rs:452-462`).
- Alpha 0 has no special case: it is `!= ALPHA_OPAQUE`, so it needs `plane_alpha` and is not dropped (`planner.rs:629`). The design (section 7, "Invisible layers") wants such layers removed for the comparison; xc-core must remove them before building the `Frame`, or core and libliftoff see different inputs.
- Cursor planes are ordinary planes to the planner. Design: keep them out of v1.

## 6. Mapping of the xcheck v1 fields

Source: design section 5. "Core field" is the destination in the objects above.

### Header and topology

| xcheck v1 field | Core field | Handling / mismatch |
| --- | --- | --- |
| `xcheck 1` | none | version gate in the reader |
| `origin <class> <date> <text>` | none (by design) | carried by the harness onto every report line |
| `profile <generic\|imx95dpu>` | none in `composition-core` | **M1**: `DriverProfile` is in `composition-drm` (`crates/composition-drm/src/profile.rs:16`, `apply` `:120`), which depends on `drm` and `rustix` (`crates/composition-drm/Cargo.toml`). The design says the core side calls `DriverProfile::Imx95Dpu.apply`, but the crate has a path dependency on `composition-core` only. See M1 below |
| `output <id> <w> <h> <refresh_mhz>` | `OutputCaps { id: OutputId(id), size: Size::new(w,h), refresh_mhz }` | direct; line order = index; more than 4: refuse |
| `plane <id>` | `PlaneCaps::id = PlaneId(id)` | direct; more than 16: refuse |
| `kind=` | `PlaneCaps::kind` | direct, but the planner never reads it (section 4). Cursor out of v1 |
| `outputs=<mask>` | `PlaneCaps::outputs` | direct, same index base; the reader refuses bits at or above the number of output lines |
| `zpos=fixed:N` | `Zpos::Fixed(N)` | direct |
| `zpos=N..M` | `Zpos::Range { min: N, max: M }` | direct |
| `zpos_now=<i32>` | **none** | **M3**: core has no current zpos. Core ignores it; the model and libliftoff use it. A core zpos different from `zpos_now` is the design's "Programmable zpos" row, not an error. For `fixed:N` the current value is N |
| `alpha=<0\|1>` | `PlaneCaps::plane_alpha` | direct |
| `blend=<opaque,premul,coverage\|->` | `PlaneCaps::blend_modes` = union of `BlendMode::mask()`; `-` = 0 | direct bit-wise. **M4**: core makes `Opaque` free for known-opaque buffer formats whatever the plane lists (`planner.rs:633`) |
| `transforms=<normal,rot90,...\|->` | `PlaneCaps::transforms` = union of `Transform::mask()` | direct; core never reads the `normal` bit (`planner.rs:637`), the reader may accept and ignore it |
| `scaling=none` | `scaling = None` | direct |
| `scaling=up:<hex>/down:<hex>` | `Some(ScaleLimits { max_upscale, max_downscale })` | direct, same convention (`caps.rs:46`). **M5**: core uses one pair for both axes (`planner.rs:604-621`), as does the file; per-axis limits cannot be expressed |
| `max_source=<WxH\|none>` | `max_source: Option<Size>` | direct |
| `full_output=<0\|1>` | `full_output_only` | direct |
| `format <fourcc> <modifier>` | `FormatModifier { PixelFormat::fourcc(..), Modifier(hex) }` pushed to `formats` | direct; more than 32 per plane: refuse (the truncation caveat of design section 3 is about the dump). Fourccs with trailing spaces must keep them |
| `group consumers=.. planes=<ids> capacity=<n>` | `ResourceGroup { consumers, planes, capacity }` | **M6**: the file names plane **ids**; core wants a mask over plane **index** (`caps.rs:139`). Convert after all plane lines. Unknown id: refuse. `capacity` must fit `u8`; more than 8 groups: refuse |

### Buffers and frames

| xcheck v1 field | Core field | Handling / mismatch |
| --- | --- | --- |
| `buffer <id> <w> <h> <fourcc> <modifier>` | entry of the `BufferLookup` table: `BufferId(id)`, `BufferInfo { size, format, modifier }` | direct. `MAX_BUFFERS` (`limits.rs:17`) applies to `Presentation`, not to a custom `BufferLookup` |
| `frame <name> output=<id>` | `Frame::output = OutputId(id)`; the name is harness-only (none in core) | direct; unknown output gives `UnknownOutput` (`skipped`) |
| `layer buffer=<id>` | `Layer::buffer` | direct; unknown gives `UnknownBuffer` |
| `src=<x,y,w,h>` | `Layer::source: Rect` | direct, whole pixels (libliftoff gets `<<16`). Negative x/y are `i32` in core and give `InvalidSource` |
| `dst=<x,y,w,h>` | `Layer::destination: Rect` | direct. Clipping per section 5; v1 frames inside the output avoid it |
| `z=<i32>` | `Layer::z_order` | direct; ties by frame order |
| `alpha=<0..65535>` | `Layer::alpha: u16` | direct; alpha 0 per section 5 (M8) |
| `blend=<opaque\|premul\|coverage>` | `Layer::blend` | direct |
| `transform=<normal\|rot90\|rot180\|rot270\|flip_h\|flip_v>` | `Layer::transform` | direct 1:1 |
| `end` | none | |
| more than 16 layers in a frame | none | `TooManyLayers`; the reader may refuse earlier |

Core fields the file does not carry: `Capabilities::replace_pending` (`caps.rs:162` struct; set `false`, not a planning input) and the `Verdict::GiveUp` answer (the model answers `Accept` or `TryNext` only).

## 7. Mismatches and proposed handling

| # | Mismatch | Proposed handling in xc-core |
| --- | --- | --- |
| M1 | `DriverProfile` is in `composition-drm`, not `composition-core` | Preferred: no new dependency; xc-core carries the profile's effect (scaling, `max_source`, groups) as a small data table copied from `profile.rs` at `d028587`, marked as a copy, with a test against explicit lines or a hand-built `Capabilities`. Alternative: path dependency on `composition-drm`, which pulls `drm` 0.15 and `rustix` into the crate's own lockfile; needs the owner's decision. Either way the reader keeps the design's refusal of "profile and explicit lines together" |
| M2 | `DriverProfile::Stm32Ltdc` exists (`profile.rs:115`); the design grammar lists only `generic` and `imx95dpu` | grammar belongs to xc-reader; xc-core uses explicit `scaling` lines for STM32 topologies |
| M3 | `zpos_now` has no core field | ignored by core; class "Programmable zpos" |
| M4 | Opaque blend free for known-opaque formats in core | the model mirrors it explicitly; one sanity frame (ARGB-free XRGB buffer, `opaque`, plane `blend=-`); mind libliftoff's missing-property quirk (design section 4) |
| M5 | one scale pair for both axes | note only |
| M6 | groups by plane id vs plane-index mask | convert after the plane list; refuse unknown ids |
| M7 | no composition/fallback; `PlaneKind` unused by the planner | contract difference, handled by the comparison rule; primary-plane special casing goes to the triage under "contract" |
| M8 | no alpha-0 special case | strip alpha-0 layers on both sides for the "invisible layers" class; compare unstripped separately |
| M9 | limits: 4 outputs, 16 planes, 16 layers, 32 formats, 8 groups, `u8` capacity | the reader refuses files above them |
| M10 | whole-pixel source and destination clipping | v1 frames inside the output with whole-pixel sources |
| M11 | only six fourccs are YUV for core | document the list in the reader; matters only for `group consumers=yuv` |
| M12 | `plan_with` closure sees only `&Plan` | capture frame and buffer table |
| M13 | `PlaneKind` ignored by search | read results of primary-only topologies with that in mind |

## 8. Run recipe for xc-core (prose, no code)

1. Build `Capabilities` from the topology (section 2), with either the profile table or explicit lines.
2. Build the buffer table (`BufferLookup`).
3. Per frame: remove alpha-0 layers if the class asks for it, build the `Layer`s and `Frame`, call `plan` (static verdict), then `plan_with` with a closure that asks the model and returns `Accept` or `TryNext`. Keep the `Result<Plan, PlanError>` of each run and the call count.
4. Read `plan.assignments()` for the model replay of design section 6, step 4.
5. Classes: `Ok` = accept; `NoPlane` = reject; `UnknownBuffer`, `InvalidSource`, `InvalidDestination`, `CannotClip`, `TooManyLayers`, `UnknownOutput` = `skipped`; `BackendRejected`, `LimitExceeded`, `Aborted` = `core-error(..)`.

## core changes that would be needed (report only)

None is required for the first version. Candidates for the core owners, not proposed as code:

- A public place for the driver-profile tables that does not require `composition-drm` (M1).
- Per-axis scale limits, if a real driver needs them (M5).
- A decision on alpha-0 layers: dropped or rejected (M8).
- A way to see which planes were tried for the winning layers, if the triage needs more than `RejectReasons` on failure.
