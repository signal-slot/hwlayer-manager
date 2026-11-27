//! Plane assignment.
//!
//! Given a frame and the backend's capabilities, find a hardware plane for
//! every layer, or report which constraint stands in the way. There is no
//! fallback composition: a frame either maps completely onto hardware planes
//! or is rejected.
//!
//! A layer that reaches beyond the output is clipped first: the plan carries
//! the visible part of its destination and the matching part of its source,
//! and backends program those. A layer that is entirely off the output gets
//! no plane at all.
//!
//! The search is a bounded depth-first backtracking over layers from bottom
//! to top. It is exhaustive within its step limit, which is adequate for the
//! small plane counts of real display controllers. It finds *a* legal
//! assignment, deterministically, and does not rank assignments by cost.

use crate::caps::{Capabilities, LayerTraits, PlaneCaps, Zpos};
use crate::fixed::FixedVec;
use crate::limits::{MAX_LAYERS, MAX_PLANES, MAX_RESOURCE_GROUPS};
use crate::types::Rect;
use crate::types::{
    BlendMode, BufferId, BufferInfo, Frame, Layer, OutputId, PlaneId, Size, Transform, ALPHA_OPAQUE,
};

/// Looks up the metadata of registered buffers.
pub trait BufferLookup {
    /// Returns the metadata of `buffer`, or `None` if it is not registered.
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo>;
}

/// Reasons why planes could not take a layer, as a bit set.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RejectReasons(u32);

impl RejectReasons {
    /// No reason.
    pub const NONE: Self = Self(0);
    /// The plane cannot be attached to the requested output.
    pub const OUTPUT: Self = Self(1 << 0);
    /// The plane does not accept the buffer's format/modifier.
    pub const FORMAT: Self = Self(1 << 1);
    /// The plane cannot scale, or not by the requested factor.
    pub const SCALING: Self = Self(1 << 2);
    /// The source region is larger than the plane accepts.
    pub const SOURCE_SIZE: Self = Self(1 << 3);
    /// The plane has no constant plane alpha.
    pub const ALPHA: Self = Self(1 << 4);
    /// The plane does not support the requested blend mode.
    pub const BLEND: Self = Self(1 << 5);
    /// The plane does not support the requested transform.
    pub const TRANSFORM: Self = Self(1 << 6);
    /// The plane must cover the whole output and the layer does not.
    pub const FULL_OUTPUT: Self = Self(1 << 7);
    /// The plane cannot be stacked above the layers already placed.
    pub const Z_ORDER: Self = Self(1 << 8);
    /// A resource shared with other planes is exhausted.
    pub const RESOURCE: Self = Self(1 << 9);
    /// The plane is already taken by a lower layer.
    pub const IN_USE: Self = Self(1 << 10);

    const NAMES: [(Self, &'static str); 11] = [
        (Self::OUTPUT, "OUTPUT"),
        (Self::FORMAT, "FORMAT"),
        (Self::SCALING, "SCALING"),
        (Self::SOURCE_SIZE, "SOURCE_SIZE"),
        (Self::ALPHA, "ALPHA"),
        (Self::BLEND, "BLEND"),
        (Self::TRANSFORM, "TRANSFORM"),
        (Self::FULL_OUTPUT, "FULL_OUTPUT"),
        (Self::Z_ORDER, "Z_ORDER"),
        (Self::RESOURCE, "RESOURCE"),
        (Self::IN_USE, "IN_USE"),
    ];

    /// Returns `true` if no reason is set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns `true` if every reason in `other` is set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Union of two sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Raw bits.
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl core::ops::BitOr for RejectReasons {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl core::ops::BitOrAssign for RejectReasons {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

impl core::fmt::Debug for RejectReasons {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_empty() {
            return f.write_str("NONE");
        }
        let mut first = true;
        for (reason, name) in Self::NAMES {
            if self.contains(reason) {
                if !first {
                    f.write_str(" | ")?;
                }
                f.write_str(name)?;
                first = false;
            }
        }
        Ok(())
    }
}

/// Why a frame could not be planned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// The backend has no such output.
    UnknownOutput(OutputId),
    /// The frame has more layers than [`MAX_LAYERS`].
    TooManyLayers {
        /// Number of layers in the frame.
        count: usize,
    },
    /// A layer refers to a buffer that is not registered.
    UnknownBuffer {
        /// Index of the layer in the frame.
        layer: usize,
        /// The unknown buffer.
        buffer: BufferId,
    },
    /// A layer's source region is empty or outside its buffer.
    InvalidSource {
        /// Index of the layer in the frame.
        layer: usize,
    },
    /// A layer's destination is empty.
    InvalidDestination {
        /// Index of the layer in the frame.
        layer: usize,
    },
    /// A layer reaches beyond the output and cannot be clipped exactly: it
    /// is rotated, or scaled so that the visible part does not start and end
    /// on whole source pixels.
    CannotClip {
        /// Index of the layer in the frame.
        layer: usize,
    },
    /// No legal assignment exists.
    NoPlane {
        /// Index in the frame of the layer the search could not get past.
        layer: usize,
        /// Why the planes tried for that layer were unsuitable.
        reasons: RejectReasons,
    },
    /// Legal assignments exist, but the backend refused all that were tried.
    BackendRejected {
        /// Number of assignments the backend was asked about.
        candidates: u32,
    },
    /// The search limits were reached before a result was found.
    LimitExceeded,
    /// The backend check asked to stop, see [`Verdict::GiveUp`].
    Aborted,
}

/// A backend's answer to a proposed assignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The assignment works.
    Accept,
    /// This assignment does not work; another one might.
    TryNext,
    /// No assignment can work right now (device lost, buffer not imported).
    GiveUp,
}

/// Bounds on the planning search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchLimits {
    /// Maximum number of layer-to-plane attempts.
    pub steps: u32,
    /// Maximum number of assignments offered to the backend check.
    pub backend_checks: u32,
}

impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            steps: 20_000,
            backend_checks: 32,
        }
    }
}

/// One layer placed on one plane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Assignment {
    /// Index of the layer in the frame.
    pub layer: usize,
    /// Plane the layer is placed on.
    pub plane: PlaneId,
    /// Index of the plane in [`Capabilities::planes`].
    pub plane_index: usize,
    /// Stacking position to program. For a fixed-position plane this is the
    /// plane's own position.
    pub zpos: i32,
    /// Region of the buffer to show: the layer's source, reduced if the
    /// layer was clipped to the output.
    pub source: Rect,
    /// Where it appears: the layer's destination clipped to the output.
    pub destination: Rect,
}

/// A validated assignment of a frame's layers to hardware planes.
///
/// A plan is only meaningful for the frame and the capabilities it was made
/// from; [`crate::Engine`] enforces both when committing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    output: OutputId,
    assignments: FixedVec<Assignment, MAX_LAYERS>,
    fingerprint: u64,
    pub(crate) generation: u32,
}

