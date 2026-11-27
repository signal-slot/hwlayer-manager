//! `drm-harness`: a minimal producer harness for composition-drm.
//!
//! Drives the engine on one output of a real DRM device, as DRM master, and
//! prints one-line `key=value` records ending in
//! `VERDICT <scenario> PASS|FAIL <reason>`. See the README.

#![forbid(unsafe_code)]

mod draw;
mod record;
mod scenarios;
mod session;
mod source;
mod stats;

use std::path::PathBuf;
use std::process::ExitCode;

use composition_drm::DriverProfile;

use crate::record::{verdict_line, Record};
use crate::scenarios::{Outcome, MOVE_FRAMES, ROTATE_FRAMES};
use crate::session::Options;
use crate::source::Source;

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

fn usage() -> String {
    let mut text = String::from(
        "drm-harness: drive composition-drm on a DRM output with two hardware planes\n\
         \n\
         Usage: drm-harness [OPTIONS] <SCENARIO>\n\
         \n\
         Scenarios:\n",
    );
    for (name, _, help) in SCENARIOS {
        text.push_str(&format!("  {name:<14}{help}\n"));
    }
    text.push_str(&format!(
        "  {:<14}all of the above, in this order\n\
         \n\
         Options:\n\
         \x20 --device <PATH>       DRM device to open as master [default: /dev/dri/card0]\n\
         \x20 --connector <NAME>    connector to drive, name or prefix such as LVDS or HDMI-A-1\n\
         \x20                       [default: the first connected one]\n\
         \x20 --profile <PROFILE>   driver profile: generic or imx95 [default: generic]\n\
         \x20 --seconds <N>         how long to hold each picture for a camera [default: 5]\n\
         \x20 --frames <N>          frames for move [default: {MOVE_FRAMES}] and rotate [default: {ROTATE_FRAMES}]\n\
         \x20 --buffers <K>         buffers per plane in rotate, at least 3 [default: 3]\n\
         \x20 -h, --help            print this help\n\
         \n\
         Output: one record per line, `kind key=value ...`; each scenario ends with\n\
         `VERDICT <scenario> PASS|FAIL <reason>`. The exit status is 0 only if every\n\
         scenario run passed.\n",
        "all"
    ));
    text
}

fn parse(args: &[String]) -> Result<Option<(Options, String)>, String> {
    let mut device = PathBuf::from("/dev/dri/card0");
    let mut connector = None;
    let mut profile = DriverProfile::Generic;
    let mut seconds = 5;
    let mut frames = None;
    let mut buffers = 3;
    let mut scenario = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let (key, inline) = match arg.split_once('=') {
            Some((k, v)) if k.starts_with("--") => (k, Some(v.to_string())),
            _ => (arg.as_str(), None),
        };
        let mut value = |name: &str| {
            inline
                .clone()
                .or_else(|| iter.next().cloned())
                .ok_or_else(|| format!("{name} needs a value"))
        };
        let number = |name: &str, text: String| {
            text.parse::<u64>()
                .map_err(|_| format!("{name}: not a number: {text}"))
        };
        match key {
            "-h" | "--help" => return Ok(None),
            "--device" => device = PathBuf::from(value(key)?),
            "--connector" => connector = Some(value(key)?),
            "--profile" => {
                profile = match value(key)?.as_str() {
                    "generic" => DriverProfile::Generic,
                    "imx95" => DriverProfile::Imx95Dpu,
                    other => return Err(format!("--profile: unknown profile {other}")),
                }
            }
            "--seconds" => seconds = number(key, value(key)?)?,
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
                if scenario.replace(other.to_string()).is_some() {
                    return Err("give one scenario".into());
                }
            }
        }
    }
    let scenario = scenario.ok_or("no scenario given")?;
    if scenario != "all" && !SCENARIOS.iter().any(|(name, _, _)| *name == scenario) {
        return Err(format!("unknown scenario {scenario}"));
    }
    Ok(Some((
        Options {
            source: Source::Master { device },
            connector,
            profile,
            seconds,
            frames,
            buffers,
        },
        scenario,
    )))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (options, scenario) = match parse(&args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => {
            print!("{}", usage());
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("drm-harness: {message}\n");
            eprint!("{}", usage());
            return ExitCode::from(2);
        }
    };
    Record::new("harness")
        .field("version", env!("CARGO_PKG_VERSION"))
        .field("scenario", &scenario)
        .field("args", args.join(" "))
        .emit();
    let mut passed = 0;
    let mut failed = 0;
    for (name, run, _) in SCENARIOS {
        if scenario != "all" && scenario != name {
            continue;
        }
        let (pass, reason) = match run(&options) {
            Ok(outcome) => (outcome.pass, outcome.reason),
            Err(error) => {
                Record::new("error")
                    .field("scenario", name)
                    .field("message", &error)
                    .emit();
                (false, error)
            }
        };
        println!("{}", verdict_line(name, pass, &reason));
        if pass {
            passed += 1;
        } else {
            failed += 1;
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
