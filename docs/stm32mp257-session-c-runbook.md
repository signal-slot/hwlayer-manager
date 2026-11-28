# STM32MP257F-DK session C: runbook for the picture sessions (Q1, Q6)

A host-side procedure for the two KVM picture sessions of the STM32MP257F-DK
LTDC, open questions Q1 (pixel colours) and Q6 (transforms), plus the NV12
colours on plane 42 and the scaling of plane 34 that sessions A and B did not
probe. Background: [`stm32mp257-ltdc-session-2026-11-28.md`](stm32mp257-ltdc-session-2026-11-28.md)
(how sessions A and B ran), [`stm32mp257-ltdc-board.md`](stm32mp257-ltdc-board.md),
[`observed/stm32mp257-kvm.md`](observed/stm32mp257-kvm.md) and
[`design/stm32mp257-backend.md`](design/stm32mp257-backend.md) §7.

The aim is a short KVM lease: everything that does not need the picture
(build, copy, state capture, the commands, the EXPECT files) is prepared
before the lease starts, and the pictures are judged on the host afterwards.
These runs are **portability evidence only**. They count toward neither M1,
M2 nor M3.

Tags used here: read from source (the repository's own files and examples, as
named) and inferred (a prediction of this runbook). The tag for a real run is
used only in the session document that records it; this runbook records no
run.

## 0. Device rules

- ssh plus KVM only. The console and reset resources are away.
- No reboot, no flashing, no boot-media, U-Boot, DRM, udev or seat
  configuration change, and no module load. No debugfs write (sessions A and B
  wrote `crtc-0/crc/control`; this runbook does not need the CRC, and its
  debugfs reads are read only).
- Stop `weston-graphical-session` and `seatd-weston` only for a run, and
  confirm afterwards that both are active again (section 2).
- Binaries go to a directory under the board's `/tmp` and are removed
  afterwards (section 5).
- Never write a board address, host name, IP, serial or credential into a log
  or into the session document. Below, `<board>` stands for the ssh target and
  `<kvm-tool>` for the KVM's screenshot tool; fill them in at the terminal
  only.

## 1. On the host: build, EXPECT helper, variables

Build (read from source: `scripts/cross-aarch64-README.md`; the script prints
the executable paths on stdout and runs `file` on them):

```sh
git rev-parse --short HEAD                      # record it in the session document
scripts/cross-aarch64.sh --locked -p composition-drm --example twoplane --example plane_commit --example props
# the three paths printed, normally
#   target/aarch64-unknown-linux-musl/release/examples/twoplane
#   target/aarch64-unknown-linux-musl/release/examples/plane_commit
#   target/aarch64-unknown-linux-musl/release/examples/props
```

`props` only reads (its header says it never sets a property or commits and
needs no DRM master). It is built to read each plane's `zpos`,
`COLOR_ENCODING` and `COLOR_RANGE` values, which the NV12 and quadrant cases
below need recorded (read from source: `examples/props.rs`).

The `plane_commit` of main has a `pattern=quadrants` layer option and prints
`EXPECT` lines for it (read from source: the header of
`crates/composition-drm/examples/plane_commit.rs`). Section 4 uses it for the
orientation cases; the bar generator below stays for the `bars` default. Do
not copy the pattern description here. Build from main, or any commit that has
the option, and record the commit.

Session variables (a scratch directory on the host, not in the repository):

```sh
OUT=$(mktemp -d)                 # pictures, logs, EXPECT files of this session
BOARD=<board>                    # ssh target, typed at the terminal only
DIR=/tmp/mp257-session-c         # on the board
EX=target/aarch64-unknown-linux-musl/release/examples
date -u +%FT%TZ                  # host UTC: this is the clock for every record
```

EXPECT generator for `plane_commit`'s bars (the example paints eight vertical
bars into each buffer: white, yellow, cyan, green, magenta, red, blue,
black; read from source). Save it as `$OUT/bars-expect.py`; it needs only
Python 3. It writes a camera-check JSON for one layer's destination rectangle
on the 1920x1080 output, with the bar order reversed for `mirror-x`:

