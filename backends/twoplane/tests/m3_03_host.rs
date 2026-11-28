//! HOST SIMULATION of the M3-03 board tests (`docs/m3-03-board-test-design.md`):
//! the safety layer and the producer's failure behaviour on the two-plane
//! backend. This is NOT a board result. Nothing here ran on an AM62P, a DSS
//! or an R5F; the "panel" is a fake driver defined in the C source below, in
//! which the test decides when a frame start happens (and whether its
//! interrupt runs). It shows that the backend's and the engine's logic
//! holds the properties the board scenes will then try on the hardware, and
//! nothing about real concurrency, a real GO bit, memory or caches.
//!
//! The cases follow the design in its order: scene 9 (the producer stops at
//! three places and restarts), scene 10 (a safety flip on top of an in-flight
//! application commit, swept over the frame start's position), scene 11
//! (back-to-back commits within a frame, and split by a frame start), the
//! producer's bad hand-overs and the latch timeout, and a seeded interleaving
//! of producer calls with latch/vsync events. Scene 12 (cache maintenance)
//! has no host content: the fake reads no memory. Pass criteria, offsets and
//! repetitions of the board are the M3 test plan's, not this file's: the
//! numbers below are those of the host.
//!
//! The fake panel is modelled on `tests/twoplane.rs` and
//! `tests/c/twoplane_test.c` (shadow registers, a frame start that copies
//! them to the active ones and makes the latched count equal the commit
//! count, a port lock that defers the interrupt), written again here so that
//! those files stay as they are. The C program is compiled with the system C
//! compiler together with `ce_twoplane.c` and the engine's static library.
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::OnceLock;

const FLAGS: [&str; 5] = ["-std=c11", "-Wall", "-Wextra", "-Werror", "-pedantic"];

fn backend_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn compiler() -> String {
    std::env::var("CC").unwrap_or_else(|_| "cc".into())
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

/// The first `libcomposition_engine.a` named in cargo's JSON messages.
fn static_library_in(messages: &str) -> Option<PathBuf> {
    messages
        .lines()
        .filter(|line| line.contains("\"reason\":\"compiler-artifact\""))
        .filter_map(|line| line.split_once("\"filenames\":["))
        .flat_map(|(_, rest)| {
            let list = rest.split_once(']').map_or(rest, |(list, _)| list);
            list.split('"')
                .skip(1)
                .step_by(2)
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "libcomposition_engine.a")
        })
}

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
    let library = static_library_in(&messages)
        .unwrap_or_else(|| panic!("cargo reported no libcomposition_engine.a:\n{messages}"));
    assert!(
        library.exists(),
        "no static library at {}",
        library.display()
    );
    library
}

/// The simulation program, built once for all the tests of this file.
fn program() -> &'static PathBuf {
    static PROGRAM: OnceLock<PathBuf> = OnceLock::new();
    PROGRAM.get_or_init(|| {
        let library = build_static_library();
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        let source = dir.join("m3_03_host.c");
        std::fs::write(&source, C_SOURCE).unwrap();
        let program = dir.join("m3_03_host");
        run(
            Command::new(compiler())
                .args(FLAGS)
                .arg("-I")
                .arg(backend_dir().join("../../crates/composition-ffi/include"))
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
        program
    })
}

fn scenario(name: &str) {
    let output = run(Command::new(program()).arg(name), name);
    let console = String::from_utf8_lossy(&output.stdout);
    print!("{console}");
    assert!(console.contains(&format!("ok {name}")), "{console}");
}

/// Scene 9: the producer stops after `ce_begin_write`, after
/// `ce_finish_write` and after `ce_present`, and restarts.
#[test]
fn scene9_producer_stops_and_restarts() {
    scenario("scene9_producer_stops");
}

/// Scene 10: a safety flip on top of an in-flight application commit, with
/// the frame start before, inside and after the second commit's writes,
/// with and without its interrupt.
#[test]
fn scene10_safety_flip_during_app_commit() {
    scenario("scene10_flip_sweep");
}

/// Scene 11: several presents within a frame, and the same split by a frame
/// start, with the producer rule's negative control.
#[test]
fn scene11_back_to_back_commits() {
    scenario("scene11_bursts");
}

/// The producer hands over a bad buffer, or never returns one; the latch
/// timeout then a replacement.
#[test]
fn producer_failure_does_not_touch_the_safety_plane() {
    scenario("producer_bad_handover");
    scenario("producer_latch_timeout");
}

/// Producer calls, presents and latch/vsync events interleaved by a seeded
/// generator.
#[test]
fn interleaved_producer_and_latch_events() {
    scenario("interleaved_seeded");
}

const C_SOURCE: &str = r##"
/* SPDX-License-Identifier: Apache-2.0
 * Host simulation of the M3-03 board tests behind a fake panel. Not a board
 * result. See m3_03_host.rs.
 */
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

#define TRY(call) \
	do { \
		if ((call) != 0) { \
			return 1; \
		} \
	} while (0)

enum { WIDTH = 800, HEIGHT = 480 };

enum {
	APP_A = 1,
	APP_B = 2,
	APP_NO_PA = 3,    /* in the table, address 0 */
	APP_UNLISTED = 4, /* registered with the engine, not in the table */
	APP_XRGB = 5,     /* XRGB8888, which the fake driver may refuse */
	SAFETY_0 = 10,
	SAFETY_1 = 11,
	SAFETY_OTHER = 12, /* ARGB8888, not the driver's */
	NOT_REGISTERED = 99,
};

/* ---- The fake panel ---- */

struct regs {
	struct ce_twoplane_buf app;
	bool app_on;
	unsigned safety; /* index into safety_mem */
};

static struct fake {
	bool formats_ok[3];
	struct regs shadow; /* what the writes left */
	struct regs active; /* what the panel scans */
	uint32_t commits;
	uint32_t latched;
	bool hold; /* a frame start latches nothing */
	unsigned frame_starts;
	unsigned writes;
	ce_twoplane_vsync_cb_t cb;
	void *cb_user;
	/* scripted frame starts, counted from now (0: none) */
	unsigned start_after_write;
	unsigned start_at_format_ok;
	unsigned start_at_commit_count;
	bool start_runs_isr;
	unsigned lock_depth;
	unsigned max_lock_depth;
	bool bad_unlock;
	bool isr_deferred;
	unsigned wakes;
	uint32_t safety_mem[2][16];
	uint32_t app_mem[2][16];
	uint32_t other_mem[16];
	uint32_t xrgb_mem[16];
} fake;

