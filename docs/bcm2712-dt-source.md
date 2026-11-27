# BCM2712 and BCM2711: CRTC and plane arithmetic from the device tree and the vc4 source

Answers open question 5 of [`hvs-backend-design.md`](hvs-backend-design.md) §7: the Pi 5 shows 4 CRTCs
and 56 planes, the CM4 6 CRTCs and 60 planes. Which device-tree node becomes which CRTC, how many
primary, cursor and overlay planes each CRTC gets, and whether the extra per-CRTC planes are cursors.

## Tags

- **read from source** (`file:symbol`): read from the Raspberry Pi kernel tree at the commit pinned
  below. Nothing was built or run for this page.
- **observed** (date): a value copied from the 2026-11-28 recordings of the Pi 5 (topology file
  `bcm2712-topology.md`) and of the CM4 (topology file `bcm2711-cm4-topology.md`, plus the CM4
  run file `bcm2711-cm4.md`), both in `docs/observed`. Those files are to be deleted; the values are
  repeated here and cited by file name only.
- **inferred**: derived from the two above, not read or measured directly.

No board was touched for this page. The boards ran kernels 6.18.50 (Pi 5) and 6.12.47 (CM4), both
observed 2026-11-28; the source below is the 6.18 branch, so for the CM4 the kernel versions differ
(inferred: the plane creation code did not change between them; not checked).

## Source pin

| Item | Value | Tag |
|---|---|---|
| Tree | Raspberry Pi kernel, branch `rpi-6.18.y` | read from source, 2026-11-28 |
| Commit | `16ae2649dcfadf9d81445b1177a3191efa3cb82e` (author date 2026-11-25, "media/hevc_d: Change warning messages to debug for control misconfiguration"), https://github.com/raspberrypi/linux/commit/16ae2649dcfadf9d81445b1177a3191efa3cb82e | read from source, 2026-11-28 |
| Files | `arch/arm64/boot/dts/broadcom/bcm2712.dtsi`, `bcm2712-rpi-5-b.dts`, `arch/arm/boot/dts/broadcom/bcm2711*.dtsi`, `bcm2711-rpi-cm4.dts`, `arch/arm/boot/dts/overlays/vc4-kms-v3d-pi4-overlay.dts`, `vc4-kms-v3d-pi5-overlay.dts`, `drivers/gpu/drm/vc4/vc4_{crtc,txp,plane,hvs,drv,kms}.c` | read from source, 2026-11-28 |

The files were fetched as raw text at that commit; no clone was made (a shallow clone was not possible
in this environment), so the file contents are as the fetch tool returned them and no line numbers
are given. Symbols are named instead.

## 1. How CRTCs and planes are created (read from source)

| Step | What the code does | Tag |
|---|---|---|
| CRTC per DT node | Every `brcm,*-pixelvalveN` node binds `vc4_crtc_bind`, every `brcm,bcm2712-mop` / `-moplet` / `brcm,bcm2835-txp` node binds `vc4_txp_bind`. Both call `vc4_crtc_init`, so each node is one CRTC | read from source: `vc4_crtc.c:vc4_crtc_bind`, `vc4_txp.c:vc4_txp_bind` |
| Primary plane | `vc4_crtc_init` calls `vc4_plane_init(drm, DRM_PLANE_TYPE_PRIMARY, 0)`, then `drmm_crtc_init_with_planes` attaches it to that CRTC only (passing `NULL` as cursor). One primary per CRTC; zpos immutable 0 | read from source: `vc4_crtc.c:vc4_crtc_init`, `vc4_plane.c:vc4_plane_init` |
| Overlay and cursor planes | Not created at CRTC bind time. `vc4_drm_bind` calls `vc4_plane_create_additional_planes` once, after `component_bind_all`, unless `firmware_kms` is set | read from source: `vc4_drv.c:vc4_drm_bind` |
| TXP CRTC chosen | The first CRTC with `feeds_txp` set (loop stops at the first). `feeds_txp` is passed as `true` by `vc4_txp_bind` and `false` by `vc4_crtc_bind` | read from source: `vc4_plane.c:vc4_plane_create_additional_planes` |
| Shared overlays | 16 (`VC4_NUM_OVERLAY_PLANES`), `possible_crtcs` = every CRTC except the one chosen as TXP CRTC (`non_txp_crtc_mask`), zpos range 1..17 | read from source: `vc4_plane.c:vc4_plane_create_additional_planes` |
| TXP overlays | 32 (`VC4_NUM_TXP_OVERLAY_PLANES`), `possible_crtcs` = the TXP CRTC only, zpos range 1..33 | read from source: `vc4_plane.c:vc4_plane_create_additional_planes` |
| Cursor planes | One per CRTC, **including the TXP CRTC**: a loop over all CRTCs after the overlays, `vc4_plane_init(DRM_PLANE_TYPE_CURSOR, drm_crtc_mask(crtc))`, zpos 17 in range 1..17 | read from source: `vc4_plane.c:vc4_plane_create_additional_planes` |
| Plane count is arbitrary | The code comment says the count is not set by hardware registers, only by HVS memory and the minimum plane size, and that a "modest number" is exposed | read from source: `vc4_plane.c:vc4_plane_create_additional_planes` (comment) |
| Plane order | Overlays first, then TXP overlays, then cursors, so object ids are in that creation order | read from source (loop order); id order observed 2026-11-28 below |

