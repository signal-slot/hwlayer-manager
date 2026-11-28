# WP5 evidence checklist: what the SoW asks for, and what exists

Written 2026-11-28. A checklist for the WP5 demo and benchmark evidence. For each SoW milestone (M1 to M3) and for WP4 and WP5 (`SoW.md` §8 and §9) it lists the evidence the SoW asks for: a dated board run, a log, a capture, a comparison table. Next to each item it says what exists in the repository, as a path, or **not measured**.

This file adds no result. Every status is taken from an existing document and named in the row: [`sow-traceability.md`](sow-traceability.md), [`m1-imx95-report.md`](m1-imx95-report.md), [`imx95-zephyr-feasibility.md`](imx95-zephyr-feasibility.md), [`host-demo.md`](host-demo.md) and [`host-metrics.md`](host-metrics.md). Where this file and one of them disagree, that document is right. The plan for how the evidence is to be produced is [`wp5-evidence-plan.md`](wp5-evidence-plan.md).

Status values:

- **observed &lt;date&gt;**: a document of the repository records a board observation made on that date.
- **host only**: checked on a development host only (build, host tests, CI, vkms, QEMU, desk reading). **Not a hardware result.**
- **not measured**: no document records a result.

Host-demo and host-metrics figures are always **host only**. They are never entered in a column of board evidence and never compared with board figures.

## M1: i.MX95 Linux DRM / AGL hardware composition

Results are in [`m1-imx95-report.md`](m1-imx95-report.md) (board runs of 2026-11-27, topology probe of 2026-11-28, Verdin iMX95 on a Mallow carrier, not an NXP EVK). The report states its own verdicts; this file does not restate them.

| # | Evidence the SoW asks for | What exists | Status |
| --- | --- | --- | --- |
| M1-1 | Build log: `no_std` core, host tests, Linux i.MX95 build | CI definition and its limits in [`ci.md`](ci.md); report "Condition 1"; the aarch64 harness build ran on the board per the report; host demo of the two `thumbv7em-none-eabihf` builds in [`host-demo.md`](host-demo.md) | host only (CI, host demo); observed 2026-11-27 (aarch64 harness ran on the board) |
| M1-2 | Topology and plane restrictions captured from the board | [`imx95-probe-2026-11-28.txt`](imx95-probe-2026-11-28.txt); [`platform-capabilities.md`](platform-capabilities.md); report "Condition 2"; driver limits KMS cannot show: [`imx95-dpu-source-6.6.md`](imx95-dpu-source-6.6.md) | observed 2026-11-28 (probe); scaler and fetch-unit limits read from source |
| M1-3 | Capture of two distinct planes, z-order and alpha, not flattened | Logs [`m1-imx95/logs/two-planes.log`](m1-imx95/logs/two-planes.log), [`m1-imx95/logs/zorder-alpha.log`](m1-imx95/logs/zorder-alpha.log); pictures in [`m1-imx95/pictures/`](m1-imx95/pictures); camera checks [`m1-imx95/checks/results.txt`](m1-imx95/checks/results.txt); run notes [`m1-imx95/master-run.md`](m1-imx95/master-run.md). Flattened-output control run: not measured | observed 2026-11-27; `--flatten` control: not measured |
| M1-4 | Capability detection, planner validation and `TEST_ONLY` told apart | [`m1-imx95/logs/combos.log`](m1-imx95/logs/combos.log); report "Condition 4". A kernel `TEST_ONLY` refusal: not measured (listed as an open item in the report) | observed 2026-11-27 (planner refusals, `TEST_ONLY` acceptance); kernel refusal: not measured |
| M1-5 | Submission, latch and release separately observable; rotation without unsafe overwrite | [`m1-imx95/logs/rotate-b3.log`](m1-imx95/logs/rotate-b3.log); report "Condition 5". Two-buffer run, content read-back, kernel vblank timestamp: not measured | observed 2026-11-27 (3 buffers); the rest not measured |
| M1-6 | AGL leased-output run without a Weston fork, or a recorded blocker | [`m1-imx95/lease-run.md`](m1-imx95/lease-run.md); logs in [`m1-imx95/lease-logs/`](m1-imx95/lease-logs); [`dlm-aarch64.md`](dlm-aarch64.md); [`weston-imx-lease.md`](weston-imx-lease.md) (source read). Weston coexistence and daemon-side revocation: not measured | observed 2026-11-27 (Weston stopped); Weston coexistence not measured |
| M1-7 | Test report: board revision, panel, BSP, lease-manager version, resolution, plane assignments, errors, frame loss, latency, CPU, memory | [`m1-imx95-report.md`](m1-imx95-report.md) "Condition 7 fields". Module and carrier PCB revisions, drm-lease-manager CPU and memory, CMA during a run, kernel-timestamp latency, input-to-photon latency: not measured | observed 2026-11-27 (fields as given); the listed items not measured |
| M1 | Benchmark figures of the Linux harness (frame loss, latency, CPU, memory) for WP5 | The same report section. They come from harness runs, not from a clean-checkout benchmark package | observed 2026-11-27; no benchmark package: not measured |

