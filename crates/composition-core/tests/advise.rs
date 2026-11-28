//! Tests of the stateless advisory planner, `composition_core::advise`
//! (`docs/advisory-planner-design.md` §5, tests 1-8 and 12).
//!
//! Random frames come from a seeded, deterministic generator written in this
//! file, as in `planner_props.rs`; there is no dependency. Hardware is either
//! random or one of three hand-typed fixtures shaped like the AM62P (tidss,
//! two planes), the i.MX95 (DPU, six planes and shared resources) and the
//! STM32MP257 LTDC (three planes, premultiplied blend only). The fixtures are
//! shapes for the search, not the recorded capability tables.
//!
//! The reference answers (plane constraints, clipping, the reduced frame
//! with the composition layer) are restated here without the module's code;
//! the planner (`plan`) is the judge of whether a reduced frame is legal.
//!
//! The no_std half of test 12 is the `thumbv7em-none-eabihf` build of
//! `composition-core` (`scripts/check-all.sh`); the footprint half is
//! `advice_footprint_is_pinned` below.
//!
//! A failure prints the seed. `ADVISE_SEED=<seed>` reruns one case,
//! `ADVISE_CASES=<n>` changes the number of cases.

use composition_core::limits::{MAX_LAYERS, MAX_PLANES};
use composition_core::planner::{plan, plan_with};
use composition_core::{
    advise, explain_layer, Advice, AdviceError, AdviseOptions, Assignment, BlendMode, Blocker,
    BufferId, BufferInfo, BufferLookup, Capabilities, ComposeTarget, FormatModifier, Frame, Layer,
    LayerTraits, Modifier, OutputCaps, OutputId, PixelFormat, Placement, Plan, PlanError,
    PlaneCaps, PlaneId, PlaneKind, Rect, RejectReasons, ResourceGroup, ScaleLimits, SearchLimits,
    Size, Transform, Verdict, Zpos, ALPHA_OPAQUE, COMPOSE_LAYER, MAX_CANDIDATES,
};

// ---------------------------------------------------------------- generator

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x9e37_79b9_7f4a_7c15)
    }
    fn next(&mut self) -> u64 {
        // SplitMix64.
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

const OUT: OutputId = OutputId(0);
const XR24: PixelFormat = PixelFormat::XRGB8888;
const AR24: PixelFormat = PixelFormat::ARGB8888;
/// A fourcc no plane lists.
const HWL0: PixelFormat = PixelFormat::fourcc(b"HWL0");
const FORMATS: [PixelFormat; 6] = [
    XR24,
    AR24,
    PixelFormat::RGB565,
    PixelFormat::NV12,
    PixelFormat::YUYV,
    HWL0,
];
const BLENDS: [BlendMode; 3] = [
    BlendMode::Opaque,
    BlendMode::Premultiplied,
    BlendMode::Coverage,
];
/// The buffer id the test gives the composition layer of a reduced frame.
const TARGET: BufferId = BufferId(0xdead_0000);

#[derive(Clone, Debug)]
struct Case {
    seed: u64,
    caps: Capabilities,
    buffers: Vec<(BufferId, BufferInfo)>,
    layers: Vec<Layer>,
    output: OutputId,
    target: ComposeTarget,
}

impl BufferLookup for Case {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.buffers
            .iter()
            .find(|(id, _)| *id == buffer)
            .map(|(_, i)| *i)
    }
}

impl Case {
    fn frame(&self) -> Frame<'_> {
        Frame {
            output: self.output,
            layers: &self.layers,
        }
    }
    fn output_index(&self) -> usize {
        self.caps.output_index(self.output).unwrap()
    }
    fn output_size(&self) -> Size {
        self.caps.outputs[self.output_index()].size
    }
    fn options(&self) -> AdviseOptions {
        AdviseOptions {
            compose: Some(self.target),
            ..AdviseOptions::default()
        }
    }
    fn advise(&self) -> Result<Advice, AdviceError> {
        advise(&self.frame(), &self.caps, self, &self.options(), |_, _| {
            Verdict::Accept
        })
    }
}

/// The lookup of a reduced frame: the case's buffers plus the target.
struct Reduced<'a>(&'a Case);

impl BufferLookup for Reduced<'_> {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        if buffer == TARGET {
            Some(BufferInfo {
                size: self.0.output_size(),
                format: self.0.target.format,
                modifier: self.0.target.modifier,
            })
        } else {
            self.0.buffer_info(buffer)
        }
    }
}

fn single_output(size: Size) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size,
            refresh_mhz: 60_000,
        })
        .unwrap();
    caps
}

fn with_formats(mut p: PlaneCaps, formats: &[PixelFormat]) -> PlaneCaps {
    for f in formats {
        p.formats.push(FormatModifier::linear(*f)).unwrap();
    }
    p
}

const ALL_BLENDS: u8 = 0b111;

/// Shaped like the AM62P tidss video port: VID scales, VIDL1 does not, both
/// stack anywhere in 0..=1 and take RGB and YUV.
fn am62p_like() -> Capabilities {
    let mut caps = single_output(Size::new(1920, 1080));
    let formats = [
        XR24,
        AR24,
        PixelFormat::RGB565,
        PixelFormat::NV12,
        PixelFormat::YUYV,
    ];
    for (id, scaling) in [
        (
            31,
            Some(ScaleLimits {
                max_upscale: 8 << 16,
                max_downscale: 4 << 16,
            }),
        ),
        (41, None),
    ] {
        let p = PlaneCaps {
            id: PlaneId(id),
            kind: if id == 31 {
                PlaneKind::Primary
            } else {
                PlaneKind::Overlay
            },
            outputs: 1,
            scaling,
            plane_alpha: true,
            blend_modes: ALL_BLENDS,
            transforms: Transform::Normal.mask(),
            zpos: Zpos::Range { min: 0, max: 1 },
            ..PlaneCaps::default()
        };
        caps.planes.push(with_formats(p, &formats)).unwrap();
    }
    caps
}

/// Shaped like the i.MX95 DPU of `m2_scene.rs`: six planes in 0..=5 and
/// shared scalers.
fn imx95_like() -> Capabilities {
    let mut caps = single_output(Size::new(1920, 1080));
    let formats = [
        XR24,
        AR24,
        PixelFormat::RGB565,
        PixelFormat::NV12,
        PixelFormat::YUYV,
    ];
    for i in 0..6u32 {
        let p = PlaneCaps {
            id: PlaneId(i + 1),
            kind: if i == 0 {
                PlaneKind::Primary
            } else {
                PlaneKind::Overlay
            },
            outputs: 1,
            scaling: Some(ScaleLimits {
                max_upscale: 2048 << 16,
                max_downscale: 1 << 16,
            }),
            max_source: Some(Size::new(8192, 8192)),
            plane_alpha: true,
            blend_modes: ALL_BLENDS,
            transforms: Transform::Normal.mask(),
            zpos: Zpos::Range { min: 0, max: 5 },
            ..PlaneCaps::default()
        };
        caps.planes.push(with_formats(p, &formats)).unwrap();
    }
    for (consumers, capacity) in [
        (LayerTraits::SCALED.union(LayerTraits::YUV), 4),
        (LayerTraits::SCALED_H, 1),
        (LayerTraits::SCALED_V, 1),
    ] {
        caps.resource_groups
            .push(ResourceGroup {
                consumers,
                planes: 0b11_1111,
                capacity,
            })
            .unwrap();
    }
    caps
}

