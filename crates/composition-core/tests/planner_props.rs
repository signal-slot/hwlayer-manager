//! Property tests of the planner on well-formed frames (SoW §6, §10).
//!
//! Random but valid [`Capabilities`] and frames come from a seeded,
//! deterministic generator written in this file; there is no dependency. The
//! planner is called through the public API only, with no backend.
//!
//! Properties:
//! 1. soundness: every returned plan satisfies every plane constraint,
//!    stacking, plane exclusivity and resource budgets, checked here
//!    independently of `planner.rs`;
//! 2. completeness: for at most 3 layers and 4 planes an exhaustive search
//!    (every injective layer-to-plane map and every zpos choice) finds a legal
//!    assignment exactly when the planner does; for larger frames a legal
//!    assignment is planted by construction and the planner must find one;
//! 3. determinism;
//! 4. an error names an unmet constraint;
//! 5. a frame that fits the hardware is never rejected.
//!
//! A failure prints the seed and the whole input. Reproduce one case with
//! `PLANNER_PROPS_SEED=<seed> cargo test -p composition-core --test
//! planner_props`; `PLANNER_PROPS_CASES=<n>` changes the number of cases.
//!
//! Where `SearchLimits` legitimately cut the search (`LimitExceeded`), only
//! that outcome is accepted: see `tight_limits_never_give_a_wrong_answer`.
//! With the default limits no case of the exhaustive size can be cut: the
//! search visits at most a few hundred nodes there.

use composition_core::planner::{plan, plan_with};
use composition_core::{
    Assignment, BlendMode, BufferId, BufferInfo, BufferLookup, Capabilities, FormatModifier, Frame,
    Layer, LayerTraits, Modifier, OutputCaps, OutputId, PixelFormat, Plan, PlanError, PlaneCaps,
    PlaneId, PlaneKind, Rect, RejectReasons, ResourceGroup, ScaleLimits, SearchLimits, Size,
    Transform, Verdict, Zpos, ALPHA_OPAQUE,
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
    /// Uniform in `0..n`.
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    /// True with probability `percent`/100.
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i as u64 + 1) as usize);
        }
    }
}

const FORMATS: [PixelFormat; 5] = [
    PixelFormat::XRGB8888,
    PixelFormat::ARGB8888,
    PixelFormat::RGB565,
    PixelFormat::NV12,
    PixelFormat::ABGR8888,
];
const MODIFIERS: [Modifier; 2] = [Modifier::LINEAR, Modifier(0x0100_0000_0000_0001)];
const BLENDS: [BlendMode; 3] = [
    BlendMode::Opaque,
    BlendMode::Premultiplied,
    BlendMode::Coverage,
];
const TRANSFORMS: [Transform; 6] = [
    Transform::Normal,
    Transform::Rotate90,
    Transform::Rotate180,
    Transform::Rotate270,
    Transform::FlipH,
    Transform::FlipV,
];

#[derive(Clone, Debug)]
struct Case {
    seed: u64,
    caps: Capabilities,
    buffers: Vec<(BufferId, BufferInfo)>,
    layers: Vec<Layer>,
    output: OutputId,
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
        self.caps
            .outputs
            .iter()
            .position(|o| o.id == self.output)
            .unwrap()
    }
    fn output_size(&self) -> Size {
        self.caps.outputs[self.output_index()].size
    }
}

/// How restrictive the generated hardware is, in percent per feature.
#[derive(Clone, Copy)]
struct Style {
    max_planes: u64,
    /// Chance that a plane lacks a given capability.
    restrict: u64,
    /// Whether zpos ranges are wide enough to make planting likely.
    roomy_zpos: bool,
}

