//! Pure parsing for `hvs_load_probe`: the vc4 debugfs files `hvs_underrun`
//! and `hvs_dlists`, and the probe's own `STEP` lines.
//!
//! No device access happens here. The example uses this module to read the
//! debugfs text and to format its output; the host test
//! `tests/hvs_load_parse.rs` includes the same file with `#[path]` and checks
//! it against the recorded BCM2711 debugfs dumps and synthetic lines.
//!
//! Formats (read from source, `vc4_hvs.c` of the Raspberry Pi kernel, and
//! seen in `tests/data/bcm2711/debugfs-2026-11-28-*.txt`):
//!
//! * `hvs_underrun`: one decimal number, the count of HVS underrun
//!   interrupts since boot.
//! * `hvs_dlists`: per channel either `HVS chan N disabled` or
//!   `HVS chan N:` followed by one `dlist: <index>: 0x<word>` line per word of
//!   the channel's active display list, up to and including the end word.
//!
//! Lines starting with `#` (the comment header of the recorded fixtures) are
//! skipped. Unknown lines are ignored, so a slightly different layout on
//! another kernel or generation does not fail the run.

#![allow(dead_code)] // the example and the test each use part of the module

use std::fmt;

/// The end-of-list bit of a dlist control word (`SCALER_CTL0_END`, bit 31).
pub const DLIST_END: u32 = 1 << 31;

/// Why a debugfs or `STEP` text could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn content_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

/// Parses `hvs_underrun`: the first non-comment line, a decimal count.
pub fn parse_underrun(text: &str) -> Result<u64, ParseError> {
    let line = content_lines(text)
        .next()
        .ok_or_else(|| ParseError("hvs_underrun is empty".into()))?;
    line.parse()
        .map_err(|_| ParseError(format!("hvs_underrun: not a count: {line:?}")))
}

/// One HVS channel of `hvs_dlists`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    /// Channel number as printed.
    pub index: u32,
    /// `false` for `HVS chan N disabled`.
    pub enabled: bool,
    /// `(dlist index, word)` in the order printed.
    pub entries: Vec<(u32, u32)>,
}

impl Channel {
    /// Number of dlist words printed for the channel (end word included).
    pub fn words(&self) -> usize {
        self.entries.len()
    }

    /// Whether the last printed word has the end bit set.
    pub fn ends_with_end_word(&self) -> bool {
        self.entries.last().is_some_and(|(_, w)| w & DLIST_END != 0)
    }
}

/// The parsed `hvs_dlists` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dlists {
    /// Channels in the order printed.
    pub channels: Vec<Channel>,
}

impl Dlists {
    /// Sum of the words of all enabled channels.
    pub fn total_words(&self) -> usize {
        self.channels.iter().map(Channel::words).sum()
    }

    /// `0:9,1:off,2:off`: words per channel, `off` for a disabled one.
    pub fn per_channel_text(&self) -> String {
        if self.channels.is_empty() {
            return "none".into();
        }
        self.channels
            .iter()
            .map(|c| {
                if c.enabled {
                    format!("{}:{}", c.index, c.words())
                } else {
                    format!("{}:off", c.index)
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn parse_hex(text: &str) -> Option<u32> {
    let digits = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))?;
    u32::from_str_radix(digits, 16).ok()
}

/// Parses `hvs_dlists`. Fails only if there is no `HVS chan` line at all
/// or a `dlist:` line is malformed or comes before any channel.
pub fn parse_dlists(text: &str) -> Result<Dlists, ParseError> {
    let mut channels: Vec<Channel> = Vec::new();
    for line in content_lines(text) {
        if let Some(rest) = line.strip_prefix("HVS chan ") {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let index = digits
                .parse()
                .map_err(|_| ParseError(format!("hvs_dlists: bad channel line {line:?}")))?;
            let tail = rest[digits.len()..].trim();
            channels.push(Channel {
                index,
                enabled: !tail.starts_with("disabled"),
                entries: Vec::new(),
            });
        } else if let Some(rest) = line.strip_prefix("dlist:") {
            let channel = channels.last_mut().ok_or_else(|| {
                ParseError(format!("hvs_dlists: dlist line before a channel: {line:?}"))
            })?;
            let (index, word) = rest
                .split_once(':')
                .ok_or_else(|| ParseError(format!("hvs_dlists: bad dlist line {line:?}")))?;
            let index = index
                .trim()
                .parse()
                .map_err(|_| ParseError(format!("hvs_dlists: bad dlist index {line:?}")))?;
            let word = parse_hex(word.trim())
                .ok_or_else(|| ParseError(format!("hvs_dlists: bad dlist word {line:?}")))?;
            channel.entries.push((index, word));
        }
    }
    if channels.is_empty() {
        return Err(ParseError("hvs_dlists: no HVS chan line".into()));
    }
    Ok(Dlists { channels })
}

/// The TEST_ONLY outcome of one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestOnly {
    /// Accepted.
    Ok,
    /// Rejected with this errno.
    Err(i32),
    /// Not tried; the token says why (`alloc`, `fb`, ...).
    Skipped(String),
}

impl fmt::Display for TestOnly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestOnly::Ok => f.write_str("ok"),
            TestOnly::Err(errno) => write!(f, "err:{errno}"),
            TestOnly::Skipped(why) => write!(f, "skipped:{why}"),
        }
    }
}

