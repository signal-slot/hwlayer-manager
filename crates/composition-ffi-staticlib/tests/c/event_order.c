/*
 * Adversarial event ordering through the C interface, as a real C caller
 * sees it. The backend is written in C and scripted by the test: it queues
 * whatever events the case wants, in whatever order, including ones no sane
 * hardware would produce.
 *
 * Usage: event_order            run every case that is expected to pass
 *        event_order <case>     run one case (names are listed in cases[])
 *
 * Exit status 0 means every check passed. Cases marked as known bugs in
 * cases[] record behaviour that contradicts the header; they run only when
 * named.
 */
#include "composition_engine.h"

#include <stdio.h>
#include <string.h>

static int failures;

#define CHECK(cond) \
	do { \
		if (!(cond)) { \
			fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__, __LINE__, #cond); \
			failures++; \
		} \
	} while (0)

#define CHECK_EQ(actual, expected) \
	do { \
		long long a_ = (long long)(actual), e_ = (long long)(expected); \
		if (a_ != e_) { \
			fprintf(stderr, "%s:%d: %s is %lld, expected %lld\n", __FILE__, __LINE__, \
				#actual, a_, e_); \
			failures++; \
		} \
	} while (0)

enum { OUT = 7, PLANE_A = 31, PLANE_B = 41 };
enum { BUF_A = 1, BUF_B = 2, BUF_C = 3, BUF_D = 4 };
enum { QUEUE = 64 };

struct script {
	ce_backend_event queue[QUEUE];
	size_t head, tail;
	ce_status commit_status;
	ce_status check_status;
	uint8_t replace_pending;
	uint32_t output_count;
	uint64_t last_submission;
	int commits, checks, capability_calls;
	/* Queue a latch of previous_submission from inside the next commit. */
	bool latch_previous_in_commit;
	/* Queue a device loss from inside the next commit. */
	bool lose_device_in_commit;
	uint64_t previous_submission;
};

static void push(struct script *s, uint8_t kind, uint32_t output, uint64_t submission)
{
	s->queue[s->tail++ % QUEUE] =
		(ce_backend_event){ .submission = submission, .output = output, .kind = kind };
}

static void script_capabilities(void *ctx, ce_capabilities *out)
{
	struct script *s = ctx;
	s->capability_calls++;
	out->output_count = s->output_count;
	out->outputs[0] = (ce_output_caps){ .id = OUT, .width = 640, .height = 480,
					    .refresh_mhz = 60000 };
	out->plane_count = 2;
	out->planes[0] = (ce_plane_caps){
		.id = PLANE_A, .kind = CE_PLANE_PRIMARY, .outputs = 1,
		.format_count = 1, .zpos_min = 0,
		.blend_modes = 1u << CE_BLEND_OPAQUE,
	};
	out->planes[0].formats[0] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_XRGB8888 };
	out->planes[1] = (ce_plane_caps){
		.id = PLANE_B, .kind = CE_PLANE_OVERLAY, .outputs = 1,
		.format_count = 1, .zpos_min = 1,
		.blend_modes = (1u << CE_BLEND_PREMULTIPLIED) | (1u << CE_BLEND_OPAQUE),
	};
	out->planes[1].formats[0] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_ARGB8888 };
	out->replace_pending = s->replace_pending;
}

static ce_status script_check(void *ctx, const ce_frame *frame, const ce_assignment *assignments,
			      size_t count)
{
	struct script *s = ctx;
	(void)frame;
	(void)assignments;
	(void)count;
	s->checks++;
	return s->check_status;
}

static ce_status script_commit(void *ctx, uint64_t submission, const ce_frame *frame,
			       const ce_assignment *assignments, size_t count)
{
	struct script *s = ctx;
	(void)frame;
	(void)assignments;
	(void)count;
	if (s->commit_status != CE_OK) {
		return s->commit_status;
	}
	if (s->latch_previous_in_commit) {
		s->latch_previous_in_commit = false;
		push(s, CE_BACKEND_LATCHED, OUT, s->previous_submission);
	}
	if (s->lose_device_in_commit) {
		s->lose_device_in_commit = false;
		push(s, CE_BACKEND_DEVICE_LOST, 0, 0);
	}
	s->commits++;
	s->last_submission = submission;
	return CE_OK;
}

static bool script_poll_event(void *ctx, ce_backend_event *out)
{
	struct script *s = ctx;
	if (s->head == s->tail) {
		return false;
	}
	*out = s->queue[s->head++ % QUEUE];
	return true;
}

static const ce_backend_ops ops = {
	.capabilities = script_capabilities,
	.check = script_check,
	.commit = script_commit,
	.poll_event = script_poll_event,
};

static _Alignas(CE_ENGINE_ALIGN) unsigned char storage[CE_ENGINE_SIZE];

static const ce_buffer_info xrgb = { CE_MODIFIER_LINEAR, 640, 480, CE_FORMAT_XRGB8888 };
static const ce_buffer_info argb = { CE_MODIFIER_LINEAR, 640, 480, CE_FORMAT_ARGB8888 };

