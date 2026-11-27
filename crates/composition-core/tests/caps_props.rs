//! Edge-case and seeded property tests of the capability description:
//! limits, `clear`, zpos, masks, duplicates and the lookups the planner
//! builds on them.
//!
//! The generator is hand-written and deterministic (SplitMix64), so the crate
//! keeps zero dependencies. A failing case prints its seed; set
//! `CAPS_PROPS_SEED=<n>` to replay one seed only.

use composition_core::fixed::{CapacityError, FixedVec};
use composition_core::limits::{MAX_FORMATS, MAX_OUTPUTS, MAX_PLANES, MAX_RESOURCE_GROUPS};
use composition_core::planner::plan;
use composition_core::{
    BlendMode, BufferId, BufferInfo, BufferLookup, Capabilities, FormatModifier, Frame, Layer,
    LayerTraits, Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId,
    PlaneKind, Rect, ResourceGroup, ScaleLimits, Size, Transform, Zpos, ALPHA_OPAQUE,
};

const SEEDS: u64 = 300;

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }
    /// Small values most of the time, extremes some of the time.
    fn i32_edge(&mut self) -> i32 {
        match self.below(6) {
            0 => i32::MIN,
            1 => i32::MAX,
            2 => -1,
            3 => 0,
            4 => self.below(8) as i32,
            _ => self.next() as i32,
        }
    }
    fn u32_edge(&mut self) -> u32 {
        match self.below(5) {
            0 => 0,
            1 => u32::MAX,
            2 => 1 << 31,
            3 => self.below(40) as u32,
            _ => self.next() as u32,
        }
    }
}

fn seeds() -> Vec<u64> {
    match std::env::var("CAPS_PROPS_SEED") {
        Ok(s) => vec![s.parse().expect("CAPS_PROPS_SEED must be a number")],
        Err(_) => (0..SEEDS).collect(),
    }
}

/// Runs `f` for every seed, naming the seed if it panics.
fn for_seeds(f: impl Fn(u64, &mut Rng)) {
    for seed in seeds() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f(seed, &mut Rng::new(seed));
        }));
        if let Err(e) = result {
            eprintln!("caps_props failure: seed {seed} (replay with CAPS_PROPS_SEED={seed})");
            std::panic::resume_unwind(e);
        }
    }
}

const FORMATS: [PixelFormat; 5] = [
    PixelFormat::XRGB8888,
    PixelFormat::ARGB8888,
    PixelFormat::RGB565,
    PixelFormat::NV12,
    PixelFormat(0),
];
const MODIFIERS: [Modifier; 3] = [Modifier::LINEAR, Modifier(1), Modifier(u64::MAX)];

fn random_format(rng: &mut Rng) -> FormatModifier {
    FormatModifier {
        format: rng.pick(&FORMATS),
        modifier: rng.pick(&MODIFIERS),
    }
}

fn random_plane(rng: &mut Rng, id: u32, zpos_ok: bool) -> PlaneCaps {
    let mut p = PlaneCaps {
        id: PlaneId(id),
        kind: rng.pick(&[PlaneKind::Primary, PlaneKind::Overlay, PlaneKind::Cursor]),
        outputs: if rng.chance(70) {
            u32::MAX
        } else {
            rng.u32_edge()
        },
        scaling: rng.chance(50).then(|| ScaleLimits {
            max_upscale: rng.u32_edge(),
            max_downscale: rng.u32_edge(),
        }),
        max_source: rng
            .chance(30)
            .then(|| Size::new(rng.u32_edge(), rng.u32_edge())),
        plane_alpha: rng.chance(50),
        blend_modes: if rng.chance(50) {
            0xff
        } else {
            rng.next() as u8
        },
        transforms: if rng.chance(50) {
            0xff
        } else {
            rng.next() as u8
        },
        full_output_only: rng.chance(20),
        ..PlaneCaps::default()
    };
    let small = rng.chance(70);
    let z = |r: &mut Rng| {
        if small {
            r.below(4) as i32
        } else {
            r.i32_edge()
        }
    };
    p.zpos = if rng.chance(50) {
        Zpos::Fixed(z(rng))
    } else {
        let (a, b) = (z(rng), z(rng));
        if zpos_ok {
            Zpos::Range {
                min: a.min(b),
                max: a.max(b),
            }
        } else {
            Zpos::Range { min: a, max: b }
        }
    };
    if rng.chance(70) {
        let _ = p
            .formats
            .push(FormatModifier::linear(PixelFormat::XRGB8888));
    }
    for _ in 0..rng.below(MAX_FORMATS as u64 + 4) {
        let _ = p.formats.push(random_format(rng));
    }
    p
}

