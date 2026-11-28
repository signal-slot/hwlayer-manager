# RK3588 (VOP2) backend design

Design note, written 2026-11-28. **No hardware result is claimed and no code
was changed.** Tags follow `docs/platform-capabilities.md`:

- **observed**, **read from source**: carried over unchanged from the source
  documents, with their dates and pins.
- **inferred**: everything new in this note. Every conclusion, proposal and
  open question below is inferred unless a row carries a source tag, and a
  source-tagged row is a quotation of the document named.

Sources used:

- [`docs/observed/rk3588-topology.md`](observed/rk3588-topology.md): one
  session on a NanoPC-T6, Armbian `6.18.55-current-rockchip64`, observed
  2026-11-28. No frame was submitted.
- [`docs/observed/rk3588-source.md`](observed/rk3588-source.md): vendor
  `rockchip-linux/kernel` `develop-6.1` @
  `77168c8d5ab82399f65a80e9f807b50ba37cf483`, read 2026-11-28. Mainline was
  **not** read.
- The engine contract: `crates/composition-core/src/{backend,caps,planner}.rs`
  and `crates/composition-drm/src/{lib,profile}.rs` at this repository's
  current tip, read 2026-11-28 (code as written, not run).

## 1. What the engine asks of a backend

Read from source (this repository):

- `DisplayBackend` has four methods: `capabilities` (fill outputs, planes,
  resource groups), `check` (final validation, `TEST_ONLY` on Linux),
  `commit`, `poll_event` (latched events).
- A plane is described by `PlaneCaps`: kind, an `outputs` mask, a list of
  (format, modifier) pairs matched **exactly** by the planner, optional
  `ScaleLimits`, `max_source`, plane alpha, blend modes, a `transforms` mask,
  `Zpos` (fixed or range) and `full_output_only`.
- `ResourceGroup` models a shared unit: member planes, which layer traits
  consume it (`ANY`, `SCALED_H/V`, `YUV`) and a capacity. It is per output
  frame; the doc comment says use of the resource by other outputs is
  invisible to the planner and the backend check has the last word.
- Limits: 4 outputs, 16 planes, 32 format pairs per plane, 8 resource groups.
- `DrmBackend::discover` (read from source) takes outputs from connected
  connectors only; the `outputs` mask of a plane covers those outputs only;
  formats come from `IN_FORMATS` with linear pairs sorted first and cut at
  32; scaling is a single global `DrmOptions::scaling` applied to every
  non-cursor plane; `transforms` is never filled, so any non-`Normal`
  transform is rejected by the planner; zpos is taken from the property when
  mutable. `DriverProfile` adds per-driver scaling and resource groups
  (only `Generic` and `Imx95Dpu` exist).

## 2. What the RK3588 recordings say

Only the facts that matter to the backend decision, with the source tag.

| Fact | Tag |
|---|---|
| 8 planes in the 6.18 session: Cluster0-3 win0 and Esmart0-3 win0; Cluster0 and Cluster1 Primary, the rest Overlay; zpos range 0..=7 on all | observed 2026-11-28, kernel 6.18.55 |
| Cluster planes list AFBC modifiers (Arm, vendor 0x08) plus YUV formats; Esmart planes list linear only (21 formats) | observed 2026-11-28 |
| Two CRTCs registered (`video_port0`, `video_port1`); one connector connected; the probe showed Cluster1 as `outputs 0b0` | observed 2026-11-28 |
| Vendor 6.1 driver: 4 video ports; Cluster sub windows (`win1`) become planes only with DT `support-multi-area` (8 planes by default, 12 with it); Esmart windows are the Primary candidates | read from source (`rockchip-linux/kernel` `develop-6.1` @ `77168c8d`, `rockchip_vop2_reg.c`, `rockchip_drm_vop2.c`) |
| Scaling hardware: Cluster 4x, Esmart 8x; the atomic check allows 1/8..8 on any window with scaler registers | read from source (same pin) |
| Cluster sub window follows its main window and both must use the same modifier; sub window input at most 2048 wide | read from source |
| Cluster: linear YUV rejected, linear XRGB2101010/XBGR2101010 rejected on any window; non-AFBC Cluster rejects REFLECT_X and ROTATE_90/270; Esmart rotation REFLECT_Y only | read from source |
| Per-window input/output size up to 4096x7680 (Cluster sub: 2048x7680); the width limit doubles in splice mode (`hdisplay` > 4096) | read from source |
| AXI 0 serves Cluster0, Cluster1, Esmart0, Esmart1; AXI 1 serves the others; 8 layers and 7 mixers bound simultaneous windows; whether all can be on together was not determined | read from source; the last part is inferred in the source |
| DT may restrict windows per CRTC (`rockchip,plane-mask`, `disable-win-move`) and then `possible_crtcs` shrinks; no such DT was read for the NanoPC-T6 | read from source |
| Mainline behaviour is recalled only | not verified in the source document |

