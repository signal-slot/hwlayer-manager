//! Core side: `Capabilities` from `xcheck` and planner runs (plan item xc-core).
//!
//! Follows the field mapping of design section 4. [`CoreSide::new`] turns a
//! parsed [`XCheck`] into `composition_core` capabilities (plane ids become
//! indices, group plane ids become index masks) and a buffer table;
//! [`CoreSide::plan_frame`] runs the planner for one frame with a pluggable
//! backend check that sees the plan as [`PlaneState`] values, so a model-backed
//! check can be passed in later. The model itself is not here.
//!
//! The origin of the input (`xcheck` `origin`) is kept in [`CoreSide::origin`]
//! so that a report line can carry it; a synthetic or recorded topology gives
//! no hardware claim.

use composition_core as core;
use composition_core::caps::{
    Capabilities, FormatModifier, LayerTraits, OutputCaps, PlaneCaps, PlaneKind as CorePlaneKind,
    ResourceGroup, ScaleLimits, Zpos,
};
use composition_core::planner::{plan_with, BufferLookup, Plan, PlanError, SearchLimits, Verdict};
use composition_drm::DriverProfile;

use crate::state::{rotation, BlendMode, PlaneState, Rect, URect};
use crate::xcheck::{self, Origin, PlaneKind, Profile, XCheck, ZposDef};

/// Why an `xcheck` file could not be turned into core capabilities. The
/// reader already refuses most of these; they are checked again so that no
/// value is ever truncated silently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    /// A table does not fit the core's fixed capacity.
    Capacity(&'static str),
    /// A group names a plane id that no `plane` line defines.
    UnknownPlane(u32),
    /// A group capacity above 255 (the core stores a `u8`).
    GroupCapacity(u32),
}

/// Result of running one frame on the core side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The planner found an assignment, bottom layer first.
    Planned(Vec<PlaneState>),
    /// Core refused the frame before planning for a reason that is not about
    /// planes (design section 6): `skipped(<reason>)`.
    Skipped(PlanError),
    /// Any other [`PlanError`], `NoPlane` with its `RejectReasons` included.
    Error(PlanError),
}

/// Whether `error` is one of the non-plane refusals that make a frame
/// `skipped`: `UnknownBuffer`, `InvalidSource`, `InvalidDestination`,
/// `CannotClip`.
pub fn is_skip(error: &PlanError) -> bool {
    matches!(
        error,
        PlanError::UnknownBuffer { .. }
            | PlanError::InvalidSource { .. }
            | PlanError::InvalidDestination { .. }
            | PlanError::CannotClip { .. }
    )
}

/// Classifies a planner result.
pub fn classify(result: Result<Vec<PlaneState>, PlanError>) -> Outcome {
    match result {
        Ok(states) => Outcome::Planned(states),
        Err(e) if is_skip(&e) => Outcome::Skipped(e),
        Err(e) => Outcome::Error(e),
    }
}

/// Distinct zpos ranks of a frame's layers (design section 4, `z_order`):
/// the position of each layer in the stable sort by `z`, so equal `z` values
/// keep frame order and the later layer ranks higher, as core stacks them.
/// Ranks are `0..layers.len()`. The C side sets each layer's `zpos` property
/// to its rank.
pub fn zpos_ranks(layers: &[xcheck::Layer]) -> Vec<i32> {
    let mut order: Vec<usize> = (0..layers.len()).collect();
    order.sort_by_key(|&i| layers[i].z); // stable
    let mut ranks = vec![0; layers.len()];
    for (rank, &i) in order.iter().enumerate() {
        ranks[i] = rank as i32;
    }
    ranks
}

/// Buffer table of a file as a core [`BufferLookup`].
#[derive(Clone, Debug, Default)]
pub struct XBuffers(Vec<(u32, core::BufferInfo)>);

impl BufferLookup for XBuffers {
    fn buffer_info(&self, buffer: core::BufferId) -> Option<core::BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer.0).map(|e| e.1)
    }
}

fn pixel_format(fourcc: [u8; 4]) -> core::PixelFormat {
    core::PixelFormat::fourcc(&fourcc)
}

fn core_blend(mode: BlendMode) -> core::BlendMode {
    match mode {
        BlendMode::Opaque => core::BlendMode::Opaque,
        BlendMode::Premultiplied => core::BlendMode::Premultiplied,
        BlendMode::Coverage => core::BlendMode::Coverage,
    }
}

