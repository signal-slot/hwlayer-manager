# i.MX8M Plus LCDIFv3 display path, read from source (SoW §5.3)

Date: 2026-11-28. **No claim here comes from a board run.** No device was used.
Every row is tagged `read from source` (the cited file and line say it) or
`inferred` (a conclusion drawn from what was read). Rows say what the code
does, not what a running i.MX8M Plus does.

## Trees

| Tree | Ref | Commit | Status |
|---|---|---|---|
| mainline Linux (`torvalds/linux`, shallow, blobless) | `Makefile` says 7.4-rc4 (`VERSION = 7`, `PATCHLEVEL = 4`, `EXTRAVERSION = -rc4`); no tag at the clone's HEAD | `f4585067150b85e33f60876149a6e742928da75e` (merge of thermal-7.4-rc5, 2026-11-27) | read: `drivers/gpu/drm/mxsfb/lcdif_*`, `arch/arm64/boot/dts/freescale/imx8mp.dtsi`, `Documentation/devicetree/bindings/display/fsl,lcdif.yaml`, `drivers/media/platform/nxp/imx-pxp.c` |
| NXP `linux-imx` | newest `lf-6.6` tag listed by `git ls-remote`: `lf-6.6.52-2.2.2` | tag object `f9a8e33f5cde3e96a57569e9dfa70481fe688943`, peeled commit `5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc` (both from `ls-remote`) | **one file read**: `drivers/gpu/drm/imx/lcdifv3/lcdifv3-plane.c` at the tag, fetched raw; clone and every other file were refused, so the rest is unread |

Consequence: of the vendor tree only the plane file (section 9) was read; the
vendor CRTC code, DT nodes, `gpu-viv` and g2d nodes are open. They are listed in
"Open questions", with no claim made about them. Paths in the tables are
relative to the mainline root, which is the only tree read. The clone is
kept outside the repository.

## 1. Instances and outputs

| Fact | Where (mainline) | Tag |
|---|---|---|
| Three LCDIF nodes exist: `lcdif1`, `lcdif2`, `lcdif3`, all `compatible = "fsl,imx8mp-lcdif"`, all `status = "disabled"` in the SoC file | `arch/arm64/boot/dts/freescale/imx8mp.dtsi:1902`, `:1920`, `:2156` | read from source |
| `lcdif1` has one endpoint, to the MIPI DSI (DSIM) input port 0 | `imx8mp.dtsi:1914`, `:1888` | read from source |
| `lcdif2` has one endpoint, to the LDB (LVDS bridge) input port 0; the LDB has two LVDS channel output ports | `imx8mp.dtsi:1932`, `:2003`, `:2020`, `:2028`, `:2035` | read from source |
| `lcdif3` sits in the HDMI block and goes to `hdmi_pvi`, which goes on to `hdmi_tx` | `imx8mp.dtsi:2169`, `:2111`, `:2125`, `:2132`, `:2175` | read from source |
| Each LCDIF node has a single `port` with one endpoint, so a CRTC drives one fixed output | `imx8mp.dtsi:1913`, `:1931`, `:2168` | read from source |
| The driver binds each node as its own DRM device: `lcdif_probe` allocates one `drm_device` per platform device, and `lcdif_drm_private` holds one CRTC | `drivers/gpu/drm/mxsfb/lcdif_drv.c:257-274`; `lcdif_drv.h:18-31` | read from source |
| Mainline matches `fsl,imx8mp-lcdif` (and `fsl,imx93-lcdif`) in this driver | `lcdif_drv.c:250-254`; `fsl,lcdif.yaml` compatible list | read from source |
| LCDIF1/2 interrupts are GIC SPI 5 and 6; LCDIF3 goes through the HDMI IRQ steer block | `imx8mp.dtsi:1906`, `:1922`, `:2159-2160` | read from source |
| Which of the three can run at the same time, and which pixel clocks they can share, was not worked out | n/a | inferred (open) |

## 2. Planes per CRTC

