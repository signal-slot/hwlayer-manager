//! Writeback check: two independently backed layers on two planes of a
//! writeback-capable CRTC, composed by the display hardware into a writeback
//! buffer, read back and compared pixel by pixel.
//!
//! ```text
//! cargo run -p composition-drm --example writeback -- /dev/dri/cardN [options]
//!
//!   --profile NAME     driver profile: generic (default) or imx95
//!   --tolerance N      largest channel difference that still passes for
//!                      opaque samples (default 2); blended samples get N + 2
//! ```
//!
//! For boards without a screen capture. The device needs a writeback
//! connector (vkms and vc4's TXP have one) and DRM master. What the check sees
//! is the CRTC's composition as the writeback connector receives it, not the
//! picture on a panel or a cable: report it as writeback evidence.
//!
//! # The run
//!
//! The writeback connector, its CRTCs and its pixel formats are found with the
//! `WRITEBACK_CONNECTORS` client capability, set on the descriptor before the
//! [`DrmBackend`] takes it over. The engine output is the one whose CRTC the
//! writeback connector can feed: on vkms the CRTC of the virtual display, on
//! vc4 the TXP's own CRTC, which the backend then drives as an output of its
//! own.
//!
//! Each step presents a frame through the shared API only ([`Engine`] over
//! [`DrmBackend`]: `validate`, which runs the planner and the atomic
//! `TEST_ONLY` check, then `commit`, then the latch and release events). The
//! background is a full-output opaque layer, the overlay a smaller ARGB layer
//! whose upper half has pixel alpha 0xff and lower half 0x80, each layer on
//! two dumb buffers used in turn. Once the frame has latched, a second, blocking
//! atomic commit made on [`DrmBackend::card`] sets `WRITEBACK_FB_ID` (and the
//! connector's `CRTC_ID`) and leaves the planes as they are, so the writeback
//! job captures the frame just presented. A further blocking commit waits one
//! more frame, then the buffer, filled with a sentinel colour beforehand, is
//! read until the sentinel is gone. `WRITEBACK_OUT_FENCE_PTR` is not used: it
//! needs a user-space address in a property value, which safe code cannot
//! give.
//!
//! The steps:
//!
//! - `above`: overlay above the background.
//! - `swapped`: the same layers with the overlay below the background, when
//!   the planner and `TEST_ONLY` accept that on two planes; else skipped.
//! - `alpha`: the overlay with plane alpha 0x8000 and the coverage blend
//!   mode, when a plane offers both; else skipped.
//!
//! # Output
//!
//! One line per sample:
//!
//! ```text
//! CHECK <name>: expected #rrggbb got #rrggbb maxdiff N tolerance T PASS|FAIL
//! CHECK <name>: SKIP <reason>
//! ```
//!
//! `got` is the pixel at the sample point, `maxdiff` the largest channel
//! difference from the expected colour over the 5x5 pixels around it. The
//! last line is `VERDICT writeback: PASS|FAIL|SKIP ...` with the driver and the
//! mode. Without a writeback connector it is `VERDICT writeback: SKIP no
//! writeback connector`. The exit status is 0 for a pass or a skip, 1 for a
//! failure and 2 for a usage error or a device that cannot be used.
//!
//! The example never stops other processes: if a desktop or compositor holds
//! DRM master, it says so and stops.

#![forbid(unsafe_code)]

use std::fs::File;
use std::os::fd::{AsFd, BorrowedFd};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use composition_core::{
    BlendMode, BufferId, BufferInfo, BufferState, Capabilities, DisplayEvent, Engine, Error, Frame,
    Layer, Modifier, PixelFormat, PlaneKind, Rect, Size, SubmissionId, Zpos, ALPHA_OPAQUE,
};
use composition_drm::{DriverProfile, DrmBackend, DrmOptions};
use drm::buffer::{DrmFourcc, DrmModifier, PlanarBuffer};
use drm::control::atomic::AtomicModeReq;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    connector, crtc, framebuffer, property, AtomicCommitFlags, Device as _, FbCmd2Flags,
};
use drm::{ClientCapability, Device as _};

const USAGE: &str = "usage: writeback /dev/dri/cardN [--profile generic|imx95] [--tolerance N]";

/// How long to wait for a latch, and for the writeback buffer to fill.
const TIMEOUT: Duration = Duration::from_secs(1);
/// Plane alpha of the `alpha` step.
const HALF: u16 = 0x8000;
/// Pixel alpha of the overlay's lower half.
const PIXEL_ALPHA: u8 = 0x80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rgb(u8, u8, u8);

impl Rgb {
    fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    fn max_diff(self, other: Rgb) -> u8 {
        self.0
            .abs_diff(other.0)
            .max(self.1.abs_diff(other.1))
            .max(self.2.abs_diff(other.2))
    }
}

