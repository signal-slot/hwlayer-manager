# AM62P DSS: the GO bit, shadow registers and the latch at VSYNC (source study)

Date of the study: 2026-11-28. No device was used and nothing here is `observed`. Every statement carries one of two tags:

- **source**: read in a public document or repository; the version and place are given.
- **inferred**: the author's conclusion from sources, or from this repository's C. It is a hypothesis for the board run to test.

Plane formats, zpos and blending are not covered here (see `docs/am62p-dss-source.md` once it exists).

## What was read, and what could not be reached

| Source | Version | Reached? |
| --- | --- | --- |
| Linux `drivers/gpu/drm/tidss/tidss_crtc.c` | v6.12 (450 lines) | Yes, through GitHub's HTML view of `blob/v6.12/...?plain=1`. Line numbers below are `~`: counted by hand from a converted page, plus or minus a few lines. Function names and quoted excerpts are exact. |
| Linux `tidss_dispc.c` (v6.12) | | **Only the first ~780 of 2959 lines**, which include the VP IRQ bit maps and the `dispc_k3_*` IRQ helpers. `dispc_vp_go`, `dispc_vp_go_busy` and `dispc_vp_enable` were not read. The GO bit position comes from an LKML patch (below). |
| Linux `tidss_dispc_regs.h` | v6.12 (243 lines) | Yes. It has no GOBIT or VP IRQ bit macros. |
| LKML patch "drm/tidss: dispc: Switch VP_REG_GET to using a mask" (2025, M. Ripard) | lkml.iu.edu hypermail 2507.3 message 06918 | Search snippet only. |
| TI AM67 TRM SPRUJB3 (March 2024), DSS shadow-register section and VP interrupt table | GUID-826823F4-1511-4A8F-9799-B408ADBD867B | Search snippets only. A **sibling** device of the AM62P (same DISPC family), not the AM62P. |
| TI AM62P silicon errata SPRZ574B (Oct 2025), advisory i2097 | GUID-366566E5-9066-4ECC-A4BF-8AAA2228E812 | Read in full on 2026-11-28 (HTML page, section 2.2 Silicon Advisories; Figures 2-1 and 2-2 are images and were not read). |
| **TI AM62P TRM** (DSS chapter, `DISPC_VP_CONTROL`, `DISPC_VP_IRQSTATUS`) | | **Not reached.** The TRM page `SPRUJ83` is over 10 MB and the fetch failed; the DSS chapter's own page addresses are unknown. Nothing below is quoted from the AM62P TRM. |
| TI MCU+ SDK DSS driver for AM62P | `mcupsdk-core` release tags of Dec 2025 (K3 11.02) and Aug 2026 (K3 12.01), branch `next` | **Not found.** The public `mcupsdk-core` has no `dss`, `dispc` or `dctrl` under `source/drivers`; `mcupsdk-core-am62px` returns 404. A TI docs page `AM62Px MCU+ SDK 12.00.00 ... cslr_dss.h` is listed by search but returned 404 when fetched. |
| Upstream Zephyr | `main`, `drivers/display` and `dts/bindings/display` listings | No TI DSS driver in the Kconfig list (complete) or the binding list (cut off before `ti,*`; not conclusive). |

Consequence: (a) and (b) rest on the AM67 TRM and Linux. That the AM62P behaves as the AM67 does is **inferred** wherever it is written so. The first thing to do when the AM62P TRM can be read is to check the rows tagged "AM67 TRM" against it.

## (a) Shadowed registers, and when shadow values reach the active ones

