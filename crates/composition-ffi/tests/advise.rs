//! Tests of the stateless advisory C ABI, `ce_advise`, `ce_explain_layer`
//! and `ce_abi_check_advise`, called from Rust as a C program would call
//! them: null and short arguments, too many layers, check callback refusal
//! and give-up, determinism, struct sizes, and that a failed call never
//! writes the advice.

// Reason: this file calls the `extern "C"` API, whose functions are `unsafe`,
// and builds the C-layout structures a C caller would pass.
#![allow(unsafe_code)]

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr::{null, null_mut};

use composition_ffi::*;

// ---- C-layout mirrors of the header's structures -------------------------
// The crate's own structures have private fields; `abi_check_advise_accepts
// _the_mirror_sizes` ties these to them.

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Rect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
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
#[derive(Clone, Copy, Default, PartialEq, Debug)]
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
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct ErrorInfo {
    status: i32,
    layer: u32,
    reasons: u32,
    buffer: u32,
    platform: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ComposeTarget {
    modifier: u64,
    format: u32,
    blend: u8,
    enabled: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AdviseOptions {
    max_steps: u32,
    max_backend_checks: u32,
    max_candidates: u32,
    compose: ComposeTarget,
}

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct LayerAdvice {
    assignment: Assignment,
    reasons: u32,
    placement: u8,
    blocker: u8,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct Advice {
    layers: [LayerAdvice; 16],
    compose: Assignment,
    layer_count: u32,
    compose_low: u32,
    compose_high: u32,
    has_compose: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BufferEntry {
    buffer: u32,
    info: BufferInfo,
}

type Check = unsafe extern "C" fn(*mut c_void, *const Frame, *const Assignment, usize) -> i32;
/// The same callback as the library declares it.
type LibCheck =
    unsafe extern "C" fn(*mut c_void, *const ce_frame, *const ce_assignment, usize) -> i32;

// ---- header constants ------------------------------------------------------

const fn fourcc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    a as u32 | (b as u32) << 8 | (c as u32) << 16 | (d as u32) << 24
}
const XRGB8888: u32 = fourcc(b'X', b'R', b'2', b'4');
const ARGB8888: u32 = fourcc(b'A', b'R', b'2', b'4');
const NV12: u32 = fourcc(b'N', b'V', b'1', b'2');
const ALPHA_OPAQUE: u16 = 0xFFFF;
const BLEND_OPAQUE: u8 = 0;
const BLEND_PREMULTIPLIED: u8 = 1;
const REJECT_FORMAT: u32 = 1 << 1;

const OUT: u32 = 7;
const PRIMARY: u32 = 31;
const OVERLAY: u32 = 41;

/// Every status `ce_advise` and `ce_explain_layer` may return.
const DOCUMENTED: &[i32] = &[
    CE_OK,
    CE_ERR_INVALID_ARGUMENT,
    CE_ERR_UNKNOWN_OUTPUT,
    CE_ERR_TOO_MANY_LAYERS,
    CE_ERR_UNKNOWN_BUFFER,
    CE_ERR_INVALID_SOURCE,
    CE_ERR_INVALID_DESTINATION,
    CE_ERR_NO_COMPOSE_PLANE,
    CE_ERR_BACKEND_REJECTED,
    CE_ERR_SEARCH_LIMIT,
    CE_ERR_BACKEND_BUSY,
    CE_ERR_DEVICE_LOST,
    CE_ERR_BACKEND_PLATFORM,
    CE_ERR_BACKEND_UNKNOWN_BUFFER,
];

// ---- fixtures --------------------------------------------------------------

fn plane(id: u32, kind: u8, format: u32, zpos: i32, blend_modes: u8) -> PlaneCaps {
    // SAFETY: every field of `PlaneCaps` is a plain integer.
    let mut plane: PlaneCaps = unsafe { core::mem::zeroed() };
    plane.id = id;
    plane.kind = kind;
    plane.outputs = 1;
    plane.format_count = 1;
    plane.formats[0] = Format {
        modifier: 0,
        format,
    };
    plane.zpos_min = zpos;
    plane.blend_modes = blend_modes;
    plane.transforms = 1;
    plane
}

/// One 640x480 output, an XRGB8888 primary at zpos 0 and an ARGB8888
/// overlay at zpos 1, as `tests/c/event_order.c` of the static library.
fn caps() -> Box<Capabilities> {
    // SAFETY: every field of `Capabilities` is a plain integer.
    let mut caps: Box<Capabilities> = Box::new(unsafe { core::mem::zeroed() });
    caps.output_count = 1;
    caps.outputs[0] = OutputCaps {
        id: OUT,
        width: 640,
        height: 480,
        refresh_mhz: 60_000,
    };
    caps.plane_count = 2;
    caps.planes[0] = plane(PRIMARY, 0, XRGB8888, 0, 1 << BLEND_OPAQUE);
    caps.planes[1] = plane(
        OVERLAY,
        1,
        ARGB8888,
        1,
        (1 << BLEND_OPAQUE) | (1 << BLEND_PREMULTIPLIED),
    );
    caps
}

fn entry(buffer: u32, width: u32, height: u32, format: u32) -> BufferEntry {
    BufferEntry {
        buffer,
        info: BufferInfo {
            modifier: 0,
            width,
            height,
            format,
        },
    }
}

/// Buffer 1: XRGB8888 640x480; 2 and 3: ARGB8888 100x100; 4: NV12 64x64.
fn buffers() -> Vec<BufferEntry> {
    vec![
        entry(1, 640, 480, XRGB8888),
        entry(2, 100, 100, ARGB8888),
        entry(3, 100, 100, ARGB8888),
        entry(4, 64, 64, NV12),
    ]
}

fn layer(buffer: u32, width: u32, height: u32, x: i32, z_order: i32, blend: u8) -> Layer {
    Layer {
        buffer,
        source: Rect {
            x: 0,
            y: 0,
            width,
            height,
        },
        destination: Rect {
            x,
            y: 0,
            width,
            height,
        },
        z_order,
        alpha: ALPHA_OPAQUE,
        blend,
        transform: 0,
    }
}

fn background() -> Layer {
    layer(1, 640, 480, 0, 0, BLEND_OPAQUE)
}

fn sprite(buffer: u32, z_order: i32) -> Layer {
    layer(buffer, 100, 100, 10, z_order, BLEND_PREMULTIPLIED)
}

fn frame(layers: &[Layer]) -> Frame {
    Frame {
        layers: layers.as_ptr(),
        layer_count: layers.len(),
        output: OUT,
    }
}

fn options(compose: bool) -> AdviseOptions {
    AdviseOptions {
        compose: ComposeTarget {
            modifier: 0,
            format: XRGB8888,
            blend: BLEND_OPAQUE,
            enabled: compose as u8,
        },
        ..AdviseOptions::default()
    }
}

/// An advice every byte of which is 0xA5: a write shows.
fn sentinel() -> Box<Advice> {
    // SAFETY: every field of `Advice` is a plain integer.
    Box::new(unsafe {
        core::mem::transmute::<[u8; size_of::<Advice>()], Advice>([0xA5; size_of::<Advice>()])
    })
}

fn is_sentinel(advice: &Advice) -> bool {
    *advice == *sentinel()
}

struct Call<'a> {
    caps: *const Capabilities,
    buffers: &'a [BufferEntry],
    frame: *const Frame,
    options: *const AdviseOptions,
    check: Option<Check>,
    ctx: *mut c_void,
}

/// Calls `ce_advise` and returns the status, the advice (a sentinel if it was
/// not written) and the error details.
fn advise(call: &Call<'_>) -> (i32, Box<Advice>, ErrorInfo) {
    let mut advice = sentinel();
    let mut info = ErrorInfo {
        status: 12345,
        ..ErrorInfo::default()
    };
    // SAFETY: the mirrors have the header's layout; every pointer is valid
    // or null as the function requires.
    let status = unsafe {
        ce_advise(
            call.caps.cast(),
            call.buffers.as_ptr().cast(),
            call.buffers.len(),
            call.frame.cast(),
            call.options.cast(),
            core::mem::transmute::<Option<Check>, Option<LibCheck>>(call.check),
            call.ctx,
            (&raw mut *advice).cast(),
            (&raw mut info).cast(),
        )
    };
    assert!(DOCUMENTED.contains(&status), "undocumented status {status}");
    assert_eq!(info.status, status, "info.status differs from the result");
    if status != CE_OK {
        assert!(is_sentinel(&advice), "a failed call wrote the advice");
    }
    (status, advice, info)
}

fn simple<'a>(
    caps: &'a Capabilities,
    buffers: &'a [BufferEntry],
    frame: &'a Frame,
    options: &'a AdviseOptions,
) -> Call<'a> {
    Call {
        caps,
        buffers,
        frame,
        options,
        check: None,
        ctx: null_mut(),
    }
}

