# STM32MP257 LTDC: sync semantics read from source

Status: **partly read.** Only the ST `ltdc.c` could be read. Mainline and `drv.c`
could not be fetched (see the fetch record), so those rows are open.

Nothing in this document is measured on hardware. No board was accessed.

## Fetch record (2026-11-28)

| Tree / file | Result |
|---|---|
| STMicroelectronics/linux, branch `v6.18-stm32mp`: `git clone --depth 1 --branch v6.18-stm32mp https://github.com/STMicroelectronics/linux` | Denied by the sandbox's approval layer ("nobody decided in time"). It never ran, so there is no HTTP or git error code. **No commit hash was obtained and none is pinned.** |
| ST `drivers/gpu/drm/stm/ltdc.c`, branch `v6.18-stm32mp`, via `curl` of the `raw.githubusercontent.com` URL | Fetched, HTTP 200, 2941 lines. This is the branch tip on 2026-11-28, with no commit hash. All `ltdc.c:N` line numbers below refer to that file. |
| ST `drv.c` | Not fetched. Only the one `ltdc.c` URL was approved. |
| Mainline `torvalds/linux` `ltdc.c` / `drv.c` at a named tag | Not fetched, for the same reason. No comparison was possible. |

Every row below is therefore pinned to "the `v6.18-stm32mp` branch as served on
2026-11-28", not to a commit. Rerun with a real checkout to pin hashes.

## Sync semantics

Tags: `source` = read in the ST `ltdc.c`; `inferred` = reasoned from the source or
the DRM core, not read directly; `not read` = needs a file I did not get.

### 1. Page-flip completion

| Finding | Tag | Where (`ltdc.c`) |
|---|---|---|
| `atomic_flush` is the only place a flip event is armed. If the commit carries an event, it takes `drm_crtc_vblank_get` and calls `drm_crtc_arm_vblank_event`; if vblank is unavailable it calls `drm_crtc_send_vblank_event` at once. | source | 1346-1369 (arm at 1364, immediate send at 1366) |
| The armed event is completed by the DRM core when `drm_crtc_handle_vblank` runs. The driver's threaded IRQ handler calls it on the line interrupt flag `ISR_LIF`, if `vblank_active`. The driver does not call `drm_crtc_send_vblank_event` itself from the IRQ path. | source | 1004-1024 (call at 1013) |
| The vblank interrupt is the line interrupt (`IER_LIE`). The register-reload interrupt flag exists (`IER_RRIE`, `ISR_RRIF`) but this file only defines it; the IRQ thread does not test `ISR_RRIF`. The enable paths set `IER_LIE`, `IER_FUWIE`, `IER_FUEIE` and `IER_TERRIE`. | source | defines 188/199; thread 1011-1034; IER sets 1265, 1455 |
| Hard IRQ reads and clears `LTDC_ISR`, then wakes the thread. | source | 1040-1056 |
| The event is therefore tied to the line interrupt, not to the reload-done interrupt. Where the line interrupt sits (`LTDC_LIPCR`) is not set in anything I read. Whether it falls at the start of vertical blanking is not confirmed. | inferred | n/a |
| Latch timing, older-style IP (`plane_reg_shadow == false`): `atomic_flush` sets `SRCR_VBR`, commented "update planes at next vblank". | source | 1355-1357 |
| Latch timing, newer IP (`plane_reg_shadow == true`): the plane update writes the per-layer reload register `LTDC_L1RCR` with `LXRCR_VBR` (and clears IMR and GRMSK), commented "update plane at next vblank". | source | 2018-2021, disable 2085-2088 |
| Both paths use the vertical-blanking reload (VBR), not the immediate reload (IMR). IMR is only used on disable, "immediately commit disable of layers before switching off LTDC". So register updates latch at vblank. | source | IMR uses at 1154, 1302, 1159, 1306; VBR as above |
| `plane_reg_shadow` is false for HW versions 0x010200, 0x010300 and 0x020101, and true for 0x040100 and 0x040101. The version number the MP25 LTDC reports is not read in this file. That the MP25 takes the 0x04010x branch (per-layer reload) is a guess. | source (flag values: 2508, 2534, 2554, 2574); MP25 version inferred | 2488-2574 |

### 2. Vblank support

