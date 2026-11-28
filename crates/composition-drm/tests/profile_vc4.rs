//! `DriverProfile::Vc4Hvs5` and `Vc4Hvs6` applied to the recorded probe
//! output of the reTerminal (CM4, BCM2711, HVS5) and the Raspberry Pi 5
//! (BCM2712, HVS6), then planned with the core planner.
//!
//! The fixtures are the 16 planes the backend keeps for the connected
//! output, as the probe printed them. The kernels expose more: 60 planes on
//! the CM4 and 56 on the Pi 5, of which 18 reach the connected CRTC
//! (primary, 16 shared overlays, cursor); the cut to 16 is private to
//! `src/lib.rs` and tested there. Nothing here reads `docs/`.
//!
//! Evidence tags as in the profile: *observed* = in the recorded dumps,
//! *source* = the vc4 source read at the profile's pin, *inferred* = derived.

mod common;

use common::recorded::{self, SAND128, SAND256, SAND64, VC4_T_TILED};
use composition_core::limits::{MAX_LAYERS, MAX_PLANES};
use composition_core::planner::{plan_with, BufferLookup, SearchLimits, Verdict};
use composition_core::{
    BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, LayerTraits, Modifier,
    PixelFormat, PlanError, PlaneKind, Rect, RejectReasons, ResourceGroup, ScaleLimits, Size, Zpos,
};
use composition_drm::DriverProfile;

// Source: tests/data/bcm2711/probe-raw-2026-11-27.txt, `probe --raw` on the
// reTerminal (CM4, BCM2711), kernel 6.12.47+rpt-rpi-v8, recorded 2026-11-27.
// Output 0 is DSI-1, 720x1280; kept planes: primary 79, overlays 127..270
// (14, stride 11), cursor 677. 32 linear formats per plane, no modifier.
const CM4_PROBE: &str = include_str!("data/bcm2711/probe-raw-2026-11-27.txt");
// Source: tests/data/bcm2712/probe-raw-2026-11-27.txt, `probe --raw` on the
// Raspberry Pi 5 (BCM2712), kernel 6.18.50+rpt-rpi-2712, recorded
// 2026-11-27. Output 0 is HDMI, 1920x1080; kept planes: primary 83,
// overlays 107..250 (14, stride 11), cursor 657. 32 linear formats per
// plane, no modifier.
const PI5_PROBE: &str = include_str!("data/bcm2712/probe-raw-2026-11-27.txt");

struct Board {
    name: &'static str,
    probe: &'static str,
    profile: DriverProfile,
    ids: [u32; MAX_PLANES],
    /// LBM capacity the profile claims for this generation.
    lbm: u8,
}

const CM4: Board = Board {
    name: "BCM2711",
    probe: CM4_PROBE,
    profile: DriverProfile::Vc4Hvs5,
    ids: [
        79, 127, 138, 149, 160, 171, 182, 193, 204, 215, 226, 237, 248, 259, 270, 677,
    ],
    lbm: 8,
};

const PI5: Board = Board {
    name: "BCM2712",
    probe: PI5_PROBE,
    profile: DriverProfile::Vc4Hvs6,
    ids: [
        83, 107, 118, 129, 140, 151, 162, 173, 184, 195, 206, 217, 228, 239, 250, 657,
    ],
    lbm: 4,
};

const BOARDS: [&Board; 2] = [&CM4, &PI5];

impl Board {
    /// The recorded caps, as KMS reported them.
    fn recorded(&self) -> Capabilities {
        let loaded = recorded::load(self.probe).expect(self.name);
        loaded.caps
    }

    /// The recorded caps with the profile applied.
    fn caps(&self) -> Capabilities {
        let mut caps = self.recorded();
        self.profile.apply(&mut caps);
        caps
    }

    /// The recorded caps with four `IN_FORMATS` modifier pairs in place of
    /// the last four linear pairs of every plane, then the profile.
    // Source: tests/data/bcm2711/props-2026-11-27.txt and
    // tests/data/bcm2712/props-2026-11-27.txt (recorded 2026-11-27): the
    // IN_FORMATS blob of every plane on both boards lists XR24 with
    // modifier 0x0700000000000001 (T-tiled) and NV12 with 0x...03 (SAND64),
    // 0x...04 (SAND128) and 0x...05 (SAND256). The probe's 32-pair cut
    // keeps only linear pairs, so the profile never sees these on today's
    // backend; the substitution exercises the modifier policy only.
    fn caps_with_modifiers(&self) -> Capabilities {
        let mut caps = self.recorded();
        for plane in caps.planes.as_mut_slice() {
            for _ in 0..4 {
                plane.formats.pop();
            }
            for (format, modifier) in [
                (PixelFormat::XRGB8888, VC4_T_TILED),
                (PixelFormat::NV12, SAND64),
                (PixelFormat::NV12, SAND128),
                (PixelFormat::NV12, SAND256),
            ] {
                plane
                    .formats
                    .push(FormatModifier {
                        format,
                        modifier: Modifier(modifier),
                    })
                    .unwrap();
            }
        }
        self.profile.apply(&mut caps);
        caps
    }

