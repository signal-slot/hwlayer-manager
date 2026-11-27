//! The strict parser: one line to one typed [`Record`].

use core::fmt;

use composition_core::{
    Assignment, BackendError, BufferId, DropReason, Error, OutputId, PlanError, PlaneId,
    PresentError, Rect, RejectReasons, SubmissionId, TraceEvent,
};

use crate::{is_quotable, is_word, Assignments, Clock, Header, Source, Stamp, PREFIX, VERSION};

/// What is wrong with a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The line breaks the grammar: two spaces in a row, a field without
    /// `=`, a malformed key, an empty value, an unterminated quote, or a
    /// line break inside the line.
    Syntax,
    /// Something follows the last field: a trailing space, or characters
    /// right after a closing quote.
    TrailingGarbage,
    /// `v=` names a version this parser does not know.
    UnknownVersion(u32),
    /// `ev=` names no event of the format.
    UnknownEvent,
    /// A key the format does not define.
    UnknownKey,
    /// A key the format defines, but not on this kind of line, or not with
    /// this `result=`.
    UnexpectedKey(&'static str),
    /// A key given twice.
    DuplicateKey(&'static str),
    /// A field this line must carry is absent. `v` and `ev` are also
    /// reported missing when they are not the first two fields.
    MissingField(&'static str),
    /// A number field is not a number of its type: sign, leading zero,
    /// overflow, upper-case hex or a stray character.
    BadNumber(&'static str),
    /// A value is not one the field takes: an unknown `result=` code,
    /// `reason=`, `src=` or `clock=`, a malformed `assign=` item, a quoted
    /// value where a word is due or the reverse, characters outside the
    /// field's character set, a zero `freq_hz`, or `reasons=` bits the core
    /// does not define.
    BadValue(&'static str),
    /// `n=` differs from the number of `assign=` items.
    CountMismatch,
    /// `assign=` has more items than the core's
    /// [`MAX_LAYERS`](composition_core::limits::MAX_LAYERS).
    TooManyAssignments,
}

/// Why [`parse_line`] refused a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParseError {
    /// What is wrong.
    pub kind: ErrorKind,
    /// Byte offset in the line where it was found (the line's length for a
    /// missing field).
    pub offset: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ErrorKind::Syntax => f.write_str("syntax error")?,
            ErrorKind::TrailingGarbage => f.write_str("trailing garbage")?,
            ErrorKind::UnknownVersion(v) => write!(f, "unknown version {v}")?,
            ErrorKind::UnknownEvent => f.write_str("unknown event")?,
            ErrorKind::UnknownKey => f.write_str("unknown key")?,
            ErrorKind::UnexpectedKey(k) => write!(f, "key `{k}` not allowed here")?,
            ErrorKind::DuplicateKey(k) => write!(f, "duplicate key `{k}`")?,
            ErrorKind::MissingField(k) => write!(f, "missing field `{k}`")?,
            ErrorKind::BadNumber(k) => write!(f, "bad number in `{k}`")?,
            ErrorKind::BadValue(k) => write!(f, "bad value of `{k}`")?,
            ErrorKind::CountMismatch => f.write_str("`n` differs from the `assign` count")?,
            ErrorKind::TooManyAssignments => f.write_str("too many assignments")?,
        }
        write!(f, " at byte {}", self.offset)
    }
}

/// One parsed line.
///
/// An event holds its assignments inline (no allocation), so the variants
/// differ in size.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record<'a> {
    /// The `hdr` line. Its quoted values borrow from the line.
    Header {
        /// Its line counter.
        seq: u64,
        /// Its fields.
        header: Header<'a>,
    },
    /// The line of one trace entry.
    Event {
        /// The common fields.
        stamp: Stamp,
        /// The entry.
        event: Event,
    },
}

