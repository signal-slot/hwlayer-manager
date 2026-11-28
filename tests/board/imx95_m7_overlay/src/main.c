/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * Two-layer DPU overlay commit on the i.MX95 Cortex-M7, after
 * docs/imx95-m7-overlay-design.md (sections 2.2 and 3, items O3/O4):
 *
 *   ConstFrame0 --prim--> LayerBlend1 --prim--> LayerBlend2 --> ExtDst0
 *   FetchLayer0 --sec---> LayerBlend1               ^
 *   FetchYUV0   -------------------sec--------------+
 *
 * The primary plane is a full-screen ARGB8888 buffer on FetchLayer0, forced
 * opaque. The overlay is a smaller ARGB8888 buffer on FetchYUV0, blended with
 * its per-pixel alpha. Each commit writes the shadow registers of both planes
 * (buffer addresses and overlay position) and then issues ONE ExtDst0 pipeline
 * shadow-load trigger, so both planes should change at the same latch. The
 * latch is the ExtDst0 shadow-load interrupt status bit, polled (no ISR).
 *
 * Every DPU access goes through the upstream hal_nxp DPU driver (fsl_dpu.c)
 * or the upstream MIMX9596 device header. The stock Zephyr DPU display driver
 * is not used: it supports one fixed plane only.
 *
 * Not run on hardware. Without -DIMX95_M7_OVERLAY_ENABLE_DPU=ON the image
 * contains no DPU code and only prints a refusal.
 */

#include <zephyr/kernel.h>
#include <zephyr/sys/printk.h>
#include <zephyr/sys/util.h>

#if !defined(IMX95_M7_OVERLAY_ENABLE_DPU)

int main(void)
{
	printk("imx95_m7_overlay: refusing to touch the DPU.\n");
	printk("Linux may own it. Rebuild with -DIMX95_M7_OVERLAY_ENABLE_DPU=ON "
	       "only if the M7 owns the DPU on this board.\n");
	return 0;
}

#else /* IMX95_M7_OVERLAY_ENABLE_DPU */

#include <zephyr/cache.h>
#include <zephyr/devicetree.h>
#include <fsl_dpu.h>

/*
 * The DPU registers as the MIMX9596 device header names them, and the base of
 * the interrupt block that the upstream Zephyr driver hard-codes for its
 * interrupt enable/status/clear (display_socionext_dpu.c, APP_DPU_DC). Which
 * of the two interrupt blocks reports the ExtDst0 shadow load is open
 * (docs/imx95-m7-overlay-design.md section 1), so the latch wait polls both
 * and logs which one saw it.
 */
#define DPU           DISPLAY_SEERIS
#define DPU_IRQ_BLOCK ((DISPLAY_SEERIS_Type *)0x4B780000U)

/* The DT node is used for nothing but this cross-check. */
BUILD_ASSERT(DT_REG_ADDR(DT_NODELABEL(dpu)) == DISPLAY_SEERIS_BASE,
	     "devicetree dpu node and device header disagree on the DPU base");
/* The stock driver would program the DPU at boot with its own pipeline. */
BUILD_ASSERT(!DT_NODE_HAS_STATUS(DT_NODELABEL(dpu), okay),
	     "keep the dpu node disabled: this app programs the DPU itself");
BUILD_ASSERT(!IS_ENABLED(CONFIG_DISPLAY_SOCIONEXT_DPU),
	     "the stock DPU driver must not be built into this app");

/*
 * Placeholder timing: 640x480@60 (VESA), the same as the imx95_m7_scanout
 * overlay. It is not the timing of any panel on any board, and no pixel clock,
 * LDB/LVDS or DSI route is configured here.
 */
#define WIDTH  640U
#define HEIGHT 480U
#define HFP    16U
#define HBP    48U
#define HSW    96U
#define VFP    10U
#define VBP    33U
#define VSW    2U

#define BPP      4U /* ARGB8888 */
#define PITCH    (WIDTH * BPP)
#define OV_W     128U
#define OV_H     128U
#define OV_PITCH (OV_W * BPP)
#define OV_STEP  4U /* overlay movement per commit, pixels */

