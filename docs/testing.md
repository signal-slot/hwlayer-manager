# Test map: SoW §10 to the tests in this repository

`SoW.md` §10 ("Quality, CI and instrumentation") has six lines. This file
lists, for each, the tests that cover it, the command that runs them, and
what is not covered. Date of writing: 2026-11-28. **Read from source**: test
files, scripts and docs were read at the commit this file was added in; the
tests were not re-run for this file. **Gap** means no test covers the line.
**Not verified on hardware by these tests** means the tests run on a host,
on the fake backend or in a VM, so they say nothing about a board.

Every command runs from the repository root. `cargo test --workspace` runs
all the non-ignored tests below at once; it is the command CI uses
(`docs/ci.md`).

## Summary

| SoW §10 line | Coverage | Hardware |
| --- | --- | --- |
| 1. Unit tests: planner conflicts, scaling limits, format mismatch, z-order, missing resources, event order | covered | not needed |
| 2. Fake-backend property/fuzz tests: malformed frames, adversarial event order | covered; known bugs are `#[ignore]`d (see below) | not needed |
| 3. Hardware integration tests: startup, steady state, stop/restart, lease loss, modeset, recovery | host models only; hardware part is a **gap** | not verified on hardware by these tests |
| 4. Structured traces | covered on the fake backend; no text format, none through the C ABI | not verified on hardware by these tests |
| 5. Image comparison or camera verification of physical output | vkms CRC only; board part is a **gap** | not verified on hardware by these tests |
| 6. Performance metrics | plane assignment time on a host only; the rest is a **gap** | not verified on hardware by these tests |

## 1. Unit tests

Planner conflicts, scaling limits, format mismatch, z-order, missing
resources, event order.

| Topic | Tests | Command |
| --- | --- | --- |
| Planner decisions and explicit errors (conflicts, scaling, format mismatch, z-order, missing plane or buffer) through `Engine::validate`/`present` | `crates/composition-fake/tests/planner.rs`, `engine.rs` | `cargo test -p composition-fake --test planner --test engine` |
| Planner soundness on well-formed random frames | `crates/composition-core/tests/planner_props.rs` | `cargo test -p composition-core --test planner_props` |
| Scale limits (16.16 check in the planner), fixed containers | `crates/composition-core/tests/fixed_props.rs`; unit tests in `src/fixed.rs` | `cargo test -p composition-core --test fixed_props` |
| Capabilities construction, limits, zpos, masks | `crates/composition-core/tests/caps_props.rs`; unit test in `src/caps.rs` | `cargo test -p composition-core --test caps_props` |
| Reference scene on two topologies | `crates/composition-core/tests/m2_scene.rs` | `cargo test -p composition-core --test m2_scene` |
| Presentation state machine (submitted, latched, released), model-based | `crates/composition-core/tests/presentation_model.rs`; unit tests in `src/presentation.rs` | `cargo test -p composition-core --test presentation_model` |
| Unit tests inside the core sources (`planner.rs`, `presentation.rs`, `fixed.rs`, `caps.rs`) | `crates/composition-core/src/` | `cargo test -p composition-core --lib` |
| Event order and buffer rotation with adversarial vsync timing | `crates/composition-fake/tests/rotation.rs`, `event_order.rs` | `cargo test -p composition-fake --test rotation --test event_order` |
| DRM driver profiles (`DriverProfile::apply`), pure code | `crates/composition-drm/tests/profile.rs`; unit tests in `src/profile.rs`, `src/lib.rs` | `cargo test -p composition-drm --test profile --lib` |
| Two-plane C backend with a fake driver (needs `cc`) | `backends/twoplane/tests/twoplane.rs`, `tests/c/twoplane_test.c` | `cargo test -p composition-twoplane --test twoplane` |
| i.MX95 port of that backend | unit tests in `backends/twoplane/src/imx95.rs` | `cargo test -p composition-twoplane --lib` |

Gap: no unit test of a missing resource on a real DRM device (only on the
fake backend and in vkms).

## 2. Fake-backend property and fuzz tests

All generators are hand-written and seeded (xorshift or SplitMix64). No
crate has a property-testing dependency, and `composition-core` has no
dependencies.