Total planes for N CRTCs: `N` primaries + 16 shared overlays + 32 TXP overlays + `N` cursors = `2N + 48`.
(inferred from the table above.) For N = 4 that is 56, for N = 6 it is 60.

## 2. BCM2712 (Raspberry Pi 5, HVS6)

### Device-tree nodes that become CRTCs

| DT node | Compatible | Becomes | Tag |
|---|---|---|---|
| `pixelvalve0` | `brcm,bcm2712-pixelvalve0`, HVS channel 0, encoder HDMI0 | CRTC, display pipe for HDMI-A-1 | read from source: `bcm2712.dtsi`, `vc4_crtc.c:bcm2712_pv0_data` |
| `pixelvalve1` | `brcm,bcm2712-pixelvalve1`, HVS channel 1, encoder HDMI1 | CRTC, display pipe for HDMI-A-2 | read from source: `bcm2712.dtsi`, `vc4_crtc.c:bcm2712_pv1_data` |
| `mop` | `brcm,bcm2712-mop`, HVS channel 2, encoder TXP0 (virtual), "Writeback-1" | CRTC feeding a writeback connector | read from source: `bcm2712.dtsi`, `vc4_txp.c:bcm2712_mop_data` |
| `moplet` | `brcm,bcm2712-moplet`, HVS channel 1 (shared with `pixelvalve1`, a mux), encoder TXP1 (virtual), "Writeback-2" | CRTC feeding a writeback connector | read from source: `bcm2712.dtsi`, `vc4_txp.c:bcm2712_moplet_data`, `vc4_kms.c:vc6_hvs_pv_muxing_commit` |
| `pixelvalve2..4`, `txp` | not present on BCM2712 | none | read from source: `bcm2712.dtsi` |
| `dsi0`, `dsi1`, `dpi`, `vec` | RP1 nodes in `rp1.dtsi`, not HVS pixelvalves | none of the vc4 CRTCs | read from source: `rp1.dtsi` |

The HVS has 3 channels (`HVS_NUM_CHANNELS` = 3, `vc4_drv.h`), so the 4 CRTCs share 3 channels: `mop`
takes channel 2, `pixelvalve0` channel 0, and `pixelvalve1` and `moplet` both list channel 1.
(read from source: `vc4_drv.h`, `vc4_crtc.c`, `vc4_txp.c`)

Status in the device tree: in `bcm2712.dtsi` the nodes `vc4` (the `gpu` node, compatible
`brcm,bcm2712-vc6`), `hvs`, `pixelvalve0/1`, `mop`, `moplet`, `disp_intr`, `hdmi0/1` are all
`status = "disabled"`, and `bcm2712-rpi-5-b.dts` does not enable any of them (it only repeats
`status = "disabled"` for `mop`, `moplet`, `pixelvalve0/1`, `disp_intr`, `hdmi0/1` and adds clocks to `hvs`).
(read from source: `bcm2712.dtsi`, `bcm2712-rpi-5-b.dts`.) The overlay `vc4-kms-v3d-pi5-overlay.dts`
enables `vc4`, `hvs`, `mop`, `moplet`, `pixelvalve0`, `pixelvalve1`, `hdmi0`, `hdmi1` and `disp_intr`
(and sets `vec` to `disabled`). (read from source: `vc4-kms-v3d-pi5-overlay.dts`.) Whether that
overlay was the one loaded on the recorded board is not in the recording; the observed 4 CRTCs and
2 TMDS plus 2 VIRTUAL encoders match it. (inferred)

### CRTC order

