/*
 * A C program driving the engine through its C interface, with a backend
 * written in C. The backend models a display controller with two planes and
 * shadow registers, in the arrangement of the AM62P safety-layer setup: an
 * RGB565 application plane below an ARGB8888 safety plane.
 *
 * Exit status 0 means every check passed.
 */
#include "composition_engine.h"

#include <stdio.h>
#include <string.h>

#define CHECK(cond) \
	do { \
		if (!(cond)) { \
			fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__, __LINE__, #cond); \
			return 1; \
		} \
	} while (0)

enum { OUT = 7, PLANE_APP = 31, PLANE_SAFETY = 41 };
enum { BUF_APP_A = 1, BUF_APP_B = 2, BUF_SAFETY = 3 };

struct fake_dss {
	/* Shadow registers: written by commit, latched at vsync. */
	bool pending;
	uint64_t pending_submission;
	uint32_t shadow[2]; /* buffer per plane, 0 = disabled */
	uint32_t active[2];
	ce_rect shadow_source[2];
	ce_rect shadow_destination[2];
	bool latched_event;
	uint64_t latched_submission;
	int fail_commits;
	int checks;
	int commits;
};

static void fake_capabilities(void *ctx, ce_capabilities *out)
{
	(void)ctx;
	out->output_count = 1;
	out->outputs[0] = (ce_output_caps){ .id = OUT, .width = 1280, .height = 720,
					    .refresh_mhz = 60000 };
	out->plane_count = 2;
	out->planes[0] = (ce_plane_caps){
		.id = PLANE_APP, .kind = CE_PLANE_PRIMARY, .outputs = 1,
		.format_count = 1, .zpos_min = 0,
		.blend_modes = 1u << CE_BLEND_OPAQUE,
	};
	out->planes[0].formats[0] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_RGB565 };
	out->planes[1] = (ce_plane_caps){
		.id = PLANE_SAFETY, .kind = CE_PLANE_OVERLAY, .outputs = 1,
		.format_count = 1, .zpos_min = 1,
		.blend_modes = (1u << CE_BLEND_PREMULTIPLIED) | (1u << CE_BLEND_COVERAGE),
	};
	out->planes[1].formats[0] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_ARGB8888 };
	/* A commit before the latch rewrites the shadow registers. */
	out->replace_pending = 1;
}

static ce_status fake_check(void *ctx, const ce_frame *frame, const ce_assignment *assignments,
			    size_t count)
{
	struct fake_dss *dss = ctx;
	dss->checks++;
	for (size_t i = 0; i < count; i++) {
		if (assignments[i].plane_index > 1 || assignments[i].layer >= frame->layer_count) {
			return CE_ERR_BACKEND_REJECTED;
		}
	}
	return CE_OK;
}

static ce_status fake_commit(void *ctx, uint64_t submission, const ce_frame *frame,
			     const ce_assignment *assignments, size_t count)
{
	struct fake_dss *dss = ctx;
	if (dss->fail_commits > 0) {
		dss->fail_commits--;
		return -5; /* an errno-like platform code */
	}
	dss->shadow[0] = dss->shadow[1] = 0;
	for (size_t i = 0; i < count; i++) {
		dss->shadow[assignments[i].plane_index] = frame->layers[assignments[i].layer].buffer;
		dss->shadow_source[assignments[i].plane_index] = assignments[i].source;
		dss->shadow_destination[assignments[i].plane_index] = assignments[i].destination;
	}
	dss->pending = true;
	dss->pending_submission = submission;
	dss->commits++;
	return CE_OK;
}

static bool fake_poll_event(void *ctx, ce_backend_event *out)
{
	struct fake_dss *dss = ctx;
	if (!dss->latched_event) {
		return false;
	}
	dss->latched_event = false;
	out->kind = CE_BACKEND_LATCHED;
	out->output = OUT;
	out->submission = dss->latched_submission;
	return true;
}

static void fake_vsync(struct fake_dss *dss)
{
	if (dss->pending) {
		dss->pending = false;
		memcpy(dss->active, dss->shadow, sizeof(dss->active));
		dss->latched_event = true;
		dss->latched_submission = dss->pending_submission;
	}
}

static ce_layer full_layer(uint32_t buffer, int32_t z, uint8_t blend)
{
	return (ce_layer){
		.buffer = buffer,
		.source = { 0, 0, 1280, 720 },
		.destination = { 0, 0, 1280, 720 },
		.z_order = z,
		.alpha = CE_ALPHA_OPAQUE,
		.blend = blend,
		.transform = CE_TRANSFORM_NORMAL,
	};
}