/// Shaped like the STM32MP257 LTDC: planes 34, 42 and 49 in 0..=2,
/// premultiplied blend only, no scaling, YUYV on 34 and 42, NV12 on 42.
fn ltdc_like() -> Capabilities {
    let mut caps = single_output(Size::new(1920, 1080));
    let rgb = [XR24, AR24, PixelFormat::RGB565];
    for (id, extra) in [
        (34, &[PixelFormat::YUYV][..]),
        (42, &[PixelFormat::YUYV, PixelFormat::NV12][..]),
        (49, &[][..]),
    ] {
        let p = PlaneCaps {
            id: PlaneId(id),
            kind: if id == 34 {
                PlaneKind::Primary
            } else {
                PlaneKind::Overlay
            },
            outputs: 1,
            plane_alpha: true,
            blend_modes: BlendMode::Premultiplied.mask(),
            transforms: Transform::Normal.mask(),
            zpos: Zpos::Range { min: 0, max: 2 },
            ..PlaneCaps::default()
        };
        let p = with_formats(with_formats(p, &rgb), extra);
        caps.planes.push(p).unwrap();
    }
    caps
}

/// Small random hardware, as in `planner_props.rs` but plainer.
fn random_caps(rng: &mut Rng) -> Capabilities {
    let size = [Size::new(64, 48), Size::new(100, 60)][rng.below(2) as usize];
    let mut caps = single_output(size);
    if rng.chance(20) {
        caps.outputs
            .push(OutputCaps {
                id: OutputId(1),
                size,
                refresh_mhz: 60_000,
            })
            .unwrap();
    }
    let all_outputs = (1u32 << caps.outputs.len()) - 1;
    for i in 0..1 + rng.below(4) {
        let mut p = PlaneCaps {
            id: PlaneId(100 + i as u32),
            kind: PlaneKind::Overlay,
            outputs: if rng.chance(10) { 0b10 } else { all_outputs },
            ..PlaneCaps::default()
        };
        for f in FORMATS[..5].iter() {
            if rng.chance(60) {
                p.formats.push(FormatModifier::linear(*f)).unwrap();
            }
        }
        if rng.chance(50) {
            p.scaling = Some(ScaleLimits {
                max_upscale: 0x1_0000 + rng.below(0x3_0000) as u32,
                max_downscale: 0x1_0000 + rng.below(0x3_0000) as u32,
            });
        }
        if rng.chance(20) {
            p.max_source = Some(Size::new(
                rng.range(16, 100) as u32,
                rng.range(12, 60) as u32,
            ));
        }
        p.plane_alpha = rng.chance(70);
        for b in BLENDS {
            if rng.chance(70) {
                p.blend_modes |= b.mask();
            }
        }
        if rng.chance(30) {
            p.transforms |= Transform::Rotate180.mask() | Transform::FlipH.mask();
        }
        p.zpos = if rng.chance(50) {
            let min = rng.range(0, 2) as i32;
            Zpos::Range {
                min,
                max: min + rng.range(0, 4) as i32,
            }
        } else {
            Zpos::Fixed(rng.range(0, 3) as i32)
        };
        p.full_output_only = rng.chance(10);
        caps.planes.push(p).unwrap();
    }
    if rng.chance(30) {
        caps.resource_groups
            .push(ResourceGroup {
                consumers: [LayerTraits::ANY, LayerTraits::SCALED, LayerTraits::YUV]
                    [rng.below(3) as usize],
                planes: rng.below(1 << caps.planes.len()) as u32,
                capacity: rng.below(3) as u8,
            })
            .unwrap();
    }
    caps
}

/// One layer. Destinations: whole output, a window inside (scaled or not),
/// unscaled and partly off (Normal only, so the reference clip is exact),
/// rotated and partly off (cannot be clipped), entirely off.
fn gen_layer(rng: &mut Rng, id: u32, out: Size, z_order: i32) -> (Layer, BufferInfo) {
    let (bw, bh) = (
        rng.range(4, out.width as i64) as u32,
        rng.range(4, out.height as i64) as u32,
    );
    let info = BufferInfo {
        size: Size::new(bw, bh),
        format: FORMATS[rng.below(FORMATS.len() as u64) as usize],
        modifier: Modifier::LINEAR,
    };
    let source = Rect::new(0, 0, bw, bh);
    let mut layer = Layer::new(BufferId(id), source, Rect::new(0, 0, bw, bh), z_order);
    layer.destination = match rng.below(10) {
        0 | 1 => Rect::new(0, 0, out.width, out.height),
        2..=4 => Rect::new(
            rng.range(0, (out.width - bw) as i64) as i32,
            rng.range(0, (out.height - bh) as i64) as i32,
            bw,
            bh,
        ),
        5 => {
            let (w, h) = (
                rng.range(1, out.width as i64) as u32,
                rng.range(1, out.height as i64) as u32,
            );
            Rect::new(0, 0, w, h)
        }
        6 | 7 => Rect::new(
            rng.range(-(bw as i64) + 1, out.width as i64 - 1) as i32,
            rng.range(-(bh as i64) + 1, out.height as i64 - 1) as i32,
            bw,
            bh,
        ),
        8 => {
            layer.transform = Transform::Rotate90;
            Rect::new(-1, 0, bh, bw)
        }
        _ => Rect::new(out.width as i32 + rng.range(0, 5) as i32, 0, bw, bh),
    };
    if layer.transform == Transform::Normal && rng.chance(10) {
        layer.transform = [Transform::Rotate180, Transform::FlipH][rng.below(2) as usize];
        // Keep the reference clip exact: a reflected layer stays inside.
        layer.destination = Rect::new(0, 0, bw, bh);
    }
    layer.blend = BLENDS[rng.below(3) as usize];
    if rng.chance(15) {
        layer.alpha = rng.below(ALPHA_OPAQUE as u64) as u16;
    }
    (layer, info)
}

#[derive(Clone, Copy)]
enum Hardware {
    Random,
    Am62p,
    Imx95,
    Ltdc,
}

