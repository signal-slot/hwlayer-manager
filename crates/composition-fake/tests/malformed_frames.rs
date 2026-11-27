//! SoW §10: fake-backend property tests for malformed frame descriptions.
//!
//! A seeded splitmix64 generator produces thousands of frame descriptions
//! against a rotating set of capability descriptions — plausible hardware,
//! and deliberately hostile descriptions (planes without formats, inverted
//! z-position ranges, zero-capacity resource groups, outputs of no pixels).
//! Frames carry zero, negative and huge sizes, rectangles whose arithmetic
//! overflows i32 and the 16.16 fixed-point scaling range, sources outside
//! their buffer, destinations partly or fully off the output, unknown or
//! unregistered buffers, formats and modifiers the planes do not list,
//! duplicate buffers and layers, more layers than the limits allow, unknown
//! output ids, alphas other than opaque, and unsupported blend modes,
//! transforms, scaling and z-positions.
//!
//! For every case the test checks that
//!
//! * [`Engine::validate`] and [`Engine::present`] never panic and never
//!   overflow (the default debug profile catches arithmetic overflow in the
//!   core),
//! * every refused frame reports an explicit error, and every accepted
//!   frame gets a [`composition_core::Plan`] whose every assignment
//!   satisfies the capabilities it was planned against: accepted
//!   format/modifier pair, plane-to-output mask, scaling within the plane's
//!   limits, source size, alpha, blend and transform support,
//!   `full_output_only`, resource group budgets, z-position inside the
//!   plane's range and strictly increasing bottom to top, destination
//!   inside the output and source inside the buffer,
//! * the assigned layers are exactly the visible ones (destinations
//!   overlapping the output), in stacking order,
//! * the same frame on an identical fresh engine yields the identical
//!   result: results are deterministic for a given seed,
//! * a refused frame leaves every buffer state, every event and the
//!   backend's commit count unchanged,
//! * a frame that `validate` accepts, `present` commits, and after the
//!   vertical blank its buffers are reported `Presented` and scanning.
//!
//! A failing case prints the seed, the case index, and the full
//! capabilities, buffers and frame. Set `COMPOSITION_FUZZ_CASES` to raise
//! the number of generated frames (default 4000, a few seconds) and
//! `COMPOSITION_FUZZ_SEED` to explore other draws.

use std::panic::{self, AssertUnwindSafe};

use composition_core::fixed::FixedVec;
use composition_core::{
    BlendMode, BufferId, BufferInfo, BufferState, Capabilities, DisplayEvent, Engine, Error,
    FormatModifier, Frame, Layer, LayerTraits, Modifier, OutputCaps, OutputId, PixelFormat, Plan,
    PlanError, PlaneCaps, PlaneId, PlaneKind, Rect, RejectReasons, ResourceGroup, ScaleLimits,
    Size, Transform, Zpos, ALPHA_OPAQUE,
};
use composition_fake::FakeBackend;

const DEFAULT_CASES: u32 = 4_000;
const DEFAULT_SEED: u64 = 0x2d59_f017_5bd1_ce15;

/// splitmix64; the same no-dependency approach the core's own tests take.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..bound`; `bound` must be at least 1.
    fn below(&mut self, bound: u32) -> u32 {
        ((self.next_u64() >> 32) as u32) % bound
    }

    fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den) < num
    }

    fn one_of<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u32) as usize]
    }

    fn signed(&mut self) -> i32 {
        (self.next_u64() >> 32) as i32
    }
}

/// Above every i32 coordinate an output can reach, below u32::MAX, and big
/// enough that clipped widths exceed i32 and products of dimensions exceed
/// i64: the ranges malformed inputs reach and the planner must survive.
const HUGE: u32 = 4_200_000_000;

const DIMENSIONS: [u32; 7] = [0, 1, 16, 101, 640, 1920, HUGE];
const FORMATS: [PixelFormat; 8] = [
    PixelFormat::XRGB8888,
    PixelFormat::XRGB8888,
    PixelFormat::ARGB8888,
    PixelFormat::RGB565,
    PixelFormat::NV12,
    PixelFormat(0),
    PixelFormat::fourcc(b"ABCD"),
    PixelFormat::ABGR8888,
];
const MODIFIERS: [u64; 5] = [0, 0, 0, 1, u64::MAX];
const BLENDS: [BlendMode; 4] = [
    BlendMode::Opaque,
    BlendMode::Premultiplied,
    BlendMode::Coverage,
    BlendMode::Opaque,
];
const TRANSFORMS: [Transform; 6] = [
    Transform::Normal,
    Transform::Rotate90,
    Transform::Rotate180,
    Transform::Rotate270,
    Transform::FlipH,
    Transform::FlipV,
];
const Z_ORDERS: [i32; 8] = [i32::MIN, -1_000_000, -7, -1, 0, 1, 7, i32::MAX];
const ALPHAS: [u16; 6] = [ALPHA_OPAQUE, ALPHA_OPAQUE, ALPHA_OPAQUE, 0, 1, 0x8000];

fn to_i32(v: u32) -> i32 {
    if v > i32::MAX as u32 {
        i32::MAX
    } else {
        v as i32
    }
}

/// A width/height pair from the interesting dimensions.
fn size(rng: &mut Rng) -> Size {
    Size::new(rng.one_of(&DIMENSIONS), rng.one_of(&DIMENSIONS))
}

fn format_list(rng: &mut Rng, count: u32) -> FixedVec<FormatModifier, 32> {
    let mut list = FixedVec::new();
    for _ in 0..count {
        let _ = list.push(FormatModifier {
            format: rng.one_of(&FORMATS),
            modifier: Modifier(rng.one_of(&MODIFIERS)),
        });
    }
    list
}

