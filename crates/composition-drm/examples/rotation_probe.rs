//! Rotation probe: which `rotation` values does each plane accept?
//!
//! ```text
//! cargo run -p composition-drm --example rotation_probe -- [/dev/dri/cardN] [--device PATH]
//!                                                          [--plane ID]
//! ```
//!
//! The tool opens the device, becomes DRM master (or exits with status 2
//! saying it could not; it never stops other processes) and looks at every
//! plane that has a `rotation` property (or only the plane given with
//! `--plane`). Each plane is attached to an **active** CRTC it can reach (one
//! with a mode set; the tool never sets a mode) with a small square dumb
//! framebuffer, and each value of the table below is tried in its own
//! `DRM_MODE_ATOMIC_TEST_ONLY` commit. **The tool never makes a real commit.**
//! Source and destination are always the same square (64x64 at 0,0), so 90 and
//! 270 are not refused only because of a non-square rectangle. A plane that
//! needs a full-screen buffer (some primary planes) may refuse every value;
//! read that as "the plane constrains its size", not as "no rotation".
//!
//! # Values
//!
//! The DRM UAPI bits are `ROTATE_0`=0x01, `ROTATE_90`=0x02, `ROTATE_180`=0x04,
//! `ROTATE_270`=0x08, `REFLECT_X`=0x10, `REFLECT_Y`=0x20. The tried values are
//! rotate-0, rotate-90, rotate-180, rotate-270, reflect-x and reflect-y (each
//! with rotate-0), rotate-90|reflect-x and rotate-0|reflect-x|reflect-y.
//!
//! # Output
//!
//! ```text
//! DATE / KERNEL / DEVICE / DRIVER / CRTC / PLANE lines
//! ROT plane=31 type=Overlay value=rotate-0 bits=0x01 result=ok
//! ROT plane=31 type=Overlay value=rotate-90 bits=0x02 result=errno=22(EINVAL)
//! ROT plane=31 type=Overlay result=skipped reason=...
//! SUMMARY rotation_probe: planes=3 tried=24 ok=18 refused=6 skipped=0
//! VERDICT rotation_probe: DONE
//! ```
//!
//! The exit status is 0 after `VERDICT ... DONE` and 2 for a usage error, a
//! device that cannot be used or a missing DRM master.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use composition_drm::Card;
use drm::buffer::DrmFourcc;
use drm::control::atomic::AtomicModeReq;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    crtc, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice, PlaneType,
    ResourceHandle,
};
use drm::{ClientCapability, Device as BasicDevice};

const USAGE: &str = "usage: rotation_probe [/dev/dri/cardN] [--device PATH] [--plane ID]";

/// Side of the square test buffer, source and destination.
const SIDE: u32 = 64;

const ROTATE_0: u64 = 1 << 0;
const ROTATE_90: u64 = 1 << 1;
const ROTATE_180: u64 = 1 << 2;
const ROTATE_270: u64 = 1 << 3;
const REFLECT_X: u64 = 1 << 4;
const REFLECT_Y: u64 = 1 << 5;

/// The values tried on every plane, in output order.
const VALUES: [(&str, u64); 8] = [
    ("rotate-0", ROTATE_0),
    ("rotate-90", ROTATE_90),
    ("rotate-180", ROTATE_180),
    ("rotate-270", ROTATE_270),
    ("reflect-x", ROTATE_0 | REFLECT_X),
    ("reflect-y", ROTATE_0 | REFLECT_Y),
    ("rotate-90+reflect-x", ROTATE_90 | REFLECT_X),
    ("reflect-x+reflect-y", ROTATE_0 | REFLECT_X | REFLECT_Y),
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct Options {
    device: String,
    plane: Option<u32>,
}

/// Parses the arguments after the program name. An empty error asks for the
/// usage text alone.
fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
    let mut options = Options {
        device: "/dev/dri/card0".into(),
        plane: None,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--device" => options.device = args.next().ok_or("--device needs a value")?,
            "--plane" => {
                let text = args.next().ok_or("--plane needs a value")?;
                options.plane = Some(text.parse().map_err(|_| format!("bad number {text}"))?);
            }
            "-h" | "--help" => return Err(String::new()),
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => options.device = other.to_string(),
        }
    }
    Ok(options)
}