```sh
cat > "$OUT/bars-expect.py" <<'EOF'
import json, sys
# usage: bars-expect.py OUT.json X Y W H [mirror-x]   (output 1920x1080)
out = sys.argv[1]
x, y, w, h = (int(v) for v in sys.argv[2:6])
mirror = len(sys.argv) > 6 and sys.argv[6] == "mirror-x"
bars = [("white", 255, 255, 255), ("yellow", 255, 255, 0), ("cyan", 0, 255, 255),
        ("green", 0, 255, 0), ("magenta", 255, 0, 255), ("red", 255, 0, 0),
        ("blue", 0, 0, 255), ("black", 0, 0, 0)]
if mirror:
    bars.reverse()
bw = w / 8
regs = []
for i, (n, r, g, b) in enumerate(bars):
    cx, cy = x + (i + 0.5) * bw, y + h / 2
    regs.append({"name": "bar%d %s" % (i, n),
                 "rect": [round((cx - bw * 0.3) / 1920, 4), round((cy - h * 0.1) / 1080, 4),
                          round((cx + bw * 0.3) / 1920, 4), round((cy + h * 0.1) / 1080, 4)],
                 "expect": [r, g, b]})
doc = {"rotation": 0, "panel": {"corners": [[0, 0], [1, 0], [1, 1], [0, 1]], "unit": "frac"},
       "tolerance": 40, "regions": regs}
with open(out, "w") as f:
    json.dump(doc, f, indent=1)
EOF
```

camera-check needs Pillow for anything but 8-bit PNG and P6 PPM, and without
Pillow a full-HD PNG takes tens of seconds (`scripts/camera-check-README.md`).
Check `python3 -c 'import PIL'` before the lease, and save the KVM frames as
PNG. KVM pictures show the whole screen edge to edge, so every check uses
`--rotation 0 --panel full`.

## 2. Board: copy, state before, stop, restore

Copy (before the lease; nothing is stopped yet):

```sh
ssh $BOARD "mkdir -p $DIR"
scp $EX/twoplane $EX/plane_commit $EX/props $BOARD:$DIR/
ssh $BOARD "chmod +x $DIR/twoplane $DIR/plane_commit $DIR/props; ls -l $DIR"
```

State before (read only; keep the output in `$OUT/state-before.txt`). Record
the board clock against the host clock, because the board clock is about 10 h
behind and is not synchronised:

```sh
{ date -u +"host %FT%TZ"; ssh $BOARD 'date -u +"board %FT%TZ"'; date -u +"host %FT%TZ"; } | tee -a "$OUT/clock.txt"
ssh $BOARD 'uname -r; systemctl is-active weston-graphical-session seatd-weston;
  grep -E "transfer_error|fifo_underrun" /sys/kernel/debug/dri/0/state;
  grep -E "^plane\[|allocated by|fb=|crtc=" /sys/kernel/debug/dri/0/state | head -40;
  ls /dev/dri' | tee "$OUT/state-before.txt"
ssh $BOARD "$DIR/props /dev/dri/card0" > "$OUT/props-before.txt"   # read only; plane zpos, COLOR_ENCODING, COLOR_RANGE
```

Expected before: `active` twice, plane 34 `allocated by = weston`.

Stop (once per part; part 1 is Q1, part 2 is Q6; do not leave Weston stopped
between parts):

```sh
date -u +%FT%TZ | tee -a "$OUT/clock.txt"
ssh $BOARD 'systemctl stop weston-graphical-session seatd-weston'
```

Restore (run it after the last case of a part, and immediately if any step
fails or the ssh connection drops):

```sh
ssh $BOARD 'systemctl start seatd-weston weston-graphical-session; sleep 3;
  systemctl is-active weston-graphical-session seatd-weston'
date -u +%FT%TZ | tee -a "$OUT/clock.txt"
```

The restore is done only when that last command prints

```text
active
active
```

If it does not, repeat the `start` and look at `systemctl status` before
anything else. Do not reboot.

## 3. Part 1, Q1: twoplane pixel colours

Two runs, each `/tmp/mp257-session-c/twoplane /dev/dri/card0 ...`
(options read from `crates/composition-drm/examples/twoplane.rs`: the device
is the positional argument, then `--frames`, `--swap-z`, `--plane-alpha`). The log of
each run is needed: it holds the `EXPECT` lines. Order matters only for the
KVM lease: both are short.

| Run | Command | Pictures | Phases |
|---|---|---|---|
| Q1a | `twoplane /dev/dri/card0 --frames 600 --swap-z` | about 2 s and about 7 s after the start | normal, swapped |
| Q1b | `twoplane /dev/dri/card0 --frames 600 --plane-alpha 32768` | about 2 s (and about 7 s as a stability check) | normal |