/// Colours away from 0 and 0xff, so that clamping cannot hide an error.
const BACKGROUND: Rgb = Rgb(0x30, 0x60, 0x90);
const OVERLAY: Rgb = Rgb(0xe0, 0xc0, 0x20);
/// Written into the writeback buffer before each capture.
const SENTINEL: Rgb = Rgb(0x01, 0x02, 0x03);

/// Buffer identifiers: two backgrounds, two overlays.
const BACKGROUND_IDS: [BufferId; 2] = [BufferId(1), BufferId(2)];
const OVERLAY_IDS: [BufferId; 2] = [BufferId(11), BufferId(12)];

struct Options {
    device: String,
    profile: DriverProfile,
    tolerance: u8,
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        device: "/dev/dri/card0".into(),
        profile: DriverProfile::Generic,
        tolerance: 2,
    };
    let mut args = std::env::args().skip(1);
    let mut device = None;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--profile" => {
                options.profile = match value("--profile")?.as_str() {
                    "generic" => DriverProfile::Generic,
                    "imx95" => DriverProfile::Imx95Dpu,
                    other => return Err(format!("unknown profile {other}")),
                }
            }
            "--tolerance" => {
                let text = value("--tolerance")?;
                options.tolerance = text
                    .parse()
                    .ok()
                    .filter(|t| *t <= 250)
                    .ok_or(format!("bad tolerance {text}"))?;
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

/// The four bytes of one pixel, for the 32-bit RGB formats used here.
fn encode(format: PixelFormat, c: Rgb, alpha: u8) -> Option<[u8; 4]> {
    if format == PixelFormat::XRGB8888 || format == PixelFormat::ARGB8888 {
        Some([c.2, c.1, c.0, alpha])
    } else if format == PixelFormat::XBGR8888 || format == PixelFormat::ABGR8888 {
        Some([c.0, c.1, c.2, alpha])
    } else {
        None
    }
}

/// The colour of a pixel of `format`; alpha is ignored.
fn decode(format: PixelFormat, bytes: [u8; 4]) -> Rgb {
    if format == PixelFormat::XBGR8888 || format == PixelFormat::ABGR8888 {
        Rgb(bytes[0], bytes[1], bytes[2])
    } else {
        Rgb(bytes[2], bytes[1], bytes[0])
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

/// Colour of `colour` at pixel alpha `alpha` over `under`, by the KMS
/// blending equations, with straight (not premultiplied) colour in the
/// buffer for [`BlendMode::Coverage`].
fn over(blend: BlendMode, plane_alpha: u16, colour: Rgb, alpha: u8, under: Rgb) -> Rgb {
    let pa = plane_alpha as f64 / ALPHA_OPAQUE as f64;
    let fa = alpha as f64 / 255.0;
    let channel = |c: u8, u: u8| {
        let (c, u) = (c as f64, u as f64);
        let value = match blend {
            BlendMode::Opaque => pa * c + (1.0 - pa) * u,
            BlendMode::Premultiplied => pa * c + (1.0 - pa * fa) * u,
            BlendMode::Coverage => pa * fa * c + (1.0 - pa * fa) * u,
        };
        value.round().clamp(0.0, 255.0) as u8
    };
    Rgb(
        channel(colour.0, under.0),
        channel(colour.1, under.1),
        channel(colour.2, under.2),
    )
}

/// The picture, in output pixels.
struct Scene {
    output: Size,
    overlay: Rect,
}

impl Scene {
    fn new(output: Size) -> Self {
        let (w, h) = (output.width, output.height);
        Self {
            output,
            overlay: Rect::new(
                (w / 4) as i32,
                (h / 4) as i32,
                (w / 2).max(8),
                (h / 2).max(8),
            ),
        }
    }

    fn overlay_size(&self) -> Size {
        Size::new(self.overlay.width, self.overlay.height)
    }

    /// Upper half of the overlay buffer, in buffer pixels.
    fn upper(&self) -> Rect {
        Rect::new(0, 0, self.overlay.width, self.overlay.height / 2)
    }

    fn lower(&self) -> Rect {
        let upper = self.overlay.height / 2;
        Rect::new(
            0,
            upper as i32,
            self.overlay.width,
            self.overlay.height - upper,
        )
    }

    /// Sample points: outside the overlay, in its upper and its lower half.
    fn background_point(&self) -> (i32, i32) {
        (
            (self.output.width / 8) as i32,
            (self.output.height / 8) as i32,
        )
    }

    fn upper_point(&self) -> (i32, i32) {
        let r = self.upper();
        (
            self.overlay.x + r.width as i32 / 2,
            self.overlay.y + r.y + r.height as i32 / 2,
        )
    }

    fn lower_point(&self) -> (i32, i32) {
        let r = self.lower();
        (
            self.overlay.x + r.width as i32 / 2,
            self.overlay.y + r.y + r.height as i32 / 2,
        )
    }

    fn draw_background(&self, pixels: &mut [u8], pitch: usize, format: PixelFormat) {
        let px = encode(format, BACKGROUND, 0xff).unwrap();
        let all = Rect::new(0, 0, self.output.width, self.output.height);
        fill(pixels, pitch, self.output, all, px);
    }

    fn draw_overlay(&self, pixels: &mut [u8], pitch: usize, format: PixelFormat) {
        let size = self.overlay_size();
        fill(
            pixels,
            pitch,
            size,
            self.upper(),
            encode(format, OVERLAY, 0xff).unwrap(),
        );
        fill(
            pixels,
            pitch,
            size,
            self.lower(),
            encode(format, OVERLAY, PIXEL_ALPHA).unwrap(),
        );
    }
}

/// The descriptor before the backend takes it over.
struct Raw(File);

impl AsFd for Raw {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for Raw {}
impl drm::control::Device for Raw {}

/// A dumb buffer as a framebuffer of any 32-bit format.
struct Planar {
    size: (u32, u32),
    format: DrmFourcc,
    pitch: u32,
    handle: drm::buffer::Handle,
}

impl PlanarBuffer for Planar {
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

/// The writeback connector and what it needs.
struct Writeback {
    connector: connector::Handle,
    name: String,
    crtcs: Vec<crtc::Handle>,
    formats: Vec<PixelFormat>,
    fb_id: property::Handle,
    crtc_id: property::Handle,
}

/// Named properties with their handles and current values.
fn properties<D: drm::control::Device, H: drm::control::ResourceHandle>(
    card: &D,
    handle: H,
) -> std::io::Result<Vec<(String, property::Handle, u64)>> {
    let set = card.get_properties(handle)?;
    let mut named = Vec::new();
    for (prop, value) in set.iter() {
        let info = card.get_property(*prop)?;
        named.push((
            info.name().to_string_lossy().into_owned(),
            info.handle(),
            *value,
        ));
    }
    Ok(named)
}

fn property_of(
    props: &[(String, property::Handle, u64)],
    name: &str,
) -> Option<(property::Handle, u64)> {
    props
        .iter()
        .find(|(n, _, _)| n == name)
        .map(|(_, h, v)| (*h, *v))
}

/// Finds the writeback connectors, probing each so that its state is fresh.
fn find_writeback(card: &Raw) -> Result<Vec<Writeback>, String> {
    let resources = card
        .resource_handles()
        .map_err(|e| format!("resources: {e}"))?;
    let mut found = Vec::new();
    for handle in resources.connectors() {
        let info = card
            .get_connector(*handle, true)
            .map_err(|e| format!("connector {}: {e}", u32::from(*handle)))?;
        if info.interface() != connector::Interface::Writeback {
            continue;
        }
        let mut crtcs = Vec::new();
        for encoder in info.encoders() {
            if let Ok(encoder) = card.get_encoder(*encoder) {
                for crtc in resources.filter_crtcs(encoder.possible_crtcs()) {
                    if !crtcs.contains(&crtc) {
                        crtcs.push(crtc);
                    }
                }
            }
        }
        let props = properties(card, *handle).map_err(|e| format!("properties: {e}"))?;
        let (Some((fb_id, _)), Some((crtc_id, _))) = (
            property_of(&props, "WRITEBACK_FB_ID"),
            property_of(&props, "CRTC_ID"),
        ) else {
            continue;
        };
        let formats = match property_of(&props, "WRITEBACK_PIXEL_FORMATS") {
            Some((_, blob)) if blob != 0 => card
                .get_property_blob(blob)
                .map_err(|e| format!("WRITEBACK_PIXEL_FORMATS: {e}"))?
                .chunks_exact(4)
                .map(|b| PixelFormat(u32::from_ne_bytes([b[0], b[1], b[2], b[3]])))
                .collect(),
            _ => Vec::new(),
        };
        found.push(Writeback {
            connector: *handle,
            name: format!("{}-{}", info.interface().as_str(), info.interface_id()),
            crtcs,
            formats,
            fb_id,
            crtc_id,
        });
    }
    Ok(found)
}

/// Handles events until `submission` latched; returns the error text if it
/// did not within [`TIMEOUT`].
fn wait_latch(
    engine: &mut Engine<DrmBackend>,
    submission: SubmissionId,
    errors: &mut Vec<String>,
) -> Result<(), String> {
    let deadline = Instant::now() + TIMEOUT;
    let mut latched = false;
    loop {
        while let Some(event) = engine
            .poll_event()
            .map_err(|e| format!("event error {e:?}"))?
        {
            match event {
                DisplayEvent::Presented { submission: s, .. } if s >= submission => latched = true,
                DisplayEvent::Presented { .. } | DisplayEvent::Released { .. } => {}
                other => errors.push(format!("event {other:?}")),
            }
        }
        if latched {
            return Ok(());
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(format!(
                "submission {} not latched within {TIMEOUT:?}",
                submission.0
            ));
        }
        engine
            .backend()
            .wait_for_events(Some(left))
            .map_err(|e| format!("waiting for events: {e}"))?;
    }
}

/// The writeback buffer.
struct Target {
    dumb: DumbBuffer,
    framebuffer: framebuffer::Handle,
    format: PixelFormat,
    size: Size,
}

/// How to read the pixels of the mapped writeback buffer.
#[derive(Clone, Copy)]
struct Layout {
    format: PixelFormat,
    size: Size,
    pitch: usize,
}

impl Layout {
    fn pixel(&self, pixels: &[u8], x: i32, y: i32) -> Option<Rgb> {
        if x < 0 || y < 0 || x as u32 >= self.size.width || y as u32 >= self.size.height {
            return None;
        }
        let start = y as usize * self.pitch + x as usize * 4;
        let bytes = pixels.get(start..start + 4)?;
        Some(decode(
            self.format,
            [bytes[0], bytes[1], bytes[2], bytes[3]],
        ))
    }
}

/// One sample of a step.
struct Sample {
    name: String,
    at: (i32, i32),
    expected: Rgb,
    tolerance: u8,
}

/// What a sample showed.
struct Seen {
    got: Rgb,
    max_diff: u8,
}

/// Commits the writeback job on the frame on screen and reads `samples`.
fn capture(
    engine: &mut Engine<DrmBackend>,
    target: &mut Target,
    writeback: &Writeback,
    crtc: crtc::Handle,
    crtc_active: property::Handle,
    samples: &[Sample],
) -> Result<Vec<Seen>, String> {
    let card = engine.backend().card();
    let layout = Layout {
        format: target.format,
        size: target.size,
        pitch: drm::buffer::Buffer::pitch(&target.dumb) as usize,
    };
    {
        let px = encode(layout.format, SENTINEL, 0xff).unwrap();
        let size = layout.size;
        let mut mapping = card
            .map_dumb_buffer(&mut target.dumb)
            .map_err(|e| format!("map the writeback buffer: {e}"))?;
        let all = Rect::new(0, 0, size.width, size.height);
        fill(&mut mapping, layout.pitch, size, all, px);
    }
    let mut request = AtomicModeReq::new();
    request.add_property(
        writeback.connector,
        writeback.crtc_id,
        property::Value::CRTC(Some(crtc)),
    );
    request.add_property(
        writeback.connector,
        writeback.fb_id,
        property::Value::Framebuffer(Some(target.framebuffer)),
    );
    request.add_property(crtc, crtc_active, property::Value::Boolean(true));
    // Blocking: returns once the commit is done. Attaching the connector to
    // the CRTC counts as a modeset on some drivers.
    card.atomic_commit(AtomicCommitFlags::ALLOW_MODESET, request)
        .map_err(|e| format!("writeback commit refused: {e}"))?;
    // One more frame, so that the job has been written.
    let mut request = AtomicModeReq::new();
    request.add_property(crtc, crtc_active, property::Value::Boolean(true));
    card.atomic_commit(AtomicCommitFlags::empty(), request)
        .map_err(|e| format!("follow-up commit refused: {e}"))?;

    let deadline = Instant::now() + TIMEOUT;
    loop {
        let mapping = card
            .map_dumb_buffer(&mut target.dumb)
            .map_err(|e| format!("map the writeback buffer: {e}"))?;
        let mut seen = Vec::new();
        let mut written = true;
        for sample in samples {
            let (x, y) = sample.at;
            let got = layout
                .pixel(&mapping, x, y)
                .ok_or(format!("sample {} outside the buffer", sample.name))?;
            let mut max_diff = 0;
            for dy in -2..=2 {
                for dx in -2..=2 {
                    if let Some(p) = layout.pixel(&mapping, x + dx, y + dy) {
                        written &= p != SENTINEL;
                        max_diff = max_diff.max(p.max_diff(sample.expected));
                    }
                }
            }
            seen.push(Seen { got, max_diff });
        }
        if written {
            return Ok(seen);
        }
        if Instant::now() > deadline {
            return Err(format!(
                "writeback buffer still holds the sentinel {} after {TIMEOUT:?}",
                SENTINEL.hex()
            ));
        }
        drop(mapping);
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A step of the run.
struct Step {
    name: &'static str,
    overlay_z: i32,
    alpha: u16,
    blend: BlendMode,
}

fn layers(step: &Step, scene: &Scene, slot: usize, background_blend: BlendMode) -> [Layer; 2] {
    let o = scene.output;
    [
        Layer {
            blend: background_blend,
            ..Layer::new(
                BACKGROUND_IDS[slot],
                Rect::new(0, 0, o.width, o.height),
                Rect::new(0, 0, o.width, o.height),
                1 - step.overlay_z,
            )
        },
        Layer {
            alpha: step.alpha,
            blend: step.blend,
            ..Layer::new(
                OVERLAY_IDS[slot],
                Rect::new(0, 0, scene.overlay.width, scene.overlay.height),
                scene.overlay,
                step.overlay_z,
            )
        },
    ]
}

fn samples(step: &Step, scene: &Scene, overlay_format: PixelFormat, tolerance: u8) -> Vec<Sample> {
    let blended = tolerance.saturating_add(2);
    let mut list = vec![Sample {
        name: format!("{}.background", step.name),
        at: scene.background_point(),
        expected: BACKGROUND,
        tolerance,
    }];
    let on_top = step.overlay_z > 0;
    let upper = if on_top {
        over(step.blend, step.alpha, OVERLAY, 0xff, BACKGROUND)
    } else {
        BACKGROUND
    };
    list.push(Sample {
        name: format!("{}.overlay-upper", step.name),
        at: scene.upper_point(),
        expected: upper,
        tolerance: if step.alpha == ALPHA_OPAQUE {
            tolerance
        } else {
            blended
        },
    });
    // The lower half only says something when the hardware uses the pixel
    // alpha, with colours stored straight.
    if has_alpha(overlay_format) && step.blend == BlendMode::Coverage {
        let lower = if on_top {
            over(step.blend, step.alpha, OVERLAY, PIXEL_ALPHA, BACKGROUND)
        } else {
            BACKGROUND
        };
        list.push(Sample {
            name: format!("{}.overlay-lower-pixel-alpha-0x80", step.name),
            at: scene.lower_point(),
            expected: lower,
            tolerance: blended,
        });
    }
    list
}

fn plan_text(plan: &composition_core::Plan, caps: &Capabilities) -> String {
    plan.assignments()
        .iter()
        .map(|a| {
            format!(
                "layer {} -> plane {} {:?} zpos {}",
                a.layer,
                a.plane.0,
                caps.planes
                    .get(a.plane_index)
                    .map(|p| p.kind)
                    .unwrap_or_default(),
                a.zpos
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
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
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    println!("KERNEL {kernel}");
    println!("DEVICE {}", options.device);

    let file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&options.device)
    {
        Ok(file) => Raw(file),
        Err(error) => {
            println!(
                "VERDICT writeback: FAIL cannot open {}: {error}",
                options.device
            );
            return ExitCode::from(2);
        }
    };
    let driver = file
        .get_driver()
        .map(|d| d.name().to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown".into());
    println!("DRIVER {driver}");
    let caps_set = file
        .set_client_capability(ClientCapability::UniversalPlanes, true)
        .and_then(|()| file.set_client_capability(ClientCapability::Atomic, true));
    if let Err(error) = caps_set {
        println!("VERDICT writeback: FAIL driver {driver}: no atomic modesetting: {error}");
        return ExitCode::from(2);
    }
    // A kernel without writeback support refuses the capability.
    if file
        .set_client_capability(ClientCapability::WritebackConnectors, true)
        .is_err()
    {
        println!("VERDICT writeback: SKIP no writeback connector (driver {driver}: client capability refused)");
        return ExitCode::SUCCESS;
    }
    if let Err(error) = file.acquire_master_lock() {
        println!(
            "DRM master is unavailable on {}: {error}. Another process (a desktop or compositor) \
             probably holds it: stop it for the run and restore it afterwards. This example never \
             stops other processes.",
            options.device
        );
        println!("VERDICT writeback: FAIL DRM master unavailable on {driver}");
        return ExitCode::from(2);
    }
    let writebacks = match find_writeback(&file) {
        Ok(found) => found,
        Err(error) => {
            println!("VERDICT writeback: FAIL driver {driver}: {error}");
            return ExitCode::from(2);
        }
    };
    if writebacks.is_empty() {
        println!("VERDICT writeback: SKIP no writeback connector (driver {driver})");
        return ExitCode::SUCCESS;
    }
    for wb in &writebacks {
        println!(
            "WRITEBACK connector {} {} crtcs [{}] formats [{}]",
            u32::from(wb.connector),
            wb.name,
            wb.crtcs
                .iter()
                .map(|c| u32::from(*c).to_string())
                .collect::<Vec<_>>()
                .join(","),
            wb.formats
                .iter()
                .map(|f| fourcc(*f))
                .collect::<Vec<_>>()
                .join(",")
        );
    }

    let drm_options = DrmOptions {
        profile: options.profile,
        ..DrmOptions::default()
    };
    let backend = match DrmBackend::from_fd(file.0.into(), drm_options) {
        Ok(backend) => backend,
        Err(error) => {
            println!("VERDICT writeback: FAIL driver {driver}: backend: {error}");
            return ExitCode::from(2);
        }
    };
    let mut engine = Engine::new(backend);
    let caps = *engine.capabilities();

    // The first output a writeback connector can capture.
    let chosen = caps.outputs.iter().enumerate().find_map(|(index, output)| {
        writebacks
            .iter()
            .find(|wb| wb.crtcs.iter().any(|c| u32::from(*c) == output.id.0))
            .map(|wb| (index, *output, wb))
    });
    let Some((output_index, output, writeback)) = chosen else {
        println!(
            "VERDICT writeback: FAIL driver {driver}: no engine output on a CRTC a writeback \
             connector can use ({} output(s))",
            caps.outputs.len()
        );
        return ExitCode::from(2);
    };
    let size = output.size;
    let mode = format!(
        "{}x{}@{}.{:03}Hz",
        size.width,
        size.height,
        output.refresh_mhz / 1000,
        output.refresh_mhz % 1000
    );
    let Some(crtc) = writeback
        .crtcs
        .iter()
        .copied()
        .find(|c| u32::from(*c) == output.id.0)
    else {
        unreachable!("the output was chosen by its CRTC");
    };
    println!(
        "MODE {mode} output {output_index} crtc {} writeback {}",
        output.id.0, writeback.name
    );
    let summary = format!("driver {driver} mode {mode} via {}", writeback.name);

    let on_output = |kind: PlaneKind| {
        caps.planes
            .iter()
            .filter(move |p| p.outputs & (1 << output_index) != 0 && p.kind == kind)
    };
    let lists = |kind: PlaneKind, format: PixelFormat| {
        on_output(kind).any(|p| {
            p.formats
                .iter()
                .any(|f| f.format == format && f.modifier == Modifier::LINEAR)
        })
    };
    for plane in caps
        .planes
        .iter()
        .filter(|p| p.outputs & (1 << output_index) != 0)
    {
        println!(
            "PLANE id {} {:?} zpos {:?} plane-alpha {} blend-mask {:#x}",
            plane.id.0, plane.kind, plane.zpos, plane.plane_alpha, plane.blend_modes
        );
    }
    let rgb32 = [
        PixelFormat::XRGB8888,
        PixelFormat::ARGB8888,
        PixelFormat::XBGR8888,
        PixelFormat::ABGR8888,
    ];
    let with_alpha_first = [
        PixelFormat::ARGB8888,
        PixelFormat::ABGR8888,
        PixelFormat::XRGB8888,
        PixelFormat::XBGR8888,
    ];
    let Some(background_format) = rgb32
        .iter()
        .copied()
        .find(|f| lists(PlaneKind::Primary, *f))
    else {
        println!(
            "VERDICT writeback: FAIL no primary plane with a linear 32-bit RGB format, {summary}"
        );
        return ExitCode::from(1);
    };
    let Some(overlay_format) = with_alpha_first
        .iter()
        .copied()
        .find(|f| lists(PlaneKind::Overlay, *f))
    else {
        println!(
            "VERDICT writeback: FAIL no overlay plane with a linear 32-bit RGB format, {summary}"
        );
        return ExitCode::from(1);
    };
    let Some(target_format) = rgb32
        .iter()
        .copied()
        .find(|f| writeback.formats.contains(f))
    else {
        println!(
            "VERDICT writeback: FAIL the writeback connector takes no 32-bit RGB format, {summary}"
        );
        return ExitCode::from(1);
    };
    let primary_modes = on_output(PlaneKind::Primary).fold(0, |m, p| m | p.blend_modes);
    let background_blend =
        if background_format.is_known_opaque() || primary_modes & BlendMode::Opaque.mask() != 0 {
            BlendMode::Opaque
        } else {
            BlendMode::Premultiplied
        };
    // With pixel alpha 0xff and plane alpha opaque every mode gives the same
    // colour; coverage first, since the alpha step needs it.
    let overlay_modes = on_output(PlaneKind::Overlay).fold(0, |m, p| m | p.blend_modes);
    let overlay_blend = [
        BlendMode::Coverage,
        BlendMode::Premultiplied,
        BlendMode::Opaque,
    ]
    .into_iter()
    .find(|m| overlay_modes & m.mask() != 0)
    .unwrap_or(BlendMode::Premultiplied);
    println!(
        "FORMATS background {} overlay {} writeback {}",
        fourcc(background_format),
        fourcc(overlay_format),
        fourcc(target_format)
    );

    let scene = Scene::new(size);
    let mut errors: Vec<String> = Vec::new();
    for slot in 0..2 {
        for (id, info, background) in [
            (
                BACKGROUND_IDS[slot],
                BufferInfo {
                    size,
                    format: background_format,
                    modifier: Modifier::LINEAR,
                },
                true,
            ),
            (
                OVERLAY_IDS[slot],
                BufferInfo {
                    size: scene.overlay_size(),
                    format: overlay_format,
                    modifier: Modifier::LINEAR,
                },
                false,
            ),
        ] {
            let created = engine
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
                        .draw_dumb_buffer(id, |pixels, pitch| {
                            if background {
                                scene.draw_background(pixels, pitch, background_format);
                            } else {
                                scene.draw_overlay(pixels, pitch, overlay_format);
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
            }
        }
    }

    let target = engine
        .backend()
        .card()
        .create_dumb_buffer(
            (size.width, size.height),
            DrmFourcc::try_from(target_format.0).unwrap_or(DrmFourcc::Xrgb8888),
            32,
        )
        .map_err(|e| format!("writeback dumb buffer: {e}"))
        .and_then(|dumb| {
            let planar = Planar {
                size: (size.width, size.height),
                format: DrmFourcc::try_from(target_format.0).unwrap_or(DrmFourcc::Xrgb8888),
                pitch: drm::buffer::Buffer::pitch(&dumb),
                handle: drm::buffer::Buffer::handle(&dumb),
            };
            match engine
                .backend()
                .card()
                .add_planar_framebuffer(&planar, FbCmd2Flags::empty())
            {
                Ok(framebuffer) => Ok(Target {
                    dumb,
                    framebuffer,
                    format: target_format,
                    size,
                }),
                Err(error) => {
                    let _ = engine.backend().card().destroy_dumb_buffer(dumb);
                    Err(format!("writeback framebuffer: {error}"))
                }
            }
        });
    let mut target = match target {
        Ok(target) => Some(target),
        Err(error) => {
            errors.push(error);
            None
        }
    };
    let crtc_props = properties(engine.backend().card(), crtc).unwrap_or_default();
    let crtc_active = property_of(&crtc_props, "ACTIVE").map(|(h, _)| h);
    if crtc_active.is_none() {
        errors.push("the CRTC has no ACTIVE property".into());
    }

    let steps = [
        Step {
            name: "above",
            overlay_z: 1,
            alpha: ALPHA_OPAQUE,
            blend: overlay_blend,
        },
        Step {
            name: "swapped",
            overlay_z: 0,
            alpha: ALPHA_OPAQUE,
            blend: overlay_blend,
        },
        Step {
            name: "alpha",
            overlay_z: 1,
            alpha: HALF,
            blend: BlendMode::Coverage,
        },
    ];
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped: Vec<&str> = Vec::new();
    let mut attached_before = None;
    for (index, step) in steps.iter().enumerate() {
        if !errors.is_empty() {
            break;
        }
        let (Some(target), Some(crtc_active)) = (target.as_mut(), crtc_active) else {
            break;
        };
        let slot = index % 2;
        let layers = layers(step, &scene, slot, background_blend);
        let frame = Frame {
            output: output.id,
            layers: &layers,
        };
        let plan = match engine.validate(&frame) {
            Ok(plan) if plan.assignments().len() == 2 => plan,
            Ok(plan) if step.name == "above" => {
                errors.push(format!(
                    "above: the plan has {} plane(s), not two: {}",
                    plan.assignments().len(),
                    plan_text(&plan, &caps)
                ));
                break;
            }
            Ok(plan) => {
                println!(
                    "CHECK {}: SKIP the plan has {} plane(s), not two: {}",
                    step.name,
                    plan.assignments().len(),
                    plan_text(&plan, &caps)
                );
                skipped.push(step.name);
                continue;
            }
            Err(error) if step.name != "above" => {
                println!(
                    "CHECK {}: SKIP planner or TEST_ONLY refused: {error:?}",
                    step.name
                );
                skipped.push(step.name);
                continue;
            }
            Err(error) => {
                errors.push(format!("{}: validate: {error:?}", step.name));
                break;
            }
        };
        println!("PLAN {}: {}", step.name, plan_text(&plan, &caps));
        if step.overlay_z == 0 {
            // With fixed zpos the planner can only swap by moving the layers
            // to other planes; say which kind of swap the samples check.
            let settable = plan.assignments().iter().all(|a| {
                caps.planes
                    .get(a.plane_index)
                    .is_some_and(|p| matches!(p.zpos, Zpos::Range { .. }))
            });
            println!(
                "NOTE {}: {}",
                step.name,
                if settable {
                    "swapped by programming zpos"
                } else {
                    "swapped by plane assignment (a plane of the plan has a fixed zpos)"
                }
            );
        }
        let submission = match engine.commit(&frame, &plan) {
            Ok(submission) => submission,
            Err(error) => {
                errors.push(format!("{}: commit: {error:?}", step.name));
                break;
            }
        };
        if let Err(error) = wait_latch(&mut engine, submission, &mut errors) {
            errors.push(format!("{}: {error}", step.name));
            break;
        }
        if attached_before.is_none() {
            let props =
                properties(engine.backend().card(), writeback.connector).unwrap_or_default();
            attached_before = Some(
                property_of(&props, "CRTC_ID")
                    .is_some_and(|(_, v)| v == u64::from(u32::from(crtc))),
            );
        }
        let samples = samples(step, &scene, overlay_format, options.tolerance);
        match capture(&mut engine, target, writeback, crtc, crtc_active, &samples) {
            Ok(seen) => {
                for (sample, seen) in samples.iter().zip(seen) {
                    let ok = seen.max_diff <= sample.tolerance;
                    println!(
                        "CHECK {}: expected {} got {} maxdiff {} tolerance {} {}",
                        sample.name,
                        sample.expected.hex(),
                        seen.got.hex(),
                        seen.max_diff,
                        sample.tolerance,
                        if ok { "PASS" } else { "FAIL" }
                    );
                    if ok {
                        passed += 1;
                    } else {
                        failed += 1;
                    }
                }
            }
            Err(error) => {
                errors.push(format!("{}: {error}", step.name));
                break;
            }
        }
    }

    // Detach the writeback connector if this run attached it.
    if attached_before == Some(false) {
        let mut request = AtomicModeReq::new();
        request.add_property(
            writeback.connector,
            writeback.crtc_id,
            property::Value::CRTC(None),
        );
        if let Err(error) = engine
            .backend()
            .card()
            .atomic_commit(AtomicCommitFlags::ALLOW_MODESET, request)
        {
            println!("NOTE detaching the writeback connector failed: {error}");
        }
    }
    // Blank the output so that the buffers come back, then free them.
    let blank = Frame {
        output: output.id,
        layers: &[],
    };
    match engine
        .validate(&blank)
        .and_then(|plan| engine.commit(&blank, &plan))
    {
        Ok(submission) => {
            if let Err(error) = wait_latch(&mut engine, submission, &mut errors) {
                errors.push(format!("blank frame: {error}"));
            }
        }
        Err(Error::Backend(error)) => println!(
            "NOTE blank frame refused ({error:?}); the buffers stay until the device is closed"
        ),
        Err(error) => println!("NOTE blank frame refused: {error:?}"),
    }
    for id in BACKGROUND_IDS.iter().chain(&OVERLAY_IDS) {
        if !matches!(
            engine.buffer_state(*id),
            Some(BufferState::Submitted | BufferState::Scanning)
        ) {
            let _ = engine.unregister_buffer(*id);
            let _ = engine.backend_mut().remove_buffer(*id);
        }
    }
    if let Some(target) = target {
        let card = engine.backend().card();
        let _ = card.destroy_framebuffer(target.framebuffer);
        let _ = card.destroy_dumb_buffer(target.dumb);
    }

    let skip_text = if skipped.is_empty() {
        String::new()
    } else {
        format!(", skipped {}", skipped.join(","))
    };
    if !errors.is_empty() || failed > 0 || passed == 0 {
        for error in &errors {
            println!("ERROR {error}");
        }
        let first = errors.first().cloned().unwrap_or_else(|| {
            if failed > 0 {
                format!("{failed} sample(s) off")
            } else {
                "no sample read".into()
            }
        });
        println!("VERDICT writeback: FAIL {first}, {passed} passed, {failed} failed{skip_text}, {summary}");
        ExitCode::from(1)
    } else {
        println!("VERDICT writeback: PASS {passed} samples{skip_text}, {summary}");
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_plane_alpha_coverage_is_the_mean() {
        let blended = over(BlendMode::Coverage, HALF, OVERLAY, 0xff, BACKGROUND);
        assert_eq!(blended, Rgb(0x88, 0x90, 0x58));
    }

    #[test]
    fn opaque_overlay_hides_the_background_in_every_mode() {
        for mode in [
            BlendMode::Opaque,
            BlendMode::Premultiplied,
            BlendMode::Coverage,
        ] {
            assert_eq!(over(mode, ALPHA_OPAQUE, OVERLAY, 0xff, BACKGROUND), OVERLAY);
        }
    }

    #[test]
    fn decode_inverts_encode() {
        for format in [
            PixelFormat::XRGB8888,
            PixelFormat::ARGB8888,
            PixelFormat::XBGR8888,
            PixelFormat::ABGR8888,
        ] {
            assert_eq!(
                decode(format, encode(format, OVERLAY, 0x80).unwrap()),
                OVERLAY
            );
        }
    }

    #[test]
    fn sample_points_lie_in_their_regions() {
        let scene = Scene::new(Size::new(1024, 768));
        let inside = |r: Rect, (x, y): (i32, i32)| {
            x >= r.x && y >= r.y && x < r.x + r.width as i32 && y < r.y + r.height as i32
        };
        let offset = |r: Rect| {
            Rect::new(
                r.x + scene.overlay.x,
                r.y + scene.overlay.y,
                r.width,
                r.height,
            )
        };
        assert!(!inside(scene.overlay, scene.background_point()));
        assert!(inside(offset(scene.upper()), scene.upper_point()));
        assert!(inside(offset(scene.lower()), scene.lower_point()));
    }
}
