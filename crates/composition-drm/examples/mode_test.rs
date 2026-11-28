//! Mode test: a `TEST_ONLY` modeset of each mode of a connector, to learn
//! which modes the driver accepts without ever changing the display.
//!
//! ```text
//! cargo run -p composition-drm --example mode_test -- [options]
//!
//!   --device PATH      DRM device (default /dev/dri/card0); a bare argument works too
//!   --connector ID     connector to test (default: every connected one)
//!   --mode NAME@HZ     test only the modes with this name and vertical refresh
//!                      (`1920x1080@60`; HZ is the rounded vrefresh)
//!   --min-clock KHZ    test only the modes whose pixel clock is at least KHZ
//! ```
//!
//! The program opens the device, becomes DRM master (or exits with status 2
//! saying it could not; it never stops other processes) and lists every
//! mode of each chosen connector. For each selected mode it builds an atomic
//! request with the connector's CRTC (the one it runs on, else the first
//! possible one), a new `MODE_ID` blob (destroyed afterwards), `ACTIVE=1`
//! and the CRTC's primary plane on a dumb XR24 buffer of the mode size,
//! and submits it with `ATOMIC_TEST_ONLY | ALLOW_MODESET`. Nothing is ever
//! committed. The `--mode` and `--min-clock` filters combine (both must hold).
//!
//! # Output
//!
//! ```text
//! DATE / KERNEL / DEVICE lines
//! CONNECTOR 33 crtc 71 modes 12
//! LISTED 1920x1080 clock=148500 htotal=2200 vtotal=1125 vrefresh=60 flags=0x5 preferred
//! mode 1920x1080@60 clock=148500 test=ok
//! mode 3840x2160@60 clock=594000 test=refused errno=22
//! mode_test: tested=2 ok=1 highest_ok_clock=148500
//! ```
//!
//! `highest_ok_clock` is the largest pixel clock (kHz) of a mode that passed,
//! 0 if none did. The exit status is 0 when the run finished (refusals are
//! the answer), 2 on a usage error or when it could not run.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use composition_drm::Card;
use drm::buffer::DrmFourcc;
use drm::control::atomic::AtomicModeReq;
use drm::control::{
    connector, crtc, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice,
    Mode, ModeTypeFlags, PlaneType, ResourceHandle,
};
use drm::{ClientCapability, Device as BasicDevice};

const USAGE: &str =
    "usage: mode_test [--device PATH] [--connector ID] [--mode NAME@HZ] [--min-clock KHZ]";

#[derive(Debug, PartialEq, Eq)]
struct Options {
    device: String,
    connector: Option<u32>,
    mode: Option<(String, u32)>,
    min_clock: Option<u32>,
}

/// What a mode looks like to the selection and the output, apart from the
/// kernel structure.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ModeRow {
    name: String,
    width: u32,
    height: u32,
    clock: u32,
    htotal: u32,
    vtotal: u32,
    vrefresh: u32,
    flags: u32,
    preferred: bool,
}

fn parse_mode_spec(text: &str) -> Result<(String, u32), String> {
    let (name, hz) = text
        .rsplit_once('@')
        .ok_or_else(|| format!("--mode wants NAME@HZ, got {text:?}"))?;
    if name.is_empty() {
        return Err(format!("--mode has no name: {text:?}"));
    }
    let hz = hz
        .parse::<u32>()
        .map_err(|_| format!("--mode refresh is not a number: {text:?}"))?;
    Ok((name.to_string(), hz))
}

fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
    let mut options = Options {
        device: "/dev/dri/card0".into(),
        connector: None,
        mode: None,
        min_clock: None,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |what: &str| args.next().ok_or_else(|| format!("{what} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => return Err(String::new()),
            "--device" => options.device = value("--device")?,
            "--connector" => {
                let v = value("--connector")?;
                options.connector = Some(v.parse().map_err(|_| format!("bad connector id {v:?}"))?);
            }
            "--mode" => options.mode = Some(parse_mode_spec(&value("--mode")?)?),
            "--min-clock" => {
                let v = value("--min-clock")?;
                options.min_clock = Some(v.parse().map_err(|_| format!("bad clock {v:?}"))?);
            }
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => options.device = other.to_string(),
        }
    }
    Ok(options)
}

