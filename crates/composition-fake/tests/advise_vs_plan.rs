//! `composition_core::advise` against the planner over the recorded driver
//! scenes of composition-fake (the RK3588 video ports and the BCM2711 HVS5
//! cut view; scene builders copied from `advise_scenes.rs`). MODELS, not
//! hardware results.
//!
//! Only relations `docs/advisory-planner-design.md` states are encoded:
//! - a frame the planner accepts (a Plan without composition) gets advice
//!   with no composed layer and no blocker (the "direct" advice);
//! - `Blocker::Static` means no plane can take the layer on its own, so the
//!   planner must refuse the layer alone (and so the whole frame);
//! - `Blocker::Combination` means some plane can take the layer alone, so
//!   the planner accepts the layer alone;
//! - a layer advice puts on a plane is on a plane that reaches the output,
//!   with no plane used twice, and the planner accepts the reduced frame.
//!
//! Left out because the note is ambiguous or the relation is not stated:
//! which layers are composed in a failing frame beyond the run length (the
//! search order is an implementation detail), and `Blocker::Backend`,
//! `Clip` and `Budget` (they need a refusing check or a hidden layer and have
//! no planner counterpart in these scenes).

use composition_core::planner::plan;
use composition_core::{
    advise, Advice, AdviseOptions, BlendMode, Blocker, BufferId, BufferInfo, BufferLookup,
    Capabilities, ComposeTarget, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId,
    PixelFormat, Placement, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect, Size, Verdict, Zpos,
};

const XR24: PixelFormat = PixelFormat::XRGB8888;
const TARGET_BUFFER: BufferId = BufferId(0xc0de);

struct Buffers(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

fn info(size: Size, format: PixelFormat) -> BufferInfo {
    BufferInfo {
        size,
        format,
        modifier: Modifier::LINEAR,
    }
}
fn options() -> AdviseOptions {
    AdviseOptions {
        compose: Some(ComposeTarget {
            format: XR24,
            modifier: Modifier::LINEAR,
            blend: BlendMode::Opaque,
        }),
        ..AdviseOptions::default()
    }
}
// ------------------------------------------------------------------ RK3588

mod rk3588 {
    use super::*;

    pub const VP0: OutputId = OutputId(0);
    pub const VP1: OutputId = OutputId(1);
    /// A third output no plane reaches.
    pub const VP2: OutputId = OutputId(2);
    pub const SCREEN: Size = Size::new(1920, 1080);
    pub const FULL: Rect = Rect::new(0, 0, 1920, 1080);

    const PLANES: [(u32, PlaneKind, u32, bool); 8] = [
        (33, PlaneKind::Primary, 0b01, false),
        (39, PlaneKind::Primary, 0b10, false),
        (45, PlaneKind::Overlay, 0b11, false),
        (51, PlaneKind::Overlay, 0b11, false),
        (57, PlaneKind::Overlay, 0b11, true),
        (63, PlaneKind::Overlay, 0b11, true),
        (69, PlaneKind::Overlay, 0b11, true),
        (75, PlaneKind::Overlay, 0b11, true),
    ];

    pub fn caps() -> Capabilities {
        let mut caps = Capabilities::default();
        for id in [VP0, VP1, VP2] {
            caps.outputs
                .push(OutputCaps {
                    id,
                    size: SCREEN,
                    refresh_mhz: 60_000,
                })
                .unwrap();
        }
        for (id, kind, outputs, esmart) in PLANES {
            let mut p = PlaneCaps {
                id: PlaneId(id),
                kind,
                outputs,
                zpos: Zpos::Range { min: 0, max: 7 },
                plane_alpha: true,
                blend_modes: BlendMode::Opaque.mask()
                    | BlendMode::Premultiplied.mask()
                    | BlendMode::Coverage.mask(),
                ..PlaneCaps::default()
            };
            p.formats.push(FormatModifier::linear(XR24)).unwrap();
            p.formats
                .push(FormatModifier::linear(PixelFormat::ARGB8888))
                .unwrap();
            if esmart {
                p.formats
                    .push(FormatModifier::linear(PixelFormat::NV12))
                    .unwrap();
            }
            caps.planes.push(p).unwrap();
        }
        caps
    }

    pub fn buffers(n: u32) -> Buffers {
        Buffers((1..=n).map(|i| (BufferId(i), info(SCREEN, XR24))).collect())
    }

    pub fn layers(n: u32) -> Vec<Layer> {
        (0..n)
            .map(|i| Layer::new(BufferId(1 + i), FULL, FULL, i as i32))
            .collect()
    }
}
mod bcm2711 {
    use super::*;

    pub const OUT: OutputId = OutputId(0);
    pub const SIZE: Size = Size::new(720, 1280);
    pub const PRIMARY_IDS: [u32; 6] = [48, 67, 79, 91, 103, 115];
    pub const DSI_PRIMARY: u32 = 79;
    pub const KEPT_OVERLAYS: [u32; 10] = [127, 138, 149, 160, 171, 182, 193, 204, 215, 226];
    pub const DROPPED_OVERLAYS: [u32; 6] = [237, 248, 259, 270, 281, 292];

    const FORMATS: [&[u8; 4]; 8] = [
        b"XR24", b"AR24", b"AB24", b"XB24", b"RG16", b"BG16", b"NV12", b"NV21",
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
                size: SIZE,
                refresh_mhz: 60_000,
            })
            .unwrap();
        caps
    }

    /// Probe's cut view: six primaries then ten overlays; eleven planes
    /// reach the DSI output.
    pub fn cut_view() -> Capabilities {
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

    /// The reachable-first cut: the primary and fifteen overlays, sixteen
    /// planes reaching the output.
    pub fn reachable_first() -> Capabilities {
        let mut caps = base();
        caps.planes
            .push(plane(DSI_PRIMARY, PlaneKind::Primary, 1))
            .unwrap();
        for id in KEPT_OVERLAYS.iter().chain(&DROPPED_OVERLAYS[..5]) {
            caps.planes.push(plane(*id, PlaneKind::Overlay, 1)).unwrap();
        }
        caps
    }

    pub fn layers(n: u32) -> Vec<Layer> {
        (0..n)
            .map(|i| {
                let z = i as i32;
                let mut layer = Layer::new(
                    BufferId(100 + i),
                    Rect::new(0, 0, 64, 64),
                    Rect::new(z * 8, z * 8, 64, 64),
                    z,
                );
                layer.blend = BlendMode::Premultiplied;
                layer
            })
            .collect()
    }

    pub fn buffers(n: u32) -> Buffers {
        Buffers(
            (0..n)
                .map(|i| (BufferId(100 + i), info(Size::new(64, 64), XR24)))
                .collect(),
        )
    }
}

