//! External producers: buffers supplied by separate processes.
//!
//! With `--producers external` the harness spawns one producer per layer
//! (`drm-harness producer`, the same binary). A producer opens the device
//! node without becoming DRM master, allocates its own dumb buffers, draws
//! them, exports them as PRIME DMA-BUF descriptors and sends them with
//! their metadata over a `SOCK_SEQPACKET` socket (the producer's standard
//! input). The harness imports them with `DrmBackend::import_dmabuf`.
//!
//! Ownership is explicit: a buffer the producer has sent is lent to the
//! harness, and the producer refuses to draw into it until the harness has
//! sent `Released` for it, which the harness does when the engine reports
//! the buffer released (after a later frame has latched).
//!
//! # Wire format
//!
//! One message per packet, little-endian, first byte the message type.
//! Rectangles are `x: i32, y: i32, width: u32, height: u32`.
//!
//! | type | message | fields |
//! |------|---------|--------|
//! | 0x01 | `Alloc` | buffer u32, width u32, height u32, format u32, ops |
//! | 0x02 | `Draw` | buffer u32, ops |
//! | 0x03 | `Released` | buffer u32 |
//! | 0x04 | `Checksum` | buffer u32 |
//! | 0x05 | `Quit` | |
//! | 0x81 | `Buffer` (+ DMA-BUF fd) | buffer u32, width u32, height u32, format u32, modifier u64, offset u32, pitch u32 |
//! | 0x82 | `Drawn` | buffer u32, draw_us u32 |
//! | 0x83 | `Ack` | buffer u32 |
//! | 0x84 | `Sum` | buffer u32, value u64 |
//! | 0x85 | `Failed` | UTF-8 text to the end |
//! | 0x86 | `Bye` | allocs u32, draws u32, releases u32, refused u32 |
//!
//! `ops` is a count (u16) followed by that many steps: 0x01 fill (rect,
//! colour u32), 0x02 border (rect, thickness u32, colour u32), 0x03
//! checker (rect, cell u32, a u32, b u32).

use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use composition_core::{BufferId, BufferInfo, Modifier, PixelFormat, Rect, Size};
use composition_drm::Card;
use drm::buffer::{Buffer as _, DrmFourcc};
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::Device as ControlDevice;
use drm::Device as BasicDevice;

use crate::draw::{self, Op};
use crate::record::Record;
use crate::stats::micros;
use crate::wire;

/// Largest message on the wire.
pub const MAX_MESSAGE: usize = 4096;
/// Most drawing steps in one message.
pub const MAX_OPS: usize = 128;
/// How long the harness waits for a producer's reply.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

/// Problems producers reported when they were stopped (refused requests,
/// unclean exits), over the life of the harness. A scenario fails if this
/// grew while it ran.
pub static PROBLEMS: AtomicU32 = AtomicU32::new(0);

/// A message of the producer protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// Harness → producer: allocate `buffer`, draw `ops`, send it.
    Alloc {
        /// Buffer id, chosen by the harness.
        buffer: u32,
        /// Size in pixels.
        size: Size,
        /// FourCC.
        format: u32,
        /// What to draw.
        ops: Vec<Op>,
    },
    /// Harness → producer: draw into a released buffer, lend it again.
    Draw {
        /// Buffer id.
        buffer: u32,
        /// What to draw.
        ops: Vec<Op>,
    },
    /// Harness → producer: the display no longer reads `buffer`.
    Released {
        /// Buffer id.
        buffer: u32,
    },
    /// Harness → producer: checksum of the visible bytes of `buffer`.
    Checksum {
        /// Buffer id.
        buffer: u32,
    },
    /// Harness → producer: exit.
    Quit,
    /// Producer → harness: the buffer, its DMA-BUF attached.
    Buffer {
        /// Buffer id.
        buffer: u32,
        /// Size in pixels.
        size: Size,
        /// FourCC.
        format: u32,
        /// Format modifier.
        modifier: u64,
        /// Offset of the first byte in the DMA-BUF.
        offset: u32,
        /// Bytes per row.
        pitch: u32,
    },
    /// Producer → harness: drawn, lent again.
    Drawn {
        /// Buffer id.
        buffer: u32,
        /// Time spent drawing, in microseconds.
        draw_us: u32,
    },
    /// Producer → harness: release noted.
    Ack {
        /// Buffer id.
        buffer: u32,
    },
    /// Producer → harness: checksum.
    Sum {
        /// Buffer id.
        buffer: u32,
        /// FNV-1a of the visible bytes.
        value: u64,
    },
    /// Producer → harness: the request failed.
    Failed(String),
    /// Producer → harness: counters, sent before exiting.
    Bye {
        /// Buffers allocated.
        allocs: u32,
        /// Redraws.
        draws: u32,
        /// Releases received.
        releases: u32,
        /// Requests refused (drawing into a lent buffer, double release).
        refused: u32,
    },
}

