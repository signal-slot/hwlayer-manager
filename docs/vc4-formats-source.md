# vc4 HVS5 / HVS6: formats x modifiers per plane, counted, against the observed blobs and `profile.rs`

Dated 2026-11-28. Source evidence for the format and modifier claims of `crates/composition-drm/src/profile/vc4.rs` (module doc, "Modifiers" bullet) and for `docs/hvs-backend-design.md` (formats and modifiers, section 4 and section 5). Nothing here was measured by this document. It adds three things to the existing read in `docs/vc4-hvs-format-source.md`: a count of every format/modifier pair per generation, the effect of `MAX_FORMATS` on those counts, and a pair-by-pair comparison with the recorded blobs and the profile.

Tags: **read from source** = read in the named tree, with file:line where a line is known; **observed 2026-11-27 / 2026-11-28** = copied from `docs/observed/` or `crates/composition-drm/tests/data/` (read only); **inferred** = counted or concluded here.

## 1. Provenance

- Pinned tree: `raspberrypi/linux`, branch `rpi-6.18.y`, commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e`, `drivers/gpu/drm/vc4/vc4_plane.c`. The line numbers below (`vc4_plane.c:41-315` the `hvs_formats[]` table, `vc4_plane.c:2761-2837` `vc4_format_mod_supported`, `vc4_plane.c:2858-2881` the `modifiers[]` array and the format filter) are those of `docs/vc4-hvs-format-source.md`, which read a full copy of the raw file (3004 lines) and checked its line count against the pin. read from source
- Independent cross-check made for this document on 2026-11-28: the raw `vc4_plane.c` at the pinned commit was fetched again as text and the table, the hook and the array in `vc4_plane_init` were compared with the earlier read. They are the same (45 table entries, the same gating flags, the same hook rules, the same modifier array order). The fetch went through a text-reading helper, so its own line counts are only as good as that helper's (it reported the table at lines 41-315, the same as the earlier read). read from source
- The 6.12 tree: `raspberrypi/linux`, branch `rpi-6.12.y`, tip as served on 2026-11-28; the commit id was not shown, and no line numbers were read (landmarks are function names). The shallow and sparse clones that this item was to place in the dependency cache could not be made here (the cache directories were empty, being filled by another item, and a git fetch was refused), so this is the raw file only. read from source, commit not known
- What the 6.12 tip shows: the same 45-entry `hvs_formats[]` with the same `hvs5_only` and `hvs6_only` flags, the same `modifiers[]` order, and a `vc4_format_mod_supported` **without** the gen-6 early return for T-tiled and SAND64 (rule 1 of `docs/vc4-hvs-format-source.md` section 3.2). Every other rule is the same. read from source
- The boards: BCM2711 = reTerminal CM4, kernel `6.12.47+rpt-rpi-v8`; BCM2712 = Raspberry Pi 5, kernel `6.18.50+rpt-rpi-2712`. Neither is a pinned commit, so the board kernels can differ from both trees read here. observed 2026-11-27
- Reference for how the core builds `IN_FORMATS` from the hook (the blob keeps a pair only when the hook returns true) is mainline `drm_plane.c`, not the rpi tree: inferred for the rpi tree (`docs/vc4-hvs-format-source.md` section 3.1).

## 2. The rule in one place

All read from source (pinned commit; the 6.12 tip differs only where stated in section 1).

1. The plane's format list is `hvs_formats[]` without `hvs5_only` entries on gen 4, and without `hvs6_only` entries below gen 6. `hvs5_only` therefore means "not gen 4": the five entries P030, XR30, AR30, AB30, XB30 stay on gen 6. The six `hvs6_only` entries are P010, P012, P016, S010, S012, S016. `vc4_plane.c:2866-2874`, `vc4_plane.c:279-313`
2. The modifier list is one array for all planes and generations: T_TILED, SAND128(0), SAND64(0), SAND256(0), LINEAR. `vc4_plane.c:2858-2865`
3. A pair is advertised when `vc4_format_mod_supported` says yes (rules in section 3 of `docs/vc4-hvs-format-source.md`):
   - gen >= 6: T_TILED and SAND64 are refused for every format (`vc4_plane.c:2767-2771`);
   - XR24, AR24, AB24, XB24, RG16, BG16, AR15, XR15: LINEAR and T_TILED;
   - NV12, NV21: LINEAR, SAND64, SAND128, SAND256;
   - P030: SAND128 only, no LINEAR;
   - every other format: LINEAR only (including XR30, AR30, AB30, XB30, which reach `default:`).
4. The hook ignores the plane, so every plane of a generation, primary, overlay and cursor, advertises the same list. read from source (`vc4_plane.c:2761-2765`); the recorded blobs agree (section 5).

## 3. Per-generation lists and counts

The table has 45 entries: 34 ungated, 5 `hvs5_only`, 6 `hvs6_only`. Counted from the table text; inferred (count), read from source (entries). `vc4_plane.c:41-315`

Table order, which is also the order of the blob's format array (inferred from the observed order, section 5): XR24 AR24 AB24 XB24 RX24 RA24 BX24 BA24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 P030 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12 RX12 RA12 P010 P012 P016 S010 S012 S016. (RG24 = RGB888, RGB8 = RGB332, BGR8 = BGR233, the four-character names are the usual fourccs; XR12 .. RA12 are the 4444 family.) read from source (entries and order)

### 3.1 BCM2711, HVS5 (`VC4_GEN_5`), pinned source

| Modifier row (blob order) | Formats | Pairs |
| --- | --- | --- |
| T_TILED (1) | XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 | 8 |
| SAND128 (4) | NV12 NV21 P030 | 3 |
| SAND64 (3) | NV12 NV21 | 2 |
| SAND256 (5) | NV12 NV21 | 2 |
| LINEAR (0) | all 39 advertised formats except P030 | 38 |
| **Total** | 39 distinct formats | **53** |

Per plane. inferred (count) from read-from-source rules. 39 = 45 - 6 `hvs6_only`; 38 = 39 - P030.

### 3.2 BCM2712, HVS6 (`VC4_GEN_6_C`/`_D`), pinned source

| Modifier row (blob order) | Formats | Pairs |
| --- | --- | --- |
| T_TILED (1) | none (rule 3, gen >= 6) | 0 |
| SAND128 (4) | NV12 NV21 P030 | 3 |
| SAND64 (3) | none | 0 |
| SAND256 (5) | NV12 NV21 | 2 |
| LINEAR (0) | all 45 table entries except P030 | 44 |
| **Total** | 45 distinct formats | **49** |

inferred (count) from read-from-source rules. 45 formats because gen 6 keeps the five `hvs5_only` entries and adds the six `hvs6_only` ones (39 + 6). The T_TILED and SAND64 rows are empty, so a blob built from the pinned source has two modifier rows besides LINEAR (SAND128, SAND256). 49 = 44 + 3 + 2.

### 3.3 The 6.12 tip and BCM2711

For gen 5 the 6.12 tip gives the same list as section 3.1 (it has no gen-6 rule to differ on): 39 formats, 53 pairs. read from source (tip), inferred (count). The running 6.12.47 kernel differs from the tip: see section 5.1.

### 3.4 Where HVS5 and HVS6 differ (all of it)

inferred from sections 3.1 and 3.2.

| Difference | HVS5 | HVS6 |
| --- | --- | --- |
| Distinct formats | 39 | 45 (+P010 P012 P016 S010 S012 S016) |
| T_TILED pairs (8 RGB formats) | advertised | refused (pinned source) |
| SAND64 pairs (NV12, NV21) | advertised | refused (pinned source) |
| LINEAR pairs | 38 | 44 |
| All pairs per plane | 53 | 49 |

Nothing else differs: SAND128 and SAND256 pairs, P030 under SAND128 only, and the four `*2101010` linear formats are the same on both.

## 4. `MAX_FORMATS`

- `MAX_FORMATS` is 32. `crates/composition-core/src/limits.rs:15`; it sizes `PlaneCaps::formats` (`crates/composition-core/src/caps.rs:84`). read from source (this repository)
- `DrmBackend::from_fd` parses the blob row by row, stable-sorts so that linear pairs come first, and keeps the first 32 (`pairs.sort_by_key(|p| p.modifier != Modifier::LINEAR)` then `take(MAX_FORMATS)`, `crates/composition-drm/src/lib.rs:531-533`). The profile runs afterwards on the already cut list (`crates/composition-drm/src/lib.rs:1300`). read from source (this repository)
- A test that restates the rule on recorded lists already exists: `crates/composition-drm/tests/format_cut_recorded.rs`. read from source (this repository)

Effect, per plane. inferred (count) from sections 3 and 4:

| Case | Pairs advertised | Kept (<= 32) | Cut | What is cut |
| --- | --- | --- | --- | --- |
| HVS5, pinned source (3.1) | 53 | 32 | 21 | 6 linear pairs (XB12 AB12 BX12 BA12 RX12 RA12) and all 15 tiled pairs |
| HVS6, pinned source (3.2) | 49 | 32 | 17 | 12 linear pairs (XB12 AB12 BX12 BA12 RX12 RA12 P010 P012 P016 S010 S012 S016) and the 5 SAND pairs |
| BCM2712 as recorded (5.2) | 59 | 32 | 27 | 12 linear pairs as above, plus all 15 non-linear pairs |
| BCM2711 as recorded (5.1) | 49 | 32 | 17 | 2 linear pairs (RX12 RA12) and all 15 non-linear pairs |

First row: 38 linear pairs, 32 kept, 6 linear cut (positions 33 to 38 of the pinned HVS5 order, which includes RX24 RA24 BX24 BA24), plus 15 non-linear pairs, is 21. inferred

Findings (inferred; none is changed here):

1. **No tiled or SAND pair ever reaches the planner on a vc4 board.** Every generation has at least 38 linear pairs, which fills the 32 slots, so T_TILED, SAND64, SAND128 and SAND256 are always cut. The planner cannot choose a tiled NV12 buffer for a vc4 plane whatever the kernel allows.
2. **The profile's HVS6 modifier filter is a no-op under the cut.** `rejected_on_hvs6` in `profile/vc4.rs` removes T_TILED and SAND64 pairs, but those pairs are already cut before `apply_vc4` runs, because they are non-linear. Its tests pass on hand-made caps; on a live backend there is nothing to remove. The filter would matter only if `MAX_FORMATS` were raised to at least 59 (the recorded BCM2712 count) or the cut were changed to keep tiled pairs. The decision is the owner's; the cap stays at 32.
3. **Formats lost from the linear list under the cut:** HVS6 loses the six `hvs6_only` formats P010, P012, P016, S010, S012, S016 (the 10 to 16-bit YCbCr formats) and six 4444 orders (XB12 AB12 BX12 BA12 RX12 RA12), so `Vc4Hvs6` cannot place a 10-bit or 16-bit video layer through the planner. HVS5 loses the same six 4444 orders and P030 (SAND128 only).
4. The 32 linear pairs kept are the same on both generations on the pinned source: XR24 .. AR12 in table order. A raise of `MAX_FORMATS` to 39 would show every HVS5 linear pair; 45 would show every HVS6 linear pair; 59 is needed for the full recorded BCM2712 list.

## 5. Comparison with the recorded blobs

### 5.1 BCM2711, `6.12.47+rpt-rpi-v8`, observed 2026-11-27

Recorded blob: 35 formats, five modifier rows (`crates/composition-drm/tests/data/bcm2711/props-2026-11-27.txt`; `docs/observed/bcm2711-cm4-topology.md`, 2026-11-28, lists the 32 probe-visible formats). observed 2026-11-27

| Quantity | Pinned source, gen 5 | Recorded | Match |
| --- | --- | --- | --- |
| Distinct formats | 39 | 35 | no: 4 fewer |
| Linear pairs | 38 | 34 (the pinned 38 without RX24 RA24 BX24 BA24) | no: the four RGBX/RGBA/BGRX/BGRA8888 formats are missing |
| T_TILED | 8 RGB formats | the same 8 | yes |
| SAND64 / SAND128 / SAND256 | NV12 NV21 / NV12 NV21 P030 / NV12 NV21 | the same | yes |
| All pairs | 53 | 49 | 4 fewer |

The probe output of the same board (`docs/observed/bcm2711-cm4-topology.md`, 2026-11-28) shows exactly 32 formats with no RX24 RA24 BX24 BA24, equal to the recorded linear list cut at 32 (XR24 .. BA12; RX12 RA12 are the two cut). observed 2026-11-28 (list), inferred (the cut)

The 6.12 tip table has the RGBX8888 family, so the running 6.12.47 kernel is older than the tip in this one respect (the cause is not read: inferred). The profile's claim "HVS5 keeps the KMS list, which matches the source" holds for the modifiers and for every format the board lists. It does not hold for the four missing linear formats, which the source offers and this board's kernel does not. observed 2026-11-27, read from source

### 5.2 BCM2712, `6.18.50+rpt-rpi-2712`, observed 2026-11-27 and 2026-11-28

Recorded blob: 45 formats, five modifier rows, identical on all five planes (`docs/observed/bcm2712-topology.md`, section IN_FORMATS, observed 2026-11-28; `crates/composition-drm/tests/data/bcm2712/props-2026-11-27.txt`, observed 2026-11-27).

| Quantity | Pinned source, gen 6 | Recorded | Match |
| --- | --- | --- | --- |
| Distinct formats | 45 | 45 | yes |
| Linear pairs | 44 | 44 (P030 not linear) | yes |
| T_TILED | none | the 8 RGB formats | **no** |
| SAND64 | none | NV12 NV21 | **no** |
| SAND128 | NV12 NV21 P030 | NV12 NV21 P030 | yes |
| SAND256 | NV12 NV21 | NV12 NV21 | yes |
| All pairs | 49 | 59 | 10 more (8 + 2) |

The recorded modifier code 3 is SAND64 and 4 is SAND128 (`docs/vc4-hvs-format-source.md` section 2 corrects the earlier reading in `docs/observed/bcm2712-topology.md`). The 6.12 tip hook, which has no gen-6 rule, would produce exactly the recorded 59 on a gen-6 plane: 45 formats, T_TILED on 8, SAND on NV12 NV21 and P030. inferred (comparison). So the recorded kernel behaves like a tree without rule 1; whether `6.18.50+rpt` predates it is not read. inferred

### 5.3 Against `profile.rs`

Claims in `crates/composition-drm/src/profile/vc4.rs`, "Modifiers" bullet. read from source (this repository)

| Claim | Evidence | Verdict |
| --- | --- | --- |
| HVS6: remove T_TILED and SAND64 pairs because `vc4_format_mod_supported` rejects them on gen 6 (`vc4_plane.c:2767-2771`) | The line range and the rule are as read (section 2, rule 3). The recorded Pi 5 blob lists both pairs (section 5.2) | Holds for the pinned source, not for the recorded kernel. Practical effect: none while the cap is 32 (finding 2). Not confirmed on the board: `TEST_ONLY` of a T-tiled layer has not been run there |
| HVS5 keeps the KMS list, which matches the source | Modifiers and format set match; four linear formats in the source are absent from the recorded kernel (section 5.1) | Holds, with the four-format gap noted. Nothing in the profile to change |
| The BCM2712 blob lists them (kernel `6.18.50+rpt-rpi-2712`) | Section 5.2, 8 + 2 pairs | Holds |

The profile's formats claims are otherwise limited to what it says it leaves to KMS. It has no table of formats of its own, so the counts of section 3 are checks of the blobs, not of the profile. inferred

## 6. What the board sessions can confirm

Expected from this read (inferred), for the sessions of the plan to confirm or refute:

- BCM2711 `TEST_ONLY` accepts a linear XR24 layer, a T_TILED XR24 layer, and NV12 under SAND64, SAND128 and SAND256 (`docs/vc4-hvs-format-source.md` section 4.1 for the gen-5 `mode_set` switch).
- BCM2712 `TEST_ONLY` refuses a T_TILED or SAND64 layer even though the blob lists them (the gen-6 `mode_set` switch has no case for either: `docs/vc4-hvs-format-source.md` section 4.1), and accepts SAND128 and SAND256 NV12.
- A raw `IN_FORMATS` count per plane of 49 (gen 5 as 6.12.47 reports it) and 59 (gen 6 as 6.18.50 reports it), matching sections 5.1 and 5.2. observed 2026-11-27 for these two numbers, as recorded.

## 7. Not read

- The rpi-6.12.y commit the 6.12.47 kernel was built from, so the missing RGBX8888 family is unexplained.
- The commit that added the gen-6 rule 1 to the 6.18 branch.
- The DRM core of the rpi tree (the blob builder is read in mainline only).
- Line numbers for the 6.12 tip.

Related: [HVS backend design](hvs-backend-design.md), [format and modifier acceptance, full read](vc4-hvs-format-source.md).
