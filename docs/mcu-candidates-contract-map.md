# MCU candidates against the composition-core model and the twoplane contract (SoW §5.4, M3-06)

Date: 2026-11-28. This is a desk document. **No device was used, none exists on the farm for these three targets, and no cell below comes from a board run.** It maps three display topologies, written up in earlier docs, onto two things this repository already has: the `Capabilities` model of `composition-core` and the C hook contract of `backends/twoplane`. No code was written and no existing file was edited. It counts toward none of M1, M2 or M3.

## Tags and sources

Each row or cell carries one of two tags.

- **S** = *read from source*. The cited document says it, and that document cites the repository, commit and line. A mapping row is **S** only when both sides are plain reads: a fact in the source doc and a fact in `caps.rs`, `ce_twoplane.h` or `ce_twoplane.c`.
- **I** = *inferred*. It is a conclusion from the rows read, or a statement that needs a hardware manual that no source doc had. Every **I** is a candidate for a later re-read, not a finding about silicon.

Abbreviations of the documents (all in `docs/` unless noted):

| Short | Document | What it holds |
|---|---|---|
| RT | [`rt1170-lcdifv2-source.md`](rt1170-lcdifv2-source.md) | i.MX RT1170 eLCDIF, LCDIFv2, PXP. Zephyr `bc833f46…`, hal_nxp `eaffbf4e…`. No Reference Manual was read. |
| RA | [`ra8p1-glcdc-source.md`](ra8p1-glcdc-source.md) | RA8P1 and RA8D1 GLCDC, DRW. Zephyr `bc833f46…`, hal_renesas `31e99f1c…`. No manual was read. |
| H7 | [`stm32h7-ltdc-source.md`](stm32h7-ltdc-source.md) | STM32H7 LTDC, DMA2D. Zephyr `bc833f46…`, ST HAL `23147a43…`. No RM0433 was read. |
| MX | [`am62p-imx95-matrix.md`](am62p-imx95-matrix.md) | The i.MX95 DPU and AM62P DSS columns. Every cell there is a recorded fact, or "not recorded". |
| TP | [`twoplane-backend.md`](twoplane-backend.md) | The twoplane hook tables. |
| CAPS | `crates/composition-core/src/caps.rs`, `types.rs`, `backend.rs` | The model. |
| HDR | `backends/twoplane/ce_twoplane.h` and `ce_twoplane.c` | The C contract. |

"§n" after RT, RA, H7 or MX is a section number of that document. Sections of this file are numbered separately.

Nothing here quotes third-party code. All hardware facts are second-hand: they come from the three source docs, with the limits those docs state. Where a source doc rests on a row that its own author flagged as unchecked (RA: rows marked `~`; H7: the cmsis and `hal_stm32` copies; RT: all register-level facts), that limit carries over to this file.

## 1. What the model can say (the yardstick)

All **S**, from CAPS and HDR.

- `Capabilities` holds `outputs` (id, size, refresh), `planes`, `resource_groups`, and one `replace_pending` bool for the whole backend.
- `PlaneCaps` holds: `kind` (Primary, Overlay, Cursor); `outputs` mask; `formats` as (FourCC, modifier) pairs; `scaling` (`None` or up and down limits); `max_source`; `plane_alpha` (one bool); `blend_modes` mask over Opaque, Premultiplied, Coverage; `transforms` mask over Normal, Rotate90/180/270, FlipH, FlipV; `zpos` (`Fixed` or `Range`); `full_output_only`.
- `ResourceGroup` is a count of units (`capacity`) consumed by layers with given traits (`ANY`, `SCALED_H/V`, `YUV`) on a set of member planes.
- The backend reports one event for a latch: `BackendEvent::Latched {output, submission}`, plus `DeviceLost` and `CapabilitiesChanged`.
- `PixelFormat` is a DRM FourCC. `types.rs` names RGB565, XRGB8888, ARGB8888, XBGR8888, ABGR8888, NV12, NV21, NV16, YUYV, UYVY and YU12. Any other code is allowed as a number, but the planner treats an unknown format as if it might carry alpha.

The twoplane backend (HDR) hard-codes this shape: one output, two planes with ids 0 and 1, both `full_output_only`, both Normal transform, no scaling, no plane alpha, zpos 0 and 1. Plane 0 is Primary, Opaque, formats RGB565, XRGB8888, ARGB8888 filtered by `app_format_ok`. Plane 1 is Overlay, Premultiplied, ARGB8888 only. `replace_pending` is 1: "writes before a latch ride on the pending one: the newer values win". The safety plane can never be switched off. So the twoplane contract is narrower than the model; a target fits it when two full-panel planes of that kind exist.

