//! Linux DRM/KMS backend using atomic modesetting.
//!
//! The backend is handed a file descriptor and drives whatever that
//! descriptor exposes. A DRM lease descriptor works the same way as a device
//! node: the kernel only lists the leased connectors, CRTCs and planes, so
//! the reported capabilities cover exactly what the lease grants. How the
//! descriptor was obtained is not this crate's concern.
//!
//! What the kernel does not describe is not claimed:
//!
//! - Scaling limits have no generic property. Planes are reported as unable
//!   to scale unless [`DrmOptions::scaling`] says otherwise; with it set, the
//!   atomic `TEST_ONLY` check is the final word.
//! - Rotation is not reported, because the property's supported values are
//!   not available through the `drm` crate yet.
//! - Without a `zpos` property the stacking order is the conventional one:
//!   primary below overlays in plane order, cursor on top.
//! - A plane listing more than [`composition_core::limits::MAX_FORMATS`]
//!   format/modifier pairs is reported with the first ones only, linear
//!   layouts first.
//!
//! What [`BackendError`] cannot carry is kept by the backend: the raw
//! `errno` and the reason of each failure ([`DrmBackend::take_failures`]),
//! and the vertical blank sequence and timestamp of each latch
//! ([`DrmBackend::take_latches`]).

#![forbid(unsafe_code)]

pub mod profile;

pub use profile::DriverProfile;

use std::collections::VecDeque;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;
use std::time::{Duration, Instant};

use composition_core::limits::{MAX_FORMATS, MAX_OUTPUTS, MAX_PLANES};
use composition_core::{
    Assignment, BackendError, BackendEvent, BlendMode, BufferId, BufferInfo, Capabilities,
    DisplayBackend, FormatModifier, Frame, Modifier, OutputCaps, OutputId, PixelFormat, Plan,
    PlaneCaps, PlaneId, PlaneKind, ScaleLimits, Size, SubmissionId, Zpos, ALPHA_OPAQUE,
};
use drm::buffer::{DrmFourcc, DrmModifier, PlanarBuffer};
use drm::control::atomic::AtomicModeReq;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    connector, crtc, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice,
    Event, FbCmd2Flags, Mode,
};
use drm::{ClientCapability, Device as BasicDevice, DriverCapability};

/// An open DRM device or lease.
#[derive(Debug)]
pub struct Card(OwnedFd);

impl Card {
    /// Wraps an open DRM file descriptor.
    pub fn from_fd(fd: OwnedFd) -> Self {
        Self(fd)
    }
}

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl BasicDevice for Card {}
impl ControlDevice for Card {}

/// Settings for what the kernel cannot tell.
#[derive(Clone, Copy, Debug, Default)]
pub struct DrmOptions {
    /// Scaling limits to assume for primary and overlay planes. `None`
    /// reports every plane as unable to scale.
    pub scaling: Option<ScaleLimits>,
    /// Driver-specific constraints to add to what KMS reports.
    pub profile: DriverProfile,
}

/// The request a failure happened in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operation {
    /// An atomic `TEST_ONLY` commit, from [`DisplayBackend::check`].
    Check,
    /// An atomic commit, from [`DisplayBackend::commit`].
    Commit,
    /// Waiting for or reading page-flip events.
    Events,
}

/// Why the backend failed, with what [`BackendError`] cannot carry.
///
/// Kept in a short log, see [`DrmBackend::take_failures`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DrmFailure {
    /// The kernel refused the request with `errno` (`EINVAL`, `ERANGE`,
    /// `EBUSY`, `ENOENT`, ...), raw.
    Refused {
        /// The request.
        operation: Operation,
        /// The kernel's error number.
        errno: i32,
        /// The request left the output's primary plane unused. Some drivers
        /// refuse such frames, see [`DrmBackend::set_require_primary`].
        primary_unused: bool,
    },
    /// The descriptor is not DRM master right now (`EACCES`, `EPERM`):
    /// another process, such as the compositor, holds it, or the lessor
    /// dropped it. The device is not lost; the error is
    /// [`BackendError::Platform`] with `errno`.
    NotMaster {
        /// The request.
        operation: Operation,
        /// The kernel's error number.
        errno: i32,
    },
    /// The `TEST_ONLY` check failed with `errno`, but passes when a modeset
    /// is allowed: the frame needs a full modeset, which this backend only
    /// does on the first commit of an output. Reported to the engine as
    /// [`BackendError::Rejected`], so that other assignments are tried.
    NeedsModeset {
        /// The kernel's error number without `ALLOW_MODESET`.
        errno: i32,
    },
    /// [`DrmBackend::set_require_primary`] is on and the bottom layer of
    /// the plan is not on the output's primary plane. Refused without asking
    /// the kernel, as [`BackendError::Rejected`].
    NoPrimaryPlane {
        /// The output.
        output: OutputId,
    },
    /// The objects are gone: a revoked lease or a removed device. `errno`
    /// is that of the request that showed it, 0 if none did.
    Lost {
        /// The request that showed the loss.
        operation: Operation,
        /// The kernel's error number.
        errno: i32,
    },
    /// Polling the descriptor reported a hangup or an error (`POLLHUP`,
    /// `POLLERR`, `POLLNVAL` bits as `revents`). The device is marked lost.
    Hangup {
        /// The `poll` result bits.
        revents: u32,
    },
    /// Polling or reading events failed with `errno`; the device is not
    /// marked lost unless the error says so.
    ReadEvents {
        /// The error number.
        errno: i32,
    },
    /// A committed frame has not latched within
    /// [`DrmBackend::set_latch_timeout`]. The submission stays pending, since
    /// the event may still come; the device was found still present.
    LatchTimeout {
        /// The output.
        output: OutputId,
        /// The submission still waiting.
        submission: SubmissionId,
        /// How long it had waited when this was noticed.
        waited: Duration,
    },
}

/// A latch, as the page-flip event reported it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LatchInfo {
    /// The output.
    pub output: OutputId,
    /// The submission that latched.
    pub submission: SubmissionId,
    /// The CRTC's vertical blank counter.
    pub sequence: u32,
    /// Time of the vertical blank (`tv_sec`, `tv_usec`), on the kernel's DRM
    /// clock, which is `CLOCK_MONOTONIC` on all current kernels.
    pub timestamp: Duration,
}

/// Number of failures and latches kept for the accessors.
const LOG_LENGTH: usize = 64;

/// Latch timeout until [`DrmBackend::set_latch_timeout`] says otherwise.
pub const DEFAULT_LATCH_TIMEOUT: Duration = Duration::from_secs(1);

