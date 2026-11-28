//! What `DriverProfile::Generic` does with the Broadcom modifiers of the
//! recorded BCM2712 (Pi 5, HVS6) and BCM2711 (CM4, HVS5) planes: linear,
//! SAND and T-tiled buffers, and the pairs the `MAX_FORMATS` cut removes.
//!
//! Baseline tests: they pin the current behaviour so the later `Vc4Hvs5` and
//! `Vc4Hvs6` profiles have something to differ from. Nothing here reads
//! `docs/`; the data is copied into the fixtures below.
//!
//! Evidence tags: *observed* = in the recorded output, *source* = read from
//! the vc4 driver (`docs/platform-capabilities.md`), *inferred* = derived,
//! not measured.

use composition_core::limits::MAX_FORMATS;
use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, Modifier,
    OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, Size, Zpos,
};
use composition_drm::DriverProfile;

const OUT: OutputId = OutputId(0);
const SIZE: Size = Size::new(1920, 1080);

// Broadcom vendor modifiers, `fourcc_mod_broadcom_code(code, 0)`. The numeric
// codes are observed in the Pi 5 `IN_FORMATS` blob (bcm2712-topology.md,
// 2026-11-28). The names are inferred from `drm_fourcc.h` as remembered (not
// read on the board); the blob agrees: code 4 is the only one that lists
// P030, and the vc4 source says P030 takes SAND128 only. (The older
// generic_vc4.rs test names code 3 SAND128; this file follows the P030 clue.)
const VC4_T_TILED: Modifier = Modifier(0x0700_0000_0000_0001);
const SAND64: Modifier = Modifier(0x0700_0000_0000_0003);
const SAND128: Modifier = Modifier(0x0700_0000_0000_0004);
const SAND256: Modifier = Modifier(0x0700_0000_0000_0005);

fn fourcc(s: &[u8; 4]) -> PixelFormat {
    PixelFormat::fourcc(s)
}

// Source: docs/platform-capabilities.md, "IN_FORMATS (identical on all
// five planes)", Raspberry Pi 5 (BCM2712), kernel 6.18.50+rpt-rpi-2712,
// recorded 2026-11-28. The linear modifier lists 44 formats, in this order;
// with P030 under SAND128 that is the blob's 45 formats.
const PI5_LINEAR: [&[u8; 4]; 44] = [
    b"XR24", b"AR24", b"AB24", b"XB24", b"RX24", b"RA24", b"BX24", b"BA24", b"RG16", b"BG16",
    b"AR15", b"XR15", b"RG24", b"BG24", b"YU16", b"YV16", b"YU24", b"YV24", b"YU12", b"YV12",
    b"NV12", b"NV21", b"NV16", b"NV61", b"XR30", b"AR30", b"AB30", b"XB30", b"RGB8", b"BGR8",
    b"XR12", b"AR12", b"XB12", b"AB12", b"BX12", b"BA12", b"RX12", b"RA12", b"P010", b"P012",
    b"P016", b"S010", b"S012", b"S016",
];
// Same recording, the Broadcom blocks in blob order: T-tiled (code 1).
const PI5_T_TILED: [&[u8; 4]; 8] = [
    b"XR24", b"AR24", b"AB24", b"XB24", b"RG16", b"BG16", b"AR15", b"XR15",
];
// Codes 3 (SAND64), 4 (SAND128, with P030) and 5 (SAND256).
const PI5_SAND64: [&[u8; 4]; 2] = [b"NV12", b"NV21"];
const PI5_SAND128: [&[u8; 4]; 3] = [b"NV12", b"NV21", b"P030"];
const PI5_SAND256: [&[u8; 4]; 2] = [b"NV12", b"NV21"];

