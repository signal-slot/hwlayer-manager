//! M2 scene: the Linux side of the WP3 Linux vs Zephyr comparison
//! (`SoW.md` §5.1 / WP3), writing `docs/trace-format.md` event lines.
//!
//! ```text
//! cargo run -p composition-drm --example m2scene -- [options]
//!
//!   --device PATH   DRM device (default /dev/dri/card0)
//!   --frames N      number of frames to commit (default 300)
//!   --output N      index of the output in the backend's capabilities (default 0)
//!   --profile P     driver profile: generic (default) or imx95 (`DriverProfile::Imx95Dpu`)
//!   --dry-run       open no device: print the RUN line, the scene and the plan
//!                   the planner makes for it on a modelled topology, then exit
//! ```
//!
//! # The scene
//!
//! Stage B of `docs/m2-linux-zephyr-comparison.md` §2, as pinned by
//! `crates/composition-core/tests/m2_scene.rs`, at fixed geometry (the move
//! test is not part of this example):
//!
//! - `base`: buffer 1 (and 3, the second slot), `W` x `H`, full output,
//!   z_order 0, linear `ARGB8888` (the agreed variant for both sides), opaque
//!   pixels, blend `Premultiplied`.
//! - `top`: buffer 2 (and 4), `W/3` x `H/3`, `ARGB8888`, blend
//!   `Premultiplied`, plane alpha `0x8000`, z_order 1, destination at
//!   (`W * 500 / 1280`, `H * 300 / 800`), which is (500, 300) on the
//!   1280 x 800 panel the docs assume.
//!
//! `W` x `H` is the mode of the output the backend reports; the docs' 1280 x
//! 800 is inferred, not measured, so the example does not force it. The
//! pixels are static: every frame commits the same picture from one of two
//! buffer slots, alternately, so a slot is free again once the next frame
//! latched.
//!
//! # What it does
//!
//! Opens the device, becomes DRM master (or exits with status 2; it never
//! stops other processes), creates the four dumb buffers and then, per
//! frame: `validate`, a non-blocking `commit` (the backend uses `NONBLOCK |
//! PAGE_FLIP_EVENT`), and a wait for that submission's page-flip event
//! before the next frame. At the end it commits an empty frame so that the
//! last buffers come back, and frees them.
//!
//! # Output
//!
//! ```text
//! RUN date=YYYY-MM-DD side=linux stage=B tool=m2scene-<version> time=<ISO 8601 UTC> frames=N mode=device|dry-run
//! DEVICE / KERNEL / DRIVER / MODE lines
//! SCENE layer=base buffer=1,3 format=AR24 src=0,0,WxH dst=0,0,WxH zorder=0 blend=premultiplied alpha=0xffff
//! SCENE layer=top ...
//! CETRACE v=1 ev=hdr ...
//! CETRACE v=1 ev=validate ... / ev=commit ... / ev=latched ... / ev=released ...
//! RESULT frames=N committed=C latched=L dropped=D timeouts=T
//! VERDICT m2scene: DONE|FAIL <reason>
//! ```
//!
//! The `RUN` line's date is the system clock (UTC) when the run started; it
//! is the form `scripts/m2-compare-report.py` reads. `CETRACE` lines follow
//! `docs/trace-format.md` §3: `t` and `at` are `CLOCK_MONOTONIC`
//! nanoseconds (the engine is given the same clock through `advance_time`,
//! hence `at_unit=same`); `latched` lines carry `tev`, the page-flip
//! event's vblank time, and the extra field `vseq`, the CRTC's vblank
//! counter; `validate` and `commit` carry `dur`, measured around the call.
//! The planner's per-candidate `check` entries are not written. Should the
//! monotonic time of the process not be readable (see `monotonic_base`),
//! `hdr` says `clock=rel_ns`, `t` counts from the start of the run, and the
//! latch time is written as `vbl_ns` instead of `tev`, because the two
//! clocks then have no common origin.
//!
//! Nothing here interprets the times: a log of this example is input for the
//! M2 comparison, not a result. Exit status 0 after `VERDICT ... DONE`, 1 for
//! a run that failed, 2 for a usage error or a device that cannot be used.

use std::collections::HashMap;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, BufferState, Capabilities, DisplayEvent,
    DropReason, Engine, Error, FormatModifier, Frame, Layer, LayerTraits, Modifier, OutputCaps,
    OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, PresentError, Rect,
    ResourceGroup, ScaleLimits, Size, SubmissionId, Trace, TraceEvent, Transform, Zpos,
};
use composition_drm::{DriverProfile, DrmBackend, DrmOptions, LatchInfo};
use drm::Device as _;

