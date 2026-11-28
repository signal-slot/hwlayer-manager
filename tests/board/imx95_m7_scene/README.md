# imx95_m7_scene: the WP3 comparison scene on the i.MX95 M7 (Zephyr)

The Zephyr side of the WP3 Linux vs Zephyr comparison (`SoW.md` WP3, milestone M2). It drives the comparison scene through the composition engine on the i.MX95 Cortex-M7 under upstream Zephyr, and writes [docs/trace-format.md](../../../docs/trace-format.md) event lines (`CETRACE v=1`) on the console. `scripts/trace-to-metrics.py` turns such a log into the `RUN`/`METRIC` lines that [scripts/m2-compare-report.py](../../../scripts/m2-compare-report.py) reads. The Linux side is `crates/composition-drm/examples/m2scene.rs`.

**Status: not run on hardware.** The application has only been built on the host and run under QEMU on a different core (see "Status"). It has not been loaded on an i.MX95. Nothing in this directory is a timing result, and no log of it is a comparison result yet.

Upstream Zephyr only. Nothing here comes from the showcase.

## Build

Board target: `imx95_evk/mimx9596/m7/ddr`, the upstream i.MX95 EVK M7 target with DDR, as used by `tests/board/imx95_m7_scanout`. The DPU variant needs DDR for its frame buffers. There is no upstream Verdin iMX95 board.

Prerequisites, as for `tests/board/imx95_m7_engine`: a Zephyr workspace (`ZEPHYR_BASE`), the Zephyr SDK with `arm-zephyr-eabi` (`ZEPHYR_SDK_INSTALL_DIR`), and Rust with the `thumbv7em-none-eabihf` target. `scripts/prepare-zephyr.sh` prepares the workspace and SDK ([docs/zephyr-workspace.md](../../../docs/zephyr-workspace.md)).

From the repository root (`target/` is git-ignored):

```sh
# Default: stage A, stub latch, the DPU is not touched
west build -p always -b imx95_evk/mimx9596/m7/ddr -d target/zephyr/imx95_m7_scene tests/board/imx95_m7_scene

# Stage B (base + top): the engine refuses it on the stock driver
west build -p always -b imx95_evk/mimx9596/m7/ddr -d target/zephyr/imx95_m7_scene_b tests/board/imx95_m7_scene -- \
    -DCONFIG_IMX95_M7_SCENE_STAGE_B=y

# Drive the DPU through the stock driver (only where the M7 owns the DPU)
west build -p always -b imx95_evk/mimx9596/m7/ddr -d target/zephyr/imx95_m7_scene_dpu tests/board/imx95_m7_scene -- \
    -DCONFIG_IMX95_M7_SCENE_ENABLE_DPU=y
```

The CMake step runs `cargo build --locked -p composition-ffi-staticlib --no-default-features --features panic-handler --target thumbv7em-none-eabihf --release` into the build directory and links `libcomposition_engine.a`, exactly as `imx95_m7_engine` does. Pass `-DCOMPOSITION_ENGINE_LIB=/path/to/libcomposition_engine.a` to link a library built beforehand.

Emulation (a Cortex-M4F in QEMU, not the i.MX95; the stub latch only):

```sh
west build -p always -b mps2/an386 -d target/zephyr/imx95_m7_scene_an386 tests/board/imx95_m7_scene -- \
    -DCONFIG_IMX95_M7_SCENE_FRAMES=20
west build -d target/zephyr/imx95_m7_scene_an386 -t run
```

QEMU keeps running after `main` returns: leave it with `Ctrl+a x`, or wrap the command in `timeout`.

## The scene

The scene pinned by [crates/composition-core/tests/m2_scene.rs](../../../crates/composition-core/tests/m2_scene.rs) and [docs/m2-linux-zephyr-comparison.md](../../../docs/m2-linux-zephyr-comparison.md) section 2. `W` x `H` is 1280 x 800 with the stub (the panel size the docs assume, inferred), or the `zephyr,display` node's size with the DPU.

| Layer | Buffers (slot 0, slot 1) | Format | Source | Destination | z | Blend, plane alpha |
| --- | --- | --- | --- | --- | --- | --- |
| `base` | 1, 3 | `ARGB8888` linear | `W` x `H` | full output | 0 | opaque, `0xffff` |
| `top` (stage B only) | 2, 4 | `ARGB8888` linear | `W/3` x `H/3` | at (`W*500/1280`, `H*300/800`) | 1 | premultiplied, `CONFIG_IMX95_M7_SCENE_TOP_ALPHA` (default `0x8000`) |

Each frame presents the same picture from the other slot, as the Linux example does. A slot is presented again once the engine has released its buffers. The pixels never change, so a buffer is marked drawn (`ce_finish_write`) without being written.

