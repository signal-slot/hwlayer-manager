# RK3588 VOP2: window-to-video-port assignment and eight simultaneous windows (mainline v6.18 source)

Written 2026-11-28 for question Q4 of `docs/rk3588-backend-design.md` §7:
can eight windows be shown at once. Source reading only: no board was touched
and nothing here is a hardware result. The `TEST_ONLY` sweep of the
`t6-commit-session` board item is the measurement; this note says what the
code predicts and which configurations let the sweep tell the cases apart.

Related reads of the same tree: `docs/rk3588-mainline-source.md`,
`docs/rk3588-dt-source.md`, `docs/rk3588-vop2-limits-source.md`,
`docs/rk3588-vop2-scale-source.md`.

## Source pin

- Tree: `torvalds/linux`, tag `v6.18`, commit
  `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` (read from source: `git describe
  --tags` of the checkout printed `v6.18` on 2026-11-28, host UTC).
- Files: `drivers/gpu/drm/rockchip/rockchip_drm_vop2.c` (cited `V2C:line`) and
  `drivers/gpu/drm/rockchip/rockchip_vop2_reg.c` (cited `V2R:line`).
- These two files were read in full for the cited parts. From the DRM core
  only the zpos normalisation was read (`drm_blend.c`, `drm_atomic_helper.c`,
  `rockchip_drm_fb.c`); other statements about the core are tagged inferred.
- The board's running kernel is not necessarily v6.18 (question Q2, see
  `docs/vc4-rk-kernel-compare.md` when it lands). Everything below is the
  mainline expectation.

## 1. Assignment rules

