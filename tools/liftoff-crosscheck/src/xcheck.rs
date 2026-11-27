//! Reader and writer of the `xcheck` v1 format (design section 5).
//!
//! One text file describes a topology and any number of frames. The format is
//! line based, whitespace separated, with `key=value` fields and `#` comments
//! (a `#` starts a comment anywhere in a line, so free text must not contain
//! one). The reader has no dependencies and **refuses** what it cannot hold:
//! a file the engine's limits cannot represent is an error, never truncated.
//!
//! Record order (each record may refer only to earlier ones):
//! `xcheck 1`, `origin`, optional `profile`, `output`s, `plane`s (each followed
//! by its `format` lines), `group`s, `buffer`s, `frame`s (each holding `layer`
//! lines and closed by `end`). The writer emits exactly this order.
//!
//! Refusals (all carry the line number): no `xcheck 1` header; no `origin`; a
//! `N..M` zpos without `zpos_now=`; `profile imx95dpu` together with explicit
//! `scaling=`, `max_source=` or `group` lines; more outputs, planes, formats
//! per plane, buffers, groups or layers per frame than the limits of
//! [`composition_core::limits`]; duplicate ids; references to unknown ids.
//!
//! `zpos_now=` on a fixed-zpos plane is accepted and kept but has no meaning.
//! `read(write(x)) == x` holds for every `x` that [`write`] accepts; comments
//! are not part of the model and are dropped.

use composition_core::limits::{
    MAX_BUFFERS, MAX_FORMATS, MAX_LAYERS, MAX_OUTPUTS, MAX_PLANES, MAX_RESOURCE_GROUPS,
};
use std::collections::HashSet;
use std::fmt;

use crate::state::{rotation, BlendMode, Rect, URect};

/// Where the described topology comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginKind {
    /// Derived from an observed document (names it and its date).
    Observed,
    /// Derived from source code.
    Source,
    /// Made up; says what it imitates. Gives no hardware claim.
    Synthetic,
}

/// The mandatory `origin` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    /// Kind of origin.
    pub kind: OriginKind,
    /// Date `YYYY-MM-DD`.
    pub date: String,
    /// Free text (no `#`, no addresses or hostnames).
    pub text: String,
}

/// Which `DriverProfile` the core side applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// No profile.
    Generic,
    /// `DriverProfile::Imx95Dpu`.
    Imx95Dpu,
}

/// An `output` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    /// Connector/CRTC id as the file names it.
    pub id: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Refresh rate in millihertz.
    pub refresh_mhz: u32,
}

/// DRM plane type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaneKind {
    /// Primary plane.
    Primary,
    /// Overlay plane.
    Overlay,
    /// Cursor plane.
    Cursor,
}

/// Z position of a plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZposDef {
    /// Immutable position.
    Fixed(i32),
    /// Programmable inclusive range.
    Range {
        /// Lowest value.
        min: i32,
        /// Highest value.
        max: i32,
    },
}

/// Set of blend modes (`opaque,premul,coverage` or `-`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlendSet(pub u8);

impl BlendSet {
    /// Bit of one mode.
    pub const fn bit(mode: BlendMode) -> u8 {
        match mode {
            BlendMode::Opaque => 1,
            BlendMode::Premultiplied => 2,
            BlendMode::Coverage => 4,
        }
    }

    /// Whether the set contains `mode`.
    pub const fn contains(self, mode: BlendMode) -> bool {
        self.0 & Self::bit(mode) != 0
    }
}

/// Set of resource consumers of a group (`any,scaled_h,scaled_v,yuv`);
/// the bits are those of `composition_core::caps::LayerTraits`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Consumers(pub u8);

impl Consumers {
    /// Every layer.
    pub const ANY: u8 = 1 << 0;
    /// Horizontally scaled layers.
    pub const SCALED_H: u8 = 1 << 1;
    /// Vertically scaled layers.
    pub const SCALED_V: u8 = 1 << 2;
    /// YUV layers.
    pub const YUV: u8 = 1 << 3;
}

/// Scaling limits, 16.16 fixed point (`scaling=up:<hex>/down:<hex>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScaleDef {
    /// Maximum upscale factor.
    pub up: u32,
    /// Maximum downscale factor.
    pub down: u32,
}

/// A format/modifier pair of a plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormatDef {
    /// DRM fourcc as four ASCII characters.
    pub fourcc: [u8; 4],
    /// Format modifier.
    pub modifier: u64,
}

