//! `DriverProfile::Rk3588Vop2` on the recorded NanoPC-T6 topology.
//!
//! These tests prove planner behaviour against a recorded description, not
//! hardware behaviour. Expected values follow the profile's table
//! (`src/profile/rk3588.rs`): board measurement first, then mainline v6.18.
//!
//! Fixture data, read from the committed board dumps (never docs/observed):
//! - `tests/data/rk3588/probe-raw-2026-11-27.txt`, observed 2026-11-27
//!   (kernel 6.18.55-current-rockchip64): the probe summary, loaded through
//!   `common::recorded`. Eight planes, one output 1920x1080; plane 39 cannot
//!   reach it (outputs 0b0); Cluster planes keep the first 32 AFBC pairs,
//!   Esmart planes their 21 linear pairs.
//! - `tests/data/rk3588/props-2026-11-27.txt`, observed 2026-11-27 (same
//!   kernel): the full IN_FORMATS list per plane, for the Q1 count.
//!
//! Window class is recognised as the profile does: AFBC (Arm vendor)
//! modifiers in the plane's formats mean Cluster. No KMS object id is used.

mod common;

use common::recorded::{self, VENDOR_ARM};
use composition_core::limits::MAX_FORMATS;
use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BufferId, BufferInfo, Capabilities, Frame, Layer, Modifier, PixelFormat, PlanError, PlaneCaps,
    Rect, RejectReasons, ScaleLimits, Size, Transform,
};
use composition_drm::DriverProfile;

// Recorded on the NanoPC-T6, 2026-11-27 (see the module comment).
const PROBE: &str = include_str!("data/rk3588/probe-raw-2026-11-27.txt");
// Recorded on the NanoPC-T6, 2026-11-27 (see the module comment).
const PROPS: &str = include_str!("data/rk3588/props-2026-11-27.txt");

const SCREEN: Rect = Rect::new(0, 0, 1920, 1080);

/// First AFBC modifier of the Cluster list (observed 2026-11-27).
const AFBC: Modifier = Modifier(0x0800_0000_0000_0001);