#define COMMITS          600U
#define LATCH_TIMEOUT_US 100000U /* about six frames at 60 Hz */

#define CNT_CELL 16U /* pixels per counter bit; counter strips are CNT_CELL rows high */
#define BAR_W    32U
#define BAR_STEP 8U

#define COLOR_BG      0xFF101040U
#define COLOR_BAR     0xFFFFFFFFU
#define COLOR_ON      0xFF40FF40U
#define COLOR_OFF     0xFF404040U
#define COLOR_OV_FILL 0x8000C0C0U /* half-transparent cyan */
#define COLOR_OV_EDGE 0xFFFF8000U /* opaque orange border */
#define OV_EDGE       4U

#define SHDLOAD kDPU_Group0ExtDst0ShadowLoadInterrupt

/* Unit set and interrupt bit numbers differ between the two HAL builds. */
#if defined(FSL_FEATURE_DISPLAY_SEERIS_MDR7) && FSL_FEATURE_DISPLAY_SEERIS_MDR7
#define HAL_BUILD "MDR7"
#else
#define HAL_BUILD "non-MDR7"
#endif

/* 32-byte aligned (the driver's pitch alignment and the M7 cache line). */
BUILD_ASSERT(PITCH % 32U == 0U && OV_PITCH % 32U == 0U, "pitch must be 32-byte aligned");
BUILD_ASSERT(OV_W <= WIDTH && OV_H + CNT_CELL <= HEIGHT, "overlay must fit on screen");
BUILD_ASSERT(8U * CNT_CELL <= OV_W, "overlay counter strip must fit in the overlay");

/*
 * Two buffers per plane. Commit n uses buffer n & 1; it is drawn only after
 * commit n - 1 latched, which released the buffers of commit n - 2.
 */
static uint32_t prim_fb[2][WIDTH * HEIGHT] __aligned(32);
static uint32_t ov_fb[2][OV_W * OV_H] __aligned(32);

static void fill_rect(uint32_t *p, uint32_t stride, uint32_t x0, uint32_t y0, uint32_t w,
		      uint32_t h, uint32_t color)
{
	for (uint32_t y = y0; y < y0 + h; y++) {
		for (uint32_t x = x0; x < x0 + w; x++) {
			p[y * stride + x] = color;
		}
	}
}

/* `bits` cells along the top edge, MSB left: the low bits of the commit number. */
static void draw_counter(uint32_t *p, uint32_t stride, uint32_t bits, uint32_t n)
{
	for (uint32_t b = 0; b < bits; b++) {
		uint32_t on = (n >> (bits - 1U - b)) & 1U;

		fill_rect(p, stride, b * CNT_CELL, 0, CNT_CELL, CNT_CELL, on ? COLOR_ON : COLOR_OFF);
	}
}

static void draw_primary(uint32_t *p, uint32_t n)
{
	fill_rect(p, WIDTH, 0, 0, WIDTH, HEIGHT, COLOR_BG);
	fill_rect(p, WIDTH, (n * BAR_STEP) % (WIDTH - BAR_W), CNT_CELL, BAR_W, HEIGHT - CNT_CELL,
		  COLOR_BAR);
	draw_counter(p, WIDTH, 16U, n);
}

static void draw_overlay(uint32_t *p, uint32_t n)
{
	fill_rect(p, OV_W, 0, 0, OV_W, OV_H, COLOR_OV_EDGE);
	fill_rect(p, OV_W, OV_EDGE, OV_EDGE, OV_W - 2U * OV_EDGE, OV_H - 2U * OV_EDGE,
		  COLOR_OV_FILL);
	/* Same low 8 bits as the primary strip: both must change at one latch. */
	draw_counter(p, OV_W, 8U, n);
}

static void overlay_pos(uint32_t n, uint16_t *x, uint16_t *y)
{
	/* Always fully on screen and below the primary counter strip. */
	*x = (uint16_t)((n * OV_STEP) % (WIDTH - OV_W));
	*y = (uint16_t)(CNT_CELL + (n * OV_STEP) % (HEIGHT - CNT_CELL - OV_H));
}

