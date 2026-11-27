# vc4 HVS load and core-clock computation (rpi-6.18.y)

Answers open question 2 (bandwidth) of `docs/hvs-backend-design.md` §7 as far as the kernel
source allows. Nothing here is observed on a board; every statement is tagged **read from source**
(with `file:function`) or **inferred**.

## 1. Source read

- Tree: Raspberry Pi kernel, branch `rpi-6.18.y`.
- Commit read: `16ae2649dcfadf9d81445b1177a3191efa3cb82e`
  ("media/hevc_d: Change warning messages to debug for control misconfiguration", committed
  2026-11-26). Tag: read from source (branch tip resolved through the GitHub API on 2026-11-28).
- Files read at that commit, as raw text from
  https://raw.githubusercontent.com/raspberrypi/linux/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4/
  (`vc4_plane.c`, `vc4_kms.c`, `vc4_hvs.c`, `vc4_crtc.c`, `vc4_drv.h`). The files were fetched
  individually over HTTPS, not from a clone (git was not usable on the worker), and were fetched
  from the branch name, which pointed at this commit at that time. `vc4_txp.c` was not read.
- Line numbers are not given; references are `file:function`.
- Generations: `enum vc4_gen` is `VC4_GEN_4`, `VC4_GEN_5`, `VC4_GEN_6_C`, `VC4_GEN_6_D`
  (`vc4_drv.h`). BCM2711 is `VC4_GEN_5` (HVS5), BCM2712 is `VC4_GEN_6_C`/`VC4_GEN_6_D` (HVS6).
  The mapping of SoC to enum value is inferred from the design doc; the enum itself is read from
  source.

## 2. Per-plane load

`vc4_plane.c:vc4_plane_calc_load` (read from source) computes two `u64` values stored in
`struct vc4_plane_state` (`vc4_drv.h`, read from source):

| Field | Unit (comment in `vc4_drv.h`) | Meaning |
|---|---|---|
| `membus_load` | bytes/sec | memory bandwidth the plane needs |
| `hvs_load` | HVS cycles/sec | load on the HVS block |

Variables (all read from source, `vc4_plane_calc_load`):

- `vrefresh` = `drm_mode_vrefresh(&crtc_state->adjusted_mode)` of the plane's CRTC.
- `src_w[i]`, `src_h[i]` = source size of fb plane `i`, 16.16 fixed point (`>> 16` gives pixels).
- `crtc_w`, `crtc_h` = destination size in pixels.
- `cpp[i]` = bytes per pixel of fb plane `i` (`fb->format->cpp[i]`).
- `num_planes` = `fb->format->num_planes` (1 for RGB, 2 or 3 for YUV).
- `x_scaling[0..1]`, `y_scaling[0..1]` = per-axis scaling mode; `VC4_SCALING_NONE` means unscaled.

Formula (read from source):

```
hvs_load_shift = 1 if any of x_scaling[0], x_scaling[1], y_scaling[0], y_scaling[1]
                   != VC4_SCALING_NONE
                 else 2
for each fb plane i:
    vscale_factor = ceil((src_h[i] >> 16) / crtc_h)
    membus_load  += (src_w[i] >> 16) * (src_h[i] >> 16) * vscale_factor * cpp[i]
    hvs_load     += crtc_h * crtc_w
hvs_load  = (hvs_load * vrefresh) >> hvs_load_shift
membus_load = membus_load * vrefresh
```

Notes:

- The source comment says the HVS handles 2 pixels/cycle when scaling and 4 otherwise
  (`vc4_plane_calc_load`, read from source). That is the `hvs_load_shift`.
- `hvs_load` sums `crtc_h * crtc_w` once per fb plane, so a 2- or 3-plane YUV framebuffer counts
  its destination area two or three times (read from source: the `+=` is inside the per-plane loop).
- `vscale_factor` is only a vertical downscale factor; it is 1 for upscaling or 1:1, since
  `DIV_ROUND_UP` of a smaller value over a larger one is 1 (derived from the code, inferred only
  for the zero-source edge case below). The source comment says it deliberately over-estimates.
  There is no horizontal factor.
