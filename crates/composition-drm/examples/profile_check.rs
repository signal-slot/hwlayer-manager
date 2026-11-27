//! Planner versus `TEST_ONLY`: how far a driver profile agrees with the
//! kernel, on a fixed matrix of frames built from the probed capabilities.
//!
//! ```text
//! cargo run -p composition-drm --example profile_check -- /dev/dri/cardN \
//!     --profile generic|imx95|stm32|rk3588|hvs5|hvs6 [--output N] [--show SECONDS]
//!
//!   --profile NAME     driver profile to add to what KMS reports (required)
//!   --output N         index of the output in the probed capabilities (default 0)
//!   --show SECONDS     afterwards, show the two-plane picture of twoplane for
//!                      that long and print its EXPECT lines
//! ```
//!
//! The capabilities are discovered with the chosen profile and printed the
//! way `probe` prints them, with the scaling limits, the largest source and
//! the resource groups the profile added.
//!
//! # The matrix
//!
//! Built from the capabilities of the chosen output, by [`build_matrix`]:
//!
//! - `layers-K` for K = 1 up to the number of planes on the output: a
//!   full-screen RGB background and K - 1 small RGB tiles above it;
//! - `scale-F` for F = 0.5, 0.75, 1.5, 2, 4 and 8: one RGB layer whose
//!   destination is F times its source on each axis, over the background;
//! - `nv12`: an unscaled linear NV12 layer over the background;
//! - `modifier-<fourcc>-<modifier>`: for each non-linear modifier a plane on
//!   the output lists, a layer of the first listed format with that modifier
//!   that has a known buffer layout;
//! - `rotate-90` and `reflect-y`: a rotated and a mirrored layer;
//! - `wide-source`: a 4352-pixel-wide source, scaled to the output width;
//! - `below-primary`: a small layer stacked below the full-screen layer, which
//!   a primary plane with a fixed zpos cannot carry on top of it.
//!
//! # Each frame
//!
//! The planner plans the frame against the capabilities. Whatever it says,
//! the backend check (`TEST_ONLY`) is asked too: when the planner accepts,
//! its candidates are offered to the check until one passes, as the engine
//! does; when it refuses, the frame is planned again against capabilities
//! that claim what could be missing (the frame's formats, scaling by up to
//! 16, no source limit, no resource groups) on every non-cursor plane of the
//! output, as `twoplane --unsupported` does, and that plan goes to the check.
//! Either way the `TEST_ONLY` result is the kernel's own answer. Rotation is
//! not programmed by composition-drm, so the rotated frames never reach the
//! kernel; neither do frames whose buffer cannot be made (dumb buffers carry
//! the other layouts through a DMA-BUF export, which ADDFB2 may refuse).
//!
//! ```text
//! CHECK <name> planner=<plan:planes|refuse:reasons> test_only=<ok|errno|not reached|error> agree=<yes|no|planner-stricter|planner-looser> via=<plan|forced|none> candidates=N [note=...]
//! AGREEMENT total=N agree=A stricter=S looser=L
//! ```
//!
//! `agree=yes` means both accept or both refuse; `planner-stricter` that the
//! planner refuses what the kernel accepts; `planner-looser` that the planner
//! accepts what the kernel refuses, the case that matters; `no` that no
//! comparison was possible (the kernel was not reached, or failed in a way
//! that is not a refusal). `scripts/sweep-summary.py` turns these lines into
//! tables.
//!
//! # The picture
//!
//! With `--show`, the two-plane picture of `twoplane` is committed once and
//! left on screen for the given time: a dark background with a red, a green
//! and a blue block on the primary plane, and an overlay window, opaque
//! yellow above and translucent magenta below, covering the right half of
//! the green block. `EXPECT phase=normal` lines in twoplane's format give
//! sample points, so `scripts/twoplane-expect.py` turns them into
//! camera-check JSON.
//!
//! # Ending
//!
//! The last line is `VERDICT profile_check: PASS|FAIL <reason>`. It fails on
//! a planner-looser frame, or when the device fails during the run. The exit
//! status is 0 for a pass, 1 for a failure and 2 for a usage error or a
//! device that cannot be used.
//!
//! The example needs DRM master. If another process (a desktop, a
//! compositor) holds it, the example says so and stops; it never stops other
//! processes itself.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::os::fd::AsFd;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use composition_core::planner::{plan, plan_with, BufferLookup};
use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, Capabilities, DisplayBackend, DisplayEvent,
    Engine, FormatModifier, Frame, Layer, Modifier, OutputId, PixelFormat, Plan, PlanError,
    PlaneKind, Rect, ScaleLimits, SearchLimits, Size, SubmissionId, Transform, Verdict, Zpos,
    ALPHA_OPAQUE,
};
use composition_drm::{DmabufPlane, DriverProfile, DrmBackend, DrmFailure, DrmOptions};
use drm::buffer::{Buffer as _, DrmFourcc};
use drm::control::Device as _;
use drm::Device as _;

const USAGE: &str = "usage: profile_check /dev/dri/cardN --profile \
generic|imx95|stm32|rk3588|hvs5|hvs6 [--output N] [--show SECONDS]";

/// How long to wait for the picture, or the blank frame, to latch.
const LATCH_TIMEOUT: Duration = Duration::from_secs(1);

/// Scale factors of the `scale-*` frames, as numerator and denominator.
const SCALES: [(u32, u32); 6] = [(1, 2), (3, 4), (3, 2), (2, 1), (4, 1), (8, 1)];

/// Width of the `wide-source` buffer: wider than 4096.
const WIDE_WIDTH: u32 = 4352;
const WIDE_HEIGHT: u32 = 64;

/// Scaling the forced capabilities claim, up and down.
const FORCED_SCALE: u32 = 16 << 16;
const FORCED_NOTE: &str =
    "forced caps claim the formats, scaling by 16, no source limit and no resource groups";
const ROTATION_NOTE: &str = "composition-drm does not program the rotation property";

/// Buffer identifiers of the matrix. Modifier buffers count up from
/// `MODIFIER_IDS`; the picture of `--show` uses its own.
const BACKGROUND_ID: BufferId = BufferId(1);
const TILE_ID: BufferId = BufferId(2);
const SCALE_ID: BufferId = BufferId(3);
const NV12_ID: BufferId = BufferId(4);
const WIDE_ID: BufferId = BufferId(5);
const ROTATE_ID: BufferId = BufferId(6);
const MODIFIER_IDS: u32 = 100;
const SHOW_PRIMARY_ID: BufferId = BufferId(1000);
const SHOW_OVERLAY_ID: BufferId = BufferId(1001);

#[derive(Debug, PartialEq, Eq)]
struct Options {
    device: String,
    profile: DriverProfile,
    output: usize,
    show: Option<u32>,
}

fn parse_profile(name: &str) -> Result<DriverProfile, String> {
    Ok(match name {
        "generic" => DriverProfile::Generic,
        "imx95" => DriverProfile::Imx95Dpu,
        "stm32" => DriverProfile::Stm32Ltdc,
        "rk3588" => DriverProfile::Rk3588Vop2,
        "hvs5" => DriverProfile::Vc4Hvs5,
        "hvs6" => DriverProfile::Vc4Hvs6,
        other => return Err(format!("unknown profile {other}")),
    })
}

/// Parses the arguments after the program name. An empty error asks for
/// the usage text only.
fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
    let mut args = args.into_iter();
    let mut device = None;
    let mut profile = None;
    let mut output = 0;
    let mut show = None;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--profile" => profile = Some(parse_profile(&value("--profile")?)?),
            "--output" => output = number(&value("--output")?)?,
            "--show" => show = Some(number(&value("--show")?)?),
            "-h" | "--help" => return Err(String::new()),
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other if device.is_none() => device = Some(other.to_string()),
            other => return Err(format!("unexpected argument {other}")),
        }
    }
    Ok(Options {
        device: device.ok_or("no device given")?,
        profile: profile.ok_or("--profile is required")?,
        output,
        show,
    })
}

fn number<T: std::str::FromStr>(text: &str) -> Result<T, String> {
    text.parse().map_err(|_| format!("bad number {text}"))
}

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

fn errno_name(errno: i32) -> String {
    let name = match errno {
        1 => "EPERM",
        2 => "ENOENT",
        5 => "EIO",
        12 => "ENOMEM",
        13 => "EACCES",
        16 => "EBUSY",
        19 => "ENODEV",
        22 => "EINVAL",
        28 => "ENOSPC",
        34 => "ERANGE",
        95 => "EOPNOTSUPP",
        _ => return format!("errno{errno}"),
    };
    name.into()
}

fn io_text(error: &std::io::Error) -> String {
    match error.raw_os_error() {
        Some(errno) => errno_name(errno),
        None => error.to_string(),
    }
}

/// Current UTC time as ISO 8601, without a date crate.
fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Days to civil date, after Howard Hinnant's `civil_from_days`.
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let rem = secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

fn factor(value: u32) -> String {
    format!("{:.2}", value as f64 / 65536.0)
}

