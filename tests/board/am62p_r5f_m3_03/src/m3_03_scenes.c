/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * M3-03 board scenes 9 to 12 (m3_03.h, docs/m3-03-board-test-design.md).
 *
 * DRAFT: run on the host behind the fake panel only. Not built for or run
 * on a board; no hardware result.
 *
 * Buffers, by engine id:
 *
 *   SAFETY_A  the driver's safety buffer that the plane scans at start-up;
 *             drawn clear with the red rectangle on the left
 *   SAFETY_B  the driver's other safety buffer; the rectangle on the right
 *   APP_1     application frame (frame number 1)
 *   APP_2     application frame (frame number 2)
 *
 * The application buffers are drawn again with new colours once the engine
 * has released them, so that a stale or a dropped frame on the panel has a
 * colour of its own in the photograph.
 *
 *   9  producer-stop      the producer stops (a) between ce_begin_write and
 *                         ce_finish_write, (b) after ce_finish_write with no
 *                         present, (c) after a present, for longer than the
 *                         latch timeout; each followed by a restart
 *   10 flip-sweep         an application commit, then a safety flip presented
 *                         at an offset from the vsync swept over the frame
 *                         period and around the next frame start
 *   11 burst              BURST_N presents between two frame starts, and a
 *                         long burst against the busy and queue-full refusals
 *   12 cache              12a: draw, clean the cache, present; 12b: draw
 *                         without the cache clean (negative control, board
 *                         only: the camera judges it)
 *
 * The firmware judges what it can read: statuses, the driver's commit and
 * latched counts, the engine's events and buffer states, the vsync count.
 * What the panel shows is the photographs' to judge: every hold prints a
 * HOLD line with what the panel should show.
 */

#include "m3_03.h"

#include <composition_engine.h>

/* ---- Parameters ----
 *
 * Every value below is a placeholder TO BE CHOSEN ON THE BOARD (the M3 test
 * plan sets the pass criteria and the numbers; docs/m3-03-board-test-design.md
 * leaves them open). Each can be overridden at compile time with -D.
 */

/* How long a commit may take to be latched and reported. To be chosen on the board. */
#ifndef M3_PRESENT_TIMEOUT_MS
#define M3_PRESENT_TIMEOUT_MS 1000u
#endif
/* Longest wait for one vsync. To be chosen on the board. */
#ifndef M3_VSYNC_TIMEOUT_MS
#define M3_VSYNC_TIMEOUT_MS 200u
#endif
/* Poll period while waiting for an event. To be chosen on the board. */
#ifndef M3_POLL_MS
#define M3_POLL_MS 20u
#endif
/* Frames over which the frame period is measured. To be chosen on the board. */
#ifndef M3_PERIOD_FRAMES
#define M3_PERIOD_FRAMES 8u
#endif
/* Frames the producer stays stopped in 9a and 9b. To be chosen on the board. */
#ifndef M3_STOP_FRAMES
#define M3_STOP_FRAMES 10u
#endif
/* The engine's latch timeout from 9c on, in ms (ticks of now_us() / 1000).
 * To be chosen on the board.
 */
#ifndef M3_LATCH_TIMEOUT_MS
#define M3_LATCH_TIMEOUT_MS 200u
#endif
/* How long the producer stays stopped in 9c, ms; longer than the latch
 * timeout. To be chosen on the board.
 */
#ifndef M3_STOP_C_MS
#define M3_STOP_C_MS 1000u
#endif
/* Scene 10: offsets spread evenly over one frame period. To be chosen on the board. */
#ifndef M3_SWEEP_COARSE_STEPS
#define M3_SWEEP_COARSE_STEPS 8u
#endif
/* Scene 10: fine offsets from one period minus this window to one period
 * plus it, around the next frame start, in us. To be chosen on the board.
 */
#ifndef M3_SWEEP_FINE_WINDOW_US
#define M3_SWEEP_FINE_WINDOW_US 200u
#endif
/* Scene 10: step of the fine offsets, us. To be chosen on the board. */
#ifndef M3_SWEEP_FINE_STEP_US
#define M3_SWEEP_FINE_STEP_US 10u
#endif
/* Scene 10: repetitions of every offset. To be chosen on the board. */
#ifndef M3_SWEEP_REPS
#define M3_SWEEP_REPS 1u
#endif
/* Retries of a present refused with CE_ERR_BACKEND_BUSY (counted, not
 * hidden). To be chosen on the board.
 */
#ifndef M3_BUSY_RETRIES_MAX
#define M3_BUSY_RETRIES_MAX 8u
#endif
/* Scene 11: presents per burst, the burst's frame that flips the safety
 * layer, and the number of bursts. To be chosen on the board.
 */
#ifndef M3_BURST_N
#define M3_BURST_N 3u
#endif
#ifndef M3_BURST_FLIP_AT
#define M3_BURST_FLIP_AT 1u
#endif
#ifndef M3_BURST_REPS
#define M3_BURST_REPS 4u
#endif
/* Scene 11: presents of the long burst. To be chosen on the board. */
#ifndef M3_BURST_LONG_N
#define M3_BURST_LONG_N 64u
#endif
/* Scene 12: the R5F's data cache line, bytes (Cortex-R5: 32; to be read on
 * the board from the configuration it runs).
 */
#ifndef M3_CACHE_LINE_BYTES
#define M3_CACHE_LINE_BYTES 32u
#endif

_Static_assert(M3_STOP_C_MS > M3_LATCH_TIMEOUT_MS, "9c must stop longer than the latch timeout");
_Static_assert(M3_BURST_N >= 2u && M3_BURST_FLIP_AT < M3_BURST_N, "a burst of two or more");
_Static_assert(M3_SWEEP_COARSE_STEPS > 0u && M3_SWEEP_FINE_STEP_US > 0u, "a sweep");
_Static_assert(M3_PERIOD_FRAMES > 0u && M3_BURST_LONG_N > 0u, "at least one");

