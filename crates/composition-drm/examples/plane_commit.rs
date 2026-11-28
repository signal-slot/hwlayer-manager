//! Plane commit: one atomic commit with explicit format, zpos, rotation and
//! alpha per plane, then a check of what the display did.
//!
//! ```text
//! cargo run -p composition-drm --example plane_commit -- [options] --layer SPEC [--layer SPEC]...
//!
//!   --device PATH      DRM device (default /dev/dri/card0); a bare argument works too
//!   --connector ID     connector to use (default: the first connected one)
//!   --layer SPEC       one plane of the request; repeatable
//!   --hold-ms N        how long to leave the commit on screen (default 3000)
//!   --test-only-only   stop after the TEST_ONLY check; nothing is committed
//!
//! SPEC: plane=ID,format=XR24|AR24|NV12,src=WxH,dst=X,Y,WxH[,zpos=N][,rotation=R][,alpha=N][,pattern=P]
//!   plane      plane id (as `props` or `kms_sweep` print it)
//!   format     XR24 (XRGB8888), AR24 (ARGB8888) or NV12; NV12 needs even sizes
//!   src        framebuffer size; the whole buffer is the source rectangle
//!   dst        position and size on the CRTC (X and Y may be negative)
//!   zpos       value of the plane's `zpos` property
//!   rotation   `0`, `90`, `180`, `270`, `reflect-x`, `reflect-y`, joined with
//!              `+` (`180+reflect-x`); a name alone with no angle gets
//!              rotate-0 added; any other name is refused
//!   alpha      value of the plane's `alpha` property (0..=65535)
//!   pattern    `bars` (default: eight vertical colour bars) or `quadrants`
//!              (four colours, one per quadrant: top left red, top right
//!              green, bottom left blue, bottom right white; unlike bars,
//!              every flip and turn gives a different picture, reflect-y
//!              included)
//! ```
//!
//! The program opens the device, becomes DRM master (or exits with status 2
//! saying it could not; it never stops other processes), takes the
//! connector's preferred mode (or, if the CRTC is already running, the mode
//! it runs: no modeset is made then) and a CRTC for it. Each layer gets a
//! dumb buffer of its `src` size filled with eight vertical colour bars
//! (white, yellow, cyan, green, magenta, red, blue, black), or with the four
//! quadrant colours for `pattern=quadrants`. NV12 buffers
//! have a Y plane and an interleaved CbCr plane (BT.601, limited range) on
//! one tall dumb buffer, the CbCr plane at an offset. The quadrant colours
//! are converted with the same integer BT.601 limited-range formula as the
//! bars (`yuv`: Y = 16 + 0.257 R + 0.504 G + 0.098 B, Cb and Cr = 128 plus
//! the usual 0.439-scaled colour differences), giving red (82, 90, 240),
//! green (144, 54, 34), blue (41, 240, 110) and white (235, 128, 128) as
//! (Y, Cb, Cr); one CbCr pair serves a 2x2 block and comes from the block's
//! top left pixel. Only the properties a
//! SPEC names are set besides the ones every placement needs (`FB_ID`,
//! `CRTC_ID`, `SRC_*`, `CRTC_*`); a property the plane lacks is a usage
//! error before anything is sent, but a value the kernel refuses
//! (immutable `zpos`, an unsupported rotation) is the answer and shows up as
//! an errno. Planes that are not named are left as they are.
//!
//! It then makes:
//!
//! 1. an atomic `TEST_ONLY` commit of all layers;
//! 2. unless `--test-only-only`, the real commit of the same request, even
//!    after a failed `TEST_ONLY`, so that a failure after `TEST_ONLY` can be
//!    seen to leave the display running or not;
//! 3. a hold of `--hold-ms`, then a read of the CRTC `ACTIVE` and of each
//!    layer plane's `FB_ID`, `zpos`, `rotation` and `alpha`;
//! 4. a recovery check: a plain commit of the primary plane alone at 1:1
//!    (the other layer planes off, rotation 0 on the primary);
//! 5. a restore of the properties the program touched to the values it read
//!    at start (a CRTC that was not active is switched off again).
//!
//! # Output
//!
//! ```text
//! DATE / KERNEL / DEVICE / DRIVER / MODE / PLANE lines, as kms_sweep prints them
//! START crtc_active=1 primary=31 primary_fb=74
//! LAYER plane=42 format=NV12 src=640x480 dst=100,80,640x480 zpos=2 rotation=0x4 alpha=none
//! TEST_ONLY ok
//! COMMIT EINVAL
//! HOLD ms=3000
//! STATE plane=42 fb=0 zpos=2 rotation=0x1 alpha=65535
//! CRTC active=1
//! RECOVER ok
//! RESTORE ok
//! VERDICT plane_commit: DONE
//! ```
//!
//! After the `COMMIT` line (not with `--test-only-only`) come the `EXPECT`
//! lines, in the format `scripts/twoplane-expect.py` reads (the one
//! `twoplane` prints; `rgb` is `#rrggbb` there):
//!
//! ```text
//! EXPECT phase=commit "42-tl" x=170 y=140 fx=0.0885 fy=0.1296 rgb=#ff0000
//! EXPECT phase=commit skipped reason=plane-alpha plane=31
//! ```
//!
//! One line per quadrant (`tl`, `tr`, `bl`, `br`, named as the *buffer* has
//! them) of every `pattern=quadrants` layer. `x`,`y` is the screen position
//! of the quadrant's centre after the layer's rotation and reflection are
//! applied to its `dst` rectangle (the buffer is scaled to `dst`, turned
//! counter-clockwise by the angle, then reflected; the order matters only
//! for 90 and 270, an assumption), `fx`,`fy` are `x`,`y` over the output
//! size, `rgb` is the colour the buffer holds there. A sample is left out
//! when it is off the output or when another layer covers it that has a
//! higher `zpos` (or an equal or unknown one). A `pattern=bars` layer, or one
//! with plane `alpha` below 65535, gets `skipped reason=bars-pattern` or
//! `reason=plane-alpha` (then `plane=ID`) instead. `twoplane-expect.py` drops
//! the whole `commit` phase when any such line is present, so use quadrant
//! layers at full alpha alone for a picture check. A translucent look is not
//! predicted.
//!
//! `TEST_ONLY`, `COMMIT`, `RECOVER` and `RESTORE` are `ok` or the errno
//! (`EINVAL`, `ERANGE`, `ENOSPC`, `EBUSY`, ... or `errno<N>`). With
//! `--test-only-only` only the lines up to `TEST_ONLY` and `CRTC` are printed.
//! `STATE` values that the plane does not have are left out. The values are
//! what the kernel reports; an `ok` is what the kernel accepted, not a
//! picture: judge the display by eye or with a camera. The rotation bit
//! positions are the standard `DRM_MODE_ROTATE_*` / `DRM_MODE_REFLECT_*`
//! ones (an assumption about the property's enum, which the `drm` crate does
//! not read back).
//!
//! The exit status is 0 after `VERDICT ... DONE` (whatever the commits
//! answered), and 2 for a usage error, a device that cannot be used or a
//! missing DRM master (`VERDICT plane_commit: FAIL ...`).

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use composition_drm::Card;
use drm::buffer::{Buffer, DrmFourcc, DrmModifier, PlanarBuffer};
use drm::control::atomic::AtomicModeReq;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    connector, crtc, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice,
    FbCmd2Flags, ModeTypeFlags, PlaneType, ResourceHandle,
};
use drm::{ClientCapability, Device as BasicDevice};

const USAGE: &str = "usage: plane_commit [--device PATH] [--connector ID] \
--layer plane=ID,format=XR24|AR24|NV12,src=WxH,dst=X,Y,WxH[,zpos=N][,rotation=R][,alpha=N][,pattern=bars|quadrants] \
[--layer ...] [--hold-ms N] [--test-only-only]";

