# Mapping the engine's backend contract onto Zephyr for the i.MX95 M7

Date of the study: 2026-11-28. No device was touched and no code was changed. Nothing here is a hardware measurement.

Revision 2026-11-28 (read from source): sections 3.1, 3.2, 5, 6 and 7 were checked against `crates/composition-ffi/include/composition_engine.h`, `backends/twoplane/ce_twoplane.{h,c}`, `backends/twoplane/src/imx95.rs` and `tests/board/imx95_m7_engine/src/main.c` of this repository, and corrected where they differed. Statements that describe what the repository code does carry the file; the design question that remains is in section 7.

This file says how an i.MX95 M7 Zephyr backend would implement each operation of the engine's backend contract, what the stock Zephyr display driver allows, what needs a driver extension, how errors and latch events map onto SoW §7, and how much of `backends/twoplane` carries over. It does not settle whether the M7 may own the DPU at all; that is the question of `docs/imx95-zephyr-feasibility.md` (written by another item). Everything below assumes that question is answered in favour of the M7, and does not claim it is.

Tags: **read from source** (a file at a pinned revision, or this repository), **inferred** (reasoning from what was read, not checked by running anything). There is no **observed** row.

## Sources

| Source | Pin |
| --- | --- |
| This repository | `f30c09350ee0f8fcc68766b161bddc1c485bf142` (`crates/composition-core/src/{backend,engine,caps,types}.rs`, `crates/composition-ffi/include/composition_engine.h`, `backends/twoplane/ce_twoplane.{c,h}`, `docs/twoplane-backend.md`, `SoW.md` §5.1, §6, §7, WP3, M2) |
| Zephyr | `zephyrproject-rtos/zephyr` commit `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (`main`, committed 2026-11-27) |
| Zephyr files at that commit | `include/zephyr/drivers/display.h`, `drivers/display/display_socionext_dpu.c`, `drivers/display/Kconfig.socionext_dpu`, `dts/bindings/display/socionext,dpu.yaml`, `dts/arm/nxp/imx/nxp_imx95_m7.dtsi`, `boards/nxp/imx95_evk/imx95_evk_mimx9596_m7.dts` |

URL form: `https://github.com/zephyrproject-rtos/zephyr/blob/e53873cbd0ef45d63ef9ded1a45d10717e7c71d7/<path>`.

Limit of the Zephyr reading: the Zephyr files were read through a web fetch that returns text without line numbers, so no line numbers are quoted for them. Names and behaviour are what the fetched text says. The `_flash` board DTS, board YAML and defconfig, pinctrl files, samples and shields were not read.

## 1. The contract to implement

**Read from source.** `DisplayBackend` (`composition-core/src/backend.rs`) and, for C firmware, `ce_backend_ops` (`composition_engine.h`) have four operations:

| Operation | Contract |
| --- | --- |
| `capabilities(out)` | Fill outputs, planes, resource groups and `replace_pending` with what is usable now. `out` arrives cleared. |
| `check(frame, assignments)` | Final validation of a planned frame, no display change. `REJECTED` means "this assignment does not work, try another"; any other error aborts planning. |
| `commit(submission, frame, assignments)` | Program the frame. Success means accepted, not shown. A replaced, unlatched commit is never reported latched, except a latch that really happened before the takeover, which must be queued by the time `commit` returns. |
| `poll_event()` | Next `Latched{output, submission}`, `DeviceLost` or `CapabilitiesChanged`; must not block. |

Buffers are not part of the trait: the backend has its own import path keyed by `BufferId`. The engine tracks buffer states (writable, ready, submitted, scanning) and turns backend events into `Presented`, `Dropped`, `Released`, `LatchTimeout`, `DeviceLost`, `CapabilitiesChanged`. Time is the client's: `advance_time` plus an optional latch timeout.

## 2. What the stock Zephyr display API and DPU driver give

**Read from source** (Zephyr pin above).

`display.h`: the driver API has `write` and `get_capabilities` mandatory, and optional `blanking_on/off`, `read`, `clear`, `get_framebuffer`, brightness, contrast, `set_pixel_format`, `set_orientation`, `set_timing/get_timing`, `register_event_cb/unregister_event_cb`. `struct display_capabilities` has resolution, supported and current pixel format, orientation, `screen_info` and `supported_events`. Events exist in the API: `DISPLAY_EVENT_LINE_INT`, `DISPLAY_EVENT_VSYNC`, `DISPLAY_EVENT_FRAME_DONE`, `DISPLAY_EVENT_FIFO_UNDERFLOW`, delivered with a timestamp. `display_register_event_cb` returns `-ENOSYS` if `supported_events == 0`. There is no plane or layer API, no "present this buffer without copying and tell me when it is on screen" call, and no flip API. `struct display_buffer_descriptor` carries `pitch` in **pixels**, not bytes.

