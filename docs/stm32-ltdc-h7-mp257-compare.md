# STM32H7 LTDC against STM32MP257 LTDC: comparison read from the existing docs

**Nothing in this file is a measurement on the STM32H7.** No H7 board was used, and
nothing was read from kernel, HAL or Zephyr source for this file. It only sets
two existing documents side by side. This work counts toward none of M1, M2 or
M3. It is portability evidence for the MCU candidate of the Zephyr path (M3-06).

Written 2026-11-28 (host clock).

## Sources and tags

- H7 side: [`stm32h7-ltdc-source.md`](stm32h7-ltdc-source.md), a desk evaluation of the
  H743 CMSIS header, the ST HAL and Zephyr's `display_stm32_ltdc`. Reference
  manual RM0433 and AN4861 were **not** read there (its section 9).
- MP257 side: [`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md),
  [`stm32mp257-ltdc-transform-source.md`](stm32mp257-ltdc-transform-source.md),
  [`stm32mp257-ltdc-yuv-source.md`](stm32mp257-ltdc-yuv-source.md),
  [`stm32mp257-ltdc-sync-source.md`](stm32mp257-ltdc-sync-source.md) and
  [`design/stm32mp257-backend.md`](design/stm32mp257-backend.md) sections 1 to 4.
  Board results (dated 2026-11-27 and 2026-11-28) come from [`stm32mp257-ltdc-board.md`](stm32mp257-ltdc-board.md)
  and the session docs.

Tags, one per cell:

- **read from source (doc, section)**: the cited doc states it as read from
  source. The doc and its section are named; line numbers stay in that doc.
- **inferred (doc, section)**: the cited doc tags it inferred, or this file
  draws it from an absence the cited doc reports.

No cell here is tagged observed. The MP257 cells that rest on a board result
are in the second half.

## Feature comparison

"Sync" is [`stm32mp257-ltdc-sync-source.md`](stm32mp257-ltdc-sync-source.md); "Transform" is
[`stm32mp257-ltdc-transform-source.md`](stm32mp257-ltdc-transform-source.md); "YUV" is
[`stm32mp257-ltdc-yuv-source.md`](stm32mp257-ltdc-yuv-source.md); "Full" is
[`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md); "Design" is
[`design/stm32mp257-backend.md`](design/stm32mp257-backend.md); "H7" is
[`stm32h7-ltdc-source.md`](stm32h7-ltdc-source.md).

