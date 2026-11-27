# Design note: HVS backend (Raspberry Pi 5 BCM2712 and CM4 BCM2711)

Status: design only, written 2026-11-28. **No measurement is claimed here and no code is
changed.** Nothing in this file was run. It is the design note of the roadmap's HVS backend. An
older, longer note on the same subject, [`../hvs-backend-design.md`](../hvs-backend-design.md), is
not replaced by this one.

## Tags

Every hardware statement carries one of three tags.

- **observed YYYY-MM-DD**: measured on a board and recorded in
  [`../observed/bcm2712-topology.md`](../observed/bcm2712-topology.md),
  [`../observed/bcm2712.md`](../observed/bcm2712.md),
  [`../observed/bcm2711-cm4-topology.md`](../observed/bcm2711-cm4-topology.md) or
  [`../observed/bcm2711-cm4.md`](../observed/bcm2711-cm4.md). This file re-measured nothing; the
  date is the one those files give.
- **source**: read from the Raspberry Pi kernel driver (`drivers/gpu/drm/vc4`, branch `rpi-6.18.y`,
  commit `16ae2649dcfa`, read 2026-11-28), as recorded in
  [`../observed/vc4-hvs-source.md`](../observed/vc4-hvs-source.md). The boards run other builds
  (Pi 5: `6.18.50+rpt-rpi-2712`; CM4: `6.12.47+rpt-rpi-v8`, both observed 2026-11-28), so a source
  row is not an observation of that board.
- **inferred**: a conclusion of this note, from observed or source rows. Statements about the
  engine's own crates are read from the repository on 2026-11-28 and are tagged **repo**.

`DrmBackend`, `DriverProfile`, `PlaneCaps`, `ResourceGroup`, `MAX_PLANES` (16) and `MAX_FORMATS`
(32) are from `crates/composition-core` and `crates/composition-drm` (**repo**). Board addresses,
host names and credentials are not in this file.

## 1. Summary

- Both generations run the same driver (`vc4`) and give the engine the same shape of topology:
  one primary per CRTC (zpos fixed 0), a pool of 16 overlays shared by all display CRTCs (zpos
  mutable 1..17), one cursor per CRTC, and plane-heavy writeback CRTCs. Plane alpha, three blend
  modes and 0/180/reflect rotation are on every plane. — observed 2026-11-28 (both boards), source
- The two-plane path (primary plus one overlay through the shared API: `TEST_ONLY`, commit, latch,
  release, buffer rotation) worked unchanged on both boards with the generic profile: 0 dropped,
  0 late. — observed 2026-11-28
- What the generic reading gets wrong or loses is not capability but **enumeration and claims**:
  the plane list is cut to 16 before reachability is considered, formats are cut to 32 with no
  modifier, scaling is not claimed and rotation is not programmed. — observed 2026-11-28
- The limits that bind on the HVS (display-list words, LBM, UPM, bandwidth) are not countable
  units of the engine's `ResourceGroup`. `TEST_ONLY` has the last word; a profile adds only cheap
  early refusals. — inferred
- The generations differ in modifiers, formats, LBM/UPM, source-width check, clock handling and
  addressing (§7). They are told apart by an explicit profile choice, because the driver name is
  `vc4` on both. — source; inferred

## 2. Topology as recorded

