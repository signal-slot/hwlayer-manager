//! The generic planner (`DriverProfile::Generic`) on the recorded HVS
//! topologies of the Raspberry Pi 5 (BCM2712, HVS6) and the CM4 (BCM2711,
//! HVS5), as the backend's probe reports them after the `MAX_PLANES` cut.
//!
//! These are baseline tests: they pin what the planner does with the data as
//! recorded and with no profile applied. Tests for the vc4 profiles show the
//! difference later. Nothing here reads `docs/observed`; the data is copied
//! into the fixtures below.
//!
//! Evidence tags: *observed* = in the recorded probe output or property dump,
//! *inferred* = derived from the recording, not measured.

use composition_core::limits::{MAX_FORMATS, MAX_LAYERS, MAX_PLANES};
use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, Size, Zpos,
};
use composition_drm::DriverProfile;

const OUT: OutputId = OutputId(0);

/// Broadcom VC4 T-tiled, `fourcc_mod_broadcom_code(1, 0)`.
/// Observed: `IN_FORMATS` modifier on the Pi 5 planes, 2026-11-28.
const VC4_T_TILED: Modifier = Modifier(0x0700_0000_0000_0001);
/// Broadcom SAND128 (code 3), NV12/NV21. Observed: `IN_FORMATS` modifier on the
/// Pi 5 planes, 2026-11-28; the name is inferred from the code.
const SAND128: Modifier = Modifier(0x0700_0000_0000_0003);

fn fourcc(s: &[u8; 4]) -> PixelFormat {
    PixelFormat::fourcc(s)
}

type Placed = Vec<(usize, u32, i32)>;

/// The recorded topology of one board: the 16 planes the backend keeps for
/// the connected output, with the format list the probe printed.
struct Topology {
    output: Size,
    /// Plane ids in the order of the backend's `keep_planes`: primary, then
    /// overlays, then the cursor plane.
    primary: u32,
    overlays: &'static [u32],
    cursor: Option<u32>,
    formats: &'static [&'static [u8; 4]],
}

// Source: docs/observed/bcm2712-topology.md, output-0 probe block, Raspberry Pi 5
// Model B (BCM2712), kernel 6.18.50+rpt-rpi-2712, recorded 2026-11-28.
// The kernel exposes 56 planes. The connected output (CRTC id 94, 1920x1080
// @ 60 Hz) reaches primary 83, the 16 shared overlays 107, 118, ... 272
// (stride 11, zpos 1..=17 mutable) and plane 657 (the per-CRTC extra plane,
// inferred to be the cursor). That is 18 reachable planes; the backend keeps
// 16 (plan #457 fix-x09): the primary, the cursor plane, then overlays in id
// order, so the last two overlays (261, 272) are cut.
// Formats: the probe's 32, identical on every plane = the first 32 of the 45
// linear entries of IN_FORMATS (XR24 .. AR12). The four Broadcom modifiers
// (T-tiled, SAND128, code 4, code 5) come after the linear pairs and are cut
// at MAX_FORMATS = 32, so no kept pair carries a modifier. Plane alpha true,
// blend [none, premultiplied, coverage], scaling "not claimed", on all planes.
const PI5: Topology = Topology {
    output: Size::new(1920, 1080),
    primary: 83,
    overlays: &[
        107, 118, 129, 140, 151, 162, 173, 184, 195, 206, 217, 228, 239, 250,
    ],
    // Observed: id 657 and "one extra plane per CRTC". Inferred: it is a
    // cursor plane. Its formats and zpos were not read.
    cursor: Some(657),
    formats: &[
        b"XR24", b"AR24", b"AB24", b"XB24", b"RX24", b"RA24", b"BX24", b"BA24", b"RG16", b"BG16",
        b"AR15", b"XR15", b"RG24", b"BG24", b"YU16", b"YV16", b"YU24", b"YV24", b"YU12", b"YV12",
        b"NV12", b"NV21", b"NV16", b"NV61", b"XR30", b"AR30", b"AB30", b"XB30", b"RGB8", b"BGR8",
        b"XR12", b"AR12",
    ],
};

