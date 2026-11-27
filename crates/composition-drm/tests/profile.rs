//! Edge cases of `DriverProfile::apply`, on hand-built capabilities.

use composition_core::limits::{MAX_PLANES, MAX_RESOURCE_GROUPS};
use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, LayerTraits,
    Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, ResourceGroup, ScaleLimits, Size, Transform,
};
use composition_drm::DriverProfile;

fn caps_of(kinds: &[PlaneKind]) -> Capabilities {
    let mut caps = Capabilities::default();
    for (i, kind) in kinds.iter().enumerate() {
        caps.planes
            .push(PlaneCaps {
                id: PlaneId(i as u32),
                kind: *kind,
                outputs: 1,
                ..PlaneCaps::default()
            })
            .unwrap();
    }
    caps
}

fn applied(kinds: &[PlaneKind]) -> Capabilities {
    let mut caps = caps_of(kinds);
    DriverProfile::Imx95Dpu.apply(&mut caps);
    caps
}

fn mask(n: usize) -> u32 {
    ((1u64 << n) - 1) as u32
}

fn check_invariants(caps: &Capabilities) {
    let all = mask(caps.planes.len());
    for g in caps.resource_groups.as_slice() {
        assert_eq!(g.planes & !all, 0, "group names a missing plane");
    }
    for p in caps.planes.as_slice() {
        if p.kind == PlaneKind::Cursor {
            assert_eq!(p.scaling, None);
            assert_eq!(p.max_source, None);
        } else {
            assert_eq!(
                p.scaling,
                Some(ScaleLimits {
                    max_upscale: 2048 << 16,
                    max_downscale: 1 << 16
                })
            );
            assert_eq!(p.max_source, Some(Size::new(8192, 8192)));
        }
    }
}

#[test]
fn generic_leaves_populated_caps_unchanged() {
    let mut caps = caps_of(&[PlaneKind::Primary, PlaneKind::Overlay, PlaneKind::Cursor]);
    let before = caps;
    DriverProfile::Generic.apply(&mut caps);
    assert_eq!(caps, before);
}

#[test]
fn zero_planes() {
    let caps = applied(&[]);
    assert!(caps.planes.is_empty());
    check_invariants(&caps);
}

#[test]
fn one_plane() {
    let caps = applied(&[PlaneKind::Primary]);
    assert_eq!(caps.resource_groups.len(), 4);
    assert!(caps.resource_groups.iter().all(|g| g.planes == 1));
    check_invariants(&caps);
}

#[test]
fn only_cursor_planes() {
    let caps = applied(&[PlaneKind::Cursor, PlaneKind::Cursor]);
    check_invariants(&caps);
}

#[test]
fn mixed_planes() {
    let caps = applied(&[
        PlaneKind::Primary,
        PlaneKind::Overlay,
        PlaneKind::Cursor,
        PlaneKind::Overlay,
    ]);
    assert!(caps.resource_groups.iter().all(|g| g.planes == 0b1111));
    check_invariants(&caps);
}

#[test]
fn maximum_plane_count() {
    // The plane limit is below 32, so the shift in `apply` cannot overflow
    // through `Capabilities`.
    const { assert!(MAX_PLANES < 32) };
    let caps = applied(&[PlaneKind::Overlay; MAX_PLANES]);
    assert_eq!(caps.planes.len(), MAX_PLANES);
    assert!(caps
        .resource_groups
        .iter()
        .all(|g| g.planes == mask(MAX_PLANES)));
    check_invariants(&caps);
}

/// Regression: a second `apply` used to push the four groups again.
#[test]
fn applying_twice_is_idempotent() {
    let mut caps = caps_of(&[PlaneKind::Primary, PlaneKind::Overlay, PlaneKind::Cursor]);
    DriverProfile::Imx95Dpu.apply(&mut caps);
    let once = caps;
    DriverProfile::Imx95Dpu.apply(&mut caps);
    assert_eq!(caps, once, "second apply changed the caps");
    DriverProfile::Imx95Dpu.apply(&mut caps);
    assert_eq!(caps, once);
}

