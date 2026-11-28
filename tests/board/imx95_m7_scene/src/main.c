/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * The WP3 comparison scene on the i.MX95 Cortex-M7 under upstream Zephyr,
 * driven through the composition engine's C ABI (libcomposition_engine.a,
 * as in tests/board/imx95_m7_engine) and written as docs/trace-format.md
 * lines, so that scripts/trace-to-metrics.py and m2-compare-report.py can
 * read the log next to the Linux one (crates/composition-drm/examples/
 * m2scene.rs).
 *
 * The scene is the one pinned by crates/composition-core/tests/m2_scene.rs
 * (docs/m2-linux-zephyr-comparison.md section 2):
 *   base: buffer 1 (slot 0) or 3 (slot 1), W x H, full output, z 0,
 *         ARGB8888 linear, opaque blend, no plane alpha;
 *   top (stage B only): buffer 2 or 4, W/3 x H/3, ARGB8888,
 *         premultiplied, plane alpha CONFIG_IMX95_M7_SCENE_TOP_ALPHA,
 *         z 1, destination (W*500/1280, H*300/800).
 * Every frame presents the same picture from the other slot.
 *
 * The backend describes the stock Zephyr DPU driver as tests/board/
 * imx95_m7_engine does (one primary plane, RGB565/RGB888/ARGB8888, opaque,
 * no scaling, full output only; read from source, not measured). Its commit
 * either calls display_write() on the DPU, the path of tests/board/
 * imx95_m7_scanout (CONFIG_IMX95_M7_SCENE_ENABLE_DPU=y), or, by default,
 * touches nothing and reports the latch at once
 * (CONFIG_IMX95_M7_SCENE_STUB_LATCH). See README.md.
 *
 * Which line comes from where (the C ABI has no trace hook, so these are
 * written by this file, in the order the core's trace would use):
 *   check     backend check callback, one per candidate the planner tries
 *   validate  after ce_present(): ok with the accepted candidate, or the
 *             planning error; t = when planning ended (commit callback entry)
 *   commit    after ce_present() returns, when planning succeeded
 *   latched / released / dropped / latch_timeout / device_lost /
 *   caps_changed   from ce_poll_event(), stamped when polled
 */
#include <zephyr/kernel.h>
#include <zephyr/sys/printk.h>
#include <zephyr/sys/util.h>
#include <zephyr/version.h>

#if defined(CONFIG_IMX95_M7_SCENE_ENABLE_DPU)
#include <zephyr/device.h>
#include <zephyr/drivers/display.h>
#endif

#include <inttypes.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "composition_engine.h"
#include "trace.h"

/* Not in the header: DRM's RGB888 (packed 24 bit). */
#define APP_FORMAT_RGB888 CE_FOURCC('R', 'G', '2', '4')

#if defined(CONFIG_IMX95_M7_SCENE_ENABLE_DPU)
#define DISP_NODE DT_CHOSEN(zephyr_display)
#define OUT_W     DT_PROP(DISP_NODE, width)
#define OUT_H     DT_PROP(DISP_NODE, height)
#define LATCH_TEXT "dpu-write-return"
#else
/* The panel size the docs assume (inferred, not measured). */
#define OUT_W      1280
#define OUT_H      800
#define LATCH_TEXT "stub"
#endif

#if defined(CONFIG_IMX95_M7_SCENE_STAGE_B)
#define STAGE_TEXT "B"
#define LAYERS     2
#else
#define STAGE_TEXT "A"
#define LAYERS     1
#endif

#define TOP_W ((OUT_W / 3) > 0 ? (OUT_W / 3) : 1)
#define TOP_H ((OUT_H / 3) > 0 ? (OUT_H / 3) : 1)
#define TOP_X (OUT_W * 500 / 1280)
#define TOP_Y (OUT_H * 300 / 800)

/* Flat, saturated, opaque (premultiplied the same). */
#define COLOR_BASE 0xFF0030C0U

enum { OUT_ID = 0, PLANE_PRIMARY = 1 };

/* Slot s uses base buffer slot_base[s] and top buffer slot_top[s]. */
static const uint32_t slot_base[2] = { 1, 3 };
static const uint32_t slot_top[2] = { 2, 4 };

#if defined(CONFIG_IMX95_M7_SCENE_ENABLE_DPU)
/*
 * Full-frame, 32-byte aligned, pitch-matching buffers take the driver's
 * zero-copy path (as in imx95_m7_scanout). Only the base layer is ever
 * shown: the stock driver has no second plane.
 */
static uint32_t fb[2][OUT_W * OUT_H] __aligned(32);
#endif

struct scene_backend {
#if defined(CONFIG_IMX95_M7_SCENE_ENABLE_DPU)
	const struct device *dev;
#endif
	bool latch_pending;
	uint64_t latch_submission;
	/* Per ce_present() call. */
	bool accepted;
	ce_assignment accepted_assignments[CE_MAX_LAYERS];
	size_t accepted_count;
	uint32_t rejected;
	bool committed;
	uint64_t plan_end;
};

static void scene_capabilities(void *ctx, ce_capabilities *out)
{
	(void)ctx;
	out->output_count = 1;
	/* The refresh rate is not known here: 0. */
	out->outputs[0] = (ce_output_caps){
		.id = OUT_ID, .width = OUT_W, .height = OUT_H, .refresh_mhz = 0,
	};
	out->plane_count = 1;
	out->planes[0] = (ce_plane_caps){
		.id = PLANE_PRIMARY,
		.kind = CE_PLANE_PRIMARY,
		.outputs = 1u << 0,
		.format_count = 3,
		.can_scale = 0,
		.plane_alpha = 0,
		.blend_modes = 1u << CE_BLEND_OPAQUE,
		.transforms = 1u << CE_TRANSFORM_NORMAL,
		.zpos_min = 0,
		.zpos_max = 0,
		.zpos_mutable = 0,
		/* One framebuffer for the whole panel (inferred from the driver). */
		.full_output_only = 1,
	};
	out->planes[0].formats[0] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_RGB565 };
	out->planes[0].formats[1] = (ce_format){ CE_MODIFIER_LINEAR, APP_FORMAT_RGB888 };
	out->planes[0].formats[2] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_ARGB8888 };
	/* display_write() blocks until the latch: no replacement. */
	out->replace_pending = 0;
}

