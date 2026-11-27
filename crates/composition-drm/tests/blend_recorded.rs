//! `DriverProfile::Generic` and plane alpha / pixel blend mode on the recorded
//! RK3588 (NanoPC-T6) and BCM2712 (Raspberry Pi 5) topologies.
//!
//! Baseline tests: they pin what the generic planner does with translucent,
//! premultiplied and coverage layers and with opaque layers stacked over
//! them, so that the later `Rk3588Vop2` and `Vc4Hvs6` tests show the
//! difference. `Capabilities` does model both properties: `PlaneCaps` has
//! `plane_alpha` (the `alpha` property) and `blend_modes` (the `pixel blend
//! mode` enum values), and the fixtures carry them as recorded. The planner
//! only compares a layer's alpha and blend mode with those two fields of one
//! plane. It does not look at what lies beneath a layer, so a layer that is
//! fully covered, or an opaque layer on top of a translucent one, is placed
//! like any other. The tests below pin exactly that.
//!
//! Fixture data, copied here so that nothing reads `docs/observed`:
//! - `docs/observed/rk3588-topology.md`, observed 2026-11-28 (kernel
//!   6.18.55-current-rockchip64): all eight planes report plane alpha true,
//!   blend [none, premultiplied, coverage], zpos 0..=7, scaling not claimed;
//!   the probe's format lists (Cluster planes list AFBC pairs only in the
//!   first 32; Esmart planes list 21 linear pairs).
//! - `docs/observed/bcm2712.md`, observed 2026-11-28 (kernel
//!   6.18.50+rpt-rpi-2712): primaries zpos fixed 0, overlays 1..=17, every
//!   plane alpha true and blend [none, premultiplied, coverage]; the 32 linear
//!   pairs of the probe; the connected output (1920x1080) reaches primary 83
//!   and overlays 107 .. 228 (stride 11).
//! - Plan #457 scen-t6 handoff, observed 2026-11-27: planes 39 and 69 cannot
//!   reach video_port0.
//!
//! Source-tagged, not observed: the drivers apply the alpha as `alpha >> 8`
//! (VOP2) and `alpha >> 4` (vc4), and VOP2 treats a global alpha of 0xff as
//! off. Generic knows none of that, which the tests show only by what they
//! do not assert.

use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, Size, Zpos, ALPHA_OPAQUE,
};
use composition_drm::DriverProfile;

const OUT: OutputId = OutputId(0);
const ALL_BLEND: u8 =
    BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask() | BlendMode::Coverage.mask();
const MODES: [BlendMode; 3] = [
    BlendMode::Opaque,
    BlendMode::Premultiplied,
    BlendMode::Coverage,
];
const HALF: u16 = 0x8000;

fn fourcc(s: &[u8; 4]) -> PixelFormat {
    PixelFormat::fourcc(s)
}

// ---- RK3588 fixture (observed 2026-11-28, see the header) ----

const AFBC_A: Modifier = Modifier(0x0800_0000_0000_0001);
const AFBC_B: Modifier = Modifier(0x0800_0000_0000_0041);
const AFBC_C: Modifier = Modifier(0x0800_0000_0000_0011);

const CLUSTER_FORMATS: [&[u8; 4]; 14] = [
    b"XR30", b"XB30", b"XR24", b"AR24", b"XB24", b"AB24", b"RG24", b"BG24", b"RG16", b"BG16",
    b"YU08", b"YU10", b"YUYV", b"Y210",
];

const ESMART_FORMATS: [&[u8; 4]; 21] = [
    b"XR24", b"AR24", b"XB24", b"AB24", b"RG24", b"BG24", b"RG16", b"BG16", b"NV12", b"NV21",
    b"NV16", b"NV61", b"NV20", b"NV24", b"NV42", b"NV30", b"NV15", b"YVYU", b"VYUY", b"YUYV",
    b"UYVY",
];

