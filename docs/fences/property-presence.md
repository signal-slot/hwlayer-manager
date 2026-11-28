# IN_FENCE_FD and OUT_FENCE_PTR in the recorded probes

Which recorded probes list the plane property `IN_FENCE_FD` and the CRTC
property `OUT_FENCE_PTR`. This is evidence for the driver-side detection in
[`../sync-fences.md`](../sync-fences.md) §3.2. It covers only what the
recorded files say about the kernel and board named in each row. Every row is
host-side reading of a recorded probe; none of it is a hardware milestone
claim (M1 to M3 are not claimed from probe files), and nothing is extrapolated
to other kernels, other boards of the same SoC or later driver versions.

The `Planes` column reads `N/M: ids`: N of the M plane sections in the file
list `IN_FENCE_FD`, and `ids` are those plane ids. The `CRTCs` column reads the
same for `OUT_FENCE_PTR`. A property listed here means the property exists on
the object; the recorded value is the default (`-1` for `IN_FENCE_FD`, `0` for
`OUT_FENCE_PTR`) and no fence was ever attached or signalled in any of these
runs, so nothing here says the driver honours a fence.

`scripts/check-fence-presence.sh` reads the files named in the table and fails
if a row's counts, ids or date disagree with the data, or if a recorded file
that lists planes or CRTCs has no row. Run it as
`sh scripts/check-fence-presence.sh`; its self-test is
`sh scripts/check-fence-presence.test.sh`.

## Presence per recorded topology

| SoC | Board, kernel, card | File | Date | Planes (IN_FENCE_FD) | CRTCs (OUT_FENCE_PTR) | Tag |
|---|---|---|---|---|---|---|
| BCM2711 (vc4, HVS5) | Raspberry Pi CM4 on Seeed reTerminal, kernel 6.12.47+rpt-rpi-v8, card0 | `crates/composition-drm/tests/data/bcm2711/probe-raw-2026-11-27.txt` | 2026-11-27 | 60/60: 48 67 79 91 103 115 127 138 149 160 171 182 193 204 215 226 237 248 259 270 281 292 303 314 325 336 347 358 369 380 391 402 413 424 435 446 457 468 479 490 501 512 523 534 545 556 567 578 589 600 611 622 633 644 655 666 677 688 699 710 | 6/6: 59 78 90 102 114 126 | observed 2026-11-27 (recorded probe) |
| BCM2711 (vc4, HVS5) | Raspberry Pi CM4 on Seeed reTerminal, kernel 6.12.47+rpt-rpi-v8, card0 | `crates/composition-drm/tests/data/bcm2711/props-2026-11-27.txt` | 2026-11-27 | 60/60: 48 67 79 91 103 115 127 138 149 160 171 182 193 204 215 226 237 248 259 270 281 292 303 314 325 336 347 358 369 380 391 402 413 424 435 446 457 468 479 490 501 512 523 534 545 556 567 578 589 600 611 622 633 644 655 666 677 688 699 710 | 6/6: 59 78 90 102 114 126 | observed 2026-11-27 (recorded probe) |
| BCM2712 (vc4, HVS6) | Raspberry Pi 5, kernel 6.18.50+rpt-rpi-2712, card1 | `crates/composition-drm/tests/data/bcm2712/probe-raw-2026-11-27.txt` | 2026-11-27 | 56/56: 48 67 83 95 107 118 129 140 151 162 173 184 195 206 217 228 239 250 261 272 283 294 305 316 327 338 349 360 371 382 393 404 415 426 437 448 459 470 481 492 503 514 525 536 547 558 569 580 591 602 613 624 635 646 657 668 | 4/4: 59 78 94 106 | observed 2026-11-27 (recorded probe) |
| BCM2712 (vc4, HVS6) | Raspberry Pi 5, kernel 6.18.50+rpt-rpi-2712, card1 | `crates/composition-drm/tests/data/bcm2712/props-2026-11-27.txt` | 2026-11-27 | 56/56: 48 67 83 95 107 118 129 140 151 162 173 184 195 206 217 228 239 250 261 272 283 294 305 316 327 338 349 360 371 382 393 404 415 426 437 448 459 470 481 492 503 514 525 536 547 558 569 580 591 602 613 624 635 646 657 668 | 4/4: 59 78 94 106 | observed 2026-11-27 (recorded probe) |
| RK3588 (rockchip VOP2) | NanoPC-T6, kernel 6.18.55-current-rockchip64, card0 | `crates/composition-drm/tests/data/rk3588/probe-raw-2026-11-27.txt` | 2026-11-27 | 8/8: 33 39 45 51 57 63 69 75 | 2/2: 81 82 | observed 2026-11-27 (recorded probe) |
| RK3588 (rockchip VOP2) | NanoPC-T6, kernel 6.18.55-current-rockchip64, card0 | `crates/composition-drm/tests/data/rk3588/props-2026-11-27.txt` | 2026-11-27 | 8/8: 33 39 45 51 57 63 69 75 | 2/2: 81 82 | observed 2026-11-27 (recorded probe) |
| RK3588 (rockchip VOP2) | NanoPC-T6, kernel 6.18.55-current-rockchip64, card0 | `crates/composition-drm/tests/data/rk3588/verify-probe-2026-11-28.txt` | 2026-11-28 | 8/8: 33 39 45 51 57 63 69 75 | 2/2: 81 82 | observed 2026-11-28 (recorded probe) |
| i.MX95 (imx95-dpu) | Verdin iMX95, kernel 6.6.157-7.8.0, card0 | `docs/imx95-probe-2026-11-28.txt` | 2026-11-28 | 7/7: 31 42 51 59 67 75 83 | 2/2: 39 50 | observed 2026-11-28 (recorded probe) |
| i.MX8M Plus (imx-drm, LCDIF) | Armadillo-X2, kernel 6.12.105-0-at, card1 | `docs/observed/imx8mp-x2-2026-11-28/probe-card1.txt` | 2026-11-28 | 2/2: 32 35 | 2/2: 34 37 | observed 2026-11-28 (recorded probe) |
| i.MX8M Plus (imx-drm, LCDIF) | Armadillo-X2, kernel 6.12.105-0-at, card1 | `docs/observed/imx8mp-x2-2026-11-28/props-card1.txt` | 2026-11-28 | 2/2: 32 35 | 2/2: 34 37 | observed 2026-11-28 (recorded probe) |

