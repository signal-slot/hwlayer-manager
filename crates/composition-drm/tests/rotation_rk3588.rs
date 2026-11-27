//! `DriverProfile::Generic` and rotated or reflected layers on the recorded
//! RK3588 (NanoPC-T6) topology.
//!
//! Baseline tests: they pin what the generic planner does with rotation and
//! reflection on the recorded planes, so that the later `Rk3588Vop2` tests show
//! the difference. The fixture is the one of `generic_rk3588.rs`, copied again
//! so that this file stands alone.
//!
//! Fixture data, copied here so that nothing reads `docs/observed`:
//! - `docs/observed/rk3588-topology.md`, observed 2026-11-28 (kernel
//!   6.18.55-current-rockchip64): plane kinds, outputs masks, zpos 0..=7, plane
//!   alpha, blend modes, and the probe's format lists (at most 32 pairs per
//!   plane, so the Cluster lists hold no linear pair). The state dump reads
//!   `rotation=1` (none) on every plane; the property's bitmask was not read.
//! - `docs/observed/rk3588-source.md`, read 2026-11-28 (vendor develop-6.1):
//!   ROTATE_90/270 and REFLECT_X/Y exist on Cluster main windows only, none on
//!   Esmart. A Cluster window without AFBC rejects REFLECT_X, ROTATE_90 and
//!   ROTATE_270; ROTATE_90 together with ROTATE_270 is rejected.
//! - Plan #457 scen-t6 handoff, observed 2026-11-27 (kernel
//!   6.18.55-current-rockchip64): 8 planes, zpos 0..7 on every plane, no plane
//!   scales, rotation and reflection only with AFBC on Cluster planes, planes 39
//!   and 69 cannot reach video_port0, and the kernel lease holds primary 33 plus
//!   overlays 45/51/57/63/69/75.
//!
//! The probe does not read the rotation property, so recorded `PlaneCaps` carry
//! `transforms == 0` and `Generic` adds nothing: every transform is refused. The
//! `caps_cluster_transforms` fixture stands for a probe or profile that sets the
//! mask on the Cluster planes; with it the planner follows the mask alone and
//! does not know that a transform needs AFBC.

use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, Size, Transform, Zpos,
};
use composition_drm::DriverProfile;

const OUT: OutputId = OutputId(0);

/// Arm AFBC modifiers as the probe printed them (observed 2026-11-28).
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

/// The probe's 32 pairs of a Cluster plane: 14 + 14 + 4.
fn cluster_pairs() -> Vec<FormatModifier> {
    let mut pairs = Vec::new();
    for (modifier, count) in [(AFBC_A, 14), (AFBC_B, 14), (AFBC_C, 4)] {
        for code in &CLUSTER_FORMATS[..count] {
            pairs.push(FormatModifier {
                format: PixelFormat::fourcc(code),
                modifier,
            });
        }
    }
    pairs
}

fn esmart_pairs() -> Vec<FormatModifier> {
    ESMART_FORMATS
        .iter()
        .map(|c| FormatModifier::linear(PixelFormat::fourcc(c)))
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Cluster,
    Esmart,
}

/// (plane id, kind, class, outputs mask on output 0)
const PLANES: [(u32, PlaneKind, Class, u32); 8] = [
    (33, PlaneKind::Primary, Class::Cluster, 0b1),
    (39, PlaneKind::Primary, Class::Cluster, 0b0),
    (45, PlaneKind::Overlay, Class::Cluster, 0b1),
    (51, PlaneKind::Overlay, Class::Cluster, 0b1),
    (57, PlaneKind::Overlay, Class::Esmart, 0b1),
    (63, PlaneKind::Overlay, Class::Esmart, 0b1),
    (69, PlaneKind::Overlay, Class::Esmart, 0b0),
    (75, PlaneKind::Overlay, Class::Esmart, 0b1),
];

/// Planes the kernel lease holds (observed 2026-11-27).
const LEASED: [u32; 7] = [33, 45, 51, 57, 63, 69, 75];

fn caps_with(ids: &[u32]) -> Capabilities {
    build(ids, 0, false)
}

