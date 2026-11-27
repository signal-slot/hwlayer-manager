# RK3588 VOP2 backend: design note

Written 2026-11-28 (design only). **No measurement is claimed and no code
changed.** `docs/rk3588-backend-design.md` is an earlier note on the same
subject; this one is organised around the hardware topology and the open
questions a board session must answer.

Tags, as in `docs/platform-capabilities.md`:

- **observed 2026-11-28**: from `docs/observed/rk3588-topology.md` (one
  topology session, Armbian `6.18.55-current-rockchip64`, DRM driver
  `rockchip`, no frame submitted).
- **read from source (file / pin)**: from `docs/observed/rk3588-source.md`
  (vendor `rockchip-linux/kernel` `develop-6.1` @ `77168c8d5ab8`, nothing run)
  or from this repository's code as written. Mainline was **not** read.
- **inferred**: a conclusion from the above, neither seen nor stated.

Everything under "Proposal" and "Open questions" is inferred.

## 1. Hardware model

### 1.1 Video ports (CRTCs)

| Fact | Tag |
|---|---|
| Two CRTCs: `video_port0` (active, HDMI-A-1, 1920x1080@60) and `video_port1` (no connector) | observed 2026-11-28 |
| The board wires HDMI0 and HDMI1 only | observed 2026-11-28 |
| Vendor driver has four video ports: VP0 up to 7680x4320, VP1 and VP2 up to 4096x2304, VP3 up to 2048x1536; VP0 can splice with VP1 above 4096 wide | read from source (`rockchip_vop2_reg.c:3034-3092`, `rockchip_drm_vop2.c:175`) |
| A non-zero `rockchip,plane-mask` on any port makes ports with a zero mask be skipped | read from source (`rockchip_drm_vop2.c:18512-18575`) |
| Why only two CRTCs exist on the board (device tree disables the others) | inferred; the device tree was not read |

### 1.2 Windows and planes

| Fact | Tag |
|---|---|
| Eight planes, one window each: Cluster0-3 win0 (ids 33, 39, 45, 51), Esmart0-3 win0 (57, 63, 69, 75) | observed 2026-11-28 |
| Cluster0 and Cluster1 are Primary, the other six Overlay; no cursor plane | observed 2026-11-28 |
| Vendor tree: 4 Cluster windows (win0 plus "sub window" win1) and 4 Esmart; win1 planes exist only with DT `support-multi-area`: 8 planes by default, 12 with the flag | read from source (`rockchip_vop2_reg.c:6035-6410`, `rockchip_drm_vop2.c:18007-18053`); counts inferred |
| Vendor tree names the Esmart windows as Primary candidates, one Primary per CRTC | read from source (`rockchip_vop2_reg.c:6275-6410`, `rockchip_drm_vop2.c:18600-18800`) |
| The two kernels disagree on which windows are Primary; cause taken to be a kernel difference | inferred; mainline not read |
| 8 layers and 7 mixers bound simultaneously active windows; whether all windows can be on together was not determined | read from source (`rockchip_vop2_reg.c:7500-7501`); reachability inferred |
| Esmart windows carry "multi-area" (3 extra areas); whether they become DRM planes was not resolved | read from source (`rockchip_vop2_reg.c:6275-6300`) |
| A Cluster main and sub window share one AFBC decoder and must use the same modifier; sub window source at most 2048 wide | read from source (`rockchip_drm_vop2.c:6916-6962`) |
| AXI 0 serves Cluster0/1 and Esmart0/1; AXI 1 the others | read from source (`axi_id`, `rockchip_vop2_reg.c:6035-6410`) |

### 1.3 Possible-CRTC masks

