# BCM2711 (reTerminal CM4): two active outputs, DSI-1 and HDMI-A-1 at once (HVS Q7)

Date: 2026-11-28. Rows tagged **observed 2026-11-28** were measured on the board over ssh, and on HDMI-A-1
through the NanoKVM capture, on 2026-11-28 between 10:33Z and 10:39Z (host UTC; the board clock was checked
against the host and agreed within 1 s). This answers the CM4 half of question Q7 ("two active outputs") of
`docs/design/hvs-backend.md` §11 for the BCM2711 (HVS5). It is portability evidence for SoW §5.4 step 3 and
X-07 in `docs/open-issues.md`. It counts toward no milestone (not M1, M2 or M3).

Evidence, in `bcm2711-cm4-two-outputs-2026-11-28/`:

- `log.txt`: the scrubbed log. It holds every client log, both drm-lease-manager logs, the `probe` output
  and all ten debugfs snapshots (state extract, `hvs_underrun`, `hvs_dlists`, `hvs_dlist_allocs`).
- `dlm-a.toml`, `dlm-b.toml`: the two lease configurations.
- `hdmi-kvm-capture.png`: the NanoKVM capture of HDMI-A-1 taken while both outputs were driven by the
  engine, reduced from 1920x1080 to 480x270.
- `expect-hdmi-two-planes.json`: the camera-check expectation for that picture.
- `expect-twoplane-hdmi-640x480.json`: what `scripts/twoplane-expect.py` wrote from the HDMI `twoplane`
  run of phase C1. **No capture exists to judge it against** (see "Capture limits").

## Short answer

- **Route used: drm-lease-manager, one lease per connector, each with an explicit plane list** (the route of
  `bcm2711-cm4-coexistence-2026-11-28.md`), for phases A and B. Phase C adds the second route: one engine
  process as DRM master on one output while the other CRTC stays active on fbcon (observed 2026-11-28).
- **Both outputs driven by the engine at once: yes** (observed 2026-11-28). HDMI-A-1 ran 1920x1080@60 with
  planes 91 + 204, and **5399 of 5399 commits latched**. DSI-1 ran 720x1280@60 with planes 79 + 127 (PASS). The
  KVM capture of HDMI passes camera-check.
- **Per-CRTC overlays**: the kernel offers every overlay to both CRTCs. `probe` gives mask `0b11` for overlays
  127 … 248, and 259 … 292 also reach HDMI-A-1 through the lease. Only the primaries and cursors are tied to
  one CRTC: 79 and 677 to CRTC 90 (DSI-1), 91 and 688 to CRTC 102 (HDMI-A-1). Which overlays a CRTC gets is
  therefore decided by userspace: the lease's plane list, or whatever plane the planner takes first
  (observed 2026-11-28).
- **Overlay sharing: one overlay serves one CRTC at a time, and it can move between CRTCs.** Overlay 237 was
  listed in both leases. While the HDMI lessee held it, the DSI lease request failed in
  `drmModeCreateLease` with `Device or resource busy`, and the same happened the other way round. After the
  holder exited, the other connector's lease got 237 and the engine put the DSI overlay on it (CRTC 90). In
  master mode the planner took overlay 127 for HDMI-A-1 (phase C1) and, in the next run, the same 127 for
  DSI-1 (phase C2) (observed 2026-11-28).
- **Dlist budget with both outputs active: at most 107 of 4064 words allocated**, of which 22 are the two
  permanent filter kernels. Frame dlists: **17 words per CRTC** for two unscaled RGB planes (8 + 8 + end
  word). The rest of the 107 are blocks still waiting to be freed after the next frame (`Stale nodes`). That is
  85 frame words, about 2 % of the 4042 words left after the kernels (observed 2026-11-28; the 4064/4042
  split is read from source, `docs/vc4-hvs-dlist-source.md` §2).
- **`hvs_underrun`: 0 in every snapshot**, from before the session to after restoration (ten snapshots,
  observed 2026-11-28). No kernel log lines were added during the session (observed 2026-11-28).

## Before the session