enum { SAFETY_A = 10, SAFETY_B = 11, APP_1 = 1, APP_2 = 2 };
enum { RECORDS = 4 };
/* Events kept per step: the long burst's ends and releases fit. */
enum { EVLOG_MAX = 4u * M3_BURST_LONG_N + 16u };

static struct ce_twoplane_record table[RECORDS];
static struct ce_twoplane_port port;
static struct ce_twoplane backend;
static _Alignas(CE_ENGINE_ALIGN) unsigned char storage[CE_ENGINE_SIZE];
static ce_engine *engine;

/* What the producer last drew into each application buffer (APP_1, APP_2). */
static uint16_t app_colour[2];
/* The frame last presented and shown: its safety and application buffers. */
static uint32_t on_safety, on_app;

/* The events of the current step, in the order the engine gave them. */
struct ev {
	uint64_t submission;
	uint32_t buffer;
	uint8_t kind;
};
static struct ev evlog[EVLOG_MAX];
static unsigned nev;
static bool ev_overflow, ev_error;

/* Draws whose cache clean returned an error (12a judges its own). */
static unsigned flush_errors;
static int last_flush_rc;

static const struct ce_twoplane_record *lookup(void *arg, uint32_t id)
{
	(void)arg;
	for (size_t i = 0; i < RECORDS; i++) {
		if (table[i].id == id) {
			return &table[i];
		}
	}
	return NULL;
}

static const struct ce_twoplane_record *record(uint32_t id)
{
	return lookup(NULL, id);
}

struct scenes_rect m3_rect(uint32_t width, uint32_t height, int side)
{
	struct scenes_rect r = {
		.width = width / 3,
		.height = height / 2,
		.y = height / 4,
	};

	r.x = side == SCENES_RECT_LEFT ? width / 12 : width - width / 12 - r.width;
	return r;
}

static unsigned app_index(uint32_t id)
{
	return id == APP_1 ? 0u : 1u;
}

static uint32_t other_app(uint32_t id)
{
	return id == APP_1 ? APP_2 : APP_1;
}

static uint32_t other_safety(uint32_t id)
{
	return id == SAFETY_A ? SAFETY_B : SAFETY_A;
}

static int safety_side(uint32_t id)
{
	return id == SAFETY_A ? SCENES_RECT_LEFT : SCENES_RECT_RIGHT;
}

static const char *colour_name(uint16_t c)
{
	switch (c) {
	case SCENES_APP_BLUE:
		return "blue";
	case SCENES_APP_GREEN:
		return "green";
	case M3_APP_YELLOW:
		return "yellow";
	case M3_APP_MAGENTA:
		return "magenta";
	case M3_APP_WHITE:
		return "white";
	default:
		return "other";
	}
}

/* wrap-safe: latched has reached count */
static bool reached(uint32_t latched, uint32_t count)
{
	return (int32_t)(latched - count) >= 0;
}

static uint32_t vsyncs(void)
{
	/* incremented in the driver's interrupt */
	return *(volatile const uint32_t *)&backend.vsyncs;
}

/* ---- Drawing ---- */

static void flush(const struct m3_env *env, void *base, size_t len)
{
	int rc = env->flush(base, len);

	last_flush_rc = rc;
	if (rc != 0) {
		flush_errors++;
		env->log("FLUSH: %u bytes: the cache clean returned %d\n", (unsigned)len, rc);
	}
}

/* Rows [from, to) of an application buffer in one colour; cleaned from the
 * cache (the whole buffer) when clean is true.
 */
static void fill_app(const struct m3_env *env, uint32_t id, uint16_t colour, uint32_t from,
		     uint32_t to, bool clean)
{
	const struct ce_twoplane_record *f = record(id);
	uint8_t *base = (uint8_t *)f->pa;

	for (uint32_t y = from; y < to; y++) {
		uint16_t *row = (uint16_t *)(base + (size_t)y * f->stride_bytes);

		for (uint32_t x = 0; x < f->width; x++) {
			row[x] = colour;
		}
	}
	if (clean) {
		flush(env, base, (size_t)f->height * f->stride_bytes);
	}
}

static void fill_safety(const struct m3_env *env, uint32_t id, int side)
{
	const struct ce_twoplane_record *s = record(id);
	struct scenes_rect r = m3_rect(s->width, s->height, side);
	uint8_t *base = (uint8_t *)s->pa;

	for (uint32_t y = 0; y < s->height; y++) {
		uint32_t *row = (uint32_t *)(base + (size_t)y * s->stride_bytes);

		for (uint32_t x = 0; x < s->width; x++) {
			bool in = x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;

			row[x] = in ? SCENES_SAFETY_RED : SCENES_CLEAR;
		}
	}
	flush(env, base, (size_t)s->height * s->stride_bytes);
}

static bool engine_said(const struct m3_env *env, const char *what, uint32_t id, ce_status rc)
{
	if (rc != CE_OK) {
		env->log("DRAW %u: %s returned %d\n", (unsigned)id, what, (int)rc);
		return false;
	}
	return true;
}

/* The producer draws a whole application buffer and hands it over. */
static bool draw_app(const struct m3_env *env, uint32_t id, uint16_t colour, bool clean)
{
	if (!engine_said(env, "ce_begin_write", id, ce_begin_write(engine, id))) {
		return false;
	}
	fill_app(env, id, colour, 0, record(id)->height, clean);
	app_colour[app_index(id)] = colour;
	return engine_said(env, "ce_finish_write", id, ce_finish_write(engine, id));
}

static bool draw_safety(const struct m3_env *env, uint32_t id)
{
	if (!engine_said(env, "ce_begin_write", id, ce_begin_write(engine, id))) {
		return false;
	}
	fill_safety(env, id, safety_side(id));
	return engine_said(env, "ce_finish_write", id, ce_finish_write(engine, id));
}

/* ---- Frames and events ---- */

