/*
 * The composition engine's planner on the i.MX95 Cortex-M7 under Zephyr.
 *
 * Links libcomposition_engine.a (composition-ffi-staticlib, built for
 * thumbv7em-none-eabihf) through crates/composition-ffi/include/
 * composition_engine.h and presents a few fixed scenes to a backend that
 * describes what the stock Zephyr DPU driver (display_socionext_dpu.c)
 * offers: one framebuffer plane, no scaling, no plane alpha, no rotation
 * (docs/platform-capabilities.md, "i.MX95, Zephyr"; read from source, not
 * measured). The backend never touches the display: check accepts every
 * assignment the planner proposes, commit prints the plan and reports the
 * latch at once. Only the console is used.
 *
 * Each scene prints its plan or the reason it could not be planned, and the
 * program ends with "RESULT: PASS" if every scene behaved as expected.
 */
#include <zephyr/kernel.h>
#include <zephyr/sys/printk.h>

#include <inttypes.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "composition_engine.h"

/* Not in the header: DRM's RGB888 (packed 24 bit). */
#define APP_FORMAT_RGB888 CE_FOURCC('R', 'G', '2', '4')

/*
 * The output size is the Toradex 10.1" LVDS panel of the Verdin iMX95
 * setup (1280x800). Nothing here depends on it except the scenes.
 */
enum { OUT_ID = 0, OUT_W = 1280, OUT_H = 800 };
enum { PLANE_FB = 0 };
enum { BUF_UI = 1, BUF_VIDEO = 2, BUF_SMALL = 3 };

struct fb_backend {
	bool latched;
	uint64_t latched_submission;
	unsigned int checks;
	unsigned int commits;
	size_t last_count;
};

static void fb_capabilities(void *ctx, ce_capabilities *out)
{
	(void)ctx;
	out->output_count = 1;
	out->outputs[0] = (ce_output_caps){
		.id = OUT_ID, .width = OUT_W, .height = OUT_H, .refresh_mhz = 0,
	};
	out->plane_count = 1;
	out->planes[0] = (ce_plane_caps){
		.id = PLANE_FB,
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
		/* The framebuffer is the whole panel (inferred from the driver). */
		.full_output_only = 1,
	};
	out->planes[0].formats[0] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_RGB565 };
	out->planes[0].formats[1] = (ce_format){ CE_MODIFIER_LINEAR, APP_FORMAT_RGB888 };
	out->planes[0].formats[2] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_ARGB8888 };
	/* No shadow-register replacement is claimed. */
	out->replace_pending = 0;
}

static ce_status fb_check(void *ctx, const ce_frame *frame, const ce_assignment *assignments,
			  size_t count)
{
	struct fb_backend *be = ctx;

	be->checks++;
	for (size_t i = 0; i < count; i++) {
		if (assignments[i].plane_index != 0 || assignments[i].layer >= frame->layer_count) {
			return CE_ERR_BACKEND_REJECTED;
		}
	}
	return CE_OK;
}

static ce_status fb_commit(void *ctx, uint64_t submission, const ce_frame *frame,
			   const ce_assignment *assignments, size_t count)
{
	struct fb_backend *be = ctx;

	be->commits++;
	be->last_count = count;
	printk("  plan: submission %u, %u assignment(s) for %u layer(s)\n",
	       (unsigned int)submission, (unsigned int)count, (unsigned int)frame->layer_count);
	for (size_t i = 0; i < count; i++) {
		const ce_assignment *a = &assignments[i];

		printk("    layer %" PRIu32 " (buffer %" PRIu32 ") -> plane %" PRIu32
		       " (index %" PRIu32 ") zpos %" PRId32 " src %" PRId32 ",%" PRId32
		       " %" PRIu32 "x%" PRIu32 " dst %" PRId32 ",%" PRId32 " %" PRIu32
		       "x%" PRIu32 "\n",
		       a->layer, frame->layers[a->layer].buffer, a->plane, a->plane_index,
		       a->zpos, a->source.x, a->source.y, a->source.width, a->source.height,
		       a->destination.x, a->destination.y, a->destination.width,
		       a->destination.height);
	}
	/* Nothing is programmed, so the "latch" is immediate. */
	be->latched = true;
	be->latched_submission = submission;
	return CE_OK;
}

static bool fb_poll_event(void *ctx, ce_backend_event *out)
{
	struct fb_backend *be = ctx;

	if (!be->latched) {
		return false;
	}
	be->latched = false;
	out->kind = CE_BACKEND_LATCHED;
	out->output = OUT_ID;
	out->submission = be->latched_submission;
	return true;
}