fn zpos(rng: &mut Rng) -> Zpos {
    match rng.below(4) {
        0..=1 => Zpos::Fixed(rng.one_of(&Z_ORDERS)),
        _ => {
            let min = rng.signed();
            let max = min.saturating_add(rng.below(4) as i32);
            // A tenth of the time describe an impossible range.
            if rng.chance(1, 10) {
                Zpos::Range { min: max, max: min }
            } else {
                Zpos::Range { min, max }
            }
        }
    }
}

fn scaling(rng: &mut Rng) -> Option<ScaleLimits> {
    match rng.below(4) {
        0 => None,
        1 => Some(ScaleLimits {
            max_upscale: 2 << 16,
            max_downscale: 4 << 16,
        }),
        2 => Some(ScaleLimits {
            max_upscale: u32::MAX,
            max_downscale: u32::MAX,
        }),
        _ => Some(ScaleLimits {
            max_upscale: rng.signed() as u32,
            max_downscale: rng.signed() as u32,
        }),
    }
}

fn caps_with(
    outputs: &[OutputCaps],
    planes: &[PlaneCaps],
    groups: &[ResourceGroup],
) -> Capabilities {
    let mut caps = Capabilities::default();
    for output in outputs {
        caps.outputs.push(*output).unwrap();
    }
    for plane in planes {
        caps.planes.push(*plane).unwrap();
    }
    for group in groups {
        caps.resource_groups.push(*group).unwrap();
    }
    caps
}

fn output(id: u32, width: u32, height: u32) -> OutputCaps {
    OutputCaps {
        id: OutputId(id),
        size: Size::new(width, height),
        refresh_mhz: 60_000,
    }
}

fn plane(id: u32, kind: PlaneKind, zpos: Zpos, accepted: &[FormatModifier]) -> PlaneCaps {
    PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs: 0b11,
        formats: FixedVec::from_slice(accepted).unwrap(),
        scaling: None,
        max_source: None,
        plane_alpha: true,
        blend_modes: 0b111,
        transforms: 0b111_111,
        zpos,
        full_output_only: false,
    }
}

const XRGB: FormatModifier = FormatModifier {
    format: PixelFormat::XRGB8888,
    modifier: Modifier(0),
};
const ARGB: FormatModifier = FormatModifier {
    format: PixelFormat::ARGB8888,
    modifier: Modifier(0),
};
const RGB565: FormatModifier = FormatModifier {
    format: PixelFormat::RGB565,
    modifier: Modifier(0),
};
const NV12: FormatModifier = FormatModifier {
    format: PixelFormat::NV12,
    modifier: Modifier(0),
};
const ALL: [FormatModifier; 4] = [XRGB, ARGB, RGB565, NV12];