static ce_layer full_layer(uint32_t id, bool safety)
{
	const struct ce_twoplane_record *r = record(id);

	return (ce_layer){
		.buffer = id,
		.source = {0, 0, r->width, r->height},
		.destination = {0, 0, backend.width, backend.height},
		.z_order = safety ? 10 : 0,
		.alpha = CE_ALPHA_OPAQUE,
		.blend = safety ? CE_BLEND_PREMULTIPLIED : CE_BLEND_OPAQUE,
		.transform = CE_TRANSFORM_NORMAL,
	};
}

/* Tells the engine the time (for the latch timeout) and takes every event
 * into evlog.
 */
static void poll_all(const struct m3_env *env)
{
	ce_event e;
	int32_t rc;

	if (ce_advance_time(engine, env->now_us() / 1000u) != CE_OK) {
		ev_error = true;
	}
	while ((rc = ce_poll_event(engine, &e)) == 1) {
		if (nev < EVLOG_MAX) {
			evlog[nev++] = (struct ev){e.submission, e.buffer, e.kind};
		} else {
			ev_overflow = true;
		}
	}
	if (rc < 0) {
		ev_error = true;
	}
}

static unsigned count_sub(uint8_t kind, uint64_t sub)
{
	unsigned n = 0;

	for (unsigned i = 0; i < nev; i++) {
		n += evlog[i].kind == kind && evlog[i].submission == sub;
	}
	return n;
}

static unsigned count_kind(uint8_t kind)
{
	unsigned n = 0;

	for (unsigned i = 0; i < nev; i++) {
		n += evlog[i].kind == kind;
	}
	return n;
}

static unsigned count_release(uint32_t buffer)
{
	unsigned n = 0;

	for (unsigned i = 0; i < nev; i++) {
		n += evlog[i].kind == CE_EVENT_RELEASED && evlog[i].buffer == buffer;
	}
	return n;
}

/* Index of the first such event, or nev. */
static unsigned find_sub(uint8_t kind, uint64_t sub)
{
	for (unsigned i = 0; i < nev; i++) {
		if (evlog[i].kind == kind && evlog[i].submission == sub) {
			return i;
		}
	}
	return nev;
}

static unsigned find_release(uint32_t buffer)
{
	for (unsigned i = 0; i < nev; i++) {
		if (evlog[i].kind == CE_EVENT_RELEASED && evlog[i].buffer == buffer) {
			return i;
		}
	}
	return nev;
}

/* Only presented, dropped and released, and the log is complete. */
static bool ordinary_events(void)
{
	if (ev_overflow || ev_error) {
		return false;
	}
	for (unsigned i = 0; i < nev; i++) {
		if (evlog[i].kind != CE_EVENT_PRESENTED && evlog[i].kind != CE_EVENT_DROPPED &&
		    evlog[i].kind != CE_EVENT_RELEASED) {
			return false;
		}
	}
	return true;
}

static void clear_log(void)
{
	nev = 0;
	ev_overflow = false;
	ev_error = false;
}

/* Starts a step with an empty log. The steps before took every event they
 * caused, so anything the engine still has is a stray event.
 */
static bool settle(const struct m3_env *env, unsigned n, const char *name)
{
	clear_log();
	poll_all(env);
	bool ok = nev == 0 && !ev_overflow && !ev_error;

	if (!ok) {
		env->log("SCENE %u %s: %u stray events before the step: FAIL\n", n, name, nev);
	}
	clear_log();
	return ok;
}

/* Polls until sub is presented or dropped, or the time is up. */
static bool wait_end(const struct m3_env *env, uint64_t sub)
{
	uint64_t deadline = env->now_us() + (uint64_t)M3_PRESENT_TIMEOUT_MS * 1000u;

	for (;;) {
		poll_all(env);
		if (count_sub(CE_EVENT_PRESENTED, sub) + count_sub(CE_EVENT_DROPPED, sub) > 0) {
			return true;
		}
		if (ev_error || env->now_us() >= deadline) {
			return false;
		}
		env->wait_wake(M3_POLL_MS);
	}
}

/* Waits until the vsync count moves: the caller runs just after a frame start. */
static bool wait_vsync(const struct m3_env *env)
{
	uint32_t v = vsyncs();
	uint64_t deadline = env->now_us() + (uint64_t)M3_VSYNC_TIMEOUT_MS * 1000u;

	while (vsyncs() == v) {
		if (env->now_us() >= deadline) {
			return false;
		}
		env->wait_wake(M3_POLL_MS);
	}
	return true;
}

/* ce_present, retried after a poll while the backend says busy. */
static ce_status present(const struct m3_env *env, uint32_t safety, uint32_t app, uint64_t *sub,
			 unsigned *retries)
{
	ce_layer l[2] = {full_layer(safety, true), full_layer(app, false)};
	ce_frame frame = {.layers = l, .layer_count = 2, .output = 0};
	ce_status st;

	for (;;) {
		st = ce_present(engine, &frame, sub, NULL);
		if (st != CE_ERR_BACKEND_BUSY || *retries >= M3_BUSY_RETRIES_MAX) {
			return st;
		}
		(*retries)++;
		poll_all(env);
	}
}

/* Presents {safety, app}, waits for it, judges it and logs a SCENE line in the
 * form of scenes.c, plus the busy retries.
 */
static bool frame_step(const struct m3_env *env, unsigned n, const char *name, uint32_t safety,
		       uint32_t app, uint32_t expect_commits, unsigned expect_released)
{
	const struct ce_twoplane_hw *hw = env->hw;
	bool ok = settle(env, n, name);
	uint32_t before = hw->commit_count(env->dev);
	uint64_t sub = 0;
	unsigned retries = 0;
	ce_status st = present(env, safety, app, &sub, &retries);
	uint32_t written = hw->commit_count(env->dev) - before;
	bool presented = false;

	if (st == CE_OK) {
		(void)wait_end(env, sub);
		presented = count_sub(CE_EVENT_PRESENTED, sub) == 1 &&
			    count_sub(CE_EVENT_DROPPED, sub) == 0;
	}
	uint32_t commits = hw->commit_count(env->dev);
	uint32_t latched = hw->latched_count(env->dev);
	unsigned released = count_kind(CE_EVENT_RELEASED);

	ok = ok && st == CE_OK && written == expect_commits && latched == commits && presented &&
	     released == expect_released && ordinary_events() &&
	     count_kind(CE_EVENT_PRESENTED) == 1 && count_kind(CE_EVENT_DROPPED) == 0 &&
	     ce_buffer_state(engine, safety) == CE_BUFFER_SCANNING &&
	     ce_buffer_state(engine, app) == CE_BUFFER_SCANNING;
	env->log("SCENE %u %s: status %d (expected %d), commits %u latched %u "
		 "(+%u, expected +%u), presented %s, released %u (expected %u), "
		 "busy retries %u: %s\n",
		 n, name, (int)st, (int)CE_OK, (unsigned)commits, (unsigned)latched,
		 (unsigned)written, (unsigned)expect_commits,
		 st == CE_OK ? (presented ? "yes" : "NO") : "-", released, expect_released, retries,
		 ok ? "ok" : "FAIL");
	if (st == CE_OK) {
		on_safety = safety;
		on_app = app;
	}
	return ok;
}

