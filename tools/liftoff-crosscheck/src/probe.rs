//! Converter from the `probe` text output to `xcheck` (plan item xc-convert).
//!
//! The input is the text of the fixtures `fixtures/*.probe.txt`: a header of
//! `# key: value` lines (`source`, `commit`, `observed` or `date`, `origin`,
//! `note`), then the probe's own lines (`device`, `output`, `plane`,
//! `formats`). Lines starting with `# ` are never parsed as probe text.
//!
//! The output is labelled with its input's origin: the xcheck `origin` line
//! carries the header's kind, date, source and commit. Recorded or synthetic
//! input gives no hardware claim about any other board or kernel.
//!
//! What the probe text does not say is not invented:
//!
//! * `transforms` is not in the probe; every plane gets `normal` only.
//! * `zpos_now` is not in the probe; a programmable range gets the plane's
//!   index in the dump, clamped into the range (the fixture documents give
//!   that index for RK3588 and STM32MP257).
//! * A plane without a `formats` line, or whose line was folded into a note
//!   (the BCM dumps), has **no** formats. [`convert`] refuses such a dump with
//!   [`ErrorKind::FormatsMissing`]; [`convert_with`] with
//!   `allow_missing_formats` produces the topology with empty format lists and
//!   flags it: [`Converted::formats_incomplete`] and the words
//!   `formats incomplete` in the origin text.
//! * The probe prints at most [`MAX_FORMATS`] formats per plane. A list of
//!   exactly that length may be cut, so it is marked: the plane id is in
//!   [`Converted::truncated_planes`] and the origin text says
//!   `format lists truncated`. A line `(identical to plane N)` is expanded to
//!   a copy of plane N's list (and inherits its truncation mark).

use composition_core::limits::MAX_FORMATS;

use crate::state::rotation;
use crate::state::BlendMode;
use crate::xcheck::{
    BlendSet, FormatDef, Origin, OriginKind, Output, Plane, PlaneKind, Profile, XCheck, ZposDef,
};
use std::fmt;

/// Provenance read from the `# ` header lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    /// `# source:` (file the probe block was copied from).
    pub source: String,
    /// `# commit:`, if given.
    pub commit: Option<String>,
    /// `# observed:` or `# date:`, `YYYY-MM-DD`.
    pub date: String,
    /// `# origin:` kind.
    pub kind: OriginKind,
}

/// A converted topology with its flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Converted {
    /// The topology (no frames, buffers or groups).
    pub xcheck: XCheck,
    /// Header provenance.
    pub provenance: Provenance,
    /// Some plane has no format list (only with `allow_missing_formats`).
    pub formats_incomplete: bool,
    /// Planes whose format list has no real content, in dump order.
    pub planes_without_formats: Vec<u32>,
    /// Planes whose format list has the probe's cut length and may be cut.
    pub truncated_planes: Vec<u32>,
}

/// Why a probe text was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// A mandatory header line is missing.
    MissingProvenance(&'static str),
    /// No `output` line.
    NoOutput,
    /// Planes lack real format lists; the converter does not invent any.
    FormatsMissing {
        /// Plane ids without formats.
        planes: Vec<u32>,
    },
    /// Anything else malformed or unsupported.
    Syntax(String),
}

