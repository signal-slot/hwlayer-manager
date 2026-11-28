//! Modeset (A5) and display-recovery (A6) scenarios on the recorded
//! STM32MP257F-DK (LTDC) topology, on the fake backend (SoW §10, X-02).
//!
//! HOST RESULTS ON A MODEL. The topology is typed in from
//! `docs/observed/stm32mp257-topology.md` (planes 34 Primary, 42 and 49
//! Overlay, zpos 0..=2, one CRTC, connector HDMI-A-1; NV12 on plane 42 only;
//! no scaling claimed). Nothing here ran on the board. These tests are never
//! a hardware result and never count toward M1, M2 or M3: they show only that
//! the shared planner and engine react to a mode change and to a display loss
//! on this topology the way the scenario text expects.
//!
//! Modeled stand-ins: a mode change is `set_capabilities` with another output
//! size (the fake has no modeset); a display loss is `lose_device` and its
//! recovery `restore` (no hotplug, EDID or link-training behaviour).
//!
//! The topology helpers repeat those of `topology_stm32mp257.rs`, which this
//! file does not change.

#![forbid(unsafe_code)]

use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, Capabilities, DisplayEvent, DropReason, Engine,
    Error, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId, PixelFormat, PlanError,
    PlaneCaps, PlaneId, PlaneKind, Rect, RejectReasons, Size, SubmissionId, Trace, TraceEvent,
    Zpos,
};
use composition_fake::{FakeBackend, PlaneState};

const OUT: OutputId = OutputId(0);
const FULL_HD: Size = Size::new(1920, 1080);
const HD: Size = Size::new(1280, 720);
const WIN: Size = Size::new(384, 540);
const P34: PlaneId = PlaneId(34);
const P42: PlaneId = PlaneId(42);
const P49: PlaneId = PlaneId(49);
const XR24: PixelFormat = PixelFormat::XRGB8888;
const AR24: PixelFormat = PixelFormat::ARGB8888;

const RGB_FORMATS: [&[u8; 4]; 17] = [
    b"AR24", b"XR24", b"AB24", b"XB24", b"RA24", b"RX24", b"BA24", b"BX24", b"RG16", b"BG16",
    b"RG24", b"BG24", b"AR15", b"XR15", b"AR12", b"XR12", b"C8  ",
];
const PACKED_YUV: [&[u8; 4]; 4] = [b"YUYV", b"YVYU", b"UYVY", b"VYUY"];
const PLANAR_YUV: [&[u8; 4]; 4] = [b"NV12", b"NV21", b"YU12", b"YV12"];

fn plane(id: PlaneId, kind: PlaneKind, extra: &[&[u8; 4]]) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id,
        kind,
        outputs: 1,
        scaling: None,
        plane_alpha: true,
        blend_modes: BlendMode::Premultiplied.mask(),
        transforms: 0,
        zpos: Zpos::Range { min: 0, max: 2 },
        ..PlaneCaps::default()
    };
    for code in RGB_FORMATS.iter().copied().chain(extra.iter().copied()) {
        caps.formats
            .push(FormatModifier::linear(PixelFormat::fourcc(code)))
            .unwrap();
    }
    caps
}

/// The observed planes with the one CRTC running `size`.
fn caps_for(size: Size) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size,
            refresh_mhz: 60_000,
        })
        .unwrap();
    let mut yuv42 = PACKED_YUV.to_vec();
    yuv42.extend_from_slice(&PLANAR_YUV);
    for p in [
        plane(P34, PlaneKind::Primary, &PACKED_YUV),
        plane(P42, PlaneKind::Overlay, &yuv42),
        plane(P49, PlaneKind::Overlay, &[]),
    ] {
        caps.planes.push(p).unwrap();
    }
    caps
}

// ------------------------------------------------------------------ trace

/// What the trace tests compare: the output is always `OUT`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Ev {
    Commit(SubmissionId),
    Latched(SubmissionId),
    Released(BufferId, Option<SubmissionId>),
    Dropped(SubmissionId, DropReason),
    DeviceLost,
    CapabilitiesChanged,
    Other,
}

#[derive(Default)]
struct Recorder(Vec<Ev>);