type Template = (&'static str, fn() -> Capabilities);

/// Plausible hardware, and hardware whose description is hostile.
fn templates() -> [Template; 12] {
    [
        ("single full-output primary", || {
            let mut primary = plane(1, PlaneKind::Primary, Zpos::Fixed(0), &[XRGB, RGB565]);
            primary.full_output_only = true;
            primary.plane_alpha = false;
            primary.blend_modes = BlendMode::Opaque.mask();
            caps_with(
                std::slice::from_ref(&output(0, 1920, 1080)),
                std::slice::from_ref(&primary),
                &[],
            )
        }),
        ("application below safety layer", || {
            let bottom = plane(31, PlaneKind::Primary, Zpos::Fixed(0), &[RGB565]);
            let mut top = plane(41, PlaneKind::Overlay, Zpos::Fixed(1), &[ARGB]);
            top.blend_modes = BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask();
            caps_with(&[output(0, 1280, 720)], &[bottom, top], &[])
        }),
        ("four planes, ranges, shared scaler", || {
            let mut a = plane(
                1,
                PlaneKind::Overlay,
                Zpos::Range { min: 0, max: 3 },
                &[XRGB],
            );
            a.scaling = Some(ScaleLimits {
                max_upscale: 8 << 16,
                max_downscale: 8 << 16,
            });
            let mut b = plane(
                2,
                PlaneKind::Overlay,
                Zpos::Range { min: 2, max: 5 },
                &[ARGB],
            );
            b.scaling = Some(ScaleLimits {
                max_upscale: 2 << 16,
                max_downscale: 2 << 16,
            });
            b.plane_alpha = false;
            let mut c = plane(3, PlaneKind::Overlay, Zpos::Fixed(10), &[RGB565, XRGB]);
            c.transforms = Transform::FlipH.mask() | Transform::FlipV.mask();
            let mut d = plane(
                4,
                PlaneKind::Overlay,
                Zpos::Range { min: 0, max: 1 },
                &[NV12],
            );
            d.max_source = Some(Size::new(4096, 4096));
            let groups = [
                ResourceGroup {
                    consumers: LayerTraits::SCALED,
                    planes: 0b0011,
                    capacity: 1,
                },
                ResourceGroup {
                    consumers: LayerTraits::YUV,
                    planes: 0b1000,
                    capacity: 1,
                },
            ];
            caps_with(&[output(0, 1920, 1080)], &[a, b, c, d], &groups)
        }),
        ("two outputs, split and shared planes", || {
            let mut only_first = plane(1, PlaneKind::Overlay, Zpos::Fixed(0), &[XRGB]);
            only_first.outputs = 0b01;
            let mut only_second = plane(2, PlaneKind::Overlay, Zpos::Fixed(0), &[ARGB]);
            only_second.outputs = 0b10;
            let both = plane(3, PlaneKind::Overlay, Zpos::Fixed(1), &[XRGB, ARGB]);
            caps_with(
                &[output(0, 1920, 1080), output(1, 800, 480)],
                &[only_first, only_second, both],
                &[],
            )
        }),
        ("cursor above full primary", || {
            let mut primary = plane(1, PlaneKind::Primary, Zpos::Fixed(0), &[XRGB]);
            primary.full_output_only = true;
            let mut cursor = plane(
                2,
                PlaneKind::Cursor,
                Zpos::Range { min: 8, max: 8 },
                &[ARGB],
            );
            cursor.max_source = Some(Size::new(64, 64));
            caps_with(&[output(0, 1280, 720)], &[primary, cursor], &[])
        }),
        ("hostile z positions", || {
            let bottom = plane(1, PlaneKind::Overlay, Zpos::Fixed(i32::MIN), &[XRGB]);
            let top = plane(2, PlaneKind::Overlay, Zpos::Fixed(i32::MAX), &[XRGB]);
            let inverted = plane(
                3,
                PlaneKind::Overlay,
                Zpos::Range { min: 5, max: 2 },
                &[ARGB],
            );
            let single = plane(
                4,
                PlaneKind::Overlay,
                Zpos::Range { min: 1, max: 1 },
                &[ARGB],
            );
            caps_with(
                &[output(0, 1920, 1080)],
                &[bottom, top, inverted, single],
                &[],
            )
        }),
        ("planes without any accepted format", || {
            let a = plane(1, PlaneKind::Overlay, Zpos::Fixed(0), &[]);
            let b = plane(2, PlaneKind::Overlay, Zpos::Fixed(1), &[]);
            caps_with(&[output(0, 1920, 1080)], &[a, b], &[])
        }),
        ("planes that reach no output", || {
            let mut a = plane(1, PlaneKind::Overlay, Zpos::Fixed(0), &[XRGB]);
            a.outputs = 0;
            let mut b = plane(2, PlaneKind::Overlay, Zpos::Fixed(1), &[ARGB]);
            b.outputs = 0;
            caps_with(&[output(0, 1920, 1080)], &[a, b], &[])
        }),
        ("resource group nobody may join", || {
            let wide = ScaleLimits {
                max_upscale: u32::MAX,
                max_downscale: u32::MAX,
            };
            let mut a = plane(1, PlaneKind::Overlay, Zpos::Fixed(0), &[XRGB, ARGB]);
            a.scaling = Some(wide);
            let mut b = plane(2, PlaneKind::Overlay, Zpos::Fixed(1), &[XRGB, ARGB]);
            b.scaling = Some(wide);
            let groups = [ResourceGroup {
                consumers: LayerTraits::ANY,
                planes: 0b11,
                capacity: 0,
            }];
            caps_with(&[output(0, 1920, 1080)], &[a, b], &groups)
        }),
        ("output without a single pixel", || {
            let a = plane(1, PlaneKind::Overlay, Zpos::Fixed(0), &[XRGB]);
            caps_with(&[output(0, 0, 0)], std::slice::from_ref(&a), &[])
        }),
        ("huge output, unlimited limits", || {
            let mut a = plane(1, PlaneKind::Overlay, Zpos::Range { min: 0, max: 2 }, &ALL);
            a.scaling = Some(ScaleLimits {
                max_upscale: u32::MAX,
                max_downscale: u32::MAX,
            });
            a.max_source = Some(Size::new(HUGE, HUGE));
            caps_with(&[output(0, HUGE, HUGE)], std::slice::from_ref(&a), &[])
        }),
        ("nothing attached", Capabilities::default),
    ]
}

fn random_capabilities(rng: &mut Rng) -> Capabilities {
    let mut caps = Capabilities {
        replace_pending: rng.chance(1, 2),
        ..Capabilities::default()
    };
    for i in 0..1 + rng.below(2) {
        caps.outputs
            .push(OutputCaps {
                id: OutputId(i),
                size: size(rng),
                refresh_mhz: rng.one_of(&[0, 50_000, 60_000]),
            })
            .unwrap();
    }
    for i in 0..rng.below(5) {
        let kind = rng.one_of(&[PlaneKind::Primary, PlaneKind::Overlay, PlaneKind::Cursor]);
        let format_count = rng.below(4);
        let formats = format_list(rng, format_count);
        caps.planes
            .push(PlaneCaps {
                id: PlaneId(100 + i),
                kind,
                outputs: rng.below(4),
                formats,
                scaling: scaling(rng),
                max_source: if rng.chance(1, 3) {
                    Some(Size::new(
                        rng.one_of(&[0, 1, 64, HUGE]),
                        rng.one_of(&[0, 1, 64, HUGE]),
                    ))
                } else {
                    None
                },
                plane_alpha: rng.chance(2, 3),
                blend_modes: if rng.chance(1, 4) {
                    (rng.signed() as u8) & 0b111
                } else {
                    0b111
                },
                transforms: if rng.chance(1, 4) {
                    (rng.signed() as u8) & 0b111_111
                } else {
                    0b111_111
                },
                zpos: zpos(rng),
                full_output_only: rng.chance(1, 6),
            })
            .unwrap();
    }
    if rng.chance(1, 2) {
        caps.resource_groups
            .push(ResourceGroup {
                consumers: LayerTraits(rng.one_of(&[1u8, 2, 4, 8, 6, 12, 0])),
                planes: rng.below(32),
                capacity: rng.below(4) as u8,
            })
            .unwrap();
    }
    caps
}

/// One generated case: everything a fresh engine and one frame needs.
#[derive(Debug)]
struct Case {
    template: Option<&'static str>,
    caps: Capabilities,
    buffers: Vec<(BufferId, BufferInfo)>,
    output: OutputId,
    layers: Vec<Layer>,
}

/// A buffer: usually sized to something extreme, and half the time with a
/// format some plane of `caps` accepts. Without that half the generator
/// would almost never produce a frame whose layers could share an output,
/// and multi-plane plans — the interesting stacking cases — would go
/// untested.
fn buffer_info(rng: &mut Rng, caps: &Capabilities) -> BufferInfo {
    let mut size = size(rng);
    if rng.chance(1, 3) {
        size.width = HUGE;
    }
    if rng.chance(1, 5) {
        size.height = HUGE;
    }
    let compatible = if rng.chance(1, 2) {
        let total: u32 = caps.planes.iter().map(|p| p.formats.len() as u32).sum();
        if total == 0 {
            None
        } else {
            let mut pick = rng.below(total);
            let mut found = None;
            for plane in caps.planes.iter() {
                let len = plane.formats.len() as u32;
                if pick < len {
                    found = Some(plane.formats[pick as usize]);
                    break;
                }
                pick -= len;
            }
            found
        }
    } else {
        None
    };
    let (format, modifier) = match compatible {
        Some(accepted) => (accepted.format, accepted.modifier),
        None => (rng.one_of(&FORMATS), Modifier(rng.one_of(&MODIFIERS))),
    };
    BufferInfo {
        size,
        format,
        modifier,
    }
}

fn gen_buffers(rng: &mut Rng, caps: &Capabilities) -> Vec<(BufferId, BufferInfo)> {
    (0..rng.below(5))
        .map(|i| (BufferId(1 + i), buffer_info(rng, caps)))
        .collect()
}

/// A source rectangle, sometimes inside the buffer and sometimes not.
fn gen_source(rng: &mut Rng, buffer: Size, whole: bool) -> Rect {
    if whole && buffer.width > 0 && buffer.height > 0 {
        // Valid, but as large as the buffer: this is what drives the clip
        // arithmetic through the widest values.
        return Rect::new(0, 0, buffer.width, buffer.height);
    }
    if buffer.width > 0 && buffer.height > 0 && rng.chance(2, 3) {
        let x = rng.below(buffer.width);
        let y = rng.below(buffer.height);
        return Rect::new(
            x as i32,
            y as i32,
            1 + rng.below(buffer.width - x),
            1 + rng.below(buffer.height - y),
        );
    }
    match rng.below(9) {
        0 => Rect::new(0, 0, 0, 1),
        1 => Rect::new(0, 0, 1, 0),
        2 => Rect::new(-1, 0, 1, 1),
        3 => Rect::new(0, -1, 1, 1),
        4 => Rect::new(i32::MIN, 0, HUGE, 1),
        5 => Rect::new(i32::MAX, 0, HUGE, 1),
        6 => Rect::new(0, 0, HUGE, HUGE),
        7 => Rect::new(1, 0, HUGE, 1),
        _ => Rect::new(0, 100_000, 1, 1),
    }
}

enum Dest {
    Full,
    Window,
    Overlap,
    Match,
    Off,
    HugeFlipped,
    HugeShifted,
    Zero,
    Random,
}

fn pick_dest(rng: &mut Rng) -> Dest {
    match rng.below(100) {
        0..=17 => Dest::Full,
        18..=35 => Dest::Window,
        36..=49 => Dest::Overlap,
        50..=69 => Dest::Match,
        70..=73 => Dest::Off,
        74..=81 => Dest::HugeFlipped,
        82..=86 => Dest::HugeShifted,
        87..=89 => Dest::Zero,
        _ => Dest::Random,
    }
}

fn gen_destination(rng: &mut Rng, out: Size, kind: &Dest, source: &Rect) -> Rect {
    match kind {
        Dest::Full => Rect::new(0, 0, out.width, out.height),
        // As large as its source: whatever the output cuts away is cut from
        // both sides, so even a plane without a scaler can program it.
        // This is how stackable frames — the multi-plane cases — arise.
        Dest::Match => Rect::new(
            to_i32(rng.below(out.width.max(1))),
            to_i32(rng.below(out.height.max(1))),
            source.width.max(1),
            source.height.max(1),
        ),
        Dest::Window | Dest::Overlap => {
            let width = rng.one_of(&[1u32, 16, 100, 1920, 4_000_000_000]);
            let height = rng.one_of(&[1u32, 16, 100, 1080, 1081]);
            if matches!(kind, Dest::Window) {
                Rect::new(
                    to_i32(rng.below(out.width.max(1))),
                    to_i32(rng.below(out.height.max(1))),
                    width,
                    height,
                )
            } else {
                Rect::new(
                    -1 - rng.below(2000) as i32,
                    -1 - rng.below(2000) as i32,
                    width,
                    height,
                )
            }
        }
        Dest::Off => match rng.below(4) {
            0 => Rect::new(to_i32(out.width), 0, 100, 100),
            1 => Rect::new(0, to_i32(out.height), 100, 100),
            2 => Rect::new(
                to_i32(out.width.saturating_add(1)),
                to_i32(out.height.saturating_add(1)),
                100,
                100,
            ),
            _ => Rect::new(-100, -100, 100, 100),
        },
        // Reaches past the output by more than 2^31 pixels; with a flip the
        // clip multiplies that cut back into the source.
        Dest::HugeFlipped => Rect::new(0, 0, HUGE, 1080),
        Dest::HugeShifted => Rect::new(i32::MIN, i32::MIN, HUGE, HUGE),
        Dest::Zero => match rng.below(2) {
            0 => Rect::new(0, 0, 0, 100),
            _ => Rect::new(0, 0, 100, 0),
        },
        Dest::Random => Rect::new(
            rng.signed(),
            rng.signed(),
            rng.one_of(&DIMENSIONS),
            rng.one_of(&DIMENSIONS),
        ),
    }
}

fn gen_layer(rng: &mut Rng, buffers: &[(BufferId, BufferInfo)], out: Size) -> Layer {
    let known = if buffers.is_empty() || rng.chance(1, 10) {
        None
    } else {
        Some(buffers[rng.below(buffers.len() as u32) as usize])
    };
    let (buffer, buffer_size) = match known {
        Some((id, info)) => (id, info.size),
        None => (BufferId(900 + rng.below(10)), Size::new(64, 64)),
    };
    let kind = pick_dest(rng);
    let whole = matches!(kind, Dest::HugeFlipped | Dest::HugeShifted | Dest::Match);
    let source = gen_source(rng, buffer_size, whole);
    let destination = gen_destination(rng, out, &kind, &source);
    let mut layer = Layer::new(buffer, source, destination, rng.one_of(&Z_ORDERS));
    layer.alpha = rng.one_of(&ALPHAS);
    if rng.chance(1, 8) {
        layer.alpha = rng.signed() as u16;
    }
    layer.blend = rng.one_of(&BLENDS);
    layer.transform = match kind {
        Dest::HugeFlipped => rng.one_of(&[Transform::FlipH, Transform::FlipV]),
        // A rotation would swap the axes and make the 1:1 mapping scale.
        Dest::Match => Transform::Normal,
        _ => rng.one_of(&TRANSFORMS),
    };
    layer
}

/// A buffers-and-layers pair: what a frame plus the engine needs built.
type Stack = (Vec<(BufferId, BufferInfo)>, Vec<Layer>);

/// A frame built to fit: two or three small 1:1 layers whose buffers the
/// chosen planes accept, so a friendly capability description should place
/// them together and multi-plane plans — stacking, zpos choices, group
/// budgets — are reached in practice. Against hostile capabilities such a
/// frame is still refused, for honest reasons.
fn coherent_stack(rng: &mut Rng, caps: &Capabilities, out: OutputId) -> Option<Stack> {
    let index = caps.output_index(out)?;
    let mut candidates: Vec<usize> = caps
        .planes
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            !p.formats.is_empty()
                && !p.full_output_only
                && p.transforms & Transform::Normal.mask() != 0
                && p.blend_modes & BlendMode::Opaque.mask() != 0
                && p.outputs & (1 << index) != 0
                && p.max_source.is_none_or(|m| m.width >= 64 && m.height >= 64)
        })
        .map(|(i, _)| i)
        .collect();
    if candidates.len() < 2 {
        return None;
    }
    let count = if rng.chance(1, 3) && candidates.len() >= 3 {
        3
    } else {
        2
    };
    let mut buffers = Vec::new();
    let mut layers = Vec::new();
    for (i, plane) in candidates.drain(..count).enumerate() {
        let accepted = caps.planes[plane].formats[0];
        buffers.push((
            BufferId(500 + i as u32),
            BufferInfo {
                size: Size::new(64, 64),
                format: accepted.format,
                modifier: accepted.modifier,
            },
        ));
        let mut layer = Layer::new(
            BufferId(500 + i as u32),
            Rect::new(0, 0, 64, 64),
            Rect::new(i as i32, i as i32, 64, 64),
            i as i32,
        );
        layer.transform = Transform::Normal;
        layer.blend = BlendMode::Opaque;
        layer.alpha = ALPHA_OPAQUE;
        layers.push(layer);
    }
    Some((buffers, layers))
}