/* A fresh engine over a fresh script; four ready buffers: A, B xrgb; C, D argb. */
static ce_engine *fresh(struct script *s, uint8_t replace_pending)
{
	memset(s, 0, sizeof(*s));
	memset(storage, 0xA5, sizeof(storage));
	s->replace_pending = replace_pending;
	s->output_count = 1;
	ce_engine *engine = ce_engine_init(storage, sizeof(storage), &ops, s);
	if (engine == NULL) {
		fprintf(stderr, "ce_engine_init failed\n");
		failures++;
		return NULL;
	}
	CHECK_EQ(ce_register_buffer(engine, BUF_A, &xrgb), CE_OK);
	CHECK_EQ(ce_register_buffer(engine, BUF_B, &xrgb), CE_OK);
	CHECK_EQ(ce_register_buffer(engine, BUF_C, &argb), CE_OK);
	CHECK_EQ(ce_register_buffer(engine, BUF_D, &argb), CE_OK);
	for (uint32_t b = BUF_A; b <= BUF_D; b++) {
		CHECK_EQ(ce_finish_write(engine, b), CE_OK);
	}
	return engine;
}

static ce_layer layer(uint32_t buffer, int32_t z, uint8_t blend)
{
	return (ce_layer){
		.buffer = buffer,
		.source = { 0, 0, 640, 480 },
		.destination = { 0, 0, 640, 480 },
		.z_order = z,
		.alpha = CE_ALPHA_OPAQUE,
		.blend = blend,
		.transform = CE_TRANSFORM_NORMAL,
	};
}

/* Present one layer of `buffer` on the plane that takes its format. */
static ce_status present_one(ce_engine *engine, uint32_t buffer, uint64_t *submission)
{
	ce_layer l = (buffer == BUF_C || buffer == BUF_D)
			     ? layer(buffer, 1, CE_BLEND_PREMULTIPLIED)
			     : layer(buffer, 0, CE_BLEND_OPAQUE);
	ce_frame frame = { .layers = &l, .layer_count = 1, .output = OUT };
	return ce_present(engine, &frame, submission, NULL);
}

static int32_t poll(ce_engine *engine, ce_event *event)
{
	memset(event, 0xEE, sizeof(*event));
	return ce_poll_event(engine, event);
}

/* Poll until empty or an error; returns the number of events, or the error. */
static int drain(ce_engine *engine)
{
	ce_event event;
	int n = 0;
	for (;;) {
		int32_t r = poll(engine, &event);
		if (r < 0) {
			return r;
		}
		if (r == 0) {
			return n;
		}
		if (++n > 1000) {
			return -1000;
		}
	}
}

/* ---- ordering of latches ------------------------------------------------ */

static void case_latch_before_commit(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t sub = 0;
	if (!engine) {
		return;
	}
	/* A latch for a submission that was never made. */
	push(&s, CE_BACKEND_LATCHED, OUT, 1);
	CHECK_EQ(poll(engine, &event), CE_ERR_UNEXPECTED_LATCH);
	/* The engine is still usable and the bad event is gone. */
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_OK);
	CHECK_EQ(poll(engine, &event), 0);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SUBMITTED);
}

static void case_latch_unknown_submission(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t sub = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_OK);
	push(&s, CE_BACKEND_LATCHED, OUT, sub + 100);
	CHECK_EQ(poll(engine, &event), CE_ERR_UNEXPECTED_LATCH);
	/* The real submission is still pending and latches normally. */
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SUBMITTED);
	push(&s, CE_BACKEND_LATCHED, OUT, sub);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(event.submission, sub);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SCANNING);
}

static void case_latch_wrong_output(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t sub = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_OK);
	push(&s, CE_BACKEND_LATCHED, OUT + 1, sub);
	CHECK_EQ(poll(engine, &event), CE_ERR_UNEXPECTED_LATCH);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SUBMITTED);
}

static void case_latch_duplicated(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t sub = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_OK);
	push(&s, CE_BACKEND_LATCHED, OUT, sub);
	push(&s, CE_BACKEND_LATCHED, OUT, sub);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(poll(engine, &event), CE_ERR_UNEXPECTED_LATCH);
	/* The first latch stands. */
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SCANNING);
	CHECK_EQ(poll(engine, &event), 0);
}

static void case_latch_out_of_order(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t first = 0, second = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &first), CE_OK);
	CHECK_EQ(present_one(engine, BUF_B, &second), CE_OK);
	CHECK(second != first);
	/* The replaced submission is already reported dropped. */
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_DROPPED);
	CHECK_EQ(event.submission, first);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_RELEASED);
	CHECK_EQ(event.buffer, BUF_A);
	/* The old submission latching afterwards is a protocol violation. */
	push(&s, CE_BACKEND_LATCHED, OUT, first);
	CHECK_EQ(poll(engine, &event), CE_ERR_UNEXPECTED_LATCH);
	CHECK_EQ(ce_buffer_state(engine, BUF_B), CE_BUFFER_SUBMITTED);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_READY);
	push(&s, CE_BACKEND_LATCHED, OUT, second);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(event.submission, second);
}

