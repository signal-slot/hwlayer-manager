# composition-core behaviour on the open questions of the cross-check design

Dated 2026-11-28. Read at commit `eab4162`, read only, nothing run:
`crates/composition-core/src/{planner,caps,types,limits,engine}.rs` and
`crates/composition-drm/src/profile.rs`. Design:
[libliftoff-crosscheck.md](../../../docs/libliftoff-crosscheck.md) (sections 4, 6, 7).

Tags: **source** = read from this repository at `eab4162`; **inferred** = concluded from the
source, not run. Line numbers are of `eab4162`; `planner.rs` (in `crates/composition-core/src/`)
is meant when no file is named. Core is not changed by this note.

## Summary against section 7

| Section 7 row | Verdict |
| --- | --- |
| zpos and intersection | **confirmed** (Q2), sharper: strict, on consecutive layers in stacking order, over every placed layer |
| Programmable zpos | confirmed, with a detail (Q7) |
| Invisible layers (alpha 0) | **refuted for core's side**: core does not drop alpha 0 (Q1) |
| Clipping | confirmed and refined (Q3) |
| zpos tie rule | confirmed (Q4) |
| Search effort | refined: a different error than predicted when backend checks run out (Q5) |
| `skipped` pre-planning errors (section 6) | confirmed, with an order (Q6) |
| Knowledge gap rows | refined by what `Imx95Dpu.apply` adds (Q8) |

## Q1. A layer with alpha 0

- **source**: alpha has one use in the planner: `static_reasons` sets `ALPHA` when
  `layer.alpha != ALPHA_OPAQUE && !plane.plane_alpha` (`planner.rs:629-631`). 0 is not special;
  it is treated like 0x8000. The layer is not removed: it counts toward `count` and
  `attachable` (`planner.rs:411-413`, `453`) and takes a plane, a zpos slot and resource-group
  units. It is part of `fingerprint` (`planner.rs:296`). `engine.rs` and `presentation.rs`
  do not use alpha at all.
- **source**: `Layer::alpha` is documented as "constant alpha for the whole layer"
  (`types.rs:225`); nothing says 0 means hidden.
- **Implication**: refutes the open half of the "Invisible layers" row. libliftoff ignores an
  alpha-0 layer (`layer_is_visible`); core places it. So a frame with an alpha-0 layer can be
  `NoPlane` in core where libliftoff has a free plane, or core can demand `plane_alpha` that
  libliftoff never asks for. The design's rule to compare with those layers removed, as a
  separate class, is right; also record `ALPHA` rejections on planes without `plane_alpha`.
  Lead: an alpha-0 layer costs a plane in core.

## Q2. Z_ORDER: all placed layers or only intersecting ones

- **source**: all placed layers; geometry is never consulted. `descend` carries `below`, the
  zpos given to the previous layer in stacking order (`planner.rs:717`, `777`). A plane is
  refused with `Z_ORDER` unless its zpos is strictly above `below`: `Fixed(z)` needs `z > b`
  (`planner.rs:748`); `Range` takes `max(min, b+1)` and needs `<= max` (`planner.rs:750-753`);
  the first layer gets the `Fixed` value or `min` (`planner.rs:747`, `749`). Layers are sorted
  by `z_order` only (`planner.rs:419-428`).
- **source**: strict, so two layers never share a zpos even on disjoint areas. Planes with
  equal `Fixed` zpos (the default is `Fixed(0)`, `caps.rs:67-71`) can never both be used in
  one frame.
- **source**: off-output layers are left out before sorting (`planner.rs:404-414`), so they
  take no part in the zpos chain.
- **Implication**: confirms the "zpos and intersection" row. Disjoint layers that need
  out-of-order planes are refused by core with `NoPlane` (reasons include `Z_ORDER`) and may be
  accepted by libliftoff: direction `liftoff-only`.
- **Lead** (inferred): the rule is stricter than the geometry needs; core's docs do not say
  whether that is intended. The frame generator should include disjoint-layer frames.

## Q3. A layer reaching beyond the output

- **source**: `clip_to_output` (`planner.rs:482-546`). Entirely off the output: no plane, no
  assignment (`planner.rs:492-494`, `414`; doc `planner.rs:8-11`, `264`). Entirely inside:
  unchanged (`planner.rs:495-497`).
- **source**: partly outside: destination clamped (`planner.rs:538-544`) and source cut in
  proportion per axis (`planner.rs:508-520`). Exact only: if `near*length % shown` or
  `kept*length % shown` is non-zero the result is `PlanError::CannotClip`
  (`planner.rs:513-515`). `FlipH`/`FlipV` swap which source edge is cut (`planner.rs:498-501`,
  `511`); rotations return `CannotClip` whenever clipping is needed (`planner.rs:505`) but are
  fine inside the output.
