# M1 test report: i.MX95 Linux DRM hardware composition

Written 2026-11-28 from the repository at the integration tip (`a491dbc`). Covers SoW §9 M1, conditions 1-7 and the exit rule. All hardware results below were observed on 2026-11-27 (master-fd and lease runs) and 2026-11-28 (topology probe) on one board. Nothing from host, CI or vkms runs is counted as hardware evidence; where such a run is cited, it is labelled as such.

**The SoW says "EVK". The board used is not an EVK:** it is a Toradex Verdin iMX95 module on a Mallow carrier. SoW §11 allows "an NXP i.MX95 EVK or equivalent i.MX95 board with working display, documented board revision, and Linux BSP". Whether this board counts as "equivalent" is the requester's call; this report only records what was used.

Detail pages (this report summarises them; the logs and pictures next to them are the evidence): [`m1-imx95/master-run.md`](m1-imx95/master-run.md), [`m1-imx95/lease-run.md`](m1-imx95/lease-run.md), [`platform-capabilities.md`](platform-capabilities.md), [`imx95-probe-2026-11-28.txt`](imx95-probe-2026-11-28.txt), test plan [`m1-imx95-test-plan.md`](m1-imx95-test-plan.md).

## Overall verdict

Overall M1 verdict: passed (exit rule met: real multi-plane composition on i.MX95 verified on the board, condition 3). Conditions 4 and 7 are passed with stated gaps, listed under their verdicts and in Follow-ups. Date of verdict: 2026-11-28; hardware evidence from 2026-11-27.

## Condition 7 fields

Each value is measured on the board unless it says "not measured". Rotation figures are from the `rotate` scenario (3 buffers per plane, two planes).