impl Plan {
    /// A plan of nothing, which [`plan_into`] fills in.
    pub(crate) const EMPTY: Self = Self {
        output: OutputId(0),
        assignments: FixedVec::empty_filled(Assignment {
            layer: 0,
            plane: PlaneId(0),
            plane_index: 0,
            zpos: 0,
            source: Rect::new(0, 0, 0, 0),
            destination: Rect::new(0, 0, 0, 0),
        }),
        fingerprint: 0,
        generation: 0,
    };

    /// Output the plan is for.
    pub fn output(&self) -> OutputId {
        self.output
    }

    /// Assignments ordered from the bottom layer to the top layer.
    ///
    /// Layers that are entirely off the output have no assignment.
    pub fn assignments(&self) -> &[Assignment] {
        &self.assignments
    }

    /// Returns `true` if the plan was made from exactly this frame.
    pub fn matches(&self, frame: &Frame<'_>) -> bool {
        self.output == frame.output && self.fingerprint == fingerprint(frame)
    }
}

fn fingerprint(frame: &Frame<'_>) -> u64 {
    struct Fnv(u64);
    impl Fnv {
        fn add(&mut self, value: u64) {
            for byte in value.to_le_bytes() {
                self.0 = (self.0 ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        fn rect(&mut self, r: &crate::types::Rect) {
            self.add(r.x as u32 as u64 | (r.y as u32 as u64) << 32);
            self.add(r.width as u64 | (r.height as u64) << 32);
        }
    }
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    h.add(frame.output.0 as u64);
    h.add(frame.layers.len() as u64);
    for layer in frame.layers {
        h.add(layer.buffer.0 as u64);
        h.rect(&layer.source);
        h.rect(&layer.destination);
        h.add(layer.z_order as u32 as u64);
        h.add(layer.alpha as u64 | (layer.blend as u64) << 16 | (layer.transform as u64) << 24);
    }
    h.0
}

/// Plans `frame` with default limits and no backend check.
pub fn plan<L: BufferLookup>(
    frame: &Frame<'_>,
    caps: &Capabilities,
    buffers: &L,
) -> Result<Plan, PlanError> {
    plan_with(frame, caps, buffers, SearchLimits::default(), |_| {
        Verdict::Accept
    })
}

/// Plans `frame`, offering each legal assignment to `accept` until one is
/// accepted.
///
/// `accept` is where a backend runs its own final validation, such as a DRM
/// atomic `TEST_ONLY` commit. [`Verdict::TryNext`] makes the search continue
/// with the next legal assignment.
pub fn plan_with<L: BufferLookup>(
    frame: &Frame<'_>,
    caps: &Capabilities,
    buffers: &L,
    limits: SearchLimits,
    accept: impl FnMut(&Plan) -> Verdict,
) -> Result<Plan, PlanError> {
    let mut plan = Plan::EMPTY;
    plan_into(frame, caps, buffers, limits, accept, &mut plan)?;
    Ok(plan)
}

/// [`plan_with`], writing the plan into `plan`, so that a caller such as
/// [`crate::Engine::present`] keeps a single plan on its stack instead of
/// copies of the one being returned. `plan` is unspecified on error.
pub(crate) fn plan_into<L: BufferLookup>(
    frame: &Frame<'_>,
    caps: &Capabilities,
    buffers: &L,
    limits: SearchLimits,
    accept: impl FnMut(&Plan) -> Verdict,
    plan: &mut Plan,
) -> Result<(), PlanError> {
    let output_index = caps
        .output_index(frame.output)
        .ok_or(PlanError::UnknownOutput(frame.output))?;
    let output_size = caps.outputs[output_index].size;
    if frame.layers.len() > MAX_LAYERS {
        return Err(PlanError::TooManyLayers {
            count: frame.layers.len(),
        });
    }

    // The search's state is filled in place: built here and moved into it,
    // the layer tables would be on the stack twice, and MCU firmwares
    // present from small thread stacks.
    // The tables start from constants, copied straight into the search;
    // array expressions would be assembled on the stack first.
    const NOTHING: Rect = Rect::new(0, 0, 0, 0);
    const NO_LAYERS: [Layer; MAX_LAYERS] =
        [Layer::new(BufferId(0), NOTHING, NOTHING, 0); MAX_LAYERS];
    const NO_REASONS: [[RejectReasons; MAX_PLANES]; MAX_LAYERS] =
        [[RejectReasons::NONE; MAX_PLANES]; MAX_LAYERS];
    let mut search = Search {
        caps,
        order: [0usize; MAX_LAYERS],
        count: 0,
        visible: NO_LAYERS,
        compat: NO_REASONS,
        traits: [LayerTraits::ANY; MAX_LAYERS],
        used: 0,
        group_use: [0; MAX_RESOURCE_GROUPS],
        plan: {
            plan.output = frame.output;
            plan.assignments.clear();
            plan.fingerprint = fingerprint(frame);
            plan.generation = 0;
            plan
        },
        accept,
        limits,
        steps: 0,
        backend_checks: 0,
        deepest: 0,
        deepest_reasons: RejectReasons::NONE,
    };

    // Validate each layer on its own, clip it to the output and collect its
    // buffer metadata. `search.visible` holds the layers as the hardware will
    // see them, `search.order` the indices of those that are on the output at
    // all.
    let mut infos = [BufferInfo::default(); MAX_LAYERS];
    for (index, layer) in frame.layers.iter().enumerate() {
        let info = buffers
            .buffer_info(layer.buffer)
            .ok_or(PlanError::UnknownBuffer {
                layer: index,
                buffer: layer.buffer,
            })?;
        if layer.source.is_empty() || !layer.source.fits_within(info.size) {
            return Err(PlanError::InvalidSource { layer: index });
        }
        if layer.destination.is_empty() {
            return Err(PlanError::InvalidDestination { layer: index });
        }
        infos[index] = info;
        match clip_to_output(layer, output_size) {
            Ok(Some((source, destination))) => {
                search.visible[index] = Layer {
                    source,
                    destination,
                    ..*layer
                };
                search.order[search.count] = index;
                search.count += 1;
            }
            Ok(None) => {}
            Err(()) => return Err(PlanError::CannotClip { layer: index }),
        }
    }

    // Stacking order, bottom to top. Insertion sort keeps equal z stable.
    let count = search.count;
    let order = &mut search.order;
    for i in 1..count {
        let mut j = i;
        while j > 0 && frame.layers[order[j - 1]].z_order > frame.layers[order[j]].z_order {
            order.swap(j - 1, j);
            j -= 1;
        }
    }

    // Constraints that depend on one layer and one plane only.
    let mut attachable = 0usize;
    for (plane_index, plane) in caps.planes.iter().enumerate() {
        if plane.outputs & (1 << output_index) != 0 {
            attachable += 1;
        }
        for depth in 0..count {
            let layer_index = search.order[depth];
            search.compat[depth][plane_index] = static_reasons(
                &search.visible[layer_index],
                &infos[layer_index],
                plane,
                output_index,
                output_size,
            );
        }
    }
    for depth in 0..count {
        let layer_index = search.order[depth];
        search.traits[depth] = traits_of(&search.visible[layer_index], &infos[layer_index]);
    }

    // More layers than planes can never work; say so without searching.
    if count > attachable {
        return Err(PlanError::NoPlane {
            layer: search.order[attachable],
            reasons: if attachable == caps.planes.len() {
                RejectReasons::IN_USE
            } else {
                RejectReasons::IN_USE | RejectReasons::OUTPUT
            },
        });
    }

    match search.descend(0, None) {
        Outcome::Found => Ok(()),
        Outcome::LimitExceeded => Err(PlanError::LimitExceeded),
        Outcome::Aborted => Err(PlanError::Aborted),
        Outcome::Exhausted if search.backend_checks > 0 => Err(PlanError::BackendRejected {
            candidates: search.backend_checks,
        }),
        Outcome::Exhausted => Err(PlanError::NoPlane {
            layer: search.order[search.deepest],
            reasons: search.deepest_reasons,
        }),
    }
}

/// Clips a layer to the output.
///
/// Returns the source and destination of the visible part, `None` if nothing
/// is visible, or an error if the visible part has no exact source region.
fn clip_to_output(layer: &Layer, output: Size) -> Result<Option<(Rect, Rect)>, ()> {
    let destination = layer.destination;
    let left = destination.x as i64;
    let top = destination.y as i64;
    let right = left + destination.width as i64;
    let bottom = top + destination.height as i64;
    let clipped_left = left.max(0);
    let clipped_top = top.max(0);
    let clipped_right = right.min(output.width as i64);
    let clipped_bottom = bottom.min(output.height as i64);
    if clipped_left >= clipped_right || clipped_top >= clipped_bottom {
        return Ok(None);
    }
    if (clipped_left, clipped_top, clipped_right, clipped_bottom) == (left, top, right, bottom) {
        return Ok(Some((layer.source, destination)));
    }
    let (flip_x, flip_y) = match layer.transform {
        Transform::Normal => (false, false),
        Transform::FlipH => (true, false),
        Transform::FlipV => (false, true),
        // The source edge a destination edge maps to depends on the
        // rotation convention of the hardware; not clipped until a backend
        // supports rotation and pins that down.
        _ => return Err(()),
    };
    // Maps what was cut off one destination axis back to the source axis.
    //
    // The source length and the kept cut can each be near 2^32 and the cut
    // near 2^33, so their products exceed i64; they are computed in i128. A
    // result that does not fit a `Rect` cannot be programmed either, and is
    // refused like a clip that would need fractional source pixels.
    let axis = |start: i32, length: u32, shown: u32, cut_low: i64, cut_high: i64, flip: bool| {
        let length = length as i128;
        let shown = shown as i128;
        let near = (if flip { cut_high } else { cut_low }) as i128;
        let kept = shown - cut_low as i128 - cut_high as i128;
        if near < 0 || kept < 0 || near * length % shown != 0 || kept * length % shown != 0 {
            return Err(());
        }
        let offset = near * length / shown;
        let width = kept * length / shown;
        if start as i128 + offset > i32::MAX as i128 || width > u32::MAX as i128 {
            return Err(());
        }
        Ok((start + offset as i32, width as u32))
    };
    let (x, width) = axis(
        layer.source.x,
        layer.source.width,
        destination.width,
        clipped_left - left,
        right - clipped_right,
        flip_x,
    )?;
    let (y, height) = axis(
        layer.source.y,
        layer.source.height,
        destination.height,
        clipped_top - top,
        bottom - clipped_bottom,
        flip_y,
    )?;
    Ok(Some((
        Rect::new(x, y, width, height),
        Rect::new(
            clipped_left as i32,
            clipped_top as i32,
            (clipped_right - clipped_left) as u32,
            (clipped_bottom - clipped_top) as u32,
        ),
    )))
}

fn is_scaled(layer: &Layer) -> bool {
    let (width, height) = oriented_source(layer);
    width != layer.destination.width || height != layer.destination.height
}

fn traits_of(layer: &Layer, info: &BufferInfo) -> LayerTraits {
    let (width, height) = oriented_source(layer);
    let mut traits = LayerTraits::ANY;
    if width != layer.destination.width {
        traits = traits.union(LayerTraits::SCALED_H);
    }
    if height != layer.destination.height {
        traits = traits.union(LayerTraits::SCALED_V);
    }
    if info.format.is_known_yuv() {
        traits = traits.union(LayerTraits::YUV);
    }
    traits
}

fn oriented_source(layer: &Layer) -> (u32, u32) {
    if layer.transform.swaps_axes() {
        (layer.source.height, layer.source.width)
    } else {
        (layer.source.width, layer.source.height)
    }
}

fn axis_within(source: u32, destination: u32, max_upscale: u32, max_downscale: u32) -> bool {
    let source = source as u64;
    let destination = destination as u64;
    if destination > source {
        destination << 16 <= source * max_upscale as u64
    } else {
        source << 16 <= destination * max_downscale as u64
    }
}

pub(crate) fn static_reasons(
    layer: &Layer,
    info: &BufferInfo,
    plane: &PlaneCaps,
    output_index: usize,
    output_size: Size,
) -> RejectReasons {
    let mut reasons = RejectReasons::NONE;
    if plane.outputs & (1 << output_index) == 0 {
        reasons |= RejectReasons::OUTPUT;
    }
    if !plane
        .formats
        .iter()
        .any(|f| f.format == info.format && f.modifier == info.modifier)
    {
        reasons |= RejectReasons::FORMAT;
    }
    if is_scaled(layer) {
        let (width, height) = oriented_source(layer);
        let fits = plane.scaling.is_some_and(|s| {
            axis_within(
                width,
                layer.destination.width,
                s.max_upscale,
                s.max_downscale,
            ) && axis_within(
                height,
                layer.destination.height,
                s.max_upscale,
                s.max_downscale,
            )
        });
        if !fits {
            reasons |= RejectReasons::SCALING;
        }
    }
    if plane
        .max_source
        .is_some_and(|max| layer.source.width > max.width || layer.source.height > max.height)
    {
        reasons |= RejectReasons::SOURCE_SIZE;
    }
    if layer.alpha != ALPHA_OPAQUE && !plane.plane_alpha {
        reasons |= RejectReasons::ALPHA;
    }
    // Ignoring pixel alpha needs no hardware support when there is none.
    let blend_free = layer.blend == BlendMode::Opaque && info.format.is_known_opaque();
    if !blend_free && plane.blend_modes & layer.blend.mask() == 0 {
        reasons |= RejectReasons::BLEND;
    }
    if layer.transform != Transform::Normal && plane.transforms & layer.transform.mask() == 0 {
        reasons |= RejectReasons::TRANSFORM;
    }
    if plane.full_output_only && !layer.destination.covers(output_size) {
        reasons |= RejectReasons::FULL_OUTPUT;
    }
    reasons
}

enum Outcome {
    Found,
    Exhausted,
    LimitExceeded,
    Aborted,
}

struct Search<'a, F> {
    caps: &'a Capabilities,
    /// Layer indices from bottom to top; everything below is indexed by
    /// position in this order ("depth").
    order: [usize; MAX_LAYERS],
    count: usize,
    /// Layers after clipping, indexed like the frame's layers.
    visible: [Layer; MAX_LAYERS],
    compat: [[RejectReasons; MAX_PLANES]; MAX_LAYERS],
    traits: [LayerTraits; MAX_LAYERS],
    used: u32,
    group_use: [u8; MAX_RESOURCE_GROUPS],
    plan: &'a mut Plan,
    accept: F,
    limits: SearchLimits,
    steps: u32,
    backend_checks: u32,
    deepest: usize,
    deepest_reasons: RejectReasons,
}

impl<F: FnMut(&Plan) -> Verdict> Search<'_, F> {
    fn note(&mut self, depth: usize, reasons: RejectReasons) {
        if depth > self.deepest {
            self.deepest = depth;
            self.deepest_reasons = reasons;
        } else if depth == self.deepest {
            self.deepest_reasons |= reasons;
        }
    }

    /// Whether taking `plane_index` for a layer keeps every group in budget.
    fn groups_allow(&self, plane_index: usize, traits: LayerTraits) -> bool {
        self.caps
            .resource_groups
            .iter()
            .enumerate()
            .all(|(g, group)| {
                !Self::consumes(group.consumers, group.planes, plane_index, traits)
                    || self.group_use[g] < group.capacity
            })
    }

    fn consumes(
        consumers: LayerTraits,
        planes: u32,
        plane_index: usize,
        traits: LayerTraits,
    ) -> bool {
        planes & (1 << plane_index) != 0 && consumers.intersects(traits)
    }

    fn adjust_groups(&mut self, plane_index: usize, traits: LayerTraits, take: bool) {
        for (g, group) in self.caps.resource_groups.iter().enumerate() {
            if Self::consumes(group.consumers, group.planes, plane_index, traits) {
                if take {
                    self.group_use[g] += 1;
                } else {
                    self.group_use[g] -= 1;
                }
            }
        }
    }

    fn descend(&mut self, depth: usize, below: Option<i32>) -> Outcome {
        if depth == self.count {
            if self.backend_checks >= self.limits.backend_checks {
                return Outcome::LimitExceeded;
            }
            self.backend_checks += 1;
            return match (self.accept)(self.plan) {
                Verdict::Accept => Outcome::Found,
                Verdict::TryNext => Outcome::Exhausted,
                Verdict::GiveUp => Outcome::Aborted,
            };
        }
        for plane_index in 0..self.caps.planes.len() {
            if self.steps >= self.limits.steps {
                return Outcome::LimitExceeded;
            }
            self.steps += 1;

            let reasons = self.compat[depth][plane_index];
            if !reasons.is_empty() {
                self.note(depth, reasons);
                continue;
            }
            if self.used & (1 << plane_index) != 0 {
                self.note(depth, RejectReasons::IN_USE);
                continue;
            }
            // The lowest position above everything already placed is the
            // choice that leaves most room for the layers still to come.
            let zpos = match (self.caps.planes[plane_index].zpos, below) {
                (Zpos::Fixed(z), None) => Some(z),
                (Zpos::Fixed(z), Some(b)) => (z > b).then_some(z),
                (Zpos::Range { min, max }, None) => (min <= max).then_some(min),
                (Zpos::Range { min, max }, Some(b)) => {
                    let z = min.max(b.saturating_add(1));
                    (z > b && z <= max).then_some(z)
                }
            };
            let Some(zpos) = zpos else {
                self.note(depth, RejectReasons::Z_ORDER);
                continue;
            };
            let traits = self.traits[depth];
            if !self.groups_allow(plane_index, traits) {
                self.note(depth, RejectReasons::RESOURCE);
                continue;
            }

            self.used |= 1 << plane_index;
            self.adjust_groups(plane_index, traits, true);
            // Cannot fail: at most `count <= MAX_LAYERS` entries are pushed.
            let _ = self.plan.assignments.push(Assignment {
                layer: self.order[depth],
                plane: self.caps.planes[plane_index].id,
                plane_index,
                zpos,
                source: self.visible[self.order[depth]].source,
                destination: self.visible[self.order[depth]].destination,
            });

            match self.descend(depth + 1, Some(zpos)) {
                Outcome::Exhausted => {}
                done => return done,
            }

            self.plan.assignments.pop();
            self.adjust_groups(plane_index, traits, false);
            self.used &= !(1 << plane_index);
        }
        Outcome::Exhausted
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::caps::{FormatModifier, OutputCaps, PlaneKind, ResourceGroup, ScaleLimits};
    use crate::types::{PixelFormat, Rect};
    use std::vec::Vec;

    const OUT: OutputId = OutputId(0);
    const FULL: Rect = Rect::new(0, 0, 1920, 1080);

    struct Buffers(Vec<(BufferId, BufferInfo)>);

    impl BufferLookup for Buffers {
        fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
            self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
        }
    }

    fn buffer(id: u32, width: u32, height: u32, format: PixelFormat) -> (BufferId, BufferInfo) {
        (
            BufferId(id),
            BufferInfo {
                size: Size::new(width, height),
                format,
                modifier: crate::types::Modifier::LINEAR,
            },
        )
    }

    fn plane(id: u32, zpos: Zpos, formats: &[PixelFormat]) -> PlaneCaps {
        let mut caps = PlaneCaps {
            id: PlaneId(id),
            kind: PlaneKind::Overlay,
            outputs: 1,
            zpos,
            blend_modes: BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask(),
            ..PlaneCaps::default()
        };
        for format in formats {
            caps.formats.push(FormatModifier::linear(*format)).unwrap();
        }
        caps
    }

    fn caps(planes: &[PlaneCaps]) -> Capabilities {
        let mut caps = Capabilities::default();
        caps.outputs
            .push(OutputCaps {
                id: OUT,
                size: Size::new(1920, 1080),
                refresh_mhz: 60_000,
            })
            .unwrap();
        for plane in planes {
            caps.planes.push(*plane).unwrap();
        }
        caps
    }

    fn full_layer(buffer: u32, z_order: i32) -> Layer {
        Layer::new(BufferId(buffer), FULL, FULL, z_order)
    }

    fn planes_of(plan: &Plan) -> Vec<(usize, u32, i32)> {
        plan.assignments()
            .iter()
            .map(|a| (a.layer, a.plane.0, a.zpos))
            .collect()
    }

    const XRGB: PixelFormat = PixelFormat::XRGB8888;
    const ARGB: PixelFormat = PixelFormat::ARGB8888;
    const RGB565: PixelFormat = PixelFormat::RGB565;

    #[test]
    fn empty_frame_plans_to_nothing() {
        let caps = caps(&[plane(1, Zpos::Fixed(0), &[XRGB])]);
        let frame = Frame {
            output: OUT,
            layers: &[],
        };
        let plan = plan(&frame, &caps, &Buffers(Vec::new())).unwrap();
        assert!(plan.assignments().is_empty());
    }

    #[test]
    fn two_layers_follow_fixed_plane_order() {
        // Plane 1 is the upper one; the top layer must land there even though
        // the frame lists it first.
        let caps = caps(&[
            plane(1, Zpos::Fixed(1), &[XRGB, ARGB]),
            plane(2, Zpos::Fixed(0), &[XRGB, ARGB]),
        ]);
        let buffers = Buffers(std::vec![
            buffer(10, 1920, 1080, ARGB),
            buffer(11, 1920, 1080, XRGB)
        ]);
        let layers = [full_layer(10, 5), full_layer(11, 0)];
        let frame = Frame {
            output: OUT,
            layers: &layers,
        };
        let plan = plan(&frame, &caps, &buffers).unwrap();
        assert_eq!(planes_of(&plan), [(1, 2, 0), (0, 1, 1)]);
        assert!(plan.matches(&frame));
    }

    #[test]
    fn backtracks_when_first_choice_starves_a_later_layer() {
        // The bottom layer fits both planes, the top layer only plane 1.
        let caps = caps(&[
            plane(1, Zpos::Range { min: 0, max: 3 }, &[XRGB, RGB565]),
            plane(2, Zpos::Range { min: 0, max: 3 }, &[XRGB]),
        ]);
        let buffers = Buffers(std::vec![
            buffer(10, 1920, 1080, XRGB),
            buffer(11, 1920, 1080, RGB565)
        ]);
        let layers = [full_layer(10, 0), full_layer(11, 1)];
        let frame = Frame {
            output: OUT,
            layers: &layers,
        };
        let plan = plan(&frame, &caps, &buffers).unwrap();
        assert_eq!(planes_of(&plan), [(0, 2, 0), (1, 1, 1)]);
    }

    #[test]
    fn equal_z_order_stacks_in_frame_order() {
        let caps = caps(&[
            plane(1, Zpos::Fixed(0), &[XRGB]),
            plane(2, Zpos::Fixed(1), &[XRGB]),
        ]);
        let buffers = Buffers(std::vec![
            buffer(10, 1920, 1080, XRGB),
            buffer(11, 1920, 1080, XRGB)
        ]);
        let layers = [full_layer(10, 7), full_layer(11, 7)];
        let frame = Frame {
            output: OUT,
            layers: &layers,
        };
        let plan = plan(&frame, &caps, &buffers).unwrap();
        assert_eq!(planes_of(&plan), [(0, 1, 0), (1, 2, 1)]);
    }

    #[test]
    fn format_mismatch_is_reported() {
        let caps = caps(&[plane(1, Zpos::Fixed(0), &[XRGB])]);
        let buffers = Buffers(std::vec![buffer(10, 1920, 1080, PixelFormat::NV12)]);
        let layers = [full_layer(10, 0)];
        let frame = Frame {
            output: OUT,
            layers: &layers,
        };
        assert_eq!(
            plan(&frame, &caps, &buffers),
            Err(PlanError::NoPlane {
                layer: 0,
                reasons: RejectReasons::FORMAT
            })
        );
    }

    #[test]
    fn fixed_planes_in_wrong_order_report_z_order() {
        // Only plane 2 takes RGB565 and it sits below plane 1, but the RGB565
        // layer is requested on top.
        let caps = caps(&[
            plane(1, Zpos::Fixed(1), &[ARGB]),
            plane(2, Zpos::Fixed(0), &[RGB565]),
        ]);
        let buffers = Buffers(std::vec![
            buffer(10, 1920, 1080, ARGB),
            buffer(11, 1920, 1080, RGB565)
        ]);
        let layers = [full_layer(10, 0), full_layer(11, 1)];
        let frame = Frame {
            output: OUT,
            layers: &layers,
        };
        assert_eq!(
            plan(&frame, &caps, &buffers),
            Err(PlanError::NoPlane {
                layer: 1,
                reasons: RejectReasons::FORMAT | RejectReasons::Z_ORDER
            })
        );
    }

    #[test]
    fn more_layers_than_planes_is_rejected() {
        let caps = caps(&[plane(1, Zpos::Fixed(0), &[XRGB])]);
        let buffers = Buffers(std::vec![buffer(10, 1920, 1080, XRGB)]);
        let layers = [full_layer(10, 0), full_layer(10, 1)];
        let frame = Frame {
            output: OUT,
            layers: &layers,
        };
        assert_eq!(
            plan(&frame, &caps, &buffers),
            Err(PlanError::NoPlane {
                layer: 1,
                reasons: RejectReasons::IN_USE
            })
        );
    }

    #[test]
    fn scaling_limits_are_enforced() {
        let mut scaler = plane(1, Zpos::Fixed(0), &[XRGB]);
        scaler.scaling = Some(ScaleLimits {
            max_upscale: 2 << 16,
            max_downscale: 4 << 16,
        });
        let caps = caps(&[scaler]);
        let buffers = Buffers(std::vec![buffer(10, 960, 540, XRGB)]);
        let small = Rect::new(0, 0, 960, 540);

        let ok = [Layer::new(BufferId(10), small, FULL, 0)];
        assert!(plan(
            &Frame {
                output: OUT,
                layers: &ok
            },
            &caps,
            &buffers
        )
        .is_ok());

        // 480 -> 1920 is a factor of 4, beyond the limit of 2.
        let too_much = [Layer::new(BufferId(10), Rect::new(0, 0, 480, 270), FULL, 0)];
        assert_eq!(
            plan(
                &Frame {
                    output: OUT,
                    layers: &too_much
                },
                &caps,
                &buffers
            ),
            Err(PlanError::NoPlane {
                layer: 0,
                reasons: RejectReasons::SCALING
            })
        );
    }

    #[test]
    fn rotation_swaps_axes_before_the_scaling_check() {
        let mut p = plane(1, Zpos::Fixed(0), &[XRGB]);
        p.transforms = Transform::Rotate90.mask();
        let caps = caps(&[p]);
        let buffers = Buffers(std::vec![buffer(10, 1080, 1920, XRGB)]);
        let mut layer = Layer::new(BufferId(10), Rect::new(0, 0, 1080, 1920), FULL, 0);
        layer.transform = Transform::Rotate90;
        let layers = [layer];
        let frame = Frame {
            output: OUT,
            layers: &layers,
        };
        // Unscaled once rotated, so a plane without a scaler is enough.
        assert!(plan(&frame, &caps, &buffers).is_ok());
    }

    #[test]
    fn shared_scaler_limits_scaled_layers() {
        let limits = Some(ScaleLimits {
            max_upscale: 8 << 16,
            max_downscale: 8 << 16,
        });
        let mut a = plane(1, Zpos::Fixed(0), &[XRGB]);
        let mut b = plane(2, Zpos::Fixed(1), &[XRGB]);
        a.scaling = limits;
        b.scaling = limits;
        let mut caps = caps(&[a, b]);
        caps.resource_groups
            .push(ResourceGroup {
                consumers: LayerTraits::SCALED,
                planes: 0b11,
                capacity: 1,
            })
            .unwrap();
        let buffers = Buffers(std::vec![
            buffer(10, 960, 540, XRGB),
            buffer(11, 1920, 1080, XRGB)
        ]);
        let small = Rect::new(0, 0, 960, 540);

        let one_scaled = [Layer::new(BufferId(10), small, FULL, 0), full_layer(11, 1)];
        assert!(plan(
            &Frame {
                output: OUT,
                layers: &one_scaled
            },
            &caps,
            &buffers
        )
        .is_ok());

        let both_scaled = [
            Layer::new(BufferId(10), small, FULL, 0),
            Layer::new(BufferId(10), small, FULL, 1),
        ];
        assert_eq!(
            plan(
                &Frame {
                    output: OUT,
                    layers: &both_scaled
                },
                &caps,
                &buffers
            ),
            Err(PlanError::NoPlane {
                layer: 1,
                reasons: RejectReasons::IN_USE | RejectReasons::RESOURCE | RejectReasons::Z_ORDER
            })
        );
    }

    #[test]
    fn full_output_only_plane_rejects_a_window() {
        let mut primary = plane(1, Zpos::Fixed(0), &[XRGB]);
        primary.full_output_only = true;
        let caps = caps(&[primary]);
        let buffers = Buffers(std::vec![buffer(10, 640, 480, XRGB)]);
        let window = Rect::new(0, 0, 640, 480);
        let layers = [Layer::new(BufferId(10), window, window, 0)];
        assert_eq!(
            plan(
                &Frame {
                    output: OUT,
                    layers: &layers
                },
                &caps,
                &buffers
            ),
            Err(PlanError::NoPlane {
                layer: 0,
                reasons: RejectReasons::FULL_OUTPUT
            })
        );
    }

    #[test]
    fn alpha_blend_and_transform_need_plane_support() {
        let mut bare = plane(1, Zpos::Fixed(0), &[ARGB]);
        bare.blend_modes = 0;
        let caps = caps(&[bare]);
        let buffers = Buffers(std::vec![buffer(10, 1920, 1080, ARGB)]);
        let mut layer = full_layer(10, 0);
        layer.alpha = 0x8000;
        layer.blend = BlendMode::Coverage;
        layer.transform = Transform::Rotate180;
        let layers = [layer];
        assert_eq!(
            plan(
                &Frame {
                    output: OUT,
                    layers: &layers
                },
                &caps,
                &buffers
            ),
            Err(PlanError::NoPlane {
                layer: 0,
                reasons: RejectReasons::ALPHA | RejectReasons::BLEND | RejectReasons::TRANSFORM
            })
        );
    }

    #[test]
    fn invalid_geometry_and_unknown_buffers_are_rejected() {
        let caps = caps(&[plane(1, Zpos::Fixed(0), &[XRGB])]);
        let buffers = Buffers(std::vec![buffer(10, 640, 480, XRGB)]);
        let window = Rect::new(0, 0, 640, 480);
        let check = |layer: Layer| {
            plan(
                &Frame {
                    output: OUT,
                    layers: &[layer],
                },
                &caps,
                &buffers,
            )
        };
        assert_eq!(
            check(Layer::new(BufferId(99), window, window, 0)),
            Err(PlanError::UnknownBuffer {
                layer: 0,
                buffer: BufferId(99)
            })
        );
        assert_eq!(
            check(Layer::new(
                BufferId(10),
                Rect::new(1, 0, 640, 480),
                window,
                0
            )),
            Err(PlanError::InvalidSource { layer: 0 })
        );
        assert_eq!(
            check(Layer::new(BufferId(10), window, Rect::new(0, 0, 0, 480), 0)),
            Err(PlanError::InvalidDestination { layer: 0 })
        );
        assert_eq!(
            plan(
                &Frame {
                    output: OutputId(9),
                    layers: &[]
                },
                &caps,
                &buffers
            ),
            Err(PlanError::UnknownOutput(OutputId(9)))
        );
    }

    #[test]
    fn backend_rejection_moves_on_to_the_next_assignment() {
        let caps = caps(&[
            plane(1, Zpos::Range { min: 0, max: 1 }, &[XRGB]),
            plane(2, Zpos::Range { min: 0, max: 1 }, &[XRGB]),
        ]);
        let buffers = Buffers(std::vec![buffer(10, 1920, 1080, XRGB)]);
        let layers = [full_layer(10, 0)];
        let frame = Frame {
            output: OUT,
            layers: &layers,
        };

        let mut seen = Vec::new();
        let plan = plan_with(&frame, &caps, &buffers, SearchLimits::default(), |p| {
            seen.push(p.assignments()[0].plane.0);
            if p.assignments()[0].plane == PlaneId(2) {
                Verdict::Accept
            } else {
                Verdict::TryNext
            }
        })
        .unwrap();
        assert_eq!(seen, [1, 2]);
        assert_eq!(plan.assignments()[0].plane, PlaneId(2));

        assert_eq!(
            plan_with(&frame, &caps, &buffers, SearchLimits::default(), |_| {
                Verdict::TryNext
            }),
            Err(PlanError::BackendRejected { candidates: 2 })
        );
        let tight = SearchLimits {
            backend_checks: 1,
            ..SearchLimits::default()
        };
        assert_eq!(
            plan_with(&frame, &caps, &buffers, tight, |_| Verdict::TryNext),
            Err(PlanError::LimitExceeded)
        );
        // Giving up stops at the first assignment.
        let mut asked = 0;
        assert_eq!(
            plan_with(&frame, &caps, &buffers, SearchLimits::default(), |_| {
                asked += 1;
                Verdict::GiveUp
            }),
            Err(PlanError::Aborted)
        );
        assert_eq!(asked, 1);
    }

    #[test]
    fn plan_does_not_match_a_different_frame() {
        let caps = caps(&[plane(1, Zpos::Fixed(0), &[XRGB])]);
        let buffers = Buffers(std::vec![buffer(10, 1920, 1080, XRGB)]);
        let layers = [full_layer(10, 0)];
        let frame = Frame {
            output: OUT,
            layers: &layers,
        };
        let plan = plan(&frame, &caps, &buffers).unwrap();
        let mut moved = layers;
        moved[0].alpha = 0x1234;
        assert!(!plan.matches(&Frame {
            output: OUT,
            layers: &moved
        }));
    }

    fn clip_caps() -> Capabilities {
        let mut scaler = plane(1, Zpos::Fixed(0), &[XRGB]);
        scaler.scaling = Some(ScaleLimits {
            max_upscale: 8 << 16,
            max_downscale: 8 << 16,
        });
        scaler.transforms = Transform::FlipH.mask() | Transform::Rotate90.mask();
        caps(&[scaler, plane(2, Zpos::Fixed(1), &[XRGB])])
    }

    fn clipped(layer: Layer) -> Result<Vec<(Rect, Rect)>, PlanError> {
        let buffers = Buffers(std::vec![buffer(10, 400, 300, XRGB)]);
        plan(
            &Frame {
                output: OUT,
                layers: &[layer],
            },
            &clip_caps(),
            &buffers,
        )
        .map(|p| {
            p.assignments()
                .iter()
                .map(|a| (a.source, a.destination))
                .collect()
        })
    }

    #[test]
    fn layer_reaching_past_the_edges_is_clipped() {
        let source = Rect::new(0, 0, 400, 300);
        let at = |x, y| Layer::new(BufferId(10), source, Rect::new(x, y, 400, 300), 0);
        // 100 pixels off the left, 50 off the top.
        assert_eq!(
            clipped(at(-100, -50)),
            Ok(std::vec![(
                Rect::new(100, 50, 300, 250),
                Rect::new(0, 0, 300, 250)
            )])
        );
        // Off the right and bottom: the source keeps its origin.
        assert_eq!(
            clipped(at(1720, 980)),
            Ok(std::vec![(
                Rect::new(0, 0, 200, 100),
                Rect::new(1720, 980, 200, 100)
            )])
        );
        // On the output: untouched.
        assert_eq!(
            clipped(at(10, 10)),
            Ok(std::vec![(source, Rect::new(10, 10, 400, 300))])
        );
    }

    #[test]
    fn clipping_a_scaled_layer_needs_whole_source_pixels() {
        let source = Rect::new(0, 0, 400, 300);
        let doubled = |x| Layer::new(BufferId(10), source, Rect::new(x, 0, 800, 600), 0);
        // Cutting 100 destination pixels removes 50 source pixels.
        assert_eq!(
            clipped(doubled(-100)),
            Ok(std::vec![(
                Rect::new(50, 0, 350, 300),
                Rect::new(0, 0, 700, 600)
            )])
        );
        // Cutting 101 would need half a source pixel.
        assert_eq!(
            clipped(doubled(-101)),
            Err(PlanError::CannotClip { layer: 0 })
        );
    }

    #[test]
    fn clipping_follows_a_horizontal_flip_but_not_a_rotation() {
        let source = Rect::new(0, 0, 400, 300);
        let mut flipped = Layer::new(BufferId(10), source, Rect::new(-100, 0, 400, 300), 0);
        flipped.transform = Transform::FlipH;
        // The left of the screen shows the right of the source, so the
        // source loses its right-hand 100 pixels.
        assert_eq!(
            clipped(flipped),
            Ok(std::vec![(
                Rect::new(0, 0, 300, 300),
                Rect::new(0, 0, 300, 300)
            )])
        );
        let mut rotated = flipped;
        rotated.transform = Transform::Rotate90;
        rotated.destination = Rect::new(-100, 0, 300, 400);
        assert_eq!(clipped(rotated), Err(PlanError::CannotClip { layer: 0 }));
    }

    #[test]
    fn layer_entirely_off_the_output_takes_no_plane() {
        let buffers = Buffers(std::vec![buffer(10, 400, 300, XRGB)]);
        let source = Rect::new(0, 0, 400, 300);
        // Three layers for two planes, but one of them is not visible.
        let layers = [
            Layer::new(BufferId(10), source, Rect::new(0, 0, 400, 300), 0),
            Layer::new(BufferId(10), source, Rect::new(1920, 0, 400, 300), 1),
            Layer::new(BufferId(10), source, Rect::new(500, 0, 400, 300), 2),
        ];
        let plan = plan(
            &Frame {
                output: OUT,
                layers: &layers,
            },
            &clip_caps(),
            &buffers,
        )
        .unwrap();
        let placed: Vec<usize> = plan.assignments().iter().map(|a| a.layer).collect();
        assert_eq!(placed, [0, 2]);
    }

    fn huge_buffer(width: u32, height: u32) -> Buffers {
        Buffers(std::vec![(
            BufferId(10),
            BufferInfo {
                size: Size::new(width, height),
                format: XRGB,
                modifier: crate::types::Modifier::LINEAR,
            },
        )])
    }

    #[test]
    fn clipping_a_huge_flipped_destination_is_refused_not_overflowed() {
        // A destination and source of more than 4 billion pixels, cut while
        // flipped: the source pixels behind the cut are a product of values
        // up to 2^33 and 2^32. That product exceeds i64, so it must be
        // rejected, not overflowed (debug builds caught this as a panic) or
        // wrapped (release builds produced a garbage source region).
        let buffers = huge_buffer(4_000_000_000, 300);
        let caps = caps(&[plane(1, Zpos::Fixed(0), &[XRGB])]);
        let mut layer = Layer::new(
            BufferId(10),
            Rect::new(0, 0, 4_000_000_000, 300),
            Rect::new(0, 0, 4_200_000_000, 300),
            0,
        );
        layer.transform = Transform::FlipH;
        assert_eq!(
            plan(
                &Frame {
                    output: OUT,
                    layers: &[layer],
                },
                &caps,
                &buffers
            ),
            Err(PlanError::CannotClip { layer: 0 })
        );
    }

    #[test]
    fn clipping_a_gigantic_source_stays_inside_the_source() {
        // The destination is shifted left out of the output by 2^31 pixels
        // and is as wide as the source, so the visible part starts at whole
        // source pixels. The offset of 2^31 does not fit an i32; clipping
        // must refuse instead of wrapping the source coordinate negative.
        let buffers = huge_buffer(4_200_000_000, 100);
        let caps = caps(&[plane(1, Zpos::Fixed(0), &[XRGB])]);
        let layer = Layer::new(
            BufferId(10),
            Rect::new(0, 0, 4_200_000_000, 100),
            Rect::new(-2_147_483_648, 0, 4_200_000_000, 100),
            0,
        );
        assert_eq!(
            plan(
                &Frame {
                    output: OUT,
                    layers: &[layer],
                },
                &caps,
                &buffers
            ),
            Err(PlanError::CannotClip { layer: 0 })
        );
    }

    #[test]
    fn plane_with_inverted_zpos_range_is_never_chosen() {
        // A capability description claiming a plane stacks in `5..=2`
        // describes no position; the plane must not be used at all instead
        // of getting the position 5.
        let caps = caps(&[plane(1, Zpos::Range { min: 5, max: 2 }, &[XRGB])]);
        let buffers = Buffers(std::vec![buffer(10, 1920, 1080, XRGB)]);
        let layers = [full_layer(10, 0)];
        assert_eq!(
            plan(
                &Frame {
                    output: OUT,
                    layers: &layers,
                },
                &caps,
                &buffers
            ),
            Err(PlanError::NoPlane {
                layer: 0,
                reasons: RejectReasons::Z_ORDER
            })
        );
    }

    /// Tiny deterministic generator; no external crates in the core.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self, bound: u32) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as u32) % bound
        }
    }

    /// Reference answer: try every injective layer-to-plane mapping.
    fn brute_force_feasible(
        frame: &Frame<'_>,
        caps: &Capabilities,
        buffers: &Buffers,
        order: &[usize],
        depth: usize,
        used: u32,
        chosen: &mut Vec<usize>,
    ) -> bool {
        if depth == order.len() {
            // Stacking: positions must be assignable strictly increasing.
            let mut below: Option<i32> = None;
            for &p in chosen.iter() {
                let z = match (caps.planes[p].zpos, below) {
                    (Zpos::Fixed(z), None) => z,
                    (Zpos::Fixed(z), Some(b)) if z > b => z,
                    (Zpos::Range { min, .. }, None) => min,
                    (Zpos::Range { min, max }, Some(b)) if min.max(b + 1) <= max => min.max(b + 1),
                    _ => return false,
                };
                below = Some(z);
            }
            // Shared resources.
            for group in caps.resource_groups.iter() {
                let mut users = 0;
                for (d, &p) in chosen.iter().enumerate() {
                    let layer = &frame.layers[order[d]];
                    let info = buffers.buffer_info(layer.buffer).unwrap();
                    if group.planes & (1 << p) != 0
                        && group.consumers.intersects(traits_of(layer, &info))
                    {
                        users += 1;
                    }
                }
                if users > group.capacity {
                    return false;
                }
            }
            return true;
        }
        let layer = &frame.layers[order[depth]];
        let info = buffers.buffer_info(layer.buffer).unwrap();
        for p in 0..caps.planes.len() {
            if used & (1 << p) != 0 {
                continue;
            }
            let reasons = static_reasons(layer, &info, &caps.planes[p], 0, Size::new(1920, 1080));
            if !reasons.is_empty() {
                continue;
            }
            chosen.push(p);
            let ok = brute_force_feasible(
                frame,
                caps,
                buffers,
                order,
                depth + 1,
                used | (1 << p),
                chosen,
            );
            chosen.pop();
            if ok {
                return true;
            }
        }
        false
    }

    #[test]
    fn planner_agrees_with_brute_force_on_random_cases() {
        let formats = [XRGB, ARGB, RGB565, PixelFormat::NV12];
        let mut rng = Lcg(0x5eed);
        let mut feasible = 0;
        for _ in 0..3000 {
            let plane_count = 1 + rng.next(4) as usize;
            let mut planes = Vec::new();
            for i in 0..plane_count {
                let zpos = if rng.next(2) == 0 {
                    Zpos::Fixed(rng.next(4) as i32)
                } else {
                    let min = rng.next(3) as i32;
                    Zpos::Range {
                        min,
                        max: min + rng.next(3) as i32,
                    }
                };
                let mut accepted = Vec::new();
                for format in formats {
                    if rng.next(3) != 0 {
                        accepted.push(format);
                    }
                }
                let mut p = plane(i as u32 + 1, zpos, &accepted);
                if rng.next(2) == 0 {
                    p.scaling = Some(ScaleLimits {
                        max_upscale: 4 << 16,
                        max_downscale: 4 << 16,
                    });
                }
                planes.push(p);
            }
            let mut caps = caps(&planes);
            if rng.next(2) == 0 {
                caps.resource_groups
                    .push(ResourceGroup {
                        consumers: [
                            LayerTraits::ANY,
                            LayerTraits::SCALED,
                            LayerTraits::SCALED_H,
                            LayerTraits::YUV,
                            LayerTraits::SCALED.union(LayerTraits::YUV),
                        ][rng.next(5) as usize],
                        planes: rng.next(1 << plane_count),
                        capacity: rng.next(3) as u8,
                    })
                    .unwrap();
            }

            let buffers = Buffers(std::vec![
                buffer(0, 1920, 1080, XRGB),
                buffer(1, 1920, 1080, ARGB),
                buffer(2, 1920, 1080, RGB565),
                buffer(3, 1920, 1080, PixelFormat::NV12),
            ]);
            let layer_count = rng.next(5) as usize;
            let mut layers = Vec::new();
            for _ in 0..layer_count {
                let source = match rng.next(6) {
                    0 => Rect::new(0, 0, 960, 540),
                    1 => Rect::new(0, 0, 960, 1080),
                    _ => FULL,
                };
                layers.push(Layer::new(
                    BufferId(rng.next(4)),
                    source,
                    FULL,
                    rng.next(3) as i32,
                ));
            }
            let frame = Frame {
                output: OUT,
                layers: &layers,
            };

            let mut order: Vec<usize> = (0..layer_count).collect();
            order.sort_by_key(|&i| layers[i].z_order);
            let expected =
                brute_force_feasible(&frame, &caps, &buffers, &order, 0, 0, &mut Vec::new());
            let result = plan(&frame, &caps, &buffers);
            assert_eq!(
                result.is_ok(),
                expected,
                "caps {caps:?}\nlayers {layers:?}\nresult {result:?}"
            );

            if let Ok(plan) = result {
                feasible += 1;
                // The plan itself must be legal, not just exist.
                let mut seen = 0u32;
                let mut below = None;
                for (depth, a) in plan.assignments().iter().enumerate() {
                    assert_eq!(a.layer, order[depth]);
                    assert_eq!(seen & (1 << a.plane_index), 0);
                    seen |= 1 << a.plane_index;
                    let info = buffers.buffer_info(layers[a.layer].buffer).unwrap();
                    assert!(static_reasons(
                        &layers[a.layer],
                        &info,
                        &caps.planes[a.plane_index],
                        0,
                        Size::new(1920, 1080)
                    )
                    .is_empty());
                    match caps.planes[a.plane_index].zpos {
                        Zpos::Fixed(z) => assert_eq!(a.zpos, z),
                        Zpos::Range { min, max } => assert!(a.zpos >= min && a.zpos <= max),
                    }
                    assert!(below.is_none_or(|b| a.zpos > b));
                    below = Some(a.zpos);
                }
            }
        }
        // Guard against a generator that only produces one kind of case.
        assert!(feasible > 300 && feasible < 2700, "feasible = {feasible}");
    }
}
