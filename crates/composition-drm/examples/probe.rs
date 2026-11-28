//! Prints the capabilities the backend derives from a DRM device.
//!
//! Usage: `probe [--raw] [/dev/dri/cardN]`
//!
//! This is the report to capture on a target board: it shows the outputs and
//! planes the descriptor really exposes, as the planner will see them. With
//! `--raw` it additionally dumps the unprocessed KMS view — every connector,
//! CRTC and plane with all their properties — so an observation can be
//! recorded as the kernel exposes it, not as the backend reads it. The probe
//! only reads resources; it never commits, so it does not change what is on
//! the display and does not need to become DRM master.

use std::io;
use std::process::ExitCode;

use composition_core::{BlendMode, Capabilities, DisplayBackend, PlaneKind, Zpos};
use composition_drm::{Card, DrmBackend, DrmOptions};
use drm::control::{crtc, plane, property, Device as ControlDevice, PlaneType, ResourceHandle};
use drm::{ClientCapability, Device as BasicDevice, DriverCapability};

fn fourcc(format: u32) -> String {
    format
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

fn main() -> ExitCode {
    let mut raw = false;
    let mut path: Option<String> = None;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--raw" => raw = true,
            other if other.starts_with('-') => {
                eprintln!("probe: unknown argument {other}");
                eprintln!("usage: probe [--raw] [/dev/dri/cardN]");
                return ExitCode::from(2);
            }
            other => {
                if path.is_some() {
                    eprintln!("probe: more than one device given");
                    eprintln!("usage: probe [--raw] [/dev/dri/cardN]");
                    return ExitCode::from(2);
                }
                path = Some(other.to_owned());
            }
        }
    }
    let path = path.unwrap_or_else(|| "/dev/dri/card0".to_owned());
    match run(raw, &path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("probe: {path}: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(raw: bool, path: &str) -> io::Result<()> {
    let mut backend = DrmBackend::open(path, DrmOptions::default())?;
    println!("device {path}, driver {}", backend.driver_name()?);

    let mut caps = Capabilities::default();
    backend.capabilities(&mut caps);
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
                    fourcc(f.format.0)
                } else {
                    format!("{}:{:#x}", fourcc(f.format.0), f.modifier.0)
                }
            })
            .collect();
        println!("  formats ({}): {}", formats.len(), formats.join(" "));
    }

    // Computed in probe: the kept set is what caps.planes holds (the backend
    // already applied its MAX_PLANES cut); the dropped reachable planes are
    // the card's planes missing from it that can still reach an output.
    let all = all_plane_reach(backend.card(), &caps)?;
    let kept: Vec<KeptPlane> = caps
        .planes
        .iter()
        .map(|p| KeptPlane {
            id: p.id.0,
            kind: p.kind,
            outputs: p.outputs,
        })
        .collect();
    let crtcs: Vec<u32> = caps.outputs.iter().map(|o| o.id.0).collect();
    for line in kept_lines(&crtcs, &kept, &all) {
        println!("{line}");
    }

    if raw {
        dump_raw(backend.card())?;
    }
    Ok(())
}

/// A plane the backend kept: id, kind, and the output bit set (bit `n` is
/// output `n`).
#[derive(Clone, Copy, Debug)]
struct KeptPlane {
    id: u32,
    kind: PlaneKind,
    outputs: u32,
}

/// Every plane the card exposes with the outputs it can reach, as
/// `(id, outputs bit set)`.
fn all_plane_reach(card: &Card, caps: &Capabilities) -> io::Result<Vec<(u32, u32)>> {
    let resources = card.resource_handles()?;
    let mut all = Vec::new();
    for handle in card.plane_handles()? {
        let info = card.get_plane(handle)?;
        let chosen = resources.filter_crtcs(info.possible_crtcs());
        let mut mask = 0u32;
        for (index, output) in caps.outputs.iter().enumerate() {
            if chosen.iter().any(|c| u32::from(*c) == output.id.0) {
                mask |= 1 << index;
            }
        }
        all.push((u32::from(handle), mask));
    }
    Ok(all)
}

fn id_list(ids: &[u32]) -> String {
    if ids.is_empty() {
        return "-".to_owned();
    }
    ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
}

