//! Tests against the kernel's virtual KMS driver.
//!
//! They need root and a `vkms` device with overlay planes that nothing else
//! drives, so they are ignored by default and run inside a throwaway VM:
//!
//! ```sh
//! scripts/vkms-vm-test.sh
//! ```
//!
//! Do not load vkms on a desktop machine to run them: the compositor picks
//! the device up as a new monitor and keeps DRM master on it.
//!
//! Only a device whose driver is `vkms` is ever opened for modesetting.
//!
//! What is on "screen" is checked through the CRC that vkms computes over its
//! composed output: a multi-plane frame must give the same CRC as a single
//! buffer painted with the expected result.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::OwnedFd;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, BufferState, DisplayBackend, DisplayEvent,
    Engine, Error, Frame, Layer, Modifier, OutputId, PixelFormat, PlanError, PlaneKind,
    PresentError, Rect, RejectReasons, Size, SubmissionId,
};
use composition_drm::{Card, DrmBackend, DrmOptions};
use drm::control::Device as ControlDevice;

/// The tests share one device and one DRM master role.
static DEVICE: Mutex<()> = Mutex::new(());

struct Vkms {
    _guard: MutexGuard<'static, ()>,
    path: String,
    index: u32,
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
                index,
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

    /// CRC of what the CRTC is composing right now.
    fn crc(&self) -> String {
        let dir = format!("/sys/kernel/debug/dri/{}/crtc-0/crc", self.index);
        File::options()
            .write(true)
            .open(format!("{dir}/control"))
            .and_then(|mut f| f.write_all(b"auto"))
            .expect("select CRC source");
        let data = File::open(format!("{dir}/data")).expect("open CRC data");
        let mut lines = BufReader::new(data).lines();
        // The first entries may predate the latest commit.
        let line = lines.nth(3).expect("CRC entry").expect("read CRC");
        line.split_whitespace().last().unwrap().to_string()
    }
}

const XRGB: PixelFormat = PixelFormat::XRGB8888;
const ARGB: PixelFormat = PixelFormat::ARGB8888;

const RED: u32 = 0xFFFF_0000;
const GREEN: u32 = 0xFF00_FF00;
const BLUE: u32 = 0xFF00_00FF;

fn output(engine: &Engine<DrmBackend>) -> (OutputId, Size) {
    let out = engine.capabilities().outputs[0];
    (out.id, out.size)
}

fn fill(pixels: &mut [u8], pitch: usize, rect: Rect, colour: u32) {
    for y in rect.y as usize..rect.y as usize + rect.height as usize {
        for x in rect.x as usize..rect.x as usize + rect.width as usize {
            let at = y * pitch + x * 4;
            pixels[at..at + 4].copy_from_slice(&colour.to_le_bytes());
        }
    }
}

/// Creates a buffer, lets `draw` paint it and marks it ready.
fn buffer(
    engine: &mut Engine<DrmBackend>,
    id: u32,
    size: Size,
    format: PixelFormat,
    draw: impl FnOnce(&mut [u8], usize),
) -> BufferId {
    let id = BufferId(id);
    let info = BufferInfo {
        size,
        format,
        modifier: Modifier::LINEAR,
    };
    engine.backend_mut().create_dumb_buffer(id, info).unwrap();
    engine.register_buffer(id, info).unwrap();
    engine.backend_mut().draw_dumb_buffer(id, draw).unwrap();
    engine.finish_write(id).unwrap();
    id
}

fn solid(
    engine: &mut Engine<DrmBackend>,
    id: u32,
    size: Size,
    format: PixelFormat,
    colour: u32,
) -> BufferId {
    buffer(engine, id, size, format, |pixels, pitch| {
        fill(
            pixels,
            pitch,
            Rect::new(0, 0, size.width, size.height),
            colour,
        );
    })
}

fn whole(size: Size) -> Rect {
    Rect::new(0, 0, size.width, size.height)
}