`display_socionext_dpu.c` (`socionext,dpu`, `DT_DRV_COMPAT socionext_dpu`, uses NXP MCUX SDK `fsl_dpu.h`):

- One fixed pipeline: ConstFrame0 (black) and FetchYUV0 source 0 into LayerBlend1, then ExtDst0, one display stream. One image plane over a constant colour. No second plane, no per-plane position, alpha or scaling API.
- Formats: `PIXEL_FORMAT_RGB_565`, `RGB_888`, `ARGB_8888`. Pitch is aligned to 32 bytes.
- `write(dev, x, y, desc, buf)`: if `x == y == 0`, the descriptor covers the full panel, `pitch * bytes_per_pixel` equals the driver's stride and `buf` is 32-byte aligned, the driver points the fetch unit at `buf` without copying (zero-copy). Otherwise it copies into one of `CONFIG_DPU_FB_NUM` (default 2, range 0 to 2) internal framebuffers, copying the whole previous frame first, then the rectangle. It calls `DPU_StartDisplay` on the first write, sets the fetch source address, triggers the pipeline shadow load (`DPU_TriggerPipelineShadowLoad` on ExtDst0), then **blocks on a semaphore** (`K_FOREVER`) that the ISR gives. `frame_incomplete` is ignored. No cache maintenance is done.
- Interrupts: DT `interrupts = <215 0>`. Only `kDPU_Group0ExtDst0ShadowLoadInterrupt` is enabled, through a register block the driver hardcodes (`0x4B780000`), not the DT `reg` (`0x4b400000`). No VSYNC, frame-done or underflow interrupt is enabled. `register_event_cb` is not implemented and `supported_events` stays 0.
- No clock, reset, power-domain, pinctrl or SCMI call in the driver. The SoC dtsi has an SCMI `disp_mix` power domain and `scmi_clk` provider, but the `dpu` node references neither. The `dpu` and `mipi_dsi` nodes are `status = "disabled"`; the EVK M7 board only adds labels (`zephyr_dpu`, `zephyr_mipi_dsi`), sets no timings and no `zephyr,display`. No LVDS/LDB node exists in the SoC dtsi, and the plan's target is a Toradex LVDS panel on the Verdin iMX95 (from the plan's task statement; `docs/imx95-board.md` does not mention it).

Consequence for this contract (**inferred**): the stock driver is a one-plane, blocking, copy-or-flip framebuffer path. It can back `capabilities`/`check`/`commit` for a single full-screen layer; it cannot back a latch event nor more than one plane.

## 3. Operation by operation

### 3.1 `capabilities`

| Part | Stock driver | Needs |
| --- | --- | --- |
| Output | One output, id 0. Size from `display_get_capabilities` (`x_resolution`, `y_resolution`). Refresh: `display_capabilities` has no rate, so `refresh_mhz = 0` (unknown, allowed by `OutputCaps`) unless the board's `display-timings` are read from the DT or `get_timing` is added. *Inferred*. | Optional: `get_timing`. |
| Planes | One `Primary` plane (FetchYUV0), `full_output_only = true` (the driver programs the plane at the panel size from the top left; the partial `write` path copies, it does not place a plane). *Inferred.* | A second plane (`Overlay`) needs a multi-plane driver; the stock pipeline has one fetch unit. Not claimed to exist on i.MX95 hardware. |
| Formats | `supported_pixel_formats` bits map to `CE_FORMAT_RGB565` (from `PIXEL_FORMAT_RGB_565`) and `CE_FORMAT_ARGB8888` (from `ARGB_8888`). `RGB_888` has no `CE_FORMAT_*` in `composition_engine.h` (read from source: the header defines RGB565, XRGB8888, ARGB8888 and NV12 only), so a backend built on that header alone does not report it. What the code does now (read from source): `tests/board/imx95_m7_engine/src/main.c` does report RGB888, through its own `APP_FORMAT_RGB888` (`CE_FOURCC('R','G','2','4')`, defined in that file, not in the header) next to RGB565 and ARGB8888; `backends/twoplane/ce_twoplane.c` and `backends/twoplane/src/imx95.rs` do not (they offer RGB565, XRGB8888 and ARGB8888, with XRGB8888 refused until its mapping is settled). The two i.MX 95 pieces therefore disagree; see section 7. `CE_FORMAT_XRGB8888` is not in the driver's list: report it only if a driver extension programs it; reporting it by treating it as ARGB would show the unused byte as alpha, which depends on what LayerBlend1 does with it and was not read. *Inferred.* | Format list read from `supported_pixel_formats`, as `ce_twoplane` already does through `app_format_ok`. |
| Modifier | Linear only. | none |
| Blend, alpha, transform, scaling | `blend_modes = Opaque`, `plane_alpha = false`, transform `Normal`, no scaling. ARGB8888 with `Opaque` blend means alpha ignored; whether the hardware ignores it is *not read*. | Driver/HAL reading of LayerBlend1 and FetchYUV0 to say anything else. |
| Resource groups | None known. | A DPU resource study (`dpu-zdrv-src` item) before any group is stated. |
| `replace_pending` | `false` for the stock driver: `write` blocks until the shadow load, so there is no pending commit to replace. With a non-blocking extension (3.3), `true` is correct only if a second shadow write before the trigger point simply overwrites the first, as in the AM62P case; that is *not known* for the DPU. | Decide with the extension. |
| `CapabilitiesChanged` | Never: no hotplug, no mode change after init. | none |