    fn output(&self) -> Size {
        self.recorded().outputs.as_slice()[0].size
    }
}

const RGB: BufferId = BufferId(1);
const NV12: BufferId = BufferId(2);
const NV12_SAND64: BufferId = BufferId(3);
const NV12_SAND128: BufferId = BufferId(4);
const XRGB_TILED: BufferId = BufferId(5);

struct Buffers;

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let (format, modifier) = match buffer {
            RGB => (PixelFormat::XRGB8888, 0),
            NV12 => (PixelFormat::NV12, 0),
            NV12_SAND64 => (PixelFormat::NV12, SAND64),
            NV12_SAND128 => (PixelFormat::NV12, SAND128),
            XRGB_TILED => (PixelFormat::XRGB8888, VC4_T_TILED),
            _ => return None,
        };
        Some(BufferInfo {
            size: Size::new(2048, 2048),
            format,
            modifier: Modifier(modifier),
        })
    }
}

type Placed = Vec<(usize, u32, i32)>;

fn run(caps: &Capabilities, layers: &[Layer]) -> Result<Placed, PlanError> {
    run_with(caps, layers, SearchLimits::default())
}

/// Like [`run`], with a step budget large enough to exhaust the search on a
/// frame the default budget gives up on.
fn run_exhaustive(caps: &Capabilities, layers: &[Layer]) -> Result<Placed, PlanError> {
    let limits = SearchLimits {
        steps: 50_000_000,
        ..SearchLimits::default()
    };
    run_with(caps, layers, limits)
}

fn run_with(
    caps: &Capabilities,
    layers: &[Layer],
    limits: SearchLimits,
) -> Result<Placed, PlanError> {
    // The loader names the output by its CRTC id.
    let frame = Frame {
        output: caps.outputs.as_slice()[0].id,
        layers,
    };
    plan_with(&frame, caps, &Buffers, limits, |_| Verdict::Accept).map(|p| {
        p.assignments()
            .iter()
            .map(|a| (a.layer, a.plane.0, a.zpos))
            .collect()
    })
}

/// A 64x64 window at grid position `i`, from a source of `src` pixels.
fn window(buffer: BufferId, i: usize, src: (u32, u32)) -> Layer {
    let (x, y) = ((i as i32 % 4) * 80, (i as i32 / 4) * 80);
    Layer::new(
        buffer,
        Rect::new(0, 0, src.0, src.1),
        Rect::new(x, y, 64, 64),
        i as i32,
    )
}

fn unscaled(buffer: BufferId, i: usize) -> Layer {
    window(buffer, i, (64, 64))
}

fn full(board: &Board, buffer: BufferId) -> Layer {
    let size = board.output();
    let rect = Rect::new(0, 0, size.width, size.height);
    Layer::new(buffer, rect, rect, 0)
}

fn lbm() -> LayerTraits {
    LayerTraits::SCALED_V.union(LayerTraits::YUV)
}

// ---- the fixtures and the table ----

#[test]
fn fixtures_are_the_recorded_cut() {
    for board in BOARDS {
        let loaded = recorded::load(board.probe).expect(board.name);
        let ids: Vec<u32> = loaded.caps.planes.iter().map(|p| p.id.0).collect();
        assert_eq!(ids, board.ids, "{}", board.name);
        let kinds: Vec<PlaneKind> = loaded.caps.planes.iter().map(|p| p.kind).collect();
        assert_eq!(kinds[0], PlaneKind::Primary, "{}", board.name);
        assert_eq!(kinds[MAX_PLANES - 1], PlaneKind::Cursor, "{}", board.name);
        assert!(loaded
            .caps
            .planes
            .iter()
            .all(|p| p.formats.len() == 32 && p.scaling.is_none() && p.max_source.is_none()));
        assert!(loaded.caps.resource_groups.is_empty());
    }
}

#[test]
fn groups_are_present_once_after_a_second_apply() {
    for board in BOARDS {
        let mut caps = board.caps();
        let once = caps;
        board.profile.apply(&mut caps);
        assert_eq!(caps, once, "{}", board.name);

        let all = (1u32 << MAX_PLANES) - 1;
        let mut expected = vec![ResourceGroup {
            consumers: lbm(),
            planes: all,
            capacity: board.lbm,
        }];
        if board.profile == DriverProfile::Vc4Hvs6 {
            // UPM: one handle per colour plane on every enabled plane.
            expected.push(ResourceGroup {
                consumers: LayerTraits::ANY,
                planes: all,
                capacity: 16,
            });
        }
        assert_eq!(caps.resource_groups.as_slice(), expected, "{}", board.name);
    }
}