/// Whether the options select this mode.
fn selected(options: &Options, row: &ModeRow) -> bool {
    if let Some((name, hz)) = &options.mode {
        if row.name != *name || row.vrefresh != *hz {
            return false;
        }
    }
    options.min_clock.is_none_or(|min| row.clock >= min)
}

fn errno_name(errno: i32) -> String {
    let name = match errno {
        1 => "EPERM",
        2 => "ENOENT",
        5 => "EIO",
        11 => "EAGAIN",
        12 => "ENOMEM",
        13 => "EACCES",
        16 => "EBUSY",
        19 => "ENODEV",
        22 => "EINVAL",
        28 => "ENOSPC",
        34 => "ERANGE",
        95 => "EOPNOTSUPP",
        _ => return format!("errno{errno}"),
    };
    name.into()
}

fn listed_line(row: &ModeRow) -> String {
    format!(
        "LISTED {} clock={} htotal={} vtotal={} vrefresh={} flags={:#x}{}",
        row.name,
        row.clock,
        row.htotal,
        row.vtotal,
        row.vrefresh,
        row.flags,
        if row.preferred { " preferred" } else { "" }
    )
}

/// `mode WxH@R clock=K test=ok` or `... test=refused errno=E`.
fn mode_line(row: &ModeRow, result: Result<(), i32>) -> String {
    let head = format!(
        "mode {}x{}@{} clock={}",
        row.width, row.height, row.vrefresh, row.clock
    );
    match result {
        Ok(()) => format!("{head} test=ok"),
        Err(errno) => format!("{head} test=refused errno={errno}"),
    }
}

