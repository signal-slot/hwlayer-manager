//! Downscale probe: which source-to-destination shrink ratios does the
//! kernel's atomic `TEST_ONLY` check accept, plane by plane?
//!
//! ```text
//! cargo run -p composition-drm --example downscale_probe -- [/dev/dri/cardN] [options]
//!
//!   --device PATH      DRM device (default /dev/dri/card0); a bare argument works too
//!   --plane ID         test only this plane (default: every overlay plane that
//!                      can drive the chosen CRTC)
//!   --dst WxH          destination size in output pixels, fixed (default 64x64)
//!   --ratios LIST      comma separated shrink ratios replacing the default
//!                      ladder; each is `N` or `1/N` with N >= 2, meaning the
//!                      source is N times the destination on the axis
//!                      (default 1/2,1/4,1/8,1/16,1/24,1/31,1/32,1/33,1/40)
//!   --connector ID     connector to use (default: the first connected one)
//! ```
//!
//! The probe becomes DRM master (it never stops other processes; without
//! master it exits 2), takes the connector's preferred mode and a CRTC, and
//! only ever issues `DRM_MODE_ATOMIC_TEST_ONLY | ALLOW_MODESET` commits, so
//! nothing is committed and nothing needs restoring.
//!
//! For each plane a baseline (source equal to destination) is checked first;
//! then, for each ratio `1/N`, the width alone (source `N*w` by `h`) and the
//! height alone (source `w` by `N*h`) are shrunk. One line per attempt:
//!
//! ```text
//! DOWNSCALE plane=P axis=w|h ratio=1/N src=WxH dst=WxH test_only=accepted|rejected errno=E
//! ```
//!
//! plus `SKIP` lines for ratios whose source does not fit a buffer, a
//! `RESULT` line per plane and axis, and a final `VERDICT`. Exit status: 0 if
//! every plane passed its baseline, 1 if some plane could not be probed, 2
//! for a usage error or no DRM master.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use composition_drm::Card;
use drm::buffer::{DrmFourcc, DrmModifier, PlanarBuffer};
use drm::control::atomic::AtomicModeReq;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    connector, crtc, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice,
    FbCmd2Flags, ModeTypeFlags, PlaneType, ResourceHandle,
};
use drm::{ClientCapability, Device as BasicDevice};

const USAGE: &str = "usage: downscale_probe [/dev/dri/cardN] [--device PATH] [--plane ID] \
[--dst WxH] [--ratios LIST] [--connector ID]";

/// Default ladder: the source is N times the destination.
const DEFAULT_RATIOS: [u32; 9] = [2, 4, 8, 16, 24, 31, 32, 33, 40];
/// Largest dumb buffer edge the probe asks for.
const MAX_EDGE: u32 = 8192;
/// 32-bit formats a dumb buffer can hold, in order of preference.
const FORMATS: [DrmFourcc; 4] = [
    DrmFourcc::Xrgb8888,
    DrmFourcc::Argb8888,
    DrmFourcc::Xbgr8888,
    DrmFourcc::Abgr8888,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Axis {
    W,
    H,
}

impl Axis {
    fn name(self) -> &'static str {
        match self {
            Axis::W => "w",
            Axis::H => "h",
        }
    }
}

/// One ratio `N` or `1/N` (N >= 2): the source is N times the destination.
fn parse_ratio(text: &str) -> Result<u32, String> {
    let den = text.strip_prefix("1/").unwrap_or(text);
    match den.parse::<u32>() {
        Ok(n) if n >= 2 => Ok(n),
        _ => Err(format!("bad ratio {text} (use N or 1/N with N >= 2)")),
    }
}

/// A `--ratios` list, in the given order; a duplicate is an error.
fn parse_ratios(text: &str) -> Result<Vec<u32>, String> {
    let mut out: Vec<u32> = Vec::new();
    for part in text.split(',') {
        let n = parse_ratio(part.trim())?;
        if out.contains(&n) {
            return Err(format!("ratio 1/{n} listed twice"));
        }
        out.push(n);
    }
    Ok(out)
}

fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let bad = || format!("bad size {text} (use WxH)");
    let (w, h) = text.split_once('x').ok_or_else(bad)?;
    let w: u32 = w.parse().map_err(|_| bad())?;
    let h: u32 = h.parse().map_err(|_| bad())?;
    if w == 0 || h == 0 {
        return Err(bad());
    }
    Ok((w, h))
}

/// Source size in whole pixels for shrinking `axis` by `den`, or `None` on
/// overflow or if an edge would exceed `limit`.
fn src_size(dst: (u32, u32), axis: Axis, den: u32, limit: (u32, u32)) -> Option<(u32, u32)> {
    let src = match axis {
        Axis::W => (dst.0.checked_mul(den)?, dst.1),
        Axis::H => (dst.0, dst.1.checked_mul(den)?),
    };
    (src.0 <= limit.0 && src.1 <= limit.1).then_some(src)
}

/// A pixel size as 16.16 fixed point, the unit of the SRC_W/SRC_H properties.
fn fixed16(size: (u32, u32)) -> (u64, u64) {
    (u64::from(size.0) << 16, u64::from(size.1) << 16)
}

/// Size of the one buffer that serves every attempt of a ladder: large
/// enough for the biggest source on each axis, never below `dst`, and
/// ignoring ratios whose source would exceed `MAX_EDGE`-style `limit`.
fn buffer_size(dst: (u32, u32), ratios: &[u32], limit: u32) -> (u32, u32) {
    let mut size = dst;
    for &den in ratios {
        for axis in [Axis::W, Axis::H] {
            if let Some(src) = src_size(dst, axis, den, (limit, limit)) {
                size = (size.0.max(src.0), size.1.max(src.1));
            }
        }
    }
    size
}

struct Options {
    device: String,
    plane: Option<u32>,
    dst: (u32, u32),
    ratios: Vec<u32>,
    connector: Option<u32>,
}

fn parse_args() -> Result<Options, String> {
    parse_arg_list(std::env::args().skip(1))
}

fn parse_arg_list(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        device: "/dev/dri/card0".into(),
        plane: None,
        dst: (64, 64),
        ratios: DEFAULT_RATIOS.to_vec(),
        connector: None,
    };
    let mut args = args.into_iter();
    let number = |text: &str| -> Result<u32, String> {
        text.parse().map_err(|_| format!("bad number {text}"))
    };
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--device" => options.device = value("--device")?,
            "--plane" => options.plane = Some(number(&value("--plane")?)?),
            "--connector" => options.connector = Some(number(&value("--connector")?)?),
            "--dst" => options.dst = parse_size(&value("--dst")?)?,
            "--ratios" => options.ratios = parse_ratios(&value("--ratios")?)?,
            "-h" | "--help" => return Err(String::new()),
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => options.device = other.to_string(),
        }
    }
    Ok(options)
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

fn fourcc_text(format: DrmFourcc) -> String {
    (format as u32)
        .to_le_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_graphic() {
                *b as char
            } else {
                '?'
            }
        })
        .collect()
}

fn named_properties<H: ResourceHandle>(
    card: &Card,
    handle: H,
) -> io::Result<HashMap<String, (property::Handle, u64)>> {
    let set = card.get_properties(handle)?;
    let mut named = HashMap::new();
    for (prop, value) in set.iter() {
        let info = card.get_property(*prop)?;
        named.insert(info.name().to_string_lossy().into_owned(), (*prop, *value));
    }
    Ok(named)
}

/// The property handle of a plane, or an error naming what is missing.
fn prop(
    props: &HashMap<String, (property::Handle, u64)>,
    name: &str,
) -> io::Result<property::Handle> {
    props
        .get(name)
        .map(|(handle, _)| *handle)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no property {name}")))
}

struct PlaneInfo {
    handle: plane::Handle,
    kind: PlaneType,
    formats: Vec<u32>,
    possible: bool,
    fb_id: property::Handle,
    crtc_id: property::Handle,
    src: [property::Handle; 4],
    dst: [property::Handle; 4],
}

