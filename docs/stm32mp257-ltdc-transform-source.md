# STM32MP257 LTDC: how reflect-x, reflect-y, 180, 90 and 270 are programmed, with predicted pictures

Date of reading: 2026-11-28. **Nothing here is a board measurement and no picture has been taken.** Every row is tagged **read from source** (stated by the code read) or **inferred** (a conclusion drawn from it). This is the baseline for the Q6 picture session of [`stm32mp257-ltdc-session-2026-11-28.md`](stm32mp257-ltdc-session-2026-11-28.md).

## What was read, and the limit of this reading (read this first)

Tree: ST `drivers/gpu/drm/stm/ltdc.c`, repository `STMicroelectronics/linux`, branch `v6.6-stm32mp`, commit `548f960c059bc4b9165cb69895fd67551f6061ca` (the commit pinned by [`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md) and [`stm32mp257-ltdc-source-diff-6.6.md`](stm32mp257-ltdc-source-diff-6.6.md)). That diff document finds the plane path of v6.18-stm32mp (`3056405e`) and 6.6 to be the same code, so no separate v6.18 rows are added.

**Limit.** This item could not run `git clone`, `git fetch`, `gh` or `curl` (each was refused as needing approval), so no checkout exists in the dependency cache for this file. The file was read through a web-fetch agent (HTTP 200, raw file at the commit above). That agent returns no line numbers and would not reproduce code text; it gave paraphrases of what the code does. Consequences, stated plainly:

- **Behaviour rows below are the agent's paraphrase of the code, not text I read line by line.** Treat them as read from source at one remove; a second reader with a real checkout should confirm them (command at the end).
- **`file:line` is given only where an earlier document counted it on a real checkout of the same commit** (cited as "diff row N"). Where no earlier count exists the cell says "line not counted" and names the function and the register or macro to search for. The contract's "file:line for each row" is therefore **not fully met**.

## 1. Reflect-x, reflect-y and their combination (`ltdc_plane_update`)

All planes go through one function, `ltdc_plane_update` (shared by `ltdc_plane_atomic_update` and `ltdc_plane_atomic_enable`). There is no vertical-flip bit; reflect-y is a negative pitch.

| Step | What the code does | Where | Tag |
| --- | --- | --- | --- |
| Fold | Any `ROTATE_x` request is first turned into a combined rotate/reflect value; the user's `REFLECT_X` and `REFLECT_Y` are then toggled into it, so a reflect that the rotation already produced is cancelled by the user's bit | `ltdc.c:1684-1717` (diff row 59) | read from source |
| Reflect-x, address | Start address written to `LTDC_L1CFBAR` is the framebuffer address plus `cpp[0] * src_width - 1`, the last byte of the first line | `ltdc_plane_update`, line not counted (search `LTDC_L1CFBAR`) | read from source |
| Reflect-x, mirror bit | `LXCR_HMEN` ("horizontal mirroring enable") is ORed into the value written to `LTDC_L1CR` | `ltdc_plane_update`, line not counted (search `LXCR_HMEN`) | read from source |
| Reflect-x, chroma | For YUV with auxiliary planes, `LTDC_L1AFBA0R` and `LTDC_L1AFBA1R` get the matching half-width offset | line not counted | read from source |
| Reflect-y, address | `LTDC_L1CFBAR` moves forward by `pitches[0] * (src_height - 1)`: the first line fetched is the last line | line not counted | read from source |
| Reflect-y, pitch | The pitch field of `LTDC_L1CFBLR` (`LXCFBLR_CFBP`, bits 31:16) is written as `0x10000 - pitches[0]`, a negative 16-bit value. Chroma planes use `pitches[1]`, `pitches[2]` and a negated `LTDC_L1AFBLR` | line not counted (search `LXCFBLR_CFBP`) | read from source |
| Both | HMEN, last-byte offset, last-line offset and negative pitch all apply | as above | read from source |
| Always | Line length `LXCFBLR_CFBLL = cpp[0] * src_width + (bus_width >> 3) - 1`; `LTDC_L1CFBLNR = src_height`; base from `drm_fb_dma_get_gem_addr` | line not counted | read from source |
| Order | Fold, window position, pixel format, constant alpha, `LXBFCR` blending, `LTDC_L1CFBAR`, `LTDC_L1CFBLR`, `LTDC_L1CFBLNR`, YCbCr auxiliary registers, burst length, CLUT, `LXCR` (with HMEN) and scaler, `LTDC_L1CR` write, shadow reload, fifo warnings | line not counted | read from source |
| Checks | `ltdc_plane_atomic_check` (`ltdc.c:1566-1605`, diff row 35) holds no rotation or reflection test; the property mask (section 3) is the only filter | diff row 35 | read from source |
| Same for overlays | Reflect programming has no layer-index condition, so plane 42 and plane 49 use the same registers with their own layer base | | inferred |

## 2. The primary's 180

| Fact | Where | Tag |
| --- | --- | --- |
| 180 is mapped to `REFLECT_X | REFLECT_Y`, i.e. HMEN plus last-byte plus last-line start address plus negative pitch | `ltdc.c:1684-1717` (diff row 59) | read from source |
| The rotator is **not** used for 180: `GCR_ROTEN` is set only in the branch where the folded value contains `ROTATE_90`; no rotation memory is needed for 180 | line not counted | read from source |
| So 180 on the primary and reflect-x+reflect-y on an overlay are the same programming | | inferred |

## 3. 90 and 270, and where they are refused

| Fact | Where | Tag |
| --- | --- | --- |
| The property offers 90/180/270 only on the primary and only if `plane_rotation && crtc_rotation` when the property is created; overlays get `ROTATE_0 | REFLECT_X | REFLECT_Y` | `ltdc.c:2263-2276` (primary), `2310-2314` (overlays) (diff row 43) | read from source |
| Why overlay `rotate-180` is `EINVAL`: it is not in the overlay's property mask, so the DRM core refuses it before the driver is called | | inferred |
| `crtc_rotation` is cleared when the DT has no `rotation-memory` (none is set in any DT file read) | `ltdc.c:2820-2826` (diff row 44) | read from source |
| Ordering: the paraphrase says this clearing runs in `ltdc_load` **after** `ltdc_crtc_init` has created the property, so 90/270 stay on the primary's property while `crtc_rotation` is false. That would explain why session B's 90/270 passed `TEST_ONLY` although the earlier diff document expected the property to lack them. Not confirmed against a checkout | `ltdc_load`, line not counted | read from source (paraphrase), confirm |
| 90 = rotate-90 plus reflect-y; 270 = rotate-90 plus reflect-x; user reflects toggle on top | `ltdc.c:1684-1717` (diff row 59) | read from source |
| Refusal 1: another layer while `GCR_ROTEN` is set ("Rotation enabled on primary plane, do not use another layer") | `ltdc.c:1669-1682`, warn 1679 (diff row 58) | read from source |
| The rotation branch first clears `GCR_LTDCEN` and waits about two frames | `ltdc.c:1738-1743` (diff row 55) | read from source |
| Refusal 2: `rot_mem->size < hdisplay*vdisplay*2*3` ("Rotation buffer too small") returns with the controller left off | `ltdc.c:1753-1756` (diff row 56) | read from source |
| Refusal 3: `vdisplay > 1366` returns silently, controller left off | after row 56; `ltdc.c:1744-1765` in v6.18 numbering (`stm32mp257-source.md`) | read from source |
| Rotator setup: `LTDC_RB0AR` = reserved base, `LTDC_RB1AR` = base + size/2, `LTDC_RBPR` = `((vdisplay + 9) / 10) * 64`, `IER_FURIE` set, then `GCR_ROTEN` and `GCR_LTDCEN` | `ltdc.c:1762-1775`, `1782` (diff rows 60, 61) | read from source |
| `GCR_ROTEN` is cleared only in `ltdc_crtc_atomic_disable`, not when a plane goes back to 0 | line not counted | read from source (paraphrase) |
| `rot_mem` is dereferenced without a null check (paraphrase). With no `rotation-memory`, a **committed** 90/270 on this tree could therefore be a null dereference rather than a clean refusal. Nobody should commit one without a console | | inferred |
| No `-EINVAL` about rotation exists in `ltdc.c`; the refusals are warnings and early returns after the display was stopped. Nothing refuses after `TEST_ONLY` with an error code | diff row 35 | read from source |

## 4. Does the source explain a frame-to-frame changing CRC with reflect-x on the primary?

**Plainly: no.** Nothing in `ltdc_plane_update` or the interrupt code treats a reflected layer specially beyond the address, pitch and HMEN writes of section 1. What was checked:

| Candidate | Finding | Tag |
| --- | --- | --- |
| CRC region not following the mirror | The CRC is a hardware block on the LTDC output, enabled by `GCR_CRCEN` when the source is "auto"; the driver only reads `LTDC_CCRCR` and reports one value per frame after skipping the first two. It covers the output of the controller, not a layer, so a mirrored picture has a different but fixed CRC. The driver offers no region setting | read from source (paraphrase); which pipeline stage it covers is not visible in the driver |
| Per-frame shadow reload | On hardware 4.x each layer's `LXRCR` is written with the vblank-reload flag at the end of `ltdc_plane_update`; this happens once per commit, not per frame, and is the same for every transform | read from source (paraphrase) |
| Rotator double buffer | Used only in the 90/270 branch (`RB0AR`/`RB1AR`); not involved in reflect-x or 180 | read from source |
| Burst length | Written on MP25 as `max_burst_length`, 0 because `st,burstlen` is absent; the same for every transform | read from source (paraphrase; `ltdc.c:1970-1972` in v6.18 numbering, `stm32mp257-ltdc-source-full.md`) |
| FIFO threshold and underrun | `LTDC_FUT` is set when the CRTC is enabled; underruns are only counted and logged by the IRQ thread. Nothing changes the pixel data on an underrun in the driver | read from source (paraphrase) |
| Fetch direction | A mirrored fetch starts at the last byte of a line and the line length is rounded with `(bus_width >> 3) - 1`; the layer reads each line backwards (HMEN) and, for reflect-y, lines from the end with a negative pitch. If the fetch FIFO or its burst alignment misbehaves for a backwards read, the picture could change between frames and so would the CRC. The source cannot confirm it; it is a hypothesis for a hardware question | inferred |
| Same symptom on overlay reflect-x+reflect-y and primary 180/reflect-x, stable on reflect-y alone | All unstable cases contain HMEN; the paraphrase shows no code path special to reflect-y alone. Both observations are session B's, not rechecked here | inferred |

**What a picture can settle.** If the picture is a correct, steady mirror while the CRC changes, the CRC is the culprit (a measurement artefact of the CRC block with HMEN) and the transform may still be usable. If the picture flickers, tears or shows colour or line corruption, the fetch is the culprit. Source alone cannot decide.

## 5. Predicted pictures for `plane_commit`'s colour bars

The source buffer is eight vertical bars, left to right: white, yellow, cyan, green, magenta, red, blue, black (`crates/composition-drm/examples/plane_commit.rs`, `BARS`, `bar_at`). The mirror acts on the framebuffer rectangle (`src`) and the destination window stays where it is. With mirroring on the picture should show the bars in the order below. Other items change the pattern to show orientation; these rows describe the eight-bar pattern only.

| Case | Predicted picture | Tell apart from rotation 0 by vertical bars? | Tag |
| --- | --- | --- | --- |
| Primary rotate-180 (reflect-x + reflect-y) | Bars left to right: black, blue, red, magenta, green, cyan, yellow, white. The vertical flip cannot be seen | Yes, by the order, but not distinguishable from reflect-x | inferred (from the programming in sections 1-2) |
| Primary reflect-x | Same order as 180: black, blue, red, magenta, green, cyan, yellow, white | Yes | inferred |
| Overlay 42 reflect-x | The overlay's own window shows black, blue, red, magenta, green, cyan, yellow, white; the primary below it is unchanged | Yes | inferred |
| Overlay 42 reflect-y | Looks exactly like rotation 0: white, yellow, cyan, green, magenta, red, blue, black | **No**: every row is the same, so a vertical flip is invisible | inferred |
| Plane 49 reflect-x | As overlay 42 reflect-x, in plane 49's window | Yes | inferred (same code path, section 1) |
| Plane 49 reflect-y | As rotation 0 | **No** | inferred |
| Plane 49 reflect-x + reflect-y | As reflect-x (and as 180) | Yes, but cannot be told from reflect-x alone | inferred |

Consequences for the session: vertical bars cannot show reflect-y at all, nor the vertical half of 180 and of reflect-x+reflect-y; reflect-y needs a pattern that changes along y (the orientation pattern another item is adding). A picture that shows the reflect-x order but also tearing, a shifted bar edge, or colour noise points at the fetch (section 4), and a steady picture with a changing CRC points at the CRC block.

## Confirm with a real checkout

```sh
git clone --depth 1 --branch v6.6-stm32mp --filter=blob:none --no-checkout \
  https://github.com/STMicroelectronics/linux "$CONNECTED_DEPS_DIR/st-linux-6.6"
git -C "$CONNECTED_DEPS_DIR/st-linux-6.6" show HEAD:drivers/gpu/drm/stm/ltdc.c | grep -n 'HMEN\|CFBAR\|CFBLR\|ROTEN\|REFLECT\|rot_mem'
```

Then replace each "line not counted" with the number and drop the "paraphrase" qualifiers.