/// What a scripted check callback returns and records.
struct Script {
    /// Returned for an assignment set that uses the overlay plane.
    overlay_rc: i32,
    /// Returned otherwise.
    other_rc: i32,
    calls: usize,
    /// Calls whose assignments named `CE_COMPOSE_LAYER`.
    composed_calls: usize,
    /// The frame pointer of the last call.
    frame: *const Frame,
}

impl Script {
    fn new(overlay_rc: i32, other_rc: i32) -> Self {
        Script {
            overlay_rc,
            other_rc,
            calls: 0,
            composed_calls: 0,
            frame: null(),
        }
    }
}

unsafe extern "C" fn script_check(
    ctx: *mut c_void,
    frame: *const Frame,
    assignments: *const Assignment,
    count: usize,
) -> i32 {
    // SAFETY: `ctx` is the `Script` the test passed; the library hands over
    // `count` valid assignments.
    let (script, assignments) = unsafe {
        (
            &mut *ctx.cast::<Script>(),
            core::slice::from_raw_parts(assignments, count),
        )
    };
    script.calls += 1;
    script.frame = frame;
    if assignments.iter().any(|a| a.layer == CE_COMPOSE_LAYER) {
        script.composed_calls += 1;
    }
    if assignments.iter().any(|a| a.plane == OVERLAY) {
        script.overlay_rc
    } else {
        script.other_rc
    }
}

