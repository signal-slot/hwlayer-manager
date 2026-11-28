//! Flip probe: how an overlay plane flips, and what the flip events say.
//!
//! ```text
//! cargo run -p composition-drm --example flip_probe -- [options]
//!
//!   --device PATH      DRM device (default /dev/dri/card0); a bare argument works too
//!   --connector ID     connector to use (default: the first connected one)
//!   --plane ID         overlay plane to flip (default: the first usable overlay on the CRTC)
//!   --frames N         number of flips to attempt (default 120)
//!   --size WxH         overlay buffer and window size, shrunk to fit the mode (default 640x360)
//!   --timeout-ms N     event wait per flip (default 200)
//! ```
//!
//! The program opens the device, becomes DRM master (or exits with status 2 saying it could
//! not; it never stops other processes), places one overlay plane at a fixed window with two
//! solid-colour dumb buffers of its own, and flips the overlay between them `--frames` times.
//! The primary plane, the connector, the mode and the CRTC state are not touched, so a
//! connector whose CRTC is not running is refused (`flip_probe` never modesets). Each flip is
//! a non-blocking atomic commit with `PAGE_FLIP_EVENT` that changes only the overlay
//! `FB_ID`; the next flip is sent only after that CRTC's page-flip event arrived, because a
//! flip sent while the previous one has not latched is rejected (`kms_sweep replace`
//! measures that rejection). The commit that adds the overlay is blocking, with
//! `PAGE_FLIP_EVENT` as well. At the end every property the program changed on the overlay
//! is committed back from the value it had at the start, the overlay is off again, and the
//! buffers are destroyed: the board is left as it was found.
//!
//! Use it on drivers whose two-plane harness cannot run: the harness needs a *primary* plane
//! that lists a linear RGB format, and on RK3588 (VOP2) no primary on the connected output
//! does (`twoplane` prints `FAIL no primary plane with a linear 32-bit RGB format`). This
//! tool needs no linear primary: it flips only an overlay.
//!
//! provenance: synthetic — this file holds no recorded board data; the unit tests build their
//! own fixtures.
//!
//! # Output
//!
//! ```text
//! DATE / KERNEL / DEVICE / DRIVER / MODE lines
//! PRIMARY plane=33
//! OVERLAY plane=45 buffers=2x640x360 dst=640,360,640x360 zpos=2
//! SETUP commit=ok events=1
//! FLIPS frames=120 committed=120 events=120 missed=0 late=0 errors=0 first_error=none drain=0 aborted=0
//! LATENCY samples=120 target_us=16666 min_us=16200 avg_us=16680 max_us=17300
//! RESTORE ok
//! VERDICT flip_probe: PASS
//! ```
//!
//! `SETUP` and `RESTORE` are `ok` or an errno. `missed` counts accepted flips whose event
//! did not arrive before `--timeout-ms`; for each, one more wait of double the timeout is
//! made, and a late event that arrives then counts in `late` too. `drain` counts page-flip
//! events of the CRTC that arrive after the last flip's wait. `errors` counts rejected
//! flips and `first_error` names the first errno; after three errors in a row the loop
//! stops (`aborted=1`). `LATENCY` times run from a commit to the arrival of its own event;
//! `target_us` is the refresh period of the running mode (`0` when it cannot be computed),
//! and a flip whose latency is more than 1.5 times `target_us` is `late`.
//!
//! `VERDICT flip_probe: PASS` when every flip was accepted, every accepted flip had its
//! event, none was missed, nothing was left in `drain` and the loop was not aborted;
//! otherwise `FAIL ...`. Exit status: 0 for `PASS`, 1 for `FAIL`, 2 for a usage error, a
//! device that cannot be used, or a session that could not run (`VERDICT flip_probe: FAIL
//! <reason>`).

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use composition_drm::Card;
use drm::buffer::{Buffer, DrmFourcc, DrmModifier, PlanarBuffer};
use drm::control::atomic::AtomicModeReq;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    connector, crtc, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice,
    Event, FbCmd2Flags, PlaneType, ResourceHandle,
};
use drm::{ClientCapability, Device as BasicDevice};

const USAGE: &str = "usage: flip_probe [--device PATH] [--connector ID] [--plane ID] \
[--frames N] [--size WxH] [--timeout-ms N]";