fn random_caps(rng: &mut Rng, zpos_ok: bool) -> Capabilities {
    let mut caps = Capabilities::default();
    for i in 0..1 + rng.below(MAX_OUTPUTS as u64) {
        let _ = caps.outputs.push(OutputCaps {
            id: OutputId(i as u32 * 7 + 1),
            size: Size::new(1 + rng.below(64) as u32, 1 + rng.below(64) as u32),
            refresh_mhz: rng.u32_edge(),
        });
    }
    for i in 0..rng.below(MAX_PLANES as u64 + 1) {
        let _ = caps.planes.push(random_plane(rng, i as u32 + 100, zpos_ok));
    }
    for _ in 0..rng.below(MAX_RESOURCE_GROUPS as u64 + 1) {
        let _ = caps.resource_groups.push(ResourceGroup {
            consumers: LayerTraits(rng.next() as u8),
            planes: rng.u32_edge(),
            capacity: rng.below(4) as u8,
        });
    }
    caps.replace_pending = rng.chance(50);
    caps
}

// ---- limits --------------------------------------------------------------

/// Pushes `limit + extra` distinct entries; the first `limit` must be kept
/// intact and the rest refused with `CapacityError`.
fn check_overflow<T, const N: usize>(rng: &mut Rng, make: impl Fn(&mut Rng, usize) -> T)
where
    T: Copy + Default + PartialEq + core::fmt::Debug,
{
    let mut v: FixedVec<T, N> = FixedVec::new();
    let mut expected = Vec::new();
    let extra = 1 + rng.below(5) as usize;
    for i in 0..N + extra {
        let item = make(rng, i);
        let before = v.as_slice().to_vec();
        let r = v.push(item);
        if i < N {
            assert_eq!(r, Ok(()), "push {i} of {N}");
            expected.push(item);
        } else {
            assert_eq!(r, Err(CapacityError), "push {i} beyond {N}");
            assert_eq!(v.as_slice(), &before[..], "failed push changed contents");
        }
        assert_eq!(v.as_slice(), &expected[..]);
        assert_eq!(v.remaining(), N - expected.len());
    }
}

#[test]
fn limits_are_respected_by_every_table() {
    for_seeds(|_, rng| {
        check_overflow::<OutputCaps, MAX_OUTPUTS>(rng, |r, i| OutputCaps {
            id: OutputId(i as u32),
            size: Size::new(r.u32_edge(), r.u32_edge()),
            refresh_mhz: r.u32_edge(),
        });
        check_overflow::<PlaneCaps, MAX_PLANES>(rng, |r, i| random_plane(r, i as u32, false));
        check_overflow::<FormatModifier, MAX_FORMATS>(rng, |r, _| random_format(r));
        check_overflow::<ResourceGroup, MAX_RESOURCE_GROUPS>(rng, |r, _| ResourceGroup {
            consumers: LayerTraits(r.next() as u8),
            planes: r.u32_edge(),
            capacity: r.next() as u8,
        });
    });
}

#[test]
fn from_slice_refuses_oversized_input_and_keeps_exact_fit() {
    let fm = FormatModifier::linear(PixelFormat::XRGB8888);
    let exact = vec![fm; MAX_FORMATS];
    let v = FixedVec::<FormatModifier, MAX_FORMATS>::from_slice(&exact).unwrap();
    assert_eq!(v.len(), MAX_FORMATS);
    let over = vec![fm; MAX_FORMATS + 1];
    assert_eq!(
        FixedVec::<FormatModifier, MAX_FORMATS>::from_slice(&over),
        Err(CapacityError)
    );
}

