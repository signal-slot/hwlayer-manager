# STM32MP257F-DK (LTDC): board session A, 2026-11-28

Session A of the open questions in
[`design/stm32mp257-backend.md`](design/stm32mp257-backend.md) §7: Q1
(pixel colours), Q2 (premultiplied or straight alpha), Q3 (scaling) and Q7
(underrun under load). Q5, Q6, Q9 and Q10 are left to session B (placeholders
at the end).

These runs are **portability evidence only**. They count toward neither M1,
M2 nor M3.

## Tags, clock and conditions

- **observed 2026-11-27** (the host's UTC date): seen on the board in this
  session. Times are UTC from
  the **host** clock. The session ran from 2026-11-27T22:31Z to
  2026-11-27T22:54Z UTC, which is 2026-11-28 in the owner's time zone (JST)
  and gives the file its name. The board clock is not NTP-synchronised. It
  printed times about 9 h 53 min behind the host (for example
  `DATE 2026-11-27T12:41:05Z` in a run started at host 22:34:28Z).
- **read from source**: read from kernel source. This session read no new
  source; such rows cite
  [`observed/stm32mp257-source.md`](observed/stm32mp257-source.md).
- **inferred**: a conclusion of this document, neither seen nor stated.

The board was reached over ssh only. There was no serial console, no reset,
no reboot, no flashing and no boot-chain change, and nothing was installed.
DRM, udev and seat configuration were not changed and no module was loaded.
Before every run, `systemctl stop weston-graphical-session seatd-weston`;
after every run, both were started again.
One write went to debugfs: `echo auto > /sys/kernel/debug/dri/0/crtc-0/crc/control`,
which selects the CRC source that the DRM CRC interface reads while its
`data` file is open (see Q2). At the end of the session the `control` file read
back empty (observed 2026-11-27T22:53Z). Binaries went to the board's `/tmp`
and were removed at the end.

**The KVM could not be used.** The NanoKVM's MCP server
(`kvm_kvm-stm32mp257f-dk`) failed to connect when the session started, with
HTTP 404 from its relay (observed 2026-11-27, as in every earlier session).
So **no screenshot exists**, and `scripts/camera-check.py` has no picture to
judge. Q1 is not answered by a picture. Q2 and the scaling quality of Q3 were
answered by another route, the LTDC's own output CRC through DRM debugfs,
described under Q2.

Binaries: `scripts/cross-aarch64.sh -p composition-drm --example twoplane
--example scale_probe --example probe` and `scripts/cross-aarch64.sh -p
composition-drm-harness`, at commit `35a4a3b4a44a`. They were built for
`aarch64-unknown-linux-musl` (rust-lld) and run unchanged. The CRC probe used
for Q2, Q3 and Q7 is a throwaway program of this session (about 450 lines,
`drm` 0.15.0, the same musl/rust-lld build). It is not in the repository. Its
method is described where it is used.

## Board and kernel

Host time 2026-11-27T22:33:50Z (observed 2026-11-27):

```text
$ uname -a      (host name left out)
Linux <hostname> 6.6.129 #1 SMP PREEMPT Thu Mar  5 15:03:43 UTC 2026 aarch64 GNU/Linux
$ cat /proc/version      ("@" written as " at " for the repository's secret check)
Linux version 6.6.129 (oe-user at oe-host) (aarch64-ostl-linux-gcc (GCC) 13.4.0, GNU ld (GNU Binutils) 2.42.0.20240723) #1 SMP PREEMPT Thu Mar  5 15:03:43 UTC 2026
```

| Item | Value | Tag |
|---|---|---|
| Distribution | `ST OpenSTLinux - Weston - (A Yocto Project Based Distro) 5.0.17-openstlinux-6.6-yocto-scarthgap-mpu-v26.06.10 (scarthgap)` (`/etc/os-release`) | observed 2026-11-27 |
| Kernel recipe | `PREFERRED_PROVIDER_virtual/kernel = linux-stm32mp`; layer `meta-st-stm32mp = v6.2.xml:438ec1ef66b8fca71afe6af869f234353c36127d`; manifest `ostl-v26-05-22-v2-0-g558ff9873069bad011534b410fe865d46e3017ff`; `/etc/version` `20260611072011` (`/etc/build`) | observed 2026-11-27 |
| Kernel git hash | **not in the version string**: `/proc/version` carries no ST tag or git hash. The kernel source revision has to be taken from the `linux-stm32mp` recipe of that `meta-st-stm32mp` revision (for the Q10 diff) | observed 2026-11-27; the recipe route is inferred |
| DRM | driver `stm` 1.0.0 (20170330), `/dev/dri/card0`, connector 32 `HDMI-A-1`, CRTC 41, mode 1920x1080@60.000 Hz, planes 34 (primary), 42, 49 (overlays), the same format lists as `observed/stm32mp257.md` | observed 2026-11-27 (`probe`) |
| CMA | `CmaTotal: 131072 kB` (128 MiB), `CmaFree: 121264 kB` with Weston running | observed 2026-11-27 |
| LTDC counters | in `/sys/kernel/debug/dri/0/state`, CRTC section: `transfer_error`, `fifo_underrun_error`, `fifo_underrun_warning`, `fifo_underrun_rotation`, `fifo_underrun_threshold=128` | observed 2026-11-27 |

The counters are cumulative. Reading `state` does not reset them: a count
of 1012 was still there in the next read, with Weston running in between. They
went back to 0 when a client enabled the CRTC again after Weston was stopped
(observed 2026-11-27). That they are cleared on CRTC enable is inferred. Each
run below therefore quotes the values after its first commit and at its end.

## Q1: pixel colours (twoplane, both `--swap-z` phases, `--plane-alpha 32768`)

**Status: harness runs observed 2026-11-27; pictures not taken (KVM failed).**

Run 1, host 2026-11-27T22:34:28Z to 22:34:51Z:

```text
/tmp/twoplane /dev/dri/card0 --frames 600 --swap-z
PLAN from frame 0:
  layer 0 (background) -> plane 34 Primary zpos 0 src 0,0 1920x1080 dst 0,0 1920x1080
  layer 1 (overlay) -> plane 42 Overlay zpos 1 src 0,0 384x540 dst 960,270 384x540
SWAPZ at frame 300: overlay now below the background
PLAN from frame 300:
  layer 1 (overlay) -> plane 34 Primary zpos 0 src 0,0 384x540 dst 960,270 384x540
  layer 0 (background) -> plane 42 Overlay zpos 1 src 0,0 1920x1080 dst 0,0 1920x1080
RESULT frames=600 committed=600 latched=600 released=1198 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=16.668 latch_interval_max_ms=17.648 elapsed_s=10.053 swapped_at=300
RELEASE after the run: released 1200 of 1200 buffer uses, 0 still on screen
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver stm mode 1920x1080@60.000Hz
```

Run 2, host 2026-11-27T22:35:04Z to 22:35:27Z:

```text
/tmp/twoplane /dev/dri/card0 --frames 600 --plane-alpha 32768
GEOMETRY overlay layer 1 dst 960,270 384x540 format AR24 blend premultiplied plane-alpha 0x8000
RESULT frames=600 committed=600 latched=600 released=1198 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=16.668 latch_interval_max_ms=22.114 elapsed_s=10.053
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver stm mode 1920x1080@60.000Hz
```

LTDC counters stayed `transfer_error=0 fifo_underrun_error=0
fifo_underrun_warning=0` before and after both runs. `dmesg` printed no new
line (observed 2026-11-27).

EXPECT JSON: `scripts/twoplane-expect.py LOG --out DIR` printed `wrote
…/expect-normal.json (8 regions)` and `wrote …/expect-swapped.json (8 regions)`
for run 1, and `wrote …/expect-normal.json (8 regions)` for run 2 (observed
2026-11-27). Both use the default `--panel full`, `--rotation 0` and
tolerance 40.

**camera-check verdict lines: none.** There was no screenshot at about 2 s,
about 7 s or in the alpha run, so `scripts/camera-check.py PICTURE
EXPECT.json` could not be run. Only its `--self-test` ran (`28 checks`,
`PASS`), which says nothing about the board.

What a picture would show is predicted from the Q2 measurement
(inferred). Region names are twoplane's `EXPECT` names; "expected" is
twoplane's `EXPECT` value, which assumes premultiplied blending.

| Run, phase | Region | twoplane EXPECT | Predicted on this board (straight alpha) | Distance | camera-check at tolerance 40 |
|---|---|---|---|---|---|
| run 1 normal | overlay translucent over green | `#807f80` | `#407f40` | 90.5 | would FAIL |
| run 1 normal | overlay translucent over background | `#901090` | `#4f0f4f` | 91.9 | would FAIL |
| run 2 | overlay opaque over green | `#80ff00` | `#80ff00` | 0 | would PASS |
| run 2 | overlay opaque over background | `#909010` | `#8f8f0f` | 1.7 | would PASS |
| run 2 | overlay translucent over green | `#40bf40` | `#20bf20` | 45.3 | would FAIL |
| run 2 | overlay translucent over background | `#581858` | `#371737` | 46.7 | would FAIL |

The opaque regions and the run 1 swapped phase need no blending (opaque
pixels, or the overlay hidden under an opaque background) and are predicted
to match their EXPECT colours (inferred). Z-order in both phases is
therefore still known only from the plans and the kernel accepting them, not
from a picture.

## Q2: premultiplied or straight alpha

**Answer: straight (non-premultiplied) alpha.** The LTDC computes
`out = Cs·α + Cd·(1 − α)` with `α = pixel alpha × plane alpha`, on 8-bit
values with each product truncated. This is observed 2026-11-27 through the
LTDC output CRC, not through a picture. The formula, rounding included, fits
all six measurements below; that the hardware works exactly this way is
inferred.

### Method

The host times were 2026-11-27T22:37:58Z, 22:39:45Z and 22:42:22Z (the third
run is the one quoted). The program ran with Weston stopped:

```text
echo auto > /sys/kernel/debug/dri/0/crtc-0/crc/control
/tmp/crcprobe /dev/dri/card0 32 41 34 42
```

- Plane 34: XR24 1920x1080, all `#202020`. Plane 42: a 384x270 buffer at
  960,540, either the test (AR24, one stored value everywhere, plane alpha
  as given) or a reference (XR24, one opaque colour, plane alpha 0xffff).
  Plane 49 stays off. Each picture is one blocking atomic commit; then
  `crtc-0/crc/data` is opened, three entries are skipped and two are read.
  Every reading in this session gave two equal entries (`unstable=0`).
- The first two runs used magenta (R = B) and gave ambiguous matches. The
  reason (observed 2026-11-27): **references with the same R xor G xor B give
  the same CRC**. `#900000`, `#901010`, `#903737`, `#009000`, `#000090` and
  `#801000` all gave `0x000002c4`; `#818080`, `#808180` and `#808081` all gave
  `0x0000af24`. So the CRC fixes only R⊕G⊕B of the pixels (inferred from
  those cases), and the earlier magenta tests could not separate the two
  modes, because R⊕B cancels.
- Third run: a table of the 256 references `(v, 0, 0)`, `v` = 0..255, gave
  `TABLE refs=256 distinct_crcs=256 collisions=0 unstable=0` (observed
  2026-11-27). So every CRC maps back to one value of R⊕G⊕B. The tests store
  one channel only (red, or green as a check) over the grey background. The
  other two output channels then both get the background term alone, are equal
  and cancel, so R⊕G⊕B is the blended value of the stored channel (inferred).

### Results (observed 2026-11-27, host 22:42:22Z to 22:43:11Z)

```text
TEST red80-a80-plane-ffff stored=#800000a80 plane-alpha=0xffff crc=0x000003b2 0x000003b2 r^g^b=0x4f
TEST green80-a80-plane-ffff stored=#008000a80 plane-alpha=0xffff crc=0x000003b2 0x000003b2 r^g^b=0x4f
TEST red80-a80-plane-8000 stored=#800000a80 plane-alpha=0x8000 crc=0x0000e6ce 0x0000e6ce r^g^b=0x37
TEST redff-aff-plane-8000 stored=#ff0000aff plane-alpha=0x8000 crc=0x0000b1f3 0x0000b1f3 r^g^b=0x8f
TEST redff-a80-plane-ffff stored=#ff0000a80 plane-alpha=0xffff crc=0x0000b1f3 0x0000b1f3 r^g^b=0x8f
TEST red40-a40-plane-ffff stored=#400000a40 plane-alpha=0xffff crc=0x0000c146 0x0000c146 r^g^b=0x27
TEST red00-a00-plane-ffff stored=#000000a00 plane-alpha=0xffff crc=0x0000463d 0x0000463d r^g^b=0x20
```

Predictions use the background `Cd = 0x20` and truncated 8-bit products. The
plane alpha `0x8000` becomes 0x80 (`alpha >> 8`). `α` is pixel alpha × plane
alpha / 255, truncated.

| Stored (R, A), plane alpha | Measured R' | Straight `⌊Cs·α⌋ + ⌊Cd·(1−α)⌋` | Premultiplied `⌊Cs·CA⌋ + ⌊Cd·(1−α)⌋` |
|---|---|---|---|
| 0x80, 0x80, 0xffff | **0x4f** | 0x40 + 0x0f = **0x4f** | 0x80 + 0x0f = 0x8f |
| 0x80, 0x80, 0x8000 | **0x37** | 0x20 + 0x17 = **0x37** | 0x40 + 0x17 = 0x57 |
| 0xff, 0xff, 0x8000 | **0x8f** | 0x80 + 0x0f = **0x8f** | 0x80 + 0x0f = 0x8f (no discrimination) |
| 0xff, 0x80, 0xffff | **0x8f** | 0x80 + 0x0f = **0x8f** | 0xff + 0x0f, saturates at 0xff |
| 0x40, 0x40, 0xffff | **0x27** | 0x10 + 0x17 = **0x27** | 0x40 + 0x17 = 0x57 |
| 0x00, 0x00, 0xffff | **0x20** | **0x20** | 0x20 |

- Observed 2026-11-27: every measured value equals the straight-alpha value,
  and the four cases that can tell the two modes apart all differ from the
  premultiplied value by 0x20 to 0x70.
- Inferred: **the LTDC multiplies the colour by the pixel alpha (straight
  alpha) for AR24 on plane 42**, as `drm_blend` calls "Coverage", although no
  `pixel blend mode` property exists. composition-drm reports
  `blend [premultiplied]` for every LTDC plane ("KMS default when the property
  is absent"). On this hardware that report is wrong: a premultiplied
  client buffer gets its colour multiplied by alpha twice, so translucent
  areas come out too dark (Q1 table).
- Inferred: plane 34 and plane 49 were not tested. That they blend the same
  way is an assumption of this note.
- Read from source (`observed/stm32mp257-source.md`, ST v6.18): no blend-mode
  property is created. Which blending factors the driver programs was not
  re-read for this note; ltdc-src-diff (Q10) should quote them from ST's
  6.6 source.

## Q3: scaling bounds, quality and underrun

### TEST_ONLY bounds (scale_probe)

Host 2026-11-27T22:43:58Z to 22:44:09Z. Weston stopped; every test is
`ATOMIC_TEST_ONLY | ALLOW_MODESET` over a full-screen XR24 primary (observed
2026-11-27):

```text
/tmp/scale_probe /dev/dri/card0 --plane 42 --max-factor 4096
NOTE plane=42 axis=h no rejection up to 960.0000 (limited by the mode size: the source is one pixel)
NOTE plane=42 axis=v no rejection up to 540.0000 (limited by the mode size: the source is one pixel)
NOTE plane=42 axis=both no rejection up to 540.0000 (limited by the mode size: the source is one pixel)
RESULT plane=42 kind=Overlay format=XR24 max_upscale_h=960.0000+ max_upscale_v=540.0000+ max_upscale_both=540.0000+ downscale=rejected min_downscale=none
VERDICT scale_probe: OK 1 plane(s) probed at dst 960x540, resolution 256/65536

/tmp/scale_probe /dev/dri/card0 --plane 49 --max-factor 4096
RESULT plane=49 kind=Overlay format=XR24 max_upscale_h=1.0000 max_upscale_v=1.0000 max_upscale_both=1.0000 downscale=rejected min_downscale=none
VERDICT scale_probe: OK 1 plane(s) probed at dst 960x540, resolution 256/65536

/tmp/scale_probe /dev/dri/card0 --plane 42 --dst 1920x1080 --max-factor 4096
NOTE plane=42 buffer 3840x2160 XR24 refused (Invalid argument (os error 22)); using 1920x1080
RESULT plane=42 kind=Overlay format=XR24 max_upscale_h=1920.0000+ max_upscale_v=1080.0000+ max_upscale_both=1080.0000+ downscale=untested min_downscale=none
VERDICT scale_probe: OK 1 plane(s) probed at dst 1920x1080, resolution 256/65536
```

- Observed 2026-11-27, plane 42 accepted factors (h, v and both): 1, 1.25,
  1.5, 2, 3, 4, 6, 8, 12, 16, 32, 64, 128, 256, 512 and the one-pixel factor
  (960 h, 540 v and both at dst 960x540; at dst 1920x1080 also 1024, 1080 and
  1920). No upscale was rejected. The downscales 0.5, 0.75 and 0.9 were
  rejected on every axis with errno 22.
- Observed 2026-11-27, plane 49: every factor other than 1.0 rejected, errno
  22, down to 1.0039 (the 1/256 search step), on h, v and both.
- This confirms the 2026-11-27 rows of
  [`stm32mp257-ltdc-board.md`](stm32mp257-ltdc-board.md) independently:
  plane 42 upscales up to the mode size, plane 49 has no scaler, and nothing
  downscales. Primary plane 34 was not re-probed in this session.

### Scaled commits: quality (CRC) and underrun

Host 2026-11-27T22:46:00Z to 22:46:22Z (first run) and 22:47:02Z to
22:47:24Z (repeat with per-case counters). Weston stopped.
`crcprobe … scale`: plane 34 full-screen `#202020`. Plane 42 shows a
checkerboard of single pixels (R = 0xff or 0x00, G = B = 0), scaled to a
960x540 destination at 240,135. Its CRC is compared with five unscaled
960x540 references computed on the CPU and shown on the same plane:
nearest-neighbour (`⌊x·sw/dw⌋`), nearest with the h/v factors swapped,
nearest with pixel-centre sampling, bilinear centre-aligned and bilinear
corner-aligned. Because the image is R-only, the CRC tells pixel values
apart (Q2).

| Case | src → dst | Commit | Matching reference | `fifo_underrun_warning` while held (≈1.4 s) |
|---|---|---|---|---|
| h4-v4 | 240x135 → 960x540 | ok, 801 ms | none | +279 |
| h4-v2 | 240x270 → 960x540 | ok, 747 ms | none | +98 |
| h2-v4 | 480x135 → 960x540 | ok, 748 ms | none | +310 |
| h1.5-v1.5 | 640x360 → 960x540 | ok, 747 ms | none | +105 |
| h3-v3 | 320x180 → 960x540 | ok, 748 ms | none | +192 |
| h1-v1 | 960x540 → 960x540 | ok, 746 ms | all five (identical pictures) | +0 |

- Observed 2026-11-27: each scaled picture gave a stable CRC (the same
  about 0.5 s later, and the same in the repeat run). It **matched none of
  the nearest-neighbour or bilinear references**. The 1:1 case matched
  them all, which shows that the comparison works.
- Inferred: the LTDC scaler filters: it neither copies pixels nor uses a
  plain bilinear kernel with either phase convention. The kernel, phase and
  edge handling are not known. A picture (KVM) or a finer CRC reference
  search would be needed to judge quality. No tearing or rolling was
  seen, because nothing was seen.
- Observed 2026-11-27: the "nearest with swapped factors" reference did not
  match either. So the CRC run neither confirms nor rules out the
  cross-assigned h/v phase registers suspected in the design note §3.
- Observed 2026-11-27: `fifo_underrun_warning` grows **only while a scaled
  layer is on screen**, by about 70 to 220 per second with the factors
  above. The 1:1 case added none, and the unscaled references added 4 to 16
  in total, around the switches. `fifo_underrun_error` and
  `transfer_error` stayed 0 and `dmesg` printed nothing. The first run ended
  at `fifo_underrun_warning=1012`, the repeat at 1024.
- Observed 2026-11-27: every one of these commits took 746 to 801 ms to
  return, including the unscaled 1:1 case. They carried `ALLOW_MODESET` and
  the connector and CRTC properties. Per-frame commits without them (Q7)
  returned at the frame rate. The cause was not determined.
- Inferred: the warning counts FIFO-below-threshold events (threshold 128),
  not lost pixels. The source names the error counter "fifo underrun" and
  logs only errors (`observed/stm32mp257-source.md`, read from source). With
  no error, no picture loss is expected, but none could be looked at.

## Q7: underrun under load

Load came from the image's own tools only, on both cores: `memtester 768M
100 > /dev/null &`, `dd if=/dev/zero of=/dev/null bs=64M count=1000000 &` and
a `sh -c 'while :; do :; done' &` busy loop, started 3 s before the run and
killed after it. The load average reached 3.6 to 7.3. `stress-ng` is not
installed (observed 2026-11-27).

### Three planes with a scaled layer (crcprobe `load`)

None of the repository tools commits three planes or a scaled layer:
`drm-harness rotate` uses two planes, `scale_probe` only tests, and
`twoplane --scaling` only claims scaling in the caps. Its overlay stayed
384x540 → 384x540 (plan line below). So this part used the throwaway probe:
plane 34 XR24 1920x1080 (zpos 0), plane 42 XR24 at 1/F of the mode size,
scaled to 1920x1080 (zpos 1), and plane 49 AR24 1920x1080, alpha 0x40, on top
(zpos 2). K buffers per plane rotate and every frame is one blocking atomic
commit. A frame is late when the interval between commit returns is more
than 1.5 periods.

| Host time (UTC, 2026-11-27) | Command (`/tmp/crcprobe /dev/dri/card0 32 41 34 42 49 load K FRAMES F`) | Load | Result line | Counters after the last frame |
|---|---|---|---|---|
| 22:48:07Z–22:48:29Z | `load 4 600 4` (42 scaled 4x) | idle | `LOADRESULT frames=600 failed=0 interval_mean_ms=16.667 interval_max_ms=19.268 late=0` | `transfer_error=0 fifo_underrun_error=0 fifo_underrun_warning=8269` |
| 22:48:36Z–22:48:59Z | `load 4 600 1` (42 not scaled) | idle | `LOADRESULT frames=600 failed=0 interval_mean_ms=16.666 interval_max_ms=18.938 late=0` | `transfer_error=0 fifo_underrun_error=0 fifo_underrun_warning=0` |
| 22:49:04Z–22:49:51Z | `load 6 1800 4` (42 scaled 4x) | loaded | `LOADRESULT frames=1800 failed=0 interval_mean_ms=16.713 interval_max_ms=50.057 late=7` | `transfer_error=0 fifo_underrun_error=0 fifo_underrun_warning=10901` |
| 22:50:23Z–22:51:10Z | `load 4 1800 1` (42 not scaled) | loaded | `LOADRESULT frames=1800 failed=0 interval_mean_ms=16.749 interval_max_ms=116.648 late=3` | `transfer_error=0 fifo_underrun_error=0 fifo_underrun_warning=0` |

All four rows are observed 2026-11-27. Every run showed all counters at 0
after its first commit, and `dmesg` printed no new line. One more attempt,
`load 6 1800 1` at 22:50:01Z, stopped before its first commit (exit 101)
because the buffers did not fit: six buffers on three full-screen planes need
about 144 MiB, more than the 128 MiB CMA (inferred from the sizes; the panic
text was not captured).

### Maximum buffers (drm-harness) and twoplane, under the same load

```text
22:51:17Z  /tmp/drm-harness --buffers 16 --frames 1800 rotate
error scenario=rotate message="create dumb buffer: Out of memory (os error 12)"
VERDICT rotate FAIL create dumb buffer: Out of memory (os error 12)

22:51:39Z  /tmp/drm-harness --buffers 11 --frames 1800 rotate
rotate_result frames=1800 buffers_per_plane=11 presented=1800 dropped=0 missed_vblanks=13 latch_timeouts=0 releases=3598 order_violations=0 overwrite_avoided=0 assignment_changes=0 planes=34,42 refresh_mhz=60000 period_us=16666 warmup_frames=1 latency_count=1799 latency_min_us=649 latency_median_us=9525 latency_p99_us=17161 latency_max_us=21625 latency_mean_us=10179 interval_count=1798 interval_min_us=10715 interval_median_us=16667 interval_p99_us=23891 interval_max_us=38041 interval_mean_us=16788 validate_count=1800 validate_min_us=91 validate_median_us=178 validate_p99_us=4398 validate_max_us=8477 validate_mean_us=333 commit_count=1800 commit_min_us=68 commit_median_us=358 commit_p99_us=6254 commit_max_us=84824 commit_mean_us=2071
VERDICT rotate PASS 1800 frames, 11 buffers per plane, 3598 releases in order, 0 dropped, 13 missed vblanks

22:52:36Z  /tmp/twoplane /dev/dri/card0 --frames 1800 --buffers 3 --scaling 4,1
  layer 1 (overlay) -> plane 42 Overlay zpos 1 src 0,0 384x540 dst 960,270 384x540
RESULT frames=1800 committed=1800 latched=1800 released=3598 dropped=0 late=16 period_ms=16.667 latch_interval_mean_ms=16.676 latch_interval_max_ms=27.985 elapsed_s=30.086
VERDICT twoplane: PASS 1800 frames on two planes, 1800 latched, 3598 released, 16 late, driver stm mode 1920x1080@60.000Hz
```

All three runs are observed 2026-11-27. The counters stayed at 0
(`transfer_error=0 fifo_underrun_error=0 fifo_underrun_warning=0`) and
`dmesg` printed no new line. The harness drew 1920x1080 and 960x540 buffers
on planes 34 and 42. Values from 12 to 15 buffers per plane were not tried.

### Conclusions

- Observed 2026-11-27: under CPU and memory load, no run raised
  `fifo_underrun_error` or `transfer_error`, and the kernel logged no
  underrun. That includes three full-screen planes with a 4x-scaled middle
  plane and the AR24 top plane blending.
- Observed 2026-11-27: `fifo_underrun_warning` appears with a scaled layer
  and only then. Three unscaled full-screen planes gave 0, idle or loaded. In
  this run load did not raise the warning rate: 8269 in 600 frames idle,
  10901 in 1800 frames loaded.
- Inferred: the late frames under load (3 to 16 per 1800, the longest
  interval 116.6 ms) come from the harness competing for the CPU, not from
  the display. They happen with and without scaling, while the counters
  stay 0, and latch timeouts were 0.
- Observed 2026-11-27: with 128 MiB of CMA, the buffer count is limited by
  memory, not by the driver. `drm-harness --buffers 16` cannot allocate on
  this board with a full-screen and a quarter-screen plane; 11 per plane
  work.
- Inferred: TEST_ONLY cannot predict the warnings (no scaled commit was
  refused). A backend that wants headroom has to watch the counters after
  a commit.

## Q5: writing zpos

Session B, not run yet.

## Q6: rotation 180 and reflections

Session B, not run yet.

## Q9: NV12 on plane 42

Session B, not run yet.

## Q10: ST v6.18 source rows against the board's 6.6.129

Session B, not run yet (the companion diff is the ltdc-src-diff item).

## State after the session

Host 2026-11-27T22:53:40Z (observed 2026-11-27): `/tmp` on the board no
longer holds any file of this session. Plane 34 shows `allocated by = weston`
with the CRTC `enable=1 active=1`, and:

```text
$ systemctl is-active weston-graphical-session seatd-weston
active
active
```