Timing: 600 frames at 60 Hz last about 10 s, and the swap happens at frame
300, about 5 s after the first commit (session A: `elapsed_s=10.053`,
`swapped_at=300`). The first grab must come after the first commit, so aim at
2 s after the command was started, and the second at 7 s, which is after the
swap and before the end. If the KVM needs longer than that to deliver a
frame, repeat with `--frames 1200` (swap at 10 s, grabs at about 4 s and 14 s)
and say so in the session document.

Run Q1a. The command goes to the background so that the host can grab:

```sh
D=$OUT/q1a; mkdir -p $D
date -u +%FT%TZ | tee $D/start.txt
ssh $BOARD "$DIR/twoplane /dev/dri/card0 --frames 600 --swap-z" > $D/twoplane.log 2>&1 &
sleep 2
date -u +%FT%TZ | tee $D/grab-normal.txt     # then grab: <kvm-tool> -> $D/normal.png
# ...  KVM grab here, then, when the clock reads about start + 7 s:
date -u +%FT%TZ | tee $D/grab-swapped.txt    # then grab: <kvm-tool> -> $D/swapped.png
wait; date -u +%FT%TZ | tee $D/end.txt
tail -3 $D/twoplane.log                      # VERDICT twoplane: PASS ...
```

Run Q1b the same way into `$OUT/q1b`, with
`--frames 600 --plane-alpha 32768` and one or two grabs (`normal.png`,
`normal-7s.png`). Between the runs wait for `wait` to finish, so that only one
program holds DRM master. If twoplane says another process holds the master,
stop: do not stop other processes yourself.

Judge on the host, after restore (no lease needed). The EXPECT files come
from the run logs:

```sh
scripts/twoplane-expect.py $OUT/q1a/twoplane.log --out $OUT/q1a     # expect-normal.json, expect-swapped.json
scripts/twoplane-expect.py $OUT/q1b/twoplane.log --out $OUT/q1b     # expect-normal.json
python3 scripts/camera-check.py $OUT/q1a/normal.png  $OUT/q1a/expect-normal.json  --rotation 0 --panel full
python3 scripts/camera-check.py $OUT/q1a/swapped.png $OUT/q1a/expect-swapped.json --rotation 0 --panel full
python3 scripts/camera-check.py $OUT/q1b/normal.png  $OUT/q1b/expect-normal.json  --rotation 0 --panel full
python3 scripts/camera-check.py $OUT/q1b/normal-7s.png $OUT/q1b/expect-normal.json --rotation 0 --panel full
```

`twoplane-expect.py` takes the first positional argument as the log and
writes one `expect-<phase>.json` per phase that ran, at its defaults
(`--panel full`, `--rotation 0`, tolerance 40, region size 0.02). The
region names are twoplane's: `background`, `red block`, `blue block`,
`green block uncovered`, `overlay opaque over green`, `overlay opaque over
background`, `overlay translucent over green`, `overlay translucent over
background`. Each camera-check line gives the measured colour and its
distance: paste those lines into the session document with the picture file
names. Exit status 0 is PASS, 1 FAIL, 2 an input error. A `--tolerance 60`
rerun of `twoplane-expect.py` is useful only to see which failures are
marginal; the verdict is the one at 40.

### Predicted verdicts under straight alpha (inferred)

twoplane writes premultiplied buffers (its default `--blend` when the plane
offers premultiplied, which the probe reports for every LTDC plane), and
session A's output-CRC measurements say the hardware blends straight
(`out = Cs·α + Cd·(1 − α)`, session A, Q2). The buffer was written for one
model and blended by the other. `scripts/alpha-mode-expect.py` prints this
as its `mismatch hw=straight buffer=premultiplied` rows. Relevant lines of
`python3 scripts/alpha-mode-expect.py --plane-alpha 65535` (Q1a) and
`--plane-alpha 32768` (Q1b), run on the host for this runbook; the EXPECT
colour is the script's `premultiplied` line:

```text
plane-alpha 65535  under #00ff00 (green block): EXPECT #807f80
  mismatch hw=straight buffer=premultiplied: #407f40 dR=64 dG=0 dB=64
plane-alpha 65535  under #202020 (background):  EXPECT #901090
  mismatch hw=straight buffer=premultiplied: #501050 dR=64 dG=0 dB=64
plane-alpha 32768  under #00ff00 (green block): EXPECT #40bf40
  mismatch hw=straight buffer=premultiplied: #20bf20 dR=32 dG=0 dB=32
plane-alpha 32768  under #202020 (background):  EXPECT #581858
  mismatch hw=straight buffer=premultiplied: #381838 dR=32 dG=0 dB=32
```

