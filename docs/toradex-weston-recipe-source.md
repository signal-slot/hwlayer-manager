# Toradex BSP layers: what the sources say about Weston on the Verdin iMX95

Scope: WP5-01 asks which Weston the Verdin iMX95 image ships and whether it
has DRM lease support. This note is the read-from-source side. It does **not**
replace the board observation (`weston --version` and the installed package on
the running image, see [weston-imx-lease.md](weston-imx-lease.md)); nothing
here was observed on a board. No board address or credential appears here.

**Result in one line:** the two Toradex layers contain no Weston recipe and
no lease setting; they only append to `weston-init`. The Weston recipe that
the i.MX BSP selects is in NXP `meta-imx` (section 3): weston-imx 12.0.5, with
no lease-related meson option set. Whether the Toradex image actually builds
from that recipe (and not oe-core's) is inferred, not read; the board
observation settles it.

## 1. Sources and pins

| Layer | Branch | Commit | How resolved |
|---|---|---|---|
| `meta-toradex-nxp`, <https://git.toradex.com/meta-toradex-nxp.git> | `scarthgap-7.x.y` | `b0b9f0fd9d3e4854e3d764bcd2b383b337318f3a` | `git ls-remote --heads` and `git clone --depth 1`, read from source (2026-11-28) |
| `meta-toradex-bsp-common`, <https://git.toradex.com/meta-toradex-bsp-common.git> | `scarthgap-7.x.y` | `8f13083c771e7187b61eb83e5ae216498e55864a` | `git clone --depth 1`, read from source (2026-11-28) |

Branch choice (inferred): [imx95-verdin-bsp-src.md](imx95-verdin-bsp-src.md)
names BSP 7.8.0 and the kernel branch `toradex_6.6-2.2.x-imx` but no layer
branch. `scarthgap-7.x.y` is Toradex's scarthgap 7.x release branch and
`meta-toradex-nxp`'s `layer.conf` declares `LAYERSERIES_COMPAT` for
`nanbield scarthgap` (read from source, 2026-11-28). Both SHAs are branch heads
on the read date, not the revisions that built the image on the board.

## 2. Findings (read from source, 2026-11-28)

- `meta-toradex-nxp` at `b0b9f0fd`: no file with "weston" in its name and no
  occurrence of "weston" in any file. No `lease` match. The only `PACKAGECONFIG`
  lines are for `imx-sm-toradex` and `linux-firmware`. Read from source
  (2026-11-28).
- `meta-toradex-nxp` `layer.conf` depends on `core`, `freescale-layer`,
  `freescale-3rdparty`, `toradex-bsp-common-layer`. Read from source
  (2026-11-28). So Weston for this machine comes from one of those: oe-core
  (`core`) or `meta-freescale`, plus NXP's `meta-imx` if the BSP adds it.
  Which one wins is inferred, not read.
- `conf/machine/verdin-imx95.conf` sets `PREFERRED_PROVIDER` for the kernel,
  U-Boot script, `imx-oei` and `imx-system-manager` and `PREFERRED_VERSION` for
  `u-boot-toradex`. It sets **no** `PREFERRED_VERSION` or provider for Weston,
  libdrm or wayland-protocols. Read from source (2026-11-28).
- `meta-toradex-bsp-common` at `8f13083c`:
  `recipes-graphics/wayland/weston-init.bbappend` is the only Weston-related
  recipe. It appends to `weston-init`, not to the `weston` recipe. Read from
  source (2026-11-28). It:
  - adds `file://weston.sh`, which exports `WAYLAND_DISPLAY=/run/wayland-0` when
    unset;
  - appends `no-idle-timeout` to `weston-init`'s `PACKAGECONFIG` for `k3` and for
    `upstream` + `tdx-distro`, and `use-pixman` for several i.MX6ULL/i.MX7/i.MX93
    machines. **Not** for `verdin-imx95`;
  - under `tdx-distro` with systemd, rewrites `weston.service` and
    `weston.socket` to run as root, and adds `ExecStartPre` lines that
    `udevadm wait` for `/dev/dri/card0` (and `card1` if a GPU by-path exists).
- `PACKAGECONFIG` of `weston-init` is not the `weston` compositor's meson
  option set, so none of the above enables or disables DRM lease support.
- No `drm-lease`, `wayland-protocols`, `libdrm` or Weston `PV`/`SRCREV` string
  appears in either layer (grep over both trees, read from source,
  2026-11-28).

## 3. NXP meta-imx Weston recipe (read from source, 2026-11-28)

Layer: `meta-imx`, <https://github.com/nxp-imx/meta-imx> branch
`scarthgap-6.6.52-2.2.2` at `474d2b45015f73e6b0d77729e2028b00e008f730`
(shallow clone; branch chosen to match kernel `6.6-2.2.x`, inferred).

- `meta-imx-bsp/conf/machine/include/imx-base-extend.inc` sets
  `PREFERRED_VERSION_weston:imx-nxp-bsp ??= "12.0.5.imx"` (i.MX6/7 use 10.0.5.imx).
  Read from source (2026-11-28).
- Recipe `meta-imx-bsp/recipes-graphics/wayland/weston_12.0.5.imx.bb`:
  `COMPATIBLE_MACHINE = "(imx-nxp-bsp)"`, `DEFAULT_PREFERENCE = "-1"`,
  `SRC_URI` from `git://github.com/nxp-imx/weston-imx.git;protocol=https`,
  `SRCBRANCH = "weston-imx-12.0.5"`,
  `SRCREV = "a29bbd0f65e68b9beda47a94144bd9b2801c42cb"`, plus one local patch
  (`0001-Revert-protocol-no-found-wayland-scanner-with-Yocto-.patch`). The
  OE-core part records upstream hash `92a460b55e7290ec0006365219189761f7226f7c`.
  Read from source (2026-11-28).
- Default `PACKAGECONFIG`: `kms wayland egl clients` (if DISTRO_FEATURES has
  wayland), `xwayland` (x11+wayland), `systemd`, `launcher-libseat`,
  `image-jpeg`, `screenshare`, `shell-desktop`, `shell-fullscreen`,
  `shell-ivi`, `shell-kiosk`; i.MX removes `wayland x11`; adds `imxg2d` only
  for `imxgpu2d` and `mx93-nxp-bsp`. `kms` maps to `-Dbackend-drm=true`.
  Extra meson: `-Dpipewire=false`. Read from source (2026-11-28).
- No `lease`/`drm-lease` PACKAGECONFIG entry or meson option exists in the
  recipe. Read from source (2026-11-28).
- Weston's lease protocol (`wp_drm_lease_device_v1`) is part of the DRM backend
  upstream rather than a build option; whether weston-imx 12.0.5's backend
  carries and enables it was **not read** here (the weston-imx source was not
  fetched). Inferred only. See [weston-imx-lease.md](weston-imx-lease.md).
- Whether Verdin iMX95 uses the `imx-nxp-bsp` override (and so this recipe):
  `verdin-imx95.conf` requires `conf/machine/include/imx-base.inc` and
  `MACHINEOVERRIDES =. "tdx:mx95:"`, and has `use-nxp-bsp` kernel-devicetree
  lines. Image-level selection is inferred, not read.

## 4. Torizon OS

Not read. `meta-toradex-torizon` and `meta-toradex-distro` were not cloned, so
the sources do not say here what the Torizon image uses. Not measured, not read.

## 5. Not read (needed to finish WP5-01 from source)

- The weston-imx source at `a29bbd0f65e68b9beda47a94144bd9b2801c42cb` (lease
  code and options in its DRM backend); see the weston-imx item.
- `meta-freescale` and oe-core `weston_*.bb` at the scarthgap branch.
- The Toradex manifest that pins layer revisions for BSP 7.8.0.
- `meta-toradex-distro` / `meta-toradex-torizon` for Torizon.

## 6. Commands

| Command | Result |
|---|---|
| `git ls-remote --heads https://git.toradex.com/meta-toradex-nxp.git` | read: `scarthgap-7.x.y` = `b0b9f0fd9d3e4854e3d764bcd2b383b337318f3a` |
| `git clone --depth 1 --branch scarthgap-7.x.y` of both layers | succeeded; trees searched with `find -iname '*weston*'` and `grep -rIn -i` for weston, lease, wayland, drm, PACKAGECONFIG |
| `git ls-remote --heads https://github.com/nxp-imx/meta-imx.git` | read: branch list only; `scarthgap-6.6.52-2.2.2` = `474d2b45015f73e6b0d77729e2028b00e008f730` |
| `git clone --depth 1 --branch scarthgap-6.6.52-2.2.2` of `meta-imx` | succeeded; read `weston_12.0.5.imx.bb` and `imx-base-extend.inc` |