| Feature | STM32H7 | STM32MP257 | Same or different |
| --- | --- | --- | --- |
| Layer count | 2, fixed: `MAX_LAYER` is 2. Some parts may have one. **read from source** (H7, section 2) | `LTDC_MAX_LAYER` 4, count read from `LTDC_LCR` at run time. **read from source** (Full, "Settled items"). Expected 3 on MP25 from the firewall children, and a layer behind a denied firewall is dropped. **inferred** (Full, "Settled items"; Design, section 1) | Different: 2 against 3 expected |
| Formats | ARGB8888, RGB888, RGB565, ARGB1555, ARGB4444, L8, AL44, AL88. **read from source** (H7, section 2) | RGB and C8 on every layer, plus the YUV set on some layers. **read from source** (YUV, "How the driver programs the conversion"; Design, section 2) | RGB core is the same family. MP257 adds formats |
| YUV | No converter: no `PCR`, no `YCEN`, no YUV format in the HAL. **read from source** (H7, section 2) for the H743 header and the HAL. That every H7 part lacks it: **inferred** (H7, section 2). DMA2D reads YCbCr but writes RGB. **read from source** (H7, section 4) | Packed YUYV family on layers with `YIA`, semi-planar NV12/NV21 with `YSPA`, planar with `YFPA`; BT.601 and BT.709, limited and full range; BT.2020 falls back to BT.601 limited. **read from source** (YUV, "How the driver programs the conversion"; Design, section 2) | Different. H7 has no YUV in the LTDC |
| Per-layer scaler | None. The layer block has no scaler register, and window and source share one size. **read from source** (H7, section 2) for the registers; "no scaling" as a conclusion: **inferred** (H7, section 2). DMA2D cannot scale either. **read from source** (H7, section 4) | Upscale only, on layers whose `LXCR_C1R_SCA` bit is set; downscale and YUV-with-resize refused; no numeric cap in the code. **read from source** (Full, "Settled items"; Design, section 3). Which layers have the scaler is a register fact, not in source. **inferred** (Full, "Settled items") | Different. H7 has no scaler |
| Rotation and mirroring | None in the registers read. **read from source** (H7, section 8, "What differs", citing the register block of section 2); "none in hardware" as a conclusion: **inferred** (H7, section 2) | Reflect-x (`HMEN`), reflect-y by negative pitch. 90 and 270 need the separate rotator and reserved memory, and are refused on the reference tree. **read from source** (Transform, sections 1 and 3; Design, section 2) | Different. No H7 counterpart |
| Blending and constant alpha | Per-layer constant alpha 0..255 (`Alpha`, `Alpha0`). Two blend factors in `BFCR`: factor 1 is `CA` or `PAxCA`, factor 2 is `CA` or `PAxCA`. **read from source** (H7, section 2). The blend equation is the usual LTDC one. **inferred** (H7, section 2; RM0433 not read) | `drm_plane_create_alpha_property` on every plane; no blend-mode property; blend factors in `LTDC_L1BFCR`. **read from source** (Full, "Settled items"). Straight alpha as the blend law is a board inference, see the second half. **inferred** (Design, section 4) | Same register structure and the same lineage. **inferred** (H7, section 8, "Same IP lineage") |
| z-order | Fixed: layer 2 over layer 1 over the background colour. There is no z-order register in the layer block. **read from source** (H7, section 2, the register absence); "layer 2 on top" is **inferred** (H7, section 2; RM not read) | Mutable `zpos` 0..`nb_layers-1`, initial value is the layer index, `normalized_zpos` written into `LTDC_L1BFCR` at shift 16. **read from source** (Full, "Settled items") | Different. The H7 order is fixed |
| Shadow reload | Layer registers are shadowed. Reload through global `SRCR` with `IMR` (immediate) or `VBR` (vertical blanking). HAL setters use IMR; a `_NoReload` twin exists for each. Zephyr swaps from the line-0 interrupt using IMR. **read from source** (H7, sections 3 and 6). The shadow model itself is **inferred** (H7, section 3) | Older IP: one global `SRCR_VBR` in `atomic_flush`. Newer IP (`plane_reg_shadow`): per-layer `LTDC_L1RCR` with `LXRCR_VBR`. IMR only on disable. **read from source** (Sync, section 1). MP25 takes the per-layer path: **inferred**, strong (Sync, "MP25 hardware version") | Different. H7 has one global reload; MP25 reloads per layer |
| Vblank and latch event | `RRIF/RRIE` exists and the HAL exposes `HAL_LTDC_ReloadEventCallback`; the line interrupt is one-shot in the HAL, and Zephyr completes its swap there. **read from source** (H7, section 3). When IMR takes effect, and whether `RRIF` is set for IMR, are RM0433 questions. **inferred** (H7, sections 3 and 10) | The flip event is armed in `atomic_flush` and completed on the line interrupt `ISR_LIF` through `drm_crtc_handle_vblank`; `ISR_RRIF` is defined and not tested. **read from source** (Sync, sections 1 and 2). Where the line interrupt sits in the frame: **inferred** (Sync, section 1) | Same completion model (line interrupt, not reload-done). Position of the line open on both. **inferred** (H7, section 10; Sync, section 1) |
| Atomicity of a multi-layer update | One `SRCR` covers both layers, so staging both with `_NoReload` and one VBR would land together. **inferred** (H7, section 3) | Older IP: one `SRCR_VBR` after all plane updates. **read from source** (Sync, section 5), "all planes in one commit" **inferred**. Newer IP: each layer reloads on its own, atomicity depends on timing. **inferred** (Sync, section 5) | Different, and both rest on inference |
| CRC | No CRC register in the global block the H7 doc lists (`SSCR` to `CDSR`), and the layer block has none. **inferred** from that absence (H7, section 2; H743 header only) | `GCR_CRCEN` and `LTDC_CCRCR` over the whole output, reported through the DRM CRC interface from the line interrupt after skipping 2 frames. **read from source** (Transform, section 4; Sync, section 2). The MP251/MP255 CRC handling is in the 4.x case, and a denied firewall can remove it. **read from source** (Sync, "MP25 hardware version"; [`stm32mp257-ltdc-session-c-outputs-2026-11-28.md`](stm32mp257-ltdc-session-c-outputs-2026-11-28.md), "Q4") | Different. No CRC readout is known for the H7 |

The H7 doc's own section 8 reaches the same overall reading: H7 is the small, old
end of the LTDC family. That is **inferred** there.

## Which observed MP257 results could inform an H7 profile

