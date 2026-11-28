//! Pending probe: flip and latch counts of an overlay commit, and what the
//! kernel does with a second commit sent before the first one's flip event.
//!
//! ```text
//! cargo run -p composition-drm --example pending_probe -- [options] --plane ID \
//!     (--flips N | --replace | --replace-blocking | --cursor-move)
//!
//!   --device PATH      DRM device (default /dev/dri/card0); a bare argument works too
//!   --connector ID     connector to use (default: the first connected one)
//!   --plane ID         the overlay plane to commit (as `props` or `kms_sweep` print it)
//!   --size WxH         size of the overlay buffers (default: half the output, even sizes)
//!   --buffers N        number of dumb buffers, 2 or 3 (default 3; `--replace` needs 3)
//!   --timeout-ms N     how long to wait for one page-flip event (default 1000)
//!   --flips N          mode 1: N nonblocking commits, each waited for
//!   --replace          mode 2: commit A, then B before A's event
//!   --replace-blocking mode 3: as `--replace`, but B is a blocking commit
//!   --cursor-move      mode 4: commit A, then a cursor-plane-only commit before A's event
//! ```
//!
//! Exactly one of `--flips`, `--replace`, `--replace-blocking` and
//! `--cursor-move` is needed.
//!
//! The program opens the device, becomes DRM master (or exits with status 2
//! saying it could not; it never stops other processes) and takes the
//! connector's CRTC with the mode the CRTC runs (no modeset is made then; a
//! CRTC that is off gets the preferred mode and a modeset). The overlay plane
//! gets 2 or 3 dumb XRGB8888 buffers with the eight colour bars of
//! `plane_commit`, each one rotated by one bar so that two buffers never look
//! alike. The primary plane keeps the framebuffer it shows; when it shows
//! none (or the CRTC is off) it gets a fresh full-size bars buffer in a
//! first, waited-for commit. The program does not reuse a kernel-lease
//! descriptor: it opens the device node and takes master like `plane_commit`.
//!
//! # `--flips N`
//!
//! One waited-for commit puts the overlay on screen (buffer 0). Then N
//! commits follow, each `NONBLOCK | PAGE_FLIP_EVENT`, each followed by a wait
//! for its event (at most `--timeout-ms`) before the next one is sent. The
//! overlay shows buffer `i % buffers`. Per flip it prints:
//!
//! ```text
//! FLIP n=1 fb=77 commit=ok seq=1234 ts_us=5678901 vblank_before=1233 vblank_after=1234 submit_to_event_us=16700
//! SUMMARY flips: submitted=10 events=10 missed=0 mean_interval_us=16667 seq_gaps=0
//! ```
//!
//! `seq` and `ts_us` are the event's frame sequence and timestamp (the clock
//! is the one the driver uses, monotonic on current kernels). `vblank_before`
//! and `vblank_after` are the counter read with the legacy `WAIT_VBLANK`
//! ioctl (relative 0) before the commit and after the event, or `n/a` when
//! the driver does not answer it. `submitted` counts commits the kernel
//! accepted, `events` the events that came, `missed` is the difference (a
//! commit with no event within the timeout), `mean_interval_us` is the mean
//! time between consecutive event timestamps and `seq_gaps` the number of
//! skipped sequence numbers between consecutive events. A late event shows up
//! at the next flip's wait, so read `events` as a total.
//!
//! # `--replace`
//!
//! Buffer 0 is put on screen and its event awaited. Then, without waiting:
//!
//! 1. commit A (overlay shows buffer 1), `NONBLOCK | PAGE_FLIP_EVENT`;
//! 2. a `TEST_ONLY | NONBLOCK` commit of B (overlay shows buffer 2);
//! 3. the real commit B, `NONBLOCK | PAGE_FLIP_EVENT`.
//!
//! The program then waits for the events and reads the overlay's `FB_ID`:
//!
//! ```text
//! A commit=ok
//! B_TEST_ONLY commit=EBUSY
//! B commit=EBUSY
//! EVENTS arrived=1 expected=1 a_event=yes b_event=n/a
//! PLANE_FB plane=42 fb=78 shows=A
//! VERDICT replace_pending: refused errno=EBUSY
//! ```
//!
//! The verdict follows the real commit B: `accepted`, or `refused errno=NAME`.
//! Events carry no commit tag here, so they are attributed by order: the
//! first event is A's, a second one is B's. `PLANE_FB` is the `FB_ID` the
//! kernel reports right after the events (or the timeout): the committed
//! state, which the kernel updates when it accepts a commit, so it says which
//! commit the kernel took as current but not what the display scans out.
//! Judge the picture by eye or with a camera.
//!
//! Every real commit line of the modes below also carries `errno=N` (0 when
//! accepted, else the raw errno) and `return_us=N`, the wall-clock time the
//! commit ioctl took to return (a blocking commit that waits shows it). After
//! the `EVENTS` line, a `COUNTS` line gives the flip events per commit:
//!
//! ```text
//! COUNTS a_events=1 b_events=0 c_events=n/a extra_events=0
//! ```
//!
//! Events are attributed by order (first A's, then the second commit's);
//! `b_events` applies to `--replace` and `--replace-blocking`, `c_events` to
//! `--cursor-move`, the other is `n/a`, and `extra_events` counts events
//! beyond the expected ones. After the expected events the program waits a
//! short grace time (at most 100 ms) for extras, so a duplicate shows up.
//!
//! # `--replace-blocking`
//!
//! As `--replace`, but there is no TEST_ONLY step and B is sent without
//! `NONBLOCK` (with `PAGE_FLIP_EVENT`). The call may wait for A to latch;
//! `return_us` of B says how long. The verdict reads
//! `VERDICT replace_pending_blocking: accepted` or `refused errno=NAME`.
//!
//! # `--cursor-move`
//!
//! Needs a cursor plane of the CRTC with `ARGB8888` (else it prints
//! `CURSOR plane=none` and `VERDICT cursor_move_pending: skipped ...`). A
//! 64x64 cursor buffer is shown at the top left in the setup commit. Then A
//! (overlay) is committed as in `--replace` and, before A's event, commit C
//! touches only the cursor plane (moved by 32,32), `NONBLOCK |
//! PAGE_FLIP_EVENT`:
//!
//! ```text
//! A commit=ok fb=77 errno=0 return_us=210
//! C commit=ok plane=31 errno=0 return_us=95
//! VERDICT cursor_move_pending: accepted
//! ```
//!
//! # Exit
//!
//! At the end the program puts the overlay and (if it touched it) the primary
//! back to the values it read at start (a CRTC that was off is switched off
//! again), and prints `RESTORE ok` or the errno. The exit status is 0 after
//! `VERDICT pending_probe: DONE` (whatever the kernel answered) and 2 for a
//! usage error, a device that cannot be used or a missing DRM master
//! (`VERDICT pending_probe: FAIL ...`). A vkms or other virtual result is
//! never a hardware result.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use composition_drm::Card;
use drm::buffer::{Buffer, DrmFourcc};
use drm::control::atomic::AtomicModeReq;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    connector, crtc, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice,
    Event, ModeTypeFlags, PlaneType, ResourceHandle,
};
use drm::{ClientCapability, Device as BasicDevice, VblankWaitFlags, VblankWaitTarget};