// DRM_MODE_ROTATE_* and DRM_MODE_REFLECT_* from include/uapi/drm/drm_mode.h
// (read from source).
const ROTATE_0: u64 = 1 << 0;
const ROTATE_90: u64 = 1 << 1;
const ROTATE_180: u64 = 1 << 2;
const ROTATE_270: u64 = 1 << 3;
const REFLECT_X: u64 = 1 << 4;
const REFLECT_Y: u64 = 1 << 5;
const ROTATE_MASK: u64 = ROTATE_0 | ROTATE_90 | ROTATE_180 | ROTATE_270;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Xr24,
    Ar24,
    Nv12,
}

impl Format {
    fn fourcc(self) -> DrmFourcc {
        match self {
            Format::Xr24 => DrmFourcc::Xrgb8888,
            Format::Ar24 => DrmFourcc::Argb8888,
            Format::Nv12 => DrmFourcc::Nv12,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Format::Xr24 => "XR24",
            Format::Ar24 => "AR24",
            Format::Nv12 => "NV12",
        }
    }
}

/// `XR24`, `AR24` or `NV12` (or the long names XRGB8888 and ARGB8888).
fn parse_format(text: &str) -> Result<Format, String> {
    match text.to_ascii_uppercase().as_str() {
        "XR24" | "XRGB8888" => Ok(Format::Xr24),
        "AR24" | "ARGB8888" => Ok(Format::Ar24),
        "NV12" => Ok(Format::Nv12),
        _ => Err(format!("bad format {text}: use XR24, AR24 or NV12")),
    }
}

/// A rotation value: names joined with `+`, as the DRM bitmask. A value with
/// reflections only gets rotate-0 added, since a rotation property value
/// always has one angle.
fn parse_rotation(text: &str) -> Result<u64, String> {
    let mut bits = 0u64;
    for name in text.split('+') {
        bits |= match name {
            "0" => ROTATE_0,
            "90" => ROTATE_90,
            "180" => ROTATE_180,
            "270" => ROTATE_270,
            "reflect-x" => REFLECT_X,
            "reflect-y" => REFLECT_Y,
            _ => return Err(format!("bad rotation {name}")),
        };
    }
    match (bits & ROTATE_MASK).count_ones() {
        0 => Ok(bits | ROTATE_0),
        1 => Ok(bits),
        _ => Err(format!("rotation {text} names more than one angle")),
    }
}

/// `WxH`, both non-zero.
fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let (w, h) = text.split_once('x').ok_or(format!("bad size {text}"))?;
    let w: u32 = w.parse().map_err(|_| format!("bad size {text}"))?;
    let h: u32 = h.parse().map_err(|_| format!("bad size {text}"))?;
    if w == 0 || h == 0 || w > 1 << 15 || h > 1 << 15 {
        return Err(format!("size {text} is empty or too large"));
    }
    Ok((w, h))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Pattern {
    /// Eight vertical colour bars.
    #[default]
    Bars,
    /// Four colours, one per quadrant.
    Quadrants,
}

fn parse_pattern(text: &str) -> Result<Pattern, String> {
    match text {
        "bars" => Ok(Pattern::Bars),
        "quadrants" => Ok(Pattern::Quadrants),
        _ => Err(format!("bad pattern {text}: use bars or quadrants")),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Layer {
    plane: u32,
    format: Format,
    src: (u32, u32),
    /// X, Y, width, height on the CRTC.
    dst: (i32, i32, u32, u32),
    zpos: Option<u64>,
    rotation: Option<u64>,
    alpha: Option<u64>,
    pattern: Pattern,
}

/// Splits a SPEC at commas. A piece without `=` continues the value before
/// it, as in `dst=10,20,640x480`.
fn spec_pairs(spec: &str) -> Result<Vec<(String, String)>, String> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for piece in spec.split(',') {
        if let Some((key, value)) = piece.split_once('=') {
            pairs.push((key.to_string(), value.to_string()));
        } else if let Some(last) = pairs.last_mut() {
            last.1.push(',');
            last.1.push_str(piece);
        } else {
            return Err(format!("bad layer {spec}: {piece} is not key=value"));
        }
    }
    Ok(pairs)
}

fn parse_layer(spec: &str) -> Result<Layer, String> {
    let number = |text: &str| -> Result<u64, String> {
        text.parse().map_err(|_| format!("bad number {text}"))
    };
    let (mut plane, mut format, mut src, mut dst) = (None, None, None, None);
    let (mut zpos, mut rotation, mut alpha) = (None, None, None);
    let mut pattern = Pattern::Bars;
    for (key, value) in spec_pairs(spec)? {
        match key.as_str() {
            "plane" => plane = Some(u32::try_from(number(&value)?).map_err(|_| "bad plane id")?),
            "format" => format = Some(parse_format(&value)?),
            "src" => src = Some(parse_size(&value)?),
            "dst" => {
                let parts: Vec<&str> = value.split(',').collect();
                let [x, y, size] = parts[..] else {
                    return Err(format!("dst takes X,Y,WxH, not {value}"));
                };
                let coordinate = |t: &str| t.parse::<i32>().map_err(|_| format!("bad number {t}"));
                let (w, h) = parse_size(size)?;
                dst = Some((coordinate(x)?, coordinate(y)?, w, h));
            }
            "zpos" => zpos = Some(number(&value)?),
            "rotation" => rotation = Some(parse_rotation(&value)?),
            "alpha" => {
                let a = number(&value)?;
                if a > 0xffff {
                    return Err(format!("alpha {a} is above 65535"));
                }
                alpha = Some(a);
            }
            "pattern" => pattern = parse_pattern(&value)?,
            other => return Err(format!("unknown layer key {other}")),
        }
    }
    let layer = Layer {
        plane: plane.ok_or("layer needs plane=ID")?,
        format: format.ok_or("layer needs format=")?,
        src: src.ok_or("layer needs src=WxH")?,
        dst: dst.ok_or("layer needs dst=X,Y,WxH")?,
        zpos,
        rotation,
        alpha,
        pattern,
    };
    if layer.format == Format::Nv12 && (layer.src.0 % 2 != 0 || layer.src.1 % 2 != 0) {
        return Err(format!(
            "NV12 needs an even src size, not {}x{}",
            layer.src.0, layer.src.1
        ));
    }
    Ok(layer)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Options {
    device: String,
    connector: Option<u32>,
    layers: Vec<Layer>,
    hold_ms: u64,
    test_only_only: bool,
}

/// Parses the arguments after the program name. An empty error asks for the
/// usage text alone.
fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
    let mut options = Options {
        device: "/dev/dri/card0".into(),
        connector: None,
        layers: Vec::new(),
        hold_ms: 3000,
        test_only_only: false,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--device" => options.device = value("--device")?,
            "--connector" => {
                let text = value("--connector")?;
                options.connector = Some(text.parse().map_err(|_| format!("bad number {text}"))?);
            }
            "--layer" => {
                let layer = parse_layer(&value("--layer")?)?;
                if options.layers.iter().any(|l| l.plane == layer.plane) {
                    return Err(format!("plane {} is named twice", layer.plane));
                }
                options.layers.push(layer);
            }
            "--hold-ms" => {
                let text = value("--hold-ms")?;
                options.hold_ms = text.parse().map_err(|_| format!("bad number {text}"))?;
            }
            "--test-only-only" => options.test_only_only = true,
            "-h" | "--help" => return Err(String::new()),
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => options.device = other.to_string(),
        }
    }
    if options.layers.is_empty() {
        return Err("at least one --layer is needed".into());
    }
    Ok(options)
}

/// The colour bars as (R, G, B).
const BARS: [[u8; 3]; 8] = [
    [255, 255, 255],
    [255, 255, 0],
    [0, 255, 255],
    [0, 255, 0],
    [255, 0, 255],
    [255, 0, 0],
    [0, 0, 255],
    [0, 0, 0],
];

/// The bar a pixel column belongs to.
fn bar_at(x: u32, width: u32) -> usize {
    (u64::from(x) * 8 / u64::from(width)) as usize
}

/// BT.601 limited-range (Y, Cb, Cr) of an RGB colour.
fn yuv([r, g, b]: [u8; 3]) -> (u8, u8, u8) {
    let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
    let y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
    let cb = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
    let cr = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
    (y as u8, cb as u8, cr as u8)
}

