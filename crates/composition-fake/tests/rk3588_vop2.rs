//! Planner and fake-engine test on the NanoPC-T6 RK3588 (Rockchip VOP2)
//! topology, frames of one to eight windows.
//!
//! This is a MODEL, not a hardware result: the fake backend has no VOP2, no
//! kernel and no `TEST_ONLY`. The constants below are copied from recorded
//! data, because `docs/observed` is going away and nothing here reads it at
//! runtime:
//!
//! - `docs/linux-board-scenarios.md` (plan #457 scen-t6 handoff), observed
//!   2026-11-27, kernel 6.18.55-current-rockchip64: 8 planes, zpos 0..=7 on
//!   every plane, no plane scales, AFBC only on the Cluster planes, planes 39
//!   and 69 cannot reach `video_port0`, the kernel lease holds primary 33 and
//!   overlays 45, 51, 57, 63, 69, 75 (69 is leased but unusable on
//!   `video_port0`, so the planner sees six usable planes).
//! - `docs/observed/rk3588-topology.md`, probe output observed 2026-11-28 on
//!   the same kernel: ids, Cluster/Esmart split, 1920x1080 at 60 Hz, the
//!   format lists (the probe cuts the Cluster lists at 32 pairs, so the
//!   Cluster planes are modelled with the AFBC pairs it printed. Plane 33
//!   also takes linear XR24 (debugfs showed fbcon scanning out), but that
//!   would be a 33rd pair and the format list holds 32, so it is left out).
//!   That record has plane 69 at outputs 0b1; the 2026-11-27 handoff says it
//!   cannot reach `video_port0`. The test follows the handoff, because the
//!   task names it.
//!
//! "Falls back to composition" is modelled the way a compositor drives the
//! engine: a window the planner refuses (`NoPlane`) is drawn by the GPU into
//! one extra buffer, which is itself an unscaled layer.

use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, Engine, Error, FormatModifier, Frame, Layer,
    Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, Size, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const SCREEN: Size = Size::new(1920, 1080);

const XR24: PixelFormat = PixelFormat::XRGB8888;
const AR24: PixelFormat = PixelFormat::ARGB8888;

const AFBC_1: Modifier = Modifier(0x800_0000_0000_0001);
const AFBC_41: Modifier = Modifier(0x800_0000_0000_0041);
const AFBC_11: Modifier = Modifier(0x800_0000_0000_0011);

// (id, kind, Cluster?, reaches video_port0): the 2026-11-27 handoff.
const PLANES: [(u32, PlaneKind, bool, bool); 8] = [
    (33, PlaneKind::Primary, true, true),
    (39, PlaneKind::Primary, true, false),
    (45, PlaneKind::Overlay, true, true),
    (51, PlaneKind::Overlay, true, true),
    (57, PlaneKind::Overlay, false, true),
    (63, PlaneKind::Overlay, false, true),
    (69, PlaneKind::Overlay, false, false),
    (75, PlaneKind::Overlay, false, true),
];
const CLUSTER_IDS: [u32; 4] = [33, 39, 45, 51];
const USABLE: [u32; 6] = [33, 45, 51, 57, 63, 75];

/// Cluster rows of the probe: 14 formats with `...001` and `...041`, the
/// first 4 of them with `...011` (the probe cut its list at 32 pairs there).
const CLUSTER_FORMATS: [&[u8; 4]; 14] = [
    b"XR30", b"XB30", b"XR24", b"AR24", b"XB24", b"AB24", b"RG24", b"BG24", b"RG16", b"BG16",
    b"YU08", b"YU10", b"YUYV", b"Y210",
];
/// Esmart rows of the probe: 21 linear formats.
const ESMART_FORMATS: [&[u8; 4]; 21] = [
    b"XR24", b"AR24", b"XB24", b"AB24", b"RG24", b"BG24", b"RG16", b"BG16", b"NV12", b"NV21",
    b"NV16", b"NV61", b"NV20", b"NV24", b"NV42", b"NV30", b"NV15", b"YVYU", b"VYUY", b"YUYV",
    b"UYVY",
];

fn fourcc(code: &[u8; 4]) -> PixelFormat {
    PixelFormat::fourcc(code)
}

fn plane(id: u32, kind: PlaneKind, cluster: bool, reach: bool) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs: u32::from(reach),
        zpos: Zpos::Range { min: 0, max: 7 },
        plane_alpha: true,
        blend_modes: BlendMode::Opaque.mask()
            | BlendMode::Premultiplied.mask()
            | BlendMode::Coverage.mask(),
        scaling: None,
        transforms: 0,
        ..PlaneCaps::default()
    };
    let mut push = |f: FormatModifier| caps.formats.push(f).unwrap();
    if cluster {
        for (modifier, count) in [(AFBC_1, 14), (AFBC_41, 14), (AFBC_11, 4)] {
            for code in CLUSTER_FORMATS.iter().take(count) {
                push(FormatModifier {
                    format: fourcc(code),
                    modifier,
                });
            }
        }
    } else {
        for code in ESMART_FORMATS {
            push(FormatModifier::linear(fourcc(code)));
        }
    }
    caps
}