fn gen_case(seed: u64, hardware: Hardware, max_layers: u64) -> Case {
    let mut rng = Rng::new(seed);
    let caps = match hardware {
        Hardware::Random => random_caps(&mut rng),
        Hardware::Am62p => am62p_like(),
        Hardware::Imx95 => imx95_like(),
        Hardware::Ltdc => ltdc_like(),
    };
    let out = caps.outputs[0].size;
    let mut buffers = Vec::new();
    let mut layers = Vec::new();
    for i in 0..rng.below(max_layers + 1) {
        let z = rng.range(-2, 3) as i32;
        let (layer, info) = gen_layer(&mut rng, i as u32 + 1, out, z);
        buffers.push((layer.buffer, info));
        layers.push(layer);
    }
    let target = if rng.chance(70) {
        ComposeTarget {
            format: XR24,
            modifier: Modifier::LINEAR,
            blend: BlendMode::Opaque,
        }
    } else {
        ComposeTarget {
            format: AR24,
            modifier: Modifier::LINEAR,
            blend: BlendMode::Premultiplied,
        }
    };
    Case {
        seed,
        caps,
        buffers,
        layers,
        output: OUT,
        target,
    }
}

fn cases() -> impl Iterator<Item = u64> {
    let fixed = std::env::var("ADVISE_SEED")
        .ok()
        .map(|s| s.parse::<u64>().expect("ADVISE_SEED is a number"));
    let n = std::env::var("ADVISE_CASES").ok().map_or(1500, |s| {
        s.parse::<u64>().expect("ADVISE_CASES is a number")
    });
    match fixed {
        Some(seed) => seed..seed + 1,
        None => 0..n,
    }
}

const HARDWARE: [Hardware; 4] = [
    Hardware::Random,
    Hardware::Am62p,
    Hardware::Imx95,
    Hardware::Ltdc,
];

fn each_case(max_layers: u64, mut f: impl FnMut(&Case)) {
    for seed in cases() {
        let case = gen_case(seed, HARDWARE[(seed % 4) as usize], max_layers);
        f(&case);
    }
}

fn fail(case: &Case, msg: &str) -> ! {
    panic!(
        "{msg}\nseed: {} (rerun with ADVISE_SEED={})\ninput: {case:#?}",
        case.seed, case.seed
    )
}

// ------------------------------------------------------------------- oracle

fn opaque_format(f: PixelFormat) -> bool {
    [
        PixelFormat::RGB565,
        PixelFormat::XRGB8888,
        PixelFormat::XBGR8888,
        PixelFormat::NV12,
        PixelFormat::YUYV,
    ]
    .contains(&f)
}

fn axis_ok(from: u32, to: u32, s: ScaleLimits) -> bool {
    let (from, to) = (from as u128, to as u128);
    if to > from {
        to << 16 <= from * s.max_upscale as u128
    } else {
        from << 16 <= to * s.max_downscale as u128
    }
}

/// Every constraint that depends on one visible layer and one plane.
fn plane_reasons(
    layer: &Layer,
    info: &BufferInfo,
    plane: &PlaneCaps,
    oi: usize,
    out: Size,
) -> RejectReasons {
    let (source, dest) = (layer.source, layer.destination);
    let mut r = RejectReasons::NONE;
    if plane.outputs >> oi & 1 == 0 {
        r |= RejectReasons::OUTPUT;
    }
    if !plane
        .formats
        .iter()
        .any(|f| f.format == info.format && f.modifier == info.modifier)
    {
        r |= RejectReasons::FORMAT;
    }
    let (sw, sh) = if layer.transform.swaps_axes() {
        (source.height, source.width)
    } else {
        (source.width, source.height)
    };
    if (sw, sh) != (dest.width, dest.height)
        && !plane
            .scaling
            .is_some_and(|s| axis_ok(sw, dest.width, s) && axis_ok(sh, dest.height, s))
    {
        r |= RejectReasons::SCALING;
    }
    if plane
        .max_source
        .is_some_and(|m| source.width > m.width || source.height > m.height)
    {
        r |= RejectReasons::SOURCE_SIZE;
    }
    if layer.alpha != ALPHA_OPAQUE && !plane.plane_alpha {
        r |= RejectReasons::ALPHA;
    }
    let free = layer.blend == BlendMode::Opaque && opaque_format(info.format);
    if !free && plane.blend_modes & (1 << layer.blend as u8) == 0 {
        r |= RejectReasons::BLEND;
    }
    if layer.transform != Transform::Normal && plane.transforms & (1 << layer.transform as u8) == 0
    {
        r |= RejectReasons::TRANSFORM;
    }
    if plane.full_output_only && dest != Rect::new(0, 0, out.width, out.height) {
        r |= RejectReasons::FULL_OUTPUT;
    }
    r
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Seen {
    Hidden,
    Unclippable,
    /// The layer as the hardware sees it.
    Shown(Layer),
}

/// Reference clip for what the generator makes: unscaled Normal layers and
/// rotated ones; nothing else is partly off.
fn seen(layer: &Layer, out: Size) -> Seen {
    let d = layer.destination;
    let (l, t) = (d.x as i64, d.y as i64);
    let (r, b) = (l + d.width as i64, t + d.height as i64);
    let (cl, ct) = (l.max(0), t.max(0));
    let (cr, cb) = (r.min(out.width as i64), b.min(out.height as i64));
    if cl >= cr || ct >= cb {
        return Seen::Hidden;
    }
    let dest = Rect::new(cl as i32, ct as i32, (cr - cl) as u32, (cb - ct) as u32);
    if dest == d {
        return Seen::Shown(*layer);
    }
    if layer.transform != Transform::Normal {
        return Seen::Unclippable;
    }
    assert_eq!(
        (layer.source.width, layer.source.height),
        (d.width, d.height)
    );
    let s = layer.source;
    Seen::Shown(Layer {
        source: Rect::new(
            s.x + (cl - l) as i32,
            s.y + (ct - t) as i32,
            dest.width,
            dest.height,
        ),
        destination: dest,
        ..*layer
    })
}

/// Layers on the output in stacking order: (frame index, how it is seen).
fn stacking(case: &Case) -> Vec<(usize, Seen)> {
    let out = case.output_size();
    let mut v: Vec<(usize, Seen)> = case
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| (i, seen(l, out)))
        .filter(|(_, s)| *s != Seen::Hidden)
        .collect();
    v.sort_by_key(|(i, _)| (case.layers[*i].z_order, *i));
    v
}

