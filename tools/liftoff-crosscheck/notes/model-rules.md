# Rules of the test-commit model (notes for `src/model.rs`)

Written 2026-11-28 for item xc-model. **Notes, not code, and no hardware claim.** The model is a fake kernel over *recorded or synthetic* topologies; what it accepts or rejects says nothing about a board. Every report line carries the `origin` of its input (design section 5).

Source: `docs/libliftoff-crosscheck.md` (the "design"), read at tip `d028587`. `§` = design section. The design's own list of model checks is §6, paragraph "The test-commit model". Where the design names no rule the entry is marked **open**: the implementer decides, writes the decision into a code comment, and tells the report author.

## 0. Shape of the model

- A pure function of the parsed xcheck file and a set of plane states (§6 "a pure function of the file", §5 "No hidden state"): `check(topology, model_kind, states) -> Result<(), Reject>`.
- A plane state: plane id, output id, buffer id (none = plane disabled), `src` (whole pixels; KMS 16.16 is the value shifted by 16, §4), `dst`, zpos, alpha, blend, transform.
- `model_kind` is `structural` or `full` (§6). R1-R7, R9, R10 apply in both; R8, R11-R13 only in `full`.
- Written **independently of `planner.rs`** (§6): no calls into the planner or its reject-reason code. Agreement with the planner is the measured quantity, so sharing code would make it circular.
- Reject carries a plane id and a KMS property name, plus an errno. libliftoff treats only `EINVAL`, `ERANGE`, `ENOSPC` as "try another"; any other errno aborts its search (§2), so the model must return only these three. Which errno per rule is **open** in the design; the proposal below follows kernel practice (**recalled**, unchecked). Proposed type: `Reject { plane: u32, prop: &'static str, errno: Errno, detail: String }`.
- Checks run in R-number order; the first failure is reported. The verdict must not depend on state order; only the reported cause may.

Example notation: `src WxH -> dst x,y WxH z=N`; buffer `WxH fourcc/modifier`. Examples assume one output `0` of 1920x1080.

## 1. Rules

### R1. Plane and output exist
- **Source:** §6 ("the plane can reach the output"), §4 (`PlaneCaps::id`, `OutputCaps`), §2 (CRTC id list).
- **xcheck field:** `output <id>`, `plane <id>`, `frame ... output=<id>`.
- **Reject:** unknown plane or output id in a state -> `Einval`, `CRTC_ID`, that plane.
- **Accept:** plane 40 on output 0, both in the file.
- **Reject:** state for plane 99, absent from the file.

### R2. Possible CRTCs
- **Source:** §6, §4 (`outputs` row: core bits index `Capabilities::outputs`, KMS bits index CRTCs in device order; the dump must carry both consistently).
- **xcheck field:** `plane ... outputs=<mask>`; the 0-based index of the `output` line in file order is the bit.
- **Reject:** bit clear -> `Einval`, `CRTC_ID`, the plane.
- **Accept:** `outputs=0x1`, output at index 0.
- **Reject:** `outputs=0x2`, output at index 0.

