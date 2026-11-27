//! KMS sweep: what does the kernel accept for plane combinations, format and
//! modifier pairs, and a commit that replaces a pending one?
//!
//! ```text
//! cargo run -p composition-drm --example kms_sweep -- <mode> [/dev/dri/cardN] [options]
//!
//!   mode               combos | modifiers | replace
//!   --device PATH      DRM device (default /dev/dri/card0); a bare argument works too
//!   --connector ID     connector to use (default: the first connected one)
//!
//! combos:
//!   --max N            largest number of active planes to try (default: all)
//!   --sets S           k-subsets to try per k besides the first (default 32)
//!   --scale-h F        horizontal scale factor (dst / src) of every layer (default 1)
//!   --scale-v F        vertical scale factor (dst / src) of every layer (default 1)
//!   --unscaled-primary keep the primary plane at 1:1 when scaling
//!
//! modifiers:
//!   --modifier M       also try M on every format of a plane: linear,
//!                      vc4-t-tiled, sand64, sand128, sand256, afbc-16x16 or a
//!                      hex value (0x...); repeatable
//!   --plane ID         test only this plane
//!   --format CODE      test only this format (fourcc such as NV12, or hex)
//!   --size WxH         framebuffer and destination size (default: the mode size)
//!
//! replace:
//!   --wait-ms N        how long to wait for page-flip events (default 1000)
//! ```
//!
//! Every mode opens the device, becomes DRM master (or exits with status 2
//! saying it could not; it never stops other processes), takes the
//! connector's preferred mode and a CRTC for it and allocates linear dumb
//! buffers. The planes it works with are the non-cursor planes whose
//! `possible_crtcs` include that CRTC (the *candidates*), in plane-id order.
//! The primary plane is the candidate primary plane currently on the CRTC, or
//! else the first candidate primary plane.
//!
//! # combos
//!
//! For k = 1..=N one atomic request per plane set is checked with
//! `DRM_MODE_ATOMIC_TEST_ONLY | ALLOW_MODESET`; nothing is committed. Every
//! set holds the primary plane and k-1 other candidates: first the lowest
//! plane ids (`set=0`), then up to `--sets` further (k-1)-subsets of the other
//! candidates, chosen deterministically at evenly spaced ranks of their
//! lexicographic order (all of them if there are fewer). Every active plane
//! shows a full-screen linear dumb buffer: XRGB8888 on the primary and
//! ARGB8888 on the others where listed, else the other one of the two.
//! Planes with a mutable `zpos` get distinct ascending values, the primary
//! lowest, then by plane id. Candidates not in the set are switched off in
//! the request.
//!
//! The scale factor F of an axis is the destination size over the source
//! size, as in `scale_probe` (F > 1 upscales). F >= 1 keeps the destination
//! full-screen and shrinks the source to mode/F; F < 1 keeps the source
//! full-screen and shrinks the destination to mode*F at 0,0. The buffer is
//! always the mode size. With `--unscaled-primary` the primary stays 1:1.
//!
//! # modifiers
//!
//! For each candidate plane, every (format, modifier) pair of its
//! `IN_FORMATS` blob (`listed=yes`), plus every `--modifier` on every format
//! of the plane that the blob does not list for it (`listed=no`), gets one
//! framebuffer: a dumb buffer large enough for the format's planes (with room
//! for AFBC headers) and ADDFB2 with the modifier, then one TEST_ONLY request
//! with that framebuffer on the plane at 0,0, full-screen XRGB8888 linear on
//! the primary below it. A plane without `IN_FORMATS` is tested with its
//! formats and the linear modifier, `listed=no`. Formats whose plane layout
//! the sweep does not know, or that the drm crate's `DrmFourcc` cannot name
//! (XRGB16161616 or R1 at drm-fourcc 2.2), are reported
//! `addfb=unsupported-format`.
//!
//! **A dumb buffer is not tiled or compressed content.** Its bytes are zero
//! and only its size is chosen for the modifier; a pair that passes ADDFB2
//! and TEST_ONLY says what the kernel accepts, not that the hardware would
//! scan out such a buffer correctly. Nothing is committed.
//!
//! # replace
//!
//! This mode makes real commits. A blocking modeset commit
//! (`ALLOW_MODESET | PAGE_FLIP_EVENT`) puts a full-screen grey XRGB8888 frame
//! on the primary plane and switches the other candidates off (`SETUP`). Once
//! its event has arrived, a nonblocking page flip to a second (dark blue)
//! frame is committed, and at once a second nonblocking flip back to the
//! grey frame, before the first flip's event. Both ask for a page-flip event.
//! The sweep then counts the page-flip events of the CRTC until it has one
//! per accepted commit or `--wait-ms` has passed.
//!
//! Nothing is restored beyond releasing the sweep's own objects. When the
//! program removes its framebuffers (and when its file is closed), the
//! kernel switches off the planes that still show them, and with the primary
//! plane the CRTC (*read from source*: `drm_framebuffer_remove` in
//! drivers/gpu/drm/drm_framebuffer.c). The output therefore stays dark until
//! the next client commits; start the display manager or compositor again
//! after the run.
//!
//! # Output
//!
//! ```text
//! DATE / KERNEL / DEVICE / DRIVER / MODE / PLANE lines, as scale_probe prints them
//! COMBO k=2 set=0 planes=31,37 scale=1.0000x1.0000 src=1920.0000x1080.0000 dst=1920x1080 result=ok
//! COMBO_MAX accepted_k=2
//! MODIFIER plane=31 format=NV12 modifier=0x0700000000000004 listed=yes addfb=ok test_only=EINVAL
//! SETUP commit=ok events=1
//! REPLACE first=ok second=EBUSY events=1
//! VERDICT kms_sweep: DONE combos|modifiers|replace
//! ```
//!
//! `result`, `addfb`, `test_only`, `first` and `second` are `ok` or the
//! errno (`EINVAL`, `ERANGE`, `ENOSPC`, `EBUSY`, ... or `errno<N>`);
//! `test_only=not reached` means ADDFB2 failed so there was nothing to test,
//! `addfb=dumb-<errno>` that the dumb buffer itself was refused. `planes`
//! lists the active planes from the bottom of the stack up. `COMBO_MAX` is
//! the largest k whose `set=0` request was accepted (0 if none).
//!
//! The exit status is 0 after `VERDICT ... DONE`, and 2 for a usage error, a
//! device that cannot be used or a missing DRM master
//! (`VERDICT kms_sweep: FAIL ...`).

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use composition_drm::Card;
use drm::buffer::{DrmFourcc, DrmModifier, PlanarBuffer};
use drm::control::atomic::AtomicModeReq;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    connector, crtc, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice,
    Event, FbCmd2Flags, ModeTypeFlags, PlaneType, ResourceHandle,
};
use drm::{ClientCapability, Device as BasicDevice};

const USAGE: &str = "usage: kms_sweep <combos|modifiers|replace> [/dev/dri/cardN] \
[--device PATH] [--connector ID]\n  combos: [--max N] [--sets S] [--scale-h F] [--scale-v F] \
[--unscaled-primary]\n  modifiers: [--modifier M]... [--plane ID] [--format CODE] [--size WxH]\n  \
replace: [--wait-ms N]";

/// `DRM_FORMAT_MOD_INVALID`.
const MOD_INVALID: u64 = 0x00ff_ffff_ffff_ffff;

