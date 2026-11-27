# liftoff-crosscheck

Cross-check of the `composition-core` planner against libliftoff (WP5-04).
Design: `docs/libliftoff-crosscheck.md` (section 8 describes this layout).

This crate is **outside the root workspace** (its `Cargo.toml` has an empty
`[workspace]` table) and is **not in CI**. Recorded or synthetic topologies give
**no hardware claim**; every result line is labelled with the `origin` of its
input (`observed`/`source`/`synthetic`, date, text) and the model setting.

Modules: `xcheck` (the input format `xcheck 1`: reader and writer), `state`
(`PlaneState`, shared), `probe` (probe text to xcheck), `model` (the
test-commit model, the fake kernel), `core_side` (composition-core planner
runs), `pipe` (the C driver's `TEST_ONLY` pipe), `compare` (design section 6)
and `sweep` (the frame generator).

## Assumption for the project owner: one model, over a pipe

The design leaves open whether the test-commit model is written once or twice.
This crate takes the design's stated preference: **the Rust side answers every
libliftoff `TEST_ONLY` commit over a pipe from its own model (`model.rs`), so
the rules exist once.** The C driver has no model (see `c/PROTOCOL.md`).

## Usage

Build libliftoff at the pinned commit `c4226a7`, the fake libdrm and the C
driver; the last line printed is the driver's path (libliftoff is fetched into
`$CONNECTED_DEPS_DIR/libliftoff` or taken from the argument, never vendored):

```sh
tools/liftoff-crosscheck/build-liftoff.sh
```

Then, from `tools/liftoff-crosscheck/`:

```sh
# Probe text to xcheck (profile generic, no constraints; truncated or missing
# format lists are flagged in comments, or refused):
cargo run -- convert fixtures/rk3588.probe.txt

# Compare the frames of one file, both model settings, every frame listed;
# --trace adds each TEST_ONLY request with the model's answer and reason:
cargo run -- run fixtures/imx95-synthetic.xcheck --driver <driver> [--trace]

# Generate frames for the file's topology (its first output) and compare them;
# the file's own frames are compared too, as family `file`:
cargo run -- sweep fixtures/imx95-synthetic.xcheck --driver <driver> \
    [--out DIR] [--samples N] [--seed N]
```

`sweep` writes into `DIR/<topology>/` (default `$TMPDIR/liftoff-crosscheck-sweep`;
keep it outside the repository): one xcheck file per generated family and chunk
(`<topology>.<origin>.<family>.<chunk>.xcheck`), one single-frame file per
finding under `findings/`
(`<topology>.<origin>.<family>.<index>.s<case seed>.<model>.xcheck`), and
`findings.txt` with the full report. Each of these files replays with `run`.
The printed report has a summary table per model setting (classes, and classes
per family) and the first 20 findings per class with their frame files.

### What is compared

Per frame and per model setting (`compare.rs`, design section 6):

- `model=structural`: core with `profile generic` (explicit `scaling`,
  `max_source`, `full_output` and `group` lines of the file stay) and the
  accept-all check; libliftoff answered by the structural model.
- `model=full`: core with the file's profile and the model as its backend
  check; libliftoff answered by the full model.

libliftoff runs twice per file; a frame whose outcome differs is `unstable`.
Accepted means: core returned a plan; libliftoff applied and put every visible
layer on a plane (a partial allocation is a rejection, `placed=m/n` keeps the
count). Each side's assignment is replayed through the model (validity, not
equality). Classes: `both-accept`, `both-reject`, `core-only`, `liftoff-only`,
`invalid-assignment(core|liftoff)`, `core-error(<PlanError>)` (any error other
than `NoPlane`), `skipped(<reason>)` (core's not-well-formed refusals, the
driver's `skipped:`, `deadline`), `unstable`. Frames with alpha-0 layers are
their own class (`alpha0` on the line): core plans them without those layers,
as libliftoff drops them.

An output with id 0 is given a fresh id in the file handed to the driver only
(the driver uses the output id as CRTC id, and `CRTC_ID=0` means "no CRTC").

### Generator

`sweep.rs`, a pure function of topology, name, seed and sample count:
families `fill`, `zperm`, `fmt`, `scale`, `tie`, `alpha0`, `rand`
(`notes/frame-generator.md`), every layer inside the output, SplitMix64 with a
per-case seed, `GEN_VERSION` bumped by any change that alters frames.

## Checks

Run from the repository root:

```sh
cargo fmt --manifest-path tools/liftoff-crosscheck/Cargo.toml --check
cargo clippy --manifest-path tools/liftoff-crosscheck/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path tools/liftoff-crosscheck/Cargo.toml
```

The comparison tests use a stub libliftoff side and need no C build; the
driver test in `tests/driver.rs` builds the driver and is skipped with a note
when it cannot.