fn bad(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn rect(&mut self, r: Rect) {
        self.i32(r.x);
        self.i32(r.y);
        self.u32(r.width);
        self.u32(r.height);
    }
    fn ops(&mut self, ops: &[Op]) {
        self.u16(ops.len() as u16);
        for op in ops {
            match *op {
                Op::Fill { rect, colour } => {
                    self.u8(1);
                    self.rect(rect);
                    self.u32(colour);
                }
                Op::Border {
                    rect,
                    thickness,
                    colour,
                } => {
                    self.u8(2);
                    self.rect(rect);
                    self.u32(thickness);
                    self.u32(colour);
                }
                Op::Checker { rect, cell, a, b } => {
                    self.u8(3);
                    self.rect(rect);
                    self.u32(cell);
                    self.u32(a);
                    self.u32(b);
                }
            }
        }
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        if self.0.len() < N {
            return Err(bad("message too short"));
        }
        let (head, rest) = self.0.split_at(N);
        self.0 = rest;
        Ok(head.try_into().expect("split at N"))
    }
    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_le_bytes(self.take()?))
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.take()?))
    }
    fn rect(&mut self) -> io::Result<Rect> {
        Ok(Rect::new(
            self.i32()?,
            self.i32()?,
            self.u32()?,
            self.u32()?,
        ))
    }
    fn size(&mut self) -> io::Result<Size> {
        Ok(Size::new(self.u32()?, self.u32()?))
    }
    fn ops(&mut self) -> io::Result<Vec<Op>> {
        let count = self.u16()? as usize;
        if count > MAX_OPS {
            return Err(bad(format!("{count} drawing steps, at most {MAX_OPS}")));
        }
        let mut ops = Vec::with_capacity(count);
        for _ in 0..count {
            ops.push(match self.u8()? {
                1 => Op::Fill {
                    rect: self.rect()?,
                    colour: self.u32()?,
                },
                2 => Op::Border {
                    rect: self.rect()?,
                    thickness: self.u32()?,
                    colour: self.u32()?,
                },
                3 => Op::Checker {
                    rect: self.rect()?,
                    cell: self.u32()?,
                    a: self.u32()?,
                    b: self.u32()?,
                },
                tag => return Err(bad(format!("unknown drawing step {tag:#04x}"))),
            });
        }
        Ok(ops)
    }
    fn end(&self) -> io::Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(bad(format!("{} bytes after the message", self.0.len())))
        }
    }
}