static const char *status_name(ce_status s)
{
	switch (s) {
	case CE_OK: return "CE_OK";
	case CE_ERR_INVALID_ARGUMENT: return "CE_ERR_INVALID_ARGUMENT";
	case CE_ERR_TOO_MANY_LAYERS: return "CE_ERR_TOO_MANY_LAYERS";
	case CE_ERR_UNKNOWN_BUFFER: return "CE_ERR_UNKNOWN_BUFFER";
	case CE_ERR_INVALID_SOURCE: return "CE_ERR_INVALID_SOURCE";
	case CE_ERR_INVALID_DESTINATION: return "CE_ERR_INVALID_DESTINATION";
	case CE_ERR_NO_PLANE: return "CE_ERR_NO_PLANE";
	case CE_ERR_BACKEND_REJECTED: return "CE_ERR_BACKEND_REJECTED";
	case CE_ERR_SEARCH_LIMIT: return "CE_ERR_SEARCH_LIMIT";
	case CE_ERR_CANNOT_CLIP: return "CE_ERR_CANNOT_CLIP";
	case CE_ERR_BUFFER_NOT_READY: return "CE_ERR_BUFFER_NOT_READY";
	case CE_ERR_BUSY: return "CE_ERR_BUSY";
	default: return "other";
	}
}

static void print_reasons(uint32_t r)
{
	static const struct {
		uint32_t bit;
		const char *name;
	} names[] = {
		{ CE_REJECT_OUTPUT, "output" },
		{ CE_REJECT_FORMAT, "format" },
		{ CE_REJECT_SCALING, "scaling" },
		{ CE_REJECT_SOURCE_SIZE, "source-size" },
		{ CE_REJECT_ALPHA, "alpha" },
		{ CE_REJECT_BLEND, "blend" },
		{ CE_REJECT_TRANSFORM, "transform" },
		{ CE_REJECT_FULL_OUTPUT, "full-output" },
		{ CE_REJECT_Z_ORDER, "z-order" },
		{ CE_REJECT_RESOURCE, "resource" },
		{ CE_REJECT_IN_USE, "in-use" },
	};

	printk("    reasons 0x%" PRIx32 ":", r);
	for (size_t i = 0; i < ARRAY_SIZE(names); i++) {
		if (r & names[i].bit) {
			printk(" %s", names[i].name);
		}
	}
	printk("\n");
}