fn gen_case(rng: &mut Rng) -> Case {
    let (template, caps) = if rng.chance(1, 2) {
        let all = templates();
        let (name, build) = rng.one_of(&all);
        (Some(name), build())
    } else {
        (None, random_capabilities(rng))
    };
    // An output the capability description may not even have.
    let out = if caps.outputs.is_empty() || rng.chance(1, 20) {
        OutputId(50 + rng.below(10))
    } else {
        caps.outputs[rng.below(caps.outputs.len() as u32) as usize].id
    };
    let out_size = caps
        .outputs
        .iter()
        .find(|o| o.id == out)
        .map(|o| o.size)
        .unwrap_or(Size::new(1920, 1080));
    let coherent = rng.chance(1, 8).then(|| coherent_stack(rng, &caps, out));
    let (buffers, layers) = match coherent {
        Some(Some(stack)) => stack,
        // Mostly frames at or below the layer limit; frames over it on top.
        _ => {
            let buffers = gen_buffers(rng, &caps);
            let count = if rng.chance(1, 40) {
                16 + rng.below(5)
            } else {
                rng.below(6)
            };
            let layers = (0..count)
                .map(|_| gen_layer(rng, &buffers, out_size))
                .collect();
            (buffers, layers)
        }
    };
    Case {
        template,
        caps,
        buffers,
        output: out,
        layers,
    }
}