fn summary_line(tested: usize, ok: usize, highest_ok_clock: u32) -> String {
    format!("mode_test: tested={tested} ok={ok} highest_ok_clock={highest_ok_clock}")
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

type Props = HashMap<String, (property::Handle, u64)>;

fn named_properties<H: ResourceHandle>(card: &Card, handle: H) -> io::Result<Props> {
    let set = card.get_properties(handle)?;
    let mut named = HashMap::new();
    for (prop, value) in set.iter() {
        let info = card.get_property(*prop)?;
        named.insert(info.name().to_string_lossy().into_owned(), (*prop, *value));
    }
    Ok(named)
}

fn prop(props: &Props, name: &str) -> Result<property::Handle, String> {
    props
        .get(name)
        .map(|(handle, _)| *handle)
        .ok_or_else(|| format!("no property {name}"))
}

fn fail(what: &str) -> impl Fn(io::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

fn errno_of(error: &io::Error) -> i32 {
    error.raw_os_error().unwrap_or(-1)
}

fn row_of(mode: &Mode) -> ModeRow {
    let (w, h) = mode.size();
    let (_, _, htotal) = mode.hsync();
    let (_, _, vtotal) = mode.vsync();
    ModeRow {
        name: mode.name().to_string_lossy().into_owned(),
        width: u32::from(w),
        height: u32::from(h),
        clock: mode.clock(),
        htotal: u32::from(htotal),
        vtotal: u32::from(vtotal),
        vrefresh: mode.vrefresh(),
        flags: mode.flags().bits(),
        preferred: mode.mode_type().contains(ModeTypeFlags::PREFERRED),
    }
}

struct Target {
    connector: connector::Handle,
    connector_props: Props,
    crtc: crtc::Handle,
    crtc_props: Props,
    plane: plane::Handle,
    plane_props: Props,
}

/// The CRTC of a connector and a primary plane that can drive it.
fn find_target(
    card: &Card,
    resources: &drm::control::ResourceHandles,
    handle: connector::Handle,
    info: &connector::Info,
) -> Result<Target, String> {
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
    let mut planes = card.plane_handles().map_err(fail("planes"))?;
    planes.sort_by_key(|h| u32::from(*h));
    for plane in planes {
        let pinfo = card.get_plane(plane).map_err(fail("plane"))?;
        if !resources
            .filter_crtcs(pinfo.possible_crtcs())
            .contains(&crtc)
        {
            continue;
        }
        let props = named_properties(card, plane).map_err(fail("plane props"))?;
        if props.get("type").map(|p| p.1 as u32) == Some(PlaneType::Primary as u32) {
            return Ok(Target {
                connector: handle,
                connector_props: named_properties(card, handle).map_err(fail("connector props"))?,
                crtc,
                crtc_props: named_properties(card, crtc).map_err(fail("crtc props"))?,
                plane,
                plane_props: props,
            });
        }
    }
    Err("no primary plane for the CRTC".into())
}

/// One TEST_ONLY modeset of `mode`; the errno of a refusal is the answer.
fn test_mode(card: &Card, target: &Target, mode: &Mode) -> Result<Result<(), i32>, String> {
    let (w, h) = mode.size();
    let size = (u32::from(w), u32::from(h));
    let dumb = card
        .create_dumb_buffer(size, DrmFourcc::Xrgb8888, 32)
        .map_err(|e| {
            format!(
                "dumb buffer {}x{}: {}",
                size.0,
                size.1,
                errno_name(errno_of(&e))
            )
        })?;
    let fb = match card.add_framebuffer(&dumb, 24, 32) {
        Ok(fb) => fb,
        Err(e) => {
            let _ = card.destroy_dumb_buffer(dumb);
            return Err(format!("ADDFB: {}", errno_name(errno_of(&e))));
        }
    };
    let blob = match card.create_property_blob(mode) {
        Ok(property::Value::Blob(id)) => Ok(id),
        Ok(_) => Err("mode blob".to_string()),
        Err(e) => Err(format!("mode blob: {e}")),
    };
    let result = blob
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|blob| request(card, target, *blob, fb, size));
    if let Ok(blob) = blob {
        let _ = card.destroy_property_blob(blob);
    }
    let _ = card.destroy_framebuffer(fb);
    let _ = card.destroy_dumb_buffer(dumb);
    result
}

fn request(
    card: &Card,
    target: &Target,
    blob: u64,
    fb: framebuffer::Handle,
    size: (u32, u32),
) -> Result<Result<(), i32>, String> {
    let mut req = AtomicModeReq::new();
    req.add_property(
        target.connector,
        prop(&target.connector_props, "CRTC_ID")?,
        property::Value::CRTC(Some(target.crtc)),
    );
    req.add_property(
        target.crtc,
        prop(&target.crtc_props, "MODE_ID")?,
        property::Value::Blob(blob),
    );
    req.add_property(
        target.crtc,
        prop(&target.crtc_props, "ACTIVE")?,
        property::Value::Boolean(true),
    );
    let p = |name: &str| prop(&target.plane_props, name);
    let h = target.plane;
    req.add_property(h, p("FB_ID")?, property::Value::Framebuffer(Some(fb)));
    req.add_property(h, p("CRTC_ID")?, property::Value::CRTC(Some(target.crtc)));
    let source = [0, 0, u64::from(size.0) << 16, u64::from(size.1) << 16];
    for (name, v) in ["SRC_X", "SRC_Y", "SRC_W", "SRC_H"].into_iter().zip(source) {
        req.add_property(h, p(name)?, property::Value::UnsignedRange(v));
    }
    req.add_property(h, p("CRTC_X")?, property::Value::SignedRange(0));
    req.add_property(h, p("CRTC_Y")?, property::Value::SignedRange(0));
    req.add_property(
        h,
        p("CRTC_W")?,
        property::Value::UnsignedRange(size.0.into()),
    );
    req.add_property(
        h,
        p("CRTC_H")?,
        property::Value::UnsignedRange(size.1.into()),
    );
    let flags = AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::ALLOW_MODESET;
    Ok(card.atomic_commit(flags, req).map_err(|e| errno_of(&e)))
}

fn run(options: &Options) -> Result<(), String> {
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
    card.set_client_capability(ClientCapability::UniversalPlanes, true)
        .map_err(fail("universal planes"))?;
    card.set_client_capability(ClientCapability::Atomic, true)
        .map_err(fail("atomic"))?;
    card.acquire_master_lock().map_err(|e| {
        format!(
            "DRM master is unavailable: {e}. Another process (a desktop or compositor) probably \
             holds it; stop it for the run. This test never stops other processes"
        )
    })?;
    let resources = card.resource_handles().map_err(fail("resources"))?;
    let (mut tested, mut ok, mut highest) = (0usize, 0usize, 0u32);
    for handle in resources.connectors() {
        if options.connector.is_some_and(|id| id != u32::from(*handle)) {
            continue;
        }
        let info = card
            .get_connector(*handle, false)
            .map_err(fail("connector"))?;
        if info.state() != connector::State::Connected {
            continue;
        }
        let target = find_target(&card, &resources, *handle, &info)?;
        println!(
            "CONNECTOR {} crtc {} modes {}",
            u32::from(*handle),
            u32::from(target.crtc),
            info.modes().len()
        );
        for mode in info.modes() {
            println!("{}", listed_line(&row_of(mode)));
        }
        for mode in info.modes() {
            let row = row_of(mode);
            if !selected(options, &row) {
                continue;
            }
            let result = test_mode(&card, &target, mode)?;
            println!("{}", mode_line(&row, result));
            tested += 1;
            if result.is_ok() {
                ok += 1;
                highest = highest.max(row.clock);
            }
        }
    }
    println!("{}", summary_line(tested, ok, highest));
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
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("mode_test: {message}");
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

    fn row(name: &str, clock: u32, hz: u32) -> ModeRow {
        ModeRow {
            name: name.into(),
            width: 1920,
            height: 1080,
            clock,
            htotal: 2200,
            vtotal: 1125,
            vrefresh: hz,
            flags: 5,
            preferred: true,
        }
    }

    #[test]
    fn defaults_and_options() {
        let o = args("").unwrap();
        assert_eq!(o.device, "/dev/dri/card0");
        assert_eq!((o.connector, o.mode, o.min_clock), (None, None, None));
        let o = args("--device /dev/dri/card1 --connector 33 --mode 1920x1080@60 --min-clock 100")
            .unwrap();
        assert_eq!(o.device, "/dev/dri/card1");
        assert_eq!(o.connector, Some(33));
        assert_eq!(o.mode, Some(("1920x1080".into(), 60)));
        assert_eq!(o.min_clock, Some(100));
        assert_eq!(args("/dev/dri/card2").unwrap().device, "/dev/dri/card2");
    }

    #[test]
    fn rejects_bad_input() {
        assert!(args("--mode 1920x1080").is_err());
        assert!(args("--mode @60").is_err());
        assert!(args("--mode x@fast").is_err());
        assert!(args("--min-clock").is_err());
        assert!(args("--min-clock abc").is_err());
        assert!(args("--connector x").is_err());
        assert!(args("--bogus").is_err());
        assert_eq!(args("--help"), Err(String::new()));
    }

    #[test]
    fn selection_filters_combine() {
        let r = row("1920x1080", 148_500, 60);
        assert!(selected(&args("").unwrap(), &r));
        assert!(selected(&args("--mode 1920x1080@60").unwrap(), &r));
        assert!(!selected(&args("--mode 1920x1080@50").unwrap(), &r));
        assert!(!selected(&args("--mode 1280x720@60").unwrap(), &r));
        assert!(selected(&args("--min-clock 148500").unwrap(), &r));
        assert!(!selected(&args("--min-clock 148501").unwrap(), &r));
        assert!(!selected(
            &args("--mode 1920x1080@60 --min-clock 200000").unwrap(),
            &r
        ));
    }

    #[test]
    fn lines() {
        let r = row("1920x1080", 148_500, 60);
        assert_eq!(
            mode_line(&r, Ok(())),
            "mode 1920x1080@60 clock=148500 test=ok"
        );
        assert_eq!(
            mode_line(&r, Err(22)),
            "mode 1920x1080@60 clock=148500 test=refused errno=22"
        );
        assert_eq!(
            listed_line(&r),
            "LISTED 1920x1080 clock=148500 htotal=2200 vtotal=1125 vrefresh=60 flags=0x5 preferred"
        );
        assert_eq!(
            summary_line(2, 1, 148_500),
            "mode_test: tested=2 ok=1 highest_ok_clock=148500"
        );
        assert_eq!(errno_name(22), "EINVAL");
        assert_eq!(errno_name(999), "errno999");
    }

    #[test]
    fn utc_shape() {
        let t = utc_now();
        assert_eq!(t.len(), 20);
        assert!(t.ends_with('Z'));
    }
}
