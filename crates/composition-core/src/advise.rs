//! Advisory planning: which layers can go on planes, which the caller has to
//! draw into a composition buffer itself, and why.
//!
//! [`advise`] answers the question a compositor asks before it renders
//! (`docs/advisory-planner-design.md`, WP5-03). It never commits, keeps no
//! state between calls and returns no [`Plan`]: to present, the caller
//! renders the composed layers, builds the real frame and plans it again.
//!
//! The model of composition is one contiguous run of layers in stacking
//! order: the caller draws the run into one buffer of its
//! [`ComposeTarget`], which then stands in for the run as one full-output
//! layer. The search tries the frame as it is first, then every contiguous
//! run in order of increasing length, lower runs first, and stops at the
//! first that the planner (and the caller's check) accepts. So the advice is
//! deterministic and composes as few layers as the model allows; it does not
//! rank by cost, and a promoted layer between two composed ones cannot be
//! expressed.

use crate::caps::Capabilities;
use crate::fixed::FixedVec;
use crate::limits::{MAX_LAYERS, MAX_PLANES};
use crate::planner::{
    plan_into, static_reasons, Assignment, BufferLookup, Plan, PlanError, RejectReasons,
    SearchLimits, Verdict,
};
use crate::types::{
    BlendMode, BufferId, BufferInfo, Frame, Layer, Modifier, OutputId, PixelFormat, Rect, Size,
    Transform, ALPHA_OPAQUE,
};

/// Most candidates [`advise`] can try: the frame as it is plus every
/// contiguous run of up to [`MAX_LAYERS`] layers.
pub const MAX_CANDIDATES: u32 = 1 + (MAX_LAYERS * (MAX_LAYERS + 1) / 2) as u32;

/// `layer` of the composition target's assignment in [`Advice::compose`].
pub const COMPOSE_LAYER: usize = usize::MAX;

/// The buffer the caller renders the composed layers into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComposeTarget {
    /// Pixel format of the composition buffer.
    pub format: PixelFormat,
    /// Layout modifier of the composition buffer.
    pub modifier: Modifier,
    /// Blend of the composed result against what is below it.
    pub blend: BlendMode,
}

/// Where one input layer ends up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    /// On a plane; the assignment is as in a [`Plan`].
    Plane(Assignment),
    /// Not on a plane: drawn by the caller into the composition buffer.
    Compose,
    /// Entirely off the output: neither on a plane nor composed.
    #[default]
    Hidden,
}

/// Why a layer is not on a plane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Blocker {
    /// The layer is on a plane, or hidden.
    #[default]
    None,
    /// No plane can take this layer on its own; [`LayerAdvice::reasons`] is
    /// what holds on every plane that reaches the output.
    Static,
    /// Some plane can take it alone but not together with the others, or
    /// it lies inside a run that had to be composed for other layers.
    Combination,
    /// The caller's check refused an otherwise legal assignment that held
    /// the layer on a plane, and no other was accepted.
    Backend,
    /// The layer reaches beyond the output and cannot be clipped exactly
    /// ([`PlanError::CannotClip`]).
    Clip,
    /// The limits cut the search of a candidate tried before the accepted
    /// one, so composing this layer may not be necessary.
    Budget,
}

/// The advice for one input layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayerAdvice {
    /// Where the layer ends up.
    pub placement: Placement,
    /// Why it is not on a plane; [`Blocker::None`] if it is or is hidden.
    pub blocker: Blocker,
    /// For [`Blocker::Static`], the reasons common to every plane that
    /// reaches the output ([`RejectReasons::OUTPUT`] if none does); empty
    /// otherwise.
    pub reasons: RejectReasons,
}

/// The advisory result. Not a [`Plan`] and not accepted for a commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Advice {
    output: OutputId,
    layers: FixedVec<LayerAdvice, MAX_LAYERS>,
    compose: Option<Assignment>,
    compose_run: Option<(u8, u8)>,
    candidates: u32,
    backend_checks: u32,
}

impl Advice {
    /// Output the advice is for.
    pub fn output(&self) -> OutputId {
        self.output
    }

    /// One entry per input layer, in frame order.
    pub fn layers(&self) -> &[LayerAdvice] {
        &self.layers
    }

