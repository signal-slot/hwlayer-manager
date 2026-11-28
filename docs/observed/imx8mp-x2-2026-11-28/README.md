# Armadillo-X2 (i.MX8M Plus) DRM probe, 2026-11-28

Raw dumps from the x2-probe item of plan 463 (SoW §5.3, evidence only; counts
toward none of M1, M2 or M3). Written up in
[`../../imx8mp-x2-board.md`](../../imx8mp-x2-board.md).

- Date: 2026-11-28 UTC, from the host clock. Every file below is observed
  2026-11-28.
- Board: Atmark Techno Armadillo-X2, i.MX8MP revision 1.1, Armadillo Base OS
  `3.24-at.3` (Alpine 3.24.1), kernel `6.12.105-0-at`.
- Binaries: `scripts/cross-aarch64.sh -p composition-drm --example probe
  --example props` at commit 798fd28 (aarch64-unknown-linux-musl, static).
  They were copied to `/tmp` on the board, run as root over ssh and removed
  afterwards.
- No process held `/dev/dri/*` and no display manager or compositor was
  running, so nothing was stopped. Both examples only read state. They never
  commit.
- Every file went through `scripts/scrub-kms-dump.py`, which made 0
  replacements in each one. The files were then read by hand. The kernel
  command line was left out of `soc.txt` because it names the UART device.
  The current-state values (FB_ID 44, MODE_ID 45, CRTC_ID 37) are the fbdev
  console's (`imx-drmdrmfb`) state at capture time, not driver defaults.

| File | Command on the board | What it shows |
| --- | --- | --- |
| [`soc.txt`](soc.txt) | a read-only shell script: `/proc/device-tree/{compatible,model}`, `/sys/devices/soc0/*`, `uname -r/-v/-m`, `/etc/os-release`, `ls -l /dev/dri`, `/sys/class/drm/*` driver links and connector status, the display device-tree nodes, `rc-status -a`, DRM fd holders, the parents of the `imx-lcdifv3-crtc.*` devices, `dmesg` display lines, `/etc/atmark-release`, `podman ps -a` | SoC identity, BSP, cards and drivers, LCDIF-to-CRTC mapping, display-manager state |
| [`probe-card1.txt`](probe-card1.txt) | `/tmp/probe --raw /dev/dri/card1` | what composition-drm derives, plus the raw KMS view of the display card (`imx-drm`) |
| [`props-card1.txt`](props-card1.txt) | `/tmp/props /dev/dri/card1` | every KMS property with its id, flags and value |
| [`probe-card0.txt`](probe-card0.txt) | `/tmp/probe --raw /dev/dri/card0` | the GPU card (`galcore`, DRM name `vivante`): fails with `Not supported (os error 95)`, exit 1 |
| [`props-card0.txt`](props-card0.txt) | `/tmp/props /dev/dri/card0` | the same failure, exit 1 |