fn round_up(value: u64, to: u64) -> u64 {
    value.div_ceil(to) * to
}

/// Where the planes of an NV12 buffer lie in one allocation: the Y plane at
/// offset 0 and the interleaved CbCr plane (half the rows, the same pitch)
/// at `uv_offset`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Nv12Layout {
    pitch: u32,
    uv_offset: u32,
    /// Bytes of the whole buffer.
    total: u64,
}

/// The pitch a buffer of `width` pixels gets before the kernel's own
/// alignment: the width rounded up to 64 bytes.
fn nv12_min_pitch(width: u32) -> u32 {
    round_up(u64::from(width), 64) as u32
}

/// Layout for a `width` x `height` NV12 image at `pitch` bytes per row. The
/// CbCr plane starts at the next 4 KiB boundary after the Y plane.
fn nv12_layout(width: u32, height: u32, pitch: u32) -> Option<Nv12Layout> {
    if width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 || pitch < width {
        return None;
    }
    let y_bytes = u64::from(pitch) * u64::from(height);
    let uv_offset = round_up(y_bytes, 4096);
    Some(Nv12Layout {
        pitch,
        uv_offset: u32::try_from(uv_offset).ok()?,
        total: uv_offset + u64::from(pitch) * u64::from(height / 2),
    })
}

/// Fills `buf` (at least `layout.total` bytes) with the colour bars: luma
/// per pixel, then one (Cb, Cr) pair per 2x2 pixels from the bar of the
/// pair's left pixel.
fn fill_nv12(buf: &mut [u8], layout: &Nv12Layout, width: u32, height: u32) {
    let pitch = layout.pitch as usize;
    for row in 0..height as usize {
        for x in 0..width {
            buf[row * pitch + x as usize] = yuv(BARS[bar_at(x, width)]).0;
        }
    }
    let uv = layout.uv_offset as usize;
    for row in 0..(height / 2) as usize {
        for pair in 0..width / 2 {
            let (_, cb, cr) = yuv(BARS[bar_at(pair * 2, width)]);
            let at = uv + row * pitch + pair as usize * 2;
            buf[at] = cb;
            buf[at + 1] = cr;
        }
    }
}

/// Fills a 32-bit buffer (bytes B, G, R, A/X) with the colour bars, `pitch`
/// bytes per row.
fn fill_rgb(buf: &mut [u8], pitch: usize, width: u32, height: u32) {
    for row in 0..height as usize {
        for x in 0..width {
            let [r, g, b] = BARS[bar_at(x, width)];
            let at = row * pitch + x as usize * 4;
            buf[at..at + 4].copy_from_slice(&[b, g, r, 0xff]);
        }
    }
}

/// The quadrant colours as (R, G, B): top left, top right, bottom left,
/// bottom right.
const QUADRANTS: [[u8; 3]; 4] = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];

/// Names of the quadrants, in the order of `QUADRANTS`.
const QUADRANT_NAMES: [&str; 4] = ["tl", "tr", "bl", "br"];

/// The quadrant (index into `QUADRANTS`) of pixel (`x`, `y`) in a
/// `width` x `height` buffer.
fn quadrant_at(x: u32, y: u32, width: u32, height: u32) -> usize {
    usize::from(u64::from(x) * 2 >= u64::from(width))
        + 2 * usize::from(u64::from(y) * 2 >= u64::from(height))
}

/// Like `fill_nv12`, with the quadrant colours. The CbCr pair of a 2x2 block
/// comes from the block's top left pixel.
fn fill_nv12_quadrants(buf: &mut [u8], layout: &Nv12Layout, width: u32, height: u32) {
    let pitch = layout.pitch as usize;
    for row in 0..height {
        for x in 0..width {
            let colour = QUADRANTS[quadrant_at(x, row, width, height)];
            buf[row as usize * pitch + x as usize] = yuv(colour).0;
        }
    }
    let uv = layout.uv_offset as usize;
    for row in 0..height / 2 {
        for pair in 0..width / 2 {
            let colour = QUADRANTS[quadrant_at(pair * 2, row * 2, width, height)];
            let (_, cb, cr) = yuv(colour);
            let at = uv + row as usize * pitch + pair as usize * 2;
            buf[at] = cb;
            buf[at + 1] = cr;
        }
    }
}

/// Like `fill_rgb`, with the quadrant colours (alpha opaque).
fn fill_rgb_quadrants(buf: &mut [u8], pitch: usize, width: u32, height: u32) {
    for row in 0..height {
        for x in 0..width {
            let [r, g, b] = QUADRANTS[quadrant_at(x, row, width, height)];
            let at = row as usize * pitch + x as usize * 4;
            buf[at..at + 4].copy_from_slice(&[b, g, r, 0xff]);
        }
    }
}

/// Where the centre of buffer quadrant `quadrant` lands on the screen: the
/// buffer is scaled to `dst`, turned counter-clockwise by the angle, then
/// reflected. Positions are kept in quarters of the `dst` size until the
/// final division.
fn quadrant_screen_pos(layer: &Layer, quadrant: usize) -> (i64, i64) {
    let rotation = layer.rotation.unwrap_or(ROTATE_0);
    let (mut a, mut b) = (
        1 + 2 * (quadrant as i64 & 1),
        1 + 2 * (quadrant as i64 >> 1),
    );
    match rotation & ROTATE_MASK {
        ROTATE_90 => (a, b) = (b, 4 - a),
        ROTATE_180 => (a, b) = (4 - a, 4 - b),
        ROTATE_270 => (a, b) = (4 - b, a),
        _ => {}
    }
    if rotation & REFLECT_X != 0 {
        a = 4 - a;
    }
    if rotation & REFLECT_Y != 0 {
        b = 4 - b;
    }
    let (x, y, w, h) = layer.dst;
    (
        i64::from(x) + a * i64::from(w) / 4,
        i64::from(y) + b * i64::from(h) / 4,
    )
}

fn covers(layer: &Layer, x: i64, y: i64) -> bool {
    let (lx, ly, w, h) = layer.dst;
    (i64::from(lx)..i64::from(lx) + i64::from(w)).contains(&x)
        && (i64::from(ly)..i64::from(ly) + i64::from(h)).contains(&y)
}

