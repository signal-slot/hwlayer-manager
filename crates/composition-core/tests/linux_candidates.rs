//! The Linux DRM candidates of the RK3588 and Raspberry Pi boards as
//! `Capabilities`, run through the planner on typical scenes. Host test
//! only: nothing here is a hardware result, and no board was used.
//!
//! Candidates, each built as composition-drm's discovery hands it to the
//! core (one connected output, `MAX_PLANES` and `MAX_FORMATS` applied), from
//! the repository's recordings:
//!
//! - `rk3588_vop2`: eight planes, 1920x1080. Cluster0..3 and Esmart0..3,
//!   zpos `0..=7` on all, plane alpha, three blend modes. Cluster planes list
//!   AFBC pairs only (no linear), Esmart planes 21 linear formats. Scaling
//!   not claimed, transforms none (as the engine reads them). Source:
//!   `docs/observed/rk3588-topology.md` (observed 2026-11-28).
//! - `bcm2711_hvs5`: the 16 planes the engine kept of 60 (6 primaries, 10
//!   overlays), 720x1280 on the DSI CRTC, of which 11 reach the output (primary
//!   plane 79 and the ten overlays; the other primaries show mask `0b0`).
//!   Source: `docs/observed/bcm2711-cm4-topology.md` (observed 2026-11-28).
//! - `bcm2712_hvs6`: the 16 planes kept of 56 (4 primaries, 12 overlays),
//!   1920x1080 on the connected CRTC. Source:
//!   `docs/observed/bcm2712-topology.md` (observed 2026-11-28).
//!
//! OPEN OWNER DECISION: `MAX_PLANES` (16) and `MAX_FORMATS` (32) are smaller
//! than the hardware. The models below contain only what fits: the HVS
//! boards lose 44 and 40 planes (the writeback overlays, the cursors, the
//! last shared overlays), the RK3588 Cluster planes keep 32 of their 126
//! pairs, and the HVS planes keep 32 of 34 or 45 linear formats and none
//! of their non-linear pairs. Whether to raise the caps is the owner's
//! decision (`docs/rk-hvs-caps-finding.md`); these tests do not change the
//! constants and pin only what the planner does with the cut model.
//!
//! The planner has no fallback composition (`planner.rs` module comment): a
//! frame maps completely onto planes or is refused. "More layers than
//! planes must fall back to composition" is therefore asserted as the
//! planner's refusal (`PlanError::NoPlane`) that `advise` turns into a
//! composition fallback; `advise` is not used here.
//!
//! Where the planner may choose among equal planes the tests assert
//! relations (distinct planes, order of zpos, plane class), not the plane
//! the depth-first search happens to pick first. `PlaneKind` is not read by
//! the planner, so a cursor is just a small layer on top.

#![forbid(unsafe_code)]

use composition_core::limits::{MAX_FORMATS, MAX_LAYERS, MAX_PLANES};
use composition_core::planner::plan;
use composition_core::{
    BlendMode, BufferId, BufferInfo, BufferLookup, Capabilities, FormatModifier, Frame, Layer,
    Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, Size, Transform, Zpos,
};

const OUT: OutputId = OutputId(0);

/// Arm AFBC modifiers read from the recording (observed 2026-11-28); the
/// exact flags are inferred from the encoding, only the values matter here.
const AFBC_A: Modifier = Modifier(0x800_0000_0000_0001);
const AFBC_B: Modifier = Modifier(0x800_0000_0000_0041);
const AFBC_C: Modifier = Modifier(0x800_0000_0000_0011);

fn f(code: &str) -> PixelFormat {
    let b = code.as_bytes();
    PixelFormat::fourcc(&[b[0], b[1], b[2], b[3]])
}

struct Buffers(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

fn buf(id: u32, w: u32, h: u32, format: PixelFormat, modifier: Modifier) -> (BufferId, BufferInfo) {
    (
        BufferId(id),
        BufferInfo {
            size: Size::new(w, h),
            format,
            modifier,
        },
    )
}

const ALL_BLENDS: u8 = (1 << BlendMode::Opaque as u8)
    | (1 << BlendMode::Coverage as u8)
    | (1 << BlendMode::Premultiplied as u8);

fn plane(
    id: u32,
    kind: PlaneKind,
    outputs: u32,
    zpos: Zpos,
    formats: &[FormatModifier],
) -> PlaneCaps {
    let mut p = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs,
        scaling: None,
        plane_alpha: true,
        blend_modes: ALL_BLENDS,
        transforms: Transform::Normal.mask(),
        zpos,
        ..PlaneCaps::default()
    };
    for fm in formats {
        p.formats.push(*fm).unwrap();
    }
    assert!(p.formats.len() <= MAX_FORMATS);
    p
}

