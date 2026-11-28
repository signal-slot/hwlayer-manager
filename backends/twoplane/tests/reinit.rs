//! Teardown and re-initialisation of the two-plane backend.
//!
//! A second board image needs a reboot, but a producer that restarts inside
//! one image creates the backend and the engine again: it must not see state
//! of the first run. The backend is C (`ce_twoplane.c`) and this crate
//! forbids unsafe code, so each test builds a small C program (a fake panel,
//! the backend and the engine's C API, built with the system C compiler like
//! `tests/twoplane.rs`), runs one scenario of it and checks the `key=value`
//! lines it prints.
//!
//! "Teardown" is what the C code offers: there is no destroy call. The
//! backend struct is overwritten (with a pattern, or left to
//! `ce_twoplane_init`) and the engine's storage is handed to
//! `ce_engine_init` again. Where a value below is only what the code does
//! today, the test says so.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

const RUNNER: &str = r#"
#include "ce_twoplane.h"
#include "composition_engine.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { W = 64, H = 32 };
enum { APP1 = 1, APP2 = 2, SAFE0 = 10, SAFE1 = 11 };

struct regs { bool app_on; unsigned safety; };
static struct {
	struct regs shadow, active;
	uint32_t commits, latched;
	unsigned writes, enable_on, set_app, flips, wakes;
	ce_twoplane_vsync_cb_t cb;
	void *cb_user;
	uint32_t safety_mem[2][4];
	uint32_t app_mem[2][4];
} fake;

static void frame_start(void)
{
	if (fake.latched != fake.commits) {
		fake.active = fake.shadow;
		fake.latched = fake.commits;
	}
	if (fake.cb) fake.cb(&fake, fake.cb_user);
}
static bool hw_app_format_ok(uint32_t f) { return f == CE_FORMAT_RGB565; }
static int hw_set_app_buffer(const void *d, const struct ce_twoplane_buf *b)
{ (void)d; (void)b; fake.set_app++; fake.commits++; fake.writes++; return 0; }
static int hw_enable_app_plane(const void *d, bool e)
{
	(void)d; fake.shadow.app_on = e; if (e) fake.enable_on++;
	fake.commits++; fake.writes++; return 0;
}
static int hw_safety_flip(const void *d)
{ (void)d; fake.shadow.safety ^= 1u; fake.flips++; fake.commits++; fake.writes++; return 0; }
static void *hw_safety_buffer(const void *d, uint32_t *w, uint32_t *h, uint32_t *s)
{ (void)d; *w = W; *h = H; *s = W * 4; return fake.safety_mem[fake.shadow.safety ^ 1u]; }
static void *hw_safety_front(const void *d) { (void)d; return fake.safety_mem[fake.active.safety]; }
static int hw_safety_premultiplied(const void *d, bool on) { (void)d; (void)on; return 0; }
static uint32_t hw_commit_count(const void *d) { (void)d; return fake.commits; }
static uint32_t hw_latched_count(const void *d) { (void)d; return fake.latched; }
static void hw_set_vsync_callback(const void *d, ce_twoplane_vsync_cb_t cb, void *u)
{ (void)d; fake.cb = cb; fake.cb_user = u; }
static const struct ce_twoplane_hw hw = {
	hw_app_format_ok, hw_set_app_buffer, hw_enable_app_plane, hw_safety_flip,
	hw_safety_buffer, hw_safety_front, hw_safety_premultiplied, hw_commit_count,
	hw_latched_count, hw_set_vsync_callback,
};

static struct ce_twoplane_record table[4];
static unsigned int port_irq_lock(void) { return 0; }
static void port_irq_unlock(unsigned key) { (void)key; }
static void port_wake(void *a) { (void)a; fake.wakes++; }
static const struct ce_twoplane_record *port_lookup(void *a, uint32_t id)
{
	(void)a;
	for (size_t i = 0; i < 4; i++) if (table[i].id == id) return &table[i];
	return NULL;
}
static bool port_ready(const void *d) { (void)d; return true; }
static const struct ce_twoplane_port port = {
	port_irq_lock, port_irq_unlock, port_wake, NULL, port_lookup, NULL, port_ready,
};

