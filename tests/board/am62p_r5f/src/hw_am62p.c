/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * struct ce_twoplane_hw for the AM62P DSS driver of the demo module
 * (<mcui/am62p_dss.h>). The backend's device is an opaque pointer and the
 * driver's calls take a const struct device *, so each call goes through a
 * function of the backend's type, and the vsync callback through a
 * trampoline, rather than through a cast of a function pointer.
 */
#include <errno.h>
#include <zephyr/device.h>
#include <zephyr/kernel.h>
#include <mcui/am62p_dss.h>

#include "ce_twoplane.h"

static int dss_format(uint32_t ce)
{
	switch (ce) {
	case CE_FORMAT_RGB565:
		return AM62P_DSS_FMT_RGB565;
	case CE_FORMAT_XRGB8888:
		return AM62P_DSS_FMT_XRGB8888;
	case CE_FORMAT_ARGB8888:
		return AM62P_DSS_FMT_ARGB8888;
	default:
		return -1;
	}
}

static bool app_format_ok(uint32_t format)
{
	return dss_format(format) >= 0;
}

static int set_app_buffer(const void *dev, const struct ce_twoplane_buf *buf)
{
	int format = dss_format(buf->format);

	if (format < 0) {
		return -EINVAL;
	}
	const struct am62p_dss_plane_cfg cfg = {
		.seq = buf->seq,
		.pa = buf->pa,
		.width = buf->width,
		.height = buf->height,
		.stride_bytes = buf->stride_bytes,
		.format = (uint32_t)format,
	};

	return am62p_dss_set_app_buffer(dev, &cfg);
}

static int enable_app_plane(const void *dev, bool enable)
{
	return am62p_dss_enable_app_plane(dev, enable);
}

static int safety_flip(const void *dev)
{
	return am62p_dss_safety_flip(dev);
}

static void *safety_buffer(const void *dev, uint32_t *width, uint32_t *height,
			   uint32_t *stride_bytes)
{
	return am62p_dss_safety_buffer(dev, width, height, stride_bytes);
}

static void *safety_front(const void *dev)
{
	return am62p_dss_safety_front(dev);
}

static int safety_premultiplied(const void *dev, bool on)
{
	return am62p_dss_safety_premultiplied(dev, on);
}

static uint32_t commit_count(const void *dev)
{
	return am62p_dss_commit_count(dev);
}

static uint32_t latched_count(const void *dev)
{
	return am62p_dss_latched_count(dev);
}

/* The backend's callback, called from the driver's vsync interrupt. */
static ce_twoplane_vsync_cb_t vsync_cb;

static void vsync_trampoline(const struct device *dev, void *user)
{
	vsync_cb(dev, user);
}

static void set_vsync_callback(const void *dev, ce_twoplane_vsync_cb_t cb, void *user)
{
	vsync_cb = cb;
	am62p_dss_set_vsync_callback(dev, vsync_trampoline, user);
}

const struct ce_twoplane_hw ce_twoplane_hw_am62p = {
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
