# Design: an engine backend for the Raspberry Pi HVS (BCM2711, BCM2712)

Status: design only, written 2026-11-28; the §7 status notes added 2026-11-29 only point at dated
session files and restate nothing as new. **No hardware result is claimed and no code is changed.**
Nothing in this file was run.

## Tags

The tags of the source documents are kept on every row that is taken from them:

- **observed** (with its date; 2026-11-27 and 2026-11-28 on the files below): measured on a board,
  as recorded in
  [`observed/bcm2711-cm4-topology.md`](observed/bcm2711-cm4-topology.md),
  [`observed/bcm2711-cm4.md`](observed/bcm2711-cm4.md),
  [`observed/bcm2712-topology.md`](observed/bcm2712-topology.md) and
  [`observed/bcm2712.md`](observed/bcm2712.md).
- **source**: read from the Raspberry Pi kernel driver, as recorded in
  [`observed/vc4-hvs-source.md`](observed/vc4-hvs-source.md) (tree `raspberrypi/linux`,
  branch `rpi-6.18.y`, commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e`, read 2026-11-28). This
  file did not re-read that source.
- **inferred**: a conclusion of this design. **Everything that is new here is inferred**, whether
  or not it is written with that word. A row that has no tag is a statement about the engine's
  own contract, read from the crates named next to it on 2026-11-28 (also inferred as far as it
  concerns the HVS).

Where the plan text points at `docs/platform-capabilities.md` for the BCM2711 and BCM2712
topology, that file in this snapshot has no such section; the topology is in the
`docs/observed/` files above, which is where this design reads it. See also
[`platform-capabilities.md`](platform-capabilities.md) for the tag definitions.

## 1. What the HVS is, in the terms the engine uses

The engine plans against a plane count and a few shared resources
(`crates/composition-core/src/caps.rs`: `PlaneCaps`, `ResourceGroup`, `Capabilities`). The HVS is
not shaped like that:

| Fact | Tag |
| --- | --- |
| The HVS composes from a **display list** (dlist) per output channel. A plane is a few words (about 7 to 20 per plane on HVS5; the source comment gives 28 bytes as the minimum) in a dlist, not a fixed pipeline stage | source (minimum); range inferred |
| One dlist memory serves all channels: 4064 words allocator range (4096 minus 32 reserved for the bootloader) on both generations, even though HVS6 reads its size from `SCALER6_CXM_SIZE`. A commit allocates the sum of its planes' words plus one end word per CRTC; space is freed lazily | source |
| The kernel creates a fixed *number* of planes per CRTC (1 primary, 1 cursor, and 16 overlays shared by all non-writeback CRTCs; 32 overlays for the writeback CRTC), but the source calls the counts arbitrary: the real limit is dlist space and a plane's minimum size | source |
| Shared units besides the dlist: line buffer memory (LBM) for planes that scale vertically (HVS5) or scale or are YUV (HVS6, see §4), and the UPM on HVS6 (one handle per colour plane) | source |
| No software bandwidth check applies at commit time on gen 5 and gen 6 (the load tracker is gen 4 only), and HVS6 never applies the computed core-clock rate. A commit the memory bus cannot sustain fails by underrun, not in `TEST_ONLY` | source |
| Observed BCM2712 (Pi 5, 2026-11-28): 56 planes, of which 16 shared overlays reach CRTCs 1 to 3; `hvs_underrun` debugfs read 0 | observed 2026-11-28 |
| Observed BCM2711 (CM4, 2026-11-28): 60 planes in the kernel's atomic state; the engine's probe reports 16 | observed 2026-11-28 |

So the HVS is "many planes, limited by dlist words, LBM/UPM units and memory bandwidth", which is
the opposite of the i.MX95 DPU (six fetch units, a few of which can scale or read YUV;
`DriverProfile::Imx95Dpu` in `crates/composition-drm/src/profile.rs`) that the existing profile
models. — inferred

## 2. The backend contract this design must fit

Read from `crates/composition-core/src/backend.rs`, `caps.rs`, `limits.rs` and
`crates/composition-drm/src/lib.rs` (read only; nothing here changes them). — inferred as far as
the HVS is concerned.

- `DisplayBackend` has four methods: `capabilities(&mut Capabilities)`, `check(frame, plan)`,
  `commit(submission, frame, plan)` and `poll_event()`. The backend never plans. It reports
  `Latched { output, submission }` when the hardware latched.
- `Capabilities` holds at most `MAX_OUTPUTS` = 4 outputs, `MAX_PLANES` = 16 planes,
  `MAX_RESOURCE_GROUPS` = 8 groups; `PlaneCaps` holds at most `MAX_FORMATS` = 32 format/modifier
  pairs. `MAX_LAYERS` = 16.
- A `ResourceGroup` is `{ consumers: LayerTraits, planes: u32 mask, capacity: u8 }`. A layer on a
  member plane takes **one unit** if it has any of the `consumers` traits (`ANY`, `SCALED_H`,
  `SCALED_V`, `YUV`). Groups describe one output's frame only.
- `DrmBackend` discovers planes and formats from KMS, keeps the first `MAX_PLANES` planes sorted
  by kind (open issue X-09 in [`open-issues.md`](open-issues.md)), and `DriverProfile::apply`
  adds what KMS does not carry: scaling limits, `max_source`, resource groups. A profile is
  chosen explicitly through `DrmOptions::profile`; `check` is an atomic `TEST_ONLY` commit.
- `driver_name()` is what the kernel reports. Both HVS generations report `vc4`
  (observed 2026-11-28 on BCM2712: driver name `vc4`; BCM2711 uses the same driver), so the driver
  name alone does not tell HVS5 from HVS6. — inferred

## 3. Mapping to the engine's plane-assignment constraints

| HVS property | Engine field | Design | Tag |
| --- | --- | --- | --- |
| Primary plane: immutable zpos 0 | `PlaneKind::Primary`, `Zpos::Fixed(0)` | Already what KMS reports (probe output: "zpos fixed 0"). No table entry needed | observed 2026-11-28 (probe); mapping inferred |
| Overlay: zpos 1..17 mutable, initial `i + 1` | `Zpos::Range { 1, 17 }` | From KMS. The dlist order is by normalised zpos, so zpos is the blend order | source; observed 2026-11-28 |
| Cursor: zpos range 1..17, initial 17 | `PlaneKind::Cursor` | On BCM2712 the extra per-CRTC plane was not typed in the probe (planes 646, 657, 668 "inferred to be the cursor planes"). The table must not rely on it | source; observed 2026-11-28 (not typed) |
| Overlay pool shared by CRTCs (one allocation, mask over all non-writeback CRTCs) | `PlaneCaps::outputs` mask per plane | The mask already says every overlay can go to every output. The pool is **not** a per-output count. See §4 for what limits it | source; mapping inferred |
| 3 HVS channels, with pv-to-channel pinning (HVS5: pv0 ch 0 only, VEC ch 1 only, TXP ch 2 only; HVS6: HDMI0 ch 0, HDMI1 ch 1, `moplet` also ch 1) | `OutputCaps` | Not expressible as a plane constraint. It decides which *outputs* can be active together (HDMI1 and `moplet` exclude each other on BCM2712). The engine sees only the outputs the backend reports, so the backend must report only outputs that can run, and `check` (`TEST_ONLY`) must have the last word | source; design inferred |
| Plane alpha, blend modes none / pre-multiplied / coverage | `plane_alpha`, `blend_modes` | From KMS; identical on every plane (observed 2026-11-28, both SoCs) | observed 2026-11-28 |
| Rotation 0/180, reflect X/Y, no 90/270 | `transforms` | From KMS. The mask must exclude 90/270 | observed 2026-11-28 (BCM2712); source |
| Scaling | `scaling: Option<ScaleLimits>` | KMS has no scaling property (observed 2026-11-28, "probe prints scaling not claimed"). The table sets `Some(..)` on non-cursor planes. See §5 for the values and what is unknown | observed 2026-11-28; design inferred |
| Source size limit | `max_source` | HVS5: width 8191 (`SCALER5_POS2_WIDTH_MASK`); HVS6: the source has no such check. Mode size limits are 7680 x 7680 (gen 5) and 8192 x 8192 (gen 6) | source |
| Formats and modifiers | `formats` (max 32) | See §5 on the 32-pair cap | observed 2026-11-28; source |
| Latch | `BackendEvent::Latched` | DRM page-flip events, as `DrmBackend` already does. HVS needs nothing special for the event itself | inferred |
| `replace_pending` | `Capabilities::replace_pending` | The source says each in-use channel waits for the previous commit on that channel before a new commit (`pending_commit`), and async plane update is only allowed for a small set of changes. Whether a pending frame can be replaced is therefore unknown here | source; conclusion inferred (open question 6) |

### Resource groups the HVS would need

`ResourceGroup` counts units one per consuming layer. The HVS limits are of three kinds, and only
some fit:

| Limit | Fits a `ResourceGroup`? | Design | Tag |
| --- | --- | --- | --- |
| Dlist words (4064, shared by all CRTCs) | Not as units | With at most `MAX_LAYERS` = 16 layers on one output and about 7 to 20 words per layer, one frame needs at most about 16 x 20 + 1 = 321 words, under 8 % of 4064. The dlist is therefore unlikely to bind a frame the engine can plan. It can still bind when other CRTCs and stale entries hold space. **No group; leave to `check`** | inferred |
| LBM, HVS5: 64 handles, 60 K words | Partly | A conservative group with `SCALED_V` as the consumer and a capacity below 64 would be sound for the handle count but not for the words (size depends on the width and on RGB vs YUV). A group is a *cheap early refusal*; `check` stays authoritative | source; design inferred |
| LBM and UPM, HVS6: LBM 1024 lines; UPM 32 handles, one per colour plane | Partly | An NV12 layer needs two UPM handles but a group charges one unit per layer, so the group undercounts. Either set the capacity to half of 32 (conservative for two-plane YUV, wasteful for packed) or leave it to `check` | source; design inferred |
| Memory bandwidth and HVS clock | No | Not visible to `TEST_ONLY` on gen 5 and gen 6 and not a countable unit. Needs a different mechanism (open question 2) | source; inferred |

## 4. What a constraint table would hold

The engine's `Imx95Dpu` profile is a function that edits `Capabilities`. An HVS profile would be
the same shape, with data per generation. A sketch of the fields (names are proposals, not an API):
— inferred

| Field | HVS5 (BCM2711) | HVS6 (BCM2712) | Basis |
| --- | --- | --- | --- |
| Generation selector | explicit `DrmOptions::profile` variant (e.g. `Vc4Hvs5`) | explicit variant (`Vc4Hvs6`) | both report driver `vc4`; D0 vs C0 stepping is not visible and is said to behave the same toward userspace (source) |
| Planes to keep (the `MAX_PLANES` cut) | the primary and cursor of every reachable CRTC first, then overlays, then writeback planes last | same | observed 2026-11-28 (probe drops 40 of 56 planes on BCM2712, and ranks 5 of 16 slots to unconnected primaries on BCM2711, X-09) |
| Scaling on non-cursor planes | `Some(upscale = ?, downscale = ?)` | same | §5 |
| `max_source` | width 8191, height from the mode limit | none from the source | source |
| Format/modifier priority list (max 32) | linear first; T-tiled for XRGB/ARGB/ABGR/XBGR/RGB565/BGR565/ARGB1555/XRGB1555; SAND64/128/256 for NV12/NV21; SAND128 for P030 | linear; SAND128 and SAND256 only (T-tiled and SAND64 are rejected by the driver on gen 6) | source |
| Groups | LBM (`SCALED_V`) and optionally nothing else | LBM and UPM groups, possibly one with `SCALED.union(YUV)` | §3, source |
| Max active outputs | 3 (HVS channels) | 3 (HVS channels), with HDMI1/`moplet` exclusive | source |
| Per-plane capability source | KMS | KMS | observed 2026-11-28 |

The table is **data plus a small amount of selection policy**, because KMS already supplies the
rest (formats, zpos, alpha, blend, rotation, `possible_crtcs`). The policy part (which 16 planes
to keep, which 32 format pairs to keep) matters more here than on the DPU, since the engine's
limits are far below the HVS's 56 to 60 planes and 45 formats.

## 5. Differences between the vc4 HVS4/5 and HVS6 variants, as recorded

HVS4 (BCM2835 to 2837) is out of scope in the source study read 2026-11-28,
[`observed/vc4-hvs-source.md`](observed/vc4-hvs-source.md).

| Item | BCM2711, HVS5 | BCM2712, HVS6 | Tag |
| --- | --- | --- | --- |
| Driver generation | `VC4_GEN_5` | `VC4_GEN_6_C` / `VC4_GEN_6_D` (D0 when `SCALER6_VERSION == 0x54`) | source |
| Display CRTCs | 5 pixelvalves, 3 HVS channels | 2 display pixelvalves, 3 HVS channels, 2 writeback blocks (`mop`, `moplet`) | source |
| Observed CRTC count | 6 CRTCs (`txp` plus 5 pixelvalves) on the CM4 | 4 CRTCs on the Pi 5. The source lists 2 pixelvalves plus 2 writeback blocks, which would add up to 4 if both writeback blocks are CRTCs | observed 2026-11-28; reconciliation inferred |
| Observed plane count | 60 | 56. The split 34 + 16 + 3 x 2 = 56 fits one writeback CRTC with 1 primary, 1 cursor and 32 overlays (34), 16 shared overlays, and three other CRTCs with a primary and an extra plane each. The BCM2712 file counts 4 primaries and 34 CRTC-0-only planes, which would give 57; the two readings differ by one plane | observed 2026-11-28; arithmetic inferred |
| Modifiers usable | LINEAR, `VC4_T_TILED`, SAND64, SAND128, SAND256 | LINEAR, SAND128, SAND256. The `IN_FORMATS` blob observed on the Pi 5 (2026-11-28) lists the Broadcom codes 1, 3, 4 and 5 (the file reads them as T-tiled, SAND128 and others); the source says the driver rejects T-tiled and SAND64 per `vc4_format_mod_supported`. The blob and the source need a side-by-side check | source; observed 2026-11-28 (blob); disagreement inferred |
| Formats | common list plus P030 and 10-bit RGB | the BCM2711 list plus P010, P012, P016, S010, S012, S016 | source |
| Source width | rejects above 8191 | no check in the source | source |
| LBM | 60 K words, 64 handles, vertical scaling only | 1024 lines; reserved for vertical scaling *or* YUV scaling (`!is_unity \|\| is_yuv`) | source |
| UPM | none | 32 handles, one per colour plane | source |
| Addressing | 32-bit | 40-bit (two pointer words) | source |
| Bandwidth and clock | core-clock rate computed and applied; no load-tracker check | rate computed, never applied; no load-tracker check | source |
| `vc4_hdmi.c` limits | HDMI 2.0 above 550 MHz core clock | 600 MHz pixel clock each, **unverified** | source; inferred (unverified) |
| Mode size | 7680 x 7680 | 8192 x 8192 | source |
| COB line width | 4096 | 3840 | source |
| Stepping C0 vs D0 | n/a | alpha-mask and fixed-alpha encoding differ; same toward userspace | source; "same" inferred |

## 6. Tests on recorded topologies

All of these are host tests. None needs a board and none says anything about hardware.
— inferred

1. **Fixtures.** Build `Capabilities` from the recorded probe outputs (observed 2026-11-28): the
   raw probe block in
   [`observed/bcm2712-topology.md`](observed/bcm2712-topology.md) and the DSI-1 probe block in
   [`observed/bcm2711-cm4-topology.md`](observed/bcm2711-cm4-topology.md), and from the fuller
   plane lists those files hold. The fixtures are data copied from the files, with the date.
2. **Profile application.** Apply the proposed HVS5 and HVS6 tables to those fixtures with the
   fake backend (`crates/composition-fake`) and check: groups are present once and not
   duplicated by a second `apply` (as the DPU profile's test does), scaling and `max_source` are
   set on non-cursor planes only, and a cursor is not given a scaler.
3. **Plane selection.** The pre-cut plane list (56 and 60 planes, as a fixture, since the
   engine's own limit is 16) must give the connected output its primary, enough overlays and a
   cursor, and put unconnected primaries last (X-09). This test belongs with the code change that
   X-09 proposes.
4. **Planner cases per generation:** a frame of N overlays up to `MAX_LAYERS`; a vertically scaled
   layer on every overlay (the LBM group refuses beyond its capacity; the engine reports why);
   an NV12 layer with a SAND modifier on HVS5 (accepted by the table) and the same on HVS6
   (SAND64 refused, SAND128 accepted); a T-tiled XRGB layer on HVS5 (accepted) and HVS6 (refused);
   a layer whose zpos would sit below the primary (refused: primary zpos is fixed 0).
5. **Cross-check against libliftoff.** The method in
   [`libliftoff-crosscheck.md`](libliftoff-crosscheck.md) applies to these fixtures unchanged.
6. **Not testable on the host:** underrun and bandwidth. The recorded
   `TEST_ONLY` results in [`observed/bcm2711-cm4.md`](observed/bcm2711-cm4.md) and
   [`observed/bcm2712.md`](observed/bcm2712.md) are the only evidence of what the kernel
   accepts, and they cover two-plane use only. A board run per generation is needed before any
   table value is called measured.

## 7. Open questions

1. **Scaling values.** *Status (2026-11-29): measured on both generations; the downscale floor
   below 0.5 stays open.* Measured 2026-11-27 and 2026-11-28 on the Pi 5 and the reTerminal
   ([`vc4-hvs-session-rpi5-scale-2026-11-28.md`](vc4-hvs-session-rpi5-scale-2026-11-28.md),
   [`vc4-hvs-session-cm4-scale-2026-11-28.md`](vc4-hvs-session-cm4-scale-2026-11-28.md)): no
   upscale ratio was rejected on any primary or overlay plane up to the ladder's end (a one-pixel
   source), and 0.5 downscale was accepted everywhere; the ladder does not go below 0.5, so the
   floor is unmeasured. The table in `crates/composition-drm/src/profile/vc4.rs` (read 2026-11-29)
   claims 32x up and 2x down, inside the measured range and the source prediction
   ([`vc4-scale-limits-source.md`](vc4-scale-limits-source.md)). The driver imposes no minimum or
   maximum through the DRM helper (min 1, max `INT_MAX`), and its own limit is a TPZ ratio field
   of 21 bits whose fixed-point meaning was not worked out. (source; inferred)
2. **Bandwidth.** The engine cannot see a limit that neither `TEST_ONLY` nor the driver
   enforces. Options: a backend-side estimate in `check` using the source's own load formula
   (`vc4_plane_calc_load`) against a threshold measured on a board; or document the limit as
   unsupported. Needs a measurement; no threshold is known. (inferred) *Status (2026-11-29):
   still open — no session measured one, and the recorded `hvs_underrun` reads of 0 (2026-11-28)
   say nothing about a threshold.*
3. **Generation detection.** Both report `vc4`. Is an explicit profile variant enough, or should
   the backend infer HVS5 vs HVS6 from the modifier set (T-tiled present or not)? The explicit
   variant matches how profiles are chosen today. (inferred) *Status (2026-11-29): settled in
   code, not measured — the profiles are the explicit `Vc4Hvs5` and `Vc4Hvs6` variants
   (`crates/composition-drm/src/profile.rs`, read 2026-11-29); modifier-based detection is not
   implemented.*
4. **Plane budget.** `MAX_PLANES` = 16 and `MAX_FORMATS` = 32 drop planes and formats the
   HVS has (observed 2026-11-28: 40 of 56 planes, 13 of 45 linear formats). Is that acceptable
   as a conscious scope limit, or should a later change raise the limits? That is a core change
   and outside this design. *Status (2026-11-29): recorded, decision with composition-core —
   [`rk-hvs-caps-finding.md`](rk-hvs-caps-finding.md) is the decision brief; nothing was raised
   here.*
5. **BCM2712 CRTC and plane arithmetic.** *Status (2026-11-29): answered on the Pi 5
   (observed 2026-11-28, [`vc4-hvs-session-rpi5-scale-2026-11-28.md`](vc4-hvs-session-rpi5-scale-2026-11-28.md),
   section Q5): the CRTCs are the two writeback blocks (59 `mop`, 78 `moplet`) and the two HDMI
   pixelvalves (94 HDMI-A-1, 106 HDMI-A-2), and the 56 planes are 4 primaries, 16 overlays shared
   by CRTCs 1 to 3, 32 overlays bound to CRTC 0 and one cursor per CRTC — the extra per-CRTC
   planes are **not** cursors, and the 57-vs-56 doubt resolves to 4 + 16 + 32 + 4 = 56.* The
   CRTC roles came out of KMS itself (encoder `possible_crtcs`, connector names), not the device
   tree, and the cursor question is closed. (observed 2026-11-28; inferred)
6. **`replace_pending`.** *Status (2026-11-29): answered on both generations (observed 2026-11-27,
   one `kms_sweep replace` run each, recorded as `sweep-replace-2026-11-27.txt` under
   `crates/composition-drm/tests/data/bcm2711/` and `.../bcm2712/`): the second commit before the
   first flip event is refused with `EBUSY` and the first flip completes
   (`REPLACE first=ok second=EBUSY events=1`); a not-yet-latched commit cannot be replaced, and
   the kernel reports `EBUSY`, as the source predicted
   ([`vc4-replace-pending-source.md`](vc4-replace-pending-source.md)). The run used the primary
   flip path; an overlay-pending replacement was not separately tested.* The commit-ordering row
   in the source suggested `false`; the board confirms it. (source; observed 2026-11-27)
7. **Which kernel.** *Status (2026-11-29): answered, with a negative result — the row-by-row
   comparison against the boards' running kernels is on the tip and finds the rows do **not**
   match the running kernels where later commits touch them
   ([`vc4-rk-kernel-compare.md`](vc4-rk-kernel-compare.md)).* The source rows are for
   `rpi-6.18.y` at one commit; the boards' kernels (`6.18.50+rpt-rpi-2712` observed on the Pi 5,
   2026-11-28) are not the same build. Plane and modifier details may differ, and the comparison
   says where. (source; observed 2026-11-28)
8. **Firmware KMS** (`vc4-fkms-v3d`) is out of scope; the design assumes full KMS. *Status
   (2026-11-29): unchanged; not a measurement question.*
9. **Shared resources across outputs.** `ResourceGroup` describes one output's frame. The HVS
   pool is shared between CRTCs; a second active output can exhaust it unseen by the planner.
   `check` must have the last word, as the contract says. (inferred) *Status (2026-11-29):
   still open — the two-output Pi 5 session could not run: HDMI-A-2 has no sink
   (observed 2026-11-28,
   [`observed/bcm2712-two-outputs-2026-11-28.md`](observed/bcm2712-two-outputs-2026-11-28.md)).*
10. **HDMI limits** on BCM2712 (600 MHz) are unverified in the source record. *Status
    (2026-11-29): still open — the attached sink's EDID caps at 1920x1080@60 (148.5 MHz), so no
    4K or 600 MHz limit could be shown with it (observed 2026-11-28,
    [`vc4-hvs-session-rpi5-scale-2026-11-28.md`](vc4-hvs-session-rpi5-scale-2026-11-28.md),
    section Q10); the writeback's 4096x2160 bound is the writeback, not HDMI (observed
    2026-11-28, [`observed/bcm2712-two-outputs-2026-11-28.md`](observed/bcm2712-two-outputs-2026-11-28.md)).
    The missing measurement is a `mode_test` run (read from source,
    `crates/composition-drm/examples/mode_test.rs`) with a 4K-capable sink.*

No board was used for the design text above. The 2026-11-29 status notes in §7 only point at
dated board sessions recorded in the linked files; nothing was re-measured here.