/// One machine-readable line per output:
/// `KEPT output=<n> crtc=<id> primary=<ids> overlay=<ids> cursor=<ids>
/// dropped_reachable=<ids>`; an empty list prints `-`. `crtcs[n]` is the CRTC
/// of output `n`, `kept` the planes the backend kept, `all` every plane of
/// the card with its reachable outputs.
fn kept_lines(crtcs: &[u32], kept: &[KeptPlane], all: &[(u32, u32)]) -> Vec<String> {
    crtcs
        .iter()
        .enumerate()
        .map(|(n, crtc)| {
            let bit = 1u32 << n;
            let of = |kind: PlaneKind| -> Vec<u32> {
                kept.iter()
                    .filter(|p| p.kind == kind && p.outputs & bit != 0)
                    .map(|p| p.id)
                    .collect()
            };
            let dropped: Vec<u32> = all
                .iter()
                .filter(|(id, mask)| mask & bit != 0 && !kept.iter().any(|p| p.id == *id))
                .map(|(id, _)| *id)
                .collect();
            format!(
                "KEPT output={n} crtc={crtc} primary={} overlay={} cursor={} dropped_reachable={}",
                id_list(&of(PlaneKind::Primary)),
                id_list(&of(PlaneKind::Overlay)),
                id_list(&of(PlaneKind::Cursor)),
                id_list(&dropped),
            )
        })
        .collect()
}

/// Refresh in millihertz, as the derived report computes it.
fn refresh_millihertz(mode: &drm::control::Mode) -> u32 {
    let (_, _, htotal) = mode.hsync();
    let (_, _, vtotal) = mode.vsync();
    let total = htotal as u64 * vtotal as u64;
    if total == 0 {
        return 0;
    }
    (mode.clock() as u64 * 1_000_000 / total) as u32
}

/// Shows a possible-CRTC set the way the kernel represents it: a bit per CRTC
/// in the order the resources list them, plus the CRTC ids for reading.
/// `possible_crtcs()` hands out an opaque filter, not the raw mask, so the
/// bits are rebuilt from the list.
fn crtcs_as_mask(crtcs: &[crtc::Handle], chosen: impl Fn(&crtc::Handle) -> bool) -> String {
    let mut bits = 0u32;
    let mut ids = Vec::new();
    for (bit, crtc) in crtcs.iter().enumerate() {
        if chosen(crtc) {
            bits |= 1 << bit;
            ids.push(format!("{}", u32::from(*crtc)));
        }
    }
    format!("{bits:#b} ({})", ids.join(" "))
}

fn print_mode(mode: &drm::control::Mode) -> String {
    let (width, height) = mode.size();
    let mhz = refresh_millihertz(mode);
    format!("{width}x{height} @ {}.{:03} Hz", mhz / 1000, mhz % 1000)
}

fn dump_raw(card: &Card) -> io::Result<()> {
    println!();
    println!("raw");

    let driver = card.get_driver()?;
    println!(
        "driver: name {} version {}.{}.{} date {} description {}",
        driver.name().to_string_lossy(),
        // The GET_VERSION ioctl returns (major, minor, patchlevel).
        driver.version.0,
        driver.version.1,
        driver.version.2,
        driver.date().to_string_lossy(),
        driver.description().to_string_lossy(),
    );
    // DrmBackend::open has already enabled both caps on this descriptor, so
    // asking again reports what the driver supports without changing state.
    let planes = card
        .set_client_capability(ClientCapability::UniversalPlanes, true)
        .is_ok();
    let atomic = card
        .set_client_capability(ClientCapability::Atomic, true)
        .is_ok();
    println!("client caps: universal planes {planes}, atomic {atomic}");
    for (capability, name) in [
        (DriverCapability::AddFB2Modifiers, "addfb2 modifiers"),
        (DriverCapability::DumbBuffer, "dumb buffers"),
        (DriverCapability::DumbPreferShadow, "dumb prefer shadow"),
        (DriverCapability::CursorWidth, "cursor width"),
        (DriverCapability::CursorHeight, "cursor height"),
        (DriverCapability::ASyncPageFlip, "async page flip"),
        (
            DriverCapability::AtomicASyncPageFlip,
            "atomic async page flip",
        ),
    ] {
        driver_cap(card, capability, name);
    }

    let resources = card.resource_handles()?;
    println!(
        "resources: {} connectors, {} crtcs, {} encoders",
        resources.connectors().len(),
        resources.crtcs().len(),
        resources.encoders().len()
    );

    println!("connectors");
    for handle in resources.connectors() {
        let info = card.get_connector(*handle, false)?;
        println!(
            "connector {} interface {:?} ({}) state {:?}",
            u32::from(*handle),
            info.interface(),
            u32::from(info.interface()),
            info.state(),
        );
        match info.current_encoder().map(u32::from) {
            Some(encoder) => println!("  current encoder {encoder}"),
            None => println!("  current encoder none"),
        }
        for encoder in info.encoders() {
            let encoder_info = card.get_encoder(*encoder)?;
            let chosen = resources.filter_crtcs(encoder_info.possible_crtcs());
            println!(
                "  encoder {} possible crtcs {}",
                u32::from(*encoder),
                crtcs_as_mask(resources.crtcs(), |crtc| chosen.contains(crtc)),
            );
        }
        println!("  modes ({}):", info.modes().len());
        for mode in info.modes() {
            println!("    {}", print_mode(mode));
        }
        print_properties(card, *handle)?;
    }

    println!("crtcs");
    for handle in resources.crtcs() {
        let info = card.get_crtc(*handle)?;
        match info.mode() {
            Some(mode) => println!("crtc {} active {}", u32::from(*handle), print_mode(&mode)),
            None => println!("crtc {} inactive", u32::from(*handle)),
        }
        print_properties(card, *handle)?;
    }

    println!("planes");
    for handle in card.plane_handles()? {
        let info = card.get_plane(handle)?;
        let mask = info.possible_crtcs();
        let properties = property_list(card, handle)?;
        let kind = match properties
            .iter()
            .find(|(name, _, _)| name == "type")
            .map(|(_, _, value)| *value)
        {
            Some(value) if value == PlaneType::Primary as u64 => "primary",
            Some(value) if value == PlaneType::Cursor as u64 => "cursor",
            Some(value) if value == PlaneType::Overlay as u64 => "overlay",
            Some(_) => "other",
            None => "none",
        };
        let chosen = resources.filter_crtcs(mask);
        println!(
            "plane {} type {kind} possible crtcs {}",
            u32::from(handle),
            crtcs_as_mask(resources.crtcs(), |crtc| chosen.contains(crtc)),
        );
        match info.crtc().map(u32::from) {
            Some(crtc) => println!("  attached to crtc {crtc}"),
            None => println!("  attached to none"),
        }
        print_formats(card, &info, &properties)?;
        print_properties_of(card, &properties);
    }
    Ok(())
}