/* Holds the frame on the panel for the photograph, announced with a HOLD
 * line; nothing may be written meanwhile.
 */
static bool hold(const struct m3_env *env, unsigned n, const char *name, const char *note)
{
	const struct ce_twoplane_hw *hw = env->hw;
	const struct m3_view view = {
		.name = name,
		.safety_side = safety_side(on_safety),
		.app_on = true,
		.app_colour = app_colour[app_index(on_app)],
		.note = note,
	};
	uint32_t commits = hw->commit_count(env->dev);

	env->log("HOLD %u %s: expect app %s, safety rect %s%s%s\n", n, name,
		 colour_name(view.app_colour),
		 view.safety_side == SCENES_RECT_LEFT ? "left" : "right", note ? "; " : "",
		 note ? note : "");
	env->hold(env->hold_ms, n, &view);
	if (hw->commit_count(env->dev) != commits) {
		env->log("SCENE %u %s: commits changed while held: %u -> %u: FAIL\n", n, name,
			 (unsigned)commits, (unsigned)hw->commit_count(env->dev));
		return false;
	}
	return true;
}

/* The producer is stopped: frames pass (wait_frames of them, or for ms
 * milliseconds when wait_frames is 0) with no call into the engine but
 * ce_poll_event; nothing may be committed or reported.
 */
static bool stopped(const struct m3_env *env, unsigned n, const char *name, uint32_t wait_frames,
		    uint32_t ms, uint32_t watch, int32_t expect_state)
{
	const struct ce_twoplane_hw *hw = env->hw;
	bool timely = settle(env, n, name);
	uint32_t commits = hw->commit_count(env->dev);
	uint32_t v = vsyncs();

	if (wait_frames > 0) {
		for (uint32_t i = 0; i < wait_frames && timely; i++) {
			timely = wait_vsync(env);
			poll_all(env);
		}
	} else {
		uint64_t deadline = env->now_us() + (uint64_t)ms * 1000u;

		while (env->now_us() < deadline) {
			env->wait_wake(M3_POLL_MS);
			poll_all(env);
		}
	}
	uint32_t after = hw->commit_count(env->dev);
	uint32_t latched = hw->latched_count(env->dev);
	int32_t state = ce_buffer_state(engine, watch);
	bool ok = timely && after == commits && latched == after && nev == 0 && !ev_overflow &&
		  !ev_error && state == expect_state;

	env->log("SCENE %u %s: stopped for %u vsyncs, commits %u latched %u (+%u, expected +0), "
		 "events %u (expected 0), buffer %u state %d (expected %d): %s\n",
		 n, name, (unsigned)(vsyncs() - v), (unsigned)after, (unsigned)latched,
		 (unsigned)(after - commits), nev, (unsigned)watch, (int)state, (int)expect_state,
		 ok ? "ok" : "FAIL");
	return ok;
}

/* ---- Setup ---- */

static bool setup(const struct m3_env *env)
{
	const struct ce_twoplane_hw *hw = env->hw;
	uint32_t w, h, stride;

	if (!CE_ABI_CHECK()) {
		env->log("SETUP: the engine's header and library disagree\n");
		return false;
	}
	port = (struct ce_twoplane_port){
		.irq_lock = env->irq_lock,
		.irq_unlock = env->irq_unlock,
		.wake = env->wake,
		.wake_arg = env->wake_arg,
		.lookup = lookup,
		.lookup_arg = NULL,
		.device_ready = env->device_ready,
	};
	int rc = ce_twoplane_init(&backend, hw, &port, env->dev);

	if (rc != 0) {
		env->log("SETUP: ce_twoplane_init returned %d\n", rc);
		return false;
	}
	void *front = hw->safety_front(env->dev);
	void *back = hw->safety_buffer(env->dev, &w, &h, &stride);

	if (front == NULL || back == NULL || front == back || w == 0 || h == 0) {
		env->log("SETUP: the driver's safety buffers are not usable\n");
		return false;
	}
	if ((size_t)w * h * 2u > env->app_size) {
		env->log("SETUP: panel %ux%u needs %u bytes per application buffer, have %u\n",
			 (unsigned)w, (unsigned)h, (unsigned)(w * h * 2u), (unsigned)env->app_size);
		return false;
	}
	table[0] = (struct ce_twoplane_record){SAFETY_A, 0, (uintptr_t)front, w, h, stride,
					       CE_FORMAT_ARGB8888};
	table[1] = (struct ce_twoplane_record){SAFETY_B, 0, (uintptr_t)back, w, h, stride,
					       CE_FORMAT_ARGB8888};
	table[2] = (struct ce_twoplane_record){APP_1, 1, (uintptr_t)env->app_mem[0], w, h, w * 2u,
					       CE_FORMAT_RGB565};
	table[3] = (struct ce_twoplane_record){APP_2, 2, (uintptr_t)env->app_mem[1], w, h, w * 2u,
					       CE_FORMAT_RGB565};

	engine = ce_engine_init(storage, sizeof(storage), &ce_twoplane_ops, &backend);
	if (engine == NULL) {
		env->log("SETUP: ce_engine_init failed\n");
		return false;
	}
	for (size_t i = 0; i < RECORDS; i++) {
		const ce_buffer_info bi = {CE_MODIFIER_LINEAR, table[i].width, table[i].height,
					   table[i].format};

		rc = ce_register_buffer(engine, table[i].id, &bi);
		if (rc != CE_OK) {
			env->log("SETUP: ce_register_buffer(%u) returned %d\n",
				 (unsigned)table[i].id, rc);
			return false;
		}
	}
	env->log("SETUP: panel %ux%u, safety stride %u, commits %u latched %u\n", (unsigned)w,
		 (unsigned)h, (unsigned)stride, (unsigned)hw->commit_count(env->dev),
		 (unsigned)hw->latched_count(env->dev));
	return true;
}

