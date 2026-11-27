/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * Adversarial event-ordering host test of the two-plane backend
 * (ce_twoplane.c) behind a fake driver, through the engine's C API.
 *
 * Where twoplane_test.c shows the honest orderings, this test attacks them:
 * a vsync with nothing pending; a latch reported once then repeated
 * vsyncs; a latch landing while the replacement's writes are in flight or
 * during the planner's check; a pending submission replaced before its
 * latch and then latched anyway; a latch that never comes (latch timeout);
 * a latch the engine has never been told about (an out-of-band commit); a
 * device loss injected through the backend's poll_event and a stale latch
 * after it; a driver that refuses a write the backend does not check; a
 * rotation of buffers over many frames with idle vsyncs, early polls and
 * replacements thrown in; and a rotation across the 32-bit count wrap.
 *
 * Every scenario asserts the behaviour composition_engine.h and
 * presentation.rs document:
 *  - no buffer is released twice between two uses (tracked by rel_seen,
 *    cleared when the buffer is submitted again);
 *  - nothing is released while still submitted or scanned (every RELEASED
 *    is checked against ce_buffer_state);
 *  - no release before the latch or drop it follows (order checked in the
 *    event log);
 *  - inconsistencies from the backend come back as a negative status from
 *    ce_poll_event (CE_ERR_UNEXPECTED_LATCH), never as a crash, and the
 *    engine stays usable afterwards;
 *  - every submission is reported exactly once, as PRESENTED or DROPPED.
 *
 * Run with no arguments: every scenario not marked known_bug is run.
 * Run with a scenario name: only that scenario runs (used by the Rust
 * harness for the known-bug scenarios, kept #[ignore] there).
 * Exit status 0 means every check passed.
 */
#include "ce_twoplane.h"
#include "composition_engine.h"

#include <stdio.h>
#include <string.h>

/*
 * CHECK collects failures instead of unwinding: a broken ordering usually
 * distorts everything after it, and the first failure printed is the one
 * that matters. The scenario's result is the failure count.
 */
static int errors;

#define CHECK(cond) \
	do { \
		if (!(cond)) { \
			fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__, __LINE__, #cond); \
			errors++; \
		} \
	} while (0)

enum { WIDTH = 800, HEIGHT = 480 };

/* Buffer ids. All stay below 16 (the release tracker's table). */
enum {
	APP_A = 1,
	APP_B = 2,
	APP_C = 3,
	SAFETY_0 = 10, /* the driver's first safety buffer */
	SAFETY_1 = 11, /* the driver's second safety buffer */
};

/* ---- The fake driver ----
 *
 * As in twoplane_test.c: every accepted write is one commit; a frame start
 * copies the shadow registers to the active ones (unless hold) and makes
 * the latched count equal the commit count, then runs the vsync callback
 * unless it was masked by the port's interrupt lock. The test can place a
 * frame start after the n-th write, at the n-th app_format_ok call (the
 * planner's and the commit's own check), at the n-th latched_count or
 * commit_count read, with or without running the interrupt. A driver that
 * refuses enable or flip models the calls whose failure ce_twoplane.c
 * does not check.
 */

struct regs {
	struct ce_twoplane_buf app;
	bool app_on;
	unsigned safety; /* index into fake.safety_mem */
};

static struct fake {
	struct regs shadow;
	struct regs active;
	uint32_t commits; /* commit_count */
	uint32_t latched; /* latched_count */
	bool hold;        /* frame starts latch nothing */
	unsigned frame_starts;
	/* calls */
	unsigned writes;
	unsigned set_app_calls;
	unsigned enable_on_calls;
	unsigned enable_off_calls;
	unsigned flip_calls;
	unsigned premultiplied_calls;
	unsigned format_ok_calls;
	int refuse_enable; /* nonzero: enable_app_plane refuses, writes nothing */
	int refuse_flip;   /* nonzero: safety_flip refuses, writes nothing */
	unsigned wrong_dev;
	ce_twoplane_vsync_cb_t cb;
	void *cb_user;
	/* scripted frame starts, counted from now */
	unsigned start_after_write;
	unsigned start_at_format_ok;
	unsigned start_at_latched;
	unsigned start_at_commit_count;
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

	fake.format_ok_calls++;
	if (fake.start_at_format_ok > 0 && --fake.start_at_format_ok == 0) {
		frame_start(fake.start_runs_isr);
	}
	return i >= 0;
}

static int hw_set_app_buffer(const void *dev, const struct ce_twoplane_buf *buf)
{
	check_dev(dev);
	fake.set_app_calls++;
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
	if (fake.refuse_enable != 0) {
		return fake.refuse_enable; /* refused: shadow unchanged, not a commit */
	}
	fake.shadow.app_on = enable;
	wrote();
	return 0;
}

static int hw_safety_flip(const void *dev)
{
	check_dev(dev);
	fake.flip_calls++;
	if (fake.refuse_flip != 0) {
		return fake.refuse_flip; /* refused: shadow unchanged, not a commit */
	}
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
	(void)on;
	fake.premultiplied_calls++;
	return 0;
}

static uint32_t hw_commit_count(const void *dev)
{
	check_dev(dev);
	if (fake.start_at_commit_count > 0 && --fake.start_at_commit_count == 0) {
		frame_start(fake.start_runs_isr);
	}
	return fake.commits;
}

static uint32_t hw_latched_count(const void *dev)
{
	check_dev(dev);
	if (fake.start_at_latched > 0 && --fake.start_at_latched == 0) {
		frame_start(fake.start_runs_isr);
	}
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
static uint32_t app_mem[3];

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

/* ---- Event recording and the engine-wide invariants ---- */

#define LOG_MAX 1024

struct logent {
	uint8_t kind;
	uint64_t submission;
	uint32_t buffer;
};

static struct logent lg[LOG_MAX];
static size_t lg_len;

/* Submissions the test recorded, with their buffers and how they were
 * reported. Used to check PRESENTED/DROPPED counts and to verify that the
 * buffers of a presented submission really are on screen. */
#define SUB_MAX 128

struct subent {
	uint64_t sub;
	uint32_t buffers[2];
	size_t n;
	unsigned presented;
	unsigned dropped;
};

static struct subent subs[SUB_MAX];
static size_t subs_len;

/* One release per buffer between two uses of it. Cleared when the test
 * records a submission using the buffer again. */
static bool rel_seen[16];

static void note_sub(uint64_t s, uint32_t b0, uint32_t b1, size_t n)
{
	CHECK(subs_len < SUB_MAX);
	if (subs_len >= SUB_MAX) {
		return;
	}
	subs[subs_len++] = (struct subent){.sub = s, .buffers = {b0, b1}, .n = n};
	if (b0 < 16) {
		rel_seen[b0] = false;
	}
	if (b1 < 16) {
		rel_seen[b1] = false;
	}
}

static struct subent *sub_of(uint64_t s)
{
	for (size_t i = 0; i < subs_len; i++) {
		if (subs[i].sub == s) {
			return &subs[i];
		}
	}
	return NULL;
}

static void log_push(const ce_event *e)
{
	CHECK(lg_len < LOG_MAX);
	if (lg_len < LOG_MAX) {
		lg[lg_len++] = (struct logent){
			.kind = e->kind, .submission = e->submission, .buffer = e->buffer};
	}
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
static int start_engine_with(const ce_backend_ops *ops)
{
	static const struct {
		uint32_t id;
		uint32_t format;
	} buffers[] = {
		{APP_A, CE_FORMAT_RGB565},
		{APP_B, CE_FORMAT_RGB565},
		{APP_C, CE_FORMAT_RGB565},
		{SAFETY_0, CE_FORMAT_ARGB8888},
		{SAFETY_1, CE_FORMAT_ARGB8888},
	};

	CHECK(ce_twoplane_init(&backend, &hw, &port, &fake) == 0);
	engine = ce_engine_init(storage, sizeof(storage), ops, &backend);
	CHECK(engine != NULL);
	for (size_t i = 0; i < sizeof(buffers) / sizeof(buffers[0]); i++) {
		const ce_buffer_info info = {CE_MODIFIER_LINEAR, WIDTH, HEIGHT, buffers[i].format};

		CHECK(ce_register_buffer(engine, buffers[i].id, &info) == CE_OK);
		CHECK(ce_finish_write(engine, buffers[i].id) == CE_OK);
	}
	return 0;
}

static int start_engine(void)
{
	return start_engine_with(&ce_twoplane_ops);
}

/*
 * One ce_poll_event, recorded and checked against the engine-wide rules
 * named at the top of this file: a buffer is never released while still
 * submitted or scanned, never twice between two uses, and every PRESENTED
 * names a submission the test submitted, whose buffers are then on
 * screen. The raw ce_poll_event result is returned; only scenarios that
 * expect a negative status accept one.
 */
static int32_t poll_one(ce_event *e)
{
	int32_t rc = ce_poll_event(engine, e);

	if (rc != 1) {
		return rc;
	}
	log_push(e);
	switch (e->kind) {
	case CE_EVENT_RELEASED: {
		uint32_t b = e->buffer;

		CHECK(b < 16);
		CHECK(b < 16 && ce_buffer_state(engine, b) == CE_BUFFER_READY);
		CHECK(b < 16 && !rel_seen[b]);
		if (b < 16) {
			rel_seen[b] = true;
		}
		break;
	}
	case CE_EVENT_PRESENTED: {
		struct subent *s = sub_of(e->submission);

		CHECK(s != NULL);
		if (s != NULL) {
			s->presented++;
			for (size_t i = 0; i < s->n; i++) {
				CHECK(ce_buffer_state(engine, s->buffers[i]) == CE_BUFFER_SCANNING);
			}
		}
		break;
	}
	case CE_EVENT_DROPPED: {
		struct subent *s = sub_of(e->submission);

		CHECK(s != NULL);
		if (s != NULL) {
			s->dropped++;
		}
		break;
	}
	default:
		break;
	}
	return rc;
}

/* The next event is kind for submission. */
static bool next_is(uint8_t kind, uint64_t submission)
{
	ce_event e;
	int32_t rc = poll_one(&e);

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

/* The next event is a release of this buffer. */
static bool next_rel(uint32_t buffer)
{
	ce_event e;
	int32_t rc = poll_one(&e);

	if (rc != 1) {
		fprintf(stderr, "  expected a release of %u, ce_poll_event gave %d\n", buffer, rc);
		return false;
	}
	if (e.kind != CE_EVENT_RELEASED || e.buffer != buffer) {
		fprintf(stderr, "  expected a release of %u, got kind %u buffer %u\n", buffer, e.kind,
			e.buffer);
		return false;
	}
	return true;
}

/* `releases` releases, in any order, and no other event. */
static bool only_releases(unsigned releases)
{
	ce_event e;

	for (unsigned i = 0; i < releases; i++) {
		if (poll_one(&e) != 1 || e.kind != CE_EVENT_RELEASED) {
			fprintf(stderr, "  expected a release\n");
			return false;
		}
	}
	return poll_one(&e) == 0;
}

/* Presented, then `releases` releases and nothing else. */
static bool presented(uint64_t submission, unsigned releases)
{
	return next_is(CE_EVENT_PRESENTED, submission) && only_releases(releases);
}

static bool no_event(void)
{
	ce_event e;
	int32_t rc = poll_one(&e);

	if (rc != 0) {
		fprintf(stderr, "  expected no event, got %d (kind %u, submission %llu)\n", rc,
			e.kind, (unsigned long long)e.submission);
	}
	return rc == 0;
}

/* Drain every queued event. */
static void drain_all(void)
{
	ce_event e;

	while (poll_one(&e) == 1) {
	}
}

/* The first log index of a kind for a submission, or -1. */
static long long log_find_submission(uint8_t kind, uint64_t submission)
{
	for (size_t i = 0; i < lg_len; i++) {
		if (lg[i].kind == kind && lg[i].submission == submission) {
			return (long long)i;
		}
	}
	return -1;
}

/* The first log index of a release of this buffer, or -1. */
static long long log_find_release(uint32_t buffer)
{
	for (size_t i = 0; i < lg_len; i++) {
		if (lg[i].kind == CE_EVENT_RELEASED && lg[i].buffer == buffer) {
			return (long long)i;
		}
	}
	return -1;
}

/* A batch the latch of `submission` must produce: PRESENTED first, then
 * exactly the releases listed, in any order, then nothing. */
static void expect_batch(uint64_t submission, const uint32_t *rels, size_t nrel)
{
	ce_event e;

	CHECK(poll_one(&e) == 1);
	CHECK(e.kind == CE_EVENT_PRESENTED && e.submission == submission);
	uint32_t left[2] = {0, 0};

	for (size_t i = 0; i < nrel && i < 2; i++) {
		left[i] = rels[i];
	}
	for (size_t r = 0; r < nrel; r++) {
		CHECK(poll_one(&e) == 1);
		CHECK(e.kind == CE_EVENT_RELEASED);
		bool hit = false;
		for (size_t i = 0; i < nrel && i < 2; i++) {
			if (left[i] != 0 && left[i] == e.buffer) {
				left[i] = 0;
				hit = true;
				break;
			}
		}
		CHECK(hit);
	}
	CHECK(poll_one(&e) == 0);
}

/* present2 also records the submission's buffers for the checks. */
static ce_status present2(uint32_t app, uint32_t safety, uint64_t *submission)
{
	ce_layer layers[2] = {safety_layer(safety), app_layer(app)};
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};

	ce_status st = ce_present(engine, &frame, submission, NULL);

	if (st == CE_OK && submission != NULL) {
		note_sub(*submission, safety, app, 2);
	}
	return st;
}

/* A frame for calling the backend directly, out of the engine's band. */
static ce_layer od_layers[2];
static ce_assignment od_assign[2];
static ce_frame od_frame;

static void out_of_band_frame(uint32_t app, uint32_t safety)
{
	od_layers[0] = app_layer(app);
	od_layers[1] = safety_layer(safety);
	od_assign[0] = (ce_assignment){
		.layer = 0, .plane = CE_TWOPLANE_PLANE_APP, .plane_index = 0, .zpos = 0};
	od_assign[1] = (ce_assignment){
		.layer = 1, .plane = CE_TWOPLANE_PLANE_SAFETY, .plane_index = 1, .zpos = 1};
	od_frame = (ce_frame){.layers = od_layers, .layer_count = 2, .output = 0};
}

/* ---- A poll_event wrapper that can inject backend events the
 * two-plane backend does not produce itself: device loss and a capability
 * change. Everything else stays the real backend's poll_event. ---- */

static bool lost_next;
static bool caps_next;
static ce_backend_ops wrapped_ops; /* ce_twoplane_ops with poll_event replaced */

static bool poll_event_injected(void *ctx, ce_backend_event *out)
{
	if (lost_next) {
		lost_next = false;
		*out = (ce_backend_event){.submission = 0, .output = 0, .kind = CE_BACKEND_DEVICE_LOST};
		return true;
	}
	if (caps_next) {
		caps_next = false;
		*out = (ce_backend_event){.submission = 0, .output = 0, .kind = CE_BACKEND_CAPABILITIES_CHANGED};
		return true;
	}
	return ce_twoplane_ops.poll_event(ctx, out);
}

static void use_wrapped_ops(void)
{
	wrapped_ops = ce_twoplane_ops;
	wrapped_ops.poll_event = poll_event_injected;
}

/* ---- A reset between scenarios ---- */

static void reset(void)
{
	memset(&fake, 0, sizeof(fake));
	fake.ready = true;
	wrong_port_arg = 0;
	table_len = 0;
	app_mem[0] = 0x1000u; /* made-up physical addresses; the fake reads none */
	app_mem[1] = 0x2000u;
	app_mem[2] = 0x3000u;
	add_record(APP_A, 1, (uintptr_t)&app_mem[0], CE_FORMAT_RGB565);
	add_record(APP_B, 1, (uintptr_t)&app_mem[1], CE_FORMAT_RGB565);
	add_record(APP_C, 1, (uintptr_t)&app_mem[2], CE_FORMAT_RGB565);
	add_record(SAFETY_0, 0, (uintptr_t)fake.safety_mem[0], CE_FORMAT_ARGB8888);
	add_record(SAFETY_1, 0, (uintptr_t)fake.safety_mem[1], CE_FORMAT_ARGB8888);
	lg_len = 0;
	subs_len = 0;
	memset(rel_seen, 0, sizeof(rel_seen));
	lost_next = false;
	caps_next = false;
}

/* ---- Scenarios ---- */

/*
 * vsync with nothing pending, before any commit, between a commit and its
 * latch, and after everything was reported: wake and vsyncs only, never an
 * event, never a latch the hardware did not reach.
 */
static int adv_vsync_idle(void)
{
	uint64_t s1, s2;

	CHECK(start_engine() == 0);
	for (unsigned i = 0; i < 5; i++) {
		frame_start(true);
	}
	CHECK(fake.wakes == 5);
	CHECK(backend.vsyncs == 5);
	for (unsigned i = 0; i < 5; i++) {
		CHECK(no_event());
	}

	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	frame_start(true);
	CHECK(presented(s1, 0));
	/* two more vsyncs after the report: nothing pending, nothing reported */
	frame_start(true);
	frame_start(false);
	CHECK(no_event());

	/* A new frame with nothing in the backend's slots: the replacement
	 * releases only when its own latch arrives.
	 */
	CHECK(present2(APP_B, SAFETY_1, &s2) == CE_OK);
	CHECK(no_event());
	frame_start(true);
	CHECK(presented(s2, 2)); /* APP_A and SAFETY_0 released */
	/* backend.vsyncs counts the callback's runs, not the frame starts: the
	 * one frame raised without an interrupt woke nobody and was counted by
	 * neither.
	 */
	CHECK(fake.wakes == 8 && fake.isr_runs == 8);
	CHECK(backend.vsyncs == fake.isr_runs);
	CHECK(fake.frame_starts == 9);
	drain_all();

	ce_backend_event bev;
	CHECK(!ce_twoplane_ops.poll_event(&backend, &bev));
	return 0;
}

/*
 * One commit, four vsyncs: the latch is noted once, reported once, and the
 * extra vsyncs change nothing. Reporting the same latch twice, or latching
 * the same commit twice, is what the counters must make impossible.
 */
static int adv_duplicate_latch(void)
{
	uint64_t s1, s2;

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	for (unsigned i = 0; i < 4; i++) {
		frame_start(true);
	}
	CHECK(presented(s1, 0));
	CHECK(no_event());
	CHECK(backend.vsyncs == 4 && fake.wakes == 4);

	ce_backend_event bev;
	CHECK(!ce_twoplane_ops.poll_event(&backend, &bev));
	CHECK(!ce_twoplane_ops.poll_event(&backend, &bev));

	/* The same buffers named again with nothing written: the new commit
	 * latches at once and is reported exactly once.
	 */
	CHECK(present2(APP_A, SAFETY_0, &s2) == CE_OK);
	CHECK(presented(s2, 0)); /* both buffers kept: none released */
	frame_start(true);
	frame_start(true);
	CHECK(no_event());
	CHECK(backend.latched == false && backend.pending == false);
	return 0;
}

/*
 * The hardware latches between two presents, and the engine is not polled
 * in between: the replacement's commit must collect that latch first,
 * report the first submission as PRESENTED, and never as DROPPED.
 */
static int adv_latch_between_presents(void)
{
	uint64_t s1, s2;

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	frame_start(true); /* latches s1; nothing polled yet */

	/* The engine drains the backend's latch before its own commit: the
	 * commit is accepted, not refused as busy.
	 */
	CHECK(present2(APP_B, SAFETY_1, &s2) == CE_OK);
	CHECK(presented(s1, 0)); /* presented, not dropped */
	CHECK(log_find_submission(CE_EVENT_DROPPED, s1) == -1);
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_SCANNING);
	CHECK(backend.pending && backend.pending_sub == s2);

	frame_start(true);
	CHECK(presented(s2, 2));
	/* no release before the latch that causes it: APP_A's release comes
	 * after PRESENTED s2, and nothing was released while s1 was shown
	 */
	long long rel = log_find_release(APP_A);
	long long latch = log_find_submission(CE_EVENT_PRESENTED, s2);

	CHECK(rel >= 0 && latch >= 0 && rel > latch);
	drain_all();
	return 0;
}

/*
 * The vsync lands during the planner's check of the replacement, before
 * the backend's commit: the latch is noted, collected by the engine's
 * commit, and the replacement is written normally (no busy, no lost
 * submission).
 */
static int adv_latch_during_plan_check(void)
{
	uint64_t s1, s2;

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);

	/* The frame start fires at the first app_format_ok call: the
	 * planner's check of the new frame.
	 */
	fake.start_at_format_ok = 1;
	fake.start_runs_isr = true;
	CHECK(present2(APP_B, SAFETY_1, &s2) == CE_OK);
	CHECK(fake.frame_starts == 1 && fake.isr_runs == 1);
	CHECK(presented(s1, 0)); /* the latched frame is reported, not dropped */
	CHECK(log_find_submission(CE_EVENT_DROPPED, s1) == -1);
	CHECK(backend.pending && backend.pending_sub == s2);

	frame_start(true);
	CHECK(presented(s2, 2));
	drain_all();
	return 0;
}

/*
 * A pending submission replaced before its latch: DROPPED, its buffers
 * released for the drop (before any latch, which is documented), the
 * replacement latched later, and the dropped submission never reported as
 * presented.
 */
static int adv_replace_pending(void)
{
	uint64_t s1, s2, s3;

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	/* Replaced before its latch. SAFETY_0 stays in the new frame, APP_A
	 * does not.
	 */
	CHECK(present2(APP_B, SAFETY_0, &s2) == CE_OK);
	CHECK(next_is(CE_EVENT_DROPPED, s1));
	CHECK(next_rel(APP_A)); /* dropped before any latch: released for the drop */
	CHECK(no_event());
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_READY);
	CHECK(ce_buffer_state(engine, SAFETY_0) == CE_BUFFER_SUBMITTED);
	CHECK(log_find_submission(CE_EVENT_PRESENTED, s1) == -1);
	/* the drop comes before the release it causes */
	CHECK(log_find_submission(CE_EVENT_DROPPED, s1) < log_find_release(APP_A));

	frame_start(true); /* s2's writes (and s1's, subsumed) reach the panel */
	CHECK(presented(s2, 0)); /* APP_B latched, SAFETY_0 kept: none released */
	frame_start(true);
	frame_start(true);
	CHECK(no_event());

	/* A third frame, replacing nothing (s2 is already on screen): the
	 * latches release the old frame's buffers after its replacement's
	 * latch, never before.
	 */
	CHECK(present2(APP_C, SAFETY_1, &s3) == CE_OK);
	CHECK(no_event());
	frame_start(true);
	CHECK(presented(s3, 2)); /* APP_B and SAFETY_0 released after PRESENTED s3 */
	CHECK(log_find_release(APP_B) > log_find_submission(CE_EVENT_PRESENTED, s3));

	/* A released buffer goes to the producer; showing a buffer that is
	 * being drawn must fail without touching the driver (no use-after-
	 * release hazard: while WRITABLE it can never be put on a plane).
	 */
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_READY);
	CHECK(ce_begin_write(engine, APP_A) == CE_OK);
	unsigned writes = fake.writes;
	uint64_t s4;

	CHECK(present2(APP_A, SAFETY_1, &s4) == CE_ERR_BUFFER_NOT_READY);
	CHECK(fake.writes == writes);
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_WRITABLE);
	CHECK(no_event());
	/* An idle buffer can be taken out of the table; an in-use one cannot.
	 */
	CHECK(ce_unregister_buffer(engine, SAFETY_1) == CE_ERR_BUFFER_IN_USE);
	CHECK(ce_unregister_buffer(engine, APP_A) == CE_OK);
	return 0;
}

