# i.MX95 display driver, kernel 6.6.157-7.8.0: source reading

Desk research, 2026-11-28. **Nothing here is observed on a board.** Tags: **source** = read from the named file; **inferred** = concluded, not stated by the source. Compared with `docs/platform-capabilities.md` (lf-6.18.y, commit `6e694eda40aa`).

## Which kernel

The board reports `6.6.157-7.8.0`.

- Layer `meta-toradex-nxp` (`https://git.toradex.com/meta-toradex-nxp.git`), branch `scarthgap-7.x.y`, recipe `recipes-kernel/linux/linux-toradex_6.6-2.2.x.bb`. It names kernel `git://git.toradex.com/linux-toradex.git`, `SRCBRANCH = "toradex_6.6-2.2.x-imx"` (read from the cgit pages of git.toradex.com).
- Recipe commit `962f3217b3e4` (2026-09-25, "linux-toradex: update to stable v6.6.157") is the state that matches the board; the next recipe change is `d1e2818c702c` ("Update to v6.6.161"). That BSP 7.8.0 equals this state is **inferred**: no release tag or notes were seen.
- **The exact Toradex kernel commit (`SRCREV_machine` at `962f3217b3e4`) was not obtained**, and no file of `linux-toradex` was read. `git fetch`, `gh api` and the cgit pages (behind a bot challenge) were not available; `raw.githubusercontent.com` has no Toradex mirror (404). `git ls-remote` gave only current heads: `linux-toradex` `toradex_6.6-2.2.x-imx` = `8ad2b7ff6b56` (this is 6.6.161, newer than the board, not read).
- **What was read instead:** NXP `nxp-imx/linux-imx`, tag `lf-6.6.52-2.2.2` (tag object `f9a8e33f5cde`, commit `5a0a5e71d2bd`, which is also the head of `lf-6.6.y` on 2026-11-28), via `raw.githubusercontent.com/nxp-imx/linux-imx/lf-6.6.52-2.2.2/<path>`. The Toradex branch is on the NXP `lf-6.6.52-2.2.x` line (the 6.6.52 base was read from the tag's Makefile in an earlier pass), so it is **inferred** that the Toradex tree has the same `dpu95` driver, plus stable updates to 6.6.157 and Toradex patches. The Toradex deltas are not read. In particular **no Verdin iMX95 / Mallow / LVDS panel device tree was read** (not in the NXP tree; `imx95-verdin.dts` is 404 there).
- Line numbers: the fetch tool returns text without them. References below give file and function/macro names; only the top of `dpu95-plane.c` was counted by hand (`dpu95_plane_formats[]` lines 29-44).

`P` below is `drivers/gpu/drm/imx/dpu95/`, in the NXP tag above.

## Table (same rows as the i.MX95 Linux table)

| Item | 6.6 finding | Basis | Differs from lf-6.18.y reading |
| --- | --- | --- | --- |
| CRTCs | 2: `#define DPU95_CRTCS 2` (`P`dpu95-drv.h); `dpu95_kms_prepare()` loops it and calls `drm_vblank_init(drm, DPU95_CRTCS)`; one encoder each (`DPU95_ENCODERS`) | source (NXP lf-6.6.52-2.2.2); Toradex tree inferred | no |
| Planes (count, types) | 7: `DPU95_PRIMARYS` = 2 (one per CRTC, created in `dpu95_crtc_init()` of dpu95-crtc.c) + `DPU95_OVERLAYS` 5 (created in `dpu95_kms_prepare()`). No cursor plane seen | source | no |
| Plane → CRTC | Primary: created with `possible_crtcs = 0` then bound by `drm_crtc_init_with_planes()`, so its own CRTC only (dpu95-crtc.c). Overlays: `possible_crtcs = dpu_drm->crtc_mask`, both CRTCs (dpu95-kms.c) | source; primary mask inferred from DRM core | no |
| Hardware behind planes | `struct dpu95_soc` (dpu95.h): `fl[2]`, `fy[4]`, `fe[4]`, `lb[6]`, `hs[2]`, `vs[2]`, `cf[4]`, `ed[4]`, `fg[2]`. `DPU95_HW_PLANES` = 6. `struct dpu95_plane_res`: `fl[2]`, `fy[4]`, `lb[6]`, shared by both CRTCs; `dpu95_atomic_assign_plane_source()` (dpu95-kms.c) gives each plane a fetch unit; stream 0 takes LayerBlends from the bottom, stream 1 from the top (`lb[DPU95_HW_PLANES - (n - i)]`); bottom stage is ConstFrame `cf[sid]` | source | no (2 FetchLayer, 4 FetchYUV, 6 LayerBlends, max 6 active planes) |
| Formats | `dpu95_plane_formats[]` (dpu95-plane.c:29-44): ARGB8888, XRGB8888, ABGR8888, XBGR8888, RGBA8888, RGBX8888, BGRA8888, BGRX8888, RGB565, YUYV, UYVY, NV12, NV21; one list for all planes | source | no |
| Modifiers | Linear only: `dpu95_drm_plane_format_mod_supported()` returns `modifier == DRM_FORMAT_MOD_LINEAR` (dpu95-plane.c:111, wired at :121); `drm_universal_plane_init()` gets a NULL modifier list | source | no |
| YUV | Needs fetch-unit capabilities: `DPU95_FETCHUNIT_CAP_USE_FETCHECO` for multi-plane, `..._PACKED_YUV422` (dpu95.h), checked in `dpu95_atomic_assign_plane_source()`. BT709 full range rejected in `dpu95_plane_atomic_check()`. Planes also expose color encoding (BT601/BT709) and range (limited/full) via `drm_plane_create_color_properties()`, default BT709 limited | source | no for the fetch-unit rule and the BT709 reject; the 6.18 reading does not mention the color-encoding/range properties (unknown) |
| Scaling (limits, shared scalers) | `drm_atomic_helper_check_plane_state()` with `min_scale = 1/2048`, `max_scale = DRM_PLANE_NO_SCALING`, i.e. upscale only (inferred from the DRM scale convention). One `hs` and one `vs` per `struct dpu95_plane_grp`, shared by both CRTCs; a second user gets `-EINVAL`. HScaler if `src_w != dst_w`, VScaler4 if `src_h != dst_h`. Scaling filter property: default, nearest-neighbour. **Silicon rev 1.0/1.1** (`imx95_rev_1_x_soc_devices`, dpu95-core.c): `use_vs4` is false and only VScaler9 is in the table ("VScaler4 is disabled for i.MX95 rev1.x SoC"); otherwise `use_vs4 = true` | source; "upscale only" inferred | mostly no. Two things the 6.18 reading does not state: the rev 1.x VScaler4 switch and the scaling-filter property (unknown whether 6.18 has them). The board's SoC revision decides whether vertical scaling works; it is not known from this reading |
| Source size | Max 8192 unscaled, 2048 scaled; max pitch 0x10000; address/pitch alignment checks; even width/height/x/y for 4:2:0 and 4:2:2 (`dpu95_plane_atomic_check()`). `mode_config` min 60x60, max 8192x8192 (`dpu95_kms_prepare()`) | source | no for 8192/2048; pitch, alignment and mode_config limits not in the 6.18 reading |
| Plane alpha | `drm_plane_create_alpha_property()` (`dpu95_plane_initialize()`) | source | no |
| Blend modes | `drm_plane_create_blend_mode_property()` with None (`PIXEL_NONE`), Pre-multiplied, Coverage. "Bottom plane forced opaque" was not checked in 6.6 | source; opaque rule not checked | no for the mode set; opaque rule unchecked |
| zpos | `drm_plane_create_zpos_property(plane, zpos, 0, DPU95_HW_PLANES - 1)`: mutable, 0..5; defaults primary 0, overlay 1 (`dpu95_plane_get_default_zpos()`, dpu95-plane.c:46-54); `drm_atomic_normalize_zpos()` in `dpu95_drm_atomic_check()` | source | no |
| Rotation | No `drm_plane_create_rotation_property` call in dpu95-plane.c | source | no |
| Off-screen destination | Rejected: dst must lie inside the adjusted mode (`dpu95_plane_atomic_check()`); `can_position = true`, no clipping | source | no |
| Active CRTC with no plane | `active && !plane_mask` returns `-EINVAL` (`dpu95_crtc_atomic_check()`) | source | no |
| Cross-CRTC migration | A fetch unit bound to another stream cannot be hot-migrated; CRTCs not in the state are pulled in as "prone to put" so their units stay stable (`dpu95_atomic_assign_plane_source()`) | source | no |
| Bandwidth check | None found in dpu95-core.c, dpu95-plane.c, dpu95-crtc.c, dpu95-kms.c. Mode check `crtc_clock > DPU95_FRAMEGEN_MAX_CLOCK` (350000 kHz, dpu95.h) returns `MODE_CLOCK_HIGH` in `dpu95_crtc_mode_valid()` | source | no for bandwidth; the 350 MHz clock limit is not in the 6.18 reading (unknown) |
| Clocks and power | dpu95-core.c takes `axi`, `apb`, `pix`, `ocram`, `ldb`, `ldb_vco` via `devm_clk_get` and never sets rates; power domain `IMX95_PD_DISPLAY` (SCMI), QoS written to blk-ctrl (`dpu95_set_qos()`); DMA mask 40 bit | source (DT part from a model-extracted summary of a truncated fetch, see below) | no (SCMI) |
| LVDS bridge and panel timing (DT) | `imx95.dtsi`: `dpu: display-controller@4b400000` (`nxp,imx95-dpu`, disabled), ports to `pixel_interleaver` channels 0/1 (`nxp,imx95-pixel-interleaver`), then `display_pixel_link` (`nxp,imx95-dc-pixel-link`), then `ldb` (`fsl,imx95-ldb`, channel@0/1, phys `fsl,imx95-lvds0-phy` / `lvds1-phy`, all disabled); `ldb_pll_pixel` is a fixed `/7` of `IMX95_CLK_LDBPLL`. Panel timing: **not read** | source for the node chain, but only the first 39,882 of 97,282 characters of `imx95.dtsi` were verbatim; the display nodes come from the tool's summary of the rest and need re-checking | not compared (the 6.18 reading has no DT row) |
| Verdin iMX95 / Mallow / LVDS panel overlay | **Not read.** Only in the Toradex tree (`imx95-verdin*.dts*`), which was not fetched; the NXP tag has no Verdin file. Nothing is known about which LDB channel or panel node the board uses | none | unknown |

## What matters for `DriverProfile::Imx95Dpu`

- The structure the profile models (2 CRTCs, 5 overlays + 2 primaries, 2 FetchLayer + 4 FetchYUV, 6 LayerBlends, one shared HScaler and VScaler, upscale only, 2048 scaled source limit, zpos 0..5, linear only) is the same in the 6.6 NXP tree as in the 6.18 reading. No row contradicts it. This is a source reading of NXP's tag, not of the Toradex kernel the board runs.
- New to check on the board: (1) SoC revision and whether VScaler4 is disabled, which would make vertical scaling depend on silicon rev 1.x; (2) the 350 MHz mode-clock limit; (3) the color-encoding/range and scaling-filter properties the probe should list; (4) which LDB channel, CRTC and connector the Verdin panel uses (needs the Toradex DT).
- To finish this note: obtain `SRCREV_machine` at recipe commit `962f3217b3e4` and read `drivers/gpu/drm/imx/dpu95/` and `arch/arm64/boot/dts/freescale/imx95-verdin*` in `linux-toradex` at that commit, which needs `git fetch` from git.toradex.com (not permitted in this run). Until then the probe on the board is the only authority.

## Fetch attempt for the exact board commit (item dpu-src-read, 2026-11-28)

**Result: the kernel tree at `SRCREV_machine` of `962f3217b3e4` was not fetched, so no row above has been re-read at that commit and none carries the tag "read from source (6.6.157-7.8.0, <commit>)".** The rows above remain the NXP `lf-6.6.52-2.2.2` reading and its caveats stand.

Exact failures, in order:

1. `git ls-remote https://git.toradex.com/meta-toradex-nxp.git scarthgap-7.x.y` (to learn the layer's recipe at `962f3217b3e4`): `fatal: refusing to work with credential missing host field` (git's own refusal, before any network traffic; the workspace git configuration supplies a credential entry without a host).
2. Retrying with the credential helper cleared (`git -c credential.helper= ls-remote ...`) and `curl https://git.toradex.com/cgit/meta-toradex-nxp.git/`: the harness answered "This command requires approval" and did not run them. The session is unattended, so network access to git.toradex.com was not granted.
3. Consequently `SRCREV_machine` is still unknown, `linux-toradex` was not cloned into `$CONNECTED_DEPS_DIR`, and nothing was read of `drivers/gpu/drm/imx/dpu95`, the LDB bridge, or the Verdin iMX95 / Mallow / 10.1" LVDS device tree at the board's commit. Panel timing remains unread. No kernel sources were committed.

To finish: run, with network access allowed, `git clone --filter=blob:none --no-checkout https://git.toradex.com/meta-toradex-nxp.git` (branch `scarthgap-7.x.y`), read `SRCREV_machine` in `recipes-kernel/linux/linux-toradex_6.6-2.2.x.bb` at `962f3217b3e4` (not `8ad2b7ff6b56`, 6.6.161), then `git init; git fetch --depth 1 git://git.toradex.com/linux-toradex.git <SRCREV>` into `$CONNECTED_DEPS_DIR/linux-toradex-6.6.157` and fill the table with file:line.

## Differences from `DriverProfile::Imx95Dpu` (`crates/composition-drm/src/profile.rs`)

Basis: the NXP `lf-6.6.52-2.2.2` reading in the table above, **not** the board's commit. Provisional until the exact tree is read; `profile.rs` was not edited.

| Profile states | 6.6 NXP reading | Difference |
| --- | --- | --- |
| At most 6 active planes (`LayerTraits::ANY`, capacity 6) | `DPU95_HW_PLANES` = 6, shared by both streams | none |
| 4 fetch units for YUV or scaled layers (`SCALED ∪ YUV`, capacity 4) | 4 FetchYUV; YUV needs `FETCHECO`/`PACKED_YUV422` capable units; scaling goes through `hs`/`vs` | none in the count. The profile lumps "scaled" with FetchYUV; whether a scaled RGB layer really consumes a FetchYUV in 6.6 was not verified |
| 1 HScaler, 1 VScaler (capacity 1 each) | one `hs`, one `vs` per plane group | none. Not modelled: on SoC rev 1.x `use_vs4` is false (VScaler9 only), which may change vertical scaling |
| Scaling: max_upscale 2048<<16, max_downscale 1<<16 | upscale only (`max_scale = DRM_PLANE_NO_SCALING`) | the profile's "2048" is the *source width limit when scaled* (2048), not an upscale ratio; as an upscale factor it is a loose bound. Check against the source before relying on it |
| `max_source` 8192x8192 for every non-cursor plane | 8192 unscaled, 2048 scaled | **differs**: scaled layers are limited to 2048, the profile allows 8192 |
| Not modelled | mode clock limit 350000 kHz (`MODE_CLOCK_HIGH`); even coordinates for 4:2:0/4:2:2; BT709 full range rejected; overlay-only frame (no primary) refused (`active && !plane_mask` is -EINVAL; whether an overlay-only plane set is accepted needs the atomic_check read); cross-CRTC unit migration | not in profile (acceptable if the kernel check catches them, but they surface as unexplained refusals) |
| Hardware shared with the other stream, invisible to the planner (doc comment) | confirmed: fetch units, layer blends, scalers are in `struct dpu95_plane_res`, shared | none |
| Doc comment: "taken from lf-6.18.y" | board kernel is 6.6.x | the profile is unchecked against the 6.6 Toradex tree; on this evidence it matches NXP 6.6.52 in structure |

### Retry after network grants (2026-11-28)

- `git ls-remote https://git.toradex.com/...` still fails with `credential missing host field` (local git configuration). `git ls-remote git://git.toradex.com/linux-toradex.git toradex_6.6-2.2.x-imx` works and returns `8ad2b7ff6b56` (6.6.161, not the board's). The exact `curl` of the cgit index returned HTTP 200, so the host is reachable.
- `git clone --bare --filter=blob:none git://git.toradex.com/meta-toradex-nxp.git`, `git init` + `git fetch`, and a `curl` of a cgit `plain/` URL were each refused by the harness ("requires approval") and not run. Only `git ls-remote` and the one exact curl were granted. `SRCREV_machine` is therefore still unread; the steps in "To finish" need `git clone`/`git fetch` (git:// works) or `curl` of cgit `plain/` URLs to be permitted.
