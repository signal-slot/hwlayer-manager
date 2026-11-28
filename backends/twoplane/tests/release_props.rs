//! Small seeded test of the two-plane backend's buffer releases and refusals.
//!
//! Host reference only, not a board result. A fake panel, the backend (C,
//! `ce_twoplane.c`) and the engine's C API run in a C program built with the
//! system C compiler (as in `tests/latch_props.rs`; this crate forbids unsafe
//! code). A fixed list of seeds each makes a script of 64 steps (commits,
//! frame starts, event polls, a format switch of the fake plane). The program
//! prints a trace; the checks below read it.
//!
//! Properties:
//! 1. Every buffer handed to the backend is released exactly once: on
//!    supersede (the next latch) or when a later frame replaces it on the
//!    panel. Never twice, never one that was not handed over.
//! 2. A commit the backend refuses (no safety layer, a buffer with no
//!    address, one not in the table, a safety buffer that is not the
//!    driver's, a format the plane lacks, a third layer) returns an error,
//!    changes no panel counter and no buffer state, and queues no event.
//! 3. After the final latch and drain, the buffers in use are exactly those
//!    of the frame on the panel.
//!
//! Not observed: a refusal caused by a failing hardware write. The fake's
//! writes never fail, and the backend is not changed here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

const SEEDS: [u64; 8] = [1, 2, 3, 0xC0FFEE, 0xDEAD_BEEF, 2026, 0xF00D, 0x1234_5678];
const STEPS: usize = 64;

/// Buffer ids registered with the engine, in the order the trace prints their
/// states. 1, 2: application (RGB565). 3: no address. 4: not in the table.
/// 5: XRGB8888 (the plane takes it only while the fake allows it). 10, 11:
/// the driver's safety buffers. 12: not the driver's.
const IDS: [u32; 8] = [1, 2, 3, 4, 5, 10, 11, 12];

const RUNNER: &str = r#"
#include "ce_twoplane.h"
#include "composition_engine.h"
#include <stdio.h>
#include <string.h>

enum { W = 64, H = 32, NBUF = 8 };
static const uint32_t ids[NBUF] = {1, 2, 3, 4, 5, 10, 11, 12};

static struct {
	uint32_t shadow_safety, active_safety, commits, latched;
	unsigned writes, lock_depth, start_after_write;
	bool deferred, xrgb_ok;
	ce_twoplane_vsync_cb_t cb;
	void *cb_user;
	uint32_t safety_mem[2][4], other_mem[4], app_mem[3][4];
} fake;

static void run_isr(void)
{
	if (fake.cb == NULL) return;
	if (fake.lock_depth > 0) { fake.deferred = true; return; }
	fake.cb(&fake, fake.cb_user);
}
static void frame_start(void)
{
	if (fake.latched != fake.commits) {
		fake.active_safety = fake.shadow_safety;
		fake.latched = fake.commits;
	}
	run_isr();
}
static void wrote(void)
{
	fake.commits++;
	fake.writes++;
	if (fake.start_after_write > 0 && --fake.start_after_write == 0) frame_start();
}
static bool hw_format_ok(uint32_t f)
{
	return f == CE_FORMAT_RGB565 || f == CE_FORMAT_ARGB8888 ||
	       (f == CE_FORMAT_XRGB8888 && fake.xrgb_ok);
}
static int hw_set_buf(const void *d, const struct ce_twoplane_buf *b) { (void)d; (void)b; wrote(); return 0; }
static int hw_enable(const void *d, bool e) { (void)d; (void)e; wrote(); return 0; }
static int hw_flip(const void *d) { (void)d; fake.shadow_safety ^= 1u; wrote(); return 0; }
static void *hw_safety_buffer(const void *d, uint32_t *w, uint32_t *h, uint32_t *s)
{
	(void)d; *w = W; *h = H; *s = W * 4;
	return fake.safety_mem[fake.shadow_safety ^ 1u];
}
static void *hw_safety_front(const void *d) { (void)d; return fake.safety_mem[fake.active_safety]; }
static int hw_premul(const void *d, bool on) { (void)d; (void)on; return 0; }
static uint32_t hw_commits(const void *d) { (void)d; return fake.commits; }
static uint32_t hw_latched(const void *d) { (void)d; return fake.latched; }
static void hw_set_cb(const void *d, ce_twoplane_vsync_cb_t cb, void *u)
{
	(void)d; fake.cb = cb; fake.cb_user = u;
}
static const struct ce_twoplane_hw hw = {
	hw_format_ok, hw_set_buf, hw_enable, hw_flip, hw_safety_buffer,
	hw_safety_front, hw_premul, hw_commits, hw_latched, hw_set_cb,
};

