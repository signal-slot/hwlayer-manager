//! One open DRM descriptor driven through the engine, on one output.

use std::collections::HashMap;
use std::io;
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use composition_core::limits::MAX_OUTPUTS;
use composition_core::planner::BufferLookup;
use composition_core::{
    BufferId, BufferInfo, DisplayEvent, Engine, Error, Frame, Layer, Modifier, OutputId,
    PixelFormat, Plan, PlaneCaps, PlaneId, PlaneKind, Rect, Size, SubmissionId, Zpos,
};
use composition_drm::{Card, DmabufPlane, DriverProfile, DrmBackend, DrmOptions};
use drm::buffer::{Buffer as _, DrmFourcc};
use drm::control::{connector, crtc, Device as ControlDevice, Mode};
use drm::Device as BasicDevice;

use crate::draw::{self, Op};
use crate::producer::Producers;
use crate::record::{list, Record};
use crate::recovery::Loss;
use crate::source::{Holder, Source};
use crate::stats::micros;
use crate::trace::SharedTrace;

/// How long a commit may take to be latched before it counts as lost.
pub const LATCH_TIMEOUT: Duration = Duration::from_secs(1);

/// Command-line settings shared by all scenarios.
#[derive(Clone, Debug)]
pub struct Options {
    /// Where the descriptor comes from.
    pub source: Source,
    /// Connector to drive, by name or name prefix (`LVDS`, `HDMI-A-1`).
    pub connector: Option<String>,
    /// Driver profile handed to the backend.
    pub profile: DriverProfile,
    /// How long to hold each picture for a camera, in seconds.
    pub seconds: u64,
    /// Frame count for `move` and `rotate`; each has its own default.
    pub frames: Option<u32>,
    /// Buffers per plane in `rotate`.
    pub buffers: u32,
    /// Who allocates and draws the buffers.
    pub producers: ProducerMode,
    /// How `recovery` takes the display away.
    pub loss: Loss,
}

/// Who allocates and draws the buffers.
#[derive(Clone, Debug)]
pub enum ProducerMode {
    /// The harness, on the descriptor the engine drives.
    Local,
    /// One producer process per layer, opening this device node without
    /// DRM master and passing DMA-BUFs (see `producer.rs`).
    External {
        /// Device node the producers open.
        device: PathBuf,
    },
}

impl ProducerMode {
    /// Short name, for the records.
    pub fn name(&self) -> &'static str {
        match self {
            ProducerMode::Local => "local",
            ProducerMode::External { .. } => "external",
        }
    }
}

/// Metadata of the buffers this session registered, for planner-only runs.
#[derive(Default)]
pub struct Infos(HashMap<BufferId, BufferInfo>);

impl BufferLookup for Infos {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.get(&buffer).copied()
    }
}

/// A frame that was validated, committed and latched.
pub struct Shown {
    /// The submission.
    pub submission: SubmissionId,
    /// The plan it was committed with.
    pub plan: Plan,
    /// Time spent in `Engine::validate` (planning and `TEST_ONLY`).
    pub validate_us: u64,
    /// Time spent in `Engine::commit`.
    pub commit_us: u64,
    /// From the start of `Engine::commit` to the latch being seen.
    pub latency_us: u64,
    /// Display events seen while waiting, the `Presented` included.
    pub events: Vec<DisplayEvent>,
}

/// The engine on one output of one descriptor.
pub struct Session {
    /// The engine.
    pub engine: Box<Engine<DrmBackend, SharedTrace>>,
    /// The output driven.
    pub output: OutputId,
    /// Its resolution.
    pub size: Size,
    /// Its refresh rate in millihertz.
    pub refresh_mhz: u32,
    infos: Infos,
    next_buffer: u32,
    /// Driver profile, for a backend reopened on the same descriptor.
    profile: DriverProfile,
    /// Producer processes, with `--producers external`.
    producers: Option<Producers>,
    /// Keeps the descriptor valid (lessor, lease-manager connection).
    /// Declared after `engine`, so the lease descriptor is closed before
    /// the lease is released.
    holder: Holder,
}

