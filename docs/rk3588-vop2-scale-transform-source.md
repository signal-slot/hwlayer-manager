# RK3588 VOP2: scale limits and rotation/reflection per window class (mainline v6.18 source)

Source evidence for the `ScaleLimits` and transform claims of
`DriverProfile::Rk3588Vop2` in `crates/composition-drm/src/profile/rk3588.rs`
(read, not changed). Design context: [rk3588-backend-design.md](rk3588-backend-design.md)
§4 and §7. Written 2026-11-28. Source reading only: no board was touched, and
nothing here is a hardware result. The board sessions of the same plan measure
these rows; where a row says "to measure" the source is only the expectation.

## Source and tags

- Tree: mainline Linux `v6.18`, commit `7d0a66e4bb9081d75c82ec4957c50034cb0ea449`
  (read from source: the pin `docs/rk3588-vop2-limits-source.md` recorded).
- Files, under `drivers/gpu/drm/rockchip/`: `rockchip_drm_vop2.c` (2793 lines,
  `V2C` below) and `rockchip_vop2_reg.c` (2577 lines, `REG`). Line numbers are
  counted on the copies in the project's dependency cache.
- Provenance (read from source, 2026-11-28): both files were also fetched
  fresh from the `v6.18` tag (raw per-file download) and are byte-identical
  (`cmp`) to the cached copies the line numbers were counted on. The tag
  object and the peeled commit were not re-resolved, so the commit hash above
  is still the recorded pin, not a new check (inferred).
- Tags: `read from source` (file:line) or `inferred`, all dated 2026-11-28.
  The running board kernel is `6.18.55-current-rockchip64`; whether its
  VOP2 code equals v6.18 is RK Q2 and not decided here (inferred).

## 1. Scale check in `atomic_check`

| Statement | Where | Tag |
|---|---|---|
| `vop2_plane_atomic_check` sets `min_scale = FRAC_16_16(1, 8)` and `max_scale = FRAC_16_16(8, 1)` as locals and passes them to `drm_atomic_helper_check_plane_state(pstate, cstate, min_scale, max_scale, true, true)` (`can_position`, `can_update_disabled` both true) | V2C:1004-1005, 1020-1022 | read from source, 2026-11-28 |
| The bounds do not depend on the window, its class, the format, the modifier or the rotation; the function has no branch on `WIN_FEATURE_CLUSTER` or `WIN_FEATURE_AFBDC` | V2C:992-1062 | read from source, 2026-11-28 |
| A scale outside 1/8..8 on either axis makes the helper fail the commit (the generic helper returns `-ERANGE`; the helper is in `drm_atomic_helper.c`, not read here) | V2C:1020-1024 | read from source for the call; the errno is inferred, 2026-11-28 |
| Source rect smaller than 4x4 or destination smaller than 4x4: plane made invisible, return 0 (no error) | V2C:1032-1040 | read from source, 2026-11-28 |
| Source larger than `max_input` (4096 x 4320 on RK3588, one value for all windows) returns `-EINVAL` | V2C:1042-1050; REG:2525 | read from source, 2026-11-28 |
| YUV format with odd `src.x1` returns `-EINVAL` | V2C:1052-1059 | read from source, 2026-11-28 |
| `max_upscale_factor` / `max_downscale_factor` per window are filled in the RK3588 table but read by no line of `rockchip_drm_vop2.c` | REG:1136-1137, 1155-1157, 1174-1176, 1193-1195 (Cluster); REG:1212-1213, 1229-1230, 1246-1247, 1263-1264 (Esmart); no reader (the earlier source doc found only the declarations) | read from source, 2026-11-28 |
| With rotation 90/270 the helper applies the rotation to the source rectangle before the scale test, so the scale is computed on the rotated rectangle (generic helper behaviour) | helper in `drm_atomic_helper.c`, not read here | inferred, 2026-11-28 |

## 2. Register-side scaling (what the scaler does with the factor)