/* ---- Scene 9: the producer stops and restarts ---- */

static unsigned scene9(const struct m3_env *env)
{
	const struct ce_twoplane_hw *hw = env->hw;
	unsigned failed = 0;

	/* Start: A (rectangle left) over APP_1 blue: the buffer and the plane's
	 * enable, no flip (A is on the plane already), nothing released.
	 */
	failed += !draw_safety(env, SAFETY_A);
	failed += !draw_app(env, APP_1, SCENES_APP_BLUE, true);
	failed += !frame_step(env, 9, "producer-stop-start", SAFETY_A, APP_1, 2, 0);

	/* (a) APP_2 begun, its first half drawn magenta, then the producer stops.
	 * A present that names it is refused before the driver is touched.
	 */
	{
		const char *name = "producer-stop-a";
		uint32_t commits = hw->commit_count(env->dev);
		uint64_t sub = 0;
		unsigned retries = 0;

		failed += !settle(env, 9, name);
		failed += !engine_said(env, "ce_begin_write", APP_2, ce_begin_write(engine, APP_2));
		fill_app(env, APP_2, M3_APP_MAGENTA, 0, record(APP_2)->height / 2, true);
		ce_status st = present(env, SAFETY_A, APP_2, &sub, &retries);
		bool ok = st == CE_ERR_BUFFER_NOT_READY && hw->commit_count(env->dev) == commits;

		env->log("SCENE 9 %s: status %d (expected %d), commits %u (+%u, expected +0): %s\n",
			 name, (int)st, (int)CE_ERR_BUFFER_NOT_READY,
			 (unsigned)hw->commit_count(env->dev),
			 (unsigned)(hw->commit_count(env->dev) - commits), ok ? "ok" : "FAIL");
		failed += !ok;
		failed += !stopped(env, 9, name, M3_STOP_FRAMES, 0, APP_2, CE_BUFFER_WRITABLE);
		failed += !hold(env, 9, name, "the magenta half-drawn frame must not appear");
	}
	/* Restart (a): the write completes in green; one commit, APP_1 released. */
	fill_app(env, APP_2, SCENES_APP_GREEN, 0, record(APP_2)->height, true);
	app_colour[app_index(APP_2)] = SCENES_APP_GREEN;
	failed += !engine_said(env, "ce_finish_write", APP_2, ce_finish_write(engine, APP_2));
	failed += !frame_step(env, 9, "producer-stop-a-restart", SAFETY_A, APP_2, 1, 1);
	failed += !hold(env, 9, "producer-stop-a-restart", NULL);

	/* (b) APP_1 drawn yellow and finished, never presented: no commit, no
	 * event; the panel stays green.
	 */
	failed += !settle(env, 9, "producer-stop-b");
	failed += !draw_app(env, APP_1, M3_APP_YELLOW, true);
	failed += !stopped(env, 9, "producer-stop-b", M3_STOP_FRAMES, 0, APP_1, CE_BUFFER_READY);
	failed += !hold(env, 9, "producer-stop-b", "the yellow frame must not appear");

	/* (c) the latch timeout on, APP_1 presented and latched, then nothing for
	 * longer than the timeout: a latched frame is not pending, no event.
	 */
	if (ce_set_latch_timeout(engine, M3_LATCH_TIMEOUT_MS) != CE_OK) {
		env->log("SCENE 9 producer-stop-c: ce_set_latch_timeout refused: FAIL\n");
		failed++;
	}
	failed += !frame_step(env, 9, "producer-stop-c", SAFETY_A, APP_1, 1, 1);
	failed += !stopped(env, 9, "producer-stop-c", 0, M3_STOP_C_MS, APP_1, CE_BUFFER_SCANNING);
	failed += !hold(env, 9, "producer-stop-c", NULL);

	/* Restart (c): both planes change, B (rectangle right) over APP_2 drawn
	 * blue: the buffer and the flip, A and APP_1 released.
	 */
	failed += !draw_safety(env, SAFETY_B);
	failed += !draw_app(env, APP_2, SCENES_APP_BLUE, true);
	failed += !frame_step(env, 9, "producer-stop-c-restart", SAFETY_B, APP_2, 2, 2);
	failed += !hold(env, 9, "producer-stop-c-restart", NULL);
	return failed;
}

/* ---- Scene 10: a safety flip on top of an in-flight application commit ---- */

struct sweep_count {
	unsigned before, mid, dropped, retries, failed;
};

/* One run: F1 = {S, X} (the application buffer only), then at offset_us
 * after the vsync F2 = {S', X} (the safety flip only). Judges one end per
 * submission, the releases and the end state, and sorts the case by what the
 * firmware can tell: the first latched before F2 was called, while F2 was
 * being written (presented, not latched when F2 began), or dropped.
 */
static void sweep_run(const struct m3_env *env, uint32_t offset_us, unsigned rep,
		      struct sweep_count *c)
{
	const struct ce_twoplane_hw *hw = env->hw;
	uint32_t s = on_safety, s2 = other_safety(on_safety);
	uint32_t y = on_app, x = other_app(on_app);
	uint64_t sub1 = 0, sub2 = 0;
	unsigned retries = 0;
	bool ok = settle(env, 10, "flip-sweep") && wait_vsync(env);
	uint64_t t0 = env->now_us();
	uint32_t c0 = hw->commit_count(env->dev);
	ce_status st1 = present(env, s, x, &sub1, &retries);
	uint32_t c1 = hw->commit_count(env->dev);
	uint64_t elapsed = env->now_us() - t0;