static ce_status scene_check(void *ctx, const ce_frame *frame, const ce_assignment *assignments,
			     size_t count)
{
	struct scene_backend *be = ctx;
	uint64_t t = trace_now();
	ce_status result = CE_OK;

	for (size_t i = 0; i < count; i++) {
		if (assignments[i].plane_index != 0 || assignments[i].layer >= frame->layer_count) {
			result = CE_ERR_BACKEND_REJECTED;
		}
	}
	trace_check(t, frame->output, assignments, count, result);
	if (result == CE_OK) {
		/* The planner takes the first candidate the backend accepts. */
		be->accepted = true;
		be->accepted_count = MIN(count, ARRAY_SIZE(be->accepted_assignments));
		memcpy(be->accepted_assignments, assignments,
		       be->accepted_count * sizeof(assignments[0]));
	} else {
		be->rejected++;
	}
	return result;
}

static ce_status scene_commit(void *ctx, uint64_t submission, const ce_frame *frame,
			      const ce_assignment *assignments, size_t count)
{
	struct scene_backend *be = ctx;

	be->plan_end = trace_now();
	be->committed = true;
#if defined(CONFIG_IMX95_M7_SCENE_ENABLE_DPU)
	if (count > 0) {
		uint32_t buffer = frame->layers[assignments[0].layer].buffer;
		const uint32_t *pixels = buffer == slot_base[1] ? fb[1] : fb[0];
		struct display_buffer_descriptor desc = {
			.buf_size = sizeof(fb[0]),
			.width = OUT_W,
			.height = OUT_H,
			.pitch = OUT_W,
		};
		/*
		 * Blocks until the ExtDst0 shadow-load interrupt (read from
		 * source, imx95_m7_scanout README). Its return is the nearest
		 * latch signal the stock driver gives.
		 */
		int ret = display_write(be->dev, 0, 0, &desc, pixels);

		if (ret != 0) {
			/* Passed on as backend.platform errno=<ret>. */
			return ret;
		}
	}
#else
	ARG_UNUSED(frame);
	ARG_UNUSED(assignments);
	ARG_UNUSED(count);
#endif
	be->latch_pending = true;
	be->latch_submission = submission;
	return CE_OK;
}

static bool scene_poll_event(void *ctx, ce_backend_event *out)
{
	struct scene_backend *be = ctx;

	if (!be->latch_pending) {
		return false;
	}
	be->latch_pending = false;
	out->kind = CE_BACKEND_LATCHED;
	out->output = OUT_ID;
	out->submission = be->latch_submission;
	return true;
}

/* Gives the engine the time; the following lines carry it as at=. */
static void advance(ce_engine *engine)
{
	uint64_t now = trace_engine_time();

	trace_set_at(now);
	ce_advance_time(engine, now);
}

