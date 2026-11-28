/* SPDX-License-Identifier: Apache-2.0 */
/*
 * M3-03 board scenes 9 to 12 (docs/m3-03-board-test-design.md): the
 * producer stops and restarts, a safety flip on top of an in-flight
 * application commit, back-to-back commits within one frame, and cache
 * maintenance. They run through the engine's C API (composition_engine.h)
 * on the two-plane backend (backends/twoplane), like scenes 1 to 8 of
 * tests/board/am62p_r5f, and print SCENE and RESULT lines in the same form.
 *
 * DRAFT: compiled and run on the host behind the fake panel only
 * (scripts/check-am62p-m3-03-c.sh). Not built for or run on a board.
 *
 * Like scenes.c, the sequence uses no operating system and no C library
 * beyond <stdbool.h>, <stddef.h> and <stdint.h>: the firmware (main.c) and
 * the host program (host/m3_03_host.c) give it the driver, the system and
 * the memory through struct m3_env.
 */
#ifndef M3_03_H_
#define M3_03_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ce_twoplane.h"
/* The colours and the rectangle of scenes 1 to 8 (tests/board/am62p_r5f),
 * so that the photographs of both apps are judged the same way.
 */
#include "scenes.h"

/* Colours added for scenes 9 to 12, RGB565. */
#define M3_APP_YELLOW  0xFFE0u /* a third application colour */
#define M3_APP_MAGENTA 0xF81Fu /* the half-drawn part in 9a: must never be on the panel */
#define M3_APP_WHITE   0xFFFFu /* 12b, drawn without a cache clean */

/* What the panel should show during a hold: the safety layer is clear with
 * the red rectangle on safety_side (SCENES_RECT_LEFT or SCENES_RECT_RIGHT),
 * over the application plane in app_colour, or over black when app_on is
 * false. The firmware only prints it (for the photograph); the host's fake
 * panel compares it with every pixel.
 */
struct m3_view {
	const char *name;
	int safety_side;
	bool app_on;
	uint16_t app_colour;
	/* NULL, or what the photograph has to decide (12b). */
	const char *note;
};

struct m3_env {
	const struct ce_twoplane_hw *hw;
	const void *dev;
	/* The port's system calls (struct ce_twoplane_port); the sequence adds
	 * its own table of buffers as the port's lookup.
	 */
	unsigned int (*irq_lock)(void);
	void (*irq_unlock)(unsigned int key);
	void (*wake)(void *wake_arg);
	void *wake_arg;
	bool (*device_ready)(const void *dev);
	/* Waits for wake() or at most ms milliseconds. */
	void (*wait_wake)(uint32_t ms);
	/* A clock in microseconds that never goes back, and a busy wait on it. */
	uint64_t (*now_us)(void);
	void (*delay_us)(uint32_t us);
	/* Keeps the panel as it is for ms milliseconds; scene is its number and
	 * expect what it should show.
	 */
	void (*hold)(uint32_t ms, unsigned scene, const struct m3_view *expect);
	/* Makes len bytes at p, just drawn, visible to the display controller;
	 * returns 0 or the cache call's error.
	 */
	int (*flush)(void *p, size_t len);
	/* The console, printk-like: %s, %d, %u and %x only. */
	void (*log)(const char *fmt, ...);
	/* Two application buffers of app_size bytes each (RGB565, the panel's
	 * size), at an address the display controller reads as given.
	 */
	void *app_mem[2];
	size_t app_size;
	/* How long each photographed state stays on the panel. */
	uint32_t hold_ms;
	/* true on the host: the fake panel reads the buffers directly, so a
	 * missing cache clean cannot show and scene 12b is skipped.
	 */
	bool host_fake;
};

/* The safety rectangle, the same as scenes_rect() of scenes.c (the host
 * program checks that they agree).
 */
struct scenes_rect m3_rect(uint32_t width, uint32_t height, int side);

/* Runs scenes 9 to 12 once, logs every step and the result line, and
 * returns true for PASS. The panel keeps the last state.
 */
bool m3_03_run(const struct m3_env *env);

#endif /* M3_03_H_ */
