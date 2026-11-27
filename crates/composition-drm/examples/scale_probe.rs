//! Scaling probe: which scale factors does the kernel's atomic `TEST_ONLY`
//! check accept, plane by plane?
//!
//! ```text
//! cargo run -p composition-drm --example scale_probe -- [/dev/dri/cardN] [options]
//!
//!   --device PATH      DRM device (default /dev/dri/card0); a bare argument works too
//!   --plane ID         test only this plane (default: every plane that can
//!                      drive the chosen CRTC, cursor planes excluded)
//!   --dst WxH          destination size in output pixels, fixed for every test
//!                      (default: half the output, at 0,0)
//!   --resolution N     stop the binary search when the accepted and the
//!                      rejected factor are N steps of 1/65536 apart
//!                      (default 256, about 0.004)
//!   --max-factor N     largest upscale factor to try (default 32). Above 32
//!                      the ladder doubles (64, 128, ...); the factor that
//!                      shrinks the source to one pixel on the axis is always
//!                      tried too, so with `--dst` set to the mode size and a
//!                      large N the search ends at the mode size
//!   --connector ID     connector to use (default: the first connected one)
//! ```
//!
//! The probe opens the device, becomes DRM master, takes the connector's
//! preferred mode and a CRTC for it, and allocates dumb buffers. It then
//! builds atomic requests that are only ever checked with
//! `DRM_MODE_ATOMIC_TEST_ONLY | ALLOW_MODESET`. It never makes a real commit,
//! so the display state is not changed; on exit the buffers, framebuffers and
//! the mode blob are released and nothing needs to be restored.
//!
//! # The tests
//!
//! The destination rectangle is fixed (`--dst`). The source rectangle is
//! derived from a scale factor `f` (destination size divided by source size)
//! with the source size in 16.16 fixed point, so fractional factors are
//! exact to 1/65536 of a pixel. Axes are varied separately (`h`: width
//! only, `v`: height only) and together (`both`).
//!
//! * An overlay plane is tested with a full-screen opaque XRGB-class buffer
//!   on the primary plane below it, and every other plane of the CRTC
//!   switched off in the request.
//! * A primary plane is tested alone.
//! * Upscale: factors 1, 1.25, 1.5, 2, 3, 4, 6, 8, 12, 16, 32 (then doubling
//!   up to `--max-factor`, and the one-pixel-source factor) in ascending
//!   order, stopping at the first rejection. If one is rejected, a binary
//!   search between the last accepted and the first rejected factor runs
//!   down to `--resolution`. It assumes the accepted factors form an
//!   interval starting at 1; the lines show every factor that was tried.
//! * Downscale: 0.5, 0.75 and 0.9 on each axis and on both.
//!
//! # Output
//!
//! ```text
//! DATE / KERNEL / DEVICE / DRIVER / MODE / PLANE lines, as twoplane prints them
//! SCALE plane=42 axis=both src=480.0000x270.0000 dst=960x540 factor=2.0000 test_only=accepted errno=0
//! RESULT plane=42 kind=Overlay max_upscale_h=… max_upscale_v=… max_upscale_both=… downscale=accepted|rejected|partial min_downscale=…
//! VERDICT scale_probe: OK|FAIL …
//! ```
//!
//! `test_only` is `accepted` or `rejected`; `errno` is the errno of a
//! rejection (0 when accepted). `src` is in pixels with the fraction of the
//! 16.16 value. A maximum is `none` if factor 1 itself is rejected, and ends
//! in `+` (`32.0000+`) if the ladder ended without a rejection. `downscale`
//! is `partial` if only some of the factors were accepted;
//! `min_downscale` is the smallest accepted one.
//!
//! The exit status is 0 if every tested plane was probed, 1 if some plane
//! could not be probed (for example factor 1 is rejected) and 2 for a usage
//! error or a device that cannot be used.

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

const USAGE: &str = "usage: scale_probe [/dev/dri/cardN] [--device PATH] [--plane ID] \
[--dst WxH] [--resolution N] [--max-factor N] [--connector ID]";

/// Upscale ladder, tried in ascending order.
const UPSCALES: [f64; 11] = [1.0, 1.25, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 32.0];
const DOWNSCALES: [f64; 3] = [0.5, 0.75, 0.9];
/// 32-bit formats a dumb buffer can hold, in order of preference.
const FORMATS: [DrmFourcc; 4] = [
    DrmFourcc::Xrgb8888,
    DrmFourcc::Argb8888,
    DrmFourcc::Xbgr8888,
    DrmFourcc::Abgr8888,
];