| Item | Value | Tag |
|---|---|---|
| Board, kernel, card | reTerminal CM4 (BCM2711), Raspberry Pi OS trixie arm64, `6.12.47+rpt-rpi-v8`; `card0` = vc4, debugfs `dri/0` (also `dri/gpu`) | observed 2026-11-28 |
| Connectors | `DSI-1` connected, enabled; `HDMI-A-1` connected, enabled (NanoKVM); `HDMI-A-2` disconnected; `Writeback-1` connected, disabled | observed 2026-11-28 |
| DRM master | Nobody (`fuser -v /dev/dri/card0` empty); `slint-launcher` and `lightdm` inactive, `slint-launcher` disabled | observed 2026-11-28 |
| CRTCs | CRTC 90 (`pixelvalve-1`) → DSI-1, 720x1280@60, plane 79 on `[fbcon]`; CRTC 102 (`pixelvalve-2`) → HDMI-A-1, 640x480@75, plane 91 on `[fbcon]` | observed 2026-11-28 |
| `hvs_dlists` | `HVS chan 0`: 9 words (one plane, 720 wide) = DSI; `HVS chan 1`: 9 words (640 wide) = HDMI; `HVS chan 2 disabled` | observed 2026-11-28 |
| `hvs_dlist_allocs` | `[0x20 + 0xb]`, `[0x2b + 0xb]` (two 11-word filter kernels), `[0x37 + 9]`, `[0x40 + 9]`: 40 words | observed 2026-11-28 |
| `hvs_underrun` | 0 | observed 2026-11-28 |
| `probe` (master, nobody else on the card) | output 0 = CRTC 102 640x480@75, output 1 = CRTC 90 720x1280@60; primary 79 `0b10`, primary 91 `0b1`; overlays 127 … 248 `0b11`; cursor 677 `0b10`, cursor 688 `0b1`. `KEPT` lines: both outputs keep primary, 12 overlays and their cursor; 259 … 292 are cut by `MAX_PLANES`, as in `bcm2711-cm4-x09-2026-11-28.md` | observed 2026-11-28 |

## Build and set-up

| Item | Value | Tag |
|---|---|---|
| Engine binaries | `scripts/cross-aarch64.sh` (no arguments: `probe` and `drm-harness`) and `scripts/cross-aarch64.sh -p composition-drm --example twoplane`, at commit 54571bb, static aarch64 musl | observed 2026-11-28 |
| drm-lease-manager | `scripts/build-dlm-aarch64.sh <deps dir>` with the pins of `docs/dlm-aarch64.md` (drm-lease-manager `68c48758c46d93c8e83abb2fb1ec168fa13d1a53`, libdrm `25dec5b91fe4d2638787d033a0b22b6c1dc145e0`, tomlc99 `62ce93ceb713dc49909d0fef1411f9d37bef83f2`), clang route; static aarch64 ELF. Build tree in the task's temporary directory, not `/tmp` | observed 2026-11-28 |
| On the board | Everything in `/tmp/q7` (binaries, TOMLs, logs, `DLM_RUNTIME_PATH=/tmp/q7/dlm`). Nothing installed; no unit, config.txt, udev, seat or DRM configuration changed; no module loaded | observed 2026-11-28 |
| Lease manager | `sudo env DLM_RUNTIME_PATH=/tmp/q7/dlm /tmp/q7/drm-lease-manager --config <toml> -v -u /dev/dri/card0` | observed 2026-11-28 |
| Debugfs | `dri/0/state`, `hvs_underrun`, `hvs_dlists`, `hvs_dlist_allocs`, read with `sudo` only | observed 2026-11-28 |

## Phase A: both outputs on the engine, disjoint leases (10:34:20Z to 10:35:51Z)

Configuration `dlm-a.toml`: lease `hdmi` = HDMI-A-1 with planes 91, 204, 215, 226, 237, 248, 259, 270, 281,
292, 688; lease `dsi` = DSI-1 with planes 79, 127, 138, 149, 160, 171, 182, 193, 677.

