/*
 * libliftoff side of the cross-check (docs/libliftoff-crosscheck.md
 * sections 6 and 8).
 *
 * Reads one xcheck v1 file, installs its topology into the fake libdrm and,
 * for every frame, runs liftoff_output_apply on a fresh device, output and
 * layer set. TEST_ONLY commits are answered over a pipe (PROTOCOL.md) or by a
 * built-in responder. One "result" line is printed per frame.
 *
 * Usage: liftoff-xcheck-driver [--accept-all | --reject-all]
 *            [--request-fd N] [--answer-fd N] [--timeout-ns N] [--verbose]
 *            FILE.xcheck
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <limits.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include <drm_fourcc.h>
#include <drm_mode.h>
#include <libliftoff.h>
#include <xf86drm.h>
#include <xf86drmMode.h>

#include "xcheck_fake.h"

/* --- parsed file ---------------------------------------------------------- */

enum transform_bits {
	T_ROT0 = DRM_MODE_ROTATE_0,
	T_ROT90 = DRM_MODE_ROTATE_90,
	T_ROT180 = DRM_MODE_ROTATE_180,
	T_ROT270 = DRM_MODE_ROTATE_270,
	T_FLIP_H = DRM_MODE_REFLECT_X,
	T_FLIP_V = DRM_MODE_REFLECT_Y,
};

struct layer {
	uint32_t buffer;
	int64_t src[4];
	int64_t dst[4];
	int32_t z;
	uint32_t alpha;
	enum xc_blend blend;
	uint32_t rotation;
};

struct frame {
	char *name;
	uint32_t output;
	size_t n_layers;
	struct layer *layers;
	int line;
};

struct file {
	char *origin;
	struct xc_topology topo;
	size_t n_frames;
	struct frame *frames;
};

static const char *path;
static int lineno;

static void die(const char *fmt, ...)
{
	va_list ap;
	va_start(ap, fmt);
	fprintf(stderr, "liftoff-xcheck-driver: %s:%d: ", path, lineno);
	vfprintf(stderr, fmt, ap);
	fputc('\n', stderr);
	va_end(ap);
	exit(1);
}

static void *grow(void *array, size_t n, size_t size)
{
	/* Grows by one element; arrays are small. */
	void *p = realloc(array, (n + 1) * size);
	if (!p)
		die("out of memory");
	memset((char *)p + n * size, 0, size);
	return p;
}

static uint64_t parse_u64(const char *s, const char *what)
{
	char *end;
	errno = 0;
	unsigned long long v = strtoull(s, &end, 0);
	if (errno || end == s || *end || *s == '-')
		die("bad %s '%s'", what, s);
	return v;
}

static int64_t parse_i64(const char *s, const char *what)
{
	char *end;
	errno = 0;
	long long v = strtoll(s, &end, 10);
	if (errno || end == s || *end)
		die("bad %s '%s'", what, s);
	return v;
}

static uint32_t parse_u32(const char *s, const char *what)
{
	uint64_t v = parse_u64(s, what);
	if (v > UINT32_MAX)
		die("%s '%s' out of range", what, s);
	return (uint32_t)v;
}

/* DRM fourcc string as the probe prints it ("XR24", "NV12"; shorter codes
 * are space padded, as in drm_fourcc.h). */
static uint32_t parse_fourcc(const char *s)
{
	size_t n = strlen(s);
	if (n == 0 || n > 4)
		die("bad fourcc '%s'", s);
	char c[4] = { ' ', ' ', ' ', ' ' };
	memcpy(c, s, n);
	return fourcc_code(c[0], c[1], c[2], c[3]);
}

static void parse_rect(const char *s, int64_t r[4], const char *what)
{
	long long v[4];
	int used = 0;
	if (sscanf(s, "%lld,%lld,%lld,%lld%n", &v[0], &v[1], &v[2], &v[3], &used) != 4 ||
	    s[used] != '\0')
		die("bad %s rectangle '%s'", what, s);
	if (v[2] < 0 || v[3] < 0)
		die("negative size in %s rectangle '%s'", what, s);
	for (int i = 0; i < 4; i++)
		r[i] = v[i];
}

