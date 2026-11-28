# AGL: which Weston ships, and how its DRM lease support is configured

Scope: WP5-01 (which Weston AGL ships). Everything here is **read from
source (2026-11-28)** from shallow clones of AGL's Gerrit repositories. Nothing
was built or run. The content of the lease patches is **not** read here; that
is `docs/agl-drm-lease-patches-source.md`.

## 1. Sources and pins

Read from source (2026-11-28): the current release branch is taken to be
`vimba` (the last release-named branch on Gerrit; **inferred**, `master` and
`next` also exist) and the earlier one is `unagi`. Shallow clones
(`--depth 1`) of the branch tips on that date:

| Repository (Gerrit `AGL/...`, https://gerrit.automotivelinux.org/gerrit/AGL/) | branch | commit |
|---|---|---|
| `meta-agl` | `vimba` | `cc2a18eb5641d8a32c83117fefe39555bfcd8d60` |
| `meta-agl` | `unagi` | `d0f97441e3fb909155e0547a5cbf8ae913db6175` |
| `meta-agl-devel` | `vimba` | `fe4a3df591c52fcea37797c1e860f694e02090ef` |
| `meta-agl-devel` | `unagi` | `73da625d195fbe3758a480e51d328bebdd57b5e9` |

Read from source (2026-11-28): `meta-agl-drm-lease` is a directory inside
`meta-agl` (`meta-agl/meta-agl-drm-lease/`), not a separate repository. The
`drm-lease-manager` source itself (Gerrit `src/drm-lease-manager`) was not
cloned (the repository URL answered 403 to `git ls-remote`); only the recipe
that pins it was read.

The `unagi` branch tip above is later than the `unagi_20.92.0` tag used in
`docs/weston-imx-lease.md` S7 (`8611917335f6d1f5b29ea495c8f4c2c914183e8a`);
the two were not diffed.

## 2. Weston version

| Item | `unagi` | `vimba` | Tag |
|---|---|---|---|
| Layer series compat of `meta-agl-drm-lease` (`conf/layer.conf`) | `scarthgap` | `wrynose` | read from source (2026-11-28) |
| Weston appended by `meta-agl-drm-lease` | `recipes-graphics/weston/weston_13.0.%.bbappend` → Weston 13.0.x | `recipes-graphics/weston/weston_15.0.%.bbappend` → Weston 15.0.x | read from source (2026-11-28) |
| Weston appended by `meta-agl-core` | `weston_13.0.%.bbappend`, `weston_13.0_aglcore.inc` | `weston_15.%.bbappend`, `weston_15_aglcore.inc` | read from source (2026-11-28) |
| Weston recipe file itself | not in these layers | not in these layers | read from source (2026-11-28) |

- Read from source (2026-11-28): AGL carries **no** Weston recipe in
  `meta-agl` or `meta-agl-devel`; it only appends to the Weston recipe of the
  underlying OE-core / meta-openembedded. The version is therefore fixed only
  as far as the `%` wildcard says (`13.0.x`, `15.0.x`). The exact point
  release comes from the OE-core revision of the AGL manifest, which was
  **not read** here.
- Read from source (2026-11-28): the only `PREFERRED_VERSION_weston` found
  in either branch is `13.0.1`, in `unagi`
  `meta-agl-bsp/conf/include/agl_imx8mq-evk.inc` for `mx8mq-generic-bsp`
  (an i.MX8MQ machine, not i.MX95). No such pin was found in `vimba`.
- Read from source (2026-11-28): no `weston-imx` (NXP) recipe or bbappend was
  found in `meta-agl` or `meta-agl-devel` on either branch (file-name search
  for `weston*`). AGL's Weston is the upstream-based recipe.
- Read from source (2026-11-28): `meta-agl-core` and `meta-agl-devel`
  (`meta-agl-rdp`, `meta-agl-ic-container`) have further Weston bbappends
  (`weston-init`, `weston-ini-conf`, packaging splits). They were not read for
  lease content; the `weston.ini` files in
  `meta-agl-ic-container/recipes-graphics/wayland/weston-ini-conf-drm-lease-test/`
  exist on both branches.

## 3. DRM-lease options for Weston

Read from source (2026-11-28), identical on `unagi` and `vimba` apart from
the file name (`weston_13.0.%` vs `weston_15.0.%`):

- `weston_<ver>.%.bbappend` requires `weston_agl-drm-lease.inc` only when
  `AGL_FEATURES` contains `agl-drm-lease`.
- `weston_agl-drm-lease.inc` defines
  `PACKAGECONFIG[drm-lease] = "-Ddrm-lease=true,-Ddrm-lease=false,drm-lease-manager"`
  and does `PACKAGECONFIG:append = " drm-lease"`. So the meson option is
  `-Ddrm-lease=true` and the build-time dependency is `drm-lease-manager`.
  The option `drm-lease` is added by AGL's patches; it is **not** upstream
  Weston's lease support (see `docs/weston-release-read.md` and
  `docs/weston-imx-lease.md`).
- `SRC_URI:append` lists three patch files, all under
  `meta-agl-drm-lease/recipes-graphics/weston/weston/`:
  - `0001-backend-drm-Add-method-to-import-DRM-fd.patch`
  - `0002-Add-DRM-lease-support.patch`
  - `0001-compositor-do-not-request-repaint-in-output_enable.patch`

  Their contents are not read here: see
  `docs/agl-drm-lease-patches-source.md` (the file names are the same on both
  branches; whether their content differs was not compared).