Disagreement between the two recordings, inferred to be a kernel difference
(vendor 6.1 vs the 6.18 kernel the board runs): the source document names the
Esmart windows as Primary candidates, while the board shows Cluster0 and
Cluster1 as Primary. The design must therefore not hard-code which window is
Primary; it must read `type` from the kernel (which the generic backend
already does).

## 3. Does the generic DRM backend suffice?

Inferred answer: **the generic backend is enough to start and to stay
correct; a VOP2 constraint table is needed to make the planner's choices
good and its refusals explainable.** The reasoning, constraint by constraint:

| Constraint | Generic backend today | Consequence on VOP2 (inferred) |
|---|---|---|
| Plane count, kind, `possible_crtcs` | Read from KMS | Correct. Cluster1 shows `outputs 0b0` only while its CRTC is not connected; with two connectors the mask is right. A lease or DT `disable-win-move` shrinks the set and the backend already reports only what it sees. |
| zpos | Mutable range read from the property | Correct. The vendor note says the DRM core normalises zpos; the planner's choice of a distinct zpos per plane fits. |
| Alpha, blend modes | From `alpha` and `pixel blend mode` | Correct on all planes. |
| Formats and modifiers | `IN_FORMATS`, linear first, cut at 32 | **Risk.** Cluster planes have more than 32 pairs (observed 2026-11-28: the probe truncated at 32 inside the AFBC group). Pairs past the cut are invisible to the planner, so a valid AFBC format may be refused as `FORMAT`. This is a capacity limit (`MAX_FORMATS`), not a table matter; see question Q1. |
| Scaling | One global `DrmOptions::scaling` for every plane, or none | **Gap.** Without options every plane is "cannot scale", so scaled layers are refused though Esmart (8x) and Cluster (4x) can scale. With one global value the planner would send an 8x layer to a Cluster plane the hardware (or the check) may refuse. Per window-type limits need a table. |
| Transforms | Never filled, so every rotation is refused | **Gap.** Cluster+AFBC can rotate 90/270 and reflect, Esmart reflects in Y. The mask depends on the window type and on the buffer's modifier, so a plain per-plane mask is not enough; see section 5. |
| `max_source` | Not read | Gap, minor: 4096x7680 (Cluster sub 2048x7680) is not stated to the planner. |
| Cluster main/sub pairing (shared AFBC decoder, same modifier) | Not representable | Only matters if `support-multi-area` is set (12 planes); the default 8-plane case has no sub planes. |
| Simultaneous-window and bandwidth limits | Not representable | 7 mixers / 8 layers, AXI split: no rejection path was found in the vendor source. The `TEST_ONLY` check is the arbiter, as for every other backend. |
| Windows shared between CRTCs | Each CRTC is its own output; the planner cannot see that a plane taken by CRTC A is gone for CRTC B | The core already documents this blind spot (`ResourceGroup`); on VOP2 it is larger than on i.MX95 because all windows are movable between ports. |