| Item | Value | Tag |
|---|---|---|
| HDMI client | `drm-harness --source dlm --lease hdmi --seconds 90 modeset`. The engine sees output 102 and exactly the 11 leased planes (primary, 9 overlays, cursor). 76 commits latched at 640x480@75; switch to 1920x1080@60 (`commit_us=46855`); **5399 of 5399 commits latched**; 640x480@75 restored | observed 2026-11-28 |
| HDMI planner pick | `plan ... planes=91,204` before and after the switch: primary 91 for the full-screen layer, **overlay 204** (the first overlay of the lease) for the 960x540 layer at 480,270 | observed 2026-11-28 |
| DSI client (started 3 s later) | `drm-harness --source dlm --lease dsi --seconds 80 two-planes`. The engine sees output 90 and the 9 leased planes; `plan ... planes=79,127`: primary 79 and **overlay 127** (360x640 at 180,320) | observed 2026-11-28 |
| Debugfs, both active (10:34:28Z) | `fuser`: drm-lease-manager + two `drm-harness`. Plane 79 (720x1280) and 127 (360x640, zpos 1) on `pixelvalve-1`; plane 91 (1920x1080) and 204 (960x540, zpos 1) on `pixelvalve-2`; CRTC 90 `plane_mask=44`, CRTC 102 `plane_mask=2008`, mode `1920x1080` 60 Hz | observed 2026-11-28 |
| `hvs_dlists`, both active | `HVS chan 0` (DSI, `0x050002d0` = 1280 x 720): 17 words, two 8-word plane entries (`0x4800d807`) + `0x80000000` end. `HVS chan 1` (HDMI, `0x04380780` = 1080 x 1920 for the primary, `0x021c03c0` = 540 x 960 for the overlay): 17 words. In the HDMI entries, words 3 and 5 (the context words) read back `0xc0c0c0c0`. This was not interpreted | observed 2026-11-28 (values); word meaning inferred from `docs/vc4-hvs-dlist-source.md` §3 |
| `hvs_dlist_allocs`, both active | 10:34:28Z: kernels 22 + four 17-word nodes = **90 words**, no stale node. 10:34:59Z: kernels 22 + five 17-word nodes = **107 words**, two of them in `Stale nodes` (`channel 1 frcnt 27`, `28`: HDMI blocks waiting for the frame counter) | observed 2026-11-28 |
| HDMI picture (KVM, 10:34:3xZ) | 1920x1080: blue full screen with a light edge, red 960x540 rectangle with a yellow edge in the middle, the harness's `expect` text. camera-check, below: PASS | observed 2026-11-28 |
| DSI picture | **Not verified on the glass**: nothing captures the DSI panel. Evidence is the harness verdict and debugfs | observed 2026-11-28 (state only) |
| `hvs_underrun` | 0 before, during (twice) and after | observed 2026-11-28 |
| After both clients exited, lease manager still master (10:35:51Z) | Both CRTCs stay active with **no plane** (`plane_mask` empty in the extract; dlist 1 word per channel, the end word): the lessor turns nothing off, and fbcon does not come back while drm-lease-manager is master | observed 2026-11-28 |
| After drm-lease-manager stopped (10:35:59Z) | Planes 79 and 91 back on `[fbcon]`, 40 dlist words as before | observed 2026-11-28 |

## Phase B: overlay sharing, overlay 237 in both leases (10:36:10Z to 10:37:02Z)

Configuration `dlm-b.toml`: lease `hdmi` = HDMI-A-1 with planes 91, 237; lease `dsi` = DSI-1 with planes 79,
237. drm-lease-manager allows a shared plane (possible on more than one CRTC) only when the connector's plane
list is explicit (`lease-manager.c:lease_add_planes`, `allow_shared`), and it creates a lease only when a
client asks (read from source, drm-lease-manager `68c48758c46d93c8e83abb2fb1ec168fa13d1a53`). The kernel
refuses a lease object that the lessor has already leased to someone else: `drm_lease.c:drm_lease_create`
returns `-EBUSY` when `_drm_has_leased()` is true for an object (read from source, `raspberrypi/linux` `rpi-6.12.y` at
`8f5f2e15840330091c7116b2ef9454108d4bc423`, commit date 2026-11-23; later than the CM4's 6.12.47 build, so
evidence for the series, not proof for that build).