/*
 * The pending submission latches while the replacement's writes are in
 * flight (the vsync lands after the replacement's first write): the first
 * frame was accepted, the hardware took it mid-write, and it must be
 * reported PRESENTED, never DROPPED; its buffers stay protected until the
 * replacement itself latches. (As twoplane_test.c already records, the
 * panel then shows the replacement's first write; the documented contract
 * is about the reported latch, not the pixels.)
 */
static int adv_replace_latch_during_writes(void)
{
	uint64_t s1, s2;

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);

	fake.start_after_write = 1;
	fake.start_runs_isr = true;
	CHECK(present2(APP_B, SAFETY_1, &s2) == CE_OK);
	CHECK(fake.frame_starts == 1 && fake.isr_runs == 1);

	CHECK(presented(s1, 0));
	CHECK(log_find_submission(CE_EVENT_DROPPED, s1) == -1);
	CHECK(backend.pending && backend.pending_sub == s2);
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_SCANNING);
	/* Nothing was released while s1 was reported on screen. */
	CHECK(log_find_release(APP_A) == -1);
	CHECK(log_find_release(SAFETY_0) == -1);

	frame_start(true);
	CHECK(presented(s2, 2)); /* APP_A and SAFETY_0 released now */
	CHECK(log_find_release(APP_A) > log_find_submission(CE_EVENT_PRESENTED, s2));
	CHECK(fake.active.app.pa == (uintptr_t)&app_mem[1]);
	drain_all();
	return 0;
}

