/* SPDX-License-Identifier: Apache-2.0 */
/*
 * A minimal freestanding image around the twoplane backend, for the link
 * check of scripts/check-arm-link.sh. It is never run: it only has to name
 * the backend and the engine symbols, so that the linker must resolve them
 * all. The driver and port below are stubs that do nothing.
 */
#include <stddef.h>
#include <stdint.h>
#include <ce_twoplane.h>

static bool app_format_ok(uint32_t format)
{
	return format == CE_FORMAT_ARGB8888;
}
static int set_app_buffer(const void *dev, const struct ce_twoplane_buf *buf)
{
	(void)dev;
	(void)buf;
	return 0;
}
static int enable_app_plane(const void *dev, bool enable)
{
	(void)dev;
	(void)enable;
	return 0;
}
static int safety_flip(const void *dev)
{
	(void)dev;
	return 0;
}
static void *safety_buffer(const void *dev, uint32_t *w, uint32_t *h, uint32_t *stride)
{
	(void)dev;
	*w = *h = *stride = 0;
	return NULL;
}
static void *safety_front(const void *dev)
{
	(void)dev;
	return NULL;
}
static int safety_premultiplied(const void *dev, bool on)
{
	(void)dev;
	(void)on;
	return 0;
}
static uint32_t commit_count(const void *dev)
{
	(void)dev;
	return 0;
}
static uint32_t latched_count(const void *dev)
{
	(void)dev;
	return 0;
}
static void set_vsync_callback(const void *dev, ce_twoplane_vsync_cb_t cb, void *user)
{
	(void)dev;
	(void)cb;
	(void)user;
}

static const struct ce_twoplane_hw hw = {
	.app_format_ok = app_format_ok,
	.set_app_buffer = set_app_buffer,
	.enable_app_plane = enable_app_plane,
	.safety_flip = safety_flip,
	.safety_buffer = safety_buffer,
	.safety_front = safety_front,
	.safety_premultiplied = safety_premultiplied,
	.commit_count = commit_count,
	.latched_count = latched_count,
	.set_vsync_callback = set_vsync_callback,
};

static unsigned int irq_lock(void)
{
	return 0;
}
static void irq_unlock(unsigned int key)
{
	(void)key;
}
static void wake(void *arg)
{
	(void)arg;
}
static const struct ce_twoplane_record *lookup(void *arg, uint32_t id)
{
	(void)arg;
	(void)id;
	return NULL;
}
static bool device_ready(const void *dev)
{
	(void)dev;
	return true;
}

static const struct ce_twoplane_port port = {
	.irq_lock = irq_lock,
	.irq_unlock = irq_unlock,
	.wake = wake,
	.lookup = lookup,
	.device_ready = device_ready,
};

static struct ce_twoplane backend;
static uint8_t storage[CE_ENGINE_SIZE] __attribute__((aligned(CE_ENGINE_ALIGN)));

int main(void)
{
	ce_event event;
	ce_engine *engine;

	if (!CE_ABI_CHECK())
		return 1;
	if (ce_twoplane_init(&backend, &hw, &port, NULL) != 0)
		return 2;
	engine = ce_engine_init(storage, sizeof storage, &ce_twoplane_ops, &backend);
	if (engine == NULL)
		return 3;
	(void)ce_advance_time(engine, 1);
	(void)ce_poll_event(engine, &event);
	return 0;
}