// Source: docs/observed/bcm2711-cm4-topology.md, DSI-1 probe block, reTerminal
// (CM4, BCM2711), recorded 2026-11-28. The kernel exposes 60 planes. The
// connected output (CRTC id 90 `pixelvalve-1`, DSI-1, 720x1280 @ 60 Hz)
// reaches primary 79 and the shared overlays 127, 138, ... 226 (stride 11,
// zpos 1..=17). Recorded: ten overlays survive the old cut; the kernel gives
// the CRTC 16 shared overlays plus a cursor (inferred), 18 reachable, of which
// the backend keeps 16 (plan #457 fix-x09, vc4-cm4 run). The ids of the four
// overlays after 226 (237, 248, 259, 270) follow the stride and are inferred,
// as is the cursor plane, whose id was not recorded (700 is a placeholder).
// Formats: the probe's 32, identical on every plane. No modifier column was
// printed, and the 32 linear pairs fill the list, so the cut would remove any
// modifier pair that follows them. Plane alpha true, blend [none,
// premultiplied, coverage], scaling "not claimed".
const CM4: Topology = Topology {
    output: Size::new(720, 1280),
    primary: 79,
    overlays: &[
        127, 138, 149, 160, 171, 182, 193, 204, 215, 226, 237, 248, 259, 270,
    ],
    cursor: Some(700),
    formats: &[
        b"XR24", b"AR24", b"AB24", b"XB24", b"RG16", b"BG16", b"AR15", b"XR15", b"RG24", b"BG24",
        b"YU16", b"YV16", b"YU24", b"YV24", b"YU12", b"YV12", b"NV12", b"NV21", b"NV16", b"NV61",
        b"XR30", b"AR30", b"AB30", b"XB30", b"RGB8", b"BGR8", b"XR12", b"AR12", b"XB12", b"AB12",
        b"BX12", b"BA12",
    ],
};

impl Topology {
    fn caps(&self) -> Capabilities {
        let mut caps = Capabilities::default();
        caps.outputs
            .push(OutputCaps {
                id: OUT,
                size: self.output,
                refresh_mhz: 60_000,
            })
            .unwrap();
        let blend =
            BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask() | BlendMode::Coverage.mask();
        let mut add = |id: u32, kind: PlaneKind, zpos: Zpos, formats: bool| {
            let mut plane = PlaneCaps {
                id: PlaneId(id),
                kind,
                outputs: 1,
                plane_alpha: true,
                blend_modes: blend,
                zpos,
                ..PlaneCaps::default()
            };
            if formats {
                for f in self.formats {
                    plane
                        .formats
                        .push(FormatModifier::linear(fourcc(f)))
                        .unwrap();
                }
            }
            caps.planes.push(plane).unwrap();
        };
        add(self.primary, PlaneKind::Primary, Zpos::Fixed(0), true);
        for id in self.overlays {
            add(
                *id,
                PlaneKind::Overlay,
                Zpos::Range { min: 1, max: 17 },
                true,
            );
        }
        if let Some(id) = self.cursor {
            // Formats not recorded: left empty, so no layer lands on it.
            add(id, PlaneKind::Cursor, Zpos::Fixed(i32::MAX), false);
        }
        caps
    }

    fn ids(&self) -> Vec<u32> {
        let mut ids = vec![self.primary];
        ids.extend_from_slice(self.overlays);
        ids.extend(self.cursor);
        ids
    }
}

const RGB: BufferId = BufferId(1);
const NV12: BufferId = BufferId(2);
const NV12_SAND128: BufferId = BufferId(3);
const XRGB_TILED: BufferId = BufferId(4);
const P010: BufferId = BufferId(5);

