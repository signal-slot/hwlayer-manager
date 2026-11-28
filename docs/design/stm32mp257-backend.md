# Design note: STM32MP257 LTDC backend (composition-drm)

Status: design only. This note makes no measurement and changes no code.

## Tags and sources

Every hardware statement carries one tag:

- **observed DATE** (a YYYY-MM-DD date, e.g. 2026-11-28): seen on the STM32MP257F-DK in a session recorded in
  [`../observed/stm32mp257-topology.md`](../observed/stm32mp257-topology.md)
  (2026-11-28) or [`../observed/stm32mp257.md`](../observed/stm32mp257.md)
  (2026-11-27, re-run 2026-11-28). Dates are the host clock's.
- **source**: read from ST kernel source (`ltdc.c` and companions of
  `STMicroelectronics/linux`, branch `v6.18-stm32mp`, commit `3056405e`), per
  [`../observed/stm32mp257-source.md`](../observed/stm32mp257-source.md),
  read 2026-11-28. The board runs ST's 6.6.129, not that tree; that the code
  is the same is inferred.
- **inferred**: a conclusion of this note, neither seen nor stated.

The board was reached over ssh only. Its console and reset were away.

**Caveat carried over: pixel colours on screen are unverified.** The KVM (a
NanoKVM) failed to connect in every session (HTTP 404 from its relay), so no
screenshot exists. Z-order and alpha are known only as what the kernel
accepted (TEST_ONLY and commit), the planner's plans, the harness's event
accounting and the kernel's atomic state. The `EXPECT` colour lines of the
harness have never been compared with a picture.

## 1. Hardware summary

| Item | Statement | Tag |
|---|---|---|
| Kernel, driver | Linux 6.6.129 (ST OpenSTLinux), DRM driver `stm` 1.0.0, `/dev/dri/card0` (LTDC) | observed 2026-11-27, 2026-11-28 |
| Output | one connector `HDMI-A-1` into the KVM, 1920x1080@60.000 Hz, 148.5 MHz | observed 2026-11-28 |
| CRTCs | 1 | observed 2026-11-28; source (`NB_CRTC 1`) |
| Planes | 3: primary (layer 0) and two overlays, no cursor plane, `possible_crtcs` `0x1` on all | observed 2026-11-28; source |
| Layer count | read at run time from `LTDC_LCR`, clamped to 1..4; a layer behind a denied firewall is not created | source |
| Display client | Weston held the CRTC; `probe` and `modetest -p` read without master | observed 2026-11-28 |

## 2. Mapping onto composition-drm

| Aspect | Hardware | What composition-drm reports | Gap |
|---|---|---|---|
| Planes and outputs | 3 planes, one CRTC, mask `0x1` | 3 planes, `outputs 0b1` | None. Two planes on one output work: harness committed it (observed 2026-11-27) |
| Formats | plane 34: 21 (AR24 XR24 AB24 XB24 RA24 RX24 BA24 BX24 RG16 BG16 RG24 BG24 AR15 XR15 AR12 XR12 C8 YUYV YVYU UYVY VYUY); plane 42: those plus NV12 NV21 YU12 YV12 (25); plane 49: RGB and C8 only (17) | same lists | C8 prints `C8??`: the engine has no palette format (observed 2026-11-28). Source agrees with the lists and adds NV12-family only to layers with a nonzero index on MP25 |
| Modifiers | `LINEAR` only, every format, every plane | linear only | None (observed 2026-11-28; source) |
| zpos | mutable range 0..=2 on all three, defaults 0, 1, 2 | `zpos 0..=2` | Writing zpos was never exercised by a commit (see section 6) |
| Plane alpha | `alpha` 0..65535 on all planes | `plane alpha true` | None; 0x8000 is commit-only evidence (observed 2026-11-27) |
| Blend mode | no `pixel blend mode` property on any plane | `blend [premultiplied]`, filled in by composition-drm when the property is absent | Whether LTDC blends premultiplied or straight is **inferred**; source has no blend-mode property either. Coverage or none is refused by the planner (`NoPlane BLEND`, observed 2026-11-27) |
| Scaling | no KMS scaling property | `scaling not claimed` | Section 3 |
| Rotation | plane 34: rotate-0/90/180/270, reflect-x/y; planes 42 and 49: rotate-0, reflect-x/y | none programmed (`NoPlane TRANSFORM`) | Feature gap in composition-drm, not a profile item (observed 2026-11-27). Source: 90/270 need the rotator (MP255 only) and `rotation-memory`, and exclude other layers |
| YUV properties | `COLOR_ENCODING` (BT.601, BT.709) and `COLOR_RANGE` on planes 34 and 42 | not read | observed 2026-11-28; source: BT.2020 unsupported, falls back to BT.601 limited |
| Writeback | none | none | observed 2026-11-27 |