static uint32_t extdst0_status(void)
{
	return DPU->PIXENG_EXTDST0CFG_EXTDST0_STATUS;
}

static bool sync_busy(uint32_t status)
{
	return (status &
		DISPLAY_SEERIS_PIXENG_EXTDST0CFG_EXTDST0_STATUS_pixeng_extdst0cfg_extdst0_sync_busy_MASK) !=
	       0U;
}

static void dump_regs(const char *when)
{
	const struct {
		const char *name;
		volatile const uint32_t *reg;
	} regs[] = {
		{"COMCTRL_IPIDENTIFIER", &DPU->COMCTRL_IPIDENTIFIER},
		{"EXTDST0CFG_STATIC", &DPU->PIXENG_EXTDST0CFG_EXTDST0_STATIC},
		{"EXTDST0CFG_DYNAMIC", &DPU->PIXENG_EXTDST0CFG_EXTDST0_DYNAMIC},
		{"EXTDST0CFG_STATUS", &DPU->PIXENG_EXTDST0CFG_EXTDST0_STATUS},
		{"EXTDST0_STATICCONTROL", &DPU->PIXENG_EXTDST0_STATICCONTROL},
		{"EXTDST0_STATUS", &DPU->PIXENG_EXTDST0_STATUS},
		{"FETCHLAYER0_STATICCONTROL", &DPU->PIXENG_FETCHLAYER0_STATICCONTROL},
		{"FETCHLAYER0_BASEADDRESS0", &DPU->PIXENG_FETCHLAYER0_BASEADDRESS0},
		{"FETCHLAYER0_LAYEROFFSET0", &DPU->PIXENG_FETCHLAYER0_LAYEROFFSET0},
		{"FETCHLAYER0_LAYERPROPERTY0", &DPU->PIXENG_FETCHLAYER0_LAYERPROPERTY0},
		{"FETCHYUV0_STATICCONTROL", &DPU->PIXENG_FETCHYUV0_STATICCONTROL},
		{"FETCHYUV0_BASEADDRESS0", &DPU->PIXENG_FETCHYUV0_BASEADDRESS0},
		{"FETCHYUV0_LAYEROFFSET0", &DPU->PIXENG_FETCHYUV0_LAYEROFFSET0},
		{"FETCHYUV0_LAYERPROPERTY0", &DPU->PIXENG_FETCHYUV0_LAYERPROPERTY0},
		{"FETCHYUV0_FRAMEDIMENSIONS", &DPU->PIXENG_FETCHYUV0_FRAMEDIMENSIONS},
		{"LAYERBLEND1_STATICCONTROL", &DPU->PIXENG_LAYERBLEND1_STATICCONTROL},
		{"LAYERBLEND1_BLENDCONTROL", &DPU->PIXENG_LAYERBLEND1_BLENDCONTROL},
		{"LAYERBLEND2_STATICCONTROL", &DPU->PIXENG_LAYERBLEND2_STATICCONTROL},
		{"LAYERBLEND2_BLENDCONTROL", &DPU->PIXENG_LAYERBLEND2_BLENDCONTROL},
		{"LAYERBLEND2_POSITION", &DPU->PIXENG_LAYERBLEND2_POSITION},
		{"FRAMEGEN0_FGTIMESTAMP", &DPU->DISENG_FRAMEGEN0_FGTIMESTAMP},
		{"FRAMEGEN0_FGCHSTAT", &DPU->DISENG_FRAMEGEN0_FGCHSTAT},
		{"IRQ_STATUS0_base_block", &DPU->IRQ_INTERRUPTSTATUS0},
	};

	/* Values are raw reads; whether a read returns the shadow or the active copy is open. */
	for (size_t i = 0; i < ARRAY_SIZE(regs); i++) {
		printk("reg,%s,%s,0x%08x\n", when, regs[i].name, *regs[i].reg);
	}
	printk("reg,%s,IRQ_STATUS0_driver_block,0x%08x\n", when,
	       DPU_GetInterruptsPendingFlags(DPU_IRQ_BLOCK, 0));
}

