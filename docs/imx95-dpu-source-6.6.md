# i.MX95 display driver, kernel 6.6.157-7.8.0: source reading

**Status: BLOCKED. The driver source was not read.** Desk research only; nothing here is observed on a board. Every fetch of file content (git clone, curl, WebFetch) was refused by the sandbox on 2026-11-28. Only `git ls-remote` worked. So the table below has no source finding, and no row is claimed. The rows are the ones of the i.MX95 Linux table in `docs/platform-capabilities.md`, which was read from lf-6.18.y (commit 6e694eda40aa). They still apply only to that tree.

## Which kernel (identified from the recipe history; SRCREV of the 6.6.157 state not yet read)

The board reports `6.6.157-7.8.0`. Read on 2026-11-28 from `git.toradex.com/cgit` (cgit `tree/`, `plain/` and `log/` pages):

- Layer: `https://git.toradex.com/meta-toradex-nxp.git`, branch `scarthgap-7.x.y`, file `recipes-kernel/linux/linux-toradex_6.6-2.2.x.bb`.
- Kernel repository and branch named by the recipe: `git://git.toradex.com/linux-toradex.git`, `SRCBRANCH = "toradex_6.6-2.2.x-imx"`. Its NXP base is the `lf-6.6.52-2.2.x` line (NXP tag `lf-6.6.52-2.2.2` has `SUBLEVEL = 52`, read from its `Makefile`).
- The recipe at the layer head has `LINUX_VERSION = "6.6.161"` and `SRCREV_machine = 8ad2b7ff6b5666b25b1b43819c83b9ae3930b8df`. That is **newer than the board** and is not the kernel read here.
- The recipe history has commit `962f3217b3e4cd49afb89075e374e90da7e35aef` (2026-09-25, "linux-toradex: update to stable v6.6.157"). The next recipe change is `d1e2818c702c` (2026-11-04, "Update to v6.6.161"), so the recipe state at `962f3217b3e4` is the 6.6.157 kernel of the board. The `SRCREV_machine` in that state is **not yet read**: the fetch was stopped by the host's Anubis bot challenge, which was not bypassed.

Retry on 2026-11-28 (second attempt): `git fetch` of the recipe history into the dependency cache, `gh api` and WebFetch of the raw recipe were each refused with "requires approval" / permission not granted, so `SRCREV_machine` at `962f3217b3e4` is still unread. Only `git ls-remote` is allowed. It gives the current heads: `meta-toradex-nxp` `scarthgap-7.x.y` = `b0b9f0fd9d3e`, `linux-toradex` `toradex_6.6-2.2.x-imx` = `8ad2b7ff6b56` (this is 6.6.161, newer than the board). Neither is the board's kernel commit.

To finish once network content fetch is allowed: `git fetch --filter=blob:none --depth 60 https://git.toradex.com/meta-toradex-nxp.git scarthgap-7.x.y`, then `git show 962f3217b3e4:recipes-kernel/linux/linux-toradex_6.6-2.2.x.bb` for `SRCREV_machine`; then a blobless fetch of `linux-toradex` `toradex_6.6-2.2.x-imx` and a sparse checkout of `drivers/gpu/drm/imx/` and `arch/arm64/boot/dts/freescale/imx95*` and `imx95-verdin*`.

Still to do (blocked): read `SRCREV_machine` at `962f3217b3e4` (`plain/...bb?id=962f3217b3e4...`); confirm that `linux-toradex` has `SUBLEVEL = 157` at it; read `drivers/gpu/drm/imx/` and the Verdin iMX95 / Mallow / LVDS device trees there. That BSP 7.8.0 equals this recipe state is inferred (no release tag or release notes were seen).

## Table (same rows as the i.MX95 Linux table)

"differs" is unknown for every row because the 6.6 source was not read.

| Item | 6.6.157-7.8.0 finding | Basis | Differs from lf-6.18.y reading |
| --- | --- | --- | --- |
| CRTCs | not read | none | unknown |
| Planes (count, types) | not read | none | unknown |
| Plane → CRTC (possible_crtcs) | not read | none | unknown |
| Hardware behind planes (fetch units, LayerBlends) | not read | none | unknown |
| Formats | not read | none | unknown |
| Modifiers | not read | none | unknown |
| YUV | not read | none | unknown |
| Scaling (limits, shared scalers) | not read | none | unknown |
| Source size | not read | none | unknown |
| Plane alpha | not read | none | unknown |
| Blend modes | not read | none | unknown |
| zpos | not read | none | unknown |
| Rotation | not read | none | unknown |
| Off-screen destination | not read | none | unknown |
| Active CRTC with no plane | not read | none | unknown |
| Cross-CRTC migration | not read | none | unknown |
| Bandwidth check | not read | none | unknown |
| Clocks and power | not read | none | unknown |
| LVDS bridge and panel timing (DT) | not read | none | unknown |
| Verdin iMX95 / Mallow / LVDS panel overlay | not read | none | unknown |

## What matters for `DriverProfile::Imx95Dpu`

Nothing can be said from source. A 6.6 NXP tree predates 6.18 by a long way, so differences are likely; the CRTC and plane counts, the shared-scaler and fetch-unit model, and the zpos/blend properties are the rows to check first. Until then the profile stays "read from 6.18, unmeasured", and `wp0-probe` (observed on the board) is the only authority.