## M2: i.MX95 Zephyr composition

No board result exists. M2 is open ([`sow-traceability.md`](sow-traceability.md), M2 section). The feasibility document records the remoteproc gate as not met and the M7 run as skipped on 2026-11-28, not failed ([`imx95-zephyr-feasibility.md`](imx95-zephyr-feasibility.md)).

| # | Evidence the SoW asks for | What exists | Status |
| --- | --- | --- | --- |
| M2-a | Working target display: a scanout log from the M7 | None. Host builds only under [`../tests/board/imx95_m7_scanout/`](../tests/board/imx95_m7_scanout) and [`../tests/board/imx95_m7/`](../tests/board/imx95_m7); the reduced-scope spec [`imx95-m7-reduced-scope.md`](imx95-m7-reduced-scope.md) has nothing run | not measured |
| M2-b | Explicit ownership model | [`imx95-zephyr-feasibility.md`](imx95-zephyr-feasibility.md); [`imx95-zephyr/observed-2026-11-27.txt`](imx95-zephyr/observed-2026-11-27.txt) (what Linux holds, a Linux-side snapshot, not an ownership proof); [`imx95-sm-display-lm.md`](imx95-sm-display-lm.md) (inferred design); [`imx95-display-handover.md`](imx95-display-handover.md) (design) | observed 2026-11-27 (Linux-side snapshot); host only (source, design); ownership proof: not measured |
| M2-c | VSYNC and latch trace from the M7, consistent with M1 semantics | Designs [`imx95-m7-latch-measure.md`](imx95-m7-latch-measure.md), [`imx95-m7-backend-contract.md`](imx95-m7-backend-contract.md); trace line format proposal [`trace-format.md`](trace-format.md); QEMU runs `tests/zephyr/qemu_engine/` and `tests/zephyr/display_dummy/` are not hardware | host only; M7 VSYNC and latch: not measured |
| M2-d | Hardware overlay capture, if supported | Design only: [`imx95-m7-overlay-design.md`](imx95-m7-overlay-design.md) | not measured |
| M2-e | Documented limitations for unsupported properties | [`platform-capabilities.md`](platform-capabilities.md) "i.MX95, Zephyr"; [`open-issues.md`](open-issues.md) | host only (source) |
| M2-f | If blocked: a reproducible investigation report and a runnable reduced-scope bring-up | Investigation: [`imx95-zephyr-feasibility.md`](imx95-zephyr-feasibility.md) (not concluded). Bring-up: spec in [`imx95-m7-reduced-scope.md`](imx95-m7-reduced-scope.md), nothing run | not concluded |
| M2-g | Linux vs Zephyr comparison table of the same scene on the same board | Plan with no measurement: [`m2-comparison-plan.md`](m2-comparison-plan.md); design [`imx95-linux-zephyr-compare.md`](imx95-linux-zephyr-compare.md); comparison [`m2-linux-zephyr-comparison.md`](m2-linux-zephyr-comparison.md); table builder `scripts/m2-compare-report.py`; trace converter `scripts/trace-to-metrics.py`; scene test `crates/composition-core/tests/m2_scene.rs` (host). Linux side of the scene was run for M1 only in other scenarios | Zephyr side: not measured; tools and scene test host only |

## M3: AM62P portability regression

No document records an AM62P board observation ([`sow-traceability.md`](sow-traceability.md)). The board run is issue #12, queued ([`open-issues.md`](open-issues.md)).