static void case_latch_missing(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 0);
	ce_event event;
	uint64_t first = 0, second = 0;
	if (!engine) {
		return;
	}
	/* Without replace_pending a second commit must wait for the latch. */
	CHECK_EQ(ce_set_latch_timeout(engine, 10), CE_OK);
	CHECK_EQ(ce_advance_time(engine, 100), CE_OK);
	CHECK_EQ(present_one(engine, BUF_A, &first), CE_OK);
	CHECK_EQ(present_one(engine, BUF_B, &second), CE_ERR_BUSY);
	CHECK_EQ(s.commits, 1);
	CHECK_EQ(ce_buffer_state(engine, BUF_B), CE_BUFFER_READY);
	/* No latch comes; the timeout reports it exactly once. */
	CHECK_EQ(ce_advance_time(engine, 105), CE_OK);
	CHECK_EQ(poll(engine, &event), 0);
	CHECK_EQ(ce_advance_time(engine, 115), CE_OK);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_LATCH_TIMEOUT);
	CHECK_EQ(event.submission, first);
	CHECK_EQ(ce_advance_time(engine, 10000), CE_OK);
	CHECK_EQ(poll(engine, &event), 0);
	/* Still pending, buffer still in use. */
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SUBMITTED);
	CHECK_EQ(ce_begin_write(engine, BUF_A), CE_ERR_BUFFER_IN_USE);
	CHECK_EQ(ce_unregister_buffer(engine, BUF_A), CE_ERR_BUFFER_IN_USE);
	CHECK_EQ(present_one(engine, BUF_B, &second), CE_ERR_BUSY);
	/* A late latch still works. */
	push(&s, CE_BACKEND_LATCHED, OUT, first);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SCANNING);
}

static void case_time_backwards(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t sub = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(ce_set_latch_timeout(engine, 10), CE_OK);
	CHECK_EQ(ce_advance_time(engine, 1000), CE_OK);
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_OK);
	/* The header forbids it; the engine must still not crash. */
	(void)ce_advance_time(engine, 5);
	(void)poll(engine, &event);
	(void)ce_advance_time(engine, UINT64_MAX);
	(void)poll(engine, &event);
	(void)ce_advance_time(engine, 0);
	CHECK(drain(engine) >= 0);
	/* Turning the timeout off silences it. */
	CHECK_EQ(ce_set_latch_timeout(engine, 0), CE_OK);
	CHECK_EQ(ce_advance_time(engine, UINT64_MAX), CE_OK);
	CHECK_EQ(drain(engine), 0);
	/* A huge timeout must not overflow into an immediate report. */
	CHECK_EQ(ce_set_latch_timeout(engine, UINT64_MAX), CE_OK);
	CHECK_EQ(ce_advance_time(engine, UINT64_MAX), CE_OK);
	CHECK_EQ(drain(engine), 0);
}

/* ---- replaced pending submissions -------------------------------------- */

static void case_replace_pending(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t first = 0, second = 0, third = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &first), CE_OK);
	CHECK_EQ(present_one(engine, BUF_B, &second), CE_OK);
	CHECK_EQ(present_one(engine, BUF_A, &third), CE_OK);
	/* first dropped + A released; second dropped + B released; no repeat. */
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_DROPPED);
	CHECK_EQ(event.submission, first);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_RELEASED);
	CHECK_EQ(event.buffer, BUF_A);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_DROPPED);
	CHECK_EQ(event.submission, second);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_RELEASED);
	CHECK_EQ(event.buffer, BUF_B);
	CHECK_EQ(poll(engine, &event), 0);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SUBMITTED);
	CHECK_EQ(ce_buffer_state(engine, BUF_B), CE_BUFFER_READY);
	/* The latch of the replaced submission must be refused. */
	push(&s, CE_BACKEND_LATCHED, OUT, second);
	CHECK_EQ(poll(engine, &event), CE_ERR_UNEXPECTED_LATCH);
	push(&s, CE_BACKEND_LATCHED, OUT, third);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(event.submission, third);
}

static void case_replace_same_buffer(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t first = 0, second = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &first), CE_OK);
	/* The same buffer again, replacing its own pending submission. */
	CHECK_EQ(present_one(engine, BUF_A, &second), CE_OK);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_DROPPED);
	CHECK_EQ(event.submission, first);
	/* The buffer is in use by the new submission: no release may be seen. */
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SUBMITTED);
	CHECK_EQ(drain(engine), 0);
	push(&s, CE_BACKEND_LATCHED, OUT, second);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SCANNING);
	CHECK_EQ(drain(engine), 0);
}

static void case_replace_latched_during_commit(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t first = 0, second = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &first), CE_OK);
	/* A vertical blank latches `first` while `second` is being written. */
	s.latch_previous_in_commit = true;
	s.previous_submission = first;
	CHECK_EQ(present_one(engine, BUF_B, &second), CE_OK);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(event.submission, first);
	CHECK_EQ(poll(engine, &event), 0);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SCANNING);
	CHECK_EQ(ce_buffer_state(engine, BUF_B), CE_BUFFER_SUBMITTED);
}

static void case_commit_fails_after_pending(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t first = 0, second = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &first), CE_OK);
	s.commit_status = CE_ERR_BACKEND_BUSY;
	CHECK_EQ(present_one(engine, BUF_B, &second), CE_ERR_BACKEND_BUSY);
	/* Nothing changed: the first is still pending, B untouched. */
	CHECK_EQ(drain(engine), 0);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SUBMITTED);
	CHECK_EQ(ce_buffer_state(engine, BUF_B), CE_BUFFER_READY);
	s.commit_status = CE_OK;
	push(&s, CE_BACKEND_LATCHED, OUT, first);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
}