/// `cluster_mask` is the transform mask of the Cluster planes; `cluster_linear`
/// puts a linear XRGB8888 pair on them (the recorded lists have none).
fn build(ids: &[u32], cluster_mask: u8, cluster_linear: bool) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(1920, 1080),
            refresh_mhz: 60_000,
        })
        .unwrap();
    for &(id, kind, class, outputs) in PLANES.iter().filter(|p| ids.contains(&p.0)) {
        let mut plane = PlaneCaps {
            id: PlaneId(id),
            kind,
            outputs,
            scaling: None,
            max_source: None,
            plane_alpha: true,
            blend_modes: BlendMode::Opaque.mask()
                | BlendMode::Premultiplied.mask()
                | BlendMode::Coverage.mask(),
            zpos: Zpos::Range { min: 0, max: 7 },
            transforms: if class == Class::Cluster {
                cluster_mask
            } else {
                0
            },
            ..PlaneCaps::default()
        };
        let pairs = match class {
            Class::Cluster => {
                let mut v = cluster_pairs();
                if cluster_linear {
                    // The format list holds 32 pairs: replace the last.
                    v.pop();
                    v.push(FormatModifier::linear(PixelFormat::XRGB8888));
                }
                v
            }
            Class::Esmart => esmart_pairs(),
        };
        for pair in pairs {
            plane.formats.push(pair).unwrap();
        }
        caps.planes.push(plane).unwrap();
    }
    caps
}

fn full_caps() -> Capabilities {
    let ids: Vec<u32> = PLANES.iter().map(|p| p.0).collect();
    caps_with(&ids)
}

fn leased_caps() -> Capabilities {
    caps_with(&LEASED)
}

fn generic(mut caps: Capabilities) -> Capabilities {
    DriverProfile::Generic.apply(&mut caps);
    caps
}

const LIN: BufferId = BufferId(1);
const AFBC: BufferId = BufferId(2);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let (size, format, modifier) = match buffer {
            LIN => (Size::new(64, 64), PixelFormat::XRGB8888, Modifier::LINEAR),
            AFBC => (Size::new(64, 64), PixelFormat::XRGB8888, AFBC_A),
            _ => return None,
        };
        Some(BufferInfo {
            size,
            format,
            modifier,
        })
    }
}

/// A 64x64 layer in its own cell, so unscaled and not overlapping.
fn layer(buffer: BufferId, n: usize) -> Layer {
    let x = (n as i32) * 100;
    Layer::new(
        buffer,
        Rect::new(0, 0, 64, 64),
        Rect::new(x, 0, 64, 64),
        n as i32,
    )
}

fn plane_ids(caps: &Capabilities, layers: &[Layer]) -> Result<Vec<u32>, PlanError> {
    let frame = Frame {
        output: OUT,
        layers,
    };
    let p = plan(&frame, caps, &Buffers)?;
    let mut ids: Vec<u32> = p.assignments().iter().map(|a| a.plane.0).collect();
    ids.sort_unstable();
    Ok(ids)
}

const CLUSTER_REACH: [u32; 3] = [33, 45, 51];
const REACHABLE: [u32; 6] = [33, 45, 51, 57, 63, 75];

