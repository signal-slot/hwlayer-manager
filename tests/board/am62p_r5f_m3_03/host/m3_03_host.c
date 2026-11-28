/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * Host run of scenes 9 to 12 (src/m3_03_scenes.c) on the two-plane backend
 * behind the fake driver and panel of tests/board/am62p_r5f/host/fake_panel.c.
 * Built and run by scripts/check-am62p-m3-03-c.sh. This is NOT a board
 * result: nothing here touches an AM62P, a DSS or an R5F.
 *
 * fake_panel.c is included, not copied, so that the fake driver (shadow and
 * active registers, one commit per write, a frame start that latches them
 * and runs the vsync callback, a port lock that defers it) stays the one the
 * eight scenes run against. Its own main() and scene table are compiled in
 * but not called (its main() needs scenes.c, which the script links too).
 *
 * Added here:
 *   - a fake clock in microseconds: a frame start every FRAME_US, and every
 *     write to the driver takes WRITE_US, so that a frame start can fall
 *     between the backend's writes of a commit (scene 10's mid-write case);
 *   - a judge of the whole panel at every hold, against the struct m3_view
 *     the scene gives;
 *   - a cache clean that checks its arguments (a whole buffer each time);
 *   - application buffers aligned to 64 bytes, as the firmware's.
 *
 * Exit status 0: the sequence said PASS, every hold looked as expected and
 * every cache clean covered a whole buffer.
 */
#define main am62p_r5f_fake_panel_main
#include "fake_panel.c"
#undef main

#include "m3_03.h"

enum { FRAME_US = 16000, WRITE_US = 50 };

/* The application buffers, aligned as the firmware's (main.c) so that scene
 * 12's alignment check means the same; fake_panel.c's own are not used.
 */
static _Alignas(64) uint16_t m3h_app_mem[2][WIDTH * HEIGHT];
static uint64_t m3h_now, m3h_next_frame = FRAME_US;
static unsigned m3h_holds, m3h_panel_failures, m3h_bad_flushes, m3h_flushes;

/* Time passes; every frame start on the way latches and runs the interrupt. */
static void m3h_advance(uint64_t us)
{
	uint64_t to = m3h_now + us;

	while (m3h_next_frame <= to) {
		m3h_now = m3h_next_frame;
		m3h_next_frame += FRAME_US;
		frame_start();
	}
	m3h_now = to;
}

/* ---- The fake driver's writes, timed ---- */

static int m3h_set_app_buffer(const void *dev, const struct ce_twoplane_buf *buf)
{
	int rc = hw.set_app_buffer(dev, buf);

	m3h_advance(WRITE_US);
	return rc;
}

static int m3h_enable_app_plane(const void *dev, bool on)
{
	int rc = hw.enable_app_plane(dev, on);

	m3h_advance(WRITE_US);
	return rc;
}

static int m3h_safety_flip(const void *dev)
{
	int rc = hw.safety_flip(dev);

	m3h_advance(WRITE_US);
	return rc;
}

/* ---- struct m3_env ---- */

static void m3h_wait_wake(uint32_t ms)
{
	(void)ms;
	m3h_advance(m3h_next_frame - m3h_now);
}

static uint64_t m3h_now_us(void)
{
	return m3h_now;
}

static void m3h_delay_us(uint32_t us)
{
	m3h_advance(us);
}

static int m3h_flush(void *p, size_t len)
{
	bool app = (p == m3h_app_mem[0] || p == m3h_app_mem[1]) && len == sizeof(m3h_app_mem[0]);
	bool safety = (p == safety_mem[0] || p == safety_mem[1]) && len == sizeof(safety_mem[0]);

	m3h_flushes++;
	if (!app && !safety) {
		m3h_bad_flushes++;
		printf("FLUSH: %u bytes, not one whole buffer\n", (unsigned)len);
	}
	return 0;
}

/* What the panel shows at (x, y), and what it should: the safety pixel
 * (red, or clear over the application plane, or black with the plane off).
 */
enum { M3H_RED = 0x10000, M3H_BLACK = 0x20000, M3H_OTHER = 0x30000 };