/// The frame the caller would present after composing positions `run` of
/// `stacking(case)` into one full-output layer, built independently of the
/// module: stacking positions become `z_order`, hidden layers are left out.
/// Returns the layers and, per reduced index, the frame index or `None` for
/// the composition layer.
fn reduced_frame(case: &Case, run: Option<(usize, usize)>) -> (Vec<Layer>, Vec<Option<usize>>) {
    let out = case.output_size();
    let full = Rect::new(0, 0, out.width, out.height);
    let mut layers = Vec::new();
    let mut map = Vec::new();
    for (position, (index, _)) in stacking(case).into_iter().enumerate() {
        let z = position as i32;
        match run {
            Some((low, high)) if position == low => {
                let mut target = Layer::new(TARGET, full, full, z);
                target.blend = case.target.blend;
                layers.push(target);
                map.push(None);
                let _ = high;
            }
            Some((low, high)) if low < position && position <= high => {}
            _ => {
                layers.push(Layer {
                    z_order: z,
                    ..case.layers[index]
                });
                map.push(Some(index));
            }
        }
    }
    (layers, map)
}

fn plan_reduced(
    case: &Case,
    run: Option<(usize, usize)>,
) -> Result<(Plan, Vec<Option<usize>>), PlanError> {
    let (layers, map) = reduced_frame(case, run);
    let frame = Frame {
        output: case.output,
        layers: &layers,
    };
    plan(&frame, &case.caps, &Reduced(case)).map(|p| (p, map))
}

/// Every contiguous run of `n` positions in the module's order.
fn runs(n: usize) -> Vec<Option<(usize, usize)>> {
    let mut v = vec![None];
    for length in 1..=n {
        for low in 0..=n - length {
            v.push(Some((low, low + length - 1)));
        }
    }
    v
}

/// Positions in the stacking order whose layers are composed.
fn composed_positions(case: &Case, advice: &Advice) -> Vec<usize> {
    stacking(case)
        .iter()
        .enumerate()
        .filter(|(_, (i, _))| advice.layers()[*i].placement == Placement::Compose)
        .map(|(p, _)| p)
        .collect()
}

// -------------------------------------------------------------------- tests

/// Test 1: when the planner places the frame, the advice is exactly that;
/// when it cannot, the advice is not direct; input errors are the same.
#[test]
fn equivalence_with_plan() {
    let mut direct = 0;
    let mut composed = 0;
    each_case(6, |case| {
        let planned = plan(&case.frame(), &case.caps, case);
        for options in [AdviseOptions::default(), case.options()] {
            let advice = advise(&case.frame(), &case.caps, case, &options, |_, _| {
                Verdict::Accept
            });
            match (&planned, &advice) {
                (Ok(p), Ok(a)) => {
                    if !a.is_direct() || a.compose().is_some() || a.candidates() != 1 {
                        fail(case, &format!("plan succeeded, advice {a:?}"));
                    }
                    for (index, l) in a.layers().iter().enumerate() {
                        let want = p.assignments().iter().find(|x| x.layer == index);
                        let got = match l.placement {
                            Placement::Plane(x) => Some(x),
                            Placement::Hidden => None,
                            Placement::Compose => fail(case, "composed layer in a direct plan"),
                        };
                        if got.as_ref() != want || l.blocker != Blocker::None {
                            fail(case, &format!("layer {index}: {l:?} vs plan {want:?}"));
                        }
                    }
                    direct += 1;
                }
                (Ok(_), Err(e)) => fail(case, &format!("plan succeeded, advise failed {e:?}")),
                (Err(PlanError::NoPlane { .. } | PlanError::CannotClip { .. }), Ok(a)) => {
                    if a.is_direct() {
                        fail(case, "plan failed, advice is direct");
                    }
                    composed += 1;
                }
                (
                    Err(PlanError::NoPlane { .. } | PlanError::CannotClip { .. }),
                    Err(AdviceError::NoComposePlane { .. }),
                ) => {}
                (Err(PlanError::UnknownOutput(o)), Err(AdviceError::UnknownOutput(p)))
                    if o == p => {}
                (Err(e), r) => fail(case, &format!("plan {e:?}, advise {r:?}")),
            }
        }
    });
    if std::env::var("ADVISE_SEED").is_err() {
        assert!(
            direct > 300 && composed > 300,
            "direct {direct}, composed {composed}"
        );
    }
}

/// Tests 2 and 3: the frame the caller presents after composing is legal
/// with exactly the advised assignments, the composed layers are one
/// contiguous run, and each composed layer names a blocker.
#[test]
fn reduced_frame_is_legal_and_run_is_contiguous() {
    let mut checked = 0;
    each_case(6, |case| {
        let Ok(advice) = case.advise() else { return };
        let positions = composed_positions(case, &advice);
        let run = advice.compose_run();
        match (run, positions.first(), positions.last()) {
            (None, None, None) => {}
            (Some((low, high)), Some(&first), Some(&last)) => {
                if (low, high) != (first, last) || positions.len() != high - low + 1 {
                    fail(
                        case,
                        &format!("run {run:?}, composed positions {positions:?}"),
                    );
                }
                if advice.compose().is_none() {
                    fail(case, "composed run without a composition assignment");
                }
            }
            _ => fail(
                case,
                &format!("run {run:?}, composed positions {positions:?}"),
            ),
        }
        for l in advice.layers() {
            if (l.placement == Placement::Compose) != (l.blocker != Blocker::None) {
                fail(case, &format!("placement and blocker disagree: {l:?}"));
            }
        }

        let Ok((plan, map)) = plan_reduced(case, run) else {
            fail(case, "the advised reduced frame does not plan")
        };
        let (oi, out) = (case.output_index(), case.output_size());
        let mut below = None;
        let mut used = 0u32;
        for a in plan.assignments() {
            let p = &case.caps.planes[a.plane_index];
            if used >> a.plane_index & 1 == 1 || below.is_some_and(|b| a.zpos <= b) {
                fail(case, "reduced plan reuses a plane or does not stack");
            }
            used |= 1 << a.plane_index;
            below = Some(a.zpos);
            let advised = match map[a.layer] {
                None => {
                    let target_info = Reduced(case).buffer_info(TARGET).unwrap();
                    let full = Rect::new(0, 0, out.width, out.height);
                    let mut t = Layer::new(TARGET, full, full, 0);
                    t.blend = case.target.blend;
                    if !plane_reasons(&t, &target_info, p, oi, out).is_empty() {
                        fail(case, "composition layer on an unsuitable plane");
                    }
                    *advice.compose().unwrap()
                }
                Some(index) => {
                    let Seen::Shown(v) = seen(&case.layers[index], out) else {
                        fail(case, "promoted layer is not shown")
                    };
                    let info = case.buffer_info(v.buffer).unwrap();
                    if !plane_reasons(&v, &info, p, oi, out).is_empty() {
                        fail(case, "promoted layer on an unsuitable plane");
                    }
                    match advice.layers()[index].placement {
                        Placement::Plane(x) => x,
                        other => fail(case, &format!("layer {index} planned, advised {other:?}")),
                    }
                }
            };
            let expect_layer = map[a.layer].unwrap_or(COMPOSE_LAYER);
            if advised
                != (Assignment {
                    layer: expect_layer,
                    ..*a
                })
            {
                fail(case, &format!("advised {advised:?}, reduced plan {a:?}"));
            }
        }
        if plan.assignments().len() != advice.promoted() + run.map_or(0, |_| 1) {
            fail(case, "assignment count differs");
        }
        checked += 1;
    });
    assert!(checked > 0);
}