static struct ce_twoplane backend_a, backend_b;
static _Alignas(CE_ENGINE_ALIGN) unsigned char storage[CE_ENGINE_SIZE];
static ce_engine *engine;
static const uint32_t ids[4] = {APP1, APP2, SAFE0, SAFE1};

static void fresh_panel(void)
{
	memset(&fake, 0, sizeof(fake));
	table[0] = (struct ce_twoplane_record){APP1, 1, (uintptr_t)fake.app_mem[0], W, H, W * 2,
					       CE_FORMAT_RGB565};
	table[1] = (struct ce_twoplane_record){APP2, 1, (uintptr_t)fake.app_mem[1], W, H, W * 2,
					       CE_FORMAT_RGB565};
	table[2] = (struct ce_twoplane_record){SAFE0, 0, (uintptr_t)fake.safety_mem[0], W, H, W * 4,
					       CE_FORMAT_ARGB8888};
	table[3] = (struct ce_twoplane_record){SAFE1, 0, (uintptr_t)fake.safety_mem[1], W, H, W * 4,
					       CE_FORMAT_ARGB8888};
}

static void register_buffers(void)
{
	for (int i = 0; i < 4; i++) {
		const ce_buffer_info info = {CE_MODIFIER_LINEAR, W, H,
					     ids[i] >= 10 ? CE_FORMAT_ARGB8888 : CE_FORMAT_RGB565};
		if (ce_register_buffer(engine, ids[i], &info) != CE_OK ||
		    ce_finish_write(engine, ids[i]) != CE_OK) exit(5);
	}
}

/* Backend b on the panel, and a new engine in the same storage; the
 * buffers are registered and ready when `buffers` is set. */
static void start(struct ce_twoplane *b, bool buffers)
{
	if (ce_twoplane_init(b, &hw, &port, &fake) != 0) exit(3);
	engine = ce_engine_init(storage, sizeof(storage), &ce_twoplane_ops, b);
	if (engine == NULL) exit(4);
	if (buffers) register_buffers();
}

static int present(uint32_t app, uint32_t safety, uint64_t *sub)
{
	ce_layer l[2];
	for (int i = 0; i < 2; i++) {
		uint32_t id = i ? app : safety;
		l[i] = (ce_layer){.buffer = id, .source = {0, 0, W, H}, .destination = {0, 0, W, H},
				  .z_order = i ? 0 : 10, .alpha = CE_ALPHA_OPAQUE,
				  .blend = i ? CE_BLEND_OPAQUE : CE_BLEND_PREMULTIPLIED,
				  .transform = CE_TRANSFORM_NORMAL};
	}
	ce_frame fr = {.layers = l, .layer_count = 2, .output = 0};
	return ce_present(engine, &fr, sub, NULL);
}

/* Takes every event: counts of presented, dropped, released, others. */
static void drain(const char *tag)
{
	ce_event e;
	unsigned p = 0, d = 0, r = 0, o = 0;
	while (ce_poll_event(engine, &e) == 1) {
		if (e.kind == CE_EVENT_PRESENTED) p++;
		else if (e.kind == CE_EVENT_DROPPED) d++;
		else if (e.kind == CE_EVENT_RELEASED) r++;
		else o++;
	}
	printf("%s.presented=%u\n%s.dropped=%u\n%s.released=%u\n%s.other=%u\n", tag, p, tag, d, tag, r,
	       tag, o);
}

static void backend_state(const char *tag, const struct ce_twoplane *b)
{
	printf("%s.pending=%d\n%s.latched=%d\n%s.vsyncs=%u\n%s.app_on=%d\n%s.app_id=%u\n"
	       "%s.app_seq=%u\n%s.pending_sub=%llu\n%s.pending_count=%u\n%s.safety_is_front=%d\n"
	       "%s.width=%u\n",
	       tag, b->pending, tag, b->latched, tag, b->vsyncs, tag, b->app_on, tag, b->app_id,
	       tag, b->app_seq, tag, (unsigned long long)b->pending_sub, tag, b->pending_count, tag,
	       b->safety_pa == (uintptr_t)fake.safety_mem[fake.active.safety], tag, b->width);
}

