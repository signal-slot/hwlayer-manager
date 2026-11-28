//! Prints `composition_core::advise` for one fixed scene on three
//! illustrative hardware shapes.
//!
//! Run: `cargo run -p composition-core --example advise_profiles`
//!
//! The scene (1920x1080 output, bottom to top):
//!
//! 0. `ui`     XRGB8888 desktop, 1:1, opaque
//! 1. `video`  NV12 1280x720, upscaled 1.5x to the whole output
//! 2. `ui2`    ARGB8888 panel, premultiplied, 1:1
//! 3. `cursor` ARGB8888 64x64, premultiplied
//!
//! Output is one line per layer: placement and blocker, then the number of
//! layers promoted to planes.
//!
//! Important limit: `DriverProfile` lives in `composition-drm`, which
//! depends on this crate. This crate cannot use it without a new dependency,
//! so the three capability sets here are hand-written sketches of the shape
//! of AM62P tidss, i.MX95 DPU and STM32MP257 LTDC (as in
//! `tests/advise.rs`). They are **not** the output of
//! `DriverProfile::apply`, not observed on a board and not a measurement.
//! The example uses `std`; examples are not built for the `no_std` target.

use composition_core::{
    advise, AdviseOptions, BlendMode, BufferId, BufferInfo, BufferLookup, Capabilities,
    ComposeTarget, FormatModifier, Frame, Layer, LayerTraits, Modifier, OutputCaps, OutputId,
    PixelFormat, Placement, PlaneCaps, PlaneId, PlaneKind, Rect, ResourceGroup, ScaleLimits, Size,
    Transform, Verdict, Zpos,
};

const OUT: OutputId = OutputId(0);
const SCREEN: Size = Size::new(1920, 1080);

struct Buffers(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

fn info(w: u32, h: u32, format: PixelFormat) -> BufferInfo {
    BufferInfo {
        size: Size::new(w, h),
        format,
        modifier: Modifier::LINEAR,
    }
}

fn base() -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SCREEN,
            refresh_mhz: 60_000,
        })
        .unwrap();
    caps
}

fn plane(
    id: u32,
    primary: bool,
    zmax: i32,
    scaling: Option<ScaleLimits>,
    blends: u8,
    formats: &[PixelFormat],
) -> PlaneCaps {
    let mut p = PlaneCaps {
        id: PlaneId(id),
        kind: if primary {
            PlaneKind::Primary
        } else {
            PlaneKind::Overlay
        },
        outputs: 1,
        scaling,
        plane_alpha: true,
        blend_modes: blends,
        transforms: Transform::Normal.mask(),
        zpos: Zpos::Range { min: 0, max: zmax },
        ..PlaneCaps::default()
    };
    for f in formats {
        p.formats.push(FormatModifier::linear(*f)).unwrap();
    }
    p
}

const RGB: [PixelFormat; 2] = [PixelFormat::XRGB8888, PixelFormat::ARGB8888];
const RGB_YUV: [PixelFormat; 3] = [
    PixelFormat::XRGB8888,
    PixelFormat::ARGB8888,
    PixelFormat::NV12,
];
const ALL_BLENDS: u8 = 0b111;

/// Two planes: one scales, the other does not; both take RGB and YUV.
fn am62p_like() -> Capabilities {
    let mut caps = base();
    let scale = ScaleLimits {
        max_upscale: 8 << 16,
        max_downscale: 4 << 16,
    };
    caps.planes
        .push(plane(31, true, 1, Some(scale), ALL_BLENDS, &RGB_YUV))
        .unwrap();
    caps.planes
        .push(plane(41, false, 1, None, ALL_BLENDS, &RGB_YUV))
        .unwrap();
    caps
}

/// Six like planes, but only four scaler/YUV units and one scaler per axis.
fn imx95_like() -> Capabilities {
    let mut caps = base();
    let scale = ScaleLimits {
        max_upscale: 2048 << 16,
        max_downscale: 1 << 16,
    };
    for i in 0..6u32 {
        caps.planes
            .push(plane(i + 1, i == 0, 5, Some(scale), ALL_BLENDS, &RGB_YUV))
            .unwrap();
    }
    for (consumers, capacity) in [
        (LayerTraits::SCALED.union(LayerTraits::YUV), 4),
        (LayerTraits::SCALED_H, 1),
        (LayerTraits::SCALED_V, 1),
    ] {
        caps.resource_groups
            .push(ResourceGroup {
                consumers,
                planes: 0b11_1111,
                capacity,
            })
            .unwrap();
    }
    caps
}

/// Three planes, no scaling, premultiplied blend only, NV12 on one plane.
fn ltdc_like() -> Capabilities {
    let mut caps = base();
    let premul = BlendMode::Premultiplied.mask();
    caps.planes
        .push(plane(34, true, 2, None, premul, &RGB))
        .unwrap();
    caps.planes
        .push(plane(42, false, 2, None, premul, &RGB_YUV))
        .unwrap();
    caps.planes
        .push(plane(49, false, 2, None, premul, &RGB))
        .unwrap();
    caps
}

fn main() {
    let ui = BufferId(1);
    let video = BufferId(2);
    let ui2 = BufferId(3);
    let cursor = BufferId(4);
    let buffers = Buffers(vec![
        (ui, info(1920, 1080, PixelFormat::XRGB8888)),
        (video, info(1280, 720, PixelFormat::NV12)),
        (ui2, info(640, 1080, PixelFormat::ARGB8888)),
        (cursor, info(64, 64, PixelFormat::ARGB8888)),
    ]);
    let premul = |mut l: Layer| {
        l.blend = BlendMode::Premultiplied;
        l
    };
    let layers = [
        Layer::new(
            ui,
            Rect::new(0, 0, 1920, 1080),
            Rect::new(0, 0, 1920, 1080),
            0,
        ),
        Layer::new(
            video,
            Rect::new(0, 0, 1280, 720),
            Rect::new(0, 0, 1920, 1080),
            1,
        ),
        premul(Layer::new(
            ui2,
            Rect::new(0, 0, 640, 1080),
            Rect::new(1280, 0, 640, 1080),
            2,
        )),
        premul(Layer::new(
            cursor,
            Rect::new(0, 0, 64, 64),
            Rect::new(900, 500, 64, 64),
            3,
        )),
    ];
    let names = ["ui", "video", "ui2", "cursor"];
    let frame = Frame {
        output: OUT,
        layers: &layers,
    };
    let options = AdviseOptions {
        compose: Some(ComposeTarget {
            format: PixelFormat::XRGB8888,
            modifier: Modifier::LINEAR,
            blend: BlendMode::Opaque,
        }),
        ..AdviseOptions::default()
    };

    let sets: [(&str, Capabilities); 3] = [
        ("am62p-like (2 planes, 1 scaler)", am62p_like()),
        ("imx95-like (6 planes, shared units)", imx95_like()),
        ("ltdc-like (3 planes, no scaling)", ltdc_like()),
    ];
    for (name, caps) in &sets {
        println!("== {name}");
        match advise(&frame, caps, &buffers, &options, |_, _| Verdict::Accept) {
            Ok(advice) => {
                for (i, l) in advice.layers().iter().enumerate() {
                    let place = match l.placement {
                        Placement::Plane(a) => format!("plane {}", a.plane.0),
                        Placement::Compose => "compose".to_string(),
                        Placement::Hidden => "hidden".to_string(),
                    };
                    println!("  {:<7} {:<10} blocker={:?}", names[i], place, l.blocker);
                }
                println!("  promoted={}", advice.promoted());
            }
            Err(e) => println!("  error: {e:?}"),
        }
    }
}