- **source**: every plane check (`static_reasons`: scaling, `max_source`, `FULL_OUTPUT`) and
  `traits_of` use the *clipped* layer (`planner.rs:406-410`, `438-440`, `449`). A layer one
  pixel past the edge therefore no longer covers the output and fails `full_output_only`
  planes (`planner.rs:640`); scale factors can change by clipping.
- **source**: the `Assignment` carries the clipped `source` and `destination`
  (`planner.rs:222-226`, `773-774`).
- **Implication**: confirms the row. Restricting v1 frames to the inside of the output (the
  design's proposal) is safe and avoids `CannotClip`. If clipped frames are added, feed
  libliftoff the clipped rectangles and treat an off-output layer as absent on the libliftoff
  side.
- **Lead** (inferred): off-output layers are still validated in full (Q6), so one with an
  invalid source is an error although it needs no plane.

## Q4. Equal `z_order` ties

- **source**: stable insertion sort bottom to top with strict `>` (`planner.rs:422-428`);
  equal values keep frame order, later on top. Documented at `types.rs:222-223`, tested by
  `equal_z_order_stacks_in_frame_order` (`planner.rs:920`).
- **Implication**: confirms the tie row. The harness rule "layer zpos = rank of the stable
  sort" matches, with the rank taken over *placed* layers only (Q3).

## Q5. `SearchLimits` and the error a frame gets

- **source**: `SearchLimits { steps: 20_000, backend_checks: 32 }` by default
  (`planner.rs:192-208`). `plan`/`plan_with` take it as an argument (`planner.rs:302-324`), so
  the harness can set it; `Engine::set_search_limits` is at `engine.rs:157`.
- **source**: `steps` counts every (layer, plane) attempt, including ones rejected statically
  (`planner.rs:729-733`). At the limit the search returns `PlanError::LimitExceeded`
  (`planner.rs:466`). There is no pruning, so the worst case grows exponentially with layers.
  The test is at the top of each iteration, so a frame that needs exactly `steps` attempts
  passes.
- **source**: `backend_checks` counts calls of the accept closure, one per complete
  assignment (`planner.rs:718-723`). If it is already at the limit when another complete
  assignment is found, the result is `LimitExceeded`, **not** `BackendRejected`
  (`planner.rs:719-721`). `BackendRejected { candidates }` arises only when the search
  finishes with `backend_checks > 0` (`planner.rs:468-470`), including when other branches
  failed statically.
- **source**: with the always-accepting check of `plan` (`planner.rs:307`) only `steps` can
  trigger `LimitExceeded`. `Verdict::GiveUp` gives `PlanError::Aborted` (`planner.rs:467`,
  `726`); `Engine` turns it into the backend's own error (`engine.rs:265`).
- **source**: `NoPlane { layer, reasons }`: `layer` is the frame index of the layer at the
  deepest depth reached, `reasons` the union over all planes tried for that layer at that
  depth only (`planner.rs:675-682`, `471-474`).
- **source**: more placed layers than attachable planes returns at once, with `IN_USE`
  (plus `OUTPUT` if some planes cannot reach the output), without search
  (`planner.rs:453-462`). `attachable` counts planes with the output bit and nothing else.
- **Implication**: refines "Search effort". Both bounds are counts, not time, so core is
  deterministic; the sweep should record `LimitExceeded` as `core-error`. When the model
  refuses every candidate expect `BackendRejected` for few candidates and `LimitExceeded` once
  32 have been tried, not always `BackendRejected`. Lead (inferred): the 32-candidate bound
  matters only for frames with many equivalent planes.

## Q6. Errors before planning

- **source**, in this order (`planner.rs:341-417`): `UnknownOutput` (`341-343`);
  `TooManyLayers` above `MAX_LAYERS` = 16 (`345-349`, `limits.rs`); then per layer in frame
  order `UnknownBuffer` (`391-396`), `InvalidSource` when the source is empty or not
  `fits_within` the buffer (`397-399`, `types.rs:68`), `InvalidDestination` when empty
  (`400-402`), `CannotClip` (`415`). The first failing layer wins; a later layer's error is
  hidden by an earlier one's.
- **source**: these run on every layer including those off the output. Alpha, blend and
  transform are not checked here. Format and modifier are checked per plane as `FORMAT`
  (`planner.rs:597-603`), so a buffer no plane lists is a `NoPlane`, not an early error.
- **source**: all are `PlanError` variants; `Engine` wraps them as `Error::Plan`
  (`engine.rs:19`, `30-33`).
- **Implication**: confirms the `skipped` class. The sweep should skip with the variant name
  for `UnknownOutput`, `TooManyLayers`, `UnknownBuffer`, `InvalidSource`,
  `InvalidDestination`, `CannotClip`. `TooManyLayers` is a core capacity, not a hardware fact:
  frames over 16 layers are `skipped`, not `liftoff-only`.

## Q7. Outputs mask, `Fixed` vs `Range` zpos, `full_output_only`

- **source**, outputs: `PlaneCaps::outputs` is a bit mask over indices of
  `Capabilities::outputs` (`caps.rs:80-82`); `static_reasons` adds `OUTPUT` when the bit of
  the frame's output index is clear (`planner.rs:594-596`). Planes are tried in array order
  (`planner.rs:729`), so the first legal assignment depends on file order (inferred; harmless,
  the comparison is by validity).
- **source**, zpos: Q2. For `Range` core picks the *lowest* legal zpos (`planner.rs:744-751`),
  not a current value (core has none). For `Fixed` the plane's own value is the
  `Assignment::zpos` (`planner.rs:219-221`). Only zpos values must increase with stacking,
  not the order of planes in the file.
- **source**, `full_output_only`: `FULL_OUTPUT` unless the clipped destination is exactly
  `0,0 .. output size` (`planner.rs:640-642`, `types.rs:76`). The source is not looked at:
  a sub-region scaled to the full output passes. No exception for a sole layer.
- **source**, the other static rules the model must copy independently: format and modifier
  both listed (`planner.rs:597-603`); scaling per axis in 16.16, upscale limit when
  `destination > source`, else downscale limit, source axes swapped for 90/270
  (`planner.rs:568-584`); `scaling: None` refuses any scaled layer (`planner.rs:604-622`);
  `max_source` compares source width/height unrotated (`planner.rs:623-628`); blend: `Opaque`
  on a known-opaque format needs no mode bit (`planner.rs:632-636`, `types.rs:133`), an
  unknown format counts as possibly carrying alpha; transforms other than `Normal` need their
  mask bit (`planner.rs:637-639`).
- **Implication**: the model must reproduce the opaque-format blend exemption, `full_output`
  judged after clipping and the strict zpos increase, or the "knowledge gap" rows turn into
  artefacts. "Programmable zpos" confirmed: on `Range` planes core fails with `Z_ORDER` only
  when the range is exhausted.
- **Lead** (inferred): `Zpos::Range` with `max < min` is not rejected anywhere; every plane
  then fails `Z_ORDER`.
- **Lead** (inferred): `PlaneKind::Cursor` has no special handling in the planner; a cursor
  plane takes layers like an overlay. Consistent with keeping cursors out of v1.

## Q8. What `DriverProfile::Imx95Dpu.apply` adds

- **source** (`crates/composition-drm/src/profile.rs:122-153`): on every non-cursor plane,
  `scaling = Some { max_upscale: 2048 << 16, max_downscale: 1 << 16 }` and
  `max_source = Some(8192 x 8192)` (`127-134`), overwriting what was there. Four resource
  groups over **all** planes, cursor included (mask `all`, `126`, `136-147`): `ANY` capacity 6;
  `SCALED | YUV` 4; `SCALED_H` 1; `SCALED_V` 1. A group already present is not added again
  (`148-151`). Nothing about formats, zpos, alpha, blend, transform or output masks.
- **source**: `Generic` changes nothing (`121`, test `344-348`). `Stm32Ltdc` (`154-173`):
  32x upscale, no downscale on the primary and the first overlay only; no groups, no
  `max_source`.
- **source**: a layer takes a group unit when its traits intersect `consumers`
  (`caps.rs:122-124`, `planner.rs:702`). On six planes the `ANY` group of 6 limits nothing
  (inferred; it matters with more planes or with units shared with the other stream, which
  the planner cannot see, `profile.rs:58-61`). Two layers scaled on the same axis are refused
  with `RESOURCE` (`planner.rs:760-763`).
- **source**, not modelled (`profile.rs:46-61`): scaled sources above 2048, even source
  coordinates for subsampled formats, off-screen destinations, units used by the other
  stream. `model=full` for i.MX95 must take these from explicit lines or leave them to
  `TEST_ONLY`; otherwise a core-accepts difference is a knowledge gap, not a core bug.
- **Implication**: refines both "Knowledge gap" rows. An xcheck file with `profile imx95`
  should produce the same planes and groups as this function. `all` is built from the plane
  count at the time of the call (`126`), so call `apply` after all planes are pushed.
- **Lead** (inferred): `1u64 << planes.len()` is safe up to `MAX_PLANES` = 16. No bug.

## Leads for the owner (none fixed)

1. Alpha 0 is an ordinary alpha and costs a plane (Q1).
2. Z_ORDER is stricter than geometry needs and is not documented as intended (Q2).
3. `LimitExceeded`, not `BackendRejected`, is returned when a 33rd candidate is reached (Q5).
4. Planes with equal `Fixed` zpos, the default included, can never be used together (Q2).
5. `Zpos::Range` with `max < min` is not rejected (Q7).

## Not determined

- Whether `Engine` adds errors before `plan_into` for a frame: only `validate`,
  `validate_into`, `plan_frame` and `set_search_limits` were read, not the `commit` path.
- Behaviour at the extremes of `i32` coordinates: `clip_to_output` computes in `i64`
  (`planner.rs:484-487`) so it cannot overflow there; not run.
