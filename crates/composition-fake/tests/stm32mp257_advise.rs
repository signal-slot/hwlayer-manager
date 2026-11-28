//! `composition_core::advise` over the modelled STM32MP257F-DK (LTDC)
//! topology.
//!
//! MODEL, not a hardware result. The capabilities are typed in from
//! `docs/observed/stm32mp257-topology.md` (observed 2026-11-27 and
//! 2026-11-28) and copied from `topology_stm32mp257.rs`: planes 34 (Primary),
//! 42 and 49 (Overlay) on one CRTC, LINEAR only, zpos 0..=2, premultiplied
//! blend, plane alpha, no transforms claimed. No TEST_ONLY or commit reached a
//! kernel here: the check closure always accepts, so these tests only show
//! what the shared advisor says about the recorded caps.
//!
//! Observed rows modelled, per test:
//! - `two_layers_fit_without_composition`: "Planes" (three planes, zpos
//!   0..2) and the "Two-plane run": background on plane 34, window on 42.
//! - `four_layers_need_composition`: "Planes" with the `UNSUPPORTED layers`
//!   row (4 layers for 3 planes, `NoPlane IN_USE`).
//! - `nv12_layer_only_takes_plane_42`, `two_nv12_layers_need_composition`:
//!   "Formats": NV12 NV21 YU12 YV12 listed on plane 42 only.
//! - `rotated_layer_is_refused_and_composed`: `UNSUPPORTED rotation`
//!   (composition-drm claims no transforms, `NoPlane TRANSFORM`).
//! - `upscaled_layer_stays_off_plane_49`,
//!   `three_upscaled_layers_need_composition`: "Scaling bounds" of
//!   `docs/stm32mp257-ltdc-board.md` (planes 34 and 42 accepted upscales,
//!   plane 49 refused any above 1.0), modelled with the per-plane claim.
//! - `upscale_without_a_claim_is_static_scaling`: the observed caps, which
//!   claim no scaling.
//!
//! Pixel colours were never verified, so nothing here says anything about
//! blending results.

#![forbid(unsafe_code)]

use composition_core::{
    advise, Advice, AdviseOptions, BlendMode, Blocker, BufferId, BufferInfo, BufferLookup,
    Capabilities, ComposeTarget, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId,
    PixelFormat, Placement, PlaneCaps, PlaneId, PlaneKind, Rect, RejectReasons, ScaleLimits, Size,
    Transform, Verdict, Zpos,
};

const OUT: OutputId = OutputId(0);
const SCREEN: Size = Size::new(1920, 1080);
const FULL: Rect = Rect::new(0, 0, 1920, 1080);
const WINDOW_SRC: Rect = Rect::new(0, 0, 384, 540);
const WINDOW_SIZE: Size = Size::new(384, 540);

const XR24: PixelFormat = PixelFormat::XRGB8888;
const AR24: PixelFormat = PixelFormat::ARGB8888;

const RGB_FORMATS: [&[u8; 4]; 17] = [
    b"AR24", b"XR24", b"AB24", b"XB24", b"RA24", b"RX24", b"BA24", b"BX24", b"RG16", b"BG16",
    b"RG24", b"BG24", b"AR15", b"XR15", b"AR12", b"XR12", b"C8  ",
];
const PACKED_YUV: [&[u8; 4]; 4] = [b"YUYV", b"YVYU", b"UYVY", b"VYUY"];
const PLANAR_YUV: [&[u8; 4]; 4] = [b"NV12", b"NV21", b"YU12", b"YV12"];

fn ltdc_plane(
    id: u32,
    kind: PlaneKind,
    extra: &[&[u8; 4]],
    scaling: Option<ScaleLimits>,
) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs: 1,
        scaling,
        plane_alpha: true,
        blend_modes: BlendMode::Premultiplied.mask(),
        transforms: 0,
        zpos: Zpos::Range { min: 0, max: 2 },
        ..PlaneCaps::default()
    };
    for code in RGB_FORMATS.iter().copied().chain(extra.iter().copied()) {
        caps.formats
            .push(FormatModifier::linear(PixelFormat::fourcc(code)))
            .unwrap();
    }
    caps
}

/// The observed planes with `scaling` given per plane (34, 42, 49).
fn caps_per_plane(scaling: [Option<ScaleLimits>; 3]) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SCREEN,
            refresh_mhz: 60_000,
        })
        .unwrap();
    let mut yuv42 = PACKED_YUV.to_vec();
    yuv42.extend_from_slice(&PLANAR_YUV);
    for plane in [
        ltdc_plane(34, PlaneKind::Primary, &PACKED_YUV, scaling[0]),
        ltdc_plane(42, PlaneKind::Overlay, &yuv42, scaling[1]),
        ltdc_plane(49, PlaneKind::Overlay, &[], scaling[2]),
    ] {
        caps.planes.push(plane).unwrap();
    }
    caps
}

