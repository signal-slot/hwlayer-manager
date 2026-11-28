//! Planner and fake-engine checks on two BCM2712 (Raspberry Pi 5, HVS6)
//! outputs that share the overlay pool.
//!
//! This is a MODEL, not a hardware result. Only one CRTC was active when the
//! topology was recorded, so the second active output is inferred from the
//! recorded masks: `docs/platform-capabilities.md` (observed 2026-11-28,
//! kernel 6.18.50+rpt-rpi-2712) lists four primaries (ids 48, 67, 83, 95, one
//! per CRTC, fixed zpos 0) and 16 overlays (ids 107, 118, ... 272, step 11,
//! zpos 1..=17) that reach CRTCs 1, 2 and 3. The test uses CRTC 1 (primary 67)
//! as output 0 and CRTC 2 (primary 83, the one that was active) as output 1.
//! The data is copied below; nothing reads docs/ at run time.
//!
//! Plane budget: the two primaries plus the 16 shared overlays are 18 planes
//! (the 34 CRTC-0-only planes and the per-CRTC extra planes are left out).
//! `MAX_PLANES` is 16, so the capabilities keep both primaries and 14 overlays
//! (107..=250); plan #457 saw one output reach 18 planes and keep 16 on a CM4.
//! Which overlays survive the real cut is inferred (lowest ids first).
//!
//! The planner and engine see one output's frame at a time. Nothing stops them
//! from giving the same shared overlay to both outputs; the kernel's
//! `TEST_ONLY` commit is the arbiter (see
//! `crates/composition-drm/tests/shared_planes.rs`). These tests pin that
//! behaviour; a later cross-output claim set will make them fail on purpose.
//! The fake backend has no kernel, so `arbiter_hook` stands in for it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use composition_core::limits::MAX_PLANES;
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, Engine, Error, FormatModifier, Frame, Layer,
    Modifier, OutputCaps, OutputId, PixelFormat, Plan, PlanError, PlaneCaps, PlaneId, PlaneKind,
    Rect, RejectReasons, Size, Zpos,
};
use composition_fake::FakeBackend;

const OUT0: OutputId = OutputId(0);
const OUT1: OutputId = OutputId(1);
const SCREEN: Size = Size::new(1920, 1080);
const FULL: Rect = Rect::new(0, 0, 1920, 1080);
const SMALL: Rect = Rect::new(0, 0, 64, 64);

/// Primary of CRTC 1 (output 0, mask bit 0) and of CRTC 2 (output 1, bit 1).
const PRIMARY0: PlaneId = PlaneId(67);
const PRIMARY1: PlaneId = PlaneId(83);
/// Overlays kept next to the two primaries by the `MAX_PLANES` cut.
const KEPT_OVERLAYS: u32 = MAX_PLANES as u32 - 2;

/// Shared overlay ids from the recorded list: 107, 118, ... (step 11).
fn overlay_id(n: u32) -> PlaneId {
    PlaneId(107 + 11 * n)
}

/// The recorded linear formats as the engine's view kept them (`MAX_FORMATS`
/// = 32, bcm2712-topology.md "Raw probe output"), in order.
const FORMATS: [&[u8; 4]; 32] = [
    b"XR24", b"AR24", b"AB24", b"XB24", b"RX24", b"RA24", b"BX24", b"BA24", b"RG16", b"BG16",
    b"AR15", b"XR15", b"RG24", b"BG24", b"YU16", b"YV16", b"YU24", b"YV24", b"YU12", b"YV12",
    b"NV12", b"NV21", b"NV16", b"NV61", b"XR30", b"AR30", b"AB30", b"XB30", b"RGB8", b"BGR8",
    b"XR12", b"AR12",
];

fn base_plane(id: PlaneId, kind: PlaneKind, zpos: Zpos, outputs: u32) -> PlaneCaps {
    let mut plane = PlaneCaps {
        id,
        kind,
        outputs,
        zpos,
        plane_alpha: true,
        blend_modes: BlendMode::Opaque.mask()
            | BlendMode::Premultiplied.mask()
            | BlendMode::Coverage.mask(),
        ..PlaneCaps::default()
    };
    for code in FORMATS {
        plane
            .formats
            .push(FormatModifier::linear(PixelFormat::fourcc(code)))
            .unwrap();
    }
    plane
}

fn primary(id: PlaneId, outputs: u32) -> PlaneCaps {
    base_plane(id, PlaneKind::Primary, Zpos::Fixed(0), outputs)
}

/// Shared overlay: reaches both modelled outputs, zpos 1..=17.
fn overlay(id: PlaneId) -> PlaneCaps {
    base_plane(
        id,
        PlaneKind::Overlay,
        Zpos::Range { min: 1, max: 17 },
        0b11,
    )
}

