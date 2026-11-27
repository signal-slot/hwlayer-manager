# i.MX95 M7: two-plane DPU overlay under Zephyr (design note)

Desk research, 2026-11-28. **Nothing here was run or observed on a board, and nothing here is a hardware claim.** SoW §5.1 puts overlay after scanout, and scanout from the M7 is not proven yet. Who owns the DPU under the Verdin iMX95 boot flow is the subject of `docs/imx95-zephyr-feasibility.md`, not of this note.

Tags:

- **source**: read from the file named, at the pinned revision below.
- **inferred**: a conclusion drawn from the sources. The sources do not state it.

## Sources

| Short name | Repository, revision | Files read |
| --- | --- | --- |
| NXP 6.6 | `github.com/nxp-imx/linux-imx`, tag `lf-6.6.52-2.2.2` = commit `5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc` | `drivers/gpu/drm/imx/dpu95/`: `dpu95.h`, `dpu95-drv.h`, `dpu95-core.c`, `dpu95-crtc.c`, `dpu95-plane.c`, `dpu95-plane.h`, `dpu95-kms.c`, `dpu95-fetchunit.c`, `dpu95-fetchunit.h`, `dpu95-fetchlayer.c`, `dpu95-fetchyuv.c`, `dpu95-fetcheco.c`, `dpu95-layerblend.c`, `dpu95-hscaler.c`, `dpu95-vscaler.c`, `dpu95-constframe.c`, `dpu95-extdst.c`, `dpu95-framegen.c`, `dpu95-domainblend.c`, `dpu95-dither.c`, `dpu95-drv.c` |
| Zephyr | `github.com/zephyrproject-rtos/zephyr`, `main` = commit `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7` (2026-11-27); the driver last changed in `f2f8bed4c19e` (2026-09-16) | `drivers/display/display_socionext_dpu.c`, `dts/bindings/display/socionext,dpu.yaml`, `dts/arm/nxp/imx/nxp_imx95_m7.dtsi`, `west.yml` |
| HAL | `github.com/zephyrproject-rtos/hal_nxp`, commit `1a8937f3f8dfe5c68f59cc2a0c0c0fccdb096650` (the revision pinned by the Zephyr `west.yml` above) | `mcux/mcux-sdk-ng/drivers/dpu_1/fsl_dpu.h`, `fsl_dpu.c`, `CMakeLists.txt`; `drivers/dpu/fsl_dpu.{h,c}` for comparison only |
| Engine | this repository, commit `f30c09350ee0` | `crates/composition-core/src/{backend.rs,caps.rs,planner.rs,engine.rs}`, `crates/composition-drm/src/profile.rs`, `docs/imx95-dpu-source-6.6.md` |

Caveats on the sources:

- The board runs the Toradex kernel `6.6.157-7.8.0`. Its `dpu95` driver was not read. That it matches NXP `lf-6.6.52-2.2.2` is **inferred** (see `docs/imx95-dpu-source-6.6.md`).
- Files were read through a fetch tool that drops line numbers, so this note cites functions and macros, not lines.
- `fsl_dpu.c` was read verbatim only up to character 117,945 of 186,658. The rest is the local-dimming tables, known only from the tool's summary.
- Which `dpu_1` build the MIMX95 uses was not confirmed, so unit sets and interrupt bit numbers that differ between the `FSL_FEATURE_DISPLAY_SEERIS_MDR7` build and the other one are open (see below).

## 1. What the stock Zephyr driver does

| Finding | Basis |
| --- | --- |
| One fixed pipeline: `ConstFrame0` (primary input) and `FetchYUV0` (secondary input) into `LayerBlend1`, then `ExtDst0`. `DomainBlend0` and display (FrameGen) index 0 sit behind it. No FetchLayer, no second LayerBlend, no scaler | source (Zephyr, `dpu_init`) |
| Every register access goes through the HAL (`DPU_*()`). The driver itself has no register offsets | source (Zephyr) |
| Flip: `DPU_SetFetchUnitSrcBufferAddr(.., kDPU_FetchYuv0, 0, fb)`, then `DPU_TriggerPipelineShadowLoad(.., kDPU_PipelineExtDst0)` | source (Zephyr) |
| Latch wait: the ISR reads and clears all pending group-0 flags. On `kDPU_Group0ExtDst0ShadowLoadInterrupt` (bit 3) it gives a semaphore, and `display_write()` blocks on it with `K_FOREVER`. There is no frame-complete or vsync callback | source (Zephyr, HAL for the bit) |
| The ISR uses a hard-coded base `0x4B780000`, separate from the DT `reg` `0x4b400000`. That is DPU base + `0x380000`. NXP 6.6 places its first display interrupt block at base + `0x381000`, and the HAL places `IRQ_INTERRUPTENABLE` at `0x1008` from the base it is given. So both point at the same block | source for each number; that they agree is inferred |
| The DT node has `interrupts = <215 0>`, the same NVIC line as `disp_irqsteer_master1`. It has no clocks or power domain, and the driver requests none | source (Zephyr dtsi) |
| Formats: RGB565, RGB888, ARGB8888. Pitch is aligned to 32 bytes | source (Zephyr) |

