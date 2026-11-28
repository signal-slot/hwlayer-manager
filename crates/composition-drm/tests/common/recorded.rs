//! Turns the text the `probe` example prints into [`Capabilities`].
//!
//! Recognised lines (leading whitespace is ignored):
//!
//! ```text
//! output 0: crtc 81 1920x1080 @ 60.000 Hz
//! plane 33 Primary: outputs 0b1, zpos 0..=7, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
//!   formats (32): XR24 AR24:0x8000000000000001 ...
//! PLANE index 2 id 79 Primary zpos fixed 0 plane-alpha true blend [none,premultiplied,coverage] scaling up 4.00 down 2.00 transforms 0x0 formats XR24 ...
//! ```
//!
//! The `PLANE` form is the twoplane example's. A `formats` line belongs to
//! the plane before it; `(identical to plane N)` copies the formats of the
//! plane with that id. Parsing stops at a line `raw`, where probe's `--raw`
//! dump starts: its lines reuse the words above with another meaning.
//!
//! Every other non-blank line is skipped and counted. A format token that is
//! not `FOURCC` or `FOURCC:0xMODIFIER` counts its line as skipped; the tokens
//! that did parse stay. The scaling word is per form. Probe form: `assumed` becomes
//! `Some(ScaleLimits::default())` (the text does not carry the limits),
//! `not claimed` becomes `None`. `PLANE` form: `none` becomes `None`;
//! `up U down D` becomes the limits in 16.16 (`round(U * 65536)`).
//! Doc-shortened lines such as `formats (same 32)` in docs/ are not
//! loadable: write fixtures out in full. Fields the text does not show (`max_source`, `full_output_only`,
//! and `transforms` in the probe form) stay at their defaults. The `PLANE`
//! form does not show the outputs mask either; it gets `0b1`.
//!
//! Callers read recorded files (`include_str!` of a file under `tests/data/`,
//! or text embedded in the test), never from `docs/`.

use composition_core::{
    BlendMode, Capabilities, FormatModifier, Modifier, OutputCaps, OutputId, PixelFormat,
    PlaneCaps, PlaneId, PlaneKind, ScaleLimits, Size, Zpos,
};

/// Broadcom modifier vendor, in the top byte of a modifier.
pub const VENDOR_BROADCOM: u8 = 0x07;
/// Arm modifier vendor.
pub const VENDOR_ARM: u8 = 0x08;

/// Modifier with the vendor in the top byte.
pub const fn vendor_modifier(vendor: u8, code: u64) -> u64 {
    ((vendor as u64) << 56) | code
}

/// Broadcom code 1.
pub const VC4_T_TILED: u64 = vendor_modifier(VENDOR_BROADCOM, 1);
/// Broadcom code 2.
pub const SAND32: u64 = vendor_modifier(VENDOR_BROADCOM, 2);
/// Broadcom code 3.
pub const SAND64: u64 = vendor_modifier(VENDOR_BROADCOM, 3);
/// Broadcom code 4.
pub const SAND128: u64 = vendor_modifier(VENDOR_BROADCOM, 4);
/// Broadcom code 5.
pub const SAND256: u64 = vendor_modifier(VENDOR_BROADCOM, 5);

/// Readable name of a modifier: `linear`, a known Broadcom code, or the
/// vendor and the low 56 bits.
pub fn modifier_name(modifier: u64) -> String {
    let vendor = (modifier >> 56) as u8;
    let code = modifier & ((1 << 56) - 1);
    match (vendor, code) {
        (0, 0) => "linear".into(),
        (VENDOR_BROADCOM, 1) => "VC4_T_TILED".into(),
        (VENDOR_BROADCOM, 2) => "SAND32".into(),
        (VENDOR_BROADCOM, 3) => "SAND64".into(),
        (VENDOR_BROADCOM, 4) => "SAND128".into(),
        (VENDOR_BROADCOM, 5) => "SAND256".into(),
        (VENDOR_ARM, _) => format!("arm {code:#x}"),
        (VENDOR_BROADCOM, _) => format!("broadcom {code:#x}"),
        _ => format!("vendor {vendor:#x} {code:#x}"),
    }
}

/// What the loader does when the text holds more than the core can keep.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    /// Fail with [`LoadError`].
    #[default]
    Error,
    /// Keep the first `MAX_*` items and drop the rest.
    Truncate,
}

/// The text holds more than `MAX_OUTPUTS`, `MAX_PLANES` or `MAX_FORMATS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum LoadError {
    /// More than `MAX_OUTPUTS` output lines.
    TooManyOutputs,
    /// More than `MAX_PLANES` plane lines.
    TooManyPlanes,
    /// More than `MAX_FORMATS` pairs on the plane with this id.
    TooManyFormats(u32),
}