| Item | Value | Tag |
|---|---|---|
| Component bind order | `vc4_hvs_driver`, hdmi, vec, dpi, dsi, `vc4_txp_driver`, `vc4_crtc_driver`; the comment says the TXP must bind before the pixelvalves | read from source: `vc4_drv.c:component_drivers` |
| Resulting CRTC index | 0 = `mop`, 1 = `moplet`, 2 = `pixelvalve0`, 3 = `pixelvalve1` | inferred (bind order of the list above; the order of the two TXP nodes among themselves and of the two pixelvalves was not traced) |
| Observed CRTC ids | 59, 78, 94, 106; only index 2 (id 94) active, driving HDMI-A-1 | observed 2026-11-28 (bcm2712-topology.md) |
| Observed encoders and connectors | 2 TMDS + 2 VIRTUAL encoders; HDMI-A-1 connected, HDMI-A-2 disconnected, Writeback-1 and -2 | observed 2026-11-28 (bcm2712-topology.md) |

The active index 2 is `pixelvalve0` with HDMI-A-1: consistent. (inferred)

### Planes per CRTC (BCM2712)

The TXP CRTC chosen by the source is `mop` (the first with `feeds_txp`), so index 0. `moplet` also feeds a
writeback connector but is not the chosen TXP CRTC: it gets the shared overlays like a display CRTC.
(inferred from the "first CRTC wins" loop in `vc4_plane_create_additional_planes`.)

| CRTC index (node) | Primary | Cursor | Shared overlays (zpos 1..17) | TXP overlays (zpos 1..33) | Planes that can reach it (`possible_crtcs` bit set) | Tag |
|---|---|---|---|---|---|---|
| 0 (`mop`) | 1 | 1 | 0 | 32 | 34 | inferred from source |
| 1 (`moplet`) | 1 | 1 | 16 | 0 | 18 | inferred from source |
| 2 (`pixelvalve0`) | 1 | 1 | 16 | 0 | 18 | inferred from source |
| 3 (`pixelvalve1`) | 1 | 1 | 16 | 0 | 18 | inferred from source |

The 16 shared overlays have `possible_crtcs` = CRTCs 1, 2, 3 (the mask is `GENMASK(num_crtc-1, 0)` minus
the TXP CRTC); the 32 TXP overlays have CRTC 0 only; each primary and cursor has its own CRTC only.
(read from source: `vc4_plane_create_additional_planes`, `vc4_plane_init`.)

### Arithmetic against the observed 56

| Group | Count | `possible_crtcs` (indices) | Tag |
|---|---|---|---|
| Primaries | 4 | each its own CRTC | read from source (`vc4_crtc_init`); ids 48, 67, 83, 95 observed 2026-11-28 |
| Shared overlays | 16 | 1, 2, 3 | read from source; observed 2026-11-28: 16 shared overlays, ids 107 to 272, mask CRTCs 1, 2, 3 |
| TXP overlays | 32 | 0 | read from source |
| Cursors | 4 | each its own CRTC | read from source (`vc4_plane_create_additional_planes`) |
| Sum | 4 + 16 + 32 + 4 = 56 | | matches observed 56 planes, 2026-11-28 |

Cross-check with the observed groups (bcm2712-topology.md, 2026-11-28):

| Observed group | Source reading | Tag |
|---|---|---|
| "CRTC-0-only planes: 34" | 1 primary + 32 TXP overlays + 1 cursor = 34 | observed 34; source arithmetic matches |
| "One extra plane per CRTC 1/2/3" (ids 646, 657, 668) | the cursor planes of CRTCs 1, 2, 3 | observed ids 2026-11-28; "are cursors" now read from source |
| Total | 34 + 16 + 3 x (primary + cursor) = 34 + 16 + 6 = 56 | arithmetic |

Object-id check: ids run in creation order with step 11 (observed: overlays 107, 118, ... 272). Continuing
the step, the 32 TXP overlays take 283 to 624, and the cursor loop then gives 635 (CRTC 0), 646, 657, 668
(CRTCs 1, 2, 3). The observed extra planes 646, 657, 668 are exactly cursors 1 to 3 in that sequence. The
CRTC 0 cursor (635) was not among the planes whose types or ids were listed. (inferred)

### Why the topology file and the design doc counted 57

`hvs-backend-design.md` §5 gives "4 primaries and 34 CRTC-0-only planes, which would give 57". The 34 already
contains CRTC 0's primary (plane 48), because it was counted from the possible-CRTC masks and that
primary has mask CRTC 0 only. Adding "4 primaries" counts plane 48 twice: 4 + 34 + 16 + 3 = 57, whereas the
distinct planes are 3 other primaries + 34 + 16 + 3 cursors = 56. The difference is a double count, not a
missing plane. (inferred from the group definitions in bcm2712-topology.md, 2026-11-28, and the source above)

