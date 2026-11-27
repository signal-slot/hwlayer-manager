//! `drm-harness`: a minimal producer harness for composition-drm.
//!
//! Drives the engine on one output of a real DRM device, on a descriptor
//! that is DRM master, a kernel DRM lease or a lease from drm-lease-manager,
//! with buffers it allocates itself or receives from producer processes,
//! and prints one-line `key=value` records ending in
//! `VERDICT <scenario> PASS|FAIL <reason>`. See the README.

#![forbid(unsafe_code)]

mod conn_names;
mod draw;
mod lease;
mod lessor;
mod producer;
mod record;
mod scenarios;
mod session;
mod source;
mod stats;
mod stop;
mod wire;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::Ordering;

use composition_drm::DriverProfile;

use crate::lease::LeaseRequest;
use crate::record::{verdict_line, Record};
use crate::scenarios::{Outcome, MOVE_FRAMES, REVOKE_FRAMES, ROTATE_FRAMES};
use crate::session::{Options, ProducerMode};
use crate::source::Source;
use crate::stats::Usage;

type Scenario = fn(&Options) -> Result<Outcome, String>;

/// The scenarios in the order `all` runs them: the one that never touches
/// the display first.
const SCENARIOS: [(&str, Scenario, &str); 5] = [
    (
        "combos",
        scenarios::combos,
        "layer combinations the planner and TEST_ONLY accept or refuse; nothing is shown",
    ),
    (
        "two-planes",
        scenarios::two_planes,
        "two separately allocated buffers on two hardware planes, held --seconds",
    ),
    (
        "zorder-alpha",
        scenarios::zorder_alpha,
        "swap the z-order, then plane alpha, then premultiplied pixel alpha, each held",
    ),
    (
        "move",
        scenarios::move_top,
        "move the top plane for --frames frames over an untouched bottom buffer",
    ),
    (
        "rotate",
        scenarios::rotate,
        "rotate --buffers buffers per plane for --frames frames; check release order, measure",
    ),
];

/// Scenarios that need a lease made by this process; `all` runs them last,
/// and only with `--source kernel-lease`.
const LEASE_SCENARIOS: [(&str, Scenario, &str); 1] = [(
    "revoke",
    scenarios::revoke,
    "kernel-lease only: rotate, then the lessor revokes the lease mid-rotation;\n\
     \x20               the engine must report DeviceLost once and release what was shown",
)];