static int parse_blend(const char *s)
{
	if (!strcmp(s, "opaque"))
		return XC_BLEND_NONE;
	if (!strcmp(s, "premul"))
		return XC_BLEND_PREMULTI;
	if (!strcmp(s, "coverage"))
		return XC_BLEND_COVERAGE;
	die("bad blend mode '%s'", s);
	return -1;
}

static uint32_t parse_transform_name(const char *s)
{
	static const struct {
		const char *name;
		uint32_t bits;
	} names[] = {
		{ "normal", T_ROT0 },   { "rot90", T_ROT90 },   { "rot180", T_ROT180 },
		{ "rot270", T_ROT270 }, { "flip_h", T_FLIP_H }, { "flip_v", T_FLIP_V },
	};
	for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); i++)
		if (!strcmp(s, names[i].name))
			return names[i].bits;
	die("bad transform '%s'", s);
	return 0;
}

/* A layer transform: one name, or names joined by '+' (rot90+flip_h). A
 * reflection alone means rotate-0 plus that reflection, as DRM requires one
 * rotation bit. */
static uint32_t parse_layer_transform(char *s)
{
	uint32_t bits = 0;
	char *save;
	for (char *t = strtok_r(s, "+", &save); t; t = strtok_r(NULL, "+", &save))
		bits |= parse_transform_name(t);
	if (!(bits & DRM_MODE_ROTATE_MASK))
		bits |= T_ROT0;
	return bits;
}

/* Splits "key=value"; returns value or NULL when the token has no '='. */
static char *kv(char *tok, char **key)
{
	char *eq = strchr(tok, '=');
	if (!eq)
		return NULL;
	*eq = '\0';
	*key = tok;
	return eq + 1;
}

static void parse_plane(struct file *f, char **tok, int n)
{
	struct xc_topology *t = &f->topo;
	if (n < 2)
		die("plane needs an id");
	t->planes = grow(t->planes, t->n_planes, sizeof(*t->planes));
	struct xc_plane *pl = &t->planes[t->n_planes++];
	pl->id = parse_u32(tok[1], "plane id");
	for (size_t i = 0; i + 1 < t->n_planes; i++)
		if (t->planes[i].id == pl->id)
			die("duplicate plane %u", pl->id);
	bool have_kind = false, have_zpos = false, have_now = false;
	for (int i = 2; i < n; i++) {
		char *key, *val = kv(tok[i], &key);
		if (!val)
			die("bad plane field '%s'", tok[i]);
		if (!strcmp(key, "kind")) {
			have_kind = true;
			if (!strcmp(val, "primary"))
				pl->kind = XC_KIND_PRIMARY;
			else if (!strcmp(val, "overlay"))
				pl->kind = XC_KIND_OVERLAY;
			else if (!strcmp(val, "cursor"))
				pl->kind = XC_KIND_CURSOR;
			else
				die("bad plane kind '%s'", val);
		} else if (!strcmp(key, "outputs")) {
			pl->possible_crtcs = parse_u32(val, "outputs mask");
		} else if (!strcmp(key, "zpos")) {
			long long a, b;
			int used = 0;
			have_zpos = true;
			if (!strncmp(val, "fixed:", 6)) {
				pl->zpos_fixed = 1;
				pl->zpos_min = pl->zpos_max = parse_i64(val + 6, "zpos");
			} else if (sscanf(val, "%lld..%lld%n", &a, &b, &used) == 2 &&
				   val[used] == '\0' && a <= b) {
				pl->zpos_min = a;
				pl->zpos_max = b;
			} else {
				die("bad zpos '%s'", val);
			}
		} else if (!strcmp(key, "zpos_now")) {
			have_now = true;
			pl->zpos_now = parse_i64(val, "zpos_now");
		} else if (!strcmp(key, "alpha")) {
			pl->has_alpha = (int)parse_u32(val, "alpha flag") != 0;
		} else if (!strcmp(key, "blend")) {
			if (strcmp(val, "-")) {
				char *save;
				for (char *b = strtok_r(val, ",", &save); b; b = strtok_r(NULL, ",", &save))
					pl->blend_mask |= 1u << parse_blend(b);
			}
		} else if (!strcmp(key, "transforms")) {
			if (strcmp(val, "-")) {
				char *save;
				for (char *b = strtok_r(val, ",", &save); b; b = strtok_r(NULL, ",", &save))
					pl->rotation_mask |= parse_transform_name(b);
				pl->rotation_mask |= T_ROT0;
			}
		} else if (!strcmp(key, "scaling") || !strcmp(key, "max_source") ||
			   !strcmp(key, "full_output")) {
			/* Constraints of the test-commit model; the answering side
			 * applies them. */
		} else {
			fprintf(stderr, "liftoff-xcheck-driver: %s:%d: ignoring plane field '%s'\n",
				path, lineno, key);
		}
	}
	if (!have_kind || !have_zpos)
		die("plane %u needs kind= and zpos=", pl->id);
	if (!pl->zpos_fixed && !have_now)
		die("plane %u has a zpos range and no zpos_now=", pl->id);
	if (!pl->zpos_fixed && (pl->zpos_now < pl->zpos_min || pl->zpos_now > pl->zpos_max))
		die("plane %u zpos_now outside its range", pl->id);
}

