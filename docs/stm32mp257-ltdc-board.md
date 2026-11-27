# STM32MP257F-DK (LTDC): z-order and alpha through the KVM, and scaling bounds

Companion to [`observed/stm32mp257.md`](observed/stm32mp257.md) (the earlier
two-plane run, whose z-order and alpha were never seen on screen). Dates are
UTC from the host clock; the board's clock is not NTP-synchronised.

## Z-order and alpha seen through the KVM

**Status: not observed. The KVM failed again (2026-11-27).**

- The NanoKVM's MCP server (`kvm_kvm-stm32mp257f-dk`) did not connect when
  this item started. The relay answered `HTTP 404` on the MCP endpoint (the
  same failure `observed/stm32mp257.md` records for the earlier run). No
  screenshot tool was available, so no picture of the board's HDMI output was
  taken.
- No `twoplane` run was made in this session and no screenshot, `camera-check`
  verdict or `VERDICT` line exists for the normal phase, the swapped phase or
  the `--plane-alpha 32768` run. Running the harness without a picture would
  only repeat the plan/RESULT lines already in `observed/stm32mp257.md`, so
  the board was left alone.
- Board state: only read over ssh (`systemctl is-active
  weston-graphical-session seatd-weston` printed `active` twice at the start).
  Weston was never stopped, nothing was copied to the board, and nothing was
  changed.

| What | Result | Tag |
|---|---|---|
| z-order, normal phase | not seen on screen | KVM failed 2026-11-27 (HTTP 404) |
| z-order, swapped phase | not seen on screen | KVM failed 2026-11-27 (HTTP 404) |
| plane alpha 32768 | not seen on screen | KVM failed 2026-11-27 (HTTP 404) |

The z-order and alpha behaviour recorded in `observed/stm32mp257.md` remains
what the kernel accepted (plans, `RESULT`, `VERDICT` lines), not what was
displayed. Re-run the procedure when the KVM relay works: stop
`weston-graphical-session`, run `twoplane --swap-z` for 600 frames and take
screenshots at about 2 s and 7 s, then `--plane-alpha 32768`; judge them with
`scripts/camera-check.py --rotation 0` with full-frame corners (unit frac,
0..1) against the `EXPECT` lines; restore Weston.

## Scaling bounds

Run on 2026-11-27 (UTC, host clock; the board's clock is not NTP-synced and
printed 2026-11-27T10:23Z) with `examples/scale_probe` from main (no change
to the example was needed), over ssh, with `weston-graphical-session` stopped.
Kernel `6.6.129`, driver `stm`, mode 1920x1080@60Hz, destination fixed at
960x540 at (0,0), source buffer XR24, search resolution 256/65536. Every
test is `ATOMIC_TEST_ONLY | ALLOW_MODESET`; no commit was made.

Printed lines (observed 2026-11-27):

```text
RESULT plane=34 kind=Primary format=XR24 max_upscale_h=32.0000+ max_upscale_v=32.0000+ max_upscale_both=32.0000+ downscale=rejected min_downscale=none
RESULT plane=42 kind=Overlay format=XR24 max_upscale_h=32.0000+ max_upscale_v=32.0000+ max_upscale_both=32.0000+ downscale=rejected min_downscale=none
RESULT plane=49 kind=Overlay format=XR24 max_upscale_h=1.0000 max_upscale_v=1.0000 max_upscale_both=1.0000 downscale=rejected min_downscale=none
VERDICT scale_probe: OK 1 plane(s) probed at dst 960x540, resolution 256/65536   (per-plane runs, planes 34, 42, 49)
VERDICT scale_probe: OK 3 plane(s) probed at dst 960x540, resolution 256/65536   (default run, all planes)
```

| Plane | Largest accepted upscale (h / v / both) | 16.16 | First rejected | Downscale 0.5 / 0.75 / 0.9 | Observed |
| --- | --- | --- | --- | --- | --- |
| 34 Primary | at least 32.0 on every axis (ladder ended without a rejection) | 0x00200000 (2097152) or more | none up to 32.0 | all rejected (errno 22) on h, v, both | observed 2026-11-27 |
| 42 Overlay | at least 32.0 on every axis | 0x00200000 (2097152) or more | none up to 32.0 | all rejected (errno 22) | observed 2026-11-27 |
| 49 Overlay | 1.0 (h, v, both) | 0x00010000 (65536) | 1.25 on the ladder; the search narrowed to 1.0039 (65792) rejected on h, v and both, errno 22 | all rejected (errno 22) | observed 2026-11-27 |

Sizes used: at 32x both, source 30x16.875 to destination 960x540 (plane 42);
at 1.0, source 960x540 to 960x540 (plane 49); downscale sources 1920x1080,
1280x720 and about 1066.67x600 to 960x540.

* Observed 2026-11-27: upscaling was accepted at every factor tried up to
  32x on planes 34 and 42, so no upper bound was found; 32x is only the
  top of the probe ladder, not a limit. Larger factors were not tried.
* Observed 2026-11-27: on plane 49 any upscale above 1.0 (down to 1/256 of
  a pixel steps) was rejected with EINVAL on both axes and each axis alone.
* Observed 2026-11-27: no downscale was accepted on any of the three planes,
  on either axis or both.
* Inferred: planes 34 and 42 scale (upscale only) and plane 49 has no
  scaler. The TEST_ONLY result alone shows the rejection, not its cause.
  `observed/stm32mp257.md` saw 2x up accepted and 2x down rejected without
  isolating planes; this run isolates them.
* The rejection errno is EINVAL for every rejected test, so the downscale
  and plane-49 rejections were not distinguished from other EINVAL causes by
  errno; the destination was a partial rectangle for plane 34 (primary),
  which was accepted.

TEST_ONLY acceptance says nothing about picture quality, filtering or
bandwidth/underrun limits at large factors. No picture was taken in this run
and the KVM was not used.

Weston was restarted afterwards: `systemctl is-active
weston-graphical-session seatd-weston` printed `active` twice; the probe
binary was removed from the board's /tmp.