/// An owned [`TraceEvent`], as parsed from a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// [`TraceEvent::Check`].
    Check {
        /// Output of the frame.
        output: OutputId,
        /// The candidate.
        assignments: Assignments,
        /// The backend's answer.
        result: Result<(), BackendError>,
    },
    /// [`TraceEvent::Validate`].
    Validate {
        /// Output of the frame.
        output: OutputId,
        /// Number of layers in the frame.
        layers: usize,
        /// The assignment, or why there is none.
        result: Result<Assignments, Error>,
    },
    /// [`TraceEvent::Commit`].
    Commit {
        /// Output of the frame.
        output: OutputId,
        /// The accepted submission, or why it was refused.
        result: Result<SubmissionId, Error>,
    },
    /// [`TraceEvent::Latched`].
    Latched {
        /// Output it was latched on.
        output: OutputId,
        /// The latched submission.
        submission: SubmissionId,
    },
    /// [`TraceEvent::Released`].
    Released {
        /// The released buffer.
        buffer: BufferId,
        /// The submission whose end released it, `None` on device loss.
        submission: Option<SubmissionId>,
    },
    /// [`TraceEvent::Dropped`].
    Dropped {
        /// Output it was submitted to.
        output: OutputId,
        /// The dropped submission.
        submission: SubmissionId,
        /// Why.
        reason: DropReason,
    },
    /// [`TraceEvent::LatchTimeout`].
    LatchTimeout {
        /// Output the submission is pending on.
        output: OutputId,
        /// The overdue submission.
        submission: SubmissionId,
    },
    /// [`TraceEvent::DeviceLost`].
    DeviceLost,
    /// [`TraceEvent::CapabilitiesChanged`].
    CapabilitiesChanged,
}

impl Event {
    /// The core's view of the event, borrowing the assignments.
    pub fn as_trace_event(&self) -> TraceEvent<'_> {
        match self {
            Event::Check {
                output,
                assignments,
                result,
            } => TraceEvent::Check {
                output: *output,
                assignments: assignments.as_slice(),
                result: *result,
            },
            Event::Validate {
                output,
                layers,
                result,
            } => TraceEvent::Validate {
                output: *output,
                layers: *layers,
                result: match result {
                    Ok(a) => Ok(a.as_slice()),
                    Err(e) => Err(*e),
                },
            },
            Event::Commit { output, result } => TraceEvent::Commit {
                output: *output,
                result: *result,
            },
            Event::Latched { output, submission } => TraceEvent::Latched {
                output: *output,
                submission: *submission,
            },
            Event::Released { buffer, submission } => TraceEvent::Released {
                buffer: *buffer,
                submission: *submission,
            },
            Event::Dropped {
                output,
                submission,
                reason,
            } => TraceEvent::Dropped {
                output: *output,
                submission: *submission,
                reason: *reason,
            },
            Event::LatchTimeout { output, submission } => TraceEvent::LatchTimeout {
                output: *output,
                submission: *submission,
            },
            Event::DeviceLost => TraceEvent::DeviceLost,
            Event::CapabilitiesChanged => TraceEvent::CapabilitiesChanged,
        }
    }
}

/// Every key of format version 1.
const KEYS: [&str; 29] = [
    "v",
    "ev",
    "seq",
    "t",
    "at",
    "tev",
    "dur",
    "out",
    "sub",
    "buf",
    "n",
    "assign",
    "layers",
    "result",
    "reason",
    "layer",
    "reasons",
    "count",
    "candidates",
    "errno",
    "err_out",
    "err_sub",
    "src",
    "clock",
    "freq_hz",
    "t0",
    "at_unit",
    "build",
    "board",
];

const EVENTS: [&str; 10] = [
    "hdr",
    "check",
    "validate",
    "commit",
    "latched",
    "released",
    "dropped",
    "latch_timeout",
    "device_lost",
    "caps_changed",
];

/// Every reason bit the core defines.
const REASONS: [RejectReasons; 11] = [
    RejectReasons::OUTPUT,
    RejectReasons::FORMAT,
    RejectReasons::SCALING,
    RejectReasons::SOURCE_SIZE,
    RejectReasons::ALPHA,
    RejectReasons::BLEND,
    RejectReasons::TRANSFORM,
    RejectReasons::FULL_OUTPUT,
    RejectReasons::Z_ORDER,
    RejectReasons::RESOURCE,
    RejectReasons::IN_USE,
];

