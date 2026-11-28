//! The two-plane backend across the wrap of the hardware's 32-bit commit and
//! latched counts.
//!
//! The backend is C (`ce_twoplane.c`) and this crate forbids unsafe code, so
//! the test builds a C program (a fake panel, the backend and the engine's C
//! API, with the system C compiler like `tests/latch_props.rs`) and runs it
//! with the counters starting at a value given on the command line. The
//! program runs one scenario: commits and frame starts, a commit that
//! replaces a pending one, a frame start in the middle of a commit's writes,
//! and a commit refused with BUSY. It prints the engine's events and the
//! backend's answers, and the counts as differences, which do not depend on
//! where the counters start. The trace with counters starting at zero is the
//! reference; every start just below (and around) the wrap point must give
//! the same trace: the same PRESENTED, DROPPED and RELEASED events in the
//! same order, and the same refusals.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

const RUNNER: &str = r#"
#include "ce_twoplane.h"
#include "composition_engine.h"
#include <stdio.h>
#include <stdlib.h>

enum { W = 64, H = 32 };
enum { APP_A = 1, APP_B = 2, SAFETY_0 = 10, SAFETY_1 = 11 };

struct regs { bool app_on; unsigned safety; };
static struct {
	struct regs shadow, active;
	uint32_t commits, latched, start_commits;
	unsigned writes, lock_depth, max_depth;
	bool deferred, bad_unlock;
	unsigned start_after_write, start_at_format_ok;
	bool start_isr;
	ce_twoplane_vsync_cb_t cb;
	void *cb_user;
	uint32_t safety_mem[2][4];
	uint32_t app_mem[2][4];
} fake;

static void run_isr(void)
{
	if (fake.cb == NULL) return;
	if (fake.lock_depth > 0) { fake.deferred = true; return; }
	fake.cb(&fake, fake.cb_user);
}
static void frame_start(bool isr)
{
	if (fake.latched != fake.commits) {
		fake.active = fake.shadow;
		fake.latched = fake.commits;
	}
	if (isr) run_isr();
}
static void wrote(void)
{
	fake.commits++;
	fake.writes++;
	if (fake.start_after_write > 0 && --fake.start_after_write == 0) frame_start(fake.start_isr);
}
static bool hw_app_format_ok(uint32_t f)
{
	if (fake.start_at_format_ok > 0 && --fake.start_at_format_ok == 0) frame_start(fake.start_isr);
	return f == CE_FORMAT_RGB565 || f == CE_FORMAT_ARGB8888;
}
static int hw_set_app_buffer(const void *d, const struct ce_twoplane_buf *b) { (void)d; (void)b; wrote(); return 0; }
static int hw_enable_app_plane(const void *d, bool e) { (void)d; fake.shadow.app_on = e; wrote(); return 0; }
static int hw_safety_flip(const void *d) { (void)d; fake.shadow.safety ^= 1u; wrote(); return 0; }
static void *hw_safety_buffer(const void *d, uint32_t *w, uint32_t *h, uint32_t *s)
{
	(void)d; *w = W; *h = H; *s = W * 4;
	return fake.safety_mem[fake.shadow.safety ^ 1u];
}
static void *hw_safety_front(const void *d) { (void)d; return fake.safety_mem[fake.active.safety]; }
static int hw_safety_premultiplied(const void *d, bool on) { (void)d; (void)on; return 0; }
static uint32_t hw_commit_count(const void *d) { (void)d; return fake.commits; }
static uint32_t hw_latched_count(const void *d) { (void)d; return fake.latched; }
static void hw_set_vsync_callback(const void *d, ce_twoplane_vsync_cb_t cb, void *u)
{
	(void)d; fake.cb = cb; fake.cb_user = u;
}
static const struct ce_twoplane_hw hw = {
	hw_app_format_ok, hw_set_app_buffer, hw_enable_app_plane, hw_safety_flip,
	hw_safety_buffer, hw_safety_front, hw_safety_premultiplied, hw_commit_count,
	hw_latched_count, hw_set_vsync_callback,
};

static struct ce_twoplane_record table[4];
static size_t table_len;
static unsigned int port_irq_lock(void)
{
	unsigned key = fake.lock_depth++;
	if (fake.lock_depth > fake.max_depth) fake.max_depth = fake.lock_depth;
	return key;
}
static void port_irq_unlock(unsigned key)
{
	if (fake.lock_depth == 0 || key != fake.lock_depth - 1) { fake.bad_unlock = true; return; }
	fake.lock_depth = key;
	if (fake.lock_depth == 0 && fake.deferred) { fake.deferred = false; run_isr(); }
}
static void port_wake(void *a) { (void)a; }
static const struct ce_twoplane_record *port_lookup(void *a, uint32_t id)
{
	(void)a;
	for (size_t i = 0; i < table_len; i++) if (table[i].id == id) return &table[i];
	return NULL;
}
static bool port_ready(const void *d) { (void)d; return true; }
static const struct ce_twoplane_port port = {
	port_irq_lock, port_irq_unlock, port_wake, NULL, port_lookup, NULL, port_ready,
};
static void add_record(uint32_t id, uintptr_t pa, uint32_t format)
{
	table[table_len++] = (struct ce_twoplane_record){
		.id = id, .seq = 1, .pa = pa, .width = W, .height = H,
		.stride_bytes = W * (format == CE_FORMAT_RGB565 ? 2 : 4), .format = format};
}

