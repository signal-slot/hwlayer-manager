//! Two-plane producer harness: two independently backed layers on two real
//! hardware planes, driven through the shared engine API.
//!
//! ```text
//! cargo run -p composition-drm --example twoplane -- /dev/dri/cardN [options]
//!
//!   --frames N         frames to present (default 600; 0 skips the run)
//!   --buffers N        dumb buffers per layer, 2 or 3 (default 3)
//!   --output N         index of the output in the probed capabilities (default 0)
//!   --swap-z           swap the stacking of the two layers halfway through,
//!                      if both planes have a mutable zpos
//!   --unsupported      afterwards, run the list of unsupported combinations
//!   --blend MODE       overlay blend mode: none, premultiplied or coverage
//!                      (default: premultiplied if a plane offers it, else
//!                      coverage, else none)
//!   --plane-alpha A    overlay plane alpha, 0..=65535 (default 65535, opaque)
//!   --scaling UP,DOWN  assume every non-cursor plane scales by up to these
//!                      factors ([`DrmOptions::scaling`]), such as `4,2`
//!   --profile NAME     driver profile: generic (default) or imx95
//! ```
//!
//! Nothing is hard-coded about the device: the output, the planes and the
//! formats come from the probed [`Capabilities`], and the planner picks the
//! planes. The harness uses the shared API only: [`Engine`] over
//! [`DrmBackend`], buffers from [`DrmBackend::create_dumb_buffer`] and
//! [`DrmBackend::draw_dumb_buffer`], and `register_buffer`, `begin_write`,
//! `finish_write`, `validate`, `commit` and `poll_event` with
//! [`DrmBackend::wait_for_events`].
//!
//! # The run
//!
//! The bottom layer is a full-screen opaque background on the primary plane;
//! the top layer is a smaller ARGB window on an overlay plane. Each layer has
//! its own set of rotating dumb buffers. Every frame is validated (the
//! planner, then an atomic `TEST_ONLY` through the backend check), committed,
//! and waited for until it latches. The harness checks that every buffer is
//! released exactly once, and only after a later frame latched.
//!
//! Late frames are latches that came more than 1.5 refresh periods after the
//! previous one. Latch times are taken when the event is read, not from the
//! kernel's page-flip timestamp, so they include the harness's own latency.
//!
//! # The picture
//!
//! The background is dark grey with a red, a green and a blue block, and a
//! white bar that moves along a strip near the bottom. The overlay covers the
//! right half of the green block and some background to its right. Its upper
//! half is opaque yellow (z-order: it must hide the green block), its lower
//! half is magenta with pixel alpha 0x80, blended as the blend mode and plane
//! alpha say. A row of eight squares in the overlay's upper right counts the
//! frames in binary (white is 1).
//!
//! `GEOMETRY` lines give every region in output pixels. `EXPECT` lines give
//! sample points, as pixels and as fractions of the output size, with the
//! colour they must show; with `--swap-z` a second set gives the colours
//! after the swap, when the overlay is hidden below the background.
//!
//! # Unsupported combinations
//!
//! `--unsupported` builds each combination from the probed capabilities and
//! prints one line per combination:
//!
//! ```text
//! UNSUPPORTED <name>: caps=<accepts|refuses:reason> planner=<plan|error:...> test_only=<ok|rejected|errno|not reached>
//! ```
//!
//! Where the planner refuses and the backend can program what is asked
//! (formats, scaling), the frame is planned again against capabilities that
//! claim the missing feature, and that plan goes through the backend check:
//! the `TEST_ONLY` result is then the kernel's own answer.
//!
//! # Ending
//!
//! The last line is `VERDICT twoplane: PASS|FAIL <reason>`. The exit status
//! is 0 for a pass, 1 for a failure and 2 for a usage error or a device that
//! cannot be used.
//!
//! The harness needs DRM master. If another process (a desktop, a
//! compositor) holds it, the harness says so and stops; it never stops other
//! processes itself.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use composition_core::planner::{plan_with, BufferLookup};
use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, BufferState, Capabilities, DisplayBackend,
    DisplayEvent, Engine, Error, FormatModifier, Frame, Layer, Modifier, OutputCaps, PixelFormat,
    Plan, PlanError, PlaneCaps, PlaneKind, Rect, ScaleLimits, SearchLimits, Size, SubmissionId,
    Transform, Verdict, Zpos, ALPHA_OPAQUE,
};
use composition_drm::{DriverProfile, DrmBackend, DrmOptions};
use drm::Device as _;

const USAGE: &str = "usage: twoplane /dev/dri/cardN [--frames N] [--buffers 2|3] [--output N] \
[--swap-z] [--unsupported] [--blend none|premultiplied|coverage] [--plane-alpha A] \
[--scaling UP,DOWN] [--profile generic|imx95]";

/// A late latch came more than this many refresh periods after the previous.
const LATE_PERIODS: f64 = 1.5;
/// How long to wait for a latch before giving up on the run.
const LATCH_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rgb(u8, u8, u8);

impl Rgb {
    fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

const BACKGROUND: Rgb = Rgb(0x20, 0x20, 0x20);
const RED: Rgb = Rgb(0xff, 0x00, 0x00);
const GREEN: Rgb = Rgb(0x00, 0xff, 0x00);
const BLUE: Rgb = Rgb(0x00, 0x00, 0xff);
const WHITE: Rgb = Rgb(0xff, 0xff, 0xff);
const BLACK: Rgb = Rgb(0x00, 0x00, 0x00);
const YELLOW: Rgb = Rgb(0xff, 0xff, 0x00);
const MAGENTA: Rgb = Rgb(0xff, 0x00, 0xff);
/// Pixel alpha of the overlay's translucent half.
const PIXEL_ALPHA: u8 = 0x80;

/// Buffer identifiers: the backgrounds, the overlays, then the extra
/// buffers of the unsupported combinations.
const PRIMARY_IDS: u32 = 1;
const OVERLAY_IDS: u32 = 11;
const FORMAT_ID: BufferId = BufferId(100);
const MODIFIER_ID: BufferId = BufferId(101);

struct Options {
    device: String,
    frames: u32,
    buffers: usize,
    output: usize,
    swap_z: bool,
    unsupported: bool,
    blend: Option<BlendMode>,
    plane_alpha: u16,
    scaling: Option<ScaleLimits>,
    profile: DriverProfile,
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        device: "/dev/dri/card0".into(),
        frames: 600,
        buffers: 3,
        output: 0,
        swap_z: false,
        unsupported: false,
        blend: None,
        plane_alpha: ALPHA_OPAQUE,
        scaling: None,
        profile: DriverProfile::Generic,
    };
    let mut args = std::env::args().skip(1);
    let mut device = None;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--frames" => options.frames = number(&value("--frames")?)?,
            "--buffers" => {
                options.buffers = number(&value("--buffers")?)?;
                if !(2..=3).contains(&options.buffers) {
                    return Err("--buffers takes 2 or 3".into());
                }
            }
            "--output" => options.output = number(&value("--output")?)?,
            "--swap-z" => options.swap_z = true,
            "--unsupported" => options.unsupported = true,
            "--blend" => {
                options.blend = Some(match value("--blend")?.as_str() {
                    "none" => BlendMode::Opaque,
                    "premultiplied" => BlendMode::Premultiplied,
                    "coverage" => BlendMode::Coverage,
                    other => return Err(format!("unknown blend mode {other}")),
                })
            }
            "--plane-alpha" => options.plane_alpha = number(&value("--plane-alpha")?)?,
            "--scaling" => {
                let text = value("--scaling")?;
                let (up, down) = text
                    .split_once(',')
                    .ok_or("--scaling takes UP,DOWN, such as 4,2")?;
                let factor = |s: &str| -> Result<u32, String> {
                    let f: f64 = s.trim().parse().map_err(|_| format!("bad factor {s}"))?;
                    if !(1.0..=32768.0).contains(&f) {
                        return Err(format!("factor {s} is not in 1..=32768"));
                    }
                    Ok((f * 65536.0) as u32)
                };
                options.scaling = Some(ScaleLimits {
                    max_upscale: factor(up)?,
                    max_downscale: factor(down)?,
                });
            }
            "--profile" => {
                options.profile = match value("--profile")?.as_str() {
                    "generic" => DriverProfile::Generic,
                    "imx95" => DriverProfile::Imx95Dpu,
                    other => return Err(format!("unknown profile {other}")),
                }
            }
            "-h" | "--help" => return Err(String::new()),
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other if device.is_none() => device = Some(other.to_string()),
            other => return Err(format!("unexpected argument {other}")),
        }
    }
    if let Some(device) = device {
        options.device = device;
    }
    Ok(options)
}