struct counters {
	uint32_t committed;
	uint32_t latched;
	uint32_t dropped;
	uint32_t timeouts;
	bool has_on_screen;
	uint64_t on_screen;
};

/*
 * Takes every queued event and writes its line. A released buffer belongs
 * to the submission the latch before it took off screen, or to the dropped
 * submission before it (the attribution rule of engine.rs trace_events).
 * Returns false on an engine error.
 */
static bool drain(ce_engine *engine, struct counters *c)
{
	bool has_owner = false;
	uint64_t owner = 0;
	ce_event ev;
	int32_t r;

	advance(engine);
	while ((r = ce_poll_event(engine, &ev)) == 1) {
		uint64_t t = trace_now();

		switch (ev.kind) {
		case CE_EVENT_PRESENTED:
			trace_latched(t, ev.output, ev.submission);
			has_owner = c->has_on_screen;
			owner = c->on_screen;
			c->has_on_screen = true;
			c->on_screen = ev.submission;
			c->latched++;
			break;
		case CE_EVENT_DROPPED:
			/*
			 * This backend never replaces a pending frame and never
			 * loses the device, so a drop can only be "replaced".
			 */
			trace_dropped(t, ev.output, ev.submission);
			has_owner = true;
			owner = ev.submission;
			c->dropped++;
			break;
		case CE_EVENT_RELEASED:
			trace_released(t, ev.buffer, has_owner, owner);
			break;
		case CE_EVENT_DEVICE_LOST:
			trace_device_lost(t);
			has_owner = false;
			c->has_on_screen = false;
			break;
		case CE_EVENT_CAPABILITIES_CHANGED:
			trace_caps_changed(t);
			break;
		case CE_EVENT_LATCH_TIMEOUT:
			trace_latch_timeout(t, ev.output, ev.submission);
			c->timeouts++;
			break;
		default:
			printk("SCENE_ERROR unknown event kind %u\n", ev.kind);
			return false;
		}
	}
	if (r < 0) {
		printk("SCENE_ERROR ce_poll_event status %" PRId32 "\n", r);
		return false;
	}
	return true;
}

/*
 * Presents one frame and writes its validate and commit lines. Returns the
 * status of ce_present(); *info says why it failed.
 */
static ce_status present(ce_engine *engine, struct scene_backend *be, const ce_layer *layers,
			 size_t count, uint64_t *submission, ce_error_info *info)
{
	const ce_frame frame = { .layers = layers, .layer_count = count, .output = OUT_ID };
	uint64_t start, done;
	ce_status s;

	be->accepted = false;
	be->accepted_count = 0;
	be->rejected = 0;
	be->committed = false;

	advance(engine);
	start = trace_now();
	s = ce_present(engine, &frame, submission, info);
	done = trace_now();

	if (be->accepted) {
		/* Planning succeeded; the core traced validate ok, then commit. */
		uint64_t plan_end = be->committed ? be->plan_end : done;

		trace_validate_ok(plan_end, OUT_ID, count, be->accepted_assignments,
				  be->accepted_count, trace_ticks(start, plan_end));
		if (s == CE_OK) {
			trace_commit_ok(done, OUT_ID, *submission, trace_ticks(plan_end, done));
		} else if (!trace_commit_err(done, OUT_ID, info, trace_ticks(plan_end, done))) {
			printk("SCENE_ERROR commit status %" PRId32 " has no trace code\n", s);
		}
	} else if (s != CE_OK) {
		if (!trace_validate_err(done, OUT_ID, count, info, be->rejected,
					trace_ticks(start, done))) {
			printk("SCENE_ERROR present status %" PRId32 " has no trace code\n", s);
		}
	}
	return s;
}

static ce_layer make_layer(uint32_t buffer, ce_rect src, ce_rect dst, int32_t z, uint16_t alpha,
			   uint8_t blend)
{
	return (ce_layer){
		.buffer = buffer,
		.source = src,
		.destination = dst,
		.z_order = z,
		.alpha = alpha,
		.blend = blend,
		.transform = CE_TRANSFORM_NORMAL,
	};
}

static size_t scene_layers(unsigned int slot, ce_layer *out)
{
	const ce_rect full = { 0, 0, OUT_W, OUT_H };
	const ce_rect top_src = { 0, 0, TOP_W, TOP_H };
	const ce_rect top_dst = { TOP_X, TOP_Y, TOP_W, TOP_H };

	out[0] = make_layer(slot_base[slot], full, full, 0, CE_ALPHA_OPAQUE, CE_BLEND_OPAQUE);
	if (LAYERS > 1) {
		out[1] = make_layer(slot_top[slot], top_src, top_dst, 1,
				    CONFIG_IMX95_M7_SCENE_TOP_ALPHA, CE_BLEND_PREMULTIPLIED);
	}
	return LAYERS;
}