fn caps() -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SCREEN,
            refresh_mhz: 60_000,
        })
        .unwrap();
    for (id, kind, cluster, reach) in PLANES {
        caps.planes.push(plane(id, kind, cluster, reach)).unwrap();
    }
    caps
}

fn engine_with(buffers: &[(PixelFormat, Modifier)]) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(caps()));
    for (i, (format, modifier)) in buffers.iter().enumerate() {
        let id = BufferId(i as u32 + 1);
        engine
            .register_buffer(
                id,
                BufferInfo {
                    size: SCREEN,
                    format: *format,
                    modifier: *modifier,
                },
            )
            .unwrap();
        engine.finish_write(id).unwrap();
    }
    engine
}

/// A window of the grid: 240x135 cells, unscaled (source size = dest size).
fn window(i: usize, z: i32) -> Layer {
    let rect = Rect::new((i as i32 % 8) * 240, 0, 240, 135);
    Layer::new(BufferId(i as u32 + 1), rect, rect, z)
}

/// A window shown at half size: scaling.
fn scaled_window(i: usize, z: i32) -> Layer {
    let src = Rect::new(0, 0, 240, 135);
    let dst = Rect::new((i as i32 % 8) * 240, 0, 120, 68);
    Layer::new(BufferId(i as u32 + 1), src, dst, z)
}

fn placement(engine: &mut Engine<FakeBackend>, layers: &[Layer]) -> Vec<(usize, u32)> {
    engine
        .validate(&Frame {
            output: OUT,
            layers,
        })
        .unwrap()
        .assignments()
        .iter()
        .map(|a| (a.layer, a.plane.0))
        .collect()
}

fn refused(engine: &mut Engine<FakeBackend>, layers: &[Layer]) -> Option<(usize, RejectReasons)> {
    match engine.present(&Frame {
        output: OUT,
        layers,
    }) {
        Ok(_) => None,
        Err(Error::Plan(PlanError::NoPlane { layer, reasons })) => Some((layer, reasons)),
        Err(e) => panic!("unexpected {e:?}"),
    }
}

fn is_cluster(id: u32) -> bool {
    CLUSTER_IDS.contains(&id)
}

#[test]
fn model_matches_the_recorded_facts() {
    let caps = caps();
    assert_eq!(caps.planes.len(), 8);
    assert!(caps
        .planes
        .iter()
        .all(|p| p.zpos == Zpos::Range { min: 0, max: 7 } && p.scaling.is_none()));
    let reach: Vec<u32> = caps
        .planes
        .iter()
        .filter(|p| p.outputs & 1 != 0)
        .map(|p| p.id.0)
        .collect();
    assert_eq!(reach, USABLE);
    for p in caps.planes.iter() {
        let afbc = p.formats.iter().any(|f| f.modifier != Modifier::LINEAR);
        assert_eq!(afbc, is_cluster(p.id.0), "AFBC only on Cluster: {}", p.id.0);
    }
}

#[test]
fn unscaled_windows_get_planes_up_to_the_usable_count() {
    // One to eight windows. Windows 0..3 are AFBC XR24 (Cluster only), the
    // rest linear AR24 (Esmart only): six usable planes, so windows 7 and 8
    // have none and the planner names the first window it cannot place.
    let bufs: Vec<_> = (0..8)
        .map(|i| {
            if i < 3 {
                (XR24, AFBC_1)
            } else {
                (AR24, Modifier::LINEAR)
            }
        })
        .collect();
    for n in 1..=8usize {
        let layers: Vec<_> = (0..n).map(|i| window(i, i as i32)).collect();
        let mut engine = engine_with(&bufs);
        if n <= 6 {
            let placed = placement(&mut engine, &layers);
            assert_eq!(placed.len(), n);
            for &(layer, plane) in &placed {
                assert_eq!(is_cluster(plane), layer < 3, "{n} windows: {placed:?}");
                assert!(USABLE.contains(&plane), "{n} windows: {placed:?}");
            }
            let mut used: Vec<u32> = placed.iter().map(|p| p.1).collect();
            used.sort_unstable();
            used.dedup();
            assert_eq!(used.len(), n, "one plane per window");
            assert!(refused(&mut engine, &layers).is_none(), "{n} windows");
            assert_eq!(engine.backend().commit_count(), 1);
        } else {
            let (layer, reasons) = refused(&mut engine, &layers).expect("no plane for window 7");
            assert_eq!(layer, 6, "{n} windows");
            assert!(reasons.contains(RejectReasons::IN_USE), "{reasons:?}");
            assert_eq!(engine.backend().commit_count(), 0);
        }
    }
}