/// Test 4: no shorter contiguous run is legal, and no run of the same
/// length further down; when the advice fails, no run is legal at all.
#[test]
fn composed_run_is_minimal() {
    let mut composing = 0;
    each_case(5, |case| {
        let n = stacking(case).len();
        let advice = case.advise();
        let legal = |run| plan_reduced(case, run).is_ok();
        match advice {
            Ok(a) => {
                let chosen = a.compose_run();
                for run in runs(n) {
                    if run == chosen {
                        break;
                    }
                    if legal(run) {
                        fail(case, &format!("{run:?} is legal and before {chosen:?}"));
                    }
                }
                if chosen.is_some() {
                    composing += 1;
                }
            }
            Err(AdviceError::NoComposePlane { .. }) => {
                if let Some(run) = runs(n).into_iter().find(|r| legal(*r)) {
                    fail(case, &format!("no advice, but {run:?} is legal"));
                }
            }
            Err(e) => fail(case, &format!("advise failed with {e:?}")),
        }
    });
    if std::env::var("ADVISE_SEED").is_err() {
        assert!(composing > 200, "composing {composing}");
    }
}

/// Test 5: `Static` means every plane refuses the layer alone, its reasons
/// are the intersection over the planes on the output, and `explain_layer`
/// gives the per-plane rows of the reference.
#[test]
fn static_reasons_are_the_per_plane_intersection() {
    let mut statics = 0;
    each_case(6, |case| {
        let Ok(advice) = case.advise() else { return };
        let (oi, out) = (case.output_index(), case.output_size());
        for (index, layer) in case.layers.iter().enumerate() {
            let info = case.buffer_info(layer.buffer).unwrap();
            let mut rows = [RejectReasons::NONE; MAX_PLANES];
            explain_layer(layer, &info, &case.caps, case.output, &mut rows).unwrap();
            let l = advice.layers()[index];
            match seen(layer, out) {
                Seen::Hidden => {
                    if rows != [RejectReasons::NONE; MAX_PLANES] || l.placement != Placement::Hidden
                    {
                        fail(case, &format!("hidden layer {index}: {l:?}, rows {rows:?}"));
                    }
                }
                Seen::Unclippable => {
                    if l.blocker != Blocker::Clip {
                        fail(case, &format!("unclippable layer {index}: {l:?}"));
                    }
                }
                Seen::Shown(v) => {
                    let reference: Vec<RejectReasons> = case
                        .caps
                        .planes
                        .iter()
                        .map(|p| plane_reasons(&v, &info, p, oi, out))
                        .collect();
                    if rows[..reference.len()] != reference[..]
                        || rows[reference.len()..].iter().any(|r| !r.is_empty())
                    {
                        fail(
                            case,
                            &format!("explain_layer {rows:?}, reference {reference:?}"),
                        );
                    }
                    let all_refuse = reference.iter().all(|r| !r.is_empty());
                    if all_refuse != (l.blocker == Blocker::Static) {
                        fail(
                            case,
                            &format!("layer {index}: {l:?}, reference {reference:?}"),
                        );
                    }
                    if all_refuse {
                        let on_output: Vec<&RejectReasons> = reference
                            .iter()
                            .zip(case.caps.planes.iter())
                            .filter(|(_, p)| p.outputs >> oi & 1 == 1)
                            .map(|(r, _)| r)
                            .collect();
                        let want = if on_output.is_empty() {
                            RejectReasons::OUTPUT
                        } else {
                            let bits = on_output.iter().fold(u32::MAX, |acc, r| acc & r.bits());
                            reasons_with_bits(bits)
                        };
                        if l.reasons != want {
                            fail(
                                case,
                                &format!("static reasons {:?}, want {want:?}", l.reasons),
                            );
                        }
                        statics += 1;
                    } else if !l.reasons.is_empty() {
                        fail(case, "reasons on a layer that is not Static");
                    }
                }
            }
        }
    });
    if std::env::var("ADVISE_SEED").is_err() {
        assert!(statics > 100, "statics {statics}");
    }
}

fn reasons_with_bits(bits: u32) -> RejectReasons {
    (0..11)
        .map(|b| 1u32 << b)
        .filter(|b| bits & b != 0)
        .fold(RejectReasons::NONE, |acc, b| acc | reason_bit(b))
}

fn reason_bit(bit: u32) -> RejectReasons {
    [
        RejectReasons::OUTPUT,
        RejectReasons::FORMAT,
        RejectReasons::SCALING,
        RejectReasons::SOURCE_SIZE,
        RejectReasons::ALPHA,
        RejectReasons::BLEND,
        RejectReasons::TRANSFORM,
        RejectReasons::FULL_OUTPUT,
        RejectReasons::Z_ORDER,
        RejectReasons::RESOURCE,
        RejectReasons::IN_USE,
    ][bit.trailing_zeros() as usize]
}

fn xr24_buffers(sizes: &[(u32, Size)]) -> Vec<(BufferId, BufferInfo)> {
    sizes
        .iter()
        .map(|(id, size)| {
            (
                BufferId(*id),
                BufferInfo {
                    size: *size,
                    format: XR24,
                    modifier: Modifier::LINEAR,
                },
            )
        })
        .collect()
}

fn fixed_case(
    caps: Capabilities,
    layers: Vec<Layer>,
    buffers: Vec<(BufferId, BufferInfo)>,
) -> Case {
    Case {
        seed: 0,
        caps,
        buffers,
        layers,
        output: OUT,
        target: ComposeTarget {
            format: XR24,
            modifier: Modifier::LINEAR,
            blend: BlendMode::Opaque,
        },
    }
}

