//! HVS load probe: where does the display drop frames or count underruns
//! while atomic `TEST_ONLY` still passes? (`docs/design/hvs-backend.md`
//! §11 Q2, bandwidth and underrun.)
//!
//! ```text
//! sudo hvs_load_probe [/dev/dri/cardN] [options]
//!
//!   --device PATH      DRM device (default /dev/dri/card0); a bare argument works too
//!   --connector ID     connector to use (default: the first connected one)
//!   --output N         index among the connected connectors (default 0);
//!                      ignored when --connector is given
//!   --max-layers N     largest layer count of the ladder (default: every
//!                      non-cursor plane that can drive the CRTC and is not
//!                      in use on another CRTC)
//!   --sizes LIST       destination size of each layer as a fraction of the
//!                      output, comma separated, each in (0, 1] (default 0.5,1)
//!   --scales LIST      scale factors, destination over source, comma
//!                      separated (default 1,2,0.5: unscaled, a 2x upscale
//!                      from a half-size source, a 2x downscale from a
//!                      double-size source)
//!   --frames N         real frames per step (default 120; 0 runs TEST_ONLY only)
//!   --buffers N        dumb buffers per source size, 1 or 2 (default 2;
//!                      with 2 the frames alternate between them)
//!   --debugfs DIR      vc4 debugfs directory (default
//!                      /sys/kernel/debug/dri/<minor of the device>)
//! ```
//!
//! # The ladder
//!
//! For N = 1 .. `--max-layers`, for each size of `--sizes`, for each scale
//! of `--scales`, one step. A step puts N layers on the first N candidate
//! planes (primary first, then the overlays by id), every layer the same
//! XRGB8888-class dumb buffer of the source size, placed with the
//! destination size and spread from the top left to the bottom right of the
//! output. Candidate planes not in the step are switched off. Planes in use
//! on another CRTC are never touched, nor are cursor planes; `zpos`, alpha
//! and blend mode are left as they were.
//!
//! Each step does three things:
//!
//! 1. reads `hvs_underrun` (before);
//! 2. checks the state with `TEST_ONLY | ALLOW_MODESET` and, if that passed
//!    and `--frames` is not 0, commits it for real (blocking, with
//!    `ALLOW_MODESET`), then `--frames` nonblocking commits with page-flip
//!    events, one per flip. A flip whose kernel timestamp is more than 1.5
//!    refresh periods after the previous flip is late. A frame whose commit
//!    is refused, or whose flip does not come within one second, is
//!    missing; after a timeout the rest of the step's frames count as missing;
//! 3. reads `hvs_dlists` while the step is still on screen, and
//!    `hvs_underrun` (after).
//!
//! The debugfs reads are read-only. If debugfs cannot be read (not root,
//! not mounted, not vc4) the probe prints `DEBUGFS unavailable reason=...`,
//! prints `na` for those fields and goes on.
//!
//! # Output
//!
//! ```text
//! DATE / KERNEL / DEVICE / DRIVER / MODE / PLANE / LADDER / DEBUGFS lines
//! STEP n=1 layers=1 size=360x640 src=360x640 scale=1.0000 test_only=ok commit=ok latched=120/120 late=0 missing=0 underrun_before=0 underrun_after=0 dlist_words=9 dlist=0:9,1:off,2:off
//! RESTORE ok|err:<errno> ...
//! SUMMARY steps=… test_only_ok=… test_only_err=… skipped=… drop_steps=… first_drop=<n|none> first_reject=<n|none> underrun_delta=<d|na>
//! VERDICT hvs_load_probe: DONE
//! ```
//!
//! `test_only` is `ok`, `err:<errno>` or `skipped:<why>` (a buffer could not
//! be made). `commit` is the first real commit: `ok`, `err:<errno>` or
//! `not-run`. `latched=a/b` counts flips of the `b` frames asked for.
//! `dlist_words` sums the words of every enabled HVS channel (the probe does
//! not know which channel belongs to its CRTC); `dlist` gives them per
//! channel. The format is parsed by `debugfs.rs` (tested on the host by
//! `tests/hvs_load_parse.rs`) and judged by `scripts/hvs-load-expect.py`.
//!
//! # Ending
//!
//! On exit the probe writes back the plane, connector and CRTC properties it
//! read at the start (a CRTC that was off is off again); if that commit is
//! refused it switches its planes off instead. Then it frees its
//! framebuffers, dumb buffers and the mode blob. The exit status is 0 when
//! the ladder ran and the state was restored, 1 when the restore failed and
//! 2 for a usage error or a device that cannot be used.
//!
//! The probe needs DRM master. If another process holds it, the probe says
//! so and stops; it never stops other processes itself.

