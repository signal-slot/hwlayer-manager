//! NV12 and RGB placement on the modelled STM32MP257F-DK (LTDC) planes.
//!
//! MODEL, NOT A HARDWARE RESULT. The topology is typed in from
//! `docs/observed/stm32mp257-topology.md` and
//! `docs/stm32mp257-ltdc-yuv-source.md`: plane 34 Primary and 49 Overlay take
//! RGB (plane 34 also packed YUV), plane 42 Overlay takes RGB, packed YUV and
//! the planar/semi-planar YUV formats including NV12; zpos 0..=2; no scaling
//! claimed. Nothing here ran on the board and nothing counts toward M1, M2 or
//! M3. The tests show only how the shared planner places formats on this
//! model.
//!
//! API note: the planner has no composition fallback. A layer no plane can
//! take makes `Engine::validate` fail with `PlanError::NoPlane` and the
//! reject reasons; falling back to composition is the caller's decision. The
//! fake carries per-plane format lists, so format constraints are expressible;
//! it does not model YUV colour encoding or range, which are not tested.

#![forbid(unsafe_code)]

use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, Engine, Error, FormatModifier, Frame, Layer,
    Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, Size, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const SIZE: Size = Size::new(1920, 1080);
const P34: u32 = 34;
const P42: u32 = 42;
const P49: u32 = 49;
const XR24: PixelFormat = PixelFormat::XRGB8888;
const AR24: PixelFormat = PixelFormat::ARGB8888;
const NV12: PixelFormat = PixelFormat::NV12;

const RGB_FORMATS: [&[u8; 4]; 17] = [
    b"AR24", b"XR24", b"AB24", b"XB24", b"RA24", b"RX24", b"BA24", b"BX24", b"RG16", b"BG16",
    b"RG24", b"BG24", b"AR15", b"XR15", b"AR12", b"XR12", b"C8  ",
];
const PACKED_YUV: [&[u8; 4]; 4] = [b"YUYV", b"YVYU", b"UYVY", b"VYUY"];
const PLANAR_YUV: [&[u8; 4]; 4] = [b"NV12", b"NV21", b"YU12", b"YV12"];

fn plane(id: u32, kind: PlaneKind, extra: &[&[u8; 4]]) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs: 1,
        scaling: None,
        plane_alpha: true,
        blend_modes: BlendMode::Premultiplied.mask(),
        transforms: 0,
        zpos: Zpos::Range { min: 0, max: 2 },
        ..PlaneCaps::default()
    };
    for code in RGB_FORMATS.iter().copied().chain(extra.iter().copied()) {
        caps.formats
            .push(FormatModifier::linear(PixelFormat::fourcc(code)))
            .unwrap();
    }
    caps
}

fn caps() -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SIZE,
            refresh_mhz: 60_000,
        })
        .unwrap();
    let mut yuv42 = PACKED_YUV.to_vec();
    yuv42.extend_from_slice(&PLANAR_YUV);
    for p in [
        plane(P34, PlaneKind::Primary, &PACKED_YUV),
        plane(P42, PlaneKind::Overlay, &yuv42),
        plane(P49, PlaneKind::Overlay, &[]),
    ] {
        caps.planes.push(p).unwrap();
    }
    caps
}

type Eng = Engine<FakeBackend>;

fn engine() -> Eng {
    Engine::new(FakeBackend::new(caps()))
}

fn register(engine: &mut Eng, id: u32, format: PixelFormat) -> BufferId {
    let id = BufferId(id);
    engine
        .register_buffer(
            id,
            BufferInfo {
                size: SIZE,
                format,
                modifier: Modifier::LINEAR,
            },
        )
        .unwrap();
    engine.finish_write(id).unwrap();
    id
}

fn layer(buffer: BufferId, z: i32) -> Layer {
    let full = Rect::new(0, 0, SIZE.width, SIZE.height);
    let mut l = Layer::new(buffer, full, full, z);
    l.blend = BlendMode::Premultiplied;
    l
}

fn frame(layers: &[Layer]) -> Frame<'_> {
    Frame {
        output: OUT,
        layers,
    }
}

fn placement(engine: &mut Eng, layers: &[Layer]) -> Vec<(usize, u32)> {
    engine
        .validate(&frame(layers))
        .unwrap()
        .assignments()
        .iter()
        .map(|a| (a.layer, a.plane.0))
        .collect()
}