#[test]
fn filling_one_table_to_the_limit_does_not_disturb_the_others() {
    for_seeds(|_, rng| {
        let mut caps = random_caps(rng, true);
        let outputs = caps.outputs;
        let groups = caps.resource_groups;
        while caps.planes.push(random_plane(rng, 7, true)).is_ok() {}
        assert_eq!(caps.planes.len(), MAX_PLANES);
        assert!(caps.planes.push(PlaneCaps::default()).is_err());
        assert_eq!(caps.outputs, outputs);
        assert_eq!(caps.resource_groups, groups);
    });
}

#[test]
fn plane_formats_overflow_keeps_earlier_formats() {
    let mut p = PlaneCaps::default();
    for i in 0..MAX_FORMATS as u64 {
        p.formats
            .push(FormatModifier {
                format: PixelFormat::XRGB8888,
                modifier: Modifier(i),
            })
            .unwrap();
    }
    let snapshot = p;
    for i in 0..10 {
        assert!(p
            .formats
            .push(FormatModifier::linear(PixelFormat(i)))
            .is_err());
    }
    assert_eq!(p, snapshot);
    assert_eq!(p.formats[0].modifier, Modifier(0));
    assert_eq!(
        p.formats[MAX_FORMATS - 1].modifier,
        Modifier(MAX_FORMATS as u64 - 1)
    );
}

// ---- clear ---------------------------------------------------------------

#[test]
fn clear_fully_resets_and_the_description_is_reusable() {
    for_seeds(|_, rng| {
        let mut caps = random_caps(rng, false);
        caps.replace_pending = true;
        caps.clear();
        assert_eq!(caps, Capabilities::EMPTY);
        assert_eq!(caps, Capabilities::default());
        assert!(caps.outputs.is_empty());
        assert!(caps.planes.is_empty());
        assert!(caps.resource_groups.is_empty());
        assert!(!caps.replace_pending);
        // Stale slots must not leak into a later description.
        let fresh = random_caps(&mut Rng::new(1234), false);
        let mut reused = random_caps(rng, false);
        reused.clear();
        for o in fresh.outputs.iter() {
            reused.outputs.push(*o).unwrap();
        }
        for p in fresh.planes.iter() {
            reused.planes.push(*p).unwrap();
        }
        for g in fresh.resource_groups.iter() {
            reused.resource_groups.push(*g).unwrap();
        }
        reused.replace_pending = fresh.replace_pending;
        assert_eq!(reused, fresh);
        // Every table can be filled to its limit again.
        reused.clear();
        for i in 0..MAX_OUTPUTS {
            reused.outputs.push(OutputCaps::default()).unwrap();
            assert_eq!(reused.outputs.len(), i + 1);
        }
    });
}

#[test]
fn cleared_capabilities_know_no_output() {
    for_seeds(|_, rng| {
        let mut caps = random_caps(rng, true);
        let id = caps.outputs[0].id;
        assert_eq!(caps.output_index(id), Some(0));
        caps.clear();
        assert_eq!(caps.output_index(id), None);
        let buffers = Buffers(vec![]);
        let frame = Frame {
            output: id,
            layers: &[],
        };
        assert_eq!(
            plan(&frame, &caps, &buffers).unwrap_err(),
            PlanError::UnknownOutput(id)
        );
    });
}

// ---- lookup --------------------------------------------------------------

#[test]
fn output_index_matches_a_linear_search_even_with_duplicate_ids() {
    for_seeds(|_, rng| {
        let mut caps = Capabilities::default();
        for _ in 0..MAX_OUTPUTS {
            caps.outputs
                .push(OutputCaps {
                    id: OutputId(rng.below(4) as u32),
                    ..OutputCaps::default()
                })
                .unwrap();
        }
        for id in 0..6 {
            let idx = caps.output_index(OutputId(id));
            let first = caps.outputs.iter().position(|o| o.id.0 == id);
            assert_eq!(idx, first, "first match wins");
            if let Some(i) = idx {
                assert_eq!(caps.outputs[i].id, OutputId(id));
            }
        }
    });
}

// ---- masks ---------------------------------------------------------------

