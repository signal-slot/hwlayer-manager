# Weston on the i.MX95 image and DRM leases (WP5-01)

Question: does the Weston that NXP and Toradex ship for the Verdin iMX95
(Toradex Reference Multimedia Image 7.8.0, NXP BSP kernel 6.6.157-7.8.0) have
any DRM lease support? Answered from source only; no board was touched.

Tags: **read from source** (URL + commit or tag + read date) or **inferred**.
All reads were made on 2026-11-28 from shallow fetches over ssh to GitHub.

## 1. Answer

- NXP's `weston-imx` has **no lease code** at any of the three revisions
  searched (rows S3 to S5). That covers the one the NXP recipe selects.
- Whether the Toradex 7.8.0 image uses that `weston-imx` is **not established**:
  Toradex's own layers could not be read (section 5). The match is **inferred**.
- AGL does carry lease patches for Weston, but as a lease **client**
  (`--drm-lease=<name>` taking an fd from `drm-lease-manager`), against Weston
  13.0.x. That is not a Wayland lease protocol and not Weston as lease
  source. It is not in the NXP or Toradex image (row S7).
- So for the Verdin image, "Weston as lease source" remains a feature to write
  (inferred). The standalone-lease route through `drm-lease-manager` needs
  no Weston change (as `docs/wp5-gap-analysis.md` already says).

## 2. Layers and recipe

| # | Fact | Tag | Source |
|---|------|-----|--------|
| L1 | NXP `meta-imx` release `rel_imx_6.6.52_2.2.2` is commit `e1ec1a20d573e3913f4ad90fa36546ed2bc87715` (2025-12-03). Branch `scarthgap-6.6.52-2.2.2` is `474d2b45015f73e6b0d77729e2028b00e008f730`; the `meta-imx-bsp/recipes-graphics` and `meta-imx-bsp/conf` trees are identical between the two. | read from source | https://github.com/nxp-imx/meta-imx |
| L2 | `meta-imx-bsp/conf/machine/include/imx-base-extend.inc:107`: `PREFERRED_VERSION_weston:imx-nxp-bsp ??= "12.0.5.imx"` (i.MX 6/7 stay on 10.0.5.imx). | read from source | same repo, tag `rel_imx_6.6.52_2.2.2` |
| L3 | Recipe `meta-imx-bsp/recipes-graphics/wayland/weston_12.0.5.imx.bb`: `WESTON_SRC ?= "git://github.com/nxp-imx/weston-imx.git;protocol=https"`, `SRCBRANCH = "weston-imx-12.0.5"`, `SRCREV = "a29bbd0f65e68b9beda47a94144bd9b2801c42cb"`, `DEFAULT_PREFERENCE = "-1"`. | read from source | same repo and tag |
| L4 | Patches in that recipe's `SRC_URI`: only `0001-Revert-protocol-no-found-wayland-scanner-with-Yocto-.patch`. Not lease related. `PACKAGECONFIG` adds `imxg2d` for `imxgpu2d` and `mx93-nxp-bsp` machines; removes `wayland x11`. Backend `kms` (DRM) is enabled by default. | read from source | same recipe |
| L5 | `weston-init.bbappend` only edits the systemd unit (log file, `User=root`) and the commented `gbm-format` line in `weston.ini`. No lease option. | read from source | same repo and tag |
| L6 | `meta-imx-bsp/conf/machine/imx95-19x19-verdin.conf` exists (`#@DESCRIPTION: Machine configuration for Toradex i.MX 95 19x19 Verdin board`), requiring `imx95-evk.inc`. | read from source | same repo and tag |
| L7 | Which `meta-imx` revision, which `meta-freescale` revision and which Toradex bbappends the 7.8.0 manifest pins, and which weston recipe is selected in the Toradex image. | **not read** | Toradex's manifest and layers are not on GitHub; `git.toradex.com` refused automated fetches (bot protection) and https git was not usable from the worker |
| L8 | Toradex's i.MX95 stack is NXP-BSP based (kernel `6.6.157-7.8.0` is a Toradex-stable-tracking 6.6; NXP userspace `scarthgap-6.6.52-2.2.x`), so the image likely builds `weston_12.0.5.imx`. | inferred | L1 to L6; the version is not confirmed by any 7.8.0 file or by `weston --version` on the board |

## 3. Lease search in weston-imx

Search: `git grep -n -iwE 'lease|leases|leasing|leased|drm-lease|drm_lease'`
over the whole tree, `*.po` excluded; plus `drm.?lease`,
`DRM_IOCTL_MODE_CREATE_LEASE`, `drmModeCreateLease`, `wp_drm_lease` (no
`-w`), and a file-name search for `lease`. (A bare `lease` substring search
matches `release` everywhere and is useless.)

