# imx95_m7_scanout

Zephyr test application for the first hardware step of SoW milestone M2
(§5.1, WP3): framebuffer scanout from the i.MX95 Cortex-M7 through upstream
Zephyr's generic display API, with per-frame timing of `display_write`.

**Status: nothing here has been run on hardware. All timing and scanout
results are not measured.** Only host builds were done (observed, 2026-11-28):
both `west build` commands below succeed, with the DPU option off and on.
They were built with the SDK 1.0.1 `arm-zephyr-eabi` toolchain through
`ZEPHYR_TOOLCHAIN_VARIANT=cross-compile`, and linked images were produced;
they were not loaded anywhere.

## Who owns the DPU

Do not run this image on a board where Linux owns the DPU, for example the
Verdin iMX95 under its Linux BSP, unless the DPU has been shown to belong to
the M7 (see the ownership question in `docs/platform-capabilities.md`). The
application therefore refuses to touch the DPU unless
`CONFIG_IMX95_M7_SCANOUT_ENABLE_DPU=y` (default `n`). With the option off the
`Kconfig` file also turns the upstream driver symbol
`DISPLAY_SOCIONEXT_DPU` off, so the driver is not built and the DPU is not
programmed at boot; `main()` only prints a refusal.

## What it does

With the option on: it reads the capabilities (`display_get_capabilities`),
calls `display_blanking_off`, and then writes full 640x480 ARGB8888 frames
with `display_write`, alternating two buffers. Each frame has a white bar
whose x position is `frame * BAR_STEP` modulo the width, and a strip along
the top that shows the low 16 bits of the frame counter. Each `display_write`
is timestamped with `k_cycle_get_32` before and after, and a CSV line is
printed per frame: frame number, the two cycle stamps, the duration of the
write in microseconds, and the time since the previous write returned.

Kconfig options: `IMX95_M7_SCANOUT_FRAMES` (default 600, 0 = forever),
`IMX95_M7_SCANOUT_BAR_WIDTH`, `IMX95_M7_SCANOUT_BAR_STEP`.

## Latch / VSYNC reporting

Read from source (Zephyr commit below, `drivers/display/display_socionext_dpu.c`):

- The driver has **no VSYNC or frame-done callback** and no API through which
  the application can ask for one.
- It implements `write`, `get_capabilities`, `set_pixel_format` and
  `get_framebuffer`. It has no `blanking_off`, so `display_blanking_off`
  returns `-ENOSYS`; the application accepts that.
- Internally `display_write` points the fetch unit at the buffer, triggers the
  ExtDst0 pipeline shadow load and then blocks on a semaphore that the DPU
  interrupt gives on the ExtDst0 shadow-load interrupt, so it has no timeout.
  The application therefore treats the return of `display_write` as the
  nearest available latch signal. That the interval between returns equals
  the refresh period is inferred, not measured. A real latch/VSYNC report
  would need driver changes, which are out of scope here.
- A full-frame, 32-byte aligned, pitch-matching write uses the caller's buffer
  as the scanout buffer without a copy; this is why the application uses
  full-frame writes.

Also noted for whoever runs this: the driver hard-codes the DPU base address
`0x4B780000` in its interrupt handler and interrupt enable, while the devicetree
node `reg` is `0x4b400000` (read from source, unexplained). The node has no
`clocks` or `power-domains`, so clock and power setup is not done by the
driver. Neither point has been tested.

## Pinned upstream

Zephyr `main` at commit `e53873cbd0ef45d63ef9ded1a45d10717e7c71d7`
(<https://github.com/zephyrproject-rtos/zephyr>). The driver was merged in May
2026, so it is not in a release tag. Source facts above were read at that
commit.

## Overlay

`boards/imx95_evk_mimx9596_m7_ddr.overlay` is picked up automatically by the
board `imx95_evk/mimx9596/m7/ddr` (the DDR variant is used because the frame
buffers need more than the TCM). Upstream only labels the `dpu` node; the
overlay enables it and supplies what the driver needs and the dtsi lacks:
`width`, `height`, `pixel-format` (ARGB8888) and a `display-timings` child, and
sets it as `zephyr,display`. The timing is a placeholder 640x480@60 (VESA), not
the timing of the Verdin panel or of any EVK panel, and no pin, clock or DSI
path is configured. The overlay is for the 19x19 EVK target only; the 15x15 EVK
has no display nodes upstream.

## Build

```sh
west build -p -b imx95_evk/mimx9596/m7/ddr tests/board/imx95_m7_scanout
# DPU enabled
west build -p -b imx95_evk/mimx9596/m7/ddr tests/board/imx95_m7_scanout -- \
    -DCONFIG_IMX95_M7_SCANOUT_ENABLE_DPU=y
```

Requires a Zephyr workspace at the commit above and the Zephyr SDK
(`ZEPHYR_BASE`, `ZEPHYR_SDK_INSTALL_DIR`).

Build result (observed, 2026-11-28): default config links with the DPU driver
not built (RAM use 37904 B); `CONFIG_IMX95_M7_SCANOUT_ENABLE_DPU=y` builds
`display_socionext_dpu.c` and links (RAM use 4953280 B of 42 MB, mostly the
frame buffers). This confirms the Kconfig default override keeps the driver
out of the default image. Whether the image works on hardware is not measured.
