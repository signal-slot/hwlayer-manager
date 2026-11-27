/*
 * Drives the composition engine through its C ABI under Zephyr.
 *
 * The backend is a fake display controller in this file: one 800x480 output,
 * a primary plane (XRGB8888, full screen only) and an overlay plane
 * (ARGB8888) above it. Commits write shadow registers; a k_timer interrupt
 * stands in for vertical blank and latches them, as a real display
 * interrupt would. Nothing touches hardware.
 *
 * Output: one line per step, "ce: <what> key=value ...", then "PASS" or
 * "FAIL <line> <condition>". With semihosting the run ends QEMU with exit
 * status 0 on PASS and 1 on FAIL.
 */
#include <zephyr/kernel.h>
#include <zephyr/sys/printk.h>
#include <zephyr/arch/common/semihost.h>

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "composition_engine.h"

enum { OUT = 0, PLANE_PRIMARY = 10, PLANE_OVERLAY = 11 };
enum { BUF_BG_A = 1, BUF_BG_B = 2, BUF_OSD = 3 };
enum { WIDTH = 800, HEIGHT = 480, VBLANK_MS = 16 };
/* Latch timeout for the engine, in milliseconds of k_uptime_get(). */
enum { LATCH_TIMEOUT_MS = 200 };

#define CHECK(cond)                                                            \
	do {                                                                   \
		if (!(cond)) {                                                 \
			fail(__LINE__, #cond);                                 \
			return false;                                          \
		}                                                              \
	} while (0)

static void finish(bool pass)
{
	/* Semihosting SYS_EXIT. ADP_Stopped_ApplicationExit (0x20026) makes
	 * QEMU exit with status 0; any other reason makes it exit with 1.
	 */
	const enum semihost_instr sys_exit = (enum semihost_instr)0x18;

	semihost_exec(sys_exit, (void *)(uintptr_t)(pass ? 0x20026 : 0x20023));
}

static void fail(int line, const char *what)
{
	printk("FAIL %d %s\n", line, what);
}

/* ---- Fake display controller ---------------------------------------- */

struct fake_plane_regs {
	uint32_t buffer; /* 0: plane off */
	ce_rect source;
	ce_rect destination;
	int32_t zpos;
};

struct fake_dc {
	/* Written by commit, latched by the vblank interrupt. */
	struct fake_plane_regs shadow[2];
	struct fake_plane_regs active[2];
	bool pending;
	uint64_t pending_submission;
	/* Set by the interrupt, consumed by poll_event. */
	bool latched;
	uint64_t latched_submission;
	uint32_t vblanks;
	uint32_t commits;
};

static struct fake_dc dc;

static void vblank_isr(struct k_timer *timer)
{
	ARG_UNUSED(timer);
	dc.vblanks++;
	if (dc.pending) {
		memcpy(dc.active, dc.shadow, sizeof(dc.active));
		dc.pending = false;
		dc.latched = true;
		dc.latched_submission = dc.pending_submission;
	}
}

K_TIMER_DEFINE(vblank_timer, vblank_isr, NULL);

static void fake_capabilities(void *ctx, ce_capabilities *out)
{
	ARG_UNUSED(ctx);
	out->output_count = 1;
	out->outputs[0] = (ce_output_caps){
		.id = OUT, .width = WIDTH, .height = HEIGHT, .refresh_mhz = 60000,
	};
	out->plane_count = 2;
	out->planes[0] = (ce_plane_caps){
		.id = PLANE_PRIMARY,
		.kind = CE_PLANE_PRIMARY,
		.outputs = 1,
		.format_count = 1,
		.zpos_min = 0,
		.zpos_max = 0,
		.blend_modes = 1u << CE_BLEND_OPAQUE,
		.transforms = 1u << CE_TRANSFORM_NORMAL,
		.full_output_only = 1,
	};
	out->planes[0].formats[0] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_XRGB8888 };
	out->planes[1] = (ce_plane_caps){
		.id = PLANE_OVERLAY,
		.kind = CE_PLANE_OVERLAY,
		.outputs = 1,
		.format_count = 1,
		.zpos_min = 1,
		.zpos_max = 1,
		.blend_modes = (1u << CE_BLEND_PREMULTIPLIED) | (1u << CE_BLEND_COVERAGE),
		.transforms = 1u << CE_TRANSFORM_NORMAL,
	};
	out->planes[1].formats[0] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_ARGB8888 };
	/* A commit before the latch rewrites the shadow registers. */
	out->replace_pending = 1;
}