| Item | BCM2711 (CM4, reTerminal) | BCM2712 (Pi 5) |
| --- | --- | --- |
| Kernel, driver | `6.12.47+rpt-rpi-v8`, `vc4` on `card0` — observed 2026-11-28 | `6.18.50+rpt-rpi-2712`, `vc4` on `card1` — observed 2026-11-28 |
| Active output | DSI-1, 720x1280 @ 60 Hz, `pixelvalve-1`, HVS channel 0 — observed 2026-11-28 | HDMI-A-1, 1920x1080 @ 60 Hz, CRTC index 2 of 4 — observed 2026-11-28 |
| CRTCs, observed | 6: `txp` plus `pixelvalve-0..4` — observed 2026-11-28 | 4: ids 59 and 78 are the writeback `mop`/`moplet`, 94 and 106 display — observed 2026-11-28 |
| CRTCs, driver | 5 pixelvalves + TXP, 3 HVS channels — source | 2 display pixelvalves, 2 writeback blocks, 3 HVS channels — source |
| Planes in the kernel | 60 — observed 2026-11-28 | 56 — observed 2026-11-28 |
| Planes in the engine | 16 (6 primaries, 10 overlays), 11 on the active output — observed 2026-11-28 | 16 (4 primaries, 12 overlays), 13 on the active output — observed 2026-11-28 |
| Planes the kernel gives the active output | primary + 16 shared overlays, plus a cursor (six zpos-17 planes; types not read) — observed 2026-11-28; cursor inferred | primary + 16 shared overlays + plane 657, not typed — observed 2026-11-28; cursor inferred |
| Layout of all planes | plane-0..5 primaries, 6..21 shared overlays, 22..53 TXP overlays (zpos 1..32), 54..59 zpos 17 — observed 2026-11-28 | 4 primaries, 16 shared overlays (CRTCs 1..3), 34 planes on CRTC 0 only, 3 extra planes (CRTCs 1..3). These sum to 57, one more than the 56 counted; the recorded counts are not reconciled here — observed 2026-11-28; arithmetic inferred |
| zpos | primary fixed 0; overlay 1..17 mutable — observed 2026-11-28 | same — observed 2026-11-28 |
| Plane alpha, blend | `alpha`; none, premultiplied, coverage on every plane the probe reported — observed 2026-11-28 | same; default premultiplied, read on five planes — observed 2026-11-28 |
| Rotation | `rotation=1` (rotate-0) on all planes; supported mask not read — observed 2026-11-28 | rotate-0, rotate-180, reflect-x, reflect-y; no 90/270 — observed 2026-11-28 |
| Underruns, errors | `hvs_underrun` 0; no new kernel messages — observed 2026-11-28 | `hvs_underrun` 0; no DRM, vc4 or HVS messages — observed 2026-11-28 |
| Display capture | none: nothing looked at the panel, so no visual check — observed 2026-11-28 | KVM gave a picture; z-order and alpha checked on the output — observed 2026-11-28 |

The 16 overlays are one allocation with a mask over all non-writeback CRTCs, not 16 per CRTC
(source). Neither board had a second output active, so sharing between outputs was not observed.

## 3. Mapping onto crates/composition-drm and DriverProfile

How the HVS facts reach `PlaneCaps`. The "design" column is inferred.

| HVS fact | Engine field | Design |
| --- | --- | --- |
| Primary zpos immutable 0 | `PlaneKind::Primary`, `Zpos::Fixed(0)` | Read from KMS; nothing to add (probe printed `zpos fixed 0` on both boards, observed 2026-11-28) |
| Overlay zpos mutable 1..17 | `Zpos::Range` | Read from KMS (observed 2026-11-28). The dlist is written in increasing normalised zpos, so zpos is the blend order (source) |
| Cursor, zpos 17 in range 1..17, on top by default | `PlaneKind::Cursor` | Types were not read on either board, so a cursor-typed plane was not observed. Keep cursors last in the plane order and do not use them for layers |
| Overlay pool shared by CRTCs | per-plane `outputs` mask | The mask lists every output a plane reaches (observed 2026-11-28, `0b1` on the probed output). The pool is not a per-output count, so it gets no group |
| Plane alpha, blend modes | `plane_alpha`, `blend_modes` | From KMS, identical on every plane on both boards (observed 2026-11-28). No profile entry |
| Rotation 0/180/reflect | `transforms` | The kernel offers them; composition-drm reported `transforms 0x0` and refused rotate-180 at the capability layer (observed 2026-11-28). A profile cannot fix this alone: the `rotation` property has to be read and programmed (**repo**) |
| Scaling | `scaling: Option<ScaleLimits>` | KMS has no scaling property; both runs had scaling not claimed. A hand-built plan claiming up to 16x passed `TEST_ONLY` for 2x up and 1/2 down on both boards (observed 2026-11-28). Larger ratios were not tried. A profile would set `Some(..)` on non-cursor planes with a conservative bound and leave refusal to `TEST_ONLY` |
| Source width | `max_source` | HVS5: width 8191 (source). HVS6: no such check in the source (source). A profile would set it for HVS5 only |
| Formats and modifiers | `formats`, max 32 | See §5 |
| Latch, release | `BackendEvent::Latched` | DRM page-flip events as `DrmBackend` already uses. Per run, every committed frame latched and every buffer use was released (e.g. 600/600 latched, 1198 released during the run and 2 at the end) on both boards (observed 2026-11-28) |
| `replace_pending` | `Capabilities::replace_pending` | Source: each used channel waits for the previous commit on it (`pending_commit`), which suggests false. Not tested (question 6) |