fn refusal(engine: &mut Eng, layers: &[Layer]) -> (usize, RejectReasons) {
    match engine.validate(&frame(layers)).unwrap_err() {
        Error::Plan(PlanError::NoPlane { layer, reasons }) => (layer, reasons),
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

fn on(got: &[(usize, u32)], layer: usize) -> u32 {
    got.iter().find(|a| a.0 == layer).unwrap().1
}

/// A lone NV12 layer lands on plane 42, whatever its z.
#[test]
fn nv12_alone_goes_to_plane_42() {
    for z in [0, 1, 2] {
        let mut e = engine();
        let nv = register(&mut e, 1, NV12);
        assert_eq!(placement(&mut e, &[layer(nv, z)]), [(0, P42)], "z={z}");
    }
}

/// NV12 under an RGB overlay: NV12 stays on 42, the RGB layer on 34 or 49.
#[test]
fn nv12_beside_rgb_keeps_rgb_off_plane_42() {
    let mut e = engine();
    let nv = register(&mut e, 1, NV12);
    let rgb = register(&mut e, 2, AR24);
    let got = placement(&mut e, &[layer(nv, 0), layer(rgb, 1)]);
    assert_eq!(on(&got, 0), P42, "{got:?}");
    assert!([P34, P49].contains(&on(&got, 1)), "{got:?}");
}

/// Three layers, NV12 at each position among two RGB ones: all three planes
/// are used once, NV12 on 42.
#[test]
fn nv12_with_two_rgb_layers_uses_every_plane_once() {
    for nv_at in 0..3usize {
        let mut e = engine();
        let mut layers = Vec::new();
        for i in 0..3usize {
            let (fmt, id) = if i == nv_at {
                (NV12, 10)
            } else {
                (XR24, i as u32 + 20)
            };
            let b = register(&mut e, id, fmt);
            layers.push(layer(b, i as i32));
        }
        let got = placement(&mut e, &layers);
        assert_eq!(on(&got, nv_at), P42, "nv_at={nv_at} {got:?}");
        let mut planes: Vec<u32> = got.iter().map(|a| a.1).collect();
        planes.sort_unstable();
        assert_eq!(planes, [P34, P42, P49], "nv_at={nv_at}");
    }
}

/// Two RGB layers place without touching YUV support; the bottom one takes
/// the primary plane.
#[test]
fn rgb_layers_alone_are_accepted() {
    let mut e = engine();
    let a = register(&mut e, 1, XR24);
    let b = register(&mut e, 2, AR24);
    let got = placement(&mut e, &[layer(a, 0), layer(b, 1)]);
    assert_eq!(got.len(), 2);
    assert_eq!(on(&got, 0), P34, "{got:?}");
}

/// A second NV12 layer finds no plane: FORMAT from planes 34 and 49, IN_USE
/// from plane 42. The planner refuses; it does not compose.
#[test]
fn second_nv12_layer_is_refused() {
    let mut e = engine();
    let a = register(&mut e, 1, NV12);
    let b = register(&mut e, 2, NV12);
    let (idx, reasons) = refusal(&mut e, &[layer(a, 0), layer(b, 1)]);
    assert_eq!(idx, 1);
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
    assert!(reasons.contains(RejectReasons::IN_USE), "{reasons:?}");
}

/// NV12 never lands on plane 34 or 49, whatever the layer order: it is either
/// placed on 42 or refused for FORMAT.
#[test]
fn nv12_never_lands_on_a_plane_without_nv12() {
    for rgb_first in [true, false] {
        let mut e = engine();
        let r1 = register(&mut e, 1, XR24);
        let r2 = register(&mut e, 2, AR24);
        let nv = register(&mut e, 3, NV12);
        let layers = if rgb_first {
            [layer(r1, 0), layer(r2, 1), layer(nv, 2)]
        } else {
            [layer(nv, 0), layer(r1, 1), layer(r2, 2)]
        };
        let nv_idx = if rgb_first { 2 } else { 0 };
        match e.validate(&frame(&layers)) {
            Ok(plan) => {
                let a = plan
                    .assignments()
                    .iter()
                    .find(|a| a.layer == nv_idx)
                    .expect("nv12 assigned");
                assert_eq!(a.plane.0, P42, "rgb_first={rgb_first}");
            }
            Err(Error::Plan(PlanError::NoPlane { layer, reasons })) => {
                assert_eq!(layer, nv_idx, "rgb_first={rgb_first}");
                assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
            }
            Err(other) => panic!("unexpected {other:?}"),
        }
    }
}

/// Packed YUV (YUYV) is allowed on plane 34 in the model, so a YUYV layer
/// under an NV12 layer places both.
#[test]
fn packed_yuv_background_and_nv12_overlay_both_place() {
    let mut e = engine();
    let bg = register(&mut e, 1, PixelFormat::fourcc(b"YUYV"));
    let nv = register(&mut e, 2, NV12);
    let got = placement(&mut e, &[layer(bg, 0), layer(nv, 1)]);
    assert_eq!(got, [(0, P34), (1, P42)]);
}

/// A format on no plane (ABGR2101010, `AB30`) is refused for FORMAT.
#[test]
fn format_on_no_plane_is_refused_for_format() {
    let mut e = engine();
    let b = register(&mut e, 1, PixelFormat::fourcc(b"AB30"));
    let (idx, reasons) = refusal(&mut e, &[layer(b, 0)]);
    assert_eq!(idx, 0);
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
}
