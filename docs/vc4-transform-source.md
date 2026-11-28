# vc4 HVS5 / HVS6: rotation and reflection, read from source

Source evidence for the transform claims about `Vc4Hvs5` and `Vc4Hvs6` in
`crates/composition-drm/src/profile.rs` and `crates/composition-drm/src/profile/vc4.rs`.
It changes no code. Design context: [HVS backend design](hvs-backend-design.md) (§3 lists the
rotation row, §7 the open questions). Earlier source read of the same file:
[vc4-hvs-props-source.md](vc4-hvs-props-source.md) §5. No board was touched for this document
and it claims no hardware result.

## 1. Sources and tags

| Name | Meaning |
| --- | --- |
| pin 6.18 | `raspberrypi/linux`, branch `rpi-6.18.y`, commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e`, `drivers/gpu/drm/vc4/vc4_plane.c` |
| head 6.12 | `raspberrypi/linux`, branch `rpi-6.12.y`, commit `8f5f2e15840330091c7116b2ef9454108d4bc423` (shallow sparse clone of `drivers/gpu/drm/vc4`, 2026-11-28 UTC), `drivers/gpu/drm/vc4/vc4_plane.c`; lines below for head 6.12 were counted on that checkout |
| read from source | the quoted code was read in this task, in that file at that branch/commit |
| inferred | concluded from the code or the DRM core, not read as written, not run |
| observed | seen on a board; the date is that of the observed file named |

Limits of this read (stated, not hidden):

- The head 6.12 tree was cloned and read with line numbers. The 6.18 pin could not be cloned on
  this node, so its `vc4_plane.c` was read through a web reader that returns no line numbers.
  **Line numbers for pin 6.18 are carried from
  [vc4-hvs-props-source.md](vc4-hvs-props-source.md) §5 and §5.3, whose readers counted them
  against the same commit; the code quoted here was matched by content in this task, the line
  numbers were not re-counted.**
- Head 6.12 lines (read from source, `vc4_plane.c`): mask `2782-2786`; `drm_rotation_simplify`
  `1405-1408` (gen 5 `vc4_plane_mode_set`, starts `1352`), `1979-1982` (`vc6_plane_mode_set`,
  starts `1899`), `2438-2441` (`vc4_plane_async_set_fb`, starts `2425`); `REFLECT_Y` base move
  `1412`, `1986`, `2444`; HVS4 flip bits `1630-1631`; HVS5 `SCALER5_POS0_VFLIP`/`HFLIP`
  `1684-1689`; HVS6 `SCALER6_POS0_HFLIP` `2158`, `SCALER6_PTR0_VFLIP` `2199`; T-tiled
  `TILE_LINE_DIR` on `REFLECT_Y` `1470`; `vc4_format_mod_supported` `2655` (no generation test);
  async rotation copy `2509`. Head 6.12 does contain `vc6_plane_mode_set`, so the HVS6 column
  has a 6.12 counterpart for the flip code; only the gen-6 modifier gate is absent there.
- The DRM core (`drm_rotation_simplify`, property value validation,
  `drm_atomic_helper_check_plane_state`) was not read. Statements about it are `inferred`.
- Nothing here was run, so nothing here says what the hardware draws.

## 2. The rotation property

| Item | HVS5 (BCM2711, gen 5) | HVS6 (BCM2712, gen 6) | Basis | Reference |
| --- | --- | --- | --- | --- |
| Creation | `drm_plane_create_rotation_property(plane, DRM_MODE_ROTATE_0, ROTATE_0 \| ROTATE_180 \| REFLECT_X \| REFLECT_Y)` in `vc4_plane_init`, unconditional: no test on generation, plane type or modifier | same call, same mask | read from source | pin 6.18 `vc4_plane.c:2896-2900`; head 6.12 `vc4_plane_init` (same text) |
| Default | `ROTATE_0` | `ROTATE_0` | read from source | same |
| 90 / 270 | not in the mask, so not offered | same | read from source (mask); inferred (the core rejects values outside the mask) | same |
| Planes | primary, overlay and cursor alike | same | read from source | same |
| Difference between the two trees | none in the mask | none | read from source | pin 6.18 vs head 6.12 |

## 3. How each transform is programmed

Both generations of code first reduce the request with
`drm_rotation_simplify(state->rotation, ROTATE_0 | REFLECT_X | REFLECT_Y)`
(pin 6.18 `vc4_plane.c:1527-1530` gen 5, `:2106-2109` gen 6; head 6.12 `vc4_plane_mode_set`,
`vc6_plane_mode_set`). Because `ROTATE_180` is not in the second argument, the helper rewrites it
as `REFLECT_X | REFLECT_Y` (inferred: the helper is DRM core, not read). The driver therefore has
no rotate-180 code path of its own; rotate-180 and reflect-x plus reflect-y are the same
programming (inferred from that).

| Transform | HVS5 programming | HVS6 programming | Basis | Reference |
| --- | --- | --- | --- | --- |
| `ROTATE_0` | no flip bits | no flip bits | read from source | `vc4_plane_mode_set` / `vc6_plane_mode_set` |
| `REFLECT_X` | `SCALER5_POS0_HFLIP` in dlist position word 0 | `SCALER6_POS0_HFLIP` in position word 0 | read from source | pin 6.18 `:1799-1804` (gen 5), `:2271` (gen 6); same text in head 6.12 |
| `REFLECT_Y` | `SCALER5_POS0_VFLIP` in position word 0, plus the base pointer moves to the last source line (`src_y += height - 1`) | `SCALER6_PTR0_VFLIP` in pointer word 0, plus the same `src_y += height - 1` | read from source | pin 6.18 `:1532-1534`, `:1799-1804` (gen 5); `:2111-2113`, `:2314` (gen 6); same text in head 6.12 |
| `ROTATE_180` | = `REFLECT_X` + `REFLECT_Y` after simplify | same | inferred | rows above |
| `ROTATE_90`, `ROTATE_270` | not offered | not offered | read from source | §2 |
| Combinations (for example `ROTATE_180 \| REFLECT_X`) | the property is a bitmask over the offered bits, so any subset is a legal value; they reduce to X, Y or X+Y | same | inferred | DRM core not read |

No `-EINVAL` on rotation exists in `vc4_plane_mode_set`, `vc6_plane_mode_set` or
`vc4_plane_atomic_check` of either tree (read from source, absence; pin 6.18
`vc4_plane.c:1467-1790`, `:2028-2440`, `:2452-2494`). The only generic route to a rotation
refusal is `drm_atomic_helper_check_plane_state` in
`vc4_plane_setup_clipping_and_scaling`, which takes rotation into account when clipping and
scaling (read from source: the call; inferred: what it does with rotation).

## 4. Restrictions tied to a format, a modifier or scaling

| Case | HVS5 | HVS6 | Basis | Reference |
| --- | --- | --- | --- | --- |
| Linear, any flip | allowed. The pitch is never negated; the hardware walks backwards from the last line | allowed, same | read from source | `vc4_plane_mode_set` / `vc6_plane_mode_set`, `DRM_FORMAT_MOD_LINEAR` case |
| T-tiled (RGB formats only), `REFLECT_Y` | allowed. Extra code: `y_off = tile_h_mask - y_off` and `SCALER_PITCH0_TILE_LINE_DIR` so that lines after the first are fetched downwards. The source comment says the author does not know why the `y_off` inversion is needed, only that it is required | not applicable: `vc4_format_mod_supported` returns false for `T_TILED` on gen 6 (pin 6.18 only; head 6.12 has no gen test) and `vc6_plane_mode_set` refuses it | read from source | pin 6.18 `:1554`, `:1586-1597` (tile code); `:2767-2771`, `:2233-2239` (gen 6 refusal) |
| T-tiled, `REFLECT_X` | allowed through the generic `HFLIP` bit; no tile-specific code | not applicable | read from source (no code); inferred (that the hardware then reflects within tiles correctly) | same |
| SAND64 / SAND128 / SAND256, any flip | allowed by the driver. The SAND offset code never reads `rotation`; it uses the already shifted `src_y`. Whether a flipped SAND picture is right is not shown by the source | SAND128 and SAND256 only (SAND64 refused on gen 6 in the pin): same, generic flip bits only | read from source; inferred (hardware result unknown) | pin 6.18 `:1624-1716` (gen 5), `:2132-2230` (gen 6) |
| Scaling with a flip | no rule: the flip bits are independent of the scaler words in the same dlist | same | read from source (absence) | `vc4_plane_mode_set`, `vc6_plane_mode_set` |
| Vertical-skip decimation (pin 6.18 only: `vc4_plane_set_vdownsample`, a TPZ vertical downscale over 3x skips source lines) with a flip | decimation is forced off (`vdownsample = 1`) for T-tiled; for linear and SAND it is applied together with the flipped base pointer and no rotation test | same in the SAND and linear cases | read from source (no test); inferred (interaction untried) | pin 6.18 T-tiled case, linear/SAND pitch words. Head 6.12 has no decimation |
| Cursor planes | same property and mask | same | read from source | §2 |

Consequence (inferred): on both generations the driver itself refuses no flip for any format
or modifier that the plane accepts at all; the only refusals are the modifier rules in
`vc4_format_mod_supported`, which are independent of rotation. Any limit on flipped SAND,
flipped T-tiled or flip plus decimation would be a hardware behaviour the source does not state.

### 4.1 Differences between pin 6.18 and head 6.12 that touch rotation

| Item | pin 6.18 | head 6.12 | Basis |
| --- | --- | --- | --- |
| Async (cursor-path) framebuffer swap, `vc4_plane_async_set_fb` | takes the address from `fb->offsets[0] + vc4_state->ptr0_src_offset`; the offset saved at mode-set time already includes the `REFLECT_Y` move | adds the `REFLECT_Y` move itself: `dma_addr += pitches[0] * ((src_h >> 16) - 1)` | read from source |
| Async update of the rotation | `vc4_plane_atomic_async_update` copies `new_plane_state->rotation` into the live state; `vc4_plane_atomic_async_check` accepts only a change of position word 0, position word 2 and pointer word 0, anything else returns `-EINVAL` so the core falls back to a synchronous update | the same; no `flip` argument on the check | read from source |
| Source height and width | from the fractional span (`vc4_src_span`) | from `src_h >> 16`, `src_w >> 16` | read from source |
| `vc4_format_mod_supported` on gen 6 | refuses `T_TILED` and SAND64 | no generation test | read from source |

The CM4 runs a 6.12 kernel (`6.12.47+rpt-rpi-v8`, per the observed files below), so head 6.12
is the nearer source for the HVS5 column; the Pi 5 runs 6.18 (`6.18.50+rpt-rpi-2712`), so pin
6.18 is nearer for the HVS6 column. Whether those running kernels carry the same `vc4_plane.c` as the two
trees read here is the question of HVS Q7 and is not answered by this file.

## 5. Table per generation

### 5.1 HVS5, BCM2711 (reTerminal CM4)

| Transform | Offered by the property | Restriction in source | Tag |
| --- | --- | --- | --- |
| rotate-0 | yes | none | read from source |
| rotate-180 | yes | none; programmed as X+Y flip | read from source; inferred (via simplify) |
| reflect-x | yes | none; hardware `HFLIP`. T-tiled and SAND have no tile-specific X handling | read from source |
| reflect-y | yes | none refused; last-line base pointer; T-tiled has its own `TILE_LINE_DIR` handling; SAND has none | read from source |
| rotate-90, rotate-270 | no | not in the mask | read from source |

### 5.2 HVS6, BCM2712 (Raspberry Pi 5)

| Transform | Offered by the property | Restriction in source | Tag |
| --- | --- | --- | --- |
| rotate-0 | yes | none | read from source |
| rotate-180 | yes | none; programmed as X+Y flip | read from source; inferred (via simplify) |
| reflect-x | yes | none; `HFLIP` in position word 0 | read from source |
| reflect-y | yes | none refused; last-line base pointer and `VFLIP` in pointer word 0. The source marks the pointer-word handling "only covers Raster Scan Order planes" (pin 6.18 `:2300-2302`) | read from source |
| rotate-90, rotate-270 | no | not in the mask | read from source |
| flips on T-tiled or SAND64 buffers | not reachable | these modifiers are not accepted on gen 6 in pin 6.18 | read from source |

## 6. Comparison with `profile.rs` and the observed files

| Claim or value | Where | Source says | Board record | Verdict |
| --- | --- | --- | --- | --- |
| `Vc4Hvs5` / `Vc4Hvs6` set no `transforms`: "rotation ... come from KMS; this table only adds what KMS does not carry" | `profile/vc4.rs` module doc, lines 13-15 | the property offers 0, 180, reflect-x, reflect-y on every plane (§2) | see next rows | consistent: nothing to add if KMS is read correctly |
| `transforms` in caps is `0x0` on both boards | `docs/observed/bcm2712.md` (probe, row "Rotation"), `docs/observed/bcm2711-cm4.md` (same row) | the property exists with a non-empty mask | observed 2026-11-28: the engine view printed `transforms 0x0`; the observed files say composition-drm "does not read or program `rotation`" | the empty mask is a reader gap, not a driver fact. The planner refuses `Rotate180` with `NoPlane{TRANSFORM}` (observed 2026-11-28, `bcm2712.md` and `bcm2711-cm4.md`), stricter than the kernel |
| Supported mask of the kernel property on BCM2712 | `docs/observed/bcm2712-topology.md` row `rotation` | 0, 180, reflect-x, reflect-y | observed 2026-11-28: rotate-0, rotate-180, reflect-x, reflect-y, no 90/270; idle value rotate-0 (bit 0). The enum values reported are bit positions (0, 2, 4, 5) | matches the source |
| Supported mask on BCM2711 | `docs/observed/bcm2711-cm4-topology.md`, `bcm2711-cm4.md` | same mask expected | observed 2026-11-28: current value only (`rotation=1`, rotate-0, in the debugfs state of all planes); the supported bits were not read | **not confirmed on BCM2711**: source expectation only |
| `docs/hvs-backend-design.md` §3 row: rotation 0/180, reflect X/Y, no 90/270, mask must exclude 90/270 | design doc | the driver mask already excludes them | observed 2026-11-28 (BCM2712) | consistent |
| Profile module doc, cursor planes: scaling stays as KMS reported | `profile/vc4.rs` | not about transforms; cursors also get the full rotation mask | none | no conflict |
| STM32 note in `profile.rs`: "Transforms ... left as KMS reports them" | `profile.rs` `Stm32Ltdc` doc | not an HVS claim | none | out of scope here |

Summary for the profile (inferred): no value in `profile.rs` or `profile/vc4.rs` contradicts the
source. The profile makes no transform claim for HVS5 or HVS6, so there is nothing to tighten
from source alone. What the source supports adding, if composition-drm reads the `rotation`
property: `ROTATE_0 | ROTATE_180 | REFLECT_X | REFLECT_Y` on every plane of both generations,
with no per-modifier mask (the driver refuses nothing by modifier). A conservative option the
source leaves open: refuse reflection on SAND and T-tiled buffers, and on a plane that also uses
decimation, until a board run shows a correct picture. That choice belongs to the profile
item, not to this file.

## 7. TEST_ONLY checks for a board session

All run on an idle display, atomic `TEST_ONLY` only unless stated, then removed. They would give
board-measured rows; none has been run.

1. Read the full `rotation` property (enum names and bit positions, current value) of every
   plane on the reTerminal (BCM2711), which is the one gap in §6. Expected: rotate-0,
   rotate-180, reflect-x, reflect-y. A mask with 90/270 refutes §2.
2. For one primary, one overlay and one cursor plane on each board: `TEST_ONLY` with each of
   rotate-180, reflect-x, reflect-y, and reflect-x plus reflect-y, on a linear XR24 buffer.
   Expected: all accepted.
3. `rotate-90` and `rotate-270`: expected refusal by the core (value outside the mask).
4. Reflect-y and reflect-x on NV12 linear, then on NV12 with SAND128 (both boards) and SAND64
   (HVS5 only; HVS6 should not list it). Expected: accepted by the driver. Accepted does not
   show a correct picture; use a picture check on the Pi 5 (which has a KVM) for one case.
5. Reflect-y on a T-tiled XR24 buffer on BCM2711 only. On BCM2712, confirm that no T-tiled
   modifier is listed for any format (the profile removes such pairs).
6. Reflect-y combined with a vertical downscale beyond 3x and with the largest upscale tried,
   on each board. Expected: accepted; a refusal would be a scaler or LBM refusal and should be
   attributed by repeating without the flip.
7. Reflect-y on a plane that is also a cursor-sized overlay with an async update: skip unless a
   cursor path is under test; the async rules in §4.1 differ between 6.12 and 6.18.
8. Record `uname -r` and the board's `vc4_plane.c`-relevant package version beside each result,
   so that HVS Q7 can tell whether pin 6.18 or head 6.12 applies.

## 8. Not covered

- The hardware reference manuals of the HVS: whether a flipped SAND or T-tiled fetch is correct
  is not in the source read here.
- The DRM core's handling of the property (`inferred` above).
- Other files of `drivers/gpu/drm/vc4` (for example `vc4_hvs.c`, `vc4_crtc.c`) were not read
  for this document.