## 2. Per-target mapping to `Capabilities`

### 2.1 i.MX RT1170, LCDIFv2 (the intended target)

| Model field | Value to report | Basis | Tag |
|---|---|---|---|
| outputs | One output. Zephyr's config picks one controller per DSI output (RT §1). Size and refresh come from the panel's `display-timings`. | RT §1, §7 | S |
| planes, count | Up to 8 (`FSL_FEATURE_LCDIFV2_LAYER_COUNT 8`). A backend may report fewer. | RT §2.2 | S |
| `kind` | Layer 0 Primary, the others Overlay. No cursor plane. | RT §2.2 (no cursor feature seen) | I |
| `formats` | RGB565, XRGB8888 / ARGB8888, ABGR8888, packed YUV 4:2:2 (UYVY, YUYV), RGB888. ARGB1555, ARGB4444 and CLUT indexes have no FourCC in `types.rs`. | RT §2.2 | S (HAL list); FourCC choice I |
| `modifiers` | Linear only. No tiling control in the HAL. | RT §2.2 (absence) | I |
| `scaling` | `None`. The HAL has size and offset per layer, no destination size. | RT §2.2 | S (absence); I (hardware, RM unread) |
| `max_source` | Not stated by the HAL header. Report the panel size and treat as open. | RT §2.2 | I |
| `plane_alpha` | `true`: alpha mode "override" uses an 8-bit global alpha per layer. | RT §2.2 | S |
| `blend_modes` | Opaque (alpha disabled), Coverage (embedded pixel alpha, straight). Premultiplied would need a Porter-Duff factor mode; the HAL lists the operators, but which combination equals a premultiplied Over was not worked out. | RT §2.2 | S (HAL names); mapping I |
| `zpos` | `Fixed(index)`. Layer index is the only ordering handle; which end is on top is open. | RT §2.2, Open question 1 | S (absence); direction I |
| `transforms` | Normal only at scan-out. The PXP rotates and flips in memory (RT §3), which is not a plane property. | RT §2.2, §3 | I |
| `full_output_only` | `false`. Each layer is a rectangle with an offset. | RT §2.2 | S |
| `resource_groups` | Two layer colour-space converters (`LAYER_CSC_COUNT 2`) could be one group: consumers `YUV`, capacity 2, over all layers. Whether the converters are shared by all layers or tied to some is not in the header. No bandwidth group is expressible (see section 5). | RT §2.2 | S (the count); grouping I |
| `replace_pending` | `true` for a buffer-address write: the shadow registers are overwritten until the shadow load is taken at VSYNC; the HAL does not refuse a second write. Atomicity across layers is open. | RT §4 | I |

The eLCDIF fallback (the other controller on the same board, RT §1, §2.1) is a degenerate case of the same table:

| Model field | Value | Basis | Tag |
|---|---|---|---|
| planes | One Primary, `full_output_only = true`, no alpha, `blend_modes` Opaque, `zpos` Fixed(0), Normal only | RT §2.1 | S (HAL: one buffer, no alpha surface); I (no hardware blend) |
| formats | RGB565, XRGB8888, RGB888 | RT §2.1 | S |
| `replace_pending` | `true`: one `NEXT_BUF` register; the driver proves a latch by `CUR_BUF` equal to the queued buffer | RT §4 | S (driver); I (overwrite is legal) |
| Fit | Single-plane only. It cannot carry a "two UI layers" or "video + UI" scene without the planner falling back to one composed layer. A PXP pass can compose, but the PXP is not in the model (section 5). | RT §3 | I |

What Zephyr gives today limits this. The stock `display_mcux_lcdifv2.c` drives layer 0 only, does not configure blend, and registers no event callback (RT §6). So the table above describes the silicon through the HAL, and a backend needs HAL calls beside the display driver or a new driver (RT §6 last row).

### 2.2 Renesas RA8P1 and RA8D1, GLCDC

RA says one `r_glcdc.c` serves both SoCs, so one table. Per-SoC silicon limits are unconfirmed (RA §1, Open question 3).