## 3. Scaling

- Source: scaling is a per-layer hardware ability (`LXCR_C1R_SCA`, read from
  the layer at run time). Upscale only; downscaling is refused; a layer
  without the scaler refuses any size mismatch; a YUV format with a size
  change is refused. These checks are in `ltdc_plane_atomic_check`, so
  TEST_ONLY sees them. The factor is 12 fractional bits masked to 16 bits
  with no numeric cap in the code. The horizontal and vertical phase
  registers look cross-assigned in `ltdc_plane_update`; unverified.
- Observed 2026-11-27 (`../observed/stm32mp257.md`): TEST_ONLY accepted a
  **2x upscale** and rejected a **2x downscale** (6 candidates, all
  rejected). Only 2x was tried there, so **the upper limit is not measured
  by the two observed documents this note is based on**. The harness could
  not say which plane scaled (inferred: at least one of three).
- With no claim, capability detection and the planner refuse any scaled
  layer (`NoPlane SCALING`) and TEST_ONLY is never asked. KMS gives no
  signal, so scaling must come from a profile or from probing with
  TEST_ONLY.
- Bandwidth and FIFO underrun are not checked at atomic-check time (source),
  so TEST_ONLY can pass a combination that underruns (inferred). The LTDC
  error counters stayed 0 in the runs (observed 2026-11-27, 2026-11-28).

Repository documents newer than those two go further and are not relied on
here: [`../stm32mp257-ltdc-board.md`](../stm32mp257-ltdc-board.md) reports
per-plane scaling (planes 34 and 42 accept upscaling up to the mode size
with no rejection, plane 49 only 1.0, no downscale anywhere; observed
2026-11-27 there). A board session should confirm them rather than copy them.

## 4. DriverProfile

`DriverProfile::Stm32Ltdc` in `crates/composition-drm/src/profile.rs` claims
(read from source, 2026-11-28, of that file):

- scaling: the primary plane and the first overlay (by kind and order) get
  upscale only (`max_downscale = 1.0`, `max_upscale` a nominal 1080x, not a
  measured bound); later overlays and the cursor get no scaling claim, so a
  scaled layer on them is refused (`NoPlane SCALING`). Evidence: plane 42
  upscales to the mode size, plane 49 does not scale, nothing downscales
  (observed 2026-11-27, session A, Q3 in
  [`../stm32mp257-ltdc-session-2026-11-28.md`](../stm32mp257-ltdc-session-2026-11-28.md)).
  The primary's scaling was not probed in sessions A and B (inferred from
  the layer-0 register fact);
- blend: **straight (coverage) alpha** on every non-cursor plane, replacing
  the generic "premultiplied" the library reports when the property is
  absent (commit `ca5393a`). The planner therefore refuses premultiplied
  layers on these planes. Evidence: the output CRC on plane 42 (AR24) fits
  pixel alpha times plane alpha as straight alpha (observed 2026-11-27; the
  formula is inferred). Planes 34 and 49 are **assumed** the same
  (inferred); no picture confirms any of it. An ARGB overlay that must
  ignore alpha still needs an XRGB buffer (inferred: no blend-mode property
  exists to select "none");
- no rule for plane 49 lacking YUV (the format lists carry it);
- transforms are not claimed (`profile.rs` adds none and leaves what KMS
  reports; pinned by the host test `stm32_ltdc_adds_no_transform_claims`;
  never 90/270). Session C took no picture, so no evidence supports a
  change (observed 2026-11-28).

