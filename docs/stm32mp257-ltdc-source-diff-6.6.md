# STM32MP257 LTDC: ST v6.18 source rows against ST's 6.6 kernel

Date of reading: 2026-11-28. **Nothing here is a board measurement and no board was touched.** Every table row is tagged **read from source**; every conclusion for the board is tagged **inferred**.

Question Q10 of [`design/stm32mp257-backend.md`](design/stm32mp257-backend.md) section 7: the "read from source" rows in that note, in [`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md), [`stm32mp257-ltdc-sync-source.md`](stm32mp257-ltdc-sync-source.md) and [`stm32mp257-ltdc-board.md`](stm32mp257-ltdc-board.md) were read in ST's v6.18 tree, while the board runs ST's 6.6.129. This file takes each cited fact and says whether the 6.6 code **matches**, **differs** or is **absent**.

## Refs and commits

Both trees are `STMicroelectronics/linux`, fetched on 2026-11-28 over ssh as shallow (depth 1), blobless fetches of just these refs, and read with `git show REF:PATH`. Line numbers below were counted on those files.

| Name | Ref | Commit | Kernel version (top `Makefile`) | Committed |
| --- | --- | --- | --- | --- |
| ST 6.18 | branch `v6.18-stm32mp`, also tag `v6.18-stm32mp-r1` | `3056405e0479330b0889c2a971452bfd10014b70` | 6.18.37 | 2026-10-23 |
| ST 6.6 | branch `v6.6-stm32mp`, also tag `v6.6-stm32mp-r3.1` | `548f960c059bc4b9165cb69895fd67551f6061ca` | 6.6.129 | 2026-06-01 |

Further ST 6.6 tags, fetched the same way to see which of them could be the board's kernel (read from source):

| Tag | Commit | `SUBLEVEL` | `ltdc.c` blob |
| --- | --- | --- | --- |
| `v6.6-stm32mp-r1` | `3682d604ecbdd233bb7e38b8e18f0496d78a8969` | 48 | `11fb670c600c010e688657395431ba5d5ffe29fa` |
| `v6.6-stm32mp-r1.1` | `f1d0da4f719b1f68c6cb6fca4efb59268a9a82af` | not read | `11fb670c600c010e688657395431ba5d5ffe29fa` |
| `v6.6-stm32mp-r1.2` | `57d6654e68728d6b6776e688a0f8ece6ecfc1209` | not read | `ff76e618b1d20e88687e72ad2e600bc5f6b80f4f` |
| `v6.6-stm32mp-r2` | `f01241fbba4d879fa770685629b49d42e904e43c` | not read | `b1eb57c142f798f2b5affed464cf12cd4e514d74` |
| `v6.6-stm32mp-r3` | `80d29586bafa4734bad35a8f063919fdd79f5dec` | 116 | `6ea30597d23f7a9adc4d5ffb5d99d0f56e36cd98` |
| `v6.6-stm32mp-r3.1` (= branch tip) | `548f960c059bc4b9165cb69895fd67551f6061ca` | 129 | `3845c19461b53ac48a8ffd8beb8339bfca38d62a` |

**What was not read.** The exact commit the board kernel was built from was **not read from the board** here (no board access in this item). The only 6.6 ref whose `Makefile` says sublevel 129 is the branch tip `548f960c`, and its version number agrees with the board's "6.6.129" reported in earlier documents. That the board kernel is built from `548f960c`, or has the same `drivers/gpu/drm/stm/` files, is **inferred**. A board session records `uname -a` in `docs/stm32mp257-ltdc-session-2026-11-28.md`; compare it against this file. The OpenSTLinux release may carry out-of-tree patches or a different device tree that were not read.

Files compared (6.6 against 6.18): `drivers/gpu/drm/stm/ltdc.c` (2943 and 2941 lines), `ltdc.h` (87 and 86), `drv.c` (317 and 303), `lvds.c` (1302 and 1308), the `st,stm32-ltdc.yaml` binding, and the DT files `stm32mp251.dtsi`, `stm32mp255.dtsi`, `stm32mp257f-dk.dts`, `stm32mp257f-dk-resmem.dtsi`. `Kconfig` and `Makefile` of the driver directory were not compared.

## What differs at all (summary of the whole-file diff)

A full `diff` of `ltdc.c` between the two refs is about 530 lines. Almost all of it is logging style (`DRM_ERROR`, `DRM_DEBUG_DRIVER` in 6.6 against `drm_err`, `drm_dbg_driver` in 6.18) and housekeeping. The functional differences in `ltdc.c` are exactly these (read from source):

1. **Compatible strings.** 6.6 tests `st,stm32mp21-ltdc` and `st,stm32mp25-ltdc`; 6.18 tests `st,stm32mp251-ltdc` and `st,stm32mp255-ltdc`. Where 6.18 separates MP251 from MP255, 6.6 has one MP25 test (rows 26, 27, 28, 29, 38).
2. **Dst-rectangle warning.** 6.18 `ltdc_plane_update` warns "Invalid dst rectangle" (`ltdc.c:1740-1742`); 6.6 has no such check. A warning only; no code path changes.
3. **CRTC enable.** 6.6 takes bus format and flags from `bridge->timings` or the connector (`ltdc.c:1106-1111`); 6.18 takes them from the bridge state (`ltdc.c:1105-1110`). 6.18 also writes the timing registers (SSCR, BPCR, AWCR, TWCR, LIPCR) after, not before, an earlier block (6.6 `ltdc.c:1193-1210`, 6.18 `ltdc.c:1213-1230`). Not a plane matter.
4. **Plane creation.** 6.18 reads `LTDC_L1C1R` unconditionally before the YUV block (`ltdc.c:2187`); 6.6 reads it inside `if (ldev->caps.ycbcr_input)` (`ltdc.c:2183`). No change for this board, where `ycbcr_input` is true.
5. **Probe.** 6.6 keeps the reset control in `ldev->rstc` (`ltdc.h:75`); 6.18 uses a local. 6.18 prints "hardware identifier not supported" after `ltdc_get_caps` fails (`ltdc.c:2735`) as well. `ltdc_plane_create` failure returns `-ENOMEM` in 6.6 and `-EINVAL` in 6.18 for the primary plane.

`ltdc.h` differs only in field order. `drv.c` differs in the compatible table, the aperture and fbdev setup, a `.shutdown` hook added in 6.18, and DMA mask handling; none touches planes.

## Rows: sync, vblank, fences, async (`stm32mp257-ltdc-sync-source.md`)

| # | Fact cited | 6.18 (`3056405e`) | 6.6 (`548f960c`) | Verdict | Tag |
| --- | --- | --- | --- | --- | --- |
| 1 | `atomic_flush` arms the event with `drm_crtc_arm_vblank_event`, falls back to `drm_crtc_send_vblank_event` | `ltdc.c:1346-1369` (arm 1364, send 1366) | `ltdc.c:1347-1370` (arm 1365, send 1367) | matches | read from source |
| 2 | IRQ thread completes the event on `ISR_LIF` via `drm_crtc_handle_vblank` | `ltdc.c:1004-1024` (call 1013) | `ltdc.c:1005-1025` (call 1014) | matches | read from source |
| 3 | Hard IRQ reads `LTDC_ISR`, clears with `LTDC_ICR`, wakes the thread | `ltdc.c:1040-1056` | `ltdc.c:1041-1057` | matches | read from source |
| 4 | `IER_RRIE` and `ISR_RRIF` defined, never tested by the IRQ thread; `IER_LIE`, `IER_FUWIE`, `IER_FUEIE`, `IER_TERRIE` are set | defines `188`, `199`; sets `1265`, `1455` | defines `191`, `202`; sets `1266`, `1456` | matches | read from source |
| 5 | Older IP latches with `SRCR_VBR` in flush | `ltdc.c:1355-1357` | `ltdc.c:1356-1358` | matches | read from source |
| 6 | Newer IP writes `LTDC_L1RCR` with `LXRCR_VBR` in plane update and plane disable | `ltdc.c:2018-2021`, `2085-2088` | `ltdc.c:2012-2015`, `2079-2082` | matches | read from source |
| 7 | IMR reload only on disable paths | `ltdc.c:1154`, `1159`, `1302`, `1306` | `ltdc.c:1155`, `1160`, `1303`, `1307` | matches | read from source |
| 8 | `plane_reg_shadow` false for 0x010200, 0x010300, 0x020101; true for 0x040100 and 0x040101 | `ltdc.c:2508`, `2534`, `2554`, `2574` | `ltdc.c:2499`, `2525`, `2545`, `2565` | matches | read from source |
| 9 | `LXRCR_GRMSK` defined as "Global (centralized) Reload MaSKed" | `ltdc.c:252` | `ltdc.c:255` | matches | read from source |
| 10 | `drm_vblank_init(ddev, NB_CRTC)` at load | `ltdc.c:2785` | `ltdc.c:2780` | matches | read from source |
| 11 | `enable_vblank` sets `vblank_active` and `IER_LIE`, `-EPERM` if CRTC off; `disable_vblank` clears them | `ltdc.c:1446-1473` | `ltdc.c:1447-1474` | matches | read from source |
| 12 | `get_scanout_position` reads `LTDC_CPSR`, `BPCR`, `AWCR`, `TWCR` when runtime-active | `ltdc.c:1385-1436` | `ltdc.c:1386-1437` | matches | read from source |
| 13 | `drm_crtc_vblank_on` in `atomic_enable`, `drm_crtc_vblank_off` in `atomic_disable` | `ltdc.c:1271`, `1291` | `ltdc.c:1272`, `1292` | matches | read from source |
| 14 | On disable an event left in `crtc->state->event` is sent at once | `ltdc.c:1337-1343` | `ltdc.c:1338-1344` (send 1341) | matches | read from source |
| 15 | Plane helper funcs have `atomic_check`, `atomic_update`, `atomic_disable`, `atomic_enable`; no `prepare_fb`, no `cleanup_fb` | `ltdc.c:2144-2149` | `ltdc.c:2138-2143`; `prepare_fb` and `async` match nothing in the file | matches | read from source |
| 16 | No `atomic_async_check` or `atomic_async_update`; `drv.c` sets no `async_page_flip` | `ltdc.c` (no match); `drv.c:94-98` | `ltdc.c` (no match); `drv.c:94-98` | matches | read from source |
| 17 | `drv.c`: `.fb_create = drm_gem_fb_create`, `.atomic_commit = drm_atomic_helper_commit`, tail `drm_atomic_helper_commit_tail_rpm` | `drv.c:38-46` | `drv.c:36-44` (fb_create 37, commit 39, tail 43) | matches | read from source |
| 18 | `drm_mode_config_helper_suspend` in `drv_suspend` | `drv.c:128` | `drv.c:129` | matches | read from source |
| 19 | Layer count read from `LTDC_LCR`, clamped to `LTDC_MAX_LAYER` (4) | `ltdc.c:2423`, `ltdc.h:14` | `ltdc.c:2415`, `ltdc.h:14` | matches | read from source |
| 20 | LTDC_IDR read at probe and switched on; unknown value gives "hardware identifier not supported" and `-ENODEV`; debug print "ltdc hw version" | `ltdc.c:2484`, `2487-2608`, `2601-2607`, `2743` | `ltdc.c:2474`, `2478-2596`, `2593-2596`, `2738` | matches | read from source |
| 21 | HWVER constants 0x010200, 0x010300, 0x020101, 0x040100, 0x040101 | `ltdc.c:52-56` | `ltdc.c:55-59` | matches | read from source |
| 22 | The 4.x case sets `plane_reg_shadow`, `dynamic_zorder`, `plane_rotation`, `fifo_threshold` true and `crtc_rotation` from `GC2R_ROTA` | `ltdc.c:2561-2600` | `ltdc.c:2552-2592` | matches | read from source |

## Rows: planes, scaling, zpos, alpha, formats (`stm32mp257-ltdc-source-full.md`, `stm32mp257-ltdc-board.md`, `design/stm32mp257-backend.md` sections 1-4)

| # | Fact cited | 6.18 (`3056405e`) | 6.6 (`548f960c`) | Verdict | Tag |
| --- | --- | --- | --- | --- | --- |
| 23 | `NB_CRTC 1`, `CRTC_MASK` gives `possible_crtcs` 0x1 | `ltdc.c:47-48` | `ltdc.c:50-51` | matches | read from source |
| 24 | `LTDC_MAX_LAYER` 4, `LTDC_MAX_FIREWALL` 4; `plane_scaling[LTDC_MAX_LAYER]` | `ltdc.h:14-15`, `41` | `ltdc.h:14-15`, `41` | matches | read from source |
| 25 | Firewalls `l1l2`, `l3`, `rot`; a denied `l3` decrements `nb_layers` and marks the layer secured | `ltdc.c:2425-2470` (decrement 2467) | `ltdc.c:2417-2468` (decrement 2459, `strcmp("l3", ...)` 2458) | matches (gated by MP21 or MP25 compatible, see row 26) | read from source |
| 26 | MP25 detection for firewall, burst length, secured-layer CRC | `st,stm32mp251-ltdc` or `st,stm32mp255-ltdc` (`ltdc.c:2425-2426`, `2595-2596`, `2656-2657`) | `st,stm32mp21-ltdc` or `st,stm32mp25-ltdc` (`ltdc.c:2417-2418`, `2586-2587`, `2653-2654`) | **differs** in string only; same effect on an MP25 | read from source |
| 27 | Scaler registers (`LTDC_L1SISR`, `L1SOSR`, `L1SVSFR`, `L1SVSPR`, ...) accessible in the regmap access tables | MP251 or MP255 (`ltdc.c:626-643`, `747-764`) | `st,stm32mp25-ltdc` (`ltdc.c:619-642`, `737-760`) | **differs** in string only; same effect on an MP25 | read from source |
| 28 | Rotator registers (`LTDC_RB0AR`, `RB1AR`, `RBPR`, `RIFCR`) accessible only for MP255 | MP255 only (`ltdc.c:616-624`, `737-745`) | `st,stm32mp25-ltdc`, no MP251/MP255 split (`ltdc.c:619-626`, `737-744`) | **differs**: in 6.6 the rotator registers are accessible on every MP25 | read from source |
| 29 | Pad frequency limit 150 MHz for MP25, 90 MHz generic | `drv.c:269-280`; `ltdc.h:49-51` | `drv.c:280-295` (generic 90 MHz, `mp21` 150 MHz, `mp25` 150 MHz); `ltdc.h:49-51` | matches for MP25; 6.6 lists an extra `mp21` entry and has a single MP25 compatible | read from source |
| 30 | `caps.pad_max_freq_hz` from plat data; 65 and 150 MHz overrides belong to the 1.2.0 and 2.1.1 cases, not 4.x | `ltdc.c:2485`, `2504`, `2550` | `ltdc.c:2476`, `2495`, `2541` | matches | read from source |
| 31 | `ltdc_encoder_mode_valid`: `MODE_CLOCK_HIGH` above the pad limit; `clock_tolerance` 5 per mille, preferred mode skips the tolerance | `ltdc.c:2328-2373` (limit 2351-2352) | `ltdc.c:2320-2368` (limit 2343-2344) | matches (the LVDS-clock branch is chosen by `st,stm32mp255-ltdc` in 6.18, `st,stm32mp25-ltdc` in 6.6) | read from source |
| 32 | `DRM_MODE_ENCODER_DPI` encoder always created; mode check tests for `DRM_MODE_ENCODER_LVDS` | `ltdc.c:2388`, `2340` | `ltdc.c:2380`, `2332` | matches | read from source |
| 33 | Per-layer scaling ability: `plane_scaling[i]` set from `LXCR_C1R_SCA` (bit 31) of `LTDC_L1C1R` | `ltdc.c:238`, `2585-2592` | `ltdc.c:241`, `2576-2584` | matches | read from source |
| 34 | Scaling rules in `ltdc_plane_atomic_check`: YUV with any size change refused ("Scaling is not supported with yuv pixel format"); source larger than destination refused ("Downscaling is not supported"); on a layer without the ability any mismatch refused ("Scaling is not supported on layer %d") | `ltdc.c:1565`, `1585-1602` | `ltdc.c:1566`, `1585-1602` (YUV 1586-1591, downscale 1593-1596, no-scaler 1597-1601); same condition text | matches | read from source |
| 35 | These checks are in `atomic_check`, so TEST_ONLY sees them | `ltdc.c:1565-1605` | `ltdc.c:1566-1605`; no rotation or size check in the function | matches | read from source |
| 36 | Scale factor `((in-1)<<12)/(out-1)` masked 0xFFFF, 0x1000 for `out == 1`; no upper cap in code | `ltdc.c:1611-1618`; used `1999`, `2006` | `ltdc.c:1609-1616` (`SCALER_FRACTION` 1607); used `1993`, `2000` | matches | read from source |
| 37 | Phase registers look cross-assigned: `LTDC_L1SHSPR` gets the vertical factor, `LTDC_L1SVSPR` gets horizontal factor plus `1<<12` | `ltdc.c:2003`, `2010` | `ltdc.c:1997`, `2004` (identical statements; the comments say "vertical scaling phase" twice) | matches | read from source |
| 38 | `LXCR_SCEN` set only when the layer can scale and the sizes differ; scaler size registers written | `ltdc.c:1985-1988` | `ltdc.c:1979-1982` | matches | read from source |
| 39 | zpos: `drm_plane_create_zpos_property(.., 0, nb_layers-1)` when `dynamic_zorder`, initial value the layer index; immutable otherwise | `ltdc.c:2267` (primary), `2314` (overlays) | `ltdc.c:2259`, `2306` | matches | read from source |
| 40 | `normalized_zpos << 16` written into `LTDC_L1BFCR` (field `LXBFCR_BOR`) with `dynamic_zorder`; `normalize_zpos = true` in `drv.c` | `ltdc.c:1842`; `drv.c:98` | `ltdc.c:1836-1837`; `drv.c:98` | matches | read from source |
| 41 | Alpha: `drm_plane_create_alpha_property` on every plane; no `blend_mode` property anywhere | `ltdc.c:2236` | `ltdc.c:2228`; `blend_mode` matches nothing | matches | read from source |
| 42 | Blend factors: `BF1_PAXCA` (0x600) with `BF2_1PAXCA` (0x007) for alpha formats; `BF1_CA` (0x400) with `BF2_1CA` (0x005) for formats without alpha; constant alpha is `alpha >> 8` | `ltdc.c:257-260`, `1825-1839` | `ltdc.c:260-263`, `1819`, `1826-1833` | matches | read from source |
| 43 | Rotation property: every layer `ROTATE_0 | REFLECT_X | REFLECT_Y` when `plane_rotation`; primary adds 90/180/270 only when `crtc_rotation` | `ltdc.c:2272-2285`, `2319-2322` | `ltdc.c:2263-2276`, `2310-2314` | matches | read from source |
| 44 | `crtc_rotation` cleared when the device tree has no `rotation-memory` | `ltdc.c:2825-2831` | `ltdc.c:2820-2826` | matches | read from source |
| 45 | Format lists: `ltdc_pix_fmt_a2`, `ltdc_drm_fmt_a2` (17 RGB and C8 formats), YUV lists `ltdc_drm_fmt_ycbcr_cp` (4), `_sp` (NV12, NV21), `_fp` (YU12, YV12) | `ltdc.c:315`, `352`, `372-384` | `ltdc.c:318`, `355`, `375-387`; the lines 300-520 of both files have no diff hunk | matches (identical text) | read from source |
| 46 | Interleaved YUV offered when the layer has `LXCR_C1R_YIA`; semi-planar and planar only for an MP25 layer with nonzero index (and `YSPA`/`YFPA`) | `ltdc.c:2196-2206` (MP251 or MP255 and `index`) | `ltdc.c:2194-2206` (`st,stm32mp25-ltdc` and `index`, or generic `st,stm32-ltdc`) | matches in effect on an MP25 | read from source |
| 47 | `non_alpha_only_l1` false in the 4.x case, so X-formats stay on all layers | `ltdc.c:2570` | `ltdc.c:2561` | matches | read from source |
| 48 | YUV colour properties BT.601 and BT.709, limited and full range; BT.2020 not supported, falls back to BT.601 limited | `ltdc.c:802`, plus `drm_plane_create_color_properties` | `ltdc.c:799`, `2217-2223` | matches | read from source |
| 49 | Modifiers: `DRM_FORMAT_MOD_LINEAR` only | `ltdc.c:514` | `ltdc.c:517` | matches | read from source |
| 50 | Max framebuffer 2048 by 2048 as `mode_config.max_width/height` | `drv.c:35-36`, `94-98` | `drv.c:33-34`, `94-95` | matches | read from source |
| 51 | Burst length: `st,burstlen` read for MP25 (`mbl/8`, 0 if absent), written to `LTDC_L1BLCR` per layer | `ltdc.c:2656-2664`, `1970-1972` | `ltdc.c:2653-2662`, `1964-1966` | matches (compatible string differs, row 26) | read from source |
| 52 | No bandwidth check in `atomic_check` or `ltdc_crtc_atomic_check` | `ltdc.c:1371` | `ltdc.c:1372` (no new check) | matches | read from source |
| 53 | CRTC gamma size 256 (`CLUT_SIZE`), CLUT for C8 | `ltdc.c` | `ltdc.c:2293-2294` | matches | read from source |

## Rows: rotation, the 90/270 refusal (`stm32mp257-ltdc-board.md`, `design/stm32mp257-backend.md` sections 2, 5, 7 item 6)

| # | Fact cited | 6.18 (`3056405e`) | 6.6 (`548f960c`) | Verdict | Tag |
| --- | --- | --- | --- | --- | --- |
| 54 | 90/270 handling in `ltdc_plane_update`, which is `atomic_update`, past TEST_ONLY | `ltdc.c:1744-1765` | `ltdc.c:1738-1760` | matches | read from source |
| 55 | Display streaming is stopped (`GCR_LTDCEN` cleared, then a delay of two frames) before the rotation checks | `ltdc.c:1744-1749` | `ltdc.c:1738-1743` (clear 1740, `mdelay` 1743) | matches | read from source |
| 56 | "Rotation buffer too small" refusal: `rot_mem->size < hdisplay*vdisplay*2*3` returns with LTDC left disabled | `ltdc.c:1759-1761` | `ltdc.c:1753-1756` | matches | read from source |
| 57 | `vdisplay > 1366` refusal, returns with LTDC left disabled | `ltdc.c:1765` | `ltdc.c:1759-1760` | matches | read from source |
| 58 | Another layer refused while the primary rotates ("Rotation enabled on primary plane, do not use another layer") | `ltdc.c:1671-1684` (warn 1681) | `ltdc.c:1669-1682` (warn 1679) | matches | read from source |
| 59 | 180 is `REFLECT_X | REFLECT_Y`, 90 and 270 add the rotator (`ROTATE_90` with a reflection); reflections toggle | `ltdc.c:1690-1723` | `ltdc.c:1684-1717` | matches | read from source |
| 60 | Rotator buffers: addresses cast to `u32`, `LTDC_RB0AR`, `LTDC_RB1AR`, `LTDC_RBPR` written | `ltdc.c:1768-1781` | `ltdc.c:1762-1775` | matches | read from source |
| 61 | `IER_FURIE` set only on the rotation path | `ltdc.c:1788` | `ltdc.c:1782` | matches | read from source |
| 62 | `rotation-memory` set in no DT file read | 6.18 `stm32mp251.dtsi`, `stm32mp255.dtsi`, `stm32mp257.dtsi`, `stm32mp25xf.dtsi`, `stm32mp257f-dk.dts`, `stm32mp257f-dk-resmem.dtsi` (no match) | 6.6 `stm32mp251.dtsi`, `stm32mp255.dtsi`, `stm32mp257f-dk.dts`, `stm32mp257f-dk-resmem.dtsi` (no match; `stm32mp257.dtsi` and `stm32mp25xf.dtsi` not searched) | matches | read from source |
| 63 | Reserved regions `ltdc-sec-layer` (8 MiB) and `ltdc-sec-rotation` (16 MiB) at fixed addresses, `no-map`, referenced by nothing | `stm32mp257f-dk-resmem.dtsi:174-182` | `stm32mp257f-dk-resmem.dtsi:175-184` (`fe800000` size 0x800000; `ff000000` size 0x1000000) | matches | read from source |
| 64 | "Invalid dst rectangle" warning for a destination outside the mode | `ltdc.c:1740-1742` | absent | **absent** in 6.6 (a warning only) | read from source |

## Rows: underrun counters and IRQ handling (`design/stm32mp257-backend.md` sections 3 and 7 item 7)

| # | Fact cited | 6.18 (`3056405e`) | 6.6 (`548f960c`) | Verdict | Tag |
| --- | --- | --- | --- | --- | --- |
| 65 | IRQ thread increments `transfer_err`, `fifo_err`, `fifo_warn`, `fifo_rot` from the status flags | `ltdc.c:1028-1034` | `ltdc.c:1029-1035` | matches | read from source |
| 66 | Error IRQs enabled on CRTC enable: `IER_FUWIE | IER_FUEIE | IER_TERRIE`; cleared with `IER_FURIE` on disable | `ltdc.c:1265`, `1298` | `ltdc.c:1266`, `1299` | matches | read from source |
| 67 | Counters reset on CRTC disable and at probe; `fifo_threshold` starts at `FUT_DFT` (128), written to `LTDC_FUT` on enable | `ltdc.c:264`, `1331-1334`, `2745-2750` | `ltdc.c:267`, `1332-1335`, `2740-2745` | matches | read from source |
| 68 | Log text "ltdc fifo underrun: please verify display mode", printed from plane update when `fifo_err` is nonzero or `fifo_warn >= fifo_threshold`; "ltdc transfer error" and "ltdc fifo rotation error" likewise | `ltdc.c:2026-2044` | `ltdc.c:2020-2038` (`DRM_WARN` instead of `drm_warn`) | matches | read from source |
| 69 | Counters printed by the CRTC state dump as `transfer_error`, `fifo_underrun_error`, `fifo_underrun_warning`, `fifo_underrun_rotation` | `ltdc.c:1528-1531` | `ltdc.c:1529-1532` | matches | read from source |
| 70 | The message is printed only when a later plane update runs; no message comes from the IRQ thread itself | `ltdc.c:2026-2044` | `ltdc.c:2020-2038` | matches | read from source |

## Rows: device tree, binding, outputs

| # | Fact cited | 6.18 (`3056405e`) | 6.6 (`548f960c`) | Verdict | Tag |
| --- | --- | --- | --- | --- | --- |
| 71 | LTDC node `display-controller@48010000`, GIC SPI 158 and 159, clocks `bus` and `lcd`, reset `LTDC_R`, firewalls `l1l2` (119), `l3` (120), `rot` (121) | `stm32mp251.dtsi:2148-2170`, compatible `st,stm32mp251-ltdc` | `stm32mp251.dtsi:2150-2176`, compatible `st,stm32mp25-ltdc` | matches except the compatible string | read from source |
| 72 | MP255 file extends `&ltdc` with the `ref` (syscfg) and `lvds` clocks; `dsi@48000000`, `lvds@48060000` | `stm32mp255.dtsi:30-33`, `124`, `138`; also sets `compatible = "st,stm32mp255-ltdc"` | `stm32mp255.dtsi:30-33`, `124`, `139`; `&ltdc` sets only clocks and clock names, no compatible override; DSI `st,stm32mp25-dsi`, LVDS `st,stm32mp25-lvds` | **differs**: 6.6 keeps the one `st,stm32mp25-ltdc` compatible for MP251 and MP255 | read from source |
| 73 | Board DT: `&ltdc` has two endpoints, `ltdc_ep0_out` to `dsi_in` (DSI to the HDMI bridge) and `ltdc_ep1_out` to `lvds_in` (LVDS panel) | `stm32mp257f-dk.dts:645-663`, `364-383`, `537-575` | `stm32mp257f-dk.dts:536-551`, `339-340`, `566-567` | matches | read from source |
| 74 | Binding: compatible enum, `rotation-memory` property, one port with up to 2 endpoints that cannot be active together, `additionalProperties: false`, no `st,burstlen` | `st,stm32-ltdc.yaml:14-18`, `95-99`, `106-109`, `163` | `st,stm32-ltdc.yaml:17-18` (`st,stm32mp21-ltdc`, `st,stm32mp25-ltdc`), `43-46` (2 endpoints, cannot be active together), `64`; **no `rotation-memory`, no children, no `st,burstlen`** | **differs**: the 6.6 binding does not describe `rotation-memory` or the layer-firewall children, which the 6.6 driver and DT use | read from source |
| 75 | `lvds.c` matches `st,stm32mp25-lvds`; VESA and JEIDA RGB888 bitmaps | `lvds.c:1279`; `825-833` | `lvds.c:1272`; `807-815` | matches | read from source |
| 76 | `lvds.c` handling of a 6-bit panel (`MEDIA_BUS_FMT_RGB666_1X7X3_SPWG`) | selects `lvds_bitmap_jeida_rgb666` (`lvds.c:825-827`) | warns "Pixel format with data mapping not yet supported" and returns (`lvds.c:807-809`) | **differs** (LVDS only; the board's HDMI path does not use it) | read from source |
| 77 | LVDS-clock lookup: `lvds` clock taken only when an `st,stm32mp25-lvds` node exists | `ltdc.c:2904-2910` | `ltdc.c:2903-2909` | matches | read from source |

## Consequences for the board (all inferred)

All of these assume that the board kernel's `drivers/gpu/drm/stm/` and device tree equal ST's `v6.6-stm32mp` tip `548f960c` (see the note on what was not read). They tag the reading, not the board.

1. **inferred.** The plane-side behaviour read in 6.18 (zpos writing, plane alpha, scaling rules and their TEST_ONLY placement, format lists, rotation property set, the post-TEST_ONLY 90/270 refusals, underrun counters and IRQ handling) is **the same code** on 6.6. Rows 1-70 can be taken as describing the board kernel with the same confidence as the board's 6.6 tree is the one read here; no row of the cited facts needs a different statement for 6.6.
2. **inferred.** `design/stm32mp257-backend.md` section 7 item 10 ("diff before treating source rows as describing the board") is answered for the plane path: the only functional differences are the compatible strings (rows 26-28, 72), the missing "Invalid dst rectangle" warning (row 64), CRTC-enable bus-format handling and LVDS RGB666 (row 76). None changes what a TEST_ONLY or a commit does for planes on an HDMI-through-DSI board.
3. **inferred.** Rotation: the 6.6 DT sets no `rotation-memory` (row 62), so `crtc_rotation` is cleared (row 44) and the plane rotation property of the primary plane lacks 90/270 on 6.6 just as on 6.18. The 90/270 refusals that leave the display stopped (rows 54-57) are therefore not reachable through the property on this tree, but a board session (Q6) is what decides what the property shows. 180 and the reflections go through the flip bits (`LXCR_HMEN`, and the vertical flip), not the rotator.
4. **inferred.** Because 6.6 has a single MP25 compatible (rows 26-28), any MP25 chip takes the MP255 rotator path in the register tables there. The MP251 against MP255 split that `stm32mp257-ltdc-source-full.md` describes for 6.18 does not exist on 6.6; the MP257 silicon has the MP255 display features, so it behaves the same on both.
5. **inferred.** Q1/Q2 (premultiplied or straight alpha): the blend factors are identical (row 42). The layer colour is weighted by `BF1_PAXCA`, pixel alpha times constant alpha, and the layer below by one minus that product. A factor applied to the layer's own colour means the hardware treats the buffer as **straight** (non-premultiplied) alpha: a premultiplied buffer would be darkened twice. This contradicts the `blend [premultiplied]` that composition-drm fills in when the property is absent. The inference rests on how the controller applies BF1, which is a hardware statement the code does not make; the board picture decides it.
6. **inferred.** Scaling (Q3, rows 33-38): the source reading cannot tell which of the three layers has `LXCR_C1R_SCA` (a register value, row 33); 6.6 does not change that. The cross-assigned phase registers (row 37) are in both trees, so a scaled overlay on the board uses them as read.
7. **inferred.** The 6.6 binding lacks `rotation-memory` and the firewall children (row 74); DT validation against that binding would flag them, but this does not change what the driver does.

## Not done here

No board access, no `uname -a`, no edit to any existing document, no code change. Existing documents still cite 6.18 line numbers; this file is the only place where the 6.6 line numbers are written.
