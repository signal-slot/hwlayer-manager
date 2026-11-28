# vc4 HVS board session, reTerminal (BCM2711, HVS5), 2026-11-28

Board answers for `docs/hvs-backend-design.md` §7 from a short session on a
reTerminal board (Raspberry Pi CM4, BCM2711, vc4 driver, HVS5 generation),
run 2026-11-28. Procedure: `docs/rk-hvs-session-checklist.md` section
`cm4-hvs5-session`.

Board facts (observed 2026-11-28): device `/dev/dri/card0`, driver `vc4
version 0.0.0 date 0 desc Broadcom VC4 graphics`, kernel
`6.12.47+rpt-rpi-v8` (KERNEL line printed by the scale and replace runs).
Outputs: HDMI-A-1 (connector 33) connected, console running 640x480@75Hz on
crtc 102, preferred mode 1920x1080@60Hz; DSI-1 (connector 47) connected,
panel 720x1280@60Hz on crtc 90, panel orientation "Right Side Up"; HDMI-A-2
disconnected; a Writeback connector exists. This board has no camera, so no
picture checks were possible.

Tags: **observed** (a dated board measurement; the tool's own UTC `DATE`
stamp quoted where printed), **read from source**, **inferred**. `probe` and
`props` print no clock; they ran earlier the same day (2026-11-28) in this
session, before the 13:08:01Z scaling run.

Tools run from the board's `/tmp` (static aarch64 builds): `probe --raw`,
`props`, `scale_probe --max-factor 64`, `kms_sweep replace` (three runs). All
copied files were removed afterwards.

## Q1: scaling values (HVS5)

Observed 2026-11-28 (`scale_probe --max-factor 64`, DATE 2026-11-28T13:08:01Z,
destination 960x540 — half of the connector's preferred 1920x1080 mode —
17 planes probed: primary 91 plus the 16 overlays usable on that CRTC):

```text
RESULT plane=91 kind=Primary format=XR24 max_upscale_h=64.0000+ max_upscale_v=64.0000+ max_upscale_both=64.0000+ downscale=accepted min_downscale=0.5000
VERDICT scale_probe: OK 17 plane(s) probed at dst 960x540, resolution 256/65536
```

All 17 planes (primary and overlays) show the identical `RESULT` line: no
rejection at any factor up to the tried 64.0 on any axis (the `+` means the
ladder ended without a rejection), and downscale 0.5, 0.75 and 0.9 all
accepted. `min_downscale=0.5` is the smallest of the three factors the tool
tries, not a measured limit (`scale_probe.rs`, read from source).

Answer (observed 2026-11-28): on this kernel `TEST_ONLY` refuses nothing it
is asked between 0.5 and 64 in either direction, on any plane of the tested
CRTC. This matches the source record — the helper is called with min 1 and
max `INT_MAX` (`docs/vc4-hvs-scaling-source.md`, read from source) — and
narrows §7 Q1 to: a table may claim at least 0.5 .. 64 on HVS5; the true
upper bound was not found (inferred). No HVS6 measurement was possible (see
"Not measured").

## Q5 context (HVS5 arithmetic; BCM2712 not measured)

Observed 2026-11-28 (`probe --raw` and the props dump): 60 planes, 6 CRTCs
(59, 78, 90, 102, 114, 126) and 4 encoders plus the writeback connector:

- 6 primary planes, one fixed to each CRTC, `zpos` immutable `0..=0`.
- 48 overlay planes: 16 of them (ids 127..292, step 11) usable on CRTCs 78,
  90, 102, 114 and 126; 32 of them (303..644) usable only on CRTC 59. All
  overlays have `zpos` mutable `1..=17`.
- 6 cursor planes, one per CRTC (655, 666, 677, 688, 699, 710). Driver caps:
  cursor width 64, cursor height 64. So on HVS5 the answer to "are the extra
  per-CRTC planes cursors" is yes, exactly one cursor each — but §7 Q5 asks
  about BCM2712, which was not measured (inferred from this board alone).
- Every probed plane lists `IN_FORMATS (49)`: 34 linear pairs plus 15
  modifier pairs (8 `0x700000000000001` T-tiled RGBA/16-bit formats, 3
  `0x700000000000004`, 2 `0x700000000000003`, 2 `0x700000000000005` NV12/
  NV21 variants; P030 only as `0x…04`).

