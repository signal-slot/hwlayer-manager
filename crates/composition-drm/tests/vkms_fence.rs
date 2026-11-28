//! Acquire fences (`IN_FENCE_FD`) against the kernel's virtual KMS driver.
//!
//! Like `tests/vkms.rs`, these need root and a `vkms` device that nothing
//! else drives, so they are ignored by default and run inside a throwaway
//! VM:
//!
//! ```sh
//! scripts/vkms-vm-test.sh
//! ```
//!
//! Do not load vkms on a desktop machine to run them. Only a device whose
//! driver is `vkms` is ever opened. vkms is a virtual device: a pass here
//! says nothing about any SoC's fence handling or timing.
//!
//! Sources of a real `sync_file` that safe Rust can reach inside the VM are
//! few: vkms writeback and `OUT_FENCE_PTR` hand back a raw descriptor
//! number (owning it needs `unsafe`), and `sw_sync` and the dma-buf
//! `EXPORT_SYNC_FILE` need raw ioctls. The one tried here is a DRM syncobj
//! created signalled and exported as a `sync_file`; if the driver has no
//! syncobj support, that test prints a `SKIP` line and passes without
//! claiming anything. The other test commits with a descriptor that is not
//! a `sync_file`, which the kernel must refuse, and needs no fence source.

use std::os::fd::OwnedFd;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use composition_core::{
    BackendError, BufferId, BufferInfo, DisplayEvent, Engine, Error, Frame, Layer, Modifier,
    OutputId, PixelFormat, Rect, Size, SubmissionId,
};
use composition_drm::{DrmBackend, DrmFailure, DrmOptions, Operation};
use drm::control::Device as ControlDevice;

/// The tests share one device and one DRM master role.
static DEVICE: Mutex<()> = Mutex::new(());

const EINVAL: i32 = 22;

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
    panic!("no vkms device; run these tests with scripts/vkms-vm-test.sh");
}

impl Vkms {
    fn engine(&self) -> Engine<DrmBackend> {
        let backend = DrmBackend::open(&self.path, DrmOptions::default()).expect("open vkms");
        assert_eq!(backend.driver_name().unwrap(), "vkms");
        Engine::new(backend)
    }
}

fn output(engine: &Engine<DrmBackend>) -> (OutputId, Size) {
    let out = engine.capabilities().outputs[0];
    (out.id, out.size)
}

/// A full-output XRGB dumb buffer painted `colour` and marked ready.
fn solid(engine: &mut Engine<DrmBackend>, id: u32, size: Size, colour: u32) -> BufferId {
    let id = BufferId(id);
    let info = BufferInfo {
        size,
        format: PixelFormat::XRGB8888,
        modifier: Modifier::LINEAR,
    };
    engine.backend_mut().create_dumb_buffer(id, info).unwrap();
    engine.register_buffer(id, info).unwrap();
    engine
        .backend_mut()
        .draw_dumb_buffer(id, |pixels, _| {
            for px in pixels.chunks_exact_mut(4) {
                px.copy_from_slice(&colour.to_le_bytes());
            }
        })
        .unwrap();
    engine.finish_write(id).unwrap();
    id
}

fn background(buffer: BufferId, size: Size) -> Layer {
    let whole = Rect::new(0, 0, size.width, size.height);
    Layer::new(buffer, whole, whole, 0)
}

fn wait_presented(engine: &mut Engine<DrmBackend>, submission: SubmissionId) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        while let Some(event) = engine.poll_event().unwrap() {
            if matches!(event, DisplayEvent::Presented { submission: s, .. } if s == submission) {
                return;
            }
        }
        assert!(Instant::now() < deadline, "no page flip for {submission:?}");
        engine
            .backend()
            .wait_for_events(Some(Duration::from_millis(100)))
            .unwrap();
    }
}