fn overlay(buffer: BufferId, size: Size, x: i32, y: i32, z_order: i32) -> Layer {
    let mut layer = Layer::new(
        buffer,
        whole(size),
        Rect::new(x, y, size.width, size.height),
        z_order,
    );
    layer.blend = BlendMode::Premultiplied;
    layer
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

#[test]
#[ignore = "needs root and vkms"]
fn two_planes_compose_like_the_flattened_reference() {
    let vkms = vkms();
    let mut engine = vkms.engine();
    let (out, size) = output(&engine);
    let square = Size::new(256, 256);

    let background = solid(&mut engine, 1, size, XRGB, RED);
    let green = solid(&mut engine, 2, square, ARGB, GREEN);
    let reference = buffer(&mut engine, 3, size, XRGB, |pixels, pitch| {
        fill(pixels, pitch, whole(size), RED);
        fill(pixels, pitch, Rect::new(100, 100, 256, 256), GREEN);
    });

    let layers = [
        Layer::new(background, whole(size), whole(size), 0),
        overlay(green, square, 100, 100, 1),
    ];
    let frame = Frame {
        output: out,
        layers: &layers,
    };
    // Two layers really go to two different hardware planes.
    let plan = engine.validate(&frame).unwrap();
    let planes: Vec<_> = plan.assignments().iter().map(|a| a.plane).collect();
    assert_eq!(planes.len(), 2);
    assert_ne!(planes[0], planes[1]);
    let caps = engine.capabilities();
    assert_eq!(
        caps.planes[plan.assignments()[0].plane_index].kind,
        PlaneKind::Primary
    );
    assert_eq!(
        caps.planes[plan.assignments()[1].plane_index].kind,
        PlaneKind::Overlay
    );

    let submission = engine.commit(&frame, &plan).unwrap();
    wait_presented(&mut engine, submission);
    let composed = vkms.crc();

    show(
        &mut engine,
        out,
        &[Layer::new(reference, whole(size), whole(size), 0)],
    );
    let flattened = vkms.crc();
    assert_eq!(composed, flattened);

    // The CRC is sensitive to the overlay: without it the result differs.
    show(
        &mut engine,
        out,
        &[Layer::new(background, whole(size), whole(size), 0)],
    );
    assert_ne!(vkms.crc(), composed);
}

#[test]
#[ignore = "needs root and vkms"]
fn z_order_decides_which_overlay_is_on_top() {
    let vkms = vkms();
    let mut engine = vkms.engine();
    let (out, size) = output(&engine);
    let square = Size::new(200, 200);

    let background = solid(&mut engine, 1, size, XRGB, RED);
    let green = solid(&mut engine, 2, square, ARGB, GREEN);
    let blue = solid(&mut engine, 3, square, ARGB, BLUE);
    // Green at 100,100 and blue at 200,200 overlap in a 100x100 corner.
    let reference = |top: u32, bottom: u32, bottom_at: i32, top_at: i32| {
        move |pixels: &mut [u8], pitch: usize| {
            fill(pixels, pitch, whole(size), RED);
            fill(
                pixels,
                pitch,
                Rect::new(bottom_at, bottom_at, 200, 200),
                bottom,
            );
            fill(pixels, pitch, Rect::new(top_at, top_at, 200, 200), top);
        }
    };
    let blue_on_top = buffer(&mut engine, 4, size, XRGB, reference(BLUE, GREEN, 100, 200));
    let green_on_top = buffer(&mut engine, 5, size, XRGB, reference(GREEN, BLUE, 200, 100));

    let base = Layer::new(background, whole(size), whole(size), 0);
    let full = |buffer| Layer::new(buffer, whole(size), whole(size), 0);

    show(
        &mut engine,
        out,
        &[
            base,
            overlay(green, square, 100, 100, 1),
            overlay(blue, square, 200, 200, 2),
        ],
    );
    let composed_blue_top = vkms.crc();
    show(&mut engine, out, &[full(blue_on_top)]);
    assert_eq!(composed_blue_top, vkms.crc());

    // Same buffers, same positions, only the stacking order swapped.
    show(
        &mut engine,
        out,
        &[
            base,
            overlay(green, square, 100, 100, 2),
            overlay(blue, square, 200, 200, 1),
        ],
    );
    let composed_green_top = vkms.crc();
    show(&mut engine, out, &[full(green_on_top)]);
    assert_eq!(composed_green_top, vkms.crc());

    assert_ne!(composed_blue_top, composed_green_top);
}

#[test]
#[ignore = "needs root and vkms"]
fn premultiplied_pixel_alpha_blends_with_the_plane_below() {
    let vkms = vkms();
    let mut engine = vkms.engine();
    let (out, size) = output(&engine);
    let square = Size::new(256, 256);

    let background = solid(&mut engine, 1, size, XRGB, RED);
    // Half-transparent green, premultiplied: alpha 0x80, green 0x80.
    let glass = solid(&mut engine, 2, square, ARGB, 0x8000_8000);
    // 0x80 over opaque red: red keeps 0x7F, green contributes 0x80.
    let reference = buffer(&mut engine, 3, size, XRGB, |pixels, pitch| {
        fill(pixels, pitch, whole(size), RED);
        fill(pixels, pitch, Rect::new(64, 64, 256, 256), 0xFF7F_8000);
    });

    show(
        &mut engine,
        out,
        &[
            Layer::new(background, whole(size), whole(size), 0),
            overlay(glass, square, 64, 64, 1),
        ],
    );
    let composed = vkms.crc();
    show(
        &mut engine,
        out,
        &[Layer::new(reference, whole(size), whole(size), 0)],
    );
    assert_eq!(composed, vkms.crc());
}

#[test]
#[ignore = "needs root and vkms"]
fn overlay_hanging_over_the_edge_is_clipped() {
    let vkms = vkms();
    let mut engine = vkms.engine();
    let (out, size) = output(&engine);
    let square = Size::new(256, 256);

    let background = solid(&mut engine, 1, size, XRGB, RED);
    // Left half green, right half blue, so a wrong source offset shows.
    let two_tone = buffer(&mut engine, 2, square, ARGB, |pixels, pitch| {
        fill(pixels, pitch, Rect::new(0, 0, 128, 256), GREEN);
        fill(pixels, pitch, Rect::new(128, 0, 128, 256), BLUE);
    });
    // Placed at -56,-40: 72 green and 128 blue columns, 216 rows remain.
    let reference = buffer(&mut engine, 3, size, XRGB, |pixels, pitch| {
        fill(pixels, pitch, whole(size), RED);
        fill(pixels, pitch, Rect::new(0, 0, 72, 216), GREEN);
        fill(pixels, pitch, Rect::new(72, 0, 128, 216), BLUE);
    });
    let base = Layer::new(background, whole(size), whole(size), 0);

    let layers = [base, overlay(two_tone, square, -56, -40, 1)];
    let frame = Frame {
        output: out,
        layers: &layers,
    };
    let plan = engine.validate(&frame).unwrap();
    assert_eq!(plan.assignments()[1].source, Rect::new(56, 40, 200, 216));
    assert_eq!(plan.assignments()[1].destination, Rect::new(0, 0, 200, 216));
    let submission = engine.commit(&frame, &plan).unwrap();
    wait_presented(&mut engine, submission);
    let composed = vkms.crc();

    show(
        &mut engine,
        out,
        &[Layer::new(reference, whole(size), whole(size), 0)],
    );
    assert_eq!(composed, vkms.crc());

    // Entirely off the output: no plane, and the buffer stays free.
    let gone = [base, overlay(two_tone, square, size.width as i32, 0, 1)];
    let frame = Frame {
        output: out,
        layers: &gone,
    };
    assert_eq!(engine.validate(&frame).unwrap().assignments().len(), 1);
    show(&mut engine, out, &gone);
    assert_eq!(engine.buffer_state(two_tone), Some(BufferState::Ready));
}

#[test]
#[ignore = "needs root and vkms"]
fn buffers_are_released_only_after_the_next_page_flip() {
    let vkms = vkms();
    let mut engine = vkms.engine();
    let (out, size) = output(&engine);
    let first = solid(&mut engine, 1, size, XRGB, RED);
    let second = solid(&mut engine, 2, size, XRGB, BLUE);
    let full = |buffer| [Layer::new(buffer, whole(size), whole(size), 0)];

    let events = show(&mut engine, out, &full(first));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DisplayEvent::Released { .. })));
    assert_eq!(engine.buffer_state(first), Some(BufferState::Scanning));

    let layers = full(second);
    let frame = Frame {
        output: out,
        layers: &layers,
    };
    let submission = engine.present(&frame).unwrap();
    // Accepted by the kernel, not yet flipped: the old buffer is still ours
    // to protect, and a further commit is refused.
    assert_eq!(
        engine.begin_write(first),
        Err(Error::Present(PresentError::BufferInUse(first)))
    );
    assert_eq!(
        engine.present(&frame),
        Err(Error::Present(PresentError::Busy(out)))
    );

    let events = wait_presented(&mut engine, submission);
    assert_eq!(
        events,
        [
            DisplayEvent::Presented {
                output: out,
                submission
            },
            DisplayEvent::Released { buffer: first }
        ]
    );
    engine.begin_write(first).unwrap();

    // Many rotations: every flip releases exactly the previous buffer.
    engine.finish_write(first).unwrap();
    let mut shown = second;
    let mut idle = first;
    for _ in 0..20 {
        let events = show(&mut engine, out, &full(idle));
        assert!(events.contains(&DisplayEvent::Released { buffer: shown }));
        std::mem::swap(&mut shown, &mut idle);
    }
}