The stock driver therefore gives one plane, with an ExtDst shadow-load interrupt that can serve as a latch signal. Everything that follows is new driver work.

## 2. DPU units for a primary + overlay configuration

### 2.1 Inventory

| Unit | In NXP 6.6 (`dpu95-core.c` tables) | In the HAL `dpu_1` | Basis |
| --- | --- | --- | --- |
| FetchLayer | FL0, FL1. RGB only: `dpu95_fl_init` sets no `cap_mask`, FetchECO or scaler. Eight sublayers exist in hardware (`SUBID_OFFSET`), and the driver uses only sublayer 0 | `kDPU_FetchLayer0/1` with the SubLayer attribute. `sublayer` 0..7 is accepted by `DPU_SetFetchUnitSrcBufferConfig`, `..SrcBufferAddr`, `..Offset` and `DPU_EnableFetchUnitSrcBuffer` | source |
| FetchYUV | FY0..FY3. YUV, packed 4:2:2, FetchECO for the chroma plane, scalers | `kDPU_FetchYuv0/1/3` in both builds. `kDPU_FetchYuv2` only in the non-MDR7 build | source |
| FetchECO | FE0, FE1, FE2, FE9 (FY3 pairs with FE9) | FetchEco0/1/9, plus FetchEco2 only in the non-MDR7 build | source |
| FetchWarp | **Does not exist** in either source: no file, no link ID. `kDPU_FetchRot9` is the HAL's nearest unit and belongs to the blit side | none | source |
| LayerBlend | LB1..LB6 (no LB0) | LB1..LB5, plus LB6 only in the non-MDR7 build | source |
| ConstFrame | CF0, CF1 (content), CF4, CF5 (safety) | same | source |
| ExtDst | ED0, ED1 (content), ED4, ED5 (safety) | same | source |
| FrameGen, DomainBlend | 0, 1 | display index 0/1, DomainBlend0/1 | source |
| Scalers | HS4/VS4 for display, HS9/VS9 for blit. VS4 is not used on SoC rev 1.0/1.1 (`use_vs4`) | HScaler4, VScaler4, Matrix4 | source |

That the MIMX95 uses the non-MDR7 build (6 LayerBlends, 4 FetchYUV) is **inferred**, because the NXP Linux driver programs LB6 and FY2 on this SoC. It is not confirmed from the HAL build files.

### 2.2 Proposed minimal two-plane pipeline (one content stream)

```
ConstFrame0 ──prim──▶ LayerBlend1 ──prim──▶ LayerBlend2 ──▶ ExtDst0 ──▶ DomainBlend0 ──▶ FrameGen0 ──▶ (pixel link / LDB)
FetchLayer0 ──sec───▶ LayerBlend1           ▲
FetchYUV0  ──────────────────sec────────────┘
```