| Fact | Where | Tag |
|---|---|---|
| The private struct holds exactly one plane, `primary`; the comment next to it says another SoC supports overlays, "add them here" | `lcdif_drv.h:27-30` | read from source |
| `lcdif_kms_init` registers one plane, of type `DRM_PLANE_TYPE_PRIMARY`, and one CRTC on it, with no cursor plane | `lcdif_kms.c:756-790` (`drm_universal_plane_init` at `:768`) | read from source |
| The CRTC check rejects an active CRTC without the primary plane | `lcdif_kms.c:441`, `:452` | read from source |
| Mainline therefore exposes 1 plane per CRTC (3 CRTCs on three DRM devices, if all enabled): a plane layout with no overlay | `lcdif_kms.c:756-790` | inferred |

## 3. Formats and modifiers

| Fact | Where | Tag |
|---|---|---|
| Primary plane formats: RGB565, RGB888, XBGR8888, XRGB1555, XRGB4444, XRGB8888, and packed YUYV, YVYU, UYVY, VYUY | `lcdif_kms.c:731-745` | read from source |
| No format with an alpha channel is advertised (only `X` variants of the 16/32-bit RGB formats) | `lcdif_kms.c:731-745` | read from source |
| The format-to-register mapping writes a BPP/format field in the layer register; 32-bit XRGB and XBGR map to the ARGB8888 and ABGR8888 codes, but the alpha is not exposed | `lcdif_kms.c:162-230`; `lcdif_regs.h:206-213` | read from source |
| Only the linear modifier is supported | `lcdif_kms.c:711-714`, `:747-750` | read from source |
| A built-in colour-space converter handles packed YCbCr to RGB, with BT.601/709/2020 and limited/full range as plane properties, and RGB to YCbCr for a UYVY bus format | `lcdif_kms.c:56-160`, `:240-292`, `:777-783` | read from source |
| No planar or semi-planar YUV (NV12, YUV420) format is listed | `lcdif_kms.c:731-745` | read from source |

## 4. Scaling, alpha, zpos

| Fact | Where | Tag |
|---|---|---|
| Plane check passes `DRM_PLANE_NO_SCALING` for both min and max scale, so source and destination must be the same size | `lcdif_kms.c:672-690` (`:687-688`) | read from source |
| The plane size must also fit the CRTC (the helper call sets "can position" false, "can update disabled" true) | `lcdif_kms.c:685-690` | read from source |
| No `alpha`, `pixel blend mode` or `zpos` property is created for the plane in `lcdif_kms_init`; the only plane properties added are the colour-encoding and colour-range ones | `lcdif_kms.c:756-790` | read from source |
| No scaler, blender or second layer register exists in the register map the driver uses: it names layer 0 only (`CTRLDESCL0_*`) and one CSC (`CSC0_*`) | `lcdif_regs.h:15-37` | read from source |
| The `DISP_PARA` register has background colour fields; the driver does not program them | `lcdif_regs.h:146-148`; no write in `lcdif_kms.c` | read from source |
| So mainline gives no per-plane scaling, alpha or zpos on this controller | `lcdif_kms.c`, `lcdif_regs.h` | inferred |

## 5. Vblank and latch semantics