static _Alignas(CE_ENGINE_ALIGN) unsigned char storage[CE_ENGINE_SIZE];

int main(void)
{
	static const ce_backend_ops ops = {
		.capabilities = fake_capabilities,
		.check = fake_check,
		.commit = fake_commit,
		.poll_event = fake_poll_event,
	};
	struct fake_dss dss = { 0 };
	ce_event event;
	ce_error_info info;
	uint64_t first = 0, second = 0, third = 0;

	CHECK(CE_ABI_CHECK());
	CHECK(ce_engine_init(storage, 16, &ops, &dss) == NULL);
	CHECK(ce_engine_init(storage + 1, sizeof(storage) - 1, &ops, &dss) == NULL);
	ce_engine *engine = ce_engine_init(storage, sizeof(storage), &ops, &dss);
	CHECK(engine != NULL);

	const ce_buffer_info app_info = { CE_MODIFIER_LINEAR, 1280, 720, CE_FORMAT_RGB565 };
	const ce_buffer_info safety_info = { CE_MODIFIER_LINEAR, 1280, 720, CE_FORMAT_ARGB8888 };
	CHECK(ce_register_buffer(engine, BUF_APP_A, &app_info) == CE_OK);
	CHECK(ce_register_buffer(engine, BUF_APP_B, &app_info) == CE_OK);
	CHECK(ce_register_buffer(engine, BUF_SAFETY, &safety_info) == CE_OK);
	CHECK(ce_register_buffer(engine, BUF_SAFETY, &safety_info) == CE_ERR_DUPLICATE_BUFFER);
	CHECK(ce_buffer_state(engine, BUF_APP_A) == CE_BUFFER_WRITABLE);
	CHECK(ce_buffer_state(engine, 99) == CE_BUFFER_UNKNOWN);

	/* Safety layer listed first: stacking comes from z_order. */
	ce_layer layers[2] = {
		full_layer(BUF_SAFETY, 10, CE_BLEND_PREMULTIPLIED),
		full_layer(BUF_APP_A, 0, CE_BLEND_OPAQUE),
	};
	ce_frame frame = { .layers = layers, .layer_count = 2, .output = OUT };

	/* A buffer still being drawn is never shown. */
	CHECK(ce_present(engine, &frame, &first, &info) == CE_ERR_BUFFER_NOT_READY);
	CHECK(dss.commits == 0);
	CHECK(ce_finish_write(engine, BUF_APP_A) == CE_OK);
	CHECK(ce_finish_write(engine, BUF_APP_B) == CE_OK);
	CHECK(ce_finish_write(engine, BUF_SAFETY) == CE_OK);

	/* A failing commit changes nothing. */
	dss.fail_commits = 1;
	CHECK(ce_present(engine, &frame, &first, &info) == CE_ERR_BACKEND_PLATFORM);
	CHECK(info.platform == -5);
	CHECK(ce_buffer_state(engine, BUF_APP_A) == CE_BUFFER_READY);

	CHECK(ce_present(engine, &frame, &first, &info) == CE_OK);
	CHECK(info.status == CE_OK);
	/* Accepted is not shown. */
	CHECK(ce_buffer_state(engine, BUF_APP_A) == CE_BUFFER_SUBMITTED);
	CHECK(ce_poll_event(engine, &event) == 0);
	CHECK(dss.active[0] == 0);

	fake_vsync(&dss);
	CHECK(ce_poll_event(engine, &event) == 1);
	CHECK(event.kind == CE_EVENT_PRESENTED && event.submission == first && event.output == OUT);
	CHECK(ce_poll_event(engine, &event) == 0);
	CHECK(dss.active[0] == BUF_APP_A && dss.active[1] == BUF_SAFETY);
	CHECK(ce_buffer_state(engine, BUF_SAFETY) == CE_BUFFER_SCANNING);

	/* Rotate the application buffer; the old one stays ours until the latch. */
	layers[1] = full_layer(BUF_APP_B, 0, CE_BLEND_OPAQUE);
	CHECK(ce_present(engine, &frame, &second, NULL) == CE_OK);
	CHECK(ce_begin_write(engine, BUF_APP_A) == CE_ERR_BUFFER_IN_USE);

	/* Replace the pending frame before the latch: it is reported dropped. */
	ce_frame safety_only = { .layers = layers, .layer_count = 1, .output = OUT };
	CHECK(ce_present(engine, &safety_only, &third, NULL) == CE_OK);
	CHECK(ce_poll_event(engine, &event) == 1);
	CHECK(event.kind == CE_EVENT_DROPPED && event.submission == second);
	CHECK(ce_poll_event(engine, &event) == 1);
	CHECK(event.kind == CE_EVENT_RELEASED && event.buffer == BUF_APP_B);
	CHECK(ce_poll_event(engine, &event) == 0);

	fake_vsync(&dss);
	CHECK(ce_poll_event(engine, &event) == 1);
	CHECK(event.kind == CE_EVENT_PRESENTED && event.submission == third);
	CHECK(ce_poll_event(engine, &event) == 1);
	CHECK(event.kind == CE_EVENT_RELEASED && event.buffer == BUF_APP_A);
	CHECK(ce_poll_event(engine, &event) == 0);
	/* The application plane is off, the safety plane still on. */
	CHECK(dss.active[0] == 0 && dss.active[1] == BUF_SAFETY);
	CHECK(ce_begin_write(engine, BUF_APP_A) == CE_OK);
	CHECK(ce_finish_write(engine, BUF_APP_A) == CE_OK);

	/*
	 * No vsync: the overdue latch is reported once and the buffer stays
	 * protected. The layer hangs 80 pixels over the left edge, so the
	 * backend is handed the clipped rectangles.
	 */
	layers[0] = full_layer(BUF_APP_A, 0, CE_BLEND_OPAQUE);
	layers[0].destination.x = -80;
	layers[1] = full_layer(BUF_SAFETY, 10, CE_BLEND_PREMULTIPLIED);
	uint64_t fourth = 0;
	CHECK(ce_set_latch_timeout(engine, 20) == CE_OK);
	CHECK(ce_advance_time(engine, 1000) == CE_OK);
	CHECK(ce_present(engine, &frame, &fourth, &info) == CE_OK);
	CHECK(dss.shadow_source[0].x == 80 && dss.shadow_source[0].width == 1200);
	CHECK(dss.shadow_destination[0].x == 0 && dss.shadow_destination[0].width == 1200);
	CHECK(dss.shadow_destination[1].x == 0 && dss.shadow_destination[1].width == 1280);
	CHECK(ce_advance_time(engine, 1019) == CE_OK);
	CHECK(ce_poll_event(engine, &event) == 0);
	CHECK(ce_advance_time(engine, 1020) == CE_OK);
	CHECK(ce_advance_time(engine, 1100) == CE_OK);
	CHECK(ce_poll_event(engine, &event) == 1);
	CHECK(event.kind == CE_EVENT_LATCH_TIMEOUT && event.submission == fourth);
	CHECK(ce_poll_event(engine, &event) == 0);
	CHECK(ce_buffer_state(engine, BUF_APP_A) == CE_BUFFER_SUBMITTED);
	fake_vsync(&dss);
	CHECK(ce_poll_event(engine, &event) == 1);
	CHECK(event.kind == CE_EVENT_PRESENTED && event.submission == fourth);
	CHECK(ce_poll_event(engine, &event) == 0);

	/* The application buffer above the safety layer has no plane. */
	layers[0] = full_layer(BUF_SAFETY, 0, CE_BLEND_PREMULTIPLIED);
	layers[1] = full_layer(BUF_APP_A, 10, CE_BLEND_OPAQUE);
	int commits = dss.commits;
	CHECK(ce_present(engine, &frame, NULL, &info) == CE_ERR_NO_PLANE);
	CHECK(info.layer == 1);
	CHECK(info.reasons == (CE_REJECT_FORMAT | CE_REJECT_Z_ORDER));
	CHECK(dss.commits == commits);

	/* Malformed input is refused, not interpreted. */
	layers[1].blend = 9;
	CHECK(ce_present(engine, &frame, NULL, &info) == CE_ERR_INVALID_ARGUMENT);
	CHECK(info.layer == 1);
	frame.layer_count = CE_MAX_LAYERS + 1;
	CHECK(ce_present(engine, &frame, NULL, NULL) == CE_ERR_TOO_MANY_LAYERS);
	CHECK(ce_present(engine, NULL, NULL, NULL) == CE_ERR_INVALID_ARGUMENT);
	CHECK(ce_poll_event(NULL, &event) == CE_ERR_INVALID_ARGUMENT);

	/* A latch nobody asked for is reported as an error. */
	dss.latched_event = true;
	dss.latched_submission = 1234;
	CHECK(ce_poll_event(engine, &event) == CE_ERR_UNEXPECTED_LATCH);

	CHECK(ce_unregister_buffer(engine, BUF_SAFETY) == CE_ERR_BUFFER_IN_USE);
	CHECK(ce_unregister_buffer(engine, BUF_APP_B) == CE_OK);

	puts("ok");
	return 0;
}
