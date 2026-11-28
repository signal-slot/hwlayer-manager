//! The display loss of the `recovery` scenario, and how it is judged.
//!
//! The scenario is for an engine on a DRM master descriptor, with no lease:
//! the display is lost while the engine commits, in one of two ways
//! (`--loss`):
//!
//! - [`Loss::Master`] (default): the engine's descriptor drops DRM master,
//!   a second descriptor the harness opens on the same node takes it and
//!   turns the CRTC off (`ACTIVE=0`), as a compositor that starts and takes
//!   the display would. This is the loss row A6 describes for a board
//!   without a lease, and it covers both halves: the engine's descriptor
//!   loses its rights, and the CRTC changes behind it.
//! - [`Loss::Disable`]: the CRTC is turned off with a commit on the engine's
//!   own descriptor, which stays master. Only the CRTC state changes behind
//!   the engine's back; nothing is refused for lack of master.
//!
//! Both are the harness's own atomic commits on the device node it was
//! given. No DRM, udev or seat configuration is touched; the CRTC is turned
//! back on by the new engine run (its first commit is a modeset) or, if
//! that fails, by the harness.

use std::fs::OpenOptions;
use std::io;
use std::os::fd::OwnedFd;
use std::path::Path;

use composition_core::{BufferId, BufferState, DisplayEvent, SubmissionId};
use composition_drm::Card;
use drm::control::atomic::AtomicModeReq;
use drm::control::{crtc, property, AtomicCommitFlags, Device as ControlDevice};
use drm::{ClientCapability, Device as BasicDevice};

use crate::record::list;

/// How the display is taken away from the engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Loss {
    /// Another descriptor takes DRM master and turns the CRTC off.
    #[default]
    Master,
    /// The CRTC is turned off on the engine's own descriptor.
    Disable,
}

impl Loss {
    /// The `--loss` value.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "master" => Ok(Loss::Master),
            "disable" => Ok(Loss::Disable),
            other => Err(format!("--loss: unknown loss {other} (master or disable)")),
        }
    }

    /// Its name, for the records.
    pub fn name(self) -> &'static str {
        match self {
            Loss::Master => "master",
            Loss::Disable => "disable",
        }
    }
}

/// How the engine answered a frame offered after the loss.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Report {
    /// `validate`, `commit` or the event poll returned an error.
    Error(String),
    /// An event about the device or the frame: `DeviceLost`,
    /// `LatchTimeout` or `Dropped`.
    Event(&'static str),
    /// The frame latched: the engine saw nothing of the loss.
    Latched,
    /// No error, no event and no latch within the time allowed.
    Silent,
}

impl Report {
    /// Whether the engine reported the loss.
    pub fn reported(&self) -> bool {
        matches!(self, Report::Error(_) | Report::Event(_))
    }

    /// One word, with the error or event, as a record value.
    pub fn text(&self) -> String {
        match self {
            Report::Error(e) => format!("error:{}", e.replace(' ', "")),
            Report::Event(name) => format!("event:{name}"),
            Report::Latched => "latched".into(),
            Report::Silent => "silent".into(),
        }
    }
}

/// What the events seen after `submission` was accepted say, the first
/// decisive one in order; `None` while none is.
pub fn report_of(submission: SubmissionId, events: &[DisplayEvent]) -> Option<Report> {
    events.iter().find_map(|event| match *event {
        DisplayEvent::DeviceLost => Some(Report::Event("DeviceLost")),
        DisplayEvent::LatchTimeout { submission: s, .. } if s == submission => {
            Some(Report::Event("LatchTimeout"))
        }
        DisplayEvent::Dropped { submission: s, .. } if s == submission => {
            Some(Report::Event("Dropped"))
        }
        DisplayEvent::Presented { submission: s, .. } if s == submission => Some(Report::Latched),
        _ => None,
    })
}

/// The buffers the display still holds: submitted or scanning.
pub fn busy(states: &[(BufferId, Option<BufferState>)]) -> Vec<BufferId> {
    states
        .iter()
        .filter(|(_, state)| matches!(state, Some(BufferState::Submitted | BufferState::Scanning)))
        .map(|(id, _)| *id)
        .collect()
}

/// Buffer states as `id:State`, comma-separated.
pub fn states_text(states: &[(BufferId, Option<BufferState>)]) -> String {
    list(states.iter().map(|(id, state)| match state {
        Some(state) => format!("{}:{state:?}", id.0),
        None => format!("{}:unknown", id.0),
    }))
}