| Fact | Tag |
|---|---|
| In the probe's view seven planes have `outputs 0b1` and Cluster1 (plane 39) has `0b0` | observed 2026-11-28 |
| The probe prints bits of connected outputs only, so Cluster1's real `possible_crtcs` is unknown; most likely it points at `video_port1` only | inferred |
| Every window can reach all four ports; the driver maps that onto the registered CRTCs, so by default every plane is possible on every registered CRTC | read from source (`rockchip_drm_vop2.c:18149-18167`) |
| With DT `disable-win-move`, a Primary reaches only its own CRTC and an Overlay only the CRTC whose `plane_mask` holds it | read from source (`rockchip_drm_vop2.c:18187, 18821-18834`) |
| Each CRTC has a `PLANE_MASK` property for moving windows between ports | read from source (`rockchip_drm_vop2.c:18751-18756`) |
| Whether the board's DT uses `disable-win-move` | not known (Q5) |

### 1.4 zpos

| Fact | Tag |
|---|---|
| Every plane reports zpos range 0..=7 | observed 2026-11-28 |
| Reset `normalized-zpos` is the plane index 0..7 (Cluster0 0 ... Esmart3 7); the active scanout was Cluster0 at zpos 0 | observed 2026-11-28 |
| The property is mutable, range 0 to registered windows minus 1; the DRM core normalises it; the driver sorts windows by it for mixer order | read from source (`rockchip_drm_vop2.c:18080, 12864, 15473`, `rockchip_drm_fb.c:423`) |
| Whether zpos is shared across CRTCs, and what happens when two planes on one CRTC share a value | not known (Q6) |

### 1.5 Formats and modifiers

| Fact | Tag |
|---|---|
| Cluster planes: 32 listed (format, modifier) pairs in Arm AFBC groups (`0x800000000000001`, `...041`, `...011`) over XR30, XB30, XR24, AR24, XB24, AB24, RG24, BG24, RG16, BG16, YU08, YU10, YUYV, Y210. The probe cuts at 32, so the list is incomplete | observed 2026-11-28 |
| Exact meaning of the AFBC flag bits | inferred from the modifier encoding |
| Esmart planes: 21 linear-only formats, RGB without 10-bit, NV12/21/16/61/20/24/42/30/15, packed YUV | observed 2026-11-28 |
| fbcon scans out of `Cluster0-win0` as XR24 with modifier 0, so that Cluster accepts linear XR24 on this kernel | observed 2026-11-28 |
| Cluster modifiers: AFBC 16x16 alone and with SPARSE, YTR, CBR, SPLIT, then LINEAR; Esmart: LINEAR only | read from source (`rockchip_vop2_reg.c:249-288`) |
| Linear XRGB2101010/XBGR2101010 rejected on any window; linear YUV rejected on Cluster; AFBC and tiled not combined | read from source (`rockchip_drm_vop2.c:7779-7810`) |
| The probe's listing begins with AFBC pairs although `DrmBackend::discover` sorts linear first (`composition-drm/src/lib.rs`): either the probe does not sort, or the Cluster list holds no linear pair. The second fits fbcon poorly | inferred (Q1) |

### 1.6 Scaling

| Fact | Tag |
|---|---|
| The probe reports "scaling not claimed" on all planes; no scale property was dumped | observed 2026-11-28 |
| Hardware factors: Cluster 4x, Esmart 8x, up and down | read from source (`max_upscale_factor`/`max_downscale_factor`, `rockchip_vop2_reg.c:6035-6410`) |
| The atomic check lets 1/8..8 through on any window with scaler registers, looser than Cluster's 4 | read from source (`rockchip_drm_vop2.c:7637-7662`); mismatch inferred |
| Source or destination under 4x4 makes the plane invisible, not an error | read from source (`rockchip_drm_vop2.c:7740-7749`) |
| Esmart cannot scale down when source width mod 16 is 1 or destination width is odd | read from source (comment, `rockchip_drm_vop2.c:8460`) |
| Each window has its own scaler; no shared scaler was found | inferred from the window table |

### 1.7 Rotation and reflection