/// A `plane` line with its `format` lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plane {
    /// Plane id.
    pub id: u32,
    /// Plane type.
    pub kind: PlaneKind,
    /// Mask over the `output` lines (bit i = i-th output).
    pub outputs: u32,
    /// Zpos definition.
    pub zpos: ZposDef,
    /// Current zpos (`zpos_now=`), mandatory for a range.
    pub zpos_now: Option<i32>,
    /// Plane alpha property exists.
    pub alpha: bool,
    /// Blend modes the plane lists.
    pub blend: BlendSet,
    /// Rotation bits the plane lists (see [`rotation`]).
    pub transforms: u32,
    /// Scaling limits, if explicit.
    pub scaling: Option<ScaleDef>,
    /// Maximum source size `(w, h)`, if explicit.
    pub max_source: Option<(u32, u32)>,
    /// The plane may only cover the whole output.
    pub full_output: bool,
    /// Accepted format/modifier pairs in dump order.
    pub formats: Vec<FormatDef>,
}

/// A `group` line: a shared resource pool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// Layers that consume the resource.
    pub consumers: Consumers,
    /// Plane ids in the group.
    pub planes: Vec<u32>,
    /// Number of consumers the pool holds.
    pub capacity: u32,
}

/// A `buffer` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferDef {
    /// Buffer id.
    pub id: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// DRM fourcc.
    pub fourcc: [u8; 4],
    /// Format modifier.
    pub modifier: u64,
}

/// A `layer` line of a frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    /// Buffer id.
    pub buffer: u32,
    /// Source rectangle in whole pixels.
    pub src: URect,
    /// Destination rectangle.
    pub dst: Rect,
    /// Z order.
    pub z: i32,
    /// Alpha, 0..=65535.
    pub alpha: u16,
    /// Blend mode.
    pub blend: BlendMode,
    /// Transform as one rotation bit, see [`rotation`].
    pub transform: u32,
}

/// A `frame`..`end` block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// Name (no whitespace).
    pub name: String,
    /// Output id the frame targets.
    pub output: u32,
    /// Layers in frame order.
    pub layers: Vec<Layer>,
}

/// A whole `xcheck` file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XCheck {
    /// Origin of the data.
    pub origin: Origin,
    /// Driver profile (default `generic`).
    pub profile: Profile,
    /// Outputs; order is the core output index.
    pub outputs: Vec<Output>,
    /// Planes.
    pub planes: Vec<Plane>,
    /// Resource groups.
    pub groups: Vec<Group>,
    /// Buffers.
    pub buffers: Vec<BufferDef>,
    /// Frames.
    pub frames: Vec<Frame>,
}

/// Why a file was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// First record is not `xcheck 1`.
    MissingHeader,
    /// No `origin` line where one is required.
    MissingOrigin,
    /// A `N..M` zpos without `zpos_now=`.
    RangeZposWithoutNow,
    /// `profile imx95dpu` together with explicit `scaling`, `max_source`
    /// or `group` lines.
    ProfileConflict,
    /// More of something than the engine can hold.
    Limit {
        /// What overflowed.
        what: &'static str,
        /// The limit.
        max: usize,
    },
    /// An id or name defined twice.
    Duplicate(String),
    /// A reference to something not defined earlier.
    UnknownRef(String),
    /// A frame not closed by `end` before the end of the file.
    UnterminatedFrame,
    /// Anything else malformed.
    Syntax(String),
}