| Element | Choice | Why | Basis |
| --- | --- | --- | --- |
| Primary plane | FetchLayer0, sublayer 0, on LayerBlend1 with ConstFrame0 as the primary input | Matches NXP 6.6: stream 0 builds from `lb[0]` (LB1) with `cf[0]` at the bottom. FetchLayer is tried first in Linux's allocation order (FL0, FL1, FY0..FY3), and RGB needs no FetchYUV | source for the Linux rule; the choice is inferred |
| Overlay plane | FetchYUV0 on LayerBlend2, primary input = LB1 | Leaves room for NV12/YUYV and a scaler later. It is also the unit the stock Zephyr driver already drives | inferred |
| ExtDst | ED0, source = top LayerBlend | `dpu95_plane_atomic_update`: `if (is_top) dpu95_ed_pec_src_sel(ed[sid], lb link)` | source |
| Bottom-plane blend | LB1 `BLENDCONTROL` with PRIM_C = ZERO, SEC_C = CONST_ALPHA, alpha opaque: the primary plane is forced opaque | `dpu95_lb_blendcontrol`, `zpos == 0` case | source |
| Overlay blend | NONE: PRIM_C = 1-CONST_ALPHA, SEC_C = CONST_ALPHA. Premultiplied: PRIM_C = 1-SEC_ALPHA, SEC_C = CONST_ALPHA. Coverage: PRIM_C = 1-SEC_ALPHA, SEC_C = SEC_ALPHA. Plane alpha goes in `BLENDCONTROL` bits 23:16 and the fetch unit's `CONSTANTALPHA`. The fetch unit's `LAYERPROPERTY` gets `ALPHACONSTENABLE`, plus `ALPHASRCENABLE` if the format has alpha | `dpu95_lb_blendcontrol`, `dpu95_fu_set_pixel_blend_mode` | source |
| Overlay position | **Two sources disagree.** NXP 6.6 writes the LayerBlend `POSITION` register (`dpu95_lb_position`) and keeps `LAYEROFFSET` at 0. The HAL writes `POSITION = 0` and places layers with `LAYEROFFSET0` (`DPU_SetFetchUnitOffset`). Its comment says the foreground "must not" be placed with `POSITION`, and that layer position belongs to "the stream" shadow domain, not the layer's | source (both). Which one is right for the overlay is open; settle it with the register probe (item O1) |
| Source crop | Folded into the base address: `addr + pitch*y + cpp*x` (`drm_plane_state_to_baseaddr`). Fetch `SOURCEBUFFERDIMENSION` and `FRAMEDIMENSIONS` = crop size | source (NXP 6.6) |
| Disabled overlay | `disable_src_buf` (clear `SOURCEBUFFERENABLE`, FetchYUV source NONE), LB `CLKEN_DISABLE` + `LB_NEUTRAL`, ED source = LB1 | `dpu95_fu_disable_src_buf`, `dpu95_crtc_atomic_begin` | source for the steps; using them for the Zephyr overlay is inferred |
| Not in the first step | Scalers (HS4/VS4), FetchECO (NV12), the second stream, the safety stream | Scope. Each adds a shared unit, see §4 | inferred |

The constraints NXP 6.6 checks in `dpu95_plane_atomic_check` apply to the hardware, not to Linux, and a Zephyr `check()` has to repeat them (**source** for the list; that they are hardware limits is **inferred**):

- 32 bpp base aligned to 4, 16 bpp to 8, pitch ≤ `0x10000` and a multiple of the pixel size.
- Destination fully on screen, no clipping.
- Even x/y/width/height for subsampled formats.
- Source ≤ 8192, or ≤ 2048 when scaled.
- Linear only.

## 3. Shadow and commit semantics for an atomic two-plane update

What the sources say:

| Finding | Basis |
| --- | --- |
| Every pixel-engine unit has `SHDEN` (bit 0 of `STATICCONTROL`, offset 0x8) set at init: fetch units, LayerBlends (also `SHDLDSEL`/`SHDTOKSEL` = BOTH), ConstFrame, scalers, ExtDst (PixEngCfg `STATIC` and ExtDst `STATICCONTROL`), FrameGen (`FGSTCTRL`), DomainBlend (token/load select SW) | source (NXP 6.6 `*_hw_init`; the HAL `DPU_Init*` does the same) |
| ExtDst PixEngCfg sync mode is SINGLE (`PIXENGCFG_STATIC.SYNC_MODE = 0`). Kick mode is EXTERNAL, with no software kick. FrameGen's kick point is `PKICKCONFIG = COL(hact+1) | ROW(vact)`, the end of the active area | source (NXP 6.6 `dpu95_ed_hw_init`, `dpu95_fg_cfg_videomode`) |
| A page flip in Linux (`dpu95_crtc_atomic_flush`, no modeset) is: program all planes' shadow registers in `atomic_begin`/`atomic_update`, then one write of `PIXENGCFG_TRIGGER.SYNC_TRIGGER` on the CRTC's content ExtDst. Then wait up to 1 s for the `EXTDSTn_SHDLOAD` interrupt (a timeout is only logged), then check `FGCHSTAT.PFIFOEMPTY` | source |
| FrameGen (`FGSLR.SHDTOKGEN`) and DomainBlend (`CONTROLTRIGGER.SHDTOKGEN`) shadow tokens are generated only in `atomic_enable`, never on a flip | source |
| The vblank event is not sent on SHDLOAD. It is queued before the trigger and sent from the `DISENGCFG_FRAMECOMPLETEn` handler. The driver comment says frame complete comes about one vblank after the ExtDst SHDLOAD | source |
| `PIXENGCFG_STATUS.SYNC_BUSY` (bit 8) exists. Neither NXP 6.6 nor the HAL reads it | source |
| `FGTIMESTAMP` (FrameGen + 0x98) holds a frame index (bits above 14) and a line index. Linux uses it for `get_vblank_counter` and the scan-out position. The HAL has no API for it | source |
| The HAL's `DPU_InitPipeline` comment says "single mode", but the function never writes the sync-mode bit. It relies on the reset value | source; that the reset value is SINGLE is not stated |

