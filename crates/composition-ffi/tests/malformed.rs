//! Malformed-input tests of the C ABI (SoW §10): the `ce_*` functions are
//! called from Rust with null pointers, out-of-range values, extreme
//! rectangles, calls in the wrong order and random descriptors. Every call
//! must return the documented status, never panic (a panic across `extern "C"`
//! aborts the process) and never read or write outside what it was given.
//!
//! The backend is a table of `extern "C"` functions over a scripted context,
//! as a C program would write it; the test backend of the crate's own unit
//! tests is not reachable from here.
//!
//! Cases that show a bug are `#[ignore = "bug: ..."]`, written as the
//! behaviour the header documents, so they pass once the bug is fixed.

// Reason: this file calls the `extern "C"` API, whose functions are `unsafe`,
// and builds the C-layout structures a C caller would pass.
#![allow(unsafe_code)]

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr::{null, null_mut};

use composition_ffi::*;

// ---- C-layout mirrors of the header's structures -------------------------
// The fields of the crate's own structures are private, so a C caller's view
// is rebuilt here; `abi_check_accepts_the_mirror_sizes` ties the two together.

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
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

// ---- header constants ------------------------------------------------------

const MAX_LAYERS: usize = 16;
const MAX_BUFFERS: u32 = 64;

const fn fourcc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    a as u32 | (b as u32) << 8 | (c as u32) << 16 | (d as u32) << 24
}
const XRGB8888: u32 = fourcc(b'X', b'R', b'2', b'4');
const ARGB8888: u32 = fourcc(b'A', b'R', b'2', b'4');
const NV12: u32 = fourcc(b'N', b'V', b'1', b'2');
const ALPHA_OPAQUE: u16 = 0xFFFF;

const EV_PRESENTED: u8 = 0;
const EV_DEVICE_LOST: u8 = 1;

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

// ---- scripted backend ------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum CapsMode {
    /// One 1920x1080 output, a primary plane and a scaling overlay plane.
    Normal,
    /// Counts far above the array sizes, unknown kinds, all-ones bytes.
    Garbage,
    /// Nothing at all.
    Empty,
}

struct Ctx {
    caps: CapsMode,
    check_rc: i32,
    commit_rc: i32,
    events: [BackendEvent; 32],
    queued: usize,
    next: usize,
    /// Submissions seen by commit, in order.
    commits: [u64; 32],
    commit_count: usize,
    check_count: usize,
}

impl Ctx {
    fn new() -> Self {
        Ctx {
            caps: CapsMode::Normal,
            check_rc: CE_OK,
            commit_rc: CE_OK,
            events: [BackendEvent::default(); 32],
            queued: 0,
            next: 0,
            commits: [0; 32],
            commit_count: 0,
            check_count: 0,
        }
    }

    fn push(&mut self, kind: u8, output: u32, submission: u64) {
        if self.queued == self.events.len() {
            self.queued = 0;
            self.next = 0;
        }
        self.events[self.queued] = BackendEvent {
            submission,
            output,
            kind,
        };
        self.queued += 1;
    }

    fn last_commit(&self) -> Option<u64> {
        (self.commit_count > 0).then(|| self.commits[(self.commit_count - 1) % 32])
    }
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

unsafe extern "C" fn be_capabilities(ctx: *mut c_void, out: *mut Capabilities) {
    // SAFETY: `ctx` is the `Ctx` given to `init`; `out` is the engine's
    // description area, valid for writes.
    let (ctx, out) = unsafe { (&mut *ctx.cast::<Ctx>(), &mut *out) };
    match ctx.caps {
        CapsMode::Empty => {}
        CapsMode::Normal => {
            out.output_count = 1;
            out.outputs[0] = OutputCaps {
                id: 0,
                width: 1920,
                height: 1080,
                refresh_mhz: 60_000,
            };
            out.plane_count = 2;
            out.planes[0] = plane(10, 0, &[XRGB8888, ARGB8888]);
            let mut overlay = plane(11, 1, &[ARGB8888, XRGB8888]);
            overlay.can_scale = 1;
            overlay.max_upscale = 4 << 16;
            overlay.max_downscale = 4 << 16;
            overlay.zpos_mutable = 1;
            overlay.zpos_min = 1;
            overlay.zpos_max = 3;
            out.planes[1] = overlay;
        }
        CapsMode::Garbage => {
            // SAFETY: every field of `Capabilities` is a plain integer.
            unsafe { core::ptr::write_bytes(out, 0xFF, 1) };
            out.replace_pending = 7;
        }
    }
}

unsafe extern "C" fn be_check(
    ctx: *mut c_void,
    frame: *const Frame,
    assignments: *const Assignment,
    count: usize,
) -> i32 {
    // SAFETY: see `be_capabilities`; the engine passes `count` assignments
    // and a valid frame.
    let ctx = unsafe { &mut *ctx.cast::<Ctx>() };
    ctx.check_count += 1;
    assert!(!frame.is_null() && count <= MAX_LAYERS);
    assert!(count == 0 || !assignments.is_null());
    ctx.check_rc
}

unsafe extern "C" fn be_commit(
    ctx: *mut c_void,
    submission: u64,
    frame: *const Frame,
    assignments: *const Assignment,
    count: usize,
) -> i32 {
    // SAFETY: as in `be_check`.
    let ctx = unsafe { &mut *ctx.cast::<Ctx>() };
    assert!(!frame.is_null() && count <= MAX_LAYERS);
    assert!(count == 0 || !assignments.is_null());
    if ctx.commit_rc != CE_OK {
        return ctx.commit_rc;
    }
    ctx.commits[ctx.commit_count % 32] = submission;
    ctx.commit_count += 1;
    CE_OK
}

unsafe extern "C" fn be_poll(ctx: *mut c_void, out: *mut BackendEvent) -> bool {
    // SAFETY: as in `be_capabilities`.
    let (ctx, out) = unsafe { (&mut *ctx.cast::<Ctx>(), &mut *out) };
    if ctx.next < ctx.queued {
        *out = ctx.events[ctx.next];
        ctx.next += 1;
        true
    } else {
        false
    }
}

const OPS: Ops = Ops {
    capabilities: Some(be_capabilities),
    check: Some(be_check),
    commit: Some(be_commit),
    poll_event: Some(be_poll),
};

// ---- fixture -------------------------------------------------------------

#[repr(C, align(8))]
struct Storage([u8; CE_ENGINE_SIZE + 64]);

struct Fixture {
    // Boxed: the engine keeps pointers into both.
    _storage: Box<Storage>,
    ctx: Box<Ctx>,
    engine: *mut ce_engine,
}

fn init(storage: *mut c_void, size: usize, ops: *const Ops, ctx: *mut c_void) -> *mut ce_engine {
    // SAFETY: the callers pass storage valid for `size` bytes (or null, which
    // is the case under test) and a valid ops table or null.
    unsafe { ce_engine_init(storage, size, ops.cast(), ctx) }
}

impl Fixture {
    fn new(ctx: Ctx) -> Self {
        Self::with_ops(ctx, &OPS)
    }