#![forbid(unsafe_code)]

mod debugfs;

use std::collections::HashMap;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use composition_drm::Card;
use debugfs::{Commit, Dlists, Step, TestOnly};
use drm::buffer::{DrmFourcc, DrmModifier, PlanarBuffer};
use drm::control::atomic::AtomicModeReq;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    connector, crtc, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice,
    Event, FbCmd2Flags, Mode, ModeTypeFlags, PlaneType, ResourceHandle,
};
use drm::{ClientCapability, Device as BasicDevice};

const USAGE: &str = "usage: hvs_load_probe [/dev/dri/cardN] [--device PATH] [--connector ID] \
[--output N] [--max-layers N] [--sizes LIST] [--scales LIST] [--frames N] [--buffers 1|2] \
[--debugfs DIR]";

/// A late flip came more than this many refresh periods after the previous.
const LATE_PERIODS: f64 = 1.5;
/// How long to wait for one flip.
const LATCH_TIMEOUT: Duration = Duration::from_secs(1);
/// 32-bit formats a dumb buffer can hold, in order of preference.
const FORMATS: [DrmFourcc; 4] = [
    DrmFourcc::Xrgb8888,
    DrmFourcc::Argb8888,
    DrmFourcc::Xbgr8888,
    DrmFourcc::Abgr8888,
];
/// Plane properties the probe writes, and so saves and restores.
const PLACEMENT: [&str; 10] = [
    "FB_ID", "CRTC_ID", "SRC_X", "SRC_Y", "SRC_W", "SRC_H", "CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H",
];

struct Options {
    device: String,
    connector: Option<u32>,
    output: usize,
    max_layers: Option<u32>,
    sizes: Vec<f64>,
    scales: Vec<f64>,
    frames: u32,
    buffers: usize,
    debugfs: Option<PathBuf>,
}