/// (DRM rotation bit, core transform), one entry per transform of the core.
const TRANSFORMS: [(u32, core::Transform); 6] = [
    (rotation::ROTATE_0, core::Transform::Normal),
    (rotation::ROTATE_90, core::Transform::Rotate90),
    (rotation::ROTATE_180, core::Transform::Rotate180),
    (rotation::ROTATE_270, core::Transform::Rotate270),
    (rotation::REFLECT_X, core::Transform::FlipH),
    (rotation::REFLECT_Y, core::Transform::FlipV),
];

fn core_transform(bit: u32) -> core::Transform {
    TRANSFORMS
        .iter()
        .find(|(b, _)| *b == bit)
        .map_or(core::Transform::Normal, |e| e.1)
}

fn transform_mask(bits: u32) -> u8 {
    TRANSFORMS
        .iter()
        .filter(|(b, _)| bits & b != 0)
        .fold(0, |m, (_, t)| m | t.mask())
}

/// The topology of one `xcheck` file as the core sees it.
pub struct CoreSide {
    /// Origin line of the input; copy it to every report line.
    pub origin: Origin,
    /// Capabilities, with the file's profile already applied.
    pub caps: Capabilities,
    /// Buffers of the file.
    pub buffers: XBuffers,
    /// Search limits for the planner (default: the core's).
    pub limits: SearchLimits,
}

impl CoreSide {
    /// Builds capabilities from `x`. `profile imx95dpu` calls
    /// `DriverProfile::Imx95Dpu.apply`; `generic` applies nothing. The
    /// reader guarantees that a profile never meets explicit constraints.
    pub fn new(x: &XCheck) -> Result<Self, BuildError> {
        let mut caps = Capabilities::default();
        for o in &x.outputs {
            caps.outputs
                .push(OutputCaps {
                    id: core::OutputId(o.id),
                    size: core::Size::new(o.width, o.height),
                    refresh_mhz: o.refresh_mhz,
                })
                .map_err(|_| BuildError::Capacity("outputs"))?;
        }
        for p in &x.planes {
            let mut plane = PlaneCaps {
                id: core::PlaneId(p.id),
                kind: match p.kind {
                    PlaneKind::Primary => CorePlaneKind::Primary,
                    PlaneKind::Overlay => CorePlaneKind::Overlay,
                    PlaneKind::Cursor => CorePlaneKind::Cursor,
                },
                outputs: p.outputs,
                scaling: p.scaling.map(|s| ScaleLimits {
                    max_upscale: s.up,
                    max_downscale: s.down,
                }),
                max_source: p.max_source.map(|(w, h)| core::Size::new(w, h)),
                plane_alpha: p.alpha,
                blend_modes: [
                    BlendMode::Opaque,
                    BlendMode::Premultiplied,
                    BlendMode::Coverage,
                ]
                .into_iter()
                .filter(|m| p.blend.contains(*m))
                .fold(0, |m, b| m | core_blend(b).mask()),
                transforms: transform_mask(p.transforms),
                zpos: match p.zpos {
                    ZposDef::Fixed(n) => Zpos::Fixed(n),
                    ZposDef::Range { min, max } => Zpos::Range { min, max },
                },
                full_output_only: p.full_output,
                ..PlaneCaps::default()
            };
            for f in &p.formats {
                plane
                    .formats
                    .push(FormatModifier {
                        format: pixel_format(f.fourcc),
                        modifier: core::Modifier(f.modifier),
                    })
                    .map_err(|_| BuildError::Capacity("formats per plane"))?;
            }
            caps.planes
                .push(plane)
                .map_err(|_| BuildError::Capacity("planes"))?;
        }
        for g in &x.groups {
            let mut mask = 0u32;
            for id in &g.planes {
                let index = x
                    .planes
                    .iter()
                    .position(|p| p.id == *id)
                    .ok_or(BuildError::UnknownPlane(*id))?;
                mask |= 1 << index;
            }
            let capacity =
                u8::try_from(g.capacity).map_err(|_| BuildError::GroupCapacity(g.capacity))?;
            caps.resource_groups
                .push(ResourceGroup {
                    consumers: LayerTraits(g.consumers.0),
                    planes: mask,
                    capacity,
                })
                .map_err(|_| BuildError::Capacity("resource groups"))?;
        }
        match x.profile {
            Profile::Generic => {}
            Profile::Imx95Dpu => DriverProfile::Imx95Dpu.apply(&mut caps),
        }
        let buffers = XBuffers(
            x.buffers
                .iter()
                .map(|b| {
                    (
                        b.id,
                        core::BufferInfo {
                            size: core::Size::new(b.width, b.height),
                            format: pixel_format(b.fourcc),
                            modifier: core::Modifier(b.modifier),
                        },
                    )
                })
                .collect(),
        );
        Ok(Self {
            origin: x.origin.clone(),
            caps,
            buffers,
            limits: SearchLimits::default(),
        })
    }