### R3. Plane type
- **Source:** §4 (`kind` row), §7 "Cursor planes: undefined", §4 (keep cursor planes out of v1).
- **xcheck field:** `plane ... kind=`.
- **Reject:** a state naming a `kind=cursor` plane -> `Einval`, `type`, the plane. This is a model guard for v1, not a kernel rule.
- **Open:** whether a state with no enabled primary plane is refused. The design is silent (libliftoff's own mock has an optional "primary must be enabled" rule, §2, but drivers differ). Recommended: do not refuse; the model refuses only what the file says.
- **Accept:** overlay plane 41 with a buffer. **Reject:** cursor plane in the state.

### R4. Buffer exists
- **Source:** §4 (`Layer::buffer` row: fake needs a framebuffer table), §6 skip list (`UnknownBuffer`).
- **xcheck field:** `buffer <id> <w> <h> <fourcc> <modifier>`.
- **Reject:** buffer id not in the file -> `Einval`, `FB_ID`, the plane.
- **Accept:** buffer 1 present. **Reject:** buffer 7 absent. (Core skips such frames earlier; the C driver could still send one.)

### R5. Format and modifier
- **Source:** §6 ("the framebuffer's format and modifier are in the plane's list"), §4 (`formats` <-> `IN_FORMATS`), §7 "Format check skipped" and "Format/modifier absent from dump".
- **xcheck field:** the plane's `format <fourcc> <modifier>` lines; the buffer's fourcc and modifier.
- **Reject:** the pair (fourcc, modifier) is not among the plane's lines -> `Einval`, `FB_ID`, the plane. Compare the pair; modifier `0x0` (linear) is not a wildcard.
- **Accept:** plane lists `AR24 0x0`; buffer `AR24 0x0`.
- **Reject:** plane lists `NV12 0x0` only, buffer `AR24 0x0`; or plane lists `AR24 0x0` only, buffer `AR24` with a non-zero modifier.
- This is the one place the format check always happens: libliftoff's own pre-check can be skipped (§7), and then the test commit refuses. Truncated dumps (§3, 32 pairs) make the model refuse more than the hardware would; the rule is unchanged, the report labels it.
- **Open:** an empty format list. Recommended: accepts nothing (the converter should refuse such planes; see `fixtures/README.md`).

### R6. zpos: fixed, range, uniqueness
- **Source:** §6 ("zpos is within range or equal to the fixed value, and zpos values in use are unique"), §4 (`zpos` Range and Fixed rows), §5 (`zpos=`, `zpos_now`), §2 (libliftoff never writes a zpos), §7 "Programmable zpos".
- **xcheck field:** `plane ... zpos=fixed:N|N..M`. `zpos_now` is not read by the model (it only drives libliftoff's plane order).
- **Reject (a)** fixed: state zpos != N -> `Einval`, `zpos`, the plane.
- **Reject (b)** range: outside `N..M`, inclusive as in core's `min..=max` (`caps.rs`) -> `Erange`, `zpos`, the plane.
- **Reject (c)** two enabled planes on the same output with equal zpos -> `Einval`, `zpos`, the higher plane id; put both ids in `detail`.
- **Accept:** (a) `fixed:1`, zpos 1; (b) `0..7`, zpos 5; (c) planes at zpos 1 and 2.
- **Reject:** (a) `fixed:1`, zpos 0; (b) `0..7`, zpos 8; (c) planes 41 and 42 both at zpos 3.
- Sweep note (§7): libliftoff submits each plane's current value; the driver puts `zpos_now` into the state for liftoff-originated requests, core-originated states carry core's zpos. The model treats both alike.
- **Open:** whether a primary plane must be lowest; whether layer order must match zpos order. Recommended: neither is a model rule; "order matches for overlapping layers" belongs to the comparison's replay (§6.4), not the model.

### R7. Source and destination bounds
- **Source:** §4 (`Layer::source`, `Layer::destination`), §6 skip list (`InvalidSource`, `InvalidDestination`, `CannotClip`), §7 "Clipping" (proposal: restrict v1 to frames inside the output).
- **xcheck field:** layer `src=`, `dst=`; the buffer's size; the output's size.
- **Reject (a):** `src` empty, negative, or not inside the buffer -> `Einval`, the first offending `SRC_X/Y/W/H`, the plane.
- **Reject (b):** `dst` with zero width or height -> `Einval`, `CRTC_W` or `CRTC_H`.
- **Reject (c):** `dst` not inside `0,0,out.w,out.h` -> `Einval`, the first offending `CRTC_X/Y/W/H`. **Open** whether to clip instead; recommended: refuse, so clipping frames stay `skipped` as §7 intends.
- **Accept:** buffer 1920x1080, `src 1920x1080 -> dst 0,0 1920x1080`.
- **Reject:** (a) buffer 960x540, `src 1920x1080`; (b) `dst 0,0 0x100`; (c) `dst 100,100 1920x1080`. The last is the second layer of frame `scaled_small` in `fixtures/imx95-synthetic.xcheck` (synthetic); the frame author should know it falls outside the in-output restriction.
- **Open:** alignment. The design names **no** source or destination alignment rule (even width for YUV, stride, 2-pixel steps) and xcheck has no field for one. The model applies none. A change would need a new field and a design edit; report it, do not invent it.
- Sub-pixel source: xcheck `src` is whole pixels (§4), so the model never sees fractions.

### R8. Scaling limits (`full` only)
- **Source:** §6 (`full` includes scaling), §4 (`scaling` row: does not map to KMS), §3 (scaling only from a `DriverProfile`).
- **xcheck field:** `plane ... scaling=none|up:<hex16.16>/down:<hex16.16>`, or the profile's values when `profile imx95dpu` (§5: one origin; the reader refuses both).
- **Reject:** `scaling=none` and `src.w != dst.w` or `src.h != dst.h` -> `Erange`, `SRC_W`/`SRC_H`, the plane. With limits: `dst/src > up` or `src/dst > down` on an axis, compared in 16.16 integers, no floats -> `Erange`, `CRTC_W`/`CRTC_H`.
- **Accept:** `none`, `src 1920x1080 -> dst 1920x1080`; or limit 2x both ways (`0x20000`), `src 960x540 -> dst 1920x1080`.
- **Reject:** `none`, `src 960x540 -> dst 1920x1080`; or 2x limit, `src 400x300 -> dst 1920x1080`.
- **Open:** core's `ScaleLimits` holds integers `max_upscale`/`max_downscale` (`caps.rs`), xcheck holds 16.16 hex. Whether a ratio equal to the limit passes: the model uses `<=` (accepts). xc-core must convert the same way.
- **Open:** `scaling=none` as "no scaling at all" (recommended) versus "unknown".
- Profile values come from the `Capabilities` that xc-core builds, not from a table in the model (see R13 note).

### R9. Alpha, blend, rotation
- **Source:** §6 ("alpha, blend mode and rotation are among those the plane lists"), §4 (`plane_alpha`, `blend_modes`, `transforms` row), §7 "Value checks before the test commit".
- **xcheck field:** plane `alpha=<0|1>`, `blend=`, `transforms=`; layer `alpha=`, `blend=`, `transform=`.
- **Reject (a):** plane `alpha=0` and layer alpha != 65535 -> `Einval`, `alpha`. Alpha 65535 is the default and passes on every plane (as in libliftoff's skip list, §2).
- **Reject (b):** layer blend not in the plane's `blend=` list -> `Einval`, `pixel blend mode`. **Open** for `blend=-`; recommended: accepts only `opaque` (§4 asks the harness to always expose the property when modes are listed).
- **Reject (c):** layer transform not in `transforms=` -> `Einval`, `rotation`. `transforms=-` accepts only `normal`; a dump that did not read the bitmask (§3) then rejects rotated frames, and the report must say so.
- **Accept:** plane `alpha=1 blend=opaque,premul`, layer `alpha=40000 blend=premul transform=normal`.
- **Reject:** (a) plane `alpha=0`, layer alpha 40000; (b) plane `blend=opaque`, layer `coverage`; (c) plane `transforms=normal`, layer `rot90`.
- **Open:** layer alpha 0 (libliftoff drops the layer, §7 "Invisible layers"; recommended: model treats a state with alpha 0 like any other value); combined rotation and flip (xcheck `transform=` is a single name, §5); rotation changing size or format constraints (not modelled).

### R10. Plane used once
- **Source:** inferred from §6.4 (replayed assignments must be valid) and §5 (one plane per layer).
- **xcheck field:** none (structure of the state set).
- **Reject:** two states for one plane id -> `Einval`, `FB_ID`, that plane.
- **Accept:** each plane at most once; one buffer on several planes is allowed (frame `six_rgb` uses buffer 1 six times).
- **Reject:** plane 41 listed twice.

### R11. Source size limit (`full` only)
- **Source:** §6 (`full` includes source limit), §4 (`max_source` row).
- **xcheck field:** `plane ... max_source=<WxH|none>`.
- **Reject:** `src.w > W` or `src.h > H` -> `Erange`, `SRC_W`/`SRC_H`, the plane. `none` never rejects.
- **Accept:** `max_source=1920x1080`, `src 1920x1080`. **Reject:** `max_source=1280x720`, `src 1920x1080`.
- **Open:** before or after rotation (recommended: as given in `SRC_*`).

### R12. Full-output planes (`full` only)
- **Source:** §6 ("`full_output` planes cover the output"), §4 (`full_output_only` row).
- **xcheck field:** `plane ... full_output=<0|1>`; output size.
- **Reject:** `full_output=1` and `dst != 0,0,out.w,out.h` -> `Einval`, first offending `CRTC_X/Y/W/H`, the plane. (With R7(c), "covers" equals "is".)
- **Accept:** `dst 0,0 1920x1080`. **Reject:** `dst 0,0 960x540`.

### R13. Resource groups (`full` only)
- **Source:** §6 ("every group's capacity holds"), §4 (`ResourceGroup` row), §5 (`group consumers=... planes=... capacity=<n>`).
- **xcheck field:** `group consumers=<any,scaled_h,scaled_v,yuv>[,...] planes=<ids> capacity=<n>`.
- **Reject:** per group, count enabled planes of `planes=` that are consumers; if the count exceeds `capacity` -> `Enospc`, `FB_ID`, the first plane in id order beyond the capacity. A plane is a consumer if, for any listed kind: `any` = enabled; `scaled_h` = `src.w != dst.w`; `scaled_v` = `src.h != dst.h`; `yuv` = buffer fourcc is YUV. A plane counts once per group.
- **Accept:** a group `consumers=yuv`, planes 40..45, capacity 4; four NV12 layers and two AR24 layers (frame `four_yuv_two_rgb` of the synthetic i.MX95 fixture).
- **Reject:** the same group with five NV12 layers (frame `five_yuv`).
- The synthetic fixture has **no** `group` line: `profile imx95dpu` supplies it on the core side (§5). So the model must take a resolved constraint set (file lines or profile, never both), built once from the file plus profile; recommended interface: a `ModelTopology` built by xc-core/xc-model together.
- **Open:** the YUV fourcc list (NV12 is the only one in the fixtures; keep one list in one place and cite it); whether `capacity` counts planes or weighted units (the design says only `<n>`: count planes).

### R14. Per-CRTC plane count and bandwidth
- **Source:** none. §6 lists the model's checks and names no per-CRTC plane cap, memory-bandwidth or clock budget; §2 mentions `ENOSPC` only as an errno libliftoff retries.
- **xcheck field:** none exists.
- **Status: open.** Recommended: not modelled in v1. The only count rule the design names is R13. Adding one needs an xcheck field and a design edit.
- No accept/reject example, by design. Test instead that the model accepts a state with all planes enabled when no other rule refuses it (frame `six_rgb` of the synthetic fixture), proving no hidden count rule.

## 2. Summary

| Rule | Topic | Model kind | Reject (proposal) | Design |
| --- | --- | --- | --- | --- |
| R1 | plane and output exist | both | `Einval` CRTC_ID | §2, §4, §6 |
| R2 | possible CRTCs | both | `Einval` CRTC_ID | §4, §6 |
| R3 | plane type (cursor guard) | both | `Einval` type | §4, §7; primary rule open |
| R4 | buffer exists | both | `Einval` FB_ID | §4, §6 |
| R5 | format and modifier | both | `Einval` FB_ID | §6, §7 |
| R6 | zpos fixed, range, unique | both | `Einval`/`Erange` zpos | §2, §5, §6 |
| R7 | src/dst bounds | both | `Einval` SRC_*/CRTC_* | §4, §6, §7; alignment open |
| R8 | scaling | full | `Erange` SRC_*/CRTC_* | §4, §6 |
| R9 | alpha, blend, rotation | both | `Einval` alpha, pixel blend mode, rotation | §4, §6, §7 |
| R10 | plane used once | both | `Einval` FB_ID | inferred §6.4 |
| R11 | max_source | full | `Erange` SRC_* | §4, §6 |
| R12 | full_output | full | `Einval` CRTC_* | §4, §6 |
| R13 | resource groups | full | `Enospc` FB_ID | §4, §5, §6 |
| R14 | per-CRTC count, bandwidth | none | none | open, not in design |

## 3. Topology files

`tools/liftoff-crosscheck/topologies/*.xcheck` did not exist on tip `d028587` when this was written, so none is cited and nothing here depends on one. Items xc-topo-* will add them; the xc-model implementer should add a per-topology test then. Cited from the tip: `fixtures/imx95-synthetic.xcheck` (origin **synthetic**), frames `six_rgb`, `four_yuv_two_rgb`, `five_yuv`, `scaled_small` (R7, R13, R14).

## 4. Test plan for the implementer

- One unit test per accept and per reject example above, on a small in-memory topology (no file reads).
- Reject tests assert plane, property and errno, not only `is_err()`.
- `structural` accepts the R8, R11, R12, R13 reject examples.
- R6(c) is symmetric, and the verdict is invariant under shuffling the states.

## 5. For the report author

A difference that traces to an **open** rule is a model choice or artefact, not a planner lead, until the choice is justified. Nothing here is a statement about any kernel or board.