fn usage() -> String {
    let mut text = String::from(
        "drm-harness: drive composition-drm on a DRM output with two hardware planes\n\
         \n\
         Usage: drm-harness [OPTIONS] <SCENARIO>\n\
         \x20      drm-harness lessor [LESSOR OPTIONS]\n\
         \x20      drm-harness producer --device <PATH> --layer <N>\n\
         \n\
         Scenarios:\n",
    );
    for (name, _, help) in SCENARIOS.iter().chain(LEASE_SCENARIOS.iter()) {
        text.push_str(&format!("  {name:<14}{help}\n"));
    }
    text.push_str(&format!(
        "  {:<14}all of the above in this order (revoke only with --source kernel-lease)\n\
         \n\
         Options:\n\
         \x20 --source <SOURCE>     where the DRM descriptor comes from [default: master]:\n\
         \x20                         master        the device node, as DRM master\n\
         \x20                         kernel-lease  this process opens the device as master and\n\
         \x20                                       leases one output (DRM_IOCTL_MODE_CREATE_LEASE);\n\
         \x20                                       the engine gets the lease descriptor only\n\
         \x20                         dlm           a lease from AGL drm-lease-manager's socket\n\
         \x20                                       (or from `drm-harness lessor`), see --lease\n\
         \x20 --device <PATH>       DRM device node [default: /dev/dri/card0]; the lessor's with\n\
         \x20                       kernel-lease, the producers' with --producers external\n\
         \x20 --connector <NAME>    connector to drive, name or prefix such as LVDS or HDMI-A-1\n\
         \x20                       [default: the first connected one]; with kernel-lease, the\n\
         \x20                       connector to lease\n\
         \x20 --planes <IDS>        kernel-lease: overlay plane ids to lease, comma-separated\n\
         \x20                       [default: every overlay usable on the connector's CRTC]\n\
         \x20 --lease <NAME>        dlm: lease name, the socket <dir>/<NAME>\n\
         \x20 --dlm-dir <DIR>       dlm: socket directory [default: $DLM_RUNTIME_PATH, else\n\
         \x20                       {}]\n\
         \x20 --producers <MODE>    who allocates the buffers [default: local]:\n\
         \x20                         local     the harness, dumb buffers on its own descriptor\n\
         \x20                         external  one producer process per layer (this binary,\n\
         \x20                                   `producer`): opens --device without DRM master,\n\
         \x20                                   allocates and draws its own buffers, passes them as\n\
         \x20                                   DMA-BUF descriptors; redraws only after the harness\n\
         \x20                                   has passed on the engine's release\n\
         \x20 --profile <PROFILE>   driver profile: generic or imx95 [default: generic]\n\
         \x20 --seconds <N>         how long to hold each picture for a camera, and the screen after the\n\
         \x20                       revocation in revoke [default: 5, at most 86400]\n\
         \x20 --frames <N>          frames for move [default: {MOVE_FRAMES}], rotate [default: {ROTATE_FRAMES}]\n\
         \x20                       and revoke, before the revocation [default: {REVOKE_FRAMES}]\n\
         \x20 --buffers <K>         buffers per plane in rotate and revoke, at least 3 [default: 3]\n\
         \x20 -h, --help            print this help\n\
         \n\
         Lessor: drm-harness lessor --lease <NAME> --dlm-dir <DIR> [--device <PATH>]\n\
         \x20       [--connector <NAME>] [--planes <IDS>] [--once]\n\
         \x20 A small leaser: opens the device as DRM master and serves <DIR>/<NAME> with\n\
         \x20 drm-lease-manager's client protocol. Each client that asks gets a fresh kernel\n\
         \x20 lease of the connector's output (descriptor passed with SCM_RIGHTS); the lease\n\
         \x20 is revoked when the client releases it or disconnects. --once: exit after the\n\
         \x20 first client. Use it with --source dlm --lease <NAME> --dlm-dir <DIR>.\n\
         \n\
         Producer: started by the harness with --producers external, one per layer, its\n\
         socket on standard input; not meant to be run by hand.\n\
         \n\
         Output: one record per line, `kind key=value ...`; each scenario ends with\n\
         `VERDICT <scenario> PASS|FAIL <reason>`. The exit status is 0 only if every\n\
         scenario run passed.\n",
        "all",
        wire::DLM_DEFAULT_RUNTIME_PATH,
    ));
    text
}

/// What the command line asks for.
enum Command {
    Help,
    Run(Box<Options>, String),
    Lessor(lessor::LessorOptions),
    Producer { device: PathBuf, layer: u32 },
}

fn plane_list(text: &str) -> Result<Vec<u32>, String> {
    text.split(',')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.trim()
                .parse::<u32>()
                .map_err(|_| format!("--planes: not a plane id: {s}"))
        })
        .collect()
}