The stock driver would be wrapped by a capability description built from `display_get_capabilities`; no number is taken from another SoC (SoW §5.1).

### 3.2 `check`

**Inferred.** No hardware test-only commit exists on this stack. `check` is a pure software function over the assignment: exactly one layer on the primary plane, `full_output_only` satisfied (destination equals the panel, source equals the buffer), format in the driver's list, buffer record found (otherwise `CE_ERR_BACKEND_UNKNOWN_BUFFER`), buffer address 32-byte aligned and `stride_bytes == driver stride` for the zero-copy path (this is the design of `check`; otherwise `CE_ERR_BACKEND_REJECTED`, or accept and take the copy path if `DPU_FB_NUM > 0`; the choice is a policy, and copying is a visible cost the SoW §7 wants stated, not hidden). The planner treats `REJECTED` as "try another assignment", which is the right outcome for a layer the single plane cannot take. `twoplane_check` follows the same pattern (format, record, address nonzero, known buffers).

What the code does now (read from source, 2026-11-28): no code tests alignment at `check`. `ce_twoplane.c` `twoplane_check` tests the record, `pa != 0` and `app_format_ok` only. The i.MX 95 alignment rules (stride a multiple of 32, address 32-byte aligned, address within 32 bits, panel-sized buffer) exist only as `check_app_buffer` in `backends/twoplane/src/imx95.rs`, which is the body planned for `set_app_buffer`, called from `twoplane_commit`, so a refusal arrives at commit as `CE_ERR_BACKEND_REJECTED` after the engine's `check` passed. `tests/board/imx95_m7_engine/src/main.c` `fb_check` accepts every assignment on plane index 0 whose layer index is in range and never looks at buffer addresses or strides (the app registers no addresses). This contradicts the sentence above that a clean `check` is only a software function over the assignment; that sentence is the design, the code does less.

The core's `check` runs before every candidate; with no test-only hardware path, a clean `check` does **not** guarantee `commit` succeeds. `commit` re-checks, as `twoplane_commit` does, and returns `REJECTED` before writing anything.

### 3.3 `commit`