struct Buffers(Size);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let (format, modifier) = match buffer {
            RGB => (PixelFormat::XRGB8888, Modifier::LINEAR),
            NV12 => (PixelFormat::NV12, Modifier::LINEAR),
            NV12_SAND128 => (PixelFormat::NV12, SAND128),
            XRGB_TILED => (PixelFormat::XRGB8888, VC4_T_TILED),
            P010 => (fourcc(b"P010"), Modifier::LINEAR),
            _ => return None,
        };
        Some(BufferInfo {
            size: self.0,
            format,
            modifier,
        })
    }
}

fn frame_plan(topo: &Topology, caps: &Capabilities, layers: &[Layer]) -> Result<Placed, PlanError> {
    let frame = Frame {
        output: OUT,
        layers,
    };
    plan(&frame, caps, &Buffers(topo.output)).map(|p| {
        p.assignments()
            .iter()
            .map(|a| (a.layer, a.plane.0, a.zpos))
            .collect()
    })
}

fn full(topo: &Topology) -> Rect {
    Rect::new(0, 0, topo.output.width, topo.output.height)
}

fn no_plane(layer: usize, reasons: RejectReasons) -> Result<Placed, PlanError> {
    Err(PlanError::NoPlane { layer, reasons })
}

fn one(topo: &Topology, buffer: BufferId, dst: Rect) -> Result<Placed, PlanError> {
    let layer = Layer::new(buffer, full(topo), dst, 0);
    frame_plan(topo, &topo.caps(), &[layer])
}

/// `n` unscaled 32x32 windows of the XRGB buffer, stacked in order.
fn tiles(n: usize) -> Vec<Layer> {
    (0..n)
        .map(|i| {
            let (x, y) = ((i as u32 % 4) * 40, (i as u32 / 4) * 40);
            Layer::new(
                RGB,
                Rect::new(x as i32, y as i32, 32, 32),
                Rect::new(x as i32, y as i32, 32, 32),
                i as i32,
            )
        })
        .collect()
}

fn both() -> [(&'static str, &'static Topology); 2] {
    [("BCM2712", &PI5), ("BCM2711", &CM4)]
}

// ---- the fixtures themselves ----

#[test]
fn fixtures_match_the_recorded_cut() {
    for (name, topo) in both() {
        let caps = topo.caps();
        assert_eq!(caps.planes.len(), MAX_PLANES, "{name}");
        assert_eq!(topo.formats.len(), MAX_FORMATS, "{name}");
        assert!(caps
            .planes
            .iter()
            .all(|p| p.outputs == 1 && p.scaling.is_none() && p.max_source.is_none()));
        let ids: Vec<u32> = caps.planes.iter().map(|p| p.id.0).collect();
        assert_eq!(ids, topo.ids(), "{name}");
    }
}

#[test]
fn no_kept_pair_carries_a_modifier() {
    for (name, topo) in both() {
        for p in topo.caps().planes.iter() {
            assert!(
                p.formats.iter().all(|f| f.modifier == Modifier::LINEAR),
                "{name}"
            );
        }
    }
}

#[test]
fn overlay_zpos_range_is_1_to_17_and_primary_is_fixed() {
    for (name, topo) in both() {
        let caps = topo.caps();
        assert_eq!(caps.planes.as_slice()[0].zpos, Zpos::Fixed(0), "{name}");
        assert!(caps
            .planes
            .iter()
            .filter(|p| p.kind == PlaneKind::Overlay)
            .all(|p| p.zpos == Zpos::Range { min: 1, max: 17 }));
    }
}

// ---- Generic changes nothing ----

#[test]
fn generic_leaves_the_recorded_caps_unchanged() {
    for (name, topo) in both() {
        let mut caps = topo.caps();
        let before = caps;
        DriverProfile::Generic.apply(&mut caps);
        assert_eq!(caps, before, "{name}");
    }
}

// ---- unscaled frames ----

#[test]
fn single_full_screen_layer_goes_to_the_primary_at_zpos_0() {
    for (name, topo) in both() {
        assert_eq!(
            one(topo, RGB, full(topo)),
            Ok(vec![(0, topo.primary, 0)]),
            "{name}"
        );
    }
}