Side observation for Q2 (bandwidth) (observed 2026-11-28): the kernel's own
debugfs dump reports the HVS load it computes — `Core Clock Rate: 71428000`,
`Channel 0: in use=1 load=71428000`, `Channel 1: in use=1 load=32000000`,
`Channel 2: in use=0 load=0` while the DSI panel and the HDMI console were on.
That confirms `vc4_plane_calc_load`'s numbers are visible on a running board;
no load threshold was measured, so Q2 stays open (observed 2026-11-28).

## Q6: replace_pending (HVS5, kernel 6.12.47+rpt)

Observed 2026-11-28 (`kms_sweep replace /dev/dri/card0 --wait-ms 1000`, three
runs, DATE 2026-11-28T13:11:59Z, 13:12:04Z, 13:12:06Z):

```text
SETUP commit=ok events=1
REPLACE first=ok second=EBUSY events=1
VERDICT kms_sweep: DONE replace
```

All three runs identical (stable over three samples). Answer (observed
2026-11-28): a non-blocking flip that has not latched yet is **not**
replaceable on vc4/HVS5 — the second commit returns `EBUSY` — and one event
arrives per accepted commit. This confirms the source inference
(`replace_pending = false`, `docs/drm-pending-commit-source.md`, read from
source) on a board kernel older than the pinned source (6.12 against
`rpi-6.18.y`), and matches the earlier BCM2711/BCM2712 records of 2026-11-27
in `docs/rk3588-vc4-profiles.md`.

## Q7: which kernel — pinned refs still canonical, boards differ

Host-side check the same day (2026-11-28, read-only `git ls-remote`):

- `raspberrypi/linux` branch `rpi-6.18.y` head is
  `16ae2649dcfadf9d81445b1177a3191efa3cb82e` — the exact commit every vc4
  source document pins (`docs/vc4-mainline-source.md`,
  `docs/drm-pending-commit-source.md`, `docs/vc4-hvs-scaling-source.md`).
- Branch `rpi-6.12.y` head is `8f5f2e15840330091c7116b2ef9454108d4bc423` —
  the exact "head 6.12" pin in `docs/vc4-transform-source.md`.

So the pins name the current heads of both lines today (observed 2026-11-28,
host check). This board runs the 6.12 line (`6.12.47+rpt-rpi-v8`); the Pi 5
record of 2026-11-28 runs a 6.18 rpt build. Whether the 6.12 head commit is
exactly what `6.12.47+rpt-rpi-v8` was built from was not verified: file
downloads (clone/fetch) are refused on this worker, so a content diff of the
vc4 driver between the two lines could not be run here. `docs/vc4-transform-source.md`
already compares `vc4_plane.c` between these two pins (read from source).

Behavior check (observed 2026-11-28): the pinned-6.18 source predictions that
this session could test — pending commit not replaceable (§2 there,
`replace_pending = false`), scaling unbounded at the helper level — held on
the 6.12 board (Q1, Q6 above). For the questions this session measured, the
kernel difference changed nothing; the remaining differences are those
`docs/vc4-mainline-source.md` and `docs/vc4-transform-source.md` record (read
from source).

## Not measured

- The BCM2712 (Raspberry Pi 5) session `rpi5-hvs6-session` from
  `docs/rk-hvs-session-checklist.md`: every ssh login tried from this worker
  for that board was refused (publickey denied), and no remote-agent path was
  permitted here. So §7 Q1-HVS6, Q5, Q10 stay open; this document adds only
  HVS5 answers. The 2026-11-27 BCM2712 records in `docs/rk3588-vc4-profiles.md`
  remain the only BCM2712 board data.
- KVM pictures: no KVM device was available for this board (it has no camera),
  and the Pi 5's KVM was unreachable with the board.

## Leave-as-found record

- `display-guard.sh detect` printed no active display unit before and after the
  runs (observed 2026-11-28): console system, nothing was stopped or started.
- Copied tools removed from the board's `/tmp`; the final `ls /tmp` showed
  only `systemd-private-*` directories (observed 2026-11-28).
- Final kernel state dump (observed 2026-11-28): crtc 90 (pixelvalve-1, DSI)
  active 720x1280 with its primary plane 79 on framebuffer 722
  (`allocated by = [fbcon]`) — untouched by the runs; crtc 102 (pixelvalve-2,
  HDMI-A-1) active again at mode "640x480": 75 — the mode found — with primary
  plane 91 back on framebuffer 722 (`mode_changed=1` records the session's
  test mode and the console's restore). All overlay and cursor planes
  `crtc=(null) fb=0`. The board is as found.