| # | Rule | Tag |
|---|---|---|
| A1 | The RK3588 has 4 video ports (`.nr_vps = 4`) and a window table of 8 entries (`.win_size = ARRAY_SIZE(rk3588_vop_win_data)`). The table lists Cluster0..3 then Esmart0..3. There are no Cluster sub-windows ("win1") and no Esmart extra regions in the table: one entry is one plane | read from source (V2R:1121-1262, V2R:2524-2529) |
| A2 | Every window has `possible_vp_mask = BIT(0)\|BIT(1)\|BIT(2)\|BIT(3)`, so the hardware table lets any window reach any port. This differs from RK3568 and RK3576, where masks are partial | read from source (V2R:1126, 1145, 1164, 1183, 1201, 1218, 1236, 1252) |
| A3 | Window types: the four Clusters are `DRM_PLANE_TYPE_PRIMARY`, the four Esmarts are `DRM_PLANE_TYPE_OVERLAY` | read from source (V2R:1139, 1152, 1171, 1190, 1208, 1225, 1242, 1259) |
| A4 | A video port becomes a CRTC only if the device tree gives it a remote endpoint (`of_graph_get_remote_node`, otherwise `continue`). `nvps` counts those ports | read from source (V2C:2368-2374) |
| A5 | Primary choice per CRTC: "Register a primary plane for every crtc". For each CRTC (in port order), the loop takes the first window in table order that is not yet registered as primary, whose mask has this port, and whose `win->type` is PRIMARY. It then registers it with `possible_crtcs = BIT(nvp)`, where `nvp` is the index among registered CRTCs, not the hardware port id. The loop breaks after one hit. With four active ports the order is Cluster0 to the first CRTC, Cluster1 to the second, and so on | read from source (V2C:2378-2414, `possible_crtcs = BIT(nvp)` at V2C:2400) |
| A6 | A CRTC with no primary candidate fails the probe: `"no primary plane for vp %d"` | read from source (V2C:2411-2414) |
| A7 | The "already registered" test is `win->base.type == DRM_PLANE_TYPE_PRIMARY`. An unregistered window has a zeroed `base`, so its type reads as 0, which is OVERLAY in the uapi enum; the test therefore only matches windows registered earlier in the loop | inferred (enum values are in `include/uapi/drm/drm_mode.h`, not read) |
| A8 | Overlay choice: every window that is not a primary is forced to `DRM_PLANE_TYPE_OVERLAY`, and `possible_crtcs` has one bit for every registered CRTC whose port is in the window's mask. On RK3588 that is all CRTCs. So the four Esmarts and the Clusters not chosen as primaries can each be used on any CRTC | read from source (V2C:2417-2445) |
| A9 | Consequence of A5 and A8 with fewer than four active ports: Clusters beyond the number of active ports are Overlay planes. With one active port: Cluster0 primary; Cluster1..3 and Esmart0..3 overlay. With two: Cluster0 and Cluster1 primary | inferred (from A3, A5, A8) |
| A10 | zpos is created on every plane as a mutable property with initial value `win_id` (table index 0..7) and range `0..registered_num_wins-1`, i.e. 0..7. The range is not per CRTC | read from source (V2C:2301-2302) |
| A11 | `vp->nlayers = win_size / nvps` for each used port: 8 with one port, 4 with two, 2 with three, 2 with four (integer division) | read from source (V2C:2483) |
| A12 | Layer mixer: the per-CRTC layer slot is `normalized_zpos + ofs`, where `ofs` is the sum of `nlayers` of the lower-id ports. `RK3588` uses the RK3568 overlay code (`.setup_overlay = rk3568_vop2_setup_overlay`) | read from source (V2R:2161-2165, V2R:2457-2461) |
| A13 | Port mux: the layer count of each of the ports 0..2 is written into `PORT0_MUX`, `PORT1_MUX`, `PORT2_MUX` as a running total minus one (or 8 for a port with no layers). Port 3 is written as the fixed value 7 for the RK3588 ("Fixed value for rk3588") | read from source (V2R:2123-2152) |
| A14 | The window-to-port binding is written per window (`OVL_PORT_SEL` field for Cluster0..3, Esmart0..3) with the id of the port that uses it. All ports share `OVL_LAYER_SEL` and `OVL_PORT_SEL`; the code waits for the port mux and layer config to be latched when a window migrates or zpos changes | read from source (V2R:2170-2260, comment at V2R:2243-2255) |
| A15 | The `normalized_zpos` the mixer reads is produced by the DRM core: `drm_atomic_helper_check` calls `drm_atomic_normalize_zpos` when `mode_config.normalize_zpos` is set, and the Rockchip driver sets it. The core sorts the planes of a CRTC by zpos, breaks ties by plane object id, and numbers them 0..n-1, so equal raw zpos values are not rejected on this path | read from source (`rockchip_drm_fb.c:84`, `drm_atomic_helper.c:1115-1116`, `drm_blend.c:437-446`) |
| A16 | A window active on one CRTC and used on another in the next commit is a migration (A14). Whether a single atomic commit may move a window and use it on a second CRTC in the same commit is not decided by either file | inferred |

## 2. Paths that reject a configuration

All statements are about atomic check. The sweep sees them as `EINVAL` (or
another errno) from a `TEST_ONLY` commit.