static uint32_t m3h_seen(uint32_t x, uint32_t y)
{
	uint32_t s = safety_mem[active.safety][y * WIDTH + x];

	if (s == SCENES_SAFETY_RED) {
		return M3H_RED;
	}
	if (s != SCENES_CLEAR) {
		return M3H_OTHER;
	}
	if (!active.app_on) {
		return M3H_BLACK;
	}
	const uint16_t *a = (const uint16_t *)active.app.pa;

	return a[y * (active.app.stride_bytes / 2) + x];
}

static uint32_t m3h_expected(const struct m3_view *v, uint32_t x, uint32_t y)
{
	struct scenes_rect r = scenes_rect(WIDTH, HEIGHT, v->safety_side);

	if (x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height) {
		return M3H_RED;
	}
	return v->app_on ? v->app_colour : M3H_BLACK;
}

static void m3h_hold(uint32_t ms, unsigned scene, const struct m3_view *v)
{
	unsigned wrong = 0;
	uint32_t fx = 0, fy = 0, fseen = 0, fwant = 0;

	m3h_advance((uint64_t)ms * 1000u);
	m3h_holds++;
	for (uint32_t y = 0; y < HEIGHT; y++) {
		for (uint32_t x = 0; x < WIDTH; x++) {
			uint32_t seen = m3h_seen(x, y), want = m3h_expected(v, x, y);

			if (seen != want && wrong++ == 0) {
				fx = x;
				fy = y;
				fseen = seen;
				fwant = want;
			}
		}
	}
	if (wrong != 0) {
		m3h_panel_failures++;
		printf("PANEL %u %s: %u pixels wrong, first at %u,%u: %x, expected %x\n", scene,
		       v->name, wrong, fx, fy, fseen, fwant);
	} else {
		printf("PANEL %u %s: every pixel as expected\n", scene, v->name);
	}
}

int main(void)
{
	static struct ce_twoplane_hw timed;

	/* What the driver's front buffer held before the test: not red, not clear. */
	for (size_t i = 0; i < WIDTH * HEIGHT; i++) {
		safety_mem[0][i] = 0xFF123456u;
	}
	for (int side = SCENES_RECT_LEFT; side <= SCENES_RECT_RIGHT; side++) {
		struct scenes_rect a = m3_rect(WIDTH, HEIGHT, side), b = scenes_rect(WIDTH, HEIGHT, side);

		if (a.x != b.x || a.y != b.y || a.width != b.width || a.height != b.height) {
			printf("RECT: m3_rect and scenes_rect disagree\n");
			return 1;
		}
	}
	timed = hw;
	timed.set_app_buffer = m3h_set_app_buffer;
	timed.enable_app_plane = m3h_enable_app_plane;
	timed.safety_flip = m3h_safety_flip;

	const struct m3_env env = {
		.hw = &timed,
		.dev = &dev_token,
		.irq_lock = port_irq_lock,
		.irq_unlock = port_irq_unlock,
		.wake = port_wake,
		.wake_arg = NULL,
		.device_ready = port_device_ready,
		.wait_wake = m3h_wait_wake,
		.now_us = m3h_now_us,
		.delay_us = m3h_delay_us,
		.hold = m3h_hold,
		.flush = m3h_flush,
		.log = env_log,
		.app_mem = {m3h_app_mem[0], m3h_app_mem[1]},
		.app_size = sizeof(m3h_app_mem[0]),
		.hold_ms = 100,
		.host_fake = true,
	};
	bool pass = m3_03_run(&env);

	printf("HOST: holds %u (panel wrong %u), cache cleans %u (not a whole buffer %u), "
	       "wakes %u, lock depth %u\n",
	       m3h_holds, m3h_panel_failures, m3h_flushes, m3h_bad_flushes, wakes, lock_depth);
	return pass && m3h_holds > 0 && m3h_panel_failures == 0 && m3h_bad_flushes == 0 &&
			       m3h_flushes > 0 && wakes > 0 && lock_depth == 0
		       ? 0
		       : 1;
}