	if (offset_us > elapsed) {
		env->delay_us((uint32_t)(offset_us - elapsed));
	}
	uint32_t l0 = hw->latched_count(env->dev);
	ce_status st2 = present(env, s2, x, &sub2, &retries);
	uint32_t c2 = hw->commit_count(env->dev);

	if (st2 == CE_OK) {
		(void)wait_end(env, sub2);
	}
	uint32_t commits = hw->commit_count(env->dev);
	uint32_t latched = hw->latched_count(env->dev);
	unsigned p1 = count_sub(CE_EVENT_PRESENTED, sub1), d1 = count_sub(CE_EVENT_DROPPED, sub1);
	unsigned p2 = count_sub(CE_EVENT_PRESENTED, sub2), d2 = count_sub(CE_EVENT_DROPPED, sub2);
	unsigned released = count_kind(CE_EVENT_RELEASED);

	ok = ok && st1 == CE_OK && st2 == CE_OK && c1 - c0 == 1u && c2 - c1 == 1u &&
	     latched == commits && p1 + d1 == 1u && p2 == 1u && d2 == 0u && ordinary_events() &&
	     released == 2u && count_release(y) == 1u && count_release(s) == 1u &&
	     find_release(s) > find_sub(CE_EVENT_PRESENTED, sub2) &&
	     ce_buffer_state(engine, x) == CE_BUFFER_SCANNING &&
	     ce_buffer_state(engine, s2) == CE_BUFFER_SCANNING &&
	     ce_buffer_state(engine, y) == CE_BUFFER_READY &&
	     ce_buffer_state(engine, s) == CE_BUFFER_READY;
	if (d1) {
		c->dropped++;
	} else if (reached(l0, c1)) {
		c->before++;
	} else {
		c->mid++;
	}
	c->retries += retries;
	if (!ok) {
		c->failed++;
		env->log("SWEEP 10 offset %u us rep %u: status %d/%d, commits +%u/+%u, first "
			 "presented %u dropped %u, second presented %u dropped %u, released %u "
			 "(expected 2), commits %u latched %u: FAIL\n",
			 (unsigned)offset_us, rep, (int)st1, (int)st2, (unsigned)(c1 - c0),
			 (unsigned)(c2 - c1), p1, d1, p2, d2, released, (unsigned)commits,
			 (unsigned)latched);
	}
	if (st2 == CE_OK) {
		on_safety = s2;
		on_app = x;
	}
}

static unsigned scene10(const struct m3_env *env)
{
	const struct ce_twoplane_hw *hw = env->hw;
	const unsigned fine = 2u * M3_SWEEP_FINE_WINDOW_US / M3_SWEEP_FINE_STEP_US;
	struct sweep_count total = {0};
	uint64_t t0;
	uint32_t period;
	bool timely = wait_vsync(env);

	/* The frame period, measured: the offsets are fractions of it. */
	t0 = env->now_us();
	for (uint32_t i = 0; i < M3_PERIOD_FRAMES && timely; i++) {
		timely = wait_vsync(env);
	}
	period = (uint32_t)((env->now_us() - t0) / M3_PERIOD_FRAMES);
	if (!timely || period <= M3_SWEEP_FINE_WINDOW_US) {
		env->log("SCENE 10 flip-sweep: no usable frame period (vsyncs %s, %u us): FAIL\n",
			 timely ? "seen" : "missing", (unsigned)period);
		return 1;
	}
	for (unsigned i = 0; i < M3_SWEEP_COARSE_STEPS + fine; i++) {
		uint32_t offset = i < M3_SWEEP_COARSE_STEPS
					  ? period / M3_SWEEP_COARSE_STEPS * i
					  : period - M3_SWEEP_FINE_WINDOW_US +
						    (i - M3_SWEEP_COARSE_STEPS) * M3_SWEEP_FINE_STEP_US;
		struct sweep_count c = {0};

		for (unsigned rep = 0; rep < M3_SWEEP_REPS; rep++) {
			sweep_run(env, offset, rep, &c);
		}
		env->log("SWEEP 10 offset %u us: before %u mid-write %u dropped %u, busy retries "
			 "%u: %s\n",
			 (unsigned)offset, c.before, c.mid, c.dropped, c.retries,
			 c.failed ? "FAIL" : "ok");
		total.before += c.before;
		total.mid += c.mid;
		total.dropped += c.dropped;
		total.retries += c.retries;
		total.failed += c.failed;
	}
	/* Which cases occurred is a result, not a requirement. */
	env->log("SCENE 10 flip-sweep: period %u us, offsets %u x %u, before %u mid-write %u "
		 "dropped %u, busy retries %u, commits %u latched %u, failed runs %u: %s\n",
		 (unsigned)period, M3_SWEEP_COARSE_STEPS + fine, (unsigned)M3_SWEEP_REPS,
		 total.before, total.mid, total.dropped, total.retries,
		 (unsigned)hw->commit_count(env->dev), (unsigned)hw->latched_count(env->dev),
		 total.failed, total.failed ? "FAIL" : "ok");
	unsigned failed = total.failed != 0;

	failed += !hold(env, 10, "flip-sweep", NULL);
	return failed;
}

/* ---- Scene 11: back-to-back commits within one frame ---- */

static unsigned bit(uint32_t id)
{
	switch (id) {
	case SAFETY_A:
		return 1u;
	case SAFETY_B:
		return 2u;
	case APP_1:
		return 4u;
	default:
		return 8u;
	}
}

static const uint32_t ids[RECORDS] = {SAFETY_A, SAFETY_B, APP_1, APP_2};