/// The properties the program writes, and so restores.
const TOUCHED: [&str; 11] = [
    "FB_ID", "CRTC_ID", "SRC_X", "SRC_Y", "SRC_W", "SRC_H", "CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H",
    "zpos",
];

#[derive(Debug, PartialEq, Eq)]
struct Options {
    device: String,
    connector: Option<u32>,
    plane: Option<u32>,
    frames: u32,
    size: (u32, u32),
    timeout_ms: u32,
}

fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let (w, h) = text
        .split_once('x')
        .ok_or_else(|| format!("--size wants WxH, got {text:?}"))?;
    let w = w
        .parse::<u32>()
        .map_err(|_| format!("--size width is not a number: {text:?}"))?;
    let h = h
        .parse::<u32>()
        .map_err(|_| format!("--size height is not a number: {text:?}"))?;
    if w == 0 || h == 0 {
        return Err(format!("--size sides must be at least 1: {text:?}"));
    }
    Ok((w, h))
}

fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
    let mut options = Options {
        device: "/dev/dri/card0".into(),
        connector: None,
        plane: None,
        frames: 120,
        size: (640, 360),
        timeout_ms: 200,
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
            "--plane" => {
                let v = value("--plane")?;
                options.plane = Some(v.parse().map_err(|_| format!("bad plane id {v:?}"))?);
            }
            "--frames" => {
                let v = value("--frames")?;
                let frames = v.parse::<u32>().map_err(|_| format!("bad frames {v:?}"))?;
                if frames == 0 {
                    return Err("--frames must be at least 1".into());
                }
                options.frames = frames;
            }
            "--size" => options.size = parse_size(&value("--size")?)?,
            "--timeout-ms" => {
                let v = value("--timeout-ms")?;
                options.timeout_ms = v.parse().map_err(|_| format!("bad timeout {v:?}"))?;
            }
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => options.device = other.to_string(),
        }
    }
    Ok(options)
}

fn errno_of(error: &io::Error) -> i32 {
    error.raw_os_error().unwrap_or(-1)
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

/// Refresh period of a mode in microseconds, from its totals and pixel clock
/// (kHz). `None` when the clock is zero.
fn period_us(htotal: u32, vtotal: u32, clock_khz: u32) -> Option<u64> {
    if clock_khz == 0 || htotal == 0 || vtotal == 0 {
        return None;
    }
    Some(u64::from(htotal) * u64::from(vtotal) * 1_000 / u64::from(clock_khz))
}

fn is_late(sample_us: u128, target_us: Option<u64>) -> bool {
    target_us.is_some_and(|t| sample_us * 2 > u128::from(t) * 3)
}

/// (min, avg, max) of the samples in microseconds; `None` for no samples.
fn latency_stats(samples: &[u128]) -> Option<(u128, u128, u128)> {
    if samples.is_empty() {
        return None;
    }
    let mut min = u128::MAX;
    let mut max = 0;
    let mut sum = 0u128;
    for s in samples {
        min = min.min(*s);
        max = max.max(*s);
        sum += s;
    }
    Some((min, sum / samples.len() as u128, max))
}

/// The zpos to write on the overlay: not below its own current value, one
/// above the primary's when the primary has a zpos, clamped to the range.
fn pick_zpos(primary_zpos: Option<u64>, current: u64, min: u64, max: u64) -> u64 {
    let want = primary_zpos.map_or(current, |p| p + 1).max(current);
    want.clamp(min, max)
}

/// Window position centred on the output for a layer of `layer` pixels.
fn centre(output: (u32, u32), layer: (u32, u32)) -> (u32, u32) {
    (
        output.0.saturating_sub(layer.0) / 2,
        output.1.saturating_sub(layer.1) / 2,
    )
}

fn flips_line(o: &FlipCount, frames: u32) -> String {
    format!(
        "FLIPS frames={frames} committed={} events={} missed={} late={} errors={} first_error={} drain={} aborted={}",
        o.committed,
        o.events,
        o.missed,
        o.late,
        o.errors,
        o.first_error.map_or("none".into(), errno_name),
        o.drain,
        u8::from(o.aborted),
    )
}

fn latency_line(samples: &[u128], target_us: Option<u64>) -> String {
    match latency_stats(samples) {
        Some((min, avg, max)) => format!(
            "LATENCY samples={} target_us={} min_us={min} avg_us={avg} max_us={max}",
            samples.len(),
            target_us.unwrap_or(0),
        ),
        None => format!("LATENCY samples=0 target_us={}", target_us.unwrap_or(0)),
    }
}

/// Every flip accepted, every accepted flip had its own event, nothing
/// missed, nothing left over, no abort.
fn flips_passed(o: &FlipCount, frames: u32) -> bool {
    !o.aborted
        && o.errors == 0
        && o.missed == 0
        && o.drain == 0
        && o.committed == u64::from(frames)
        && o.events == o.committed
}

fn verdict_text(passed: bool) -> &'static str {
    if passed {
        "VERDICT flip_probe: PASS"
    } else {
        "VERDICT flip_probe: FAIL see the FLIPS line"
    }
}