/// Every plane vkms exposes lists `IN_FENCE_FD` (atomic helpers).
fn assert_in_fence_listed(engine: &Engine<DrmBackend>) {
    for plane in engine.capabilities().planes.iter() {
        assert!(
            engine.backend().plane_in_fence_supported(plane.id),
            "plane {:?} without IN_FENCE_FD",
            plane.id
        );
    }
    let (out, _) = output(engine);
    println!(
        "vkms: IN_FENCE_FD on all {} planes, OUT_FENCE_PTR on output {:?}: {}",
        engine.capabilities().planes.len(),
        out,
        engine.backend().output_out_fence_supported(out)
    );
}

#[test]
#[ignore = "needs root and vkms"]
fn a_descriptor_that_is_not_a_sync_file_is_refused_as_a_platform_error() {
    let vkms = vkms();
    let mut engine = vkms.engine();
    assert_in_fence_listed(&engine);
    let (out, size) = output(&engine);
    let buffer = solid(&mut engine, 1, size, 0xFF20_4080);
    let not_a_fence: OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
    engine
        .backend_mut()
        .set_acquire_fence(buffer, not_a_fence)
        .unwrap();

    let layers = [background(buffer, size)];
    let frame = Frame {
        output: out,
        layers: &layers,
    };
    let error = engine.present(&frame).unwrap_err();
    assert_eq!(error, Error::Backend(BackendError::Platform(EINVAL)));
    assert!(matches!(
        engine.backend().last_failure(),
        Some(DrmFailure::Refused {
            operation: Operation::Check,
            errno: EINVAL,
            ..
        })
    ));
    // Refused, not dropped: the fence is still attached, nothing was shown.
    assert!(engine.backend().acquire_fence(buffer).is_some());
    assert!(engine.backend().pending_latch(out).is_none());

    // Without the fence the same frame is presented as before.
    drop(engine.backend_mut().clear_acquire_fence(buffer));
    let submission = engine.present(&frame).unwrap();
    wait_presented(&mut engine, submission);
    println!("VERDICT vkms_fence not-a-sync-file: PASS (refused with EINVAL as Platform)");
}

#[test]
#[ignore = "needs root and vkms"]
fn a_frame_with_a_signalled_in_fence_is_presented() {
    let vkms = vkms();
    let mut engine = vkms.engine();
    assert_in_fence_listed(&engine);
    let card = engine.backend().card();
    let fence = match card.create_syncobj(true).and_then(|handle| {
        let fd = card.syncobj_to_fd(handle, true);
        let _ = card.destroy_syncobj(handle);
        fd
    }) {
        Ok(fd) => fd,
        Err(error) => {
            println!(
                "VERDICT vkms_fence signalled: SKIP no sync_file source (syncobj: {error}); \
                 nothing claimed"
            );
            return;
        }
    };
    let (out, size) = output(&engine);
    let first = solid(&mut engine, 1, size, 0xFF00_8000);
    let second = solid(&mut engine, 2, size, 0xFF80_0000);

    engine
        .backend_mut()
        .set_acquire_fence(first, fence)
        .unwrap();
    let layers = [background(first, size)];
    let submission = engine
        .present(&Frame {
            output: out,
            layers: &layers,
        })
        .unwrap();
    // The backend closes its copy once the commit was accepted.
    assert!(engine.backend().acquire_fence(first).is_none());
    wait_presented(&mut engine, submission);

    // A page flip (no modeset) with a fence on the new buffer.
    let fence = card_fence(&engine);
    engine
        .backend_mut()
        .set_acquire_fence(second, fence)
        .unwrap();
    let layers = [background(second, size)];
    let submission = engine
        .present(&Frame {
            output: out,
            layers: &layers,
        })
        .unwrap();
    assert!(engine.backend().acquire_fence(second).is_none());
    wait_presented(&mut engine, submission);
    println!("VERDICT vkms_fence signalled: PASS (modeset and flip with IN_FENCE_FD)");
}

/// A signalled `sync_file` from a syncobj on the device.
fn card_fence(engine: &Engine<DrmBackend>) -> OwnedFd {
    let card = engine.backend().card();
    let handle = card.create_syncobj(true).unwrap();
    let fd = card.syncobj_to_fd(handle, true).unwrap();
    let _ = card.destroy_syncobj(handle);
    fd
}