static const char wake_token[] = "wake";
static const char lookup_token[] = "lookup";
static unsigned wrong_port_arg, wrong_dev;

static void run_isr(void)
{
	if (fake.cb == NULL) {
		return;
	}
	if (fake.lock_depth > 0) {
		fake.isr_deferred = true; /* masked: runs at irq_unlock */
		return;
	}
	fake.cb(&fake, fake.cb_user);
}

static void frame_start(bool isr)
{
	fake.frame_starts++;
	if (!fake.hold && fake.latched != fake.commits) {
		fake.active = fake.shadow;
		fake.latched = fake.commits;
	}
	if (isr) {
		run_isr();
	}
}

static void check_dev(const void *dev)
{
	if (dev != &fake) {
		wrong_dev++;
	}
}

static void wrote(void)
{
	fake.commits++;
	fake.writes++;
	if (fake.start_after_write > 0 && --fake.start_after_write == 0) {
		frame_start(fake.start_runs_isr);
	}
}

static int format_index(uint32_t format)
{
	switch (format) {
	case CE_FORMAT_RGB565:
		return 0;
	case CE_FORMAT_XRGB8888:
		return 1;
	case CE_FORMAT_ARGB8888:
		return 2;
	default:
		return -1;
	}
}

static bool hw_app_format_ok(uint32_t format)
{
	int i = format_index(format);

	if (fake.start_at_format_ok > 0 && --fake.start_at_format_ok == 0) {
		frame_start(fake.start_runs_isr);
	}
	return i >= 0 && fake.formats_ok[i];
}

static int hw_set_app_buffer(const void *dev, const struct ce_twoplane_buf *buf)
{
	check_dev(dev);
	fake.shadow.app = *buf;
	wrote();
	return 0;
}

static int hw_enable_app_plane(const void *dev, bool enable)
{
	check_dev(dev);
	fake.shadow.app_on = enable;
	wrote();
	return 0;
}

static int hw_safety_flip(const void *dev)
{
	check_dev(dev);
	fake.shadow.safety ^= 1u;
	wrote();
	return 0;
}

static void *hw_safety_buffer(const void *dev, uint32_t *width, uint32_t *height,
			      uint32_t *stride_bytes)
{
	check_dev(dev);
	*width = WIDTH;
	*height = HEIGHT;
	*stride_bytes = WIDTH * 4;
	return fake.safety_mem[fake.shadow.safety ^ 1u];
}

static void *hw_safety_front(const void *dev)
{
	check_dev(dev);
	return fake.safety_mem[fake.active.safety];
}

static int hw_safety_premultiplied(const void *dev, bool on)
{
	(void)on;
	check_dev(dev);
	return 0;
}

static uint32_t hw_commit_count(const void *dev)
{
	check_dev(dev);
	if (fake.start_at_commit_count > 0 && --fake.start_at_commit_count == 0) {
		frame_start(fake.start_runs_isr);
	}
	return fake.commits;
}

static uint32_t hw_latched_count(const void *dev)
{
	check_dev(dev);
	return fake.latched;
}

static void hw_set_vsync_callback(const void *dev, ce_twoplane_vsync_cb_t cb, void *user)
{
	check_dev(dev);
	fake.cb = cb;
	fake.cb_user = user;
}

static const struct ce_twoplane_hw hw = {
	.app_format_ok = hw_app_format_ok,
	.set_app_buffer = hw_set_app_buffer,
	.enable_app_plane = hw_enable_app_plane,
	.safety_flip = hw_safety_flip,
	.safety_buffer = hw_safety_buffer,
	.safety_front = hw_safety_front,
	.safety_premultiplied = hw_safety_premultiplied,
	.commit_count = hw_commit_count,
	.latched_count = hw_latched_count,
	.set_vsync_callback = hw_set_vsync_callback,
};

static struct ce_twoplane_record table[8];
static size_t table_len;

static unsigned int port_irq_lock(void)
{
	unsigned int key = fake.lock_depth++;

	if (fake.lock_depth > fake.max_lock_depth) {
		fake.max_lock_depth = fake.lock_depth;
	}
	return key;
}

static void port_irq_unlock(unsigned int key)
{
	if (fake.lock_depth == 0 || key != fake.lock_depth - 1) {
		fake.bad_unlock = true;
		return;
	}
	fake.lock_depth = key;
	if (fake.lock_depth == 0 && fake.isr_deferred) {
		fake.isr_deferred = false;
		run_isr();
	}
}

static void port_wake(void *arg)
{
	if (arg != wake_token) {
		wrong_port_arg++;
	}
	fake.wakes++;
}

static const struct ce_twoplane_record *port_lookup(void *arg, uint32_t id)
{
	if (arg != lookup_token) {
		wrong_port_arg++;
	}
	for (size_t i = 0; i < table_len; i++) {
		if (table[i].id == id) {
			return &table[i];
		}
	}
	return NULL;
}

static bool port_device_ready(const void *dev)
{
	check_dev(dev);
	return true;
}

static const struct ce_twoplane_port port = {
	.irq_lock = port_irq_lock,
	.irq_unlock = port_irq_unlock,
	.wake = port_wake,
	.wake_arg = (void *)wake_token,
	.lookup = port_lookup,
	.lookup_arg = (void *)lookup_token,
	.device_ready = port_device_ready,
};

static void add_record(uint32_t id, uint32_t seq, uintptr_t pa, uint32_t format)
{
	uint32_t bpp = format == CE_FORMAT_RGB565 ? 2 : 4;

	table[table_len++] = (struct ce_twoplane_record){
		.id = id,
		.seq = seq,
		.pa = pa,
		.width = WIDTH,
		.height = HEIGHT,
		.stride_bytes = WIDTH * bpp,
		.format = format,
	};
}

static bool port_misused(void)
{
	bool bad = fake.lock_depth != 0 || fake.max_lock_depth > 1 || fake.bad_unlock ||
		   fake.isr_deferred || wrong_dev != 0 || wrong_port_arg != 0;

	if (bad) {
		fprintf(stderr, "port misused\n");
	}
	return bad;
}

