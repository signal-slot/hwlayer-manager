//! Seeded property tests of the two-plane backend's commit/latch accounting.
//!
//! The backend is C (`ce_twoplane.c`), and this crate forbids unsafe code, so
//! the tests do not call it: each test generates a random script of
//! commits, frame starts (VSYNC, with or without the interrupt, also in the
//! middle of a commit's writes), event polls, buffer redraws and refused
//! operations from a seed, runs it through a C program (the fake panel
//! below, the backend and the engine's C API, built with the system C
//! compiler like `tests/twoplane.rs`), and checks the program's trace:
//!
//! * the panel's latched count never exceeds its commit count, and the
//!   presented submissions never exceed the accepted ones;
//! * a submission is presented at most once, and never both presented and
//!   dropped;
//! * a submission the panel latched before the next commit's writes were
//!   complete is presented; one it did not is dropped (superseded), never
//!   presented;
//! * every buffer is released exactly once per use (once the events are
//!   taken, a buffer is in use if and only if it has one release owing);
//! * a refused commit, or a refused buffer call, changes no counter, no
//!   buffer state and no event.
//!
//! The backend counts nothing itself (its `vsyncs` field is the only
//! counter and the tests do not read it); the counters here are the fake
//! panel's commit and latched counts and the engine's events.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// The fake panel, the port, and the script runner. Modelled on
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
        let source = dir.join("latch_props_runner.c");
        std::fs::write(&source, RUNNER).unwrap();
        let program = dir.join("latch_props_runner");
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

/// SplitMix64: small, deterministic, good enough for scripts.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick(&mut self, items: &[u32]) -> u32 {
        items[self.below(items.len() as u64) as usize]
    }
}