- Board revision: Toradex Verdin iMX95 WB module on a Mallow carrier (`/proc/device-tree/model`: "Toradex Verdin iMX95 WB on Verdin Development Board"), SoC i.MX95 revision 2.0 (`/sys/devices/soc0`), read 2026-11-27 and 2026-11-28. Module and carrier PCB revisions: not measured (not read from the board). Not an NXP EVK.
- Panel and interface: Toradex Capacitive Touch Display 10.1" on the LVDS connector X12; DRM connector `LVDS-1` (id 41), encoder 40, CRTC 39; one mode. Observed 2026-11-27 (harness `output` record) and 2026-11-28 (probe).
- BSP/kernel version: Toradex Reference Multimedia Image 7.8.0 (`TDX Wayland with XWayland 7.8.0+build.17 (scarthgap)`, Yocto, not Torizon); kernel `6.6.157-7.8.0` (`#1 SMP PREEMPT Fri Sep 25 14:06:41 UTC 2026`); DRM driver `imx95-dpu` 1.0.0; Weston 12.0.4 (stopped for the runs, restarted and checked afterwards). Driver source read at kernel commit `64f85d65c112` (`linux-toradex`, `imx95-dpu-source-6.6.md`). Observed 2026-11-27.
- Lease route and version: (a) master descriptor, no lease, 2026-11-27 19:33-19:51 UTC; (b) kernel lease (`DRM_IOCTL_MODE_CREATE_LEASE`, lessor in the harness process), 2026-11-27 20:08-20:12 UTC; (c) AGL drm-lease-manager 0.1.0 at commit `68c48758c46d` (libdrm `25dec5b91fe4`, tomlc99 `62ce93ceb713`), built with `scripts/build-dlm-aarch64.sh`, run from `/tmp` with a config of `LVDS-1` and planes 31, 51, 2026-11-27 20:18-20:21 UTC; (d) the harness's own `lessor` stand-in speaking the drm-lease-manager protocol, 2026-11-27 20:12-20:13 UTC (not the daemon). libdlmclient: not used (the harness has its own client; upstream `dlm-client-test` was run once). No Weston fork was used; Weston was stopped during every run.
- Resolution and refresh: 1280x800, mode refresh 59.993 Hz (`refresh_mhz=59993`, pixel clock 71 100 kHz, period 16 668 µs). Measured latch interval (`rotate`, 3000 frames, master): median 16 668 µs, min 16 388, p99 16 892, max 16 972. 2026-11-27.
- Plane assignments: two layers on two distinct planes of CRTC 39: layer 0 (bottom) on primary plane 31 (zpos 0), layer 1 (top) on overlay plane 51 (zpos 1); `assignment_changes=0` in every `rotate`; the z-order swap is done by swapping the assignment between 31 and 51. Up to six planes (31, 51, 59, 67, 75, 83) were accepted by `TEST_ONLY` in `combos` (no commit). Through drm-lease-manager with `planes = [31, 51]` the lessee sees exactly 31 and 51. 2026-11-27. Full table: `m1-imx95/master-run.md`, "Plane assignments".
- Errors: no non-zero errno in any commit on any route; kernel log (`dmesg`) identical before and after each session. Expected or operator-caused failures only: a second DRM master refused with `EBUSY` (operator error, 2026-11-27 19:39:51 UTC), `--buffers 2` refused (harness minimum is 3), a second drm-lease-manager client refused (`ERROR: Can't fulfill lease request`), `revoke` refused for `--source dlm` by design, and a `rotate` FAIL with the lessor killed (`DeviceLost` at the next validate; the `rotate` checker counts the releases after the loss as violations, not a lease defect). Kernel-lease revocation: `DeviceLost` once, buffers released once.
- Frame loss: `rotate` 3000 frames, master: presented 3000, dropped 0, missed vblanks 0, latch timeouts 0, order violations 0, 5998 releases. 1800 frames through the kernel lease and through drm-lease-manager, and 600 frames with external producers: the same, 0 dropped. `move` 3600 frames: 3600 `frame` records. 2026-11-27. Not measured: loss under CPU or memory load, or over hours.
- Latency: submit to latched (commit to page-flip event, harness wake-up included, no kernel vblank timestamp read): `rotate` master median 11 168 µs, p99 14 490, max 15 020, min 9 624; kernel lease median 11 337 µs; drm-lease-manager median 11 303 µs; external producers about 10 600 µs. `Engine::validate` (TEST_ONLY): median 135-142 µs, p99 208-310. Commit call: median 133 µs, first (modeset) commits 1.3-16.8 ms. Not measured: input-to-photon latency, latency against a kernel vblank timestamp.
- CPU: harness `rotate` 3000 frames: 34.4 % of one core (user 2.44 s + system 14.75 s over 50.0 s), almost all of it the harness redrawing both dumb buffers every frame; `move` (no redraw) 4.7 %; static holds at most 0.2 %; external-producer `rotate` 5.6 % in the harness (drawing is in the producer processes, not counted). The lease routes cost the same per frame. Board load average 0.36 after the runs. drm-lease-manager's own CPU: not measured. 2026-11-27.
- Memory: harness peak RSS 812 KiB to 1 280 KiB (`VmHWM`); `rotate` holds 15.4 MB of dumb buffers (3 x 4 096 000 B + 3 x 1 024 000 B). CMA `CmaFree` 1 070 488 kB (Weston running) before, 1 098 836 kB after, of `CmaTotal` 1 114 112 kB. CMA during a run: not measured. drm-lease-manager's memory: not measured. 2026-11-27.

## Verdicts

Condition 1: passed (host and cross builds; not a hardware condition)
- `cargo test --workspace` on a host on 2026-11-28 at the integration tip: every test result `ok`, no failure (the `#[ignore]`d vkms tests did not run).
- no_std: CI builds `composition-core` for `thumbv7em-none-eabihf` and the static library without `std` (`docs/ci.md`, jobs `checks` and `arm-backend`). That is CI evidence, not run by this item.
- Linux i.MX95 builds: `composition-drm`, the probe and `drm-harness` were cross-built for `aarch64-unknown-linux-musl` (`scripts/cross-aarch64.sh`; CI step in `docs/ci.md`) and the harness ran on the board 2026-11-27, which is the hardware proof that the aarch64 build works. Caveat: `scripts/cross-aarch64.sh` failed on one host because of a half-installed rustup target (`docs/m1-imx95/lease-run.md`, limitation 3); the plain `cargo build --target` of the harness README worked.