/// Test 6: an off-output layer is `Hidden`; a rotated layer partly off the
/// output, which `plan` refuses with `CannotClip`, is composed with
/// `Blocker::Clip`.
#[test]
fn hidden_and_clipped_layers() {
    let mut caps = am62p_like();
    caps.planes.as_mut_slice()[0].transforms |= Transform::Rotate90.mask();
    let full = Rect::new(0, 0, 1920, 1080);
    let mut rotated = Layer::new(
        BufferId(2),
        Rect::new(0, 0, 400, 300),
        Rect::new(-10, 0, 300, 400),
        1,
    );
    rotated.transform = Transform::Rotate90;
    let off = Layer::new(
        BufferId(2),
        Rect::new(0, 0, 400, 300),
        Rect::new(1920, 0, 400, 300),
        2,
    );
    let case = fixed_case(
        caps,
        vec![Layer::new(BufferId(1), full, full, 0), rotated, off],
        xr24_buffers(&[(1, Size::new(1920, 1080)), (2, Size::new(400, 300))]),
    );
    assert_eq!(
        plan(&case.frame(), &case.caps, &case),
        Err(PlanError::CannotClip { layer: 1 })
    );
    let advice = case.advise().unwrap();
    let l = advice.layers();
    assert!(matches!(l[0].placement, Placement::Plane(_)));
    assert_eq!(
        (l[1].placement, l[1].blocker),
        (Placement::Compose, Blocker::Clip)
    );
    assert_eq!(
        (l[2].placement, l[2].blocker),
        (Placement::Hidden, Blocker::None)
    );
    assert_eq!(advice.compose_run(), Some((1, 1)));
    assert_eq!(advice.compose().unwrap().layer, COMPOSE_LAYER);
    // Without a composition target the clip still names the layer.
    let none = advise(
        &case.frame(),
        &case.caps,
        &case,
        &AdviseOptions::default(),
        |_, _| Verdict::Accept,
    )
    .unwrap();
    assert!(!none.is_direct() && none.compose().is_none());
    assert_eq!(none.layers()[1].blocker, Blocker::Clip);
    assert_eq!(none.layers()[2].placement, Placement::Hidden);
}

/// Test 6, broader: the module's clipping agrees with the planner's on
/// single layers of every transform, scaled or not, inside, partly off and
/// entirely off, against a plane that takes anything.
#[test]
fn clipping_agrees_with_the_planner() {
    let mut caps = single_output(Size::new(64, 48));
    let any = PlaneCaps {
        id: PlaneId(1),
        outputs: 1,
        scaling: Some(ScaleLimits {
            max_upscale: 64 << 16,
            max_downscale: 64 << 16,
        }),
        plane_alpha: true,
        blend_modes: ALL_BLENDS,
        transforms: 0x3f,
        zpos: Zpos::Range { min: 0, max: 3 },
        ..PlaneCaps::default()
    };
    caps.planes.push(with_formats(any, &[XR24])).unwrap();
    let transforms = [
        Transform::Normal,
        Transform::Rotate90,
        Transform::Rotate180,
        Transform::Rotate270,
        Transform::FlipH,
        Transform::FlipV,
    ];
    let mut rng = Rng::new(7);
    let mut kinds = [0; 3];
    for _ in 0..5000 {
        let (bw, bh) = (rng.range(1, 40) as u32, rng.range(1, 40) as u32);
        let (sw, sh) = (
            rng.range(1, bw as i64) as u32,
            rng.range(1, bh as i64) as u32,
        );
        let source = Rect::new(
            rng.range(0, (bw - sw) as i64) as i32,
            rng.range(0, (bh - sh) as i64) as i32,
            sw,
            sh,
        );
        let (dw, dh) = match rng.below(3) {
            0 => (sw, sh),
            1 => (sw * 2, sh * 2),
            _ => (rng.range(1, 90) as u32, rng.range(1, 90) as u32),
        };
        let destination = Rect::new(
            rng.range(-(dw as i64) - 4, 66) as i32,
            rng.range(-(dh as i64) - 4, 50) as i32,
            dw,
            dh,
        );
        let mut layer = Layer::new(BufferId(1), source, destination, 0);
        layer.transform = transforms[rng.below(6) as usize];
        let case = fixed_case(caps, vec![layer], xr24_buffers(&[(1, Size::new(bw, bh))]));
        let planned = plan(&case.frame(), &case.caps, &case);
        let advice = case.advise().unwrap();
        let l = advice.layers()[0];
        match planned {
            Ok(p) if p.assignments().is_empty() => {
                assert_eq!(l.placement, Placement::Hidden, "{layer:?}");
                kinds[0] += 1;
            }
            Ok(p) => {
                assert_eq!(
                    l.placement,
                    Placement::Plane(p.assignments()[0]),
                    "{layer:?}"
                );
                kinds[1] += 1;
            }
            Err(PlanError::CannotClip { .. }) => {
                assert_eq!(
                    (l.placement, l.blocker),
                    (Placement::Compose, Blocker::Clip),
                    "{layer:?}"
                );
                kinds[2] += 1;
            }
            Err(e) => panic!("{layer:?}: {e:?}"),
        }
        // explain_layer clips the same way.
        let mut rows = [RejectReasons::NONE; MAX_PLANES];
        explain_layer(&layer, &case.buffers[0].1, &case.caps, OUT, &mut rows).unwrap();
        if l.placement != Placement::Compose {
            assert!(rows[0].is_empty(), "{layer:?}: {rows:?}");
        }
    }
    assert!(kinds.iter().all(|&k| k > 300), "{kinds:?}");
}

fn two_full_layers_on_am62p() -> Case {
    let full = Rect::new(0, 0, 1920, 1080);
    fixed_case(
        am62p_like(),
        vec![
            Layer::new(BufferId(1), full, full, 0),
            Layer::new(BufferId(2), full, full, 1),
        ],
        xr24_buffers(&[(1, Size::new(1920, 1080)), (2, Size::new(1920, 1080))]),
    )
}

/// Test 7: a check that refuses the top layer on any plane gets it composed
/// with `Blocker::Backend`; `GiveUp` aborts the call.
#[test]
fn backend_refusal_and_give_up() {
    let case = two_full_layers_on_am62p();
    let mut calls = 0;
    let advice = advise(
        &case.frame(),
        &case.caps,
        &case,
        &case.options(),
        |p, compose| {
            calls += 1;
            let top_on_plane = p
                .assignments()
                .iter()
                .enumerate()
                .any(|(i, a)| Some(i) != compose && a.layer == 1);
            if top_on_plane {
                Verdict::TryNext
            } else {
                Verdict::Accept
            }
        },
    )
    .unwrap();
    let l = advice.layers();
    assert!(matches!(l[0].placement, Placement::Plane(_)));
    assert_eq!(
        (l[1].placement, l[1].blocker),
        (Placement::Compose, Blocker::Backend)
    );
    assert_eq!(advice.compose_run(), Some((1, 1)));
    assert_eq!(advice.backend_checks(), calls);

    // The check sees the composition target at the index it is told.
    let mut seen_target = false;
    let _ = advise(
        &case.frame(),
        &case.caps,
        &case,
        &case.options(),
        |p, compose| {
            if let Some(i) = compose {
                let a = p.assignments()[i];
                seen_target |=
                    a.destination == Rect::new(0, 0, 1920, 1080) && a.source == a.destination;
            }
            Verdict::TryNext
        },
    );
    assert!(seen_target);

    let mut asked = 0;
    assert_eq!(
        advise(&case.frame(), &case.caps, &case, &case.options(), |_, _| {
            asked += 1;
            Verdict::GiveUp
        }),
        Err(AdviceError::Aborted)
    );
    assert_eq!(asked, 1);

    // Without composition the refusal is reported per layer.
    let none = advise(
        &case.frame(),
        &case.caps,
        &case,
        &AdviseOptions::default(),
        |_, _| Verdict::TryNext,
    )
    .unwrap();
    assert!(none.layers().iter().all(|l| l.blocker == Blocker::Backend));
}

