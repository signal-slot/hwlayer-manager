# RK3588 VOP2 and vc4 HVS against MAX_PLANES and MAX_FORMATS: decision brief

Written 2026-11-28 (host UTC) for the owner. Nothing here changes code, and
no board was used. Every row is built from what the repository already
records. Tags: **observed** (with the date of the recording),
**read from source** (with the file and tree), **inferred**. Values that
board sessions still in progress will measure are marked **not measured
yet** and are not guessed.

## 1. The caps as they stand

| Cap | Value | Tag | Where |
|---|---|---|---|
| `MAX_PLANES` | 16, and a `const` assert keeps it at or below 32 because plane sets are `u32` bit masks | read from source (`crates/composition-core/src/limits.rs`, repository HEAD) | `limits.rs:11`, `limits.rs:26` |
| `MAX_FORMATS` | 32 format/modifier pairs per plane | read from source (same file) | `limits.rs:15` |
| `MAX_OUTPUTS` | 4 | read from source (same file) | `limits.rs:7` |
| `MAX_LAYERS` | 16, also the second dimension of the planner's per-layer by per-plane reject table | read from source (`planner.rs:359`, repository HEAD) | `limits.rs:13` |
| What the DRM backend does at the cap | planes: reachable first, then primaries, cursors, overlays, truncated to `MAX_PLANES` before the final order; pairs: linear first, `take(MAX_FORMATS)` | read from source (`crates/composition-drm/src/lib.rs:1407-1433`, `:533`, repository HEAD) | [rk3588-backend-design.md §1](rk3588-backend-design.md), [hvs-backend-design.md §2](hvs-backend-design.md) |

The cut is silent: the planner never sees a dropped plane or pair, so a
dropped AFBC pair shows up as a `FORMAT` refusal and a dropped plane as a
`NoPlane` (inferred from the backend design docs).

## 2. What the SoCs have

### RK3588 VOP2 (NanoPC-T6, kernel 6.18.55, 2026-11-28)

| Item | Value | Tag | Doc |
|---|---|---|---|
| Planes, total | 8 (Cluster0-3 win0, Esmart0-3 win0), one window each | observed 2026-11-28 | [observed/rk3588-topology.md](observed/rk3588-topology.md) |
| CRTCs registered | 2 (`video_port0`, `video_port1`); one connected | observed 2026-11-28 | [observed/rk3588-topology.md](observed/rk3588-topology.md) |
| Planes per CRTC | 7 usable on each: its own Cluster primary (Cluster0 on port 0, Cluster1 on port 1) plus the six movable planes 45..75, which reach both ports (`0b11`) | read from source (mainline 6.18 `rockchip_drm_vop2.c`, pin in the doc) | [rk3588-dt-source.md](rk3588-dt-source.md) |
| Plane types | Cluster0, Cluster1 Primary; the other six Overlay; no cursor plane | observed 2026-11-28 | [observed/rk3588-topology.md](observed/rk3588-topology.md) |
| Pairs per Cluster plane | 126 = 14 formats x 9 AFBC modifiers, no linear pair | observed 2026-11-27 (raw blob); read from source (mainline 6.18, `rockchip_vop2_reg.c`) | [rk3588-vop2-formats-source.md](rk3588-vop2-formats-source.md) |
| Pairs per Esmart plane | 21 (21 formats, linear only) | observed 2026-11-27 and 2026-11-28 | [rk3588-vop2-formats-source.md](rk3588-vop2-formats-source.md), [observed/rk3588-topology.md](observed/rk3588-topology.md) |
| Vendor 6.1 variant | 4 video ports; 8 planes, or 12 with DT `support-multi-area` | read from source (`rockchip-linux/kernel` `develop-6.1` @ `77168c8d`) | [rk3588-backend-design.md §3](rk3588-backend-design.md) |

Do the caps hide anything?

- **Planes: no.** 8 (12 with multi-area) is below 16. Observed 2026-11-28 the
  probe showed Cluster1 as `outputs 0b0` only because it sees connected
  outputs only; that is a mask matter, not a cap matter.
