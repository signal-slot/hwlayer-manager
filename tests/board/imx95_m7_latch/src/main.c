/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * Latch reporting test for the i.MX95 M7 DPU (SoW M2, WP3; M2-04).
 *
 * The application programs the DPU itself through the unchanged NXP MCUX DPU
 * HAL (hal_nxp dpu_1) and owns the DPU interrupt. Upstream Zephyr is not
 * changed and its socionext,dpu driver is not built. Compared with that
 * driver's display_write(), which blocks forever on a semaphore, this
 * application:
 *
 *  - commits without blocking: commit() writes the fetch address, triggers
 *    the ExtDst0 pipeline shadow load and returns; a second commit while one
 *    is pending is refused with -EBUSY;
 *  - counts latches from the ExtDst0 shadow-load interrupt, clearing only that
 *    bit, and stamps each with the DWT cycle counter, k_cycle_get_32() and
 *    the FrameGen0 FGTIMESTAMP register (frame and line index);
 *  - waits for each latch with a timeout, and on timeout reads the interrupt
 *    status to tell "no shadow load" from "shadow load without interrupt".
 *
 * Events go to a RAM ring during the run and are printed after it, as
 * CETRACE v=1 lines (docs/trace-format.md), then one RESULT and one VERDICT
 * line. The application converts no count to time; the host does that from
 * the frequencies in the hdr line.
 */

#include <zephyr/kernel.h>
#include <zephyr/sys/printk.h>

#if !IS_ENABLED(CONFIG_IMX95_M7_LATCH_ENABLE_DPU)

int main(void)
{
	printk("imx95_m7_latch: refusing to touch the DPU.\n");
	printk("Linux may own it. Rebuild with CONFIG_IMX95_M7_LATCH_ENABLE_DPU=y "
	       "only if the M7 owns the DPU on this board.\n");
	printk("VERDICT imx95_m7_latch: SKIP DPU access disabled at build time\n");
	return 0;
}

#else

#include <zephyr/cache.h>
#include <zephyr/devicetree.h>
#include <zephyr/irq.h>
#include <zephyr/sys/sys_io.h>
#include <zephyr/sys/util.h>
#include <zephyr/timing/timing.h>
#include <zephyr/version.h>
#include <zephyr/dt-bindings/display/panel.h>
#include <fsl_dpu.h>

BUILD_ASSERT(!IS_ENABLED(CONFIG_DISPLAY_SOCIONEXT_DPU),
	     "the upstream DPU driver owns the same interrupt; keep it disabled");

#define DPU_NODE    DT_NODELABEL(dpu)
#define TIMING_NODE DT_CHILD(DPU_NODE, display_timings)

BUILD_ASSERT(DT_NODE_HAS_STATUS_OKAY(DPU_NODE), "enable the dpu node in a board overlay");
BUILD_ASSERT(DT_PROP(DPU_NODE, pixel_format) == PANEL_PIXEL_FORMAT_ARGB_8888,
	     "this application only builds ARGB8888 buffers");

#define WIDTH        DT_PROP(DPU_NODE, width)
#define HEIGHT       DT_PROP(DPU_NODE, height)
#define PITCH_ALIGN  32U
#define STRIDE_BYTES ROUND_UP(WIDTH * 4U, PITCH_ALIGN)
#define STRIDE_PX    (STRIDE_BYTES / 4U)

#define FRAMES     CONFIG_IMX95_M7_LATCH_FRAMES
#define TIMEOUT_MS CONFIG_IMX95_M7_LATCH_TIMEOUT_MS
#define RING_LEN   CONFIG_IMX95_M7_LATCH_RING

/* commit + latched + released per frame, plus a latch_timeout. */
BUILD_ASSERT(RING_LEN >= 3 * FRAMES + 1,
	     "CONFIG_IMX95_M7_LATCH_RING is too small for CONFIG_IMX95_M7_LATCH_FRAMES");

