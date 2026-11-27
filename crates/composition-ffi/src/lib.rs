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
    AdviceError, AdviseOptions, Assignment, BackendError, BackendEvent, BlendMode, Blocker,
    BufferId, BufferInfo, BufferLookup, BufferState, Capabilities, ComposeTarget, DisplayBackend,
    DisplayEvent, Engine, Error, FormatModifier, Frame, Layer, LayerTraits, Modifier, OutputCaps,
    OutputId, PixelFormat, Placement, Plan, PlanError, PlaneCaps, PlaneId, PlaneKind, PresentError,
    Rect, RejectReasons, ResourceGroup, ScaleLimits, SearchLimits, Size, SubmissionId, Transform,
    Verdict, Zpos, MAX_CANDIDATES,
};

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
/// `ce_advise`: no plane takes the composition target on this output.
pub const CE_ERR_NO_COMPOSE_PLANE: ce_status = -19;
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
pub const CE_ENGINE_SIZE: usize = 28672;
/// Alignment the header requires for the engine storage.
pub const CE_ENGINE_ALIGN: usize = 8;

/// `ce_layer_advice.placement`: on a plane, see the assignment.
pub const CE_PLACE_PLANE: u8 = 0;
/// `ce_layer_advice.placement`: drawn by the caller into the composition
/// buffer.
pub const CE_PLACE_COMPOSE: u8 = 1;
/// `ce_layer_advice.placement`: entirely off the output.
pub const CE_PLACE_HIDDEN: u8 = 2;

/// `ce_layer_advice.blocker`: on a plane, or hidden.
pub const CE_BLOCKER_NONE: u8 = 0;
/// No plane can take the layer alone; see `reasons`.
pub const CE_BLOCKER_STATIC: u8 = 1;
/// Fits a plane alone, not together with the others.
pub const CE_BLOCKER_COMBINATION: u8 = 2;
/// The check callback refused it.
pub const CE_BLOCKER_BACKEND: u8 = 3;
/// Cannot be clipped exactly to the output.
pub const CE_BLOCKER_CLIP: u8 = 4;
/// The search budget ended first.
pub const CE_BLOCKER_BUDGET: u8 = 5;

/// `ce_assignment.layer` of the composition target, in `ce_advice.compose`
/// and in what `ce_advise` hands to the check callback.
pub const CE_COMPOSE_LAYER: u32 = u32::MAX;

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

/// See `ce_compose_target` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_compose_target {
    modifier: u64,
    format: u32,
    blend: u8,
    enabled: u8,
}

/// See `ce_advise_options` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_advise_options {
    max_steps: u32,
    max_backend_checks: u32,
    max_candidates: u32,
    compose: ce_compose_target,
}

/// See `ce_layer_advice` in the header.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ce_layer_advice {
    assignment: ce_assignment,
    reasons: u32,
    placement: u8,
    blocker: u8,
}

/// See `ce_advice` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_advice {
    layers: [ce_layer_advice; MAX_LAYERS],
    compose: ce_assignment,
    layer_count: u32,
    compose_low: u32,
    compose_high: u32,
    has_compose: u8,
}

/// See `ce_buffer_entry` in the header.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ce_buffer_entry {
    buffer: u32,
    info: ce_buffer_info,
}

/// The check callback of `ce_backend_ops` and `ce_advise`.
type CheckFn =
    unsafe extern "C" fn(*mut c_void, *const ce_frame, *const ce_assignment, usize) -> i32;

/// A backend implemented by C functions.
pub struct CBackend {
    ops: ce_backend_ops,
    ctx: *mut c_void,
    /// Where the C backend describes itself: the second field of the
    /// `ce_engine` that holds this backend, in the caller's storage, which
    /// does not move. Not on the stack: it is about 9 KiB.
    raw_caps: *mut ce_capabilities,
}

/// The engine as seen from C.
pub struct ce_engine(Engine<CBackend>, ce_capabilities);

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