fn driver_cap(card: &Card, capability: DriverCapability, name: &str) {
    match card.get_driver_capability(capability) {
        Ok(value) => println!("driver cap {name}: {value}"),
        Err(error) => println!("driver cap {name}: not reported ({error})"),
    }
}

/// A plane's usable format/modifier pairs, from `IN_FORMATS` when the plane
/// has that property and from the plain format list otherwise.
fn print_formats(
    card: &Card,
    info: &plane::Info,
    properties: &[(String, property::Info, u64)],
) -> io::Result<()> {
    match properties
        .iter()
        .find(|(name, _, _)| name == "IN_FORMATS")
        .map(|(_, _, value)| *value)
        .filter(|value| *value != 0)
    {
        Some(blob_id) => match card.get_property_blob(blob_id) {
            Ok(blob) => {
                let pairs = in_formats(&blob);
                let pairs: Vec<String> = pairs
                    .iter()
                    .map(|(format, modifier)| format!("{}:{modifier:#x}", fourcc(*format)))
                    .collect();
                println!("  IN_FORMATS ({}): {}", pairs.len(), pairs.join(" "));
            }
            Err(error) => println!("  IN_FORMATS: unreadable ({error})"),
        },
        None => {
            let formats: Vec<String> = info
                .formats()
                .iter()
                .map(|format| fourcc(*format))
                .collect();
            println!(
                "  formats ({}): linear {}",
                formats.len(),
                formats.join(" ")
            );
        }
    }
    Ok(())
}

/// Decodes an `IN_FORMATS` blob. The blob starts with an 8-byte header, then
/// four u32 (count_formats, formats_offset, count_modifiers,
/// modifiers_offset), then the format array, then 24-byte modifier entries:
/// a 64-bit mask over formats, the u32 offset of its first format, padding,
/// and the 64-bit modifier. Byte order is native, as the kernel writes it.
fn in_formats(blob: &[u8]) -> Vec<(u32, u64)> {
    let u32_at = |offset: usize| {
        blob.get(offset..offset + 4)
            .map(|bytes| u32::from_ne_bytes(bytes.try_into().unwrap()))
    };
    let u64_at = |offset: usize| {
        blob.get(offset..offset + 8)
            .map(|bytes| u64::from_ne_bytes(bytes.try_into().unwrap()))
    };
    let (Some(count_formats), Some(formats_offset), Some(count_modifiers), Some(modifiers_offset)) =
        (u32_at(8), u32_at(12), u32_at(16), u32_at(20))
    else {
        return Vec::new();
    };
    let mut pairs = Vec::new();
    for entry in 0..count_modifiers as usize {
        let base = modifiers_offset as usize + entry * 24;
        let (Some(mask), Some(first), Some(modifier)) =
            (u64_at(base), u32_at(base + 8), u64_at(base + 16))
        else {
            break;
        };
        for bit in 0..64usize {
            let index = first as usize + bit;
            if mask & (1 << bit) == 0 || index >= count_formats as usize {
                continue;
            }
            if let Some(format) = u32_at(formats_offset as usize + index * 4) {
                pairs.push((format, modifier));
            }
        }
    }
    pairs
}