impl Message {
    /// The message as sent.
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let mut w = Writer(Vec::with_capacity(64));
        match self {
            Message::Alloc {
                buffer,
                size,
                format,
                ops,
            } => {
                w.u8(0x01);
                w.u32(*buffer);
                w.u32(size.width);
                w.u32(size.height);
                w.u32(*format);
                check_ops(ops)?;
                w.ops(ops);
            }
            Message::Draw { buffer, ops } => {
                w.u8(0x02);
                w.u32(*buffer);
                check_ops(ops)?;
                w.ops(ops);
            }
            Message::Released { buffer } => {
                w.u8(0x03);
                w.u32(*buffer);
            }
            Message::Checksum { buffer } => {
                w.u8(0x04);
                w.u32(*buffer);
            }
            Message::Quit => w.u8(0x05),
            Message::Buffer {
                buffer,
                size,
                format,
                modifier,
                offset,
                pitch,
            } => {
                w.u8(0x81);
                w.u32(*buffer);
                w.u32(size.width);
                w.u32(size.height);
                w.u32(*format);
                w.u64(*modifier);
                w.u32(*offset);
                w.u32(*pitch);
            }
            Message::Drawn { buffer, draw_us } => {
                w.u8(0x82);
                w.u32(*buffer);
                w.u32(*draw_us);
            }
            Message::Ack { buffer } => {
                w.u8(0x83);
                w.u32(*buffer);
            }
            Message::Sum { buffer, value } => {
                w.u8(0x84);
                w.u32(*buffer);
                w.u64(*value);
            }
            Message::Failed(text) => {
                w.u8(0x85);
                let mut end = text.len().min(MAX_MESSAGE - 1);
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                w.0.extend_from_slice(&text.as_bytes()[..end]);
            }
            Message::Bye {
                allocs,
                draws,
                releases,
                refused,
            } => {
                w.u8(0x86);
                w.u32(*allocs);
                w.u32(*draws);
                w.u32(*releases);
                w.u32(*refused);
            }
        }
        if w.0.len() > MAX_MESSAGE {
            return Err(bad("message too long"));
        }
        Ok(w.0)
    }

    /// Parses a message.
    pub fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut r = Reader(bytes);
        let message = match r.u8()? {
            0x01 => Message::Alloc {
                buffer: r.u32()?,
                size: r.size()?,
                format: r.u32()?,
                ops: r.ops()?,
            },
            0x02 => Message::Draw {
                buffer: r.u32()?,
                ops: r.ops()?,
            },
            0x03 => Message::Released { buffer: r.u32()? },
            0x04 => Message::Checksum { buffer: r.u32()? },
            0x05 => Message::Quit,
            0x81 => Message::Buffer {
                buffer: r.u32()?,
                size: r.size()?,
                format: r.u32()?,
                modifier: r.u64()?,
                offset: r.u32()?,
                pitch: r.u32()?,
            },
            0x82 => Message::Drawn {
                buffer: r.u32()?,
                draw_us: r.u32()?,
            },
            0x83 => Message::Ack { buffer: r.u32()? },
            0x84 => Message::Sum {
                buffer: r.u32()?,
                value: r.u64()?,
            },
            0x85 => {
                let text = String::from_utf8_lossy(r.0).into_owned();
                r.0 = &[];
                Message::Failed(text)
            }
            0x86 => Message::Bye {
                allocs: r.u32()?,
                draws: r.u32()?,
                releases: r.u32()?,
                refused: r.u32()?,
            },
            tag => return Err(bad(format!("unknown message type {tag:#04x}"))),
        };
        r.end()?;
        Ok(message)
    }
}

fn check_ops(ops: &[Op]) -> io::Result<()> {
    if ops.len() > MAX_OPS {
        return Err(bad(format!(
            "{} drawing steps, at most {MAX_OPS}",
            ops.len()
        )));
    }
    Ok(())
}

/// Sends `message`, with `fd` attached if given.
pub fn send(
    socket: BorrowedFd<'_>,
    message: &Message,
    fd: Option<BorrowedFd<'_>>,
) -> io::Result<()> {
    wire::send(socket, &message.encode()?, fd)
}

/// Receives one message and the descriptors that came with it. `None` is
/// the end of the stream.
pub fn recv(socket: BorrowedFd<'_>) -> io::Result<Option<(Message, Vec<OwnedFd>)>> {
    let mut buf = vec![0u8; MAX_MESSAGE];
    let received = wire::recv(socket, &mut buf)?;
    if received.len == 0 {
        return Ok(None);
    }
    Ok(Some((Message::decode(&buf[..received.len])?, received.fds)))
}