    /// Core layers of an `xcheck` frame.
    pub fn layers(frame: &xcheck::Frame) -> Vec<core::Layer> {
        frame
            .layers
            .iter()
            .map(|l| core::Layer {
                buffer: core::BufferId(l.buffer),
                source: core::Rect::new(l.src.x as i32, l.src.y as i32, l.src.w, l.src.h),
                destination: core::Rect::new(l.dst.x, l.dst.y, l.dst.w, l.dst.h),
                z_order: l.z,
                alpha: l.alpha,
                blend: core_blend(l.blend),
                transform: core_transform(l.transform),
            })
            .collect()
    }

    /// Plans `frame` with `check` as the backend check: the check is offered
    /// each legal assignment as [`PlaneState`] values (bottom layer first)
    /// and answers a [`Verdict`]. Use [`accept_all`] for the planner's own
    /// static verdict.
    pub fn plan_frame(
        &self,
        frame: &xcheck::Frame,
        check: &mut dyn FnMut(&[PlaneState]) -> Verdict,
    ) -> Result<Vec<PlaneState>, PlanError> {
        let layers = Self::layers(frame);
        let core_frame = core::Frame {
            output: core::OutputId(frame.output),
            layers: &layers,
        };
        let plan = plan_with(
            &core_frame,
            &self.caps,
            &self.buffers,
            self.limits,
            |plan| check(&plan_states(plan, frame)),
        )?;
        Ok(plan_states(&plan, frame))
    }

    /// [`CoreSide::plan_frame`] with the result classified.
    pub fn run_frame(
        &self,
        frame: &xcheck::Frame,
        check: &mut dyn FnMut(&[PlaneState]) -> Verdict,
    ) -> Outcome {
        classify(self.plan_frame(frame, check))
    }
}

/// The default backend check: accepts every assignment.
pub fn accept_all(_: &[PlaneState]) -> Verdict {
    Verdict::Accept
}

