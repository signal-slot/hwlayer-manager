/*
 * C interface of the hardware composition engine.
 *
 * The engine plans which hardware plane shows which layer and tracks when a
 * buffer may be reused. It never touches hardware itself: the program gives
 * it a backend, a table of four functions that describe and drive the
 * display controller.
 *
 * Nothing here allocates. The caller provides the storage for the engine.
 * The engine is not thread safe: call it from one context, or lock.
 *
 * This header is written by hand and must match crates/composition-ffi/src.
 * ce_abi_check() verifies the structure sizes at run time.
 */
#ifndef COMPOSITION_ENGINE_H_
#define COMPOSITION_ENGINE_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define CE_MAX_OUTPUTS 4
#define CE_MAX_PLANES 16
#define CE_MAX_LAYERS 16
#define CE_MAX_FORMATS 32
#define CE_MAX_BUFFERS 64
#define CE_MAX_RESOURCE_GROUPS 8

/* Storage for one engine: CE_ENGINE_SIZE bytes aligned to CE_ENGINE_ALIGN. */
#define CE_ENGINE_SIZE 20480
#define CE_ENGINE_ALIGN 8

/* DRM-style FourCC pixel formats. */
#define CE_FOURCC(a, b, c, d) \
	((uint32_t)(a) | ((uint32_t)(b) << 8) | ((uint32_t)(c) << 16) | ((uint32_t)(d) << 24))
#define CE_FORMAT_RGB565 CE_FOURCC('R', 'G', '1', '6')
#define CE_FORMAT_XRGB8888 CE_FOURCC('X', 'R', '2', '4')
#define CE_FORMAT_ARGB8888 CE_FOURCC('A', 'R', '2', '4')
#define CE_FORMAT_NV12 CE_FOURCC('N', 'V', '1', '2')
#define CE_MODIFIER_LINEAR 0ULL

#define CE_ALPHA_OPAQUE 0xFFFF

/* Status codes. Zero is success, everything else is negative. */
typedef int32_t ce_status;
#define CE_OK 0
#define CE_ERR_INVALID_ARGUMENT (-1)
/* The frame cannot be mapped onto the hardware. */
#define CE_ERR_UNKNOWN_OUTPUT (-10)
#define CE_ERR_TOO_MANY_LAYERS (-11)
#define CE_ERR_UNKNOWN_BUFFER (-12)
#define CE_ERR_INVALID_SOURCE (-13)
#define CE_ERR_INVALID_DESTINATION (-14)
#define CE_ERR_NO_PLANE (-15)
#define CE_ERR_BACKEND_REJECTED (-16)
#define CE_ERR_SEARCH_LIMIT (-17)
#define CE_ERR_CANNOT_CLIP (-18)
/* The buffer or submission lifecycle forbids the operation. */
#define CE_ERR_DUPLICATE_BUFFER (-30)
#define CE_ERR_BUFFER_TABLE_FULL (-31)
#define CE_ERR_BUFFER_IN_USE (-32)
#define CE_ERR_BUFFER_NOT_READY (-33)
#define CE_ERR_BUFFER_NOT_WRITABLE (-34)
#define CE_ERR_BUSY (-35)
#define CE_ERR_TOO_MANY_OUTPUTS (-36)
#define CE_ERR_TOO_MANY_BUFFERS (-37)
#define CE_ERR_EVENT_QUEUE_FULL (-38)
#define CE_ERR_UNEXPECTED_LATCH (-39)
/* The backend failed. */
#define CE_ERR_BACKEND_BUSY (-50)
#define CE_ERR_DEVICE_LOST (-51)
#define CE_ERR_BACKEND_PLATFORM (-52)
#define CE_ERR_BACKEND_UNKNOWN_BUFFER (-53)

/* Why no plane could take a layer (ce_error_info.reasons, a bit set). */
#define CE_REJECT_OUTPUT (1u << 0)
#define CE_REJECT_FORMAT (1u << 1)
#define CE_REJECT_SCALING (1u << 2)
#define CE_REJECT_SOURCE_SIZE (1u << 3)
#define CE_REJECT_ALPHA (1u << 4)
#define CE_REJECT_BLEND (1u << 5)
#define CE_REJECT_TRANSFORM (1u << 6)
#define CE_REJECT_FULL_OUTPUT (1u << 7)
#define CE_REJECT_Z_ORDER (1u << 8)
#define CE_REJECT_RESOURCE (1u << 9)
#define CE_REJECT_IN_USE (1u << 10)

enum { CE_BLEND_OPAQUE = 0, CE_BLEND_PREMULTIPLIED = 1, CE_BLEND_COVERAGE = 2 };
enum {
	CE_TRANSFORM_NORMAL = 0,
	CE_TRANSFORM_ROTATE_90 = 1,
	CE_TRANSFORM_ROTATE_180 = 2,
	CE_TRANSFORM_ROTATE_270 = 3,
	CE_TRANSFORM_FLIP_H = 4,
	CE_TRANSFORM_FLIP_V = 5,
};
enum { CE_PLANE_PRIMARY = 0, CE_PLANE_OVERLAY = 1, CE_PLANE_CURSOR = 2 };