struct Candidate {
    caps: Capabilities,
    size: Size,
}

fn candidate(size: Size, planes: &[PlaneCaps]) -> Candidate {
    assert!(planes.len() <= MAX_PLANES);
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size,
            refresh_mhz: 60_000,
        })
        .unwrap();
    for p in planes {
        caps.planes.push(*p).unwrap();
    }
    Candidate { caps, size }
}

// ---------------------------------------------------------------------------
// Candidates
// ---------------------------------------------------------------------------

/// Plane ids of the RK3588 Esmart class and of the Cluster class that reach
/// the connected output (Cluster1, id 39, shows mask `0b0`: its own port is
/// not connected).
const ESMART: [u32; 4] = [57, 63, 69, 75];
const CLUSTER_REACHING: [u32; 3] = [33, 45, 51];

/// RK3588 VOP2, observed 2026-11-28 (kernel 6.18.55). The Cluster list is
/// the first 32 pairs of the 126 (14 formats x 9 AFBC modifiers), the cut the
/// engine applies: 14 pairs of the first modifier, 14 of the second, 4 of the
/// third. There is no linear pair on Cluster. The Esmart list is the 21
/// recorded linear formats.
fn rk3588_vop2() -> Candidate {
    let cluster_formats = [
        "XR30", "XB30", "XR24", "AR24", "XB24", "AB24", "RG24", "BG24", "RG16", "BG16", "YU08",
        "YU10", "YUYV", "Y210",
    ];
    let mut cluster = Vec::new();
    for m in [AFBC_A, AFBC_B] {
        for c in cluster_formats {
            cluster.push(FormatModifier {
                format: f(c),
                modifier: m,
            });
        }
    }
    for c in &cluster_formats[..4] {
        cluster.push(FormatModifier {
            format: f(c),
            modifier: AFBC_C,
        });
    }
    assert_eq!(cluster.len(), 32);
    let esmart: Vec<FormatModifier> = [
        "XR24", "AR24", "XB24", "AB24", "RG24", "BG24", "RG16", "BG16", "NV12", "NV21", "NV16",
        "NV61", "NV20", "NV24", "NV42", "NV30", "NV15", "YVYU", "VYUY", "YUYV", "UYVY",
    ]
    .iter()
    .map(|c| FormatModifier::linear(f(c)))
    .collect();
    assert_eq!(esmart.len(), 21);
    let z = Zpos::Range { min: 0, max: 7 };
    candidate(
        Size::new(1920, 1080),
        &[
            plane(33, PlaneKind::Primary, 0b1, z, &cluster),
            plane(39, PlaneKind::Primary, 0b0, z, &cluster),
            plane(45, PlaneKind::Overlay, 0b1, z, &cluster),
            plane(51, PlaneKind::Overlay, 0b1, z, &cluster),
            plane(57, PlaneKind::Overlay, 0b1, z, &esmart),
            plane(63, PlaneKind::Overlay, 0b1, z, &esmart),
            plane(69, PlaneKind::Overlay, 0b1, z, &esmart),
            plane(75, PlaneKind::Overlay, 0b1, z, &esmart),
        ],
    )
}

/// The 32 linear formats the engine kept on BCM2711 (observed 2026-11-28).
const HVS5_FORMATS: [&str; 32] = [
    "XR24", "AR24", "AB24", "XB24", "RG16", "BG16", "AR15", "XR15", "RG24", "BG24", "YU16", "YV16",
    "YU24", "YV24", "YU12", "YV12", "NV12", "NV21", "NV16", "NV61", "XR30", "AR30", "AB30", "XB30",
    "RGB8", "BGR8", "XR12", "AR12", "XB12", "AB12", "BX12", "BA12",
];

/// The 32 linear formats the engine kept on BCM2712 (observed 2026-11-28);
/// the 13 dropped ones include P010, P012, P016 and the S01x formats.
const HVS6_FORMATS: [&str; 32] = [
    "XR24", "AR24", "AB24", "XB24", "RX24", "RA24", "BX24", "BA24", "RG16", "BG16", "AR15", "XR15",
    "RG24", "BG24", "YU16", "YV16", "YU24", "YV24", "YU12", "YV12", "NV12", "NV21", "NV16", "NV61",
    "XR30", "AR30", "AB30", "XB30", "RGB8", "BGR8", "XR12", "AR12",
];

