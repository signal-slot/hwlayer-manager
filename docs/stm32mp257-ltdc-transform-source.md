# STM32MP257 LTDC: how reflect-x, reflect-y, 180, 90 and 270 are programmed, with predicted pictures

Date of reading: 2026-11-28. **Nothing here is a board measurement and no picture has been taken.** Every row is tagged **read from source** (stated by the code read) or **inferred** (a conclusion drawn from it). This is the baseline for the Q6 picture session of [`stm32mp257-ltdc-session-2026-11-28.md`](stm32mp257-ltdc-session-2026-11-28.md).

## What was read

`drivers/gpu/drm/stm/ltdc.c` (2943 lines) of `STMicroelectronics/linux`, branch `v6.6-stm32mp`, commit `548f960c059bc4b9165cb69895fd67551f6061ca`, the commit pinned by [`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md) and [`stm32mp257-ltdc-source-diff-6.6.md`](stm32mp257-ltdc-source-diff-6.6.md). The raw file at that commit was downloaded with `curl` and read with line numbers; all `ltdc.c:N` below are lines of that file. The earlier diff document finds the plane path of v6.18-stm32mp (`3056405e`) identical to 6.6, so no separate v6.18 rows are added (the v6.18 numbers are 5 to 6 lines higher in this region, see its rows 43, 55-61). No checkout was kept in the dependency cache (`git clone` was refused); the file was held in a scratch file and removed.

## 1. Reflect-x, reflect-y and their combination: `ltdc_plane_update` (`ltdc.c:1648`)

All planes go through this one function (called from `ltdc_plane_atomic_update`, `ltdc.c:2056`, and from the enable path, `ltdc.c:2107`). There is no vertical-flip bit: reflect-y is a negative pitch.

| Step | What the code does | Where | Tag |
| --- | --- | --- | --- |
| Fold | `plane_rotation` starts at `ROTATE_0`; `ROTATE_90` sets `ROTATE_90 | REFLECT_Y`; `ROTATE_180` sets `REFLECT_X | REFLECT_Y`; `ROTATE_270` sets `ROTATE_90 | REFLECT_X`. The user's `REFLECT_X` and `REFLECT_Y` are then toggled in (cleared if the rotation already set it) | `ltdc.c:1688-1717` | read from source |
| Reflect-x, start address | `paddr = drm_fb_dma_get_gem_addr(...)`, then `paddr += cpp[0] * src_width - 1`: the last byte of the first line | `ltdc.c:1845-1848` | read from source |
| Reflect-y, start address | `paddr += pitches[0] * (src_height - 1)`: the first line fetched is the last line | `ltdc.c:1850-1851` | read from source |
| Address write | `LTDC_L1CFBAR + lofs` (colour frame buffer address) | `ltdc.c:1854` | read from source |
| Reflect-y, pitch | `pitch_in_bytes = 0x10000 - pitches[0]` ("negative value, signed on 16 bits"), otherwise `pitches[0]` | `ltdc.c:1859-1863` | read from source |
| Pitch and length write | `val = (pitch_in_bytes << 16) | line_length`, `line_length = cpp[0] * src_width + (bus_width >> 3) - 1`, written to `LTDC_L1CFBLR` under `LXCFBLR_CFBLL | LXCFBLR_CFBP` (pitch is `GENMASK(31, 16)`, `ltdc.c:234`) | `ltdc.c:1857`, `1865-1866` | read from source |
| Line count | `LTDC_L1CFBLNR = src_height` (unchanged by reflection) | `ltdc.c:1869-1870` | read from source |
| Reflect-x, mirror bit | `lxcr |= LXCR_HMEN` (`BIT(8)`, "Horizontal Mirroring ENable", `ltdc.c:214`); written to `LTDC_L1CR` with `LXCR_MASK` (which includes `LXCR_HMEN`, `ltdc.c:216`) | `ltdc.c:1975-1977`, `2007-2010` | read from source |
| YUV, auxiliary planes | The same reflect-x and reflect-y adjustments are applied to `LTDC_L1AFBA0R`/`LTDC_L1AFBA1R` (half-width / half-height offsets) and the negative pitch to `LTDC_L1AFBLR` | `ltdc.c:1880-1922`, `1931-1945` | read from source |
| Reflect-x and reflect-y together | All of the above apply: HMEN, last-byte offset, last-line offset, negative pitch | as above | read from source |
| Reflection and scaler | Independent: the scaler registers (`LTDC_L1SISR`, `L1SOSR`, scaling factors) are written from `src`/`dst` sizes, with no reference to `plane_rotation` | `ltdc.c:1979-2005` | read from source |
| Shadow reload | `LTDC_L1RCR + lofs = LXRCR_VBR` once at the end of the update when `plane_reg_shadow` (true on 4.x): vertical-blanking reload, once per commit | `ltdc.c:2012-2015` | read from source |
| Atomic check | `ltdc_plane_atomic_check` has no rotation or reflection test (diff row 35, `ltdc.c:1566-1605`); only the property mask (section 3) filters | | read from source |
| Same registers on overlays | The reflect code has no layer-index condition, so plane 42 and plane 49 use the same registers at their own layer offset `lofs` | | inferred |

## 2. The primary's 180

| Fact | Where | Tag |
| --- | --- | --- |
| 180 is `REFLECT_X | REFLECT_Y`: HMEN plus last-byte and last-line start address plus negative pitch | `ltdc.c:1695-1696` | read from source |
| The rotator is **not** involved: `GCR_ROTEN` and the rotation memory are used only inside `if (plane_rotation & DRM_MODE_ROTATE_90)`, and 180 never sets `ROTATE_90` | `ltdc.c:1738-1784` | read from source |
| 180 on the primary and reflect-x + reflect-y on an overlay are the same programming | | inferred |

## 3. 90 and 270, and where they are refused

| Fact | Where | Tag |
| --- | --- | --- |
| Primary property: 90/180/270 are in the mask when `crtc_rotation` is true at creation; otherwise only `ROTATE_0 | REFLECT_X | REFLECT_Y` | `ltdc.c:2263-2276` | read from source |
| Overlay property: only `ROTATE_0 | REFLECT_X | REFLECT_Y`. So `rotate-180/90/270` on plane 42 or 49 are refused by the DRM core (not in the mask), which matches the `EINVAL` of session B | `ltdc.c:2311-2314` | read from source (mask); that EINVAL comes from the core is inferred |
| `crtc_rotation` is set from `GC2R_ROTA` for the 4.x hardware | `ltdc.c:2571-2573` | read from source |
| **Ordering:** `ltdc_crtc_init` (which creates the property) is called at `ltdc.c:2774`; `crtc_rotation` is cleared when there is no `rotation-memory` only at `ltdc.c:2820-2826`, later in `ltdc_load`. So on a tree with `GC2R_ROTA` set and no `rotation-memory` (this board, per the earlier documents) the primary **still offers 90/180/270**. This explains why session B's 90/270 passed `TEST_ONLY`; it corrects the expectation in conclusion 3 of the diff document that the property lacks them | `ltdc.c:2774`, `2263-2271`, `2825-2826` | read from source |
| Because `crtc_rotation` ends up false, the guard against a second layer while `GCR_ROTEN` is set (`ltdc.c:1669-1682`) is skipped on this board | `ltdc.c:1669` | read from source |
| 90 and 270 reach the rotator through `if (plane_rotation & DRM_MODE_ROTATE_90)`: clear `GCR_LTDCEN`, wait about two frames | `ltdc.c:1738-1743` | read from source |
| Refusal "Rotation buffer too small": `ldev->rot_mem->size < hdisplay * vdisplay * 2 * 3`, a `DRM_WARN` and a bare `return` with the controller still off | `ltdc.c:1753-1756` | read from source |
| `vdisplay > 1366` returns silently, controller still off | `ltdc.c:1759-1760` | read from source |
| `ldev->rot_mem` is dereferenced at `ltdc.c:1753` **with no null check**; it is null when the DT has no `rotation-memory` (`ltdc.c:2820-2822`). So a committed 90/270 on this board, after `GCR_LTDCEN` was cleared and the delay, reaches a null pointer dereference rather than the "too small" warning. Not tried on the board; nobody should commit 90/270 without a console | `ltdc.c:1740-1753`, `2820-2826` | inferred from the lines cited |
| Rotator setup (not reached here): `LTDC_RB0AR` = base, `LTDC_RB1AR` = base + size/2, `LTDC_RBPR = ((vdisplay + 9) / 10) * 64`, `IER_FURIE`, then `GCR_ROTEN | GCR_LTDCEN` | `ltdc.c:1762-1784` | read from source |
| `GCR_ROTEN` is cleared only in the CRTC disable (`ltdc.c:1313-1314`), not when the plane returns to 0 | `ltdc.c:1313` | read from source |
| No `-EINVAL` about rotation exists in the plane path; the refusals are `DRM_WARN` plus `return` | `ltdc.c:1648-2042` | read from source |

## 4. Does the source explain a frame-to-frame changing CRC with reflect-x on the primary?

**Plainly: no.** Nothing in the driver treats a reflected layer differently from a plain one beyond sections 1 and 2. What was checked:

| Candidate | Finding | Where | Tag |
| --- | --- | --- | --- |
| CRC region that does not follow the mirror | The driver has no CRC region: `set_crc_source("auto")` sets `IER_LIE` and `GCR_CRCEN` and nothing else. The value is the controller's `LTDC_CCRCR` for the whole output | `ltdc.c:1488-1491` | read from source |
| When the value is read | `ltdc_irq` reads `LTDC_CCRCR` in the hard interrupt for every interrupt while the CRC is active; the thread reports it with `drm_crtc_add_crc_entry` only for a line interrupt, after skipping 2 frames. The line interrupt is placed at `accum_act_h + 1`, just after the active area. Separate interrupts (fifo, transfer) also re-read `ldev->crc` | `ltdc.c:1041-1056`, `1005-1025`, `1209` | read from source |
| What that implies | A CRC taken after the active area is of the frame just shown. It is stable for a static picture only if every frame is fetched identically; the driver cannot make it differ by itself | | inferred |
| Per-frame shadow reload | Written once per commit (`LXRCR_VBR`), the same for every transform. No reload is repeated per frame | `ltdc.c:2012-2015` | read from source |
| Rotator double buffer | Only in the 90/270 branch; not used for reflect-x or 180 | `ltdc.c:1738-1784` | read from source |
| Burst length | `LTDC_L1BLCR` is written with `max_burst_length`, which is 0 here (`st,burstlen` absent); identical for every transform | `ltdc.c:1963-1966`, `2655-2661` | read from source |
| FIFO and underrun | `LTDC_FUT` set at CRTC enable (`ltdc.c:1276`). Underrun and warning interrupts only increment counters (`ltdc.c:1027-1036`) and are logged at the *next* plane update (`ltdc.c:2019-2041`); nothing in the driver alters pixels on an underrun | | read from source |
| Fetch reversed | With HMEN the layer reads each line starting at the last byte (`ltdc.c:1848`) and, for reflect-y, lines from the end with a negative pitch (`ltdc.c:1861`). The line length is rounded by `(bus_width >> 3) - 1` (`ltdc.c:1857`). If the hardware fetches mirrored lines in a way that races the pixel clock (an underrun that corrupts a few pixels, or a burst alignment issue), the output and hence the CRC would change from frame to frame. The source has no test for this | | inferred (a hypothesis, not in the source) |
| Pattern in session B | Every unstable case (primary reflect-x, primary 180, overlay reflect-x + reflect-y) includes HMEN; reflect-y alone is not reported unstable. No code path is special to reflect-y alone | session B, `ltdc.c:1975-1977` | inferred |

**What settles it, using only what the source exposes.** The CRTC debugfs state dump prints `transfer_error`, `fifo_underrun_error`, `fifo_underrun_warning` and `fifo_underrun_rotation` (`ltdc.c:1529-1532`); an increasing underrun counter with reflect-x, and none without, points at the fetch. A correct steady mirror in the picture with a changing CRC would point at the CRC block instead. Source alone does not decide it.

## 5. Predicted pictures for `plane_commit`'s colour bars

The source buffer is eight vertical bars, left to right: white, yellow, cyan, green, magenta, red, blue, black (`crates/composition-drm/examples/plane_commit.rs`, `BARS` and `bar_at`). The mirror acts on the source rectangle; the destination window stays where it is. These rows describe that eight-bar pattern; other items may add an orientation pattern.

| Case | Predicted picture | Vertical bars tell it from rotation 0? | Tag |
| --- | --- | --- | --- |
| Primary rotate-180 | Left to right: black, blue, red, magenta, green, cyan, yellow, white (the vertical half of the flip is invisible) | Yes by the order, but not from reflect-x | inferred (from `ltdc.c:1695-1696`, `1847-1863`, `1976`) |
| Primary reflect-x | Same: black, blue, red, magenta, green, cyan, yellow, white | Yes | inferred (from `ltdc.c:1847-1848`, `1976`) |
| Overlay 42 reflect-x | The overlay window shows black, blue, red, magenta, green, cyan, yellow, white; the primary below it is not mirrored | Yes | inferred |
| Overlay 42 reflect-y | White, yellow, cyan, green, magenta, red, blue, black, as for rotation 0 | **No**: every row is identical, so a vertical flip is invisible | inferred (from `ltdc.c:1850-1851`, `1859-1863`) |
| Plane 49 reflect-x | As overlay 42 reflect-x, in plane 49's window | Yes | inferred (same code path) |
| Plane 49 reflect-y | As rotation 0 | **No** | inferred |
| Plane 49 reflect-x + reflect-y | As reflect-x | Yes, but not from reflect-x alone | inferred |

Consequences: vertical bars cannot show reflect-y, nor the vertical part of 180 or of reflect-x + reflect-y; separating those needs a pattern that changes along y. A mirrored picture that also tears, shifts a bar edge or shows colour noise points at the fetch (section 4); a steady mirror with a changing CRC points at the CRC read.