    fn with_ops(ctx: Ctx, ops: &Ops) -> Self {
        let mut storage = Box::new(Storage([0xA5; CE_ENGINE_SIZE + 64]));
        let mut ctx = Box::new(ctx);
        let engine = init(
            storage.0.as_mut_ptr().cast(),
            CE_ENGINE_SIZE,
            ops,
            (&raw mut *ctx).cast(),
        );
        assert!(!engine.is_null());
        Fixture {
            _storage: storage,
            ctx,
            engine,
        }
    }

    fn reg(&mut self, id: u32, w: u32, h: u32, format: u32) -> i32 {
        let info = BufferInfo {
            modifier: 0,
            width: w,
            height: h,
            format,
        };
        // SAFETY: the engine is live and `info` is a valid local.
        unsafe { ce_register_buffer(self.engine, id, (&raw const info).cast()) }
    }

    fn unreg(&mut self, id: u32) -> i32 {
        // SAFETY: the engine is live.
        unsafe { ce_unregister_buffer(self.engine, id) }
    }

    fn begin(&mut self, id: u32) -> i32 {
        // SAFETY: the engine is live.
        unsafe { ce_begin_write(self.engine, id) }
    }

    fn finish(&mut self, id: u32) -> i32 {
        // SAFETY: the engine is live.
        unsafe { ce_finish_write(self.engine, id) }
    }

    fn state(&self, id: u32) -> i32 {
        // SAFETY: the engine is live.
        unsafe { ce_buffer_state(self.engine, id) }
    }

    /// A registered 64x64 XRGB buffer that is ready to show.
    fn ready(&mut self, id: u32) {
        assert_eq!(self.reg(id, 64, 64, XRGB8888), CE_OK);
        assert_eq!(self.finish(id), CE_OK);
        assert_eq!(self.state(id), 1);
    }

    fn present(&mut self, frame: &Frame) -> (i32, ErrorInfo, u64) {
        let mut info = ErrorInfo {
            status: 12345,
            layer: 0xDEAD,
            reasons: 0xDEAD,
            buffer: 0xDEAD,
            platform: 0x1234,
        };
        let mut submission = u64::MAX;
        // SAFETY: the engine is live; `frame` and its layers are valid for
        // the call; the out pointers are valid locals.
        let status = unsafe {
            ce_present(
                self.engine,
                (frame as *const Frame).cast(),
                &mut submission,
                (&raw mut info).cast(),
            )
        };
        assert_eq!(info.status, status, "info.status must repeat the return");
        (status, info, submission)
    }

    fn present_layers(&mut self, layers: &[Layer]) -> (i32, ErrorInfo, u64) {
        let frame = Frame {
            layers: layers.as_ptr(),
            layer_count: layers.len(),
            output: 0,
        };
        self.present(&frame)
    }

    fn poll(&mut self) -> (i32, Event) {
        let mut event = Event {
            submission: 0xDEAD,
            output: 0xDEAD,
            buffer: 0xDEAD,
            kind: 0xDE,
        };
        // SAFETY: the engine is live; `event` is a valid local.
        let rc = unsafe { ce_poll_event(self.engine, (&raw mut event).cast()) };
        (rc, event)
    }

