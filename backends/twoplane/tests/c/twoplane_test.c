/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * Host test of the two-plane backend (ce_twoplane.c) behind a fake driver.
 *
 * The fake models what ce_twoplane.h asks of a driver: every call that
 * writes the planes goes to shadow registers and counts one commit; a frame
 * start copies the shadow registers to the active ones and makes the latched
 * count equal the commit count, then runs the vsync callback. The test
 * decides when a frame starts, including in the middle of the backend's
 * writes, and whether the interrupt runs then (it waits while the port's
 * interrupt lock is held, as a masked interrupt does).
 *
 * Tests named engine_* drive the backend through the engine's C API
 * (ce_present, ce_poll_event). Tests named ops_* call ce_twoplane_ops
 * directly, for what the engine never does (a commit while a latch is
 * unreported) or to look at the backend's state between the engine's calls.
 *
 * Exit status 0 means every check passed.
 */
#include "ce_twoplane.h"
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

#define RUN(test) \
	do { \
		reset(); \
		if (test() != 0 || port_misused()) { \
			fprintf(stderr, "FAILED %s\n", #test); \
			return 1; \
		} \
		printf("ok %s\n", #test); \
	} while (0)

enum { WIDTH = 800, HEIGHT = 480 };

/* Buffer ids. */
enum {
	APP_A = 1,
	APP_B = 2,
	APP_NO_PA = 3,    /* in the table, address 0 */
	APP_UNLISTED = 4, /* registered with the engine, not in the table */
	APP_XRGB = 5,     /* XRGB8888, which the fake driver may refuse */
	SAFETY_0 = 10,    /* the driver's first safety buffer */
	SAFETY_1 = 11,    /* the driver's second safety buffer */
	SAFETY_OTHER = 12, /* ARGB8888, but not the driver's */
	SAFETY_UNLISTED = 13,
};

/* ---- The fake driver ---- */

struct regs {
	struct ce_twoplane_buf app;
	bool app_on;
	unsigned safety; /* index into fake.safety_mem */
};

static struct fake {
	bool formats_ok[3]; /* RGB565, XRGB8888, ARGB8888 */
	struct regs shadow;
	struct regs active;
	uint32_t commits; /* commit_count */
	uint32_t latched; /* latched_count */
	bool hold;        /* frame starts latch nothing (the commit missed them) */
	unsigned frame_starts;
	/* calls */
	unsigned writes;
	unsigned set_app_calls;
	unsigned enable_on_calls;
	unsigned enable_off_calls;
	unsigned flip_calls;
	unsigned premultiplied_calls;
	bool premultiplied;
	int refuse_set_app;
	unsigned wrong_dev;
	ce_twoplane_vsync_cb_t cb;
	void *cb_user;
	/* scripted frame starts: after the n-th write from now, or at the n-th
	 * app_format_ok call from now (0: none), running the interrupt or not */
	unsigned start_after_write;
	unsigned start_at_format_ok;
	bool start_runs_isr;
	/* the port */
	bool ready;
	unsigned lock_depth;
	unsigned max_lock_depth;
	bool bad_unlock;
	bool isr_deferred;
	unsigned isr_runs;
	unsigned wakes;
	uint32_t safety_mem[2][16];
	uint32_t other_mem[16];
} fake;

static const char wake_token[] = "wake";
static const char lookup_token[] = "lookup";
static unsigned wrong_port_arg;

static void run_isr(void)
{
	if (fake.cb == NULL) {
		return;
	}
	if (fake.lock_depth > 0) {
		fake.isr_deferred = true; /* masked: runs at irq_unlock */
		return;
	}
	fake.isr_runs++;
	fake.cb(&fake, fake.cb_user);
}

/* A frame start: the hardware takes the shadow registers if a commit is
 * waiting, then raises its interrupt if isr.
 */
static void frame_start(bool isr)
{
	fake.frame_starts++;
	if (!fake.hold && fake.latched != fake.commits) {
		fake.active = fake.shadow;
		fake.latched = fake.commits;
	}
	if (isr) {
		run_isr();
	}
}

static void check_dev(const void *dev)
{
	if (dev != &fake) {
		fake.wrong_dev++;
	}
}

static void wrote(void)
{
	fake.commits++;
	fake.writes++;
	if (fake.start_after_write > 0 && --fake.start_after_write == 0) {
		frame_start(fake.start_runs_isr);
	}
}

static int format_index(uint32_t format)
{
	switch (format) {
	case CE_FORMAT_RGB565:
		return 0;
	case CE_FORMAT_XRGB8888:
		return 1;
	case CE_FORMAT_ARGB8888:
		return 2;
	default:
		return -1;
	}
}

static bool hw_app_format_ok(uint32_t format)
{
	int i = format_index(format);

	if (fake.start_at_format_ok > 0 && --fake.start_at_format_ok == 0) {
		frame_start(fake.start_runs_isr);
	}
	return i >= 0 && fake.formats_ok[i];
}

static int hw_set_app_buffer(const void *dev, const struct ce_twoplane_buf *buf)
{
	check_dev(dev);
	fake.set_app_calls++;
	if (fake.refuse_set_app) {
		return fake.refuse_set_app;
	}
	fake.shadow.app = *buf;
	wrote();
	return 0;
}

static int hw_enable_app_plane(const void *dev, bool enable)
{
	check_dev(dev);
	if (enable) {
		fake.enable_on_calls++;
	} else {
		fake.enable_off_calls++;
	}
	fake.shadow.app_on = enable;
	wrote();
	return 0;
}

static int hw_safety_flip(const void *dev)
{
	check_dev(dev);
	fake.flip_calls++;
	fake.shadow.safety ^= 1u;
	wrote();
	return 0;
}

static void *hw_safety_buffer(const void *dev, uint32_t *width, uint32_t *height,
			      uint32_t *stride_bytes)
{
	check_dev(dev);
	*width = WIDTH;
	*height = HEIGHT;
	*stride_bytes = WIDTH * 4;
	return fake.safety_mem[fake.shadow.safety ^ 1u];
}

static void *hw_safety_front(const void *dev)
{
	check_dev(dev);
	return fake.safety_mem[fake.active.safety];
}

static int hw_safety_premultiplied(const void *dev, bool on)
{
	check_dev(dev);
	fake.premultiplied_calls++;
	fake.premultiplied = on;
	return 0;
}

static uint32_t hw_commit_count(const void *dev)
{
	check_dev(dev);
	return fake.commits;
}

static uint32_t hw_latched_count(const void *dev)
{
	check_dev(dev);
	return fake.latched;
}

static void hw_set_vsync_callback(const void *dev, ce_twoplane_vsync_cb_t cb, void *user)
{
	check_dev(dev);
	fake.cb = cb;
	fake.cb_user = user;
}

static const struct ce_twoplane_hw hw = {
	.app_format_ok = hw_app_format_ok,
	.set_app_buffer = hw_set_app_buffer,
	.enable_app_plane = hw_enable_app_plane,
	.safety_flip = hw_safety_flip,
	.safety_buffer = hw_safety_buffer,
	.safety_front = hw_safety_front,
	.safety_premultiplied = hw_safety_premultiplied,
	.commit_count = hw_commit_count,
	.latched_count = hw_latched_count,
	.set_vsync_callback = hw_set_vsync_callback,
};

/* ---- The port and the integrator's buffer table ---- */

static struct ce_twoplane_record table[8];
static size_t table_len;

static unsigned int port_irq_lock(void)
{
	unsigned int key = fake.lock_depth++;

	if (fake.lock_depth > fake.max_lock_depth) {
		fake.max_lock_depth = fake.lock_depth;
	}
	return key;
}

static void port_irq_unlock(unsigned int key)
{
	if (fake.lock_depth == 0 || key != fake.lock_depth - 1) {
		fake.bad_unlock = true;
		return;
	}
	fake.lock_depth = key;
	if (fake.lock_depth == 0 && fake.isr_deferred) {
		fake.isr_deferred = false;
		run_isr();
	}
}

static void port_wake(void *arg)
{
	if (arg != wake_token) {
		wrong_port_arg++;
	}
	fake.wakes++;
}

static const struct ce_twoplane_record *port_lookup(void *arg, uint32_t id)
{
	if (arg != lookup_token) {
		wrong_port_arg++;
	}
	for (size_t i = 0; i < table_len; i++) {
		if (table[i].id == id) {
			return &table[i];
		}
	}
	return NULL;
}

static bool port_device_ready(const void *dev)
{
	check_dev(dev);
	return fake.ready;
}

static const struct ce_twoplane_port port = {
	.irq_lock = port_irq_lock,
	.irq_unlock = port_irq_unlock,
	.wake = port_wake,
	.wake_arg = (void *)wake_token,
	.lookup = port_lookup,
	.lookup_arg = (void *)lookup_token,
	.device_ready = port_device_ready,
};

static void add_record(uint32_t id, uint32_t seq, uintptr_t pa, uint32_t format)
{
	uint32_t bpp = format == CE_FORMAT_RGB565 ? 2 : 4;

	table[table_len++] = (struct ce_twoplane_record){
		.id = id,
		.seq = seq,
		.pa = pa,
		.width = WIDTH,
		.height = HEIGHT,
		.stride_bytes = WIDTH * bpp,
		.format = format,
	};
}

static struct ce_twoplane_record *record_of(uint32_t id)
{
	for (size_t i = 0; i < table_len; i++) {
		if (table[i].id == id) {
			return &table[i];
		}
	}
	return NULL;
}

/* Application buffers have made-up addresses: the fake never reads them. */
static uint32_t app_mem[3][16];

static bool port_misused(void)
{
	bool bad = fake.lock_depth != 0 || fake.max_lock_depth > 1 || fake.bad_unlock ||
		   fake.isr_deferred || fake.wrong_dev != 0 || wrong_port_arg != 0;

	if (bad) {
		fprintf(stderr,
			"port misused: depth %u max %u bad_unlock %d deferred %d wrong_dev %u "
			"wrong_arg %u\n",
			fake.lock_depth, fake.max_lock_depth, fake.bad_unlock, fake.isr_deferred,
			fake.wrong_dev, wrong_port_arg);
	}
	return bad;
}

static void reset(void)
{
	memset(&fake, 0, sizeof(fake));
	fake.ready = true;
	fake.formats_ok[0] = fake.formats_ok[1] = fake.formats_ok[2] = true;
	wrong_port_arg = 0;
	table_len = 0;
	add_record(APP_A, 1, (uintptr_t)app_mem[0], CE_FORMAT_RGB565);
	add_record(APP_B, 1, (uintptr_t)app_mem[1], CE_FORMAT_RGB565);
	add_record(APP_NO_PA, 1, 0, CE_FORMAT_RGB565);
	add_record(APP_XRGB, 1, (uintptr_t)app_mem[2], CE_FORMAT_XRGB8888);
	add_record(SAFETY_0, 0, (uintptr_t)fake.safety_mem[0], CE_FORMAT_ARGB8888);
	add_record(SAFETY_1, 0, (uintptr_t)fake.safety_mem[1], CE_FORMAT_ARGB8888);
	add_record(SAFETY_OTHER, 0, (uintptr_t)fake.other_mem, CE_FORMAT_ARGB8888);
}

/* ---- The engine ---- */

static struct ce_twoplane backend;
static _Alignas(CE_ENGINE_ALIGN) unsigned char storage[CE_ENGINE_SIZE];
static ce_engine *engine;

static ce_layer app_layer(uint32_t buffer)
{
	return (ce_layer){
		.buffer = buffer,
		.source = {0, 0, WIDTH, HEIGHT},
		.destination = {0, 0, WIDTH, HEIGHT},
		.z_order = 0,
		.alpha = CE_ALPHA_OPAQUE,
		.blend = CE_BLEND_OPAQUE,
		.transform = CE_TRANSFORM_NORMAL,
	};
}

static ce_layer safety_layer(uint32_t buffer)
{
	ce_layer l = app_layer(buffer);

	l.z_order = 10;
	l.blend = CE_BLEND_PREMULTIPLIED;
	return l;
}

/* The backend and an engine on it, every buffer registered and ready. */
static int start_engine(void)
{
	static const struct {
		uint32_t id;
		uint32_t format;
	} buffers[] = {
		{APP_A, CE_FORMAT_RGB565},         {APP_B, CE_FORMAT_RGB565},
		{APP_NO_PA, CE_FORMAT_RGB565},     {APP_UNLISTED, CE_FORMAT_RGB565},
		{APP_XRGB, CE_FORMAT_XRGB8888},    {SAFETY_0, CE_FORMAT_ARGB8888},
		{SAFETY_1, CE_FORMAT_ARGB8888},    {SAFETY_OTHER, CE_FORMAT_ARGB8888},
		{SAFETY_UNLISTED, CE_FORMAT_ARGB8888},
	};

	CHECK(ce_twoplane_init(&backend, &hw, &port, &fake) == 0);
	engine = ce_engine_init(storage, sizeof(storage), &ce_twoplane_ops, &backend);
	CHECK(engine != NULL);
	for (size_t i = 0; i < sizeof(buffers) / sizeof(buffers[0]); i++) {
		const ce_buffer_info info = {CE_MODIFIER_LINEAR, WIDTH, HEIGHT, buffers[i].format};

		CHECK(ce_register_buffer(engine, buffers[i].id, &info) == CE_OK);
		CHECK(ce_finish_write(engine, buffers[i].id) == CE_OK);
	}
	return 0;
}

static ce_status present2(uint32_t app, uint32_t safety, uint64_t *submission)
{
	ce_layer layers[2] = {safety_layer(safety), app_layer(app)};
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};

	return ce_present(engine, &frame, submission, NULL);
}

static ce_status present1(ce_layer layer, uint64_t *submission)
{
	ce_frame frame = {.layers = &layer, .layer_count = 1, .output = 0};

	return ce_present(engine, &frame, submission, NULL);
}

/* The next event is kind for submission. */
static bool next_is(uint8_t kind, uint64_t submission)
{
	ce_event e;
	int32_t rc = ce_poll_event(engine, &e);

	if (rc != 1) {
		fprintf(stderr, "  expected event %u, ce_poll_event gave %d\n", kind, rc);
		return false;
	}
	if (e.kind != kind || e.submission != submission) {
		fprintf(stderr, "  expected event %u/%llu, got %u/%llu\n", kind,
			(unsigned long long)submission, e.kind, (unsigned long long)e.submission);
		return false;
	}
	return true;
}

/* `releases` releases, in any order, and no other event. */
static bool only_releases(unsigned releases)
{
	ce_event e;

	for (unsigned i = 0; i < releases; i++) {
		if (ce_poll_event(engine, &e) != 1 || e.kind != CE_EVENT_RELEASED) {
			fprintf(stderr, "  expected a release\n");
			return false;
		}
	}
	return ce_poll_event(engine, &e) == 0;
}

/* Presented, then `releases` releases and nothing else. */
static bool presented(uint64_t submission, unsigned releases)
{
	return next_is(CE_EVENT_PRESENTED, submission) && only_releases(releases);
}

static bool no_event(void)
{
	ce_event e;
	int32_t rc = ce_poll_event(engine, &e);

	if (rc != 0) {
		fprintf(stderr, "  expected no event, got %d (kind %u, submission %llu)\n", rc,
			e.kind, (unsigned long long)e.submission);
	}
	return rc == 0;
}

/* ---- Tests ---- */

/* ce_twoplane_init: the device's readiness, what it sets on the driver. */
static int ops_init(void)
{
	fake.ready = false;
	CHECK(ce_twoplane_init(&backend, &hw, &port, &fake) == -CE_TWOPLANE_ENODEV);
	CHECK(fake.cb == NULL && fake.premultiplied_calls == 0);

	fake.ready = true;
	fake.active.safety = fake.shadow.safety = 1;
	CHECK(ce_twoplane_init(&backend, &hw, &port, &fake) == 0);
	CHECK(backend.width == WIDTH && backend.height == HEIGHT);
	/* the plane already scans the driver's front buffer */
	CHECK(backend.safety_pa == (uintptr_t)fake.safety_mem[1]);
	CHECK(fake.premultiplied_calls == 1 && fake.premultiplied);
	CHECK(fake.cb != NULL && fake.cb_user == &backend);
	CHECK(fake.writes == 0);
	return 0;
}

/* One output of the driver's size, two planes, app formats filtered. */
static int ops_capabilities(void)
{
	static ce_capabilities caps;

	fake.formats_ok[1] = false; /* no XRGB8888 */
	CHECK(ce_twoplane_init(&backend, &hw, &port, &fake) == 0);
	memset(&caps, 0, sizeof(caps));
	ce_twoplane_ops.capabilities(&backend, &caps);

	CHECK(caps.output_count == 1);
	CHECK(caps.outputs[0].id == 0 && caps.outputs[0].width == WIDTH &&
	      caps.outputs[0].height == HEIGHT);
	CHECK(caps.plane_count == 2 && caps.group_count == 0);
	CHECK(caps.replace_pending == 1);

	const ce_plane_caps *app = &caps.planes[0];

	CHECK(app->id == CE_TWOPLANE_PLANE_APP && app->kind == CE_PLANE_PRIMARY);
	CHECK(app->outputs == 1 && app->zpos_min == 0 && app->zpos_max == 0 && !app->zpos_mutable);
	CHECK(app->blend_modes == 1u << CE_BLEND_OPAQUE);
	CHECK(app->transforms == 1u << CE_TRANSFORM_NORMAL && app->full_output_only == 1);
	CHECK(!app->can_scale && !app->plane_alpha);
	CHECK(app->format_count == 2);
	CHECK(app->formats[0].format == CE_FORMAT_RGB565 &&
	      app->formats[0].modifier == CE_MODIFIER_LINEAR);
	CHECK(app->formats[1].format == CE_FORMAT_ARGB8888);

	const ce_plane_caps *safety = &caps.planes[1];

	CHECK(safety->id == CE_TWOPLANE_PLANE_SAFETY && safety->kind == CE_PLANE_OVERLAY);
	CHECK(safety->outputs == 1 && safety->zpos_min == 1 && safety->zpos_max == 1);
	CHECK(safety->blend_modes == 1u << CE_BLEND_PREMULTIPLIED);
	CHECK(safety->transforms == 1u << CE_TRANSFORM_NORMAL && safety->full_output_only == 1);
	CHECK(safety->format_count == 1 && safety->formats[0].format == CE_FORMAT_ARGB8888);

	/* All three when the driver takes all three. */
	fake.formats_ok[1] = true;
	memset(&caps, 0, sizeof(caps));
	ce_twoplane_ops.capabilities(&backend, &caps);
	CHECK(caps.planes[0].format_count == 3 &&
	      caps.planes[0].formats[1].format == CE_FORMAT_XRGB8888);
	return 0;
}

/* The engine sees the filtered list: an application buffer in a format the
 * driver refuses gets no plane.
 */
static int engine_refused_format(void)
{
	ce_error_info info;
	ce_layer layers[2] = {safety_layer(SAFETY_0), app_layer(APP_XRGB)};
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};

	fake.formats_ok[1] = false;
	CHECK(start_engine() == 0);
	CHECK(ce_present(engine, &frame, NULL, &info) == CE_ERR_NO_PLANE);
	CHECK(info.layer == 1 && (info.reasons & CE_REJECT_FORMAT) != 0);
	CHECK(fake.writes == 0);
	return 0;
}

