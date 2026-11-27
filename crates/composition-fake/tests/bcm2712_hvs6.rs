//! Planner checks on the full observed BCM2712 (Raspberry Pi 5, HVS6) plane
//! topology, with the per-output reach and the plane cut of `composition-drm`.
//!
//! A MODEL, not a hardware result. Data copied from
//! `docs/observed/bcm2712-topology.md` and `docs/observed/bcm2712.md`
//! (observed 2026-11-28, kernel 6.18.50+rpt-rpi-2712); nothing is read at
//! runtime. `topology_bcm2712.rs` models the engine's old 16-plane view; this
//! file models the kernel's 56 planes and the reachable-first cut of
//! `keep_planes` (crates/composition-drm/src/lib.rs, plan #457 fix-x09),
//! reimplemented below because it is private.
//!
//! Observed: 4 CRTCs (indices 0..3); primaries 48, 67, 83, 95, one per CRTC;
//! 16 shared overlays 107, 118, .. 272 (step 11) on CRTCs 1, 2, 3, zpos
//! 1..=17; one extra plane per CRTC 1/2/3 (646, 657, 668); 34 planes on CRTC
//! 0 only (including primary 48), types not read; 56 planes in all; every
//! probed plane lists the same 32 linear formats after the `MAX_FORMATS`
//! cut; scaling not claimed, TEST_ONLY accepted 2x up and 1/2 down.
//! Inferred, not observed: the extra planes are cursors; the 33 other CRTC-0
//! planes are overlays with ids 283 + 11 * n (n = 0..32): the kernel lists
//! planes by id, ids step by 11, 272 is the last shared overlay, 646 the first
//! extra plane, and 283..=635 is exactly 33 ids.

use composition_core::limits::MAX_PLANES;
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, Engine, Error, FormatModifier, Frame, Layer,
    Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, ScaleLimits, Size, Zpos,
};
use composition_fake::FakeBackend;

const SCREEN: Size = Size::new(1920, 1080);

/// The 32 linear formats of the probe (`MAX_FORMATS` cut of the 45 listed).
const FORMATS: [&[u8; 4]; 32] = [
    b"XR24", b"AR24", b"AB24", b"XB24", b"RX24", b"RA24", b"BX24", b"BA24", b"RG16", b"BG16",
    b"AR15", b"XR15", b"RG24", b"BG24", b"YU16", b"YV16", b"YU24", b"YV24", b"YU12", b"YV12",
    b"NV12", b"NV21", b"NV16", b"NV61", b"XR30", b"AR30", b"AB30", b"XB30", b"RGB8", b"BGR8",
    b"XR12", b"AR12",
];

/// A kernel plane: id, kind, CRTC-index bit mask.
type Kernel = (u32, PlaneKind, u32);

/// The 56 kernel planes.
fn kernel_planes() -> Vec<Kernel> {
    let mut v: Vec<Kernel> = vec![(48, PlaneKind::Primary, 1 << 0)];
    v.push((67, PlaneKind::Primary, 1 << 1));
    v.push((83, PlaneKind::Primary, 1 << 2));
    v.push((95, PlaneKind::Primary, 1 << 3));
    v.extend((0..16).map(|n| (107 + 11 * n, PlaneKind::Overlay, 0b1110)));
    // 33 more CRTC-0-only planes (ids and types inferred), after the shared
    // overlays as the kernel lists by id.
    v.extend((0..33).map(|n| (283 + 11 * n, PlaneKind::Overlay, 1 << 0)));
    v.push((646, PlaneKind::Cursor, 1 << 1));
    v.push((657, PlaneKind::Cursor, 1 << 2));
    v.push((668, PlaneKind::Cursor, 1 << 3));
    v
}

/// Model of composition-drm `keep_planes`: reachable first, primaries, then
/// cursors, then overlays under the cut; survivors ordered reachable, primary,
/// overlay, cursor. Stable sorts.
fn keep(mut planes: Vec<Kernel>, reach: impl Fn(&Kernel) -> bool) -> Vec<Kernel> {
    let prio = |k: PlaneKind| match k {
        PlaneKind::Primary => 0,
        PlaneKind::Cursor => 1,
        PlaneKind::Overlay => 2,
    };
    planes.sort_by_key(|p| (!reach(p), prio(p.1)));
    planes.truncate(MAX_PLANES);
    let order = |k: PlaneKind| match k {
        PlaneKind::Primary => 0,
        PlaneKind::Overlay => 1,
        PlaneKind::Cursor => 2,
    };
    planes.sort_by_key(|p| (!reach(p), order(p.1)));
    planes
}