| # | Reject path | Tag |
|---|---|---|
| R1 | Per-CRTC layer count: `vop2_crtc_atomic_check` counts the planes in the CRTC state and returns `-EINVAL` when `nplanes > vp->nlayers`. The count includes the primary. This is the only per-VP layer-count limit in the driver | read from source (V2C:1816-1834) |
| R2 | Scaling: `min_scale = 1/8`, `max_scale = 8` are fixed constants in `vop2_plane_atomic_check`, passed to `drm_atomic_helper_check_plane_state` with `can_position = true`, `can_update_disabled = true`. The per-window `max_upscale_factor` and `max_downscale_factor` fields in the table (4 for Clusters, 8 for Esmarts) are not referenced by `rockchip_drm_vop2.c`, so they do not reject anything in this file | read from source (V2C:1004-1022; the table fields at V2R:1136-1137, 1155-1156; absence of use by search of V2C) |
| R3 | Size: a visible plane with a source or destination edge under 4 pixels is set invisible and passes (`return 0`) after logging an error; it is not rejected | read from source (V2C:1034-1041) |
| R4 | Source size: source width or height above `max_input` (4096 x 4320 for the RK3588) returns `-EINVAL` | read from source (V2C:1042-1053, V2R:2525) |
| R5 | YUV: a YUV framebuffer with an odd source x returns `-EINVAL` | read from source (V2C:1055-1058) |
| R6 | Format: an unsupported fourcc returns the negative result of `vop2_convert_format` | read from source (V2C:1030-1032) |
| R7 | Modifier and format pair, checked when the framebuffer is created (`format_mod_supported`), not in atomic check: `DRM_FORMAT_MOD_INVALID` is refused; `XRGB2101010` and `XBGR2101010` on the RK3588 need an AFBC modifier; any non-linear modifier must be in the plane's list and its format must convert to an AFBC format. On RK3568 only, a Cluster refuses linear | read from source (V2C:380-422) |
| R8 | AFBC limits seen in the plane update path are not rejections: a width not a multiple of 4 is aligned down, and a stride not a multiple of 64 with rotation or mirror only logs a debug line. Clusters take AFBC (`format_modifiers_afbc`); Esmarts list linear only (`format_modifiers`) | read from source (V2C:1245-1249, 1290-1296; V2R:198-210, V2R:1128 and 1202 for the per-window assignment) |
| R9 | Rotation: the supported set comes from `supported_rotations` per window (Cluster: 90, 270, reflect X and Y; Esmart: reflect Y only). The core rejects an unsupported value through the property range, not this driver | read from source (V2R:1131-1132, 1207); the rejection by the core is inferred |
| R10 | Gamma: `vop2_crtc_atomic_check_gamma` can reject (not read in detail here; it concerns the CRTC gamma LUT, not windows) | read from source (V2C:1789, 1825) |
| R11 | Not present: no bandwidth or memory-bus check, no line-buffer check, no check of how many Cluster or Esmart windows are used, and no check of AFBC window count appears in atomic check of either file. A mode's pixel clock is handled in CRTC enable (dclk selection against `VOP2_MAX_DCLK_RATE`), not in the plane check | read from source (absence by reading V2C:992-1062, 1816-1834, and a search for `bandwidth`, which has no match; dclk at V2C:104, 1740) |
| R12 | A hardware limit can still make the picture wrong with a commit that passes check (underflow). The kernel does not detect it at check time, so a `TEST_ONLY` success is not proof of a good picture | inferred (from R11) |

## 3. What the code implies for eight windows

- **All eight on one CRTC.** Possible only if exactly one port is active:
  `nlayers = 8 / 1 = 8` (A11), so R1 passes with 8 planes. All eight windows
  are bindable to port 0 (A2, A8). With two active ports `nlayers` is 4, so a
  fifth plane on one CRTC is rejected by R1, even if the other CRTC is idle
  (read from source: A11, R1). inferred: a board that exposes two CRTCs
  while one is idle may still count it in `nvps` if the idle port has a DT
  endpoint (A4). The design and observation docs record that the recorded
  board exposes two CRTCs, so the expected limit there is 4 per CRTC.
- **Spread over CRTCs.** With `n` active ports and `nlayers = 8 / n`, the
  total across CRTCs is at most `n * (8 / n)`: 8 for n = 1, 2 or 4; 6 for
  n = 3 (inferred, arithmetic from A11). Eight windows on two CRTCs need four
  per CRTC. This needs a second active CRTC in the commit, which needs a
  connected output or a way to light a CRTC in `TEST_ONLY`; the recorded board
  has one connected output, so this may not be testable.