struct latch {
	uint32_t cyc;
	uint32_t timestamp; /* FGTIMESTAMP read right after the latch was seen */
	uint32_t seen_by;   /* bit 0: driver interrupt block, bit 1: DPU base block */
};

static void clear_shdload(void)
{
	DPU_ClearInterruptsPendingFlags(DPU_IRQ_BLOCK, 0, SHDLOAD);
	DPU_ClearInterruptsPendingFlags(DPU, 0, SHDLOAD);
}

/* Poll for the ExtDst0 shadow load. Clears only that bit (design note section 5). */
static bool wait_latch(struct latch *l)
{
	const uint32_t start = k_cycle_get_32();
	const uint32_t timeout = k_us_to_cyc_ceil32(LATCH_TIMEOUT_US);

	for (;;) {
		uint32_t a = DPU_GetInterruptsPendingFlags(DPU_IRQ_BLOCK, 0) & SHDLOAD;
		uint32_t b = DPU_GetInterruptsPendingFlags(DPU, 0) & SHDLOAD;

		if ((a | b) != 0U) {
			l->cyc = k_cycle_get_32();
			l->timestamp = DPU->DISENG_FRAMEGEN0_FGTIMESTAMP;
			l->seen_by = (a != 0U ? 1U : 0U) | (b != 0U ? 2U : 0U);
			clear_shdload();
			return true;
		}
		if (k_cycle_get_32() - start > timeout) {
			return false;
		}
	}
}

static uint32_t frame_index(uint32_t timestamp)
{
	return (timestamp &
		DISPLAY_SEERIS_DISENG_FRAMEGEN0_FGTIMESTAMP_diseng_framegen0_FrameIndex_MASK) >>
	       DISPLAY_SEERIS_DISENG_FRAMEGEN0_FGTIMESTAMP_diseng_framegen0_FrameIndex_SHIFT;
}

static uint32_t line_index(uint32_t timestamp)
{
	return (timestamp &
		DISPLAY_SEERIS_DISENG_FRAMEGEN0_FGTIMESTAMP_diseng_framegen0_LineIndex_MASK) >>
	       DISPLAY_SEERIS_DISENG_FRAMEGEN0_FGTIMESTAMP_diseng_framegen0_LineIndex_SHIFT;
}

/*
 * Write the shadow registers of both planes for commit n: draw, clean the
 * buffers to memory, set both fetch addresses and the overlay position. No
 * trigger here; the caller issues one trigger for both planes.
 */
static void write_layers(uint32_t n, uint16_t ox, uint16_t oy)
{
	uint32_t *prim = prim_fb[n & 1U];
	uint32_t *ov = ov_fb[n & 1U];

	draw_primary(prim, n);
	draw_overlay(ov, n);
	sys_cache_data_flush_range(prim, sizeof(prim_fb[0]));
	sys_cache_data_flush_range(ov, sizeof(ov_fb[0]));

	DPU_SetFetchUnitSrcBufferAddr(DPU, kDPU_FetchLayer0, 0, (uint32_t)(uintptr_t)prim);
	DPU_SetFetchUnitSrcBufferAddr(DPU, kDPU_FetchYuv0, 0, (uint32_t)(uintptr_t)ov);
#if IMX95_M7_OVERLAY_PLACE_BY_POSITION
	DPU->PIXENG_LAYERBLEND2_POSITION =
		DISPLAY_SEERIS_PIXENG_LAYERBLEND2_POSITION_pixeng_layerblend2_XPOS(ox) |
		DISPLAY_SEERIS_PIXENG_LAYERBLEND2_POSITION_pixeng_layerblend2_YPOS(oy);
#else
	DPU_SetFetchUnitOffset(DPU, kDPU_FetchYuv0, 0, ox, oy);
#endif
}

