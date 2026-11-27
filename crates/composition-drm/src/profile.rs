//! Per-driver knowledge that KMS properties do not carry.
//!
//! KMS describes formats, stacking and blending, but not scaling limits or
//! hardware units shared between planes. A profile adds those so that the
//! planner can refuse a frame, and say why, before the kernel is asked.
//! Without a profile nothing is lost in correctness: the atomic `TEST_ONLY`
//! check still rejects what the hardware cannot do, only later and without a
//! reason.
//!
//! A profile is chosen explicitly through [`crate::DrmOptions::profile`].

use composition_core::{Capabilities, LayerTraits, PlaneKind, ResourceGroup, ScaleLimits, Size};

/// Which driver's constraints to add to the discovered capabilities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DriverProfile {
    /// Only what KMS reports.
    #[default]
    Generic,
    /// NXP i.MX95 display processing unit, vendor driver `dpu95`.
    ///
    /// **Read from source, not measured.** Taken from `nxp-imx/linux-imx`
    /// `lf-6.18.y` (`drivers/gpu/drm/imx/dpu95`); see
    /// `docs/platform-capabilities.md`. Confirm on a board before relying on
    /// it.
    ///
    /// - Six fetch units feed all planes of both display streams, so at most
    ///   six planes are active.
    /// - Four of them (FetchYUV) can read YCbCr or feed the scaler; a layer
    ///   that is YCbCr or scaled needs one of those.
    /// - There is one horizontal and one vertical scaler, upscaling only.
    ///
    /// The units are shared with the other display stream, which the planner
    /// cannot see, so the kernel check may still refuse a frame this profile
    /// allows.
    Imx95Dpu,
}

