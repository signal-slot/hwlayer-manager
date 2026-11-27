# Frame generator of the cross-check sweep: design notes

Input for item xc-sweep (`tools/liftoff-crosscheck/src/sweep.rs`). It specifies; it writes no
code and adds no frame files. Basis: `docs/libliftoff-crosscheck.md` sections 5 (xcheck v1),
6 (comparison), 7 (expected differences) and 9 (order of work). "Proposal" marks a choice of this
note that xc-sweep may change with a reason.

**No hardware claim.** Every topology read is recorded, hand-written or synthetic. A sweep result
describes the planner and libliftoff on that description, never a board. The bounds below are
proposals; no run time is measured.

## 1. Unit and limits

The unit is a *case*: topology, model setting (`structural` or `full`, design section 6) and one
frame (or a short sequence, section 5). The generator is a pure function of
`(topology, seed, bounds)`. It reads topology files from the crate (never `docs/observed/`).

Engine limits (`crates/composition-core/src/limits.rs`): `MAX_LAYERS = 16`, `MAX_BUFFERS = 64`,
`MAX_PLANES = 16`, `MAX_FORMATS = 32`. A frame the reader would refuse is a generator bug.
Frames core refuses as not well formed (`UnknownBuffer`, `InvalidSource`, `InvalidDestination`,
`CannotClip`) are `skipped` by the comparison; the generator avoids them except in the opt-in
`edge` class, where the skipped count is itself a result.

## 2. Dimensions

`P` = planes whose mask includes the output (one output per run in v1).

| # | Dimension | Values | Notes |
|---|---|---|---|
| D1 | Layer count `n` | `1 ..= min(P+2, MAX_LAYERS)` | `P+1`, `P+2` exceed the plane count on purpose (contract, search effort) |
| D2 | Format of a layer's buffer | `(fourcc, modifier)` pairs from the topology's planes, walked in dump order | Classes: accepted by every plane; by exactly one plane; by none (a pair from another topology). Pairs beyond the 32 a truncated dump holds are not in the file and not generable |
| D3 | Buffer size | 16x16, 64x64, 320x240, 1280x720, output size, output size +1 in w or h, 4096x2304; odd: 17x15, 33x33, 641x361, 1279x719 | One `buffer` record each; at most `n` buffers per file |
| D4 | Source rectangle | whole buffer; centred crop; crop with odd origin or size; odd width on a YUV format | Aims at alignment/chroma rules of the model |
| D5 | Destination | inside the output; touching an edge; odd position/size; **edge class (opt-in):** partly off-screen (negative x/y or past the output), fully off-screen, zero size | Design section 7 "Clipping" proposes restricting v1 to frames inside the output; default sweep follows that |
| D6 | Scaling | 1:1; 2x, 4x up; 2x, 4x down; 1/16 down; w and h ratios differing; exactly the plane's limit and one 16.16 unit past it | Limits read from the file's `scaling` or from what `profile imx95dpu` applies. `scaling=none` topologies get the scaled rows too (model refuses; `structural` vs `full` shows) |
| D7 | zpos order (`z`) | identity; reversed; rotated by 1; adjacent swap; random permutation; ties (all equal, some equal); negative; large | Core stable-sorts, later in frame on top; the harness maps to distinct ranks for libliftoff (design section 4) |
| D8 | Alpha | 0xFFFF; 0x8000; 0x0001; 0; mixed over layers | 0 = "invisible layers" class, compared with those layers removed |
| D9 | Blend | opaque, premul, coverage, mixed | Decided by each plane's `blend=` list |
| D10 | Transform | normal, rot90, rot180, rot270, flip_h, flip_v, one rot+flip | Planes with `transforms=-` reject all but normal |
| D11 | Overlap | disjoint; all overlapping; chain (neighbours only) | Targets libliftoff's intersect-only ordering |
| D12 | Change between frames | section 5 | Sequences |