#[test]
fn one_layer_per_usable_plane_is_placed() {
    // 16 planes are kept, but the cursor plane has no recorded formats, so
    // 15 can take a layer: the primary and 14 overlays.
    for (name, topo) in both() {
        let caps = topo.caps();
        let usable = caps.planes.iter().filter(|p| !p.formats.is_empty()).count();
        assert_eq!(usable, MAX_PLANES - 1, "{name}");
        let layers = tiles(usable);
        let placed = frame_plan(topo, &caps, &layers).unwrap();
        assert_eq!(placed.len(), usable, "{name}");
        // The bottom layer is on the primary, the rest ascend over the overlays.
        assert_eq!(placed[0], (0, topo.primary, 0), "{name}");
        let zpos: Vec<i32> = placed.iter().map(|a| a.2).collect();
        assert!(zpos.windows(2).all(|w| w[0] < w[1]), "{name}: {zpos:?}");
        assert!(zpos[1..].iter().all(|z| (1..=17).contains(z)), "{name}");
        let mut planes: Vec<u32> = placed.iter().map(|a| a.1).collect();
        planes.sort_unstable();
        planes.dedup();
        assert_eq!(planes.len(), usable, "{name}: a plane was used twice");
    }
}

#[test]
fn a_full_max_layers_frame_of_rgb_does_not_fit() {
    // MAX_LAYERS == MAX_PLANES, and one kept plane is the cursor plane, which
    // has no recorded formats: only 15 planes can take a layer. The planner
    // does not report that as `NoPlane`: with 16 interchangeable candidates
    // its search runs into the step limit first, so the frame is refused
    // with `LimitExceeded`. One layer fewer is placed.
    const { assert!(MAX_LAYERS == MAX_PLANES) };
    for (name, topo) in both() {
        let layers = tiles(MAX_LAYERS);
        assert_eq!(
            frame_plan(topo, &topo.caps(), &layers),
            Err(PlanError::LimitExceeded),
            "{name}"
        );
        assert!(frame_plan(topo, &topo.caps(), &layers[..MAX_LAYERS - 1]).is_ok());
    }
}

#[test]
fn more_than_max_layers_is_too_many() {
    for (name, topo) in both() {
        let mut layers = tiles(MAX_LAYERS);
        layers.push(layers[0]);
        assert_eq!(
            frame_plan(topo, &topo.caps(), &layers),
            Err(PlanError::TooManyLayers {
                count: MAX_LAYERS + 1
            }),
            "{name}"
        );
    }
}

// ---- scaling ----

/// The refusal names SCALING; the format-less cursor plane adds FORMAT.
fn assert_scaling_refused(name: &str, result: Result<Placed, PlanError>) {
    let Err(PlanError::NoPlane { layer: 0, reasons }) = result else {
        panic!("{name}: expected NoPlane, got {result:?}");
    };
    assert_eq!(
        reasons,
        RejectReasons::SCALING | RejectReasons::FORMAT,
        "{name}"
    );
}

#[test]
fn scaled_layer_is_refused_because_scaling_is_not_claimed() {
    // Probe: "scaling not claimed" on every plane. The kernel accepted scaled
    // planes in TEST_ONLY on the CM4 (observed 2026-11-28, bcm2711-cm4.md),
    // so this is the conservative baseline that a vc4 profile will change.
    for (name, topo) in both() {
        let (w, h) = (topo.output.width, topo.output.height);
        let half = Rect::new(0, 0, w / 2, h / 2);
        assert_scaling_refused(name, one(topo, RGB, half));
        // Upscaling too.
        let layer = Layer::new(RGB, half, full(topo), 0);
        assert_scaling_refused(name, frame_plan(topo, &topo.caps(), &[layer]));
    }
}

// ---- formats and modifiers as kept ----

#[test]
fn linear_nv12_is_placed() {
    for (name, topo) in both() {
        assert_eq!(
            one(topo, NV12, full(topo)),
            Ok(vec![(0, topo.primary, 0)]),
            "{name}"
        );
    }
}