/*
 * A vsync is masked by the port's lock while the commit reads
 * commit_count, and runs at the unlock: the new submission's own latch is
 * then already there when commit returns, is reported as PRESENTED (never
 * DROPPED), and the releases of the frame it replaces come after it.
 */
static int adv_own_latch_deferred_isr(void)
{
	uint64_t s1, s2;

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	frame_start(true);
	CHECK(presented(s1, 0));
	CHECK(no_event());

	/* The frame start fires at the commit's commit_count read, which the
	 * backend takes under the interrupt lock: the vsync callback runs at
	 * the unlock, right after the commit is complete.
	 */
	fake.start_at_commit_count = 1;
	fake.start_runs_isr = true;
	CHECK(present2(APP_B, SAFETY_1, &s2) == CE_OK);
	CHECK(fake.isr_deferred == false); /* it ran at the unlock */
	CHECK(fake.isr_runs == 2);
	CHECK(backend.latched == false && backend.pending == false);

	/* s2 latched as its own commit finished: recorded after the submit,
	 * reported once.
	 */
	CHECK(presented(s2, 2)); /* APP_A, SAFETY_0 released after PRESENTED s2 */
	CHECK(log_find_submission(CE_EVENT_DROPPED, s2) == -1);
	CHECK(log_find_release(APP_A) > log_find_submission(CE_EVENT_PRESENTED, s2));
	frame_start(true);
	CHECK(no_event());
	drain_all();
	return 0;
}