### Cursors

The extra per-CRTC planes are cursors by construction: `vc4_plane_create_additional_planes` creates a
`DRM_PLANE_TYPE_CURSOR` plane in a loop over every CRTC, and `vc4_crtc_bind` / `vc4_txp_bind` create no
cursor of their own (the cursor argument of `drmm_crtc_init_with_planes` is `NULL`). (read from source) The
`type` property of plane 646 / 657 / 668 was not read on the board, so "type = Cursor" is not observed.
(observed 2026-11-28: types read for planes 48, 83, 107, 239 and 382 only)

## 3. BCM2711 (CM4, HVS5)

### Device-tree nodes that become CRTCs

| DT node | Becomes | Tag |
|---|---|---|
| `txp` (`brcm,bcm2835-txp`, `bcm283x.dtsi`) | CRTC feeding Writeback-1, HVS channel 2 | read from source: `vc4_txp.c:bcm2835_txp_data` |
| `pixelvalve0` (`brcm,bcm2711-pixelvalve0`) | CRTC, encoders DSI0 / DPI, channel 0 | read from source: `vc4_crtc.c:bcm2711_pv0_data` |
| `pixelvalve1` | CRTC, encoders DSI1 / SMI, channels 0..2 | read from source: `vc4_crtc.c:bcm2711_pv1_data` |
| `pixelvalve2` | CRTC, encoder HDMI0, channels 0..2 | read from source: `vc4_crtc.c:bcm2711_pv2_data` |
| `pixelvalve4` | CRTC, encoder HDMI1, channels 0..2 | read from source: `vc4_crtc.c:bcm2711_pv4_data` |
| `pixelvalve3` | CRTC, encoder VEC, channel 1 | read from source: `vc4_crtc.c:bcm2711_pv3_data` |
| `mop`, `moplet`, `disp_intr` | not present on BCM2711 | read from source |

Status: `bcm2711-rpi-ds.dtsi` sets `pixelvalve0..4` and `vc4` to `disabled`; `bcm2711-rpi-cm4.dts` sets
`pixelvalve0`, `1`, `2`, `4`, `vc4` and the HDMI nodes to `okay`, `vec` to `disabled`, and has no `pixelvalve3`
block. By plain merge order `pixelvalve3` stays `disabled` in the device tree. The overlay
`vc4-kms-v3d-pi4-overlay.dts` enables `pixelvalve0` to `pixelvalve4` (fragments 6 to 10), `hvs`, `txp`, `vc4`
and both HDMI nodes, and sets `vec` to `disabled`. (read from source) The observed 6 CRTCs include
`pixelvalve-3`, which fits the overlay having enabled it, not the plain device tree. That the overlay
was the one in use on the recorded board was not read from the board. (inferred)

### CRTC order and arithmetic

| Item | Value | Tag |
|---|---|---|
| Observed CRTCs, in order | `txp` (59), `pixelvalve-0` (78), `pixelvalve-1` (90, active, DSI-1), `pixelvalve-2` (102), `pixelvalve-4` (114), `pixelvalve-3` (126) | observed 2026-11-28 (bcm2711-cm4-topology.md) |
| Order explained | TXP binds before the pixelvalves; among the pixelvalves the order follows the device tree, where `pixelvalve4` (`7e216000`) precedes `pixelvalve3` (`7ec12000`) | inferred (bind order from `vc4_drv.c:component_drivers`; the node-order rule is not traced) |
| TXP CRTC chosen by the source | `txp`, index 0 (the only `feeds_txp` CRTC) | read from source: `vc4_plane_create_additional_planes` |

| Group | Count | `possible_crtcs` (indices) | Tag |
|---|---|---|---|
| Primaries | 6 | each its own CRTC | read from source; ids 48, 67, 79, 91, 103, 115 observed 2026-11-28 |
| Shared overlays | 16 | 1 to 5 (everything but `txp`) | read from source |
| TXP overlays | 32 | 0 | read from source |
| Cursors | 6 | each its own CRTC | read from source |
| Sum | 6 + 16 + 32 + 6 = 60 | | matches observed 60 planes, 2026-11-28 |