/// A refusal with its (1-based) line number; 0 when no line applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// Line number, 1-based.
    pub line: usize,
    /// Reason.
    pub kind: ErrorKind,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "xcheck line {}: ", self.line)?;
        match &self.kind {
            ErrorKind::MissingHeader => write!(f, "first record must be `xcheck 1`"),
            ErrorKind::MissingOrigin => write!(f, "the `origin` line is mandatory"),
            ErrorKind::RangeZposWithoutNow => {
                write!(f, "a range zpos needs zpos_now=")
            }
            ErrorKind::ProfileConflict => write!(
                f,
                "`profile imx95dpu` cannot be combined with explicit scaling, max_source or group lines"
            ),
            ErrorKind::Limit { what, max } => write!(f, "more than {max} {what}"),
            ErrorKind::Duplicate(s) => write!(f, "duplicate {s}"),
            ErrorKind::UnknownRef(s) => write!(f, "unknown {s}"),
            ErrorKind::UnterminatedFrame => write!(f, "frame not closed by `end`"),
            ErrorKind::Syntax(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {}

fn err<T>(line: usize, kind: ErrorKind) -> Result<T, Error> {
    Err(Error { line, kind })
}

fn syntax<T>(line: usize, msg: impl Into<String>) -> Result<T, Error> {
    err(line, ErrorKind::Syntax(msg.into()))
}

// ---------------------------------------------------------------- names

const TRANSFORM_NAMES: [(&str, u32); 6] = [
    ("normal", rotation::ROTATE_0),
    ("rot90", rotation::ROTATE_90),
    ("rot180", rotation::ROTATE_180),
    ("rot270", rotation::ROTATE_270),
    ("flip_h", rotation::REFLECT_X),
    ("flip_v", rotation::REFLECT_Y),
];

const BLEND_NAMES: [(&str, BlendMode); 3] = [
    ("opaque", BlendMode::Opaque),
    ("premul", BlendMode::Premultiplied),
    ("coverage", BlendMode::Coverage),
];

const CONSUMER_NAMES: [(&str, u8); 4] = [
    ("any", Consumers::ANY),
    ("scaled_h", Consumers::SCALED_H),
    ("scaled_v", Consumers::SCALED_V),
    ("yuv", Consumers::YUV),
];

fn blend_name(m: BlendMode) -> &'static str {
    BLEND_NAMES
        .iter()
        .find(|(_, b)| *b == m)
        .map(|(n, _)| *n)
        .unwrap_or("opaque")
}

// ---------------------------------------------------------------- scalar parsing

fn num<T: std::str::FromStr>(line: usize, what: &str, s: &str) -> Result<T, Error> {
    match s.parse::<T>() {
        Ok(v) => Ok(v),
        Err(_) => syntax(line, format!("bad {what} `{s}`")),
    }
}

fn hex_u64(line: usize, what: &str, s: &str) -> Result<u64, Error> {
    match s.strip_prefix("0x").map(|h| u64::from_str_radix(h, 16)) {
        Some(Ok(v)) => Ok(v),
        _ => syntax(line, format!("bad {what} `{s}` (want 0x hex)")),
    }
}

fn hex_u32(line: usize, what: &str, s: &str) -> Result<u32, Error> {
    let v = hex_u64(line, what, s)?;
    match u32::try_from(v) {
        Ok(v) => Ok(v),
        Err(_) => syntax(line, format!("{what} `{s}` does not fit 32 bits")),
    }
}

fn fourcc(line: usize, s: &str) -> Result<[u8; 4], Error> {
    let b = s.as_bytes();
    if b.len() == 4 && b.iter().all(|c| c.is_ascii_graphic() && *c != b'#') {
        Ok([b[0], b[1], b[2], b[3]])
    } else {
        syntax(line, format!("bad fourcc `{s}`"))
    }
}

fn fourcc_str(f: [u8; 4]) -> String {
    f.iter().map(|&c| c as char).collect()
}

fn bool01(line: usize, what: &str, s: &str) -> Result<bool, Error> {
    match s {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => syntax(line, format!("bad {what} `{s}` (want 0 or 1)")),
    }
}

/// Parse `a,b,c,d` into four values.
fn quad<T: std::str::FromStr>(line: usize, what: &str, s: &str) -> Result<[T; 4], Error> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return syntax(line, format!("bad {what} `{s}` (want x,y,w,h)"));
    }
    Ok([
        num(line, what, parts[0])?,
        num(line, what, parts[1])?,
        num(line, what, parts[2])?,
        num(line, what, parts[3])?,
    ])
}

/// Parse a comma list of names, or `-` for empty, into a bit set.
fn name_set<T: Copy>(
    line: usize,
    what: &str,
    s: &str,
    names: &[(&str, T)],
    bit: impl Fn(T) -> u32,
    allow_empty: bool,
) -> Result<u32, Error> {
    if s == "-" {
        return if allow_empty {
            Ok(0)
        } else {
            syntax(line, format!("{what} must not be empty"))
        };
    }
    let mut bits = 0;
    for part in s.split(',') {
        match names.iter().find(|(n, _)| *n == part) {
            Some((_, v)) => bits |= bit(*v),
            None => return syntax(line, format!("unknown {what} `{part}`")),
        }
    }
    Ok(bits)
}