Condition 2: passed (topology observed; some limits source only)
- `probe --raw` on the board, 2026-11-28, kernel `6.6.157-7.8.0`: [`imx95-probe-2026-11-28.txt`](imx95-probe-2026-11-28.txt), tables tagged observed in [`platform-capabilities.md`](platform-capabilities.md): 2 CRTCs (39, 50), 1 connector (41), 7 planes (primaries 31 and 42, overlays 51, 59, 67, 75, 83), 13 formats on every plane, linear modifier only, zpos 0..5 mutable, plane alpha, three blend modes, no rotation, no cursor plane.
- Restriction limits that KMS cannot show (6 fetch units, 4 of them FetchYUV, one HScaler and one VScaler, upscale only, 2048 scaled source, shared across both CRTCs) are **source** (kernel 6.6.157-7.8.0 driver read, `imx95-dpu-source-6.6.md`), not measured. The vertical scaler is inferred from SoC revision 2.0. The `DriverProfile::Imx95Dpu` check on 2026-11-28 (`crates/composition-drm/src/profile.rs`, `platform-capabilities.md`) found that `max_source` 8192 is an unscaled-only limit (the 2048 scaled limit is not modelled; `TEST_ONLY` refuses it) and changed no value.

Condition 3: passed
- Hardware evidence, 2026-11-27 (kernel 6.6.157-7.8.0, master descriptor): every commit put the two layers on two distinct planes, bottom layer on plane 31 (primary, zpos 0, full-screen blue, buffer 1) and top layer on plane 51 (overlay, zpos 1, 640x400 red at 320,200, buffer 2), from separately allocated buffers, after `TEST_ONLY` (`docs/m1-imx95/logs/two-planes.log`: `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)`).
- Camera, both planes at once, red inside blue with no blue in the red area, 3 of 3 pictures PASS after rectification: `docs/m1-imx95/pictures/20261127T193910Z-two-planes-1-raw.jpg` (rectified: `docs/m1-imx95/pictures/20261127T193910Z-two-planes-1-rect.jpg`); checks in `docs/m1-imx95/checks/results.txt` (`ALL: PASS`).
- Z-order: `docs/m1-imx95/pictures/20261127T194022Z-a-over-b-1-raw.jpg` (overlap red, plane 51 on top) and `docs/m1-imx95/pictures/20261127T194102Z-b-over-a-1-raw.jpg` (overlap green after the swap; the planner swapped the assignment between planes 31 and 51, buffers unchanged). Not tested: swapping `zpos` on fixed planes.
- Alpha: plane alpha 0x8000 on plane 51 (`docs/m1-imx95/pictures/20261127T194141Z-plane-alpha-50-1-raw.jpg`: overlap (201,169,1), 16 from the expected blend, about 100 from either opaque colour) and per-pixel premultiplied alpha (`docs/m1-imx95/pictures/20261127T194225Z-pixel-alpha-1-raw.jpg`: same result as plane alpha 50 %). Cross-check through the real drm-lease-manager: `docs/m1-imx95/lease-pictures/lease-20261127T2019Z-dlmd-two-planes-raw.jpg`, planes 31 and 51.
- Not a flattened framebuffer: in `move` (3600 frames, `docs/m1-imx95/logs/move.log`) plane 31 kept buffer 1 in all commits, never released it, checksum `ad2ffa4dee576f25` before and after, only plane 51's destination changed; the pictures `docs/m1-imx95/pictures/20261127T194303Z-move-1-raw.jpg`, `docs/m1-imx95/pictures/20261127T194339Z-move-2-raw.jpg`, `docs/m1-imx95/pictures/20261127T194357Z-move-3-raw.jpg` show the overlay at three positions over an unchanged checkerboard (checker distances 0.3-1.3 against threshold 30). Not done: a `--flatten` control run (the harness has no such mode).
- Camera caveat: the camera sees the panel upside down at an angle; pictures were rectified with `docs/m1-imx95/rectify.py` from the outer corners of the red layer, then checked with `scripts/camera-check.py`. Two move expectations were wrong guesses and were reset after failing (outputs kept in `docs/m1-imx95/checks/`).