What this means for the M7 driver (**inferred** throughout):

1. **Atomicity comes from one trigger per pipeline.** All units feeding ExtDst0 (FL0, FY0, LB1, LB2, CF0, ED0) belong to the ExtDst0 pipeline's shadow domain. Writing both planes' shadow registers and then issuing a single `DPU_TriggerPipelineShadowLoad(kDPU_PipelineExtDst0)` should make the hardware load both at the same kick. Writing order between the two planes does not matter as long as the trigger comes last. That this includes the LayerBlend `POSITION` (or `LAYEROFFSET`) of the overlay is exactly the open point in §2.2.
2. **No new writes between the trigger and SHDLOAD.** In SINGLE mode a trigger arms one load at the next kick. Shadow writes made after the trigger and before the load could be taken partly into that load. So the M7 backend should refuse a second commit until SHDLOAD (`Capabilities::replace_pending = false`, return `BackendError::Busy`). The Linux driver avoids the window the same way, by waiting synchronously in `atomic_flush`.
3. **Latch = `EXTDST0_SHDLOAD`.** That is the moment the new configuration becomes active, at the kick at the end of the active area, before the next frame is fetched. It fits the engine's `Latched` meaning ("its buffers are now being scanned out", plus the previous buffers are free). `DISENGCFG_FRAMECOMPLETE0`, about one vblank later, is the DRM vblank event, and would only add delay. Read `FGTIMESTAMP` in the ISR to stamp each latch with a frame index, which makes missed frames visible.
4. **Missed kick.** A trigger issued after the kick of frame N latches at the kick of frame N+1. Nothing reports this except the frame index, so the timestamp in step 3 is also the way to measure latch latency.
5. **Disabling the overlay goes through the same trigger.** Neutral LB2 plus ED0 source = LB1, plus `SOURCEBUFFERENABLE` = 0, all in shadow, then one trigger.
6. **First enable is different.** Bring-up needs the FrameGen and DomainBlend tokens (`DPU_TriggerDisplayShadowLoad`, `DPU_TriggerDisplayDbShadowLoad`) and `DPU_StartDisplay`, as the stock Zephyr driver does in `dpu_init` and its first `write()`. Later flips need only the ExtDst trigger.

The interrupt bit for ExtDst0 SHDLOAD is bit 3 of group 0 in both HAL builds. The DisEngCfg and FrameGen bits differ between the MDR7 and non-MDR7 builds (for example DisEngCfg FrameComplete0 is bit 19 in one and 18 in the other) (**source**). NXP 6.6 uses 19 (`DPU95_IRQ_DISENGCFG_FRAMECOMPLETE0`), which agrees with the non-MDR7 numbering (**inferred**).

## 4. Conflicts with the Linux side if both use the DPU

NXP 6.6 assumes it owns the whole DPU. Each row is a reason why sharing it with the M7 needs work on the Linux side too, or why the M7 should own the DPU while Linux does not use it.