static void panel_state(const char *tag)
{
	printf("%s.commits=%u\n%s.latched=%u\n%s.cb_user_is_b=%d\n%s.cb_user_is_a=%d\n", tag,
	       fake.commits, tag, fake.latched, tag, fake.cb_user == &backend_b, tag,
	       fake.cb_user == &backend_a);
}

static void states(const char *tag)
{
	printf("%s.buf1=%d\n%s.buf2=%d\n%s.safe0=%d\n%s.safe1=%d\n", tag, ce_buffer_state(engine, APP1),
	       tag, ce_buffer_state(engine, APP2), tag, ce_buffer_state(engine, SAFE0), tag,
	       ce_buffer_state(engine, SAFE1));
}

/* A first run: three presents, a frame start after each of the first two. */
static void first_run(void)
{
	uint64_t s;
	start(&backend_a, true);
	if (present(APP1, SAFE0, &s) != 0) exit(6);
	printf("run1.sub1=%llu\n", (unsigned long long)s);
	frame_start();
	if (present(APP2, SAFE1, &s) != 0) exit(6);
	frame_start();
	if (present(APP1, SAFE1, &s) != 0) exit(6);
	printf("run1.sub3=%llu\n", (unsigned long long)s);
}

/* A present and a frame start on whatever panel there is. */
static void second_run(void)
{
	uint64_t s = 0;
	int rc = present(APP1, SAFE0, &s);
	printf("run2.rc=%d\nrun2.sub1=%llu\n", rc, (unsigned long long)s);
	panel_state("run2.before_vsync");
	frame_start();
	panel_state("run2.after_vsync");
	backend_state("run2.backend", &backend_b);
	drain("run2.events");
	states("run2.state");
	printf("run2.set_app=%u\nrun2.enable_on=%u\nrun2.flips=%u\n", fake.set_app, fake.enable_on,
	       fake.flips);
}

int main(int argc, char **argv)
{
	uint64_t s;

	if (argc != 2) return 2;
	fresh_panel();
	if (strcmp(argv[1], "fresh_panel") == 0) {
		first_run();
		backend_state("run1.backend", &backend_a);
		panel_state("run1.panel");
		drain("run1.events");
		/* torn down: a new panel, the old backend struct is left behind */
		fresh_panel();
		memset(&backend_b, 0xA5, sizeof(backend_b));
		start(&backend_b, false);
		backend_state("init.backend", &backend_b);
		panel_state("init.panel");
		states("init.state");
		drain("init.events");
		register_buffers();
		second_run();
	} else if (strcmp(argv[1], "same_panel") == 0) {
		first_run();
		drain("run1.events");
		panel_state("run1.panel");
		/* the panel keeps its counters, plane registers and callback */
		start(&backend_b, true);
		backend_state("init.backend", &backend_b);
		panel_state("init.panel");
		second_run();
	} else if (strcmp(argv[1], "pending_dropped") == 0) {
		first_run(); /* the third commit is still waiting for its frame start */
		panel_state("run1.panel");
		printf("run1.pending=%d\nrun1.pending_sub=%llu\n", backend_a.pending,
		       (unsigned long long)backend_a.pending_sub);
		/* the second engine starts before the panel latches it */
		start(&backend_b, true);
		backend_state("init.backend", &backend_b);
		panel_state("init.panel");
		frame_start(); /* the panel latches the old commit */
		backend_state("late.backend", &backend_b);
		printf("late.old_vsyncs=%u\nlate.old_pending=%d\nlate.old_latched=%d\n",
		       backend_a.vsyncs, backend_a.pending, backend_a.latched);
		drain("late.events");
		states("late.state");
	} else if (strcmp(argv[1], "releases_owing") == 0) {
		start(&backend_a, true);
		present(APP1, SAFE0, &s);
		frame_start();
		present(APP2, SAFE0, &s);
		frame_start();
		states("run1.state");
		/* no event was taken: a release is owing for buffer 1 and the
		 * presented events of two submissions */
		start(&backend_b, false);
		states("init.state");
		drain("init.events");
		register_buffers();
		states("reg.state");
		second_run();
	} else {
		return 2;
	}
	return 0;
}
"#;