struct Scene {
    name: String,
    caps: Capabilities,
    buffers: Buffers,
    layers: Vec<Layer>,
    output: OutputId,
    size: Size,
}

fn scenes() -> Vec<Scene> {
    let mut out = Vec::new();
    let rk = rk3588::caps();
    for output in [rk3588::VP0, rk3588::VP1, rk3588::VP2] {
        for n in 1..=16u32 {
            out.push(Scene {
                name: format!("rk3588 port {} x{n}", output.0),
                caps: rk,
                buffers: rk3588::buffers(n),
                layers: rk3588::layers(n),
                output,
                size: rk3588::SCREEN,
            });
        }
    }
    // A layer no modelled RK3588 plane can scale.
    let mut buffers = rk3588::buffers(3);
    buffers.0[1].1.size = Size::new(960, 540);
    let mut layers = rk3588::layers(3);
    layers[1].source = Rect::new(0, 0, 960, 540);
    out.push(Scene {
        name: "rk3588 unscalable layer".into(),
        caps: rk,
        buffers,
        layers,
        output: rk3588::VP0,
        size: rk3588::SCREEN,
    });
    for (label, caps) in [
        ("cut", bcm2711::cut_view()),
        ("reachable", bcm2711::reachable_first()),
    ] {
        for n in 1..=16u32 {
            out.push(Scene {
                name: format!("bcm2711 {label} x{n}"),
                caps,
                buffers: bcm2711::buffers(n),
                layers: bcm2711::layers(n),
                output: bcm2711::OUT,
                size: bcm2711::SIZE,
            });
        }
    }
    // A format no plane lists, at both ends and in the middle.
    for bad in [0usize, 4, 15] {
        let mut buffers = bcm2711::buffers(16);
        buffers.0[bad].1.format = PixelFormat::fourcc(b"HWL0");
        out.push(Scene {
            name: format!("bcm2711 cut x16 bad format at {bad}"),
            caps: bcm2711::cut_view(),
            buffers,
            layers: bcm2711::layers(16),
            output: bcm2711::OUT,
            size: bcm2711::SIZE,
        });
    }
    out
}

