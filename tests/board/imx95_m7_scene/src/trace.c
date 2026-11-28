/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * CETRACE v=1 line writer (docs/trace-format.md). Field order and error
 * codes follow crates/composition-trace/src/format.rs, so the strict parser
 * of that crate reads these lines.
 */
#include "trace.h"

#include <zephyr/kernel.h>
#include <zephyr/sys/printk.h>
#include <zephyr/sys/util.h>

#include <inttypes.h>
#include <stdarg.h>
#include <string.h>

#if defined(CONFIG_TIMER_HAS_64BIT_CYCLE_COUNTER)
#define CLOCK_64 1
#else
#define CLOCK_64 0
#endif

/* One line; 16 assignments of about 60 characters fit. */
#define LINE_MAX 1280

static uint64_t seq;
static uint64_t at;
static uint32_t lost;
static char line[LINE_MAX];
static size_t line_len;

#if CONFIG_IMX95_M7_SCENE_TRACE_BUFFER_KB > 0
static char ring[CONFIG_IMX95_M7_SCENE_TRACE_BUFFER_KB * 1024];
static size_t ring_len;
#endif

uint64_t trace_now(void)
{
#if CLOCK_64
	return k_cycle_get_64();
#else
	return k_cycle_get_32();
#endif
}

uint64_t trace_ticks(uint64_t a, uint64_t b)
{
#if CLOCK_64
	return b - a;
#else
	return (uint32_t)((uint32_t)b - (uint32_t)a);
#endif
}

uint64_t trace_engine_time(void)
{
#if CLOCK_64
	return k_cycle_get_64();
#else
	static uint64_t ext;
	static uint32_t last;
	uint32_t now = k_cycle_get_32();

	ext += (uint32_t)(now - last);
	last = now;
	return ext;
#endif
}

uint32_t trace_freq_hz(void)
{
	return sys_clock_hw_cycles_per_sec();
}

const char *trace_clock_name(void)
{
	return CLOCK_64 ? "cyc64" : "cyc32";
}

void trace_set_at(uint64_t engine_time)
{
#if CLOCK_64
	at = engine_time;
#else
	/* at= is on the same raw 32-bit counter as t= (at_unit=same). */
	at = (uint32_t)engine_time;
#endif
}

static void put(const char *fmt, ...)
{
	va_list ap;
	int n;

	if (line_len >= sizeof(line)) {
		return;
	}
	va_start(ap, fmt);
	n = vsnprintk(line + line_len, sizeof(line) - line_len, fmt, ap);
	va_end(ap);
	line_len = n < 0 ? sizeof(line) : MIN(line_len + (size_t)n, sizeof(line));
}

static void begin(const char *ev, uint64_t t)
{
	line_len = 0;
	put("CETRACE v=1 ev=%s seq=%" PRIu64 " t=%" PRIu64 " at=%" PRIu64, ev, seq, t, at);
}

/* Ends the line. Every line takes a seq, also one that is lost. */
static void end(void)
{
	seq++;
	if (line_len + 1 >= sizeof(line)) {
		/* Truncated: never write a partial line. */
		lost++;
		return;
	}
	line[line_len++] = '\n';
	line[line_len] = '\0';
#if CONFIG_IMX95_M7_SCENE_TRACE_BUFFER_KB > 0
	if (ring_len + line_len > sizeof(ring)) {
		lost++;
		return;
	}
	memcpy(ring + ring_len, line, line_len);
	ring_len += line_len;
#else
	printk("%s", line);
#endif
}

static void put_rect(const ce_rect *r)
{
	put("%" PRId32 ",%" PRId32 ",%" PRIu32 ",%" PRIu32, r->x, r->y, r->width, r->height);
}

static void put_assignments(const ce_assignment *a, size_t n)
{
	put(" n=%u assign=", (unsigned int)n);
	if (n == 0) {
		put("-");
		return;
	}
	for (size_t i = 0; i < n; i++) {
		put("%s%" PRIu32 ":%" PRIu32 ":%" PRIu32 ":%" PRId32 ":", i == 0 ? "" : ";",
		    a[i].layer, a[i].plane, a[i].plane_index, a[i].zpos);
		put_rect(&a[i].source);
		put(":");
		put_rect(&a[i].destination);
	}
}