- Callers: only `vc4_plane_mode_set` (gen 4/5) and `vc6_plane_mode_set` (gen 6), at their end,
  both reached from `vc4_plane_atomic_check` (read from source). A plane with a zero source or
  destination returns before the call, so its loads keep the value they had in the duplicated
  state (inferred: the duplicate is a `kmemdup` of the whole state per the agent's reading of
  `vc4_plane_duplicate_state`; not re-verified against the full body).
- The load fields are not copied by `vc4_plane_atomic_async_update` (read from source), so an
  async page flip leaves the previous load in the tracker.

## 3. Aggregation: the load tracker

`vc4_kms.c:vc4_load_tracker_atomic_check` (read from source):

- For every plane in the atomic state, subtracts the old plane state's `membus_load`/`hvs_load`
  from `struct vc4_load_tracker_state` when the old state had both `fb` and `crtc`, and adds the
  new plane state's the same way. CRTCs with `feeds_txp` (writeback) are skipped.
  `struct vc4_load_tracker_state` is a `drm_private_obj` state, so the totals persist across
  commits. The sums are global over all CRTCs, not per CRTC.
- Then `if (!vc4->load_tracker_enabled) return 0;`
- Otherwise it enforces two limits, both returning `-ENOSPC`:
  - `membus_load > SZ_1G + SZ_512M` (1.5 GiB/s; the comment says the absolute limit is 2 GB/s);
  - `hvs_load > 240000000` (the comment says the HVS runs at 250 MHz, minus a margin).

Where the tracker runs and when it enforces (read from source):

- `vc4_kms.c:vc4_atomic_check` calls `drm_atomic_helper_check`, then
  `vc4_load_tracker_atomic_check`, then `vc4_core_clock_atomic_check`, with no generation test.
  Both `vc4_mode_funcs` and `vc5_mode_funcs` use it. The accumulation therefore runs on every
  generation.
- `vc4_kms.c:vc4_kms_load` sets `load_tracker_enabled = true` only for `VC4_GEN_4`. Its comment
  says the limits "aren't relevant for the BCM2711", but the computations are used for the core
  clock rate.
- `vc4_hvs.c:vc4_hvs_debugfs_init` creates the debugfs bool `hvs_load_tracker`
  (`&vc4->load_tracker_enabled`) only for `VC4_GEN_4`.

| | `VC4_GEN_4` | `VC4_GEN_5` (BCM2711) | `VC4_GEN_6_C/D` (BCM2712) |
|---|---|---|---|
| plane load computed | yes | yes | yes |
| totals accumulated | yes | yes | yes |
| limits enforced in `atomic_check` | yes (default on) | no (flag stays false) | no (flag stays false) |
| debugfs `hvs_load_tracker` | yes | no | no |