#[test]
fn planes_39_and_69_are_never_used_for_video_port0() {
    for afbc in 0..=3usize {
        for linear in 0..=3usize {
            let n = afbc + linear;
            if n == 0 {
                continue;
            }
            let bufs: Vec<_> = (0..n)
                .map(|i| {
                    if i < afbc {
                        (XR24, AFBC_41)
                    } else {
                        (AR24, Modifier::LINEAR)
                    }
                })
                .collect();
            let layers: Vec<_> = (0..n).map(|i| window(i, i as i32)).collect();
            let mut engine = engine_with(&bufs);
            for (_, plane) in placement(&mut engine, &layers) {
                assert!(plane != 39 && plane != 69, "{afbc} AFBC + {linear} linear");
            }
        }
    }
    // A fourth AFBC window cannot borrow plane 39, and a fourth linear
    // window cannot borrow plane 69.
    let layers: Vec<_> = (0..4).map(|i| window(i, i as i32)).collect();
    let mut engine = engine_with(&[(XR24, AFBC_1); 4]);
    let (layer, _) = refused(&mut engine, &layers).unwrap();
    assert_eq!(layer, 3);
    let mut engine = engine_with(&[(AR24, Modifier::LINEAR); 4]);
    let (layer, _) = refused(&mut engine, &layers).unwrap();
    assert_eq!(layer, 3);
}

#[test]
fn afbc_is_never_placed_on_an_esmart_plane() {
    for modifier in [AFBC_1, AFBC_41, AFBC_11] {
        for n in 1..=8usize {
            let mut engine = engine_with(&vec![(XR24, modifier); n]);
            let layers: Vec<_> = (0..n).map(|i| window(i, i as i32)).collect();
            match refused(&mut engine, &layers) {
                None => {
                    assert!(n <= 3);
                    for (_, plane) in placement(&mut engine, &layers) {
                        assert!(is_cluster(plane), "AFBC on plane {plane}");
                    }
                }
                Some((layer, reasons)) => {
                    // Past the six usable planes the planner stops at the
                    // first layer without a plane left (index 6).
                    assert!(n > 3);
                    if n <= 6 {
                        assert_eq!(layer, 3, "{n} AFBC windows");
                        assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
                    } else {
                        assert_eq!(layer, 6, "{n} AFBC windows");
                    }
                }
            }
        }
    }
}

#[test]
fn scaled_windows_fall_back_to_composition() {
    // No plane scales: a scaled window is refused for SCALING at any count.
    for n in 1..=8usize {
        let mut bufs = vec![(AR24, Modifier::LINEAR); n];
        bufs.push((AR24, Modifier::LINEAR)); // the composed buffer, id n + 1
        let mut engine = engine_with(&bufs);
        let layers: Vec<_> = (0..n).map(|i| scaled_window(i, i as i32)).collect();
        let (layer, reasons) = refused(&mut engine, &layers).expect("scaled window refused");
        if n <= 6 {
            assert_eq!(layer, 0);
            assert!(reasons.contains(RejectReasons::SCALING), "{reasons:?}");
        } else {
            // More windows than usable planes: refused at the first window
            // with no plane left, before scaling is looked at.
            assert_eq!(layer, 6);
        }
        assert_eq!(engine.backend().commit_count(), 0);

        // Fallback: the GPU draws the scaled windows into one full-screen
        // unscaled buffer, which takes a plane.
        let full = Rect::new(0, 0, 1920, 1080);
        let composed = Layer::new(BufferId(n as u32 + 1), full, full, 0);
        assert_eq!(placement(&mut engine, &[composed]).len(), 1);
        assert!(refused(&mut engine, &[composed]).is_none());
        assert_eq!(engine.backend().commit_count(), 1);
    }

    // A scaled window between unscaled ones is refused at that window, and
    // without it the other three place.
    let mut engine = engine_with(&[(AR24, Modifier::LINEAR); 4]);
    let mixed = [
        window(0, 0),
        scaled_window(1, 1),
        window(2, 2),
        window(3, 3),
    ];
    let (layer, reasons) = refused(&mut engine, &mixed).unwrap();
    assert_eq!(layer, 1);
    assert!(reasons.contains(RejectReasons::SCALING), "{reasons:?}");
    let rest = [window(0, 0), window(2, 1), window(3, 2)];
    assert_eq!(placement(&mut engine, &rest).len(), 3);
}