/// The observed caps: no plane claims scaling.
fn observed_caps() -> Capabilities {
    caps_per_plane([None; 3])
}

struct Buffers(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

fn buf(id: u32, size: Size, format: PixelFormat) -> (BufferId, BufferInfo) {
    (
        BufferId(id),
        BufferInfo {
            size,
            format,
            modifier: Modifier::LINEAR,
        },
    )
}

/// Buffer 1 is the full-screen XR24 background, 2..=5 are 384x540 AR24
/// windows, 6 is a full-screen NV12, 7 a 384x540 NV12.
fn buffers() -> Buffers {
    let mut all = vec![buf(1, SCREEN, XR24)];
    for id in 2..=5 {
        all.push(buf(id, WINDOW_SIZE, AR24));
    }
    all.push(buf(6, SCREEN, PixelFormat::NV12));
    all.push(buf(7, WINDOW_SIZE, PixelFormat::NV12));
    Buffers(all)
}

fn layer(buffer: u32, src: Rect, dst: Rect, z: i32) -> Layer {
    let mut layer = Layer::new(BufferId(buffer), src, dst, z);
    layer.blend = BlendMode::Premultiplied;
    layer
}

fn window(buffer: u32, x: i32, z: i32) -> Layer {
    layer(buffer, WINDOW_SRC, Rect::new(x, 100, 384, 540), z)
}

fn options() -> AdviseOptions {
    AdviseOptions {
        compose: Some(ComposeTarget {
            format: XR24,
            modifier: Modifier::LINEAR,
            blend: BlendMode::Premultiplied,
        }),
        ..AdviseOptions::default()
    }
}

fn run(caps: &Capabilities, layers: &[Layer]) -> Advice {
    let frame = Frame {
        output: OUT,
        layers,
    };
    advise(&frame, caps, &buffers(), &options(), |_, _| Verdict::Accept).expect("advice")
}

fn plane_of(advice: &Advice, layer: usize) -> Option<u32> {
    match advice.layers()[layer].placement {
        Placement::Plane(a) => Some(a.plane.0),
        _ => None,
    }
}

fn composed(advice: &Advice) -> Vec<usize> {
    advice
        .layers()
        .iter()
        .enumerate()
        .filter(|(_, l)| l.placement == Placement::Compose)
        .map(|(i, _)| i)
        .collect()
}

// Models: "Two-plane run": background on plane 34, window on plane 42.
#[test]
fn two_layers_fit_without_composition() {
    let layers = [layer(1, FULL, FULL, 0), window(2, 960, 1)];
    let advice = run(&observed_caps(), &layers);
    assert!(advice.is_direct());
    assert!(advice.compose().is_none());
    assert_eq!(advice.promoted(), 2);
    assert!(advice.layers().iter().all(|l| l.blocker == Blocker::None));
    let mut planes = [plane_of(&advice, 0).unwrap(), plane_of(&advice, 1).unwrap()];
    assert_eq!(planes[0], 34);
    planes.sort_unstable();
    assert_ne!(planes[0], planes[1], "distinct planes");
}

// Models: `UNSUPPORTED layers`, 4 layers for 3 planes (`NoPlane IN_USE`).
// The composition buffer takes one plane, so two layers are composed and two
// stay on planes; the composed layers are blocked by the combination, not by
// any single plane.
#[test]
fn four_layers_need_composition() {
    let layers = [
        layer(1, FULL, FULL, 0),
        window(2, 100, 1),
        window(3, 300, 2),
        window(4, 500, 3),
    ];
    let advice = run(&observed_caps(), &layers);
    assert!(!advice.is_direct());
    let composed_layers = composed(&advice);
    assert_eq!(
        composed_layers.len(),
        2,
        "four layers, three planes: a run of two"
    );
    assert_eq!(advice.compose_run().map(|(a, b)| b - a + 1), Some(2));
    for i in &composed_layers {
        let l = advice.layers()[*i];
        assert_eq!(l.blocker, Blocker::Combination, "layer {i}");
        assert!(l.reasons.is_empty());
    }
    // Two layers on planes plus the composition target: all three planes.
    assert_eq!(advice.promoted(), 2);
    let mut planes: Vec<u32> = (0..4).filter_map(|i| plane_of(&advice, i)).collect();
    planes.push(advice.compose().unwrap().plane.0);
    planes.sort_unstable();
    assert_eq!(planes, [34, 42, 49]);
}

// Models: "Formats": NV12 is listed on plane 42 only, so an NV12 layer sits
// on 42 and the RGB layer takes another plane.
#[test]
fn nv12_layer_only_takes_plane_42() {
    let layers = [layer(6, FULL, FULL, 0), window(2, 960, 1)];
    let advice = run(&observed_caps(), &layers);
    assert!(advice.is_direct());
    assert_eq!(plane_of(&advice, 0), Some(42));
    assert!(matches!(plane_of(&advice, 1), Some(34 | 49)));
}

// Models: "Formats": a second NV12 layer finds only plane 42, which is taken.
// Plane 42 takes each alone, so the blocker is the combination. The advisor
// composes the lower layer (drawn by the caller into the XR24 target, which
// planes 34 or 49 can show) and leaves the upper NV12 layer on plane 42.
#[test]
fn two_nv12_layers_need_composition() {
    let layers = [
        layer(6, FULL, FULL, 0),
        layer(7, WINDOW_SRC, Rect::new(960, 270, 384, 540), 1),
    ];
    let advice = run(&observed_caps(), &layers);
    assert!(!advice.is_direct());
    assert_eq!(composed(&advice), [0]);
    assert_eq!(advice.layers()[0].blocker, Blocker::Combination);
    assert_eq!(plane_of(&advice, 1), Some(42));
    let target = advice.compose().expect("a composition target");
    assert_ne!(target.plane.0, 42, "the target is RGB, so not plane 42");
}

// Models: `UNSUPPORTED rotation`: no transforms claimed, `NoPlane TRANSFORM`.
// No plane takes the layer on its own: a static blocker naming TRANSFORM.
#[test]
fn rotated_layer_is_refused_and_composed() {
    let mut rotated = window(2, 960, 1);
    rotated.transform = Transform::Rotate180;
    let layers = [layer(1, FULL, FULL, 0), rotated];
    let advice = run(&observed_caps(), &layers);
    assert!(!advice.is_direct());
    assert_eq!(composed(&advice), [1]);
    let l = advice.layers()[1];
    assert_eq!(l.blocker, Blocker::Static);
    assert!(
        l.reasons.contains(RejectReasons::TRANSFORM),
        "{:?}",
        l.reasons
    );
    assert_eq!(plane_of(&advice, 0), Some(34));
}

fn up() -> Option<ScaleLimits> {
    Some(ScaleLimits {
        max_upscale: 2 << 16,
        max_downscale: 1 << 16,
    })
}

/// Per-plane claim: upscaling on 34 and 42, none on 49.
fn upscale_caps() -> Capabilities {
    caps_per_plane([up(), up(), None])
}

// Models: "Scaling bounds": 2x upscaled layers go to plane 34 or 42, never
// 49, while the unscaled layer may use 49.
#[test]
fn upscaled_layer_stays_off_plane_49() {
    let layers = [
        layer(1, Rect::new(0, 0, 960, 540), FULL, 0),
        window(2, 100, 1),
        layer(3, WINDOW_SRC, Rect::new(500, 100, 768, 1080), 2),
    ];
    let advice = run(&upscale_caps(), &layers);
    assert!(advice.is_direct(), "{:?}", advice.layers());
    assert!(matches!(plane_of(&advice, 0), Some(34 | 42)));
    assert_eq!(plane_of(&advice, 1), Some(49), "the unscaled layer");
    assert!(matches!(plane_of(&advice, 2), Some(34 | 42)));
}

// Models: "Scaling bounds": three upscaled layers need three scaling planes,
// only two exist, so a run is composed. No single layer is statically
// blocked: planes 34 and 42 take each alone.
#[test]
fn three_upscaled_layers_need_composition() {
    let scaled = |buffer, z| layer(buffer, WINDOW_SRC, Rect::new(0, 0, 768, 1080), z);
    let layers = [scaled(2, 0), scaled(3, 1), scaled(4, 2)];
    let advice = run(&upscale_caps(), &layers);
    assert!(!advice.is_direct());
    assert!(!composed(&advice).is_empty());
    for i in composed(&advice) {
        assert_eq!(advice.layers()[i].blocker, Blocker::Combination);
    }
    assert!((0..3).filter_map(|i| plane_of(&advice, i)).all(|p| p != 49));
}

// Models: the observed caps (no scaling claimed): an upscaled layer is
// statically blocked by SCALING.
#[test]
fn upscale_without_a_claim_is_static_scaling() {
    let layers = [
        layer(1, FULL, FULL, 0),
        layer(2, WINDOW_SRC, Rect::new(960, 0, 768, 1080), 1),
    ];
    let advice = run(&observed_caps(), &layers);
    assert_eq!(composed(&advice), [1]);
    let l = advice.layers()[1];
    assert_eq!(l.blocker, Blocker::Static);
    assert!(l.reasons.contains(RejectReasons::SCALING));
}