    /// Assignment of the composition target, if any layer is composed with
    /// a [`ComposeTarget`]. Its `layer` is [`COMPOSE_LAYER`]; source and
    /// destination are the whole output.
    pub fn compose(&self) -> Option<&Assignment> {
        self.compose.as_ref()
    }

    /// The composed run as `(low, high)` positions, inclusive, in the
    /// stacking order of the layers that are on the output (bottom first,
    /// equal `z_order` in frame order; hidden layers have no position).
    pub fn compose_run(&self) -> Option<(usize, usize)> {
        self.compose_run
            .map(|(low, high)| (low as usize, high as usize))
    }

    /// Number of layers placed on planes, the composition target excluded.
    pub fn promoted(&self) -> usize {
        self.layers
            .iter()
            .filter(|l| matches!(l.placement, Placement::Plane(_)))
            .count()
    }

    /// `true` if every layer is on a plane or hidden and nothing needs
    /// composing.
    pub fn is_direct(&self) -> bool {
        self.compose_run.is_none()
            && self
                .layers
                .iter()
                .all(|l| !matches!(l.placement, Placement::Compose))
    }

    /// Number of candidates planned, the frame as it is included.
    pub fn candidates(&self) -> u32 {
        self.candidates
    }

    /// Number of times the caller's check was called.
    pub fn backend_checks(&self) -> u32 {
        self.backend_checks
    }
}

/// Errors of the whole call: caller input or a configuration problem, not a
/// per-layer outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdviceError {
    /// The capabilities have no such output.
    UnknownOutput(OutputId),
    /// The frame has more layers than [`MAX_LAYERS`].
    TooManyLayers {
        /// Number of layers in the frame.
        count: usize,
    },
    /// A layer refers to a buffer the lookup does not know.
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
    /// Every candidate failed, even the composition target alone: no plane
    /// takes the target on this output. `reasons` are the planner's for the
    /// target alone, empty if the caller's check refused it.
    NoComposePlane {
        /// Why the planes were unsuitable for the composition target.
        reasons: RejectReasons,
    },
    /// The limits ended the search before any candidate was accepted.
    LimitExceeded,
    /// The caller's check gave up ([`Verdict::GiveUp`]).
    Aborted,
}

/// Bounds and the composition target of [`advise`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdviseOptions {
    /// `steps` bounds the planner search of each candidate;
    /// `backend_checks` bounds the calls of the caller's check over the
    /// whole call.
    pub limits: SearchLimits,
    /// Most candidates to plan, the frame as it is included.
    pub max_candidates: u32,
    /// `None` forbids composition: only the frame as it is is planned, and
    /// if it fails every layer on the output is reported as
    /// [`Placement::Compose`] with its blocker and no composition target.
    pub compose: Option<ComposeTarget>,
}

impl Default for AdviseOptions {
    fn default() -> Self {
        Self {
            limits: SearchLimits::default(),
            max_candidates: MAX_CANDIDATES,
            compose: None,
        }
    }
}

/// What the validation pass found out about one layer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Visibility {
    Hidden,
    Unclippable,
    Shown,
}

/// Lookup of the caller's buffers plus the composition target.
struct WithTarget<'a, L> {
    buffers: &'a L,
    id: BufferId,
    info: BufferInfo,
}

impl<L: BufferLookup> BufferLookup for WithTarget<'_, L> {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        if buffer == self.id {
            Some(self.info)
        } else {
            self.buffers.buffer_info(buffer)
        }
    }
}

