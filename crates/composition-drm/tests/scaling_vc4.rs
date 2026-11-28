//! The generic planner (`DriverProfile::Generic`) on scaled layers, on the
//! recorded Raspberry Pi 5 (BCM2712, HVS6) and CM4 (BCM2711, HVS5) topologies.
//!
//! Baseline tests: they pin what the planner does today with no vc4 profile
//! applied, so that `Vc4Hvs5` and `Vc4Hvs6` change it on purpose. Nothing here
//! reads `docs/`; the recorded data is copied into the fixtures below
//! (the same fixtures as `generic_vc4.rs`, which does not share them).
//!
//! Evidence tags: *observed* = in the recorded probe output or `TEST_ONLY` run,
//! *source* = read from the vc4 driver source, *inferred* = derived, not
//! measured. The tests on "claimed" caps are hypothetical: they stand for what
//! a profile would add, not for a recording.

use composition_core::limits::MAX_PLANES;
use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, ScaleLimits, Size, Zpos,
};
use composition_drm::DriverProfile;

const OUT: OutputId = OutputId(0);

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

// Source: docs/platform-capabilities.md, output-0 probe block, Raspberry Pi 5
// Model B (BCM2712), kernel 6.18.50+rpt-rpi-2712, recorded 2026-11-28.
// The kernel exposes 56 planes. The connected output (CRTC id 94, 1920x1080
// @ 60 Hz) reaches primary 83, the 16 shared overlays 107, 118, ... 272
// (stride 11, zpos 1..=17 mutable) and plane 657 (the per-CRTC extra plane,
// inferred to be the cursor). That is 18 reachable planes; the backend keeps
// 16 (plan #457 fix-x09): the primary, the cursor plane, then overlays in id
// order, so the last two overlays (261, 272) are cut.
// Formats: the probe's 32, identical on every plane = the first 32 of the 45
// linear entries of IN_FORMATS (XR24 .. AR12). The four Broadcom modifiers
// come after the linear pairs and are cut at MAX_FORMATS = 32, so no kept pair
// carries a modifier. Plane alpha true, blend [none, premultiplied, coverage],
// scaling "not claimed", on all planes.
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

// Source: docs/platform-capabilities.md, DSI-1 probe block, reTerminal
// (CM4, BCM2711), recorded 2026-11-28. The kernel exposes 60 planes. The
// connected output (CRTC id 90 `pixelvalve-1`, DSI-1, 720x1280 @ 60 Hz)
// reaches primary 79 and the shared overlays 127, 138, ... 226 (stride 11,
// zpos 1..=17). Recorded: ten overlays survive the old cut; the kernel gives
// the CRTC 16 shared overlays plus a cursor (inferred), 18 reachable, of which
// the backend keeps 16 (plan #457 fix-x09, vc4-cm4 run). The ids of the four
// overlays after 226 (237, 248, 259, 270) follow the stride and are inferred,
// as is the cursor plane, whose id was not recorded (700 is a placeholder).
// Formats: the probe's 32, identical on every plane. Plane alpha true, blend
// [none, premultiplied, coverage], scaling "not claimed". The kernel accepted
// scale-up and scale-down in a TEST_ONLY run on this board (observed
// 2026-11-28, docs/platform-capabilities.md, "Unsupported combinations"); the
// limits were not probed.
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
}

const RGB: BufferId = BufferId(1);

/// An XRGB buffer of the given size, so that large source rectangles fit.
struct Buffers(Size);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        (buffer == RGB).then_some(BufferInfo {
            size: self.0,
            format: PixelFormat::XRGB8888,
            modifier: Modifier::LINEAR,
        })
    }
}

fn place(caps: &Capabilities, buffer: Size, layers: &[Layer]) -> Result<Placed, PlanError> {
    let frame = Frame {
        output: OUT,
        layers,
    };
    plan(&frame, caps, &Buffers(buffer)).map(|p| {
        p.assignments()
            .iter()
            .map(|a| (a.layer, a.plane.0, a.zpos))
            .collect()
    })
}

fn both() -> [(&'static str, &'static Topology); 2] {
    [("BCM2712", &PI5), ("BCM2711", &CM4)]
}

/// Caps as a vc4 profile might claim them: every non-cursor plane scales by up
/// to `up` and down to 1/`down`, and takes sources up to `max_source`.
/// Hypothetical, not a recording.
fn claimed(topo: &Topology, up: u32, down: u32, max_source: Option<Size>) -> Capabilities {
    let mut caps = topo.caps();
    for p in caps.planes.as_mut_slice() {
        if p.kind != PlaneKind::Cursor {
            p.scaling = Some(ScaleLimits {
                max_upscale: up << 16,
                max_downscale: down << 16,
            });
            p.max_source = max_source;
        }
    }
    caps
}

/// A layer showing a `sw` x `sh` source at the origin in a `dw` x `dh` window.
fn scaled(sw: u32, sh: u32, dw: u32, dh: u32, z: i32) -> Layer {
    Layer::new(RGB, Rect::new(0, 0, sw, sh), Rect::new(0, 0, dw, dh), z)
}

/// The refusal of layer 0 names SCALING; the format-less cursor plane adds
/// FORMAT.
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

/// The ratios of the item as (name, source factor, destination factor) of a
/// 160x90 base size: 1/4 and 1/2 are reductions, 2 and 4 enlargements.
const RATIOS: [(&str, u32, u32); 4] = [("1/4", 4, 1), ("1/2", 2, 1), ("2", 1, 2), ("4", 1, 4)];

// ---- Generic: scaling is not claimed, so no scaled layer is placed ----

