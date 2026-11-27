//! The M2 reference scene, planned on both topologies the M2 comparison
//! touches (`docs/m2-comparison-plan.md` §2; `SoW.md` WP3).
//!
//! Purpose: the Linux side (`composition-drm`) and the Zephyr side (an engine
//! build on the M7) both claim to run "the same scene". This test fixes what
//! the engine plans for that scene, so either side can check it plans the
//! same thing. It is a host test of the planner. It measures nothing, runs on
//! no board and is no hardware claim.
//!
//! Topologies (both are *read from source / recorded in docs*, not observed
//! here):
//!
//! - `linux_dpu`: the i.MX95 DPU as seen by one CRTC under the NXP vendor
//!   kernel: primary + 5 overlays, mutable zpos 0..5, plane alpha, three
//!   blend modes, linear only, upscale only, no rotation, and the shared
//!   resource groups of `DriverProfile::Imx95Dpu`
//!   (`docs/platform-capabilities.md`, `docs/imx95-dpu-source-6.6.md`).
//!   The format list is a subset of the driver's 13 (the ones this test
//!   needs plus a few neighbours).
//! - `zephyr_stock`: the stock upstream `socionext,dpu` driver as
//!   `docs/imx95-twoplane-port.md` records it: one plane, RGB565 / RGB888 /
//!   ARGB8888, no scaling, no alpha, no zpos, no way to turn the plane off.
//!   Stage B has no second plane there; the test pins that as a refusal.
//!
//! Open points of the comparison plan and the reading chosen here (simplest
//! reading each time):
//!
//! 1. `W` x `H` are "the panel's native mode", not measured. Chosen:
//!    1280 x 800 at 60 Hz, the mode the docs derive for the panel by
//!    inference. Only the shape of the plan depends on it.
//! 2. Stage B `top` is "about W/3 x H/3, offset from the centre". Chosen:
//!    426 x 266 at (500, 300), fully inside `base`, so the "overlap at least
//!    a quarter" rule is met trivially.
//! 3. "premultiplied or straight as the plane reports it": chosen
//!    `BlendMode::Premultiplied`, the Linux driver's default mode.
//! 4. Stage A `XRGB8888` versus the stock Zephyr driver's list: the plan says
//!    the format becomes the one the stock driver exposes on both sides.
//!    Chosen: `ARGB8888` for that variant, and the test pins that the
//!    `XRGB8888` scene is refused on the stock topology (`FORMAT`).
//! 5. Plane ids and which of the six planes gets which layer are not
//!    specified by the plan. The planner is deterministic, so the test pins
//!    the properties of the plan it makes (primary takes the base, distinct
//!    planes, zpos order), not the exact ids of the overlay. Ids 1..=6 are
//!    arbitrary test ids; the primary is 1.
//! 6. Plane alpha "one half": `0x8000` of `0xFFFF`.
//! 7. "Fallback" in the engine means a refusal: the planner never composes
//!    in software (`planner.rs` module doc). The fallback cases here are
//!    therefore `PlanError::NoPlane` with the reason bits.
//! 8. Not modelled by the planner and so not tested: the Linux driver
//!    ignores alpha on the bottom plane (`imx95-dpu-source-6.6.md` item 10).

use composition_core::planner::plan;
use composition_core::{
    Assignment, BlendMode, BufferId, BufferInfo, BufferLookup, Capabilities, FormatModifier, Frame,
    Layer, LayerTraits, Modifier, OutputCaps, OutputId, PixelFormat, Plan, PlanError, PlaneCaps,
    PlaneId, PlaneKind, Rect, RejectReasons, ResourceGroup, ScaleLimits, Size, Transform, Zpos,
};

const W: u32 = 1280;
const H: u32 = 800;
const OUT: OutputId = OutputId(0);
const BASE_BUF: BufferId = BufferId(1);
const TOP_BUF: BufferId = BufferId(2);
const TOP_W: u32 = W / 3;
const TOP_H: u32 = H / 3;
const HALF_ALPHA: u16 = 0x8000;
const RGB888: PixelFormat = PixelFormat::fourcc(b"RG24");

struct Buffers(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

fn info(w: u32, h: u32, format: PixelFormat) -> BufferInfo {
    BufferInfo {
        size: Size::new(w, h),
        format,
        modifier: Modifier::LINEAR,
    }
}

fn output(caps: &mut Capabilities) {
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(W, H),
            refresh_mhz: 60_000,
        })
        .unwrap();
}

