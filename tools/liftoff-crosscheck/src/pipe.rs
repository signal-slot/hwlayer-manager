//! Rust side of the TEST_ONLY pipe answering the C driver (plan item xc-driver).
//!
//! The C driver (`c/driver.c`, protocol in `c/PROTOCOL.md`) runs libliftoff on
//! an xcheck file. Every `TEST_ONLY` atomic commit libliftoff makes is written
//! as a `test` line; this module parses it into [`PlaneState`]s, asks a
//! [`TestCommit`] and writes the one-word answer back. At the end of each
//! frame the driver prints a `result` line, which becomes a [`FrameResult`]
//! that keeps the input's origin text.
//!
//! # Assumption for the project owner
//!
//! The design (`docs/libliftoff-crosscheck.md`) leaves open whether the
//! test-commit model is written once or twice. This module follows the
//! design's stated preference: **the rules live once, on the Rust side**. The
//! C driver has no model; whatever implements [`TestCommit`] (the model in
//! `model.rs`) is the only fake kernel. If the owner decides on two models,
//! this module becomes a trace reader only.
//!
//! # Mapping to [`PlaneState`]
//!
//! `fb=0` becomes `fb: None`. `blend=-` (the plane has no such property)
//! becomes [`BlendMode::Opaque`] (KMS value 0), which is what the model accepts
//! on a plane that lists no blend mode (`model.rs`).
//! A request `zpos` outside `i32` is a [`PipeError::Parse`]. [`PlaneState`]
//! has no CRTC id, so a listed plane with `fb=0` or `crtc=0` (half set up,
//! which the kernel refuses) is answered `einval` here, without asking the
//! responder; see [`Request::half_set_up`].

use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::state::{BlendMode, PlaneState, Rect, URect};

/// The responder's answer to one test commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The commit passes (`0`).
    Ok,
    /// `-EINVAL`.
    Einval,
    /// `-ERANGE`.
    Erange,
    /// `-ENOSPC`.
    Enospc,
}

impl Answer {
    /// The protocol word.
    pub fn word(self) -> &'static str {
        match self {
            Answer::Ok => "ok",
            Answer::Einval => "einval",
            Answer::Erange => "erange",
            Answer::Enospc => "enospc",
        }
    }
}

/// The test-commit model: decides whether the kernel would accept a commit.
pub trait TestCommit {
    /// Judge the commit in which exactly `states` are enabled (all other
    /// planes are disabled), in the driver's plane order.
    fn test(&self, states: &[PlaneState]) -> Answer;
}

/// Answers `ok` to everything.
#[derive(Clone, Copy, Debug, Default)]
pub struct AcceptAll;

impl TestCommit for AcceptAll {
    fn test(&self, _: &[PlaneState]) -> Answer {
        Answer::Ok
    }
}

/// Answers `einval` to everything.
#[derive(Clone, Copy, Debug, Default)]
pub struct RejectAll;

impl TestCommit for RejectAll {
    fn test(&self, _: &[PlaneState]) -> Answer {
        Answer::Einval
    }
}

impl<F: Fn(&[PlaneState]) -> Answer> TestCommit for F {
    fn test(&self, states: &[PlaneState]) -> Answer {
        self(states)
    }
}

/// Errors of the pipe.
#[derive(Debug)]
pub enum PipeError {
    /// I/O failure talking to the driver or starting it.
    Io(io::Error),
    /// A line that does not follow the protocol.
    Parse(String),
    /// The driver exited with a non-zero status (its stderr went to ours).
    Exit(Option<i32>),
}

impl fmt::Display for PipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipeError::Io(e) => write!(f, "driver i/o: {e}"),
            PipeError::Parse(m) => write!(f, "driver protocol: {m}"),
            PipeError::Exit(Some(c)) => write!(f, "driver exited with status {c}"),
            PipeError::Exit(None) => write!(f, "driver was killed by a signal"),
        }
    }
}

impl std::error::Error for PipeError {}

impl From<io::Error> for PipeError {
    fn from(e: io::Error) -> Self {
        PipeError::Io(e)
    }
}

fn perr<T>(line: &str, msg: &str) -> Result<T, PipeError> {
    Err(PipeError::Parse(format!("{msg}: {line:?}")))
}