Distances from the EXPECT colour (Euclidean RGB, as camera-check computes
them), with session A's truncating arithmetic where it differs:

| Run, phase | Region | EXPECT | Predicted picture (straight hardware) | Distance | Predicted verdict at tolerance 40 |
|---|---|---|---|---|---|
| Q1a normal | background, red block, blue block, green block uncovered | the block or background colour | the same | about 0 | PASS |
| Q1a normal | overlay opaque over green | `#ffff00` | `#ffff00` | 0 | PASS |
| Q1a normal | overlay opaque over background | `#ffff00` | `#ffff00` | 0 | PASS |
| Q1a normal | overlay translucent over green | `#807f80` | `#407f40` | 90.5 | **FAIL** |
| Q1a normal | overlay translucent over background | `#901090` | `#4f0f4f` (`#501050` rounded) | about 91 | **FAIL** |
| Q1a swapped | all 8 regions | background and blocks (overlay hidden under the opaque background) | the same | about 0 | PASS |
| Q1b normal | overlay opaque over green | `#80ff00` | `#80ff00` | 0 | PASS |
| Q1b normal | overlay opaque over background | `#909010` | `#8f8f0f` | 1.7 | PASS |
| Q1b normal | overlay translucent over green | `#40bf40` | `#20bf20` | 45.3 | **FAIL**, marginal |
| Q1b normal | overlay translucent over background | `#581858` | `#371737` (`#381838` rounded) | 45.3 to 46.7 | **FAIL**, marginal |

These match session A's table (the translucent regions fail, the opaque ones
pass) and are inferred from the Q2 CRC measurements. Read them as follows:

- All translucent regions of Q1a and Q1b measure **darker** than EXPECT
  (about half the stored colour) if Q2 is right; the opaque regions and the
  whole swapped phase need no blending and match.
- The Q1b distances of 45 to 47 sit close to tolerance 40, so a camera or
  capture colour shift of a few units can flip them. Record the measured
  colour as well as PASS/FAIL, and compare it with the predicted colour
  above rather than with the verdict alone.
- If a translucent region passes in Q1a (distance below 40 from
  `#807f80`/`#901090`), that contradicts Q2 and must be reported as such:
  say which colour was measured and keep the CRC evidence of session A next
  to it.
- Q1 is judged **per run, per phase**: three verdict lines: Q1a
  (normal, swapped) and Q1b, each with its picture file.

`twoplane --blend coverage` cannot test the straight-alpha profile: the
generic profile refuses it on this board (`NoPlane ... BLEND`, committed=0; see
[the KVM note](./observed/stm32mp257-kvm.md)), and twoplane's `--profile` takes only generic
or imx95.

## 4. Part 2, Q6: plane_commit transforms, NV12, scaling

All runs use `plane_commit` (SPEC syntax read from
`crates/composition-drm/examples/plane_commit.rs`):

```text
--layer plane=ID,format=XR24|AR24|NV12,src=WxH,dst=X,Y,WxH[,zpos=N][,rotation=R][,alpha=N]
```

Plane ids on this board: **34** primary, **42** and **49** overlays (session
A and B, `probe`). Layer specs (the same as session B, so that its CRC and
`TEST_ONLY` rows stay comparable):

| Name | Spec |
|---|---|
| `P` | `plane=34,format=XR24,src=1920x1080,dst=0,0,1920x1080` |
| `O` | `plane=42,format=XR24,src=640x480,dst=640,300,640x480` |
| `N` | `plane=42,format=NV12,src=640x480,dst=640,300,640x480` |
| `S` | `plane=49,format=XR24,src=320x240,dst=800,420,320x240` (inside `O`) |

Common prefix: `$DIR/plane_commit --device /dev/dri/card0 --hold-ms 10000`. The
10 s hold gives three grabs per case, about 3 s, 5 s and 7 s after the start;
more than one grab is the point for the transforms whose CRC changed from
frame to frame in session B (primary reflect-x and 180, overlay
reflect-x+reflect-y), because a picture defect may be intermittent. Rotations
join with `+`, and a name alone gets rotate-0 added (`reflect-x` is written
`0x11`), as in session B.

Start the part with a `--test-only-only` run of every case, inside the stop
window and before the first grab (it commits nothing and shows nothing; its
`TEST_ONLY` line says whether the case is possible, and a case that fails
there needs no picture):

```sh
ssh $BOARD "$DIR/plane_commit --device /dev/dri/card0 --test-only-only --layer 'SPEC' --layer 'SPEC'"
```

Cases (every `--layer` spec as in the table above, `rotation=` added):