/* Both layers, then the safety layer alone, then both again. */
static int engine_frames(void)
{
	uint64_t s1, s2, s3, s4, s5;

	CHECK(start_engine() == 0);

	/* Both layers: the application plane is set and enabled; the safety
	 * plane already scans SAFETY_0, so no flip.
	 */
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	CHECK(fake.set_app_calls == 1 && fake.enable_on_calls == 1 && fake.flip_calls == 0);
	CHECK(fake.shadow.app.pa == (uintptr_t)app_mem[0] && fake.shadow.app.seq == 1);
	CHECK(fake.shadow.app.format == CE_FORMAT_RGB565 &&
	      fake.shadow.app.stride_bytes == WIDTH * 2 && fake.shadow.app.width == WIDTH &&
	      fake.shadow.app.height == HEIGHT);
	CHECK(no_event());
	CHECK(!fake.active.app_on);
	frame_start(true);
	CHECK(fake.active.app_on && fake.active.app.pa == (uintptr_t)app_mem[0]);
	CHECK(presented(s1, 0));

	/* A new safety frame in the other buffer: one flip, the app plane left
	 * alone.
	 */
	CHECK(present2(APP_A, SAFETY_1, &s2) == CE_OK);
	CHECK(fake.flip_calls == 1 && fake.set_app_calls == 1 && fake.enable_on_calls == 1);
	frame_start(true);
	CHECK(fake.active.safety == 1);
	CHECK(presented(s2, 1)); /* SAFETY_0 released */

	/* The safety layer alone: the application plane is switched off. */
	CHECK(present1(safety_layer(SAFETY_1), &s3) == CE_OK);
	CHECK(fake.enable_off_calls == 1 && fake.set_app_calls == 1 && fake.flip_calls == 1);
	frame_start(true);
	CHECK(!fake.active.app_on);
	CHECK(presented(s3, 1)); /* APP_A released */

	/* Again: already off, nothing written; such a commit is latched at once. */
	unsigned writes = fake.writes;

	CHECK(present1(safety_layer(SAFETY_1), &s4) == CE_OK);
	CHECK(fake.writes == writes && fake.enable_off_calls == 1);
	CHECK(presented(s4, 0));

	/* Both again: the plane was off, so its buffer is set again and it is
	 * enabled, even though it is the buffer it had.
	 */
	CHECK(present2(APP_A, SAFETY_1, &s5) == CE_OK);
	CHECK(fake.set_app_calls == 2 && fake.enable_on_calls == 2);
	frame_start(true);
	CHECK(fake.active.app_on && fake.active.app.pa == (uintptr_t)app_mem[0]);
	CHECK(presented(s5, 0));
	return 0;
}