// ---------------------------------------------------------------------------
// The producer process.
// ---------------------------------------------------------------------------

struct Owned {
    dumb: DumbBuffer,
    size: Size,
    /// Lent to the harness: may be on screen, must not be drawn.
    lent: bool,
}

/// What a producer counts.
#[derive(Default)]
struct Counters {
    allocs: u32,
    draws: u32,
    releases: u32,
    refused: u32,
}

/// The producer's side of its buffers.
struct Producer {
    card: Card,
    buffers: HashMap<u32, Owned>,
    counters: Counters,
}

impl Producer {
    fn alloc(
        &mut self,
        buffer: u32,
        size: Size,
        format: u32,
        ops: &[Op],
    ) -> io::Result<(Message, OwnedFd)> {
        if self.buffers.contains_key(&buffer) {
            return Err(bad(format!("buffer {buffer} exists")));
        }
        let fourcc = DrmFourcc::try_from(format).map_err(|_| bad("unknown fourcc"))?;
        if !matches!(
            fourcc,
            DrmFourcc::Xrgb8888 | DrmFourcc::Argb8888 | DrmFourcc::Xbgr8888 | DrmFourcc::Abgr8888
        ) {
            return Err(bad(format!("producer draws 32-bit RGB only, not {fourcc}")));
        }
        let mut dumb = self
            .card
            .create_dumb_buffer((size.width, size.height), fourcc, 32)?;
        let pitch = dumb.pitch();
        let drawn = self.card.map_dumb_buffer(&mut dumb).map(|mut map| {
            draw::apply(map.as_mut(), pitch as usize, ops);
        });
        let flags = (rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::RDWR).bits();
        let exported = drawn.and_then(|()| self.card.buffer_to_prime_fd(dumb.handle(), flags));
        let fd = match exported {
            Ok(fd) => fd,
            Err(error) => {
                let _ = self.card.destroy_dumb_buffer(dumb);
                return Err(error);
            }
        };
        self.buffers.insert(
            buffer,
            Owned {
                dumb,
                size,
                lent: true,
            },
        );
        self.counters.allocs += 1;
        Ok((
            Message::Buffer {
                buffer,
                size,
                format,
                modifier: Modifier::LINEAR.0,
                offset: 0,
                pitch,
            },
            fd,
        ))
    }

    fn draw(&mut self, buffer: u32, ops: &[Op]) -> io::Result<Message> {
        let owned = self
            .buffers
            .get_mut(&buffer)
            .ok_or_else(|| bad(format!("no buffer {buffer}")))?;
        if owned.lent {
            self.counters.refused += 1;
            return Err(bad(format!(
                "buffer {buffer} is lent to the harness: no release seen, not drawing"
            )));
        }
        let start = Instant::now();
        let pitch = owned.dumb.pitch() as usize;
        let mut map = self.card.map_dumb_buffer(&mut owned.dumb)?;
        draw::apply(map.as_mut(), pitch, ops);
        drop(map);
        owned.lent = true;
        self.counters.draws += 1;
        Ok(Message::Drawn {
            buffer,
            draw_us: micros(start.elapsed()).min(u64::from(u32::MAX)) as u32,
        })
    }

    fn released(&mut self, buffer: u32) -> io::Result<Message> {
        let owned = self
            .buffers
            .get_mut(&buffer)
            .ok_or_else(|| bad(format!("no buffer {buffer}")))?;
        if !owned.lent {
            self.counters.refused += 1;
            return Err(bad(format!("buffer {buffer} released twice")));
        }
        owned.lent = false;
        self.counters.releases += 1;
        Ok(Message::Ack { buffer })
    }

    fn checksum(&mut self, buffer: u32) -> io::Result<Message> {
        let owned = self
            .buffers
            .get_mut(&buffer)
            .ok_or_else(|| bad(format!("no buffer {buffer}")))?;
        let pitch = owned.dumb.pitch() as usize;
        let size = owned.size;
        let map = self.card.map_dumb_buffer(&mut owned.dumb)?;
        let value = draw::checksum(map.as_ref(), pitch, size.width, size.height);
        Ok(Message::Sum { buffer, value })
    }
}