/* ---- release ordering ---------------------------------------------------- */

static void case_release_sequence(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t first = 0, second = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &first), CE_OK);
	push(&s, CE_BACKEND_LATCHED, OUT, first);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(present_one(engine, BUF_B, &second), CE_OK);
	CHECK_EQ(ce_begin_write(engine, BUF_A), CE_ERR_BUFFER_IN_USE);
	CHECK_EQ(poll(engine, &event), 0);
	push(&s, CE_BACKEND_LATCHED, OUT, second);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_RELEASED);
	CHECK_EQ(event.buffer, BUF_A);
	/* Released exactly once. */
	CHECK_EQ(poll(engine, &event), 0);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_READY);
	/* begin_write is idempotent on an idle buffer; finish_write is not. */
	CHECK_EQ(ce_begin_write(engine, BUF_A), CE_OK);
	CHECK_EQ(ce_begin_write(engine, BUF_A), CE_OK);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_WRITABLE);
	CHECK_EQ(ce_finish_write(engine, BUF_A), CE_OK);
	CHECK_EQ(ce_finish_write(engine, BUF_A), CE_ERR_BUFFER_NOT_WRITABLE);
}

static void case_release_never_for_scanned_buffer(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t sub = 0, again = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_OK);
	push(&s, CE_BACKEND_LATCHED, OUT, sub);
	CHECK_EQ(poll(engine, &event), 1);
	/* Presenting a buffer that is on screen again is allowed. */
	CHECK_EQ(present_one(engine, BUF_A, &again), CE_OK);
	/* Still on screen until the new submission latches. */
	CHECK(ce_buffer_state(engine, BUF_A) == CE_BUFFER_SUBMITTED ||
	      ce_buffer_state(engine, BUF_A) == CE_BUFFER_SCANNING);
	CHECK_EQ(ce_begin_write(engine, BUF_A), CE_ERR_BUFFER_IN_USE);
	push(&s, CE_BACKEND_LATCHED, OUT, again);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	/* The old scan-out of the same buffer must not release it. */
	CHECK_EQ(drain(engine), 0);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SCANNING);
}

static void case_blank_frame_releases(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t sub = 0, blank = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_OK);
	push(&s, CE_BACKEND_LATCHED, OUT, sub);
	CHECK_EQ(poll(engine, &event), 1);
	ce_frame empty = { .layers = NULL, .layer_count = 0, .output = OUT };
	CHECK_EQ(ce_present(engine, &empty, &blank, NULL), CE_OK);
	push(&s, CE_BACKEND_LATCHED, OUT, blank);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_RELEASED);
	CHECK_EQ(event.buffer, BUF_A);
	CHECK_EQ(drain(engine), 0);
}

/* ---- device lost and capability changes ---------------------------------- */

static void case_device_lost_event(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t first = 0, second = 0;
	int saw_lost = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &first), CE_OK);
	push(&s, CE_BACKEND_LATCHED, OUT, first);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(present_one(engine, BUF_B, &second), CE_OK);
	push(&s, CE_BACKEND_DEVICE_LOST, 0, 0);
	/* Everything in flight is forgotten; the caller hears of it once. */
	for (;;) {
		int32_t r = poll(engine, &event);
		CHECK(r >= 0);
		if (r != 1) {
			break;
		}
		CHECK(event.kind != CE_EVENT_PRESENTED);
		if (event.kind == CE_EVENT_DEVICE_LOST) {
			saw_lost++;
		}
	}
	CHECK_EQ(saw_lost, 1);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_READY);
	CHECK_EQ(ce_buffer_state(engine, BUF_B), CE_BUFFER_READY);
	/* A latch of a forgotten submission is a protocol violation. */
	push(&s, CE_BACKEND_LATCHED, OUT, second);
	CHECK_EQ(poll(engine, &event), CE_ERR_UNEXPECTED_LATCH);
	/* A duplicated device-lost is reported again, not a crash. */
	push(&s, CE_BACKEND_DEVICE_LOST, 0, 0);
	push(&s, CE_BACKEND_DEVICE_LOST, 0, 0);
	CHECK(drain(engine) >= 0);
}

static void case_device_lost_then_present(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t sub = 0;
	if (!engine) {
		return;
	}
	push(&s, CE_BACKEND_DEVICE_LOST, 0, 0);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_DEVICE_LOST);
	/* The backend now refuses; the engine passes DEVICE_LOST on. */
	s.commit_status = CE_ERR_DEVICE_LOST;
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_ERR_DEVICE_LOST);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_READY);
	/* Recovery: the backend works again and the engine presents. */
	s.commit_status = CE_OK;
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_OK);
	push(&s, CE_BACKEND_LATCHED, OUT, sub);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
}

static void case_device_lost_in_commit(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t first = 0, second = 0;
	int saw_lost = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &first), CE_OK);
	s.lose_device_in_commit = true;
	CHECK_EQ(present_one(engine, BUF_B, &second), CE_ERR_DEVICE_LOST);
	for (;;) {
		int32_t r = poll(engine, &event);
		CHECK(r >= 0);
		if (r != 1) {
			break;
		}
		saw_lost += event.kind == CE_EVENT_DEVICE_LOST;
		CHECK(event.kind != CE_EVENT_PRESENTED);
	}
	CHECK_EQ(saw_lost, 1);
	CHECK_EQ(ce_buffer_state(engine, BUF_B), CE_BUFFER_READY);
}

