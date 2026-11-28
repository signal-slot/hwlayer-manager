//! `ce_advise` over the capability shapes of the repository's
//! `DriverProfile`s, compared with `composition_core::advise` on the same
//! input.
//!
//! `DriverProfile` lives in `composition-drm`, which this crate does not
//! depend on (and this test adds no dependency). Each profile's constraints
//! are therefore restated below as plain data, read from
//! `crates/composition-drm/src/profile.rs` and `profile/{rk3588,vc4}.rs`
//! (read from source, 2026-11-28; the plane lists and format lists are test
//! stand-ins, not discovered from a board). One description feeds both the
//! C-layout `Capabilities` for `ce_advise` and the Rust `Capabilities` for
//! `advise`, so the two calls see the same input. If a profile changes, this
//! restatement does not follow by itself. The test measures nothing and
//! makes no hardware claim.
//!
//! Compared per scene: status, and for a success the advice of every layer
//! (placement, assignment, blocker, reasons), the composition assignment and
//! the composed run; for a failure the error code and its details.

// Reason: this file calls the `extern "C"` API, whose functions are `unsafe`,
// and builds the C-layout structures a C caller would pass.
#![allow(unsafe_code)]

use core::ptr::null_mut;

use composition_core::{
    advise, AdviceError, AdviseOptions, BlendMode, Blocker, BufferId, BufferInfo, BufferLookup,
    Capabilities, ComposeTarget, FormatModifier, Frame, Layer, LayerTraits, Modifier, OutputCaps,
    OutputId, PixelFormat, Placement, PlaneCaps, PlaneId, PlaneKind, Rect, ResourceGroup,
    ScaleLimits, Size, Transform, Verdict, Zpos, ALPHA_OPAQUE,
};
use composition_ffi::*;