static void parse_layer(struct frame *fr, char **tok, int n)
{
	fr->layers = grow(fr->layers, fr->n_layers, sizeof(*fr->layers));
	struct layer *l = &fr->layers[fr->n_layers++];
	l->alpha = 0xFFFF;
	l->blend = XC_BLEND_PREMULTI;
	l->rotation = T_ROT0;
	bool have_buf = false, have_src = false, have_dst = false, have_z = false;
	for (int i = 1; i < n; i++) {
		char *key, *val = kv(tok[i], &key);
		if (!val)
			die("bad layer field '%s'", tok[i]);
		if (!strcmp(key, "buffer")) {
			have_buf = true;
			l->buffer = parse_u32(val, "buffer id");
		} else if (!strcmp(key, "src")) {
			have_src = true;
			parse_rect(val, l->src, "src");
			if (l->src[0] < 0 || l->src[1] < 0)
				die("negative src origin");
		} else if (!strcmp(key, "dst")) {
			have_dst = true;
			parse_rect(val, l->dst, "dst");
		} else if (!strcmp(key, "z")) {
			have_z = true;
			int64_t z = parse_i64(val, "z");
			if (z < INT32_MIN || z > INT32_MAX)
				die("z out of range");
			l->z = (int32_t)z;
		} else if (!strcmp(key, "alpha")) {
			uint32_t a = parse_u32(val, "alpha");
			if (a > 0xFFFF)
				die("alpha above 65535");
			l->alpha = a;
		} else if (!strcmp(key, "blend")) {
			l->blend = parse_blend(val);
		} else if (!strcmp(key, "transform")) {
			l->rotation = parse_layer_transform(val);
		} else {
			fprintf(stderr, "liftoff-xcheck-driver: %s:%d: ignoring layer field '%s'\n",
				path, lineno, key);
		}
	}
	if (!have_buf || !have_src || !have_dst || !have_z)
		die("layer needs buffer=, src=, dst= and z=");
}

#define MAX_TOKENS 64

