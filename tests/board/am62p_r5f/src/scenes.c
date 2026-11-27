/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * The board test's scene sequence (scenes.h).
 *
 * Buffers, by engine id:
 *
 *   SAFETY_A  the driver's safety buffer that the plane scans at start-up
 *   SAFETY_B  the driver's other safety buffer
 *   APP_1     application frame, blue, frame number 1
 *   APP_2     application frame, green, frame number 2
 *   FOREIGN   ARGB8888 like a safety buffer, but not one of the driver's
 *
 * Scenes, and what the firmware judges of each: the status of ce_present,
 * the number of commits the driver counted for it, that the hardware
 * latched them all (latched count reaches the commit count), that the
 * engine reported the frame presented, and the buffers it released.
 *
 *   1 safety-only            B: red, whole panel                 1 commit (flip)
 *   2 app-under-safety       A: clear, red rect left; app blue   3 (buffer, enable, flip)
 *   3 app-replaced           app green, frame 2                  1 (buffer)
 *   4 safety-flipped         B: clear, red rect right            1 (flip)
 *   5 app-off                application plane off               1 (enable off)
 *   6 refused-no-safety      application layer alone             refused, 0
 *   7 refused-blank          no layer at all                     refused, 0
 *   8 refused-foreign-safety a safety buffer not the driver's    refused, 0
 *
 * What the panel shows is not judged here: the photographs do that.
 */

#include "scenes.h"

#include <composition_engine.h>

enum { SAFETY_A = 10, SAFETY_B = 11, APP_1 = 1, APP_2 = 2, FOREIGN = 12 };
enum { RECORDS = 5 };

/* How long a commit may take to be latched and reported. */
#define PRESENT_TIMEOUT_MS 1000u
#define POLL_MS            20u

static struct ce_twoplane_record table[RECORDS];
static struct ce_twoplane_port port;
static struct ce_twoplane backend;
static _Alignas(CE_ENGINE_ALIGN) unsigned char storage[CE_ENGINE_SIZE];
static ce_engine *engine;
/* FOREIGN's address: refused before anything scans it. */
static uint32_t foreign_mem[16];

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

static struct ce_twoplane_record *record(uint32_t id)
{
	return (struct ce_twoplane_record *)lookup(NULL, id);
}

struct scenes_rect scenes_rect(uint32_t width, uint32_t height, int side)
{
	struct scenes_rect r = {
		.width = width / 3,
		.height = height / 2,
		.y = height / 4,
	};

	r.x = side == SCENES_RECT_LEFT ? width / 12 : width - width / 12 - r.width;
	return r;
}

static void fill_safety(const struct scenes_env *env, uint32_t id, bool full, int side)
{
	const struct ce_twoplane_record *s = record(id);
	struct scenes_rect r = scenes_rect(s->width, s->height, side);
	uint8_t *base = (uint8_t *)s->pa;

	for (uint32_t y = 0; y < s->height; y++) {
		uint32_t *row = (uint32_t *)(base + (size_t)y * s->stride_bytes);

		for (uint32_t x = 0; x < s->width; x++) {
			bool in = full || (x >= r.x && x < r.x + r.width && y >= r.y &&
					   y < r.y + r.height);

			row[x] = in ? SCENES_SAFETY_RED : SCENES_CLEAR;
		}
	}
	env->flush(base, (size_t)s->height * s->stride_bytes);
}

static void fill_app(const struct scenes_env *env, uint32_t id, uint16_t colour)
{
	const struct ce_twoplane_record *f = record(id);
	uint8_t *base = (uint8_t *)f->pa;

	for (uint32_t y = 0; y < f->height; y++) {
		uint16_t *row = (uint16_t *)(base + (size_t)y * f->stride_bytes);

		for (uint32_t x = 0; x < f->width; x++) {
			row[x] = colour;
		}
	}
	env->flush(base, (size_t)f->height * f->stride_bytes);
}

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

struct outcome {
	bool presented;
	unsigned released;
	bool bad_event;
};

/* Polls the engine until it reports submission presented, or the time is up;
 * then takes the events that came with it.
 */
static struct outcome wait_presented(const struct scenes_env *env, uint64_t submission)
{
	struct outcome o = {0};
	ce_event e;

	for (uint32_t waited = 0; waited <= PRESENT_TIMEOUT_MS;) {
		int32_t rc;

		while ((rc = ce_poll_event(engine, &e)) == 1) {
			if (e.kind == CE_EVENT_PRESENTED && e.submission == submission) {
				o.presented = true;
			} else if (e.kind == CE_EVENT_RELEASED) {
				o.released++;
			} else if (e.kind != CE_EVENT_PRESENTED) {
				o.bad_event = true;
			}
		}
		if (rc < 0) {
			o.bad_event = true;
		}
		if (o.presented || o.bad_event) {
			break;
		}
		env->wait_wake(POLL_MS);
		waited += POLL_MS;
	}
	return o;
}