/* Frames the backend refuses: nothing is written and the engine reports why. */
static int engine_rejected(void)
{
	ce_frame blank = {.layers = NULL, .layer_count = 0, .output = 0};

	CHECK(start_engine() == 0);

	/* No safety layer: the safety plane is never switched off. */
	CHECK(present1(app_layer(APP_A), NULL) == CE_ERR_BACKEND_REJECTED);
	CHECK(ce_present(engine, &blank, NULL, NULL) == CE_ERR_BACKEND_REJECTED);
	/* A safety buffer that is neither of the driver's two. */
	CHECK(present2(APP_A, SAFETY_OTHER, NULL) == CE_ERR_BACKEND_REJECTED);
	CHECK(present1(safety_layer(SAFETY_OTHER), NULL) == CE_ERR_BACKEND_REJECTED);
	/* Ids the integrator's table does not know. */
	CHECK(present1(safety_layer(SAFETY_UNLISTED), NULL) == CE_ERR_BACKEND_UNKNOWN_BUFFER);
	CHECK(present2(APP_UNLISTED, SAFETY_0, NULL) == CE_ERR_BACKEND_UNKNOWN_BUFFER);
	/* An application buffer without an address. */
	CHECK(present2(APP_NO_PA, SAFETY_0, NULL) == CE_ERR_BACKEND_REJECTED);
	CHECK(fake.writes == 0 && fake.set_app_calls == 0);

	/* The driver refuses the application buffer: nothing written, nothing
	 * pending, and the engine still has every buffer.
	 */
	fake.refuse_set_app = -22;
	CHECK(present2(APP_A, SAFETY_1, &(uint64_t){0}) == CE_ERR_BACKEND_REJECTED);
	CHECK(fake.set_app_calls == 1 && fake.writes == 0 && fake.flip_calls == 0);
	CHECK(!backend.pending && !backend.app_on);
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_READY);
	CHECK(no_event());

	/* And the same frame goes through once the driver takes it. */
	uint64_t s;

	fake.refuse_set_app = 0;
	CHECK(present2(APP_A, SAFETY_1, &s) == CE_OK);
	CHECK(fake.set_app_calls == 2 && fake.enable_on_calls == 1 && fake.flip_calls == 1);
	frame_start(true);
	CHECK(presented(s, 0));
	return 0;
}