impl DriverProfile {
    /// Adds the profile's constraints to `caps`.
    pub fn apply(self, caps: &mut Capabilities) {
        match self {
            DriverProfile::Generic => {}
            DriverProfile::Imx95Dpu => {
                // Widened so that a (currently unreachable) count of 32 cannot
                // overflow.
                let all = ((1u64 << caps.planes.len()) - 1) as u32;
                for plane in caps.planes.as_mut_slice() {
                    if plane.kind != PlaneKind::Cursor {
                        plane.scaling = Some(ScaleLimits {
                            max_upscale: 2048 << 16,
                            max_downscale: 1 << 16,
                        });
                        plane.max_source = Some(Size::new(8192, 8192));
                    }
                }
                let groups = [
                    (LayerTraits::ANY, 6),
                    (LayerTraits::SCALED.union(LayerTraits::YUV), 4),
                    (LayerTraits::SCALED_H, 1),
                    (LayerTraits::SCALED_V, 1),
                ];
                for (consumers, capacity) in groups {
                    let group = ResourceGroup {
                        consumers,
                        planes: all,
                        capacity,
                    };
                    // Applying twice must not duplicate the groups.
                    if !caps.resource_groups.as_slice().contains(&group) {
                        let _ = caps.resource_groups.push(group);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use composition_core::planner::{plan, BufferLookup};
    use composition_core::{
        BlendMode, BufferId, BufferInfo, FormatModifier, Frame, Layer, Modifier, OutputCaps,
        OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, Rect, RejectReasons, Zpos,
    };

    const OUT: OutputId = OutputId(0);
    const FULL: Rect = Rect::new(0, 0, 1920, 1080);
    const RGB: BufferId = BufferId(1);
    const YUV: BufferId = BufferId(2);
    const SMALL: BufferId = BufferId(3);

    struct Buffers;

    impl BufferLookup for Buffers {
        fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
            let (size, format) = match buffer {
                RGB => (Size::new(1920, 1080), PixelFormat::ARGB8888),
                YUV => (Size::new(1920, 1080), PixelFormat::NV12),
                SMALL => (Size::new(960, 540), PixelFormat::ARGB8888),
                _ => return None,
            };
            Some(BufferInfo {
                size,
                format,
                modifier: Modifier::LINEAR,
            })
        }
    }

    /// One display stream of the i.MX95 as the vendor driver describes it:
    /// its primary plane and the five overlays, all alike.
    fn dpu95_stream() -> Capabilities {
        let mut caps = Capabilities::default();
        caps.outputs
            .push(OutputCaps {
                id: OUT,
                size: Size::new(1920, 1080),
                refresh_mhz: 60_000,
            })
            .unwrap();
        for index in 0..6 {
            let mut plane = PlaneCaps {
                id: PlaneId(40 + index),
                kind: if index == 0 {
                    PlaneKind::Primary
                } else {
                    PlaneKind::Overlay
                },
                outputs: 1,
                zpos: Zpos::Range { min: 0, max: 5 },
                plane_alpha: true,
                blend_modes: BlendMode::Opaque.mask()
                    | BlendMode::Premultiplied.mask()
                    | BlendMode::Coverage.mask(),
                ..PlaneCaps::default()
            };
            for format in [PixelFormat::ARGB8888, PixelFormat::NV12] {
                plane.formats.push(FormatModifier::linear(format)).unwrap();
            }
            caps.planes.push(plane).unwrap();
        }
        DriverProfile::Imx95Dpu.apply(&mut caps);
        caps
    }

    fn plan_layers(layers: &[Layer]) -> Result<usize, PlanError> {
        let frame = Frame {
            output: OUT,
            layers,
        };
        plan(&frame, &dpu95_stream(), &Buffers).map(|p| p.assignments().len())
    }

    fn full(buffer: BufferId, z: i32) -> Layer {
        Layer::new(buffer, FULL, FULL, z)
    }

    #[test]
    fn six_rgb_layers_fit() {
        let layers: Vec<Layer> = (0..6).map(|z| full(RGB, z)).collect();
        assert_eq!(plan_layers(&layers), Ok(6));
    }

    #[test]
    fn a_fifth_yuv_layer_has_no_fetch_unit() {
        let mut layers: Vec<Layer> = (0..4).map(|z| full(YUV, z)).collect();
        layers.push(full(RGB, 4));
        layers.push(full(RGB, 5));
        assert_eq!(plan_layers(&layers), Ok(6));

        layers[4] = full(YUV, 4);
        let Err(PlanError::NoPlane { layer: 4, reasons }) = plan_layers(&layers) else {
            panic!("a fifth YUV layer must be refused");
        };
        assert!(reasons.contains(RejectReasons::RESOURCE));
    }

    #[test]
    fn only_one_layer_can_use_each_scaler() {
        let half = Rect::new(0, 0, 960, 540);
        let upscaled = |z| Layer::new(SMALL, half, FULL, z);
        assert_eq!(plan_layers(&[full(RGB, 0), upscaled(1)]), Ok(2));

        let Err(PlanError::NoPlane { layer: 1, reasons }) =
            plan_layers(&[upscaled(0), upscaled(1)])
        else {
            panic!("two scaled layers must be refused");
        };
        assert!(reasons.contains(RejectReasons::RESOURCE));

        // One layer scaled only horizontally and one only vertically use
        // different scalers.
        let wide = Layer::new(SMALL, half, Rect::new(0, 0, 1920, 540), 0);
        let tall = Layer::new(SMALL, half, Rect::new(0, 0, 960, 1080), 1);
        assert_eq!(plan_layers(&[wide, tall]), Ok(2));
    }

    #[test]
    fn downscaling_is_refused() {
        let shrunk = Layer::new(RGB, FULL, Rect::new(0, 0, 960, 540), 0);
        assert_eq!(
            plan_layers(&[shrunk]),
            Err(PlanError::NoPlane {
                layer: 0,
                reasons: RejectReasons::SCALING
            })
        );
    }

    #[test]
    fn generic_profile_adds_nothing() {
        let mut caps = Capabilities::default();
        DriverProfile::Generic.apply(&mut caps);
        assert_eq!(caps, Capabilities::default());
    }
}