State at this commit: the profile above is in the tree, built on the
session A and B measurements. The transform claims proposed in sessions A and B (reflect-x on
plane 42 only, nothing on the primary, never 90/270) are **not applied**:
they wait for pictures (Q6).
Two design points:

- Which overlay has a scaler is a register fact, seen on one board (layers 0
  and 1; inferred for others). Planes should be identified by kind and order,
  not KMS id, and the profile stays explicitly selected, not chosen from the
  driver name.
- YUV scaling is refused by source and is not modelled; TEST_ONLY decides.

## 5. Unsupported combinations: who says no

From the harness's `--unsupported` run (observed 2026-11-27, same results on
the 2026-11-28 re-run):

| Combination | Capability detection | Planner | TEST_ONLY |
|---|---|---|---|
| Unknown format | refuses | `NoPlane FORMAT` | not reached |
| Tiled modifier | refuses (LINEAR only) | `NoPlane FORMAT` | not reached |
| 4 layers on 3 planes | refuses | `NoPlane IN_USE` on layer 3 | not reached |
| 2x upscale, no claim | refuses | `NoPlane SCALING` | accepts when claimed by hand |
| 2x downscale | refuses | `NoPlane SCALING` | rejects all 6 candidates |
| Rotate180 | refuses (no transforms reported) | `NoPlane TRANSFORM` | not reached |
| Coverage or none blend | refuses | `NoPlane BLEND` | not reached |
| Plane alpha 0x8000 | accepts | plan | ok |
| Partly off-screen, odd size | accepts | plan (clipped) | ok |

Source adds that the 90/270 buffer-size and `vdisplay` > 1366 refusals sit in
`ltdc_plane_update`, past TEST_ONLY, after the controller was disabled, so
TEST_ONLY can pass and the commit leave the display stopped (source; the
consequence is inferred). A first backend should therefore not program
90/270.

## 6. What the backend has been shown to do

Observed 2026-11-27: background XR24 1920x1080 on plane 34, AR24 384x540
overlay at 960,270 on plane 42, plane 49 unused; 600 frames committed, 600
latched, 1198 released, 0 dropped, 0 late; latch interval mean 16.667 ms,
max 17.446 ms; every buffer use released once. `--swap-z` the same (max
17.421 ms). Re-run 2026-11-28: same plans and verdicts, 0 dropped, 0 late,
max 17.970 ms and 17.411 ms. The kernel's atomic state agreed with the plans;
LTDC `transfer_error`, `fifo_underrun_error` and `fifo_underrun_warning`
stayed 0; no DRM or LTDC messages in `dmesg` after boot.

The swap was done by moving layers between planes, not by writing crossed
zpos values (inferred from plan lines and `normalized-zpos`). A backend may
rely on plane order following zpos defaults 0, 1, 2 (observed), but the
property being writable was not exercised by a commit.

## 7. Open questions for a board session

The console and reset are away: a session is ssh only and must not reboot,
touch boot media or U-Boot. Weston holds DRM master and is stopped only for
the run and restored afterwards (this worked on 2026-11-27).

Sessions A and B are in
[`../stm32mp257-ltdc-session-2026-11-28.md`](../stm32mp257-ltdc-session-2026-11-28.md)
(summary table at its end) and session C (2026-11-28: no picture, outputs
read over ssh). Each question below is **answered** or **open**.