`base` is opaque here, as in `m2_scene.rs` (`Layer::new`). The Linux example in review sets `base` to premultiplied; the stock Zephyr plane offers only opaque blending, so a premultiplied `base` would be refused there (`blend`). This difference is listed in the Handoff of the item that added this app.

The engine's backend describes the stock upstream DPU driver the way `tests/board/imx95_m7_engine` does: one output, one primary plane (id 1, as in `m2_scene.rs`), linear `RGB565`/`RGB888`/`ARGB8888`, opaque only, no plane alpha, no scaling, fixed zpos, full output only (read from source, not measured). So:

- Stage A plans `base` on plane 1 and commits it every frame.
- Stage B is refused: `top` finds the only plane taken, `plan.no_plane layer=1 reasons=0x400` (`in-use`). The engine never composes in software. The run then stops with `VERDICT m2scene-zephyr: BLOCKED`. This is the expected stage B result on the stock driver, not a fault of the run.

## Kconfig options

| Option | Default | Meaning |
| --- | --- | --- |
| `IMX95_M7_SCENE_STAGE_A` / `_B` | A | which scene stage to present |
| `IMX95_M7_SCENE_ENABLE_DPU` | n | drive the DPU with `display_write()` through the stock `socionext,dpu` driver; off, the driver and `CONFIG_DISPLAY` are not built |
| `IMX95_M7_SCENE_STUB_LATCH` | y when the DPU is off (not settable) | the missing piece, stubbed: the backend's commit draws nothing and reports the latch at once |
| `IMX95_M7_SCENE_FRAMES` | 300 | frames to present (the Linux example's default; the comparison plan asks for at least 1000 for latency) |
| `IMX95_M7_SCENE_TOP_ALPHA` | `0x8000` | plane alpha of `top` |
| `IMX95_M7_SCENE_TRACE_BUFFER_KB` | 512 with the DPU, else 0 | trace lines go to a RAM buffer printed after the run; 0 prints each line at once |
| `IMX95_M7_SCENE_LATCH_TIMEOUT_MS` | 1000 | the engine's latch timeout and the wait for a buffer; the comparison plan's one-second watchdog |

**Who owns the DPU.** `IMX95_M7_SCENE_ENABLE_DPU` stays off on a board where Linux owns the DPU, as on the Verdin iMX95 under its BSP ([docs/imx95-zephyr-feasibility.md](../../../docs/imx95-zephyr-feasibility.md)). The same rule and the same Kconfig override of `DISPLAY_SOCIONEXT_DPU` as in `tests/board/imx95_m7_scanout` keep the driver, and its DPU programming at boot, out of the default image.

**The DPU overlay.** `boards/imx95_evk_mimx9596_m7_ddr.overlay` enables the `dpu` node at 1280 x 800 `ARGB8888` and makes it `zephyr,display`. Its timing values are the "typ" column of the original 10.1" panel in [docs/imx95-verdin-lvds-panel.md](../../../docs/imx95-verdin-lvds-panel.md). The sync polarities are placeholders, and no LVDS route, pin or clock is set up, so on the EVK target this is not a working panel configuration. It gives the driver its mode and nothing more.

## Output

Plain lines first, then the trace, then the result:

```text
SCENE_APP imx95_m7_scene board=<target> stage=A|B frames=N latch=stub|dpu-write-return trace=live|buffered
SCENE layer=base buffer=1,3 format=AR24 src=0,0,WxH dst=0,0,WxH zorder=0 blend=opaque alpha=0xffff
SCENE layer=top ...                                   (stage B only)
SCENE_NOTE ...
CETRACE v=1 ev=hdr seq=0 src=zephyr clock=cyc32|cyc64 freq_hz=<read at run time> t0=... at_unit=same build="hwlayer-manager <rev> zephyr <rev>" board="zephyr <target> latch=<stub|dpu-write-return>"
CETRACE v=1 ev=check / validate / commit / latched / released ...
RESULT frames=N committed=C latched=L dropped=D timeouts=T trace_lost=X
VERDICT m2scene-zephyr: DONE|BLOCKED|FAIL [reason]
```

The C ABI has no trace hook, so `src/trace.c` writes the lines itself, with the field order and error codes of `crates/composition-trace/src/format.rs`:

| Line | Written when |
| --- | --- |
| `check` | in the backend's check callback, once per candidate the planner tries |
| `validate` | after `ce_present()`: `result=ok` with the accepted candidate, or the planning error. `t` is when planning ended (entry of the commit callback); `dur` is from the call to that point |
| `commit` | after `ce_present()` returns, when planning succeeded; `dur` is from the end of planning to the return |
| `latched`, `released`, `dropped`, `latch_timeout`, `device_lost`, `caps_changed` | from `ce_poll_event()`, stamped when polled. A `released` line names the submission the core's rule gives it (`engine.rs` `trace_events`) |

`t`, `at`, `dur` and `t0` are raw counts of the Zephyr system cycle counter (`k_cycle_get_64()` if the target has a 64-bit one, else `k_cycle_get_32()`), and `freq_hz` is `sys_clock_hw_cycles_per_sec()` at run time. `at` is the value last given to `ce_advance_time()`, on the same counter (`at_unit=same`).

To feed the log to the comparison scripts, add the run date as the first line, then convert:

```sh
sed -i '1i # date: YYYY-MM-DD' zephyr.log
python3 -I scripts/trace-to-metrics.py --stage A zephyr.log > zephyr.run
python3 -I scripts/m2-compare-report.py linux.run zephyr.run
```

### Limits of what a log can show

- **Stub latch.** With the DPU off, a `latched` line is not a display event, and the `hdr` board text says `latch=stub`. Such a log shows that the scene plans and that the trace pipeline works. It gives no M-1 or M-2a figure ([docs/m2-linux-zephyr-comparison.md](../../../docs/m2-linux-zephyr-comparison.md) section 5).
- **No `tev`.** The stock driver's `display_write()` blocks until the ExtDst0 shadow-load interrupt and gives no time stamp of it (read from source, `tests/board/imx95_m7_scanout/README.md`). This app writes no `tev=`. With the DPU, `latched` is stamped when it is polled, right after `ce_present()` returns. That this is close to the interrupt is inferred, not measured. Because the commit itself waits for the latch, commit-to-latch (M-2a) is not measurable with the stock driver. A driver with a latch callback and an interrupt time stamp is needed for that (`docs/imx95-m7-latch-measure.md`).
- **Clock resolution.** On `imx95_evk/mimx9596/m7/ddr` the Zephyr build selects the LPTMR system timer with `CONFIG_SYS_CLOCK_HW_CYCLES_PER_SEC=32768` (read from the generated `.config`, 2026-11-28). The `hdr` line reports the rate the running system gives. At that rate one tick is about 30.5 us, so short `dur` values round to 0 or 1. A finer counter (for example the Cortex-M7 DWT cycle counter) is not used here.
- **Console.** With `IMX95_M7_SCENE_TRACE_BUFFER_KB=0` every line is printed when it is made, so the console UART is part of the run. With a buffer, formatting still runs in the measured path; only the UART output waits until the end. A line that does not fit is lost, leaves a gap in `seq`, and the run ends with `FAIL`.
- **Ordering.** The app drains the engine's events before each frame, so the backend queue is empty when `ce_present()` starts. Lines then come in the order the core's own trace would write them (`docs/trace-format.md` section 2) for this backend. This backend never replaces a pending frame and never reports device loss, so every `dropped` line would be `reason=replaced`.
- **No final empty frame.** The Linux example commits an empty frame at the end; the stock Zephyr plane cannot be turned off, so this app does not. Its `RESULT` counts only scene frames.
- **Status codes the C ABI cannot name.** `CE_ERR_UNEXPECTED_LATCH` and `CE_ERR_INVALID_ARGUMENT` carry no trace code here (the C ABI does not give the output and submission). They are printed as `SCENE_ERROR` lines instead.

## Status

As of 2026-11-28, with upstream Zephyr `main` at `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (`v4.5.0-5218-ge53873cbd0ef`) and Zephyr SDK 1.0.1 (`arm-zephyr-eabi`):

- Done (host build): the three `west build -p always -b imx95_evk/mimx9596/m7/ddr` commands above link with no compiler or CMake warning. Default: RAM 182624 B of 42 MB. Stage B: the same. DPU: `display_socionext_dpu.c` is built, RAM 17091080 B (mostly frame buffers).
- Done (emulation, not hardware): the `mps2/an386` build under QEMU. Stage A with 20 frames ended `RESULT frames=20 committed=20 latched=20 dropped=0 timeouts=0 trace_lost=0` and `VERDICT m2scene-zephyr: DONE`. Stage B printed one `validate ... result=plan.no_plane layer=1 reasons=0x400` line and `VERDICT m2scene-zephyr: BLOCKED`. Every `CETRACE` line of both logs parsed with the strict parser of `crates/composition-trace`, with no gap in `seq`. `trace-to-metrics.py` (as in review on 2026-11-28) converted the stage A log, and `m2-compare-report.py` read the result. The stub/QEMU figures are not results and are not recorded.
- Not done: any run on hardware, any run with the DPU enabled, and any timing result.