fn gen_caps(rng: &mut Rng, style: Style) -> Capabilities {
    let mut caps = Capabilities::default();
    let outputs = 1 + rng.below(2) as usize;
    for i in 0..outputs {
        let size = [Size::new(64, 48), Size::new(100, 60)][rng.below(2) as usize];
        caps.outputs
            .push(OutputCaps {
                id: OutputId(10 + i as u32),
                size,
                refresh_mhz: 60_000,
            })
            .unwrap();
    }
    let planes = 1 + rng.below(style.max_planes) as usize;
    for i in 0..planes {
        let mut p = PlaneCaps {
            id: PlaneId(100 + i as u32 * 3),
            kind: [PlaneKind::Primary, PlaneKind::Overlay, PlaneKind::Cursor]
                [rng.below(3) as usize],
            ..PlaneCaps::default()
        };
        let all_outputs = (1u32 << outputs) - 1;
        p.outputs = if rng.chance(style.restrict / 3) {
            rng.below(all_outputs as u64 + 1) as u32
        } else {
            all_outputs
        };
        for f in FORMATS {
            for m in MODIFIERS {
                let keep = if m == Modifier::LINEAR { 65 } else { 25 };
                if !rng.chance(style.restrict) && rng.chance(keep + 10) {
                    p.formats
                        .push(FormatModifier {
                            format: f,
                            modifier: m,
                        })
                        .unwrap();
                }
            }
        }
        if rng.chance(100 - style.restrict) {
            p.scaling = Some(ScaleLimits {
                max_upscale: 0x1_0000 + rng.below(0x3_0000) as u32,
                max_downscale: 0x1_0000 + rng.below(0x3_0000) as u32,
            });
        }
        if rng.chance(30) {
            p.max_source = Some(Size::new(
                rng.range(16, 100) as u32,
                rng.range(12, 60) as u32,
            ));
        }
        p.plane_alpha = !rng.chance(style.restrict);
        for b in BLENDS {
            if !rng.chance(style.restrict) {
                p.blend_modes |= b.mask();
            }
        }
        for t in TRANSFORMS {
            let keep = if t == Transform::Normal {
                0
            } else {
                style.restrict
            };
            if !rng.chance(keep) && rng.chance(60) {
                p.transforms |= t.mask();
            }
        }
        p.zpos = if style.roomy_zpos || rng.chance(50) {
            let min = [-1, 0, 0, 1][rng.below(4) as usize];
            Zpos::Range {
                min,
                max: min + rng.range(2, 6) as i32,
            }
        } else if rng.chance(15) {
            // Edge of the i32 range, narrow enough to enumerate.
            let min = i32::MAX - rng.range(0, 3) as i32;
            Zpos::Range { min, max: i32::MAX }
        } else {
            Zpos::Fixed(rng.range(-1, 4) as i32)
        };
        p.full_output_only = rng.chance(8);
        caps.planes.push(p).unwrap();
    }
    for _ in 0..rng.below(3) {
        let mut consumers = LayerTraits(0);
        for t in [
            LayerTraits::ANY,
            LayerTraits::SCALED_H,
            LayerTraits::SCALED_V,
            LayerTraits::YUV,
        ] {
            if rng.chance(40) {
                consumers = consumers.union(t);
            }
        }
        caps.resource_groups
            .push(ResourceGroup {
                consumers,
                planes: rng.below(1 << planes) as u32,
                capacity: rng.below(4) as u8,
            })
            .unwrap();
    }
    caps.replace_pending = rng.chance(50);
    caps
}

/// One random layer and its buffer; sizes stay small so that scaling limits
/// and output edges matter.
fn gen_layer(rng: &mut Rng, id: u32, out: Size, z_order: i32) -> (Layer, BufferInfo) {
    let (bw, bh) = (rng.range(4, 90) as u32, rng.range(4, 70) as u32);
    let info = BufferInfo {
        size: Size::new(bw, bh),
        format: FORMATS[rng.below(FORMATS.len() as u64) as usize],
        modifier: MODIFIERS[rng.below(2) as usize],
    };
    let source = if rng.chance(60) {
        Rect::new(0, 0, bw, bh)
    } else {
        let (w, h) = (rng.range(1, bw as i64), rng.range(1, bh as i64));
        Rect::new(
            rng.range(0, (bw as i64) - w) as i32,
            rng.range(0, (bh as i64) - h) as i32,
            w as u32,
            h as u32,
        )
    };
    let transform = if rng.chance(75) {
        Transform::Normal
    } else {
        TRANSFORMS[rng.below(6) as usize]
    };
    let (ow, oh) = if transform.swaps_axes() {
        (source.height, source.width)
    } else {
        (source.width, source.height)
    };
    // Destination classes: whole output, unscaled inside, scaled inside,
    // unscaled and partly off the output (Normal only, so that the clip is
    // exact), unscaled and entirely off the output.
    let destination = match rng.below(10) {
        0 | 1 => Rect::new(0, 0, out.width, out.height),
        2..=4 => {
            let (w, h) = (ow.min(out.width), oh.min(out.height));
            if (w, h) == (ow, oh) {
                Rect::new(
                    rng.range(0, (out.width - w) as i64) as i32,
                    rng.range(0, (out.height - h) as i64) as i32,
                    w,
                    h,
                )
            } else {
                Rect::new(0, 0, out.width, out.height)
            }
        }
        5..=7 => {
            let (w, h) = (
                rng.range(1, out.width as i64) as u32,
                rng.range(1, out.height as i64) as u32,
            );
            Rect::new(
                rng.range(0, (out.width - w) as i64) as i32,
                rng.range(0, (out.height - h) as i64) as i32,
                w,
                h,
            )
        }
        8 if transform == Transform::Normal => Rect::new(
            rng.range(-(source.width as i64) + 1, out.width as i64 - 1) as i32,
            rng.range(-(source.height as i64) + 1, out.height as i64 - 1) as i32,
            source.width,
            source.height,
        ),
        8 => Rect::new(0, 0, out.width, out.height),
        _ if transform == Transform::Normal => Rect::new(
            out.width as i32 + rng.range(0, 5) as i32,
            rng.range(-5, 5) as i32,
            source.width,
            source.height,
        ),
        _ => Rect::new(0, 0, out.width, out.height),
    };
    let mut layer = Layer::new(BufferId(id), source, destination, z_order);
    layer.transform = transform;
    layer.blend = BLENDS[rng.below(3) as usize];
    layer.alpha = if rng.chance(70) {
        ALPHA_OPAQUE
    } else {
        rng.below(ALPHA_OPAQUE as u64) as u16
    };
    (layer, info)
}