static struct ce_twoplane_record table[8];
static size_t table_len;
static unsigned int port_lock(void) { return fake.lock_depth++; }
static void port_unlock(unsigned key)
{
	fake.lock_depth = key;
	if (key == 0 && fake.deferred) { fake.deferred = false; run_isr(); }
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
	port_lock, port_unlock, port_wake, NULL, port_lookup, NULL, port_ready,
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
static void drain(void)
{
	ce_event e;
	while (ce_poll_event(engine, &e) == 1) {
		polled++;
		printf("E %c %llu\n", "PDR"[e.kind % 3],
		       e.kind == CE_EVENT_RELEASED ? (unsigned long long)e.buffer
						   : (unsigned long long)e.submission);
	}
}

int main(int argc, char **argv)
{
	FILE *f;
	char op;

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
		unsigned id[3] = {0, 0, 0}, k = 0, take = 0, x = 0;

		switch (op) {
		case 'P': { /* three layer buffers (0: none), frame start after write k, drain? */
			if (fscanf(f, "%u %u %u %u %u", &id[0], &id[1], &id[2], &k, &take) != 5) return 6;
			ce_layer l[3];
			size_t n = 0;
			for (int j = 0; j < 3; j++) {
				if (id[j] == 0) continue;
				bool safety = id[j] == 10 || id[j] == 11 || id[j] == 12;
				l[n++] = safety ? layer(id[j], 10 + j, CE_BLEND_PREMULTIPLIED)
						: layer(id[j], j, CE_BLEND_OPAQUE);
			}
			ce_frame fr = {.layers = l, .layer_count = n, .output = 0};
			fake.start_after_write = k;
			rc = ce_present(engine, &fr, &sub, NULL);
			fake.start_after_write = 0;
			break;
		}
		case 'V': frame_start(); take = 1; break;
		case 'E': take = 1; break;
		case 'F': if (fscanf(f, "%u", &x) != 1) return 6; fake.xrgb_ok = x != 0; break;
		default: return 7;
		}
		printf("S %c %d %llu", op, rc, (unsigned long long)(rc == 0 && op == 'P' ? sub : 0));
		snap();
		printf("\n");
		/* a refused commit polls nothing */
		if (rc == 0 && take) drain();
		printf("Z");
		snap();
		printf("\n");
	}
	frame_start();
	drain();
	printf("END");
	snap();
	printf("\n");
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
        let source = dir.join("release_props_runner.c");
        std::fs::write(&source, RUNNER).unwrap();
        let program = dir.join("release_props_runner");
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

/// SplitMix64: small and deterministic.
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

/// One script: `STEPS` steps, then a final frame start and drain.
fn script(seed: u64) -> String {
    let mut rng = Rng(seed);
    let mut out = String::new();
    for _ in 0..STEPS {
        let line = match rng.below(100) {
            0..=49 => {
                // a frame: application buffer (or none) and a safety buffer
                let app = if rng.below(8) == 0 {
                    0
                } else {
                    rng.pick(&[1, 2, 2, 5])
                };
                let (app, safety, third) = match rng.below(12) {
                    0 => (3, 10, 0),   // application buffer without an address
                    1 => (4, 10, 0),   // not in the integrator's table
                    2 => (app, 12, 0), // not the driver's safety buffer
                    3 => (app, 0, 0),  // no safety layer
                    4 => (1, 10, 2),   // a third layer: no plane for it
                    _ => (app, rng.pick(&[10, 11, 11]), 0),
                };
                let k = rng.pick(&[0, 0, 0, 1, 2, 3]);
                format!("P {app} {safety} {third} {k} {}", rng.below(2))
            }
            50..=69 => "V".to_string(),
            70..=89 => "E".to_string(),
            _ => format!("F {}", rng.below(2)),
        };
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("V\nE\n");
    out
}

const SUBMITTED: i64 = 2;
const SCANNING: i64 = 3;

/// The program's counters and buffer states after a step:
/// commits, latched, writes, polled events, then one state per buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snap(Vec<i64>);

impl Snap {
    fn parse(fields: &[&str]) -> Snap {
        Snap(fields.iter().map(|f| f.parse().unwrap()).collect())
    }
}

fn index(id: u64) -> usize {
    IDS.iter().position(|&b| u64::from(b) == id).unwrap()
}

fn check(seed: u64, script: &str, trace: &str) {
    let mut steps = script.lines();
    let mut last: Option<Snap> = None;
    let mut handed = [0i64; 8]; // buffers handed to the backend, per accepted commit
    let mut released = [0i64; 8];
    let mut owed = [0i64; 8]; // releases due once the frames leave the panel
    let mut frames: BTreeMap<u64, BTreeSet<usize>> = BTreeMap::new();
    let mut scanning: BTreeSet<usize> = BTreeSet::new();
    let mut presented = BTreeSet::new();
    let mut dropped = BTreeSet::new();
    let (mut accepted, mut refused) = (0usize, 0usize);
    let mut end = None;
    let mut step_refused = false;

    // Events of one step arrive in any order, so the release bound is checked
    // when the next step line (or the end) is read.
    let bounded = |released: &[i64; 8], owed: &[i64; 8], handed: &[i64; 8], at: &str| {
        for b in 0..8 {
            assert!(
                released[b] <= owed[b] && released[b] <= handed[b],
                "seed {seed:#x}: buffer {} released {} times, owed {}, handed {} ({at})",
                IDS[b],
                released[b],
                owed[b],
                handed[b]
            );
        }
    };

    for line in trace.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f[0] {
            // the state after a step's events were taken
            "B" | "Z" => last = Some(Snap::parse(&f[1..])),
            "S" => {
                bounded(&released, &owed, &handed, line);
                let cmd = steps.next().expect("more steps than script lines");
                let rc: i32 = f[2].parse().unwrap();
                let sub: u64 = f[3].parse().unwrap();
                let snap = Snap::parse(&f[4..]);
                let before = last.clone().unwrap();
                assert!(snap.0[1] <= snap.0[0], "seed {seed:#x}: latched > commits");
                step_refused = rc != 0;
                if rc == 0 && cmd.starts_with('P') {
                    accepted += 1;
                    let n: Vec<u64> = cmd[2..]
                        .split(' ')
                        .map(|x| x.parse().unwrap())
                        .take(3)
                        .collect();
                    let set: BTreeSet<usize> =
                        n.iter().filter(|&&b| b != 0).map(|&b| index(b)).collect();
                    for &b in &set {
                        handed[b] += 1;
                    }
                    frames.insert(sub, set);
                } else if rc != 0 {
                    // property 2: a refusal changes no counter, no buffer state
                    assert!(cmd.starts_with('P'), "seed {seed:#x}: `{cmd}` refused");
                    refused += 1;
                    assert_eq!(
                        before, snap,
                        "seed {seed:#x}: refused `{cmd}` changed state"
                    );
                }
            }
            "E" => {
                // property 2: a refused step queues and polls no event
                assert!(!step_refused, "seed {seed:#x}: event after a refusal");
                let id: u64 = f[2].parse().unwrap();
                match f[1] {
                    "P" => {
                        assert!(frames.contains_key(&id) && presented.insert(id));
                        assert!(!dropped.contains(&id), "seed {seed:#x}: {id} both");
                        let frame = &frames[&id];
                        for b in scanning.iter().filter(|b| !frame.contains(b)) {
                            owed[*b] += 1;
                        }
                        scanning = frame.clone();
                    }
                    "D" => {
                        assert!(frames.contains_key(&id) && dropped.insert(id));
                        assert!(!presented.contains(&id), "seed {seed:#x}: {id} both");
                        let next = frames.range(id + 1..).next().map(|(_, s)| s);
                        for b in &frames[&id] {
                            if !scanning.contains(b) && !next.is_some_and(|s| s.contains(b)) {
                                owed[*b] += 1;
                            }
                        }
                    }
                    _ => released[index(id)] += 1,
                }
            }
            "END" => {
                bounded(&released, &owed, &handed, line);
                end = Some(Snap::parse(&f[1..]));
            }
            other => panic!("seed {seed:#x}: unexpected trace line {other}"),
        }
    }
    let end = end.expect("no END");
    // property 1: after the last drain, every release owed was delivered once
    assert_eq!(released, owed, "seed {seed:#x}: releases at the end");
    assert_eq!(
        end.0[0], end.0[1],
        "seed {seed:#x}: last frame start latches all"
    );
    assert_eq!(presented.len() + dropped.len(), accepted, "seed {seed:#x}");
    // property 3: a buffer is in use iff the frame on the panel holds it (a
    // buffer shown by consecutive frames is released once, when it leaves the
    // panel, so `handed` is only an upper bound on releases)
    for (b, id) in IDS.iter().enumerate() {
        let state = end.0[4 + b];
        let on_panel = scanning.contains(&b);
        assert_eq!(
            matches!(state, SUBMITTED | SCANNING),
            on_panel,
            "seed {seed:#x}: buffer {id} state {state}"
        );
    }
    assert!(
        accepted >= 8 && refused >= 1 && !presented.is_empty(),
        "seed {seed:#x}: script too thin ({accepted} accepted, {refused} refused)"
    );
}

#[test]
fn releases_and_refusals_over_fixed_seeds() {
    let program = runner();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    for seed in SEEDS {
        let text = script(seed);
        let path = dir.join(format!("release_props_{seed:x}.script"));
        std::fs::write(&path, &text).unwrap();
        let output = run(Command::new(program).arg(&path), "the release runner");
        check(seed, &text, &String::from_utf8_lossy(&output.stdout));
    }
}

#[test]
fn the_script_is_deterministic() {
    assert_eq!(script(7), script(7));
    assert_ne!(script(7), script(8));
}