Condition 4: passed (planner and capability detection exercised; no kernel TEST_ONLY refusal obtained)
- `combos` on the board (no commit), 2026-11-27 19:34:50 UTC, `docs/m1-imx95/logs/combos.log`: `VERDICT combos PASS 16 accepted, 4 refused by the planner, 0 refused by TEST_ONLY, nothing committed`. Accepted: 1 to 6 layers on planes 31, 51, 59, 67, 75, 83, plane alpha, ARGB with three blend modes, 2x upscale, NV12, NV12 upscale, partly off-screen. Refused by the planner with a named reason: 7 layers (`NoPlane`, `OUTPUT | IN_USE`), 2x downscale (`SCALING`), 90° rotation (`TRANSFORM`), unknown format (`FORMAT`).
- Every accepted frame also passed atomic `TEST_ONLY` (`validate_us` 45-163 µs). Gap: no case was refused by the kernel's `TEST_ONLY`, so planner and kernel could not be seen to disagree (the harness has no `--skip-planner`; the forced-past-planner comparison of test plan §6.1 was not done). Source-only limits (2048 scaled source, even coordinates for subsampled formats, BT.709 full range) are therefore untested on the board.

Condition 5: passed (ordering and release after latch; two limits)
- `rotate`, 3 buffers per plane, two planes, 2026-11-27 19:44:22-19:45:12 UTC, `docs/m1-imx95/logs/rotate-b3.log`: `VERDICT rotate PASS 3000 frames, 3 buffers per plane, 5998 releases in order, 0 dropped, 0 missed vblanks`. Submission, page-flip latch and release are separate records in the harness log; no buffer was released while latched; `latched + dropped = submitted`; every latch interval within 20 % of the period (largest deviation +1.8 %). The same passed over the kernel lease (`docs/m1-imx95/lease-logs/kl-rotate.log`, 1800 frames), drm-lease-manager (`docs/m1-imx95/lease-logs/dlmd-rotate.log`) and with external producer processes (`docs/m1-imx95/lease-logs/dlmd-ext-rotate.log`, 600 frames, buffers via DMA-BUF).
- Limits: the harness minimum is 3 buffers, so the 2-buffer case of the test plan was not run (`docs/m1-imx95/logs/rotate-b2.log`); the harness checks event order, not a read-back of each buffer's content; the "latch" is the page-flip event, no kernel vblank timestamp is read. By eye the camera shows each plane with its own moving bar and no tear line (`docs/m1-imx95/pictures/20261127T194433Z-rotate-b3-1-raw.jpg`); that is not a check.

Condition 6: passed (both routes; Weston coexistence and daemon-side revocation not covered)
- Kernel lease, 2026-11-27 20:08-20:12 UTC: a lease of connector 41, CRTC 39, primary 31 and overlays 51..83 was created without error, so the overlays are leasable on this kernel; `two-planes`, `rotate`, external producers and `revoke` PASS (`docs/m1-imx95/lease-logs/kl-two-planes.log`, `kl-rotate.log`, `kl-revoke.log`, `kl-p51-all.log`). Picture: `docs/m1-imx95/lease-pictures/lease-20261127T2009Z-kl-two-planes-raw.jpg`, planes 31 and 51 both visible.
- AGL drm-lease-manager `68c48758c46d`, 2026-11-27 20:18-20:21 UTC, run from `/tmp`, nothing installed into the image: its lease carries overlay 51 and primary 31; `two-planes`, `rotate` (1800 and 600 external) and `all` PASS (`docs/m1-imx95/lease-logs/dlmd-two-planes.log`, `dlmd-rotate.log`, `dlmd-ext-rotate.log`, `dlmd-all.log`); a second client is refused (`dlmd-second-client.log`). Picture: `docs/m1-imx95/lease-pictures/lease-20261127T2019Z-dlmd-two-planes-raw.jpg`.
- No Weston fork was involved, but also no Weston lease: Weston was stopped during every run and restarted afterwards (checked active, same boot). Upstream Weston at `main` has no lease code (`docs/wp5-gap-analysis.md`, `docs/weston-imx-lease.md`), and this board has one panel, so leasing while Weston owns another output was not demonstrated.
- Not covered: the daemon's revocation while a client holds the lease, `-t`/`-k`, a `-v` log (the redirected output stayed empty, probably buffering; inferred), reading `lease-manager.c`. The lease pictures were judged by eye (automatic exposure, no `camera-check.py` run). No BSP limitation blocked the route.

