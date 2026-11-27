# Board test applications

Board tests in this repository are Zephyr applications (or small programs)
kept in the repository, one directory per app. See
[CONTRIBUTING.md](../../CONTRIBUTING.md) for the rule and
[docs/board-tests.md](../../docs/board-tests.md) for how to write and run one.

## Apps

Status is quoted from each app's own README. None of these READMEs records a
run on hardware.

| Directory | Purpose | Milestone item | What its README says about hardware runs |
|---|---|---|---|
| [imx95_m7](imx95_m7/README.md) | Hello and optional read-only DPU register probe for the i.MX95 Cortex-M7; can announce an rpmsg endpoint. | M2 (WP3), the first board program | "built on the host only. Not run on any board." |
| [imx95_m7_scanout](imx95_m7_scanout/README.md) | Framebuffer scanout through Zephyr's display API, with per-frame timing of `display_write`; the DPU stays untouched unless `CONFIG_IMX95_M7_SCANOUT_ENABLE_DPU=y`. | M2 first hardware step (SoW §5.1, WP3) | "nothing here has been run on hardware"; all timing and scanout results are not measured. |
| [imx95_m7_engine](imx95_m7_engine/README.md) | Links the engine's C ABI static library and prints plans for five fixed scenes against a backend that describes the stock Zephyr DPU driver. It does not access the display. | M2, engine on the M7 toolchain and memory budget | "Not done: any run on hardware." An emulation run on a different core (`mps2/an386`) is recorded there, not a hardware run. |

Apps added later get a row in this table, with a link to their README.

## Build

Every app builds with `west build` in a Zephyr workspace prepared as
described in [docs/zephyr-workspace.md](../../docs/zephyr-workspace.md).
[scripts/build-zephyr-apps.sh](../../scripts/build-zephyr-apps.sh) builds the
apps in this directory. Each app's README gives its board target, pinned
upstream versions and any extra options (for example the Rust target the
engine app needs).