- **source** (AM67 TRM SPRUJB3, DSS shadow-register section, via search snippet): "When all the values for a given configuration are written into the shadow registers, software must set only one register bit to validate the configuration. The DSS0_VP_CONTROL[5] GOBIT bit enables the hardware to use the new configuration, for all shadow registers associated with each VP output."
- **source** (same section): some shadow registers are associated "statically" with one output ("for example, timing registers") and others "dynamically ... to one output at a time (for example, video registers)". So the GO bit that applies to a plane's registers is that of the video port (VP) the plane is routed to at that moment.
- **source** (Linux v6.12 `tidss_crtc.c`, `tidss_crtc_atomic_flush` ~176-208): the driver writes `dispc_vp_setup` and the planes first, then calls `dispc_vp_go()` once per VP, under the event lock. It writes no per-plane commit bit. Plane writes are therefore latched by the VP's GO, not by a bit of their own.
- **source** (LKML patch 2507.3/06918): `dispc_vp_go_busy()` and `dispc_vp_go()` read and write `GENMASK(5, 5)` of `DISPC_VP_CONTROL`: bit 5 is the GO bit in the driver, the same position as the TRM's `DSS_VP_CONTROL[5]`.
- **source** (Linux v6.12 `tidss_crtc.c` `tidss_crtc_finish_page_flip` ~33-37): "New settings are taken into use at VFP, and GO bit is cleared at the same time. This happens before the vertical blank interrupt." So the copy happens at the vertical front porch of the frame, not at the VSYNC pulse. The driver's comment also says the driver may set GO after VFP but before vblank.
- **source** (AM67 TRM interrupt table, via snippet): "Shadow to work copy of registers associated with VP1 has occurred. DSS_VP1_CONTROL[5] GO register bit is cleared." (the `VPSYNC_IRQ` entry). This is the TRM's statement that the GO bit clears itself when the copy is done.
- **inferred**: which exact registers are shadowed on the AM62P (the full list) is not known from the sources read; the AM67 text names timing and video (plane) registers as shadowed classes. The backend writes plane buffer address, plane enable and the safety flip, which are plane ("video") registers, so they fall under the GO of the VP they are routed to.
- **inferred**: the AM62P DSS has two video ports (VP1, VP2) with separate GO bits; the two-plane backend uses one output, so one GO bit matters. The count of VPs on the AM62P was not verified in a source for this study.

## (b) How software learns that a configuration has latched

1. **GO bit reads back 0.**
   - **source** (Linux `tidss_crtc.c` ~38-41): the vblank handler calls `dispc_vp_go_busy()` and, `if (busy)`, returns without sending the page-flip event; the event stays armed for the next vblank. This is the driver's definition of "not yet latched".
   - **source** (AM67 TRM, above): the GO bit is cleared when the shadow-to-work copy has happened. Reading back 0 therefore means latched.
2. **VP interrupts.**
   - **source** (AM67 TRM table, via snippet): `VPSYNC_IRQ` at bit 11 and `FRAMEDONE_IRQ` at bit 0 of the VP IRQ status register of that family; the VPSYNC entry is the one that mentions the copy and the GO clear. The AM69 TRM instead names the same event `VSYNC_IRQ` at bit 1. Bit positions differ between device families.
   - **source** (Linux v6.12 `tidss_dispc.c` first ~780 lines, `dispc_vp_irq_from_raw`): the driver maps raw bits to `DSS_IRQ_VP_FRAME_DONE` (bit 0), `DSS_IRQ_VP_VSYNC_EVEN` (bit 1), `DSS_IRQ_VP_VSYNC_ODD` (bit 2), `DSS_IRQ_VP_SYNC_LOST` (bit 4). Plane underflow is bit 0 of the video-plane IRQ register.
   - **source** (same file, `dispc_k3_clear_irqstatus`, `dispc_k3_set_irqenable`): the top-level `DISPC_IRQSTATUS` has one bit per VP (bit i) and per plane (bit 4+i); each VP and plane has its own status and enable register; `DISPC_IRQENABLE_SET` and `DISPC_IRQENABLE_CLR` enable and disable; the status is cleared by writing it back, with a read-back as a posted-write flush. Newly enabled IRQs get their status cleared first.
   - **source** (Linux `tidss_crtc.c` ~57-71): the VSYNC handler runs `drm_crtc_handle_vblank` and the page-flip finish above; the FRAMEDONE handler only completes `framedone_completion` (used when disabling a VP, with a 500 ms wait); the error handler only logs "SYNC LOST".
   - **inferred**: the AM62P VP IRQ bit numbering is the one `tidss` uses for K3 (this driver serves the AM62P), but the AM62P TRM table was not read. Which of VSYNC or FRAMEDONE a firmware should use for the latch is stated by the TRM text only for VSYNC (it names the copy and the GO clear); FRAMEDONE is described as the end of the frame.
