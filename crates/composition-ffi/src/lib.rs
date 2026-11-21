//! C ABI of the composition engine.
//!
//! The C side supplies a backend as a table of functions
//! ([`ce_backend_ops`]) and the storage for the engine; this crate wraps
//! [`composition_core::Engine`] around it. The declarations here must match
//! `include/composition_engine.h`, which is written by hand; `ce_abi_check`
//! lets a C program verify the structure sizes.
//!
//! This is the one crate that needs `unsafe`: it dereferences pointers handed
//! over by C. Each function states what it requires of them.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unsafe_code)]
#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use core::ffi::c_void;
use core::mem::{align_of, size_of, MaybeUninit};

use composition_core::limits::{
    MAX_FORMATS, MAX_LAYERS, MAX_OUTPUTS, MAX_PLANES, MAX_RESOURCE_GROUPS,
};
use composition_core::{
    BackendError, BackendEvent, BlendMode, BufferId, BufferInfo, BufferState, Capabilities,
    DisplayBackend, DisplayEvent, Engine, Error, FormatModifier, Frame, Layer, LayerTraits,
    Modifier, OutputCaps, OutputId, PixelFormat, Plan, PlanError, PlaneCaps, PlaneId, PlaneKind,
    PresentError, Rect, ResourceGroup, ScaleLimits, Size, SubmissionId, Transform, Zpos,
};

#[cfg(all(not(feature = "std"), feature = "panic-handler"))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}

/// Status code: zero for success, negative otherwise.
pub type ce_status = i32;

/// Success.
pub const CE_OK: ce_status = 0;
/// A pointer was null or a value out of range.
pub const CE_ERR_INVALID_ARGUMENT: ce_status = -1;
/// The backend has no such output.
pub const CE_ERR_UNKNOWN_OUTPUT: ce_status = -10;
/// More layers than the engine supports.
pub const CE_ERR_TOO_MANY_LAYERS: ce_status = -11;
/// The buffer is not registered.
pub const CE_ERR_UNKNOWN_BUFFER: ce_status = -12;
/// A source rectangle is empty or outside its buffer.
pub const CE_ERR_INVALID_SOURCE: ce_status = -13;
/// A destination rectangle is empty or not on the output.
pub const CE_ERR_INVALID_DESTINATION: ce_status = -14;
/// No legal plane assignment exists.
pub const CE_ERR_NO_PLANE: ce_status = -15;
/// The backend rejected every assignment tried.
pub const CE_ERR_BACKEND_REJECTED: ce_status = -16;
/// The planning search hit its limits.
pub const CE_ERR_SEARCH_LIMIT: ce_status = -17;
/// A layer reaches beyond the output and cannot be clipped exactly.
pub const CE_ERR_CANNOT_CLIP: ce_status = -18;
/// The buffer identifier is already registered.
pub const CE_ERR_DUPLICATE_BUFFER: ce_status = -30;
/// No room for another buffer.
pub const CE_ERR_BUFFER_TABLE_FULL: ce_status = -31;
/// The buffer is submitted or on screen.
pub const CE_ERR_BUFFER_IN_USE: ce_status = -32;
/// The buffer is still being drawn.
pub const CE_ERR_BUFFER_NOT_READY: ce_status = -33;
/// The buffer is not being drawn.
pub const CE_ERR_BUFFER_NOT_WRITABLE: ce_status = -34;
/// A submission is still waiting to be latched.
pub const CE_ERR_BUSY: ce_status = -35;
/// More outputs in use than the engine tracks.
pub const CE_ERR_TOO_MANY_OUTPUTS: ce_status = -36;
/// More buffers in a submission than the engine tracks.
pub const CE_ERR_TOO_MANY_BUFFERS: ce_status = -37;
/// The event queue is full; poll events and retry.
pub const CE_ERR_EVENT_QUEUE_FULL: ce_status = -38;
/// The backend reported a latch that matches no pending submission.
pub const CE_ERR_UNEXPECTED_LATCH: ce_status = -39;
/// The backend cannot take a commit yet.
pub const CE_ERR_BACKEND_BUSY: ce_status = -50;
/// The display resources are gone.
pub const CE_ERR_DEVICE_LOST: ce_status = -51;
/// A platform error; the code is in `ce_error_info.platform`.
pub const CE_ERR_BACKEND_PLATFORM: ce_status = -52;
/// The backend does not know the buffer.
pub const CE_ERR_BACKEND_UNKNOWN_BUFFER: ce_status = -53;