/// Advises how to show `frame`: which layers go on which planes and which
/// contiguous run of layers the caller composes into one buffer of
/// `options.compose`.
///
/// `accept` is the caller's final check of each legal assignment, such as a
/// DRM `TEST_ONLY` commit; `|_, _| Verdict::Accept` gives the pure planner
/// answer. Its second argument is the index in `plan.assignments()` of the
/// composition target, or `None` when no layer is composed. The other
/// assignments' `layer` is the index in `frame.layers`.
///
/// Buffers need not be ready; only their metadata is looked up. Nothing is
/// remembered between calls, and the same input always gives the same
/// advice.
///
/// Work is bounded: at most `options.max_candidates` (and at most
/// [`MAX_CANDIDATES`]) planner searches of `options.limits.steps` steps
/// each, and at most `options.limits.backend_checks` calls of `accept`. If a
/// search is cut and a later candidate is accepted, the composed layers that
/// the cut candidate would have kept on planes get [`Blocker::Budget`].
pub fn advise<L: BufferLookup>(
    frame: &Frame<'_>,
    caps: &Capabilities,
    buffers: &L,
    options: &AdviseOptions,
    mut accept: impl FnMut(&Plan, Option<usize>) -> Verdict,
) -> Result<Advice, AdviceError> {
    let output_index = caps
        .output_index(frame.output)
        .ok_or(AdviceError::UnknownOutput(frame.output))?;
    let output_size = caps.outputs[output_index].size;
    let count = frame.layers.len();
    if count > MAX_LAYERS {
        return Err(AdviceError::TooManyLayers { count });
    }

    // Validate every layer as the planner does, and classify it.
    let mut visibility = [Visibility::Hidden; MAX_LAYERS];
    // `Some(reasons)` for a layer no plane can take on its own.
    let mut fixed = [None; MAX_LAYERS];
    for (index, layer) in frame.layers.iter().enumerate() {
        let info = checked_info(layer, buffers, index)?;
        visibility[index] = match clip_to_output(layer, output_size) {
            Ok(None) => Visibility::Hidden,
            Err(()) => Visibility::Unclippable,
            Ok(Some((source, destination))) => {
                let shown = Layer {
                    source,
                    destination,
                    ..*layer
                };
                fixed[index] = static_blocker(&shown, &info, caps, output_index, output_size);
                Visibility::Shown
            }
        };
    }

    // Stacking order of the layers on the output; insertion sort keeps
    // equal z in frame order, as the planner does.
    let mut order = [0usize; MAX_LAYERS];
    let mut n = 0;
    for (index, seen) in visibility[..count].iter().enumerate() {
        if *seen != Visibility::Hidden {
            order[n] = index;
            n += 1;
        }
    }
    for i in 1..n {
        let mut j = i;
        while j > 0 && frame.layers[order[j - 1]].z_order > frame.layers[order[j]].z_order {
            order.swap(j - 1, j);
            j -= 1;
        }
    }

    // Positions every run has to cover: layers no plane can ever take.
    let mut must: Option<(usize, usize)> = None;
    for (position, &index) in order[..n].iter().enumerate() {
        if visibility[index] == Visibility::Unclippable || fixed[index].is_some() {
            must = Some(must.map_or((position, position), |(low, _)| (low, position)));
        }
    }

    // The composition target stands in for a run under a buffer id no layer
    // uses: at most MAX_LAYERS ids are taken, so one of these is free.
    let target_id = (0..=MAX_LAYERS as u32)
        .map(|k| BufferId(u32::MAX - k))
        .find(|id| frame.layers.iter().all(|l| l.buffer != *id))
        .unwrap_or(BufferId(u32::MAX));
    let target = options.compose.map(|t| WithTarget {
        buffers,
        id: target_id,
        info: BufferInfo {
            size: output_size,
            format: t.format,
            modifier: t.modifier,
        },
    });

    let mut plan = Plan::EMPTY;
    let mut layers =
        [Layer::new(BufferId(0), Rect::new(0, 0, 0, 0), Rect::new(0, 0, 0, 0), 0); MAX_LAYERS];
    let mut candidates = 0u32;
    let mut checks = 0u32;
    let mut cut = false;
    let mut out_of_budget = false;
    // Frame indices of layers a refused candidate kept on planes.
    let mut refused = 0u32;
    let mut target_reasons = RejectReasons::NONE;
    let mut found: Option<Option<(usize, usize)>> = None;

    let longest = if target.is_some() { n } else { 0 };
    'search: for length in 0..=longest {
        let starts = if length == 0 { 1 } else { n - length + 1 };
        for low in 0..starts {
            let run = (length > 0).then(|| (low, low + length - 1));
            let covers = match (must, run) {
                (None, _) => true,
                (Some(_), None) => false,
                (Some((m_low, m_high)), Some((r_low, r_high))) => {
                    r_low <= m_low && m_high <= r_high
                }
            };
            if !covers {
                continue;
            }
            if candidates >= options.max_candidates.min(MAX_CANDIDATES) {
                out_of_budget = true;
                break 'search;
            }
            candidates += 1;

            let limits = SearchLimits {
                steps: options.limits.steps,
                backend_checks: options.limits.backend_checks.saturating_sub(checks),
            };
            let result = match (run, &target) {
                (Some((r_low, r_high)), Some(target)) => {
                    let target_layer = order[r_low];
                    layers[..count].copy_from_slice(frame.layers);
                    for &index in &order[r_low..=r_high] {
                        layers[index] = if index == target_layer {
                            Layer {
                                blend: options.compose.map_or(BlendMode::Opaque, |t| t.blend),
                                ..Layer::new(
                                    target_id,
                                    Rect::new(0, 0, output_size.width, output_size.height),
                                    Rect::new(0, 0, output_size.width, output_size.height),
                                    frame.layers[index].z_order,
                                )
                            }
                        } else {
                            hide(&frame.layers[index])
                        };
                    }
                    let reduced = Frame {
                        output: frame.output,
                        layers: &layers[..count],
                    };
                    plan_into(
                        &reduced,
                        caps,
                        target,
                        limits,
                        |p: &Plan| {
                            checks += 1;
                            let at = p.assignments().iter().position(|a| a.layer == target_layer);
                            accept(p, at)
                        },
                        &mut plan,
                    )
                }
                _ => plan_into(
                    frame,
                    caps,
                    buffers,
                    limits,
                    |p: &Plan| {
                        checks += 1;
                        accept(p, None)
                    },
                    &mut plan,
                ),
            };
            match result {
                Ok(()) => {
                    found = Some(run);
                    break 'search;
                }
                Err(PlanError::NoPlane { reasons, .. }) => {
                    if length == n {
                        target_reasons = reasons;
                    }
                }
                Err(PlanError::BackendRejected { .. }) => {
                    for (position, &index) in order[..n].iter().enumerate() {
                        if run.is_none_or(|(r_low, r_high)| position < r_low || position > r_high) {
                            refused |= 1 << index;
                        }
                    }
                    if length == n {
                        target_reasons = RejectReasons::NONE;
                    }
                }
                Err(PlanError::LimitExceeded) => {
                    cut = true;
                    if checks >= options.limits.backend_checks {
                        out_of_budget = true;
                        break 'search;
                    }
                }
                Err(PlanError::Aborted) => return Err(AdviceError::Aborted),
                // The layers were validated above and every run covers the
                // unclippable ones; nothing else can come back.
                Err(_) => {}
            }
        }
    }

    let run = match found {
        Some(run) => run,
        None if cut || out_of_budget => return Err(AdviceError::LimitExceeded),
        None if target.is_some() => {
            return Err(AdviceError::NoComposePlane {
                reasons: target_reasons,
            })
        }
        // No composition allowed and the frame as it is failed: every layer
        // on the output is off planes.
        None => Some((0, n.max(1) - 1)),
    };

    let mut advice = Advice {
        output: frame.output,
        layers: FixedVec::empty_filled(LayerAdvice::default()),
        compose: None,
        compose_run: None,
        candidates,
        backend_checks: checks,
    };
    let in_run =
        |position: usize| run.is_some_and(|(low, high)| low <= position && position <= high);
    let mut position_of = [usize::MAX; MAX_LAYERS];
    for (position, &index) in order[..n].iter().enumerate() {
        position_of[index] = position;
    }
    for index in 0..count {
        let position = position_of[index];
        let entry = if visibility[index] == Visibility::Hidden {
            LayerAdvice::default()
        } else if in_run(position) {
            let (blocker, reasons) = if visibility[index] == Visibility::Unclippable {
                (Blocker::Clip, RejectReasons::NONE)
            } else if let Some(reasons) = fixed[index] {
                (Blocker::Static, reasons)
            } else if cut {
                (Blocker::Budget, RejectReasons::NONE)
            } else if refused & (1 << index) != 0 {
                (Blocker::Backend, RejectReasons::NONE)
            } else {
                (Blocker::Combination, RejectReasons::NONE)
            };
            LayerAdvice {
                placement: Placement::Compose,
                blocker,
                reasons,
            }
        } else {
            LayerAdvice {
                placement: plan
                    .assignments()
                    .iter()
                    .find(|a| a.layer == index)
                    .map_or(Placement::Hidden, |a| Placement::Plane(*a)),
                ..LayerAdvice::default()
            }
        };
        // Cannot fail: at most `count <= MAX_LAYERS` entries are pushed.
        let _ = advice.layers.push(entry);
    }
    if found.is_some() && target.is_some() {
        if let Some((low, high)) = run {
            advice.compose = plan
                .assignments()
                .iter()
                .find(|a| a.layer == order[low])
                .map(|a| Assignment {
                    layer: COMPOSE_LAYER,
                    ..*a
                });
            advice.compose_run = Some((low as u8, high as u8));
        }
    }
    Ok(advice)
}