#define DPU_BASE ((uintptr_t)DT_REG_ADDR(DPU_NODE))
/* Pixel-engine and FrameGen registers: devicetree base, as the HAL expects. */
#define DPU      ((DISPLAY_SEERIS_Type *)DPU_BASE)
/* Interrupt registers: see CONFIG_IMX95_M7_LATCH_IRQ_BANK_OFFSET. */
#define DPU_IRQ  ((DISPLAY_SEERIS_Type *)(DPU_BASE + CONFIG_IMX95_M7_LATCH_IRQ_BANK_OFFSET))

#define LATCH_BIT ((uint32_t)kDPU_Group0ExtDst0ShadowLoadInterrupt)

/*
 * FrameGen FGTIMESTAMP. The HAL has no accessor; its private
 * DPU_FRAMEGEN_Type (fsl_dpu.c) puts FGTIMESTAMP at offset 0x98. The NXP
 * Linux dpu95 driver (linux-imx 5a0a5e71d2bd, dpu95-framegen.c and dpu95.h)
 * uses the same offset, FRAMEINDEX_SHIFT 14, LINEINDEX_MASK 0x3fff and a
 * frame index maximum of 0x3ffff (18 bits).
 * The raw value is logged as well, so the host can decode it differently.
 */
#define FG_TIMESTAMP_OFFSET 0x98U
#define FG_LINE_BITS        14U
#define FG_LINE_MASK        BIT_MASK(FG_LINE_BITS)
#define FG_FRAME_BITS       (32U - FG_LINE_BITS)

static uint32_t fb[2][HEIGHT * STRIDE_PX] __aligned(32);

enum ev_kind {
	EV_COMMIT,
	EV_LATCHED,
	EV_RELEASED,
	EV_LATCH_TIMEOUT,
};

struct rec {
	uint8_t kind;
	uint8_t buf;
	uint32_t sub;
	uint32_t t;      /* k_cycle_get_32() */
	uint32_t dwt;    /* DWT CYCCNT through the timing API */
	uint32_t fgts;   /* FrameGen0 FGTIMESTAMP, raw */
	uint32_t status; /* interrupt group 0 status (latched, latch_timeout) */
};

static struct rec ring[RING_LEN];
static uint32_t ring_used;
static uint32_t ring_overrun;

struct latch_msg {
	uint32_t sub;
	uint8_t buf;
};

K_MSGQ_DEFINE(latch_q, sizeof(struct latch_msg), 4, 4);

/* Shared with the ISR; written by the thread only with interrupts locked. */
static volatile bool pending;
static volatile uint32_t pending_sub;
static volatile uint8_t pending_buf;

static volatile uint32_t isr_entries;
static volatile uint32_t isr_other;      /* entries without the latch bit */
static volatile uint32_t isr_unexpected; /* latch bit with nothing pending */
static volatile uint32_t isr_q_full;

static uint32_t next_sub;
static bool started;

static inline uint32_t fg_timestamp(void)
{
	return sys_read32((mem_addr_t)(DPU_BASE + DPU_FRAME_GEN0_OFFSET + FG_TIMESTAMP_OFFSET));
}

static inline uint32_t dwt_now(void)
{
	/* The Cortex-M DWT counter is 32 bits wide; the host unwraps it. */
	return (uint32_t)timing_counter_get();
}

/* Caller runs in the ISR or holds irq_lock(). */
static void ring_put(enum ev_kind kind, uint8_t buf, uint32_t sub, uint32_t t, uint32_t dwt,
		     uint32_t fgts, uint32_t status)
{
	if (ring_used >= RING_LEN) {
		ring_overrun++;
		return;
	}
	ring[ring_used++] = (struct rec){
		.kind = kind,
		.buf = buf,
		.sub = sub,
		.t = t,
		.dwt = dwt,
		.fgts = fgts,
		.status = status,
	};
}