fn property_list<H: ResourceHandle>(
    card: &Card,
    handle: H,
) -> io::Result<Vec<(String, property::Info, u64)>> {
    let mut list = Vec::new();
    for (prop, value) in card.get_properties(handle)?.iter() {
        let info = card.get_property(*prop)?;
        list.push((info.name().to_string_lossy().into_owned(), info, *value));
    }
    Ok(list)
}

fn print_properties<H: ResourceHandle>(card: &Card, handle: H) -> io::Result<()> {
    print_properties_of(card, &property_list(card, handle)?);
    Ok(())
}

fn print_properties_of(card: &Card, properties: &[(String, property::Info, u64)]) {
    println!("  properties:");
    for (_, info, value) in properties {
        println!("    {}", describe_property(card, info, *value));
    }
}

/// One property as one line: name, its kernel flags, its allowed values when
/// the kernel lists them, and its current value. A blob property is reported
/// by size; the bytes themselves are out of scope except for `IN_FORMATS`.
fn describe_property(card: &Card, info: &property::Info, value: u64) -> String {
    let mut line = format!("{}:", info.name().to_string_lossy());
    line.push_str(if info.mutable() {
        " mutable"
    } else {
        " immutable"
    });
    if info.atomic() {
        line.push_str(" atomic");
    }
    match info.value_type() {
        property::ValueType::UnsignedRange(min, max) => {
            line.push_str(&format!(" range {min}..={max}"));
        }
        property::ValueType::SignedRange(min, max) => {
            line.push_str(&format!(" range {min}..={max}"));
        }
        property::ValueType::Enum(values) => {
            let (_, entries) = values.values();
            let entries: Vec<String> = entries
                .iter()
                .map(|entry| format!("{}={}", entry.name().to_string_lossy(), entry.value()))
                .collect();
            line.push_str(&format!(" enum [{}]", entries.join(", ")));
        }
        property::ValueType::Boolean => {
            line.push_str(" boolean");
        }
        property::ValueType::Object => {
            line.push_str(" object");
        }
        property::ValueType::Blob => match card.get_property_blob(value) {
            Ok(blob) => line.push_str(&format!(" blob, {} bytes", blob.len())),
            Err(_) => line.push_str(" blob, unreadable"),
        },
        _ => {}
    }
    line.push_str(&format!(" value {value}"));
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept_plane(id: u32, kind: PlaneKind, outputs: u32) -> KeptPlane {
        KeptPlane { id, kind, outputs }
    }

    #[test]
    fn kept_line_lists_kept_and_dropped_reachable_planes() {
        // 21 planes, ids 1..=21, on two outputs. Output 0 reaches all of
        // them, output 1 only 15..=20.
        let mut all: Vec<(u32, u32)> = (1..=21).map(|id| (id, 0b01)).collect();
        for entry in all.iter_mut().filter(|(id, _)| (15..=20).contains(id)) {
            entry.1 = 0b11;
        }
        assert!(all.len() > 16);
        // The backend kept 16: primary 1, overlays 2..=15, cursor 21.
        let mut kept = vec![kept_plane(1, PlaneKind::Primary, 0b01)];
        kept.extend((2..=15).map(|id| kept_plane(id, PlaneKind::Overlay, 0b01)));
        kept.push(kept_plane(21, PlaneKind::Cursor, 0b01));
        assert_eq!(kept.len(), 16);
        let lines = kept_lines(&[100, 101], &kept, &all);
        assert_eq!(
            lines[0],
            "KEPT output=0 crtc=100 primary=1 overlay=2,3,4,5,6,7,8,9,10,11,12,13,14,15 \
             cursor=21 dropped_reachable=16,17,18,19,20"
        );
        assert_eq!(
            lines[1],
            "KEPT output=1 crtc=101 primary=- overlay=- cursor=- \
             dropped_reachable=16,17,18,19,20"
        );
    }
}