| Statement | Where | Tag |
|---|---|---|
| Vertical pre-decimation: `gt4` when `src_h >= 4*dst_h` (height divided by 4), else `gt2` when `src_h >= 2*dst_h` (divided by 2), before the scaler factor is computed; written for every window including Cluster | V2C:573-581, 613-614 | read from source, 2026-11-28 |
| Only the vertical axis has pre-decimation; the horizontal factor goes straight into `vop2_scale_factor` | V2C:560-612 | read from source, 2026-11-28 |
| Cluster returns right after the YRGB mode registers: no filter-mode and no chroma (CBCR) scale registers are written (`if (vop2_cluster_window(win)) return;`) | V2C:619-620 | read from source, 2026-11-28 |
| Esmart additionally writes the YRGB filter modes and, for YUV formats, the chroma factors | V2C:623-657 | read from source, 2026-11-28 |
| Even-width workaround: for a non-`AFBDC` window (Esmart) that scales down horizontally, an odd `dst_w` is incremented by one | V2C:595-606 | read from source, 2026-11-28 |
| Esmart workaround: `actual_w > dsp_w && (actual_w & 0xf) == 1` shortens the source width by 1 ("can't support scale down when actual_w % 16 == 1") | V2C:1235-1243 | read from source, 2026-11-28 |
| AFBC planes: source width rounded down to a multiple of 4, silently | V2C:1245-1249 | read from source, 2026-11-28 |
| Whether the scaler really achieves 1/8 or 8x on Cluster (table says 4) is not stated by the source | REG:1136-1137 vs V2C:1004-1005 | inferred, 2026-11-28; to measure |

## 3. Rotation and reflection

| Statement | Where | Tag |
|---|---|---|
| The rotation property is created only if the window's `supported_rotations` is non-zero: `drm_plane_create_rotation_property(&win->base, DRM_MODE_ROTATE_0, DRM_MODE_ROTATE_0 \| win->data->supported_rotations)`; ROTATE_0 is always in the mask and is the default | V2C:2295-2298 | read from source, 2026-11-28 |
| Cluster0..3 mask: `ROTATE_90 \| ROTATE_270 \| REFLECT_X \| REFLECT_Y` (so with ROTATE_0 the property can express rotate 0/90/180/270 combined with reflect X/Y) | REG:1131-1132, 1150-1151, 1169-1170, 1188-1189 | read from source, 2026-11-28 |
| Esmart0..3 mask: `REFLECT_Y` only (so rotate-0 plus reflect-Y; no 90/180/270, no reflect X) | REG:1207, 1224, 1241, 1258 | read from source, 2026-11-28 |
| 180 degrees is not listed separately; for Cluster it is the combination reflect X plus reflect Y, which the mask allows. For Esmart 180 is not expressible | REG:1131-1132 vs REG:1207 | inferred, 2026-11-28 |
| Cluster registers for rotate/mirror live in the AFBC rotate-mode register (`ROTATE_90` bit 0, `ROTATE_270` bit 1, `XMIRROR` bit 2, `YMIRROR` bit 3); Esmart has `YMIRROR` in `SMART_CTRL1` bit 31 and `XMIRROR`/`AFBC_ROTATE_*` are unused (`0xffffffff`) | REG:331-334 (cluster); REG:367, 393, 410-411 (smart) | read from source, 2026-11-28 |
| Cluster rotate 90/270 switches the line-buffer mode to 2 (any other rotation: 0) | V2C:520-528, 1397-1399 | read from source, 2026-11-28 |
| Cluster rotate 90/270 turns the AFBC half-block mode off (16-line tiles); with 0/180 and reflect it stays on | V2C:428-434 | read from source, 2026-11-28 |
| With rotate 90/270 the active width and height are swapped for the window registers | V2C:1363-1367 | read from source, 2026-11-28 |
| X mirror is written only in the AFBC path (`VOP2_WIN_XMIRROR` at V2C:1349, inside the `afbc_en` branch); Y mirror is written for every plane (V2C:1361) | V2C:1349, 1361 | read from source, 2026-11-28 |
| Reflect Y on a non-AFBC plane is done in the buffer address (start from the last line), on AFBC it is left to the transform offset | V2C:1183-1201, 436-503 | read from source, 2026-11-28 |
| No rotation or reflection check in `atomic_check` rejects a combination with scaling, YUV, a format or a size: the function has no use of `pstate->rotation` | V2C:992-1062 | read from source, 2026-11-28 |
| Rotation on a linear (non-AFBC) buffer of a Cluster window: Cluster has no linear format on this SoC, so rotation applies to AFBC buffers (every Cluster buffer is AFBC) | REG:1121-1203 modifier list; V2C:1349-1351 | inferred from the Cluster format list, 2026-11-28 |
| The vendor 6.1 rule "rotation only with AFBC" is not in this tree; the source has no gate of that kind | V2C whole file | read from source, 2026-11-28 |
| Whether rotate 90/270 works with a 4096-wide source, or with scaling, on Cluster is not stated by the source | V2C:514-528 | inferred; to measure |

## 4. Per-class table

Both Cluster and Esmart rows are the same on all four windows of the class
(REG lines in the sections above). "Table" is the unread per-window figure.