| Step | Stock driver | Gap |
| --- | --- | --- |
| Point the plane at the buffer | `display_write(dev, 0, 0, &desc, buf)` with a full-panel descriptor. Zero-copy if the alignment and pitch conditions hold; the pitch in the descriptor is in pixels, so `desc.pitch = stride_bytes / bytes_per_pixel` and the division must be exact. | Plane address by buffer, not by `write` heuristics: an extension taking `(plane, pa, stride, format)` would remove the copy path and the divisibility rule. |
| Enable/disable a plane | None; the plane is on after the first write (`DPU_StartDisplay`). `blanking_on/off` are not implemented. | A zero-layer frame ("blanks the output") needs `blanking_on` or a constant-frame-only mode. |
| Atomic shadow commit | `write` sets the fetch address and triggers `DPU_TriggerPipelineShadowLoad` for ExtDst0. The HAL calls named `...ShadowLoad` suggest double-buffered registers latched at a frame boundary, which is the shadow model `ce_twoplane` already assumes. *Inferred from the names; the reference manual was not read.* Several plane writes sharing one trigger is the property needed for overlay. | An extension that writes all plane registers and then triggers once. |
| Non-blocking | `write` blocks on the semaphore until the ExtDst0 shadow-load interrupt, `K_FOREVER`. If that interrupt never comes (interrupt not routed, display not started), the caller hangs. | Required: a call that triggers and returns, with the interrupt feeding a latch counter; and a bounded wait (`K_MSEC`) if the blocking form is kept. |
| Cache | None: neither the copy path nor the zero-copy path cleans the D-cache. | Backend or producer must clean the buffer range before commit (SoW §7 asks for this to be documented). Whether the M7 maps scanout memory cacheable is not read. |
| Which buffers | `get_framebuffer` returns the active buffer, and nothing tracks the last-used one. | The engine's buffer states already guard reuse; the backend relies on the engine, not on the driver. |

Commit with the stock driver (**inferred**): validate as in `check`; call `display_write`; on return the shadow load has completed (the semaphore was given by the ExtDst0 shadow-load interrupt), so the submission can be marked latched immediately after `display_write` returns, in `commit` or at the next `poll_event`. What that interrupt means in display terms (the start of the next frame, or earlier) is not read from the HAL or the manual; if it fires at a frame boundary then "Latched" is honest; if it fires when the shadow registers are copied it still is the hardware's latch, but a pixel may not be on the panel yet. The doc a later item writes must say which, measured, not assumed.

Ordering rule to keep (**read from source**, `backend.rs`): if a pending commit was latched before the new commit took over, queue that latch before `commit` returns. With a blocking `write` this cannot happen: each commit is complete before the next begins. With a non-blocking extension it can, and `ce_twoplane`'s `note_latch` under the interrupt lock before overwriting `pending` is the model.

### 3.4 `poll_event` and the latch

**Read from source** that the engine only learns about latches through `poll_event`, which must not block, and that `ce_twoplane` implements it with `pending`/`latched` flags guarded by `irq_lock`, fed from a vsync callback comparing `commit_count` with `latched_count`.

For the DPU:

- Stock: no callback exists. The shadow-load interrupt is consumed inside the driver by `write`. With the stock driver the backend can only synthesize `Latched` from the return of `write` (above). It cannot observe a missed latch: a missing interrupt hangs `write`, it does not time out.
- With `display_register_event_cb`: the API already defines `DISPLAY_EVENT_VSYNC` and `DISPLAY_EVENT_FRAME_DONE`, with a timestamp and `in_isr` delivery. A driver extension would set `supported_events` and enable the matching DPU interrupt (the driver enables only the ExtDst0 shadow-load one, via the hardcoded block). The backend registers a callback with `in_isr = true`, counts events, and reports `Latched` from `poll_event` exactly like `ce_twoplane`.
- Which event is "latched": a frame-done or vsync event says a frame boundary passed; it does not by itself say the shadow load for **this** commit took place. The two-plane design solves that with `commit_count`/`latched_count`; the DPU extension needs the equivalent, from the shadow-load interrupt (it fires per trigger; a counter of triggers taken against a counter of shadow-load interrupts gives the same property). *Inferred.*

The `DISPLAY_EVENT_*` API gives a timestamp in the event data. That timestamp, plus the latch timeout, is the basis for latch timing hooks in a test app; no timing is claimed here.

### 3.5 Buffers

**Read from source:** buffer import is outside the trait. `ce_twoplane` takes a `lookup(id)` port returning address, size, stride, format and sequence number; the integrator's table fills it. For the M7 the table holds scanout addresses (DDR addresses the DPU can reach and the M7 may use; this is a resource-ownership question for the feasibility doc, not decided here). `display_write` takes a pointer from the M7's view of memory; the DPU is given `(uint32_t)(uintptr_t)buf`, so the M7 address and the address the DPU bus master sees must be the same, or the driver extension must translate. *Inferred from the cast in the driver.*

## 4. Errors, and the latch, mapped onto SoW §7

