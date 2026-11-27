/*
 * Interface between the cross-check driver and the fake libdrm.
 *
 * The driver parses an xcheck v1 file into a struct xc_topology and installs
 * it; the fake answers libliftoff's libdrm calls from it. Atomic commits are
 * resolved to plane states and handed to the driver's commit handler, which
 * answers TEST_ONLY (see PROTOCOL.md).
 */
#ifndef XCHECK_FAKE_H
#define XCHECK_FAKE_H

#include <stddef.h>
#include <stdint.h>

#define XC_EXPORT __attribute__((visibility("default")))

/* DRM plane type enum values. */
enum xc_plane_kind {
	XC_KIND_OVERLAY = 0,
	XC_KIND_PRIMARY = 1,
	XC_KIND_CURSOR = 2,
};

/* DRM "pixel blend mode" enum values. */
enum xc_blend {
	XC_BLEND_NONE = 0,    /* xcheck "opaque" */
	XC_BLEND_PREMULTI = 1, /* xcheck "premul" */
	XC_BLEND_COVERAGE = 2, /* xcheck "coverage" */
};

struct xc_format {
	uint32_t fourcc;
	uint64_t modifier;
};

struct xc_plane {
	uint32_t id;
	enum xc_plane_kind kind;
	uint32_t possible_crtcs; /* bit i = i-th output line */
	int zpos_fixed;          /* 1: immutable zpos = zpos_min = zpos_max */
	int64_t zpos_min, zpos_max;
	int64_t zpos_now; /* current value of the zpos property */
	int has_alpha;
	uint32_t blend_mask;    /* bit (1 << enum xc_blend); 0: no property */
	uint32_t rotation_mask; /* DRM_MODE_ROTATE_* | REFLECT_*; 0: no property */
	size_t n_formats;
	struct xc_format *formats;
};

struct xc_output {
	uint32_t id; /* used as the CRTC object id */
	uint32_t width, height, refresh_mhz;
};

struct xc_buffer {
	uint32_t id; /* used as the framebuffer object id */
	uint32_t width, height, fourcc;
	uint64_t modifier;
};

struct xc_topology {
	size_t n_outputs;
	struct xc_output *outputs;
	size_t n_planes;
	struct xc_plane *planes;
	size_t n_buffers;
	struct xc_buffer *buffers;
};

/* One plane as an atomic request leaves it (request applied on top of the
 * fake's current state, which is "all planes disabled"). */
struct xc_plane_state {
	uint32_t plane_id;
	uint64_t fb_id, crtc_id;
	int64_t crtc_x, crtc_y;
	uint64_t crtc_w, crtc_h;
	uint64_t src_x, src_y, src_w, src_h; /* 16.16 */
	uint64_t alpha;    /* 0xFFFF when the plane has no alpha property */
	int blend;         /* enum xc_blend, -1 when the plane has no property */
	uint64_t rotation; /* DRM_MODE_ROTATE_0 when the plane has no property */
	int64_t zpos;
};

/* Returns 0 to accept, or -EINVAL / -ERANGE / -ENOSPC / other -errno. */
typedef int (*xc_commit_handler)(const struct xc_plane_state *states,
				 size_t n_states, uint32_t flags, void *data);

/* The topology must stay valid while the fake is in use. Resets all object,
 * property and blob ids. */
XC_EXPORT int xc_fake_install(const struct xc_topology *topology);
XC_EXPORT void xc_fake_set_commit_handler(xc_commit_handler handler,
					  void *data);

#endif
