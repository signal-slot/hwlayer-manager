# vc4 HVS5 / HVS6 scaler limits: what the source says, and what the board sweeps should try

Dated 2026-11-28. Answers the source half of `docs/hvs-backend-design.md` §7 Q1 (ScaleLimits of BCM2711 HVS5 and BCM2712 HVS6; on the boards only 2x up and 1/2 down were tried). It claims no hardware result: nothing here was run on a board. The board answer comes from the `scale_probe` runs in `rpi5-hvs6-session` and `cm4-hvs5-session`. Design doc: [hvs-backend-design.md](hvs-backend-design.md).

## Provenance and how far the evidence goes

- **6.18 pin**: `raspberrypi/linux`, branch `rpi-6.18.y`, commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e`, `drivers/gpu/drm/vc4/` ([tree at the pin](https://github.com/raspberrypi/linux/tree/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4)). The `file:line` rows below for the pin are those of the earlier full read in [vc4-hvs-scaling-source.md](vc4-hvs-scaling-source.md), which read the raw files at that SHA. In this session the raw files could not be downloaded to the dependency cache (network access from the shell was refused), so the pin's function bodies were re-read only through a page fetch that returns text without line numbers and was cut off after about 40000 characters of each file. Result: the 6.18 line numbers are **not re-verified here**; what was re-read at the pin matches them (`vc4_plane_allocate_lbm` error paths). Rows tagged "read from source" for 6.18 therefore mean: read in `vc4-hvs-scaling-source.md` at the pin.
- **6.12**: `raspberrypi/linux`, branch `rpi-6.12.y`, tip `8f5f2e15840330091c7116b2ef9454108d4bc423` (commit date 2026-11-23), files `vc4_plane.c` and `vc4_hvs.c`, read as raw files by function name. **No line numbers**: the fetch returned unnumbered text, and numbers are not guessed. The CM4 runs `6.12.47+rpt-rpi-v8`; the tip is later than that, so a 6.12 row is evidence for the 6.12 series, not proof for the CM4's exact build (that is the job of `docs/vc4-rk-kernel-compare.md`, item kernel-source-compare).
- The sparse clones into `$CONNECTED_DEPS_DIR/rpi-linux-6.18` and `rpi-linux-6.12` were **not made** (git and curl from the shell were refused). A later session with network access should fetch them and confirm the 6.18 line numbers and add 6.12 line numbers.
- Tags: **read from source** (branch, commit, function or file:line as said above) and **inferred** (derived from those rows). Generation names: HVS5 = `VC4_GEN_5` = BCM2711; HVS6 = `VC4_GEN_6_C`/`VC4_GEN_6_D` = BCM2712.

## 1. What decides the limit (the same code shape on both generations)

| Step | Finding | Tag | Where |
| --- | --- | --- | --- |
| Core helper | `drm_atomic_helper_check_plane_state` is called with `min_scale = 1`, `max_scale = INT_MAX`: the DRM helper adds no ratio limit. Same arguments at 6.12 | read from source (6.18 pin `vc4_plane.c:593-594`; 6.12 `vc4_plane_setup_clipping_and_scaling`) | both |
| Mode per axis | `NONE` if `dst == src >> 16`; `TPZ` if the source is at most one pixel; `PPF` if `3 * dst >= 2 * (src >> 16)` (downscale up to 1.5x, or any upscale); else `TPZ`. Identical code at 6.12 (`vc4_get_scaling_mode`) | read from source (6.18 `vc4_plane.c:329-342`; 6.12 by function) | both |
| Only refusal in the setup function | `-EINVAL` for a missing CRTC state, and from `vc4_plane_margins_adj` for margins that cover the mode or for an empty destination. No ratio check there | read from source (6.12 by function; 6.18 `vc4_plane.c:593-621`) | both |
| Upscale | `TPZ` upscale writes a constant scale (`(1<<16)+1`) and `PPF` upscale gives `scale < 0x10000`: neither has a ratio field that can overflow. The only upscale bound is the destination rectangle (13-bit width and height fields) and a source of at least one pixel | read from source (6.18 `vc4_plane.c:707-713`; 6.12 `vc4_write_tpz`), bound inferred | both |
| Downscale up to 1.5x | `PPF`; the 17-bit scale field holds up to 1.99998x, so it cannot overflow | read from source (6.18 `vc4_regs.h:1348`), inferred | both |
| Downscale beyond 1.5x | `TPZ`; the 21-bit scale field holds up to 31.99998x | read from source (6.18 `vc4_regs.h:1333`), ratio inferred | both |

## 2. Per generation: limits and what fails beyond them

### HVS5 (BCM2711, `VC4_GEN_5`)

| Quantity | 6.18 pin | 6.12 tip | Tag |
| --- | --- | --- | --- |
| Upscale limit | none in the driver; bounded by destination width field (`crtc_w` at most 8191) | same | read from source / inferred |
| Downscale, horizontal | **-EINVAL** in atomic_check when a TPZ axis has `src_w / crtc_w > 0x1FFFFF` (31.99998x), luma and chroma (`vc4_plane.c:686-690`) | **no such check**: `vc4_write_tpz` is `void`, writes `scale = src / dst` into `SCALER_TPZ0_SCALE` unchecked | read from source |
| Downscale, vertical | -EINVAL when `src_h / (vdownsample * crtc_h) > 0x1FFFFF` (`vc4_plane.c:1446-1452`); line skipping (`vdownsample`, only if ratio > 3x) raises the usable ratio to 31.99998 x (1 for T-tiled, up to 8 for SAND, `max_pitch / pitch` for linear) | **no `vc4_plane_set_vdownsample`, no `max_pitch`, no `max_tile_skip`, no refusal**: the whole vertical path is `vc4_get_scaling_mode` plus LBM sizing | read from source |
| Source width | -EINVAL when the fetched span exceeds 8191 (`vc4_plane.c:1725-1731`) | no width or height check in `vc4_plane_mode_set`: values go into the dlist fields unchecked | read from source |
| LBM capacity | 60 K words (61440), 64 handles (`vc4_hvs.c:1693-1711`, `vc4_drv.h:304`) | 60 K words, same constant (`__vc4_hvs_alloc`) | read from source |
| LBM need | zero if both y axes are `NONE`; otherwise `__vc4_lbm_size`: 8 or 16 words-units per pixel by TPZ or PPF and RGB or YUV, rounded to 128, divided by 4, aligned to 64 | same function at 6.12 | read from source |
| LBM failure | `vc4_plane_allocate_lbm` returns the error of `ida_alloc_range` ("Out of lbm_handles") or `drm_mm_insert_node_generic` ("Failed to allocate LBM entry"), typically `-ENOSPC`, from atomic_check, so `TEST_ONLY` fails. A zero `lbm_offset` gives -EINVAL | read from source (6.18 `vc4_plane.c:1146-1165`; 6.12 by function), errno value inferred | both |

### HVS6 (BCM2712, `VC4_GEN_6_C`/`VC4_GEN_6_D`)

| Quantity | 6.18 pin | 6.12 tip | Tag |
| --- | --- | --- | --- |
| Upscale limit | none in the driver; destination width at most 8192 (13-bit field holding `crtc_w - 1`) | same code path (`vc6_plane_mode_set` exists at 6.12) | read from source / inferred |
| Downscale, horizontal | same -EINVAL at 31.99998x (`vc4_plane.c:686-690`, the check is generation independent) | no check | read from source |
| Downscale, vertical | same -EINVAL form; `max_pitch` 131071, `max_tile_skip` 8 (`vc4_plane.c:2087-2088`); linear 1920-wide XRGB (pitch 7680) allows 17 line skips, so a vertical limit of about 544x | none | read from source / inferred |
| Source width | no check; `width - 1` goes into a 13-bit field, so wider sources truncate silently (`vc4_plane.c:2289-2296`) | no check | read from source |
| LBM capacity | 1024 units, hard-coded (the register is not read) (`vc4_hvs.c:1693-1711`) | `lbm_size = 1024` for `GEN_6_C` and `GEN_6_D` | read from source |
| LBM need | `__vc6_lbm_size`: width x words-per-component (4 PPF, 2 TPZ) x components (up to 4), per 128-component line. 1920-wide ARGB, PPF vertical: 240 of 1024 | same functions at 6.12 (`vc4_lbm_channel_size`, `__vc6_lbm_size`) | read from source (formula), number inferred |
| LBM failure | same allocator errors as HVS5 | same | read from source |
| UPM (every plane, not only scaled ones) | `Out of upm_handles` / `drm_mm` failure from atomic_check | `vc6_plane_allocate_upm` exists | read from source |

Resulting boundary table (inferred from the rows above; the thing the boards must confirm):

| Axis and path | HVS5 at 6.18 | HVS6 at 6.18 | HVS5 at 6.12 | HVS6 at 6.12 |
| --- | --- | --- | --- | --- |
| Upscale | no ratio limit; stops at dst size 8191 | no ratio limit; stops at dst size 8192 | same as 6.18 | same as 6.18 |
| Horizontal downscale | accepted up to 31.99998x, -EINVAL above | same | no -EINVAL; above 31.99998x the field is truncated (`VC4_SET_FIELD` only warns), so `TEST_ONLY` accepts a wrong scale | same |
| Vertical downscale, T-tiled | 31.99998x | T-tiled rejected on HVS6 | no refusal | no refusal |
| Vertical downscale, linear | about 8 x 31.99998 for a 7680-byte pitch | about 17 x 31.99998 | no refusal | no refusal |
| Sources of 8192 or more wide | -EINVAL above 8191 | truncated, no -EINVAL | truncated, no -EINVAL | truncated |
| Failure by LBM | `-ENOSPC`-type error when 61440 words or 64 handles run out | when 1024 units or 64 handles run out; a single 8191-wide PPF ARGB layer needs about 1024 (full) | same allocator | same allocator |

A large downscale does not by itself raise the LBM need on a TPZ axis (the TPZ size uses `crtc_w`, the smaller width); it grows with the destination width and with PPF. So the LBM can reject a wide, mildly scaled layer (PPF vertical, 1.0x to 1.5x) and accept a much more strongly downscaled one (inferred from `__vc4_lbm_size` and `vc4_lbm_channel_size`).

## 3. Differences between 6.12 and 6.18

All read from source (6.12 branch `rpi-6.12.y` at `8f5f2e15840330091c7116b2ef9454108d4bc423`, 6.18 pin `16ae2649dcfadf9d81445b1177a3191efa3cb82e`):

1. **TPZ scale overflow check**: present at 6.18 (`vc4_plane.c:686-690`), absent at 6.12. A downscale beyond 31.99998x is -EINVAL on 6.18 and silently programmed with a truncated field on 6.12.
2. **Vertical line skipping** (`vc4_plane_set_vdownsample`, the `> 3x` trigger, `max_pitch` and `max_tile_skip`, and its -EINVAL): present at 6.18, absent at 6.12.
3. **Gen 5 source width check** (8191): present at 6.18, absent at 6.12.
4. **Gen 6 modifier acceptance**: at 6.18 `vc4_format_mod_supported` rejects T-tiled and SAND64 on gen 6 (`vc4_plane.c:2767-2771`); at 6.12 it has no generation test and the rejection happens later in `vc6_plane_mode_set` (default case, -EINVAL). Not a scale difference, but it changes what `TEST_ONLY` reports for those modifiers.
5. LBM sizing, mode choice (`vc4_get_scaling_mode`), LBM capacities and the LBM allocator are the same.

Consequence (inferred): the RPi5 (6.18.50+rpt) should follow the 6.18 column if its build carries these commits; the CM4 (6.12.47+rpt) may follow the 6.12 column, depending on whether its build is before or after the commits that added items 1 to 3. The board sweep decides; whichever way it falls, it also tells which side of the 6.12 series the CM4's build lies on.

## 4. Comparison with `crates/composition-drm/src/profile/vc4.rs` (read only, not edited)

The profile (`Vc4Hvs5`, `Vc4Hvs6`; `crates/composition-drm/src/profile/vc4.rs`, function `apply_vc4`) sets `max_upscale = 32 << 16` and `max_downscale = 2 << 16` on every non-cursor plane, with `max_source` 8191 x 7680 (HVS5) and 8192 x 8192 (HVS6).

| Profile claim | Source says | Verdict (inferred) |
| --- | --- | --- |
| Upscale to 32x | No driver limit; destination size only | Conservative: sources need not be limited to 32x. Safe for the planner; loses a layer only when a client wants more than 32x |
| Downscale to 2x | 6.18: 31.99998x horizontal, up to about 8 to 17 times that vertically for linear. 6.12: no refusal | Very conservative at 6.18; at 6.12 the driver would accept values the hardware renders wrongly, so a limit is still wanted. `PPF` covers only up to 1.5x, `TPZ` (lossy, trapezoid) is used from there up, which may be why 2x was chosen: it is inside `TPZ` and the first ratio the boards could prove |
| `max_source` HVS5 8191 wide | -EINVAL above 8191 at 6.18; absent at 6.12 | Matches 6.18 |
| `max_source` HVS6 8192 | No check; silent truncation | Matches the field width |
| LBM groups (HVS5 capacity 8, HVS6 capacity 4) | Formula-derived for 1920-wide layers | Unaffected by the ratio sweep except that a wide, PPF-scaled layer uses most of the LBM |

Nothing in the source contradicts the profile: it stays inside what the driver accepts. The open question is only how much headroom the profile leaves, which matters when a client wants a stronger downscale (for example a 4K source in a 1080p window needs 2.0x per axis, exactly the profile's edge).

## 5. Proposed sweeps for the board sessions

First a limit of the probe: `scale_probe --max-factor` controls only the **upscale** ladder (1, 1.25, 1.5, 2, ... 32, then doubling). The **downscale** test is fixed at 0.5, 0.75 and 0.9 (`crates/composition-drm/examples/scale_probe.rs`, module comment, "Downscale: 0.5, 0.75 and 0.9"). So the source's downscale boundaries (31.99998x, and above) cannot be reached by `--max-factor`; a downscale ladder needs a new example (the plan forbids changing the existing ones), or a manual atomic request. This is a finding for the planner, not something this item changes.

What `--max-factor` can do (all inferred as proposals):

| Session | Command shape | Purpose |
| --- | --- | --- |
| `rpi5-hvs6-session` | `scale_probe --max-factor 32` (default) on one plane per class, then `--max-factor 256` with `--dst` set to the mode size | Default repeats the ladder end seen on 2026-11-27 (32x all accepted). The 256 run goes to 64, 128, 256 and the one-pixel-source factor (the mode size, for example 1920x or more): source expects acceptance up to the destination size on HVS6; a rejection is a new fact |
| `cm4-hvs5-session` | same two runs | Same expectation (upscale has no limit at either kernel series) |
| both | `--max-factor 4096` once, only if both runs above end with `+`, `--dst` at the full mode size | Finds whether the destination field (8191 / 8192) or a hardware limit ends the ladder |

The downscale ratios to try, if a downscale ladder is added (source expectations, ratio = source size divided by destination size per axis; a source of 1920 wide needs a destination at least 60 wide for 32x):

| Ratio | Why | Expected at 6.18 (RPi5) | Expected at 6.12 (CM4) |
| --- | --- | --- | --- |
| 1.5 | last `PPF` ratio (mode boundary `3 * dst >= 2 * src`) | accepted | accepted |
| 1.51 | first `TPZ` ratio | accepted | accepted |
| 2 | the profile's limit | accepted | accepted |
| 3 | `TPZ`, vertical line skipping starts above 3x | accepted | accepted |
| 4, 8, 16 | inside the TPZ field | accepted | accepted |
| 31 | just inside the TPZ field | accepted | accepted |
| 32 | field maximum is 31.99998x, so this is just outside | -EINVAL horizontal; vertical accepted for linear | accepted (no check), wrongly rendered: needs a KVM picture on the RPi5 only, the CM4 has no camera |
| 33, 64 | outside the field | -EINVAL horizontal; vertical accepted while the line skip covers it | accepted on 6.12 |
| 300 vertical, linear, 1920 wide | beyond 8 x 32 on HVS5, inside 17 x 32 on HVS6 | HVS5 -EINVAL; HVS6 accepted | accepted |
| 600 vertical, linear, 1920 wide | beyond 17 x 32 | -EINVAL on both | accepted |

The horizontal and vertical axes should be run separately (`h`, `v`, `both` as the existing probe does) and with a linear XRGB buffer first; a NV12 run adds the chroma refusals. LBM exhaustion needs a different run: several full-width layers with PPF vertical scaling (ratio 1.0x to 1.5x) at once, ten or more on HVS5 for the 61440 words, five on HVS6 for the 1024 units (inferred from the worked numbers in [vc4-hvs-scaling-source.md](vc4-hvs-scaling-source.md) section 2.4). That is a `kms_sweep`-style run, not a `scale_probe` run.

Reading the outcome: a refusal at ratio `r` on the CM4 and not at 6.12 tip tells that the CM4's build carries the TPZ check; an acceptance at 33 tells it does not. Either is evidence for `docs/vc4-rk-kernel-compare.md`.

## 6. For the later items

- `hvs-design-q-status` can mark Q1 **answered** only after the board sweeps; until then it is "still open, source expects": upscale unlimited to the destination size; downscale 31.99998x horizontal at 6.18, unchecked at 6.12.
- `profile-tighten` should not raise `max_downscale` on the strength of this file alone. If the boards confirm 32x on the RPi5, a per-generation or per-kernel value is a decision for the profile's owner, since 6.12 has no refusal at all.
- No board address, host name, serial number or credential is in this file.