static ce_layer layer(uint32_t buffer, ce_rect src, ce_rect dst, int32_t z, uint16_t alpha,
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

/* Drains the engine's events; returns the number of PRESENTED events. */
static int drain(ce_engine *engine)
{
	static const char *const kinds[] = {
		"presented", "dropped", "released", "device-lost", "capabilities-changed",
		"latch-timeout",
	};
	ce_event ev;
	int presented = 0;
	int32_t r;

	while ((r = ce_poll_event(engine, &ev)) == 1) {
		printk("  event: %s submission %u buffer %" PRIu32 "\n",
		       ev.kind < ARRAY_SIZE(kinds) ? kinds[ev.kind] : "?",
		       (unsigned int)ev.submission, ev.buffer);
		if (ev.kind == CE_EVENT_PRESENTED) {
			presented++;
		}
	}
	if (r < 0) {
		printk("  ce_poll_event failed: %" PRId32 "\n", r);
		return -1;
	}
	return presented;
}

/*
 * Presents one scene and compares the outcome with what is expected:
 * expect_status, and on success the number of assignments.
 */
static bool scene(ce_engine *engine, struct fb_backend *be, const char *name,
		  const ce_layer *layers, size_t count, ce_status expect_status,
		  size_t expect_assignments)
{
	const ce_frame frame = { .layers = layers, .layer_count = count, .output = OUT_ID };
	ce_error_info info = { 0 };
	uint64_t submission = 0;
	unsigned int commits_before = be->commits;
	ce_status s;
	bool ok;

	printk("scene: %s (%u layer(s))\n", name, (unsigned int)count);
	/* Every buffer in the scene is drawn and ready. */
	for (size_t i = 0; i < count; i++) {
		if (ce_buffer_state(engine, layers[i].buffer) == CE_BUFFER_WRITABLE) {
			ce_finish_write(engine, layers[i].buffer);
		}
	}
	s = ce_present(engine, &frame, &submission, &info);
	if (s == CE_OK) {
		int presented = drain(engine);

		ok = expect_status == CE_OK && be->last_count == expect_assignments &&
		     presented == 1;
	} else {
		printk("  not planned: %s (%" PRId32 "), layer %" PRIu32 "\n", status_name(s), s,
		       info.layer);
		if (s == CE_ERR_NO_PLANE) {
			print_reasons(info.reasons);
		}
		ok = s == expect_status && be->commits == commits_before;
	}
	printk("  -> %s\n", ok ? "as expected" : "UNEXPECTED");
	return ok;
}

static _Alignas(CE_ENGINE_ALIGN) unsigned char engine_storage[CE_ENGINE_SIZE];

int main(void)
{
	static const ce_backend_ops ops = {
		.capabilities = fb_capabilities,
		.check = fb_check,
		.commit = fb_commit,
		.poll_event = fb_poll_event,
	};
	static struct fb_backend be;
	const ce_rect full = { 0, 0, OUT_W, OUT_H };
	const ce_rect video_src = { 0, 0, 640, 360 };
	const ce_rect video_dst = { 320, 220, 640, 360 };
	const ce_rect scaled_dst = { 0, 0, 1280, 720 };
	bool pass = true;

	printk("hwlayer-manager composition engine on %s\n", CONFIG_BOARD_TARGET);
	printk("capabilities: 1 output %ux%u, 1 primary framebuffer plane"
	       " (RGB565/RGB888/ARGB8888, linear, opaque, no scaling, full output only)\n",
	       OUT_W, OUT_H);

	if (!CE_ABI_CHECK()) {
		printk("ce_abi_check failed: header and library disagree\nRESULT: FAIL\n");
		return 0;
	}
	ce_engine *engine = ce_engine_init(engine_storage, sizeof(engine_storage), &ops, &be);
	if (engine == NULL) {
		printk("ce_engine_init failed\nRESULT: FAIL\n");
		return 0;
	}

	static const struct {
		uint32_t id;
		ce_buffer_info info;
	} buffers[] = {
		{ BUF_UI, { CE_MODIFIER_LINEAR, OUT_W, OUT_H, CE_FORMAT_XRGB8888 } },
		{ BUF_VIDEO, { CE_MODIFIER_LINEAR, 640, 360, CE_FORMAT_RGB565 } },
		{ BUF_SMALL, { CE_MODIFIER_LINEAR, 640, 360, CE_FORMAT_ARGB8888 } },
	};
	for (size_t i = 0; i < ARRAY_SIZE(buffers); i++) {
		ce_status s = ce_register_buffer(engine, buffers[i].id, &buffers[i].info);

		if (s != CE_OK) {
			printk("ce_register_buffer(%" PRIu32 ") failed: %" PRId32 "\nRESULT: FAIL\n",
			       buffers[i].id, s);
			return 0;
		}
	}

	/*
	 * 1. A full-screen UI layer in XRGB8888: the plane lists no XRGB8888,
	 *    so the planner refuses it with a format reason.
	 */
	const ce_layer s1[] = {
		layer(BUF_UI, full, full, 0, CE_ALPHA_OPAQUE, CE_BLEND_OPAQUE),
	};
	pass &= scene(engine, &be, "XRGB8888 primary", s1, ARRAY_SIZE(s1), CE_ERR_NO_PLANE, 0);

	/* 2. A full-screen ARGB8888 primary, opaque: one plane, one assignment. */
	static const ce_buffer_info fb_info = { CE_MODIFIER_LINEAR, OUT_W, OUT_H,
						CE_FORMAT_ARGB8888 };
	enum { BUF_FB = 4 };
	if (ce_register_buffer(engine, BUF_FB, &fb_info) != CE_OK) {
		printk("ce_register_buffer(%d) failed\nRESULT: FAIL\n", BUF_FB);
		return 0;
	}
	const ce_layer s2[] = {
		layer(BUF_FB, full, full, 0, CE_ALPHA_OPAQUE, CE_BLEND_OPAQUE),
	};
	pass &= scene(engine, &be, "primary only", s2, ARRAY_SIZE(s2), CE_OK, 1);

	/*
	 * 3. The primary plus one overlay (a 640x360 RGB565 video window): the
	 *    stock driver has one plane, so the frame cannot be mapped onto
	 *    the hardware. The caller would have to compose in software.
	 */
	const ce_layer s3[] = {
		layer(BUF_FB, full, full, 0, CE_ALPHA_OPAQUE, CE_BLEND_OPAQUE),
		layer(BUF_VIDEO, video_src, video_dst, 1, CE_ALPHA_OPAQUE, CE_BLEND_OPAQUE),
	};
	pass &= scene(engine, &be, "primary + overlay", s3, ARRAY_SIZE(s3), CE_ERR_NO_PLANE, 0);

	/* 4. A single window scaled to 1280x720: no scaling, not full output. */
	const ce_layer s4[] = {
		layer(BUF_SMALL, video_src, scaled_dst, 0, CE_ALPHA_OPAQUE, CE_BLEND_OPAQUE),
	};
	pass &= scene(engine, &be, "scaled window", s4, ARRAY_SIZE(s4), CE_ERR_NO_PLANE, 0);

	/* 5. The primary again, from the same buffer: still plannable. */
	pass &= scene(engine, &be, "primary again", s2, ARRAY_SIZE(s2), CE_OK, 1);

	printk("checks %u, commits %u\n", be.checks, be.commits);
	printk("RESULT: %s\n", pass ? "PASS" : "FAIL");
	return 0;
}