fn err<T>(kind: ErrorKind, offset: usize) -> Result<T, ParseError> {
    Err(ParseError { kind, offset })
}

/// Parses one line.
///
/// The line may end in one `\n` or `\r\n`, or in nothing. A line that does
/// not start with `CETRACE` followed by a space or the end of the line is
/// not a trace line and gives `Ok(None)`, so a parser can run over a whole
/// console log. Every trace line must follow `docs/trace-format.md` version
/// 1 exactly, except for the order of the fields after `ev=`; anything else
/// is a [`ParseError`].
pub fn parse_line(line: &str) -> Result<Option<Record<'_>>, ParseError> {
    let body = line
        .strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .unwrap_or(line);
    let Some(rest) = body.strip_prefix(PREFIX) else {
        return Ok(None);
    };
    if let Some(at) = body.bytes().position(|b| b == b'\n' || b == b'\r') {
        return err(ErrorKind::Syntax, at);
    }
    if rest.is_empty() {
        return err(ErrorKind::MissingField("v"), body.len());
    }
    if !rest.starts_with(' ') {
        return Ok(None);
    }

    let mut lexer = Lexer {
        s: body,
        pos: PREFIX.len(),
    };

    // The version comes first and is checked before anything else, so a
    // line of a later version is refused for its version alone.
    let Some(v) = lexer.next()? else {
        return err(ErrorKind::MissingField("v"), body.len());
    };
    if KEYS[v.key] != "v" || v.quoted {
        return err(ErrorKind::MissingField("v"), v.key_offset);
    }
    let version = number::<u32>(v.value).ok_or(ParseError {
        kind: ErrorKind::BadNumber("v"),
        offset: v.offset,
    })?;
    if version != VERSION {
        return err(ErrorKind::UnknownVersion(version), v.offset);
    }

    let Some(ev) = lexer.next()? else {
        return err(ErrorKind::MissingField("ev"), body.len());
    };
    if KEYS[ev.key] != "ev" || ev.quoted {
        return err(ErrorKind::MissingField("ev"), ev.key_offset);
    }
    let Some(kind) = EVENTS.iter().position(|&e| e == ev.value) else {
        return err(ErrorKind::UnknownEvent, ev.offset);
    };

    let mut fields = Fields {
        slots: [None; KEYS.len()],
        end: body.len(),
    };
    fields.insert(v)?;
    fields.slots[v.key].as_mut().unwrap().used = true;
    fields.insert(ev)?;
    fields.slots[ev.key].as_mut().unwrap().used = true;
    while let Some(pair) = lexer.next()? {
        fields.insert(pair)?;
    }

    let record = if EVENTS[kind] == "hdr" {
        parse_header(&mut fields)?
    } else {
        parse_event(EVENTS[kind], &mut fields)?
    };
    fields.finish()?;
    Ok(Some(record))
}

fn parse_header<'a>(f: &mut Fields<'a>) -> Result<Record<'a>, ParseError> {
    let seq = f.number("seq")?;
    let (src, at) = f.word("src")?;
    let src = match src {
        "linux" => Source::Linux,
        "zephyr" => Source::Zephyr,
        _ => return err(ErrorKind::BadValue("src"), at),
    };
    let (clock, at) = f.word("clock")?;
    let clock = match clock {
        "monotonic_ns" => Clock::MonotonicNs,
        "cyc32" => Clock::Cyc32,
        "cyc64" => Clock::Cyc64,
        "uptime_ticks" => Clock::UptimeTicks,
        _ => return err(ErrorKind::BadValue("clock"), at),
    };
    let freq_hz = f.number("freq_hz")?;
    if freq_hz == 0 {
        return err(ErrorKind::BadValue("freq_hz"), f.offset("freq_hz"));
    }
    let header = Header {
        src,
        clock,
        freq_hz,
        t0: f.number("t0")?,
        at_unit: f.word("at_unit")?.0,
        build: f.quoted("build")?,
        board: f.quoted("board")?,
    };
    Ok(Record::Header { seq, header })
}