fn errno_name(errno: i32) -> String {
    let name = match errno {
        1 => "EPERM",
        2 => "ENOENT",
        7 => "E2BIG",
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

/// `ROT plane=<id> type=<type> value=<name> bits=0x.. result=ok|errno=<n>(<name>)`.
fn rot_line(plane: u32, kind: &str, value: &str, bits: u64, result: Result<(), i32>) -> String {
    let result = match result {
        Ok(()) => "ok".to_string(),
        Err(errno) => format!("errno={errno}({})", errno_name(errno)),
    };
    format!("ROT plane={plane} type={kind} value={value} bits={bits:#04x} result={result}")
}

/// `ROT plane=<id> type=<type> result=skipped reason=<why>`.
fn skip_line(plane: u32, kind: &str, reason: &str) -> String {
    format!("ROT plane={plane} type={kind} result=skipped reason={reason}")
}

/// Counts of the run, for the SUMMARY line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Tally {
    planes: u32,
    tried: u32,
    ok: u32,
    refused: u32,
    skipped: u32,
}

fn summary_line(t: &Tally) -> String {
    format!(
        "SUMMARY rotation_probe: planes={} tried={} ok={} refused={} skipped={}",
        t.planes, t.tried, t.ok, t.refused, t.skipped
    )
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

fn prop(props: &Props, name: &str) -> io::Result<property::Handle> {
    props
        .get(name)
        .map(|(handle, _)| *handle)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no property {name}")))
}

fn fail(what: &str) -> impl Fn(io::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

/// A dumb buffer and the framebuffer made from it.
struct Fb {
    dumb: DumbBuffer,
    fb: framebuffer::Handle,
}

fn make_fb(card: &Card, format: DrmFourcc) -> Result<Fb, String> {
    let dumb = card
        .create_dumb_buffer((SIDE, SIDE), format, 32)
        .map_err(fail("dumb buffer"))?;
    let depth = if format == DrmFourcc::Xrgb8888 {
        24
    } else {
        32
    };
    match card.add_framebuffer(&dumb, depth, 32) {
        Ok(fb) => Ok(Fb { dumb, fb }),
        Err(e) => {
            let _ = card.destroy_dumb_buffer(dumb);
            Err(format!("framebuffer: {e}"))
        }
    }
}

struct PlaneInfo {
    handle: plane::Handle,
    id: u32,
    kind: &'static str,
    /// An active CRTC the plane can reach, if any.
    crtc: Option<crtc::Handle>,
    format: Option<DrmFourcc>,
    fb_id: property::Handle,
    crtc_id: property::Handle,
    src: [property::Handle; 4],
    dst: [property::Handle; 4],
    rotation: property::Handle,
}

fn open(options: &Options) -> Result<(Card, Vec<PlaneInfo>), String> {
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
    match card.get_driver() {
        Ok(info) => println!(
            "DRIVER {} version {}.{}.{}",
            info.name().to_string_lossy(),
            info.version.0,
            info.version.1,
            info.version.2
        ),
        Err(_) => println!("DRIVER unknown"),
    }
    card.set_client_capability(ClientCapability::UniversalPlanes, true)
        .map_err(fail("universal planes"))?;
    card.set_client_capability(ClientCapability::Atomic, true)
        .map_err(fail("atomic"))?;
    card.acquire_master_lock().map_err(|e| {
        format!(
            "DRM master is unavailable: {e}. Another process (a desktop or compositor) probably \
             holds it; stop it for the run. This tool never stops other processes"
        )
    })?;

    let resources = card.resource_handles().map_err(fail("resources"))?;
    let mut active = Vec::new();
    for handle in resources.crtcs() {
        let info = card.get_crtc(*handle).map_err(fail("crtc"))?;
        if info.mode().is_some() {
            println!("CRTC id {}", u32::from(*handle));
            active.push(*handle);
        }
    }
    if active.is_empty() {
        return Err("no active CRTC (a mode must already be set)".into());
    }

    let mut planes = Vec::new();
    let mut handles = card.plane_handles().map_err(fail("planes"))?;
    handles.sort_by_key(|h| u32::from(*h));
    for handle in handles {
        let id = u32::from(handle);
        if options.plane.is_some_and(|p| p != id) {
            continue;
        }
        let pinfo = card.get_plane(handle).map_err(fail("plane"))?;
        let props = named_properties(&card, handle).map_err(fail("plane props"))?;
        let Some(&(rotation, current)) = props.get("rotation") else {
            if options.plane.is_some() {
                return Err(format!("plane {id} has no rotation property"));
            }
            continue;
        };
        let kind = match props.get("type").map(|(_, v)| *v as u32) {
            Some(v) if v == PlaneType::Primary as u32 => "Primary",
            Some(v) if v == PlaneType::Cursor as u32 => "Cursor",
            _ => "Overlay",
        };
        let possible = resources.filter_crtcs(pinfo.possible_crtcs());
        let crtc = active.iter().copied().find(|c| possible.contains(c));
        let format = [DrmFourcc::Xrgb8888, DrmFourcc::Argb8888]
            .into_iter()
            .find(|f| pinfo.formats().contains(&(*f as u32)));
        let names = |p: [&str; 4]| -> io::Result<[property::Handle; 4]> {
            Ok([
                prop(&props, p[0])?,
                prop(&props, p[1])?,
                prop(&props, p[2])?,
                prop(&props, p[3])?,
            ])
        };
        println!("PLANE id {id} {kind} rotation_current={current:#x}");
        planes.push(PlaneInfo {
            handle,
            id,
            kind,
            crtc,
            format,
            fb_id: prop(&props, "FB_ID").map_err(fail("plane"))?,
            crtc_id: prop(&props, "CRTC_ID").map_err(fail("plane"))?,
            src: names(["SRC_X", "SRC_Y", "SRC_W", "SRC_H"]).map_err(fail("plane"))?,
            dst: names(["CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H"]).map_err(fail("plane"))?,
            rotation,
        });
    }
    if planes.is_empty() {
        return Err("no plane with a rotation property".into());
    }
    Ok((card, planes))
}

/// One TEST_ONLY request: the plane on its CRTC with `bits`.
fn test_value(
    card: &Card,
    p: &PlaneInfo,
    crtc: crtc::Handle,
    fb: framebuffer::Handle,
    bits: u64,
) -> Result<(), i32> {
    let mut req = AtomicModeReq::new();
    let h = p.handle;
    let side = u64::from(SIDE);
    req.add_property(h, p.fb_id, property::Value::Framebuffer(Some(fb)));
    req.add_property(h, p.crtc_id, property::Value::CRTC(Some(crtc)));
    for (id, v) in p.src.iter().zip([0, 0, side << 16, side << 16]) {
        req.add_property(h, *id, property::Value::UnsignedRange(v));
    }
    req.add_property(h, p.dst[0], property::Value::SignedRange(0));
    req.add_property(h, p.dst[1], property::Value::SignedRange(0));
    req.add_property(h, p.dst[2], property::Value::UnsignedRange(side));
    req.add_property(h, p.dst[3], property::Value::UnsignedRange(side));
    req.add_property(h, p.rotation, property::Value::UnsignedRange(bits));
    card.atomic_commit(AtomicCommitFlags::TEST_ONLY, req)
        .map_err(|e| e.raw_os_error().unwrap_or(-1))
}

fn run(options: &Options) -> Result<(), String> {
    let (card, planes) = open(options)?;
    let mut tally = Tally::default();
    let mut buffers: HashMap<u32, Fb> = HashMap::new();
    let mut result = Ok(());
    for p in &planes {
        tally.planes += 1;
        let (Some(crtc), Some(format)) = (p.crtc, p.format) else {
            let why = if p.crtc.is_none() {
                "no active CRTC reachable"
            } else {
                "no XRGB8888 or ARGB8888"
            };
            println!("{}", skip_line(p.id, p.kind, why));
            tally.skipped += 1;
            continue;
        };
        if let std::collections::hash_map::Entry::Vacant(slot) = buffers.entry(format as u32) {
            match make_fb(&card, format) {
                Ok(fb) => {
                    slot.insert(fb);
                }
                Err(message) => {
                    result = Err(message);
                    break;
                }
            }
        }
        let fb = buffers[&(format as u32)].fb;
        for (name, bits) in VALUES {
            let check = test_value(&card, p, crtc, fb, bits);
            tally.tried += 1;
            if check.is_ok() {
                tally.ok += 1;
            } else {
                tally.refused += 1;
            }
            println!("{}", rot_line(p.id, p.kind, name, bits, check));
        }
    }
    for (_, fb) in buffers {
        let _ = card.destroy_framebuffer(fb.fb);
        let _ = card.destroy_dumb_buffer(fb.dumb);
    }
    println!("{}", summary_line(&tally));
    result
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
        Ok(()) => {
            println!("VERDICT rotation_probe: DONE");
            ExitCode::SUCCESS
        }
        Err(message) => {
            println!("VERDICT rotation_probe: FAIL {message}");
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

    #[test]
    fn parses_arguments() {
        let o = args("").unwrap();
        assert_eq!((o.device.as_str(), o.plane), ("/dev/dri/card0", None));
        let o = args("/dev/dri/card1 --plane 7").unwrap();
        assert_eq!((o.device.as_str(), o.plane), ("/dev/dri/card1", Some(7)));
        assert_eq!(args("--device /x").unwrap().device, "/x");
        assert!(args("--plane").is_err());
        assert!(args("--plane x").is_err());
        assert!(args("--bogus").is_err());
        assert_eq!(args("-h"), Err(String::new()));
    }

    #[test]
    fn value_table_matches_uapi_bits() {
        assert_eq!(VALUES.len(), 8);
        assert_eq!(VALUES[0], ("rotate-0", 0x01));
        assert_eq!(VALUES[1], ("rotate-90", 0x02));
        assert_eq!(VALUES[2], ("rotate-180", 0x04));
        assert_eq!(VALUES[3], ("rotate-270", 0x08));
        assert_eq!(VALUES[4], ("reflect-x", 0x11));
        assert_eq!(VALUES[5], ("reflect-y", 0x21));
        assert_eq!(VALUES[6], ("rotate-90+reflect-x", 0x12));
        assert_eq!(VALUES[7], ("reflect-x+reflect-y", 0x31));
        // Every value has exactly one rotate bit; names are unique.
        for (_, b) in VALUES {
            assert_eq!((b & 0x0f).count_ones(), 1);
        }
        let mut names: Vec<&str> = VALUES.iter().map(|v| v.0).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 8);
    }

    #[test]
    fn formats_lines() {
        assert_eq!(
            rot_line(31, "Overlay", "rotate-0", 1, Ok(())),
            "ROT plane=31 type=Overlay value=rotate-0 bits=0x01 result=ok"
        );
        assert_eq!(
            rot_line(31, "Primary", "reflect-x+reflect-y", 0x31, Err(22)),
            "ROT plane=31 type=Primary value=reflect-x+reflect-y bits=0x31 \
             result=errno=22(EINVAL)"
        );
        assert!(rot_line(1, "Cursor", "x", 2, Err(999)).ends_with("result=errno=999(errno999)"));
        assert_eq!(
            skip_line(5, "Cursor", "no active CRTC reachable"),
            "ROT plane=5 type=Cursor result=skipped reason=no active CRTC reachable"
        );
        let t = Tally {
            planes: 3,
            tried: 16,
            ok: 10,
            refused: 6,
            skipped: 1,
        };
        assert_eq!(
            summary_line(&t),
            "SUMMARY rotation_probe: planes=3 tried=16 ok=10 refused=6 skipped=1"
        );
    }
}