#[test]
#[ignore = "needs root and vkms"]
fn unsupported_frames_are_rejected_before_touching_the_display() {
    let vkms = vkms();
    let mut engine = vkms.engine();
    let (out, size) = output(&engine);
    let background = solid(&mut engine, 1, size, XRGB, RED);
    let small = solid(&mut engine, 2, Size::new(64, 64), ARGB, GREEN);
    show(
        &mut engine,
        out,
        &[Layer::new(background, whole(size), whole(size), 0)],
    );
    let before = vkms.crc();

    // Scaling is not claimed for this device, so the planner refuses.
    let scaled = [
        Layer::new(background, whole(size), whole(size), 0),
        Layer::new(small, Rect::new(0, 0, 64, 64), Rect::new(0, 0, 128, 128), 1),
    ];
    let result = engine.present(&Frame {
        output: out,
        layers: &scaled,
    });
    let Err(Error::Plan(PlanError::NoPlane { layer: 1, reasons })) = result else {
        panic!("unexpected {result:?}");
    };
    assert!(reasons.contains(RejectReasons::SCALING));

    // More layers than planes.
    let plane_count = engine.capabilities().planes.len();
    let many: Vec<Layer> = (0..plane_count as i32 + 1)
        .map(|z| overlay(small, Size::new(64, 64), 8 * z, 8 * z, z))
        .collect();
    assert!(matches!(
        engine.present(&Frame {
            output: out,
            layers: &many
        }),
        Err(Error::Plan(PlanError::NoPlane { .. }))
    ));

    // A buffer the backend never imported fails the kernel-side check.
    let ghost = BufferId(99);
    engine
        .register_buffer(
            ghost,
            BufferInfo {
                size,
                format: XRGB,
                modifier: Modifier::LINEAR,
            },
        )
        .unwrap();
    engine.finish_write(ghost).unwrap();
    assert_eq!(
        engine.present(&Frame {
            output: out,
            layers: &[Layer::new(ghost, whole(size), whole(size), 0)]
        }),
        Err(Error::Backend(BackendError::UnknownBuffer(ghost)))
    );

    assert_eq!(vkms.crc(), before);
}