| Step | Result | Tag |
|---|---|---|
| B1: HDMI client (`two-planes`, 25 s) | Lease granted; `plan ... planes=91,237`; overlay 237 on `pixelvalve-2` (debugfs 10:36:14Z); `VERDICT two-planes PASS 2 buffers on planes 91,237 (bottom,top)` | observed 2026-11-28 |
| B2: DSI client while B1 holds 237 | `VERDICT two-planes FAIL ... lease refused: the lease manager closed the connection`; drm-lease-manager: `ERROR: drmModeCreateLease failed on lease dsi: Device or resource busy` | observed 2026-11-28 |
| B3: DSI client after B1 exited | Lease granted; `plan ... planes=79,237`; **overlay 237 now on `pixelvalve-1`** (CRTC 90) (debugfs 10:36:39Z); `VERDICT two-planes PASS 2 buffers on planes 79,237 (bottom,top)` | observed 2026-11-28 |
| B4: HDMI client while B3 holds 237 | `VERDICT two-planes FAIL ... lease refused`; `ERROR: drmModeCreateLease failed on lease hdmi: Device or resource busy` | observed 2026-11-28 |
| Debugfs in B | B2: DSI channel 9 words (plane 79 still on `[fbcon]`: the new lessor had not touched DSI), HDMI channel 17 words. B3: DSI 17 words, HDMI 1 word (CRTC 102 active, no plane). `hvs_underrun` 0 | observed 2026-11-28 |

So an overlay is not shared between CRTCs in the same frame: one lease, and so one CRTC, holds it at a time.
It moves between CRTCs once the holder lets go. On this hardware, the overlay pool of 16 (plus the 4 that
`MAX_PLANES` hides) is shared by both outputs, and userspace has to split it (inferred, from B and from the
`0b11` masks).

## Phase C: one engine process as master, the other CRTC on fbcon (10:37:16Z to 10:38:33Z)

| Item | Value | Tag |
|---|---|---|
| C1: `twoplane /dev/dri/card0 --output 0 --frames 2400` (HDMI-A-1) | `MODE 640x480@75.000Hz output 0 (crtc 102) of 2`; layer 0 → primary 91, layer 1 → **overlay 127**; `RESULT frames=2400 committed=2400 latched=2400 released=4798 dropped=0 late=0`; `VERDICT twoplane: PASS` | observed 2026-11-28 |
| C1 debugfs (10:37:20Z) | Plane 79 on `[fbcon]` on CRTC 90 (DSI stays active); planes 91 (640x480) and 127 (128x240, zpos 1) on CRTC 102, `allocated by = twoplane`; `hvs_underrun` 0; allocations 22 + 9 + four 17-word nodes = 99 words, one stale (`channel 1`). `hvs_dlists` printed `HVS chan 1 disabled` in this snapshot | observed 2026-11-28 |
| Why `chan 1 disabled` | The debugfs dump prints "disabled" when the channel's `SCALER_DISPSTATX` mode is `DISABLED` **or `EOF`** (`vc4_hvs.c:vc4_hvs_debugfs_dlist`, `rpi-6.12.y` at `8f5f2e15840330091c7116b2ef9454108d4bc423`). The read probably fell between end of frame and the next start. The CRTC was active and every frame latched | read from source; the timing explanation is inferred |
| C2: `twoplane /dev/dri/card0 --output 1 --frames 1800` (DSI-1) | `MODE 720x1280@60.000Hz output 1 (crtc 90) of 2`; layer 0 → primary 79, layer 1 → **overlay 127** (144x640 at 360,320); `RESULT frames=1800 committed=1800 latched=1800 released=3598 dropped=0 late=0`; `VERDICT twoplane: PASS` | observed 2026-11-28 |
| C2 debugfs (10:38:03Z) | Planes 79 and 127 on CRTC 90 (`twoplane`); plane 91 on `[fbcon]` on CRTC 102; channel 0 17 words, channel 1 9 words; `hvs_underrun` 0 | observed 2026-11-28 |
| Planner pick in master mode | With the full plane list, the planner takes the lowest-numbered overlay that reaches the output, 127, for either output. Two engine processes cannot both be master, so this route never has the engine on both outputs (inferred, from C1 and C2) | observed 2026-11-28 (picks); rest inferred |