fn build(case: &Case) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(case.caps));
    for (id, info) in &case.buffers {
        engine.register_buffer(*id, *info).unwrap();
        engine.finish_write(*id).unwrap();
    }
    engine
}

fn states(engine: &Engine<FakeBackend>, case: &Case) -> Vec<(BufferId, Option<BufferState>)> {
    case.buffers
        .iter()
        .map(|(id, _)| (*id, engine.buffer_state(*id)))
        .collect()
}

fn drain(engine: &mut Engine<FakeBackend>) -> Vec<DisplayEvent> {
    let mut events = Vec::new();
    while let Some(event) = engine.poll_event().unwrap() {
        events.push(event);
    }
    events
}

/// `true` if `destination` covers at least one pixel of the output. An
/// output without pixels, and a rectangle without pixels, cover none.
fn overlaps(destination: &Rect, out: Size) -> bool {
    let left = destination.x as i64;
    let top = destination.y as i64;
    !destination.is_empty()
        && out.width > 0
        && out.height > 0
        && left + destination.width as i64 > 0
        && top + destination.height as i64 > 0
        && left < out.width as i64
        && top < out.height as i64
}

/// Returns `true` if `inner` lies entirely inside `outer` (in exact math).
fn rect_within(inner: Rect, outer: Rect) -> bool {
    !inner.is_empty()
        && !outer.is_empty()
        && inner.x as i64 >= outer.x as i64
        && inner.y as i64 >= outer.y as i64
        && inner.x as i64 + inner.width as i64 <= outer.x as i64 + outer.width as i64
        && inner.y as i64 + inner.height as i64 <= outer.y as i64 + outer.height as i64
}

