# Triage template for the cross-check report (design section 7)

Input to item xc-report. Source: `docs/libliftoff-crosscheck.md` sections 6 and 7 (design at the tip). This file holds predictions and criteria only: no results, no measurements, no hardware claim. Every row is *inferred* in the design and stays so until a run confirms or refutes it. Recorded or synthetic topologies give no hardware claim; the report labels each result with its input's origin.

Frame files: `tools/liftoff-crosscheck/frames/section7/` does not exist on the tip this note was written against, so no frame file is named here. xc-report fills the `frame file` column from what is on its tip. A difference without a replayable frame file is not triaged (see "Lead").

## How to read a sweep line

Outcome classes (design section 6): `both-accept`, `both-reject`, `core-only`, `liftoff-only`, `invalid-assignment(core|liftoff)`, `core-error(<PlanError>)`, `skipped(<reason>)`.

- Section 6 step 3 defines acceptance: core `Ok` and every visible layer on a plane for libliftoff. A partial libliftoff allocation counts as rejected; the number of placed layers is extra data.
- "Which side composites": libliftoff composites the layers with `liftoff_layer_needs_composition` true (no plane). Core composites nothing; it returns `Err(PlanError)` (with `RejectReasons` for `NoPlane`).
- Plane ids are only comparable when both sides accept, and equal assignments are not required (section 6 step 4). A plane-id difference alone is never a difference.
- The design names the class of S7-2, S7-5 and S7-7 `liftoff-only` and S7-3 `core-only`. Read as: `core-only` = core accepts and libliftoff does not; `liftoff-only` = libliftoff accepts and core does not. S7-11 is the one row where the design's label (`liftoff-only`) and its prose (libliftoff refuses what core accepts) disagree; the row below uses the prose. xc-sweep should print which meaning it uses.
- Core runs `profile generic` against `model=structural` and the file's profile against `model=full`. State both when a row depends on it.

## Predicted differences

Every row's criterion has three parts: the observed class, the signature, and the confirming check. If the signature fits but the check fails, it is not this row; go on to the artefact and lead tests.

| id | Statement | Observable signature (sweep output) | Classing criterion |
| --- | --- | --- | --- |
| S7-1 | Contract: libliftoff places some layers, core errors (partial vs all-or-nothing). | Core `Err(PlanError)`, typically `NoPlane`. libliftoff places k layers with 0 < k < visible layers; the rest are composited. Counted `both-reject`, reported with k. | libliftoff's placed layers replay through the model and pass, and the model refuses the full layer set. If the model accepts the full set, it is not S7-1 (see S7-11 or lead). |
| S7-2 | zpos only between intersecting layers: libliftoff accepts non-overlapping layers needing out-of-order planes, core refuses. | `liftoff-only`. Core `Err` with a z-order reason. The out-of-order layers' destination rectangles are disjoint. libliftoff's assignment passes the model. | The refusal goes away in core (or appears in libliftoff) when the out-of-order pair is made to overlap. State whether core's refusal was checked against `planner.rs`; the design did not. |
| S7-3 | Programmable zpos: core may assign any zpos in range; libliftoff only orders planes by current zpos. | `core-only`. Topology has restackable planes (zpos range wider than one value, e.g. 0..=7). Layer order disagrees with the planes' current zpos order. libliftoff leaves a layer composited. | Core's assignment passes the model with a zpos different from the plane's current one, and libliftoff's refusal disappears when the frame's layer order matches the planes' current order. |
| S7-4 | Invisible layers: libliftoff drops alpha-0 and `FB_ID`-0 layers; core's handling of alpha 0 is unchecked. | Either direction. Frame has alpha-0 layer(s). Compared as a separate class. | The outcome differs with the layer present and agrees with it removed from both sides, and no other signature applies. Report how core treated the alpha-0 layer from the sweep output, not from assumption. |
| S7-5 | Format check skipped: libliftoff defers the format check unless the FB has modifiers info and the `IN_FORMATS` blob exists. | `liftoff-only`: format absent from the plane's list, core refuses, libliftoff accepts. | Artefact first. With the harness setting `DRM_MODE_FB_MODIFIERS` and the blob this must not occur; if it does, it is a harness bug. It is S7-5 only on a deliberate mode that omits them. |
| S7-6 | Value checks before the test commit: libliftoff refuses out-of-range alpha, unknown blend enum or unsupported rotation itself, when the fake defines them. | Either. One side refuses on alpha, blend mode or rotation not in the plane's list while the other does not. | The refusing side's rule is the property-definition check (`values[]`/`enums[]`) and the fake defined them, and the model refuses the same value. If the fake omitted flags or enums it is an artefact (fake limit). |
| S7-7 | Search effort: libliftoff hits its deadline or core hits `SearchLimits`. | `liftoff-only`, or `core-error(LimitExceeded)` / `core-error(BackendRejected)`, or the deadline-reached flag set in the sweep record. Many layers on many planes. | The record carries the deadline flag or the core limit error, and a re-run with a larger timeout or limit agrees with the other side. Without such a flag it is not S7-7. |
| S7-8 | Search determinism: libliftoff flips between runs (1 ms default deadline). | Same frame, different libliftoff outcomes across repeated runs; recorded `unstable`. | At least two runs of the same frame differ with the raised timeout in force. Report run counts. A flip with the default timeout is an artefact (driver bug). |
| S7-9 | Truncated format list: dumps cut at 32 formats per plane, so the dump differs from the hardware. | `core-only` or `both-reject`. The frame's format is absent from the plane's truncated list. | The frame's format is among those the probe marks truncated (design section 3) and both sides see the same list. A property of the input, not a planner difference. |
| S7-10 | Knowledge gap, scaling/groups: core with a profile knows limits libliftoff cannot learn from properties. | `core-only` under `model=structural`: core refuses, libliftoff accepts. Under `model=full` both refuse. | The difference is present under `structural` and gone under `full`. If it persists under `full`, it is a lead, not S7-10. |
| S7-11 | Knowledge gap, other direction: libliftoff refuses what core accepts, under `model=full`. | libliftoff leaves a layer composited, core `Ok`, `model=full`. | Always a lead, after one check: the model refusing is by a rule core lacks, or the planner misses a constraint. Check the model rule against the topology first; a wrong rule is an artefact. |
| S7-12 | Clipping: core clips a layer beyond the output and programs the visible part; libliftoff gets the original coordinates. | A layer extends beyond the output; core `Ok` with clipped `Assignment::source/destination`; the sides differ. | First versions restrict frames to inside the output (design proposal). Such a frame in the sweep is a generator artefact unless clipping frames were generated on purpose. |
| S7-13 | zpos tie rule: core stacks equal `z_order` in frame order; the harness maps ties to distinct zpos ranks. | Either. Frames with equal `z_order` layers. | The difference vanishes when ties are replaced by distinct `z_order` in frame order. If it does not vanish, the mapping is wrong (artefact) or it is a lead. |
| S7-14 | Cursor planes: not exercised in v1. | No frame involves a cursor plane assignment. | Any cursor-plane involvement is an artefact. |
| S7-15 | Composition layer: excluded by design. | None. | Any composition layer in a record is an artefact. |
| S7-16 | Format/modifier absent from dump: both reject. | `both-reject` on the same input. | Not a difference; listed so it is not reported as one. If one side accepts, see S7-5, S7-6 or artefact. |
| S7-17 | Planes visible per output: both see all planes whose mask includes the single output. | Either, only if plane reachability differs between the sides. | The expected case is no difference. A difference caused by a plane's CRTC mask handled differently by fake, model or core is an artefact, or a lead if the fake and model are right and core differs. |

