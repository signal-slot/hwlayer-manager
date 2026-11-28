//! Host test: how the two-plane backend (`ce_twoplane.c`) and the engine
//! above it treat plane geometry at the edges of the panel, behind a fake
//! driver and an 800x480 panel (the fake of `tests/c/twoplane_test.c`,
//! reduced to what these cases need).
//!
//! The backend declares both planes `full_output_only` and `can_scale = 0`:
//! the driver programs a plane at the buffer's size from the top left. The
//! cases are: a layer exactly the size of the panel, a layer at a zero
//! origin that does not cover the panel, a layer past the right or bottom
//! edge, a layer entirely off the panel, a zero-sized layer, a source
//! rectangle larger than its buffer, and a scale request. Each case asserts
//! accepted, or refused with an error, and for a refusal that nothing was
//! written to the driver and no commit counted (so no latch can follow).
//!
//! Documented (`composition_engine.h`, `ce_twoplane.c`): the destination is
//! clipped to the output; a layer entirely off the output has no
//! assignment; `full_output_only` and scaling are plane capabilities the
//! engine checks. Where the header does not fix the status or the reason
//! bits, the case says "current behaviour" and pins what happens now.
//!
//! The C program is embedded below and written to the target's temporary
//! directory. This is a host reference only, not a board result.

use std::path::PathBuf;
use std::process::{Command, Output};

const FLAGS: [&str; 5] = ["-std=c11", "-Wall", "-Wextra", "-Werror", "-pedantic"];

fn backend_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn engine_include() -> PathBuf {
    backend_dir().join("../../crates/composition-ffi/include")
}

fn run(command: &mut Command, what: &str) -> Output {
    let output = command
        .output()
        .unwrap_or_else(|e| panic!("cannot run {what}: {e}"));
    assert!(
        output.status.success(),
        "{what} failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Builds the engine's static library in a nested cargo (as `twoplane.rs`
/// does) and returns its path from Cargo's JSON messages.
fn build_static_library() -> PathBuf {
    let mut build = Command::new(env!("CARGO"));
    build
        .args(["build", "--lib", "-p", "composition-ffi-staticlib"])
        .arg("--message-format=json-render-diagnostics")
        .current_dir(backend_dir());
    if !cfg!(debug_assertions) {
        build.arg("--release");
    }
    let output = run(&mut build, "cargo build of the static library");
    let messages = String::from_utf8_lossy(&output.stdout);
    messages
        .lines()
        .filter(|line| line.contains("\"reason\":\"compiler-artifact\""))
        .filter_map(|line| line.split_once("\"filenames\":["))
        .filter_map(|(_, rest)| rest.split_once(']'))
        .flat_map(|(list, _)| list.split('"').skip(1).step_by(2).map(str::to_owned))
        .map(PathBuf::from)
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "libcomposition_engine.a")
        })
        .unwrap_or_else(|| panic!("cargo reported no libcomposition_engine.a:\n{messages}"))
}

#[test]
fn geometry_at_the_edges_of_the_panel() {
    let library = build_static_library();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let source = dir.join("geometry_bounds_test.c");
    std::fs::write(&source, C_PROGRAM).unwrap();
    let program = dir.join("composition_geometry_bounds");
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    run(
        Command::new(cc)
            .args(FLAGS)
            .arg("-I")
            .arg(engine_include())
            .arg("-I")
            .arg(backend_dir())
            .arg(backend_dir().join("ce_twoplane.c"))
            .arg(&source)
            .arg(&library)
            .args(["-lpthread", "-ldl", "-lm"])
            .arg("-o")
            .arg(&program),
        "the C compiler",
    );
    let output = run(&mut Command::new(&program), "the geometry C test");
    let console = String::from_utf8_lossy(&output.stdout);
    print!("{console}");
    assert_eq!(console.matches("ok ").count(), CASES, "{console}");
}

/// The number of `ok` lines the C program prints.
const CASES: usize = 9;

const C_PROGRAM: &str = r##"
#include "ce_twoplane.h"
#include "composition_engine.h"

#include <stdio.h>
#include <string.h>