| SoW §7 requirement | Mapping | Basis |
| --- | --- | --- |
| No scanout of a partially rendered buffer | Engine: only buffers finished with `finish_write` become planable. Backend: nothing to add. | read from source |
| No reuse of a displayed or pending buffer | Engine releases a buffer only after the latch that takes it off screen. With a blocking `write`, release follows the next commit's return; with callbacks, the next latch. Zero-copy `write` has no guard of its own. | read from source / inferred |
| Accepted / latched / released are distinct | commit `Ok` = accepted; `Latched` from the shadow-load interrupt; `Released` by the engine. | read from source |
| Dropped or overwritten pending frames observable | Only if the backend commits while one is pending (`replace_pending = true`): the engine emits `Dropped`. The stock blocking driver never has a pending frame. | inferred |
| VSYNC, missing interrupts | Stock: a missing shadow-load interrupt blocks `write` forever, indistinguishable from a hang. Needed: non-blocking trigger plus `ce_set_latch_timeout`/`ce_advance_time` so that `LatchTimeout` is reported once and recovery is the caller's. | read from source / inferred |
| Atomic commit failure | `display_write` returns a negative errno or asserts: the driver asserts on buffer size (`height * stride <= buf_size`) rather than returning an error, so the backend must check size itself. Map: invalid arguments before any write: `BackendError::Rejected`; `-ENOTSUP` (copy path with `FB_NUM == 0`): `Rejected`; other: `Platform(errno)`. `Busy` only when the engine commits while the backend holds an unreported latch, as `ce_twoplane` does (`CE_ERR_BACKEND_BUSY` before writing anything). | inferred |
| Hotplug, modeset, lease revocation, reset | The M7 has no lease; the display can be lost if its owner (Linux, the System Manager) takes it back or the DPU is reset. The Zephyr driver has no detection of this. `DeviceLost` would come from a device-ready check or an underflow event, if an extension supplies one. Unknown whether Linux and Zephyr can both be running with the display (feasibility doc). | inferred |
| Format, alpha, pitch, memory layout validated | In `check`/`commit` as in 3.2; premultiplication: `ce_twoplane` assumes a driver call; none exists. | inferred |
| MCU memory protection, cache, DMA visibility, producer trust | Not covered by the stock driver; a backend doc item must state the M7 cache attributes and who writes the scanout memory. | not read |
| Deterministic fallback on producer failure | Not in the backend contract; `ce_twoplane`'s safety layer is its AM62P approach and has no stock equivalent. | read from source |

## 5. Reuse of `backends/twoplane`

**Read from source:** `ce_twoplane.c` is C, freestanding, and talks to the driver through `struct ce_twoplane_hw` and to the system through `struct ce_twoplane_port` (`irq_lock/unlock`, `wake`, `lookup`, `device_ready`). Its caps are two planes: an opaque RGB565/XRGB8888/ARGB8888 application plane at zpos 0 and an ARGB8888-premultiplied safety plane at zpos 1; its `check` and `commit` reject a frame with no safety layer (`CE_ERR_BACKEND_REJECTED`). `docs/twoplane-backend.md` says the demo's firmware ran the same backend on the i.MX 95 DPU through `display_port_<soc>.c`, which is **not in this repository**; nothing about that run is evidenced here.

| Piece | Reusable on the i.MX95 M7? |
| --- | --- |
| `reached`, `note_latch`, `vsync_cb`, `poll_event`, pending/latched protocol and the "latch before takeover" ordering | Yes, unchanged, **if** the driver supplies `commit_count`, `latched_count` and a callback. The protocol is the part worth keeping. |
| `struct ce_twoplane_port` | Yes: Zephyr `irq_lock`, `irq_unlock`, `k_sem_give`, `device_is_ready`. |
| `lookup` table | Yes in kind; the table and its producer are the integrator's. |
| Capabilities and `check`/`commit` | **No, as they are.** Two planes, an obligatory safety layer, and safety-buffer calls (`safety_flip`, `safety_buffer`, `safety_front`, `safety_premultiplied`) have no counterpart in the stock DPU driver, and the stock pipeline has one plane. A single-plane backend needs its own `capabilities`, `check` and `commit` (read from source: `tests/board/imx95_m7_engine/src/main.c` has one that programs nothing: `commit` prints the plan and sets a flag, and `poll_event` reports the latch at once, so it measures no latch); the shared parts could be factored out, but that edits `backends/twoplane` and is a later item. |
| Hardware table | Only `app_format_ok`, `set_app_buffer`, `enable_app_plane`, `commit_count`, `latched_count`, `set_vsync_callback` would be wired; the safety calls would be unused, and that is where a stricter reading of "reuse" ends. |