/* One burst of M3_BURST_N presents right after a frame start: application
 * buffers X, Y, X, ... (X the one not on the panel), the safety layer
 * flipped from frame M3_BURST_FLIP_AT on. Within one frame every frame but
 * the last is dropped, the last presented, and the releases are those the
 * engine owes: at each drop the dropped frame's buffers that are neither in
 * the new frame nor on the panel; at the last latch the panel's buffers not
 * in the last frame.
 */
static bool burst(const struct m3_env *env, unsigned rep, bool *crossed_out)
{
	const struct ce_twoplane_hw *hw = env->hw;
	uint32_t s = on_safety, y = on_app, x = other_app(on_app);
	uint64_t sub[M3_BURST_N];
	uint32_t safety[M3_BURST_N], app[M3_BURST_N];
	unsigned owed[RECORDS] = {0};
	unsigned retries = 0, writes = 0, accepted = 0;
	unsigned scanning = bit(s) | bit(y), pending = 0;
	uint32_t prev_safety = s, prev_app = y;
	bool ok = settle(env, 11, "burst") && wait_vsync(env);
	uint32_t c0 = hw->commit_count(env->dev);
	uint32_t v0 = vsyncs();

	for (unsigned k = 0; k < M3_BURST_N; k++) {
		safety[k] = k >= M3_BURST_FLIP_AT ? other_safety(s) : s;
		app[k] = k % 2u == 0 ? x : y;
		if (present(env, safety[k], app[k], &sub[k], &retries) == CE_OK) {
			accepted++;
		}
	}
	bool crossed = vsyncs() != v0;
	uint32_t c1 = hw->commit_count(env->dev);

	for (unsigned k = 0; k < M3_BURST_N; k++) {
		unsigned f = bit(safety[k]) | bit(app[k]);

		writes += (app[k] != prev_app) + (safety[k] != prev_safety);
		prev_app = app[k];
		prev_safety = safety[k];
		if (pending) {
			unsigned r = pending & ~f & ~scanning;

			for (unsigned i = 0; i < RECORDS; i++) {
				owed[i] += (r & bit(ids[i])) != 0;
			}
		}
		pending = f;
	}
	for (unsigned i = 0; i < RECORDS; i++) {
		owed[i] += (scanning & ~pending & bit(ids[i])) != 0;
	}
	ok = ok && accepted == M3_BURST_N && wait_end(env, sub[M3_BURST_N - 1]);

	unsigned dropped = count_kind(CE_EVENT_DROPPED);
	unsigned presented = count_kind(CE_EVENT_PRESENTED);
	unsigned released = count_kind(CE_EVENT_RELEASED);
	unsigned owed_total = 0;
	uint32_t commits = hw->commit_count(env->dev);
	uint32_t latched = hw->latched_count(env->dev);

	ok = ok && ordinary_events() && c1 - c0 == writes && latched == commits &&
	     count_sub(CE_EVENT_PRESENTED, sub[M3_BURST_N - 1]) == 1u &&
	     ce_buffer_state(engine, safety[M3_BURST_N - 1]) == CE_BUFFER_SCANNING &&
	     ce_buffer_state(engine, app[M3_BURST_N - 1]) == CE_BUFFER_SCANNING;
	for (unsigned k = 0; k < M3_BURST_N; k++) {
		ok = ok && count_sub(CE_EVENT_PRESENTED, sub[k]) + count_sub(CE_EVENT_DROPPED, sub[k]) ==
				   1u;
	}
	for (unsigned i = 0; i < RECORDS; i++) {
		owed_total += owed[i];
	}
	/* Within one frame the counts are exact; a burst that crossed a frame
	 * start is another case, judged by one end per submission only.
	 */
	if (!crossed) {
		ok = ok && dropped == M3_BURST_N - 1u && presented == 1u && released == owed_total;
		for (unsigned i = 0; i < RECORDS; i++) {
			ok = ok && count_release(ids[i]) == owed[i];
		}
	}
	env->log("BURST 11 rep %u: presents %u, crossed a frame start %s, dropped %u presented "
		 "%u, released %u (expected %u), commits +%u (expected +%u), busy retries %u: "
		 "%s\n",
		 rep, (unsigned)M3_BURST_N, crossed ? "yes" : "no", dropped, presented, released,
		 crossed ? released : owed_total, (unsigned)(c1 - c0), writes, retries,
		 ok ? "ok" : "FAIL");
	if (accepted == M3_BURST_N) {
		on_safety = safety[M3_BURST_N - 1];
		on_app = app[M3_BURST_N - 1];
	}
	*crossed_out = crossed;
	return ok;
}

/* A long burst against the backend's busy refusal and the engine's event
 * queue, polled only when refused and every 16 presents. Every status must
 * be one the caller can act on, every accepted submission end once and the
 * last accepted be presented. The counts are recorded, not required.
 */
static bool burst_long(const struct m3_env *env)
{
	const struct ce_twoplane_hw *hw = env->hw;
	static uint64_t sub[M3_BURST_LONG_N];
	static uint32_t safety[M3_BURST_LONG_N], app[M3_BURST_LONG_N];
	uint32_t s = on_safety, a = other_app(on_app);
	/* present() retries no more once its count is at the limit: here every
	 * refusal is counted instead.
	 */
	unsigned accepted = 0, busy = 0, full = 0, other = 0, no_retry = M3_BUSY_RETRIES_MAX;
	bool ok = settle(env, 11, "burst-long") && wait_vsync(env);

	for (unsigned i = 0; i < M3_BURST_LONG_N; i++) {
		uint32_t si = (i / 3u) % 2u ? other_safety(s) : s;
		ce_status st = present(env, si, a, &sub[accepted], &no_retry);

		if (st == CE_OK) {
			safety[accepted] = si;
			app[accepted] = a;
			accepted++;
			a = other_app(a);
		} else {
			busy += st == CE_ERR_BACKEND_BUSY;
			full += st == CE_ERR_EVENT_QUEUE_FULL;
			other += st != CE_ERR_BACKEND_BUSY && st != CE_ERR_EVENT_QUEUE_FULL;
			poll_all(env);
		}
		if (i % 16u == 15u) {
			poll_all(env);
		}
	}
	ok = ok && accepted > 0 && other == 0 && wait_end(env, sub[accepted - 1]);
	if (accepted > 0) {
		ok = ok && count_sub(CE_EVENT_PRESENTED, sub[accepted - 1]) == 1u &&
		     ce_buffer_state(engine, safety[accepted - 1]) == CE_BUFFER_SCANNING &&
		     ce_buffer_state(engine, app[accepted - 1]) == CE_BUFFER_SCANNING;
	}
	for (unsigned i = 0; i < accepted; i++) {
		ok = ok && count_sub(CE_EVENT_PRESENTED, sub[i]) + count_sub(CE_EVENT_DROPPED, sub[i]) ==
				   1u;
	}
	uint32_t commits = hw->commit_count(env->dev);
	uint32_t latched = hw->latched_count(env->dev);

	ok = ok && ordinary_events() && latched == commits;
	env->log("SCENE 11 burst-long: presents %u, accepted %u, busy %u, queue full %u, other %u "
		 "(expected 0), commits %u latched %u: %s\n",
		 (unsigned)M3_BURST_LONG_N, accepted, busy, full, other, (unsigned)commits,
		 (unsigned)latched, ok ? "ok" : "FAIL");
	if (accepted > 0) {
		on_safety = safety[accepted - 1];
		on_app = app[accepted - 1];
	}
	return ok;
}

