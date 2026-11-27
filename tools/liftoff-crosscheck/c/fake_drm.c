/*
 * Fake libdrm for the libliftoff cross-check.
 *
 * Built as a shared library named libdrm.so and linked in place of libdrm.
 * It provides the libdrm functions libliftoff calls
 * (docs/libliftoff-crosscheck.md section 2) on top of a topology read from an
 * xcheck v1 file: one CRTC per output line, planes with type, possible_crtcs,
 * zpos, IN_FORMATS, alpha, pixel blend mode and rotation properties carrying
 * kernel-like flags, ranges and enums, and framebuffers readable with
 * drmModeGetFB2. Atomic commits are resolved to plane states and passed to
 * the driver's commit handler. No hardware is touched; the fd is not used.
 */
#define _GNU_SOURCE
#include <errno.h>
#include <inttypes.h>
#include <limits.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <drm_fourcc.h>
#include <drm_mode.h>
#include <xf86drm.h>
#include <xf86drmMode.h>

#include "xcheck_fake.h"

enum field {
	F_TYPE,
	F_FB_ID,
	F_CRTC_ID,
	F_CRTC_X,
	F_CRTC_Y,
	F_CRTC_W,
	F_CRTC_H,
	F_SRC_X,
	F_SRC_Y,
	F_SRC_W,
	F_SRC_H,
	F_ZPOS,
	F_IN_FORMATS,
	F_ALPHA,
	F_BLEND,
	F_ROTATION,
};

#define MAX_ENUMS 8
#define PROPS_PER_PLANE 16 /* one per enum field value */

struct fprop {
	uint32_t id;
	uint32_t obj_id;
	size_t plane_index;
	enum field field;
	const char *name;
	uint32_t flags;
	uint64_t value; /* current value */
	size_t n_values;
	uint64_t values[MAX_ENUMS];
	size_t n_enums;
	struct drm_mode_property_enum enums[MAX_ENUMS];
};

struct fblob {
	uint32_t id;
	size_t length;
	void *data;
};

static const struct xc_topology *topo;
static struct fprop *props;
static size_t n_props;
static struct fblob *blobs;
static size_t n_blobs;
static xc_commit_handler commit_handler;
static void *commit_data;

static void *xcalloc(size_t n, size_t size)
{
	void *p = calloc(n ? n : 1, size ? size : 1);
	if (!p) {
		fprintf(stderr, "fake_drm: out of memory\n");
		abort();
	}
	return p;
}

static void reset(void)
{
	for (size_t i = 0; i < n_blobs; i++)
		free(blobs[i].data);
	free(blobs);
	free(props);
	blobs = NULL;
	props = NULL;
	n_blobs = 0;
	n_props = 0;
}

static struct fprop *add_prop(uint32_t *next_id, size_t plane_index,
			      enum field field, const char *name,
			      uint32_t flags, uint64_t value)
{
	struct fprop *p = &props[n_props++];
	memset(p, 0, sizeof(*p));
	p->id = (*next_id)++;
	p->obj_id = topo->planes[plane_index].id;
	p->plane_index = plane_index;
	p->field = field;
	p->name = name;
	p->flags = flags;
	p->value = value;
	return p;
}

static void prop_range(struct fprop *p, uint64_t min, uint64_t max)
{
	p->n_values = 2;
	p->values[0] = min;
	p->values[1] = max;
}

static void prop_enum(struct fprop *p, uint64_t value, const char *name)
{
	struct drm_mode_property_enum *e = &p->enums[p->n_enums++];
	e->value = value;
	snprintf(e->name, sizeof(e->name), "%s", name);
	p->values[p->n_values++] = value;
}

/* IN_FORMATS blob, laid out as the kernel does (struct
 * drm_format_modifier_blob, then the format list, then the modifier list). */
