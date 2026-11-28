# vc4 source rows against the RPi boards' running kernels (HVS Q7)

Written 2026-11-28 (host UTC) on the host. No board was visited. This page
answers question 7 of [hvs-backend-design.md](hvs-backend-design.md) §7
("which kernel"): do the vc4 source rows, read at `rpi-6.18.y` commit
`16ae2649dcfadf9d81445b1177a3191efa3cb82e`, match what the Raspberry Pi 5
(BCM2712, HVS6) and the reTerminal CM4 (BCM2711, HVS5) actually run?

Short answer: **no, not row for row.** The pin was made on 2026-11-26. The
Pi 5's package was built on 2026-09-11, and about twenty `vc4_plane.c`
commits (most of them on 2026-11-17) sit between the two. The CM4 runs 6.12
and its package is from 2025-09-16, with six `vc4_plane.c` commits between it
and the 6.12 tip that the source docs read. The source rows that cite the
pin for HVS6 are therefore **not** safe claims about the Pi 5 where one of
those commits touches the row. Two differences are also confirmed by the
boards' own records (rows R15 and C7).

Tags: **read from source** = a statement about the history of a path at a
named branch, taken from the GitHub commits list (see "How this was read");
**observed** = a dated board fact copied from `docs/observed/`; **inferred** =
a comparison or mapping drawn by this page. Verdict words: **same**,
**differs**, **not checked**, and **still open** (the row cannot be settled
here, reason given).

## Running kernels

| Board | Kernel string | Tag |
| --- | --- | --- |
| Raspberry Pi 5 (BCM2712, HVS6) | `6.18.50+rpt-rpi-2712 #1 SMP PREEMPT Debian 1:6.18.50-1+rpt1 (2026-09-11) aarch64` | observed 2026-11-28 ([observed/bcm2712.md](observed/bcm2712.md), [observed/bcm2712-topology.md](observed/bcm2712-topology.md), [observed/bcm2712-recheck.md](observed/bcm2712-recheck.md)) |
| reTerminal CM4 (BCM2711, HVS5) | `6.12.47+rpt-rpi-v8 #1 SMP PREEMPT Debian 1:6.12.47-1+rpt1 (2025-09-16) aarch64` | observed 2026-11-28 ([observed/bcm2711-cm4.md](observed/bcm2711-cm4.md), [observed/bcm2711-cm4-topology.md](observed/bcm2711-cm4-topology.md), [observed/bcm2711-cm4-writeback.md](observed/bcm2711-cm4-writeback.md)) |

`uname -r` and `uname -v` are in those records. No `docs/vc4-hvs-session-*.md`
had been merged when this page was written, so no session output was used.
The date in `uname -v` is the Debian package's date (inferred: the build date
of the kernel package). The exact `raspberrypi/linux` commit the package was
built from is **not** recorded anywhere; this page uses the date as an upper
bound (inferred): a commit committed after the package date cannot be in the
running kernel.

## How this was read