/// Bytes of storage the header reserves for one engine.
pub const CE_ENGINE_SIZE: usize = 20480;
/// Alignment the header requires for the engine storage.
pub const CE_ENGINE_ALIGN: usize = 8;

const _: () = assert!(size_of::<Engine<CBackend>>() <= CE_ENGINE_SIZE);
const _: () = assert!(align_of::<Engine<CBackend>>() <= CE_ENGINE_ALIGN);

/// See `ce_rect` in the header.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ce_rect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// See `ce_buffer_info` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_buffer_info {
    modifier: u64,
    width: u32,
    height: u32,
    format: u32,
}

/// See `ce_layer` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_layer {
    buffer: u32,
    source: ce_rect,
    destination: ce_rect,
    z_order: i32,
    alpha: u16,
    blend: u8,
    transform: u8,
}

/// See `ce_frame` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_frame {
    layers: *const ce_layer,
    layer_count: usize,
    output: u32,
}

/// See `ce_assignment` in the header.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ce_assignment {
    layer: u32,
    plane: u32,
    plane_index: u32,
    zpos: i32,
    source: ce_rect,
    destination: ce_rect,
}

/// See `ce_format` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_format {
    modifier: u64,
    format: u32,
}

/// See `ce_plane_caps` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_plane_caps {
    formats: [ce_format; MAX_FORMATS],
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

/// See `ce_output_caps` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_output_caps {
    id: u32,
    width: u32,
    height: u32,
    refresh_mhz: u32,
}

/// See `ce_resource_group` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_resource_group {
    planes: u32,
    consumers: u8,
    capacity: u8,
}

/// See `ce_capabilities` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_capabilities {
    planes: [ce_plane_caps; MAX_PLANES],
    outputs: [ce_output_caps; MAX_OUTPUTS],
    groups: [ce_resource_group; MAX_RESOURCE_GROUPS],
    plane_count: u32,
    output_count: u32,
    group_count: u32,
    replace_pending: u8,
}

/// See `ce_backend_event` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_backend_event {
    submission: u64,
    output: u32,
    kind: u8,
}

/// See `ce_backend_ops` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_backend_ops {
    capabilities: Option<unsafe extern "C" fn(*mut c_void, *mut ce_capabilities)>,
    check: Option<
        unsafe extern "C" fn(*mut c_void, *const ce_frame, *const ce_assignment, usize) -> i32,
    >,
    commit: Option<
        unsafe extern "C" fn(*mut c_void, u64, *const ce_frame, *const ce_assignment, usize) -> i32,
    >,
    poll_event: Option<unsafe extern "C" fn(*mut c_void, *mut ce_backend_event) -> bool>,
}

/// See `ce_event` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_event {
    submission: u64,
    output: u32,
    buffer: u32,
    kind: u8,
}

/// See `ce_error_info` in the header.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ce_error_info {
    status: ce_status,
    layer: u32,
    reasons: u32,
    buffer: u32,
    platform: i32,
}

/// A backend implemented by C functions.
pub struct CBackend {
    ops: ce_backend_ops,
    ctx: *mut c_void,
}

/// The engine as seen from C.
pub struct ce_engine(Engine<CBackend>);

fn blend_from(value: u8) -> Option<BlendMode> {
    Some(match value {
        0 => BlendMode::Opaque,
        1 => BlendMode::Premultiplied,
        2 => BlendMode::Coverage,
        _ => return None,
    })
}

fn transform_from(value: u8) -> Option<Transform> {
    Some(match value {
        0 => Transform::Normal,
        1 => Transform::Rotate90,
        2 => Transform::Rotate180,
        3 => Transform::Rotate270,
        4 => Transform::FlipH,
        5 => Transform::FlipV,
        _ => return None,
    })
}

fn rect_from(rect: ce_rect) -> Rect {
    Rect::new(rect.x, rect.y, rect.width, rect.height)
}

