# Section 7 frame files

Frame files for the predicted kinds of difference in section 7 of [the cross-check design](../../../../docs/libliftoff-crosscheck.md). Written 2026-11-28. Every file is a self-contained xcheck v1 file (section 5 of the design) with its own synthetic topology.

**Nothing here was run.** Every outcome below is **inferred** from the design, from `planner.rs` as read for the design, and from `profile.rs`. A run may refute any of them. All input is synthetic: no board data, and no result of these files says anything about hardware.

Settings follow design section 6: `structural` and `full` are the model settings; a file with `profile generic` is run with core generic against `structural` and with the file's profile against `full` (for these files the two give the same prediction unless a row says otherwise). Class names follow section 6: `core-only` means core accepts and libliftoff does not; `liftoff-only` the reverse.

## Files

| File | Section 7 row | Topology (origin synthetic) | Frame: predicted class (inferred) |
| --- | --- | --- | --- |
| `zpos-and-intersection.xcheck` | zpos and intersection | 2 fixed-zpos planes: 10 ARGB zpos 0, 11 NV12 zpos 1 | `swapped-disjoint`: liftoff-only; `swapped-overlapping`: both-reject; `ordered-disjoint`: both-accept; `ordered-overlapping`: both-accept |
| `programmable-zpos.xcheck` | programmable zpos | 2 restackable planes 0..1: 20 ARGB now 0, 21 NV12 now 1 | `restack-overlapping`: core-only; `restack-disjoint`: both-accept; `current-order-overlap`: both-accept |
| `invisible-layers.xcheck` | invisible layers | i.MX95 imitation (ids 40..45) | `six-visible-plus-hidden`: liftoff accepts, core unknown (liftoff-only or both-accept); `hidden-only`: core unknown; `hidden-in-middle`: both-accept |
| `format-check-skipped.xcheck` | format check skipped | 2 planes: 30 ARGB, 31 ARGB and NV12 | `yuv-fits-overlay`: both-accept; `two-yuv-layers`: both-reject (liftoff-only if the harness leaves the format check out); `yuv-and-two-argb`: both-reject; `yuv-and-argb`: both-accept |
| `value-checks-before-test-commit.xcheck` | value checks before the test commit | 2 planes: 40 no alpha or blend list, 41 alpha, opaque and premul, rot180 | `half-alpha-one-layer`: both-accept; `half-alpha-two-layers`: both-reject; `coverage-blend`: both-reject (liftoff-only if the fake omits the enum); `rot270-on-overlay`: both-reject (same remark); `rot180-on-overlay`: both-accept; `opaque-on-primary`: both-accept expected, uncertain on the libliftoff side |
| `search-effort.xcheck` | search effort | 10 planes 50..59, one NV12-capable | `ten-argb-layers`: both-accept; `nv12-lowest-nine-argb`: both-accept; `eight-argb-two-nv12-late`: both-reject in outcome, core either NoPlane or core-error(LimitExceeded) (uncertain) |
| `truncated-format-list.xcheck` | truncated format list (in part) | 1 plane with exactly 32 formats | `first-listed-format`: both-accept; `last-listed-format`: both-accept; `cut-off-format`: both-reject |
| `knowledge-gap-scaling-groups.xcheck` | knowledge gap, scaling/groups | i.MX95 imitation, `profile imx95dpu` | S / F / M per frame, see the file header. `one-upscaled`: S liftoff-only (core SCALING), F and M both-accept; `two-upscaled-both`, `downscale-half`: S liftoff-only (core SCALING), F both-reject, M liftoff-only; `h-and-v-separately`: S liftoff-only (core SCALING), F and M both-accept; `five-yuv-layers` (unscaled): S both-accept, F both-reject, M liftoff-only |
| `clipping.xcheck` | clipping | i.MX95 imitation | `inside-touching-edge`: both-accept; `overhang-right-bottom`, `negative-origin`, `fully-outside`: skipped under the v1 restriction |
| `zpos-tie-rule.xcheck` | zpos tie rule | 2 fixed-zpos planes: 70 ARGB zpos 0, 71 NV12 zpos 1 | `tie-argb-then-nv12`: both-accept; `tie-nv12-then-argb`: both-reject; `distinct-z-beats-order`: both-reject; `distinct-z-and-order`: both-accept |
| `format-modifier-absent.xcheck` | format/modifier absent from dump | 2 planes, a modifier on the overlay only | `modifier-not-listed-anywhere`: both-reject; `modifier-only-on-overlay`: both-accept; `two-layers-need-the-modifier`: both-reject; `format-not-listed-anywhere`: both-reject |
| `planes-per-output.xcheck` | planes visible per output | 2 outputs, per-output primaries and a shared overlay | `output-31-two-layers`: both-accept; `output-32-two-layers`: both-accept; `output-32-three-layers`: both-reject; `output-31-one-layer`: both-accept |

## Rows of section 7 not covered by a file

| Row | Reason |
| --- | --- |
| Contract (libliftoff places some layers, core errors) | Not a file of its own. It is a way of counting outcomes ("every layer on a plane"), so every `both-reject` or `core-error` frame here also shows it: libliftoff's partial placement is extra data on those frames. |
| Search determinism (1 ms deadline) | Depends on wall-clock time, not on frame content. A frame file cannot provoke or prevent it; the harness raises the timeout and flags `unstable` itself. `search-effort.xcheck` only makes a long search likely. |
| Knowledge gap, other direction (`liftoff-only` under `model=full`) | It arises only when the model has a rule that core lacks. That is a property of the model, not something a chosen frame can create without knowing the missing rule. Left to the sweep. |
| Cursor planes | Not exercised in v1 (design sections 4 and 7). No cursor plane appears in any file. |
| Composition layer | Not used by the harness, by design. |

Rows covered only in part: truncated format list (a file cannot carry what a dump cut off, and a core-only outcome cannot arise from one file); clipping (v1 restricts frames to the output, so three of four frames are expected to be skipped); value checks (out-of-range alpha values are not expressible, because v1 alpha is 0..65535).

## Caveats for the reader and the sweep

- Every file starts with `xcheck 1` and puts the header comment right after it; the `origin synthetic` line follows the comment. Whether a reader accepts comments before `origin` is for the reader's author to check (the design says `#` comments are allowed).
- The `imx95dpu` files carry `scaling=none max_source=none full_output=0` on each plane, and no `group` lines. The design says a file with `profile imx95dpu` must not also carry explicit constraints. If the reader counts those neutral fields as explicit, they must become optional or be given defaults.
- Two lines of design section 7 disagree with section 6 on which class a "core refuses, libliftoff accepts" case belongs to (`core-only` in the row, `liftoff-only` in section 6). The files use section 6.
- The `knowledge-gap-scaling-groups` row, setting S (core generic against `structural`): the planes carry `scaling=none`, which in core means source and destination sizes must match, so generic core refuses every scaled frame (NoPlane, SCALING) while libliftoff accepts. S therefore shows a liftoff-only difference on the scaled frames, but only because generic core has no scaler, not because of the profile's group or limit knowledge. Setting M (profile core against `structural`) is the one that isolates the profile's scaler groups and limits; the sweep needs to offer it for that. Only unscaled `five-yuv-layers` is both-accept under S.
- Modifiers `0x0100000000000001` and `0x0100000000000002` and the format list of `truncated-format-list.xcheck` are made up for the test and do not describe any hardware.
