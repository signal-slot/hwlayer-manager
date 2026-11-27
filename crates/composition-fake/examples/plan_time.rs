//! Plane assignment time on the host (SoW §10 metric).
//!
//! Times `Engine::validate` (planner search plus the fake backend's check)
//! for several capability sets and frames, using `std::time::Instant` only.
//! This is a host measurement of the portable core, not a hardware result.
//!
//! Run with `cargo run --release -p composition-fake --example plan_time`.
//! An optional argument sets the number of iterations per case.

use std::time::Instant;

use composition_core::limits::{MAX_LAYERS, MAX_PLANES, MAX_RESOURCE_GROUPS};
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, Engine, Error, FormatModifier, Frame, Layer,
    LayerTraits, Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId,
    PlaneKind, Rect, ResourceGroup, ScaleLimits, Size, Transform, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const SCREEN: Size = Size::new(1920, 1080);
/// Source crops: `CROP` is the destination size, so it is unscaled;
/// `HALF` is half of it in each direction, so it is a 2x upscale.
const CROP: Rect = Rect::new(0, 0, 1280, 720);
const HALF: Rect = Rect::new(0, 0, 640, 360);

const RGB: BufferId = BufferId(1);
const ARGB: BufferId = BufferId(2);
const VIDEO: BufferId = BufferId(3);
const THUMB: BufferId = BufferId(4);

const ONE: u32 = 0x1_0000;

fn plane(index: usize) -> PlaneCaps {
    let primary = index == 0;
    let mut caps = PlaneCaps {
        id: PlaneId(30 + index as u32),
        kind: if primary {
            PlaneKind::Primary
        } else {
            PlaneKind::Overlay
        },
        outputs: 1,
        zpos: if primary {
            Zpos::Fixed(0)
        } else {
            Zpos::Range {
                min: 1,
                max: MAX_PLANES as i32,
            }
        },
        plane_alpha: !primary,
        blend_modes: BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask(),
        transforms: Transform::Normal.mask(),
        ..PlaneCaps::default()
    };
    let formats: &[PixelFormat] = if primary {
        &[PixelFormat::XRGB8888, PixelFormat::ARGB8888]
    } else {
        &[
            PixelFormat::XRGB8888,
            PixelFormat::ARGB8888,
            PixelFormat::NV12,
        ]
    };
    for format in formats {
        caps.formats.push(FormatModifier::linear(*format)).unwrap();
    }
    if !primary {
        caps.scaling = Some(ScaleLimits {
            max_upscale: 8 * ONE,
            max_downscale: 4 * ONE,
        });
    }
    caps
}

fn caps(planes: usize, groups: &[ResourceGroup]) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SCREEN,
            refresh_mhz: 60_000,
        })
        .unwrap();
    for index in 0..planes {
        caps.planes.push(plane(index)).unwrap();
    }
    for group in groups {
        caps.resource_groups.push(*group).unwrap();
    }
    caps
}

/// Eight planes with shared resources in the style of a DPU: two scalers
/// shared by planes 2..=5, one YUV converter shared by planes 4..=7, and at
/// most six enabled planes overall.
fn dpu_groups() -> [ResourceGroup; 3] {
    [
        ResourceGroup {
            consumers: LayerTraits::SCALED,
            planes: 0b0011_1100,
            capacity: 2,
        },
        ResourceGroup {
            consumers: LayerTraits::YUV,
            planes: 0b1111_0000,
            capacity: 1,
        },
        ResourceGroup {
            consumers: LayerTraits::ANY,
            planes: 0b1111_1111,
            capacity: 6,
        },
    ]
}

/// The largest description the limits allow: every resource group, over
/// every plane but one.
fn max_groups() -> [ResourceGroup; MAX_RESOURCE_GROUPS] {
    let mut groups = [ResourceGroup::default(); MAX_RESOURCE_GROUPS];
    let all = ((1u64 << MAX_PLANES) - 1) as u32;
    for (i, group) in groups.iter_mut().enumerate() {
        *group = ResourceGroup {
            consumers: match i % 3 {
                0 => LayerTraits::SCALED,
                1 => LayerTraits::YUV,
                _ => LayerTraits::ANY,
            },
            planes: all & !(1 << (i % 4)),
            capacity: MAX_LAYERS as u8,
        };
    }
    groups
}

#[derive(Clone, Copy)]
enum Mix {
    Plain,
    Scaled,
    Yuv,
    Alpha,
    /// Cycles through the other kinds, with z-order opposite to frame order.
    Mixed,
}

impl Mix {
    const ALL: [Mix; 5] = [Mix::Plain, Mix::Scaled, Mix::Yuv, Mix::Alpha, Mix::Mixed];