So the decision is the same one the i.MX95 DPU got (`DriverProfile::Imx95Dpu`):
the generic path is correct because `TEST_ONLY` rejects what the hardware
cannot do, but only a profile lets the planner avoid trial and error and
report `SCALING`, `TRANSFORM`, `RESOURCE` instead of a bare rejection.

## 4. What the constraint table would hold

Proposal, inferred; a `DriverProfile::Rk3588Vop2` variant next to
`Imx95Dpu` is one way to host it. Values are quoted from the source document
and must be re-checked against the kernel actually used (Q2).

Per window class, keyed by the plane's debugfs/KMS name or by the order the
profile can recognise (Q3):

| Field | Cluster (main window) | Esmart | Basis |
|---|---|---|---|
| `scaling` | up 4x, down 4x (hardware); the kernel check lets 1/8..8 through | up 8x, down 8x | read from source; the tighter Cluster figure is the safe choice (inferred) |
| `max_source` | 4096 x 7680 (splice doubles width) | 4096 x 7680 | read from source |
| `transforms` | rotate 90/270 and reflect X/Y **only for AFBC buffers**; none for linear and none for tiled | reflect Y only | read from source; the buffer dependence is the point of section 5 |
| Format/modifier set | AFBC combinations + linear (no linear YUV, no linear 10-bit RGB) | linear only (no 10-bit RGB) | read from source; and observed 2026-11-28 for the AFBC list |
| Sub-window pairing | follows main, same modifier, 2048 width (only with `support-multi-area`) | none | read from source |
| Scaling resource | each window has its own scaler registers | same | read from source; no shared scaler was found, so inferred: no scaler `ResourceGroup` is needed |

Per board/SoC (not per window):

| Field | Content | Basis |
|---|---|---|
| Port capability | VP0 7680x4320, VP1/VP2 4096x2304, VP3 2048x1536; VP0+VP1 splice | read from source |
| Window-to-port policy | default VP plane masks (Cluster0/Esmart0/Cluster2/Esmart2 on the first display, the odd ones on the second) | read from source; this is a default policy, the DT overrides it |
| Layer budget | at most 8 simultaneously active windows (`nr_layers`), inferred upper bound; not proven reachable | inferred |

How it maps to the existing contract (inferred):

- `scaling`, `max_source`, `transforms`: existing `PlaneCaps` fields, no core
  change.
- AFBC-only transforms: `PlaneCaps.transforms` is per plane, not per
  (plane, modifier). Options: (a) report the AFBC transforms and rely on
  `check` to refuse linear+rotate (safe, but the planner will propose
  assignments that fail and move on, which the planner supports through
  backend rejection); (b) report none and lose rotation on AFBC. Option (a)
  needs no core change; a precise model would need a core field and is **not**
  proposed here.
- Esmart vs Cluster class: a planner that prefers the cheapest plane would
  want Cluster for AFBC buffers and Esmart for scaled or YUV layers. The core
  has no cost model; this is a question for the planner, not the table (Q5).
- Resource groups: none needed for scalers. A `ResourceGroup` with
  `LayerTraits::ANY` and capacity 8 over all planes would encode the layer
  budget; leave it out until the budget is observed (Q4).

## 5. Cluster vs Esmart, AFBC, scaling, zpos, per-VP assignment

All inferred from the rows above.

- **Cluster vs Esmart.** Cluster is the AFBC/rotation class (and the only
  class with 10-bit RGB), Esmart is the scaling-8x/NV-format class with
  linear buffers. A layer's requirements (modifier, YUV, scale factor,
  transform) already pick between them through `FORMAT`, `SCALING` and
  `TRANSFORM` reject reasons once the table is filled. No new `LayerTraits`
  is required.