/* ---- The engine, the event log ---- */

static struct ce_twoplane backend;
static _Alignas(CE_ENGINE_ALIGN) unsigned char storage[CE_ENGINE_SIZE];
static ce_engine *engine;

static struct {
	uint8_t kind;
	uint64_t sub;
	uint32_t buf;
} log_[8192];
static size_t nlog;

static void reset(void)
{
	memset(&fake, 0, sizeof(fake));
	fake.formats_ok[0] = fake.formats_ok[1] = fake.formats_ok[2] = true;
	wrong_port_arg = wrong_dev = 0;
	table_len = 0;
	nlog = 0;
	add_record(APP_A, 1, (uintptr_t)fake.app_mem[0], CE_FORMAT_RGB565);
	add_record(APP_B, 1, (uintptr_t)fake.app_mem[1], CE_FORMAT_RGB565);
	add_record(APP_NO_PA, 1, 0, CE_FORMAT_RGB565);
	add_record(APP_XRGB, 1, (uintptr_t)fake.xrgb_mem, CE_FORMAT_XRGB8888);
	add_record(SAFETY_0, 0, (uintptr_t)fake.safety_mem[0], CE_FORMAT_ARGB8888);
	add_record(SAFETY_1, 0, (uintptr_t)fake.safety_mem[1], CE_FORMAT_ARGB8888);
	add_record(SAFETY_OTHER, 0, (uintptr_t)fake.other_mem, CE_FORMAT_ARGB8888);
}

static int start_engine(void)
{
	static const struct {
		uint32_t id;
		uint32_t format;
	} buffers[] = {
		{APP_A, CE_FORMAT_RGB565},      {APP_B, CE_FORMAT_RGB565},
		{APP_NO_PA, CE_FORMAT_RGB565},  {APP_UNLISTED, CE_FORMAT_RGB565},
		{APP_XRGB, CE_FORMAT_XRGB8888}, {SAFETY_0, CE_FORMAT_ARGB8888},
		{SAFETY_1, CE_FORMAT_ARGB8888}, {SAFETY_OTHER, CE_FORMAT_ARGB8888},
	};

	CHECK(CE_ABI_CHECK());
	CHECK(ce_twoplane_init(&backend, &hw, &port, &fake) == 0);
	engine = ce_engine_init(storage, sizeof(storage), &ce_twoplane_ops, &backend);
	CHECK(engine != NULL);
	for (size_t i = 0; i < sizeof(buffers) / sizeof(buffers[0]); i++) {
		const ce_buffer_info info = {CE_MODIFIER_LINEAR, WIDTH, HEIGHT, buffers[i].format};

		CHECK(ce_register_buffer(engine, buffers[i].id, &info) == CE_OK);
		CHECK(ce_finish_write(engine, buffers[i].id) == CE_OK);
	}
	return 0;
}

static ce_layer app_layer(uint32_t buffer)
{
	return (ce_layer){
		.buffer = buffer,
		.source = {0, 0, WIDTH, HEIGHT},
		.destination = {0, 0, WIDTH, HEIGHT},
		.z_order = 0,
		.alpha = CE_ALPHA_OPAQUE,
		.blend = CE_BLEND_OPAQUE,
		.transform = CE_TRANSFORM_NORMAL,
	};
}

static ce_layer safety_layer(uint32_t buffer)
{
	ce_layer l = app_layer(buffer);

	l.z_order = 10;
	l.blend = CE_BLEND_PREMULTIPLIED;
	return l;
}

static ce_status present2(uint32_t app, uint32_t safety, uint64_t *submission)
{
	ce_layer layers[2] = {safety_layer(safety), app_layer(app)};
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};

	return ce_present(engine, &frame, submission, NULL);
}

static ce_status present1(ce_layer layer, uint64_t *submission)
{
	ce_frame frame = {.layers = &layer, .layer_count = 1, .output = 0};

	return ce_present(engine, &frame, submission, NULL);
}

/* Polls every event into the log. */
static int drain(void)
{
	ce_event e;
	int32_t rc;

	while ((rc = ce_poll_event(engine, &e)) == 1) {
		CHECK(nlog < sizeof(log_) / sizeof(log_[0]));
		log_[nlog].kind = e.kind;
		log_[nlog].sub = e.submission;
		log_[nlog].buf = e.buffer;
		nlog++;
	}
	CHECK(rc == 0);
	return 0;
}

static unsigned count_sub(uint8_t kind, uint64_t sub)
{
	unsigned n = 0;

	for (size_t i = 0; i < nlog; i++) {
		n += log_[i].kind == kind && log_[i].sub == sub;
	}
	return n;
}

static unsigned count_release(uint32_t buf)
{
	unsigned n = 0;

	for (size_t i = 0; i < nlog; i++) {
		n += log_[i].kind == CE_EVENT_RELEASED && log_[i].buf == buf;
	}
	return n;
}

static long find_sub(uint8_t kind, uint64_t sub)
{
	for (size_t i = 0; i < nlog; i++) {
		if (log_[i].kind == kind && log_[i].sub == sub) {
			return (long)i;
		}
	}
	return -1;
}

static long find_release(uint32_t buf)
{
	for (size_t i = 0; i < nlog; i++) {
		if (log_[i].kind == CE_EVENT_RELEASED && log_[i].buf == buf) {
			return (long)i;
		}
	}
	return -1;
}

/* Events other than presented/dropped/released in the log: none expected. */
static bool only_ordinary_events(void)
{
	for (size_t i = 0; i < nlog; i++) {
		if (log_[i].kind != CE_EVENT_PRESENTED && log_[i].kind != CE_EVENT_DROPPED &&
		    log_[i].kind != CE_EVENT_RELEASED) {
			return false;
		}
	}
	return true;
}

/* The submission reached exactly one end, shown or dropped. */
static bool one_end(uint64_t sub)
{
	return count_sub(CE_EVENT_PRESENTED, sub) + count_sub(CE_EVENT_DROPPED, sub) == 1;
}