/// Per-plane static reasons of one layer, without a search: `out[i]` is
/// what stops `layer` on `caps.planes[i]` taken alone; entries past the
/// planes are cleared.
///
/// The layer is clipped to the output first, as the planner does. A layer
/// that is entirely off the output needs no plane and gets no reasons; one
/// that cannot be clipped exactly is judged as given (the planner refuses
/// it whatever the planes, see [`Blocker::Clip`]). The `layer` of an
/// [`AdviceError`] is 0.
pub fn explain_layer(
    layer: &Layer,
    info: &BufferInfo,
    caps: &Capabilities,
    output: OutputId,
    out: &mut [RejectReasons; MAX_PLANES],
) -> Result<(), AdviceError> {
    let output_index = caps
        .output_index(output)
        .ok_or(AdviceError::UnknownOutput(output))?;
    let output_size = caps.outputs[output_index].size;
    if layer.source.is_empty() || !layer.source.fits_within(info.size) {
        return Err(AdviceError::InvalidSource { layer: 0 });
    }
    if layer.destination.is_empty() {
        return Err(AdviceError::InvalidDestination { layer: 0 });
    }
    *out = [RejectReasons::NONE; MAX_PLANES];
    let judged = match clip_to_output(layer, output_size) {
        Ok(None) => return Ok(()),
        Ok(Some((source, destination))) => Layer {
            source,
            destination,
            ..*layer
        },
        Err(()) => *layer,
    };
    for (plane_index, plane) in caps.planes.iter().enumerate() {
        out[plane_index] = static_reasons(&judged, info, plane, output_index, output_size);
    }
    Ok(())
}