const USAGE: &str = "usage: pending_probe [--device PATH] [--connector ID] --plane ID \
[--size WxH] [--buffers 2|3] [--timeout-ms N] \
(--flips N | --replace | --replace-blocking | --cursor-move)";

const ONE_MODE: &str = "use only one of --flips, --replace, --replace-blocking and --cursor-move";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Flips(u32),
    Replace,
    ReplaceBlocking,
    CursorMove,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Options {
    device: String,
    connector: Option<u32>,
    plane: u32,
    size: Option<(u32, u32)>,
    buffers: usize,
    timeout_ms: u64,
    mode: Mode,
}

fn number<T: std::str::FromStr>(text: &str) -> Result<T, String> {
    text.parse().map_err(|_| format!("bad number {text}"))
}

/// `WxH`, both non-zero and even (the bars pattern needs no more, but even
/// sizes keep every driver's alignment rules out of the way).
fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let (w, h) = text.split_once('x').ok_or(format!("bad size {text}"))?;
    let (w, h): (u32, u32) = (number(w)?, number(h)?);
    if w == 0 || h == 0 || w > 1 << 15 || h > 1 << 15 || w % 2 != 0 || h % 2 != 0 {
        return Err(format!("size {text} is empty, odd or too large"));
    }
    Ok((w, h))
}

/// Parses the arguments after the program name. An empty error asks for the
/// usage text alone.
fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
    let mut device = "/dev/dri/card0".to_string();
    let (mut connector, mut plane, mut size, mut mode) = (None, None, None, None);
    let (mut buffers, mut timeout_ms) = (3usize, 1000u64);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--device" => device = value("--device")?,
            "--connector" => connector = Some(number(&value("--connector")?)?),
            "--plane" => plane = Some(number(&value("--plane")?)?),
            "--size" => size = Some(parse_size(&value("--size")?)?),
            "--buffers" => {
                buffers = number(&value("--buffers")?)?;
                if !(2..=3).contains(&buffers) {
                    return Err("--buffers takes 2 or 3".into());
                }
            }
            "--timeout-ms" => {
                timeout_ms = number(&value("--timeout-ms")?)?;
                if timeout_ms == 0 {
                    return Err("--timeout-ms must be above 0".into());
                }
            }
            "--flips" => {
                let n: u32 = number(&value("--flips")?)?;
                if n == 0 {
                    return Err("--flips must be above 0".into());
                }
                if mode.replace(Mode::Flips(n)).is_some() {
                    return Err(ONE_MODE.into());
                }
            }
            "--replace" | "--replace-blocking" | "--cursor-move" => {
                let chosen = match arg.as_str() {
                    "--replace" => Mode::Replace,
                    "--replace-blocking" => Mode::ReplaceBlocking,
                    _ => Mode::CursorMove,
                };
                if mode.replace(chosen).is_some() {
                    return Err(ONE_MODE.into());
                }
            }
            "-h" | "--help" => return Err(String::new()),
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => device = other.to_string(),
        }
    }
    let mode =
        mode.ok_or("one of --flips N, --replace, --replace-blocking and --cursor-move is needed")?;
    if matches!(mode, Mode::Replace | Mode::ReplaceBlocking) && buffers < 3 {
        return Err("--replace and --replace-blocking need --buffers 3".into());
    }
    Ok(Options {
        device,
        connector,
        plane: plane.ok_or("--plane ID is needed")?,
        size,
        buffers,
        timeout_ms,
        mode,
    })
}

/// The overlay size and position: the requested size (or half the output)
/// cut to the output, centred.
fn overlay_geometry(output: (u32, u32), size: Option<(u32, u32)>) -> ((u32, u32), (i32, i32)) {
    let even = |v: u32| (v.max(2)) & !1;
    let (w, h) = size.unwrap_or((even(output.0 / 2), even(output.1 / 2)));
    let (w, h) = (w.min(output.0 & !1).max(2), h.min(output.1 & !1).max(2));
    (
        (w, h),
        (((output.0 - w) / 2) as i32, ((output.1 - h) / 2) as i32),
    )
}

/// The colour bars as (R, G, B).
const BARS: [[u8; 3]; 8] = [
    [255, 255, 255],
    [255, 255, 0],
    [0, 255, 255],
    [0, 255, 0],
    [255, 0, 255],
    [255, 0, 0],
    [0, 0, 255],
    [0, 0, 0],
];

/// The bar of pixel column `x`, rotated by `shift` bars.
fn bar_at(x: u32, width: u32, shift: usize) -> usize {
    ((u64::from(x) * 8 / u64::from(width)) as usize + shift) % 8
}

/// Fills a 32-bit buffer (bytes B, G, R, X) with the colour bars.
fn fill_bars(buf: &mut [u8], pitch: usize, width: u32, height: u32, shift: usize) {
    for row in 0..height as usize {
        for x in 0..width {
            let [r, g, b] = BARS[bar_at(x, width, shift)];
            let at = row * pitch + x as usize * 4;
            buf[at..at + 4].copy_from_slice(&[b, g, r, 0xff]);
        }
    }
}

/// One check's outcome: `Ok` if accepted, the errno otherwise.
type Check = Result<(), i32>;

