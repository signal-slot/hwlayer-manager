//! zpos probe: does the kernel accept equal `zpos` values on one CRTC, and is
//! `zpos` compared across CRTCs?
//!
//! ```text
//! cargo run -p composition-drm --example zpos_probe -- [/dev/dri/cardN] [--device PATH]
//!                                                      [--connector ID]
//! ```
//!
//! The tool opens the device, becomes DRM master (or exits with status 2
//! saying it could not; it never stops other processes) and picks every
//! connected connector with modes, each with its own CRTC, at its preferred
//! mode (up to two CRTCs are used). The planes it works with are the
//! non-cursor planes with a mutable `zpos` property that can reach a used CRTC.
//! Every request is `DRM_MODE_ATOMIC_TEST_ONLY | ALLOW_MODESET`; **the tool
//! never makes a real commit**. Every plane that is not in the case is switched
//! off in the request, and the planes in it show a full-screen dumb buffer
//! (XRGB8888 where listed, else ARGB8888).
//!
//! # Cases
//!
//! * `distinct` (a): the first two planes of CRTC 0 with zpos `v` and `v+1`.
//! * `equal` (b): the same two planes with the same zpos.
//! * `cross_equal` (c), only with two CRTCs: one plane on CRTC 0 and a
//!   different plane on CRTC 1, both with the same zpos.
//!
//! The values are the smallest ones the planes' ranges allow. A case that
//! cannot be built (too few planes, no common value) prints
//! `ZPOS case=<name> result=skipped reason=...`.
//!
//! # Output
//!
//! ```text
//! DATE / KERNEL / DEVICE / DRIVER / CRTC / PLANE lines
//! ZPOS case=distinct planes=31,37 zpos=0,1 result=ok
//! ZPOS case=equal planes=31,37 zpos=1,1 result=EINVAL
//! ZPOS case=cross_equal planes=31,57 zpos=1,1 result=ok
//! VERDICT zpos_probe: DONE
//! ```
//!
//! `planes` and `zpos` are in the same order; `result` is `ok` or the errno.
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
    connector, crtc, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice,
    ModeTypeFlags, PlaneType, ResourceHandle,
};
use drm::{ClientCapability, Device as BasicDevice};

const USAGE: &str = "usage: zpos_probe [/dev/dri/cardN] [--device PATH] [--connector ID]";

/// At most this many CRTCs are used.
const MAX_CRTCS: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Options {
    device: String,
    connector: Option<u32>,
}

/// Parses the arguments after the program name. An empty error asks for the
/// usage text alone.
fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
    let mut options = Options {
        device: "/dev/dri/card0".into(),
        connector: None,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--device" => options.device = args.next().ok_or("--device needs a value")?,
            "--connector" => {
                let text = args.next().ok_or("--connector needs a value")?;
                options.connector = Some(text.parse().map_err(|_| format!("bad number {text}"))?);
            }
            "-h" | "--help" => return Err(String::new()),
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => options.device = other.to_string(),
        }
    }
    Ok(options)
}

/// A plane with a mutable zpos, as far as case generation needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PlaneSpec {
    id: u32,
    /// Indices of the used CRTCs the plane can reach.
    crtcs: Vec<usize>,
    zmin: u64,
    zmax: u64,
}

/// One plane of a case: on which used CRTC, with which zpos.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    plane: u32,
    crtc: usize,
    zpos: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Case {
    name: &'static str,
    entries: Vec<Entry>,
}

