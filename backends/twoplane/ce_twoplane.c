/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * The composition engine's backend for a two-plane display driver
 * (ce_twoplane.h).
 *
 * Latches. The driver counts commits (hw->commit_count, read after a
 * commit's writes) and the commits the hardware has latched
 * (hw->latched_count: from the GO bit on the AM62P, from ExtDst0's sync
 * busy bit on the i.MX 95). A submission is latched when
 * the second reaches the first. The check runs at every vsync (the
 * driver's callback, in its interrupt), in poll_event, and at the end of a
 * commit, before the new submission replaces the pending one: a pending
 * submission the hardware latched while the new one was being written is
 * reported, as composition_engine.h asks, and is never counted as dropped
 * while it is on the panel.
 *
 * Uses nothing beyond <stdbool.h>, <stddef.h> and <stdint.h>: the system
 * comes through b->port (ce_twoplane.h).
 */

#include "ce_twoplane.h"
#include <stddef.h>

/* The integrator's record of buffer id, or NULL. */
static const struct ce_twoplane_record *record(const struct ce_twoplane *b, uint32_t id)
{
	return b->port->lookup(b->port->lookup_arg, id);
}

static bool reached(uint32_t latched, uint32_t count)
{
	return (int32_t)(latched - count) >= 0;
}

/* Interrupts locked. A latch waits while the previous one is unreported: it
 * stays pending, and is found again at the next check (the hardware's count
 * does not go back).
 */
static void note_latch(struct ce_twoplane *b)
{
	if (!b->latched && b->pending &&
	    reached(b->hw->latched_count(b->dev), b->pending_count)) {
		b->latched = true;
		b->latched_sub = b->pending_sub;
		b->pending = false;
	}
}

static void vsync_cb(const void *dev, void *user)
{
	struct ce_twoplane *b = user;

	(void)dev;
	b->vsyncs++;
	note_latch(b);
	b->port->wake(b->port->wake_arg);
}

static void plane_caps(ce_plane_caps *p, uint32_t id, uint8_t kind, int32_t zpos, uint8_t blend)
{
	p->id = id;
	p->outputs = 1u << 0;
	p->kind = kind;
	p->zpos_min = zpos;
	p->zpos_max = zpos;
	p->blend_modes = (uint8_t)(1u << blend);
	p->transforms = 1u << CE_TRANSFORM_NORMAL;
	/* the driver programs a plane at the buffer's size from the top left */
	p->full_output_only = 1;
}

static void twoplane_capabilities(void *ctx, ce_capabilities *out)
{
	struct ce_twoplane *b = ctx;
	ce_plane_caps *app = &out->planes[0];
	ce_plane_caps *safety = &out->planes[1];

	out->outputs[0] = (ce_output_caps){.id = 0, .width = b->width, .height = b->height};
	out->output_count = 1;

	plane_caps(app, CE_TWOPLANE_PLANE_APP, CE_PLANE_PRIMARY, 0, CE_BLEND_OPAQUE);
	static const uint32_t app_formats[] = {CE_FORMAT_RGB565, CE_FORMAT_XRGB8888,
					       CE_FORMAT_ARGB8888};

	app->format_count = 0;
	for (size_t i = 0; i < sizeof(app_formats) / sizeof(app_formats[0]); i++) {
		if (b->hw->app_format_ok(app_formats[i])) {
			app->formats[app->format_count++] = (ce_format){.format = app_formats[i]};
		}
	}

	plane_caps(safety, CE_TWOPLANE_PLANE_SAFETY, CE_PLANE_OVERLAY, 1, CE_BLEND_PREMULTIPLIED);
	safety->formats[0] = (ce_format){.format = CE_FORMAT_ARGB8888};
	safety->format_count = 1;

	out->plane_count = 2;
	/* Writes before a latch ride on the pending one: the newer values win. */
	out->replace_pending = 1;
}

/* The assignment's layer on plane `plane`, or NULL. */
static const ce_layer *layer_on(const ce_frame *frame, const ce_assignment *a, size_t n,
				uint32_t plane)
{
	for (size_t i = 0; i < n; i++) {
		if (a[i].plane == plane) {
			return &frame->layers[a[i].layer];
		}
	}
	return NULL;
}

static ce_status twoplane_check(void *ctx, const ce_frame *frame, const ce_assignment *a,
				size_t n)
{
	struct ce_twoplane *b = ctx;
	const ce_layer *app = layer_on(frame, a, n, CE_TWOPLANE_PLANE_APP);
	const ce_layer *safety = layer_on(frame, a, n, CE_TWOPLANE_PLANE_SAFETY);

	/* The safety plane is never switched off: the driver has no call for it. */
	if (safety == NULL) {
		return CE_ERR_BACKEND_REJECTED;
	}
	const struct ce_twoplane_record *s = record(b, safety->buffer);

	if (s == NULL) {
		return CE_ERR_BACKEND_UNKNOWN_BUFFER;
	}
	/* Only the driver's two buffers: the one on the plane, or the other one,
	 * which a flip puts there.
	 */
	if (s->pa != b->safety_pa) {
		uint32_t w, h, stride;
		void *back = b->hw->safety_buffer(b->dev, &w, &h, &stride);

		if (s->pa != (uintptr_t)back) {
			return CE_ERR_BACKEND_REJECTED;
		}
	}
	if (app != NULL) {
		const struct ce_twoplane_record *f = record(b, app->buffer);

		if (f == NULL) {
			return CE_ERR_BACKEND_UNKNOWN_BUFFER;
		}
		if (f->pa == 0 || !b->hw->app_format_ok(f->format)) {
			return CE_ERR_BACKEND_REJECTED;
		}
	}
	return CE_OK;
}

static ce_status twoplane_commit(void *ctx, uint64_t submission, const ce_frame *frame,
				 const ce_assignment *a, size_t n)
{
	struct ce_twoplane *b = ctx;
	ce_status rc = twoplane_check(ctx, frame, a, n);

	if (rc != CE_OK) {
		return rc;
	}
	/* The engine takes every latch before it commits. An unreported one here
	 * would leave no room to report the pending submission's, should it latch
	 * during these writes: refuse before writing anything.
	 */
	unsigned int key = b->port->irq_lock();
	bool slot_full = b->latched;

	b->port->irq_unlock(key);
	if (slot_full) {
		return CE_ERR_BACKEND_BUSY;
	}
	const ce_layer *app = layer_on(frame, a, n, CE_TWOPLANE_PLANE_APP);
	const ce_layer *safety = layer_on(frame, a, n, CE_TWOPLANE_PLANE_SAFETY);

	if (app != NULL) {
		const struct ce_twoplane_record *f = record(b, app->buffer);
		const struct ce_twoplane_buf buf = {
			.seq = f->seq,
			.pa = f->pa,
			.width = f->width,
			.height = f->height,
			.stride_bytes = f->stride_bytes,
			.format = f->format,
		};

		/* A new application frame is committed, a buffer drawn again
		 * included: its frame number is what the A53 learns back. A frame
		 * that only redraws the safety layer leaves the plane alone, as
		 * before. One commit per change: the plane is enabled only when off.
		 */
		if (!b->app_on || f->id != b->app_id || f->seq != b->app_seq) {
			if (b->hw->set_app_buffer(b->dev, &buf) != 0) {
				return CE_ERR_BACKEND_REJECTED; /* refused before any write */
			}
			b->app_id = f->id;
			b->app_seq = f->seq;
		}
		if (!b->app_on) {
			if (b->hw->enable_app_plane(b->dev, true) != 0) {
				return CE_ERR_BACKEND_REJECTED; /* stays off: a later frame retries */
			}
			b->app_on = true;
		}
	} else if (b->app_on) {
		if (b->hw->enable_app_plane(b->dev, false) != 0) {
			return CE_ERR_BACKEND_REJECTED; /* stays on: a later frame retries */
		}
		b->app_on = false;
	}
	const struct ce_twoplane_record *s = record(b, safety->buffer);

	if (s->pa != b->safety_pa) {
		if (b->hw->safety_flip(b->dev) != 0) {
			return CE_ERR_BACKEND_REJECTED; /* the plane keeps its buffer */
		}
		b->safety_pa = s->pa;
	}
	key = b->port->irq_lock();

	/* The pending submission, if the hardware took it before these writes
	 * were complete, was on the panel: report it rather than replace it.
	 */
	note_latch(b);
	b->pending = true;
	b->pending_sub = submission;
	b->pending_count = b->hw->commit_count(b->dev);
	b->port->irq_unlock(key);
	return CE_OK;
}

static bool twoplane_poll_event(void *ctx, ce_backend_event *out)
{
	struct ce_twoplane *b = ctx;
	bool have;
	unsigned int key = b->port->irq_lock();

	/* the latch already noted first: it is the older one */
	note_latch(b);
	have = b->latched;
	if (have) {
		*out = (ce_backend_event){
			.submission = b->latched_sub, .output = 0, .kind = CE_BACKEND_LATCHED};
		b->latched = false;
		/* the pending submission may have latched behind it */
		note_latch(b);
	}
	b->port->irq_unlock(key);
	return have;
}

const ce_backend_ops ce_twoplane_ops = {
	.capabilities = twoplane_capabilities,
	.check = twoplane_check,
	.commit = twoplane_commit,
	.poll_event = twoplane_poll_event,
};

int ce_twoplane_init(struct ce_twoplane *b, const struct ce_twoplane_hw *hw,
		     const struct ce_twoplane_port *port, const void *dev)
{
	uint32_t stride;

	if (!port->device_ready(dev)) {
		return -CE_TWOPLANE_ENODEV;
	}
	/* The safety plane already scans the driver's front buffer: a frame that
	 * names it needs no flip.
	 */
	*b = (struct ce_twoplane){.hw = hw,
				  .port = port,
				  .dev = dev,
				  .safety_pa = (uintptr_t)hw->safety_front(dev)};
	(void)hw->safety_buffer(dev, &b->width, &b->height, &stride);
	/* the safety plane's blend mode, as its capabilities say (Slint's
	 * software renderer writes premultiplied pixels) */
	(void)hw->safety_premultiplied(dev, true);
	hw->set_vsync_callback(dev, vsync_cb, b);
	return 0;
}