#define CHECK(cond) \
	do { \
		if (!(cond)) { \
			fprintf(stderr, "line %d: check failed: %s\n", __LINE__, #cond); \
			return 1; \
		} \
	} while (0)

enum { WIDTH = 800, HEIGHT = 480 };
enum { APP = 1, APP_SMALL = 2, SAFETY_0 = 10, SAFETY_1 = 11 };

struct regs {
	struct ce_twoplane_buf app;
	bool app_on;
	unsigned safety;
};

static struct fake {
	struct regs shadow, active;
	uint32_t commits, latched;
	unsigned writes;
	ce_twoplane_vsync_cb_t cb;
	void *cb_user;
	uint32_t safety_mem[2][16];
	uint32_t app_mem[16];
	uint32_t small_mem[16];
} fake;

static void frame_start(void)
{
	if (fake.latched != fake.commits) {
		fake.active = fake.shadow;
		fake.latched = fake.commits;
	}
	if (fake.cb) {
		fake.cb(&fake, fake.cb_user);
	}
}

static void wrote(void)
{
	fake.commits++;
	fake.writes++;
}

static bool hw_format_ok(uint32_t f) { (void)f; return true; }
static int hw_set_app(const void *d, const struct ce_twoplane_buf *b)
{
	(void)d;
	fake.shadow.app = *b;
	wrote();
	return 0;
}
static int hw_enable(const void *d, bool on)
{
	(void)d;
	fake.shadow.app_on = on;
	wrote();
	return 0;
}
static int hw_flip(const void *d)
{
	(void)d;
	fake.shadow.safety ^= 1u;
	wrote();
	return 0;
}
static void *hw_safety_buffer(const void *d, uint32_t *w, uint32_t *h, uint32_t *s)
{
	(void)d;
	*w = WIDTH;
	*h = HEIGHT;
	*s = WIDTH * 4;
	return fake.safety_mem[fake.shadow.safety ^ 1u];
}
static void *hw_safety_front(const void *d)
{
	(void)d;
	return fake.safety_mem[fake.active.safety];
}
static int hw_premul(const void *d, bool on) { (void)d; (void)on; return 0; }
static uint32_t hw_commits(const void *d) { (void)d; return fake.commits; }
static uint32_t hw_latched(const void *d) { (void)d; return fake.latched; }
static void hw_set_cb(const void *d, ce_twoplane_vsync_cb_t cb, void *u)
{
	(void)d;
	fake.cb = cb;
	fake.cb_user = u;
}

static const struct ce_twoplane_hw hw = {
	.app_format_ok = hw_format_ok,
	.set_app_buffer = hw_set_app,
	.enable_app_plane = hw_enable,
	.safety_flip = hw_flip,
	.safety_buffer = hw_safety_buffer,
	.safety_front = hw_safety_front,
	.safety_premultiplied = hw_premul,
	.commit_count = hw_commits,
	.latched_count = hw_latched,
	.set_vsync_callback = hw_set_cb,
};

static struct ce_twoplane_record table[4];

static unsigned int port_lock(void) { return 0; }
static void port_unlock(unsigned int k) { (void)k; }
static void port_wake(void *a) { (void)a; }
static const struct ce_twoplane_record *port_lookup(void *a, uint32_t id)
{
	(void)a;
	for (size_t i = 0; i < sizeof(table) / sizeof(table[0]); i++) {
		if (table[i].id == id) {
			return &table[i];
		}
	}
	return NULL;
}
static bool port_ready(const void *d) { (void)d; return true; }

static const struct ce_twoplane_port port = {
	.irq_lock = port_lock,
	.irq_unlock = port_unlock,
	.wake = port_wake,
	.lookup = port_lookup,
	.device_ready = port_ready,
};

static struct ce_twoplane backend;
static _Alignas(CE_ENGINE_ALIGN) unsigned char storage[CE_ENGINE_SIZE];
static ce_engine *engine;

/* A fresh driver, backend and engine; APP is the panel's size, APP_SMALL
 * is 400x240 (both RGB565, in the integrator's table and the engine's). */
static int start(void)
{
	memset(&fake, 0, sizeof(fake));
	table[0] = (struct ce_twoplane_record){APP, 1, (uintptr_t)fake.app_mem, WIDTH, HEIGHT,
					       WIDTH * 2, CE_FORMAT_RGB565};
	table[1] = (struct ce_twoplane_record){APP_SMALL, 1, (uintptr_t)fake.small_mem, 400, 240,
					       400 * 2, CE_FORMAT_RGB565};
	table[2] = (struct ce_twoplane_record){SAFETY_0, 0, (uintptr_t)fake.safety_mem[0], WIDTH,
					       HEIGHT, WIDTH * 4, CE_FORMAT_ARGB8888};
	table[3] = (struct ce_twoplane_record){SAFETY_1, 0, (uintptr_t)fake.safety_mem[1], WIDTH,
					       HEIGHT, WIDTH * 4, CE_FORMAT_ARGB8888};
	CHECK(ce_twoplane_init(&backend, &hw, &port, &fake) == 0);
	engine = ce_engine_init(storage, sizeof(storage), &ce_twoplane_ops, &backend);
	CHECK(engine != NULL);
	for (size_t i = 0; i < 4; i++) {
		const ce_buffer_info info = {CE_MODIFIER_LINEAR, table[i].width, table[i].height,
					     table[i].format};
		CHECK(ce_register_buffer(engine, table[i].id, &info) == CE_OK);
		CHECK(ce_finish_write(engine, table[i].id) == CE_OK);
	}
	return 0;
}

static ce_layer layer(uint32_t buffer, ce_rect src, ce_rect dst)
{
	return (ce_layer){.buffer = buffer, .source = src, .destination = dst,
			  .alpha = CE_ALPHA_OPAQUE, .blend = CE_BLEND_OPAQUE,
			  .transform = CE_TRANSFORM_NORMAL};
}

static ce_layer safety(void)
{
	ce_layer l = layer(SAFETY_0, (ce_rect){0, 0, WIDTH, HEIGHT},
			   (ce_rect){0, 0, WIDTH, HEIGHT});

	l.z_order = 10;
	l.blend = CE_BLEND_PREMULTIPLIED;
	return l;
}

/* Presents the safety layer and app (below it); returns ce_present's status. */
static ce_status present(ce_layer app, ce_error_info *info)
{
	ce_layer layers[2] = {safety(), app};
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};
	uint64_t sub;

	memset(info, 0, sizeof(*info));
	return ce_present(engine, &frame, &sub, info);
}