fn errno_name(errno: i32) -> String {
    let name = match errno {
        1 => "EPERM",
        2 => "ENOENT",
        4 => "EINTR",
        5 => "EIO",
        7 => "E2BIG",
        11 => "EAGAIN",
        12 => "ENOMEM",
        13 => "EACCES",
        14 => "EFAULT",
        16 => "EBUSY",
        19 => "ENODEV",
        22 => "EINVAL",
        28 => "ENOSPC",
        34 => "ERANGE",
        35 => "EDEADLK",
        95 => "EOPNOTSUPP",
        _ => return format!("errno{errno}"),
    };
    name.into()
}

fn check_text(check: Check) -> String {
    match check {
        Ok(()) => "ok".into(),
        Err(errno) => errno_name(errno),
    }
}

/// Events seen so far in `--flips` mode, as (sequence, timestamp in us).
#[derive(Debug, Default)]
struct FlipStats {
    submitted: u32,
    events: Vec<(u32, u64)>,
}

impl FlipStats {
    fn missed(&self) -> u32 {
        self.submitted.saturating_sub(self.events.len() as u32)
    }

    /// Mean time between consecutive event timestamps, if there are two.
    fn mean_interval_us(&self) -> Option<u64> {
        let n = self.events.len() as u64;
        if n < 2 {
            return None;
        }
        let first = self.events[0].1;
        let last = self.events[self.events.len() - 1].1;
        Some(last.saturating_sub(first) / (n - 1))
    }

    /// Sequence numbers skipped between consecutive events.
    fn seq_gaps(&self) -> u64 {
        self.events
            .windows(2)
            .map(|w| u64::from(w[1].0.wrapping_sub(w[0].0)).saturating_sub(1))
            .filter(|gap| *gap < u64::from(u32::MAX / 2))
            .sum()
    }

    fn summary_line(&self) -> String {
        let mean = self
            .mean_interval_us()
            .map_or("n/a".to_string(), |v| v.to_string());
        format!(
            "SUMMARY flips: submitted={} events={} missed={} mean_interval_us={mean} seq_gaps={}",
            self.submitted,
            self.events.len(),
            self.missed(),
            self.seq_gaps()
        )
    }
}

/// A counter that may be missing, as text.
fn opt_text<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or("n/a".to_string(), |v| v.to_string())
}

/// What the events of a `--replace` run say. Events carry no tag, so they are
/// attributed by order: the first is A's, a second one is B's.
#[derive(Debug, PartialEq, Eq)]
struct ReplaceEvents {
    arrived: usize,
    expected: usize,
    a_event: bool,
    /// `None` when B was refused: no event was expected.
    b_event: Option<bool>,
}

fn replace_events(a: Check, b: Check, arrived: usize) -> ReplaceEvents {
    let expected = usize::from(a.is_ok()) + usize::from(a.is_ok() && b.is_ok());
    ReplaceEvents {
        arrived,
        expected,
        a_event: a.is_ok() && arrived >= 1,
        b_event: (a.is_ok() && b.is_ok()).then_some(arrived >= 2),
    }
}

fn events_line(e: &ReplaceEvents) -> String {
    let yes_no = |v: bool| if v { "yes" } else { "no" };
    format!(
        "EVENTS arrived={} expected={} a_event={} b_event={}",
        e.arrived,
        e.expected,
        yes_no(e.a_event),
        e.b_event.map_or("n/a", yes_no)
    )
}

/// The final verdict follows the real commit B; a refused A makes the run
/// say nothing about replacing.
#[cfg(test)]
fn verdict_line(a: Check, b: Check) -> String {
    case_verdict("replace_pending", a, b)
}

/// The verdict of a case: `label` names the question.
fn case_verdict(label: &str, a: Check, second: Check) -> String {
    match (a, second) {
        (Err(errno), _) => format!(
            "{label}: unknown (A was refused errno={})",
            errno_name(errno)
        ),
        (Ok(()), Ok(())) => format!("{label}: accepted"),
        (Ok(()), Err(errno)) => format!("{label}: refused errno={}", errno_name(errno)),
    }
}

/// `errno=N`: 0 when accepted, else the raw errno.
fn errno_value(check: Check) -> i32 {
    check.err().unwrap_or(0)
}

/// The keys every timed commit line ends with.
fn timing_keys(check: Check, return_us: u128) -> String {
    format!("errno={} return_us={return_us}", errno_value(check))
}

/// Flip events per commit of a pending case, attributed by order.
#[derive(Debug, PartialEq, Eq)]
struct EventCounts {
    a: usize,
    /// The second commit's events; `None` when it was refused.
    second: Option<usize>,
    extra: usize,
}

fn event_counts(a: Check, second: Check, arrived: usize) -> EventCounts {
    let a_ok = a.is_ok();
    let second_ok = a_ok && second.is_ok();
    let a_events = usize::from(a_ok && arrived >= 1);
    let second_events = usize::from(second_ok && arrived >= 2);
    EventCounts {
        a: a_events,
        second: second_ok.then_some(second_events),
        extra: arrived.saturating_sub(a_events + second_events),
    }
}

/// `cursor` picks `c_events` for the second commit, else `b_events`; the
/// other key is `n/a`.
fn counts_line(c: &EventCounts, cursor: bool) -> String {
    let second = opt_text(c.second);
    let na = "n/a".to_string();
    let (b, cur) = if cursor { (na, second) } else { (second, na) };
    format!(
        "COUNTS a_events={} b_events={b} c_events={cur} extra_events={}",
        c.a, c.extra
    )
}

/// Which commit's framebuffer the plane shows, by `FB_ID` value.
fn which_fb(value: u64, setup: u64, a: u64, b: u64) -> &'static str {
    if value == 0 {
        "none"
    } else if value == a {
        "A"
    } else if value == b {
        "B"
    } else if value == setup {
        "setup"
    } else {
        "other"
    }
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

/// A property of an object: handle, value at the time of the read and
/// whether userspace may set it.
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

fn prop(props: &Props, name: &str) -> io::Result<Prop> {
    props
        .get(name)
        .copied()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no property {name}")))
}