#[test]
fn nv12_with_sand128_is_refused_on_format() {
    // The Pi 5 IN_FORMATS lists NV12 under SAND128 (observed 2026-11-28), but
    // that pair is cut at MAX_FORMATS, so the planner sees linear pairs only.
    // The CM4's modifiers were not recorded; the same cut applies.
    for (name, topo) in both() {
        assert_eq!(
            one(topo, NV12_SAND128, full(topo)),
            no_plane(0, RejectReasons::FORMAT),
            "{name}"
        );
    }
}

#[test]
fn t_tiled_xrgb_is_refused_on_format_on_both() {
    // XR24 with the T-tiled modifier is in the Pi 5 kernel list, not in the
    // kept 32 pairs. On the CM4 it is not in the kept list either (inferred:
    // no modifier was printed and the linear pairs fill the list), so it is
    // refused there too; "placed on BCM2711 if the pair is kept" does not
    // arise with this recording.
    for (name, topo) in both() {
        assert_eq!(
            one(topo, XRGB_TILED, full(topo)),
            no_plane(0, RejectReasons::FORMAT),
            "{name}"
        );
    }
}

#[test]
fn t_tiled_xrgb_is_placed_when_its_pair_is_kept() {
    // Hypothetical, not a recording: every CM4 plane lists the T-tiled pair
    // in place of its last linear pair. With the pair present the layer is
    // placed on the primary.
    let mut caps = CM4.caps();
    for p in caps.planes.as_mut_slice() {
        if p.kind != PlaneKind::Cursor {
            p.formats.pop();
            p.formats
                .push(FormatModifier {
                    format: PixelFormat::XRGB8888,
                    modifier: VC4_T_TILED,
                })
                .unwrap();
        }
    }
    let layer = Layer::new(XRGB_TILED, full(&CM4), full(&CM4), 0);
    assert_eq!(frame_plan(&CM4, &caps, &[layer]), Ok(vec![(0, 79, 0)]));
}

#[test]
fn formats_cut_after_the_32nd_pair_are_refused() {
    // P010 is a linear pair of the Pi 5 kernel list, past the 32nd (observed
    // 2026-11-28, bcm2712-topology.md, discrepancy 2): refused on format.
    for (name, topo) in both() {
        assert_eq!(
            one(topo, P010, full(topo)),
            no_plane(0, RejectReasons::FORMAT),
            "{name}"
        );
    }
}

// ---- zpos ----

#[test]
fn a_layer_that_only_the_primary_can_show_cannot_sit_above_another() {
    // Primary zpos is fixed at 0 and overlays start at 1 (observed 2026-11-28).
    // Derived caps (not a recording): only the primary lists NV12, so the NV12
    // layer needs zpos 0 while a layer below it already occupies an overlay.
    for (name, topo) in both() {
        let mut caps = topo.caps();
        for p in caps.planes.as_mut_slice() {
            if p.kind == PlaneKind::Overlay {
                let kept: Vec<_> = p
                    .formats
                    .iter()
                    .copied()
                    .filter(|f| f.format != PixelFormat::NV12)
                    .collect();
                p.formats = Default::default();
                for f in kept {
                    p.formats.push(f).unwrap();
                }
            }
        }
        let layers = [
            Layer::new(RGB, full(topo), full(topo), 0),
            Layer::new(NV12, full(topo), full(topo), 1),
        ];
        let Err(PlanError::NoPlane { layer: 1, reasons }) = frame_plan(topo, &caps, &layers) else {
            panic!("{name}: expected NoPlane for layer 1");
        };
        assert!(
            reasons.contains(RejectReasons::Z_ORDER),
            "{name}: {reasons:?}"
        );
        // Swapped, the NV12 layer is the bottom one and takes the primary.
        let swapped = [
            Layer::new(NV12, full(topo), full(topo), 0),
            Layer::new(RGB, full(topo), full(topo), 1),
        ];
        let placed = frame_plan(topo, &caps, &swapped).unwrap();
        assert_eq!(placed[0], (0, topo.primary, 0), "{name}");
        assert!(placed[1].2 >= 1, "{name}");
    }
}