    /// Polls until the queue is empty or reports a failure; returns the
    /// event kinds seen.
    fn drain(&mut self) -> Vec<u8> {
        let mut kinds = Vec::new();
        for _ in 0..256 {
            let (rc, event) = self.poll();
            if rc != 1 {
                return kinds;
            }
            kinds.push(event.kind);
        }
        panic!("poll never ran dry");
    }
}

fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn layer(buffer: u32) -> Layer {
    Layer {
        buffer,
        source: rect(0, 0, 64, 64),
        destination: rect(0, 0, 64, 64),
        z_order: 0,
        alpha: ALPHA_OPAQUE,
        blend: 0,
        transform: 0,
    }
}

fn blank() -> Frame {
    Frame {
        layers: null(),
        layer_count: 0,
        output: 0,
    }
}

// ---- ce_abi_check ----------------------------------------------------------

fn abi(sizes: [usize; 7]) -> bool {
    ce_abi_check(
        sizes[0], sizes[1], sizes[2], sizes[3], sizes[4], sizes[5], sizes[6],
    )
}

fn good_sizes() -> [usize; 7] {
    [
        size_of::<Capabilities>(),
        size_of::<Layer>(),
        size_of::<Frame>(),
        size_of::<Event>(),
        size_of::<Ops>(),
        size_of::<Assignment>(),
        CE_ENGINE_SIZE,
    ]
}

#[test]
fn abi_check_accepts_the_mirror_sizes() {
    // Also proves the mirrors above have the layout the library has.
    assert!(abi(good_sizes()));
}

#[test]
fn abi_check_rejects_every_wrong_size() {
    for slot in 0..7 {
        let right = good_sizes()[slot];
        for wrong in [
            0,
            1,
            right.wrapping_sub(1),
            right + 1,
            right * 2,
            usize::MAX,
        ] {
            if wrong == right {
                continue;
            }
            let mut sizes = good_sizes();
            sizes[slot] = wrong;
            assert!(!abi(sizes), "argument {slot} = {wrong} was accepted");
        }
    }
    assert!(!abi([0; 7]));
    assert!(!abi([usize::MAX; 7]));
}

// ---- ce_engine_init --------------------------------------------------------

#[test]
fn init_rejects_null_storage_and_null_ops() {
    let mut storage = Box::new(Storage([0; CE_ENGINE_SIZE + 64]));
    let mut ctx = Ctx::new();
    let ctx_ptr = (&raw mut ctx).cast::<c_void>();
    assert!(init(null_mut(), CE_ENGINE_SIZE, &OPS, ctx_ptr).is_null());
    assert!(init(
        storage.0.as_mut_ptr().cast(),
        CE_ENGINE_SIZE,
        null(),
        ctx_ptr
    )
    .is_null());
    assert!(init(null_mut(), 0, null(), null_mut()).is_null());
}

#[test]
fn init_rejects_small_storage() {
    let mut storage = Box::new(Storage([0; CE_ENGINE_SIZE + 64]));
    let mut ctx = Ctx::new();
    for size in [0, 1, 7, 8, 64, 1024, 4096] {
        let engine = init(
            storage.0.as_mut_ptr().cast(),
            size,
            &OPS,
            (&raw mut ctx).cast(),
        );
        assert!(engine.is_null(), "size {size} was accepted");
    }
    // Nothing may have been written to storage that was refused.
    assert!(storage.0.iter().all(|&b| b == 0));
}

#[test]
fn init_rejects_misaligned_storage() {
    let mut storage = Box::new(Storage([0; CE_ENGINE_SIZE + 64]));
    let mut ctx = Ctx::new();
    for offset in 1..8 {
        // SAFETY: `offset` stays inside the 64 spare bytes.
        let misaligned = unsafe { storage.0.as_mut_ptr().add(offset) };
        let engine = init(
            misaligned.cast(),
            CE_ENGINE_SIZE,
            &OPS,
            (&raw mut ctx).cast(),
        );
        assert!(engine.is_null(), "offset {offset} was accepted");
    }
}

#[test]
fn init_copies_the_ops_table() {
    // The header: "*ops is copied".
    let mut ops = OPS;
    let mut fixture = Fixture::with_ops(Ctx::new(), &ops);
    // Overwrite the caller's table: the engine must have its own copy.
    ops = Ops {
        capabilities: None,
        check: None,
        commit: None,
        poll_event: None,
    };
    assert!(ops.commit.is_none());
    fixture.ready(1);
    let (status, _, _) = fixture.present_layers(&[layer(1)]);
    assert_eq!(status, CE_OK);
    assert_eq!(fixture.ctx.commit_count, 1);
}

#[test]
#[ignore = "bug: ce_engine_init accepts storage smaller than CE_ENGINE_SIZE (header: 'at least CE_ENGINE_SIZE bytes')"]
fn init_rejects_storage_below_the_documented_size() {
    // The library accepts any `size` of at least size_of::<ce_engine>(),
    // which is below CE_ENGINE_SIZE. Harmless for the library's own use, but
    // not what the header says.
    let mut storage = Box::new(Storage([0; CE_ENGINE_SIZE + 64]));
    let mut ctx = Ctx::new();
    let engine = init(
        storage.0.as_mut_ptr().cast(),
        CE_ENGINE_SIZE - 1,
        &OPS,
        (&raw mut ctx).cast(),
    );
    assert!(engine.is_null());
}

#[test]
#[ignore = "bug: ce_engine_init accepts a NULL ctx although the header returns NULL if an argument is NULL"]
fn init_rejects_null_ctx() {
    // The header's wording is literal ("or an argument is NULL"); a backend
    // with no state may reasonably want a NULL ctx, so the header or the
    // code should change.
    let mut storage = Box::new(Storage([0; CE_ENGINE_SIZE + 64]));
    // No callbacks: the engine must never hand a NULL ctx to the test backend.
    let ops = Ops {
        capabilities: None,
        check: None,
        commit: None,
        poll_event: None,
    };
    let engine = init(
        storage.0.as_mut_ptr().cast(),
        CE_ENGINE_SIZE,
        &ops,
        null_mut(),
    );
    assert!(engine.is_null());
}

// ---- null pointers ---------------------------------------------------------

#[test]
fn null_engine_is_invalid_argument_everywhere() {
    let info = BufferInfo {
        modifier: 0,
        width: 64,
        height: 64,
        format: XRGB8888,
    };
    let layers = [layer(1)];
    let frame = Frame {
        layers: layers.as_ptr(),
        layer_count: 1,
        output: 0,
    };
    let mut event = Event::default();
    // SAFETY: a null engine is the case under test; every other pointer is
    // a valid local or null.
    unsafe {
        let e: *mut ce_engine = null_mut();
        assert_eq!(
            ce_register_buffer(e, 1, (&raw const info).cast()),
            CE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(ce_unregister_buffer(e, 1), CE_ERR_INVALID_ARGUMENT);
        assert_eq!(ce_begin_write(e, 1), CE_ERR_INVALID_ARGUMENT);
        assert_eq!(ce_finish_write(e, 1), CE_ERR_INVALID_ARGUMENT);
        assert_eq!(ce_buffer_state(e, 1), -1);
        let mut err = ErrorInfo::default();
        let mut submission = 0u64;
        assert_eq!(
            ce_present(
                e,
                (&raw const frame).cast(),
                &mut submission,
                (&raw mut err).cast()
            ),
            CE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(err.status, CE_ERR_INVALID_ARGUMENT);
        assert_eq!(
            ce_present(e, (&raw const frame).cast(), null_mut(), null_mut()),
            CE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(
            ce_poll_event(e, (&raw mut event).cast()),
            CE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(ce_set_latch_timeout(e, 5), CE_ERR_INVALID_ARGUMENT);
        assert_eq!(ce_advance_time(e, 5), CE_ERR_INVALID_ARGUMENT);
    }
}

#[test]
fn null_arguments_with_a_live_engine() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    // SAFETY: the engine is live; the null pointers are the cases under test.
    unsafe {
        assert_eq!(
            ce_register_buffer(f.engine, 2, null()),
            CE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(f.state(2), -1, "a refused registration must not register");
        assert_eq!(ce_poll_event(f.engine, null_mut()), CE_ERR_INVALID_ARGUMENT);
        let mut err = ErrorInfo::default();
        assert_eq!(
            ce_present(f.engine, null(), null_mut(), (&raw mut err).cast()),
            CE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(err.status, CE_ERR_INVALID_ARGUMENT);
        assert_eq!(
            ce_present(f.engine, null(), null_mut(), null_mut()),
            CE_ERR_INVALID_ARGUMENT
        );
    }
    // Layers null with a count: invalid; with count zero: a blank frame.
    let frame = Frame {
        layers: null(),
        layer_count: 1,
        output: 0,
    };
    let (status, info, _) = f.present(&frame);
    assert_eq!(status, CE_ERR_INVALID_ARGUMENT);
    assert_eq!(info.status, status);
    let (status, _, submission) = f.present(&blank());
    assert_eq!(status, CE_OK);
    assert_ne!(submission, u64::MAX);
}

#[test]
fn null_submission_and_info_are_optional() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    let layers = [layer(1)];
    let frame = Frame {
        layers: layers.as_ptr(),
        layer_count: 1,
        output: 0,
    };
    // SAFETY: engine and frame are valid; null out pointers are allowed.
    let status = unsafe { ce_present(f.engine, (&raw const frame).cast(), null_mut(), null_mut()) };
    assert_eq!(status, CE_OK);
    assert_eq!(f.state(1), 2);
    // A failing call with null info must also not crash.
    let bad = Frame {
        layers: layers.as_ptr(),
        layer_count: 1,
        output: 99,
    };
    // SAFETY: as above.
    let status = unsafe { ce_present(f.engine, (&raw const bad).cast(), null_mut(), null_mut()) };
    assert_ne!(status, CE_OK);
}

#[test]
fn failed_present_leaves_the_submission_untouched() {
    let mut f = Fixture::new(Ctx::new());
    let (status, _, submission) = f.present_layers(&[layer(77)]);
    assert_eq!(status, CE_ERR_UNKNOWN_BUFFER);
    assert_eq!(submission, u64::MAX, "*submission written on failure");
}

// ---- layer counts, enums, buffers, rectangles ------------------------------

#[test]
fn layer_count_above_the_limit() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    // Only one layer exists in memory: the engine must refuse on the count
    // alone, before it reads past the array.
    let layers = [layer(1)];
    for count in [MAX_LAYERS + 1, 64, 1 << 20, usize::MAX / 2, usize::MAX] {
        let frame = Frame {
            layers: layers.as_ptr(),
            layer_count: count,
            output: 0,
        };
        let (status, info, _) = f.present(&frame);
        assert_eq!(status, CE_ERR_TOO_MANY_LAYERS, "count {count}");
        assert_eq!(info.status, status);
    }
    assert_eq!(f.state(1), 1, "a refused frame must change nothing");
}

#[test]
fn layer_count_exactly_the_limit() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    let layers = [layer(1); MAX_LAYERS];
    let (status, _, _) = f.present_layers(&layers);
    // Sixteen layers on two planes: some documented plan error, never a
    // crash and never TOO_MANY_LAYERS.
    assert_ne!(status, CE_ERR_TOO_MANY_LAYERS);
    assert!(DOCUMENTED.contains(&status), "{status}");
}

#[test]
fn out_of_range_blend_and_transform() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    for blend in [3u8, 4, 7, 0x80, 0xFF] {
        let mut l = layer(1);
        l.blend = blend;
        let (status, info, _) = f.present_layers(&[layer(1), l]);
        assert_eq!(status, CE_ERR_INVALID_ARGUMENT, "blend {blend}");
        assert_eq!(info.layer, 1, "the offending layer index");
    }
    for transform in [6u8, 7, 8, 0x40, 0xFF] {
        let mut l = layer(1);
        l.transform = transform;
        let (status, info, _) = f.present_layers(&[l]);
        assert_eq!(status, CE_ERR_INVALID_ARGUMENT, "transform {transform}");
        assert_eq!(info.layer, 0);
    }
    assert_eq!(f.state(1), 1);
    assert_eq!(f.ctx.commit_count, 0);
}

#[test]
fn out_of_range_alpha_and_z_order_do_not_crash() {
    for (alpha, z) in [
        (0, i32::MIN),
        (0, i32::MAX),
        (1, -1),
        (ALPHA_OPAQUE, i32::MAX),
    ] {
        let mut f = Fixture::new(Ctx::new());
        f.ready(1);
        let mut l = layer(1);
        l.alpha = alpha;
        l.z_order = z;
        let (status, _, _) = f.present_layers(&[l]);
        assert!(DOCUMENTED.contains(&status), "{status}");
    }
}

#[test]
fn unknown_buffer_and_output() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    for id in [0u32, 2, 63, 64, 1 << 31, u32::MAX] {
        assert_eq!(f.state(id), -1, "id {id}");
        assert_eq!(f.unreg(id), CE_ERR_UNKNOWN_BUFFER);
        assert_eq!(f.begin(id), CE_ERR_UNKNOWN_BUFFER);
        assert_eq!(f.finish(id), CE_ERR_UNKNOWN_BUFFER);
        let (status, info, _) = f.present_layers(&[layer(1), layer(id)]);
        assert_eq!(status, CE_ERR_UNKNOWN_BUFFER);
        assert_eq!((info.layer, info.buffer), (1, id));
    }
    let layers = [layer(1)];
    for output in [1u32, 4, 99, u32::MAX] {
        let frame = Frame {
            layers: layers.as_ptr(),
            layer_count: 1,
            output,
        };
        let (status, _, _) = f.present(&frame);
        assert_eq!(status, CE_ERR_UNKNOWN_OUTPUT, "output {output}");
    }
}

#[test]
fn buffer_registration_edges() {
    let mut f = Fixture::new(Ctx::new());
    assert_eq!(f.reg(5, 64, 64, XRGB8888), CE_OK);
    assert_eq!(f.state(5), 0, "a new buffer is writable");
    assert_eq!(f.reg(5, 64, 64, XRGB8888), CE_ERR_DUPLICATE_BUFFER);
    // Odd descriptions are accepted as data; the plan rejects them later.
    assert_eq!(f.reg(u32::MAX, 0, 0, 0), CE_OK);
    assert_eq!(f.reg(0, u32::MAX, u32::MAX, u32::MAX), CE_OK);
    assert_eq!(f.reg(6, 64, 64, NV12), CE_OK);
    for id in 100..(100 + MAX_BUFFERS) {
        let status = f.reg(id, 8, 8, XRGB8888);
        assert!(
            status == CE_OK || status == CE_ERR_BUFFER_TABLE_FULL,
            "{status}"
        );
    }
    assert_eq!(f.reg(999, 8, 8, XRGB8888), CE_ERR_BUFFER_TABLE_FULL);
    // The table frees up again.
    assert_eq!(f.unreg(5), CE_OK);
    assert_eq!(f.reg(999, 8, 8, XRGB8888), CE_OK);
    assert_eq!(f.unreg(5), CE_ERR_UNKNOWN_BUFFER);
}

#[test]
fn buffers_with_garbage_descriptions_cannot_be_shown() {
    for (id, (w, h, format)) in [
        (1, (0, 0, XRGB8888)),
        (2, (64, 64, 0)),
        (3, (64, 64, 0xFFFF_FFFF)),
        (5, (64, 64, NV12)),
    ] {
        let mut f = Fixture::new(Ctx::new());
        assert_eq!(f.reg(id, w, h, format), CE_OK);
        assert_eq!(f.finish(id), CE_OK);
        let mut l = layer(id);
        l.source = rect(0, 0, w.min(64), h.min(64));
        let (status, _, _) = f.present_layers(&[l]);
        assert!(
            matches!(
                status,
                CE_ERR_INVALID_SOURCE | CE_ERR_NO_PLANE | CE_ERR_INVALID_DESTINATION
            ),
            "buffer {id}: {status}"
        );
        assert_eq!(f.state(id), 1, "a refused frame changes nothing");
    }
}

#[test]
fn extreme_rectangles() {
    let extremes = [
        rect(0, 0, 0, 0),
        rect(0, 0, 0, 64),
        rect(0, 0, 64, 0),
        rect(i32::MAX, 0, 64, 64),
        rect(i32::MIN, 0, 64, 64),
        rect(0, i32::MAX, 64, 64),
        rect(0, i32::MIN, 64, 64),
        rect(0, 0, u32::MAX, u32::MAX),
        rect(i32::MAX, i32::MAX, u32::MAX, u32::MAX),
        rect(i32::MIN, i32::MIN, u32::MAX, u32::MAX),
        rect(i32::MIN, i32::MIN, 64, 64),
        rect(i32::MAX - 10, i32::MAX - 10, 64, 64),
        rect(-32, -32, 64, 64),
        rect(1900, 1060, 64, 64),
        rect(0, 0, 1 << 31, 1 << 31),
        rect(-1, -1, u32::MAX, u32::MAX),
    ];
    for source in extremes {
        for destination in extremes {
            // A fresh engine per case: an accepted frame would otherwise
            // leave a submission pending.
            let mut f = Fixture::new(Ctx::new());
            f.ready(1);
            let mut l = layer(1);
            l.source = source;
            l.destination = destination;
            let (status, info, _) = f.present_layers(&[l]);
            assert!(
                DOCUMENTED.contains(&status),
                "{source:?} -> {destination:?}: {status}"
            );
            if status != CE_OK {
                assert_eq!(f.state(1), 1, "refused: {source:?} -> {destination:?}");
                assert_eq!(info.layer, 0, "{source:?} -> {destination:?}");
            }
        }
    }
}

#[test]
fn documented_rectangle_statuses() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    let mut empty_source = layer(1);
    empty_source.source = rect(0, 0, 0, 64);
    assert_eq!(f.present_layers(&[empty_source]).0, CE_ERR_INVALID_SOURCE);
    let mut outside = layer(1);
    outside.source = rect(32, 32, 64, 64);
    assert_eq!(f.present_layers(&[outside]).0, CE_ERR_INVALID_SOURCE);
    let mut empty_destination = layer(1);
    empty_destination.destination = rect(0, 0, 64, 0);
    assert_eq!(
        f.present_layers(&[empty_destination]).0,
        CE_ERR_INVALID_DESTINATION
    );
    let mut off_output = layer(1);
    off_output.destination = rect(5000, 5000, 64, 64);
    let (status, _, _) = f.present_layers(&[off_output]);
    // Entirely off the output: no assignment, so a blank frame may result;
    // or a destination error. Either is documented.
    assert!(
        status == CE_OK || status == CE_ERR_INVALID_DESTINATION,
        "{status}"
    );
}

