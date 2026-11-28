# M3-06: optional third target, decision inputs

Written 2026-11-28. **This document makes no decision.** SoW §9 M3-06 is the optional third target; [`open-issues.md`](open-issues.md) row M3-06 says "Decision recorded after M1; not required for M3". The decision therefore waits for M1 (the i.MX95 Linux run). This note only collects what the repository already says, so that the decision can be made quickly then.

**No new hardware claim is made.** Nothing here was measured, read from a datasheet or checked on a board for this note. Every fact is copied from a repository document that is named next to it, and keeps that document's tag. Tags as in [`platform-capabilities.md`](platform-capabilities.md): **observed** (read on a board, with a date), **read from source** (driver or tool source), **inferred** (a conclusion). Where the repository records nothing, the cell says "not recorded". "Not recorded" does not mean "does not exist". Where a fact is quoted, the tag of the quoted document is shown; a tag on a line of this note is that document's tag, not a new one.

Read-only inputs: `SoW.md`, `open-issues.md`, `platform-capabilities.md`, `sow-traceability.md` and the platform docs named below. None of them is changed by this note.

## 1. What the SoW asks

| Statement | Where |
| --- | --- |
| "Evaluate the next MCU candidate (i.MX RT1170 or RA8P1), subject to board availability; implementation is optional until i.MX95 is demonstrated." Deliverable: "optional third-target prototype". | [`SoW.md`](../SoW.md) §8 WP4, last bullet and Deliverables |
| §5.4 lists candidate backends "dependent on board availability and hardware verification": i.MX RT1170 / LCDIFv2, Renesas RA8P1 / GLCDC and DRW, STM32H7 / LTDC and DMA2D, Raspberry Pi 4/5 / HVS, RK3588 / VOP2. A backend is implemented "only after documenting the SoC's actual display topology, plane/formats/scaling/alpha restrictions, framebuffer memory accessibility, synchronization semantics, and testable board configuration." | [`SoW.md`](../SoW.md) §5.4 |
| "Additional MCU boards and 2D engines are optional until i.MX95 M1 (and the M2 feasibility decision) is complete." | [`SoW.md`](../SoW.md) §13 (line 383) |
| M3-06 "is not started"; decision after M1; not required for M3. | [`open-issues.md`](open-issues.md) M3-06; [`sow-traceability.md`](sow-traceability.md) WP4 row; [`m3-am62p-test-plan.md`](m3-am62p-test-plan.md) ("Not covered") |

The §5.4 sentence is the repository's own checklist for a candidate: topology, plane/format/scaling/alpha limits, framebuffer memory, synchronisation, and a testable board setup. Section 4 uses it as a criterion.

## 2. Candidates the repository discusses

Two groups appear. The SoW names the MCU candidates (RT1170, RA8P1, STM32H7). The repository holds material, much of it from boards, for five Linux DRM SoCs that the SoW lists under §5.4 or §5.3 as portability or contrast targets. The records of those five say, in their own words, that they count toward none of M1, M2 or M3 (for example [`stm32mp257-board-scenarios.md`](stm32mp257-board-scenarios.md), [`rk3588-vc4-profiles.md`](rk3588-vc4-profiles.md), [`imx8mp-x2-board.md`](imx8mp-x2-board.md)). Whether a Linux DRM SoC may serve as the "third target" at all is part of the decision (section 4, criterion C1); this note does not assume either answer.

| Candidate | In the SoW as | Repository material |
| --- | --- | --- |
| NXP i.MX RT1170 (LCDIFv2) | M3-06 candidate; §5.4 | Name only |
| Renesas RA8P1 (GLCDC, DRW) | M3-06 candidate; §5.4 | Name only |
| STM32H7 (LTDC, DMA2D) | §5.4 | Name only |
| STM32MP257 (LTDC) | not named | Board and source records, profile in code |
| Rockchip RK3588 (VOP2) | §5.4 | Board and source records, design note, profile in code |
| Broadcom BCM2712 (Pi 5) and BCM2711 (CM4), HVS | §5.4 ("Raspberry Pi 4/5 / HVS") | Board and source records, design note, profiles in code |
| NXP i.MX8M Plus (LCDIFv3) | §5.3 contrasting design, not a §5.4 candidate | Desk study, one read-only board probe, design note |

## 3. Per-candidate inputs