/* A refusal: the status, nothing written, no commit counted, and no event
 * after a frame start, so nothing latched. */
static int refused(ce_status got, ce_status want)
{
	ce_event e;

	CHECK(got == want);
	CHECK(fake.writes == 0 && fake.commits == 0);
	frame_start();
	CHECK(fake.latched == 0);
	CHECK(ce_poll_event(engine, &e) == 0);
	return 0;
}

/* An acceptance: after a frame start every commit is latched and the next
 * event is PRESENTED. */
static int accepted(ce_status got, bool app_on)
{
	ce_event e;

	CHECK(got == CE_OK);
	CHECK(fake.shadow.app_on == app_on);
	frame_start();
	CHECK(fake.latched == fake.commits);
	CHECK(ce_poll_event(engine, &e) == 1 && e.kind == CE_EVENT_PRESENTED);
	return 0;
}

static const ce_rect FULL = {0, 0, WIDTH, HEIGHT};

/* The backend's capabilities, which the cases below rest on. */
static int declared_capabilities(void)
{
	ce_capabilities caps;

	CHECK(start() == 0);
	memset(&caps, 0, sizeof(caps));
	ce_twoplane_ops.capabilities(&backend, &caps);
	CHECK(caps.output_count == 1);
	CHECK(caps.outputs[0].width == WIDTH && caps.outputs[0].height == HEIGHT);
	CHECK(caps.plane_count == 2);
	CHECK(caps.planes[0].can_scale == 0 && caps.planes[1].can_scale == 0);
	CHECK(caps.planes[0].full_output_only == 1 && caps.planes[1].full_output_only == 1);
	return 0;
}

/* Accepted: a layer exactly the size of the panel at the zero origin; the
 * plane is programmed from the buffer. */
static int exact_panel(void)
{
	ce_error_info info;

	CHECK(start() == 0);
	CHECK(accepted(present(layer(APP, FULL, FULL), &info), true) == 0);
	CHECK(fake.active.app.width == WIDTH && fake.active.app.height == HEIGHT);
	CHECK(fake.active.app.pa == (uintptr_t)fake.app_mem);
	return 0;
}