static void latch_isr(const void *arg)
{
	/* Stamp first, before any register access. */
	uint32_t dwt = dwt_now();
	uint32_t t = k_cycle_get_32();
	uint32_t status = DPU_GetInterruptsPendingFlags(DPU_IRQ, 0);
	uint32_t fgts = fg_timestamp();
	struct latch_msg msg;

	ARG_UNUSED(arg);
	isr_entries++;

	if ((status & LATCH_BIT) == 0U) {
		isr_other++;
		return;
	}
	/* Clear only the bit this application owns. */
	DPU_ClearInterruptsPendingFlags(DPU_IRQ, 0, LATCH_BIT);

	if (!pending) {
		isr_unexpected++;
		return;
	}

	ring_put(EV_LATCHED, pending_buf, pending_sub, t, dwt, fgts, status);
	msg.sub = pending_sub;
	msg.buf = pending_buf;
	pending = false;
	if (k_msgq_put(&latch_q, &msg, K_NO_WAIT) != 0) {
		isr_q_full++;
	}
}

/*
 * Non-blocking commit of fb[idx]: program the shadow address, arm one shadow
 * load and return. Refused with -EBUSY while an earlier commit has not
 * latched (no replacement of a pending commit, as in
 * docs/imx95-m7-overlay-design.md section 3).
 */
static int commit(uint8_t idx)
{
	unsigned int key = irq_lock();

	if (pending) {
		irq_unlock(key);
		return -EBUSY;
	}

	if (!started) {
		DPU_StartDisplay(DPU, 0);
		started = true;
	}

	DPU_SetFetchUnitSrcBufferAddr(DPU, kDPU_FetchYuv0, 0, (uint32_t)(uintptr_t)fb[idx]);
	pending_sub = ++next_sub;
	pending_buf = idx;
	pending = true;
	DPU_TriggerPipelineShadowLoad(DPU, kDPU_PipelineExtDst0);

	/* Commit "return" time; the ISR cannot run before this record exists. */
	ring_put(EV_COMMIT, idx, pending_sub, k_cycle_get_32(), dwt_now(), fg_timestamp(), 0);
	irq_unlock(key);
	return 0;
}

static void fill_buffers(void)
{
	static const uint32_t colour[2] = {0xFF2040C0U, 0xFFC04020U};

	for (uint32_t i = 0; i < 2U; i++) {
		for (uint32_t p = 0; p < ARRAY_SIZE(fb[i]); p++) {
			fb[i][p] = colour[i];
		}
		/* A white square at a buffer-specific place, to tell them apart. */
		for (uint32_t y = 16U; y < 48U; y++) {
			for (uint32_t x = 16U + i * 48U; x < 48U + i * 48U; x++) {
				fb[i][y * STRIDE_PX + x] = 0xFFFFFFFFU;
			}
		}
	}
	/* The buffers are never written again; clean them once for the DPU. */
	sys_cache_data_flush_range(fb, sizeof(fb));
}

