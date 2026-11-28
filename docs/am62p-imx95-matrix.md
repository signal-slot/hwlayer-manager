# AM62P and i.MX95: difference matrix (WP4, from recorded docs)

> **Two matrices, one scope (WP4).** This file is the longer, sectioned form of the AM62P vs i.MX95 matrix, written from the documents on 2026-11-28 before the i.MX95 board observations were merged. [`wp4-difference-matrix.md`](wp4-difference-matrix.md) is the compact one-table form with per-cell tags and dates, and it carries those observations. Where the two disagree, `wp4-difference-matrix.md` lists the differences under "Known differences between the two matrices"; neither file is the winner.

This page compares the TI AM62P and the NXP i.MX95 using **only what other files of this repository record**. It adds no finding. It was written on 2026-11-28 from the documents at the commit this file was added in. Nothing here was measured by this page, and no device was used.

Every cell carries a tag and the document it comes from. Tags are those of the source, carried over unchanged:

- **observed (date)**: measured on a board or on vkms, with the date the source gives. No cell below has this tag; see "What is observed".
- **source**: read from driver, tool or firmware source, not run. The source's own date is given where it has one.
- **inferred**: a conclusion drawn by the source from source, not stated by it.
- **SoW**: stated in `SoW.md` as scope or as a description of the existing demo. Not a measurement and not verified by this repository.
- **repo**: a fact about this repository's own code or tests (what is built and tested on the host).
- **not recorded**: no document in the repository says it. It is not "no" and not "absent".

Abbreviations of the documents: `PC` = `docs/platform-capabilities.md`, `DPU66` = `docs/imx95-dpu-source-6.6.md`, `TP` = `docs/twoplane-backend.md`, `BOARD` = `docs/imx95-board.md`, `WP5` = `docs/wp5-gap-analysis.md`, `M1` = `docs/m1-imx95-test-plan.md`, `SoW` = `SoW.md`. `docs/observed/**` was searched for both SoCs (AM62P, DSS, tidss, sysfw, TISCI); it has no mention of either and holds other SoCs only.

## What is observed

Nothing in the cells below is observed on an AM62P or an i.MX95. `PC` states "Nothing here has been measured on a board" (desk research 2026-11-19); `DPU66` states "Nothing here is observed on a board" (2026-11-28); `TP` states "No board run has been made from this repository." Where a source names a measurement to be made later, the cell says so.

## 1. Linux DRM side