/// The Rust form of a C layer, `None` if its blend or transform is unknown.
fn layer_from(raw: ce_layer) -> Option<Layer> {
    Some(Layer {
        buffer: BufferId(raw.buffer),
        source: rect_from(raw.source),
        destination: rect_from(raw.destination),
        z_order: raw.z_order,
        alpha: raw.alpha,
        blend: blend_from(raw.blend)?,
        transform: transform_from(raw.transform)?,
    })
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

/// The C form of an assignment; the composition target's `layer`
/// (`usize::MAX`) becomes `CE_COMPOSE_LAYER`.
fn assignment_to(assignment: &Assignment) -> ce_assignment {
    ce_assignment {
        layer: u32::try_from(assignment.layer).unwrap_or(CE_COMPOSE_LAYER),
        plane: assignment.plane.0,
        plane_index: assignment.plane_index as u32,
        zpos: assignment.zpos,
        source: rect_to(assignment.source),
        destination: rect_to(assignment.destination),
    }
}

/// Converts what the C backend reported, dropping anything malformed.
/// Under-reporting is safe; inventing capabilities is not.
fn capabilities_from(raw: &ce_capabilities, caps: &mut Capabilities) {
    caps.replace_pending = raw.replace_pending != 0;
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
            *slot = assignment_to(assignment);
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
    fn capabilities(&mut self, caps: &mut Capabilities) {
        let Some(capabilities) = self.ops.capabilities else {
            return;
        };
        let raw = self.raw_caps;
        // SAFETY: `raw` points to the second field of the `ce_engine` that
        // owns this backend (`ce_engine_init`), valid for reads and writes
        // and used by nothing else; zero is a valid value of every field of
        // `ce_capabilities`, and it is cleared in place, not through a
        // zeroed temporary on the stack. The function and `ctx` were
        // supplied together through `ce_engine_init`, whose caller vouches
        // for them.
        unsafe {
            raw.write_bytes(0, 1);
            capabilities(self.ctx, raw);
            capabilities_from(&*raw, caps);
        }
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

/// Returns true if the caller's sizes of the `ce_advise` structures match
/// this library. A second function, so that `ce_abi_check` keeps its
/// signature.
#[no_mangle]
pub extern "C" fn ce_abi_check_advise(
    compose_target: usize,
    advise_options: usize,
    layer_advice: usize,
    advice: usize,
    buffer_entry: usize,
) -> bool {
    compose_target == size_of::<ce_compose_target>()
        && advise_options == size_of::<ce_advise_options>()
        && layer_advice == size_of::<ce_layer_advice>()
        && advice == size_of::<ce_advice>()
        && buffer_entry == size_of::<ce_buffer_entry>()
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
        // The backend's description area first, cleared in place: zero is a
        // valid value of every field. The engine is built straight into the
        // storage too; neither is assembled on the stack and copied.
        let raw_caps = &raw mut (*engine).1;
        raw_caps.write_bytes(0, 1);
        let backend = CBackend {
            ops: *ops,
            ctx,
            raw_caps,
        };
        let placed = &raw mut (*engine).0;
        placed.write(Engine::new_unprobed(backend));
        (*placed).probe();
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
        let Some(layer) = layer_from(unsafe { *frame.layers.add(index) }) else {
            return report(ce_error_info {
                layer: index as u32,
                ..invalid
            });
        };
        *slot = layer;
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

/// The caller's buffer table of `ce_advise`; the first entry of an id wins.
struct CBuffers<'a>(&'a [ce_buffer_entry]);

impl BufferLookup for CBuffers<'_> {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0
            .iter()
            .find(|entry| entry.buffer == buffer.0)
            .map(|entry| buffer_info_from(&entry.info))
    }
}

fn buffer_info_from(info: &ce_buffer_info) -> BufferInfo {
    BufferInfo {
        size: Size::new(info.width, info.height),
        format: PixelFormat(info.format),
        modifier: Modifier(info.modifier),
    }
}

/// Status and details of an advisory error. `fatal` is what the check
/// callback returned when it stopped the search, if it did.
fn describe_advice(error: AdviceError, fatal: Option<i32>, first_buffer: u32) -> ce_error_info {
    let mut info = ce_error_info::default();
    info.status = match error {
        AdviceError::UnknownOutput(_) => CE_ERR_UNKNOWN_OUTPUT,
        AdviceError::TooManyLayers { .. } => CE_ERR_TOO_MANY_LAYERS,
        AdviceError::UnknownBuffer { layer, buffer } => {
            info.layer = layer as u32;
            info.buffer = buffer.0;
            CE_ERR_UNKNOWN_BUFFER
        }
        AdviceError::InvalidSource { layer } => {
            info.layer = layer as u32;
            CE_ERR_INVALID_SOURCE
        }
        AdviceError::InvalidDestination { layer } => {
            info.layer = layer as u32;
            CE_ERR_INVALID_DESTINATION
        }
        AdviceError::NoComposePlane { reasons } => {
            info.reasons = reasons.bits();
            CE_ERR_NO_COMPOSE_PLANE
        }
        AdviceError::LimitExceeded => CE_ERR_SEARCH_LIMIT,
        AdviceError::Aborted => match fatal {
            // As `ce_present` reports a backend check that stopped the search.
            Some(CE_ERR_BACKEND_UNKNOWN_BUFFER) => {
                return describe(Error::Backend(BackendError::UnknownBuffer(BufferId(
                    first_buffer,
                ))))
            }
            Some(code) => return describe(Error::Backend(backend_error(code))),
            None => CE_ERR_BACKEND_REJECTED,
        },
    };
    info
}

fn placement_to(placement: &Placement) -> (u8, ce_assignment) {
    match placement {
        Placement::Plane(assignment) => (CE_PLACE_PLANE, assignment_to(assignment)),
        Placement::Compose => (CE_PLACE_COMPOSE, ce_assignment::default()),
        Placement::Hidden => (CE_PLACE_HIDDEN, ce_assignment::default()),
    }
}

fn blocker_to(blocker: Blocker) -> u8 {
    match blocker {
        Blocker::None => CE_BLOCKER_NONE,
        Blocker::Static => CE_BLOCKER_STATIC,
        Blocker::Combination => CE_BLOCKER_COMBINATION,
        Blocker::Backend => CE_BLOCKER_BACKEND,
        Blocker::Clip => CE_BLOCKER_CLIP,
        Blocker::Budget => CE_BLOCKER_BUDGET,
    }
}

/// Stateless advisory planning: which layers of `frame` go on which planes
/// of `caps`, and which contiguous run the caller composes into one buffer.
///
/// `check`, if not null, is called like `ce_backend_ops.check` with `ctx`,
/// `frame` as given, and each assignment the planner finds legal; the
/// composition target's assignment has `layer == CE_COMPOSE_LAYER`. It
/// returns `CE_OK` to accept, `CE_ERR_BACKEND_REJECTED` to try another
/// assignment, and anything else stops the call with that error.
///
/// On success `*advice` is written in full. On failure `*advice` is not
/// written, and `*info` (if not null) says why. Nothing is remembered
/// between calls.
///
/// # Safety
///
/// `caps`, `frame`, `options` and `advice` must be valid or null. `buffers`
/// must point to `buffer_count` entries (it may be null if the count is
/// zero). `frame.layers` must point to `layer_count` layers (it may be null
/// if the count is zero). `info` must be valid for writes or null. `check`
/// must be safe to call with `ctx` for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn ce_advise(
    caps: *const ce_capabilities,
    buffers: *const ce_buffer_entry,
    buffer_count: usize,
    frame: *const ce_frame,
    options: *const ce_advise_options,
    check: Option<CheckFn>,
    ctx: *mut c_void,
    advice: *mut ce_advice,
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
    // SAFETY: per the contract each is valid when not null.
    let (Some(raw_caps), Some(raw_frame), Some(raw_options), Some(out)) = (
        unsafe { caps.as_ref() },
        unsafe { frame.as_ref() },
        unsafe { options.as_ref() },
        unsafe { advice.as_mut() },
    ) else {
        return report(invalid);
    };
    if buffers.is_null() && buffer_count > 0
        || buffer_count > isize::MAX as usize / size_of::<ce_buffer_entry>()
    {
        return report(invalid);
    }
    if raw_frame.layer_count > MAX_LAYERS {
        return report(ce_error_info {
            status: CE_ERR_TOO_MANY_LAYERS,
            ..ce_error_info::default()
        });
    }
    if raw_frame.layer_count > 0 && raw_frame.layers.is_null() {
        return report(invalid);
    }
    let compose = if raw_options.compose.enabled != 0 {
        let Some(blend) = blend_from(raw_options.compose.blend) else {
            return report(invalid);
        };
        Some(ComposeTarget {
            format: PixelFormat(raw_options.compose.format),
            modifier: Modifier(raw_options.compose.modifier),
            blend,
        })
    } else {
        None
    };
    let defaults = SearchLimits::default();
    let or_default = |value: u32, default: u32| if value == 0 { default } else { value };
    let options = AdviseOptions {
        limits: SearchLimits {
            steps: or_default(raw_options.max_steps, defaults.steps),
            backend_checks: or_default(raw_options.max_backend_checks, defaults.backend_checks),
        },
        max_candidates: or_default(raw_options.max_candidates, MAX_CANDIDATES),
        compose,
    };

    let mut layers = [Layer::new(BufferId(0), Rect::default(), Rect::default(), 0); MAX_LAYERS];
    for (index, slot) in layers.iter_mut().enumerate().take(raw_frame.layer_count) {
        // SAFETY: `layers` points to `layer_count` layers per the contract.
        let Some(layer) = layer_from(unsafe { *raw_frame.layers.add(index) }) else {
            return report(ce_error_info {
                layer: index as u32,
                ..invalid
            });
        };
        *slot = layer;
    }
    let count = raw_frame.layer_count;
    let rust_frame = Frame {
        output: OutputId(raw_frame.output),
        layers: &layers[..count],
    };
    let table = if buffer_count == 0 {
        &[][..]
    } else {
        // SAFETY: `buffers` is not null and points to `buffer_count`
        // entries per the contract; the size was checked above.
        unsafe { core::slice::from_raw_parts(buffers, buffer_count) }
    };
    let mut converted = Capabilities::default();
    capabilities_from(raw_caps, &mut converted);

    let mut fatal = None;
    let result = composition_core::advise(
        &rust_frame,
        &converted,
        &CBuffers(table),
        &options,
        |plan: &Plan, target: Option<usize>| {
            let Some(check) = check else {
                return Verdict::Accept;
            };
            let mut assignments = [ce_assignment::default(); MAX_LAYERS];
            let assigned = plan.assignments();
            let n = assigned.len().min(MAX_LAYERS);
            for (index, (slot, assignment)) in assignments.iter_mut().zip(assigned).enumerate() {
                *slot = assignment_to(assignment);
                if Some(index) == target {
                    slot.layer = CE_COMPOSE_LAYER;
                }
            }
            // SAFETY: `frame` is the caller's, valid for the call; the
            // assignments are a live local. See the contract for `check`.
            match unsafe { check(ctx, frame, assignments.as_ptr(), n) } {
                CE_OK => Verdict::Accept,
                CE_ERR_BACKEND_REJECTED => Verdict::TryNext,
                code => {
                    fatal = Some(code);
                    Verdict::GiveUp
                }
            }
        },
    );
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            let first = layers[..count].first().map_or(0, |l| l.buffer.0);
            return report(describe_advice(error, fatal, first));
        }
    };

    let mut written = ce_advice {
        layers: [ce_layer_advice::default(); MAX_LAYERS],
        compose: ce_assignment::default(),
        layer_count: 0,
        compose_low: 0,
        compose_high: 0,
        has_compose: 0,
    };
    for (slot, entry) in written.layers.iter_mut().zip(result.layers()) {
        let (placement, assignment) = placement_to(&entry.placement);
        *slot = ce_layer_advice {
            assignment,
            reasons: entry.reasons.bits(),
            placement,
            blocker: blocker_to(entry.blocker),
        };
    }
    written.layer_count = result.layers().len().min(MAX_LAYERS) as u32;
    if let Some(assignment) = result.compose() {
        written.compose = assignment_to(assignment);
        written.compose.layer = CE_COMPOSE_LAYER;
        written.has_compose = 1;
    }
    if let Some((low, high)) = result.compose_run() {
        written.compose_low = low as u32;
        written.compose_high = high as u32;
    }
    *out = written;
    report(ce_error_info::default())
}