/*
 * A latch that never comes: 20 vsyncs with the hardware holding, polls,
 * the engine's latch timeout reported exactly once, the buffers still
 * protected, a late latch still reported as PRESENTED, and a timeout that
 * ends in a replacement instead (DROPPED, released for the drop, never
 * reported presented later).
 */
static int adv_latch_timeout_cycle(void)
{
	uint64_t s1, s2, s3;

	CHECK(start_engine() == 0);
	CHECK(ce_set_latch_timeout(engine, 10) == CE_OK);
	CHECK(ce_advance_time(engine, 0) == CE_OK);

	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	fake.hold = true;
	for (unsigned i = 0; i < 20; i++) {
		frame_start(true);
	}
	CHECK(fake.wakes == 20 && backend.vsyncs == 20);
	CHECK(no_event());
	CHECK(no_event());

	/* Not yet overdue. */
	CHECK(ce_advance_time(engine, 9) == CE_OK);
	CHECK(no_event());
	/* Overdue: reported once. */
	CHECK(ce_advance_time(engine, 10) == CE_OK);
	CHECK(next_is(CE_EVENT_LATCH_TIMEOUT, s1));
	CHECK(no_event());
	/* Still in use while it may latch. */
	CHECK(ce_begin_write(engine, APP_A) == CE_ERR_BUFFER_IN_USE);
	CHECK(ce_begin_write(engine, SAFETY_0) == CE_ERR_BUFFER_IN_USE);
	CHECK(ce_advance_time(engine, 11) == CE_OK);
	CHECK(no_event());

	/* A late latch is still a latch. */
	fake.hold = false;
	frame_start(true);
	CHECK(presented(s1, 0));
	CHECK(no_event());

	/* The timeout ending in a replacement instead: s2 times out and is
	 * replaced by s3; the dropped frame's buffers are released for the
	 * drop, and s2 is never presented, timeout or no timeout.
	 */
	CHECK(present2(APP_B, SAFETY_1, &s2) == CE_OK);
	fake.hold = true;
	CHECK(ce_advance_time(engine, 21) == CE_OK); /* 21 - 11 >= 10 */
	CHECK(next_is(CE_EVENT_LATCH_TIMEOUT, s2));
	CHECK(no_event());
	CHECK(ce_begin_write(engine, APP_B) == CE_ERR_BUFFER_IN_USE);
	CHECK(ce_advance_time(engine, 30) == CE_OK);
	CHECK(no_event()); /* still only once */

	CHECK(present2(APP_A, SAFETY_0, &s3) == CE_OK);
	/* The engine releases in its own internal order; the set is what is
	 * fixed: APP_B and SAFETY_1 go, APP_A and SAFETY_0 are kept on the
	 * screen by s1 and re-enter the frame.
	 */
	CHECK(next_is(CE_EVENT_DROPPED, s2));
	drain_all();
	CHECK(rel_seen[APP_B] && rel_seen[SAFETY_1]);
	CHECK(!rel_seen[APP_A] && !rel_seen[SAFETY_0]);
	CHECK(no_event());
	CHECK(log_find_submission(CE_EVENT_PRESENTED, s2) == -1);

	fake.hold = false;
	frame_start(true);
	CHECK(presented(s3, 0)); /* APP_A and SAFETY_0 were kept on s1: none released */
	drain_all();
	return 0;
}

