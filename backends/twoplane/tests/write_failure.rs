//! A failed two-plane hardware write becomes a refusal.
//!
//! Host reference only, not a board result. The backend is C
//! (`ce_twoplane.c`) and reaches the panel through `struct ce_twoplane_hw`,
//! so a test supplies its own fake whose writes can fail, in a C program
//! built with the system C compiler (as in `tests/release_props.rs`; this
//! crate forbids unsafe code). The program drives six commits through the
//! engine's C API, two of them with a failing write (the application
//! buffer, then a safety flip), and prints one line per step; the tests read
//! those lines.
//!
//! Checked: the failing commit returns an error and no submission is
//! presented or dropped for it; the panel's latched count never passes its
//! commit count; a failed first write leaves the commit count where it was;
//! the next commit succeeds and is presented.
//!
//! No change to `src` was needed: the fake is the `ce_twoplane_hw` table.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

const RUNNER: &str = r#"
#include "ce_twoplane.h"
#include "composition_engine.h"
#include <stdio.h>

enum { W = 64, H = 32 };

/* Which hardware write fails: 0 none, 1 set_app_buffer, 3 safety_flip. */
static struct {
	int fail;
	uint32_t commits, latched, writes;
	uint32_t shadow_safety, active_safety;
	ce_twoplane_vsync_cb_t cb;
	void *cb_user;
	uint32_t safety_mem[2][4];
	uint32_t app_mem[2][4];
} fake;

static void wrote(void) { fake.commits++; fake.writes++; }
static bool hw_format_ok(uint32_t f) { return f == CE_FORMAT_RGB565 || f == CE_FORMAT_ARGB8888; }
static int hw_set_app(const void *d, const struct ce_twoplane_buf *b)
{
	(void)d; (void)b;
	if (fake.fail == 1) return -5;
	wrote();
	return 0;
}
static int hw_enable(const void *d, bool e)
{
	(void)d; (void)e;
	wrote();
	return 0;
}
static int hw_flip(const void *d)
{
	(void)d;
	if (fake.fail == 3) return -5;
	fake.shadow_safety ^= 1u;
	wrote();
	return 0;
}
static void *hw_safety_buffer(const void *d, uint32_t *w, uint32_t *h, uint32_t *s)
{
	(void)d; *w = W; *h = H; *s = W * 4;
	return fake.safety_mem[fake.shadow_safety ^ 1u];
}
static void *hw_front(const void *d) { (void)d; return fake.safety_mem[fake.active_safety]; }
static int hw_premul(const void *d, bool on) { (void)d; (void)on; return 0; }
static uint32_t hw_commit_count(const void *d) { (void)d; return fake.commits; }
static uint32_t hw_latched_count(const void *d) { (void)d; return fake.latched; }
static void hw_set_cb(const void *d, ce_twoplane_vsync_cb_t cb, void *u)
{
	(void)d; fake.cb = cb; fake.cb_user = u;
}
static const struct ce_twoplane_hw hw = {
	hw_format_ok, hw_set_app, hw_enable, hw_flip, hw_safety_buffer, hw_front,
	hw_premul, hw_commit_count, hw_latched_count, hw_set_cb,
};

static struct ce_twoplane_record table[4];
static unsigned int port_lock(void) { return 0; }
static void port_unlock(unsigned key) { (void)key; }
static void port_wake(void *a) { (void)a; }
static const struct ce_twoplane_record *port_lookup(void *a, uint32_t id)
{
	(void)a;
	for (size_t i = 0; i < 4; i++) if (table[i].id == id) return &table[i];
	return NULL;
}
static bool port_ready(const void *d) { (void)d; return true; }
static const struct ce_twoplane_port port = {
	port_lock, port_unlock, port_wake, NULL, port_lookup, NULL, port_ready,
};

static struct ce_twoplane backend;
static _Alignas(CE_ENGINE_ALIGN) unsigned char storage[CE_ENGINE_SIZE];
static ce_engine *engine;
static const uint32_t ids[4] = {1, 2, 10, 11};

static void frame_start(void)
{
	fake.latched = fake.commits;
	fake.active_safety = fake.shadow_safety;
	fake.cb(&fake, fake.cb_user);
}
static ce_layer layer(uint32_t buffer, int32_t z, uint8_t blend)
{
	return (ce_layer){.buffer = buffer, .source = {0, 0, W, H}, .destination = {0, 0, W, H},
			  .z_order = z, .alpha = CE_ALPHA_OPAQUE, .blend = blend,
			  .transform = CE_TRANSFORM_NORMAL};
}
static void add(int i, uint32_t id, uintptr_t pa, uint32_t fmt)
{
	table[i] = (struct ce_twoplane_record){.id = id, .seq = 1, .pa = pa, .width = W,
		.height = H, .stride_bytes = W * (fmt == CE_FORMAT_RGB565 ? 2 : 4), .format = fmt};
}

/* One line: STEP <name> <rc> <submission> <commits> <latched> <writes>
 * <states of 1 2 10 11>, taken after the events; then the events taken, as "E <kind> <id>". */
