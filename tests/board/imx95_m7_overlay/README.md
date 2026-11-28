# imx95_m7_overlay

Zephyr test application for the overlay step of SoW milestone M2 (§5.1, WP3):
a primary plane and a second (overlay) plane on the i.MX95 DPU, driven from
the Cortex-M7, with both planes changed by one commit. It follows
[`docs/imx95-m7-overlay-design.md`](../../../docs/imx95-m7-overlay-design.md)
(section 2.2 pipeline, section 3 commit semantics, items O1–O4) and uses
upstream Zephyr and the upstream `hal_nxp` module only.

**Status: not run on hardware. Every scanout, overlay, latch and timing
result is not measured.** Only host builds were done (observed, 2026-11-28;
see "Build"). SoW §5.1 puts overlay after scanout, and scanout from the M7 is
not proven yet ([`tests/board/imx95_m7_scanout`](../imx95_m7_scanout/README.md)
has not been run either).

## Who owns the DPU

Do not run the DPU-enabled image on a board where Linux owns the DPU, for
example the Verdin iMX95 under its Linux BSP, unless the DPU, its clocks and
its power domain have been shown to belong to the M7 (see
[`docs/imx95-zephyr-feasibility.md`](../../../docs/imx95-zephyr-feasibility.md)).
The design note calls this ownership model A: the M7 owns the whole DPU and
Linux does not use it in that boot.

The DPU code is therefore opt-in through the CMake option
`IMX95_M7_OVERLAY_ENABLE_DPU` (default `OFF`). With it off, the image
contains no DPU code (the HAL DPU source is not even compiled) and `main()`
only prints a refusal.

## What it proves (once run)

Pipeline, as in design note section 2.2 (read from source for the units; the
choice of units is inferred there):

```
ConstFrame0 --prim--> LayerBlend1 --prim--> LayerBlend2 --> ExtDst0 --> DomainBlend0 --> FrameGen0
FetchLayer0 --sec---> LayerBlend1               ^
FetchYUV0   -------------------sec--------------+
```

- **Primary plane**: FetchLayer0, sublayer 0, 640x480 ARGB8888, forced
  opaque on LayerBlend1 (PRIM_C = ZERO, SEC_C = CONST_ALPHA, alpha 0xFF).
  It shows a background, a white bar that moves 8 px per commit, and a strip
  of 16 cells along the top with the low 16 bits of the commit number.
- **Overlay plane**: FetchYUV0 with RGB input (as the stock driver uses it),
  128x128 ARGB8888, blended on LayerBlend2 with its per-pixel alpha
  (coverage: PRIM_C = 1 - SEC_ALPHA, SEC_C = SEC_ALPHA). The fetch unit's
  `AlphaSrcEnable0` bit is set directly, as the HAL has no API for it. It is a
  half-transparent cyan square with an opaque orange border and a strip of 8
  cells with the low 8 bits of the commit number. It moves 4 px right and
  down per commit and stays fully on screen.
- **One commit for both planes**: for each commit the app draws both
  buffers, cleans them from the D-cache (`sys_cache_data_flush_range`), writes
  both fetch addresses and the overlay position, and then issues **one**
  `DPU_TriggerPipelineShadowLoad(kDPU_PipelineExtDst0)`. If the shadow domain
  covers everything written (inferred in the design note, section 3), both
  planes change at the same latch. A camera can check this: the two counter
  strips must always show the same low 8 bits.
- **Latch**: the ExtDst0 shadow-load status bit (group 0, bit 3), polled with
  a 100 ms timeout. There is no ISR. The app never writes shadow registers
  while `sync_busy` is set (design note section 5, `replace_pending =
  false`); if it finds it set, it logs a refusal and stops.
- Each buffer pair is double-buffered: commit `n` uses buffer `n & 1`, and it
  is drawn only after commit `n - 1` latched.

### Overlay placement (open point O2)

The two upstream sources disagree on how to place the overlay (design note
section 2.2). The CMake variable `IMX95_M7_OVERLAY_PLACEMENT` selects one:

| Value | Fetch frame of FetchYUV0 | Placement register | Source |
| --- | --- | --- | --- |
| `LAYEROFFSET` (default) | full screen; outside the 128x128 buffer the constant colour (fully transparent) | `LAYEROFFSET0` of FetchYUV0 (`DPU_SetFetchUnitOffset`), LayerBlend2 `POSITION` = 0 | the HAL (`DPU_InitLayerBlend` comment) |
| `POSITION` | 128x128 | LayerBlend2 `POSITION`, `LAYEROFFSET0` = 0 | NXP Linux 6.6 (`dpu95_lb_position`) |

### Log (same path as `imx95_m7_scanout`: `printk` on the Zephyr console)

All lines are CSV-like so they can be parsed:

- `config,...`: placement method, HAL build (MDR7 or not), plane sizes,
  pitches, buffer addresses, cache handling.
- `reg,<when>,<name>,<value>`: raw register reads at `reset`,
  `after_first_trigger`, `after_first_latch` and `end` (and on a timeout).
  They cover the O1 probe subset: `COMCTRL_IPIDENTIFIER`; ExtDst0 pipeline
  `STATIC` (sync mode, bit 8), `DYNAMIC` and `STATUS`; `STATICCONTROL` of
  FetchLayer0, FetchYUV0, LayerBlend1, LayerBlend2 and ExtDst0; both planes'
  base address, layer offset and layer property; LayerBlend2 `POSITION`;
  both `BLENDCONTROL`s; FrameGen0 `FGTIMESTAMP` and `FGCHSTAT`; and the group 0
  interrupt status of both interrupt blocks. Whether a read returns the
  shadow or the active copy of a register is open.
- `first latch: ...`, then one line per commit:
  `commit,ov_x,ov_y,status_before,status_after_trigger,status_after_latch,trigger_cyc,latch_cyc,trigger_to_latch_us,frame,line,frame_step,seen_by`.
  `status_*` is the ExtDst0 pipeline `STATUS` (bits 1:0 pipeline status,
  bit 8 `sync_busy`). `frame`/`line` come from `FGTIMESTAMP`, read right
  after the latch was seen; `frame_step` is the frame-index step from the
  previous latch (1 = no missed frame). `seen_by` says which interrupt block
  reported the shadow load: 1 = the block at DPU base + `0x380000` that the
  upstream Zephyr driver hard-codes, 2 = the block at the DPU base that the
  device header names `IRQ_INTERRUPTSTATUS0`, 3 = both. Which one is real is
  open (design note section 1).

## Facts the code relies on

Read from source, at the pins under "Pinned upstream":

- The MIMX9596 M7 build of the HAL is the non-MDR7 build:
  `MIMX9596_cm7_features.h` defines `FSL_FEATURE_DISPLAY_SEERIS_MDR5 (1)`
  and no `FSL_FEATURE_DISPLAY_SEERIS_MDR7`. This settles the "which HAL build"
  open point of the design note from source; the hardware is not probed.
- The HAL DPU source (`mcux/mcux-sdk-ng/drivers/dpu_1/fsl_dpu.c`) is only
  compiled by Zephyr together with the stock driver
  (`modules/hal_nxp/mcux/mcux-sdk-ng/drivers/drivers.cmake`:
  `CONFIG_DISPLAY_SOCIONEXT_DPU` → `driver.dpu_1`). The stock driver programs
  its own single-plane pipeline at boot. So this app leaves the `dpu` node
  disabled (as upstream ships it), keeps the stock driver out (checked by
  `BUILD_ASSERT`s), and adds `fsl_dpu.c` from `${ZEPHYR_HAL_NXP_MODULE_DIR}`
  to its own sources.
- Register names, offsets and bit masks (`PIXENG_EXTDST0CFG_EXTDST0_STATUS`
  `sync_busy`, `FGTIMESTAMP` frame/line fields, `AlphaSrcEnable0`, LayerBlend2
  `POSITION`) come from `devices/i.MX/i.MX95/periph/PERI_DISPLAY_SEERIS.h`.
- The display-stream bring-up (DomainBlend token, timing, `kDPU_DisplayOnlyPrim`,
  `DPU_StartDisplay`) copies the stock driver's `dpu_init`/first `write`,
  including its unexplained "front porch - 1, back porch + 1" adjustment.

Not done, and needed before a run means anything (inferred):

- No pixel clock, LDB/LVDS or DSI route, panel power or backlight is set up.
  The timing is a placeholder 640x480@60 (VESA) and matches no panel.