// ---- calls in the wrong order ----------------------------------------------

#[test]
fn present_before_the_buffer_is_ready() {
    let mut f = Fixture::new(Ctx::new());
    assert_eq!(f.reg(1, 64, 64, XRGB8888), CE_OK);
    let (status, info, _) = f.present_layers(&[layer(1)]);
    assert_eq!(status, CE_ERR_BUFFER_NOT_READY);
    assert_eq!(info.buffer, 1);
    assert_eq!(f.ctx.commit_count, 0);
}

#[test]
fn buffer_lifecycle_order_errors() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    assert_eq!(f.finish(1), CE_ERR_BUFFER_NOT_WRITABLE);
    assert_eq!(f.begin(1), CE_OK);
    assert_eq!(f.state(1), 0);
    assert_eq!(f.finish(1), CE_OK);
    let (status, _, submission) = f.present_layers(&[layer(1)]);
    assert_eq!(status, CE_OK);
    assert_eq!(f.state(1), 2);
    assert_eq!(f.begin(1), CE_ERR_BUFFER_IN_USE);
    assert_eq!(f.unreg(1), CE_ERR_BUFFER_IN_USE);
    assert_eq!(f.finish(1), CE_ERR_BUFFER_NOT_WRITABLE);
    // A second frame while the first is not latched.
    f.ready(2);
    let (status, _, _) = f.present_layers(&[layer(2)]);
    assert_eq!(status, CE_ERR_BUSY);
    assert_eq!(f.state(2), 1);
    f.ctx.push(EV_PRESENTED, 0, submission);
    assert_eq!(f.poll().0, 1);
    assert_eq!(f.state(1), 3);
    assert_eq!(f.begin(1), CE_ERR_BUFFER_IN_USE);
    assert_eq!(f.unreg(1), CE_ERR_BUFFER_IN_USE);
}