static int setup_layers(void)
{
	dpu_const_frame_config_t cf;
	dpu_layer_blend_config_t lb;
	dpu_fetch_unit_config_t fu;
	dpu_src_buffer_config_t sb;
	status_t st;

	DPU_Init(DPU);
	DPU_PreparePathConfig(DPU);
	DPU_InitPipeline(DPU, kDPU_PipelineExtDst0);

	/* ExtDst0 takes the top LayerBlend. */
	DPU_InitExtDst(DPU, kDPU_ExtDst0, DPU_MAKE_SRC_REG1(kDPU_UnitSrcLayerBlend2));

	/* LayerBlend1: ConstFrame0 under FetchLayer0, primary forced opaque. */
	DPU_InitLayerBlend(DPU, kDPU_LayerBlend1,
			   DPU_MAKE_SRC_REG2(kDPU_UnitSrcConstFrame0, kDPU_UnitSrcFetchLayer0));
	DPU_LayerBlendGetDefaultConfig(&lb);
	lb.constAlpha = 0xFFU;
	lb.primColorBlendMode = kDPU_BlendZero;
	lb.secColorBlendMode = kDPU_BlendConstAlpha;
	lb.primAlphaBlendMode = kDPU_BlendZero;
	lb.secAlphaBlendMode = kDPU_BlendConstAlpha;
	DPU_SetLayerBlendConfig(DPU, kDPU_LayerBlend1, &lb);
	DPU_EnableLayerBlend(DPU, kDPU_LayerBlend1, true);

	/* LayerBlend2: LayerBlend1 under FetchYUV0, coverage blend on source alpha. */
	DPU_InitLayerBlend(DPU, kDPU_LayerBlend2,
			   DPU_MAKE_SRC_REG2(kDPU_UnitSrcLayerBlend1, kDPU_UnitSrcFetchYUV0));
	DPU_LayerBlendGetDefaultConfig(&lb);
	lb.constAlpha = 0xFFU;
	lb.primColorBlendMode = kDPU_BlendSecAlphaInv;
	lb.secColorBlendMode = kDPU_BlendSecAlpha;
	lb.primAlphaBlendMode = kDPU_BlendSecAlphaInv;
	lb.secAlphaBlendMode = kDPU_BlendOne;
	DPU_SetLayerBlendConfig(DPU, kDPU_LayerBlend2, &lb);
	DPU_EnableLayerBlend(DPU, kDPU_LayerBlend2, true);

	DPU_InitDomainBlend(DPU, kDPU_DomainBlend0);

	DPU_ConstFrameGetDefaultConfig(&cf);
	cf.frameWidth = WIDTH;
	cf.frameHeight = HEIGHT;
	cf.constColor = DPU_MAKE_CONST_COLOR(0, 0, 0, 0);
	DPU_InitConstFrame(DPU, kDPU_ConstFrame0);
	DPU_SetConstFrameConfig(DPU, kDPU_ConstFrame0, &cf);

	/* Primary: FetchLayer0, sublayer 0, full screen. */
	DPU_FetchUnitGetDefaultConfig(&fu);
	fu.frameWidth = WIDTH;
	fu.frameHeight = HEIGHT;
	DPU_InitFetchUnit(DPU, kDPU_FetchLayer0, &fu);
	DPU_SrcBufferGetDefaultConfig(&sb);
	sb.bitsPerPixel = 32U;
	sb.pixelFormat = kDPU_PixelFormatARGB8888;
	sb.strideBytes = PITCH;
	sb.bufferWidth = WIDTH;
	sb.bufferHeight = HEIGHT;
	sb.baseAddr = (uint32_t)(uintptr_t)prim_fb[0];
	sb.constColor = DPU_MAKE_CONST_COLOR(0, 0, 0, 0);
	st = DPU_SetFetchUnitSrcBufferConfig(DPU, kDPU_FetchLayer0, 0, &sb);
	if (st != kStatus_Success) {
		printk("primary source buffer rejected by the HAL: %d\n", (int)st);
		return -EINVAL;
	}
	DPU_SetFetchUnitOffset(DPU, kDPU_FetchLayer0, 0, 0, 0);
	DPU_EnableFetchUnitSrcBuffer(DPU, kDPU_FetchLayer0, 0, true);

	/*
	 * Overlay: FetchYUV0 (RGB input, as the stock driver uses it), no FetchECO.
	 * LAYEROFFSET placement: fetch frame = full screen and the area outside the
	 * source buffer takes the constant colour, here fully transparent.
	 * POSITION placement: fetch frame = overlay size, placed by LayerBlend2.
	 */
	DPU_FetchUnitGetDefaultConfig(&fu);
	fu.srcReg = DPU_MAKE_SRC_REG1(kDPU_UnitSrcNone);
#if IMX95_M7_OVERLAY_PLACE_BY_POSITION
	fu.frameWidth = OV_W;
	fu.frameHeight = OV_H;
#else
	fu.frameWidth = WIDTH;
	fu.frameHeight = HEIGHT;
#endif
	DPU_InitFetchUnit(DPU, kDPU_FetchYuv0, &fu);
	DPU_SrcBufferGetDefaultConfig(&sb);
	sb.bitsPerPixel = 32U;
	sb.pixelFormat = kDPU_PixelFormatARGB8888;
	sb.strideBytes = OV_PITCH;
	sb.bufferWidth = OV_W;
	sb.bufferHeight = OV_H;
	sb.baseAddr = (uint32_t)(uintptr_t)ov_fb[0];
	sb.constColor = DPU_MAKE_CONST_COLOR(0, 0, 0, 0);
	st = DPU_SetFetchUnitSrcBufferConfig(DPU, kDPU_FetchYuv0, 0, &sb);
	if (st != kStatus_Success) {
		printk("overlay source buffer rejected by the HAL: %d\n", (int)st);
		return -EINVAL;
	}
	DPU_SetFetchUnitOffset(DPU, kDPU_FetchYuv0, 0, 0, 0);
	/* The HAL has no API for this bit: take alpha from the ARGB8888 pixels. */
	DPU->PIXENG_FETCHYUV0_LAYERPROPERTY0 |=
		DISPLAY_SEERIS_PIXENG_FETCHYUV0_LAYERPROPERTY0_pixeng_fetchyuv0_AlphaSrcEnable0_MASK;
	DPU_EnableFetchUnitSrcBuffer(DPU, kDPU_FetchYuv0, 0, true);

	return 0;
}