// ---- tests -----------------------------------------------------------------

#[test]
fn abi_check_advise_accepts_the_mirror_sizes() {
    let sizes = [
        size_of::<ComposeTarget>(),
        size_of::<AdviseOptions>(),
        size_of::<LayerAdvice>(),
        size_of::<Advice>(),
        size_of::<BufferEntry>(),
    ];
    assert!(ce_abi_check_advise(
        sizes[0], sizes[1], sizes[2], sizes[3], sizes[4]
    ));
    for wrong in 0..sizes.len() {
        let mut off = sizes;
        off[wrong] += 1;
        assert!(
            !ce_abi_check_advise(off[0], off[1], off[2], off[3], off[4]),
            "a wrong size of argument {wrong} passed"
        );
    }
    // The pinned footprint of the advice a C caller holds.
    assert_eq!(size_of::<LayerAdvice>(), 56);
    assert_eq!(size_of::<Advice>(), 16 * 56 + 48 + 16);
}

#[test]
fn direct_advice_puts_every_layer_on_a_plane() {
    let caps = caps();
    let buffers = buffers();
    let layers = [background(), sprite(2, 1)];
    let frame = frame(&layers);
    let options = options(true);
    let (status, advice, _) = advise(&simple(&caps, &buffers, &frame, &options));
    assert_eq!(status, CE_OK);
    assert_eq!(advice.layer_count, 2);
    assert_eq!(advice.has_compose, 0);
    for (index, plane) in [(0, PRIMARY), (1, OVERLAY)] {
        let entry = advice.layers[index];
        assert_eq!(entry.placement, CE_PLACE_PLANE);
        assert_eq!(entry.blocker, CE_BLOCKER_NONE);
        assert_eq!(entry.assignment.layer, index as u32);
        assert_eq!(entry.assignment.plane, plane);
    }
    assert_eq!(advice.layers[1].assignment.zpos, 1);
}