fn gen_case(seed: u64, style: Style, max_layers: u64) -> Case {
    let mut rng = Rng::new(seed);
    let caps = gen_caps(&mut rng, style);
    let output = caps.outputs[rng.below(caps.outputs.len() as u64) as usize].id;
    let out = caps.outputs.iter().find(|o| o.id == output).unwrap().size;
    let n = rng.below(max_layers + 1) as usize;
    let mut buffers = Vec::new();
    let mut layers = Vec::new();
    for i in 0..n {
        let z = rng.range(-2, 3) as i32;
        let (layer, info) = gen_layer(&mut rng, i as u32 + 1, out, z);
        buffers.push((layer.buffer, info));
        layers.push(layer);
    }
    Case {
        seed,
        caps,
        buffers,
        layers,
        output,
    }
}

// ------------------------------------------------------------------- oracle
//
// Everything below restates the rules of SoW §6 and the documentation of
// `caps.rs` without using the planner's code.

fn opaque_format(f: PixelFormat) -> bool {
    [
        PixelFormat::RGB565,
        PixelFormat::XRGB8888,
        PixelFormat::XBGR8888,
        PixelFormat::NV12,
        PixelFormat::NV21,
        PixelFormat::NV16,
        PixelFormat::YUYV,
        PixelFormat::UYVY,
        PixelFormat::YUV420,
    ]
    .contains(&f)
}

fn yuv_format(f: PixelFormat) -> bool {
    [
        PixelFormat::NV12,
        PixelFormat::NV21,
        PixelFormat::NV16,
        PixelFormat::YUYV,
        PixelFormat::UYVY,
        PixelFormat::YUV420,
    ]
    .contains(&f)
}

/// A layer as the hardware sees it: clipped to the output, or `None` if it is
/// entirely off. The generator only produces clips that are exact.
fn visible(layer: &Layer, out: Size) -> Option<(Rect, Rect)> {
    let d = layer.destination;
    let (l, t) = (d.x as i64, d.y as i64);
    let (r, b) = (l + d.width as i64, t + d.height as i64);
    let (cl, ct) = (l.max(0), t.max(0));
    let (cr, cb) = (r.min(out.width as i64), b.min(out.height as i64));
    if cl >= cr || ct >= cb {
        return None;
    }
    let dest = Rect::new(cl as i32, ct as i32, (cr - cl) as u32, (cb - ct) as u32);
    if dest == d {
        return Some((layer.source, d));
    }
    assert_eq!(layer.transform, Transform::Normal, "generator invariant");
    assert_eq!(
        (layer.source.width, layer.source.height),
        (d.width, d.height),
        "generator invariant"
    );
    let s = layer.source;
    Some((
        Rect::new(
            s.x + (cl - l) as i32,
            s.y + (ct - t) as i32,
            dest.width,
            dest.height,
        ),
        dest,
    ))
}