/* A smaller layer at the zero origin does not cover the panel; the planes
 * are full_output_only (declared above), so it is refused. The status and
 * reason bit are current behaviour. */
static int zero_origin(void)
{
	ce_error_info info;

	CHECK(start() == 0);
	CHECK(refused(present(layer(APP_SMALL, (ce_rect){0, 0, 400, 240},
				    (ce_rect){0, 0, 400, 240}), &info),
		      CE_ERR_NO_PLANE) == 0);
	CHECK(info.status == CE_ERR_NO_PLANE);
	CHECK((info.reasons & CE_REJECT_FULL_OUTPUT) != 0);
	return 0;
}

/* The destination is clipped to the output (documented): 400 pixels past
 * the right edge clips to 400x480, not the whole panel, so it is refused
 * (status and reason: current behaviour). */
static int past_right_edge(void)
{
	ce_error_info info;

	CHECK(start() == 0);
	CHECK(refused(present(layer(APP, FULL, (ce_rect){400, 0, WIDTH, HEIGHT}), &info),
		      CE_ERR_NO_PLANE) == 0);
	CHECK((info.reasons & CE_REJECT_FULL_OUTPUT) != 0);
	return 0;
}

static int past_bottom_edge(void)
{
	ce_error_info info;

	CHECK(start() == 0);
	CHECK(refused(present(layer(APP, FULL, (ce_rect){0, 240, WIDTH, HEIGHT}), &info),
		      CE_ERR_NO_PLANE) == 0);
	CHECK((info.reasons & CE_REJECT_FULL_OUTPUT) != 0);
	return 0;
}

/* A layer entirely off the output has no assignment (documented). Current
 * behaviour: the frame is accepted with the safety layer alone and the
 * application plane stays off. No write is needed (the safety buffer is
 * already on the plane), so the commit count stays 0 and the latch is
 * reported at once. */
static int entirely_off(void)
{
	ce_error_info info;

	CHECK(start() == 0);
	CHECK(accepted(present(layer(APP, FULL, (ce_rect){WIDTH, 0, WIDTH, HEIGHT}), &info),
		       false) == 0);
	CHECK(fake.writes == 0);
	return 0;
}

/* A zero-sized destination: CE_ERR_INVALID_DESTINATION. */
static int zero_sized(void)
{
	ce_error_info info;

	CHECK(start() == 0);
	CHECK(refused(present(layer(APP, FULL, (ce_rect){0, 0, 0, 0}), &info),
		      CE_ERR_INVALID_DESTINATION) == 0);
	return 0;
}

/* A source rectangle larger than its buffer: CE_ERR_INVALID_SOURCE. */
static int source_larger_than_buffer(void)
{
	ce_error_info info;

	CHECK(start() == 0);
	CHECK(refused(present(layer(APP, (ce_rect){0, 0, WIDTH + 1, HEIGHT}, FULL), &info),
		      CE_ERR_INVALID_SOURCE) == 0);
	return 0;
}

/* The backend says can_scale = 0: a half-size source onto the whole panel
 * is refused, with CE_REJECT_SCALING among the reasons (the exact status
 * and the other bits: current behaviour). */
static int scale_request(void)
{
	ce_error_info info;

	CHECK(start() == 0);
	CHECK(refused(present(layer(APP_SMALL, (ce_rect){0, 0, 400, 240}, FULL), &info),
		      CE_ERR_NO_PLANE) == 0);
	CHECK((info.reasons & CE_REJECT_SCALING) != 0);
	return 0;
}

#define RUN(t) \
	do { \
		if (t() != 0) { \
			fprintf(stderr, "FAILED %s\n", #t); \
			return 1; \
		} \
		printf("ok %s\n", #t); \
	} while (0)

int main(void)
{
	RUN(declared_capabilities);
	RUN(exact_panel);
	RUN(zero_origin);
	RUN(past_right_edge);
	RUN(past_bottom_edge);
	RUN(entirely_off);
	RUN(zero_sized);
	RUN(source_larger_than_buffer);
	RUN(scale_request);
	return 0;
}
"##;