fn linux_dpu() -> Capabilities {
    let mut caps = Capabilities::default();
    output(&mut caps);
    let formats = [
        PixelFormat::ARGB8888,
        PixelFormat::XRGB8888,
        PixelFormat::ABGR8888,
        PixelFormat::XBGR8888,
        PixelFormat::RGB565,
        PixelFormat::YUYV,
        PixelFormat::UYVY,
        PixelFormat::NV12,
        PixelFormat::NV21,
    ];
    for i in 0..6u32 {
        let mut p = PlaneCaps {
            id: PlaneId(i + 1),
            kind: if i == 0 {
                PlaneKind::Primary
            } else {
                PlaneKind::Overlay
            },
            outputs: 1,
            scaling: Some(ScaleLimits {
                max_upscale: 2048 << 16,
                max_downscale: 1 << 16,
            }),
            max_source: Some(Size::new(8192, 8192)),
            plane_alpha: true,
            blend_modes: BlendMode::Opaque.mask()
                | BlendMode::Premultiplied.mask()
                | BlendMode::Coverage.mask(),
            transforms: Transform::Normal.mask(),
            zpos: Zpos::Range { min: 0, max: 5 },
            ..PlaneCaps::default()
        };
        for f in formats {
            p.formats.push(FormatModifier::linear(f)).unwrap();
        }
        caps.planes.push(p).unwrap();
    }
    let all = 0b11_1111;
    let groups = [
        (LayerTraits::ANY, all, 6),
        (LayerTraits::SCALED.union(LayerTraits::YUV), all, 4),
        (LayerTraits::SCALED_H, all, 1),
        (LayerTraits::SCALED_V, all, 1),
    ];
    for (consumers, planes, capacity) in groups {
        caps.resource_groups
            .push(ResourceGroup {
                consumers,
                planes,
                capacity,
            })
            .unwrap();
    }
    caps
}

fn zephyr_stock() -> Capabilities {
    let mut caps = Capabilities::default();
    output(&mut caps);
    let mut p = PlaneCaps {
        id: PlaneId(1),
        kind: PlaneKind::Primary,
        outputs: 1,
        transforms: Transform::Normal.mask(),
        blend_modes: BlendMode::Opaque.mask(),
        zpos: Zpos::Fixed(0),
        full_output_only: true,
        ..PlaneCaps::default()
    };
    for f in [PixelFormat::RGB565, RGB888, PixelFormat::ARGB8888] {
        p.formats.push(FormatModifier::linear(f)).unwrap();
    }
    caps.planes.push(p).unwrap();
    caps
}

fn base_layer() -> Layer {
    Layer::new(BASE_BUF, Rect::new(0, 0, W, H), Rect::new(0, 0, W, H), 0)
}

fn top_layer(x: i32, y: i32) -> Layer {
    let mut l = Layer::new(
        TOP_BUF,
        Rect::new(0, 0, TOP_W, TOP_H),
        Rect::new(x, y, TOP_W, TOP_H),
        1,
    );
    l.blend = BlendMode::Premultiplied;
    l
}

fn buffers(base: PixelFormat) -> Buffers {
    Buffers(vec![
        (BASE_BUF, info(W, H, base)),
        (TOP_BUF, info(TOP_W, TOP_H, PixelFormat::ARGB8888)),
    ])
}

fn run(caps: &Capabilities, layers: &[Layer], base: PixelFormat) -> Result<Plan, PlanError> {
    plan(
        &Frame {
            output: OUT,
            layers,
        },
        caps,
        &buffers(base),
    )
}

fn assign(p: &Plan) -> Vec<(usize, u32, i32)> {
    p.assignments()
        .iter()
        .map(|a: &Assignment| (a.layer, a.plane.0, a.zpos))
        .collect()
}

