//! Prints the capabilities the backend derives from a DRM device.
//!
//! Usage: `probe /dev/dri/cardN`
//!
//! This is the report to capture on a target board: it shows the outputs and
//! planes the descriptor really exposes, as the planner will see them.

use composition_core::{BlendMode, DisplayBackend, PixelFormat, Zpos};
use composition_drm::{DrmBackend, DrmOptions};

fn fourcc(format: PixelFormat) -> String {
    format
        .0
        .to_le_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_graphic() {
                *b as char
            } else {
                '?'
            }
        })
        .collect()
}

fn main() -> std::io::Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/dev/dri/card0".into());
    let backend = DrmBackend::open(&path, DrmOptions::default())?;
    println!("device {path}, driver {}", backend.driver_name()?);

    let caps = backend.capabilities();
    for (index, output) in caps.outputs.iter().enumerate() {
        println!(
            "output {index}: crtc {} {}x{} @ {}.{:03} Hz",
            output.id.0,
            output.size.width,
            output.size.height,
            output.refresh_mhz / 1000,
            output.refresh_mhz % 1000
        );
    }
    for plane in caps.planes.iter() {
        let zpos = match plane.zpos {
            Zpos::Fixed(z) => format!("fixed {z}"),
            Zpos::Range { min, max } => format!("{min}..={max}"),
        };
        let mut blend = Vec::new();
        for (mode, name) in [
            (BlendMode::Opaque, "none"),
            (BlendMode::Premultiplied, "premultiplied"),
            (BlendMode::Coverage, "coverage"),
        ] {
            if plane.blend_modes & mode.mask() != 0 {
                blend.push(name);
            }
        }
        println!(
            "plane {} {:?}: outputs {:#b}, zpos {zpos}, plane alpha {}, blend [{}], scaling {}",
            plane.id.0,
            plane.kind,
            plane.outputs,
            plane.plane_alpha,
            blend.join(", "),
            if plane.scaling.is_some() {
                "assumed"
            } else {
                "not claimed"
            },
        );
        let formats: Vec<String> = plane
            .formats
            .iter()
            .map(|f| {
                if f.modifier.0 == 0 {
                    fourcc(f.format)
                } else {
                    format!("{}:{:#x}", fourcc(f.format), f.modifier.0)
                }
            })
            .collect();
        println!("  formats ({}): {}", formats.len(), formats.join(" "));
    }
    Ok(())
}