/*
 * A BackendError, as a check result ("rejected") or nested in an Error
 * ("backend.rejected"). Returns false for a status that is not one.
 */
static bool put_backend_error(const ce_error_info *e, const char *prefix)
{
	switch (e->status) {
	case CE_ERR_BACKEND_REJECTED:
		put("%srejected", prefix);
		return true;
	case CE_ERR_BACKEND_BUSY:
		put("%sbusy", prefix);
		return true;
	case CE_ERR_BACKEND_UNKNOWN_BUFFER:
		put("%sunknown_buffer buf=%" PRIu32, prefix, e->buffer);
		return true;
	case CE_ERR_DEVICE_LOST:
		put("%sdevice_lost", prefix);
		return true;
	case CE_ERR_BACKEND_PLATFORM:
		put("%splatform errno=%" PRId32, prefix, e->platform);
		return true;
	default:
		return false;
	}
}

void trace_hdr(const char *build, const char *board)
{
	line_len = 0;
	put("CETRACE v=1 ev=hdr seq=%" PRIu64 " src=zephyr clock=%s freq_hz=%" PRIu32
	    " t0=%" PRIu64 " at_unit=same build=\"%s\" board=\"%s\"",
	    seq, trace_clock_name(), trace_freq_hz(), trace_now(), build, board);
	end();
}

void trace_check(uint64_t t, uint32_t out, const ce_assignment *a, size_t n, ce_status result)
{
	ce_error_info e = { .status = result };

	begin("check", t);
	put(" out=%" PRIu32, out);
	put_assignments(a, n);
	put(" result=");
	if (result == CE_OK) {
		put("ok");
	} else if (!put_backend_error(&e, "")) {
		/* The backend returns its own errno-style codes as platform errors. */
		put("platform errno=%" PRId32, result);
	}
	end();
}

void trace_validate_ok(uint64_t t, uint32_t out, size_t layers, const ce_assignment *a, size_t n,
		       uint64_t dur)
{
	begin("validate", t);
	put(" out=%" PRIu32 " layers=%u", out, (unsigned int)layers);
	put_assignments(a, n);
	put(" result=ok dur=%" PRIu64, dur);
	end();
}

bool trace_validate_err(uint64_t t, uint32_t out, size_t layers, const ce_error_info *info,
			uint32_t candidates, uint64_t dur)
{
	begin("validate", t);
	put(" out=%" PRIu32 " layers=%u result=", out, (unsigned int)layers);
	switch (info->status) {
	case CE_ERR_UNKNOWN_OUTPUT:
		put("plan.unknown_output err_out=%" PRIu32, out);
		break;
	case CE_ERR_TOO_MANY_LAYERS:
		put("plan.too_many_layers count=%u", (unsigned int)layers);
		break;
	case CE_ERR_UNKNOWN_BUFFER:
		put("plan.unknown_buffer layer=%" PRIu32 " buf=%" PRIu32, info->layer, info->buffer);
		break;
	case CE_ERR_INVALID_SOURCE:
		put("plan.invalid_source layer=%" PRIu32, info->layer);
		break;
	case CE_ERR_INVALID_DESTINATION:
		put("plan.invalid_destination layer=%" PRIu32, info->layer);
		break;
	case CE_ERR_CANNOT_CLIP:
		put("plan.cannot_clip layer=%" PRIu32, info->layer);
		break;
	case CE_ERR_NO_PLANE:
		put("plan.no_plane layer=%" PRIu32 " reasons=0x%" PRIx32, info->layer,
		    info->reasons);
		break;
	case CE_ERR_BACKEND_REJECTED:
		/* Every candidate was rejected by check. */
		put("plan.backend_rejected candidates=%" PRIu32, candidates);
		break;
	case CE_ERR_SEARCH_LIMIT:
		put("plan.limit_exceeded");
		break;
	default:
		/* A check that failed with something else ends the search. */
		if (!put_backend_error(info, "backend.")) {
			line_len = 0;
			return false;
		}
		break;
	}
	put(" dur=%" PRIu64, dur);
	end();
	return true;
}