/// What the scenario found after the loss, for [`judge`].
#[derive(Debug)]
pub struct Findings {
    /// The loss made.
    pub loss: Loss,
    /// Whether the loss could be made as intended: master dropped, taken by
    /// the second descriptor, CRTC turned off. The error text if not.
    pub made: Result<(), String>,
    /// The first frame after the loss.
    pub first: Report,
    /// With [`Loss::Master`]: the frame after master was taken back, on the
    /// same engine (the CRTC is still off).
    pub second: Option<Report>,
    /// Buffers still submitted or scanning once the loss was reported.
    pub busy: Vec<BufferId>,
    /// The buffers of the last frame that latched before the loss was
    /// reported: what the display still holds unless the engine reported
    /// `DeviceLost`. See [`kept`].
    pub shown: Vec<BufferId>,
    /// With [`Loss::Master`]: taking master back on the engine's descriptor.
    pub master_back: Option<Result<(), String>>,
    /// Commits the new engine run latched.
    pub relatched: u32,
    /// What ended the new run early, if anything.
    pub new_run_error: Option<String>,
    /// Whether the CRTC is on at the end; `None` if it could not be read.
    pub active_at_end: Option<bool>,
}

/// The busy buffers the engine should have let go of after the loss was
/// reported.
///
/// After `DeviceLost` the engine forgets every submission and releases
/// every buffer, so none may stay busy. After an error from `validate` or
/// `commit`, or a `LatchTimeout`, the engine's contract is that the display
/// and the buffer states are unchanged (`Engine::commit`: "On failure the
/// display and all buffer states are unchanged"): the last latched frame
/// stays scanning, as its framebuffers stay on the planes when the CRTC is
/// only turned off. Those buffers are released when the engine and its
/// descriptor go; any other busy buffer is one the engine did not let go.
pub fn kept(f: &Findings) -> Vec<BufferId> {
    let lost = [Some(&f.first), f.second.as_ref()].contains(&Some(&Report::Event("DeviceLost")));
    f.busy
        .iter()
        .filter(|b| lost || !f.shown.contains(b))
        .copied()
        .collect()
}

/// The failures of a recovery run; empty means PASS. `budget_ms` is the
/// time the engine had to report the loss.
pub fn judge(f: &Findings, budget_ms: u64) -> Vec<String> {
    let mut failures = Vec::new();
    if let Err(e) = &f.made {
        failures.push(format!("the {} loss was not made: {e}", f.loss.name()));
    }
    match &f.first {
        Report::Latched => failures
            .push("the frame after the loss latched: the engine saw nothing of the loss".into()),
        Report::Silent => failures.push(format!(
            "no error, DeviceLost or LatchTimeout within {budget_ms} ms of the loss: \
             the engine stalled silently"
        )),
        _ => {}
    }
    if f.second == Some(Report::Silent) {
        failures.push(format!(
            "with master back and the CRTC off, no error, event or latch within \
             {budget_ms} ms: the engine stalled silently"
        ));
    }
    let kept = kept(f);
    if !kept.is_empty() {
        failures.push(format!(
            "buffers {} still submitted or scanning after the loss was reported: \
             the engine did not release them",
            list(kept.iter().map(|b| b.0))
        ));
    }
    if let Some(Err(e)) = &f.master_back {
        failures.push(format!("taking DRM master back failed: {e}"));
    }
    match &f.new_run_error {
        Some(e) => failures.push(format!("new run: {e}")),
        None if f.relatched == 0 => failures.push("the new run latched no commit".into()),
        None => {}
    }
    if f.active_at_end == Some(false) {
        failures.push("the CRTC is still off at the end".into());
    }
    failures
}

/// Opens a second descriptor on `device`, with the client capabilities the
/// backend sets, and makes it DRM master. The engine's descriptor must have
/// dropped master first.
pub fn open_master(device: &Path) -> io::Result<Card> {
    let file = OpenOptions::new().read(true).write(true).open(device)?;
    let card = Card::from_fd(OwnedFd::from(file));
    card.set_client_capability(ClientCapability::UniversalPlanes, true)?;
    card.set_client_capability(ClientCapability::Atomic, true)?;
    card.acquire_master_lock()?;
    Ok(card)
}

fn active_property(card: &Card, crtc: crtc::Handle) -> io::Result<(property::Handle, u64)> {
    let set = card.get_properties(crtc)?;
    let (handles, values) = set.as_props_and_values();
    for (handle, value) in handles.iter().zip(values) {
        if card.get_property(*handle)?.name().to_bytes() == b"ACTIVE" {
            return Ok((*handle, *value));
        }
    }
    Err(io::Error::other(format!(
        "CRTC {} has no ACTIVE property",
        u32::from(crtc)
    )))
}