/// Converts a plan to plane states: framebuffer is the layer's buffer, source
/// 16.16, destination as clipped by the planner, zpos as assigned, alpha,
/// blend and rotation from the layer.
fn plan_states(plan: &Plan, frame: &xcheck::Frame) -> Vec<PlaneState> {
    plan.assignments()
        .iter()
        .map(|a| {
            let layer = &frame.layers[a.layer];
            PlaneState {
                plane_id: a.plane.0,
                fb: Some(layer.buffer),
                src: URect {
                    x: (a.source.x as u32) << 16,
                    y: (a.source.y as u32) << 16,
                    w: a.source.width << 16,
                    h: a.source.height << 16,
                },
                crtc: Rect {
                    x: a.destination.x,
                    y: a.destination.y,
                    w: a.destination.width,
                    h: a.destination.height,
                },
                zpos: a.zpos,
                alpha: layer.alpha,
                blend: layer.blend,
                rotation: rotation::property_value(layer.transform),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use composition_core::planner::RejectReasons;

    const TWO_PLANES: &str = "\
xcheck 1
origin synthetic 2026-11-28 core_side unit test
output 7 100 100 60000
plane 10 kind=primary outputs=0x1 zpos=fixed:0 alpha=0 blend=opaque transforms=normal scaling=none max_source=none full_output=0
  format AR24 0x0
plane 11 kind=overlay outputs=0x1 zpos=1..3 zpos_now=1 alpha=1 blend=opaque,premul transforms=normal,rot90 scaling=up:0x20000/down:0x10000 max_source=64x64 full_output=0
  format AR24 0x0
  format NV12 0x0
group consumers=scaled_h,yuv planes=11,10 capacity=2
buffer 1 100 100 AR24 0x0
buffer 2 50 50 NV12 0x0
buffer 3 50 50 XR24 0x0
frame two output=7
  layer buffer=1 src=0,0,100,100 dst=0,0,100,100 z=0 alpha=65535 blend=opaque transform=normal
  layer buffer=2 src=0,0,50,50 dst=10,10,50,50 z=1 alpha=32768 blend=premul transform=normal
end
frame nofmt output=7
  layer buffer=3 src=0,0,50,50 dst=0,0,50,50 z=0 alpha=65535 blend=opaque transform=normal
end
";

    fn side() -> (CoreSide, XCheck) {
        let x = xcheck::read(TWO_PLANES).unwrap();
        (CoreSide::new(&x).unwrap(), x)
    }

    #[test]
    fn capabilities_follow_the_mapping() {
        let (s, _) = side();
        assert_eq!(s.caps.outputs[0].id, core::OutputId(7));
        assert_eq!(s.caps.outputs[0].size, core::Size::new(100, 100));
        let p = &s.caps.planes[1];
        assert_eq!(p.id, core::PlaneId(11));
        assert_eq!(p.kind, CorePlaneKind::Overlay);
        assert_eq!(p.zpos, Zpos::Range { min: 1, max: 3 });
        assert!(p.plane_alpha);
        assert_eq!(
            p.blend_modes,
            core::BlendMode::Opaque.mask() | core::BlendMode::Premultiplied.mask()
        );
        assert_eq!(
            p.transforms,
            core::Transform::Normal.mask() | core::Transform::Rotate90.mask()
        );
        assert_eq!(
            p.scaling,
            Some(ScaleLimits {
                max_upscale: 0x20000,
                max_downscale: 0x10000
            })
        );
        assert_eq!(p.max_source, Some(core::Size::new(64, 64)));
        assert_eq!(p.formats.len(), 2);
        assert_eq!(s.caps.planes[0].zpos, Zpos::Fixed(0));
        // Group planes 11,10 become index bits 1 and 0.
        let g = s.caps.resource_groups[0];
        assert_eq!(g.planes, 0b11);
        assert_eq!(g.capacity, 2);
        assert_eq!(
            g.consumers,
            LayerTraits::SCALED_H.union(LayerTraits(1 << 3))
        );
    }

    #[test]
    fn accept_all_plans_and_states_follow_the_layers() {
        let (s, x) = side();
        let states = s.plan_frame(&x.frames[0], &mut accept_all).unwrap();
        assert_eq!(states.len(), 2);
        assert_eq!(states[0].plane_id, 10);
        assert_eq!(states[0].fb, Some(1));
        assert_eq!(states[0].src.w, 100 << 16);
        assert_eq!(states[1].plane_id, 11);
        assert_eq!(states[1].fb, Some(2));
        assert_eq!(states[1].alpha, 32768);
        assert_eq!(states[1].blend, BlendMode::Premultiplied);
        assert_eq!(
            states[1].crtc,
            Rect {
                x: 10,
                y: 10,
                w: 50,
                h: 50
            }
        );
        assert!(states[1].zpos > states[0].zpos);
    }

    #[test]
    fn a_lone_reflection_is_sent_with_rotate_0() {
        // A flip_h layer: core plans it, and its state carries ROTATE_0 |
        // REFLECT_X (as the C driver sends it), which the model accepts.
        let text = "\
xcheck 1
origin synthetic 2026-11-28 core_side unit test
output 7 100 100 60000
plane 10 kind=primary outputs=0x1 zpos=fixed:0 alpha=0 blend=opaque transforms=normal,flip_h scaling=none max_source=none full_output=0
  format AR24 0x0
buffer 1 100 100 AR24 0x0
frame flip output=7
  layer buffer=1 src=0,0,100,100 dst=0,0,100,100 z=0 alpha=65535 blend=opaque transform=flip_h
end
";
        let x = xcheck::read(text).unwrap();
        assert_eq!(x.frames[0].layers[0].transform, rotation::REFLECT_X);
        let s = CoreSide::new(&x).unwrap();
        let states = s.plan_frame(&x.frames[0], &mut accept_all).unwrap();
        assert_eq!(states[0].rotation, rotation::ROTATE_0 | rotation::REFLECT_X);
        for setting in [
            crate::model::Setting::Structural,
            crate::model::Setting::Full,
        ] {
            let m = crate::model::Model::new(&x, setting).unwrap();
            assert_eq!(m.check(7, &states).map_err(|r| r.reason), Ok(()));
        }
    }

    #[test]
    fn check_sees_states_and_can_refuse() {
        let (s, x) = side();
        let mut seen = 0;
        let r = s.plan_frame(&x.frames[0], &mut |st| {
            seen += 1;
            assert_eq!(st.len(), 2);
            Verdict::TryNext
        });
        assert!(seen >= 1);
        assert!(matches!(r, Err(PlanError::BackendRejected { .. })));
        let r = s.plan_frame(&x.frames[0], &mut |_| Verdict::GiveUp);
        assert_eq!(r, Err(PlanError::Aborted));
    }

    #[test]
    fn missing_format_is_no_plane_with_reasons() {
        let (s, x) = side();
        match s.run_frame(&x.frames[1], &mut accept_all) {
            Outcome::Error(PlanError::NoPlane { layer: 0, reasons }) => {
                assert!(reasons.contains(RejectReasons::FORMAT));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn non_plane_refusals_are_skipped() {
        let (s, mut x) = side();
        x.frames[0].layers[0].buffer = 99;
        assert!(matches!(
            s.run_frame(&x.frames[0], &mut accept_all),
            Outcome::Skipped(PlanError::UnknownBuffer { layer: 0, .. })
        ));
        x.frames[0].layers[0].buffer = 1;
        x.frames[0].layers[0].src.w = 200;
        assert!(matches!(
            s.run_frame(&x.frames[0], &mut accept_all),
            Outcome::Skipped(PlanError::InvalidSource { layer: 0 })
        ));
        x.frames[0].layers[0].src.w = 100;
        x.frames[0].layers[0].dst.w = 0;
        assert!(matches!(
            s.run_frame(&x.frames[0], &mut accept_all),
            Outcome::Skipped(PlanError::InvalidDestination { layer: 0 })
        ));
        x.frames[0].layers[0].dst = Rect {
            x: -10,
            y: 0,
            w: 100,
            h: 100,
        };
        x.frames[0].layers[0].transform = rotation::ROTATE_90;
        assert!(matches!(
            s.run_frame(&x.frames[0], &mut accept_all),
            Outcome::Skipped(PlanError::CannotClip { layer: 0 })
        ));
    }

    #[test]
    fn unknown_output_is_an_error_not_a_skip() {
        let (s, mut x) = side();
        x.frames[0].output = 99;
        assert!(matches!(
            s.run_frame(&x.frames[0], &mut accept_all),
            Outcome::Error(PlanError::UnknownOutput(_))
        ));
    }

    #[test]
    fn unknown_group_plane_is_refused() {
        let (_, mut x) = side();
        x.groups[0].planes.push(77);
        assert_eq!(CoreSide::new(&x).err(), Some(BuildError::UnknownPlane(77)));
    }

    fn layer(z: i32) -> xcheck::Layer {
        xcheck::Layer {
            buffer: 1,
            src: URect::default(),
            dst: Rect::default(),
            z,
            alpha: 0xFFFF,
            blend: BlendMode::Opaque,
            transform: rotation::ROTATE_0,
        }
    }

    #[test]
    fn ties_rank_in_frame_order() {
        let ls = [layer(5), layer(1), layer(5), layer(1), layer(-3)];
        // Sorted: -3 (4), 1 (1), 1 (3), 5 (0), 5 (2).
        assert_eq!(zpos_ranks(&ls), vec![3, 1, 4, 2, 0]);
        assert_eq!(zpos_ranks(&[]), Vec::<i32>::new());
    }

    const IMX95: &str = include_str!("../fixtures/imx95-synthetic.xcheck");

    #[test]
    fn imx95_fixture_applies_the_profile() {
        let x = xcheck::read(IMX95).unwrap();
        assert_eq!(x.profile, Profile::Imx95Dpu);
        assert!(x.groups.is_empty() && x.planes.iter().all(|p| p.scaling.is_none()));
        assert_eq!(x.origin.kind, xcheck::OriginKind::Synthetic);
        let s = CoreSide::new(&x).unwrap();
        assert_eq!(s.origin, x.origin);
        assert_eq!(s.caps.resource_groups.len(), 4);
        assert!(s
            .caps
            .planes
            .iter()
            .all(|p| p.scaling.is_some() && p.max_source == Some(core::Size::new(8192, 8192))));
        // Generic: nothing applied.
        let mut g = x.clone();
        g.profile = Profile::Generic;
        let gs = CoreSide::new(&g).unwrap();
        assert_eq!(gs.caps.resource_groups.len(), 0);
        assert!(gs.caps.planes.iter().all(|p| p.scaling.is_none()));
    }

    #[test]
    fn imx95_fixture_frames_run() {
        let x = xcheck::read(IMX95).unwrap();
        let s = CoreSide::new(&x).unwrap();
        let by = |n: &str| x.frames.iter().find(|f| f.name == n).unwrap();
        match s.run_frame(by("six_rgb"), &mut accept_all) {
            Outcome::Planned(st) => assert_eq!(st.len(), 6),
            other => panic!("{other:?}"),
        }
        // The profile's fetch-unit pool refuses a fifth YUV layer.
        match s.run_frame(by("five_yuv"), &mut accept_all) {
            Outcome::Error(PlanError::NoPlane { reasons, .. }) => {
                assert!(reasons.contains(RejectReasons::RESOURCE))
            }
            other => panic!("{other:?}"),
        }
    }
}