fn rk_plane(id: u32, kind: PlaneKind, cluster: bool, outputs: u32) -> PlaneCaps {
    let mut plane = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs,
        plane_alpha: true,
        blend_modes: ALL_BLEND,
        zpos: Zpos::Range { min: 0, max: 7 },
        ..PlaneCaps::default()
    };
    if cluster {
        for (modifier, count) in [(AFBC_A, 14), (AFBC_B, 14), (AFBC_C, 4)] {
            for code in &CLUSTER_FORMATS[..count] {
                let pair = FormatModifier {
                    format: fourcc(code),
                    modifier,
                };
                plane.formats.push(pair).unwrap();
            }
        }
    } else {
        for code in &ESMART_FORMATS {
            let pair = FormatModifier::linear(fourcc(code));
            plane.formats.push(pair).unwrap();
        }
    }
    plane
}

fn rk_caps() -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(1920, 1080),
            refresh_mhz: 60_000,
        })
        .unwrap();
    // (id, kind, Cluster, outputs mask on output 0)
    for (id, kind, cluster, outputs) in [
        (33, PlaneKind::Primary, true, 0b1),
        (39, PlaneKind::Primary, true, 0b0),
        (45, PlaneKind::Overlay, true, 0b1),
        (51, PlaneKind::Overlay, true, 0b1),
        (57, PlaneKind::Overlay, false, 0b1),
        (63, PlaneKind::Overlay, false, 0b1),
        (69, PlaneKind::Overlay, false, 0b0),
        (75, PlaneKind::Overlay, false, 0b1),
    ] {
        caps.planes
            .push(rk_plane(id, kind, cluster, outputs))
            .unwrap();
    }
    caps
}

// ---- BCM2712 fixture (observed 2026-11-28, see the header) ----

const PI5_FORMATS: [&[u8; 4]; 32] = [
    b"XR24", b"AR24", b"AB24", b"XB24", b"RX24", b"RA24", b"BX24", b"BA24", b"RG16", b"BG16",
    b"AR15", b"XR15", b"RG24", b"BG24", b"YU16", b"YV16", b"YU24", b"YV24", b"YU12", b"YV12",
    b"NV12", b"NV21", b"NV16", b"NV61", b"XR30", b"AR30", b"AB30", b"XB30", b"RGB8", b"BGR8",
    b"XR12", b"AR12",
];

const PI5_OVERLAYS: [u32; 12] = [107, 118, 129, 140, 151, 162, 173, 184, 195, 206, 217, 228];

fn pi5_plane(id: u32, kind: PlaneKind, zpos: Zpos, outputs: u32) -> PlaneCaps {
    let mut plane = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs,
        plane_alpha: true,
        blend_modes: ALL_BLEND,
        zpos,
        ..PlaneCaps::default()
    };
    for code in &PI5_FORMATS {
        plane
            .formats
            .push(FormatModifier::linear(fourcc(code)))
            .unwrap();
    }
    plane
}

fn pi5_caps() -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(1920, 1080),
            refresh_mhz: 60_000,
        })
        .unwrap();
    // The four primaries; only 83 reaches the connected output.
    for (id, outputs) in [(48, 0b0), (67, 0b0), (83, 0b1), (95, 0b0)] {
        let plane = pi5_plane(id, PlaneKind::Primary, Zpos::Fixed(0), outputs);
        caps.planes.push(plane).unwrap();
    }
    for id in PI5_OVERLAYS {
        let zpos = Zpos::Range { min: 1, max: 17 };
        let plane = pi5_plane(id, PlaneKind::Overlay, zpos, 0b1);
        caps.planes.push(plane).unwrap();
    }
    caps
}

fn generic(mut caps: Capabilities) -> Capabilities {
    DriverProfile::Generic.apply(&mut caps);
    caps
}

// ---- buffers and layers ----

/// Opaque XRGB, linear.
const XR_LIN: BufferId = BufferId(1);
/// ARGB, linear.
const AR_LIN: BufferId = BufferId(2);
/// ARGB with the first recorded AFBC modifier (Cluster planes only).
const AR_AFBC: BufferId = BufferId(3);
/// XRGB with the first recorded AFBC modifier.
const XR_AFBC: BufferId = BufferId(4);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let (format, modifier) = match buffer {
            XR_LIN => (PixelFormat::XRGB8888, Modifier::LINEAR),
            AR_LIN => (PixelFormat::ARGB8888, Modifier::LINEAR),
            AR_AFBC => (PixelFormat::ARGB8888, AFBC_A),
            XR_AFBC => (PixelFormat::XRGB8888, AFBC_A),
            _ => return None,
        };
        Some(BufferInfo {
            size: Size::new(64, 64),
            format,
            modifier,
        })
    }
}

