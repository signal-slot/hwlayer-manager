//! Host reference for three two-plane GO-bit timing cases (not a board result).
//!
//! Like `tests/latch_props.rs`, the backend is C and this crate forbids
//! unsafe code, so each case is a short script run through a C program (the
//! fake panel below, the backend and the engine's C API). The fake panel
//! models the GO bit as its commit count: every write counts one commit, and
//! a frame start (VSYNC) makes the latched count equal the commit count.
//!
//! The cases: a commit after the frame's latch and before the next vsync
//! (`late_commit_after_go_set`), two commits before one vsync
//! (`two_commits_one_frame`), and a commit with no vsync
//! (`commit_without_vsync`). Where the docs say nothing, the assertions are
//! the current behaviour and are marked so.
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// The fake panel, the port and the script runner, as in `tests/latch_props.rs`.
/// `tests/c/twoplane_test.c`: every write goes to shadow registers and
/// counts one commit; a frame start makes the latched count equal the
/// commit count, then runs the vsync callback (not while the port's
/// interrupt lock is held: it runs at the unlock).
const RUNNER: &str = r#"
#include "ce_twoplane.h"
#include "composition_engine.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { W = 64, H = 32 };
enum { NBUF = 9 };
static const uint32_t ids[NBUF] = {1, 2, 3, 4, 5, 10, 11, 12, 13};
/* 1, 2: application buffers; 3: no address; 4: not in the table; 5: XRGB8888;
 * 10, 11: the driver's safety buffers; 12: not the driver's; 13: not in the table */