static void build_in_formats(struct fblob *b, const struct xc_plane *pl)
{
	uint32_t *fmts = xcalloc(pl->n_formats, sizeof(*fmts));
	uint64_t *mods = xcalloc(pl->n_formats, sizeof(*mods));
	size_t n_fmts = 0, n_mods = 0;

	for (size_t i = 0; i < pl->n_formats; i++) {
		size_t j;
		for (j = 0; j < n_fmts && fmts[j] != pl->formats[i].fourcc; j++)
			;
		if (j == n_fmts)
			fmts[n_fmts++] = pl->formats[i].fourcc;
		for (j = 0; j < n_mods && mods[j] != pl->formats[i].modifier; j++)
			;
		if (j == n_mods)
			mods[n_mods++] = pl->formats[i].modifier;
	}

	/* One drm_format_modifier entry per (modifier, 64-format window) that
	 * has at least one format. */
	size_t n_windows = (n_fmts + 63) / 64;
	struct drm_format_modifier *entries =
		xcalloc(n_mods * (n_windows ? n_windows : 1), sizeof(*entries));
	size_t n_entries = 0;
	for (size_t m = 0; m < n_mods; m++) {
		for (size_t w = 0; w < n_windows; w++) {
			uint64_t bits = 0;
			for (size_t i = 0; i < pl->n_formats; i++) {
				if (pl->formats[i].modifier != mods[m])
					continue;
				for (size_t f = w * 64; f < n_fmts && f < w * 64 + 64; f++)
					if (fmts[f] == pl->formats[i].fourcc)
						bits |= UINT64_C(1) << (f - w * 64);
			}
			if (!bits)
				continue;
			entries[n_entries].formats = bits;
			entries[n_entries].offset = (uint32_t)(w * 64);
			entries[n_entries].modifier = mods[m];
			n_entries++;
		}
	}

	struct drm_format_modifier_blob head = {
		.version = FORMAT_BLOB_CURRENT,
		.flags = 0,
		.count_formats = (uint32_t)n_fmts,
		.formats_offset = sizeof(head),
		.count_modifiers = (uint32_t)n_entries,
	};
	size_t fmt_bytes = n_fmts * sizeof(uint32_t);
	/* drm_format_modifier holds u64 fields: keep the list 8-byte aligned. */
	head.modifiers_offset = (uint32_t)((sizeof(head) + fmt_bytes + 7) & ~(size_t)7);
	b->length = head.modifiers_offset + n_entries * sizeof(*entries);
	b->data = xcalloc(1, b->length);
	memcpy(b->data, &head, sizeof(head));
	memcpy((char *)b->data + head.formats_offset, fmts, fmt_bytes);
	memcpy((char *)b->data + head.modifiers_offset, entries,
	       n_entries * sizeof(*entries));
	free(fmts);
	free(mods);
	free(entries);
}

static const struct {
	uint32_t bit;
	const char *name;
} rotation_names[] = {
	{ DRM_MODE_ROTATE_0, "rotate-0" },
	{ DRM_MODE_ROTATE_90, "rotate-90" },
	{ DRM_MODE_ROTATE_180, "rotate-180" },
	{ DRM_MODE_ROTATE_270, "rotate-270" },
	{ DRM_MODE_REFLECT_X, "reflect-x" },
	{ DRM_MODE_REFLECT_Y, "reflect-y" },
};

static const char *const blend_names[] = { "None", "Pre-multiplied", "Coverage" };