static ce_status fake_check(void *ctx, const ce_frame *frame, const ce_assignment *assignments,
			    size_t count)
{
	ARG_UNUSED(ctx);
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
	struct fake_plane_regs regs[2] = { 0 };

	ARG_UNUSED(ctx);
	for (size_t i = 0; i < count; i++) {
		const ce_assignment *a = &assignments[i];

		regs[a->plane_index] = (struct fake_plane_regs){
			.buffer = frame->layers[a->layer].buffer,
			.source = a->source,
			.destination = a->destination,
			.zpos = a->zpos,
		};
		printk("ce: commit submission=%u layer=%u buffer=%u plane=%u zpos=%d "
		       "src=%d,%d,%ux%u dst=%d,%d,%ux%u\n",
		       (unsigned int)submission, a->layer, frame->layers[a->layer].buffer,
		       a->plane, a->zpos, a->source.x, a->source.y, a->source.width,
		       a->source.height, a->destination.x, a->destination.y,
		       a->destination.width, a->destination.height);
	}

	/* The vblank interrupt must see the registers and the flag together. */
	unsigned int key = irq_lock();

	memcpy(dc.shadow, regs, sizeof(dc.shadow));
	dc.pending = true;
	dc.pending_submission = submission;
	dc.commits++;
	irq_unlock(key);
	return CE_OK;
}

static bool fake_poll_event(void *ctx, ce_backend_event *out)
{
	bool have = false;

	ARG_UNUSED(ctx);
	unsigned int key = irq_lock();

	if (dc.latched) {
		dc.latched = false;
		out->kind = CE_BACKEND_LATCHED;
		out->output = OUT;
		out->submission = dc.latched_submission;
		have = true;
	}
	irq_unlock(key);
	return have;
}

/* ---- Test sequence -------------------------------------------------- */

static _Alignas(CE_ENGINE_ALIGN) unsigned char engine_storage[CE_ENGINE_SIZE];

static const char *event_name(uint8_t kind)
{
	switch (kind) {
	case CE_EVENT_PRESENTED:
		return "presented";
	case CE_EVENT_DROPPED:
		return "dropped";
	case CE_EVENT_RELEASED:
		return "released";
	case CE_EVENT_DEVICE_LOST:
		return "device_lost";
	case CE_EVENT_CAPABILITIES_CHANGED:
		return "capabilities_changed";
	case CE_EVENT_LATCH_TIMEOUT:
		return "latch_timeout";
	default:
		return "unknown";
	}
}

/*
 * Polls the engine until it reports an event or the deadline passes.
 * Returns 1 with *event filled, 0 on timeout, or a negative ce_status.
 */
static int32_t wait_event(ce_engine *engine, ce_event *event, int64_t deadline_ms)
{
	for (;;) {
		int64_t now = k_uptime_get();
		ce_status status = ce_advance_time(engine, (uint64_t)now);

		if (status != CE_OK) {
			return status;
		}
		int32_t got = ce_poll_event(engine, event);

		if (got != 0) {
			if (got == 1) {
				printk("ce: event kind=%s output=%u submission=%u buffer=%u "
				       "uptime_ms=%u\n",
				       event_name(event->kind), event->output,
				       (unsigned int)event->submission, event->buffer,
				       (unsigned int)now);
			}
			return got;
		}
		if (now >= deadline_ms) {
			return 0;
		}
		k_msleep(1);
	}
}

static ce_layer full_screen(uint32_t buffer)
{
	return (ce_layer){
		.buffer = buffer,
		.source = { 0, 0, WIDTH, HEIGHT },
		.destination = { 0, 0, WIDTH, HEIGHT },
		.z_order = 0,
		.alpha = CE_ALPHA_OPAQUE,
		.blend = CE_BLEND_OPAQUE,
		.transform = CE_TRANSFORM_NORMAL,
	};
}

static ce_layer osd(void)
{
	return (ce_layer){
		.buffer = BUF_OSD,
		.source = { 0, 0, 320, 80 },
		.destination = { WIDTH - 320 - 16, HEIGHT - 80 - 16, 320, 80 },
		.z_order = 1,
		.alpha = CE_ALPHA_OPAQUE,
		.blend = CE_BLEND_PREMULTIPLIED,
		.transform = CE_TRANSFORM_NORMAL,
	};
}

/*
 * Presents a two-layer frame (background + OSD), waits for its latch, and
 * expects the given buffer to be released by it (0: none).
 */