Not generated in v1: cursor planes, composition layers, more than one output, `replace_pending`,
groups as an axis (groups come from the topology and are exercised via D1 x D2, e.g. more YUV
layers than a group's `capacity`).

## 3. Enumerated part and sampled part

The full product of D1..D11 is far too large, so a sweep is the union of two parts.

**E: enumerated, no randomness.** Named families in a fixed order, each with a bound:

| Family | Enumerates | Layers |
|---|---|---|
| `fill` | D1 with identical full-screen 1:1 layers of a format accepted by all planes, z identity | `1 ..= P+2` |
| `zperm` | D7 all permutations for `n <= 5` (at most 120); for larger `n`: identity, reversed, all adjacent swaps, all rotations; each disjoint and overlapping (D11) | up to `P+2` |
| `fmt` | D2: each pair of the format union alone; then ordered pairs of the three classes | 1, 2 |
| `scale` | D6 each factor, up and down, on primary and overlay planes | 1 |
| `scalebound` | D6 limit and limit+1 unit, per axis | 1 |
| `prop` | D8, D9, D10 each value alone | 1 |
| `propmix` | D8 x D9 x D10, each from {default, one non-default} | 2 |
| `size` | D3 x D4 x D5 inside the output, one axis varied against defaults | 1 |
| `edge` | D5 off-screen/zero-size classes (opt-in) | 1 |
| `tie` | D7 tie classes | 2..=4 |
| `alpha0` | D8 alpha 0 on one layer and on all | 2..=P+2 |
| `chg` | D12 (section 5) | 2..=P+2 |

**S: sampled (`rand`).** Per frame: pick `n`, then each layer's values across D2..D10, D11 and D7
per frame. Weights:

- `n` uniform over `1 ..= P+2`, with at least a third at `n >= P` (just fits / just fails).
- Per layer 0.8 chance of the default for each of D6, D8, D9, D10; 0.5 for a whole-buffer source.
- Format: 70% from the classes "all planes" or "one plane", 30% anything in the union.
- Inside the output unless the run has `--edge`.

Sample count per topology and model setting: `--samples N`, default 2000 (proposal).

## 4. Seed scheme

- One 64-bit master seed, a fixed constant in the code (chosen once; never from time or
  environment). `--seed <u64>` overrides it for exploration; reports use the default.
- Self-contained PRNG (SplitMix64, proposal), no `rand` dependency; the crate stays free of
  registry dependencies. No `HashMap` iteration order anywhere; sort formats and planes before
  drawing from them.
- Per-case seed so one case replays alone:
  `case_seed = splitmix64(master ^ fnv1a64(topology_name) ^ (family_id << 32) ^ index)`.
  Adding a family or changing `--samples` does not change frames of other families or earlier indices.
- Recorded in the output: master seed, `GEN_VERSION` (an integer bumped by any change that alters
  produced frames), topology name and `origin`, bounds, model setting: in the report header and as
  comments in each written frame file.
- Tests (proposal): generate twice and compare byte for byte; a golden list of the first frames of
  each family on the synthetic i.MX95 topology catches silent generator changes.

## 5. Changes between consecutive frames

The design uses a fresh libliftoff device per frame, so a sequence is not a test of
libliftoff's allocation reuse. Sequences instead check (a) whether core's answer for frame k+1
depends on frame k (core `plan` should be a function of frame and `Capabilities`; any dependence
is a finding) and (b) replay in order from one file.

Family `chg`, base frame of `n` layers, `n` in {2, P-1, P, P+1}: add a layer on top; add at the
bottom; remove top / bottom / middle; swap two layers' `z`; move one destination by a small odd
delta; change one layer's format to another class; alpha to or from 0; replace a buffer by another
size. Sampled sequences: base plus 1..3 such changes. At most 4 frames per sequence.

## 6. Dimensions versus design section 7

| Section 7 row | Families / dimensions |
|---|---|
| Contract (partial allocation) | `fill` with `n = P+1, P+2`; D1 sampled |
| zpos and intersection | `zperm`, disjoint vs overlapping |
| Programmable zpos | `zperm`, `fill` on topologies with `N..M` zpos, against `fixed:N` planes |
| Invisible layers | `alpha0`, D8 |
| Format check skipped | `fmt`, the "accepted by none" class; the C driver must set `DRM_MODE_FB_MODIFIERS` and `IN_FORMATS`, else this row measures the driver |
| Value checks before the test commit | `prop`, `propmix`, including values the plane does not list |
| Search effort | `fill` and sampled `n >= P` on `P >= 6`; many same-format layers (fits everywhere, search exhaustive) |
| Search determinism | not by generation: the runner repeats a frame (proposal: 3 times) for the `unstable` class; generator marks `repeat=3` on `fill` and the first 50 sampled frames |
| Truncated format list | `fmt` on Broadcom and RK3588 topologies; report header flags topologies with a plane at exactly 32 formats as `truncated?` |
| Knowledge gap, scaling/groups | `scale`, `scalebound`, group-capacity cases of D1 x D2; same frames under `structural` and `full` |
| Knowledge gap, other direction | whole sampled part under `full`; any `liftoff-only` there is a lead |
| Clipping | `edge`, D5 off-screen; opt-in |
| zpos tie rule | `tie`; run before any other `z` family is believed |
| Format absent from dump | `fmt`, class "none" |
| Planes per output | through `P`; one output in v1 |
| Cursor, composition layer | not generated |

D4 odd alignment and D10 rotation have no section 7 row; their differences are triaged as *lead*
unless they trace to a model rule, and the report says the dimension had no prediction.

## 7. Replayable file names

Each generated frame (or sequence) is a complete xcheck v1 file: the topology's records
(`origin`, `profile`, `output`, `plane`, `group`), the `buffer` records it uses (renumbered from 1
in order of first use) and the `frame` block (a sequence: several, `...-f0`, `...-f1`). `replay
<file>` needs nothing else.

```
<topology>.<origin>.<family>.<index>.s<case_seed, 16 hex digits>.<model>.xcheck
e.g. rk3588.observed.zperm.0042.s00ab12cd34ef5678.full.xcheck
```

- `<topology>`: stem of the topology file (`rk3588`, `bcm2711-cm4`, `imx95-synthetic`).
- `<origin>`: the file's own `origin` word. The design has `observed`, `source`, `synthetic`;
  probe-derived (recorded) topologies carry `observed`. Do not add a fourth word. A hand-written
  topology that adds constraints to a recorded dump keeps its `origin` word and the report names both files.
- `<family>`: section 3 id; sampled frames use `rand`. `<index>`: 4 digits, per family, topology
  and model. Enumerated frames carry the seed field too (derived the same way; it does not drive
  them), so the pattern is uniform.
- `frame <name>` inside the file is the file name without extension, only `[A-Za-z0-9._-]`.
- Top-of-file comments (reader ignores `#`): `GEN_VERSION`, master seed, `case_seed`, family,
  index, topology file name, and the copied `origin` line.
- Only interesting frames are written: any outcome class other than `both-accept` and
  `both-reject`, plus the first frame of each family. Proposal: into a `--out` directory under
  `$TMPDIR`, never the repository; a report commits only the named lead frames (design section 7:
  a lead is never summarised without its frame file).

## 8. Output labelling

- Every report line starts with `[<origin>:<topology>]` (e.g. `[synthetic:imx95-synthetic]`),
  then model setting, family, case id.
- Summary per topology: file, `origin` line verbatim, counts by family and by outcome class, seed,
  `GEN_VERSION`, bounds, whether the edge sweep ran, whether the sweep was `incomplete`.
- Footer: synthetic and hand-written inputs print `no hardware claim: topology is <synthetic |
  hand-written | recorded at DATE>`; observed prints `topology recorded at <date>; no claim about
  any other board, kernel or boot`.
- No board address, host name or device path appears in generated files (design section 5, last paragraph).

## 9. Size bounds (a few minutes on a host)

Cost per case: one request to a long-lived C driver, pipe round trips for `TEST_ONLY` answers, and
search effort at `n >= P` (core up to 20 000 steps and 32 backend checks; libliftoff many test
commits). Nothing is measured; xc-sweep times a first run and adjusts.

- One long-lived driver process per topology, not one per case.
- Per topology and model setting: enumerated at most 3000 cases, sampled 2000 by default. No family
  above 500 frames (assert in the generator). Permutations: no `n!` beyond `n = 5`.
- `n <= min(P+2, 16)`, at most `n` buffers per frame, so `MAX_BUFFERS` is never near.
- Per-case limit 2 s on the libliftoff side, with `timeout_ns` far above the 1 ms default and the
  deadline-fired flag recorded; expiry is `skipped(timeout)`, counted and printed.
- Per-sweep limit 5 min; the runner stops, prints progress and marks `incomplete`.
- Report lists classes with counts and the first 20 frame names per non-trivial class; the full
  list goes to `--out`.
- Order: the synthetic i.MX95 stream first (6 planes, `n <= 8`), then RK3588, then the others.

## 10. Caveats for xc-sweep

- Needs from the reader: planes, formats in dump order, zpos kind and `zpos_now`, property lists,
  scaling limits, groups. Add accessors in the crate if missing; no change to
  `crates/composition-core/src/**` is needed.
- Equal layer zpos maps to distinct ranks (design section 4); `tie` must pass first.
- Alpha-0 layers: compare with those layers removed (design section 7).
- The seed constant and numbers in section 9 are proposals; record final values in code and report.
- Nothing generated here, and no result built on it, is a hardware observation.