All of these are board results on the STM32MP257F-DK under Linux. The H7 has not
been seen at all. "Could inform" means the result touches something both IPs are
known to share by the cited docs, and then only as a hypothesis to test. The
dates and values are those of the cited docs.

| MP257 result | Could it inform the H7? | Why | Tag |
| --- | --- | --- | --- |
| Plane alpha applied as straight alpha on the output, fitted through the output CRC (observed 2026-11-27, [`stm32mp257-ltdc-session-2026-11-28.md`](stm32mp257-ltdc-session-2026-11-28.md), "Q2") | Yes, as a hypothesis | Both have per-layer constant alpha and a two-factor blend in `BFCR` (see the table). The H7 factors `CA` and `PAxCA` suggest the same law. RM0433 would settle it | inferred |
| The 600-frame two-plane run: all frames latched, no drop, latch interval about 16.7 ms (observed 2026-11-27, Design, section 6) | Partly | It shows a line-interrupt completion with a per-layer reload working. It says nothing about IMR from the line-0 interrupt, which is the H7 Zephyr path | inferred |
| `fifo_underrun_warning` grows only while a scaled layer is on screen (observed 2026-11-27, [`stm32mp257-ltdc-session-2026-11-28.md`](stm32mp257-ltdc-session-2026-11-28.md), "Scaled commits") | Only as a reminder | The H7 has no scaler. The H7 doc shows the FIFO underrun flag and its enable, so bandwidth starvation is reportable there. The MP257 numbers say nothing about H7 bandwidth, bus or memory | inferred |
| Reading the LTDC output CRC to check pixels (observed 2026-11-27, same doc, "Q2") | No | No CRC register is known for the H7 (table, CRC row) | inferred |
| TEST_ONLY accepted 2x and 32x upscale on planes 34 and 42, plane 49 only 1.0, downscale rejected everywhere (observed 2026-11-27, [`stm32mp257-ltdc-board.md`](stm32mp257-ltdc-board.md), "Scaling bounds") | No | Scaler ability is a per-layer MP25 register bit. The H7 has no scaler | read from source (Full, "Settled items") for the mechanism |
| Planes' `zpos` range 0..=2 and defaults 0, 1, 2 (observed 2026-11-28, Design, section 2) | No | The H7 order is fixed, so a profile has no zpos to claim | read from source (H7, section 2) |
| YUV format lists and `COLOR_ENCODING` and `COLOR_RANGE` properties (observed 2026-11-28, Design, section 2); the NV12 colour check that was never run ([`stm32mp257-ltdc-session-c-2026-11-28.md`](stm32mp257-ltdc-session-c-2026-11-28.md)) | No | No YUV converter in the H7 LTDC | read from source (H7, section 2) |
| Reflect and rotation plane properties; pictures of transforms never run (same session C doc) | No | No transform registers read on the H7 | inferred (H7, section 2) |
| One connector, one CRTC, three planes, the DSI to ADV7535 route and a disabled LVDS node (observed 2026-11-28, [`stm32mp257-ltdc-session-c-outputs-2026-11-28.md`](stm32mp257-ltdc-session-c-outputs-2026-11-28.md), "Q8") | No | Topology belongs to the SoC and the board, and the H7 under Zephyr has no DRM objects | read from source (H7, section 8, "What differs") |
| The DRM-level findings: TEST_ONLY and what the kernel refuses, harness event accounting, and the unsupported-combination table (observed 2026-11-27, Design, section 5) | No | Zephyr's `display` API has no atomic commit and no TEST_ONLY on the H7 | read from source (H7, section 8, "What differs") |

What the MP257 work does give the H7 path is method, not data: the evidence tags,
the idea of stating a claim in a profile only when a result supports it, and the
open questions that carry over by name (where the line interrupt sits, when
IMR takes effect), which the H7 doc lists as questions 1 and 2 in its section 10.
They stay open until the reference manual is read or an H7 board is run.

## What would be needed before an H7 profile claims anything

All inferred, none of it done here:

- Read RM0433 and AN4861 (not reachable in the H7 doc, section 9) for the blend
  equation, the timing of IMR and the position of line 0.
- Run the H7 on a board (the boards listed in the H7 doc, section 7) for any
  tag of the kind "seen on the board" (none exists, 2026-11-28). This file creates none.
- Keep the MP257 profile fields (`max_downscale`, `zpos`, YUV, transforms) out of
  an H7 profile. The H7 doc already reads them as "no scaling", "fixed order",
  and nothing for YUV and rotation (section 8).