const AFBC_XR24: BufferId = BufferId(1);
const LINEAR_XR24: BufferId = BufferId(2);
const LINEAR_NV12: BufferId = BufferId(3);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let (format, modifier) = match buffer {
            AFBC_XR24 => (PixelFormat::XRGB8888, AFBC),
            LINEAR_XR24 => (PixelFormat::XRGB8888, Modifier::LINEAR),
            LINEAR_NV12 => (PixelFormat::NV12, Modifier::LINEAR),
            _ => return None,
        };
        // Larger than max_source, so that the source rectangle decides.
        Some(BufferInfo {
            size: Size::new(5000, 5000),
            format,
            modifier,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Window {
    Cluster,
    Esmart,
}

fn window_of(plane: &PlaneCaps) -> Window {
    if plane
        .formats
        .iter()
        .any(|f| (f.modifier.0 >> 56) as u8 == VENDOR_ARM)
    {
        Window::Cluster
    } else {
        Window::Esmart
    }
}

/// The recorded capabilities as the generic backend would report them.
fn recorded_caps() -> Capabilities {
    recorded::load(PROBE).expect("probe dump loads").caps
}

fn rk3588() -> Capabilities {
    let mut caps = recorded_caps();
    DriverProfile::Rk3588Vop2.apply(&mut caps);
    caps
}

/// Only the planes of one class that reach output 0.
fn only(caps: &Capabilities, window: Window) -> Capabilities {
    let mut out = *caps;
    out.planes.clear();
    for plane in caps.planes.as_slice() {
        if window_of(plane) == window && plane.outputs & 1 != 0 {
            out.planes.push(*plane).unwrap();
        }
    }
    assert!(!out.planes.is_empty());
    out
}

/// A buffer the class lists in its formats: AFBC on Cluster, linear on Esmart.
fn native(window: Window) -> BufferId {
    match window {
        Window::Cluster => AFBC_XR24,
        Window::Esmart => LINEAR_XR24,
    }
}

fn plan_on(caps: &Capabilities, layers: &[Layer]) -> Result<usize, PlanError> {
    // The loader keeps the CRTC id as the output id; take it from the caps.
    let frame = Frame {
        output: caps.outputs.as_slice()[0].id,
        layers,
    };
    plan(&frame, caps, &Buffers).map(|p| p.assignments().len())
}

fn refused(reasons: RejectReasons) -> Result<usize, PlanError> {
    Err(PlanError::NoPlane { layer: 0, reasons })
}

fn scaled(buffer: BufferId, src: (u32, u32), dst: (u32, u32)) -> Layer {
    Layer::new(
        buffer,
        Rect::new(0, 0, src.0, src.1),
        Rect::new(0, 0, dst.0, dst.1),
        0,
    )
}

#[test]
fn fixture_is_the_recorded_topology() {
    let caps = recorded_caps();
    assert_eq!(caps.outputs.len(), 1);
    assert_eq!(caps.outputs.as_slice()[0].size, Size::new(1920, 1080));
    assert_eq!(caps.planes.len(), 8);
    let planes = caps.planes.as_slice();
    let count = |w| planes.iter().filter(|p| window_of(p) == w).count();
    assert_eq!(count(Window::Cluster), 4);
    assert_eq!(count(Window::Esmart), 4);
    // One Cluster primary belongs to the other CRTC.
    assert_eq!(planes.iter().filter(|p| p.outputs & 1 != 0).count(), 7);
    for plane in planes {
        match window_of(plane) {
            // The 32-pair cut: AFBC only, no linear pair.
            Window::Cluster => {
                assert_eq!(plane.formats.len(), MAX_FORMATS);
                assert!(plane.formats.iter().all(|f| f.modifier != Modifier::LINEAR));
            }
            Window::Esmart => {
                assert_eq!(plane.formats.len(), 21);
                assert!(plane.formats.iter().all(|f| f.modifier == Modifier::LINEAR));
            }
        }
        // What the profile changes, as KMS left it.
        assert_eq!(plane.scaling, None);
        assert_eq!(plane.max_source, None);
        assert_eq!(plane.transforms, 0);
    }
}

#[test]
fn profile_sets_the_class_limits_and_nothing_else() {
    let before = recorded_caps();
    let after = rk3588();
    assert_eq!(after.outputs, before.outputs);
    assert_eq!(after.resource_groups, before.resource_groups);
    assert!(after.resource_groups.is_empty());
    assert_eq!(after.replace_pending, before.replace_pending);
    for (old, new) in before.planes.iter().zip(after.planes.iter()) {
        // Same for Cluster and Esmart on this kernel (observed 2026-11-27
        // for the upscale, mainline v6.18 source for the rest).
        assert_eq!(
            new.scaling,
            Some(ScaleLimits {
                max_upscale: 8 << 16,
                max_downscale: 8 << 16,
            })
        );
        assert_eq!(new.max_source, Some(Size::new(4096, 4320)));
        let mut restored = *new;
        restored.scaling = old.scaling;
        restored.max_source = old.max_source;
        assert_eq!(&restored, old);
    }
}

#[test]
fn both_classes_scale_up_to_8x_each_way() {
    for window in [Window::Cluster, Window::Esmart] {
        let caps = only(&rk3588(), window);
        let buffer = native(window);
        let ok = |src, dst| plan_on(&caps, &[scaled(buffer, src, dst)]);
        assert_eq!(ok((1920, 1080), (1920, 1080)), Ok(1), "{window:?}");
        // Upscale 8x accepted on the board; 1920 / 239 = 8.03 is past it.
        assert_eq!(ok((240, 135), (1920, 1080)), Ok(1), "{window:?}");
        assert_eq!(
            ok((239, 135), (1920, 1080)),
            refused(RejectReasons::SCALING),
            "{window:?}"
        );
        assert_eq!(
            ok((240, 134), (1920, 1080)),
            refused(RejectReasons::SCALING),
            "{window:?}"
        );
        // Downscale 8x: the 1/8 bound of the mainline check.
        assert_eq!(ok((3840, 2160), (480, 270)), Ok(1), "{window:?}");
        assert_eq!(
            ok((3840, 2160), (479, 270)),
            refused(RejectReasons::SCALING),
            "{window:?}"
        );
    }
}

#[test]
fn generic_refuses_every_scaled_layer() {
    for window in [Window::Cluster, Window::Esmart] {
        let caps = only(&recorded_caps(), window);
        assert_eq!(
            plan_on(&caps, &[scaled(native(window), (960, 540), (1920, 1080))]),
            refused(RejectReasons::SCALING),
            "{window:?}"
        );
    }
}

#[test]
fn nv12_linear_only_on_esmart() {
    let layer = Layer::new(LINEAR_NV12, SCREEN, SCREEN, 0);
    assert_eq!(
        plan_on(&only(&rk3588(), Window::Cluster), &[layer]),
        refused(RejectReasons::FORMAT)
    );
    assert_eq!(plan_on(&only(&rk3588(), Window::Esmart), &[layer]), Ok(1));
}

#[test]
fn afbc_only_on_cluster() {
    let layer = Layer::new(AFBC_XR24, SCREEN, SCREEN, 0);
    assert_eq!(plan_on(&only(&rk3588(), Window::Cluster), &[layer]), Ok(1));
    assert_eq!(
        plan_on(&only(&rk3588(), Window::Esmart), &[layer]),
        refused(RejectReasons::FORMAT)
    );
    // Linear RGB is not in the Cluster list either.
    let linear = Layer::new(LINEAR_XR24, SCREEN, SCREEN, 0);
    assert_eq!(
        plan_on(&only(&rk3588(), Window::Cluster), &[linear]),
        refused(RejectReasons::FORMAT)
    );
}

#[test]
fn no_transform_is_offered() {
    // The recorded rotation property shows its current value only, so the
    // profile reports no transform on either class.
    let square = Rect::new(0, 0, 512, 512);
    for window in [Window::Cluster, Window::Esmart] {
        let caps = only(&rk3588(), window);
        for transform in [Transform::Rotate90, Transform::FlipV] {
            let mut layer = Layer::new(native(window), square, square, 0);
            layer.transform = transform;
            assert_eq!(
                plan_on(&caps, &[layer]),
                refused(RejectReasons::TRANSFORM),
                "{window:?} {transform:?}"
            );
        }
    }
}

#[test]
fn source_beyond_4096x4320_is_refused() {
    for window in [Window::Cluster, Window::Esmart] {
        let caps = only(&rk3588(), window);
        let buffer = native(window);
        let ok = |src| plan_on(&caps, &[scaled(buffer, src, (1920, 1080))]);
        assert_eq!(ok((4096, 4320)), Ok(1), "{window:?}");
        assert_eq!(
            ok((4097, 4320)),
            refused(RejectReasons::SOURCE_SIZE),
            "{window:?}"
        );
        assert_eq!(
            ok((4096, 4321)),
            refused(RejectReasons::SOURCE_SIZE),
            "{window:?}"
        );
    }
}

#[test]
fn seven_reachable_planes_take_seven_layers_and_no_more() {
    // Output 0 reaches 7 of the 8 planes: 3 Cluster (AFBC) and 4 Esmart
    // (linear). No layer budget is modelled, so all 7 fit.
    let caps = rk3588();
    let mut layers: Vec<Layer> = (0..3)
        .map(|z| Layer::new(AFBC_XR24, SCREEN, SCREEN, z))
        .chain((3..7).map(|z| Layer::new(LINEAR_XR24, SCREEN, SCREEN, z)))
        .collect();
    assert_eq!(plan_on(&caps, &layers), Ok(7));

    for extra in [AFBC_XR24, LINEAR_XR24] {
        let mut more = layers.clone();
        more.push(Layer::new(extra, SCREEN, SCREEN, 7));
        assert!(
            matches!(plan_on(&caps, &more), Err(PlanError::NoPlane { .. })),
            "an 8th layer has no plane on output 0"
        );
    }

    // A fourth AFBC layer has no Cluster plane left on this output.
    layers[3] = Layer::new(AFBC_XR24, SCREEN, SCREEN, 3);
    assert!(matches!(
        plan_on(&caps, &layers),
        Err(PlanError::NoPlane { .. })
    ));
}

#[test]
fn applying_twice_is_idempotent() {
    let once = rk3588();
    let mut twice = once;
    DriverProfile::Rk3588Vop2.apply(&mut twice);
    assert_eq!(twice, once);
}

#[test]
fn generic_leaves_the_fixture_untouched() {
    let mut caps = recorded_caps();
    DriverProfile::Generic.apply(&mut caps);
    assert_eq!(caps, recorded_caps());
}

/// Q1 of the design note: how much of a Cluster plane's IN_FORMATS list the
/// 32-pair cut drops. Result (observed 2026-11-27): each Cluster plane lists
/// 126 pairs (14 formats x 9 AFBC modifiers), 32 are kept, so 94 pairs and
/// 6 of the 9 modifiers (plus 10 formats of the third) never reach the
/// planner. Esmart lists 21 pairs and loses none.
#[test]
fn cluster_in_formats_exceeds_max_formats() {
    // (pairs, modifiers) per plane block of the props dump.
    let mut blocks: Vec<(usize, Vec<u64>)> = Vec::new();
    for line in PROPS.lines() {
        if line.starts_with("plane ") {
            blocks.push((0, Vec::new()));
        } else if line.starts_with(|c: char| !c.is_whitespace()) {
            // crtc/connector headers end the plane section of a block.
            blocks.push((0, Vec::new()));
        } else if let Some(rest) = line.trim().strip_prefix("modifier 0x") {
            let (hex, formats) = rest.split_once(':').expect("modifier line");
            let block = blocks.last_mut().expect("inside a block");
            block.0 += formats.split_whitespace().count();
            block.1.push(u64::from_str_radix(hex, 16).unwrap());
        }
    }
    let planes: Vec<_> = blocks.into_iter().filter(|b| b.0 > 0).collect();
    assert_eq!(planes.len(), 8);
    let cluster: Vec<_> = planes
        .iter()
        .filter(|b| b.1.iter().any(|m| (m >> 56) as u8 == VENDOR_ARM))
        .collect();
    assert_eq!(cluster.len(), 4);
    for (pairs, modifiers) in &cluster {
        assert_eq!(modifiers.len(), 9);
        assert_eq!(*pairs, 126);
        assert!(*pairs > MAX_FORMATS);
        assert_eq!(pairs - MAX_FORMATS, 94);
    }
    for (pairs, modifiers) in planes.iter().filter(|b| !cluster.contains(b)) {
        assert_eq!(modifiers.as_slice(), &[0]);
        assert_eq!(*pairs, 21);
        assert!(*pairs <= MAX_FORMATS);
    }

    // The loaded Cluster planes hold the first 32: modifiers 1 and 2 whole,
    // 4 formats of the third.
    for plane in rk3588().planes.iter() {
        if window_of(plane) == Window::Cluster {
            let mut seen: Vec<u64> = Vec::new();
            for f in plane.formats.iter() {
                if !seen.contains(&f.modifier.0) {
                    seen.push(f.modifier.0);
                }
            }
            assert_eq!(
                seen,
                [
                    0x0800_0000_0000_0001,
                    0x0800_0000_0000_0041,
                    0x0800_0000_0000_0011
                ]
            );
        }
    }
}
