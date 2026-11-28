# WP5-04 / WP5-06 plan: cross-check `advise` against libliftoff on i.MX95 board data

Written 2026-11-28 from the repository at commit `df507ed`. **This is a plan, not a result.** Nothing in it was run for this document, no board was used, and every advantage of this engine over libliftoff is **not measured**. It closes nothing in `docs/open-issues.md`; it says what would move WP5-04 (cross-check) and WP5-06 (benchmarks from the M1 board runs) forward.

Tags, as in `docs/platform-capabilities.md`: **observed** (from a board, with its date), **read from source** (a file at a commit), **inferred**, **not measured**, **proposal**. Dates are YYYY-MM-DD.

Sources read for this plan, 2026-11-28: `docs/libliftoff-crosscheck.md` (design), `docs/liftoff-crosscheck-report.md` (host results), `scripts/check-liftoff-crosscheck.sh`, `tools/liftoff-crosscheck/` (README, fixtures, topologies, `src/main.rs`), `docs/host-metrics.md`, `docs/advisory-planner-design.md`, `docs/m1-imx95-report.md` (read only), the test file list of `crates/composition-fake/tests/`.

## 1. Data that exists today

| Data | What it is | Tag | Where |
| --- | --- | --- | --- |
| i.MX95 probe dump | `probe --raw` of the board: 2 CRTCs (39, 50), 1 connector (41), 7 planes (primaries 31 and 42, overlays 51, 59, 67, 75, 83), 13 linear formats per plane, zpos 0..5 mutable, plane alpha, three blend modes, no rotation, no cursor plane | **observed** 2026-11-28 (kernel `6.6.157-7.8.0`, per `docs/m1-imx95-report.md`) | `docs/imx95-probe-2026-11-28.txt` |
| i.MX95 xcheck topology | The dump written by hand into the cross-check input format; `profile imx95dpu`; output 39 at 1280x800; `zpos_now` is the compositor's state at capture (plane 31: 5, plane 42: 0, overlays all 1) | **observed** 2026-11-28 for the plane data; scaling/group constraints are **read from source** (`DriverProfile::Imx95Dpu`), not measured | `tools/liftoff-crosscheck/topologies/imx95-observed.xcheck` |
| i.MX95 synthetic stream | Six planes (ids 40..45, `zpos_now` 0..5, 1920x1080) imitating the `Imx95Dpu` unit test; four frames | **synthetic** 2026-11-28, describes no board | `tools/liftoff-crosscheck/fixtures/imx95-synthetic.xcheck` |
| RK3588, STM32MP257 probe dumps and hand-written topologies | Used as the harness's other inputs | **observed** 2026-11-28 (recorded) | `tools/liftoff-crosscheck/fixtures/*.probe.txt`, `topologies/` |
| BCM2711, BCM2712 probe dumps | Converter refuses them (no per-plane format list in the documents) | **observed** 2026-11-28, unusable as is | `fixtures/bcm2711-cm4.probe.txt`, `fixtures/bcm2712.probe.txt` |
| Cross-check results | Host sweeps on all of the above, including step (f) on the i.MX95 topology: 450 frames per model setting; structural 70 both-accept / 207 both-reject / 156 core-only / 17 liftoff-only; full 80 / 193 / 177 / 0 | host results of 2026-11-28, libliftoff `c4226a7`, fake libdrm; **no hardware claim** | `docs/liftoff-crosscheck-report.md` ("Step (f)") |
| M1 board figures | Two layers on planes 31 and 51 accepted by `TEST_ONLY`; `combos` 16 accepted, 4 refused by the planner, 0 refused by `TEST_ONLY`, up to six planes; `Engine::validate` (`TEST_ONLY`) median 135-142 µs, p99 208-310; `rotate` submit-to-latch median 11 168 µs; commit call median 133 µs; harness RSS 812-1 280 KiB | **observed** 2026-11-27 (board), 2026-11-28 (probe) | `docs/m1-imx95-report.md` (conditions 3, 4, 5) |
| Host planner timing | `Engine::validate` on the fake backend, 2 to 16 planes, 1 to 16 layers; median about 135 ns to 2.9 µs for found plans | **observed** 2026-11-28 on a development host (i7-1165G7), not hardware | `docs/host-metrics.md` |
| `advise` host tests | `advise` over models of recorded RK3588 and BCM2711 scenes, fuzz, and `advise` against `plan` | host tests, no libliftoff, no board | `crates/composition-fake/tests/advise_scenes.rs`, `advise_fuzz.rs`, `advise_vs_plan.rs`; `crates/composition-core/tests/advise.rs` |