XC_EXPORT int xc_fake_install(const struct xc_topology *topology)
{
	reset();
	topo = topology;

	/* Property and blob ids start above every object id of the file, so no
	 * id means two things. */
	uint32_t max_id = 0;
	for (size_t i = 0; i < topo->n_outputs; i++)
		if (topo->outputs[i].id > max_id)
			max_id = topo->outputs[i].id;
	for (size_t i = 0; i < topo->n_planes; i++)
		if (topo->planes[i].id > max_id)
			max_id = topo->planes[i].id;
	for (size_t i = 0; i < topo->n_buffers; i++)
		if (topo->buffers[i].id > max_id)
			max_id = topo->buffers[i].id;
	if (max_id > UINT32_MAX / 2)
		return -EINVAL;
	uint32_t next_id = max_id + 1000;

	props = xcalloc(topo->n_planes * PROPS_PER_PLANE, sizeof(*props));
	blobs = xcalloc(topo->n_planes, sizeof(*blobs));

	for (size_t i = 0; i < topo->n_planes; i++) {
		const struct xc_plane *pl = &topo->planes[i];
		struct fprop *p;

		p = add_prop(&next_id, i, F_TYPE, "type",
			     DRM_MODE_PROP_ENUM | DRM_MODE_PROP_IMMUTABLE, pl->kind);
		prop_enum(p, XC_KIND_OVERLAY, "Overlay");
		prop_enum(p, XC_KIND_PRIMARY, "Primary");
		prop_enum(p, XC_KIND_CURSOR, "Cursor");

		/* The standard plane properties, flagged as the kernel flags them. */
		const uint32_t atomic = DRM_MODE_PROP_ATOMIC;
		p = add_prop(&next_id, i, F_FB_ID, "FB_ID", atomic | DRM_MODE_PROP_OBJECT, 0);
		p->n_values = 1;
		p->values[0] = DRM_MODE_OBJECT_FB;
		p = add_prop(&next_id, i, F_CRTC_ID, "CRTC_ID", atomic | DRM_MODE_PROP_OBJECT, 0);
		p->n_values = 1;
		p->values[0] = DRM_MODE_OBJECT_CRTC;

		p = add_prop(&next_id, i, F_CRTC_X, "CRTC_X", atomic | DRM_MODE_PROP_SIGNED_RANGE, 0);
		prop_range(p, (uint64_t)(int64_t)INT_MIN, (uint64_t)(int64_t)INT_MAX);
		p = add_prop(&next_id, i, F_CRTC_Y, "CRTC_Y", atomic | DRM_MODE_PROP_SIGNED_RANGE, 0);
		prop_range(p, (uint64_t)(int64_t)INT_MIN, (uint64_t)(int64_t)INT_MAX);
		p = add_prop(&next_id, i, F_CRTC_W, "CRTC_W", atomic | DRM_MODE_PROP_RANGE, 0);
		prop_range(p, 0, INT_MAX);
		p = add_prop(&next_id, i, F_CRTC_H, "CRTC_H", atomic | DRM_MODE_PROP_RANGE, 0);
		prop_range(p, 0, INT_MAX);
		static const char *const src_names[] = { "SRC_X", "SRC_Y", "SRC_W", "SRC_H" };
		for (int s = 0; s < 4; s++) {
			p = add_prop(&next_id, i, (enum field)(F_SRC_X + s), src_names[s],
				     atomic | DRM_MODE_PROP_RANGE, 0);
			prop_range(p, 0, UINT32_MAX);
		}

		if (pl->zpos_fixed) {
			p = add_prop(&next_id, i, F_ZPOS, "zpos",
				     DRM_MODE_PROP_RANGE | DRM_MODE_PROP_IMMUTABLE,
				     (uint64_t)pl->zpos_min);
		} else {
			p = add_prop(&next_id, i, F_ZPOS, "zpos", DRM_MODE_PROP_RANGE,
				     (uint64_t)pl->zpos_now);
		}
		prop_range(p, (uint64_t)pl->zpos_min, (uint64_t)pl->zpos_max);

		if (pl->n_formats) {
			struct fblob *b = &blobs[n_blobs++];
			b->id = next_id++;
			build_in_formats(b, pl);
			p = add_prop(&next_id, i, F_IN_FORMATS, "IN_FORMATS",
				     DRM_MODE_PROP_BLOB | DRM_MODE_PROP_IMMUTABLE, b->id);
		}

		if (pl->has_alpha) {
			p = add_prop(&next_id, i, F_ALPHA, "alpha", DRM_MODE_PROP_RANGE, 0xFFFF);
			prop_range(p, 0, 0xFFFF);
		}

		if (pl->blend_mask) {
			/* Kernel default is pre-multiplied when it is offered. */
			uint64_t def = XC_BLEND_PREMULTI;
			if (!(pl->blend_mask & (1u << XC_BLEND_PREMULTI)))
				def = (pl->blend_mask & 1u) ? XC_BLEND_NONE : XC_BLEND_COVERAGE;
			p = add_prop(&next_id, i, F_BLEND, "pixel blend mode", DRM_MODE_PROP_ENUM, def);
			for (int m = 0; m < 3; m++)
				if (pl->blend_mask & (1u << m))
					prop_enum(p, (uint64_t)m, blend_names[m]);
		}

		if (pl->rotation_mask) {
			p = add_prop(&next_id, i, F_ROTATION, "rotation", DRM_MODE_PROP_BITMASK,
				     DRM_MODE_ROTATE_0);
			for (size_t r = 0; r < sizeof(rotation_names) / sizeof(rotation_names[0]); r++)
				if ((pl->rotation_mask | DRM_MODE_ROTATE_0) & rotation_names[r].bit)
					prop_enum(p, (uint64_t)__builtin_ctz(rotation_names[r].bit),
						  rotation_names[r].name);
		}
	}
	return 0;
}