fn axis_within(source: u32, destination: u32, limits: ScaleLimits) -> bool {
    let source = source as u128;
    let destination = destination as u128;
    if destination > source {
        destination << 16 <= source * limits.max_upscale as u128
    } else {
        source << 16 <= destination * limits.max_downscale as u128
    }
}

/// Every assignment must satisfy the capabilities the plan came from, and
/// the assignments must be exactly the visible layers, stacked bottom to top.
fn check_plan(plan: &Plan, frame: &Frame<'_>, case: &Case) {
    let caps = &case.caps;
    let index = caps
        .output_index(frame.output)
        .expect("a plan for an output the capabilities do not have");
    let out = caps.outputs[index].size;
    assert_eq!(plan.output(), frame.output);
    assert!(plan.matches(frame), "the plan must match its frame");

    let mut order: Vec<usize> = (0..frame.layers.len()).collect();
    order.sort_by_key(|&i| (frame.layers[i].z_order, i));
    let expected: Vec<usize> = order
        .into_iter()
        .filter(|&i| overlaps(&frame.layers[i].destination, out))
        .collect();
    let got: Vec<usize> = plan.assignments().iter().map(|a| a.layer).collect();
    assert_eq!(
        got, expected,
        "assigned layers are the visible ones, stacked"
    );

    let mut used = 0u32;
    let mut below: Option<i32> = None;
    let mut group_use = vec![0u32; caps.resource_groups.len()];
    for a in plan.assignments() {
        let layer = *frame
            .layers
            .get(a.layer)
            .expect("an assignment to a layer the frame does not have");
        let info = case
            .buffers
            .iter()
            .find(|(id, _)| *id == layer.buffer)
            .map(|(_, info)| *info)
            .expect("an assignment from a buffer the engine does not know");
        let plane = *caps
            .planes
            .get(a.plane_index)
            .expect("an assignment to a plane the capabilities do not have");
        assert_eq!(plane.id, a.plane);
        assert_eq!(used & (1 << a.plane_index), 0, "one plane, one layer");
        used |= 1 << a.plane_index;
        assert_ne!(
            plane.outputs & (1 << index),
            0,
            "plane must reach the output"
        );

        assert!(!a.source.is_empty() && !a.destination.is_empty());
        assert!(
            a.source.fits_within(info.size),
            "assignment source escapes its buffer: {a:?} in {info:?}"
        );
        assert!(
            rect_within(a.source, layer.source),
            "clipped source escapes the layer source: {a:?} of {:?}",
            layer.source
        );
        assert!(
            a.destination.fits_within(out),
            "destination escapes the output: {a:?}"
        );
        assert!(
            plane
                .formats
                .iter()
                .any(|f| f.format == info.format && f.modifier == info.modifier),
            "plane {plane:?} does not accept {info:?}"
        );

        let oriented = if layer.transform.swaps_axes() {
            (a.source.height, a.source.width)
        } else {
            (a.source.width, a.source.height)
        };
        let scaled_h = oriented.0 != a.destination.width;
        let scaled_v = oriented.1 != a.destination.height;
        if scaled_h || scaled_v {
            let limits = plane
                .scaling
                .expect("assignment scales on a plane without a scaler");
            assert!(
                axis_within(oriented.0, a.destination.width, limits)
                    && axis_within(oriented.1, a.destination.height, limits),
                "scaling beyond the plane's limits: {a:?} with {limits:?}"
            );
        }
        if let Some(max) = plane.max_source {
            assert!(
                a.source.width <= max.width && a.source.height <= max.height,
                "source larger than the plane accepts: {a:?} vs {max:?}"
            );
        }
        if layer.alpha != ALPHA_OPAQUE {
            assert!(
                plane.plane_alpha,
                "plane alpha on a plane without it: {a:?}"
            );
        }
        let blend_free = layer.blend == BlendMode::Opaque && info.format.is_known_opaque();
        if !blend_free {
            assert_ne!(
                plane.blend_modes & layer.blend.mask(),
                0,
                "blend mode the plane does not support: {a:?}"
            );
        }
        if layer.transform != Transform::Normal {
            assert_ne!(
                plane.transforms & layer.transform.mask(),
                0,
                "transform the plane does not support: {a:?}"
            );
        }
        if plane.full_output_only {
            assert!(
                a.destination.covers(out),
                "window on a whole-output plane: {a:?}"
            );
        }
        match plane.zpos {
            Zpos::Fixed(z) => assert_eq!(a.zpos, z, "fixed position rewritten"),
            Zpos::Range { min, max } => {
                assert!(
                    a.zpos >= min && a.zpos <= max,
                    "zpos {} outside the plane's range {min}..={max}",
                    a.zpos
                );
            }
        }
        assert!(
            below.is_none_or(|b| a.zpos > b),
            "stacking must strictly increase"
        );
        below = Some(a.zpos);

        let mut traits = LayerTraits::ANY;
        if scaled_h {
            traits = traits.union(LayerTraits::SCALED_H);
        }
        if scaled_v {
            traits = traits.union(LayerTraits::SCALED_V);
        }
        if info.format.is_known_yuv() {
            traits = traits.union(LayerTraits::YUV);
        }
        for (g, group) in caps.resource_groups.iter().enumerate() {
            if group.planes & (1 << a.plane_index) != 0 && group.consumers.intersects(traits) {
                group_use[g] += 1;
            }
        }
    }
    for (g, users) in group_use.iter().enumerate() {
        assert!(
            *users <= caps.resource_groups[g].capacity as u32,
            "resource group over budget: {users} of {:?}",
            caps.resource_groups[g]
        );
    }
}