| Item | AM62P | i.MX95 |
| --- | --- | --- |
| Display controller | DSS (name used by the engine's firmware table; SoW §5.2, `TP`). **The Linux DRM driver is not recorded.** `PC` §"TI AM62P": "Not researched yet." | DPU, driver `imx95-dpu` in `drivers/gpu/drm/imx/dpu95/`. NXP `lf-6.18.y` `6e694eda40aa` (source, `PC`, 2026-11-19); NXP `lf-6.6.52-2.2.2` for the board's 6.6.157-7.8.0 kernel, Toradex tree inferred (source / inferred, `DPU66`, 2026-11-28) |
| CRTCs / outputs | not recorded | 2 CRTCs, one encoder each (source, `PC`, `DPU66`). The Verdin panel's LDB channel, CRTC and connector: not read (`DPU66`) |
| Planes (count, type) | The existing demo uses two: an application plane and a safety plane (SoW §5.2; `TP`). The total number of DSS planes: not recorded | 7: 2 primary, 5 overlay, no cursor (source, `PC`, `DPU66`). At most 6 active across both CRTCs (source, `PC`) |
| Plane → CRTC | not recorded | Primary: own CRTC only (source; mask inferred from DRM core). Overlays: both CRTCs (source) (`PC`, `DPU66`) |
| Formats | Application plane RGB565, safety plane ARGB8888 in the existing demo (SoW §5.2). The DSS format list: not recorded | One list on every plane: ARGB/XRGB/ABGR/XBGR/RGBA/RGBX/BGRA/BGRX8888, RGB565, YUYV, UYVY, NV12, NV21 (source, `PC`, `DPU66`). Modifiers: linear only (source). YUV needs a FetchYUV unit (4 exist); BT.709 full range rejected (source) |
| Scaling | not recorded | Upscale only (source; "upscale only" inferred from the DRM scale convention, `DPU66`). One horizontal and one vertical scaler shared by all planes on both CRTCs (source). Source size up to 8192 unscaled, 2048 scaled (source). On silicon rev 1.0/1.1 the 6.6 driver disables VScaler4; the board's revision is not known (source, `DPU66`) |
| Blending / alpha | The safety plane is ARGB8888 and the engine's driver table has a premultiplied-blending switch for it (`safety_premultiplied`, `TP`; repo). Whether DSS offers per-plane alpha or blend modes: not recorded | Plane alpha: yes. Blend modes: none, pre-multiplied, coverage (source, `PC`, `DPU66`). Bottom plane forced opaque (source, `PC` only; not checked in 6.6, `DPU66`) |
| Z-order | not recorded | Mutable zpos 0..5 (source, `PC`, `DPU66`) |
| Rotation | not recorded | No property (source, `PC`, `DPU66`) |
| Constraints on a commit | not recorded | Off-screen destination rejected; active CRTC with no plane rejected; a fetch unit, scaler or LayerBlend freed on one CRTC cannot be taken by the other in the same commit; no bandwidth check found; mode clock limit 350 MHz in 6.6 (source, `PC`, `DPU66`) |
| Hardware behind planes | not recorded | 6 fetch units (2 FetchLayer, 4 FetchYUV), 6 LayerBlends, allocated per commit (source, `PC`, `DPU66`) |
| Commit / latch model (Linux) | not recorded. For the R5F side see section 2 | The engine's DRM backend uses atomic commit with `TEST_ONLY` (repo; SoW §4.1). The hardware latch behaviour under Linux: not recorded |
| VSYNC reporting (Linux) | not recorded | DRM vblank initialised for 2 CRTCs, `drm_vblank_init(drm, DPU95_CRTCS)` (source, `DPU66`). Measured vblank/flip timing: not recorded (the M1 plan lists it as a planned measurement, `M1`) |
| Clocks and power | not recorded | From SCMI (System Manager): clocks `axi`, `apb`, `pix`, `ocram`, `ldb`, `ldb_vco`, power domain `IMX95_PD_DISPLAY`; the DT part comes from a summary of a truncated fetch and needs re-checking (source, `PC`, `DPU66`) |
| Sharing between two engines on one SoC | not recorded | Two engines on the two CRTCs are not independent: they draw from the same fetch units and scalers, so one lessee can make the other's commit fail (inferred, `PC`) |
| Engine backend | `composition-drm` is not recorded as used for the AM62P. The recorded AM62P path is the R5F's (SoW §5.2) | `composition-drm` (DRM/KMS), with the opt-in `DriverProfile::Imx95Dpu` encoding the rows above, "exactly as unverified as they are" (repo, `PC`) |

## 2. MCU / Zephyr side

| Item | AM62P (R5F) | i.MX95 (M7) |
| --- | --- | --- |
| Core that drives the display | R5F, an "R5F-owned safety plane" beside an A53-generated application frame (SoW §5.2). An existing working demo, not run from this repository (`TP`: "No board run has been made from this repository") | M7 is the candidate; **who may own the DPU is not established**. DPU and DSI nodes exist only for the M7 and are disabled by default; neither EVK lists display support (source, `PC`) |
| Zephyr display driver | `drivers/display/display_am62p_dss.c` in the demo repository `signal-slot/slint-zephyr-yakogl-mixed-criticality-ui` (SoW §5.2). Its content is not recorded in this repository's docs | `drivers/display/display_socionext_dpu.c` (`socionext,dpu`), merged May 2026: one fixed pipeline ConstFrame0 + FetchYUV0 → LayerBlend1 → ExtDst0; RGB565, RGB888, ARGB8888; no planes, scaling or alpha (source, `PC`). Overlay under Zephyr "does not exist in the stock driver" (source / inferred, `PC`) |
| Planes and overlay | Two-plane hardware overlay: application frame (RGB565) beneath a safety plane (ARGB8888) (SoW §5.2) | Single framebuffer in the stock driver (source, `PC`). A two-plane use of the DPU from the M7: not recorded. `TP` says the demo firmware fills the driver table for both "the AM62P DSS and the i.MX 95 DPU"; that firmware is not in this repository |
| Formats, scaling, alpha | Application RGB565 / safety ARGB8888 (SoW §5.2). Rest: not recorded | RGB565, RGB888, ARGB8888; no scaling or alpha in the stock driver (source, `PC`) |
| Commit / latch model | A commit is a set of plane writes that reach the panel together at a frame start; `latched_count` follows the GO bit (`TP`). Single ownership of GO/commit is an invariant to preserve (SoW §5.2). Register-level behaviour: not recorded | `latched_count` follows ExtDst0's sync busy bit (`TP`). Whether the driver follows its hardware is not shown (`TP`, "What the host test cannot show"). Latch semantics on the M7 are to be established first (SoW §5.1): not recorded |
| VSYNC reporting | The driver runs a callback in its interrupt once a frame (`set_vsync_callback`); latched-vsync acknowledgement is an invariant (`TP`; SoW §5.2). Interrupt ownership: not recorded | The same driver-table call is what the engine expects (`TP`). Whether the M7 can receive the DPU interrupts under the board's boot flow: not recorded (SoW §5.1 lists it as to be established) |
| Resource sharing between cores | A53 produces the application frame; R5F owns the safety plane and the GO/commit (SoW §5.2; `TP`). What enforces the split: not recorded. SoW §7: an R5F safety plane surviving an A53 crash "is evidence of a particular configuration, not a general safety guarantee" | Not established. SoW §5.1: which core owns DPU, clocks, power domains, display routes, interrupts and scanout memory is to be determined. Whether Linux and the M7 can use the DPU at the same time: not recorded |
| Ownership mechanism | TI resource management / system firmware: **not recorded in any document** of this repository | System Manager (SCMI) configuration would decide A55 versus M7; "nothing in Linux or Zephyr arbitrates it" and the System Manager "was not examined" (not found / inferred, `PC`). On Linux the DPU's clocks and power come from SCMI (source, `PC`) |
| Memory and caches | SoW §7 requires cache, DMA visibility and trust boundaries to be documented on MCU platforms. AM62P values: not recorded | Same requirement (SoW §7). `TP`: ordering and caches between R5F/M7, the controller and the producer are not shown by the host test. i.MX95 values: not recorded |
| Failure behaviour | Producer-stops resilience and ordered safety/background transitions are invariants of the existing demo (SoW §5.2, WP4). Not reproduced here | Not recorded. The engine's fallback policy is a SoW §7 requirement, not a result |
| Engine backend | `backends/twoplane` (`ce_twoplane.h`, `ce_twoplane.c`), taken from the AM62P firmware; tested on the host against a fake driver only (`TP`; repo) | `backends/twoplane` is what `TP` describes the demo firmware filling for the i.MX 95 DPU; that table is not in this repository. Whether the stock single-framebuffer Zephyr driver can fill it: not recorded. No Zephyr i.MX95 backend exists in the repository |

### `backends/twoplane`, per SoC

From `TP`. The driver table is the same for both SoCs; the documents record these differences:

| Item | AM62P | i.MX95 |
| --- | --- | --- |
| `latched_count` follows | the GO bit (`TP`) | ExtDst0's sync busy bit (`TP`) |
| Driver file in the demo | `display_port_<soc>.c` for the AM62P DSS (`TP`; not in this repository) | `display_port_<soc>.c` for the i.MX 95 DPU (`TP`; not in this repository) |
| Compiles for Arm | The backend, not a SoC driver: `thumbv7em-none-eabihf` (Cortex-M7) and `armv7r-none-eabihf` (Cortex-R5), by hand once on 2026-11-26 with clang 23; objects not linked or run (`TP`). It is a compile check, not a board result | same (one backend, one check) |
| Board test | None made (`TP`) | None made (`TP`) |

## 3. Lease and Weston coexistence

| Item | AM62P | i.MX95 |
| --- | --- | --- |
| Linux DRM lease use | not recorded | AGL `drm-lease-manager` (`68c48758c46d`): default lease per connector holds the connector, one CRTC and the planes usable only with that CRTC. i.MX95 overlays are shared between both CRTCs, so a default lease holds the primary only and the overlays must be listed by plane ID in the TOML file (source; consequence inferred, `PC`) |
| Weston on the board | not recorded | Weston owns the display and holds DRM master on the Verdin iMX95 image; a DRM test means stopping it and restoring it (`BOARD`; a board rule, not a measurement) |
| Weston lease support | not recorded | None at upstream `main` `c8d00e2` (source, `WP5`). The board's Weston, or an AGL/Toradex fork: not read (`WP5`, W5-02) |
| Coexistence in practice | Two cores share the one display by plane: A53 content below the R5F's safety plane (SoW §5.2). This is core-to-core sharing, not Weston. Weston on the AM62P: not recorded | Standalone leased output first; Weston-owned output later (SoW §4.2). A leased output beside Weston's other output may be blocked by the single panel (inferred, `WP5` W5-10). No coexistence run recorded |
| Revocation | not recorded | `ENOENT` after revocation, mapped by the backend to `DeviceLost` (source, `PC`) |

## 4. What the recorded documents let one compare

- **Model size.** The two-plane backend targets two planes on one output (`TP`; repo). The i.MX95 Linux DPU is recorded with up to 6 active planes drawn from shared units (source, `PC`). That a two-plane use of the i.MX95 DPU fits inside the Linux profile is only what `PC` says: "look achievable" (inferred).
- **Latch signal.** Both are recorded as a hardware latch the driver must report: GO bit against ExtDst0 sync busy (`TP`). Neither has been checked against hardware.
- **Ownership.** The AM62P split (R5F safety plane, A53 application) is described by SoW as an existing working configuration. The i.MX95 split is an open question that comes first (`PC`, SoW §5.1). The mechanisms (TI system firmware versus System Manager) are documented for neither SoC beyond the i.MX95 SCMI clock/power note.
- **Which constraints are truly generic (WP4's question).** The repository cannot answer yet: the AM62P column is mostly "not recorded" on the Linux side and no cell of either column is observed.

## Gaps for the planner (not done here)

1. AM62P Linux DRM driver, plane list, formats, scaling, blending, z-order: `PC` says "Not researched yet". The demo's `docs/display-sharing.md`, `display_am62p_dss.c` and `display_hw.c` (named in SoW §5.2) are not read in this repository.
2. AM62P ownership: TI system firmware and resource assignment, interrupt ownership, cache and memory-protection setup.
3. i.MX95 ownership under the board's boot flow (System Manager configuration), the first M2 question (`PC`, SoW §5.1).
4. Any observed value: the `probe` example on both boards (`PC`, `M1`).
