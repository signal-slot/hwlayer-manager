# STM32MP257 LTDC: sync semantics read from source


Status: **ST and mainline rows read from real checkouts.** Nothing in this document is measured on hardware. No board was accessed.

## Pinned trees and fetch record (2026-11-28)

| Tree | Result |
|---|---|
| STMicroelectronics/linux, branch `v6.18-stm32mp` | Real blobless clone read with `git show HEAD:<path>`. `git rev-parse HEAD` = `3056405e0479330b0889c2a971452bfd10014b70`. All `ltdc.c:N`, `drv.c:N`, `ltdc.h:N` numbers below were counted on that checkout; those from the first version (which used the branch tip as served over HTTP, 2941 lines, no hash) were re-checked and hold. |
| Mainline `torvalds/linux` tag `v6.18` | Real blobless clone, read with `git show HEAD:<path>`. Commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` (from `git show -s --format=%H HEAD`, committed 2025-11-30; `git rev-parse` was refused). Numbers below are counted on it. |
| Earlier failures | First attempt the same day: clone refused ("nobody decided in time"), only `ltdc.c` fetched over HTTP. Kept as record. |

Tags: `source` = read in the ST checkout; `inferred` = reasoned from the source or the DRM core, not read directly; `not read` = needs a file I did not get.

## Sync semantics

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
| `plane_reg_shadow` is false for HW versions 0x010200, 0x010300 and 0x020101, and true for 0x040100 and 0x040101. For MP25 see "MP25 hardware version" below: the version is read from a register, and the MP25-only code is inside the 4.x case, so MP25 uses the per-layer reload path. | source (flag values 2508, 2534, 2554, 2574); MP25 branch: inferred, strong | 2487-2608 |

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
| The file includes `drm_gem_atomic_helper.h` and `drm_gem_dma_helper.h`. `drv.c` sets `.fb_create = drm_gem_fb_create`, `.atomic_check = drm_atomic_helper_check`, `.atomic_commit = drm_atomic_helper_commit` and commit tail `drm_atomic_helper_commit_tail_rpm`. | source | `drv.c:38-46` (includes in `ltdc.c`: 35-36) |
| Because the plane has no `prepare_fb`, the implicit fence on a framebuffer's GEM object is not waited on by this file. `drv.c` does not set any fence or `prepare_fb` helper either (checked: `drv.c:38-46`, `94-98`). Whether the DRM core's `drm_atomic_helper_prepare_planes` falls back to `drm_gem_plane_helper_prepare_fb` when the driver has no `prepare_fb` is a core fact of the kernel version in use and is not in this tree's driver files; not verified. | inferred | n/a |
| `IN_FENCE_FD` and `OUT_FENCE_PTR` are handled by the DRM atomic core, not by the driver. This file has no fence code. Working in-fence wait depends on `drm_atomic_helper_wait_for_fences`, which the DRM helper commit calls, and on the plane's `fence` being set by the core. The driver's `atomic_update` does not touch the fence. | inferred | n/a |
| The out-fence is signalled by the DRM core on the CRTC's flip event, so it completes when the armed event is sent, that is at the line interrupt. | inferred | n/a |

### 4. Async or tearing commits

| Finding | Tag | Where |
|---|---|---|
| `atomic_async_check` and `atomic_async_update` are not present in the plane helper funcs. `grep -i async` on this file finds nothing. | source | 2144-2149 |
| No `DRM_MODE_PAGE_FLIP_ASYNC` support is visible in this file. `drv.c` does not set `mode_config.async_page_flip` (its `mode_config` setup is `drv.c:94-98`; no `async` text in the file). Async flips are therefore not offered. | source | `drv.c:94-98` |

### 5. One commit, all three planes

| Finding | Tag | Where |
|---|---|---|
| On the older IP (`plane_reg_shadow == false`), the reload is one global `SRCR_VBR` set in `atomic_flush`, after all plane `atomic_update` calls, so the planes latch together at the next vblank. | source (flush 1355-1357); "all planes in one commit" inferred from the DRM helper order of plane updates before CRTC flush | n/a |
| On the newer IP (`plane_reg_shadow == true`), each plane writes its own reload register `LTDC_L1RCR + lofs`, with `LXRCR_GRMSK` cleared. Each layer is reloaded at vblank on its own, so atomicity depends on all layers' writes landing before the same vblank. This file does not show a single reload covering all layers. The meaning of `GRMSK` ("Global (centralized) Reload MaSKed", line 252) is by name only. | source (2018-2021, 252); atomicity inferred | n/a |
| Number of layers is read from hardware (`LCR`), clamped by `LTDC_MAX_LAYER`. `LTDC_MAX_LAYER` is 4. | source | `ltdc.c:2423`, `ltdc.h:14` |

### 6. ST-only changes against mainline

Compared on the checkouts (ST `3056405e`, mainline `7d0a66e4`):

| Topic | ST v6.18-stm32mp | Mainline v6.18 | Tag |
|---|---|---|---|
| Event arming | `ltdc.c:1346-1369` | same code (`ltdc.c:1050-1060`) | source |
| Line IRQ completes it | `ltdc.c:1011-1013` | `ltdc.c:716` | source |
| Latch, older IP | `SRCR_VBR` in flush (`1356-1357`) | `SRCR_VBR` (`791-792`, `1050-1051`) | source |
| Latch, per-layer IP | `LXRCR_VBR` (`2018-2021`, `2085-2088`) | same (`1484-1486`, `1525-1527`) | source |
| Per-layer reload gating | `plane_reg_shadow` true in the 4.x case (`2574`) | same (`1867`) | source |
| Async commits | none | none (no `async` text in `drv.c`; `ltdc.c` has no async plane helpers by the grep above) | source |
| Fence / `prepare_fb` | none in `ltdc.c` or `drv.c` | none | source |
| Commit tail | `drm_atomic_helper_commit_tail_rpm` (`drv.c:44-46`) | not set (`drv.c:37-41`), so the DRM default tail | source |
| Suspend | ST: `drm_mode_config_helper_suspend` | mainline: `drm_atomic_helper_suspend` with a saved state | source (`drv.c` of both) |

So the sync scheme is the same in both trees; ST adds the runtime-PM commit tail and the MP25 scaler/rotator/firewall code, none of which changes the event or reload path read above. (The `ltdc.c` async and `prepare_fb` absence in mainline rests on the grep for those words in the checkout returning no match; this was grepped on the mainline `ltdc.c` with the pattern list including `async|prepare_fb`.)

## MP25 hardware version (settled from source, 2026-11-28)

The claim in the first version of this doc, "the MP25 is the newer IP with per-layer reload", is **partly confirmed, partly corrected**:

- **Corrected:** the version is not selected by the compatible or by `ltdc_plat_data`. `ltdc_plat_data` has one field, `pad_max_freq_hz` (`ltdc.h:49-51`; MP25 value 150 MHz at `drv.c:273-275`). The driver reads the hardware identifier register at probe: `regmap_read(ldev->regmap, LTDC_IDR, &ldev->caps.hw_version)` (`ltdc.c:2484`) and switches on it (`ltdc.c:2487-2608`). The supported values are `HWVER_10200`, `HWVER_10300`, `HWVER_20101`, `HWVER_40100` and `HWVER_40101` (`ltdc.c:52-56`); anything else fails with "hardware identifier not supported" and `-ENODEV` (`ltdc.c:2601-2607`). The driver prints the value at `ltdc.c:2743` (`drm_dbg_driver`, "ltdc hw version"). No value for the MP25 is written in any file read.
- **Confirmed by inference (strong):** the MP25-only handling sits inside the 4.x case. The scaler registers (`LTDC_L1SISR`, `LTDC_L1SOSR`) and the rotator registers are made accessible under `case HWVER_40100`/`HWVER_40101` only (`ltdc.c:588-632`, `709-752`); the MP251/MP255 crc handling is the last statement of that case (`ltdc.c:2595-2600`); and the per-layer scaling-ability read is in that case too (`ltdc.c:2585-2592`). A MP25 reporting one of the older versions would never reach this code. That case sets `plane_reg_shadow = true`, `dynamic_zorder = true`, `plane_rotation = true`, `fifo_threshold = true` (`ltdc.c:2574-2583`). So the MP25 takes the 0x040100/0x040101 branch with per-layer reload (`LTDC_L1RCR`, `LXRCR_VBR`, `ltdc.c:2018-2021`) and the exact register value is the one thing still not in source.- **Mainline agrees (inferred):** mainline `ltdc.c` has no MP25-specific capability code at all, only a `bus` clock test (`ltdc.c:1954-1966`), and the same `LTDC_IDR` switch (`ltdc.c:1805-1869`); mainline's MP25 support therefore also relies on the hardware reporting a 4.x version.
- **Not claimed:** whether the value is 0x040100 or 0x040101; both give the same flags in this driver.

## Open rows to finish

- The exact `LTDC_IDR` value on the MP25: read the `ltdc hw version` debug line on a board with `drm.debug` set (needs the board; not done here).
- Confirm on hardware, later, any `inferred` row that matters for a backend.