/// Test 7: a check that refuses every candidate with no composition target
/// is `Ok` (design §6, decision 13): every shown layer is `Compose` with
/// `Blocker::Backend`, there is no target and no run, and the call is not
/// an error. Pinned so a change to `Blocker` or an error is deliberate.
#[test]
fn refused_frame_without_target_is_ok_and_all_composed() {
    let case = two_full_layers_on_am62p();
    let advice = advise(
        &case.frame(),
        &case.caps,
        &case,
        &AdviseOptions::default(),
        |_, _| Verdict::TryNext,
    )
    .expect("a refused frame with no target is Ok, not an error");
    assert_eq!(advice.layers().len(), 2);
    for l in advice.layers() {
        assert_eq!(l.placement, Placement::Compose);
        assert_eq!(l.blocker, Blocker::Backend);
    }
    assert!(advice.compose().is_none());
    assert_eq!(advice.promoted(), 0);
    assert!(!advice.is_direct());
    // The per-layer placements carry the answer; there is no run to name.
    assert_eq!(advice.compose_run(), None);
    assert_eq!(advice.candidates(), 1);
}

/// Test 7: the check is never called more than `backend_checks` times over
/// the whole call, whatever it answers.
#[test]
fn backend_check_count_is_bounded() {
    each_case(6, |case| {
        for budget in [0, 1, 3, 32] {
            let mut options = case.options();
            options.limits.backend_checks = budget;
            let mut calls = 0u32;
            let mut rng = Rng::new(case.seed);
            let result = advise(&case.frame(), &case.caps, case, &options, |_, _| {
                calls += 1;
                if rng.chance(60) {
                    Verdict::TryNext
                } else {
                    Verdict::Accept
                }
            });
            if calls > budget {
                fail(case, &format!("{calls} checks for a budget of {budget}"));
            }
            if let Ok(a) = result {
                if a.backend_checks() != calls {
                    fail(case, "backend_checks() differs from the calls");
                }
            }
        }
    });
}

/// Test 8: tiny budgets end in `LimitExceeded` or an advice with
/// `Blocker::Budget`, never a hang or a wrong answer.
#[test]
fn tiny_budgets_are_honoured() {
    let full = Rect::new(0, 0, 1920, 1080);
    let mut caps = single_output(Size::new(1920, 1080));
    for id in 1..=3 {
        let p = PlaneCaps {
            id: PlaneId(id),
            outputs: 1,
            blend_modes: ALL_BLENDS,
            zpos: Zpos::Range { min: 0, max: 2 },
            ..PlaneCaps::default()
        };
        caps.planes.push(with_formats(p, &[XR24])).unwrap();
    }
    let case = fixed_case(
        caps,
        (0..3)
            .map(|i| Layer::new(BufferId(1), full, full, i))
            .collect(),
        xr24_buffers(&[(1, Size::new(1920, 1080))]),
    );
    assert!(case.advise().unwrap().is_direct());
    // Three layers need 1 + 2 + 3 = 6 steps; two need 3. The frame as it
    // is and every one-layer run are cut, a two-layer run fits.
    let mut options = case.options();
    options.limits.steps = 3;
    let advice = advise(&case.frame(), &case.caps, &case, &options, |_, _| {
        Verdict::Accept
    })
    .unwrap();
    assert_eq!(advice.compose_run(), Some((0, 1)));
    assert_eq!(advice.candidates(), 5);
    let blockers: Vec<Blocker> = advice.layers().iter().map(|l| l.blocker).collect();
    assert_eq!(blockers, [Blocker::Budget, Blocker::Budget, Blocker::None]);

    options.limits.steps = 0;
    assert_eq!(
        advise(&case.frame(), &case.caps, &case, &options, |_, _| {
            Verdict::Accept
        }),
        Err(AdviceError::LimitExceeded)
    );
    options = case.options();
    options.max_candidates = 0;
    assert_eq!(
        advise(&case.frame(), &case.caps, &case, &options, |_, _| {
            Verdict::Accept
        }),
        Err(AdviceError::LimitExceeded)
    );
    // A refused frame and no room for a second candidate.
    options.max_candidates = 1;
    assert_eq!(
        advise(&case.frame(), &case.caps, &case, &options, |_, _| {
            Verdict::TryNext
        }),
        Err(AdviceError::LimitExceeded)
    );

    // Random tiny budgets: an answer is either an error of the budget, or an
    // advice whose reduced frame is legal.
    each_case(6, |case| {
        let mut rng = Rng::new(case.seed ^ 0xb0d9);
        let mut options = case.options();
        options.limits.steps = rng.below(8) as u32;
        options.max_candidates = rng.below(6) as u32;
        match advise(&case.frame(), &case.caps, case, &options, |_, _| {
            Verdict::Accept
        }) {
            Ok(a) => {
                if a.candidates() > options.max_candidates {
                    fail(case, "more candidates than allowed");
                }
                if plan_reduced(case, a.compose_run()).is_err() {
                    fail(case, "advice under a tiny budget does not plan");
                }
            }
            Err(AdviceError::LimitExceeded | AdviceError::NoComposePlane { .. }) => {}
            Err(e) => fail(case, &format!("unexpected {e:?}")),
        }
    });
}