static void case_backend_error_codes(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	uint64_t sub = 0;
	ce_error_info info;
	if (!engine) {
		return;
	}
	ce_layer l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	ce_frame frame = { .layers = &l, .layer_count = 1, .output = OUT };
	s.check_status = CE_ERR_DEVICE_LOST;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_DEVICE_LOST);
	CHECK_EQ(info.status, CE_ERR_DEVICE_LOST);
	s.check_status = CE_ERR_BACKEND_UNKNOWN_BUFFER;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_BACKEND_UNKNOWN_BUFFER);
	s.check_status = -22;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_BACKEND_PLATFORM);
	CHECK_EQ(info.platform, -22);
	s.check_status = CE_ERR_BACKEND_REJECTED;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_BACKEND_REJECTED);
	s.check_status = CE_OK;
	s.commit_status = CE_ERR_BACKEND_UNKNOWN_BUFFER;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_BACKEND_UNKNOWN_BUFFER);
	CHECK_EQ(s.commits, 0);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_READY);
}

static void case_caps_changed(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t sub = 0;
	if (!engine) {
		return;
	}
	int before = s.capability_calls;
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_OK);
	push(&s, CE_BACKEND_CAPABILITIES_CHANGED, 0, 0);
	push(&s, CE_BACKEND_CAPABILITIES_CHANGED, 0, 0);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_CAPABILITIES_CHANGED);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_CAPABILITIES_CHANGED);
	CHECK(s.capability_calls >= before + 2);
	/* The pending submission survives a capability change. */
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_SUBMITTED);
	push(&s, CE_BACKEND_LATCHED, OUT, sub);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	/* The output disappears: the frame is refused with a plan error. */
	s.output_count = 0;
	push(&s, CE_BACKEND_CAPABILITIES_CHANGED, 0, 0);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(present_one(engine, BUF_B, &sub), CE_ERR_UNKNOWN_OUTPUT);
}

static void case_unknown_backend_event_kind(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t sub = 0;
	if (!engine) {
		return;
	}
	/* A backend that reports a kind the header does not define: no crash. */
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_OK);
	push(&s, 200, OUT, sub);
	push(&s, 255, OUT, sub);
	push(&s, CE_BACKEND_LATCHED, OUT, sub);
	/* Unknown kinds are skipped; the latch behind them still counts. */
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(event.submission, sub);
	CHECK_EQ(drain(engine), 0);
	/* A latch for submission 0, which is never handed out. */
	push(&s, CE_BACKEND_LATCHED, OUT, 0);
	CHECK_EQ(poll(engine, &event), CE_ERR_UNEXPECTED_LATCH);
}

static void case_abi_check(void)
{
	CHECK(ce_abi_check(sizeof(ce_capabilities), sizeof(ce_layer), sizeof(ce_frame),
			   sizeof(ce_event), sizeof(ce_backend_ops), sizeof(ce_assignment),
			   CE_ENGINE_SIZE));
	CHECK(!ce_abi_check(0, sizeof(ce_layer), sizeof(ce_frame), sizeof(ce_event),
			    sizeof(ce_backend_ops), sizeof(ce_assignment), CE_ENGINE_SIZE));
	CHECK(!ce_abi_check(sizeof(ce_capabilities), sizeof(ce_layer) + 1, sizeof(ce_frame),
			    sizeof(ce_event), sizeof(ce_backend_ops), sizeof(ce_assignment),
			    CE_ENGINE_SIZE));
	CHECK(!ce_abi_check(sizeof(ce_capabilities), sizeof(ce_layer), sizeof(ce_frame),
			    sizeof(ce_event), sizeof(ce_backend_ops), sizeof(ce_assignment),
			    CE_ENGINE_SIZE + 1));
	CHECK(!ce_abi_check(SIZE_MAX, SIZE_MAX, SIZE_MAX, SIZE_MAX, SIZE_MAX, SIZE_MAX, SIZE_MAX));
}

static void case_event_flood(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	int n = 0;
	if (!engine) {
		return;
	}
	/* More queued events than the engine's own queue holds. */
	for (int i = 0; i < 60; i++) {
		push(&s, CE_BACKEND_CAPABILITIES_CHANGED, 0, 0);
	}
	for (int guard = 0; guard < 200; guard++) {
		int32_t r = poll(engine, &event);
		if (r == 0) {
			break;
		}
		CHECK(r > 0 || r == CE_ERR_EVENT_QUEUE_FULL);
		if (r < 0) {
			break;
		}
		n++;
	}
	CHECK_EQ(n, 60);
}

/* ---- malformed arguments -------------------------------------------------- */