| Case | Layers | Pictures | EXPECT (bars) |
|---|---|---|---|
| Q6-p180 | `P` with `,rotation=180`, plus `O` | 3 | primary strip mirrored; overlay unmirrored |
| Q6-pRX | `P` with `,rotation=reflect-x`, plus `O` | 3 | same as p180 (row-identical bars) |
| Q6-oRX | `P` plus `O` with `,rotation=reflect-x` | 3 | primary unmirrored; overlay mirrored |
| Q6-oRY | `P` plus `O` with `,rotation=reflect-y` | 2 | unmirrored (bars are equal in every row) |
| Q6-49RX | `P`, `O`, `S` with `,rotation=reflect-x` | 3 | `S` mirrored on top of `O` |
| Q6-49RY | `P`, `O`, `S` with `,rotation=reflect-y` | 2 | `S` unmirrored |
| NV12 | `P` plus `N` | 2 | the bars at 1:1, within the colour conversion's rounding |
| Scale34 | plane 34 `src=960x540` to `dst=0,0,1920x1080`, alone | 2 | the bars across the whole screen |

The exact command lines. `run_case` starts one `plane_commit` in the
background and logs host UTC; the grabs are KVM tool calls. The variables are
expanded on the host, and the specs contain no shell special characters:

```sh
PC="$DIR/plane_commit --device /dev/dri/card0 --hold-ms 10000"
P=plane=34,format=XR24,src=1920x1080,dst=0,0,1920x1080
O=plane=42,format=XR24,src=640x480,dst=640,300,640x480
N=plane=42,format=NV12,src=640x480,dst=640,300,640x480
S=plane=49,format=XR24,src=320x240,dst=800,420,320x240
run_case() {   # run_case NAME "LAYER ARGS"
  D=$OUT/$1; mkdir -p $D; date -u +%FT%TZ | tee $D/start.txt
  ssh $BOARD "$PC $2" > $D/plane_commit.log 2>&1 &
}
# after each: sleep 3; date -u +%FT%TZ; grab <kvm-tool> -> $D/g1.png; grab again at
# about 5 s and 7 s; then: wait; date -u +%FT%TZ | tee $D/end.txt; tail -4 $D/plane_commit.log

# primary
run_case q6-p180  "--layer $P,rotation=180 --layer $O"
run_case q6-pRX   "--layer $P,rotation=reflect-x --layer $O"
# overlay 42
run_case q6-oRX   "--layer $P --layer $O,rotation=reflect-x"
run_case q6-oRY   "--layer $P --layer $O,rotation=reflect-y"
# plane 49 (on top of 42 by default zpos)
run_case q6-49RX  "--layer $P --layer $O --layer $S,rotation=reflect-x"
run_case q6-49RY  "--layer $P --layer $O --layer $S,rotation=reflect-y"
# NV12 on plane 42 at 1:1
run_case nv12     "--layer $P --layer $N"
# plane 34 upscale 2x (src smaller than dst)
run_case scale34  "--layer plane=34,format=XR24,src=960x540,dst=0,0,1920x1080"
# optional: a second factor, and the plane 42 reference
run_case scale34b "--layer plane=34,format=XR24,src=1280x720,dst=0,0,1920x1080"
run_case scale42  "--layer $P --layer plane=42,format=XR24,src=320x240,dst=640,300,640x480"
```

Run one case at a time: `wait` for it before starting the next, so that only
one program holds DRM master. The same lines with `--test-only-only` added to
`PC` are the TEST_ONLY pass mentioned above.

Never use rotation 90 or 270 on any plane: they passed `TEST_ONLY` on the
primary in session B although the source and the board's device tree do not
support them, and committing them is unsafe (session B, Q6 conclusions). A
`TEST_ONLY EINVAL` is an answer, not a failure of the session: the plane
refuses it (the overlay refuses rotate-180). `plane_commit` still makes the
real commit and its recovery after an EINVAL; record the whole output and the
`CRTC active` line, because a refused commit leaves the CRTC off until the
RECOVER commit.

Orientation: the bars are the same in every row, so reflect-y and rotation 0
look the same, and reflect-x, 180 and reflect-x+reflect-y look the same
(session B). With the bars, **reflect-y can only be judged as "no defect,
unchanged", not as "flipped"**. The `pattern=quadrants` layer option of
`plane_commit` (four colours, one per quadrant: top left red, top right
green, bottom left blue, bottom right white; read from source) gives every
flip a different picture, reflect-y included. Run the quadrant cases of
section 4.1 as well, and add reflect-y to the verdicts as flipped or not.