/* Layer traits that consume a shared resource (ce_resource_group.consumers). */
#define CE_TRAIT_ANY (1u << 0)
#define CE_TRAIT_SCALED_H (1u << 1)
#define CE_TRAIT_SCALED_V (1u << 2)
#define CE_TRAIT_YUV (1u << 3)

typedef struct ce_rect {
	int32_t x;
	int32_t y;
	uint32_t width;
	uint32_t height;
} ce_rect;

/* Immutable description of a registered buffer. */
typedef struct ce_buffer_info {
	uint64_t modifier;
	uint32_t width;
	uint32_t height;
	uint32_t format;
} ce_buffer_info;

typedef struct ce_layer {
	uint32_t buffer;
	ce_rect source;      /* in buffer pixels */
	ce_rect destination; /* in output pixels; clipped to the output */
	int32_t z_order;     /* larger is closer to the viewer */
	uint16_t alpha;      /* CE_ALPHA_OPAQUE for none */
	uint8_t blend;       /* CE_BLEND_* */
	uint8_t transform;   /* CE_TRANSFORM_* */
} ce_layer;

/* The complete content of one output. No layers blanks it. */
typedef struct ce_frame {
	const ce_layer *layers;
	size_t layer_count;
	uint32_t output;
} ce_frame;

/*
 * One layer placed on one plane, as decided by the engine.
 *
 * Program source and destination from here, not from the layer: they are
 * the layer's rectangles after clipping to the output. A layer entirely off
 * the output has no assignment.
 */
typedef struct ce_assignment {
	uint32_t layer;       /* index into ce_frame.layers */
	uint32_t plane;       /* ce_plane_caps.id */
	uint32_t plane_index; /* index into ce_capabilities.planes */
	int32_t zpos;         /* stacking position to program */
	ce_rect source;
	ce_rect destination;
} ce_assignment;

typedef struct ce_format {
	uint64_t modifier;
	uint32_t format;
} ce_format;

typedef struct ce_plane_caps {
	ce_format formats[CE_MAX_FORMATS];
	uint32_t format_count;
	uint32_t id;
	uint32_t outputs;           /* bit mask over ce_capabilities.outputs */
	uint32_t max_upscale;       /* 16.16 fixed point; used if can_scale */
	uint32_t max_downscale;     /* 16.16 fixed point; used if can_scale */
	uint32_t max_source_width;  /* 0: unlimited */
	uint32_t max_source_height; /* 0: unlimited */
	int32_t zpos_min;           /* the fixed position if !zpos_mutable */
	int32_t zpos_max;
	uint8_t kind;               /* CE_PLANE_* */
	uint8_t can_scale;
	uint8_t plane_alpha;
	uint8_t blend_modes;        /* bit mask: 1 << CE_BLEND_* */
	uint8_t transforms;         /* bit mask: 1 << CE_TRANSFORM_* */
	uint8_t zpos_mutable;
	uint8_t full_output_only;
} ce_plane_caps;

typedef struct ce_output_caps {
	uint32_t id;
	uint32_t width;
	uint32_t height;
	uint32_t refresh_mhz; /* 0: unknown */
} ce_output_caps;

/* A resource shared by planes; see CE_TRAIT_*. */
typedef struct ce_resource_group {
	uint32_t planes; /* bit mask over ce_capabilities.planes */
	uint8_t consumers;
	uint8_t capacity;
} ce_resource_group;

/* What the display controller can do. Report only what really works. */
typedef struct ce_capabilities {
	ce_plane_caps planes[CE_MAX_PLANES];
	ce_output_caps outputs[CE_MAX_OUTPUTS];
	ce_resource_group groups[CE_MAX_RESOURCE_GROUPS];
	uint32_t plane_count;
	uint32_t output_count;
	uint32_t group_count;
	/* Nonzero if a commit may replace one that is not latched yet. */
	uint8_t replace_pending;
} ce_capabilities;

enum {
	CE_BACKEND_LATCHED = 0,              /* output, submission */
	CE_BACKEND_DEVICE_LOST = 1,
	CE_BACKEND_CAPABILITIES_CHANGED = 2,
};

typedef struct ce_backend_event {
	uint64_t submission;
	uint32_t output;
	uint8_t kind;
} ce_backend_event;

/*
 * The display backend. All functions are called with the ctx pointer given
 * to ce_engine_init, from within engine calls only.
 *
 * check and commit return CE_OK, or one of CE_ERR_BACKEND_REJECTED (this
 * assignment does not work, another might), CE_ERR_BACKEND_BUSY,
 * CE_ERR_DEVICE_LOST, CE_ERR_BACKEND_UNKNOWN_BUFFER. Any other value is
 * passed on as a platform error.
 */