static unsigned scene11(const struct m3_env *env)
{
	unsigned failed = 0, crossed = 0;

	/* The application buffer not on the panel gets a colour the panel does
	 * not show now (the panel is blue or yellow after scene 10): a stale
	 * frame and the last frame differ.
	 */
	uint32_t x = other_app(on_app);
	uint16_t on_colour = app_colour[app_index(on_app)];

	failed += !settle(env, 11, "burst");
	failed += !draw_app(env, x, on_colour == SCENES_APP_GREEN ? M3_APP_YELLOW : SCENES_APP_GREEN,
			    true);
	for (unsigned rep = 0; rep < M3_BURST_REPS; rep++) {
		bool c = false;

		failed += !burst(env, rep, &c);
		crossed += c;
	}
	env->log("SCENE 11 burst: %u bursts of %u, %u crossed a frame start: %s\n",
		 (unsigned)M3_BURST_REPS, (unsigned)M3_BURST_N, crossed, failed ? "FAIL" : "ok");
	failed += !hold(env, 11, "burst", "the other application colour must not stay on");
	failed += !burst_long(env);
	failed += !hold(env, 11, "burst-long", NULL);
	return failed;
}

/* ---- Scene 12: cache maintenance ---- */

static unsigned scene12(const struct m3_env *env)
{
	unsigned failed = 0;
	uint32_t x = other_app(on_app);
	const struct ce_twoplane_record *f = record(x);
	size_t len = (size_t)f->height * f->stride_bytes;
	uint16_t old = app_colour[app_index(on_app)];
	uint16_t a_colour = old == SCENES_APP_BLUE ? SCENES_APP_GREEN : SCENES_APP_BLUE;

	/* 12a: draw, clean the cache, present: the flush's own result and
	 * arguments are what the firmware can judge.
	 */
	failed += !settle(env, 12, "cache-flush");
	failed += !draw_app(env, x, a_colour, true);
	{
		bool ok = last_flush_rc == 0 && f->pa % M3_CACHE_LINE_BYTES == 0 &&
			  len % M3_CACHE_LINE_BYTES == 0;

		env->log("FLUSH 12 cache-flush: %u bytes, start aligned %s, length aligned %s "
			 "(line %u bytes), returned %d: %s\n",
			 (unsigned)len, f->pa % M3_CACHE_LINE_BYTES == 0 ? "yes" : "no",
			 len % M3_CACHE_LINE_BYTES == 0 ? "yes" : "no",
			 (unsigned)M3_CACHE_LINE_BYTES, last_flush_rc, ok ? "ok" : "FAIL");
		failed += !ok;
	}
	failed += !frame_step(env, 12, "cache-flush", on_safety, x, 1, 1);
	failed += !hold(env, 12, "cache-flush", NULL);

	/* 12b: the negative control, a new colour drawn without the cache clean.
	 * Only the photograph can say whether it reached the panel.
	 */
	if (env->host_fake) {
		env->log("SCENE 12 skipped (host): cache-no-flush: the fake panel reads the buffers "
			 "directly, so a missing cache clean cannot show\n");
		return failed;
	}
	uint32_t y = other_app(x);

	failed += !settle(env, 12, "cache-no-flush");
	failed += !draw_app(env, y, M3_APP_WHITE, false);
	failed += !frame_step(env, 12, "cache-no-flush", on_safety, y, 1, 1);
	failed += !hold(env, 12, "cache-no-flush",
			"photograph decides: all white = no difference from 12a; the old colour or "
			"bands of it = the buffer is cached and the clean is needed");
	/* Leave the panel clean: the clean 12b left out, after the photograph. */
	flush(env, (void *)record(y)->pa, len);
	env->log("FLUSH 12 cache-no-flush: cleaned after the photograph, returned %d\n",
		 last_flush_rc);
	return failed;
}

bool m3_03_run(const struct m3_env *env)
{
	unsigned failed = 0;

	env->log("BEGIN hwlayer-manager board test am62p_r5f_m3_03 (scenes 9-12, draft)\n");
	if (!setup(env)) {
		env->log("RESULT FAIL (setup)\n");
		return false;
	}
	failed += scene9(env);
	failed += scene10(env);
	failed += scene11(env);
	failed += scene12(env);

	if (failed == 0) {
		env->log("RESULT PASS (scenes 9-12%s, flush errors %u, vsyncs %u)\n",
			 env->host_fake ? ", 12b skipped on the host" : "", flush_errors,
			 (unsigned)vsyncs());
	} else {
		env->log("RESULT FAIL (%u checks failed, flush errors %u, vsyncs %u)\n", failed,
			 flush_errors, (unsigned)vsyncs());
	}
	return failed == 0;
}