/// A dumb buffer and the framebuffer made from it.
struct Fb {
    dumb: DumbBuffer,
    fb: framebuffer::Handle,
}

struct Layout {
    size: (u32, u32),
    format: DrmFourcc,
    pitch: u32,
    handle: drm::buffer::Handle,
}

impl PlanarBuffer for Layout {
    fn size(&self) -> (u32, u32) {
        self.size
    }
    fn format(&self) -> DrmFourcc {
        self.format
    }
    fn modifier(&self) -> Option<DrmModifier> {
        None
    }
    fn pitches(&self) -> [u32; 4] {
        [self.pitch, 0, 0, 0]
    }
    fn handles(&self) -> [Option<drm::buffer::Handle>; 4] {
        [Some(self.handle), None, None, None]
    }
    fn offsets(&self) -> [u32; 4] {
        [0; 4]
    }
}

fn make_fb(card: &Card, size: (u32, u32), format: DrmFourcc) -> io::Result<Fb> {
    let dumb = card.create_dumb_buffer(size, format, 32)?;
    let layout = Layout {
        size,
        format,
        pitch: drm::buffer::Buffer::pitch(&dumb),
        handle: drm::buffer::Buffer::handle(&dumb),
    };
    match card.add_planar_framebuffer(&layout, FbCmd2Flags::empty()) {
        Ok(fb) => Ok(Fb { dumb, fb }),
        Err(error) => {
            let _ = card.destroy_dumb_buffer(dumb);
            Err(error)
        }
    }
}

fn fail(what: &str) -> impl Fn(io::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

fn pick_format(plane: &PlaneInfo) -> Option<DrmFourcc> {
    FORMATS
        .into_iter()
        .find(|f| plane.formats.contains(&(*f as u32)))
}

/// One TEST_ONLY check's outcome: `Ok` if accepted, the errno otherwise.
type Check = Result<(), i32>;

fn status(outcome: Check) -> (&'static str, i32) {
    match outcome {
        Ok(()) => ("accepted", 0),
        Err(errno) => ("rejected", errno),
    }
}

struct Probe<'a> {
    card: &'a Card,
    crtc: crtc::Handle,
    connector: connector::Handle,
    connector_crtc_id: property::Handle,
    crtc_mode_id: property::Handle,
    crtc_active: property::Handle,
    mode_blob: u64,
    output: (u32, u32),
    dst: (u32, u32),
    planes: &'a [PlaneInfo],
}

impl Probe<'_> {
    /// One TEST_ONLY check of `plane` with the source size `src` (pixels),
    /// over a full-screen primary `background`.
    fn check(
        &self,
        plane: &PlaneInfo,
        fb: framebuffer::Handle,
        background: Option<(&PlaneInfo, framebuffer::Handle)>,
        src: (u32, u32),
    ) -> Check {
        let mut req = AtomicModeReq::new();
        req.add_property(
            self.connector,
            self.connector_crtc_id,
            property::Value::CRTC(Some(self.crtc)),
        );
        req.add_property(
            self.crtc,
            self.crtc_mode_id,
            property::Value::Blob(self.mode_blob),
        );
        req.add_property(self.crtc, self.crtc_active, property::Value::Boolean(true));
        for other in self.planes {
            let used = other.handle == plane.handle
                || background.is_some_and(|(p, _)| p.handle == other.handle);
            if !used {
                req.add_property(
                    other.handle,
                    other.fb_id,
                    property::Value::Framebuffer(None),
                );
                req.add_property(other.handle, other.crtc_id, property::Value::CRTC(None));
            }
        }
        if let Some((primary, bfb)) = background {
            self.place(&mut req, primary, bfb, fixed16(self.output), self.output);
        }
        self.place(&mut req, plane, fb, fixed16(src), self.dst);
        match self.card.atomic_commit(
            AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::ALLOW_MODESET,
            req,
        ) {
            Ok(()) => Ok(()),
            Err(error) => Err(error.raw_os_error().unwrap_or(-1)),
        }
    }

    fn place(
        &self,
        req: &mut AtomicModeReq,
        plane: &PlaneInfo,
        fb: framebuffer::Handle,
        src: (u64, u64),
        dst: (u32, u32),
    ) {
        let h = plane.handle;
        req.add_property(h, plane.fb_id, property::Value::Framebuffer(Some(fb)));
        req.add_property(h, plane.crtc_id, property::Value::CRTC(Some(self.crtc)));
        let source = [0, 0, src.0, src.1];
        for (p, v) in plane.src.iter().zip(source) {
            req.add_property(h, *p, property::Value::UnsignedRange(v));
        }
        req.add_property(h, plane.dst[0], property::Value::SignedRange(0));
        req.add_property(h, plane.dst[1], property::Value::SignedRange(0));
        req.add_property(
            h,
            plane.dst[2],
            property::Value::UnsignedRange(u64::from(dst.0)),
        );
        req.add_property(
            h,
            plane.dst[3],
            property::Value::UnsignedRange(u64::from(dst.1)),
        );
    }
}