/// Result of a successful load.
#[derive(Debug)]
pub struct Loaded {
    /// The capabilities, with no resource groups and `replace_pending` false.
    pub caps: Capabilities,
    /// Non-blank lines (and lines with unparseable format tokens) not used.
    pub skipped: usize,
}

/// Loads with [`Overflow::Error`].
pub fn load(text: &str) -> Result<Loaded, LoadError> {
    load_with(text, Overflow::Error)
}

/// Loads recorded probe text into capabilities.
pub fn load_with(text: &str, overflow: Overflow) -> Result<Loaded, LoadError> {
    let truncate = overflow == Overflow::Truncate;
    let mut caps = Capabilities::default();
    let mut skipped = 0;
    let mut current: Option<usize> = None;
    // The last plane line was dropped by truncation, so its formats are too.
    let mut dropped = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "raw" {
            break;
        }
        if let Some(output) = parse_output(line) {
            if caps.outputs.push(output).is_err() && !truncate {
                return Err(LoadError::TooManyOutputs);
            }
        } else if let Some((mut plane, formats)) = parse_plane(line) {
            if caps.planes.remaining() == 0 {
                current = None;
                dropped = true;
                if !truncate {
                    return Err(LoadError::TooManyPlanes);
                }
                continue;
            }
            if let Some(formats) = formats {
                skipped += usize::from(!formats.clean);
                fill_formats(&mut plane, &formats.pairs, truncate)?;
            }
            caps.planes.push(plane).unwrap();
            dropped = false;
            current = Some(caps.planes.len() - 1);
        } else if let Some(list) = line
            .strip_prefix("formats (")
            .and_then(|rest| rest.split_once("):"))
            .map(|(_, list)| list.trim())
        {
            let Some(index) = current else {
                skipped += usize::from(!dropped);
                continue;
            };
            if let Some(id) = list
                .strip_prefix("(identical to plane ")
                .and_then(|s| s.strip_suffix(')'))
                .and_then(|s| s.trim().parse::<u32>().ok())
            {
                match caps.planes.iter().find(|p| p.id.0 == id).map(|p| p.formats) {
                    Some(formats) => caps.planes.as_mut_slice()[index].formats = formats,
                    None => skipped += 1,
                }
                continue;
            }
            let parsed = parse_formats(list);
            skipped += usize::from(!parsed.clean);
            fill_formats(
                &mut caps.planes.as_mut_slice()[index],
                &parsed.pairs,
                truncate,
            )?;
        } else {
            skipped += 1;
        }
    }
    Ok(Loaded { caps, skipped })
}

fn fill_formats(
    plane: &mut PlaneCaps,
    pairs: &[FormatModifier],
    truncate: bool,
) -> Result<(), LoadError> {
    plane.formats.clear();
    for pair in pairs {
        if plane.formats.push(*pair).is_err() {
            if truncate {
                break;
            }
            return Err(LoadError::TooManyFormats(plane.id.0));
        }
    }
    Ok(())
}

struct Formats {
    pairs: Vec<FormatModifier>,
    clean: bool,
}

fn parse_formats(list: &str) -> Formats {
    let mut formats = Formats {
        pairs: Vec::new(),
        clean: true,
    };
    for token in list.split_whitespace() {
        // The raw dump says `linear` before the list of a plain plane.
        if token == "linear" {
            continue;
        }
        match parse_format(token) {
            Some(pair) => formats.pairs.push(pair),
            None => formats.clean = false,
        }
    }
    formats
}

fn parse_format(token: &str) -> Option<FormatModifier> {
    let (name, modifier) = match token.split_once(':') {
        Some((name, hex)) => (name, u64::from_str_radix(hex.strip_prefix("0x")?, 16).ok()?),
        None => (token, 0),
    };
    let code: [u8; 4] = name.as_bytes().try_into().ok()?;
    if !code.iter().all(|b| b.is_ascii_graphic()) {
        return None;
    }
    Some(FormatModifier {
        format: PixelFormat::fourcc(&code),
        modifier: Modifier(modifier),
    })
}

/// `output 0: crtc 81 1920x1080 @ 60.000 Hz`
fn parse_output(line: &str) -> Option<OutputCaps> {
    let rest = line.strip_prefix("output ")?;
    let (_, rest) = rest.split_once(": crtc ")?;
    let mut words = rest.split_whitespace();
    let id = words.next()?.parse().ok()?;
    let (width, height) = words.next()?.split_once('x')?;
    if words.next()? != "@" {
        return None;
    }
    let (whole, milli) = words.next()?.split_once('.')?;
    if milli.len() != 3 || words.next()? != "Hz" {
        return None;
    }
    Some(OutputCaps {
        id: OutputId(id),
        size: Size::new(width.parse().ok()?, height.parse().ok()?),
        refresh_mhz: whole.parse::<u32>().ok()? * 1000 + milli.parse::<u32>().ok()?,
    })
}