| Fact | Tag |
|---|---|
| All planes read `rotation=1` (none) in the state dump; the property's bitmask was not read | observed 2026-11-28 |
| Cluster main: ROTATE_90/270, REFLECT_X/Y; Cluster sub: REFLECT only; Esmart: REFLECT_Y only | read from source (`rockchip_vop2_reg.c:6035-6410`) |
| ROTATE_90 with ROTATE_270 rejected; non-AFBC Cluster rejects REFLECT_X and ROTATE_90/270; tiled Cluster rejects all | read from source (`rockchip_drm_vop2.c:7699-7703, 7813-7826`) |
| Support therefore depends on the buffer's modifier, not on the plane alone | inferred |

### 1.8 Alpha and blend

| Fact | Tag |
|---|---|
| All eight planes show plane alpha and blend modes none, premultiplied, coverage (probe-derived) | observed 2026-11-28 |
| The active plane showed `glb_alpha` 0xff | observed 2026-11-28 |
| Plane alpha is applied as `alpha >> 8`; global and per-pixel alpha are chosen from format and value, 0xff counting as off | read from source (`rockchip_drm_vop2.c:7733, 13567-13623`) |
| That alpha blending follows zpos on the physical output | inferred; needs a KVM capture |

### 1.9 Size limits

Cluster main and Esmart: input and output up to 4096x7680; Cluster sub
2048x7680; the width limit doubles in splice mode: read from source
(`rockchip_vop2_reg.c:6035-6410`, `rockchip_drm_vop2.c:7695`). Not reachable at
the observed 1920x1080 output (inferred).

## 2. How the engine's DRM backend sees this

Read from source (this repository: `crates/composition-drm/src/lib.rs`,
`profile.rs`, `crates/composition-core/src/limits.rs`):

- Outputs come from connected connectors only, and a plane's `outputs` mask
  covers those only. This is why Cluster1 shows `0b0`.
- Formats come from `IN_FORMATS`, linear first, cut at `MAX_FORMATS = 32`.
  Planes are capped at `MAX_PLANES = 16`.
- `scaling` is one global `DrmOptions::scaling` for non-cursor planes;
  `transforms` is never filled, so the planner refuses any non-`Normal`
  transform; zpos, alpha and blend modes come from the properties.
- `DriverProfile` (`Generic`, `Imx95Dpu`, `Stm32Ltdc`) adds scaling,
  `max_source` and resource groups after discovery.

## 3. Proposal (inferred)

### 3.1 Generic first, profile second

The generic backend should stay correct on VOP2 because atomic `TEST_ONLY`
refuses what the hardware cannot do. A profile only lets the planner avoid
trial and report `SCALING`, `TRANSFORM` or `SOURCE_SIZE` instead of a bare
refusal. Do not add one before a board session confirms the table (Q2, Q3).

### 3.2 Gaps and how a `DriverProfile::Rk3588Vop2` would close them

| Gap | Mapping | Note |
|---|---|---|
| Per-class scaling | Esmart up and down 8x; Cluster 4x (the tighter hardware figure) | needs the window class (3.3) |
| `max_source` | 4096x7680 on non-cursor planes | splice doubling ignored: outputs wider than 4096 are not planned |
| Transforms | `PlaneCaps.transforms` is per plane, support depends on the modifier. Option A: report the Cluster AFBC set and let `TEST_ONLY` refuse linear plus rotate. Option B: report none, losing rotation. Recommend A | no core change; a per-(plane, modifier) model is not proposed |
| Esmart reflect | report REFLECT_Y on Esmart | |
| Format cut at 32 | Cluster lists exceed 32 pairs; pairs past the cut are invisible and a valid AFBC pair may be refused as `FORMAT` | needs the full list (Q1); fixes: raise `MAX_FORMATS` (core change) or prune by modifier family; decide only with data |
| Windows shared between CRTCs | the planner is per output and cannot see a plane used by the other CRTC | mitigate with one DRM lease per output (existing lease path), not a profile |
| Layer and bandwidth budget | `ResourceGroup { ANY, capacity 8 }` only if a board shows the limit (Q4) | vendor source shows no bandwidth rejection (inferred) |
| Cluster win1 pairing | out of scope unless `support-multi-area` is on | Q8 |