| # | Revision | Weston base | Result | Tag | Source |
|---|----------|-------------|--------|-----|--------|
| S3 | `weston-imx-12.0.5` = `a29bbd0f65e68b9beda47a94144bd9b2801c42cb` (2025-09-23), the revision the NXP recipe selects (L3) | `meson.build` `version: '12.0.5'` (`SCR.txt` still says `12.0.4.imx`, release location `lf-6.6.23-2.0.0`; stale text) | **0 hits** for every pattern; no file with `lease` in its name; `libweston/backend-drm/` has `drm-gbm.c drm-virtual.c drm.c fb.c kms-color.c kms.c modes.c state-helpers.c state-propose.c`, no lease file; no `drm-lease` protocol XML | read from source | https://github.com/nxp-imx/weston-imx, branch `weston-imx-12.0.5` |
| S4 | `weston-imx-14.0.2` = `be99fd1adad7e77c8c31926b09520ade5cdaca35` | `14.0.2` | 0 hits (word search) | read from source | same repo, branch `weston-imx-14.0.2` |
| S5 | `weston-imx-15.0.1` = `bfd5abc894b10bc0941024ca5b9254d95a0628ae` (2026-09-01) | `15.0.1` | 0 hits (word search) | read from source | same repo, branch `weston-imx-15.0.1` |
| S6 | `non-desktop` handling exists (`modes.c`, `kms.c`, `drm-internal.h`), as in upstream; it marks a head as non-desktop and is not leasing. | | | read from source | S3 revision |

S4 and S5 are context (newer NXP releases). The extra patterns (`drm.?lease`,
ioctl and protocol names, file names) were run on S3 only.

## 4. AGL

| # | Fact | Tag | Source |
|---|------|-----|--------|
| S7 | `meta-agl`, tag `unagi_20.92.0` = `8611917335f6d1f5b29ea495c8f4c2c914183e8a` (2026-02-13; the tag object is `df524c37ec617b6df7839e2b7678591bf649b52c`). Layer `meta-agl-drm-lease` has `recipes-graphics/weston/weston_13.0.%.bbappend`, enabled by the `agl-drm-lease` AGL feature, applying to **upstream-based Weston 13.0.x** from OE-core (not `weston-imx`): `0001-backend-drm-Add-method-to-import-DRM-fd.patch`, `0002-Add-DRM-lease-support.patch`, `0001-compositor-do-not-request-repaint-in-output_enable.patch`; sets `PACKAGECONFIG += drm-lease` (`-Ddrm-lease=true`, depends on `drm-lease-manager`). | read from source | https://github.com/automotive-grade-linux/meta-agl (mirror of the AGL Gerrit repo), tag `unagi_20.92.0` |
| S8 | What the patches do: a `--drm-lease=<name>` option on the `weston` binary; `compositor/drm-lease.c` calls `dlm_get_lease()` from `libdlmclient` and hands the fd to the DRM backend through a new "import DRM fd" method; the DRM backend then bypasses the launcher. Patch headers say `Upstream-Status: Pending`, authors Collabora and IGEL. Weston is a lease **client** of `drm-lease-manager`; no `wp_drm_lease_device_v1`. | read from source | patch `0002-Add-DRM-lease-support.patch` in the same layer and tag (first 170 lines read; the rest of `drm.c` hunks of patch 0001 not read) |
| S9 | The README says the layer is tested on Renesas R-Car Gen3; other Linux DRM platforms "may work". | read from source | `meta-agl-drm-lease/README.md`, same tag |
| S10 | `agl-compositor_git.bb` also has a `drm-lease` PACKAGECONFIG (`-Ddrm-lease=true`, `drm-lease-manager`); agl-compositor is not Weston and was not read further. | read from source | `meta-agl-core/recipes-graphics/wayland/agl-compositor_git.bb:27`, same tag |
| S11 | The AGL patches would apply to the i.MX95 image's Weston 12.0.5 `weston-imx`. | **not tested**; patches were written against 13.0.x. Applying them is a code change, not a configuration. |

## 5. Not read

- Toradex manifest, `meta-toradex-nxp`, `meta-toradex-bsp-common` and any
  Toradex weston bbappend for 7.8.0 (L7). The Toradex release pages were not
  fetched either; a search found a 7.8.0 download listing for Verdin iMX95
  dated 2026-11-16 and no 7.8.0 release notes, so even the 7.8.0 layer
  revisions are unconfirmed.
- Any file on the board; `weston --version` on the image. `docs/m1-imx95-test-plan.md` §8 already requires the report to state it.
- Whether an `imx-nxp-bsp` Toradex build uses `weston_12.0.5.imx` or a different Weston (for example the OE-core one).
- AGL's `meta-agl-devel` (`drm-lease-manager` container recipes referenced by the README) and any non-`unagi_20.92.0` AGL tag.
- Weston upstream releases (`weston-imx` revisions not listed above, e.g. `weston-imx-12.0.4`, `14.0.1`).
- The `drm-lease-manager` and `libdlmclient` sources; licences are WP5-05.

## 6. Consequence for the open issues (not edited here)

- WP5-01: the NXP side is answered ("no lease code in weston-imx 12.0.5,
  14.0.2, 15.0.1"); the Toradex side stays open until one `weston --version`
  from the board or the Toradex layers are read.
- The AGL difference: a client-side patch set exists, for 13.0.x, `Pending`
  upstream. The wp5 gap analysis said AGL's Weston "may carry lease
  support"; this is the form it takes.
- Condition 6 of `docs/m1-imx95-test-plan.md` (lease without a Weston fork) is
  unaffected: the lease holder there is `drm-lease-manager`, not Weston.