/// Per-plane static reasons of one layer, without a search.
///
/// # Safety
///
/// `caps`, `layer` and `buffer` must be valid or null. `reasons` must be
/// valid for writes of `CE_MAX_PLANES` values or null. `info` must be valid
/// for writes or null.
#[no_mangle]
pub unsafe extern "C" fn ce_explain_layer(
    caps: *const ce_capabilities,
    output: u32,
    layer: *const ce_layer,
    buffer: *const ce_buffer_info,
    reasons: *mut u32,
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
    // SAFETY: per the contract each is valid when not null.
    let (Some(raw_caps), Some(raw_layer), Some(buffer)) = (
        unsafe { caps.as_ref() },
        unsafe { layer.as_ref() },
        unsafe { buffer.as_ref() },
    ) else {
        return report(invalid);
    };
    if reasons.is_null() {
        return report(invalid);
    }
    let Some(layer) = layer_from(*raw_layer) else {
        return report(invalid);
    };
    let mut converted = Capabilities::default();
    capabilities_from(raw_caps, &mut converted);
    let mut out = [RejectReasons::NONE; MAX_PLANES];
    if let Err(error) = composition_core::explain_layer(
        &layer,
        &buffer_info_from(buffer),
        &converted,
        OutputId(output),
        &mut out,
    ) {
        return report(describe_advice(error, None, 0));
    }
    for (index, value) in out.iter().enumerate() {
        // SAFETY: `reasons` holds `CE_MAX_PLANES` values per the contract.
        unsafe { reasons.add(index).write(value.bits()) };
    }
    report(ce_error_info::default())
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