/// One plane of a request line, with the id [`PlaneState`] lacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestPlane {
    /// The plane state handed to the model.
    pub state: PlaneState,
    /// `crtc=` of the line (0 when unset).
    pub crtc_id: u32,
}

/// A parsed `test` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// Frame name.
    pub frame: String,
    /// 1, 2, ... within the frame.
    pub seq: u32,
    /// Commit flags.
    pub flags: u32,
    /// Listed planes, in the file's plane order.
    pub planes: Vec<RequestPlane>,
}

impl Request {
    /// The plane states, for [`TestCommit::test`].
    pub fn states(&self) -> Vec<PlaneState> {
        self.planes.iter().map(|p| p.state).collect()
    }

    /// True when a listed plane has `fb=0` or `crtc=0`, which the kernel
    /// refuses with `EINVAL`.
    pub fn half_set_up(&self) -> bool {
        self.planes
            .iter()
            .any(|p| p.state.fb.is_none() || p.crtc_id == 0)
    }
}

fn num<T: std::str::FromStr>(s: &str, line: &str) -> Result<T, PipeError> {
    match s.parse() {
        Ok(v) => Ok(v),
        Err(_) => perr(line, &format!("bad number {s:?}")),
    }
}

fn hex(s: &str, line: &str) -> Result<u32, PipeError> {
    match s.strip_prefix("0x").map(|h| u32::from_str_radix(h, 16)) {
        Some(Ok(v)) => Ok(v),
        _ => perr(line, &format!("bad hex number {s:?}")),
    }
}

fn four<T: std::str::FromStr>(s: &str, line: &str) -> Result<[T; 4], PipeError> {
    let v: Vec<&str> = s.split(',').collect();
    if v.len() != 4 {
        return perr(line, &format!("expected four numbers, got {s:?}"));
    }
    Ok([
        num(v[0], line)?,
        num(v[1], line)?,
        num(v[2], line)?,
        num(v[3], line)?,
    ])
}

/// Parse a `test …` line (without its newline).
pub fn parse_request(line: &str) -> Result<Request, PipeError> {
    let mut it = line.split_whitespace();
    if it.next() != Some("test") {
        return perr(line, "not a test line");
    }
    let mut frame = None;
    let mut seq = None;
    let mut flags = None;
    let mut count: Option<usize> = None;
    let mut planes: Vec<RequestPlane> = Vec::new();
    // Fields of the plane being read; `plane=` starts a new one.
    let mut cur: Option<PlaneFields> = None;
    for tok in it {
        let Some((k, v)) = tok.split_once('=') else {
            return perr(line, &format!("field without '=': {tok:?}"));
        };
        match k {
            "frame" => frame = Some(v.to_string()),
            "seq" => seq = Some(num(v, line)?),
            "flags" => flags = Some(hex(v, line)?),
            "planes" => count = Some(num(v, line)?),
            "plane" => {
                if let Some(c) = cur.take() {
                    planes.push(c.finish(line)?);
                }
                cur = Some(PlaneFields {
                    plane: num(v, line)?,
                    ..PlaneFields::default()
                });
            }
            _ => {
                let Some(c) = cur.as_mut() else {
                    return perr(line, &format!("unknown field {k:?}"));
                };
                c.set(k, v, line)?;
            }
        }
    }
    if let Some(c) = cur.take() {
        planes.push(c.finish(line)?);
    }
    let (Some(frame), Some(seq), Some(flags), Some(count)) = (frame, seq, flags, count) else {
        return perr(line, "missing frame, seq, flags or planes");
    };
    if planes.len() != count {
        return perr(line, &format!("planes={count} but {} listed", planes.len()));
    }
    Ok(Request {
        frame,
        seq,
        flags,
        planes,
    })
}

#[derive(Default)]
struct PlaneFields {
    plane: u32,
    fb: Option<u32>,
    crtc: Option<u32>,
    src: Option<[u32; 4]>,
    dst: Option<[i64; 4]>,
    alpha: Option<u16>,
    blend: Option<BlendMode>,
    rotation: Option<u32>,
    zpos: Option<i64>,
}