/* Presents one scene, judges it, logs its line and holds it. */
static bool scene(const struct scenes_env *env, unsigned n, const char *name,
		  const ce_layer *layers, size_t count, ce_status expect_status,
		  uint32_t expect_commits, unsigned expect_released)
{
	const struct ce_twoplane_hw *hw = env->hw;
	ce_frame frame = {.layers = layers, .layer_count = count, .output = 0};
	uint32_t before = hw->commit_count(env->dev);
	uint64_t submission = 0;
	ce_error_info info = {0};
	struct outcome o = {0};
	ce_status st = ce_present(engine, &frame, &submission, &info);
	uint32_t written = hw->commit_count(env->dev) - before;

	if (st == CE_OK) {
		o = wait_presented(env, submission);
	}
	uint32_t commits = hw->commit_count(env->dev);
	uint32_t latched = hw->latched_count(env->dev);
	bool ok = st == expect_status && written == expect_commits && latched == commits &&
		  !o.bad_event;

	if (st == CE_OK) {
		ok = ok && o.presented && o.released == expect_released;
	}
	env->log("SCENE %u %s: status %d (expected %d), commits %u latched %u "
		 "(+%u, expected +%u), presented %s, released %u (expected %u): %s\n",
		 n, name, (int)st, (int)expect_status, (unsigned)commits, (unsigned)latched,
		 (unsigned)written, (unsigned)expect_commits,
		 st == CE_OK ? (o.presented ? "yes" : "NO") : "-", o.released,
		 st == CE_OK ? expect_released : 0u, ok ? "ok" : "FAIL");

	env->hold(env->hold_ms, n);

	/* Nothing may have been written behind the engine's back while held. */
	if (hw->commit_count(env->dev) != commits) {
		env->log("SCENE %u %s: commits changed while held: %u -> %u: FAIL\n", n, name,
			 (unsigned)commits, (unsigned)hw->commit_count(env->dev));
		ok = false;
	}
	return ok;
}

static bool setup(const struct scenes_env *env)
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
	table[4] = (struct ce_twoplane_record){FOREIGN, 0, (uintptr_t)foreign_mem, w, h, stride,
					       CE_FORMAT_ARGB8888};

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

/* Draws id (begin_write unless first) and hands it to the engine; false and a
 * log line if the engine refuses.
 */
static bool drawn(const struct scenes_env *env, uint32_t id, bool again)
{
	ce_status rc = again ? ce_begin_write(engine, id) : CE_OK;

	if (rc == CE_OK) {
		switch (id) {
		case SAFETY_B: /* first red all over, then the rectangle on the right */
			fill_safety(env, id, !again, SCENES_RECT_RIGHT);
			break;
		case SAFETY_A:
			fill_safety(env, id, false, SCENES_RECT_LEFT);
			break;
		case APP_1:
			fill_app(env, id, SCENES_APP_BLUE);
			break;
		default:
			fill_app(env, id, SCENES_APP_GREEN);
			break;
		}
		rc = ce_finish_write(engine, id);
	}
	if (rc != CE_OK) {
		env->log("DRAW %u: engine returned %d\n", (unsigned)id, (int)rc);
		return false;
	}
	return true;
}

bool scenes_run(const struct scenes_env *env)
{
	unsigned failed = 0;
	ce_layer l[2];

	env->log("BEGIN hwlayer-manager board test am62p_r5f\n");
	if (!setup(env)) {
		env->log("RESULT FAIL (setup)\n");
		return false;
	}

	/* 1: B red over the whole panel, the application plane off. */
	failed += !drawn(env, SAFETY_B, false);
	l[0] = full_layer(SAFETY_B, true);
	failed += !scene(env, 1, "safety-only", l, 1, CE_OK, 1, 0);

	/* 2: A clear with a red rectangle on the left, over a blue application
	 * frame. B is released.
	 */
	failed += !drawn(env, SAFETY_A, false);
	failed += !drawn(env, APP_1, false);
	l[0] = full_layer(SAFETY_A, true);
	l[1] = full_layer(APP_1, false);
	failed += !scene(env, 2, "app-under-safety", l, 2, CE_OK, 3, 1);

	/* 3: a green application frame (frame number 2); the rectangle stays. */
	failed += !drawn(env, APP_2, false);
	l[1] = full_layer(APP_2, false);
	failed += !scene(env, 3, "app-replaced", l, 2, CE_OK, 1, 1);

	/* 4: B again, clear with the rectangle on the right: one flip. */
	failed += !drawn(env, SAFETY_B, true);
	l[0] = full_layer(SAFETY_B, true);
	failed += !scene(env, 4, "safety-flipped", l, 2, CE_OK, 1, 1);

	/* 5: the safety layer alone: the application plane is switched off. */
	failed += !scene(env, 5, "app-off", l, 1, CE_OK, 1, 1);

	/* 6, 7: no safety layer, then no layer at all: refused, nothing written. */
	l[0] = full_layer(APP_1, false);
	failed += !scene(env, 6, "refused-no-safety", l, 1, CE_ERR_BACKEND_REJECTED, 0, 0);
	failed += !scene(env, 7, "refused-blank", l, 0, CE_ERR_BACKEND_REJECTED, 0, 0);

	/* 8: a safety buffer that is not one of the driver's two: refused. */
	l[0] = full_layer(FOREIGN, true);
	failed += !scene(env, 8, "refused-foreign-safety", l, 1, CE_ERR_BACKEND_REJECTED, 0, 0);

	if (failed == 0) {
		env->log("RESULT PASS (8 scenes, vsyncs %u)\n", (unsigned)backend.vsyncs);
	} else {
		env->log("RESULT FAIL (%u checks failed, vsyncs %u)\n", failed,
			 (unsigned)backend.vsyncs);
	}
	return failed == 0;
}