Whether the two-plane backend ever suits the i.MX95 depends on a second plane existing. The stock driver does not offer one; whether the hardware does is not established here (SoW §5.1: no copying of i.MX8QXP/QM capabilities). Not decided in this file.

## 6. Ordered code items

1. **Decision gate (not code).** The feasibility doc's answer on whether the M7 may own the DPU, clocks, power domain `disp_mix`, display route and scanout memory under the board's boot flow. Without it no step below runs on the board. Host-only work may continue.
2. **Driver extension, step 1: latch/VSYNC events.** Enable a frame boundary interrupt, set `supported_events`, implement `register_event_cb`/`unregister_event_cb` with timestamp, and give a trigger counter and a shadow-load counter so a latch is attributable to a commit. Take the interrupt register block from the DT instead of the hardcoded address. Keep it upstreamable (Zephyr display API, no engine types).
3. **Driver extension, step 2: non-blocking, bounded present.** A way to set the plane address and trigger without `K_FOREVER`, with an error on timeout; plane address, stride and format taken from the caller, so no copy path and no pixel-pitch division. Cache maintenance rule documented.
4. **Test app (single-plane scanout).** Zephyr application under `tests/board/` (read from source, 2026-11-28: `tests/board/imx95_m7/`, `tests/board/imx95_m7_scanout/` and `tests/board/imx95_m7_scmi_probe/` exist; whether the scanout one covers every point below was not checked here): full-screen framebuffer, two buffers alternated, event callback counting, timestamps of trigger and latch, latch-timeout case with the interrupt masked. Host build only until the gate passes.
5. **Backend crate or C file for one plane.** `capabilities`, `check`, `commit`, `poll_event` over the extended driver, with a host test using a fake driver in the style of `backends/twoplane/tests/c/twoplane_test.c`. Decide whether to factor `note_latch` and the pending/latched protocol out of `ce_twoplane.c` so both backends share it; that changes `backends/twoplane`, so it is its own commit with the AM62P regression (M3) rerun on the host.
6. **Engine-linked app.** Read from source: `tests/board/imx95_m7_engine/` exists and links the engine with a backend that programs nothing (see 3.2); what remains is the same app over a real backend. The M7 app linking `libcomposition_engine.a` (the staticlib already built for `thumbv7em`), with a buffer table and a producer, showing `Presented`, `Released`, `LatchTimeout` for the single-plane case.
7. **Overlay: driver study first.** Only after steps 2 to 6 work on the board: read what the DPU offers beyond FetchYUV0 (additional fetch units, layer blends, scaler), state it with sources, and only then write multi-plane registers behind a driver extension that triggers once. Capabilities and resource groups come from that study, not from another SoC. If blocked, deliver the SoW WP3 gap report and keep the single-plane result.
8. **Linux vs Zephyr comparison (WP3).** Same scene on the same board, once steps 4 to 6 exist; the differences in format, blend and latch reporting come from rows 3.1 to 3.4 of this file, measured.

## 7. Open questions this study could not close

- Whether the ExtDst0 shadow-load interrupt marks the frame boundary or earlier (needs the reference manual or a measurement).
- Whether a second `write`/trigger before the shadow load simply overwrites the first (decides `replace_pending`).
- What LayerBlend1 does with the alpha byte of ARGB8888 and whether XRGB8888 can be added.
- The M7 cache attributes of scanout memory and whether the DPU's bus address equals the M7's address for it.
- The hardcoded `0x4B780000` register block versus the DT `reg` (`0x4b400000`): which blocks they are, and whether Linux's use of the display subsystem conflicts.
- The LVDS route: the stock SoC dtsi has `mipi_dsi` but no LVDS/LDB node, and the Verdin panel is LVDS.
- **Open design question (RGB888 and alignment).** Two things differ between the repository's i.MX 95 pieces, and nothing was changed to settle them (read from source, 2026-11-28): (a) whether the single-plane M7 backend reports RGB888: `tests/board/imx95_m7_engine/src/main.c` does, with a private FOURCC absent from `composition_engine.h`, while `ce_twoplane.c` and `imx95.rs` do not; the choice is between adding a `CE_FORMAT_RGB888` to the header (and `crates/composition-ffi/src/lib.rs` in the same commit) and not reporting the format; (b) whether buffer alignment is judged at `check` or only at `commit`: today only at commit (`twoplane_commit` through `set_app_buffer`), and not at all in the engine app. The planner treats a `check` refusal as "try another assignment", so a commit-time refusal costs the frame; the design choice is open and is not decided here.