/* First enable: display stream, as the stock driver does in dpu_init(). */
static void setup_display(void)
{
	dpu_display_timing_config_t tc;
	dpu_display_config_t dc;

	DPU_EnableInterrupts(DPU_IRQ_BLOCK, 0, SHDLOAD);
	DPU_TriggerDisplayDbShadowLoad(DPU, kDPU_DomainBlend0);

	DPU_DisplayTimingGetDefaultConfig(&tc);
	/* VESA 640x480 has negative syncs. */
	tc.flags = kDPU_DisplayHsyncActiveLow | kDPU_DisplayVsyncActiveLow |
		   kDPU_DisplayDataEnableActiveHigh;
	tc.width = WIDTH;
	tc.hsw = HSW;
	tc.hfp = HFP;
	tc.hbp = HBP;
	tc.height = HEIGHT;
	tc.vsw = VSW;
	/* The stock driver moves one line from the front to the back porch; copied as is. */
	tc.vfp = VFP - 1U;
	tc.vbp = VBP + 1U;
	DPU_InitDisplayTiming(DPU, 0, &tc);

	DPU_DisplayGetDefaultConfig(&dc);
	dc.displayMode = kDPU_DisplayOnlyPrim;
	dc.primAreaStartX = 1;
	dc.primAreaStartY = 1;
	DPU_SetDisplayConfig(DPU, 0, &dc);

	DPU_TriggerDisplayShadowLoad(DPU, 0);
}