fn run(options: &Options) -> Result<bool, String> {
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
            info.version.2,
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
             holds it; stop it for the run. This probe never stops other processes"
        )
    })?;

    let resources = card.resource_handles().map_err(fail("resources"))?;
    let mut chosen = None;
    for handle in resources.connectors() {
        if options.connector.is_some_and(|id| id != u32::from(*handle)) {
            continue;
        }
        let info = card
            .get_connector(*handle, false)
            .map_err(fail("connector"))?;
        if info.state() == connector::State::Connected && !info.modes().is_empty() {
            chosen = Some((*handle, info));
            break;
        }
    }
    let (connector, info) = chosen.ok_or("no connected connector with modes")?;
    let mode = info
        .modes()
        .iter()
        .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
        .or_else(|| info.modes().first())
        .copied()
        .ok_or("no mode")?;
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
    let (mw, mh) = mode.size();
    let output = (u32::from(mw), u32::from(mh));
    println!(
        "MODE {}x{}@{}Hz connector {} crtc {}",
        output.0,
        output.1,
        mode.vrefresh(),
        u32::from(connector),
        u32::from(crtc)
    );
    let dst = options.dst;
    if dst.0 > output.0 || dst.1 > output.1 {
        return Err(format!(
            "--dst {}x{} is larger than the output",
            dst.0, dst.1
        ));
    }

    let connector_props = named_properties(&card, connector).map_err(fail("connector props"))?;
    let crtc_props = named_properties(&card, crtc).map_err(fail("crtc props"))?;
    let mode_blob = match card
        .create_property_blob(&mode)
        .map_err(fail("mode blob"))?
    {
        property::Value::Blob(id) => id,
        _ => return Err("mode blob".into()),
    };

    let mut planes = Vec::new();
    for handle in card.plane_handles().map_err(fail("planes"))? {
        let pinfo = card.get_plane(handle).map_err(fail("plane"))?;
        let props = named_properties(&card, handle).map_err(fail("plane props"))?;
        let kind = match props.get("type").map(|(_, v)| *v as u32) {
            Some(v) if v == PlaneType::Primary as u32 => PlaneType::Primary,
            Some(v) if v == PlaneType::Cursor as u32 => PlaneType::Cursor,
            _ => PlaneType::Overlay,
        };
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
            kind,
            formats: pinfo.formats().to_vec(),
            possible: resources
                .filter_crtcs(pinfo.possible_crtcs())
                .contains(&crtc),
            fb_id: prop(&props, "FB_ID").map_err(fail("plane"))?,
            crtc_id: prop(&props, "CRTC_ID").map_err(fail("plane"))?,
            src: names(["SRC_X", "SRC_Y", "SRC_W", "SRC_H"]).map_err(fail("plane"))?,
            dst: names(["CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H"]).map_err(fail("plane"))?,
        });
    }
    for p in &planes {
        println!(
            "PLANE id {} {:?} possible_on_crtc {}",
            u32::from(p.handle),
            p.kind,
            p.possible
        );
    }

    let probe = Probe {
        card: &card,
        crtc,
        connector,
        connector_crtc_id: prop(&connector_props, "CRTC_ID").map_err(fail("connector"))?,
        crtc_mode_id: prop(&crtc_props, "MODE_ID").map_err(fail("crtc"))?,
        crtc_active: prop(&crtc_props, "ACTIVE").map_err(fail("crtc"))?,
        mode_blob,
        output,
        dst,
        planes: &planes,
    };
    let result = probe_planes(&probe, options);
    // Nothing was committed, so only the mode blob is left to drop.
    let _ = card.destroy_property_blob(mode_blob);
    result
}