// Source: docs/platform-capabilities.md (BCM2711 section), reTerminal
// (CM4, BCM2711), recorded 2026-11-28. The probe printed 32 linear formats and
// no modifier column, so the CM4's `IN_FORMATS` modifiers were NOT recorded.
// The modifier blocks used for it are inferred from the vc4 source (HVS5
// advertises LINEAR, T-tiled, SAND64, SAND128, SAND256 with the same
// per-format rules as the Pi 5 blocks above). Tagged inferred.
const CM4_LINEAR: [&[u8; 4]; 32] = [
    b"XR24", b"AR24", b"AB24", b"XB24", b"RG16", b"BG16", b"AR15", b"XR15", b"RG24", b"BG24",
    b"YU16", b"YV16", b"YU24", b"YV24", b"YU12", b"YV12", b"NV12", b"NV21", b"NV16", b"NV61",
    b"XR30", b"AR30", b"AB30", b"XB30", b"RGB8", b"BGR8", b"XR12", b"AR12", b"XB12", b"AB12",
    b"BX12", b"BA12",
];

fn block(modifier: Modifier, formats: &[&[u8; 4]]) -> Vec<FormatModifier> {
    formats
        .iter()
        .map(|f| FormatModifier {
            format: fourcc(f),
            modifier,
        })
        .collect()
}

/// The kernel's `IN_FORMATS` pairs in blob order.
fn full_list(linear: &[&[u8; 4]]) -> Vec<FormatModifier> {
    let mut pairs = block(Modifier::LINEAR, linear);
    pairs.extend(block(VC4_T_TILED, &PI5_T_TILED));
    pairs.extend(block(SAND64, &PI5_SAND64));
    pairs.extend(block(SAND128, &PI5_SAND128));
    pairs.extend(block(SAND256, &PI5_SAND256));
    pairs
}

fn pi5_pairs() -> Vec<FormatModifier> {
    full_list(&PI5_LINEAR)
}

fn cm4_pairs() -> Vec<FormatModifier> {
    full_list(&CM4_LINEAR)
}

/// The backend's cut (`composition-drm` `lib.rs`, plane probe): a stable sort
/// with linear pairs first, then the first `MAX_FORMATS`.
fn kept(mut pairs: Vec<FormatModifier>) -> Vec<FormatModifier> {
    pairs.sort_by_key(|p| p.modifier != Modifier::LINEAR);
    pairs.truncate(MAX_FORMATS);
    pairs
}

/// A primary (zpos fixed 0) and one overlay (zpos 1..=17), both with `pairs`,
/// as recorded for planes 83 and 107 on the Pi 5 (observed 2026-11-28).
fn caps(pairs: &[FormatModifier]) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SIZE,
            refresh_mhz: 60_000,
        })
        .unwrap();
    let blend =
        BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask() | BlendMode::Coverage.mask();
    for (id, kind, zpos) in [
        (83, PlaneKind::Primary, Zpos::Fixed(0)),
        (107, PlaneKind::Overlay, Zpos::Range { min: 1, max: 17 }),
    ] {
        let mut plane = PlaneCaps {
            id: PlaneId(id),
            kind,
            outputs: 1,
            plane_alpha: true,
            blend_modes: blend,
            zpos,
            ..PlaneCaps::default()
        };
        for p in pairs {
            plane.formats.push(*p).unwrap();
        }
        caps.planes.push(plane).unwrap();
    }
    caps
}

const RGB: BufferId = BufferId(1);
const NV12: BufferId = BufferId(2);
const NV12_SAND64: BufferId = BufferId(3);
const NV12_SAND128: BufferId = BufferId(4);
const NV12_SAND256: BufferId = BufferId(5);
const XRGB_TILED: BufferId = BufferId(6);
const P030_SAND128: BufferId = BufferId(7);
const P010: BufferId = BufferId(8);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let (format, modifier) = match buffer {
            RGB => (PixelFormat::XRGB8888, Modifier::LINEAR),
            NV12 => (PixelFormat::NV12, Modifier::LINEAR),
            NV12_SAND64 => (PixelFormat::NV12, SAND64),
            NV12_SAND128 => (PixelFormat::NV12, SAND128),
            NV12_SAND256 => (PixelFormat::NV12, SAND256),
            XRGB_TILED => (PixelFormat::XRGB8888, VC4_T_TILED),
            P030_SAND128 => (fourcc(b"P030"), SAND128),
            P010 => (fourcc(b"P010"), Modifier::LINEAR),
            _ => return None,
        };
        Some(BufferInfo {
            size: SIZE,
            format,
            modifier,
        })
    }
}

type Placed = Vec<(usize, u32, i32)>;