impl PlaneFields {
    fn set(&mut self, k: &str, v: &str, line: &str) -> Result<(), PipeError> {
        match k {
            "fb" => self.fb = Some(num(v, line)?),
            "crtc" => self.crtc = Some(num(v, line)?),
            "src16" => self.src = Some(four(v, line)?),
            "dst" => self.dst = Some(four(v, line)?),
            "alpha" => self.alpha = Some(num(v, line)?),
            "blend" => {
                self.blend = Some(match v {
                    // "-": plane has no property; see the module docs.
                    "opaque" | "-" => BlendMode::Opaque,
                    "premul" => BlendMode::Premultiplied,
                    "coverage" => BlendMode::Coverage,
                    _ => return perr(line, &format!("unknown blend {v:?}")),
                })
            }
            "rotation" => self.rotation = Some(hex(v, line)?),
            "zpos" => self.zpos = Some(num(v, line)?),
            // Unknown plane fields may be added later; ignore them.
            _ => {}
        }
        Ok(())
    }

    fn finish(self, line: &str) -> Result<RequestPlane, PipeError> {
        let (Some(fb), Some(crtc), Some(src), Some(dst)) = (self.fb, self.crtc, self.src, self.dst)
        else {
            return perr(line, "plane lacks fb, crtc, src16 or dst");
        };
        let (Some(alpha), Some(blend), Some(rotation), Some(zpos)) =
            (self.alpha, self.blend, self.rotation, self.zpos)
        else {
            return perr(line, "plane lacks alpha, blend, rotation or zpos");
        };
        let Ok(zpos) = i32::try_from(zpos) else {
            return perr(line, "zpos outside i32");
        };
        let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
            i32::try_from(dst[0]),
            i32::try_from(dst[1]),
            u32::try_from(dst[2]),
            u32::try_from(dst[3]),
        ) else {
            return perr(line, "dst outside range");
        };
        Ok(RequestPlane {
            state: PlaneState {
                plane_id: self.plane,
                fb: (fb != 0).then_some(fb),
                src: URect {
                    x: src[0],
                    y: src[1],
                    w: src[2],
                    h: src[3],
                },
                crtc: Rect { x, y, w, h },
                zpos,
                alpha,
                blend,
                rotation,
            },
            crtc_id: crtc,
        })
    }
}

/// Where libliftoff put one layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// On this plane.
    Plane(u32),
    /// Left to composition.
    Composition,
    /// Invisible layer (`none`).
    None,
}

/// Outcome of `liftoff_output_apply`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Apply {
    /// Applied.
    Ok,
    /// Failed with this positive errno.
    Error(i32),
    /// Not run, with the driver's reason (`unknown-output`, `unknown-buffer`).
    Skipped(String),
}

/// A parsed `result` line: libliftoff's outcome for one frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameResult {
    /// Frame name.
    pub frame: String,
    /// Output id.
    pub output: u32,
    /// Layers in the frame.
    pub layers: u32,
    /// Result of `liftoff_output_apply`.
    pub apply: Apply,
    /// Layers libliftoff put on a plane.
    pub placed: u32,
    /// One entry per layer, in frame order.
    pub planes: Vec<Placement>,
    /// libliftoff's deadline passed.
    pub deadline: bool,
    /// Request lines written for this frame.
    pub tests: u32,
    /// The input's origin text, verbatim.
    pub origin: String,
}