- No SCMI clock or power-domain request is made for the DPU. If the DPU is
  not clocked and powered when the image starts, the first register access
  may fault or hang.
- The buffers are in the image's DDR RAM region of the
  `imx95_evk/mimx9596/m7/ddr` board. That the DPU sees the same addresses as
  the M7, and that this region does not collide with Linux memory, is not
  checked (see [`docs/imx95-m7-fb-memory.md`](../../../docs/imx95-m7-fb-memory.md)).
- The board target is the NXP i.MX95 19x19 EVK M7 (DDR variant), as for
  `imx95_m7_scanout`. There is no upstream Zephyr board for the Verdin iMX95.
- At the end the last frame stays on screen; the app does not stop the
  display.

## Build

From a west workspace at the Zephyr commit below, with the Zephyr SDK
(`ZEPHYR_WORKSPACE`, `ZEPHYR_SDK_INSTALL_DIR`; see
[`docs/zephyr-workspace.md`](../../../docs/zephyr-workspace.md)):

```sh
cd "$ZEPHYR_WORKSPACE"
# Default: no DPU code, prints a refusal
west build -p always -b imx95_evk/mimx9596/m7/ddr <repo>/tests/board/imx95_m7_overlay -d <build dir>
# DPU enabled, overlay placed with the fetch unit's LAYEROFFSET0 (default)
west build -p always -b imx95_evk/mimx9596/m7/ddr <repo>/tests/board/imx95_m7_overlay -d <build dir> -- \
    -DIMX95_M7_OVERLAY_ENABLE_DPU=ON
# DPU enabled, overlay placed with the LayerBlend2 POSITION register
west build -p always -b imx95_evk/mimx9596/m7/ddr <repo>/tests/board/imx95_m7_overlay -d <build dir> -- \
    -DIMX95_M7_OVERLAY_ENABLE_DPU=ON -DIMX95_M7_OVERLAY_PLACEMENT=POSITION
```

Keep the build directory outside the repository and outside `/tmp`.

Build result (observed, 2026-11-28, Zephyr SDK 1.0.1 `arm-zephyr-eabi`
GCC 14.3.0, `zephyr` toolchain variant): all three commands configure,
compile without warnings and link `zephyr.elf`.

| Configuration | RAM used (of 42 MB) |
| --- | --- |
| default | 37936 B |
| `IMX95_M7_OVERLAY_ENABLE_DPU=ON` | 2626616 B (mostly the 2 x 1.2 MB primary and 2 x 64 KB overlay buffers) |
| `... PLACEMENT=POSITION` | 2626616 B |

The three commands were built again (observed, 2026-11-28) in a workspace at
the `v4.5.0` tag that [`docs/zephyr-workspace.md`](../../../docs/zephyr-workspace.md)
pins (Zephyr `41d10b59730406bbab4ca9f1fbbe9e96aafb4966`, `hal_nxp`
`a8b437ef366ff9a6f9bee3377dc189c4d2623f5b`, same SDK). All three link
`zephyr.elf`; RAM used is 37928 B (default) and 2626608 B (both DPU
configurations).

The DPU-enabled build compiles `fsl_dpu.c` from `hal_nxp` and does not build
`display_socionext_dpu.c`. None of these images was loaded anywhere.

## How to run (not done)

Only on a board where the M7 owns the DPU (see above), and only after the
scanout app has shown a picture there. Load the DPU-enabled image the way
the board's M7 images are loaded (for the Verdin iMX95, see
[`docs/imx95-m7-rproc-load.md`](../../../docs/imx95-m7-rproc-load.md) and the
board rules in [`docs/imx95-board.md`](../../../docs/imx95-board.md)), read
the M7 console, and keep the log. Then report a dated result as
[`docs/board-tests.md`](../../../docs/board-tests.md) describes.

## Pinned upstream

- Zephyr `main` at commit `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7`
  (<https://github.com/zephyrproject-rtos/zephyr>), version string
  `v4.5.0-5218-ge53873cbd0ef`.
- `hal_nxp` at the revision that Zephyr commit's `west.yml` pins:
  `1a8937f3f8dfe5c68f59cc2a0c0c0fccdb096650`
  (<https://github.com/zephyrproject-rtos/hal_nxp>).