fn parse(args: &[String]) -> Result<Command, String> {
    let (mode, args) = match args.first().map(String::as_str) {
        Some("lessor") => ("lessor", &args[1..]),
        Some("producer") => ("producer", &args[1..]),
        _ => ("run", args),
    };
    let mut device = PathBuf::from("/dev/dri/card0");
    let mut connector = None;
    let mut profile = DriverProfile::Generic;
    let mut seconds = 5;
    let mut frames = None;
    let mut buffers = 3;
    let mut source = "master".to_string();
    let mut planes = None;
    let mut lease_name = None;
    let mut dlm_dir = None;
    let mut producers = "local".to_string();
    let mut once = false;
    let mut layer = None;
    let mut scenario = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let (key, inline) = match arg.split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k, Some(v.to_string())),
            _ => (arg.as_str(), None),
        };
        let mut value = |name: &str| {
            match inline.clone() {
                Some(text) => Ok(text),
                // The next word is the value, unless it is another option.
                None => match iter.next() {
                    Some(next) if !next.starts_with("--") => Ok(next.clone()),
                    _ => Err(format!("{name} needs a value")),
                },
            }
        };
        let number = |name: &str, text: String| {
            text.parse::<u64>()
                .map_err(|_| format!("{name}: not a number: {text}"))
        };
        match key {
            "-h" | "--help" => return Ok(Command::Help),
            "--device" => device = PathBuf::from(value(key)?),
            "--connector" => connector = Some(value(key)?),
            "--planes" => planes = Some(plane_list(&value(key)?)?),
            "--lease" => lease_name = Some(value(key)?),
            "--dlm-dir" => dlm_dir = Some(PathBuf::from(value(key)?)),
            "--once" if mode == "lessor" => once = true,
            "--layer" if mode == "producer" => {
                let n = number(key, value(key)?)?;
                layer = Some(u32::try_from(n).map_err(|_| "--layer: too large".to_string())?);
            }
            "--source" => source = value(key)?,
            "--producers" => producers = value(key)?,
            "--profile" => {
                profile = match value(key)?.as_str() {
                    "generic" => DriverProfile::Generic,
                    "imx95" => DriverProfile::Imx95Dpu,
                    other => return Err(format!("--profile: unknown profile {other}")),
                }
            }
            "--seconds" => {
                seconds = number(key, value(key)?)?;
                if seconds > stop::MAX_SECONDS {
                    return Err(format!("--seconds: at most {}", stop::MAX_SECONDS));
                }
            }
            "--frames" => {
                let n = number(key, value(key)?)?;
                frames = Some(u32::try_from(n).map_err(|_| "--frames: too large".to_string())?);
            }
            "--buffers" => {
                let n = number(key, value(key)?)?;
                if !(3..=16).contains(&n) {
                    return Err("--buffers: must be between 3 and 16".into());
                }
                buffers = n as u32;
            }
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => {
                if mode != "run" {
                    return Err(format!("{mode}: unexpected argument {other}"));
                }
                if scenario.replace(other.to_string()).is_some() {
                    return Err("give one scenario".into());
                }
            }
        }
    }
    let request = LeaseRequest {
        device: device.clone(),
        connector: connector.clone(),
        overlays: planes.clone(),
    };
    match mode {
        "producer" => {
            let layer = layer.ok_or("producer: --layer is required")?;
            return Ok(Command::Producer { device, layer });
        }
        "lessor" => {
            let name = lease_name.ok_or("lessor: --lease is required")?;
            let dir = dlm_dir.ok_or("lessor: --dlm-dir is required")?;
            return Ok(Command::Lessor(lessor::LessorOptions {
                request,
                socket: wire::dlm_socket_path(Some(&dir), &name),
                once,
            }));
        }
        _ => {}
    }
    let source = match source.as_str() {
        "master" => Source::Master {
            device: device.clone(),
        },
        "kernel-lease" => Source::KernelLease(request),
        "dlm" => {
            let name = lease_name.ok_or("--source dlm needs --lease <NAME>")?;
            Source::Dlm {
                socket: wire::dlm_socket_path(dlm_dir.as_deref(), &name),
            }
        }
        other => return Err(format!("--source: unknown source {other}")),
    };
    if planes.is_some() && !matches!(source, Source::KernelLease(_)) {
        return Err("--planes is for --source kernel-lease".into());
    }
    let producers = match producers.as_str() {
        "local" => ProducerMode::Local,
        "external" => ProducerMode::External { device },
        other => return Err(format!("--producers: unknown mode {other}")),
    };
    let scenario = scenario.ok_or("no scenario given")?;
    let known = SCENARIOS
        .iter()
        .chain(LEASE_SCENARIOS.iter())
        .any(|(name, _, _)| *name == scenario);
    if scenario != "all" && !known {
        return Err(format!("unknown scenario {scenario}"));
    }
    Ok(Command::Run(
        Box::new(Options {
            source,
            connector,
            profile,
            seconds,
            frames,
            buffers,
            producers,
        }),
        scenario,
    ))
}