fn list(text: &str, name: &str) -> Result<Vec<f64>, String> {
    let values = text
        .split(',')
        .map(|v| {
            v.trim()
                .parse::<f64>()
                .ok()
                .filter(|f| f.is_finite() && *f > 0.0)
                .ok_or(format!("{name}: bad value {v:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.is_empty() {
        return Err(format!("{name} is empty"));
    }
    Ok(values)
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        device: "/dev/dri/card0".into(),
        connector: None,
        output: 0,
        max_layers: None,
        sizes: vec![0.5, 1.0],
        scales: vec![1.0, 2.0, 0.5],
        frames: 120,
        buffers: 2,
        debugfs: None,
    };
    let mut args = std::env::args().skip(1);
    fn number<T: std::str::FromStr>(text: &str) -> Result<T, String> {
        text.parse().map_err(|_| format!("bad number {text}"))
    }
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--device" => options.device = value("--device")?,
            "--connector" => options.connector = Some(number(&value("--connector")?)?),
            "--output" => options.output = number(&value("--output")?)?,
            "--max-layers" => {
                let n: u32 = number(&value("--max-layers")?)?;
                if n == 0 {
                    return Err("--max-layers must be at least 1".into());
                }
                options.max_layers = Some(n);
            }
            "--sizes" => {
                options.sizes = list(&value("--sizes")?, "--sizes")?;
                if options.sizes.iter().any(|s| *s > 1.0) {
                    return Err("--sizes must be fractions in (0, 1]".into());
                }
            }
            "--scales" => options.scales = list(&value("--scales")?, "--scales")?,
            "--frames" => options.frames = number(&value("--frames")?)?,
            "--buffers" => {
                options.buffers = number(&value("--buffers")?)?;
                if !(1..=2).contains(&options.buffers) {
                    return Err("--buffers takes 1 or 2".into());
                }
            }
            "--debugfs" => options.debugfs = Some(value("--debugfs")?.into()),
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

/// Prefixes an I/O error with what was being done.
fn fail(what: &str) -> impl Fn(io::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

fn errno(error: &io::Error) -> i32 {
    error.raw_os_error().unwrap_or(-1)
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

fn prop(props: &Props, name: &str) -> io::Result<(property::Handle, u64)> {
    props
        .get(name)
        .copied()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no property {name}")))
}

/// Raw values are passed through as they were read.
fn raw(value: u64) -> property::Value<'static> {
    property::Value::Unknown(value)
}

/// A candidate plane and the values of [`PLACEMENT`] it had at the start.
struct PlaneInfo {
    handle: plane::Handle,
    kind: PlaneType,
    format: DrmFourcc,
    /// `PLACEMENT` handles, in that order.
    props: Vec<property::Handle>,
    /// `PLACEMENT` values at the start, in that order.
    saved: Vec<u64>,
}

impl PlaneInfo {
    fn set(&self, req: &mut AtomicModeReq, index: usize, value: property::Value<'_>) {
        req.add_property(self.handle, self.props[index], value);
    }

    fn off(&self, req: &mut AtomicModeReq) {
        self.set(req, 0, property::Value::Framebuffer(None));
        self.set(req, 1, property::Value::CRTC(None));
    }
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

/// Fills a buffer with a solid colour and a white 8-pixel border, so every
/// layer's edge can be seen on a capture. `index` picks the colour.
fn paint(pixels: &mut [u8], pitch: usize, size: (u32, u32), index: usize) {
    let fill: u32 = if index % 2 == 0 {
        0x0030_5080
    } else {
        0x0030_8050
    };
    let (w, h) = (size.0 as usize, size.1 as usize);
    for (y, row) in pixels.chunks_mut(pitch).take(h).enumerate() {
        for (x, px) in row.chunks_exact_mut(4).take(w).enumerate() {
            let edge = x < 8 || y < 8 || x + 8 >= w || y + 8 >= h;
            let value = if edge { 0x00ff_ffff } else { fill };
            px.copy_from_slice(&value.to_le_bytes());
        }
    }
}

fn make_fb(card: &Card, size: (u32, u32), format: DrmFourcc, index: usize) -> Result<Fb, String> {
    let mut dumb = card
        .create_dumb_buffer(size, format, 32)
        .map_err(|e| format!("alloc:{}", errno(&e)))?;
    let pitch = drm::buffer::Buffer::pitch(&dumb);
    let painted = card
        .map_dumb_buffer(&mut dumb)
        .map(|mut map| paint(map.as_mut(), pitch as usize, size, index));
    if let Err(e) = painted {
        // An unpainted (zero) buffer still loads the HVS the same way.
        println!("NOTE buffer {}x{} not painted: {e}", size.0, size.1);
    }
    let layout = Layout {
        size,
        format,
        pitch,
        handle: drm::buffer::Buffer::handle(&dumb),
    };
    match card.add_planar_framebuffer(&layout, FbCmd2Flags::empty()) {
        Ok(fb) => Ok(Fb { dumb, fb }),
        Err(error) => {
            let _ = card.destroy_dumb_buffer(dumb);
            Err(format!("fb:{}", errno(&error)))
        }
    }
}

fn release(card: &Card, fb: Fb) {
    let _ = card.destroy_framebuffer(fb.fb);
    let _ = card.destroy_dumb_buffer(fb.dumb);
}

/// The debugfs files the probe reads.
struct Debugfs {
    dir: PathBuf,
    underrun: bool,
    dlists: bool,
}

impl Debugfs {
    fn read(&self, name: &str) -> Result<String, String> {
        let path = self.dir.join(name);
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn underrun(&self) -> Option<u64> {
        if !self.underrun {
            return None;
        }
        match self
            .read("hvs_underrun")
            .and_then(|t| debugfs::parse_underrun(&t).map_err(|e| e.to_string()))
        {
            Ok(count) => Some(count),
            Err(e) => {
                println!("NOTE hvs_underrun read failed: {e}");
                None
            }
        }
    }

    fn dlists(&self) -> Option<Dlists> {
        if !self.dlists {
            return None;
        }
        match self
            .read("hvs_dlists")
            .and_then(|t| debugfs::parse_dlists(&t).map_err(|e| e.to_string()))
        {
            Ok(d) => Some(d),
            Err(e) => {
                println!("NOTE hvs_dlists read failed: {e}");
                None
            }
        }
    }
}

/// The device minor of a DRM node, from its `st_rdev`.
fn minor(device: &str) -> Option<u64> {
    let rdev = std::fs::metadata(device).ok()?.rdev();
    Some(((rdev >> 12) & 0xffff_ff00) | (rdev & 0xff))
}

fn open_debugfs(options: &Options) -> Debugfs {
    let dir = match (&options.debugfs, minor(&options.device)) {
        (Some(dir), _) => dir.clone(),
        (None, Some(minor)) => PathBuf::from(format!("/sys/kernel/debug/dri/{minor}")),
        (None, None) => {
            println!(
                "DEBUGFS unavailable reason=no device minor for {}",
                options.device
            );
            return Debugfs {
                dir: PathBuf::new(),
                underrun: false,
                dlists: false,
            };
        }
    };
    let mut debugfs = Debugfs {
        dir,
        underrun: true,
        dlists: true,
    };
    let underrun = debugfs
        .read("hvs_underrun")
        .and_then(|t| debugfs::parse_underrun(&t).map_err(|e| e.to_string()));
    let dlists = debugfs
        .read("hvs_dlists")
        .and_then(|t| debugfs::parse_dlists(&t).map_err(|e| e.to_string()));
    match (&underrun, &dlists) {
        (Err(a), Err(_)) => {
            println!("DEBUGFS unavailable reason={a}");
            debugfs.underrun = false;
            debugfs.dlists = false;
        }
        _ => {
            println!(
                "DEBUGFS dir={} underrun={} dlist_words={} dlist={}",
                debugfs.dir.display(),
                underrun
                    .as_ref()
                    .map_or("na".into(), |u: &u64| u.to_string()),
                dlists
                    .as_ref()
                    .map_or("na".into(), |d| d.total_words().to_string()),
                dlists
                    .as_ref()
                    .map_or("na".into(), Dlists::per_channel_text),
            );
            if let Err(e) = underrun {
                println!("DEBUGFS hvs_underrun unavailable reason={e}");
                debugfs.underrun = false;
            }
            if let Err(e) = dlists {
                println!("DEBUGFS hvs_dlists unavailable reason={e}");
                debugfs.dlists = false;
            }
        }
    }
    debugfs
}

/// Refresh period of a mode, from its pixel clock and totals.
fn period(mode: &Mode) -> Duration {
    let (_, _, htotal) = mode.hsync();
    let (_, _, vtotal) = mode.vsync();
    let pixels = u64::from(htotal) * u64::from(vtotal);
    if mode.clock() > 0 && pixels > 0 {
        Duration::from_nanos(pixels * 1_000_000 / u64::from(mode.clock()))
    } else {
        Duration::from_secs_f64(1.0 / f64::from(mode.vrefresh().max(1)))
    }
}

struct Ctx {
    card: Card,
    crtc: crtc::Handle,
    connector: connector::Handle,
    connector_crtc_id: (property::Handle, u64),
    crtc_mode_id: (property::Handle, u64),
    crtc_active: (property::Handle, u64),
    mode_blob: u64,
    output: (u32, u32),
    period: Duration,
    planes: Vec<PlaneInfo>,
}

impl Ctx {
    fn crtc_on(&self, req: &mut AtomicModeReq) {
        req.add_property(
            self.connector,
            self.connector_crtc_id.0,
            property::Value::CRTC(Some(self.crtc)),
        );
        req.add_property(
            self.crtc,
            self.crtc_mode_id.0,
            property::Value::Blob(self.mode_blob),
        );
        req.add_property(
            self.crtc,
            self.crtc_active.0,
            property::Value::Boolean(true),
        );
    }

    /// The request for a step: `layers` planes showing `fb` of size `src`
    /// at destination size `dst`, every other candidate off.
    fn request(
        &self,
        layers: usize,
        fb: framebuffer::Handle,
        src: (u32, u32),
        dst: (u32, u32),
        with_crtc: bool,
    ) -> AtomicModeReq {
        let mut req = AtomicModeReq::new();
        if with_crtc {
            self.crtc_on(&mut req);
        }
        let spread = |out: u32, size: u32, i: usize| -> i64 {
            if layers < 2 {
                0
            } else {
                (i64::from(out - size) * i as i64) / (layers as i64 - 1)
            }
        };
        for (i, plane) in self.planes.iter().enumerate() {
            if i >= layers {
                plane.off(&mut req);
                continue;
            }
            plane.set(&mut req, 0, property::Value::Framebuffer(Some(fb)));
            plane.set(&mut req, 1, property::Value::CRTC(Some(self.crtc)));
            let source = [0, 0, u64::from(src.0) << 16, u64::from(src.1) << 16];
            for (k, v) in source.into_iter().enumerate() {
                plane.set(&mut req, 2 + k, property::Value::UnsignedRange(v));
            }
            plane.set(
                &mut req,
                6,
                property::Value::SignedRange(spread(self.output.0, dst.0, i)),
            );
            plane.set(
                &mut req,
                7,
                property::Value::SignedRange(spread(self.output.1, dst.1, i)),
            );
            plane.set(
                &mut req,
                8,
                property::Value::UnsignedRange(u64::from(dst.0)),
            );
            plane.set(
                &mut req,
                9,
                property::Value::UnsignedRange(u64::from(dst.1)),
            );
        }
        req
    }

    /// The kernel timestamp of the next page flip of the CRTC, or `None` if
    /// none comes before `timeout`.
    fn wait_flip(&self, timeout: Duration) -> Result<Option<Duration>, String> {
        use rustix::event::{poll, PollFd, PollFlags, Timespec};
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            let timeout = Timespec {
                tv_sec: left.as_secs() as _,
                tv_nsec: left.subsec_nanos() as _,
            };
            let mut fds = [PollFd::new(&self.card, PollFlags::IN)];
            match poll(&mut fds, Some(&timeout)) {
                Ok(_) => {}
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(format!("poll: {error}")),
            }
            if !fds[0].revents().contains(PollFlags::IN) {
                continue;
            }
            for event in self.card.receive_events().map_err(fail("events"))? {
                if let Event::PageFlip(flip) = event {
                    if flip.crtc == self.crtc {
                        return Ok(Some(flip.duration));
                    }
                }
            }
        }
    }
}

fn open(options: &Options) -> Result<(Ctx, bool), String> {
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

    // Connector: by id, else the --output-th connected one.
    let resources = card.resource_handles().map_err(fail("resources"))?;
    let mut connected = Vec::new();
    for handle in resources.connectors() {
        let info = card
            .get_connector(*handle, false)
            .map_err(fail("connector"))?;
        if info.state() == connector::State::Connected && !info.modes().is_empty() {
            connected.push((*handle, info));
        }
    }
    let (connector, info) = match options.connector {
        Some(id) => connected
            .into_iter()
            .find(|(h, _)| u32::from(*h) == id)
            .ok_or(format!("connector {id} is not connected or has no modes"))?,
        None => {
            let count = connected.len();
            connected.into_iter().nth(options.output).ok_or(format!(
                "--output {} but {count} connected connector(s)",
                options.output
            ))?
        }
    };
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
    let crtc_props = named_properties(&card, crtc).map_err(fail("crtc props"))?;
    let crtc_active = prop(&crtc_props, "ACTIVE").map_err(fail("crtc"))?;
    let was_active = crtc_active.1 != 0;
    let preferred = info
        .modes()
        .iter()
        .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
        .or_else(|| info.modes().first())
        .copied()
        .ok_or("no mode")?;
    // A running CRTC keeps its mode: no modeset is needed then.
    let running = card
        .get_crtc(crtc)
        .map_err(fail("crtc"))?
        .mode()
        .filter(|_| was_active);
    let mode = running.unwrap_or(preferred);
    let (mw, mh) = mode.size();
    let output = (u32::from(mw), u32::from(mh));
    let period = period(&mode);
    println!(
        "MODE {}x{}@{}Hz period_us={} connector {} crtc {}{}",
        output.0,
        output.1,
        mode.vrefresh(),
        period.as_micros(),
        u32::from(connector),
        u32::from(crtc),
        if running.is_some() {
            " (running mode, no modeset)"
        } else {
            ""
        }
    );
    let connector_props = named_properties(&card, connector).map_err(fail("connector props"))?;

    // Candidate planes: not cursor, can drive the CRTC, not on another CRTC.
    let mut planes = Vec::new();
    for handle in card.plane_handles().map_err(fail("planes"))? {
        let pinfo = card.get_plane(handle).map_err(fail("plane"))?;
        if !resources
            .filter_crtcs(pinfo.possible_crtcs())
            .contains(&crtc)
        {
            continue;
        }
        let props = named_properties(&card, handle).map_err(fail("plane props"))?;
        let kind = match props.get("type").map(|(_, v)| *v as u32) {
            Some(v) if v == PlaneType::Primary as u32 => PlaneType::Primary,
            Some(v) if v == PlaneType::Cursor as u32 => PlaneType::Cursor,
            _ => PlaneType::Overlay,
        };
        let id = u32::from(handle);
        if kind == PlaneType::Cursor {
            println!("PLANE id {id} {kind:?} skipped: cursor");
            continue;
        }
        let on = props.get("CRTC_ID").map_or(0, |(_, v)| *v);
        if on != 0 && on != u64::from(u32::from(crtc)) {
            println!("PLANE id {id} {kind:?} skipped: in use on CRTC {on}");
            continue;
        }
        let Some(format) = FORMATS
            .into_iter()
            .find(|f| pinfo.formats().contains(&(*f as u32)))
        else {
            println!("PLANE id {id} {kind:?} skipped: no 32-bit RGB format");
            continue;
        };
        let Ok(placement) = PLACEMENT
            .iter()
            .map(|name| prop(&props, name))
            .collect::<io::Result<Vec<_>>>()
        else {
            println!("PLANE id {id} {kind:?} skipped: placement properties missing");
            continue;
        };
        let (handles, saved): (Vec<_>, Vec<_>) = placement.into_iter().unzip();
        println!(
            "PLANE id {id} {kind:?} candidate format {}",
            fourcc_text(format)
        );
        planes.push(PlaneInfo {
            handle,
            kind,
            format,
            props: handles,
            saved,
        });
    }
    // Primary first, then by id.
    planes.sort_by_key(|p| (p.kind != PlaneType::Primary, u32::from(p.handle)));
    if planes.is_empty() {
        return Err("no candidate plane can drive the CRTC".into());
    }
    let mode_blob = match card
        .create_property_blob(&mode)
        .map_err(fail("mode blob"))?
    {
        property::Value::Blob(id) => id,
        _ => return Err("mode blob".into()),
    };
    Ok((
        Ctx {
            card,
            crtc,
            connector,
            connector_crtc_id: prop(&connector_props, "CRTC_ID").map_err(fail("connector"))?,
            crtc_mode_id: prop(&crtc_props, "MODE_ID").map_err(fail("crtc"))?,
            crtc_active,
            mode_blob,
            output,
            period,
            planes,
        },
        was_active,
    ))
}

/// Writes back what was read at the start; on refusal, switches the
/// candidate planes off (and the CRTC, if it was off).
fn restore(ctx: &Ctx, was_active: bool) -> bool {
    let mut req = AtomicModeReq::new();
    for plane in &ctx.planes {
        for (handle, value) in plane.props.iter().zip(plane.saved.iter().copied()) {
            req.add_property(plane.handle, *handle, raw(value));
        }
    }
    req.add_property(
        ctx.connector,
        ctx.connector_crtc_id.0,
        raw(ctx.connector_crtc_id.1),
    );
    req.add_property(ctx.crtc, ctx.crtc_mode_id.0, raw(ctx.crtc_mode_id.1));
    req.add_property(ctx.crtc, ctx.crtc_active.0, raw(ctx.crtc_active.1));
    match ctx
        .card
        .atomic_commit(AtomicCommitFlags::ALLOW_MODESET, req)
    {
        Ok(()) => {
            println!("RESTORE ok saved state written back");
            return true;
        }
        Err(e) => println!("RESTORE err:{} saved state refused", errno(&e)),
    }
    let mut req = AtomicModeReq::new();
    for plane in &ctx.planes {
        plane.off(&mut req);
    }
    if !was_active {
        req.add_property(ctx.crtc, ctx.crtc_active.0, property::Value::Boolean(false));
    }
    match ctx
        .card
        .atomic_commit(AtomicCommitFlags::ALLOW_MODESET, req)
    {
        Ok(()) => println!("RESTORE fallback ok planes off"),
        Err(e) => println!("RESTORE fallback err:{}", errno(&e)),
    }
    false
}

/// Buffers per source size.
type Pool = Vec<((u32, u32), Vec<Fb>)>;

/// Frame counts of one step.
struct Frames {
    commit: Commit,
    latched: u32,
    late: u32,
    missing: u32,
}

/// The step's real commit and its frames. `fbs` are the step's buffers.
fn run_frames(
    ctx: &Ctx,
    options: &Options,
    layers: usize,
    fbs: &[framebuffer::Handle],
    src: (u32, u32),
    dst: (u32, u32),
) -> Result<Frames, String> {
    let frames = options.frames;
    let mut result = Frames {
        commit: Commit::Ok,
        latched: 0,
        late: 0,
        missing: 0,
    };
    let first = ctx.request(layers, fbs[0], src, dst, true);
    if let Err(e) = ctx.card.atomic_commit(
        AtomicCommitFlags::ALLOW_MODESET | AtomicCommitFlags::PAGE_FLIP_EVENT,
        first,
    ) {
        result.commit = Commit::Err(errno(&e));
        result.missing = frames;
        return Ok(result);
    }
    let Some(mut previous) = ctx.wait_flip(LATCH_TIMEOUT)? else {
        println!("NOTE no flip event for the step's first commit");
        result.missing = frames;
        return Ok(result);
    };
    let late_after = ctx.period.mul_f64(LATE_PERIODS);
    for frame in 0..frames {
        let fb = fbs[(frame as usize + 1) % fbs.len()];
        let req = ctx.request(layers, fb, src, dst, false);
        if ctx
            .card
            .atomic_commit(
                AtomicCommitFlags::NONBLOCK | AtomicCommitFlags::PAGE_FLIP_EVENT,
                req,
            )
            .is_err()
        {
            result.missing += 1;
            continue;
        }
        match ctx.wait_flip(LATCH_TIMEOUT)? {
            Some(at) => {
                result.latched += 1;
                if at.saturating_sub(previous) > late_after {
                    result.late += 1;
                }
                previous = at;
            }
            None => {
                // The flip may still be pending: stop the step here.
                result.missing += frames - frame;
                println!("NOTE flip timeout at frame {frame}; rest of the step counted missing");
                break;
            }
        }
    }
    Ok(result)
}

fn ladder(ctx: &Ctx, options: &Options, debugfs: &Debugfs) -> Result<(Vec<Step>, bool), String> {
    let max_layers = options
        .max_layers
        .map_or(ctx.planes.len(), |n| (n as usize).min(ctx.planes.len()));
    if options
        .max_layers
        .is_some_and(|n| n as usize > ctx.planes.len())
    {
        println!(
            "NOTE --max-layers capped to the {} candidate plane(s)",
            ctx.planes.len()
        );
    }
    let format = ctx.planes[0].format;
    if ctx.planes.iter().any(|p| p.format != format) {
        println!(
            "NOTE candidate planes prefer different formats; every layer uses {}",
            fourcc_text(format)
        );
    }
    println!(
        "LADDER max_layers={max_layers} sizes={} scales={} frames={} buffers={} format={}",
        options
            .sizes
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(","),
        options
            .scales
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(","),
        options.frames,
        options.buffers,
        fourcc_text(format),
    );
    let mut steps = Vec::new();
    let mut pool: Pool = Vec::new();
    let mut on_screen: Option<(u32, u32)> = None;
    let mut touched = false;
    let mut n = 0;
    let outcome = (|| -> Result<(), String> {
        for layers in 1..=max_layers {
            for &size in &options.sizes {
                for &scale in &options.scales {
                    n += 1;
                    let axis = |out: u32| ((f64::from(out) * size).round() as u32).clamp(1, out);
                    let dst = (axis(ctx.output.0), axis(ctx.output.1));
                    let source = |d: u32| ((f64::from(d) / scale).round() as u32).max(1);
                    let src = (source(dst.0), source(dst.1));
                    let underrun_before = debugfs.underrun();
                    let mut step = Step {
                        n,
                        layers: layers as u32,
                        size: dst,
                        src,
                        scale,
                        test_only: TestOnly::Ok,
                        commit: Commit::NotRun,
                        latched: (0, 0),
                        late: 0,
                        missing: 0,
                        underrun_before,
                        underrun_after: None,
                        dlist_words: None,
                        dlist: "na".into(),
                    };
                    // Buffers of this source size.
                    if !pool.iter().any(|(s, _)| *s == src) {
                        let mut fbs = Vec::new();
                        for i in 0..options.buffers {
                            match make_fb(&ctx.card, src, format, i) {
                                Ok(fb) => fbs.push(fb),
                                Err(why) => {
                                    step.test_only = TestOnly::Skipped(why);
                                    break;
                                }
                            }
                        }
                        if step.test_only == TestOnly::Ok {
                            pool.push((src, fbs));
                        } else {
                            for fb in fbs {
                                release(&ctx.card, fb);
                            }
                        }
                    }
                    if step.test_only == TestOnly::Ok {
                        let fbs: Vec<framebuffer::Handle> = pool
                            .iter()
                            .find(|(s, _)| *s == src)
                            .map(|(_, f)| f.iter().map(|f| f.fb).collect())
                            .unwrap_or_default();
                        let req = ctx.request(layers, fbs[0], src, dst, true);
                        match ctx.card.atomic_commit(
                            AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::ALLOW_MODESET,
                            req,
                        ) {
                            Ok(()) if options.frames > 0 => {
                                touched = true;
                                let frames = run_frames(ctx, options, layers, &fbs, src, dst)?;
                                if frames.commit == Commit::Ok {
                                    on_screen = Some(src);
                                }
                                step.commit = frames.commit;
                                step.latched = (frames.latched, options.frames);
                                step.late = frames.late;
                                step.missing = frames.missing;
                            }
                            Ok(()) => {}
                            Err(e) => step.test_only = TestOnly::Err(errno(&e)),
                        }
                    }
                    if let Some(d) = debugfs.dlists() {
                        step.dlist_words = Some(d.total_words());
                        step.dlist = d.per_channel_text();
                    }
                    step.underrun_after = debugfs.underrun();
                    println!("{}", step.line());
                    steps.push(step);
                    // Only the buffers on screen are kept.
                    let (keep, drop): (Pool, Pool) =
                        pool.drain(..).partition(|(s, _)| Some(*s) == on_screen);
                    pool = keep;
                    for (_, fbs) in drop {
                        for fb in fbs {
                            release(&ctx.card, fb);
                        }
                    }
                }
            }
        }
        Ok(())
    })();
    let restored = if touched {
        restore(ctx, ctx.crtc_active.1 != 0)
    } else {
        println!("RESTORE ok nothing was committed");
        true
    };
    for (_, fbs) in pool {
        for fb in fbs {
            release(&ctx.card, fb);
        }
    }
    outcome?;
    Ok((steps, restored))
}

fn run(options: &Options) -> Result<bool, String> {
    let (ctx, _was_active) = open(options)?;
    let debugfs = open_debugfs(options);
    let result = ladder(&ctx, options, &debugfs);
    let _ = ctx.card.destroy_property_blob(ctx.mode_blob);
    let (steps, restored) = result?;
    println!("{}", debugfs::summary_line(&steps));
    println!("VERDICT hvs_load_probe: DONE");
    Ok(restored)
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
            println!("VERDICT hvs_load_probe: FAIL {message}");
            ExitCode::from(2)
        }
    }
}