| Finding | Tag | Where |
|---|---|---|
| `drm_vblank_init(ddev, NB_CRTC)` is called at load. | source | 2785 |
| `enable_vblank` sets `vblank_active` and `IER_LIE`; it returns `-EPERM` if the CRTC state is not enabled. | source | 1446-1461 |
| `disable_vblank` clears `vblank_active`, and clears `IER_LIE` unless the CRC source is active. | source | 1463-1473 |
| `get_vblank_timestamp` is `drm_crtc_vblank_helper_get_vblank_timestamp`, which uses the driver's `get_scanout_position`. | source | 1543, 1555 |
| `get_scanout_position` is a crtc helper. When the device is runtime-active it reads `LTDC_CPSR` (current line), `LTDC_BPCR`, `LTDC_AWCR` and `LTDC_TWCR`, and returns `vpos` (negative in blanking). When not active, `vpos` is 0. `hpos` is always 0. It returns true. | source | 1385-1436 |
| `atomic_enable` calls `drm_crtc_vblank_on`; `atomic_disable` calls `drm_crtc_vblank_off` first. | source | 1271, 1291 |
| Quirk: on disable, an event still in `crtc->state->event` with an inactive CRTC is sent at once, so the flush can complete without a real vblank. | source | 1337-1343 |
| Quirk: `enable_vblank` fails with `-EPERM` while the CRTC is off, so `drm_crtc_vblank_get` fails there; `atomic_flush` then falls back to the immediate event. | source (1457, 1363-1366); consequence inferred | n/a |
| Vblank is counted by the DRM core, with `drm_crtc_handle_vblank` called from the threaded IRQ, so vblank time stamps include the thread wake latency. The core helper compensates by using scanout position. | inferred | n/a |

### 3. Fences

| Finding | Tag | Where |
|---|---|---|
| `ltdc_plane_helper_funcs` has `atomic_check`, `atomic_update`, `atomic_disable` and `atomic_enable`. There is no `prepare_fb` or `cleanup_fb` entry in this file, and no `drm_gem_plane_helper_prepare_fb` call. | source | 2144-2149; grep for `prepare_fb` finds nothing |
| The file includes `drm_gem_atomic_helper.h` and `drm_gem_dma_helper.h`. Whether the framebuffer is created with `drm_gem_fb_create` and the mode-config `atomic_commit` is the helper commit lives in `drv.c`, which I did not read. | source (includes: 35-36); rest not read | n/a |
| Because the plane has no `prepare_fb`, the implicit fence on a framebuffer's GEM object is not waited on by this file. Whether `drv.c` sets it some other way (for example `drm_plane_enable_fb_damage_clips`, or the helper being set at plane init) is not read. | inferred | n/a |
| `IN_FENCE_FD` and `OUT_FENCE_PTR` are handled by the DRM atomic core, not by the driver. This file has no fence code. Working in-fence wait depends on `drm_atomic_helper_wait_for_fences`, which the DRM helper commit calls, and on the plane's `fence` being set by the core. The driver's `atomic_update` does not touch the fence. | inferred | n/a |
| The out-fence is signalled by the DRM core on the CRTC's flip event, so it completes when the armed event is sent, that is at the line interrupt. | inferred | n/a |

### 4. Async or tearing commits

| Finding | Tag | Where |
|---|---|---|
| `atomic_async_check` and `atomic_async_update` are not present in the plane helper funcs. `grep -i async` on this file finds nothing. | source | 2144-2149 |
| No `DRM_MODE_PAGE_FLIP_ASYNC` support is visible in this file. Whether `drv.c` sets `mode_config.async_page_flip` is not read. | source (absence in `ltdc.c`); `drv.c` not read | n/a |

### 5. One commit, all three planes

| Finding | Tag | Where |
|---|---|---|
| On the older IP (`plane_reg_shadow == false`), the reload is one global `SRCR_VBR` set in `atomic_flush`, after all plane `atomic_update` calls, so the planes latch together at the next vblank. | source (flush 1355-1357); "all planes in one commit" inferred from the DRM helper order of plane updates before CRTC flush | n/a |
| On the newer IP (`plane_reg_shadow == true`), each plane writes its own reload register `LTDC_L1RCR + lofs`, with `LXRCR_GRMSK` cleared. Each layer is reloaded at vblank on its own, so atomicity depends on all layers' writes landing before the same vblank. This file does not show a single reload covering all layers. The meaning of `GRMSK` ("Global (centralized) Reload MaSKed", line 252) is by name only. | source (2018-2021, 252); atomicity inferred | n/a |
| Number of layers is read from hardware (`LCR`), clamped by `LTDC_MAX_LAYER`. The value of `LTDC_MAX_LAYER` is in `ltdc.h`, not read here. | source (2423); constant not read | n/a |

### 6. ST-only changes against mainline

Not read. No mainline file was fetched, so I cannot say which of the above is ST-only.
No claim is made.

## Open rows to finish

- Pin both trees to commit hashes, using a real checkout.
- Read ST `drv.c` and mainline `ltdc.c` / `drv.c` for rows 3, 4 and 6.
- Confirm which `HWVER` the MP25 LTDC reports, and that the plat data / compatible selects it (`ltdc.h`, dtsi; owned by the source-reread item).
- Confirm on hardware, later, any `inferred` row that matters for a backend.
