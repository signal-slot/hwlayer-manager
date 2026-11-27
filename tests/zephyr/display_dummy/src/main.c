/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * The composition engine driving Zephyr's dummy display, under QEMU.
 *
 * This is the backend contract of docs/imx95-m7-backend-contract.md written
 * out against the display API: capabilities() from display_get_capabilities(),
 * check() and commit() validating the assignments and then calling
 * display_write(), poll_event() reporting a latch.
 *
 * The dummy display has no VSYNC and no latch of its own: display_write()
 * returns at once and shows nothing. The latch reported here is therefore
 * synthesized at the return of the last display_write() of a commit, which is
 * what section 3.3 of the contract says a stock blocking driver permits. It
 * says nothing about timing and nothing about any hardware.
 *
 * Output: one PASS or FAIL line per check, then "RESULT: PASS" or
 * "RESULT: FAIL". The guest then idles; the caller stops QEMU (see README.md).
 */
#include <string.h>

#include <zephyr/device.h>
#include <zephyr/drivers/display.h>
#include <zephyr/kernel.h>

#include "composition_engine.h"

#define SCREEN_W 64
#define SCREEN_H 32
#define FG_W 32
#define FG_H 16
#define BYTES_PER_PIXEL 4

enum { OUTPUT_ID = 0, PLANE_PRIMARY = 10, PLANE_OVERLAY = 11 };
enum { BUF_BG = 1, BUF_FG = 2, BUF_RGB565 = 3 };
enum { MAX_WRITES = 8 };

#define BG_COLOR 0xFF102030u
#define FG_COLOR 0xFFA0B0C0u

static int failures;
static int checks;

#define CHECK(cond, name)                                                                         \
	do {                                                                                      \
		checks++;                                                                         \
		if (cond) {                                                                       \
			printk("PASS: %s\n", name);                                               \
		} else {                                                                          \
			failures++;                                                               \
			printk("FAIL: %s (line %d)\n", name, __LINE__);                           \
		}                                                                                 \
	} while (0)

/* Pixel storage. The engine is told about these; the backend finds them by id. */
static uint32_t bg_pixels[SCREEN_W * SCREEN_H];
static uint32_t fg_pixels[FG_W * FG_H];
static uint16_t rgb565_pixels[FG_W * FG_H];

struct buf_record {
	uint32_t id;
	const void *pixels;
	uint32_t width;
	uint32_t height;
	uint32_t format;
};

static const struct buf_record buffers[] = {
	{BUF_BG, bg_pixels, SCREEN_W, SCREEN_H, CE_FORMAT_ARGB8888},
	{BUF_FG, fg_pixels, FG_W, FG_H, CE_FORMAT_ARGB8888},
	{BUF_RGB565, rgb565_pixels, FG_W, FG_H, CE_FORMAT_RGB565},
};

struct write_log {
	uint16_t x, y;
	uint16_t width, height, pitch;
	uint32_t first_pixel;
};

struct backend {
	const struct device *display;
	struct display_capabilities dcaps;
	/* One latch at most is outstanding: commit() is synchronous. */
	bool latch_ready;
	uint64_t latch_submission;
	struct write_log writes[MAX_WRITES];
	int write_count;
	int commit_count;
};

static const struct buf_record *lookup(uint32_t id)
{
	for (size_t i = 0; i < ARRAY_SIZE(buffers); i++) {
		if (buffers[i].id == id) {
			return &buffers[i];
		}
	}
	return NULL;
}

static void be_capabilities(void *ctx, ce_capabilities *out)
{
	struct backend *be = ctx;

	out->output_count = 1;
	out->outputs[0] = (ce_output_caps){
		.id = OUTPUT_ID,
		.width = be->dcaps.x_resolution,
		.height = be->dcaps.y_resolution,
		.refresh_mhz = 0, /* the display API has no refresh rate */
	};

	/* Plane 0: the whole screen, as the stock DPU driver's one plane would be. */
	out->plane_count = 2;
	out->planes[0] = (ce_plane_caps){
		.id = PLANE_PRIMARY,
		.kind = CE_PLANE_PRIMARY,
		.outputs = 1,
		.zpos_min = 0,
		.zpos_max = 0,
		.blend_modes = 1u << CE_BLEND_OPAQUE,
		.transforms = 1u << CE_TRANSFORM_NORMAL,
		.full_output_only = 1,
		.format_count = 0,
	};
	/* Plane 1: a rectangle written with display_write(x, y, ...). */
	out->planes[1] = (ce_plane_caps){
		.id = PLANE_OVERLAY,
		.kind = CE_PLANE_OVERLAY,
		.outputs = 1,
		.zpos_min = 1,
		.zpos_max = 1,
		.blend_modes = 1u << CE_BLEND_OPAQUE,
		.transforms = 1u << CE_TRANSFORM_NORMAL,
	};
	/* Formats: only those the driver reports and the engine has a name for. */
	if (be->dcaps.supported_pixel_formats & PIXEL_FORMAT_ARGB_8888) {
		for (int p = 0; p < 2; p++) {
			out->planes[p].formats[0] =
				(ce_format){CE_MODIFIER_LINEAR, CE_FORMAT_ARGB8888};
			out->planes[p].format_count = 1;
		}
	}
	out->replace_pending = 0; /* commit() is synchronous: nothing stays pending */
}

