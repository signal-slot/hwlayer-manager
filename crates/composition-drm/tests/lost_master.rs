//! How the DRM backend reports a lost DRM master or a lost CRTC, pinned
//! against the kernel's virtual KMS driver.
//!
//! The helpers in `tests/common` only load recorded capabilities; there is no
//! mock card that can refuse a commit or drop a CRTC, so these tests need
//! root and a `vkms` device and are ignored by default, like `tests/vkms.rs`.
//! Do not load vkms on a desktop machine to run them; use a throwaway VM
//! (`scripts/vkms-vm-test.sh` builds and runs only the `vkms` test binary).
//!
//! What they pin:
//!
//! * a commit refused because the descriptor is no longer DRM master is
//!   [`DrmFailure::NotMaster`] with `EACCES` (or `EPERM`), reaches the engine
//!   as `BackendError::Platform(errno)`, and the device is *not* lost;
//! * the same engine commits again once the master role is taken back;
//! * a revoked lease (the lost CRTC) found while a frame waits for its latch
//!   is [`DrmFailure::Lost`], marks the device lost, queues `DeviceLost`,
//!   drops the pending latch and releases every buffer, rather than stalling;
//! * a new engine on a working lease latches again.

use std::fs::File;
use std::os::fd::OwnedFd;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use composition_core::{
    BackendError, BufferId, BufferInfo, DisplayEvent, Engine, Error, Frame, Layer, Modifier,
    OutputId, PixelFormat, Rect, Size, SubmissionId,
};
use composition_drm::{Card, DrmBackend, DrmFailure, DrmOptions, Operation};
use drm::control::Device as ControlDevice;
use drm::Device as _;

/// The tests share one device and one DRM master role.
static DEVICE: Mutex<()> = Mutex::new(());

struct Vkms {
    _guard: MutexGuard<'static, ()>,
    path: String,
}

fn vkms() -> Vkms {
    let guard = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
    for index in 0..16 {
        let path = format!("/dev/dri/card{index}");
        let driver = format!("/sys/class/drm/card{index}/device/driver");
        // Decide from sysfs, without opening anything that is not vkms.
        let is_vkms = std::fs::read_link(&driver)
            .is_ok_and(|target| target.file_name().is_some_and(|name| name == "vkms"))
            || std::fs::canonicalize(format!("/sys/class/drm/card{index}/device"))
                .is_ok_and(|p| p.ends_with("vkms"));
        if is_vkms {
            return Vkms {
                _guard: guard,
                path,
            };
        }
    }
    panic!("no vkms device; run these tests in a VM that has one");
}

const XRGB: PixelFormat = PixelFormat::XRGB8888;
const RED: u32 = 0xFFFF_0000;
const BLUE: u32 = 0xFF00_00FF;

fn whole(size: Size) -> Rect {
    Rect::new(0, 0, size.width, size.height)
}

fn output(engine: &Engine<DrmBackend>) -> (OutputId, Size) {
    let out = engine.capabilities().outputs[0];
    (out.id, out.size)
}

fn solid(engine: &mut Engine<DrmBackend>, id: u32, size: Size, colour: u32) -> BufferId {
    let id = BufferId(id);
    let info = BufferInfo {
        size,
        format: XRGB,
        modifier: Modifier::LINEAR,
    };
    engine.backend_mut().create_dumb_buffer(id, info).unwrap();
    engine.register_buffer(id, info).unwrap();
    engine
        .backend_mut()
        .draw_dumb_buffer(id, |pixels, _pitch| {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.copy_from_slice(&colour.to_le_bytes());
            }
        })
        .unwrap();
    engine.finish_write(id).unwrap();
    id
}

/// Collects events until `submission` is presented.
fn wait_presented(engine: &mut Engine<DrmBackend>, submission: SubmissionId) -> Vec<DisplayEvent> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut events = Vec::new();
    let mut seen = false;
    loop {
        while let Some(event) = engine.poll_event().unwrap() {
            seen |=
                matches!(event, DisplayEvent::Presented { submission: s, .. } if s == submission);
            events.push(event);
        }
        if seen {
            return events;
        }
        assert!(Instant::now() < deadline, "no page flip for {submission:?}");
        engine
            .backend()
            .wait_for_events(Some(Duration::from_millis(100)))
            .unwrap();
    }
}