### 3.3 Identifying the window class

KMS does not expose window names (debugfs does). Candidates in order: (1) any
non-linear modifier in `IN_FORMATS` means Cluster, otherwise Esmart; (2)
presence of `SCALE_RATE` on the vendor kernel; (3) plane order. Candidate 1
fits both the observed 6.18 list and the vendor tables. Do not key on plane
type (the kernels disagree) or on object ids (they change).

### 3.4 Left out on purpose

- The Esmart odd-width scale-down case and the invisible sub-4x4 plane:
  `TEST_ONLY` covers them.
- Hard-coding which window is Primary.
- A cost model preferring Esmart for plain RGB: the core has none, and a
  layer's modifier, YUV, scale and transform already pick the class through the
  reject reasons once the table is filled.

## 4. Tests (host-only, no hardware claim)

They test planner behaviour on a **recorded** description, not hardware.

1. A fixture of the observed 8-plane topology with date and source in a
   comment (compare `dpu95_stream()` in `profile.rs`).
2. If a profile is added: 4x scaled ARGB on either class, 6x only on Esmart;
   NV12 linear refused on Cluster, accepted on Esmart; AFBC accepted on
   Cluster, refused on Esmart; 90-degree rotation refused on Esmart; source
   wider than 4096 refused; eight RGB layers fit, a ninth is `NoPlane`.
3. A two-output fixture pinning the documented blind spot.
4. A fixture with more than 32 pairs on a Cluster plane, to pin which survive
   the cut.
5. The `composition-drm` harness example is the device-side check: `TEST_ONLY`
   verdicts for the same combinations, recorded next to the planner's.

## 5. Open questions for a board session

None is answered here; each needs the board.

- **Q1.** Full `IN_FORMATS` of a Cluster plane (the probe cut at 32). Is
  linear listed? Does the cut hide usable AFBC pairs? Why did the probe list
  begin with AFBC pairs?
- **Q2.** Which kernel is the target: the 6.18 kernel of the board or the
  vendor 6.1 tree? Do Primary window choice, modifier list, rotation and
  `support-multi-area` differ? (Mainline not read.)
- **Q3.** Does "non-linear modifier present means Cluster" hold on the target
  kernel? Does `SCALE_RATE` exist there?
- **Q4.** How many windows can be on at once, with which mix (`TEST_ONLY`
  sweep)? Do AFBC, rotation or scaling lower it?
- **Q5.** Real `possible_crtcs` of every plane with both CRTCs visible (raw
  property dump, not the connected-only probe view). Does the DT use
  `disable-win-move`?
- **Q6.** Is zpos global or per CRTC? What does the kernel do with equal
  values on one CRTC? Does the alpha stack follow zpos on the physical output
  (KVM capture)?
- **Q7.** Does a second output (second connector or a lease split) work, and
  what does the second commit report when the same plane is taken?
- **Q8.** Do Cluster win1 or Esmart multi-area planes appear, with which
  pairing rule?
- **Q9.** Scaling limits as the kernel enforces them on one Cluster and one
  Esmart plane (check says 1/8..8, table says 4x and 8x), and the rotation
  bitmask per class.
- **Q10.** Latch and release behaviour per CRTC under two-plane commits,
  repeated buffer rotation and dropped frames (nothing was committed in the
  topology session).
- **Q11.** Can a producer fill an AFBC buffer on this board (GPU modifier
  support), so AFBC can be tested end to end?

## 6. Sequence after this note

1. A board session answers Q1-Q11 into `docs/platform-capabilities.md`, dated.
2. If the answers support it: `DriverProfile::Rk3588Vop2` and the section 4
   tests, with `scripts/vkms-vm-test.sh` run where it can be.
3. Only if Q1 shows loss: a `MAX_FORMATS` change (core change, separate item).