/* Common validation of check() and commit(): no display change. */
static ce_status be_validate(struct backend *be, const ce_frame *frame,
			     const ce_assignment *as, size_t count)
{
	for (size_t i = 0; i < count; i++) {
		const struct buf_record *rec = lookup(frame->layers[as[i].layer].buffer);

		if (rec == NULL) {
			return CE_ERR_BACKEND_UNKNOWN_BUFFER;
		}
		if (rec->format != CE_FORMAT_ARGB8888 ||
		    !(be->dcaps.supported_pixel_formats & PIXEL_FORMAT_ARGB_8888)) {
			return CE_ERR_BACKEND_REJECTED;
		}
		if (as[i].source.x < 0 || as[i].source.y < 0 ||
		    (uint32_t)as[i].source.x + as[i].source.width > rec->width ||
		    (uint32_t)as[i].source.y + as[i].source.height > rec->height ||
		    as[i].destination.x < 0 || as[i].destination.y < 0 ||
		    (uint32_t)as[i].destination.x + as[i].destination.width >
			    be->dcaps.x_resolution ||
		    (uint32_t)as[i].destination.y + as[i].destination.height >
			    be->dcaps.y_resolution ||
		    as[i].source.width != as[i].destination.width ||
		    as[i].source.height != as[i].destination.height) {
			return CE_ERR_BACKEND_REJECTED; /* no scaling, no off-screen */
		}
		if (as[i].plane == PLANE_PRIMARY &&
		    (as[i].destination.width != be->dcaps.x_resolution ||
		     as[i].destination.height != be->dcaps.y_resolution)) {
			return CE_ERR_BACKEND_REJECTED; /* full output only */
		}
	}
	return CE_OK;
}

static ce_status be_check(void *ctx, const ce_frame *frame, const ce_assignment *as, size_t count)
{
	return be_validate(ctx, frame, as, count);
}

static ce_status be_commit(void *ctx, uint64_t submission, const ce_frame *frame,
			   const ce_assignment *as, size_t count)
{
	struct backend *be = ctx;
	ce_status st = be_validate(be, frame, as, count);

	if (st != CE_OK) {
		return st; /* nothing written yet */
	}
	if (be->latch_ready) {
		return CE_ERR_BACKEND_BUSY; /* an unreported latch: poll first */
	}
	be->write_count = 0;

	/* Assignments come in layer order; write the lower plane first. */
	for (uint32_t plane_index = 0; plane_index < 2; plane_index++) {
		for (size_t i = 0; i < count; i++) {
			if (as[i].plane_index != plane_index) {
				continue;
			}
			const struct buf_record *rec = lookup(frame->layers[as[i].layer].buffer);
			const uint32_t *base = rec->pixels;
			const uint32_t *start = base + as[i].source.y * rec->width + as[i].source.x;
			struct display_buffer_descriptor desc = {
				.buf_size = as[i].source.height * rec->width * BYTES_PER_PIXEL,
				.width = as[i].source.width,
				.height = as[i].source.height,
				.pitch = rec->width, /* in pixels */
			};
			int rc = display_write(be->display, as[i].destination.x,
					       as[i].destination.y, &desc, start);

			if (rc != 0) {
				return CE_ERR_BACKEND_PLATFORM;
			}
			if (be->write_count < MAX_WRITES) {
				be->writes[be->write_count++] = (struct write_log){
					.x = as[i].destination.x,
					.y = as[i].destination.y,
					.width = desc.width,
					.height = desc.height,
					.pitch = desc.pitch,
					.first_pixel = *start,
				};
			}
		}
	}
	be->commit_count++;
	/* Synthesized latch: the dummy display has no VSYNC (see the file header). */
	be->latch_ready = true;
	be->latch_submission = submission;
	return CE_OK;
}

static bool be_poll_event(void *ctx, ce_backend_event *out)
{
	struct backend *be = ctx;

	if (!be->latch_ready) {
		return false;
	}
	be->latch_ready = false;
	*out = (ce_backend_event){
		.submission = be->latch_submission,
		.output = OUTPUT_ID,
		.kind = CE_BACKEND_LATCHED,
	};
	return true;
}

