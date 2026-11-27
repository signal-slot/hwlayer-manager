# liftoff-crosscheck

Cross-check of the `composition-core` planner against libliftoff (WP5-04).
Design: `docs/libliftoff-crosscheck.md` (section 8 describes this layout).

This crate is **outside the root workspace** (its `Cargo.toml` has an empty
`[workspace]` table) and is **not in CI**. Recorded or synthetic topologies give
no hardware claim; every result is labelled with the `origin` of its input.

Modules: `xcheck` (the input format `xcheck 1`: reader and writer), `state`
(`PlaneState`, shared), and placeholders filled by later items: `probe`,
`model`, `core_side`, `pipe`, `compare`, `sweep`.

Run the checks from the repository root:

```sh
cargo fmt --manifest-path tools/liftoff-crosscheck/Cargo.toml --check
cargo clippy --manifest-path tools/liftoff-crosscheck/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path tools/liftoff-crosscheck/Cargo.toml
```