struct Options {
    device: String,
    plane: Option<u32>,
    dst: Option<(u32, u32)>,
    resolution: u32,
    max_factor: u32,
    connector: Option<u32>,
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        device: "/dev/dri/card0".into(),
        plane: None,
        dst: None,
        resolution: 256,
        max_factor: 32,
        connector: None,
    };
    let mut args = std::env::args().skip(1);
    let number = |text: &str| -> Result<u32, String> {
        text.parse().map_err(|_| format!("bad number {text}"))
    };
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--device" => options.device = value("--device")?,
            "--plane" => options.plane = Some(number(&value("--plane")?)?),
            "--connector" => options.connector = Some(number(&value("--connector")?)?),
            "--resolution" => {
                options.resolution = number(&value("--resolution")?)?;
                if options.resolution == 0 {
                    return Err("--resolution must be at least 1".into());
                }
            }
            "--max-factor" => {
                options.max_factor = number(&value("--max-factor")?)?;
                if options.max_factor == 0 {
                    return Err("--max-factor must be at least 1".into());
                }
            }
            "--dst" => {
                let text = value("--dst")?;
                let (w, h) = text.split_once('x').ok_or("--dst takes WxH")?;
                let (w, h) = (number(w)?, number(h)?);
                if w == 0 || h == 0 {
                    return Err("--dst must not be empty".into());
                }
                options.dst = Some((w, h));
            }
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

/// One TEST_ONLY check's outcome: `Ok` if accepted, the errno otherwise.
type Check = Result<(), i32>;

/// What the probe tests against.
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

/// A buffer of the format and size a plane under test needs.
struct Under<'a> {
    plane: &'a PlaneInfo,
    fb: framebuffer::Handle,
    /// Size of the buffer, in pixels.
    fb_size: (u32, u32),
    /// Primary plane and its full-screen buffer, when it is not the plane
    /// under test.
    background: Option<(&'a PlaneInfo, framebuffer::Handle)>,
}

impl Probe<'_> {
    /// Runs one TEST_ONLY check with the source size `src` (16.16 pixels).
    fn check(&self, under: &Under<'_>, src: (u64, u64)) -> Check {
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
        // Everything else on the CRTC's side is switched off in the request.
        for other in self.planes {
            let used = other.handle == under.plane.handle
                || under
                    .background
                    .is_some_and(|(p, _)| p.handle == other.handle);
            if !used {
                req.add_property(
                    other.handle,
                    other.fb_id,
                    property::Value::Framebuffer(None),
                );
                req.add_property(other.handle, other.crtc_id, property::Value::CRTC(None));
            }
        }
        if let Some((primary, fb)) = under.background {
            self.place(
                &mut req,
                primary,
                fb,
                (
                    (
                        u64::from(self.output.0) << 16,
                        u64::from(self.output.1) << 16,
                    ),
                    self.output,
                ),
            );
        }
        self.place(&mut req, under.plane, under.fb, (src, self.dst));
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
        (src, dst): ((u64, u64), (u32, u32)),
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    H,
    V,
    Both,
}

impl Axis {
    fn name(self) -> &'static str {
        match self {
            Axis::H => "h",
            Axis::V => "v",
            Axis::Both => "both",
        }
    }
}

/// A scale factor in 16.16 fixed point.
fn fixed(factor: f64) -> u64 {
    (factor * 65536.0).round() as u64
}

fn factor_text(factor16: u64) -> String {
    format!("{:.4}", factor16 as f64 / 65536.0)
}

fn pixels16(value: u64) -> String {
    format!("{:.4}", value as f64 / 65536.0)
}

/// Runs one test at `factor16` (dst / src, 16.16) on `axis`, printing its
/// `SCALE` line. Returns the outcome, or `None` if the source would be
/// smaller than one pixel or larger than the buffer (nothing was tested).
fn test(probe: &Probe<'_>, under: &Under<'_>, axis: Axis, factor16: u64) -> Option<Check> {
    let scaled = |dst: u32, on: bool| {
        if on {
            // dst / factor in 16.16, rounded to nearest.
            ((u64::from(dst) << 32) + factor16 / 2) / factor16
        } else {
            u64::from(dst) << 16
        }
    };
    let src = (
        scaled(probe.dst.0, axis != Axis::V),
        scaled(probe.dst.1, axis != Axis::H),
    );
    let fits = |v: u64, limit: u32| v >= 1 << 16 && v <= u64::from(limit) << 16;
    if !fits(src.0, under.fb_size.0) || !fits(src.1, under.fb_size.1) {
        return None;
    }
    let outcome = probe.check(under, src);
    let (status, errno) = match outcome {
        Ok(()) => ("accepted", 0),
        Err(errno) => ("rejected", errno),
    };
    println!(
        "SCALE plane={} axis={} src={}x{} dst={}x{} factor={} test_only={status} errno={errno}",
        u32::from(under.plane.handle),
        axis.name(),
        pixels16(src.0),
        pixels16(src.1),
        probe.dst.0,
        probe.dst.1,
        factor_text(factor16),
    );
    Some(outcome)
}