- **Formats: yes, on all four Cluster planes.** 126 pairs against 32 leaves
  94 pairs invisible per plane (arithmetic on the observed 126; inferred
  consequence). The raw 2026-11-27 list has no linear pair, so "linear
  first" has nothing to sort, and which 32 survive follows the kernel's
  order (inferred). The recorded probe line starts with 14 pairs of the
  first AFBC modifier (observed 2026-11-28), so the survivors are probably
  three or fewer of the nine modifiers (inferred). Esmart (21) is intact.
- Whether the hidden pairs are usable (the producer may only ever hand out
  one or two modifiers): **not measured yet**, a question of the
  `t6-topology-session` item (Q1).

### vc4 HVS5 (BCM2711, reTerminal CM4, 2026-11-28)

| Item | Value | Tag | Doc |
|---|---|---|---|
| Planes, total | 60 = 6 primaries + 16 shared overlays + 32 writeback overlays + 6 cursors | read from source (vc4 `vc4_plane_create_additional_planes`, 6.18 pin in the doc); matches observed 60 on 2026-11-28 | [bcm2712-dt-source.md](bcm2712-dt-source.md), [observed/bcm2711-cm4.md](observed/bcm2711-cm4.md) |
| CRTCs | 6: `txp` (writeback) plus 5 pixelvalves | observed 2026-11-28 | [observed/bcm2711-cm4-topology.md](observed/bcm2711-cm4-topology.md) |
| Planes per display CRTC | 18 = own primary + 16 shared overlays + own cursor; the 32 writeback overlays reach `txp` only | read from source (same file) | [bcm2712-dt-source.md](bcm2712-dt-source.md) |
| Formats per plane | 35 formats, 5 modifiers in the blob; 34 of the formats are linear | observed 2026-11-27 (blob) | [vc4-hvs-format-source.md](vc4-hvs-format-source.md) |
| Pairs per plane (total) | not counted in the repository | **not measured yet** | |
| What the engine saw | 16 planes, 32 formats per plane | observed 2026-11-28 | [observed/bcm2711-cm4-topology.md](observed/bcm2711-cm4-topology.md) |

Do the caps hide anything?

- **Planes: yes.** 60 against 16. A display CRTC reaches 18, so even a
  single CRTC does not fit whole. The ranking keeps reachable primaries first;
  observed 2026-11-28 five of the 16 slots went to primaries of unconnected
  CRTCs ([hvs-backend-design.md §4](hvs-backend-design.md)).
- **Formats: yes.** 34 linear formats against 32 (the 2 dropped are
  inferred from the two counts); the modifier pairs after the linear ones
  are cut as well, so the T-tiled and SAND pairs are the first to go
  (inferred from "linear first").

### vc4 HVS6 (BCM2712, Raspberry Pi 5, 2026-11-28)

| Item | Value | Tag | Doc |
|---|---|---|---|
| Planes, total | 56 = 34 on CRTC 0 only (32 writeback overlays, primary, cursor) + 16 shared overlays + 3 x (primary + cursor) | observed 2026-11-28 (56); split read from source and arithmetic | [bcm2712-dt-source.md](bcm2712-dt-source.md), [observed/bcm2712-topology.md](observed/bcm2712-topology.md) |
| CRTCs | 4 (ids by index 0..3); only index 2 active | observed 2026-11-28 | [observed/bcm2712-topology.md](observed/bcm2712-topology.md) |
| Planes per display CRTC | 18 = own primary + 16 shared overlays + own cursor (the extra plane being a cursor is read from source, not yet seen on the board) | read from source; board check **not measured yet** (HVS Q5, `rpi5-hvs6-scale-session`) | [bcm2712-dt-source.md](bcm2712-dt-source.md) |
| Formats per plane | 45 formats, 5 modifiers; 44 formats linear (P030 has no linear pair) | observed 2026-11-27 (blob); read from source (45-entry table, 6.18 pin) | [vc4-hvs-format-source.md](vc4-hvs-format-source.md) |
| Pairs per plane (total) | not counted in the repository | **not measured yet** | |
| What the engine saw | 16 of 56 planes (40 dropped), 32 of 45 linear formats (13 dropped) | observed 2026-11-28 | [hvs-backend-design.md §4](hvs-backend-design.md) |