/*
 * A commit whose latch the hardware holds, polled repeatedly before it
 * arrives; found from the counts without the vsync callback ever running;
 * reported once; later vsyncs silent.
 */
static int adv_poll_during_pending(void)
{
	uint64_t s1;

	CHECK(start_engine() == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	fake.hold = true;
	CHECK(no_event());
	CHECK(no_event());
	CHECK(no_event());
	fake.hold = false;
	frame_start(false); /* no vsync callback: the latch comes from the counts */
	CHECK(fake.isr_runs == 0);
	CHECK(presented(s1, 0));
	frame_start(true);
	CHECK(no_event());
	frame_start(true);
	CHECK(no_event());
	drain_all();
	return 0;
}

/*
 * A commit made out of the engine's band (an integrator bug, a driver
 * that double-books): its latch matches no submission the engine knows,
 * and the documented answer is a negative status from ce_poll_event, no
 * event, no release, and an engine that still works afterwards. Includes
 * the harder form, where the out-of-band commit replaces a submission the
 * engine is already tracking.
 */
static int adv_unknown_submission_latch(void)
{
	ce_event e;
	uint64_t s1, s2, s3;

	CHECK(start_engine() == 0);

	/* Phase 1: the engine knows nothing; the backend has a pending
	 * submission 999 it was never given.
	 */
	out_of_band_frame(APP_A, SAFETY_0);
	CHECK(ce_twoplane_ops.commit(&backend, 999, &od_frame, od_assign, 2) == CE_OK);
	CHECK(fake.set_app_calls == 1 && fake.enable_on_calls == 1 && fake.flip_calls == 0);
	frame_start(true);
	CHECK(ce_poll_event(engine, &e) == CE_ERR_UNEXPECTED_LATCH);
	CHECK(no_event());
	/* Nothing moved: every buffer is still idle, and the engine keeps
	 * working.
	 */
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_READY);
	CHECK(ce_buffer_state(engine, SAFETY_0) == CE_BUFFER_READY);
	CHECK(present2(APP_B, SAFETY_1, &s1) == CE_OK);
	frame_start(true);
	CHECK(presented(s1, 0));
	drain_all();

	/* Phase 2: the out-of-band commit replaces the engine's own pending
	 * submission s2. s2's latch never comes; s2 is only ever reported by
	 * the replacement's DROPPED.
	 */
	CHECK(present2(APP_A, SAFETY_0, &s2) == CE_OK);
	out_of_band_frame(APP_A, SAFETY_0); /* same buffers: nothing written */
	CHECK(ce_twoplane_ops.commit(&backend, 998, &od_frame, od_assign, 2) == CE_OK);
	frame_start(true); /* the hardware latches 998, not s2 */
	CHECK(ce_poll_event(engine, &e) == CE_ERR_UNEXPECTED_LATCH);
	CHECK(no_event());
	CHECK(log_find_release(APP_A) == -1); /* s2's buffers are not freed early */

	/* The engine recovers by replacing s2 itself; the old frame is
	 * dropped once and its two buffers released exactly once each (the
	 * engine releases in its own internal order).
	 */
	CHECK(present2(APP_C, SAFETY_1, &s3) == CE_OK);
	CHECK(next_is(CE_EVENT_DROPPED, s2));
	drain_all();
	CHECK(rel_seen[APP_A] && rel_seen[SAFETY_0]);
	CHECK(no_event());

	/* s3's latch then replaces the frame s1 had put on screen: APP_B is
	 * released for it (exactly once), SAFETY_1 is kept.
	 */
	frame_start(true);
	CHECK(presented(s3, 1));
	CHECK(rel_seen[APP_B]);
	CHECK(log_find_release(APP_B) > log_find_submission(CE_EVENT_PRESENTED, s3));
	drain_all();
	return 0;
}