fn backend_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn engine_include() -> PathBuf {
    backend_dir().join("../../crates/composition-ffi/include")
}

fn run(command: &mut Command, what: &str) -> std::process::Output {
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

/// The engine's static library, built by a nested cargo (see
/// `tests/twoplane.rs`); its path is the one Cargo reports.
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
    let library = messages
        .lines()
        .filter(|line| line.contains("\"reason\":\"compiler-artifact\""))
        .find_map(|line| {
            let end = line.find("libcomposition_engine.a")? + "libcomposition_engine.a".len();
            let start = line[..end].rfind('"')? + 1;
            Some(PathBuf::from(&line[start..end]))
        });
    let library =
        library.unwrap_or_else(|| panic!("cargo reported no libcomposition_engine.a:\n{messages}"));
    assert!(
        library.exists(),
        "no static library at {}",
        library.display()
    );
    library
}

fn runner() -> &'static PathBuf {
    static RUNNER_PATH: OnceLock<PathBuf> = OnceLock::new();
    RUNNER_PATH.get_or_init(|| {
        let library = build_static_library();
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        let source = dir.join("reinit_runner.c");
        std::fs::write(&source, RUNNER).unwrap();
        let program = dir.join("reinit_runner");
        run(
            Command::new(std::env::var("CC").unwrap_or_else(|_| "cc".into()))
                .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
                .arg("-I")
                .arg(engine_include())
                .arg("-I")
                .arg(backend_dir())
                .arg(&source)
                .arg(backend_dir().join("ce_twoplane.c"))
                .arg(&library)
                .args(["-lpthread", "-ldl", "-lm"])
                .arg("-o")
                .arg(&program),
            "the C compiler",
        );
        program
    })
}

/// What a scenario printed.
struct Out(BTreeMap<String, String>);

impl Out {
    fn of(scenario: &str) -> Out {
        let output = run(Command::new(runner()).arg(scenario), scenario);
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        print!("{text}");
        Out(text
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect())
    }

    /// Asserts that every `key` has the given value.
    fn is(&self, expected: &[(&str, &str)]) {
        for (key, want) in expected {
            let got = self.0.get(*key).unwrap_or_else(|| panic!("no {key}"));
            assert_eq!(got, want, "{key}");
        }
    }
}

/// A backend struct and an engine over a fresh panel are clean, whatever the
/// old struct held (it is overwritten with a pattern first).
#[test]
fn reinit_on_a_fresh_panel_starts_clean() {
    Out::of("fresh_panel").is(&[
        // the first run: two latched commits and a pending third
        ("run1.sub1", "1"),
        ("run1.sub3", "3"),
        ("run1.backend.pending", "1"),
        ("run1.events.presented", "2"),
        ("run1.events.released", "2"),
        ("init.backend.pending", "0"),
        ("init.backend.latched", "0"),
        ("init.backend.vsyncs", "0"),
        ("init.backend.app_on", "0"),
        ("init.backend.app_id", "0"),
        ("init.backend.app_seq", "0"),
        ("init.backend.pending_sub", "0"),
        ("init.backend.pending_count", "0"),
        ("init.backend.safety_is_front", "1"),
        ("init.panel.cb_user_is_b", "1"),
        ("init.panel.cb_user_is_a", "0"),
        // a fresh panel counts from zero, no buffer is known, no event owed
        ("init.panel.commits", "0"),
        ("init.panel.latched", "0"),
        ("init.state.buf1", "-1"),
        ("init.state.safe0", "-1"),
        ("init.events.presented", "0"),
        ("init.events.released", "0"),
        // submission numbers restart with the engine
        ("run2.sub1", "1"),
        ("run2.before_vsync.commits", "2"),
        ("run2.before_vsync.latched", "0"),
        ("run2.after_vsync.latched", "2"),
        ("run2.events.presented", "1"),
        ("run2.events.dropped", "0"),
        ("run2.state.buf1", "3"),
        ("run2.state.safe0", "3"),
    ]);
}