#[test]
fn full_group_table_does_not_panic_and_keeps_existing_groups() {
    let mut caps = caps_of(&[PlaneKind::Primary, PlaneKind::Overlay]);
    let own = ResourceGroup {
        consumers: LayerTraits::YUV,
        planes: 0b10,
        capacity: 1,
    };
    for _ in 0..MAX_RESOURCE_GROUPS {
        caps.resource_groups.push(own).unwrap();
    }
    DriverProfile::Imx95Dpu.apply(&mut caps);
    assert_eq!(caps.resource_groups.len(), MAX_RESOURCE_GROUPS);
    assert!(caps.resource_groups.iter().all(|g| *g == own));
}

// ---- DriverProfile::Stm32Ltdc ----

const LTDC_LIMITS: ScaleLimits = ScaleLimits {
    max_upscale: 1080 << 16,
    max_downscale: 1 << 16,
};
const OUT: OutputId = OutputId(0);
const FULL: Rect = Rect::new(0, 0, 1920, 1080);
const RGB: BufferId = BufferId(1);
const SMALL: BufferId = BufferId(2);
const TINY: BufferId = BufferId(3);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let size = match buffer {
            RGB => Size::new(1920, 1080),
            SMALL => Size::new(960, 540),
            // One pixel: any destination width is the factor.
            TINY => Size::new(1, 1),
            _ => return None,
        };
        Some(BufferInfo {
            size,
            format: PixelFormat::XRGB8888,
            modifier: Modifier::LINEAR,
        })
    }
}

/// The board's topology (observed 2026-11-27): primary, overlay, overlay,
/// on one output, with unscaled planes as KMS reports them.
fn ltdc_caps() -> Capabilities {
    let mut caps = caps_of(&[PlaneKind::Primary, PlaneKind::Overlay, PlaneKind::Overlay]);
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(1920, 1080),
            refresh_mhz: 60_000,
        })
        .unwrap();
    for plane in caps.planes.as_mut_slice() {
        plane
            .formats
            .push(FormatModifier::linear(PixelFormat::XRGB8888))
            .unwrap();
    }
    DriverProfile::Stm32Ltdc.apply(&mut caps);
    caps
}

fn plan_one(layer: Layer) -> Result<usize, PlanError> {
    let layers = [layer];
    let frame = Frame {
        output: OUT,
        layers: &layers,
    };
    plan(&frame, &ltdc_caps(), &Buffers).map(|p| p.assignments().len())
}

#[test]
fn ltdc_limits_on_the_scaling_planes_only() {
    let caps = ltdc_caps();
    let planes = caps.planes.as_slice();
    assert_eq!(planes[0].scaling, Some(LTDC_LIMITS));
    assert_eq!(planes[1].scaling, Some(LTDC_LIMITS));
    assert_eq!(planes[2].scaling, None);
    assert!(planes.iter().all(|p| p.max_source.is_none()));
    assert!(caps.resource_groups.is_empty());
}

#[test]
fn ltdc_cursor_and_later_overlays_are_unclaimed() {
    let mut caps = caps_of(&[
        PlaneKind::Overlay,
        PlaneKind::Cursor,
        PlaneKind::Primary,
        PlaneKind::Overlay,
        PlaneKind::Overlay,
    ]);
    DriverProfile::Stm32Ltdc.apply(&mut caps);
    let scaling: Vec<_> = caps.planes.iter().map(|p| p.scaling).collect();
    assert_eq!(
        scaling,
        [Some(LTDC_LIMITS), None, Some(LTDC_LIMITS), None, None]
    );
}

#[test]
fn ltdc_zero_and_one_plane() {
    let mut caps = caps_of(&[]);
    DriverProfile::Stm32Ltdc.apply(&mut caps);
    assert!(caps.planes.is_empty());
    let mut caps = caps_of(&[PlaneKind::Primary]);
    DriverProfile::Stm32Ltdc.apply(&mut caps);
    assert_eq!(caps.planes.as_slice()[0].scaling, Some(LTDC_LIMITS));
}