EXPECT files for the bars (host, no lease), using `$OUT/bars-expect.py` of
section 1. The coordinates are the layer's `dst`. The strip `0 0 1920 216`
samples the primary away from the overlay (it covers y 300 to 780):

```sh
python3 $OUT/bars-expect.py $OUT/e-primary.json          0 0 1920 216              # unmirrored primary strip
python3 $OUT/bars-expect.py $OUT/e-primary-mirror.json   0 0 1920 216 mirror-x     # primary 180 / reflect-x
python3 $OUT/bars-expect.py $OUT/e-o.json              640 300 640 480              # overlay 42, unmirrored
python3 $OUT/bars-expect.py $OUT/e-o-mirror.json       640 300 640 480 mirror-x     # overlay 42, reflect-x
python3 $OUT/bars-expect.py $OUT/e-s.json              800 420 320 240              # plane 49, unmirrored
python3 $OUT/bars-expect.py $OUT/e-s-mirror.json       800 420 320 240 mirror-x     # plane 49, reflect-x
python3 $OUT/bars-expect.py $OUT/e-full.json             0 0 1920 1080              # scaled primary, whole screen
```

Check each grab against the file(s) of its row:

| Case | camera-check calls (each `python3 scripts/camera-check.py GRAB EXPECT --rotation 0 --panel full`) |
|---|---|
| Q6-p180, Q6-pRX | `e-primary-mirror.json`, then `e-o.json` |
| Q6-oRX | `e-primary.json`, then `e-o-mirror.json` |
| Q6-oRY | `e-primary.json`, then `e-o.json` |
| Q6-49RX | `e-primary.json`, `e-o.json` (outside `S` only; the check is coarse) and `e-s-mirror.json` |
| Q6-49RY | `e-primary.json`, `e-o.json` and `e-s.json` |
| NV12 | `e-primary.json`, then `e-o.json` at tolerance 40 (NV12 uses BT.601 limited range: judge the distances) |
| Scale34 | `e-full.json` |

Then **look at each grab** (open the PNG) and write down, besides the
verdict lines: tearing, jitter between grabs, a wrong column or line at the
edges, black or flickering output, colour fringes at bar edges after scaling
(the scaler filters, Q3). The primary's frame-to-frame changing CRC (Q6)
shows as a picture defect only if grabs differ or an edge defect appears; if
three grabs of primary reflect-x look the same and clean, say "no defect seen
in 3 grabs", not "stable".

The LTDC counters need no CRC: after each case read (read only)

```sh
ssh $BOARD 'grep -E "transfer_error|fifo_underrun" /sys/kernel/debug/dri/0/state; dmesg | tail -5'
```

and record `fifo_underrun_warning` for the scaled cases (session A: the
warning counts grow only while a scaled layer is shown).

NV12 colours: the source baseline is
[`stm32mp257-ltdc-yuv-source.md`](stm32mp257-ltdc-yuv-source.md) (read from
source: the driver honours `COLOR_ENCODING` and `COLOR_RANGE`, defaults BT.601
limited range; the rounding is inferred, expect +-1 to +-2). Take the two
property values from `$OUT/props-before.txt` (plane 42) and write them in
the session document next to the NV12 verdict; `plane_commit` sets neither.
The verdict is against the plain bar colours at tolerance 40, and the
measured distances are compared with that baseline.

### 4.1 Quadrant cases (orientation of every flip)

`pattern=quadrants` is added to a layer spec. Its `EXPECT` lines are written
by `plane_commit` itself into its log, in the format
`scripts/twoplane-expect.py` reads (phase `commit`), so no generator is needed:

```sh
Q=,pattern=quadrants
run_case q6-oRYq  "--layer plane=34,format=XR24,src=1920x1080,dst=0,0,1920x1080$Q --layer $O$Q,rotation=reflect-y"
# the same for rotation=reflect-x, rotation=reflect-x+reflect-y and 180 on the overlay,
# and for the primary alone (rotation=... on plane 34, no overlay)
scripts/twoplane-expect.py $OUT/q6-oRYq/plane_commit.log --out $OUT/q6-oRYq    # expect-commit.json
python3 scripts/camera-check.py $OUT/q6-oRYq/g1.png $OUT/q6-oRYq/expect-commit.json --rotation 0 --panel full
```

Caveats (read from source, the `plane_commit.rs` header):