fn fail(what: &str) -> impl Fn(io::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

fn errno_of(error: &io::Error) -> i32 {
    error.raw_os_error().unwrap_or(-1)
}

/// A plane the program may touch.
struct PlaneInfo {
    handle: plane::Handle,
    kind: PlaneType,
    props: Props,
    formats: Vec<u32>,
}

impl PlaneInfo {
    fn id(&self) -> u32 {
        u32::from(self.handle)
    }
}

/// A dumb buffer and the framebuffer made from it.
struct Fb {
    dumb: DumbBuffer,
    fb: framebuffer::Handle,
}

/// An XRGB8888 framebuffer of `size` with the bars rotated by `shift`.
fn make_fb(card: &Card, size: (u32, u32), shift: usize) -> Result<Fb, String> {
    make_fb_format(card, size, shift, DrmFourcc::Xrgb8888)
}

/// A 32-bit framebuffer of `format` (XRGB8888 or ARGB8888).
fn make_fb_format(
    card: &Card,
    size: (u32, u32),
    shift: usize,
    format: DrmFourcc,
) -> Result<Fb, String> {
    let mut dumb = card
        .create_dumb_buffer(size, format, 32)
        .map_err(|e| format!("dumb buffer: {}", errno_name(errno_of(&e))))?;
    let pitch = dumb.pitch() as usize;
    let filled = card
        .map_dumb_buffer(&mut dumb)
        .map(|mut mapping| fill_bars(mapping.as_mut(), pitch, size.0, size.1, shift))
        .map_err(fail("map"));
    if let Err(message) = filled {
        let _ = card.destroy_dumb_buffer(dumb);
        return Err(message);
    }
    let depth = if format == DrmFourcc::Argb8888 {
        32
    } else {
        24
    };
    match card.add_framebuffer(&dumb, depth, 32) {
        Ok(fb) => Ok(Fb { dumb, fb }),
        Err(error) => {
            let _ = card.destroy_dumb_buffer(dumb);
            Err(format!("ADDFB: {}", errno_name(errno_of(&error))))
        }
    }
}

fn release(card: &Card, fb: Fb) {
    let _ = card.destroy_framebuffer(fb.fb);
    let _ = card.destroy_dumb_buffer(fb.dumb);
}

/// The cursor buffer size, and where `--cursor-move` moves it to.
const CURSOR_SIZE: (u32, u32) = (64, 64);
const CURSOR_MOVE: (i32, i32) = (32, 32);
/// How long to wait for events beyond the expected ones.
const EXTRA_GRACE: Duration = Duration::from_millis(100);

const PLACEMENT: [&str; 10] = [
    "FB_ID", "CRTC_ID", "SRC_X", "SRC_Y", "SRC_W", "SRC_H", "CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H",
];

struct Ctx {
    card: Card,
    crtc: crtc::Handle,
    /// Index of the CRTC in the resource list, the `WAIT_VBLANK` pipe.
    pipe: u32,
    connector: connector::Handle,
    connector_crtc_id: property::Handle,
    crtc_mode_id: property::Handle,
    crtc_active: Prop,
    mode_blob: u64,
    output: (u32, u32),
    overlay: PlaneInfo,
    primary: PlaneInfo,
    /// A cursor plane of the CRTC that takes ARGB8888, if there is one.
    cursor: Option<PlaneInfo>,
}

impl Ctx {
    fn modeset_request(&self, req: &mut AtomicModeReq) {
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
        req.add_property(
            self.crtc,
            self.crtc_active.handle,
            property::Value::Boolean(true),
        );
    }

    /// Places `fb` on `plane`: the whole buffer of `src` size at `dst`.
    fn place(
        &self,
        req: &mut AtomicModeReq,
        plane: &PlaneInfo,
        fb: framebuffer::Handle,
        src: (u32, u32),
        dst: (i32, i32),
    ) -> io::Result<()> {
        let p = |name: &str| prop(&plane.props, name).map(|p| p.handle);
        let h = plane.handle;
        req.add_property(h, p("FB_ID")?, property::Value::Framebuffer(Some(fb)));
        req.add_property(h, p("CRTC_ID")?, property::Value::CRTC(Some(self.crtc)));
        let source = [0, 0, u64::from(src.0) << 16, u64::from(src.1) << 16];
        for (name, v) in ["SRC_X", "SRC_Y", "SRC_W", "SRC_H"].into_iter().zip(source) {
            req.add_property(h, p(name)?, property::Value::UnsignedRange(v));
        }
        req.add_property(h, p("CRTC_X")?, property::Value::SignedRange(dst.0.into()));
        req.add_property(h, p("CRTC_Y")?, property::Value::SignedRange(dst.1.into()));
        req.add_property(
            h,
            p("CRTC_W")?,
            property::Value::UnsignedRange(src.0.into()),
        );
        req.add_property(
            h,
            p("CRTC_H")?,
            property::Value::UnsignedRange(src.1.into()),
        );
        Ok(())
    }

    fn commit(&self, flags: AtomicCommitFlags, req: AtomicModeReq) -> Check {
        self.card
            .atomic_commit(flags, req)
            .map_err(|e| errno_of(&e))
    }

    /// A commit and the microseconds its ioctl took to return.
    fn timed_commit(&self, flags: AtomicCommitFlags, req: AtomicModeReq) -> (Check, u128) {
        let start = Instant::now();
        let check = self.commit(flags, req);
        (check, start.elapsed().as_micros())
    }

    /// The vblank counter of the CRTC, if the driver answers `WAIT_VBLANK`.
    fn vblank(&self) -> Option<u32> {
        self.card
            .wait_vblank(
                VblankWaitTarget::Relative(0),
                VblankWaitFlags::empty(),
                self.pipe,
                0,
            )
            .ok()
            .map(|reply| reply.frame())
    }

    /// Page-flip events of the CRTC that arrive before `deadline`, stopping
    /// early once `want` have arrived. Each is (sequence, timestamp in us).
    fn wait_events(&self, want: usize, deadline: Instant) -> Result<Vec<(u32, u64)>, String> {
        use rustix::event::{poll, PollFd, PollFlags, Timespec};
        let mut seen = Vec::new();
        while seen.len() < want {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
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
                        seen.push((flip.frame, flip.duration.as_micros() as u64));
                    }
                }
            }
        }
        Ok(seen)
    }
}