static void case_null_arguments(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	ce_layer l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	ce_frame frame = { .layers = &l, .layer_count = 1, .output = OUT };
	uint64_t sub = 0;
	ce_error_info info;
	if (!engine) {
		return;
	}
	CHECK(ce_engine_init(NULL, sizeof(storage), &ops, &s) == NULL);
	CHECK(ce_engine_init(storage, sizeof(storage), NULL, &s) == NULL);
	CHECK(ce_engine_init(storage, 0, &ops, &s) == NULL);
	CHECK(ce_engine_init(storage, 1, &ops, &s) == NULL);
	CHECK(ce_engine_init(storage + 4, sizeof(storage) - 4, &ops, &s) == NULL);

	CHECK_EQ(ce_register_buffer(NULL, 9, &xrgb), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(ce_register_buffer(engine, 9, NULL), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(ce_unregister_buffer(NULL, 9), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(ce_begin_write(NULL, 1), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(ce_finish_write(NULL, 1), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(ce_buffer_state(NULL, 1), CE_BUFFER_UNKNOWN);
	CHECK_EQ(ce_present(NULL, &frame, &sub, &info), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(info.status, CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(ce_present(engine, NULL, &sub, &info), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(ce_poll_event(NULL, &event), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(ce_poll_event(engine, NULL), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(ce_set_latch_timeout(NULL, 1), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(ce_advance_time(NULL, 1), CE_ERR_INVALID_ARGUMENT);
	/* Output pointers are optional where documented. */
	frame.layer_count = 0;
	frame.layers = NULL;
	CHECK_EQ(ce_present(engine, &frame, NULL, NULL), CE_OK);
	frame.layer_count = 1;
	CHECK_EQ(ce_present(engine, &frame, NULL, &info), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(info.status, CE_ERR_INVALID_ARGUMENT);
}

static void case_unknown_ids(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_error_info info;
	uint64_t sub = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(ce_unregister_buffer(engine, 99), CE_ERR_UNKNOWN_BUFFER);
	CHECK_EQ(ce_begin_write(engine, 99), CE_ERR_UNKNOWN_BUFFER);
	CHECK_EQ(ce_finish_write(engine, 99), CE_ERR_UNKNOWN_BUFFER);
	CHECK_EQ(ce_buffer_state(engine, 99), CE_BUFFER_UNKNOWN);
	CHECK_EQ(ce_buffer_state(engine, UINT32_MAX), CE_BUFFER_UNKNOWN);
	CHECK_EQ(ce_buffer_state(engine, 0), CE_BUFFER_UNKNOWN);

	ce_layer l = layer(99, 0, CE_BLEND_OPAQUE);
	ce_frame frame = { .layers = &l, .layer_count = 1, .output = OUT };
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_UNKNOWN_BUFFER);
	CHECK_EQ(info.buffer, 99);
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	frame.output = 12345;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_UNKNOWN_OUTPUT);
	frame.output = UINT32_MAX;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_UNKNOWN_OUTPUT);
	/* An empty frame for an unknown output is as wrong as any other. */
	frame.layer_count = 0;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_UNKNOWN_OUTPUT);
	CHECK_EQ(s.commits, 0);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_READY);
}

static void case_buffer_registration(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_status last = CE_OK, r;
	if (!engine) {
		return;
	}
	ce_buffer_info zero_w = xrgb, zero_h = xrgb, huge = xrgb, odd_format = xrgb;
	zero_w.width = 0;
	zero_h.height = 0;
	huge.width = UINT32_MAX;
	huge.height = UINT32_MAX;
	odd_format.format = 0;
	/* Whatever the verdict, it is a status and the table stays consistent. */
	r = ce_register_buffer(engine, 20, &zero_w);
	CHECK(r == CE_OK || r < 0);
	r = ce_register_buffer(engine, 21, &zero_h);
	CHECK(r == CE_OK || r < 0);
	r = ce_register_buffer(engine, 22, &huge);
	CHECK(r == CE_OK || r < 0);
	r = ce_register_buffer(engine, 23, &odd_format);
	CHECK(r == CE_OK || r < 0);
	/* A zero-size buffer must never be shown. */
	if (ce_buffer_state(engine, 20) != CE_BUFFER_UNKNOWN) {
		CHECK_EQ(ce_finish_write(engine, 20), CE_OK);
		ce_layer l = layer(20, 0, CE_BLEND_OPAQUE);
		ce_frame frame = { .layers = &l, .layer_count = 1, .output = OUT };
		CHECK(ce_present(engine, &frame, NULL, NULL) != CE_OK);
	}
	/* Fill the table: it refuses one more. */
	for (uint32_t id = 1000; id < 1000 + CE_MAX_BUFFERS + 4; id++) {
		last = ce_register_buffer(engine, id, &xrgb);
		if (last != CE_OK) {
			break;
		}
	}
	CHECK(last == CE_ERR_BUFFER_TABLE_FULL || last == CE_ERR_TOO_MANY_BUFFERS);
	/* Unregister frees a slot again. */
	CHECK_EQ(ce_unregister_buffer(engine, 1000), CE_OK);
	CHECK_EQ(ce_register_buffer(engine, 5000, &xrgb), CE_OK);
	CHECK_EQ(ce_unregister_buffer(engine, 1000), CE_ERR_UNKNOWN_BUFFER);
	/* Writing lifecycle misuse. */
	CHECK_EQ(ce_finish_write(engine, 5000), CE_OK);
	CHECK_EQ(ce_finish_write(engine, 5000), CE_ERR_BUFFER_NOT_WRITABLE);
	CHECK_EQ(ce_begin_write(engine, 5000), CE_OK);
	CHECK_EQ(ce_begin_write(engine, 5000), CE_OK);
}

static void case_malformed_layers(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_error_info info;
	uint64_t sub = 0;
	ce_status r;
	ce_layer many[CE_MAX_LAYERS + 1];
	if (!engine) {
		return;
	}
	ce_layer l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	ce_frame frame = { .layers = &l, .layer_count = 1, .output = OUT };

	/* Out-of-range enums. */
	l.blend = 3;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_ARGUMENT);
	CHECK_EQ(info.layer, 0);
	l.blend = 255;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_ARGUMENT);
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.transform = 6;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_ARGUMENT);
	l.transform = 255;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_ARGUMENT);

	/* Zero sizes. */
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.source.width = 0;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_SOURCE);
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.source.height = 0;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_SOURCE);
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.destination.width = 0;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_DESTINATION);
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.destination.height = 0;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_DESTINATION);

	/* Source outside the buffer, and extreme coordinates (overflow). */
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.source.width = 641;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_SOURCE);
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.source.x = INT32_MAX;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_SOURCE);
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.source.x = INT32_MIN;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_SOURCE);
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.source.width = UINT32_MAX;
	l.source.height = UINT32_MAX;
	CHECK_EQ(ce_present(engine, &frame, &sub, &info), CE_ERR_INVALID_SOURCE);
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.destination.x = INT32_MAX;
	l.destination.width = UINT32_MAX;
	CHECK(ce_present(engine, &frame, &sub, &info) <= 0);
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.destination.x = INT32_MIN;
	l.destination.y = INT32_MIN;
	l.destination.width = UINT32_MAX;
	l.destination.height = UINT32_MAX;
	CHECK(ce_present(engine, &frame, &sub, &info) <= 0);
	/* Entirely off the output: no plane needed, no crash. */
	l = layer(BUF_A, 0, CE_BLEND_OPAQUE);
	l.destination.x = 100000;
	r = ce_present(engine, &frame, &sub, &info);
	CHECK(r == CE_OK || r < 0);
	if (r == CE_OK) {
		push(&s, CE_BACKEND_LATCHED, OUT, sub);
		CHECK(drain(engine) >= 0);
	}

	/* Alpha and z extremes are legal values. */
	l = layer(BUF_B, INT32_MIN, CE_BLEND_OPAQUE);
	l.alpha = 0;
	r = ce_present(engine, &frame, &sub, &info);
	CHECK(r == CE_OK || r < 0);
	l = layer(BUF_B, INT32_MAX, CE_BLEND_OPAQUE);
	r = ce_present(engine, &frame, &sub, &info);
	CHECK(r == CE_OK || r < 0);

	/* Too many layers. */
	for (int i = 0; i < CE_MAX_LAYERS + 1; i++) {
		many[i] = layer(BUF_A, i, CE_BLEND_OPAQUE);
	}
	ce_frame big = { .layers = many, .layer_count = CE_MAX_LAYERS + 1, .output = OUT };
	CHECK_EQ(ce_present(engine, &big, &sub, &info), CE_ERR_TOO_MANY_LAYERS);
	big.layer_count = SIZE_MAX;
	CHECK_EQ(ce_present(engine, &big, &sub, &info), CE_ERR_TOO_MANY_LAYERS);
	big.layer_count = CE_MAX_LAYERS;
	CHECK(ce_present(engine, &big, &sub, &info) < 0);
	big.layer_count = 3; /* three layers, two planes */
	CHECK(ce_present(engine, &big, &sub, &info) < 0);
	/* The engine still works. */
	CHECK(drain(engine) >= 0);
}