/// `from` to `to` along one axis is within the 16.16 limits.
fn axis_ok(from: u32, to: u32, s: ScaleLimits) -> bool {
    let (from, to) = (from as u128, to as u128);
    if to > from {
        to << 16 <= from * s.max_upscale as u128
    } else {
        from << 16 <= to * s.max_downscale as u128
    }
}

/// Every constraint that depends on one layer and one plane. `source` and
/// `dest` are the visible rectangles.
fn plane_reasons(
    layer: &Layer,
    source: Rect,
    dest: Rect,
    info: &BufferInfo,
    plane: &PlaneCaps,
    out_index: usize,
    out: Size,
) -> RejectReasons {
    let mut r = RejectReasons::NONE;
    if plane.outputs >> out_index & 1 == 0 {
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
    if (sw, sh) != (dest.width, dest.height) {
        let ok = plane
            .scaling
            .is_some_and(|s| axis_ok(sw, dest.width, s) && axis_ok(sh, dest.height, s));
        if !ok {
            r |= RejectReasons::SCALING;
        }
    }
    if let Some(max) = plane.max_source {
        if source.width > max.width || source.height > max.height {
            r |= RejectReasons::SOURCE_SIZE;
        }
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
    if plane.full_output_only
        && !(dest.x == 0 && dest.y == 0 && dest.width == out.width && dest.height == out.height)
    {
        r |= RejectReasons::FULL_OUTPUT;
    }
    r
}

fn traits_of(layer: &Layer, source: Rect, dest: Rect, info: &BufferInfo) -> LayerTraits {
    let (sw, sh) = if layer.transform.swaps_axes() {
        (source.height, source.width)
    } else {
        (source.width, source.height)
    };
    let mut t = LayerTraits::ANY;
    if sw != dest.width {
        t = t.union(LayerTraits::SCALED_H);
    }
    if sh != dest.height {
        t = t.union(LayerTraits::SCALED_V);
    }
    if yuv_format(info.format) {
        t = t.union(LayerTraits::YUV);
    }
    t
}

/// One layer that needs a plane.
struct Visible {
    index: usize,
    layer: Layer,
    source: Rect,
    dest: Rect,
    info: BufferInfo,
    traits: LayerTraits,
}

/// The layers that need a plane, bottom to top; equal z keeps frame order.
fn visible_layers(case: &Case) -> Vec<Visible> {
    let out = case.output_size();
    let mut v: Vec<Visible> = case
        .layers
        .iter()
        .enumerate()
        .filter_map(|(index, layer)| {
            let (source, dest) = visible(layer, out)?;
            let info = case.buffer_info(layer.buffer).unwrap();
            Some(Visible {
                index,
                layer: *layer,
                source,
                dest,
                info,
                traits: traits_of(layer, source, dest, &info),
            })
        })
        .collect();
    v.sort_by_key(|x| (x.layer.z_order, x.index));
    v
}

fn groups_ok(caps: &Capabilities, placed: &[(usize, LayerTraits)]) -> Result<(), String> {
    for (g, group) in caps.resource_groups.iter().enumerate() {
        let used = placed
            .iter()
            .filter(|(plane, traits)| {
                group.planes >> plane & 1 == 1 && group.consumers.intersects(*traits)
            })
            .count();
        if used > group.capacity as usize {
            return Err(format!(
                "resource group {g} over budget: {used} > {}",
                group.capacity
            ));
        }
    }
    Ok(())
}

/// Smallest stacking position of `plane` strictly above `below`.
fn lowest_above(plane: &PlaneCaps, below: Option<i64>) -> Option<i64> {
    match (plane.zpos, below) {
        (Zpos::Fixed(z), None) => Some(z as i64),
        (Zpos::Fixed(z), Some(b)) => (z as i64 > b).then_some(z as i64),
        (Zpos::Range { min, .. }, None) => Some(min as i64),
        (Zpos::Range { min, max }, Some(b)) => {
            let z = (min as i64).max(b + 1);
            (z <= max as i64).then_some(z)
        }
    }
}

/// Exhaustive search: every injective map of layers to statically compatible
/// planes and every zpos choice. Returns whether a legal assignment exists.
fn exhaustive_feasible(case: &Case) -> bool {
    let vis = visible_layers(case);
    let out = case.output_size();
    let oi = case.output_index();
    let n = case.caps.planes.len();
    let compat: Vec<Vec<bool>> = vis
        .iter()
        .map(|v| {
            (0..n)
                .map(|p| {
                    plane_reasons(
                        &v.layer,
                        v.source,
                        v.dest,
                        &v.info,
                        &case.caps.planes[p],
                        oi,
                        out,
                    )
                    .is_empty()
                })
                .collect()
        })
        .collect();
    #[allow(clippy::too_many_arguments)]
    fn rec(
        case: &Case,
        vis: &[Visible],
        compat: &[Vec<bool>],
        k: usize,
        used: u32,
        prev: Option<i64>,
        placed: &mut Vec<(usize, LayerTraits)>,
    ) -> bool {
        if k == vis.len() {
            return groups_ok(&case.caps, placed).is_ok();
        }
        for p in 0..case.caps.planes.len() {
            if used >> p & 1 == 1 || !compat[k][p] {
                continue;
            }
            let zs: Vec<i64> = match case.caps.planes[p].zpos {
                Zpos::Fixed(z) => vec![z as i64],
                Zpos::Range { min, max } => (min as i64..=max as i64).collect(),
            };
            for z in zs {
                if prev.is_some_and(|b| z <= b) {
                    continue;
                }
                placed.push((p, vis[k].traits));
                let ok = rec(case, vis, compat, k + 1, used | 1 << p, Some(z), placed);
                placed.pop();
                if ok {
                    return true;
                }
            }
        }
        false
    }
    rec(case, &vis, &compat, 0, 0, None, &mut Vec::new())
}

/// Soundness of one plan, from the plan alone.
fn check_plan(case: &Case, plan: &Plan) -> Result<(), String> {
    let out = case.output_size();
    let oi = case.output_index();
    if plan.output() != case.output {
        return Err(format!("plan output {:?}", plan.output()));
    }
    if !plan.matches(&case.frame()) {
        return Err("plan does not match its own frame".into());
    }
    let vis = visible_layers(case);
    let a: &[Assignment] = plan.assignments();
    if a.len() != vis.len() {
        return Err(format!(
            "{} assignments for {} visible layers",
            a.len(),
            vis.len()
        ));
    }
    let mut used = 0u32;
    let mut prev: Option<i64> = None;
    let mut placed = Vec::new();
    for (k, (asg, v)) in a.iter().zip(&vis).enumerate() {
        let ctx = |m: &str| format!("assignment {k} (layer {}): {m}", asg.layer);
        if asg.layer != v.index {
            return Err(ctx(&format!(
                "layer order: expected layer {} at this stacking position",
                v.index
            )));
        }
        let Some(plane) = case.caps.planes.get(asg.plane_index) else {
            return Err(ctx("plane_index out of range"));
        };
        if plane.id != asg.plane {
            return Err(ctx("plane id does not match plane_index"));
        }
        if used >> asg.plane_index & 1 == 1 {
            return Err(ctx("plane used twice"));
        }
        used |= 1 << asg.plane_index;
        if asg.source != v.source || asg.destination != v.dest {
            return Err(ctx(&format!(
                "rects {:?} -> {:?}, expected {:?} -> {:?}",
                asg.source, asg.destination, v.source, v.dest
            )));
        }
        let r = plane_reasons(&v.layer, v.source, v.dest, &v.info, plane, oi, out);
        if !r.is_empty() {
            return Err(ctx(&format!("plane constraint violated: {r:?}")));
        }
        let z = asg.zpos as i64;
        match plane.zpos {
            Zpos::Fixed(f) if f != asg.zpos => return Err(ctx("zpos differs from fixed zpos")),
            Zpos::Range { min, max } if asg.zpos < min || asg.zpos > max => {
                return Err(ctx("zpos outside the plane's range"))
            }
            _ => {}
        }
        if prev.is_some_and(|b| z <= b) {
            return Err(ctx(&format!("zpos {z} not above {prev:?}")));
        }
        prev = Some(z);
        placed.push((asg.plane_index, v.traits));
    }
    groups_ok(&case.caps, &placed)
}

/// Checks an error names at least one constraint that really is unmet.
fn check_error(case: &Case, err: &PlanError) -> Result<(), String> {
    match err {
        PlanError::NoPlane { layer, reasons } => {
            if reasons.is_empty() {
                return Err("NoPlane names no constraint".into());
            }
            if *layer >= case.layers.len() {
                return Err(format!("NoPlane layer {layer} out of range"));
            }
            let any = RejectReasons::OUTPUT
                | RejectReasons::FORMAT
                | RejectReasons::SCALING
                | RejectReasons::SOURCE_SIZE
                | RejectReasons::ALPHA
                | RejectReasons::BLEND
                | RejectReasons::TRANSFORM
                | RejectReasons::FULL_OUTPUT
                | RejectReasons::Z_ORDER
                | RejectReasons::RESOURCE
                | RejectReasons::IN_USE;
            if reasons.bits() & !any.bits() != 0 {
                return Err(format!("unknown reason bits {reasons:?}"));
            }
            let vis = visible_layers(case);
            let Some(v) = vis.iter().find(|v| v.index == *layer) else {
                return Err(format!(
                    "NoPlane blames layer {layer}, which needs no plane"
                ));
            };
            let (oi, out) = (case.output_index(), case.output_size());
            let statics: Vec<RejectReasons> = case
                .caps
                .planes
                .iter()
                .map(|p| plane_reasons(&v.layer, v.source, v.dest, &v.info, p, oi, out))
                .collect();
            let attachable = case
                .caps
                .planes
                .iter()
                .filter(|p| p.outputs >> oi & 1 == 1)
                .count();
            if vis.len() > attachable {
                // The pre-search "more layers than planes" check.
                if !reasons.contains(RejectReasons::IN_USE) {
                    return Err("too few planes, but IN_USE is not named".into());
                }
            } else if statics.iter().all(|s| !s.is_empty()) {
                // No plane can ever take this layer: the reasons must be
                // exactly what the planes reject it for.
                let union = statics.iter().fold(RejectReasons::NONE, |acc, s| acc | *s);
                if union != *reasons {
                    return Err(format!(
                        "layer {layer} fits no plane for {union:?}, error says {reasons:?}"
                    ));
                }
            } else if reasons.bits()
                & (RejectReasons::Z_ORDER | RejectReasons::RESOURCE | RejectReasons::IN_USE).bits()
                == 0
            {
                return Err(format!(
                    "layer {layer} has compatible planes but only static reasons {reasons:?}"
                ));
            }
            Ok(())
        }
        // Well-formed input never needs these.
        other => Err(format!(
            "unexpected error for a well-formed frame: {other:?}"
        )),
    }
}

// --------------------------------------------------------------- the checks

const DEFAULT_CASES: u64 = 4000;

fn cases() -> impl Iterator<Item = u64> {
    if let Ok(s) = std::env::var("PLANNER_PROPS_SEED") {
        let seed = s.parse::<u64>().expect("PLANNER_PROPS_SEED is a number");
        return seed..seed + 1;
    }
    let n = std::env::var("PLANNER_PROPS_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_CASES);
    0..n
}

fn fail(case: &Case, what: &str, msg: &str) -> ! {
    panic!(
        "{what} failed: {msg}\nseed: {} (rerun with PLANNER_PROPS_SEED={})\ninput: {:#?}",
        case.seed, case.seed, case
    );
}

/// Properties 1, 3 and 4 on any case. Returns the planner's verdict.
fn check_basic(case: &Case, what: &str) -> Result<Plan, PlanError> {
    let result = plan(&case.frame(), &case.caps, case);
    if result != plan(&case.frame(), &case.caps, case) {
        fail(case, what, "not deterministic");
    }
    match &result {
        Ok(p) => {
            if let Err(m) = check_plan(case, p) {
                fail(case, what, &format!("unsound plan: {m}\nplan: {p:#?}"));
            }
        }
        Err(e) => {
            if let Err(m) = check_error(case, e) {
                fail(case, what, &format!("{m}\nerror: {e:?}"));
            }
        }
    }
    result
}

#[test]
fn small_cases_match_exhaustive_search() {
    let style = Style {
        max_planes: 4,
        restrict: 30,
        roomy_zpos: false,
    };
    let (mut yes, mut no) = (0, 0);
    for seed in cases() {
        let case = gen_case(seed, style, 3);
        assert!(case.caps.planes.len() <= 4 && case.layers.len() <= 3);
        let result = check_basic(&case, "small case");
        let feasible = exhaustive_feasible(&case);
        match (&result, feasible) {
            (Ok(_), true) => yes += 1,
            (Err(_), false) => no += 1,
            (Ok(_), false) => fail(
                &case,
                "completeness",
                "planner found a plan, exhaustive search did not",
            ),
            (Err(e), true) => fail(
                &case,
                "completeness",
                &format!("exhaustive search found an assignment, planner said {e:?}"),
            ),
        }
    }
    // The generator must exercise both outcomes, or the test proves little.
    if std::env::var("PLANNER_PROPS_SEED").is_err() && cases().count() >= 1000 {
        assert!(yes >= 300, "only {yes} feasible cases");
        assert!(no >= 300, "only {no} infeasible cases");
    }
}

/// Plants a legal assignment: choose a plane per visible layer, then make the
/// layer fit it by rejection sampling.
fn gen_planted(seed: u64) -> Option<Case> {
    let style = Style {
        max_planes: 10,
        restrict: 15,
        roomy_zpos: true,
    };
    let mut rng = Rng::new(seed ^ 0xabcd_ef01);
    let caps = gen_caps(&mut rng, style);
    let output = caps.outputs[rng.below(caps.outputs.len() as u64) as usize].id;
    let oi = caps.outputs.iter().position(|o| o.id == output).unwrap();
    let out = caps.outputs[oi].size;
    let mut candidates: Vec<usize> = (0..caps.planes.len())
        .filter(|&p| caps.planes[p].outputs >> oi & 1 == 1)
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let n = 1 + rng.below(candidates.len().min(6) as u64) as usize;
    rng.shuffle(&mut candidates);
    candidates.truncate(n);
    // Planes in stacking order must admit a strictly increasing zpos.
    let mut prev = None;
    let mut chosen = candidates.clone();
    chosen.sort_by_key(|&p| match caps.planes[p].zpos {
        Zpos::Fixed(z) => z as i64,
        Zpos::Range { min, .. } => min as i64,
    });
    for &p in &chosen {
        prev = Some(lowest_above(&caps.planes[p], prev)?);
    }
    let mut buffers = Vec::new();
    let mut layers = Vec::new();
    for (i, &p) in chosen.iter().enumerate() {
        let z = i as i32 * 2 + rng.range(0, 1) as i32;
        let mut found = None;
        for _ in 0..60 {
            let (layer, info) = gen_layer(&mut rng, i as u32 + 1, out, z);
            let Some((s, d)) = visible(&layer, out) else {
                continue;
            };
            let r = plane_reasons(&layer, s, d, &info, &caps.planes[p], oi, out);
            if r.is_empty() {
                found = Some((layer, info));
                break;
            }
        }
        let (layer, info) = found?;
        buffers.push((layer.buffer, info));
        layers.push(layer);
    }
    // Shuffle the frame order: the planner orders by z, not by position.
    let mut order: Vec<usize> = (0..layers.len()).collect();
    rng.shuffle(&mut order);
    let layers: Vec<Layer> = order.iter().map(|&i| layers[i]).collect();
    let case = Case {
        seed,
        caps,
        buffers,
        layers,
        output,
    };
    // The plant must also respect resource groups.
    let placed: Vec<(usize, LayerTraits)> = {
        let vis = visible_layers(&case);
        vis.iter()
            .map(|v| (chosen[v.layer.buffer.0 as usize - 1], v.traits))
            .collect()
    };
    groups_ok(&case.caps, &placed).ok()?;
    Some(case)
}

#[test]
fn planted_assignments_are_always_found() {
    let mut planted = 0;
    let mut cut = 0;
    for seed in cases() {
        let Some(case) = gen_planted(seed) else {
            continue;
        };
        planted += 1;
        match check_basic(&case, "planted case") {
            Ok(_) => {}
            // The default step budget (20 000) can end a search over many
            // planes and layers before it finds the plant; that is a
            // documented, legitimate cut, but never a wrong answer.
            Err(PlanError::LimitExceeded) => cut += 1,
            Err(e) => fail(
                &case,
                "property 5",
                &format!("a frame that fits the hardware was rejected: {e:?}"),
            ),
        }
        if exhaustive_small(&case) && !exhaustive_feasible(&case) {
            fail(&case, "oracle", "plant is not legal");
        }
    }
    if std::env::var("PLANNER_PROPS_SEED").is_err() && cases().count() >= 1000 {
        assert!(planted >= 300, "only {planted} planted cases");
        assert!(
            cut * 50 <= planted,
            "{cut} of {planted} cut by SearchLimits"
        );
    }
}

fn exhaustive_small(case: &Case) -> bool {
    case.layers.len() <= 3 && case.caps.planes.len() <= 4
}

#[test]
fn tight_limits_never_give_a_wrong_answer() {
    let style = Style {
        max_planes: 6,
        restrict: 25,
        roomy_zpos: false,
    };
    let mut cut = 0;
    for seed in cases() {
        let case = gen_case(seed ^ 0x55, style, 4);
        let mut rng = Rng::new(seed);
        let limits = SearchLimits {
            steps: rng.range(1, 40) as u32,
            backend_checks: rng.range(1, 3) as u32,
        };
        let r = plan_with(&case.frame(), &case.caps, &case, limits, |_| {
            Verdict::Accept
        });
        if r != plan_with(&case.frame(), &case.caps, &case, limits, |_| {
            Verdict::Accept
        }) {
            fail(&case, "limits", "not deterministic");
        }
        match r {
            Ok(p) => {
                if let Err(m) = check_plan(&case, &p) {
                    fail(&case, "limits", &format!("unsound plan: {m}"));
                }
                if case.layers.len() <= 3
                    && case.caps.planes.len() <= 4
                    && !exhaustive_feasible(&case)
                {
                    fail(&case, "limits", "plan for an infeasible case");
                }
            }
            // A cut search may say nothing, but nothing wrong.
            Err(PlanError::LimitExceeded) => cut += 1,
            Err(e) => {
                if let Err(m) = check_error(&case, &e) {
                    fail(&case, "limits", &format!("{m}\nerror: {e:?}"));
                }
                if case.layers.len() <= 3
                    && case.caps.planes.len() <= 4
                    && exhaustive_feasible(&case)
                {
                    fail(&case, "limits", &format!("feasible case reported as {e:?}"));
                }
            }
        }
    }
    if std::env::var("PLANNER_PROPS_SEED").is_err() && cases().count() >= 1000 {
        assert!(cut > 0, "tight limits never cut a search");
    }
}

/// A backend check that refuses candidates sees only sound, distinct plans,
/// and the planner never claims "no plane" when the hardware could do it.
#[test]
fn backend_refusals_see_sound_distinct_candidates() {
    let style = Style {
        max_planes: 4,
        restrict: 25,
        roomy_zpos: true,
    };
    for seed in cases() {
        let case = gen_case(seed ^ 0x77, style, 3);
        let mut rng = Rng::new(seed);
        let refuse = rng.range(0, 6) as u32;
        let mut seen: Vec<Plan> = Vec::new();
        let mut bad: Option<String> = None;
        let r = plan_with(
            &case.frame(),
            &case.caps,
            &case,
            SearchLimits::default(),
            |p| {
                if let Err(m) = check_plan(&case, p) {
                    bad.get_or_insert(m);
                }
                if seen.iter().any(|q| q.assignments() == p.assignments()) {
                    bad.get_or_insert("candidate offered twice".into());
                }
                seen.push(*p);
                if (seen.len() as u32) <= refuse {
                    Verdict::TryNext
                } else {
                    Verdict::Accept
                }
            },
        );
        if let Some(m) = bad {
            fail(&case, "backend candidates", &m);
        }
        let feasible = exhaustive_feasible(&case);
        match r {
            Ok(p) => {
                if !feasible {
                    fail(&case, "backend candidates", "plan for an infeasible case");
                }
                if p.assignments() != seen.last().unwrap().assignments() {
                    fail(
                        &case,
                        "backend candidates",
                        "result is not the accepted candidate",
                    );
                }
            }
            Err(PlanError::BackendRejected { candidates }) => {
                if candidates as usize != seen.len() || !feasible {
                    fail(
                        &case,
                        "backend candidates",
                        "BackendRejected is inconsistent",
                    );
                }
            }
            Err(PlanError::NoPlane { .. }) => {
                if feasible && seen.is_empty() {
                    fail(&case, "backend candidates", "NoPlane for a feasible case");
                }
            }
            Err(e) => fail(&case, "backend candidates", &format!("unexpected {e:?}")),
        }
    }
}

#[test]
fn generator_is_deterministic() {
    let style = Style {
        max_planes: 4,
        restrict: 30,
        roomy_zpos: false,
    };
    for seed in 0..50 {
        let (a, b) = (gen_case(seed, style, 3), gen_case(seed, style, 3));
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }
}