| Shared thing | What Linux does with it | Conflict | Basis |
| --- | --- | --- | --- |
| Fetch-unit pool FL0/1, FY0..3 | `dpu95_drm_atomic_check` marks every fetch unit available and reallocates them on each commit, across both CRTCs. There is no way to reserve units for another core | Linux can hand the M7's FL0/FY0 to its own plane | source for the allocation; the conflict is inferred |
| LayerBlends LB1..LB6 | Stream 0 takes from LB1 upward, stream 1 packs downward to LB6 | Any M7 LayerBlend can be taken by either Linux stream | source; conflict inferred |
| Scalers HS4/VS4 (one pair) | `hs_used`/`vs_used`, bound to one stream, no hot migration | An M7 scaled overlay and any Linux scaled plane exclude each other. This is the `SCALED_H`/`SCALED_V` capacity-1 groups of `DriverProfile::Imx95Dpu` | source; conflict inferred |
| `DriverProfile::Imx95Dpu` | Models 6 fetch units, 4 of them for YUV/scaled, 1 HScaler and 1 VScaler, all planes of one output | Units held by the M7 would shrink these capacities. The profile (read only here) has no such parameter, and Linux's `TEST_ONLY` would not know about the M7 either | source (profile.rs); conflict inferred |
| `PLANE_ASSOCIATION` (blk-ctrl syscon `0x20`) | `dpu95_fu_set_stream_id` does a `regmap_update_bits` per fetch unit on every update | A read-modify-write register shared by two cores with no lock | source; conflict inferred |
| Hardware init on resume | `dpu95_runtime_resume` → `dpu95_irq_hw_init` (masks and clears every interrupt) → `dpu95_submodules_hw_init` (resets every unit, sets the ExtDst domain mask/`MASTER`) | A Linux runtime resume would wipe the M7's pipeline and interrupt enables | source; conflict inferred |
| Power and clocks | Runtime PM: the DPU is powered via SCMI `IMX95_PD_DISPLAY` and taken when a CRTC gets planes, dropped when it has none. Clocks `axi`, `apb`, `pix`, `ocram`, `ldb`, `ldb_vco` | Linux may power the DPU off under the M7. Who may vote on the display power domain and clocks is SCMI/SM configuration | source (NXP 6.6, `docs/imx95-dpu-source-6.6.md`); the SM side is not read here |
| Interrupt blocks | Linux reads and acks the display interrupt banks at base + `0x381000` / `0x3a1000` through its own IRQ domains. The Zephyr ISR reads the same group-0 status and clears **all** pending bits | Each core would ack the other's events | source for both handlers; conflict inferred |
| irqsteer | Zephyr routes the DPU through `disp_irqsteer` (master 1 → NVIC 215). The Linux side's routing of the same events was not read | A shared irqsteer channel would need a split per core | source (Zephyr dtsi); Linux side not read |
| Safety stream ED4/ED5, CF4/CF5, DomainBlend | Initialised (`hw_init`, domain mask `ALLOW_ALL`, `MASTER`). The `*_safe_get` getters are never called, and DomainBlend is in `DB_MODE_PRIMARY` | Not a conflict today. The hardware has a second, independent pipeline per output that DomainBlend can blend over the content stream, which looks like a route for an M7 overlay above a Linux scene. Nothing in the sources shows how it is meant to be split between cores | source for the init; the route is inferred and unverified |

Workable ownership models, all **inferred**:

- **A. The M7 owns the whole DPU, Linux has it disabled** (`dpu` node off in the Linux DT, or a Linux DT without the display pipeline). There are no conflicts and the full unit set is the M7's. Weston loses its display, so this is a separate boot configuration, and the board rules forbid making it the persistent one. This is the model for the first overlay step.
- **B. Split by stream.** Linux keeps stream 0, and the M7 takes stream 1 (FrameGen1, ED1, CF1) with a fixed set of units. This needs Linux changes: a reserved-unit list in `dpu95_get_plane_grp_res`, no resets of those units on resume, a lock for `PLANE_ASSOCIATION`, and an interrupt split. It also needs a second display output on the carrier. The engine on the Linux side would then see a profile with smaller capacities.
- **C. M7 overlay on the safety stream over a Linux content stream.** This has the least Linux-side unit sharing, but its existence and semantics are inferred only. Research, not a plan.

## 5. Mapping onto the engine's backend contract

The trait is `DisplayBackend` in `crates/composition-core/src/backend.rs` (**source**). Proposed M7 backend for model A, one output, two planes (**inferred** design):