fn no_plane_reasons(r: Result<Vec<u32>, PlanError>) -> RejectReasons {
    match r {
        Err(PlanError::NoPlane { reasons, .. }) => reasons,
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

const TRANSFORMS: [Transform; 5] = [
    Transform::Rotate90,
    Transform::Rotate180,
    Transform::Rotate270,
    Transform::FlipH,
    Transform::FlipV,
];

fn all_transforms_mask() -> u8 {
    TRANSFORMS.iter().fold(0, |m, t| m | t.mask())
}

/// The recorded planes as a probe that reads the rotation property would
/// report them: every transform on the Cluster planes, none on Esmart.
fn caps_cluster_transforms(ids: &[u32]) -> Capabilities {
    build(ids, all_transforms_mask(), false)
}

fn rotated(buffer: BufferId, n: usize, transform: Transform) -> Layer {
    let mut l = layer(buffer, n);
    l.transform = transform;
    l
}

fn transform_refused(r: Result<Vec<u32>, PlanError>) -> bool {
    no_plane_reasons(r).contains(RejectReasons::TRANSFORM)
}

// ---- recorded caps: no transform is advertised ----

#[test]
fn recorded_planes_carry_no_transforms_and_generic_keeps_it() {
    for caps in [full_caps(), leased_caps()] {
        assert!(caps.planes.iter().all(|p| p.transforms == 0));
        assert!(generic(caps).planes.iter().all(|p| p.transforms == 0));
    }
}

#[test]
fn generic_refuses_every_transform_with_cluster_planes_free() {
    // Linear lands on Esmart, AFBC on Cluster; both refuse for TRANSFORM
    // although the hardware rotates AFBC on Cluster. Rk3588Vop2 should accept
    // rotation and reflection for AFBC on a Cluster plane.
    for caps in [generic(full_caps()), generic(leased_caps())] {
        for t in TRANSFORMS {
            for buffer in [LIN, AFBC] {
                assert!(
                    transform_refused(plane_ids(&caps, &[rotated(buffer, 0, t)])),
                    "{t:?}"
                );
            }
        }
    }
}

#[test]
fn generic_refuses_every_transform_with_cluster_planes_taken() {
    // Only the Esmart planes remain.
    let caps = generic(caps_with(&[57, 63, 75]));
    for t in TRANSFORMS {
        assert!(
            transform_refused(plane_ids(&caps, &[rotated(LIN, 0, t)])),
            "{t:?}"
        );
        // AFBC has no plane here at all.
        let reasons = no_plane_reasons(plane_ids(&caps, &[rotated(AFBC, 0, t)]));
        assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
    }
}

#[test]
fn transform_on_one_layer_fails_the_frame_even_if_the_others_fit() {
    // The planner has no fallback: one refused layer is a NoPlane for the frame.
    let caps = generic(full_caps());
    let layers = [
        layer(AFBC, 0),
        rotated(LIN, 1, Transform::Rotate90),
        layer(LIN, 2),
    ];
    assert!(transform_refused(plane_ids(&caps, &layers)));
    let layers = [layer(AFBC, 0), layer(LIN, 2)];
    assert_eq!(plane_ids(&caps, &layers).unwrap().len(), 2);
}

#[test]
fn normal_transform_is_unaffected() {
    let caps = generic(full_caps());
    let l = rotated(AFBC, 0, Transform::Normal);
    assert_eq!(plane_ids(&caps, &[l]).unwrap().len(), 1);
}

// ---- a probe that reads the rotation property (Cluster planes only) ----

#[test]
fn afbc_rotation_goes_to_a_free_cluster_plane() {
    let caps = generic(caps_cluster_transforms(&REACHABLE));
    for t in TRANSFORMS {
        let ids = plane_ids(&caps, &[rotated(AFBC, 0, t)]).unwrap();
        assert_eq!(ids.len(), 1);
        assert!(CLUSTER_REACH.contains(&ids[0]), "{t:?} {ids:?}");
    }
}

#[test]
fn afbc_rotation_fails_when_the_cluster_planes_are_taken() {
    let caps = generic(caps_cluster_transforms(&REACHABLE));
    let three: Vec<Layer> = (0..3)
        .map(|i| rotated(AFBC, i, Transform::Rotate90))
        .collect();
    assert_eq!(plane_ids(&caps, &three).unwrap(), CLUSTER_REACH);
    // A fourth has no Cluster plane left; the Esmart planes lack the mask and
    // the AFBC pair.
    let four: Vec<Layer> = (0..4)
        .map(|i| rotated(AFBC, i, Transform::Rotate90))
        .collect();
    assert!(matches!(
        plane_ids(&caps, &four),
        Err(PlanError::NoPlane { .. })
    ));
}

#[test]
fn linear_rotation_is_refused_even_with_the_cluster_mask() {
    // The recorded Cluster lists hold no linear pair and the Esmart planes lack
    // the mask, so the hardware rule "a Cluster window without AFBC rejects
    // rotation" is never reached.
    let caps = generic(caps_cluster_transforms(&REACHABLE));
    for t in TRANSFORMS {
        assert!(
            transform_refused(plane_ids(&caps, &[rotated(LIN, 0, t)])),
            "{t:?}"
        );
    }
}

#[test]
fn planner_does_not_know_that_a_transform_needs_afbc() {
    // If a Cluster plane listed a linear pair next to the mask, the planner
    // would put a rotated linear layer there, though the kernel (vendor source:
    // a Cluster window without AFBC rejects REFLECT_X, ROTATE_90 and ROTATE_270)
    // would refuse it in TEST_ONLY. Rk3588Vop2 has to add the AFBC condition.
    let caps = generic(build(&[33], all_transforms_mask(), true));
    assert_eq!(
        plane_ids(&caps, &[rotated(LIN, 0, Transform::Rotate90)]).unwrap(),
        [33]
    );
}

#[test]
fn rotated_layer_still_fails_the_scaling_check_without_a_scaler() {
    // No recorded plane scales. A 90 degree rotation swaps the axes first, so a
    // square source with a square destination is unscaled and passes; a
    // stretched destination is refused for SCALING.
    let caps = generic(caps_cluster_transforms(&[33, 45, 51]));
    let l = |dst: Rect| {
        let mut l = Layer::new(AFBC, Rect::new(0, 0, 64, 64), dst, 0);
        l.transform = Transform::Rotate270;
        l
    };
    assert_eq!(
        plane_ids(&caps, &[l(Rect::new(0, 0, 64, 64))])
            .unwrap()
            .len(),
        1
    );
    let reasons = no_plane_reasons(plane_ids(&caps, &[l(Rect::new(0, 0, 128, 64))]));
    assert!(reasons.contains(RejectReasons::SCALING), "{reasons:?}");
}
