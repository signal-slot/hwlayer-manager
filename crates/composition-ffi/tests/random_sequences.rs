//! Seeded random call sequences through the C ABI (M3-05 hardening).
//!
//! Each seed drives a long random sequence of the `ce_*` calls: register,
//! unregister, begin/finish write, present, poll, time, and "destroy" (the
//! engine has no destroy call: its storage is the caller's, so a destroyed
//! handle is a null handle, and re-creating is `ce_engine_init` on the same
//! storage). Invalid orders and invalid ids are part of the mix. After each
//! call the test checks that the status is a documented one (a panic would
//! abort the process), and that a small model of the buffer table agrees
//! with `ce_buffer_state`.

// Reason: this file calls the `extern "C"` API, whose functions are `unsafe`.
#![allow(unsafe_code)]

use core::ffi::c_void;
use core::ptr::{null, null_mut};

use composition_ffi::*;

// ---- C-layout mirrors of the header's structures (see malformed.rs) --------

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BufferInfo {
    modifier: u64,
    width: u32,
    height: u32,
    format: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Layer {
    buffer: u32,
    source: Rect,
    destination: Rect,
    z_order: i32,
    alpha: u16,
    blend: u8,
    transform: u8,
}

#[repr(C)]
struct Frame {
    layers: *const Layer,
    layer_count: usize,
    output: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Assignment {
    layer: u32,
    plane: u32,
    plane_index: u32,
    zpos: i32,
    source: Rect,
    destination: Rect,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Format {
    modifier: u64,
    format: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PlaneCaps {
    formats: [Format; 32],
    format_count: u32,
    id: u32,
    outputs: u32,
    max_upscale: u32,
    max_downscale: u32,
    max_source_width: u32,
    max_source_height: u32,
    zpos_min: i32,
    zpos_max: i32,
    kind: u8,
    can_scale: u8,
    plane_alpha: u8,
    blend_modes: u8,
    transforms: u8,
    zpos_mutable: u8,
    full_output_only: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OutputCaps {
    id: u32,
    width: u32,
    height: u32,
    refresh_mhz: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ResourceGroup {
    planes: u32,
    consumers: u8,
    capacity: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Capabilities {
    planes: [PlaneCaps; 16],
    outputs: [OutputCaps; 4],
    groups: [ResourceGroup; 8],
    plane_count: u32,
    output_count: u32,
    group_count: u32,
    replace_pending: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct BackendEvent {
    submission: u64,
    output: u32,
    kind: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Event {
    submission: u64,
    output: u32,
    buffer: u32,
    kind: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ErrorInfo {
    status: i32,
    layer: u32,
    reasons: u32,
    buffer: u32,
    platform: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Ops {
    capabilities: Option<unsafe extern "C" fn(*mut c_void, *mut Capabilities)>,
    check: Option<unsafe extern "C" fn(*mut c_void, *const Frame, *const Assignment, usize) -> i32>,
    commit: Option<
        unsafe extern "C" fn(*mut c_void, u64, *const Frame, *const Assignment, usize) -> i32,
    >,
    poll_event: Option<unsafe extern "C" fn(*mut c_void, *mut BackendEvent) -> bool>,
}

const XRGB8888: u32 = 0x3432_5258; // "XR24"
const ARGB8888: u32 = 0x3432_5241; // "AR24"
const NV12: u32 = 0x3231_564E; // "NV12"

/// Every status the header documents for a call that can fail.
const DOCUMENTED: &[i32] = &[
    CE_OK,
    CE_ERR_INVALID_ARGUMENT,
    CE_ERR_UNKNOWN_OUTPUT,
    CE_ERR_TOO_MANY_LAYERS,
    CE_ERR_UNKNOWN_BUFFER,
    CE_ERR_INVALID_SOURCE,
    CE_ERR_INVALID_DESTINATION,
    CE_ERR_NO_PLANE,
    CE_ERR_BACKEND_REJECTED,
    CE_ERR_SEARCH_LIMIT,
    CE_ERR_CANNOT_CLIP,
    CE_ERR_DUPLICATE_BUFFER,
    CE_ERR_BUFFER_TABLE_FULL,
    CE_ERR_BUFFER_IN_USE,
    CE_ERR_BUFFER_NOT_READY,
    CE_ERR_BUFFER_NOT_WRITABLE,
    CE_ERR_BUSY,
    CE_ERR_TOO_MANY_OUTPUTS,
    CE_ERR_TOO_MANY_BUFFERS,
    CE_ERR_EVENT_QUEUE_FULL,
    CE_ERR_UNEXPECTED_LATCH,
    CE_ERR_BACKEND_BUSY,
    CE_ERR_DEVICE_LOST,
    CE_ERR_BACKEND_PLATFORM,
    CE_ERR_BACKEND_UNKNOWN_BUFFER,
];

// ---- PRNG ---------------------------------------------------------------

/// SplitMix64: small, deterministic, good enough for choosing calls.
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

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

// ---- backend --------------------------------------------------------------

/// What the backend does next, chosen at random by the test.
struct Ctx {
    check_rc: i32,
    commit_rc: i32,
    /// Backend events not yet polled.
    events: Vec<BackendEvent>,
}

fn plane(id: u32, kind: u8, formats: &[u32]) -> PlaneCaps {
    // SAFETY: every field of `PlaneCaps` is a plain integer.
    let mut plane: PlaneCaps = unsafe { core::mem::zeroed() };
    plane.id = id;
    plane.kind = kind;
    plane.outputs = 1;
    plane.format_count = formats.len() as u32;
    for (slot, format) in plane.formats.iter_mut().zip(formats) {
        *slot = Format {
            modifier: 0,
            format: *format,
        };
    }
    plane.plane_alpha = 1;
    plane.blend_modes = 0b111;
    plane.transforms = 1;
    plane
}

unsafe extern "C" fn be_capabilities(_ctx: *mut c_void, out: *mut Capabilities) {
    // SAFETY: `out` is the engine's zeroed description area.
    let out = unsafe { &mut *out };
    out.output_count = 1;
    out.outputs[0] = OutputCaps {
        id: 0,
        width: 256,
        height: 256,
        refresh_mhz: 60_000,
    };
    out.plane_count = 2;
    out.planes[0] = plane(10, 0, &[XRGB8888, ARGB8888]);
    let mut overlay = plane(11, 1, &[ARGB8888, XRGB8888]);
    overlay.zpos_mutable = 1;
    overlay.zpos_min = 1;
    overlay.zpos_max = 3;
    out.planes[1] = overlay;
}

unsafe extern "C" fn be_check(
    ctx: *mut c_void,
    _frame: *const Frame,
    _assignments: *const Assignment,
    _count: usize,
) -> i32 {
    // SAFETY: `ctx` is the `Ctx` given to init, alive for the engine.
    unsafe { (*ctx.cast::<Ctx>()).check_rc }
}

unsafe extern "C" fn be_commit(
    ctx: *mut c_void,
    _submission: u64,
    _frame: *const Frame,
    _assignments: *const Assignment,
    _count: usize,
) -> i32 {
    // SAFETY: as in `be_check`.
    unsafe { (*ctx.cast::<Ctx>()).commit_rc }
}

unsafe extern "C" fn be_poll(ctx: *mut c_void, out: *mut BackendEvent) -> bool {
    // SAFETY: as in `be_check`; `out` is valid for writes.
    let (ctx, out) = unsafe { (&mut *ctx.cast::<Ctx>(), &mut *out) };
    if ctx.events.is_empty() {
        return false;
    }
    *out = ctx.events.remove(0);
    true
}

const OPS: Ops = Ops {
    capabilities: Some(be_capabilities),
    check: Some(be_check),
    commit: Some(be_commit),
    poll_event: Some(be_poll),
};

// ---- the run --------------------------------------------------------------

#[repr(C, align(8))]
struct Storage([u8; CE_ENGINE_SIZE]);

const IDS: u32 = 8;
/// Documented values of `ce_buffer_state`, from -1 (unknown) to 3.
const STATE_RANGE: core::ops::RangeInclusive<i32> = -1..=3;

fn documented(status: i32, what: &str) {
    assert!(
        DOCUMENTED.contains(&status),
        "{what}: undocumented status {status}"
    );
}

fn rand_rect(rng: &mut Rng, max: u32) -> Rect {
    Rect {
        x: rng.below(40) as i32 - 8,
        y: rng.below(40) as i32 - 8,
        width: rng.below(u64::from(max) + 8) as u32,
        height: rng.below(u64::from(max) + 8) as u32,
    }
}

/// (Re)creates an engine in `storage`, starting from a dirty state.
fn create(storage: &mut Storage, ctx: &mut Ctx) -> *mut ce_engine {
    storage.0.fill(0xA5);
    ctx.events.clear();
    // `ops` is copied by the engine.
    let ops = OPS;
    // SAFETY: the storage is large enough and aligned (`repr(align(8))`);
    // `ctx` outlives every use of the engine, as the caller keeps it boxed.
    let engine = unsafe {
        ce_engine_init(
            storage.0.as_mut_ptr().cast(),
            CE_ENGINE_SIZE,
            (&raw const ops).cast(),
            (ctx as *mut Ctx).cast(),
        )
    };
    assert!(!engine.is_null());
    engine
}

fn run(seed: u64, steps: usize) {
    let mut rng = Rng(seed);
    let mut storage = Box::new(Storage([0; CE_ENGINE_SIZE]));
    let mut ctx = Box::new(Ctx {
        check_rc: CE_OK,
        commit_rc: CE_OK,
        events: Vec::new(),
    });
    let mut engine = create(&mut storage, &mut ctx);

    // Model: registered ids, and the last submission number seen.
    let mut registered = [false; IDS as usize + 2];
    let mut last_submission: Option<u64> = None;
    let mut now = 0u64;
    // Coverage: the sequence must reach the interesting paths.
    let (mut presented, mut events) = (0u32, 0u32);

    for step in 0..steps {
        let id = rng.below(u64::from(IDS) + 2) as u32; // two ids never registered
        let known = registered[id as usize] && !engine.is_null();
        let tag = format!("seed {seed} step {step}");
        // SAFETY (all calls below): `engine` is either a live engine in
        // `storage` or null; every out pointer is a valid local or null,
        // exactly the cases the header allows.
        match rng.below(14) {
            0 | 1 => {
                let info = BufferInfo {
                    modifier: 0,
                    width: 8 + rng.below(56) as u32,
                    height: 8 + rng.below(56) as u32,
                    format: [XRGB8888, ARGB8888, NV12, 0][rng.below(4) as usize],
                };
                let rc = unsafe { ce_register_buffer(engine, id, (&raw const info).cast()) };
                documented(rc, &tag);
                if engine.is_null() {
                    assert_eq!(rc, CE_ERR_INVALID_ARGUMENT, "{tag}");
                } else if rc == CE_OK {
                    assert!(!known, "{tag}: registered twice");
                    registered[id as usize] = true;
                } else if known {
                    assert_eq!(rc, CE_ERR_DUPLICATE_BUFFER, "{tag}");
                }
            }
            2 => {
                let before = unsafe { ce_buffer_state(engine, id) };
                let rc = unsafe { ce_unregister_buffer(engine, id) };
                documented(rc, &tag);
                if rc == CE_OK {
                    // Only idle buffers may go.
                    assert!(matches!(before, 0 | 1), "{tag}: removed state {before}");
                    registered[id as usize] = false;
                } else if !engine.is_null() && !known {
                    assert_eq!(rc, CE_ERR_UNKNOWN_BUFFER, "{tag}");
                }
            }
            3 => {
                let rc = unsafe { ce_begin_write(engine, id) };
                documented(rc, &tag);
                if rc == CE_OK {
                    assert_eq!(unsafe { ce_buffer_state(engine, id) }, 0, "{tag}");
                }
            }
            4 => {
                let rc = unsafe { ce_finish_write(engine, id) };
                documented(rc, &tag);
                if rc == CE_OK {
                    assert_eq!(unsafe { ce_buffer_state(engine, id) }, 1, "{tag}");
                }
            }
            5..=7 => {
                let count = rng.below(4) as usize;
                let mut layers = [Layer {
                    buffer: 0,
                    source: Rect::default(),
                    destination: Rect::default(),
                    z_order: 0,
                    alpha: 0xFFFF,
                    blend: 0,
                    transform: 0,
                }; 4];
                for (i, layer) in layers.iter_mut().enumerate().take(count) {
                    layer.buffer = rng.below(u64::from(IDS) + 2) as u32;
                    layer.source = if rng.chance(70) {
                        Rect {
                            x: 0,
                            y: 0,
                            width: 8,
                            height: 8,
                        }
                    } else {
                        rand_rect(&mut rng, 64)
                    };
                    layer.destination = rand_rect(&mut rng, 256);
                    layer.z_order = i as i32 + rng.below(3) as i32 - 1;
                    layer.blend = rng.below(4) as u8; // 3 is not defined
                    layer.transform = rng.below(7) as u8; // 6 is not defined
                }
                let frame = Frame {
                    layers: layers.as_ptr(),
                    layer_count: count,
                    output: if rng.chance(90) { 0 } else { 3 },
                };
                ctx.check_rc =
                    [CE_OK, CE_OK, CE_OK, CE_ERR_BACKEND_REJECTED][rng.below(4) as usize];
                ctx.commit_rc =
                    [CE_OK, CE_OK, CE_OK, CE_OK, CE_ERR_BACKEND_BUSY, -77][rng.below(6) as usize];
                let mut info = ErrorInfo::default();
                let mut submission = 0u64;
                let rc = unsafe {
                    ce_present(
                        engine,
                        (&raw const frame).cast(),
                        &mut submission,
                        (&raw mut info).cast(),
                    )
                };
                documented(rc, &tag);
                if engine.is_null() {
                    assert_eq!(rc, CE_ERR_INVALID_ARGUMENT, "{tag}");
                } else if rc == CE_OK {
                    assert!(
                        last_submission.is_none_or(|last| submission > last),
                        "{tag}: submission numbers must increase"
                    );
                    last_submission = Some(submission);
                    presented += 1;
                    for layer in &layers[..count] {
                        let state = unsafe { ce_buffer_state(engine, layer.buffer) };
                        assert!(
                            matches!(state, 2 | 3),
                            "{tag}: submitted buffer state {state}"
                        );
                    }
                    // The hardware latches it, sometimes.
                    if rng.chance(70) {
                        ctx.events.push(BackendEvent {
                            submission,
                            output: 0,
                            kind: 0,
                        });
                    }
                }
                if rc != CE_OK {
                    assert_eq!(info.status, rc, "{tag}: info.status repeats the return");
                }
            }
            8 | 9 => {
                // Sometimes the backend reports a latch of something that
                // was never committed (or was already latched).
                if rng.chance(10) {
                    ctx.events.push(BackendEvent {
                        submission: rng.below(50),
                        output: 0,
                        kind: 0,
                    });
                }
                let mut event = Event::default();
                let rc = unsafe { ce_poll_event(engine, (&raw mut event).cast()) };
                if engine.is_null() {
                    assert_eq!(rc, CE_ERR_INVALID_ARGUMENT, "{tag}");
                } else {
                    assert!(
                        rc == 0 || rc == 1 || DOCUMENTED.contains(&rc),
                        "{tag}: poll {rc}"
                    );
                    if rc == 1 {
                        events += 1;
                        assert!(event.kind <= 5, "{tag}: event kind {}", event.kind);
                    }
                }
            }
            10 => {
                now += rng.below(20);
                documented(unsafe { ce_advance_time(engine, now) }, &tag);
            }
            11 => {
                documented(
                    unsafe { ce_set_latch_timeout(engine, rng.below(3) * 10) },
                    &tag,
                );
            }
            12 => {
                // Destroy: the handle is dropped; every call must now be
                // rejected without touching the old storage.
                engine = null_mut();
                registered = [false; IDS as usize + 2];
                let mut event = Event::default();
                assert_eq!(
                    unsafe { ce_poll_event(engine, (&raw mut event).cast()) },
                    CE_ERR_INVALID_ARGUMENT
                );
                assert_eq!(
                    unsafe { ce_begin_write(engine, 0) },
                    CE_ERR_INVALID_ARGUMENT
                );
                assert_eq!(unsafe { ce_buffer_state(engine, 0) }, -1);
                assert_eq!(
                    unsafe { ce_register_buffer(engine, 0, null()) },
                    CE_ERR_INVALID_ARGUMENT
                );
            }
            _ => {
                // Create again (also over a live engine: it starts fresh).
                engine = create(&mut storage, &mut ctx);
                registered = [false; IDS as usize + 2];
                last_submission = None;
                now = 0;
            }
        }

        // The table agrees with the model after every call.
        for probe in 0..IDS + 2 {
            let state = unsafe { ce_buffer_state(engine, probe) };
            assert!(STATE_RANGE.contains(&state), "{tag}: state {state}");
            let model = registered[probe as usize] && !engine.is_null();
            assert_eq!(state >= 0, model, "{tag}: buffer {probe}");
        }
    }
    assert!(
        presented > 0 && events > 0,
        "seed {seed}: {presented} presents, {events} events"
    );
}

#[test]
fn random_sequences_seed_1() {
    run(1, 4000);
}

#[test]
fn random_sequences_seed_2() {
    run(0xC0FF_EE00_1234_5678, 4000);
}

#[test]
fn random_sequences_seed_3() {
    run(0xDEAD_BEEF, 4000);
}

#[test]
fn random_sequences_seed_4() {
    run(42, 4000);
}