/// The outcome of the step's first real (blocking) commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Commit {
    /// Committed.
    Ok,
    /// Refused with this errno.
    Err(i32),
    /// Not run (TEST_ONLY failed or was skipped, or `--frames 0`).
    NotRun,
}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Commit::Ok => f.write_str("ok"),
            Commit::Err(errno) => write!(f, "err:{errno}"),
            Commit::NotRun => f.write_str("not-run"),
        }
    }
}

/// One `STEP` line of the probe.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// Step number, from 1.
    pub n: u32,
    /// Layers (planes) in the step.
    pub layers: u32,
    /// Destination size of each layer, output pixels.
    pub size: (u32, u32),
    /// Source size of each layer, buffer pixels.
    pub src: (u32, u32),
    /// Scale factor, destination over source (2 = upscale 2x).
    pub scale: f64,
    /// TEST_ONLY outcome.
    pub test_only: TestOnly,
    /// First real commit.
    pub commit: Commit,
    /// Page flips that arrived, of the frames asked for.
    pub latched: (u32, u32),
    /// Flips more than 1.5 refresh periods after the previous one.
    pub late: u32,
    /// Frames whose flip never arrived or whose commit was refused.
    pub missing: u32,
    /// `hvs_underrun` before the step; `None` if debugfs is unavailable.
    pub underrun_before: Option<u64>,
    /// `hvs_underrun` after the step.
    pub underrun_after: Option<u64>,
    /// Total dlist words of the enabled channels while the step was shown.
    pub dlist_words: Option<usize>,
    /// Per-channel words (`0:9,1:off`), or `na`.
    pub dlist: String,
}

fn opt<T: fmt::Display>(value: &Option<T>) -> String {
    value.as_ref().map_or("na".into(), ToString::to_string)
}

impl Step {
    /// Whether the step shows a drop: TEST_ONLY passed and the underrun
    /// count moved, a flip was late or missing, or the real commit failed.
    pub fn dropped(&self) -> bool {
        if self.test_only != TestOnly::Ok {
            return false;
        }
        let underrun = matches!(
            (self.underrun_before, self.underrun_after),
            (Some(a), Some(b)) if b > a
        );
        underrun || self.late > 0 || self.missing > 0 || matches!(self.commit, Commit::Err(_))
    }

    /// The `STEP ...` line, as the probe prints it.
    pub fn line(&self) -> String {
        format!(
            "STEP n={} layers={} size={}x{} src={}x{} scale={:.4} test_only={} commit={} \
             latched={}/{} late={} missing={} underrun_before={} underrun_after={} \
             dlist_words={} dlist={}",
            self.n,
            self.layers,
            self.size.0,
            self.size.1,
            self.src.0,
            self.src.1,
            self.scale,
            self.test_only,
            self.commit,
            self.latched.0,
            self.latched.1,
            self.late,
            self.missing,
            opt(&self.underrun_before),
            opt(&self.underrun_after),
            opt(&self.dlist_words),
            self.dlist,
        )
    }
}

fn field<'a>(fields: &[(&'a str, &'a str)], key: &str) -> Result<&'a str, ParseError> {
    fields
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .ok_or_else(|| ParseError(format!("STEP: no {key}=")))
}