/* The panel scans what the writes left: nothing waits for a latch. */
static bool panel_caught_up(void)
{
	return fake.latched == fake.commits && fake.active.app_on == fake.shadow.app_on &&
	       fake.active.safety == fake.shadow.safety &&
	       fake.active.app.pa == fake.shadow.app.pa;
}

/* ---- Scene 9: the producer stops and restarts ---- */

static int scene9_producer_stops(void)
{
	uint64_t s1, s2, s3, s4, s5;
	uint32_t commits;
	ce_event e;

	TRY(start_engine());
	CHECK(present2(APP_A, SAFETY_0, &s1) == CE_OK);
	frame_start(true);
	TRY(drain());
	CHECK(count_sub(CE_EVENT_PRESENTED, s1) == 1);

	/* (a) the producer stops between ce_begin_write and ce_finish_write. */
	CHECK(ce_begin_write(engine, APP_B) == CE_OK);
	CHECK(ce_buffer_state(engine, APP_B) == CE_BUFFER_WRITABLE);
	commits = fake.commits;
	nlog = 0;
	CHECK(present2(APP_B, SAFETY_0, &s2) == CE_ERR_BUFFER_NOT_READY);
	CHECK(fake.commits == commits && fake.writes == commits);
	CHECK(ce_buffer_state(engine, APP_B) == CE_BUFFER_WRITABLE);
	for (unsigned i = 0; i < 5; i++) {
		frame_start(true);
	}
	CHECK(ce_poll_event(engine, &e) == 0);
	CHECK(fake.commits == commits && panel_caught_up());
	CHECK(fake.active.app.pa == (uintptr_t)fake.app_mem[0]);

	/* The safety plane flips while the producer is stopped mid-draw: one
	 * commit, taken at the first frame start, the old safety buffer released
	 * after it, the half-drawn buffer untouched.
	 */
	CHECK(present2(APP_A, SAFETY_1, &s2) == CE_OK);
	CHECK(fake.commits == commits + 1);
	CHECK(fake.active.safety == 0);
	frame_start(true);
	CHECK(fake.active.safety == 1 && panel_caught_up());
	TRY(drain());
	CHECK(count_sub(CE_EVENT_PRESENTED, s2) == 1 && count_release(SAFETY_0) == 1);
	CHECK(find_release(SAFETY_0) > find_sub(CE_EVENT_PRESENTED, s2));
	CHECK(only_ordinary_events() && nlog == 2);
	CHECK(ce_buffer_state(engine, APP_B) == CE_BUFFER_WRITABLE);

	/* Restart: the write completes, the next present is one commit. */
	CHECK(ce_finish_write(engine, APP_B) == CE_OK);
	commits = fake.commits;
	CHECK(present2(APP_B, SAFETY_1, &s3) == CE_OK);
	CHECK(fake.commits == commits + 1); /* the application buffer only */
	frame_start(true);
	CHECK(panel_caught_up() && fake.active.app.pa == (uintptr_t)fake.app_mem[1]);
	TRY(drain());
	CHECK(count_sub(CE_EVENT_PRESENTED, s3) == 1 && count_release(APP_A) == 1);

	/* (b) finished, never presented: no commit, no event over many frames. */
	CHECK(ce_begin_write(engine, APP_A) == CE_OK);
	CHECK(ce_finish_write(engine, APP_A) == CE_OK);
	commits = fake.commits;
	nlog = 0;
	for (unsigned i = 0; i < 10; i++) {
		frame_start(true);
	}
	CHECK(fake.commits == commits && panel_caught_up());
	CHECK(ce_poll_event(engine, &e) == 0);
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_READY);

	/* (c) presented and latched, then no more frames for longer than the
	 * latch timeout: a latched frame is not pending, so no timeout event.
	 */
	CHECK(ce_set_latch_timeout(engine, 10) == CE_OK);
	CHECK(ce_advance_time(engine, 0) == CE_OK);
	CHECK(present2(APP_A, SAFETY_1, &s4) == CE_OK);
	frame_start(true);
	TRY(drain());
	CHECK(count_sub(CE_EVENT_PRESENTED, s4) == 1);
	nlog = 0;
	for (unsigned i = 0; i < 5; i++) {
		frame_start(true);
	}
	CHECK(ce_advance_time(engine, 100) == CE_OK);
	CHECK(ce_advance_time(engine, 1000) == CE_OK);
	CHECK(ce_poll_event(engine, &e) == 0);

	/* Restart after (c): both planes change, two commits, one latch. */
	commits = fake.commits;
	CHECK(present2(APP_B, SAFETY_0, &s5) == CE_OK);
	CHECK(fake.commits == commits + 2);
	frame_start(true);
	CHECK(panel_caught_up() && fake.active.safety == 0);
	TRY(drain());
	CHECK(count_sub(CE_EVENT_PRESENTED, s5) == 1 && count_sub(CE_EVENT_DROPPED, s5) == 0);
	CHECK(only_ordinary_events());

	/* The engine's advice for the frame the scenes present: both layers on
	 * planes, the safety layer on the safety plane.
	 */
	{
		static ce_capabilities caps;
		ce_buffer_entry entries[3] = {
			{APP_A, {CE_MODIFIER_LINEAR, WIDTH, HEIGHT, CE_FORMAT_RGB565}},
			{SAFETY_0, {CE_MODIFIER_LINEAR, WIDTH, HEIGHT, CE_FORMAT_ARGB8888}},
			{APP_XRGB, {CE_MODIFIER_LINEAR, WIDTH, HEIGHT, CE_FORMAT_XRGB8888}},
		};
		ce_layer layers[2] = {safety_layer(SAFETY_0), app_layer(APP_A)};
		ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};
		ce_advise_options options;
		ce_advice advice;
		ce_error_info info;

		memset(&caps, 0, sizeof(caps));
		memset(&options, 0, sizeof(options));
		CHECK(CE_ABI_CHECK_ADVISE());
		ce_twoplane_ops.capabilities(&backend, &caps);
		CHECK(ce_advise(&caps, entries, 3, &frame, &options, NULL, NULL, &advice, &info) ==
		      CE_OK);
		CHECK(!advice.has_compose);
		CHECK(advice.layers[0].placement == CE_PLACE_PLANE &&
		      advice.layers[0].assignment.plane == CE_TWOPLANE_PLANE_SAFETY);
		CHECK(advice.layers[1].placement == CE_PLACE_PLANE &&
		      advice.layers[1].assignment.plane == CE_TWOPLANE_PLANE_APP);
	}
	CHECK(!port_misused());
	return 0;
}

