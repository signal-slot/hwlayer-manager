# Evidence index: RK3588 and HVS open questions

Written 2026-11-28 (host UTC) on the host, from the docs on the integration tip. No board was used and nothing was run. It maps each open question of section 7 of [`rk3588-backend-design.md`](rk3588-backend-design.md) and [`hvs-backend-design.md`](hvs-backend-design.md) to the docs that hold evidence for it. It states no result of its own.

Tags in the evidence column are the linked doc's own tag for that evidence: **observed** (with the date the doc gives), **read from source**, or **inferred**. Status is one of "answered by <doc>", "source read only, board session <item> pending" or "still open". Only docs that exist on the tip are linked. Plan items that have not delivered yet are named by description, without a link:

- the NanoPC-T6 `TEST_ONLY` sweep session up to 8 windows;
- the NanoPC-T6 overlay flip/latch session with `pending_probe` and KVM pictures;
- the NanoPC-T6 `zpos_probe` session (equal and cross-CRTC zpos);
- the RPi5 session B (HVS6 `replace_pending`, downscale ladder, extra plane types, `TEST_ONLY` HDMI modes);
- the reTerminal session B (HVS5 `replace_pending` and downscale ladder);
- the host comparison of the vc4 source rows with the RPi boards' running kernels;
- the source read of BCM2712 HDMI pixel-clock and mode limits.

## RK3588 (`rk3588-backend-design.md` section 7)

| Q | Question | Evidence docs and tags | Status |
|---|---|---|---|
| Q1 | Does `MAX_FORMATS = 32` hide AFBC pairs on Cluster planes; is linear first? | [`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md) (observed 2026-11-28: 126 pairs per Cluster plane, 94 beyond the cap, linear not listed); [`rk3588-vop2-formats-source.md`](rk3588-vop2-formats-source.md) (read from source, inferred) | answered by [`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md) |
| Q2 | Which kernel does the target run, vendor 6.1 or 6.18-based? | [`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md) (observed 2026-11-28: mainline-based 6.18.55); [`rk3588-kernel-compare.md`](rk3588-kernel-compare.md) (read from source, inferred: source rows against the running kernel); [`rk3588-mainline-source.md`](rk3588-mainline-source.md), [`rk3588-vendor-vop2-source.md`](rk3588-vendor-vop2-source.md) (read from source) | answered by [`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md) and [`rk3588-kernel-compare.md`](rk3588-kernel-compare.md) |
| Q3 | How does a profile identify a window class? | [`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md) (observed 2026-11-28: by the `IN_FORMATS` modifier vendor; the doc leaves the supported rotation bits open); [`observed/rk3588-props.md`](observed/rk3588-props.md) (observed 2026-11-28); [`rk3588-vop2-formats-source.md`](rk3588-vop2-formats-source.md) (read from source) | answered by [`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md) (rotation bits still open there); a source read of KMS-visible window-class properties is not yet on the tip |
| Q4 | Are eight simultaneous windows possible? | [`rk3588-mainline-source.md`](rk3588-mainline-source.md) (read from source: no cap on active windows in the three files read; says the eight-window case needs more than one port); [`rk3588-vop2-limits-source.md`](rk3588-vop2-limits-source.md) (read from source) | source read only, board session NanoPC-T6 `TEST_ONLY` sweep up to 8 windows pending |
| Q6 | Is zpos global or per CRTC; what happens on equal zpos? | [`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md) (observed 2026-11-28: partial data, equal zpos and per-CRTC zpos not measured; `kms_sweep` cannot request them); [`rk3588-mainline-source.md`](rk3588-mainline-source.md) section 7 and [`rk3588-vop2-limits-source.md`](rk3588-vop2-limits-source.md) (read from source: range 0..7, initial value is the window id) | source read only, board session NanoPC-T6 `zpos_probe` (equal and cross-CRTC zpos) pending; the source read of zpos with equal values and across CRTCs is not yet on the tip |
| Q8 | Do Cluster `win1` and Esmart multi-area become planes? | [`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md) (observed 2026-11-28: 8 planes, no `win1`, no multi-area planes on this kernel); [`observed/rk3588-topology.md`](observed/rk3588-topology.md) (observed 2026-11-28); [`rk3588-mainline-source.md`](rk3588-mainline-source.md) (read from source) | answered by [`rk3588-vop2-session-2026-11-28.md`](rk3588-vop2-session-2026-11-28.md) |
| Q9 | Overlay flip and `Latched` semantics, since nothing was committed? | [`drm-pending-commit-source.md`](drm-pending-commit-source.md) (read from source: DRM core and VOP2 flip events); [`rk3588-mainline-source.md`](rk3588-mainline-source.md) (read from source) | source read only, board session NanoPC-T6 overlay flip/latch with `pending_probe` and KVM pictures pending |