- Without the `agl-drm-lease` feature none of the above applies; the
  default Weston PACKAGECONFIG is not touched by this layer.
- Read from source (2026-11-28): `agl-compositor_git.bb` (`meta-agl-core`,
  agl-compositor `AGL_COMPOSITOR_VERSION = "0.0.24"` on `vimba`) has
  `PACKAGECONFIG[drm-lease] = "-Ddrm-lease=true,-Ddrm-lease=false,drm-lease-manager"`;
  `meta-agl-drm-lease/dynamic-layers/meta-agl-core/.../agl-compositor_agl-drm-lease.inc`
  appends `drm-lease` to it. agl-compositor is not Weston and was not read
  further.
- Read from source (2026-11-28): Weston here is a lease **client** (the README
  starts it with `weston --drm-lease=<lease name>`, for example
  `card0-HDMI-A-1`). The layer also patches `kmscube`, `psplash`, and (under
  `dynamic-layers/meta-qt6`) `qtbase` for lease clients.

## 4. drm-lease-manager

Read from source (2026-11-28), `recipes-graphics/drm-lease-manager/drm-lease-manager_git.bb`:

| Item | `unagi` | `vimba` |
|---|---|---|
| Source | Gerrit `src/drm-lease-manager`, `protocol=https`, `branch=${AGL_BRANCH}` | same |
| `SRCREV` | `e8d3ddade5e1c29593a78d73eb1314fe349de434` | `e9cd6b5c03f707709310b0ab91e9b9da41c296e5` |
| `PV` | `0.2+git${SRCPV}` | `0.2+git${SRCPV}` |
| Recipe `LICENSE` | `Apache-2.0` (`LICENSE` md5 `3b83ef96387f14655fc854ddc3c6bd57`) | same |
| Build | `inherit meson pkgconfig systemd ptest`; `DEPENDS = "libdrm libcheck fff tomlc99"` | same |
| meson options | `-Denable-systemd=true/false` (PACKAGECONFIG `systemd`, default from `DISTRO_FEATURES`), `-Denable-tests=true` when `PTEST_ENABLED = 1` | same |
| Packages | `libdlmclient` split out of the main package | same |
| Service | `drm-lease-manager.service`; runtime config from `drm-lease-manager-init` (`/etc/xdg/drm-lease-manager/drm-lease-manager.ini`, recipe `LICENSE = "MIT"`, `PACKAGE_ARCH = ${MACHINE_ARCH}`) | same |

The version is `0.2` plus the git revision; no tag was read. The recipe
`LICENSE` is what the Yocto recipe declares; the licence files of the
`drm-lease-manager` source are not covered here (WP5-05 records those in
`docs/third-party-licences.md`).

## 5. How `meta-agl-drm-lease` is enabled

Read from source (2026-11-28), both branches:

1. **Feature template**: `meta-agl/templates/feature/agl-drm-lease/` with
   `50_bblayers.conf.inc` (`BBLAYERS =+ "${METADIR}/meta-agl/meta-agl-drm-lease"`)
   and `50_local.conf.inc` (`require conf/include/agl-drm-lease.inc`). It is
   selected by naming the feature `agl-drm-lease` when running
   `aglsetup.sh` (README, "Setup").
2. **`AGL_FEATURES`**: `meta-agl-drm-lease/conf/include/agl-drm-lease.inc`
   contains `AGL_FEATURES += "agl-drm-lease"`. The bbappends above test this
   variable. It is `AGL_FEATURES`, not `DISTRO_FEATURES`; no
   `DISTRO_FEATURES` entry for leases was found in the files read.
3. **Layer**: `BBFILE_PRIORITY_agl-drm-lease = "80"`, `LAYERDEPENDS = "core"`,
   dynamic layers for `rcar-gen3`, `sparrow-hawk`, `qt6-layer`, `raspberrypi`
   and `aglcore`. README: tested on Renesas R-Car Gen3 (as in
   `docs/weston-imx-lease.md` S9).
4. **Container demos in `meta-agl-devel`**: `meta-agl-ic-container`
   (both branches) has `drm-lease-manager-init.bbappend` (the `r8a7796`
   machine changes the connector in the ini from `HDMI-A-2` to `VGA-1`),
   `packagegroup-drm-lease-client-support`, `agl-host-image-drm-lease-test`
   and guest images `guest-image-drm-lease-test-{ivi,cluster}`. Its feature
   template `agl-ic-container` lists `agl-drm-lease` in `included.dep` on
   `vimba` (`agl-drm-lease agl-ic agl-pipewire agl-selinux`).

## 6. libliftoff

Read from source (2026-11-28): a search for `liftoff` in `*.bb`,
`*.bbappend`, `*.inc` and `*.conf` of `meta-agl` on both branches found
nothing. Whether libliftoff is pulled in transitively through OE-core's
Weston recipe was **not read** (that recipe is outside these layers). The
libliftoff licence and version inside AGL is another item
(`agl-liftoff-licence-read`).

## 7. What this does not settle

- **Not read**: the exact Weston point release (OE-core revision of the AGL
  manifest), the content of the patches, the `drm-lease-manager` source tree.
- **Not tested**: whether the AGL patches apply to NXP's `weston-imx`
  (`docs/weston-imx-lease.md` S11).
- **Not measured**: nothing here was built or run.
- `vimba` as the current release is inferred from the branch list.