/* ---- Scene 10: a safety flip on top of an in-flight app commit ---- */

enum sweep {
	SWEEP_BEFORE,          /* latch between the two presents */
	SWEEP_AT_PLAN,         /* latch inside the planner's check of the second */
	SWEEP_AT_WRITE1_ISR,   /* latch after the second's first write */
	SWEEP_AT_WRITE1_QUIET, /* same, the interrupt withheld */
	SWEEP_AT_WRITE2_ISR,   /* latch after its last write */
	SWEEP_AT_WRITE2_QUIET,
	SWEEP_AT_COUNT_READ,   /* latch at the commit_count read, interrupt at unlock */
	SWEEP_AFTER,           /* latch after the second present returned */
	SWEEP_COUNT,
};

static const char *const sweep_names[SWEEP_COUNT] = {
	"before", "at-plan", "write1-isr", "write1-quiet", "write2-isr", "write2-quiet",
	"count-read", "after",
};

static int sweep_case(enum sweep at)
{
	uint64_t s0, s1, s2;
	bool mid = at == SWEEP_AT_WRITE1_ISR || at == SWEEP_AT_WRITE1_QUIET;
	bool behind;

	reset();
	TRY(start_engine());
	CHECK(present2(APP_A, SAFETY_0, &s0) == CE_OK);
	frame_start(true);
	TRY(drain());
	CHECK(count_sub(CE_EVENT_PRESENTED, s0) == 1);

	/* s1: the application commit in flight (one write). */
	CHECK(present2(APP_B, SAFETY_0, &s1) == CE_OK);
	CHECK(fake.latched + 1 == fake.commits);
	if (at == SWEEP_BEFORE) {
		frame_start(true);
	}
	switch (at) {
	case SWEEP_AT_PLAN:
		fake.start_at_format_ok = 1;
		fake.start_runs_isr = true;
		break;
	case SWEEP_AT_WRITE1_ISR:
	case SWEEP_AT_WRITE1_QUIET:
		fake.start_after_write = 1;
		fake.start_runs_isr = at == SWEEP_AT_WRITE1_ISR;
		break;
	case SWEEP_AT_WRITE2_ISR:
	case SWEEP_AT_WRITE2_QUIET:
		fake.start_after_write = 2;
		fake.start_runs_isr = at == SWEEP_AT_WRITE2_ISR;
		break;
	case SWEEP_AT_COUNT_READ:
		fake.start_at_commit_count = 1;
		fake.start_runs_isr = true;
		break;
	default:
		break;
	}
	/* s2: the safety flip, together with the application buffer back. */
	CHECK(present2(APP_A, SAFETY_1, &s2) == CE_OK);
	behind = fake.active.app.pa != fake.shadow.app.pa || fake.active.safety != fake.shadow.safety;
	CHECK(fake.latched <= fake.commits);
	if (at == SWEEP_AFTER) {
		frame_start(true);
	}
	/* The panel scans the full second commit at the first frame start
	 * after its writes, whatever happened before: the flip is not late.
	 */
	frame_start(true);
	CHECK(panel_caught_up());
	CHECK(fake.active.safety == 1 && fake.active.app_on);
	CHECK(fake.active.app.pa == (uintptr_t)fake.app_mem[0]);
	TRY(drain());
	CHECK(only_ordinary_events());

	/* One end per submission; the second is shown, never dropped. */
	CHECK(one_end(s1) && one_end(s2));
	CHECK(count_sub(CE_EVENT_PRESENTED, s2) == 1);
	CHECK(count_sub(CE_EVENT_PRESENTED, s0) == 1);
	if (at == SWEEP_BEFORE || at == SWEEP_AT_PLAN || mid || at == SWEEP_AT_WRITE2_ISR ||
	    at == SWEEP_AT_WRITE2_QUIET) {
		/* The latch reached s1 before s2 was taken over, or while it was
		 * written: reported presented, never dropped (design scene 10, and
		 * adv_replace_latch_during_writes). Where the interrupt was
		 * withheld the report may come with the later latch; still not a
		 * drop when the hardware did take it.
		 */
		if (at != SWEEP_AT_WRITE1_QUIET && at != SWEEP_AT_WRITE2_QUIET) {
			CHECK(count_sub(CE_EVENT_DROPPED, s1) == 0);
		}
	}
	if (at == SWEEP_AFTER) {
		CHECK(count_sub(CE_EVENT_DROPPED, s1) == 1);
	}
	/* The end state: the second frame's buffers on screen, no more events. */
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_SCANNING);
	CHECK(ce_buffer_state(engine, SAFETY_1) == CE_BUFFER_SCANNING);
	CHECK(ce_buffer_state(engine, APP_B) == CE_BUFFER_READY);
	CHECK(ce_buffer_state(engine, SAFETY_0) == CE_BUFFER_READY);
	CHECK(count_release(SAFETY_1) == 0 && count_release(APP_B) == 1 &&
	      count_release(SAFETY_0) == 1 && count_release(APP_A) <= 1);
	/* A buffer is released after the report that permits it. */
	CHECK(find_release(SAFETY_0) > find_sub(CE_EVENT_PRESENTED, s2));
	frame_start(true);
	TRY(drain());
	CHECK(only_ordinary_events());
	printf("  sweep %-12s s1=%-9s panel-behind-writes-after-present=%d\n", sweep_names[at],
	       count_sub(CE_EVENT_PRESENTED, s1) ? "presented" : "dropped", behind);
	CHECK(!port_misused());
	return 0;
}

static int scene10_flip_sweep(void)
{
	for (int at = 0; at < SWEEP_COUNT; at++) {
		TRY(sweep_case((enum sweep)at));
	}
	return 0;
}

/* ---- Scene 11: back-to-back commits within a frame ---- */