impl Trace for Recorder {
    fn record(&mut self, _at: u64, event: TraceEvent<'_>) {
        self.0.push(match event {
            TraceEvent::Commit { result: Ok(s), .. } => Ev::Commit(s),
            TraceEvent::Latched { submission, .. } => Ev::Latched(submission),
            TraceEvent::Released { buffer, submission } => Ev::Released(buffer, submission),
            TraceEvent::Dropped {
                submission, reason, ..
            } => Ev::Dropped(submission, reason),
            TraceEvent::DeviceLost => Ev::DeviceLost,
            TraceEvent::CapabilitiesChanged => Ev::CapabilitiesChanged,
            _ => Ev::Other,
        });
    }
}

type Eng = Engine<FakeBackend, Recorder>;

fn engine(size: Size) -> Eng {
    Engine::new(FakeBackend::new(caps_for(size))).with_trace(Recorder::default())
}

fn register(engine: &mut Eng, id: u32, size: Size, format: PixelFormat) -> BufferId {
    let id = BufferId(id);
    engine
        .register_buffer(
            id,
            BufferInfo {
                size,
                format,
                modifier: Modifier::LINEAR,
            },
        )
        .unwrap();
    engine.finish_write(id).unwrap();
    id
}

fn layer(buffer: BufferId, size: Size, dst: Rect, z: i32) -> Layer {
    let mut l = Layer::new(buffer, full(size), dst, z);
    l.blend = BlendMode::Premultiplied;
    l
}

fn full(size: Size) -> Rect {
    Rect::new(0, 0, size.width, size.height)
}

fn frame(layers: &[Layer]) -> Frame<'_> {
    Frame {
        output: OUT,
        layers,
    }
}

fn drain(engine: &mut Eng) -> Vec<DisplayEvent> {
    let mut events = Vec::new();
    while let Some(e) = engine.poll_event().unwrap() {
        events.push(e);
    }
    events
}

fn placement(engine: &mut Eng, layers: &[Layer]) -> Vec<(usize, u32, i32)> {
    engine
        .validate(&frame(layers))
        .unwrap()
        .assignments()
        .iter()
        .map(|a| (a.layer, a.plane.0, a.zpos))
        .collect()
}