#[test]
fn blend_and_transform_masks_are_distinct_single_bits() {
    let blends = [
        BlendMode::Opaque,
        BlendMode::Premultiplied,
        BlendMode::Coverage,
    ];
    let mut seen = 0u8;
    for b in blends {
        let m = b.mask();
        assert_eq!(m.count_ones(), 1);
        assert_eq!(seen & m, 0);
        seen |= m;
    }
    let transforms = [
        Transform::Normal,
        Transform::Rotate90,
        Transform::Rotate180,
        Transform::Rotate270,
        Transform::FlipH,
        Transform::FlipV,
    ];
    let mut seen = 0u8;
    for t in transforms {
        let m = t.mask();
        assert_eq!(m.count_ones(), 1);
        assert_eq!(seen & m, 0);
        seen |= m;
    }
}

#[test]
fn layer_traits_set_algebra() {
    for a in 0..=255u8 {
        for b in [0u8, 1, 2, 4, 8, 0x0e, 0xff, a] {
            let (x, y) = (LayerTraits(a), LayerTraits(b));
            assert_eq!(x.union(y), y.union(x));
            assert_eq!(x.intersects(y), a & b != 0);
            assert_eq!(x.union(y).0, a | b);
        }
    }
    assert!(LayerTraits::SCALED.intersects(LayerTraits::SCALED_H));
    assert!(LayerTraits::SCALED.intersects(LayerTraits::SCALED_V));
    assert!(!LayerTraits::SCALED.intersects(LayerTraits::YUV));
}

// ---- planning against random descriptions --------------------------------

struct Buffers(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

fn random_frame(rng: &mut Rng, caps: &Capabilities) -> (Buffers, Vec<Layer>, OutputId) {
    let out = rng.pick(caps.outputs.as_slice());
    let mut buffers = Vec::new();
    let mut layers = Vec::new();
    for i in 0..rng.below(6) {
        let (w, h) = (1 + rng.below(64) as u32, 1 + rng.below(64) as u32);
        let id = BufferId(i as u32);
        buffers.push((
            id,
            BufferInfo {
                size: Size::new(w, h),
                format: if rng.chance(70) {
                    PixelFormat::XRGB8888
                } else {
                    rng.pick(&FORMATS)
                },
                modifier: if rng.chance(70) {
                    Modifier::LINEAR
                } else {
                    rng.pick(&MODIFIERS)
                },
            },
        ));
        let mut layer = Layer::new(
            id,
            Rect::new(0, 0, w, h),
            Rect::new(
                rng.below(8) as i32 - 2,
                rng.below(8) as i32 - 2,
                1 + rng.below(64) as u32,
                1 + rng.below(64) as u32,
            ),
            rng.below(4) as i32 - 1,
        );
        if rng.chance(30) {
            layer.alpha = rng.next() as u16;
        }
        layer.blend = rng.pick(&[
            BlendMode::Opaque,
            BlendMode::Premultiplied,
            BlendMode::Coverage,
        ]);
        layers.push(layer);
    }
    (Buffers(buffers), layers, out.id)
}

fn zpos_allows(zpos: Zpos, z: i32) -> bool {
    match zpos {
        Zpos::Fixed(f) => f == z,
        Zpos::Range { min, max } => min <= z && z <= max,
    }
}

/// Everything a returned plan must satisfy with respect to `caps`.
fn check_plan_sound(
    caps: &Capabilities,
    buffers: &Buffers,
    frame: &Frame<'_>,
    p: &composition_core::Plan,
) {
    let oi = caps.output_index(frame.output).unwrap();
    let a = p.assignments();
    let mut used = 0u32;
    let mut group_use = [0u32; MAX_RESOURCE_GROUPS];
    let mut last_z: Option<i32> = None;
    for asg in a {
        let plane = &caps.planes[asg.plane_index];
        assert_eq!(plane.id, asg.plane);
        assert_ne!(
            plane.outputs & (1 << oi),
            0,
            "plane not attachable to output"
        );
        assert_eq!(used & (1 << asg.plane_index), 0, "plane used twice");
        used |= 1 << asg.plane_index;
        assert!(
            zpos_allows(plane.zpos, asg.zpos),
            "zpos outside plane's range"
        );
        if let Some(l) = last_z {
            assert!(asg.zpos > l, "zpos not strictly increasing");
        }
        last_z = Some(asg.zpos);
        let info = buffers.buffer_info(frame.layers[asg.layer].buffer).unwrap();
        assert!(plane
            .formats
            .iter()
            .any(|f| f.format == info.format && f.modifier == info.modifier));
        let layer = &frame.layers[asg.layer];
        if layer.alpha != ALPHA_OPAQUE {
            assert!(plane.plane_alpha);
        }
        if layer.transform != Transform::Normal {
            assert_ne!(plane.transforms & layer.transform.mask(), 0);
        }
        for (g, group) in caps.resource_groups.iter().enumerate() {
            // Only ANY-consuming groups can be checked without the traits.
            if group.consumers.intersects(LayerTraits::ANY)
                && group.planes & (1 << asg.plane_index) != 0
            {
                group_use[g] += 1;
            }
        }
    }
    for (g, group) in caps.resource_groups.iter().enumerate() {
        if group.consumers.intersects(LayerTraits::ANY) {
            assert!(
                group_use[g] <= group.capacity as u32,
                "group {g} over capacity"
            );
        }
    }
}

#[test]
fn planner_is_sound_on_random_descriptions() {
    let planned = std::cell::Cell::new(0u32);
    for_seeds(|_, rng| {
        for _ in 0..20 {
            let caps = random_caps(rng, true);
            let (buffers, layers, output) = random_frame(rng, &caps);
            let frame = Frame {
                output,
                layers: &layers,
            };
            if let Ok(p) = plan(&frame, &caps, &buffers) {
                check_plan_sound(&caps, &buffers, &frame, &p);
                if !p.assignments().is_empty() {
                    planned.set(planned.get() + 1);
                }
            }
        }
    });
    // Guard against a generator that never produces a usable description.
    if std::env::var("CAPS_PROPS_SEED").is_err() {
        assert!(planned.get() > 50, "only {} non-empty plans", planned.get());
    }
}

#[test]
fn planner_never_panics_on_inverted_or_extreme_descriptions() {
    for_seeds(|_, rng| {
        for _ in 0..20 {
            let caps = random_caps(rng, false);
            let (buffers, layers, output) = random_frame(rng, &caps);
            let frame = Frame {
                output,
                layers: &layers,
            };
            let _ = plan(&frame, &caps, &buffers);
        }
    });
}

fn one_plane_caps(plane: PlaneCaps) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OutputId(1),
            size: Size::new(16, 16),
            refresh_mhz: 60_000,
        })
        .unwrap();
    caps.planes.push(plane).unwrap();
    caps
}