/// Whether the CRTC is on (`ACTIVE`).
pub fn crtc_active(card: &Card, crtc: crtc::Handle) -> io::Result<bool> {
    active_property(card, crtc).map(|(_, value)| value != 0)
}

/// Turns the CRTC off with a blocking `ALLOW_MODESET` commit of `ACTIVE=0`
/// alone: its mode, connector and planes stay set, as a DPMS off.
pub fn turn_off(card: &Card, crtc: crtc::Handle) -> io::Result<()> {
    let (active, _) = active_property(card, crtc)?;
    let mut request = AtomicModeReq::new();
    request.add_property(crtc, active, property::Value::Boolean(false));
    card.atomic_commit(AtomicCommitFlags::ALLOW_MODESET, request)
}

/// A result as a record value: `ok` or the error without spaces.
pub fn result_text<E: std::fmt::Display>(result: &Result<(), E>) -> String {
    match result {
        Ok(()) => "ok".into(),
        Err(e) => e.to_string().replace(' ', "_"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use composition_core::OutputId;

    const SUB: SubmissionId = SubmissionId(7);
    const OUT: OutputId = OutputId(31);

    fn passing() -> Findings {
        Findings {
            loss: Loss::Master,
            made: Ok(()),
            first: Report::Error("validate: Backend(Platform(13))".into()),
            second: Some(Report::Error("commit: Backend(Rejected)".into())),
            busy: Vec::new(),
            shown: Vec::new(),
            master_back: Some(Ok(())),
            relatched: 3,
            new_run_error: None,
            active_at_end: Some(true),
        }
    }

    #[test]
    fn loss_parses() {
        assert_eq!(Loss::parse("master"), Ok(Loss::Master));
        assert_eq!(Loss::parse("disable"), Ok(Loss::Disable));
        assert!(Loss::parse("unplug").is_err());
        assert_eq!(Loss::default(), Loss::Master);
        assert_eq!(Loss::Disable.name(), "disable");
    }

    #[test]
    fn the_first_decisive_event_is_the_report() {
        let other = SubmissionId(6);
        let released = DisplayEvent::Released {
            buffer: BufferId(1),
        };
        assert_eq!(report_of(SUB, &[]), None);
        assert_eq!(report_of(SUB, &[released]), None);
        // Events about another submission decide nothing.
        let stale = [
            DisplayEvent::Presented {
                output: OUT,
                submission: other,
            },
            DisplayEvent::LatchTimeout {
                output: OUT,
                submission: other,
            },
        ];
        assert_eq!(report_of(SUB, &stale), None);
        let timed_out = [
            released,
            DisplayEvent::LatchTimeout {
                output: OUT,
                submission: SUB,
            },
            DisplayEvent::DeviceLost,
        ];
        assert_eq!(
            report_of(SUB, &timed_out),
            Some(Report::Event("LatchTimeout"))
        );
        assert_eq!(
            report_of(SUB, &[DisplayEvent::DeviceLost]),
            Some(Report::Event("DeviceLost"))
        );
        let dropped = DisplayEvent::Dropped {
            output: OUT,
            submission: SUB,
        };
        assert_eq!(report_of(SUB, &[dropped]), Some(Report::Event("Dropped")));
        let latched = DisplayEvent::Presented {
            output: OUT,
            submission: SUB,
        };
        assert_eq!(report_of(SUB, &[latched]), Some(Report::Latched));
    }

    #[test]
    fn only_errors_and_events_are_reports() {
        assert!(Report::Error("x".into()).reported());
        assert!(Report::Event("DeviceLost").reported());
        assert!(!Report::Latched.reported());
        assert!(!Report::Silent.reported());
        assert_eq!(
            Report::Error("validate: Backend(Platform(13))".into()).text(),
            "error:validate:Backend(Platform(13))"
        );
        assert_eq!(Report::Event("LatchTimeout").text(), "event:LatchTimeout");
    }

    #[test]
    fn busy_buffers_are_submitted_or_scanning() {
        let states = [
            (BufferId(1), Some(BufferState::Scanning)),
            (BufferId(2), Some(BufferState::Ready)),
            (BufferId(3), Some(BufferState::Submitted)),
            (BufferId(4), Some(BufferState::Writable)),
            (BufferId(5), None),
        ];
        assert_eq!(busy(&states), vec![BufferId(1), BufferId(3)]);
        assert_eq!(states_text(&states[..2]), "1:Scanning,2:Ready".to_string());
        assert_eq!(states_text(&[]), "-");
    }

    #[test]
    fn a_reported_loss_with_everything_released_and_relatched_passes() {
        assert_eq!(judge(&passing(), 2000), Vec::<String>::new());
        let disable = Findings {
            loss: Loss::Disable,
            first: Report::Event("LatchTimeout"),
            second: None,
            master_back: None,
            ..passing()
        };
        assert_eq!(judge(&disable, 2000), Vec::<String>::new());
        // The same engine may latch again once master is back: not a stall.
        let latched_again = Findings {
            second: Some(Report::Latched),
            ..passing()
        };
        assert!(judge(&latched_again, 2000).is_empty());
    }

    #[test]
    fn a_silent_stall_fails_with_its_reason() {
        let f = Findings {
            first: Report::Silent,
            ..passing()
        };
        let failures = judge(&f, 2000);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("within 2000 ms"), "{failures:?}");
        assert!(failures[0].contains("stalled silently"), "{failures:?}");
        let f = Findings {
            second: Some(Report::Silent),
            ..passing()
        };
        assert!(judge(&f, 500)[0].contains("with master back"));
    }

    #[test]
    fn an_unnoticed_loss_fails() {
        let f = Findings {
            first: Report::Latched,
            ..passing()
        };
        assert!(judge(&f, 2000)[0].contains("saw nothing of the loss"));
    }

    #[test]
    fn buffers_kept_after_the_report_fail() {
        let f = Findings {
            busy: vec![BufferId(3), BufferId(4)],
            ..passing()
        };
        let failures = judge(&f, 2000);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].starts_with("buffers 3,4 still submitted or scanning"));
    }

    // The vkms A6 runs (both --loss modes): the loss is reported as an
    // error, which by the engine's contract changes no buffer state, so the
    // last latched frame (5, 6) stays scanning. That is not a leak.
    #[test]
    fn the_last_frame_still_scanning_after_an_error_report_passes() {
        let master = Findings {
            busy: vec![BufferId(5), BufferId(6)],
            shown: vec![BufferId(5), BufferId(6)],
            ..passing()
        };
        assert_eq!(judge(&master, 2000), Vec::<String>::new());
        let disable = Findings {
            loss: Loss::Disable,
            first: Report::Error("commit: Backend(Rejected)".into()),
            second: None,
            master_back: None,
            ..master
        };
        assert_eq!(judge(&disable, 2000), Vec::<String>::new());
    }

    #[test]
    fn buffers_beyond_the_last_frame_or_after_device_lost_fail() {
        // A buffer of a frame offered after the loss stays submitted.
        let f = Findings {
            busy: vec![BufferId(1), BufferId(5), BufferId(6)],
            shown: vec![BufferId(5), BufferId(6)],
            ..passing()
        };
        let failures = judge(&f, 2000);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].starts_with("buffers 1 still submitted"));
        // DeviceLost releases everything, the last frame as well.
        for (first, second) in [
            (Report::Event("DeviceLost"), None),
            (
                Report::Error("validate: Backend(Platform(13))".into()),
                Some(Report::Event("DeviceLost")),
            ),
        ] {
            let f = Findings {
                first,
                second,
                busy: vec![BufferId(5), BufferId(6)],
                shown: vec![BufferId(5), BufferId(6)],
                ..passing()
            };
            assert_eq!(kept(&f), vec![BufferId(5), BufferId(6)]);
            assert!(judge(&f, 2000)[0].starts_with("buffers 5,6 still submitted"));
        }
    }

    #[test]
    fn the_recovery_half_has_its_own_failures() {
        let f = Findings {
            master_back: Some(Err("Permission denied".into())),
            relatched: 0,
            ..passing()
        };
        let failures = judge(&f, 2000);
        assert_eq!(
            failures,
            vec![
                "taking DRM master back failed: Permission denied".to_string(),
                "the new run latched no commit".to_string(),
            ]
        );
        let f = Findings {
            new_run_error: Some("not DRM master".into()),
            relatched: 0,
            active_at_end: Some(false),
            ..passing()
        };
        assert_eq!(
            judge(&f, 2000),
            vec![
                "new run: not DRM master".to_string(),
                "the CRTC is still off at the end".to_string(),
            ]
        );
        let f = Findings {
            made: Err("the second descriptor is not master".into()),
            ..passing()
        };
        assert_eq!(
            judge(&f, 2000),
            vec!["the master loss was not made: the second descriptor is not master".to_string()]
        );
        // An unreadable CRTC state is recorded, not failed.
        let f = Findings {
            active_at_end: None,
            ..passing()
        };
        assert!(judge(&f, 2000).is_empty());
    }

    #[test]
    fn results_are_one_word() {
        assert_eq!(result_text::<String>(&Ok(())), "ok");
        assert_eq!(
            result_text(&Err(io::Error::other("Permission denied"))),
            "Permission_denied"
        );
    }
}
