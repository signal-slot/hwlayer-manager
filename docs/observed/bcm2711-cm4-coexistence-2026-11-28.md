# BCM2711 (reTerminal CM4): WP5-07 coexistence trial, compositor and drm-lease-manager on one card

Date: 2026-11-28. Rows tagged **observed 2026-11-28** were measured on the board over ssh and through the
NanoKVM HDMI capture on 2026-11-28 (board clock UTC 08:44 to 08:51). This is the trial that
`docs/wp5-coexistence-survey.md` left open for this board: can a compositor own one connector of vc4 while
AGL drm-lease-manager leases the other connector to this repository's harness (W5-10 in
`docs/wp5-gap-analysis.md`, WP5-07 in `docs/open-issues.md`). It is portability evidence on a Raspberry Pi
CM4 only. It says nothing about the i.MX95, Weston or M1, and counts toward no milestone.

Evidence: `bcm2711-cm4-coexistence-2026-11-28/log.txt` (scrubbed log of the run that is recorded here) and
`bcm2711-cm4-coexistence-2026-11-28/hdmi-kvm-capture.png` (the NanoKVM capture of HDMI-A-1, reduced from
1920x1080 to 480x270).

## Short answer

- **Compositor and lease manager on the same card, side by side: no** (observed 2026-11-28). Both need DRM
  master of `/dev/dri/card0`, and only one process can hold it. Compositor first: drm-lease-manager starts,
  but `drmModeCreateLease` fails with `Permission denied` and the harness's lease request is refused.
  Lease manager first: the compositor starts, then exits with `Error presenting framebuffer on screen:
  Permission denied (os error 13)`.
- **Two lease clients on the two connectors at the same time: yes** (observed 2026-11-28). drm-lease-manager
  holds master and grants an HDMI-A-1 lease and a DSI-1 lease. The harness runs on each lease: on HDMI
  (`modeset`, 2398 of 2398 commits latched at 1920x1080, captured through the KVM) and on DSI (`two-planes`,
  PASS). **The second harness instance on DSI-1 stands in for a compositor. It is not a compositor.**
