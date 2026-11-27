//! Comparison of the two planners' outcomes per frame (design section 6).
//!
//! For every frame of a file and each model setting:
//!
//! * **core side** ([`crate::core_side`]): under [`Setting::Structural`] the
//!   file is planned with `profile generic` and the accept-all check (the
//!   planner's own static verdict); under [`Setting::Full`] with the file's
//!   profile and the model as the backend check (`TEST_ONLY` modelled).
//! * **libliftoff side**: a [`LiftoffSource`] runs the file; the real one,
//!   [`DriverSource`], runs the C driver through [`crate::pipe`] with
//!   [`ModelCommit`] (the same [`Model`] and setting) answering every
//!   `TEST_ONLY` commit. It runs twice; a frame whose outcome differs
//!   between the runs is `unstable`.
//! * **assignment validity, not equality**: each side's assignment is
//!   replayed through the model and must pass; equal assignments are not
//!   required.
//!
//! "Accepted" is: core returns a plan; libliftoff applied and put **every**
//! visible layer on a plane. A partial libliftoff allocation counts as
//! rejected and the number of layers it placed is kept.
//!
//! Frames with an alpha-0 layer form their own class (design section 7,
//! "invisible layers"): libliftoff drops such layers itself, so the core side
//! plans the frame with those layers removed, and the report marks the line
//! `alpha0`.
//!
//! Every [`FrameReport`] carries the input's origin and the model setting; a
//! recorded or synthetic topology gives no hardware claim.

use std::fmt;
use std::path::{Path, PathBuf};

use composition_core::planner::{PlanError, Verdict};

use crate::core_side::{accept_all, CoreSide, Outcome};
use crate::model::{Errno, Model, Setting};
use crate::pipe::{self, Answer, Apply, FrameResult, Mode, Placement, Run, TestCommit};
use crate::state::{PlaneState, Rect, URect};
use crate::xcheck::{self, Origin, OriginKind, Profile, XCheck, ZposDef};

/// Which side an `invalid-assignment` names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// composition-core.
    Core,
    /// libliftoff.
    Liftoff,
}

/// Outcome class of one frame under one model setting (design section 6).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    /// Both sides accepted, both assignments pass the model.
    BothAccept,
    /// Neither side accepted.
    BothReject,
    /// Core accepted, libliftoff did not.
    CoreOnly,
    /// libliftoff accepted, core did not.
    LiftoffOnly,
    /// That side's assignment fails the model.
    InvalidAssignment(Side),
    /// Core failed with an error that is not a plain `NoPlane` refusal
    /// (`BackendRejected`, `LimitExceeded`, ...); the variant name.
    CoreError(String),
    /// Not compared, with the reason.
    Skipped(String),
    /// libliftoff gave different outcomes in two runs.
    Unstable,
}

impl Class {
    /// The class as printed: `both-accept`, `core-error(LimitExceeded)`, ...
    pub fn label(&self) -> String {
        match self {
            Class::BothAccept => "both-accept".into(),
            Class::BothReject => "both-reject".into(),
            Class::CoreOnly => "core-only".into(),
            Class::LiftoffOnly => "liftoff-only".into(),
            Class::InvalidAssignment(Side::Core) => "invalid-assignment(core)".into(),
            Class::InvalidAssignment(Side::Liftoff) => "invalid-assignment(liftoff)".into(),
            Class::CoreError(e) => format!("core-error({e})"),
            Class::Skipped(r) => format!("skipped({r})"),
            Class::Unstable => "unstable".into(),
        }
    }

    /// Whether the class is a finding (anything but agreement).
    pub fn is_finding(&self) -> bool {
        !matches!(self, Class::BothAccept | Class::BothReject)
    }
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

/// The word of a model setting, as printed.
pub fn setting_word(s: Setting) -> &'static str {
    match s {
        Setting::Structural => "structural",
        Setting::Full => "full",
    }
}