/// The capabilities as `probe` prints them, with what a profile adds.
fn caps_lines(caps: &Capabilities) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, output) in caps.outputs.iter().enumerate() {
        lines.push(format!(
            "output {index}: crtc {} {}x{} @ {}.{:03} Hz",
            output.id.0,
            output.size.width,
            output.size.height,
            output.refresh_mhz / 1000,
            output.refresh_mhz % 1000
        ));
    }
    for plane in caps.planes.iter() {
        let zpos = match plane.zpos {
            Zpos::Fixed(z) => format!("fixed {z}"),
            Zpos::Range { min, max } => format!("{min}..={max}"),
        };
        let blend: Vec<&str> = [
            (BlendMode::Opaque, "none"),
            (BlendMode::Premultiplied, "premultiplied"),
            (BlendMode::Coverage, "coverage"),
        ]
        .iter()
        .filter(|(mode, _)| plane.blend_modes & mode.mask() != 0)
        .map(|(_, name)| *name)
        .collect();
        lines.push(format!(
            "plane {} {:?}: outputs {:#b}, zpos {zpos}, plane alpha {}, blend [{}], scaling {}, max source {}, transforms {:#x}",
            plane.id.0,
            plane.kind,
            plane.outputs,
            plane.plane_alpha,
            blend.join(", "),
            plane.scaling.map_or("not claimed".into(), |s| format!(
                "up {} down {}",
                factor(s.max_upscale),
                factor(s.max_downscale)
            )),
            plane
                .max_source
                .map_or("none".into(), |s| format!("{}x{}", s.width, s.height)),
            plane.transforms,
        ));
        let formats: Vec<String> = plane
            .formats
            .iter()
            .map(|f| {
                if f.modifier == Modifier::LINEAR {
                    fourcc(f.format)
                } else {
                    format!("{}:{:#x}", fourcc(f.format), f.modifier.0)
                }
            })
            .collect();
        lines.push(format!(
            "  formats ({}): {}",
            formats.len(),
            formats.join(" ")
        ));
    }
    for group in caps.resource_groups.iter() {
        lines.push(format!(
            "resource group: consumers {:#x}, planes {:#b}, capacity {}",
            group.consumers.0, group.planes, group.capacity
        ));
    }
    lines
}

/// A buffer a frame of the matrix needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BufferSpec {
    id: BufferId,
    info: BufferInfo,
}

/// One frame of the matrix.
#[derive(Clone, Debug)]
struct Case {
    name: String,
    layers: Vec<Layer>,
    buffers: Vec<BufferSpec>,
    /// Why the kernel cannot be asked about this frame, whatever the planner
    /// says.
    unreachable: Option<String>,
}

/// Buffer metadata for the planner.
struct Lookup<'a>(&'a [BufferSpec]);

impl BufferLookup for Lookup<'_> {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|b| b.id == buffer).map(|b| b.info)
    }
}

/// Bytes, pixels per block and subsampling of one plane of a format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FormatPlane {
    bytes: u32,
    block_w: u32,
    hsub: u32,
    vsub: u32,
}

const fn packed(bytes: u32) -> FormatPlane {
    FormatPlane {
        bytes,
        block_w: 1,
        hsub: 1,
        vsub: 1,
    }
}

const fn sub(bytes: u32, block_w: u32, hsub: u32, vsub: u32) -> FormatPlane {
    FormatPlane {
        bytes,
        block_w,
        hsub,
        vsub,
    }
}

/// Plane layouts of the formats a buffer can be built for, after
/// `drm_format_info` in `drivers/gpu/drm/drm_fourcc.c`, as in `kms_sweep`.
fn format_planes(format: PixelFormat) -> Option<Vec<FormatPlane>> {
    let planes = match fourcc(format).as_str() {
        "C8" | "R8" => vec![packed(1)],
        "R16" | "RG88" | "GR88" | "RG16" | "BG16" | "XR15" | "AR15" | "XB15" | "AB15" | "RX15"
        | "RA15" | "BX15" | "BA15" | "XR12" | "AR12" | "XB12" | "AB12" | "RX12" | "RA12"
        | "BX12" | "BA12" | "YUYV" | "YVYU" | "UYVY" | "VYUY" => vec![packed(2)],
        "RG24" | "BG24" | "VU24" => vec![packed(3)],
        "XR24" | "AR24" | "XB24" | "AB24" | "RX24" | "RA24" | "BX24" | "BA24" | "XR30" | "AR30"
        | "XB30" | "AB30" | "RX30" | "RA30" | "BX30" | "BA30" | "AYUV" | "XYUV" | "Y410"
        | "XV30" | "Y210" | "Y212" | "Y216" => vec![packed(4)],
        "XR4H" | "AR4H" | "XB4H" | "AB4H" | "XR48" | "AR48" | "XB48" | "AB48" | "Y412" | "Y416"
        | "XV48" => vec![packed(8)],
        "NV12" | "NV21" => vec![packed(1), sub(2, 1, 2, 2)],
        "NV16" | "NV61" => vec![packed(1), sub(2, 1, 2, 1)],
        "NV24" | "NV42" => vec![packed(1), sub(2, 1, 1, 1)],
        "NV15" => vec![sub(5, 4, 1, 1), sub(5, 2, 2, 2)],
        "P010" | "P012" | "P016" => vec![packed(2), sub(4, 1, 2, 2)],
        "P030" => vec![sub(4, 3, 1, 1), sub(8, 3, 2, 2)],
        "YU12" | "YV12" => vec![packed(1), sub(1, 1, 2, 2), sub(1, 1, 2, 2)],
        _ => return None,
    };
    Some(planes)
}

/// Pitches, offsets and total size of a buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
struct FbLayout {
    count: usize,
    pitches: [u32; 4],
    offsets: [u32; 4],
    /// Bytes to allocate, with room for compression headers.
    alloc: u64,
}

fn round_up(value: u64, to: u64) -> u64 {
    value.div_ceil(to) * to
}

/// The width is padded to 16 pixels and each pitch to 64 bytes, every plane
/// starts at a 4 KiB boundary and the rows are padded to 16, and a quarter
/// more plus 256 KiB holds compression headers, as in `kms_sweep`.
fn fb_layout(planes: &[FormatPlane], size: Size) -> Option<FbLayout> {
    if planes.is_empty() || planes.len() > 4 {
        return None;
    }
    let width = round_up(u64::from(size.width), 16);
    let height = round_up(u64::from(size.height), 16);
    let mut layout = FbLayout {
        count: planes.len(),
        pitches: [0; 4],
        offsets: [0; 4],
        alloc: 0,
    };
    let mut offset = 0u64;
    for (i, p) in planes.iter().enumerate() {
        let plane_w = width.div_ceil(u64::from(p.hsub));
        let plane_h = height.div_ceil(u64::from(p.vsub));
        let pitch = round_up(
            plane_w.div_ceil(u64::from(p.block_w)) * u64::from(p.bytes),
            64,
        );
        layout.pitches[i] = u32::try_from(pitch).ok()?;
        layout.offsets[i] = u32::try_from(offset).ok()?;
        offset = round_up(offset + pitch * plane_h, 4096);
    }
    layout.alloc = offset + offset / 4 + (256 << 10);
    Some(layout)
}

/// Formats `DrmBackend::create_dumb_buffer` takes directly.
fn plain_dumb(info: &BufferInfo) -> bool {
    info.modifier == Modifier::LINEAR
        && [
            PixelFormat::RGB565,
            PixelFormat::XRGB8888,
            PixelFormat::ARGB8888,
            PixelFormat::XBGR8888,
            PixelFormat::ABGR8888,
        ]
        .contains(&info.format)
}

fn on_output(
    caps: &Capabilities,
    output_index: usize,
) -> impl Iterator<Item = &composition_core::PlaneCaps> {
    caps.planes
        .iter()
        .filter(move |p| p.outputs & (1 << output_index) != 0)
}

fn lists_linear(caps: &Capabilities, output_index: usize, format: PixelFormat) -> bool {
    on_output(caps, output_index).any(|p| {
        p.formats
            .iter()
            .any(|f| f.format == format && f.modifier == Modifier::LINEAR)
    })
}

/// The 32-bit RGB format of the matrix: the first one a plane on the output
/// lists linear, XRGB8888 if none does.
fn rgb_format(caps: &Capabilities, output_index: usize) -> PixelFormat {
    [
        PixelFormat::XRGB8888,
        PixelFormat::ARGB8888,
        PixelFormat::XBGR8888,
        PixelFormat::ABGR8888,
    ]
    .into_iter()
    .find(|f| lists_linear(caps, output_index, *f))
    .unwrap_or(PixelFormat::XRGB8888)
}

/// Non-linear modifiers listed on the output, in order of first appearance,
/// each with the format to use: the first listed with it that has a buffer
/// layout, else the first listed with it.
fn listed_modifiers(caps: &Capabilities, output_index: usize) -> Vec<FormatModifier> {
    let mut modifiers: Vec<Modifier> = Vec::new();
    for plane in on_output(caps, output_index) {
        for pair in plane.formats.iter() {
            // DRM_FORMAT_MOD_INVALID is no layout.
            if pair.modifier != Modifier::LINEAR
                && pair.modifier.0 != 0x00ff_ffff_ffff_ffff
                && !modifiers.contains(&pair.modifier)
            {
                modifiers.push(pair.modifier);
            }
        }
    }
    modifiers
        .into_iter()
        .map(|modifier| {
            let formats: Vec<PixelFormat> = on_output(caps, output_index)
                .flat_map(|p| p.formats.iter().copied())
                .filter(|f| f.modifier == modifier)
                .map(|f| f.format)
                .collect();
            let format = formats
                .iter()
                .copied()
                .find(|f| format_planes(*f).is_some())
                .unwrap_or(formats[0]);
            FormatModifier { format, modifier }
        })
        .collect()
}