/// One plane of a DMA-BUF to import.
#[derive(Clone, Copy, Debug)]
pub struct DmabufPlane<'a> {
    /// The DMA-BUF.
    pub fd: BorrowedFd<'a>,
    /// Offset of the plane's first byte.
    pub offset: u32,
    /// Bytes per row.
    pub pitch: u32,
}

#[derive(Clone, Copy, Debug)]
struct PlaneProps {
    fb_id: property::Handle,
    crtc_id: property::Handle,
    src: [property::Handle; 4],
    dst: [property::Handle; 4],
    /// Present only if the position can be set.
    zpos: Option<property::Handle>,
    /// Property and its maximum value.
    alpha: Option<(property::Handle, u64)>,
    /// Property and the enum values for opaque, premultiplied, coverage.
    blend: Option<(property::Handle, [Option<u64>; 3])>,
}

#[derive(Clone, Copy, Debug)]
struct PlaneEntry {
    handle: plane::Handle,
    props: PlaneProps,
    /// CRTC the plane is attached to, as far as this backend knows.
    crtc: Option<crtc::Handle>,
    caps: PlaneCaps,
}

#[derive(Debug)]
struct OutputEntry {
    connector: connector::Handle,
    crtc: crtc::Handle,
    mode: Mode,
    connector_crtc_id: property::Handle,
    crtc_mode_id: property::Handle,
    crtc_active: property::Handle,
    mode_blob: u64,
    /// Whether this backend has programmed the mode yet.
    modeset_done: bool,
    pending: Option<SubmissionId>,
    /// When the pending submission was committed.
    pending_since: Option<Instant>,
    /// Whether the pending submission was reported as overdue.
    overdue: bool,
    last_latch: Option<LatchInfo>,
}

#[derive(Debug)]
struct BufferEntry {
    id: BufferId,
    framebuffer: framebuffer::Handle,
    dumb: Option<DumbBuffer>,
}

struct ImportedBuffer {
    size: (u32, u32),
    format: DrmFourcc,
    modifier: Option<DrmModifier>,
    pitches: [u32; 4],
    handles: [Option<drm::buffer::Handle>; 4],
    offsets: [u32; 4],
}

impl PlanarBuffer for ImportedBuffer {
    fn size(&self) -> (u32, u32) {
        self.size
    }
    fn format(&self) -> DrmFourcc {
        self.format
    }
    fn modifier(&self) -> Option<DrmModifier> {
        self.modifier
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

/// The DRM/KMS backend.
#[derive(Debug)]
pub struct DrmBackend {
    card: Card,
    options: DrmOptions,
    fb_modifiers: bool,
    outputs: Vec<OutputEntry>,
    planes: Vec<PlaneEntry>,
    buffers: Vec<BufferEntry>,
    events: VecDeque<BackendEvent>,
    lost: bool,
    failures: VecDeque<DrmFailure>,
    latches: VecDeque<LatchInfo>,
    require_primary: bool,
    latch_timeout: Option<Duration>,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn missing(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, message)
}

/// Named properties of a KMS object with their current raw values.
fn named_properties<H: drm::control::ResourceHandle>(
    card: &Card,
    handle: H,
) -> io::Result<Vec<(String, property::Info, u64)>> {
    let set = card.get_properties(handle)?;
    let mut named = Vec::new();
    for (prop, value) in set.iter() {
        let info = card.get_property(*prop)?;
        named.push((info.name().to_string_lossy().into_owned(), info, *value));
    }
    Ok(named)
}

fn find<'a>(
    props: &'a [(String, property::Info, u64)],
    name: &str,
) -> Option<&'a (String, property::Info, u64)> {
    props.iter().find(|(n, _, _)| n == name)
}

fn require(
    props: &[(String, property::Info, u64)],
    name: &'static str,
) -> io::Result<property::Handle> {
    find(props, name)
        .map(|(_, info, _)| info.handle())
        .ok_or_else(|| missing(name))
}

/// Parses an `IN_FORMATS` blob into format/modifier pairs.
fn parse_in_formats(blob: &[u8]) -> Vec<FormatModifier> {
    let u32_at = |offset: usize| {
        blob.get(offset..offset + 4)
            .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
    };
    let u64_at = |offset: usize| {
        blob.get(offset..offset + 8)
            .map(|b| u64::from_ne_bytes(b.try_into().unwrap()))
    };
    let mut pairs = Vec::new();
    let (Some(count_formats), Some(formats_offset), Some(count_modifiers), Some(modifiers_offset)) =
        (u32_at(8), u32_at(12), u32_at(16), u32_at(20))
    else {
        return pairs;
    };
    for m in 0..count_modifiers as usize {
        let base = modifiers_offset as usize + m * 24;
        let (Some(mask), Some(first), Some(modifier)) =
            (u64_at(base), u32_at(base + 8), u64_at(base + 16))
        else {
            break;
        };
        for bit in 0..64usize {
            let index = first as usize + bit;
            if mask & (1 << bit) == 0 || index >= count_formats as usize {
                continue;
            }
            if let Some(format) = u32_at(formats_offset as usize + index * 4) {
                pairs.push(FormatModifier {
                    format: PixelFormat(format),
                    modifier: Modifier(modifier),
                });
            }
        }
    }
    pairs
}

fn refresh_millihertz(mode: &Mode) -> u32 {
    let (_, _, htotal) = mode.hsync();
    let (_, _, vtotal) = mode.vsync();
    let total = htotal as u64 * vtotal as u64;
    if total == 0 {
        return 0;
    }
    (mode.clock() as u64 * 1_000_000 / total) as u32
}

