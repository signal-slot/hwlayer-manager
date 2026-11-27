# Cross-check of the planner against libliftoff: report of 2026-11-28 (WP5-04)

Step (d), the sweep, and step (e), this report, of [the cross-check design](libliftoff-crosscheck.md) (section 9). The harness is [`tools/liftoff-crosscheck/`](../tools/liftoff-crosscheck/README.md); the replayable frame files of the findings are in [`tools/liftoff-crosscheck/findings/`](../tools/liftoff-crosscheck/findings/README.md).

## What this is, and what it is not

- **No hardware claim.** Every input is a recorded probe dump, a hand-written topology or a synthetic stream. A result says how the two planners treat that input under the harness's test-commit model (the fake kernel). It says nothing about what any board or kernel accepts.
- **Nothing ran on a board.** Everything ran on one Linux build host, with libliftoff linked against a fake libdrm whose `TEST_ONLY` commits are answered by the Rust model.
- **Not in CI.** The harness is outside the root workspace and runs by hand.
- **Counts toward no milestone.** These runs are not evidence for M1, M2 or M3.
- Step (f), the run on the observed i.MX95 dump (`topologies/imx95-observed.xcheck`), is not part of this report.

Tags, as in [`platform-capabilities.md`](platform-capabilities.md): **observed** (from a board, with its date) is used here only for where an input came from: the probe dumps were observed on 2026-11-28, and nothing in this report observed a board. **source** means read from libliftoff `c4226a7`, from `crates/composition-core` or from the harness. **inferred** means a conclusion drawn from those. The outcomes in the tables are results of the host run of 2026-11-28 (harness output). They are not KMS observations, so they carry none of the board tags.

## Versions