/// Raw values are passed through as they were read.
fn raw(value: u64) -> property::Value<'static> {
    property::Value::Unknown(value)
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
            "DRIVER {} version {}.{}.{}",
            info.name().to_string_lossy(),
            info.version.0,
            info.version.1,
            info.version.2
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
    let pipe = resources
        .crtcs()
        .iter()
        .position(|c| *c == crtc)
        .ok_or("CRTC is not in the resource list")? as u32;
    let crtc_props = named_properties(&card, crtc).map_err(fail("crtc props"))?;
    let crtc_active = prop(&crtc_props, "ACTIVE").map_err(fail("crtc"))?;
    let was_active = crtc_active.value != 0;
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
    println!(
        "MODE {}x{}@{}Hz connector {} crtc {}{}",
        output.0,
        output.1,
        mode.vrefresh(),
        u32::from(connector),
        u32::from(crtc),
        if running.is_some() {
            " (running mode, no modeset)"
        } else {
            ""
        }
    );
    let connector_props = named_properties(&card, connector).map_err(fail("connector props"))?;

    let mut overlay = None;
    let mut primary = None;
    let mut cursor: Option<PlaneInfo> = None;
    for handle in card.plane_handles().map_err(fail("planes"))? {
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
        let plane = PlaneInfo {
            handle,
            kind,
            props,
            formats: pinfo.formats().to_vec(),
        };
        if plane.id() == options.plane {
            overlay = Some(plane);
        } else if kind == PlaneType::Primary
            && plane.props.contains_key("FB_ID")
            && primary
                .as_ref()
                .is_none_or(|p: &PlaneInfo| plane.id() < p.id())
        {
            primary = Some(plane);
        } else if kind == PlaneType::Cursor
            && PLACEMENT.iter().all(|n| plane.props.contains_key(*n))
            && plane.formats.contains(&(DrmFourcc::Argb8888 as u32))
            && cursor.as_ref().is_none_or(|c| plane.id() < c.id())
        {
            cursor = Some(plane);
        }
    }
    let overlay = overlay.ok_or(format!(
        "plane {} is not a plane of the CRTC",
        options.plane
    ))?;
    if overlay.kind != PlaneType::Overlay {
        return Err(format!(
            "plane {} is {:?}, not an overlay",
            overlay.id(),
            overlay.kind
        ));
    }
    if !overlay.formats.contains(&(DrmFourcc::Xrgb8888 as u32)) {
        return Err(format!("plane {} does not list XR24", overlay.id()));
    }
    for name in PLACEMENT {
        prop(&overlay.props, name).map_err(|_| format!("plane {} has no {name}", options.plane))?;
    }
    let primary = primary.ok_or("no primary plane can drive the CRTC")?;
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
            pipe,
            connector,
            connector_crtc_id: prop(&connector_props, "CRTC_ID")
                .map_err(fail("connector"))?
                .handle,
            crtc_mode_id: prop(&crtc_props, "MODE_ID").map_err(fail("crtc"))?.handle,
            crtc_active,
            mode_blob,
            output,
            overlay,
            primary,
            cursor,
        },
        was_active,
    ))
}

/// What to take back at the end: (plane, property, raw value).
type Saved = Vec<(plane::Handle, property::Handle, u64)>;

fn save(plane: &PlaneInfo, saved: &mut Saved) {
    for name in PLACEMENT {
        if let Some(p) = plane.props.get(name).filter(|p| p.mutable) {
            saved.push((plane.handle, p.handle, p.value));
        }
    }
}

/// Takes the program's changes back. A CRTC that was off is switched off
/// again.
fn restore(ctx: &Ctx, saved: &Saved, was_active: bool) -> Check {
    let mut req = AtomicModeReq::new();
    for (plane, handle, value) in saved {
        req.add_property(*plane, *handle, raw(*value));
    }
    let mut flags = AtomicCommitFlags::empty();
    if !was_active {
        req.add_property(
            ctx.crtc,
            ctx.crtc_active.handle,
            property::Value::Boolean(false),
        );
        flags = AtomicCommitFlags::ALLOW_MODESET;
    }
    ctx.commit(flags, req)
}

fn overlay_request(
    ctx: &Ctx,
    fb: &Fb,
    src: (u32, u32),
    dst: (i32, i32),
) -> io::Result<AtomicModeReq> {
    let mut req = AtomicModeReq::new();
    ctx.place(&mut req, &ctx.overlay, fb.fb, src, dst)?;
    Ok(req)
}

fn overlay_fb_id(ctx: &Ctx) -> Result<u64, String> {
    let props = named_properties(&ctx.card, ctx.overlay.handle).map_err(fail("plane props"))?;
    Ok(prop(&props, "FB_ID").map_err(fail("plane"))?.value)
}

fn run(options: &Options) -> Result<(), String> {
    let (ctx, was_active) = open(options)?;
    let mut buffers: Vec<Fb> = Vec::new();
    let mut saved = Saved::new();
    let mut touched = false;
    let result = session(
        &ctx,
        options,
        was_active,
        &mut buffers,
        &mut saved,
        &mut touched,
    );
    if touched {
        // The overlay and the primary go back before the buffers are freed.
        println!("RESTORE {}", check_text(restore(&ctx, &saved, was_active)));
    }
    for fb in buffers {
        release(&ctx.card, fb);
    }
    let _ = ctx.card.destroy_property_blob(ctx.mode_blob);
    result
}

