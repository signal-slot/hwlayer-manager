//! Dumps the raw KMS state of a DRM device: CRTCs, connectors and planes with
//! every property the kernel exposes.
//!
//! Usage: `props /dev/dri/cardN`
//!
//! Unlike `probe`, which prints only what composition-drm derives (zpos,
//! alpha, blend modes, formats and modifiers as the planner sees them), this
//! prints what the kernel exposes: each property by name with its type, flags
//! and current value, enum values by name, range limits, the `possible_crtcs`
//! mask of each plane and the decoded `IN_FORMATS` blob. Use both on a board
//! session to tell what the kernel offers from what the engine derives.
//!
//! The example only reads: it sets the universal-planes and atomic client
//! capabilities (neither needs DRM master), then queries. It never sets a
//! property, creates a blob or commits. The `drm` crate does not return the
//! value names of bitmask properties (such as `rotation`), so those print the
//! raw value, and `rotation` additionally the bits decoded with the standard
//! DRM_MODE_ROTATE_* / DRM_MODE_REFLECT_* positions, which is an assumption
//! about the kernel's naming, not something read from it.

use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};

use drm::control::{property, Device as ControlDevice, ResourceHandle};
use drm::{ClientCapability, Device as BasicDevice};

struct Card(std::fs::File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl BasicDevice for Card {}
impl ControlDevice for Card {}

fn fourcc(code: u32) -> String {
    code.to_le_bytes()
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

fn rotation_bits(value: u64) -> String {
    let names = [
        "rotate-0",
        "rotate-90",
        "rotate-180",
        "rotate-270",
        "reflect-x",
        "reflect-y",
    ];
    let set: Vec<&str> = names
        .iter()
        .enumerate()
        .filter(|(bit, _)| value & (1 << bit) != 0)
        .map(|(_, n)| *n)
        .collect();
    set.join("|")
}

/// Decodes an `IN_FORMATS` blob (struct drm_format_modifier_blob) into one
/// line per modifier.
fn print_in_formats(blob: &[u8]) {
    let u32_at = |offset: usize| {
        blob.get(offset..offset + 4)
            .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
    };
    let u64_at = |offset: usize| {
        blob.get(offset..offset + 8)
            .map(|b| u64::from_ne_bytes(b.try_into().unwrap()))
    };
    let (Some(count_formats), Some(formats_offset), Some(count_modifiers), Some(modifiers_offset)) =
        (u32_at(8), u32_at(12), u32_at(16), u32_at(20))
    else {
        println!("      IN_FORMATS blob too short ({} bytes)", blob.len());
        return;
    };
    println!(
        "      IN_FORMATS: {} formats, {} modifiers",
        count_formats, count_modifiers
    );
    for m in 0..count_modifiers as usize {
        let base = modifiers_offset as usize + m * 24;
        let (Some(mask), Some(first), Some(modifier)) =
            (u64_at(base), u32_at(base + 8), u64_at(base + 16))
        else {
            println!("      modifier entry {m} is truncated");
            break;
        };
        let mut formats = String::new();
        for bit in 0..64usize {
            let index = first as usize + bit;
            if mask & (1 << bit) == 0 || index >= count_formats as usize {
                continue;
            }
            if let Some(code) = u32_at(formats_offset as usize + index * 4) {
                if !formats.is_empty() {
                    formats.push(' ');
                }
                formats.push_str(&fourcc(code));
            }
        }
        println!("      modifier {modifier:#018x}: {formats}");
    }
}

fn describe(card: &Card, name: &str, info: &property::Info, value: u64) -> io::Result<()> {
    let mut line = format!("    {name} (id {}): ", u32::from(info.handle()));
    let mut flags = Vec::new();
    if !info.mutable() {
        flags.push("immutable");
    }
    if info.atomic() {
        flags.push("atomic");
    }
    let mut blob = None;
    let kind = match info.value_type() {
        property::ValueType::Unknown => "unknown".to_string(),
        property::ValueType::Boolean => "bool".to_string(),
        property::ValueType::UnsignedRange(min, max) => format!("range {min}..={max}"),
        property::ValueType::SignedRange(min, max) => format!("signed range {min}..={max}"),
        property::ValueType::Enum(values) => {
            let (raw, enums) = values.values();
            let mut text = String::from("enum {");
            for (i, (v, e)) in raw.iter().zip(enums).enumerate() {
                if i > 0 {
                    text.push_str(", ");
                }
                let _ = write!(text, "{}={v}", e.name().to_string_lossy());
            }
            text.push('}');
            text
        }
        property::ValueType::Bitmask => "bitmask".to_string(),
        property::ValueType::Blob => {
            blob = Some(value);
            "blob".to_string()
        }
        property::ValueType::Object => "object".to_string(),
        property::ValueType::CRTC => "object crtc".to_string(),
        property::ValueType::Connector => "object connector".to_string(),
        property::ValueType::Encoder => "object encoder".to_string(),
        property::ValueType::Framebuffer => "object framebuffer".to_string(),
        property::ValueType::Plane => "object plane".to_string(),
        property::ValueType::Property => "object property".to_string(),
    };
    line.push_str(&kind);
    if !flags.is_empty() {
        let _ = write!(line, " [{}]", flags.join(","));
    }
    let _ = write!(line, " = {value}");
    match info.value_type() {
        property::ValueType::Enum(values) => {
            if let Some(e) = values.get_value_from_raw_value(value) {
                let _ = write!(line, " ({})", e.name().to_string_lossy());
            }
        }
        property::ValueType::Bitmask if name == "rotation" => {
            let _ = write!(line, " ({})", rotation_bits(value));
        }
        property::ValueType::Bitmask => {
            let _ = write!(line, " ({value:#x})");
        }
        _ => {}
    }
    println!("{line}");
    if let (Some(id), "IN_FORMATS") = (blob, name) {
        if id != 0 {
            print_in_formats(&card.get_property_blob(id)?);
        }
    }
    Ok(())
}

fn properties<H: ResourceHandle>(card: &Card, handle: H) -> io::Result<()> {
    let set = card.get_properties(handle)?;
    let mut all = Vec::new();
    for (prop, value) in set.iter() {
        let info = card.get_property(*prop)?;
        all.push((info.name().to_string_lossy().into_owned(), info, *value));
    }
    all.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, info, value) in &all {
        describe(card, name, info, *value)?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/dev/dri/card0".into());
    let card = Card(OpenOptions::new().read(true).write(true).open(&path)?);
    card.set_client_capability(ClientCapability::UniversalPlanes, true)?;
    card.set_client_capability(ClientCapability::Atomic, true)?;

    let driver = card.get_driver()?;
    let (major, minor, patch) = driver.version;
    println!(
        "device {path}, driver {} {major}.{minor}.{patch} ({}), date {}",
        driver.name().to_string_lossy(),
        driver.description().to_string_lossy(),
        driver.date().to_string_lossy()
    );

    let resources = card.resource_handles()?;
    // Plane possible_crtcs bits refer to the position in this list.
    for (index, handle) in resources.crtcs().iter().enumerate() {
        let info = card.get_crtc(*handle)?;
        let mode = match info.mode() {
            Some(m) => {
                let (w, h) = m.size();
                format!("{w}x{h}@{}", m.vrefresh())
            }
            None => "no mode".to_string(),
        };
        println!("crtc {} (index {index}): {mode}", u32::from(*handle));
        properties(&card, *handle)?;
    }
    for handle in resources.connectors() {
        let info = card.get_connector(*handle, false)?;
        println!(
            "connector {} {:?}-{}: {:?}, {} modes",
            u32::from(*handle),
            info.interface(),
            info.interface_id(),
            info.state(),
            info.modes().len()
        );
        properties(&card, *handle)?;
    }
    for handle in card.plane_handles()? {
        let info = card.get_plane(handle)?;
        let allowed = resources.filter_crtcs(info.possible_crtcs());
        let indices: Vec<usize> = resources
            .crtcs()
            .iter()
            .enumerate()
            .filter(|(_, c)| allowed.contains(*c))
            .map(|(i, _)| i)
            .collect();
        let mask = indices.iter().fold(0u32, |m, i| m | 1 << i);
        let crtcs = indices
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "plane {}: possible_crtcs {mask:#b} (crtc indices [{crtcs}]), current crtc {:?}, fb {:?}",
            u32::from(handle),
            info.crtc().map(u32::from),
            info.framebuffer().map(u32::from)
        );
        properties(&card, handle)?;
    }
    Ok(())
}