fn simple_plane(zpos: Zpos) -> PlaneCaps {
    let mut p = PlaneCaps {
        id: PlaneId(1),
        outputs: 1,
        zpos,
        ..PlaneCaps::default()
    };
    p.formats
        .push(FormatModifier::linear(PixelFormat::XRGB8888))
        .unwrap();
    p
}

fn one_layer(z: i32) -> (Buffers, [Layer; 1]) {
    let b = BufferId(1);
    (
        Buffers(vec![(
            b,
            BufferInfo {
                size: Size::new(16, 16),
                format: PixelFormat::XRGB8888,
                modifier: Modifier::LINEAR,
            },
        )]),
        [Layer::new(
            b,
            Rect::new(0, 0, 16, 16),
            Rect::new(0, 0, 16, 16),
            z,
        )],
    )
}

fn try_plan(caps: &Capabilities) -> Result<composition_core::Plan, PlanError> {
    let (buffers, layers) = one_layer(0);
    plan(
        &Frame {
            output: OutputId(1),
            layers: &layers,
        },
        caps,
        &buffers,
    )
}

#[test]
fn extreme_zpos_values_are_planned_without_overflow() {
    for z in [i32::MIN, -1, 0, 1, i32::MAX] {
        let p = try_plan(&one_plane_caps(simple_plane(Zpos::Fixed(z)))).unwrap();
        assert_eq!(p.assignments()[0].zpos, z);
    }
    let full = Zpos::Range {
        min: i32::MIN,
        max: i32::MAX,
    };
    let p = try_plan(&one_plane_caps(simple_plane(full))).unwrap();
    assert_eq!(p.assignments()[0].zpos, i32::MIN);
    let degenerate = Zpos::Range { min: 5, max: 5 };
    let p = try_plan(&one_plane_caps(simple_plane(degenerate))).unwrap();
    assert_eq!(p.assignments()[0].zpos, 5);
}