/// The word of an origin kind, as printed.
pub fn origin_word(k: OriginKind) -> &'static str {
    match k {
        OriginKind::Observed => "observed",
        OriginKind::Source => "source",
        OriginKind::Synthetic => "synthetic",
    }
}

/// Variant name of a [`PlanError`] (`NoPlane`, `LimitExceeded`, ...).
pub fn error_name(e: &PlanError) -> String {
    let d = format!("{e:?}");
    d.split(|c: char| !c.is_ascii_alphanumeric())
        .next()
        .unwrap_or_default()
        .to_string()
}

/// What core did with a frame, as kept in a report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreResult {
    /// A plan, as plane states.
    Accept(Vec<PlaneState>),
    /// `NoPlane`: a refusal about planes.
    Reject(PlanError),
    /// Any other planning error.
    Error(PlanError),
    /// Not well formed for core (design section 6) or an unknown output.
    Skipped(PlanError),
}

impl fmt::Display for CoreResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreResult::Accept(states) => {
                let ids: Vec<String> = states.iter().map(|s| s.plane_id.to_string()).collect();
                write!(f, "accept(planes={})", ids.join(","))
            }
            CoreResult::Reject(PlanError::NoPlane { layer, reasons }) => {
                write!(f, "reject(NoPlane layer={layer} {reasons:?})")
            }
            CoreResult::Reject(e) | CoreResult::Error(e) => write!(f, "error({e:?})"),
            CoreResult::Skipped(e) => write!(f, "skipped({e:?})"),
        }
    }
}

/// What libliftoff did with a frame, as kept in a report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiftoffResult {
    /// The driver's result line (first run).
    pub result: FrameResult,
    /// Every visible layer is on a plane and `apply=ok`.
    pub accepted: bool,
    /// Layers libliftoff did not treat as invisible.
    pub visible: u32,
}

impl fmt::Display for LiftoffResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = &self.result;
        let apply = match &r.apply {
            Apply::Ok => "ok".to_string(),
            Apply::Error(n) => format!("error:{n}"),
            Apply::Skipped(w) => format!("skipped:{w}"),
        };
        let planes: Vec<String> = r
            .planes
            .iter()
            .map(|p| match p {
                Placement::Plane(id) => id.to_string(),
                Placement::Composition => "comp".into(),
                Placement::None => "none".into(),
            })
            .collect();
        write!(
            f,
            "{}(placed={}/{} apply={apply} planes={} tests={}{})",
            if self.accepted { "accept" } else { "reject" },
            r.placed,
            self.visible,
            if planes.is_empty() {
                "-".to_string()
            } else {
                planes.join(",")
            },
            r.tests,
            if r.deadline { " deadline" } else { "" }
        )
    }
}

/// The comparison of one frame under one setting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameReport {
    /// Frame name.
    pub frame: String,
    /// Output id of the frame.
    pub output: u32,
    /// Model setting.
    pub setting: Setting,
    /// Outcome class.
    pub class: Class,
    /// Core side.
    pub core: CoreResult,
    /// libliftoff side (`None` when it did not report the frame).
    pub liftoff: Option<LiftoffResult>,
    /// The frame had alpha-0 layers; core planned it without them.
    pub alpha0: bool,
    /// Why a replay failed, for `invalid-assignment`.
    pub detail: Option<String>,
    /// Origin of the input.
    pub origin: Origin,
}

impl FrameReport {
    /// One report line, with `label` (usually `[<origin>:<topology>]`) in
    /// front; it ends with the origin line of the input.
    pub fn line(&self, label: &str) -> String {
        let mut s = format!(
            "{label} model={} frame={} class={}{} core={} liftoff={}",
            setting_word(self.setting),
            self.frame,
            self.class,
            if self.alpha0 { " alpha0" } else { "" },
            self.core,
            self.liftoff
                .as_ref()
                .map_or_else(|| "-".to_string(), |l| l.to_string()),
        );
        if let Some(d) = &self.detail {
            s.push_str(&format!(" detail=\"{d}\""));
        }
        s.push_str(&format!(
            " origin=\"{} {} {}\"",
            origin_word(self.origin.kind),
            self.origin.date,
            self.origin.text
        ));
        s
    }
}