static struct ce_twoplane backend;
static _Alignas(CE_ENGINE_ALIGN) unsigned char storage[CE_ENGINE_SIZE];
static ce_engine *engine;
static uint64_t first_sub;

static ce_layer layer(uint32_t buffer, int32_t z, uint8_t blend)
{
	return (ce_layer){.buffer = buffer, .source = {0, 0, W, H}, .destination = {0, 0, W, H},
			  .z_order = z, .alpha = CE_ALPHA_OPAQUE, .blend = blend,
			  .transform = CE_TRANSFORM_NORMAL};
}
/* Differences only: independent of where the counters started. */
static void line(const char *what, int rc, uint64_t sub)
{
	printf("%s rc=%d sub=%llu pending=%u writes=%u\n", what, rc,
	       sub ? (unsigned long long)(sub - first_sub + 1) : 0ull,
	       (unsigned)(fake.commits - fake.latched), fake.writes);
}
static void drain(const char *what)
{
	ce_event e;
	int32_t rc;
	unsigned n = 0;
	while ((rc = ce_poll_event(engine, &e)) == 1) {
		unsigned long long sub = (unsigned long long)(e.submission - first_sub + 1);
		n++;
		if (e.kind == CE_EVENT_PRESENTED) printf("%s PRESENTED %llu\n", what, sub);
		else if (e.kind == CE_EVENT_DROPPED) printf("%s DROPPED %llu\n", what, sub);
		else if (e.kind == CE_EVENT_RELEASED) printf("%s RELEASED %llu\n", what, (unsigned long long)e.buffer);
		else printf("%s UNKNOWN %u\n", what, e.kind);
	}
	if (rc != 0) printf("%s POLL_ERROR %d\n", what, rc);
	printf("%s events=%u\n", what, n);
}
static int present(const char *what, uint32_t app, uint32_t safety, ce_error_info *info)
{
	ce_layer l[2] = {layer(safety, 10, CE_BLEND_PREMULTIPLIED), layer(app, 0, CE_BLEND_OPAQUE)};
	ce_frame fr = {.layers = l, .layer_count = 2, .output = 0};
	uint64_t s = 0;
	int rc = ce_present(engine, &fr, &s, info);
	if (first_sub == 0 && rc == 0) first_sub = s;
	line(what, rc, rc == 0 ? s : 0);
	return rc;
}

int main(int argc, char **argv)
{
	ce_error_info info = {0};

	if (argc != 2) return 2;
	fake.commits = fake.latched = fake.start_commits = (uint32_t)strtoul(argv[1], NULL, 0);
	add_record(APP_A, (uintptr_t)fake.app_mem[0], CE_FORMAT_RGB565);
	add_record(APP_B, (uintptr_t)fake.app_mem[1], CE_FORMAT_RGB565);
	add_record(SAFETY_0, (uintptr_t)fake.safety_mem[0], CE_FORMAT_ARGB8888);
	add_record(SAFETY_1, (uintptr_t)fake.safety_mem[1], CE_FORMAT_ARGB8888);
	if (ce_twoplane_init(&backend, &hw, &port, &fake) != 0) return 3;
	engine = ce_engine_init(storage, sizeof(storage), &ce_twoplane_ops, &backend);
	if (engine == NULL) return 4;
	static const uint32_t ids[4] = {APP_A, APP_B, SAFETY_0, SAFETY_1};
	for (int n = 0; n < 4; n++) {
		const ce_buffer_info bi = {CE_MODIFIER_LINEAR, W, H,
					   ids[n] >= 10 ? CE_FORMAT_ARGB8888 : CE_FORMAT_RGB565};
		if (ce_register_buffer(engine, ids[n], &bi) != CE_OK ||
		    ce_finish_write(engine, ids[n]) != CE_OK) return 5;
	}

	/* 1. commit (set, enable, flip), nothing latched yet, then a frame start */
	present("c1", APP_A, SAFETY_0, &info);
	drain("c1-before-vsync");
	frame_start(true);
	drain("c1-after-vsync");
	/* 2. a commit replaced while pending: the first never reaches the panel */
	present("c2", APP_B, SAFETY_1, &info);
	present("c3", APP_A, SAFETY_0, &info);
	drain("c3-before-vsync");
	frame_start(false); /* no interrupt: the latch is found by polling */
	drain("c3-after-poll");
	/* 3. a frame start in the middle of a commit's writes: the first commit
	 * is on the panel, the second is not complete and stays pending */
	present("c4", APP_B, SAFETY_1, &info);
	fake.start_after_write = 1;
	fake.start_isr = false;
	present("c5", APP_A, SAFETY_0, &info);
	fake.start_after_write = 0;
	drain("c5-mid");
	frame_start(true);
	drain("c5-after-vsync");
	/* 4. BUSY: the interrupt notes a latch after the engine collected its
	 * latches and before the commit looks at the slot */
	present("c6", APP_B, SAFETY_1, &info);
	fake.start_at_format_ok = 2;
	fake.start_isr = true;
	int rc = present("busy", APP_A, SAFETY_0, &info);
	printf("busy status=%d\n", (int)info.status);
	fake.start_at_format_ok = 0;
	drain("busy-latch");
	if (rc != 0) present("retry", APP_A, SAFETY_0, &info);
	frame_start(true);
	drain("retry-after-vsync");
	present("c7", APP_B, SAFETY_1, &info);
	frame_start(true);
	present("c8", APP_A, SAFETY_0, &info);
	frame_start(true);
	drain("end");
	printf("misuse lock=%u bad_unlock=%d nested=%d\n", fake.lock_depth, fake.bad_unlock,
	       fake.max_depth > 1);
	printf("counts commits-latched=%u\n", (unsigned)(fake.commits - fake.latched));
	/* how far the counters moved, and whether they went past zero */
	printf("WRAPPED %d %u\n", fake.commits < fake.start_commits,
	       (unsigned)(fake.commits - fake.start_commits));
	return 0;
}
"#;