fn parse_event<'a>(name: &str, f: &mut Fields<'a>) -> Result<Record<'a>, ParseError> {
    let mut stamp = Stamp {
        seq: f.number("seq")?,
        t: f.number("t")?,
        at: f.number("at")?,
        tev: None,
        dur: None,
    };
    let event = match name {
        "check" => {
            let output = OutputId(f.number("out")?);
            let assignments = f.assignments()?;
            let (code, at) = f.word("result")?;
            let result = if code == "ok" {
                Ok(())
            } else {
                Err(backend_error(code, at, f)?)
            };
            Event::Check {
                output,
                assignments,
                result,
            }
        }
        "validate" => {
            let output = OutputId(f.number("out")?);
            let layers = f.number("layers")?;
            let (code, at) = f.word("result")?;
            let result = if code == "ok" {
                Ok(f.assignments()?)
            } else {
                Err(engine_error(code, at, f)?)
            };
            stamp.dur = f.opt_number("dur")?;
            Event::Validate {
                output,
                layers,
                result,
            }
        }
        "commit" => {
            let output = OutputId(f.number("out")?);
            let (code, at) = f.word("result")?;
            let result = if code == "ok" {
                Ok(SubmissionId(f.number("sub")?))
            } else {
                Err(engine_error(code, at, f)?)
            };
            stamp.dur = f.opt_number("dur")?;
            Event::Commit { output, result }
        }
        "latched" => {
            let output = OutputId(f.number("out")?);
            let submission = SubmissionId(f.number("sub")?);
            stamp.tev = f.opt_number("tev")?;
            Event::Latched { output, submission }
        }
        "released" => {
            let buffer = BufferId(f.number("buf")?);
            let submission = if f.peek("sub") == Some("-") {
                f.word("sub")?;
                None
            } else {
                Some(SubmissionId(f.number("sub")?))
            };
            Event::Released { buffer, submission }
        }
        "dropped" => {
            let output = OutputId(f.number("out")?);
            let submission = SubmissionId(f.number("sub")?);
            let (reason, at) = f.word("reason")?;
            let reason = match reason {
                "replaced" => DropReason::Replaced,
                "device_lost" => DropReason::DeviceLost,
                _ => return err(ErrorKind::BadValue("reason"), at),
            };
            Event::Dropped {
                output,
                submission,
                reason,
            }
        }
        "latch_timeout" => Event::LatchTimeout {
            output: OutputId(f.number("out")?),
            submission: SubmissionId(f.number("sub")?),
        },
        "device_lost" => Event::DeviceLost,
        _ => Event::CapabilitiesChanged,
    };
    Ok(Record::Event { stamp, event })
}

/// A bare [`BackendError`] code (`check`, or after `backend.`) and its
/// extras.
fn backend_error(code: &str, at: usize, f: &mut Fields<'_>) -> Result<BackendError, ParseError> {
    Ok(match code {
        "rejected" => BackendError::Rejected,
        "busy" => BackendError::Busy,
        "unknown_buffer" => BackendError::UnknownBuffer(BufferId(f.number("buf")?)),
        "device_lost" => BackendError::DeviceLost,
        "platform" => BackendError::Platform(f.number("errno")?),
        _ => return err(ErrorKind::BadValue("result"), at),
    })
}