| Item | Value |
| --- | --- |
| Date | 2026-11-28 |
| Repository | commit `7d0481f731925b0c424a0ebe7aabd46c0c2fd569` (`7d0481f`), plus this item's harness fix (section "Artefact fixed") |
| libliftoff | `c4226a79b7a52f59bb51789b1eda112658609125` (`c4226a7`), fetched from freedesktop GitLab into `$CONNECTED_DEPS_DIR/libliftoff`, built in `$TMPDIR`, not vendored. `build-liftoff.sh` checks the commit |
| libdrm | headers only, system libdrm 2.4.134; the symbols come from the harness's fake libdrm (`c/fake_drm.c`) |
| Toolchain | rustc 1.99.0, GCC 16.2.1, meson/ninja for libliftoff (`debugoptimized`, static) |
| Generator | `GEN_VERSION=1`, seed `0x5eed11f70ffc4226`, 200 random samples, every layer inside the output (`edge=off`). Each frame's case seed is derived from the seed, the input's file name, the family and the index, so a renamed input gets different random frames |
| libliftoff timeout | `INT64_MAX` (exhaustive search); every frame on a fresh device |
| Model settings | both: `model=structural` (core `profile generic` with the accept-all check) and `model=full` (core with the file's profile and the model as backend check) |

## Commands

From the repository root:

```sh
bash tools/liftoff-crosscheck/build-liftoff.sh        # last line printed = driver path
cd tools/liftoff-crosscheck
cargo run --release -- convert fixtures/rk3588.probe.txt      > "$TMPDIR/rk3588-probe.xcheck"
cargo run --release -- convert fixtures/stm32mp257.probe.txt  > "$TMPDIR/stm32mp257-probe.xcheck"
cargo run --release -- convert fixtures/bcm2711-cm4.probe.txt # refused, see below
cargo run --release -- convert fixtures/bcm2712.probe.txt     # refused, see below
for f in fixtures/imx95-synthetic.xcheck "$TMPDIR/rk3588-probe.xcheck" \
         "$TMPDIR/stm32mp257-probe.xcheck" topologies/rk3588.xcheck topologies/stm32mp257.xcheck; do
    cargo run --release -- sweep "$f" --driver "$DRIVER" --out "$TMPDIR/sweep"
done
for f in frames/section7/*.xcheck calibration/*.xcheck findings/*.xcheck; do
    cargo run --release -- run "$f" --driver "$DRIVER"
done
```

The converted files must keep the names `rk3588-probe.xcheck` and `stm32mp257-probe.xcheck` to reproduce the random frames, since the case seed depends on the file name. The sweep used the defaults `--samples 200` and `--seed 0x5eed11f70ffc4226`.

## Inputs and their origin

| Input | Origin label (as printed on every result line) | Swept |
| --- | --- | --- |
| `fixtures/imx95-synthetic.xcheck`, `profile imx95dpu` | **synthetic** 2026-11-28: imitates the plane stream of the `DriverProfile::Imx95Dpu` unit test in `crates/composition-drm/src/profile.rs` (commit `99d965d`). Describes no board | yes |
| RK3588: `convert fixtures/rk3588.probe.txt` | **observed** 2026-11-28: probe text of `docs/observed/rk3588-topology.md` at commit `db71077`, recorded. Format lists are cut at 32 by the probe; the converter flags planes 33, 39, 45, 51 as truncated. `profile generic`, no constraints ("scaling not claimed" becomes no scaler) | yes |
| STM32MP257: `convert fixtures/stm32mp257.probe.txt` | **observed** 2026-11-28: probe text of `docs/observed/stm32mp257-topology.md` at commit `20df182`, recorded. `profile generic`, no constraints | yes |
| BCM2711 (CM4): `convert fixtures/bcm2711-cm4.probe.txt` | **observed** 2026-11-28, `docs/observed/bcm2711-cm4-topology.md` at `472cc75` | **no**: refused by the converter, "planes [48, 67, ...] have no per-plane format list; not inventing one" (the document folds the per-plane `formats` lines) |
| BCM2712: `convert fixtures/bcm2712.probe.txt` | **observed** 2026-11-28, `docs/observed/bcm2712-topology.md` at `5d30922` | **no**: refused for the same reason |
| `topologies/rk3588.xcheck` (hand-written, supplementary) | **observed** 2026-11-28: `rk3588-topology.md` at commit `eab4162`; constraints (scaling, `max_source`) **source**, from `docs/rk3588-backend-design.md` | yes |
| `topologies/stm32mp257.xcheck` (hand-written, supplementary) | **observed** 2026-11-28: `stm32mp257-topology.md` at commit `eab4162`; constraints from the `stm32mp257-ltdc-*.md` documents (tags per line in the file) | yes |
| `frames/section7/*.xcheck`, `calibration/*.xcheck` | **synthetic** 2026-11-28, hand-made topologies and frames | `run` only (their own frames) |

The hand-written BCM topologies (`topologies/bcm2711.xcheck`, `topologies/bcm2712.xcheck`) were tried as supplementary inputs and could not be swept: see "Harness limits found, not fixed".

## Summary per input and model setting

Frames per outcome class, final runs (after the fix). `alpha0` frames are compared without their alpha-0 layers and are included in the counts; none of them differs.

| Input (origin) | Setting | both-accept | both-reject | core-only | liftoff-only | other | total |
| --- | --- | --- | --- | --- | --- | --- | --- |
| imx95-synthetic (synthetic) | structural | 215 | 145 | 0 | 72 | 0 | 432 |
| imx95-synthetic (synthetic) | full | 236 | 195 | 1 | 0 | 0 | 432 |
| rk3588-probe (observed 2026-11-28, recorded) | structural | 176 | 353 | 1 | 40 | 0 | 570 |
| rk3588-probe (observed 2026-11-28, recorded) | full | 176 | 393 | 1 | 0 | 0 | 570 |
| stm32mp257-probe (observed 2026-11-28, recorded) | structural | 147 | 172 | 1 | 58 | 0 | 378 |
| stm32mp257-probe (observed 2026-11-28, recorded) | full | 147 | 230 | 1 | 0 | 0 | 378 |
| rk3588 hand-written (observed 2026-11-28 + source) | structural | 209 | 346 | 8 | 7 | 0 | 570 |
| rk3588 hand-written (observed 2026-11-28 + source) | full | 209 | 353 | 8 | 0 | 0 | 570 |
| stm32mp257 hand-written (observed 2026-11-28 + source) | structural | 146 | 197 | 11 | 22 | 0 | 376 |
| stm32mp257 hand-written (observed 2026-11-28 + source) | full | 146 | 219 | 11 | 0 | 0 | 376 |

No frame of any run was `unstable`, `skipped`, `invalid-assignment(...)` or `core-error(...)` after the fix, and no libliftoff run reached its deadline. Before the fix, the hand-written STM32MP257 topology had 13 frames `invalid-assignment(core)` under `structural` (both-accept 133) and the same 13 `core-error(BackendRejected)` under `full`. Under `full`, no input has a `liftoff-only` frame.

By family, the differences sit in `rand` and `scale` only; `fill`, `zperm`, `fmt`, `tie` and `alpha0` agree everywhere:

| Input | structural: liftoff-only (rand / scale) | core-only (rand / file), both settings |
| --- | --- | --- |
| imx95-synthetic | 60 / 12 | 0 / 1 (`full` only) |
| rk3588-probe | 28 / 12 | 1 / 0 |
| stm32mp257-probe | 46 / 12 | 1 / 0 |
| rk3588 hand-written | 3 / 4 | 8 / 0 |
| stm32mp257 hand-written | 16 / 6 | 11 / 0 |

Committed frame files, replayed with `run` (synthetic). Frames not listed agree under both settings.

| File | Frame | structural | full |
| --- | --- | --- | --- |
| `frames/section7/clipping.xcheck` | `overhang-right-bottom`, `negative-origin`, `fully-outside` | core-only | core-only |
| `frames/section7/knowledge-gap-scaling-groups.xcheck` | `one-upscaled`, `h-and-v-separately` | liftoff-only | both-accept |
| `frames/section7/knowledge-gap-scaling-groups.xcheck` | `two-upscaled-both`, `downscale-half` | liftoff-only | both-reject |
| `frames/section7/programmable-zpos.xcheck` | `restack-overlapping`, `restack-disjoint` | core-only | core-only |
| `frames/section7/search-effort.xcheck` | `eight-argb-two-nv12-late` | core-error(LimitExceeded) | core-error(LimitExceeded) |
| `frames/section7/value-checks-before-test-commit.xcheck` | `half-alpha-one-layer`, `rot180-on-overlay` | liftoff-only | liftoff-only |
| `calibration/scaled-group.xcheck` | `two-scaled`, `three-scaled` | liftoff-only | both-reject |
| `calibration/zpos-disjoint-inverted.xcheck` | `disjoint-inverted` | liftoff-only | liftoff-only |
| `calibration/zpos-programmable.xcheck` | `restack-needed` | core-only | core-only |

Every calibration frame matched its `.expect` line; `alpha-zero`'s open `liftoff-only|both-accept` resolved to `both-accept`.

## Triage of every difference

Verdicts as in design section 7 and `tools/liftoff-crosscheck/notes/section7-triage.md`: **expected (row)**, **artefact** (harness bug, fixed here) or **lead**. Frame files are under `tools/liftoff-crosscheck/` unless a path says otherwise. "core-only" means core accepts and libliftoff does not; "liftoff-only" the reverse (section 6 meaning, as `compare.rs` prints it).

| Difference | Inputs (frames) | Verdict | Check that classes it (row criterion) | Frame file |
| --- | --- | --- | --- | --- |
| liftoff-only under `structural`, core `NoPlane` with `SCALING` | imx95-synthetic (72), rk3588-probe (40), stm32mp257-probe (58), rk3588 hand-written (6), stm32mp257 hand-written (22); `knowledge-gap-scaling-groups` (4 frames), `calibration/scaled-group` (2) | **expected (S7-10)** | Present under `structural`, gone under `full` for every frame: the `full` runs have no liftoff-only frame, and the same frames are both-accept or both-reject there. On the probe conversions the scaling knowledge is "no scaler", because the probe claims none; on the i.MX95 stream it comes from `profile imx95dpu`, on the hand-written topologies from their `scaling=` lines | `findings/imx95-synthetic.synthetic.scale.0000.sb9bb94abb79b5957.structural.xcheck`, `findings/rk3588-probe.observed.scale.0000.s8b43ab541ee4c321.structural.xcheck`, `findings/stm32mp257-probe.observed.scale.0000.s5173e21bf4dcdf3f.structural.xcheck` |
| liftoff-only under `structural`, core `NoPlane` with `SOURCE_SIZE` | rk3588 hand-written (1: `rand.0138`) | **expected (S7-10)** | `max_source` is a profile/explicit limit the structural model does not check; under `full` the frame is both-reject | `findings/rk3588.observed.rand.0138.s68c4144c39ea5904.structural.xcheck` |
| core-only, both settings, on planes with a zpos range | rk3588-probe (1), stm32mp257-probe (1), rk3588 hand-written (8), stm32mp257 hand-written (11); `programmable-zpos` (2), `calibration/zpos-programmable` (1) | **expected (S7-3)** | (1) Core's plan replays through the model (no `invalid-assignment`), and with every plane's zpos pinned to `zpos_now` (`zpos=fixed:N`) all 21 swept frames become both-reject while libliftoff's placement is unchanged, so core's plan restacks planes. (2) For the two observed-probe frames, the libliftoff refusal goes away when the layer order matches the planes' current zpos order (both-accept); in the committed files `current-order-overlap` and `restack-not-needed` show the same. Every libliftoff test commit of the two probe frames was answered `ok`, so libliftoff refused by its own zpos rules, not through the model | `findings/rk3588-probe.observed.rand.0195.*` (finding, `fixed-zpos`, `reordered`), `findings/stm32mp257-probe.observed.rand.0028.*` (same three), `findings/rk3588.observed.rand.0005.s74a1119c5f6bf35c.full.xcheck`, `findings/stm32mp257.observed.rand.0015.s8570e2b48eb42e2d.full.xcheck` |
| core-only, layer reaching beyond the output | imx95-synthetic `scaled_small` (file frame, `full` only); `clipping` `overhang-right-bottom`, `negative-origin`, `fully-outside` | **expected (S7-12)** | Core clips (or, for `fully-outside`, drops the layer and returns an empty plan); libliftoff is given the unclipped rectangle and the model answers `EINVAL` "destination outside the 1920x1080 output". All generated frames lie inside the output (`edge=off`); these frames come from committed files, and `run` does not apply the v1 restriction. Under `structural`, `scaled_small` is both-reject (generic core has no scaler) | `fixtures/imx95-synthetic.xcheck` (frame `scaled_small`), `frames/section7/clipping.xcheck` |
| liftoff-only, disjoint layers that need out-of-order fixed-zpos planes | `calibration/zpos-disjoint-inverted` `disjoint-inverted` | **expected (S7-2)** | Core `NoPlane` with `Z_ORDER`; libliftoff places both layers (overlays 12, 11) and its assignment passes the model. With the pair overlapping (`overlap-inverted`) libliftoff refuses too (both-reject). Core's refusal was checked against its reject reason only, not against `planner.rs` line by line | `calibration/zpos-disjoint-inverted.xcheck` |
| core-error(LimitExceeded), both settings | `search-effort` `eight-argb-two-nv12-late` | **expected (S7-7)** | Re-run of the core side with raised `SearchLimits` (a throwaway test, not committed): still `LimitExceeded` at 20 000 000 steps; with `steps = u32::MAX` core returns `NoPlane { layer: 9, FORMAT \| IN_USE }` in 65 ms, which agrees with libliftoff (placed 1/10, a rejection). libliftoff reached no deadline (`INT64_MAX`) | `frames/section7/search-effort.xcheck` |
| invalid-assignment(core) / core-error(BackendRejected) on `flip_h`/`flip_v` layers | stm32mp257 hand-written (13 per setting) | **artefact, fixed** | See "Artefact fixed". After the fix these frames are both-accept or agree otherwise | `findings/stm32mp257.observed.rand.0018.lone-reflection.xcheck` |
| liftoff-only, both settings, layer with `blend=opaque` on an ARGB buffer over a plane without a blend property | `value-checks-before-test-commit` `half-alpha-one-layer`, `rot180-on-overlay` | **lead L-1** | Matches no row: not S7-6, because the refusing side is core, by its own blend rule, not libliftoff's property-definition check; not S7-11, because libliftoff accepts. See "Lead L-1" | `findings/L1-opaque-blend-without-blend-property.xcheck` |

Rows with no difference in this run:

- **S7-1 (contract):** partial libliftoff allocations occur inside `both-reject` (for example `five_yuv`, placed 5/6). They are counted as rejections, as designed, and are not differences.
- **S7-4 (invisible layers):** every alpha-0 frame (24 + 27 + 15 + 27 + 15 swept, `invisible-layers`, `alpha-zero`) agrees. On these frames, core plans without the alpha-0 layers, as libliftoff does.
- **S7-5 (format check skipped):** no liftoff-only frame on a format refusal. The fake sets `DRM_MODE_FB_MODIFIERS` and the `IN_FORMATS` blob, and `format-check-skipped` and `format-modifier-absent` agree.
- **S7-6 (value checks):** `coverage-blend` and `rot270-on-overlay` are both-reject on both sides. The fake defines the enums and libliftoff refuses before the test commit.
- **S7-8 (determinism):** no `unstable` frame (two libliftoff runs per file, `INT64_MAX` timeout).
- **S7-9 (truncated format list):** no difference on rk3588-probe can be traced to a truncated list. Both sides see the same cut list, and the frames' formats come from it.
- **S7-11:** none (no liftoff-only frame under `full` other than L-1, which is the other direction).
- **S7-13 (zpos ties):** `tie` families, `zpos-tie-rule` and `calibration/zpos-tie` agree.
- **S7-14, S7-15:** no cursor plane and no composition layer appear.
- **S7-16:** `format-modifier-absent` and `calibration/format-absent` are both-reject as predicted.
- **S7-17:** `planes-per-output` agrees on both outputs.

Predictions refuted by the run: `frames/section7/zpos-and-intersection.xcheck` `swapped-disjoint` was predicted liftoff-only and is both-reject. `programmable-zpos` `restack-disjoint` was predicted both-accept and is core-only. In both, the lower layer can only use an overlay whose zpos is above the primary, and the primary holds the higher layer. libliftoff then refuses even for disjoint layers: by rule 4 of `check_layer_plane_compatible` (**source**, `notes/libliftoff-c4226a7.md` section 8: no "underlay" below the primary's layer), which does not look at intersection. So S7-2 holds for overlays only (`calibration/zpos-disjoint-inverted`), not when the primary plane is involved (**inferred** from these runs and the code reading).

## Artefact fixed

**Lone reflection sent without a rotation bit** (**source**, harness). For a layer `transform=flip_h` (or `flip_v`), the C driver sends `rotation = ROTATE_0 | REFLECT_X` (0x11), as DRM requires exactly one rotation bit (`parse_layer_transform` in `c/driver.c`). The Rust core side copied the bare layer bit (0x10) into its plane states (`core_side.rs`, `plan_states`), and so did the fallback rebuild of libliftoff's states (`compare.rs`, `liftoff_states`). The model refused 0x10 as "malformed rotation". So on the hand-written STM32MP257 topology, the only swept input with reflections, every core plan with a flipped layer came out `invalid-assignment(core)` under `structural` and `core-error(BackendRejected)` under `full` (13 frames each).

Fix in `tools/liftoff-crosscheck/`: a new `state::rotation::property_value`, used in both places, adds `ROTATE_0` when no rotation bit is set. A unit test (`a_lone_reflection_is_sent_with_rotate_0` in `core_side.rs`) checks that a `flip_h` frame planned by core carries 0x11 and passes the model under both settings. All numbers in this report are from runs after the fix. `tests/input_files.rs` now also parses the files under `findings/`.

## Lead L-1: opaque blend on a plane without a blend property

- **Input:** synthetic, `frames/section7/value-checks-before-test-commit.xcheck`. Replay file: `tools/liftoff-crosscheck/findings/L1-opaque-blend-without-blend-property.xcheck`, which adds XR24 controls. No hardware claim.
- **Topology:** primary 40 has no `pixel blend mode` property (`blend=-`) and no `alpha`; overlay 41 has `alpha` and the blend modes opaque and premul.
- **Outcomes (both settings):** `half-alpha-one-layer` is liftoff-only: core `NoPlane { layer: 1, ALPHA | BLEND | IN_USE }`, libliftoff places the layers on 40 and 41, and the model accepts that. `rot180-on-overlay` is liftoff-only: core `NoPlane { layer: 1, BLEND | TRANSFORM | IN_USE }`, libliftoff places them on 40 and 41. In `opaque-on-primary` both accept, but core uses overlay 41 and libliftoff primary 40. The same frames with an XR24 bottom layer are both-accept, and core then uses 40 and 41.
- **Deciding rule (source, checked against the topology):** core lets a `blend=opaque` layer onto a plane without blend modes only if the buffer's format is known to be opaque (`crates/composition-core/src/planner.rs`, `blend_free = layer.blend == BlendMode::Opaque && info.format.is_known_opaque()`). AR24 carries alpha, so core refuses plane 40 for the bottom layer, gives it 41, and has no plane left for the top layer. libliftoff skips a property the plane lacks when the layer's value is 0 (`pixel blend mode` 0 = "None", `notes/libliftoff-c4226a7.md` section 6). The model accepts opaque on a plane without the property, which is what a kernel's `TEST_ONLY` would do, since there is nothing to set.
- **Reading (inferred):** this is a difference of meaning, not of validity. A plane without `pixel blend mode` blends the way the kernel defaults to (pre-multiplied, by DRM's convention), so an ARGB buffer marked "ignore pixel alpha" on such a plane would be shown with its alpha applied. libliftoff, and a real `TEST_ONLY` commit, accept it anyway. Core refuses, and so composites (or fails) where libliftoff would show a different picture. Core's behaviour looks like the defensible one, and **no core change is proposed**.
- **For the planner owner:** confirm that the strict rule is intended. If it is, the model could copy the opaque-format exemption so that the `full` setting stops reporting it. `notes/core-behaviour.md` already recommends that, but it would make the model follow core rather than the kernel, which the design warns against (circular agreement). Choosing between the two is a decision for the owner; this report changes neither the model nor the core.

## Harness limits found, not fixed

On supplementary inputs only; none of the inputs this item names reaches them.

- `sweep topologies/bcm2712.xcheck` panics: `family zperm above 500 frames` (`src/sweep.rs`, the `FAMILY_CAP` assertion). The topology has 13 planes on its output, so `n_max` is 15 layers. 2..4 layers give 32 orders, n = 5..15 give 2n orders each, and every order comes disjoint and overlapping: 64 + 440 = 504 frames, over the cap (**source**, `sweep.rs`). A fix changes the generated frames for many-plane topologies, so it needs a `GEN_VERSION` bump; it belongs to a generator item.
- `sweep topologies/bcm2711.xcheck` did not finish within 20 minutes and was stopped; no output. Not investigated further. Its 11 planes on the output stay under the `zperm` cap (388 frames by the same count), so the cause is elsewhere. Likely (**inferred**) it is a long search on many layers: libliftoff's exhaustive search with `INT64_MAX`, or core's (S7-7 territory). The sweep has no per-frame time bound.
- The model refuses any destination outside the output; real KMS drivers commonly accept partly off-screen planes and clip. This is the design's v1 restriction, and it is why S7-12 frames from committed files show up as core-only.

## Assumptions for the project owner

- **One model, over a pipe** (the design's open decision, section 8): the Rust side answers every libliftoff `TEST_ONLY` commit from its own model (`src/model.rs`), so the rules exist once; the C driver has no model. This is the design's stated preference, not an owner decision.
- **`composition-drm` is a path dependency** of the harness, besides `composition-core`, so that `profile imx95dpu` runs through `DriverProfile::Imx95Dpu.apply`. The root `Cargo.toml`, `Cargo.lock` and CI are unchanged.
- The hand-written topologies under `topologies/` and the committed section-7 and calibration frames were run in addition to the five inputs that step (d) names. Their results are kept separate in the tables.