For each: planes and composition as recorded (with tags), remote-core or MCU path, what the repository has, risks. Risks are this note's reading of the recorded gaps; they are **inferred** from the cited rows and are not measurements.

### 3.1 NXP i.MX RT1170 and Renesas RA8P1, and STM32H7

- **Planes and composition:** not recorded. The only text is the SoW's list of controller names (LCDIFv2; GLCDC and DRW; LTDC and DMA2D). The repository has no source reading, no observation and no tag for any of them. A grep of the repository finds the names RT1170 and RA8P1 only in `SoW.md`, `open-issues.md` and `m3-am62p-test-plan.md`.
- **Remote-core or MCU path:** these are MCUs, so the target would be a bare-metal or Zephyr firmware. The repository does not say whether a Zephyr board or display driver exists for them. Not recorded.
- **What the repository has:** the engine's C API and the no-allocation core are the part that would be reused. For comparison, the MCU work already done is for the i.MX95 M7 and the AM62P R5F: see [`imx95-m7-backend-contract.md`](imx95-m7-backend-contract.md), [`imx95-twoplane-port.md`](imx95-twoplane-port.md), `tests/board/imx95_m7*`, `tests/board/am62p_r5f` and `backends/twoplane`. Those are the pattern, not material about these SoCs. The glossary, harness and profiles hold nothing for them.
- **Risks (inferred):**
  - The §5.4 sentence needs the topology and limits documented first. For these three, that desk work has not started, so it is the whole cost.
  - "Subject to board availability": the repository does not record that any such board exists in the farm. Not recorded.
  - The SoW itself says implementation is optional until i.MX95 is demonstrated, and M1 and M2 are not done in the repository's records.

### 3.2 STM32MP257 (LTDC)

