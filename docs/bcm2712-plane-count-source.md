# BCM2712 CRTC and plane arithmetic from the vc4 source (HVS Q5)

Answers the source half of question 5 of [`hvs-backend-design.md`](hvs-backend-design.md) §7:
which CRTCs and how many planes the vc4 driver creates on BCM2712, and whether the extra
per-CRTC planes are cursors. The measured counts are in
[`observed/bcm2712-topology.md`](observed/bcm2712-topology.md) (observed 2026-11-28) and
[`observed/bcm2712.md`](observed/bcm2712.md). No board was used for this document.

Source: raspberrypi/linux, branch `rpi-6.18.y`, commit
`16ae2649dcfadf9d81445b1177a3191efa3cb82e`
(https://github.com/raspberrypi/linux/tree/16ae2649dcfadf9d81445b1177a3191efa3cb82e).
Files are under `drivers/gpu/drm/vc4/` unless stated.

Tags: **read from source** (branch, commit, file, function) · **inferred** · **observed** (date as
the observed file gives it, 2026-11-28).

**Caveat on line numbers.** The source was read through a web fetch that returns raw text without
line numbers, and the sandbox refused a local clone (`$CONNECTED_DEPS_DIR/rpi-linux-6.18` is
empty). Every "read from source" row therefore names the file and function at the commit above
instead of a `file:line`. A reader with a checkout can find each by `grep -n` on the function
name. Not read: `vc4_kms.c`, `vc4_regs.h`, `vc4_hdmi.c`.

## 1. What the driver creates — read from source

| # | Statement | Where (rpi-6.18.y 16ae2649) | Tag |
|---|---|---|---|
| S1 | `vc4_plane_init(dev, type, possible_crtcs)` passes `type` and `possible_crtcs` straight to `drmm_universal_plane_alloc`; it adds `rotation` = ROTATE_0 with ROTATE_0/180/REFLECT_X/REFLECT_Y, alpha, blend, colour encoding/range, scaling filter, chroma siting. Only a Primary gets an immutable zpos 0 here. No HVS5-vs-HVS6 branch except the format table and the helper funcs | `vc4_plane.c`, `vc4_plane_init` | read from source |
| S2 | `vc4_crtc_init` creates one **Primary** per CRTC (`vc4_plane_init(drm, DRM_PLANE_TYPE_PRIMARY, 0)`); its possible_crtcs is fixed up by `drmm_crtc_init_with_planes`. It is used for the pixelvalve CRTCs and for the TXP (writeback) CRTC | `vc4_crtc.c`, `vc4_crtc_init`, `__vc4_crtc_init` | read from source |
| S3 | `vc4_plane_create_additional_planes` runs from `vc4_drm_bind` after `component_bind_all`, only when `!vc4->firmware_kms` | `vc4_drv.c`, `vc4_drm_bind` | read from source |
| S4 | `VC4_NUM_OVERLAY_PLANES` = 16 and `VC4_NUM_TXP_OVERLAY_PLANES` = 32 | `vc4_plane.c`, `vc4_plane_create_additional_planes` | read from source |
| S5 | The function finds the **first** CRTC with `feeds_txp` and `break`s. `non_txp_crtc_mask = GENMASK(num_crtc - 1, 0) - drm_crtc_mask(txp_crtc)` | same | read from source |
| S6 | 16 **Overlay** planes are created with `possible_crtcs = non_txp_crtc_mask`; zpos property initial `i + 1`, range 1..17 | same | read from source |
| S7 | 32 **Overlay** planes are created with `possible_crtcs = drm_crtc_mask(txp_crtc)`; zpos initial `i + 1`, range 1..33. A failed allocation is skipped (`continue`) | same | read from source |
| S8 | A **Cursor**-type plane (`DRM_PLANE_TYPE_CURSOR`) is created for **every** CRTC (`drm_for_each_crtc`), `possible_crtcs = drm_crtc_mask(crtc)`, set as `crtc->cursor`, zpos initial 17, range 1..17. The loop runs after the overlays, so the cursor has the highest plane id of its group. The same code gives the TXP CRTC a cursor at zpos 17 inside a 1..33 range | same | read from source |
| S9 | The vc4 CRTCs on BCM2712 are: `bcm2712_pv0_data` (HVS channel mask BIT(0), output 0, encoder HDMI0), `bcm2712_pv1_data` (BIT(1), output 1, HDMI1), `bcm2712_mop_data` (name "mop", BIT(2), output 2, TXP0, `feeds_txp` true), `bcm2712_moplet_data` (name "moplet", BIT(1), output 4, TXP1). Compatibles: `brcm,bcm2712-pixelvalve0/1`, `brcm,bcm2712-mop`, `brcm,bcm2712-moplet`. The pixelvalves call `vc4_crtc_init(..., false)`; `vc4_txp_bind` calls it with `true` for both TXP units | `vc4_crtc.c`, `vc4_txp.c` | read from source |
| S10 | DSI, DPI and VEC on the Pi 5 are RP1 devices (`raspberrypi,rp1dsi`, `rp1dpi`, `rp1vec`), `status = "disabled"` in `rp1.dtsi`; they are not vc4 components. The vc4 driver on BCM2712 binds HDMI0, HDMI1, mop, moplet and the two pixelvalves | `arch/arm64/boot/dts/broadcom/rp1.dtsi`, `bcm2712.dtsi`; component list in `vc4_drv.c` | read from source (HDMI encoder matching in `vc4_hdmi.c` not read) |
| S11 | HVS6 has `HVS_NUM_CHANNELS` = 3 (DT interrupts `ch0-eof`, `ch1-eof`, `ch2-eof`). The GEN_6 output-to-FIFO map is: output 0 → channel 0, output 2 → channel 2, outputs 1, 3 and 4 → channel 1. So PV1 (output 1) and moplet (output 4) share channel 1 | `vc4_drv.h`; `vc4_hvs.c`, `vc4_hvs_get_fifo_from_output`; `bcm2712.dtsi` hvs node | read from source |
| S12 | HVS6 dlist memory size is read from the register `SCALER6_CXM_SIZE` at bind time (4096 only under kunit); `drm_mm_init(&hvs->dlist_mm, HVS_BOOTLOADER_DLIST_END, ...)` follows. The numeric size on the board and the relation to the second `drm_mm_init` that uses `SCALER_DLIST_SIZE` were not resolved | `vc4_hvs.c`, `__vc4_hvs_alloc` | read from source (value not read) |
| S13 | HVS6 LBM: `lbm_size = 1024`; UPM: `1024 * HVS_UBM_WORD_SIZE` (256); `VC4_NUM_LBM_HANDLES` 64, `VC4_NUM_UPM_HANDLES` 32 | `vc4_hvs.c`, `vc4_drv.h` | read from source |
| S14 | No fixed plane count exists in hardware terms in the source; the count is the software arithmetic of S2, S6, S7, S8 | — | inferred |

## 2. Derived plane arithmetic — inferred

With four vc4 CRTCs (S9: pv0, pv1, mop, moplet):

| Group | Count | possible CRTCs | Type | Basis |
|---|---|---|---|---|
| Primaries | 4 (one per CRTC) | own CRTC | Primary | S2 |
| Shared overlays | 16 | every CRTC except the first TXP CRTC found: pv0, pv1 and, by the `break` in S5, also the second TXP unit | Overlay | S5, S6 |
| TXP overlays | 32 | the first TXP CRTC only | Overlay | S5, S7 |
| Cursors | 4 (one per CRTC) | own CRTC | Cursor | S8 |
| **Total** | 4 + 16 + 32 + 4 = **56** | | | |

Per CRTC, the reachable plane count is:

| CRTC | Planes it can use | Arithmetic |
|---|---|---|
| First TXP CRTC (mop, if it is enumerated first) | 1 + 32 + 1 = 34 | primary + TXP overlays + cursor |
| Each of the other three CRTCs | 1 + 16 + 1 = 18 | primary + shared overlays + cursor |

The `break` in S5 means that when both mop and moplet bind, only the first-enumerated one is
treated as the TXP CRTC. That the other falls into the shared mask is the reading of S5; the
code was not run. — inferred

## 3. Reconciliation with the observed counts

Observed values (2026-11-28, from `observed/bcm2712-topology.md`): 4 CRTCs (ids 59 "mop", 78
"moplet", 94, 106, names from debugfs `state`); 56 planes; 4 primaries (one per CRTC);
16 shared overlays reachable from CRTC indices 1, 2, 3; 34 planes reachable from CRTC index 0
only; one extra plane per CRTC 1, 2 and 3 (ids 646, 657, 668), not typed.

| Item | Derived (§2) | Observed 2026-11-28 | Result |
|---|---|---|---|
| CRTC count | 4 | 4 (59 "mop", 78 "moplet", 94, 106) | match |
| CRTC names | mop, moplet, pv0, pv1 | "mop" and "moplet" named in debugfs | match; which of 94 and 106 is pv0 is not stated in the observed file |
| Total planes | 56 | 56 | match |
| Primaries | 4 | 4 (ids 48, 67, 83, 95) | match |
| Shared overlays | 16, on three CRTCs (moplet, pv0, pv1) | 16, on CRTC indices 1, 2, 3 | match |
| CRTC-0-only planes | 34 = primary + 32 + cursor | 34 | match, **if** the 34 includes primary 48 (see below) |
| Extra plane per CRTC 1/2/3 | 3 cursors (type Cursor, own CRTC, zpos 17) | 3 planes, ids 646, 657, 668, own CRTC only, not typed | consistent; type confirmed by source only |
| Mask of the first CRTC being mop | by S5 the TXP CRTC is excluded from the shared 16 | CRTC 0 is outside the shared mask and is "mop" | match |

**Answer to the 56-vs-57 question** raised in `hvs-backend-design.md` (the row at its line 133,
which notes the topology file counts "4 primaries and 34 CRTC-0-only planes"). The arithmetic
closes at 56 only when the 34 CRTC-0-only planes include CRTC 0's own primary (48): 34 + 3
primaries + 16 + 3 cursors = 56. Read as 34 *besides* the primary, the sum would be 57. The
source gives 1 + 32 + 1 = 34 for the TXP CRTC, so the first reading is the right one. — inferred
from S2, S7, S8 and the observed 56.

**Are the extra per-CRTC planes cursors?** By source, yes: S8 creates exactly one
`DRM_PLANE_TYPE_CURSOR` plane per CRTC, with `possible_crtcs` of that CRTC only, after the
overlays. The observed extra planes are single per CRTC, own-CRTC-only, and have the highest ids
of the groups (646, 657, 668 above the overlay ids 107..272), which fits. This is a consistent
fit, not a measurement: nobody has read `type` on ids 646, 657, 668 or on CRTC 0's cursor.
— inferred. The HVS Q5 board step is a `type` read of those four planes (see §5).

## 4. Differences and unexplained items

1. **CRTC 0's own cursor is not listed in the observed table.** The observed table lists
   cursors for CRTC 1, 2, 3 only. By S8 a fourth cursor exists for CRTC 0 and sits inside the
   34. It is not individually identified. — inferred.
2. **The 32 TXP overlays and CRTC-0 cursor types are not read on the board** ("not individually
   read"). The arithmetic accounts for all 34, but only planes 48, 83, 107, 239 and 382 were
   typed. — observed 2026-11-28 (what was typed); inferred (the rest).
3. **Which of pv0 and pv1 drives HDMI-A-1 on CRTC index 2.** The source gives pv0 → HDMI0 and
   pv1 → HDMI1 (S9) and the observed file reports CRTC id 94 active. The mapping from CRTC id
   or index to pv0/pv1 was not recorded; unexplained from these two sources alone.
4. **TXP cursor zpos.** S8 gives the TXP CRTC's cursor zpos 17 inside a 1..33 range, so by
   zpos the cursor is not topmost there. Not observed; it matters only for writeback, which the
   backend does not use. — inferred.
5. **HVS6 channels versus CRTCs.** Four CRTCs share three HVS channels (S11); pv1 and moplet
   share channel 1, so they cannot be driven at the same time through the HVS. The driver's
   allocation in `vc4_kms.c` was not read. — inferred; HVS Q5 board step: none needed.
6. **Dlist size** (S12) is a register value; it is not in the observed files either. Unexplained
   until `hvs_dlists` debugfs is read on the board.

No difference between the derived and observed plane counts remains unexplained, other than the
untyped planes in items 1 and 2.

## 5. What the board session should confirm (expected answers)

| Check | Expected from source |
|---|---|
| `type` of ids 646, 657, 668 and CRTC 0's fourth extra plane | Cursor (2); possible_crtcs = own CRTC |
| Plane counts per type | 4 Primary, 48 Overlay (16 + 32), 4 Cursor |
| Plane mask of CRTC index 0 | 34 planes: its primary, 32 overlays, its cursor |
| Plane mask of CRTC index 1, 2, 3 | 18 planes each: own primary, 16 shared overlays, own cursor |
| Cursor `zpos` | 17, range 1..17 (33 range on CRTC 0) |

## 6. Findings for the planner (outside this document's scope)

- The engine's view today keeps 16 of 56 planes (`MAX_PLANES` = 16, observed 2026-11-28). From
  S6 and S8, a reachable-planes view of one pixelvalve CRTC is 18 planes: primary, 16 shared
  overlays and its cursor. `MAX_PLANES` = 16 therefore cannot hold even one CRTC's full set.
  Raising it is a composition-core decision and is not made here. — inferred
- The source line numbers are missing (see the caveat). A later pass with a checkout can add
  them without changing any statement.