/// The largest accepted upscale on `axis`, as text, and whether factor 1
/// itself was accepted.
fn upscale(
    probe: &Probe<'_>,
    under: &Under<'_>,
    axis: Axis,
    resolution: u32,
    max_factor: u32,
) -> (String, bool) {
    let mut accepted: Option<u64> = None;
    let mut rejected: Option<u64> = None;
    let ladder = ladder(probe.dst, axis, max_factor);
    for &factor in &ladder {
        let factor16 = fixed(factor);
        match test(probe, under, axis, factor16) {
            Some(Ok(())) => accepted = Some(factor16),
            Some(Err(_)) => {
                rejected = Some(factor16);
                break;
            }
            None => break,
        }
    }
    let Some(mut low) = accepted else {
        return ("none".into(), false);
    };
    let Some(mut high) = rejected else {
        // No rejection: the ladder ended, or the buffer could not go further.
        println!(
            "NOTE plane={} axis={} no rejection up to {} ({})",
            u32::from(under.plane.handle),
            axis.name(),
            factor_text(low),
            ladder_end(probe.dst, axis, max_factor, low),
        );
        return (format!("{}+", factor_text(low)), true);
    };
    while high - low > u64::from(resolution) {
        let mid = (low + high) / 2;
        match test(probe, under, axis, mid) {
            Some(Ok(())) => low = mid,
            _ => high = mid,
        }
    }
    (factor_text(low), true)
}

/// The largest factor that keeps the source at least one pixel on `axis`.
fn one_pixel_factor(dst: (u32, u32), axis: Axis) -> u32 {
    match axis {
        Axis::H => dst.0,
        Axis::V => dst.1,
        Axis::Both => dst.0.min(dst.1),
    }
}

/// Upscale factors to try on `axis`, ascending: the base ladder, doublings
/// above it, and the one-pixel-source factor, all up to `max_factor`.
fn ladder(dst: (u32, u32), axis: Axis, max_factor: u32) -> Vec<f64> {
    let cap = f64::from(max_factor.min(one_pixel_factor(dst, axis)));
    let mut factors: Vec<f64> = UPSCALES.into_iter().filter(|f| *f <= cap).collect();
    let mut next = 64.0;
    while next <= cap {
        factors.push(next);
        next *= 2.0;
    }
    if factors.last().is_none_or(|last| *last < cap) {
        factors.push(cap);
    }
    factors
}

/// Why an upscale ladder that ended without a rejection ended where it did.
fn ladder_end(dst: (u32, u32), axis: Axis, max_factor: u32, reached: u64) -> &'static str {
    let one_pixel = one_pixel_factor(dst, axis);
    if reached < fixed(f64::from(max_factor.min(one_pixel))) {
        "the source buffer or a source below one pixel stopped the ladder"
    } else if one_pixel <= max_factor {
        "limited by the mode size: the source is one pixel"
    } else {
        "limited by --max-factor"
    }
}

/// Tests the downscales on all axes. Returns `accepted`, `rejected` or
/// `partial`, and the smallest accepted factor.
fn downscale(probe: &Probe<'_>, under: &Under<'_>) -> (&'static str, String) {
    let (mut ok, mut bad) = (0, 0);
    let mut smallest: Option<u64> = None;
    for axis in [Axis::H, Axis::V, Axis::Both] {
        for factor in DOWNSCALES {
            let factor16 = fixed(factor);
            match test(probe, under, axis, factor16) {
                Some(Ok(())) => {
                    ok += 1;
                    smallest = Some(smallest.map_or(factor16, |s| s.min(factor16)));
                }
                Some(Err(_)) => bad += 1,
                None => {}
            }
        }
    }
    let verdict = match (ok, bad) {
        (0, 0) => "untested",
        (_, 0) => "accepted",
        (0, _) => "rejected",
        _ => "partial",
    };
    (verdict, smallest.map_or("none".into(), factor_text))
}