static void case_unregister_in_flight(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t sub = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_OK);
	CHECK_EQ(ce_unregister_buffer(engine, BUF_A), CE_ERR_BUFFER_IN_USE);
	push(&s, CE_BACKEND_LATCHED, OUT, sub);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(ce_unregister_buffer(engine, BUF_A), CE_ERR_BUFFER_IN_USE);
	/* Blank the output, then it may go. */
	ce_frame empty = { .layers = NULL, .layer_count = 0, .output = OUT };
	CHECK_EQ(ce_present(engine, &empty, &sub, NULL), CE_OK);
	push(&s, CE_BACKEND_LATCHED, OUT, sub);
	CHECK_EQ(drain(engine), 2);
	CHECK_EQ(ce_unregister_buffer(engine, BUF_A), CE_OK);
	CHECK_EQ(ce_buffer_state(engine, BUF_A), CE_BUFFER_UNKNOWN);
	/* Presenting a removed buffer is an unknown buffer. */
	CHECK_EQ(present_one(engine, BUF_A, &sub), CE_ERR_UNKNOWN_BUFFER);
}

static void case_busy_without_replace(void)
{
	struct script s;
	ce_engine *engine = fresh(&s, 0);
	ce_event event;
	uint64_t first = 0, second = 0;
	if (!engine) {
		return;
	}
	CHECK_EQ(present_one(engine, BUF_A, &first), CE_OK);
	CHECK_EQ(present_one(engine, BUF_B, &second), CE_ERR_BUSY);
	/* After the latch the next commit goes through. */
	push(&s, CE_BACKEND_LATCHED, OUT, first);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(present_one(engine, BUF_B, &second), CE_OK);
	/* A latch queued ahead of time is seen by the next present. */
	push(&s, CE_BACKEND_LATCHED, OUT, second);
	CHECK_EQ(present_one(engine, BUF_A, &first), CE_OK);
	CHECK_EQ(poll(engine, &event), 1);
	CHECK_EQ(event.kind, CE_EVENT_PRESENTED);
	CHECK_EQ(event.submission, second);
}