/*
 * A device loss injected through the backend's poll_event (the two-plane
 * backend never reports one itself): the engine drops what was pending,
 * releases every buffer exactly once, and reports DEVICE_LOST. The
 * backend, however, still holds its pending submission; its stale latch
 * must come back as CE_ERR_UNEXPECTED_LATCH, must not release anything a
 * second time, and must not wedge the engine.
 */
static int adv_latch_after_device_lost(void)
{
	ce_event e;
	uint64_t s1, s2, s3;

	use_wrapped_ops();
	CHECK(start_engine_with(&wrapped_ops) == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	frame_start(true);
	CHECK(presented(s1, 0));
	CHECK(no_event());

	CHECK(present2(APP_B, SAFETY_1, &s2) == CE_OK); /* pending, not latched */
	CHECK(no_event());

	lost_next = true;
	/* The loss drops s2 and releases every buffer that was in use, then
	 * reports DEVICE_LOST. Order within the releases is the engine's
	 * choice; the sets are not.
	 */
	CHECK(next_is(CE_EVENT_DROPPED, s2));
	drain_all();
	CHECK(rel_seen[APP_A] && rel_seen[APP_B] && rel_seen[SAFETY_0] && rel_seen[SAFETY_1]);
	CHECK(log_find_submission(CE_EVENT_DEVICE_LOST, 0) >= 0);
	CHECK(log_find_submission(CE_EVENT_PRESENTED, s2) == -1);
	/* DEVICE_LOST comes after the drop and its releases. */
	CHECK(log_find_submission(CE_EVENT_DEVICE_LOST, 0) >
	      log_find_submission(CE_EVENT_DROPPED, s2));

	/* The backend still holds s2 as pending; the hardware now latches it. */
	frame_start(true);
	CHECK(backend.pending == false && backend.latched == true);
	CHECK(ce_poll_event(engine, &e) == CE_ERR_UNEXPECTED_LATCH);
	CHECK(no_event()); /* nothing released a second time */
	CHECK(backend.latched == false);

	/* The engine works on. A fresh frame re-programs the planes (the
	 * backend's state was not reset) and is reported once.
	 */
	CHECK(present2(APP_A, SAFETY_0, &s3) == CE_OK);
	frame_start(true);
	CHECK(presented(s3, 0)); /* nothing was current since the loss */
	drain_all();
	return 0;
}

/*
 * Known bug 1 (flip_error_swallowed), asserted against the behaviour a
 * fix must produce — this scenario currently fails and is marked
 * known_bug: the Rust test `flip_error_swallowed` carries #[ignore].
 *
 * ce_twoplane.c discards hw->safety_flip's return value. A driver that
 * refuses the flip (it returns nonzero and, per the driver contract,
 * writes nothing) is recorded as a flip that happened: ce_present reports
 * success, the frame is reported PRESENTED, and the backend's
 * safety_pa no longer matches the driver's plane. The second safety
 * buffer then becomes unreachable: every later frame that names it is
 * refused, because the backend believes the plane already scans it. A
 * refused plane write is a refused commit ("Success means accepted,"
 * composition_engine.h; ce_twoplane.h spells the refusal out for
 * set_app_buffer, and the flip's own int return is discarded) — the
 * error never reaches the engine. The fix must report the refusal and
 * consume nothing; where in the sequence it checks is the fixer's
 * choice.
 */
static int adv_flip_error_swallowed(void)
{
	ce_layer layers[2] = {safety_layer(SAFETY_1), app_layer(APP_A)};
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};

	CHECK(start_engine() == 0);
	fake.refuse_flip = -5;
	ce_status st = ce_present(engine, &frame, NULL, NULL);

	if (st != CE_ERR_BACKEND_REJECTED) {
		fprintf(stderr,
			"BUG flip_error_swallowed: safety_flip refused (%d) but ce_present said %d; "
			"the refused flip left safety_pa at %p while the plane still scans %p\n",
			fake.refuse_flip, st, (void *)backend.safety_pa, (void *)fake.safety_mem[0]);
		return 1;
	}
	/* The expected behaviour, for when the bug is fixed: nothing was
	 * consumed, nothing was reported, every buffer is idle again.
	 */
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_READY);
	CHECK(ce_buffer_state(engine, SAFETY_1) == CE_BUFFER_READY);
	CHECK(backend.safety_pa == (uintptr_t)fake.safety_mem[0]);
	CHECK(no_event());
	return 0;
}

/*
 * Known bug 2 (enable_error_swallowed), same form: ce_twoplane.c discards
 * hw->enable_app_plane's return value. A refused enable is recorded as a
 * plane that is on: the frame is accepted and reported PRESENTED, and
 * later frames never retry the enable, so the panel shows no application
 * plane while the engine says otherwise.
 */
static int adv_enable_error_swallowed(void)
{
	uint64_t s1;

	CHECK(start_engine() == 0);
	fake.refuse_enable = -5;
	ce_status st = present2(APP_A, SAFETY_0, &s1);

	if (st != CE_ERR_BACKEND_REJECTED) {
		fprintf(stderr,
			"BUG enable_error_swallowed: enable_app_plane refused (%d) but ce_present said %d; "
			"backend.app_on=%d while the driver's plane stays off\n", fake.refuse_enable, st,
			(int)backend.app_on);
		return 1;
	}
	CHECK(backend.app_on == false);
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_READY);
	CHECK(ce_buffer_state(engine, SAFETY_0) == CE_BUFFER_READY);
	CHECK(no_event());
	return 0;
}