fn parse_zpos(line: usize, s: &str) -> Result<ZposDef, Error> {
    if let Some(n) = s.strip_prefix("fixed:") {
        return Ok(ZposDef::Fixed(num(line, "zpos", n)?));
    }
    match s.split_once("..") {
        Some((a, b)) => {
            let (min, max): (i32, i32) = (num(line, "zpos", a)?, num(line, "zpos", b)?);
            if min > max {
                return syntax(line, format!("empty zpos range `{s}`"));
            }
            Ok(ZposDef::Range { min, max })
        }
        None => syntax(line, format!("bad zpos `{s}` (want fixed:N or N..M)")),
    }
}

fn parse_scaling(line: usize, s: &str) -> Result<Option<ScaleDef>, Error> {
    if s == "none" {
        return Ok(None);
    }
    let bad = || {
        syntax(
            line,
            format!("bad scaling `{s}` (want none or up:<hex>/down:<hex>)"),
        )
    };
    let Some((up, down)) = s.split_once('/') else {
        return bad();
    };
    let (Some(up), Some(down)) = (up.strip_prefix("up:"), down.strip_prefix("down:")) else {
        return bad();
    };
    Ok(Some(ScaleDef {
        up: hex_u32(line, "scaling", up)?,
        down: hex_u32(line, "scaling", down)?,
    }))
}

fn parse_max_source(line: usize, s: &str) -> Result<Option<(u32, u32)>, Error> {
    if s == "none" {
        return Ok(None);
    }
    match s.split_once('x') {
        Some((w, h)) => Ok(Some((
            num(line, "max_source", w)?,
            num(line, "max_source", h)?,
        ))),
        None => syntax(line, format!("bad max_source `{s}` (want WxH or none)")),
    }
}

fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let month: u32 = s[5..7].parse().unwrap_or(0);
    let day: u32 = s[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

// ---------------------------------------------------------------- key=value fields

struct Fields<'a> {
    line: usize,
    items: Vec<(&'a str, &'a str)>,
}

impl<'a> Fields<'a> {
    fn new(line: usize, toks: &[&'a str]) -> Result<Self, Error> {
        let mut items: Vec<(&str, &str)> = Vec::new();
        for t in toks {
            let Some((k, v)) = t.split_once('=') else {
                return syntax(line, format!("expected key=value, got `{t}`"));
            };
            if items.iter().any(|(ek, _)| *ek == k) {
                return err(line, ErrorKind::Duplicate(format!("field `{k}`")));
            }
            items.push((k, v));
        }
        Ok(Self { line, items })
    }

    fn take(&mut self, key: &str) -> Option<&'a str> {
        let i = self.items.iter().position(|(k, _)| *k == key)?;
        Some(self.items.remove(i).1)
    }

    fn need(&mut self, key: &str) -> Result<&'a str, Error> {
        match self.take(key) {
            Some(v) => Ok(v),
            None => syntax(self.line, format!("missing field `{key}`")),
        }
    }

    fn finish(self) -> Result<(), Error> {
        match self.items.first() {
            Some((k, _)) => syntax(self.line, format!("unknown field `{k}`")),
            None => Ok(()),
        }
    }
}

// ---------------------------------------------------------------- reader

/// Parse an `xcheck` v1 text.
pub fn read(text: &str) -> Result<XCheck, Error> {
    Reader::default().run(text)
}

#[derive(Default)]
struct Reader {
    outputs: Vec<Output>,
    planes: Vec<Plane>,
    groups: Vec<Group>,
    buffers: Vec<BufferDef>,
    frames: Vec<Frame>,
    open_frame: Option<(usize, Frame)>,
    frame_names: HashSet<String>,
    profile: Option<(usize, Profile)>,
    first_explicit: Option<usize>,
}

impl Reader {
    fn run(mut self, text: &str) -> Result<XCheck, Error> {
        let mut origin: Option<Origin> = None;
        let mut seen_header = false;
        let mut last_line = 0;
        for (idx, raw) in text.lines().enumerate() {
            let line = idx + 1;
            last_line = line;
            let content = raw.split('#').next().unwrap_or("").trim();
            if content.is_empty() {
                continue;
            }
            let toks: Vec<&str> = content.split_whitespace().collect();
            if !seen_header {
                if toks != ["xcheck", "1"] {
                    return err(line, ErrorKind::MissingHeader);
                }
                seen_header = true;
                continue;
            }
            if origin.is_none() {
                if toks[0] != "origin" {
                    return err(line, ErrorKind::MissingOrigin);
                }
                origin = Some(parse_origin(line, content)?);
                continue;
            }
            self.record(line, &toks)?;
        }
        if !seen_header {
            return err(0, ErrorKind::MissingHeader);
        }
        let Some(origin) = origin else {
            return err(last_line, ErrorKind::MissingOrigin);
        };
        if let Some((line, _)) = self.open_frame {
            return err(line, ErrorKind::UnterminatedFrame);
        }
        let profile = self.profile.map_or(Profile::Generic, |(_, p)| p);
        if let (Some((pline, Profile::Imx95Dpu)), Some(eline)) = (self.profile, self.first_explicit)
        {
            return err(pline.max(eline), ErrorKind::ProfileConflict);
        }
        Ok(XCheck {
            origin,
            profile,
            outputs: self.outputs,
            planes: self.planes,
            groups: self.groups,
            buffers: self.buffers,
            frames: self.frames,
        })
    }