### 3.1 Plane enumeration: the `MAX_PLANES` truncation

Observed on both boards (2026-11-28): `DrmBackend` sorts planes by kind and then keeps
`MAX_PLANES` = 16 (`planes.truncate(MAX_PLANES)` in `crates/composition-drm/src/lib.rs`,
**repo**). The cut comes **before** planes are filtered by reachability. On the CM4 five of the
sixteen slots go to primaries of CRTCs that are not connected, and six reachable overlays are
lost (11 planes left on the output). On the Pi 5 three slots go to such primaries and the last
four shared overlays (ids 239, 250, 261, 272) are lost (13 left). The two-plane run is not
affected (observed 2026-11-28).

This is a known finding. It is being fixed in `crates/composition-drm` by the plan item
`drm-plane-truncate` (filter planes by CRTC before applying `MAX_PLANES`). That item is not part
of this one and its change was not read here. This note assumes the fix: each probed output then
gets its own primary, the overlays that reach it, and a cursor, up to 16. Two points stay open
after the fix and are design decisions, not bugs:

1. With one active output the HVS offers primary + 16 overlays + a cursor, one or two more than
   16. Seeing all of them needs a larger `MAX_PLANES` (a core change, out of scope here) or a
   policy that drops the cursor first. — inferred
2. With several active outputs the shared overlays appear once per output in the engine's list
   but exist once in hardware. `PlaneCaps::outputs` can say "usable on both"; a frame pair that
   uses the same overlay twice has to be rejected, by `TEST_ONLY` at the latest. — inferred

## 4. Resource groups and what a profile could hold

`ResourceGroup` charges one unit per consuming layer for a set of planes (**repo**). The HVS
limits are of four kinds. None was exercised on a board near its limit.

| Limit | Fits a group? | Design | Tag |
| --- | --- | --- | --- |
| Display-list words: 4096 minus 32 reserved = 4064, one memory for all channels; about 7 to 20 words per plane, minimum 28 bytes | no | A frame of at most 16 layers is about 16 x 20 + 1 words, well under 4064, so the dlist is unlikely to bind one output's frame. It can bind when other CRTCs and stale entries hold space. Leave to `TEST_ONLY` | source; bound inferred |
| LBM, HVS5: 64 handles, 60 K words, vertical scaling only | partly | A group with consumer `SCALED_V` and capacity at most 64 is sound for handles, not for words. An early refusal only | source; inferred |
| LBM, HVS6: 1024 lines, for vertical scaling or YUV scaling. UPM: 32 handles, one per colour plane | partly | An NV12 layer needs two UPM handles but a group charges one unit per layer, so it undercounts. Use half the handles as capacity, or leave to `TEST_ONLY` | source; inferred |
| Memory bandwidth, HVS clock | no | No software check at commit time on either generation (the load tracker is gen 4 only; gen 6 never applies the computed clock). An overloaded frame fails by underrun, not in `TEST_ONLY` | source; inferred |
| HVS channels: 3, pinned to pixelvalves | not a plane limit | HVS5: pv0 channel 0 only, VEC channel 1 only, TXP channel 2 only, others any. HVS6: HDMI0 channel 0, HDMI1 channel 1, `moplet` channel 1, `mop` channel 2, so HDMI1 and `moplet` exclude each other. It decides which outputs can run together; the backend should report only outputs that can run and let `TEST_ONLY` decide | source; inferred |

A profile for the HVS, if the board sessions call for one, would be **data plus a plane and
format selection policy**, because KMS already carries zpos, alpha, blend and `possible_crtcs`:

| Field | HVS5 (BCM2711) | HVS6 (BCM2712) |
| --- | --- | --- |
| Variant name | explicit, e.g. `Vc4Hvs5` | explicit, e.g. `Vc4Hvs6` |
| `scaling` on non-cursor planes | `Some(conservative)` | same |
| `max_source` | width 8191 | none |
| Groups | none, or an LBM `SCALED_V` group | LBM/UPM group, optional |
| Format priority within 32 | linear first, then the pairs of §5 | linear first, then SAND128 and SAND256 |