/* An application buffer drawn again is committed again with its new frame
 * number; the same number is not written again.
 */
static int engine_redraw(void)
{
	uint64_t s1, s2, s3, s4;

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	frame_start(true);
	CHECK(presented(s1, 0));
	CHECK(fake.set_app_calls == 1 && fake.enable_on_calls == 1);

	/* The producer drew into the buffer again (the integrator's table says
	 * frame 2): committed again, not enabled again.
	 */
	record_of(APP_A)->seq = 2;
	CHECK(present2(APP_A, SAFETY_0, &s2) == CE_OK);
	CHECK(fake.set_app_calls == 2 && fake.enable_on_calls == 1 && fake.writes == 3);
	CHECK(fake.shadow.app.seq == 2);
	frame_start(true);
	CHECK(fake.active.app.seq == 2);
	CHECK(presented(s2, 0));

	/* Same buffer, same number: nothing written. With nothing written the
	 * commit counts as latched at once.
	 */
	CHECK(present2(APP_A, SAFETY_0, &s3) == CE_OK);
	CHECK(fake.set_app_calls == 2 && fake.writes == 3);
	CHECK(presented(s3, 0));

	/* Another buffer with the same number: committed, not enabled again. */
	CHECK(present2(APP_B, SAFETY_0, &s4) == CE_OK);
	CHECK(fake.set_app_calls == 3 && fake.enable_on_calls == 1);
	CHECK(fake.shadow.app.pa == (uintptr_t)app_mem[1]);
	frame_start(true);
	CHECK(presented(s4, 1)); /* APP_A released */
	return 0;
}