#[test]
fn composed_advice_names_the_run_and_its_plane() {
    let caps = caps();
    let buffers = buffers();
    let layers = [background(), sprite(2, 1), sprite(3, 2)];
    let frame = frame(&layers);
    let options = options(true);
    let (status, advice, _) = advise(&simple(&caps, &buffers, &frame, &options));
    assert_eq!(status, CE_OK);
    assert_eq!(advice.layer_count, 3);
    assert_eq!(advice.has_compose, 1);
    assert_eq!((advice.compose_low, advice.compose_high), (0, 1));
    assert_eq!(advice.compose.layer, CE_COMPOSE_LAYER);
    assert_eq!(advice.compose.plane, PRIMARY);
    assert_eq!(
        advice.compose.destination,
        Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480
        }
    );
    for index in 0..2 {
        assert_eq!(advice.layers[index].placement, CE_PLACE_COMPOSE);
        assert_eq!(advice.layers[index].blocker, CE_BLOCKER_COMBINATION);
        assert_eq!(advice.layers[index].assignment, Assignment::default());
    }
    assert_eq!(advice.layers[2].placement, CE_PLACE_PLANE);
    assert_eq!(advice.layers[2].assignment.plane, OVERLAY);
    assert_eq!(advice.layers[2].assignment.layer, 2);
}

#[test]
fn static_blocker_carries_the_common_reasons() {
    let caps = caps();
    let buffers = buffers();
    let layers = [background(), layer(4, 64, 64, 0, 1, BLEND_OPAQUE)];
    let frame = frame(&layers);
    let options = options(true);
    let (status, advice, _) = advise(&simple(&caps, &buffers, &frame, &options));
    assert_eq!(status, CE_OK);
    let video = advice.layers[1];
    assert_eq!(video.placement, CE_PLACE_COMPOSE);
    assert_eq!(video.blocker, CE_BLOCKER_STATIC);
    assert_ne!(video.reasons & REJECT_FORMAT, 0);
}

#[test]
fn hidden_layer_needs_neither_plane_nor_composition() {
    let caps = caps();
    let buffers = buffers();
    let layers = [
        background(),
        layer(2, 100, 100, 5000, 1, BLEND_PREMULTIPLIED),
    ];
    let frame = frame(&layers);
    let options = options(true);
    let (status, advice, _) = advise(&simple(&caps, &buffers, &frame, &options));
    assert_eq!(status, CE_OK);
    assert_eq!(advice.layers[1].placement, CE_PLACE_HIDDEN);
    assert_eq!(advice.layers[1].blocker, CE_BLOCKER_NONE);
    assert_eq!(advice.has_compose, 0);
}

#[test]
fn composition_disabled_reports_every_layer_composed() {
    let caps = caps();
    let buffers = buffers();
    let layers = [background(), sprite(2, 1), sprite(3, 2)];
    let frame = frame(&layers);
    let options = options(false);
    let (status, advice, _) = advise(&simple(&caps, &buffers, &frame, &options));
    assert_eq!(status, CE_OK);
    assert_eq!(advice.has_compose, 0);
    assert!(advice.layers[..3]
        .iter()
        .all(|l| l.placement == CE_PLACE_COMPOSE && l.blocker != CE_BLOCKER_NONE));
}