#[test]
fn generic_leaves_the_recorded_caps_unchanged() {
    for (name, topo) in both() {
        let mut caps = topo.caps();
        let before = caps;
        DriverProfile::Generic.apply(&mut caps);
        assert_eq!(caps, before, "{name}");
        assert!(caps
            .planes
            .iter()
            .all(|p| p.scaling.is_none() && p.max_source.is_none()));
    }
}

#[test]
fn generic_refuses_every_ratio() {
    // Probe: "scaling not claimed" on every plane (observed 2026-11-28). The
    // kernel accepted scale-up and scale-down in TEST_ONLY on the CM4
    // (observed 2026-11-28), so this is the conservative baseline that
    // Vc4Hvs5/Vc4Hvs6 will change.
    for (name, topo) in both() {
        let caps = topo.caps();
        for (ratio, s, d) in RATIOS {
            let layer = scaled(160 * s, 90 * s, 160 * d, 90 * d, 0);
            assert_scaling_refused(
                &format!("{name} {ratio}"),
                place(&caps, topo.output, &[layer]),
            );
        }
    }
}

#[test]
fn generic_refuses_a_single_axis_scale() {
    for (name, topo) in both() {
        let caps = topo.caps();
        assert_scaling_refused(
            name,
            place(&caps, topo.output, &[scaled(160, 90, 320, 90, 0)]),
        );
        assert_scaling_refused(
            name,
            place(&caps, topo.output, &[scaled(160, 90, 160, 45, 0)]),
        );
    }
}

#[test]
fn generic_refuses_an_8191_pixel_source_scaled_to_the_output() {
    // HVS5 rejects a source wider than 8191 (source: SCALER5_POS2_WIDTH_MASK,
    // docs/platform-capabilities.md). A source of exactly 8191 shown on the
    // output is a reduction, so Generic refuses it on SCALING, not on source
    // size: it has no max_source.
    for (name, topo) in both() {
        let layer = Layer::new(
            RGB,
            Rect::new(0, 0, 8191, 1080),
            Rect::new(0, 0, topo.output.width, topo.output.height),
            0,
        );
        assert_scaling_refused(name, place(&topo.caps(), Size::new(8191, 1080), &[layer]));
    }
}

#[test]
fn generic_places_zero_scaled_layers_per_output() {
    // Scaled layers placed per output: none, whatever else is in the frame.
    // The unscaled layers around the scaled one are placeable on their own.
    for (name, topo) in both() {
        let caps = topo.caps();
        let layers = [
            scaled(160, 90, 160, 90, 0),
            scaled(160, 90, 320, 180, 1),
            scaled(160, 90, 160, 90, 2),
        ];
        let Err(PlanError::NoPlane { layer, reasons }) = place(&caps, topo.output, &layers) else {
            panic!("{name}: expected NoPlane");
        };
        assert_eq!(layer, 1, "{name}");
        assert!(reasons.contains(RejectReasons::SCALING), "{name}");
        let placed = place(&caps, topo.output, &[layers[0], layers[2]]).unwrap();
        assert_eq!(placed.len(), 2, "{name}");
    }
}

// ---- hypothetical: what a profile that claims scaling changes ----

#[test]
fn claimed_scaling_places_the_item_ratios_within_the_limit() {
    // Every plane scales by up to 4 in either direction.
    for (name, topo) in both() {
        let caps = claimed(topo, 4, 4, None);
        for (ratio, s, d) in RATIOS {
            let layer = scaled(160 * s, 90 * s, 160 * d, 90 * d, 0);
            assert_eq!(
                place(&caps, topo.output, &[layer]),
                Ok(vec![(0, topo.primary, 0)]),
                "{name} {ratio}"
            );
        }
    }
}

#[test]
fn claimed_scaling_refuses_beyond_the_limit_on_either_axis() {
    for (name, topo) in both() {
        let caps = claimed(topo, 4, 4, None);
        for layer in [
            scaled(160, 90, 641, 90, 0),
            scaled(641, 90, 160, 90, 0),
            scaled(160, 90, 160, 361, 0),
        ] {
            assert_scaling_refused(name, place(&caps, topo.output, &[layer]));
        }
    }
}

#[test]
fn claimed_scaling_places_one_scaled_layer_per_usable_plane() {
    // Without resource groups (none is visible through KMS; the LBM/UPM groups
    // of docs/hvs-backend-design.md section 4 are source-only) the planner
    // places as many scaled layers as there are planes that can take a layer:
    // 15 of the 16 kept, the cursor plane having no recorded formats. A
    // profile that adds those groups lowers this number.
    for (name, topo) in both() {
        let caps = claimed(topo, 4, 4, None);
        let usable = MAX_PLANES - 1;
        let layers: Vec<Layer> = (0..usable)
            .map(|i| scaled(64, 64, 32, 32, i as i32))
            .collect();
        let placed = place(&caps, topo.output, &layers).unwrap();
        assert_eq!(placed.len(), usable, "{name}");
        assert_eq!(placed[0].1, topo.primary, "{name}");
    }
}

#[test]
fn claimed_max_source_8191_refuses_a_wider_source_on_source_size() {
    // A Vc4Hvs5 value (source): width 8191. 8192 is one over.
    for (name, topo) in both() {
        let caps = claimed(topo, 4, 16, Some(Size::new(8191, 8191)));
        let buffer = Size::new(8192, 90);
        let ok = scaled(8191, 90, 640, 90, 0);
        assert!(place(&caps, buffer, &[ok]).is_ok(), "{name}");
        let over = scaled(8192, 90, 640, 90, 0);
        let Err(PlanError::NoPlane { reasons, .. }) = place(&caps, buffer, &[over]) else {
            panic!("{name}: expected NoPlane");
        };
        assert!(reasons.contains(RejectReasons::SOURCE_SIZE), "{name}");
    }
}