/// The planner's per-layer input checks, with the same errors in the same
/// order.
fn checked_info<L: BufferLookup>(
    layer: &Layer,
    buffers: &L,
    index: usize,
) -> Result<BufferInfo, AdviceError> {
    let info = buffers
        .buffer_info(layer.buffer)
        .ok_or(AdviceError::UnknownBuffer {
            layer: index,
            buffer: layer.buffer,
        })?;
    if layer.source.is_empty() || !layer.source.fits_within(info.size) {
        return Err(AdviceError::InvalidSource { layer: index });
    }
    if layer.destination.is_empty() {
        return Err(AdviceError::InvalidDestination { layer: index });
    }
    Ok(info)
}

/// `Some(reasons)` if no plane can take `layer` (already clipped) on its
/// own, with the reasons every plane that reaches the output shares.
fn static_blocker(
    layer: &Layer,
    info: &BufferInfo,
    caps: &Capabilities,
    output_index: usize,
    output_size: Size,
) -> Option<RejectReasons> {
    let mut common: Option<u32> = None;
    for plane in caps.planes.iter() {
        let reasons = static_reasons(layer, info, plane, output_index, output_size);
        if reasons.is_empty() {
            return None;
        }
        if plane.outputs & (1 << output_index) != 0 {
            common = Some(common.unwrap_or(u32::MAX) & reasons.bits());
        }
    }
    Some(match common {
        None => RejectReasons::OUTPUT,
        Some(bits) => reasons_from_bits(bits),
    })
}

/// Rebuilds a reason set from bits that came out of reason sets.
fn reasons_from_bits(bits: u32) -> RejectReasons {
    const ALL: [RejectReasons; 11] = [
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
    ];
    let mut reasons = RejectReasons::NONE;
    for reason in ALL {
        if bits & reason.bits() != 0 {
            reasons |= reason;
        }
    }
    reasons
}

/// `layer` moved entirely off any output, so that the planner drops it.
fn hide(layer: &Layer) -> Layer {
    Layer {
        destination: Rect::new(i32::MIN, i32::MIN, 1, 1),
        transform: Transform::Normal,
        alpha: ALPHA_OPAQUE,
        ..*layer
    }
}

/// Clips a layer to the output, exactly as the planner does.
///
/// A copy of the planner's private `clip_to_output`, which this module may
/// not reach: returns the visible source and destination, `None` if nothing
/// is visible, or an error if the visible part has no exact source region.
/// `tests/advise.rs` checks that both agree.
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
        _ => return Err(()),
    };
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