/* Reported once the latched count reaches the commit's, found by poll_event
 * when the interrupt has not run, and never before.
 */
static int engine_latch_by_poll(void)
{
	uint64_t s1, s2;

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	CHECK(no_event());

	/* A frame start that misses the commit: the interrupt runs and wakes,
	 * nothing is reported.
	 */
	fake.hold = true;
	frame_start(true);
	CHECK(fake.isr_runs == 1 && fake.wakes == 1 && backend.vsyncs == 1);
	CHECK(no_event());
	fake.hold = false;

	/* The hardware latches, its interrupt has not run yet: poll_event finds
	 * it from the counts.
	 */
	frame_start(false);
	CHECK(fake.isr_runs == 1 && fake.wakes == 1);
	CHECK(presented(s1, 0));
	frame_start(true);
	CHECK(fake.wakes == 2);
	CHECK(no_event());

	CHECK(present2(APP_B, SAFETY_1, &s2) == CE_OK);
	frame_start(false);
	CHECK(presented(s2, 2)); /* APP_A and SAFETY_0 released */
	return 0;
}

/* Noted by the vsync callback, held until poll_event, reported once. */
static int ops_latch_by_vsync(void)
{
	ce_layer layers[2] = {app_layer(APP_A), safety_layer(SAFETY_0)};
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};
	ce_assignment a[2] = {{.layer = 0, .plane = CE_TWOPLANE_PLANE_APP, .plane_index = 0},
			      {.layer = 1, .plane = CE_TWOPLANE_PLANE_SAFETY, .plane_index = 1}};
	ce_backend_event ev;

	CHECK(ce_twoplane_init(&backend, &hw, &port, &fake) == 0);
	CHECK(ce_twoplane_ops.check(&backend, &frame, a, 2) == CE_OK);
	CHECK(fake.writes == 0);
	CHECK(ce_twoplane_ops.commit(&backend, 7, &frame, a, 2) == CE_OK);
	CHECK(backend.pending && backend.pending_sub == 7 && backend.pending_count == 2);
	CHECK(!ce_twoplane_ops.poll_event(&backend, &ev));

	frame_start(true);
	CHECK(backend.vsyncs == 1 && fake.wakes == 1);
	CHECK(backend.latched && backend.latched_sub == 7 && !backend.pending);
	CHECK(ce_twoplane_ops.poll_event(&backend, &ev));
	CHECK(ev.kind == CE_BACKEND_LATCHED && ev.submission == 7 && ev.output == 0);
	CHECK(!ce_twoplane_ops.poll_event(&backend, &ev));
	frame_start(true);
	CHECK(!backend.latched && !ce_twoplane_ops.poll_event(&backend, &ev));
	return 0;
}