/// Errors of a comparison run.
#[derive(Debug)]
pub enum CompareError {
    /// The file cannot be turned into core capabilities or a model.
    Build(String),
    /// The libliftoff side failed.
    Liftoff(String),
}

impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompareError::Build(m) => write!(f, "cannot build the comparison: {m}"),
            CompareError::Liftoff(m) => write!(f, "libliftoff side: {m}"),
        }
    }
}

impl std::error::Error for CompareError {}

/// The libliftoff side as the comparison sees it: run every frame of `file`
/// with `model` answering the test commits. `file` holds frames of one
/// output only. Implemented by [`DriverSource`] and by test stubs.
pub trait LiftoffSource {
    /// Run `file`; `attempt` is 0 for the first run, 1 for the repeat.
    fn run(&mut self, file: &XCheck, model: &ModelCommit<'_>, attempt: u32) -> Result<Run, String>;
}

/// Answers `TEST_ONLY` commits from a [`Model`] for one output.
pub struct ModelCommit<'m> {
    /// The model.
    pub model: &'m Model<'m>,
    /// The output every commit is for.
    pub output: u32,
}

impl TestCommit for ModelCommit<'_> {
    fn test(&self, states: &[PlaneState]) -> Answer {
        match self.model.check(self.output, states) {
            Ok(()) => Answer::Ok,
            Err(r) => match r.errno {
                Errno::Einval => Answer::Einval,
                Errno::Erange => Answer::Erange,
                Errno::Enospc => Answer::Enospc,
            },
        }
    }
}

/// The real libliftoff side: the C driver over the pipe.
pub struct DriverSource {
    /// Path of `liftoff-xcheck-driver`.
    pub driver: PathBuf,
    /// Directory for the files handed to the driver.
    pub scratch: PathBuf,
    /// Keep a trace of every test commit of the first run in [`Self::trace`].
    pub tracing: bool,
    /// One line per test commit: frame, sequence, answer and the model's
    /// reason for a refusal, then the plane states.
    pub trace: Vec<String>,
    count: u32,
}

impl DriverSource {
    /// A source running `driver`, writing its input files into `scratch`
    /// (created if missing).
    pub fn new(driver: &Path, scratch: &Path) -> Self {
        Self {
            driver: driver.to_path_buf(),
            scratch: scratch.to_path_buf(),
            tracing: false,
            trace: Vec::new(),
            count: 0,
        }
    }
}

impl LiftoffSource for DriverSource {
    fn run(&mut self, file: &XCheck, model: &ModelCommit<'_>, attempt: u32) -> Result<Run, String> {
        std::fs::create_dir_all(&self.scratch)
            .map_err(|e| format!("{}: {e}", self.scratch.display()))?;
        self.count += 1;
        let path = self.scratch.join(format!("run-{:05}.xcheck", self.count));
        let text = xcheck::write(file).map_err(|e| format!("cannot write xcheck: {e}"))?;
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        let run = pipe::run_driver(&self.driver, &path, Mode::Pipe, model)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        // Keep the file only while it is needed: findings are written by the
        // caller as their own files.
        let _ = std::fs::remove_file(&path);
        if self.tracing && attempt == 0 {
            for (req, answer) in &run.requests {
                let states = req.states();
                let reason = match (req.half_set_up(), model.model.check(model.output, &states)) {
                    (true, _) => " reason=\"half set up\"".to_string(),
                    (false, Err(r)) => format!(" reason=\"{}\"", r.reason),
                    (false, Ok(())) => String::new(),
                };
                let planes: Vec<String> = states
                    .iter()
                    .map(|s| {
                        format!(
                            "plane={} fb={:?} src16={},{},{},{} dst={},{},{},{} zpos={} alpha={} blend={:?} rot={:#x}",
                            s.plane_id, s.fb, s.src.x, s.src.y, s.src.w, s.src.h,
                            s.crtc.x, s.crtc.y, s.crtc.w, s.crtc.h, s.zpos, s.alpha, s.blend, s.rotation
                        )
                    })
                    .collect();
                self.trace.push(format!(
                    "trace model={} frame={} seq={} answer={}{reason} {}",
                    setting_word(model.model.setting()),
                    req.frame,
                    req.seq,
                    answer.map_or("-", Answer::word),
                    planes.join(" | ")
                ));
            }
        }
        Ok(run)
    }
}

