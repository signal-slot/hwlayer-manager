//! Event reporting of the C ABI: what `ce_poll_event` hands a C caller after
//! `ce_present` and a backend latch, a replaced submission, a device loss and
//! a latch timeout. Null arguments and the ABI version belong to other test
//! files and are not repeated here.
//!
//! The backend is a table of `extern "C"` functions over a scripted context,
//! as a C program would write it. The assertions record what the code does
//! now; where the header leaves a point open, a comment says so.

// Reason: this file calls the `extern "C"` API, whose functions are `unsafe`,
// and builds the C-layout structures a C caller would pass.
#![allow(unsafe_code)]

use core::ffi::c_void;

use composition_ffi::*;

// ---- C-layout mirrors of the header's structures -------------------------

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
#[derive(Clone, Copy)]
struct Frame {
    layers: *const Layer,
    layer_count: usize,
    output: u32,
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
#[derive(Clone, Copy)]
struct Event {
    submission: u64,
    output: u32,
    buffer: u32,
    kind: u8,
}

type CheckFn = unsafe extern "C" fn(*mut c_void, *const Frame, *const c_void, usize) -> i32;
type CommitFn = unsafe extern "C" fn(*mut c_void, u64, *const Frame, *const c_void, usize) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
struct Ops {
    capabilities: Option<unsafe extern "C" fn(*mut c_void, *mut Capabilities)>,
    check: Option<CheckFn>,
    commit: Option<CommitFn>,
    poll_event: Option<unsafe extern "C" fn(*mut c_void, *mut BackendEvent) -> bool>,
}

// ---- header constants ------------------------------------------------------

const XRGB8888: u32 = 0x3432_5258; // 'X' 'R' '2' '4'
const EV_PRESENTED: u8 = 0;
const EV_DROPPED: u8 = 1;
const EV_RELEASED: u8 = 2;
const EV_DEVICE_LOST: u8 = 3;
const EV_LATCH_TIMEOUT: u8 = 5;
const BACKEND_LATCHED: u8 = 0;
const BACKEND_DEVICE_LOST: u8 = 1;
const READY: i32 = 1;
const SUBMITTED: i32 = 2;
const SCANNING: i32 = 3;

// ---- scripted backend ------------------------------------------------------

struct Ctx {
    replace_pending: bool,
    events: Vec<BackendEvent>,
    commits: Vec<u64>,
}

unsafe extern "C" fn be_capabilities(ctx: *mut c_void, out: *mut Capabilities) {
    // SAFETY: `ctx` is the `Ctx` given to init; `out` is the engine's
    // description area, valid for writes.
    let (ctx, out) = unsafe { (&*ctx.cast::<Ctx>(), &mut *out) };
    out.output_count = 1;
    out.outputs[0] = OutputCaps {
        id: 0,
        width: 1920,
        height: 1080,
        refresh_mhz: 60_000,
    };
    out.plane_count = 1;
    // SAFETY: every field of `PlaneCaps` is a plain integer.
    let mut plane: PlaneCaps = unsafe { core::mem::zeroed() };
    plane.id = 10;
    plane.outputs = 1;
    plane.format_count = 1;
    plane.formats[0] = Format {
        modifier: 0,
        format: XRGB8888,
    };
    plane.blend_modes = 1;
    plane.transforms = 1;
    out.planes[0] = plane;
    out.replace_pending = u8::from(ctx.replace_pending);
}

unsafe extern "C" fn be_check(
    _ctx: *mut c_void,
    _frame: *const Frame,
    _assignments: *const c_void,
    _count: usize,
) -> i32 {
    CE_OK
}

unsafe extern "C" fn be_commit(
    ctx: *mut c_void,
    submission: u64,
    _frame: *const Frame,
    _assignments: *const c_void,
    _count: usize,
) -> i32 {
    // SAFETY: `ctx` is the `Ctx` given to init.
    unsafe { &mut *ctx.cast::<Ctx>() }.commits.push(submission);
    CE_OK
}

unsafe extern "C" fn be_poll(ctx: *mut c_void, out: *mut BackendEvent) -> bool {
    // SAFETY: `ctx` is the `Ctx` given to init; `out` is valid for writes.
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

// ---- fixture -------------------------------------------------------------

#[repr(C, align(8))]
struct Storage([u8; CE_ENGINE_SIZE]);

struct Fixture {
    // Boxed: the engine keeps pointers into both.
    _storage: Box<Storage>,
    ctx: Box<Ctx>,
    engine: *mut ce_engine,
}

impl Fixture {
    fn new(replace_pending: bool) -> Self {
        let mut storage = Box::new(Storage([0; CE_ENGINE_SIZE]));
        let mut ctx = Box::new(Ctx {
            replace_pending,
            events: Vec::new(),
            commits: Vec::new(),
        });
        // SAFETY: the storage is large and aligned enough and outlives the
        // engine; `OPS` is a valid table whose functions accept `ctx`, which
        // is boxed and lives as long as the fixture.
        let engine = unsafe {
            ce_engine_init(
                storage.0.as_mut_ptr().cast(),
                CE_ENGINE_SIZE,
                (&OPS as *const Ops).cast(),
                (&raw mut *ctx).cast(),
            )
        };
        assert!(!engine.is_null());
        Fixture {
            _storage: storage,
            ctx,
            engine,
        }
    }

    /// A registered 64x64 buffer, complete and idle.
    fn ready(&mut self, id: u32) {
        let info = BufferInfo {
            modifier: 0,
            width: 64,
            height: 64,
            format: XRGB8888,
        };
        // SAFETY: the engine is live and `info` is a valid local.
        unsafe {
            let status = ce_register_buffer(self.engine, id, (&raw const info).cast());
            assert_eq!(status, CE_OK);
            assert_eq!(ce_finish_write(self.engine, id), CE_OK);
        }
        assert_eq!(self.state(id), READY);
    }

    fn state(&self, id: u32) -> i32 {
        // SAFETY: the engine is live.
        unsafe { ce_buffer_state(self.engine, id) }
    }

    fn time(&mut self, now: u64) {
        // SAFETY: the engine is live.
        assert_eq!(unsafe { ce_advance_time(self.engine, now) }, CE_OK);
    }

    /// Shows `buffer` full-size at the top left; returns the status and the
    /// submission identifier.
    fn present(&mut self, buffer: u32) -> (i32, u64) {
        let rect = Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        };
        let layer = Layer {
            buffer,
            source: rect,
            destination: rect,
            z_order: 0,
            alpha: 0xFFFF,
            blend: 0,
            transform: 0,
        };
        let frame = Frame {
            layers: &layer,
            layer_count: 1,
            output: 0,
        };
        let mut submission = u64::MAX;
        // SAFETY: the engine is live; the frame and its layer are valid for
        // the call; `submission` is a valid local; a null `info` is allowed.
        let status = unsafe {
            ce_present(
                self.engine,
                (&raw const frame).cast(),
                &mut submission,
                core::ptr::null_mut(),
            )
        };
        (status, submission)
    }

    /// The backend reports an event of `kind` for `submission`.
    fn backend(&mut self, kind: u8, submission: u64) {
        self.ctx.events.push(BackendEvent {
            submission,
            output: 0,
            kind,
        });
    }

    /// Drains the queue: (kind, output, submission, buffer) per event. The
    /// fields are poisoned first, so one the call leaves alone would show.
    fn drain(&mut self) -> Vec<(u8, u32, u64, u32)> {
        let mut seen = Vec::new();
        for _ in 0..64 {
            let mut event = Event {
                submission: 0xDEAD,
                output: 0xDEAD,
                buffer: 0xDEAD,
                kind: 0xDE,
            };
            // SAFETY: the engine is live; `event` is a valid local.
            let rc = unsafe { ce_poll_event(self.engine, (&raw mut event).cast()) };
            if rc != 1 {
                assert_eq!(rc, 0, "a poll on a healthy engine returns 1 or 0");
                return seen;
            }
            seen.push((event.kind, event.output, event.submission, event.buffer));
        }
        panic!("poll never ran dry");
    }
}

// ---- tests ---------------------------------------------------------------

/// Present, commit, latch: the caller sees one Presented with the
/// submission it got from `ce_present`, then an empty queue.
#[test]
fn latch_reports_presented_once() {
    let mut f = Fixture::new(false);
    f.ready(1);
    assert!(f.drain().is_empty(), "nothing is reported before a present");
    let (status, sub) = f.present(1);
    assert_eq!(status, CE_OK);
    assert_eq!(f.ctx.commits, [sub]);
    assert_eq!(f.state(1), SUBMITTED);
    assert!(f.drain().is_empty(), "no latch yet, no event");

    f.backend(BACKEND_LATCHED, sub);
    // Presented carries output and submission; buffer is zeroed by the call.
    assert_eq!(f.drain(), [(EV_PRESENTED, 0, sub, 0)]);
    assert_eq!(f.state(1), SCANNING);
    assert!(f.drain().is_empty());
}

/// The latch of a second frame takes the first off screen: Presented for the
/// new one, then Released for the old buffer, in that order.
#[test]
fn second_latch_releases_the_first_buffer() {
    let mut f = Fixture::new(false);
    f.ready(1);
    f.ready(2);
    let (_, a) = f.present(1);
    f.backend(BACKEND_LATCHED, a);
    assert_eq!(f.drain(), [(EV_PRESENTED, 0, a, 0)]);

    let (status, b) = f.present(2);
    assert_eq!(status, CE_OK);
    assert_ne!(a, b, "submission identifiers are distinct");
    f.backend(BACKEND_LATCHED, b);
    assert_eq!(f.drain(), [(EV_PRESENTED, 0, b, 0), (EV_RELEASED, 0, 0, 1)]);
    assert_eq!(f.state(1), READY);
    assert_eq!(f.state(2), SCANNING);
}

/// A backend that cannot replace a pending commit answers CE_ERR_BUSY and the
/// pending submission is untouched; one that can replaces it, and the caller
/// sees Dropped and Released for the replaced one.
#[test]
fn replacing_a_pending_submission() {
    let mut f = Fixture::new(false);
    f.ready(1);
    f.ready(2);
    let (_, a) = f.present(1);
    assert_eq!(f.present(2).0, CE_ERR_BUSY);
    assert_eq!(f.ctx.commits, [a], "the refused frame never reached commit");
    assert_eq!(f.state(2), READY);
    assert!(f.drain().is_empty(), "a refusal is a status, not an event");

    let mut f = Fixture::new(true);
    f.ready(1);
    f.ready(2);
    let (_, a) = f.present(1);
    let (status, b) = f.present(2);
    assert_eq!(status, CE_OK);
    assert_eq!(f.ctx.commits, [a, b]);
    // Header: a replaced commit is never reported as latched. The order
    // Dropped, Released is what the code does now.
    assert_eq!(f.drain(), [(EV_DROPPED, 0, a, 0), (EV_RELEASED, 0, 0, 1)]);
    f.backend(BACKEND_LATCHED, b);
    assert_eq!(f.drain(), [(EV_PRESENTED, 0, b, 0)]);
}

/// A device loss is reported as DeviceLost.
#[test]
fn device_loss_is_reported() {
    let mut f = Fixture::new(false);
    f.ready(1);
    let (_, a) = f.present(1);
    f.backend(BACKEND_DEVICE_LOST, 0);
    let events = f.drain();
    // Unclear from the header whether the pending frame also yields Dropped
    // and Released; only what is certain is asserted: DeviceLost is reported
    // and a lost frame is never reported as presented.
    assert!(events.contains(&(EV_DEVICE_LOST, 0, 0, 0)), "{events:?}");
    assert!(
        events.iter().all(|e| !(e.0 == EV_PRESENTED && e.2 == a)),
        "{events:?}"
    );
}

/// Without a latch within the timeout, one LatchTimeout is reported and the
/// submission stays pending.
#[test]
fn latch_timeout_is_reported_once() {
    let mut f = Fixture::new(false);
    f.ready(1);
    f.ready(2);
    // SAFETY: the engine is live.
    assert_eq!(unsafe { ce_set_latch_timeout(f.engine, 100) }, CE_OK);
    f.time(1000);
    let (_, a) = f.present(1);
    f.time(1050);
    assert!(f.drain().is_empty(), "not yet");
    f.time(1100);
    assert_eq!(f.drain(), [(EV_LATCH_TIMEOUT, 0, a, 0)]);
    f.time(5000);
    assert!(f.drain().is_empty(), "reported once");
    assert_eq!(f.state(1), SUBMITTED, "the submission stays pending");
    assert_eq!(f.present(2).0, CE_ERR_BUSY);
}