    fn record(&mut self, line: usize, toks: &[&str]) -> Result<(), Error> {
        if self.open_frame.is_some() {
            return match toks[0] {
                "layer" => self.layer(line, &toks[1..]),
                "end" if toks.len() == 1 => {
                    if let Some((_, f)) = self.open_frame.take() {
                        self.frames.push(f);
                    }
                    Ok(())
                }
                other => syntax(
                    line,
                    format!("`{other}` inside a frame (want layer or end)"),
                ),
            };
        }
        match toks[0] {
            "origin" => err(line, ErrorKind::Duplicate("origin".into())),
            "profile" => self.profile_line(line, &toks[1..]),
            "output" => self.output(line, &toks[1..]),
            "plane" => self.plane(line, &toks[1..]),
            "format" => self.format(line, &toks[1..]),
            "group" => self.group(line, &toks[1..]),
            "buffer" => self.buffer(line, &toks[1..]),
            "frame" => self.frame(line, &toks[1..]),
            other => syntax(line, format!("unknown record `{other}`")),
        }
    }

    fn profile_line(&mut self, line: usize, a: &[&str]) -> Result<(), Error> {
        if self.profile.is_some() {
            return err(line, ErrorKind::Duplicate("profile".into()));
        }
        let p = match a {
            ["generic"] => Profile::Generic,
            ["imx95dpu"] => Profile::Imx95Dpu,
            _ => return syntax(line, "bad profile (want generic or imx95dpu)"),
        };
        self.profile = Some((line, p));
        Ok(())
    }

    fn output(&mut self, line: usize, a: &[&str]) -> Result<(), Error> {
        let [id, w, h, r] = a else {
            return syntax(line, "output wants <id> <w> <h> <refresh_mhz>");
        };
        if !self.planes.is_empty() {
            return syntax(line, "output after plane lines");
        }
        if self.outputs.len() >= MAX_OUTPUTS {
            return err(
                line,
                ErrorKind::Limit {
                    what: "outputs",
                    max: MAX_OUTPUTS,
                },
            );
        }
        let o = Output {
            id: num(line, "output id", id)?,
            width: num(line, "width", w)?,
            height: num(line, "height", h)?,
            refresh_mhz: num(line, "refresh", r)?,
        };
        if self.outputs.iter().any(|e| e.id == o.id) {
            return err(line, ErrorKind::Duplicate(format!("output {}", o.id)));
        }
        self.outputs.push(o);
        Ok(())
    }