All cells: read from source. Consequence: on gen 5 and gen 6 a commit is never rejected for load,
and `TEST_ONLY` cannot reveal a bandwidth or HVS-cycle overload (inferred from the table; it
agrees with the design doc's source-tagged row).

## 4. Core clock

CRTC load, `vc4_crtc.c:vc4_crtc_atomic_check` (read from source, no generation test): if the CRTC
has an encoder, `vc4_crtc_state->hvs_load` is `max(crtc_clock * hdisplay / htotal + 8000,
crtc_clock * 9 / 10) * 1000` for the HDMI0 encoder type, and `crtc_clock * 1000` otherwise
(`crtc_clock` in kHz, so the result is pixels/sec).

Core-clock rate, `vc4_kms.c:vc4_core_clock_atomic_check` (read from source, no generation test):

```
fifo_load[ch]  = vc4_crtc_state->hvs_load for each active CRTC (0 for CRTCs going inactive)
cob_rate       = max over in-use channels of fifo_load[ch]
pixel_rate     = load_tracker.hvs_load * (num_outputs > 1 ? 40 : 60) / 100
core_clock_rate = max(cob_rate, pixel_rate)      -> vc4_hvs_state.core_clock_rate
```

`load_tracker.hvs_load` is the plane sum of §3. `num_outputs` is the count of in-use channels.
The 40 % and 60 % factors have no explanation in the source (inferred only that they are
empirical).

Applying it, `vc4_kms.c:vc4_atomic_commit_tail` (read from source):

- Both clock requests are inside `if (vc4->gen == VC4_GEN_5 && !vc4->firmware_kms)`.
  Before the modeset:
  `core_rate = min(max(old.core_clock_rate, new.core_clock_rate), hvs->max_core_rate)`; if any CRTC
  needs a modeset, `core_rate` is clamped up to at least 500 MHz (and at most `max_core_rate`).
  After the commit: `core_rate = min(max_core_rate, new.core_clock_rate)`.
- `vc4_hvs_set_min_core_rate` skips the call when the value equals `hvs->core_clk_min_rate`, else
  calls `clk_set_min_rate` on both `hvs->core_clk` and `hvs->disp_clk`. It requests a minimum, so
  the clock framework may run faster; the actual rate is only logged with `drm_dbg`.
- `VC4_GEN_6_C/D` takes no branch: the computed `core_clock_rate` is stored in the HVS state and
  never applied in `vc4_kms.c` (read from source). `vc4_hvs.c` has no `clk_set_min_rate` or
  `clk_set_rate` besides `vc4_hvs_unbind` (min rate 0). Whether firmware or another driver sets
  the gen 6 clock is not visible in these files (unknown).
- `hvs->max_core_rate` is set in `vc4_hvs.c:vc4_hvs_bind` for gen >= 5 from
  `rpi_firmware_clk_get_max_rate(RPI_FIRMWARE_CORE_CLK_ID)`; `core` and `disp` clocks are enabled
  there. The same function derives `vc5_hdmi_enable_hdmi_20` (max rate >= 550 MHz) and
  `vc5_hdmi_enable_4096by2160` (>= 600 MHz) (read from source).
- With `0 &&` in front, a block in `vc4_atomic_commit_tail` that masks underrun for gen < 6 is dead
  code (read from source).
- A failed `clk_set_min_rate` is only `WARN_ON`; the commit proceeds (read from source).

So on gen 5 an excessive load raises the requested clock up to the firmware maximum and is then
silently capped; on gen 6 nothing is requested. Neither fails a commit (inferred from the above).

## 5. What a backend estimate in `DrmBackend::check` would need

Why it would be an estimate: the kernel does not reject on gen 5/6 (§3), so `TEST_ONLY` gives no
answer; a backend check could only warn or reject on its own policy (inferred).

Inputs (all available in the commit being checked, inferred):

| Input | Source in the backend |
|---|---|
| `vrefresh` | the CRTC mode, `(clock * 1000) / (htotal * vtotal)` with the kernel's `drm_mode_vrefresh` rounding |
| `src_w`, `src_h` per plane | the window's source rectangle |
| `crtc_w`, `crtc_h` | the window's destination rectangle |
| `cpp[i]` and `num_planes` | the buffer's fourcc (needs a per-format table including YUV chroma planes) |
| scaling on/off | whether source size differs from destination size per axis; the kernel's `x_scaling`/`y_scaling` may also pick a non-`NONE` mode for 1:1 YUV (chroma upsampling), which is unknown here |
| output channels | number of active CRTCs |
| CRTC pixel clock, htotal, hdisplay | for the `cob_rate` term |

Formula: the same as §2 per window, summed over all windows of all active CRTCs except
writeback, with the §4 `pixel_rate`/`cob_rate`/`core_clock_rate` for the clock estimate
(inferred; the formulas themselves are read from source).

Unknown threshold (inferred):

- Gen 4 limits (1.5 GiB/s membus, 240 Mcycles/s) are enforced only on gen 4; the kernel states they
  are not relevant for BCM2711. No gen 5 or gen 6 limit exists in the source.
- The usable bound on gen 5 is `max_core_rate` (firmware, read at bind time, not in source as a
  constant) against the estimated `core_clock_rate`, and the real DRAM bandwidth, which is board
  and workload dependent. The gen 6 clock is not controlled by this driver. No number can be
  derived from the source; a backend would have to take it from a board measurement (an underrun
  boundary) or leave the check as a warning only.
- Two documented model weaknesses: `membus_load` over-estimates on downscale (by design) and
  ignores modifiers/compression and caching; `hvs_load` ignores the per-plane dlist cost and
  LBM/UPM stalls.

Conclusion for open question 2 (inferred): bandwidth is not testable through `TEST_ONLY` on gen 5
or gen 6, the load formula is simple enough to reproduce in the backend, and the missing piece
is a threshold that only a board run can supply.