/// A case, or its name and why it cannot be built.
type Planned = Result<Case, (&'static str, String)>;

/// The two cases on CRTC 0 and, with two CRTCs, the cross-CRTC case. `planes`
/// must be sorted by id.
fn plan_cases(planes: &[PlaneSpec], crtc_count: usize) -> Vec<Planned> {
    let on = |crtc: usize| -> Vec<&PlaneSpec> {
        planes.iter().filter(|p| p.crtcs.contains(&crtc)).collect()
    };
    let mut cases = Vec::new();
    let first = on(0);
    let pair = (first.len() >= 2).then(|| (first[0], first[1]));
    let few = || {
        format!(
            "{} plane(s) with mutable zpos on CRTC 0, need 2",
            first.len()
        )
    };
    // (a) a, a+1: the lowest values the ranges allow.
    cases.push(match pair {
        None => Err(("distinct", few())),
        Some((a, b)) => {
            let va = a.zmin;
            let vb = b.zmin.max(va + 1);
            if va <= a.zmax && vb <= b.zmax {
                Ok(pair_case("distinct", a, b, va, vb))
            } else {
                Err((
                    "distinct",
                    "the zpos ranges leave no distinct values".into(),
                ))
            }
        }
    });
    // (b) the same value on both.
    cases.push(match pair {
        None => Err(("equal", few())),
        Some((a, b)) => {
            let v = a.zmin.max(b.zmin);
            if v <= a.zmax.min(b.zmax) {
                Ok(pair_case("equal", a, b, v, v))
            } else {
                Err(("equal", "the zpos ranges have no common value".into()))
            }
        }
    });
    // (c) different planes on different CRTCs, the same value.
    if crtc_count >= 2 {
        let on1 = on(1);
        let found = on(0)
            .into_iter()
            .find_map(|x| on1.iter().copied().find(|p| p.id != x.id).map(|y| (x, y)));
        cases.push(match found {
            Some((x, y)) => {
                let v = x.zmin.max(y.zmin);
                if v <= x.zmax.min(y.zmax) {
                    Ok(Case {
                        name: "cross_equal",
                        entries: vec![
                            Entry {
                                plane: x.id,
                                crtc: 0,
                                zpos: v,
                            },
                            Entry {
                                plane: y.id,
                                crtc: 1,
                                zpos: v,
                            },
                        ],
                    })
                } else {
                    Err(("cross_equal", "the zpos ranges have no common value".into()))
                }
            }
            None => Err((
                "cross_equal",
                "no two different planes for CRTC 0 and CRTC 1".into(),
            )),
        });
    }
    cases
}

fn pair_case(name: &'static str, a: &PlaneSpec, b: &PlaneSpec, va: u64, vb: u64) -> Case {
    Case {
        name,
        entries: vec![
            Entry {
                plane: a.id,
                crtc: 0,
                zpos: va,
            },
            Entry {
                plane: b.id,
                crtc: 0,
                zpos: vb,
            },
        ],
    }
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

fn join<T: ToString>(values: impl Iterator<Item = T>) -> String {
    values.map(|v| v.to_string()).collect::<Vec<_>>().join(",")
}

/// `ZPOS case=<name> planes=<ids> zpos=<values> result=ok|<errno>`.
fn case_line(case: &Case, result: Result<(), i32>) -> String {
    format!(
        "ZPOS case={} planes={} zpos={} result={}",
        case.name,
        join(case.entries.iter().map(|e| e.plane)),
        join(case.entries.iter().map(|e| e.zpos)),
        match result {
            Ok(()) => "ok".into(),
            Err(errno) => errno_name(errno),
        }
    )
}

fn skip_line(name: &str, reason: &str) -> String {
    format!("ZPOS case={name} result=skipped reason={reason}")
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

struct Output {
    crtc: crtc::Handle,
    connector: connector::Handle,
    connector_crtc_id: property::Handle,
    crtc_mode_id: property::Handle,
    crtc_active: property::Handle,
    mode_blob: u64,
    size: (u32, u32),
}

struct PlaneInfo {
    handle: plane::Handle,
    spec: PlaneSpec,
    format: DrmFourcc,
    fb_id: property::Handle,
    crtc_id: property::Handle,
    src: [property::Handle; 4],
    dst: [property::Handle; 4],
    zpos: property::Handle,
}

/// A dumb buffer and the framebuffer made from it.
struct Fb {
    dumb: DumbBuffer,
    fb: framebuffer::Handle,
}

fn open(options: &Options) -> Result<(Card, Vec<Output>, Vec<PlaneInfo>), String> {
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

    // One connector and CRTC per output, distinct CRTCs.
    let resources = card.resource_handles().map_err(fail("resources"))?;
    let mut outputs: Vec<Output> = Vec::new();
    for handle in resources.connectors() {
        if outputs.len() == MAX_CRTCS {
            break;
        }
        if options.connector.is_some_and(|id| id != u32::from(*handle)) {
            continue;
        }
        let info = card
            .get_connector(*handle, false)
            .map_err(fail("connector"))?;
        if info.state() != connector::State::Connected || info.modes().is_empty() {
            continue;
        }
        let mode = info
            .modes()
            .iter()
            .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
            .or_else(|| info.modes().first())
            .copied()
            .ok_or("no mode")?;
        let taken = |c: crtc::Handle| outputs.iter().any(|o| o.crtc == c);
        let mut chosen = None;
        if let Some(encoder) = info.current_encoder() {
            chosen = card
                .get_encoder(encoder)
                .map_err(fail("encoder"))?
                .crtc()
                .filter(|c| !taken(*c));
        }
        if chosen.is_none() {
            for encoder in info.encoders() {
                let possible = card
                    .get_encoder(*encoder)
                    .map_err(fail("encoder"))?
                    .possible_crtcs();
                chosen = resources
                    .filter_crtcs(possible)
                    .into_iter()
                    .find(|c| !taken(*c));
                if chosen.is_some() {
                    break;
                }
            }
        }
        let Some(crtc) = chosen else { continue };
        let connector_props = named_properties(&card, *handle).map_err(fail("connector props"))?;
        let crtc_props = named_properties(&card, crtc).map_err(fail("crtc props"))?;
        let mode_blob = match card
            .create_property_blob(&mode)
            .map_err(fail("mode blob"))?
        {
            property::Value::Blob(id) => id,
            _ => return Err("mode blob".into()),
        };
        let (w, h) = mode.size();
        println!(
            "CRTC index {} id {} connector {} mode {w}x{h}@{}Hz",
            outputs.len(),
            u32::from(crtc),
            u32::from(*handle),
            mode.vrefresh()
        );
        outputs.push(Output {
            crtc,
            connector: *handle,
            connector_crtc_id: prop(&connector_props, "CRTC_ID").map_err(fail("connector"))?,
            crtc_mode_id: prop(&crtc_props, "MODE_ID").map_err(fail("crtc"))?,
            crtc_active: prop(&crtc_props, "ACTIVE").map_err(fail("crtc"))?,
            mode_blob,
            size: (u32::from(w), u32::from(h)),
        });
    }
    if outputs.is_empty() {
        return Err("no connected connector with modes and a free CRTC".into());
    }

    let mut planes = Vec::new();
    let mut handles = card.plane_handles().map_err(fail("planes"))?;
    handles.sort_by_key(|h| u32::from(*h));
    for handle in handles {
        let pinfo = card.get_plane(handle).map_err(fail("plane"))?;
        let props = named_properties(&card, handle).map_err(fail("plane props"))?;
        let kind = match props.get("type").map(|(_, v)| *v as u32) {
            Some(v) if v == PlaneType::Primary as u32 => PlaneType::Primary,
            Some(v) if v == PlaneType::Cursor as u32 => PlaneType::Cursor,
            _ => PlaneType::Overlay,
        };
        let possible = resources.filter_crtcs(pinfo.possible_crtcs());
        let crtcs: Vec<usize> = outputs
            .iter()
            .enumerate()
            .filter(|(_, o)| possible.contains(&o.crtc))
            .map(|(i, _)| i)
            .collect();
        let zpos = match props.get("zpos") {
            Some((h, _)) => {
                let info = card.get_property(*h).map_err(fail("zpos"))?;
                match info.value_type() {
                    property::ValueType::UnsignedRange(min, max) if info.mutable() => {
                        Some((*h, *min, *max))
                    }
                    _ => None,
                }
            }
            None => None,
        };
        println!(
            "PLANE id {} {:?} crtcs {} zpos {}",
            u32::from(handle),
            kind,
            if crtcs.is_empty() {
                "none".into()
            } else {
                join(crtcs.iter())
            },
            zpos.map_or("none or fixed".into(), |(_, min, max)| format!(
                "{min}..{max}"
            ))
        );
        let format = [DrmFourcc::Xrgb8888, DrmFourcc::Argb8888]
            .into_iter()
            .find(|f| pinfo.formats().contains(&(*f as u32)));
        let (Some((zpos, zmin, zmax)), Some(format)) = (zpos, format) else {
            continue;
        };
        if kind == PlaneType::Cursor || crtcs.is_empty() {
            continue;
        }
        let names = |p: [&str; 4]| -> io::Result<[property::Handle; 4]> {
            Ok([
                prop(&props, p[0])?,
                prop(&props, p[1])?,
                prop(&props, p[2])?,
                prop(&props, p[3])?,
            ])
        };
        planes.push(PlaneInfo {
            handle,
            spec: PlaneSpec {
                id: u32::from(handle),
                crtcs,
                zmin,
                zmax,
            },
            format,
            fb_id: prop(&props, "FB_ID").map_err(fail("plane"))?,
            crtc_id: prop(&props, "CRTC_ID").map_err(fail("plane"))?,
            src: names(["SRC_X", "SRC_Y", "SRC_W", "SRC_H"]).map_err(fail("plane"))?,
            dst: names(["CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H"]).map_err(fail("plane"))?,
            zpos,
        });
    }
    Ok((card, outputs, planes))
}

fn make_fb(card: &Card, size: (u32, u32), format: DrmFourcc) -> Result<Fb, String> {
    let dumb = card
        .create_dumb_buffer(size, format, 32)
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

/// One TEST_ONLY request for `case`; planes outside it are switched off.
fn test_case(
    card: &Card,
    outputs: &[Output],
    planes: &[PlaneInfo],
    case: &Case,
    buffers: &mut HashMap<(usize, u32), Fb>,
) -> Result<Result<(), i32>, String> {
    let mut req = AtomicModeReq::new();
    let mut used_crtcs: Vec<usize> = case.entries.iter().map(|e| e.crtc).collect();
    used_crtcs.sort_unstable();
    used_crtcs.dedup();
    for &i in &used_crtcs {
        let o = &outputs[i];
        req.add_property(
            o.connector,
            o.connector_crtc_id,
            property::Value::CRTC(Some(o.crtc)),
        );
        req.add_property(o.crtc, o.crtc_mode_id, property::Value::Blob(o.mode_blob));
        req.add_property(o.crtc, o.crtc_active, property::Value::Boolean(true));
    }
    for p in planes {
        let Some(entry) = case.entries.iter().find(|e| e.plane == p.spec.id) else {
            req.add_property(p.handle, p.fb_id, property::Value::Framebuffer(None));
            req.add_property(p.handle, p.crtc_id, property::Value::CRTC(None));
            continue;
        };
        let output = &outputs[entry.crtc];
        let key = (entry.crtc, p.format as u32);
        if let std::collections::hash_map::Entry::Vacant(slot) = buffers.entry(key) {
            slot.insert(make_fb(card, output.size, p.format)?);
        }
        let fb = buffers[&key].fb;
        let h = p.handle;
        req.add_property(h, p.fb_id, property::Value::Framebuffer(Some(fb)));
        req.add_property(h, p.crtc_id, property::Value::CRTC(Some(output.crtc)));
        let source = [
            0,
            0,
            u64::from(output.size.0) << 16,
            u64::from(output.size.1) << 16,
        ];
        for (id, v) in p.src.iter().zip(source) {
            req.add_property(h, *id, property::Value::UnsignedRange(v));
        }
        req.add_property(h, p.dst[0], property::Value::SignedRange(0));
        req.add_property(h, p.dst[1], property::Value::SignedRange(0));
        req.add_property(
            h,
            p.dst[2],
            property::Value::UnsignedRange(u64::from(output.size.0)),
        );
        req.add_property(
            h,
            p.dst[3],
            property::Value::UnsignedRange(u64::from(output.size.1)),
        );
        req.add_property(h, p.zpos, property::Value::UnsignedRange(entry.zpos));
    }
    Ok(card
        .atomic_commit(
            AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::ALLOW_MODESET,
            req,
        )
        .map_err(|e| e.raw_os_error().unwrap_or(-1)))
}

fn run(options: &Options) -> Result<(), String> {
    let (card, outputs, planes) = open(options)?;
    let specs: Vec<PlaneSpec> = planes.iter().map(|p| p.spec.clone()).collect();
    let mut buffers: HashMap<(usize, u32), Fb> = HashMap::new();
    let mut result = Ok(());
    for planned in plan_cases(&specs, outputs.len()) {
        match planned {
            Err((name, reason)) => println!("{}", skip_line(name, &reason)),
            Ok(case) => match test_case(&card, &outputs, &planes, &case, &mut buffers) {
                Ok(check) => println!("{}", case_line(&case, check)),
                Err(message) => {
                    result = Err(message);
                    break;
                }
            },
        }
    }
    for (_, fb) in buffers {
        let _ = card.destroy_framebuffer(fb.fb);
        let _ = card.destroy_dumb_buffer(fb.dumb);
    }
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
            println!("VERDICT zpos_probe: DONE");
            ExitCode::SUCCESS
        }
        Err(message) => {
            println!("VERDICT zpos_probe: FAIL {message}");
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

    fn spec(id: u32, crtcs: &[usize], zmin: u64, zmax: u64) -> PlaneSpec {
        PlaneSpec {
            id,
            crtcs: crtcs.to_vec(),
            zmin,
            zmax,
        }
    }

    #[test]
    fn parses_arguments() {
        let o = args("").unwrap();
        assert_eq!((o.device.as_str(), o.connector), ("/dev/dri/card0", None));
        let o = args("/dev/dri/card1 --connector 7").unwrap();
        assert_eq!(
            (o.device.as_str(), o.connector),
            ("/dev/dri/card1", Some(7))
        );
        assert_eq!(args("--device /x").unwrap().device, "/x");
        assert!(args("--connector").is_err());
        assert!(args("--connector x").is_err());
        assert!(args("--bogus").is_err());
        assert_eq!(args("-h"), Err(String::new()));
    }

    #[test]
    fn single_crtc_gives_distinct_and_equal() {
        let planes = [
            spec(31, &[0], 0, 3),
            spec(37, &[0], 0, 3),
            spec(40, &[1], 0, 3),
        ];
        let cases = plan_cases(&planes, 1);
        assert_eq!(cases.len(), 2);
        let d = cases[0].as_ref().unwrap();
        assert_eq!(d.name, "distinct");
        assert_eq!(
            case_line(d, Ok(())),
            "ZPOS case=distinct planes=31,37 zpos=0,1 result=ok"
        );
        let e = cases[1].as_ref().unwrap();
        assert_eq!(
            case_line(e, Err(22)),
            "ZPOS case=equal planes=31,37 zpos=0,0 result=EINVAL"
        );
    }

    #[test]
    fn two_crtcs_add_cross_case() {
        let planes = [
            spec(31, &[0], 1, 3),
            spec(37, &[0, 1], 0, 3),
            spec(40, &[1], 2, 3),
        ];
        let cases = plan_cases(&planes, 2);
        assert_eq!(cases.len(), 3);
        let c = cases[2].as_ref().unwrap();
        assert_eq!(c.name, "cross_equal");
        assert_eq!(
            case_line(c, Err(95)),
            "ZPOS case=cross_equal planes=31,37 zpos=1,1 result=EOPNOTSUPP"
        );
        assert_eq!(c.entries[0].crtc, 0);
        assert_eq!(c.entries[1].crtc, 1);
    }

    #[test]
    fn cross_case_picks_different_planes_or_skips() {
        // Plane 31 reaches both CRTCs, 37 only CRTC 0: 37 on CRTC 0, 31 on 1.
        let planes = [spec(31, &[0, 1], 0, 3), spec(37, &[0], 0, 3)];
        let c = plan_cases(&planes, 2)[2].clone().unwrap();
        assert_eq!((c.entries[0].plane, c.entries[1].plane), (37, 31));
    }

    #[test]
    fn skips_when_cases_cannot_be_built() {
        let one = [spec(31, &[0], 0, 3)];
        let cases = plan_cases(&one, 1);
        assert!(cases.iter().all(|c| c.is_err()));
        assert_eq!(
            skip_line("equal", "r"),
            "ZPOS case=equal result=skipped reason=r"
        );
        // No common value / no distinct values.
        let apart = [spec(1, &[0], 0, 0), spec(2, &[0], 2, 3)];
        let cases = plan_cases(&apart, 1);
        assert!(cases[0].is_ok());
        assert!(cases[1].is_err());
        let same = [spec(1, &[0], 0, 0), spec(2, &[0], 0, 0)];
        let cases = plan_cases(&same, 1);
        assert!(cases[0].is_err());
        assert!(cases[1].is_ok());
        let lone = [spec(1, &[0, 1], 0, 3)];
        assert!(plan_cases(&lone, 2)[2].is_err());
    }
}