All of this is inferred. `DriverProfile::Imx95Dpu` is the shape to copy (a function that edits
`Capabilities`), but its resource is a count of fetch units, while the HVS has many planes limited
by shared memories. The observations so far (two-plane runs on the generic profile passing on both
boards) do **not** by themselves call for a profile. They call for the enumeration fix (§3.1) and
for programming `rotation`. — inferred

## 5. Modifiers and formats (SAND and others)

| Item | BCM2711 | BCM2712 |
| --- | --- | --- |
| Formats advertised | The engine saw 32; the kernel list was not dumped, since no raw property dump exists for this board (observed 2026-11-28). Source: a common list plus P030 and 10-bit RGB | 45 linear formats plus formats under Broadcom modifiers, read from `IN_FORMATS` on five planes (observed 2026-11-28). The engine kept 32, linear first; cut: XB12 AB12 BX12 BA12 RX12 RA12 P010 P012 P016 S010 S012 S016, and P030 under a Broadcom modifier (observed 2026-11-28) |
| Modifiers advertised | Source: linear, `VC4_T_TILED`, SAND64, SAND128, SAND256. Not observed | `0x0` and Broadcom codes 1, 3, 4, 5 (observed 2026-11-28) |
| Driver rule per format | RGB formats (XRGB/ARGB/ABGR/XBGR8888, RGB565, BGR565, ARGB1555, XRGB1555): linear or T-tiled. NV12/NV21: linear, SAND64/128/256. P030: SAND128 only. All others: linear only (source) | The driver rejects T-tiled and SAND64 (`vc4_format_mod_supported`; `vc6_plane_mode_set` returns `-EINVAL`). Usable: linear, SAND128, SAND256 (source) |
| What the engine sees | No modifier (probe prints fourccs only). All buffers used were linear, `modifier=0x0` in debugfs (observed 2026-11-28) | Same: 32 linear pairs, no modifier; all buffers linear (observed 2026-11-28) |

On the Pi 5 the observed blob lists code 1 for the RGB formats (XR24 AR24 AB24 XB24 RG16 BG16
AR15 XR15), code 3 for NV12/NV21, code 4 for NV12/NV21/P030 and code 5 for NV12/NV21
(observed 2026-11-28). That pattern is the one the source gives for the *HVS5* rule above (RGB
with T-tiled, NV12/NV21 with three SAND variants, P030 with one). From recollection of
`drm_fourcc.h` the numbers are T-tiled = 1, SAND32 = 2, SAND64 = 3, SAND128 = 4, SAND256 = 5; the
observed file read code 3 as SAND128 "from memory". This note uses the first reading, which makes
code 4 the P030 one (SAND128) and matches the source. **Neither numbering was checked against a
header in this work.** — inferred, unverified

If that reading is right, the BCM2712 blob shows T-tiled and SAND64 pairs although the source says
the gen 6 driver rejects them. That is a **disagreement between the observed blob and the source**
(a different kernel build, or `IN_FORMATS` built from a wider list than `format_mod_supported`
allows). It is not resolved here (question 3). — inferred

Design consequences, all inferred:

- A backend that wants SAND video must keep `NV12`/`NV21` with SAND128 or SAND256 pairs inside the
  32-pair limit, ahead of the 13 linear formats it would otherwise keep. Today the cut keeps
  linear first and no modifier reaches the engine; a profile's priority list is where this goes.
- Buffer import must pass the modifier explicitly. The harness used linear dumb buffers, so no SAND
  or T-tiled buffer was ever committed. SAND modifiers carry a column-height parameter; the source
  advertises `COL_HEIGHT(0)`. Whether a decoder's SAND buffers match was not examined. — source
- T-tiled RGB is an HVS5-only path (source). For a portable producer, linear is the only layout
  that both generations are read to accept and that was seen latching (observed 2026-11-28).

## 6. Alpha, blend, scaling, rotation, z-order

- **Z-order.** Pi 5, observed 2026-11-28 with the KVM: the overlay hides the green block below it
  and blends over the background. CM4, observed 2026-11-28: no capture; only `normalized-zpos` 0
  and 1 and the dlist (overlay entry after the background entry, channel 0) were read. That the
  HVS composes lowest zpos first is source; z-order on the CM4 panel is inferred from the two.
- **Alpha and blend.** Pi 5: premultiplied with plane alpha 0xffff, and coverage with plane alpha
  0x8000, matched the expected blend within the capture's colour error (up to 34 per channel on
  unblended points, up to 13 on blended ones, which the file attributes to the capture chain) —
  observed 2026-11-28. CM4: the same configurations latched and passed `TEST_ONLY`; the output was
  not seen — observed 2026-11-28. Source: plane alpha acts only when the format has alpha and the
  blend mode is not none; a non-opaque plane needs the background fill.