/// A random script of `steps` operations (see the runner's switch).
fn script(seed: u64, steps: usize) -> String {
    let mut rng = Rng(seed);
    let mut out = String::new();
    // the script ends with a frame start and the events taken
    let end = "V\nE\n";
    for _ in 0..steps {
        let line = match rng.below(100) {
            // a commit: mostly valid, some the backend or the engine refuse
            0..=44 => {
                let app = if rng.below(10) == 0 {
                    0
                } else {
                    rng.pick(&[1, 1, 2, 2, 2, 5])
                };
                let safety = rng.pick(&[10, 10, 11, 11, 11]);
                let (app, safety) = match rng.below(14) {
                    0 => (3, safety),  // no address: refused
                    1 => (4, safety),  // not in the integrator's table
                    2 => (app, 12),    // not the driver's safety buffer
                    3 => (app, 13),    // not in the integrator's table
                    4 => (app, 0),     // no safety layer
                    5 => (99, safety), // not registered with the engine
                    _ => (app, safety),
                };
                // a frame start after the k-th write of this commit (0: none)
                let k = rng.pick(&[0, 0, 0, 1, 2, 3]);
                format!("P {app} {safety} {k} {}", rng.below(2))
            }
            45..=64 => "V".to_string(),
            65..=69 => "Q".to_string(),
            70..=84 => "E".to_string(),
            85..=91 => format!("D {}", rng.pick(&[1, 2, 5])),
            92..=96 => format!("W {}", rng.pick(&[1, 2, 5, 10, 11, 99])),
            _ => format!("F {}", rng.below(2)),
        };
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str(end);
    out
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

fn check_trace(seed: u64, trace: &str) {
    let (steps, end) = parse(trace);
    let ctx = |i: usize| format!("seed {seed:#x}, step {i}");

    let mut accepted: BTreeMap<u64, (usize, u32)> = BTreeMap::new(); // sub -> (step, pc)
    let mut presented: BTreeSet<u64> = BTreeSet::new();
    let mut dropped: BTreeSet<u64> = BTreeSet::new();
    let mut releases = [0i64; 9];
    // The buffer model, as the engine's header describes it: a frame on the
    // panel holds its buffers; when the next frame is presented, the buffers
    // it does not use are released; a frame dropped before it was latched
    // releases the buffers the panel is not showing.
    let mut owed = [0i64; 9];
    let mut frames: BTreeMap<u64, BTreeSet<usize>> = BTreeMap::new();
    let mut scanning: BTreeSet<usize> = BTreeSet::new();
    let mut last = steps[0].pre.clone();
    let mut present_ok = 0u32;
    let mut commit_steps = 0u32;
    let mut refused = 0u32;

    for (i, s) in steps.iter().enumerate() {
        // the pre-snapshot is where the last step left things
        assert_eq!(s.pre, last, "{}: state moved between steps", ctx(i));
        // the panel never latches more than was committed
        assert!(
            s.post.latched <= s.post.commits,
            "{}: latched > commits",
            ctx(i)
        );
        assert!(
            s.post.latched >= s.pre.latched,
            "{}: latched went back",
            ctx(i)
        );

        let failed = s.rc != 0;
        if failed {
            refused += 1;
            // a refused commit or buffer call changes nothing
            assert_eq!(
                s.pre,
                s.post,
                "{}: refused op {} changed state",
                ctx(i),
                s.op
            );
            assert!(s.events.is_empty(), "{}: refused op polled events", ctx(i));
        }
        if s.op == 'P' {
            commit_steps += 1;
        }
        if s.op == 'P' && !failed {
            present_ok += 1;
            assert!(
                accepted.insert(s.sub, (i, s.pc)).is_none(),
                "{}: submission {} accepted twice",
                ctx(i),
                s.sub
            );
            assert_eq!(s.pc, s.post.commits);
            frames.insert(
                s.sub,
                [s.layers.0, s.layers.1]
                    .iter()
                    .filter_map(|id| BUFFERS.iter().position(|b| b == id))
                    .collect(),
            );
        }

        for &(kind, id) in &s.events {
            match kind {
                'P' => {
                    assert!(
                        accepted.contains_key(&id),
                        "{}: presented {id} never accepted",
                        ctx(i)
                    );
                    assert!(
                        presented.insert(id),
                        "{}: submission {id} latched twice",
                        ctx(i)
                    );
                    assert!(
                        !dropped.contains(&id),
                        "{}: {id} dropped, then presented",
                        ctx(i)
                    );
                    // presented only once the panel reached its commit count
                    let pc = accepted[&id].1;
                    assert!(
                        s.post.latched.wrapping_sub(pc) as i32 >= 0,
                        "{}: {id} presented before the panel latched it",
                        ctx(i)
                    );
                }
                'D' => {
                    assert!(
                        accepted.contains_key(&id),
                        "{}: dropped {id} never accepted",
                        ctx(i)
                    );
                    assert!(
                        dropped.insert(id),
                        "{}: submission {id} dropped twice",
                        ctx(i)
                    );
                    assert!(
                        !presented.contains(&id),
                        "{}: {id} presented, then dropped",
                        ctx(i)
                    );
                }
                _ => {
                    let b = BUFFERS.iter().position(|&x| x == id).unwrap();
                    releases[b] += 1;
                }
            }
            // What the engine owes for this outcome (see `owed`).
            match kind {
                'P' => {
                    let frame = frames[&id].clone();
                    for b in &scanning {
                        if !frame.contains(b) {
                            owed[*b] += 1;
                        }
                    }
                    scanning = frame;
                }
                'D' => {
                    // the frame that took its place keeps what it shares
                    let next = frames.range(id + 1..).next().map(|(_, f)| f);
                    for b in &frames[&id] {
                        if !scanning.contains(b) && !next.is_some_and(|f| f.contains(b)) {
                            owed[*b] += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        // Every buffer is released exactly once for each time it leaves the
        // panel or a frame that never reached it, no more, no less.
        if !s.events.is_empty() {
            assert_eq!(
                releases,
                owed,
                "{}: releases against frames leaving the panel ({:?})",
                ctx(i),
                s.events
            );
        }
        // presented never exceeds accepted (hence commits), at any step
        assert!(
            presented.len() <= accepted.len(),
            "{}: more latches than commits",
            ctx(i)
        );
        assert!(
            (presented.len() + dropped.len()) as u32 <= present_ok,
            "{}: more outcomes than commits",
            ctx(i)
        );

        last = s.post.clone();
    }

    // The fate of a commit follows the panel: latched before the next commit's
    // writes were complete: presented; otherwise superseded, never presented.
    let commits: Vec<(u64, usize, u32)> =
        accepted.iter().map(|(&s, &(i, pc))| (s, i, pc)).collect();
    for (n, &(sub, step, pc)) in commits.iter().enumerate() {
        let (latched_by_next, what) = match commits.get(n + 1) {
            Some(&(_, next, _)) => (steps[next].post.latched, "next commit"),
            None => (end.latched, "the end"),
        };
        let reached = latched_by_next.wrapping_sub(pc) as i32 >= 0;
        assert_eq!(
            presented.contains(&sub),
            reached,
            "seed {seed:#x}: submission {sub} (step {step}), panel latched {latched_by_next} \
             of commit count {pc} by {what}"
        );
        assert_eq!(
            dropped.contains(&sub),
            !reached,
            "seed {seed:#x}: submission {sub}"
        );
    }
    // every commit has exactly one outcome
    assert_eq!(presented.len() + dropped.len(), accepted.len());
    assert!(end.latched <= end.commits);
    assert_eq!(
        end.latched, end.commits,
        "the last frame start latches everything"
    );

    // after the last drain: a buffer is in use iff the last frame shows it,
    // and every release was owed
    assert_eq!(releases, owed, "seed {seed:#x}: releases");
    for (b, state) in end.states.iter().enumerate() {
        assert_eq!(
            matches!(*state, SUBMITTED | SCANNING),
            scanning.contains(&b),
            "seed {seed:#x}: buffer {} state {state}",
            BUFFERS[b]
        );
    }
    // the script exercised what it claims to
    assert!(
        present_ok >= 10,
        "seed {seed:#x}: only {present_ok} commits accepted"
    );
    assert!(refused >= 1, "seed {seed:#x}: nothing was refused");
    assert!(
        commit_steps > present_ok,
        "seed {seed:#x}: no commit refused"
    );
    assert!(!dropped.is_empty(), "seed {seed:#x}: nothing superseded");
    assert!(!presented.is_empty(), "seed {seed:#x}: nothing latched");
}

fn property(seed: u64, steps: usize) {
    let program = runner();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let path = dir.join(format!("latch_props_{seed:x}.script"));
    std::fs::write(&path, script(seed, steps)).unwrap();
    let output = run(Command::new(program).arg(&path), "the latch runner");
    let trace = String::from_utf8_lossy(&output.stdout);
    // kept next to the script, for reading a failure
    std::fs::write(path.with_extension("trace"), trace.as_bytes()).unwrap();
    check_trace(seed, &trace);
}

#[test]
fn latch_accounting_seed_1() {
    property(1, 300);
}

#[test]
fn latch_accounting_seed_c0ffee() {
    property(0xC0FFEE, 300);
}

#[test]
fn latch_accounting_seed_deadbeef() {
    property(0xDEAD_BEEF, 300);
}

#[test]
fn latch_accounting_seed_2026() {
    property(2026, 500);
}

#[test]
fn latch_accounting_seed_f00d() {
    property(0xF00D_CAFE_1234_5678, 500);
}

#[test]
fn the_script_is_deterministic() {
    assert_eq!(script(7, 200), script(7, 200));
    assert_ne!(script(7, 200), script(8, 200));
}