| Contract | M7 implementation |
| --- | --- |
| `capabilities()` | One `OutputCaps` (panel size and refresh rate from the DT timings). Two `PlaneCaps`: <br>• Primary `PlaneId(0)` on FL0/LB1, `Zpos::Fixed(0)`, formats ARGB/XRGB8888 and RGB565 linear, blend modes Opaque only (forced opaque at the bottom), `scaling: None`. <br>• Overlay `PlaneId(1)` on FY0/LB2, `Zpos::Fixed(1)`, the same formats (NV12 later), `plane_alpha: true`, blend modes None/Premultiplied/Coverage, `scaling: None` in step one. <br>No `resource_groups`: the units are fixed per plane. `replace_pending: false` |
| `check()` | The §2.2 alignment, pitch and on-screen rules against the imported buffers. Returns `Rejected` if a rule fails. No hardware access |
| `commit()` | `Busy` if a submission has no latch yet. Otherwise write the shadow registers for every assignment (base, stride, dimensions, format, alpha, LB select/blend/position), disable the unused plane, set the ED0 source to the top LB, then one `DPU_TriggerPipelineShadowLoad(ExtDst0)`. Record the submission as pending. Must not block, unlike the stock `display_write()` |
| Latch | The `EXTDST0_SHDLOAD` ISR clears only its own bit, reads `FGTIMESTAMP`, and pushes `Latched { output, submission }` into an ISR-safe single-slot queue. `poll_event()` drains it. Because `replace_pending` is false, the "latched before it was replaced" case in the trait documentation cannot happen |
| Timeout and loss | The engine's `set_latch_timeout`/`advance_time` covers a missing SHDLOAD. If the DPU loses power or is reset under the M7 (model B/C), report `DeviceLost` (detection mechanism open) |
| Buffer import | Out of the trait, as the trait requires: physical addresses in M7-visible memory, mapped to `BufferId` |

For model B, `capabilities()` would list only the M7's units, and the Linux-side `DriverProfile::Imx95Dpu` would need reduced capacities. That is a change to `composition-drm`, outside this note.

## 6. Proposed M2 overlay work items

All of these come after M2 scanout (SoW §5.1) and after the ownership answer in `docs/imx95-zephyr-feasibility.md`.

| # | Item | Output | Depends on |
| --- | --- | --- | --- |
| O1 | Register probe on the M7: read back `STATICCONTROL`/`SHDEN` of FL0, FY0, LB1, LB2, ED0, the ExtDst PixEngCfg `STATIC` (sync mode), `PIXENGCFG_STATUS`, and the IPIDENTIFIER. Settles the HAL build (MDR7 or not), the LB6/FY2 presence and the sync-mode reset value | Dated observed values in a doc | M7 image loads (m7-run) |
| O2 | Confirm `POSITION` vs `LAYEROFFSET` for overlay placement, with a small test pattern at a non-zero offset | Dated result from the board, one of the §2.2 rows closed | O1, scanout |
| O3 | Extend the Zephyr application (in `tests/board/imx95_m7/`) to two planes: FL0 primary and FY0 overlay as in §2.2, with static buffers, opaque and then plane-alpha blending | Board test kept in the repository | scanout app, O1 |
| O4 | Atomic update test: change both planes' buffers and the overlay position in one commit and check with the latch frame index that both changed at the same latch. Also check that a commit is refused while one is pending | Dated measurement or a blocker | O3 |
| O5 | Latch path: ExtDst0 SHDLOAD ISR that clears only its own bit, plus `FGTIMESTAMP` stamping. Measure trigger-to-latch time and frame-index steps | Dated latch-latency numbers | O3 |
| O6 | `DisplayBackend` for the M7 as in §5, linking the engine staticlib (see the `m7-engine-app` and `zephyr-backend-contract` items) | Board test that runs the engine on two planes | O4, O5 |
| O7 | Ownership model B/C study: read the SM configuration for the display power domain, clocks and irqsteer, and list the Linux `dpu95` patches model B needs | Doc, source-tagged | the feasibility doc |
| O8 | Optional: YUV overlay (FetchECO for NV12), then one shared scaler, with the matching `PlaneCaps` | Board test | O6 |

O1–O6 assume model A, with the DPU not used by Linux in that boot. If the feasibility doc finds that the M7 cannot own the DPU under the Toradex boot flow, O1–O6 become the reduced-scope bring-up or the blocker report that WP3 asks for.

## Open points

- The Toradex `dpu95` driver and the Verdin iMX95 DT were not read (see `docs/imx95-dpu-source-6.6.md`).
- Which HAL build is compiled for the MIMX95 (MDR7 or not).
- `POSITION` vs `LAYEROFFSET` for the overlay.
- Whether the ExtDst pipeline shadow domain covers every register a two-plane flip writes. This is inferred in §3, not stated.
- The register layout of the stream-1 interrupt bank: NXP 6.6 uses a second block at base + `0x3a1000`, while the HAL has three groups in one block. Not reconciled.