/// Both primaries and `overlays` shared overlays.
fn caps_with(overlays: u32) -> Capabilities {
    let mut caps = Capabilities::default();
    for id in [OUT0, OUT1] {
        caps.outputs
            .push(OutputCaps {
                id,
                size: SCREEN,
                refresh_mhz: 60_000,
            })
            .unwrap();
    }
    caps.planes.push(primary(PRIMARY0, 0b01)).unwrap();
    caps.planes.push(primary(PRIMARY1, 0b10)).unwrap();
    for n in 0..overlays {
        caps.planes.push(overlay(overlay_id(n))).unwrap();
    }
    caps
}

/// What the capabilities hold after the cut: both primaries, 14 overlays.
fn caps_after_cut() -> Capabilities {
    caps_with(KEPT_OVERLAYS)
}

fn engine(caps: Capabilities) -> Engine<FakeBackend> {
    Engine::new(FakeBackend::new(caps))
}

fn register(engine: &mut Engine<FakeBackend>, buffer: u32, size: Size) -> BufferId {
    let id = BufferId(buffer);
    engine
        .register_buffer(
            id,
            BufferInfo {
                size,
                format: PixelFormat::XRGB8888,
                modifier: Modifier::LINEAR,
            },
        )
        .unwrap();
    engine.finish_write(id).unwrap();
    id
}

/// A background plus `windows` small windows, bottom to top.
fn layers(bg: BufferId, win: BufferId, windows: usize) -> Vec<Layer> {
    let mut layers = vec![Layer::new(bg, FULL, FULL, 0)];
    for n in 0..windows {
        layers.push(Layer::new(
            win,
            SMALL,
            Rect::new(20 * n as i32, 20 * n as i32, 64, 64),
            n as i32 + 1,
        ));
    }
    layers
}

fn frame(output: OutputId, layers: &[Layer]) -> Frame<'_> {
    Frame { output, layers }
}

fn scanout_planes(engine: &Engine<FakeBackend>, output: OutputId) -> Vec<PlaneId> {
    engine
        .backend()
        .scanout(output)
        .expect("latched")
        .iter()
        .map(|s| s.plane)
        .collect()
}

fn plan_planes(plan: &Plan) -> Vec<PlaneId> {
    plan.assignments().iter().map(|a| a.plane).collect()
}