static void parse_file(struct file *f, FILE *in)
{
	char *line = NULL;
	size_t cap = 0;
	bool header = false;
	struct frame *cur = NULL;
	memset(f, 0, sizeof(*f));
	lineno = 0;

	while (getline(&line, &cap, in) >= 0) {
		lineno++;
		char *hash = strchr(line, '#');
		if (hash)
			*hash = '\0';
		line[strcspn(line, "\r\n")] = '\0';

		/* origin keeps its free text verbatim, so it is split by hand. */
		char *p = line + strspn(line, " \t");
		if (!strncmp(p, "origin", 6) && (p[6] == ' ' || p[6] == '\t')) {
			if (!header)
				die("origin before the xcheck header");
			if (f->origin)
				die("second origin line");
			char *text = p + 6 + strspn(p + 6, " \t");
			size_t len = strlen(text);
			while (len && (text[len - 1] == ' ' || text[len - 1] == '\t'))
				text[--len] = '\0';
			char kind[16];
			if (sscanf(text, "%15s", kind) != 1 ||
			    (strcmp(kind, "observed") && strcmp(kind, "source") &&
			     strcmp(kind, "synthetic")))
				die("origin must start with observed, source or synthetic");
			f->origin = strdup(text);
			continue;
		}

		char *tok[MAX_TOKENS];
		int n = 0;
		char *save;
		for (char *t = strtok_r(line, " \t", &save); t; t = strtok_r(NULL, " \t", &save)) {
			if (n == MAX_TOKENS)
				die("too many fields");
			tok[n++] = t;
		}
		if (n == 0)
			continue;

		if (!header) {
			if (n != 2 || strcmp(tok[0], "xcheck") || strcmp(tok[1], "1"))
				die("first line must be 'xcheck 1'");
			header = true;
			continue;
		}
		if (!f->origin)
			die("origin must follow the xcheck header");

		struct xc_topology *t = &f->topo;
		if (!strcmp(tok[0], "profile") || !strcmp(tok[0], "group")) {
			/* Constraints of the test-commit model, not KMS properties. */
		} else if (!strcmp(tok[0], "output")) {
			if (n != 5)
				die("output needs <id> <w> <h> <refresh_mhz>");
			if (t->n_outputs == 32)
				die("more than 32 outputs");
			t->outputs = grow(t->outputs, t->n_outputs, sizeof(*t->outputs));
			struct xc_output *o = &t->outputs[t->n_outputs++];
			o->id = parse_u32(tok[1], "output id");
			o->width = parse_u32(tok[2], "width");
			o->height = parse_u32(tok[3], "height");
			o->refresh_mhz = parse_u32(tok[4], "refresh");
		} else if (!strcmp(tok[0], "plane")) {
			cur = NULL;
			parse_plane(f, tok, n);
		} else if (!strcmp(tok[0], "format")) {
			if (cur || !t->n_planes)
				die("format outside a plane");
			if (n != 3)
				die("format needs <fourcc> <modifier>");
			struct xc_plane *pl = &t->planes[t->n_planes - 1];
			pl->formats = grow(pl->formats, pl->n_formats, sizeof(*pl->formats));
			pl->formats[pl->n_formats].fourcc = parse_fourcc(tok[1]);
			pl->formats[pl->n_formats].modifier = parse_u64(tok[2], "modifier");
			pl->n_formats++;
		} else if (!strcmp(tok[0], "buffer")) {
			if (n != 6)
				die("buffer needs <id> <w> <h> <fourcc> <modifier>");
			t->buffers = grow(t->buffers, t->n_buffers, sizeof(*t->buffers));
			struct xc_buffer *b = &t->buffers[t->n_buffers++];
			b->id = parse_u32(tok[1], "buffer id");
			b->width = parse_u32(tok[2], "width");
			b->height = parse_u32(tok[3], "height");
			b->fourcc = parse_fourcc(tok[4]);
			b->modifier = parse_u64(tok[5], "modifier");
			if (b->id == 0)
				die("buffer id 0 means 'no framebuffer'");
		} else if (!strcmp(tok[0], "frame")) {
			if (n != 3 || strncmp(tok[2], "output=", 7))
				die("frame needs <name> output=<id>");
			f->frames = grow(f->frames, f->n_frames, sizeof(*f->frames));
			cur = &f->frames[f->n_frames++];
			cur->name = strdup(tok[1]);
			cur->output = parse_u32(tok[2] + 7, "frame output");
			cur->line = lineno;
		} else if (!strcmp(tok[0], "layer")) {
			if (!cur)
				die("layer outside a frame");
			parse_layer(cur, tok, n);
		} else if (!strcmp(tok[0], "end")) {
			cur = NULL;
		} else {
			die("unknown record '%s'", tok[0]);
		}
	}
	free(line);
	if (!header)
		die("empty file");
	if (!f->origin)
		die("no origin line");
}

/* --- TEST_ONLY responder --------------------------------------------------- */

enum mode { MODE_PIPE, MODE_ACCEPT_ALL, MODE_REJECT_ALL };

