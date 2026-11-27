//! Plane assignment time benchmark (SoW §10 metric "plane assignment time").
//!
//! Times `Engine::validate` over N iterations for frames of 1, 2, 4 and 8
//! layers on two fake display controllers: 1 primary + 3 overlays, and a
//! vkms-like 1 primary + 8 overlays. Each frame size runs twice: once with
//! layers the planes accept, once with a last layer in a format no plane
//! supports, so that the frame is refused.
//!
//! The numbers measure the planner and the fake backend's check only. They do
//! not include DRM `TEST_ONLY` commits, ioctls or any hardware, and a run on
//! a host says nothing about the board: only a run of the aarch64 build on
//! the board is a board number, and even that is still planner-only.
//!
//! Run with `cargo run --release -p composition-fake --example plan_bench`.
//! An optional argument sets the iterations per case (default 20000).
//! Cross-build: `scripts/cross-aarch64.sh -p composition-fake --example plan_bench`.

use std::time::Instant;

use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, Engine, Error, FormatModifier, Frame, Layer,
    Modifier, OutputCaps, OutputId, PixelFormat, PlaneCaps, PlaneId, PlaneKind, Rect, Size,
    Transform, Zpos,
};
use composition_fake::FakeBackend;

const OUT: OutputId = OutputId(0);
const SCREEN: Size = Size::new(1280, 800);
const FULL: Rect = Rect::new(0, 0, 1280, 800);
const GOOD: BufferId = BufferId(1);
/// RGB565: no plane below supports it.
const BAD: BufferId = BufferId(2);

fn caps(overlays: usize) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SCREEN,
            refresh_mhz: 60_000,
        })
        .unwrap();
    for index in 0..=overlays {
        let primary = index == 0;
        let mut plane = PlaneCaps {
            id: PlaneId(30 + index as u32),
            kind: if primary {
                PlaneKind::Primary
            } else {
                PlaneKind::Overlay
            },
            outputs: 1,
            zpos: Zpos::Fixed(index as i32),
            blend_modes: BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask(),
            transforms: Transform::Normal.mask(),
            ..PlaneCaps::default()
        };
        for format in [PixelFormat::XRGB8888, PixelFormat::ARGB8888] {
            plane.formats.push(FormatModifier::linear(format)).unwrap();
        }
        caps.planes.push(plane).unwrap();
    }
    caps
}

fn engine(caps: Capabilities) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(caps));
    for (buffer, format) in [(GOOD, PixelFormat::ARGB8888), (BAD, PixelFormat::RGB565)] {
        let info = BufferInfo {
            size: SCREEN,
            format,
            modifier: Modifier::LINEAR,
        };
        engine.register_buffer(buffer, info).unwrap();
        engine.finish_write(buffer).unwrap();
    }
    engine
}

fn layers(count: usize, refused: bool) -> Vec<Layer> {
    (0..count)
        .map(|i| {
            let buffer = if refused && i == count - 1 { BAD } else { GOOD };
            let mut layer = Layer::new(buffer, FULL, FULL, i as i32);
            layer.blend = BlendMode::Premultiplied;
            layer
        })
        .collect()
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
    println!(
        "plan_bench: profile={} target={}-{}{} iterations={iterations}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::env::consts::ARCH,
        std::env::consts::OS,
        if cfg!(target_env = "gnu") {
            "-gnu"
        } else if cfg!(target_env = "musl") {
            "-musl"
        } else {
            ""
        },
    );
    println!("planner + fake backend check only; not DRM TEST_ONLY, not hardware");
    println!(
        "{:<12} {:>6} {:<8} {:<8} {:>9} {:>9} {:>9} {:>9}",
        "controller", "layers", "frame", "result", "min_us", "median_us", "p99_us", "max_us"
    );
    for (name, overlays) in [("1+3", 3), ("1+8 (vkms)", 8)] {
        let mut engine = engine(caps(overlays));
        for count in [1, 2, 4, 8] {
            for refused in [false, true] {
                let layers = layers(count, refused);
                let frame = Frame {
                    output: OUT,
                    layers: &layers,
                };
                let result = match engine.validate(&frame) {
                    Ok(_) => "ok",
                    Err(Error::Plan(_)) => "refused",
                    Err(_) => "error",
                };
                for _ in 0..iterations / 10 {
                    std::hint::black_box(engine.validate(std::hint::black_box(&frame)).ok());
                }
                let mut samples = Vec::with_capacity(iterations);
                for _ in 0..iterations {
                    let start = Instant::now();
                    let out = engine.validate(std::hint::black_box(&frame));
                    let ns = start.elapsed().as_nanos() as u64;
                    std::hint::black_box(out.ok());
                    samples.push(ns);
                }
                samples.sort_unstable();
                let us = |ns: u64| ns as f64 / 1000.0;
                println!(
                    "{:<12} {:>6} {:<8} {:<8} {:>9.3} {:>9.3} {:>9.3} {:>9.3}",
                    name,
                    count,
                    if refused { "refused" } else { "normal" },
                    result,
                    us(samples[0]),
                    us(percentile(&samples, 0.5)),
                    us(percentile(&samples, 0.99)),
                    us(samples[samples.len() - 1]),
                );
            }
        }
    }
}
