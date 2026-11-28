# STM32MP257 LTDC: the CRC readout as a pixel check that needs no picture, read from source

Date of writing: 2026-11-28. **Nothing here is a board result and no board was touched.** Every claim carries a tag: **read from source** (stated by the kernel code, with file and line) or **inferred** (a conclusion drawn from it, or from the generic DRM core). No claim is tagged as a measurement.

## Scope and a limit of this reading

The driver is `drivers/gpu/drm/stm/ltdc.c` of `STMicroelectronics/linux`, branch `v6.6-stm32mp`, commit `548f960c059bc4b9165cb69895fd67551f6061ca`
(<https://raw.githubusercontent.com/STMicroelectronics/linux/548f960c059bc4b9165cb69895fd67551f6061ca/drivers/gpu/drm/stm/ltdc.c>).
That is the commit the other LTDC source notes pin ([`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md), [`stm32mp257-ltdc-transform-source.md`](stm32mp257-ltdc-transform-source.md)). Line numbers are the lines of that file. The v6.18-stm32mp tree has the same plane path and its numbers are a few lines higher in the regions compared ([`stm32mp257-ltdc-source-diff-6.6.md`](stm32mp257-ltdc-source-diff-6.6.md)); no separate v6.18 reading was done for the CRC.

**Honest limit.** In the session that wrote this note the raw file could not be downloaded (the sandbox refused network commands, and a fetch through a helper returned HTTP 429). So the CRC rows below are **read from source in the earlier readings, carried over with their line numbers** from the notes named in the "Where" column, not re-read here. The bodies of `set_crc_source` and `verify_crc_source` were **not** re-read; rows that depend on them say so. A later session with network access should check the rows marked "to re-read" against the file.

## 1. What the driver implements

| Claim | Where | Tag |
| --- | --- | --- |
| `struct ltdc_caps` has a `crc` capability flag | `ltdc.h:19-42` (via `stm32mp257-ltdc-source-full.md` row 48) | read from source |
| The driver has a CRTC `set_crc_source` hook; with the source `"auto"` it sets `IER_LIE` and `GCR_CRCEN` and nothing else. There is no CRC region: the value is the controller's `LTDC_CCRCR` for the whole output | `ltdc.c:1488-1491` (via the transform note, section 4) | read from source |
| The LTDC CRC register is `LTDC_CCRCR` (computed CRC register); the driver reads it as a single register value | `ltdc.c:1041-1056` (via the transform note) | read from source |
| Sources accepted: `"auto"` is the one the earlier note names. Whether any other string is accepted, and what a `NULL` source does (switch the CRC off), is in the hook body | `ltdc.c:1488-1491`, body to re-read | inferred (that `"auto"` is the only name); to re-read |
| `verify_crc_source`: whether `ltdc.c` implements its own hook or leaves it to the DRM core is not in the notes this reading used | to re-read | inferred (open) |
| Readout is in the interrupt path: `ltdc_irq` reads `LTDC_CCRCR` in the hard interrupt for every interrupt while the CRC is active; the threaded handler reports it with `drm_crtc_add_crc_entry` only for a line interrupt, after skipping 2 frames. Fifo and transfer interrupts also re-read `ldev->crc` | `ltdc.c:1041-1056`, `1005-1025`, `1209` (via the transform note) | read from source |
| The line interrupt is placed at `accum_act_h + 1`, one line after the end of the active area, so the value read belongs to the frame just shown | `ltdc.c:1041-1056` (via the transform note) | read from source |
| `disable_vblank` clears `IER_LIE` unless the CRC source is active, so the line interrupt keeps running while the CRC is on | `ltdc.c:1463-1473` (via [`stm32mp257-ltdc-sync-source.md`](stm32mp257-ltdc-sync-source.md) row at line 38) | read from source |
| The driver reports one 32-bit value per frame, so an entry has one value, not one per colour channel | derived from the single `LTDC_CCRCR` read above | inferred |

## 2. How user space would read it

The debugfs files come from the DRM core (`drivers/gpu/drm/drm_debugfs_crc.c`), not from `ltdc.c`. This part is **inferred** from the generic DRM interface, not read in the ST tree in this session.

| Step | What | Tag |
| --- | --- | --- |
| 1 | The core creates `/sys/kernel/debug/dri/<n>/crtc-<m>/crc/control` and `crc/data` for a CRTC whose funcs have `set_crc_source` | inferred |
| 2 | Write the source name (`auto`) to `crc/control`; the core asks the driver to check it (`verify_crc_source`) | inferred |
| 3 | Open `crc/data`. The core calls `set_crc_source` when the file is opened, and the driver turns the CRC on. Each line read is a frame counter followed by the CRC value(s) | inferred |
| 4 | Read blocks until the next entry; the first entries after a change may belong to the old picture, so skip a few (the driver itself skips 2 frames) | inferred, with the skip count read from source (`ltdc.c:1041-1056`) |
| 5 | Closing `crc/data` turns the CRC off in the driver | inferred |

The earlier board sessions already used this path ([`stm32mp257-ltdc-session-2026-11-28.md`](stm32mp257-ltdc-session-2026-11-28.md), Q2: `auto` written to `crtc-0/crc/control`, `crc/data` opened, three entries skipped, two read). This note only points at that record; it does not repeat it as evidence.

## 3. What the CRC covers

| Claim | Where | Tag |
| --- | --- | --- |
| The CRC is computed by the LTDC for the whole output, with no region, so it covers all layers blended and the background | `ltdc.c:1488-1491` (via the transform note) | read from source |
| It is the controller's own register, so it is taken at the LTDC output, before the DSI host, the DSI-to-HDMI bridge and the HDMI connector; none of those can change it, and none of their faults can show in it | the register belongs to the LTDC block | inferred |
| So the CRC cannot detect a fault after the LTDC: a bridge that mangles the picture, a wrong HDMI mode on the monitor side, a dead cable | same | inferred |
| A change in plane geometry that the LTDC performs (scaling, mirroring, alpha) does change the CRC, because the CRC is taken after those stages | `ltdc.c:1979-2005` (scaler), `1975-1977` (HMEN) (via the transform note) | inferred |
| The value is not a plain CRC of the pixel stream with colour channels kept apart. Session A's references show that colours with the same R xor G xor B gave the same value, and 256 grey-ramp references gave 256 different values. This is a session note's own inference, restated here; it is not in `ltdc.c` | `stm32mp257-ltdc-session-2026-11-28.md`, Q2 method | inferred |

## 4. Limits: the RIF firewall

| Claim | Where | Tag |
| --- | --- | --- |
| `ltdc_get_caps()` asks the firewall for `l3`; when access is refused it decrements `nb_layers` (layer 3 is not exposed) **and clears `caps.crc`**. A refusal of `cmn`, `l1l2` or `rot` fails the probe | `ltdc.c` caps code (via [`stm32mp257-ltdc-session-c-outputs-2026-11-28.md`](stm32mp257-ltdc-session-c-outputs-2026-11-28.md), Q4 table, row "layer count"); lines to re-read | read from source |
| The CRC enable for MP251 and MP255 is the last statement of the 4.x hardware-version case | `ltdc.c:2595-2600` (via the sync note) | read from source |
| The compatible string for that test differs between the 6.6 and 6.18 branches (`st,stm32mp251-ltdc`, `st,stm32mp255-ltdc` against `st,stm32mp21-ltdc`, `st,stm32mp25-ltdc`); same effect on an MP25 | `ltdc.c:2595-2596` and the diff note, row 26 | read from source |
| So a boot chain that denies `l3` removes both layer 3 (plane 49) and the CRC. The board's three planes (34, 42, 49) mean `l3` was granted when the Q4 note was written, so the CRC capability is on in that configuration | Q4 note, firewall rows | inferred (the grant is from that note's reading of the plane list) |
| Without `caps.crc` the driver would not register `set_crc_source` for the CRTC, so `crc/control` would be missing. The check "does `crc/control` exist" therefore shows whether the firewall left the CRC on | the use of `caps.crc` to register the hook is not in the rows read | inferred (to re-read) |
| A different boot chain or another board variant can have a different RIF setting for `l3`; the device tree is the same for every variant | Q4 note | inferred |

## 5. Can a CRC tell the expected pictures apart?

A CRC does not say what a picture looks like. It says whether two pictures are equal. So there are two ways to use it.

| Method | What it needs | Strength | Tag |
| --- | --- | --- | --- |
| Reference CRC: compare to a value stored from a known-good picture | A known-good run on the same hardware and tree. The only source is an earlier run of the same program, so it detects change, not correctness | Detects regressions; cannot say that the quadrants are the right way round | inferred |
| Same frame twice: two readings of one static picture are equal | Nothing | The LTDC adds no change of its own to a static frame, but session A's note records a frame-to-frame changing CRC with reflect-x on the primary, so this check can fail for a reason that is not the picture | inferred (the source gives no cause; see [`stm32mp257-ltdc-transform-source.md`](stm32mp257-ltdc-transform-source.md) section 4) |
| Transform against identity: the same buffer unrotated and transformed must give different CRCs; reflect-x and reflect-y must differ from each other and from 180 | A buffer that is not symmetric: distinct quadrant colours with distinct R xor G xor B, and no mirror symmetry in x or y | A transform that the hardware ignores gives the same CRC as the identity and is caught. A transform that is applied gives a different one, but not which one | inferred |
| Transform twice: reflect-x applied with a horizontally mirrored source buffer must equal the unreflected original | A host-made mirrored buffer of the same picture | Checks the direction of the mirror, not only that something changed, because the CRC of the host-mirrored original is the expected value | inferred |
| Predict the value | Compute the CRC on the host | Not possible from the source alone: the polynomial, seed and byte order of `LTDC_CCRCR` are in the reference manual, not in `ltdc.c` | inferred |

What this means for the open session C questions:

- **Q6 transforms (planes 34, 42, 49).** The "transform against identity" and "transform twice" methods can answer *is it applied* and *which way*, using one buffer and its host-mirrored twin, for reflect-x and reflect-y on each plane. 180 is the same programming as reflect-x plus reflect-y on an overlay, so it can be checked the same way (transform note, section 2). 90 and 270 are not testable: the overlay property refuses them and a committed 90 or 270 on the primary risks a null `rot_mem` dereference (transform note, section 3).
- **Quadrant orientation (Q1).** A buffer whose quadrants differ in R xor G xor B gives different CRCs for different arrangements; the arrangement is known only against a reference run. The unrotated frame is the natural reference.
- **NV12 colours (plane 42).** The CRC cannot say which colour appeared, only that two encodings differ. COLOR_ENCODING and COLOR_RANGE changes should change it, but whether the result is BT.601 or BT.709 is not decidable without a host model of the LTDC conversion.
- **Plane-34 scaling.** The session A scaled-picture run already shows the CRC reacts to scaling, and that it matched none of the simple references. The CRC cannot grade the scaler filter (session A, scaled commits). 
- **Not covered at all:** anything after the LTDC (section 3), and the question of whether the picture on a monitor is correct.

## Possible board use

**This section is a proposal, not a procedure, and nothing in it was run.** Writing `auto` to debugfs `crc/control` is a board-side action. Under the device rules (no change to DRM configuration) it needs the owner's approval before any session does it, even though earlier sessions did it and removed it again. The same applies to opening `crc/data`. The proposal:

1. The owner decides whether a debugfs write for the CRC is allowed in the next STM32MP257 session, if the KVM capture still fails.
2. If yes, a session first checks that `crc/control` exists (section 4: its absence would mean the firewall removed the CRC), and states which kernel the board runs against the commit pinned here.
3. For each Q6 transform the session would take: the identity CRC, the transformed CRC, and the CRC of the host-mirrored buffer, each as two equal readings (section 5). It would record "not stable" instead of a verdict when two readings of a static frame differ, because of the primary reflect-x note.
4. The verdicts would be written in the session file as CRC-based "applied / direction" results, never as colour or quadrant results, and with the Q1 colours, NV12 colours and scaling quality left as "not run, no picture".

## Related

- [`stm32mp257-ltdc-session-c-2026-11-28.md`](stm32mp257-ltdc-session-c-2026-11-28.md): session C, the picture items left as "not run, no picture".
- [`stm32mp257-ltdc-session-c-outputs-2026-11-28.md`](stm32mp257-ltdc-session-c-outputs-2026-11-28.md): Q8 and Q4, including the firewall rows used in section 4.
- [`stm32mp257-session-c-runbook.md`](stm32mp257-session-c-runbook.md): the picture commands this note would stand in for.
- [`stm32mp257-ltdc-transform-source.md`](stm32mp257-ltdc-transform-source.md): transform programming and the changing-CRC question.
- [`stm32mp257-ltdc-session-2026-11-28.md`](stm32mp257-ltdc-session-2026-11-28.md): sessions A and B, where the CRC path was used.
