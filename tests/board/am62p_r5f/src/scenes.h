/* SPDX-License-Identifier: Apache-2.0 */
/*
 * The board test's scene sequence: a fixed list of frames presented through
 * the engine's C API (composition_engine.h) on the two-plane backend
 * (backends/twoplane), each held long enough to photograph and announced
 * with one console line.
 *
 * The sequence uses no operating system and no C library beyond
 * <stdbool.h>, <stddef.h> and <stdint.h>: the firmware (main.c, Zephyr)
 * and the host test (host/fake_panel.c) give it the driver, the system and
 * the memory through struct scenes_env.
 */
#ifndef SCENES_H_
#define SCENES_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "ce_twoplane.h"

/* The flat colours of the scenes. */
#define SCENES_SAFETY_RED  0xFFFF0000u /* ARGB8888, opaque */
#define SCENES_CLEAR       0x00000000u /* ARGB8888, premultiplied transparent */
#define SCENES_APP_BLUE    0x001Fu     /* RGB565 */
#define SCENES_APP_GREEN   0x07E0u     /* RGB565 */

/* The rectangle of the safety layer, in fractions of the panel: a third of
 * its width and half its height, centred vertically, left or right.
 */
#define SCENES_RECT_LEFT   0
#define SCENES_RECT_RIGHT  1

struct scenes_rect {
	uint32_t x, y, width, height;
};

/* Where the rectangle of the safety layer is on a panel of this size. */
struct scenes_rect scenes_rect(uint32_t width, uint32_t height, int side);

struct scenes_env {
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
	/* Keeps the panel as it is for ms milliseconds; scene is its number. */
	void (*hold)(uint32_t ms, unsigned scene);
	/* Makes len bytes at p, just drawn, visible to the display controller. */
	void (*flush)(void *p, size_t len);
	/* The console, printk-like: %s, %d, %u and %x only. */
	void (*log)(const char *fmt, ...);
	/* Two application buffers of app_size bytes each (RGB565, the panel's
	 * size), at an address the display controller reads as given.
	 */
	void *app_mem[2];
	size_t app_size;
	/* How long each scene stays on the panel. */
	uint32_t hold_ms;
};

/* Runs the sequence once, logs every scene and the result line, and returns
 * true for PASS. The panel keeps the last scene.
 */
bool scenes_run(const struct scenes_env *env);

#endif /* SCENES_H_ */