fn num<T: std::str::FromStr>(text: &str, key: &str) -> Result<T, ParseError> {
    text.parse()
        .map_err(|_| ParseError(format!("STEP: bad {key}={text}")))
}

fn pair(text: &str, sep: char, key: &str) -> Result<(u32, u32), ParseError> {
    let (a, b) = text
        .split_once(sep)
        .ok_or_else(|| ParseError(format!("STEP: bad {key}={text}")))?;
    Ok((num(a, key)?, num(b, key)?))
}

fn optional<T: std::str::FromStr>(text: &str, key: &str) -> Result<Option<T>, ParseError> {
    if text == "na" {
        Ok(None)
    } else {
        num(text, key).map(Some)
    }
}

/// Parses one `STEP ...` line (as written by [`Step::line`]).
pub fn parse_step(line: &str) -> Result<Step, ParseError> {
    let rest = line
        .trim()
        .strip_prefix("STEP ")
        .ok_or_else(|| ParseError(format!("not a STEP line: {line:?}")))?;
    let fields: Vec<(&str, &str)> = rest
        .split_whitespace()
        .map(|token| {
            token
                .split_once('=')
                .ok_or_else(|| ParseError(format!("STEP: token without '=': {token:?}")))
        })
        .collect::<Result<_, _>>()?;
    let test_only = match field(&fields, "test_only")? {
        "ok" => TestOnly::Ok,
        other => {
            if let Some(errno) = other.strip_prefix("err:") {
                TestOnly::Err(num(errno, "test_only")?)
            } else if let Some(why) = other.strip_prefix("skipped:") {
                TestOnly::Skipped(why.to_string())
            } else {
                return Err(ParseError(format!("STEP: bad test_only={other}")));
            }
        }
    };
    let commit = match field(&fields, "commit")? {
        "ok" => Commit::Ok,
        "not-run" => Commit::NotRun,
        other => match other.strip_prefix("err:") {
            Some(errno) => Commit::Err(num(errno, "commit")?),
            None => return Err(ParseError(format!("STEP: bad commit={other}"))),
        },
    };
    Ok(Step {
        n: num(field(&fields, "n")?, "n")?,
        layers: num(field(&fields, "layers")?, "layers")?,
        size: pair(field(&fields, "size")?, 'x', "size")?,
        src: pair(field(&fields, "src")?, 'x', "src")?,
        scale: num(field(&fields, "scale")?, "scale")?,
        test_only,
        commit,
        latched: pair(field(&fields, "latched")?, '/', "latched")?,
        late: num(field(&fields, "late")?, "late")?,
        missing: num(field(&fields, "missing")?, "missing")?,
        underrun_before: optional(field(&fields, "underrun_before")?, "underrun_before")?,
        underrun_after: optional(field(&fields, "underrun_after")?, "underrun_after")?,
        dlist_words: optional(field(&fields, "dlist_words")?, "dlist_words")?,
        dlist: field(&fields, "dlist")?.to_string(),
    })
}

/// The `SUMMARY ...` line for a list of steps.
pub fn summary_line(steps: &[Step]) -> String {
    let ok = steps.iter().filter(|s| s.test_only == TestOnly::Ok).count();
    let err = steps
        .iter()
        .filter(|s| matches!(s.test_only, TestOnly::Err(_)))
        .count();
    let skipped = steps.len() - ok - err;
    let drops = steps.iter().filter(|s| s.dropped()).count();
    let first = steps
        .iter()
        .find(|s| s.dropped())
        .map_or("none".into(), |s| s.n.to_string());
    let first_reject = steps
        .iter()
        .find(|s| matches!(s.test_only, TestOnly::Err(_)))
        .map_or("none".into(), |s| s.n.to_string());
    let underrun = match (
        steps.iter().find_map(|s| s.underrun_before),
        steps.iter().rev().find_map(|s| s.underrun_after),
    ) {
        (Some(a), Some(b)) => b.saturating_sub(a).to_string(),
        _ => "na".into(),
    };
    format!(
        "SUMMARY steps={} test_only_ok={ok} test_only_err={err} skipped={skipped} \
         drop_steps={drops} first_drop={first} first_reject={first_reject} underrun_delta={underrun}",
        steps.len()
    )
}