fn number<T: std::str::FromStr>(text: &str) -> Result<T, String> {
    text.parse().map_err(|_| format!("bad number {text}"))
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

fn blend_name(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Opaque => "none",
        BlendMode::Premultiplied => "premultiplied",
        BlendMode::Coverage => "coverage",
    }
}

fn rect_text(r: Rect) -> String {
    format!("{},{} {}x{}", r.x, r.y, r.width, r.height)
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

/// The four bytes of one pixel, for the 32-bit RGB formats the harness draws.
fn encode(format: PixelFormat, c: Rgb, alpha: u8) -> Option<[u8; 4]> {
    if format == PixelFormat::XRGB8888 || format == PixelFormat::ARGB8888 {
        Some([c.2, c.1, c.0, alpha])
    } else if format == PixelFormat::XBGR8888 || format == PixelFormat::ABGR8888 {
        Some([c.0, c.1, c.2, alpha])
    } else {
        None
    }
}

fn has_alpha(format: PixelFormat) -> bool {
    format == PixelFormat::ARGB8888 || format == PixelFormat::ABGR8888
}

/// Fills `rect`, clipped to `size`, with one pixel value.
fn fill(pixels: &mut [u8], pitch: usize, size: Size, rect: Rect, pixel: [u8; 4]) {
    let clip = |start: i32, length: u32, limit: u32| {
        let end = (start as i64 + length as i64).clamp(0, limit as i64) as usize;
        (start.clamp(0, limit as i32) as usize, end)
    };
    let (x0, x1) = clip(rect.x, rect.width, size.width);
    let (y0, y1) = clip(rect.y, rect.height, size.height);
    if x0 >= x1 {
        return;
    }
    let row = pixel.repeat(x1 - x0);
    for y in y0..y1 {
        let start = y * pitch + x0 * 4;
        if let Some(target) = pixels.get_mut(start..start + row.len()) {
            target.copy_from_slice(&row);
        }
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width as i32).min(b.x + b.width as i32);
    let y1 = (a.y + a.height as i32).min(b.y + b.height as i32);
    Rect::new(x0, y0, (x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32)
}

fn contains(r: Rect, x: i32, y: i32) -> bool {
    x >= r.x && y >= r.y && x < r.x + r.width as i32 && y < r.y + r.height as i32
}

fn centre(r: Rect) -> (i32, i32) {
    (r.x + r.width as i32 / 2, r.y + r.height as i32 / 2)
}

fn offset(r: Rect, by: Rect) -> Rect {
    Rect::new(r.x + by.x, r.y + by.y, r.width, r.height)
}

/// How the overlay's pixels combine with the background.
#[derive(Clone, Copy)]
struct Look {
    format: PixelFormat,
    blend: BlendMode,
    plane_alpha: u16,
}

impl Look {
    /// Whether the hardware uses the pixel alpha at all.
    fn pixel_alpha(&self) -> bool {
        has_alpha(self.format) && self.blend != BlendMode::Opaque
    }

    /// Colour stored in the buffer for `colour` at pixel alpha `alpha`.
    fn stored(&self, colour: Rgb, alpha: u8) -> Rgb {
        if self.blend != BlendMode::Premultiplied || !self.pixel_alpha() {
            return colour;
        }
        let mul = |c: u8| ((c as u32 * alpha as u32 + 127) / 255) as u8;
        Rgb(mul(colour.0), mul(colour.1), mul(colour.2))
    }

    /// Colour on screen, by the KMS blending equations.
    fn over(&self, colour: Rgb, alpha: u8, under: Rgb) -> Rgb {
        let stored = self.stored(colour, alpha);
        let pa = self.plane_alpha as f64 / ALPHA_OPAQUE as f64;
        let fa = if self.pixel_alpha() {
            alpha as f64 / 255.0
        } else {
            1.0
        };
        let channel = |c: u8, u: u8| {
            let (c, u) = (c as f64, u as f64);
            let value = match self.blend {
                BlendMode::Opaque => pa * c + (1.0 - pa) * u,
                BlendMode::Premultiplied => pa * c + (1.0 - pa * fa) * u,
                BlendMode::Coverage => pa * fa * c + (1.0 - pa * fa) * u,
            };
            value.round().clamp(0.0, 255.0) as u8
        };
        Rgb(
            channel(stored.0, under.0),
            channel(stored.1, under.1),
            channel(stored.2, under.2),
        )
    }
}

/// Whether the z swap of `--swap-z` runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SwapPlan {
    /// Not asked for.
    Off,
    /// Asked for and possible.
    Runs,
    /// Asked for, but a plane of the plan has a fixed zpos.
    Skipped,
}

/// The swap can run when every plane of the plan has a settable zpos.
fn swap_plan(requested: bool, zpos: &[Zpos]) -> SwapPlan {
    if !requested {
        SwapPlan::Off
    } else if zpos.iter().all(|z| matches!(z, Zpos::Range { .. })) {
        SwapPlan::Runs
    } else {
        SwapPlan::Skipped
    }
}

/// The picture, in output pixels unless said otherwise.
struct Scene {
    output: Size,
    blocks: [(&'static str, Rect, Rgb); 3],
    /// Where the bar moves; plain background otherwise.
    strip: Rect,
    bar_width: u32,
    bar_step: u32,
    /// Overlay destination.
    overlay: Rect,
    /// Overlay regions, in overlay buffer pixels.
    opaque: Rect,
    translucent: Rect,
    counter: [Rect; 8],
}

impl Scene {
    fn new(output: Size) -> Self {
        let (w, h) = (output.width, output.height);
        let block = |x: u32| Rect::new(x as i32, (h / 8) as i32, w / 5, h / 2);
        let overlay = Rect::new(
            (w / 2) as i32,
            (h / 4) as i32,
            (w / 5).max(4),
            (h / 2).max(4),
        );
        let upper = overlay.height / 2;
        let half = overlay.width / 2;
        let square = (half / 12).max(1);
        let gap = (square / 4).max(1);
        let counter = std::array::from_fn(|k| {
            let x = half + gap + k as u32 * (square + gap);
            Rect::new(x as i32, gap as i32, square, square)
        });
        Self {
            output,
            blocks: [
                ("red", block(w / 10), RED),
                ("green", block(2 * w / 5), GREEN),
                ("blue", block(7 * w / 10), BLUE),
            ],
            strip: Rect::new(0, (h * 4 / 5) as i32, w, (h * 3 / 20).max(1)),
            bar_width: (w / 40).max(1),
            bar_step: (w / 120).max(1),
            overlay,
            opaque: Rect::new(0, 0, overlay.width, upper),
            translucent: Rect::new(0, upper as i32, overlay.width, overlay.height - upper),
            counter,
        }
    }

    fn overlay_size(&self) -> Size {
        Size::new(self.overlay.width, self.overlay.height)
    }

    fn bar(&self, frame: u32) -> Rect {
        let travel = self.strip.width.saturating_sub(self.bar_width).max(1);
        let x = (frame as u64 * self.bar_step as u64 % travel as u64) as i32;
        Rect::new(x, self.strip.y, self.bar_width, self.strip.height)
    }

    /// Background colour, ignoring the bar.
    fn background_at(&self, x: i32, y: i32) -> Rgb {
        self.blocks
            .iter()
            .find(|(_, r, _)| contains(*r, x, y))
            .map_or(BACKGROUND, |(_, _, c)| *c)
    }

    fn expected_at(&self, look: &Look, x: i32, y: i32, overlay_on_top: bool) -> Rgb {
        let under = self.background_at(x, y);
        if !overlay_on_top || !contains(self.overlay, x, y) {
            return under;
        }
        let local = (x - self.overlay.x, y - self.overlay.y);
        if contains(self.opaque, local.0, local.1) {
            look.over(YELLOW, 0xff, under)
        } else {
            look.over(MAGENTA, PIXEL_ALPHA, under)
        }
    }

    /// Named sample points away from edges, the bar and the counter.
    fn samples(&self) -> Vec<(&'static str, (i32, i32))> {
        let green = self.blocks[1].1;
        let green_end = green.x + green.width as i32;
        let overlay_end = self.overlay.x + self.overlay.width as i32;
        let opaque = offset(self.opaque, self.overlay);
        let translucent = offset(self.translucent, self.overlay);
        let right = |r: Rect| Rect::new(green_end, r.y, (overlay_end - green_end) as u32, r.height);
        let (w, h) = (self.output.width as i32, self.output.height as i32);
        vec![
            ("background", (w / 20, h * 7 / 10)),
            ("red block", centre(self.blocks[0].1)),
            ("blue block", centre(self.blocks[2].1)),
            (
                "green block uncovered",
                centre(Rect::new(
                    green.x,
                    green.y,
                    (self.overlay.x - green.x).max(1) as u32,
                    green.height,
                )),
            ),
            (
                "overlay opaque over green",
                centre(intersect(opaque, green)),
            ),
            ("overlay opaque over background", centre(right(opaque))),
            (
                "overlay translucent over green",
                centre(intersect(translucent, green)),
            ),
            (
                "overlay translucent over background",
                centre(right(translucent)),
            ),
        ]
    }

    fn print_geometry(&self, look: &Look, primary_format: PixelFormat) {
        let o = self.output;
        println!("GEOMETRY output {}x{}", o.width, o.height);
        println!(
            "GEOMETRY background layer 0 dst {} format {} rgb {}",
            rect_text(Rect::new(0, 0, o.width, o.height)),
            fourcc(primary_format),
            BACKGROUND.hex()
        );
        for (name, rect, colour) in &self.blocks {
            println!(
                "GEOMETRY block {name} {} rgb {}",
                rect_text(*rect),
                colour.hex()
            );
        }
        println!(
            "GEOMETRY bar strip {} bar {}x{} rgb {} moving {} px per frame",
            rect_text(self.strip),
            self.bar_width,
            self.strip.height,
            WHITE.hex(),
            self.bar_step
        );
        println!(
            "GEOMETRY overlay layer 1 dst {} format {} blend {} plane-alpha {:#06x}",
            rect_text(self.overlay),
            fourcc(look.format),
            blend_name(look.blend),
            look.plane_alpha
        );
        println!(
            "GEOMETRY overlay opaque {} colour {} pixel-alpha 0xff",
            rect_text(offset(self.opaque, self.overlay)),
            YELLOW.hex()
        );
        println!(
            "GEOMETRY overlay translucent {} colour {} pixel-alpha {:#04x}{}",
            rect_text(offset(self.translucent, self.overlay)),
            MAGENTA.hex(),
            PIXEL_ALPHA,
            if look.pixel_alpha() {
                ""
            } else {
                " (pixel alpha not used)"
            }
        );
        let first = offset(self.counter[0], self.overlay);
        println!(
            "GEOMETRY overlay counter 8 squares of {}x{} from {},{} step {} px, most significant bit left, white {} = 1, black {} = 0",
            first.width,
            first.height,
            first.x,
            first.y,
            self.counter[1].x - self.counter[0].x,
            WHITE.hex(),
            BLACK.hex()
        );
    }

    /// The `EXPECT` lines: the normal phase always, the swapped phase only
    /// when the swap runs, a marker line when it was asked for but cannot.
    fn expect_lines(&self, look: &Look, swap: SwapPlan) -> Vec<String> {
        let o = self.output;
        let phases: &[(&str, bool)] = if swap == SwapPlan::Runs {
            &[("normal", true), ("swapped", false)]
        } else {
            &[("normal", true)]
        };
        let mut lines = Vec::new();
        for (phase, on_top) in phases {
            for (name, (x, y)) in self.samples() {
                lines.push(format!(
                    "EXPECT phase={phase} \"{name}\" x={x} y={y} fx={:.4} fy={:.4} rgb={}",
                    x as f64 / o.width as f64,
                    y as f64 / o.height as f64,
                    self.expected_at(look, x, y, *on_top).hex()
                ));
            }
        }
        if swap == SwapPlan::Skipped {
            lines.push("EXPECT phase=swapped skipped reason=fixed-zpos".into());
        }
        lines
    }

    fn draw_background(&self, pixels: &mut [u8], pitch: usize, format: PixelFormat) {
        let px = |c| encode(format, c, 0xff).unwrap();
        fill(
            pixels,
            pitch,
            self.output,
            Rect::new(0, 0, self.output.width, self.output.height),
            px(BACKGROUND),
        );
        for (_, rect, colour) in &self.blocks {
            fill(pixels, pitch, self.output, *rect, px(*colour));
        }
    }

    fn draw_overlay(&self, pixels: &mut [u8], pitch: usize, look: &Look) {
        let size = self.overlay_size();
        let opaque = encode(look.format, YELLOW, 0xff).unwrap();
        let stored = look.stored(MAGENTA, PIXEL_ALPHA);
        let translucent = encode(look.format, stored, PIXEL_ALPHA).unwrap();
        fill(pixels, pitch, size, self.opaque, opaque);
        fill(pixels, pitch, size, self.translucent, translucent);
    }

    fn draw_counter(&self, pixels: &mut [u8], pitch: usize, look: &Look, frame: u32) {
        let size = self.overlay_size();
        for (bit, square) in self.counter.iter().enumerate() {
            let on = frame >> (7 - bit) & 1 != 0;
            let colour = if on { WHITE } else { BLACK };
            fill(
                pixels,
                pitch,
                size,
                *square,
                encode(look.format, colour, 0xff).unwrap(),
            );
        }
    }
}

/// Buffer metadata for planning outside the engine.
struct Lookup(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Lookup {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

/// Checks the buffer lifecycle from the display events.
#[derive(Default)]
struct Tracker {
    /// Buffers shown and not released yet, with the submission that last
    /// used them.
    outstanding: HashMap<BufferId, SubmissionId>,
    latest_presented: Option<SubmissionId>,
    committed: u64,
    uses: u64,
    latched: u64,
    released: u64,
    dropped: u64,
    latch_times: Vec<Instant>,
    errors: Vec<String>,
}

impl Tracker {
    fn submitted(&mut self, submission: SubmissionId, buffers: &[BufferId]) {
        self.committed += 1;
        for buffer in buffers {
            self.uses += 1;
            if let Some(previous) = self.outstanding.insert(*buffer, submission) {
                self.errors.push(format!(
                    "buffer {} submitted in {} while still held by {}",
                    buffer.0, submission.0, previous.0
                ));
            }
        }
    }

    fn event(&mut self, event: DisplayEvent) {
        match event {
            DisplayEvent::Presented { submission, .. } => {
                self.latched += 1;
                self.latest_presented = Some(submission);
                self.latch_times.push(Instant::now());
            }
            DisplayEvent::Released { buffer } => match self.outstanding.remove(&buffer) {
                None => self
                    .errors
                    .push(format!("buffer {} released twice or never shown", buffer.0)),
                Some(used) => {
                    self.released += 1;
                    if self.latest_presented.is_none_or(|p| p <= used) {
                        self.errors.push(format!(
                            "buffer {} of submission {} released before a later frame latched",
                            buffer.0, used.0
                        ));
                    }
                }
            },
            DisplayEvent::Dropped { submission, .. } => {
                self.dropped += 1;
                self.errors
                    .push(format!("submission {} dropped", submission.0));
            }
            DisplayEvent::DeviceLost => self.errors.push("device lost".into()),
            DisplayEvent::LatchTimeout { submission, .. } => self
                .errors
                .push(format!("submission {} timed out", submission.0)),
            DisplayEvent::CapabilitiesChanged => self
                .errors
                .push("capabilities changed during the run".into()),
        }
    }
}

/// Handles events until `submission` latched or the timeout passed.
fn wait_latch(
    engine: &mut Engine<DrmBackend>,
    tracker: &mut Tracker,
    submission: SubmissionId,
) -> Result<(), String> {
    let deadline = Instant::now() + LATCH_TIMEOUT;
    loop {
        while let Some(event) = engine
            .poll_event()
            .map_err(|e| format!("event error {e:?}"))?
        {
            tracker.event(event);
        }
        if tracker.latest_presented.is_some_and(|p| p >= submission) {
            return Ok(());
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(format!(
                "submission {} not latched within {LATCH_TIMEOUT:?}",
                submission.0
            ));
        }
        engine
            .backend()
            .wait_for_events(Some(left))
            .map_err(|e| format!("waiting for events: {e}"))?;
    }
}

fn describe_plan(plan: &Plan, caps: &Capabilities, names: &[&str]) -> Vec<String> {
    plan.assignments()
        .iter()
        .map(|a| {
            let kind = caps.planes.get(a.plane_index).map(|p| p.kind);
            format!(
                "layer {} ({}) -> plane {} {:?} zpos {} src {} dst {}",
                a.layer,
                names.get(a.layer).copied().unwrap_or("?"),
                a.plane.0,
                kind.unwrap_or_default(),
                a.zpos,
                rect_text(a.source),
                rect_text(a.destination)
            )
        })
        .collect()
}

fn mask_text(caps_bits: u8, modes: &[BlendMode]) -> String {
    modes
        .iter()
        .filter(|m| caps_bits & m.mask() != 0)
        .map(|m| blend_name(*m))
        .collect::<Vec<_>>()
        .join(",")
}

fn print_planes(caps: &Capabilities, output_index: usize) {
    for (index, plane) in caps.planes.iter().enumerate() {
        if plane.outputs & (1 << output_index) == 0 {
            continue;
        }
        let zpos = match plane.zpos {
            Zpos::Fixed(z) => format!("fixed {z}"),
            Zpos::Range { min, max } => format!("{min}..={max}"),
        };
        let formats: Vec<String> = plane
            .formats
            .iter()
            .map(|f| {
                if f.modifier == Modifier::LINEAR {
                    fourcc(f.format)
                } else {
                    format!("{}:{:#x}", fourcc(f.format), f.modifier.0)
                }
            })
            .collect();
        println!(
            "PLANE index {index} id {} {:?} zpos {zpos} plane-alpha {} blend [{}] scaling {} transforms {:#x} formats {}",
            plane.id.0,
            plane.kind,
            plane.plane_alpha,
            mask_text(
                plane.blend_modes,
                &[BlendMode::Opaque, BlendMode::Premultiplied, BlendMode::Coverage]
            ),
            plane.scaling.map_or("none".into(), |s| format!(
                "up {:.2} down {:.2}",
                s.max_upscale as f64 / 65536.0,
                s.max_downscale as f64 / 65536.0
            )),
            plane.transforms,
            formats.join(" ")
        );
    }
}

fn lists_linear(plane: &PlaneCaps, format: PixelFormat) -> bool {
    plane
        .formats
        .iter()
        .any(|f| f.format == format && f.modifier == Modifier::LINEAR)
}

/// What the backend check said about the plans the planner offered.
enum TestOnly {
    Ok,
    Rejected(u32),
    RejectedUntilLimit(u32),
    Failed(BackendError),
}

impl TestOnly {
    fn text(&self) -> String {
        match self {
            TestOnly::Ok => "ok".into(),
            TestOnly::Rejected(candidates) => {
                format!("rejected (EINVAL/ERANGE/ENOSPC, {candidates} candidate(s))")
            }
            TestOnly::RejectedUntilLimit(candidates) => format!(
                "rejected (EINVAL/ERANGE/ENOSPC, {candidates} candidate(s) until the search limit)"
            ),
            TestOnly::Failed(BackendError::Platform(errno)) => format!(
                "errno {errno} ({})",
                std::io::Error::from_raw_os_error(*errno)
            ),
            TestOnly::Failed(BackendError::Busy) => "errno EBUSY".into(),
            TestOnly::Failed(BackendError::DeviceLost) => {
                "errno EACCES/ENOENT/ENODEV (the backend now treats the device as lost)".into()
            }
            TestOnly::Failed(BackendError::UnknownBuffer(_)) => {
                "not reached (buffer not imported)".into()
            }
            TestOnly::Failed(BackendError::Rejected) => "rejected".into(),
        }
    }
}

/// Plans `frame` against `caps`, offering each plan to the backend check.
/// Returns the planner's verdict and, if a plan was found, what the check
/// said.
fn evaluate(
    engine: &mut Engine<DrmBackend>,
    caps: &Capabilities,
    lookup: &Lookup,
    frame: &Frame<'_>,
) -> Result<TestOnly, PlanError> {
    let backend = engine.backend_mut();
    let mut failure = None;
    let mut checked = 0;
    let planned = plan_with(frame, caps, lookup, SearchLimits::default(), |plan| {
        checked += 1;
        match backend.check(frame, plan) {
            Ok(()) => Verdict::Accept,
            Err(BackendError::Rejected) => Verdict::TryNext,
            Err(error) => {
                failure = Some(error);
                Verdict::GiveUp
            }
        }
    });
    match planned {
        Ok(_) => Ok(TestOnly::Ok),
        Err(PlanError::BackendRejected { candidates }) => Ok(TestOnly::Rejected(candidates)),
        // Legal plans existed, and the check refused every one it was
        // offered before the search limits ran out.
        Err(PlanError::LimitExceeded) if checked > 0 => Ok(TestOnly::RejectedUntilLimit(checked)),
        Err(PlanError::Aborted) => Ok(TestOnly::Failed(
            failure.unwrap_or(BackendError::Platform(0)),
        )),
        Err(error) => Err(error),
    }
}

fn plan_error_text(error: PlanError) -> String {
    format!("{error:?}").replace(' ', "")
}

/// What the forced attempt changes, if the planner refused.
enum Fallback<'a> {
    /// Plan again against these capabilities and run the backend check.
    Forced(Box<Capabilities>, &'a str),
    /// The backend check cannot be reached; why.
    NotReached(String),
}

/// Prints the `UNSUPPORTED` line of one combination.
#[allow(clippy::too_many_arguments)]
fn report(
    engine: &mut Engine<DrmBackend>,
    caps: &Capabilities,
    lookup: &Lookup,
    name: &str,
    caps_verdict: &str,
    layers: &[Layer],
    output: composition_core::OutputId,
    fallback: Fallback<'_>,
) {
    let frame = Frame { output, layers };
    let (planner, test_only) = match evaluate(engine, caps, lookup, &frame) {
        Ok(test_only) => ("plan".to_string(), test_only.text()),
        Err(error) => {
            let test_only = match fallback {
                Fallback::NotReached(why) => format!("not reached ({why})"),
                Fallback::Forced(forced, what) => match evaluate(engine, &forced, lookup, &frame) {
                    Ok(result) => format!("{} (hand-built plan: {what})", result.text()),
                    Err(error) => format!(
                        "not reached (hand-built plan with {what} still refused: {})",
                        plan_error_text(error)
                    ),
                },
            };
            (format!("error:{}", plan_error_text(error)), test_only)
        }
    };
    println!("UNSUPPORTED {name}: caps={caps_verdict} planner={planner} test_only={test_only}");
}

/// Everything the unsupported combinations are built from.
struct Setup<'a> {
    caps: Capabilities,
    output: composition_core::OutputId,
    output_index: usize,
    scene: &'a Scene,
    look: Look,
    background: Layer,
    overlay_buffer: BufferId,
}

impl Setup<'_> {
    fn eligible(&self) -> impl Iterator<Item = &PlaneCaps> + '_ {
        self.caps
            .planes
            .iter()
            .filter(|p| p.outputs & (1 << self.output_index) != 0)
    }

    fn overlay(&self, buffer: BufferId, source: Rect, destination: Rect) -> Layer {
        Layer {
            blend: self.look.blend,
            ..Layer::new(buffer, source, destination, 1)
        }
    }

    fn full_overlay(&self) -> Rect {
        Rect::new(0, 0, self.scene.overlay.width, self.scene.overlay.height)
    }

    /// The capabilities with every eligible, non-cursor plane changed.
    fn forced(&self, change: impl Fn(&mut PlaneCaps)) -> Box<Capabilities> {
        let mut forced = Box::new(self.caps);
        for plane in forced.planes.as_mut_slice() {
            if plane.outputs & (1 << self.output_index) != 0 && plane.kind != PlaneKind::Cursor {
                change(plane);
            }
        }
        forced
    }
}

fn run_unsupported(engine: &mut Engine<DrmBackend>, setup: &Setup<'_>, lookup: &mut Lookup) {
    let caps = &setup.caps;
    let scene = setup.scene;
    let output = setup.output;
    let bg = setup.background;
    let full = setup.full_overlay();
    let dst = scene.overlay;
    println!("UNSUPPORTED-BEGIN");

    // A format no plane advertises, preferring one a dumb buffer can hold.
    {
        let advertised = |f: PixelFormat| setup.eligible().any(|p| lists_linear(p, f));
        let dumb = [
            PixelFormat::RGB565,
            PixelFormat::XBGR8888,
            PixelFormat::ABGR8888,
            PixelFormat::XRGB8888,
            PixelFormat::ARGB8888,
        ];
        let (format, allocatable) = dumb
            .iter()
            .find(|f| !advertised(**f))
            .map(|f| (*f, true))
            .or((!advertised(PixelFormat::NV12)).then_some((PixelFormat::NV12, false)))
            .unwrap_or((PixelFormat::fourcc(b"HWL0"), false));
        let info = BufferInfo {
            size: scene.overlay_size(),
            format,
            modifier: Modifier::LINEAR,
        };
        lookup.0.push((FORMAT_ID, info));
        let imported = if allocatable {
            engine
                .backend_mut()
                .create_dumb_buffer(FORMAT_ID, info)
                .map_err(|e| format!("ADDFB2 refused {}: {e}", fourcc(format)))
        } else {
            Err(format!("dumb buffers cannot hold {}", fourcc(format)))
        };
        let blend = if format.is_known_opaque() {
            BlendMode::Opaque
        } else {
            setup.look.blend
        };
        let layer = Layer {
            blend,
            ..setup.overlay(FORMAT_ID, full, dst)
        };
        let fallback = match &imported {
            Ok(()) => Fallback::Forced(
                setup.forced(|p| {
                    let pair = FormatModifier::linear(format);
                    if p.formats.push(pair).is_err() {
                        let last = p.formats.len() - 1;
                        p.formats.as_mut_slice()[last] = pair;
                    }
                }),
                "format added to every plane's caps",
            ),
            Err(why) => Fallback::NotReached(why.clone()),
        };
        report(
            engine,
            caps,
            lookup,
            "format",
            &format!(
                "refuses:no plane on the output lists {} linear",
                fourcc(format)
            ),
            &[bg, layer],
            output,
            fallback,
        );
        if imported.is_ok() {
            let _ = engine.backend_mut().remove_buffer(FORMAT_ID);
        }
    }

    // A modifier no plane advertises for the overlay's format.
    {
        let format = setup.look.format;
        let candidates = [
            (Modifier(0x0100_0000_0000_0001), "I915_X_TILED"),
            (Modifier(0x0700_0000_0000_0001), "BROADCOM_VC4_T_TILED"),
            (Modifier(0x0600_0000_0000_0001), "VIVANTE_TILED"),
            (Modifier(0x0300_0000_0000_0001), "NVIDIA_TEGRA_TILED"),
        ];
        let advertised = |m: Modifier| {
            setup
                .eligible()
                .any(|p| p.formats.iter().any(|f| f.modifier == m))
        };
        match candidates.iter().find(|(m, _)| !advertised(*m)) {
            Some((modifier, name)) => {
                lookup.0.push((
                    MODIFIER_ID,
                    BufferInfo {
                        size: scene.overlay_size(),
                        format,
                        modifier: *modifier,
                    },
                ));
                report(
                    engine,
                    caps,
                    lookup,
                    "modifier",
                    &format!(
                        "refuses:no plane on the output lists {}:{name}",
                        fourcc(format)
                    ),
                    &[bg, setup.overlay(MODIFIER_ID, full, dst)],
                    output,
                    Fallback::NotReached(
                        "dumb buffers are linear, so no buffer of this layout can be imported"
                            .into(),
                    ),
                );
            }
            None => println!(
                "UNSUPPORTED modifier: caps=accepts (every candidate modifier is listed) planner=not run test_only=not reached"
            ),
        }
    }

    // More layers than the output has planes.
    {
        let planes = setup.eligible().count();
        let count = planes + 1;
        let (w, h) = (scene.output.width, scene.output.height);
        let tile = (w.min(h) / 10).clamp(1, full.width.min(full.height));
        let pitch = tile + tile / 2;
        let columns = (w / pitch).max(1);
        let mut layers = vec![bg];
        for k in 1..count as u32 {
            let x = (tile / 2 + (k - 1) % columns * pitch) as i32;
            let y = (tile / 2 + (k - 1) / columns * pitch).min(h - tile) as i32;
            layers.push(Layer {
                z_order: k as i32,
                ..setup.overlay(
                    setup.overlay_buffer,
                    Rect::new(0, 0, tile, tile),
                    Rect::new(x, y, tile, tile),
                )
            });
        }
        report(
            engine,
            caps,
            lookup,
            "layers",
            &format!("refuses:{count} layers for {planes} planes on the output"),
            &layers,
            output,
            Fallback::NotReached("no plan places more layers than there are planes".into()),
        );
    }

    // Scaling up and down.
    for (name, source, destination, up) in [
        (
            "scale-up",
            Rect::new(0, 0, (full.width / 2).max(1), (full.height / 2).max(1)),
            dst,
            true,
        ),
        (
            "scale-down",
            full,
            Rect::new(
                dst.x,
                dst.y,
                (dst.width / 2).max(1),
                (dst.height / 2).max(1),
            ),
            false,
        ),
    ] {
        let declared: Vec<ScaleLimits> = setup
            .eligible()
            .filter(|p| p.kind != PlaneKind::Cursor)
            .filter_map(|p| p.scaling)
            .collect();
        let enough = |s: &ScaleLimits| {
            if up {
                s.max_upscale >= 2 << 16
            } else {
                s.max_downscale >= 2 << 16
            }
        };
        let verdict = if declared.is_empty() {
            "refuses:no plane declares scaling".to_string()
        } else if declared.iter().any(enough) {
            "accepts".to_string()
        } else {
            "refuses:declared limits are below a factor of 2".to_string()
        };
        report(
            engine,
            caps,
            lookup,
            name,
            &verdict,
            &[bg, setup.overlay(setup.overlay_buffer, source, destination)],
            output,
            Fallback::Forced(
                setup.forced(|p| {
                    p.scaling = Some(ScaleLimits {
                        max_upscale: 16 << 16,
                        max_downscale: 16 << 16,
                    })
                }),
                "scaling by up to 16 claimed in the caps",
            ),
        );
    }

    // A rotation no plane advertises. composition-drm reports no transforms
    // yet, so in practice every one is missing.
    {
        let transforms = [
            Transform::Rotate180,
            Transform::Rotate90,
            Transform::Rotate270,
            Transform::FlipH,
            Transform::FlipV,
        ];
        let advertised = |t: Transform| setup.eligible().any(|p| p.transforms & t.mask() != 0);
        let missing = transforms.iter().copied().find(|t| !advertised(*t));
        let transform = missing.unwrap_or(Transform::Rotate180);
        let destination = if transform.swaps_axes() {
            Rect::new(dst.x, dst.y, dst.height, dst.width)
        } else {
            dst
        };
        let layer = Layer {
            transform,
            ..setup.overlay(setup.overlay_buffer, full, destination)
        };
        let verdict = match missing {
            Some(t) => format!("refuses:no plane on the output advertises {t:?}"),
            None => "accepts".into(),
        };
        report(
            engine,
            caps,
            lookup,
            "rotation",
            &verdict,
            &[bg, layer],
            output,
            Fallback::NotReached("composition-drm does not program the rotation property".into()),
        );
    }

    // Plane alpha.
    {
        let offered = setup.eligible().any(|p| p.plane_alpha);
        let layer = Layer {
            alpha: 0x8000,
            ..setup.overlay(setup.overlay_buffer, full, dst)
        };
        report(
            engine,
            caps,
            lookup,
            "plane-alpha",
            if offered {
                "accepts"
            } else {
                "refuses:no plane on the output has an alpha property"
            },
            &[bg, layer],
            output,
            Fallback::NotReached(if offered {
                "the planner refused".into()
            } else {
                "no alpha property to program".into()
            }),
        );
    }

    // A blend mode no plane offers; coverage if every one is offered.
    {
        let offered = setup.eligible().fold(0, |m, p| m | p.blend_modes);
        let mut order = vec![BlendMode::Coverage, BlendMode::Premultiplied];
        // Ignoring pixel alpha needs no support when the format has none.
        if has_alpha(setup.look.format) {
            order.push(BlendMode::Opaque);
        }
        let missing = order.iter().copied().find(|m| offered & m.mask() == 0);
        let blend = missing.unwrap_or(BlendMode::Coverage);
        let layer = Layer {
            blend,
            ..setup.overlay(setup.overlay_buffer, full, dst)
        };
        let verdict = match missing {
            Some(m) => format!(
                "refuses:no plane on the output offers blend {}",
                blend_name(m)
            ),
            None => "accepts".into(),
        };
        report(
            engine,
            caps,
            lookup,
            &format!("blend-{}", blend_name(blend)),
            &verdict,
            &[bg, layer],
            output,
            Fallback::NotReached(if missing.is_some() {
                "no pixel blend mode value to program".into()
            } else {
                "the planner refused".into()
            }),
        );
    }

    // A destination partly off the output.
    {
        let destination = Rect::new(
            scene.output.width as i32 - dst.width as i32 / 2,
            -(dst.height as i32 / 2),
            dst.width,
            dst.height,
        );
        report(
            engine,
            caps,
            lookup,
            "offscreen",
            "accepts (the planner clips to the output)",
            &[bg, setup.overlay(setup.overlay_buffer, full, destination)],
            output,
            Fallback::NotReached("the planner refused".into()),
        );
    }

    // An odd size at an odd offset, which the caps model has no rule against.
    {
        let odd = |n: u32| {
            if n % 2 == 1 {
                n
            } else {
                n.saturating_sub(1).max(1)
            }
        };
        let width = odd(full.width.saturating_sub(3).max(1));
        let height = odd(full.height.saturating_sub(3).max(1));
        let source = Rect::new(1, 1, width, height);
        let destination = Rect::new(dst.x | 1, dst.y | 1, width, height);
        report(
            engine,
            caps,
            lookup,
            "odd",
            "accepts (the caps model has no alignment rule)",
            &[bg, setup.overlay(setup.overlay_buffer, source, destination)],
            output,
            Fallback::NotReached("the planner refused".into()),
        );
    }
    println!("UNSUPPORTED-END");
}

fn fail(reason: impl std::fmt::Display) -> ExitCode {
    println!("VERDICT twoplane: FAIL {reason}");
    ExitCode::from(1)
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
    println!("DATE {}", utc_now());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    println!("KERNEL {kernel}");
    println!("DEVICE {}", options.device);

    let drm_options = DrmOptions {
        scaling: options.scaling,
        profile: options.profile,
    };
    let backend = match DrmBackend::open(&options.device, drm_options) {
        Ok(backend) => backend,
        Err(error) => {
            println!(
                "VERDICT twoplane: FAIL cannot open {}: {error}",
                options.device
            );
            return ExitCode::from(2);
        }
    };
    let driver = backend.driver_name().unwrap_or_else(|_| "unknown".into());
    match backend.card().get_driver() {
        Ok(info) => println!(
            "DRIVER {driver} version {}.{}.{} date {} desc {}",
            info.version.0,
            info.version.1,
            info.version.2,
            info.date().to_string_lossy(),
            info.description().to_string_lossy()
        ),
        Err(_) => println!("DRIVER {driver}"),
    }
    println!("PROFILE {:?}", options.profile);
    if let Err(error) = backend.card().acquire_master_lock() {
        println!(
            "DRM master is unavailable on {}: {error}. Another process (a desktop or compositor) \
             probably holds it: stop it for the run and restore it afterwards. This harness never \
             stops other processes.",
            options.device
        );
        println!("VERDICT twoplane: FAIL DRM master unavailable on {driver}");
        return ExitCode::from(2);
    }

    let mut engine = Engine::new(backend);
    let caps = *engine.capabilities();
    let Some(output) = caps.outputs.get(options.output).copied() else {
        return fail(format!(
            "driver {driver}: no output {} ({} connected)",
            options.output,
            caps.outputs.len()
        ));
    };
    let OutputCaps {
        id: output_id,
        size,
        refresh_mhz,
    } = output;
    let mode = format!(
        "{}x{}@{}.{:03}Hz",
        size.width,
        size.height,
        refresh_mhz / 1000,
        refresh_mhz % 1000
    );
    println!(
        "MODE {mode} output {} (crtc {}) of {}",
        options.output,
        output_id.0,
        caps.outputs.len()
    );
    print_planes(&caps, options.output);
    let summary = |verdict: &str| format!("{verdict} driver {driver} mode {mode}");

    // Formats from what the planes on this output list.
    let on_output = |kind: PlaneKind| {
        caps.planes
            .iter()
            .filter(move |p| p.outputs & (1 << options.output) != 0 && p.kind == kind)
    };
    let pick = |kind: PlaneKind, order: &[PixelFormat]| {
        order
            .iter()
            .copied()
            .find(|f| on_output(kind).any(|p| lists_linear(p, *f)))
    };
    if on_output(PlaneKind::Overlay).next().is_none() {
        return fail(summary("no overlay plane on the output:"));
    }
    let Some(primary_format) = pick(
        PlaneKind::Primary,
        &[
            PixelFormat::XRGB8888,
            PixelFormat::ARGB8888,
            PixelFormat::XBGR8888,
            PixelFormat::ABGR8888,
        ],
    ) else {
        return fail(summary("no primary plane with a linear 32-bit RGB format:"));
    };
    let Some(overlay_format) = pick(
        PlaneKind::Overlay,
        &[
            PixelFormat::ARGB8888,
            PixelFormat::ABGR8888,
            PixelFormat::XRGB8888,
            PixelFormat::XBGR8888,
        ],
    ) else {
        return fail(summary("no overlay plane with a linear 32-bit RGB format:"));
    };
    let overlay_modes = on_output(PlaneKind::Overlay).fold(0, |m, p| m | p.blend_modes);
    let blend = options.blend.unwrap_or(if !has_alpha(overlay_format) {
        BlendMode::Opaque
    } else if overlay_modes & BlendMode::Premultiplied.mask() != 0 {
        BlendMode::Premultiplied
    } else if overlay_modes & BlendMode::Coverage.mask() != 0 {
        BlendMode::Coverage
    } else {
        BlendMode::Opaque
    });
    let look = Look {
        format: overlay_format,
        blend,
        plane_alpha: options.plane_alpha,
    };
    // The background is opaque; a format with alpha carries 0xff everywhere,
    // so either mode gives the same picture.
    let primary_modes = on_output(PlaneKind::Primary).fold(0, |m, p| m | p.blend_modes);
    let primary_blend =
        if primary_format.is_known_opaque() || primary_modes & BlendMode::Opaque.mask() != 0 {
            BlendMode::Opaque
        } else {
            BlendMode::Premultiplied
        };

    let scene = Scene::new(size);
    let mut lookup = Lookup(Vec::new());
    let mut primary = Vec::new();
    let mut overlay = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for slot in 0..options.buffers as u32 {
        for (id, info, is_primary) in [
            (
                BufferId(PRIMARY_IDS + slot),
                BufferInfo {
                    size,
                    format: primary_format,
                    modifier: Modifier::LINEAR,
                },
                true,
            ),
            (
                BufferId(OVERLAY_IDS + slot),
                BufferInfo {
                    size: scene.overlay_size(),
                    format: overlay_format,
                    modifier: Modifier::LINEAR,
                },
                false,
            ),
        ] {
            if let Err(error) = engine.backend_mut().create_dumb_buffer(id, info) {
                errors.push(format!("dumb buffer {}: {error}", id.0));
                break;
            }
            lookup.0.push((id, info));
            let created = engine
                .register_buffer(id, info)
                .map_err(|e| format!("register {}: {e:?}", id.0))
                .and_then(|()| {
                    engine
                        .backend_mut()
                        .draw_dumb_buffer(id, |pixels, pitch| {
                            if is_primary {
                                scene.draw_background(pixels, pitch, primary_format);
                            } else {
                                scene.draw_overlay(pixels, pitch, &look);
                            }
                        })
                        .map_err(|e| format!("draw {}: {e}", id.0))
                })
                .and_then(|()| {
                    engine
                        .finish_write(id)
                        .map_err(|e| format!("finish {}: {e:?}", id.0))
                });
            if let Err(error) = created {
                errors.push(error);
                break;
            }
            if is_primary {
                primary.push((id, None::<Rect>));
            } else {
                overlay.push(id);
            }
        }
    }
    if !errors.is_empty() {
        cleanup(&mut engine, &lookup);
        return fail(summary(&format!("buffers: {}:", errors.join("; "))));
    }
    println!(
        "BUFFERS {} per layer: background {} {}x{}, overlay {} {}x{}",
        options.buffers,
        fourcc(primary_format),
        size.width,
        size.height,
        fourcc(overlay_format),
        scene.overlay.width,
        scene.overlay.height
    );

    let names = ["background", "overlay"];
    let mut z = [0, 1];
    let mut swap_pending = options.swap_z && options.frames > 1;
    scene.print_geometry(&look, primary_format);

    let mut tracker = Tracker::default();
    let mut previous_plan: Option<Vec<(usize, u32, i32)>> = None;
    let mut first_plan_checked = false;
    let started = Instant::now();
    let mut swapped_at = None;
    for frame_no in 0..options.frames {
        if swap_pending && frame_no == options.frames / 2 {
            swap_pending = false;
            z = [1, 0];
            swapped_at = Some(frame_no);
            println!("SWAPZ at frame {frame_no}: overlay now below the background");
        }
        let slot = frame_no as usize % options.buffers;
        let (primary_id, last_bar) = primary[slot];
        let overlay_id = overlay[slot];
        let bar = scene.bar(frame_no);
        let drawn = engine
            .begin_write(primary_id)
            .map_err(|e| format!("frame {frame_no}: begin_write {}: {e:?}", primary_id.0))
            .and_then(|()| {
                engine
                    .backend_mut()
                    .draw_dumb_buffer(primary_id, |pixels, pitch| {
                        if let Some(old) = last_bar {
                            let px = encode(primary_format, BACKGROUND, 0xff).unwrap();
                            fill(pixels, pitch, size, old, px);
                        }
                        let px = encode(primary_format, WHITE, 0xff).unwrap();
                        fill(pixels, pitch, size, bar, px);
                    })
                    .map_err(|e| format!("frame {frame_no}: draw: {e}"))
            })
            .and_then(|()| {
                engine
                    .finish_write(primary_id)
                    .map_err(|e| format!("frame {frame_no}: finish_write: {e:?}"))
            })
            .and_then(|()| {
                engine
                    .begin_write(overlay_id)
                    .map_err(|e| format!("frame {frame_no}: begin_write {}: {e:?}", overlay_id.0))
            })
            .and_then(|()| {
                engine
                    .backend_mut()
                    .draw_dumb_buffer(overlay_id, |pixels, pitch| {
                        scene.draw_counter(pixels, pitch, &look, frame_no)
                    })
                    .map_err(|e| format!("frame {frame_no}: draw: {e}"))
            })
            .and_then(|()| {
                engine
                    .finish_write(overlay_id)
                    .map_err(|e| format!("frame {frame_no}: finish_write: {e:?}"))
            });
        if let Err(error) = drawn {
            errors.push(error);
            break;
        }
        primary[slot].1 = Some(bar);

        let layers = [
            Layer {
                blend: primary_blend,
                ..Layer::new(
                    primary_id,
                    Rect::new(0, 0, size.width, size.height),
                    Rect::new(0, 0, size.width, size.height),
                    z[0],
                )
            },
            Layer {
                alpha: look.plane_alpha,
                blend: look.blend,
                ..Layer::new(
                    overlay_id,
                    Rect::new(0, 0, scene.overlay.width, scene.overlay.height),
                    scene.overlay,
                    z[1],
                )
            },
        ];
        let frame = Frame {
            output: output_id,
            layers: &layers,
        };
        let plan = match engine.validate(&frame) {
            Ok(plan) => plan,
            Err(error) => {
                errors.push(format!("frame {frame_no}: validate: {error:?}"));
                break;
            }
        };
        let key: Vec<(usize, u32, i32)> = plan
            .assignments()
            .iter()
            .map(|a| (a.layer, a.plane.0, a.zpos))
            .collect();
        if previous_plan.as_ref() != Some(&key) {
            println!("PLAN from frame {frame_no}:");
            for line in describe_plan(&plan, &caps, &names) {
                println!("  {line}");
            }
            previous_plan = Some(key);
        }
        let assignments = plan.assignments();
        if assignments.len() != 2 || assignments[0].plane == assignments[1].plane {
            errors.push(format!(
                "frame {frame_no}: {} assignment(s), not two layers on two planes",
                assignments.len()
            ));
            break;
        }
        if !first_plan_checked {
            first_plan_checked = true;
            let kind = |i: usize| caps.planes.get(assignments[i].plane_index).map(|p| p.kind);
            if assignments[0].layer != 0 || kind(0) != Some(PlaneKind::Primary) {
                errors.push("the background is not on the primary plane".into());
            }
            if kind(1) == Some(PlaneKind::Primary) {
                errors.push("the overlay is on a primary plane".into());
            }
            let zpos: Vec<Zpos> = assignments
                .iter()
                .filter_map(|a| caps.planes.get(a.plane_index).map(|p| p.zpos))
                .collect();
            let swap = swap_plan(swap_pending, &zpos);
            if swap == SwapPlan::Skipped {
                println!("SWAPZ skipped: a plane of the plan has a fixed zpos");
                swap_pending = false;
            }
            for line in scene.expect_lines(&look, swap) {
                println!("{line}");
            }
        }
        let submission = match engine.commit(&frame, &plan) {
            Ok(submission) => submission,
            Err(Error::Backend(BackendError::DeviceLost)) => {
                errors.push(format!(
                    "frame {frame_no}: commit: device lost (DRM master or lease gone?)"
                ));
                break;
            }
            Err(error) => {
                errors.push(format!("frame {frame_no}: commit: {error:?}"));
                break;
            }
        };
        tracker.submitted(submission, &[primary_id, overlay_id]);
        if let Err(error) = wait_latch(&mut engine, &mut tracker, submission) {
            errors.push(format!("frame {frame_no}: {error}"));
            break;
        }
    }
    let elapsed = started.elapsed();

    // Intervals between consecutive latches: the modeset before the first
    // latch is not counted.
    let period_ms = if refresh_mhz > 0 {
        1_000_000.0 / refresh_mhz as f64
    } else {
        println!("NOTE refresh rate unknown, late frames counted against 60 Hz");
        1000.0 / 60.0
    };
    let intervals: Vec<f64> = tracker
        .latch_times
        .windows(2)
        .map(|w| (w[1] - w[0]).as_secs_f64() * 1000.0)
        .collect();
    let late = intervals
        .iter()
        .filter(|i| **i > period_ms * LATE_PERIODS)
        .count();
    let mean = if intervals.is_empty() {
        0.0
    } else {
        intervals.iter().sum::<f64>() / intervals.len() as f64
    };
    let max = intervals.iter().copied().fold(0.0, f64::max);
    let run_latched = tracker.latched;
    let run_released = tracker.released;
    println!(
        "RESULT frames={} committed={} latched={} released={} dropped={} late={} period_ms={:.3} latch_interval_mean_ms={:.3} latch_interval_max_ms={:.3} elapsed_s={:.3}{}",
        options.frames,
        tracker.committed,
        run_latched,
        run_released,
        tracker.dropped,
        late,
        period_ms,
        mean,
        max,
        elapsed.as_secs_f64(),
        swapped_at.map_or(String::new(), |f| format!(" swapped_at={f}"))
    );

    // Blank the output so that the last frame's buffers come back too. Not
    // every driver allows an active output without a primary plane; then
    // they stay on screen until the device is closed.
    let mut blank_note = String::new();
    if tracker.committed > 0 && errors.is_empty() {
        let blank = Frame {
            output: output_id,
            layers: &[],
        };
        match engine
            .validate(&blank)
            .and_then(|plan| engine.commit(&blank, &plan))
        {
            Ok(submission) => {
                if let Err(error) = wait_latch(&mut engine, &mut tracker, submission) {
                    errors.push(format!("blank frame: {error}"));
                }
            }
            Err(error) => {
                blank_note = format!(
                    " (blank frame refused: {error:?}; the last frame stays on screen until the device is closed)"
                );
            }
        }
        let held = tracker.outstanding.len() as u64;
        println!(
            "RELEASE after the run: released {} of {} buffer uses, {held} still on screen{blank_note}",
            tracker.released, tracker.uses
        );
        if blank_note.is_empty() && held != 0 {
            errors.push(format!("{held} buffer(s) never released"));
        }
    }

    if options.unsupported {
        let setup = Setup {
            caps,
            output: output_id,
            output_index: options.output,
            scene: &scene,
            look,
            background: Layer {
                blend: primary_blend,
                ..Layer::new(
                    primary[0].0,
                    Rect::new(0, 0, size.width, size.height),
                    Rect::new(0, 0, size.width, size.height),
                    0,
                )
            },
            overlay_buffer: overlay[0],
        };
        run_unsupported(&mut engine, &setup, &mut lookup);
    }

    cleanup(&mut engine, &lookup);
    errors.append(&mut tracker.errors);

    if errors.is_empty() && options.frames == 0 {
        println!(
            "VERDICT twoplane: PASS no frames requested, nothing presented, {}",
            summary("").trim_start()
        );
        ExitCode::SUCCESS
    } else if errors.is_empty() && tracker.committed == u64::from(options.frames) {
        println!(
            "VERDICT twoplane: PASS {} frames on two planes, {run_latched} latched, {run_released} released, {late} late, {}",
            options.frames,
            summary("")
                .trim_start()
        );
        ExitCode::SUCCESS
    } else {
        for error in &errors {
            println!("ERROR {error}");
        }
        let first = errors
            .first()
            .cloned()
            .unwrap_or_else(|| "incomplete run".into());
        fail(format!(
            "{first} ({} error(s)), {}",
            errors.len(),
            summary("").trim_start()
        ))
    }
}

/// Removes every buffer the display no longer uses, from the engine and the
/// device. Buffers still on screen are freed when the device is closed.
fn cleanup(engine: &mut Engine<DrmBackend>, lookup: &Lookup) {
    let mut kept = 0;
    for (id, _) in &lookup.0 {
        if *id == FORMAT_ID || *id == MODIFIER_ID {
            continue;
        }
        match engine.buffer_state(*id) {
            Some(BufferState::Submitted | BufferState::Scanning) => kept += 1,
            Some(_) => {
                let _ = engine.unregister_buffer(*id);
                let _ = engine.backend_mut().remove_buffer(*id);
            }
            // Created on the device but never registered.
            None => {
                let _ = engine.backend_mut().remove_buffer(*id);
            }
        }
    }
    if kept > 0 {
        println!("CLEANUP {kept} buffer(s) still on screen, freed when the device is closed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_and_look() -> (Scene, Look) {
        let look = Look {
            format: PixelFormat::ARGB8888,
            blend: BlendMode::Premultiplied,
            plane_alpha: ALPHA_OPAQUE,
        };
        (Scene::new(Size::new(800, 600)), look)
    }

    const RANGE: Zpos = Zpos::Range { min: 0, max: 3 };

    fn count(lines: &[String], prefix: &str) -> usize {
        lines.iter().filter(|l| l.starts_with(prefix)).count()
    }

    #[test]
    fn swap_plan_needs_settable_zpos_on_every_plane() {
        assert_eq!(swap_plan(false, &[RANGE, RANGE]), SwapPlan::Off);
        assert_eq!(swap_plan(false, &[Zpos::Fixed(0)]), SwapPlan::Off);
        assert_eq!(swap_plan(true, &[RANGE, RANGE]), SwapPlan::Runs);
        assert_eq!(swap_plan(true, &[Zpos::Fixed(0), RANGE]), SwapPlan::Skipped);
    }

    #[test]
    fn expect_lines_swap_possible_gives_both_phases() {
        let (scene, look) = scene_and_look();
        let lines = scene.expect_lines(&look, swap_plan(true, &[RANGE, RANGE]));
        let normal = count(&lines, "EXPECT phase=normal ");
        assert!(normal > 0);
        assert_eq!(count(&lines, "EXPECT phase=swapped "), normal);
        assert_eq!(count(&lines, "EXPECT phase=swapped skipped"), 0);
    }

    #[test]
    fn expect_lines_fixed_zpos_has_no_unmarked_swapped_block() {
        let (scene, look) = scene_and_look();
        let lines = scene.expect_lines(&look, swap_plan(true, &[Zpos::Fixed(0), RANGE]));
        assert!(count(&lines, "EXPECT phase=normal ") > 0);
        let swapped: Vec<_> = lines
            .iter()
            .filter(|l| l.starts_with("EXPECT phase=swapped"))
            .collect();
        assert_eq!(swapped, ["EXPECT phase=swapped skipped reason=fixed-zpos"]);
    }

    #[test]
    fn expect_lines_normal_phase_is_the_same_in_every_case() {
        let (scene, look) = scene_and_look();
        let normal = |swap| {
            scene
                .expect_lines(&look, swap)
                .into_iter()
                .filter(|l| l.starts_with("EXPECT phase=normal "))
                .collect::<Vec<_>>()
        };
        assert_eq!(normal(SwapPlan::Off), normal(SwapPlan::Runs));
        assert_eq!(normal(SwapPlan::Off), normal(SwapPlan::Skipped));
    }

    #[test]
    fn expect_lines_without_swap_have_only_the_normal_phase() {
        let (scene, look) = scene_and_look();
        let lines = scene.expect_lines(&look, SwapPlan::Off);
        assert_eq!(count(&lines, "EXPECT phase=swapped"), 0);
    }
}