static void step(const char *name, int fail, uint32_t app, uint32_t safety)
{
	ce_layer l[2] = {layer(safety, 10, CE_BLEND_PREMULTIPLIED), layer(app, 0, CE_BLEND_OPAQUE)};
	uint64_t sub = 0;
	ce_event e;
	ce_frame fr = {.layers = l, .layer_count = 2, .output = 0};

	fake.fail = fail;
	int rc = ce_present(engine, &fr, &sub, NULL);
	fake.fail = 0;
	frame_start();
	char evs[512];
	size_t len = 0;

	evs[0] = 0;
	while (ce_poll_event(engine, &e) == 1 && len < sizeof(evs) - 64) {
		len += (size_t)snprintf(evs + len, sizeof(evs) - len, "E %u %llu\n", (unsigned)e.kind,
					e.kind == CE_EVENT_RELEASED ? (unsigned long long)e.buffer
								    : (unsigned long long)e.submission);
	}
	printf("STEP %s %d %llu %u %u %u", name, rc, (unsigned long long)sub, fake.commits,
	       fake.latched, fake.writes);
	for (int i = 0; i < 4; i++) printf(" %d", ce_buffer_state(engine, ids[i]));
	printf("\n%s", evs);
}

int main(void)
{
	add(0, 1, (uintptr_t)fake.app_mem[0], CE_FORMAT_RGB565);
	add(1, 2, (uintptr_t)fake.app_mem[1], CE_FORMAT_RGB565);
	add(2, 10, (uintptr_t)fake.safety_mem[0], CE_FORMAT_ARGB8888);
	add(3, 11, (uintptr_t)fake.safety_mem[1], CE_FORMAT_ARGB8888);
	if (ce_twoplane_init(&backend, &hw, &port, &fake) != 0) return 3;
	engine = ce_engine_init(storage, sizeof(storage), &ce_twoplane_ops, &backend);
	if (engine == NULL) return 4;
	for (int n = 0; n < 4; n++) {
		const ce_buffer_info info = {CE_MODIFIER_LINEAR, W, H,
			ids[n] >= 10 ? CE_FORMAT_ARGB8888 : CE_FORMAT_RGB565};
		if (ce_register_buffer(engine, ids[n], &info) != CE_OK ||
		    ce_finish_write(engine, ids[n]) != CE_OK) return 5;
	}
	step("first", 0, 1, 10);
	step("app-write-fails", 1, 2, 10);
	step("app-write-recovers", 0, 2, 10);
	step("safety-flip-fails", 3, 2, 11);
	step("safety-flip-recovers", 0, 2, 11);
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

/// The engine's static library, built by a nested cargo (see `tests/twoplane.rs`).
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
        let source = dir.join("write_failure_runner.c");
        std::fs::write(&source, RUNNER).unwrap();
        let program = dir.join("write_failure_runner");
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

struct Step {
    name: String,
    rc: i32,
    commits: u32,
    latched: u32,
    states: Vec<i32>,
    events: Vec<(u32, u64)>,
}

fn trace() -> Vec<Step> {
    let output = run(&mut Command::new(runner()), "the write-failure runner");
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut steps: Vec<Step> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f[0] {
            "STEP" => steps.push(Step {
                name: f[1].to_string(),
                rc: f[2].parse().unwrap(),
                commits: f[4].parse().unwrap(),
                latched: f[5].parse().unwrap(),
                states: f[7..].iter().map(|s| s.parse().unwrap()).collect(),
                events: Vec::new(),
            }),
            "E" => steps
                .last_mut()
                .unwrap()
                .events
                .push((f[1].parse().unwrap(), f[2].parse().unwrap())),
            _ => panic!("unexpected line: {line}"),
        }
    }
    steps
}

fn step<'a>(steps: &'a [Step], name: &str) -> &'a Step {
    steps.iter().find(|s| s.name == name).unwrap()
}

const PRESENTED: u32 = 0;
const DROPPED: u32 = 1;

fn no_verdict_events(s: &Step) {
    assert!(
        s.events.iter().all(|e| e.0 != PRESENTED && e.0 != DROPPED),
        "{}: events for a refused commit: {:?}",
        s.name,
        s.events
    );
}

#[test]
fn a_failed_application_write_is_refused_and_the_next_commit_recovers() {
    let steps = trace();
    let first = step(&steps, "first");
    assert_eq!(first.rc, 0);
    assert!(first.events.iter().any(|e| e.0 == PRESENTED));
    assert_eq!(first.latched, first.commits);

    let failed = step(&steps, "app-write-fails");
    assert!(failed.rc < 0, "the failed write gave {}", failed.rc);
    assert_eq!(failed.commits, first.commits, "no write reached the panel");
    assert_eq!(failed.latched, first.latched);
    no_verdict_events(failed);
    // buffer 2 was not accepted: not submitted, not on screen
    assert!(failed.states[1] <= 1, "buffer 2 state {}", failed.states[1]);

    let ok = step(&steps, "app-write-recovers");
    assert_eq!(ok.rc, 0);
    assert!(ok.commits > failed.commits);
    assert_eq!(ok.latched, ok.commits);
    assert!(ok.events.iter().any(|e| e.0 == PRESENTED));
    assert_eq!(ok.states[1], 3, "buffer 2 is on screen");
}

#[test]
fn a_failed_safety_flip_is_refused_and_the_next_commit_recovers() {
    let steps = trace();
    let before = step(&steps, "app-write-recovers");

    let failed = step(&steps, "safety-flip-fails");
    assert!(failed.rc < 0, "the failed flip gave {}", failed.rc);
    assert!(failed.latched <= failed.commits);
    no_verdict_events(failed);
    // buffer 11 was not accepted: not submitted, not on screen
    assert!(
        failed.states[3] <= 1,
        "buffer 11 state {}",
        failed.states[3]
    );
    assert_eq!(failed.states[2], before.states[2]);

    let ok = step(&steps, "safety-flip-recovers");
    assert_eq!(ok.rc, 0);
    assert!(ok.commits > failed.commits, "the flip is written now");
    assert_eq!(ok.latched, ok.commits);
    assert!(ok.events.iter().any(|e| e.0 == PRESENTED));
    assert_eq!(ok.states[3], 3, "safety buffer 11 is on screen");
}