## HVS (`hvs-backend-design.md` section 7)

HVS6 is BCM2712 on the RPi5. HVS5 is BCM2711 on the reTerminal CM4.

| Q | Question | Evidence docs and tags | Status |
|---|---|---|---|
| Q1 | `ScaleLimits`: `TEST_ONLY` with growing ratios (only 2x up and 1/2 down were tried) | [`vc4-scale-limits-source.md`](vc4-scale-limits-source.md) (read from source, inferred: what the sweeps should try); [`vc4-hvs-scaling-source.md`](vc4-hvs-scaling-source.md) (read from source); [`observed/bcm2711-cm4.md`](observed/bcm2711-cm4.md) and [`observed/bcm2712.md`](observed/bcm2712.md) (observed 2026-11-28, the 2x and 1/2 runs) | source read only, board sessions RPi5 session B (HVS6 downscale ladder) and reTerminal session B (HVS5 downscale ladder) pending; the first scale-limit sessions of each board are in progress and not yet on the tip |
| Q5 | BCM2712 CRTC and plane arithmetic; are the extra per-CRTC planes cursors? | [`bcm2712-plane-count-source.md`](bcm2712-plane-count-source.md) and [`bcm2712-dt-source.md`](bcm2712-dt-source.md) (read from source, inferred); [`observed/bcm2712-topology.md`](observed/bcm2712-topology.md) (observed 2026-11-28: 4 CRTCs, 56 planes); [`observed/bcm2712-recheck.md`](observed/bcm2712-recheck.md) (observed 2026-11-28) | source read only, board session RPi5 session B (extra plane types) pending |
| Q6 | `replace_pending`: can an unlatched commit be replaced, and what is reported? | [`vc4-replace-pending-source.md`](vc4-replace-pending-source.md) (read from source; no hardware result claimed); [`drm-pending-commit-source.md`](drm-pending-commit-source.md) (read from source) | source read only, board sessions RPi5 session B and reTerminal session B (`replace_pending`) pending |
| Q7 | Do the source rows at `rpi-6.18.y` `16ae2649` match the boards' running kernels? | Running kernels: [`observed/bcm2712-recheck.md`](observed/bcm2712-recheck.md) (observed 2026-11-28: `6.18.50+rpt-rpi-2712`) and [`observed/bcm2711-cm4-x09-2026-11-28.md`](observed/bcm2711-cm4-x09-2026-11-28.md) (observed 2026-11-28: `6.12.47+rpt-rpi-v8`); pins in [`vc4-scale-limits-source.md`](vc4-scale-limits-source.md) (read from source) | still open: the kernels are not the pinned build; the host comparison of the vc4 source rows with the RPi boards' running kernels is not yet on the tip |
| Q10 | BCM2712 HDMI limits (600 MHz) | [`observed/bcm2712-topology.md`](observed/bcm2712-topology.md) (observed 2026-11-28: one active 1920x1080 @ 60 mode, 148.5 MHz; no high-clock mode tried) | still open: the source read of BCM2712 HDMI pixel-clock and mode limits is not yet on the tip, and RPi5 session B (`TEST_ONLY` HDMI modes) is pending |

For reference, [`rk3588-vc4-board-procedure.md`](rk3588-vc4-board-procedure.md) and [`rk-hvs-session-checklist.md`](rk-hvs-session-checklist.md) describe how the board sessions are run.