- The route that would make a real compositor coexist with drm-lease-manager is for the compositor to be a
  lease client itself, as AGL's Weston is (inferred, from `docs/weston-imx-lease.md` and
  `docs/drm-lease-source.md`). The compositor on this image cannot do that: it opens `/dev/dri/card0` itself
  and has no drm-lease-manager client (inferred from the failure above and the binary's strings, below).

## Read-only probe before the trial

| Item | Value | Tag |
|---|---|---|
| Kernel, card | `6.12.47+rpt-rpi-v8`; `card0` = vc4 (display), `card1` + `renderD128` = v3d | observed 2026-11-28 |
| Connectors (sysfs `status`, `enabled`, mode count, EDID bytes) | `DSI-1` connected, enabled, 1 mode (720x1280), no EDID; **`HDMI-A-1` connected, enabled, 20 modes, EDID 256 bytes, first mode 1920x1080** (the NanoKVM's EDID); `HDMI-A-2` disconnected, disabled; `Writeback-1` connected (virtual), disabled | observed 2026-11-28 |
| Change since the survey | `docs/wp5-coexistence-survey.md` (same date, earlier) saw both HDMI connectors disconnected. The NanoKVM on HDMI-A-1 was set up after it, so there is now a **second physical sink** | observed 2026-11-28 |
| CRTC state before | CRTC 90 (`pixelvalve-1`) → DSI-1, 720x1280@60, plane 79 on the fbcon framebuffer; CRTC 102 (`pixelvalve-2`) → HDMI-A-1, **640x480@75** (fbcon), plane 91 on fbcon; `hvs_underrun` 0 | observed 2026-11-28 |
| DRM master before | Nobody (`fuser -v /dev/dri/*` empty); `slint-launcher.service` and `lightdm.service` inactive, `slint-launcher` disabled; default target `multi-user.target`; logind shows only the ssh user's session | observed 2026-11-28 |
| Compositors on the image | `weston`, `labwc`, `wayfire`, `cage`, `sway`, `Xorg`, `mutter`, `kwin_wayland`, `gnome-shell`, `kmscube`, `modetest`: none installed. **One compositor exists: `/opt/slint-demo/launcher`**, the binary of the disabled `slint-launcher.service` ("a Wayland compositor written with Slint"; environment `SLINT_BACKEND=linuxkms-skia-opengl`, `SLINT_KMS_ROTATION=90`). `kmstest` (kms++-utils) is installed | observed 2026-11-28 |
| Launcher's output selection and lease support | Its strings contain `SLINT_DRM_OUTPUT` ("Please select an output with the SLINT_DRM_OUTPUT environment variable"), smithay 0.7.0's `wayland/drm_lease` module and the `wp_drm_lease_device_v1` protocol names. It has no drm-lease-manager or `DLM_RUNTIME_PATH` string. So it can be pinned to one connector, and it might offer Wayland DRM leases to its own clients. Whether it does was not tested, and the harness is not a Wayland client | observed 2026-11-28 (strings); the rest inferred |
| drm-lease-manager on vc4 | Source at `68c48758c46d93c8e83abb2fb1ec168fa13d1a53` (`drm-lease-manager/lease-manager.c`) opens the device, never calls `drmSetMaster`/`drmDropMaster`, and calls `drmModeCreateLease` when a client asks. It needs to be master at that moment. With `-u` and an explicit `planes` list per connector it leases the CRTC, the connector and the listed planes, shared overlays included | read from source |
| Plane topology with two outputs (`probe` example, built from this tree, run once with sudo) | output 0 = CRTC 102 (HDMI-A-1), primary 91, cursor 688; output 1 = CRTC 90 (DSI-1), primary 79, cursor 677; overlays 127 … 292 reach both (mask `0b11`). The `MAX_PLANES` cap drops overlays 259, 270, 281 and 292 from the listing (as in `bcm2711-cm4-x09-2026-11-28.md`) | observed 2026-11-28 |

## Build and set-up

| Item | Value | Tag |
|---|---|---|
| Harness | `scripts/cross-aarch64.sh` (no arguments) at commit df507ed: `drm-harness` and the `probe` example, static aarch64 musl | observed 2026-11-28 |
| drm-lease-manager | `scripts/build-dlm-aarch64.sh <deps dir>`, pins as in `docs/dlm-aarch64.md` (drm-lease-manager `68c48758c46d93c8e83abb2fb1ec168fa13d1a53`, libdrm `25dec5b91fe4d2638787d033a0b22b6c1dc145e0`, tomlc99 `62ce93ceb713dc49909d0fef1411f9d37bef83f2`), route 6. Output: `drm-lease-manager` and `dlm-client-test`, `ELF 64-bit LSB executable, ARM aarch64 ... statically linked`. Build tree in the task's temporary directory, not `/tmp` | observed 2026-11-28 |
| First execution of the aarch64 drm-lease-manager on hardware | It ran and created its sockets. This closes "Not tested: any execution" of `docs/dlm-aarch64.md` for **this CM4 only**, not for the i.MX95 | observed 2026-11-28 |
| On the board | Everything in `/tmp/cx` (binaries, two TOML files, `run.sh`, logs, `DLM_RUNTIME_PATH=/tmp/cx/dlm`, the launcher's `XDG_RUNTIME_DIR=/tmp/cx/xdg`). No package installed. No unit, config.txt, udev, seat or DRM configuration changed | observed 2026-11-28 |
| Compositor command (run by hand as the normal user, not via the service) | `XDG_RUNTIME_DIR=/tmp/cx/xdg SLINT_BACKEND=linuxkms-skia-opengl SLINT_KMS_ROTATION=90 SLINT_DRM_OUTPUT=DSI-1 /opt/slint-demo/launcher` | observed 2026-11-28 |
| Lease manager command | `sudo env DLM_RUNTIME_PATH=/tmp/cx/dlm /tmp/cx/drm-lease-manager --config <toml> -v -u 1 /dev/dri/card0` | observed 2026-11-28 |

Lease configuration for order 2 (`dlm.toml`). Order 1 used only the `hdmi` lease and its plane list
(`dlm-hdmi.toml`):

```toml
[[lease]]
name = "hdmi"
connectors = ["HDMI-A-1"]

[[lease]]
name = "dsi"
connectors = ["DSI-1"]

[HDMI-A-1]
planes = [91, 237, 248]

[DSI-1]
planes = [79, 127, 138]
```

The overlay sets are disjoint, because the kernel does not lease one object to two lessees (inferred). The
plane IDs come from the `probe` run above.

## Order 1: compositor first, then the lease manager

| Item | Value | Tag |
|---|---|---|
| Compositor on DSI-1 | Started; log `Using Skia OpenGL renderer`, `Rendering at 720x1280 with 90 rotation_degrees rotation`, `Clients can use GPU buffers`. `fuser`: the launcher alone holds `card0`. debugfs: plane 79 on CRTC 90 `allocated by = launcher`; HDMI's plane 91 still on fbcon at 640x480 | observed 2026-11-28 |
| drm-lease-manager | Starts and creates socket `/tmp/cx/dlm/hdmi` (and `.lock`) | observed 2026-11-28 |
| Harness `--source dlm --lease hdmi two-planes` | `lease refused: the lease manager closed the connection`, `VERDICT two-planes FAIL`, exit 1 | observed 2026-11-28 |
| drm-lease-manager log | `ERROR: drmModeCreateLease failed on lease hdmi: Permission denied`, `ERROR: Can't fulfill lease request: lease=hdmi` | observed 2026-11-28 |
| Cause | The compositor got DRM master by opening `card0` first. A non-master descriptor may not create a lease | inferred (the EACCES and the source of drm-lease-manager) |

## Order 2: the lease manager first, then the clients

| Item | Value | Tag |
|---|---|---|
| drm-lease-manager | Master of `card0` (alone in `fuser`), sockets `dsi` and `hdmi`. Its log stayed empty with `-v`: no error, and grants are not logged | observed 2026-11-28 |
| Harness on HDMI-A-1: `drm-harness --source dlm --lease hdmi --seconds 40 modeset` | Lease granted (`lease route=drm-lease-manager`). The engine sees output 102 / HDMI-A-1 and **only the leased planes 91, 237, 248**. 76 commits latched at 640x480@75, then switch to 1920x1080@60 (`commit_us=49001`), **2398 of 2398 commits latched**, original 640x480@75 restored. `VERDICT modeset PASS` | observed 2026-11-28 |
| Compositor started while the lease manager is master | Exits by itself: `Error: Error presenting framebuffer on screen: Permission denied (os error 13)`. The HDMI harness kept running | observed 2026-11-28 |
| Stand-in for the compositor, **not a compositor**: a second harness, `drm-harness --source dlm --lease dsi --seconds 20 two-planes` | Lease granted; the engine sees output 90 / DSI-1 and only planes 79, 127, 138; planes 79 + 127 committed; `VERDICT two-planes PASS 2 buffers on planes 79,127 (bottom,top)` | observed 2026-11-28 |
| Both lease clients at once (08:50:04Z) | `fuser`: drm-lease-manager + two `drm-harness`. debugfs: plane 79 (720x1280) and plane 127 (360x640) on CRTC 90 `allocated by = drm-harness`, `plane_mask=44`; plane 91 (1920x1080) and plane 237 (960x540) on CRTC 102 `allocated by = drm-harness`, CRTC 102 mode `1920x1080` 60 Hz, `plane_mask=10008` | observed 2026-11-28 |
| HDMI picture (KVM capture, while both clients ran) | 1920x1080: blue full screen with a light edge, red rectangle with yellow border, 960x540 in the middle. This is the harness's `expect` text ("blue full screen with white edge, red rectangle with yellow edge on top in the middle"). See `bcm2711-cm4-coexistence-2026-11-28/hdmi-kvm-capture.png` | observed 2026-11-28 |
| DSI picture | **Not verified on the glass**: nothing captures the DSI panel. Evidence is the harness verdict and the driver's debugfs state above | observed 2026-11-28 (state only) |
| Underruns | `hvs_underrun` 0 before and after | observed 2026-11-28 |
| Kernel log | No new `dmesg` lines during the trial | observed 2026-11-28 |

The whole sequence ran twice on 2026-11-28 with the same outcome. On the first pass the HDMI `modeset` run
held 60 s (3601 of 3601 commits latched after the switch). Its log files were removed from the board
before they were copied, so the log in the evidence directory and the numbers above are from the second
pass (`run.sh`, 08:49:34Z to 08:50:33Z). Both KVM captures, one per pass, showed the same picture.

## Verdict lines

```text
second sink present: yes (HDMI-A-1 connected to the NanoKVM, EDID 256 bytes, besides DSI-1)
compositor available: yes (the Slint launcher on the image, pinned to DSI-1 with SLINT_DRM_OUTPUT), but it cannot run beside drm-lease-manager: whichever opens first holds DRM master
lease granted: yes when drm-lease-manager is master (HDMI-A-1 and DSI-1 leases); no (EACCES) while the compositor is master
harness frame on leased output: yes (HDMI-A-1, 1920x1080, KVM capture; modeset PASS, 2398/2398 latched), with a second harness, not a compositor, on the DSI-1 lease (two-planes PASS)
board restored: yes
```

## State after

| Item | State | Tag |
|---|---|---|
| Processes | drm-lease-manager, both harnesses and the launcher stopped; `fuser -v /dev/dri/card0` empty | observed 2026-11-28 |
| Display | Plane 79 (CRTC 90, 720x1280) and plane 91 (CRTC 102, 640x480@75) back on `[fbcon]`; planes 127 and 237 off; `plane_mask` 4 and 8 as before | observed 2026-11-28 |
| Services | `slint-launcher` inactive and disabled, `lightdm` inactive, as found | observed 2026-11-28 |
| `/tmp` | `/tmp/cx` removed; only the systemd-private directories remain, as before | observed 2026-11-28 |
| Configuration | No install, config.txt, overlay, boot chain, udev, seat or DRM change; no module load; no reboot | observed 2026-11-28 |

## What remains (not done here)

- A real compositor as a lease client. One way is to give a compositor drm-lease-manager support, as AGL's
  Weston has. The other is to use the launcher's own Wayland DRM lease (`wp_drm_lease_device_v1`) with a
  Wayland lease client. Neither exists in this repository, and neither was tried.
- The DSI panel's picture is not captured (no camera or capture on DSI).
