# Filling the two-plane backend's tables for the i.MX 95 DPU (M2-05)

Written 2026-11-28. Design study, host only. Nothing here was run on the board, and no DPU register was read or written for this note. `docs/open-issues.md` M2-05 asks what an i.MX 95 implementation of `ce_twoplane_hw` and `ce_twoplane_port` (`backends/twoplane/ce_twoplane.h`) would be. This is the answer on paper.

Tags: **read from source (ref)** means a statement checked in the file named; **inferred** means my reasoning from those sources, not checked in any source or on hardware. No row is *observed*.

## Sources pinned

| Ref | What | Commit |
| --- | --- | --- |
| Z | `zephyrproject-rtos/zephyr`, `main`, `VERSION` 4.5.99, files `drivers/display/display_socionext_dpu.c`, `drivers/display/Kconfig.socionext_dpu`, `dts/bindings/display/socionext,dpu.yaml`, `dts/arm/nxp/imx/nxp_imx95_m7.dtsi` | `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` |
| H | `hal_nxp` as pinned by Z's `west.yml`, `mcux/mcux-sdk-ng/drivers/dpu_1/fsl_dpu.{h,c}` (the one with `kDPU_FetchYuv0`, which the driver uses) and `devices/i.MX/i.MX95/periph/PERI_DISPLAY_SEERIS.h` | `1a8937f3f8dfe5c68f59cc2a0c0c0fccdb096650` |
| B | `backends/twoplane/ce_twoplane.h`, `docs/twoplane-backend.md` in this repository | HEAD `4812c86` |
| L | `docs/imx95-dpu-source-6.6.md` (Linux driver reading, units per plane) | this repository |

Deviation: the task asked for a release tag, and a fresh shallow clone was not permitted on this node. The reading was done on an existing `main` checkout of Z in the dependency cache at the commit above. `git describe` of that commit gives `v4.5.0-5218-ge53873cbd0e` (checked 2026-11-28), so it is a development commit after tag `v4.5.0`, not a release. The v4.2.0 tag was not cloned, since the driver (merged May 2026) is not expected in it; this is not checked. `platform-capabilities.md` says the driver was merged May 2026.

## What the stock Zephyr driver is (read from source, Z)

`display_socionext_dpu.c` builds one fixed pipeline in `dpu_init()`: ConstFrame0 (primary) and FetchYUV0 (secondary) into LayerBlend1, LayerBlend1 into ExtDst0, DomainBlend0, display stream 0 in primary-only mode. Its public API is `display_write`, `get_capabilities`, `get_framebuffer`, `set_pixel_format` (RGB565, RGB888, ARGB8888). `display_write` with a full-panel, 32-byte-aligned buffer points FetchYUV0 at it by `DPU_SetFetchUnitSrcBufferAddr`, calls `DPU_TriggerPipelineShadowLoad(ExtDst0)` and blocks on a semaphore that the ISR gives on `kDPU_Group0ExtDst0ShadowLoadInterrupt`. There is no second plane, no alpha, no scaling and no way to enable or disable the plane, and no vsync callback or counters. The driver hands out no register-level handle: everything below that is not the display API has to use the HAL calls directly (the HAL is a public module of the same workspace), on the same `DISPLAY_SEERIS_Type *` base.

Observations on the driver that matter for a port (read from source, Z):

- The ISR reads `APP_DPU_DC`, hard-coded `0x4B780000`, while the devicetree node is at `0x4b400000` (`nxp_imx95_m7.dtsi`). They differ. Which is the DPU's real interrupt-register base is not settled here; the port must not copy that macro (inferred: HAL register offsets are relative to the node's base).
- The DT node has `interrupts = <215 0>`, `status = "disabled"`, and DPU is present for the M7 only (Z, dtsi).
- `display_write` is synchronous: it cannot be the commit primitive of the backend, which wants non-blocking writes and a counter.

Consequence: the stock driver cannot fill `ce_twoplane_hw`. A SoC-specific driver (or a patch of the stock one) on the HAL is needed, as the rest of this note describes.