/// Primaries `(id, outputs)` then `count` overlays from `first_overlay`, 11
/// apart (the recorded ids), all reaching the output.
fn hvs(
    size: Size,
    primaries: &[(u32, u32)],
    first_overlay: u32,
    overlays: u32,
    fmts: &[&str],
) -> Candidate {
    let formats: Vec<FormatModifier> = fmts.iter().map(|c| FormatModifier::linear(f(c))).collect();
    let mut planes: Vec<PlaneCaps> = primaries
        .iter()
        .map(|&(id, outputs)| plane(id, PlaneKind::Primary, outputs, Zpos::Fixed(0), &formats))
        .collect();
    for i in 0..overlays {
        planes.push(plane(
            first_overlay + 11 * i,
            PlaneKind::Overlay,
            0b1,
            Zpos::Range { min: 1, max: 17 },
            &formats,
        ));
    }
    candidate(size, &planes)
}

/// BCM2711 HVS5 on the reTerminal's DSI panel: six primaries of which only
/// plane 79 reaches the connected CRTC, then ten overlays (127..226).
/// The cursor planes are not in the kept 16.
fn bcm2711_hvs5() -> Candidate {
    hvs(
        Size::new(720, 1280),
        &[(48, 0), (67, 0), (79, 1), (91, 0), (103, 0), (115, 0)],
        127,
        10,
        &HVS5_FORMATS,
    )
}

/// BCM2712 HVS6 on the Pi 5's HDMI output: four primaries of which only
/// plane 83 reaches the connected CRTC, then twelve overlays (107..238).
fn bcm2712_hvs6() -> Candidate {
    hvs(
        Size::new(1920, 1080),
        &[(48, 0), (67, 0), (83, 1), (95, 0)],
        107,
        12,
        &HVS6_FORMATS,
    )
}

// ---------------------------------------------------------------------------
// Scene helpers
// ---------------------------------------------------------------------------

struct Scene {
    layers: Vec<Layer>,
    buffers: Buffers,
}

fn full(c: &Candidate) -> Rect {
    Rect::new(0, 0, c.size.width, c.size.height)
}

/// Full-screen video (`video` format) below a full-screen coverage UI.
fn video_ui(c: &Candidate, video: PixelFormat) -> Scene {
    let r = full(c);
    let (w, h) = (c.size.width, c.size.height);
    Scene {
        layers: vec![
            Layer::new(BufferId(1), r, r, 0),
            Layer {
                blend: BlendMode::Coverage,
                ..Layer::new(BufferId(2), r, r, 1)
            },
        ],
        buffers: Buffers(vec![
            buf(1, w, h, video, Modifier::LINEAR),
            buf(2, w, h, f("AR24"), Modifier::LINEAR),
        ]),
    }
}

/// `video_ui` plus a 64x64 coverage pointer image on top.
fn video_ui_cursor(c: &Candidate, video: PixelFormat) -> Scene {
    let mut s = video_ui(c, video);
    s.layers.push(Layer {
        blend: BlendMode::Coverage,
        ..Layer::new(
            BufferId(3),
            Rect::new(0, 0, 64, 64),
            Rect::new(100, 100, 64, 64),
            2,
        )
    });
    s.buffers
        .0
        .push(buf(3, 64, 64, f("AR24"), Modifier::LINEAR));
    s
}

/// `n` small coverage layers, linear ARGB8888, at distinct places, bottom to
/// top.
fn n_layers(n: usize) -> Scene {
    let mut layers = Vec::new();
    let mut bufs = Vec::new();
    for i in 0..n {
        layers.push(Layer {
            blend: BlendMode::Coverage,
            ..Layer::new(
                BufferId(i as u32 + 1),
                Rect::new(0, 0, 32, 32),
                Rect::new(10 + 40 * i as i32, 10, 32, 32),
                i as i32,
            )
        });
        bufs.push(buf(i as u32 + 1, 32, 32, f("AR24"), Modifier::LINEAR));
    }
    Scene {
        layers,
        buffers: Buffers(bufs),
    }
}