    fn plane(&mut self, line: usize, a: &[&str]) -> Result<(), Error> {
        let Some((id, rest)) = a.split_first() else {
            return syntax(line, "plane wants <id> key=value...");
        };
        let id: u32 = num(line, "plane id", id)?;
        if self.planes.len() >= MAX_PLANES {
            return err(
                line,
                ErrorKind::Limit {
                    what: "planes",
                    max: MAX_PLANES,
                },
            );
        }
        if self.planes.iter().any(|p| p.id == id) {
            return err(line, ErrorKind::Duplicate(format!("plane {id}")));
        }
        let mut f = Fields::new(line, rest)?;
        let kind = match f.need("kind")? {
            "primary" => PlaneKind::Primary,
            "overlay" => PlaneKind::Overlay,
            "cursor" => PlaneKind::Cursor,
            k => return syntax(line, format!("bad kind `{k}`")),
        };
        let outputs = hex_u32(line, "outputs", f.need("outputs")?)?;
        if (outputs as u64) >> self.outputs.len() != 0 {
            return err(
                line,
                ErrorKind::UnknownRef(format!(
                    "output in mask 0x{outputs:x} ({} defined)",
                    self.outputs.len()
                )),
            );
        }
        let zpos = parse_zpos(line, f.need("zpos")?)?;
        let zpos_now = match f.take("zpos_now") {
            Some(s) => Some(num(line, "zpos_now", s)?),
            None => None,
        };
        if matches!(zpos, ZposDef::Range { .. }) && zpos_now.is_none() {
            return err(line, ErrorKind::RangeZposWithoutNow);
        }
        let alpha = bool01(line, "alpha", f.need("alpha")?)?;
        let blend = BlendSet(name_set(
            line,
            "blend mode",
            f.need("blend")?,
            &BLEND_NAMES,
            |m| BlendSet::bit(m) as u32,
            true,
        )? as u8);
        let transforms = name_set(
            line,
            "transform",
            f.need("transforms")?,
            &TRANSFORM_NAMES,
            |b| b,
            true,
        )?;
        let scaling = match f.take("scaling") {
            Some(s) => parse_scaling(line, s)?,
            None => None,
        };
        let max_source = match f.take("max_source") {
            Some(s) => parse_max_source(line, s)?,
            None => None,
        };
        let full_output = match f.take("full_output") {
            Some(s) => bool01(line, "full_output", s)?,
            None => false,
        };
        f.finish()?;
        if scaling.is_some() || max_source.is_some() {
            self.first_explicit.get_or_insert(line);
        }
        self.planes.push(Plane {
            id,
            kind,
            outputs,
            zpos,
            zpos_now,
            alpha,
            blend,
            transforms,
            scaling,
            max_source,
            full_output,
            formats: Vec::new(),
        });
        Ok(())
    }

    fn format(&mut self, line: usize, a: &[&str]) -> Result<(), Error> {
        let [fc, modifier] = a else {
            return syntax(line, "format wants <fourcc> <modifier>");
        };
        if !self.groups.is_empty() || !self.buffers.is_empty() || !self.frames.is_empty() {
            return syntax(line, "format line not directly after its plane");
        }
        let Some(plane) = self.planes.last_mut() else {
            return syntax(line, "format line before any plane");
        };
        if plane.formats.len() >= MAX_FORMATS {
            return err(
                line,
                ErrorKind::Limit {
                    what: "formats per plane",
                    max: MAX_FORMATS,
                },
            );
        }
        plane.formats.push(FormatDef {
            fourcc: fourcc(line, fc)?,
            modifier: hex_u64(line, "modifier", modifier)?,
        });
        Ok(())
    }

    fn group(&mut self, line: usize, a: &[&str]) -> Result<(), Error> {
        if self.groups.len() >= MAX_RESOURCE_GROUPS {
            return err(
                line,
                ErrorKind::Limit {
                    what: "groups",
                    max: MAX_RESOURCE_GROUPS,
                },
            );
        }
        if !self.buffers.is_empty() || !self.frames.is_empty() {
            return syntax(line, "group after buffer or frame lines");
        }
        let mut f = Fields::new(line, a)?;
        let consumers = Consumers(name_set(
            line,
            "consumer",
            f.need("consumers")?,
            &CONSUMER_NAMES,
            |b| b as u32,
            false,
        )? as u8);
        let mut planes = Vec::new();
        for p in f.need("planes")?.split(',') {
            let id: u32 = num(line, "plane id", p)?;
            if !self.planes.iter().any(|e| e.id == id) {
                return err(line, ErrorKind::UnknownRef(format!("plane {id}")));
            }
            if planes.contains(&id) {
                return err(line, ErrorKind::Duplicate(format!("plane {id} in group")));
            }
            planes.push(id);
        }
        let capacity = num(line, "capacity", f.need("capacity")?)?;
        f.finish()?;
        self.first_explicit.get_or_insert(line);
        self.groups.push(Group {
            consumers,
            planes,
            capacity,
        });
        Ok(())
    }

    fn buffer(&mut self, line: usize, a: &[&str]) -> Result<(), Error> {
        let [id, w, h, fc, modifier] = a else {
            return syntax(line, "buffer wants <id> <w> <h> <fourcc> <modifier>");
        };
        if !self.frames.is_empty() {
            return syntax(line, "buffer after frame lines");
        }
        if self.buffers.len() >= MAX_BUFFERS {
            return err(
                line,
                ErrorKind::Limit {
                    what: "buffers",
                    max: MAX_BUFFERS,
                },
            );
        }
        let b = BufferDef {
            id: num(line, "buffer id", id)?,
            width: num(line, "width", w)?,
            height: num(line, "height", h)?,
            fourcc: fourcc(line, fc)?,
            modifier: hex_u64(line, "modifier", modifier)?,
        };
        if self.buffers.iter().any(|e| e.id == b.id) {
            return err(line, ErrorKind::Duplicate(format!("buffer {}", b.id)));
        }
        self.buffers.push(b);
        Ok(())
    }