int main(void)
{
	struct latch l;
	uint32_t prev_frame = 0;
	uint16_t ox, oy;

	printk("imx95_m7_overlay: two-layer DPU commit, not a verified hardware result\n");
	printk("config,placement,%s\n",
	       IMX95_M7_OVERLAY_PLACE_BY_POSITION ? "LAYERBLEND_POSITION" : "FETCH_LAYEROFFSET");
	printk("config,hal_build,%s\n", HAL_BUILD);
	printk("config,primary,FetchLayer0/sub0->LayerBlend1.sec,%ux%u,pitch %u,opaque\n", WIDTH,
	       HEIGHT, PITCH);
	printk("config,overlay,FetchYUV0->LayerBlend2.sec,%ux%u,pitch %u,source alpha\n", OV_W,
	       OV_H, OV_PITCH);
	printk("config,chain,ConstFrame0->LayerBlend1->LayerBlend2->ExtDst0\n");
	printk("config,buffers,prim %p %p,overlay %p %p\n", (void *)prim_fb[0],
	       (void *)prim_fb[1], (void *)ov_fb[0], (void *)ov_fb[1]);
	printk("config,cache,flush-before-commit\n");

	dump_regs("reset");

	if (setup_layers() != 0) {
		return 0;
	}
	clear_shdload();

	/* Commit 0 together with the first enable of the display stream. */
	overlay_pos(0, &ox, &oy);
	write_layers(0, ox, oy);
	DPU_TriggerPipelineShadowLoad(DPU, kDPU_PipelineExtDst0);
	setup_display();
	DPU_StartDisplay(DPU, 0);
	dump_regs("after_first_trigger");

	if (!wait_latch(&l)) {
		printk("no ExtDst0 shadow load within %u us of the first commit, stopping\n",
		       LATCH_TIMEOUT_US);
		dump_regs("first_latch_timeout");
		return 0;
	}
	prev_frame = frame_index(l.timestamp);
	printk("first latch: frame %u line %u seen_by %u\n", prev_frame, line_index(l.timestamp),
	       l.seen_by);
	dump_regs("after_first_latch");

	printk("commit,ov_x,ov_y,status_before,status_after_trigger,status_after_latch,"
	       "trigger_cyc,latch_cyc,trigger_to_latch_us,frame,line,frame_step,seen_by\n");

	for (uint32_t n = 1; n < COMMITS; n++) {
		uint32_t before, after_trigger, trigger_cyc;

		/* replace_pending = false: never write shadows while a load is armed. */
		before = extdst0_status();
		if (sync_busy(before)) {
			printk("commit %u refused: previous shadow load still pending (0x%08x)\n",
			       n, before);
			return 0;
		}

		overlay_pos(n, &ox, &oy);
		write_layers(n, ox, oy);

		trigger_cyc = k_cycle_get_32();
		DPU_TriggerPipelineShadowLoad(DPU, kDPU_PipelineExtDst0);
		after_trigger = extdst0_status();

		if (!wait_latch(&l)) {
			printk("commit %u: no ExtDst0 shadow load within %u us, stopping\n", n,
			       LATCH_TIMEOUT_US);
			dump_regs("latch_timeout");
			return 0;
		}

		uint32_t frame = frame_index(l.timestamp);
		uint32_t step = (frame - prev_frame) &
				(DISPLAY_SEERIS_DISENG_FRAMEGEN0_FGTIMESTAMP_diseng_framegen0_FrameIndex_MASK >>
				 DISPLAY_SEERIS_DISENG_FRAMEGEN0_FGTIMESTAMP_diseng_framegen0_FrameIndex_SHIFT);

		/* Unsigned subtraction survives wrap of the 32-bit cycle counter. */
		printk("%u,%u,%u,0x%08x,0x%08x,0x%08x,%u,%u,%u,%u,%u,%u,%u\n", n, ox, oy, before,
		       after_trigger, extdst0_status(), trigger_cyc, l.cyc,
		       (uint32_t)k_cyc_to_us_floor32(l.cyc - trigger_cyc), frame,
		       line_index(l.timestamp), step, l.seen_by);
		prev_frame = frame;
	}

	dump_regs("end");
	printk("done\n");
	return 0;
}

#endif /* IMX95_M7_OVERLAY_ENABLE_DPU */