typedef struct ce_backend_ops {
	/* Fill *out, which is zeroed beforehand. */
	void (*capabilities)(void *ctx, ce_capabilities *out);
	/* Would the hardware take this? Must not change the display. */
	ce_status (*check)(void *ctx, const ce_frame *frame, const ce_assignment *assignments,
			   size_t count);
	/*
	 * Program the frame. Success means accepted, not shown: report
	 * CE_BACKEND_LATCHED with the same submission once the hardware has
	 * latched it. A commit replaced before its latch must never be
	 * reported as latched, unless the hardware latched it before this
	 * commit took it over (a vertical blank while the new frame was being
	 * written): that latch must be ready for poll_event by the time
	 * commit returns, and the engine records it before the new frame.
	 */
	ce_status (*commit)(void *ctx, uint64_t submission, const ce_frame *frame,
			    const ce_assignment *assignments, size_t count);
	/* Store the next event in *out and return true, or return false. */
	bool (*poll_event)(void *ctx, ce_backend_event *out);
} ce_backend_ops;

enum {
	CE_BUFFER_UNKNOWN = -1,
	CE_BUFFER_WRITABLE = 0,  /* the producer may draw */
	CE_BUFFER_READY = 1,     /* complete and idle */
	CE_BUFFER_SUBMITTED = 2, /* accepted, not latched yet */
	CE_BUFFER_SCANNING = 3,  /* on screen */
};

enum {
	CE_EVENT_PRESENTED = 0,            /* output, submission: now on screen */
	CE_EVENT_DROPPED = 1,              /* output, submission: never shown */
	CE_EVENT_RELEASED = 2,             /* buffer: may be reused */
	CE_EVENT_DEVICE_LOST = 3,
	CE_EVENT_CAPABILITIES_CHANGED = 4,
	/*
	 * output, submission: no latch within the time set by
	 * ce_set_latch_timeout. Reported once. The submission stays pending
	 * and its buffers stay in use; recovery is up to the caller.
	 */
	CE_EVENT_LATCH_TIMEOUT = 5,
};

typedef struct ce_event {
	uint64_t submission;
	uint32_t output;
	uint32_t buffer;
	uint8_t kind;
} ce_event;

/* Details of the last failure of a call that takes one. */
typedef struct ce_error_info {
	ce_status status;
	uint32_t layer;   /* layer index, for layer-related plan errors */
	uint32_t reasons; /* CE_REJECT_*, for CE_ERR_NO_PLANE */
	uint32_t buffer;  /* buffer id, for buffer-related errors */
	int32_t platform; /* backend code, for CE_ERR_BACKEND_PLATFORM */
} ce_error_info;

typedef struct ce_engine ce_engine;

/*
 * Returns true if this header and the library agree on structure sizes.
 * Call it once at start-up with CE_ABI_CHECK().
 */
bool ce_abi_check(size_t capabilities, size_t layer, size_t frame, size_t event,
		  size_t backend_ops, size_t assignment, size_t engine_size);
#define CE_ABI_CHECK() \
	ce_abi_check(sizeof(ce_capabilities), sizeof(ce_layer), sizeof(ce_frame), \
		     sizeof(ce_event), sizeof(ce_backend_ops), sizeof(ce_assignment), \
		     CE_ENGINE_SIZE)

/*
 * Creates an engine in caller-provided storage of at least CE_ENGINE_SIZE
 * bytes aligned to CE_ENGINE_ALIGN. Returns NULL if the storage is too
 * small or misaligned, or an argument is NULL. *ops is copied; ctx must
 * stay valid for the life of the engine. There is nothing to destroy.
 */
ce_engine *ce_engine_init(void *storage, size_t size, const ce_backend_ops *ops, void *ctx);

/* Registers a buffer. It starts out writable. */
ce_status ce_register_buffer(ce_engine *engine, uint32_t buffer, const ce_buffer_info *info);
/* Removes a buffer that is neither submitted nor on screen. */
ce_status ce_unregister_buffer(ce_engine *engine, uint32_t buffer);
/* Hands an idle buffer to the producer. Fails while it is in use. */
ce_status ce_begin_write(ce_engine *engine, uint32_t buffer);
/* The producer finished drawing: the buffer may be shown. */
ce_status ce_finish_write(ce_engine *engine, uint32_t buffer);
/* CE_BUFFER_* */
int32_t ce_buffer_state(const ce_engine *engine, uint32_t buffer);

/*
 * Plans and submits a frame. On success *submission (if not NULL) receives
 * its identifier; CE_EVENT_PRESENTED follows once it is on screen. On
 * failure nothing changed and *info (if not NULL) says why.
 */
ce_status ce_present(ce_engine *engine, const ce_frame *frame, uint64_t *submission,
		     ce_error_info *info);

/*
 * Takes the next event. Returns 1 and fills *event, 0 if there is none, or
 * a negative status if the backend reported something inconsistent.
 * Call it regularly: the engine only learns about latches here.
 */
int32_t ce_poll_event(ce_engine *engine, ce_event *event);

/*
 * The engine has no clock. To have overdue latches reported, set a timeout
 * (in ticks of your choice, 0 turns it off) and tell the engine the time
 * regularly, typically before ce_poll_event. Time must not go backwards.
 */
ce_status ce_set_latch_timeout(ce_engine *engine, uint64_t ticks);
ce_status ce_advance_time(ce_engine *engine, uint64_t now);

#ifdef __cplusplus
}
#endif

#endif /* COMPOSITION_ENGINE_H_ */