/* The same pipeline as the upstream driver's dpu_init(), with fb[0]. */
static void dpu_setup(void)
{
	dpu_const_frame_config_t cf;
	dpu_layer_blend_config_t lb;
	dpu_fetch_unit_config_t fetch;
	dpu_src_buffer_config_t sb;
	dpu_display_timing_config_t timing;
	dpu_display_config_t display;

	DPU_Init(DPU);
	DPU_PreparePathConfig(DPU);
	DPU_InitPipeline(DPU, kDPU_PipelineExtDst0);
	DPU_InitExtDst(DPU, kDPU_ExtDst0, DPU_MAKE_SRC_REG1(kDPU_UnitSrcLayerBlend1));

	DPU_InitLayerBlend(DPU, kDPU_LayerBlend1,
			   DPU_MAKE_SRC_REG2(kDPU_UnitSrcConstFrame0, kDPU_UnitSrcFetchYUV0));
	DPU_LayerBlendGetDefaultConfig(&lb);
	DPU_SetLayerBlendConfig(DPU, kDPU_LayerBlend1, &lb);
	DPU_EnableLayerBlend(DPU, kDPU_LayerBlend1, true);

	DPU_InitDomainBlend(DPU, kDPU_DomainBlend0);

	cf.frameWidth = WIDTH;
	cf.frameHeight = HEIGHT;
	cf.constColor = DPU_MAKE_CONST_COLOR(0, 0, 0, 0);
	DPU_InitConstFrame(DPU, kDPU_ConstFrame0);
	DPU_SetConstFrameConfig(DPU, kDPU_ConstFrame0, &cf);

	DPU_FetchUnitGetDefaultConfig(&fetch);
	fetch.frameWidth = WIDTH;
	fetch.frameHeight = HEIGHT;
	DPU_InitFetchUnit(DPU, kDPU_FetchYuv0, &fetch);
	DPU_SrcBufferGetDefaultConfig(&sb);
	sb.bitsPerPixel = 32;
	sb.pixelFormat = kDPU_PixelFormatARGB8888;
	sb.constColor = DPU_MAKE_CONST_COLOR(0, 0, 0, 0);
	sb.strideBytes = STRIDE_BYTES;
	sb.bufferWidth = WIDTH;
	sb.bufferHeight = HEIGHT;
	sb.baseAddr = (uint32_t)(uintptr_t)fb[0];
	DPU_SetFetchUnitSrcBufferConfig(DPU, kDPU_FetchYuv0, 0, &sb);
	DPU_SetFetchUnitOffset(DPU, kDPU_FetchYuv0, 0, 0, 0);
	DPU_EnableFetchUnitSrcBuffer(DPU, kDPU_FetchYuv0, 0, true);

	/* The driver arms one content-stream load here; keep the same order. */
	DPU_TriggerPipelineShadowLoad(DPU, kDPU_PipelineExtDst0);

	/* Drop any stale latch flag before the interrupt is enabled. */
	DPU_ClearInterruptsPendingFlags(DPU_IRQ, 0, LATCH_BIT);
	DPU_EnableInterrupts(DPU_IRQ, 0, LATCH_BIT);

	DPU_TriggerDisplayDbShadowLoad(DPU, kDPU_DomainBlend0);

	DPU_DisplayTimingGetDefaultConfig(&timing);
	/* Polarity mapping copied unchanged from the upstream driver. */
	timing.flags = (DT_PROP(TIMING_NODE, hsync_active) ? kDPU_DisplayHsyncActiveLow
							    : kDPU_DisplayHsyncActiveHigh) |
		       (DT_PROP(TIMING_NODE, vsync_active) ? kDPU_DisplayVsyncActiveLow
							    : kDPU_DisplayVsyncActiveHigh) |
		       (DT_PROP(TIMING_NODE, de_active) ? kDPU_DisplayDataEnableActiveLow
							 : kDPU_DisplayDataEnableActiveHigh);
	timing.width = WIDTH;
	timing.hsw = DT_PROP(TIMING_NODE, hsync_len);
	timing.hfp = DT_PROP(TIMING_NODE, hfront_porch);
	timing.hbp = DT_PROP(TIMING_NODE, hback_porch);
	timing.height = HEIGHT;
	timing.vsw = DT_PROP(TIMING_NODE, vsync_len);
	timing.vfp = DT_PROP(TIMING_NODE, vfront_porch) - 1U;
	timing.vbp = DT_PROP(TIMING_NODE, vback_porch) + 1U;
	DPU_InitDisplayTiming(DPU, 0, &timing);

	DPU_DisplayGetDefaultConfig(&display);
	display.displayMode = kDPU_DisplayOnlyPrim;
	display.primAreaStartX = 1;
	display.primAreaStartY = 1;
	DPU_SetDisplayConfig(DPU, 0, &display);

	DPU_TriggerDisplayShadowLoad(DPU, 0);
}

static const char *const ev_name[] = {
	[EV_COMMIT] = "commit",
	[EV_LATCHED] = "latched",
	[EV_RELEASED] = "released",
	[EV_LATCH_TIMEOUT] = "latch_timeout",
};