/// Builds the matrix for output `output_index` of `caps`. Pure: nothing is
/// allocated or asked.
fn build_matrix(caps: &Capabilities, output_index: usize) -> Vec<Case> {
    let Some(output) = caps.outputs.get(output_index) else {
        return Vec::new();
    };
    let (w, h) = (output.size.width.max(1), output.size.height.max(1));
    let full = Rect::new(0, 0, w, h);
    let rgb = rgb_format(caps, output_index);
    let spec = |id: BufferId, width: u32, height: u32, format: PixelFormat, modifier: Modifier| {
        BufferSpec {
            id,
            info: BufferInfo {
                size: Size::new(width, height),
                format,
                modifier,
            },
        }
    };
    let background = spec(BACKGROUND_ID, w, h, rgb, Modifier::LINEAR);
    let bg = Layer::new(BACKGROUND_ID, full, full, 0);
    // A multiple of 24, so that every scale factor gives whole pixels.
    let d = ((w.min(h) / 2) / 24 * 24).max(24);
    let at = Rect::new((w / 8) as i32, (h / 8) as i32, d, d);
    let tile_size = (w.min(h) / 10).max(16);
    let tile = spec(TILE_ID, tile_size, tile_size, rgb, Modifier::LINEAR);
    let mut cases = Vec::new();
    let case = |name: String, layers: Vec<Layer>, buffers: Vec<BufferSpec>| Case {
        name,
        layers,
        buffers,
        unreachable: None,
    };

    let planes = on_output(caps, output_index).count();
    let pitch = tile_size + tile_size / 2;
    let columns = (w / pitch).max(1);
    for count in 1..=planes.max(1) {
        let mut layers = vec![bg];
        for k in 1..count as u32 {
            let x = (tile_size / 2 + (k - 1) % columns * pitch) as i32;
            let y =
                (tile_size / 2 + (k - 1) / columns * pitch).min(h.saturating_sub(tile_size)) as i32;
            layers.push(Layer::new(
                TILE_ID,
                Rect::new(0, 0, tile_size, tile_size),
                Rect::new(x, y, tile_size, tile_size),
                k as i32,
            ));
        }
        let buffers = if count > 1 {
            vec![background, tile]
        } else {
            vec![background]
        };
        cases.push(case(format!("layers-{count}"), layers, buffers));
    }

    let scale_buffer = spec(SCALE_ID, 2 * d, 2 * d, rgb, Modifier::LINEAR);
    for (num, den) in SCALES {
        let source = d * den / num;
        let name = if den == 1 {
            format!("scale-{num}")
        } else {
            format!("scale-{}", num as f64 / den as f64)
        };
        cases.push(case(
            name,
            vec![
                bg,
                Layer::new(SCALE_ID, Rect::new(0, 0, source, source), at, 1),
            ],
            vec![background, scale_buffer],
        ));
    }

    cases.push(case(
        "nv12".into(),
        vec![bg, Layer::new(NV12_ID, Rect::new(0, 0, d, d), at, 1)],
        vec![
            background,
            spec(NV12_ID, d, d, PixelFormat::NV12, Modifier::LINEAR),
        ],
    ));

    for (k, pair) in listed_modifiers(caps, output_index).into_iter().enumerate() {
        let id = BufferId(MODIFIER_IDS + k as u32);
        let mut entry = case(
            format!("modifier-{}-{:#018x}", fourcc(pair.format), pair.modifier.0),
            vec![bg, Layer::new(id, Rect::new(0, 0, d, d), at, 1)],
            vec![background, spec(id, d, d, pair.format, pair.modifier)],
        );
        if format_planes(pair.format).is_none() {
            entry.unreachable = Some(format!("no buffer layout for {}", fourcc(pair.format)));
        }
        cases.push(entry);
    }

    let rotate = spec(ROTATE_ID, d, d / 2, rgb, Modifier::LINEAR);
    for (name, transform, destination) in [
        (
            "rotate-90",
            Transform::Rotate90,
            Rect::new(at.x, at.y, d / 2, d),
        ),
        (
            "reflect-y",
            Transform::FlipV,
            Rect::new(at.x, at.y, d, d / 2),
        ),
    ] {
        let mut entry = case(
            name.into(),
            vec![
                bg,
                Layer {
                    transform,
                    ..Layer::new(ROTATE_ID, Rect::new(0, 0, d, d / 2), destination, 1)
                },
            ],
            vec![background, rotate],
        );
        entry.unreachable = Some(ROTATION_NOTE.into());
        cases.push(entry);
    }

    let wide_w = w.min(WIDE_WIDTH);
    let wide_h = (WIDE_HEIGHT * wide_w / WIDE_WIDTH).max(1);
    cases.push(case(
        "wide-source".into(),
        vec![
            bg,
            Layer::new(
                WIDE_ID,
                Rect::new(0, 0, WIDE_WIDTH, WIDE_HEIGHT),
                Rect::new(0, (h / 2) as i32, wide_w, wide_h),
                1,
            ),
        ],
        vec![
            background,
            spec(WIDE_ID, WIDE_WIDTH, WIDE_HEIGHT, rgb, Modifier::LINEAR),
        ],
    ));

    cases.push(case(
        "below-primary".into(),
        vec![
            Layer::new(
                TILE_ID,
                Rect::new(0, 0, tile_size, tile_size),
                Rect::new(at.x, at.y, tile_size, tile_size),
                0,
            ),
            Layer::new(BACKGROUND_ID, full, full, 1),
        ],
        vec![background, tile],
    ));
    cases
}

/// The capabilities with every non-cursor plane of the output claiming the
/// frame's formats, scaling by up to 16 and no source limit, and with no
/// resource groups. Zpos and transforms stay: the backend could not program
/// a claimed one.
fn relax(caps: &Capabilities, output_index: usize, buffers: &[BufferSpec]) -> Box<Capabilities> {
    let mut forced = Box::new(*caps);
    for plane in forced.planes.as_mut_slice() {
        if plane.outputs & (1 << output_index) == 0 || plane.kind == PlaneKind::Cursor {
            continue;
        }
        plane.scaling = Some(ScaleLimits {
            max_upscale: FORCED_SCALE,
            max_downscale: FORCED_SCALE,
        });
        plane.max_source = None;
        for buffer in buffers {
            let pair = FormatModifier {
                format: buffer.info.format,
                modifier: buffer.info.modifier,
            };
            if plane.formats.contains(&pair) {
                continue;
            }
            if plane.formats.push(pair).is_err() {
                let last = plane.formats.len() - 1;
                plane.formats.as_mut_slice()[last] = pair;
            }
        }
    }
    forced.resource_groups.clear();
    forced
}

/// What the planner said.
type PlannerVerdict = Result<Vec<u32>, String>;

fn planner_text(verdict: &PlannerVerdict) -> String {
    match verdict {
        Ok(planes) => format!(
            "plan:{}",
            planes
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
        Err(reasons) => format!("refuse:{reasons}"),
    }
}

fn refuse_text(error: &PlanError) -> String {
    match error {
        PlanError::NoPlane { layer, reasons } => {
            format!(
                "layer{layer}:{}",
                format!("{reasons:?}").replace(" | ", "+")
            )
        }
        other => format!("{other:?}").replace(' ', ""),
    }
}

fn planner_verdict(result: &Result<Plan, PlanError>) -> PlannerVerdict {
    match result {
        Ok(plan) => Ok(plan.assignments().iter().map(|a| a.plane.0).collect()),
        Err(error) => Err(refuse_text(error)),
    }
}

/// What the kernel said.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Kernel {
    /// `TEST_ONLY` passed.
    Ok,
    /// `TEST_ONLY` refused with this errno (0 if not known).
    Refused(i32),
    /// The kernel was not asked; why.
    NotReached(String),
    /// The device failed, not as a refusal; how.
    Error(String),
}

impl Kernel {
    fn text(&self) -> String {
        match self {
            Kernel::Ok => "ok".into(),
            Kernel::Refused(0) => "rejected".into(),
            Kernel::Refused(errno) => errno_name(*errno),
            Kernel::NotReached(_) => "not reached".into(),
            Kernel::Error(_) => "error".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Agree {
    Yes,
    No,
    Stricter,
    Looser,
}

impl Agree {
    fn text(self) -> &'static str {
        match self {
            Agree::Yes => "yes",
            Agree::No => "no",
            Agree::Stricter => "planner-stricter",
            Agree::Looser => "planner-looser",
        }
    }
}

fn classify(planner_accepts: bool, kernel: &Kernel) -> Agree {
    match (planner_accepts, kernel) {
        (true, Kernel::Ok) | (false, Kernel::Refused(_)) => Agree::Yes,
        (false, Kernel::Ok) => Agree::Stricter,
        (true, Kernel::Refused(_)) => Agree::Looser,
        (_, Kernel::NotReached(_) | Kernel::Error(_)) => Agree::No,
    }
}

/// How the kernel was asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Via {
    /// With the planner's own candidates.
    Plan,
    /// With a plan against the forced capabilities.
    Forced,
    /// Not at all.
    None,
}

impl Via {
    fn text(self) -> &'static str {
        match self {
            Via::Plan => "plan",
            Via::Forced => "forced",
            Via::None => "none",
        }
    }
}