/// The panel is kept: its counters and plane registers carry over, the
/// backend's state does not.
#[test]
fn reinit_on_the_same_panel_keeps_only_the_panels_state() {
    Out::of("same_panel").is(&[
        ("run1.panel.commits", "5"),
        ("run1.panel.latched", "4"),
        ("init.backend.pending", "0"),
        ("init.backend.latched", "0"),
        ("init.backend.vsyncs", "0"),
        ("init.backend.app_on", "0"),
        ("init.backend.app_id", "0"),
        ("init.backend.pending_sub", "0"),
        ("init.backend.pending_count", "0"),
        ("init.backend.safety_is_front", "1"),
        ("init.panel.cb_user_is_b", "1"),
        ("init.panel.cb_user_is_a", "0"),
        // the panel's counters carry over, the first commit adds to them
        ("init.panel.commits", "5"),
        ("init.panel.latched", "4"),
        ("run2.sub1", "1"),
        // Current behaviour: the new backend believes the application plane
        // is off although the panel's is on, so its first commit writes the
        // buffer, enables the plane again and flips the safety buffer (it
        // is not the one on the plane): 3 writes, not a no-op.
        ("run2.before_vsync.commits", "8"),
        ("run2.after_vsync.latched", "8"),
        ("run2.set_app", "4"),
        ("run2.enable_on", "2"),
        ("run2.flips", "2"),
        // the old pending commit (count 5) is not reported: one presented
        ("run2.events.presented", "1"),
        ("run2.events.dropped", "0"),
        ("run2.backend.pending_count", "8"),
    ]);
}

/// A commit still waiting for its latch when the backend is torn down is
/// never reported by the new backend.
#[test]
fn a_pending_commit_of_the_old_backend_is_not_reported_again() {
    Out::of("pending_dropped").is(&[
        ("run1.pending", "1"),
        ("run1.pending_sub", "3"),
        ("init.backend.pending", "0"),
        ("init.backend.latched", "0"),
        ("init.backend.vsyncs", "0"),
        ("init.backend.pending_sub", "0"),
        ("init.panel.cb_user_is_b", "1"),
        ("init.panel.cb_user_is_a", "0"),
        // the panel latches the old commit after the new backend took over:
        // its vsync callback runs on the new backend only
        ("late.backend.vsyncs", "1"),
        ("late.backend.pending", "0"),
        ("late.backend.latched", "0"),
        ("late.old_vsyncs", "2"),
        ("late.old_pending", "1"),
        ("late.old_latched", "0"),
        // No event for the old submission 3, neither presented nor dropped.
        // Current behaviour: the new engine starts with an empty queue, so
        // the old submission is simply forgotten.
        ("late.events.presented", "0"),
        ("late.events.dropped", "0"),
        ("late.events.released", "0"),
        ("late.events.other", "0"),
    ]);
}

/// Buffers in use and events owing at teardown do not reach the new engine.
#[test]
fn releases_owing_at_teardown_do_not_reach_the_new_engine() {
    Out::of("releases_owing").is(&[
        // before teardown: buffer 1 scanning, buffer 2 submitted
        ("run1.state.buf1", "3"),
        ("run1.state.buf2", "2"),
        // The new engine knows no buffer and owes no event: the owing
        // release of buffer 1 and the presented events are lost with the
        // old engine (current behaviour; the producer must register again).
        ("init.state.buf1", "-1"),
        ("init.state.buf2", "-1"),
        ("init.events.presented", "0"),
        ("init.events.released", "0"),
        ("init.events.other", "0"),
        // registered again, buffers are ready, not in use
        ("reg.state.buf1", "1"),
        ("reg.state.buf2", "1"),
        ("reg.state.safe0", "1"),
        ("run2.events.presented", "1"),
        ("run2.events.released", "0"),
        ("run2.state.buf1", "3"),
    ]);
}