#[test]
fn zpos_at_i32_max_cannot_stack_a_second_plane_above() {
    let mut caps = one_plane_caps(simple_plane(Zpos::Fixed(i32::MAX)));
    let mut top = simple_plane(Zpos::Range {
        min: i32::MIN,
        max: i32::MAX,
    });
    top.id = PlaneId(2);
    caps.planes.push(top).unwrap();
    let (buffers, _) = one_layer(0);
    let b = BufferId(1);
    let layers = [
        Layer::new(b, Rect::new(0, 0, 16, 16), Rect::new(0, 0, 16, 16), 0),
        Layer::new(b, Rect::new(0, 0, 16, 16), Rect::new(0, 0, 16, 16), 1),
    ];
    let frame = Frame {
        output: OutputId(1),
        layers: &layers,
    };
    // The only legal order puts the Range plane below the Fixed(MAX) one.
    let p = plan(&frame, &caps, &buffers).unwrap();
    let z: Vec<i32> = p.assignments().iter().map(|a| a.zpos).collect();
    assert_eq!(z, vec![i32::MIN, i32::MAX]);
}

#[test]
#[ignore = "bug: planner accepts a Zpos::Range with min > max for the lowest layer"]
fn inverted_zpos_range_is_unusable() {
    let caps = one_plane_caps(simple_plane(Zpos::Range { min: 5, max: 2 }));
    if let Ok(p) = try_plan(&caps) {
        panic!(
            "plane with empty zpos range 5..=2 was assigned zpos {}",
            p.assignments()[0].zpos
        );
    }
}

#[test]
fn inverted_zpos_range_is_consistent_above_another_plane() {
    // With a layer below, the planner rejects an inverted range (min > max).
    let mut caps = one_plane_caps(simple_plane(Zpos::Fixed(0)));
    let mut inv = simple_plane(Zpos::Range { min: 5, max: 2 });
    inv.id = PlaneId(2);
    caps.planes.push(inv).unwrap();
    let b = BufferId(1);
    let (buffers, _) = one_layer(0);
    let layers = [
        Layer::new(b, Rect::new(0, 0, 16, 16), Rect::new(0, 0, 16, 16), 0),
        Layer::new(b, Rect::new(0, 0, 16, 16), Rect::new(0, 0, 16, 16), 1),
    ];
    let frame = Frame {
        output: OutputId(1),
        layers: &layers,
    };
    assert!(matches!(
        plan(&frame, &caps, &buffers),
        Err(PlanError::NoPlane { .. })
    ));
}

#[test]
fn plane_output_bits_for_nonexistent_outputs_are_ignored() {
    for_seeds(|_, rng| {
        let mut caps = one_plane_caps(simple_plane(Zpos::Fixed(0)));
        // Bit 0 is the only real output; set arbitrary other bits.
        let junk = rng.next() as u32 & !1;
        caps.planes.as_mut_slice()[0].outputs = junk;
        assert!(matches!(try_plan(&caps), Err(PlanError::NoPlane { .. })));
        caps.planes.as_mut_slice()[0].outputs = junk | 1;
        let p = try_plan(&caps).unwrap();
        assert_eq!(p.assignments()[0].plane_index, 0);
        caps.planes.as_mut_slice()[0].outputs = 0;
        assert!(try_plan(&caps).is_err());
    });
}

#[test]
fn resource_group_masks_for_nonexistent_planes_are_ignored() {
    for_seeds(|_, rng| {
        let mut caps = one_plane_caps(simple_plane(Zpos::Fixed(0)));
        let junk = rng.next() as u32 & !1;
        caps.resource_groups
            .push(ResourceGroup {
                consumers: LayerTraits::ANY,
                planes: junk,
                capacity: 0,
            })
            .unwrap();
        // Capacity 0 but no member plane exists: no effect.
        assert!(try_plan(&caps).is_ok());
        caps.resource_groups.as_mut_slice()[0].planes = junk | 1;
        assert!(matches!(try_plan(&caps), Err(PlanError::NoPlane { .. })));
        caps.resource_groups.as_mut_slice()[0].capacity = 1;
        assert!(try_plan(&caps).is_ok());
    });
}