/*
 * Polls until every buffer of the frame may be shown again, then marks the
 * ones the producer holds as drawn (the pixels never change). Returns false
 * on a timeout or an engine error.
 */
static bool make_ready(ce_engine *engine, struct counters *c, const ce_layer *layers,
		       size_t count)
{
	int64_t deadline = k_uptime_get() + CONFIG_IMX95_M7_SCENE_LATCH_TIMEOUT_MS;

	for (size_t i = 0; i < count; i++) {
		int32_t state;

		while ((state = ce_buffer_state(engine, layers[i].buffer)) == CE_BUFFER_SUBMITTED ||
		       state == CE_BUFFER_SCANNING) {
			if (!drain(engine, c) || c->timeouts > 0) {
				return false;
			}
			if (k_uptime_get() > deadline) {
				printk("SCENE_ERROR buffer %" PRIu32 " not released in %d ms\n",
				       layers[i].buffer, CONFIG_IMX95_M7_SCENE_LATCH_TIMEOUT_MS);
				return false;
			}
			k_yield();
		}
		if (state == CE_BUFFER_WRITABLE) {
			ce_status s = ce_finish_write(engine, layers[i].buffer);

			if (s != CE_OK) {
				printk("SCENE_ERROR ce_finish_write(%" PRIu32 ") status %" PRId32 "\n",
				       layers[i].buffer, s);
				return false;
			}
		}
	}
	return true;
}

static void print_scene(void)
{
	printk("SCENE layer=base buffer=%" PRIu32 ",%" PRIu32 " format=AR24 src=0,0,%ux%u"
	       " dst=0,0,%ux%u zorder=0 blend=opaque alpha=0x%04x\n",
	       slot_base[0], slot_base[1], OUT_W, OUT_H, OUT_W, OUT_H, CE_ALPHA_OPAQUE);
	if (LAYERS > 1) {
		printk("SCENE layer=top buffer=%" PRIu32 ",%" PRIu32 " format=AR24 src=0,0,%ux%u"
		       " dst=%u,%u,%ux%u zorder=1 blend=premultiplied alpha=0x%04x\n",
		       slot_top[0], slot_top[1], TOP_W, TOP_H, TOP_X, TOP_Y, TOP_W, TOP_H,
		       CONFIG_IMX95_M7_SCENE_TOP_ALPHA);
	}
}

static _Alignas(CE_ENGINE_ALIGN) unsigned char engine_storage[CE_ENGINE_SIZE];