## Dlist budget against `profile/vc4.rs`

| Snapshot | Allocated words (of 4064) | Frame words per channel (DSI / HDMI) | `hvs_underrun` | Tag |
|---|---|---|---|---|
| Idle, both on fbcon | 40 | 9 / 9 | 0 | observed 2026-11-28 |
| A, both on the engine (10:34:28Z) | 90 | 17 / 17 | 0 | observed 2026-11-28 |
| A, both on the engine (10:34:59Z) | 107 (2 stale) | 17 / 17 | 0 | observed 2026-11-28 |
| A, clients gone, lessor master | 24 | 1 / 1 | 0 | observed 2026-11-28 |
| B2 / B3 | 48 / 40 | 9 / 17, then 17 / 1 | 0 | observed 2026-11-28 |
| C1 / C2 | 99 (1 stale) / 82 (1 stale) | 9 / (dump said disabled), then 17 / 9 | 0 | observed 2026-11-28 |
| After restoration | 40 | 9 / 9 | 0 | observed 2026-11-28 |

- One unscaled RGB plane costs 8 dlist words on HVS5 and a CRTC adds 1 end word. This matches the source
  count of `docs/vc4-hvs-dlist-source.md` §3 (observed 2026-11-28 against read from source).
- Both CRTCs allocate from **one** pool. Their blocks sit side by side in `hvs_dlist_allocs` (offsets 0x36 …
  0x85), with no per-channel split. This agrees with `drm_mm_init(&hvs->dlist_mm, 32, 4064)` in the source
  (observed 2026-11-28; source as in `docs/vc4-hvs-dlist-source.md` §2).
- Stale blocks double the footprint of an output that commits every frame for a short while. The highest seen
  was 107 words with two outputs and two planes each. The comment in `crates/composition-drm/src/profile/vc4.rs`
  ("dlist words (4064 shared by all outputs, far above one frame's need)") holds for this load. Leaving the
  dlist to `TEST_ONLY` is consistent with it (inferred). A load ladder that comes near the limit is Q2's job,
  not this run's.

## Capture limits

- The NanoKVM cannot capture HDMI at 640x480@75, the mode fbcon drives: `failed to capture screenshot` before
  the session, twice during C1 (while twoplane drew at 640x480@75), and after restoration
  (observed 2026-11-28). `twoplane` has no mode option and uses the CRTC's current mode, so the C1 picture was not
  captured. `expect-twoplane-hdmi-640x480.json` (from `scripts/twoplane-expect.py --rotation 0 --panel full`)
  is stored, but nothing was judged against it.
- The judged capture is therefore the phase A one: drm-harness `modeset` switched HDMI to 1920x1080@60,
  which the KVM captures. The harness prints no `EXPECT` lines, so the camera-check expectation
  (`expect-hdmi-two-planes.json`) was written by hand. Its geometry comes from the harness's `assign` lines and
  its colours from `tools/drm-harness/src/draw.rs` (`BLUE` 0x002080, `RED` 0xE01010).

## Camera-check

`python3 scripts/camera-check.py hdmi-kvm-capture.png expect-hdmi-two-planes.json --rotation 0`
(observed 2026-11-28; the same result on the full-size 1920x1080 capture and on the stored 480x270 reduction):