## Artefact signatures

An artefact is a harness defect or a limit of the model or the fake libdrm. It is fixed or documented, not reported as a planner finding. Signatures the design names:

- Format check done or skipped depending on whether the fake sets `DRM_MODE_FB_MODIFIERS` and `IN_FORMATS` (S7-5).
- Value checks off because the fake registers property definitions without flags, values or enums (S7-6; the stock libliftoff mock has `/* TODO: fill flags */`).
- Stock mock limits, if it is ever used: one CRTC, `possible_crtcs` fixed to bit 0, one device per process, `MAX_PLANES` 64, `MAX_LAYERS` 512.
- libliftoff run not fresh (previous allocation reused; layer priority depends on earlier calls).
- libliftoff deadline reached because `timeout_ns` stayed at the 1 ms default (S7-7, S7-8).
- Clipping frames fed unclipped to one side only (S7-12).
- Wrong zpos-rank mapping for ties (S7-13).
- Model rules that call core's `planner.rs` (circular agreement), or model and fake disagreeing on the same file.
- Cursor planes or a composition layer present (S7-14, S7-15).
- `invalid-assignment(core|liftoff)`: an assignment that fails replay through the model. Artefact until the model rule is checked against the topology; a confirmed one is a lead.
- Frames core refuses before planning for non-plane reasons (`UnknownBuffer`, `InvalidSource`, `InvalidDestination`, `CannotClip`) are `skipped`, not differences.

Rule of thumb: rerun the frame with the fake's flags, enums and blob as intended and a long timeout. If the difference disappears, it was an artefact.

## What makes something a lead

A lead is a difference that matches no S7 row's criterion and no artefact signature after the checks above, or a row whose criterion turns it into one (S7-10 persisting under `model=full`, S7-11, S7-17 with a correct fake and model, a confirmed `invalid-assignment`). Before it is written up it needs:

1. a replayable frame file in xcheck v1 (a lead is never summarised without it);
2. the outcome class under both model settings, and the planner outcome (`Ok` assignments, or `PlanError` with `RejectReasons`);
3. the model rule or plane constraint that decides it, named, with a statement that it was checked against the topology;
4. the origin of the input (recorded, synthetic or hand-written).

A lead is a finding for the planner owner. The cross-check edits no core sources, and a lead is not a hardware claim.

## Results (empty; filled by xc-report)

Verdict values: `expected(S7-n)`, `artefact`, `lead`. One row per found difference.

| id | topology | origin | frame file | verdict | note |
| --- | --- | --- | --- | --- | --- |
|  |  |  |  |  |  |