fn probe_planes(probe: &Probe<'_>, options: &Options) -> Result<bool, String> {
    let card = probe.card;
    let dst = probe.dst;
    let planes = probe.planes;
    if let Some(id) = options.plane {
        if !planes.iter().any(|p| u32::from(p.handle) == id) {
            return Err(format!("no plane {id}"));
        }
    }
    let primary = planes
        .iter()
        .find(|p| p.kind == PlaneType::Primary && p.possible);
    let mut buffers: Vec<Fb> = Vec::new();
    let mut all_ok = true;
    let mut tested = 0;
    let outcome = (|| -> Result<(), String> {
        let background = match primary {
            Some(primary) => {
                let format =
                    pick_format(primary).ok_or("primary plane lists no 32-bit RGB format")?;
                let fb = make_fb(card, probe.output, format)
                    .map_err(|e| format!("background buffer {}: {e}", fourcc_text(format)))?;
                let handle = fb.fb;
                buffers.push(fb);
                Some((primary, handle))
            }
            None => None,
        };
        for plane in planes {
            if plane.kind != PlaneType::Overlay
                || !plane.possible
                || options
                    .plane
                    .is_some_and(|id| id != u32::from(plane.handle))
            {
                continue;
            }
            let pid = u32::from(plane.handle);
            let Some(format) = pick_format(plane) else {
                println!("SKIP plane={pid} no 32-bit RGB format");
                continue;
            };
            let size = buffer_size(dst, &options.ratios, MAX_EDGE);
            let fb = match make_fb(card, size, format) {
                Ok(fb) => fb,
                Err(e) => {
                    println!(
                        "SKIP plane={pid} buffer {}x{} {} refused ({e})",
                        size.0,
                        size.1,
                        fourcc_text(format)
                    );
                    all_ok = false;
                    continue;
                }
            };
            let fb_handle = fb.fb;
            buffers.push(fb);
            tested += 1;
            let (base, errno) = status(probe.check(plane, fb_handle, background, dst));
            println!(
                "DOWNSCALE plane={pid} axis=base ratio=1/1 src={}x{} dst={}x{} \
                 test_only={base} errno={errno}",
                dst.0, dst.1, dst.0, dst.1
            );
            if base != "accepted" {
                println!(
                    "RESULT plane={pid} format={} baseline=rejected",
                    fourcc_text(format)
                );
                all_ok = false;
                continue;
            }
            for axis in [Axis::W, Axis::H] {
                let mut max_ok: Option<u32> = None;
                let mut min_rej: Option<u32> = None;
                for &den in &options.ratios {
                    let Some(src) = src_size(dst, axis, den, size) else {
                        println!(
                            "SKIP plane={pid} axis={} ratio=1/{den} source does not fit",
                            axis.name()
                        );
                        continue;
                    };
                    let (st, errno) = status(probe.check(plane, fb_handle, background, src));
                    println!(
                        "DOWNSCALE plane={pid} axis={} ratio=1/{den} src={}x{} dst={}x{} \
                         test_only={st} errno={errno}",
                        axis.name(),
                        src.0,
                        src.1,
                        dst.0,
                        dst.1
                    );
                    if st == "accepted" {
                        max_ok = Some(max_ok.map_or(den, |m| m.max(den)));
                    } else {
                        min_rej = Some(min_rej.map_or(den, |m| m.min(den)));
                    }
                }
                println!(
                    "RESULT plane={pid} axis={} format={} max_accepted={} min_rejected={}",
                    axis.name(),
                    fourcc_text(format),
                    max_ok.map_or("none".into(), |n| format!("1/{n}")),
                    min_rej.map_or("none".into(), |n| format!("1/{n}")),
                );
            }
        }
        Ok(())
    })();
    for fb in buffers {
        let _ = card.destroy_framebuffer(fb.fb);
        let _ = card.destroy_dumb_buffer(fb.dumb);
    }
    outcome?;
    if tested == 0 {
        return Err("no overlay plane to test".into());
    }
    println!(
        "VERDICT downscale_probe: {} {tested} plane(s) probed at dst {}x{}",
        if all_ok {
            "OK"
        } else {
            "FAIL some plane could not be probed:"
        },
        dst.0,
        dst.1,
    );
    Ok(all_ok)
}