| Model field | Value to report | Basis | Tag |
|---|---|---|---|
| outputs | One. Exactly one GLCDC node in both SoC trees; DSI host is a separate driver. | RA §1, §7 | I (absence of a second node) |
| planes, count | Two: `DISPLAY_FRAME_LAYER_1` and `_2`. | RA §2 | S |
| `kind` | Layer 1 Primary, layer 2 Overlay. | RA §2 | I |
| `formats` | ARGB8888, XRGB8888 (RGB888 stored in 32 bits), RGB565. ARGB1555, ARGB4444 and CLUT8/4/1 have no FourCC in `types.rs`. No YUV. | RA §3 | S |
| `modifiers` | Linear. | RA §3 | I |
| `scaling` | `None`: the input config has size and stride, no destination size. | RA §2 | S |
| `max_source` | Panel-sized. The FSP limits (active width 16-2039, height 16-2043) were not re-read. | RA §2 | I |
| `plane_alpha` | `false` in the model. The API has no constant-alpha field. A timed fade ramp exists, which the model cannot state (section 5). | RA §3 | S |
| `blend_modes` | Opaque (private blend mode "non-transparent") and Coverage (per-pixel alpha, "transparent"). No premultiplied mode is named. | RA §2, §3 | S (modes private to `r_glcdc.c`); Coverage reading I |
| `zpos` | `Fixed(0)` and `Fixed(1)`: layer 1 under layer 2 on the background. No order setter in the API. | RA §2 | I |
| `transforms` | Normal. The API can show a line-descending buffer (a vertical flip) and repeat lines, but only as buffer properties; whether it maps to `FlipV` is not established. | RA §2 | S (fields); mapping I |
| `full_output_only` | `false`. A layer is a rectangle with a start offset. | RA §2 | S |
| `resource_groups` | None found. FSP has a bus-arbitration enum; it limits bandwidth, not a unit count. | RA §6 | S (enum); I (effect) |
| `replace_pending` | **`false`.** `R_GLCDC_BufferChange` and `LayerChange` return `FSP_ERR_INVALID_UPDATE_TIMING` while the layer's `PVEN` bit is set. That is the model's "committing while a submission is pending fails as busy". | RA §5 | S (refusal); I (that the bit clears at the next frame start) |

### 2.3 STM32H7, LTDC

| Model field | Value to report | Basis | Tag |
|---|---|---|---|
| outputs | One. Zephyr's driver hard-codes instance 0. | H7 §6 | S |
| planes, count | Two (`MAX_LAYER 2`). Layer 2 is guarded by `#if defined(LTDC_Layer2_BASE)`, so some parts may have one; not checked per part. | H7 §2, Open question 4 | S (HAL); per-part I |
| `kind` | Layer 1 Primary, layer 2 Overlay. | H7 §2 | I |
| `formats` | ARGB8888, XRGB8888 (RGB888 and ARGB8888 are the 32-bit options), RGB565. ARGB1555, ARGB4444, L8, AL44, AL88 have no FourCC in `types.rs`. No YUV on H7. | H7 §2 | S |
| `modifiers` | Linear. | H7 §2 | I |
| `scaling` | `None`: window size equals source size in `LTDC_SetConfig`. | H7 §2 | S (registers); I (absence in RM) |
| `max_source` | Panel-sized in Zephyr (`set_timing` rejects a mode above the DT size). Hardware limit not read. | H7 §2 | S (Zephyr); I (hardware) |
| `plane_alpha` | `true`: `CACR` holds a constant alpha 0..255. | H7 §2 | S |
| `blend_modes` | Opaque, Premultiplied and Coverage can all be chosen. The two blending factors are each "constant alpha" or "pixel alpha x constant alpha". Reading the names: straight alpha is factor 1 = PAxCA with factor 2 = PAxCA; premultiplied is factor 1 = CA with factor 2 = PAxCA. The equation is in RM0433, which was not read. | H7 §2 | S (the factor names); mapping I |
| `zpos` | `Fixed(0)`, `Fixed(1)`: layer 2 over layer 1. There is no z-order register. | H7 §2 | S (no register); direction I |
| `transforms` | Normal. | H7 §2 | I |
| `full_output_only` | `false`. Each layer has a window. | H7 §2 | S |
| `resource_groups` | None. The only shared resource is the memory bus, which is not a unit count. | H7 §5 | I |
| `replace_pending` | `true`. A reload request is `SRCR.VBR`. A write to a layer register before the reload overwrites the shadow value. | H7 §3 | I (shadow model inferred from the structure) |

H7 is the one target whose registers (judging by the HAL names) can express both blend roles of the twoplane contract: an opaque primary and a premultiplied overlay.