/// The file as the structural core side sees it: `profile generic`.
/// Explicit `scaling`, `max_source`, `full_output` and `group` lines stay:
/// they are the file's own data, only the profile is dropped.
pub fn structural_file(x: &XCheck) -> XCheck {
    let mut s = x.clone();
    s.profile = Profile::Generic;
    s
}

/// The file as handed to libliftoff: an output with id 0 gets a fresh id
/// (one above every plane, output and buffer id). The C driver uses the
/// output id as the CRTC id, and a KMS CRTC id is never 0: with `CRTC_ID=0`
/// every request is half set up and refused. The model still answers for
/// the original id ([`ModelCommit::output`]), so results are unaffected.
pub fn driver_file(x: &XCheck) -> XCheck {
    let mut d = x.clone();
    if !d.outputs.iter().any(|o| o.id == 0) {
        return d;
    }
    let top = x
        .planes
        .iter()
        .map(|p| p.id)
        .chain(x.outputs.iter().map(|o| o.id))
        .chain(x.buffers.iter().map(|b| b.id))
        .max()
        .unwrap_or(0);
    let fresh = top + 1;
    for o in &mut d.outputs {
        if o.id == 0 {
            o.id = fresh;
        }
    }
    for f in &mut d.frames {
        if f.output == 0 {
            f.output = fresh;
        }
    }
    d
}

/// `frame` without its alpha-0 layers, and whether there were any.
pub fn visible_frame(frame: &xcheck::Frame) -> (xcheck::Frame, bool) {
    let mut f = frame.clone();
    f.layers.retain(|l| l.alpha != 0);
    let had = f.layers.len() != frame.layers.len();
    (f, had)
}

/// Runs core on one frame for `setting`.
fn core_result(
    side: &CoreSide,
    frame: &xcheck::Frame,
    setting: Setting,
    model: &Model<'_>,
) -> CoreResult {
    let outcome = match setting {
        Setting::Structural => side.run_frame(frame, &mut accept_all),
        Setting::Full => side.run_frame(frame, &mut |states: &[PlaneState]| {
            if model.check(frame.output, states).is_ok() {
                Verdict::Accept
            } else {
                Verdict::TryNext
            }
        }),
    };
    match outcome {
        Outcome::Planned(states) => CoreResult::Accept(states),
        Outcome::Skipped(e) => CoreResult::Skipped(e),
        Outcome::Error(e @ PlanError::UnknownOutput(_)) => CoreResult::Skipped(e),
        Outcome::Error(e @ PlanError::NoPlane { .. }) => CoreResult::Reject(e),
        Outcome::Error(e) => CoreResult::Error(e),
    }
}

/// The zpos libliftoff leaves on a plane: the fixed value or `zpos_now`.
fn plane_zpos(x: &XCheck, plane: u32) -> i32 {
    match x.planes.iter().find(|p| p.id == plane) {
        Some(p) => match p.zpos {
            ZposDef::Fixed(n) => n,
            ZposDef::Range { .. } => p.zpos_now.unwrap_or(0),
        },
        None => 0,
    }
}

