/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * Board test of the composition engine on the Verdin AM62P's MCU R5F: the
 * scene sequence (scenes.c) on the two-plane backend (backends/twoplane),
 * bound to the demo module's DSS driver (hw_am62p.c), with Zephyr as the
 * port. The console is printk.
 */
#include <zephyr/cache.h>
#include <zephyr/device.h>
#include <zephyr/kernel.h>
#include <zephyr/sys/printk.h>

#include "scenes.h"

#ifndef TEST_APP_MAX_WIDTH
#define TEST_APP_MAX_WIDTH 1280
#endif
#ifndef TEST_APP_MAX_HEIGHT
#define TEST_APP_MAX_HEIGHT 800
#endif
#ifndef TEST_HOLD_MS
#define TEST_HOLD_MS 5000
#endif

/* The demo module's DSS driver, as struct ce_twoplane_hw (hw_am62p.c). */
extern const struct ce_twoplane_hw ce_twoplane_hw_am62p;

static const struct device *const display = DEVICE_DT_GET(DT_CHOSEN(zephyr_display));

/* Given at every vsync (port wake), taken by the thread that polls the engine. */
static K_SEM_DEFINE(vsync_sem, 0, 1);

/* The application frames, RGB565 at the panel's size. The display controller
 * reads them at the address the R5F sees them at.
 */
static uint16_t app_mem[2][TEST_APP_MAX_WIDTH * TEST_APP_MAX_HEIGHT] __aligned(64);

static unsigned int port_irq_lock(void)
{
	return irq_lock();
}

static void port_irq_unlock(unsigned int key)
{
	irq_unlock(key);
}

static void port_wake(void *arg)
{
	k_sem_give(arg);
}

static bool port_device_ready(const void *dev)
{
	return device_is_ready(dev);
}

static void env_wait_wake(uint32_t ms)
{
	(void)k_sem_take(&vsync_sem, K_MSEC(ms));
}

static void env_hold(uint32_t ms, unsigned scene)
{
	(void)scene;
	k_msleep(ms);
}

static void env_flush(void *p, size_t len)
{
	(void)sys_cache_data_flush_range(p, len);
}

int main(void)
{
	const struct scenes_env env = {
		.hw = &ce_twoplane_hw_am62p,
		.dev = display,
		.irq_lock = port_irq_lock,
		.irq_unlock = port_irq_unlock,
		.wake = port_wake,
		.wake_arg = &vsync_sem,
		.device_ready = port_device_ready,
		.wait_wake = env_wait_wake,
		.hold = env_hold,
		.flush = env_flush,
		.log = printk,
		.app_mem = {app_mem[0], app_mem[1]},
		.app_size = sizeof(app_mem[0]),
		.hold_ms = TEST_HOLD_MS,
	};

	(void)scenes_run(&env);

	/* Idle with the last scene on the panel. */
	for (;;) {
		k_sleep(K_FOREVER);
	}
	return 0;
}
