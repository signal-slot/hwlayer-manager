# Finding frame files of the cross-check report

Replayable `xcheck 1` files for the differences triaged in [the cross-check report](../../../docs/liftoff-crosscheck-report.md) of 2026-11-28. Each one replays with

```sh
cargo run -- run findings/<file> --driver <driver>
```

from `tools/liftoff-crosscheck/`, the driver being the last line printed by `build-liftoff.sh` (libliftoff `c4226a7`). `tests/input_files.rs` checks that every file here parses and round-trips.

**No hardware claim.** Each file keeps the `origin` line of its input: `observed` means the topology was copied from a recorded probe dump (date in the line), not that anything ran on a board; `synthetic` files describe no board. Nothing here ran on a board, nothing is in CI, and none of it counts toward M1, M2 or M3.

Files whose name ends in `.s<seed>.<model>.xcheck` were written by `sweep` (GEN_VERSION=1, seed 0x5eed11f70ffc4226) and are copied unchanged. The others were derived by hand for a triage check; their first comment line says what was changed.

| File | Input | Verdict | Expected class on replay (structural / full) |
| --- | --- | --- | --- |
| `L1-opaque-blend-without-blend-property.xcheck` | `frames/section7/value-checks-before-test-commit.xcheck` + XR24 controls | lead L-1 | `half-alpha-one-layer`, `rot180-on-overlay`: liftoff-only / liftoff-only; `opaque-on-primary` and the `xr24-*` controls: both-accept |
| `imx95-synthetic.synthetic.scale.0000.sb9bb94abb79b5957.structural.xcheck` | `fixtures/imx95-synthetic.xcheck` | expected (S7-10) | liftoff-only / both-accept |
| `rk3588-probe.observed.scale.0000.s8b43ab541ee4c321.structural.xcheck` | RK3588 probe conversion | expected (S7-10) | liftoff-only / both-reject |
| `stm32mp257-probe.observed.scale.0000.s5173e21bf4dcdf3f.structural.xcheck` | STM32MP257 probe conversion | expected (S7-10) | liftoff-only / both-reject |
| `rk3588.observed.rand.0138.s68c4144c39ea5904.structural.xcheck` | `topologies/rk3588.xcheck` | expected (S7-10, `max_source`) | liftoff-only / both-reject |
| `rk3588-probe.observed.rand.0195.s40e9522c59a7ec79.full.xcheck` | RK3588 probe conversion | expected (S7-3) | core-only / core-only |
| `rk3588-probe.observed.rand.0195.fixed-zpos.xcheck` | the same, zpos pinned to `zpos_now` | S7-3 check | both-reject / both-reject |
| `rk3588-probe.observed.rand.0195.reordered.xcheck` | the same, layer order = planes' current zpos order | S7-3 check | both-accept / both-accept |
| `stm32mp257-probe.observed.rand.0028.s1dcb3aacd9131757.full.xcheck` | STM32MP257 probe conversion | expected (S7-3) | core-only / core-only |
| `stm32mp257-probe.observed.rand.0028.fixed-zpos.xcheck` | the same, zpos pinned to `zpos_now` | S7-3 check | both-reject / both-reject |
| `stm32mp257-probe.observed.rand.0028.reordered.xcheck` | the same, layer order = planes' current zpos order | S7-3 check | both-accept / both-accept |
| `rk3588.observed.rand.0005.s74a1119c5f6bf35c.full.xcheck` | `topologies/rk3588.xcheck` | expected (S7-3) | core-only / core-only |
| `stm32mp257.observed.rand.0015.s8570e2b48eb42e2d.full.xcheck` | `topologies/stm32mp257.xcheck` | expected (S7-3) | core-only / core-only |
| `stm32mp257.observed.rand.0018.lone-reflection.xcheck` | `topologies/stm32mp257.xcheck` | artefact, fixed | both-accept / both-accept (was invalid-assignment(core) / core-error(BackendRejected)) |