/// An engine [`Error`] code and its extras.
fn engine_error(code: &str, at: usize, f: &mut Fields<'_>) -> Result<Error, ParseError> {
    let bad = || ParseError {
        kind: ErrorKind::BadValue("result"),
        offset: at,
    };
    let (group, code) = match code.split_once('.') {
        Some(split) => split,
        None => match code {
            "stale_plan" => return Ok(Error::StalePlan),
            "plan_mismatch" => return Ok(Error::PlanMismatch),
            _ => return Err(bad()),
        },
    };
    Ok(match group {
        "backend" => Error::Backend(backend_error(code, at, f)?),
        "plan" => Error::Plan(match code {
            "unknown_output" => PlanError::UnknownOutput(OutputId(f.number("err_out")?)),
            "too_many_layers" => PlanError::TooManyLayers {
                count: f.number("count")?,
            },
            "unknown_buffer" => PlanError::UnknownBuffer {
                layer: f.number("layer")?,
                buffer: BufferId(f.number("buf")?),
            },
            "invalid_source" => PlanError::InvalidSource {
                layer: f.number("layer")?,
            },
            "invalid_destination" => PlanError::InvalidDestination {
                layer: f.number("layer")?,
            },
            "cannot_clip" => PlanError::CannotClip {
                layer: f.number("layer")?,
            },
            "no_plane" => PlanError::NoPlane {
                layer: f.number("layer")?,
                reasons: f.reasons()?,
            },
            "backend_rejected" => PlanError::BackendRejected {
                candidates: f.number("candidates")?,
            },
            "limit_exceeded" => PlanError::LimitExceeded,
            "aborted" => PlanError::Aborted,
            _ => return Err(bad()),
        }),
        "present" => Error::Present(match code {
            "unknown_buffer" => PresentError::UnknownBuffer(BufferId(f.number("buf")?)),
            "duplicate_buffer" => PresentError::DuplicateBuffer(BufferId(f.number("buf")?)),
            "buffer_table_full" => PresentError::BufferTableFull,
            "buffer_in_use" => PresentError::BufferInUse(BufferId(f.number("buf")?)),
            "buffer_not_ready" => PresentError::BufferNotReady(BufferId(f.number("buf")?)),
            "buffer_not_writable" => PresentError::BufferNotWritable(BufferId(f.number("buf")?)),
            "busy" => PresentError::Busy(OutputId(f.number("err_out")?)),
            "too_many_outputs" => PresentError::TooManyOutputs,
            "too_many_buffers" => PresentError::TooManyBuffers,
            "event_queue_full" => PresentError::EventQueueFull,
            "unexpected_latch" => PresentError::UnexpectedLatch {
                output: OutputId(f.number("err_out")?),
                submission: SubmissionId(f.number("err_sub")?),
            },
            _ => return Err(bad()),
        }),
        _ => return Err(bad()),
    })
}

/// One `key=value` field of a line.
#[derive(Clone, Copy)]
struct Pair<'a> {
    /// Index in [`KEYS`].
    key: usize,
    key_offset: usize,
    value: &'a str,
    /// Offset of the value (after a quote).
    offset: usize,
    quoted: bool,
}

struct Lexer<'a> {
    s: &'a str,
    /// Offset of the space before the next field, or the end.
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn next(&mut self) -> Result<Option<Pair<'a>>, ParseError> {
        let s = self.s;
        let bytes = s.as_bytes();
        if self.pos == bytes.len() {
            return Ok(None);
        }
        // Every field follows exactly one space.
        debug_assert_eq!(bytes[self.pos], b' ');
        let start = self.pos + 1;
        if start == bytes.len() {
            return err(ErrorKind::TrailingGarbage, self.pos);
        }
        let key_end = start
            + bytes[start..]
                .iter()
                .position(|&b| b == b'=' || b == b' ')
                .unwrap_or(bytes.len() - start);
        if key_end == bytes.len() || bytes[key_end] != b'=' || !is_key(&s[start..key_end]) {
            return err(ErrorKind::Syntax, start);
        }
        let Some(key) = KEYS.iter().position(|&k| k == &s[start..key_end]) else {
            return err(ErrorKind::UnknownKey, start);
        };
        let name = KEYS[key];
        let vstart = key_end + 1;
        let field = if bytes.get(vstart) == Some(&b'"') {
            let Some(len) = bytes[vstart + 1..].iter().position(|&b| b == b'"') else {
                return err(ErrorKind::Syntax, vstart);
            };
            let value = &s[vstart + 1..vstart + 1 + len];
            if !is_quotable(value) {
                return err(ErrorKind::BadValue(name), vstart + 1);
            }
            self.pos = vstart + 2 + len;
            if self.pos < bytes.len() && bytes[self.pos] != b' ' {
                return err(ErrorKind::TrailingGarbage, self.pos);
            }
            Pair {
                key,
                key_offset: start,
                value,
                offset: vstart + 1,
                quoted: true,
            }
        } else {
            let end = vstart
                + bytes[vstart..]
                    .iter()
                    .position(|&b| b == b' ')
                    .unwrap_or(bytes.len() - vstart);
            let value = &s[vstart..end];
            if value.is_empty() {
                return err(ErrorKind::Syntax, vstart);
            }
            if !is_word(value) {
                return err(ErrorKind::BadValue(name), vstart);
            }
            self.pos = end;
            Pair {
                key,
                key_offset: start,
                value,
                offset: vstart,
                quoted: false,
            }
        };
        Ok(Some(field))
    }
}