| Fact | Where | Tag |
|---|---|---|
| The driver writes the new framebuffer address into `LCDC_V8_CTRLDESCL_LOW0_4` and the high bits (4 bits) into `..._HIGH0_4` in the plane update | `lcdif_kms.c:692-707`; `lcdif_regs.h:27-28`, `:201-202` | read from source |
| The CRTC flush sets `CTRLDESCL0_5_SHADOW_LOAD_EN` (bit 30 of `CTRLDESCL0_5`) after that write, so the address is a shadowed value that takes effect when the hardware loads it | `lcdif_kms.c:515-525`; `lcdif_regs.h:205` | read from source |
| The interrupt handler treats the `VS_BLANK` status bit as a real vblank only when the shadow-load enable bit has been cleared, i.e. the hardware consumed the load | `lcdif_drv.c:107-126` (`:117-120`) | read from source |
| Completion event: flush arms the DRM vblank event if `drm_crtc_vblank_get` succeeds, otherwise sends it at once | `lcdif_kms.c:527-538` | read from source |
| Vblank enable clears the status and enables `INT_ENABLE_D0_VS_BLANK_EN`; disable does the reverse | `lcdif_kms.c:632-652` | read from source |
| Disable sets the shadow-load bit and polls until the hardware clears it (200 µs steps, 50 ms limit) before turning the display off | `lcdif_kms.c:372-402` (`:386-391`) | read from source |
| Enable also writes the address before turning the controller on, "to avoid an initial corrupt frame" | `lcdif_kms.c:540-568` | read from source |
| The new buffer is shown at the next frame boundary after the flush, and the old buffer is free only after the event; this matches "scan-out latched at vblank, release on flip event" | `lcdif_kms.c:515-538`; `lcdif_drv.c:117-120` | inferred |
| No fence, out-fence or "frame done" register other than `VS_BLANK` is used | `lcdif_drv.c:107-126` | read from source |
| A FIFO-panic watermark is set (low 1/3, high 2/3) to raise bus priority; it raises no interrupt | `lcdif_kms.c:348-368` | read from source |

## 6. Framebuffer memory

| Fact | Where | Tag |
|---|---|---|
| Framebuffers come from the GEM DMA helpers: the driver uses `drm_gem_fb_create` and `DRM_GEM_DMA_DRIVER_OPS`, so buffers are physically contiguous | `lcdif_drv.c:36-40`, `:241` | read from source |
| The DMA mask is 36 bits, matching the 4-bit high address field | `lcdif_drv.c:161`; `lcdif_regs.h:201-202` | read from source |
| The layer pitch field is 16 bits wide; the driver writes the plane's pitch directly, with no alignment rounding in the driver | `lcdif_regs.h:198-199`; `lcdif_kms.c:340-344` | read from source |
| Maximum width and height are both 0xffff in the driver | `lcdif_regs.h:263-264`; `lcdif_drv.c:191-194` | read from source |
| The driver sets burst-size fields (P_SIZE, T_SIZE = 2) described in a comment as 256 B bursts; the comment says the fields are undocumented | `lcdif_kms.c:332-344` | read from source |
| No IOMMU or SMMU is referenced by the LCDIF nodes, only a power domain and interconnect names on the media block control | `imx8mp.dtsi:1902-1930`; `:1967` | read from source |
| So scan-out needs contiguous, linear, CMA-style memory with no address translation; buffers from a non-contiguous allocator would not work | `lcdif_drv.c:36-40`, `:161` | inferred |
| Alignment of the buffer address and of the pitch beyond the register widths is not stated in the driver | n/a | inferred (open) |

## 7. 2D blitter, PXP and GPUs

| Fact | Where | Tag |
|---|---|---|
| The mainline PXP mem2mem driver matches only `fsl,imx6ull-pxp` and `fsl,imx7d-pxp` | `drivers/media/platform/nxp/imx-pxp.c:1928-1929` | read from source |
| `imx8mp.dtsi` has no PXP node (the grep for `pxp` found none) | `imx8mp.dtsi` (no match) | read from source |
| `imx8mp.dtsi` has two Vivante `vivante,gc` nodes: `gpu3d` at 0x38000000 and `gpu2d` at 0x38008000 | `imx8mp.dtsi:2311`, `:2329` | read from source |
| `gpu2d` is a separate 2D core, so a 2D engine exists in hardware; whether it is GC520L and whether it can blit with blending is not stated in the file | `imx8mp.dtsi:2329-2342` | inferred (open) |
| ISP and dewarp nodes exist, a camera path, not a display compositor | `imx8mp.dtsi:1733`, `:1758`, `:1783` | read from source |
| No DRM driver in the display path uses `gpu2d` as an overlay; the display path in mainline is LCDIF only | `lcdif_drv.c`, `lcdif_kms.c` | inferred |

## 8. Consequences for a composition engine