```text
PASS  bg_top_left      measured ( 20.0, 40.0,126.0) expected (  0.0, 32.0,128.0) distance   21.6 tol  50.0
PASS  bg_top_right     measured ( 20.0, 40.0,126.0) expected (  0.0, 32.0,128.0) distance   21.6 tol  50.0
PASS  bg_bottom_left   measured ( 20.0, 40.0,126.0) expected (  0.0, 32.0,128.0) distance   21.6 tol  50.0
PASS  bg_bottom_right  measured ( 20.0, 40.0,126.0) expected (  0.0, 32.0,128.0) distance   21.6 tol  50.0
PASS  bg_left_of_top   measured ( 20.0, 40.0,126.0) expected (  0.0, 32.0,128.0) distance   21.6 tol  50.0
PASS  top_centre       measured (196.0,  7.0, 29.0) expected (224.0, 16.0, 16.0) distance   32.2 tol  50.0
PASS  top_inner_tl     measured (196.0,  7.0, 29.0) expected (224.0, 16.0, 16.0) distance   32.2 tol  50.0
PASS  top_inner_br     measured (196.0,  7.0, 29.0) expected (224.0, 16.0, 16.0) distance   32.2 tol  50.0
VERDICT: PASS
```

## Verdict lines

```text
A HDMI-A-1 (lease hdmi): VERDICT modeset PASS 640x480@75 -> 1920x1080@60: 76 commits latched before, 5399 after the switch; 640x480@75 restored
A DSI-1    (lease dsi):  VERDICT two-planes PASS 2 buffers on planes 79,127 (bottom,top)
A HDMI-A-1 KVM capture:  VERDICT: PASS (camera-check)
B1 HDMI-A-1: VERDICT two-planes PASS 2 buffers on planes 91,237 (bottom,top)
B2 DSI-1:    VERDICT two-planes FAIL open /tmp/q7/dlm/dsi: lease refused: the lease manager closed the connection   (expected: 237 held by HDMI, EBUSY)
B3 DSI-1:    VERDICT two-planes PASS 2 buffers on planes 79,237 (bottom,top)
B4 HDMI-A-1: VERDICT two-planes FAIL open /tmp/q7/dlm/hdmi: lease refused: the lease manager closed the connection  (expected: 237 held by DSI, EBUSY)
C1 HDMI-A-1 (master): VERDICT twoplane: PASS 2400 frames on two planes, 2400 latched, 4798 released, 0 late, driver vc4 mode 640x480@75.000Hz, swapz=off
C2 DSI-1    (master): VERDICT twoplane: PASS 1800 frames on two planes, 1800 latched, 3598 released, 0 late, driver vc4 mode 720x1280@60.000Hz, swapz=off
hvs_underrun: 0 in all ten snapshots
board restored: yes
```

## State after (restoration)

| Item | State | Tag |
|---|---|---|
| Processes | drm-lease-manager, every `drm-harness` and `twoplane` stopped; `fuser -v /dev/dri/card0` empty (10:39:15Z) | observed 2026-11-28 |
| Display | Plane 79 on CRTC 90 (720x1280) and plane 91 on CRTC 102 (640x480@75) back on `[fbcon]`; `plane_mask` 4 and 8, as before; dlist 40 words, as before; `hvs_underrun` 0 | observed 2026-11-28 |
| KVM | The capture fails, as it did before the session, because fbcon drives HDMI at 640x480@75 (see "Capture limits"). That fbcon owns HDMI is shown by debugfs, not by a picture | observed 2026-11-28 |
| Services | `slint-launcher` inactive and disabled, `lightdm` inactive, as found | observed 2026-11-28 |
| `/tmp` | `/tmp/q7` removed; only the systemd-private directories remain, as before | observed 2026-11-28 |
| Kernel log | No `dmesg` line during the session (none after uptime 81900 s, about 10:31Z). An older vc4 HDMI warning from about five hours earlier (`vc4_hdmi_write_infoframe`, process `wireplumber`) is not from this session | observed 2026-11-28 |
| Configuration | No install, config.txt, overlay, boot chain, udev, seat or DRM change; no module load; no reboot | observed 2026-11-28 |

## What remains (not done here)

- A picture of the HDMI `twoplane` run, judged with `scripts/twoplane-expect.py`. It needs HDMI in a mode the
  KVM captures (1920x1080 or 1280x720) before `twoplane` starts. `twoplane` has no mode option, and fbcon's
  640x480@75 cannot be captured.
- The DSI panel's picture (no capture on DSI).
- The dlist near its limit, and underruns under load (Q2, item cm4-q2-load).
- The Pi 5 half of Q7 (HDMI-A-1 and HDMI-A-2), item rpi5-q7-two-outputs.