XC_EXPORT void xc_fake_set_commit_handler(xc_commit_handler handler, void *data)
{
	commit_handler = handler;
	commit_data = data;
}

static void *fail(int err)
{
	errno = err;
	return NULL;
}

/* --- resources ----------------------------------------------------------- */

drmModeResPtr drmModeGetResources(int fd)
{
	(void)fd;
	if (!topo)
		return fail(ENODEV);
	drmModeResPtr res = xcalloc(1, sizeof(*res));
	res->count_crtcs = (int)topo->n_outputs;
	res->crtcs = xcalloc(topo->n_outputs, sizeof(uint32_t));
	for (size_t i = 0; i < topo->n_outputs; i++)
		res->crtcs[i] = topo->outputs[i].id;
	res->min_width = 1;
	res->min_height = 1;
	res->max_width = 16384;
	res->max_height = 16384;
	return res;
}

void drmModeFreeResources(drmModeResPtr res)
{
	if (!res)
		return;
	free(res->fbs);
	free(res->crtcs);
	free(res->connectors);
	free(res->encoders);
	free(res);
}

drmModePlaneResPtr drmModeGetPlaneResources(int fd)
{
	(void)fd;
	if (!topo)
		return fail(ENODEV);
	drmModePlaneResPtr res = xcalloc(1, sizeof(*res));
	res->count_planes = (uint32_t)topo->n_planes;
	res->planes = xcalloc(topo->n_planes, sizeof(uint32_t));
	for (size_t i = 0; i < topo->n_planes; i++)
		res->planes[i] = topo->planes[i].id;
	return res;
}

void drmModeFreePlaneResources(drmModePlaneResPtr res)
{
	if (!res)
		return;
	free(res->planes);
	free(res);
}

static const struct xc_plane *find_plane(uint32_t id, size_t *index)
{
	for (size_t i = 0; topo && i < topo->n_planes; i++) {
		if (topo->planes[i].id == id) {
			if (index)
				*index = i;
			return &topo->planes[i];
		}
	}
	return NULL;
}

drmModePlanePtr drmModeGetPlane(int fd, uint32_t plane_id)
{
	(void)fd;
	const struct xc_plane *pl = find_plane(plane_id, NULL);
	if (!pl)
		return fail(ENOENT);
	drmModePlanePtr p = xcalloc(1, sizeof(*p));
	p->plane_id = pl->id;
	p->possible_crtcs = pl->possible_crtcs;
	/* Legacy format list: the distinct fourccs. */
	p->formats = xcalloc(pl->n_formats, sizeof(uint32_t));
	for (size_t i = 0; i < pl->n_formats; i++) {
		uint32_t j;
		for (j = 0; j < p->count_formats && p->formats[j] != pl->formats[i].fourcc; j++)
			;
		if (j == p->count_formats)
			p->formats[p->count_formats++] = pl->formats[i].fourcc;
	}
	return p;
}

void drmModeFreePlane(drmModePlanePtr plane)
{
	if (!plane)
		return;
	free(plane->formats);
	free(plane);
}

/* --- properties ---------------------------------------------------------- */

drmModeObjectPropertiesPtr drmModeObjectGetProperties(int fd, uint32_t object_id,
						      uint32_t object_type)
{
	(void)fd;
	if (!topo)
		return fail(ENODEV);
	bool known = false;
	if (object_type == DRM_MODE_OBJECT_PLANE || object_type == DRM_MODE_OBJECT_ANY)
		known = find_plane(object_id, NULL) != NULL;
	if (!known && (object_type == DRM_MODE_OBJECT_CRTC || object_type == DRM_MODE_OBJECT_ANY))
		for (size_t i = 0; i < topo->n_outputs; i++)
			if (topo->outputs[i].id == object_id)
				known = true;
	if (!known)
		return fail(ENOENT);

	drmModeObjectPropertiesPtr res = xcalloc(1, sizeof(*res));
	res->props = xcalloc(PROPS_PER_PLANE, sizeof(uint32_t));
	res->prop_values = xcalloc(PROPS_PER_PLANE, sizeof(uint64_t));
	for (size_t i = 0; i < n_props; i++) {
		if (props[i].obj_id != object_id || res->count_props >= PROPS_PER_PLANE)
			continue;
		res->props[res->count_props] = props[i].id;
		res->prop_values[res->count_props] = props[i].value;
		res->count_props++;
	}
	return res;
}

