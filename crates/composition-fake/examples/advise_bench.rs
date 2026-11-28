//! Host timing of `composition_core::advise`.
//!
//! Times `advise` for scenes of 2, 4 and 8 layers on fake display controllers
//! of 2, 4 and 8 planes, using `std::time::Instant` only. Each scene runs in
//! two modes: the frame must fit on planes (composition forbidden), and
//! composition into one buffer is allowed. The check closure is
//! `|_, _| Verdict::Accept`, so the figures cover the advisory planner only:
//! no DRM `TEST_ONLY` commit, no ioctl, no hardware, no `DriverProfile` data
//! (that lives in `composition-drm`, which this crate does not depend on).
//!
//! This is a host measurement. It is not a board result and not a comparison
//! with libliftoff.
//!
//! Run with `cargo run --release -p composition-fake --example advise_bench`.
//! An optional argument sets the iterations per case (default 20000).

#![forbid(unsafe_code)]

use std::time::Instant;

use composition_core::{
    advise, AdviseOptions, BlendMode, BufferId, BufferInfo, BufferLookup, Capabilities,
    ComposeTarget, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat,
    PlaneCaps, PlaneId, PlaneKind, Rect, ScaleLimits, Size, Transform, Verdict, Zpos,
};

const OUT: OutputId = OutputId(0);
const SCREEN: Size = Size::new(1920, 1080);
const CROP: Rect = Rect::new(0, 0, 1280, 720);
const HALF: Rect = Rect::new(0, 0, 640, 360);
const ONE: u32 = 0x1_0000;

const RGB: BufferId = BufferId(1);
const VIDEO: BufferId = BufferId(2);
const THUMB: BufferId = BufferId(3);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let (size, format) = match buffer {
            RGB => (SCREEN, PixelFormat::ARGB8888),
            VIDEO => (SCREEN, PixelFormat::NV12),
            THUMB => (Size::new(960, 540), PixelFormat::XRGB8888),
            _ => return None,
        };
        Some(BufferInfo {
            size,
            format,
            modifier: Modifier::LINEAR,
        })
    }
}

fn plane(index: usize, planes: usize) -> PlaneCaps {
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
                max: planes as i32,
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

fn caps(planes: usize) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SCREEN,
            refresh_mhz: 60_000,
        })
        .unwrap();
    for index in 0..planes {
        caps.planes.push(plane(index, planes)).unwrap();
    }
    caps
}

/// A mixed scene: translucent ARGB, scaled thumbnail and video layers in turn.
fn layer(index: usize) -> Layer {
    let at = (index as i32) * 16;
    let dest = Rect::new(at, at, 1280, 720);
    let z = index as i32;
    match index % 3 {
        0 => {
            let mut l = Layer::new(RGB, CROP, dest, z);
            l.blend = BlendMode::Premultiplied;
            l.alpha = 0x8000;
            l
        }
        1 => Layer::new(THUMB, HALF, dest, z),
        _ => Layer::new(VIDEO, CROP, dest, z),
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
    let target = ComposeTarget {
        format: PixelFormat::ARGB8888,
        modifier: Modifier::LINEAR,
        blend: BlendMode::Premultiplied,
    };
    let modes = [("planes-only", None), ("compose-allowed", Some(target))];

    println!("iterations per case: {iterations}; times in ns per advise call");
    println!(
        "{:<7} {:>6} {:<16} {:<12} {:>8} {:>8} {:>9}",
        "planes", "layers", "mode", "result", "median", "p99", "max"
    );
    for planes in [2, 4, 8] {
        let caps = caps(planes);
        for count in [2usize, 4, 8] {
            let layers: Vec<Layer> = (0..count).map(layer).collect();
            let frame = Frame {
                output: OUT,
                layers: &layers,
            };
            for (mode, compose) in modes {
                let options = AdviseOptions {
                    compose,
                    ..AdviseOptions::default()
                };
                let run = || advise(&frame, &caps, &Buffers, &options, |_, _| Verdict::Accept);
                let result = match run() {
                    Ok(advice) if advice.is_direct() => "direct".to_string(),
                    Ok(_) => "not direct".to_string(),
                    Err(_) => "error".to_string(),
                };
                for _ in 0..iterations / 10 {
                    std::hint::black_box(run().ok());
                }
                let mut samples = Vec::with_capacity(iterations);
                for _ in 0..iterations {
                    let start = Instant::now();
                    let out = run();
                    let elapsed = start.elapsed().as_nanos() as u64;
                    std::hint::black_box(out.ok());
                    samples.push(elapsed);
                }
                samples.sort_unstable();
                println!(
                    "{:<7} {:>6} {:<16} {:<12} {:>8} {:>8} {:>9}",
                    planes,
                    count,
                    mode,
                    result,
                    percentile(&samples, 0.5),
                    percentile(&samples, 0.99),
                    samples[samples.len() - 1],
                );
            }
        }
    }
}