/// A connected connector as the backend pairs it with a CRTC.
pub struct Pairing {
    /// The CRTC.
    pub crtc: crtc::Handle,
    /// The connector.
    pub handle: connector::Handle,
    /// Its name, such as `LVDS-1`.
    pub connector: String,
    /// The mode it runs or would run.
    pub mode: Mode,
}

fn connector_name(info: &connector::Info) -> String {
    format!(
        "{}-{}",
        crate::conn_names::interface_name(info.interface()),
        info.interface_id()
    )
}

/// Pairs connected connectors with CRTCs the way `DrmBackend` does, to name
/// the outputs it reports (it identifies them by CRTC only).
pub fn pairings(card: &Card) -> io::Result<Vec<Pairing>> {
    let resources = card.resource_handles()?;
    let mut found: Vec<Pairing> = Vec::new();
    for handle in resources.connectors() {
        if found.len() == MAX_OUTPUTS {
            break;
        }
        let info = card.get_connector(*handle, false)?;
        if info.state() != connector::State::Connected {
            continue;
        }
        let taken = |c: &crtc::Handle| found.iter().any(|p| p.crtc == *c);
        let mut chosen = None;
        if let Some(encoder) = info.current_encoder() {
            chosen = card.get_encoder(encoder)?.crtc().filter(|c| !taken(c));
        }
        if chosen.is_none() {
            for encoder in info.encoders() {
                let possible = card.get_encoder(*encoder)?.possible_crtcs();
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
        let Some(mode) = card
            .get_crtc(crtc)?
            .mode()
            .or_else(|| info.modes().first().copied())
        else {
            continue;
        };
        found.push(Pairing {
            crtc,
            handle: *handle,
            connector: connector_name(&info),
            mode,
        });
    }
    Ok(found)
}

/// A FourCC code as text, such as `XR24`.
pub fn fourcc(format: PixelFormat) -> String {
    format
        .0
        .to_le_bytes()
        .iter()
        .map(|&b| if b.is_ascii_graphic() { b as char } else { '?' })
        .collect::<String>()
}

/// A rectangle as `x,y,wxh`.
pub fn rect(r: Rect) -> String {
    format!("{},{},{}x{}", r.x, r.y, r.width, r.height)
}

/// Plane kind as text.
pub fn kind_name(kind: PlaneKind) -> &'static str {
    match kind {
        PlaneKind::Primary => "primary",
        PlaneKind::Overlay => "overlay",
        PlaneKind::Cursor => "cursor",
    }
}

fn zpos_text(zpos: Zpos) -> String {
    match zpos {
        Zpos::Fixed(z) => format!("fixed:{z}"),
        Zpos::Range { min, max } => format!("{min}..{max}"),
    }
}

/// Plane ids of a plan, in layer order.
pub fn planes_of(plan: &Plan) -> Vec<u32> {
    let mut assignments = plan.assignments().to_vec();
    assignments.sort_by_key(|a| a.layer);
    assignments.iter().map(|a| a.plane.0).collect()
}

/// An engine error as text.
pub fn describe(error: &Error) -> String {
    format!("{error:?}")
}

fn os(context: &str, error: io::Error) -> String {
    format!("{context}: {error}")
}

impl Session {
    /// Opens the source, makes sure it is DRM master if it has to be, and
    /// picks the output. Prints what was found.
    pub fn open(options: &Options, scenario: &str) -> Result<Self, String> {
        let opened = options
            .source
            .open()
            .map_err(|e| os(&format!("open {}", options.source.describe()), e))?;
        let holder = opened.holder;
        let backend = DrmBackend::from_fd(
            opened.fd,
            DrmOptions {
                scaling: None,
                profile: options.profile,
            },
        )
        .map_err(|e| os("DRM backend", e))?;
        let driver = backend.driver_name().unwrap_or_else(|_| "?".into());
        let master = if options.source.needs_master() {
            Some(backend.card().acquire_master_lock())
        } else {
            None
        };
        let pairings = pairings(backend.card()).map_err(|e| os("connectors", e))?;
        Record::new("session")
            .field("scenario", scenario)
            .field("source", options.source.kind())
            .field("device", options.source.describe())
            .field("driver", &driver)
            .field_opt(
                "master",
                master
                    .as_ref()
                    .map(|m| if m.is_ok() { "yes" } else { "no" }),
            )
            .field(
                "profile",
                match options.profile {
                    DriverProfile::Generic => "generic",
                    DriverProfile::Imx95Dpu => "imx95",
                    DriverProfile::Stm32Ltdc => "stm32",
                    DriverProfile::Rk3588Vop2 => "rk3588",
                    DriverProfile::Vc4Hvs5 => "vc4-hvs5",
                    DriverProfile::Vc4Hvs6 => "vc4-hvs6",
                },
            )
            .field("producers", options.producers.name())
            .emit();
        if let Some(Err(error)) = master {
            return Err(format!(
                "not DRM master ({error}); stop the compositor that owns the device, \
                 or use another source"
            ));
        }

        let engine = Box::new(Engine::new(backend).with_trace(SharedTrace));
        let caps = engine.capabilities();
        let mut selected = None;
        for (index, out) in caps.outputs.iter().enumerate() {
            let pairing = pairings.iter().find(|p| u32::from(p.crtc) == out.id.0);
            let name = pairing.map_or("?".to_string(), |p| p.connector.clone());
            let wanted = match &options.connector {
                Some(want) => crate::conn_names::connector_matches(&name, want),
                None => true,
            };
            let pick = wanted && selected.is_none();
            if pick {
                selected = Some(*out);
            }
            let mut record = Record::new("output")
                .field("index", index)
                .field("id", out.id.0)
                .field("connector", &name)
                .field("width", out.size.width)
                .field("height", out.size.height)
                .field("refresh_mhz", out.refresh_mhz);
            if let Some(p) = pairing {
                record = record
                    .field("mode", p.mode.name().to_string_lossy())
                    .field("vrefresh", p.mode.vrefresh())
                    .field("clock_khz", p.mode.clock());
            }
            record
                .field("selected", if pick { "yes" } else { "no" })
                .emit();
        }
        let Some(out) = selected else {
            return Err(match &options.connector {
                Some(want) => format!("no connected output matches connector {want}"),
                None => "no connected output".into(),
            });
        };
        let out_index = caps
            .outputs
            .iter()
            .position(|o| o.id == out.id)
            .unwrap_or(0);
        for plane in caps.planes.iter() {
            let formats = list(plane.formats.iter().map(|f| {
                if f.modifier == Modifier::LINEAR {
                    fourcc(f.format)
                } else {
                    format!("{}@{:#x}", fourcc(f.format), f.modifier.0)
                }
            }));
            Record::new("plane")
                .field("id", plane.id.0)
                .field("kind", kind_name(plane.kind))
                .field("zpos", zpos_text(plane.zpos))
                .field(
                    "on_output",
                    if plane.outputs & (1 << out_index) != 0 {
                        "yes"
                    } else {
                        "no"
                    },
                )
                .field("alpha", plane.plane_alpha)
                .field("blend_mask", plane.blend_modes)
                .field("scaling", plane.scaling.is_some())
                .field("formats", formats)
                .emit();
        }
        if let Holder::Lessor(lessor) = &holder {
            // The engine must see the leased planes and nothing else.
            let leased = lessor.leased.plane_ids();
            let seen: Vec<u32> = caps.planes.iter().map(|p| p.id.0).collect();
            let foreign: Vec<u32> = seen
                .iter()
                .copied()
                .filter(|id| !leased.contains(id))
                .collect();
            let missing: Vec<u32> = leased
                .iter()
                .copied()
                .filter(|id| !seen.contains(id))
                .collect();
            Record::new("lease_check")
                .field("leased_planes", list(leased.iter()))
                .field("engine_planes", list(seen.iter()))
                .field("not_leased", list(foreign.iter()))
                .field("leased_not_seen", list(missing.iter()))
                .field("outputs", caps.outputs.len())
                .emit();
            if !foreign.is_empty() {
                return Err(format!(
                    "the engine sees planes outside the lease: {}",
                    list(foreign.iter())
                ));
            }
        }
        for group in caps.resource_groups.iter() {
            Record::new("resource_group")
                .field("consumers", group.consumers.0)
                .field("planes", format!("{:#x}", group.planes))
                .field("capacity", group.capacity)
                .emit();
        }
        Ok(Self {
            engine,
            output: out.id,
            size: out.size,
            refresh_mhz: out.refresh_mhz,
            infos: Infos::default(),
            next_buffer: 1,
            profile: options.profile,
            producers: match &options.producers {
                ProducerMode::Local => None,
                ProducerMode::External { device } => Some(Producers::new(device.clone())),
            },
            holder,
        })
    }

    /// The connector and CRTC of the selected output, with the mode the
    /// CRTC runs now.
    pub fn pairing(&self) -> Result<Pairing, String> {
        pairings(self.engine.backend().card())
            .map_err(|e| os("connectors", e))?
            .into_iter()
            .find(|p| u32::from(p.crtc) == self.output.0)
            .ok_or_else(|| "the selected output has no connector any more".into())
    }

    /// Replaces the engine with a new one on the same descriptor (the same
    /// DRM file, so the same master or lease), which finds the outputs as
    /// they are now: after a mode change, the new size. Buffers of the old
    /// engine are forgotten; their memory goes with the descriptor. Buffer
    /// ids keep counting, so producers never see one twice.
    pub fn reopen(&mut self) -> Result<(), String> {
        let fd = self
            .engine
            .backend()
            .card()
            .as_fd()
            .try_clone_to_owned()
            .map_err(|e| os("duplicate the DRM descriptor", e))?;
        let backend = DrmBackend::from_fd(
            fd,
            DrmOptions {
                scaling: None,
                profile: self.profile,
            },
        )
        .map_err(|e| os("DRM backend", e))?;
        let engine = Box::new(Engine::new(backend).with_trace(SharedTrace));
        let out = engine
            .capabilities()
            .outputs
            .iter()
            .find(|o| o.id == self.output)
            .copied()
            .ok_or("the reopened backend does not report the output")?;
        self.engine = engine;
        self.size = out.size;
        self.refresh_mhz = out.refresh_mhz;
        self.infos = Infos::default();
        Ok(())
    }

    /// Whether a lessor in this process holds the lease.
    pub fn can_revoke(&self) -> bool {
        matches!(&self.holder, Holder::Lessor(l) if l.holds_lease())
    }

    /// Revokes the lease, as its lessor.
    pub fn revoke(&mut self) -> Result<(), String> {
        match &mut self.holder {
            Holder::Lessor(lessor) => lessor.revoke().map_err(|e| os("revoke lease", e)),
            _ => Err("no lease made by this process to revoke".into()),
        }
    }

    /// Planes that can show something on the selected output.
    pub fn planes_on_output(&self) -> Vec<PlaneCaps> {
        let caps = self.engine.capabilities();
        let index = caps
            .outputs
            .iter()
            .position(|o| o.id == self.output)
            .unwrap_or(0);
        caps.planes
            .iter()
            .filter(|p| p.outputs & (1 << index) != 0)
            .copied()
            .collect()
    }

    /// Kind of a plane, as text.
    pub fn plane_kind(&self, plane: PlaneId) -> &'static str {
        self.engine
            .capabilities()
            .planes
            .iter()
            .find(|p| p.id == plane)
            .map_or("?", |p| kind_name(p.kind))
    }

    /// The metadata of every buffer registered so far.
    pub fn infos(&self) -> &Infos {
        &self.infos
    }

    /// Every buffer registered with this engine, by id.
    pub fn buffer_ids(&self) -> Vec<BufferId> {
        let mut ids: Vec<BufferId> = self.infos.0.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// A frame on the selected output.
    pub fn frame<'a>(&self, layers: &'a [Layer]) -> Frame<'a> {
        Frame {
            output: self.output,
            layers,
        }
    }

    fn new_id(&mut self) -> BufferId {
        let id = BufferId(self.next_buffer);
        self.next_buffer += 1;
        id
    }

    /// Allocates a buffer for `layer`, draws `ops` into it and marks it
    /// ready. The harness allocates it as a dumb buffer, or with
    /// `--producers external` the layer's producer process does (32-bit
    /// RGB formats; other formats stay with the harness).
    pub fn dumb(
        &mut self,
        layer: usize,
        size: Size,
        format: PixelFormat,
        ops: &[Op],
    ) -> Result<BufferId, String> {
        let id = self.new_id();
        let info = BufferInfo {
            size,
            format,
            modifier: Modifier::LINEAR,
        };
        let producer_format = [
            PixelFormat::XRGB8888,
            PixelFormat::ARGB8888,
            PixelFormat::XBGR8888,
            PixelFormat::ABGR8888,
        ]
        .contains(&format);
        if let (Some(producers), true) = (self.producers.as_mut(), producer_format) {
            let supplied = producers.alloc(layer, id, size, format, ops)?;
            if supplied.info != info {
                return Err(format!(
                    "producer {layer} supplied {:?}, asked for {info:?}",
                    supplied.info
                ));
            }
            self.engine
                .backend_mut()
                .import_dmabuf(
                    id,
                    info,
                    &[DmabufPlane {
                        fd: supplied.fd.as_fd(),
                        offset: supplied.offset,
                        pitch: supplied.pitch,
                    }],
                )
                .map_err(|e| os(&format!("import buffer of producer {layer}"), e))?;
            self.engine
                .register_buffer(id, info)
                .map_err(|e| describe(&e))?;
            self.infos.0.insert(id, info);
            self.engine.finish_write(id).map_err(|e| describe(&e))?;
            return Ok(id);
        }
        self.engine
            .backend_mut()
            .create_dumb_buffer(id, info)
            .map_err(|e| os("create dumb buffer", e))?;
        self.engine
            .register_buffer(id, info)
            .map_err(|e| describe(&e))?;
        self.infos.0.insert(id, info);
        self.engine
            .backend_mut()
            .draw_dumb_buffer(id, |pixels, pitch| draw::apply(pixels, pitch, ops))
            .map_err(|e| os("map dumb buffer", e))?;
        self.engine.finish_write(id).map_err(|e| describe(&e))?;
        Ok(id)
    }

    /// Redraws an idle buffer: `begin_write`, draw `ops`, `finish_write`.
    /// The engine refuses `begin_write` while the buffer is submitted or
    /// shown. A producer's buffer is drawn by the producer, which refuses
    /// unless it was told the buffer is released. If drawing fails the
    /// buffer stays writable, so it cannot be shown.
    pub fn redraw(&mut self, id: BufferId, ops: &[Op]) -> Result<(), String> {
        self.engine
            .begin_write(id)
            .map_err(|e| format!("begin_write: {}", describe(&e)))?;
        match self.producers.as_mut().filter(|p| p.owns(id)) {
            Some(producers) => {
                producers.draw(id, ops)?;
            }
            None => self
                .engine
                .backend_mut()
                .draw_dumb_buffer(id, |pixels, pitch| draw::apply(pixels, pitch, ops))
                .map_err(|e| os("map dumb buffer", e))?,
        }
        self.engine
            .finish_write(id)
            .map_err(|e| format!("finish_write: {}", describe(&e)))
    }

    /// Checksum of the visible bytes of a buffer, read without changing
    /// its state (by its producer, for a producer's buffer).
    pub fn checksum(&mut self, id: BufferId) -> Result<u64, String> {
        if let Some(producers) = self.producers.as_mut().filter(|p| p.owns(id)) {
            return producers.checksum(id);
        }
        let size = self
            .infos
            .0
            .get(&id)
            .ok_or_else(|| format!("unknown buffer {}", id.0))?
            .size;
        self.engine
            .backend_mut()
            .draw_dumb_buffer(id, |pixels, pitch| {
                draw::checksum(pixels, pitch, size.width, size.height)
            })
            .map_err(|e| os("map dumb buffer", e))
    }

    /// Allocates a solid NV12 buffer through a dumb buffer exported as a
    /// DMA-BUF and imported back, the path a video producer would take.
    pub fn nv12(&mut self, size: Size, yuv: (u8, u8, u8)) -> Result<BufferId, String> {
        let (w, h) = (size.width & !1, size.height & !1);
        let card = self.engine.backend().card();
        let mut dumb = card
            .create_dumb_buffer((w, h + h / 2), DrmFourcc::C8, 8)
            .map_err(|e| os("create NV12 dumb buffer", e))?;
        let pitch = dumb.pitch();
        let filled = card.map_dumb_buffer(&mut dumb).map(|mut map| {
            let pixels: &mut [u8] = map.as_mut();
            let chroma = pitch as usize * h as usize;
            for y in 0..h as usize {
                let row = y * pitch as usize;
                pixels[row..row + w as usize].fill(yuv.0);
            }
            for y in 0..(h / 2) as usize {
                let row = chroma + y * pitch as usize;
                for x in 0..(w / 2) as usize {
                    pixels[row + 2 * x] = yuv.1;
                    pixels[row + 2 * x + 1] = yuv.2;
                }
            }
        });
        let flags = (rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::RDWR).bits();
        let exported = filled.and_then(|()| card.buffer_to_prime_fd(dumb.handle(), flags));
        let fd = match exported {
            Ok(fd) => fd,
            Err(error) => {
                let _ = card.destroy_dumb_buffer(dumb);
                return Err(os("export NV12 buffer", error));
            }
        };
        let id = self.new_id();
        let info = BufferInfo {
            size: Size::new(w, h),
            format: PixelFormat::NV12,
            modifier: Modifier::LINEAR,
        };
        // The import closes the GEM handle (it is the same handle as the dumb
        // buffer's); the framebuffer keeps the memory alive from then on.
        let imported = self.engine.backend_mut().import_dmabuf(
            id,
            info,
            &[
                DmabufPlane {
                    fd: fd.as_fd(),
                    offset: 0,
                    pitch,
                },
                DmabufPlane {
                    fd: fd.as_fd(),
                    offset: pitch * h,
                    pitch,
                },
            ],
        );
        imported.map_err(|e| os("import NV12 buffer", e))?;
        self.engine
            .register_buffer(id, info)
            .map_err(|e| describe(&e))?;
        self.engine.finish_write(id).map_err(|e| describe(&e))?;
        self.infos.0.insert(id, info);
        Ok(id)
    }

    /// Registers a buffer with the engine only, with no memory behind it:
    /// for frames the planner is expected to refuse.
    pub fn register_only(&mut self, info: BufferInfo) -> Result<BufferId, String> {
        let id = self.new_id();
        self.engine
            .register_buffer(id, info)
            .map_err(|e| describe(&e))?;
        self.engine.finish_write(id).map_err(|e| describe(&e))?;
        self.infos.0.insert(id, info);
        Ok(id)
    }

    /// Prints the plane assignment of a plan.
    pub fn print_plan(&self, step: &str, layers: &[Layer], plan: &Plan) {
        let mut assignments = plan.assignments().to_vec();
        assignments.sort_by_key(|a| a.layer);
        for a in &assignments {
            let layer = &layers[a.layer];
            Record::new("assign")
                .field("step", step)
                .field("layer", a.layer)
                .field("buffer", layer.buffer.0)
                .field("plane", a.plane.0)
                .field("kind", self.plane_kind(a.plane))
                .field("zpos", a.zpos)
                .field("alpha", format!("{:#06x}", layer.alpha))
                .field("blend", format!("{:?}", layer.blend))
                .field("src", rect(a.source))
                .field("dst", rect(a.destination))
                .emit();
        }
        Record::new("plan")
            .field("step", step)
            .field("layers", layers.len())
            .field("assigned", assignments.len())
            .field("planes", list(planes_of(plan)))
            .emit();
    }

    /// Validates, commits and waits for the latch.
    pub fn present(&mut self, layers: &[Layer]) -> Result<Shown, String> {
        crate::stop::check()?;
        let frame = self.frame(layers);
        let start = Instant::now();
        let plan = self
            .engine
            .validate(&frame)
            .map_err(|e| format!("validate: {}", describe(&e)))?;
        let validate_us = micros(start.elapsed());
        let submitted = Instant::now();
        let submission = self
            .engine
            .commit(&frame, &plan)
            .map_err(|e| format!("commit: {}", describe(&e)))?;
        let commit_us = micros(submitted.elapsed());
        let (latched, events) = self.wait_presented(submission, LATCH_TIMEOUT)?;
        Ok(Shown {
            submission,
            plan,
            validate_us,
            commit_us,
            latency_us: micros(latched - submitted),
            events,
        })
    }

    /// Takes every display event available now. A release of a producer's
    /// buffer is passed on to the producer here, so that it may draw into
    /// the buffer again.
    pub fn drain(&mut self) -> Result<Vec<DisplayEvent>, String> {
        let mut events = Vec::new();
        while let Some(event) = self
            .engine
            .poll_event()
            .map_err(|e| format!("events: {}", describe(&e)))?
        {
            if let (DisplayEvent::Released { buffer }, Some(producers)) =
                (event, self.producers.as_mut())
            {
                if producers.owns(buffer) {
                    producers.released(buffer)?;
                }
            }
            events.push(event);
        }
        Ok(events)
    }

    /// Waits until the device has events or `timeout` passes.
    pub fn wait(&self, timeout: Duration) -> Result<(), String> {
        crate::stop::check()?;
        self.engine
            .backend()
            .wait_for_events(Some(timeout))
            .map(|_| ())
            .map_err(|e| os("poll", e))
    }

    /// Collects events until `submission` is presented. Returns when it was
    /// seen and every event seen.
    pub fn wait_presented(
        &mut self,
        submission: SubmissionId,
        timeout: Duration,
    ) -> Result<(Instant, Vec<DisplayEvent>), String> {
        let deadline = Instant::now() + timeout;
        let mut events = Vec::new();
        loop {
            for event in self.drain()? {
                events.push(event);
                match event {
                    DisplayEvent::Presented { submission: s, .. } if s == submission => {
                        return Ok((Instant::now(), events));
                    }
                    DisplayEvent::Dropped { submission: s, .. } if s == submission => {
                        return Err(format!("submission {} was dropped", s.0));
                    }
                    DisplayEvent::DeviceLost => return Err("device lost".into()),
                    _ => {}
                }
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(format!(
                    "submission {} not latched within {} ms",
                    submission.0,
                    timeout.as_millis()
                ));
            }
            self.wait(deadline - now)?;
        }
    }

    /// Keeps the current picture for `seconds`, so that a camera can look
    /// at it. Returns the events seen meanwhile (normally none).
    pub fn hold(&mut self, step: &str, seconds: u64) -> Result<Vec<DisplayEvent>, String> {
        Record::new("hold")
            .field("step", step)
            .field("seconds", seconds)
            .emit();
        let mut events = Vec::new();
        // A stop ends the hold early, not the scenario: the caller's checks
        // were made, and `Outcome` carries the stop with what they found.
        crate::stop::hold_for(
            seconds,
            Duration::from_millis(200),
            &crate::stop::requested,
            |slice| {
                events.extend(self.drain()?);
                self.wait(slice)
            },
        )?;
        events.extend(self.drain()?);
        Ok(events)
    }
}