fn session(
    ctx: &Ctx,
    options: &Options,
    was_active: bool,
    buffers: &mut Vec<Fb>,
    saved: &mut Saved,
    touched: &mut bool,
) -> Result<(), String> {
    let (size, dst) = overlay_geometry(ctx.output, options.size);
    let primary_fb = prop(&ctx.primary.props, "FB_ID").map_or(0, |p| p.value);
    let fresh_primary = !was_active || primary_fb == 0;
    println!(
        "START crtc_active={} overlay={} primary={} primary_fb={primary_fb} fresh_primary={}",
        u8::from(was_active),
        ctx.overlay.id(),
        ctx.primary.id(),
        u8::from(fresh_primary)
    );
    println!(
        "OVERLAY size={}x{} dst={},{} buffers={}",
        size.0, size.1, dst.0, dst.1, options.buffers
    );
    let timeout = Duration::from_millis(options.timeout_ms);
    let cursor = if options.mode == Mode::CursorMove {
        match &ctx.cursor {
            Some(cursor) => {
                println!("CURSOR plane={} size=64x64", cursor.id());
                Some(cursor)
            }
            None => {
                println!("CURSOR plane=none");
                println!("VERDICT cursor_move_pending: skipped (no cursor plane with ARGB8888)");
                return Ok(());
            }
        }
    } else {
        None
    };
    save(&ctx.overlay, saved);
    if fresh_primary {
        save(&ctx.primary, saved);
    }
    if let Some(cursor) = cursor {
        save(cursor, saved);
    }
    *touched = true;

    // Buffers: the overlay's, then (if needed) the primary's.
    let mut fbs = Vec::new();
    for i in 0..options.buffers {
        let fb = make_fb(&ctx.card, size, i).map_err(|e| format!("overlay buffer {i}: {e}"))?;
        fbs.push(fb.fb);
        buffers.push(fb);
    }
    let setup_fb = buffers[0].fb;

    // Setup: the overlay shows buffer 0 and the event is awaited, so the
    // measured commits start from a latched state.
    let mut req = AtomicModeReq::new();
    let mut flags = AtomicCommitFlags::PAGE_FLIP_EVENT;
    if fresh_primary {
        let fb = make_fb(&ctx.card, ctx.output, 0).map_err(|e| format!("primary buffer: {e}"))?;
        ctx.place(&mut req, &ctx.primary, fb.fb, ctx.output, (0, 0))
            .map_err(fail("place"))?;
        buffers.push(fb);
    }
    if !was_active {
        ctx.modeset_request(&mut req);
        flags |= AtomicCommitFlags::ALLOW_MODESET;
    }
    let mut cursor_fb = None;
    if let Some(cursor) = cursor {
        let fb = make_fb_format(&ctx.card, CURSOR_SIZE, 0, DrmFourcc::Argb8888)
            .map_err(|e| format!("cursor buffer: {e}"))?;
        ctx.place(&mut req, cursor, fb.fb, CURSOR_SIZE, (0, 0))
            .map_err(fail("place"))?;
        cursor_fb = Some(fb.fb);
        buffers.push(fb);
    }
    ctx.place(&mut req, &ctx.overlay, setup_fb, size, dst)
        .map_err(fail("place"))?;
    let setup = ctx.commit(flags, req);
    println!("SETUP commit={}", check_text(setup));
    setup.map_err(|e| format!("setup commit {}", errno_name(e)))?;
    let events = ctx.wait_events(1, Instant::now() + timeout)?;
    println!("SETUP events={}", events.len());

    let nonblock = AtomicCommitFlags::NONBLOCK | AtomicCommitFlags::PAGE_FLIP_EVENT;
    match options.mode {
        Mode::Flips(n) => {
            let mut stats = FlipStats::default();
            for i in 0..n {
                let fb = &buffers[(i as usize + 1) % options.buffers];
                let req = overlay_request(ctx, fb, size, dst).map_err(fail("place"))?;
                let before = ctx.vblank();
                let sent = Instant::now();
                let (commit, commit_us) = ctx.timed_commit(nonblock, req);
                if commit.is_ok() {
                    stats.submitted += 1;
                }
                let events = ctx.wait_events(1, sent + timeout)?;
                let waited = sent.elapsed().as_micros();
                let after = ctx.vblank();
                let first = events.first().copied();
                stats.events.extend(events);
                println!(
                    "FLIP n={} fb={} commit={} seq={} ts_us={} vblank_before={} vblank_after={} \
                     submit_to_event_us={} {}",
                    i + 1,
                    u32::from(fb.fb),
                    check_text(commit),
                    opt_text(first.map(|e| e.0)),
                    opt_text(first.map(|e| e.1)),
                    opt_text(before),
                    opt_text(after),
                    if first.is_some() {
                        waited.to_string()
                    } else {
                        "n/a".into()
                    },
                    timing_keys(commit, commit_us)
                );
            }
            // A late event of the last flip still counts.
            let late = ctx.wait_events(
                stats.submitted as usize - stats.events.len().min(stats.submitted as usize),
                Instant::now() + timeout,
            )?;
            stats.events.extend(late);
            println!("{}", stats.summary_line());
        }
        Mode::Replace | Mode::ReplaceBlocking | Mode::CursorMove => {
            let mode = options.mode;
            let (fb_a, fb_b) = (&buffers[1], &buffers[2.min(buffers.len() - 1)]);
            let before = ctx.vblank();
            let (a, a_us) = ctx.timed_commit(
                nonblock,
                overlay_request(ctx, fb_a, size, dst).map_err(fail("place"))?,
            );
            // The second commit: B (nonblocking or blocking) or C (cursor).
            let (b, b_us, b_test) = match mode {
                Mode::CursorMove => {
                    let (cursor, fb) = (cursor.ok_or("no cursor")?, cursor_fb.ok_or("no cursor")?);
                    let mut req = AtomicModeReq::new();
                    ctx.place(&mut req, cursor, fb, CURSOR_SIZE, CURSOR_MOVE)
                        .map_err(fail("place"))?;
                    let (c, us) = ctx.timed_commit(nonblock, req);
                    (c, us, None)
                }
                Mode::ReplaceBlocking => {
                    let req = overlay_request(ctx, fb_b, size, dst).map_err(fail("place"))?;
                    let (b, us) = ctx.timed_commit(AtomicCommitFlags::PAGE_FLIP_EVENT, req);
                    (b, us, None)
                }
                _ => {
                    let req = overlay_request(ctx, fb_b, size, dst).map_err(fail("place"))?;
                    let test = ctx.timed_commit(
                        AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::NONBLOCK,
                        req,
                    );
                    let req = overlay_request(ctx, fb_b, size, dst).map_err(fail("place"))?;
                    let (b, us) = ctx.timed_commit(nonblock, req);
                    (b, us, Some(test))
                }
            };
            let cursor_case = mode == Mode::CursorMove;
            println!(
                "A commit={} fb={} {}",
                check_text(a),
                u32::from(fb_a.fb),
                timing_keys(a, a_us)
            );
            if cursor_case {
                println!(
                    "C commit={} plane={} {}",
                    check_text(b),
                    cursor.map_or(0, PlaneInfo::id),
                    timing_keys(b, b_us)
                );
            } else {
                if let Some((test, us)) = b_test {
                    println!(
                        "B_TEST_ONLY commit={} fb={} {}",
                        check_text(test),
                        u32::from(fb_b.fb),
                        timing_keys(test, us)
                    );
                }
                println!(
                    "B commit={} fb={} {}",
                    check_text(b),
                    u32::from(fb_b.fb),
                    timing_keys(b, b_us)
                );
            }
            let expected = usize::from(a.is_ok()) + usize::from(a.is_ok() && b.is_ok());
            let mut events = ctx.wait_events(expected, Instant::now() + timeout)?;
            // A short grace shows an event that came more than once.
            let grace = timeout.min(EXTRA_GRACE);
            events.extend(ctx.wait_events(usize::MAX, Instant::now() + grace)?);
            let after = ctx.vblank();
            for (i, (seq, ts)) in events.iter().enumerate() {
                println!("EVENT n={} seq={seq} ts_us={ts}", i + 1);
            }
            println!(
                "VBLANK before={} after={}",
                opt_text(before),
                opt_text(after)
            );
            println!("{}", events_line(&replace_events(a, b, events.len())));
            println!(
                "{}",
                counts_line(&event_counts(a, b, events.len()), cursor_case)
            );
            let shown = overlay_fb_id(ctx)?;
            println!(
                "PLANE_FB plane={} fb={shown} shows={}",
                ctx.overlay.id(),
                which_fb(
                    shown,
                    u64::from(u32::from(setup_fb)),
                    u64::from(u32::from(fb_a.fb)),
                    u64::from(u32::from(fb_b.fb))
                )
            );
            let label = match mode {
                Mode::ReplaceBlocking => "replace_pending_blocking",
                Mode::CursorMove => "cursor_move_pending",
                _ => "replace_pending",
            };
            println!("VERDICT {}", case_verdict(label, a, b));
        }
    }
    Ok(())
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
        Ok(()) => {
            println!("VERDICT pending_probe: DONE");
            ExitCode::SUCCESS
        }
        Err(message) => {
            println!("VERDICT pending_probe: FAIL {message}");
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
    fn parses_flips_with_defaults() {
        let o = args("--plane 42 --flips 10").unwrap();
        assert_eq!(o.device, "/dev/dri/card0");
        assert_eq!(o.plane, 42);
        assert_eq!(o.mode, Mode::Flips(10));
        assert_eq!(
            (o.buffers, o.timeout_ms, o.connector, o.size),
            (3, 1000, None, None)
        );
    }

    #[test]
    fn parses_replace_and_every_option() {
        let o = args(
            "/dev/dri/card1 --connector 7 --plane 9 --size 640x360 --buffers 3 \
             --timeout-ms 250 --replace",
        )
        .unwrap();
        assert_eq!(o.device, "/dev/dri/card1");
        assert_eq!(o.connector, Some(7));
        assert_eq!(o.size, Some((640, 360)));
        assert_eq!(o.timeout_ms, 250);
        assert_eq!(o.mode, Mode::Replace);
    }

    #[test]
    fn refuses_bad_arguments() {
        assert!(args("--flips 3").is_err(), "plane is needed");
        assert!(args("--plane 1").is_err(), "a mode is needed");
        assert!(args("--plane 1 --flips 3 --replace").is_err());
        assert!(args("--plane 1 --replace --flips 3").is_err());
        assert!(args("--plane 1 --flips 0").is_err());
        assert!(args("--plane 1 --flips x").is_err());
        assert!(args("--plane 1 --flips 2 --buffers 4").is_err());
        assert!(args("--plane 1 --flips 2 --buffers 1").is_err());
        assert!(args("--plane 1 --replace --buffers 2").is_err());
        assert!(args("--plane 1 --flips 2 --size 641x360").is_err());
        assert!(args("--plane 1 --flips 2 --timeout-ms 0").is_err());
        assert!(args("--plane 1 --flips 2 --bogus").is_err());
        assert!(args("--plane").is_err());
        assert_eq!(args("--help"), Err(String::new()));
    }

    #[test]
    fn two_buffers_are_fine_for_flips() {
        assert_eq!(args("--plane 1 --flips 2 --buffers 2").unwrap().buffers, 2);
    }

    #[test]
    fn geometry_defaults_to_half_and_centres() {
        assert_eq!(
            overlay_geometry((1920, 1080), None),
            ((960, 540), (480, 270))
        );
        assert_eq!(
            overlay_geometry((1920, 1080), Some((400, 200))),
            ((400, 200), (760, 440))
        );
        // Cut to the output.
        assert_eq!(
            overlay_geometry((640, 480), Some((4000, 4000))),
            ((640, 480), (0, 0))
        );
    }

    #[test]
    fn bars_rotate_with_the_shift() {
        assert_eq!(bar_at(0, 80, 0), 0);
        assert_eq!(bar_at(79, 80, 0), 7);
        assert_eq!(bar_at(0, 80, 1), 1);
        assert_eq!(bar_at(79, 80, 1), 0);
        let mut a = vec![0u8; 16 * 4];
        let mut b = vec![0u8; 16 * 4];
        fill_bars(&mut a, 16 * 4, 16, 1, 0);
        fill_bars(&mut b, 16 * 4, 16, 1, 1);
        assert_ne!(a, b);
        // First pixel of shift 0 is white (B, G, R, X).
        assert_eq!(a[..4], [255, 255, 255, 255]);
    }

    fn stats(submitted: u32, events: &[(u32, u64)]) -> FlipStats {
        FlipStats {
            submitted,
            events: events.to_vec(),
        }
    }

    #[test]
    fn summary_counts_and_mean_interval() {
        let s = stats(3, &[(100, 1_000_000), (101, 1_016_667), (102, 1_033_333)]);
        assert_eq!(s.missed(), 0);
        assert_eq!(s.mean_interval_us(), Some(16_666));
        assert_eq!(s.seq_gaps(), 0);
        assert_eq!(
            s.summary_line(),
            "SUMMARY flips: submitted=3 events=3 missed=0 mean_interval_us=16666 seq_gaps=0"
        );
    }

    #[test]
    fn summary_with_missed_events_and_gaps() {
        let s = stats(5, &[(10, 0), (12, 33_000), (13, 50_000)]);
        assert_eq!(s.missed(), 2);
        assert_eq!(s.seq_gaps(), 1);
        assert!(s
            .summary_line()
            .starts_with("SUMMARY flips: submitted=5 events=3 missed=2 mean_interval_us=25000"));
    }

    #[test]
    fn summary_without_two_events_has_no_mean() {
        assert_eq!(stats(1, &[]).mean_interval_us(), None);
        assert_eq!(stats(1, &[(5, 9)]).mean_interval_us(), None);
        assert!(stats(1, &[])
            .summary_line()
            .ends_with("missed=1 mean_interval_us=n/a seq_gaps=0"));
        // More events than commits never underflows.
        assert_eq!(stats(0, &[(1, 1)]).missed(), 0);
    }

    #[test]
    fn sequence_wraparound_is_not_a_gap() {
        let s = stats(2, &[(u32::MAX, 0), (0, 16_000)]);
        assert_eq!(s.seq_gaps(), 0);
    }

    #[test]
    fn verdict_follows_the_second_commit() {
        assert_eq!(verdict_line(Ok(()), Ok(())), "replace_pending: accepted");
        assert_eq!(
            verdict_line(Ok(()), Err(16)),
            "replace_pending: refused errno=EBUSY"
        );
        assert_eq!(
            verdict_line(Ok(()), Err(999)),
            "replace_pending: refused errno=errno999"
        );
        assert!(verdict_line(Err(22), Ok(())).starts_with("replace_pending: unknown"));
    }

    #[test]
    fn event_attribution_is_by_order() {
        let both = replace_events(Ok(()), Ok(()), 2);
        assert_eq!(
            events_line(&both),
            "EVENTS arrived=2 expected=2 a_event=yes b_event=yes"
        );
        let one = replace_events(Ok(()), Ok(()), 1);
        assert_eq!(
            events_line(&one),
            "EVENTS arrived=1 expected=2 a_event=yes b_event=no"
        );
        let refused = replace_events(Ok(()), Err(16), 1);
        assert_eq!(
            events_line(&refused),
            "EVENTS arrived=1 expected=1 a_event=yes b_event=n/a"
        );
        let none = replace_events(Ok(()), Err(16), 0);
        assert!(!none.a_event);
        let a_refused = replace_events(Err(16), Ok(()), 0);
        assert_eq!((a_refused.expected, a_refused.b_event), (0, None));
    }

    #[test]
    fn plane_fb_names_the_commit() {
        assert_eq!(which_fb(0, 1, 2, 3), "none");
        assert_eq!(which_fb(2, 1, 2, 3), "A");
        assert_eq!(which_fb(3, 1, 2, 3), "B");
        assert_eq!(which_fb(1, 1, 2, 3), "setup");
        assert_eq!(which_fb(9, 1, 2, 3), "other");
    }

    #[test]
    fn parses_the_new_cases() {
        assert_eq!(
            args("--plane 1 --replace-blocking").unwrap().mode,
            Mode::ReplaceBlocking
        );
        assert_eq!(
            args("--plane 1 --cursor-move").unwrap().mode,
            Mode::CursorMove
        );
        // Cursor move needs no third buffer; blocking B does.
        assert_eq!(
            args("--plane 1 --cursor-move --buffers 2").unwrap().buffers,
            2
        );
        assert!(args("--plane 1 --replace-blocking --buffers 2").is_err());
        assert!(args("--plane 1 --replace --cursor-move").is_err());
        assert!(args("--plane 1 --cursor-move --replace-blocking").is_err());
        assert!(args("--plane 1 --flips 2 --cursor-move").is_err());
        assert!(args("--plane 1 --cursor-move --flips 2").is_err());
    }

    #[test]
    fn timing_keys_carry_errno_and_return_time() {
        assert_eq!(timing_keys(Ok(()), 120), "errno=0 return_us=120");
        assert_eq!(timing_keys(Err(16), 7), "errno=16 return_us=7");
        assert_eq!(errno_value(Err(22)), 22);
    }

    #[test]
    fn counts_line_for_b_and_c() {
        let one = event_counts(Ok(()), Err(16), 1);
        assert_eq!(
            counts_line(&one, false),
            "COUNTS a_events=1 b_events=n/a c_events=n/a extra_events=0"
        );
        let both = event_counts(Ok(()), Ok(()), 2);
        assert_eq!(
            counts_line(&both, false),
            "COUNTS a_events=1 b_events=1 c_events=n/a extra_events=0"
        );
        assert_eq!(
            counts_line(&both, true),
            "COUNTS a_events=1 b_events=n/a c_events=1 extra_events=0"
        );
        let late = event_counts(Ok(()), Ok(()), 1);
        assert_eq!(late.second, Some(0));
        // A duplicate of A's event shows as extra.
        let dup = event_counts(Ok(()), Err(16), 2);
        assert_eq!((dup.a, dup.second, dup.extra), (1, None, 1));
        let none = event_counts(Err(16), Ok(()), 0);
        assert_eq!((none.a, none.second, none.extra), (0, None, 0));
    }

    #[test]
    fn case_verdict_labels() {
        assert_eq!(
            case_verdict("replace_pending_blocking", Ok(()), Ok(())),
            "replace_pending_blocking: accepted"
        );
        assert_eq!(
            case_verdict("cursor_move_pending", Ok(()), Err(16)),
            "cursor_move_pending: refused errno=EBUSY"
        );
        assert!(case_verdict("cursor_move_pending", Err(22), Ok(()))
            .starts_with("cursor_move_pending: unknown"));
    }

    #[test]
    fn errno_text() {
        assert_eq!(check_text(Ok(())), "ok");
        assert_eq!(check_text(Err(16)), "EBUSY");
        assert_eq!(check_text(Err(22)), "EINVAL");
        assert_eq!(check_text(Err(12345)), "errno12345");
        assert_eq!(opt_text(Some(3)), "3");
        assert_eq!(opt_text::<u32>(None), "n/a");
    }
}