/// The result of one frame.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Outcome {
    planner: PlannerVerdict,
    kernel: Kernel,
    via: Via,
    candidates: u32,
}

impl Outcome {
    fn agree(&self) -> Agree {
        classify(self.planner.is_ok(), &self.kernel)
    }

    fn note(&self) -> Option<String> {
        match (&self.kernel, self.via) {
            (Kernel::NotReached(why) | Kernel::Error(why), _) => Some(why.clone()),
            (_, Via::Forced) => Some(FORCED_NOTE.into()),
            _ => None,
        }
    }
}

fn check_line(name: &str, outcome: &Outcome) -> String {
    let mut line = format!(
        "CHECK {name} planner={} test_only={} agree={} via={} candidates={}",
        planner_text(&outcome.planner),
        outcome.kernel.text(),
        outcome.agree().text(),
        outcome.via.text(),
        outcome.candidates
    );
    if let Some(note) = outcome.note() {
        // Keep the line parseable as key=value tokens.
        line.push_str(&format!(" note={}", note.replace('=', ":")));
    }
    line
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Tally {
    total: usize,
    agree: usize,
    stricter: usize,
    looser: usize,
}

fn tally(agreements: &[Agree]) -> Tally {
    let count = |a: Agree| agreements.iter().filter(|x| **x == a).count();
    Tally {
        total: agreements.len(),
        agree: count(Agree::Yes),
        stricter: count(Agree::Stricter),
        looser: count(Agree::Looser),
    }
}

fn agreement_line(t: &Tally) -> String {
    format!(
        "AGREEMENT total={} agree={} stricter={} looser={}",
        t.total, t.agree, t.stricter, t.looser
    )
}

/// Offers the candidates of `frame` against `caps` to the backend check
/// until one passes. Returns what the kernel said and how many candidates
/// were checked, or the planner's error if it found none.
fn ask_kernel(
    backend: &mut DrmBackend,
    caps: &Capabilities,
    lookup: &Lookup<'_>,
    frame: &Frame<'_>,
) -> Result<(Kernel, u32), PlanError> {
    let mut checked = 0;
    let mut errno = 0;
    let mut failure = None;
    let _ = backend.take_failures().count();
    let planned = plan_with(frame, caps, lookup, SearchLimits::default(), |plan| {
        checked += 1;
        match backend.check(frame, plan) {
            Ok(()) => Verdict::Accept,
            Err(BackendError::Rejected) => {
                errno = match backend.take_failures().next_back() {
                    Some(DrmFailure::Refused { errno, .. }) => errno,
                    Some(DrmFailure::NeedsModeset { errno }) => errno,
                    _ => 0,
                };
                Verdict::TryNext
            }
            Err(error) => {
                failure = Some(error);
                Verdict::GiveUp
            }
        }
    });
    match planned {
        Ok(_) => Ok((Kernel::Ok, checked)),
        Err(PlanError::BackendRejected { .. }) => Ok((Kernel::Refused(errno), checked)),
        Err(PlanError::LimitExceeded) if checked > 0 => Ok((Kernel::Refused(errno), checked)),
        Err(PlanError::Aborted) => {
            let why = match failure {
                Some(BackendError::Platform(errno)) => {
                    format!("check failed with {}", errno_name(errno))
                }
                Some(other) => format!("check failed: {other:?}"),
                None => "check aborted".into(),
            };
            Ok((Kernel::Error(why), checked))
        }
        Err(error) => Err(error),
    }
}

/// Plans one frame and asks the kernel. `allocated` says, per buffer, whether
/// it could be made on the device.
fn run_case(
    backend: &mut DrmBackend,
    caps: &Capabilities,
    output_index: usize,
    output: OutputId,
    case: &Case,
    allocated: &HashMap<BufferId, Result<(), String>>,
) -> Outcome {
    let lookup = Lookup(&case.buffers);
    let frame = Frame {
        output,
        layers: &case.layers,
    };
    let planner = planner_verdict(&plan(&frame, caps, &lookup));
    let not_reached = |why: String| Outcome {
        planner: planner.clone(),
        kernel: Kernel::NotReached(why),
        via: Via::None,
        candidates: 0,
    };
    if let Some(why) = &case.unreachable {
        return not_reached(why.clone());
    }
    for buffer in &case.buffers {
        if let Some(Err(why)) = allocated.get(&buffer.id) {
            return not_reached(why.clone());
        }
    }
    let (forced, via) = if planner.is_ok() {
        (Box::new(*caps), Via::Plan)
    } else {
        (relax(caps, output_index, &case.buffers), Via::Forced)
    };
    match ask_kernel(backend, &forced, &lookup, &frame) {
        Ok((kernel, candidates)) => Outcome {
            planner,
            kernel,
            via,
            candidates,
        },
        Err(error) => not_reached(format!(
            "the forced caps are still refused: {}",
            refuse_text(&error)
        )),
    }
}

/// Makes `spec` on the device: a dumb buffer when the backend takes the
/// format directly, else a dumb buffer exported as a DMA-BUF and imported
/// with the format's layout and modifier. The contents are zero.
fn allocate(backend: &mut DrmBackend, spec: &BufferSpec) -> Result<(), String> {
    let info = spec.info;
    let name = format!("{}:{:#x}", fourcc(info.format), info.modifier.0);
    if plain_dumb(&info) {
        return backend
            .create_dumb_buffer(spec.id, info)
            .map_err(|e| format!("dumb buffer {name} refused: {}", io_text(&e)));
    }
    let layout = format_planes(info.format)
        .and_then(|planes| fb_layout(&planes, info.size))
        .ok_or(format!("no buffer layout for {name}"))?;
    let widest = layout.pitches.iter().copied().max().unwrap_or(0).max(4);
    let rows = u32::try_from(layout.alloc.div_ceil(u64::from(widest)))
        .map_err(|_| format!("buffer {name} too large"))?;
    let card = backend.card();
    let dumb = card
        .create_dumb_buffer((widest / 4, rows), DrmFourcc::Xrgb8888, 32)
        .map_err(|e| format!("dumb buffer for {name} refused: {}", io_text(&e)))?;
    let flags = (rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::RDWR).bits();
    let fd = match card.buffer_to_prime_fd(dumb.handle(), flags) {
        Ok(fd) => fd,
        Err(error) => {
            let _ = card.destroy_dumb_buffer(dumb);
            return Err(format!("export of {name} refused: {}", io_text(&error)));
        }
    };
    let planes: Vec<DmabufPlane<'_>> = (0..layout.count)
        .map(|i| DmabufPlane {
            fd: fd.as_fd(),
            offset: layout.offsets[i],
            pitch: layout.pitches[i],
        })
        .collect();
    let imported = backend.import_dmabuf(spec.id, info, &planes);
    // The import closed the GEM handle if it got that far (the same handle as
    // the dumb buffer's); closing it again only fails. The framebuffer keeps
    // the memory alive.
    let _ = backend.card().destroy_dumb_buffer(dumb);
    imported.map_err(|e| format!("ADDFB2 of {name} refused: {}", io_text(&e)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rgb(u8, u8, u8);

impl Rgb {
    fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

const BACKGROUND: Rgb = Rgb(0x20, 0x20, 0x20);
const RED: Rgb = Rgb(0xff, 0x00, 0x00);
const GREEN: Rgb = Rgb(0x00, 0xff, 0x00);
const BLUE: Rgb = Rgb(0x00, 0x00, 0xff);
const YELLOW: Rgb = Rgb(0xff, 0xff, 0x00);
const MAGENTA: Rgb = Rgb(0xff, 0x00, 0xff);
/// Pixel alpha of the overlay's translucent half.
const PIXEL_ALPHA: u8 = 0x80;

fn encode(format: PixelFormat, c: Rgb, alpha: u8) -> Option<[u8; 4]> {
    if format == PixelFormat::XRGB8888 || format == PixelFormat::ARGB8888 {
        Some([c.2, c.1, c.0, alpha])
    } else if format == PixelFormat::XBGR8888 || format == PixelFormat::ABGR8888 {
        Some([c.0, c.1, c.2, alpha])
    } else {
        None
    }
}

fn has_alpha(format: PixelFormat) -> bool {
    format == PixelFormat::ARGB8888 || format == PixelFormat::ABGR8888
}

/// Fills `rect`, clipped to `size`, with one pixel value.
fn fill(pixels: &mut [u8], pitch: usize, size: Size, rect: Rect, pixel: [u8; 4]) {
    let clip = |start: i32, length: u32, limit: u32| {
        let end = (start as i64 + length as i64).clamp(0, limit as i64) as usize;
        (start.clamp(0, limit as i32) as usize, end)
    };
    let (x0, x1) = clip(rect.x, rect.width, size.width);
    let (y0, y1) = clip(rect.y, rect.height, size.height);
    if x0 >= x1 {
        return;
    }
    let row = pixel.repeat(x1 - x0);
    for y in y0..y1 {
        let start = y * pitch + x0 * 4;
        if let Some(target) = pixels.get_mut(start..start + row.len()) {
            target.copy_from_slice(&row);
        }
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width as i32).min(b.x + b.width as i32);
    let y1 = (a.y + a.height as i32).min(b.y + b.height as i32);
    Rect::new(x0, y0, (x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32)
}

fn contains(r: Rect, x: i32, y: i32) -> bool {
    x >= r.x && y >= r.y && x < r.x + r.width as i32 && y < r.y + r.height as i32
}

fn centre(r: Rect) -> (i32, i32) {
    (r.x + r.width as i32 / 2, r.y + r.height as i32 / 2)
}

fn offset(r: Rect, by: Rect) -> Rect {
    Rect::new(r.x + by.x, r.y + by.y, r.width, r.height)
}

/// How the overlay's pixels combine with the background, as in twoplane.
#[derive(Clone, Copy)]
struct Look {
    format: PixelFormat,
    blend: BlendMode,
}

impl Look {
    fn pixel_alpha(&self) -> bool {
        has_alpha(self.format) && self.blend != BlendMode::Opaque
    }

    fn stored(&self, colour: Rgb, alpha: u8) -> Rgb {
        if self.blend != BlendMode::Premultiplied || !self.pixel_alpha() {
            return colour;
        }
        let mul = |c: u8| ((c as u32 * alpha as u32 + 127) / 255) as u8;
        Rgb(mul(colour.0), mul(colour.1), mul(colour.2))
    }

    /// Colour on screen, by the KMS blending equations at opaque plane alpha.
    fn over(&self, colour: Rgb, alpha: u8, under: Rgb) -> Rgb {
        let stored = self.stored(colour, alpha);
        let fa = if self.pixel_alpha() {
            alpha as f64 / 255.0
        } else {
            1.0
        };
        let channel = |c: u8, u: u8| {
            let (c, u) = (c as f64, u as f64);
            let value = match self.blend {
                BlendMode::Opaque => c,
                BlendMode::Premultiplied => c + (1.0 - fa) * u,
                BlendMode::Coverage => fa * c + (1.0 - fa) * u,
            };
            value.round().clamp(0.0, 255.0) as u8
        };
        Rgb(
            channel(stored.0, under.0),
            channel(stored.1, under.1),
            channel(stored.2, under.2),
        )
    }
}

/// The two-plane picture of twoplane, without the moving bar and the frame
/// counter: the picture stays still.
struct Scene {
    output: Size,
    blocks: [(&'static str, Rect, Rgb); 3],
    overlay: Rect,
    /// Overlay regions, in overlay buffer pixels.
    opaque: Rect,
    translucent: Rect,
}

impl Scene {
    fn new(output: Size) -> Self {
        let (w, h) = (output.width, output.height);
        let block = |x: u32| Rect::new(x as i32, (h / 8) as i32, w / 5, h / 2);
        let overlay = Rect::new(
            (w / 2) as i32,
            (h / 4) as i32,
            (w / 5).max(4),
            (h / 2).max(4),
        );
        let upper = overlay.height / 2;
        Self {
            output,
            blocks: [
                ("red", block(w / 10), RED),
                ("green", block(2 * w / 5), GREEN),
                ("blue", block(7 * w / 10), BLUE),
            ],
            overlay,
            opaque: Rect::new(0, 0, overlay.width, upper),
            translucent: Rect::new(0, upper as i32, overlay.width, overlay.height - upper),
        }
    }

    fn overlay_size(&self) -> Size {
        Size::new(self.overlay.width, self.overlay.height)
    }

    fn background_at(&self, x: i32, y: i32) -> Rgb {
        self.blocks
            .iter()
            .find(|(_, r, _)| contains(*r, x, y))
            .map_or(BACKGROUND, |(_, _, c)| *c)
    }

    fn expected_at(&self, look: &Look, x: i32, y: i32) -> Rgb {
        let under = self.background_at(x, y);
        if !contains(self.overlay, x, y) {
            return under;
        }
        let local = (x - self.overlay.x, y - self.overlay.y);
        if contains(self.opaque, local.0, local.1) {
            look.over(YELLOW, 0xff, under)
        } else {
            look.over(MAGENTA, PIXEL_ALPHA, under)
        }
    }

    /// Named sample points away from edges, as twoplane's.
    fn samples(&self) -> Vec<(&'static str, (i32, i32))> {
        let green = self.blocks[1].1;
        let green_end = green.x + green.width as i32;
        let overlay_end = self.overlay.x + self.overlay.width as i32;
        let opaque = offset(self.opaque, self.overlay);
        let translucent = offset(self.translucent, self.overlay);
        let right = |r: Rect| Rect::new(green_end, r.y, (overlay_end - green_end) as u32, r.height);
        let (w, h) = (self.output.width as i32, self.output.height as i32);
        vec![
            ("background", (w / 20, h * 7 / 10)),
            ("red block", centre(self.blocks[0].1)),
            ("blue block", centre(self.blocks[2].1)),
            (
                "green block uncovered",
                centre(Rect::new(
                    green.x,
                    green.y,
                    (self.overlay.x - green.x).max(1) as u32,
                    green.height,
                )),
            ),
            (
                "overlay opaque over green",
                centre(intersect(opaque, green)),
            ),
            ("overlay opaque over background", centre(right(opaque))),
            (
                "overlay translucent over green",
                centre(intersect(translucent, green)),
            ),
            (
                "overlay translucent over background",
                centre(right(translucent)),
            ),
        ]
    }

    /// The `EXPECT` lines, in twoplane's format, phase `normal` only.
    fn expect_lines(&self, look: &Look) -> Vec<String> {
        let o = self.output;
        self.samples()
            .into_iter()
            .map(|(name, (x, y))| {
                format!(
                    "EXPECT phase=normal \"{name}\" x={x} y={y} fx={:.4} fy={:.4} rgb={}",
                    x as f64 / o.width as f64,
                    y as f64 / o.height as f64,
                    self.expected_at(look, x, y).hex()
                )
            })
            .collect()
    }

    fn draw_background(&self, pixels: &mut [u8], pitch: usize, format: PixelFormat) {
        let px = |c| encode(format, c, 0xff).unwrap();
        let all = Rect::new(0, 0, self.output.width, self.output.height);
        fill(pixels, pitch, self.output, all, px(BACKGROUND));
        for (_, rect, colour) in &self.blocks {
            fill(pixels, pitch, self.output, *rect, px(*colour));
        }
    }

    fn draw_overlay(&self, pixels: &mut [u8], pitch: usize, look: &Look) {
        let size = self.overlay_size();
        let opaque = encode(look.format, YELLOW, 0xff).unwrap();
        let stored = look.stored(MAGENTA, PIXEL_ALPHA);
        let translucent = encode(look.format, stored, PIXEL_ALPHA).unwrap();
        fill(pixels, pitch, size, self.opaque, opaque);
        fill(pixels, pitch, size, self.translucent, translucent);
    }
}

/// Handles events until `submission` latched or the timeout passed.
fn wait_latch(engine: &mut Engine<DrmBackend>, submission: SubmissionId) -> Result<(), String> {
    let deadline = Instant::now() + LATCH_TIMEOUT;
    loop {
        while let Some(event) = engine
            .poll_event()
            .map_err(|e| format!("event error {e:?}"))?
        {
            match event {
                DisplayEvent::Presented { submission: s, .. } if s >= submission => return Ok(()),
                DisplayEvent::DeviceLost => return Err("device lost".into()),
                _ => {}
            }
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(format!(
                "submission {} not latched within {LATCH_TIMEOUT:?}",
                submission.0
            ));
        }
        engine
            .backend()
            .wait_for_events(Some(left))
            .map_err(|e| format!("waiting for events: {e}"))?;
    }
}

/// Commits the two-plane picture, keeps it for `seconds`, then blanks the
/// output. Returns a note on the blanking, or what failed.
fn show(
    engine: &mut Engine<DrmBackend>,
    output_index: usize,
    output: OutputId,
    size: Size,
    seconds: u32,
) -> Result<String, String> {
    let caps = *engine.capabilities();
    let pick = |kind: PlaneKind, order: &[PixelFormat]| {
        order.iter().copied().find(|f| {
            on_output(&caps, output_index)
                .filter(|p| p.kind == kind)
                .any(|p| {
                    p.formats
                        .iter()
                        .any(|m| m.format == *f && m.modifier == Modifier::LINEAR)
                })
        })
    };
    let primary_format = pick(
        PlaneKind::Primary,
        &[
            PixelFormat::XRGB8888,
            PixelFormat::ARGB8888,
            PixelFormat::XBGR8888,
            PixelFormat::ABGR8888,
        ],
    )
    .ok_or("no primary plane with a linear 32-bit RGB format")?;
    let overlay_format = pick(
        PlaneKind::Overlay,
        &[
            PixelFormat::ARGB8888,
            PixelFormat::ABGR8888,
            PixelFormat::XRGB8888,
            PixelFormat::XBGR8888,
        ],
    )
    .ok_or("no overlay plane with a linear 32-bit RGB format")?;
    let modes = |kind: PlaneKind| {
        on_output(&caps, output_index)
            .filter(|p| p.kind == kind)
            .fold(0, |m, p| m | p.blend_modes)
    };
    let overlay_modes = modes(PlaneKind::Overlay);
    let blend = if !has_alpha(overlay_format) {
        BlendMode::Opaque
    } else if overlay_modes & BlendMode::Premultiplied.mask() != 0 {
        BlendMode::Premultiplied
    } else if overlay_modes & BlendMode::Coverage.mask() != 0 {
        BlendMode::Coverage
    } else {
        BlendMode::Opaque
    };
    let look = Look {
        format: overlay_format,
        blend,
    };
    let primary_blend = if primary_format.is_known_opaque()
        || modes(PlaneKind::Primary) & BlendMode::Opaque.mask() != 0
    {
        BlendMode::Opaque
    } else {
        BlendMode::Premultiplied
    };
    let scene = Scene::new(size);
    for (id, info, is_primary) in [
        (
            SHOW_PRIMARY_ID,
            BufferInfo {
                size,
                format: primary_format,
                modifier: Modifier::LINEAR,
            },
            true,
        ),
        (
            SHOW_OVERLAY_ID,
            BufferInfo {
                size: scene.overlay_size(),
                format: overlay_format,
                modifier: Modifier::LINEAR,
            },
            false,
        ),
    ] {
        engine
            .backend_mut()
            .create_dumb_buffer(id, info)
            .map_err(|e| format!("dumb buffer {}: {e}", id.0))?;
        engine
            .register_buffer(id, info)
            .map_err(|e| format!("register {}: {e:?}", id.0))?;
        engine
            .backend_mut()
            .draw_dumb_buffer(id, |pixels, pitch| {
                if is_primary {
                    scene.draw_background(pixels, pitch, primary_format);
                } else {
                    scene.draw_overlay(pixels, pitch, &look);
                }
            })
            .map_err(|e| format!("draw {}: {e}", id.0))?;
        engine
            .finish_write(id)
            .map_err(|e| format!("finish {}: {e:?}", id.0))?;
    }
    let full = Rect::new(0, 0, size.width, size.height);
    let layers = [
        Layer {
            blend: primary_blend,
            ..Layer::new(SHOW_PRIMARY_ID, full, full, 0)
        },
        Layer {
            blend: look.blend,
            alpha: ALPHA_OPAQUE,
            ..Layer::new(
                SHOW_OVERLAY_ID,
                Rect::new(0, 0, scene.overlay.width, scene.overlay.height),
                scene.overlay,
                1,
            )
        },
    ];
    let frame = Frame {
        output,
        layers: &layers,
    };
    let plan = engine
        .validate(&frame)
        .map_err(|e| format!("picture: validate: {e:?}"))?;
    for a in plan.assignments() {
        println!(
            "SHOW layer {} -> plane {} zpos {}",
            a.layer, a.plane.0, a.zpos
        );
    }
    let submission = engine
        .commit(&frame, &plan)
        .map_err(|e| format!("picture: commit: {e:?}"))?;
    wait_latch(engine, submission).map_err(|e| format!("picture: {e}"))?;
    for line in scene.expect_lines(&look) {
        println!("{line}");
    }
    println!("SHOW on screen for {seconds} s");
    std::thread::sleep(Duration::from_secs(u64::from(seconds)));

    // Blank the output so that the buffers come back. Not every driver
    // allows an active output without a primary plane; then they stay on
    // screen until the device is closed.
    let blank = Frame {
        output,
        layers: &[],
    };
    let note = match engine
        .validate(&blank)
        .and_then(|plan| engine.commit(&blank, &plan))
    {
        Ok(submission) => match wait_latch(engine, submission) {
            Ok(()) => "blanked".to_string(),
            Err(error) => format!("blank frame: {error}"),
        },
        Err(error) => {
            format!("blank frame refused: {error:?}; the picture stays until the device is closed")
        }
    };
    Ok(note)
}

fn verdict(pass: bool, reason: impl std::fmt::Display, code: u8) -> ExitCode {
    println!(
        "VERDICT profile_check: {} {reason}",
        if pass { "PASS" } else { "FAIL" }
    );
    ExitCode::from(code)
}

fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("{message}");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    println!("DATE {}", utc_now());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    println!("KERNEL {kernel}");
    println!("DEVICE {}", options.device);

    let drm_options = DrmOptions {
        scaling: None,
        profile: options.profile,
    };
    let backend = match DrmBackend::open(&options.device, drm_options) {
        Ok(backend) => backend,
        Err(error) => {
            return verdict(false, format!("cannot open {}: {error}", options.device), 2);
        }
    };
    let driver = backend.driver_name().unwrap_or_else(|_| "unknown".into());
    println!("DRIVER {driver}");
    println!("PROFILE {:?}", options.profile);
    if let Err(error) = backend.card().acquire_master_lock() {
        println!(
            "DRM master is unavailable on {}: {error}. Another process (a desktop or compositor) \
             probably holds it: stop it for the run and restore it afterwards. This example never \
             stops other processes.",
            options.device
        );
        return verdict(false, format!("DRM master unavailable on {driver}"), 2);
    }

    let mut engine = Engine::new(backend);
    let caps = *engine.capabilities();
    for line in caps_lines(&caps) {
        println!("{line}");
    }
    let Some(output) = caps.outputs.get(options.output).copied() else {
        return verdict(
            false,
            format!(
                "driver {driver}: no output {} ({} connected)",
                options.output,
                caps.outputs.len()
            ),
            2,
        );
    };
    println!(
        "MODE {}x{} output {} (crtc {})",
        output.size.width, output.size.height, options.output, output.id.0
    );

    let matrix = build_matrix(&caps, options.output);
    let mut allocated: HashMap<BufferId, Result<(), String>> = HashMap::new();
    let mut agreements = Vec::new();
    let mut errors = Vec::new();
    for case in &matrix {
        for buffer in &case.buffers {
            allocated
                .entry(buffer.id)
                .or_insert_with(|| allocate(engine.backend_mut(), buffer));
        }
        let outcome = run_case(
            engine.backend_mut(),
            &caps,
            options.output,
            output.id,
            case,
            &allocated,
        );
        if let Kernel::Error(why) = &outcome.kernel {
            errors.push(format!("{}: {why}", case.name));
        }
        println!("{}", check_line(&case.name, &outcome));
        agreements.push(outcome.agree());
    }
    for (id, made) in &allocated {
        if made.is_ok() {
            let _ = engine.backend_mut().remove_buffer(*id);
        }
    }
    let counts = tally(&agreements);
    println!("{}", agreement_line(&counts));

    if let Some(seconds) = options.show {
        match show(&mut engine, options.output, output.id, output.size, seconds) {
            Ok(note) => println!("SHOW done: {note}"),
            Err(error) => errors.push(format!("show: {error}")),
        }
    }

    let summary = format!(
        "profile {:?} driver {driver}: {} frames, {} agree, {} stricter, {} looser",
        options.profile, counts.total, counts.agree, counts.stricter, counts.looser
    );
    if counts.looser > 0 {
        verdict(false, format!("planner-looser frames, {summary}"), 1)
    } else if let Some(first) = errors.first() {
        for error in &errors {
            println!("ERROR {error}");
        }
        verdict(
            false,
            format!("{first} ({} error(s)), {summary}", errors.len()),
            1,
        )
    } else {
        verdict(true, summary, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use composition_core::{OutputCaps, PlaneCaps, PlaneId, RejectReasons};

    fn args(text: &str) -> Result<Options, String> {
        parse_args(text.split_whitespace().map(String::from))
    }

    #[test]
    fn parses_device_profile_and_options() {
        assert_eq!(
            args("/dev/dri/card1 --profile rk3588"),
            Ok(Options {
                device: "/dev/dri/card1".into(),
                profile: DriverProfile::Rk3588Vop2,
                output: 0,
                show: None,
            })
        );
        let o = args("--profile hvs6 /dev/dri/card0 --output 1 --show 20").unwrap();
        assert_eq!(o.profile, DriverProfile::Vc4Hvs6);
        assert_eq!((o.output, o.show), (1, Some(20)));
        for (name, profile) in [
            ("generic", DriverProfile::Generic),
            ("imx95", DriverProfile::Imx95Dpu),
            ("stm32", DriverProfile::Stm32Ltdc),
            ("hvs5", DriverProfile::Vc4Hvs5),
        ] {
            assert_eq!(
                args(&format!("d --profile {name}")).unwrap().profile,
                profile
            );
        }
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(args("/dev/dri/card0").is_err(), "profile required");
        assert!(args("--profile generic").is_err(), "device required");
        assert!(args("d --profile vop3").is_err());
        assert!(args("d --profile").is_err());
        assert!(args("d --profile generic --output x").is_err());
        assert!(args("d --profile generic --show -1").is_err());
        assert!(args("d e --profile generic").is_err());
        assert!(args("d --profile generic --frobnicate").is_err());
        assert_eq!(args("--help"), Err(String::new()));
    }

    #[test]
    fn classification_of_the_four_cases_and_no_answer() {
        assert_eq!(classify(true, &Kernel::Ok), Agree::Yes);
        assert_eq!(classify(false, &Kernel::Refused(22)), Agree::Yes);
        assert_eq!(classify(false, &Kernel::Ok), Agree::Stricter);
        assert_eq!(classify(true, &Kernel::Refused(34)), Agree::Looser);
        assert_eq!(classify(true, &Kernel::NotReached("x".into())), Agree::No);
        assert_eq!(classify(false, &Kernel::Error("x".into())), Agree::No);
    }

    #[test]
    fn check_and_agreement_lines() {
        let outcome = Outcome {
            planner: Ok(vec![33, 45]),
            kernel: Kernel::Refused(34),
            via: Via::Plan,
            candidates: 3,
        };
        assert_eq!(
            check_line("scale-8", &outcome),
            "CHECK scale-8 planner=plan:33,45 test_only=ERANGE agree=planner-looser via=plan candidates=3"
        );
        let outcome = Outcome {
            planner: Err("layer1:SCALING".into()),
            kernel: Kernel::Ok,
            via: Via::Forced,
            candidates: 1,
        };
        assert_eq!(
            check_line("scale-2", &outcome),
            format!("CHECK scale-2 planner=refuse:layer1:SCALING test_only=ok agree=planner-stricter via=forced candidates=1 note={FORCED_NOTE}")
        );
        let outcome = Outcome {
            planner: Err("layer1:TRANSFORM".into()),
            kernel: Kernel::NotReached("a=b".into()),
            via: Via::None,
            candidates: 0,
        };
        assert!(check_line("rotate-90", &outcome)
            .ends_with("test_only=not reached agree=no via=none candidates=0 note=a:b"));

        let t = tally(&[
            Agree::Yes,
            Agree::Yes,
            Agree::Stricter,
            Agree::Looser,
            Agree::No,
        ]);
        assert_eq!(
            agreement_line(&t),
            "AGREEMENT total=5 agree=2 stricter=1 looser=1"
        );
    }

    #[test]
    fn refusal_text_names_the_layer_and_reasons() {
        let error = PlanError::NoPlane {
            layer: 1,
            reasons: RejectReasons::SCALING | RejectReasons::SOURCE_SIZE,
        };
        assert_eq!(refuse_text(&error), "layer1:SCALING+SOURCE_SIZE");
        assert_eq!(
            refuse_text(&PlanError::CannotClip { layer: 0 }),
            "CannotClip{layer:0}"
        );
    }

    #[test]
    fn nv12_layout_has_two_planes() {
        let layout = fb_layout(
            &format_planes(PixelFormat::NV12).unwrap(),
            Size::new(100, 50),
        )
        .unwrap();
        assert_eq!(layout.count, 2);
        assert_eq!(layout.pitches[..2], [128, 128]);
        assert_eq!(layout.offsets[..2], [0, 8192]);
        assert!(format_planes(PixelFormat::fourcc(b"HWL0")).is_none());
    }

    const AFBC: Modifier = Modifier(0x0800_0000_0000_0051);
    const T_TILED: Modifier = Modifier(0x0700_0000_0000_0001);
    const SAND64: Modifier = Modifier(0x0700_0000_0000_0003);
    const SAND128: Modifier = Modifier(0x0700_0000_0000_0004);

    fn plane(id: u32, kind: PlaneKind, zpos: Zpos, pairs: &[(PixelFormat, Modifier)]) -> PlaneCaps {
        let mut plane = PlaneCaps {
            id: PlaneId(id),
            kind,
            outputs: 1,
            zpos,
            plane_alpha: true,
            blend_modes: BlendMode::Opaque.mask()
                | BlendMode::Premultiplied.mask()
                | BlendMode::Coverage.mask(),
            ..PlaneCaps::default()
        };
        for (format, modifier) in pairs {
            plane
                .formats
                .push(FormatModifier {
                    format: *format,
                    modifier: *modifier,
                })
                .unwrap();
        }
        plane
    }

    fn with_output(planes: Vec<PlaneCaps>, profile: DriverProfile) -> Capabilities {
        let mut caps = Capabilities::default();
        caps.outputs
            .push(OutputCaps {
                id: OutputId(80),
                size: Size::new(1920, 1080),
                refresh_mhz: 60_000,
            })
            .unwrap();
        for p in planes {
            caps.planes.push(p).unwrap();
        }
        profile.apply(&mut caps);
        caps
    }

    // Hand-built shapes, not copies of the recorded lists: the plane kinds,
    // classes, reach and zpos follow tests/data/rk3588/README.md (NanoPC-T6,
    // observed 2026-11-27), with two formats per class instead of the
    // recorded 126 and 21 pairs; the VC4 shape takes the modifier kinds of
    // tests/data/bcm2711/probe-raw-2026-11-27.txt with six planes instead of
    // the recorded count.

    /// RK3588-shaped: Cluster planes 33 (primary), 39 (primary on the other
    /// CRTC only), 45 and 51 list AFBC only; Esmart overlays 57 to 75 list
    /// linear RGB and NV12; zpos 0..=7 everywhere.
    fn rk3588() -> Capabilities {
        let z = Zpos::Range { min: 0, max: 7 };
        let linear = [
            (PixelFormat::XRGB8888, Modifier::LINEAR),
            (PixelFormat::ARGB8888, Modifier::LINEAR),
            (PixelFormat::NV12, Modifier::LINEAR),
        ];
        let afbc = [(PixelFormat::XRGB8888, AFBC), (PixelFormat::ARGB8888, AFBC)];
        let mut planes = Vec::new();
        for (id, kind) in [
            (33, PlaneKind::Primary),
            (39, PlaneKind::Primary),
            (45, PlaneKind::Overlay),
            (51, PlaneKind::Overlay),
        ] {
            let mut cluster = plane(id, kind, z, &afbc);
            if id == 39 {
                cluster.outputs = 0b10;
            }
            planes.push(cluster);
        }
        for id in [57, 63, 69, 75] {
            planes.push(plane(id, PlaneKind::Overlay, z, &linear));
        }
        with_output(planes, DriverProfile::Rk3588Vop2)
    }

    /// VC4-shaped: a primary, four overlays and a cursor, with T-tiled RGB
    /// and SAND NV12 pairs.
    fn vc4(profile: DriverProfile) -> Capabilities {
        let pairs = [
            (PixelFormat::XRGB8888, Modifier::LINEAR),
            (PixelFormat::ARGB8888, Modifier::LINEAR),
            (PixelFormat::XRGB8888, T_TILED),
            (PixelFormat::NV12, Modifier::LINEAR),
            (PixelFormat::NV12, SAND64),
            (PixelFormat::NV12, SAND128),
        ];
        let mut planes = vec![plane(
            60,
            PlaneKind::Primary,
            Zpos::Range { min: 0, max: 5 },
            &pairs,
        )];
        for k in 0..4 {
            planes.push(plane(
                70 + k,
                PlaneKind::Overlay,
                Zpos::Range { min: 0, max: 5 },
                &pairs,
            ));
        }
        planes.push(plane(
            90,
            PlaneKind::Cursor,
            Zpos::Range { min: 0, max: 5 },
            &[(PixelFormat::ARGB8888, Modifier::LINEAR)],
        ));
        with_output(planes, profile)
    }

    fn outcomes(caps: &Capabilities) -> HashMap<String, PlannerVerdict> {
        build_matrix(caps, 0)
            .iter()
            .map(|case| {
                let frame = Frame {
                    output: OutputId(80),
                    layers: &case.layers,
                };
                let verdict = planner_verdict(&plan(&frame, caps, &Lookup(&case.buffers)));
                (case.name.clone(), verdict)
            })
            .collect()
    }

    fn names(caps: &Capabilities) -> Vec<String> {
        build_matrix(caps, 0).into_iter().map(|c| c.name).collect()
    }

    fn refused_for(verdicts: &HashMap<String, PlannerVerdict>, name: &str, reason: &str) -> bool {
        matches!(&verdicts[name], Err(r) if r.contains(reason))
    }

    #[test]
    fn matrix_has_the_fixed_frames() {
        let caps = rk3588();
        let names = names(&caps);
        // Plane 39 cannot reach the output: seven planes.
        let mut expected: Vec<String> = (1..=7).map(|k| format!("layers-{k}")).collect();
        for s in ["0.5", "0.75", "1.5", "2", "4", "8"] {
            expected.push(format!("scale-{s}"));
        }
        expected.push("nv12".into());
        expected.push("modifier-XR24-0x0800000000000051".into());
        for n in ["rotate-90", "reflect-y", "wide-source", "below-primary"] {
            expected.push(n.into());
        }
        assert_eq!(names, expected);
        for case in build_matrix(&caps, 0) {
            // Every layer's buffer is listed, and its source fits the buffer.
            for layer in &case.layers {
                let spec = case.buffers.iter().find(|b| b.id == layer.buffer).unwrap();
                assert!(layer.source.fits_within(spec.info.size), "{}", case.name);
            }
            let rotated = case.layers.iter().any(|l| l.transform != Transform::Normal);
            assert_eq!(case.unreachable.is_some(), rotated, "{}", case.name);
        }
        // No output, no matrix.
        assert!(build_matrix(&caps, 3).is_empty());
    }

    #[test]
    fn scale_frames_have_the_named_factors() {
        let caps = rk3588();
        for case in build_matrix(&caps, 0) {
            let Some(text) = case.name.strip_prefix("scale-") else {
                continue;
            };
            let f: f64 = text.parse().unwrap();
            let l = case.layers[1];
            assert_eq!(l.destination.width as f64, l.source.width as f64 * f);
            assert_eq!(l.destination.height as f64, l.source.height as f64 * f);
        }
    }

    #[test]
    fn matrix_on_rk3588_vop2() {
        let v = outcomes(&rk3588());
        // Only the four Esmart planes take the linear RGB tiles; Cluster
        // planes list AFBC only.
        for k in 1..=4 {
            assert!(v[&format!("layers-{k}")].is_ok(), "layers-{k}");
        }
        for k in 5..=7 {
            assert!(
                refused_for(&v, &format!("layers-{k}"), "FORMAT"),
                "layers-{k}"
            );
        }
        // Up to 8x either way on both window classes.
        for s in ["0.5", "0.75", "1.5", "2", "4", "8"] {
            assert!(v[&format!("scale-{s}")].is_ok(), "scale-{s}");
        }
        assert!(v["nv12"].is_ok());
        assert!(v["modifier-XR24-0x0800000000000051"].is_ok());
        // max_source 4096 wide.
        assert!(refused_for(&v, "wide-source", "SOURCE_SIZE"));
        assert!(refused_for(&v, "rotate-90", "TRANSFORM"));
        assert!(refused_for(&v, "reflect-y", "TRANSFORM"));
        // Every zpos is settable, so the small layer can go below.
        assert!(v["below-primary"].is_ok());
    }

    #[test]
    fn matrix_on_vc4_hvs5() {
        let caps = vc4(DriverProfile::Vc4Hvs5);
        let names = names(&caps);
        assert!(names.contains(&"modifier-XR24-0x0700000000000001".to_string()));
        assert!(names.contains(&"modifier-NV12-0x0700000000000003".to_string()));
        assert!(names.contains(&"modifier-NV12-0x0700000000000004".to_string()));
        assert!(names.contains(&"layers-6".to_string()));
        let v = outcomes(&caps);
        // Up to 32x up, 2x down.
        for s in ["0.5", "0.75", "1.5", "2", "4", "8"] {
            assert!(v[&format!("scale-{s}")].is_ok(), "scale-{s}");
        }
        assert!(v["nv12"].is_ok());
        // 4352 to 1920 is a downscale beyond 2.
        assert!(refused_for(&v, "wide-source", "SCALING"));
        assert!(refused_for(&v, "rotate-90", "TRANSFORM"));
    }

    #[test]
    fn matrix_on_vc4_hvs6() {
        let caps = vc4(DriverProfile::Vc4Hvs6);
        let names = names(&caps);
        // The profile drops the T-tiled and SAND64 pairs.
        assert!(!names.iter().any(|n| n.ends_with("0x0700000000000001")));
        assert!(!names.iter().any(|n| n.ends_with("0x0700000000000003")));
        assert!(names.contains(&"modifier-NV12-0x0700000000000004".to_string()));
        let v = outcomes(&caps);
        assert!(v["scale-8"].is_ok());
        assert!(v["scale-0.5"].is_ok());
        assert!(v["modifier-NV12-0x0700000000000004"].is_ok());
        assert!(refused_for(&v, "wide-source", "SCALING"));
    }

    #[test]
    fn matrix_on_generic_refuses_scaling_and_a_fixed_primary_on_top() {
        let mut planes = vec![plane(
            31,
            PlaneKind::Primary,
            Zpos::Fixed(0),
            &[(PixelFormat::XRGB8888, Modifier::LINEAR)],
        )];
        planes.push(plane(
            40,
            PlaneKind::Overlay,
            Zpos::Fixed(1),
            &[(PixelFormat::ARGB8888, Modifier::LINEAR)],
        ));
        let caps = with_output(planes, DriverProfile::Generic);
        let v = outcomes(&caps);
        assert!(v["layers-1"].is_ok());
        assert!(refused_for(&v, "scale-2", "SCALING"));
        assert!(refused_for(&v, "nv12", "FORMAT"));
        assert!(v["below-primary"].is_err());
        // The forced caps claim what was missing, except stacking.
        let case = build_matrix(&caps, 0)
            .into_iter()
            .find(|c| c.name == "scale-2")
            .unwrap();
        let forced = relax(&caps, 0, &case.buffers);
        let frame = Frame {
            output: OutputId(80),
            layers: &case.layers,
        };
        assert!(plan(&frame, &forced, &Lookup(&case.buffers)).is_ok());
        assert_eq!(forced.planes.first().unwrap().zpos, Zpos::Fixed(0));
    }

    #[test]
    fn relax_claims_formats_and_drops_limits_but_not_on_cursors() {
        let caps = vc4(DriverProfile::Vc4Hvs6);
        assert!(!caps.resource_groups.is_empty());
        let wanted = BufferSpec {
            id: BufferId(9),
            info: BufferInfo {
                size: Size::new(64, 64),
                format: PixelFormat::YUYV,
                modifier: Modifier::LINEAR,
            },
        };
        let forced = relax(&caps, 0, &[wanted]);
        assert!(forced.resource_groups.is_empty());
        for p in forced.planes.iter() {
            let listed = p.formats.iter().any(|f| f.format == PixelFormat::YUYV);
            if p.kind == PlaneKind::Cursor {
                assert!(!listed && p.scaling.is_none());
            } else {
                assert!(listed && p.max_source.is_none());
                assert_eq!(p.scaling.unwrap().max_downscale, FORCED_SCALE);
            }
        }
    }

    #[test]
    fn caps_lines_show_what_the_profile_added() {
        let lines = caps_lines(&rk3588());
        assert_eq!(lines[0], "output 0: crtc 80 1920x1080 @ 60.000 Hz");
        assert!(lines[1].starts_with("plane 33 Primary: outputs 0b1, zpos 0..=7"));
        assert!(lines[1].contains("scaling up 8.00 down 8.00, max source 4096x4320"));
        assert_eq!(
            lines[2],
            "  formats (2): XR24:0x800000000000051 AR24:0x800000000000051"
        );
        assert!(lines[3].starts_with("plane 39 Primary: outputs 0b10,"));
        assert_eq!(lines[14], "  formats (3): XR24 AR24 NV12");
        let hvs6 = caps_lines(&vc4(DriverProfile::Vc4Hvs6));
        assert!(hvs6
            .iter()
            .any(|l| l.starts_with("resource group: consumers")));
    }

    fn picture() -> (Scene, Look) {
        let look = Look {
            format: PixelFormat::ARGB8888,
            blend: BlendMode::Premultiplied,
        };
        (Scene::new(Size::new(1920, 1080)), look)
    }

    #[test]
    fn expect_lines_are_twoplanes_normal_phase() {
        let (scene, look) = picture();
        let lines = scene.expect_lines(&look);
        assert_eq!(lines.len(), 8);
        assert!(lines
            .iter()
            .all(|l| l.starts_with("EXPECT phase=normal \"")));
        assert!(lines.contains(
            &"EXPECT phase=normal \"red block\" x=384 y=405 fx=0.2000 fy=0.3750 rgb=#ff0000"
                .to_string()
        ));
        let yellow = lines
            .iter()
            .find(|l| l.contains("\"overlay opaque over green\""))
            .unwrap();
        assert!(yellow.ends_with("rgb=#ffff00"), "{yellow}");
        // Magenta at alpha 0x80 over green, premultiplied.
        let blended = lines
            .iter()
            .find(|l| l.contains("\"overlay translucent over green\""))
            .unwrap();
        assert!(blended.ends_with("rgb=#807f80"), "{blended}");
    }

    /// Feeds the EXPECT lines through `scripts/twoplane-expect.py`, which
    /// must write `expect-normal.json` with one region per line.
    #[test]
    fn twoplane_expect_accepts_the_expect_lines() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let (scene, look) = picture();
        let mut output = String::from(
            "CHECK layers-1 planner=plan:33 test_only=ok agree=yes via=plan candidates=1\n",
        );
        for line in scene.expect_lines(&look) {
            output.push_str(&line);
            output.push('\n');
        }
        output.push_str("VERDICT profile_check: PASS\n");
        let script = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/twoplane-expect.py"
        );
        let out = std::env::temp_dir().join(format!("profile-check-expect-{}", std::process::id()));
        let child = Command::new("python3")
            .arg("-I")
            .arg(script)
            .arg("-")
            .arg("--out")
            .arg(&out)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("python3 not found, twoplane-expect.py not run");
                return;
            }
            Err(error) => panic!("python3: {error}"),
        };
        child
            .stdin
            .take()
            .unwrap()
            .write_all(output.as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        let json = std::fs::read_to_string(out.join("expect-normal.json"));
        let _ = std::fs::remove_dir_all(&out);
        assert!(
            result.status.success(),
            "twoplane-expect.py failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("(8 regions)"), "{stdout}");
        let json = json.unwrap();
        assert!(json.contains("\"red block\""), "{json}");
        assert_eq!(json.matches("\"expect\"").count(), 8, "{json}");
    }
}