#[test]
fn scaling_and_max_source_only_on_non_cursor_planes() {
    for board in BOARDS {
        let max_source = match board.profile {
            DriverProfile::Vc4Hvs5 => Size::new(8191, 7680),
            _ => Size::new(8192, 8192),
        };
        for plane in board.caps().planes.iter() {
            if plane.kind == PlaneKind::Cursor {
                assert_eq!(plane.scaling, None, "{}", board.name);
                assert_eq!(plane.max_source, None, "{}", board.name);
            } else {
                // Observed 2026-11-27: 32x up and 0.5 down accepted on every
                // tested primary and overlay of both boards.
                assert_eq!(
                    plane.scaling,
                    Some(ScaleLimits {
                        max_upscale: 32 << 16,
                        max_downscale: 2 << 16,
                    }),
                    "{}",
                    board.name
                );
                assert_eq!(plane.max_source, Some(max_source), "{}", board.name);
            }
        }
    }
}

#[test]
fn generic_leaves_the_fixtures_untouched() {
    for board in BOARDS {
        let mut caps = board.recorded();
        let before = caps;
        DriverProfile::Generic.apply(&mut caps);
        assert_eq!(caps, before, "{}", board.name);
    }
}

// ---- planner cases ----

#[test]
fn n_unscaled_overlays_up_to_max_layers() {
    for board in BOARDS {
        let caps = board.caps();
        for n in 1..=MAX_LAYERS {
            let layers: Vec<Layer> = (0..n).map(|i| unscaled(RGB, i)).collect();
            let placed =
                run(&caps, &layers).unwrap_or_else(|e| panic!("{} {n}: {e:?}", board.name));
            assert_eq!(placed.len(), n, "{}", board.name);
            assert_eq!(placed[0], (0, board.ids[0], 0), "{}", board.name);
        }
    }
}

#[test]
fn scaling_inside_the_limits_is_accepted_and_outside_refused() {
    for board in BOARDS {
        let caps = board.caps();
        // 32x up (2x2 into 64x64) and 2x down (128x128 into 64x64).
        for src in [(2, 2), (128, 128)] {
            assert!(run(&caps, &[window(RGB, 0, src)]).is_ok(), "{}", board.name);
        }
        // One step beyond either way: refused for scaling on every plane but
        // the cursor, which has no scaler at all.
        for src in [(1, 64), (130, 64)] {
            let Err(PlanError::NoPlane { layer: 0, reasons }) = run(&caps, &[window(RGB, 0, src)])
            else {
                panic!("{}: {src:?} must be refused", board.name);
            };
            assert_eq!(reasons, RejectReasons::SCALING, "{}", board.name);
        }
    }
}

/// The recorded primary and the first `overlays` recorded overlays, with the
/// profile applied: small enough for the planner to search exhaustively when
/// a frame does not fit.
fn subset(board: &Board, overlays: usize) -> Capabilities {
    let mut caps = board.recorded();
    let planes = caps.planes;
    caps.planes.clear();
    let mut kept = 0;
    for plane in planes.iter() {
        match plane.kind {
            PlaneKind::Primary => {}
            PlaneKind::Overlay if kept < overlays => kept += 1,
            _ => continue,
        }
        caps.planes.push(*plane).unwrap();
    }
    board.profile.apply(&mut caps);
    caps
}

/// An unscaled RGB bottom layer, then `n` layers made by `upper`.
fn stack(n: usize, upper: impl Fn(usize) -> Layer) -> Vec<Layer> {
    let mut layers = vec![unscaled(RGB, 0)];
    layers.extend((1..=n).map(upper));
    layers
}

/// Asserts that `lbm` layers made by `upper` fit and one more is refused
/// with RESOURCE.
fn assert_lbm_refuses(board: &Board, upper: impl Fn(usize) -> Layer) {
    let lbm = board.lbm as usize;
    // On the full fixture the planner accepts `lbm` such layers but cannot
    // report the refusal of one more: with 15 alike planes the search runs
    // through every placement of the earlier layers and hits the default
    // step budget first.
    let caps = board.caps();
    assert_eq!(
        run(&caps, &stack(lbm, &upper)).map(|p| p.len()),
        Ok(lbm + 1),
        "{}",
        board.name
    );
    assert_eq!(
        run(&caps, &stack(lbm + 1, &upper)),
        Err(PlanError::LimitExceeded),
        "{}",
        board.name
    );
    // With enough planes for every layer (primary, lbm + 1 overlays) and an
    // exhaustive search, the refusal names the group.
    let caps = subset(board, lbm + 1);
    let result = run_exhaustive(&caps, &stack(lbm + 1, &upper));
    let Err(PlanError::NoPlane { layer, reasons }) = result else {
        panic!("{}: one more layer than LBM units: {result:?}", board.name);
    };
    assert_eq!(layer, lbm + 1, "{}", board.name);
    assert!(reasons.contains(RejectReasons::RESOURCE), "{}", board.name);
    assert!(run(&caps, &stack(lbm, &upper)).is_ok(), "{}", board.name);
}