- A sample is dropped when another layer with a higher, equal or unknown
  `zpos` covers it. Give each layer its `zpos=N` from `$OUT/props-before.txt`
  (a value the plane refuses is an errno, which is an answer) or use one
  quadrant layer on its own; otherwise a layer under or over another gets
  no samples.
- A layer with `alpha` below 65535 gets `skipped reason=plane-alpha`, and a
  `bars` layer gets `skipped reason=bars-pattern`; `twoplane-expect.py` then
  writes no `commit` file. For a quadrant check use full alpha, and no bars
  layer in the same request (the primary may be a quadrant layer too).
- The sample position after a rotation is computed with an assumed order
  (scale, turn counter-clockwise, reflect) that matters only for 90 and 270,
  which this runbook never commits. For 0, 180 and the reflections it is
  independent of that order.
- For NV12 with quadrants, the `rgb` of an `EXPECT` line is the source
  colour; the converted colour is judged with the baseline above.

### 4.2 Taking the KVM frame with `scripts/picture-hook.sh`

`scripts/picture-hook.sh` (see `scripts/picture-hook-README.md`) is the hook of
`board-scenarios.sh --hook`, and it also works by hand: `picture-hook.sh ROW
PHASE` takes one KVM frame with the command in `PICTURE_CAPTURE` (called as
`$PICTURE_CAPTURE OUTFILE`; the KVM lease provides it, so it is typed at the
terminal and written in no file) and judges it with `camera-check.py` against
`$PICTURE_EXPECT_DIR/ROW.json`, if that file exists. It writes
`$PICTURE_DIR/pictures.log` with one line per picture (`ROW PHASE UTC-TIME FILE
VERDICT`), and the camera-check output beside the PNG. The log time is host
UTC. The hook calls camera-check without options, so the EXPECT file must
carry its own panel: `twoplane-expect.py` and `bars-expect.py` write the full
screen, which is right for KVM frames.

For the grabs of a case, name the grab phases in `PICTURE_ON_PHASES`, so that
each call takes its picture at once, and call the hook at the clock times of
section 4:

```sh
export PICTURE_CAPTURE='<capture command from the lease>'   # typed at the terminal only
export PICTURE_DIR=$OUT/pictures PICTURE_EXPECT_DIR=$OUT/expect
export PICTURE_ON_PHASES="g1 g2 g3"
mkdir -p $PICTURE_EXPECT_DIR
cp $OUT/q6-oRYq/expect-commit.json $PICTURE_EXPECT_DIR/q6-oRYq.json
run_case q6-oRYq "..."                  # as above
sleep 3; sh scripts/picture-hook.sh q6-oRYq g1
sleep 2; sh scripts/picture-hook.sh q6-oRYq g2
sleep 2; sh scripts/picture-hook.sh q6-oRYq g3
wait; cat $PICTURE_DIR/pictures.log
```

Limits: one EXPECT file per ROW, so a case that is judged against two files
(primary and overlay) uses the hook only for the capture and is then judged
with the camera-check calls of section 4; or give each file its own ROW name
and take one picture per ROW. The picture counter is shared across rows, so
call the hook one case at a time. `hold-start`/`hold-end` (the delayed picture
with `PICTURE_DELAY`) are meant for `board-scenarios.sh`; by hand the grab
phases above are simpler. Exit status 1 is a failed capture or a FAIL verdict,
2 an environment error. If the capture fails, `pictures.log` holds
`capture failed (status N)`: that is the error text to record (section 5).

A helper `scripts/ltdc-picture-expect.py` may be added by a parallel item to
write EXPECT files for the transform, NV12 and plane-34 scaling cases. It is
not on main when this runbook is written, so nothing above uses it; use it
only after checking that it exists.

## 5. Recording rules

- **Times are host UTC** (`date -u +%FT%TZ` on the host), taken before and
  after each command and at each KVM grab. The board clock is about 10 h
  behind (session A: 9 h 53 min) and is only written down as an offset, from
  `$OUT/clock.txt`. A board `DATE` line in `plane_commit` or twoplane output
  is the board's clock; do not use it as a time.
- Record the commit of the build (`git rev-parse --short HEAD`), the kernel
  (`uname -r`, expected `6.6.129`) and the KVM tool outcome of the first grab
  (success or the error text, with the date).
- If the KVM cannot deliver a picture (the earlier sessions got HTTP 404 from
  its relay), write down the error text and the host UTC time, stop the
  picture items and do restore and clean-up. Do not retry the run without a
  picture: its log answers nothing that sessions A and B did not.