struct regs { bool app_on; unsigned safety; };
static struct {
	struct regs shadow, active;
	uint32_t commits, latched;
	unsigned writes, vsyncs, lock_depth, max_depth;
	bool deferred, bad_unlock;
	bool xrgb_ok;
	unsigned start_after_write;
	bool start_isr;
	ce_twoplane_vsync_cb_t cb;
	void *cb_user;
	uint32_t safety_mem[2][4];
	uint32_t other_mem[4];
	uint32_t app_mem[3][4];
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
	return f == CE_FORMAT_RGB565 || f == CE_FORMAT_ARGB8888 ||
	       (f == CE_FORMAT_XRGB8888 && fake.xrgb_ok);
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

static struct ce_twoplane_record table[8];
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
static void port_wake(void *a) { (void)a; fake.vsyncs++; }
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
static unsigned polled;

static ce_layer layer(uint32_t buffer, int32_t z, uint8_t blend)
{
	return (ce_layer){.buffer = buffer, .source = {0, 0, W, H}, .destination = {0, 0, W, H},
			  .z_order = z, .alpha = CE_ALPHA_OPAQUE, .blend = blend,
			  .transform = CE_TRANSFORM_NORMAL};
}
static void snap(void)
{
	printf(" %u %u %u %u", fake.commits, fake.latched, fake.writes, polled);
	for (int i = 0; i < NBUF; i++) printf(" %d", ce_buffer_state(engine, ids[i]));
}
/* Events taken, as lines printed after the step's own line. */
static char evbuf[8192];
static size_t evlen;
static void ev(const char *fmt, unsigned a, unsigned long long b)
{
	evlen += (size_t)snprintf(evbuf + evlen, sizeof(evbuf) - evlen, fmt, a, b);
}
static void drain(void)
{
	ce_event e;
	int32_t rc;
	while ((rc = ce_poll_event(engine, &e)) == 1) {
		polled++;
		static const char k[] = "PDR";
		if (e.kind > CE_EVENT_RELEASED) { ev("E ? %u %llu\n", e.kind, 0); continue; }
		ev("E %c %llu\n", (unsigned)k[e.kind],
		   e.kind == CE_EVENT_RELEASED ? e.buffer : e.submission);
	}
	if (rc != 0) ev("E ! %u %llu\n", (unsigned)-rc, 0);
}

int main(int argc, char **argv)
{
	FILE *f;
	char op;
	unsigned i = 0;

	if (argc != 2 || (f = fopen(argv[1], "r")) == NULL) return 2;
	fake.xrgb_ok = true;
	add_record(1, (uintptr_t)fake.app_mem[0], CE_FORMAT_RGB565);
	add_record(2, (uintptr_t)fake.app_mem[1], CE_FORMAT_RGB565);
	add_record(3, 0, CE_FORMAT_RGB565);
	add_record(5, (uintptr_t)fake.app_mem[2], CE_FORMAT_XRGB8888);
	add_record(10, (uintptr_t)fake.safety_mem[0], CE_FORMAT_ARGB8888);
	add_record(11, (uintptr_t)fake.safety_mem[1], CE_FORMAT_ARGB8888);
	add_record(12, (uintptr_t)fake.other_mem, CE_FORMAT_ARGB8888);
	if (ce_twoplane_init(&backend, &hw, &port, &fake) != 0) return 3;
	engine = ce_engine_init(storage, sizeof(storage), &ce_twoplane_ops, &backend);
	if (engine == NULL) return 4;
	for (int n = 0; n < NBUF; n++) {
		uint32_t fmt = ids[n] >= 10 ? CE_FORMAT_ARGB8888 :
			       ids[n] == 5 ? CE_FORMAT_XRGB8888 : CE_FORMAT_RGB565;
		const ce_buffer_info info = {CE_MODIFIER_LINEAR, W, H, fmt};
		if (ce_register_buffer(engine, ids[n], &info) != CE_OK ||
		    ce_finish_write(engine, ids[n]) != CE_OK) return 5;
	}
	printf("B");
	snap();
	printf("\n");
	while (fscanf(f, " %c", &op) == 1) {
		int rc = 0;
		uint64_t sub = 0;
		unsigned a = 0, s = 0, k = 0, isr = 0;
		uint32_t pc = 0;

		/* the layers of a commit, as "app,safety", in the step's line */
		if (op == 'P') {
			long pos = ftell(f);
			if (fscanf(f, "%u %u", &a, &s) != 2) return 6;
			fseek(f, pos, SEEK_SET);
		}
		printf("S %u %c %u,%u", i++, op, a, s);
		switch (op) {
		case 'P': { /* present: app buffer (0: none), safety buffer (0: none) */
			if (fscanf(f, "%u %u %u %u", &a, &s, &k, &isr) != 4) return 6;
			ce_layer l[2];
			size_t n = 0;
			if (s) l[n++] = layer(s, 10, CE_BLEND_PREMULTIPLIED);
			if (a) l[n++] = layer(a, 0, CE_BLEND_OPAQUE);
			ce_frame fr = {.layers = l, .layer_count = n, .output = 0};
			fake.start_after_write = k;
			fake.start_isr = isr != 0;
			snap();
			rc = ce_present(engine, &fr, &sub, NULL);
			fake.start_after_write = 0;
			pc = fake.commits;
			break;
		}
		case 'V': snap(); frame_start(true); break;
		case 'Q': snap(); frame_start(false); break;
		case 'E': snap(); break;
		case 'D': /* the application drew buffer a again */
			if (fscanf(f, "%u", &a) != 1) return 6;
			snap();
			for (size_t t = 0; t < table_len; t++) if (table[t].id == a) table[t].seq++;
			break;
		case 'W': /* begin_write, and finish_write if it was taken */
			if (fscanf(f, "%u", &a) != 1) return 6;
			snap();
			rc = ce_begin_write(engine, a);
			if (rc == CE_OK) rc = ce_finish_write(engine, a);
			break;
		case 'F': /* XRGB8888 on the application plane: on or off */
			if (fscanf(f, "%u", &a) != 1) return 6;
			snap();
			fake.xrgb_ok = a != 0;
			break;
		default: return 7;
		}
		/* events are taken after some steps only: the engine queues the rest;
		 * a refused call takes none */
		if (rc == 0 && (op == 'E' || op == 'V' || op == 'Q' || op == 'W' ||
				(op == 'P' && (a + s) % 3 == 0))) {
			drain();
		}
		printf(" |%d %llu %u", rc, rc == 0 && op == 'P' ? (unsigned long long)sub : 0ull, pc);
		snap();
		printf(" %u %u\n", fake.lock_depth, fake.bad_unlock + 2 * (fake.max_depth > 1));
		fputs(evbuf, stdout);
		evlen = 0;
		evbuf[0] = 0;
		printf("T\n");
	}
	frame_start(true);
	drain();
	fputs(evbuf, stdout);
	printf("END");
	snap();
	printf("\n");
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

/// The static library of the engine, built by a nested cargo (see
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

/// The runner program, built once for all tests.
fn runner() -> &'static PathBuf {
    static RUNNER_PATH: OnceLock<PathBuf> = OnceLock::new();
    RUNNER_PATH.get_or_init(|| {
        let library = build_static_library();
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        let source = dir.join("late_commit_runner.c");
        std::fs::write(&source, RUNNER).unwrap();
        let program = dir.join("late_commit_runner");
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

/// What the runner prints around one step (the numbers are cumulative).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snap {
    commits: u32,
    latched: u32,
    writes: u32,
    polled: u32,
    states: Vec<i32>,
}

impl Snap {
    fn parse(fields: &[&str]) -> Snap {
        let n: Vec<i64> = fields.iter().map(|f| f.parse().unwrap()).collect();
        Snap {
            commits: n[0] as u32,
            latched: n[1] as u32,
            writes: n[2] as u32,
            polled: n[3] as u32,
            states: n[4..].iter().map(|&s| s as i32).collect(),
        }
    }
}

#[allow(dead_code)]
struct Step {
    op: char,
    /// the buffers of a commit (application, safety; 0: none)
    layers: (u64, u64),
    pre: Snap,
    rc: i32,
    sub: u64,
    pc: u32,
    post: Snap,
    events: Vec<(char, u64)>,
}

const BUFFERS: [u64; 9] = [1, 2, 3, 4, 5, 10, 11, 12, 13];
const SUBMITTED: i32 = 2;
const SCANNING: i32 = 3;

fn parse(trace: &str) -> (Vec<Step>, Snap) {
    let mut steps: Vec<Step> = Vec::new();
    let mut end = None;
    for line in trace.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f[0] {
            "B" => {}
            "S" => {
                let bar = f.iter().position(|x| x.starts_with('|')).unwrap();
                steps.push(Step {
                    op: f[2].chars().next().unwrap(),
                    layers: {
                        let (a, s) = f[3].split_once(',').unwrap();
                        (a.parse().unwrap(), s.parse().unwrap())
                    },
                    // after "S", the index, the op and the layers
                    pre: Snap::parse(&f[4..bar]),
                    rc: f[bar][1..].parse().unwrap(),
                    sub: f[bar + 1].parse().unwrap(),
                    pc: f[bar + 2].parse().unwrap(),
                    post: Snap::parse(&f[bar + 3..bar + 3 + 4 + 9]),
                    events: Vec::new(),
                });
                // port misuse flags: lock depth zero, no bad unlock or nesting
                assert_eq!(&f[f.len() - 2..], ["0", "0"], "port misused: {line}");
            }
            "E" => {
                let kind = f[1].chars().next().unwrap();
                assert!(matches!(kind, 'P' | 'D' | 'R'), "bad event: {line}");
                steps
                    .last_mut()
                    .unwrap()
                    .events
                    .push((kind, f[2].parse().unwrap()));
            }
            "T" => {}
            "END" => end = Some(Snap::parse(&f[1..])),
            other => panic!("unexpected trace line {other}: {line}"),
        }
    }
    (steps, end.expect("trace has no END"))
}

/// Runs a script and returns its steps and the final snapshot.
fn trace(name: &str, script: &str) -> (Vec<Step>, Snap) {
    let program = runner();
    let path =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("late_commit_{name}.script"));
    std::fs::write(&path, script).unwrap();
    let output = run(Command::new(program).arg(&path), "the late-commit runner");
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    parse(&text)
}

/// Index of a buffer's state in a snapshot.
fn state(snap: &Snap, buffer: u64) -> i32 {
    snap.states[BUFFERS.iter().position(|&b| b == buffer).unwrap()]
}

/// A commit (application buffer 2, safety buffer 11) arrives after frame 1
/// was latched and before the next vsync: it latches on that vsync, not on
/// the one that already happened.
#[test]
fn late_commit_after_go_set() {
    let (steps, end) = trace("late", "P 1 10 0 0\nV\nP 2 11 0 0\nE\nV\nE\n");
    // frame 1: two writes, latched at the first vsync, presented there
    assert_eq!((steps[0].post.commits, steps[0].post.latched), (2, 0));
    assert_eq!((steps[1].post.commits, steps[1].post.latched), (2, 2));
    assert_eq!(steps[1].events, [('P', steps[0].sub)]);
    // the late commit: counted, not latched, nothing presented or released
    assert_eq!((steps[2].post.commits, steps[2].post.latched), (4, 2));
    assert!(steps[3].events.is_empty());
    assert_eq!((steps[3].post.commits, steps[3].post.latched), (4, 2));
    assert_eq!(state(&steps[3].post, 1), SCANNING);
    assert_eq!(state(&steps[3].post, 2), SUBMITTED);
    // the next vsync latches it: its presentation, and the release of the
    // buffers of frame 1 that it replaces
    assert_eq!((steps[4].post.commits, steps[4].post.latched), (4, 4));
    assert_eq!(steps[4].events, [('P', steps[2].sub), ('R', 1), ('R', 10)]);
    assert_eq!(state(&end, 2), SCANNING);
    assert_eq!(state(&end, 11), SCANNING);
    assert_eq!((end.commits, end.latched), (4, 4));
}

/// Two commits before one vsync: only the second latches. Current
/// behaviour (the docs do not say): the first is reported as dropped
/// (DROPPED event) at the vsync and its buffers are released then, before the
/// second one's presentation event.
#[test]
fn two_commits_one_frame() {
    let (steps, end) = trace("two", "P 1 10 0 0\nP 2 11 0 0\nV\nE\n");
    let (first, second) = (steps[0].sub, steps[1].sub);
    assert_ne!(first, second);
    assert_eq!((steps[1].post.commits, steps[1].post.latched), (4, 0));
    // nothing is taken before the vsync (the runner polls no events at a
    // commit of these buffers)
    assert!(steps[0].events.is_empty() && steps[1].events.is_empty());
    // current behaviour: the second commit supersedes the first in the
    // engine at once, so the first's buffer is idle (release queued) and the
    // second's is submitted, before the vsync
    assert_eq!(state(&steps[1].post, 1), 1);
    assert_eq!(state(&steps[1].post, 2), SUBMITTED);
    // the vsync latches both commits' writes at once: one latched value
    assert_eq!((steps[2].post.commits, steps[2].post.latched), (4, 4));
    // current behaviour: DROPPED for the first, its buffers released, then
    // PRESENTED for the second only
    assert_eq!(
        steps[2].events,
        [('D', first), ('R', 1), ('R', 10), ('P', second)]
    );
    assert_eq!(state(&end, 1), 1);
    assert_eq!(state(&end, 10), 1);
    assert_eq!(state(&end, 2), SCANNING);
    assert_eq!(state(&end, 11), SCANNING);
}

/// A commit and no vsync: nothing latches, nothing is presented, nothing is
/// released.
#[test]
fn commit_without_vsync() {
    let (steps, end) = trace("novsync", "P 1 10 0 0\nE\n");
    // the commit's two writes are counted and none is latched
    assert_eq!(steps[0].rc, 0);
    assert_eq!((steps[0].post.commits, steps[0].post.latched), (2, 0));
    assert_eq!(state(&steps[0].post, 1), SUBMITTED);
    assert_eq!(state(&steps[0].post, 10), SUBMITTED);
    // events taken afterwards: none (the runner's own final drain, after
    // its last frame start, is listed under the last step), and the counters
    // did not move
    assert!(steps[0].events.is_empty());
    assert_eq!(steps[1].post.polled, 0);
    assert_eq!((steps[1].post.commits, steps[1].post.latched), (2, 0));
    assert_eq!(state(&steps[1].post, 1), SUBMITTED);
    assert_eq!(state(&steps[1].post, 10), SUBMITTED);
    // (the runner's final frame start, after the script, latches it: only
    // that shows the commit was held back, not lost)
    assert_eq!((end.commits, end.latched), (2, 2));
}