const USAGE: &str =
    "usage: m2scene [--device PATH] [--frames N] [--output N] [--profile generic|imx95] [--dry-run]";

/// Plane alpha of `top`: one half (`m2_scene.rs`).
const HALF_ALPHA: u16 = 0x8000;
/// Scene format on both sides (`docs/m2-linux-zephyr-comparison.md` §2).
const FORMAT: PixelFormat = PixelFormat::ARGB8888;
/// Buffer ids: slot 0 is (1, 2), slot 1 is (3, 4).
const SLOTS: [(BufferId, BufferId); 2] = [(BufferId(1), BufferId(2)), (BufferId(3), BufferId(4))];
/// How long to wait for one submission's page-flip event.
const LATCH_WAIT: Duration = Duration::from_secs(1);
/// Size the docs assume for the panel, used by the dry run only.
const DOC_SIZE: Size = Size::new(1280, 800);

struct Options {
    device: String,
    frames: u32,
    output: usize,
    profile: DriverProfile,
    dry_run: bool,
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        device: "/dev/dri/card0".into(),
        frames: 300,
        output: 0,
        profile: DriverProfile::Generic,
        dry_run: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--device" => options.device = value("--device")?,
            "--frames" => options.frames = number(&value("--frames")?)?,
            "--output" => options.output = number(&value("--output")?)?,
            "--profile" => {
                options.profile = match value("--profile")?.as_str() {
                    "generic" => DriverProfile::Generic,
                    "imx95" => DriverProfile::Imx95Dpu,
                    other => return Err(format!("unknown profile {other}")),
                }
            }
            "--dry-run" => options.dry_run = true,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(options)
}

fn number<T: std::str::FromStr>(text: &str) -> Result<T, String> {
    text.parse().map_err(|_| format!("bad number {text}"))
}