- **Primary constraint.** Each CRTC's primary plane is a Cluster (A5). A CRTC
  whose state has no primary is not rejected by R1 (inferred: R1 counts only
  planes in the CRTC state; the core's primary-plane check is not read).
- **zpos.** zpos values live in 0..7 on every plane (A10). On a CRTC with
  `nlayers = 4` the normalized zpos of its four planes is 0..3, and the mixer
  adds `ofs` (A12). The driver does not reject two equal raw zpos values in
  files read, and the core breaks ties by plane id (A15), so equal values
  should pass check (read from source); the rest of Q6 is the subject of
  `docs/rk3588-vop2-zpos-source.md` when it lands.
- **Expected answer for Q4 (to confirm or refute).** inferred: eight
  simultaneous windows pass `TEST_ONLY` only on a single active CRTC. On a
  two-CRTC configuration the single-CRTC maximum is 4. The kernel has no
  bandwidth gate (R11), so any failure below the layer count would come from
  format, scaling, size or modifier rules (R2 to R7), and a pass says
  nothing about underflow (R12).

## 4. Suggested sweep matrix for `t6-commit-session`

Run with the existing `kms_sweep` example (`TEST_ONLY` only). Record, for
each row, the errno, the number of CRTCs the kernel exposes, and which CRTCs
have a connected output. A row that cannot be built on the board is recorded
as not run, not as a failure.

| ID | Configuration | What it separates |
|---|---|---|
| S1 | 1 plane (the primary) on CRTC A, then add planes one at a time up to 8, same CRTC, all windows full-screen XRGB8888 linear, in plane-id order | The per-CRTC cap: the count at which R1 first fails. Expected 4 if two CRTCs exist, 8 if one |
| S2 | Same as S1 but start from the Esmarts, leaving Cluster planes out until last | Whether the primary Cluster must be present or the order matters (A5, R1) |
| S3 | Four planes on CRTC A, four on CRTC B, in one commit (needs two active CRTCs) | Spreading: whether 8 total works (A11) |
| S4 | Five planes on CRTC A with CRTC B idle but exposed | Whether an idle exposed CRTC lowers `nlayers` (A4, A11): if the fifth fails, the divisor counts it |
| S5 | S1 repeated with equal zpos on all planes, then with distinct zpos 0..7 | Whether zpos affects the count (A10, A12) and what the core does on equality (Q6) |
| S6 | One Cluster with an AFBC framebuffer modifier (only if a buffer can be allocated), others linear | Whether AFBC and linear windows mix (R7, R8). Do not count a failure at fb creation as a plane-count failure |
| S7 | S1 with small windows (for example 64x64) versus full-screen | Whether anything but the count limits the result (R11: expect the same count at any size) |
| S8 | One window scaled 8x up, then 8x down, then slightly beyond | The fixed 1/8..8 gate (R2) against the table's 4 for Clusters (should not matter in this file) |
| S9 | One plane with a source under 4 pixels | R3: expected to pass with the plane invisible, not to fail |
| S10 | Rotation and reflect values on each plane class | R9: which values the core accepts per class |
| S11 | Move one window from CRTC A to CRTC B between two `TEST_ONLY` commits | Migration (A14, A16) |

Reading the result against the expectation:

- S1 ends at 4 and S4 fails at 5: the layer divisor counts the exposed CRTCs
  (A11 confirmed, with two exposed).
- S1 ends at 8: one port is in `nvps`, or the board kernel differs from
  v6.18 (check Q2).
- S1 ends below 4 or fails on a particular plane: a rule other than R1 is
  responsible (format, scaling, modifier); compare S2 and S7.
- S3 passes with 8 planes total: spreading beats the single-CRTC cap, as
  expected.
- S1 passes with 8 but a real commit shows underflow: R11 and R12; this is a
  question for the real-commit part of `t6-commit-session`, with the KVM
  picture.

## Limits of this read

- Two files only, one tag. Vendor kernels (`docs/rk3588-vendor-vop2-source.md`)
  differ: they have Cluster sub-windows and extra Esmart regions as planes.
- DRM core behaviour (zpos normalization, primary-plane requirement, the
  rotation property range check) was not read and is tagged inferred.
- The `nvps` of the recorded board's device tree was not read in this item.