#[test]
fn engine_is_usable_straight_after_init() {
    let mut f = Fixture::new(Ctx::new());
    assert_eq!(f.poll().0, 0);
    assert_eq!(f.present(&blank()).0, CE_OK);
}

#[test]
fn latch_that_matches_nothing() {
    let mut f = Fixture::new(Ctx::new());
    f.ctx.push(EV_PRESENTED, 0, 12345);
    assert_eq!(f.poll().0, CE_ERR_UNEXPECTED_LATCH);
    // And for an output the engine has never used.
    f.ctx.push(EV_PRESENTED, 3, 1);
    f.ctx.push(EV_PRESENTED, u32::MAX, u64::MAX);
    for _ in 0..2 {
        let (rc, _) = f.poll();
        assert!(rc < 0 && DOCUMENTED.contains(&rc), "{rc}");
    }
    assert_eq!(f.poll().0, 0);
}

#[test]
fn latch_with_the_wrong_submission() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    let (status, _, submission) = f.present_layers(&[layer(1)]);
    assert_eq!(status, CE_OK);
    f.ctx.push(EV_PRESENTED, 0, submission.wrapping_add(1));
    assert_eq!(f.poll().0, CE_ERR_UNEXPECTED_LATCH);
    assert_eq!(f.state(1), 2, "a wrong latch must not move the buffer");
    // The right one still works afterwards.
    f.ctx.push(EV_PRESENTED, 0, submission);
    assert_eq!(f.poll().0, 1);
    assert_eq!(f.state(1), 3);
}