#[test]
fn null_arguments_are_rejected_without_writing() {
    let caps = caps();
    let buffers = buffers();
    let layers = [background()];
    let frame = frame(&layers);
    let options = options(true);
    let good = simple(&caps, &buffers, &frame, &options);
    for call in [
        Call {
            caps: null(),
            ..simple(&caps, &buffers, &frame, &options)
        },
        Call {
            frame: null(),
            ..simple(&caps, &buffers, &frame, &options)
        },
        Call {
            options: null(),
            ..simple(&caps, &buffers, &frame, &options)
        },
    ] {
        assert_eq!(advise(&call).0, CE_ERR_INVALID_ARGUMENT);
    }
    // A null advice: nothing to write to, and info still says why.
    let mut info = ErrorInfo::default();
    // SAFETY: valid pointers or null as the function requires.
    let status = unsafe {
        ce_advise(
            good.caps.cast(),
            buffers.as_ptr().cast(),
            buffers.len(),
            good.frame.cast(),
            good.options.cast(),
            None,
            null_mut(),
            null_mut(),
            (&raw mut info).cast(),
        )
    };
    assert_eq!(status, CE_ERR_INVALID_ARGUMENT);
    assert_eq!(info.status, CE_ERR_INVALID_ARGUMENT);
    // A null info is allowed.
    let mut advice = sentinel();
    // SAFETY: as above.
    let status = unsafe {
        ce_advise(
            good.caps.cast(),
            buffers.as_ptr().cast(),
            buffers.len(),
            good.frame.cast(),
            good.options.cast(),
            None,
            null_mut(),
            (&raw mut *advice).cast(),
            null_mut(),
        )
    };
    assert_eq!(status, CE_OK);
    assert_eq!(advice.layer_count, 1);
}

#[test]
fn null_tables_with_a_count_are_rejected() {
    let caps = caps();
    let layers = [background()];
    let options = options(true);
    let mut advice = sentinel();
    let frame_ok = frame(&layers);
    // SAFETY: a null buffer table with a nonzero count is what is tested.
    let status = unsafe {
        ce_advise(
            (&raw const *caps).cast(),
            null(),
            3,
            (&raw const frame_ok).cast(),
            (&raw const options).cast(),
            None,
            null_mut(),
            (&raw mut *advice).cast(),
            null_mut(),
        )
    };
    assert_eq!(status, CE_ERR_INVALID_ARGUMENT);
    assert!(is_sentinel(&advice));

    let buffers = buffers();
    let null_layers = Frame {
        layers: null(),
        layer_count: 1,
        output: OUT,
    };
    let (status, _, _) = advise(&simple(&caps, &buffers, &null_layers, &options));
    assert_eq!(status, CE_ERR_INVALID_ARGUMENT);

    // No layers at all is fine and blanks the output.
    let empty = Frame {
        layers: null(),
        layer_count: 0,
        output: OUT,
    };
    let (status, advice, _) = advise(&simple(&caps, &buffers, &empty, &options));
    assert_eq!(status, CE_OK);
    assert_eq!(advice.layer_count, 0);
    assert_eq!(advice.has_compose, 0);
}

#[test]
fn too_many_layers_are_rejected_without_writing() {
    let caps = caps();
    let buffers = buffers();
    let layers = [background(); 17];
    let frame = frame(&layers);
    let options = options(true);
    let (status, _, _) = advise(&simple(&caps, &buffers, &frame, &options));
    assert_eq!(status, CE_ERR_TOO_MANY_LAYERS);
    // A count far beyond the array is caught before anything is read.
    let huge = Frame {
        layers: layers.as_ptr(),
        layer_count: usize::MAX,
        output: OUT,
    };
    let (status, _, _) = advise(&simple(&caps, &buffers, &huge, &options));
    assert_eq!(status, CE_ERR_TOO_MANY_LAYERS);
}