- **AFBC.** The modifier is part of the buffer, matched exactly by the
  planner, so an AFBC buffer is only placed on a Cluster plane provided its
  (format, modifier) pair is listed. The AFBC variants are flag
  combinations (SPARSE, YTR, CBR, SPLIT); the list the backend sees decides
  which are usable. Producers (GPU, decoder) must hand out a modifier the
  Cluster plane lists; negotiating that is outside the backend.
- **Scaling.** Esmart 8x, Cluster 4x, each with its own scaler: no shared
  scaler to model. The documented odd-width/mod-16 Esmart down-scale
  exception is a driver-internal case the table cannot express; `TEST_ONLY`
  covers it.
- **zpos.** Mutable range, normalised by the DRM core. The existing
  `Zpos::Range` handling applies. Open point: every plane reports 0..=7
  regardless of CRTC, while a window on a CRTC competes with other windows of
  that CRTC for positions only (Q6).
- **Per-VP assignment.** All windows can reach all registered CRTCs unless
  the DT narrows it. With one active connector the 6.18 recording shows 8
  planes, 7 of which are usable on it. With two active connectors the
  planner (per output) can propose the same plane to both, and the second
  commit would fail. Two ways, both without a core change: (a) the caller
  gives each output its own DRM lease (the repository's lease path already
  limits `capabilities` to granted objects); (b) a backend-level policy that
  splits windows between ports using the vendor default masks. (a) is the
  smaller proposal.

## 6. Tests on recorded topologies

No test is written by this item. Proposed tests, all host-only; they prove
planner behaviour against a **recorded** description, never hardware
behaviour:

1. **Fixture from the recording.** Build `Capabilities` for the 8-plane
   topology (plane kinds, `outputs` masks, zpos 0..=7, alpha, three blend
   modes, format lists as recorded) as a constant in a test, with the
   observation date and doc in a comment, mirroring `dpu95_stream()` in
   `crates/composition-drm/src/profile.rs`.
2. **Table tests** (mirroring the `Imx95Dpu` tests):
   - an ARGB layer scaled 4x up lands on Esmart or Cluster, 6x up only on
     Esmart (`SCALING` on Cluster);
   - an NV12 linear layer is refused on Cluster (`FORMAT`) and accepted on
     Esmart;
   - an AFBC buffer is accepted on Cluster and refused on Esmart (`FORMAT`);
   - a 90-degree rotation is refused on Esmart (`TRANSFORM`), accepted on
     Cluster for the AFBC case as designed in section 4;
   - a source wider than 4096 is refused (`SOURCE_SIZE`);
   - eight RGB layers fit the eight planes, a ninth is `NoPlane`;
   - `generic_profile_adds_nothing`-style check that `Generic` leaves the
     fixture untouched.
3. **Two-output fixture.** Both CRTCs connected: assert the documented blind
   spot (the same plane proposed for both outputs), so a later change that
   fixes it is a visible diff. Reuse the cross-check idea of
   [`docs/libliftoff-crosscheck.md`](libliftoff-crosscheck.md) to compare
   the planner's refusal set with libliftoff on the same recording, as a
   later step.
4. **Truncation test.** A fixture with more than 32 (format, modifier)
   pairs on a Cluster plane, asserting which pairs survive the cut and that
   linear stays first (Q1).
5. **Format-list recording.** The existing probe truncates at 32 pairs; a
   complete `IN_FORMATS` recording is a prerequisite of tests 1 and 4 and is
   a board task, not part of this design.

## 7. Open questions

