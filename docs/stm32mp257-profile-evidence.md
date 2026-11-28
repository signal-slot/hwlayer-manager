# STM32MP257F-DK: evidence ledger for the `DriverProfile::Stm32Ltdc` claims

One row per claim made by the `Stm32Ltdc` arm of `DriverProfile::apply` and
its doc comment in `crates/composition-drm/src/profile.rs`, plus the things
the profile says it does **not** model. Each row gives the evidence, its tag
and the test that pins it. A later item uses this table to change the profile
only where a session C picture supports it.

This is **portability evidence only**. It counts toward neither M1, M2 nor M3.
This file records no new observation: every date below is the date of the
cited document, which this file only points at.

## Tags and sources

- **observed** with a date (2026-11-27 or 2026-11-28): seen on the board (`TEST_ONLY`, CRC, debugfs or
  `modetest`) and recorded in the cited document. Dates are host UTC dates.
  Kernel `6.6.129`.
- **read from source**: stated by ST's kernel tree, as read in
  [`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md),
  [`stm32mp257-ltdc-source-diff-6.6.md`](stm32mp257-ltdc-source-diff-6.6.md),
  [`stm32mp257-ltdc-transform-source.md`](stm32mp257-ltdc-transform-source.md)
  and [`stm32mp257-ltdc-yuv-source.md`](stm32mp257-ltdc-yuv-source.md).
- **inferred**: concluded, neither seen nor stated.
- **not yet supported**: the profile makes no claim and nothing was seen on a
  picture. The planner then follows what KMS reports.
- Tests are in `crates/composition-drm/tests/profile.rs` (`ltdc_*`), except
  those marked `src`, which are in `crates/composition-drm/src/profile.rs`.
- Last column: **Q1** pixel colours (twoplane, both `--swap-z` phases,
  `--plane-alpha 32768`); **Q6** transforms on planes 34, 42, 49; **NV12**
  colours on plane 42; **P34** plane-34 scaling. These session C pictures are
  still "not run, no picture" because the KVM capture failed
  ([`stm32mp257-ltdc-session-c-2026-11-28.md`](stm32mp257-ltdc-session-c-2026-11-28.md)).
  "no" means a picture cannot decide the row.

Short names: S = [`stm32mp257-ltdc-session-2026-11-28.md`](stm32mp257-ltdc-session-2026-11-28.md)
(sessions A and B, run 2026-11-27); SC = [`stm32mp257-ltdc-session-c-outputs-2026-11-28.md`](stm32mp257-ltdc-session-c-outputs-2026-11-28.md);
OBS = [`observed/stm32mp257.md`](observed/stm32mp257.md);
BOARD = [`stm32mp257-ltdc-board.md`](stm32mp257-ltdc-board.md).

## Claims the profile makes

| # | Claim in `profile.rs` | Evidence | Tag | Pinning test | Picture could decide? |
|---|---|---|---|---|---|
| 1 | Planes are matched by kind and overlay order, not by KMS object id | Ids 34 < 42 < 49 follow DRM's layer order; the rule is a design choice | inferred (ids observed 2026-11-27, OBS) | `ltdc_cursor_and_later_overlays_are_unclaimed`, `ltdc_zero_and_one_plane` | no |
| 2 | Primary plane (34) scales | `TEST_ONLY` accepted every upscale up to the one-pixel source on plane 34 (S Q3; BOARD "Scaling bounds"). Scaling is a per-layer register bit `LXCR_C1R_SCA` | observed 2026-11-27; register nature read from source | `ltdc_limits_on_the_scaling_planes_only` | P34: quality only; acceptance is already seen |
| 3 | First overlay (42) scales | `scale_probe` on plane 42: no upscale rejected (S Q3; BOARD). CRCs of scaled commits were taken (S Q3) | observed 2026-11-27 | `ltdc_limits_on_the_scaling_planes_only` | no |
| 4 | Second and later overlays (49) do not scale (left unclaimed) | Plane 49 accepts only 1.0; every other factor gave EINVAL on h, v and both (S Q3; BOARD) | observed 2026-11-27 | `ltdc_cursor_and_later_overlays_are_unclaimed` | no |
| 5 | Which layers scale is a register fact, not in the source; layers 0 and 1 is concluded from one board | No per-variant table in the source; the driver reads `SCA` per layer at probe (SC Q4) | read from source (no table); inferred (layers 0 and 1) | `ltdc_limits_on_the_scaling_planes_only` | no; another STM32MP25 board would be the check |
| 6 | The cursor plane never scales | The board has no cursor plane, so this is a default | inferred | `ltdc_cursor_and_later_overlays_are_unclaimed` | no |
| 7 | `max_downscale` 1.0 (upscale only) | "Downscaling is not supported", `ltdc.c:1585-1600`; 0.5, 0.75 and 0.9 rejected (EINVAL) on all three planes (S Q3) | read from source; observed 2026-11-27 | `ltdc_downscale_is_refused_by_the_planner` | no |
| 8 | `max_upscale` 1080.0 (`1080 << 16`) | No upscale rejected up to the mode size (1920 h, 1080 v and both); 1080 is the largest factor seen accepted on every axis. The source has no numeric cap (factor masked to 16 bits, `ltdc.c:1609`) | observed 2026-11-27 (S Q3); read from source (no cap) | `ltdc_upscale_at_the_bound_accepted_and_above_refused` | no (larger factors need a sub-pixel source) |
| 9 | Straight alpha: `BlendMode::Coverage` on every non-cursor plane, replacing the reported premultiplied | Output CRC on plane 42 (AR24) matches colour x pixel alpha x plane alpha in all four discriminating cases (S Q2). KMS has no `pixel blend mode` property, so discovery reports premultiplied | observed 2026-11-27 (plane 42) | `ltdc_blends_straight_alpha_on_every_non_cursor_plane`, `ltdc_refuses_a_premultiplied_layer_and_takes_a_coverage_one` | Q1 can confirm or overturn: the CRC proof is indirect, and the predicted translucent colours (S Q1 table) are untested on a picture |
| 10 | Planes 34 and 49 blend straight like 42 | The same `BF1_PAXCA` blend factors are programmed for every layer (diff row 42). Not measured on 34 or 49 | read from source; inferred (not measured) | `ltdc_blends_straight_alpha_on_every_non_cursor_plane` (synthetic planes) | Q1 for the primary only (twoplane's background is plane 34); plane 49 is not in Q1 |
| 11 | Plane alpha is left as KMS reports it | KMS reports `alpha` 0..65535 on all three planes; plane alpha changes the CRC as predicted (S Q2, cases with plane alpha 0x8000) | observed 2026-11-27 | `ltdc_blends_straight_alpha_on_every_non_cursor_plane` (checks `plane_alpha` stays set) | Q1 (`--plane-alpha 32768`) |
| 12 | Transforms are left as KMS reports them; never claim 90/270 | KMS reports rotate 0/90/180/270 plus reflect x and y on plane 34, rotate 0 plus reflect x and y on 42 and 49 (OBS "Plane properties"). Source: reflection and the primary's 180 are programmed; 90/270 are refused or need secured rotation memory (transform source, sections 1 to 3). No transform was committed and viewed | observed 2026-11-27 (reported masks only); read from source (behaviour); picture not yet supported | `stm32_ltdc_adds_no_transform_claims` (`src`) | Q6 for each of planes 34, 42, 49 |
| 13 | Formats are left as KMS reports them | `IN_FORMATS`: plane 34 has 21 formats, 42 has 25, 49 has 17, all `LINEAR` (S Q9, probe) | observed 2026-11-27 | `ltdc_leaves_transforms_and_formats_as_reported` | no |
| 14 | Applying the profile twice gives the same result | Design property | inferred | `ltdc_applying_twice_is_idempotent` | no |
| 15 | Three planes on one CRTC: primary, overlay, overlay | `modetest` and debugfs state (OBS 2026-11-27; SC Q8 2026-11-28). The layer count is read from `LTDC_LCR`; a denied `l3` firewall access would drop layer 3 | observed 2026-11-27; read from source (`LTDC_LCR`, firewall) | `ltdc_limits_on_the_scaling_planes_only` (builds this topology) | no |

## What the profile says it does not model

| # | Statement | Evidence | Tag | Pinning test | Picture could decide? |
|---|---|---|---|---|---|
| 16 | YUV formats cannot be scaled on any plane; left to `TEST_ONLY` | Source: "Scaling is not supported with yuv pixel format" (diff row 34, `ltdc.c:1585-1600`). A scaled NV12 layer on plane 42 gave EINVAL in `TEST_ONLY` (S Q9) | read from source; observed 2026-11-27 | no test (not modelled) | no |
| 17 | NV12 on plane 42 at 1:1: no claim | `P` plus NV12 on 42 passes `TEST_ONLY`, commits, CRC `0x8d0f` stable, different from the XR24 overlay `0xd38f` (S Q9). **Colours are not answered**: the CRC cannot tell rounding from wrong colours | observed 2026-11-27 (acceptance, CRC); colours not yet supported | no test; `ltdc_leaves_transforms_and_formats_as_reported` covers "as reported" only | NV12; record `COLOR_ENCODING` and `COLOR_RANGE` (both BT.601 limited by default, S Q9) with the picture |
| 18 | NV12 on plane 49 | Not in its format list; `TEST_ONLY` gave EINVAL (S Q9) | observed 2026-11-27 | no test | no |
| 19 | C8 | Offered on all three planes; the CRTC has the 256-entry gamma LUT used as CLUT (diff row 53). Never committed | observed 2026-11-27 (offered); read from source (CLUT); use not yet supported | no test | no (not in session C) |
| 20 | `zpos`: no claim | KMS: `zpos` 0..2 on all three planes, initial values 0, 1, 2 (OBS). Crossed values commit; the plane with the lower value is below: `P` at 2 hid `O` at 0; overlays crossed put `O` over `S` (S Q5) | observed 2026-11-27 | no test (the profile adds no zpos claim) | Q1 `--swap-z` phases show it on screen; the CRCs already give the order |
| 21 | The 2048x2048 limit (`STM_MAX_FB_WIDTH`/`HEIGHT`) is a framebuffer limit; `max_source` is not set | Source (`drv.c`). A 3840x2160 XR24 buffer was refused (EINVAL) at framebuffer creation (S Q3 note) | read from source; observed 2026-11-27 (refusal) | no test | no |
| 22 | Scaling quality and underrun limits at large factors: not measured | `fifo_underrun_warning` rises with a scaled layer and only then; `fifo_underrun_error` and `transfer_error` stayed 0 under load (S Q3, Q7) | observed 2026-11-27 | no test | P34 for quality; not for the counters |
| 23 | Selection by driver name (`stm`) is not done | Design choice | inferred | no test | no |
| 24 | Outputs: no claim | One connector `HDMI-A-1`, one encoder, one CRTC; the LVDS route is disabled (SC Q8) | observed 2026-11-28 | no test | no |

## What a session C picture could change

| Picture item | Rows | Direction |
|---|---|---|
| Q1 pixel colours | 9, 10, 11, 20 | Confirms straight alpha on screen, or overturns row 9 (then `Coverage` would go back to the reported mode). Row 10 for the primary only |
| Q6 transforms on 34, 42, 49 | 12 | Could add a transform claim for a plane whose picture is right. 90/270 cannot be added on 42 and 49: KMS does not offer them |
| NV12 colours on plane 42 | 17 | Could confirm the BT.601 limited conversion; a wrong picture would call for refusing NV12 on plane 42 |
| Plane 34 scaling | 2, 22 | Quality only; acceptance is already observed |

Rows no picture can decide (1, 3 to 8, 13 to 16, 18, 19, 21, 23, 24) need
board `TEST_ONLY` runs, the reference manual (not read, SC Q4) or other
STM32MP25 boards.

## Gaps for the planner

- Rows 16 to 19, 21, 23 and 24 have no test. They are "does not claim" rows,
  so a test is needed only if the profile starts to claim them.
- Row 5 rests on one board; SC Q4 found no per-variant table.
- Row 12's test uses a synthetic topology where every plane reports rotate-0
  only. The real board reports more (plane 34 offers all four rotations), and
  no test builds the real masks.