/// Counters of the flip loop.
#[derive(Debug, Default)]
struct FlipCount {
    committed: u64,
    events: u64,
    missed: u64,
    late: u64,
    errors: u64,
    first_error: Option<i32>,
    drain: u64,
    aborted: bool,
}

#[derive(Clone, Copy)]
struct Prop {
    handle: property::Handle,
    value: u64,
    mutable: bool,
}

type Props = HashMap<String, Prop>;

fn named_properties<H: ResourceHandle>(card: &Card, handle: H) -> io::Result<Props> {
    let set = card.get_properties(handle)?;
    let mut named = HashMap::new();
    for (prop, value) in set.iter() {
        let info = card.get_property(*prop)?;
        named.insert(
            info.name().to_string_lossy().into_owned(),
            Prop {
                handle: *prop,
                value: *value,
                mutable: info.mutable(),
            },
        );
    }
    Ok(named)
}

fn prop(props: &Props, name: &str) -> Result<Prop, String> {
    props
        .get(name)
        .copied()
        .ok_or_else(|| format!("no property {name}"))
}

fn fail(what: &str) -> impl Fn(io::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

/// The fourcc of the dumb buffer a plane should get: XRGB8888 when the plane
/// lists it, else ARGB8888 when it lists that, else nothing.
fn choose_fourcc(formats: &[u32]) -> Option<DrmFourcc> {
    if formats.contains(&(DrmFourcc::Xrgb8888 as u32)) {
        Some(DrmFourcc::Xrgb8888)
    } else if formats.contains(&(DrmFourcc::Argb8888 as u32)) {
        Some(DrmFourcc::Argb8888)
    } else {
        None
    }
}

/// The fourcc codes in an `IN_FORMATS` blob (`formats_offset`, count at
/// offset 8..16 of `struct drm_format_modifier_blob`), ignoring modifiers.
fn in_formats_fourcc(blob: &[u8]) -> Option<Vec<u32>> {
    let u32_at = |offset: usize| {
        blob.get(offset..offset + 4)
            .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
    };
    let (count, offset) = (u32_at(8)?, u32_at(12)?);
    let mut codes = Vec::new();
    for i in 0..count {
        codes.push(u32_at(offset as usize + usize::try_from(i).ok()? * 4)?);
    }
    Some(codes)
}

fn fill_solid(bytes: &mut [u8], pitch: usize, width: u32, height: u32, rgba: [u8; 4]) {
    for y in 0..height as usize {
        for x in 0..width as usize {
            for (k, value) in rgba.iter().enumerate() {
                let at = y * pitch + x * 4 + k;
                if at < bytes.len() {
                    bytes[at] = *value;
                }
            }
        }
    }
}

/// A dumb buffer (XRGB8888 storage) presented to the kernel as `fourcc`.
struct Fb {
    dumb: DumbBuffer,
    fb: framebuffer::Handle,
}

struct Layout {
    size: (u32, u32),
    format: DrmFourcc,
    pitches: [u32; 4],
    offsets: [u32; 4],
    handles: [Option<drm::buffer::Handle>; 4],
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
        self.pitches
    }
    fn handles(&self) -> [Option<drm::buffer::Handle>; 4] {
        self.handles
    }
    fn offsets(&self) -> [u32; 4] {
        self.offsets
    }
}

