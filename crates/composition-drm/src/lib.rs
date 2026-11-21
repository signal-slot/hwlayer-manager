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

#![forbid(unsafe_code)]

pub mod profile;

pub use profile::DriverProfile;

use std::collections::VecDeque;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;
use std::time::Duration;

use composition_core::limits::{MAX_FORMATS, MAX_OUTPUTS, MAX_PLANES};
use composition_core::{
    BackendError, BackendEvent, BlendMode, BufferId, BufferInfo, Capabilities, DisplayBackend,
    FormatModifier, Frame, Modifier, OutputCaps, OutputId, PixelFormat, Plan, PlaneCaps, PlaneId,
    PlaneKind, ScaleLimits, Size, SubmissionId, Zpos, ALPHA_OPAQUE,
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

        // Primary first so that the bottom layer lands there by preference.
        planes.sort_by_key(|(_, p)| match p.caps.kind {
            PlaneKind::Primary => 0,
            PlaneKind::Overlay => 1,
            PlaneKind::Cursor => 2,
        });
        let mut overlay_position = 1;
        for (conventional, plane) in &mut planes {
            if *conventional && plane.caps.kind == PlaneKind::Overlay {
                plane.caps.zpos = Zpos::Fixed(overlay_position);
                overlay_position += 1;
            }
        }
        planes.truncate(MAX_PLANES);

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
    /// Returns `true` if [`DisplayBackend::poll_event`] has something to do.
    pub fn wait_for_events(&self, timeout: Option<Duration>) -> io::Result<bool> {
        if !self.events.is_empty() {
            return Ok(true);
        }
        self.readable(timeout)
    }

    fn readable(&self, timeout: Option<Duration>) -> io::Result<bool> {
        use rustix::event::{poll, PollFd, PollFlags, Timespec};
        let timeout = timeout.map(|t| Timespec {
            tv_sec: t.as_secs() as _,
            tv_nsec: t.subsec_nanos() as _,
        });
        let mut fds = [PollFd::new(&self.card, PollFlags::IN)];
        loop {
            match poll(&mut fds, timeout.as_ref()) {
                Ok(count) => return Ok(count > 0),
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn read_events(&mut self) {
        while let Ok(true) = self.readable(Some(Duration::ZERO)) {
            let Ok(events) = self.card.receive_events() else {
                break;
            };
            for event in events {
                if let Event::PageFlip(flip) = event {
                    let output = self.outputs.iter_mut().find(|o| o.crtc == flip.crtc);
                    if let Some(output) = output {
                        if let Some(submission) = output.pending.take() {
                            self.events.push_back(BackendEvent::Latched {
                                output: OutputId(u32::from(output.crtc)),
                                submission,
                            });
                        }
                    }
                }
            }
        }
    }

    fn fail(&mut self, error: io::Error) -> BackendError {
        use rustix::io::Errno;
        let errno = error.raw_os_error().unwrap_or(0);
        let code = Errno::from_raw_os_error(errno);
        if code == Errno::BUSY {
            BackendError::Busy
        } else if code == Errno::INVAL || code == Errno::RANGE || code == Errno::NOSPC {
            BackendError::Rejected
        } else if code == Errno::ACCESS || code == Errno::NOENT || code == Errno::NODEV {
            // The objects are gone or no longer ours: a revoked lease, a
            // dropped master, or a removed device.
            if !self.lost {
                self.lost = true;
                for output in &mut self.outputs {
                    output.pending = None;
                }
                self.events.push_back(BackendEvent::DeviceLost);
            }
            BackendError::DeviceLost
        } else {
            BackendError::Platform(errno)
        }
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
        let (request, modeset) = self.request(frame, plan)?;
        let mut flags = AtomicCommitFlags::TEST_ONLY;
        if modeset {
            flags |= AtomicCommitFlags::ALLOW_MODESET;
        }
        self.card
            .atomic_commit(flags, request)
            .map_err(|e| self.fail(e))
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
        let (request, modeset) = self.request(frame, plan)?;
        let mut flags = AtomicCommitFlags::NONBLOCK | AtomicCommitFlags::PAGE_FLIP_EVENT;
        if modeset {
            flags |= AtomicCommitFlags::ALLOW_MODESET;
        }
        self.card
            .atomic_commit(flags, request)
            .map_err(|e| self.fail(e))?;

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