| Topic | Tests | Command |
| --- | --- | --- |
| Malformed frame descriptions (zero and extreme sizes, unknown formats, unknown or removed buffers, extreme positions) on the fake backend | `crates/composition-fake/tests/malformed.rs` | `cargo test -p composition-fake --test malformed` |
| Adversarial event ordering on the fake backend (spurious, duplicated, missing latches, device loss and recovery, capability change, replaced pending submission) | `crates/composition-fake/tests/event_order.rs` | `cargo test -p composition-fake --test event_order` |
| Malformed arguments through the C ABI | `crates/composition-ffi/tests/malformed.rs` | `cargo test -p composition-ffi --test malformed` |
| Adversarial event order as a C caller sees it (C program built against the header and the static library; needs `cc` and a nested `cargo build`) | `crates/composition-ffi-staticlib/tests/event_order.rs`, `tests/c/event_order.c` | `cargo test -p composition-ffi-staticlib --test event_order` |
| Adversarial event order against the C two-plane backend | `backends/twoplane/tests/adversarial.rs`, `tests/c/adversarial_test.c` | `cargo test -p composition-twoplane --test adversarial` |
| C API smoke tests | `crates/composition-ffi-staticlib/tests/c_api.rs` | `cargo test -p composition-ffi-staticlib --test c_api` |
| Pure parsing code of `composition-drm` (IN_FORMATS and modifier blobs, property enums) | **gap**: no fuzz test | none |

## 3. Hardware integration scenarios

Startup, steady state, app stop/restart, lease loss, modeset, display
recovery.

| Topic | Tests | Command |
| --- | --- | --- |
| The engine-side logic of each scenario on the fake backend | `crates/composition-fake/tests/lifecycle.rs` | `cargo test -p composition-fake --test lifecycle` |
| The kernel DRM/KMS atomic path on vkms: TEST_ONLY, commit, latch and release, rotation, multi-plane output | `crates/composition-drm/tests/vkms.rs` (all `#[ignore = "needs root and vkms"]`; run in a throwaway QEMU VM) | `scripts/vkms-vm-test.sh` (not part of CI; needs QEMU) |
| Board scenarios | `tools/drm-harness` (`src/scenarios.rs`), `scripts/board-scenarios.sh`; plans in `docs/m1-imx95-test-plan.md`, `docs/board-scenarios.md`, `docs/linux-board-scenarios.md`, `docs/stm32mp257-board-scenarios.md` | harness unit tests: `cargo test -p composition-drm-harness`; the scenarios need a board |

Not verified on hardware by these tests: `lifecycle.rs` (the fake backend has
no real timing, scanout or driver) and vkms (a virtual driver, not a display
controller). Lease loss and display recovery against a real lease manager
and display controller are a **gap**; they are board work.

## 4. Structured traces

Traces for `validate`, `commit`, `latched`, `released`, dropped frames and
resource assignments.

| Topic | Tests | Command |
| --- | --- | --- |
| Exact entry sequence for a normal rotation, refused frames, a replaced pending submission, a latch timeout and a device loss (code in `crates/composition-core/src/trace.rs`) | `crates/composition-fake/tests/trace.rs` | `cargo test -p composition-fake --test trace` |
| Text format of trace lines for board logs | **gap**: `docs/trace-format.md` is a proposal; no sink, formatter or parser | none |
| Traces through the C ABI | **gap** | none |

Not verified on hardware by these tests.

## 5. Image comparison or camera verification

| Topic | Tests | Command |
| --- | --- | --- |
| Composed output of vkms compared by CRC with a buffer painted with the expected result | `crates/composition-drm/tests/vkms.rs` | `scripts/vkms-vm-test.sh` |
| Camera check of a physical output | `scripts/camera-check.py` (`scripts/camera-check-README.md`); no test runs it against a display | none |

Physical output of a real board (KVM or camera captures) is board evidence
in the board docs, not a test: **gap** in the suite. Not verified on
hardware by these tests.

## 6. Performance metrics

| Metric | Tests or tools | Command |
| --- | --- | --- |
| Plane assignment time | `crates/composition-fake/examples/plan_time.rs`, `plan_bench.rs`; results in `docs/host-metrics.md` (host only, "Nothing here is a hardware result") | `cargo run --release -p composition-fake --example plan_time` |
| Ready buffer to latch; end-to-end latency; dropped or late frames | **gap** as tests. `tools/drm-harness` (`stats.rs`, `record.rs`) records them on a board; no test runs it on a board | `cargo test -p composition-drm-harness` runs only its unit tests |
| CPU and memory consumption, DMA/display bandwidth, power | **gap** | none |