static int scene11_bursts(void)
{
	uint64_t s0, s1, s2, s3;
	uint32_t before, vsyncs;

	/* Three presents between two frame starts. */
	TRY(start_engine());
	CHECK(present2(APP_A, SAFETY_0, &s0) == CE_OK);
	frame_start(true);
	TRY(drain());
	before = fake.commits;
	vsyncs = backend.vsyncs;
	CHECK(present2(APP_B, SAFETY_0, &s1) == CE_OK);
	/* The producer rule's negative control: a submitted buffer is not drawn. */
	CHECK(ce_begin_write(engine, APP_B) == CE_ERR_BUFFER_IN_USE);
	CHECK(present2(APP_A, SAFETY_1, &s2) == CE_OK);
	CHECK(present2(APP_B, SAFETY_1, &s3) == CE_OK);
	CHECK(backend.vsyncs == vsyncs); /* the burst did not cross a frame start */
	CHECK(fake.commits == before + 1 + 2 + 1);
	CHECK(fake.latched + 4 == fake.commits); /* four commits wait for one GO */
	frame_start(true);
	CHECK(panel_caught_up()); /* the latched count jumped by all four */
	CHECK(fake.active.safety == 1 && fake.active.app.pa == (uintptr_t)fake.app_mem[1]);
	TRY(drain());
	CHECK(only_ordinary_events());
	CHECK(count_sub(CE_EVENT_DROPPED, s1) == 1 && count_sub(CE_EVENT_DROPPED, s2) == 1);
	CHECK(count_sub(CE_EVENT_PRESENTED, s1) == 0 && count_sub(CE_EVENT_PRESENTED, s2) == 0);
	CHECK(count_sub(CE_EVENT_PRESENTED, s3) == 1 && count_sub(CE_EVENT_DROPPED, s3) == 0);
	/* The buffers of the dropped frames not carried into the last one, each
	 * once; the last frame's buffers stay on screen.
	 */
	CHECK(count_release(APP_B) == 1 && count_release(APP_A) == 1);
	CHECK(count_release(SAFETY_0) == 1 && count_release(SAFETY_1) == 0);
	CHECK(find_release(APP_A) > find_sub(CE_EVENT_PRESENTED, s3));
	CHECK(ce_buffer_state(engine, APP_B) == CE_BUFFER_SCANNING);
	CHECK(ce_buffer_state(engine, SAFETY_1) == CE_BUFFER_SCANNING);
	frame_start(true);
	TRY(drain());
	CHECK(only_ordinary_events());

	/* The same burst split by a frame start: the first is shown. */
	reset();
	TRY(start_engine());
	CHECK(present2(APP_A, SAFETY_0, &s0) == CE_OK);
	frame_start(true);
	TRY(drain());
	CHECK(present2(APP_B, SAFETY_0, &s1) == CE_OK);
	frame_start(true);
	CHECK(present2(APP_A, SAFETY_1, &s2) == CE_OK);
	CHECK(present2(APP_B, SAFETY_1, &s3) == CE_OK);
	frame_start(true);
	TRY(drain());
	CHECK(count_sub(CE_EVENT_PRESENTED, s1) == 1 && count_sub(CE_EVENT_DROPPED, s1) == 0);
	CHECK(count_sub(CE_EVENT_DROPPED, s2) == 1 && count_sub(CE_EVENT_PRESENTED, s2) == 0);
	CHECK(count_sub(CE_EVENT_PRESENTED, s3) == 1);
	CHECK(panel_caught_up() && fake.active.safety == 1);
	CHECK(count_release(SAFETY_1) == 0 && count_release(SAFETY_0) == 1);

	/* A long burst against the event queue and the busy refusal: every
	 * status is one the caller can act on, every accepted submission ends
	 * once, the last one is shown, and the safety plane shows the last
	 * accepted flip.
	 */
	reset();
	TRY(start_engine());
	CHECK(present2(APP_A, SAFETY_0, &s0) == CE_OK);
	frame_start(true);
	TRY(drain());
	{
		uint64_t subs[64];
		unsigned accepted = 0, refused = 0, want = 0;
		uint32_t app = APP_B;

		for (unsigned i = 0; i < 64; i++) {
			uint32_t safety = (i / 3) % 2 ? SAFETY_1 : SAFETY_0;
			ce_status rc = present2(app, safety, &subs[accepted]);

			CHECK(rc == CE_OK || rc == CE_ERR_BACKEND_BUSY || rc == CE_ERR_EVENT_QUEUE_FULL);
			if (rc == CE_OK) {
				want = safety == SAFETY_1;
				accepted++;
				app = app == APP_A ? APP_B : APP_A;
			} else {
				refused++;
				TRY(drain());
			}
			if (i % 16 == 15) {
				TRY(drain());
			}
		}
		frame_start(true);
		TRY(drain());
		CHECK(accepted > 0 && panel_caught_up() && fake.active.safety == want);
		for (unsigned i = 0; i < accepted; i++) {
			CHECK(one_end(subs[i]));
		}
		CHECK(count_sub(CE_EVENT_PRESENTED, subs[accepted - 1]) == 1);
		printf("  burst of 64: accepted %u refused %u\n", accepted, refused);
	}
	CHECK(!port_misused());
	return 0;
}

/* ---- The producer's bad hand-overs ---- */

static int refused_untouched(ce_status rc, ce_status want, uint32_t commits, unsigned writes)
{
	ce_event e;

	CHECK(rc == want);
	CHECK(fake.commits == commits && fake.writes == writes);
	CHECK(ce_poll_event(engine, &e) == 0);
	return 0;
}

