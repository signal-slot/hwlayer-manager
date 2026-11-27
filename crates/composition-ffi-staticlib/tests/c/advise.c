/*
 * The stateless advisory planner through the C interface, compiled against
 * the header: CE_ABI_CHECK_ADVISE(), one frame that fits the planes and one
 * that needs a composition buffer, with a check callback.
 *
 * Exit status 0 means every check passed.
 */
#include "composition_engine.h"

#include <stdio.h>
#include <string.h>

static int failures;

#define CHECK_EQ(actual, expected) \
	do { \
		long long a_ = (long long)(actual), e_ = (long long)(expected); \
		if (a_ != e_) { \
			fprintf(stderr, "%s:%d: %s is %lld, expected %lld\n", __FILE__, __LINE__, \
				#actual, a_, e_); \
			failures++; \
		} \
	} while (0)

enum { OUT = 7, PRIMARY = 31, OVERLAY = 41 };

/* 640x480 output, an XRGB8888 primary at zpos 0, an ARGB8888 overlay at 1. */
static ce_capabilities caps;

static void make_caps(void)
{
	memset(&caps, 0, sizeof(caps));
	caps.output_count = 1;
	caps.outputs[0] = (ce_output_caps){ .id = OUT, .width = 640, .height = 480,
					    .refresh_mhz = 60000 };
	caps.plane_count = 2;
	caps.planes[0] = (ce_plane_caps){
		.id = PRIMARY, .kind = CE_PLANE_PRIMARY, .outputs = 1,
		.format_count = 1, .zpos_min = 0,
		.blend_modes = 1u << CE_BLEND_OPAQUE,
	};
	caps.planes[0].formats[0] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_XRGB8888 };
	caps.planes[1] = (ce_plane_caps){
		.id = OVERLAY, .kind = CE_PLANE_OVERLAY, .outputs = 1,
		.format_count = 1, .zpos_min = 1,
		.blend_modes = (1u << CE_BLEND_PREMULTIPLIED) | (1u << CE_BLEND_OPAQUE),
	};
	caps.planes[1].formats[0] = (ce_format){ CE_MODIFIER_LINEAR, CE_FORMAT_ARGB8888 };
}

static const ce_buffer_entry buffers[] = {
	{ 1, { CE_MODIFIER_LINEAR, 640, 480, CE_FORMAT_XRGB8888 } },
	{ 2, { CE_MODIFIER_LINEAR, 100, 100, CE_FORMAT_ARGB8888 } },
	{ 3, { CE_MODIFIER_LINEAR, 100, 100, CE_FORMAT_ARGB8888 } },
};

static ce_layer layer(uint32_t buffer, uint32_t w, uint32_t h, int32_t z, uint8_t blend)
{
	return (ce_layer){
		.buffer = buffer,
		.source = { 0, 0, w, h },
		.destination = { 0, 0, w, h },
		.z_order = z,
		.alpha = CE_ALPHA_OPAQUE,
		.blend = blend,
		.transform = CE_TRANSFORM_NORMAL,
	};
}

struct counter {
	int calls, composed;
};

static ce_status count_check(void *ctx, const ce_frame *frame, const ce_assignment *assignments,
			     size_t count)
{
	struct counter *c = ctx;
	(void)frame;
	c->calls++;
	for (size_t i = 0; i < count; i++) {
		if (assignments[i].layer == CE_COMPOSE_LAYER) {
			c->composed++;
		}
	}
	return CE_OK;
}

static ce_advise_options options(void)
{
	ce_advise_options o;
	memset(&o, 0, sizeof(o));
	o.compose = (ce_compose_target){ .modifier = CE_MODIFIER_LINEAR,
					 .format = CE_FORMAT_XRGB8888,
					 .blend = CE_BLEND_OPAQUE,
					 .enabled = 1 };
	return o;
}

static void direct(void)
{
	ce_layer layers[2] = { layer(1, 640, 480, 0, CE_BLEND_OPAQUE),
			       layer(2, 100, 100, 1, CE_BLEND_PREMULTIPLIED) };
	ce_frame frame = { layers, 2, OUT };
	ce_advise_options o = options();
	ce_advice advice;
	ce_error_info info;
	struct counter c = { 0, 0 };
	CHECK_EQ(ce_advise(&caps, buffers, 3, &frame, &o, count_check, &c, &advice, &info), CE_OK);
	CHECK_EQ(info.status, CE_OK);
	CHECK_EQ(advice.layer_count, 2);
	CHECK_EQ(advice.has_compose, 0);
	CHECK_EQ(advice.layers[0].placement, CE_PLACE_PLANE);
	CHECK_EQ(advice.layers[0].assignment.plane, PRIMARY);
	CHECK_EQ(advice.layers[1].placement, CE_PLACE_PLANE);
	CHECK_EQ(advice.layers[1].blocker, CE_BLOCKER_NONE);
	CHECK_EQ(advice.layers[1].assignment.plane, OVERLAY);
	CHECK_EQ(c.calls, 1);
	CHECK_EQ(c.composed, 0);
}

static void composed(void)
{
	ce_layer layers[3] = { layer(1, 640, 480, 0, CE_BLEND_OPAQUE),
			       layer(2, 100, 100, 1, CE_BLEND_PREMULTIPLIED),
			       layer(3, 100, 100, 2, CE_BLEND_PREMULTIPLIED) };
	ce_frame frame = { layers, 3, OUT };
	ce_advise_options o = options();
	ce_advice advice;
	ce_error_info info;
	struct counter c = { 0, 0 };
	CHECK_EQ(ce_advise(&caps, buffers, 3, &frame, &o, count_check, &c, &advice, &info), CE_OK);
	CHECK_EQ(advice.layer_count, 3);
	CHECK_EQ(advice.has_compose, 1);
	CHECK_EQ(advice.compose_low, 0);
	CHECK_EQ(advice.compose_high, 1);
	CHECK_EQ(advice.compose.layer, CE_COMPOSE_LAYER);
	CHECK_EQ(advice.compose.plane, PRIMARY);
	CHECK_EQ(advice.layers[0].placement, CE_PLACE_COMPOSE);
	CHECK_EQ(advice.layers[1].placement, CE_PLACE_COMPOSE);
	CHECK_EQ(advice.layers[1].blocker, CE_BLOCKER_COMBINATION);
	CHECK_EQ(advice.layers[2].placement, CE_PLACE_PLANE);
	CHECK_EQ(advice.layers[2].assignment.plane, OVERLAY);
	CHECK_EQ(c.composed, 1);

	/* Too many layers: an error, and the advice is not written. */
	memset(&advice, 0xA5, sizeof(advice));
	frame.layer_count = CE_MAX_LAYERS + 1;
	CHECK_EQ(ce_advise(&caps, buffers, 3, &frame, &o, NULL, NULL, &advice, &info),
		 CE_ERR_TOO_MANY_LAYERS);
	CHECK_EQ(advice.layer_count, 0xA5A5A5A5u);
}

int main(void)
{
	CHECK_EQ(CE_ABI_CHECK(), 1);
	CHECK_EQ(CE_ABI_CHECK_ADVISE(), 1);
	CHECK_EQ(ce_abi_check_advise(sizeof(ce_compose_target), sizeof(ce_advise_options),
				     sizeof(ce_layer_advice), sizeof(ce_advice) + 1,
				     sizeof(ce_buffer_entry)),
		 0);
	make_caps();
	direct();
	composed();
	if (failures) {
		fprintf(stderr, "%d check(s) failed\n", failures);
		return 1;
	}
	puts("ok");
	return 0;
}