#[derive(Default, Debug)]
struct Stats {
    planned: u32,
    present_ok: u32,
    two_or_more_planes: u32,
    unknown_output: u32,
    too_many_layers: u32,
    unknown_buffer: u32,
    invalid_source: u32,
    invalid_destination: u32,
    cannot_clip: u32,
    no_plane: u32,
    search_gave_up: u32,
    unexpected: u32,
}

fn note(stats: &mut Stats, error: &Error) {
    match error {
        Error::Plan(PlanError::UnknownOutput(_)) => stats.unknown_output += 1,
        Error::Plan(PlanError::TooManyLayers { .. }) => stats.too_many_layers += 1,
        Error::Plan(PlanError::UnknownBuffer { .. }) => stats.unknown_buffer += 1,
        Error::Plan(PlanError::InvalidSource { .. }) => stats.invalid_source += 1,
        Error::Plan(PlanError::InvalidDestination { .. }) => stats.invalid_destination += 1,
        Error::Plan(PlanError::CannotClip { .. }) => stats.cannot_clip += 1,
        Error::Plan(PlanError::NoPlane { .. }) => stats.no_plane += 1,
        Error::Plan(
            PlanError::BackendRejected { .. } | PlanError::LimitExceeded | PlanError::Aborted,
        ) => stats.search_gave_up += 1,
        _ => stats.unexpected += 1,
    }
}

fn run_case(case: &Case, stats: &mut Stats) {
    let frame = Frame {
        output: case.output,
        layers: &case.layers,
    };

    let mut engine = build(case);
    let before = states(&engine, case);
    assert!(drain(&mut engine).is_empty(), "a fresh engine is quiet");

    // Two validations of the same frame agree.
    let validated = engine.validate(&frame);
    assert_eq!(
        engine.validate(&frame),
        validated,
        "validate is deterministic"
    );
    let presented = engine.present(&frame);
    match &validated {
        Ok(plan) => {
            stats.planned += 1;
            if plan.assignments().len() >= 2 {
                stats.two_or_more_planes += 1;
            }
            check_plan(plan, &frame, case);

            let submission = match presented {
                Ok(submission) => submission,
                Err(error) => panic!("present refused a validated frame: {error:?}"),
            };
            stats.present_ok += 1;
            assert_eq!(engine.backend().commit_count(), 1);
            for a in plan.assignments() {
                let buffer = frame.layers[a.layer].buffer;
                assert_eq!(
                    engine.buffer_state(buffer),
                    Some(BufferState::Submitted),
                    "a committed buffer must be Submitted"
                );
            }
            for (id, state) in &before {
                if plan
                    .assignments()
                    .iter()
                    .any(|a| frame.layers[a.layer].buffer == *id)
                {
                    continue;
                }
                assert_eq!(&engine.buffer_state(*id), state, "unused buffers stay idle");
            }

            engine.backend_mut().vsync();
            assert_eq!(
                drain(&mut engine),
                [DisplayEvent::Presented {
                    output: case.output,
                    submission,
                }],
                "the latch reports exactly one Presented"
            );
            assert_eq!(
                engine.backend().scanout(case.output).map(|p| p.len()),
                Some(plan.assignments().len()),
                "the fake shows what was planned"
            );
            for a in plan.assignments() {
                assert_eq!(
                    engine.buffer_state(frame.layers[a.layer].buffer),
                    Some(BufferState::Scanning),
                    "a latched buffer must be Scanning"
                );
            }
        }
        Err(first) => {
            note(stats, first);
            assert_eq!(
                presented,
                Err(*first),
                "present must refuse exactly what validate refused"
            );
            assert_eq!(engine.backend().commit_count(), 0);
            assert_eq!(
                states(&engine, case),
                before,
                "a refused frame must not move buffer states"
            );
            assert!(
                drain(&mut engine).is_empty(),
                "a refused frame must not report events"
            );
        }
    }

    // A fresh engine replays the frame to the same outcome.
    let mut second = build(case);
    assert_eq!(
        second.validate(&frame),
        validated,
        "validate is deterministic across engines"
    );
    assert_eq!(
        second.present(&frame),
        presented,
        "present is deterministic across engines"
    );
}

fn describe(seed: u64, index: u32, case: &Case, inner: &str) -> String {
    format!(
        "seed {seed:#x}, case {index}, template {:?}\ncaptured panic: {inner}\ncase: {case:#?}",
        case.template
    )
}