/// A 64x64 layer, unscaled, at the given stacking position. All layers sit
/// at the same place, so each one overlaps the ones below it.
fn layer(buffer: BufferId, z: i32, alpha: u16, blend: BlendMode) -> Layer {
    Layer {
        alpha,
        blend,
        ..Layer::new(
            buffer,
            Rect::new(0, 0, 64, 64),
            Rect::new(10, 10, 64, 64),
            z,
        )
    }
}

/// (layer index, plane id, zpos) of the plan, bottom to top.
type Placed = Vec<(usize, u32, u32)>;

fn placed(caps: &Capabilities, layers: &[Layer]) -> Result<Placed, PlanError> {
    let frame = Frame {
        output: OUT,
        layers,
    };
    let p = plan(&frame, caps, &Buffers)?;
    let mut out: Placed = p
        .assignments()
        .iter()
        .map(|a| (a.layer, a.plane.0, a.zpos as u32))
        .collect();
    out.sort_by_key(|&(_, _, z)| z);
    Ok(out)
}

fn reasons_of(r: Result<Placed, PlanError>) -> RejectReasons {
    match r {
        Err(PlanError::NoPlane { reasons, .. }) => reasons,
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

/// zpos strictly increases from the bottom layer to the top.
fn assert_stacked(p: &[(usize, u32, u32)]) {
    assert!(p.windows(2).all(|w| w[0].2 < w[1].2), "{p:?}");
}

// ---- the fixtures carry the recorded properties ----

#[test]
fn fixtures_carry_plane_alpha_and_all_three_blend_modes() {
    for caps in [rk_caps(), pi5_caps()] {
        assert!(!caps.planes.is_empty());
        for p in caps.planes.iter() {
            assert!(p.plane_alpha, "plane {}", p.id.0);
            assert_eq!(p.blend_modes, ALL_BLEND, "plane {}", p.id.0);
        }
    }
    assert_eq!(rk_caps().planes.len(), 8);
    assert_eq!(pi5_caps().planes.len(), 16);
}

#[test]
fn generic_leaves_alpha_and_blend_as_recorded() {
    for caps in [rk_caps(), pi5_caps()] {
        assert_eq!(generic(caps), caps);
    }
}

// ---- every alpha and blend value is accepted where the properties exist ----

#[test]
fn rk3588_accepts_every_alpha_and_blend_combination_on_both_plane_classes() {
    let caps = generic(rk_caps());
    for blend in MODES {
        for alpha in [0, 1, 0xFF, HALF, 0xFEFF, ALPHA_OPAQUE] {
            // AFBC ARGB can only be a Cluster plane (33 is first), linear ARGB
            // an Esmart plane (57 is first): the plane class does not change
            // what is accepted.
            let p = placed(&caps, &[layer(AR_AFBC, 0, alpha, blend)]).unwrap();
            assert_eq!(p, [(0, 33, 0)], "{alpha:#x} {blend:?}");
            let p = placed(&caps, &[layer(AR_LIN, 0, alpha, blend)]).unwrap();
            assert_eq!(p, [(0, 57, 0)], "{alpha:#x} {blend:?}");
        }
    }
}

#[test]
fn bcm2712_accepts_every_alpha_and_blend_combination_on_the_primary() {
    let caps = generic(pi5_caps());
    for blend in MODES {
        for alpha in [0, 1, 0xFF, HALF, 0xFEFF, ALPHA_OPAQUE] {
            let p = placed(&caps, &[layer(AR_LIN, 0, alpha, blend)]).unwrap();
            // Primary 83 (zpos fixed 0) is the first plane that reaches the
            // output, and it takes a translucent layer like any other.
            assert_eq!(p, [(0, 83, 0)], "{alpha:#x} {blend:?}");
        }
    }
}

// ---- stacking: translucent, premultiplied, coverage and opaque layers ----

#[test]
fn rk3588_translucent_layers_over_an_opaque_one_take_successive_zpos() {
    let caps = generic(rk_caps());
    // Bottom: opaque XRGB; then premultiplied ARGB at half alpha; then
    // coverage ARGB at half alpha. All linear, so all on Esmart planes.
    let layers = [
        layer(XR_LIN, 0, ALPHA_OPAQUE, BlendMode::Opaque),
        layer(AR_LIN, 1, HALF, BlendMode::Premultiplied),
        layer(AR_LIN, 2, HALF, BlendMode::Coverage),
    ];
    let p = placed(&caps, &layers).unwrap();
    assert_eq!(p, [(0, 57, 0), (1, 63, 1), (2, 75, 2)]);
}

#[test]
fn rk3588_opaque_layers_over_translucent_ones_are_still_placed() {
    let caps = generic(rk_caps());
    // Premultiplied and coverage translucent layers, then an opaque XRGB
    // layer on top that hides both: the planner does not drop layers that
    // are covered, nor does it keep translucent layers off any plane.
    let layers = [
        layer(AR_LIN, 0, HALF, BlendMode::Premultiplied),
        layer(AR_LIN, 1, HALF, BlendMode::Coverage),
        layer(XR_LIN, 2, ALPHA_OPAQUE, BlendMode::Opaque),
    ];
    let p = placed(&caps, &layers).unwrap();
    assert_eq!(p, [(0, 57, 0), (1, 63, 1), (2, 75, 2)]);
    assert_stacked(&p);
}

#[test]
fn rk3588_a_translucent_afbc_layer_over_a_linear_one_lands_on_a_cluster_plane() {
    let caps = generic(rk_caps());
    // Linear opaque bottom on Esmart 57 (zpos 0); the AFBC translucent top
    // needs a Cluster plane, and the first one (the primary 33) gets zpos 1:
    // the planner gives the primary no special bottom role.
    let layers = [
        layer(XR_LIN, 0, ALPHA_OPAQUE, BlendMode::Opaque),
        layer(AR_AFBC, 1, HALF, BlendMode::Premultiplied),
    ];
    let p = placed(&caps, &layers).unwrap();
    assert_eq!(p, [(0, 57, 0), (1, 33, 1)]);
}

#[test]
fn rk3588_frame_order_does_not_matter_only_z_order_does() {
    let caps = generic(rk_caps());
    let bottom = layer(XR_LIN, 0, ALPHA_OPAQUE, BlendMode::Opaque);
    let top = layer(AR_LIN, 5, HALF, BlendMode::Premultiplied);
    let a = placed(&caps, &[bottom, top]).unwrap();
    let b = placed(&caps, &[top, bottom]).unwrap();
    // Layer indices differ, the planes and zpos do not.
    assert_eq!(a, [(0, 57, 0), (1, 63, 1)]);
    assert_eq!(b, [(1, 57, 0), (0, 63, 1)]);
}

#[test]
fn bcm2712_primary_hosts_a_translucent_bottom_layer_and_overlays_follow() {
    let caps = generic(pi5_caps());
    let layers = [
        layer(AR_LIN, 0, HALF, BlendMode::Premultiplied),
        layer(AR_LIN, 1, HALF, BlendMode::Coverage),
        layer(XR_LIN, 2, ALPHA_OPAQUE, BlendMode::Opaque),
        layer(AR_LIN, 3, ALPHA_OPAQUE, BlendMode::Premultiplied),
    ];
    let p = placed(&caps, &layers).unwrap();
    // The primary is fixed at zpos 0 and is the bottom layer's plane; the
    // overlays (mutable 1..=17) take zpos 1, 2, 3 in id order.
    assert_eq!(p, [(0, 83, 0), (1, 107, 1), (2, 118, 2), (3, 129, 3)]);
}

#[test]
fn bcm2712_places_twelve_alternating_layers_in_one_stack() {
    let caps = generic(pi5_caps());
    // 12 layers: the primary plus eleven of the twelve recorded overlays.
    let layers: Vec<Layer> = (0..12)
        .map(|i| match i % 3 {
            0 => layer(AR_LIN, i, HALF, BlendMode::Premultiplied),
            1 => layer(AR_LIN, i, HALF, BlendMode::Coverage),
            _ => layer(XR_LIN, i, ALPHA_OPAQUE, BlendMode::Opaque),
        })
        .collect();
    let p = placed(&caps, &layers).unwrap();
    assert_eq!(p.len(), 12);
    assert_stacked(&p);
    assert_eq!(p[0], (0, 83, 0));
    for (n, entry) in p.iter().enumerate().skip(1) {
        assert_eq!(*entry, (n, PI5_OVERLAYS[n - 1], n as u32));
    }
}

#[test]
fn bcm2712_a_second_layer_never_uses_the_fixed_zpos_primary() {
    // The only primary on the output has zpos fixed 0 and the first layer
    // already holds it: the second layer needs an overlay whatever its alpha
    // or blend mode, and with none reachable it has no plane.
    let mut caps = pi5_caps();
    for p in caps.planes.as_mut_slice() {
        if p.kind == PlaneKind::Overlay {
            p.outputs = 0;
        }
    }
    let caps = generic(caps);
    let layers = [
        layer(AR_LIN, 0, HALF, BlendMode::Premultiplied),
        layer(XR_LIN, 1, ALPHA_OPAQUE, BlendMode::Opaque),
    ];
    assert!(matches!(
        placed(&caps, &layers),
        Err(PlanError::NoPlane { .. })
    ));
    assert_eq!(placed(&caps, &layers[..1]).unwrap(), [(0, 83, 0)]);
}

// ---- the planner honours what the plane reports ----

#[test]
fn a_plane_without_plane_alpha_refuses_translucent_layers_only() {
    for caps in [rk_caps(), pi5_caps()] {
        let mut bare = caps;
        for p in bare.planes.as_mut_slice() {
            p.plane_alpha = false;
        }
        let bare = generic(bare);
        let reasons = reasons_of(placed(&bare, &[layer(XR_LIN, 0, HALF, BlendMode::Opaque)]));
        assert!(reasons.contains(RejectReasons::ALPHA), "{reasons:?}");
        // An opaque plane alpha needs no support.
        let p = placed(
            &bare,
            &[layer(AR_LIN, 0, ALPHA_OPAQUE, BlendMode::Premultiplied)],
        );
        assert_eq!(p.unwrap().len(), 1);
    }
}

#[test]
fn a_plane_missing_a_blend_mode_refuses_layers_that_need_it() {
    for caps in [rk_caps(), pi5_caps()] {
        // Premultiplied only, as the STM32 LTDC reports.
        let mut premult = caps;
        for p in premult.planes.as_mut_slice() {
            p.blend_modes = BlendMode::Premultiplied.mask();
        }
        let premult = generic(premult);
        let reasons = reasons_of(placed(
            &premult,
            &[layer(AR_LIN, 0, ALPHA_OPAQUE, BlendMode::Coverage)],
        ));
        assert!(reasons.contains(RejectReasons::BLEND), "{reasons:?}");
        // An ARGB layer asked to ignore its alpha needs the "none" mode...
        let reasons = reasons_of(placed(
            &premult,
            &[layer(AR_LIN, 0, ALPHA_OPAQUE, BlendMode::Opaque)],
        ));
        assert!(reasons.contains(RejectReasons::BLEND), "{reasons:?}");
        // ...an XRGB layer does not.
        let p = placed(
            &premult,
            &[layer(XR_LIN, 0, ALPHA_OPAQUE, BlendMode::Opaque)],
        );
        assert_eq!(p.unwrap().len(), 1);
        let p = placed(
            &premult,
            &[layer(AR_LIN, 0, HALF, BlendMode::Premultiplied)],
        );
        assert_eq!(p.unwrap().len(), 1);
    }
}

#[test]
fn rk3588_opaque_xrgb_layers_need_no_blend_support() {
    // XRGB is known opaque: an Opaque layer needs no blend support, and
    // planes with no blend modes at all still take it, AFBC or linear.
    let mut caps = rk_caps();
    for p in caps.planes.as_mut_slice() {
        p.blend_modes = 0;
    }
    let caps = generic(caps);
    let layers = [
        layer(XR_AFBC, 0, ALPHA_OPAQUE, BlendMode::Opaque),
        layer(XR_LIN, 1, ALPHA_OPAQUE, BlendMode::Opaque),
    ];
    assert_eq!(placed(&caps, &layers).unwrap(), [(0, 33, 0), (1, 57, 1)]);
}