- Trees compared: the pin `rpi-6.18.y` @
  `16ae2649dcfadf9d81445b1177a3191efa3cb82e` (committer date 2026-11-26
  18:04 UTC, tree at
  https://github.com/raspberrypi/linux/tree/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4),
  and the `rpi-6.12.y` tip that the source docs read
  (`8f5f2e15840330091c7116b2ef9454108d4bc423`, 2026-11-23).
- **No tree was fetched.** The brief asked for a shallow sparse fetch into the
  dependency cache. Every `git fetch`, `git remote add`, `curl` and `gh`
  command was refused on this node, so there is no checkout of either branch
  and **no `vc4_plane.c` was diffed**. What was read instead is the commit
  history of single files, through the GitHub commits list (path filter, with
  `since` and `until`), 2026-11-28, via a web reader. The reader returned
  only part of the longer lists verbatim; the rest came as its own summary.
  Commit subjects and dates from the summarised part are as reliable as that
  summary. read from source (history of the path, by subject); no hunk was
  read.
- Consequence: a **differs** below means "a commit whose subject names this
  behaviour lies between the board's package date and the cited ref". A
  **same** means "no commit in that window touches the file", or, for
  `vc4_plane.c`, "no commit subject in the window concerns this behaviour"
  (inferred; the diff was not read). **not checked** means a subject might
  touch the row but is not specific enough to decide.
- Line numbers in the rows are those of the source docs, carried over. They
  were not re-counted here.
- Windows used (committer dates, UTC):
  - Pi 5, pin side: commits after 2026-09-11 up to the pin. `vc4_plane.c`: 24
    since 2026-09-01; the last one before 2026-09-12 is `11d29aa`
    (2026-06-09, "Skip input lines when doing a large downscale"). `vc4_hvs.c`
    (8 since 2026-09-01): suspend/resume, DMA device, min-clocks request,
    "Tabulate the PPF filter kernel", unbind fix; none about LBM or UPM sizes.
    `vc4_crtc.c` and `vc4_regs.h`: one commit, `aa4df0e` (2026-11-17,
    interlaced scanout position). `vc4_txp.c`: none. `vc4_drv.c`: 11, of which
    seven are firmware-KMS, suspend and DMA-device changes.
    `vc4_hdmi.c`: 15 (scrambling, audio, suspend, PHY power-down).
  - CM4, 6.12 side: commits after 2025-09-16 up to the 6.12 tip.
    `vc4_plane.c`: six, `f55958e` 2025-11-03 and `df1d52b` 2025-11-04 (swap
    Cb/Cr for YVU formats), `d04e379` and `a8c4c46` 2025-12-04 (dead
    `channels_scaled` code; "TPZ scaling modes cannot reduce lbm size when
    alpha-less"), `590178d` 2026-01-13 (async commit buffer address when
    vertically flipped), `80bdb25` 2026-03-04 ("Add support for RGB[AX]8888
    and BGR[AX]8888 formats"). The last one before 2025-09-17 is `c4d7426`
    (2025-07-16, P01x and Q01x formats). `vc4_hvs.c`: `8d376f1` (2025-09-16,
    GEN_6D YUV matrices; same day as the package, HVS6 only) and later
    unbind/min-clocks fixes.

## Rows: the Pi 5 (HVS6) against the 6.18 pin

Source docs: [vc4-scale-limits-source.md](vc4-scale-limits-source.md) (rows
S-n), [vc4-formats-source.md](vc4-formats-source.md) (F-n),
[vc4-transform-source.md](vc4-transform-source.md) (T-n),
[bcm2712-plane-count-source.md](bcm2712-plane-count-source.md) (P-n, its own
S-numbers). Only HVS6 rows are compared here; the HVS5 columns of the 6.18
pin have no board running them.

| Row | Source row (doc, function or line carried from it) | Verdict | Reason | Tag |
| --- | --- | --- | --- | --- |
| R1 | S: horizontal downscale -EINVAL beyond 31.99998x (TPZ check, `vc4_plane.c:686-690`) | differs | The check was added by `8c4f2d0` "Reject a horizontal downscale that overflows the TPZ scale", 2026-11-17, after the 2026-09-11 package. The running kernel most likely has no such refusal, i.e. it behaves like the 6.12 column of the source doc (accepted, truncated field). | read from source (history), inferred |
| R2 | S: vertical line skip and its -EINVAL (`vc4_plane_set_vdownsample`, `vc4_plane.c:1421-1452`, `:2087-2088`) | differs | The line skip itself exists before the package (`11d29aa`, 2026-06-09). Its limits come from `68d8e92` "Keep the vertical line skip within the hardware limits" and `1f086f0` "Multiply the chroma pitch by the line skip", both 2026-11-17, after it. The `max_tile_skip` / `max_pitch` refusal quoted from the pin may be absent or different on the Pi 5. | read from source (history), inferred |
| R3 | S: scaling mode per axis (`vc4_get_scaling_mode`, `vc4_plane.c:329-342`) | differs | `069d5c6` "Don't treat a fractional source as unscaled" (2026-11-17) concerns the `NONE` rule (`dst == src >> 16`). | read from source (history), inferred |
| R4 | S: no refusal other than a missing CRTC state and margins (`vc4_plane.c:593-621`) | differs | `7484579` "Reject YUV444 sources too wide for the doubled width" and `2798693` "Reject YUV sources smaller than a chroma sample" (both 2026-11-17) add refusals. | read from source (history) |
| R5 | S: upscale constants, TPZ upscale writes `(1<<16)+1` (`vc4_plane.c:707-713`) | differs | `00a8cba` "Only use the TPZ upscale for RGB at an integer position" (2026-11-17) changes when TPZ upscale is used. | read from source (history), inferred |
| R6 | S: scale field widths, 17-bit PPF and 21-bit TPZ (`vc4_regs.h:1333`, `:1348`) | same | `vc4_regs.h` has one commit in the window, `aa4df0e` (interlaced position bit); it does not name the scale fields. | read from source (history), inferred |
| R7 | S, F, T: source width on gen 6, no check; wider sources truncate (`vc4_plane.c:2289-2296`) | not checked | No subject names the generic width. `7484579` adds a YUV444 width refusal only. The diff was not read. | read from source (history) |
| R8 | S: HVS6 LBM capacity 1024 units, 64 handles (`vc4_hvs.c:1693-1711`, `vc4_drv.h:304`) | same | No `vc4_hvs.c` commit in the window concerns LBM size. `vc4_drv.h` was not listed, so the handle count is **not checked**. | read from source (history), inferred |
| R9 | S: LBM need, `__vc6_lbm_size` (`vc4_plane.c:931`) and the 240-of-1024 worked number | differs | `fee384e` (round up LBM width of subsampled channels), `76999cf` (LBM for YUV444 chroma), `0f9adcd` (size the LBM from the pixels actually fetched), `9cef3b4` (size the vc6 LBM for H-TPZ from the destination width), all 2026-11-17. | read from source (history) |
| R10 | S: LBM failure errors, `vc4_plane_allocate_lbm` (`vc4_plane.c:1142`, `:1159`) | same | No subject in the window concerns the allocator. | inferred |
| R11 | S: UPM allocation on every plane | same | No `vc4_hvs.c` or `vc4_plane.c` subject concerns UPM. | inferred |
| R12 | S: resulting boundary table for HVS6 at 6.18 | differs | It depends on R1, R2, R9. For this board the 6.12 column is the nearer guess for horizontal downscale (inferred); the board sweep of the planned downscale probe decides. | inferred |
| R13 | F: `hvs_formats[]` is 45 entries, XR24 ... S016 in table order (`vc4_plane.c:41-315`) | same | RGB[AX]8888 was added on this branch on 2026-04-20 (`268b9eb`), before the package. The board's list holds RX24 RA24 BX24 BA24 at positions 5 to 8 and the IN_FORMATS blob has 45 formats (observed 2026-11-28). No format commit follows the package. | read from source (history), observed 2026-11-28 |
| R14 | F: modifier array order and per-format rules (`vc4_plane.c:2858-2881`, `vc4_format_mod_supported:2761-2837`), except the gen-6 refusal | same | No subject in the window concerns it. | inferred |
| R15 | F, T: on gen >= 6 `T_TILED` and `SAND64` are refused for every format (`vc4_plane.c:2767-2771`); 49 pairs | differs | **Confirmed by the board.** `7f362e5` "hvs6: Don't advertise T-tiled or SAND64 modifiers" is dated 2026-11-17. The Pi 5's IN_FORMATS (observed 2026-11-28) lists `0x0700000000000001` T-tiled for XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 and code 3 for NV12 NV21, in addition to codes 4 (NV12 NV21 P030) and 5 (NV12 NV21): 44 linear + 8 + 2 + 3 + 2 = 59 pairs, not 49. The recorded doc matches code 3 with SAND128 from memory; the source docs' blob order (SAND64 = 3, SAND128 = 4, SAND256 = 5) says code 3 is SAND64 (inferred). | observed 2026-11-28, read from source (history), inferred |
| R16 | T: rotation mask 0, 180, reflect-x, reflect-y on every plane (`vc4_plane_init`, `vc4_plane.c:2896-2900`) | same | Observed 2026-11-28 on the Pi 5: rotate-0, 180, reflect-x, reflect-y, no 90/270 ([observed/bcm2712-topology.md](observed/bcm2712-topology.md)). No window commit names the mask. | observed 2026-11-28 |
| R17 | T: reflect-x and reflect-y programming on gen 6 (`vc6_plane_mode_set`, `vc4_plane.c:2111-2113`, `:2271`, `:2314`) | not checked | `b0b7d20` "Keep the source offset for an async page flip" and `68d8e92` (line skip) touch code next to the flip base pointer; the diffs were not read. | read from source (history) |
| R18 | T: async path, `vc4_plane_async_set_fb` takes `ptr0_src_offset` (pin) | differs | `b0b7d20`, 2026-11-17, is that change (the 6.12 side has the older form, see C10). The running 6.18.50 kernel most likely has the older form. | read from source (history), inferred |
| R19 | P: S1 plane properties (rotation, alpha, blend, colour encoding, scaling filter, chroma siting) in `vc4_plane_init` | differs | Chroma-siting default changed by `bb74628` "Default vertical chroma siting to interstitial" (2026-09-23) and `268c0eb` (ignore siting on a non-subsampled axis). The property set is not named by any subject. | read from source (history) |
| R20 | P: S2 one Primary per CRTC (`vc4_crtc_init`); S9 CRTC data and HDMI/TXP bindings | same | `vc4_crtc.c` has only the interlaced-position commit; `vc4_txp.c` none. Observed 2026-11-28: four CRTCs, four primaries, 56 planes ([observed/bcm2712-topology.md](observed/bcm2712-topology.md), [hvs-backend-design.md](hvs-backend-design.md) §7 Q4). | read from source (history), observed 2026-11-28 |
| R21 | P: S3 `vc4_plane_create_additional_planes` runs after `component_bind_all`, only when `!vc4->firmware_kms` (`vc4_drv.c:vc4_drm_bind`) | not checked | Eleven `vc4_drv.c` commits since 2026-09-01, five of them about the firmware-KMS mode (revert, reapply, IOMMU), which is the condition this row names. The board runs full KMS (observed 2026-11-28), so the branch taken is the same; the code around it was not read. | read from source (history), observed 2026-11-28 |
| R22 | P: S4, S6 counts (16 shared overlays, 32 TXP overlays, a cursor per CRTC) | same | 4 + 16 + 32 + 4 = 56 planes observed on the Pi 5 (observed 2026-11-28). `fda2f62` (2026-11-26) concerns the zpos range of the writeback overlays, not the count. | observed 2026-11-28, inferred |
| R23 | P: S6 to S8 zpos ranges (shared overlays 1..17; TXP overlays 1..33; cursor 17) | differs | `fda2f62` "Fix the zpos range of the writeback overlay planes" (2026-11-26) changes the TXP overlay range. The shared-overlay range 1..=17 was observed 2026-11-28 and matches the row; the TXP overlay range was not recorded. | read from source (history), observed 2026-11-28 |
| R24 | P: S11, S13 HVS6 channels, output-to-FIFO map, LBM and UPM sizes (`vc4_hvs.c`, `vc4_drv.h`) | same | `vc4_hvs.c` commits in the window do not concern them (see R8). | inferred |
| R25 | P: S12 dlist size from `SCALER6_CXM_SIZE` | not checked | The source doc did not resolve the value; no window commit is named for it. | read from source (history) |
| R26 | HDMI limits, `docs/bcm2712-hdmi-limits-source.md` | still open | The doc is not in this base (its item had not merged). The window has fifteen `vc4_hdmi.c` commits (scrambling before the TMDS clock, PHY power-down, audio), so the row will need this comparison when it lands. | read from source (history) |
| R27 | Replace-pending, `vc4-replace-pending-source.md` | still open | The doc is not in this base. Commit ordering code is in `vc4_kms.c`, which no window query covered. | none |
| R28 | Any row quoted from `vc4_kms.c`, `vc4_hdmi.c` bodies or `drm_fourcc.h` | still open | Not queried: no tree could be fetched, and `include/uapi/drm/drm_fourcc.h` is only needed for the modifier names in R15. | none |

## Rows: the CM4 (HVS5) against the 6.12 tip

The 6.12 tip is what [vc4-scale-limits-source.md](vc4-scale-limits-source.md),
[vc4-formats-source.md](vc4-formats-source.md) and
[vc4-transform-source.md](vc4-transform-source.md) call "6.12". The 6.18 pin
rows for HVS5 have no board and are not compared.

| Row | Source row (6.12 column) | Verdict | Reason | Tag |
| --- | --- | --- | --- | --- |
| C1 | S: horizontal downscale not checked (`vc4_write_tpz` is `void`) | same | None of the six `vc4_plane.c` commits after the package concerns the TPZ scale. | read from source (history), inferred |
| C2 | S: no line skip, no `max_pitch`, no vertical refusal | same | Same reason. The CM4's code is the older 6.12 state. | inferred |
| C3 | S: no source width or height check | same | Same reason. | inferred |
| C4 | S: scaling mode choice and upscale rows | same | Same reason. | inferred |
| C5 | S: LBM capacity 60 K words, 64 handles (`vc4_hvs.c`, `vc4_drv.h:325`) | same | `vc4_hvs.c` after the package: GEN_6D matrices (HVS6), unbind and min-clocks fixes. None is HVS5 LBM. `vc4_drv.h` not listed: handle count **not checked**. | inferred |
| C6 | S: LBM need, `__vc4_lbm_size` (`vc4_plane.c:752`) | differs | `a8c4c46` "TPZ scaling modes cannot reduce lbm size when alpha-less" (2025-12-04) and `d04e379` (remove dead `channels_scaled` code) change the LBM size choice. The CM4 package is from 2025-09-16. | read from source (history) |
| C7 | F: `hvs_formats[]` is 45 entries with RX24 RA24 BX24 BA24 at positions 5 to 8 | differs | **Confirmed by the board.** `80bdb25` adds those four on 2026-03-04, after the package. The CM4's 32 listed formats run XR24 AR24 AB24 XB24 RG16 ... with no RX24 RA24 BX24 BA24 in positions 5 to 8 (observed 2026-11-28, [observed/bcm2711-cm4-topology.md](observed/bcm2711-cm4-topology.md); the list is cut at 32, but those positions are inside the cut, so their absence is real). Likely 41 formats and 49 pairs instead of 45 and 53 (count inferred). Note that P030 does not appear either in the first 32, where the source table puts it at position 25 (after NV61); it is not explained by any commit here. | observed 2026-11-28, read from source (history), inferred |
| C8 | F: modifier array order and rules on gen 5 (`vc4_plane.c:2858-2881`) | same | No window subject concerns them. The CM4 modifier set was not recorded (the probe prints fourccs only, [observed/bcm2711-cm4.md](observed/bcm2711-cm4.md)), so it is **not confirmed by the board**. | inferred |
| C9 | T: rotation mask and gen-5 programming (`vc4_plane_mode_set`, `vc4_plane.c:1405-1408`, `:1684-1689`) | same | No subject concerns the mask or the flip bits. The CM4's supported mask was not read (observed 2026-11-28 shows the current value only). | inferred |
| C10 | T: async path, `vc4_plane_async_set_fb` adds the `REFLECT_Y` move itself (6.12, `vc4_plane.c:2425-2444`) | differs | `590178d` "Correct buffer address on async commit when vertically flipped" (2026-01-13) changed that code after the package. | read from source (history) |
| C11 | F: YUV plane order of Cb/Cr for YVU formats | differs | `f55958e`, `df1d52b` (2025-11-03, -04) swap Cb/Cr pointers and offsets for YVU. The source docs do not cite this row; listed because it changes what YV12/YV16/YV24 mean on the CM4 against the tip. | read from source (history) |
| C12 | Replace-pending (`vc4_kms.c`) and HDMI limits on HVS5 | still open | The source docs are not in this base; `vc4_kms.c` and `vc4_hdmi.c` were not queried for 6.12. | none |

## What this changes for HVS Q7

1. **Pi 5.** Do not read the pin's HVS6 scale-limit, line-skip, LBM, modifier
   and async rows as claims about the running `6.18.50+rpt` kernel. Two are
   already contradicted by the board record (R15, modifiers; R13 is the one
   place where the pin and the board agree). The scale sweep on the Pi 5 is the
   authority for ratios; an acceptance above 31.99998x on that board would
   mean the TPZ check (R1) is absent (inferred).
2. **CM4.** The 6.12 tip differs in the LBM size and in four more formats
   (C6, C7). The scale rows (C1 to C4) are the same code as the running
   kernel, by commit subject only.
3. **Rows that remain open:** R7, R8 (handle count), R17, R21, R25 (not
   checked), R26 to R28 and C12 (still open). To settle them a checkout of
   `rpi-6.18.y` and `rpi-6.12.y` is needed, or the board's source package.
   The exact package commit would turn every "after the package date" in this
   page into a fact.
4. Wording for the design doc (not edited here, by the item's contract): Q7
   can be marked **answered: the rows do not all match**, with this page as
   the link, and the open rows above as the remainder.

## RK Q2

The RK3588 half of this question (VOP2 source rows against the NanoPC-T6's
running kernel) is answered in [rk3588-kernel-compare.md](rk3588-kernel-compare.md).
It is not repeated here.

## Not done

- No tree was fetched (git, curl and gh were refused on this node), so no
  hunk was read and every "same" is by commit subject or by an empty file
  history.
- No board was visited; no vendor or Armbian tree was read.
- No existing doc was edited.