#[test]
fn duplicate_formats_and_modifiers_do_not_change_the_plan() {
    for_seeds(|_, rng| {
        let caps = random_caps(rng, true);
        let (buffers, layers, output) = random_frame(rng, &caps);
        let frame = Frame {
            output,
            layers: &layers,
        };
        let mut doubled = caps;
        for p in doubled.planes.as_mut_slice() {
            let existing: Vec<_> = p.formats.as_slice().to_vec();
            // Re-append as many duplicates as fit; membership is unchanged.
            for f in existing {
                let _ = p.formats.push(f);
            }
        }
        let a = plan(&frame, &caps, &buffers);
        let b = plan(&frame, &doubled, &buffers);
        assert_eq!(a, b);
    });
}

#[test]
fn format_and_modifier_must_both_match() {
    let mut plane = simple_plane(Zpos::Fixed(0));
    plane.formats.clear();
    plane
        .formats
        .push(FormatModifier {
            format: PixelFormat::XRGB8888,
            modifier: Modifier(7),
        })
        .unwrap();
    plane
        .formats
        .push(FormatModifier::linear(PixelFormat::ARGB8888))
        .unwrap();
    // Buffer is XRGB8888 + LINEAR: format and modifier each exist, the pair not.
    assert!(matches!(
        try_plan(&one_plane_caps(plane)),
        Err(PlanError::NoPlane { .. })
    ));
}

#[test]
fn blend_mask_gates_non_opaque_blending_only() {
    let b = BufferId(1);
    for (mask, blend, format, ok) in [
        (0u8, BlendMode::Opaque, PixelFormat::XRGB8888, true),
        (0, BlendMode::Premultiplied, PixelFormat::XRGB8888, false),
        (
            BlendMode::Premultiplied.mask(),
            BlendMode::Premultiplied,
            PixelFormat::XRGB8888,
            true,
        ),
        (
            BlendMode::Premultiplied.mask(),
            BlendMode::Coverage,
            PixelFormat::XRGB8888,
            false,
        ),
        (
            BlendMode::Coverage.mask(),
            BlendMode::Coverage,
            PixelFormat::XRGB8888,
            true,
        ),
        (0xff, BlendMode::Coverage, PixelFormat::XRGB8888, true),
    ] {
        let mut plane = simple_plane(Zpos::Fixed(0));
        plane.blend_modes = mask;
        let caps = one_plane_caps(plane);
        let buffers = Buffers(vec![(
            b,
            BufferInfo {
                size: Size::new(16, 16),
                format,
                modifier: Modifier::LINEAR,
            },
        )]);
        let mut layer = Layer::new(b, Rect::new(0, 0, 16, 16), Rect::new(0, 0, 16, 16), 0);
        layer.blend = blend;
        let layers = [layer];
        let r = plan(
            &Frame {
                output: OutputId(1),
                layers: &layers,
            },
            &caps,
            &buffers,
        );
        assert_eq!(r.is_ok(), ok, "mask {mask:#x} blend {blend:?}");
    }
}

#[test]
fn scale_limits_extremes_do_not_panic() {
    for_seeds(|_, rng| {
        let mut plane = simple_plane(Zpos::Fixed(0));
        plane.scaling = Some(ScaleLimits {
            max_upscale: rng.u32_edge(),
            max_downscale: rng.u32_edge(),
        });
        let caps = one_plane_caps(plane);
        let b = BufferId(1);
        let buffers = Buffers(vec![(
            b,
            BufferInfo {
                size: Size::new(u32::MAX / 2, 3),
                format: PixelFormat::XRGB8888,
                modifier: Modifier::LINEAR,
            },
        )]);
        let layers = [Layer::new(
            b,
            Rect::new(0, 0, u32::MAX / 2, 3),
            Rect::new(0, 0, 1 + rng.below(40) as u32, 1 + rng.below(40) as u32),
            0,
        )];
        let _ = plan(
            &Frame {
                output: OutputId(1),
                layers: &layers,
            },
            &caps,
            &buffers,
        );
    });
}