static void case_stress_ordering(void)
{
	/* A deterministic pseudo-random mix of valid and invalid operations. */
	struct script s;
	ce_engine *engine = fresh(&s, 1);
	ce_event event;
	uint64_t x = 0x9E3779B97F4A7C15ull;
	uint64_t pending[4] = { 0 };
	if (!engine) {
		return;
	}
	for (int step = 0; step < 20000; step++) {
		x ^= x << 13;
		x ^= x >> 7;
		x ^= x << 17;
		switch (x % 9) {
		case 0:
		case 1: {
			uint64_t sub = 0;
			if (present_one(engine, BUF_A + (uint32_t)((x >> 8) % 4), &sub) == CE_OK) {
				pending[(x >> 16) % 4] = sub;
			}
			break;
		}
		case 2:
			push(&s, CE_BACKEND_LATCHED, OUT, pending[(x >> 8) % 4]);
			break;
		case 3:
			push(&s, CE_BACKEND_LATCHED, OUT, x >> 40);
			break;
		case 4:
			if ((x >> 8) % 16 == 0) {
				push(&s, CE_BACKEND_DEVICE_LOST, 0, 0);
			}
			break;
		case 5:
			push(&s, CE_BACKEND_CAPABILITIES_CHANGED, 0, 0);
			break;
		case 6:
			(void)ce_begin_write(engine, BUF_A + (uint32_t)((x >> 8) % 4));
			(void)ce_finish_write(engine, BUF_A + (uint32_t)((x >> 8) % 4));
			break;
		case 7:
			(void)ce_advance_time(engine, (uint64_t)step);
			break;
		default:
			(void)poll(engine, &event);
			break;
		}
		if (s.tail - s.head > QUEUE - 4) {
			s.head = s.tail;
		}
		for (uint32_t b = BUF_A; b <= BUF_D; b++) {
			int32_t st = ce_buffer_state(engine, b);
			if (st < CE_BUFFER_WRITABLE || st > CE_BUFFER_SCANNING) {
				CHECK(st >= CE_BUFFER_WRITABLE && st <= CE_BUFFER_SCANNING);
				return;
			}
		}
	}
	CHECK(drain(engine) != -1000);
}

/* ---- the table ------------------------------------------------------------- */

struct test_case {
	const char *name;
	void (*run)(void);
	bool known_bug;
};

static const struct test_case cases[] = {
	{ "latch_before_commit", case_latch_before_commit, false },
	{ "latch_unknown_submission", case_latch_unknown_submission, false },
	{ "latch_wrong_output", case_latch_wrong_output, false },
	{ "latch_duplicated", case_latch_duplicated, false },
	{ "latch_out_of_order", case_latch_out_of_order, false },
	{ "latch_missing", case_latch_missing, false },
	{ "time_backwards", case_time_backwards, false },
	{ "replace_pending", case_replace_pending, false },
	{ "replace_same_buffer", case_replace_same_buffer, false },
	{ "replace_latched_during_commit", case_replace_latched_during_commit, false },
	{ "commit_fails_after_pending", case_commit_fails_after_pending, false },
	{ "release_sequence", case_release_sequence, false },
	{ "release_never_for_scanned_buffer", case_release_never_for_scanned_buffer, false },
	{ "blank_frame_releases", case_blank_frame_releases, false },
	{ "device_lost_event", case_device_lost_event, false },
	{ "device_lost_then_present", case_device_lost_then_present, false },
	{ "device_lost_in_commit", case_device_lost_in_commit, false },
	{ "backend_error_codes", case_backend_error_codes, false },
	{ "caps_changed", case_caps_changed, false },
	{ "unknown_backend_event_kind", case_unknown_backend_event_kind, false },
	{ "abi_check", case_abi_check, false },
	{ "event_flood", case_event_flood, false },
	{ "null_arguments", case_null_arguments, false },
	{ "unknown_ids", case_unknown_ids, false },
	{ "buffer_registration", case_buffer_registration, false },
	{ "malformed_layers", case_malformed_layers, false },
	{ "unregister_in_flight", case_unregister_in_flight, false },
	{ "busy_without_replace", case_busy_without_replace, false },
	{ "stress_ordering", case_stress_ordering, false },
};

int main(int argc, char **argv)
{
	size_t n = sizeof(cases) / sizeof(cases[0]);
	CHECK(CE_ABI_CHECK());
	if (argc > 1) {
		for (size_t i = 0; i < n; i++) {
			if (strcmp(argv[1], cases[i].name) == 0) {
				cases[i].run();
				if (failures) {
					fprintf(stderr, "FAIL %s\n", cases[i].name);
					return 1;
				}
				puts("ok");
				return 0;
			}
		}
		fprintf(stderr, "unknown case %s\n", argv[1]);
		return 2;
	}
	for (size_t i = 0; i < n; i++) {
		int before = failures;
		if (cases[i].known_bug) {
			continue;
		}
		cases[i].run();
		if (failures != before) {
			fprintf(stderr, "FAIL %s\n", cases[i].name);
		}
	}
	if (failures) {
		return 1;
	}
	puts("ok");
	return 0;
}