#[test]
fn ltdc_downscale_is_refused_by_the_planner() {
    let shrunk = Layer::new(RGB, FULL, Rect::new(0, 0, 960, 540), 0);
    assert_eq!(
        plan_one(shrunk),
        Err(PlanError::NoPlane {
            layer: 0,
            reasons: RejectReasons::SCALING
        })
    );
}

#[test]
fn ltdc_upscale_at_the_bound_accepted_and_above_refused() {
    let dst = Rect::new(0, 0, 960, 540);
    // 2x up.
    let two = Layer::new(SMALL, Rect::new(0, 0, 480, 270), dst, 0);
    assert_eq!(plan_one(two), Ok(1));
    // 1x1 to 1080x1: 1080.0x horizontally (observed 2026-11-27: no bound
    // up to the mode size).
    let at = Layer::new(TINY, Rect::new(0, 0, 1, 1), Rect::new(0, 0, 1080, 1), 0);
    assert_eq!(plan_one(at), Ok(1));
    // 1x1 to 1081x1: 1081.0x horizontally.
    let above = Layer::new(TINY, Rect::new(0, 0, 1, 1), Rect::new(0, 0, 1081, 1), 0);
    assert_eq!(
        plan_one(above),
        Err(PlanError::NoPlane {
            layer: 0,
            reasons: RejectReasons::SCALING
        })
    );
}

#[test]
fn ltdc_applying_twice_is_idempotent() {
    let mut caps = caps_of(&[PlaneKind::Primary, PlaneKind::Overlay, PlaneKind::Overlay]);
    DriverProfile::Stm32Ltdc.apply(&mut caps);
    let once = caps;
    DriverProfile::Stm32Ltdc.apply(&mut caps);
    assert_eq!(caps, once);
}

#[test]
fn ltdc_blends_straight_alpha_on_every_non_cursor_plane() {
    // Session 2026-11-27, Q2: straight alpha. KMS reports premultiplied
    // because the property is absent.
    let mut caps = caps_of(&[
        PlaneKind::Primary,
        PlaneKind::Overlay,
        PlaneKind::Cursor,
        PlaneKind::Overlay,
    ]);
    for plane in caps.planes.as_mut_slice() {
        plane.blend_modes = BlendMode::Premultiplied.mask();
        plane.plane_alpha = true;
    }
    DriverProfile::Stm32Ltdc.apply(&mut caps);
    let modes: Vec<u8> = caps.planes.iter().map(|p| p.blend_modes).collect();
    let straight = BlendMode::Coverage.mask();
    let pre = BlendMode::Premultiplied.mask();
    assert_eq!(modes, [straight, straight, pre, straight]);
    assert!(caps.planes.iter().all(|p| p.plane_alpha));
}

#[test]
fn ltdc_refuses_a_premultiplied_layer_and_takes_a_coverage_one() {
    let mut layer = Layer::new(RGB, FULL, FULL, 0);
    layer.blend = BlendMode::Premultiplied;
    let Err(PlanError::NoPlane { reasons, .. }) = plan_one(layer) else {
        panic!("premultiplied must be refused");
    };
    assert!(reasons.contains(RejectReasons::BLEND));
    layer.blend = BlendMode::Coverage;
    assert_eq!(plan_one(layer), Ok(1));
}

#[test]
fn ltdc_leaves_transforms_and_formats_as_reported() {
    // Q6 and Q9 were not run in the 2026-11-27 session: nothing is claimed.
    let mut caps = caps_of(&[PlaneKind::Primary, PlaneKind::Overlay]);
    let rot = Transform::Rotate180.mask() | Transform::FlipH.mask();
    for plane in caps.planes.as_mut_slice() {
        plane.transforms = rot;
        plane
            .formats
            .push(FormatModifier::linear(PixelFormat::XRGB8888))
            .unwrap();
    }
    let before = caps;
    DriverProfile::Stm32Ltdc.apply(&mut caps);
    for (b, a) in before.planes.iter().zip(caps.planes.iter()) {
        assert_eq!(b.transforms, a.transforms);
        assert_eq!(b.formats, a.formats);
    }
}