/// The `EXPECT phase=commit` lines for the committed layers on an output of
/// `output` pixels.
fn expect_lines(layers: &[Layer], output: (u32, u32)) -> Vec<String> {
    let mut lines = Vec::new();
    for layer in layers {
        let reason = if layer.pattern == Pattern::Bars {
            Some("bars-pattern")
        } else if layer.alpha.is_some_and(|a| a < 0xffff) {
            Some("plane-alpha")
        } else {
            None
        };
        if let Some(reason) = reason {
            lines.push(format!(
                "EXPECT phase=commit skipped reason={reason} plane={}",
                layer.plane
            ));
            continue;
        }
        for (quadrant, [r, g, b]) in QUADRANTS.into_iter().enumerate() {
            let (x, y) = quadrant_screen_pos(layer, quadrant);
            if x < 0 || y < 0 || x >= i64::from(output.0) || y >= i64::from(output.1) {
                continue;
            }
            // Another layer hides the sample unless it is known to be below.
            let hidden = layers.iter().any(|other| {
                other.plane != layer.plane
                    && covers(other, x, y)
                    && !matches!((other.zpos, layer.zpos), (Some(o), Some(l)) if o < l)
            });
            if hidden {
                continue;
            }
            lines.push(format!(
                "EXPECT phase=commit \"{}-{}\" x={x} y={y} fx={:.4} fy={:.4} rgb=#{r:02x}{g:02x}{b:02x}",
                layer.plane,
                QUADRANT_NAMES[quadrant],
                x as f64 / f64::from(output.0),
                y as f64 / f64::from(output.1),
            ));
        }
    }
    lines
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

fn result_line(name: &str, check: Check) -> String {
    format!("{name} {}", check_text(check))
}

fn layer_line(layer: &Layer) -> String {
    let (x, y, w, h) = layer.dst;
    let opt = |v: Option<u64>, hex: bool| match v {
        Some(v) if hex => format!("{v:#x}"),
        Some(v) => v.to_string(),
        None => "none".into(),
    };
    let pattern = match layer.pattern {
        Pattern::Bars => "",
        Pattern::Quadrants => " pattern=quadrants",
    };
    format!(
        "LAYER plane={} format={} src={}x{} dst={x},{y},{w}x{h} zpos={} rotation={} alpha={}{pattern}",
        layer.plane,
        layer.format.name(),
        layer.src.0,
        layer.src.1,
        opt(layer.zpos, false),
        opt(layer.rotation, true),
        opt(layer.alpha, false)
    )
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

/// A property of an object: handle, value at the time of the read and
/// whether userspace may set it.
#[derive(Clone, Copy)]
struct Prop {
    handle: property::Handle,
    value: u64,
    mutable: bool,
}

type Props = HashMap<String, Prop>;

fn named_properties<H: ResourceHandle>(card: &Card, handle: H) -> io::Result<Props> {
    let set = card.get_properties(handle)?;
    let mut named = HashMap::new();
    for (prop, value) in set.iter() {
        let info = card.get_property(*prop)?;
        named.insert(
            info.name().to_string_lossy().into_owned(),
            Prop {
                handle: *prop,
                value: *value,
                mutable: info.mutable(),
            },
        );
    }
    Ok(named)
}

fn prop(props: &Props, name: &str) -> io::Result<Prop> {
    props
        .get(name)
        .copied()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no property {name}")))
}

fn fail(what: &str) -> impl Fn(io::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

fn errno_of(error: &io::Error) -> i32 {
    error.raw_os_error().unwrap_or(-1)
}

/// A plane the program may touch.
struct PlaneInfo {
    handle: plane::Handle,
    kind: PlaneType,
    props: Props,
}

impl PlaneInfo {
    fn id(&self) -> u32 {
        u32::from(self.handle)
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
        None
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

/// A framebuffer of `format` and `size` filled with the colour bars. NV12 is
/// one 32 bpp dumb buffer wide as the pitch and tall enough for both planes.
fn make_fb(card: &Card, format: Format, size: (u32, u32), pattern: Pattern) -> Result<Fb, String> {
    let (width, height) = size;
    let dumb_size = match format {
        Format::Nv12 => {
            let layout = nv12_layout(width, height, nv12_min_pitch(width)).ok_or("NV12 size")?;
            (
                layout.pitch / 4,
                u32::try_from(layout.total.div_ceil(u64::from(layout.pitch)))
                    .map_err(|_| "NV12 size")?,
            )
        }
        _ => size,
    };
    let mut dumb = card
        .create_dumb_buffer(dumb_size, DrmFourcc::Xrgb8888, 32)
        .map_err(|e| format!("dumb buffer: {}", errno_name(errno_of(&e))))?;
    let pitch = dumb.pitch();
    let handle = dumb.handle();
    let mut buffer = Layout {
        size,
        format: format.fourcc(),
        pitches: [pitch, 0, 0, 0],
        offsets: [0; 4],
        handles: [Some(handle), None, None, None],
    };
    let filled: Result<(), String> = (|| {
        let mut mapping = card.map_dumb_buffer(&mut dumb).map_err(fail("map"))?;
        let bytes = mapping.as_mut();
        match format {
            Format::Nv12 => {
                let layout = nv12_layout(width, height, pitch).ok_or("NV12 size")?;
                if layout.total > bytes.len() as u64 {
                    return Err("NV12 planes do not fit the dumb buffer".into());
                }
                match pattern {
                    Pattern::Bars => fill_nv12(bytes, &layout, width, height),
                    Pattern::Quadrants => fill_nv12_quadrants(bytes, &layout, width, height),
                }
                buffer.pitches = [pitch, pitch, 0, 0];
                buffer.offsets = [0, layout.uv_offset, 0, 0];
                buffer.handles = [Some(handle), Some(handle), None, None];
            }
            _ => match pattern {
                Pattern::Bars => fill_rgb(bytes, pitch as usize, width, height),
                Pattern::Quadrants => fill_rgb_quadrants(bytes, pitch as usize, width, height),
            },
        }
        Ok(())
    })();
    if let Err(message) = filled {
        let _ = card.destroy_dumb_buffer(dumb);
        return Err(message);
    }
    match card.add_planar_framebuffer(&buffer, FbCmd2Flags::empty()) {
        Ok(fb) => Ok(Fb { dumb, fb }),
        Err(error) => {
            let _ = card.destroy_dumb_buffer(dumb);
            Err(format!("ADDFB2: {}", errno_name(errno_of(&error))))
        }
    }
}

fn release(card: &Card, fb: Fb) {
    let _ = card.destroy_framebuffer(fb.fb);
    let _ = card.destroy_dumb_buffer(fb.dumb);
}

const PLACEMENT: [&str; 10] = [
    "FB_ID", "CRTC_ID", "SRC_X", "SRC_Y", "SRC_W", "SRC_H", "CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H",
];

/// What the program put on the display, to be taken back at the end.
struct Saved {
    crtc_active: bool,
    /// (plane, property handle, raw value) of every mutable property the
    /// program may set on the planes it uses.
    values: Vec<(plane::Handle, property::Handle, u64)>,
}

struct Ctx {
    card: Card,
    crtc: crtc::Handle,
    connector: connector::Handle,
    connector_crtc_id: property::Handle,
    crtc_mode_id: property::Handle,
    crtc_active: Prop,
    mode_blob: u64,
    output: (u32, u32),
    planes: Vec<PlaneInfo>,
    primary: usize,
}

impl Ctx {
    fn plane(&self, id: u32) -> Option<&PlaneInfo> {
        self.planes.iter().find(|p| p.id() == id)
    }

    fn modeset_request(&self, req: &mut AtomicModeReq) {
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
        req.add_property(
            self.crtc,
            self.crtc_active.handle,
            property::Value::Boolean(true),
        );
    }

    /// The placement properties, the same set on every call.
    fn place(
        &self,
        req: &mut AtomicModeReq,
        plane: &PlaneInfo,
        fb: framebuffer::Handle,
        src: (u32, u32),
        dst: (i32, i32, u32, u32),
    ) -> io::Result<()> {
        let p = |name: &str| prop(&plane.props, name).map(|p| p.handle);
        let h = plane.handle;
        req.add_property(h, p("FB_ID")?, property::Value::Framebuffer(Some(fb)));
        req.add_property(h, p("CRTC_ID")?, property::Value::CRTC(Some(self.crtc)));
        let source = [0, 0, u64::from(src.0) << 16, u64::from(src.1) << 16];
        for (name, v) in ["SRC_X", "SRC_Y", "SRC_W", "SRC_H"].into_iter().zip(source) {
            req.add_property(h, p(name)?, property::Value::UnsignedRange(v));
        }
        req.add_property(h, p("CRTC_X")?, property::Value::SignedRange(dst.0.into()));
        req.add_property(h, p("CRTC_Y")?, property::Value::SignedRange(dst.1.into()));
        req.add_property(
            h,
            p("CRTC_W")?,
            property::Value::UnsignedRange(dst.2.into()),
        );
        req.add_property(
            h,
            p("CRTC_H")?,
            property::Value::UnsignedRange(dst.3.into()),
        );
        Ok(())
    }

    fn commit(&self, flags: AtomicCommitFlags, req: AtomicModeReq) -> Check {
        self.card
            .atomic_commit(flags, req)
            .map_err(|e| errno_of(&e))
    }
}

/// Raw values are passed through as they were read.
fn raw(value: u64) -> property::Value<'static> {
    property::Value::Unknown(value)
}

fn open(options: &Options) -> Result<(Ctx, bool, AtomicCommitFlags), String> {
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
             holds it; stop it for the run. This tool never stops other processes"
        )
    })?;

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
    let crtc_props = named_properties(&card, crtc).map_err(fail("crtc props"))?;
    let crtc_active = prop(&crtc_props, "ACTIVE").map_err(fail("crtc"))?;
    let was_active = crtc_active.value != 0;
    let preferred = info
        .modes()
        .iter()
        .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
        .or_else(|| info.modes().first())
        .copied()
        .ok_or("no mode")?;
    // A running CRTC keeps its mode: no modeset is needed then.
    let running = card
        .get_crtc(crtc)
        .map_err(fail("crtc"))?
        .mode()
        .filter(|_| was_active);
    let mode = running.unwrap_or(preferred);
    let (mw, mh) = mode.size();
    let output = (u32::from(mw), u32::from(mh));
    println!(
        "MODE {}x{}@{}Hz connector {} crtc {}{}",
        output.0,
        output.1,
        mode.vrefresh(),
        u32::from(connector),
        u32::from(crtc),
        if running.is_some() {
            " (running mode, no modeset)"
        } else {
            ""
        }
    );
    let connector_props = named_properties(&card, connector).map_err(fail("connector props"))?;

    let mut planes = Vec::new();
    let mut handles = card.plane_handles().map_err(fail("planes"))?;
    handles.sort_by_key(|h| u32::from(*h));
    for handle in handles {
        let pinfo = card.get_plane(handle).map_err(fail("plane"))?;
        let props = named_properties(&card, handle).map_err(fail("plane props"))?;
        let kind = match props.get("type").map(|p| p.value as u32) {
            Some(v) if v == PlaneType::Primary as u32 => PlaneType::Primary,
            Some(v) if v == PlaneType::Cursor as u32 => PlaneType::Cursor,
            _ => PlaneType::Overlay,
        };
        let possible = resources
            .filter_crtcs(pinfo.possible_crtcs())
            .contains(&crtc);
        let describe = |name: &str| {
            props
                .get(name)
                .map_or("none".to_string(), |p| p.value.to_string())
        };
        println!(
            "PLANE id {} {:?} possible_on_crtc {possible} zpos {} rotation {} alpha {} formats {}",
            u32::from(handle),
            kind,
            describe("zpos"),
            describe("rotation"),
            describe("alpha"),
            pinfo
                .formats()
                .iter()
                .map(|f| fourcc_text(*f))
                .collect::<Vec<_>>()
                .join(",")
        );
        if possible {
            planes.push(PlaneInfo {
                handle,
                kind,
                props,
            });
        }
    }
    let primary = planes
        .iter()
        .position(|p| p.kind == PlaneType::Primary && p.props.contains_key("FB_ID"))
        .ok_or("no primary plane can drive the CRTC")?;
    let mode_blob = match card
        .create_property_blob(&mode)
        .map_err(fail("mode blob"))?
    {
        property::Value::Blob(id) => id,
        _ => return Err("mode blob".into()),
    };
    let flags = if was_active {
        AtomicCommitFlags::empty()
    } else {
        AtomicCommitFlags::ALLOW_MODESET
    };
    Ok((
        Ctx {
            card,
            crtc,
            connector,
            connector_crtc_id: prop(&connector_props, "CRTC_ID")
                .map_err(fail("connector"))?
                .handle,
            crtc_mode_id: prop(&crtc_props, "MODE_ID").map_err(fail("crtc"))?.handle,
            crtc_active,
            mode_blob,
            output,
            planes,
            primary,
        },
        was_active,
        flags,
    ))
}

/// Checks every layer against the plane's properties before anything is
/// sent.
fn validate(ctx: &Ctx, layers: &[Layer]) -> Result<(), String> {
    for layer in layers {
        let plane = ctx
            .plane(layer.plane)
            .ok_or(format!("plane {} is not a plane of the CRTC", layer.plane))?;
        if plane.kind == PlaneType::Cursor {
            return Err(format!("plane {} is a cursor plane", layer.plane));
        }
        for (name, wanted) in [
            ("zpos", layer.zpos.is_some()),
            ("rotation", layer.rotation.is_some()),
            ("alpha", layer.alpha.is_some()),
        ] {
            if wanted && !plane.props.contains_key(name) {
                return Err(format!("plane {} has no {name} property", layer.plane));
            }
        }
        for name in PLACEMENT {
            prop(&plane.props, name).map_err(|_| format!("plane {} has no {name}", layer.plane))?;
        }
    }
    Ok(())
}

/// The request that shows every layer.
fn layers_request(
    ctx: &Ctx,
    layers: &[Layer],
    fbs: &[framebuffer::Handle],
    modeset: bool,
) -> Result<AtomicModeReq, String> {
    let mut req = AtomicModeReq::new();
    if modeset {
        ctx.modeset_request(&mut req);
    }
    for (layer, fb) in layers.iter().zip(fbs) {
        let plane = ctx.plane(layer.plane).ok_or("plane")?;
        ctx.place(&mut req, plane, *fb, layer.src, layer.dst)
            .map_err(fail("place"))?;
        for (name, value) in [
            ("zpos", layer.zpos),
            ("rotation", layer.rotation),
            ("alpha", layer.alpha),
        ] {
            if let (Some(value), Some(p)) = (value, plane.props.get(name)) {
                req.add_property(plane.handle, p.handle, raw(value));
            }
        }
    }
    Ok(req)
}

fn crtc_active_now(ctx: &Ctx) -> Result<bool, String> {
    let props = named_properties(&ctx.card, ctx.crtc).map_err(fail("crtc props"))?;
    Ok(prop(&props, "ACTIVE").map_err(fail("crtc"))?.value != 0)
}

fn state_lines(ctx: &Ctx, layers: &[Layer]) -> Result<(), String> {
    for layer in layers {
        let Some(plane) = ctx.plane(layer.plane) else {
            continue;
        };
        let now = named_properties(&ctx.card, plane.handle).map_err(fail("plane props"))?;
        let mut line = format!("STATE plane={}", plane.id());
        for (name, key, hex) in [
            ("fb", "FB_ID", false),
            ("zpos", "zpos", false),
            ("rotation", "rotation", true),
            ("alpha", "alpha", false),
        ] {
            if let Some(p) = now.get(key) {
                if hex {
                    line.push_str(&format!(" {name}={:#x}", p.value));
                } else {
                    line.push_str(&format!(" {name}={}", p.value));
                }
            }
        }
        println!("{line}");
    }
    Ok(())
}

fn save(ctx: &Ctx, layers: &[Layer], was_active: bool) -> Saved {
    let mut ids: Vec<u32> = layers.iter().map(|l| l.plane).collect();
    ids.push(ctx.planes[ctx.primary].id());
    ids.sort_unstable();
    ids.dedup();
    let mut values = Vec::new();
    for id in ids {
        let Some(plane) = ctx.plane(id) else { continue };
        for name in PLACEMENT.into_iter().chain(["zpos", "rotation", "alpha"]) {
            if let Some(p) = plane.props.get(name).filter(|p| p.mutable) {
                values.push((plane.handle, p.handle, p.value));
            }
        }
    }
    Saved {
        crtc_active: was_active,
        values,
    }
}

/// Takes the program's changes back. A CRTC that was off is switched off
/// again with its planes.
fn restore(ctx: &Ctx, saved: &Saved) -> Check {
    let mut req = AtomicModeReq::new();
    for (plane, handle, value) in &saved.values {
        req.add_property(*plane, *handle, raw(*value));
    }
    let mut flags = AtomicCommitFlags::empty();
    if !saved.crtc_active {
        // Planes off, then the CRTC: the saved values already say so for a
        // CRTC that had nothing on it, but do not rely on that.
        for plane in &ctx.planes {
            if let (Some(fb), Some(crtc_id)) =
                (plane.props.get("FB_ID"), plane.props.get("CRTC_ID"))
            {
                req.add_property(plane.handle, fb.handle, property::Value::Framebuffer(None));
                req.add_property(plane.handle, crtc_id.handle, property::Value::CRTC(None));
            }
        }
        req.add_property(
            ctx.crtc,
            ctx.crtc_active.handle,
            property::Value::Boolean(false),
        );
        flags = AtomicCommitFlags::ALLOW_MODESET;
    }
    ctx.commit(flags, req)
}

fn run(options: &Options) -> Result<(), String> {
    let (ctx, was_active, flags) = open(options)?;
    let mut buffers: Vec<Fb> = Vec::new();
    let result = session(&ctx, options, was_active, flags, &mut buffers);
    for fb in buffers {
        release(&ctx.card, fb);
    }
    let _ = ctx.card.destroy_property_blob(ctx.mode_blob);
    result
}

fn session(
    ctx: &Ctx,
    options: &Options,
    was_active: bool,
    flags: AtomicCommitFlags,
    buffers: &mut Vec<Fb>,
) -> Result<(), String> {
    validate(ctx, &options.layers)?;
    let primary = &ctx.planes[ctx.primary];
    println!(
        "START crtc_active={} primary={} primary_fb={}",
        u8::from(was_active),
        primary.id(),
        prop(&primary.props, "FB_ID").map_or(0, |p| p.value)
    );
    let saved = save(ctx, &options.layers, was_active);
    let mut fbs = Vec::new();
    for layer in &options.layers {
        println!("{}", layer_line(layer));
        let fb = make_fb(&ctx.card, layer.format, layer.src, layer.pattern)
            .map_err(|e| format!("plane {} {}: {e}", layer.plane, layer.format.name()))?;
        fbs.push(fb.fb);
        buffers.push(fb);
    }
    let modeset = !was_active;
    let req = layers_request(ctx, &options.layers, &fbs, modeset)?;
    println!(
        "{}",
        result_line(
            "TEST_ONLY",
            ctx.commit(flags | AtomicCommitFlags::TEST_ONLY, req)
        )
    );
    if options.test_only_only {
        println!("CRTC active={}", u8::from(crtc_active_now(ctx)?));
        return Ok(());
    }
    let req = layers_request(ctx, &options.layers, &fbs, modeset)?;
    println!("{}", result_line("COMMIT", ctx.commit(flags, req)));
    for line in expect_lines(&options.layers, ctx.output) {
        println!("{line}");
    }
    std::thread::sleep(Duration::from_millis(options.hold_ms));
    println!("HOLD ms={}", options.hold_ms);
    state_lines(ctx, &options.layers)?;
    println!("CRTC active={}", u8::from(crtc_active_now(ctx)?));

    // Primary alone at 1:1, the other layer planes off, rotation 0.
    let primary_format = Format::Xr24;
    let recover = make_fb(&ctx.card, primary_format, ctx.output, Pattern::Bars)
        .map_err(|e| format!("recovery buffer: {e}"))?;
    let recover_fb = recover.fb;
    buffers.push(recover);
    let mut req = AtomicModeReq::new();
    if modeset {
        ctx.modeset_request(&mut req);
    }
    ctx.place(
        &mut req,
        primary,
        recover_fb,
        ctx.output,
        (0, 0, ctx.output.0, ctx.output.1),
    )
    .map_err(fail("place"))?;
    if let Some(rotation) = primary.props.get("rotation") {
        req.add_property(primary.handle, rotation.handle, raw(ROTATE_0));
    }
    for layer in &options.layers {
        if layer.plane == primary.id() {
            continue;
        }
        if let Some(plane) = ctx.plane(layer.plane) {
            req.add_property(
                plane.handle,
                prop(&plane.props, "FB_ID").map_err(fail("plane"))?.handle,
                property::Value::Framebuffer(None),
            );
            req.add_property(
                plane.handle,
                prop(&plane.props, "CRTC_ID").map_err(fail("plane"))?.handle,
                property::Value::CRTC(None),
            );
        }
    }
    println!("{}", result_line("RECOVER", ctx.commit(flags, req)));

    println!("{}", result_line("RESTORE", restore(ctx, &saved)));
    Ok(())
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
            println!("VERDICT plane_commit: DONE");
            ExitCode::SUCCESS
        }
        Err(message) => {
            println!("VERDICT plane_commit: FAIL {message}");
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

    const NV12_LAYER: &str = "plane=42,format=NV12,src=640x480,dst=100,-80,640x480";

    #[test]
    fn parses_a_full_layer() {
        let l = parse_layer(
            "plane=31,format=AR24,src=64x32,dst=10,20,128x64,zpos=3,rotation=180+reflect-x,alpha=32768",
        )
        .unwrap();
        assert_eq!(
            l,
            Layer {
                plane: 31,
                format: Format::Ar24,
                src: (64, 32),
                dst: (10, 20, 128, 64),
                zpos: Some(3),
                rotation: Some(ROTATE_180 | REFLECT_X),
                alpha: Some(32768),
                pattern: Pattern::Bars,
            }
        );
        let n = parse_layer(NV12_LAYER).unwrap();
        assert_eq!(n.dst, (100, -80, 640, 480));
        assert_eq!((n.zpos, n.rotation, n.alpha), (None, None, None));
    }

    #[test]
    fn parses_options() {
        let o = args(&format!(
            "/dev/dri/card1 --connector 5 --layer {NV12_LAYER} --hold-ms 250 --test-only-only"
        ))
        .unwrap();
        assert_eq!(
            (o.device.as_str(), o.connector),
            ("/dev/dri/card1", Some(5))
        );
        assert_eq!(
            (o.hold_ms, o.test_only_only, o.layers.len()),
            (250, true, 1)
        );
        let o = args(&format!("--layer {NV12_LAYER}")).unwrap();
        assert_eq!((o.device.as_str(), o.hold_ms), ("/dev/dri/card0", 3000));
        assert!(!o.test_only_only);
    }

    #[test]
    fn rejects_bad_arguments() {
        assert_eq!(args("--help"), Err(String::new()));
        assert!(args("").unwrap_err().contains("--layer"));
        assert!(args("--bogus").unwrap_err().contains("unknown option"));
        assert!(args("--layer").unwrap_err().contains("needs a value"));
        assert!(args("--hold-ms x").is_err());
        let two = format!("--layer {NV12_LAYER} --layer {NV12_LAYER}");
        assert!(args(&two).unwrap_err().contains("twice"));
        for bad in [
            "format=XR24,src=8x8,dst=0,0,8x8",
            "plane=1,src=8x8,dst=0,0,8x8",
            "plane=1,format=XR24,dst=0,0,8x8",
            "plane=1,format=XR24,src=8x8",
            "plane=1,format=XR24,src=8x8,dst=0,0",
            "plane=1,format=XR24,src=0x8,dst=0,0,8x8",
            "plane=1,format=RG16,src=8x8,dst=0,0,8x8",
            "plane=1,format=XR24,src=8x8,dst=0,0,8x8,zpos=-1",
            "plane=1,format=XR24,src=8x8,dst=0,0,8x8,alpha=65536",
            "plane=1,format=XR24,src=8x8,dst=0,0,8x8,colour=1",
            "plane=1,format=XR24,src=8x8,dst=0,0,8x8,rotation=45",
            "plane=1,format=NV12,src=7x8,dst=0,0,8x8",
            "plane=1,format=NV12,src=8x7,dst=0,0,8x8",
            "src=8x8,plane=1",
            "8x8",
        ] {
            assert!(parse_layer(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn rotation_bits() {
        assert_eq!(parse_rotation("0"), Ok(1));
        assert_eq!(parse_rotation("90"), Ok(2));
        assert_eq!(parse_rotation("180"), Ok(4));
        assert_eq!(parse_rotation("270"), Ok(8));
        // Reflections alone carry rotate-0.
        assert_eq!(parse_rotation("reflect-x"), Ok(0x11));
        assert_eq!(parse_rotation("reflect-y"), Ok(0x21));
        assert_eq!(parse_rotation("reflect-x+reflect-y"), Ok(0x31));
        assert_eq!(parse_rotation("180+reflect-y"), Ok(0x24));
        assert_eq!(parse_rotation("reflect-x+180"), Ok(0x14));
        assert!(parse_rotation("90+180").is_err());
        assert!(parse_rotation("flip").is_err());
        assert!(parse_rotation("").is_err());
        assert!(parse_rotation("reflect-x+").is_err());
        assert!(parse_rotation("rotate-180").is_err());
    }

    #[test]
    fn formats() {
        assert_eq!(parse_format("XR24"), Ok(Format::Xr24));
        assert_eq!(parse_format("ar24"), Ok(Format::Ar24));
        assert_eq!(parse_format("NV12"), Ok(Format::Nv12));
        assert_eq!(parse_format("XRGB8888"), Ok(Format::Xr24));
        assert!(parse_format("NV21").is_err());
        assert!(parse_format("").is_err());
        assert_eq!(Format::Nv12.fourcc(), DrmFourcc::Nv12);
        assert_eq!(fourcc_text(Format::Xr24.fourcc() as u32), "XR24");
    }

    #[test]
    fn nv12_pitch_and_offsets() {
        assert_eq!(nv12_min_pitch(640), 640);
        assert_eq!(nv12_min_pitch(100), 128);
        let l = nv12_layout(640, 480, 640).unwrap();
        // Y plane 307200 bytes, rounded to 4096: 307200 = 75 * 4096.
        assert_eq!(l.uv_offset, 307_200);
        assert_eq!(l.total, 307_200 + 640 * 240);
        let l = nv12_layout(100, 50, 128).unwrap();
        assert_eq!(l.uv_offset, 8192);
        assert_eq!(l.total, 8192 + 128 * 25);
        // A wider pitch moves the CbCr plane.
        assert!(nv12_layout(640, 480, 1024).unwrap().uv_offset > 307_200);
        assert_eq!(nv12_layout(641, 480, 1024), None);
        assert_eq!(nv12_layout(640, 481, 1024), None);
        assert_eq!(nv12_layout(640, 480, 639), None);
        assert_eq!(nv12_layout(0, 480, 640), None);
    }

    #[test]
    fn bt601_values() {
        assert_eq!(yuv([255, 255, 255]), (235, 128, 128));
        assert_eq!(yuv([0, 0, 0]), (16, 128, 128));
        let (y, cb, cr) = yuv([255, 0, 0]);
        assert_eq!((y, cb, cr), (82, 90, 240));
        let (y, cb, cr) = yuv([0, 0, 255]);
        assert_eq!((y, cb, cr), (41, 240, 110));
    }

    #[test]
    fn nv12_pattern_layout() {
        let (w, h, pitch) = (32u32, 16u32, 64u32);
        let layout = nv12_layout(w, h, pitch).unwrap();
        let mut buf = vec![0xeeu8; layout.total as usize];
        fill_nv12(&mut buf, &layout, w, h);
        let p = pitch as usize;
        // Luma: bars are 4 pixels wide; every row alike; padding untouched.
        for row in [0usize, 15] {
            assert_eq!(buf[row * p], 235, "white");
            assert_eq!(buf[row * p + 3], 235);
            assert_eq!(buf[row * p + 4], yuv(BARS[1]).0);
            assert_eq!(buf[row * p + 31], 16, "black");
            assert_eq!(buf[row * p + 32], 0xee, "padding");
        }
        // Chroma: interleaved Cb, Cr, half the rows, at the offset.
        let uv = layout.uv_offset as usize;
        assert_eq!((buf[uv], buf[uv + 1]), (128, 128));
        let (_, cb, cr) = yuv(BARS[5]);
        let red = uv + 5 * 4; // bar 5 starts at pixel 20, pair 10, byte 20
        assert_eq!((buf[red], buf[red + 1]), (cb, cr));
        let last_row = uv + 7 * p;
        assert_eq!((buf[last_row + 30], buf[last_row + 31]), (128, 128));
        assert_eq!(buf[last_row + 32], 0xee);
        // Between the planes nothing was written.
        assert_eq!(buf[15 * p + 40], 0xee);
        assert_eq!(buf[layout.total as usize - 1], 0xee);
        assert_eq!(uv + 8 * p, buf.len(), "eight chroma rows end the buffer");
    }

    #[test]
    fn rgb_pattern() {
        let mut buf = vec![0u8; 32 * 4 * 2];
        fill_rgb(&mut buf, 32 * 4, 32, 2);
        assert_eq!(&buf[0..4], &[255, 255, 255, 255]);
        // Bar 5 is red: B, G, R, A.
        assert_eq!(&buf[20 * 4..20 * 4 + 4], &[0, 0, 255, 255]);
        assert_eq!(&buf[31 * 4..31 * 4 + 4], &[0, 0, 0, 255]);
        assert_eq!(&buf[32 * 4..32 * 4 + 4], &[255, 255, 255, 255]);
        assert_eq!(bar_at(0, 640), 0);
        assert_eq!(bar_at(639, 640), 7);
        assert_eq!(bar_at(80, 640), 1);
    }

    #[test]
    fn output_lines() {
        assert_eq!(result_line("TEST_ONLY", Ok(())), "TEST_ONLY ok");
        assert_eq!(result_line("COMMIT", Err(22)), "COMMIT EINVAL");
        assert_eq!(result_line("RECOVER", Err(999)), "RECOVER errno999");
        let l =
            parse_layer("plane=42,format=NV12,src=640x480,dst=100,80,640x480,zpos=2,rotation=180")
                .unwrap();
        assert_eq!(
            layer_line(&l),
            "LAYER plane=42 format=NV12 src=640x480 dst=100,80,640x480 zpos=2 rotation=0x4 \
             alpha=none"
        );
    }

    fn layer(spec: &str) -> Layer {
        parse_layer(spec).unwrap()
    }

    #[test]
    fn parses_pattern() {
        let l = layer("plane=1,format=XR24,src=8x8,dst=0,0,8x8,pattern=quadrants");
        assert_eq!(l.pattern, Pattern::Quadrants);
        let l = layer("plane=1,format=XR24,src=8x8,dst=0,0,8x8,pattern=bars");
        assert_eq!(l.pattern, Pattern::Bars);
        assert_eq!(layer(NV12_LAYER).pattern, Pattern::Bars);
        assert!(parse_layer("plane=1,format=XR24,src=8x8,dst=0,0,8x8,pattern=dots").is_err());
        assert!(parse_layer("plane=1,format=XR24,src=8x8,dst=0,0,8x8,pattern=").is_err());
        let o = args("--layer plane=1,format=AR24,src=8x8,dst=0,0,8x8,pattern=quadrants").unwrap();
        assert_eq!(o.layers[0].pattern, Pattern::Quadrants);
        // Only quadrants change the LAYER line.
        assert_eq!(
            layer_line(&o.layers[0]),
            "LAYER plane=1 format=AR24 src=8x8 dst=0,0,8x8 zpos=none rotation=none alpha=none \
             pattern=quadrants"
        );
    }

    #[test]
    fn quadrant_rgb_pattern() {
        let (w, h) = (8u32, 4u32);
        let mut buf = vec![0u8; (w * h * 4) as usize];
        fill_rgb_quadrants(&mut buf, (w * 4) as usize, w, h);
        let px = |x: u32, y: u32| {
            let at = ((y * w + x) * 4) as usize;
            buf[at..at + 4].to_vec()
        };
        // B, G, R, A with opaque alpha.
        assert_eq!(px(0, 0), [0, 0, 255, 255], "top left red");
        assert_eq!(px(3, 1), [0, 0, 255, 255]);
        assert_eq!(px(4, 0), [0, 255, 0, 255], "top right green");
        assert_eq!(px(7, 1), [0, 255, 0, 255]);
        assert_eq!(px(0, 2), [255, 0, 0, 255], "bottom left blue");
        assert_eq!(px(3, 3), [255, 0, 0, 255]);
        assert_eq!(px(4, 2), [255, 255, 255, 255], "bottom right white");
        assert_eq!(px(7, 3), [255, 255, 255, 255]);
        // Padding after a row is left alone.
        let mut padded = vec![0xeeu8; 40 * 2];
        fill_rgb_quadrants(&mut padded, 40, 8, 2);
        assert_eq!(&padded[32..40], &[0xee; 8]);
        assert_eq!(quadrant_at(0, 0, 640, 480), 0);
        assert_eq!(quadrant_at(320, 239, 640, 480), 1);
        assert_eq!(quadrant_at(319, 240, 640, 480), 2);
        assert_eq!(quadrant_at(639, 479, 640, 480), 3);
    }

    #[test]
    fn quadrant_nv12_pattern() {
        let (w, h, pitch) = (32u32, 16u32, 64u32);
        let layout = nv12_layout(w, h, pitch).unwrap();
        let mut buf = vec![0xeeu8; layout.total as usize];
        fill_nv12_quadrants(&mut buf, &layout, w, h);
        let p = pitch as usize;
        let uv = layout.uv_offset as usize;
        for (q, (x, y)) in [(0, (0, 0)), (1, (16, 0)), (2, (0, 8)), (3, (31, 15))] {
            let (luma, cb, cr) = yuv(QUADRANTS[q]);
            assert_eq!(buf[y * p + x], luma, "luma of quadrant {q}");
            let at = uv + (y / 2) * p + (x / 2) * 2;
            assert_eq!((buf[at], buf[at + 1]), (cb, cr), "chroma of quadrant {q}");
        }
        // The BT.601 limited-range values the module doc names.
        assert_eq!(yuv(QUADRANTS[0]), (82, 90, 240), "red");
        assert_eq!(yuv(QUADRANTS[1]), (144, 54, 34), "green");
        assert_eq!(yuv(QUADRANTS[2]), (41, 240, 110), "blue");
        assert_eq!(yuv(QUADRANTS[3]), (235, 128, 128), "white");
        for (a, ca) in QUADRANTS.iter().enumerate() {
            for (b, cb) in QUADRANTS.iter().enumerate().skip(a + 1) {
                let (ya, cba, cra) = yuv(*ca);
                let (yb, cbb, crb) = yuv(*cb);
                let d = ya
                    .abs_diff(yb)
                    .max(cba.abs_diff(cbb))
                    .max(cra.abs_diff(crb));
                assert!(d >= 60, "{a} vs {b}: {d}");
            }
        }
        assert_eq!(buf[15 * p + 32], 0xee, "padding untouched");
        assert_eq!(uv + 8 * p, buf.len());
    }

    /// (name, x, y, rgb) of the EXPECT lines, in order.
    fn samples(lines: &[String]) -> Vec<(String, i64, i64, String)> {
        lines
            .iter()
            .map(|l| {
                let name = l.split('"').nth(1).unwrap().to_string();
                let field = |k: &str| {
                    l.split_whitespace()
                        .find_map(|t| t.strip_prefix(k))
                        .unwrap()
                        .to_string()
                };
                (
                    name,
                    field("x=").parse().unwrap(),
                    field("y=").parse().unwrap(),
                    field("rgb="),
                )
            })
            .collect()
    }

    fn quad(rotation: &str) -> Layer {
        layer(&format!(
            "plane=42,format=XR24,src=64x32,dst=100,40,400x200,pattern=quadrants,rotation={rotation}"
        ))
    }

    /// Colours at the screen's top left, top right, bottom left and bottom
    /// right sample for a rotation.
    fn screen_colours(rotation: &str) -> Vec<String> {
        let s = samples(&expect_lines(&[quad(rotation)], (800, 480)));
        assert_eq!(s.len(), 4, "{rotation}");
        let mut by_pos: Vec<_> = s.into_iter().map(|(_, x, y, c)| (y, x, c)).collect();
        by_pos.sort();
        by_pos.into_iter().map(|(_, _, c)| c).collect()
    }

    #[test]
    fn expect_positions_follow_the_transform() {
        let lines = expect_lines(&[quad("0")], (800, 480));
        assert_eq!(
            lines[0],
            "EXPECT phase=commit \"42-tl\" x=200 y=90 fx=0.2500 fy=0.1875 rgb=#ff0000"
        );
        let s = samples(&lines);
        let at = |name: &str| s.iter().find(|e| e.0 == name).map(|e| (e.1, e.2)).unwrap();
        assert_eq!(at("42-tr"), (400, 90));
        assert_eq!(at("42-bl"), (200, 190));
        assert_eq!(at("42-br"), (400, 190));
        // Buffer colours: tl red, tr green, bl blue, br white.
        let (r, g, b, w) = ("#ff0000", "#00ff00", "#0000ff", "#ffffff");
        let want = |c: [&str; 4]| c.map(String::from).to_vec();
        assert_eq!(screen_colours("0"), want([r, g, b, w]));
        assert_eq!(screen_colours("180"), want([w, b, g, r]));
        assert_eq!(screen_colours("reflect-x"), want([g, r, w, b]));
        assert_eq!(screen_colours("reflect-y"), want([b, w, r, g]));
        assert_eq!(screen_colours("reflect-x+reflect-y"), want([w, b, g, r]));
        assert_eq!(screen_colours("180+reflect-x"), screen_colours("reflect-y"));
        // 90 is counter-clockwise: the top right goes to the top left.
        assert_eq!(screen_colours("90"), want([g, w, r, b]));
        assert_eq!(screen_colours("270"), want([b, r, w, g]));
        // 0, 180, reflect-x and reflect-y are pairwise different pictures.
        let all = ["0", "180", "reflect-x", "reflect-y"].map(screen_colours);
        for (i, pi) in all.iter().enumerate() {
            for pj in all.iter().skip(i + 1) {
                assert_ne!(pi, pj);
            }
        }
    }

    #[test]
    fn expect_leaves_out_covered_and_off_screen_samples() {
        let low = layer("plane=31,format=XR24,src=64x32,dst=0,0,400x200,zpos=1,pattern=quadrants");
        let high = layer("plane=42,format=AR24,src=64x32,dst=0,0,200x100,zpos=2,pattern=quadrants");
        // The high layer covers the low layer's top left centre (100, 50).
        let s = samples(&expect_lines(&[low.clone(), high.clone()], (800, 480)));
        let names: Vec<_> = s.iter().map(|e| e.0.as_str()).collect();
        assert_eq!(
            names,
            ["31-tr", "31-bl", "31-br", "42-tl", "42-tr", "42-bl", "42-br"]
        );
        // Equal or unknown zpos: no sample under the other layer is predictable.
        let mut same = high.clone();
        same.zpos = Some(1);
        assert_eq!(expect_lines(&[low.clone(), same], (800, 480)).len(), 3);
        let mut none = high;
        none.zpos = None;
        assert_eq!(expect_lines(&[low.clone(), none], (800, 480)).len(), 3);
        // Off the output.
        assert_eq!(expect_lines(&[low], (150, 480)).len(), 2);
    }

    #[test]
    fn expect_skips_bars_and_plane_alpha() {
        let bars = layer("plane=31,format=XR24,src=64x32,dst=0,0,64x32");
        let translucent =
            layer("plane=42,format=AR24,src=64x32,dst=0,0,64x32,alpha=32768,pattern=quadrants");
        let opaque =
            layer("plane=49,format=AR24,src=64x32,dst=0,0,64x32,alpha=65535,pattern=quadrants");
        assert_eq!(
            expect_lines(&[bars], (800, 480)),
            ["EXPECT phase=commit skipped reason=bars-pattern plane=31"]
        );
        assert_eq!(
            expect_lines(&[translucent], (800, 480)),
            ["EXPECT phase=commit skipped reason=plane-alpha plane=42"]
        );
        assert_eq!(expect_lines(&[opaque], (800, 480)).len(), 4);
    }

    #[test]
    fn expect_lines_are_read_by_twoplane_expect() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("python3 not found: skipped");
            return;
        }
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/twoplane-expect.py");
        let dir = std::env::temp_dir().join(format!("plane_commit_expect_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("input.txt");
        let mut text = expect_lines(&[quad("reflect-y")], (800, 480)).join("\n");
        text.push('\n');
        std::fs::write(&input, text).unwrap();
        let out = std::process::Command::new("python3")
            .arg("-I")
            .arg(&script)
            .arg(&input)
            .arg("--out")
            .arg(&dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let json = std::fs::read_to_string(dir.join("expect-commit.json")).unwrap();
        assert!(json.contains("\"42-tl\""), "{json}");
        assert!(json.contains("\"expect\""), "{json}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