Not verified on hardware by these tests.

## Other checks that are not §10 lines

| What | Command |
| --- | --- |
| Header and `composition-ffi` stay in step | `scripts/check-ffi-header.sh` |
| Documentation links, evidence tags, board secrets | `scripts/check-doc-links.sh`, `scripts/check-doc-tags.sh`, `scripts/check-no-board-secrets.sh` |
| libliftoff comparison | `cargo test -p liftoff-crosscheck` |
| Arm link of the C backend (runs nothing) | `CC_KIND=clang scripts/check-arm-link.sh` |

## Known-bug tests (`#[ignore]`d)

Found with `grep -rn '#\[ignore' --include=*.rs .` at the commit of this
file. Each fails today by design; the fix removes `#[ignore]`. Other plan
items (`drm-plane-truncate`, `twoplane-write-errors`) may change this list,
so run the grep again. Run one test with
`cargo test -p <package> --test <file> -- --ignored <name>`.

| Test | File | Reason | Command |
| --- | --- | --- | --- |
| `clipping_overflows_i64_on_extreme_scale` | `crates/composition-fake/tests/malformed.rs` | planner clip overflows i64 for extreme source and destination extents | `cargo test -p composition-fake --test malformed -- --ignored clipping_overflows_i64_on_extreme_scale` |
| `inverted_zpos_range_is_unusable` | `crates/composition-core/tests/caps_props.rs` | planner accepts a `Zpos::Range` with min > max for the lowest layer | `cargo test -p composition-core --test caps_props -- --ignored inverted_zpos_range_is_unusable` |
| `unscaled_axis_must_not_be_limited` | `crates/composition-core/tests/fixed_props.rs` | an unscaled axis is checked against `max_downscale` | `cargo test -p composition-core --test fixed_props -- --ignored unscaled_axis_must_not_be_limited` |
| `init_rejects_storage_below_the_documented_size` | `crates/composition-ffi/tests/malformed.rs` | `ce_engine_init` accepts storage smaller than `CE_ENGINE_SIZE` | `cargo test -p composition-ffi --test malformed -- --ignored init_rejects_storage_below_the_documented_size` |
| `init_rejects_null_ctx` | `crates/composition-ffi/tests/malformed.rs` | `ce_engine_init` accepts a NULL ctx | `cargo test -p composition-ffi --test malformed -- --ignored init_rejects_null_ctx` |
| `flip_error_swallowed` | `backends/twoplane/tests/adversarial.rs` | `ce_twoplane.c` swallows a flip error | `cargo test -p composition-twoplane --test adversarial -- --ignored flip_error_swallowed` |
| `enable_error_swallowed` | `backends/twoplane/tests/adversarial.rs` | `ce_twoplane.c` swallows an enable error | `cargo test -p composition-twoplane --test adversarial -- --ignored enable_error_swallowed` |
| 12 tests | `crates/composition-drm/tests/vkms.rs` | not bugs: `#[ignore = "needs root and vkms"]` | `scripts/vkms-vm-test.sh` |

`crates/composition-ffi-staticlib/tests/event_order.rs` mentions
`#[ignore]`d tests in its header comment, but has none: its one test,
`c_caller_survives_adversarial_event_order`, runs every expected-pass case.
`tests/c/event_order.c` still has a `known_bug` flag for cases that run only
by name.

## How CI runs them

Read from `docs/ci.md`: workflow `ci`, job `checks`, on pull requests to
`main` and pushes to `main`, runs `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace` (this includes the C tests, which use the runner's
`cc`). Ignored tests do not run, so the known-bug tests and the vkms tests
are not in CI. It then builds `composition-core` and the static library for
`thumbv7em-none-eabihf` and cross-builds for aarch64 Linux (compile and link
only). Job `arm-backend` links the C backend for Cortex-M7 and Cortex-R5 and
runs nothing. A green run says nothing about hardware. `sh
scripts/check-all.sh` runs the same steps locally (`--lint` adds the lint
scripts).