fn parse_kind(word: &str) -> Option<PlaneKind> {
    match word.trim_end_matches(':') {
        "Primary" => Some(PlaneKind::Primary),
        "Overlay" => Some(PlaneKind::Overlay),
        "Cursor" => Some(PlaneKind::Cursor),
        _ => None,
    }
}

fn parse_zpos(text: &str) -> Option<Zpos> {
    let text = text.trim();
    if let Some(z) = text.strip_prefix("fixed ") {
        return Some(Zpos::Fixed(z.trim().parse().ok()?));
    }
    let (min, max) = text.split_once("..=")?;
    Some(Zpos::Range {
        min: min.trim().parse().ok()?,
        max: max.trim().parse().ok()?,
    })
}

fn parse_blend(text: &str) -> Option<u8> {
    let inner = text.trim().strip_prefix('[')?.strip_suffix(']')?;
    let mut mask = 0;
    for name in inner.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        mask |= match name {
            "none" => BlendMode::Opaque.mask(),
            "premultiplied" => BlendMode::Premultiplied.mask(),
            "coverage" => BlendMode::Coverage.mask(),
            _ => return None,
        };
    }
    Some(mask)
}

/// Probe form: `not claimed` or `assumed`.
fn parse_probe_scaling(text: &str) -> Option<Option<ScaleLimits>> {
    match text.trim() {
        "not claimed" => Some(None),
        "assumed" => Some(Some(ScaleLimits::default())),
        _ => None,
    }
}

/// `PLANE` form: `none` or `up U down D` (ratios with two decimals).
fn parse_plane_scaling(text: &str) -> Option<Option<ScaleLimits>> {
    let text = text.trim();
    if text == "none" {
        return Some(None);
    }
    let rest = text.strip_prefix("up ")?;
    let (up, down) = rest.split_once(" down ")?;
    let fixed = |ratio: &str| -> Option<u32> {
        let ratio: f64 = ratio.trim().parse().ok()?;
        (ratio.is_finite() && ratio >= 0.0).then(|| (ratio * 65536.0).round() as u32)
    };
    Some(Some(ScaleLimits {
        max_upscale: fixed(up)?,
        max_downscale: fixed(down)?,
    }))
}

fn parse_mask(text: &str) -> Option<u32> {
    u32::from_str_radix(text.trim().strip_prefix("0b")?, 2).ok()
}

/// A plane line of either form; the `PLANE` form also carries its formats.
fn parse_plane(line: &str) -> Option<(PlaneCaps, Option<Formats>)> {
    if let Some(rest) = line.strip_prefix("plane ") {
        let (id, rest) = rest.split_once(' ')?;
        let (kind, rest) = rest.split_once(' ')?;
        let rest = rest.strip_prefix("outputs ")?;
        let (outputs, rest) = rest.split_once(", zpos ")?;
        let (zpos, rest) = rest.split_once(", plane alpha ")?;
        let (alpha, rest) = rest.split_once(", blend ")?;
        let (blend, scaling) = rest.split_once(", scaling ")?;
        let plane = PlaneCaps {
            id: PlaneId(id.parse().ok()?),
            kind: parse_kind(kind)?,
            outputs: parse_mask(outputs)?,
            zpos: parse_zpos(zpos)?,
            plane_alpha: alpha.trim().parse().ok()?,
            blend_modes: parse_blend(blend)?,
            scaling: parse_probe_scaling(scaling)?,
            ..PlaneCaps::default()
        };
        return Some((plane, None));
    }
    // PLANE index I id N Kind zpos Z plane-alpha B blend [a,b] scaling S transforms 0xT formats ...
    let rest = line.strip_prefix("PLANE index ")?;
    let (_, rest) = rest.split_once(" id ")?;
    let (id, rest) = rest.split_once(' ')?;
    let (kind, rest) = rest.split_once(" zpos ")?;
    let (zpos, rest) = rest.split_once(" plane-alpha ")?;
    let (alpha, rest) = rest.split_once(" blend ")?;
    let (blend, rest) = rest.split_once(" scaling ")?;
    let (scaling, rest) = rest.split_once(" transforms ")?;
    let (transforms, formats) = rest.split_once(" formats ")?;
    let plane = PlaneCaps {
        id: PlaneId(id.parse().ok()?),
        kind: parse_kind(kind)?,
        outputs: 1,
        zpos: parse_zpos(zpos)?,
        plane_alpha: alpha.trim().parse().ok()?,
        blend_modes: parse_blend(blend)?,
        scaling: parse_plane_scaling(scaling)?,
        transforms: u8::from_str_radix(transforms.trim().strip_prefix("0x")?, 16).ok()?,
        ..PlaneCaps::default()
    };
    Some((plane, Some(parse_formats(formats))))
}
