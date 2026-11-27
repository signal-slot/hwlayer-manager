# Host demo

> **This is a host run. It is not a hardware result.** It claims nothing about
> any board, display controller, panel, timing or milestone (M1, M2, M3). It
> shows only that the portable engine builds and behaves as its host tests
> say, on the machine that ran it. Evidence tags follow
> [CONTRIBUTING.md](../CONTRIBUTING.md): everything the script prints is
> *observed* on that host on the printed date, and nothing else.

`scripts/host-demo.sh` is the WP5 reproducible demo that needs no device
(SoW.md, WP5). It uses only what already exists in the repository and changes
no crate.

## What it shows

1. `cargo build --locked --workspace`: the whole workspace builds.
2. Scenes composed against the fake backend (`composition-fake`), as
   tests: `advise_scenes` (planning scenes), `engine` (the engine driving a
   frame through validate and commit) and `event_order` (the order of the
   events it emits). The fake backend is software; no display is involved.
3. The `plan_time` example of `composition-fake` with 50 iterations: the
   first lines of its table of `Engine::validate` times. These are host
   timings of the planner and the fake backend's check, measured once on the
   printed date. They are not board numbers.
4. The two `thumbv7em-none-eabihf` builds of [docs/ci.md](ci.md):
   `composition-core`, and `composition-ffi-staticlib` without `std`, with
   `panic-handler`, in release. They compile and link; nothing is run.

It does not run clippy, rustfmt, the aarch64 cross builds or the C tests
beyond what `cargo test` of the steps above covers; `scripts/check-all.sh`
mirrors the CI for that.

## How to run it

Needs a Rust toolchain and the target
(`rustup target add thumbv7em-none-eabihf`). Nothing is installed by the
script, which stops with that command when the target is missing.

```sh
bash scripts/host-demo.sh
```

It writes only to the cargo target directory and a temporary directory it
removes on exit. It exits 0 when every step passed and stops at the first
failure (non-zero).

## Expected output shape

```
host demo: HOST RUN, NOT A HARDWARE RESULT
date:  <UTC timestamp>
rev:   <12-hex git revision>[ (+uncommitted changes)]
rustc: <version>

==> build workspace: cargo build ...
PASS  build workspace
==> scene tests: advise_scenes: ...      (cargo test output: "test result: ok.")
PASS  scene tests: advise_scenes
... same for engine and event_order
==> example plan_time (50 iterations, first lines; ...)
<header and 7 table rows: capabilities, layers, mix, result, min/median/p99/max ns>
... (<N> lines in all)
PASS  example plan_time
==> build composition-core (thumbv7em-none-eabihf): ...
PASS  build composition-core (thumbv7em-none-eabihf)
==> build composition-ffi-staticlib (...): ...
PASS  build composition-ffi-staticlib (...)
----
summary: 7 steps passed, 0 failed
date:    <UTC date>
rev:     <revision>
This was a host run. It says nothing about any board or display.
```

The timing values differ on every run and machine; only the shape is stable.
Test counts and the step list change when tests are added; the summary count
follows the script.

## What it does not show

- No DRM device, atomic commit, VSYNC or scanout: those need a board and are
  recorded in the board docs, with their own dates, not here.
- No claim toward M1, M2 or M3 and no Linux versus Zephyr comparison.
- No comparison of the printed timings with any board or with another host.