## Latch source

B calls the i.MX 95 latch a design statement, not a measurement (`docs/twoplane-backend.md`: "ExtDst0's sync busy bit on the i.MX 95"). Here is what the sources support.

- **Request**: write 1 to `EXTDST0_TRIGGER` `Sync_Trigger` (bit 0). `DPU_TriggerPipelineShadowLoad()` does this on the pipeline unit; read from source (H, `fsl_dpu.c`; register `DISPLAY_SEERIS_PIXENG_EXTDST0CFG_EXTDST0_TRIGGER_…Sync_Trigger_MASK`, H register header).
- **Busy**: `EXTDST0_STATUS` has `sync_busy`, bit 8 (mask `0x100`), defined in the register header; read from source (H). The HAL has no function that reads it. Meaning (set from trigger until the shadow registers are loaded) is inferred from the bit's name and the Linux/NXP reading of the unit; not checked in a reference manual.
- **Interrupt**: `kDPU_Group0ExtDst0ShadowLoadInterrupt`, bit 3 of group 0 (H, `fsl_dpu.h`), enabled with `DPU_EnableInterrupts(base, 0, …)`; the stock ISR uses it as "frame sent" (Z). Read from source.
- **Open**: whether the shadow load happens at the next frame start (as the backend's contract needs: "reach the panel together at a frame start") or immediately on trigger, and how the ExtDst0 load relates to DomainBlend0's and the display stream's own shadow loads, is not in anything read. **Not measured.** The Linux harness of M1 and the M2 scanout app are where it would be measured.

Proposed latch model (inferred): `commit_count` is a software counter incremented by each hook that writes plane registers and then triggers ExtDst0 shadow load; `latched_count` is a counter incremented in the ExtDst0 shadow-load interrupt (bit 3), with `sync_busy` read as a cross-check that the counter and the bit agree. If a trigger written while `sync_busy` is set is lost or merged, `latched_count` would not reach `commit_count`. The backend's contract ("reaches `commit_count` once the writes are on the panel") would then break, so the port must not trigger while busy and must count merged triggers as one. This is the first thing to measure.

## Hooks of `ce_twoplane_hw`

Unit names and enums are from H (`fsl_dpu.h`): FetchYuv0..3, FetchLayer0..1, LayerBlend1..6, ConstFrame0/1/4/5, ExtDst0/1/4/5. Plane mapping, pipeline (inferred): application plane = FetchYUV0 → LayerBlend1 as in the stock driver; safety plane = a second fetch unit (FetchYUV1 or FetchLayer0) → LayerBlend2 with LayerBlend1 as its primary → ExtDst0. LayerBlend primary-source lists in H doc comments (ConstFrame, LayerBlend, scalers) suggest chaining, but which unit sources are legal for LayerBlend2's primary was not verified here (inferred).

| Hook | i.MX 95 version | Generic or SoC-specific | Tag |
| --- | --- | --- | --- |
| `app_format_ok(format)` | Stock driver takes RGB565, RGB888, ARGB8888 (`dpu_supported_fmts`). Map `CE_FORMAT_*` to `kDPU_PixelFormat…` and to bytes per pixel. XRGB8888 would need a pixel format the stock list lacks; whether `dpu_1` has one is not checked. Linux lists ARGB8888, XRGB8888, RGB565 and more (L). A pure table lookup, no register access | SoC-specific table | read from source (Z `dpu_supported_fmts`; L); XRGB8888 mapping inferred |
| `set_app_buffer(dev, buf)` | Check width, height equal to the panel, stride a multiple of 32 (`DPU_FB_PITCH_ALIGN`), address 32-byte aligned; refuse (nonzero) otherwise. Then `DPU_SetFetchUnitSrcBufferConfig(base, kDPU_FetchYuv0, 0, &sb)` with `baseAddr`, `strideBytes`, `pixelFormat`, `bitsPerPixel`, or only `DPU_SetFetchUnitSrcBufferAddr` when format and stride are unchanged (what the stock driver does), then `DPU_TriggerPipelineShadowLoad(kDPU_PipelineExtDst0)`, `commit_count++`. Stock `display_write` cannot do this: it blocks, copies for non-aligned input, and has no counter. `pa` is a 32-bit address in the stock driver (`(uint32_t)(uintptr_t)`); the M7 sees DDR through its own window, so whether `ce_twoplane_buf.pa` is the DPU's bus address or the M7's CPU address must be settled (`m7-fb-cache` item, not read here) | Call sequence is SoC-specific; the checks are generic | read from source (Z `dpu_write`, H); address question inferred |
| `enable_app_plane(dev, on)` | `DPU_EnableLayerBlend(base, kDPU_LayerBlend1, on)`: H says that disabled, the primary input is output (ConstFrame0, black), which hides the plane. Alternative: `DPU_EnableFetchUnitSrcBuffer(base, kDPU_FetchYuv0, 0, on)`. Then trigger and count one commit. Not in the stock driver | SoC-specific | LayerBlend disable semantics read from source (H `fsl_dpu.c` doc); choice between the two inferred |
| `safety_flip(dev)` | Write the address of the back safety buffer into the second fetch unit's source buffer (`DPU_SetFetchUnitSrcBufferAddr`), swap the front/back pointers, trigger ExtDst0 shadow load, `commit_count++`. The backend only needs "presented at the next frame start" | SoC-specific | inferred (the API calls are read from source; the second unit is not in the stock driver) |
| `safety_buffer(dev, &w, &h, &stride)` | Not hardware. The driver owns two ARGB8888 panel-sized buffers in non-cached or explicitly flushed memory (stock driver's `dpu_frame_buffer_<n>`, 32-byte aligned, shows the pattern). Returns the one that is not on the plane, the panel size from DT `width`/`height`, stride = `ROUND_UP(w*4, 32)`. Cache rules on the M7 are for `m7-fb-cache` | Generic logic; sizes from DT | read from source (Z, buffer declaration); rest inferred |
| `safety_front(dev)` | Not hardware. The buffer the second unit was programmed with in init. Per the header it is the one scanned at start-up | Generic | inferred |
| `safety_premultiplied(dev, on)` | LayerBlend2 blend modes: premultiplied: secondary colour `kDPU_BlendOne`, primary colour `kDPU_BlendSecAlphaInv`; non-premultiplied: secondary colour `kDPU_BlendSecAlpha`, primary `kDPU_BlendSecAlphaInv`. `DPU_SetLayerBlendConfig(base, kDPU_LayerBlend2, &cfg)` (BLENDCONTROL register), then trigger; commit counts only if it wrote. The mode names exist in H (`dpu_blend_mode_t`); the stock driver sets one fixed mode (secondary One, primary Zero, i.e. opaque replace) | SoC-specific | enum and function read from source (H); the formula is inferred |
| `commit_count(dev)` | A 32-bit software counter (atomic or incremented under the driver's own interrupt-safe lock), bumped by each hook above after its writes and trigger | Generic | inferred |
| `latched_count(dev)` | A 32-bit counter incremented in the ISR on `kDPU_Group0ExtDst0ShadowLoadInterrupt`. Cross-check by `sync_busy` (EXTDST0 STATUS bit 8). Which interrupt, if any, fires at the frame start is open. FrameGen0 interrupts 0..3 (`kDPU_Group0FrameGen0Int0..3Interrupt`, H) exist and are the alternative source of "frame start", configured by FrameGen compare registers; not examined | SoC-specific | read from source (interrupt bit, busy bit); the counting scheme inferred; timing **not measured** |
| `set_vsync_callback(dev, cb, user)` | Store `cb`, `user` in driver data; the ISR calls `cb(dev, user)` once a frame. A once-a-frame source must exist: the ExtDst0 shadow-load interrupt fires only after a trigger (stock driver uses it that way), so if frames pass without commits it does not fire. The backend wants a call at every vsync (`wake` at every vsync). That wants a per-frame interrupt, e.g. a FrameGen0 interrupt (inferred) or `kDPU_Group0Display0…` frame-complete group bits (name pattern in H; not checked on this SoC) | SoC-specific | read from source (interrupt list); fit to the contract inferred |

Interrupt delivery (inferred, SoC-specific): the DT node has one `interrupts = <215 0>` for the DPU (Z, dtsi); the stock driver ISR reads the group 0 pending flags and clears them (`DPU_GetInterruptsPendingFlags`, `DPU_ClearInterruptsPendingFlags`). One ISR would serve all bits; it must dispatch latch counting and vsync callback separately.

## Members of `ce_twoplane_port`

These are system hooks. None touches the DPU. Zephyr is the integrator's system here (read from source: B names the Zephyr call for each).

| Member | i.MX 95 M7 under Zephyr | Generic or SoC-specific | Tag |
| --- | --- | --- | --- |
| `irq_lock` | `irq_lock()` of Zephyr, returns the key. The Cortex-M7 masks all interrupts, including the DPU's, so the vsync callback cannot run | Generic Zephyr | read from source (B, "Zephyr equivalent in the demo") |
| `irq_unlock(key)` | `irq_unlock(key)` | Generic | read from source (B) |
| `wake`, `wake_arg` | `k_sem_give` on a semaphore of the thread that polls the engine, `wake_arg` the semaphore. Called from the ISR (allowed for `k_sem_give`) | Generic | read from source (B) |
| `lookup`, `lookup_arg` | The integrator's table of registered buffers by `ce_layer.buffer` id, returning id, seq, `pa`, size, stride, format. Needs no DPU. On the M7 the table lives in M7 RAM; a producer (A55 side or an M7 task) fills it. Nothing exists in this repository (M2-05 first half). Whether `pa` is the DPU-visible address is the same question as for `set_app_buffer` | Generic mechanism; the content depends on the producer | read from source (B "Also missing"); address question inferred |
| `device_ready(dev)` | `device_is_ready(dev)` on the `socionext,dpu` device (or the new driver's). If the SM leaves the DPU's clocks and power domain to Linux, the driver cannot come up: `dpu_init` would touch registers of an unclocked block. That ownership question is `imx95-doc`'s, not settled here | Generic call; result SoC-specific | call read from source (B); ownership dependence inferred |

## What is generic and what is SoC-specific

- Generic (same code on AM62P and i.MX 95, hence in the backend or the port): the whole `ce_twoplane_port`; the counters; `safety_front`, most of `safety_buffer`; the rule "one hook that writes = one commit".
- SoC-specific (a `display_port_imx95.c`): every register sequence, the format table, blend modes, the choice of fetch units and LayerBlends, the interrupt bits for latch and vsync. The AM62P equivalent uses the GO bit (B); there is no code sharing below the table.
- Blocked in the stock driver: second plane (single FetchYUV0, no safety layer), per-plane enable, blend control, non-blocking write, counters, per-frame callback, correct register base for the ISR. All have to be new code against the HAL. Only upstream Zephyr plus the HAL are in scope of this note, so this is a driver written for this project, not something to claim as stock Zephyr behaviour.

## What is still unknown (report, not a change)

1. Which core owns the DPU, clocks and power domain under the board's boot flow: `imx95-src` / `imx95-doc`. Every row above assumes the M7 can program the DPU.
2. Whether ExtDst0 shadow load happens at a frame start, and the latency of `sync_busy`.
3. Whether a chain FetchYUV0→LayerBlend1→LayerBlend2→ExtDst0 is a legal route and whether a second fetch unit can read ARGB8888 (not just YUV-named units: the stock driver already feeds RGB through FetchYUV0, read from source Z).
4. Scanout memory address as the DPU sees it, and cache maintenance: `m7-fb-cache`.
5. Which per-frame interrupt to use for `set_vsync_callback`.

The two-plane backend itself needs no change for any of this (B is unchanged by this note).