/// Named modifiers `--modifier` accepts, with their values from
/// include/uapi/drm/drm_fourcc.h (*read from source*).
const MODIFIER_NAMES: [(&str, u64); 6] = [
    ("linear", 0),
    // fourcc_mod_code(BROADCOM, 1)
    ("vc4-t-tiled", 0x0700_0000_0000_0001),
    // fourcc_mod_code(BROADCOM, 3..5), column-height parameter 0
    ("sand64", 0x0700_0000_0000_0003),
    ("sand128", 0x0700_0000_0000_0004),
    ("sand256", 0x0700_0000_0000_0005),
    // DRM_FORMAT_MOD_ARM_AFBC(AFBC_FORMAT_MOD_BLOCK_SIZE_16x16)
    ("afbc-16x16", 0x0800_0000_0000_0001),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Combos,
    Modifiers,
    Replace,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Combos => "combos",
            Mode::Modifiers => "modifiers",
            Mode::Replace => "replace",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Options {
    mode: Mode,
    device: String,
    connector: Option<u32>,
    /// combos
    max: Option<usize>,
    sets: usize,
    /// (horizontal, vertical) dst / src.
    scale: (f64, f64),
    unscaled_primary: bool,
    /// modifiers
    modifiers: Vec<u64>,
    plane: Option<u32>,
    format: Option<u32>,
    size: Option<(u32, u32)>,
    /// replace
    wait_ms: u32,
}

/// Parses the arguments after the program name. An empty error asks for the
/// usage text alone.
fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
    let mut args = args.into_iter();
    let mode = match args.next().as_deref() {
        Some("combos") => Mode::Combos,
        Some("modifiers") => Mode::Modifiers,
        Some("replace") => Mode::Replace,
        Some("-h" | "--help") | None => return Err(String::new()),
        Some(other) => return Err(format!("unknown mode {other}")),
    };
    let mut options = Options {
        mode,
        device: "/dev/dri/card0".into(),
        connector: None,
        max: None,
        sets: 32,
        scale: (1.0, 1.0),
        unscaled_primary: false,
        modifiers: Vec::new(),
        plane: None,
        format: None,
        size: None,
        wait_ms: 1000,
    };
    let number = |text: &str| -> Result<u32, String> {
        text.parse().map_err(|_| format!("bad number {text}"))
    };
    let factor = |text: &str| -> Result<f64, String> {
        match text.parse::<f64>() {
            Ok(f) if f.is_finite() && f > 0.0 => Ok(f),
            _ => Err(format!("bad scale factor {text}")),
        }
    };
    let only = |want: Mode, name: &str| -> Result<(), String> {
        if mode == want {
            Ok(())
        } else {
            Err(format!("{name} is only for {}", want.name()))
        }
    };
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--device" => options.device = value("--device")?,
            "--connector" => options.connector = Some(number(&value("--connector")?)?),
            "--max" => {
                only(Mode::Combos, "--max")?;
                let max = number(&value("--max")?)?;
                if max == 0 {
                    return Err("--max must be at least 1".into());
                }
                options.max = Some(max as usize);
            }
            "--sets" => {
                only(Mode::Combos, "--sets")?;
                options.sets = number(&value("--sets")?)? as usize;
            }
            "--scale-h" => {
                only(Mode::Combos, "--scale-h")?;
                options.scale.0 = factor(&value("--scale-h")?)?;
            }
            "--scale-v" => {
                only(Mode::Combos, "--scale-v")?;
                options.scale.1 = factor(&value("--scale-v")?)?;
            }
            "--unscaled-primary" => {
                only(Mode::Combos, "--unscaled-primary")?;
                options.unscaled_primary = true;
            }
            "--modifier" => {
                only(Mode::Modifiers, "--modifier")?;
                let modifier = parse_modifier(&value("--modifier")?)?;
                if !options.modifiers.contains(&modifier) {
                    options.modifiers.push(modifier);
                }
            }
            "--plane" => {
                only(Mode::Modifiers, "--plane")?;
                options.plane = Some(number(&value("--plane")?)?);
            }
            "--format" => {
                only(Mode::Modifiers, "--format")?;
                options.format = Some(parse_fourcc(&value("--format")?)?);
            }
            "--size" => {
                only(Mode::Modifiers, "--size")?;
                let text = value("--size")?;
                let (w, h) = text.split_once('x').ok_or("--size takes WxH")?;
                let (w, h) = (number(w)?, number(h)?);
                if w == 0 || h == 0 {
                    return Err("--size must not be empty".into());
                }
                options.size = Some((w, h));
            }
            "--wait-ms" => {
                only(Mode::Replace, "--wait-ms")?;
                options.wait_ms = number(&value("--wait-ms")?)?;
            }
            "-h" | "--help" => return Err(String::new()),
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => options.device = other.to_string(),
        }
    }
    Ok(options)
}

/// A modifier given by name or as a hex value (with or without `0x`).
fn parse_modifier(text: &str) -> Result<u64, String> {
    if let Some((_, value)) = MODIFIER_NAMES.iter().find(|(name, _)| *name == text) {
        return Ok(*value);
    }
    let digits = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    if digits.is_empty() || digits.len() > 16 {
        return Err(format!("bad modifier {text}"));
    }
    u64::from_str_radix(digits, 16).map_err(|_| format!("bad modifier {text}"))
}

/// A fourcc given as up to four characters (padded with spaces, as in
/// `R8  `) or as a hex value with `0x`.
fn parse_fourcc(text: &str) -> Result<u32, String> {
    if let Some(digits) = text.strip_prefix("0x") {
        return u32::from_str_radix(digits, 16).map_err(|_| format!("bad format {text}"));
    }
    if text.is_empty() || text.len() > 4 || !text.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(format!("bad format {text}"));
    }
    let mut bytes = [b' '; 4];
    bytes[..text.len()].copy_from_slice(text.as_bytes());
    Ok(u32::from_le_bytes(bytes))
}

/// The four characters of a fourcc, trailing spaces dropped, `?` for bytes
/// that are not printable.
fn fourcc_text(code: u32) -> String {
    let text: String = code
        .to_le_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_graphic() || *b == b' ' {
                *b as char
            } else {
                '?'
            }
        })
        .collect();
    text.trim_end().to_string()
}

/// One check's outcome: `Ok` if accepted, the errno otherwise.
type Check = Result<(), i32>;

fn errno_name(errno: i32) -> String {
    let name = match errno {
        1 => "EPERM",
        2 => "ENOENT",
        4 => "EINTR",
        5 => "EIO",
        7 => "E2BIG",
        11 => "EAGAIN",
        12 => "ENOMEM",
        13 => "EACCES",
        14 => "EFAULT",
        16 => "EBUSY",
        19 => "ENODEV",
        22 => "EINVAL",
        28 => "ENOSPC",
        34 => "ERANGE",
        35 => "EDEADLK",
        95 => "EOPNOTSUPP",
        _ => return format!("errno{errno}"),
    };
    name.into()
}

fn check_text(check: Check) -> String {
    match check {
        Ok(()) => "ok".into(),
        Err(errno) => errno_name(errno),
    }
}

/// `n` choose `k`.
fn binomial(n: usize, k: usize) -> u128 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut value: u128 = 1;
    for i in 0..k {
        // Exact at every step: the product of i+1 consecutive integers is
        // divisible by (i+1)!.
        value = value * (n - i) as u128 / (i + 1) as u128;
    }
    value
}

/// The `rank`-th `k`-subset of `0..n` in lexicographic order.
fn unrank(n: usize, k: usize, mut rank: u128) -> Vec<usize> {
    let mut subset = Vec::with_capacity(k);
    let mut next = 0;
    for remaining in (1..=k).rev() {
        loop {
            let with_next = binomial(n - next - 1, remaining - 1);
            if rank < with_next {
                break;
            }
            rank -= with_next;
            next += 1;
        }
        subset.push(next);
        next += 1;
    }
    subset
}