struct responder {
	enum mode mode;
	FILE *requests;
	FILE *answers;
	const char *frame;
	unsigned long seq; /* test commits in this frame */
};

static const char *blend_word(int blend)
{
	switch (blend) {
	case XC_BLEND_NONE: return "opaque";
	case XC_BLEND_PREMULTI: return "premul";
	case XC_BLEND_COVERAGE: return "coverage";
	default: return "-";
	}
}

static int answer_commit(const struct xc_plane_state *st, size_t n, uint32_t flags, void *data)
{
	struct responder *r = data;
	FILE *o = r->requests;
	r->seq++;
	fprintf(o, "test frame=%s seq=%lu flags=0x%" PRIx32 " planes=%zu", r->frame, r->seq,
		flags, n);
	for (size_t i = 0; i < n; i++) {
		const struct xc_plane_state *s = &st[i];
		fprintf(o,
			" plane=%" PRIu32 " fb=%" PRIu64 " crtc=%" PRIu64
			" src16=%" PRIu64 ",%" PRIu64 ",%" PRIu64 ",%" PRIu64
			" dst=%" PRId64 ",%" PRId64 ",%" PRIu64 ",%" PRIu64
			" alpha=%" PRIu64 " blend=%s rotation=0x%" PRIx64 " zpos=%" PRId64,
			s->plane_id, s->fb_id, s->crtc_id, s->src_x, s->src_y, s->src_w, s->src_h,
			s->crtc_x, s->crtc_y, s->crtc_w, s->crtc_h, s->alpha, blend_word(s->blend),
			s->rotation, s->zpos);
	}
	fputc('\n', o);
	fflush(o);

	switch (r->mode) {
	case MODE_ACCEPT_ALL:
		return 0;
	case MODE_REJECT_ALL:
		return -EINVAL;
	case MODE_PIPE:
		break;
	}

	char buf[256];
	if (!fgets(buf, sizeof(buf), r->answers)) {
		fprintf(stderr, "liftoff-xcheck-driver: no answer to test seq=%lu of frame %s\n",
			r->seq, r->frame);
		exit(3);
	}
	char word[16] = "";
	if (sscanf(buf, "%15s", word) != 1)
		word[0] = '\0';
	if (!strcmp(word, "ok"))
		return 0;
	if (!strcmp(word, "einval"))
		return -EINVAL;
	if (!strcmp(word, "erange"))
		return -ERANGE;
	if (!strcmp(word, "enospc"))
		return -ENOSPC;
	fprintf(stderr, "liftoff-xcheck-driver: bad answer '%s' to test seq=%lu of frame %s\n",
		word, r->seq, r->frame);
	exit(3);
}

/* --- libliftoff run -------------------------------------------------------- */

static bool verbose;
static bool deadline_logged;

static void log_handler(enum liftoff_log_priority priority, const char *fmt, va_list args)
{
	char msg[512];
	vsnprintf(msg, sizeof(msg), fmt, args);
	if (strcasestr(msg, "deadline"))
		deadline_logged = true;
	if (verbose || priority == LIFTOFF_ERROR)
		fprintf(stderr, "libliftoff: %s\n", msg);
}

static int64_t now_ns(void)
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	return (int64_t)ts.tv_sec * 1000000000 + ts.tv_nsec;
}

static const struct xc_buffer *find_buffer(const struct file *f, uint32_t id)
{
	for (size_t i = 0; i < f->topo.n_buffers; i++)
		if (f->topo.buffers[i].id == id)
			return &f->topo.buffers[i];
	return NULL;
}