## Recorded files that list no plane or CRTC properties

These files are recorded probes of the same boards but carry no per-plane or
per-CRTC property listing, so they say nothing about the fence properties.
They have no row above and the check does not require one.

- `crates/composition-drm/tests/data/bcm2711/verify-probe-2026-11-28.txt` and
  `crates/composition-drm/tests/data/bcm2712/verify-probe-2026-11-28.txt`
  (observed 2026-11-28 (recorded probe)): the summary form of `probe` (plane
  type, zpos, formats), without the `--raw` property sections. The properties
  of those boards are in the 2026-11-27 rows above.
- `docs/observed/imx8mp-x2-2026-11-28/probe-card0.txt` and `props-card0.txt`
  (observed 2026-11-28 (recorded probe)): `card0` of the Armadillo-X2 answered
  "Not supported (os error 95)" to the probe and to `props`, so no plane or
  CRTC of `card0` was listed.

## SoCs with no recorded probe of the fence properties

- STM32MP257 (LTDC, `stm`): the recorded dump in
  `docs/observed/stm32mp257.md` is a condensed `modetest -p` excerpt,
  observed 2026-11-27 (recorded probe). It lists `COLOR_ENCODING`, `COLOR_RANGE`,
  `alpha`, `zpos` and `rotation` on planes 34, 42 and 49 and `VRR_ENABLED`,
  `GAMMA_LUT` and `GAMMA_LUT_SIZE` on CRTC 41, and omits the generic atomic
  properties. It therefore neither lists nor excludes `IN_FENCE_FD` and
  `OUT_FENCE_PTR`; this document makes no claim for the STM32MP257.
- AM62P (tidss): no recorded file in `crates/composition-drm/tests/data/**`,
  `docs/observed/**` or `docs/*probe*` lists plane or CRTC properties of its
  DRM device. No claim.
- Every other SoC (RA8P1, RT1170, STM32H7 and any not named in the table):
  no recorded probe, no claim.
- vkms and any QEMU or host DRM device: no file under the paths above
  records one. `scripts/testdata/probe-table/vkms-sample.txt` is a test input
  of `scripts/probe-table.py`, not a recorded probe, and is not used here.

The session documents (`docs/*-session-*.md`, `docs/linux-board-scenarios-*.md`)
were searched too. The only fence-property text in them is prose in
`docs/rk3588-vop2-session-2026-11-28.md` and `docs/observed/rk3588-props.md`
(the RK3588 rows above); both agree with the recorded files. No session
document embeds another probe output that lists these properties.

## What the rows do not show

- Kernel versions: a row holds for the kernel in that row only. The same SoC
  on another kernel, or the same driver on another SoC (for example vc4 on
  BCM2711 and BCM2712 are separate rows), is not covered.
- Behaviour: presence of `IN_FENCE_FD` says the plane accepts the property in
  an atomic commit; whether the driver waits on the fence, and when
  `OUT_FENCE_PTR` signals, is not in these files.
- Hardware milestones: M1 to M3 are not claimed from these rows.