    fn frame(&mut self, line: usize, a: &[&str]) -> Result<(), Error> {
        let Some((name, rest)) = a.split_first() else {
            return syntax(line, "frame wants <name> output=<id>");
        };
        let mut f = Fields::new(line, rest)?;
        let output: u32 = num(line, "output id", f.need("output")?)?;
        f.finish()?;
        if !self.outputs.iter().any(|o| o.id == output) {
            return err(line, ErrorKind::UnknownRef(format!("output {output}")));
        }
        if !self.frame_names.insert((*name).to_string()) {
            return err(line, ErrorKind::Duplicate(format!("frame {name}")));
        }
        self.open_frame = Some((
            line,
            Frame {
                name: (*name).to_string(),
                output,
                layers: Vec::new(),
            },
        ));
        Ok(())
    }

    fn layer(&mut self, line: usize, a: &[&str]) -> Result<(), Error> {
        let mut f = Fields::new(line, a)?;
        let buffer: u32 = num(line, "buffer id", f.need("buffer")?)?;
        if !self.buffers.iter().any(|b| b.id == buffer) {
            return err(line, ErrorKind::UnknownRef(format!("buffer {buffer}")));
        }
        let [sx, sy, sw, sh] = quad::<u32>(line, "src", f.need("src")?)?;
        let dst_s = f.need("dst")?;
        let dst_p: Vec<&str> = dst_s.split(',').collect();
        if dst_p.len() != 4 {
            return syntax(line, format!("bad dst `{dst_s}` (want x,y,w,h)"));
        }
        let dst = Rect {
            x: num(line, "dst", dst_p[0])?,
            y: num(line, "dst", dst_p[1])?,
            w: num(line, "dst", dst_p[2])?,
            h: num(line, "dst", dst_p[3])?,
        };
        let z = num(line, "z", f.need("z")?)?;
        let alpha = num(line, "alpha", f.need("alpha")?)?;
        let blend_s = f.need("blend")?;
        let blend = match BLEND_NAMES.iter().find(|(n, _)| *n == blend_s) {
            Some((_, b)) => *b,
            None => return syntax(line, format!("bad blend `{blend_s}`")),
        };
        let tr_s = f.need("transform")?;
        let transform = match TRANSFORM_NAMES.iter().find(|(n, _)| *n == tr_s) {
            Some((_, b)) => *b,
            None => return syntax(line, format!("bad transform `{tr_s}`")),
        };
        f.finish()?;
        let Some((_, frame)) = self.open_frame.as_mut() else {
            return syntax(line, "layer outside a frame");
        };
        if frame.layers.len() >= MAX_LAYERS {
            return err(
                line,
                ErrorKind::Limit {
                    what: "layers per frame",
                    max: MAX_LAYERS,
                },
            );
        }
        frame.layers.push(Layer {
            buffer,
            src: URect {
                x: sx,
                y: sy,
                w: sw,
                h: sh,
            },
            dst,
            z,
            alpha,
            blend,
            transform,
        });
        Ok(())
    }
}

fn parse_origin(line: usize, content: &str) -> Result<Origin, Error> {
    let rest = content.strip_prefix("origin").unwrap_or("").trim_start();
    let mut it = rest.splitn(3, char::is_whitespace);
    let kind = match it.next().unwrap_or("") {
        "observed" => OriginKind::Observed,
        "source" => OriginKind::Source,
        "synthetic" => OriginKind::Synthetic,
        k => return syntax(line, format!("bad origin kind `{k}`")),
    };
    let date = it.next().unwrap_or("");
    if !valid_date(date) {
        return syntax(line, format!("bad origin date `{date}` (want YYYY-MM-DD)"));
    }
    let text = it.next().unwrap_or("").trim();
    if text.is_empty() {
        return syntax(line, "origin needs free text naming its source");
    }
    Ok(Origin {
        kind,
        date: date.to_string(),
        text: text.to_string(),
    })
}

// ---------------------------------------------------------------- writer