/* A second commit while the first waits for its latch replaces it: the
 * first is dropped, never reported latched.
 */
static int engine_replace_pending(void)
{
	uint64_t s1, s2;

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	CHECK(present2(APP_B, SAFETY_1, &s2) == CE_OK);
	/* dropped: APP_A and SAFETY_0 go back to the producer */
	CHECK(next_is(CE_EVENT_DROPPED, s1));
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_READY);
	CHECK(ce_buffer_state(engine, SAFETY_0) == CE_BUFFER_READY);
	CHECK(only_releases(2));
	CHECK(backend.pending && backend.pending_sub == s2);

	/* Both commits' writes reach the panel together, as the second. */
	frame_start(true);
	CHECK(fake.active.app.pa == (uintptr_t)app_mem[1] && fake.active.safety == 1 &&
	      fake.active.app_on);
	CHECK(presented(s2, 0));
	return 0;
}

/* The first submission latches while the second is being written: it was on
 * the panel, so it is reported presented, not dropped. Once with the
 * interrupt running at that frame start, once without (found at the end of
 * the commit).
 */
static int latch_during_writes(bool isr)
{
	uint64_t s1, s2;

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);

	/* the second commit writes the app buffer and flips the safety plane; a
	 * frame start falls between the two
	 */
	fake.start_after_write = 1;
	fake.start_runs_isr = isr;
	CHECK(present2(APP_B, SAFETY_1, &s2) == CE_OK);
	CHECK(fake.frame_starts == 1 && fake.isr_runs == (isr ? 1u : 0u));
	/* the panel shows the first frame plus the second's first write */
	CHECK(fake.active.app.pa == (uintptr_t)app_mem[1] && fake.active.safety == 0);
	CHECK(presented(s1, 0));
	CHECK(backend.pending && backend.pending_sub == s2);
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_SCANNING);

	frame_start(true);
	CHECK(fake.active.safety == 1);
	CHECK(presented(s2, 2)); /* APP_A and SAFETY_0 released */
	return 0;
}