- Keep the raw material of the session under `$OUT` (logs, PNGs, JSON); the
  session document quotes the camera-check lines and names the files, and
  the pictures are not committed unless the repository already carries
  pictures of that kind.
- Binaries only under `/tmp/mp257-session-c` on the board. Remove them at the
  end, and show a count of 0 left:

```sh
ssh $BOARD "rm -rf $DIR; find /tmp -maxdepth 1 -name 'mp257-session-c*' | wc -l"   # must print 0
```

- Never put the board address, host name, IP, serial or any credential into
  `$OUT` files that are pasted into the session document; the checks
  `scripts/check-no-board-secrets.sh` and `scripts/check-no-farm-ids.sh` must
  pass on it.

### State after (template)

Copy into the session document, fill in after the restore and the clean-up:

```text
State after the session
Host <UTC time> (tag with the session date): Weston started again (host <UTC time>),
plane 34 shows `allocated by = weston`, CRTC `active=1`, counters
`transfer_error=<n> fifo_underrun_error=<n> fifo_underrun_warning=<n>`,
`dmesg` new lines: <none | the lines>, /tmp entries of this session left: 0.

$ systemctl is-active weston-graphical-session seatd-weston
active
active
```

If a unit was not active, say so, say what was done to start it (`systemctl
start`, nothing else) and give the final output.

## 6. Skeleton of the session document

Save as `docs/stm32mp257-ltdc-session-c-<date>.md`, `<date>` being the real
(host UTC) date of the session. Do not create it from this runbook; the
picture sessions do.

````markdown
# STM32MP257F-DK (LTDC): board session C, <date>

Session C of the open questions in
[`design/stm32mp257-backend.md`](design/stm32mp257-backend.md) §7: Q1 pixel
colours, Q6 transforms, NV12 colours on plane 42 and the scaling of plane 34,
from KVM pictures. Sessions A and B:
[`stm32mp257-ltdc-session-2026-11-28.md`](stm32mp257-ltdc-session-2026-11-28.md).
Procedure: [`stm32mp257-session-c-runbook.md`](stm32mp257-session-c-runbook.md).
Portability evidence only: counts toward neither M1, M2 nor M3.

## Tags, clock and conditions

- observed <date>: seen on the board or in a picture in this session; times
  are UTC from the host clock, <start> to <end>. The board clock was <offset>
  behind the host (<board DATE line> at host <time>).
- inferred: a conclusion of this document.
- Build commit <short hash>; kernel <uname -r>; KVM tool <name>, first grab at
  host <time>: <ok | error text>.
- Device rules kept: ssh plus KVM only; no reboot, flashing, boot-media,
  U-Boot, DRM, udev or seat configuration change, no module load, no debugfs
  write. Weston stopped at host <time>, started again at host <time>.

## Q1: pixel colours

VERDICT Q1: <PASS | FAIL | NOT RUN> <one line: do the translucent regions
fail under straight alpha as predicted?>

| Run, phase | Picture (host UTC) | Region | EXPECT | Measured | Distance | camera-check | Predicted |
|---|---|---|---|---|---|---|---|
| Q1a normal | <file>, <time> | overlay translucent over green | `#807f80` | <rgb> | <d> | <PASS/FAIL> | FAIL (`#407f40`) |
| ... | | | | | | | |

camera-check lines (pasted): <...>

## Q6: transforms

VERDICT Q6 primary rotate-180: <PASS | FAIL | NOT RUN> <picture result>
VERDICT Q6 primary reflect-x: <...> <changing CRC seen as a defect? yes/no>
VERDICT Q6 overlay 42 reflect-x: <...>
VERDICT Q6 overlay 42 reflect-y: <...> <flip not visible with these bars | flipped>
VERDICT Q6 plane 49 reflect-x: <...>
VERDICT Q6 plane 49 reflect-y: <...>

| Case | TEST_ONLY | COMMIT | CRTC active | RECOVER | RESTORE | Grabs (host UTC) | camera-check | Defects seen |
|---|---|---|---|---|---|---|---|---|
| Q6-p180 | | | | | | | | |

## NV12 on plane 42

VERDICT NV12 colours: <PASS | FAIL | NOT RUN> <worst distance, which bar>

## Scaling of plane 34

VERDICT plane 34 scaling: <PASS | FAIL | NOT RUN> <TEST_ONLY and COMMIT result,
picture, counters>

## Summary

| Question | Answer | Tag |
|---|---|---|
| Q1 | | tag with the session date |
| Q6 | | tag with the session date |

## State after the session

<the template of runbook section 5>
````