static void print_rec(uint32_t seq, const struct rec *r)
{
	uint32_t fi = r->fgts >> FG_LINE_BITS;
	uint32_t line = r->fgts & FG_LINE_MASK;

	switch (r->kind) {
	case EV_COMMIT:
		printk("CETRACE v=1 ev=commit seq=%u t=%u at=%u out=0 result=ok sub=%u buf=%u "
		       "dwt=%u fgts=0x%08x fi=%u line=%u\n",
		       seq, r->t, r->t, r->sub, r->buf, r->dwt, r->fgts, fi, line);
		break;
	case EV_LATCHED:
		printk("CETRACE v=1 ev=latched seq=%u t=%u at=%u out=0 sub=%u tev=%u buf=%u "
		       "dwt=%u fgts=0x%08x fi=%u line=%u st=0x%08x\n",
		       seq, r->t, r->t, r->sub, r->t, r->buf, r->dwt, r->fgts, fi, line,
		       r->status);
		break;
	case EV_RELEASED:
		printk("CETRACE v=1 ev=released seq=%u t=%u at=%u buf=%u sub=%u dwt=%u\n", seq,
		       r->t, r->t, r->buf, r->sub, r->dwt);
		break;
	default:
		printk("CETRACE v=1 ev=%s seq=%u t=%u at=%u out=0 sub=%u buf=%u dwt=%u "
		       "fgts=0x%08x fi=%u line=%u st=0x%08x\n",
		       ev_name[r->kind], seq, r->t, r->t, r->sub, r->buf, r->dwt, r->fgts, fi,
		       line, r->status);
		break;
	}
}