static int engine_latch_during_writes_isr(void)
{
	return latch_during_writes(true);
}

static int engine_latch_during_writes_no_isr(void)
{
	return latch_during_writes(false);
}

/* The same on the ops: the first latch is ready when commit returns; the
 * second, latching while the first is unreported, waits and follows it.
 */
static int ops_latch_behind_unreported(void)
{
	ce_layer layers[2] = {app_layer(APP_A), safety_layer(SAFETY_0)};
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};
	ce_assignment a[2] = {{.layer = 0, .plane = CE_TWOPLANE_PLANE_APP, .plane_index = 0},
			      {.layer = 1, .plane = CE_TWOPLANE_PLANE_SAFETY, .plane_index = 1}};
	ce_backend_event ev;

	CHECK(ce_twoplane_init(&backend, &hw, &port, &fake) == 0);
	CHECK(ce_twoplane_ops.commit(&backend, 1, &frame, a, 2) == CE_OK);

	layers[0] = app_layer(APP_B);
	layers[1] = safety_layer(SAFETY_1);
	fake.start_after_write = 1;
	fake.start_runs_isr = false;
	CHECK(ce_twoplane_ops.commit(&backend, 2, &frame, a, 2) == CE_OK);
	CHECK(backend.latched && backend.latched_sub == 1);
	CHECK(backend.pending && backend.pending_sub == 2);

	/* the second latches; the slot is taken, so it stays pending */
	frame_start(true);
	CHECK(backend.latched_sub == 1 && backend.pending);
	CHECK(ce_twoplane_ops.poll_event(&backend, &ev) && ev.submission == 1);
	CHECK(backend.latched && backend.latched_sub == 2 && !backend.pending);
	CHECK(ce_twoplane_ops.poll_event(&backend, &ev) && ev.submission == 2);
	CHECK(!ce_twoplane_ops.poll_event(&backend, &ev));
	return 0;
}

/* A commit while a latch is unreported is refused before any write. */
static int ops_busy(void)
{
	ce_layer layers[2] = {app_layer(APP_A), safety_layer(SAFETY_0)};
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};
	ce_assignment a[2] = {{.layer = 0, .plane = CE_TWOPLANE_PLANE_APP, .plane_index = 0},
			      {.layer = 1, .plane = CE_TWOPLANE_PLANE_SAFETY, .plane_index = 1}};
	ce_backend_event ev;

	CHECK(ce_twoplane_init(&backend, &hw, &port, &fake) == 0);
	CHECK(ce_twoplane_ops.commit(&backend, 1, &frame, a, 2) == CE_OK);
	frame_start(true);
	CHECK(backend.latched);

	layers[0] = app_layer(APP_B);
	layers[1] = safety_layer(SAFETY_1);
	unsigned writes = fake.writes;
	struct regs shadow = fake.shadow;

	CHECK(ce_twoplane_ops.commit(&backend, 2, &frame, a, 2) == CE_ERR_BACKEND_BUSY);
	CHECK(fake.writes == writes && fake.set_app_calls == 1 && fake.flip_calls == 0);
	CHECK(memcmp(&fake.shadow, &shadow, sizeof(shadow)) == 0);
	CHECK(backend.app_id == APP_A && backend.safety_pa == (uintptr_t)fake.safety_mem[0]);
	CHECK(backend.latched_sub == 1 && !backend.pending);

	CHECK(ce_twoplane_ops.poll_event(&backend, &ev) && ev.submission == 1);
	CHECK(ce_twoplane_ops.commit(&backend, 2, &frame, a, 2) == CE_OK);
	CHECK(fake.set_app_calls == 2 && fake.flip_calls == 1);
	frame_start(true);
	CHECK(ce_twoplane_ops.poll_event(&backend, &ev) && ev.submission == 2);
	return 0;
}