- **Scaling.** Both boards accepted 2x up and 1/2 down in `TEST_ONLY` with a hand-built plan
  (observed 2026-11-28). Source: PPF for up and for down to 1.5x, TPZ beyond that, no limit from
  the DRM helper, a 21-bit ratio field in the driver, vertical decimation above 3x on 6.18. The
  real ceiling is LBM and dlist space. No scaled frame was committed or looked at. — source
- **Rotation.** 0, 180, reflect-x, reflect-y, no 90 or 270: observed 2026-11-28 (BCM2712 mask),
  source (both generations). Where the flip bits live differs on gen 6 (source), which matters to
  the kernel only. The engine has to program the property; today it refuses rotate-180 before the
  kernel is asked (observed 2026-11-28).
- **Z-swap.** `--swap-z` could not run on either board: the planner puts the background on the
  immutable primary, and a swap needs two overlays (observed 2026-11-28). Whether a CRTC accepts
  two overlays with a disabled primary is not known (question 5).

## 7. Differences between the two generations

| Item | BCM2711, HVS5 (`VC4_GEN_5`) | BCM2712, HVS6 (`VC4_GEN_6_C`/`_D`) | Tag |
| --- | --- | --- | --- |
| Display CRTCs | 5 pixelvalves | 2 pixelvalves, 2 writeback blocks | source |
| CRTCs observed | 6 | 4 | observed 2026-11-28 |
| Channel pinning | pv0 ch 0, VEC ch 1, TXP ch 2, rest any | HDMI0 ch 0, HDMI1 ch 1, `moplet` ch 1, `mop` ch 2 | source |
| Modifiers usable | linear, T-tiled, SAND64/128/256 | linear, SAND128, SAND256 | source (blob: §5) |
| Formats | common + P030 + 10-bit RGB | the HVS5 list + P010, P012, P016, S010, S012, S016 | source |
| Source width | rejected above 8191 | no check | source |
| LBM | 60 K words, 64 handles; vertical scaling only | 1024 lines; vertical or YUV scaling | source |
| UPM | none | 32 handles, one per colour plane | source |
| Addressing | 32-bit | 40-bit (two pointer words) | source |
| Dlist memory | 4064 words | allocator range 4064 words although the size is read from `SCALER6_CXM_SIZE` | source; "quirk" inferred |
| Core clock | computed and applied | computed, never applied | source |
| Bandwidth check | none (load tracker is gen 4 only) | none | source |
| Mode size limit | 7680 x 7680 | 8192 x 8192 | source |
| COB line width | 4096 | 3840 | source |
| Stepping | n/a | C0 and D0 differ in alpha-mask encoding; same toward userspace | source; "same" inferred |
| Async update | few changes only (position, pointer, LBM) | same, with the 40-bit pointer rewrite | source |
| Plane structure, zpos, alpha, blend, rotation set | as in §2 | as in §2 | observed 2026-11-28 |
| Driver name | `vc4` | `vc4` | observed 2026-11-28 |

Because the driver name is `vc4` on both, the profile has to be chosen explicitly through
`DrmOptions::profile` (**repo**) or by a detection rule. A rule from the modifier set is weak given
the §5 disagreement. The compatible string of the GPU node (`brcm,bcm2711-vc5` against
`brcm,bcm2712-vc6`, source; the Pi 5 read `brcm,bcm2712-vc6`, observed 2026-11-28) would be a
stronger key, but the backend does not read it today. — inferred

## 8. Unsupported combinations: which layer says no

From the harness `--unsupported` runs on both boards (observed 2026-11-28):

| Combination | Capability detection | Planner | `TEST_ONLY` |
| --- | --- | --- | --- |
| unknown format `HWL0`; modifier `I915_X_TILED` | refuses | `NoPlane` (FORMAT) | not reached (dumb buffers cannot hold them) |
| more layers than planes (engine view: 13 on the Pi 5, 11 on the CM4) | refuses | `NoPlane` (IN_USE) | not reached |
| scaling up and down | refuses (none claimed) | `NoPlane` (SCALING) | **ok**, the kernel accepts both |
| rotate-180 | refuses (`transforms 0x0`) | `NoPlane` (TRANSFORM) | not reached (rotation not programmed) |
| plane alpha, coverage blend, off-screen, odd geometry | accepts | plan | ok |