void drmModeFreeObjectProperties(drmModeObjectPropertiesPtr ptr)
{
	if (!ptr)
		return;
	free(ptr->props);
	free(ptr->prop_values);
	free(ptr);
}

static struct fprop *find_prop(uint32_t prop_id)
{
	for (size_t i = 0; i < n_props; i++)
		if (props[i].id == prop_id)
			return &props[i];
	return NULL;
}

drmModePropertyPtr drmModeGetProperty(int fd, uint32_t property_id)
{
	(void)fd;
	const struct fprop *fp = find_prop(property_id);
	if (!fp)
		return fail(ENOENT);
	drmModePropertyPtr p = xcalloc(1, sizeof(*p));
	p->prop_id = fp->id;
	p->flags = fp->flags;
	snprintf(p->name, sizeof(p->name), "%s", fp->name);
	p->count_values = (int)fp->n_values;
	p->values = xcalloc(fp->n_values, sizeof(uint64_t));
	memcpy(p->values, fp->values, fp->n_values * sizeof(uint64_t));
	p->count_enums = (int)fp->n_enums;
	p->enums = xcalloc(fp->n_enums, sizeof(struct drm_mode_property_enum));
	memcpy(p->enums, fp->enums, fp->n_enums * sizeof(struct drm_mode_property_enum));
	if (fp->flags & DRM_MODE_PROP_BLOB) {
		p->count_blobs = 1;
		p->blob_ids = xcalloc(1, sizeof(uint32_t));
		p->blob_ids[0] = (uint32_t)fp->value;
	}
	return p;
}

void drmModeFreeProperty(drmModePropertyPtr ptr)
{
	if (!ptr)
		return;
	free(ptr->values);
	free(ptr->enums);
	free(ptr->blob_ids);
	free(ptr);
}

drmModePropertyBlobPtr drmModeGetPropertyBlob(int fd, uint32_t blob_id)
{
	(void)fd;
	for (size_t i = 0; i < n_blobs; i++) {
		if (blobs[i].id != blob_id)
			continue;
		drmModePropertyBlobPtr b = xcalloc(1, sizeof(*b));
		b->id = blob_id;
		b->length = (uint32_t)blobs[i].length;
		b->data = xcalloc(1, blobs[i].length);
		memcpy(b->data, blobs[i].data, blobs[i].length);
		return b;
	}
	return fail(ENOENT);
}

void drmModeFreePropertyBlob(drmModePropertyBlobPtr ptr)
{
	if (!ptr)
		return;
	free(ptr->data);
	free(ptr);
}

/* Walks the (format, modifier) pairs of an IN_FORMATS blob: for each format
 * in list order, each modifier entry that covers it. */
bool drmModeFormatModifierBlobIterNext(const drmModePropertyBlobRes *blob,
				       drmModeFormatModifierIterator *iter)
{
	const struct drm_format_modifier_blob *head = blob->data;
	const uint32_t *fmts = (const uint32_t *)((const char *)blob->data + head->formats_offset);
	const struct drm_format_modifier *mods =
		(const void *)((const char *)blob->data + head->modifiers_offset);

	while (iter->fmt_idx < head->count_formats) {
		while (iter->mod_idx < head->count_modifiers) {
			const struct drm_format_modifier *m = &mods[iter->mod_idx++];
			uint32_t f = iter->fmt_idx;
			if (f < m->offset || f >= m->offset + 64)
				continue;
			if (!(m->formats & (UINT64_C(1) << (f - m->offset))))
				continue;
			iter->fmt = fmts[f];
			iter->mod = m->modifier;
			return true;
		}
		iter->mod_idx = 0;
		iter->fmt_idx++;
	}
	return false;
}

/* --- framebuffers -------------------------------------------------------- */

