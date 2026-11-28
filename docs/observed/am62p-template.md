# Verdin AM62P: observed (TEMPLATE)

**This file is a template, not an observation.** It records no value. It was
written from `docs/am62p-imx95-matrix.md` as a skeleton and is source and
desk work only; no device was used.

How to fill it (item `am62p-observed-matrix`):

1. Copy this file to `docs/observed/am62p.md`. Keep this template unchanged.
2. Fill a cell only from the board run, and cite the section of
   `docs/m3-am62p-report.md` that holds the evidence. Replace
   `<observed (YYYY-MM-DD)>` with the tag `observed` and the date of the run,
   in the form the other `docs/observed/` files use, and replace the report
   section placeholder.
3. A row the run did not measure is not deleted: write `not observed` and the
   reason. Do not carry over a `source` or `SoW` value from the matrix as an
   observation. Where a line is a conclusion, tag it `inferred`.
4. Keep board addresses, credentials and session identifiers out of the file.
5. Delete this note and the "TEMPLATE" word in the title and in the intro.

Placeholders: `<value>` is the observed value; `<observed (YYYY-MM-DD)>` is
the tag and date; `<report §name>` is the section of
`docs/m3-am62p-report.md` (Date, Board, BSP, Kernel, Panel, Firmware image,
Console (trace buffer), Two-plane composition, Commits vs latched (GO bit),
Releases, Refused scenes, Photographs and camera-check, Fixes and repeat
runs, Restore).

## Session

| Item | Value | Tag | Report |
|---|---|---|---|
| Date of the run | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Date>` |
| Board (module, carrier, revision, silicon revision) | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Board>` |
| BSP / image (Torizon OS version, build) | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §BSP>` |
| Kernel (`uname -a`) | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Kernel>` |
| Panel (model, mode, connector, DSS route) | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Panel>` |
| Firmware image (R5F test image, how installed) | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Firmware image>` |
| Console (remoteproc trace buffer) | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Console (trace buffer)>` |

## 1. Linux DRM side

One row per row of section 1 of the matrix. This file is the AM62P column
only; the i.MX95 column is not copied.

| Item | AM62P | Tag | Report |
|---|---|---|---|
| Display controller | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Board>` |
| CRTCs / outputs | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Panel>` |
| Planes (count, type) | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Two-plane composition>` |
| Plane → CRTC | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Two-plane composition>` |
| Formats | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Two-plane composition>` |
| Scaling | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Refused scenes>` |
| Blending / alpha | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Photographs and camera-check>` |
| Z-order | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Photographs and camera-check>` |
| Rotation | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Refused scenes>` |
| Constraints on a commit | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Refused scenes>` |
| Hardware behind planes | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Two-plane composition>` |
| Commit / latch model (Linux) | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Commits vs latched (GO bit)>` |
| VSYNC reporting (Linux) | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Commits vs latched (GO bit)>` |
| Clocks and power | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Board>` |
| Sharing between two engines on one SoC | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Two-plane composition>` |
| Engine backend | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Two-plane composition>` |

## 2. MCU / Zephyr side (R5F)

One row per row of section 2 of the matrix.

| Item | AM62P (R5F) | Tag | Report |
|---|---|---|---|
| Core that drives the display | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Firmware image>` |
| Zephyr display driver | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Firmware image>` |
| Planes and overlay | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Two-plane composition>` |
| Formats, scaling, alpha | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Photographs and camera-check>` |
| Commit / latch model | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Commits vs latched (GO bit)>` |
| VSYNC reporting | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Commits vs latched (GO bit)>` |
| Resource sharing between cores | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Two-plane composition>` |
| Ownership mechanism | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Firmware image>` |
| Memory and caches | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Fixes and repeat runs>` |
| Failure behaviour | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Releases>` |
| Engine backend | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Two-plane composition>` |

### `backends/twoplane` on the AM62P

One row per row of the matrix's per-SoC table.

| Item | AM62P | Tag | Report |
|---|---|---|---|
| `latched_count` follows | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Commits vs latched (GO bit)>` |
| Driver file in the demo | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Firmware image>` |
| Compiles for Arm / links | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Firmware image>` |
| Board test | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Two-plane composition>` |

## 3. Lease and Weston coexistence

One row per row of section 3 of the matrix.

| Item | AM62P | Tag | Report |
|---|---|---|---|
| Linux DRM lease use | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Panel>` |
| Weston on the board | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Restore>` |
| Weston lease support | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Panel>` |
| Coexistence in practice | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Two-plane composition>` |
| Revocation | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §Releases>` |

## Rows added by the board run

The run may show rows the matrix does not have (for example commits against
the latched count per scene). Add them here with the same columns; the matrix
item decides whether the matrix gets them too.

| Item | AM62P | Tag | Report |
|---|---|---|---|
| `<row>` | `<value>` | `<observed (YYYY-MM-DD)>` | `<report §name>` |
