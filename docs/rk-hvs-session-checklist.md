# Board-session checklist: RK3588 VOP2 and Raspberry Pi HVS

Written 2026-11-28. **No board was touched and no hardware result is claimed.** This is a command checklist for three pending sessions: `t6-commit-session` (RK3588 on the NanoPC-T6: Q4, Q9), `rpi5-hvs6-session` (BCM2712 HVS6 on the RPi5: Q1, Q5, Q6, Q10) and `cm4-hvs5-session` (BCM2711 HVS5 on the reTerminal CM4: Q1, Q6). The questions are those of §7 of `docs/rk3588-backend-design.md` and `docs/hvs-backend-design.md`.

Tags used here: every statement about what a tool does or prints is **read from source**, with the file named. The "how to tag" lines are **inferred** instructions for the result file: a result is written **observed** only with its date and host UTC (the tool's own `DATE` line) after the board ran it. Nothing below is observed.

## Tools (read from source)

Paths are under `crates/composition-drm/examples/` unless stated. All of them are read from source in the file named.

| Tool | Flags (all of them) | File |
|---|---|---|
| `probe` | `[--raw] [/dev/dri/cardN]`. Read-only; no DRM master needed. | `probe.rs` |
| `props` | `/dev/dri/cardN`. Read-only; prints every property, `possible_crtcs`, decoded `IN_FORMATS`. | `props.rs` |
| `kms_sweep` | `<mode> [/dev/dri/cardN]`, mode = `combos`, `modifiers` or `replace`; `--device PATH`, `--connector ID`; combos: `--max N`, `--sets S`, `--scale-h F`, `--scale-v F`, `--unscaled-primary`; modifiers: `--modifier M`, `--plane ID`, `--format CODE`, `--size WxH`; replace: `--wait-ms N`. Needs DRM master. | `kms_sweep.rs` |
| `scale_probe` | `[/dev/dri/cardN] [--device PATH] [--plane ID] [--dst WxH] [--resolution N] [--max-factor N] [--connector ID]`. TEST_ONLY only; needs DRM master. | `scale_probe.rs` |
| `twoplane` | `/dev/dri/cardN [--frames N] [--buffers 2\|3] [--output N] [--swap-z] [--unsupported] [--blend MODE] [--plane-alpha A] [--scaling UP,DOWN] [--profile generic\|imx95]`. Real commits; needs DRM master. | `twoplane.rs` |
| `plane_commit` | `[--device PATH] [--connector ID] --layer SPEC... [--hold-ms N] [--test-only-only]`; SPEC = `plane=ID,format=XR24\|AR24\|NV12,src=WxH,dst=X,Y,WxH[,zpos=N][,rotation=R][,alpha=N][,pattern=P]`. | `plane_commit.rs` |
| `pending_probe` | see the example's `--help` once it lands (item `pending-probe-tool`). | not yet |
| `mode_test` | see the example's `--help` once it lands (item `mode-test-tool`). | not yet |

Other read-from-source facts:

- `kms_sweep replace` makes real commits: a blocking modeset puts a grey frame on the primary plane, then a nonblocking flip to a second frame and at once a second nonblocking flip back, before the first event. Its output lines are `SETUP commit=ok events=N`, `REPLACE first=<ok|errno> second=<ok|errno> events=N` and `VERDICT kms_sweep: DONE replace`. It leaves the output dark until the next client commits (`kms_sweep.rs`, section "replace").
- `kms_sweep combos` and `modifiers` and `scale_probe` only check `ATOMIC_TEST_ONLY | ALLOW_MODESET`; nothing is committed (`kms_sweep.rs`, `scale_probe.rs`).
- `kms_sweep combos` prints `COMBO k=.. set=.. planes=.. result=ok|errno` and `COMBO_MAX accepted_k=N` (`kms_sweep.rs`).
- `scale_probe` prints `SCALE ... test_only=accepted|rejected errno=N` and `RESULT plane=ID kind=.. max_upscale_h/v/both=.. downscale=.. min_downscale=..`; a maximum ending in `+` means the ladder ended without a rejection (`scale_probe.rs`).
- `twoplane` prints `RESULT frames= committed= latched= released= dropped= late=` and `VERDICT twoplane: PASS|FAIL` (`twoplane.rs`); `plane_commit` prints `TEST_ONLY`, `COMMIT`, `STATE`, `EXPECT phase=commit "<name>" x= y= fx= fy= rgb=#rrggbb` and `VERDICT plane_commit: DONE` (`plane_commit.rs`).
- Every session tool except `probe`, `props` prints `DATE`, `KERNEL`, `DEVICE`, `DRIVER`, `MODE`, `PLANE` lines first; `DATE` is the host UTC to quote (`scale_probe.rs`, `twoplane.rs`).
- `scripts/display-guard.sh [--wait N] detect | stop STATEFILE | restore STATEFILE`. `stop` prints `DISPLAY-GUARD stopped <unit>` or `DISPLAY-GUARD none`; `restore` prints `DISPLAY-GUARD restored <unit> active` per unit and exits 0 only if all are active (`scripts/display-guard.sh`).
- `scripts/twoplane-expect.py [FILE|-] --out DIR [--size F] [--tolerance N] [--rotation N] [--panel full|corners] [--corners x,y,...]` writes `DIR/expect-<phase>.json` per phase (`scripts/twoplane-expect.py`).
- `python3 scripts/camera-check.py PICTURE EXPECT.json [--compare PICTURE2] [--rotation N] [--json] [--panel full|corners]`; exit 0 PASS, 1 FAIL, 2 input error. For a KVM (HDMI capture) picture use `--panel full --rotation 0` (`scripts/camera-check-README.md`).

## Build (read from source)

The repository's documented aarch64 recipe is `scripts/cross-aarch64.sh`, which builds static `aarch64-unknown-linux-musl` binaries and ends with `file` on each (`scripts/cross-aarch64-README.md`, `docs/ci.md`). It forwards its arguments to `cargo build --release`, so an example is built with:

```sh
scripts/cross-aarch64.sh --locked -p composition-drm --example probe
scripts/cross-aarch64.sh --locked -p composition-drm --example props
scripts/cross-aarch64.sh --locked -p composition-drm --example kms_sweep
scripts/cross-aarch64.sh --locked -p composition-drm --example scale_probe
scripts/cross-aarch64.sh --locked -p composition-drm --example twoplane
scripts/cross-aarch64.sh --locked -p composition-drm --example plane_commit
```

The repository documents the `probe` line and the no-argument recipe (probe plus harness); the other `--example` lines apply the same documented "any cargo build args" form and are inferred, not documented one by one. The script prints each executable's path on stdout. The BCM2711 and BCM2712 boards are aarch64 too, so the same binaries serve all three boards (inferred; the board architecture is stated in `docs/observed/bcm2711-*.md` and `docs/observed/bcm2712*.md`).

## Hygiene for every session

1. Copy binaries to the board's `/tmp` only, and `rm` them (and any state file, output file) before leaving. Check with `ls /tmp` that nothing of yours remains.
2. Stop a display manager only through `scripts/display-guard.sh stop /tmp/<statefile>` and, at the end, run `restore` on the same file and quote its `DISPLAY-GUARD restored <unit> active` line. If `restore` exits non-zero, say so in the report.
3. Never change DRM, udev, seat or boot configuration; load no module; do not reboot or flash. Use the master or kernel-lease source only.
4. Write no board address, host name, IP, serial number, UART path or credential into any file, commit or report. Name boards by model only. Run `scripts/check-no-board-secrets.sh` and `scripts/check-no-farm-ids.sh` on every result file.
5. Keep the session short: the boards are needed next by plan #461. Leave each board as found.
6. Read-only first (`probe --raw`, `props`), then TEST_ONLY tools, and real commits last.

Every session starts the same way (the guard line is skipped for a tool that needs no DRM master):

```sh
# on the host: build, then copy to the board's /tmp (the copy command is the session's own)
# on the board:
uname -r
/tmp/probe --raw /dev/dri/card0        # confirm which card is the display card first (ls /dev/dri)
/tmp/props /dev/dri/card0
```

The card number is not assumed: on boards with more than one DRM device, `probe` prints the driver, so pick the card whose driver is the display driver (inferred).

Tag for every result block: record as observed with the date and host UTC time from the tool's DATE line, then the kernel (`uname -r`), the tool name and the commit it was built at. A source expectation that was not measured stays `read from source` or `inferred`. Run `scripts/check-doc-tags.sh <file>`.

## t6-commit-session: NanoPC-T6 (RK3588 VOP2), Q4 and Q9

### Q4: are eight simultaneous windows possible?

```sh
# TEST_ONLY: nothing is committed (kms_sweep.rs)
scripts/display-guard.sh stop /tmp/dg.state      # only if probe shows DRM master is busy
/tmp/kms_sweep combos /dev/dri/card0 --sets 32
/tmp/kms_sweep combos /dev/dri/card0 --max 8 --sets 64   # optional: more subsets per k
```

Pick the connected output with `--connector ID` if more than one is connected (ids from `props`).

- Answers Q4: `COMBO_MAX accepted_k=N`. The highest k with `result=ok` for `set=0` is the largest simultaneous plane set on that CRTC with full-screen buffers. Read the `COMBO k=.. set=..` lines for k = 2..8: `result=EINVAL` or `ENOSPC` or `ERANGE` at a given k says where it fails and which plane ids were in the set.
- A pass at k=8 for `set=0` only shows one set works; the other sets show whether it depends on which planes are chosen, and so on the window-to-video-port assignment.
- Run it once per connected output if the board has two; the planes listed per CRTC differ.
- Tag: record as observed with the run date and host UTC time (YYYY-MM-DD HH:MMZ), kernel, `COMBO_MAX` value and the count of `ok` vs rejected sets per k. A TEST_ONLY result is what the kernel accepts, not a picture (`kms_sweep.rs`).

### Q9: overlay flip and Latched semantics

Use the tools that exist now. A flip/latch count comes from `twoplane`; the replace-before-flip case from `kms_sweep replace`. `pending_probe`: see the example's `--help` once it lands.

```sh
scripts/display-guard.sh stop /tmp/dg.state
/tmp/twoplane /dev/dri/card0 --frames 300 --buffers 3 | tee /tmp/tp.out
/tmp/twoplane /dev/dri/card0 --frames 120 --swap-z | tee /tmp/tp-swap.out
/tmp/kms_sweep replace /dev/dri/card0 --wait-ms 1000
# optional: one fixed picture for the KVM, with an overlay window (SPEC fields: plane_commit.rs)
/tmp/plane_commit --device /dev/dri/card0 --hold-ms 5000 \
  --layer plane=<primary id>,format=XR24,src=1920x1080,dst=0,0,1920x1080,pattern=quadrants \
  --layer plane=<overlay id>,format=AR24,src=640x360,dst=100,80,640x360,zpos=<n>,pattern=quadrants | tee /tmp/pc.out
scripts/display-guard.sh restore /tmp/dg.state
```

Plane ids and the primary size come from `props` / the tool's `PLANE` and `MODE` lines; they are filled in on the board, not guessed here. `twoplane` needs a mutable-`zpos` pair for `--swap-z` and prints `SWAPZ skipped` otherwise (`twoplane.rs`).

- Answers Q9, flip count: `RESULT frames=300 committed=300 latched=300 released=…` with `dropped=0`; and `VERDICT twoplane: PASS`. `latched` below `committed` or `dropped` above 0 is the finding. `late=` counts latches later than 1.5 periods (`twoplane.rs`).
- Answers Q9, pending replacement: `REPLACE first=ok second=<ok|EBUSY|...> events=<1|2>`. `second=ok` with `events=2` means the second commit was accepted and both flips completed (their order is not shown by this line); `second=EBUSY` with `events=1` means a pending commit is not replaceable (`kms_sweep.rs`). `pending_probe`: see the example's `--help` once it lands.
- KVM picture judged by `camera-check` (the KVM resource of the board; capture command is the session's own):

```sh
python3 scripts/twoplane-expect.py /tmp/pc.out --out /tmp/expect --panel full
python3 scripts/camera-check.py <picture.png> /tmp/expect/expect-commit.json --panel full --rotation 0
```

  `VERDICT: PASS` with the overlay quadrants in the right places confirms the overlay is visible with the zpos stated. Use `pattern=quadrants` at full alpha only: a `bars` pattern or `alpha` below 65535 makes `twoplane-expect.py` drop the phase (`plane_commit.rs`). Keep `pc.out`, the picture and the verdict out of git if they name anything board-specific; quote the verdict line.
- Tag: record as observed with the run date and host UTC time (YYYY-MM-DD HH:MMZ), kernel, the `RESULT` / `REPLACE` line verbatim, the `camera-check` verdict line.
- After the run: `rm /tmp/<binaries> /tmp/*.out /tmp/dg.state`, and quote the `DISPLAY-GUARD restored ... active` line.

## rpi5-hvs6-session: Raspberry Pi 5 (BCM2712 HVS6), Q1, Q5, Q6, Q10

### Q1: scaler limits (ScaleLimits)

```sh
scripts/display-guard.sh stop /tmp/dg.state
/tmp/scale_probe /dev/dri/card<N> --max-factor 64
/tmp/scale_probe /dev/dri/card<N> --max-factor 64 --dst 1920x1080   # fixed dst; adjust to the mode
scripts/display-guard.sh restore /tmp/dg.state
```

- Answers Q1: per plane `RESULT plane=ID kind=Overlay max_upscale_h=… max_upscale_v=… max_upscale_both=… downscale=accepted|rejected|partial min_downscale=…`. A maximum with a trailing `+` (`64.0000+`) means no limit was found up to `--max-factor`: raise it, or record "at least that". The `SCALE` lines show each factor and errno (`scale_probe.rs`).
- Expected shape from the earlier 2x up / 1/2 down trials is only a starting point; do not state a limit from them.
- Tag: record as observed with the run date and host UTC time (YYYY-MM-DD HH:MMZ), kernel, `RESULT` lines for every plane, and the `--dst` used. Downscale is only tried at 0.5, 0.75 and 0.9 (`scale_probe.rs`), so "min_downscale" is the smallest of those, not the true limit. State that.

### Q5: CRTC and plane arithmetic; are the extra per-CRTC planes cursors?

```sh
/tmp/probe --raw /dev/dri/card<N>
/tmp/props /dev/dri/card<N>
```

- Answers Q5: from `props`, the plane list with each plane's type (Primary, Overlay, Cursor), its `possible_crtcs` mask, and the CRTC list. Count planes per CRTC by type: the extra planes are cursors if their `type` is Cursor. `probe` lists what the backend derives (cursor planes are excluded there), so a difference between the two counts is the cursor count (`probe.rs`, `props.rs`).
- Then `kms_sweep combos <device>` (TEST_ONLY) gives the largest accepted plane set per output (`COMBO_MAX`), which checks the arithmetic against what the kernel accepts.
- Tag: record as observed with the run date and host UTC time (YYYY-MM-DD HH:MMZ), kernel, plane count per type and per CRTC as printed.

### Q6: replace_pending

```sh
scripts/display-guard.sh stop /tmp/dg.state
/tmp/kms_sweep replace /dev/dri/card<N> --wait-ms 1000
scripts/display-guard.sh restore /tmp/dg.state
```

`pending_probe`: see the example's `--help` once it lands.

- Answers Q6: `REPLACE first=ok second=<ok|EBUSY|errno> events=<N>`. `second=ok` and `events=2`: a not-yet-latched commit can be followed by another one; `second=EBUSY`: it cannot. `events` fewer than the accepted commits means an event was merged or lost (`kms_sweep.rs`).
- Tag: record as observed with the run date and host UTC time (YYYY-MM-DD HH:MMZ), kernel, the `SETUP` and `REPLACE` lines verbatim. Repeat three times and say whether the answer is stable; one run is one sample.

### Q10: BCM2712 HDMI limits

```sh
/tmp/probe --raw /dev/dri/card<N>
/tmp/props /dev/dri/card<N>
```

`mode_test`: see the example's `--help` once it lands.

- Answers Q10, what is listed: the connector's mode list and the CRTC/connector properties in `props`. The largest mode listed is the kernel's offer for the attached display, not the SoC's limit. A limit is shown only by a TEST_ONLY modeset of a mode that is rejected, which needs `mode_test` (not yet available); until then Q10 stays open with the listed modes as the observed context (inferred).
- Tag: record as observed with the run date and host UTC time (YYYY-MM-DD HH:MMZ), kernel, the connected display's mode list by `WxH@Hz` only (no EDID serial or manufacturer-unique id in the file).

### KVM

The RPi5's KVM may take one picture of a `plane_commit` hold exactly as in the t6 section; it is optional for Q1, Q5, Q6, Q10, which are TEST_ONLY or event counts.

## cm4-hvs5-session: reTerminal CM4 (BCM2711 HVS5), Q1 and Q6

This board has no camera (plan item text), so no picture checks.

```sh
scripts/display-guard.sh stop /tmp/dg.state
/tmp/scale_probe /dev/dri/card<N> --max-factor 64
/tmp/kms_sweep replace /dev/dri/card<N> --wait-ms 1000
scripts/display-guard.sh restore /tmp/dg.state
```

The panel on this board may be a DSI panel; `scale_probe` and `kms_sweep` use the first connected connector unless `--connector ID` names one (`scale_probe.rs`, `kms_sweep.rs`).

- Q1 answer and tag: as for the RPi5 Q1 above.
- Q6 answer and tag: as for the RPi5 Q6 above. `pending_probe`: see the example's `--help` once it lands.
- Both HVS generations share one probing method, so the two result files can be compared line by line (inferred).

## After every session

```sh
rm -f /tmp/probe /tmp/props /tmp/kms_sweep /tmp/scale_probe /tmp/twoplane /tmp/plane_commit /tmp/dg.state /tmp/*.out
ls /tmp
```

Then, on the host: write the result to the dated session file named by the plan, run `scripts/check-doc-tags.sh`, `scripts/check-doc-links.sh`, `scripts/check-no-board-secrets.sh` and `scripts/check-no-farm-ids.sh` on it, and state in the report which board, which kernel, and that the display manager was shown active again.