#[test]
#[ignore = "needs root and vkms"]
fn leased_output_exposes_only_leased_planes_and_reports_revocation() {
    let vkms = vkms();
    // The lessor: plain DRM master that hands out a lease.
    let lessor = Card::from_fd(OwnedFd::from(
        File::options()
            .read(true)
            .write(true)
            .open(&vkms.path)
            .unwrap(),
    ));
    use drm::Device as _;
    lessor
        .set_client_capability(drm::ClientCapability::UniversalPlanes, true)
        .unwrap();
    lessor
        .set_client_capability(drm::ClientCapability::Atomic, true)
        .unwrap();
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
    let planes = lessor.plane_handles().unwrap();
    let primary = *planes
        .iter()
        .find(|p| plane_type(**p) == drm::control::PlaneType::Primary as u32)
        .unwrap();
    let overlay_plane = *planes
        .iter()
        .find(|p| plane_type(**p) == drm::control::PlaneType::Overlay as u32)
        .unwrap();

    // Lease the connector, the CRTC, the primary plane and one overlay.
    let objects = [
        connector.into(),
        crtc.into(),
        primary.into(),
        overlay_plane.into(),
    ];
    let (lessee, lease_fd) = lessor.create_lease(&objects, 0).expect("create lease");

    let mut engine = Engine::new(DrmBackend::from_fd(lease_fd, DrmOptions::default()).unwrap());
    let caps = *engine.capabilities();
    assert_eq!(caps.outputs.len(), 1);
    let leased: Vec<u32> = caps.planes.iter().map(|p| p.id.0).collect();
    assert_eq!(leased, [u32::from(primary), u32::from(overlay_plane)]);

    let (out, size) = output(&engine);
    let square = Size::new(256, 256);
    let background = solid(&mut engine, 1, size, XRGB, RED);
    let green = solid(&mut engine, 2, square, ARGB, GREEN);
    let blue = solid(&mut engine, 3, square, ARGB, BLUE);
    let reference = buffer(&mut engine, 4, size, XRGB, |pixels, pitch| {
        fill(pixels, pitch, whole(size), RED);
        fill(pixels, pitch, Rect::new(100, 100, 256, 256), GREEN);
    });
    let base = Layer::new(background, whole(size), whole(size), 0);

    // Two leased planes compose through the lease.
    show(
        &mut engine,
        out,
        &[base, overlay(green, square, 100, 100, 1)],
    );
    let composed = vkms.crc();
    show(
        &mut engine,
        out,
        &[Layer::new(reference, whole(size), whole(size), 0)],
    );
    assert_eq!(composed, vkms.crc());

    // The device has more overlays, but the lease does not: a third layer
    // has nowhere to go.
    let three = [
        base,
        overlay(green, square, 100, 100, 1),
        overlay(blue, square, 300, 300, 2),
    ];
    assert_eq!(
        engine.present(&Frame {
            output: out,
            layers: &three
        }),
        Err(Error::Plan(PlanError::NoPlane {
            layer: 2,
            reasons: RejectReasons::IN_USE
        }))
    );

    // Revocation: the next attempt fails and the loss is reported once,
    // with everything that was on screen released.
    lessor.revoke_lease(lessee).unwrap();
    let result = engine.present(&Frame {
        output: out,
        layers: &[base],
    });
    assert_eq!(result, Err(Error::Backend(BackendError::DeviceLost)));
    let mut events = Vec::new();
    while let Some(event) = engine.poll_event().unwrap() {
        events.push(event);
    }
    assert_eq!(
        events,
        [
            DisplayEvent::Released { buffer: reference },
            DisplayEvent::DeviceLost
        ]
    );
    assert!(engine.capabilities().outputs.is_empty());
    assert_eq!(
        engine.backend_mut().capabilities().planes.len(),
        0,
        "a lost device claims nothing"
    );
}