fn main() -> ExitCode {
    let options = match parse_args() {
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
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(message) => {
            println!("VERDICT downscale_probe: FAIL {message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ladder_is_the_specified_one() {
        assert_eq!(DEFAULT_RATIOS, [2, 4, 8, 16, 24, 31, 32, 33, 40]);
    }

    #[test]
    fn ratios_parse_in_both_spellings_and_keep_order() {
        assert_eq!(parse_ratios("1/32,1/2,40").unwrap(), vec![32, 2, 40]);
        assert_eq!(parse_ratios(" 1/8 , 4 ").unwrap(), vec![8, 4]);
    }

    #[test]
    fn bad_ratios_are_refused() {
        for bad in [
            "", "1", "1/1", "0", "1/0", "2/3", "x", "1/2,1/2", "1/2,", "-4",
        ] {
            assert!(parse_ratios(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn source_scales_one_axis_only() {
        let big = (8192, 8192);
        assert_eq!(src_size((64, 48), Axis::W, 32, big), Some((2048, 48)));
        assert_eq!(src_size((64, 48), Axis::H, 32, big), Some((64, 1536)));
    }

    #[test]
    fn source_beyond_the_limit_or_overflow_is_none() {
        assert_eq!(src_size((64, 64), Axis::W, 129, (8192, 8192)), None);
        assert_eq!(
            src_size((64, 64), Axis::W, 128, (8192, 8192)),
            Some((8192, 64))
        );
        assert_eq!(src_size((u32::MAX, 1), Axis::W, 2, (u32::MAX, 1)), None);
        // Only the scaled axis grows, but both edges are checked.
        assert_eq!(src_size((64, 64), Axis::H, 2, (64, 100)), None);
    }

    #[test]
    fn fixed_point_is_shifted_by_sixteen() {
        assert_eq!(fixed16((2048, 1)), (2048 << 16, 1 << 16));
    }

    #[test]
    fn buffer_covers_the_largest_source_on_each_axis() {
        assert_eq!(buffer_size((64, 48), &[2, 40], 8192), (2560, 1920));
        assert_eq!(buffer_size((64, 48), &[], 8192), (64, 48));
        // Ratios that do not fit the limit do not enlarge the buffer.
        assert_eq!(buffer_size((64, 64), &[2, 500], 8192), (128, 128));
    }

    #[test]
    fn size_parses() {
        assert_eq!(parse_size("64x32"), Ok((64, 32)));
        assert!(parse_size("0x32").is_err());
        assert!(parse_size("64").is_err());
    }

    #[test]
    fn arguments_set_options() {
        let args = [
            "/dev/dri/card1",
            "--plane",
            "7",
            "--dst",
            "32x16",
            "--ratios",
            "1/32,1/33",
        ]
        .map(String::from);
        let o = parse_arg_list(args).unwrap();
        assert_eq!(o.device, "/dev/dri/card1");
        assert_eq!(o.plane, Some(7));
        assert_eq!(o.dst, (32, 16));
        assert_eq!(o.ratios, vec![32, 33]);
        assert!(parse_arg_list(["--bogus".to_string()]).is_err());
        assert!(parse_arg_list(["--dst".to_string()]).is_err());
    }

    #[test]
    fn status_maps_errno() {
        assert_eq!(status(Ok(())), ("accepted", 0));
        assert_eq!(status(Err(22)), ("rejected", 22));
    }
}
