//! Deterministic fuzz of the device-free code `composition-drm` exposes.
//!
//! The crate's pure parsers (`parse_in_formats`, the property lookups, the
//! errno classification) are private and have no public entry that does not
//! need an open DRM device, and `src/` is not to be changed for this test.
//! What is public and device-free is [`DriverProfile::apply`] on
//! [`Capabilities`], and the planner that consumes its result. Those are
//! fuzzed here with a hand-written SplitMix64 generator and fixed seeds, on
//! inconsistent inputs: plane masks and groups naming planes that do not
//! exist, empty and full tables, zero and extreme limits, repeated
//! applications, frames with out-of-range rectangles.
//!
//! No DRM device is opened.

use composition_core::limits::{MAX_FORMATS, MAX_PLANES, MAX_RESOURCE_GROUPS};
use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, FormatModifier, Frame, Layer, LayerTraits,
    Modifier, OutputCaps, OutputId, PixelFormat, PlaneCaps, PlaneId, PlaneKind, Rect,
    ResourceGroup, ScaleLimits, Size, Zpos,
};
use composition_drm::DriverProfile;

const PROFILES: [DriverProfile; 6] = [
    DriverProfile::Generic,
    DriverProfile::Imx95Dpu,
    DriverProfile::Stm32Ltdc,
    DriverProfile::Rk3588Vop2,
    DriverProfile::Vc4Hvs5,
    DriverProfile::Vc4Hvs6,
];
const SEEDS: [u64; 4] = [0, 1, 0x5eed_0fd1, u64::MAX];
const ROUNDS: usize = 800;

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// Mostly small values, sometimes the extremes.
    fn edge_u32(&mut self) -> u32 {
        match self.below(6) {
            0 => 0,
            1 => 1,
            2 => u32::MAX,
            3 => i32::MAX as u32,
            4 => self.next() as u32,
            _ => self.below(4096) as u32,
        }
    }

    fn edge_i32(&mut self) -> i32 {
        match self.below(6) {
            0 => 0,
            1 => i32::MIN,
            2 => i32::MAX,
            3 => -(self.below(4096) as i32),
            _ => self.below(4096) as i32,
        }
    }
}

fn random_kind(rng: &mut SplitMix64) -> PlaneKind {
    match rng.below(3) {
        0 => PlaneKind::Primary,
        1 => PlaneKind::Overlay,
        _ => PlaneKind::Cursor,
    }
}

fn random_plane(rng: &mut SplitMix64) -> PlaneCaps {
    let mut plane = PlaneCaps {
        id: PlaneId(rng.next() as u32),
        kind: random_kind(rng),
        // Bits beyond the output table are legal input from a broken driver.
        outputs: rng.next() as u32,
        plane_alpha: rng.below(2) == 0,
        blend_modes: rng.next() as u8,
        transforms: rng.next() as u8,
        full_output_only: rng.below(4) == 0,
        zpos: if rng.below(2) == 0 {
            Zpos::Fixed(rng.edge_i32())
        } else {
            Zpos::Range {
                min: rng.edge_i32(),
                max: rng.edge_i32(),
            }
        },
        ..PlaneCaps::default()
    };
    if rng.below(2) == 0 {
        plane.scaling = Some(ScaleLimits {
            max_upscale: rng.edge_u32(),
            max_downscale: rng.edge_u32(),
        });
    }
    if rng.below(2) == 0 {
        plane.max_source = Some(Size::new(rng.edge_u32(), rng.edge_u32()));
    }
    // Overfilling is refused by the table, never a panic.
    for _ in 0..rng.below(MAX_FORMATS as u64 + 4) {
        let modifier = if rng.below(2) == 0 {
            Modifier::LINEAR
        } else {
            Modifier(rng.next())
        };
        let _ = plane.formats.push(FormatModifier {
            format: PixelFormat(rng.next() as u32),
            modifier,
        });
    }
    plane
}

fn random_caps(rng: &mut SplitMix64) -> Capabilities {
    let mut caps = Capabilities::default();
    for _ in 0..rng.below(MAX_PLANES as u64 + 3) {
        let _ = caps.planes.push(random_plane(rng));
    }
    for i in 0..rng.below(6) {
        let _ = caps.outputs.push(OutputCaps {
            id: OutputId(i as u32),
            size: Size::new(rng.edge_u32(), rng.edge_u32()),
            refresh_mhz: rng.edge_u32(),
        });
    }
    for _ in 0..rng.below(MAX_RESOURCE_GROUPS as u64 + 3) {
        let _ = caps.resource_groups.push(ResourceGroup {
            consumers: LayerTraits(rng.next() as u8),
            // May name planes that do not exist.
            planes: rng.next() as u32,
            capacity: rng.next() as u8,
        });
    }
    caps.replace_pending = rng.below(2) == 0;
    caps
}

struct RandomBuffers(u64);

impl BufferLookup for RandomBuffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let mut rng = SplitMix64(self.0 ^ u64::from(buffer.0));
        if rng.below(8) == 0 {
            return None;
        }
        Some(BufferInfo {
            size: Size::new(rng.edge_u32(), rng.edge_u32()),
            format: PixelFormat(rng.next() as u32),
            modifier: Modifier(rng.next()),
        })
    }
}

fn random_rect(rng: &mut SplitMix64) -> Rect {
    Rect::new(
        rng.edge_i32(),
        rng.edge_i32(),
        rng.edge_u32(),
        rng.edge_u32(),
    )
}