static bool present_and_latch(ce_engine *engine, uint32_t background, uint32_t released)
{
	const ce_layer layers[2] = { full_screen(background), osd() };
	const ce_frame frame = { .layers = layers, .layer_count = 2, .output = OUT };
	const int64_t deadline = k_uptime_get() + 2 * LATCH_TIMEOUT_MS;
	ce_error_info info;
	uint64_t submission = 0;
	ce_event event;

	ce_status status = ce_present(engine, &frame, &submission, &info);

	printk("ce: present submission=%u status=%d layers=2\n", (unsigned int)submission,
	       status);
	CHECK(status == CE_OK);
	/* Accepted is not shown: until the vblank the buffer is only submitted. */
	CHECK(ce_buffer_state(engine, background) == CE_BUFFER_SUBMITTED ||
	      ce_buffer_state(engine, background) == CE_BUFFER_SCANNING);

	CHECK(wait_event(engine, &event, deadline) == 1);
	CHECK(event.kind == CE_EVENT_PRESENTED);
	CHECK(event.submission == submission && event.output == OUT);
	CHECK(dc.active[0].buffer == background && dc.active[1].buffer == BUF_OSD);
	CHECK(dc.active[1].destination.x == WIDTH - 320 - 16);
	CHECK(ce_buffer_state(engine, background) == CE_BUFFER_SCANNING);
	CHECK(ce_buffer_state(engine, BUF_OSD) == CE_BUFFER_SCANNING);

	if (released != 0) {
		CHECK(wait_event(engine, &event, deadline) == 1);
		CHECK(event.kind == CE_EVENT_RELEASED && event.buffer == released);
		CHECK(ce_buffer_state(engine, released) == CE_BUFFER_READY);
	}
	/* Nothing else is due. */
	CHECK(ce_poll_event(engine, &event) == 0);
	return true;
}

static bool run(void)
{
	static const ce_backend_ops ops = {
		.capabilities = fake_capabilities,
		.check = fake_check,
		.commit = fake_commit,
		.poll_event = fake_poll_event,
	};
	const ce_buffer_info bg_info = { CE_MODIFIER_LINEAR, WIDTH, HEIGHT, CE_FORMAT_XRGB8888 };
	const ce_buffer_info osd_info = { CE_MODIFIER_LINEAR, 320, 80, CE_FORMAT_ARGB8888 };

	CHECK(CE_ABI_CHECK());
	printk("ce: abi_check ok engine_size=%u\n", (unsigned int)CE_ENGINE_SIZE);

	ce_engine *engine = ce_engine_init(engine_storage, sizeof(engine_storage), &ops, NULL);

	CHECK(engine != NULL);
	CHECK(ce_set_latch_timeout(engine, LATCH_TIMEOUT_MS) == CE_OK);

	CHECK(ce_register_buffer(engine, BUF_BG_A, &bg_info) == CE_OK);
	CHECK(ce_register_buffer(engine, BUF_BG_B, &bg_info) == CE_OK);
	CHECK(ce_register_buffer(engine, BUF_OSD, &osd_info) == CE_OK);
	CHECK(ce_finish_write(engine, BUF_BG_A) == CE_OK);
	CHECK(ce_finish_write(engine, BUF_BG_B) == CE_OK);
	CHECK(ce_finish_write(engine, BUF_OSD) == CE_OK);
	printk("ce: buffers registered count=3\n");

	k_timer_start(&vblank_timer, K_MSEC(VBLANK_MS), K_MSEC(VBLANK_MS));

	/* Frame 1: background A under the OSD. */
	if (!present_and_latch(engine, BUF_BG_A, 0)) {
		return false;
	}
	/* Frame 2: background B. A is released at B's latch, not before. */
	CHECK(ce_begin_write(engine, BUF_BG_B) == CE_OK);
	CHECK(ce_finish_write(engine, BUF_BG_B) == CE_OK);
	if (!present_and_latch(engine, BUF_BG_B, BUF_BG_A)) {
		return false;
	}
	/* The producer may draw into A again, but not into B on screen. */
	CHECK(ce_begin_write(engine, BUF_BG_A) == CE_OK);
	CHECK(ce_begin_write(engine, BUF_BG_B) == CE_ERR_BUFFER_IN_USE);
	CHECK(ce_finish_write(engine, BUF_BG_A) == CE_OK);
	printk("ce: reuse buffer=%u ok\n", BUF_BG_A);

	/* Frame 3: back to A; B is released. */
	if (!present_and_latch(engine, BUF_BG_A, BUF_BG_B)) {
		return false;
	}

	k_timer_stop(&vblank_timer);
	CHECK(ce_unregister_buffer(engine, BUF_BG_B) == CE_OK);
	CHECK(ce_unregister_buffer(engine, BUF_BG_A) == CE_ERR_BUFFER_IN_USE);
	printk("ce: summary commits=%u vblanks=%u\n", dc.commits, dc.vblanks);
	return true;
}

int main(void)
{
	printk("ce: start board=%s\n", CONFIG_BOARD_TARGET);
	bool pass = run();

	size_t unused = 0;

	if (k_thread_stack_space_get(k_current_get(), &unused) == 0) {
		printk("ce: stack size=%u unused=%u\n", (unsigned int)CONFIG_MAIN_STACK_SIZE,
		       (unsigned int)unused);
	}
	printk("%s\n", pass ? "PASS" : "FAIL");
	finish(pass);
	return 0;
}