fn cases_to_run(default: u32) -> u32 {
    std::env::var("COMPOSITION_FUZZ_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn seed_to_run(default: u64) -> u64 {
    std::env::var("COMPOSITION_FUZZ_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

/// Runs every generated case with the panic hook silenced; the first
/// captured panic is reported together with the case that caused it.
#[test]
fn malformed_frames_property_sweep() {
    let total = cases_to_run(DEFAULT_CASES);
    let seed = seed_to_run(DEFAULT_SEED);
    let mut stats = Stats::default();

    // The hook silences the aborting thread's noise and records the
    // message; on 1.98 a formatted panic's payload is neither `&str` nor
    // `String`, so the hook is the reliable copy.
    let captured = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let hook = panic::take_hook();
    {
        let captured = std::sync::Arc::clone(&captured);
        panic::set_hook(Box::new(move |info| {
            let mut message = captured
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *message = format!("{info}");
        }));
    }
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        let mut rng = Rng(seed);
        for index in 0..total {
            let case = gen_case(&mut rng);
            let ran = panic::catch_unwind(AssertUnwindSafe(|| run_case(&case, &mut stats)));
            if let Err(payload) = ran {
                let message = captured
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone();
                let inner = if message.is_empty() {
                    panic_message(&payload)
                } else {
                    message
                };
                return Some(describe(seed, index, &case, &inner));
            }
        }
        None
    }));
    panic::set_hook(hook);

    match outcome {
        Ok(Some(report)) => panic!("{report}"),
        Ok(None) => {}
        Err(payload) => panic!(
            "the sweep harness itself panicked: {}",
            panic_message(&payload)
        ),
    }

    // The generator must actually produce each kind of outcome, or the
    // sweep proves nothing. The floors sit at roughly half the rates the
    // calibrated generator reaches, so they fail loudly if a change stops
    // covering a case, yet do not wobble when an unrelated edit shifts the
    // random stream a little.
    println!("{total} cases: {stats:?}");
    assert!(stats.planned >= total / 8, "planned: {stats:?}");
    assert_eq!(stats.present_ok, stats.planned);
    assert!(
        stats.two_or_more_planes >= total / 100,
        "two-plane plans: {stats:?}"
    );
    assert!(stats.no_plane >= total / 30, "no-plane: {stats:?}");
    assert!(stats.unknown_buffer >= 10, "unknown buffers: {stats:?}");
    assert!(stats.invalid_source >= 10, "invalid sources: {stats:?}");
    assert!(
        stats.invalid_destination >= 10,
        "invalid destinations: {stats:?}"
    );
    assert!(stats.unknown_output >= 1, "unknown outputs: {stats:?}");
    assert!(stats.cannot_clip >= 1, "uncroppable layers: {stats:?}");
    assert!(stats.too_many_layers >= 1, "over-long frames: {stats:?}");
    assert_eq!(
        stats.search_gave_up, 0,
        "searches that hit limits: {stats:?}"
    );
    assert_eq!(stats.unexpected, 0, "unexpected error kinds: {stats:?}");
}

/// One layer over the limit is refused as a whole, even when the frame
/// would otherwise be examined, and the frame one layer shorter fails for
/// a hardware reason: proving both paths were reached.
#[test]
fn frames_over_the_layer_limit_are_refused_as_a_whole() {
    let caps = caps_with(&[output(0, 1920, 1080)], &[], &[]);
    let case = Case {
        template: None,
        caps,
        buffers: vec![(
            BufferId(1),
            BufferInfo {
                size: Size::new(1920, 1080),
                format: PixelFormat::XRGB8888,
                modifier: Modifier(0),
            },
        )],
        output: OutputId(0),
        layers: (0..17)
            .map(|i| {
                Layer::new(
                    BufferId(1),
                    Rect::new(0, 0, 1920, 1080),
                    Rect::new(0, 0, 1920, 1080),
                    i,
                )
            })
            .collect(),
    };
    let mut engine = build(&case);
    assert_eq!(
        engine.present(&Frame {
            output: case.output,
            layers: &case.layers,
        }),
        Err(Error::Plan(PlanError::TooManyLayers { count: 17 }))
    );
    assert_eq!(engine.buffer_state(BufferId(1)), Some(BufferState::Ready));

    // The same frame one layer shorter fails for a hardware reason, not
    // for the limit: the engine looked at it at all.
    let sixteen = &case.layers[..16];
    assert_eq!(
        engine.present(&Frame {
            output: case.output,
            layers: sixteen,
        }),
        Err(Error::Plan(PlanError::NoPlane {
            layer: 0,
            reasons: RejectReasons::IN_USE,
        }))
    );
}

/// Refusals name the unknown output and the unknown buffer, and an unknown
/// output is reported before anything else is examined.
#[test]
fn unknown_ids_are_named_in_their_errors() {
    let case = Case {
        template: None,
        caps: caps_with(
            &[output(3, 1920, 1080)],
            &[plane(1, PlaneKind::Overlay, Zpos::Fixed(0), &[XRGB])],
            &[],
        ),
        buffers: vec![(
            BufferId(1),
            BufferInfo {
                size: Size::new(1920, 1080),
                format: PixelFormat::XRGB8888,
                modifier: Modifier(0),
            },
        )],
        output: OutputId(0),
        layers: vec![Layer::new(
            BufferId(1),
            Rect::new(0, 0, 1920, 1080),
            Rect::new(0, 0, 1920, 1080),
            0,
        )],
    };
    let mut engine = build(&case);
    let one = case.layers.clone();

    assert_eq!(
        engine.present(&Frame {
            output: OutputId(99),
            layers: &one,
        }),
        Err(Error::Plan(PlanError::UnknownOutput(OutputId(99))))
    );
    assert_eq!(
        engine.present(&Frame {
            output: OutputId(3),
            layers: &[Layer::new(
                BufferId(777),
                Rect::new(0, 0, 1920, 1080),
                Rect::new(0, 0, 1920, 1080),
                0,
            )],
        }),
        Err(Error::Plan(PlanError::UnknownBuffer {
            layer: 0,
            buffer: BufferId(777),
        }))
    );
    // A frame with an unknown output is refused before the layer limit.
    let seventeen = vec![one[0]; 17];
    assert_eq!(
        engine.present(&Frame {
            output: OutputId(99),
            layers: &seventeen,
        }),
        Err(Error::Plan(PlanError::UnknownOutput(OutputId(99))))
    );
}