/// The plane states of libliftoff's final allocation. Taken from the last
/// test commit of the frame that was answered `ok` and enables exactly the
/// placed (plane, framebuffer, source, destination) set; when there is none,
/// rebuilt from the frame's layers (plane zpos, layer alpha, blend and
/// transform), and `false` is returned with it.
pub fn liftoff_states(
    x: &XCheck,
    frame: &xcheck::Frame,
    result: &FrameResult,
    run: &Run,
) -> (Vec<PlaneState>, bool) {
    let rebuilt: Vec<PlaneState> = frame
        .layers
        .iter()
        .zip(&result.planes)
        .filter_map(|(l, p)| match p {
            Placement::Plane(id) => Some(PlaneState {
                plane_id: *id,
                fb: Some(l.buffer),
                src: URect {
                    x: l.src.x << 16,
                    y: l.src.y << 16,
                    w: l.src.w << 16,
                    h: l.src.h << 16,
                },
                crtc: Rect {
                    x: l.dst.x,
                    y: l.dst.y,
                    w: l.dst.w,
                    h: l.dst.h,
                },
                zpos: plane_zpos(x, *id),
                alpha: l.alpha,
                blend: l.blend,
                rotation: l.transform,
            }),
            _ => None,
        })
        .collect();
    let key = |s: &PlaneState| {
        let (r, c) = (s.src, s.crtc);
        (
            s.plane_id,
            s.fb,
            [r.x, r.y, r.w, r.h],
            [c.x, c.y],
            [c.w, c.h],
        )
    };
    let mut want: Vec<_> = rebuilt.iter().map(key).collect();
    want.sort();
    let matched = run
        .requests
        .iter()
        .rev()
        .filter(|(r, a)| r.frame == frame.name && *a == Some(Answer::Ok))
        .map(|(r, _)| r.states())
        .find(|states| {
            let mut have: Vec<_> = states.iter().filter(|s| s.fb.is_some()).map(key).collect();
            have.sort();
            have == want
        });
    match matched {
        Some(states) => (states, true),
        None => (rebuilt, false),
    }
}

fn liftoff_result(r: &FrameResult) -> LiftoffResult {
    let visible = r.planes.iter().filter(|p| **p != Placement::None).count() as u32;
    let all_placed = r
        .planes
        .iter()
        .all(|p| matches!(p, Placement::Plane(_) | Placement::None));
    LiftoffResult {
        result: r.clone(),
        accepted: r.apply == Apply::Ok && all_placed && r.placed == visible,
        visible,
    }
}

/// Whether two runs of libliftoff disagree on a frame.
fn differs(a: &FrameResult, b: &FrameResult) -> bool {
    (&a.apply, a.placed, &a.planes) != (&b.apply, b.placed, &b.planes)
}

/// Classifies one frame from both sides' results. `frame` is the frame as
/// given to libliftoff (alpha-0 layers included), `run` the first libliftoff
/// run (its requests rebuild libliftoff's assignment). Public so that the
/// branches [`compare`] reaches only when core and the model drift apart (a
/// core error from the backend check, a core plan the model refuses) can be
/// tested on made-up results.
pub fn classify(
    x: &XCheck,
    model: &Model<'_>,
    frame: &xcheck::Frame,
    core: &CoreResult,
    first: Option<&FrameResult>,
    second: Option<&FrameResult>,
    run: &Run,
) -> (Class, Option<String>) {
    if let CoreResult::Skipped(e) = core {
        return (Class::Skipped(error_name(e)), None);
    }
    let Some(first) = first else {
        return (Class::Skipped("liftoff:no-result".into()), None);
    };
    if let Apply::Skipped(w) = &first.apply {
        return (Class::Skipped(format!("liftoff:{w}")), None);
    }
    if second.is_some_and(|s| differs(first, s)) {
        return (Class::Unstable, None);
    }
    if first.deadline || second.is_some_and(|s| s.deadline) {
        return (Class::Skipped("deadline".into()), None);
    }
    if let CoreResult::Error(e) = core {
        return (Class::CoreError(error_name(e)), None);
    }
    if let CoreResult::Accept(states) = core {
        if let Err(r) = model.check(frame.output, states) {
            return (
                Class::InvalidAssignment(Side::Core),
                Some(format!("core: {}", r.reason)),
            );
        }
    }
    let lo = liftoff_result(first);
    if lo.accepted {
        let (states, matched) = liftoff_states(x, frame, first, run);
        if let Err(r) = model.check(frame.output, &states) {
            let how = if matched {
                "accepted test commit"
            } else {
                "no accepted test commit matches; rebuilt"
            };
            return (
                Class::InvalidAssignment(Side::Liftoff),
                Some(format!("liftoff ({how}): {}", r.reason)),
            );
        }
    }
    let class = match (matches!(core, CoreResult::Accept(_)), lo.accepted) {
        (true, true) => Class::BothAccept,
        (false, false) => Class::BothReject,
        (true, false) => Class::CoreOnly,
        (false, true) => Class::LiftoffOnly,
    };
    (class, None)
}