fn is_key(s: &str) -> bool {
    let mut bytes = s.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

#[derive(Clone, Copy)]
struct Slot<'a> {
    pair: Pair<'a>,
    used: bool,
}

/// The fields of a line by key, each taken at most once.
struct Fields<'a> {
    slots: [Option<Slot<'a>>; KEYS.len()],
    /// Length of the line, the offset of a missing field.
    end: usize,
}

fn index(key: &str) -> usize {
    KEYS.iter().position(|&k| k == key).unwrap()
}

impl<'a> Fields<'a> {
    fn insert(&mut self, pair: Pair<'a>) -> Result<(), ParseError> {
        let slot = &mut self.slots[pair.key];
        if slot.is_some() {
            return err(ErrorKind::DuplicateKey(KEYS[pair.key]), pair.key_offset);
        }
        *slot = Some(Slot { pair, used: false });
        Ok(())
    }

    fn peek(&self, key: &'static str) -> Option<&'a str> {
        self.slots[index(key)].map(|s| s.pair.value)
    }

    fn offset(&self, key: &'static str) -> usize {
        self.slots[index(key)].map_or(self.end, |s| s.pair.offset)
    }

    fn take(&mut self, key: &'static str) -> Option<Pair<'a>> {
        let slot = self.slots[index(key)].as_mut()?;
        slot.used = true;
        Some(slot.pair)
    }

    fn opt_word(&mut self, key: &'static str) -> Result<Option<(&'a str, usize)>, ParseError> {
        match self.take(key) {
            None => Ok(None),
            Some(t) if t.quoted => err(ErrorKind::BadValue(key), t.offset),
            Some(t) => Ok(Some((t.value, t.offset))),
        }
    }

    fn word(&mut self, key: &'static str) -> Result<(&'a str, usize), ParseError> {
        self.opt_word(key)?.ok_or(ParseError {
            kind: ErrorKind::MissingField(key),
            offset: self.end,
        })
    }

    fn quoted(&mut self, key: &'static str) -> Result<&'a str, ParseError> {
        match self.take(key) {
            None => err(ErrorKind::MissingField(key), self.end),
            Some(t) if !t.quoted => err(ErrorKind::BadValue(key), t.offset),
            Some(t) => Ok(t.value),
        }
    }

    fn opt_number<T: Number>(&mut self, key: &'static str) -> Result<Option<T>, ParseError> {
        match self.opt_word(key)? {
            None => Ok(None),
            Some((value, at)) => number(value).map(Some).ok_or(ParseError {
                kind: ErrorKind::BadNumber(key),
                offset: at,
            }),
        }
    }

    fn number<T: Number>(&mut self, key: &'static str) -> Result<T, ParseError> {
        self.opt_number(key)?.ok_or(ParseError {
            kind: ErrorKind::MissingField(key),
            offset: self.end,
        })
    }

    /// `reasons=0x<hex>`, lower case, no leading zero.
    fn reasons(&mut self) -> Result<RejectReasons, ParseError> {
        let (value, at) = self.word("reasons")?;
        let bad_number = ParseError {
            kind: ErrorKind::BadNumber("reasons"),
            offset: at,
        };
        let digits = value.strip_prefix("0x").ok_or(bad_number)?;
        if digits.is_empty()
            || digits.len() > 8
            || (digits.len() > 1 && digits.starts_with('0'))
            || !digits
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(bad_number);
        }
        let bits = u32::from_str_radix(digits, 16).map_err(|_| bad_number)?;
        // The core has no constructor from raw bits, so the set is rebuilt
        // from the reasons it defines; other bits cannot be represented.
        let reasons = REASONS
            .iter()
            .filter(|r| bits & r.bits() != 0)
            .fold(RejectReasons::NONE, |acc, &r| acc | r);
        if reasons.bits() != bits {
            return err(ErrorKind::BadValue("reasons"), at);
        }
        Ok(reasons)
    }

    /// `n=` and `assign=`, which must agree.
    fn assignments(&mut self) -> Result<Assignments, ParseError> {
        let n: usize = self.number("n")?;
        let n_at = self.offset("n");
        let (list, at) = self.word("assign")?;
        let mut out = Assignments::new();
        if list != "-" {
            for item in list.split(';') {
                let a = assignment(item).ok_or(ParseError {
                    kind: ErrorKind::BadValue("assign"),
                    offset: at,
                })?;
                out.push(a).map_err(|_| ParseError {
                    kind: ErrorKind::TooManyAssignments,
                    offset: at,
                })?;
            }
        }
        if out.len() != n {
            return err(ErrorKind::CountMismatch, n_at);
        }
        Ok(out)
    }

    /// Fails on the first field (in line order) no part of the parse took.
    fn finish(&self) -> Result<(), ParseError> {
        let unused = self
            .slots
            .iter()
            .flatten()
            .filter(|s| !s.used)
            .min_by_key(|s| s.pair.key_offset);
        match unused {
            Some(s) => err(
                ErrorKind::UnexpectedKey(KEYS[s.pair.key]),
                s.pair.key_offset,
            ),
            None => Ok(()),
        }
    }
}

/// `layer:plane:index:zpos:x,y,w,h:x,y,w,h`.
fn assignment(item: &str) -> Option<Assignment> {
    let mut parts = item.split(':');
    let a = Assignment {
        layer: number(parts.next()?)?,
        plane: PlaneId(number(parts.next()?)?),
        plane_index: number(parts.next()?)?,
        zpos: number(parts.next()?)?,
        source: rect(parts.next()?)?,
        destination: rect(parts.next()?)?,
    };
    parts.next().is_none().then_some(a)
}

fn rect(s: &str) -> Option<Rect> {
    let mut parts = s.split(',');
    let r = Rect::new(
        number(parts.next()?)?,
        number(parts.next()?)?,
        number(parts.next()?)?,
        number(parts.next()?)?,
    );
    parts.next().is_none().then_some(r)
}

/// A decimal number in its one canonical spelling: no `+`, no leading zero,
/// `-` only for a signed type and never on 0.
trait Number: Sized {
    fn parse(s: &str) -> Option<Self>;
}

fn number<T: Number>(s: &str) -> Option<T> {
    T::parse(s)
}

fn magnitude(s: &str) -> Option<u64> {
    if s.is_empty() || (s.len() > 1 && s.starts_with('0')) {
        return None;
    }
    s.bytes().try_fold(0u64, |acc, b| {
        if !b.is_ascii_digit() {
            return None;
        }
        acc.checked_mul(10)?.checked_add(u64::from(b - b'0'))
    })
}

macro_rules! unsigned {
    ($($t:ty),*) => {$(
        impl Number for $t {
            fn parse(s: &str) -> Option<Self> {
                <$t>::try_from(magnitude(s)?).ok()
            }
        }
    )*};
}

unsigned!(u32, u64, usize);

impl Number for i32 {
    fn parse(s: &str) -> Option<Self> {
        match s.strip_prefix('-') {
            Some(m) => {
                let m = magnitude(m)?;
                if m == 0 {
                    return None;
                }
                i32::try_from(-i64::try_from(m).ok()?).ok()
            }
            None => i32::try_from(magnitude(s)?).ok(),
        }
    }
}