static const ce_backend_ops ops = {
	.capabilities = be_capabilities,
	.check = be_check,
	.commit = be_commit,
	.poll_event = be_poll_event,
};

static struct backend be;
static uint8_t engine_storage[CE_ENGINE_SIZE] __aligned(CE_ENGINE_ALIGN);

struct seen {
	int presented;
	int released_bg;
	int released_fg;
	int dropped;
	int other;
	uint64_t presented_submission;
};

static void drain(ce_engine *e, struct seen *s)
{
	ce_event ev;
	int32_t r;

	memset(s, 0, sizeof(*s));
	while ((r = ce_poll_event(e, &ev)) == 1) {
		switch (ev.kind) {
		case CE_EVENT_PRESENTED:
			s->presented++;
			s->presented_submission = ev.submission;
			break;
		case CE_EVENT_RELEASED:
			if (ev.buffer == BUF_BG) {
				s->released_bg++;
			} else if (ev.buffer == BUF_FG) {
				s->released_fg++;
			} else {
				s->other++;
			}
			break;
		case CE_EVENT_DROPPED:
			s->dropped++;
			break;
		default:
			s->other++;
			break;
		}
	}
	if (r < 0) {
		s->other += 100;
	}
}

static void run(void)
{
	be.display = DEVICE_DT_GET(DT_CHOSEN(zephyr_display));
	CHECK(device_is_ready(be.display), "dummy display device ready");
	if (!device_is_ready(be.display)) {
		return;
	}
	display_get_capabilities(be.display, &be.dcaps);
	CHECK(be.dcaps.x_resolution == SCREEN_W && be.dcaps.y_resolution == SCREEN_H,
	      "display reports the size of the overlay node");
	CHECK(display_set_pixel_format(be.display, PIXEL_FORMAT_ARGB_8888) == 0,
	      "display takes ARGB_8888");
	display_get_capabilities(be.display, &be.dcaps);
	CHECK(display_blanking_off(be.display) == 0, "display_blanking_off");

	CHECK(CE_ABI_CHECK(), "engine ABI check (header and library agree)");
	ce_engine *e = ce_engine_init(engine_storage, sizeof(engine_storage), &ops, &be);

	CHECK(e != NULL, "ce_engine_init");
	if (e == NULL) {
		return;
	}

	for (size_t i = 0; i < ARRAY_SIZE(bg_pixels); i++) {
		bg_pixels[i] = BG_COLOR;
	}
	for (size_t i = 0; i < ARRAY_SIZE(fg_pixels); i++) {
		fg_pixels[i] = FG_COLOR;
	}
	for (size_t i = 0; i < ARRAY_SIZE(buffers); i++) {
		ce_buffer_info info = {
			.modifier = CE_MODIFIER_LINEAR,
			.width = buffers[i].width,
			.height = buffers[i].height,
			.format = buffers[i].format,
		};
		CHECK(ce_register_buffer(e, buffers[i].id, &info) == CE_OK, "ce_register_buffer");
	}
	CHECK(ce_begin_write(e, BUF_BG) == CE_OK && ce_finish_write(e, BUF_BG) == CE_OK,
	      "background buffer written and finished");
	CHECK(ce_begin_write(e, BUF_FG) == CE_OK && ce_finish_write(e, BUF_FG) == CE_OK,
	      "foreground buffer written and finished");
	CHECK(ce_begin_write(e, BUF_RGB565) == CE_OK && ce_finish_write(e, BUF_RGB565) == CE_OK,
	      "RGB565 buffer written and finished");

	/* 1. The two-layer scene: background full screen, foreground over it. */
	ce_layer two[2] = {
		{.buffer = BUF_BG,
		 .source = {0, 0, SCREEN_W, SCREEN_H},
		 .destination = {0, 0, SCREEN_W, SCREEN_H},
		 .z_order = 0,
		 .alpha = CE_ALPHA_OPAQUE,
		 .blend = CE_BLEND_OPAQUE},
		{.buffer = BUF_FG,
		 .source = {0, 0, FG_W, FG_H},
		 .destination = {16, 8, FG_W, FG_H},
		 .z_order = 1,
		 .alpha = CE_ALPHA_OPAQUE,
		 .blend = CE_BLEND_OPAQUE},
	};
	ce_frame frame = {.layers = two, .layer_count = 2, .output = OUTPUT_ID};
	uint64_t sub1 = 0;
	ce_error_info err = {0};
	ce_status st = ce_present(e, &frame, &sub1, &err);

	CHECK(st == CE_OK, "ce_present of the two-layer scene");
	CHECK(be.commit_count == 1 && be.write_count == 2, "two display_write calls for two layers");
	CHECK(be.write_count == 2 && be.writes[0].x == 0 && be.writes[0].y == 0 &&
		      be.writes[0].width == SCREEN_W && be.writes[0].height == SCREEN_H &&
		      be.writes[0].pitch == SCREEN_W && be.writes[0].first_pixel == BG_COLOR,
	      "first write: background, full screen, background pixels");
	CHECK(be.write_count == 2 && be.writes[1].x == 16 && be.writes[1].y == 8 &&
		      be.writes[1].width == FG_W && be.writes[1].height == FG_H &&
		      be.writes[1].pitch == FG_W && be.writes[1].first_pixel == FG_COLOR,
	      "second write: foreground at (16, 8), foreground pixels");
	CHECK(ce_buffer_state(e, BUF_BG) >= CE_BUFFER_SUBMITTED &&
		      ce_buffer_state(e, BUF_FG) >= CE_BUFFER_SUBMITTED,
	      "both buffers submitted or scanning, no longer writable");

	struct seen s;

	drain(e, &s);
	CHECK(s.presented == 1 && s.presented_submission == sub1 && s.dropped == 0 &&
		      s.other == 0,
	      "one PRESENTED for the submission after poll");
	CHECK(ce_buffer_state(e, BUF_BG) == CE_BUFFER_SCANNING &&
		      ce_buffer_state(e, BUF_FG) == CE_BUFFER_SCANNING,
	      "both buffers scanning after the latch");
	CHECK(ce_begin_write(e, BUF_FG) == CE_ERR_BUFFER_IN_USE,
	      "a scanned buffer cannot be rewritten");

	/* 2. Background only: the foreground buffer is released by this latch. */
	frame.layer_count = 1;
	uint64_t sub2 = 0;

	st = ce_present(e, &frame, &sub2, &err);
	CHECK(st == CE_OK && sub2 != sub1, "ce_present of the background alone");
	CHECK(be.commit_count == 2 && be.write_count == 1, "one display_write for one layer");
	drain(e, &s);
	CHECK(s.presented == 1 && s.presented_submission == sub2, "PRESENTED for the second submission");
	CHECK(s.released_fg == 1 && s.released_bg == 0, "foreground released, background still shown");
	CHECK(ce_buffer_state(e, BUF_FG) == CE_BUFFER_READY, "foreground idle again");

	/* 3. Refusals change nothing and write nothing. */
	int writes_before = be.commit_count;
	ce_layer bad[2] = {two[0], two[1]};

	bad[1].buffer = BUF_RGB565; /* no plane takes RGB565 */
	frame.layers = bad;
	frame.layer_count = 2;
	st = ce_present(e, &frame, NULL, &err);
	CHECK(st == CE_ERR_NO_PLANE && (err.reasons & CE_REJECT_FORMAT) != 0,
	      "RGB565 layer refused with a format reason");
	ce_layer three[3] = {two[0], two[1], two[1]};

	three[2].destination.x = 0;
	frame.layers = three;
	frame.layer_count = 3;
	st = ce_present(e, &frame, NULL, &err);
	CHECK(st < 0, "three layers on two planes refused");
	CHECK(be.commit_count == writes_before, "refused frames reached no display_write");
	drain(e, &s);
	CHECK(s.presented == 0 && s.dropped == 0, "refused frames produced no events");

	/* 4. An empty frame blanks the output: no layers, no write. */
	frame.layers = two;
	frame.layer_count = 0;
	uint64_t sub3 = 0;

	st = ce_present(e, &frame, &sub3, &err);
	CHECK(st == CE_OK, "empty frame accepted");
	drain(e, &s);
	CHECK(s.presented == 1 && s.released_bg == 1, "empty frame presented, background released");
}

int main(void)
{
	printk("display_dummy: engine over the dummy display, board %s\n", CONFIG_BOARD_TARGET);
	printk("NOT a hardware result: QEMU, a display that shows nothing, no VSYNC\n");
	run();
	size_t unused = 0;

	if (k_thread_stack_space_get(k_current_get(), &unused) == 0) {
		printk("main stack: %u of %u bytes never used\n", (unsigned int)unused,
		       (unsigned int)CONFIG_MAIN_STACK_SIZE);
	}
	printk("%d checks, %d failed\n", checks, failures);
	printk("RESULT: %s\n", failures == 0 ? "PASS" : "FAIL");
	/* QEMU has no exit for the guest here: the caller stops it (README). */
	k_sleep(K_FOREVER);
	return 0;
}