## 3. Mapping to the twoplane hook contract

The contract (HDR, TP) has these properties. Each commit is a set of plane writes that reach the panel together at a frame start. `commit_count` is read after the writes. `latched_count` reaches it once the writes are on the panel. A callback runs in the driver's interrupt once a frame. The backend locks interrupts only around its own state.

### 3.1 Hook map

Calls named in the source docs are given as the concrete driver call. Anything that is a guess is marked I.

| Hook | RT1170 LCDIFv2 | RA8P1 / RA8D1 GLCDC | STM32H7 LTDC |
|---|---|---|---|
| `app_format_ok` | Table lookup over RGB565, XRGB8888, ARGB8888, RGB888 (RT §2.2) | Same three plus RGB888 stored as 32 bit (RA §3) | Same (H7 §2) |
| `set_app_buffer` | `LCDIFV2_SetLayerBufferAddr` on layer 0, plus stride and format when they change (RT §2.2). The address must be 64-byte aligned (RT §5), and the D-cache is cleaned first (RT §5). | `R_GLCDC_BufferChange(layer 1)`; the address must be a multiple of 64 (RA §6). Cache clean is not done by Zephyr (RA §6). | `HAL_LTDC_SetAddress_NoReload` on layer 1, then the reload. The cache is flushed first (H7 §5). |
| `enable_app_plane` | `LCDIFV2_EnableLayer` on the layer, then the shadow load (RT §2.2, §4) | `R_GLCDC_LayerChange` with the layer enable; the exact field was not read (RA §5) | `__HAL_LTDC_LAYER_ENABLE`-style enable bit of layer 1, then the reload (H7 §6 uses the disable macro on layer 2) |
| `safety_flip` | Set the next address of layer 1, shadow load for that layer | `R_GLCDC_BufferChange(layer 2)` with the other buffer | `SetAddress_NoReload` on layer 2, then the reload |
| `safety_buffer`, `safety_front` | The driver's two buffers in cached RAM, 64-byte aligned (RT §5). The board default RAM is external SDRAM (RT §5). | Two buffers in the framebuffer array or `ext-ram`; Zephyr's array is `__aligned(8)`, not 64 (RA §6, Open question 7) | Static `frame_buffer_n`, 32-byte aligned, optionally in SDRAM (H7 §5) |
| `safety_premultiplied` | Porter-Duff or embedded alpha choice in `LCDIFV2_SetBlendConfig`; the right combination is I (RT §2.2) | **No premultiplied mode exists in the API**: the call can only return nonzero or do nothing (RA §3) | Choose blending factors 1 = CA and 2 = PAxCA, via the layer config (H7 §2); I |
| `commit_count` | Software counter of calls that write the layers | Software counter | Software counter |
| `latched_count` | Counts a commit when the vertical-blanking interrupt fires after the shadow-load trigger (the proof Zephyr's driver uses, RT §4). A read-back flag is not documented. | Count when `PVEN` of every written layer is seen clear in the line-detect handler; reading `GR[n].VEN_b.PVEN` is a register read the FSP does itself (RA §5) | Count on the reload-done interrupt `RRIF` after a VBR request, through `HAL_LTDC_ReloadEventCallback` (H7 §3) |
| `set_vsync_callback` | Vertical-blanking interrupt of LCDIFv2 (RT §4). The stock driver does not register a callback. | The `LINE_DETECTION` event, near the end of the active area, not at a pure VSYNC pulse (RA §5) | The line interrupt at line 0, which is one-shot in the HAL and is re-armed each time (H7 §3, §6) |

All rows of the first column of this table that name HAL calls are **S** for the call and **I** for "this is the right call for the hook". Rows that name an event are **S** for the event and **I** for its position in the frame, since no manual was read.

The port table (`irq_lock`, `wake`, `lookup`, `device_ready`) is system glue and does not depend on the display IP. It maps to the Zephyr calls TP lists, **S**; no target changes it.

### 3.2 How the latch and VSYNC callback would be driven

The twoplane backend (HDR) reports a latch when `latched_count` has reached the `commit_count` that was read after the commit's writes. It checks at every vsync callback, in `poll_event`, and at the end of the next commit. A target needs a driver-side counter pair with the same meaning.

- **RT1170 LCDIFv2** (I). Count a commit as written after the last layer shadow load of the commit. Count it as latched at the next vertical-blanking interrupt after that. This follows the one proof Zephyr's driver uses (RT §4). The weak point is atomicity: the trigger is per layer, and RT says whether several triggers apply together at one VSYNC is not stated (RT §4, Open question 2). A commit that touches two layers can be split over two frames, and a counter that follows the interrupt alone would count it latched when only one layer has moved.
- **RA8P1 / RA8D1 GLCDC** (I). Count a commit as written when its last `BufferChange` or `LayerChange` returned. Count it as latched when the line-detect handler sees every written layer's `PVEN` clear. The event is raised near the end of the active area (RA §5), so it lags the actual frame start by most of a frame. A commit written between the true frame start and the line-detect position would be counted latched one frame late, which only makes the backend report later, never early.
- **STM32H7 LTDC** (I). Stage both layers with `_NoReload` setters, request `SRCR.VBR` once, and count the commit latched when `RRIF` fires. One `SRCR` covers both layers, so the two-layer commit is atomic by construction (H7 §3, last row). The HAL setters reload immediately (`IMR`) and Zephyr uses `IMR` from the line-0 interrupt (H7 §3, §6); the contract's "reach the panel together at a frame start" needs VBR instead. Whether `RRIF` is set for both reload kinds is in RM0433 (H7 §3).

### 3.3 Where the twoplane contract itself does not fit

Each point is read in HDR or TP (**S**) and set against a source doc (**I** for the consequence).

1. **`replace_pending = 1` is hard-coded in `twoplane_capabilities`.** On the GLCDC a second `BufferChange` before the first one's `PVEN` clears is refused by FSP (RA §5). The adapter would have to hold the new buffer in software until the bit clears, or write the layer registers without the FSP check. Otherwise `set_app_buffer` returns nonzero and the backend returns `CE_ERR_BACKEND_REJECTED` for what the contract calls legal. The GLCDC fits the model's `replace_pending = false`, but the twoplane backend cannot say that.
2. **The safety plane is always premultiplied** (`CE_BLEND_PREMULTIPLIED`), and `ce_twoplane_init` ignores the return value of `safety_premultiplied`. On the GLCDC (no premultiplied mode, RA §3) the capabilities would still claim premultiplied, so a premultiplied Slint buffer would be blended wrongly with no error.
3. **Every plane is `full_output_only`** and "programmed at the buffer's size from the top left". All three IPs can place a rectangle (RT §2.2, RA §2, H7 §2), so the contract uses less than they offer; it does not conflict.
4. **One output and exactly two planes.** RT1170 LCDIFv2 has 8 layers (RT §2.2). A backend with more than two planes is not a twoplane backend; it needs a new backend with the same port table.
5. **The safety plane has no off switch** (`safety == NULL` is rejected in `twoplane_check`). All three IPs have a layer enable, so this is a policy of the contract, not a limit of the targets.
6. **Timing of the writes in one commit.** The contract says the writes of a commit "reach the panel together at a frame start, with any later writes made before that frame start". LCDIFv2 and GLCDC have a per-layer latch (RT §4, RA §5), so two writes in one commit can land on different frames unless the adapter holds the lock around both and checks the frame position. H7 has one global `SRCR` and so does not have this problem (H7 §3).

## 4. Fit to the WP0 scenes (summary, inferred)

All **I**: they combine section 2 with the scene names in RT §2.2.

| Scene | RT1170 LCDIFv2 | RA8P1 / RA8D1 | STM32H7 |
|---|---|---|---|
| Two UI layers | Fits (8 layers) | Fits (2 layers) | Fits (2 layers) |
| Video + UI | Packed YUV 4:2:2 only, no NV12, no scaling | No YUV, no scaling: fails unless the video is RGB | No YUV: fails unless the video is RGB |
| UI + priority layer | Fits | Fits; premultiplied is missing, so the overlay must be straight alpha | Fits; premultiplied is available |
| Unsupported geometry (needs scaling) | Rejected by `scaling = None`; a PXP pass could scale, but only outside the model | Rejected; DRW is not used by Zephyr (RA §4) | Rejected; DMA2D does not scale (H7 §4) |

## 5. What the model cannot express

Each item says what the hardware has (source doc row), what the model offers, and what a backend would have to do. All the hardware facts are **S** from the source doc cited; the statement that the model lacks a field is **S** from CAPS; the work-around is **I**.

1. **CLUT formats.** LCDIFv2 has CLUT 1/2/4/8 bpp (RT §2.2) and a 256-entry-per-CLUT table with shadow-load control (RT §4). GLCDC has CLUT8/4/1 (RA §3). LTDC has L8, AL44, AL88 (H7 §2). `PixelFormat` is a FourCC: a code can name an indexed format, but the palette is not part of a layer or a plane, and `FormatModifier` has no field for it. A palette needs a side channel in the backend. The planner treats an unknown FourCC as possibly carrying alpha.
2. **Formats with no FourCC in `types.rs`.** ARGB1555, ARGB4444, AL44 and AL88 can be written as numeric codes but are unknown to the planner's opacity logic.
3. **The background colour plane.** GLCDC has `R_GLCDC->BG` and a per-layer `bg_color` (RA §2). LTDC has `BCCR` (H7 §2). LCDIFv2 has a per-layer background call (RT §2.2). `OutputCaps` has size and refresh only, and a frame has no "colour behind all layers". The area a layer does not cover shows an unmodelled colour.
4. **Chroma key.** LTDC `CKCR` (H7 §2), GLCDC key types that FSP returns `FSP_ERR_UNSUPPORTED` for (RA §3), PXP overlay key (RT §3). No `PlaneCaps` field, no layer property.
5. **Fade.** GLCDC's timed fade-in and fade-out with a frame counter (RA §3). The model has a `plane_alpha` bool and, in a frame, one alpha value; it has no ramp and no pending-fade state.
6. **Granularity of plane alpha.** `plane_alpha` is a bool. LCDIFv2 has 8-bit global alpha (RT §2.2) and LTDC 0..255 (H7 §2), while `ALPHA_OPAQUE` is a 16-bit value (`types.rs`). The planner cannot know how a 16-bit value is quantised, or that GLCDC has none.
7. **Immediate versus vertical-blanking reload.** LTDC offers both (`SRCR.IMR`, `SRCR.VBR`) and Zephyr picks IMR (H7 §3, §6). The model has no reload-mode field. A tear-free commit requires VBR (section 3.2), so this is a backend-internal choice that the planner cannot see or request.
8. **Shadow-register granularity and atomic groups.** LCDIFv2 (RT §4) and GLCDC (RA §5) latch per layer. LTDC (H7) latches the whole block at once. `Capabilities` has no "these planes latch together" relation, and `replace_pending` is one bool for the whole backend, though GLCDC refuses per layer, and LCDIFv2's overwrite rule is per layer.
9. **A latch that can be refused, or applied late.** GLCDC's refusal (`FSP_ERR_INVALID_UPDATE_TIMING`) maps to `replace_pending = false`, but only as "busy" for the commit. The model does not say which layer is pending, so a commit touching only the other layer is refused too.
10. **A 2D engine as a composition target.** PXP (RT §3), DRW (RA §4), DMA2D (H7 §4) write to memory and never scan out. The model has planes that scan out and resource groups that count units on planes. A fallback "blend two layers into one buffer when planes run out" has no place in `Capabilities`. A DMA2D pass also needs cache maintenance on both buffers (H7 §5) and takes memory bandwidth from the LTDC (H7 §4, dead-time controls), neither of which the model can say.
11. **Format conversion and rotation by an engine.** The PXP rotates, flips, scales and converts YUV (RT §3). DMA2D converts formats, including YCbCr, but does not scale or rotate (H7 §4). These could widen what a plane accepts if the backend ran a pass first, but then the layer is no longer scanned from the client's buffer. The model has no "needs a pass" resource.
12. **Memory placement and visibility.** 64-byte alignment of layer addresses and strides (RT §5, RA §6), 32-byte alignment (H7 §5), D-cache clean before scan-out, SDRAM or OCRAM placement, the FMC speculative-read workaround (H7 §5). `PlaneCaps` has no alignment, no stride rule and no memory-region field. A mis-placed buffer is caught by `check` at best.
13. **Bandwidth and underrun.** All three can underrun (RT §4 FIFO-empty; RA §5 underflow events; H7 §3 `FUIF`). Resource groups count units, not bytes per second. Zephyr's drivers do not report underruns (RA §5, H7 §6).
14. **Colour encoding of YUV.** LCDIFv2 has two layer colour-space converters with a colour-space mode (RT §2.2). The model has the `YUV` trait but no matrix or range field, which DRM carries as plane properties elsewhere.
15. **Blend equations beyond three modes.** LCDIFv2 has 12 Porter-Duff operators (RT §2.2); LTDC has two-factor selection (H7 §2). The model has Opaque, Premultiplied and Coverage.
16. **Output-side processing.** Brightness, contrast, gamma and dither (RA §3); global dither (H7 §2). These are properties of an output, not of a plane, and `OutputCaps` has none.
17. **Event kinds.** `BackendEvent` has `Latched`, `DeviceLost`, `CapabilitiesChanged`. FIFO underflow, line detection and frame end (RT §4, RA §5) have no counterpart; Zephyr surfaces `FIFO_UNDERFLOW` on eLCDIF but not on the other drivers (RT §4).
18. **z-order that is unknown.** LCDIFv2's stacking direction is open (RT, Open question 1). `Zpos::Fixed(n)` states a number, not a direction, so the planner would have to be told.
19. **Two controllers as alternatives.** The EVK selects eLCDIF or LCDIFv2 for the one DSI output (RT §1). The model has one backend per `Capabilities`; "either of these two, not both" is a choice of the integrator at build time, not a model feature.

## 6. Comparison table

Columns: the three candidates, then the i.MX95 DPU and the AM62P DSS. The last two are taken from MX, never from another source. MX "not recorded" means that no document of this repository says it; it does not mean the hardware lacks the feature. Tags: **S** = *read from source*, **I** = *inferred*. For the two MX columns, **S** means the MX cell is a source-tagged cell and the section cited is where it appears; **I** means MX itself marks it inferred. "Not recorded" cells are tagged **S** because MX says so.

| Item | RT1170 (LCDIFv2) | RA8P1 (GLCDC; RA8D1 same code) | STM32H7 (LTDC) | i.MX95 DPU | AM62P DSS |
|---|---|---|---|---|---|
| Outputs | 1 chosen DSI route (RT §1) **S** | 1 GLCDC (RA §1) **I** | 1 (H7 §6) **S** | 2 CRTCs, one encoder each (MX §1) **S** | Not recorded (MX §1) **S** |
| Plane count | 8 layers; Zephyr uses 1 (RT §2.2, §6) **S** | 2 layers; Zephyr uses 1 (RA §2, §7) **S** | 2 layers; Zephyr uses 1 (H7 §2, §6) **S** | 7 planes, at most 6 active (MX §1) **S** | 2 used by the demo; total not recorded (MX §1) **S** |
| Plane kinds | Primary + overlays (RT §2.2) **I** | Primary + overlay (RA §2) **I** | Primary + overlay (H7 §2) **I** | 2 primary, 5 overlay, no cursor (MX §1) **S** | Application and safety plane (MX §1) **S** |
| Formats | RGB565, ARGB8888, ABGR8888, RGB888, packed YUV 4:2:2, ARGB1555/4444, CLUT (RT §2.2) **S** | ARGB8888, RGB888 (32 bit), RGB565, ARGB1555/4444, CLUT (RA §3) **S** | ARGB8888, RGB888, RGB565, ARGB1555/4444, L8, AL44, AL88 (H7 §2) **S** | ARGB/XRGB/ABGR/XBGR/RGBA/RGBX/BGRA/BGRX8888, RGB565, YUYV, UYVY, NV12, NV21 (MX §1) **S** | RGB565 (application), ARGB8888 (safety) in the demo; full list not recorded (MX §1) **S** |
| YUV | Packed 4:2:2 only; 2 layer converters (RT §2.2) **S** | None (RA §3) **S** | None (H7 §2) **S** | Yes, via FetchYUV units (MX §1) **S** | Not recorded (MX §1) **S** |
| Scaling | None in the HAL (RT §2.2) **S**; hardware **I** | None (RA §2) **S** | None (H7 §2) **S**; hardware **I** | Upscale only; shared scalers (MX §1) **S** | Not recorded (MX §1) **S** |
| Plane alpha | Yes, 8 bit (RT §2.2) **S** | No constant alpha; fade ramp (RA §3) **S** | Yes, 0..255 (H7 §2) **S** | Yes (MX §1) **S** | Not recorded (MX §1) **S** |
| Blend modes | Disable, override, embedded, Porter-Duff (RT §2.2) **S** | Non-transparent, transparent (RA §2) **S** | Two factors, CA or PAxCA each (H7 §2) **S** | None, premultiplied, coverage (MX §1) **S** | Premultiplied switch on the safety plane in the driver table (MX §1, TP) **S** |
| z-order | Layer index; direction open (RT §2.2) **S**/**I** | Fixed, layer 2 over 1 (RA §2) **I** | Fixed, layer 2 over 1 (H7 §2) **S**/**I** | Mutable, 0..5 (MX §1) **S** | Not recorded (MX §1) **S** |
| Transforms | None at scan-out; PXP in memory (RT §3) **I** | None in the model; line-descending buffer flag (RA §2) **S** | None (H7 §2) **I** | No property (MX §1) **S** | Not recorded (MX §1) **S** |
| 2D helper | PXP: rotate, flip, scale, blend (RT §3) **S** | DRW/D2: no Zephyr use (RA §4) **S** | DMA2D: fill, convert, blend; no Zephyr driver (H7 §4) **S** | Not recorded (MX) **S** | Not recorded (MX) **S** |
| Latch granularity | Per layer shadow load (RT §4) **S** | Per layer valid bit (RA §5) **S** | Global `SRCR` for both layers (H7 §3) **S** | Not recorded; the backend's `latched_count` follows ExtDst0's sync busy bit (MX §2) **S** | The GO bit (MX §2) **S** |
| Latch / frame signal | Vertical-blanking interrupt; no Zephyr callback (RT §4) **S** | Line-detect near end of active area (RA §5) **S** | Line interrupt (one-shot) and `RRIF` (H7 §3) **S** | Driver callback once a frame, same table (MX §2) **S** | Driver callback once a frame (MX §2) **S** |
| Pending behaviour | Second shadow write overwrites; refusal not shown (RT §4) **I** | Second change refused until the bit clears (RA §5) **S** | Overwrites until the reload (H7 §3) **I** | Backend sets `replace_pending = 1` (MX §2 via TP) **S** | Same backend (MX §2 via TP) **S** |
| Zephyr display driver | eLCDIF mature, LCDIFv2 new, layer 0 only (RT §6) **S** | `display_renesas_ra`, layer 1 only (RA §7) **S** | `display_stm32_ltdc`, layer 1 only (H7 §6) **S** | `display_socionext_dpu.c`, one fixed pipeline, single framebuffer (MX §2) **S** | `display_am62p_dss.c` in the demo repository, content not recorded (MX §2) **S** |
| Fit to `backends/twoplane` | Fits two layers; per-layer latch is the risk (section 3.3) **I** | Fits two layers; `replace_pending` and premultiplied do not (section 3.3) **I** | Fits; one reload covers both layers (section 3.3) **I** | The demo firmware fills the table (MX §2, TP) **S** | The backend was taken from this firmware (MX §2) **S** |
| Board on the farm | None (RT §7) **S** | None (RA §8) **S** | None (H7 §7) **S** | Verdin iMX95 board (MX §3 refers to it) **S** | Not recorded in MX (MX §2 describes the demo, not a board) **S** |
| Board evidence | None; desk only **S** | None; desk only **S** | None; desk only **S** | None in the cells (MX "What is observed") **S** | None in the cells (MX "What is observed") **S** |

## 7. Findings for later items

These are not changes made here.

1. **Premultiplied blend is the discriminator for the second plane.** H7 can plausibly express it, GLCDC cannot (RA §3). RT1170 may, through Porter-Duff, but the factor choice is unworked. This matters for the "UI + priority layer" scene when the overlay buffer is premultiplied.
2. **The twoplane backend cannot describe a GLCDC.** `replace_pending = 1` is fixed, and the safety plane's premultiplied claim is fixed. Both are in `backends/twoplane/`, which this work may not edit. A GLCDC port needs either a software shadow in the driver adapter, or a change to the backend so these two values come from `hw`.
3. **The model lacks a reload-mode and a latch-group field.** The H7 case (one global reload) and the RT1170 and RA cases (per-layer) differ in a way the planner cannot see. If a later item pins planner behaviour for these targets, it should record this as a finding, not model it.
4. **The 2D engines are the real "more layers than planes" answer on these parts**, and the model has no place for them. The `Capabilities` model fits the planes; it does not fit a compose-into-a-buffer path.
5. **All three targets need the manuals before any backend.** RT: Reference Manual (stacking order, shadow-load atomicity, scaler absence, memory attributes). RA: user's manual GLCDC chapter (`PVEN` timing, layer count per SoC, M85 cache). H7: RM0433 and AN4861 (reload timing, line 0 position, `RRIF` for IMR). Section 3.2 and the blend rows of section 2 depend on them.
6. **For `docs/open-issues.md` M3-06**: this file is the second desk step after the three source docs. The row change belongs to the item that owns that file.