#[test]
fn short_and_malformed_input_maps_to_its_status() {
    let caps = caps();
    let buffers = buffers();
    let options = options(true);

    // A buffer missing from a short table.
    let layers = [background(), sprite(3, 1)];
    let frame_short = frame(&layers);
    let (status, _, info) = advise(&simple(&caps, &buffers[..2], &frame_short, &options));
    assert_eq!(status, CE_ERR_UNKNOWN_BUFFER);
    assert_eq!((info.layer, info.buffer), (1, 3));
    let (status, _, _) = advise(&simple(&caps, &[], &frame_short, &options));
    assert_eq!(status, CE_ERR_UNKNOWN_BUFFER);

    // An unknown blend or transform.
    let mut bad = [background(), sprite(2, 1)];
    bad[1].blend = 9;
    let (status, _, info) = advise(&simple(&caps, &buffers, &frame(&bad), &options));
    assert_eq!(status, CE_ERR_INVALID_ARGUMENT);
    assert_eq!(info.layer, 1);
    bad[1].blend = BLEND_PREMULTIPLIED;
    bad[1].transform = 200;
    let (status, _, _) = advise(&simple(&caps, &buffers, &frame(&bad), &options));
    assert_eq!(status, CE_ERR_INVALID_ARGUMENT);

    // An unknown blend of the composition target.
    let mut bad_target = options;
    bad_target.compose.blend = 7;
    let layers = [background()];
    let one = frame(&layers);
    let (status, _, _) = advise(&simple(&caps, &buffers, &one, &bad_target));
    assert_eq!(status, CE_ERR_INVALID_ARGUMENT);

    // Unknown output, bad source, empty destination.
    let other = Frame { output: 99, ..one };
    let (status, _, _) = advise(&simple(&caps, &buffers, &other, &options));
    assert_eq!(status, CE_ERR_UNKNOWN_OUTPUT);
    let mut source = [background()];
    source[0].source.width = 641;
    let (status, _, info) = advise(&simple(&caps, &buffers, &frame(&source), &options));
    assert_eq!((status, info.layer), (CE_ERR_INVALID_SOURCE, 0));
    let mut destination = [background()];
    destination[0].destination.height = 0;
    let (status, _, _) = advise(&simple(&caps, &buffers, &frame(&destination), &options));
    assert_eq!(status, CE_ERR_INVALID_DESTINATION);
}

#[test]
fn no_compose_plane_and_search_limit_map() {
    let caps = caps();
    let buffers = buffers();
    let layers = [background(), sprite(2, 1), sprite(3, 2)];
    let frame = frame(&layers);

    let mut nv12_target = options(true);
    nv12_target.compose.format = NV12;
    let (status, _, info) = advise(&simple(&caps, &buffers, &frame, &nv12_target));
    assert_eq!(status, CE_ERR_NO_COMPOSE_PLANE);
    assert_ne!(info.reasons & REJECT_FORMAT, 0);

    let mut one_candidate = options(true);
    one_candidate.max_candidates = 1;
    let (status, _, _) = advise(&simple(&caps, &buffers, &frame, &one_candidate));
    assert_eq!(status, CE_ERR_SEARCH_LIMIT);
}

#[test]
fn check_refusal_moves_layers_to_composition() {
    let caps = caps();
    let buffers = buffers();
    let layers = [background(), sprite(2, 1)];
    let frame = frame(&layers);
    let options = options(true);
    let mut script = Script::new(CE_ERR_BACKEND_REJECTED, CE_OK);
    let call = Call {
        check: Some(script_check),
        ctx: (&raw mut script).cast(),
        ..simple(&caps, &buffers, &frame, &options)
    };
    let (status, advice, _) = advise(&call);
    assert_eq!(status, CE_OK);
    assert_eq!(advice.has_compose, 1);
    assert_eq!(advice.compose.plane, PRIMARY);
    for index in 0..2 {
        assert_eq!(advice.layers[index].placement, CE_PLACE_COMPOSE);
        assert_eq!(advice.layers[index].blocker, CE_BLOCKER_BACKEND);
    }
    assert!(script.calls >= 2);
    assert!(script.composed_calls >= 1, "the target was never checked");
    assert_eq!(script.frame, &raw const frame, "check saw another frame");
}

#[test]
fn check_calls_stay_within_the_budget() {
    let caps = caps();
    let buffers = buffers();
    let layers = [background(), sprite(2, 1), sprite(3, 2)];
    let frame = frame(&layers);
    let mut options = options(true);
    options.max_backend_checks = 2;
    let mut script = Script::new(CE_ERR_BACKEND_REJECTED, CE_ERR_BACKEND_REJECTED);
    let call = Call {
        check: Some(script_check),
        ctx: (&raw mut script).cast(),
        ..simple(&caps, &buffers, &frame, &options)
    };
    let (status, _, _) = advise(&call);
    assert!(
        status == CE_ERR_SEARCH_LIMIT || status == CE_ERR_NO_COMPOSE_PLANE,
        "status {status}"
    );
    assert!(script.calls <= 2, "{} calls", script.calls);
}

