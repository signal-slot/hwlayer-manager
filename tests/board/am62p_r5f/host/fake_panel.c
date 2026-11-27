/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * Host run of the board test's scene sequence (src/scenes.c) on the
 * two-plane backend behind a fake driver and a fake panel.
 *
 * The fake driver writes shadow registers and counts one commit per write;
 * a frame start (at every wait for a wake-up, and every 16 ms of a hold)
 * copies them to the active registers, makes the latched count equal the
 * commit count, and runs the vsync callback. At every hold, the fake panel
 * composes the active planes at three points (a corner, the middle of the
 * left rectangle, the middle of the right one) and compares them with what
 * the photograph of that scene should show.
 *
 * Exit status 0: the sequence said PASS and every scene looked as expected.
 */
#include "scenes.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

enum { WIDTH = 96, HEIGHT = 48 };

static struct regs {
	struct ce_twoplane_buf app;
	bool app_on;
	unsigned safety;
} shadow, active;

static uint32_t commits, latched;
static uint32_t safety_mem[2][WIDTH * HEIGHT];
static uint16_t app_mem[2][WIDTH * HEIGHT];
static ce_twoplane_vsync_cb_t vsync_cb;
static void *vsync_user;
static unsigned lock_depth;
static bool isr_deferred;
static unsigned wakes, flushes;
static int dev_token;

static void run_isr(void)
{
	if (vsync_cb == NULL) {
		return;
	}
	if (lock_depth > 0) {
		isr_deferred = true;
		return;
	}
	vsync_cb(&dev_token, vsync_user);
}

static void frame_start(void)
{
	if (latched != commits) {
		active = shadow;
		latched = commits;
	}
	run_isr();
}

static bool hw_app_format_ok(uint32_t format)
{
	return format == CE_FORMAT_RGB565;
}

static int hw_set_app_buffer(const void *dev, const struct ce_twoplane_buf *buf)
{
	(void)dev;
	shadow.app = *buf;
	commits++;
	return 0;
}

static int hw_enable_app_plane(const void *dev, bool on)
{
	(void)dev;
	shadow.app_on = on;
	commits++;
	return 0;
}

static int hw_safety_flip(const void *dev)
{
	(void)dev;
	shadow.safety ^= 1u;
	commits++;
	return 0;
}

static void *hw_safety_buffer(const void *dev, uint32_t *w, uint32_t *h, uint32_t *stride)
{
	(void)dev;
	*w = WIDTH;
	*h = HEIGHT;
	*stride = WIDTH * 4;
	return safety_mem[shadow.safety ^ 1u];
}

static void *hw_safety_front(const void *dev)
{
	(void)dev;
	return safety_mem[active.safety];
}

static int hw_safety_premultiplied(const void *dev, bool on)
{
	(void)dev;
	(void)on;
	return 0;
}

static uint32_t hw_commit_count(const void *dev)
{
	(void)dev;
	return commits;
}

static uint32_t hw_latched_count(const void *dev)
{
	(void)dev;
	return latched;
}