/*
 * Saturating the engine's event queue from outside: replacements without
 * ever latching, so events pile up (one DROPPED and two releases each).
 * check_submit runs before the backend's commit, so the frame that finds
 * no room must be refused before it reaches the driver
 * (CE_ERR_EVENT_QUEUE_FULL, nothing written), and after draining, every
 * submission so far is accounted for exactly once and the last pending
 * submission still latches and is reported normally.
 */
static int adv_event_queue_full(void)
{
	unsigned ok = 0;
	uint64_t last = 0, s;
	ce_status st = CE_OK;
	unsigned writes_before = 0;

	CHECK(start_engine() == 0);
	while (ok < 100) {
		uint32_t app = (ok % 2) ? APP_A : APP_B;
		uint32_t saf = (ok % 2) ? SAFETY_0 : SAFETY_1;

		writes_before = fake.writes;
		st = present2(app, saf, &s);
		if (st != CE_OK) {
			break;
		}
		last = s;
		ok++;
	}
	CHECK(st == CE_ERR_EVENT_QUEUE_FULL);
	CHECK(ok >= 5 && ok < 100);
	/* The refused submission reached no driver. */
	CHECK(fake.writes == writes_before);
	CHECK(ce_buffer_state(engine, APP_C) == CE_BUFFER_READY); /* never submitted */

	/* Drain the backlog. Nothing was ever latched, so every event is a
	 * DROPPED or a RELEASED, and counts are the invariant (not the
	 * poll-time state: each dropped frame's buffers were legitimately
	 * re-submitted before the backlog was drained, which the engine
	 * allows — see the finding recorded in this scenario's note).
	 */
	uint32_t use_count[16] = {0}, rel_count[16] = {0}, in_last[16] = {0};
	struct subent *p = sub_of(last);

	CHECK(p != NULL && p->n == 2);
	if (p != NULL) {
		for (size_t j = 0; j < p->n; j++) {
			in_last[p->buffers[j]] = 1;
		}
	}
	for (size_t i = 0; i < subs_len; i++) {
		for (size_t j = 0; j < subs[i].n; j++) {
			use_count[subs[i].buffers[j]]++;
		}
	}
	ce_event e;
	int32_t rc;

	while ((rc = ce_poll_event(engine, &e)) == 1) {
		log_push(&e);
		if (e.kind == CE_EVENT_RELEASED) {
			CHECK(e.buffer < 16);
			rel_count[e.buffer]++;
		} else if (e.kind == CE_EVENT_DROPPED) {
			struct subent *s = sub_of(e.submission);

			CHECK(s != NULL);
			if (s != NULL) {
				s->dropped++;
			}
		} else {
			/* No latch ever happened: nothing else may be reported. */
			fprintf(stderr, "unexpected event kind %u in the backlog\n", e.kind);
			errors++;
		}
	}
	CHECK(rc == 0);
	for (size_t i = 0; i < subs_len; i++) {
		if (subs[i].sub == last) {
			CHECK(subs[i].dropped == 0 && subs[i].presented == 0); /* still pending */
		} else {
			CHECK(subs[i].dropped == 1 && subs[i].presented == 0);
		}
	}
	static const uint32_t all[4] = {APP_A, APP_B, SAFETY_0, SAFETY_1};

	for (size_t i = 0; i < 4; i++) {
		CHECK(rel_count[all[i]] == use_count[all[i]] - in_last[all[i]]);
	}
	/* No state was lost: exactly the last submission's two buffers are
	 * still in use; every other buffer is back to READY.
	 */
	if (p != NULL) {
		for (size_t i = 0; i < 4; i++) {
			CHECK(ce_buffer_state(engine, all[i]) ==
			      (in_last[all[i]] ? CE_BUFFER_SUBMITTED : CE_BUFFER_READY));
		}
	}

	/* The last pending submission still latches normally. */
	frame_start(true);
	CHECK(presented(last, 0));
	frame_start(true);
	CHECK(no_event());
	return 0;
}

/*
 * A CE_BACKEND_CAPABILITIES_CHANGED injected between a submit and its
 * latch (the SoW §10 "modeset" ordering): the engine re-asks the backend
 * for its capabilities, reports CAPABILITIES_CHANGED, and must change
 * nothing else — the pending submission keeps its buffers protected, is
 * never presented or dropped by the event, and still latches after it.
 */
static int adv_capabilities_changed(void)
{
	uint64_t s1, s2;

	use_wrapped_ops();
	CHECK(start_engine_with(&wrapped_ops) == 0);
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	CHECK(no_event());

	unsigned formats_before = fake.format_ok_calls;
	caps_next = true;
	CHECK(next_is(CE_EVENT_CAPABILITIES_CHANGED, 0));
	CHECK(no_event());
	/* The engine described the backend again: the three application
	 * formats, asked once per probe.
	 */
	CHECK(fake.format_ok_calls == formats_before + 3);
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_SUBMITTED);
	CHECK(ce_buffer_state(engine, SAFETY_0) == CE_BUFFER_SUBMITTED);
	CHECK(ce_begin_write(engine, APP_A) == CE_ERR_BUFFER_IN_USE);
	CHECK(log_find_submission(CE_EVENT_PRESENTED, s1) == -1);
	CHECK(log_find_submission(CE_EVENT_DROPPED, s1) == -1);
	CHECK(log_find_release(APP_A) == -1);

	/* The submission completes normally, and a later frame works against
	 * the refreshed capabilities.
	 */
	frame_start(true);
	CHECK(presented(s1, 0));
	CHECK(present2(APP_B, SAFETY_1, &s2) == CE_OK);
	frame_start(true);
	CHECK(presented(s2, 2)); /* APP_A and SAFETY_0 released */
	drain_all();
	return 0;
}

/*
 * A rotation of three application buffers and the driver's two safety
 * buffers over 64 frames, with the producer redrawing (begin_write, a new
 * frame number, finish_write) every reused buffer: and adversarial
 * extras thrown in -- every 8th frame carries two idle vsyncs after the
 * real one, every 8th (offset 5) polls while the latch has not come, every
 * 16th (offset 11) replaces the pending frame with a redraw before it
 * latches. The engine-wide checks (poll_one) guard every event; the
 * batches must be exactly PRESENTED plus the releases of the buffers that
 * left the frame; every submission must end reported exactly once.
 */