#[test]
fn check_give_up_stops_with_its_error() {
    let caps = caps();
    let buffers = buffers();
    let layers = [background(), sprite(2, 1)];
    let frame = frame(&layers);
    let options = options(true);
    for (rc, expected) in [
        (CE_ERR_DEVICE_LOST, CE_ERR_DEVICE_LOST),
        (CE_ERR_BACKEND_BUSY, CE_ERR_BACKEND_BUSY),
        (CE_ERR_BACKEND_UNKNOWN_BUFFER, CE_ERR_BACKEND_UNKNOWN_BUFFER),
        (-1234, CE_ERR_BACKEND_PLATFORM),
    ] {
        let mut script = Script::new(rc, rc);
        let call = Call {
            check: Some(script_check),
            ctx: (&raw mut script).cast(),
            ..simple(&caps, &buffers, &frame, &options)
        };
        let (status, _, info) = advise(&call);
        assert_eq!(status, expected, "check returned {rc}");
        assert_eq!(script.calls, 1, "the search went on after giving up");
        if expected == CE_ERR_BACKEND_PLATFORM {
            assert_eq!(info.platform, rc);
        }
        if expected == CE_ERR_BACKEND_UNKNOWN_BUFFER {
            assert_eq!(info.buffer, 1);
        }
    }
}

#[test]
fn advice_is_deterministic() {
    let caps = caps();
    let buffers = buffers();
    let layers = [
        background(),
        sprite(2, 1),
        layer(4, 64, 64, 0, 2, BLEND_OPAQUE),
        sprite(3, 3),
    ];
    let frame = frame(&layers);
    let options = options(true);
    let (status, first, _) = advise(&simple(&caps, &buffers, &frame, &options));
    assert_eq!(status, CE_OK);
    for _ in 0..8 {
        let (status, again, _) = advise(&simple(&caps, &buffers, &frame, &options));
        assert_eq!(status, CE_OK);
        assert_eq!(*again, *first);
    }
}

#[test]
fn explain_layer_reports_per_plane_reasons() {
    let caps = caps();
    let video = layer(4, 64, 64, 0, 1, BLEND_OPAQUE);
    let nv12 = buffers()[3].info;
    let mut reasons = [0xFFFF_FFFFu32; 16];
    let mut info = ErrorInfo::default();
    // SAFETY: valid pointers as the function requires.
    let status = unsafe {
        ce_explain_layer(
            (&raw const *caps).cast(),
            OUT,
            (&raw const video).cast(),
            (&raw const nv12).cast(),
            reasons.as_mut_ptr(),
            (&raw mut info).cast(),
        )
    };
    assert_eq!(status, CE_OK);
    assert_ne!(reasons[0] & REJECT_FORMAT, 0);
    assert_ne!(reasons[1] & REJECT_FORMAT, 0);
    assert!(reasons[2..].iter().all(|r| *r == 0));

    let mut untouched = [0xFFFF_FFFFu32; 16];
    // SAFETY: a null `reasons` is what is tested.
    let status = unsafe {
        ce_explain_layer(
            (&raw const *caps).cast(),
            OUT,
            (&raw const video).cast(),
            (&raw const nv12).cast(),
            null_mut(),
            null_mut(),
        )
    };
    assert_eq!(status, CE_ERR_INVALID_ARGUMENT);
    // SAFETY: an unknown output is what is tested.
    let status = unsafe {
        ce_explain_layer(
            (&raw const *caps).cast(),
            99,
            (&raw const video).cast(),
            (&raw const nv12).cast(),
            untouched.as_mut_ptr(),
            null_mut(),
        )
    };
    assert_eq!(status, CE_ERR_UNKNOWN_OUTPUT);
    assert!(untouched.iter().all(|r| *r == 0xFFFF_FFFF));
}