#[test]
fn duplicate_latch() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    let (_, _, submission) = f.present_layers(&[layer(1)]);
    f.ctx.push(EV_PRESENTED, 0, submission);
    f.ctx.push(EV_PRESENTED, 0, submission);
    let (rc, event) = f.poll();
    assert_eq!((rc, event.kind), (1, 0));
    assert_eq!(f.poll().0, CE_ERR_UNEXPECTED_LATCH);
    assert_eq!(f.state(1), 3);
}

#[test]
fn unknown_backend_event_kinds_are_skipped() {
    let mut f = Fixture::new(Ctx::new());
    for kind in [3u8, 6, 100, 255] {
        f.ctx.push(kind, 9, 9);
    }
    assert_eq!(f.poll().0, 0);
}

#[test]
fn use_after_device_lost() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    f.ready(2);
    let (status, _, _) = f.present_layers(&[layer(1)]);
    assert_eq!(status, CE_OK);
    f.ctx.push(EV_DEVICE_LOST, 0, 0);
    // Drain: the pending frame is dropped and its buffer released, then the
    // loss itself is reported; the exact order is the engine's.
    let kinds = f.drain();
    assert!(kinds.contains(&3), "no DEVICE_LOST event in {kinds:?}");
    assert!(kinds.contains(&1), "pending frame not dropped in {kinds:?}");
    assert!(kinds.contains(&2), "buffer not released in {kinds:?}");
    // The backend now fails every commit, as a lost device does.
    f.ctx.commit_rc = CE_ERR_DEVICE_LOST;
    let (status, _, _) = f.present_layers(&[layer(2)]);
    assert_eq!(status, CE_ERR_DEVICE_LOST);
    assert_eq!(f.state(2), 1);
    // Housekeeping still works and is bounded.
    assert_eq!(f.begin(2), CE_OK);
    assert_eq!(f.finish(2), CE_OK);
    assert_eq!(f.unreg(2), CE_OK);
    assert_eq!(f.poll().0, 0);
}

#[test]
fn device_lost_with_nothing_pending() {
    let mut f = Fixture::new(Ctx::new());
    f.ctx.push(EV_DEVICE_LOST, 0, 0);
    f.ctx.push(EV_DEVICE_LOST, 0, 0);
    let kinds = f.drain();
    assert!(kinds.contains(&3), "{kinds:?}");
}

#[test]
fn backend_events_are_decoded_by_backend_kind() {
    // Backend kinds: 0 latched, 1 device lost, 2 capabilities changed.
    let mut f = Fixture::new(Ctx::new());
    f.ctx.push(2, 0, 0);
    assert_eq!(f.drain(), vec![4]);
}

// ---- backend misbehaviour --------------------------------------------------