/// Planner on the single layer `i` of the scene, alone.
fn alone(s: &Scene, i: usize) -> Result<composition_core::Plan, PlanError> {
    let one = [s.layers[i]];
    plan(
        &Frame {
            output: s.output,
            layers: &one,
        },
        &s.caps,
        &s.buffers,
    )
}

/// The planner accepts the frame with the composed run replaced by one
/// full-output layer of the composition buffer.
fn check_reduced(s: &Scene, advice: &Advice) {
    let full = Rect::new(0, 0, s.size.width, s.size.height);
    let mut reduced: Vec<Layer> = Vec::new();
    let mut inserted = false;
    for (layer, a) in s.layers.iter().zip(advice.layers()) {
        if a.placement == Placement::Compose {
            if !inserted {
                reduced.push(Layer::new(TARGET_BUFFER, full, full, layer.z_order));
                inserted = true;
            }
        } else {
            reduced.push(*layer);
        }
    }
    let mut all = Buffers(s.buffers.0.clone());
    all.0.push((TARGET_BUFFER, info(s.size, XR24)));
    let frame = Frame {
        output: s.output,
        layers: &reduced,
    };
    plan(&frame, &s.caps, &all).unwrap_or_else(|e| panic!("{}: reduced frame: {e:?}", s.name));
}

fn check_plane_placements(s: &Scene, advice: &Advice) {
    let bit = 1u32 << s.caps.output_index(s.output).unwrap_or(31);
    let mut used = Vec::new();
    for l in advice.layers() {
        if let Placement::Plane(a) = l.placement {
            let reach = s
                .caps
                .planes
                .iter()
                .find(|c| c.id == a.plane)
                .unwrap()
                .outputs;
            assert!(reach & bit != 0, "{}", s.name);
            used.push(a.plane.0);
        }
    }
    let n = used.len();
    used.sort_unstable();
    used.dedup();
    assert_eq!(used.len(), n, "{}: a plane is used twice", s.name);
}

#[test]
fn advice_agrees_with_the_planner_over_the_recorded_scenes() {
    let mut accepted = 0;
    let mut refused = 0;
    for s in scenes() {
        let frame = Frame {
            output: s.output,
            layers: &s.layers,
        };
        let verdict = plan(&frame, &s.caps, &s.buffers);
        for opts in [options(), AdviseOptions::default()] {
            let Ok(advice) = advise(&frame, &s.caps, &s.buffers, &opts, |_, _| Verdict::Accept)
            else {
                // A whole-call error (no plane for the composition buffer):
                // there is no advice to compare, and the planner refused.
                assert!(verdict.is_err(), "{}", s.name);
                continue;
            };
            match &verdict {
                Ok(p) => {
                    // A Plan without composition: nothing to compose, no blocker.
                    assert!(advice.is_direct(), "{}", s.name);
                    assert!(advice.compose().is_none() && advice.compose_run().is_none());
                    assert_eq!(advice.promoted(), p.assignments().len(), "{}", s.name);
                    for l in advice.layers() {
                        assert_eq!(l.blocker, Blocker::None, "{}", s.name);
                        assert!(matches!(l.placement, Placement::Plane(_)), "{}", s.name);
                    }
                }
                Err(e) => {
                    assert!(!advice.is_direct(), "{}: planner refused {e:?}", s.name);
                    for (i, l) in advice.layers().iter().enumerate() {
                        match l.blocker {
                            Blocker::Static => {
                                assert_eq!(l.placement, Placement::Compose, "{}", s.name);
                                assert!(
                                    matches!(alone(&s, i), Err(PlanError::NoPlane { .. })),
                                    "{}: layer {i} is Static but plans alone",
                                    s.name
                                );
                            }
                            Blocker::Combination => {
                                assert_eq!(l.placement, Placement::Compose, "{}", s.name);
                                assert!(
                                    alone(&s, i).is_ok(),
                                    "{}: layer {i} is Combination but has no plane alone",
                                    s.name
                                );
                            }
                            _ => {}
                        }
                    }
                    if advice.compose().is_some() {
                        check_reduced(&s, &advice);
                    }
                }
            }
            check_plane_placements(&s, &advice);
        }
        if verdict.is_ok() {
            accepted += 1;
        } else {
            refused += 1;
        }
    }
    assert!(
        accepted > 10 && refused > 10,
        "{accepted} accepted, {refused} refused"
    );
}