fn make_fb(card: &Card, size: (u32, u32), fourcc: DrmFourcc, rgba: [u8; 4]) -> Result<Fb, String> {
    let (width, height) = size;
    let mut dumb = card
        .create_dumb_buffer((width, height), DrmFourcc::Xrgb8888, 32)
        .map_err(|e| format!("dumb buffer: {}", errno_name(errno_of(&e))))?;
    let pitch = dumb.pitch();
    let handle = dumb.handle();
    let layout = Layout {
        size,
        format: fourcc,
        pitches: [pitch, 0, 0, 0],
        offsets: [0; 4],
        handles: [Some(handle), None, None, None],
    };
    (|| {
        let mut mapping = card.map_dumb_buffer(&mut dumb).map_err(fail("map"))?;
        fill_solid(mapping.as_mut(), pitch as usize, width, height, rgba);
        Ok::<(), String>(())
    })()?;
    let fb = match card.add_planar_framebuffer(&layout, FbCmd2Flags::empty()) {
        Ok(fb) => fb,
        Err(error) => {
            let _ = card.destroy_dumb_buffer(dumb);
            return Err(format!("addfb: {}", errno_name(errno_of(&error))));
        }
    };
    Ok(Fb { dumb, fb })
}

fn release(card: &Card, fb: Fb) {
    let _ = card.destroy_framebuffer(fb.fb);
    let _ = card.destroy_dumb_buffer(fb.dumb);
}

struct Overlay {
    handle: plane::Handle,
    props: Props,
    /// The mutable `zpos` property and its range: `None` when the plane has
    /// no writable zpos.
    zpos: Option<(Prop, u64, u64)>,
}

impl Overlay {
    fn id(&self) -> u32 {
        u32::from(self.handle)
    }
}

struct Ctx {
    card: Card,
    crtc: crtc::Handle,
    output: (u32, u32),
    target_us: Option<u64>,
    primary_id: u32,
    primary_zpos: Option<u64>,
    overlay: Overlay,
    /// (property handle, value) of every mutable property on the overlay,
    /// read before anything was touched.
    saved: Vec<(property::Handle, u64)>,
}

fn open(options: &Options) -> Result<Ctx, String> {
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
             holds it; stop it for the run. This tool never stops other processes"
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
    let crtc = info
        .current_encoder()
        .map(|encoder| card.get_encoder(encoder).map_err(fail("encoder")))
        .transpose()?
        .and_then(|encoder| encoder.crtc())
        .ok_or("the connector has no running CRTC; flip_probe never modesets")?;
    let crtc_info = card.get_crtc(crtc).map_err(fail("crtc"))?;
    let crtc_props = named_properties(&card, crtc).map_err(fail("crtc props"))?;
    let active = prop(&crtc_props, "ACTIVE")?;
    if active.value == 0 {
        return Err("the CRTC is not active; flip_probe never modesets".into());
    }
    let mode = crtc_info.mode().ok_or("no running mode")?;
    let (mw, mh) = mode.size();
    let output = (u32::from(mw), u32::from(mh));
    let (_, _, htotal) = mode.hsync();
    let (_, _, vtotal) = mode.vsync();
    let target_us = period_us(u32::from(htotal), u32::from(vtotal), mode.clock());
    println!(
        "MODE {}x{}@{}Hz connector {} crtc {} (running mode, no modeset)",
        output.0,
        output.1,
        mode.vrefresh(),
        u32::from(connector),
        u32::from(crtc),
    );

    // Planes usable on this CRTC, split into primaries and overlays.
    let mut primaries: Vec<(plane::Handle, Props)> = Vec::new();
    let mut overlays: Vec<(plane::Handle, Props)> = Vec::new();
    let mut handles = card.plane_handles().map_err(fail("planes"))?;
    handles.sort_by_key(|h| u32::from(*h));
    for handle in handles {
        let pinfo = card.get_plane(handle).map_err(fail("plane"))?;
        if !resources
            .filter_crtcs(pinfo.possible_crtcs())
            .contains(&crtc)
        {
            continue;
        }
        let props = named_properties(&card, handle).map_err(fail("plane props"))?;
        let kind = match props.get("type").map(|p| p.value as u32) {
            Some(v) if v == PlaneType::Primary as u32 => PlaneType::Primary,
            Some(v) if v == PlaneType::Cursor as u32 => PlaneType::Cursor,
            _ => PlaneType::Overlay,
        };
        if kind == PlaneType::Cursor {
            continue;
        }
        if kind == PlaneType::Primary {
            primaries.push((handle, props));
        } else {
            overlays.push((handle, props));
        }
    }
    let primary = primaries
        .iter()
        .find(|(_, props)| {
            props
                .get("CRTC_ID")
                .is_some_and(|p| p.value == u64::from(u32::from(crtc)))
        })
        .or_else(|| primaries.first())
        .ok_or("no primary plane for the CRTC")?;
    let primary_zpos = prop(&primary.1, "zpos").ok().map(|p| p.value);
    let primary_id = u32::from(primary.0);

    let chosen = match options.plane {
        Some(id) => overlays
            .iter()
            .find(|(handle, _)| u32::from(*handle) == id)
            .ok_or(format!("plane {id} is not an overlay usable on this CRTC"))?,
        None => overlays
            .iter()
            .find(|(_, props)| fourccs(&card, props).is_some_and(|c| choose_fourcc(&c).is_some()))
            .ok_or("no overlay plane on this CRTC lists XRGB8888 or ARGB8888")?,
    };
    let overlay_props = chosen.1.clone();
    let zpos = prop(&overlay_props, "zpos")
        .ok()
        .filter(|p| p.mutable)
        .and_then(|p| {
            let info = card.get_property(p.handle).ok()?;
            match info.value_type() {
                property::ValueType::UnsignedRange(min, max) if info.mutable() => {
                    Some((p, *min, *max))
                }
                _ => None,
            }
        });
    let saved = TOUCHED
        .into_iter()
        .filter_map(|name| overlay_props.get(name))
        .filter(|p| p.mutable)
        .map(|p| (p.handle, p.value))
        .collect();
    Ok(Ctx {
        card,
        crtc,
        output,
        target_us,
        primary_id,
        primary_zpos,
        overlay: Overlay {
            handle: chosen.0,
            props: overlay_props,
            zpos,
        },
        saved,
    })
}

