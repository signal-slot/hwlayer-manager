/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * docs/trace-format.md (CETRACE v=1) lines, written from C.
 *
 * The C ABI of the engine has no trace hook, so the application writes the
 * lines itself from what it sees around ce_present() and ce_poll_event(),
 * in the field order of crates/composition-trace/src/format.rs. main.c
 * explains which call produces which line.
 */
#ifndef IMX95_M7_SCENE_TRACE_H_
#define IMX95_M7_SCENE_TRACE_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "composition_engine.h"

/* Reads the trace clock: the raw counter value written as t= (and t0=). */
uint64_t trace_now(void);
/*
 * Ticks from a to b, both trace_now() values. Modulo 2^32 for the 32-bit
 * clock, so it survives one wrap.
 */
uint64_t trace_ticks(uint64_t a, uint64_t b);
/*
 * A non-decreasing 64-bit version of the clock for ce_advance_time(). For
 * the 32-bit clock it must be called at least once per counter wrap.
 */
uint64_t trace_engine_time(void);
/* Ticks per second of the trace clock, read from the running system. */
uint32_t trace_freq_hz(void);
/* "cyc64" or "cyc32". */
const char *trace_clock_name(void);

/* The at= value of the following lines: the last ce_advance_time() value. */
void trace_set_at(uint64_t engine_time);

void trace_hdr(const char *build, const char *board);
void trace_check(uint64_t t, uint32_t out, const ce_assignment *a, size_t n, ce_status result);
void trace_validate_ok(uint64_t t, uint32_t out, size_t layers, const ce_assignment *a, size_t n,
		       uint64_t dur);
/* Returns false (and writes nothing) if the status has no trace code. */
bool trace_validate_err(uint64_t t, uint32_t out, size_t layers, const ce_error_info *info,
			uint32_t candidates, uint64_t dur);
void trace_commit_ok(uint64_t t, uint32_t out, uint64_t sub, uint64_t dur);
/* Returns false (and writes nothing) if the status has no trace code. */
bool trace_commit_err(uint64_t t, uint32_t out, const ce_error_info *info, uint64_t dur);
void trace_latched(uint64_t t, uint32_t out, uint64_t sub);
void trace_released(uint64_t t, uint32_t buf, bool has_sub, uint64_t sub);
void trace_dropped(uint64_t t, uint32_t out, uint64_t sub);
void trace_latch_timeout(uint64_t t, uint32_t out, uint64_t sub);
void trace_device_lost(uint64_t t);
void trace_caps_changed(uint64_t t);

/* Prints the buffered lines, if any (CONFIG_IMX95_M7_SCENE_TRACE_BUFFER_KB). */
void trace_flush(void);
/* Lines that did not fit into the buffer; each one is a gap in seq=. */
uint32_t trace_lost(void);
/* true if lines go to a buffer and are printed by trace_flush(). */
bool trace_buffered(void);

#endif /* IMX95_M7_SCENE_TRACE_H_ */