Condition 7: passed (every field present; gaps stated)
- All eleven fields are in "Condition 7 fields" above with dates. Recorded as not measured: module and carrier PCB revision, CPU and memory of drm-lease-manager, CMA during a run, latency against a kernel vblank timestamp, input-to-photon latency, bandwidth and power. This verdict assumes the board is accepted as "equivalent" to an EVK (see the top of this report).

## What does not count as hardware evidence

Host `cargo test`, CI (`docs/ci.md`, including the aarch64 cross-build step, CI-verified only), the vkms tests and the fake backend prove nothing about the board. They are used only for condition 1. The resource groups of `DriverProfile::Imx95Dpu` remain source-tagged, not measured.

## Follow-ups

### CLAUDE.md status paragraph (git-ignored; not added here)

The paragraph says no board is available and no CI exists. It should say instead:
- CI exists (`.github/workflows/ci.yml`: `checks`, `arm-backend`, and an aarch64 cross-build step; `docs/ci.md`); it runs no hardware.
- The farm has a Toradex Verdin iMX95 on a Mallow carrier with the 10.1" LVDS panel (Reference Multimedia Image 7.8.0, kernel `6.6.157-7.8.0`, not Torizon), with a camera and a console. M1 was run on it on 2026-11-27/28; the result is this report.
- Done and verified on that board: topology probe, `composition-drm` with `DriverProfile::Imx95Dpu`, `tools/drm-harness` (master fd, kernel lease, drm-lease-manager), camera checks. The profile's resource groups are source-tagged, not measured.
- The AM62P board run is issue #12 (queued, not done); i.MX95 Zephyr (M2) is not started.
- Board work must restore the display client (Weston and `wayland-app-launch.service`) and clean `/tmp`; no addresses, host names or serial paths in files.

### Open items for M1 itself (reported, not done here)

- Condition 4: obtain a kernel `TEST_ONLY` refusal (harness `--skip-planner` or equivalent) and test the source-only limits on the board.
- Condition 5: a 2-buffer run, a content read-back, a kernel vblank timestamp.
- Condition 3: a `--flatten` control run; swapping `zpos` on fixed planes instead of the assignment.
- Condition 6: drm-lease-manager revocation, `-t`/`-k`, a logged `-v` run; a `rotate` checker that treats a lease loss as expected.
- Condition 7: read module and carrier revisions; measure drm-lease-manager CPU and memory, and CMA during a run.
- Tooling: `scripts/cross-aarch64.sh` needs a host whose rustup can add the musl target; fetching the drm-lease-manager sources needed `GIT_CONFIG_GLOBAL=/dev/null` on one host. Both are host state.

### M2: i.MX95 Zephyr (not started)

Open issues are M2-01..M2-07 in [`open-issues.md`](open-issues.md). The two that matter first: the stock Zephyr `socionext,dpu` driver is a single-framebuffer pipeline (no planes, scaling or alpha), and display ownership between the A55 (Linux) and the M7 (Zephyr) is unknown (System Manager configuration not examined). This board runs the Linux image; whether its M7 can own a display route is unrecorded.

### M3: AM62P (issue #12)

Tracked as issue #12, queued for the AM62P board. Not touched by this report. See the M3 rows of `open-issues.md`.

### WP5: upstream-readiness

Open issues are WP5-01..WP5-09 in `open-issues.md`. From M1: Weston owns the output mode, so the leased-output route has no Weston coexistence evidence (WP5-07; one panel here); libliftoff overlaps the planner and no cross-check exists, the observed topology and the board numbers above are now its input (WP5-04, WP5-06); `DriverProfile::Imx95Dpu` has no generic KMS counterpart, so an upstream path is open (WP5-09); licences of drm-lease-manager and libdlmclient are not recorded and there is no NOTICE or contribution policy (WP5-05); a clean-checkout demo path is missing (WP5-08).