/// Parse a `result …` line (without its newline).
pub fn parse_result(line: &str) -> Result<FrameResult, PipeError> {
    let Some((head, origin)) = line.split_once(" origin=") else {
        return perr(line, "result line without origin");
    };
    let mut it = head.split_whitespace();
    if it.next() != Some("result") {
        return perr(line, "not a result line");
    }
    let (mut frame, mut output, mut layers, mut apply) = (None, None, None, None);
    let (mut placed, mut planes, mut deadline, mut tests) = (None, None, None, None);
    for tok in it {
        let Some((k, v)) = tok.split_once('=') else {
            return perr(line, &format!("field without '=': {tok:?}"));
        };
        match k {
            "frame" => frame = Some(v.to_string()),
            "output" => output = Some(num(v, line)?),
            "layers" => layers = Some(num(v, line)?),
            "apply" => {
                apply = Some(if v == "ok" {
                    Apply::Ok
                } else if let Some(e) = v.strip_prefix("error:") {
                    Apply::Error(num(e, line)?)
                } else if let Some(w) = v.strip_prefix("skipped:") {
                    Apply::Skipped(w.to_string())
                } else {
                    return perr(line, &format!("unknown apply {v:?}"));
                })
            }
            "placed" => placed = Some(num(v, line)?),
            "planes" => {
                let mut list = Vec::new();
                if v != "-" {
                    for e in v.split(',') {
                        list.push(match e {
                            "composition" => Placement::Composition,
                            "none" => Placement::None,
                            _ => Placement::Plane(num(e, line)?),
                        });
                    }
                }
                planes = Some(list);
            }
            "deadline" => {
                deadline = Some(match v {
                    "0" => false,
                    "1" => true,
                    _ => return perr(line, "deadline is not 0 or 1"),
                })
            }
            "tests" => tests = Some(num(v, line)?),
            // Unknown fields may be added later; ignore them.
            _ => {}
        }
    }
    let (
        Some(frame),
        Some(output),
        Some(layers),
        Some(apply),
        Some(placed),
        Some(planes),
        Some(deadline),
        Some(tests),
    ) = (
        frame, output, layers, apply, placed, planes, deadline, tests,
    )
    else {
        return perr(line, "result line lacks a field");
    };
    Ok(FrameResult {
        frame,
        output,
        layers,
        apply,
        placed,
        planes,
        deadline,
        tests,
        origin: origin.to_string(),
    })
}

/// How the driver answers: over the pipe, or by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Ask the responder (default).
    Pipe,
    /// The driver's own `--accept-all`; requests are logged, not answered.
    DriverAcceptAll,
    /// The driver's own `--reject-all`.
    DriverRejectAll,
}

/// What a run of the driver produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Run {
    /// Per-frame results, in file order.
    pub frames: Vec<FrameResult>,
    /// Every request with the answer given (`None` when the driver answered).
    pub requests: Vec<(Request, Option<Answer>)>,
}

/// Run the driver on `file` and answer its requests with `model`.
pub fn run_driver(
    driver: &Path,
    file: &Path,
    mode: Mode,
    model: &dyn TestCommit,
) -> Result<Run, PipeError> {
    let mut cmd = Command::new(driver);
    match mode {
        Mode::Pipe => {}
        Mode::DriverAcceptAll => {
            cmd.arg("--accept-all");
        }
        Mode::DriverRejectAll => {
            cmd.arg("--reject-all");
        }
    }
    cmd.arg(file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = cmd.spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("no stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no stdout"))?;
    let out = serve(BufReader::new(stdout), &mut stdin, mode, model);
    drop(stdin);
    if out.is_err() {
        // Do not wait for a driver that may still be blocked on us.
        let _ = child.kill();
    }
    let status = child.wait()?;
    let run = out?;
    if !status.success() {
        return Err(PipeError::Exit(status.code()));
    }
    Ok(run)
}

/// The protocol loop over any reader and writer (testable without a driver).
pub fn serve<R: BufRead, W: Write>(
    reader: R,
    answers: &mut W,
    mode: Mode,
    model: &dyn TestCommit,
) -> Result<Run, PipeError> {
    let mut run = Run::default();
    for line in reader.lines() {
        let line = line?;
        match line.split_whitespace().next() {
            Some("test") => {
                let req = parse_request(&line)?;
                let given = if mode == Mode::Pipe {
                    let a = if req.half_set_up() {
                        Answer::Einval
                    } else {
                        model.test(&req.states())
                    };
                    writeln!(answers, "{}", a.word())?;
                    answers.flush()?;
                    Some(a)
                } else {
                    None
                };
                run.requests.push((req, given));
            }
            Some("result") => run.frames.push(parse_result(&line)?),
            // Other first words may be added later; readers ignore them.
            _ => {}
        }
    }
    Ok(run)
}

/// Where `build-liftoff.sh` lives, for tests and the sweep.
pub fn build_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("build-liftoff.sh")
}