No combination was refused by the kernel alone: every refusal came from the engine's own claims,
which are stricter than the hardware for scaling, rotation and plane count. The layer counts come
from the truncated view; the kernel offers 18 (Pi 5) and 17 or more (CM4) planes to the active
output, so those frames might fit in hardware, which was not tried. — observed 2026-11-28 (lines);
hardware reading inferred

A refusal that only `TEST_ONLY` can give (dlist, LBM or UPM exhaustion; a modifier pair the driver
rejects; two outputs using one overlay) was not run. Bandwidth overload cannot be given by
`TEST_ONLY` at all (source).

## 9. Out of scope

HVS4 (BCM2835 to 2837, `VC4_GEN_4`, the only generation with a load tracker), firmware KMS
(`vc4-fkms-v3d`), the writeback connectors as a frame source, and the Pi 5 DSI/DPI connectors on
RP1 (the 2712 source tables list no DSI, DPI or VEC encoder; the rest was not read). The `v3d`
render node on both boards is not a display path. — source; observed 2026-11-28 (nodes)

## 10. Tests that need no board

- `Capabilities` fixtures built from the recorded probe blocks (Pi 5 in `bcm2712-topology.md`,
  DSI-1 in `bcm2711-cm4-topology.md`), plus a fixture of the pre-cut 56 and 60 planes to test the
  enumeration fix. — inferred
- If a profile is added: scaling and `max_source` set on non-cursor planes only, and idempotent
  `apply`, as the DPU profile's test does. — inferred
- Planner cases per generation: N overlays up to 16 layers; an NV12 SAND pair; T-tiled XRGB
  accepted on HVS5 and refused on HVS6; a layer needing zpos below the primary refused. These test
  the engine's table, not the hardware. — inferred
- Not host-testable: underrun, bandwidth, what `TEST_ONLY` accepts. — source

## 11. Open questions for board sessions

1. **Scaling limits.** `TEST_ONLY` with growing ratios up and down per generation, to find where
   LBM, TPZ or the ratio field refuse; then commit a scaled frame and look at it (the Pi 5 KVM
   works; the CM4 has no capture).
2. **Bandwidth and underrun.** Load the HVS with many large and scaled layers and read
   `hvs_underrun`; find where frames drop while `TEST_ONLY` still passes. No threshold is known;
   both boards read 0 under two planes only.
3. **Modifier set per generation.** Dump the raw `IN_FORMATS` on the CM4 (not done), check the
   modifier numbering against `drm_fourcc.h`, `TEST_ONLY` a T-tiled and a SAND64 buffer on the Pi 5
   to settle the blob-versus-source disagreement (§5), then commit a real SAND NV12 buffer on both.
4. **Plane count after the enumeration fix.** Re-run probe on both boards with the fix; record how
   many planes each output gets; decide cursor handling and whether `MAX_PLANES` 16 is enough.
5. **Two overlays, primary off.** `TEST_ONLY` a CRTC with its primary disabled and two overlays
   with swapped zpos (the Pi 5 harness note's proposal), which would let `--swap-z` run.
6. **`replace_pending`.** Replace a not-yet-latched commit on one channel and see what the kernel
   does; the source suggests commits on a channel are serialised.
7. **Two active outputs.** Neither board had a second output active. Drive two (CM4 HDMI, Pi 5
   HDMI-A-2) and check overlay sharing, channel pinning (Pi 5: HDMI1 against `moplet`) and the
   dlist budget.
8. **Pi 5 plane arithmetic.** The recorded 4 primaries, 34, 16 and 3 add up to 57, not 56; read the
   device tree and `state` side by side and type the extra planes (cursor or not).
9. **Rotation.** Program rotate-180 and reflect on both generations once composition-drm does, and
   look at the result.
10. **Which kernel.** Source rows are `rpi-6.18.y` at one commit; the CM4 runs 6.12.47 and the Pi 5
    6.18.50. Re-check plane, modifier and LBM details against the kernels that run.
11. **CM4 display check.** The panel has no capture; z-order and alpha there stay unverified until
    something looks at the glass.
12. **Detection key.** Whether an HVS profile is keyed on the GPU compatible string, the modifier
    set, or an explicit option.

No board was used and nothing was measured for this note.