int main(void)
{
	static const ce_backend_ops ops = {
		.capabilities = scene_capabilities,
		.check = scene_check,
		.commit = scene_commit,
		.poll_event = scene_poll_event,
	};
	static struct scene_backend be;
	static struct counters c;
	const char *verdict = "DONE";
	const char *reason = "";
	uint32_t frames = 0;

	printk("SCENE_APP imx95_m7_scene board=%s stage=%s frames=%d latch=%s trace=%s\n",
	       CONFIG_BOARD_TARGET, STAGE_TEXT, CONFIG_IMX95_M7_SCENE_FRAMES, LATCH_TEXT,
	       trace_buffered() ? "buffered" : "live");
	print_scene();
	printk("SCENE_NOTE prepend '# date: YYYY-MM-DD' (the run date) to the captured log"
	       " for scripts/trace-to-metrics.py\n");
	if (IS_ENABLED(CONFIG_IMX95_M7_SCENE_STUB_LATCH)) {
		printk("SCENE_NOTE latch=stub: no display is driven; latched lines are not"
		       " display events and this log is not a comparison result\n");
	}

	if (!CE_ABI_CHECK()) {
		printk("VERDICT m2scene-zephyr: FAIL ce_abi_check\n");
		return 0;
	}

#if defined(CONFIG_IMX95_M7_SCENE_ENABLE_DPU)
	be.dev = DEVICE_DT_GET(DISP_NODE);
	if (!device_is_ready(be.dev)) {
		printk("VERDICT m2scene-zephyr: FAIL display device not ready\n");
		return 0;
	}
	struct display_capabilities caps;

	display_get_capabilities(be.dev, &caps);
	if (caps.x_resolution != OUT_W || caps.y_resolution != OUT_H ||
	    caps.current_pixel_format != PIXEL_FORMAT_ARGB_8888) {
		printk("VERDICT m2scene-zephyr: FAIL display is %ux%u format 0x%x, not the"
		       " overlay's\n", caps.x_resolution, caps.y_resolution,
		       caps.current_pixel_format);
		return 0;
	}
	/* Static pixels, as the Linux side: every frame shows the same picture. */
	for (size_t i = 0; i < ARRAY_SIZE(fb[0]); i++) {
		fb[0][i] = COLOR_BASE;
		fb[1][i] = COLOR_BASE;
	}
#endif

	ce_engine *engine = ce_engine_init(engine_storage, sizeof(engine_storage), &ops, &be);

	if (engine == NULL) {
		printk("VERDICT m2scene-zephyr: FAIL ce_engine_init\n");
		return 0;
	}

	const ce_buffer_info base_info = { CE_MODIFIER_LINEAR, OUT_W, OUT_H, CE_FORMAT_ARGB8888 };
	const ce_buffer_info top_info = { CE_MODIFIER_LINEAR, TOP_W, TOP_H, CE_FORMAT_ARGB8888 };

	for (unsigned int s = 0; s < 2; s++) {
		if (ce_register_buffer(engine, slot_base[s], &base_info) != CE_OK ||
		    (LAYERS > 1 && ce_register_buffer(engine, slot_top[s], &top_info) != CE_OK)) {
			printk("VERDICT m2scene-zephyr: FAIL ce_register_buffer\n");
			return 0;
		}
	}

	uint64_t timeout = (uint64_t)trace_freq_hz() * CONFIG_IMX95_M7_SCENE_LATCH_TIMEOUT_MS / 1000;

	ce_set_latch_timeout(engine, timeout);
	trace_engine_time();

	char board[64];

	/* The Zephyr board target, not a measured SoC identity. */
	snprintk(board, sizeof(board), "zephyr %s latch=%s", CONFIG_BOARD_TARGET, LATCH_TEXT);
	/* build= names both sources: this repository and the Zephyr tree. */
	trace_hdr("hwlayer-manager " SCENE_REPO_REV " zephyr " STRINGIFY(BUILD_VERSION), board);

	for (frames = 0; frames < CONFIG_IMX95_M7_SCENE_FRAMES; frames++) {
		ce_layer layers[LAYERS];
		size_t count = scene_layers(frames & 1U, layers);
		ce_error_info info = { 0 };
		uint64_t submission = 0;
		ce_status s;

		if (!make_ready(engine, &c, layers, count)) {
			verdict = "FAIL";
			reason = c.timeouts > 0 ? "latch timeout" : "buffer not released";
			break;
		}
		s = present(engine, &be, layers, count, &submission, &info);
		if (s != CE_OK) {
			/*
			 * Stage B is expected to stop here on the stock driver:
			 * the second layer finds the only plane taken.
			 */
			verdict = IS_ENABLED(CONFIG_IMX95_M7_SCENE_STAGE_B) &&
						  s == CE_ERR_NO_PLANE
					  ? "BLOCKED"
					  : "FAIL";
			reason = s == CE_ERR_NO_PLANE ? "plan.no_plane (see the validate line)"
						      : "ce_present failed (see the trace)";
			break;
		}
		c.committed++;
		if (!drain(engine, &c)) {
			verdict = "FAIL";
			reason = "ce_poll_event";
			break;
		}
	}

	/* Wait for the last submission's latch, as the frame loop would. */
	int64_t deadline = k_uptime_get() + CONFIG_IMX95_M7_SCENE_LATCH_TIMEOUT_MS;

	while (strcmp(verdict, "DONE") == 0 && c.latched + c.dropped < c.committed) {
		if (!drain(engine, &c)) {
			verdict = "FAIL";
			reason = "ce_poll_event";
		} else if (c.timeouts > 0 || k_uptime_get() > deadline) {
			verdict = "FAIL";
			reason = "latch timeout";
		}
		k_yield();
	}

	trace_flush();
	if (strcmp(verdict, "DONE") == 0 && trace_lost() > 0) {
		verdict = "FAIL";
		reason = "trace lines lost (raise CONFIG_IMX95_M7_SCENE_TRACE_BUFFER_KB)";
	}
	printk("RESULT frames=%u committed=%u latched=%u dropped=%u timeouts=%u trace_lost=%u\n",
	       frames, c.committed, c.latched, c.dropped, c.timeouts, trace_lost());
	printk("VERDICT m2scene-zephyr: %s%s%s\n", verdict, reason[0] ? " " : "", reason);
	return 0;
}
