//! Planner test on the observed BCM2711 (reTerminal CM4, vc4 / HVS5) topology:
//! per-output plane reach, plane types, formats and the `MAX_PLANES` cut.
//!
//! The data below is copied from `docs/observed/bcm2711-cm4-topology.md` and
//! `docs/observed/bcm2711-cm4.md` (observed 2026-11-28, kernel
//! 6.12.47+rpt-rpi-v8, `probe /dev/dri/card0`). The tests never read those
//! files. This is a model, not a hardware result: the fake backend has no HVS
//! and no `TEST_ONLY`. It complements `topology_bcm2711.rs` (the two-plane
//! harness rows); this file covers reach, types and the cut.
//!
//! The kernel has 60 planes: 6 primaries, 16 shared overlays, 32 TXP overlays
//! and 6 cursors (cursor types are inferred, not read). Only the DSI CRTC
//! (output 0, primary 79) is connected. composition-drm keeps `MAX_PLANES` =
//! 16 planes, primaries first.

use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, Engine, Error, FormatModifier, Frame, Layer,
    Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, Size, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const W: u32 = 720;
const H: u32 = 1280;

/// Primaries, plane-0..5 (observed 2026-11-28).
const PRIMARY_IDS: [u32; 6] = [48, 67, 79, 91, 103, 115];
/// The DSI CRTC's primary (observed 2026-11-28).
const DSI_PRIMARY: u32 = 79;
/// The ten overlays probe kept (observed 2026-11-28).
const KEPT_OVERLAYS: [u32; 10] = [127, 138, 149, 160, 171, 182, 193, 204, 215, 226];
/// The six shared overlays the cut dropped (plane-16..21, ids on the observed
/// step of 11).
const DROPPED_OVERLAYS: [u32; 6] = [237, 248, 259, 270, 281, 292];
/// A cursor plane. The kernel has six (plane-54..59, zpos 17, types inferred);
/// this id is a model value, not an observed one.
const CURSOR_ID: u32 = 700;

/// The 32 formats probe printed on every plane (observed 2026-11-28).
const FORMATS: [&[u8; 4]; 32] = [
    b"XR24", b"AR24", b"AB24", b"XB24", b"RG16", b"BG16", b"AR15", b"XR15", b"RG24", b"BG24",
    b"YU16", b"YV16", b"YU24", b"YV24", b"YU12", b"YV12", b"NV12", b"NV21", b"NV16", b"NV61",
    b"XR30", b"AR30", b"AB30", b"XB30", b"RGB8", b"BGR8", b"XR12", b"AR12", b"XB12", b"AB12",
    b"BX12", b"BA12",
];

fn plane(id: u32, kind: PlaneKind, outputs: u32) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs,
        plane_alpha: true,
        blend_modes: BlendMode::Opaque.mask()
            | BlendMode::Premultiplied.mask()
            | BlendMode::Coverage.mask(),
        zpos: match kind {
            PlaneKind::Primary => Zpos::Fixed(0),
            _ => Zpos::Range { min: 1, max: 17 },
        },
        ..PlaneCaps::default()
    };
    for code in FORMATS {
        caps.formats
            .push(FormatModifier::linear(PixelFormat::fourcc(code)))
            .unwrap();
    }
    caps
}

fn base() -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(W, H),
            refresh_mhz: 60_000,
        })
        .unwrap();
    caps
}

/// Probe's cut view: six primaries then ten overlays, as observed.
fn cut_view() -> Capabilities {
    let mut caps = base();
    for id in PRIMARY_IDS {
        let outputs = u32::from(id == DSI_PRIMARY);
        caps.planes
            .push(plane(id, PlaneKind::Primary, outputs))
            .unwrap();
    }
    for id in KEPT_OVERLAYS {
        caps.planes.push(plane(id, PlaneKind::Overlay, 1)).unwrap();
    }
    caps
}

fn layer(id: u32, z: i32) -> Layer {
    let source = Rect::new(0, 0, 64, 64);
    let mut layer = Layer::new(BufferId(id), source, Rect::new(z * 8, z * 8, 64, 64), z);
    layer.blend = BlendMode::Premultiplied;
    layer
}

/// Engine with `n` registered XR24 64x64 buffers, ids 100..100+n.
fn engine(caps: Capabilities, n: u32) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(caps));
    for i in 0..n {
        let id = BufferId(100 + i);
        engine
            .register_buffer(
                id,
                BufferInfo {
                    size: Size::new(64, 64),
                    format: PixelFormat::XRGB8888,
                    modifier: Modifier::LINEAR,
                },
            )
            .unwrap();
        engine.finish_write(id).unwrap();
    }
    engine
}

fn layers(n: u32) -> Vec<Layer> {
    (0..n).map(|i| layer(100 + i, i as i32)).collect()
}