fn plane_caps(id: u32, kind: PlaneKind, outputs: u32) -> PlaneCaps {
    let mut plane = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs,
        zpos: match kind {
            PlaneKind::Overlay => Zpos::Range { min: 1, max: 17 },
            _ => Zpos::Fixed(0),
        },
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

/// Engine view with the given CRTC indices probed as outputs 0.. in order.
fn view(crtcs: &[u32], scaling: Option<ScaleLimits>) -> Capabilities {
    let mask_of = |p: &Kernel| {
        crtcs
            .iter()
            .enumerate()
            .filter(|(_, c)| p.2 & (1 << **c) != 0)
            .fold(0u32, |m, (i, _)| m | 1 << i)
    };
    let kept = keep(kernel_planes(), |p| mask_of(p) != 0);
    let mut caps = Capabilities::default();
    for i in 0..crtcs.len() {
        caps.outputs
            .push(OutputCaps {
                id: OutputId(i as u32),
                size: SCREEN,
                refresh_mhz: 60_000,
            })
            .unwrap();
    }
    for p in &kept {
        let mut plane = plane_caps(p.0, p.1, mask_of(p));
        plane.scaling = scaling;
        caps.planes.push(plane).unwrap();
    }
    caps
}

/// What TEST_ONLY accepted on the board: 2x up and 1/2 down.
const OBSERVED_SCALING: ScaleLimits = ScaleLimits {
    max_upscale: 2 << 16,
    max_downscale: 2 << 16,
};

fn engine(caps: Capabilities) -> Engine<FakeBackend> {
    Engine::new(FakeBackend::new(caps))
}

fn register(e: &mut Engine<FakeBackend>, n: u32, size: Size, format: PixelFormat) -> BufferId {
    let id = BufferId(n);
    e.register_buffer(
        id,
        BufferInfo {
            size,
            format,
            modifier: Modifier::LINEAR,
        },
    )
    .unwrap();
    e.finish_write(id).unwrap();
    id
}

fn small(e: &mut Engine<FakeBackend>, n: u32) -> BufferId {
    register(e, n, Size::new(64, 64), PixelFormat::XRGB8888)
}

fn tile(b: BufferId, n: i32) -> Layer {
    Layer::new(
        b,
        Rect::new(0, 0, 64, 64),
        Rect::new(20 * n, 20 * n, 64, 64),
        n,
    )
}

fn tiles(e: &mut Engine<FakeBackend>, count: u32) -> Vec<Layer> {
    (0..count)
        .map(|n| {
            let b = small(e, 1 + n);
            tile(b, n as i32)
        })
        .collect()
}

fn planes_of(e: &mut Engine<FakeBackend>, output: u32, layers: &[Layer]) -> Vec<u32> {
    let plan = e
        .validate(&Frame {
            output: OutputId(output),
            layers,
        })
        .unwrap();
    plan.assignments().iter().map(|a| a.plane.0).collect()
}

fn refusal(e: &mut Engine<FakeBackend>, layers: &[Layer]) -> (usize, RejectReasons) {
    match e.validate(&Frame {
        output: OutputId(0),
        layers,
    }) {
        Err(Error::Plan(PlanError::NoPlane { layer, reasons })) => (layer, reasons),
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

#[test]
fn kernel_table_has_the_observed_56_planes() {
    assert_eq!(kernel_planes().len(), 56);
}

// Per-output reach: CRTC 2 (HDMI-A-1) reaches 83, 16 overlays and 657 = 18
// planes; the cut keeps 16, primary and cursor first, then overlays by id.
#[test]
fn crtc2_reaches_18_planes_and_the_cut_keeps_16() {
    let caps = view(&[2], None);
    assert_eq!(caps.planes.len(), MAX_PLANES);
    let ids: Vec<u32> = caps.planes.iter().map(|p| p.id.0).collect();
    assert_eq!(ids[0], 83);
    assert_eq!(*ids.last().unwrap(), 657);
    let overlays: Vec<u32> = (0..14).map(|n| 107 + 11 * n).collect();
    assert_eq!(&ids[1..15], &overlays[..]);
    assert!(caps.planes.iter().all(|p| p.outputs == 1));
}

// With four outputs probed every plane is reachable; primaries are kept
// first, one per CRTC, and each primary reaches only its own output.
#[test]
fn four_outputs_keep_the_four_primaries_and_their_own_reach() {
    let caps = view(&[0, 1, 2, 3], None);
    let primaries: Vec<(u32, u32)> = caps
        .planes
        .iter()
        .filter(|p| p.kind == PlaneKind::Primary)
        .map(|p| (p.id.0, p.outputs))
        .collect();
    assert_eq!(
        primaries,
        [(48, 0b1), (67, 0b10), (83, 0b100), (95, 0b1000)]
    );
    // The cut keeps 4 primaries, 3 cursors and the first 9 overlays by id,
    // all of them shared ones; none of the CRTC-0-only overlays survive.
    assert_eq!(caps.planes.len(), MAX_PLANES);
    let overlays: Vec<(u32, u32)> = caps
        .planes
        .iter()
        .filter(|p| p.kind == PlaneKind::Overlay)
        .map(|p| (p.id.0, p.outputs))
        .collect();
    let expected: Vec<(u32, u32)> = (0..9).map(|n| (107 + 11 * n, 0b1110)).collect();
    assert_eq!(overlays, expected);
}

// The planner plans each output on planes that reach it.
#[test]
fn each_output_is_planned_on_planes_that_reach_it() {
    // CRTCs 0, 1 and 2: 3 primaries, 2 cursors and the shared overlays by id
    // fill the cut, so CRTC 0's own overlays (higher ids) are not kept and
    // output 0 is left with its primary alone (a model result of the cut, not
    // an observation). Outputs 1 and 2 have the shared overlays.
    let caps = view(&[0, 1, 2], None);
    let reaching = |bit: u32| caps.planes.iter().filter(|p| p.outputs & bit != 0).count();
    assert_eq!(reaching(0b001), 1);
    assert!(reaching(0b010) > 3 && reaching(0b100) > 3);
    let mut e = engine(caps);
    let layers = tiles(&mut e, 3);
    for (out, crtc, count) in [(0u32, 0u32, 1usize), (1, 1, 3), (2, 2, 3)] {
        let ids = planes_of(&mut e, out, &layers[..count]);
        assert_eq!(ids.len(), count);
        for id in ids {
            let (_, _, mask) = *kernel_planes().iter().find(|p| p.0 == id).unwrap();
            assert!(
                mask & (1 << crtc) != 0,
                "plane {id} cannot reach crtc {crtc}"
            );
        }
    }
    let (index, _) = match e.validate(&Frame {
        output: OutputId(0),
        layers: &layers[..2],
    }) {
        Err(Error::Plan(PlanError::NoPlane { layer, reasons })) => (layer, reasons),
        other => panic!("expected NoPlane, got {other:?}"),
    };
    assert_eq!(index, 1);
}

// Plane budget: 16 planes are kept on CRTC 2, one of them the cursor 657.
// Pins how many layers fit.
#[test]
fn plane_budget_on_crtc2() {
    let mut e = engine(view(&[2], None));
    let layers = tiles(&mut e, 16);
    let ids = planes_of(&mut e, 0, &layers[..15]);
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 15);
    assert!(!ids.contains(&657), "the cursor plane carries no layer");
    // The 16th layer has no plane; the planner runs out of search steps first.
    let result = e.validate(&Frame {
        output: OutputId(0),
        layers: &layers,
    });
    assert!(
        matches!(result, Err(Error::Plan(PlanError::LimitExceeded))),
        "{result:?}"
    );
}

// Scaled windows: refused unless scaling is claimed; with the observed
// TEST_ONLY range (2x up, 1/2 down) accepted, and 3x refused.
#[test]
fn scaled_windows_follow_the_claimed_range() {
    let scaled = |e: &mut Engine<FakeBackend>, src: Size, dst: Rect| {
        let bg = small(e, 1);
        let ovl = register(e, 2, src, PixelFormat::ARGB8888);
        [
            Layer::new(bg, Rect::new(0, 0, 64, 64), Rect::new(0, 0, 64, 64), 0),
            Layer::new(ovl, Rect::new(0, 0, src.width, src.height), dst, 1),
        ]
    };
    let dst = Rect::new(960, 270, 384, 540);
    let mut none = engine(view(&[2], None));
    let l = scaled(&mut none, Size::new(192, 270), dst);
    let (index, reasons) = refusal(&mut none, &l);
    assert_eq!(index, 1);
    assert!(reasons.contains(RejectReasons::SCALING));

    for src in [Size::new(192, 270), Size::new(768, 1080)] {
        let mut e = engine(view(&[2], Some(OBSERVED_SCALING)));
        let l = scaled(&mut e, src, dst);
        planes_of(&mut e, 0, &l);
    }
    let mut e = engine(view(&[2], Some(OBSERVED_SCALING)));
    let three = scaled(&mut e, Size::new(128, 180), dst);
    let (index, reasons) = refusal(&mut e, &three);
    assert_eq!(index, 1);
    assert!(reasons.contains(RejectReasons::SCALING));
}

// YUV windows: NV12, YU12 and NV16 are among the 32 formats the engine keeps;
// P010 is in the kernel's list but behind the MAX_FORMATS cut, so it has no
// plane.
#[test]
fn yuv_windows_use_listed_formats_only() {
    for (fmt, ok) in [
        (PixelFormat::fourcc(b"NV12"), true),
        (PixelFormat::YUV420, true),
        (PixelFormat::fourcc(b"NV16"), true),
        (PixelFormat::fourcc(b"P010"), false),
    ] {
        let mut e = engine(view(&[2], None));
        let bg = small(&mut e, 1);
        let y = register(&mut e, 2, Size::new(640, 360), fmt);
        let layers = [
            Layer::new(bg, Rect::new(0, 0, 64, 64), Rect::new(0, 0, 64, 64), 0),
            Layer::new(
                y,
                Rect::new(0, 0, 640, 360),
                Rect::new(100, 100, 640, 360),
                1,
            ),
        ];
        if ok {
            let ids = planes_of(&mut e, 0, &layers);
            assert_ne!(ids[0], ids[1]);
        } else {
            let (index, _) = refusal(&mut e, &layers);
            assert_eq!(index, 1);
        }
    }
}

// A scaled YUV window combines both rules.
#[test]
fn scaled_yuv_window_is_planned_with_the_claimed_range() {
    let mut e = engine(view(&[2], Some(OBSERVED_SCALING)));
    let bg = small(&mut e, 1);
    let y = register(&mut e, 2, Size::new(960, 540), PixelFormat::fourcc(b"NV12"));
    let layers = [
        Layer::new(bg, Rect::new(0, 0, 64, 64), Rect::new(0, 0, 64, 64), 0),
        Layer::new(y, Rect::new(0, 0, 960, 540), Rect::new(0, 0, 1920, 1080), 1),
    ];
    let ids = planes_of(&mut e, 0, &layers);
    assert_ne!(ids[0], ids[1]);
}

// Determinism: the view and the plan are the same on every run, and the
// bottom layer lands on the primary.
#[test]
fn view_and_plan_are_deterministic() {
    let ids = |caps: &Capabilities| caps.planes.iter().map(|p| p.id.0).collect::<Vec<_>>();
    assert_eq!(ids(&view(&[2], None)), ids(&view(&[2], None)));
    let run = || {
        let mut e = engine(view(&[2], Some(OBSERVED_SCALING)));
        let layers = tiles(&mut e, 8);
        let plan = e
            .validate(&Frame {
                output: OutputId(0),
                layers: &layers,
            })
            .unwrap();
        plan.assignments()
            .iter()
            .map(|a| (a.layer, a.plane.0, a.zpos))
            .collect::<Vec<_>>()
    };
    let first = run();
    assert_eq!(first, run());
    assert_eq!(first.len(), 8);
    assert_eq!(first[0].1, 83, "the bottom layer lands on the primary");
}
