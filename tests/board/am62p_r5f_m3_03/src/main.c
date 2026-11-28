/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * M3-03 board scenes 9 to 12 on the Verdin AM62P's MCU R5F: the sequence
 * (m3_03_scenes.c) on the two-plane backend (backends/twoplane), bound to
 * the demo module's DSS driver (../am62p_r5f/src/hw_am62p.c), with Zephyr
 * as the port. The console is printk, as in tests/board/am62p_r5f.
 *
 * DRAFT: not built for or run on a board; no hardware result.
 */
#include <zephyr/cache.h>
#include <zephyr/device.h>
#include <zephyr/kernel.h>
#include <zephyr/sys/printk.h>
#include <mcui/am62p_dss.h>
#include <mcui/panel_sn65dsi83.h>

#include "m3_03.h"

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
/* The panel whose backlight this core may switch, if the board has one. */
static const struct device *const panel = DEVICE_DT_GET_ANY(mcui_sn65dsi83_lvds_panel);

/* Given at every vsync (port wake), taken by the thread that polls the engine. */
static K_SEM_DEFINE(vsync_sem, 0, 1);

/* The application frames, RGB565 at the panel's size, as in
 * tests/board/am62p_r5f. Whether this region is cached by the R5F is what
 * scene 12 asks; the MPU attributes are to be read on the board.
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

/* The timer's resolution bounds scene 10's offsets: to be read on the board. */
static uint64_t env_now_us(void)
{
	return k_ticks_to_us_floor64((uint64_t)k_uptime_ticks());
}

static void env_delay_us(uint32_t us)
{
	k_busy_wait(us);
}

static void env_hold(uint32_t ms, unsigned scene, const struct m3_view *expect)
{
	(void)scene;
	(void)expect; /* printed by the sequence as a HOLD line */
	k_msleep(ms);
}

static int env_flush(void *p, size_t len)
{
	return sys_cache_data_flush_range(p, len);
}

int main(void)
{
	const struct m3_env env = {
		.hw = &ce_twoplane_hw_am62p,
		.dev = display,
		.irq_lock = port_irq_lock,
		.irq_unlock = port_irq_unlock,
		.wake = port_wake,
		.wake_arg = &vsync_sem,
		.device_ready = port_device_ready,
		.wait_wake = env_wait_wake,
		.now_us = env_now_us,
		.delay_us = env_delay_us,
		.hold = env_hold,
		.flush = env_flush,
		.log = printk,
		.app_mem = {app_mem[0], app_mem[1]},
		.app_size = sizeof(app_mem[0]),
		.hold_ms = TEST_HOLD_MS,
		.host_fake = false,
	};

	/* As tests/board/am62p_r5f: start the panel's timing, then the backlight
	 * (a no-op where Linux holds it).
	 */
	int rc = am62p_dss_output_enable(display, true);

	printk("board test m3-03: output enable %d\n", rc);
	if (panel != NULL && device_is_ready(panel)) {
		rc = panel_sn65dsi83_backlight(panel, true);
		printk("board test m3-03: backlight %d\n", rc);
	}

	(void)m3_03_run(&env);

	/* Idle with the last state on the panel. */
	for (;;) {
		k_sleep(K_FOREVER);
	}
	return 0;
}
