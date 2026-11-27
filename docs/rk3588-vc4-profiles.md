# DriverProfile variants for RK3588 (VOP2) and Raspberry Pi (HVS5, HVS6)

First draft, written 2026-11-28. It documents the three planned `DriverProfile` variants next to `Imx95Dpu` and `Stm32Ltdc` in `crates/composition-drm/src/profile.rs`: `Rk3588Vop2` (NanoPC-T6, RK3588), `Vc4Hvs5` (BCM2711) and `Vc4Hvs6` (BCM2712). This draft holds the source and inferred parts only. **No board result is claimed here**; the board sections are empty slots. Nothing in this file was run.

## Portability evidence, not milestone evidence

Every run on these boards (NanoPC-T6, Raspberry Pi 5, reTerminal CM4) is portability evidence. It never counts toward M1 (i.MX95 Linux DRM / AGL), M2 (i.MX95 Zephyr) or M3 (AM62P regression). It shows that the planner and the profile mechanism carry over to other SoCs, nothing more.

## Tags

Tags follow `docs/platform-capabilities.md`:

- **observed** (always with its date, for example 2026-11-28): read from a real board, as recorded in the dated files named in the row. That is the KMS view the kernel exposes, or a recorded `TEST_ONLY` result; it is not a promise of what other commits accept.
- **read from source**: read from driver source, with the pin the recording gave. Not run. The RK3588 source reading is the vendor kernel, not mainline; the vc4 reading is the Raspberry Pi tree.
- **inferred**: a conclusion drawn from the other two, or a proposal of the design notes.

The "Doc" column names the file a row was copied from. The files named with an `observed/` prefix are the recordings dated 2026-11-28 under `docs/observed`; this file does not link to them because that directory is to be removed. Pins and dates quoted:

- RK3588 topology: NanoPC-T6, Armbian kernel `6.18.55-current-rockchip64`, observed 2026-11-28 (`rk3588-topology`). No frame was submitted. A later NanoPC-T6 session (kernel `6.18.55-current-rockchip64`, observed 2026-11-27 per the plan handoff, not yet recorded in the repository) adds: planes 39 and 69 cannot reach `video_port0`, and the kernel lease holds primary 33 plus overlays 45, 51, 57, 63, 69 and 75. It is mentioned here as plan context only and is inferred until a recorded file exists.
- RK3588 source: vendor `rockchip-linux/kernel` branch `develop-6.1`, commit `77168c8d5ab82399f65a80e9f807b50ba37cf483`, read 2026-11-28 (`rk3588-source`). Mainline was not read.
- vc4 source: Raspberry Pi kernel, branch `rpi-6.18.y`, commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e`, read 2026-11-28 (`vc4-hvs-source`).
- BCM2712 (Pi 5): kernel `6.18.50+rpt-rpi-2712`, observed 2026-11-28 (`bcm2712-topology`, `bcm2712`).
- BCM2711 (reTerminal, CM4): kernel `6.12.47+rpt-rpi-v8`, observed 2026-11-28 (`bcm2711-cm4-topology`, `bcm2711-cm4`).

Design documents: [`rk3588-backend-design.md`](rk3588-backend-design.md) (RK design) and [`hvs-backend-design.md`](hvs-backend-design.md) (HVS design).

## 1. Profile `Rk3588Vop2`

Values come from `rk3588-backend-design.md` section 4 unless noted. "RK design" below means that document.

| Constraint | Value | Tag | Doc |
| --- | --- | --- | --- |
| Planes kept (`MAX_PLANES` cut) | 8 planes: Cluster0..3 win0 and Esmart0..3 win0; Cluster0 and Cluster1 are Primary, the rest Overlay. All 8 fit under the cut of 16. The plan handoff adds that reachable planes sort ahead of the cut (fix-x09), so the order matters only on larger SoC variants | observed 2026-11-28, kernel 6.18.55 | RK design §2; `observed/rk3588-topology` |
| Planes that cannot reach the connected output | Plane 39 (Cluster1) shows `outputs 0b0` in the probe view. Whether it reaches `video_port1` is not read | observed 2026-11-28 (mask); reach to `video_port1` inferred | `observed/rk3588-topology` |
| Scaling, Cluster | Hardware up 4x and down 4x; the kernel check lets 1/8..8 through. The profile takes the tighter 4x | read from source (vendor `develop-6.1` @ `77168c8d`); choice of 4x inferred | RK design §4 |
| Scaling, Esmart | Up 8x, down 8x | read from source (vendor `develop-6.1` @ `77168c8d`) | RK design §4 |
| Scaling resource | Each window has its own scaler registers; no scaler `ResourceGroup` | read from source; "no group needed" inferred | RK design §4 |
| `max_source` | 4096 x 7680 for both classes (Cluster sub window 2048 x 7680; width limit doubles in splice mode) | read from source (vendor `develop-6.1` @ `77168c8d`) | RK design §4 |
| Format/modifier priority, Cluster | RGB with Arm AFBC modifiers (`0x800000000000001`, `...041`, `...011` seen), plus YU08, YU10, YUYV, Y210. The recorded list was cut at 32 pairs by the probe. Linear YUV and linear 10-bit RGB rejected | observed 2026-11-28 (AFBC list, first 32 pairs); rejections read from source | `observed/rk3588-topology`; RK design §2, §4 |
| Format/modifier priority, Esmart | 21 formats, linear only: RGB plus NV12, NV16, NV24, NV15, NV30 and packed YUV; no 10-bit RGB | observed 2026-11-28 | `observed/rk3588-topology` |
| Resource groups | None for scalers. A group with `ANY` and capacity 8 over all planes would encode the layer budget, but is left out until the budget is measured | inferred | RK design §4 |
| Layer budget | At most 8 simultaneously active windows (`nr_layers`), 7 mixers; AXI 0 serves Cluster0, Cluster1, Esmart0, Esmart1 and AXI 1 the others. Whether all can be on together is undetermined | read from source (bound); reachability inferred | RK design §2, §4 |
| Transforms, Cluster | Rotate 90/270 and reflect X/Y only for AFBC buffers; linear and tiled buffers get none. `PlaneCaps.transforms` is per plane, so the profile reports the AFBC set and relies on `check` to refuse linear plus rotate (option (a)) | read from source (vendor); option (a) inferred | RK design §2, §4 |
| Transforms, Esmart | Reflect Y only | read from source (vendor) | RK design §2, §4 |
| zpos | Mutable range 0..=7 on every plane; reset state is the plane index 0..7 (Cluster0 0 through Esmart3 7) | observed 2026-11-28, kernel 6.18.55 | `observed/rk3588-topology` |
| Alpha and blend | Plane alpha and `none`, `premultiplied`, `coverage` on all planes | observed 2026-11-28 | `observed/rk3588-topology` |
| Outputs | 2 CRTCs registered (`video_port0`, `video_port1`); one connector connected (`HDMI-A-1`), so one output. Vendor port limits: VP0 7680x4320, VP1/VP2 4096x2304, VP3 2048x1536; VP0+VP1 splice | observed 2026-11-28 (CRTCs, connectors); port limits read from source | `observed/rk3588-topology`; RK design §4 |
| Window-to-port policy | Vendor default: Cluster0, Esmart0, Cluster2, Esmart2 on the first display, the odd ones on the second; the device tree overrides it. Two outputs would need one lease each (RK design §5 option (a)) | read from source; lease option inferred | RK design §4, §5 |
| Class recognition | KMS exposes no window name. Candidate: plane `type` plus `IN_FORMATS` content (AFBC present means Cluster). Debugfs names were used in the recording only | inferred | RK design §7 Q3 |
| Cluster sub windows | Follow the main window, same modifier, 2048 wide; planes only with DT `support-multi-area` (12 planes then). Not present in the 8-plane recording | read from source | RK design §2, §4 |

Disagreement carried over: the vendor source names Esmart windows as Primary candidates, while the 6.18 board shows Cluster0 and Cluster1 as Primary (inferred to be a kernel difference). The profile must read `type` from KMS and not hard-code it.

## 2. Profile `Vc4Hvs5` (BCM2711)

Values come from `hvs-backend-design.md` sections 3 to 5. "HVS design" below means that document. The generation is chosen by this explicit variant, because both generations report driver name `vc4`.

| Constraint | Value | Tag | Doc |
| --- | --- | --- | --- |
| Generation selector | Explicit `DrmOptions::profile` variant `Vc4Hvs5`; the driver name `vc4` does not tell HVS5 from HVS6 | observed 2026-11-28 (driver name); design inferred | HVS design §2, §4 |
| Planes kept (`MAX_PLANES` cut) | The kernel has 60 planes; the probe kept 16 (6 primaries, 10 overlays) and 5 of the slots went to primaries of unconnected CRTCs. Proposed order: primary and cursor of each reachable CRTC first, then overlays, then writeback planes last. The DSI CRTC can use its primary plus 16 shared overlays (11 in the engine's cut view) | observed 2026-11-28 (counts); order inferred | HVS design §1, §4; `observed/bcm2711-cm4`, `observed/bcm2711-cm4-topology` |
| Scaling, non-cursor planes | `Some(..)` with values open. The kernel accepted a 2x scale-up and a 1/2 scale-down in `TEST_ONLY`; the claim in the profile is the smallest range a recorded run accepted. The driver imposes no bounds through the DRM helper (min 1, max `INT_MAX`); its own limit is a 21-bit TPZ ratio field | observed 2026-11-28 (`TEST_ONLY`, 2x up and 1/2 down accepted on a hand-built plan); driver limit read from source; the claim inferred | HVS design §4, §7 Q1; `observed/bcm2711-cm4` |
| Scaling, cursor plane | None | inferred | HVS design §6 test 2 |
| `max_source` | Width 8191 (`SCALER5_POS2_WIDTH_MASK`); height from the mode limit 7680 x 7680 | read from source (rpi-6.18.y @ `16ae2649`) | HVS design §4, §5 |
| Format/modifier priority | Linear first; T-tiled for XRGB, ARGB, ABGR, XBGR, RGB565, BGR565, ARGB1555, XRGB1555; SAND64, SAND128, SAND256 for NV12/NV21; SAND128 for P030. The probe lists 32 identical formats per plane and prints no modifier column | read from source (rpi-6.18.y @ `16ae2649`); format list observed 2026-11-28 | HVS design §4; `observed/bcm2711-cm4-topology` |
| Resource groups | LBM: 64 handles, 60 K words, consumed by vertical scaling. A group with consumer `SCALED_V` and capacity below 64 would be sound for handles but not words; `check` stays authoritative. No dlist group (321 words of 4064 at 16 layers) | read from source (limits); group design inferred | HVS design §3 |
| Transforms | The probe shows `rotation=1` (rotate-0 bit) on all planes in the state dump; the supported mask was not read on this board. By the Pi 5 record (same driver) the mask is 0/180 plus reflect X/Y, no 90/270 | observed 2026-11-28 (current value only); mask from the BCM2712 record, so inferred for BCM2711 | `observed/bcm2711-cm4-topology`; `observed/bcm2712-topology` |
| zpos | Primary fixed 0; overlays mutable 1..=17 | observed 2026-11-28 | `observed/bcm2711-cm4-topology` |
| Alpha and blend | Plane alpha and `none`, `premultiplied`, `coverage` on all reported planes | observed 2026-11-28 | `observed/bcm2711-cm4-topology` |
| Outputs | 6 CRTCs (`txp` writeback plus 5 pixelvalves); 3 HVS channels, so at most 3 active outputs. Pixelvalve-to-channel pinning: pv0 channel 0 only, VEC channel 1 only, TXP channel 2 only. The reTerminal has one connected output (`DSI-1`) | observed 2026-11-28 (CRTCs, connector); channel pinning read from source | HVS design §3, §5; `observed/bcm2711-cm4-topology` |
| Bandwidth | Core-clock rate computed and applied; no load-tracker check on this generation. A commit the bus cannot sustain fails by underrun, not in `TEST_ONLY`. `hvs_underrun` read 0 | read from source; underrun counter observed 2026-11-28 | HVS design §1, §5 |
| `replace_pending` | Each channel waits for the previous commit on it; whether a pending commit can be replaced is unknown, the source suggests `false` | read from source; conclusion inferred | HVS design §3, §7 Q6 |

## 3. Profile `Vc4Hvs6` (BCM2712)

| Constraint | Value | Tag | Doc |
| --- | --- | --- | --- |
| Generation selector | Explicit variant `Vc4Hvs6`; driver name `vc4` as on BCM2711. C0 vs D0 stepping is not visible and behaves the same toward userspace | observed 2026-11-28 (driver name); "same" inferred | HVS design §2, §4, §5 |
| Planes kept (`MAX_PLANES` cut) | The kernel has 56 planes: 4 primaries (one per CRTC), 16 shared overlays reaching CRTCs 1 to 3, 34 CRTC-0-only planes, and one extra plane per CRTC 1 to 3 (ids 646, 657, 668, inferred to be cursors). The probe dropped 40 of 56. The connected output (CRTC index 2) reaches its primary 83, the 16 overlays and plane 657. Same ordering proposal as HVS5 | observed 2026-11-28 (counts, masks); cursor role and order inferred | HVS design §1, §4; `observed/bcm2712-topology` |
| Plane arithmetic | 34 + 16 + 3 x 2 = 56 fits one writeback CRTC (1 primary, 1 cursor, 32 overlays), 16 shared overlays and three CRTCs with a primary and one extra plane. The BCM2712 file counts 4 primaries and 34 CRTC-0-only planes, which would give 57; the readings differ by one plane | observed 2026-11-28 (counts); arithmetic inferred | HVS design §5, §7 Q5 |
| Scaling, non-cursor planes | `Some(..)` with values open, same method as HVS5. The kernel accepted 2x scale-up and 1/2 scale-down in `TEST_ONLY` | observed 2026-11-28 (`TEST_ONLY`, hand-built plan); claim inferred | HVS design §4, §7 Q1; `observed/bcm2712` |
| `max_source` | None: the source has no width check on gen 6. Mode limit 8192 x 8192 | read from source (rpi-6.18.y @ `16ae2649`) | HVS design §4, §5 |
| Format/modifier priority | Linear first; SAND128 and SAND256 only, T-tiled and SAND64 rejected by `vc4_format_mod_supported`. Conflict: the `IN_FORMATS` blob observed on the Pi 5 lists 45 formats with 5 modifiers (Broadcom codes 1, 3, 4, 5 besides linear, read by the recording as T-tiled, SAND128 and others). Source and blob are not reconciled. Extra formats over HVS5: P010, P012, P016, S010, S012, S016 | read from source; blob observed 2026-11-28; disagreement inferred | HVS design §5; `observed/bcm2712-topology` |
| Resource groups | LBM: 1024 lines, reserved for vertical scaling or YUV scaling (`!is_unity \|\| is_yuv`). UPM: 32 handles, one per colour plane, so an NV12 layer needs two while a group charges one per layer. Conservative: capacity 16 for YUV, or leave to `check` | read from source (limits); group design inferred | HVS design §3 |
| Transforms | `rotate-0`, `rotate-180`, `reflect-x`, `reflect-y`; no 90/270 | observed 2026-11-28 | `observed/bcm2712-topology` |
| zpos | Primary immutable 0; overlays mutable 1..=17 (read values 1, 13 and 10 on three planes) | observed 2026-11-28 | `observed/bcm2712-topology` |
| Alpha and blend | `alpha` 0..65535 default 0xffff; `none`, `premultiplied` (default), `coverage` on all five planes read | observed 2026-11-28 | `observed/bcm2712-topology` |
| Outputs | 4 CRTCs (ids 59, 78, 94, 106; one active, driving `HDMI-A-1`), 2 TMDS and 2 virtual encoders, 3 HVS channels. Source: 2 display pixelvalves plus 2 writeback blocks (`mop`, `moplet`); HDMI0 channel 0, HDMI1 channel 1, `moplet` also channel 1, so HDMI1 and `moplet` exclude each other | observed 2026-11-28 (CRTCs, encoders); channel pinning read from source; reconciliation inferred | HVS design §3, §5; `observed/bcm2712-topology` |
| Bandwidth | Rate computed but never applied; no load-tracker check. Underrun not visible in `TEST_ONLY`. `hvs_underrun` read 0 | read from source; counter observed 2026-11-28 | HVS design §1, §5 |
| `replace_pending` | As HVS5: unknown, source suggests `false` | read from source; conclusion inferred | HVS design §7 Q6 |

## 4. Open questions still unanswered

Numbers are those of the design documents. All are unanswered as of this draft; none has board evidence in the repository.

RK3588 (`rk3588-backend-design.md` section 7):

- Q1: does `MAX_FORMATS` = 32 hide usable AFBC pairs on Cluster planes; full `IN_FORMATS` needed, and the linear-first ordering claim is unresolved.
- Q2: which kernel's values apply (vendor 6.1 or a 6.18-based kernel); table values must be tagged per kernel.
- Q3: how a profile identifies a window class.
- Q4: whether eight simultaneous windows are achievable (`TEST_ONLY` sweep).
- Q5: whether the planner should prefer Esmart for plain RGB (planner policy, outside the table).
- Q6: whether zpos is global or per CRTC, and behaviour on equal values.
- Q7: whether two leases give a second output (no second connected output was recorded).
- Q8: whether Cluster `win1` and Esmart multi-area become planes.
- Q9: overlay flip behaviour and `Latched` semantics, since nothing was committed.

HVS (`hvs-backend-design.md` section 7):

- Q1: `ScaleLimits` values for the profile; a `TEST_ONLY` sweep with growing ratios on a board.
- Q2: bandwidth, which neither `TEST_ONLY` nor the driver enforces.
- Q3: whether an explicit variant is enough for generation detection (design leans yes).
- Q4: the plane and format budget (`MAX_PLANES`, `MAX_FORMATS`); a core decision outside this work.
- Q5: BCM2712 CRTC and plane arithmetic and whether the extra planes are cursors.
- Q6: `replace_pending`.
- Q7: whether the source rows (`rpi-6.18.y` @ `16ae2649`) match the kernels the boards run.
- Q9: shared HVS resources across outputs, invisible to the planner.
- Q10: BCM2712 HDMI limits, unverified in the source record.

(HVS Q8, firmware KMS, is out of scope and not a question.)

## 5. Board results

Dated results will be written by a later item. Each subsection stays an empty slot until then; no value in sections 1 to 3 is a board result of these runs.

### NanoPC-T6

Not measured yet (pending t6-verify / rpi5-verify / cm4-verify).

### Raspberry Pi 5

Not measured yet (pending t6-verify / rpi5-verify / cm4-verify).

### reTerminal CM4

Not measured yet (pending t6-verify / rpi5-verify / cm4-verify).