/// `drm-harness producer`: serves the harness on standard input.
pub fn run_producer(device: &Path, layer: u32) -> Result<(), String> {
    let socket = std::io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map_err(|e| format!("standard input: {e}"))?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(device)
        .map_err(|e| format!("open {}: {e}", device.display()))?;
    let card = Card::from_fd(OwnedFd::from(file));
    // Opening a device nobody is master of makes the opener master: give it
    // up, a producer only allocates.
    let dropped_master = card.release_master_lock().is_ok();
    Record::new("producer")
        .field("layer", layer)
        .field("pid", std::process::id())
        .field("device", device.display())
        .field(
            "was_master",
            if dropped_master { "yes-dropped" } else { "no" },
        )
        .emit();
    let mut producer = Producer {
        card,
        buffers: HashMap::new(),
        counters: Counters::default(),
    };
    loop {
        let Some((message, _fds)) = recv(socket.as_fd()).map_err(|e| format!("receive: {e}"))?
        else {
            break;
        };
        let (reply, fd) = match message {
            Message::Alloc {
                buffer,
                size,
                format,
                ops,
            } => match producer.alloc(buffer, size, format, &ops) {
                Ok((reply, fd)) => (reply, Some(fd)),
                Err(e) => (Message::Failed(format!("alloc {buffer}: {e}")), None),
            },
            Message::Draw { buffer, ops } => (
                producer
                    .draw(buffer, &ops)
                    .unwrap_or_else(|e| Message::Failed(format!("draw: {e}"))),
                None,
            ),
            Message::Released { buffer } => (
                producer
                    .released(buffer)
                    .unwrap_or_else(|e| Message::Failed(format!("release: {e}"))),
                None,
            ),
            Message::Checksum { buffer } => (
                producer
                    .checksum(buffer)
                    .unwrap_or_else(|e| Message::Failed(format!("checksum: {e}"))),
                None,
            ),
            Message::Quit => break,
            other => (Message::Failed(format!("not a request: {other:?}")), None),
        };
        send(socket.as_fd(), &reply, fd.as_ref().map(|f| f.as_fd()))
            .map_err(|e| format!("send: {e}"))?;
    }
    let c = &producer.counters;
    let _ = send(
        socket.as_fd(),
        &Message::Bye {
            allocs: c.allocs,
            draws: c.draws,
            releases: c.releases,
            refused: c.refused,
        },
        None,
    );
    for (_, owned) in producer.buffers.drain() {
        let _ = producer.card.destroy_dumb_buffer(owned.dumb);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The harness's side.
// ---------------------------------------------------------------------------

/// A buffer received from a producer.
pub struct Supplied {
    /// The DMA-BUF.
    pub fd: OwnedFd,
    /// Offset of the first byte.
    pub offset: u32,
    /// Bytes per row.
    pub pitch: u32,
    /// Metadata as the producer reported it.
    pub info: BufferInfo,
}

struct Process {
    layer: usize,
    child: Child,
    socket: OwnedFd,
}

/// The producer processes of one session, one per layer, started on first
/// use.
pub struct Producers {
    device: PathBuf,
    processes: Vec<Process>,
    owner: HashMap<BufferId, usize>,
    /// Buffers the producers consider lent to the harness.
    lent: HashSet<BufferId>,
    /// Release notices sent.
    releases_sent: u32,
    /// Of those, buffers handed back because they were never shown.
    returned_unshown: u32,
}

impl Producers {
    /// Producers that will open `device`.
    pub fn new(device: PathBuf) -> Self {
        Self {
            device,
            processes: Vec::new(),
            owner: HashMap::new(),
            lent: HashSet::new(),
            releases_sent: 0,
            returned_unshown: 0,
        }
    }

    fn process(&mut self, layer: usize) -> Result<&Process, String> {
        if let Some(index) = self.processes.iter().position(|p| p.layer == layer) {
            return Ok(&self.processes[index]);
        }
        let (ours, theirs) = wire::seqpacket_pair().map_err(|e| format!("socketpair: {e}"))?;
        wire::set_receive_timeout(ours.as_fd(), REPLY_TIMEOUT)
            .map_err(|e| format!("socket timeout: {e}"))?;
        let exe = std::env::current_exe().map_err(|e| format!("own executable: {e}"))?;
        let child = Command::new(exe)
            .arg("producer")
            .arg("--device")
            .arg(&self.device)
            .arg("--layer")
            .arg(layer.to_string())
            .stdin(Stdio::from(theirs))
            .spawn()
            .map_err(|e| format!("spawn producer: {e}"))?;
        Record::new("producer_spawned")
            .field("layer", layer)
            .field("pid", child.id())
            .emit();
        self.processes.push(Process {
            layer,
            child,
            socket: ours,
        });
        Ok(self.processes.last().expect("just pushed"))
    }

    fn ask(&self, index: usize, message: &Message) -> Result<(Message, Vec<OwnedFd>), String> {
        let process = &self.processes[index];
        let socket = process.socket.as_fd();
        send(socket, message, None)
            .map_err(|e| format!("producer {}: send: {e}", process.layer))?;
        match recv(socket) {
            Ok(Some((Message::Failed(text), _))) => {
                Err(format!("producer {}: {text}", process.layer))
            }
            Ok(Some(reply)) => Ok(reply),
            Ok(None) => Err(format!("producer {}: exited", process.layer)),
            Err(e) => Err(format!("producer {}: receive: {e}", process.layer)),
        }
    }

    fn index_of(&self, buffer: BufferId) -> Result<usize, String> {
        let layer = *self
            .owner
            .get(&buffer)
            .ok_or_else(|| format!("buffer {} has no producer", buffer.0))?;
        Ok(self
            .processes
            .iter()
            .position(|p| p.layer == layer)
            .expect("owner has a process"))
    }

    /// Whether a producer supplied `buffer`.
    pub fn owns(&self, buffer: BufferId) -> bool {
        self.owner.contains_key(&buffer)
    }

    /// Has the producer of `layer` allocate and draw `buffer`.
    pub fn alloc(
        &mut self,
        layer: usize,
        buffer: BufferId,
        size: Size,
        format: PixelFormat,
        ops: &[Op],
    ) -> Result<Supplied, String> {
        self.process(layer)?;
        let index = self
            .processes
            .iter()
            .position(|p| p.layer == layer)
            .expect("started");
        let (reply, mut fds) = self.ask(
            index,
            &Message::Alloc {
                buffer: buffer.0,
                size,
                format: format.0,
                ops: ops.to_vec(),
            },
        )?;
        match reply {
            Message::Buffer {
                buffer: b,
                size,
                format,
                modifier,
                offset,
                pitch,
            } if b == buffer.0 && fds.len() == 1 => {
                self.owner.insert(buffer, layer);
                self.lent.insert(buffer);
                let info = BufferInfo {
                    size,
                    format: PixelFormat(format),
                    modifier: Modifier(modifier),
                };
                Record::new("supplied")
                    .field("layer", layer)
                    .field("pid", self.processes[index].child.id())
                    .field("buffer", buffer.0)
                    .field("width", size.width)
                    .field("height", size.height)
                    .field("pitch", pitch)
                    .field("modifier", format!("{modifier:#x}"))
                    .emit();
                Ok(Supplied {
                    fd: fds.remove(0),
                    offset,
                    pitch,
                    info,
                })
            }
            other => Err(format!(
                "producer {layer}: unexpected reply to alloc ({other:?}, {} fds)",
                fds.len()
            )),
        }
    }

    /// Has the producer redraw `buffer`. Call it only once the engine has
    /// accepted `begin_write` for the buffer. A buffer that was drawn but
    /// never shown since (so the engine never reported a release) is
    /// handed back to the producer first, since the display never read it.
    pub fn draw(&mut self, buffer: BufferId, ops: &[Op]) -> Result<u32, String> {
        let index = self.index_of(buffer)?;
        if self.lent.contains(&buffer) {
            self.released(buffer)?;
            self.returned_unshown += 1;
        }
        let drawn = match self.ask(
            index,
            &Message::Draw {
                buffer: buffer.0,
                ops: ops.to_vec(),
            },
        )? {
            (Message::Drawn { buffer: b, draw_us }, _) if b == buffer.0 => draw_us,
            (other, _) => return Err(format!("unexpected reply to draw: {other:?}")),
        };
        self.lent.insert(buffer);
        Ok(drawn)
    }

    /// Tells the producer that `buffer` is no longer read by the display.
    pub fn released(&mut self, buffer: BufferId) -> Result<(), String> {
        let index = self.index_of(buffer)?;
        match self.ask(index, &Message::Released { buffer: buffer.0 })? {
            (Message::Ack { buffer: b }, _) if b == buffer.0 => {
                self.lent.remove(&buffer);
                self.releases_sent += 1;
                Ok(())
            }
            (other, _) => Err(format!("unexpected reply to release: {other:?}")),
        }
    }

    /// Checksum of `buffer`'s visible bytes, computed by its producer.
    pub fn checksum(&mut self, buffer: BufferId) -> Result<u64, String> {
        let index = self.index_of(buffer)?;
        match self.ask(index, &Message::Checksum { buffer: buffer.0 })? {
            (Message::Sum { buffer: b, value }, _) if b == buffer.0 => Ok(value),
            (other, _) => Err(format!("unexpected reply to checksum: {other:?}")),
        }
    }

    /// Stops every producer and prints what each counted. Returns the
    /// number of refused requests (drawing into a lent buffer, double
    /// releases) and of producers that did not end cleanly.
    pub fn finish(&mut self) -> u32 {
        let mut problems = 0;
        for mut process in self.processes.drain(..) {
            let socket = process.socket.as_fd();
            let _ = send(socket, &Message::Quit, None);
            let mut record = Record::new("producer_exit")
                .field("layer", process.layer)
                .field("pid", process.child.id());
            match recv(socket) {
                Ok(Some((
                    Message::Bye {
                        allocs,
                        draws,
                        releases,
                        refused,
                    },
                    _,
                ))) => {
                    problems += refused;
                    record = record
                        .field("allocs", allocs)
                        .field("draws", draws)
                        .field("releases", releases)
                        .field("refused", refused);
                }
                other => {
                    problems += 1;
                    record = record.field("bye", format!("{other:?}"));
                }
            }
            drop(process.socket);
            match process.child.wait() {
                Ok(status) if status.success() => record = record.field("status", "ok"),
                Ok(status) => {
                    problems += 1;
                    record = record.field("status", status);
                }
                Err(e) => {
                    problems += 1;
                    record = record.field("status", e);
                }
            }
            record.emit();
        }
        Record::new("producers")
            .field("releases_passed_on", self.releases_sent)
            .field("returned_unshown", self.returned_unshown)
            .field("problems", problems)
            .emit();
        PROBLEMS.fetch_add(problems, Ordering::Relaxed);
        problems
    }
}

impl Drop for Producers {
    fn drop(&mut self) {
        if !self.processes.is_empty() {
            self.finish();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_messages() -> Vec<Message> {
        let rect = Rect::new(-3, 4, 100, 50);
        let ops = vec![
            Op::Fill {
                rect,
                colour: 0xFF00_2080,
            },
            Op::Border {
                rect,
                thickness: 8,
                colour: 0xFFFF_FFFF,
            },
            Op::Checker {
                rect,
                cell: 64,
                a: 1,
                b: 2,
            },
        ];
        vec![
            Message::Alloc {
                buffer: 7,
                size: Size::new(1280, 800),
                format: PixelFormat::ARGB8888.0,
                ops: ops.clone(),
            },
            Message::Draw { buffer: 7, ops },
            Message::Draw {
                buffer: 8,
                ops: Vec::new(),
            },
            Message::Released { buffer: 7 },
            Message::Checksum { buffer: 7 },
            Message::Quit,
            Message::Buffer {
                buffer: 7,
                size: Size::new(640, 400),
                format: PixelFormat::XRGB8888.0,
                modifier: 0x00ff_ffff_ffff_ffff,
                offset: 0,
                pitch: 2560,
            },
            Message::Drawn {
                buffer: 7,
                draw_us: 1234,
            },
            Message::Ack { buffer: 7 },
            Message::Sum {
                buffer: 7,
                value: 0xcbf2_9ce4_8422_2325,
            },
            Message::Failed("buffer 7 is lent".into()),
            Message::Bye {
                allocs: 6,
                draws: 1194,
                releases: 1194,
                refused: 0,
            },
        ]
    }

    #[test]
    fn every_message_survives_encoding() {
        for message in all_messages() {
            let bytes = message.encode().unwrap();
            assert_eq!(Message::decode(&bytes).unwrap(), message, "{bytes:02x?}");
        }
    }

    #[test]
    fn encoding_is_little_endian_and_fixed() {
        assert_eq!(
            Message::Released {
                buffer: 0x0102_0304
            }
            .encode()
            .unwrap(),
            [0x03, 0x04, 0x03, 0x02, 0x01]
        );
        assert_eq!(Message::Quit.encode().unwrap(), [0x05]);
        let fill = Message::Draw {
            buffer: 1,
            ops: vec![Op::Fill {
                rect: Rect::new(1, 2, 3, 4),
                colour: 0xAABB_CCDD,
            }],
        };
        assert_eq!(
            fill.encode().unwrap(),
            [
                0x02, 1, 0, 0, 0, // Draw, buffer 1
                1, 0, // one step
                1, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, // fill, rect
                0xDD, 0xCC, 0xBB, 0xAA, // colour
            ]
        );
    }

    #[test]
    fn truncated_or_padded_messages_are_refused() {
        for message in all_messages() {
            let bytes = message.encode().unwrap();
            if !matches!(message, Message::Failed(_)) {
                for cut in 0..bytes.len() {
                    assert!(
                        Message::decode(&bytes[..cut]).is_err(),
                        "{message:?} cut at {cut}"
                    );
                }
                let mut padded = bytes.clone();
                padded.push(0);
                assert!(Message::decode(&padded).is_err(), "{message:?} padded");
            }
        }
        assert!(Message::decode(&[0x42]).is_err());
        // Unknown drawing step.
        assert!(Message::decode(&[0x02, 1, 0, 0, 0, 1, 0, 9]).is_err());
    }

    #[test]
    fn too_many_steps_are_refused() {
        let ops = vec![
            Op::Fill {
                rect: Rect::new(0, 0, 1, 1),
                colour: 0
            };
            MAX_OPS + 1
        ];
        assert!(Message::Draw { buffer: 1, ops }.encode().is_err());
        let mut bytes = vec![0x02, 1, 0, 0, 0];
        bytes.extend_from_slice(&((MAX_OPS + 1) as u16).to_le_bytes());
        assert!(Message::decode(&bytes).is_err());
    }

    #[test]
    fn a_buffer_reply_carries_its_descriptor_over_a_socket() {
        let (harness, producer) = wire::seqpacket_pair().unwrap();
        let file = std::fs::File::open("/dev/null").unwrap();
        let reply = Message::Buffer {
            buffer: 3,
            size: Size::new(16, 16),
            format: PixelFormat::XRGB8888.0,
            modifier: 0,
            offset: 0,
            pitch: 64,
        };
        send(producer.as_fd(), &reply, Some(file.as_fd())).unwrap();
        let (got, fds) = recv(harness.as_fd()).unwrap().unwrap();
        assert_eq!(got, reply);
        assert_eq!(fds.len(), 1);
        drop(producer);
        assert!(recv(harness.as_fd()).unwrap().is_none());
    }
}