1. **Pixel colours.** Status: **open**. Session C was to take the pictures,
   but the KVM capture failed again (HTTP 404 from the MCP relay, observed
   2026-11-28), so no picture exists. Wanted: screenshots in both
   `--swap-z` phases and with `--plane-alpha 32768`, compared with the
   `EXPECT` lines. See
   [session C](../stm32mp257-ltdc-session-c-2026-11-28.md#part-1-q1-pixel-colours--not-run)
   and [the rehearsal](../observed/stm32mp257-kvm.md).
2. **Premultiplied or straight alpha.** Status: **answered**. Straight alpha
   on plane 42, pixel alpha times plane alpha, from the output CRC (observed
   2026-11-27; the formula is inferred). See
   [Q2](../stm32mp257-ltdc-session-2026-11-28.md#q2-premultiplied-or-straight-alpha).
3. **Scaling upper bound and quality.** Status: **answered**. Plane 42
   upscales to the mode size, plane 49 does not scale, nothing downscales,
   the scaler filters, underrun warnings only while a scaled layer is shown
   (observed 2026-11-27); plane 34 was not probed. See
   [Q3](../stm32mp257-ltdc-session-2026-11-28.md#q3-scaling-bounds-quality-and-underrun).
4. **Scaler layers on other MP25 variants or firewall settings** (register
   fact, one board seen). Status: **answered** from source only. No file
   names the layers that scale; the driver reads `LXCR_C1R_SCA` per layer at
   probe, code and device trees are the same for all variants, and a
   firewall refusal of layer 3 removes that plane but does not change which
   layers scale (read from source). The public reference manual was not
   read; identical silicon is inferred. See
   [session C, Q4](../stm32mp257-ltdc-session-c-outputs-2026-11-28.md#q4-which-layers-have-a-scaler-on-other-stm32mp25-variants).
5. **Writing zpos.** Status: **answered**. Crossed zpos passes TEST_ONLY and
   commits and the hardware follows it (observed 2026-11-27). See
   [Q5](../stm32mp257-ltdc-session-2026-11-28.md#q5-writing-zpos-crossed-values-on-the-fixed-planes).
6. **Rotation.** Status: **open**. Seen so far (observed 2026-11-27, see
   [Q6](../stm32mp257-ltdc-session-2026-11-28.md#q6-rotation-180-and-reflections)):
   overlay reflect-x and reflect-y accepted, `rotate-180/90/270` EINVAL;
   primary 180 and reflections commit, 90/270 pass TEST_ONLY; primary
   reflect-x/180 and overlay reflect-x+reflect-y give an unstable CRC.
   Missing: the effect in a picture, plane 49 (not tested), and whether a
   failure after a passing TEST_ONLY leaves the display stopped (none seen).
   Session C took no picture (attempted 2026-11-28, KVM capture failed), so
   it stays open; see
   [session C, part 2](../stm32mp257-ltdc-session-c-2026-11-28.md#part-2-q6-transforms-nv12-colours-plane-34-scaling--not-run).
   How reflect-x, reflect-y and 180 are programmed, with predicted
   pictures, is in
   [`stm32mp257-ltdc-transform-source.md`](../stm32mp257-ltdc-transform-source.md)
   (read from source).
7. **Underrun under load.** Status: **answered**. No `fifo_underrun_error` or
   `transfer_error` under CPU and memory load with three planes and a scaled
   layer; warnings only with scaling (observed 2026-11-27). See
   [Q7](../stm32mp257-ltdc-session-2026-11-28.md#q7-underrun-under-load).
8. **Other outputs.** Status: **answered**. One connector `HDMI-A-1`, one
   encoder, one CRTC. The encoder drives a DSI host and an ADV7535 bridge
   chain, which is the live route. An LVDS route exists in the device tree
   but is `disabled` and not bound, so it needs a device-tree change
   (observed 2026-11-28). See
   [session C, Q8](../stm32mp257-ltdc-session-c-outputs-2026-11-28.md#q8-other-outputs).
9. **C8 and YUV.** Status: **answered**. NV12 commits on plane 42 at 1:1 and
   is scanned out, refused on plane 49 and when scaled; C8 is offered on all
   three planes, not tried (observed 2026-11-27; NV12 colours not judged).
   See
   [Q9](../stm32mp257-ltdc-session-2026-11-28.md#q9-nv12-on-plane-42-and-c8).
10. **Source gap.** Status: **answered**. The 6.6 plane path matches the 6.18
    rows except compatible strings and minor items; the board kernel is
    probably older than the 6.6 tip read (read from source, observed
    2026-11-27, inferred). See
    [Q10](../stm32mp257-ltdc-session-2026-11-28.md#q10-st-v618-source-rows-against-the-boards-66129).
11. **Clock.** Status: **answered**. The board clock is not NTP-synchronised;
    sessions A and B recorded host UTC for every run (observed 2026-11-27).
    See
    [conditions](../stm32mp257-ltdc-session-2026-11-28.md#tags-clock-and-conditions).

## 8. Not done here

No code, no change to `docs/platform-capabilities.md` or `docs/observed/`,
no new measurement. No board address, host name, serial or credential is
recorded here.