fn no_plane(error: Error) -> (usize, RejectReasons) {
    match error {
        Error::Plan(PlanError::NoPlane { layer, reasons }) => (layer, reasons),
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

fn position<F: Fn(&Ev) -> bool>(log: &[Ev], f: F) -> usize {
    log.iter().position(f).expect("event in the trace")
}

fn presented(output: OutputId, submission: SubmissionId) -> DisplayEvent {
    DisplayEvent::Presented { output, submission }
}

// -------------------------------------------------------------- A5 modeset

/// A5: 1920x1080 to 1280x720 and back. The planner places the same scene on
/// the same planes in each mode, and frames keep latching across the change.
#[test]
fn mode_change_down_and_back_keeps_plane_assignment() {
    let mut e = engine(FULL_HD);
    let bg_hd = register(&mut e, 1, FULL_HD, XR24);
    let ovl = register(&mut e, 2, WIN, AR24);
    let scene_hd = [
        layer(bg_hd, FULL_HD, full(FULL_HD), 0),
        layer(ovl, WIN, Rect::new(960, 270, 384, 540), 1),
    ];
    assert_eq!(placement(&mut e, &scene_hd), [(0, 34, 0), (1, 42, 1)]);
    let s1 = e.present(&frame(&scene_hd)).unwrap();
    e.backend_mut().vsync();
    assert!(drain(&mut e).contains(&presented(OUT, s1)));

    // Smaller mode: a background sized for it fits plane 34 unscaled.
    e.backend_mut().set_capabilities(caps_for(HD));
    assert_eq!(drain(&mut e), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(e.capabilities().outputs[0].size, HD);
    let bg_small = register(&mut e, 3, HD, XR24);
    let scene_small = [
        layer(bg_small, HD, full(HD), 0),
        layer(ovl, WIN, Rect::new(448, 90, 384, 540), 1),
    ];
    assert_eq!(placement(&mut e, &scene_small), [(0, 34, 0), (1, 42, 1)]);
    let s2 = e.present(&frame(&scene_small)).unwrap();
    assert!(s2 > s1);
    e.backend_mut().vsync();
    assert!(drain(&mut e).contains(&presented(OUT, s2)));
    assert_eq!(
        e.backend().scanout(OUT).unwrap(),
        [
            PlaneState {
                plane: P34,
                buffer: bg_small,
                zpos: 0
            },
            PlaneState {
                plane: P42,
                buffer: ovl,
                zpos: 1
            },
        ]
    );

    // And back: the original scene plans as before.
    e.backend_mut().set_capabilities(caps_for(FULL_HD));
    assert_eq!(drain(&mut e), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(placement(&mut e, &scene_hd), [(0, 34, 0), (1, 42, 1)]);
    let s3 = e.present(&frame(&scene_hd)).unwrap();
    assert!(s3 > s2);
    e.backend_mut().vsync();
    assert!(drain(&mut e).contains(&presented(OUT, s3)));
}

/// A5: after the smaller mode, a buffer still sized for 1920x1080 would need
/// a downscale, which no plane claims, so it is refused for SCALING; at
/// 1920x1080 it is accepted again.
#[test]
fn scaled_layer_is_refused_in_the_smaller_mode_only() {
    let mut e = engine(FULL_HD);
    let bg = register(&mut e, 1, FULL_HD, XR24);
    let layers_hd = [layer(bg, FULL_HD, full(FULL_HD), 0)];
    assert_eq!(placement(&mut e, &layers_hd), [(0, 34, 0)]);

    e.backend_mut().set_capabilities(caps_for(HD));
    drain(&mut e);
    let shrunk = [layer(bg, FULL_HD, full(HD), 0)];
    assert_eq!(
        no_plane(e.validate(&frame(&shrunk)).unwrap_err()),
        (0, RejectReasons::SCALING)
    );
    assert_eq!(e.backend().commit_count(), 0);

    e.backend_mut().set_capabilities(caps_for(FULL_HD));
    drain(&mut e);
    assert_eq!(placement(&mut e, &layers_hd), [(0, 34, 0)]);
}

/// A5: NV12 reaches plane 42 only, in both modes. A second NV12 layer finds
/// no plane (FORMAT), and NV12 sized for the larger mode needs a downscale in
/// the smaller one and is refused for SCALING.
#[test]
fn nv12_stays_on_plane_42_and_a_second_nv12_layer_is_refused() {
    for size in [FULL_HD, HD] {
        let mut e = engine(size);
        let a = register(&mut e, 1, size, PixelFormat::NV12);
        let b = register(&mut e, 2, size, PixelFormat::NV12);
        assert_eq!(
            placement(&mut e, &[layer(a, size, full(size), 0)]),
            [(0, 42, 0)],
            "{size:?}"
        );
        let two = [layer(a, size, full(size), 0), layer(b, size, full(size), 1)];
        let (idx, reasons) = no_plane(e.validate(&frame(&two)).unwrap_err());
        assert_eq!(idx, 1, "{size:?}");
        // FORMAT: planes 34 and 49 lack NV12; IN_USE: plane 42 holds layer 0.
        assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
    }

    let mut e = engine(FULL_HD);
    let nv = register(&mut e, 1, FULL_HD, PixelFormat::NV12);
    e.backend_mut().set_capabilities(caps_for(HD));
    drain(&mut e);
    let (idx, reasons) = no_plane(
        e.validate(&frame(&[layer(nv, FULL_HD, full(HD), 0)]))
            .unwrap_err(),
    );
    assert_eq!(idx, 0);
    assert!(reasons.contains(RejectReasons::SCALING), "{reasons:?}");
}

/// A5: three layers use planes 34, 42 and 49 in either mode.
#[test]
fn three_layers_use_all_planes_in_either_mode() {
    for size in [FULL_HD, HD] {
        let mut e = engine(size);
        let bg = register(&mut e, 1, size, XR24);
        let small = Size::new(256, 256);
        let a = register(&mut e, 2, small, AR24);
        let b = register(&mut e, 3, small, AR24);
        let layers = [
            layer(bg, size, full(size), 0),
            layer(a, small, Rect::new(10, 10, 256, 256), 1),
            layer(b, small, Rect::new(300, 10, 256, 256), 2),
        ];
        assert_eq!(
            placement(&mut e, &layers),
            [(0, 34, 0), (1, 42, 1), (2, 49, 2)],
            "{size:?}"
        );
    }
}

// ------------------------------------------------------------ A6 recovery

/// A6: a display loss with a frame pending, then recovery on the same
/// topology. Trace order: commit, latch, commit, loss (drop of the pending
/// frame), caps change; then frames flow again and the buffers of the dropped
/// frame were released once.
#[test]
fn display_loss_and_recovery_release_frames_and_trace_in_order() {
    let mut e = engine(FULL_HD);
    let bgs = [
        register(&mut e, 1, FULL_HD, XR24),
        register(&mut e, 2, FULL_HD, XR24),
    ];
    let ovls = [
        register(&mut e, 3, WIN, AR24),
        register(&mut e, 4, WIN, AR24),
    ];
    let scene = |n: usize| {
        [
            layer(bgs[n % 2], FULL_HD, full(FULL_HD), 0),
            layer(ovls[n % 2], WIN, Rect::new(960, 270, 384, 540), 1),
        ]
    };

    let on_screen = e.present(&frame(&scene(0))).unwrap();
    e.backend_mut().vsync();
    drain(&mut e);
    let pending = e.present(&frame(&scene(1))).unwrap();

    e.backend_mut().lose_device();
    let events = drain(&mut e);
    assert_eq!(events.last(), Some(&DisplayEvent::DeviceLost));

    // While lost, presenting fails and commits nothing.
    let commits = e.backend().commit_count();
    assert_eq!(
        e.present(&frame(&scene(0))),
        Err(Error::Backend(BackendError::DeviceLost))
    );
    assert_eq!(e.backend().commit_count(), commits);

    // The display returns with the same topology.
    e.backend_mut().restore(caps_for(FULL_HD));
    assert_eq!(drain(&mut e), [DisplayEvent::CapabilitiesChanged]);
    let first_after = e.present(&frame(&scene(0))).unwrap();
    assert!(first_after > pending, "submission ids keep increasing");
    e.backend_mut().vsync();
    assert!(drain(&mut e).contains(&presented(OUT, first_after)));
    assert_eq!(
        e.backend().scanout(OUT).unwrap(),
        [
            PlaneState {
                plane: P34,
                buffer: bgs[0],
                zpos: 0
            },
            PlaneState {
                plane: P42,
                buffer: ovls[0],
                zpos: 1
            },
        ]
    );

    let log = &e.trace().0;
    let c_on = position(log, |x| *x == Ev::Commit(on_screen));
    let l_on = position(log, |x| *x == Ev::Latched(on_screen));
    let c_pend = position(log, |x| *x == Ev::Commit(pending));
    let lost = position(log, |x| *x == Ev::DeviceLost);
    let d_pend = position(log, |x| *x == Ev::Dropped(pending, DropReason::DeviceLost));
    let caps = position(log, |x| *x == Ev::CapabilitiesChanged);
    let c_after = position(log, |x| *x == Ev::Commit(first_after));
    let l_after = position(log, |x| *x == Ev::Latched(first_after));
    assert!(c_on < l_on && l_on < c_pend && c_pend < d_pend);
    assert!(
        lost < caps && d_pend < caps,
        "loss is reported before recovery"
    );
    assert!(caps < c_after && c_after < l_after);

    // Releases between the loss and the recovery: each buffer once, and the
    // dropped frame's buffers are among them.
    let released: Vec<BufferId> = log[lost.min(d_pend)..caps]
        .iter()
        .filter_map(|ev| match ev {
            Ev::Released(b, _) => Some(*b),
            _ => None,
        })
        .collect();
    let mut unique = released.clone();
    unique.sort_by_key(|b| b.0);
    unique.dedup();
    assert_eq!(unique.len(), released.len(), "no buffer released twice");
    for b in [bgs[1], ovls[1]] {
        assert!(released.contains(&b), "{b:?} released at the loss");
    }
}

/// A6: recovery in a different mode (the monitor came back at 1280x720) plans
/// on the same planes; the old 1920x1080 buffer is refused for SCALING.
#[test]
fn recovery_in_a_smaller_mode_plans_on_the_same_planes() {
    let mut e = engine(FULL_HD);
    let big = register(&mut e, 1, FULL_HD, XR24);
    let small = register(&mut e, 2, HD, XR24);
    e.present(&frame(&[layer(big, FULL_HD, full(FULL_HD), 0)]))
        .unwrap();
    e.backend_mut().vsync();
    e.backend_mut().lose_device();
    drain(&mut e);
    e.backend_mut().restore(caps_for(HD));
    assert_eq!(drain(&mut e), [DisplayEvent::CapabilitiesChanged]);

    let stale = [layer(big, FULL_HD, full(HD), 0)];
    assert_eq!(
        no_plane(e.validate(&frame(&stale)).unwrap_err()),
        (0, RejectReasons::SCALING)
    );
    let fresh = [layer(small, HD, full(HD), 0)];
    assert_eq!(placement(&mut e, &fresh), [(0, 34, 0)]);
    let s = e.present(&frame(&fresh)).unwrap();
    e.backend_mut().vsync();
    assert!(drain(&mut e).contains(&presented(OUT, s)));
}
