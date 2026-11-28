//! Pins the cross-output plane blind spot on VOP2- and HVS-shaped topologies.
//!
//! A `ResourceGroup` and the planner see one output's frame only. When two
//! outputs share movable planes, planning one frame per output may propose
//! the same plane for both. Nothing in the planner prevents that; the
//! kernel's atomic `TEST_ONLY` commit (the backend check, run through
//! `plan_with`) is the arbiter and refuses the second commit.
//!
//! These tests pin the current behaviour (docs/rk3588-backend-design.md §5
//! "Per-VP assignment" and §6 item 3, docs/hvs-backend-design.md §7 Q9). A
//! later fix, such as a cross-output claim set, will make them fail and show
//! as a visible diff. The fixtures are hand-built from the recorded shapes;
//! nothing reads docs/ at runtime.

use composition_core::limits::MAX_PLANES;
use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, Modifier, OutputCaps,
    OutputId, PixelFormat, PlaneCaps, PlaneId, PlaneKind, Rect, Size, Zpos,
};
use composition_drm::DriverProfile;

const OUT0: OutputId = OutputId(0);
const OUT1: OutputId = OutputId(1);
const BOTH: u32 = 0b11;
const SCREEN: Size = Size::new(1920, 1080);
const FULL: Rect = Rect::new(0, 0, 1920, 1080);
const BG: BufferId = BufferId(1);
const WIN: BufferId = BufferId(2);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let size = match buffer {
            BG => SCREEN,
            WIN => Size::new(256, 256),
            _ => return None,
        };
        Some(BufferInfo {
            size,
            format: PixelFormat::XRGB8888,
            modifier: Modifier::LINEAR,
        })
    }
}

fn plane(id: u32, kind: PlaneKind, outputs: u32, zpos: Zpos) -> PlaneCaps {
    let mut p = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs,
        zpos,
        ..PlaneCaps::default()
    };
    p.formats
        .push(FormatModifier::linear(PixelFormat::XRGB8888))
        .unwrap();
    p
}

fn with_outputs(mut caps: Capabilities) -> Capabilities {
    for id in [OUT0, OUT1] {
        caps.outputs
            .push(OutputCaps {
                id,
                size: SCREEN,
                refresh_mhz: 60_000,
            })
            .unwrap();
    }
    DriverProfile::Generic.apply(&mut caps);
    caps
}

/// VOP2-like: the NanoPC-T6 (RK3588) topology, observed 2026-11-28, recorded
/// in tests/data/rk3588/topology-2026-11-28.md. Eight planes, every plane reaching two
/// outputs (the outputs mask covers both), zpos 0..=7 on every plane.
fn vop2_caps() -> Capabilities {
    let mut caps = Capabilities::default();
    for i in 0..8 {
        let kind = if i == 0 {
            PlaneKind::Primary
        } else {
            PlaneKind::Overlay
        };
        caps.planes
            .push(plane(30 + i, kind, BOTH, Zpos::Range { min: 0, max: 7 }))
            .unwrap();
    }
    with_outputs(caps)
}

/// HVS-like: the Raspberry Pi 5 (BCM2712) shared overlay pool, observed
/// 2026-11-28, recorded in docs/platform-capabilities.md. Each output has its own
/// primary at fixed zpos 0; the overlays form one pool whose mask covers both
/// outputs, zpos 1..=17. The recorded list is longer than `MAX_PLANES`, so the
/// pool is cut to what fits next to the two primaries.
fn hvs_caps() -> Capabilities {
    let mut caps = Capabilities::default();
    caps.planes
        .push(plane(100, PlaneKind::Primary, 0b01, Zpos::Fixed(0)))
        .unwrap();
    caps.planes
        .push(plane(101, PlaneKind::Primary, 0b10, Zpos::Fixed(0)))
        .unwrap();
    for i in 0..(MAX_PLANES as u32 - 2) {
        caps.planes
            .push(plane(
                110 + i,
                PlaneKind::Overlay,
                BOTH,
                Zpos::Range { min: 1, max: 17 },
            ))
            .unwrap();
    }
    with_outputs(caps)
}

/// A background plus two small windows, all unscaled.
fn layers() -> [Layer; 3] {
    let win = Rect::new(0, 0, 256, 256);
    [
        Layer::new(BG, FULL, FULL, 0),
        Layer::new(WIN, win, Rect::new(100, 100, 256, 256), 1),
        Layer::new(WIN, win, Rect::new(500, 300, 256, 256), 2),
    ]
}

fn plane_ids(caps: &Capabilities, output: OutputId) -> Vec<PlaneId> {
    let layers = layers();
    let frame = Frame {
        output,
        layers: &layers,
    };
    let p = plan(&frame, caps, &Buffers).expect("each output plans on its own");
    assert_eq!(p.assignments().len(), layers.len());
    p.assignments().iter().map(|a| a.plane).collect()
}

fn shared(caps: &Capabilities) -> Vec<PlaneId> {
    let (a, b) = (plane_ids(caps, OUT0), plane_ids(caps, OUT1));
    a.into_iter().filter(|id| b.contains(id)).collect()
}

/// Current behaviour: both outputs get the same planes. In the kernel only
/// one of the two commits can use them; `TEST_ONLY` refuses the other.
#[test]
fn vop2_outputs_plan_the_same_planes() {
    let shared = shared(&vop2_caps());
    assert!(!shared.is_empty(), "blind spot closed? update this pin");
}

/// Current behaviour: the per-output primaries differ, the shared overlay
/// pool is proposed to both outputs.
#[test]
fn hvs_outputs_plan_the_same_overlays() {
    let shared = shared(&hvs_caps());
    assert!(!shared.is_empty(), "blind spot closed? update this pin");
    assert!(
        shared.iter().all(|id| id.0 >= 110),
        "primaries are per output and must not be shared"
    );
}