    fn name(self) -> &'static str {
        match self {
            Mix::Plain => "plain",
            Mix::Scaled => "scaled",
            Mix::Yuv => "yuv",
            Mix::Alpha => "alpha",
            Mix::Mixed => "mixed",
        }
    }
}

fn layer(mix: Mix, index: usize, count: usize) -> Layer {
    let at = (index as i32) * 16;
    let dest = Rect::new(at, at, 1280, 720);
    let z = index as i32;
    match mix {
        Mix::Plain => Layer::new(RGB, CROP, dest, z),
        Mix::Scaled => Layer::new(THUMB, HALF, dest, z),
        Mix::Yuv => Layer::new(VIDEO, CROP, dest, z),
        Mix::Alpha => {
            let mut l = Layer::new(ARGB, CROP, dest, z);
            l.blend = BlendMode::Premultiplied;
            l.alpha = 0x8000;
            l
        }
        Mix::Mixed => {
            let kind = [Mix::Plain, Mix::Scaled, Mix::Yuv, Mix::Alpha][index % 4];
            let mut l = layer(kind, index, count);
            l.z_order = (count - 1 - index) as i32;
            l
        }
    }
}

fn engine(caps: Capabilities) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(caps));
    for (buffer, size, format) in [
        (RGB, SCREEN, PixelFormat::XRGB8888),
        (ARGB, SCREEN, PixelFormat::ARGB8888),
        (VIDEO, SCREEN, PixelFormat::NV12),
        (THUMB, Size::new(960, 540), PixelFormat::XRGB8888),
    ] {
        let info = BufferInfo {
            size,
            format,
            modifier: Modifier::LINEAR,
        };
        engine.register_buffer(buffer, info).unwrap();
        engine.finish_write(buffer).unwrap();
    }
    engine
}

fn plan_error_name(error: PlanError) -> &'static str {
    match error {
        PlanError::UnknownOutput(_) => "UnknownOutput",
        PlanError::TooManyLayers { .. } => "TooManyLayers",
        PlanError::UnknownBuffer { .. } => "UnknownBuffer",
        PlanError::InvalidSource { .. } => "InvalidSource",
        PlanError::InvalidDestination { .. } => "InvalidDestination",
        PlanError::CannotClip { .. } => "CannotClip",
        PlanError::NoPlane { .. } => "NoPlane",
        PlanError::BackendRejected { .. } => "BackendRejected",
        PlanError::LimitExceeded => "LimitExceeded",
        PlanError::Aborted => "Aborted",
    }
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    let rank = ((sorted.len() as f64) * p).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

fn main() {
    let iterations: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000)
        .max(10);
    let profiles: [(&str, Capabilities); 4] = [
        ("2 planes", caps(2, &[])),
        ("4 planes", caps(4, &[])),
        ("8 planes, DPU-like groups", caps(8, &dpu_groups())),
        ("max (16 planes, 8 groups)", caps(MAX_PLANES, &max_groups())),
    ];
    let counts = [1, 2, 4, MAX_LAYERS];

    println!("iterations per case: {iterations}; times in ns per Engine::validate call");
    println!(
        "{:<27} {:>6} {:<7} {:<10} {:>8} {:>8} {:>8} {:>9}",
        "capabilities", "layers", "mix", "result", "min", "median", "p99", "max"
    );
    for (name, caps) in profiles {
        let mut engine = engine(caps);
        for &count in &counts {
            for mix in Mix::ALL {
                let layers: Vec<Layer> = (0..count).map(|i| layer(mix, i, count)).collect();
                let frame = Frame {
                    output: OUT,
                    layers: &layers,
                };
                let result = match engine.validate(&frame) {
                    Ok(_) => "ok".to_string(),
                    Err(Error::Plan(e)) => plan_error_name(e).to_string(),
                    Err(_) => "error".to_string(),
                };
                for _ in 0..iterations / 10 {
                    std::hint::black_box(engine.validate(std::hint::black_box(&frame)).ok());
                }
                let mut samples = Vec::with_capacity(iterations);
                for _ in 0..iterations {
                    let start = Instant::now();
                    let out = engine.validate(std::hint::black_box(&frame));
                    let elapsed = start.elapsed().as_nanos() as u64;
                    std::hint::black_box(out.ok());
                    samples.push(elapsed);
                }
                samples.sort_unstable();
                println!(
                    "{:<27} {:>6} {:<7} {:<10} {:>8} {:>8} {:>8} {:>9}",
                    name,
                    count,
                    mix.name(),
                    result,
                    samples[0],
                    percentile(&samples, 0.5),
                    percentile(&samples, 0.99),
                    samples[samples.len() - 1],
                );
            }
        }
    }
}