/// The `k`-subsets of `0..n` to test: the first in lexicographic order, then
/// up to `extra` more at evenly spaced ranks (all of them if there are no
/// more than `extra`).
fn subsets(n: usize, k: usize, extra: usize) -> Vec<Vec<usize>> {
    let total = binomial(n, k);
    if total == 0 {
        return Vec::new();
    }
    let others = total - 1;
    let mut ranks = vec![0u128];
    if others <= extra as u128 {
        ranks.extend(1..total);
    } else {
        // others > extra, so the ranks are distinct and stay below total.
        ranks.extend((0..extra as u128).map(|i| 1 + i * others / extra as u128));
    }
    ranks.into_iter().map(|rank| unrank(n, k, rank)).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Zpos {
    Mutable { min: u64, max: u64 },
    Fixed(u64),
    Absent,
}

/// zpos values for a stack listed bottom first: distinct ascending values
/// for mutable planes, each at least its minimum and above every plane
/// below. `None` for planes whose zpos cannot be set. A value is clamped to
/// the plane's maximum (the kernel then decides whether the order is valid).
fn assign_zpos(stack: &[Zpos]) -> Vec<Option<u64>> {
    let mut next = 0u64;
    stack
        .iter()
        .map(|z| match *z {
            Zpos::Mutable { min, max } => {
                let value = next.max(min).min(max);
                next = value + 1;
                Some(value)
            }
            Zpos::Fixed(value) => {
                next = next.max(value + 1);
                None
            }
            Zpos::Absent => None,
        })
        .collect()
}

/// Source size (16.16) and destination size of one axis scaled by `factor`
/// (dst / src) on an output `size` pixels long.
fn scaled_axis(size: u32, factor: f64) -> (u64, u32) {
    if factor >= 1.0 {
        let src = (f64::from(size) * 65536.0 / factor).round() as u64;
        (src.max(1 << 16), size)
    } else {
        let dst = (f64::from(size) * factor).round() as u32;
        (u64::from(size) << 16, dst.max(1))
    }
}

/// Source (16.16) and destination rectangles of a layer scaled by `scale`.
fn scaled_rects(output: (u32, u32), scale: (f64, f64)) -> ((u64, u64), (u32, u32)) {
    let (src_w, dst_w) = scaled_axis(output.0, scale.0);
    let (src_h, dst_h) = scaled_axis(output.1, scale.1);
    ((src_w, src_h), (dst_w, dst_h))
}

fn pixels16(value: u64) -> String {
    format!("{:.4}", value as f64 / 65536.0)
}

fn combo_line(
    k: usize,
    set: usize,
    planes: &[u32],
    scale: (f64, f64),
    rects: ((u64, u64), (u32, u32)),
    result: Check,
) -> String {
    let ids: Vec<String> = planes.iter().map(u32::to_string).collect();
    let ((sw, sh), (dw, dh)) = rects;
    format!(
        "COMBO k={k} set={set} planes={} scale={:.4}x{:.4} src={}x{} dst={dw}x{dh} result={}",
        ids.join(","),
        scale.0,
        scale.1,
        pixels16(sw),
        pixels16(sh),
        check_text(result)
    )
}

fn combo_max_line(accepted_k: usize) -> String {
    format!("COMBO_MAX accepted_k={accepted_k}")
}

/// What became of a framebuffer for a modifier test.
#[derive(Debug)]
enum FbError {
    /// The sweep does not know the format's plane layout, or the drm crate
    /// has no name for it.
    UnsupportedFormat,
    /// The dumb buffer was refused.
    Dumb(i32),
    /// ADDFB2 was refused.
    AddFb(i32),
}

fn modifier_line(
    plane: u32,
    format: u32,
    modifier: u64,
    listed: bool,
    addfb: &Result<(), FbError>,
    test_only: Option<Check>,
) -> String {
    let addfb = match addfb {
        Ok(()) => "ok".into(),
        Err(FbError::UnsupportedFormat) => "unsupported-format".into(),
        Err(FbError::Dumb(errno)) => format!("dumb-{}", errno_name(*errno)),
        Err(FbError::AddFb(errno)) => errno_name(*errno),
    };
    format!(
        "MODIFIER plane={plane} format={} modifier={modifier:#018x} listed={} addfb={addfb} \
         test_only={}",
        fourcc_text(format),
        if listed { "yes" } else { "no" },
        test_only.map_or("not reached".into(), check_text)
    )
}

fn replace_line(first: Check, second: Option<Check>, events: usize) -> String {
    format!(
        "REPLACE first={} second={} events={events}",
        check_text(first),
        second.map_or("not reached".into(), check_text)
    )
}

/// Parses an `IN_FORMATS` blob (struct drm_format_modifier_blob) into
/// (format, modifier) pairs, in blob order.
fn parse_in_formats(blob: &[u8]) -> Vec<(u32, u64)> {
    let u32_at = |offset: usize| {
        blob.get(offset..offset + 4)
            .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
    };
    let u64_at = |offset: usize| {
        blob.get(offset..offset + 8)
            .map(|b| u64::from_ne_bytes(b.try_into().unwrap()))
    };
    let mut pairs = Vec::new();
    let (Some(count_formats), Some(formats_offset), Some(count_modifiers), Some(modifiers_offset)) =
        (u32_at(8), u32_at(12), u32_at(16), u32_at(20))
    else {
        return pairs;
    };
    for m in 0..count_modifiers as usize {
        let base = modifiers_offset as usize + m * 24;
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

/// The pairs to test on a plane: the listed ones, then each extra modifier
/// on each of the plane's formats where it is not listed, optionally only
/// for one format. `bool` says whether the pair is listed.
fn modifier_pairs(
    listed: &[(u32, u64)],
    formats: &[u32],
    extra: &[u64],
    only_format: Option<u32>,
) -> Vec<(u32, u64, bool)> {
    let mut pairs: Vec<(u32, u64, bool)> = Vec::new();
    let mut push = |format: u32, modifier: u64, is_listed: bool| {
        if only_format.is_some_and(|f| f != format) {
            return;
        }
        if !pairs.iter().any(|p| p.0 == format && p.1 == modifier) {
            pairs.push((format, modifier, is_listed));
        }
    };
    for (format, modifier) in listed {
        push(*format, *modifier, true);
    }
    for modifier in extra {
        for format in formats {
            push(*format, *modifier, false);
        }
    }
    pairs
}

/// One plane of a pixel format: bytes per block, block width in pixels and
/// the subsampling of the plane.
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

/// Plane layouts of the formats the sweep can build a buffer for, after
/// drm_format_info in drivers/gpu/drm/drm_fourcc.c (*read from source*).
fn format_planes(code: u32) -> Option<Vec<FormatPlane>> {
    let text = fourcc_text(code);
    let planes = match text.as_str() {
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
        "NV20" => vec![sub(5, 4, 1, 1), sub(5, 2, 2, 1)],
        "NV30" => vec![sub(5, 4, 1, 1), sub(5, 2, 1, 1)],
        "P010" | "P012" | "P016" => vec![packed(2), sub(4, 1, 2, 2)],
        "P210" => vec![packed(2), sub(4, 1, 2, 1)],
        "P030" => vec![sub(4, 3, 1, 1), sub(8, 3, 2, 2)],
        "YU12" | "YV12" => vec![packed(1), sub(1, 1, 2, 2), sub(1, 1, 2, 2)],
        "YU16" | "YV16" => vec![packed(1), sub(1, 1, 2, 1), sub(1, 1, 2, 1)],
        "YU24" | "YV24" => vec![packed(1), packed(1), packed(1)],
        _ => return None,
    };
    Some(planes)
}

/// Pitches, offsets and total size of a buffer for `planes` at `size`.
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
/// starts at a 4 KiB boundary and the rows are padded to 16, so the pitch of
/// a 32-bit plane is the 16-pixel-aligned width times four (what AFBC wants)
/// and tiled layouts have whole tile rows. A quarter more plus 256 KiB holds
/// AFBC headers.
fn fb_layout(planes: &[FormatPlane], size: (u32, u32)) -> Option<FbLayout> {
    if planes.is_empty() || planes.len() > 4 {
        return None;
    }
    let width = round_up(u64::from(size.0), 16);
    let height = round_up(u64::from(size.1), 16);
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

type Props = HashMap<String, (property::Handle, u64)>;

fn named_properties<H: ResourceHandle>(card: &Card, handle: H) -> io::Result<Props> {
    let set = card.get_properties(handle)?;
    let mut named = HashMap::new();
    for (prop, value) in set.iter() {
        let info = card.get_property(*prop)?;
        named.insert(info.name().to_string_lossy().into_owned(), (*prop, *value));
    }
    Ok(named)
}

/// The property handle of an object, or an error naming what is missing.
fn prop(props: &Props, name: &str) -> io::Result<property::Handle> {
    props
        .get(name)
        .map(|(handle, _)| *handle)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no property {name}")))
}

/// Prefixes an I/O error with what was being done.
fn fail(what: &str) -> impl Fn(io::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

fn errno_of(error: &io::Error) -> i32 {
    error.raw_os_error().unwrap_or(-1)
}

struct PlaneInfo {
    handle: plane::Handle,
    kind: PlaneType,
    formats: Vec<u32>,
    /// `IN_FORMATS` pairs, `None` without the property.
    in_formats: Option<Vec<(u32, u64)>>,
    possible: bool,
    on_crtc: bool,
    zpos: Zpos,
    zpos_prop: Option<property::Handle>,
    fb_id: property::Handle,
    crtc_id: property::Handle,
    src: [property::Handle; 4],
    dst: [property::Handle; 4],
}

impl PlaneInfo {
    fn id(&self) -> u32 {
        u32::from(self.handle)
    }

    fn lists(&self, format: DrmFourcc) -> bool {
        self.formats.contains(&(format as u32))
    }
}

/// A dumb buffer and the framebuffer made from it.
struct Fb {
    dumb: DumbBuffer,
    fb: framebuffer::Handle,
}

struct Layout {
    size: (u32, u32),
    format: DrmFourcc,
    modifier: Option<DrmModifier>,
    pitches: [u32; 4],
    offsets: [u32; 4],
    handles: [Option<drm::buffer::Handle>; 4],
}

impl PlanarBuffer for Layout {
    fn size(&self) -> (u32, u32) {
        self.size
    }
    fn format(&self) -> DrmFourcc {
        self.format
    }
    fn modifier(&self) -> Option<DrmModifier> {
        self.modifier
    }
    fn pitches(&self) -> [u32; 4] {
        self.pitches
    }
    fn handles(&self) -> [Option<drm::buffer::Handle>; 4] {
        self.handles
    }
    fn offsets(&self) -> [u32; 4] {
        self.offsets
    }
}

/// A framebuffer of `format` at `size` on one dumb buffer, ADDFB2 with
/// `modifier` (`None`: without the modifiers flag).
fn make_fb(
    card: &Card,
    size: (u32, u32),
    format: u32,
    modifier: Option<u64>,
) -> Result<Fb, FbError> {
    let fourcc = DrmFourcc::try_from(format).map_err(|_| FbError::UnsupportedFormat)?;
    let planes = format_planes(format).ok_or(FbError::UnsupportedFormat)?;
    let layout = fb_layout(&planes, size).ok_or(FbError::UnsupportedFormat)?;
    // One 32 bpp dumb buffer as wide as the widest pitch and tall enough
    // for the whole allocation; the format's planes are offsets in it.
    let widest = layout.pitches.iter().copied().max().unwrap_or(0);
    let dumb_size = (
        widest / 4,
        u32::try_from(layout.alloc.div_ceil(u64::from(widest))).map_err(|_| FbError::Dumb(-1))?,
    );
    let dumb = card
        .create_dumb_buffer(dumb_size, DrmFourcc::Xrgb8888, 32)
        .map_err(|e| FbError::Dumb(errno_of(&e)))?;
    let handle = drm::buffer::Buffer::handle(&dumb);
    let mut handles = [None; 4];
    for h in handles.iter_mut().take(layout.count) {
        *h = Some(handle);
    }
    // The drm crate drops an INVALID modifier and then insists on no
    // modifiers flag, so that value goes without it.
    let modifier = modifier.filter(|m| *m != MOD_INVALID);
    let buffer = Layout {
        size,
        format: fourcc,
        modifier: modifier.map(DrmModifier::from),
        pitches: layout.pitches,
        offsets: layout.offsets,
        handles,
    };
    let flags = if modifier.is_some() {
        FbCmd2Flags::MODIFIERS
    } else {
        FbCmd2Flags::empty()
    };
    match card.add_planar_framebuffer(&buffer, flags) {
        Ok(fb) => Ok(Fb { dumb, fb }),
        Err(error) => {
            let _ = card.destroy_dumb_buffer(dumb);
            Err(FbError::AddFb(errno_of(&error)))
        }
    }
}

fn release(card: &Card, fb: Fb) {
    let _ = card.destroy_framebuffer(fb.fb);
    let _ = card.destroy_dumb_buffer(fb.dumb);
}

/// Fills a 32-bit dumb buffer with one pixel value (little-endian bytes).
fn fill(card: &Card, fb: &mut Fb, pixel: [u8; 4]) -> io::Result<()> {
    let mut mapping = card.map_dumb_buffer(&mut fb.dumb)?;
    for chunk in mapping.as_mut().chunks_exact_mut(4) {
        chunk.copy_from_slice(&pixel);
    }
    Ok(())
}

/// The output, its CRTC and the planes that can reach it.
struct Ctx {
    card: Card,
    crtc: crtc::Handle,
    connector: connector::Handle,
    connector_crtc_id: property::Handle,
    crtc_mode_id: property::Handle,
    crtc_active: property::Handle,
    mode_blob: u64,
    output: (u32, u32),
    /// Candidate planes (non-cursor, reach the CRTC), by id.
    candidates: Vec<PlaneInfo>,
    /// Index of the primary plane in `candidates`.
    primary: usize,
}

/// Where a plane goes in a request.
struct Placement {
    fb: framebuffer::Handle,
    src: (u64, u64),
    dst: (u32, u32),
    zpos: Option<u64>,
}

impl Ctx {
    fn primary(&self) -> &PlaneInfo {
        &self.candidates[self.primary]
    }

    /// A request that sets the mode on the CRTC.
    fn modeset_request(&self) -> AtomicModeReq {
        let mut req = AtomicModeReq::new();
        req.add_property(
            self.connector,
            self.connector_crtc_id,
            property::Value::CRTC(Some(self.crtc)),
        );
        req.add_property(
            self.crtc,
            self.crtc_mode_id,
            property::Value::Blob(self.mode_blob),
        );
        req.add_property(self.crtc, self.crtc_active, property::Value::Boolean(true));
        req
    }

    /// Switches off, in `req`, every candidate not in `used`.
    fn disable_others(&self, req: &mut AtomicModeReq, used: &[plane::Handle]) {
        for plane in &self.candidates {
            if !used.contains(&plane.handle) {
                req.add_property(
                    plane.handle,
                    plane.fb_id,
                    property::Value::Framebuffer(None),
                );
                req.add_property(plane.handle, plane.crtc_id, property::Value::CRTC(None));
            }
        }
    }

    fn place(&self, req: &mut AtomicModeReq, plane: &PlaneInfo, at: &Placement) {
        let h = plane.handle;
        req.add_property(h, plane.fb_id, property::Value::Framebuffer(Some(at.fb)));
        req.add_property(h, plane.crtc_id, property::Value::CRTC(Some(self.crtc)));
        let source = [0, 0, at.src.0, at.src.1];
        for (p, v) in plane.src.iter().zip(source) {
            req.add_property(h, *p, property::Value::UnsignedRange(v));
        }
        req.add_property(h, plane.dst[0], property::Value::SignedRange(0));
        req.add_property(h, plane.dst[1], property::Value::SignedRange(0));
        req.add_property(
            h,
            plane.dst[2],
            property::Value::UnsignedRange(u64::from(at.dst.0)),
        );
        req.add_property(
            h,
            plane.dst[3],
            property::Value::UnsignedRange(u64::from(at.dst.1)),
        );
        if let (Some(zpos), Some(handle)) = (at.zpos, plane.zpos_prop) {
            req.add_property(h, handle, property::Value::UnsignedRange(zpos));
        }
    }

    fn test(&self, req: AtomicModeReq) -> Check {
        self.card
            .atomic_commit(
                AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::ALLOW_MODESET,
                req,
            )
            .map_err(|e| errno_of(&e))
    }

    fn full(&self, fb: framebuffer::Handle) -> Placement {
        Placement {
            fb,
            src: (
                u64::from(self.output.0) << 16,
                u64::from(self.output.1) << 16,
            ),
            dst: self.output,
            zpos: None,
        }
    }
}

fn open(options: &Options) -> Result<Ctx, String> {
    println!("DATE {}", utc_now());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    println!("KERNEL {kernel}");
    println!("DEVICE {}", options.device);

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&options.device)
        .map_err(|e| format!("cannot open {}: {e}", options.device))?;
    let card = Card::from_fd(file.into());
    match card.get_driver() {
        Ok(info) => println!(
            "DRIVER {} version {}.{}.{} date {} desc {}",
            info.name().to_string_lossy(),
            info.version.0,
            info.version.1,
            info.version.2,
            info.date().to_string_lossy(),
            info.description().to_string_lossy()
        ),
        Err(_) => println!("DRIVER unknown"),
    }
    card.set_client_capability(ClientCapability::UniversalPlanes, true)
        .map_err(fail("universal planes"))?;
    card.set_client_capability(ClientCapability::Atomic, true)
        .map_err(fail("atomic"))?;
    card.acquire_master_lock().map_err(|e| {
        format!(
            "DRM master is unavailable: {e}. Another process (a desktop or compositor) probably \
             holds it; stop it for the run. This sweep never stops other processes"
        )
    })?;

    // Connector, preferred mode, CRTC.
    let resources = card.resource_handles().map_err(fail("resources"))?;
    let mut chosen = None;
    for handle in resources.connectors() {
        if options.connector.is_some_and(|id| id != u32::from(*handle)) {
            continue;
        }
        let info = card
            .get_connector(*handle, false)
            .map_err(fail("connector"))?;
        if info.state() == connector::State::Connected && !info.modes().is_empty() {
            chosen = Some((*handle, info));
            break;
        }
    }
    let (connector, info) = chosen.ok_or("no connected connector with modes")?;
    let mode = info
        .modes()
        .iter()
        .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
        .or_else(|| info.modes().first())
        .copied()
        .ok_or("no mode")?;
    let mut crtc = None;
    if let Some(encoder) = info.current_encoder() {
        crtc = card.get_encoder(encoder).map_err(fail("encoder"))?.crtc();
    }
    if crtc.is_none() {
        for encoder in info.encoders() {
            let possible = card
                .get_encoder(*encoder)
                .map_err(fail("encoder"))?
                .possible_crtcs();
            crtc = resources.filter_crtcs(possible).into_iter().next();
            if crtc.is_some() {
                break;
            }
        }
    }
    let crtc = crtc.ok_or("no CRTC for the connector")?;
    let (mw, mh) = mode.size();
    let output = (u32::from(mw), u32::from(mh));
    println!(
        "MODE {}x{}@{}Hz connector {} crtc {}",
        output.0,
        output.1,
        mode.vrefresh(),
        u32::from(connector),
        u32::from(crtc)
    );

    let connector_props = named_properties(&card, connector).map_err(fail("connector props"))?;
    let crtc_props = named_properties(&card, crtc).map_err(fail("crtc props"))?;

    // Planes.
    let mut candidates = Vec::new();
    let mut handles = card.plane_handles().map_err(fail("planes"))?;
    handles.sort_by_key(|h| u32::from(*h));
    for handle in handles {
        let pinfo = card.get_plane(handle).map_err(fail("plane"))?;
        let props = named_properties(&card, handle).map_err(fail("plane props"))?;
        let kind = match props.get("type").map(|(_, v)| *v as u32) {
            Some(v) if v == PlaneType::Primary as u32 => PlaneType::Primary,
            Some(v) if v == PlaneType::Cursor as u32 => PlaneType::Cursor,
            _ => PlaneType::Overlay,
        };
        let in_formats = match props.get("IN_FORMATS") {
            Some((_, blob)) if *blob != 0 => Some(parse_in_formats(
                &card
                    .get_property_blob(*blob)
                    .map_err(fail("IN_FORMATS blob"))?,
            )),
            _ => None,
        };
        let (zpos, zpos_prop) = match props.get("zpos") {
            Some((handle, value)) => {
                let info = card.get_property(*handle).map_err(fail("zpos"))?;
                match info.value_type() {
                    property::ValueType::UnsignedRange(min, max) if info.mutable() => (
                        Zpos::Mutable {
                            min: *min,
                            max: *max,
                        },
                        Some(*handle),
                    ),
                    _ => (Zpos::Fixed(*value), None),
                }
            }
            None => (Zpos::Absent, None),
        };
        let possible = resources
            .filter_crtcs(pinfo.possible_crtcs())
            .contains(&crtc);
        let zpos_text = match zpos {
            Zpos::Mutable { min, max } => format!("{min}..{max}"),
            Zpos::Fixed(v) => format!("fixed {v}"),
            Zpos::Absent => "none".into(),
        };
        println!(
            "PLANE id {} {:?} possible_on_crtc {possible} zpos {zpos_text} in_formats {} formats {}",
            u32::from(handle),
            kind,
            in_formats
                .as_ref()
                .map_or("none".into(), |p| format!("{} pairs", p.len())),
            pinfo
                .formats()
                .iter()
                .map(|f| fourcc_text(*f))
                .collect::<Vec<_>>()
                .join(",")
        );
        if !possible || kind == PlaneType::Cursor {
            continue;
        }
        let names = |p: [&str; 4]| -> io::Result<[property::Handle; 4]> {
            Ok([
                prop(&props, p[0])?,
                prop(&props, p[1])?,
                prop(&props, p[2])?,
                prop(&props, p[3])?,
            ])
        };
        candidates.push(PlaneInfo {
            handle,
            kind,
            formats: pinfo.formats().to_vec(),
            in_formats,
            possible,
            on_crtc: pinfo.crtc() == Some(crtc),
            zpos,
            zpos_prop,
            fb_id: prop(&props, "FB_ID").map_err(fail("plane"))?,
            crtc_id: prop(&props, "CRTC_ID").map_err(fail("plane"))?,
            src: names(["SRC_X", "SRC_Y", "SRC_W", "SRC_H"]).map_err(fail("plane"))?,
            dst: names(["CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H"]).map_err(fail("plane"))?,
        });
    }
    let primaries = || {
        candidates
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind == PlaneType::Primary && p.possible)
    };
    let primary = primaries()
        .find(|(_, p)| p.on_crtc)
        .or_else(|| primaries().next())
        .map(|(i, _)| i)
        .ok_or("no primary plane can drive the CRTC")?;
    println!(
        "CANDIDATES {} primary {}",
        candidates
            .iter()
            .map(|p| p.id().to_string())
            .collect::<Vec<_>>()
            .join(","),
        candidates[primary].id()
    );

    let mode_blob = match card
        .create_property_blob(&mode)
        .map_err(fail("mode blob"))?
    {
        property::Value::Blob(id) => id,
        _ => return Err("mode blob".into()),
    };
    Ok(Ctx {
        card,
        crtc,
        connector,
        connector_crtc_id: prop(&connector_props, "CRTC_ID").map_err(fail("connector"))?,
        crtc_mode_id: prop(&crtc_props, "MODE_ID").map_err(fail("crtc"))?,
        crtc_active: prop(&crtc_props, "ACTIVE").map_err(fail("crtc"))?,
        mode_blob,
        output,
        candidates,
        primary,
    })
}

/// The first of `preference` the plane lists.
fn pick(plane: &PlaneInfo, preference: [DrmFourcc; 2]) -> Option<DrmFourcc> {
    preference.into_iter().find(|f| plane.lists(*f))
}

fn combos(ctx: &Ctx, options: &Options, buffers: &mut Vec<Fb>) -> Result<(), String> {
    let card = &ctx.card;
    let mut fbs: HashMap<u32, framebuffer::Handle> = HashMap::new();
    let mut fb_for = |format: DrmFourcc, buffers: &mut Vec<Fb>| -> Result<_, String> {
        if let Some(fb) = fbs.get(&(format as u32)) {
            return Ok(*fb);
        }
        let fb = make_fb(card, ctx.output, format as u32, None)
            .map_err(|e| format!("full-screen {format:?} buffer: {e:?}"))?;
        let handle = fb.fb;
        buffers.push(fb);
        fbs.insert(format as u32, handle);
        Ok(handle)
    };
    // (plane, format) for the primary and every usable other candidate.
    let primary = ctx.primary();
    let primary_format = pick(primary, [DrmFourcc::Xrgb8888, DrmFourcc::Argb8888])
        .ok_or("the primary plane lists neither XRGB8888 nor ARGB8888")?;
    let mut others = Vec::new();
    for (i, plane) in ctx.candidates.iter().enumerate() {
        if i == ctx.primary {
            continue;
        }
        match pick(plane, [DrmFourcc::Argb8888, DrmFourcc::Xrgb8888]) {
            Some(format) => others.push((plane, format)),
            None => println!(
                "NOTE plane={} lists neither ARGB8888 nor XRGB8888; left out",
                plane.id()
            ),
        }
    }
    let available = others.len() + 1;
    let max = options.max.unwrap_or(available);
    if max > available {
        println!("NOTE --max {max} is more than the {available} usable planes");
    }
    let max = max.min(available);
    let rects = scaled_rects(ctx.output, options.scale);
    let mut accepted_k = 0;
    for k in 1..=max {
        for (set, subset) in subsets(others.len(), k - 1, options.sets)
            .into_iter()
            .enumerate()
        {
            let mut stack = vec![(primary, primary_format)];
            stack.extend(subset.iter().map(|&i| others[i]));
            let zpos = assign_zpos(&stack.iter().map(|(p, _)| p.zpos).collect::<Vec<_>>());
            let used: Vec<plane::Handle> = stack.iter().map(|(p, _)| p.handle).collect();
            let mut req = ctx.modeset_request();
            ctx.disable_others(&mut req, &used);
            for ((plane, format), zpos) in stack.iter().zip(zpos) {
                let fb = fb_for(*format, buffers)?;
                let (src, dst) = if options.unscaled_primary && plane.handle == primary.handle {
                    scaled_rects(ctx.output, (1.0, 1.0))
                } else {
                    rects
                };
                ctx.place(&mut req, plane, &Placement { fb, src, dst, zpos });
            }
            let result = ctx.test(req);
            if set == 0 && result.is_ok() {
                accepted_k = k;
            }
            let ids: Vec<u32> = stack.iter().map(|(p, _)| p.id()).collect();
            println!("{}", combo_line(k, set, &ids, options.scale, rects, result));
        }
    }
    println!("{}", combo_max_line(accepted_k));
    Ok(())
}

fn modifiers(ctx: &Ctx, options: &Options, buffers: &mut Vec<Fb>) -> Result<(), String> {
    let card = &ctx.card;
    let size = options.size.unwrap_or(ctx.output);
    if size.0 > ctx.output.0 || size.1 > ctx.output.1 {
        return Err(format!(
            "--size {}x{} is larger than the output",
            size.0, size.1
        ));
    }
    if let Some(id) = options.plane {
        if !ctx.candidates.iter().any(|p| p.id() == id) {
            return Err(format!("plane {id} is not a non-cursor plane of the CRTC"));
        }
    }
    let primary = ctx.primary();
    let background_format = pick(primary, [DrmFourcc::Xrgb8888, DrmFourcc::Argb8888])
        .ok_or("the primary plane lists neither XRGB8888 nor ARGB8888")?;
    let background = make_fb(card, ctx.output, background_format as u32, None)
        .map_err(|e| format!("background buffer: {e:?}"))?;
    let background_fb = background.fb;
    buffers.push(background);
    for plane in &ctx.candidates {
        if options.plane.is_some_and(|id| id != plane.id()) {
            continue;
        }
        let listed: Vec<(u32, u64)> = match &plane.in_formats {
            Some(pairs) => pairs.clone(),
            None => {
                println!(
                    "NOTE plane={} has no IN_FORMATS; its formats are tested with the linear \
                     modifier, listed=no",
                    plane.id()
                );
                Vec::new()
            }
        };
        let mut extra = options.modifiers.clone();
        if plane.in_formats.is_none() && !extra.contains(&0) {
            extra.insert(0, 0);
        }
        let is_primary = plane.handle == primary.handle;
        for (format, modifier, is_listed) in
            modifier_pairs(&listed, &plane.formats, &extra, options.format)
        {
            let (addfb, test_only) = match make_fb(card, size, format, Some(modifier)) {
                Ok(fb) => {
                    let mut req = ctx.modeset_request();
                    let mut used = vec![plane.handle];
                    if !is_primary {
                        used.push(primary.handle);
                        ctx.place(&mut req, primary, &ctx.full(background_fb));
                    }
                    ctx.disable_others(&mut req, &used);
                    let at = Placement {
                        fb: fb.fb,
                        src: (u64::from(size.0) << 16, u64::from(size.1) << 16),
                        dst: size,
                        zpos: None,
                    };
                    // Above the background where the zpos can be set.
                    let zpos = if is_primary {
                        vec![primary.zpos]
                    } else {
                        vec![primary.zpos, plane.zpos]
                    };
                    let zpos = assign_zpos(&zpos);
                    if !is_primary {
                        if let (Some(z), Some(handle)) = (zpos[0], primary.zpos_prop) {
                            req.add_property(
                                primary.handle,
                                handle,
                                property::Value::UnsignedRange(z),
                            );
                        }
                    }
                    let at = Placement {
                        zpos: *zpos.last().unwrap_or(&None),
                        ..at
                    };
                    ctx.place(&mut req, plane, &at);
                    let check = ctx.test(req);
                    release(card, fb);
                    (Ok(()), Some(check))
                }
                Err(error) => (Err(error), None),
            };
            println!(
                "{}",
                modifier_line(plane.id(), format, modifier, is_listed, &addfb, test_only)
            );
        }
    }
    Ok(())
}

/// Page-flip events of the CRTC that arrive before `deadline`, stopping
/// early once `want` have arrived.
fn wait_flips(ctx: &Ctx, want: usize, deadline: Instant) -> Result<usize, String> {
    use rustix::event::{poll, PollFd, PollFlags, Timespec};
    let mut seen = 0;
    while seen < want {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let timeout = Timespec {
            tv_sec: left.as_secs() as _,
            tv_nsec: left.subsec_nanos() as _,
        };
        let mut fds = [PollFd::new(&ctx.card, PollFlags::IN)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(format!("poll: {error}")),
        }
        if !fds[0].revents().contains(PollFlags::IN) {
            continue;
        }
        for event in ctx.card.receive_events().map_err(fail("events"))? {
            if let Event::PageFlip(flip) = event {
                if flip.crtc == ctx.crtc {
                    seen += 1;
                }
            }
        }
    }
    Ok(seen)
}

fn replace(ctx: &Ctx, options: &Options, buffers: &mut Vec<Fb>) -> Result<(), String> {
    let card = &ctx.card;
    let primary = ctx.primary();
    let format = pick(primary, [DrmFourcc::Xrgb8888, DrmFourcc::Argb8888])
        .ok_or("the primary plane lists neither XRGB8888 nor ARGB8888")?;
    let mut frames = Vec::new();
    // Grey, then dark blue (B, G, R, X/A bytes).
    for pixel in [[0x80, 0x80, 0x80, 0xff], [0x60, 0x20, 0x20, 0xff]] {
        let mut fb = make_fb(card, ctx.output, format as u32, None)
            .map_err(|e| format!("frame buffer: {e:?}"))?;
        let filled = fill(card, &mut fb, pixel);
        frames.push(fb.fb);
        buffers.push(fb);
        filled.map_err(fail("fill"))?;
    }
    let wait = Duration::from_millis(u64::from(options.wait_ms));
    let flip = |fb| {
        let mut req = AtomicModeReq::new();
        req.add_property(
            primary.handle,
            primary.fb_id,
            property::Value::Framebuffer(Some(fb)),
        );
        req
    };

    let mut setup = ctx.modeset_request();
    ctx.disable_others(&mut setup, &[primary.handle]);
    ctx.place(&mut setup, primary, &ctx.full(frames[0]));
    card.atomic_commit(
        AtomicCommitFlags::ALLOW_MODESET | AtomicCommitFlags::PAGE_FLIP_EVENT,
        setup,
    )
    .map_err(|e| format!("setup commit: {}", errno_name(errno_of(&e))))?;
    let events = wait_flips(ctx, 1, Instant::now() + wait)?;
    println!("SETUP commit=ok events={events}");

    let flags = AtomicCommitFlags::NONBLOCK | AtomicCommitFlags::PAGE_FLIP_EVENT;
    let first = card
        .atomic_commit(flags, flip(frames[1]))
        .map_err(|e| errno_of(&e));
    let second = first.ok().map(|()| {
        card.atomic_commit(flags, flip(frames[0]))
            .map_err(|e| errno_of(&e))
    });
    let want = usize::from(first.is_ok()) + usize::from(second.is_some_and(|s| s.is_ok()));
    let events = wait_flips(ctx, want, Instant::now() + wait)?;
    println!("{}", replace_line(first, second, events));
    println!(
        "NOTE the sweep's framebuffers are removed on exit; the kernel then switches the \
         planes showing them off, and the output stays dark until the next client commits"
    );
    Ok(())
}

fn run(options: &Options) -> Result<(), String> {
    let ctx = open(options)?;
    let mut buffers = Vec::new();
    let result = match options.mode {
        Mode::Combos => combos(&ctx, options, &mut buffers),
        Mode::Modifiers => modifiers(&ctx, options, &mut buffers),
        Mode::Replace => replace(&ctx, options, &mut buffers),
    };
    for fb in buffers {
        release(&ctx.card, fb);
    }
    let _ = ctx.card.destroy_property_blob(ctx.mode_blob);
    result
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
    match run(&options) {
        Ok(()) => {
            println!("VERDICT kms_sweep: DONE {}", options.mode.name());
            ExitCode::SUCCESS
        }
        Err(message) => {
            println!("VERDICT kms_sweep: FAIL {message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Result<Options, String> {
        parse_args(text.split_whitespace().map(String::from))
    }

    #[test]
    fn parses_modes_and_defaults() {
        let o = args("combos").unwrap();
        assert_eq!(o.mode, Mode::Combos);
        assert_eq!(o.device, "/dev/dri/card0");
        assert_eq!((o.max, o.sets, o.scale), (None, 32, (1.0, 1.0)));
        assert_eq!(args("modifiers").unwrap().mode, Mode::Modifiers);
        let o = args("replace /dev/dri/card1").unwrap();
        assert_eq!(
            (o.mode, o.device.as_str(), o.wait_ms),
            (Mode::Replace, "/dev/dri/card1", 1000)
        );
    }

    #[test]
    fn parses_mode_options() {
        let o = args(
            "combos --max 8 --sets 0 --scale-h 0.5 --scale-v 2 --unscaled-primary --connector 7",
        )
        .unwrap();
        assert_eq!(o.max, Some(8));
        assert_eq!(o.sets, 0);
        assert_eq!(o.scale, (0.5, 2.0));
        assert!(o.unscaled_primary);
        assert_eq!(o.connector, Some(7));
        let o = args(
            "modifiers --modifier sand128 --modifier 0x0700000000000001 --modifier sand128 \
                      --plane 42 --format NV12 --size 640x480",
        )
        .unwrap();
        assert_eq!(o.modifiers, [0x0700_0000_0000_0004, 0x0700_0000_0000_0001]);
        assert_eq!(o.plane, Some(42));
        assert_eq!(o.format, Some(u32::from_le_bytes(*b"NV12")));
        assert_eq!(o.size, Some((640, 480)));
        assert_eq!(args("replace --wait-ms 250").unwrap().wait_ms, 250);
    }

    #[test]
    fn rejects_bad_arguments() {
        assert_eq!(args(""), Err(String::new()));
        assert_eq!(args("--help"), Err(String::new()));
        assert_eq!(args("combos -h"), Err(String::new()));
        assert!(args("sweep").unwrap_err().contains("unknown mode"));
        assert!(args("combos --bogus")
            .unwrap_err()
            .contains("unknown option"));
        assert!(args("combos --max").unwrap_err().contains("needs a value"));
        assert!(args("combos --max 0").is_err());
        assert!(args("combos --scale-h 0").is_err());
        assert!(args("combos --scale-h -1").is_err());
        assert!(args("combos --scale-v inf").is_err());
        assert!(args("modifiers --size 0x10").is_err());
        assert!(args("modifiers --size 10").is_err());
        assert!(args("modifiers --modifier tiled").is_err());
        assert_eq!(
            args("replace --max 2").unwrap_err(),
            "--max is only for combos"
        );
        assert_eq!(
            args("combos --modifier linear").unwrap_err(),
            "--modifier is only for modifiers"
        );
        assert_eq!(
            args("modifiers --wait-ms 5").unwrap_err(),
            "--wait-ms is only for replace"
        );
    }

    #[test]
    fn modifier_names_and_hex() {
        assert_eq!(parse_modifier("linear"), Ok(0));
        assert_eq!(parse_modifier("vc4-t-tiled"), Ok(0x0700_0000_0000_0001));
        assert_eq!(parse_modifier("sand64"), Ok(0x0700_0000_0000_0003));
        assert_eq!(parse_modifier("sand128"), Ok(0x0700_0000_0000_0004));
        assert_eq!(parse_modifier("sand256"), Ok(0x0700_0000_0000_0005));
        assert_eq!(parse_modifier("afbc-16x16"), Ok(0x0800_0000_0000_0001));
        assert_eq!(
            parse_modifier("0x0700000000000004"),
            Ok(0x0700_0000_0000_0004)
        );
        assert_eq!(parse_modifier("0X10"), Ok(0x10));
        assert_eq!(parse_modifier("ff"), Ok(0xff));
        assert!(parse_modifier("0x").is_err());
        assert!(parse_modifier("0x10000000000000000").is_err());
        assert!(parse_modifier("0xzz").is_err());
    }

    #[test]
    fn fourcc_parsing_and_text() {
        assert_eq!(parse_fourcc("XR24"), Ok(DrmFourcc::Xrgb8888 as u32));
        assert_eq!(parse_fourcc("R8"), Ok(u32::from_le_bytes(*b"R8  ")));
        assert_eq!(parse_fourcc("0x34325258"), Ok(DrmFourcc::Xrgb8888 as u32));
        assert!(parse_fourcc("").is_err());
        assert!(parse_fourcc("NV122").is_err());
        assert_eq!(fourcc_text(u32::from_le_bytes(*b"R8  ")), "R8");
        assert_eq!(fourcc_text(DrmFourcc::Nv12 as u32), "NV12");
        assert_eq!(fourcc_text(0x0102_0304), "????");
    }

    #[test]
    fn errno_names() {
        assert_eq!(errno_name(22), "EINVAL");
        assert_eq!(errno_name(16), "EBUSY");
        assert_eq!(errno_name(28), "ENOSPC");
        assert_eq!(errno_name(34), "ERANGE");
        assert_eq!(errno_name(200), "errno200");
        assert_eq!(check_text(Ok(())), "ok");
        assert_eq!(check_text(Err(22)), "EINVAL");
    }

    #[test]
    fn binomials() {
        assert_eq!(binomial(5, 0), 1);
        assert_eq!(binomial(5, 2), 10);
        assert_eq!(binomial(5, 5), 1);
        assert_eq!(binomial(3, 4), 0);
        assert_eq!(binomial(0, 0), 1);
        assert_eq!(binomial(64, 32), 1_832_624_140_942_590_534);
    }

    #[test]
    fn unrank_is_lexicographic() {
        let all: Vec<Vec<usize>> = (0..binomial(4, 2)).map(|r| unrank(4, 2, r)).collect();
        assert_eq!(
            all,
            [[0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3]].map(|s| s.to_vec())
        );
        assert_eq!(unrank(5, 0, 0), Vec::<usize>::new());
        assert_eq!(unrank(7, 7, 0), (0..7).collect::<Vec<_>>());
    }

    #[test]
    fn subsets_first_then_spread() {
        // Few enough: every subset, first in id order.
        assert_eq!(
            subsets(3, 2, 32),
            [[0, 1], [0, 2], [1, 2]].map(|s| s.to_vec())
        );
        // k = 0: only the empty set (the primary alone).
        assert_eq!(subsets(7, 0, 32), [Vec::<usize>::new()]);
        assert!(subsets(2, 3, 32).is_empty());
        // --sets 0: only the first.
        assert_eq!(subsets(7, 3, 0), [vec![0, 1, 2]]);
        // Many: the first plus `extra` distinct ones, deterministic.
        let picked = subsets(17, 8, 32);
        assert_eq!(picked.len(), 33);
        assert_eq!(picked[0], (0..8).collect::<Vec<_>>());
        let mut unique = picked.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 33);
        assert!(picked
            .iter()
            .all(|s| s.len() == 8 && s.windows(2).all(|w| w[0] < w[1])));
        assert!(picked.iter().all(|s| s.iter().all(|&i| i < 17)));
        assert_eq!(picked, subsets(17, 8, 32));
    }

    #[test]
    fn zpos_distinct_and_ascending() {
        let m = |min, max| Zpos::Mutable { min, max };
        assert_eq!(
            assign_zpos(&[m(0, 7), m(0, 7), m(0, 7)]),
            [Some(0), Some(1), Some(2)]
        );
        // A primary fixed at 0 below mutable overlays starting at 1.
        assert_eq!(
            assign_zpos(&[Zpos::Fixed(0), m(1, 3), m(1, 3)]),
            [None, Some(1), Some(2)]
        );
        // Minimums are respected; the maximum clamps.
        assert_eq!(assign_zpos(&[m(2, 5), m(0, 2)]), [Some(2), Some(2)]);
        assert_eq!(assign_zpos(&[Zpos::Absent, m(0, 7)]), [None, Some(0)]);
    }

    #[test]
    fn scaling_keeps_buffer_within_the_output() {
        let out = (1920, 1080);
        assert_eq!(
            scaled_rects(out, (1.0, 1.0)),
            ((1920 << 16, 1080 << 16), (1920, 1080))
        );
        // Upscale: full-screen destination, smaller source.
        assert_eq!(
            scaled_rects(out, (2.0, 4.0)),
            ((960 << 16, 270 << 16), (1920, 1080))
        );
        // Downscale: full source, smaller destination.
        assert_eq!(
            scaled_rects(out, (0.5, 0.25)),
            ((1920 << 16, 1080 << 16), (960, 270))
        );
        // Fractions of a source pixel are kept in 16.16.
        assert_eq!(scaled_axis(1000, 3.0).0, 21_845_333);
        // Never below one pixel.
        assert_eq!(scaled_axis(100, 1000.0), (1 << 16, 100));
        assert_eq!(scaled_axis(100, 0.0001), (100 << 16, 1));
    }

    #[test]
    fn lines() {
        let rects = scaled_rects((1920, 1080), (1.0, 1.0));
        assert_eq!(
            combo_line(2, 0, &[31, 37], (1.0, 1.0), rects, Ok(())),
            "COMBO k=2 set=0 planes=31,37 scale=1.0000x1.0000 src=1920.0000x1080.0000 \
             dst=1920x1080 result=ok"
        );
        assert_eq!(
            combo_line(
                1,
                3,
                &[31],
                (0.5, 2.0),
                scaled_rects((100, 100), (0.5, 2.0)),
                Err(22)
            ),
            "COMBO k=1 set=3 planes=31 scale=0.5000x2.0000 src=100.0000x50.0000 dst=50x100 \
             result=EINVAL"
        );
        assert_eq!(combo_max_line(0), "COMBO_MAX accepted_k=0");
        let nv12 = DrmFourcc::Nv12 as u32;
        assert_eq!(
            modifier_line(
                31,
                nv12,
                0x0700_0000_0000_0004,
                true,
                &Ok(()),
                Some(Err(22))
            ),
            "MODIFIER plane=31 format=NV12 modifier=0x0700000000000004 listed=yes addfb=ok \
             test_only=EINVAL"
        );
        assert_eq!(
            modifier_line(31, nv12, 0, false, &Err(FbError::AddFb(22)), None),
            "MODIFIER plane=31 format=NV12 modifier=0x0000000000000000 listed=no addfb=EINVAL \
             test_only=not reached"
        );
        assert!(
            modifier_line(31, nv12, 0, false, &Err(FbError::Dumb(12)), None)
                .contains("addfb=dumb-ENOMEM")
        );
        assert!(
            modifier_line(31, nv12, 0, false, &Err(FbError::UnsupportedFormat), None)
                .contains("addfb=unsupported-format")
        );
        assert_eq!(
            replace_line(Ok(()), Some(Err(16)), 1),
            "REPLACE first=ok second=EBUSY events=1"
        );
        assert_eq!(
            replace_line(Err(22), None, 0),
            "REPLACE first=EINVAL second=not reached events=0"
        );
    }

    /// Builds a drm_format_modifier_blob.
    fn blob(formats: &[u32], modifiers: &[(u64, u32, u64)]) -> Vec<u8> {
        let mut b = Vec::new();
        let formats_offset = 24u32;
        let modifiers_offset = formats_offset + 4 * formats.len() as u32;
        for v in [
            1u32,
            0,
            formats.len() as u32,
            formats_offset,
            modifiers.len() as u32,
            modifiers_offset,
        ] {
            b.extend_from_slice(&v.to_ne_bytes());
        }
        for f in formats {
            b.extend_from_slice(&f.to_ne_bytes());
        }
        for (mask, offset, modifier) in modifiers {
            b.extend_from_slice(&mask.to_ne_bytes());
            b.extend_from_slice(&offset.to_ne_bytes());
            b.extend_from_slice(&0u32.to_ne_bytes());
            b.extend_from_slice(&modifier.to_ne_bytes());
        }
        b
    }

    #[test]
    fn in_formats_blob() {
        let xr24 = DrmFourcc::Xrgb8888 as u32;
        let nv12 = DrmFourcc::Nv12 as u32;
        let sand = 0x0700_0000_0000_0004;
        let b = blob(&[xr24, nv12], &[(0b11, 0, 0), (0b10, 0, sand)]);
        assert_eq!(parse_in_formats(&b), [(xr24, 0), (nv12, 0), (nv12, sand)]);
        assert!(parse_in_formats(&b[..10]).is_empty());
        // A truncated modifier entry ends the list.
        assert_eq!(parse_in_formats(&b[..b.len() - 4]), [(xr24, 0), (nv12, 0)]);
    }

    #[test]
    fn pairs_listed_then_extra() {
        let xr24 = DrmFourcc::Xrgb8888 as u32;
        let nv12 = DrmFourcc::Nv12 as u32;
        let tiled = 0x0700_0000_0000_0001;
        let pairs = modifier_pairs(&[(xr24, 0), (nv12, 0)], &[xr24, nv12], &[0, tiled], None);
        assert_eq!(
            pairs,
            [
                (xr24, 0, true),
                (nv12, 0, true),
                (xr24, tiled, false),
                (nv12, tiled, false)
            ]
        );
        assert_eq!(
            modifier_pairs(&[(xr24, 0), (nv12, 0)], &[xr24, nv12], &[tiled], Some(nv12)),
            [(nv12, 0, true), (nv12, tiled, false)]
        );
    }

    #[test]
    fn layouts() {
        let xr24 = format_planes(DrmFourcc::Xrgb8888 as u32).unwrap();
        let l = fb_layout(&xr24, (1920, 1080)).unwrap();
        assert_eq!((l.count, l.pitches[0], l.offsets[0]), (1, 7680, 0));
        // 1088 rows of 7680 bytes, page aligned, plus the AFBC margin.
        let body = 7680 * 1088;
        assert_eq!(l.alloc, body + body / 4 + (256 << 10));

        let nv12 = format_planes(DrmFourcc::Nv12 as u32).unwrap();
        let l = fb_layout(&nv12, (1920, 1080)).unwrap();
        assert_eq!(l.count, 2);
        assert_eq!(l.pitches[..2], [1920, 1920]);
        assert_eq!(l.offsets[..2], [0, 1920 * 1088]);

        // P030: three pixels per 32-bit word.
        let p030 = format_planes(u32::from_le_bytes(*b"P030")).unwrap();
        let l = fb_layout(&p030, (1920, 1080)).unwrap();
        assert_eq!(l.pitches[..2], [2560, 2560]);

        // Widths pad to 16 pixels, pitches to 64 bytes.
        let l = fb_layout(&xr24, (1366, 768)).unwrap();
        assert_eq!(l.pitches[0], 1376 * 4);
        let rgb565 = format_planes(DrmFourcc::Rgb565 as u32).unwrap();
        assert_eq!(fb_layout(&rgb565, (100, 10)).unwrap().pitches[0], 256);

        let yu12 = format_planes(DrmFourcc::Yuv420 as u32).unwrap();
        let l = fb_layout(&yu12, (64, 64)).unwrap();
        assert_eq!(l.count, 3);
        assert_eq!(l.offsets[..3], [0, 4096, 8192]);

        assert_eq!(format_planes(u32::from_le_bytes(*b"ZZZZ")), None);
        assert!(fb_layout(&[], (16, 16)).is_none());
    }

    #[test]
    fn every_known_format_has_a_layout_that_fits() {
        for name in [
            "C8", "R8", "RG16", "XR24", "AR30", "AR4H", "YUYV", "RG24", "NV12", "NV16", "NV24",
            "NV15", "P010", "P030", "YU12", "YU24",
        ] {
            let planes = format_planes(parse_fourcc(name).unwrap()).unwrap();
            let l = fb_layout(&planes, (3840, 2160)).unwrap();
            assert_eq!(l.count, planes.len(), "{name}");
            let mut previous_end = 0;
            for (i, p) in planes.iter().enumerate() {
                let start = u64::from(l.offsets[i]);
                assert!(start >= previous_end, "{name} plane {i} overlaps");
                let width = 3840u64.div_ceil(u64::from(p.hsub));
                let min_pitch = width.div_ceil(u64::from(p.block_w)) * u64::from(p.bytes);
                assert!(
                    u64::from(l.pitches[i]) >= min_pitch,
                    "{name} plane {i} pitch"
                );
                previous_end =
                    start + u64::from(l.pitches[i]) * 2160u64.div_ceil(u64::from(p.vsub));
            }
            assert!(previous_end <= l.alloc, "{name}");
        }
    }
}