/// `(layer index, plane id, zpos)` of each assignment.
fn planned(c: &Candidate, s: &Scene) -> Result<Vec<(usize, u32, i32)>, PlanError> {
    let frame = Frame {
        output: OUT,
        layers: &s.layers,
    };
    plan(&frame, &c.caps, &s.buffers).map(|p| {
        p.assignments()
            .iter()
            .map(|a| (a.layer, a.plane.0, a.zpos))
            .collect()
    })
}

/// The planner's own rules: every layer on its own plane, plane ids unique,
/// and a layer higher in the stack has a larger zpos.
fn assert_stack_is_ordered(a: &[(usize, u32, i32)], layers: usize) {
    assert_eq!(a.len(), layers);
    for (i, x) in a.iter().enumerate() {
        assert_eq!(x.0, i, "assignments are ordered bottom to top");
        for y in &a[i + 1..] {
            assert_ne!(x.1, y.1, "a plane takes one layer");
            assert!(x.2 < y.2, "zpos follows the stack: {a:?}");
        }
    }
}

fn refused(r: Result<Vec<(usize, u32, i32)>, PlanError>) -> (usize, RejectReasons) {
    match r {
        Err(PlanError::NoPlane { layer, reasons }) => (layer, reasons),
        other => panic!("expected NoPlane, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// RK3588 VOP2
// ---------------------------------------------------------------------------

#[test]
fn rk3588_model_is_the_cut_view() {
    let c = rk3588_vop2();
    // 8 planes fit MAX_PLANES; nothing is hidden.
    assert_eq!(c.caps.planes.len(), 8);
    // Cluster planes show 32 of 126 pairs and no linear one; Esmart is whole.
    for p in c.caps.planes.iter() {
        let expected = if ESMART.contains(&p.id.0) { 21 } else { 32 };
        assert_eq!(p.formats.len(), expected);
    }
    let cluster = &c.caps.planes[0];
    assert!(cluster
        .formats
        .iter()
        .all(|x| x.modifier != Modifier::LINEAR));
}

#[test]
fn rk3588_video_with_ui_overlay_uses_two_esmart_planes() {
    // Linear NV12 is only on Esmart (the Cluster list has no linear pair and
    // no NV12); the linear ARGB UI is only on Esmart as well. Two layers
    // need two distinct planes.
    let c = rk3588_vop2();
    let s = video_ui(&c, PixelFormat::NV12);
    let a = planned(&c, &s).unwrap();
    assert_stack_is_ordered(&a, 2);
    assert!(a.iter().all(|x| ESMART.contains(&x.1)), "{a:?}");
    // Every plane is zpos 0..=7: the planner programs a position in range.
    assert!(a.iter().all(|x| (0..=7).contains(&x.2)));
}

#[test]
fn rk3588_afbc_ui_lands_on_cluster_and_linear_ui_on_esmart() {
    let c = rk3588_vop2();
    let mut s = video_ui(&c, PixelFormat::NV12);
    // Swap the UI for an AFBC ARGB buffer: only Cluster lists that pair.
    s.buffers.0[1] = buf(2, 1920, 1080, f("AR24"), AFBC_A);
    let a = planned(&c, &s).unwrap();
    assert_stack_is_ordered(&a, 2);
    assert!(ESMART.contains(&a[0].1), "video on Esmart: {a:?}");
    assert!(
        CLUSTER_REACHING.contains(&a[1].1),
        "AFBC UI on Cluster: {a:?}"
    );
}

#[test]
fn rk3588_cursor_is_a_third_plane_on_top() {
    // The recording has no cursor plane (observed 2026-11-28), so the
    // pointer is a third Esmart layer.
    let c = rk3588_vop2();
    let s = video_ui_cursor(&c, PixelFormat::NV12);
    let a = planned(&c, &s).unwrap();
    assert_stack_is_ordered(&a, 3);
    assert!(a.iter().all(|x| ESMART.contains(&x.1)), "{a:?}");
}

#[test]
fn rk3588_more_linear_layers_than_linear_planes_is_refused() {
    // Four Esmart planes take linear ARGB; Cluster has no linear pair. Four
    // layers fit, a fifth does not, although seven planes reach the output.
    // `advise` would compose the excess; the planner refuses.
    let c = rk3588_vop2();
    let a = planned(&c, &n_layers(4)).unwrap();
    assert_stack_is_ordered(&a, 4);
    let (layer, reasons) = refused(planned(&c, &n_layers(5)));
    assert_eq!(layer, 4);
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
}

#[test]
fn rk3588_afbc_layers_fill_the_cluster_planes_too() {
    // Seven planes reach the output; with AFBC ARGB buffers three of them
    // (Cluster0, 2, 3) take a layer, and the Esmart planes refuse the
    // modifier: the fourth AFBC layer is refused on format.
    let c = rk3588_vop2();
    let mut s = n_layers(4);
    for i in 0..4u32 {
        s.buffers.0[i as usize] = buf(i + 1, 32, 32, f("AR24"), AFBC_A);
    }
    let (layer, reasons) = refused(planned(&c, &s));
    assert_eq!(layer, 3);
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
}

#[test]
fn rk3588_unsupported_format_is_refused() {
    // XRGB2101010 linear: Cluster lists it with AFBC only, Esmart not at all.
    let c = rk3588_vop2();
    let mut s = video_ui(&c, PixelFormat::NV12);
    s.buffers.0[0] = buf(1, 1920, 1080, f("XR30"), Modifier::LINEAR);
    let (layer, reasons) = refused(planned(&c, &s));
    assert_eq!(layer, 0);
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
}

#[test]
fn rk3588_pairs_past_the_format_cap_are_refused() {
    // The kernel lists RG24 with the third AFBC modifier (inferred from the
    // 14 x 9 layout), but the engine's 32-pair cut ends after the fourth
    // format of that modifier, so the model refuses it. This is the effect
    // of the open MAX_FORMATS decision, not a hardware statement.
    let c = rk3588_vop2();
    let mut s = n_layers(1);
    s.buffers.0[0] = buf(1, 32, 32, f("RG24"), AFBC_C);
    let (layer, reasons) = refused(planned(&c, &s));
    assert_eq!(layer, 0);
    assert!(reasons.contains(RejectReasons::FORMAT), "{reasons:?}");
    // The AR24 pair of the same modifier is inside the cut.
    let mut s = n_layers(1);
    s.buffers.0[0] = buf(1, 32, 32, f("AR24"), AFBC_C);
    let a = planned(&c, &s).unwrap();
    assert!(CLUSTER_REACHING.contains(&a[0].1));
}

#[test]
fn rk3588_scaled_or_rotated_layers_are_refused_as_the_engine_reads_it() {
    // Scaling is "not claimed" and transforms are never filled (observed
    // 2026-11-28, probe), so the planner refuses both on every plane.
    let c = rk3588_vop2();
    let mut s = n_layers(1);
    s.layers[0].destination = Rect::new(0, 0, 64, 64);
    let (_, reasons) = refused(planned(&c, &s));
    assert!(reasons.contains(RejectReasons::SCALING), "{reasons:?}");
    let mut s = n_layers(1);
    s.layers[0].transform = Transform::Rotate90;
    let (_, reasons) = refused(planned(&c, &s));
    assert!(reasons.contains(RejectReasons::TRANSFORM), "{reasons:?}");
}

// ---------------------------------------------------------------------------
// BCM2711 HVS5 and BCM2712 HVS6
// ---------------------------------------------------------------------------

/// Number of planes reaching the output: the one connected primary plus all
/// the overlays kept.
fn reachable(c: &Candidate) -> usize {
    c.caps.planes.iter().filter(|p| p.outputs & 1 != 0).count()
}

fn hvs_candidates() -> [(&'static str, Candidate); 2] {
    [("hvs5", bcm2711_hvs5()), ("hvs6", bcm2712_hvs6())]
}

#[test]
fn hvs_models_are_the_cut_views() {
    for (name, c) in hvs_candidates() {
        assert_eq!(c.caps.planes.len(), MAX_PLANES, "{name}");
        assert!(c.caps.planes.iter().all(|p| p.formats.len() == MAX_FORMATS));
    }
    // One primary plus the overlays reach the output.
    assert_eq!(reachable(&bcm2711_hvs5()), 11);
    assert_eq!(reachable(&bcm2712_hvs6()), 13);
}

#[test]
fn hvs_video_with_ui_overlay_is_planned_in_stack_order() {
    for (name, c) in hvs_candidates() {
        let s = video_ui(&c, PixelFormat::NV12);
        let a = planned(&c, &s).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_stack_is_ordered(&a, 2);
        // The primary has fixed zpos 0 and can only be the bottom layer.
        for x in &a {
            if x.2 == 0 {
                assert_eq!(x.0, 0, "{name}: only the bottom layer sits at zpos 0");
            }
        }
    }
}

#[test]
fn hvs_cursor_on_top_of_video_and_ui_gets_the_highest_zpos() {
    for (name, c) in hvs_candidates() {
        let s = video_ui_cursor(&c, PixelFormat::NV12);
        let a = planned(&c, &s).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_stack_is_ordered(&a, 3);
        // Overlay zpos range is 1..=17.
        assert!(a[2].2 <= 17, "{name}: {a:?}");
    }
}

#[test]
fn hvs_fills_every_reaching_plane_and_refuses_the_next_layer() {
    // As many layers as reaching planes fit (the primary at zpos 0 and the
    // overlays at 1..=17); one more has no plane. `advise` would compose the
    // excess.
    for (name, c) in hvs_candidates() {
        let n = reachable(&c);
        assert!(n < MAX_LAYERS);
        let a = planned(&c, &n_layers(n)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_stack_is_ordered(&a, n);
        let (layer, reasons) = refused(planned(&c, &n_layers(n + 1)));
        assert_eq!(layer, n, "{name}");
        assert!(
            reasons.contains(RejectReasons::IN_USE),
            "{name}: {reasons:?}"
        );
    }
    // A frame of MAX_LAYERS layers is above both models.
    for (name, c) in hvs_candidates() {
        assert!(
            matches!(
                planned(&c, &n_layers(MAX_LAYERS)),
                Err(PlanError::NoPlane { .. })
            ),
            "{name}"
        );
    }
}

#[test]
fn hvs_unsupported_format_is_refused() {
    // NV24 is on neither HVS list; the RK3588 Esmart planes do list it.
    for (name, c) in hvs_candidates() {
        let mut s = n_layers(1);
        s.buffers.0[0] = buf(1, 32, 32, f("NV24"), Modifier::LINEAR);
        let (layer, reasons) = refused(planned(&c, &s));
        assert_eq!(layer, 0, "{name}");
        assert!(
            reasons.contains(RejectReasons::FORMAT),
            "{name}: {reasons:?}"
        );
    }
    let c = rk3588_vop2();
    let mut s = n_layers(1);
    s.buffers.0[0] = buf(1, 32, 32, f("NV24"), Modifier::LINEAR);
    assert!(planned(&c, &s).is_ok());
}

#[test]
fn hvs_non_linear_modifier_is_refused_because_the_cut_keeps_linear_only() {
    // The blob also lists T-tiled and SAND pairs (observed 2026-11-27), all
    // past the 32-pair cut that keeps linear first. Open MAX_FORMATS
    // decision: the model cannot place a tiled buffer.
    for (name, c) in hvs_candidates() {
        let mut s = n_layers(1);
        s.buffers.0[0] = buf(1, 32, 32, f("AR24"), Modifier(0x700_0000_0000_0001));
        let (_, reasons) = refused(planned(&c, &s));
        assert!(
            reasons.contains(RejectReasons::FORMAT),
            "{name}: {reasons:?}"
        );
    }
}

#[test]
fn hvs6_formats_past_the_cap_are_refused_and_hvs5_has_none_of_them() {
    // P010 is among the BCM2712 formats the 32-entry cut dropped
    // (observed 2026-11-28); neither HVS list in the model has it.
    for (name, c) in hvs_candidates() {
        let mut s = n_layers(1);
        s.buffers.0[0] = buf(1, 32, 32, f("P010"), Modifier::LINEAR);
        let (_, reasons) = refused(planned(&c, &s));
        assert!(
            reasons.contains(RejectReasons::FORMAT),
            "{name}: {reasons:?}"
        );
    }
}

#[test]
fn hvs_scaling_is_not_claimed_so_scaled_layers_are_refused() {
    // The probe prints "scaling not claimed" for every HVS plane (observed
    // 2026-11-28); the kernel accepted scaled TEST_ONLY commits, which the
    // model does not know. The planner trusts the model.
    for (name, c) in hvs_candidates() {
        let mut s = n_layers(1);
        s.layers[0].destination = Rect::new(0, 0, 64, 64);
        let (_, reasons) = refused(planned(&c, &s));
        assert!(
            reasons.contains(RejectReasons::SCALING),
            "{name}: {reasons:?}"
        );
    }
}