fn no_plane(err: PlanError, want_layer: usize, want: RejectReasons) {
    match err {
        PlanError::NoPlane { layer, reasons } => {
            assert_eq!(layer, want_layer);
            assert!(reasons.contains(want), "{reasons:?}");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn stage_a_one_plane_linux_dpu() {
    let p = run(&linux_dpu(), &[base_layer()], PixelFormat::XRGB8888).unwrap();
    let a = p.assignments();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].layer, 0);
    // The primary plane takes the only layer, unscaled, full panel.
    assert_eq!(a[0].plane, PlaneId(1));
    assert_eq!(a[0].source, Rect::new(0, 0, W, H));
    assert_eq!(a[0].destination, Rect::new(0, 0, W, H));
}

#[test]
fn stage_a_zephyr_stock_needs_the_format_substitution() {
    let layers = [base_layer()];
    // The scene's XRGB8888 is not in the stock driver's list: refused.
    let err = run(&zephyr_stock(), &layers, PixelFormat::XRGB8888).unwrap_err();
    no_plane(err, 0, RejectReasons::FORMAT);
    // The agreed substitution (plan §2.1) plans on the one plane, and both
    // sides then give the same layer, plane and destination.
    let z = run(&zephyr_stock(), &layers, PixelFormat::ARGB8888).unwrap();
    let l = run(&linux_dpu(), &layers, PixelFormat::ARGB8888).unwrap();
    assert_eq!(z.assignments().len(), 1);
    assert_eq!(z.assignments()[0].plane, PlaneId(1));
    assert_eq!(z.assignments()[0].layer, l.assignments()[0].layer);
    assert_eq!(z.assignments()[0].plane, l.assignments()[0].plane);
    assert_eq!(
        z.assignments()[0].destination,
        l.assignments()[0].destination
    );
}

#[test]
fn stage_b_opaque_linux_dpu() {
    let layers = [base_layer(), top_layer(500, 300)];
    let p = run(&linux_dpu(), &layers, PixelFormat::XRGB8888).unwrap();
    let a = p.assignments();
    assert_eq!(a.len(), 2);
    // Bottom to top, two distinct planes, strictly increasing zpos.
    assert_eq!((a[0].layer, a[1].layer), (0, 1));
    assert_ne!(a[0].plane, a[1].plane);
    assert!(a[0].zpos < a[1].zpos);
    assert_eq!(a[0].plane, PlaneId(1), "base on the primary plane");
    assert_eq!(a[1].destination, Rect::new(500, 300, TOP_W, TOP_H));
    assert_eq!(a[1].source, Rect::new(0, 0, TOP_W, TOP_H));
}

#[test]
fn stage_b_alpha_linux_dpu() {
    let mut top = top_layer(500, 300);
    top.alpha = HALF_ALPHA;
    let p = run(&linux_dpu(), &[base_layer(), top], PixelFormat::XRGB8888).unwrap();
    let opaque = run(
        &linux_dpu(),
        &[base_layer(), top_layer(500, 300)],
        PixelFormat::XRGB8888,
    )
    .unwrap();
    // Plane alpha costs no extra plane: same assignment as the opaque case.
    assert_eq!(assign(&p), assign(&opaque));
}

#[test]
fn stage_b_move_keeps_the_assignment() {
    // Only the destination position changes between frames (sub-scene 3),
    // so layer-to-plane assignment and zpos must stay the same.
    let first = run(
        &linux_dpu(),
        &[base_layer(), top_layer(500, 300)],
        PixelFormat::XRGB8888,
    )
    .unwrap();
    let far = (W as i32 - TOP_W as i32, H as i32 - TOP_H as i32);
    for (x, y) in [(0, 0), (300, 100), far] {
        let p = run(
            &linux_dpu(),
            &[base_layer(), top_layer(x, y)],
            PixelFormat::XRGB8888,
        )
        .unwrap();
        assert_eq!(assign(&p), assign(&first), "at {x},{y}");
        assert_eq!(
            p.assignments()[1].destination,
            Rect::new(x, y, TOP_W, TOP_H)
        );
    }
}

#[test]
fn stage_b_zswap_linux_dpu() {
    // top below base: z_order 0 for top, 1 for base. zpos is mutable here.
    let mut base = base_layer();
    base.z_order = 1;
    let mut top = top_layer(500, 300);
    top.z_order = 0;
    let p = run(&linux_dpu(), &[base, top], PixelFormat::XRGB8888).unwrap();
    let a = p.assignments();
    assert_eq!(a.len(), 2);
    // Bottom to top: the top layer (index 1) first, then the base (index 0).
    assert_eq!((a[0].layer, a[1].layer), (1, 0));
    assert!(a[0].zpos < a[1].zpos);
}

#[test]
fn stage_b_zephyr_stock_is_refused() {
    // One plane: the second layer finds it taken. The planner refuses; there
    // is no software composition.
    let err = run(
        &zephyr_stock(),
        &[base_layer(), top_layer(500, 300)],
        PixelFormat::ARGB8888,
    )
    .unwrap_err();
    match err {
        PlanError::NoPlane { layer, reasons } => {
            assert!(layer <= 1);
            assert!(!reasons.is_empty());
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn stage_b_zephyr_stock_alpha_is_refused_even_alone() {
    // The stock plane has no plane alpha: refused for ALPHA.
    let mut l = base_layer();
    l.alpha = HALF_ALPHA;
    let err = run(&zephyr_stock(), &[l], PixelFormat::ARGB8888).unwrap_err();
    no_plane(err, 0, RejectReasons::ALPHA);
}

#[test]
fn out_of_scope_combinations_are_refused_on_linux_dpu() {
    // Plan §2.2: scaling and rotation are out of the comparison. If they are
    // tried, the engine refuses explicitly (SoW WP3), it does not compose.
    let caps = linux_dpu();

    // Downscale: the DPU is upscale-only.
    let mut down = top_layer(500, 300);
    down.source = Rect::new(0, 0, TOP_W * 2, TOP_H * 2);
    let bufs = Buffers(vec![
        (BASE_BUF, info(W, H, PixelFormat::XRGB8888)),
        (TOP_BUF, info(TOP_W * 2, TOP_H * 2, PixelFormat::ARGB8888)),
    ]);
    let frame = Frame {
        output: OUT,
        layers: &[base_layer(), down],
    };
    no_plane(
        plan(&frame, &caps, &bufs).unwrap_err(),
        1,
        RejectReasons::SCALING,
    );

    // Rotation: no rotation property on the plane.
    let mut rot = top_layer(500, 300);
    rot.transform = Transform::Rotate90;
    let err = run(&caps, &[base_layer(), rot], PixelFormat::XRGB8888).unwrap_err();
    no_plane(err, 1, RejectReasons::TRANSFORM);
}

#[test]
fn planning_is_deterministic() {
    let layers = [base_layer(), top_layer(500, 300)];
    let a = run(&linux_dpu(), &layers, PixelFormat::XRGB8888).unwrap();
    let b = run(&linux_dpu(), &layers, PixelFormat::XRGB8888).unwrap();
    assert_eq!(a, b);
}