/// The fourcc codes a plane advertises: from `IN_FORMATS` when it has the
/// blob, else from the legacy plane format list.
fn fourccs(card: &Card, props: &Props) -> Option<Vec<u32>> {
    let blob = props.get("IN_FORMATS")?.value;
    if blob == 0 {
        return None;
    }
    let blob = card.get_property_blob(blob).ok()?;
    in_formats_fourcc(&blob)
}

/// Page-flip event of `crtc` that arrives before `t0 + timeout`, measured from
/// `t0`. `Ok(None)` when none arrived.
fn wait_flip(
    card: &Card,
    crtc: crtc::Handle,
    t0: Instant,
    timeout: Duration,
) -> Result<Option<Duration>, String> {
    use rustix::event::{poll, PollFd, PollFlags, Timespec};
    let deadline = t0 + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(None);
        }
        let ns = Timespec {
            tv_sec: left.as_secs() as _,
            tv_nsec: left.subsec_nanos() as _,
        };
        let mut fds = [PollFd::new(card, PollFlags::IN)];
        match poll(&mut fds, Some(&ns)) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(format!("poll: {error}")),
        }
        if !fds[0].revents().contains(PollFlags::IN) {
            continue;
        }
        for event in card.receive_events().map_err(fail("events"))? {
            if let Event::PageFlip(flip) = event {
                if flip.crtc == crtc {
                    return Ok(Some(t0.elapsed()));
                }
            }
        }
    }
}

/// Page-flip events of `crtc` that arrive before `timeout` from now.
fn drain_events(card: &Card, crtc: crtc::Handle, timeout: Duration) -> Result<u64, String> {
    use rustix::event::{poll, PollFd, PollFlags, Timespec};
    let t0 = Instant::now();
    let deadline = t0 + timeout;
    let mut seen = 0;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(seen);
        }
        let ns = Timespec {
            tv_sec: left.as_secs() as _,
            tv_nsec: left.subsec_nanos() as _,
        };
        let mut fds = [PollFd::new(card, PollFlags::IN)];
        match poll(&mut fds, Some(&ns)) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(format!("poll: {error}")),
        }
        if !fds[0].revents().contains(PollFlags::IN) {
            continue;
        }
        for event in card.receive_events().map_err(fail("events"))? {
            if let Event::PageFlip(flip) = event {
                if flip.crtc == crtc {
                    seen += 1;
                }
            }
        }
    }
}

