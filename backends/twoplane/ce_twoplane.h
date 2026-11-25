/* SPDX-License-Identifier: Apache-2.0 */
/*
 * The composition engine's backend (composition_engine.h) for a display
 * driver with two planes on one output, the AM62P DSS and the i.MX 95 DPU
 * alike:
 *
 *   plane 0  the application's frame, RGB565/XRGB8888/ARGB8888 as the driver
 *            takes them, opaque, the whole panel, at the bottom
 *   plane 1  the safety layer, ARGB8888 premultiplied, the whole panel, on
 *            top; its two buffers are the driver's
 *
 * The driver is reached through struct ce_twoplane_hw, which each SoC fills
 * with its calls (display_port_<soc>.c in the demo firmware). What the
 * backend needs of them: every call that writes the planes is one commit;
 * the writes of a commit reach the panel together at a frame start, with any
 * later writes made before that frame start; commit_count, read after a
 * commit's writes, is reached by latched_count once they have. A commit is
 * reported latched then. It writes only what changed (the application
 * buffer, the plane's enable, a safety flip).
 *
 * The backend needs no operating system and no C library beyond
 * <stdbool.h>, <stddef.h> and <stdint.h>. What it needs of the system (the
 * interrupt lock, the wake-up at every vsync, whether the device is ready)
 * and the integrator's table of registered buffers come through struct
 * ce_twoplane_port. The device is an opaque pointer that the backend only
 * hands back to the driver's and the port's calls (a const struct device *
 * under Zephyr).
 */
#ifndef CE_TWOPLANE_H_
#define CE_TWOPLANE_H_

#include <stdbool.h>
#include <stdint.h>
#include <composition_engine.h>

#define CE_TWOPLANE_PLANE_APP    0u
#define CE_TWOPLANE_PLANE_SAFETY 1u

/* ce_twoplane_init: the device is not ready (-ENODEV of Zephyr, newlib and
 * Linux).
 */
#define CE_TWOPLANE_ENODEV 19

/* A buffer for the application plane, as the engine registered it. */
struct ce_twoplane_buf {
	uint32_t seq;          /* frame number, reported back as shown */
	uintptr_t pa;
	uint32_t width;
	uint32_t height;
	uint32_t stride_bytes;
	uint32_t format;       /* CE_FORMAT_* */
};

/* A buffer of the integrator's table, by its engine id (ce_layer.buffer). */
struct ce_twoplane_record {
	uint32_t id;
	uint32_t seq;          /* frame number, reported back as shown */
	uintptr_t pa;          /* the address the plane scans */
	uint32_t width;
	uint32_t height;
	uint32_t stride_bytes;
	uint32_t format;       /* CE_FORMAT_* */
};

typedef void (*ce_twoplane_vsync_cb_t)(const void *dev, void *user);

struct ce_twoplane_hw {
	/* Whether the application plane takes this CE_FORMAT_*. */
	bool (*app_format_ok)(uint32_t format);
	/* Points the application plane at buf; nonzero: refused, nothing written. */
	int (*set_app_buffer)(const void *dev, const struct ce_twoplane_buf *buf);
	int (*enable_app_plane)(const void *dev, bool enable);
	/* Presents safety_buffer() at the next frame start. */
	int (*safety_flip)(const void *dev);
	void *(*safety_buffer)(const void *dev, uint32_t *width, uint32_t *height,
			       uint32_t *stride_bytes);
	void *(*safety_front)(const void *dev);
	int (*safety_premultiplied)(const void *dev, bool on);
	uint32_t (*commit_count)(const void *dev);
	uint32_t (*latched_count)(const void *dev);
	/* cb runs in the driver's interrupt once a frame. */
	void (*set_vsync_callback)(const void *dev, ce_twoplane_vsync_cb_t cb, void *user);
};

/* What the backend needs of the system, filled by the integrator. Every
 * member is required.
 */
struct ce_twoplane_port {
	/* Keeps the driver's vsync callback from running until irq_unlock
	 * (Zephyr: irq_lock). Returns what irq_unlock takes. The backend does
	 * not nest them and calls them from the engine's context only.
	 */
	unsigned int (*irq_lock)(void);
	void (*irq_unlock)(unsigned int key);
	/* Called with wake_arg from the vsync callback at every vsync, to wake
	 * whoever polls the engine (Zephyr: k_sem_give).
	 */
	void (*wake)(void *wake_arg);
	void *wake_arg;
	/* The registered buffer with this id, or NULL (the demo: fw_comp_get).
	 * Called from the engine's context only; the record is read during the
	 * engine call that looked it up.
	 */
	const struct ce_twoplane_record *(*lookup)(void *lookup_arg, uint32_t id);
	void *lookup_arg;
	/* Whether dev can be used (Zephyr: device_is_ready). */
	bool (*device_ready)(const void *dev);
};

struct ce_twoplane {
	const struct ce_twoplane_hw *hw;
	const struct ce_twoplane_port *port;
	const void *dev;
	uint32_t width;         /* the panel */
	uint32_t height;
	/* The planes as the commits left them (engine lock). */
	uint32_t app_id;        /* buffer on the application plane */
	uint32_t app_seq;       /* ... and its frame number */
	bool app_on;
	uintptr_t safety_pa;    /* the safety buffer last flipped to, 0: none */
	/* Shared with the vsync interrupt (port->irq_lock). */
	bool pending;           /* a commit waits for its latch */
	uint64_t pending_sub;
	uint32_t pending_count; /* hw->commit_count() after its writes */
	bool latched;           /* a latch to report */
	uint64_t latched_sub;
	uint32_t vsyncs;
};

/* The backend for ce_engine_init, with a struct ce_twoplane as its ctx. */
extern const ce_backend_ops ce_twoplane_ops;

/* Takes the driver's vsync callback. port->wake is called at every vsync.
 * hw and port must outlive b. Returns 0, or -CE_TWOPLANE_ENODEV if
 * port->device_ready(dev) says no (nothing is called on the driver then).
 */
int ce_twoplane_init(struct ce_twoplane *b, const struct ce_twoplane_hw *hw,
		     const struct ce_twoplane_port *port, const void *dev);

#endif /* CE_TWOPLANE_H_ */