| # | Evidence the SoW asks for | What exists | Status |
| --- | --- | --- | --- |
| M3-a | Two-plane composition run on the AM62P | Host tests of `backends/twoplane` with a fake driver ([`twoplane-backend.md`](twoplane-backend.md)) | host only; board: not measured |
| M3-b | VSYNC latch tracking log | `backends/twoplane/tests/adversarial.rs` (host); "What the host test cannot show" in [`twoplane-backend.md`](twoplane-backend.md) | host only; board: not measured |
| M3-c | Safe frame release evidence | Host tests `crates/composition-fake/tests/rotation.rs`, `lifecycle.rs` | host only; board: not measured |
| M3-d | No visible intermediate-state regression (safety layer, producer failure), with a capture | None. [`../scripts/camera-check.py`](../scripts/camera-check.py) has not been used on the AM62P | not measured |
| M3-e | Difference matrix versus i.MX95 | [`am62p-imx95-matrix.md`](am62p-imx95-matrix.md) and [`wp4-difference-matrix.md`](wp4-difference-matrix.md), both from recorded documents with no AM62P board data | host only (desk); AM62P column: not measured |

## WP4: AM62P integration and portability

| Deliverable | What exists | Status |
| --- | --- | --- |
| AM62P adapter | `backends/twoplane`, [`twoplane-backend.md`](twoplane-backend.md) | host only |
| Regression report | None; issue #12 | not measured |
| Cross-SoC portability report | [`am62p-imx95-matrix.md`](am62p-imx95-matrix.md), [`wp4-difference-matrix.md`](wp4-difference-matrix.md), [`imx95-twoplane-port.md`](imx95-twoplane-port.md) | host only (desk) |
| Optional third-target prototype | None ([`open-issues.md`](open-issues.md) M3-06) | not measured |

## WP5: demo, benchmark evidence, documentation, licensing

| Deliverable | What exists | Status |
| --- | --- | --- |
| Reproducible demo | Host demo script `scripts/host-demo.sh` and [`host-demo.md`](host-demo.md): builds the workspace, runs fake-backend scenes, builds for `thumbv7em-none-eabihf`. **Not a hardware result.** A clean-checkout board demo does not exist | host only; board demo: not measured |
| Benchmark evidence, host | [`host-metrics.md`](host-metrics.md): plane assignment time on a development host, 2026-11-28, with `crates/composition-fake/examples/plan_time.rs`. **Not a hardware result**; never compared with a board figure | host only |
| Benchmark evidence, board | Harness figures in [`m1-imx95-report.md`](m1-imx95-report.md) "Condition 7 fields" (2026-11-27). Plane assignment time on a board, M7 frame interval, M7 submit-to-latch, CPU and memory on the M7, display bandwidth and power: not measured ([`wp5-evidence-plan.md`](wp5-evidence-plan.md) §3) | observed 2026-11-27 (Linux harness only); the rest not measured |
| Gap analysis | [`wp5-gap-analysis.md`](wp5-gap-analysis.md), a desk analysis | host only (desk) |
| Upstreaming proposal | None. See `W5-08` in [`wp5-gap-analysis.md`](wp5-gap-analysis.md) | not measured |
| Minimal documentation | [`../README.md`](../README.md), [`glossary.md`](glossary.md), [`architecture.md`](architecture.md), [`testing.md`](testing.md), [`ci.md`](ci.md) | host only |
| Project licensing | [`../LICENSE`](../LICENSE), [`../NOTICE`](../NOTICE), [`third-party-licences.md`](third-party-licences.md) | host only (desk record) |
| Contribution guidelines | [`../CONTRIBUTING.md`](../CONTRIBUTING.md), [`../SECURITY.md`](../SECURITY.md) | host only |

## Rules for adding a row's result

- A board result is entered only after the document that records it exists, with the date that document gives. Name the document in the row.
- Host demo, host metrics, CI, vkms and QEMU results stay **host only**. They never raise a board row.
- A figure from one board is not used for another, and M7 and A55 timestamps are never subtracted ([`wp5-evidence-plan.md`](wp5-evidence-plan.md) §3).
- If an item cannot be measured on the board, write **not measured**, not an estimate.