/* Through the engine the slot is empty at the commit, except when the
 * interrupt notes a latch after the engine collected the latches and before
 * the commit looks at the slot. ce_present then fails with
 * CE_ERR_BACKEND_BUSY and changes nothing; the latch is reported next.
 *
 * The frame start is placed in the commit's own check (the second
 * app_format_ok of the present: the first is the planner's check).
 */
static int engine_busy_race(void)
{
	uint64_t s1, s2;
	ce_error_info info;
	ce_layer layers[2] = {safety_layer(SAFETY_1), app_layer(APP_B)};
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	unsigned writes = fake.writes;

	fake.start_at_format_ok = 2;
	fake.start_runs_isr = true;
	CHECK(ce_present(engine, &frame, &s2, &info) == CE_ERR_BACKEND_BUSY);
	CHECK(info.status == CE_ERR_BACKEND_BUSY);
	CHECK(fake.frame_starts == 1 && fake.writes == writes);
	CHECK(ce_buffer_state(engine, APP_B) == CE_BUFFER_READY);
	CHECK(presented(s1, 0));

	CHECK(ce_present(engine, &frame, &s2, NULL) == CE_OK);
	frame_start(true);
	CHECK(presented(s2, 2)); /* APP_A and SAFETY_0 released */
	return 0;
}

/* The 32-bit counts wrap around. */
static int engine_wrap(void)
{
	uint64_t s;

	fake.commits = fake.latched = UINT32_MAX - 2;
	CHECK(start_engine() == 0);
	for (unsigned i = 0; i < 6; i++) {
		CHECK(present2(i % 2 ? APP_B : APP_A, i % 2 ? SAFETY_1 : SAFETY_0, &s) == CE_OK);
		CHECK(no_event());
		frame_start(i % 2 == 0);
		CHECK(next_is(CE_EVENT_PRESENTED, s));
		while (ce_poll_event(engine, &(ce_event){0}) == 1) {
		}
	}
	CHECK(fake.commits < 16); /* went past zero */
	return 0;
}

/* Across zero, a count not yet reached is not taken for reached. */
static int ops_wrap(void)
{
	ce_layer layers[2] = {app_layer(APP_A), safety_layer(SAFETY_1)};
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};
	ce_assignment a[2] = {{.layer = 0, .plane = CE_TWOPLANE_PLANE_APP, .plane_index = 0},
			      {.layer = 1, .plane = CE_TWOPLANE_PLANE_SAFETY, .plane_index = 1}};
	ce_backend_event ev;

	fake.commits = fake.latched = UINT32_MAX;
	CHECK(ce_twoplane_init(&backend, &hw, &port, &fake) == 0);
	/* set, enable, flip: the count goes 0xffffffff -> 2 */
	CHECK(ce_twoplane_ops.commit(&backend, 1, &frame, a, 2) == CE_OK);
	CHECK(backend.pending_count == 2 && fake.latched == UINT32_MAX);
	CHECK(!ce_twoplane_ops.poll_event(&backend, &ev));
	frame_start(false);
	CHECK(ce_twoplane_ops.poll_event(&backend, &ev) && ev.submission == 1);

	/* Half the range behind still counts as not reached. */
	backend.pending = true;
	backend.pending_sub = 2;
	backend.pending_count = 2;
	fake.latched = 2u + 0x80000000u;
	CHECK(!ce_twoplane_ops.poll_event(&backend, &ev));
	fake.latched = 2u + 0x7fffffffu;
	CHECK(ce_twoplane_ops.poll_event(&backend, &ev) && ev.submission == 2);
	return 0;
}

int main(void)
{
	CHECK(CE_ABI_CHECK());
	RUN(ops_init);
	RUN(ops_capabilities);
	RUN(engine_refused_format);
	RUN(engine_frames);
	RUN(engine_rejected);
	RUN(engine_redraw);
	RUN(engine_latch_by_poll);
	RUN(ops_latch_by_vsync);
	RUN(engine_replace_pending);
	RUN(engine_latch_during_writes_isr);
	RUN(engine_latch_during_writes_no_isr);
	RUN(ops_latch_behind_unreported);
	RUN(ops_busy);
	RUN(engine_busy_race);
	RUN(engine_wrap);
	RUN(ops_wrap);
	puts("ok");
	return 0;
}
