//! A single plane per CRTC forces composition: the i.MX8M Plus LCDIFv3
//! topology through the planner and `advise()`.
//!
//! Topology facts, all 'read from source (mainline lcdif_kms.c), not
//! observed':
//!
//! - `drivers/gpu/drm/mxsfb/lcdif_kms.c` registers one primary plane per CRTC
//!   and no overlay plane, with no scaling, no per-plane alpha and no
//!   settable zpos.
//! - `imx8mp.dtsi` has three LCDIF instances: `lcdif1` to MIPI DSI, `lcdif2`
//!   to LVDS/LDB, `lcdif3` to HDMI.
//!
//! The capability tables below are shapes built from those facts; nothing was
//! measured on a board and no hardware was used. Only the public API is used.
//! The point is the planner's behaviour on such a topology, not the exact
//! driver format list.

#![forbid(unsafe_code)]

use composition_core::planner::plan;
use composition_core::{
    advise, AdviseOptions, BlendMode, Blocker, BufferId, BufferInfo, BufferLookup, Capabilities,
    ComposeTarget, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat,
    Placement, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect, Size, Transform, Verdict, Zpos,
    COMPOSE_LAYER,
};

const W: u32 = 1920;
const H: u32 = 1080;

struct Buffers(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

fn buf(id: u32, w: u32, h: u32, format: PixelFormat) -> (BufferId, BufferInfo) {
    (
        BufferId(id),
        BufferInfo {
            size: Size::new(w, h),
            format,
            modifier: Modifier::LINEAR,
        },
    )
}

/// One LCDIF: an output and its single primary plane.
fn add_lcdif(caps: &mut Capabilities, index: u32) {
    caps.outputs
        .push(OutputCaps {
            id: OutputId(index),
            size: Size::new(W, H),
            refresh_mhz: 60_000,
        })
        .unwrap();
    let mut plane = PlaneCaps {
        id: PlaneId(index + 1),
        kind: PlaneKind::Primary,
        outputs: 1 << index,
        scaling: None,
        plane_alpha: false,
        blend_modes: BlendMode::Opaque.mask(),
        transforms: Transform::Normal.mask(),
        zpos: Zpos::Fixed(0),
        full_output_only: true,
        ..PlaneCaps::default()
    };
    for f in [
        PixelFormat::XRGB8888,
        PixelFormat::ARGB8888,
        PixelFormat::RGB565,
    ] {
        plane.formats.push(FormatModifier::linear(f)).unwrap();
    }
    caps.planes.push(plane).unwrap();
}

fn lcdif(outputs: u32) -> Capabilities {
    let mut caps = Capabilities::default();
    for i in 0..outputs {
        add_lcdif(&mut caps, i);
    }
    caps
}

fn full(buffer: u32, z: i32) -> Layer {
    Layer::new(
        BufferId(buffer),
        Rect::new(0, 0, W, H),
        Rect::new(0, 0, W, H),
        z,
    )
}

/// A UI layer over the whole output and a smaller video-like layer on top.
fn two_layers() -> [Layer; 2] {
    [
        full(1, 0),
        Layer::new(
            BufferId(2),
            Rect::new(0, 0, 640, 360),
            Rect::new(100, 100, 640, 360),
            1,
        ),
    ]
}

fn target() -> ComposeTarget {
    ComposeTarget {
        format: PixelFormat::XRGB8888,
        modifier: Modifier::LINEAR,
        blend: BlendMode::Opaque,
    }
}

fn buffers() -> Buffers {
    Buffers(vec![
        buf(1, W, H, PixelFormat::XRGB8888),
        buf(2, 640, 360, PixelFormat::XRGB8888),
    ])
}

#[test]
fn two_layers_have_no_plan_on_one_plane() {
    let caps = lcdif(1);
    let layers = two_layers();
    let frame = Frame {
        output: OutputId(0),
        layers: &layers,
    };
    // Two layers need two planes; the topology has one and no overlay.
    let result = plan(&frame, &caps, &buffers());
    assert!(
        matches!(result, Err(PlanError::NoPlane { .. })),
        "{result:?}"
    );
    assert!(caps.planes.iter().all(|p| p.kind != PlaneKind::Overlay));
}

#[test]
fn advise_composes_both_layers_and_assigns_no_overlay() {
    let caps = lcdif(1);
    let layers = two_layers();
    let frame = Frame {
        output: OutputId(0),
        layers: &layers,
    };
    let options = AdviseOptions {
        compose: Some(target()),
        ..AdviseOptions::default()
    };
    let advice = advise(&frame, &caps, &buffers(), &options, |_, _| Verdict::Accept).unwrap();

    assert!(!advice.is_direct());
    // The only plane carries the composition target, so no input layer is
    // promoted to a plane of its own.
    assert_eq!(advice.promoted(), 0);
    assert!(advice
        .layers()
        .iter()
        .all(|l| l.placement == Placement::Compose));
    assert_eq!(advice.compose_run(), Some((0, 1)));
    let compose = advice.compose().expect("composition target");
    assert_eq!(compose.layer, COMPOSE_LAYER);
    assert_eq!(compose.plane, PlaneId(1));
    assert_eq!(caps.planes[compose.plane_index].kind, PlaneKind::Primary);
}

#[test]
fn advise_without_composition_target_reports_every_layer_composed() {
    let caps = lcdif(1);
    let layers = two_layers();
    let frame = Frame {
        output: OutputId(0),
        layers: &layers,
    };
    let advice = advise(
        &frame,
        &caps,
        &buffers(),
        &AdviseOptions::default(),
        |_, _| Verdict::Accept,
    )
    .unwrap();
    assert_eq!(advice.promoted(), 0);
    assert!(advice.compose().is_none());
    assert!(advice
        .layers()
        .iter()
        .all(|l| l.placement == Placement::Compose && l.blocker != Blocker::None));
}

#[test]
fn single_full_screen_layer_uses_the_primary_directly() {
    let caps = lcdif(1);
    let layers = [full(1, 0)];
    let frame = Frame {
        output: OutputId(0),
        layers: &layers,
    };
    let p = plan(&frame, &caps, &buffers()).unwrap();
    assert_eq!(p.assignments().len(), 1);
    assert_eq!(p.assignments()[0].plane, PlaneId(1));
    assert_eq!(
        caps.planes[p.assignments()[0].plane_index].kind,
        PlaneKind::Primary
    );

    let advice = advise(
        &frame,
        &caps,
        &buffers(),
        &AdviseOptions::default(),
        |_, _| Verdict::Accept,
    )
    .unwrap();
    assert!(advice.is_direct());
    assert_eq!(advice.promoted(), 1);
    assert!(advice.compose().is_none());
}

#[test]
fn three_lcdifs_never_share_a_layer_across_crtcs() {
    let caps = lcdif(3);
    let buffers = buffers();
    for out in 0..3u32 {
        let layers = two_layers();
        let frame = Frame {
            output: OutputId(out),
            layers: &layers,
        };
        // Still one plane per CRTC: two layers do not fit.
        assert!(matches!(
            plan(&frame, &caps, &buffers),
            Err(PlanError::NoPlane { .. })
        ));
        let options = AdviseOptions {
            compose: Some(target()),
            ..AdviseOptions::default()
        };
        let advice = advise(&frame, &caps, &buffers, &options, |_, _| Verdict::Accept).unwrap();
        assert_eq!(advice.promoted(), 0);
        // The composition target sits on this output's own plane only.
        let compose = advice.compose().unwrap();
        assert_eq!(compose.plane, PlaneId(out + 1));
        assert_eq!(caps.planes[compose.plane_index].outputs, 1 << out);

        // A single layer goes to this output's plane, not another's.
        let one = [full(1, 0)];
        let frame = Frame {
            output: OutputId(out),
            layers: &one,
        };
        let p = plan(&frame, &caps, &buffers).unwrap();
        assert_eq!(p.output(), OutputId(out));
        assert_eq!(p.assignments()[0].plane, PlaneId(out + 1));
    }
}