static int producer_bad_handover(void)
{
	uint64_t s0, s1, sub = 0;
	uint32_t commits;
	unsigned writes;
	ce_error_info info;
	ce_layer layers[2];
	ce_frame frame = {.layers = layers, .layer_count = 2, .output = 0};

	fake.formats_ok[1] = false; /* the driver refuses XRGB8888 */
	TRY(start_engine());
	CHECK(present2(APP_A, SAFETY_0, &s0) == CE_OK);
	frame_start(true);
	TRY(drain());
	commits = fake.commits;
	writes = fake.writes;

	/* The application layer's buffer: still being drawn, unknown to the
	 * engine, not in the integrator's table, without an address, in a
	 * format the driver refuses.
	 */
	CHECK(ce_begin_write(engine, APP_B) == CE_OK);
	TRY(refused_untouched(present2(APP_B, SAFETY_1, &sub), CE_ERR_BUFFER_NOT_READY, commits,
			      writes));
	TRY(refused_untouched(present2(NOT_REGISTERED, SAFETY_1, &sub), CE_ERR_UNKNOWN_BUFFER,
			      commits, writes));
	CHECK(present2(APP_UNLISTED, SAFETY_1, &sub) != CE_OK);
	CHECK(fake.commits == commits && fake.writes == writes);
	CHECK(present2(APP_NO_PA, SAFETY_1, &sub) != CE_OK);
	CHECK(fake.commits == commits && fake.writes == writes);
	layers[0] = safety_layer(SAFETY_1);
	layers[1] = app_layer(APP_XRGB);
	TRY(refused_untouched(ce_present(engine, &frame, &sub, &info), CE_ERR_NO_PLANE, commits,
			      writes));
	CHECK((info.reasons & CE_REJECT_FORMAT) != 0);
	/* The same frame in the engine's advice: the layer has no plane. */
	{
		static ce_capabilities caps;
		ce_buffer_entry entries[2] = {
			{APP_XRGB, {CE_MODIFIER_LINEAR, WIDTH, HEIGHT, CE_FORMAT_XRGB8888}},
			{SAFETY_1, {CE_MODIFIER_LINEAR, WIDTH, HEIGHT, CE_FORMAT_ARGB8888}},
		};
		ce_advise_options options;
		ce_advice advice;

		memset(&caps, 0, sizeof(caps));
		memset(&options, 0, sizeof(options));
		ce_twoplane_ops.capabilities(&backend, &caps);
		CHECK(ce_advise(&caps, entries, 2, &frame, &options, NULL, NULL, &advice, &info) ==
		      CE_OK);
		CHECK(advice.layers[1].placement == CE_PLACE_COMPOSE &&
		      advice.layers[1].blocker == CE_BLOCKER_STATIC &&
		      (advice.layers[1].reasons & CE_REJECT_FORMAT) != 0);
		/* With composition off the frame does not fit the planes as a
		 * whole: every layer is composed, with its blocker. */
		CHECK(!advice.has_compose && advice.layers[0].placement == CE_PLACE_COMPOSE);
	}
	/* The frame lacks a safety layer, or names a safety buffer that is not
	 * the driver's: the backend refuses; the safety plane is never off.
	 */
	TRY(refused_untouched(present1(app_layer(APP_A), &sub), CE_ERR_BACKEND_REJECTED, commits,
			      writes));
	TRY(refused_untouched(present2(APP_A, SAFETY_OTHER, &sub), CE_ERR_BACKEND_REJECTED,
			      commits, writes));
	/* The safety buffer itself is still being drawn. */
	CHECK(ce_begin_write(engine, SAFETY_1) == CE_OK);
	TRY(refused_untouched(present2(APP_A, SAFETY_1, &sub), CE_ERR_BUFFER_NOT_READY, commits,
			      writes));

	/* After every refusal: the panel and the engine are as before. */
	CHECK(panel_caught_up() && fake.active.safety == 0);
	CHECK(ce_buffer_state(engine, APP_A) == CE_BUFFER_SCANNING);
	CHECK(ce_buffer_state(engine, SAFETY_0) == CE_BUFFER_SCANNING);
	CHECK(backend.pending == false);

	/* The producer never finishes APP_B. A good safety flip still commits
	 * once and latches at the first frame start.
	 */
	CHECK(ce_finish_write(engine, SAFETY_1) == CE_OK);
	CHECK(present2(APP_A, SAFETY_1, &s1) == CE_OK);
	CHECK(fake.commits == commits + 1);
	frame_start(true);
	CHECK(panel_caught_up() && fake.active.safety == 1);
	TRY(drain());
	CHECK(count_sub(CE_EVENT_PRESENTED, s1) == 1 && count_release(SAFETY_0) == 1);
	CHECK(count_sub(CE_EVENT_DROPPED, s1) == 0);
	CHECK(ce_buffer_state(engine, APP_B) == CE_BUFFER_WRITABLE);
	CHECK(!port_misused());
	return 0;
}

/* A latch that does not come, then a replacement: host only, as the design
 * says (the board cannot withhold the GO bit).
 */
static int producer_latch_timeout(void)
{
	uint64_t s0, s1, s2;
	ce_event e;

	TRY(start_engine());
	CHECK(ce_set_latch_timeout(engine, 10) == CE_OK);
	CHECK(ce_advance_time(engine, 0) == CE_OK);
	CHECK(present2(APP_A, SAFETY_0, &s0) == CE_OK);
	frame_start(true);
	TRY(drain());

	fake.hold = true;
	CHECK(present2(APP_A, SAFETY_1, &s1) == CE_OK);
	for (unsigned i = 0; i < 20; i++) {
		frame_start(true);
	}
	CHECK(ce_poll_event(engine, &e) == 0);
	CHECK(fake.active.safety == 0); /* held: the hardware has not taken it */
	CHECK(ce_advance_time(engine, 10) == CE_OK);
	CHECK(ce_poll_event(engine, &e) == 1 && e.kind == CE_EVENT_LATCH_TIMEOUT &&
	      e.submission == s1);
	CHECK(ce_poll_event(engine, &e) == 0); /* once */
	CHECK(ce_begin_write(engine, SAFETY_1) == CE_ERR_BUFFER_IN_USE);

	/* The caller's recovery: a replacement. The timed-out frame is dropped
	 * (never shown), and the replacement latches at the first frame start
	 * the hardware takes.
	 */
	CHECK(present2(APP_A, SAFETY_0, &s2) == CE_OK);
	TRY(drain());
	CHECK(count_sub(CE_EVENT_DROPPED, s1) == 1 && count_sub(CE_EVENT_PRESENTED, s1) == 0);
	fake.hold = false;
	frame_start(true);
	TRY(drain());
	CHECK(panel_caught_up() && fake.active.safety == 0);
	CHECK(count_sub(CE_EVENT_PRESENTED, s2) == 1);
	CHECK(ce_buffer_state(engine, SAFETY_0) == CE_BUFFER_SCANNING);

	/* A timed-out frame that latches late is still shown. */
	fake.hold = true;
	CHECK(present2(APP_A, SAFETY_1, &s1) == CE_OK);
	CHECK(ce_advance_time(engine, 21) == CE_OK);
	CHECK(ce_poll_event(engine, &e) == 1 && e.kind == CE_EVENT_LATCH_TIMEOUT);
	fake.hold = false;
	frame_start(true);
	TRY(drain());
	CHECK(count_sub(CE_EVENT_PRESENTED, s1) == 1 && fake.active.safety == 1);
	CHECK(!port_misused());
	return 0;
}