fn show(engine: &mut Engine<DrmBackend>, out: OutputId, layers: &[Layer]) -> Vec<DisplayEvent> {
    let submission = engine
        .present(&Frame {
            output: out,
            layers,
        })
        .unwrap();
    wait_presented(engine, submission)
}

/// Opens the device as plain DRM master with the client capabilities set.
fn lessor(vkms: &Vkms) -> Card {
    let card = Card::from_fd(OwnedFd::from(
        File::options()
            .read(true)
            .write(true)
            .open(&vkms.path)
            .unwrap(),
    ));
    card.set_client_capability(drm::ClientCapability::UniversalPlanes, true)
        .unwrap();
    card.set_client_capability(drm::ClientCapability::Atomic, true)
        .unwrap();
    card
}

/// Leases the first connected connector, the first CRTC and the primary
/// plane.
fn lease(lessor: &Card) -> (drm::control::LeaseId, OwnedFd) {
    let resources = lessor.resource_handles().unwrap();
    let crtc = resources.crtcs()[0];
    let connector = *resources
        .connectors()
        .iter()
        .find(|c| {
            lessor
                .get_connector(**c, false)
                .is_ok_and(|i| i.state() == drm::control::connector::State::Connected)
        })
        .unwrap();
    let plane_type = |plane| {
        let props = lessor.get_properties(plane).unwrap();
        let (handles, values) = props.as_props_and_values();
        handles
            .iter()
            .zip(values)
            .find(|(h, _)| lessor.get_property(**h).unwrap().name().to_bytes() == b"type")
            .map(|(_, v)| *v as u32)
            .unwrap()
    };
    let primary = *lessor
        .plane_handles()
        .unwrap()
        .iter()
        .find(|p| plane_type(**p) == drm::control::PlaneType::Primary as u32)
        .unwrap();
    lessor
        .create_lease(&[connector.into(), crtc.into(), primary.into()], 0)
        .expect("create lease")
}

fn is_not_master_errno(errno: i32) -> bool {
    errno == rustix::io::Errno::ACCESS.raw_os_error()
        || errno == rustix::io::Errno::PERM.raw_os_error()
}

/// (a) A descriptor that was master and gave the role up is refused with
/// `NotMaster`, not found lost, and works again once it takes the role back.
#[test]
#[ignore = "needs root and vkms"]
fn a_commit_after_losing_master_is_not_master_and_not_lost() {
    let vkms = vkms();
    let mut engine = Engine::new(DrmBackend::open(&vkms.path, DrmOptions::default()).unwrap());
    let (out, size) = output(&engine);
    let red = solid(&mut engine, 1, size, RED);
    let blue = solid(&mut engine, 2, size, BLUE);
    let red_layers = [Layer::new(red, whole(size), whole(size), 0)];
    let blue_layers = [Layer::new(blue, whole(size), whole(size), 0)];
    show(&mut engine, out, &red_layers);
    let _ = engine.backend_mut().take_failures().count();

    // Another master takes the role: here the opener drops it, and a second
    // opener takes it, as a compositor starting up would.
    engine.backend().card().release_master_lock().unwrap();
    let other = lessor(&vkms);
    other.acquire_master_lock().unwrap();

    let refused = engine.present(&Frame {
        output: out,
        layers: &blue_layers,
    });
    let errno = match refused {
        Err(Error::Backend(BackendError::Platform(errno))) => errno,
        other => panic!("expected a platform error, got {other:?}"),
    };
    assert!(is_not_master_errno(errno), "errno {errno}");
    let failures: Vec<_> = engine.backend_mut().take_failures().collect();
    assert!(
        matches!(
            failures.as_slice(),
            [DrmFailure::NotMaster { operation: Operation::Check, errno: e }] if *e == errno
        ),
        "{failures:?}"
    );
    assert!(!engine.backend().is_lost());
    assert!(engine.backend().pending_latch(out).is_none());
    assert_eq!(engine.poll_event().unwrap(), None, "no DeviceLost");
    // The buffer of the shown frame is still held, the refused one is free.
    assert_eq!(engine.capabilities().outputs.len(), 1);

    // Master is back: the same engine commits and latches.
    drop(other);
    engine.backend().card().acquire_master_lock().unwrap();
    show(&mut engine, out, &blue_layers);
    assert!(engine.backend_mut().take_failures().next().is_none());
}