static void hw_set_vsync_callback(const void *dev, ce_twoplane_vsync_cb_t cb, void *user)
{
	(void)dev;
	vsync_cb = cb;
	vsync_user = user;
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

static unsigned int port_irq_lock(void)
{
	return lock_depth++;
}

static void port_irq_unlock(unsigned int key)
{
	lock_depth = key;
	if (lock_depth == 0 && isr_deferred) {
		isr_deferred = false;
		run_isr();
	}
}

static void port_wake(void *arg)
{
	(void)arg;
	wakes++;
}

static bool port_device_ready(const void *dev)
{
	return dev == &dev_token;
}

static void env_wait_wake(uint32_t ms)
{
	(void)ms;
	frame_start();
}

/* ---- The fake panel ---- */

enum colour { BLACK, RED, BLUE, GREEN, OTHER };
static const char *const colour_names[] = {"black", "red", "blue", "green", "other"};

/* What the panel shows at (x, y): the safety pixel, premultiplied, over the
 * application plane (or the controller's black background when it is off).
 */
static enum colour panel_at(uint32_t x, uint32_t y)
{
	uint32_t s = safety_mem[active.safety][y * WIDTH + x];

	if (s == SCENES_SAFETY_RED) {
		return RED;
	}
	if (s != SCENES_CLEAR) {
		return OTHER;
	}
	if (!active.app_on) {
		return BLACK;
	}
	const uint16_t *app = (const uint16_t *)active.app.pa;
	uint16_t p = app[y * (active.app.stride_bytes / 2) + x];

	return p == SCENES_APP_BLUE ? BLUE : p == SCENES_APP_GREEN ? GREEN : OTHER;
}

/* corner, left rectangle, right rectangle, by scene (1-based) */
static const enum colour expected[][3] = {
	{BLACK, BLACK, BLACK}, /* unused */
	{RED, RED, RED},       /* 1 safety-only */
	{BLUE, RED, BLUE},     /* 2 app-under-safety */
	{GREEN, RED, GREEN},   /* 3 app-replaced */
	{GREEN, GREEN, RED},   /* 4 safety-flipped */
	{BLACK, BLACK, RED},   /* 5 app-off */
	{BLACK, BLACK, RED},   /* 6 refused-no-safety: unchanged */
	{BLACK, BLACK, RED},   /* 7 refused-blank: unchanged */
	{BLACK, BLACK, RED},   /* 8 refused-foreign-safety: unchanged */
};
enum { SCENES = sizeof(expected) / sizeof(expected[0]) - 1 };

static unsigned held, panel_failures;

static void env_hold(uint32_t ms, unsigned scene)
{
	for (uint32_t t = 0; t < ms; t += 16) {
		frame_start();
	}
	struct scenes_rect l = scenes_rect(WIDTH, HEIGHT, SCENES_RECT_LEFT);
	struct scenes_rect r = scenes_rect(WIDTH, HEIGHT, SCENES_RECT_RIGHT);
	enum colour seen[3] = {
		panel_at(0, 0),
		panel_at(l.x + l.width / 2, l.y + l.height / 2),
		panel_at(r.x + r.width / 2, r.y + r.height / 2),
	};

	held++;
	if (scene < 1 || scene > SCENES || memcmp(seen, expected[scene], sizeof(seen)) != 0) {
		panel_failures++;
		printf("PANEL %u: corner %s, left %s, right %s: unexpected\n", scene,
		       colour_names[seen[0]], colour_names[seen[1]], colour_names[seen[2]]);
	} else {
		printf("PANEL %u: corner %s, left %s, right %s\n", scene, colour_names[seen[0]],
		       colour_names[seen[1]], colour_names[seen[2]]);
	}
}

static void env_flush(void *p, size_t len)
{
	(void)p;
	(void)len;
	flushes++;
}

static void env_log(const char *fmt, ...)
{
	va_list ap;

	va_start(ap, fmt);
	vprintf(fmt, ap);
	va_end(ap);
}

int main(void)
{
	/* What the driver's front buffer held before the test: not red, not clear. */
	for (size_t i = 0; i < WIDTH * HEIGHT; i++) {
		safety_mem[0][i] = 0xFF123456u;
	}
	const struct scenes_env env = {
		.hw = &hw,
		.dev = &dev_token,
		.irq_lock = port_irq_lock,
		.irq_unlock = port_irq_unlock,
		.wake = port_wake,
		.wake_arg = NULL,
		.device_ready = port_device_ready,
		.wait_wake = env_wait_wake,
		.hold = env_hold,
		.flush = env_flush,
		.log = env_log,
		.app_mem = {app_mem[0], app_mem[1]},
		.app_size = sizeof(app_mem[0]),
		.hold_ms = 100,
	};
	bool pass = scenes_run(&env);

	if (held != SCENES) {
		printf("PANEL: %u scenes held, expected %u\n", held, (unsigned)SCENES);
		return 1;
	}
	if (wakes == 0 || flushes == 0 || lock_depth != 0) {
		printf("PORT: wakes %u flushes %u lock depth %u\n", wakes, flushes, lock_depth);
		return 1;
	}
	return pass && panel_failures == 0 ? 0 : 1;
}