fn validate(
    engine: &mut Engine<FakeBackend>,
    layers: &[Layer],
) -> Result<composition_core::Plan, PlanError> {
    match engine.validate(&Frame {
        output: OUT,
        layers,
    }) {
        Ok(plan) => Ok(plan),
        Err(Error::Plan(error)) => Err(error),
        Err(other) => panic!("unexpected error {other:?}"),
    }
}

fn out_in_use(layer: usize) -> PlanError {
    PlanError::NoPlane {
        layer,
        reasons: RejectReasons::OUTPUT.union(RejectReasons::IN_USE),
    }
}

// "Planes probe reports", "Plane -> CRTC masks": 16 planes, six primaries then
// ten overlays, only primary 79 and the overlays reach output 0.
#[test]
fn cut_view_has_the_observed_types_reach_and_formats() {
    let caps = cut_view();
    assert_eq!(caps.planes.len(), 16);
    let count = |k: PlaneKind| caps.planes.iter().filter(|p| p.kind == k).count();
    assert_eq!(count(PlaneKind::Primary), 6);
    assert_eq!(count(PlaneKind::Overlay), 10);
    assert_eq!(count(PlaneKind::Cursor), 0, "no cursor survives the cut");
    let reach: Vec<u32> = caps
        .planes
        .iter()
        .filter(|p| p.outputs & 1 != 0)
        .map(|p| p.id.0)
        .collect();
    let mut want = vec![DSI_PRIMARY];
    want.extend(KEPT_OVERLAYS);
    assert_eq!(reach, want);
    assert!(caps.planes.iter().all(|p| p.formats.len() == 32));
}

// The planner uses the DSI primary and the ten overlays and never the five
// primaries of unconnected CRTCs; a twelfth layer has no plane.
#[test]
fn eleven_layers_use_exactly_the_planes_that_reach_the_output() {
    let mut eng = engine(cut_view(), 12);
    let plan = validate(&mut eng, &layers(11)).unwrap();
    let mut used: Vec<u32> = plan.assignments().iter().map(|a| a.plane.0).collect();
    used.sort_unstable();
    let mut want = vec![DSI_PRIMARY];
    want.extend(KEPT_OVERLAYS);
    want.sort_unstable();
    assert_eq!(used, want);
    assert_eq!(validate(&mut eng, &layers(12)).unwrap_err(), out_in_use(11));
}

// The `MAX_PLANES` cut: a 17th plane does not fit the capability table.
#[test]
fn the_capability_table_holds_sixteen_planes() {
    let mut caps = cut_view();
    assert!(caps
        .planes
        .push(plane(DROPPED_OVERLAYS[0], PlaneKind::Overlay, 1))
        .is_err());
    assert_eq!(caps.planes.len(), 16);
}

// Had the cut kept reachable planes first, output 0 would hold the primary
// plus 15 overlays: 16 layers fit and 17 are refused (the layer limit),
// although the kernel has 17 planes for this CRTC.
#[test]
fn reachable_first_cut_gives_sixteen_layers_and_refuses_the_seventeenth() {
    let mut caps = base();
    caps.planes
        .push(plane(DSI_PRIMARY, PlaneKind::Primary, 1))
        .unwrap();
    for id in KEPT_OVERLAYS.iter().chain(&DROPPED_OVERLAYS[..5]) {
        caps.planes.push(plane(*id, PlaneKind::Overlay, 1)).unwrap();
    }
    assert_eq!(caps.planes.len(), 16);
    let mut eng = engine(caps, 17);
    assert_eq!(
        validate(&mut eng, &layers(16)).unwrap().assignments().len(),
        16
    );
    // 17 layers hit the engine's layer limit before any plane is looked at.
    assert_eq!(
        validate(&mut eng, &layers(17)).unwrap_err(),
        PlanError::TooManyLayers { count: 17 }
    );
}

// The planner does not look at the plane type; with a cursor plane listed last
// the first two layers still go to the primary and the first overlay.
#[test]
fn cursor_plane_listed_last_does_not_take_the_first_layers() {
    let mut caps = base();
    caps.planes
        .push(plane(DSI_PRIMARY, PlaneKind::Primary, 1))
        .unwrap();
    caps.planes
        .push(plane(KEPT_OVERLAYS[0], PlaneKind::Overlay, 1))
        .unwrap();
    caps.planes
        .push(plane(CURSOR_ID, PlaneKind::Cursor, 1))
        .unwrap();
    let mut eng = engine(caps, 2);
    let plan = validate(&mut eng, &layers(2)).unwrap();
    let a = plan.assignments();
    assert_eq!(a[0].plane, PlaneId(DSI_PRIMARY));
    assert_eq!(a[1].plane, PlaneId(KEPT_OVERLAYS[0]));
}