fn place(caps: &Capabilities, layers: &[Layer]) -> Result<Placed, PlanError> {
    let frame = Frame {
        output: OUT,
        layers,
    };
    plan(&frame, caps, &Buffers).map(|p| {
        p.assignments()
            .iter()
            .map(|a| (a.layer, a.plane.0, a.zpos))
            .collect()
    })
}

fn full() -> Rect {
    Rect::new(0, 0, SIZE.width, SIZE.height)
}

fn one(caps: &Capabilities, buffer: BufferId) -> Result<Placed, PlanError> {
    place(caps, &[Layer::new(buffer, full(), full(), 0)])
}

fn refused(layer: usize) -> Result<Placed, PlanError> {
    Err(PlanError::NoPlane {
        layer,
        reasons: RejectReasons::FORMAT,
    })
}

/// Both generations as the backend keeps them after the cut.
fn kept_caps() -> [(&'static str, Capabilities); 2] {
    [
        ("BCM2712", caps(&kept(pi5_pairs()))),
        ("BCM2711", caps(&kept(cm4_pairs()))),
    ]
}

// ---- the cut ----

#[test]
fn the_kernel_lists_more_pairs_than_the_cut_keeps() {
    // Pi 5 observed: 44 linear + 8 T-tiled + 2 + 3 + 2 Broadcom pairs.
    // CM4: 32 recorded linear + the same inferred modifier blocks.
    assert_eq!(pi5_pairs().len(), 44 + 15);
    assert_eq!(cm4_pairs().len(), 32 + 15);
    assert!(cm4_pairs().len() > MAX_FORMATS);
}

#[test]
fn the_cut_keeps_32_linear_pairs_and_every_modifier_pair_is_lost() {
    for (name, pairs) in [("BCM2712", pi5_pairs()), ("BCM2711", cm4_pairs())] {
        let kept = kept(pairs.clone());
        assert_eq!(kept.len(), MAX_FORMATS, "{name}");
        assert!(
            kept.iter().all(|p| p.modifier == Modifier::LINEAR),
            "{name}"
        );
        // The kept pairs are the first 32 linear ones, in blob order.
        let linear: Vec<_> = pairs
            .iter()
            .copied()
            .filter(|p| p.modifier == Modifier::LINEAR)
            .take(MAX_FORMATS)
            .collect();
        assert_eq!(kept, linear, "{name}");
    }
}

#[test]
fn the_pi5_cut_loses_12_linear_and_all_15_broadcom_pairs() {
    // Observed discrepancy 2 of bcm2712-topology.md: XB12 .. S016 are cut.
    let kept = kept(pi5_pairs());
    let lost: Vec<_> = pi5_pairs()
        .into_iter()
        .filter(|p| !kept.contains(p))
        .collect();
    assert_eq!(lost.len(), 12 + 15);
    assert_eq!(
        lost.iter()
            .filter(|p| p.modifier != Modifier::LINEAR)
            .count(),
        15
    );
}

// ---- linear frames survive ----

#[test]
fn linear_xrgb_and_nv12_go_to_the_primary_on_both() {
    for (name, caps) in kept_caps() {
        assert_eq!(one(&caps, RGB), Ok(vec![(0, 83, 0)]), "{name}");
        assert_eq!(one(&caps, NV12), Ok(vec![(0, 83, 0)]), "{name}");
    }
}

#[test]
fn linear_rgb_below_linear_nv12_uses_primary_and_overlay() {
    for (name, caps) in kept_caps() {
        let layers = [
            Layer::new(RGB, full(), full(), 0),
            Layer::new(NV12, full(), full(), 1),
        ];
        let placed = place(&caps, &layers).unwrap();
        assert_eq!(placed[0], (0, 83, 0), "{name}");
        assert_eq!(placed[1].1, 107, "{name}");
        assert!(placed[1].2 >= 1, "{name}");
    }
}

// ---- modifier frames are refused because their pair was cut ----

#[test]
fn sand_nv12_is_refused_on_format_for_every_sand_width() {
    // Baseline: the kernel lists NV12 under SAND64/128/256 (Pi 5 observed;
    // CM4 inferred), but the pairs are cut, so Generic refuses all of them.
    // A Vc4Hvs5/Vc4Hvs6 profile that keeps modifier pairs changes this.
    for (name, caps) in kept_caps() {
        for buffer in [NV12_SAND64, NV12_SAND128, NV12_SAND256] {
            assert_eq!(one(&caps, buffer), refused(0), "{name} {buffer:?}");
        }
    }
}

#[test]
fn t_tiled_xrgb_and_sand128_p030_are_refused_on_format() {
    for (name, caps) in kept_caps() {
        assert_eq!(one(&caps, XRGB_TILED), refused(0), "{name}");
        assert_eq!(one(&caps, P030_SAND128), refused(0), "{name}");
    }
}

#[test]
fn a_linear_pair_past_the_cut_is_refused_too() {
    // P010 is linear but the 39th linear pair of the Pi 5 list (observed).
    for (name, caps) in kept_caps() {
        assert_eq!(one(&caps, P010), refused(0), "{name}");
    }
}

#[test]
fn a_sand_layer_in_a_mixed_frame_names_that_layer() {
    for (name, caps) in kept_caps() {
        let layers = [
            Layer::new(RGB, full(), full(), 0),
            Layer::new(NV12_SAND128, full(), full(), 1),
        ];
        assert_eq!(place(&caps, &layers), refused(1), "{name}");
    }
}

#[test]
fn generic_does_not_change_the_cut_caps() {
    for (name, caps) in kept_caps() {
        let mut applied = caps;
        DriverProfile::Generic.apply(&mut applied);
        assert_eq!(applied, caps, "{name}");
    }
}

// ---- when the pair is kept (hypothetical: not what the backend builds) ----

/// The 32 pairs a plane would keep if the cut did not put linear first: the
/// Broadcom blocks, then linear pairs to fill the list.
fn modifier_first(pairs: Vec<FormatModifier>) -> Vec<FormatModifier> {
    let mut v: Vec<_> = pairs
        .iter()
        .copied()
        .filter(|p| p.modifier != Modifier::LINEAR)
        .collect();
    v.extend(
        pairs
            .iter()
            .copied()
            .filter(|p| p.modifier == Modifier::LINEAR),
    );
    v.truncate(MAX_FORMATS);
    v
}

#[test]
fn modifier_layers_are_placed_when_their_pairs_are_kept() {
    // Not a recording: the same planes with the Broadcom pairs first. The
    // modifier frames are placed on the primary.
    for (name, pairs) in [("BCM2712", pi5_pairs()), ("BCM2711", cm4_pairs())] {
        let caps = caps(&modifier_first(pairs));
        for buffer in [
            NV12_SAND64,
            NV12_SAND128,
            NV12_SAND256,
            XRGB_TILED,
            P030_SAND128,
        ] {
            assert_eq!(
                one(&caps, buffer),
                Ok(vec![(0, 83, 0)]),
                "{name} {buffer:?}"
            );
        }
    }
}

#[test]
fn generic_has_no_per_generation_modifier_rule() {
    // Source (vc4-hvs-source.md): HVS6 rejects T-tiled and SAND64 in
    // `vc6_plane_mode_set`, HVS5 accepts both. The recorded Pi 5 blob still
    // lists them (the open conflict). Generic only compares pairs, so Pi 5
    // caps with those pairs kept place a T-tiled and a SAND64 layer exactly
    // as the CM4 does. Vc4Hvs6 is expected to refuse them; this test marks
    // the behaviour to change.
    let hvs6 = caps(&modifier_first(pi5_pairs()));
    for buffer in [XRGB_TILED, NV12_SAND64] {
        assert_eq!(one(&hvs6, buffer), Ok(vec![(0, 83, 0)]), "{buffer:?}");
    }
}

#[test]
fn the_modifier_must_match_exactly() {
    // A plane that lists NV12 only under SAND128 refuses linear NV12 and
    // SAND256 NV12, and takes SAND128 NV12.
    let caps = caps(&block(SAND128, &[b"NV12"]));
    assert_eq!(one(&caps, NV12_SAND128), Ok(vec![(0, 83, 0)]));
    assert_eq!(one(&caps, NV12), refused(0));
    assert_eq!(one(&caps, NV12_SAND256), refused(0));
}