int main(void)
{
	uint32_t committed = 0, latched = 0, released = 0, timeouts = 0;
	uint32_t busy_refused = 0, busy_accepted = 0;
	uint32_t fi_step1 = 0, fi_step_other = 0;
	uint32_t timeout_status = 0, timeout_sub = 0;
	uint32_t on_sub = 0, t0, dwt_hz, cyc_hz;
	uint8_t on_buf = 0, idx = 0;
	bool have_prev_fi = false;
	uint32_t prev_fi = 0;

	printk("imx95_m7_latch: %ux%u ARGB8888, %u commits, timeout %u ms, "
	       "irq bank offset 0x%x\n",
	       WIDTH, HEIGHT, FRAMES, TIMEOUT_MS, CONFIG_IMX95_M7_LATCH_IRQ_BANK_OFFSET);

	timing_init();
	timing_start();
	dwt_hz = (uint32_t)timing_freq_get();
	cyc_hz = (uint32_t)sys_clock_hw_cycles_per_sec();

	fill_buffers();

	/* A bus fault after this line means the M7 may not access the DPU. */
	printk("imx95_m7_latch: programming the DPU\n");
	IRQ_CONNECT(DT_IRQN(DPU_NODE), DT_IRQ(DPU_NODE, priority), latch_isr, NULL, 0);
	dpu_setup();
	irq_enable(DT_IRQN(DPU_NODE));
	printk("imx95_m7_latch: run start; log follows after the run\n");

	t0 = k_cycle_get_32();
	for (uint32_t n = 0; n < FRAMES; n++) {
		struct latch_msg msg;
		unsigned int key;
		int ret, probe;

		/*
		 * Commit, then try a second commit in the same locked section,
		 * where the latch interrupt cannot have run: it must be refused.
		 */
		key = irq_lock();
		ret = commit(idx);
		probe = commit(idx ^ 1U);
		irq_unlock(key);

		if (ret != 0) {
			/* Cannot happen: the previous commit latched or the run ended. */
			break;
		}
		committed++;
		if (probe == -EBUSY) {
			busy_refused++;
		} else {
			busy_accepted++;
			break;
		}

		if (k_msgq_get(&latch_q, &msg, K_MSEC(TIMEOUT_MS)) != 0) {
			key = irq_lock();
			timeout_status = DPU_GetInterruptsPendingFlags(DPU_IRQ, 0);
			timeout_sub = pending_sub;
			ring_put(EV_LATCH_TIMEOUT, pending_buf, pending_sub, k_cycle_get_32(),
				 dwt_now(), fg_timestamp(), timeout_status);
			irq_unlock(key);
			timeouts++;
			break;
		}
		latched++;

		/* The latch took the previous buffer off screen: release it. */
		if (on_sub != 0U) {
			key = irq_lock();
			ring_put(EV_RELEASED, on_buf, on_sub, k_cycle_get_32(), dwt_now(), 0, 0);
			irq_unlock(key);
			released++;
		}
		on_sub = msg.sub;
		on_buf = msg.buf;
		idx ^= 1U;
	}

	irq_disable(DT_IRQN(DPU_NODE));
	DPU_DisableInterrupts(DPU_IRQ, 0, LATCH_BIT);
	DPU_StopDisplay(DPU, 0);

	/* Output, after the measured path. */
	printk("CETRACE v=1 ev=hdr seq=0 src=zephyr clock=cyc32 freq_hz=%u t0=%u at_unit=same "
	       "dwt_hz=%u mode=%ux%u timeout_ms=%u irq_bank_off=0x%x "
	       "build=\"zephyr %s, tests/board/imx95_m7_latch\" board=\"%s\"\n",
	       cyc_hz, t0, dwt_hz, WIDTH, HEIGHT, TIMEOUT_MS,
	       CONFIG_IMX95_M7_LATCH_IRQ_BANK_OFFSET, KERNEL_VERSION_STRING, CONFIG_BOARD);
	for (uint32_t i = 0; i < ring_used; i++) {
		const struct rec *r = &ring[i];

		print_rec(i + 1U, r);
		if (r->kind == EV_LATCHED) {
			uint32_t fi = r->fgts >> FG_LINE_BITS;

			if (have_prev_fi) {
				uint32_t step = (fi - prev_fi) & BIT_MASK(FG_FRAME_BITS);

				if (step == 1U) {
					fi_step1++;
				} else {
					fi_step_other++;
				}
			}
			prev_fi = fi;
			have_prev_fi = true;
		}
	}

	printk("RESULT imx95_m7_latch: frames=%u committed=%u latched=%u released=%u dropped=0 "
	       "timeouts=%u busy_refused=%u isr=%u isr_other=%u unexpected_latch=%u "
	       "queue_full=%u fi_step1=%u fi_step_other=%u records=%u overrun=%u\n",
	       FRAMES, committed, latched, released, timeouts, busy_refused, isr_entries,
	       isr_other, isr_unexpected, isr_q_full, fi_step1, fi_step_other, ring_used,
	       ring_overrun);

	/* Verdict: latch reporting only. No timing is judged here. */
	if (timeouts != 0U && (timeout_status & LATCH_BIT) != 0U) {
		printk("VERDICT imx95_m7_latch: BLOCKED sub=%u ExtDst0 shadow-load flag set but "
		       "no interrupt within %u ms (interrupt routing)\n",
		       timeout_sub, TIMEOUT_MS);
	} else if (timeouts != 0U) {
		printk("VERDICT imx95_m7_latch: FAIL sub=%u no ExtDst0 shadow load within %u ms\n",
		       timeout_sub, TIMEOUT_MS);
	} else if (busy_accepted != 0U) {
		printk("VERDICT imx95_m7_latch: FAIL a commit was accepted while one was pending\n");
	} else if (isr_unexpected != 0U || isr_q_full != 0U) {
		printk("VERDICT imx95_m7_latch: FAIL %u latch interrupts without a pending commit, "
		       "%u lost\n",
		       isr_unexpected, isr_q_full);
	} else if (ring_overrun != 0U) {
		printk("VERDICT imx95_m7_latch: FAIL event log incomplete, %u records lost\n",
		       ring_overrun);
	} else if (committed != FRAMES || latched != committed) {
		printk("VERDICT imx95_m7_latch: FAIL committed=%u latched=%u of %u\n", committed,
		       latched, FRAMES);
	} else {
		printk("VERDICT imx95_m7_latch: PASS %u commits, %u latched by the ExtDst0 "
		       "shadow-load interrupt, %u released, 0 timeouts\n",
		       committed, latched, released);
	}
	return 0;
}

#endif /* CONFIG_IMX95_M7_LATCH_ENABLE_DPU */