fn place(
    ctx: &Ctx,
    req: &mut AtomicModeReq,
    fb: framebuffer::Handle,
    src: (u32, u32),
    dst: (u32, u32, u32, u32),
    zpos: Option<u64>,
) -> Result<(), String> {
    let p = |name: &str| prop(&ctx.overlay.props, name).map(|p| p.handle);
    let h = ctx.overlay.handle;
    req.add_property(h, p("FB_ID")?, property::Value::Framebuffer(Some(fb)));
    req.add_property(h, p("CRTC_ID")?, property::Value::CRTC(Some(ctx.crtc)));
    let source = [0, 0, u64::from(src.0) << 16, u64::from(src.1) << 16];
    for (name, v) in ["SRC_X", "SRC_Y", "SRC_W", "SRC_H"].into_iter().zip(source) {
        req.add_property(h, p(name)?, property::Value::UnsignedRange(v));
    }
    req.add_property(
        h,
        p("CRTC_X")?,
        property::Value::SignedRange(i64::from(dst.0)),
    );
    req.add_property(
        h,
        p("CRTC_Y")?,
        property::Value::SignedRange(i64::from(dst.1)),
    );
    req.add_property(
        h,
        p("CRTC_W")?,
        property::Value::UnsignedRange(u64::from(dst.2)),
    );
    req.add_property(
        h,
        p("CRTC_H")?,
        property::Value::UnsignedRange(u64::from(dst.3)),
    );
    if let Some(value) = zpos {
        req.add_property(
            h,
            ctx.overlay
                .zpos
                .as_ref()
                .ok_or("no zpos property")?
                .0
                .handle,
            property::Value::UnsignedRange(value),
        );
    }
    Ok(())
}

fn commit(card: &Card, flags: AtomicCommitFlags, req: AtomicModeReq) -> Result<(), i32> {
    card.atomic_commit(flags, req).map_err(|e| errno_of(&e))
}

fn restore(ctx: &Ctx) -> Result<(), i32> {
    let mut req = AtomicModeReq::new();
    for (handle, value) in &ctx.saved {
        req.add_property(
            ctx.overlay.handle,
            *handle,
            property::Value::Unknown(*value),
        );
    }
    commit(&ctx.card, AtomicCommitFlags::empty(), req)
}

fn run(options: &Options) -> Result<bool, String> {
    let ctx = open(options)?;
    println!("PRIMARY plane={}", ctx.primary_id);

    let fourcc = fourccs(&ctx.card, &ctx.overlay.props)
        .and_then(|c| choose_fourcc(&c))
        .unwrap_or(DrmFourcc::Xrgb8888);
    let size = (
        options.size.0.min(ctx.output.0),
        options.size.1.min(ctx.output.1),
    );
    let at = centre(ctx.output, size);
    let zpos = ctx
        .overlay
        .zpos
        .as_ref()
        .map(|(p, min, max)| pick_zpos(ctx.primary_zpos, p.value, *min, *max));
    let zpos_text = zpos.map_or("kept".into(), |v| v.to_string());
    println!(
        "OVERLAY plane={} buffers={}x{}x{} dst={},{},{}x{} zpos={zpos_text}",
        ctx.overlay.id(),
        2,
        size.0,
        size.1,
        at.0,
        at.1,
        size.0,
        size.1,
    );

    let mut buffers = Vec::new();
    let mut fbs = Vec::new();
    for rgba in [[0xE0u8, 0x30, 0x30, 0xFF], [0x20, 0x60, 0xD0, 0xFF]] {
        let fb = make_fb(&ctx.card, size, fourcc, rgba)?;
        fbs.push(fb.fb);
        buffers.push(fb);
    }

    // The setup commit: overlay on, blocking, with a flip event.
    let mut req = AtomicModeReq::new();
    place(
        &ctx,
        &mut req,
        fbs[0],
        size,
        (at.0, at.1, size.0, size.1),
        zpos,
    )?;
    let t0 = Instant::now();
    match commit(&ctx.card, AtomicCommitFlags::PAGE_FLIP_EVENT, req) {
        Ok(()) => {
            let events = wait_flip(
                &ctx.card,
                ctx.crtc,
                t0,
                Duration::from_millis(u64::from(options.timeout_ms) + 1000),
            )?;
            println!("SETUP commit=ok events={}", u64::from(events.is_some()));
        }
        Err(errno) => {
            println!("SETUP commit={}", errno_name(errno));
            println!("{}", result_restore(&ctx));
            let _ = drain_events(&ctx.card, ctx.crtc, Duration::from_millis(200));
            for fb in buffers {
                release(&ctx.card, fb);
            }
            return Err(format!("setup commit: {}", errno_name(errno)));
        }
    }

    // The flip loop: alternate the overlay's framebuffer, one event each.
    let timeout = Duration::from_millis(u64::from(options.timeout_ms));
    let mut count = FlipCount::default();
    let mut samples: Vec<u128> = Vec::new();
    let mut consecutive = 0;
    for i in 0..options.frames {
        // Start on the buffer that is not already scanned out by the setup.
        let fb = if i % 2 == 0 { fbs[1] } else { fbs[0] };
        let mut req = AtomicModeReq::new();
        req.add_property(
            ctx.overlay.handle,
            prop(&ctx.overlay.props, "FB_ID")?.handle,
            property::Value::Framebuffer(Some(fb)),
        );
        let t0 = Instant::now();
        match commit(
            &ctx.card,
            AtomicCommitFlags::NONBLOCK | AtomicCommitFlags::PAGE_FLIP_EVENT,
            req,
        ) {
            Ok(()) => {
                count.committed += 1;
                consecutive = 0;
                match wait_flip(&ctx.card, ctx.crtc, t0, timeout)? {
                    Some(elapsed) => {
                        let us = elapsed.as_micros();
                        samples.push(us);
                        count.events += 1;
                        if is_late(us, ctx.target_us) {
                            count.late += 1;
                        }
                    }
                    None => {
                        count.missed += 1;
                        if let Some(elapsed) = wait_flip(&ctx.card, ctx.crtc, t0, timeout * 2)? {
                            let us = elapsed.as_micros();
                            samples.push(us);
                            count.events += 1;
                            count.late += 1;
                        }
                    }
                }
            }
            Err(errno) => {
                count.errors += 1;
                *count.first_error.get_or_insert(errno) = errno;
                consecutive += 1;
                if consecutive >= 3 {
                    count.aborted = true;
                    break;
                }
                // Let a pending flip finish before trying again.
                let _ = wait_flip(&ctx.card, ctx.crtc, Instant::now(), timeout)?;
            }
        }
    }
    count.drain = drain_events(&ctx.card, ctx.crtc, timeout)?;
    println!("{}", flips_line(&count, options.frames));
    println!("{}", latency_line(&samples, ctx.target_us));
    println!("{}", result_restore(&ctx));

    let _ = drain_events(&ctx.card, ctx.crtc, Duration::from_millis(200));
    for fb in buffers {
        release(&ctx.card, fb);
    }
    Ok(flips_passed(&count, options.frames))
}

