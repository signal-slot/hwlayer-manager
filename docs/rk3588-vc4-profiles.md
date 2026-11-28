# DriverProfile variants for RK3588 (VOP2) and Raspberry Pi (HVS5, HVS6)

Written 2026-11-28 (board results folded in the same day). It documents the three `DriverProfile` variants next to `Imx95Dpu` and `Stm32Ltdc` in `crates/composition-drm/src/profile.rs`: `Rk3588Vop2` (NanoPC-T6, RK3588; `src/profile/rk3588.rs`), `Vc4Hvs5` (BCM2711) and `Vc4Hvs6` (BCM2712), both in `src/profile/vc4.rs`. The tables in sections 1 to 3 state the values that landed in those two files, each with its tag. Section 5 holds the dated board results.

## Portability evidence, not milestone evidence

Every run on these boards (NanoPC-T6, Raspberry Pi 5, reTerminal CM4) is portability evidence. It never counts toward M1 (i.MX95 Linux DRM / AGL), M2 (i.MX95 Zephyr) or M3 (AM62P regression). It shows that the planner and the profile mechanism carry over to other SoCs, nothing more.

## Tags

Tags follow `docs/platform-capabilities.md`:

- **observed** (always with its date, e.g. 2026-11-27): read from a real board, as recorded in the dated files under `crates/composition-drm/tests/data/{rk3588,bcm2711,bcm2712}/`. That is the KMS view the kernel exposes, or a recorded `TEST_ONLY` result; it is not a promise of what other commits accept. A `TEST_ONLY` ladder that never hit a rejection is a lower bound, and is written so.
- **read from source**: read from driver source, with the pin the reading gave. Not run. Sources: [`rk3588-mainline-source.md`](rk3588-mainline-source.md), [`rk3588-vop2-limits-source.md`](rk3588-vop2-limits-source.md), [`rk3588-vop2-scale-source.md`](rk3588-vop2-scale-source.md), [`rk3588-vop2-formats-source.md`](rk3588-vop2-formats-source.md), [`rk3588-dt-source.md`](rk3588-dt-source.md) and [`rk3588-vendor-vop2-source.md`](rk3588-vendor-vop2-source.md) for RK3588 (mainline v6.18 takes precedence over the vendor 6.1 kernel); [`vc4-hvs-scaling-source.md`](vc4-hvs-scaling-source.md), [`vc4-mainline-source.md`](vc4-mainline-source.md), [`vc4-hvs-load-source.md`](vc4-hvs-load-source.md), [`vc4-hvs-lbm-upm-source.md`](vc4-hvs-lbm-upm-source.md), [`vc4-hvs-format-source.md`](vc4-hvs-format-source.md) and [`bcm2712-dt-source.md`](bcm2712-dt-source.md) for vc4.
- **inferred**: a conclusion drawn from the other two, or a proposal of the design notes.

Pins and kernels:

- NanoPC-T6 (RK3588): Armbian kernel `6.18.55-current-rockchip64`; topology, scaling and sweeps observed 2026-11-27, verify runs observed 2026-11-28. Mainline source read at v6.18; the vendor `develop-6.1` @ `77168c8d5ab82399f65a80e9f807b50ba37cf483` reading is kept only as a comparison.
- Raspberry Pi 5 (BCM2712): kernel `6.18.50+rpt-rpi-2712`; topology, scaling and sweeps observed 2026-11-27, verify run observed 2026-11-28.
- reTerminal (CM4, BCM2711): kernel `6.12.47+rpt-rpi-v8`; same dates. This kernel is not the 6.18 source pin, so source rows are unconfirmed for it.
- vc4 source: `raspberrypi/linux` branch `rpi-6.18.y`, commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e`, read 2026-11-28.

Design documents: [`rk3588-backend-design.md`](rk3588-backend-design.md) (RK design) and [`hvs-backend-design.md`](hvs-backend-design.md) (HVS design). Where this file and a design table differ, this file follows the board and the mainline reading; the design tables were written from the vendor kernel and from the Pi tree before any board run.

## 1. Profile `Rk3588Vop2`

Values are those of `src/profile/rk3588.rs`. They are applied per plane by window class and agree with the board; the vendor figures of the RK design (Cluster 4x, 4096x7680, AFBC-only rotation) are not used.

| Constraint | Value | Tag | Source file |
| --- | --- | --- | --- |
| Planes | 8 planes: Cluster 33, 39, 45, 51 and Esmart 57, 63, 69, 75; 33 and 39 are Primary, the rest Overlay. All fit under `MAX_PLANES` | observed 2026-11-27 | `tests/data/rk3588/README.md` |
| Reach | The connected output (CRTC 81, `video_port0`) is reached by 7 planes; plane 39 has mask `0b10` (CRTC 82 only). Plane 69 listed `0b11` in the master run; the earlier lease-run note says 69 cannot reach `video_port0`, so the lease may hide it (not tested). Mask follows from the DT port order | observed 2026-11-27 (masks); mask rule read from source | `rk3588-dt-source.md` |
| Window class | A plane listing any Arm AFBC modifier (vendor `0x08`) is Cluster, any other is Esmart; plane type and id do not tell it. Every Cluster pair was AFBC (0 of 126 linear), no Esmart pair was | observed 2026-11-27; rule matches source (`format_modifiers_afbc` only on Cluster) | `rk3588-vop2-formats-source.md` |
| Scaling, Cluster and Esmart | Up 8x and down 8x per axis, the same for both classes. Board: every probed plane (Cluster 33, 45, 51; Esmart 57, 63, 69, 75) accepted 8.0 on h, v and both and refused 8.0039 with ERANGE, at destinations 960x540 and 1920x1080 (and `--max-factor 64` changed nothing). Down: 0.9, 0.75, 0.5 accepted | observed 2026-11-27 (up to 8x; down to 0.5 only) | `tests/data/rk3588/scale-probe-*.txt` |
| Scaling, below 0.5 | The 8x down figure (`min_scale = 1/8`) is source, never reached by a board ladder | read from source (mainline v6.18, one bound for every window) | `rk3588-vop2-scale-source.md` |
| Per-window table factors | Cluster 4, Esmart 8 exist in the register tables but no code reads them, so the vendor 4x Cluster limit does not apply on 6.18. Picture quality of a Cluster window scaled beyond 4x was not looked at and is not modelled | read from source; board accepted 8x on Cluster 2026-11-27 | `rk3588-vop2-scale-source.md` |
| Plane 39 scaling | Not probed (cannot reach CRTC 81); given the Cluster values by class | inferred | |
| `max_source` | 4096 x 4320 both classes (`max_input`, one limit for every window). Not measured: the `wide-source` frame was refused by the planner and at dumb-buffer creation, so `TEST_ONLY` never saw it | read from source (mainline v6.18) | `rk3588-vop2-limits-source.md` |
| Format and modifier lists | Cluster 14 formats under 9 AFBC modifiers = 126 pairs, no linear; Esmart 21 linear formats including NV12 (one modifier). The probe cuts a plane to 32 pairs: a Cluster plane then shows 2 modifiers and part of a third | observed 2026-11-27 (full lists, from `probe --raw`) | `tests/data/rk3588/probe-raw-2026-11-27.txt` |
| Modifier acceptance | All 400 listed pairs the sweep could build passed `TEST_ONLY`. Linear (0x0) on Cluster planes 33, 45, 51 was also accepted for 10 formats each (XR24, AR24, XB24, AB24, RG24, BG24, RG16, BG16, YUYV, Y210) although the blob lists only AFBC. AFBC on Esmart: ADDFB2 ok for 9 formats, `TEST_ONLY` EINVAL. SAND128 fails at ADDFB2 on both classes. Linear XR30/XB30 failed at ADDFB2 (sweep layout, not investigated) | observed 2026-11-27 | `tests/data/rk3588/sweep-modifiers-2026-11-27.txt` |
| Layer budget | 4 planes per frame on CRTC 81: `COMBO_MAX accepted_k=4`. All 20 four-plane sets (Cluster and Esmart mixed, plane 33 fixed) passed; all 15 five-, 6 six- and 1 seven-plane sets gave EINVAL. Scaled runs (v 2x, h 2x, h 0.5) gave the same boundary. 8 windows on one port are not reachable on this DT (7 candidates) | observed 2026-11-27 | `tests/data/rk3588/sweep-combos-*.txt` |
| Layer budget, cause | The CRTC check refuses more than `nlayers = win_size / nvps` planes, 8 / 2 = 4 with the two used ports. The observed EINVAL matching that count and errno is a match, not a proof; the ports coming from the DT is also not read here | read from source (`V2C:2483`, `V2C:1816-1835`); cause inferred | `rk3588-dt-source.md` |
| Resource group | One group over all planes, every layer counted, capacity 4 (`LAYERS_PER_PORT`). A DT with one used port allows 8 (the planner then refuses frames of 5 to 7 planes the kernel would take); three or four used ports allow 2 (the kernel refuses what the planner lets through) | inferred from the observed 4 and the source formula | `src/profile/rk3588.rs` |
| Transforms | Left as KMS reports them (the DRM backend reports none). Source: Cluster rotates 90/270 and reflects X/Y, Esmart reflects Y. The board `rotation` property was recorded with its current value (rotate-0) only, not its supported bits, so the AFBC-only option (a) of the design is not taken | read from source; current value observed 2026-11-27 | `rk3588-vop2-formats-source.md` |
| zpos | Mutable range 0..=7 on every plane, distinct initial values | observed 2026-11-27 | `tests/data/rk3588/README.md` |
| Alpha and blend | `alpha` 0..=65535 and `none`, `premultiplied`, `coverage` on all planes | observed 2026-11-27 | `tests/data/rk3588/README.md` |
| Outputs | 2 CRTCs registered (`video_port0`, `video_port1`); one connector connected (`HDMI-A-1`, 1920x1080@60), so one output. Per-port output limits (VP0 to VP2 4096x2304, VP3 2048x1536) | observed 2026-11-28 (CRTCs, connectors); limits read from source, not modelled | `rk3588-vop2-limits-source.md` |
| Scaler resource | Each window has its own scaler registers; no scaler resource group | read from source; "no group needed" inferred | `rk3588-vop2-scale-source.md` |
| Cluster sub windows | Planes only with DT `support-multi-area`; not present in the 8-plane recording | read from source | `rk3588-vop2-formats-source.md` |

Disagreements with the design table, settled by the board or mainline: Cluster scaling is 8x (design 4x), `max_source` 4096x4320 (design 4096x7680), and the layer budget is 4 (design 8). Esmart windows as Primary candidates (vendor source) against Cluster0 and Cluster1 Primary on 6.18: the profile reads `type` from KMS.

## 2. Profile `Vc4Hvs5` (BCM2711)

Values are those of `src/profile/vc4.rs` with `Generation::Hvs5`. The generation is chosen by this explicit variant, because both generations report driver name `vc4`. "Source" below is the Pi tree pin above, not the 6.12 kernel the CM4 runs.

| Constraint | Value | Tag | Source file |
| --- | --- | --- | --- |
| Generation selector | Explicit variant; the driver name `vc4` does not tell HVS5 from HVS6. On the CM4 the `hvs6` profile (negative control) gave the same 27-frame counts as `hvs5`, so the run does not tell the two apart | observed 2026-11-28 (driver name, counts); design inferred | `tests/data/bcm2711/verify-profile-check-2026-11-28.txt` |
| Planes | 60 planes, 6 CRTCs (ids 59, 78, 90, 102, 114, 126), 3 connectors; connected CRTC index 2 (DSI, 720x1280@60) reaches primary 79, 16 shared overlays and cursor 677 = 18; the probe keeps 16 (primary, 14 overlays, cursor) and cuts overlays 281 and 292 | observed 2026-11-27 (counts), 2026-11-28 (kept) | `tests/data/bcm2711/README.md` |
| Scaling, non-cursor planes | Up 32x, down 2x per axis (`32 << 16`, `2 << 16`), the smallest range a run accepted. Board: every factor 1 to 32 up and 0.9, 0.75, 0.5 down accepted on primary 79 and overlays 127, 138, 149, 160, 292, at both destination sizes; with `--max-factor 64` up to 64x also accepted. No rejection occurred, so these are lower bounds, not ceilings | observed 2026-11-27 (lower bounds); ceilings: upscale unbounded and downscale refused above 31.99998x horizontally (TPZ field) | read from source: `vc4-hvs-scaling-source.md` |
| Scaling, cursor | `scale_probe` refuses to test cursor planes; KMS value kept | observed 2026-11-27 (not testable); no scaler inferred | |
| `max_source` | Width 8191 (`SCALER5_POS2_WIDTH_MASK`), height 7680. The `wide-source` frame (4352 wide) was refused by the planner by a SCALING reason and accepted by `TEST_ONLY` with the limit removed, so the board neither confirms nor refutes the 8191 | read from source; board run 2026-11-28 shows `TEST_ONLY` accepting 4352 | `vc4-hvs-scaling-source.md` |
| Format and modifier lists | Identical on all 60 planes: 35 formats, 5 modifiers (linear 34 formats; T-tiled XR24, AR24, AB24, XB24, RG16, BG16, AR15, XR15; SAND64 and SAND256 NV12/NV21; SAND128 NV12, NV21, P030). The profile keeps the KMS list | observed 2026-11-27 | `tests/data/bcm2711/README.md` |
| Modifier acceptance | Every listed pair the sweep could build passed ADDFB2 and `TEST_ONLY` (T-tiled on the 8 RGB formats, SAND64/128/256 on NV12/NV21; planes 79, 127, 138). Unlisted pairs fail at ADDFB2 with EINVAL. RGB8, BGR8, P030 not buildable by the sweep, so not measured | observed 2026-11-27 | `tests/data/bcm2711/sweep-modifiers-2026-11-27.txt` |
| Layer budget | 16 planes full-screen active passed at 1:1, with `--scale-v 0.5` and with `--scale-v 2` (`COMBO_MAX accepted_k=16`, 462 requests per run) at 720x1280. No LBM limit showed in `TEST_ONLY` at that size; scan-out behaviour is not measured | observed 2026-11-27 | `tests/data/bcm2711/sweep-combos-scaled-2026-11-27.txt` |
| LBM group | Consumers scaled-vertically or YUV, capacity 8 over all planes: 61440 words and 64 handles, a 1920-wide layer needs at most 7680 words. Sizes are for a 1920-wide fetch; the 720-wide panel never reached the limit | read from source (limits); capacity inferred | `vc4-hvs-lbm-upm-source.md` |
| Dlist, bandwidth | No dlist group (far above a frame's need). Core-clock rate computed, no load-tracker check: a commit the bus cannot sustain fails by underrun, not in `TEST_ONLY` | read from source | `vc4-hvs-dlist-source.md`, `vc4-hvs-load-source.md` |
| Transforms | `rotation` current value 1 (rotate-0) on all planes; the supported mask was not decoded on this board. By the Pi 5 record (same driver) it is 0/180 plus reflect X/Y, no 90/270 | observed 2026-11-27 (current value only); mask inferred for BCM2711 | `tests/data/bcm2711/README.md` |
| zpos | Primary fixed 0; overlays mutable 1..=17; cursor value 17 | observed 2026-11-27 | `tests/data/bcm2711/README.md` |
| Alpha and blend | `alpha` 0..=65535; `none`, `premultiplied`, `coverage` | observed 2026-11-27 | `tests/data/bcm2711/README.md` |
| Outputs | 6 CRTCs, 3 HVS channels, at most 3 active outputs; pinning pv0 channel 0 only, VEC channel 1 only, TXP channel 2 only. One connected output (`DSI-1`) | observed 2026-11-27 (CRTCs, connector); pinning read from source | `vc4-mainline-source.md` |
| `replace_pending` | A second non-blocking flip committed before the first one's event returned EBUSY; the first one's event arrived. A pending commit cannot be replaced: the profile leaves the reported default | observed 2026-11-27 (`kms_sweep replace`) | `tests/data/bcm2711/sweep-replace-2026-11-27.txt`; `drm-pending-commit-source.md` |

## 3. Profile `Vc4Hvs6` (BCM2712)

Values are those of `src/profile/vc4.rs` with `Generation::Hvs6`.

| Constraint | Value | Tag | Source file |
| --- | --- | --- | --- |
| Generation selector | Explicit variant `Vc4Hvs6`; driver name `vc4`. C0 vs D0 stepping is not visible | observed 2026-11-28 (driver name); "same behaviour" inferred | `tests/data/bcm2712/verify-profile-check-2026-11-28.txt` |
| Planes | 56 planes, 4 CRTCs (ids 59, 78, 94, 106; index 2, CRTC 94, drives `HDMI-A-1`): 4 primaries, 16 shared overlays (reach CRTCs 1 to 3), 32 CRTC-0-only overlays, 4 cursors. CRTC 94 reaches primary 83, the 16 overlays and cursor 657 = 18; the probe keeps 16 | observed 2026-11-27 (counts, masks), 2026-11-28 (kept) | `tests/data/bcm2712/README.md` |
| Plane arithmetic | 4 + 16 + 32 + 4 = 56. The earlier "57" counted primary 48 twice; the extra planes 646, 657 and 668 are the cursors of CRTCs 1 to 3 (HVS Q5) | read from source; ids observed 2026-11-27 | `bcm2712-dt-source.md` |
| Scaling, non-cursor planes | Up 32x, down 2x (`32 << 16`, `2 << 16`). Board: primary 83 and all 16 shared overlays accepted every factor to 32x up, and with `--max-factor 64` primary 83 and overlay 107 accepted 64x up, plus 0.9, 0.75, 0.5 down; no rejection, so lower bounds. The real bound is the source one (TPZ field), not measured | observed 2026-11-27 (lower bounds); bound read from source | `vc4-hvs-scaling-source.md` |
| `max_source` | 8192 x 8192: the driver has no source-size check but writes `width - 1` and the line count into 13-bit fields. Not measured: `wide-source` (4352 wide) was accepted by `TEST_ONLY` | read from source; value inferred | `vc4-hvs-scaling-source.md` |
| Format and modifier lists | Identical on all 56 planes: 45 formats, 5 modifiers (linear 39 of them; T-tiled 8 RGB formats; SAND64 and SAND256 NV12/NV21; SAND128 NV12, NV21, P030) | observed 2026-11-27 | `tests/data/bcm2712/README.md` |
| Modifier acceptance | On primary 83 and overlays 107, 118: linear accepted (39 of 45 formats; 5 per plane unbuildable by the sweep); **T-tiled EINVAL on all 24 pairs; SAND64 EINVAL on all 6**; SAND128 and SAND256 accepted (NV12, NV21 on each plane, 6 of 6 each). Unlisted pairs fail at ADDFB2. The blob over-reports and the source is right: HVS6 rejects T-tiled and SAND64. This settles the blob-versus-source conflict of the HVS design | observed 2026-11-27 | `tests/data/bcm2712/sweep-modifiers-2026-11-27.txt`; `vc4-hvs-format-source.md` |
| Profile filter | The profile removes pairs with the T-tiled modifier or a SAND64 base modifier (`vc4_format_mod_supported`, rpi-6.18.y) | read from source; confirmed by the board 2026-11-27 | `src/profile/vc4.rs` |
| Layer budget | 1:1: 16 planes accepted (`COMBO_MAX accepted_k=16`, `--max 16`; 17 not tried). `--scale-v 0.5` (2:1 downscale): k <= 8 ok, k = 9..16 ENOSPC. `--scale-v 2` (2x upscale): k <= 4 ok, k = 5..16 ENOSPC, coincident with `Failed to allocate LBM entry: -28` in the kernel log. One data point at 1920x1080, not the HVS6 limit | observed 2026-11-27 | `tests/data/bcm2712/sweep-combos-*.txt` |
| LBM group | Consumers scaled-vertically or YUV, capacity 4: 1024 lines; a 1920-wide ARGB layer with opaque plane alpha and polyphase scaling needs 240. The observed 2x-upscale boundary of 4 layers equals this capacity. The 8 at 2:1 down is looser than the group, so the planner may refuse a frame the kernel took (a lost choice) | read from source (limits); capacity inferred; boundary observed 2026-11-27 | `vc4-hvs-lbm-upm-source.md` |
| UPM group | All layers counted, capacity 16: 32 handles, one per colour plane. With `MAX_PLANES` = 16 it cannot bind a frame alone | read from source; capacity inferred | `vc4-hvs-lbm-upm-source.md` |
| Transforms | `rotate-0` only on every plane in the 2026-11-27 dump (`rotation` value 1); source allows 0/180 and reflect X/Y, no 90/270 | observed 2026-11-27 (current value); mask read from source | `vc4-hvs-props-source.md` |
| zpos | Primary immutable 0; overlays mutable 1..=17; cursor 17 | observed 2026-11-27 | `tests/data/bcm2712/README.md` |
| Alpha and blend | `alpha`, `none`, `premultiplied`, `coverage` on all planes | observed 2026-11-27 | `tests/data/bcm2712/README.md` |
| Outputs | 4 CRTCs, 3 HVS channels: HDMI0 channel 0, HDMI1 channel 1, `moplet` also channel 1 (HDMI1 and `moplet` exclude each other) | observed 2026-11-27 (CRTCs); pinning read from source | `bcm2712-dt-source.md` |
| Bandwidth | Rate computed but never applied; no load-tracker check. Underrun not visible in `TEST_ONLY`; `hvs_underrun` read 0 | read from source; counter observed 2026-11-28 | `vc4-hvs-load-source.md` |
| `replace_pending` | Second non-blocking flip while the first was pending: EBUSY, one event. Not replaceable | observed 2026-11-27 | `tests/data/bcm2712/sweep-replace-2026-11-27.txt` |

## 4. Open questions

Numbers are those of the design documents. "Answered" means a dated board run or a source reading settled it.

RK3588 (`rk3588-backend-design.md` section 7):

- Q1: **answered.** Full `IN_FORMATS` read: Cluster 126 pairs (14 formats x 9 AFBC modifiers, no linear), Esmart 21 (observed 2026-11-27). The first-32 cut hides 94 Cluster pairs. The linear-first ordering claim does not hold on Cluster: the list has no linear pair.
- Q2: **answered.** The 6.18.55 kernel uses the mainline values (8x both classes, `max_input` 4096x4320, 4 planes per port); the vendor 6.1 values (Cluster 4x, 4096x7680) differ and are not used.
- Q3: **answered.** AFBC modifier present means Cluster (observed 2026-11-27; source agrees).
- Q4: **answered for this DT.** 4 simultaneous planes on CRTC 81, 7 reachable; 8 not reachable here (observed 2026-11-27).
- Q5: open (planner policy, not a table value).
- Q6: partly answered: zpos is mutable 0..=7 on every plane (observed 2026-11-27); global versus per CRTC and equal-value behaviour not measured.
- Q7: open. One connected output only.
- Q8: open. The 8-plane recording has no sub windows.
- Q9: open. No commit was made on this board: `profile_check --show` and `twoplane` stopped before the first commit (2026-11-28).

HVS (`hvs-backend-design.md` section 7):

- Q1: **partly answered.** The smallest accepted range is 32x up and 2x down on both Pi boards (observed 2026-11-27, lower bounds, no rejection up to 64x up). The real ceilings were not reached; the source gives the TPZ field limit.
- Q2: open. Bandwidth is not enforced; `hvs_underrun` read 0 on both Pi boards (2026-11-28) and no stress frame was run.
- Q3: **answered in practice.** The explicit variant works; HVS5 and HVS6 profiles give the same counts on a BCM2711 board and on BCM2712 (2026-11-28), so a board run cannot tell them apart; they differ in the modifier filter and the groups.
- Q4: open (core decision). The probe keeps 16 of 18 reachable planes on both boards.
- Q5: **answered.** 4 + 16 + 32 + 4 = 56; the extra planes are cursors (source, ids observed 2026-11-27).
- Q6: **answered.** A pending commit cannot be replaced, EBUSY on both boards (observed 2026-11-27).
- Q7: **partly answered.** BCM2712 on 6.18.50 matches the source for modifiers (T-tiled and SAND64 rejected, observed 2026-11-27); the CM4 runs 6.12.47 and its `TEST_ONLY` accepted everything its blob lists, which also matches the HVS5 source.
- Q9: open. Shared LBM and UPM across outputs are invisible to the planner; only one output was connected per board.
- Q10: open.

(HVS Q8, firmware KMS, is out of scope.)

## 5. Board results

All three sections are portability evidence and never count toward M1, M2 or M3. Runs were on idle boards without a display manager or compositor, and every `TEST_ONLY` frame is the output of `profile_check`.

### NanoPC-T6

- Kernel `6.18.55-current-rockchip64`, driver `rockchip`, HDMI-A-1 1920x1080@60 on CRTC 81, DRM master. Profile runs observed 2026-11-28 (repository commit `2d689bac`); topology, scaling and sweeps observed 2026-11-27. The display managers (gdm, lightdm, sddm) were inactive before and after, so nothing was stopped.
- `profile_check`, 21 frames each. Generic: `AGREEMENT total=21 agree=12 stricter=6 looser=0`; the 6 stricter frames are `scale-0.5` to `scale-8` (the Generic profile claims no scaling, `TEST_ONLY` accepts all six). `Rk3588Vop2`: `total=21 agree=18 stricter=0 looser=0`, PASS. The 3 remaining frames are `agree=no`: `rotate-90` and `reflect-y` (composition-drm does not program rotation, `TEST_ONLY` not reached) and `wide-source` (dumb buffer refused at creation). No planner-looser case (observed 2026-11-28).
- Layer frames: 1 to 4 layers planned on Esmart planes 57, 63, 69, 75 and accepted; 5 to 7 refused by the planner (`IN_USE`) and `TEST_ONLY` EINVAL with forced caps. The profile's capacity of 4 agrees with the board (observed 2026-11-28; the 2026-11-27 sweep gave the same boundary).
- Modifier frames `modifier-XR30-...01`, `...41`, `...11` (AFBC on primary 33 plus Esmart 57): planned and accepted by `TEST_ONLY` (observed 2026-11-28).
- With the profile the probe shows scaling up 8.00 down 8.00 and max source 4096x4320 on every plane (observed 2026-11-28).
- **No picture was judged.** `profile_check --show 20` and `twoplane --frames 600` both stopped before the first commit with "no primary plane with a linear 32-bit RGB format": primary plane 33 lists AFBC modifiers only. No KVM picture was taken, `camera-check.py` and `twoplane-expect.py` did not run, so there is no verdict (observed 2026-11-28). A linear XR24 on a Cluster primary was accepted by `TEST_ONLY` 2026-11-27, but the examples do not use unlisted modifiers.
- Findings: scaling 8x on both classes, 0.5 down measured (observed 2026-11-27); 4-layer budget (observed 2026-11-27); Cluster class from AFBC; modifier acceptance as in section 1. Pending-commit replacement was not measured on this board.
- Data: `crates/composition-drm/tests/data/rk3588/verify-*-2026-11-28.txt`, `sweep-*`, `scale-probe-*`, `README.md`.

### Raspberry Pi 5

- Kernel `6.18.50+rpt-rpi-2712`, driver `vc4`, HDMI 1920x1080@60 on CRTC 94. Profile runs observed 2026-11-28 (repository commit `8136775`, cross-built aarch64 musl, run as root); the image has no display manager or compositor, so none was stopped or restarted.
- `profile_check`, 27 frames each. Generic: `agree=18 stricter=7 looser=0` (the six scale frames and `wide-source`). `Vc4Hvs6`: `agree=24 stricter=1 looser=0`; `Vc4Hvs5` negative control: the same `24/1/0`. The one stricter frame is `wide-source`: the planner refuses a 4352-wide source and `TEST_ONLY` accepts it with the limit removed. Every scale frame agrees. No planner-looser case (observed 2026-11-28).
- The `hvs5` negative control did not differ from `hvs6`: `profile_check` produced no `modifier-*` frame because no non-linear modifier reaches the planner's capabilities, so T-tiled and SAND64 were not exercised (observed 2026-11-28).
- Layer frames 1 to 16 all planned and accepted; the 16th uses cursor 657. These frames are unscaled, so they do not reach the LBM capacity of 4 (observed 2026-11-28).
- `twoplane --frames 600` (generic): `PASS 600 frames on two planes, 600 latched, 1198 released, 0 late` (observed 2026-11-28).
- Picture: `profile_check --profile hvs6 --show 20`, KVM screenshot of the HDMI capture taken during the show (a few seconds after the commit; the picture is not kept), judged by `twoplane-expect.py` and `camera-check.py` with `--panel full --rotation 0 --tolerance 60`: `VERDICT: PASS`, all 8 regions, largest distance 37.2 (red block) (observed 2026-11-28).
- Findings (observed 2026-11-27): modifiers (T-tiled and SAND64 `TEST_ONLY` EINVAL, SAND128 and SAND256 accepted); scaling accepted to 64x up and down to 0.5; LBM ENOSPC boundary of 8 layers at 2:1 down and 4 at 2x up; pending commit not replaceable (EBUSY, one event). The output stays dark after the `replace` run until the next client commits.
- Data: `crates/composition-drm/tests/data/bcm2712/verify-*-2026-11-28.txt`, `sweep-*`, `scale-probe-*`, `debugfs-*`, `README.md`.

### reTerminal CM4

- Kernel `6.12.47+rpt-rpi-v8`, `card0` (vc4), DSI output 720x1280@60 on CRTC 90. Profile runs observed 2026-11-28 (repository commit `8136775`, aarch64 musl binaries run from a temporary directory and removed afterwards). No display manager or compositor was running (the launcher is disabled for this lease), so none was stopped. **There is no KVM or camera for this board: no picture was judged** (observed 2026-11-28).
- `profile_check`, 27 frames each: generic `agree=18 stricter=7 looser=0`; `hvs5` `agree=24 stricter=1 looser=0`, PASS; `hvs6` (negative control) `24/1/0`; `hvs5 --show 10` `24/1/0`, PASS. Generic refuses all six scale ratios (no scaling claimed) and `wide-source`; Hvs5 plans all six and the only stricter case is `wide-source` (accepted by `TEST_ONLY` with forced caps). No planner-looser case. `hvs6` did not disagree with `hvs5` (observed 2026-11-28).
- `profile_check --show 10` counted by its `TEST_ONLY` and commit results only (`SHOW on screen for 10 s`, `SHOW done: blanked`), not by a picture (observed 2026-11-28).
- `twoplane --frames 600`: `PASS 600 frames on two planes, 600 latched, 1198 released, 0 late`, driver `vc4` (observed 2026-11-28).
- Findings (observed 2026-11-27): every listed modifier pair passed `TEST_ONLY`; unlisted pairs fail at ADDFB2; 16 planes active passed at 1:1 and scaled; scaling accepted to 64x up and 0.5 down; `replace` second flip EBUSY. This kernel is 6.12, not the 6.18 source pin.
- Data: `crates/composition-drm/tests/data/bcm2711/verify-*-2026-11-28.txt`, `sweep-*`, `scale-probe-*`, `debugfs-*`, `README.md`.

### Left for later

Not measured by any of the three runs: rotation and reflection bits per plane (all three boards), the RK Cluster scaling below 0.5 and the quality beyond 4x, the real vc4 scaling ceilings, bandwidth or underrun under load, a second output on any board, a pending-commit replacement on the NanoPC-T6, and a KVM picture of the NanoPC-T6 (needs an example that can scan out an AFBC or an unlisted linear primary).
