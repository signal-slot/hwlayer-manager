# STM32MP257 LTDC: the CRC readout as a pixel check that needs no picture, read from source

Date of writing: 2026-11-28. **Nothing here is a board result and no board was touched.** Every claim carries a tag: **read from source** (stated by the kernel code, with file and line) or **inferred** (a conclusion drawn from it, or from the generic DRM core). No claim is tagged as a measurement.

## Scope and a limit of this reading

The driver is `drivers/gpu/drm/stm/ltdc.c` of `STMicroelectronics/linux`, branch `v6.6-stm32mp`, commit `548f960c059bc4b9165cb69895fd67551f6061ca`
(<https://raw.githubusercontent.com/STMicroelectronics/linux/548f960c059bc4b9165cb69895fd67551f6061ca/drivers/gpu/drm/stm/ltdc.c>).
That is the commit the other LTDC source notes pin ([`stm32mp257-ltdc-source-full.md`](stm32mp257-ltdc-source-full.md), [`stm32mp257-ltdc-transform-source.md`](stm32mp257-ltdc-transform-source.md)). Line numbers are the lines of that file. The v6.18-stm32mp tree has the same plane path and its numbers are a few lines higher in the regions compared ([`stm32mp257-ltdc-source-diff-6.6.md`](stm32mp257-ltdc-source-diff-6.6.md)); no separate v6.18 reading was done for the CRC.

**Honest limit.** The first version of this note could not download the raw file and carried its CRC rows over from earlier notes. This revision (2026-11-28, host only, no board touched) re-read the bodies of `ltdc_crtc_set_crc_source`, `ltdc_crtc_verify_crc_source`, `ltdc_crtc_init`, `ltdc_irq`, `ltdc_irq_thread` and `ltdc_crtc_disable_vblank` in `ltdc.c` at the pinned commit, and the CRC debugfs code of **mainline v6.6** `drivers/gpu/drm/drm_debugfs_crc.c`. The limits of that reading:

- A first pass used a web-fetch helper that gave no line numbers and truncated the file. A second pass downloaded both raw files in full (`ltdc.c` at the pinned commit, `drm_debugfs_crc.c` of mainline `v6.6`) and took the line numbers below from `grep -n` on them. The rows now carry those numbers and the tag **read from source**. Rows whose numbers still come from earlier notes say "via" and were not re-checked; two of them disagree with the file (see section 4).
- `drm_debugfs_crc.c` was read from mainline `v6.6`, not from the ST tree's copy. The ST branch is `v6.6-stm32mp`, so the two should match, but the ST copy was not read. That one is still open.

"Read from source" below means read in that text; "inferred" is a conclusion from it.

## 1. What the driver implements

| Claim | Where | Tag |
| --- | --- | --- |
| `struct ltdc_caps` has a `crc` capability flag | `ltdc.h:19-42` (via `stm32mp257-ltdc-source-full.md` row 48) | read from source |
| The driver has a CRTC `set_crc_source` hook, `ltdc_crtc_set_crc_source`. With the source `"auto"` it sets `ldev->crc_active`, sets `IER_LIE` and sets `GCR_CRCEN`. No CRC region is programmed: the value is the controller's `LTDC_CCRCR` for the whole output. Every path ends with `ldev->crc_skip_count = 0` | `ltdc.c:1476-1502` (`"auto"` branch 1488-1491, `crc_skip_count = 0` at 1501) | read from source |
| The LTDC CRC register is `LTDC_CCRCR` (computed CRC register, defined at `ltdc.c:89`); the driver reads it as a single register value in `ltdc_irq` | `ltdc.c:1053-1054` | read from source |
| Sources accepted by `set_crc_source`: only `"auto"`. A `NULL` source clears `crc_active`, clears `IER_LIE` only if `vblank_active` is also false, and clears `GCR_CRCEN`. Any other string returns `-EINVAL`. A NULL `crtc` returns `-ENODEV` | `ltdc.c:1488-1501` (`NULL` branch 1492-1497, `-EINVAL` in the final `else`) | read from source |
| `verify_crc_source`: `ltdc.c` implements its own, `ltdc_crtc_verify_crc_source`. A non-NULL source other than `"auto"` returns `-EINVAL`; `"auto"` and `NULL` both pass, and it sets `*values_cnt = 1` | `ltdc.c:1505-1522` (rejection at 1512-1516) | read from source |
| Both hooks are set only in `ltdc_crtc_with_crc_support_funcs`; the plain `ltdc_crtc_funcs` has neither. `ltdc_crtc_init` uses the first when `ldev->caps.crc` is true, the second otherwise | `ltdc.c:1548-1558` (the CRC funcs struct; hooks at 1557-1558) and `2280-2282` (`ltdc_crtc_init`, which starts at 2235) | read from source |
| `IER_CRCIE` (`ltdc.c:193`) and `ISR_CRCIF` (`204`) are defined and `IER_CRCIE` is part of `IER_MASK` (`195`), but no other line of the file mentions it (`grep -n -i crc`); the CRC path uses only the line interrupt (`IER_LIE`, `ISR_LIF`) | `ltdc.c:193-204` and the whole-file grep | read from source |
| `ltdc_crtc_atomic_enable` (starts 1063) and `ltdc_crtc_atomic_disable` (starts 1282) contain no CRC identifier (the grep lists no line of them), so they do not touch `GCR_CRCEN` or `crc_active`. A modeset while `crc/data` is open therefore does not switch the CRC off in the driver | `ltdc.c:1063-1282` region, by the grep | inferred (absence of the identifiers; the function bodies were not read line by line for indirect effects) |
| Readout is in the interrupt path: `ltdc_irq` reads `LTDC_CCRCR` with `readl_relaxed` (not regmap) into `ldev->crc` on every interrupt while `crc_active` is set. `ltdc_irq_thread`, only for `ISR_LIF`, skips the first `CRC_SKIP_FRAMES` (2) entries after each `set_crc_source`, then calls `drm_crtc_add_crc_entry(crtc, true, drm_crtc_accurate_vblank_count(crtc), &ldev->crc)` | `ltdc.c:1053-1054` (`ltdc_irq`), `1016-1023` (`ltdc_irq_thread`), `270-274` (`CRC_SKIP_FRAMES`) | read from source |
| The line interrupt is placed at `accum_act_h + 1`, one line after the end of the active area, so the value read belongs to the frame just shown | `ltdc.c:1041-1056` (via the transform note; not re-checked, and the line-interrupt code is not at those numbers in the file: `1053-1054` is the CRC read) | read from source (earlier reading); still open: the line-interrupt placement was not re-read |
| `ltdc_crtc_disable_vblank` clears `vblank_active` and clears `IER_LIE` unless `crc_active`, so the line interrupt keeps running while the CRC is on | `ltdc.c:1472` (the `!vblank_active && !crc_active` test); earlier `1463-1473` via [`stm32mp257-ltdc-sync-source.md`](stm32mp257-ltdc-sync-source.md) | read from source |
| An entry has one 32-bit value, not one per colour channel (`values_cnt = 1` in `verify_crc_source`, one `&ldev->crc` passed to the core) | the functions above | read from source |

## 2. How user space would read it

The debugfs files come from the DRM core (`drivers/gpu/drm/drm_debugfs_crc.c`), not from `ltdc.c`. This part was **read from mainline v6.6** of that file (the ST tree's copy was not read; see the limit above). Line numbers below are those of `drm_debugfs_crc.c` at `v6.6`: `drm_debugfs_crtc_crc_add` 368-372, `crc_control_write` 120-146, `crtc_crc_open` 195-245, `crtc_crc_release` 258-268, `LINE_LEN` 281, `crtc_crc_read` 284-341.

| Step | What | Tag |
| --- | --- | --- |
| 1 | `drm_debugfs_crtc_crc_add` creates `crc/control` and `crc/data` under `/sys/kernel/debug/dri/<n>/crtc-<m>/` only when **both** `set_crc_source` and `verify_crc_source` are set | read from source (mainline v6.6) |
| 2 | Writing the source name to `crc/control` calls `verify_crc_source`; on error the write returns its error and the stored source is unchanged; while `crc/data` is open it returns `-EBUSY`. A trailing newline is stripped. `set_crc_source` is not called from the control write | read from source (mainline v6.6) |
| 3 | Opening `crc/data` returns `-EIO` if the CRTC is not active, calls `verify_crc_source` again with the stored source (a NULL source passes in this driver), allows one opener (`-EBUSY` otherwise), then calls `set_crc_source`. A line read is `0x%08x` (frame counter, or `XXXXXXXXXX`) then ` 0x%08x` per value, then a newline | read from source (mainline v6.6) |
| 4 | Read blocks until the next entry (`-EAGAIN` if non-blocking; 0 bytes if no source is stored; `-EINVAL` if the buffer is shorter than a line). The core drops entries with a one-time error if user space reads too slowly. The driver skips 2 entries after each `set_crc_source` | read from source (core v6.6; `CRC_SKIP_FRAMES` in `ltdc.c`); the advice to skip a few more is inferred |
| 5 | Closing `crc/data` calls `set_crc_source(crtc, NULL)` (return value ignored), which turns the CRC off in this driver | read from source (core v6.6 and `ltdc.c`) |

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
| `ltdc_get_caps()` asks the firewall for `l3`; when access is refused it decrements `nb_layers` (layer 3 is not exposed) **and clears `caps.crc`**. A refusal of `cmn`, `l1l2` or `rot` fails the probe | `ltdc.c` caps code (via [`stm32mp257-ltdc-session-c-outputs-2026-11-28.md`](stm32mp257-ltdc-session-c-outputs-2026-11-28.md), Q4 table, row "layer count"); re-read: a refused `l3` sets `is_layer_secured` and decrements `nb_layers` (`ltdc.c:2458-2460`); any other refused entry releases access and returns the error (2461-2464). Then, for the compatibles `st,stm32mp21-ltdc` and `st,stm32mp25-ltdc`, `caps.crc` is set false when `is_layer_secured` (2586-2592, comment at 2588, assignment at 2590) | read from source |
| Per hardware version, `caps.crc` is false for `HWVER_10200` (case at 2479, assignment 2500), `HWVER_10300` (2506, 2526) and `HWVER_20101` (2532, 2546), and true for `HWVER_40100` and `HWVER_40101` (2552-2553, assignment 2566) | `ltdc.c:2479-2566` | read from source |
| **Correction.** The earlier note cited `ltdc.c:2595-2600` for the CRC enable and said the compatible strings differ between the 6.6 and 6.18 branches (`st,stm32mp251-ltdc`, `st,stm32mp255-ltdc` against `st,stm32mp21-ltdc`, `st,stm32mp25-ltdc`). At the pinned commit the test at `2586-2587` uses `st,stm32mp21-ltdc` and `st,stm32mp25-ltdc`, and the block is 2586-2592, not 2595-2600. The `mp251`/`mp255` strings do not appear in this reading; the earlier claim is not confirmed, and the diff note's row 26 should be checked against it | `ltdc.c:2586-2592` | read from source (the correction); the earlier branch-difference claim is still open |
| So a boot chain that denies `l3` removes both layer 3 (plane 49) and the CRC. The board's three planes (34, 42, 49) mean `l3` was granted when the Q4 note was written, so the CRC capability is on in that configuration | Q4 note, firewall rows | inferred (the grant is from that note's reading of the plane list) |
| Without `caps.crc` the driver registers `ltdc_crtc_funcs` with neither hook, and the core then creates no `crc/` files. The check "does `crc/control` exist" therefore shows whether `caps.crc` is set (hardware version and firewall together) | `ltdc.c:2280-2282`; core `drm_debugfs_crc.c:368-372` | read from source |
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