void trace_commit_ok(uint64_t t, uint32_t out, uint64_t sub, uint64_t dur)
{
	begin("commit", t);
	put(" out=%" PRIu32 " result=ok sub=%" PRIu64 " dur=%" PRIu64, out, sub, dur);
	end();
}

bool trace_commit_err(uint64_t t, uint32_t out, const ce_error_info *info, uint64_t dur)
{
	begin("commit", t);
	put(" out=%" PRIu32 " result=", out);
	switch (info->status) {
	case CE_ERR_UNKNOWN_BUFFER:
		put("present.unknown_buffer buf=%" PRIu32, info->buffer);
		break;
	case CE_ERR_DUPLICATE_BUFFER:
		put("present.duplicate_buffer buf=%" PRIu32, info->buffer);
		break;
	case CE_ERR_BUFFER_TABLE_FULL:
		put("present.buffer_table_full");
		break;
	case CE_ERR_BUFFER_IN_USE:
		put("present.buffer_in_use buf=%" PRIu32, info->buffer);
		break;
	case CE_ERR_BUFFER_NOT_READY:
		put("present.buffer_not_ready buf=%" PRIu32, info->buffer);
		break;
	case CE_ERR_BUFFER_NOT_WRITABLE:
		put("present.buffer_not_writable buf=%" PRIu32, info->buffer);
		break;
	case CE_ERR_BUSY:
		/* The engine only presents one frame per output at a time. */
		put("present.busy err_out=%" PRIu32, out);
		break;
	case CE_ERR_TOO_MANY_OUTPUTS:
		put("present.too_many_outputs");
		break;
	case CE_ERR_TOO_MANY_BUFFERS:
		put("present.too_many_buffers");
		break;
	case CE_ERR_EVENT_QUEUE_FULL:
		put("present.event_queue_full");
		break;
	default:
		/*
		 * Not representable: CE_ERR_UNEXPECTED_LATCH (the C ABI does
		 * not say which output and submission), CE_ERR_INVALID_ARGUMENT,
		 * and anything unknown. The caller prints a plain line instead.
		 */
		if (!put_backend_error(info, "backend.")) {
			line_len = 0;
			return false;
		}
		break;
	}
	put(" dur=%" PRIu64, dur);
	end();
	return true;
}

void trace_latched(uint64_t t, uint32_t out, uint64_t sub)
{
	begin("latched", t);
	put(" out=%" PRIu32 " sub=%" PRIu64, out, sub);
	end();
}

void trace_released(uint64_t t, uint32_t buf, bool has_sub, uint64_t sub)
{
	begin("released", t);
	if (has_sub) {
		put(" buf=%" PRIu32 " sub=%" PRIu64, buf, sub);
	} else {
		put(" buf=%" PRIu32 " sub=-", buf);
	}
	end();
}

void trace_dropped(uint64_t t, uint32_t out, uint64_t sub)
{
	begin("dropped", t);
	put(" out=%" PRIu32 " sub=%" PRIu64 " reason=replaced", out, sub);
	end();
}

void trace_latch_timeout(uint64_t t, uint32_t out, uint64_t sub)
{
	begin("latch_timeout", t);
	put(" out=%" PRIu32 " sub=%" PRIu64, out, sub);
	end();
}

void trace_device_lost(uint64_t t)
{
	begin("device_lost", t);
	end();
}

void trace_caps_changed(uint64_t t)
{
	begin("caps_changed", t);
	end();
}

void trace_flush(void)
{
#if CONFIG_IMX95_M7_SCENE_TRACE_BUFFER_KB > 0
	size_t start = 0;

	for (size_t i = 0; i < ring_len; i++) {
		if (ring[i] == '\n') {
			printk("%.*s\n", (int)(i - start), ring + start);
			start = i + 1;
		}
	}
	ring_len = 0;
#endif
}

uint32_t trace_lost(void)
{
	return lost;
}

bool trace_buffered(void)
{
	return CONFIG_IMX95_M7_SCENE_TRACE_BUFFER_KB > 0;
}