fn result_restore(ctx: &Ctx) -> String {
    match restore(ctx) {
        Ok(()) => "RESTORE ok".into(),
        Err(errno) => format!("RESTORE {}", errno_name(errno)),
    }
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
        Ok(passed) => {
            println!("{}", verdict_text(passed));
            if passed {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(message) => {
            println!("VERDICT flip_probe: FAIL {message}");
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
    fn defaults() {
        let o = args("").unwrap();
        assert_eq!(
            o,
            Options {
                device: "/dev/dri/card0".into(),
                connector: None,
                plane: None,
                frames: 120,
                size: (640, 360),
                timeout_ms: 200,
            }
        );
        assert_eq!(args("/dev/dri/card1").unwrap().device, "/dev/dri/card1");
    }

    #[test]
    fn flags() {
        let o = args(
            "--device /d --connector 84 --plane 45 --frames 30 --size 320x200 --timeout-ms 500",
        )
        .unwrap();
        assert_eq!(o.connector, Some(84));
        assert_eq!(o.plane, Some(45));
        assert_eq!(o.frames, 30);
        assert_eq!(o.size, (320, 200));
        assert_eq!(o.timeout_ms, 500);
    }

    #[test]
    fn bad_args() {
        assert!(args("--frames 0").is_err());
        assert!(args("--frames").is_err());
        assert!(args("--size 640").is_err());
        assert!(args("--size 0x360").is_err());
        assert!(args("--plane x").is_err());
        assert!(args("--nope").is_err());
        assert_eq!(args("--help").unwrap_err(), "");
    }

    #[test]
    fn sizes() {
        assert_eq!(parse_size("640x360").unwrap(), (640, 360));
        assert!(parse_size("640x0").is_err());
        assert!(parse_size("abc").is_err());
    }

    #[test]
    fn periods_and_lateness() {
        assert_eq!(period_us(2200, 1125, 148_500), Some(16_666));
        assert_eq!(period_us(0, 1, 1), None);
        assert_eq!(period_us(1, 1, 0), None);
        assert!(!is_late(16_600, Some(16_666)));
        assert!(!is_late(24_999, Some(16_666))); // exactly 1.5x is not late
        assert!(is_late(25_001, Some(16_666)));
        assert!(!is_late(1_000_000, None));
    }

    #[test]
    fn latency_statistics() {
        assert_eq!(latency_stats(&[]), None);
        assert_eq!(latency_stats(&[10, 30, 20]), Some((10, 20, 30)));
        assert_eq!(latency_stats(&[7]), Some((7, 7, 7)));
    }

    #[test]
    fn zpos_choice() {
        assert_eq!(pick_zpos(Some(0), 2, 0, 7), 2);
        assert_eq!(pick_zpos(Some(0), 0, 0, 7), 1);
        assert_eq!(pick_zpos(Some(6), 0, 0, 7), 7);
        assert_eq!(pick_zpos(Some(7), 7, 0, 7), 7);
        assert_eq!(pick_zpos(None, 3, 0, 7), 3);
    }

    #[test]
    fn centring() {
        assert_eq!(centre((1920, 1080), (640, 360)), (640, 360));
        assert_eq!(centre((640, 360), (640, 360)), (0, 0));
        assert_eq!(centre((320, 200), (640, 360)), (0, 0));
    }

    #[test]
    fn in_formats_decode() {
        // count=2 formats at offset 24: XR24, NV12.
        let mut blob = vec![0u8; 32];
        blob[8..12].copy_from_slice(&2u32.to_ne_bytes());
        blob[12..16].copy_from_slice(&24u32.to_ne_bytes());
        blob[24..28].copy_from_slice(&(DrmFourcc::Xrgb8888 as u32).to_ne_bytes());
        blob[28..32].copy_from_slice(&(DrmFourcc::Nv12 as u32).to_ne_bytes());
        let codes = in_formats_fourcc(&blob).unwrap();
        assert_eq!(codes.len(), 2);
        assert_eq!(codes[0], DrmFourcc::Xrgb8888 as u32);
        assert_eq!(choose_fourcc(&codes), Some(DrmFourcc::Xrgb8888));
        let only_argb = vec![DrmFourcc::Argb8888 as u32];
        assert_eq!(choose_fourcc(&only_argb), Some(DrmFourcc::Argb8888));
        assert_eq!(choose_fourcc(&[0x0000_0000u32]), None);
        // A truncated blob decodes to None rather than reading out of range.
        assert!(in_formats_fourcc(&blob[..16]).is_none());
    }

    #[test]
    fn fill_writes_rows() {
        // Two rows of two pixels at pitch 12: one spare word at the end of each row.
        let mut bytes = vec![0u8; 24];
        fill_solid(&mut bytes, 12, 2, 2, [0xE0, 0x30, 0x30, 0xFF]);
        assert_eq!(
            bytes,
            vec![
                0xE0, 0x30, 0x30, 0xFF, 0xE0, 0x30, 0x30, 0xFF, 0, 0, 0, 0, 0xE0, 0x30, 0x30, 0xFF,
                0xE0, 0x30, 0x30, 0xFF, 0, 0, 0, 0
            ]
        );
    }

    #[test]
    fn lines_and_verdict() {
        let mut count = FlipCount {
            committed: 3,
            events: 3,
            ..FlipCount::default()
        };
        assert_eq!(
            flips_line(&count, 3),
            "FLIPS frames=3 committed=3 events=3 missed=0 late=0 errors=0 first_error=none drain=0 aborted=0"
        );
        assert!(flips_passed(&count, 3));
        assert_eq!(verdict_text(true), "VERDICT flip_probe: PASS");
        count.missed = 1;
        assert!(!flips_passed(&count, 3));
        assert!(verdict_text(false).starts_with("VERDICT flip_probe: FAIL"));
        let count = FlipCount {
            errors: 1,
            first_error: Some(16),
            ..FlipCount::default()
        };
        assert!(flips_line(&count, 3).contains("first_error=EBUSY"));
        assert_eq!(
            latency_line(&[10, 20], Some(16)),
            "LATENCY samples=2 target_us=16 min_us=10 avg_us=15 max_us=20"
        );
        assert_eq!(latency_line(&[], None), "LATENCY samples=0 target_us=0");
    }

    #[test]
    fn errno_names() {
        assert_eq!(errno_name(16), "EBUSY");
        assert_eq!(errno_name(22), "EINVAL");
        assert_eq!(errno_name(4242), "errno4242");
    }
}