#[test]
fn vertical_scaling_on_every_overlay_until_the_lbm_group_refuses() {
    for board in BOARDS {
        // Vertically scaled windows, 64x32 into 64x64.
        assert_lbm_refuses(board, |i| window(RGB, i, (64, 32)));

        // Horizontal scaling takes no LBM (source), so as many horizontally
        // scaled windows fit as there are overlays.
        let wide = stack(MAX_PLANES - 2, |i| window(RGB, i, (32, 64)));
        assert!(run(&board.caps(), &wide).is_ok(), "{}", board.name);
    }
}

#[test]
fn nv12_takes_lbm_even_unscaled() {
    // Source: multi-plane formats force the chroma y axis to the polyphase
    // filter, so NV12 needs LBM at 1:1.
    for board in BOARDS {
        assert_lbm_refuses(board, |i| unscaled(NV12, i));
    }
}

#[test]
fn nv12_sand128_is_accepted_on_both_and_sand64_refused_on_hvs6() {
    for board in BOARDS {
        let caps = board.caps_with_modifiers();
        assert_eq!(
            run(&caps, &[full(board, NV12_SAND128)]),
            Ok(vec![(0, board.ids[0], 0)]),
            "{}",
            board.name
        );
        let sand64 = run(&caps, &[full(board, NV12_SAND64)]);
        match board.profile {
            DriverProfile::Vc4Hvs5 => {
                assert_eq!(sand64, Ok(vec![(0, board.ids[0], 0)]), "{}", board.name)
            }
            _ => assert_eq!(
                sand64,
                Err(PlanError::NoPlane {
                    layer: 0,
                    reasons: RejectReasons::FORMAT
                }),
                "{}",
                board.name
            ),
        }
    }
}

#[test]
fn t_tiled_xrgb_is_accepted_on_hvs5_and_refused_on_hvs6() {
    for board in BOARDS {
        let caps = board.caps_with_modifiers();
        let tiled = run(&caps, &[full(board, XRGB_TILED)]);
        match board.profile {
            DriverProfile::Vc4Hvs5 => {
                assert_eq!(tiled, Ok(vec![(0, board.ids[0], 0)]), "{}", board.name)
            }
            _ => assert_eq!(
                tiled,
                Err(PlanError::NoPlane {
                    layer: 0,
                    reasons: RejectReasons::FORMAT
                }),
                "{}",
                board.name
            ),
        }
        // HVS6 removes exactly the T-tiled and SAND64 pairs.
        let kept = caps.planes.as_slice()[0].formats.len();
        let expected = if board.profile == DriverProfile::Vc4Hvs5 {
            32
        } else {
            30
        };
        assert_eq!(kept, expected, "{}", board.name);
    }
}

#[test]
fn a_layer_below_the_fixed_zpos_0_primary_is_refused() {
    // The recorded primary and cursor alone (the pair every CRTC owns; the
    // shared overlays could be held by another output: inferred). The cursor
    // has no scaler, so a scaled top layer could only go on the primary,
    // whose zpos is fixed at 0, below the cursor.
    for board in BOARDS {
        let mut caps = board.recorded();
        let planes = caps.planes;
        caps.planes.clear();
        for plane in planes.iter().filter(|p| p.kind != PlaneKind::Overlay) {
            caps.planes.push(*plane).unwrap();
        }
        board.profile.apply(&mut caps);
        assert_eq!(caps.planes.as_slice()[0].zpos, Zpos::Fixed(0));

        let scaled_bottom = [window(RGB, 0, (32, 32)), unscaled(RGB, 1)];
        assert!(run(&caps, &scaled_bottom).is_ok(), "{}", board.name);

        let scaled_top = [unscaled(RGB, 0), window(RGB, 1, (32, 32))];
        let Err(PlanError::NoPlane { layer: 1, reasons }) = run(&caps, &scaled_top) else {
            panic!("{}: the scaled top layer must be refused", board.name);
        };
        assert!(reasons.contains(RejectReasons::Z_ORDER), "{}", board.name);
    }
}