/// (b) A frame waits for its latch while the CRTC goes away (a lease
/// revoked at once, which the lessee is not told about). The wait ends in
/// `Lost`, not in silence, and every buffer is given back.
///
/// (c) Afterwards a new engine on a fresh lease latches.
#[test]
#[ignore = "needs root and vkms"]
fn a_lost_crtc_ends_the_latch_wait_releases_buffers_and_a_new_run_latches() {
    let vkms = vkms();
    let lessor = lessor(&vkms);
    let (lessee, fd) = lease(&lessor);
    let mut engine = Engine::new(DrmBackend::from_fd(fd, DrmOptions::default()).unwrap());
    let (out, size) = output(&engine);
    let red = solid(&mut engine, 1, size, RED);
    let blue = solid(&mut engine, 2, size, BLUE);
    show(
        &mut engine,
        out,
        &[Layer::new(red, whole(size), whole(size), 0)],
    );
    let _ = engine.backend_mut().take_failures().count();

    engine.set_latch_timeout(Some(10));
    engine.backend_mut().set_latch_timeout(Some(Duration::ZERO));
    let submission = engine
        .present(&Frame {
            output: out,
            layers: &[Layer::new(blue, whole(size), whole(size), 0)],
        })
        .unwrap();
    lessor.revoke_lease(lessee).unwrap();

    // Drive the engine as a caller would; it must not stall. The flip may
    // have been read before the revocation was seen, so also probe once.
    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut now = 0;
    while !engine.backend().is_lost() && Instant::now() < deadline {
        now += 20;
        engine.advance_time(now);
        while let Some(event) = engine.poll_event().unwrap() {
            events.push(event);
        }
        let _ = engine
            .backend()
            .wait_for_events(Some(Duration::from_millis(20)));
    }
    if !engine.backend().is_lost() {
        assert!(!engine.backend_mut().probe_device());
    }
    now += 20;
    engine.advance_time(now);
    while let Some(event) = engine.poll_event().unwrap() {
        events.push(event);
    }

    assert!(engine.backend().is_lost());
    assert!(engine.backend().pending_latch(out).is_none());
    assert!(events.contains(&DisplayEvent::DeviceLost), "{events:?}");
    for buffer in [red, blue] {
        assert!(
            events.contains(&DisplayEvent::Released { buffer }),
            "{buffer:?} not released: {events:?}"
        );
    }
    let enoent = rustix::io::Errno::NOENT.raw_os_error();
    let failures: Vec<_> = engine.backend_mut().take_failures().collect();
    assert!(
        failures.iter().any(|f| matches!(
            f,
            DrmFailure::Lost { errno, .. } if *errno == enoent || *errno == 0
        )),
        "{failures:?}"
    );
    let _ = submission;

    // A new run: a fresh lease and a new engine latch again.
    let (_lessee, fd) = lease(&lessor);
    let mut engine = Engine::new(DrmBackend::from_fd(fd, DrmOptions::default()).unwrap());
    let (out, size) = output(&engine);
    let green = solid(&mut engine, 1, size, 0xFF00_FF00);
    show(
        &mut engine,
        out,
        &[Layer::new(green, whole(size), whole(size), 0)],
    );
    assert!(!engine.backend().is_lost());
    assert!(engine.backend().last_latch(out).is_some());
}