static int run_rotation(unsigned frames, const uint32_t *apps, unsigned napps, bool twists)
{
	uint32_t cur[2] = {0, 0};
	uint32_t pend[2] = {0, 0};
	size_t cur_n = 0;
	uint64_t pend_sub = 0;
	unsigned isr_starts = 0;

	for (unsigned i = 0; i < frames; i++) {
		uint32_t app = apps[i % napps];
		uint32_t saf = (i % 2) ? SAFETY_1 : SAFETY_0;

		/* The producer redraws every buffer it reuses: it must be free
		 * by now, or this scenario itself is wrong.
		 */
		CHECK(ce_begin_write(engine, app) == CE_OK);
		record_of(app)->seq = i + 1;
		CHECK(ce_finish_write(engine, app) == CE_OK);
		CHECK(ce_begin_write(engine, saf) == CE_OK);
		CHECK(ce_finish_write(engine, saf) == CE_OK);

		uint64_t s;
		CHECK(present2(app, saf, &s) == CE_OK);
		pend[0] = saf;
		pend[1] = app;
		pend_sub = s;
		size_t pend_n = 2;

		if (twists && i % 16 == 11) {
			/* Replace the pending submission with a redraw of the
			 * same frame: DROPPED, and no release (the new pending
			 * frame references the same buffers).
			 */
			record_of(app)->seq = i + 2;
			uint64_t s2;
			CHECK(present2(app, saf, &s2) == CE_OK);
			pend_sub = s2;
			CHECK(next_is(CE_EVENT_DROPPED, s));
			CHECK(no_event());
		}
		if (twists && i % 8 == 5) {
			/* Poll twice before the latch has come. */
			CHECK(no_event());
			CHECK(no_event());
		}

		unsigned nvsync = (twists && i % 8 == 3) ? 3 : 1;
		for (unsigned v = 0; v < nvsync; v++) {
			frame_start(true);
			isr_starts++;
		}

		/* The latch reports pend_sub and releases exactly the previous
		 * frame's buffers that the new frame does not keep.
		 */
		uint32_t rels[2] = {0, 0};
		size_t nrel = 0;
		for (size_t c = 0; c < cur_n; c++) {
			bool kept = false;
			for (size_t p = 0; p < pend_n; p++) {
				if (pend[p] == cur[c]) {
					kept = true;
				}
			}
			if (!kept) {
				CHECK(nrel < 2);
				rels[nrel++] = cur[c];
			}
		}
		expect_batch(pend_sub, rels, nrel);

		cur[0] = pend[0];
		cur[1] = pend[1];
		cur_n = pend_n;
		pend_n = 0;
		pend_sub = 0;
	}

	/* The rotation drove the backend's interrupt every frame and nothing
	 * else misused the port.
	 */
	CHECK(fake.wakes == isr_starts);
	CHECK(backend.vsyncs == fake.frame_starts);

	/* Every submission was reported exactly once, as presented or as
	 * dropped, never both and never neither.
	 */
	for (size_t i = 0; i < subs_len; i++) {
		bool once_presented = subs[i].presented == 1 && subs[i].dropped == 0;
		bool once_dropped = subs[i].dropped == 1 && subs[i].presented == 0;

		CHECK(once_presented || once_dropped);
	}
	return 0;
}

static int adv_rotation_64(void)
{
	static const uint32_t apps[3] = {APP_A, APP_B, APP_C};

	CHECK(start_engine() == 0);
	CHECK(run_rotation(64, apps, 3, true) == 0);
	return 0;
}

/* The same rotation across the 32-bit count wrap: the reached comparison
 * must not mistake a count for unreached or reached on the far side of
 * zero. */
static int adv_wrap_rotation(void)
{
	static const uint32_t apps[2] = {APP_A, APP_B};

	CHECK(start_engine() == 0);
	fake.commits = UINT32_MAX - 5;
	fake.latched = UINT32_MAX - 5;
	CHECK(run_rotation(40, apps, 2, false) == 0);
	CHECK(fake.commits < 100); /* went past zero */
	CHECK(fake.latched == fake.commits);
	return 0;
}

/* ---- The table ----
 *
 * known_bug marks a scenario whose failure is a recorded engine or backend
 * bug: the scenario is skipped when running all, and the Rust harness
 * carries it as an #[ignore] test named after it.
 */

struct scenario {
	const char *name;
	int (*fn)(void);
	bool known_bug;
};

static const struct scenario scenarios[] = {
	{"vsync_idle", adv_vsync_idle, false},
	{"duplicate_latch", adv_duplicate_latch, false},
	{"latch_between_presents", adv_latch_between_presents, false},
	{"latch_during_plan_check", adv_latch_during_plan_check, false},
	{"replace_pending", adv_replace_pending, false},
	{"replace_latch_during_writes", adv_replace_latch_during_writes, false},
	{"own_latch_deferred_isr", adv_own_latch_deferred_isr, false},
	{"latch_timeout_cycle", adv_latch_timeout_cycle, false},
	{"poll_during_pending", adv_poll_during_pending, false},
	{"unknown_submission_latch", adv_unknown_submission_latch, false},
	{"latch_after_device_lost", adv_latch_after_device_lost, false},
	{"event_queue_full", adv_event_queue_full, false},
	{"capabilities_changed", adv_capabilities_changed, false},
	{"rotation_64", adv_rotation_64, false},
	{"wrap_rotation", adv_wrap_rotation, false},
	{"flip_error_swallowed", adv_flip_error_swallowed, true},
	{"enable_error_swallowed", adv_enable_error_swallowed, true},
};

static int run_one(const struct scenario *s)
{
	reset();
	errors = 0;
	int rc = s->fn();

	if (rc != 0 || errors != 0 || port_misused()) {
		fprintf(stderr, "FAILED %s\n", s->name);
		return 1;
	}
	printf("ok %s\n", s->name);
	return 0;
}

int main(int argc, char **argv)
{
	CHECK(CE_ABI_CHECK());
	if (errors != 0) {
		return 1;
	}

	if (argc > 1) {
		for (size_t i = 0; i < sizeof(scenarios) / sizeof(scenarios[0]); i++) {
			if (strcmp(scenarios[i].name, argv[1]) == 0) {
				return run_one(&scenarios[i]);
			}
		}
		fprintf(stderr, "no such scenario: %s\n", argv[1]);
		return 2;
	}

	for (size_t i = 0; i < sizeof(scenarios) / sizeof(scenarios[0]); i++) {
		if (scenarios[i].known_bug) {
			printf("skip %s (known bug)\n", scenarios[i].name);
			continue;
		}
		if (run_one(&scenarios[i]) != 0) {
			return 1;
		}
	}
	puts("ok");
	return 0;
}