fn backend_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

/// The static library of the engine, built by a nested cargo (see
/// `tests/twoplane.rs`).
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
        })
        .unwrap_or_else(|| panic!("cargo reported no libcomposition_engine.a:\n{messages}"));
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
        let source = dir.join("counter_wrap_runner.c");
        std::fs::write(&source, RUNNER).unwrap();
        let program = dir.join("counter_wrap_runner");
        run(
            Command::new(std::env::var("CC").unwrap_or_else(|_| "cc".into()))
                .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
                .arg("-I")
                .arg(backend_dir().join("../../crates/composition-ffi/include"))
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

/// The trace with the counters starting at `start`.
fn trace(start: u32) -> String {
    let output = run(
        Command::new(runner()).arg(start.to_string()),
        "the counter runner",
    );
    String::from_utf8(output.stdout).unwrap()
}

/// The trace without the line where the counters ended.
fn events(trace: &str) -> String {
    trace
        .lines()
        .filter(|l| !l.starts_with("WRAPPED"))
        .map(|l| format!("{l}\n"))
        .collect()
}

/// Whether the commit count went past zero, and how far the count moved.
fn moved(trace: &str) -> (bool, u32) {
    let l = trace.lines().find(|l| l.starts_with("WRAPPED")).unwrap();
    let f: Vec<&str> = l.split_whitespace().collect();
    (f[1] == "1", f[2].parse().unwrap())
}

#[test]
fn reference_run_away_from_the_wrap() {
    let t = trace(0);
    let e = events(&t);
    // the scenario does what it says
    assert!(!moved(&t).0);
    assert!(moved(&t).1 >= 12, "too few commits: {t}");
    // c1: presented at the frame start, not before
    assert!(e.contains("c1-before-vsync events=0"), "{e}");
    assert!(e.contains("c1-after-vsync PRESENTED 1"), "{e}");
    // c2 replaced by c3 while pending: c2 is dropped, c3 presented
    assert!(e.contains("c3-before-vsync DROPPED 2"), "{e}");
    assert!(!e.contains("PRESENTED 2"), "{e}");
    assert!(e.contains("c3-after-poll PRESENTED 3"), "{e}");
    // c4 latched in the middle of c5's writes
    assert!(e.contains("c5-mid PRESENTED 4"), "{e}");
    assert!(e.contains("c5-after-vsync PRESENTED 5"), "{e}");
    // BUSY refusal: the retry works
    assert!(e.contains("busy rc=-"), "no BUSY refusal: {e}");
    assert!(e.contains("RELEASED"), "no release: {e}");
    assert!(e.contains("retry rc=0"), "{e}");
    assert!(e.contains("misuse lock=0 bad_unlock=0 nested=0"), "{e}");
    assert!(e.contains("counts commits-latched=0"), "{e}");
}

#[test]
fn same_events_across_the_wrap() {
    let reference = trace(0);
    let reference_events = events(&reference);
    let reference_moved = moved(&reference).1;
    // starts from 40 commits before the wrap to the wrap itself, and around
    // the signed boundary (where a signed compare would go wrong)
    let starts: Vec<u32> = (0..=40u32)
        .map(|k| u32::MAX - k)
        .chain([
            0x7FFF_FFF0,
            0x7FFF_FFFE,
            0x7FFF_FFFF,
            0x8000_0000,
            0x8000_0001,
        ])
        .collect();
    let mut crossed = 0;
    for start in starts {
        let t = trace(start);
        let (wrapped, by) = moved(&t);
        crossed += usize::from(wrapped);
        assert_eq!(by, reference_moved, "start {start:#x}");
        assert_eq!(
            events(&t),
            reference_events,
            "start {start:#x}: events differ from the run away from the wrap"
        );
    }
    // every start within the scenario's own length of the wrap crosses it
    assert_eq!(crossed, reference_moved as usize, "runs crossing the wrap");
}