#[test]
fn backend_codes_map_to_documented_statuses() {
    for (code, expected) in [
        (CE_ERR_BACKEND_REJECTED, CE_ERR_BACKEND_REJECTED),
        (CE_ERR_BACKEND_BUSY, CE_ERR_BACKEND_BUSY),
        (CE_ERR_DEVICE_LOST, CE_ERR_DEVICE_LOST),
        (CE_ERR_BACKEND_UNKNOWN_BUFFER, CE_ERR_BACKEND_UNKNOWN_BUFFER),
        (-7, CE_ERR_BACKEND_PLATFORM),
        (1, CE_ERR_BACKEND_PLATFORM),
        (i32::MIN, CE_ERR_BACKEND_PLATFORM),
        (i32::MAX, CE_ERR_BACKEND_PLATFORM),
    ] {
        let mut ctx = Ctx::new();
        ctx.commit_rc = code;
        let mut f = Fixture::new(ctx);
        f.ready(1);
        let (status, info, _) = f.present_layers(&[layer(1)]);
        assert_eq!(status, expected, "commit returned {code}");
        if expected == CE_ERR_BACKEND_PLATFORM {
            assert_eq!(info.platform, code);
        }
        assert_eq!(f.state(1), 1, "failed commit must leave the buffer ready");
    }
}

#[test]
fn backend_check_rejects_everything() {
    let mut ctx = Ctx::new();
    ctx.check_rc = CE_ERR_BACKEND_REJECTED;
    let mut f = Fixture::new(ctx);
    f.ready(1);
    let (status, _, _) = f.present_layers(&[layer(1)]);
    assert_eq!(status, CE_ERR_BACKEND_REJECTED);
    assert!(f.ctx.check_count >= 1);
    assert_eq!(f.ctx.commit_count, 0);
    assert_eq!(f.state(1), 1);
}

#[test]
fn backend_with_no_functions() {
    let ops = Ops {
        capabilities: None,
        check: None,
        commit: None,
        poll_event: None,
    };
    let mut f = Fixture::with_ops(Ctx::new(), &ops);
    assert_eq!(f.poll().0, 0);
    f.ready(1);
    let (status, _, _) = f.present_layers(&[layer(1)]);
    // Without capabilities no output exists.
    assert_eq!(status, CE_ERR_UNKNOWN_OUTPUT);
    let (status, _, _) = f.present(&blank());
    assert!(DOCUMENTED.contains(&status), "{status}");
}

#[test]
fn backend_without_commit_reports_device_lost() {
    let ops = Ops {
        commit: None,
        ..OPS
    };
    let mut f = Fixture::with_ops(Ctx::new(), &ops);
    f.ready(1);
    assert_eq!(f.present_layers(&[layer(1)]).0, CE_ERR_DEVICE_LOST);
    assert_eq!(f.state(1), 1);
}

#[test]
fn backend_capabilities_that_are_garbage() {
    let mut ctx = Ctx::new();
    ctx.caps = CapsMode::Garbage;
    let mut f = Fixture::new(ctx);
    f.ready(1);
    for output in [0u32, 1, u32::MAX] {
        let layers = [layer(1)];
        let frame = Frame {
            layers: layers.as_ptr(),
            layer_count: 1,
            output,
        };
        let (status, _, _) = f.present(&frame);
        assert!(DOCUMENTED.contains(&status), "{status}");
    }
}

#[test]
fn backend_capabilities_empty() {
    let mut ctx = Ctx::new();
    ctx.caps = CapsMode::Empty;
    let mut f = Fixture::new(ctx);
    f.ready(1);
    let (status, _, _) = f.present_layers(&[layer(1)]);
    assert_eq!(status, CE_ERR_UNKNOWN_OUTPUT);
}

// ---- time ------------------------------------------------------------------

#[test]
fn time_calls_accept_the_whole_u64_range() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    // SAFETY: the engine is live.
    unsafe {
        assert_eq!(ce_set_latch_timeout(f.engine, u64::MAX), CE_OK);
        assert_eq!(ce_advance_time(f.engine, u64::MAX / 2), CE_OK);
    }
    let (status, _, _) = f.present_layers(&[layer(1)]);
    assert_eq!(status, CE_OK);
    // SAFETY: as above.
    unsafe {
        // `now + timeout` would overflow a u64.
        assert_eq!(ce_advance_time(f.engine, u64::MAX), CE_OK);
        assert_eq!(ce_advance_time(f.engine, u64::MAX), CE_OK);
        // The header forbids going back; it must still not crash.
        assert_eq!(ce_advance_time(f.engine, 0), CE_OK);
        assert_eq!(ce_set_latch_timeout(f.engine, 0), CE_OK);
        assert_eq!(ce_set_latch_timeout(f.engine, 1), CE_OK);
        assert_eq!(ce_advance_time(f.engine, 1), CE_OK);
    }
    let _ = f.drain();
}

#[test]
fn latch_timeout_is_reported_once() {
    let mut f = Fixture::new(Ctx::new());
    f.ready(1);
    // SAFETY: the engine is live.
    unsafe {
        assert_eq!(ce_set_latch_timeout(f.engine, 10), CE_OK);
        assert_eq!(ce_advance_time(f.engine, 100), CE_OK);
    }
    let (status, _, submission) = f.present_layers(&[layer(1)]);
    assert_eq!(status, CE_OK);
    // SAFETY: as above.
    unsafe { assert_eq!(ce_advance_time(f.engine, 111), CE_OK) };
    let (rc, event) = f.poll();
    assert_eq!((rc, event.kind, event.submission), (1, 5, submission));
    // SAFETY: as above.
    unsafe { assert_eq!(ce_advance_time(f.engine, 500), CE_OK) };
    assert_eq!(f.poll().0, 0, "reported twice");
    assert_eq!(f.state(1), 2, "the buffer stays in use");
}

// ---- seeded random descriptors ---------------------------------------------