drmModeFB2Ptr drmModeGetFB2(int fd, uint32_t fb_id)
{
	(void)fd;
	for (size_t i = 0; topo && i < topo->n_buffers; i++) {
		const struct xc_buffer *b = &topo->buffers[i];
		if (b->id != fb_id)
			continue;
		drmModeFB2Ptr fb = xcalloc(1, sizeof(*fb));
		fb->fb_id = b->id;
		fb->width = b->width;
		fb->height = b->height;
		fb->pixel_format = b->fourcc;
		fb->modifier = b->modifier;
		fb->flags = DRM_MODE_FB_MODIFIERS;
		/* A non-zero handle, as a DRM master would see; closing it is a
		 * no-op. Pitch is nominal (4 bytes per pixel). */
		fb->handles[0] = 1;
		fb->pitches[0] = b->width * 4;
		return fb;
	}
	return fail(ENOENT);
}

void drmModeFreeFB2(drmModeFB2Ptr ptr)
{
	free(ptr);
}

int drmCloseBufferHandle(int fd, uint32_t handle)
{
	(void)fd;
	(void)handle;
	return 0;
}

/* --- atomic -------------------------------------------------------------- */

struct atomic_item {
	uint32_t object_id;
	uint32_t property_id;
	uint64_t value;
};

struct _drmModeAtomicReq {
	uint32_t cursor;
	uint32_t size;
	struct atomic_item *items;
};

drmModeAtomicReqPtr drmModeAtomicAlloc(void)
{
	return xcalloc(1, sizeof(struct _drmModeAtomicReq));
}

void drmModeAtomicFree(drmModeAtomicReqPtr req)
{
	if (!req)
		return;
	free(req->items);
	free(req);
}

drmModeAtomicReqPtr drmModeAtomicDuplicate(const drmModeAtomicReqPtr old)
{
	if (!old)
		return fail(EINVAL);
	drmModeAtomicReqPtr req = drmModeAtomicAlloc();
	req->cursor = old->cursor;
	req->size = old->cursor;
	req->items = xcalloc(old->cursor, sizeof(struct atomic_item));
	memcpy(req->items, old->items, old->cursor * sizeof(struct atomic_item));
	return req;
}

static int reserve(drmModeAtomicReqPtr req, uint32_t n)
{
	if (n <= req->size)
		return 0;
	uint32_t size = req->size ? req->size : 16;
	while (size < n)
		size *= 2;
	struct atomic_item *items = realloc(req->items, size * sizeof(*items));
	if (!items)
		return -ENOMEM;
	req->items = items;
	req->size = size;
	return 0;
}

int drmModeAtomicMerge(drmModeAtomicReqPtr base, const drmModeAtomicReqPtr augment)
{
	if (!base)
		return -EINVAL;
	if (!augment || !augment->cursor)
		return 0;
	int ret = reserve(base, base->cursor + augment->cursor);
	if (ret)
		return ret;
	memcpy(&base->items[base->cursor], augment->items,
	       augment->cursor * sizeof(struct atomic_item));
	base->cursor += augment->cursor;
	return 0;
}

int drmModeAtomicGetCursor(const drmModeAtomicReqPtr req)
{
	return req ? (int)req->cursor : -EINVAL;
}

void drmModeAtomicSetCursor(drmModeAtomicReqPtr req, int cursor)
{
	if (req && cursor >= 0 && (uint32_t)cursor <= req->cursor)
		req->cursor = (uint32_t)cursor;
}

int drmModeAtomicAddProperty(drmModeAtomicReqPtr req, uint32_t object_id,
			     uint32_t property_id, uint64_t value)
{
	if (!req)
		return -EINVAL;
	int ret = reserve(req, req->cursor + 1);
	if (ret)
		return ret;
	req->items[req->cursor].object_id = object_id;
	req->items[req->cursor].property_id = property_id;
	req->items[req->cursor].value = value;
	req->cursor++;
	return (int)req->cursor;
}

static void initial_state(const struct xc_plane *pl, struct xc_plane_state *st)
{
	memset(st, 0, sizeof(*st));
	st->plane_id = pl->id;
	st->alpha = 0xFFFF;
	st->blend = -1;
	st->rotation = DRM_MODE_ROTATE_0;
	st->zpos = pl->zpos_fixed ? pl->zpos_min : pl->zpos_now;
}