Answers for Q1–Q4, Q6, Q8 and Q9 came from the NanoPC-T6 board session of
2026-11-28, recorded in [`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md);
each answer line links to that document. Q5 and Q7 stay open. Where a line
below is tagged `observed`, the dated measurement is in the session document;
the rest keeps its original inferred status.

- **Q1. Answered (observed 2026-11-28).** Yes: each Cluster plane carries
  126 (format, modifier) pairs over 9 Rockchip modifiers, so the cut at
  `MAX_FORMATS = 32` hides 94 of them. Cluster planes list no linear pair at
  all, and the probe prints in the kernel's blob order — the ordering claim
  resolves, and linear buffers are still accepted on a Cluster plane.
  Whether `MAX_FORMATS` must grow is a core decision; the session records it
  as a finding and does not change it
  ([session Q1](rk3588-vop2-session-2026-11-28.md)).
- **Q2. Answered for the measured board (observed 2026-11-28); vendor 6.1
  half open.** The board runs `6.18.55-current-rockchip64`, the pinned tags
  `v6.18` and `v6.18.55` are still the canonical refs today, and every
  source prediction the session could test held on the board. The vendor
  tree's own patch list was not diffed, so the vendor-6.1 half stays open
  ([session Q2](rk3588-vop2-session-2026-11-28.md)).
- **Q3. Answered (observed 2026-11-28).** Identify the class from the
  `IN_FORMATS` family: a 9-modifier, 14-format set means Cluster; a
  21-format, linear-only set means Esmart. This kernel offers no
  `SCALE_RATE`/`SCALING` property, and `rotation` is identical on all
  planes, so neither separates the classes
  ([session Q3](rk3588-vop2-session-2026-11-28.md)).
- **Q4. Answered (observed 2026-11-28).** No: a `TEST_ONLY` sweep accepted at
  most 4 planes on one CRTC (5, 6 and 7 rejected `EINVAL`), matching the
  source limit `nlayers = 4` per CRTC
  ([session Q4](rk3588-vop2-session-2026-11-28.md)).
- **Q5.** Open, unchanged. The core has no cost model; adding one is outside
  this note (inferred).
- **Q6. Answered for one CRTC (observed 2026-11-28).** The `zpos` property is
  global-looking (`0..=7` on every plane) but ordering is normalized per
  CRTC; equal values are accepted, the tie breaks by lower object id, and
  normalized ranks are contiguous `0..n-1`, not the written values. The
  two-CRTC predictions stay untested: the board's second CRTC has no
  connector ([session Q6](rk3588-vop2-session-2026-11-28.md),
  `docs/rk3588-vop2-zpos-source.md` §6).
- **Q7.** Open, unchanged. The measured board still has only one connected
  output, so two leases on one board were not tested.
- **Q8. Answered (observed 2026-11-28).** On this kernel the extra windows do
  not become DRM planes: exactly 8 planes exist, all `win0` — no `win1`, no
  multi-area plane — so the pairing rule does not arise
  ([session Q8](rk3588-vop2-session-2026-11-28.md)).
- **Q9. Answered (observed 2026-11-28).** A new example,
  [`flip_probe`](../crates/composition-drm/examples/flip_probe.rs), measured
  overlay flips on the board: 120 non-blocking commits on one overlay, all
  accepted, each with exactly one page-flip event; a second commit against a
  flip that has not latched returns `EBUSY` (three runs). The `twoplane`
  harness cannot measure this driver: its gate requires a primary plane that
  *lists* linear RGB pairs, and Cluster primaries list none even though the
  kernel accepts linear buffers
  ([session Q9](rk3588-vop2-session-2026-11-28.md)).

## 8. Proposed items

Inferred, not scheduled by this note:

1. Board: full `IN_FORMATS`, rotation and scaling property dump, and kernel
   version for a Rockchip board (answers Q1, Q2, Q3, Q6).
2. Board: `TEST_ONLY` sweep of plane combinations (Q4).
3. Code: `DriverProfile::Rk3588Vop2` with the table of section 4 and the
   tests of section 6.
4. Code, only if Q1 shows loss: raise `MAX_FORMATS` or prune the list by
   modifier family.
5. Docs: move the answers into `docs/platform-capabilities.md` with their
   dates when they exist.

Progress 2026-11-28: the board session in
[`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md)
carried out items 1 and 2 and recorded the item-4 finding (94 hidden pairs)
without changing `MAX_FORMATS`; item 3 stays code work, item 5 was not done
in that session.