- **Planes and composition (observed 2026-11-28 unless noted):** one CRTC; three planes (one primary, two overlays), all with `possible_crtcs` `0x1`; zpos mutable 0..2; `alpha` 0..65535; no `pixel blend mode` property, so the engine reports `blend [premultiplied]` as its default; modifiers `LINEAR` only; plane 49 RGB only, planes 34 and 42 include packed YUV, plane 42 also planar. Source: [`platform-capabilities.md`](platform-capabilities.md) section "STM32MP257 (LTDC)", [`observed/stm32mp257-topology.md`](observed/stm32mp257-topology.md), [`observed/stm32mp257.md`](observed/stm32mp257.md). Whether LTDC blends premultiplied is **inferred (unverified)** there.
- **Two-plane composition:** the harness confirmed two hardware planes on one output (planes 34 and 42), observed 2026-11-27 per the same section. **No picture was captured** (the KVM failed), so z-order, alpha and colours on the glass are not verified there.
- **Scaling and TEST_ONLY:** see [`observed/stm32mp257.md`](observed/stm32mp257.md) and the profile notes in `crates/composition-drm/src/profile.rs`; rotation checks that run past `TEST_ONLY` are **inferred**, "not run on the board", in the same section.
- **Remote-core or MCU path:** not recorded in any STM32MP257 document read for this note.
- **What the repository has:** topology and run records above, source readings ([`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md), [`stm32mp257-ltdc-sync-source.md`](stm32mp257-ltdc-sync-source.md), [`stm32mp257-ltdc-transform-source.md`](stm32mp257-ltdc-transform-source.md), [`stm32mp257-ltdc-yuv-source.md`](stm32mp257-ltdc-yuv-source.md), [`stm32mp257-ltdc-source-diff-6.6.md`](stm32mp257-ltdc-source-diff-6.6.md)), board scenarios A1-A5 ([`stm32mp257-board-scenarios.md`](stm32mp257-board-scenarios.md)), session records and a runbook ([`stm32mp257-session-c-runbook.md`](stm32mp257-session-c-runbook.md)), `DriverProfile::Stm32Ltdc`, and the `tools/drm-harness` binary.
- **Risks (inferred):** it is a Linux DRM target, so it adds a second Linux data point rather than a new execution environment; it does not exercise the R5F or M7 style of backend the SoW's WP4 is about. No picture evidence yet. The board clock was not NTP-synchronised, so run dates come from the session notes, not the board.

### 3.3 Rockchip RK3588 (VOP2)

- **Planes and composition (observed 2026-11-28):** two video ports registered (the board wires HDMI0 and HDMI1 only), CRTC 81 active; eight planes, one window each: Cluster0-win0 and Cluster1-win0 primary, the other six overlay; zpos range 0..=7; `alpha` and `pixel blend mode` (none, premultiplied, coverage) on every plane; cluster planes with AFBC modifiers, esmart planes linear only; plane 39 has an empty output mask in the probe's single-output view. Source: [`observed/rk3588-topology.md`](observed/rk3588-topology.md), [`observed/rk3588-props.md`](observed/rk3588-props.md). The observed scaling results there are `TEST_ONLY` results; no frame was committed in those sessions.
- **Source:** vendor `rockchip-linux/kernel` `develop-6.1` read 2026-11-28; mainline **not** read ([`observed/rk3588-source.md`](observed/rk3588-source.md), read from source).
- **Remote-core or MCU path:** not recorded in the RK3588 documents read for this note.
- **What the repository has:** the observed files above, a design note ([`rk3588-backend-design.md`](rk3588-backend-design.md), inferred), vendor and mainline source notes ([`rk3588-vendor-vop2-source.md`](rk3588-vendor-vop2-source.md), [`rk3588-mainline-source.md`](rk3588-mainline-source.md), [`rk3588-vop2-formats-source.md`](rk3588-vop2-formats-source.md), [`rk3588-vop2-limits-source.md`](rk3588-vop2-limits-source.md), [`rk3588-vop2-scale-source.md`](rk3588-vop2-scale-source.md)), a board procedure ([`rk3588-vc4-board-procedure.md`](rk3588-vc4-board-procedure.md)), profile results ([`rk3588-vc4-profiles.md`](rk3588-vc4-profiles.md)) and `DriverProfile::Rk3588Vop2`.
- **Risks (inferred):** the richest plane set of the Linux candidates, so it is the least like an MCU target; the vendor kernel is what was read, while the board runs mainline-style Armbian 6.18 (the two trees differ and mainline is unread); `platform-capabilities.md` has no RK3588 section yet (another plan is adding it), so tags there are not yet unified.

### 3.4 Broadcom BCM2712 (Pi 5) and BCM2711 (CM4)

- **Planes and composition:** BCM2712: two hardware planes on one output work, as the harness showed; the primary's zpos is immutable, so a z-swap needs two overlays; `TEST_ONLY` accepted 2x up and 1/2 down scaling and no limit was measured (observed, session dated 2026-11-27 and 2026-11-28; see [`platform-capabilities.md`](platform-capabilities.md) sections "BCM2712" and [`observed/bcm2712.md`](observed/bcm2712.md), [`observed/bcm2712-topology.md`](observed/bcm2712-topology.md)). BCM2711 (CM4 / reTerminal): two real planes on one output exist; **no KVM exists for this board and nothing captures its DSI panel**, so z-order and alpha on the glass are not verified ([`observed/bcm2711-cm4.md`](observed/bcm2711-cm4.md)). Failures of a load the HVS cannot sustain appear at run time, not in `TEST_ONLY` (source and inferred rows in the same section).
- **Remote-core or MCU path:** not recorded.
- **What the repository has:** observed and source files ([`observed/vc4-hvs-source.md`](observed/vc4-hvs-source.md), the `vc4-hvs-*-source.md` family, [`vc4-mainline-source.md`](vc4-mainline-source.md), [`bcm2712-dt-source.md`](bcm2712-dt-source.md)), a design note ([`hvs-backend-design.md`](hvs-backend-design.md)), profiles `Vc4Hvs5` and `Vc4Hvs6`, the procedure and results in [`rk3588-vc4-board-procedure.md`](rk3588-vc4-board-procedure.md) and [`rk3588-vc4-profiles.md`](rk3588-vc4-profiles.md).
- **Risks (inferred):** both are Linux targets with a display list rather than a fixed plane pipeline, so the "plane" model is the engine's mapping; the CM4 has no photographic check; HVS5 and HVS6 report the same driver name, so the profile needs an explicit variant ([`rk3588-vc4-profiles.md`](rk3588-vc4-profiles.md)).

### 3.5 NXP i.MX8M Plus (LCDIFv3)

- **Planes and composition:** one primary plane per CRTC; no overlay, no cursor; no scaling; zpos, alpha and blend properties absent or immutable. Tags: read from source in [`platform-capabilities.md`](platform-capabilities.md) section "NXP i.MX8M Plus" and [`imx8mp-lcdif-source.md`](imx8mp-lcdif-source.md); observed 2026-11-28 on one read-only probe of an Armadillo-X2 (two CRTCs, two primary planes, zpos immutable `0..=0`, no commit made) in [`imx8mp-x2-board.md`](imx8mp-x2-board.md). The planner therefore has nothing to assign to an overlay; all but one layer fall back to composition into the primary frame (inferred).
- **Remote-core or MCU path:** yes in the SoW's text: "Route 3" is an M7 software blend into an A53-produced frame before LCDIF scan-out. The SoW says this "is not equivalent to an additional hardware overlay plane" ([`SoW.md`](../SoW.md) §5.3). The repository's design note on its interface and buffer ownership is [`imx8mp-route3-precomposition.md`](imx8mp-route3-precomposition.md) (inferred, not implemented, no timing claim). The SoW names `platform/fw_imx8mp/` of the reference repository as material; that code is not in this repository.
- **What the repository has:** the two docs above, the board probe files under [`observed/imx8mp-x2-2026-11-28/`](observed/imx8mp-x2-2026-11-28/README.md), [`imx8mp-x2-board.md`](imx8mp-x2-board.md). No `DriverProfile` for it and no commit-level observation.
- **Risks (inferred):** it does not offer a hardware overlay, so a hardware-overlay milestone cannot be counted on it; Route 3 is a different design (pre-composition) that the SoW says not to count as an overlay implementation; the vendor CRTC, DT and `gpu2d` paths are unread.

## 4. Proposed decision criteria

These are proposals for the decision, not results. Each is phrased so that it can be answered from the repository or from one short session.

| # | Criterion | Why it matters |
| --- | --- | --- |
| C1 | **Scope:** does the third target have to be an MCU (SoW §8 WP4 names RT1170 or RA8P1), or may a Linux DRM SoC count as the "third target"? | The SoW names two MCUs; the repository's board evidence is for Linux SoCs. Answer first. |
| C2 | **Board availability:** is a board physically in the farm, with its resource id and a way to see the panel (camera or KVM)? | §8 WP4 says "subject to board availability". Nothing in this repository records a board for RT1170 or RA8P1. |
| C3 | **Display documented to the §5.4 level:** topology, plane/format/scaling/alpha limits, framebuffer memory, sync semantics, testable board setup. | The SoW's own entry condition. Count how many of the five items are already recorded, with tags. |
| C4 | **New information for the portability report:** does it add a plane model not already covered (single plane with pre-composition; fixed plane pipeline; display list; MCU with a 2D engine)? | WP4 asks which constraints are truly generic. A target that repeats an existing plane model adds little. |
| C5 | **Remote-core path:** does a firmware path exist (Zephyr or bare-metal) that can run the engine's C API like `tests/board/am62p_r5f`? | Keeps the third target in the same family as M2 and M3. |
| C6 | **Cost against M1 findings:** after M1, how much of the engine's open work (open-issues rows) is still unresolved? | SoW §13: optional until i.MX95 is demonstrated. Do not start if M1 findings need the effort. |
| C7 | **Evidence quality:** can the result be photographed, so it can be judged with `scripts/camera-check.py`, as for M3? | Several Linux records above state "no picture", which weakens them as evidence. |
| C8 | **Reuse of the harness:** is there a `DriverProfile` or harness support already, or must both be written? | Cheap candidates are those with existing code (section 3). |

A way to use them: answer C1 and C2 first; they can end the discussion. Then fill C3 to C8 per candidate with a tag for each answer, and record the choice, or "none", in `open-issues.md` M3-06 as that row asks.

## 5. What this note does not settle

- The decision itself. It waits for M1.
- Whether any RT1170 or RA8P1 board is available. Not recorded.
- Anything about the RT1170, RA8P1 or STM32H7 hardware. The repository holds only their names.
- Remote-core paths for the Linux SoCs other than i.MX8M Plus. The repository documents read for this note do not record them, which is not a statement that none exists.

## 6. Sources in this repository

[`SoW.md`](../SoW.md) §5.3, §5.4, §8 WP4, §13; [`open-issues.md`](open-issues.md); [`sow-traceability.md`](sow-traceability.md); [`m3-am62p-test-plan.md`](m3-am62p-test-plan.md); [`platform-capabilities.md`](platform-capabilities.md) (read only); the platform and observed documents linked in section 3.
