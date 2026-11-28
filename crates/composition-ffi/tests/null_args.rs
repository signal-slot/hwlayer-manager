//! Null and invalid arguments of every `ce_*` function that takes a pointer
//! or a handle, called from Rust. Each call must return the status that
//! `include/composition_engine.h` documents and must not panic (a panic
//! across `extern "C"` aborts the test process).
//!
//! The structures of the crate have private fields. Where a test only needs a
//! valid-but-empty value it uses an all-zero one (valid for every field: plain
//! integers, `Option` of a function pointer, raw pointers). `ce_frame`,
//! `ce_layer` and `ce_advise_options` are mirrored below because the tests
//! need to set their fields.
//!
//! `malformed.rs` covers the engine functions in more depth with a scripted
//! backend; this file is the per-function null/invalid matrix and also covers
//! `ce_advise` and `ce_explain_layer`.

// Reason: this file calls the `unsafe extern "C"` API.
#![allow(unsafe_code)]

use core::ffi::c_void;
use core::mem::{size_of, zeroed};
use core::ptr::{null, null_mut};

use composition_ffi::*;

const MAX_LAYERS: usize = 16;

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
struct ComposeTarget {
    modifier: u64,
    format: u32,
    blend: u8,
    enabled: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AdviseOptions {
    max_steps: u32,
    max_backend_checks: u32,
    max_candidates: u32,
    compose: ComposeTarget,
}

const RECT: Rect = Rect {
    x: 0,
    y: 0,
    width: 16,
    height: 16,
};

fn layer() -> Layer {
    Layer {
        buffer: 1,
        source: RECT,
        destination: RECT,
        z_order: 0,
        alpha: 0xFFFF,
        blend: 0,
        transform: 0,
    }
}

fn frame(layers: *const Layer, count: usize) -> Frame {
    Frame {
        layers,
        layer_count: count,
        output: 0,
    }
}

/// A zeroed value of a crate structure.
fn zero<T>() -> T {
    // SAFETY: only used for the `ce_*` structures and the mirrors above, whose
    // fields are integers, raw pointers and `Option<fn>`: all-zero is valid.
    unsafe { zeroed() }
}

/// A pointer to a mirror, as the crate's type the function expects.
fn as_c<T, U>(value: &T) -> *const U {
    core::ptr::from_ref(value).cast()
}

/// Engine storage with a backend that has no functions and no outputs.
struct Live {
    _storage: Vec<u64>,
    engine: *mut ce_engine,
}

fn live() -> Live {
    let mut storage = vec![0u64; CE_ENGINE_SIZE / 8];
    let ops: ce_backend_ops = zero();
    // SAFETY: storage is CE_ENGINE_SIZE bytes, 8-aligned and outlives the
    // engine (kept in `Live`); `ops` is a valid table with no functions.
    let engine = unsafe {
        ce_engine_init(
            storage.as_mut_ptr().cast(),
            CE_ENGINE_SIZE,
            &ops,
            null_mut::<c_void>(),
        )
    };
    assert!(!engine.is_null());
    Live {
        _storage: storage,
        engine,
    }
}

fn status_of(info: &ce_error_info) -> i32 {
    // SAFETY: `ce_error_info` is `repr(C)` and starts with `status: i32`;
    // `info` is a valid reference.
    unsafe { core::ptr::from_ref(info).cast::<i32>().read() }
}

// ---- ce_engine_init --------------------------------------------------------

#[test]
fn init_null_storage_or_ops() {
    let mut storage = vec![0u64; CE_ENGINE_SIZE / 8];
    let ops: ce_backend_ops = zero();
    // SAFETY: the null pointers are rejected before use; the others are valid.
    unsafe {
        assert!(ce_engine_init(null_mut(), CE_ENGINE_SIZE, &ops, null_mut()).is_null());
        assert!(ce_engine_init(
            storage.as_mut_ptr().cast(),
            CE_ENGINE_SIZE,
            null(),
            null_mut()
        )
        .is_null());
        assert!(ce_engine_init(null_mut(), 0, null(), null_mut()).is_null());
    }
}

#[test]
fn init_small_or_misaligned_storage() {
    let mut storage = vec![0u64; CE_ENGINE_SIZE / 8 + 1];
    let ops: ce_backend_ops = zero();
    let base = storage.as_mut_ptr().cast::<u8>();
    // SAFETY: rejected before the storage is touched; sizes 0 and 1 are below
    // the engine size, and base + 1 stays inside the allocation.
    unsafe {
        assert!(ce_engine_init(base.cast(), 0, &ops, null_mut()).is_null());
        assert!(ce_engine_init(base.cast(), 1, &ops, null_mut()).is_null());
        assert!(ce_engine_init(base.add(1).cast(), CE_ENGINE_SIZE, &ops, null_mut()).is_null());
    }
}

// ---- engine functions with a null engine -----------------------------------

#[test]
fn null_engine_is_invalid_argument() {
    let bi: ce_buffer_info = zero();
    let fr = frame(null(), 0);
    let mut ev: ce_event = zero();
    let mut sub = 77u64;
    let mut inf: ce_error_info = zero();
    // SAFETY: the engine is null, which every function checks first; the other
    // pointers are valid.
    unsafe {
        assert_eq!(
            ce_register_buffer(null_mut(), 1, &bi),
            CE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(ce_unregister_buffer(null_mut(), 1), CE_ERR_INVALID_ARGUMENT);
        assert_eq!(ce_begin_write(null_mut(), 1), CE_ERR_INVALID_ARGUMENT);
        assert_eq!(ce_finish_write(null_mut(), 1), CE_ERR_INVALID_ARGUMENT);
        assert_eq!(ce_buffer_state(null(), 1), -1);
        assert_eq!(
            ce_present(null_mut(), as_c(&fr), &mut sub, &mut inf),
            CE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(ce_poll_event(null_mut(), &mut ev), CE_ERR_INVALID_ARGUMENT);
        assert_eq!(ce_set_latch_timeout(null_mut(), 5), CE_ERR_INVALID_ARGUMENT);
        assert_eq!(ce_advance_time(null_mut(), 5), CE_ERR_INVALID_ARGUMENT);
    }
    assert_eq!(sub, 77, "a failed present leaves *submission alone");
    assert_eq!(status_of(&inf), CE_ERR_INVALID_ARGUMENT);
}

// ---- engine functions with a live engine -----------------------------------

#[test]
fn register_buffer_null_info() {
    let e = live();
    // SAFETY: engine from init; a null info is rejected.
    unsafe {
        assert_eq!(
            ce_register_buffer(e.engine, 1, null()),
            CE_ERR_INVALID_ARGUMENT
        );
        assert_eq!(ce_buffer_state(e.engine, 1), -1, "nothing registered");
    }
}

#[test]
fn unknown_buffer_ids() {
    let e = live();
    // SAFETY: engine from init, plain integer arguments.
    unsafe {
        for id in [0, 1, 63, 64, 0xFFFF_FFFF] {
            assert_eq!(ce_unregister_buffer(e.engine, id), CE_ERR_UNKNOWN_BUFFER);
            assert_eq!(ce_begin_write(e.engine, id), CE_ERR_UNKNOWN_BUFFER);
            assert_eq!(ce_finish_write(e.engine, id), CE_ERR_UNKNOWN_BUFFER);
            assert_eq!(ce_buffer_state(e.engine, id), -1);
        }
    }
}

#[test]
fn poll_event_null_out_pointer() {
    let e = live();
    let mut ev: ce_event = zero();
    // SAFETY: valid engine; a null `event` is rejected, a valid one gets "none".
    unsafe {
        assert_eq!(ce_poll_event(e.engine, null_mut()), CE_ERR_INVALID_ARGUMENT);
        assert_eq!(ce_poll_event(e.engine, &mut ev), 0);
    }
}

#[test]
fn present_null_frame() {
    let e = live();
    let mut sub = 9u64;
    let mut inf: ce_error_info = zero();
    // SAFETY: engine from init; the null frame is rejected, the others are valid.
    unsafe {
        assert_eq!(
            ce_present(e.engine, null(), &mut sub, &mut inf),
            CE_ERR_INVALID_ARGUMENT
        );
        // submission and info are optional.
        assert_eq!(
            ce_present(e.engine, null(), null_mut(), null_mut()),
            CE_ERR_INVALID_ARGUMENT
        );
    }
    assert_eq!(status_of(&inf), CE_ERR_INVALID_ARGUMENT);
    assert_eq!(sub, 9);
}

#[test]
fn present_layer_pointer_and_count() {
    let e = live();
    let mut inf: ce_error_info = zero();
    let layers = [layer(); MAX_LAYERS + 1];

    let null_layers = frame(null(), 1);
    // SAFETY: engine and frame valid; the null `layers` is checked by the call.
    let rc = unsafe { ce_present(e.engine, as_c(&null_layers), null_mut(), &mut inf) };
    assert_eq!(rc, CE_ERR_INVALID_ARGUMENT);

    for count in [MAX_LAYERS + 1, usize::MAX] {
        let too_many = frame(layers.as_ptr(), count);
        // SAFETY: the count is rejected before any layer is read.
        let rc = unsafe { ce_present(e.engine, as_c(&too_many), null_mut(), &mut inf) };
        assert_eq!(rc, CE_ERR_TOO_MANY_LAYERS);
        assert_eq!(status_of(&inf), CE_ERR_TOO_MANY_LAYERS);
    }

    // Null layers with a zero count is valid; the backend has no output.
    let empty = frame(null(), 0);
    // SAFETY: valid engine and frame.
    let rc = unsafe { ce_present(e.engine, as_c(&empty), null_mut(), null_mut()) };
    assert_eq!(rc, CE_ERR_UNKNOWN_OUTPUT);
}

#[test]
fn present_invalid_layer_values() {
    let e = live();
    let mut inf: ce_error_info = zero();
    let mut bad_blend = layer();
    bad_blend.blend = 200;
    let mut bad_transform = layer();
    bad_transform.transform = 200;
    for bad in [bad_blend, bad_transform] {
        let layers = [layer(), bad];
        let fr = frame(layers.as_ptr(), 2);
        // SAFETY: the frame points to two valid layers.
        let rc = unsafe { ce_present(e.engine, as_c(&fr), null_mut(), &mut inf) };
        assert_eq!(rc, CE_ERR_INVALID_ARGUMENT);
    }
}

#[test]
fn time_functions_take_any_value() {
    let e = live();
    // SAFETY: engine from init.
    unsafe {
        for v in [0, 1, u64::MAX] {
            assert_eq!(ce_set_latch_timeout(e.engine, v), CE_OK);
        }
        assert_eq!(ce_advance_time(e.engine, 0), CE_OK);
    }
}

// ---- ce_advise ---------------------------------------------------------------

struct Advise {
    caps: Box<ce_capabilities>,
    options: AdviseOptions,
    layers: [Layer; 1],
    advice: Box<ce_advice>,
}

fn advise_setup() -> Advise {
    Advise {
        // Boxed: several KiB each.
        caps: Box::new(zero()),
        options: zero(),
        layers: [layer()],
        advice: Box::new(zero()),
    }
}

/// The header: on failure "*advice untouched"; it starts zeroed.
fn advice_untouched(advice: &ce_advice) -> bool {
    // SAFETY: reads the bytes of a valid reference; padding bytes of a zeroed
    // value are zero too.
    let bytes = unsafe {
        core::slice::from_raw_parts(
            core::ptr::from_ref(advice).cast::<u8>(),
            size_of::<ce_advice>(),
        )
    };
    bytes.iter().all(|b| *b == 0)
}

#[test]
fn advise_null_required_pointers() {
    let mut a = advise_setup();
    let fr = frame(a.layers.as_ptr(), 1);
    let frp: *const ce_frame = as_c(&fr);
    let adv: *mut ce_advice = &mut *a.advice;
    let caps: *const ce_capabilities = &*a.caps;
    let opts: *const ce_advise_options = as_c(&a.options);
    let mut inf: ce_error_info = zero();
    // SAFETY: each call has exactly one required pointer null; the others point
    // to live values, and the layers pointer of `fr` is valid for one layer.
    let calls = unsafe {
        [
            ce_advise(
                null(),
                null(),
                0,
                frp,
                opts,
                None,
                null_mut(),
                adv,
                &mut inf,
            ),
            ce_advise(
                caps,
                null(),
                0,
                null(),
                opts,
                None,
                null_mut(),
                adv,
                &mut inf,
            ),
            ce_advise(
                caps,
                null(),
                0,
                frp,
                null(),
                None,
                null_mut(),
                adv,
                &mut inf,
            ),
            ce_advise(
                caps,
                null(),
                0,
                frp,
                opts,
                None,
                null_mut(),
                null_mut(),
                &mut inf,
            ),
            // `info` is optional.
            ce_advise(
                null(),
                null(),
                0,
                frp,
                opts,
                None,
                null_mut(),
                adv,
                null_mut(),
            ),
        ]
    };
    for rc in calls {
        assert_eq!(rc, CE_ERR_INVALID_ARGUMENT);
    }
    assert_eq!(status_of(&inf), CE_ERR_INVALID_ARGUMENT);
    assert!(advice_untouched(&a.advice));
}

#[test]
fn advise_buffer_table_and_layer_arguments() {
    let mut a = advise_setup();
    let fr = frame(a.layers.as_ptr(), 1);
    let frp: *const ce_frame = as_c(&fr);
    let adv: *mut ce_advice = &mut *a.advice;
    let caps: *const ce_capabilities = &*a.caps;
    let opts: *const ce_advise_options = as_c(&a.options);
    let mut inf: ce_error_info = zero();

    let too_many = frame(a.layers.as_ptr(), MAX_LAYERS + 1);
    let no_layers = frame(null(), 1);
    let too_many_p: *const ce_frame = as_c(&too_many);
    let no_layers_p: *const ce_frame = as_c(&no_layers);
    // SAFETY: the null table with a count, the huge count and the bad frames are
    // all rejected before being read; every other pointer is valid.
    let results = unsafe {
        [
            // null buffers with a count
            ce_advise(caps, null(), 1, frp, opts, None, null_mut(), adv, &mut inf),
            // count beyond isize::MAX entries
            ce_advise(
                caps,
                null(),
                usize::MAX,
                frp,
                opts,
                None,
                null_mut(),
                adv,
                &mut inf,
            ),
            ce_advise(
                caps,
                null(),
                0,
                too_many_p,
                opts,
                None,
                null_mut(),
                adv,
                &mut inf,
            ),
            // null layers with a count
            ce_advise(
                caps,
                null(),
                0,
                no_layers_p,
                opts,
                None,
                null_mut(),
                adv,
                &mut inf,
            ),
        ]
    };
    assert_eq!(
        results,
        [
            CE_ERR_INVALID_ARGUMENT,
            CE_ERR_INVALID_ARGUMENT,
            CE_ERR_TOO_MANY_LAYERS,
            CE_ERR_INVALID_ARGUMENT
        ]
    );
    assert!(advice_untouched(&a.advice));
}

#[test]
fn advise_invalid_values() {
    let mut a = advise_setup();
    let mut inf: ce_error_info = zero();
    let adv: *mut ce_advice = &mut *a.advice;
    let caps: *const ce_capabilities = &*a.caps;

    // An enabled composition target with an unknown blend mode.
    let mut options = a.options;
    options.compose.enabled = 1;
    options.compose.blend = 200;
    let fr = frame(a.layers.as_ptr(), 1);
    // SAFETY: all pointers valid; the blend value is rejected.
    let rc = unsafe {
        ce_advise(
            caps,
            null(),
            0,
            as_c(&fr),
            as_c(&options),
            None,
            null_mut(),
            adv,
            &mut inf,
        )
    };
    assert_eq!(rc, CE_ERR_INVALID_ARGUMENT, "unknown compose blend mode");

    // A layer with an unknown blend mode.
    let mut bad = layer();
    bad.blend = 200;
    let bad_layers = [bad];
    let fr = frame(bad_layers.as_ptr(), 1);
    // SAFETY: valid pointers throughout.
    let rc = unsafe {
        ce_advise(
            caps,
            null(),
            0,
            as_c(&fr),
            as_c(&a.options),
            None,
            null_mut(),
            adv,
            &mut inf,
        )
    };
    assert_eq!(rc, CE_ERR_INVALID_ARGUMENT);
    assert!(advice_untouched(&a.advice));
}

// ---- ce_explain_layer --------------------------------------------------------

#[test]
fn explain_layer_null_and_invalid() {
    let caps: Box<ce_capabilities> = Box::new(zero());
    let lay = layer();
    let lay_p: *const ce_layer = as_c(&lay);
    let buf: ce_buffer_info = zero();
    let mut reasons = [0xAAAA_AAAAu32; 16];
    let mut inf: ce_error_info = zero();
    let r = reasons.as_mut_ptr();
    let caps_p: *const ce_capabilities = &*caps;
    // SAFETY: one required pointer is null per call; the rest are valid, and
    // `reasons` holds CE_MAX_PLANES (16) values.
    let calls = unsafe {
        [
            ce_explain_layer(null(), 0, lay_p, &buf, r, &mut inf),
            ce_explain_layer(caps_p, 0, null(), &buf, r, &mut inf),
            ce_explain_layer(caps_p, 0, lay_p, null(), r, &mut inf),
            ce_explain_layer(caps_p, 0, lay_p, &buf, null_mut(), &mut inf),
            // `info` is optional.
            ce_explain_layer(null(), 0, lay_p, &buf, r, null_mut()),
        ]
    };
    for rc in calls {
        assert_eq!(rc, CE_ERR_INVALID_ARGUMENT);
    }
    assert_eq!(status_of(&inf), CE_ERR_INVALID_ARGUMENT);

    let mut bad = layer();
    bad.blend = 200;
    // SAFETY: valid pointers; the layer's blend mode is rejected.
    let rc = unsafe { ce_explain_layer(caps_p, 0, as_c(&bad), &buf, r, &mut inf) };
    assert_eq!(rc, CE_ERR_INVALID_ARGUMENT);

    // An output the caps do not have.
    // SAFETY: valid pointers.
    let rc = unsafe { ce_explain_layer(caps_p, 99, lay_p, &buf, r, &mut inf) };
    assert_eq!(rc, CE_ERR_UNKNOWN_OUTPUT);
    assert!(
        reasons.iter().all(|v| *v == 0xAAAA_AAAA),
        "reasons untouched on failure"
    );
}

// ---- ce_abi_check ------------------------------------------------------------

#[test]
fn abi_checks_reject_zero_sizes() {
    assert!(!ce_abi_check(0, 0, 0, 0, 0, 0, 0));
    assert!(!ce_abi_check_advise(0, 0, 0, 0, 0));
}