| Consequence | Basis | Tag |
|---|---|---|
| With mainline LCDIFv3 a CRTC has one plane with no scaling, alpha or zpos; every layer must be composed into one buffer before scan-out, and an overlay assignment is never available | sections 2 and 4 | inferred |
| A planner that sees one primary plane and several layers must fall back to composition for all but one layer; the primary is a pre-composed frame | sections 2 and 4 | inferred |
| The pre-composed frame must be contiguous linear memory in one of the section 3 formats; a producer other than the CPU (a GPU, the 2D core, or another processor) has to write into such memory | sections 3 and 6 | inferred |
| Buffer release can only follow the flip event; there is no explicit scan-out fence | section 5 | inferred |
| Whether a blitter could do the composition (and under which driver) depends on the vendor tree, which was not read | section 7 | inferred (open) |

## 9. Vendor tree (`linux-imx` lf-6.6.52-2.2.2): plane file only

Paths relative to the linux-imx root. Line numbers are counted from the fetched
file and may be off by a few lines.

| Fact | Where (linux-imx) | Tag |
|---|---|---|
| Vendor plane formats: XRGB8888, ARGB8888, RGB565, XBGR8888, ABGR8888, ARGB1555, XRGB1555; unlike mainline this includes ARGB and ABGR 32-bit and has no YUV format | `drivers/gpu/drm/imx/lcdifv3/lcdifv3-plane.c:22-30` | read from source |
| Plane check uses `DRM_PLANE_NO_SCALING` both ways; no scaling in the vendor driver either | `lcdifv3-plane.c:~62-65` | read from source |
| The plane must sit at CRTC position (0,0); a nonzero `crtc_x` or `crtc_y` is rejected, and an invisible plane is rejected | `lcdifv3-plane.c:~55-57`, `~67-68` | read from source |
| A change of framebuffer pitch forces `mode_changed`, because the pitch registers cannot be changed while the controller runs (comment in the file) | `lcdifv3-plane.c:~70-77` | read from source |
| Only the primary plane type is handled in `atomic_update`; the default branch is a "TODO: add overlay later" return, so overlay planes are not implemented in this file | `lcdifv3-plane.c:~101-110` | read from source |
| The plane init refuses a nonzero `zpos` and creates an immutable zpos property of 0; no alpha or blend-mode property is created in this file | `lcdifv3-plane.c:~150-160`, `~172-177` | read from source |
| The vendor driver sets `fb_modifiers_not_supported`: linear only | `lcdifv3-plane.c:~147` | read from source |
| The scan-out address is source offset plus the buffer's DMA address, written through `lcdifv3_set_fb_addr`; the format is set through `lcdifv3_set_pix_fmt` for the primary plane | `lcdifv3-plane.c:~97-117` | read from source |
| Vendor ARGB formats are advertised but, with a single plane at zpos 0, there is nothing to blend with in this file | `lcdifv3-plane.c:22-30`, `~150-177` | inferred |
| The vendor tree adds no overlay or blending beyond mainline, as far as this plane file shows; the CRTC file, register layer, DT nodes and `gpu-viv` stay unread | section above | inferred (open) |

## Open questions

1. **Vendor tree, mostly unread.** Only `lcdifv3-plane.c` was read. The
   vendor CRTC code, register layer and DT nodes of `lf-6.6.52-2.2.2`
   (commit `5a0a5e71d2bd`) are unread; does any of them add overlay or
   blending hardware use?
2. Does the vendor tree expose `gpu2d` through `drivers/mxc/gpu-viv` or a g2d
   node, and which blit operations (blend, scale, format convert) can it do?
   Mainline only shows two `vivante,gc` nodes.
3. The shadow-load behaviour: when exactly does the hardware clear
   `SHADOW_LOAD_EN`, at the start or at the end of the frame? The driver
   relies on it; the reference manual was not consulted.
4. Address and pitch alignment limits that the hardware needs, beyond the
   register field widths.
5. Which LCDIF/output combinations can run together, and what pixel-clock
   sharing applies; the LDB dual-channel mode and the HDMI path (PVI) drivers
   were not read.
6. Whether any IOMMU or address translation sits between the media block and
   memory on this SoC; the DT shows none for LCDIF, which is a statement
   about the node, not about the bus.
7. Everything above has to be checked on a board before it is reported as a
   capability.