impl DrmBackend {
    /// Opens a DRM device node such as `/dev/dri/card0`.
    pub fn open(path: impl AsRef<Path>, options: DrmOptions) -> io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        Self::from_fd(file.into(), options)
    }

    /// Takes over a DRM file descriptor: a device node or a lease.
    ///
    /// The descriptor must be, or be able to become, DRM master of the
    /// objects it exposes.
    pub fn from_fd(fd: OwnedFd, options: DrmOptions) -> io::Result<Self> {
        let card = Card(fd);
        card.set_client_capability(ClientCapability::UniversalPlanes, true)?;
        card.set_client_capability(ClientCapability::Atomic, true)?;
        let fb_modifiers = card
            .get_driver_capability(DriverCapability::AddFB2Modifiers)
            .is_ok_and(|v| v != 0);
        let mut backend = Self {
            card,
            options,
            fb_modifiers,
            outputs: Vec::new(),
            planes: Vec::new(),
            buffers: Vec::new(),
            events: VecDeque::new(),
            lost: false,
            failures: VecDeque::new(),
            latches: VecDeque::new(),
            require_primary: false,
            latch_timeout: Some(DEFAULT_LATCH_TIMEOUT),
        };
        backend.discover()?;
        Ok(backend)
    }

    /// The underlying device, for queries this crate does not wrap.
    pub fn card(&self) -> &Card {
        &self.card
    }

    /// Name of the kernel driver, such as `vkms`.
    pub fn driver_name(&self) -> io::Result<String> {
        Ok(self
            .card
            .get_driver()?
            .name()
            .to_string_lossy()
            .into_owned())
    }

    /// Looks at what the descriptor exposes and rebuilds outputs and planes.
    fn discover(&mut self) -> io::Result<()> {
        let resources = self.card.resource_handles()?;
        let mut outputs: Vec<OutputEntry> = Vec::new();

        for handle in resources.connectors() {
            if outputs.len() == MAX_OUTPUTS {
                break;
            }
            let info = self.card.get_connector(*handle, false)?;
            if info.state() != connector::State::Connected {
                continue;
            }
            let taken = |crtc: &crtc::Handle| outputs.iter().any(|o| o.crtc == *crtc);
            // Keep the CRTC the connector already uses; otherwise take the
            // first free one an encoder can route to.
            let mut chosen = None;
            if let Some(encoder) = info.current_encoder() {
                chosen = self.card.get_encoder(encoder)?.crtc().filter(|c| !taken(c));
            }
            if chosen.is_none() {
                for encoder in info.encoders() {
                    let possible = self.card.get_encoder(*encoder)?.possible_crtcs();
                    chosen = resources
                        .filter_crtcs(possible)
                        .into_iter()
                        .find(|c| !taken(c));
                    if chosen.is_some() {
                        break;
                    }
                }
            }
            let Some(crtc) = chosen else { continue };
            let crtc_info = self.card.get_crtc(crtc)?;
            let Some(mode) = crtc_info.mode().or_else(|| info.modes().first().copied()) else {
                continue;
            };
            let connector_props = named_properties(&self.card, *handle)?;
            let crtc_props = named_properties(&self.card, crtc)?;
            let mode_blob = match self.card.create_property_blob(&mode)? {
                property::Value::Blob(id) => id,
                _ => return Err(invalid("mode blob")),
            };
            outputs.push(OutputEntry {
                connector: *handle,
                crtc,
                mode,
                connector_crtc_id: require(&connector_props, "CRTC_ID")?,
                crtc_mode_id: require(&crtc_props, "MODE_ID")?,
                crtc_active: require(&crtc_props, "ACTIVE")?,
                mode_blob,
                modeset_done: false,
                pending: None,
                pending_since: None,
                overdue: false,
                last_latch: None,
            });
        }

        let mut planes = Vec::new();
        for handle in self.card.plane_handles()? {
            let info = self.card.get_plane(handle)?;
            let props = named_properties(&self.card, handle)?;
            let possible = resources.filter_crtcs(info.possible_crtcs());

            let kind = match find(&props, "type").map(|(_, _, v)| *v as u32) {
                Some(v) if v == drm::control::PlaneType::Primary as u32 => PlaneKind::Primary,
                Some(v) if v == drm::control::PlaneType::Cursor as u32 => PlaneKind::Cursor,
                _ => PlaneKind::Overlay,
            };
            let mut caps = PlaneCaps {
                id: PlaneId(u32::from(handle)),
                kind,
                ..PlaneCaps::default()
            };
            for (index, output) in outputs.iter().enumerate() {
                if possible.contains(&output.crtc) {
                    caps.outputs |= 1 << index;
                }
            }

            let mut pairs = match find(&props, "IN_FORMATS") {
                Some((_, _, blob)) if *blob != 0 => {
                    parse_in_formats(&self.card.get_property_blob(*blob)?)
                }
                _ => Vec::new(),
            };
            if pairs.is_empty() {
                pairs = info
                    .formats()
                    .iter()
                    .map(|f| FormatModifier::linear(PixelFormat(*f)))
                    .collect();
            }
            pairs.sort_by_key(|p| p.modifier != Modifier::LINEAR);
            for pair in pairs.into_iter().take(MAX_FORMATS) {
                let _ = caps.formats.push(pair);
            }

            if kind != PlaneKind::Cursor {
                caps.scaling = self.options.scaling;
            }

            let alpha = find(&props, "alpha").and_then(|(_, info, _)| match info.value_type() {
                property::ValueType::UnsignedRange(_, max) if *max > 0 => {
                    Some((info.handle(), *max))
                }
                _ => None,
            });
            caps.plane_alpha = alpha.is_some();

            let blend = find(&props, "pixel blend mode").and_then(|(_, info, _)| {
                let property::ValueType::Enum(values) = info.value_type() else {
                    return None;
                };
                let (_, entries) = values.values();
                let value_of = |name: &str| {
                    entries
                        .iter()
                        .find(|e| e.name().to_bytes() == name.as_bytes())
                        .map(|e| e.value())
                };
                Some((
                    info.handle(),
                    [
                        value_of("None"),
                        value_of("Pre-multiplied"),
                        value_of("Coverage"),
                    ],
                ))
            });
            caps.blend_modes = match blend {
                Some((_, values)) => {
                    let modes = [
                        BlendMode::Opaque,
                        BlendMode::Premultiplied,
                        BlendMode::Coverage,
                    ];
                    modes
                        .iter()
                        .zip(values)
                        .filter(|(_, v)| v.is_some())
                        .fold(0, |mask, (mode, _)| mask | mode.mask())
                }
                // Without the property KMS blends as premultiplied.
                None => BlendMode::Premultiplied.mask(),
            };

            let mut zpos_prop = None;
            caps.zpos = match find(&props, "zpos") {
                Some((_, info, value)) => match info.value_type() {
                    property::ValueType::UnsignedRange(min, max) if info.mutable() => {
                        zpos_prop = Some(info.handle());
                        Zpos::Range {
                            min: *min as i32,
                            max: *max as i32,
                        }
                    }
                    _ => Zpos::Fixed(*value as i32),
                },
                // Conventional order, refined below once all planes are known.
                None => Zpos::Fixed(match kind {
                    PlaneKind::Primary => 0,
                    PlaneKind::Overlay => 1,
                    PlaneKind::Cursor => i32::MAX,
                }),
            };
            let conventional = find(&props, "zpos").is_none();

            let src = ["SRC_X", "SRC_Y", "SRC_W", "SRC_H"];
            let dst = ["CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H"];
            let handles = |names: [&'static str; 4]| -> io::Result<[property::Handle; 4]> {
                Ok([
                    require(&props, names[0])?,
                    require(&props, names[1])?,
                    require(&props, names[2])?,
                    require(&props, names[3])?,
                ])
            };
            planes.push((
                conventional,
                PlaneEntry {
                    handle,
                    props: PlaneProps {
                        fb_id: require(&props, "FB_ID")?,
                        crtc_id: require(&props, "CRTC_ID")?,
                        src: handles(src)?,
                        dst: handles(dst)?,
                        zpos: zpos_prop,
                        alpha,
                        blend,
                    },
                    crtc: info.crtc(),
                    caps,
                },
            ));
        }

        let mut planes = keep_planes(planes, |(_, p)| (p.caps.outputs != 0, p.caps.kind));
        // Numbered after the cut: reachable overlays come first, so their
        // numbers do not depend on planes of unconnected CRTCs.
        let mut overlay_position = 1;
        for (conventional, plane) in &mut planes {
            if *conventional && plane.caps.kind == PlaneKind::Overlay {
                plane.caps.zpos = Zpos::Fixed(overlay_position);
                overlay_position += 1;
            }
        }

        self.outputs = outputs;
        self.planes = planes.into_iter().map(|(_, p)| p).collect();
        Ok(())
    }

    fn insert_buffer(&mut self, entry: BufferEntry) -> io::Result<()> {
        if self.buffers.iter().any(|b| b.id == entry.id) {
            let _ = self.card.destroy_framebuffer(entry.framebuffer);
            if let Some(dumb) = entry.dumb {
                let _ = self.card.destroy_dumb_buffer(dumb);
            }
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "buffer id already imported",
            ));
        }
        self.buffers.push(entry);
        Ok(())
    }

    /// Imports a DMA-BUF as `buffer`.
    ///
    /// The descriptors only need to stay open for the duration of the call.
    pub fn import_dmabuf(
        &mut self,
        buffer: BufferId,
        info: BufferInfo,
        planes: &[DmabufPlane<'_>],
    ) -> io::Result<()> {
        if planes.is_empty() || planes.len() > 4 {
            return Err(invalid("a buffer has one to four planes"));
        }
        let format = DrmFourcc::try_from(info.format.0).map_err(|_| invalid("unknown fourcc"))?;
        if !self.fb_modifiers && info.modifier != Modifier::LINEAR {
            return Err(invalid("device does not accept format modifiers"));
        }
        let mut imported = ImportedBuffer {
            size: (info.size.width, info.size.height),
            format,
            modifier: self
                .fb_modifiers
                .then(|| DrmModifier::from(info.modifier.0)),
            pitches: [0; 4],
            handles: [None; 4],
            offsets: [0; 4],
        };
        // Importing the same DMA-BUF twice yields the same GEM handle, and
        // handles are not reference counted: close each one exactly once.
        let mut opened: Vec<drm::buffer::Handle> = Vec::new();
        let mut result = Ok(());
        for (index, plane) in planes.iter().enumerate() {
            match self.card.prime_fd_to_buffer(plane.fd) {
                Ok(handle) => {
                    if !opened.contains(&handle) {
                        opened.push(handle);
                    }
                    imported.handles[index] = Some(handle);
                    imported.pitches[index] = plane.pitch;
                    imported.offsets[index] = plane.offset;
                }
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        let flags = if self.fb_modifiers {
            FbCmd2Flags::MODIFIERS
        } else {
            FbCmd2Flags::empty()
        };
        let framebuffer = result.and_then(|()| self.card.add_planar_framebuffer(&imported, flags));
        // The framebuffer keeps the memory alive; the handles are not needed.
        for handle in opened {
            let _ = self.card.close_buffer(handle);
        }
        self.insert_buffer(BufferEntry {
            id: buffer,
            framebuffer: framebuffer?,
            dumb: None,
        })
    }

    /// Allocates a dumb buffer on the device and imports it as `buffer`.
    ///
    /// Meant for tests and simple producers that draw with the CPU. Only
    /// 16- and 32-bit single-plane RGB formats are supported.
    pub fn create_dumb_buffer(&mut self, buffer: BufferId, info: BufferInfo) -> io::Result<()> {
        let format = DrmFourcc::try_from(info.format.0).map_err(|_| invalid("unknown fourcc"))?;
        let bpp = match format {
            DrmFourcc::Rgb565 => 16,
            DrmFourcc::Xrgb8888
            | DrmFourcc::Argb8888
            | DrmFourcc::Xbgr8888
            | DrmFourcc::Abgr8888 => 32,
            _ => return Err(invalid("unsupported dumb buffer format")),
        };
        if info.modifier != Modifier::LINEAR {
            return Err(invalid("dumb buffers are linear"));
        }
        let dumb =
            self.card
                .create_dumb_buffer((info.size.width, info.size.height), format, bpp)?;
        let planar = ImportedBuffer {
            size: (info.size.width, info.size.height),
            format,
            modifier: None,
            pitches: [drm::buffer::Buffer::pitch(&dumb), 0, 0, 0],
            handles: [Some(drm::buffer::Buffer::handle(&dumb)), None, None, None],
            offsets: [0; 4],
        };
        let framebuffer = match self
            .card
            .add_planar_framebuffer(&planar, FbCmd2Flags::empty())
        {
            Ok(framebuffer) => framebuffer,
            Err(error) => {
                let _ = self.card.destroy_dumb_buffer(dumb);
                return Err(error);
            }
        };
        self.insert_buffer(BufferEntry {
            id: buffer,
            framebuffer,
            dumb: Some(dumb),
        })
    }

    /// Gives `draw` the pixels of a dumb buffer and its pitch in bytes.
    pub fn draw_dumb_buffer<R>(
        &mut self,
        buffer: BufferId,
        draw: impl FnOnce(&mut [u8], usize) -> R,
    ) -> io::Result<R> {
        let entry = self
            .buffers
            .iter_mut()
            .find(|b| b.id == buffer)
            .ok_or_else(|| missing("buffer"))?;
        let dumb = entry
            .dumb
            .as_mut()
            .ok_or_else(|| invalid("not a dumb buffer"))?;
        let pitch = drm::buffer::Buffer::pitch(dumb) as usize;
        let mut mapping = self.card.map_dumb_buffer(dumb)?;
        Ok(draw(mapping.as_mut(), pitch))
    }

    /// Removes an imported buffer.
    ///
    /// The caller must make sure the buffer is no longer shown, which the
    /// engine guarantees once it has let the buffer be unregistered.
    pub fn remove_buffer(&mut self, buffer: BufferId) -> io::Result<()> {
        let index = self
            .buffers
            .iter()
            .position(|b| b.id == buffer)
            .ok_or_else(|| missing("buffer"))?;
        let entry = self.buffers.swap_remove(index);
        self.card.destroy_framebuffer(entry.framebuffer)?;
        if let Some(dumb) = entry.dumb {
            self.card.destroy_dumb_buffer(dumb)?;
        }
        Ok(())
    }

    /// Waits until the device has events to read, or the timeout passes.
    ///
    /// Returns `true` if [`DisplayBackend::poll_event`] has something to do:
    /// events to read, a hangup or error on the descriptor to report, or a
    /// pending latch that has just exceeded the latch timeout
    /// ([`DrmBackend::set_latch_timeout`]). The wait ends at that deadline
    /// even if `timeout` is longer, so that a missing page-flip event is
    /// reported instead of waited for.
    pub fn wait_for_events(&self, timeout: Option<Duration>) -> io::Result<bool> {
        if !self.events.is_empty() {
            return Ok(true);
        }
        let deadline = self
            .next_latch_deadline()
            .map(|at| at.saturating_duration_since(Instant::now()));
        let wait = match (timeout, deadline) {
            (Some(timeout), Some(deadline)) => Some(timeout.min(deadline)),
            (timeout, deadline) => timeout.or(deadline),
        };
        if !self.poll_device(wait)?.is_empty() {
            return Ok(true);
        }
        // Woken by the latch deadline rather than by the caller's timeout.
        Ok(deadline.is_some_and(|d| timeout.is_none_or(|t| d < t)))
    }

    /// Requires the bottom layer of every frame to be on the output's
    /// primary plane.
    ///
    /// Off by default. Some drivers refuse a frame that leaves the primary
    /// plane off; with this set such plans are refused before the kernel is
    /// asked, as [`BackendError::Rejected`] with
    /// [`DrmFailure::NoPrimaryPlane`] logged, and the engine moves on to an
    /// assignment that uses the primary plane. Without it, a kernel refusal
    /// of a plan without a primary plane is logged as
    /// [`DrmFailure::Refused`] with `primary_unused` set.
    ///
    /// The i.MX95 `dpu95` source read so far refuses an active CRTC with no
    /// plane at all, not one without its primary plane (see
    /// `docs/imx95-dpu-source-6.6.md`), so [`DriverProfile::Imx95Dpu`] does
    /// not turn this on. Not measured on hardware.
    pub fn set_require_primary(&mut self, require: bool) {
        self.require_primary = require;
    }

    /// Whether [`DrmBackend::set_require_primary`] is on.
    pub fn require_primary(&self) -> bool {
        self.require_primary
    }

    /// Sets how long a committed frame may wait for its page-flip event
    /// before [`DrmFailure::LatchTimeout`] is logged, or `None` for no limit.
    /// [`DEFAULT_LATCH_TIMEOUT`] until set.
    ///
    /// When the time is up the backend first checks that its objects still
    /// exist ([`DrmBackend::probe_device`]): if they do not, the device is
    /// marked lost and [`BackendEvent::DeviceLost`] queued. Otherwise the
    /// submission stays pending, because the event may still come, and the
    /// timeout is logged once.
    pub fn set_latch_timeout(&mut self, timeout: Option<Duration>) {
        self.latch_timeout = timeout;
    }

    /// The latch timeout, see [`DrmBackend::set_latch_timeout`].
    pub fn latch_timeout(&self) -> Option<Duration> {
        self.latch_timeout
    }

    /// Takes the logged failures, oldest first. Only the last 64 are kept.
    pub fn take_failures(&mut self) -> std::collections::vec_deque::Drain<'_, DrmFailure> {
        self.failures.drain(..)
    }

    /// The most recent failure still in the log.
    pub fn last_failure(&self) -> Option<DrmFailure> {
        self.failures.back().copied()
    }

    /// Takes the logged latches with their vertical blank sequence and
    /// timestamp, oldest first. Only the last 64 are kept. Each was also
    /// reported as [`BackendEvent::Latched`].
    pub fn take_latches(&mut self) -> std::collections::vec_deque::Drain<'_, LatchInfo> {
        self.latches.drain(..)
    }

    /// The most recent latch of `output`, kept even after
    /// [`DrmBackend::take_latches`].
    pub fn last_latch(&self, output: OutputId) -> Option<LatchInfo> {
        self.output(output).and_then(|o| o.last_latch)
    }

    /// The submission of `output` waiting for its latch, and since when.
    pub fn pending_latch(&self, output: OutputId) -> Option<(SubmissionId, Instant)> {
        self.output(output)
            .and_then(|o| Some((o.pending?, o.pending_since?)))
    }

    /// Whether the device has been found lost.
    pub fn is_lost(&self) -> bool {
        self.lost
    }

    /// Checks that the CRTCs this backend drives still exist for this
    /// descriptor. If they do not, as after a lease revocation, which the
    /// lessee is not told about, the device is marked lost and
    /// [`BackendEvent::DeviceLost`] is queued. Returns whether the device is
    /// usable.
    pub fn probe_device(&mut self) -> bool {
        if !self.lost {
            if let Some(errno) = self.objects_gone() {
                self.mark_lost(DrmFailure::Lost {
                    operation: Operation::Events,
                    errno,
                });
            }
        }
        !self.lost
    }

    fn output(&self, output: OutputId) -> Option<&OutputEntry> {
        self.outputs
            .iter()
            .find(|o| OutputId(u32::from(o.crtc)) == output)
    }

    /// The error number with which a CRTC lookup failed because the object
    /// is gone, if one did.
    fn objects_gone(&self) -> Option<i32> {
        self.outputs.iter().find_map(|output| {
            let error = self.card.get_crtc(output.crtc).err()?;
            let errno = error.raw_os_error().unwrap_or(0);
            matches!(class_of(errno), ErrnoClass::Missing | ErrnoClass::Gone).then_some(errno)
        })
    }

    fn log_failure(&mut self, failure: DrmFailure) {
        if self.failures.len() == LOG_LENGTH {
            self.failures.pop_front();
        }
        self.failures.push_back(failure);
    }

    fn mark_lost(&mut self, failure: DrmFailure) {
        self.log_failure(failure);
        if !self.lost {
            self.lost = true;
            for output in &mut self.outputs {
                output.pending = None;
                output.pending_since = None;
            }
            self.events.push_back(BackendEvent::DeviceLost);
        }
    }

    fn next_latch_deadline(&self) -> Option<Instant> {
        let timeout = self.latch_timeout?;
        self.outputs
            .iter()
            .filter(|o| o.pending.is_some() && !o.overdue)
            .filter_map(|o| o.pending_since)
            .map(|since| since + timeout)
            .min()
    }

    fn poll_device(&self, timeout: Option<Duration>) -> io::Result<rustix::event::PollFlags> {
        use rustix::event::{poll, PollFd, PollFlags, Timespec};
        let timeout = timeout.map(|t| Timespec {
            tv_sec: t.as_secs() as _,
            tv_nsec: t.subsec_nanos() as _,
        });
        let mut fds = [PollFd::new(&self.card, PollFlags::IN)];
        loop {
            match poll(&mut fds, timeout.as_ref()) {
                Ok(_) => return Ok(fds[0].revents()),
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn read_events(&mut self) {
        use rustix::event::PollFlags;
        loop {
            let revents = match self.poll_device(Some(Duration::ZERO)) {
                Ok(revents) => revents,
                Err(error) => {
                    let errno = error.raw_os_error().unwrap_or(0);
                    self.log_failure(DrmFailure::ReadEvents { errno });
                    break;
                }
            };
            if revents.intersects(PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL) {
                self.mark_lost(DrmFailure::Hangup {
                    revents: u32::from(revents.bits()),
                });
                return;
            }
            if !revents.contains(PollFlags::IN) {
                break;
            }
            let events = match self.card.receive_events() {
                Ok(events) => events,
                Err(error) => {
                    let errno = error.raw_os_error().unwrap_or(0);
                    if class_of(errno) == ErrnoClass::Gone {
                        self.mark_lost(DrmFailure::Lost {
                            operation: Operation::Events,
                            errno,
                        });
                        return;
                    }
                    self.log_failure(DrmFailure::ReadEvents { errno });
                    break;
                }
            };
            for event in events {
                let Event::PageFlip(flip) = event else {
                    continue;
                };
                let Some(output) = self.outputs.iter_mut().find(|o| o.crtc == flip.crtc) else {
                    continue;
                };
                let Some(submission) = output.pending.take() else {
                    continue;
                };
                output.pending_since = None;
                output.overdue = false;
                let info = LatchInfo {
                    output: OutputId(u32::from(output.crtc)),
                    submission,
                    sequence: flip.frame,
                    timestamp: flip.duration,
                };
                output.last_latch = Some(info);
                if self.latches.len() == LOG_LENGTH {
                    self.latches.pop_front();
                }
                self.latches.push_back(info);
                self.events.push_back(BackendEvent::Latched {
                    output: info.output,
                    submission,
                });
            }
        }
        self.check_overdue();
    }

    /// Logs pending latches older than the latch timeout, once each, after
    /// making sure the device is still there.
    fn check_overdue(&mut self) {
        let Some(timeout) = self.latch_timeout else {
            return;
        };
        let now = Instant::now();
        let overdue: Vec<(usize, SubmissionId, Duration)> = self
            .outputs
            .iter()
            .enumerate()
            .filter(|(_, o)| !o.overdue)
            .filter_map(|(index, o)| {
                let waited = now.saturating_duration_since(o.pending_since?);
                (waited >= timeout).then_some((index, o.pending?, waited))
            })
            .collect();
        if overdue.is_empty() || !self.probe_device() {
            return;
        }
        for (index, submission, waited) in overdue {
            let output = &mut self.outputs[index];
            output.overdue = true;
            let output = OutputId(u32::from(output.crtc));
            self.log_failure(DrmFailure::LatchTimeout {
                output,
                submission,
                waited,
            });
        }
    }

    /// Turns a failed request into the engine's error and logs the reason
    /// with the raw `errno`.
    fn fail(&mut self, operation: Operation, error: io::Error, primary_used: bool) -> BackendError {
        let errno = error.raw_os_error().unwrap_or(0);
        let refused = DrmFailure::Refused {
            operation,
            errno,
            primary_unused: !primary_used,
        };
        match class_of(errno) {
            ErrnoClass::Busy => {
                self.log_failure(refused);
                BackendError::Busy
            }
            ErrnoClass::Refused => {
                self.log_failure(refused);
                BackendError::Rejected
            }
            // Not master now: a compositor holds it, or the lessor dropped
            // it. That can change again, so the device is not lost, unless
            // its objects are gone as well.
            ErrnoClass::Permission if self.objects_gone().is_none() => {
                self.log_failure(DrmFailure::NotMaster { operation, errno });
                BackendError::Platform(errno)
            }
            // Also what an unknown framebuffer or property gives: lost only
            // if the CRTCs are gone, as after a lease revocation.
            ErrnoClass::Missing if self.objects_gone().is_none() => {
                self.log_failure(refused);
                BackendError::Platform(errno)
            }
            ErrnoClass::Permission | ErrnoClass::Missing | ErrnoClass::Gone => {
                self.mark_lost(DrmFailure::Lost { operation, errno });
                BackendError::DeviceLost
            }
            ErrnoClass::Other => {
                self.log_failure(refused);
                BackendError::Platform(errno)
            }
        }
    }

    /// Whether the plan uses a primary plane. With
    /// [`DrmBackend::set_require_primary`], refuses a plan whose bottom layer
    /// is not on one.
    fn primary_check(&mut self, frame: &Frame<'_>, plan: &Plan) -> Result<bool, BackendError> {
        let planes = &self.planes;
        let kind_of = |index: usize| planes.get(index).map(|p| p.caps.kind);
        let used = plan
            .assignments()
            .iter()
            .any(|a| kind_of(a.plane_index) == Some(PlaneKind::Primary));
        if self.require_primary && !bottom_on_primary(plan.assignments(), kind_of) {
            self.log_failure(DrmFailure::NoPrimaryPlane {
                output: frame.output,
            });
            return Err(BackendError::Rejected);
        }
        Ok(used)
    }

    /// Builds the atomic request for a planned frame. Returns the request
    /// and whether it includes a modeset.
    fn request(
        &self,
        frame: &Frame<'_>,
        plan: &Plan,
    ) -> Result<(AtomicModeReq, bool), BackendError> {
        let output = self
            .outputs
            .iter()
            .find(|o| OutputId(u32::from(o.crtc)) == frame.output)
            .ok_or(BackendError::Rejected)?;
        let mut request = AtomicModeReq::new();
        let modeset = !output.modeset_done;
        if modeset {
            request.add_property(
                output.connector,
                output.connector_crtc_id,
                property::Value::CRTC(Some(output.crtc)),
            );
            request.add_property(
                output.crtc,
                output.crtc_mode_id,
                property::Value::Blob(output.mode_blob),
            );
            request.add_property(
                output.crtc,
                output.crtc_active,
                property::Value::Boolean(true),
            );
        }

        for assignment in plan.assignments() {
            let plane = self
                .planes
                .get(assignment.plane_index)
                .filter(|p| p.caps.id == assignment.plane)
                .ok_or(BackendError::Rejected)?;
            let layer = frame
                .layers
                .get(assignment.layer)
                .ok_or(BackendError::Rejected)?;
            let framebuffer = self
                .buffers
                .iter()
                .find(|b| b.id == layer.buffer)
                .ok_or(BackendError::UnknownBuffer(layer.buffer))?
                .framebuffer;
            let props = &plane.props;
            let handle = plane.handle;
            request.add_property(
                handle,
                props.fb_id,
                property::Value::Framebuffer(Some(framebuffer)),
            );
            request.add_property(
                handle,
                props.crtc_id,
                property::Value::CRTC(Some(output.crtc)),
            );
            // Source coordinates are 16.16 fixed point.
            let source = [
                assignment.source.x as u64,
                assignment.source.y as u64,
                assignment.source.width as u64,
                assignment.source.height as u64,
            ];
            for (prop, value) in props.src.iter().zip(source) {
                request.add_property(handle, *prop, property::Value::UnsignedRange(value << 16));
            }
            let destination = assignment.destination;
            request.add_property(
                handle,
                props.dst[0],
                property::Value::SignedRange(destination.x as i64),
            );
            request.add_property(
                handle,
                props.dst[1],
                property::Value::SignedRange(destination.y as i64),
            );
            request.add_property(
                handle,
                props.dst[2],
                property::Value::UnsignedRange(destination.width as u64),
            );
            request.add_property(
                handle,
                props.dst[3],
                property::Value::UnsignedRange(destination.height as u64),
            );
            if let Some(zpos) = props.zpos {
                request.add_property(
                    handle,
                    zpos,
                    property::Value::UnsignedRange(assignment.zpos as u64),
                );
            }
            if let Some((alpha, max)) = props.alpha {
                let value = layer.alpha as u64 * max / ALPHA_OPAQUE as u64;
                request.add_property(handle, alpha, property::Value::UnsignedRange(value));
            }
            if let Some((blend, values)) = props.blend {
                // An opaque format ignores the mode; leave it alone if the
                // plane has no matching value.
                if let Some(value) = values[layer.blend as usize] {
                    request.add_raw_property(handle.into(), blend, value);
                }
            }
        }

        // Turn off planes this output was showing that the frame leaves out.
        for plane in &self.planes {
            let assigned = plan.assignments().iter().any(|a| a.plane == plane.caps.id);
            if plane.crtc == Some(output.crtc) && !assigned {
                request.add_property(
                    plane.handle,
                    plane.props.fb_id,
                    property::Value::Framebuffer(None),
                );
                request.add_property(
                    plane.handle,
                    plane.props.crtc_id,
                    property::Value::CRTC(None),
                );
            }
        }
        Ok((request, modeset))
    }
}

impl DisplayBackend for DrmBackend {
    fn capabilities(&mut self, caps: &mut Capabilities) {
        if self.lost {
            return;
        }
        for output in &self.outputs {
            let (width, height) = output.mode.size();
            let _ = caps.outputs.push(OutputCaps {
                id: OutputId(u32::from(output.crtc)),
                size: Size::new(width as u32, height as u32),
                refresh_mhz: refresh_millihertz(&output.mode),
            });
        }
        for plane in &self.planes {
            let _ = caps.planes.push(plane.caps);
        }
        self.options.profile.apply(caps);
    }

    fn check(&mut self, frame: &Frame<'_>, plan: &Plan) -> Result<(), BackendError> {
        if self.lost {
            return Err(BackendError::DeviceLost);
        }
        let primary_used = self.primary_check(frame, plan)?;
        let (request, modeset) = self.request(frame, plan)?;
        let mut flags = AtomicCommitFlags::TEST_ONLY;
        if modeset {
            flags |= AtomicCommitFlags::ALLOW_MODESET;
        }
        let Err(error) = self.card.atomic_commit(flags, request) else {
            return Ok(());
        };
        // A frame that works only with a modeset is not unsupported, and is
        // logged as such. It is still refused: commit() does not allow a
        // modeset after the first one.
        let errno = error.raw_os_error().unwrap_or(0);
        if !modeset && class_of(errno) == ErrnoClass::Refused {
            let (request, _) = self.request(frame, plan)?;
            let flags = AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::ALLOW_MODESET;
            if self.card.atomic_commit(flags, request).is_ok() {
                self.log_failure(DrmFailure::NeedsModeset { errno });
                return Err(BackendError::Rejected);
            }
        }
        Err(self.fail(Operation::Check, error, primary_used))
    }

    fn commit(
        &mut self,
        submission: SubmissionId,
        frame: &Frame<'_>,
        plan: &Plan,
    ) -> Result<(), BackendError> {
        if self.lost {
            return Err(BackendError::DeviceLost);
        }
        let primary_used = self.primary_check(frame, plan)?;
        let (request, modeset) = self.request(frame, plan)?;
        // The first commit of an output sets the mode and blocks. A modeset
        // can take several frame times (PLL lock, LVDS PHY power-up), and a
        // nonblocking one fails with EBUSY while the kernel is still
        // finishing a commit of whoever drove the output before, or of a
        // CRTC sharing hardware with this one (dpu95 pulls the other
        // stream's state in). Blocking, the kernel waits for those instead,
        // and the latch timeout starts once the mode is set. Later commits
        // are page flips and do not block.
        let flags = if modeset {
            AtomicCommitFlags::ALLOW_MODESET | AtomicCommitFlags::PAGE_FLIP_EVENT
        } else {
            AtomicCommitFlags::NONBLOCK | AtomicCommitFlags::PAGE_FLIP_EVENT
        };
        if let Err(error) = self.card.atomic_commit(flags, request) {
            return Err(self.fail(Operation::Commit, error, primary_used));
        }

        let crtc = self
            .outputs
            .iter()
            .find(|o| OutputId(u32::from(o.crtc)) == frame.output)
            .map(|o| o.crtc);
        for plane in &mut self.planes {
            let assigned = plan.assignments().iter().any(|a| a.plane == plane.caps.id);
            if assigned {
                plane.crtc = crtc;
            } else if plane.crtc == crtc {
                plane.crtc = None;
            }
        }
        if let Some(output) = self.outputs.iter_mut().find(|o| Some(o.crtc) == crtc) {
            output.modeset_done = true;
            output.pending = Some(submission);
            output.pending_since = Some(Instant::now());
            output.overdue = false;
        }
        Ok(())
    }

    fn poll_event(&mut self) -> Option<BackendEvent> {
        if !self.lost {
            self.read_events();
        }
        self.events.pop_front()
    }
}

/// How an `errno` from the kernel is treated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ErrnoClass {
    /// `EBUSY`: a previous commit is not done.
    Busy,
    /// `EINVAL`, `ERANGE`, `ENOSPC`: this configuration is refused.
    Refused,
    /// `EACCES`, `EPERM`: not DRM master.
    Permission,
    /// `ENOENT`: an object is unknown, or gone with a revoked lease.
    Missing,
    /// `ENODEV`: the device was removed.
    Gone,
    /// Anything else.
    Other,
}

/// The backend's plane selection: order the planes, then drop those beyond
/// [`MAX_PLANES`].
///
/// `key` gives whether a plane can reach a probed output (its possible-CRTC
/// mask covers a probed CRTC) and its kind. Reachable planes are ranked
/// before all others, so a device with more planes than the limit cannot
/// crowd the planes of the connected outputs out of it; the cap is applied
/// only after this ranking, never to the raw plane list. When the limit
/// bites, primaries are kept first, then cursors, then overlays: a cursor is
/// worth more than the last overlay. The survivors are then put in the order
/// the rest of the backend wants: reachable first, within each group primary,
/// overlays, cursor (the bottom layer lands on a primary by preference). Both
/// sorts are stable, so the device's own order is kept among equals and the
/// selection is deterministic.
///
/// Exposed for testing the selection on synthetic plane lists without a DRM
/// device; the backend applies it to the whole list the card exposes.
pub fn keep_planes<T>(mut planes: Vec<T>, key: impl Fn(&T) -> (bool, PlaneKind)) -> Vec<T> {
    planes.sort_by_key(|p| {
        let (reachable, kind) = key(p);
        let priority = match kind {
            PlaneKind::Primary => 0,
            PlaneKind::Cursor => 1,
            PlaneKind::Overlay => 2,
        };
        (!reachable, priority)
    });
    planes.truncate(MAX_PLANES);
    planes.sort_by_key(|p| {
        let (reachable, kind) = key(p);
        let order = match kind {
            PlaneKind::Primary => 0,
            PlaneKind::Overlay => 1,
            PlaneKind::Cursor => 2,
        };
        (!reachable, order)
    });
    planes
}

fn class_of(errno: i32) -> ErrnoClass {
    use rustix::io::Errno;
    // 0 is what an error without an errno gives; rustix refuses it.
    if !(1..4096).contains(&errno) {
        return ErrnoClass::Other;
    }
    let code = Errno::from_raw_os_error(errno);
    if code == Errno::BUSY {
        ErrnoClass::Busy
    } else if code == Errno::INVAL || code == Errno::RANGE || code == Errno::NOSPC {
        ErrnoClass::Refused
    } else if code == Errno::ACCESS || code == Errno::PERM {
        ErrnoClass::Permission
    } else if code == Errno::NOENT {
        ErrnoClass::Missing
    } else if code == Errno::NODEV {
        ErrnoClass::Gone
    } else {
        ErrnoClass::Other
    }
}

/// Whether the lowest assignment of a plan is on a primary plane. An empty
/// plan has none.
fn bottom_on_primary(
    assignments: &[Assignment],
    kind_of: impl Fn(usize) -> Option<PlaneKind>,
) -> bool {
    assignments
        .iter()
        .min_by_key(|a| a.zpos)
        .is_some_and(|a| kind_of(a.plane_index) == Some(PlaneKind::Primary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use composition_core::Rect;

    #[test]
    fn reachable_planes_survive_truncation_on_a_many_crtc_device() {
        // Shaped like vc4: 5 CRTCs with a primary and a cursor each, and 50
        // overlays. Only CRTC 0 is connected; the device lists the other
        // CRTCs' planes first.
        let mut planes: Vec<(u32, bool, PlaneKind)> = Vec::new();
        let mut id = 0;
        for crtc in (0..5).rev() {
            planes.push((id, crtc == 0, PlaneKind::Primary));
            planes.push((id + 1, crtc == 0, PlaneKind::Cursor));
            id += 2;
        }
        for n in 0..50 {
            planes.push((id + n, n < 20, PlaneKind::Overlay));
        }
        assert!(planes.len() > MAX_PLANES);
        let kept = keep_planes(planes, |p| (p.1, p.2));
        assert_eq!(kept.len(), MAX_PLANES);
        assert!(kept.iter().all(|p| p.1), "an unreachable plane was kept");
        let count = |k| kept.iter().filter(|p| p.2 == k).count();
        assert_eq!(count(PlaneKind::Primary), 1);
        assert_eq!(count(PlaneKind::Overlay), MAX_PLANES - 2);
        assert_eq!(count(PlaneKind::Cursor), 1);
        assert_eq!(kept[0].2, PlaneKind::Primary);
        assert_eq!(kept[MAX_PLANES - 1].2, PlaneKind::Cursor);
        // Overlays keep the device order.
        let ids: Vec<u32> = kept[1..MAX_PLANES - 1].iter().map(|p| p.0).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn cursor_of_a_reachable_group_is_kept_when_it_fits() {
        let planes = vec![
            (0, false, PlaneKind::Primary),
            (1, true, PlaneKind::Cursor),
            (2, true, PlaneKind::Overlay),
            (3, true, PlaneKind::Primary),
        ];
        let kept = keep_planes(planes, |p| (p.1, p.2));
        let ids: Vec<u32> = kept.iter().map(|p| p.0).collect();
        assert_eq!(ids, [3, 2, 1, 0]);
    }

    #[test]
    fn errno_classes_keep_permission_apart_from_loss() {
        use rustix::io::Errno;
        let class = |e: Errno| class_of(e.raw_os_error());
        assert_eq!(class(Errno::ACCESS), ErrnoClass::Permission);
        assert_eq!(class(Errno::PERM), ErrnoClass::Permission);
        assert_eq!(class(Errno::NOENT), ErrnoClass::Missing);
        assert_eq!(class(Errno::NODEV), ErrnoClass::Gone);
        assert_eq!(class(Errno::BUSY), ErrnoClass::Busy);
        assert_eq!(class(Errno::INVAL), ErrnoClass::Refused);
        assert_eq!(class(Errno::RANGE), ErrnoClass::Refused);
        assert_eq!(class(Errno::NOSPC), ErrnoClass::Refused);
        assert_eq!(class(Errno::NOMEM), ErrnoClass::Other);
        assert_eq!(class_of(0), ErrnoClass::Other);
    }

    fn assignment(plane_index: usize, zpos: i32) -> Assignment {
        Assignment {
            layer: plane_index,
            plane: PlaneId(plane_index as u32 + 30),
            plane_index,
            zpos,
            source: Rect::new(0, 0, 8, 8),
            destination: Rect::new(0, 0, 8, 8),
        }
    }

    #[test]
    fn bottom_layer_must_be_on_the_primary_plane() {
        let kinds = [PlaneKind::Overlay, PlaneKind::Primary, PlaneKind::Overlay];
        let kind_of = |i: usize| kinds.get(i).copied();
        assert!(bottom_on_primary(
            &[assignment(0, 2), assignment(1, 0)],
            kind_of
        ));
        assert!(!bottom_on_primary(
            &[assignment(0, 0), assignment(1, 1)],
            kind_of
        ));
        assert!(!bottom_on_primary(
            &[assignment(0, 0), assignment(2, 1)],
            kind_of
        ));
        assert!(!bottom_on_primary(&[], kind_of));
        assert!(!bottom_on_primary(&[assignment(7, 0)], kind_of));
    }
}