int drmModeAtomicCommit(int fd, const drmModeAtomicReqPtr req, uint32_t flags,
			void *user_data)
{
	(void)fd;
	(void)user_data;
	if (!req || !topo)
		return -EINVAL;
	if (!(flags & DRM_MODE_ATOMIC_TEST_ONLY)) {
		fprintf(stderr, "fake_drm: refusing a commit without TEST_ONLY\n");
		errno = EINVAL;
		return -EINVAL;
	}

	struct xc_plane_state *st = xcalloc(topo->n_planes, sizeof(*st));
	for (size_t i = 0; i < topo->n_planes; i++) {
		initial_state(&topo->planes[i], &st[i]);
		for (size_t p = 0; p < n_props; p++)
			if (props[p].plane_index == i && props[p].field == F_BLEND)
				st[i].blend = (int)props[p].value;
	}

	int ret = 0;
	for (uint32_t k = 0; k < req->cursor; k++) {
		const struct atomic_item *it = &req->items[k];
		struct fprop *p = find_prop(it->property_id);
		if (!p || p->obj_id != it->object_id) {
			fprintf(stderr, "fake_drm: commit sets unknown property %u on object %u\n",
				it->property_id, it->object_id);
			ret = -ENOENT;
			break;
		}
		if (p->flags & DRM_MODE_PROP_IMMUTABLE) {
			/* The kernel refuses writes to immutable properties. */
			ret = -EINVAL;
			break;
		}
		struct xc_plane_state *s = &st[p->plane_index];
		switch (p->field) {
		case F_FB_ID: s->fb_id = it->value; break;
		case F_CRTC_ID: s->crtc_id = it->value; break;
		case F_CRTC_X: s->crtc_x = (int64_t)it->value; break;
		case F_CRTC_Y: s->crtc_y = (int64_t)it->value; break;
		case F_CRTC_W: s->crtc_w = it->value; break;
		case F_CRTC_H: s->crtc_h = it->value; break;
		case F_SRC_X: s->src_x = it->value; break;
		case F_SRC_Y: s->src_y = it->value; break;
		case F_SRC_W: s->src_w = it->value; break;
		case F_SRC_H: s->src_h = it->value; break;
		case F_ZPOS: s->zpos = (int64_t)it->value; break;
		case F_ALPHA: s->alpha = it->value; break;
		case F_BLEND: s->blend = (int)it->value; break;
		case F_ROTATION: s->rotation = it->value; break;
		case F_TYPE:
		case F_IN_FORMATS:
			ret = -EINVAL;
			break;
		}
		if (ret)
			break;
	}

	if (!ret) {
		/* Hand over the planes the request leaves enabled (or half set
		 * up), in topology order; the rest stay disabled. */
		size_t n = 0;
		for (size_t i = 0; i < topo->n_planes; i++)
			if (st[i].fb_id || st[i].crtc_id)
				st[n++] = st[i];
		ret = commit_handler ? commit_handler(st, n, flags, commit_data) : 0;
	}
	free(st);
	if (ret)
		errno = -ret;
	return ret;
}

/* --- capabilities: atomic and universal planes are on ---------------------- */

int drmSetClientCap(int fd, uint64_t capability, uint64_t value)
{
	(void)fd;
	(void)capability;
	(void)value;
	return 0;
}

int drmGetCap(int fd, uint64_t capability, uint64_t *value)
{
	(void)fd;
	(void)capability;
	*value = 1;
	return 0;
}

/* --- helpers some libliftoff builds use for logging ---------------------- */

char *drmGetFormatName(uint32_t format)
{
	char *name = xcalloc(1, 5);
	for (int i = 0; i < 4; i++) {
		char c = (char)((format >> (8 * i)) & 0xff);
		name[i] = (c >= 0x20 && c < 0x7f) ? c : '?';
	}
	return name;
}

char *drmGetFormatModifierName(uint64_t modifier)
{
	char *name = NULL;
	if (asprintf(&name, "0x%" PRIx64, modifier) < 0)
		return NULL;
	return name;
}

char *drmGetFormatModifierVendor(uint64_t modifier)
{
	char *name = NULL;
	if (asprintf(&name, "0x%02x", (unsigned)(modifier >> 56)) < 0)
		return NULL;
	return name;
}