static void run_frame(const struct file *f, const struct frame *fr, struct responder *r,
		      int64_t timeout_ns)
{
	size_t n = fr->n_layers;
	r->frame = fr->name;
	r->seq = 0;
	deadline_logged = false;
	lineno = fr->line;

	/* Frames that name objects the file lacks are not run: core refuses
	 * them before planning too (design section 6, "skipped"). */
	const char *skip = "unknown-output";
	for (size_t i = 0; i < f->topo.n_outputs; i++)
		if (f->topo.outputs[i].id == fr->output)
			skip = NULL;
	for (size_t i = 0; !skip && i < n; i++)
		if (!find_buffer(f, fr->layers[i].buffer))
			skip = "unknown-buffer";
	if (skip) {
		printf("result frame=%s output=%" PRIu32 " layers=%zu apply=skipped:%s placed=0"
		       " planes=- deadline=0 tests=0 origin=%s\n",
		       fr->name, fr->output, n, skip, f->origin);
		fflush(stdout);
		return;
	}

	/* Layer zpos = rank of a stable sort by z: equal z keeps frame order,
	 * later on top, as composition-core stacks ties. */
	size_t *rank = calloc(n ? n : 1, sizeof(*rank));
	for (size_t i = 0; i < n; i++)
		for (size_t j = 0; j < n; j++)
			if (fr->layers[j].z < fr->layers[i].z ||
			    (fr->layers[j].z == fr->layers[i].z && j < i))
				rank[i]++;

	int fd = open("/dev/null", O_RDWR | O_CLOEXEC);
	if (fd < 0)
		die("open /dev/null: %s", strerror(errno));
	struct liftoff_device *dev = liftoff_device_create(fd);
	close(fd);
	if (!dev)
		die("liftoff_device_create failed");
	if (liftoff_device_register_all_planes(dev) != 0)
		die("liftoff_device_register_all_planes failed");
	struct liftoff_output *out = liftoff_output_create(dev, fr->output);
	if (!out)
		die("liftoff_output_create failed for output %u", fr->output);

	struct liftoff_layer **layers = calloc(n ? n : 1, sizeof(*layers));
	for (size_t i = 0; i < n; i++) {
		const struct layer *l = &fr->layers[i];
		struct liftoff_layer *ll = liftoff_layer_create(out);
		if (!ll)
			die("liftoff_layer_create failed");
		layers[i] = ll;
		int ret = 0;
		ret |= liftoff_layer_set_property(ll, "FB_ID", l->buffer);
		ret |= liftoff_layer_set_property(ll, "SRC_X", (uint64_t)l->src[0] << 16);
		ret |= liftoff_layer_set_property(ll, "SRC_Y", (uint64_t)l->src[1] << 16);
		ret |= liftoff_layer_set_property(ll, "SRC_W", (uint64_t)l->src[2] << 16);
		ret |= liftoff_layer_set_property(ll, "SRC_H", (uint64_t)l->src[3] << 16);
		ret |= liftoff_layer_set_property(ll, "CRTC_X", (uint64_t)l->dst[0]);
		ret |= liftoff_layer_set_property(ll, "CRTC_Y", (uint64_t)l->dst[1]);
		ret |= liftoff_layer_set_property(ll, "CRTC_W", (uint64_t)l->dst[2]);
		ret |= liftoff_layer_set_property(ll, "CRTC_H", (uint64_t)l->dst[3]);
		ret |= liftoff_layer_set_property(ll, "zpos", rank[i]);
		ret |= liftoff_layer_set_property(ll, "alpha", l->alpha);
		ret |= liftoff_layer_set_property(ll, "pixel blend mode", (uint64_t)l->blend);
		ret |= liftoff_layer_set_property(ll, "rotation", l->rotation);
		if (ret)
			die("liftoff_layer_set_property failed");
	}

	drmModeAtomicReq *req = drmModeAtomicAlloc();
	struct liftoff_output_apply_options opts = { 0 };
	opts.timeout_ns = timeout_ns;
	int64_t t0 = now_ns();
	int ret = liftoff_output_apply(out, req, 0, &opts);
	int64_t elapsed = now_ns() - t0;
	bool deadline = deadline_logged || elapsed >= timeout_ns;

	size_t placed = 0;
	printf("result frame=%s output=%" PRIu32 " layers=%zu", fr->name, fr->output, n);
	char *list = NULL;
	size_t list_len = 0;
	FILE *ls = open_memstream(&list, &list_len);
	for (size_t i = 0; i < n; i++) {
		struct liftoff_plane *pl = ret == 0 ? liftoff_layer_get_plane(layers[i]) : NULL;
		if (i)
			fputc(',', ls);
		if (pl) {
			placed++;
			fprintf(ls, "%" PRIu32, liftoff_plane_get_id(pl));
		} else if (ret != 0 || liftoff_layer_needs_composition(layers[i])) {
			fputs("composition", ls);
		} else {
			fputs("none", ls); /* invisible: alpha 0 */
		}
	}
	fclose(ls);
	if (ret == 0)
		printf(" apply=ok");
	else
		printf(" apply=error:%d", ret < 0 ? -ret : ret);
	printf(" placed=%zu planes=%s deadline=%d tests=%lu origin=%s\n", placed,
	       n ? list : "-", deadline ? 1 : 0, r->seq, f->origin);
	fflush(stdout);
	free(list);

	drmModeAtomicFree(req);
	for (size_t i = 0; i < n; i++)
		liftoff_layer_destroy(layers[i]);
	free(layers);
	free(rank);
	liftoff_output_destroy(out);
	liftoff_device_destroy(dev);
}