/// Compares every frame of `x` under `setting`.
pub fn compare(
    x: &XCheck,
    setting: Setting,
    liftoff: &mut dyn LiftoffSource,
) -> Result<Vec<FrameReport>, CompareError> {
    let core_file = match setting {
        Setting::Structural => structural_file(x),
        Setting::Full => x.clone(),
    };
    let side = CoreSide::new(&core_file).map_err(|e| CompareError::Build(format!("{e:?}")))?;
    let model = Model::new(x, setting).map_err(CompareError::Build)?;

    // One driver run per output: the model answers for one output.
    let mut outputs: Vec<u32> = Vec::new();
    for f in &x.frames {
        if !outputs.contains(&f.output) {
            outputs.push(f.output);
        }
    }
    let mut reports: Vec<Option<FrameReport>> = vec![None; x.frames.len()];
    for out in outputs {
        let index: Vec<usize> = (0..x.frames.len())
            .filter(|&i| x.frames[i].output == out)
            .collect();
        let mut sub = x.clone();
        sub.frames = index.iter().map(|&i| x.frames[i].clone()).collect();
        let sub = driver_file(&sub);
        let commit = ModelCommit {
            model: &model,
            output: out,
        };
        let first = liftoff
            .run(&sub, &commit, 0)
            .map_err(CompareError::Liftoff)?;
        let second = liftoff
            .run(&sub, &commit, 1)
            .map_err(CompareError::Liftoff)?;
        for (k, &i) in index.iter().enumerate() {
            let frame = &x.frames[i];
            let pick = |run: &'_ Run| -> Result<Option<FrameResult>, CompareError> {
                match run.frames.get(k) {
                    Some(r) if r.frame == frame.name => Ok(Some(r.clone())),
                    Some(r) => Err(CompareError::Liftoff(format!(
                        "result {} for frame {} out of order",
                        r.frame, frame.name
                    ))),
                    None => Ok(None),
                }
            };
            let a = pick(&first)?;
            let b = pick(&second)?;
            let (visible, alpha0) = visible_frame(frame);
            let core = core_result(&side, &visible, setting, &model);
            let (class, detail) = classify(x, &model, frame, &core, a.as_ref(), b.as_ref(), &first);
            reports[i] = Some(FrameReport {
                frame: frame.name.clone(),
                output: frame.output,
                setting,
                class,
                core,
                liftoff: a.as_ref().map(liftoff_result),
                alpha0,
                detail,
                origin: x.origin.clone(),
            });
        }
    }
    Ok(reports.into_iter().flatten().collect())
}

/// [`compare`] under both settings, structural first.
pub fn compare_both(
    x: &XCheck,
    liftoff: &mut dyn LiftoffSource,
) -> Result<Vec<FrameReport>, CompareError> {
    let mut all = compare(x, Setting::Structural, liftoff)?;
    all.extend(compare(x, Setting::Full, liftoff)?);
    Ok(all)
}
