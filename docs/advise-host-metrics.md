# Host timing of `composition_core::advise`

This is a host measurement of the portable advisory planner. It is **not a
hardware result** and **not a comparison with libliftoff**: no libliftoff code
was run, and nothing here says the planner is faster or slower than it. It
gives the §4.1 "why not reuse libliftoff" case a host figure for `advise`
(`docs/host-metrics.md` has the figures for the planner itself).

## Record

- Date: 2026-11-28
- CPU, from `/proc/cpuinfo`: Intel(R) Core(TM) Ultra 7 155H
- rustc: `rustc 1.99.0 (b940084d7 2026-09-28)`
- Command: `cargo run -p composition-fake --release --example advise_bench`
  (default 20000 iterations per case, a warm-up of a tenth of that first)
- Source: `crates/composition-fake/examples/advise_bench.rs`

## What is timed

`composition_core::advise` with `|_, _| Verdict::Accept` as the check, so the
advisory planner only: no `TEST_ONLY` commit, no ioctl, no `DriverProfile` data
(it lives in `composition-drm`; the fake capabilities here are the example's
own 2, 4 and 8 plane controllers, the same style as `plan_time`). Scenes of 2,
4 and 8 layers (translucent ARGB, a 2x-scaled thumbnail and an NV12 video in
turn) on one 1920x1080 output. Two modes: `planes-only` (no composition
allowed) and `compose-allowed` (one ARGB8888 composition target).
`result` is `direct` (every layer on a plane), `not direct` (some layers
reported for composition, or blocked when composition is forbidden), or
`error`. Times are `std::time::Instant` around one call, in ns.

## Figures: observed on host (2026-11-28)

| planes | layers | mode | result | median ns | p99 ns |
|---|---|---|---|---|---|
| 2 | 2 | planes-only | not direct | 1543 | 1829 |
| 2 | 2 | compose-allowed | not direct | 2588 | 3098 |
| 2 | 4 | planes-only | not direct | 2354 | 2614 |
| 2 | 4 | compose-allowed | not direct | 15764 | 18433 |
| 2 | 8 | planes-only | not direct | 4713 | 5479 |
| 2 | 8 | compose-allowed | not direct | 108409 | 144867 |
| 4 | 2 | planes-only | direct | 1460 | 2083 |
| 4 | 2 | compose-allowed | direct | 1476 | 2043 |
| 4 | 4 | planes-only | not direct | 3327 | 5909 |
| 4 | 4 | compose-allowed | not direct | 5592 | 9614 |
| 4 | 8 | planes-only | not direct | 5257 | 5854 |
| 4 | 8 | compose-allowed | not direct | 99522 | 126247 |
| 8 | 2 | planes-only | direct | 1849 | 2204 |
| 8 | 2 | compose-allowed | direct | 1844 | 2240 |
| 8 | 4 | planes-only | direct | 3161 | 3714 |
| 8 | 4 | compose-allowed | direct | 3220 | 3887 |
| 8 | 8 | planes-only | error | 203364 | 301920 |
| 8 | 8 | compose-allowed | not direct | 224581 | 378177 |

The `max` column of the program output (up to about 13 ms) is dominated by
scheduler noise on a shared host and is not tabulated. Medians and p99 vary
between runs (an earlier run of the same command gave, for example, 96982 ns
median for 2 planes, 8 layers, compose-allowed); read them as an order of
magnitude, not as a specification.

## Limits of this record

- Observed on host (2026-11-28): one machine, one run per table, no pinning
  of the CPU, no isolation from other load.
- Not measured: any board (the aarch64 build was not run), the cost with a
  real `TEST_ONLY` check as `accept`, `DriverProfile` capability sets, and
  libliftoff on the same scenes. Any speed or quality advantage over
  libliftoff is therefore not measured.