/* ---- Producer calls and latch events interleaved, from a seed ---- */

static uint64_t rng_state;

static unsigned rnd(unsigned n)
{
	rng_state ^= rng_state << 13;
	rng_state ^= rng_state >> 7;
	rng_state ^= rng_state << 17;
	return (unsigned)(rng_state % n);
}

static int interleaved(uint64_t seed)
{
	static uint64_t subs[4096];
	unsigned nsubs = 0, want = 0, undrained = 0;
	bool any = false;
	uint32_t last_commits;
	const uint32_t apps[2] = {APP_A, APP_B};

	reset();
	rng_state = seed * 0x9E3779B97F4A7C15ull + 1;
	TRY(start_engine());
	last_commits = fake.commits;

	for (unsigned step = 0; step < 3000; step++) {
		unsigned op = rnd(12);
		uint32_t app = apps[rnd(2)];
		uint32_t safety = rnd(2) ? SAFETY_1 : SAFETY_0;

		switch (op) {
		case 0: /* the producer starts drawing */
			if (ce_buffer_state(engine, app) == CE_BUFFER_READY) {
				CHECK(ce_begin_write(engine, app) == CE_OK);
			}
			break;
		case 1: /* ... and finishes, or never does */
			if (ce_buffer_state(engine, app) == CE_BUFFER_WRITABLE && rnd(3) != 0) {
				CHECK(ce_finish_write(engine, app) == CE_OK);
			}
			break;
		case 2:
		case 3:
		case 4:
		case 5: /* a present, with a frame start scripted into it sometimes */
		{
			int32_t state = ce_buffer_state(engine, app);
			uint32_t commits = fake.commits;
			ce_status rc;

			if (op == 5) {
				fake.start_after_write = 1 + rnd(2);
				fake.start_runs_isr = rnd(2);
			}
			rc = present2(app, safety, &subs[nsubs]);
			fake.start_after_write = 0;
			CHECK(rc == CE_OK || rc == CE_ERR_BUFFER_NOT_READY ||
			      rc == CE_ERR_BACKEND_BUSY || rc == CE_ERR_EVENT_QUEUE_FULL);
			if (rc == CE_OK) {
				CHECK(state != CE_BUFFER_WRITABLE && nsubs < 4096);
				nsubs++;
				want = safety == SAFETY_1;
				any = true;
			} else {
				CHECK(fake.commits == commits);
				if (rc == CE_ERR_BUFFER_NOT_READY) {
					CHECK(state == CE_BUFFER_WRITABLE);
				}
			}
			undrained++;
			break;
		}
		case 6: /* a bad hand-over: nothing written, nothing submitted */
		{
			uint32_t commits = fake.commits;
			uint64_t sub = 0;
			ce_status rc;

			switch (rnd(4)) {
			case 0:
				rc = present2(NOT_REGISTERED, safety, &sub);
				break;
			case 1:
				rc = present1(app_layer(app), &sub);
				break;
			case 2:
				rc = present2(app, SAFETY_OTHER, &sub);
				break;
			default:
				rc = present2(APP_NO_PA, safety, &sub);
				break;
			}
			CHECK(rc != CE_OK && fake.commits == commits);
			break;
		}
		case 7:
		case 8: /* a frame start with its interrupt */
		case 9: /* a frame start whose interrupt is late */
			frame_start(op != 9);
			/* Nothing the producer did keeps the hardware from taking
			 * the writes at a frame start, and the safety plane shows
			 * the last flip accepted.
			 */
			CHECK(panel_caught_up());
			CHECK(!any || fake.active.safety == want);
			break;
		default:
			TRY(drain());
			undrained = 0;
			break;
		}
		CHECK((int32_t)(fake.commits - fake.latched) >= 0);
		CHECK((int32_t)(fake.commits - last_commits) >= 0);
		last_commits = fake.commits;
		if (undrained >= 3) {
			TRY(drain());
			undrained = 0;
		}
	}
	/* Quiesce: a frame start, with and without the interrupt, then drain. */
	frame_start(false);
	frame_start(true);
	TRY(drain());
	CHECK(panel_caught_up() && (!any || fake.active.safety == want));
	CHECK(only_ordinary_events());
	for (unsigned i = 0; i < nsubs; i++) {
		CHECK(one_end(subs[i]));
	}
	if (nsubs > 0) {
		CHECK(count_sub(CE_EVENT_PRESENTED, subs[nsubs - 1]) == 1);
	}
	CHECK(!port_misused());
	return 0;
}

static int interleaved_seeded(void)
{
	for (uint64_t seed = 1; seed <= 25; seed++) {
		if (interleaved(seed) != 0) {
			fprintf(stderr, "seed %llu\n", (unsigned long long)seed);
			return 1;
		}
	}
	return 0;
}

static const struct {
	const char *name;
	int (*fn)(void);
} scenarios[] = {
	{"scene9_producer_stops", scene9_producer_stops},
	{"scene10_flip_sweep", scene10_flip_sweep},
	{"scene11_bursts", scene11_bursts},
	{"producer_bad_handover", producer_bad_handover},
	{"producer_latch_timeout", producer_latch_timeout},
	{"interleaved_seeded", interleaved_seeded},
};

int main(int argc, char **argv)
{
	if (argc != 2) {
		fprintf(stderr, "usage: m3_03_host <scenario>\n");
		return 2;
	}
	for (size_t i = 0; i < sizeof(scenarios) / sizeof(scenarios[0]); i++) {
		if (strcmp(scenarios[i].name, argv[1]) == 0) {
			reset();
			if (scenarios[i].fn() != 0 || port_misused()) {
				fprintf(stderr, "FAILED %s\n", argv[1]);
				return 1;
			}
			printf("ok %s\n", argv[1]);
			return 0;
		}
	}
	fprintf(stderr, "no such scenario: %s\n", argv[1]);
	return 2;
}
"##;