Two things the table shows. First, the cross-check input from the board is a **topology** only: no frame (layer set) that the board actually ran has been written down as a cross-check frame. Second, the M1 numbers are timings of the Rust engine and of the kernel; none is a libliftoff timing.

## 2. What the existing cross-check covers, and what it does not

**Covered** (host, **no hardware claim**; `docs/liftoff-crosscheck-report.md`):

- `composition_core::planner::plan` (through `core_side.rs`) against libliftoff `c4226a7` on the observed i.MX95 topology, 450 generated frames per setting (families `fill`, `zperm`, `fmt`, `tie`, `alpha0`, `rand`, `scale`; seed `0x5eed11f70ffc4226`, 200 random samples), both model settings (`structural`, `full`).
- Every difference triaged (expected S7-3 and S7-10, none an artefact or a lead on that input).
- Outcome classes `both-accept`, `both-reject`, `core-only`, `liftoff-only`, plus libliftoff's placed-layer count per frame.

**Not covered:**

1. **`advise` was never run through the harness.** The harness has no call to `composition_core::advise` (no match for `advise` in `tools/liftoff-crosscheck/src`). The design's test 13 ("libliftoff cross-check", `docs/advisory-planner-design.md` §5/§7) does not exist. The comparison it makes is `plan` (all layers on planes, or an error) against libliftoff's *all layers placed* reading; libliftoff's actual contract is a **partial allocation plus composition**, which is what `advise` answers (`docs/advisory-planner-design.md` §2). So the closest like-for-like comparison, accepted planes and composed layers, is untouched.
2. **No frame from the board.** All 450 frames are generated or synthetic; the two-layer scene of M1 (bottom layer on plane 31, top layer on overlay 51) and the six-plane `combos` cases are not frames in any file.
3. **Zpos state of the dump (observed 2026-11-28).** The `zpos_now` equal-at-1 state is Weston's at capture time. The report says whether the driver lets a client change `zpos` of these planes in one atomic commit is **not shown**; the 156 / 177 `core-only` frames hinge on it (**inferred**).
4. **No hardware answer to `TEST_ONLY`.** The fake kernel is the Rust model. On the board, `combos` showed 16 accepted and 0 refused by `TEST_ONLY` (observed 2026-11-27), which agrees with no model rule being too loose on those 16 cases, but 16 cases do not validate the model.
5. **Not in CI**, and `scripts/check-liftoff-crosscheck.sh` only checks fmt, clippy, tests and layout of the crate, not any cross-check result.
6. **No timing of libliftoff at all**, on host or board. Host timing exists only for the Rust engine (`docs/host-metrics.md`).
7. Harness limits found and not fixed: `sweep` panics on 13-plane topologies (`zperm` cap), and `sweep topologies/bcm2711.xcheck` did not finish in 20 minutes. Neither touches the i.MX95 topology (7 planes).
8. The two-model question is open: the model answers both sides, so agreement can be circular (design §6, report "Assumptions").

## 3. Cross-check on the i.MX95 topology: exact steps

All steps run on a host. None needs the board. Run from the repository root unless a `cd` is shown.

**3.1 Build the fixed pieces**

```sh
sh scripts/check-liftoff-crosscheck.sh                 # crate fmt, clippy -D warnings, tests, layout
bash tools/liftoff-crosscheck/build-liftoff.sh         # last line printed = driver path
```

The script fetches libliftoff at `c4226a79b7a52f59bb51789b1eda112658609125` into `$CONNECTED_DEPS_DIR/libliftoff` (or takes a path) and refuses another commit. Keep the driver path in `DRIVER`.

**3.2 Reproduce step (f) first** (the baseline; the file name is part of the seeds, so keep it)

```sh
cd tools/liftoff-crosscheck
cargo run --release -- sweep topologies/imx95-observed.xcheck --driver "$DRIVER" --out "$TMPDIR/sweep"
```

Expected: 450 frames per setting and the counts in section 1. A different count is a finding about the harness or the pin, to be explained before going on.

**3.3 Write the board frames** (proposal; new files, no code change)

Input: `docs/m1-imx95-report.md` conditions 3 and 4, read only. One new file, for example `tools/liftoff-crosscheck/frames/imx95-m1.xcheck`, with the topology lines of `topologies/imx95-observed.xcheck` (same `origin observed 2026-11-28 ...` line) and these frames, each tagged in a comment with the M1 evidence line it imitates:

| Frame | Layers | Imitates |
| --- | --- | --- |
| `two-planes` | full-screen buffer at 0,0; 640x400 buffer at 320,200 on top | the `two-planes` run, planes 31 and 51 (observed 2026-11-27) |
| `rotate` / `move` | the same two layers with the top layer's destination moved | `rotate`, `move` |
| `alpha-plane`, `alpha-pixel` | plane alpha 0x8000 on the top layer; premultiplied ARGB | the alpha runs |
| `combos-1..6` | 1 to 6 layers, XR24 / AR24 / NV12, 2x upscale, partly off-screen | the 16 accepted `combos` cases, as far as the report names them |
| `combos-refused` | 7 layers; 2x downscale; 90° rotation; unknown format | the 4 refused `combos` cases |

What the report does not give (exact source/destination rectangles of each `combos` case, buffer ids) must be taken from `docs/m1-imx95/logs/combos.log` and the harness source, not guessed; any value taken by assumption is marked **synthetic** in the file. The frame content is then **observed-shaped but not observed**: the label on result lines stays `observed` for the topology and the frames say what they imitate.

Run:

```sh
cargo run --release -- run frames/imx95-m1.xcheck --driver "$DRIVER"
```

`run` prints one line per frame under both settings with the origin label.

**3.4 Compare `advise` with libliftoff** (needs a harness change, so it is a work item, not part of this plan's deliverable)

The harness lacks an `advise` side. Proposal for that item, outside the workspace like the harness itself, no change to library crates:

- Add an `advise_side` module that builds the same `Capabilities` as `core_side.rs`, calls `composition_core::advise` once with composition disabled (`AdviseOptions` with `compose: None`) and once with a composition target, and records per frame: layers placed on planes, layers composed, `Blocker` per composed layer, whether a composition target fit a plane, and the backend-check count (`Advice::backend_checks()`).
- Feed it the test-commit model as the `accept`/check callback under `full`, and the accept-all check under `structural`, as for `plan`.
- Map libliftoff's side per design §6 step 2 but **keep the partial result**: `liftoff_layer_get_plane` per layer gives planes used; `liftoff_layer_needs_composition` gives composed layers. Record both without the "every layer placed" rule.
- Replay each side's assignment through the model (design §6 item 4); a placement the model refuses is `invalid-assignment`, not a difference.
- New outcome classes, **proposal**: `same-split` (same count of layers on planes and composed), `advise-more-planes`, `liftoff-more-planes`, `advise-fewer-composed` and the mirror. Equal split is not equal assignment: assignments are compared by validity only.

Commands once the item exists (names are a proposal; the existing sub-commands are `convert`, `run`, `sweep`):

```sh
cargo run --release -- run frames/imx95-m1.xcheck --driver "$DRIVER" --advise
cargo run --release -- sweep topologies/imx95-observed.xcheck --driver "$DRIVER" --advise --out "$TMPDIR/sweep-advise"
```

The sweep keeps `GEN_VERSION=1`, the same seed and file name, so the 450 frames are the ones of section 3.2 and the two reports line up frame by frame.

**3.5 Host checks that must stay green when the item changes anything**

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, the two `thumbv7em-none-eabihf` builds, and `sh scripts/check-liftoff-crosscheck.sh` (the harness is outside the workspace). No library crate changes in this plan.

**3.6 Report**

A dated report next to `docs/liftoff-crosscheck-report.md` (a new file; that report is not edited): versions as in its "Versions" table, origin labels, one table per setting, triage of every difference, and the sentence that nothing ran on a board.

## 4. Metrics to compare

All host metrics. Hardware metrics only if they were measured on the board in section 5, with date.

| Metric | Source on each side | Status today |
| --- | --- | --- |
| Accepted planes (layers on planes) | `advise`: `LayerAdvice` placement; libliftoff: `liftoff_layer_get_plane` | **not measured** for `advise` against libliftoff; exists for `plan` only (report counts) |
| Composed layers | `advise`: composed count and run; libliftoff: `liftoff_layer_needs_composition` | **not measured** |
| Whether composed layers are contiguous | `advise` guarantees one contiguous run by design (`advisory-planner-design.md` §2); libliftoff has no such rule | **not measured**; a property to check, not to claim |
| Both-accept / core-only / liftoff-only counts | existing classes | observed on host 2026-11-28 for `plan` (section 1) |
| Search effort | `advise`: `backend_checks()`, candidates tried; libliftoff: test commits answered (counted by the fake) | **not measured** |
| Host time per frame | `std::time::Instant` around the call, release build, median over a stated iteration count, as in `docs/host-metrics.md`; libliftoff called through the C driver with timeout `INT64_MAX`, fresh device per frame | **not measured** for libliftoff and for `advise`. `Engine::validate` host figures exist (`docs/host-metrics.md`) and are not `advise` figures |
| Hardware time | commit or `TEST_ONLY` time on the board | **not measured** for libliftoff; for `validate` observed 2026-11-27 (135-142 µs median), which includes the real kernel `TEST_ONLY`, not libliftoff |

Rules for the comparison:

- Compare medians, not maxima (`docs/host-metrics.md`). A time includes whatever the fake answers cost; the Rust model answers libliftoff's test commits over a pipe, so libliftoff's host time includes pipe round trips and is **not** comparable with `advise`'s time unless `advise` is given the same model as its check. State which, in the report. Anything else is an unfair figure and is not reported.
- No hardware timing is stated unless it was measured on the board and dated. Host time is labelled host.
- A result of "same split on N of M frames" supports nothing about speed or about a plane or two on hardware; it says how two planners treat a description. This is the project's rule (`docs/liftoff-crosscheck-report.md` "What this is, and what it is not").
- Every advantage of `advise` (all-or-explained answer, `Blocker` reasons, bounded search, DriverProfile knowledge) is **not measured** until a run in this plan shows it, and then it is stated with the frame count and the input's origin.

## 5. Open data needed from the board

None of this needs doing for the host steps. Each item is a request for a later device item; the Verdin iMX95 is in use by other plans and this plan does not take it.

1. **Zpos writability, one atomic commit.** Whether the driver accepts a different `zpos` on planes 31 and 51..83 in a `TEST_ONLY` commit, starting from Weston's equal-zpos state and from a clean state. Decides whether the 156 / 177 core-only frames are expected or a planner constraint gap (report, "For the planner owner"). Read-only ioctls are not enough; a `TEST_ONLY` commit on a master descriptor with Weston stopped (as in M1) is. **Not measured.**
2. **Board frame list.** The exact layer rectangles, formats and buffer sizes of the 16 accepted and 4 refused `combos` cases, as machine-readable lines, so that section 3.3 does not rely on prose.
3. **`TEST_ONLY` verdicts for the generated frames.** The same 450 frames (or the `core-only` and `liftoff-only` subset) submitted as `TEST_ONLY`, to compare the model with the kernel per frame. Only a kernel answer can turn a difference from "harness model" into "hardware"; today none exists. No commit, no display change.
4. **Timing of a libliftoff run on the board** (WP5-06): libliftoff built for aarch64 (not done; `docs/dlm-aarch64.md` covers drm-lease-manager only) and run against the real device on the same frames with the same timeout, versus `advise` driven by the real `TEST_ONLY` check. **Not done, not measured.** It also needs a decision on whether a libliftoff build for the board may be made (MIT notice; the cross-check design keeps libliftoff out of the repository).
5. **A second observed topology** with a different connector layout is not required; the CM4 and RK3588 dumps already give other inputs. BCM2711 and BCM2712 need either the converter relaxed or their per-plane format lists recorded (report, "refused").

All board work would follow the repository's board rules (`docs/imx95-board.md`): the board restored as found, no addresses, hosts, serials or UART paths in files.

## 6. Order of work and owners' decisions

1. Host: section 3.2 reproduction, then 3.3 frames (docs and fixture files only).
2. Host, small harness item: the `advise` side (3.4) and its report (3.6).
3. Host: a timing example for `advise` (the plan has a separate item for `crates/composition-fake`) and, for libliftoff, a timing mode of the harness with the fairness rules of section 4.
4. Board (later, only if the owner wants a hardware figure): items 1 to 4 of section 5.

Decisions the plan does not take: whether the model is written once or twice (design §8); whether the harness joins the workspace or CI; whether libliftoff may be built for the board or vendored; whether stateful `Engine::advise` (advisory-planner-design §6 item 6) is first, since its cross-check would differ from the stateless one.

## 7. What this plan does not claim

- No result: the comparison of `advise` with libliftoff, the benchmark, and the board runs are **not done**.
- No advantage is claimed: fewer composed layers, better placement, lower time, bounded effort, or better reasons than libliftoff are all **not measured**.
- The existing `plan` results are host results against a model of the kernel; they do not show what the board accepts.
- Nothing here promises that any of it is offered to, or accepted by, libliftoff or Weston.