fn rect_to(rect: Rect) -> ce_rect {
    ce_rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

fn layer_to(layer: &Layer) -> ce_layer {
    ce_layer {
        buffer: layer.buffer.0,
        source: rect_to(layer.source),
        destination: rect_to(layer.destination),
        z_order: layer.z_order,
        alpha: layer.alpha,
        blend: layer.blend as u8,
        transform: layer.transform as u8,
    }
}

/// Converts what the C backend reported, dropping anything malformed.
/// Under-reporting is safe; inventing capabilities is not.
fn capabilities_from(raw: &ce_capabilities) -> Capabilities {
    let mut caps = Capabilities {
        replace_pending: raw.replace_pending != 0,
        ..Capabilities::default()
    };
    for output in raw.outputs.iter().take(raw.output_count as usize) {
        let _ = caps.outputs.push(OutputCaps {
            id: OutputId(output.id),
            size: Size::new(output.width, output.height),
            refresh_mhz: output.refresh_mhz,
        });
    }
    for plane in raw.planes.iter().take(raw.plane_count as usize) {
        let kind = match plane.kind {
            0 => PlaneKind::Primary,
            1 => PlaneKind::Overlay,
            2 => PlaneKind::Cursor,
            // Keep the slot so that plane indices stay aligned with the C
            // side, but make the plane unusable.
            _ => {
                let _ = caps.planes.push(PlaneCaps {
                    id: PlaneId(plane.id),
                    ..PlaneCaps::default()
                });
                continue;
            }
        };
        let mut converted = PlaneCaps {
            id: PlaneId(plane.id),
            kind,
            outputs: plane.outputs,
            scaling: (plane.can_scale != 0).then_some(ScaleLimits {
                max_upscale: plane.max_upscale,
                max_downscale: plane.max_downscale,
            }),
            max_source: (plane.max_source_width != 0 && plane.max_source_height != 0)
                .then_some(Size::new(plane.max_source_width, plane.max_source_height)),
            plane_alpha: plane.plane_alpha != 0,
            blend_modes: plane.blend_modes & 0b111,
            transforms: plane.transforms & 0b11_1111,
            zpos: if plane.zpos_mutable != 0 {
                Zpos::Range {
                    min: plane.zpos_min,
                    max: plane.zpos_max,
                }
            } else {
                Zpos::Fixed(plane.zpos_min)
            },
            full_output_only: plane.full_output_only != 0,
            ..PlaneCaps::default()
        };
        for format in plane.formats.iter().take(plane.format_count as usize) {
            let _ = converted.formats.push(FormatModifier {
                format: PixelFormat(format.format),
                modifier: Modifier(format.modifier),
            });
        }
        let _ = caps.planes.push(converted);
    }
    for group in raw.groups.iter().take(raw.group_count as usize) {
        let _ = caps.resource_groups.push(ResourceGroup {
            consumers: LayerTraits(group.consumers & 0b1111),
            planes: group.planes,
            capacity: group.capacity,
        });
    }
    caps
}

fn backend_error(code: i32) -> BackendError {
    match code {
        CE_ERR_BACKEND_REJECTED => BackendError::Rejected,
        CE_ERR_BACKEND_BUSY => BackendError::Busy,
        CE_ERR_DEVICE_LOST => BackendError::DeviceLost,
        other => BackendError::Platform(other),
    }
}

impl CBackend {
    /// Runs `call` with the frame and plan in their C form.
    fn with_c_frame(
        &mut self,
        frame: &Frame<'_>,
        plan: &Plan,
        call: impl FnOnce(&ce_backend_ops, *mut c_void, &ce_frame, &[ce_assignment]) -> i32,
    ) -> Result<(), BackendError> {
        let mut layers = [MaybeUninit::<ce_layer>::uninit(); MAX_LAYERS];
        let count = frame.layers.len().min(MAX_LAYERS);
        for (slot, layer) in layers.iter_mut().zip(frame.layers) {
            slot.write(layer_to(layer));
        }
        let c_frame = ce_frame {
            layers: layers.as_ptr().cast(),
            layer_count: count,
            output: frame.output.0,
        };
        let mut assignments = [ce_assignment::default(); MAX_LAYERS];
        let assigned = plan.assignments();
        for (slot, assignment) in assignments.iter_mut().zip(assigned) {
            *slot = ce_assignment {
                layer: assignment.layer as u32,
                plane: assignment.plane.0,
                plane_index: assignment.plane_index as u32,
                zpos: assignment.zpos,
                source: rect_to(assignment.source),
                destination: rect_to(assignment.destination),
            };
        }
        let code = call(
            &self.ops,
            self.ctx,
            &c_frame,
            &assignments[..assigned.len().min(MAX_LAYERS)],
        );
        if code == CE_OK {
            return Ok(());
        }
        Err(match code {
            // The backend names no buffer; report the first one of the frame
            // that the caller can then inspect.
            CE_ERR_BACKEND_UNKNOWN_BUFFER => {
                BackendError::UnknownBuffer(frame.layers.first().map_or(BufferId(0), |l| l.buffer))
            }
            other => backend_error(other),
        })
    }
}

impl DisplayBackend for CBackend {
    fn capabilities(&self) -> Capabilities {
        let Some(capabilities) = self.ops.capabilities else {
            return Capabilities::default();
        };
        let mut raw = MaybeUninit::<ce_capabilities>::zeroed();
        // SAFETY: `raw` is valid for writes and zero is a valid value of
        // every field. The function and `ctx` were supplied together through
        // `ce_engine_init`, whose caller vouches for them.
        let raw = unsafe {
            capabilities(self.ctx, raw.as_mut_ptr());
            raw.assume_init_ref()
        };
        capabilities_from(raw)
    }

    fn check(&mut self, frame: &Frame<'_>, plan: &Plan) -> Result<(), BackendError> {
        self.with_c_frame(frame, plan, |ops, ctx, frame, assignments| {
            match ops.check {
                // SAFETY: the pointers refer to live locals for the duration of
                // the call; see `capabilities` for the function pointer.
                Some(check) => unsafe {
                    check(ctx, frame, assignments.as_ptr(), assignments.len())
                },
                None => CE_OK,
            }
        })
    }

    fn commit(
        &mut self,
        submission: SubmissionId,
        frame: &Frame<'_>,
        plan: &Plan,
    ) -> Result<(), BackendError> {
        self.with_c_frame(frame, plan, |ops, ctx, frame, assignments| {
            match ops.commit {
                // SAFETY: as in `check`.
                Some(commit) => unsafe {
                    commit(
                        ctx,
                        submission.0,
                        frame,
                        assignments.as_ptr(),
                        assignments.len(),
                    )
                },
                None => CE_ERR_DEVICE_LOST,
            }
        })
    }

    fn poll_event(&mut self) -> Option<BackendEvent> {
        let poll_event = self.ops.poll_event?;
        loop {
            let mut raw = MaybeUninit::<ce_backend_event>::zeroed();
            // SAFETY: `raw` is valid for writes and zero-initialised; see
            // `capabilities` for the function pointer.
            let raw = unsafe {
                if !poll_event(self.ctx, raw.as_mut_ptr()) {
                    return None;
                }
                raw.assume_init()
            };
            match raw.kind {
                0 => {
                    return Some(BackendEvent::Latched {
                        output: OutputId(raw.output),
                        submission: SubmissionId(raw.submission),
                    })
                }
                1 => return Some(BackendEvent::DeviceLost),
                2 => return Some(BackendEvent::CapabilitiesChanged),
                // Unknown kinds are skipped rather than guessed at.
                _ => continue,
            }
        }
    }
}

fn describe(error: Error) -> ce_error_info {
    let mut info = ce_error_info::default();
    info.status = match error {
        Error::Plan(plan) => match plan {
            PlanError::UnknownOutput(_) => CE_ERR_UNKNOWN_OUTPUT,
            PlanError::TooManyLayers { .. } => CE_ERR_TOO_MANY_LAYERS,
            PlanError::UnknownBuffer { layer, buffer } => {
                info.layer = layer as u32;
                info.buffer = buffer.0;
                CE_ERR_UNKNOWN_BUFFER
            }
            PlanError::InvalidSource { layer } => {
                info.layer = layer as u32;
                CE_ERR_INVALID_SOURCE
            }
            PlanError::InvalidDestination { layer } => {
                info.layer = layer as u32;
                CE_ERR_INVALID_DESTINATION
            }
            PlanError::CannotClip { layer } => {
                info.layer = layer as u32;
                CE_ERR_CANNOT_CLIP
            }
            PlanError::NoPlane { layer, reasons } => {
                info.layer = layer as u32;
                info.reasons = reasons.bits();
                CE_ERR_NO_PLANE
            }
            PlanError::BackendRejected { .. } | PlanError::Aborted => CE_ERR_BACKEND_REJECTED,
            PlanError::LimitExceeded => CE_ERR_SEARCH_LIMIT,
        },
        Error::Present(present) => match present {
            PresentError::UnknownBuffer(buffer) => {
                info.buffer = buffer.0;
                CE_ERR_UNKNOWN_BUFFER
            }
            PresentError::DuplicateBuffer(buffer) => {
                info.buffer = buffer.0;
                CE_ERR_DUPLICATE_BUFFER
            }
            PresentError::BufferTableFull => CE_ERR_BUFFER_TABLE_FULL,
            PresentError::BufferInUse(buffer) => {
                info.buffer = buffer.0;
                CE_ERR_BUFFER_IN_USE
            }
            PresentError::BufferNotReady(buffer) => {
                info.buffer = buffer.0;
                CE_ERR_BUFFER_NOT_READY
            }
            PresentError::BufferNotWritable(buffer) => {
                info.buffer = buffer.0;
                CE_ERR_BUFFER_NOT_WRITABLE
            }
            PresentError::Busy(_) => CE_ERR_BUSY,
            PresentError::TooManyOutputs => CE_ERR_TOO_MANY_OUTPUTS,
            PresentError::TooManyBuffers => CE_ERR_TOO_MANY_BUFFERS,
            PresentError::EventQueueFull => CE_ERR_EVENT_QUEUE_FULL,
            PresentError::UnexpectedLatch { .. } => CE_ERR_UNEXPECTED_LATCH,
        },
        Error::Backend(backend) => match backend {
            BackendError::Rejected => CE_ERR_BACKEND_REJECTED,
            BackendError::Busy => CE_ERR_BACKEND_BUSY,
            BackendError::UnknownBuffer(buffer) => {
                info.buffer = buffer.0;
                CE_ERR_BACKEND_UNKNOWN_BUFFER
            }
            BackendError::DeviceLost => CE_ERR_DEVICE_LOST,
            BackendError::Platform(code) => {
                info.platform = code;
                CE_ERR_BACKEND_PLATFORM
            }
        },
        // `ce_present` plans and commits in one step, so these cannot occur.
        Error::StalePlan | Error::PlanMismatch => CE_ERR_INVALID_ARGUMENT,
    };
    info
}

fn status(result: Result<(), Error>) -> ce_status {
    match result {
        Ok(()) => CE_OK,
        Err(error) => describe(error).status,
    }
}

/// Returns true if the caller's structure sizes match this library.
#[no_mangle]
pub extern "C" fn ce_abi_check(
    capabilities: usize,
    layer: usize,
    frame: usize,
    event: usize,
    backend_ops: usize,
    assignment: usize,
    engine_size: usize,
) -> bool {
    capabilities == size_of::<ce_capabilities>()
        && assignment == size_of::<ce_assignment>()
        && layer == size_of::<ce_layer>()
        && frame == size_of::<ce_frame>()
        && event == size_of::<ce_event>()
        && backend_ops == size_of::<ce_backend_ops>()
        && engine_size == CE_ENGINE_SIZE
}

/// Creates an engine in caller-provided storage.
///
/// # Safety
///
/// `storage` must be valid for writes of `size` bytes and stay untouched by
/// the caller afterwards. `ops` must point to a valid table whose functions
/// are safe to call with `ctx` for as long as the engine is used.
#[no_mangle]
pub unsafe extern "C" fn ce_engine_init(
    storage: *mut c_void,
    size: usize,
    ops: *const ce_backend_ops,
    ctx: *mut c_void,
) -> *mut ce_engine {
    if storage.is_null()
        || ops.is_null()
        || size < size_of::<ce_engine>()
        || (storage as usize) % align_of::<ce_engine>() != 0
    {
        return core::ptr::null_mut();
    }
    let engine = storage.cast::<ce_engine>();
    // SAFETY: `ops` is valid per the contract; `storage` is large enough,
    // aligned and writable as checked above and per the contract.
    unsafe {
        let backend = CBackend { ops: *ops, ctx };
        engine.write(ce_engine(Engine::new(backend)));
    }
    engine
}

/// Registers a buffer.
///
/// # Safety
///
/// `engine` must come from `ce_engine_init`; `info` must be valid or null.
#[no_mangle]
pub unsafe extern "C" fn ce_register_buffer(
    engine: *mut ce_engine,
    buffer: u32,
    info: *const ce_buffer_info,
) -> ce_status {
    // SAFETY: per the contract both are valid when not null.
    let (Some(engine), Some(info)) = (unsafe { engine.as_mut() }, unsafe { info.as_ref() }) else {
        return CE_ERR_INVALID_ARGUMENT;
    };
    status(engine.0.register_buffer(
        BufferId(buffer),
        BufferInfo {
            size: Size::new(info.width, info.height),
            format: PixelFormat(info.format),
            modifier: Modifier(info.modifier),
        },
    ))
}

/// Removes a buffer.
///
/// # Safety
///
/// `engine` must come from `ce_engine_init`.
#[no_mangle]
pub unsafe extern "C" fn ce_unregister_buffer(engine: *mut ce_engine, buffer: u32) -> ce_status {
    // SAFETY: per the contract.
    match unsafe { engine.as_mut() } {
        Some(engine) => status(engine.0.unregister_buffer(BufferId(buffer))),
        None => CE_ERR_INVALID_ARGUMENT,
    }
}

/// Hands an idle buffer to the producer.
///
/// # Safety
///
/// `engine` must come from `ce_engine_init`.
#[no_mangle]
pub unsafe extern "C" fn ce_begin_write(engine: *mut ce_engine, buffer: u32) -> ce_status {
    // SAFETY: per the contract.
    match unsafe { engine.as_mut() } {
        Some(engine) => status(engine.0.begin_write(BufferId(buffer))),
        None => CE_ERR_INVALID_ARGUMENT,
    }
}

/// Declares a buffer complete.
///
/// # Safety
///
/// `engine` must come from `ce_engine_init`.
#[no_mangle]
pub unsafe extern "C" fn ce_finish_write(engine: *mut ce_engine, buffer: u32) -> ce_status {
    // SAFETY: per the contract.
    match unsafe { engine.as_mut() } {
        Some(engine) => status(engine.0.finish_write(BufferId(buffer))),
        None => CE_ERR_INVALID_ARGUMENT,
    }
}

/// Returns the state of a buffer, or -1 if it is not registered.
///
/// # Safety
///
/// `engine` must come from `ce_engine_init`.
#[no_mangle]
pub unsafe extern "C" fn ce_buffer_state(engine: *const ce_engine, buffer: u32) -> i32 {
    // SAFETY: per the contract.
    let Some(engine) = (unsafe { engine.as_ref() }) else {
        return -1;
    };
    match engine.0.buffer_state(BufferId(buffer)) {
        None => -1,
        Some(BufferState::Writable) => 0,
        Some(BufferState::Ready) => 1,
        Some(BufferState::Submitted) => 2,
        Some(BufferState::Scanning) => 3,
    }
}

/// Plans and submits a frame.
///
/// # Safety
///
/// `engine` must come from `ce_engine_init`. `frame` must be valid, with
/// `layers` pointing to `layer_count` layers (it may be null if the count is
/// zero). `submission` and `info` must each be valid for writes or null.
#[no_mangle]
pub unsafe extern "C" fn ce_present(
    engine: *mut ce_engine,
    frame: *const ce_frame,
    submission: *mut u64,
    info: *mut ce_error_info,
) -> ce_status {
    let report = |details: ce_error_info| {
        // SAFETY: `info` is valid for writes or null per the contract.
        if let Some(info) = unsafe { info.as_mut() } {
            *info = details;
        }
        details.status
    };
    let invalid = ce_error_info {
        status: CE_ERR_INVALID_ARGUMENT,
        ..ce_error_info::default()
    };
    // SAFETY: per the contract both are valid when not null.
    let (Some(engine), Some(frame)) = (unsafe { engine.as_mut() }, unsafe { frame.as_ref() })
    else {
        return report(invalid);
    };
    if frame.layer_count > MAX_LAYERS {
        return report(ce_error_info {
            status: CE_ERR_TOO_MANY_LAYERS,
            ..ce_error_info::default()
        });
    }
    if frame.layer_count > 0 && frame.layers.is_null() {
        return report(invalid);
    }
    let mut layers = [Layer::new(BufferId(0), Rect::default(), Rect::default(), 0); MAX_LAYERS];
    for (index, slot) in layers.iter_mut().enumerate().take(frame.layer_count) {
        // SAFETY: `layers` points to `layer_count` layers per the contract.
        let raw = unsafe { *frame.layers.add(index) };
        let (Some(blend), Some(transform)) = (blend_from(raw.blend), transform_from(raw.transform))
        else {
            return report(ce_error_info {
                layer: index as u32,
                ..invalid
            });
        };
        *slot = Layer {
            buffer: BufferId(raw.buffer),
            source: rect_from(raw.source),
            destination: rect_from(raw.destination),
            z_order: raw.z_order,
            alpha: raw.alpha,
            blend,
            transform,
        };
    }
    let frame = Frame {
        output: OutputId(frame.output),
        layers: &layers[..frame.layer_count],
    };
    match engine.0.present(&frame) {
        Ok(id) => {
            // SAFETY: `submission` is valid for writes or null per the
            // contract.
            if let Some(submission) = unsafe { submission.as_mut() } {
                *submission = id.0;
            }
            report(ce_error_info::default())
        }
        Err(error) => report(describe(error)),
    }
}

/// Takes the next event: 1 with `*event` filled, 0 for none, negative on a
/// backend inconsistency.
///
/// # Safety
///
/// `engine` must come from `ce_engine_init`; `event` must be valid for
/// writes.
#[no_mangle]
pub unsafe extern "C" fn ce_poll_event(engine: *mut ce_engine, event: *mut ce_event) -> i32 {
    // SAFETY: per the contract both are valid when not null.
    let (Some(engine), Some(out)) = (unsafe { engine.as_mut() }, unsafe { event.as_mut() }) else {
        return CE_ERR_INVALID_ARGUMENT;
    };
    let event = match engine.0.poll_event() {
        Ok(Some(event)) => event,
        Ok(None) => return 0,
        Err(error) => return describe(error).status,
    };
    *out = ce_event {
        submission: 0,
        output: 0,
        buffer: 0,
        kind: 0,
    };
    match event {
        DisplayEvent::Presented { output, submission } => {
            out.kind = 0;
            out.output = output.0;
            out.submission = submission.0;
        }
        DisplayEvent::Dropped { output, submission } => {
            out.kind = 1;
            out.output = output.0;
            out.submission = submission.0;
        }
        DisplayEvent::Released { buffer } => {
            out.kind = 2;
            out.buffer = buffer.0;
        }
        DisplayEvent::DeviceLost => out.kind = 3,
        DisplayEvent::CapabilitiesChanged => out.kind = 4,
        DisplayEvent::LatchTimeout { output, submission } => {
            out.kind = 5;
            out.output = output.0;
            out.submission = submission.0;
        }
    }
    1
}

/// Sets the latch timeout in ticks; zero turns it off.
///
/// # Safety
///
/// `engine` must come from `ce_engine_init`.
#[no_mangle]
pub unsafe extern "C" fn ce_set_latch_timeout(engine: *mut ce_engine, ticks: u64) -> ce_status {
    // SAFETY: per the contract.
    match unsafe { engine.as_mut() } {
        Some(engine) => {
            engine.0.set_latch_timeout((ticks != 0).then_some(ticks));
            CE_OK
        }
        None => CE_ERR_INVALID_ARGUMENT,
    }
}

/// Tells the engine the current time in ticks.
///
/// # Safety
///
/// `engine` must come from `ce_engine_init`.
#[no_mangle]
pub unsafe extern "C" fn ce_advance_time(engine: *mut ce_engine, now: u64) -> ce_status {
    // SAFETY: per the contract.
    match unsafe { engine.as_mut() } {
        Some(engine) => {
            engine.0.advance_time(now);
            CE_OK
        }
        None => CE_ERR_INVALID_ARGUMENT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The header's storage size should cover the engine without being
    /// wasteful; this fails when the engine grows or shrinks a lot.
    #[test]
    fn engine_storage_constant_is_reasonable() {
        let actual = size_of::<ce_engine>();
        assert!(actual <= CE_ENGINE_SIZE, "engine needs {actual} bytes");
        assert!(
            actual * 2 > CE_ENGINE_SIZE,
            "engine needs only {actual} bytes, shrink CE_ENGINE_SIZE"
        );
    }
}
