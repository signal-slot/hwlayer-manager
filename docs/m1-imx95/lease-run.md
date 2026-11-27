# M1 lease run on the Verdin iMX95 (2026-11-27)

Board session of item `m1-lease-run` (SoW §9 M1, condition 6; test plan `docs/m1-imx95-test-plan.md`). `tools/drm-harness` drove `composition-drm` on the LVDS output through a **DRM lease** instead of a master descriptor, and the camera photographed the panel. Everything here was observed on 2026-11-27 in two sessions, 20:06–20:15 UTC (kernel lease, protocol stand-in) and 20:18–20:22 UTC (the real drm-lease-manager), board clock. The master-descriptor run of the same day is [master-run.md](master-run.md); board, panel, image and kernel are the same as there.

Short result:

- **Kernel lease route: passed.** A lessor in the harness process leases connector 41, CRTC 39, primary 31 and the overlays with `DRM_IOCTL_MODE_CREATE_LEASE`. The lease **carries the overlays**: the lessee sees exactly the leased planes (31, 51, 59, 67, 75, 83, or 31 and 51 with `--planes 51`). Two planes, rotation, external producer processes and revocation all pass, and the camera shows both planes.
- **drm-lease-manager route: passed.** AGL drm-lease-manager at commit `68c48758c46d` was built with `scripts/build-dlm-aarch64.sh` and run from `/tmp` with a config that lists `LVDS-1` and planes 31 and 51. Its lease carries the overlay. The harness (`--source dlm`) composed two planes through it: two-planes with a camera picture, rotate, rotate with external producers, and every scenario of `all` (revoke does not apply) passed. A second client was refused while the first held the lease. The same client also passed against the repository's stand-in server `drm-harness lessor`, before the daemon could be built (first session).

## Setup (observed 2026-11-27)

