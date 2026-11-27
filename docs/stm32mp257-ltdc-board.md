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

pending (scale-bounds item)