Cross-check with the id order recorded in the 2026-11-28 CM4 topology file: ids 48 to 710 in steps of 11;
plane-0 to 5 primaries, plane-6 to 21 shared overlays (zpos 1 to 16), plane-22 to 53 TXP overlays (zpos
up to 0x20), plane-54 to 59 with zpos 17 (observed 2026-11-28). That is 6 + 16 + 32 + 6, the same order as
the creation loops. The six zpos-17 planes are the cursors (inferred before; now consistent with
`vc4_plane_create_additional_planes`, which gives the cursor zpos 17). Their `type` was not read on the
board.

| CRTC index (node) | Primary | Cursor | Shared overlays | TXP overlays | Reachable planes | Tag |
|---|---|---|---|---|---|---|
| 0 (`txp`) | 1 | 1 | 0 | 32 | 34 | inferred from source |
| 1 (`pixelvalve0`) | 1 | 1 | 16 | 0 | 18 | inferred from source |
| 2 (`pixelvalve1`, DSI-1) | 1 | 1 | 16 | 0 | 18 | inferred from source |
| 3 (`pixelvalve2`) | 1 | 1 | 16 | 0 | 18 | inferred from source |
| 4 (`pixelvalve4`) | 1 | 1 | 16 | 0 | 18 | inferred from source |
| 5 (`pixelvalve3`) | 1 | 1 | 16 | 0 | 18 | inferred from source |

## 4. What this means for `MAX_PLANES` = 16

`MAX_PLANES` is 16 (`crates/composition-core/src/limits.rs`, read from source in this repository). The cut is
`keep_planes` in `crates/composition-drm/src/lib.rs` (read only): planes that can reach a probed output
come first, then within each group primaries, then cursors, then overlays; the list is truncated to 16 and
the survivors are re-ordered as reachable first, primary, overlays, cursor. (read from source: `lib.rs:keep_planes`)

| Case | Reachable planes | Kept | Dropped | Tag |
|---|---|---|---|---|
| Pi 5, connected output = CRTC index 2 (`pixelvalve0`) | 18: primary 83, 16 shared overlays, cursor 657 | primary, cursor and 14 overlays | 2 reachable overlays, plus all 38 planes that cannot reach the connected output (the 34 CRTC-0-only planes, the primaries of CRTCs 1 and 3, and the cursors of CRTCs 1 and 3) | inferred from `keep_planes` and the table in section 2 |
| CM4, connected output = DSI-1 on `pixelvalve1` (index 2) | 18: primary 79, 16 shared overlays, its cursor | primary, cursor and 14 overlays | 2 reachable overlays, all unreachable planes | inferred from `keep_planes` and the table in section 3 |
| A writeback output (index 0) were probed as well | 34 more reachable planes | the 16-plane cut would then bite on the combined set | | inferred |

- With reachable planes sorted first, the cut never sacrifices a reachable primary or cursor on either board; it
  loses at most two shared overlays. This agrees with the plan #457 note that output 0 reaches 18 planes and
  keeps 16 on the CM4. (inferred; that run is not part of this page)
- The topology recordings (bcm2712-topology.md, bcm2711-cm4-topology.md, 2026-11-28) were made before the
  reachable-first sort: they show 4 and 6 primaries among the 16 kept planes (observed 2026-11-28). Those
  lists are the old behaviour, not what the code now keeps.
- Per-CRTC reach is the same shape on both SoCs: 1 primary + 1 cursor + 16 shared overlays = 18 planes for a
  display CRTC; only the writeback CRTC reaches 34 (32 overlays). So any profile that wants all planes of a
  display CRTC needs a limit of at least 18, not 56 or 60. (inferred)
- Planes beyond 16 are not usable together anyway for other reasons (dlist space, LBM/UPM, bandwidth), which
  this page does not examine.
- A table keyed on `possible_crtcs` should treat the 32 TXP overlays as writeback-only: they are never
  reachable from a connector-driving CRTC. (read from source: mask in `vc4_plane_create_additional_planes`)

## 5. Open points left after this reading

| Point | State | Tag |
|---|---|---|
| Raspberry Pi's tree was read through fetched text at one commit; no line numbers | Re-check with a clone if line anchors are wanted | read from source |
| `type` of the per-CRTC extra planes on the boards | Not read on a board; a single `props` read of plane 646 / 657 / 668 (Pi 5) or 655 and later (CM4) would turn "cursor" from source into observed | inferred |
| Order of CRTCs among equal-type nodes (`mop` vs `moplet`, `pixelvalve0` vs `1`) | Not traced; the active index matches observed 2 | inferred |
| Whether the CM4 kernel 6.12 creates the planes the same way as 6.18 | The counts agree (60), so no difference is visible | inferred |