/// SplitMix64: small, seedable, the same on every platform.
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

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }

    fn i32(&mut self) -> i32 {
        match self.below(4) {
            0 => self.pick(&[i32::MIN, i32::MAX, -1, 0, 1]),
            1 => self.below(2200) as i32 - 100,
            2 => self.next() as i32,
            _ => self.below(128) as i32 - 64,
        }
    }

    fn u32(&mut self) -> u32 {
        match self.below(4) {
            0 => self.pick(&[0, 1, u32::MAX, 1 << 31, (1 << 31) - 1]),
            1 => self.below(2100) as u32,
            2 => self.next() as u32,
            _ => self.below(130) as u32,
        }
    }

    fn rect(&mut self) -> Rect {
        rect(self.i32(), self.i32(), self.u32(), self.u32())
    }

    fn layer(&mut self) -> Layer {
        Layer {
            buffer: self.below(12) as u32,
            source: if self.below(2) == 0 {
                rect(0, 0, 64, 64)
            } else {
                self.rect()
            },
            destination: if self.below(2) == 0 {
                rect(self.below(1800) as i32, self.below(1000) as i32, 64, 64)
            } else {
                self.rect()
            },
            z_order: self.i32(),
            alpha: self.next() as u16,
            blend: if self.below(4) == 0 {
                self.next() as u8
            } else {
                self.below(3) as u8
            },
            transform: if self.below(4) == 0 {
                self.next() as u8
            } else {
                self.below(6) as u8
            },
        }
    }
}

fn run_random(seed: u64, caps: CapsMode) {
    let mut rng = Rng(seed);
    let mut ctx = Ctx::new();
    ctx.caps = caps;
    let mut f = Fixture::new(ctx);
    // SAFETY: the engine is live.
    unsafe { assert_eq!(ce_set_latch_timeout(f.engine, 1 + rng.below(50)), CE_OK) };
    let mut now = 0u64;
    for step in 0..1500 {
        let id = rng.below(14) as u32;
        let at = |what: &str, status: i32| format!("seed {seed} step {step}: {what} -> {status}");
        match rng.below(14) {
            0 => {
                let formats = [XRGB8888, ARGB8888, NV12, 0, u32::MAX, rng.next() as u32];
                let w = rng.pick(&[0, 1, 64, 128, 4096, u32::MAX]);
                let h = rng.pick(&[0, 1, 64, 4096]);
                let status = f.reg(id, w, h, rng.pick(&formats));
                assert!(DOCUMENTED.contains(&status), "{}", at("register", status));
            }
            1 => {
                let status = f.unreg(id);
                assert!(DOCUMENTED.contains(&status), "{}", at("unregister", status));
            }
            2 => {
                let status = f.begin(id);
                assert!(DOCUMENTED.contains(&status), "{}", at("begin", status));
            }
            3 | 4 => {
                let status = f.finish(id);
                assert!(DOCUMENTED.contains(&status), "{}", at("finish", status));
            }
            5 => {
                let state = f.state(id);
                assert!((-1..=3).contains(&state), "{}", at("state", state));
            }
            6..=8 => {
                let count = rng.below(MAX_LAYERS as u64 + 4) as usize;
                let mut layers = [layer(0); MAX_LAYERS + 4];
                for slot in layers.iter_mut() {
                    *slot = rng.layer();
                }
                let frame = Frame {
                    layers: layers.as_ptr(),
                    layer_count: count,
                    output: if rng.below(8) == 0 {
                        rng.next() as u32
                    } else {
                        0
                    },
                };
                let (status, _, submission) = f.present(&frame);
                assert!(DOCUMENTED.contains(&status), "{}", at("present", status));
                if status != CE_OK {
                    assert_eq!(submission, u64::MAX, "{}", at("submission", status));
                }
            }
            9 => {
                // The backend latches the newest commit, or something wrong.
                match rng.below(5) {
                    0 | 1 => {
                        if let Some(last) = f.ctx.last_commit() {
                            f.ctx.push(EV_PRESENTED, 0, last);
                        }
                    }
                    2 => f.ctx.push(EV_PRESENTED, rng.below(6) as u32, rng.next()),
                    3 => f.ctx.push(rng.below(6) as u8, 0, rng.next()),
                    _ => {
                        if rng.below(8) == 0 {
                            f.ctx.push(EV_DEVICE_LOST, 0, 0);
                        }
                    }
                }
            }
            10 | 11 => {
                let (rc, event) = f.poll();
                assert!(
                    rc == 1 || rc == 0 || DOCUMENTED.contains(&rc),
                    "{}",
                    at("poll", rc)
                );
                if rc == 1 {
                    assert!(event.kind <= 5, "{}", at("event kind", event.kind as i32));
                }
            }
            12 => {
                now = now.saturating_add(rng.below(40));
                if rng.below(30) == 0 {
                    now = u64::MAX - rng.below(3);
                }
                // SAFETY: the engine is live.
                unsafe { assert_eq!(ce_advance_time(f.engine, now), CE_OK) };
            }
            _ => {
                f.ctx.commit_rc = rng.pick(&[CE_OK, CE_OK, CE_OK, CE_ERR_BACKEND_BUSY, -77]);
                f.ctx.check_rc = rng.pick(&[CE_OK, CE_OK, CE_OK, CE_ERR_BACKEND_REJECTED]);
            }
        }
    }
    // The engine is still usable.
    let _ = f.poll();
    let _ = f.state(0);
}

#[test]
fn random_descriptors_normal_backend() {
    for seed in 0..40 {
        run_random(seed, CapsMode::Normal);
    }
}

#[test]
fn random_descriptors_garbage_and_empty_backends() {
    for seed in 100..110 {
        run_random(seed, CapsMode::Garbage);
        run_random(seed, CapsMode::Empty);
    }
}

#[test]
fn random_generator_is_deterministic() {
    let mut a = Rng(7);
    let mut b = Rng(7);
    for _ in 0..100 {
        assert_eq!(a.next(), b.next());
    }
}