fn set_names<T: Copy>(bits: u32, names: &[(&str, T)], bit: impl Fn(T) -> u32) -> String {
    let v: Vec<&str> = names
        .iter()
        .filter(|(_, t)| bit(*t) & bits != 0)
        .map(|(n, _)| *n)
        .collect();
    if v.is_empty() {
        "-".to_string()
    } else {
        v.join(",")
    }
}

/// Serialize to `xcheck` v1 text.
///
/// The text is read back and compared with `x`; a model the format cannot
/// represent (a `#` or newline in free text, a name with whitespace, a limit
/// exceeded, a dangling reference, ...) is an error instead of unreadable
/// output, so `read(write(x)?) == x` always holds.
pub fn write(x: &XCheck) -> Result<String, Error> {
    use std::fmt::Write as _;
    let mut s = String::from("xcheck 1\n");
    let kind = match x.origin.kind {
        OriginKind::Observed => "observed",
        OriginKind::Source => "source",
        OriginKind::Synthetic => "synthetic",
    };
    let _ = writeln!(s, "origin {kind} {} {}", x.origin.date, x.origin.text);
    let profile = match x.profile {
        Profile::Generic => "generic",
        Profile::Imx95Dpu => "imx95dpu",
    };
    let _ = writeln!(s, "profile {profile}");
    for o in &x.outputs {
        let _ = writeln!(
            s,
            "output {} {} {} {}",
            o.id, o.width, o.height, o.refresh_mhz
        );
    }
    for p in &x.planes {
        let kind = match p.kind {
            PlaneKind::Primary => "primary",
            PlaneKind::Overlay => "overlay",
            PlaneKind::Cursor => "cursor",
        };
        let _ = write!(s, "plane {} kind={kind} outputs=0x{:x}", p.id, p.outputs);
        match p.zpos {
            ZposDef::Fixed(n) => {
                let _ = write!(s, " zpos=fixed:{n}");
            }
            ZposDef::Range { min, max } => {
                let _ = write!(s, " zpos={min}..{max}");
            }
        }
        if let Some(n) = p.zpos_now {
            let _ = write!(s, " zpos_now={n}");
        }
        let _ = write!(
            s,
            " alpha={} blend={} transforms={}",
            u8::from(p.alpha),
            set_names(p.blend.0 as u32, &BLEND_NAMES, |m| BlendSet::bit(m) as u32),
            set_names(p.transforms, &TRANSFORM_NAMES, |b| b),
        );
        match p.scaling {
            Some(c) => {
                let _ = write!(s, " scaling=up:0x{:x}/down:0x{:x}", c.up, c.down);
            }
            None => s.push_str(" scaling=none"),
        }
        match p.max_source {
            Some((w, h)) => {
                let _ = write!(s, " max_source={w}x{h}");
            }
            None => s.push_str(" max_source=none"),
        }
        let _ = writeln!(s, " full_output={}", u8::from(p.full_output));
        for f in &p.formats {
            let _ = writeln!(s, "  format {} 0x{:x}", fourcc_str(f.fourcc), f.modifier);
        }
    }
    for g in &x.groups {
        let planes: Vec<String> = g.planes.iter().map(u32::to_string).collect();
        let _ = writeln!(
            s,
            "group consumers={} planes={} capacity={}",
            set_names(g.consumers.0 as u32, &CONSUMER_NAMES, |b| b as u32),
            planes.join(","),
            g.capacity
        );
    }
    for b in &x.buffers {
        let _ = writeln!(
            s,
            "buffer {} {} {} {} 0x{:x}",
            b.id,
            b.width,
            b.height,
            fourcc_str(b.fourcc),
            b.modifier
        );
    }
    for f in &x.frames {
        let _ = writeln!(s, "frame {} output={}", f.name, f.output);
        for l in &f.layers {
            let _ = writeln!(
                s,
                "  layer buffer={} src={},{},{},{} dst={},{},{},{} z={} alpha={} blend={} transform={}",
                l.buffer,
                l.src.x,
                l.src.y,
                l.src.w,
                l.src.h,
                l.dst.x,
                l.dst.y,
                l.dst.w,
                l.dst.h,
                l.z,
                l.alpha,
                blend_name(l.blend),
                TRANSFORM_NAMES
                    .iter()
                    .find(|(_, b)| *b == l.transform)
                    .map_or("normal", |(n, _)| *n),
            );
        }
        s.push_str("end\n");
    }
    // Verify that the text means `x`: anything else is a model the format
    // cannot carry.
    match read(&s) {
        Ok(back) if back == *x => Ok(s),
        Ok(_) => syntax(0, "model does not survive a write/read round trip"),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests;