/// Prefixes an I/O error with what was being done.
fn fail(what: &str) -> impl Fn(io::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

fn pick_format(plane: &PlaneInfo) -> Option<DrmFourcc> {
    FORMATS
        .into_iter()
        .find(|f| plane.formats.contains(&(*f as u32)))
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
            "DRIVER {} version {}.{}.{} date {} desc {}",
            info.name().to_string_lossy(),
            info.version.0,
            info.version.1,
            info.version.2,
            info.date().to_string_lossy(),
            info.description().to_string_lossy()
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

    // Connector, preferred mode, CRTC.
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
    let dst = options
        .dst
        .unwrap_or(((output.0 / 2).max(1), (output.1 / 2).max(1)));
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

    // Planes.
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
            "PLANE id {} {:?} possible_on_crtc {} formats {}",
            u32::from(p.handle),
            p.kind,
            p.possible,
            p.formats
                .iter()
                .map(|f| match DrmFourcc::try_from(*f) {
                    Ok(f) => fourcc_text(f),
                    Err(_) => format!("{f:#x}"),
                })
                .collect::<Vec<_>>()
                .join(",")
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

    let result = probe_planes(&probe, options, &planes, output);
    // Nothing was committed, so only the mode blob is left to drop.
    let _ = card.destroy_property_blob(mode_blob);
    result
}

fn probe_planes(
    probe: &Probe<'_>,
    options: &Options,
    planes: &[PlaneInfo],
    output: (u32, u32),
) -> Result<bool, String> {
    let card = probe.card;
    let dst = probe.dst;
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
        // The opaque full-screen background, on the primary plane.
        let background = match primary {
            Some(primary) => {
                let format =
                    pick_format(primary).ok_or("primary plane lists no 32-bit RGB format")?;
                let fb = make_fb(card, output, format)
                    .map_err(|e| format!("background buffer {}: {e}", fourcc_text(format)))?;
                let handle = fb.fb;
                buffers.push(fb);
                Some((primary, handle))
            }
            None => None,
        };
        for plane in planes {
            if plane.kind == PlaneType::Cursor
                || !plane.possible
                || options
                    .plane
                    .is_some_and(|id| id != u32::from(plane.handle))
            {
                continue;
            }
            let Some(format) = pick_format(plane) else {
                println!(
                    "SKIP plane={} no 32-bit RGB format",
                    u32::from(plane.handle)
                );
                continue;
            };
            // Large enough for the 2x source of the 0.5 downscale; if the
            // driver refuses a buffer that big (a full-screen `--dst`), fall
            // back to the destination size: upscales still run, downscales
            // that do not fit are not tested.
            let doubled = (dst.0.saturating_mul(2), dst.1.saturating_mul(2));
            let (size, fb) = match make_fb(card, doubled, format) {
                Ok(fb) => (doubled, fb),
                Err(e) => {
                    println!(
                        "NOTE plane={} buffer {}x{} {} refused ({e}); using {}x{}",
                        u32::from(plane.handle),
                        doubled.0,
                        doubled.1,
                        fourcc_text(format),
                        dst.0,
                        dst.1
                    );
                    let fb = make_fb(card, dst, format).map_err(|e| {
                        format!("buffer {}x{} {}: {e}", dst.0, dst.1, fourcc_text(format))
                    })?;
                    (dst, fb)
                }
            };
            let fb_handle = fb.fb;
            buffers.push(fb);
            let under = Under {
                plane,
                fb: fb_handle,
                fb_size: size,
                // An overlay needs a visible primary below it; the primary
                // plane is tested alone.
                background: if plane.kind == PlaneType::Primary {
                    None
                } else {
                    background
                },
            };
            tested += 1;
            let (max_h, ok_h) = upscale(
                probe,
                &under,
                Axis::H,
                options.resolution,
                options.max_factor,
            );
            let (max_v, ok_v) = upscale(
                probe,
                &under,
                Axis::V,
                options.resolution,
                options.max_factor,
            );
            let (max_both, ok_both) = upscale(
                probe,
                &under,
                Axis::Both,
                options.resolution,
                options.max_factor,
            );
            let (down, min_down) = downscale(probe, &under);
            all_ok &= ok_h && ok_v && ok_both;
            println!(
                "RESULT plane={} kind={:?} format={} max_upscale_h={max_h} max_upscale_v={max_v} \
                 max_upscale_both={max_both} downscale={down} min_downscale={min_down}",
                u32::from(plane.handle),
                plane.kind,
                fourcc_text(format),
            );
        }
        Ok(())
    })();
    for fb in buffers {
        let _ = card.destroy_framebuffer(fb.fb);
        let _ = card.destroy_dumb_buffer(fb.dumb);
    }
    outcome?;
    if tested == 0 {
        return Err("no plane to test".into());
    }
    println!(
        "VERDICT scale_probe: {} {tested} plane(s) probed at dst {}x{}, resolution {}/65536",
        if all_ok {
            "OK"
        } else {
            "FAIL some plane rejects factor 1 or the baseline:"
        },
        dst.0,
        dst.1,
        options.resolution
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
            println!("VERDICT scale_probe: FAIL {message}");
            ExitCode::from(2)
        }
    }
}