/// A refusal with its 1-based line number (0 when none applies).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// Line number.
    pub line: usize,
    /// Reason.
    pub kind: ErrorKind,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "probe line {}: ", self.line)?;
        match &self.kind {
            ErrorKind::MissingProvenance(k) => write!(f, "header line `# {k}:` is missing"),
            ErrorKind::NoOutput => write!(f, "no output line"),
            ErrorKind::FormatsMissing { planes } => write!(
                f,
                "planes {planes:?} have no per-plane format list; not inventing one"
            ),
            ErrorKind::Syntax(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {}

fn bad<T>(line: usize, msg: impl Into<String>) -> Result<T, Error> {
    Err(Error {
        line,
        kind: ErrorKind::Syntax(msg.into()),
    })
}

/// Convert, refusing a dump that lacks real format lists.
pub fn convert(text: &str) -> Result<Converted, Error> {
    convert_with(text, false)
}

/// Convert; with `allow_missing_formats`, planes without formats get an empty
/// list and the result is flagged `formats incomplete` instead of refused.
pub fn convert_with(text: &str, allow_missing_formats: bool) -> Result<Converted, Error> {
    let mut header: Vec<(&str, &str)> = Vec::new();
    let mut outputs: Vec<Output> = Vec::new();
    let mut planes: Vec<Plane> = Vec::new();
    let mut truncated: Vec<u32> = Vec::new();
    let mut has_formats: Vec<bool> = Vec::new();
    let mut folded = false;

    for (idx, raw) in text.lines().enumerate() {
        let line = idx + 1;
        if let Some(h) = raw.strip_prefix("# ") {
            if let Some((k, v)) = h.split_once(':') {
                header.push((k.trim(), v.trim()));
            }
            continue;
        }
        let t = raw.trim();
        if t.is_empty() || t.starts_with("device ") {
            continue;
        }
        if t.starts_with("output ") {
            outputs.push(parse_output(line, t)?);
        } else if t.starts_with("plane ") {
            let mut p = parse_plane(line, t, planes.len())?;
            if planes.iter().any(|e| e.id == p.id) {
                return bad(line, format!("duplicate plane {}", p.id));
            }
            if (p.outputs as u64) >> outputs.len() != 0 {
                return bad(line, format!("plane {} names an undefined output", p.id));
            }
            p.formats.clear();
            planes.push(p);
            has_formats.push(false);
        } else if t.starts_with("formats ") {
            if folded {
                // A formats line after a "(after each plane line:)" note
                // describes every plane at once; it is not attached.
                continue;
            }
            let Some(last) = planes.len().checked_sub(1) else {
                return bad(line, "formats line before any plane");
            };
            if has_formats[last] {
                return bad(
                    line,
                    format!("second formats line for plane {}", planes[last].id),
                );
            }
            let (list, cut) = parse_formats(line, t, &planes)?;
            if cut {
                truncated.push(planes[last].id);
            }
            planes[last].formats = list;
            has_formats[last] = true;
        } else if t.starts_with('(') {
            folded = true;
        } else {
            return bad(line, format!("unknown line `{t}`"));
        }
    }

    let get = |k: &'static str| -> Result<&str, Error> {
        header
            .iter()
            .find(|(hk, _)| *hk == k)
            .map(|(_, v)| *v)
            .ok_or(Error {
                line: 0,
                kind: ErrorKind::MissingProvenance(k),
            })
    };
    let source = get("source")?.to_string();
    let commit = header
        .iter()
        .find(|(k, _)| *k == "commit")
        .map(|(_, v)| (*v).to_string());
    let date = match get("observed") {
        Ok(d) => d.to_string(),
        Err(_) => get("date")?.to_string(),
    };
    let kind = match get("origin")? {
        "observed" => OriginKind::Observed,
        "synthetic" => OriginKind::Synthetic,
        "source" => OriginKind::Source,
        o => return bad(0, format!("bad header origin `{o}`")),
    };
    if outputs.is_empty() {
        return Err(Error {
            line: 0,
            kind: ErrorKind::NoOutput,
        });
    }
    let without: Vec<u32> = planes
        .iter()
        .zip(&has_formats)
        .filter(|(_, h)| !**h)
        .map(|(p, _)| p.id)
        .collect();
    if !without.is_empty() && !allow_missing_formats {
        return Err(Error {
            line: 0,
            kind: ErrorKind::FormatsMissing { planes: without },
        });
    }

    let mut text = format!("probe text of {source}");
    if let Some(c) = &commit {
        text.push_str(&format!(" at commit {c}"));
    }
    text.push_str(match kind {
        OriginKind::Observed => ", recorded, no claim about any other board or kernel",
        OriginKind::Synthetic => ", synthetic, no hardware claim",
        OriginKind::Source => ", from source, no hardware claim",
    });
    if !without.is_empty() {
        text.push_str("; formats incomplete: no per-plane format lists, none invented");
    }
    if !truncated.is_empty() {
        text.push_str(&format!(
            "; format lists truncated at {MAX_FORMATS} by the probe"
        ));
    }
    let text = text.replace('#', "");

    Ok(Converted {
        xcheck: XCheck {
            origin: Origin {
                kind,
                date: date.clone(),
                text,
            },
            profile: Profile::Generic,
            outputs,
            planes,
            groups: Vec::new(),
            buffers: Vec::new(),
            frames: Vec::new(),
        },
        provenance: Provenance {
            source,
            commit,
            date,
            kind,
        },
        formats_incomplete: !without.is_empty(),
        planes_without_formats: without,
        truncated_planes: truncated,
    })
}

/// `output 0: crtc 81 1920x1080 @ 60.000 Hz`
fn parse_output(line: usize, t: &str) -> Result<Output, Error> {
    let toks: Vec<&str> = t.split_whitespace().collect();
    let ["output", _, "crtc", crtc, mode, "@", hz, "Hz"] = toks.as_slice() else {
        return bad(line, "output line not `output N: crtc ID WxH @ R Hz`");
    };
    let n = |s: &str| {
        s.parse::<u32>()
            .or_else(|_| bad(line, format!("bad number `{s}`")))
    };
    let (w, h) = mode
        .split_once('x')
        .ok_or(())
        .or_else(|_| bad(line, format!("bad mode `{mode}`")))?;
    let (whole, frac) = hz.split_once('.').unwrap_or((hz, ""));
    if frac.len() > 3 || !frac.bytes().all(|c| c.is_ascii_digit()) {
        return bad(line, format!("bad refresh `{hz}`"));
    }
    let frac = format!("{frac:0<3}");
    Ok(Output {
        id: n(crtc)?,
        width: n(w)?,
        height: n(h)?,
        refresh_mhz: n(whole)? * 1000 + n(&frac)?,
    })
}

/// `plane 33 Primary: outputs 0b1, zpos 0..=7, plane alpha true, blend [..], scaling not claimed`
fn parse_plane(line: usize, t: &str, index: usize) -> Result<Plane, Error> {
    let rest = t.strip_prefix("plane ").unwrap_or(t);
    let Some((id, rest)) = rest.split_once(' ') else {
        return bad(line, "plane line too short");
    };
    let id: u32 = id
        .parse()
        .or_else(|_| bad(line, format!("bad plane id `{id}`")))?;
    let Some((kind, rest)) = rest.split_once(':') else {
        return bad(line, "plane line without `Kind:`");
    };
    let kind = match kind {
        "Primary" => PlaneKind::Primary,
        "Overlay" => PlaneKind::Overlay,
        "Cursor" => PlaneKind::Cursor,
        k => return bad(line, format!("bad plane kind `{k}`")),
    };
    let Some((head, tail)) = rest.split_once(", blend [") else {
        return bad(line, "plane line without `blend [..]`");
    };
    let Some((blend_s, tail)) = tail.split_once("], scaling ") else {
        return bad(line, "plane line without `scaling`");
    };
    if tail.trim() != "not claimed" {
        return bad(line, format!("unsupported scaling text `{tail}`"));
    }
    let fields: Vec<&str> = head.split(", ").map(str::trim).collect();
    let [outputs_s, zpos_s, alpha_s] = fields.as_slice() else {
        return bad(line, "plane line fields not `outputs, zpos, plane alpha`");
    };
    let outputs = outputs_s
        .strip_prefix("outputs 0b")
        .and_then(|b| u32::from_str_radix(b, 2).ok());
    let Some(outputs) = outputs else {
        return bad(line, format!("bad outputs `{outputs_s}`"));
    };
    let alpha = match *alpha_s {
        "plane alpha true" => true,
        "plane alpha false" => false,
        a => return bad(line, format!("bad `{a}`")),
    };
    let zs = zpos_s.strip_prefix("zpos ").unwrap_or("");
    let num = |s: &str| {
        s.parse::<i32>()
            .or_else(|_| bad(line, format!("bad zpos `{zs}`")))
    };
    let (zpos, zpos_now) = if let Some(n) = zs.strip_prefix("fixed ") {
        (ZposDef::Fixed(num(n)?), None)
    } else if let Some((a, b)) = zs.split_once("..=") {
        let (min, max) = (num(a)?, num(b)?);
        if min > max {
            return bad(line, format!("empty zpos range `{zs}`"));
        }
        let now = i32::try_from(index).unwrap_or(i32::MAX).clamp(min, max);
        (ZposDef::Range { min, max }, Some(now))
    } else {
        return bad(line, format!("bad zpos `{zs}`"));
    };
    let mut blend = 0u8;
    for name in blend_s.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        blend |= BlendSet::bit(match name {
            "none" => BlendMode::Opaque,
            "premultiplied" => BlendMode::Premultiplied,
            "coverage" => BlendMode::Coverage,
            o => return bad(line, format!("bad blend mode `{o}`")),
        });
    }
    Ok(Plane {
        id,
        kind,
        outputs,
        zpos,
        zpos_now,
        alpha,
        blend: BlendSet(blend),
        transforms: rotation::ROTATE_0,
        scaling: None,
        max_source: None,
        full_output: false,
        formats: Vec::new(),
    })
}