/// What a stop (SIGTERM, SIGINT) adds to the reason of a scenario that had
/// made its checks and was ending its hold.
const STOPPED_NOTE: &str = "stopped by SIGTERM or SIGINT during the hold, buffers released";

/// The reason of a scenario that a stop cut off before its checks were made.
const CUT_OFF_REASON: &str = "stopped by SIGTERM or SIGINT before the checks were made";

/// How a scenario ended.
#[derive(Debug, PartialEq)]
enum End {
    /// It ran to its checks: whether they passed, why, and whether a stop
    /// ended its hold.
    Done(bool, String, bool),
    /// It stopped with an error.
    Failed(String),
    /// A stop ended it before its checks were made.
    CutOff,
}

/// Classifies a scenario's result. Only the stop's own error is a cut-off;
/// any other error stays what it was, even if a signal came meanwhile.
fn end_of(result: Result<Outcome, String>) -> End {
    match result {
        Ok(outcome) => End::Done(outcome.pass, outcome.reason, outcome.stopped),
        Err(error) if error == stop::STOPPED => End::CutOff,
        Err(error) => End::Failed(error),
    }
}

fn run(options: &Options, scenario: &str, args: &[String]) -> ExitCode {
    Record::new("harness")
        .field("version", env!("CARGO_PKG_VERSION"))
        .field("scenario", scenario)
        .field("args", args.join(" "))
        .emit();
    let lease_scenarios: &[(&str, Scenario, &str)] =
        if matches!(options.source, Source::KernelLease(_)) || scenario != "all" {
            &LEASE_SCENARIOS
        } else {
            &[]
        };
    let mut passed = 0;
    let mut failed = 0;
    for (name, run, _) in SCENARIOS.iter().chain(lease_scenarios.iter()) {
        if scenario != "all" && scenario != *name {
            continue;
        }
        let problems_before = producer::PROBLEMS.load(Ordering::Relaxed);
        let usage = Usage::now();
        let mut stopped = false;
        let (mut pass, mut reason) = match end_of(run(options)) {
            End::Done(pass, reason, was_stopped) => {
                stopped = was_stopped;
                if was_stopped {
                    (pass, format!("{reason}; {STOPPED_NOTE}"))
                } else {
                    (pass, reason)
                }
            }
            End::Failed(error) => {
                Record::new("error")
                    .field("scenario", name)
                    .field("message", &error)
                    .emit();
                (false, error)
            }
            End::CutOff => {
                // The session ended with the scenario, so its buffers are
                // released, but the checks were not made: that is no PASS.
                stopped = true;
                scenarios::print_resources(name, &usage);
                (false, CUT_OFF_REASON.to_string())
            }
        };
        // The session, and with it the producers, ended with the scenario.
        let problems = producer::PROBLEMS.load(Ordering::Relaxed) - problems_before;
        if problems > 0 {
            pass = false;
            reason = format!("{reason}; producers reported {problems} problems");
        }
        println!("{}", verdict_line(name, pass, &reason));
        if pass {
            passed += 1;
        } else {
            failed += 1;
        }
        if stopped {
            // No further scenario after a stop.
            break;
        }
    }
    if scenario == "all" {
        Record::new("SUMMARY")
            .field("passed", passed)
            .field("failed", failed)
            .emit();
    }
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match parse(&args) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("drm-harness: {message}\n");
            eprint!("{}", usage());
            return ExitCode::from(2);
        }
    };
    match command {
        Command::Help => {
            print!("{}", usage());
            ExitCode::SUCCESS
        }
        Command::Run(options, scenario) => {
            if let Err(error) = stop::install() {
                eprintln!("drm-harness: cannot handle SIGTERM: {error}");
            }
            run(&options, &scenario, &args)
        }
        Command::Lessor(options) => match lessor::run_lessor(&options) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("drm-harness lessor: {error}");
                ExitCode::FAILURE
            }
        },
        Command::Producer { device, layer } => match producer::run_producer(&device, layer) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("drm-harness producer {layer}: {error}");
                ExitCode::FAILURE
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(String::from).collect()
    }

    fn options(text: &str) -> (Options, String) {
        match parse(&args(text)).unwrap() {
            Command::Run(options, scenario) => (*options, scenario),
            _ => panic!("not a run: {text}"),
        }
    }

    #[test]
    fn sources_parse() {
        let (o, scenario) = options("two-planes");
        assert_eq!(scenario, "two-planes");
        assert!(matches!(o.source, Source::Master { .. }));
        assert!(matches!(o.producers, ProducerMode::Local));

        let (o, _) = options("--source kernel-lease --connector LVDS --planes 31,32 revoke");
        match o.source {
            Source::KernelLease(r) => {
                assert_eq!(r.overlays, Some(vec![31, 32]));
                assert_eq!(r.connector.as_deref(), Some("LVDS"));
            }
            other => panic!("{other:?}"),
        }

        let (o, _) = options("--source=dlm --lease card0-LVDS-1 --dlm-dir /run/dlm rotate");
        match o.source {
            Source::Dlm { socket } => assert_eq!(socket, PathBuf::from("/run/dlm/card0-LVDS-1")),
            other => panic!("{other:?}"),
        }

        let (o, _) = options("--producers external --device /dev/dri/card1 all");
        match o.producers {
            ProducerMode::External { device } => {
                assert_eq!(device, PathBuf::from("/dev/dri/card1"))
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bad_combinations_are_refused() {
        for bad in [
            "--source dlm rotate",
            "--source nope rotate",
            "--planes 31 rotate",
            "--producers remote rotate",
            "--source kernel-lease --planes 31,x revoke",
            "frobnicate",
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn an_option_does_not_swallow_the_next_option() {
        for bad in [
            "--connector --seconds 5 revoke",
            "--seconds --frames 3 revoke",
            "revoke --connector",
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad}");
        }
        let (o, _) = options("--connector=HDMI-A-1 --seconds 7 two-planes");
        assert_eq!(o.connector.as_deref(), Some("HDMI-A-1"));
        assert_eq!(o.seconds, 7);
    }

    #[test]
    fn absurd_seconds_are_refused_not_a_panic() {
        assert!(parse(&args("--seconds 18446744073709551615 revoke")).is_err());
        assert!(parse(&args("--seconds 86400 revoke")).is_ok());
    }

    #[test]
    fn subcommands_parse() {
        match parse(&args("producer --device /dev/dri/card0 --layer 2")).unwrap() {
            Command::Producer { layer, .. } => assert_eq!(layer, 2),
            _ => panic!(),
        }
        assert!(parse(&args("producer --device /dev/dri/card0")).is_err());
        match parse(&args(
            "lessor --lease l --dlm-dir /run/x --connector LVDS --once",
        ))
        .unwrap()
        {
            Command::Lessor(o) => {
                assert_eq!(o.socket, PathBuf::from("/run/x/l"));
                assert!(o.once);
            }
            _ => panic!(),
        }
        assert!(parse(&args("lessor --lease l")).is_err());
        assert!(parse(&args("lessor --dlm-dir /x --lease l rotate")).is_err());
    }

    #[test]
    fn a_stop_never_turns_a_failure_into_a_pass() {
        // A scenario that found a failure and was then stopped in its hold.
        let failed = Outcome {
            pass: false,
            reason: "no event".into(),
            stopped: true,
        };
        assert_eq!(
            end_of(Ok(failed)),
            End::Done(false, "no event".into(), true)
        );
        // A stop before the checks is its own end, never a pass.
        assert_eq!(end_of(Err(stop::STOPPED.into())), End::CutOff);
        // An error that raced with the signal keeps its text.
        assert_eq!(
            end_of(Err("poll: EINTR".into())),
            End::Failed("poll: EINTR".into())
        );
        let line = verdict_line("revoke", false, CUT_OFF_REASON);
        assert!(line.starts_with("VERDICT revoke FAIL stopped by SIGTERM"));
    }

    #[test]
    fn help_lists_sources_producers_and_revoke() {
        let text = usage();
        for needle in [
            "--source",
            "kernel-lease",
            "dlm",
            "--producers",
            "external",
            "revoke",
            "lessor",
        ] {
            assert!(text.contains(needle), "{needle}");
        }
    }
}