static void usage(void)
{
	fprintf(stderr,
		"usage: liftoff-xcheck-driver [--accept-all | --reject-all] [--request-fd N]\n"
		"           [--answer-fd N] [--timeout-ns N] [--verbose] FILE.xcheck\n");
	exit(2);
}

static int parse_fd(const char *s)
{
	char *end;
	long v = strtol(s, &end, 10);
	if (end == s || *end || v < 0 || v > INT_MAX)
		usage();
	return (int)v;
}

int main(int argc, char **argv)
{
	struct responder r = { .mode = MODE_PIPE };
	int request_fd = STDOUT_FILENO, answer_fd = STDIN_FILENO;
	/* Far beyond the 1 ms default, so the search runs to its end. */
	int64_t timeout_ns = INT_MAX;
	const char *file = NULL;

	for (int i = 1; i < argc; i++) {
		const char *a = argv[i];
		if (!strcmp(a, "--accept-all")) {
			r.mode = MODE_ACCEPT_ALL;
		} else if (!strcmp(a, "--reject-all")) {
			r.mode = MODE_REJECT_ALL;
		} else if (!strcmp(a, "--request-fd") && i + 1 < argc) {
			request_fd = parse_fd(argv[++i]);
		} else if (!strcmp(a, "--answer-fd") && i + 1 < argc) {
			answer_fd = parse_fd(argv[++i]);
		} else if (!strcmp(a, "--timeout-ns") && i + 1 < argc) {
			char *end;
			long long v = strtoll(argv[++i], &end, 10);
			if (*end || v <= 0 || v > INT_MAX)
				usage();
			timeout_ns = v;
		} else if (!strcmp(a, "--verbose")) {
			verbose = true;
		} else if (a[0] == '-' || file) {
			usage();
		} else {
			file = a;
		}
	}
	if (!file)
		usage();

	path = file;
	FILE *in = fopen(file, "r");
	if (!in) {
		fprintf(stderr, "liftoff-xcheck-driver: %s: %s\n", file, strerror(errno));
		return 1;
	}
	struct file f;
	parse_file(&f, in);
	fclose(in);

	if (request_fd == STDOUT_FILENO) {
		r.requests = stdout;
	} else if (!(r.requests = fdopen(request_fd, "w"))) {
		fprintf(stderr, "liftoff-xcheck-driver: request fd %d: %s\n", request_fd,
			strerror(errno));
		return 2;
	}
	if (r.mode == MODE_PIPE) {
		if (answer_fd == STDIN_FILENO)
			r.answers = stdin;
		else if (!(r.answers = fdopen(answer_fd, "r"))) {
			fprintf(stderr, "liftoff-xcheck-driver: answer fd %d: %s\n", answer_fd,
				strerror(errno));
			return 2;
		}
	}

	if (xc_fake_install(&f.topo) != 0) {
		fprintf(stderr, "liftoff-xcheck-driver: object ids too large\n");
		return 1;
	}
	xc_fake_set_commit_handler(answer_commit, &r);
	liftoff_log_set_priority(LIFTOFF_DEBUG);
	liftoff_log_set_handler(log_handler);

	for (size_t i = 0; i < f.n_frames; i++)
		run_frame(&f, &f.frames[i], &r, timeout_ns);
	return 0;
}