/// Stands in for the kernel's TEST_ONLY arbiter: refuses a frame that uses a
/// plane another output's last accepted frame holds.
fn arbiter_hook() -> impl FnMut(&Frame<'_>, &Plan) -> bool + 'static {
    let held: Rc<RefCell<HashMap<u32, Vec<PlaneId>>>> = Rc::default();
    move |frame, plan| {
        let mine = plan_planes(plan);
        let mut held = held.borrow_mut();
        let clash = held
            .iter()
            .any(|(out, planes)| *out != frame.output.0 && mine.iter().any(|p| planes.contains(p)));
        if !clash {
            held.insert(frame.output.0, mine);
        }
        !clash
    }
}

// The capabilities cannot hold the recorded reach: 2 primaries + 16 shared
// overlays are 18 planes, the push after the 16th fails (MAX_PLANES = 16).
#[test]
fn the_recorded_reach_of_two_outputs_exceeds_max_planes() {
    let mut caps = caps_with(0);
    let mut pushed = 2;
    for n in 0..16 {
        if caps.planes.push(overlay(overlay_id(n))).is_err() {
            break;
        }
        pushed += 1;
    }
    assert_eq!(pushed, MAX_PLANES);
    assert_eq!(2 + 16 - pushed, 2, "two recorded planes do not fit");
}

// Each output's own primary takes its background; the overlay pool is not
// partitioned, so both outputs get the SAME first overlay (107).
// Known cross-output blind spot: nothing claims 107 for output 0 only.
#[test]
fn both_outputs_plan_the_same_first_shared_overlay() {
    let mut engine = engine(caps_after_cut());
    let bg = register(&mut engine, 1, SCREEN);
    let win = register(&mut engine, 2, Size::new(64, 64));
    let ls = layers(bg, win, 1);
    for output in [OUT0, OUT1] {
        engine.present(&frame(output, &ls)).unwrap();
    }
    engine.backend_mut().vsync();
    assert_eq!(scanout_planes(&engine, OUT0), [PRIMARY0, overlay_id(0)]);
    assert_eq!(scanout_planes(&engine, OUT1), [PRIMARY1, overlay_id(0)]);
}

// Same with four windows: both outputs land on overlays 107..=140, so every
// overlay they use is double-booked. Pinned current behaviour.
#[test]
fn both_outputs_double_book_every_shared_overlay_they_use() {
    let mut engine = engine(caps_after_cut());
    let bg = register(&mut engine, 1, SCREEN);
    let win = register(&mut engine, 2, Size::new(64, 64));
    let ls = layers(bg, win, 4);
    for output in [OUT0, OUT1] {
        engine.present(&frame(output, &ls)).unwrap();
    }
    engine.backend_mut().vsync();
    let (a, b) = (scanout_planes(&engine, OUT0), scanout_planes(&engine, OUT1));
    let shared: Vec<_> = a.iter().filter(|p| b.contains(p)).collect();
    assert_eq!(shared.len(), 4);
    assert!(shared.iter().all(|p| (107..=140).contains(&p.0)));
}

// With the TEST_ONLY stand-in the second output's first candidate (overlay
// 107 again) is refused, and the engine's search over backend rejections moves
// it to the next free overlay. So the kernel arbiter, not the planner, is what
// keeps a plane out of both outputs, and it costs an extra check. No plane is
// in both scanouts.
#[test]
fn an_arbiter_keeps_a_plane_from_both_outputs_in_one_commit() {
    let mut engine = engine(caps_after_cut());
    engine.backend_mut().set_check_hook(arbiter_hook());
    let bg = register(&mut engine, 1, SCREEN);
    let win = register(&mut engine, 2, Size::new(64, 64));
    let ls = layers(bg, win, 1);
    engine.present(&frame(OUT0, &ls)).unwrap();
    assert_eq!(engine.backend().check_count(), 1);
    engine.present(&frame(OUT1, &ls)).unwrap();
    assert!(
        engine.backend().check_count() > 2,
        "a candidate was refused"
    );
    assert_eq!(engine.backend().commit_count(), 2);
    engine.backend_mut().vsync();
    let (a, b) = (scanout_planes(&engine, OUT0), scanout_planes(&engine, OUT1));
    assert_eq!(a, [PRIMARY0, overlay_id(0)]);
    assert_eq!(b[0], PRIMARY1);
    assert!(!b[1..].iter().any(|p| a.contains(p)));
}

// Per-output cut: an output reaches its primary plus 14 shared overlays = 15
// of the 16 planes (the other primary is masked to the other output), so 15
// layers fit and the 16th is refused with IN_USE. One output never gets the
// 16 planes MAX_PLANES allows.
#[test]
fn one_output_keeps_fifteen_of_the_sixteen_planes() {
    let mut engine = engine(caps_after_cut());
    let bg = register(&mut engine, 1, SCREEN);
    let win = register(&mut engine, 2, Size::new(64, 64));
    let fits = layers(bg, win, 14);
    assert_eq!(fits.len(), 15);
    for (output, own) in [(OUT0, PRIMARY0), (OUT1, PRIMARY1)] {
        let plan = engine.validate(&frame(output, &fits)).unwrap();
        let mut planes = plan_planes(&plan);
        assert!(planes.contains(&own));
        planes.sort_unstable();
        planes.dedup();
        assert_eq!(planes.len(), 15);
    }
    let too_many = layers(bg, win, 15);
    assert_eq!(too_many.len(), 16);
    for output in [OUT0, OUT1] {
        match engine.validate(&frame(output, &too_many)) {
            Err(Error::Plan(PlanError::NoPlane { layer, reasons })) => {
                assert_eq!(layer, 15);
                assert!(reasons.contains(RejectReasons::IN_USE));
            }
            other => panic!("expected NoPlane, got {other:?}"),
        }
    }
}

// A full 15-layer frame on each output uses the same 14 overlays twice: the
// shared pool is never split, so the per-output answers add up to 28 overlay
// uses of 14 planes. Pinned blind spot.
#[test]
fn full_frames_on_both_outputs_overbook_the_pool_twofold() {
    let mut engine = engine(caps_after_cut());
    let bg = register(&mut engine, 1, SCREEN);
    let win = register(&mut engine, 2, Size::new(64, 64));
    let ls = layers(bg, win, 14);
    let mut uses: HashMap<PlaneId, u32> = HashMap::new();
    for output in [OUT0, OUT1] {
        let plan = engine.validate(&frame(output, &ls)).unwrap();
        for plane in plan_planes(&plan) {
            *uses.entry(plane).or_default() += 1;
        }
    }
    assert!((0..KEPT_OVERLAYS).all(|n| uses[&overlay_id(n)] == 2));
    assert_eq!(uses[&PRIMARY0], 1);
    assert_eq!(uses[&PRIMARY1], 1);
}

// zpos is assigned per output: primary 0 (fixed), overlays inside 1..=17 and
// strictly rising, regardless of the other output.
#[test]
fn zpos_is_assigned_per_output_and_stays_in_range() {
    let mut engine = engine(caps_after_cut());
    let bg = register(&mut engine, 1, SCREEN);
    let win = register(&mut engine, 2, Size::new(64, 64));
    let ls = layers(bg, win, 3);
    for output in [OUT0, OUT1] {
        let plan = engine.validate(&frame(output, &ls)).unwrap();
        let zs: Vec<_> = plan.assignments().iter().map(|a| a.zpos).collect();
        assert_eq!(zs[0], 0);
        assert!(zs[1..].iter().all(|z| (1..=17).contains(z)));
        assert!(zs.windows(2).all(|w| w[0] < w[1]));
    }
}