3. **Is the GO bit already 0 when the VSYNC interrupt runs?**
   - **source**: the Linux comment says the clear "happens before the vertical blank interrupt", and the driver still checks `go_busy` in the handler because GO may have been set again after VFP.
   - **inferred**: a firmware reading `go_busy` in a VSYNC handler can see a 1 for two reasons: the commit did not latch yet, or a later commit set GO after the copy. The driver treats both the same way (wait for the next vblank).
4. **No "GO done" event other than VPSYNC was found in the sources read.** (Absence of evidence in snippets and in a partly read file, not a claim about the silicon.)

## (c) GO set again before it cleared; plane writes while GO is pending

- **source** (Linux `tidss_crtc_atomic_flush` ~183-185): "If the GO bit is stuck we better quit here." followed by `WARN_ON(dispc_vp_go_busy(...))` and a return. The driver does not set GO again while it reads 1; it drops the flush and warns.
- **source** (same function ~187-208, and `tidss_crtc_finish_page_flip`): the order is: check GO not busy, write VP and plane registers, take the event lock, set GO, arm the event. This is the driver's way to keep the plane writes of one commit under one GO and the writes of the next commit out of the pending one.
- **source** (AM67 TRM text above): software "must set only one register bit to validate the configuration", after writing "all the values". The text snippets read give no statement of what the hardware does when GO is written 1 while it is already 1, or when a shadow register is written between GO and the copy. **No source read answers this.**
- **inferred**: plane registers written while GO is pending go into the shadow set. If the copy happens before the write completes (the driver's comment says the copy happens at VFP), the first half of a multi-register update (for example buffer address and enable) can be in the copied set and the second half in the next one: a torn configuration across two frames. The `tidss` driver avoids this by never writing planes while GO reads 1 (the `WARN_ON` and return). A firmware that does not check GO before writing has this risk.
- **inferred**: a write to GO while it is 1 is most likely a no-op on the bit (it stays 1 until the copy) and merges the second commit's shadow values into the same copy. If so, two commits are latched by one copy: a latched counter that counts copies is then lower than the commit counter until a later copy, and a backend that waits for "latched >= commit count" waits for the next GO. The board run can show it (see (d)); it is not established.
- **source** (AM62P errata SPRZ574B, advisory i2097, "DSS: Disabling a Layer Connected to Overlay May Result in Synclost During the Next Frame"; this is an AM62P document, read directly; it also says the frame may be "corrupted or blank", all pixels 0x0, and that "If the disable-layer write and the set-GO-bit write fall within the same frame boundary, no synclost occurs", otherwise one frame is lost): disabling a layer (`DSS_VID_ATTRIBUTES[0]` ENABLE 1 to 0) may cause a synclost "during the next frame"; the revised text says it "is dependent on the timing of setting the GO bit (that is DSS_VP_CONTROL[5] GOBIT to 1) vis-à-vis the disabling of the layer", that "the design automatically recovers and returns to normal operation from the next frame after GO bit is set", and the workaround is to move the layer to the overlay's non-visible area (POSX or POSY maximum) before disabling it. A sibling erratum (AM67, SPRZ575) says that if the disable write and the GO write fall in the same frame boundary there is no synclost. The pseudo-code figure was not read.
- **inferred**: the backend's `enable_app_plane(dev, false)` is a layer disable, so the board run may meet advisory i2097 as a `SYNC_LOST` interrupt on scenes that remove the application plane. The workaround is a driver matter (`display_port_am62p.c` of the showcase or the board test's own driver), not the backend's.

## (d) The two-plane backend's commits and latches, mapped to (a) to (c)

What the backend asks of a driver is in `backends/twoplane/ce_twoplane.h` (header comment) and `docs/twoplane-backend.md`; the code is `backends/twoplane/ce_twoplane.c` (commit 654646010f5c of this repository).