Do the caps hide anything? Yes on both: 40 planes and at least 13 formats
(more pairs, since the 44 linear formats already fill the 32 and every
non-linear pair is past the cut; inferred). The modifier rows disagree
between blob and source (T-tiled and SAND64 are listed by the blob but
refused by the driver on gen 6), see
[vc4-hvs-format-source.md](vc4-hvs-format-source.md); that is a table
matter, not a cap matter.

### Profiles

The `Rk3588Vop2`, `Vc4Hvs5` and `Vc4Hvs6` variants of `DriverProfile`
(`crates/composition-drm/src/profile.rs:140-147`, repository HEAD) adjust
a plane after the backend has read it. They run after the cut and cannot
bring back a plane or pair that was cut (read from source,
`profile.rs:211-213`). Only the cap or the discovery step can change that.

## 3. Summary

| SoC | Planes in hardware | Reachable per CRTC | Cap 16 hides planes | Pairs per plane | Cap 32 hides pairs |
|---|---|---|---|---|---|
| RK3588 | 8 (observed 2026-11-28) | 7 (read from source) | no | Cluster 126, Esmart 21 (observed 2026-11-27) | Cluster: 94 each; Esmart: none |
| BCM2711 | 60 (observed 2026-11-28) | 18 (read from source) | yes, 44 | 35 formats x 5 modifiers, total not counted | yes (inferred) |
| BCM2712 | 56 (observed 2026-11-28) | 18 (read from source; not measured yet on board) | yes, 40 (observed 2026-11-28) | 45 formats x 5 modifiers, total not counted | yes, at least 13 formats (observed 2026-11-28) |

## 4. Options

**A. Keep both caps.** Cost to the core: none. Cost to the boards: RK3588
Cluster planes lose 94 pairs each, which may hide the AFBC pairs a GPU
or decoder actually hands out; HVS loses most overlays and all non-linear
pairs. The planner stays a bounded problem (16 layers x 16 planes). The
backend design docs already treat this as a conscious scope limit.

**B. Raise the caps globally.** Sizes are compile-time and every table
is sized by them, so memory grows at once for every target, including the
Cortex-M builds. `PlaneCaps` holds the pair array inline, so
`MAX_FORMATS` 32 to 128 multiplies that array by four per plane, times
`MAX_PLANES` (rough, inferred; not measured). `MAX_FORMATS` is a plain
constant, so 128 is a one-line change plus the thumbv7em builds. For
`MAX_PLANES`: up to 32 needs no type change; to cover 56 or 60 planes
the `u32` masks (`outputs`, `ResourceGroup::planes`, candidate sets,
the assert at `limits.rs:26`) would have to become `u64`, which is wider
integer work on the no_std targets and touches the FFI header (inferred;
the FFI was not read for this brief). The planner's search cost also
grows with planes (read from source, `planner.rs` per-plane tables).
No new dependency is needed in either case.

**C. Make the cap per profile.** Not expressible without either const
generics threaded through `Capabilities`, `PlaneCaps`, the planner and the
FFI types, or a heap, and the core is `no_std` and dependency-free. A
cheaper variant is a build-time feature or `cfg` that selects the constant
(for example a small set for the M-class firmware, a larger one for Linux
hosts): no new dependency and no heap, but one binary no longer fits all
and the tests have to run for each setting (inferred).

**D. Prune in the backend instead (no core change).** Pick the 32 pairs
and 16 planes better (for example the modifiers a producer lists, one
representative AFBC modifier per format). Cost to the core: none; the cost is
in `composition-drm` and loses pairs the planner could match. It needs
the producer's modifier list, which the repository does not have yet.

## 5. What is not decided here

- Which option to take, and what the memory budget on the Cortex-M and
  R5F targets allows: the owner's decision.
- The facts that would settle the RK3588 side (are the hidden Cluster
  pairs ever used) and the per-CRTC counts on BCM2712 are **not measured
  yet**; they come from the running board sessions, and this brief must be
  re-read when those land.
- `MAX_PLANES` and `MAX_FORMATS` stay as they are.