fn random_layer(rng: &mut SplitMix64) -> Layer {
    let mut layer = Layer::new(
        BufferId(rng.below(6) as u32),
        random_rect(rng),
        random_rect(rng),
        rng.edge_i32(),
    );
    layer.alpha = rng.next() as u16;
    layer.blend = match rng.below(3) {
        0 => BlendMode::default(),
        1 => BlendMode::Premultiplied,
        _ => BlendMode::Coverage,
    };
    layer
}

fn mask_of(len: usize) -> u32 {
    ((1u64 << len) - 1) as u32
}

/// What `apply` must keep whatever the input.
fn check_sane(before: &Capabilities, after: &Capabilities, profile: DriverProfile) {
    assert_eq!(after.planes.len(), before.planes.len(), "{profile:?}");
    assert_eq!(after.outputs, before.outputs, "{profile:?}");
    assert_eq!(after.replace_pending, before.replace_pending, "{profile:?}");
    assert!(after.resource_groups.len() <= MAX_RESOURCE_GROUPS);
    assert!(after.resource_groups.len() >= before.resource_groups.len());
    // Existing groups are kept as they were, in front.
    assert_eq!(
        &after.resource_groups.as_slice()[..before.resource_groups.len()],
        before.resource_groups.as_slice()
    );
    for (b, a) in before.planes.iter().zip(after.planes.iter()) {
        // A profile only touches scaling and the source limit (Vc4Hvs6 also formats).
        let mut b2 = *b;
        b2.scaling = a.scaling;
        b2.max_source = a.max_source;
        if profile == DriverProfile::Vc4Hvs6 {
            // Gen 6 also drops the pairs its kernel rejects (T-tiled, SAND64),
            // keeping the rest in order.
            let mut rest = b.formats.as_slice().iter();
            for pair in a.formats.as_slice() {
                assert!(
                    rest.any(|p| p == pair),
                    "{profile:?} formats not a subsequence"
                );
            }
            b2.formats = a.formats;
        }
        assert_eq!(&b2, a, "{profile:?} changed more than scaling/max_source");
        // Rk3588Vop2 sets scaling and max_source on cursor planes too; see
        // `rk3588_gives_a_cursor_plane_no_scaler` below.
        if b.kind == PlaneKind::Cursor && profile != DriverProfile::Rk3588Vop2 {
            assert_eq!(b.scaling, a.scaling, "{profile:?} touched a cursor plane");
            assert_eq!(b.max_source, a.max_source);
        }
    }
    if profile == DriverProfile::Generic {
        assert_eq!(before, after);
    }
    // Groups the profile added name only existing planes.
    let all = mask_of(after.planes.len());
    for g in &after.resource_groups.as_slice()[before.resource_groups.len()..] {
        assert_eq!(
            g.planes & !all,
            0,
            "{profile:?} group names a missing plane"
        );
    }
}

#[test]
fn apply_on_random_capabilities() {
    for seed in SEEDS {
        let mut rng = SplitMix64(seed);
        for _ in 0..ROUNDS {
            let before = random_caps(&mut rng);
            for profile in PROFILES {
                let mut caps = before;
                profile.apply(&mut caps);
                check_sane(&before, &caps, profile);
                let once = caps;
                profile.apply(&mut caps);
                assert_eq!(caps, once, "seed {seed}: {profile:?} not idempotent");
            }
        }
    }
}

#[test]
fn profiles_applied_in_any_order_stay_sane() {
    for seed in SEEDS {
        let mut rng = SplitMix64(seed ^ 0xabcd);
        for _ in 0..ROUNDS / 4 {
            let mut caps = random_caps(&mut rng);
            for _ in 0..rng.below(6) {
                let before = caps;
                let profile = PROFILES[rng.below(PROFILES.len() as u64) as usize];
                profile.apply(&mut caps);
                check_sane(&before, &caps, profile);
            }
        }
    }
}

#[test]
fn planner_on_profiled_random_capabilities_never_panics() {
    for seed in SEEDS {
        let mut rng = SplitMix64(seed ^ 0x1234);
        for _ in 0..ROUNDS {
            let mut caps = random_caps(&mut rng);
            PROFILES[rng.below(PROFILES.len() as u64) as usize].apply(&mut caps);
            let buffers = RandomBuffers(rng.next());
            let layers: Vec<Layer> = (0..rng.below(20)).map(|_| random_layer(&mut rng)).collect();
            let frame = Frame {
                output: OutputId(rng.below(6) as u32),
                layers: &layers,
            };
            if let Ok(plan) = plan(&frame, &caps, &buffers) {
                let mut used = 0u32;
                for a in plan.assignments() {
                    assert!(a.plane_index < caps.planes.len());
                    assert!(a.layer < layers.len());
                    assert_eq!(used & (1 << a.plane_index), 0, "plane used twice");
                    used |= 1 << a.plane_index;
                }
            }
        }
    }
}

/// Defect found by the fuzz: `Rk3588Vop2` assigns a window class to every
/// plane, cursor planes included, so a cursor plane ends up with a scaler and
/// a source limit. `Vc4Hvs5` and `Vc4Hvs6` skip cursor planes. Un-ignore when
/// `src/profile/rk3588.rs` leaves cursor planes alone.
#[test]
#[ignore = "defect: Rk3588Vop2 gives cursor planes a scaler"]
fn rk3588_gives_a_cursor_plane_no_scaler() {
    let mut caps = Capabilities::default();
    let cursor = PlaneCaps {
        id: PlaneId(1),
        kind: PlaneKind::Cursor,
        outputs: 1,
        ..PlaneCaps::default()
    };
    let _ = caps.planes.push(cursor);
    DriverProfile::Rk3588Vop2.apply(&mut caps);
    assert_eq!(caps.planes.as_slice()[0].scaling, None);
    assert_eq!(caps.planes.as_slice()[0].max_source, None);
}