| Item | Value | Source |
| --- | --- | --- |
| Board | Toradex Verdin iMX95 on a Mallow carrier, `/proc/device-tree/model` "Toradex Verdin iMX95 WB on Verdin Development Board" | board, 20:06 UTC |
| Panel | Toradex 10.1" LVDS on X12, connector `LVDS-1` (id 41), CRTC 39, 1280x800, `refresh_mhz=59993`, 71100 kHz | harness `output` record |
| Kernel | `6.6.157-7.8.0`, driver `imx95-dpu` | `uname -r`, `session` record |
| Display client before | `weston.service`, `weston.socket` and `wayland-app-launch.service` active (demo full screen); board up 38 min | `systemctl is-active`, `uptime` |
| Harness | `drm-harness` 0.1.0, built from the integration tip `82a0355b995c` with `cargo build --release -p composition-drm-harness --target aarch64-unknown-linux-musl --config 'target.aarch64-unknown-linux-musl.linker="rust-lld"'` (the harness README's command, see [Limitations](#limitations) for why not `scripts/cross-aarch64.sh`); static aarch64 ELF, run from `/tmp` | host build |
| Lease routes | kernel: `--source kernel-lease` (in-process lessor, `src/lease.rs`); lease-manager protocol: `--source dlm` against `drm-harness lessor` (`src/wire.rs`, protocol of drm-lease-manager `68c48758c46d`) | `tools/drm-harness/README.md` |
| drm-lease-manager | version 0.1.0, commit `68c48758c46d93c8e83abb2fb1ec168fa13d1a53`, with libdrm `25dec5b91fe4` and tomlc99 `62ce93ceb713` linked in; built 2026-11-28 (host date) with `scripts/build-dlm-aarch64.sh` route 6 (clang, Debian arm64 glibc 2.41 sysroot, `-static`): `ELF 64-bit LSB executable, ARM aarch64, ... statically linked, ... stripped`, the same as the record in `docs/dlm-aarch64.md`. The fetch needed `GIT_CONFIG_GLOBAL=/dev/null` on that host (see [Limitations](#limitations)) | build output |
| drm-lease-manager config | `/tmp/dlm.toml`: `[[lease]] name = "hwlayer"`, `connectors = ["LVDS-1"]`; `[LVDS-1] planes = [31, 51]` | copied to the board |
| drm-lease-manager command | `DLM_RUNTIME_PATH=/tmp/lease-dlm /tmp/drm-lease-manager --config /tmp/dlm.toml -v -u 1 /dev/dri/card0` (started with `setsid` in the background) | board |
| Log wrapper | each log starts with `# start_utc=... uname_r=...` and `# command=...` and ends with `# end_utc=... exit_status=N`; the harness lines in between are unedited | `lease-logs/` |
| Camera | the farm's camera on the panel; full 4608x2592 frames from `capture_debug` (raw), saved downscaled 4x, upside down as the camera sees them. The camera tool draws its screen-detection outline and red corner dots on these frames as well; they are not on the panel | camera tool |

## Weston before and after

- **Before** (20:06:55 UTC): `weston.service`, `weston.socket`, `wayland-app-launch.service` active; the demo on the panel ([picture](lease-pictures/lease-20261127T2008Z-weston-before-raw.jpg)). The board's `/tmp` held only system entries.
- **Stopped** at 20:08:21 UTC with `systemctl stop weston.service` (systemd also stopped `wayland-app-launch.service`, which `Requires=` Weston; `weston.socket` stayed listening and did not re-activate Weston during the session).
- **Restored** at 20:14:22 UTC: `systemctl start weston.service`, then `systemctl start wayland-app-launch.service`; at 20:14:32 UTC all three `active`, the demo back on the panel ([picture](lease-pictures/lease-20261127T2014Z-weston-after-raw.jpg)). Same boot (no reboot).
- **Cleaned**: everything copied or written to `/tmp` (`drm-harness`, the wrapper, `lease-out/`, `lease-dlm/`, the dmesg copy) removed at 20:14:32 UTC; `ls -la /tmp` showed the same system entries as before. No `drm-harness` or lessor process left (`ps`). No drm-lease-manager was started in this first session.
- **Second session** (drm-lease-manager): at 20:18:15 UTC Weston and the demo were active again; stopped at 20:18:24 UTC the same way; restored at 20:21:12 UTC (`weston.service`, `wayland-app-launch.service`, `weston.socket` all `active` at 20:21:22 UTC, demo on the panel: [picture](lease-pictures/lease-20261127T2021Z-weston-after-2-raw.jpg)). drm-lease-manager was stopped with `kill -TERM` at 20:21:07 UTC (`pidof` empty afterwards). It left its socket `hwlayer` and `hwlayer.lock` in the runtime directory; they were removed with it. At 20:21:22 UTC `/tmp` held only the system entries listed before the first session, and no `drm-harness` or `drm-lease-manager` process was running.
- `dmesg` before and after each session: identical (empty diff). CMA after: `CmaFree` 1 098 836 kB of 1 114 112 kB (Weston stopped).

## VERDICT lines (observed 2026-11-27, kernel 6.6.157-7.8.0)

Common options: `/tmp/drm-harness --connector LVDS --profile imx95`. The exact command is the second line of each log.

### Kernel lease route

| Date (UTC) | Log | Options | VERDICT line, as printed |
| --- | --- | --- | --- |
| 20:08:26–20:09:06 | [kl-two-planes.log](lease-logs/kl-two-planes.log) | `--source kernel-lease --seconds 40 two-planes` | `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)` |
| 20:09:17–20:09:47 | [kl-ext-two-planes.log](lease-logs/kl-ext-two-planes.log) | `--source kernel-lease --producers external --seconds 30 two-planes` | `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)` |
| 20:10:10–20:10:41 | [kl-rotate.log](lease-logs/kl-rotate.log) | `--source kernel-lease --frames 1800 rotate` | `VERDICT rotate PASS 1800 frames, 3 buffers per plane, 3598 releases in order, 0 dropped, 0 missed vblanks` |
| 20:11:16–20:11:26 | [kl-ext-rotate.log](lease-logs/kl-ext-rotate.log) | `--source kernel-lease --producers external --frames 600 rotate` | `VERDICT rotate PASS 600 frames, 3 buffers per plane, 1198 releases in order, 0 dropped, 0 missed vblanks` |
| 20:11:26–20:11:27 | [kl-revoke.log](lease-logs/kl-revoke.log) | `--source kernel-lease revoke` | `VERDICT revoke PASS 60 frames on planes 31,51, then revoked: DeviceLost once, buffers 3,6 released` |
| 20:11:27–20:11:29 | [kl-ext-revoke.log](lease-logs/kl-ext-revoke.log) | `--source kernel-lease --producers external revoke` | `VERDICT revoke PASS 60 frames on planes 31,51, then revoked: DeviceLost once, buffers 3,6 released` |
| 20:11:29–20:12:01 | [kl-p51-all.log](lease-logs/kl-p51-all.log) | `--source kernel-lease --planes 51 --seconds 3 all` (lease of primary 31 + overlay 51 only) | `VERDICT combos PASS 12 accepted, 4 refused by the planner, 0 refused by TEST_ONLY, nothing committed`, `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)`, `VERDICT zorder-alpha PASS steps a-over-b,b-over-a,plane-alpha-50,pixel-alpha-premultiplied`, `VERDICT move PASS 240 frames, bottom buffer 1 unchanged on plane 31, top on its own plane`, `VERDICT rotate PASS 600 frames, 3 buffers per plane, 1198 releases in order, 0 dropped, 0 missed vblanks`, `VERDICT revoke PASS 60 frames on planes 31,51, then revoked: DeviceLost once, buffers 3,6 released`, `SUMMARY passed=6 failed=0` |

### drm-lease-manager route (the real daemon, second session)

Before the harness: `/tmp/drm-lease-manager --help` ran (exit 0, [dlm-daemon-help.log](lease-logs/dlm-daemon-help.log); the garbled first line is the upstream `usage()` bug noted in `docs/dlm-aarch64.md`). With Weston stopped the daemon became DRM master and created `/tmp/lease-dlm/hwlayer` and `hwlayer.lock`. Upstream's `dlm-client-test hwlayer` (with `DLM_RUNTIME_PATH=/tmp/lease-dlm`) received a lease and printed `crtc-id: 39`, `connector-id: 41`, `plane-id: 51`, exit 0 ([dlm-client-test.log](lease-logs/dlm-client-test.log)). It lists planes without the universal-planes capability, so it does not show the primary. The harness does, below. Client options: `--source dlm --lease hwlayer --dlm-dir /tmp/lease-dlm`.

| Date (UTC) | Log | Options | VERDICT line, as printed |
| --- | --- | --- | --- |
| 20:18:44–20:19:14 | [dlmd-two-planes.log](lease-logs/dlmd-two-planes.log) | `--seconds 30 two-planes` | `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)` |
| 20:19:35–20:20:05 | [dlmd-rotate.log](lease-logs/dlmd-rotate.log) | `--frames 1800 rotate` | `VERDICT rotate PASS 1800 frames, 3 buffers per plane, 3598 releases in order, 0 dropped, 0 missed vblanks` |
| 20:20:05–20:20:15 | [dlmd-ext-rotate.log](lease-logs/dlmd-ext-rotate.log) | `--producers external --frames 600 rotate` | `VERDICT rotate PASS 600 frames, 3 buffers per plane, 1198 releases in order, 0 dropped, 0 missed vblanks` |
| 20:20:15–20:20:46 | [dlmd-all.log](lease-logs/dlmd-all.log) | `--seconds 3 all` (no `revoke` with this source) | `VERDICT combos PASS 12 accepted, 4 refused by the planner, 0 refused by TEST_ONLY, nothing committed`, `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)`, `VERDICT zorder-alpha PASS steps a-over-b,b-over-a,plane-alpha-50,pixel-alpha-premultiplied`, `VERDICT move PASS 240 frames, bottom buffer 1 unchanged on plane 31, top on its own plane`, `VERDICT rotate PASS 600 frames, 3 buffers per plane, 1198 releases in order, 0 dropped, 0 missed vblanks`, `SUMMARY passed=5 failed=0` |
| 20:20:52–20:21:00 | [dlmd-first-holder.log](lease-logs/dlmd-first-holder.log) | `--seconds 8 two-planes` (holds the lease) | `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)` |
| 20:20:55 | [dlmd-second-client.log](lease-logs/dlmd-second-client.log) | `--seconds 1 two-planes`, started while the first holder had the lease | `VERDICT two-planes FAIL open /tmp/lease-dlm/hwlayer: lease refused: the lease manager closed the connection` (expected: one client per lease without `-t`) |

The daemon's own output ([dlm-daemon.log](lease-logs/dlm-daemon.log)) is one line, `ERROR: Can't fulfill lease request: lease=hwlayer`, from the refused second client. Despite `-v`, nothing else was written to the redirected file, neither at startup nor for the granted leases. The likely reason is stdout buffering in a file that was never flushed, because the daemon was stopped by SIGTERM (inferred, not checked). The first holder was not disturbed by the refusal.

### drm-lease-manager protocol against `drm-harness lessor` (first session)

The server was `drm-harness lessor --lease lvds --dlm-dir /tmp/lease-dlm --connector LVDS --planes 51`, started at 20:12 UTC in its own session ([dlm-lessor.log](lease-logs/dlm-lessor.log)); it leased connector 41, CRTC 39, primary 31 and overlay 51 (`objects=4`), one fresh kernel lease per client. Client options: `--source dlm --lease lvds --dlm-dir /tmp/lease-dlm`. The harness labels this route `lease route=drm-lease-manager` because it speaks that protocol; the server was **not** drm-lease-manager.

| Date (UTC) | Log | Options | VERDICT line, as printed |
| --- | --- | --- | --- |
| 20:12:11–20:12:41 | [dlm-two-planes.log](lease-logs/dlm-two-planes.log) | `--seconds 30 two-planes` | `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)` |
| 20:12:54–20:13:24 | [dlm-rotate.log](lease-logs/dlm-rotate.log) | `--frames 1800 rotate` | `VERDICT rotate PASS 1800 frames, 3 buffers per plane, 3598 releases in order, 0 dropped, 0 missed vblanks` |
| 20:13:24–20:13:34 | [dlm-ext-rotate.log](lease-logs/dlm-ext-rotate.log) | `--producers external --frames 600 rotate` | `VERDICT rotate PASS 600 frames, 3 buffers per plane, 1198 releases in order, 0 dropped, 0 missed vblanks` |
| 20:13:34 | [dlm-revoke-refused.log](lease-logs/dlm-revoke-refused.log) | `revoke` | `VERDICT revoke FAIL revoke needs --source kernel-lease: this process must be the lessor` (by design, see the harness README: with this source the lessor is another process) |
| 20:13:40–20:13:46 | [dlm-lessor-killed.log](lease-logs/dlm-lessor-killed.log) | `--frames 1200 rotate`, the lessor process killed (`kill`, SIGTERM) at 20:13:46 | `VERDICT rotate FAIL frame 355: validate: Backend(DeviceLost); 355 of 1200 frames presented; 2 lifecycle order violations; 710 releases, expected 708` (expected failure, see below) |

**Lessor killed during a client's rotation** (an extra check, not in the test plan). When the lessor process died, the kernel revoked the lease with the lessor's master descriptor. The client noticed at its next `Engine::validate` (frame 355): `Backend(DeviceLost)`, no hang, no kernel message. The engine then released the two buffers on screen (1 and 4), as the `revoke` scenario expects. `rotate` does not expect a loss, so its checker counts those two releases as "released while Latched" violations and fails. The behaviour on loss matches `revoke`; the FAIL is the `rotate` scenario's checker, not a lease defect. The killed lessor left its socket file `/tmp/lease-dlm/lvds` behind (no signal handler); it was removed in the cleanup.

## What the lease exposes (observed 2026-11-27)

| Lease | `lease` record (lessor side) | Planes the lessee's engine sees (`plane` / `lease_check`) |
| --- | --- | --- |
| kernel, default `--planes` | `lessee=1 connector=41 connector_name=LVDS-1 crtc=39 primary=31 overlays=51,59,67,75,83 objects=8` | 31 (primary), 51, 59, 67, 75, 83 (overlay), each `zpos=0..5`, `alpha=true`, `blend_mask=7`, `scaling=true`, the same 13 formats as on the master descriptor; `not_leased=- leased_not_seen=- outputs=1` |
| kernel, `--planes 51` | `... primary=31 overlays=51 objects=4` | 31, 51; `not_leased=- leased_not_seen=- outputs=1` |
| via `drm-harness lessor` (`--planes 51`) | `... primary=31 overlays=51 objects=4` (lessor log) | 31, 51 (the client's `plane` records; the `dlm` source prints no `lease_check`, it does not know the lessor's object list) |
| drm-lease-manager, `planes = [31, 51]`, `-u 1` | not logged by the daemon; `dlm-client-test`: CRTC 39, connector 41, plane 51 (no universal planes) | 31 (primary), 51 (overlay), output 39 / `LVDS-1` 1280x800 (`plane` and `output` records of every `dlmd-*` log) |

- **The overlays are leasable.** The kernel accepts overlays whose `possible_crtcs` include the second CRTC (50) in a lease that holds only CRTC 39: the default lease with all five overlays was created without error. **drm-lease-manager's lease carries the overlay** listed in its config (51), with the primary (31).
- The lessee sees only the leased planes. With two leased planes, `combos` refuses three layers in the planner (`NoPlane { layer: 2, reasons: IN_USE }`), where the master run accepted up to six. The lease limits the engine's plane set as expected.
- Lease creation, use and revocation produced no kernel message (`dmesg` unchanged over the session).

## Plane assignments (observed 2026-11-27)

From the `assign` records. Every commit was preceded by `Engine::validate` (atomic `TEST_ONLY`).

| Run | Assignment |
| --- | --- |
| every `two-planes` (kernel, kernel + external, `drm-harness lessor`, drm-lease-manager) | layer 0 → buf 1 → **31** (primary, zpos 0), full screen 1280x800; layer 1 → buf 2 → **51** (overlay, zpos 1), 640x400 at 320,200; both opaque XR24, alpha 0xffff |
| every `rotate` | layer 0 → **31**, layer 1 → **51**, `assignment_changes=0`, 3 buffers per plane |
| `revoke` | planes 31, 51 until the revocation; afterwards `outputs_after=0 planes_after=0` |
| `kl-p51-all` and `dlmd-all` `zorder-alpha` / `move` | as in the master run (z-order swapped by plane assignment between 31 and 51; `move`: bottom buffer 1 on 31 in all 240 frames, checksum `ad2ffa4dee576f25` before and after, never released) |

With `--producers external` the buffers were allocated by two separate `drm-harness producer` processes (one per layer, `was_master=no`), exported as DMA-BUF and imported by the engine. Producer exits: `refused=0 status=ok`, `problems=0` (e.g. [kl-ext-revoke.log](lease-logs/kl-ext-revoke.log): 126 releases passed on, 6 returned unshown).

## Measurements (observed 2026-11-27)

| Item | Kernel lease | drm-lease-manager | Through `drm-harness lessor` | Log |
| --- | --- | --- | --- | --- |
| Frame loss, rotate | 1800 frames: presented 1800, dropped 0, missed vblanks 0, latch timeouts 0, order violations 0, releases 3598. 600 frames external: same, 1198 releases | same as kernel (1800 and 600 external) | same as kernel | kl-, dlmd-, dlm-rotate / -ext-rotate |
| Latch interval | median 16 668 µs, min 16 425, p99 16 886, max 16 930 (1798 intervals) | median 16 668, min 16 228, p99 16 905, max 16 954 | median 16 668, min 16 421, p99 16 878, max 16 933 | kl-rotate, dlmd-rotate, dlm-rotate |
| Latency, submit → latched | median 11 337 µs, p99 11 682, max 15 161, min 9 839 | median 11 303, p99 11 619, max 13 666, min 10 149 | median 11 313, p99 11 662, max 13 635, min 10 001 | kl-rotate, dlmd-rotate, dlm-rotate |
| Latency, external producers | median 10 599, p99 11 245, max 14 444 | median 10 618, p99 11 176, max 14 101 | median 10 587, p99 11 211, max 13 818 | -ext-rotate logs |
| validate (TEST_ONLY) | median 135 µs, p99 208 | median 140, p99 266 | median 139, p99 284 | kl-rotate, dlmd-rotate, dlm-rotate |
| Revocation | first frame after the revoke fails with `DeviceLost` after 7.7–8.2 ms; second with `UnknownOutput(39)`; `DeviceLost` reported once; buffers 3, 6 released once; `still_busy=-` | not tested (`revoke` needs the in-process lessor; the daemon was stopped only after its clients had ended) | lessor killed: `DeviceLost` at the next validate | kl-revoke, kl-ext-revoke, kl-p51-all, dlm-lessor-killed |
| CPU | rotate 1800 frames: 34.0 % of one core (user 1.45 s, sys 8.76 s over 30.0 s; the harness redraws both buffers each frame); rotate external 600: 5.6 % in the harness (the drawing is in the producers, not counted); `two-planes` holds 0.0 % | 34.4 % (1800), 5.6 % (external 600); the daemon's own CPU was not measured | 34.3 % (1800), 5.7 % (external 600) | `resources` records |
| Memory | harness peak RSS 1 236 KiB (rotate), 876–1 008 KiB otherwise | 1 280 KiB (rotate) | 1 232 KiB (rotate) | `resources` records |
| Errors | no non-zero errno in any commit; no kernel message | the expected refusal of a second client (EOF, daemon: `Can't fulfill lease request`); no kernel message | the expected `revoke` refusal and the lessor-killed `DeviceLost` above | logs, dmesg diff |

The latency and CPU figures match the master run (rotate median 11 168 µs, 34.4 %): on this kernel a lease costs nothing measurable per frame. The latency includes the harness's wake-up time (no kernel vblank timestamp is read), as in the master run.

## Camera pictures (observed 2026-11-27)

The camera sees the panel upside down and at an angle (see [master-run.md](master-run.md#camera-pictures-and-checks-observed-2026-11-27)). These pictures were judged by eye. `camera-check.py` was not run on them: they were taken with automatic exposure, and the camera's position was not re-measured for `rectify.py`.

| Picture | State | What it shows |
| --- | --- | --- |
| [before](lease-pictures/lease-20261127T2008Z-weston-before-raw.jpg) | Weston and the demo, before the session | demo full screen |
| [kl-two-planes](lease-pictures/lease-20261127T2009Z-kl-two-planes-raw.jpg) | kernel lease, `two-planes` hold | both planes at once: blue primary 31 with its white edge and label "1", the red overlay 51 with its yellow edge and label "2" on top in the middle, no blue inside it |
| [kl-ext-two-planes](lease-pictures/lease-20261127T2010Z-kl-ext-two-planes-raw.jpg) | kernel lease, external producers, `two-planes` hold | the same picture, buffers drawn by the producer processes |
| [kl-rotate](lease-pictures/lease-20261127T2010Z-kl-rotate-raw.jpg) | kernel lease, `rotate` | each plane shows its own white bar at a different position (one sweeping the blue primary, one the red overlay); no pattern of one layer in the other, no tear line |
| [dlm-two-planes](lease-pictures/lease-20261127T2013Z-dlm-two-planes-raw.jpg) | `--source dlm` via `drm-harness lessor`, `two-planes` hold | the same picture as kl-two-planes |
| [after](lease-pictures/lease-20261127T2014Z-weston-after-raw.jpg) | Weston and the demo restarted after the first session | demo full screen again |
| [dlmd-two-planes](lease-pictures/lease-20261127T2019Z-dlmd-two-planes-raw.jpg) | `--source dlm` through **drm-lease-manager**, `two-planes` hold | the same picture as kl-two-planes: blue primary 31 and red overlay 51 on top, both at once |
| [after-2](lease-pictures/lease-20261127T2021Z-weston-after-2-raw.jpg) | Weston and the demo restarted after the second session | demo full screen again |

The red overlay's right part looks orange/yellow in the pictures: the panel's angle-dependent hot spot, also seen in the master run, not the content.

## Limitations

1. **The drm-lease-manager source fetch failed on the session's host at first (host, not BSP); solved for the second session.** `scripts/build-dlm-aarch64.sh "$CONNECTED_DEPS_DIR"` chose its toolchain route 6 (clang + Debian arm64 sysroot) and fetched the Debian packages. The source fetch then failed. First try: `fatal: unable to access 'https://gerrit.automotivelinux.org/gerrit/src/drm-lease-manager/': Resolving timed out after 300100 milliseconds`. Two later tries, inside and outside the tool sandbox: `fatal: refusing to work with credential missing host field` for the fetch by hash and for the fetch-all fallback. The script stopped before building, with nothing written to the board. A later check the same day showed the server itself was reachable: `curl` got HTTP 200 from the gerrit host and its name resolved. Only git failed, with the same `credential missing host field` on a plain `git ls-remote`, and no proxy variable was set. The cause was the host user's global git configuration, which sets an https client certificate (`http.sslcert`/`http.sslkey`) for which git asks for a credential. `GIT_CONFIG_GLOBAL=/dev/null scripts/build-dlm-aarch64.sh "$CONNECTED_DEPS_DIR"` then fetched all three pins by hash and built the daemon. A host without such a configuration needs no workaround.
2. **drm-lease-manager details not covered:** the `-v` log was not obtained (see above). Revocation by the daemon while a client holds the lease was not tested. The daemon was stopped only after its clients had ended, and `-k`/`-t` were not tried. `lease-manager.c` was still not read, so how the daemon builds the object list is known only from the result (the harness saw 31 and 51).
3. `scripts/cross-aarch64.sh` failed on the session's host: `rustup target add aarch64-unknown-linux-musl` stopped with `detected conflict: 'lib/rustlib/aarch64-unknown-linux-musl/lib/libaddr2line-...rlib'`. The target's files were present but not registered with rustup, probably after an interrupted or concurrent install. The plain `cargo build ... --target aarch64-unknown-linux-musl` of the harness README worked with those files. This is host state, not a recipe defect.
4. `revoke` does not apply to `--source dlm` (by design). Revocation from the lessor side through the socket route was shown only by killing the lessor.

## Files

- [`lease-logs/`](lease-logs/): harness output of every run (`kl-*`: kernel lease; `dlmd-*`: `--source dlm` through drm-lease-manager, with `dlm-daemon.log`, `dlm-daemon-help.log`, `dlm-client-test.log` from the daemon and its upstream test client; other `dlm-*`: `--source dlm` through `drm-harness lessor` and the lessor's own log), unedited apart from the wrapper's `#` lines.
- [`lease-pictures/`](lease-pictures/): every picture used above, `lease-<UTC time>-<state>-raw.jpg`, the camera frame downscaled 4x, upside down as the camera sees it, with the camera tool's detection marks.