// ---- C-layout mirrors of the header's structures (as tests/advise.rs) ------

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct CRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct CBufferInfo {
    modifier: u64,
    width: u32,
    height: u32,
    format: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CLayer {
    buffer: u32,
    source: CRect,
    destination: CRect,
    z_order: i32,
    alpha: u16,
    blend: u8,
    transform: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CFrame {
    layers: *const CLayer,
    layer_count: usize,
    output: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct CAssignment {
    layer: u32,
    plane: u32,
    plane_index: u32,
    zpos: i32,
    source: CRect,
    destination: CRect,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CFormat {
    modifier: u64,
    format: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CPlaneCaps {
    formats: [CFormat; 32],
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
struct COutputCaps {
    id: u32,
    width: u32,
    height: u32,
    refresh_mhz: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CResourceGroup {
    planes: u32,
    consumers: u8,
    capacity: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CCapabilities {
    planes: [CPlaneCaps; 16],
    outputs: [COutputCaps; 4],
    groups: [CResourceGroup; 8],
    plane_count: u32,
    output_count: u32,
    group_count: u32,
    replace_pending: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct CErrorInfo {
    status: i32,
    layer: u32,
    reasons: u32,
    buffer: u32,
    platform: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CComposeTarget {
    modifier: u64,
    format: u32,
    blend: u8,
    enabled: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CAdviseOptions {
    max_steps: u32,
    max_backend_checks: u32,
    max_candidates: u32,
    compose: CComposeTarget,
}

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct CLayerAdvice {
    assignment: CAssignment,
    reasons: u32,
    placement: u8,
    blocker: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CAdvice {
    layers: [CLayerAdvice; 16],
    compose: CAssignment,
    layer_count: u32,
    compose_low: u32,
    compose_high: u32,
    has_compose: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CBufferEntry {
    buffer: u32,
    info: CBufferInfo,
}

// ---- constants ---------------------------------------------------------------

const fn fourcc(code: &[u8; 4]) -> u32 {
    code[0] as u32 | (code[1] as u32) << 8 | (code[2] as u32) << 16 | (code[3] as u32) << 24
}
const XRGB8888: u32 = fourcc(b"XR24");
const ARGB8888: u32 = fourcc(b"AR24");
const NV12: u32 = fourcc(b"NV12");

const BLEND_OPAQUE: u8 = 0;
const BLEND_PREMULTIPLIED: u8 = 1;
const BLEND_COVERAGE: u8 = 2;

const PLANE_PRIMARY: u8 = 0;
const PLANE_OVERLAY: u8 = 1;
const PLANE_CURSOR: u8 = 2;

// Layer traits of `composition_core::LayerTraits`, as the header's group
// `consumers` bits.
const ANY: u8 = 1;
const SCALED_H: u8 = 2;
const SCALED_V: u8 = 4;
const YUV: u8 = 8;

const OUT: u32 = 7;
const OUT_W: u32 = 1280;
const OUT_H: u32 = 800;

const VENDOR_ARM: u64 = 0x08;
const VENDOR_BROADCOM: u64 = 0x07;
const ARM_AFBC: u64 = VENDOR_ARM << 56 | 1;
const BROADCOM_T_TILED: u64 = VENDOR_BROADCOM << 56 | 1;
const BROADCOM_SAND64: u64 = VENDOR_BROADCOM << 56 | 3;

// ---- one description of a capability set --------------------------------------

#[derive(Clone)]
struct PlaneSpec {
    id: u32,
    kind: u8,
    /// `(modifier, format)`.
    formats: Vec<(u64, u32)>,
    blend_modes: u8,
    plane_alpha: bool,
    /// `(max_upscale, max_downscale)`, 16.16.
    scaling: Option<(u32, u32)>,
    max_source: Option<(u32, u32)>,
    zpos: i32,
    zpos_max: Option<i32>,
}

#[derive(Clone)]
struct Spec {
    name: &'static str,
    planes: Vec<PlaneSpec>,
    /// `(consumers, member plane mask, capacity)`.
    groups: Vec<(u8, u32, u8)>,
}

/// A primary, `overlays` overlays and optionally a cursor plane that KMS
/// would report: XRGB8888 and ARGB8888 linear, NV12 on all but the cursor,
/// all blend modes, no scaling or source limit yet (a profile adds those).
fn kms_planes(overlays: usize, cursor: bool, mutable_zpos: bool) -> Vec<PlaneSpec> {
    let count = 1 + overlays;
    let mut planes = Vec::new();
    for index in 0..count {
        planes.push(PlaneSpec {
            id: 31 + index as u32,
            kind: if index == 0 {
                PLANE_PRIMARY
            } else {
                PLANE_OVERLAY
            },
            formats: vec![(0, XRGB8888), (0, ARGB8888), (0, NV12)],
            blend_modes: 0b111,
            plane_alpha: true,
            scaling: None,
            max_source: None,
            zpos: if mutable_zpos { 0 } else { index as i32 },
            zpos_max: mutable_zpos.then_some(count as i32 - 1),
        });
    }
    if cursor {
        planes.push(PlaneSpec {
            id: 31 + count as u32,
            kind: PLANE_CURSOR,
            formats: vec![(0, ARGB8888)],
            blend_modes: 0b011,
            plane_alpha: false,
            scaling: None,
            max_source: None,
            zpos: count as i32,
            zpos_max: None,
        });
    }
    planes
}

fn all_planes(spec: &Spec) -> u32 {
    ((1u64 << spec.planes.len()) - 1) as u32
}

/// `DriverProfile::Generic`: only what KMS reports.
fn generic() -> Spec {
    Spec {
        name: "Generic",
        planes: kms_planes(2, true, false),
        groups: Vec::new(),
    }
}

/// `DriverProfile::Imx95Dpu`: six planes, scaling on all but a cursor, the
/// four resource groups.
fn imx95() -> Spec {
    let mut spec = Spec {
        name: "Imx95Dpu",
        planes: kms_planes(5, false, true),
        groups: Vec::new(),
    };
    for plane in &mut spec.planes {
        plane.scaling = Some((2048 << 16, 1 << 16));
        plane.max_source = Some((8192, 8192));
    }
    let all = all_planes(&spec);
    spec.groups = vec![
        (ANY, all, 6),
        (SCALED_H | SCALED_V | YUV, all, 4),
        (SCALED_H, all, 1),
        (SCALED_V, all, 1),
    ];
    spec
}

/// `DriverProfile::Stm32Ltdc`: straight alpha only, scaling on the primary
/// and the first overlay.
fn stm32() -> Spec {
    let mut spec = Spec {
        name: "Stm32Ltdc",
        planes: kms_planes(2, false, false),
        groups: Vec::new(),
    };
    let mut overlays = 0;
    for plane in &mut spec.planes {
        plane.blend_modes = 1 << BLEND_COVERAGE;
        let scales = match plane.kind {
            PLANE_PRIMARY => true,
            _ => {
                overlays += 1;
                overlays == 1
            }
        };
        if scales {
            plane.scaling = Some((1080 << 16, 1 << 16));
        }
    }
    spec
}

/// `DriverProfile::Rk3588Vop2`: two Cluster windows (ARM modifier formats)
/// and four Esmart windows, 8x scaling both ways, 4096x4320, four layers
/// per port.
fn rk3588() -> Spec {
    let mut spec = Spec {
        name: "Rk3588Vop2",
        planes: kms_planes(5, false, true),
        groups: Vec::new(),
    };
    for (index, plane) in spec.planes.iter_mut().enumerate() {
        if index < 2 {
            plane.formats.push((ARM_AFBC, XRGB8888));
        }
        plane.scaling = Some((8 << 16, 8 << 16));
        plane.max_source = Some((4096, 4320));
    }
    let all = all_planes(&spec);
    spec.groups = vec![(ANY, all, 4)];
    spec
}

/// `DriverProfile::Vc4Hvs5` / `Vc4Hvs6`: scaling on all but the cursor, a
/// shared line-buffer pool, and on HVS6 no T-tiled or SAND64 formats.
fn vc4(hvs6: bool) -> Spec {
    let mut spec = Spec {
        name: if hvs6 { "Vc4Hvs6" } else { "Vc4Hvs5" },
        planes: kms_planes(3, true, false),
        groups: Vec::new(),
    };
    for plane in &mut spec.planes {
        plane.formats.push((BROADCOM_T_TILED, XRGB8888));
        plane.formats.push((BROADCOM_SAND64, NV12));
        if hvs6 {
            plane
                .formats
                .retain(|&(m, _)| m != BROADCOM_T_TILED && m != BROADCOM_SAND64);
        }
        if plane.kind != PLANE_CURSOR {
            plane.scaling = Some((32 << 16, 2 << 16));
            plane.max_source = Some(if hvs6 { (8192, 8192) } else { (8191, 7680) });
        }
    }
    let all = all_planes(&spec);
    let lbm = SCALED_V | YUV;
    spec.groups = if hvs6 {
        vec![(lbm, all, 4), (ANY, all, 16)]
    } else {
        vec![(lbm, all, 8)]
    };
    spec
}

fn profiles() -> Vec<Spec> {
    vec![generic(), imx95(), stm32(), rk3588(), vc4(false), vc4(true)]
}

fn c_caps(spec: &Spec) -> Box<CCapabilities> {
    // SAFETY: every field of `CCapabilities` is a plain integer.
    let mut caps: Box<CCapabilities> = Box::new(unsafe { core::mem::zeroed() });
    caps.output_count = 1;
    caps.outputs[0] = COutputCaps {
        id: OUT,
        width: OUT_W,
        height: OUT_H,
        refresh_mhz: 60_000,
    };
    caps.plane_count = spec.planes.len() as u32;
    for (slot, plane) in caps.planes.iter_mut().zip(&spec.planes) {
        slot.id = plane.id;
        slot.kind = plane.kind;
        slot.outputs = 1;
        slot.format_count = plane.formats.len() as u32;
        for (target, &(modifier, format)) in slot.formats.iter_mut().zip(&plane.formats) {
            *target = CFormat { modifier, format };
        }
        if let Some((up, down)) = plane.scaling {
            slot.can_scale = 1;
            slot.max_upscale = up;
            slot.max_downscale = down;
        }
        if let Some((w, h)) = plane.max_source {
            slot.max_source_width = w;
            slot.max_source_height = h;
        }
        slot.plane_alpha = plane.plane_alpha as u8;
        slot.blend_modes = plane.blend_modes;
        slot.transforms = 1;
        slot.zpos_min = plane.zpos;
        slot.zpos_max = plane.zpos_max.unwrap_or(plane.zpos);
        slot.zpos_mutable = plane.zpos_max.is_some() as u8;
    }
    caps.group_count = spec.groups.len() as u32;
    for (slot, &(consumers, planes, capacity)) in caps.groups.iter_mut().zip(&spec.groups) {
        *slot = CResourceGroup {
            planes,
            consumers,
            capacity,
        };
    }
    caps
}

fn rust_caps(spec: &Spec) -> Capabilities {
    let mut caps = Capabilities::default();
    let _ = caps.outputs.push(OutputCaps {
        id: OutputId(OUT),
        size: Size::new(OUT_W, OUT_H),
        refresh_mhz: 60_000,
    });
    for plane in &spec.planes {
        let mut converted = PlaneCaps {
            id: PlaneId(plane.id),
            kind: match plane.kind {
                PLANE_PRIMARY => PlaneKind::Primary,
                PLANE_OVERLAY => PlaneKind::Overlay,
                _ => PlaneKind::Cursor,
            },
            outputs: 1,
            scaling: plane.scaling.map(|(up, down)| ScaleLimits {
                max_upscale: up,
                max_downscale: down,
            }),
            max_source: plane.max_source.map(|(w, h)| Size::new(w, h)),
            plane_alpha: plane.plane_alpha,
            blend_modes: plane.blend_modes,
            transforms: 1,
            zpos: match plane.zpos_max {
                Some(max) => Zpos::Range {
                    min: plane.zpos,
                    max,
                },
                None => Zpos::Fixed(plane.zpos),
            },
            ..PlaneCaps::default()
        };
        for &(modifier, format) in &plane.formats {
            let _ = converted.formats.push(FormatModifier {
                format: PixelFormat(format),
                modifier: Modifier(modifier),
            });
        }
        let _ = caps.planes.push(converted);
    }
    for &(consumers, planes, capacity) in &spec.groups {
        let _ = caps.resource_groups.push(ResourceGroup {
            consumers: LayerTraits(consumers),
            planes,
            capacity,
        });
    }
    caps
}

// ---- one description of a scene -----------------------------------------------

#[derive(Clone, Copy)]
struct Scene {
    buffer: u32,
    /// Source width and height (source origin is 0, 0).
    source: (u32, u32),
    destination: Rect,
    z_order: i32,
    blend: u8,
}

const BG: u32 = 1;
const VIDEO: u32 = 2;
const UI: u32 = 3;
const SPRITE_BASE: u32 = 100;

/// Buffer table: background, a 320x180 NV12 video frame, a 300x200 ARGB UI
/// layer and, from 100 on, 100x100 ARGB sprites.
fn buffers() -> Vec<(u32, CBufferInfo)> {
    let info = |width, height, format| CBufferInfo {
        modifier: 0,
        width,
        height,
        format,
    };
    let mut table = vec![
        (BG, info(OUT_W, OUT_H, XRGB8888)),
        (VIDEO, info(320, 180, NV12)),
        (UI, info(300, 200, ARGB8888)),
    ];
    for index in 0..16 {
        table.push((SPRITE_BASE + index, info(100, 100, ARGB8888)));
    }
    table
}

fn background() -> Scene {
    Scene {
        buffer: BG,
        source: (OUT_W, OUT_H),
        destination: Rect::new(0, 0, OUT_W, OUT_H),
        z_order: 0,
        blend: BLEND_OPAQUE,
    }
}

/// The video frame shown at twice its size (an upscale).
fn video() -> Scene {
    Scene {
        buffer: VIDEO,
        source: (320, 180),
        destination: Rect::new(100, 100, 640, 360),
        z_order: 1,
        blend: BLEND_OPAQUE,
    }
}

fn ui(blend: u8) -> Scene {
    Scene {
        buffer: UI,
        source: (300, 200),
        destination: Rect::new(800, 500, 300, 200),
        z_order: 2,
        blend,
    }
}

fn sprite(index: u32, z_order: i32, blend: u8) -> Scene {
    Scene {
        buffer: SPRITE_BASE + index,
        source: (100, 100),
        destination: Rect::new(10 + 20 * index as i32, 10, 100, 100),
        z_order,
        blend,
    }
}

/// A background and enough sprites to have `count` layers.
fn stack(count: usize, blend: u8) -> Vec<Scene> {
    let mut layers = vec![background()];
    for index in 0..count - 1 {
        layers.push(sprite(index as u32, index as i32 + 1, blend));
    }
    layers
}

fn rust_layer(scene: &Scene) -> Layer {
    Layer {
        buffer: BufferId(scene.buffer),
        source: Rect::new(0, 0, scene.source.0, scene.source.1),
        destination: scene.destination,
        z_order: scene.z_order,
        alpha: ALPHA_OPAQUE,
        blend: match scene.blend {
            BLEND_OPAQUE => BlendMode::Opaque,
            BLEND_PREMULTIPLIED => BlendMode::Premultiplied,
            _ => BlendMode::Coverage,
        },
        transform: Transform::Normal,
    }
}

fn c_layer(scene: &Scene) -> CLayer {
    CLayer {
        buffer: scene.buffer,
        source: CRect {
            x: 0,
            y: 0,
            width: scene.source.0,
            height: scene.source.1,
        },
        destination: CRect {
            x: scene.destination.x,
            y: scene.destination.y,
            width: scene.destination.width,
            height: scene.destination.height,
        },
        z_order: scene.z_order,
        alpha: ALPHA_OPAQUE,
        blend: scene.blend,
        transform: 0,
    }
}

struct Table(Vec<(u32, CBufferInfo)>);

impl BufferLookup for Table {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0
            .iter()
            .find(|(id, _)| *id == buffer.0)
            .map(|(_, info)| BufferInfo {
                size: Size::new(info.width, info.height),
                format: PixelFormat(info.format),
                modifier: Modifier(info.modifier),
            })
    }
}

// ---- the two calls and their comparison ---------------------------------------

/// Composition target: `Some(format)` is an opaque linear buffer of it.
type Compose = Option<u32>;

fn c_advise(
    spec: &Spec,
    table: &[(u32, CBufferInfo)],
    output: u32,
    layers: &[Scene],
    compose: Compose,
) -> (i32, CAdvice, CErrorInfo) {
    let caps = c_caps(spec);
    let entries: Vec<CBufferEntry> = table
        .iter()
        .map(|&(buffer, info)| CBufferEntry { buffer, info })
        .collect();
    let c_layers: Vec<CLayer> = layers.iter().map(c_layer).collect();
    let frame = CFrame {
        layers: c_layers.as_ptr(),
        layer_count: c_layers.len(),
        output,
    };
    let options = CAdviseOptions {
        compose: CComposeTarget {
            modifier: 0,
            format: compose.unwrap_or(0),
            blend: BLEND_OPAQUE,
            enabled: compose.is_some() as u8,
        },
        ..CAdviseOptions::default()
    };
    // SAFETY: every field of `CAdvice` is a plain integer.
    let mut advice: CAdvice = unsafe { core::mem::zeroed() };
    let mut info = CErrorInfo::default();
    // SAFETY: the mirrors have the header's layout; every pointer is valid
    // for the call.
    let status = unsafe {
        ce_advise(
            (&raw const *caps).cast(),
            entries.as_ptr().cast(),
            entries.len(),
            (&raw const frame).cast(),
            (&raw const options).cast(),
            None,
            null_mut(),
            (&raw mut advice).cast(),
            (&raw mut info).cast(),
        )
    };
    (status, advice, info)
}

/// The status `ce_advise` is documented to return for `error`.
fn code(error: &AdviceError) -> i32 {
    match error {
        AdviceError::UnknownOutput(_) => CE_ERR_UNKNOWN_OUTPUT,
        AdviceError::TooManyLayers { .. } => CE_ERR_TOO_MANY_LAYERS,
        AdviceError::UnknownBuffer { .. } => CE_ERR_UNKNOWN_BUFFER,
        AdviceError::InvalidSource { .. } => CE_ERR_INVALID_SOURCE,
        AdviceError::InvalidDestination { .. } => CE_ERR_INVALID_DESTINATION,
        AdviceError::NoComposePlane { .. } => CE_ERR_NO_COMPOSE_PLANE,
        AdviceError::LimitExceeded => CE_ERR_SEARCH_LIMIT,
        AdviceError::Aborted => CE_ERR_BACKEND_REJECTED,
    }
}

fn blocker_code(blocker: Blocker) -> u8 {
    match blocker {
        Blocker::None => CE_BLOCKER_NONE,
        Blocker::Static => CE_BLOCKER_STATIC,
        Blocker::Combination => CE_BLOCKER_COMBINATION,
        Blocker::Backend => CE_BLOCKER_BACKEND,
        Blocker::Clip => CE_BLOCKER_CLIP,
        Blocker::Budget => CE_BLOCKER_BUDGET,
    }
}

fn c_assignment(a: &composition_core::Assignment) -> CAssignment {
    let rect = |r: Rect| CRect {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    };
    CAssignment {
        layer: u32::try_from(a.layer).unwrap_or(CE_COMPOSE_LAYER),
        plane: a.plane.0,
        plane_index: a.plane_index as u32,
        zpos: a.zpos,
        source: rect(a.source),
        destination: rect(a.destination),
    }
}

/// What a comparison found, for the tests that want to know the outcome was
/// not trivial.
#[derive(Default)]
struct Outcome {
    ok: bool,
    on_planes: usize,
    composed: usize,
}

/// Runs both forms on one input and asserts they agree.
fn compare(
    spec: &Spec,
    scene: &str,
    output: u32,
    layers: &[Scene],
    compose: Compose,
    buffer_table: &[(u32, CBufferInfo)],
) -> Outcome {
    let what = format!("{} / {scene} / compose {compose:?}", spec.name);
    let (status, c, info) = c_advise(spec, buffer_table, output, layers, compose);

    let rust_layers: Vec<Layer> = layers.iter().map(rust_layer).collect();
    let options = AdviseOptions {
        compose: compose.map(|format| ComposeTarget {
            format: PixelFormat(format),
            modifier: Modifier(0),
            blend: BlendMode::Opaque,
        }),
        ..AdviseOptions::default()
    };
    let result = advise(
        &Frame {
            output: OutputId(output),
            layers: &rust_layers,
        },
        &rust_caps(spec),
        &Table(buffer_table.to_vec()),
        &options,
        |_, _| Verdict::Accept,
    );
    assert_eq!(info.status, status, "{what}: info.status differs");

    let advice = match result {
        Err(error) => {
            assert_eq!(status, code(&error), "{what}: {error:?}");
            match error {
                AdviceError::UnknownBuffer { layer, buffer } => {
                    assert_eq!(
                        (info.layer, info.buffer),
                        (layer as u32, buffer.0),
                        "{what}"
                    );
                }
                AdviceError::InvalidSource { layer }
                | AdviceError::InvalidDestination { layer } => {
                    assert_eq!(info.layer, layer as u32, "{what}");
                }
                AdviceError::NoComposePlane { reasons } => {
                    assert_eq!(info.reasons, reasons.bits(), "{what}");
                }
                _ => {}
            }
            return Outcome::default();
        }
        Ok(advice) => advice,
    };
    assert_eq!(status, CE_OK, "{what}: Rust succeeded, C returned {status}");

    let mut outcome = Outcome {
        ok: true,
        ..Outcome::default()
    };
    assert_eq!(c.layer_count as usize, advice.layers().len(), "{what}");
    for (index, expected) in advice.layers().iter().enumerate() {
        let got = c.layers[index];
        let (placement, assignment) = match expected.placement {
            Placement::Plane(a) => {
                outcome.on_planes += 1;
                (CE_PLACE_PLANE, c_assignment(&a))
            }
            Placement::Compose => {
                outcome.composed += 1;
                (CE_PLACE_COMPOSE, CAssignment::default())
            }
            Placement::Hidden => (CE_PLACE_HIDDEN, CAssignment::default()),
        };
        assert_eq!(got.placement, placement, "{what}: layer {index} placement");
        assert_eq!(
            got.assignment, assignment,
            "{what}: layer {index} assignment"
        );
        assert_eq!(
            got.blocker,
            blocker_code(expected.blocker),
            "{what}: layer {index} blocker"
        );
        assert_eq!(
            got.reasons,
            expected.reasons.bits(),
            "{what}: layer {index} reasons"
        );
    }
    match advice.compose() {
        Some(a) => {
            assert_eq!(c.has_compose, 1, "{what}");
            assert_eq!(c.compose, c_assignment(a), "{what}: compose assignment");
            let (low, high) = advice.compose_run().expect("a run with a target");
            assert_eq!((c.compose_low, c.compose_high), (low as u32, high as u32));
        }
        None => assert_eq!(c.has_compose, 0, "{what}"),
    }
    outcome
}

// ---- tests ------------------------------------------------------------------------

fn scenes(spec: &Spec) -> Vec<(String, Vec<Scene>)> {
    let planes = spec.planes.len();
    vec![
        ("one layer".into(), vec![background()]),
        (
            "video and UI, premultiplied".into(),
            vec![background(), video(), ui(BLEND_PREMULTIPLIED)],
        ),
        (
            "video and UI, straight alpha".into(),
            vec![background(), video(), ui(BLEND_COVERAGE)],
        ),
        (
            "more layers than planes, premultiplied".into(),
            stack((planes + 2).min(16), BLEND_PREMULTIPLIED),
        ),
        (
            "more layers than planes, straight alpha".into(),
            stack((planes + 2).min(16), BLEND_COVERAGE),
        ),
    ]
}

#[test]
fn the_profile_restatements_fit_the_c_arrays() {
    for spec in profiles() {
        assert!(spec.planes.len() <= 16, "{}", spec.name);
        assert!(spec.groups.len() <= 8, "{}", spec.name);
        for plane in &spec.planes {
            assert!(plane.formats.len() <= 32, "{}", spec.name);
        }
        // `kms_planes` gives every plane the same ids on both sides.
        assert_eq!(rust_caps(&spec).planes.len(), spec.planes.len());
        assert_eq!(rust_caps(&spec).resource_groups.len(), spec.groups.len());
    }
}

#[test]
fn every_profile_and_scene_matches_the_rust_form() {
    let table = buffers();
    for spec in profiles() {
        for (name, layers) in scenes(&spec) {
            for compose in [Some(XRGB8888), None] {
                compare(&spec, &name, OUT, &layers, compose, &table);
            }
        }
    }
}

#[test]
fn one_layer_goes_on_a_plane_on_every_profile() {
    let table = buffers();
    for spec in profiles() {
        let outcome = compare(
            &spec,
            "one layer",
            OUT,
            &[background()],
            Some(XRGB8888),
            &table,
        );
        assert!(outcome.ok, "{}", spec.name);
        assert_eq!(
            (outcome.on_planes, outcome.composed),
            (1, 0),
            "{}",
            spec.name
        );
    }
}

#[test]
fn more_layers_than_planes_need_composition_where_it_is_allowed() {
    let table = buffers();
    for spec in profiles() {
        let layers = stack(spec.planes.len() + 2, BLEND_PREMULTIPLIED);
        let with = compare(&spec, "overflow", OUT, &layers, Some(XRGB8888), &table);
        if with.ok {
            assert!(with.composed > 0, "{}: nothing composed", spec.name);
        }
        let without = compare(&spec, "overflow", OUT, &layers, None, &table);
        if without.ok {
            assert!(without.composed > 0, "{}: all on planes", spec.name);
        }
    }
}

#[test]
fn the_profiles_do_not_all_give_the_same_advice() {
    // A guard against the comparison passing because every profile reduces
    // to one shape: the straight-alpha-only profile cannot take the
    // premultiplied UI layer on a plane, the others can.
    let table = buffers();
    let layers = [background(), ui(BLEND_PREMULTIPLIED)];
    let composed = |spec: &Spec| compare(spec, "ui", OUT, &layers, Some(XRGB8888), &table).composed;
    assert!(composed(&stm32()) > 0);
    assert_eq!(composed(&imx95()), 0);
    assert_eq!(composed(&generic()), 0);
}

#[test]
fn error_codes_match_on_every_profile() {
    let table = buffers();
    for spec in profiles() {
        let one = [background()];
        // Unknown output.
        let o = compare(&spec, "unknown output", 99, &one, Some(XRGB8888), &table);
        assert!(!o.ok);
        // Unknown buffer: an entry the table lacks.
        let mut missing = background();
        missing.buffer = 999;
        compare(
            &spec,
            "unknown buffer",
            OUT,
            &[missing],
            Some(XRGB8888),
            &table,
        );
        // Source outside the buffer, empty destination.
        let mut source = background();
        source.source.0 += 1;
        compare(
            &spec,
            "invalid source",
            OUT,
            &[source],
            Some(XRGB8888),
            &table,
        );
        let mut destination = background();
        destination.destination.height = 0;
        compare(
            &spec,
            "invalid destination",
            OUT,
            &[destination],
            None,
            &table,
        );
        // Too many layers: the C array holds 16.
        let many = stack(17, BLEND_OPAQUE);
        compare(&spec, "too many layers", OUT, &many, Some(XRGB8888), &table);
        // No plane takes an NV12 composition target on a full overflow.
        let overflow = stack(spec.planes.len() + 2, BLEND_PREMULTIPLIED);
        compare(&spec, "NV12 target", OUT, &overflow, Some(NV12), &table);
    }
}

#[test]
fn error_codes_are_the_documented_ones() {
    // Pins the statuses themselves, so that two forms that agree on a wrong
    // code would still fail.
    let table = buffers();
    let spec = imx95();
    let (status, _, _) = c_advise(&spec, &table, 99, &[background()], Some(XRGB8888));
    assert_eq!(status, CE_ERR_UNKNOWN_OUTPUT);
    let (status, _, _) = c_advise(&spec, &table, OUT, &stack(17, BLEND_OPAQUE), None);
    assert_eq!(status, CE_ERR_TOO_MANY_LAYERS);
    let mut missing = background();
    missing.buffer = 999;
    let (status, _, info) = c_advise(&spec, &table, OUT, &[missing], None);
    assert_eq!((status, info.buffer), (CE_ERR_UNKNOWN_BUFFER, 999));
}