/// Test 8: the search ends at `MAX_LAYERS` layers and `MAX_PLANES` planes,
/// with a check that refuses everything, within `MAX_CANDIDATES`.
#[test]
fn terminates_at_the_maxima() {
    let full = Rect::new(0, 0, 1920, 1080);
    let mut caps = single_output(Size::new(1920, 1080));
    for id in 0..MAX_PLANES as u32 {
        let p = PlaneCaps {
            id: PlaneId(id + 1),
            outputs: 1,
            blend_modes: ALL_BLENDS,
            zpos: Zpos::Range {
                min: 0,
                max: MAX_PLANES as i32 - 1,
            },
            ..PlaneCaps::default()
        };
        caps.planes.push(with_formats(p, &[XR24])).unwrap();
    }
    let layers: Vec<Layer> = (0..MAX_LAYERS as i32)
        .map(|i| Layer::new(BufferId(1), full, full, i))
        .collect();
    let case = fixed_case(caps, layers, xr24_buffers(&[(1, Size::new(1920, 1080))]));
    let advice = case.advise().unwrap();
    assert!(advice.is_direct() && advice.promoted() == MAX_LAYERS);

    // A check that refuses everything: the shared check budget ends it.
    let mut calls = 0u32;
    let result = advise(&case.frame(), &case.caps, &case, &case.options(), |_, _| {
        calls += 1;
        Verdict::TryNext
    });
    assert_eq!(result, Err(AdviceError::LimitExceeded));
    assert_eq!(calls, SearchLimits::default().backend_checks);
    // Without a check budget every candidate runs out of steps or
    // assignments, and the call still ends.
    let unlimited = AdviseOptions {
        limits: SearchLimits {
            backend_checks: u32::MAX,
            ..SearchLimits::default()
        },
        ..case.options()
    };
    let result = advise(&case.frame(), &case.caps, &case, &unlimited, |_, _| {
        Verdict::TryNext
    });
    assert_eq!(result, Err(AdviceError::LimitExceeded));

    // Every layer needs composing; every run is tried at most once.
    let mut nothing = case.clone();
    nothing.buffers[0].1.format = HWL0;
    let advice = nothing.advise().unwrap();
    assert_eq!(advice.compose_run(), Some((0, MAX_LAYERS - 1)));
    assert!(advice.candidates() <= MAX_CANDIDATES);
    assert!(advice
        .layers()
        .iter()
        .all(|l| l.blocker == Blocker::Static && l.reasons == RejectReasons::FORMAT));
    assert_eq!(advice.candidates(), 1);
    // And no plane takes the composition target either.
    nothing.target.format = HWL0;
    assert_eq!(
        nothing.advise(),
        Err(AdviceError::NoComposePlane {
            reasons: RejectReasons::FORMAT
        })
    );

    // Too many layers is a whole-call error, as for the planner.
    let mut more = case.layers.clone();
    more.push(case.layers[0]);
    let frame = Frame {
        output: OUT,
        layers: &more,
    };
    assert_eq!(
        advise(&frame, &case.caps, &case, &case.options(), |_, _| {
            Verdict::Accept
        }),
        Err(AdviceError::TooManyLayers {
            count: MAX_LAYERS + 1
        })
    );
}

/// Decision 4: unknown buffers and bad geometry are whole-call errors, as in
/// the planner; an unknown output too.
#[test]
fn input_errors_are_whole_call_errors() {
    let case = two_full_layers_on_am62p();
    let check = |layers: &[Layer]| {
        advise(
            &Frame {
                output: OUT,
                layers,
            },
            &case.caps,
            &case,
            &case.options(),
            |_, _| Verdict::Accept,
        )
    };
    let mut bad = case.layers.clone();
    bad[1].buffer = BufferId(99);
    assert_eq!(
        check(&bad),
        Err(AdviceError::UnknownBuffer {
            layer: 1,
            buffer: BufferId(99)
        })
    );
    bad = case.layers.clone();
    bad[0].source = Rect::new(1, 0, 1920, 1080);
    assert_eq!(check(&bad), Err(AdviceError::InvalidSource { layer: 0 }));
    bad = case.layers.clone();
    bad[1].destination.width = 0;
    assert_eq!(
        check(&bad),
        Err(AdviceError::InvalidDestination { layer: 1 })
    );
    assert_eq!(
        advise(
            &Frame {
                output: OutputId(5),
                layers: &case.layers,
            },
            &case.caps,
            &case,
            &case.options(),
            |_, _| Verdict::Accept,
        ),
        Err(AdviceError::UnknownOutput(OutputId(5)))
    );
    let mut rows = [RejectReasons::NONE; MAX_PLANES];
    assert_eq!(
        explain_layer(
            &case.layers[0],
            &case.buffers[0].1,
            &case.caps,
            OutputId(5),
            &mut rows
        ),
        Err(AdviceError::UnknownOutput(OutputId(5)))
    );
}

/// Decision 8: the same input always gives the same advice; and the advice
/// does not depend on a buffer id that happens to be the module's internal
/// one for the composition target.
#[test]
fn advice_is_deterministic() {
    each_case(6, |case| {
        if case.advise() != case.advise() {
            fail(case, "two calls differ");
        }
        let mut renamed = case.clone();
        for (k, (id, _)) in renamed.buffers.iter_mut().enumerate() {
            let new = BufferId(u32::MAX - k as u32);
            for l in renamed.layers.iter_mut().filter(|l| l.buffer == *id) {
                l.buffer = new;
            }
            *id = new;
        }
        let strip = |r: Result<Advice, AdviceError>| r.map(|a| a.layers().to_vec());
        if strip(case.advise()) != strip(renamed.advise()) {
            fail(case, "advice depends on buffer ids");
        }
    });
}

/// The default options allow every candidate run and the planner's limits.
#[test]
fn default_options() {
    let o = AdviseOptions::default();
    assert_eq!(o.max_candidates, MAX_CANDIDATES);
    assert_eq!(
        MAX_CANDIDATES as usize,
        1 + MAX_LAYERS * (MAX_LAYERS + 1) / 2
    );
    assert_eq!(o.limits, SearchLimits::default());
    assert_eq!(o.compose, None);
    // An empty frame is direct with no check beyond the planner's one.
    let case = fixed_case(am62p_like(), Vec::new(), Vec::new());
    let a = case.advise().unwrap();
    assert!(a.is_direct() && a.promoted() == 0 && a.layers().is_empty());
    assert_eq!(a.output(), OUT);
    // plan_with with the same check agrees on the empty frame.
    assert!(plan_with(
        &case.frame(),
        &case.caps,
        &case,
        SearchLimits::default(),
        |_| Verdict::Accept
    )
    .is_ok());
}

/// Test 12: the footprint of an advice, pinned. It lives on the caller's
/// stack; growing it is a decision, not an accident.
#[test]
fn advice_footprint_is_pinned() {
    let size = core::mem::size_of::<Advice>();
    #[cfg(target_pointer_width = "64")]
    assert_eq!(size, ADVICE_SIZE_64, "size_of::<Advice>() changed");
    assert!(size <= 2048, "size_of::<Advice>() = {size}");
    assert!(core::mem::size_of::<AdviseOptions>() <= 32);
}

#[cfg(target_pointer_width = "64")]
const ADVICE_SIZE_64: usize = 1240;