/// Current UTC date and time as (`YYYY-MM-DD`, ISO 8601), without a date
/// crate. Days to civil date after Howard Hinnant's `civil_from_days`.
fn utc_now() -> (String, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
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
    let date = format!("{year:04}-{month:02}-{day:02}");
    let time = format!(
        "{date}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    );
    (date, time)
}

fn fourcc(format: PixelFormat) -> String {
    format
        .0
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

fn rect_text(r: Rect) -> String {
    format!("{},{},{}x{}", r.x, r.y, r.width, r.height)
}

/// The scene's two layers for an output of `size`, from buffer slot `slot`.
fn scene(size: Size, slot: usize) -> [Layer; 2] {
    let (base, top) = SLOTS[slot];
    let full = Rect::new(0, 0, size.width, size.height);
    let top_size = top_size(size);
    let x = (u64::from(size.width) * 500 / 1280) as i32;
    let y = (u64::from(size.height) * 300 / 800) as i32;
    [
        Layer {
            blend: BlendMode::Premultiplied,
            ..Layer::new(base, full, full, 0)
        },
        Layer {
            blend: BlendMode::Premultiplied,
            alpha: HALF_ALPHA,
            ..Layer::new(
                top,
                Rect::new(0, 0, top_size.width, top_size.height),
                Rect::new(x, y, top_size.width, top_size.height),
                1,
            )
        },
    ]
}

fn top_size(size: Size) -> Size {
    Size::new((size.width / 3).max(1), (size.height / 3).max(1))
}

fn buffer_info(size: Size) -> BufferInfo {
    BufferInfo {
        size,
        format: FORMAT,
        modifier: Modifier::LINEAR,
    }
}

fn print_scene(size: Size) {
    let names = ["base", "top"];
    let slot0 = scene(size, 0);
    let slot1 = scene(size, 1);
    for (i, layer) in slot0.iter().enumerate() {
        println!(
            "SCENE layer={} buffer={},{} format={} src={} dst={} zorder={} blend=premultiplied alpha=0x{:04x}",
            names[i],
            layer.buffer.0,
            slot1[i].buffer.0,
            fourcc(FORMAT),
            rect_text(layer.source),
            rect_text(layer.destination),
            layer.z_order,
            layer.alpha
        );
    }
}

/// The base layer: opaque dark grey. `top`: opaque light blue; the plane
/// alpha makes it half transparent. Byte order of `ARGB8888` in memory is
/// B, G, R, A.
const BASE_PIXEL: [u8; 4] = [0x20, 0x20, 0x20, 0xff];
const TOP_PIXEL: [u8; 4] = [0xff, 0xc0, 0x00, 0xff];

fn fill(pixels: &mut [u8], pitch: usize, size: Size, pixel: [u8; 4]) {
    for row in pixels.chunks_mut(pitch).take(size.height as usize) {
        for px in row.chunks_exact_mut(4).take(size.width as usize) {
            px.copy_from_slice(&pixel);
        }
    }
}

// --- Clock -----------------------------------------------------------------

/// `CLOCK_MONOTONIC` in nanoseconds at `now`, read from the `Debug` form of
/// `Instant` (`Instant { tv_sec: S, tv_nsec: N }` on Linux; `Instant` is
/// `CLOCK_MONOTONIC` there). The crate's dependencies have no
/// `clock_gettime` without a feature change, so this is the only way to
/// put the commit times and the page-flip times on one clock. `None` when
/// the form is not recognised.
fn monotonic_base(now: Instant) -> Option<u64> {
    let text = format!("{now:?}");
    let field = |name: &str| -> Option<u64> {
        let rest = &text[text.find(name)? + name.len()..];
        let digits: String = rest
            .trim_start_matches([':', ' '])
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        digits.parse().ok()
    };
    let secs = field("tv_sec")?;
    let nanos = field("tv_nsec")?;
    (nanos < 1_000_000_000).then(|| secs * 1_000_000_000 + nanos)
}

#[derive(Clone, Copy)]
struct Clock {
    start: Instant,
    /// Monotonic nanoseconds at `start`, if known.
    base: Option<u64>,
}

impl Clock {
    fn new() -> Self {
        let start = Instant::now();
        Clock {
            start,
            base: monotonic_base(start),
        }
    }

    fn now(&self) -> u64 {
        self.base.unwrap_or(0) + self.start.elapsed().as_nanos() as u64
    }

    fn name(&self) -> &'static str {
        if self.base.is_some() {
            "monotonic_ns"
        } else {
            "rel_ns"
        }
    }
}

// --- Trace sink --------------------------------------------------------------

/// One recorded entry, formatted except for what needs the backend's latch
/// record (`tev`, `vseq` of a `latched` line).
struct Entry {
    seq: u64,
    text: String,
    latched: Option<(OutputId, SubmissionId)>,
}

/// Formats the engine's trace into `docs/trace-format.md` lines. The engine
/// owns the backend while it records, so entries are held until `flush`,
/// which also gets the page-flip records the backend kept.
struct Sink {
    clock: Clock,
    seq: u64,
    /// Clock value when the operation now running (validate, commit)
    /// started, for `dur`.
    op_start: Option<u64>,
    entries: Vec<Entry>,
    committed: u64,
    latched: u64,
    dropped: u64,
    timeouts: u64,
    lost: bool,
    last_latched: Option<SubmissionId>,
}

impl Sink {
    fn new(clock: Clock) -> Self {
        Sink {
            clock,
            seq: 1,
            op_start: None,
            entries: Vec::new(),
            committed: 0,
            latched: 0,
            dropped: 0,
            timeouts: 0,
            lost: false,
            last_latched: None,
        }
    }

    fn header(&self, board: &str) -> String {
        format!(
            "CETRACE v=1 ev=hdr seq=0 src=linux clock={} freq_hz=1000000000 t0={} at_unit=same build=\"composition-drm {} m2scene\" board=\"{}\"",
            self.clock.name(),
            self.clock.base.unwrap_or(0),
            env!("CARGO_PKG_VERSION"),
            board.replace('"', "'")
        )
    }

    /// Prints the held entries, with latch times from `latches`.
    fn flush(&mut self, latches: &HashMap<(OutputId, SubmissionId), LatchInfo>) {
        let base = self.clock.base;
        for entry in self.entries.drain(..) {
            let mut line = format!(
                "CETRACE v=1 ev={}",
                entry
                    .text
                    .replacen("\u{0}", &format!(" seq={}", entry.seq), 1,)
            );
            if let Some(info) = entry.latched.and_then(|key| latches.get(&key)) {
                let vbl = info.timestamp.as_nanos() as u64;
                match base {
                    Some(_) => line.push_str(&format!(" tev={vbl}")),
                    None => line.push_str(&format!(" vbl_ns={vbl}")),
                }
                line.push_str(&format!(" vseq={}", info.sequence));
            }
            println!("{line}");
        }
    }
}

fn assign_text(assignments: &[composition_core::Assignment]) -> String {
    assignments
        .iter()
        .map(|a| {
            format!(
                "{}:{}:{}:{}:{},{},{},{}:{},{},{},{}",
                a.layer,
                a.plane.0,
                a.plane_index,
                a.zpos,
                a.source.x,
                a.source.y,
                a.source.width,
                a.source.height,
                a.destination.x,
                a.destination.y,
                a.destination.width,
                a.destination.height
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// `result=` code and extras of `docs/trace-format.md` §3.7.
fn backend_code(error: BackendError) -> String {
    match error {
        BackendError::Rejected => "rejected".into(),
        BackendError::Busy => "busy".into(),
        BackendError::UnknownBuffer(b) => format!("unknown_buffer buf={}", b.0),
        BackendError::DeviceLost => "device_lost".into(),
        BackendError::Platform(errno) => format!("platform errno={errno}"),
    }
}

fn error_code(error: Error) -> String {
    match error {
        Error::StalePlan => "stale_plan".into(),
        Error::PlanMismatch => "plan_mismatch".into(),
        Error::Backend(e) => format!("backend.{}", backend_code(e)),
        Error::Plan(e) => format!(
            "plan.{}",
            match e {
                PlanError::UnknownOutput(o) => format!("unknown_output out={}", o.0),
                PlanError::TooManyLayers { count } => format!("too_many_layers count={count}"),
                PlanError::UnknownBuffer { layer, buffer } => {
                    format!("unknown_buffer layer={layer} buf={}", buffer.0)
                }
                PlanError::InvalidSource { layer } => format!("invalid_source layer={layer}"),
                PlanError::InvalidDestination { layer } => {
                    format!("invalid_destination layer={layer}")
                }
                PlanError::CannotClip { layer } => format!("cannot_clip layer={layer}"),
                PlanError::NoPlane { layer, reasons } => {
                    format!("no_plane layer={layer} reasons=0x{:x}", reasons.bits())
                }
                PlanError::BackendRejected { candidates } => {
                    format!("backend_rejected candidates={candidates}")
                }
                PlanError::LimitExceeded => "limit_exceeded".into(),
                PlanError::Aborted => "aborted".into(),
            }
        ),
        Error::Present(e) => format!(
            "present.{}",
            match e {
                PresentError::UnknownBuffer(b) => format!("unknown_buffer buf={}", b.0),
                PresentError::DuplicateBuffer(b) => format!("duplicate_buffer buf={}", b.0),
                PresentError::BufferInUse(b) => format!("buffer_in_use buf={}", b.0),
                PresentError::BufferNotReady(b) => format!("buffer_not_ready buf={}", b.0),
                PresentError::BufferNotWritable(b) => format!("buffer_not_writable buf={}", b.0),
                PresentError::Busy(o) => format!("busy out={}", o.0),
                PresentError::UnexpectedLatch { output, submission } => {
                    format!("unexpected_latch out={} sub={}", output.0, submission.0)
                }
                PresentError::BufferTableFull => "buffer_table_full".into(),
                PresentError::TooManyOutputs => "too_many_outputs".into(),
                PresentError::TooManyBuffers => "too_many_buffers".into(),
                PresentError::EventQueueFull => "event_queue_full".into(),
            }
        ),
    }
}

impl Trace for Sink {
    fn record(&mut self, at: u64, event: TraceEvent<'_>) {
        let t = self.clock.now();
        let dur = |start: Option<u64>| start.map_or(String::new(), |s| format!(" dur={}", t - s));
        // `\0` marks where `seq=` goes, so that `flush` writes it.
        let common = format!("\u{0} t={t} at={at}");
        let mut latched = None;
        let text = match event {
            // Not written: one entry per candidate, see the module doc.
            TraceEvent::Check { .. } => return,
            TraceEvent::Validate {
                output,
                layers,
                result,
            } => match result {
                Ok(a) => format!(
                    "validate{common} out={} layers={layers} n={} assign={} result=ok{}",
                    output.0,
                    a.len(),
                    assign_text(a),
                    dur(self.op_start)
                ),
                Err(e) => format!(
                    "validate{common} out={} layers={layers} result={}{}",
                    output.0,
                    error_code(e),
                    dur(self.op_start)
                ),
            },
            TraceEvent::Commit { output, result } => match result {
                Ok(sub) => {
                    self.committed += 1;
                    format!(
                        "commit{common} out={} result=ok sub={}{}",
                        output.0,
                        sub.0,
                        dur(self.op_start)
                    )
                }
                Err(e) => format!(
                    "commit{common} out={} result={}{}",
                    output.0,
                    error_code(e),
                    dur(self.op_start)
                ),
            },
            TraceEvent::Latched { output, submission } => {
                self.latched += 1;
                self.last_latched = Some(submission);
                latched = Some((output, submission));
                format!("latched{common} out={} sub={}", output.0, submission.0)
            }
            TraceEvent::Released { buffer, submission } => format!(
                "released{common} buf={} sub={}",
                buffer.0,
                submission.map_or("-".into(), |s| s.0.to_string())
            ),
            TraceEvent::Dropped {
                output,
                submission,
                reason,
            } => {
                self.dropped += 1;
                let reason = match reason {
                    DropReason::Replaced => "replaced",
                    DropReason::DeviceLost => "device_lost",
                };
                format!(
                    "dropped{common} out={} sub={} reason={reason}",
                    output.0, submission.0
                )
            }
            TraceEvent::LatchTimeout { output, submission } => {
                self.timeouts += 1;
                format!(
                    "latch_timeout{common} out={} sub={}",
                    output.0, submission.0
                )
            }
            TraceEvent::DeviceLost => {
                self.lost = true;
                format!("device_lost{common}")
            }
            TraceEvent::CapabilitiesChanged => format!("caps_changed{common}"),
        };
        self.entries.push(Entry {
            seq: self.seq,
            text,
            latched,
        });
        self.seq += 1;
    }
}

// --- Dry run -----------------------------------------------------------------

struct Buffers(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

/// The i.MX95 DPU topology as `crates/composition-core/tests/m2_scene.rs`
/// models it (`linux_dpu`, read from source and docs, not from a device):
/// primary + 5 overlays, zpos 0..5, plane alpha, three blend modes, linear,
/// upscale only, the four resource groups of the DPU profile. Only the
/// formats this scene needs are listed.
fn modelled_dpu(size: Size) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OutputId(0),
            size,
            refresh_mhz: 60_000,
        })
        .unwrap();
    for i in 0..6u32 {
        let mut p = PlaneCaps {
            id: PlaneId(i + 1),
            kind: if i == 0 {
                PlaneKind::Primary
            } else {
                PlaneKind::Overlay
            },
            outputs: 1,
            scaling: Some(ScaleLimits {
                max_upscale: 2048 << 16,
                max_downscale: 1 << 16,
            }),
            max_source: Some(Size::new(8192, 8192)),
            plane_alpha: true,
            blend_modes: BlendMode::Opaque.mask()
                | BlendMode::Premultiplied.mask()
                | BlendMode::Coverage.mask(),
            transforms: Transform::Normal.mask(),
            zpos: Zpos::Range { min: 0, max: 5 },
            ..PlaneCaps::default()
        };
        for f in [PixelFormat::ARGB8888, PixelFormat::XRGB8888] {
            p.formats.push(FormatModifier::linear(f)).unwrap();
        }
        caps.planes.push(p).unwrap();
    }
    let all = 0b11_1111;
    for (consumers, capacity) in [
        (LayerTraits::ANY, 6),
        (LayerTraits::SCALED.union(LayerTraits::YUV), 4),
        (LayerTraits::SCALED_H, 1),
        (LayerTraits::SCALED_V, 1),
    ] {
        caps.resource_groups
            .push(ResourceGroup {
                consumers,
                planes: all,
                capacity,
            })
            .unwrap();
    }
    caps
}

fn dry_run(options: &Options, clock: Clock) -> ExitCode {
    println!(
        "NOTE dry run: no device opened, nothing committed; size {}x{} is the docs' inferred panel mode",
        DOC_SIZE.width, DOC_SIZE.height
    );
    println!("DEVICE {} (not opened)", options.device);
    print_scene(DOC_SIZE);
    let caps = modelled_dpu(DOC_SIZE);
    let layers = scene(DOC_SIZE, 0);
    let buffers = Buffers(vec![
        (layers[0].buffer, buffer_info(DOC_SIZE)),
        (layers[1].buffer, buffer_info(top_size(DOC_SIZE))),
    ]);
    let frame = Frame {
        output: OutputId(0),
        layers: &layers,
    };
    match plan(&frame, &caps, &buffers) {
        Ok(p) => {
            println!(
                "PLAN model=imx95-dpu-m2_scene n={} assign={}",
                p.assignments().len(),
                assign_text(p.assignments())
            );
            println!(
                "PLANNED frames={} commit=nonblocking wait=page-flip-per-frame profile={:?} clock={}",
                options.frames,
                options.profile,
                clock.name()
            );
            println!("VERDICT m2scene: DONE dry run");
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!(
                "PLAN model=imx95-dpu-m2_scene result={}",
                error_code(e.into())
            );
            println!("VERDICT m2scene: FAIL the modelled topology refused the scene");
            ExitCode::from(1)
        }
    }
}

// --- Device run --------------------------------------------------------------

fn take_latches(
    engine: &mut Engine<DrmBackend, Sink>,
    latches: &mut HashMap<(OutputId, SubmissionId), LatchInfo>,
) {
    for info in engine.backend_mut().take_latches() {
        latches.insert((info.output, info.submission), info);
    }
}

/// Handles events until `submission` latched, was dropped, or the wait
/// timed out.
fn wait_latch(
    engine: &mut Engine<DrmBackend, Sink>,
    latches: &mut HashMap<(OutputId, SubmissionId), LatchInfo>,
    output: OutputId,
    submission: SubmissionId,
) -> Result<(), String> {
    let deadline = Instant::now() + LATCH_WAIT;
    loop {
        let now = engine.trace().clock.now();
        engine.advance_time(now);
        let mut ended = false;
        loop {
            match engine.poll_event() {
                Ok(Some(DisplayEvent::Presented {
                    output: o,
                    submission: s,
                })) if o == output && s >= submission => ended = true,
                Ok(Some(DisplayEvent::Dropped {
                    output: o,
                    submission: s,
                })) if o == output && s == submission => ended = true,
                Ok(Some(DisplayEvent::DeviceLost)) => {
                    take_latches(engine, latches);
                    engine.trace_mut().flush(latches);
                    return Err("device lost".into());
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(e) => return Err(format!("event error {e:?}")),
            }
        }
        take_latches(engine, latches);
        engine.trace_mut().flush(latches);
        if ended {
            return Ok(());
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(format!(
                "submission {} not latched within {LATCH_WAIT:?}",
                submission.0
            ));
        }
        engine
            .backend()
            .wait_for_events(Some(left))
            .map_err(|e| format!("waiting for events: {e}"))?;
    }
}

fn cleanup(engine: &mut Engine<DrmBackend, Sink>) {
    let mut kept = 0;
    for (base, top) in SLOTS {
        for id in [base, top] {
            match engine.buffer_state(id) {
                Some(BufferState::Submitted | BufferState::Scanning) => kept += 1,
                Some(_) => {
                    let _ = engine.unregister_buffer(id);
                    let _ = engine.backend_mut().remove_buffer(id);
                }
                None => {
                    let _ = engine.backend_mut().remove_buffer(id);
                }
            }
        }
    }
    if kept > 0 {
        println!("CLEANUP {kept} buffer(s) still on screen, freed when the device is closed");
    }
}

fn fail(reason: impl std::fmt::Display, status: u8) -> ExitCode {
    println!("VERDICT m2scene: FAIL {reason}");
    ExitCode::from(status)
}

fn device_run(options: &Options, clock: Clock) -> ExitCode {
    println!("DEVICE {}", options.device);
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    println!("KERNEL {kernel}");
    let drm_options = DrmOptions {
        scaling: None,
        profile: options.profile,
    };
    let backend = match DrmBackend::open(&options.device, drm_options) {
        Ok(backend) => backend,
        Err(error) => return fail(format!("cannot open {}: {error}", options.device), 2),
    };
    let driver = backend.driver_name().unwrap_or_else(|_| "unknown".into());
    println!("DRIVER {driver} profile {:?}", options.profile);
    if let Err(error) = backend.card().acquire_master_lock() {
        return fail(
            format!(
                "DRM master unavailable on {}: {error} (another process holds it; this example never stops other processes)",
                options.device
            ),
            2,
        );
    }

    let mut engine = Engine::new(backend).with_trace(Sink::new(clock));
    let caps = *engine.capabilities();
    let Some(output) = caps.outputs.get(options.output).copied() else {
        return fail(
            format!(
                "no output {} ({} connected)",
                options.output,
                caps.outputs.len()
            ),
            2,
        );
    };
    let size = output.size;
    println!(
        "MODE {}x{}@{}.{:03}Hz output {} (crtc {})",
        size.width,
        size.height,
        output.refresh_mhz / 1000,
        output.refresh_mhz % 1000,
        options.output,
        output.id.0
    );
    print_scene(size);
    println!("{}", engine.trace().header(&driver));

    let mut errors = Vec::new();
    'create: for (base, top) in SLOTS {
        for (id, info, pixel) in [
            (base, buffer_info(size), BASE_PIXEL),
            (top, buffer_info(top_size(size)), TOP_PIXEL),
        ] {
            let made = engine
                .backend_mut()
                .create_dumb_buffer(id, info)
                .map_err(|e| format!("dumb buffer {}: {e}", id.0))
                .and_then(|()| {
                    engine
                        .register_buffer(id, info)
                        .map_err(|e| format!("register {}: {e:?}", id.0))
                })
                .and_then(|()| {
                    engine
                        .backend_mut()
                        .draw_dumb_buffer(id, |pixels, pitch| fill(pixels, pitch, info.size, pixel))
                        .map_err(|e| format!("draw {}: {e}", id.0))
                })
                .and_then(|()| {
                    engine
                        .finish_write(id)
                        .map_err(|e| format!("finish {}: {e:?}", id.0))
                });
            if let Err(error) = made {
                errors.push(error);
                break 'create;
            }
        }
    }

    let mut latches = HashMap::new();
    if errors.is_empty() {
        for frame_no in 0..options.frames {
            let layers = scene(size, frame_no as usize % SLOTS.len());
            let frame = Frame {
                output: output.id,
                layers: &layers,
            };
            let now = clock.now();
            engine.advance_time(now);
            engine.trace_mut().op_start = Some(now);
            let plan = engine.validate(&frame);
            let now = clock.now();
            engine.trace_mut().op_start = Some(now);
            let committed = plan.and_then(|plan| engine.commit(&frame, &plan));
            engine.trace_mut().op_start = None;
            take_latches(&mut engine, &mut latches);
            engine.trace_mut().flush(&latches);
            let submission = match committed {
                Ok(submission) => submission,
                Err(error) => {
                    errors.push(format!("frame {frame_no}: {}", error_code(error)));
                    break;
                }
            };
            if let Err(error) = wait_latch(&mut engine, &mut latches, output.id, submission) {
                errors.push(format!("frame {frame_no}: {error}"));
                break;
            }
            // Keep the map small: only the latest latch matters from here on.
            latches.clear();
        }
    }

    // Commit an empty frame so that the last buffers come back.
    if engine.trace().committed > 0 && errors.is_empty() {
        let blank = Frame {
            output: output.id,
            layers: &[],
        };
        engine.advance_time(clock.now());
        match engine
            .validate(&blank)
            .and_then(|plan| engine.commit(&blank, &plan))
        {
            Ok(submission) => {
                if let Err(error) = wait_latch(&mut engine, &mut latches, output.id, submission) {
                    errors.push(format!("blank frame: {error}"));
                }
            }
            Err(error) => println!(
                "NOTE blank frame refused ({}); the last frame stays on screen until the device is closed",
                error_code(error)
            ),
        }
        take_latches(&mut engine, &mut latches);
        engine.trace_mut().flush(&latches);
    }
    cleanup(&mut engine);
    take_latches(&mut engine, &mut latches);
    engine.trace_mut().flush(&latches);

    let sink = engine.trace();
    println!(
        "RESULT frames={} committed={} latched={} dropped={} timeouts={}{}",
        options.frames,
        sink.committed,
        sink.latched,
        sink.dropped,
        sink.timeouts,
        if sink.lost { " device_lost=1" } else { "" }
    );
    if errors.is_empty() {
        println!("VERDICT m2scene: DONE");
        ExitCode::SUCCESS
    } else {
        fail(errors.join("; "), 1)
    }
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
    let clock = Clock::new();
    let (date, time) = utc_now();
    println!(
        "RUN date={date} side=linux stage=B tool=m2scene-{} time={time} frames={} mode={}",
        env!("CARGO_PKG_VERSION"),
        options.frames,
        if options.dry_run { "dry-run" } else { "device" }
    );
    if options.dry_run {
        dry_run(&options, clock)
    } else {
        device_run(&options, clock)
    }
}