| Backend notion | Where | Mapped to the hardware |
| --- | --- | --- |
| A **commit** is every call that writes the planes; `commit_count()` is read after the writes | **source** `ce_twoplane.h` header; `ce_twoplane.c` `twoplane_commit`: `b->pending_count = b->hw->commit_count(b->dev)` | **inferred**: a commit is one GO write of the VP, issued by the driver after its plane writes, as `tidss_crtc_atomic_flush` does. The driver, not the backend, counts it. |
| **Latched** means `latched_count()` has reached the commit's count; "driver's count of commits the hardware has latched" | **source** `ce_twoplane.c` lines 7-15 ("from the GO bit on the AM62P"), `reached()`, `note_latch()` | **inferred**: one increment of `latched_count` per observed GO 1 to 0 transition, that is, per shadow-to-work copy, seen either by reading the GO bit or in the VSYNC interrupt. This is (b)1 and (b)2. |
| The check runs at each vsync callback, in `poll_event`, and at the end of a commit | **source** `ce_twoplane.c` `vsync_cb`, `twoplane_poll_event`, `twoplane_commit` | **inferred**: the VP VSYNC interrupt is the place where (b)2 delivers the copy event; the check at the end of a commit covers a copy that happened while the writes were in progress (for the AM62P: GO set early, copy at VFP before the second write finished; compare the torn case in (c)). |
| A commit's writes "reach the panel together at a frame start, with any later writes made before that frame start" | **source** `ce_twoplane.h` header | **inferred**: this is the GO-copy rule of (a) plus the case of (c) in which the driver sets GO once after all its writes. If a driver sets GO per call, a commit's writes can reach the panel in two copies and the count would still match. |
| `latched_count` wraps; `reached()` uses signed 32-bit difference | **source** `ce_twoplane.c:31-34` | Not a hardware matter. |
| A new commit is refused (`CE_ERR_BACKEND_BUSY`) while a latch is unreported | **source** `twoplane_commit` | No hardware counterpart; it is the backend's single-slot report. |
| "Two commits latched by one copy" | **source**: not addressed in the C beyond `reached()` | **inferred**: if (c)'s merge case holds, `latched_count` jumps by the number of merged commits only if the driver credits each commit; if the driver increments by one per copy, the backend's `pending_count` (a later commit) is never reached until another commit. The driver's rule decides this. |

### What the board run can measure: commit count against latched count

All of these need only the driver table and a readable counter; none is observed yet.

1. `commit_count` and `latched_count` after each scene settles (the backend's own counters; the board test prints `SCENE`/`RESULT` lines). Expected, **inferred**: equal after a scene's last frame start; any difference at the end of a settled scene points to a missed copy or a merged GO.
2. The GO bit (`DISPC_VP_CONTROL` bit 5) read right after each GO write (expected 1) and in the VSYNC handler (expected 0 for a lone commit). A trace of GO as seen at each VSYNC interrupt separates "not latched yet" from "set again after the copy" ((b)3).
3. Number of VSYNC interrupts between a commit and its latch. **inferred** expectation: at most one, if GO is set before the VFP of the current frame; a commit made after VFP latches one frame later.
4. A commit made while the GO bit still reads 1: the commit count rises by one (if the driver counts every GO write) and the latched count either rises by two at the next copy (merge credited) or by one (merge not credited). This is the experiment for the unanswered question of (c); run it only in a test scene made for it, as `scenes.c` may not contain it.
5. VP `SYNC_LOST` status and the plane underflow status after the scenes that disable the application plane (advisory i2097), read from the VP IRQ status register with the TRM bit numbers once checked.
6. `FRAMEDONE` against `VSYNC` counts, to see which one the latch follows on the AM62P (the sources only tie the GO clear to VSYNC).
7. A register dump of the VP control and the plane attribute registers before and after a commit (shadow against active is not separately readable in the sources read; whether a register read returns the shadow or the work value is **not established** by any source above).

## Open points for the next reader of the AM62P TRM

- The text of the `GOBIT` field of the AM62P `DISPC_VP_CONTROL` and of the VP IRQ status register (bit numbers of VSYNC, FRAMEDONE, SYNC_LOST).
- What the hardware does for GO written while it is 1, and for shadow writes after GO.
- Whether reads of plane registers return shadow or work values.
- Whether TI's MCU+ SDK or another public TI source has a DSS driver for the AM62P R5F (none was found in `mcupsdk-core` release K3 12.01).