| Class | Up (kernel check) | Down (kernel check) | Table up/down | Rotation mask (plus ROTATE_0) | Restrictions found |
|---|---|---|---|---|---|
| Cluster0-3 (AFBC, `WIN_FEATURE_CLUSTER`) | 8x per axis | 8x per axis (1/8), vertical pre-decimation helps beyond 4x | 4 / 4 (unused) | ROTATE_90, ROTATE_270, REFLECT_X, REFLECT_Y | source width rounded down to a multiple of 4 (AFBC); rotation 90/270 uses lb mode 2 and 16-line tiles; no filter or chroma scale registers; no linear format |
| Esmart0-3 | 8x per axis | 8x per axis (1/8) | 8 / 8 (unused) | REFLECT_Y | odd `dst_w` bumped when scaling down horizontally; `actual_w % 16 == 1` workaround when scaling down; YUV needs even `src.x1`; no rotate, no reflect X |
| All windows | | | | | source at most 4096x4320; 4x4 minimum (silently invisible below); YUV origin even |

## 5. Comparison with the profile

Profile values (`rk3588.rs`, `Window::limits`): `max_upscale = 8 << 16`,
`max_downscale = 8 << 16`, same for both classes; `max_source` 4096x4320 for both;
transforms not changed ("none reported"). Stricter means the profile refuses
what the kernel check would accept; looser means the profile allows what the
kernel (or, for the hardware rows, the table) refuses.

| Row | Source value | Profile value | Verdict | Tag |
|---|---|---|---|---|
| Cluster up | check 8x (V2C:1005); table 4x (REG:1136) | 8x | consistent with the kernel check; looser than the table | read from source, 2026-11-28 |
| Cluster down | check 1/8 (V2C:1004); table 4x | 8x | consistent with the check; looser than the table | read from source, 2026-11-28 |
| Esmart up | check 8x; table 8x (REG:1212) | 8x | consistent | read from source, 2026-11-28 |
| Esmart down | check 1/8; table 8x | 8x | consistent | read from source, 2026-11-28 |
| `max_source` both | 4096x4320 (REG:2525, V2C:1042-1050) | 4096x4320 | consistent | read from source, 2026-11-28 |
| Cluster rotation 90/270, reflect X/Y | offered (REG:1131-1132) | not claimed (transforms left as KMS reported) | stricter (profile claims nothing) | read from source, 2026-11-28 |
| Esmart reflect Y | offered (REG:1207) | not claimed | stricter | read from source, 2026-11-28 |
| Esmart rotate / reflect X | not offered | not claimed | consistent | read from source, 2026-11-28 |
| Cluster scale beyond 4x | table says 4; check accepts 8 | accepted as 8x | looser than the table only; the kernel will not refuse it (the profile text already says picture quality is not modelled) | inferred, 2026-11-28 |
| Cluster width with AFBC | rounded down to a multiple of 4 silently | not modelled | not a refusal, left to the kernel (matches the profile's "not modelled" list) | read from source, 2026-11-28 |

Reading (inferred, 2026-11-28): no row makes the profile looser than the
kernel's own check, so a profile-accepted scale is not refused by
`atomic_check`. The one row looser than a hardware figure is Cluster beyond 4x,
which is a quality question, not a refusal. The transform claim is the one
place where source offers more than the profile uses: the kernel offers the
masks above, the profile reports none.

## 6. What the board sessions must confirm or refute

Expected answers (inferred from the source above, 2026-11-28); nothing here
is measured:

- The `rotation` property of each Cluster plane lists rotate 0/90/180/270 and
  reflect X and reflect Y in its supported bits; each Esmart plane lists
  rotate-0 and reflect-Y only. Needs the property's enum/bitmask values,
  not only the current value (the 2026-11-27 recording kept only the current
  value, `docs/rk3588-backend-design.md` §7).
- `TEST_ONLY` accepts a Cluster plane at 8x up and 8x down per axis and refuses
  slightly beyond 8x, as the 2026-11-27 sweep did for up; down below 1/2 up to
  1/8 is not yet measured (source expects accept; the profile's 8x down rests on
  source only).
- Rotation with scaling and with a 4096-wide source on Cluster, and any
  reflect on an Esmart YUV (NV12) plane: the source has no check, so
  `TEST_ONLY` should accept; a refusal would be a hardware or driver
  restriction this read missed.
- Picture quality of Cluster beyond 4x and of rotated planes needs the KVM
  camera; `TEST_ONLY` cannot judge it.

## 7. Not covered

Device tree and per-VP reach, zpos, formats and modifier order, the flip
path: other items of the plan. The generic helper
`drm_atomic_helper_check_plane_state` and `drm_plane_create_rotation_property`
were not read (inferred behaviour above).