/// `formats (N): A:0xM B ...` or `formats (N): (identical to plane P)`;
/// returns the list and whether it has the probe's cut length.
fn parse_formats(line: usize, t: &str, earlier: &[Plane]) -> Result<(Vec<FormatDef>, bool), Error> {
    let Some(rest) = t.strip_prefix("formats (") else {
        return bad(line, "bad formats line");
    };
    let Some((n, body)) = rest.split_once("):") else {
        return bad(line, "formats line without `(N):`");
    };
    let n: usize = n
        .parse()
        .or_else(|_| bad(line, format!("bad count `{n}`")))?;
    let body = body.trim();
    if let Some(p) = body
        .strip_prefix("(identical to plane ")
        .and_then(|b| b.strip_suffix(')'))
    {
        let p: u32 = p
            .parse()
            .or_else(|_| bad(line, format!("bad plane `{p}`")))?;
        let Some(src) = earlier.iter().rev().skip(1).find(|e| e.id == p) else {
            return bad(line, format!("identical to unknown plane {p}"));
        };
        if src.formats.len() != n {
            return bad(
                line,
                format!("count {n} differs from plane {p} ({})", src.formats.len()),
            );
        }
        return Ok((src.formats.clone(), n >= MAX_FORMATS));
    }
    let mut list = Vec::new();
    for tok in body.split_whitespace() {
        let (cc, modifier) = match tok.split_once(':') {
            Some((c, m)) => {
                let v = m
                    .strip_prefix("0x")
                    .and_then(|h| u64::from_str_radix(h, 16).ok());
                let Some(v) = v else {
                    return bad(line, format!("bad modifier in `{tok}`"));
                };
                (c, v)
            }
            None => (tok, 0),
        };
        let b = cc.as_bytes();
        if b.len() != 4 || !b.iter().all(|c| c.is_ascii_graphic() && *c != b'#') {
            return bad(line, format!("bad fourcc `{cc}`"));
        }
        list.push(FormatDef {
            fourcc: [b[0], b[1], b[2], b[3]],
            modifier,
        });
    }
    if list.len() != n {
        return bad(
            line,
            format!("formats count {n} but {} entries listed", list.len()),
        );
    }
    Ok((list, n >= MAX_FORMATS))
}
