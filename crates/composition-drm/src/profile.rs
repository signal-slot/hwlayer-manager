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

mod rk3588;
mod vc4;

use composition_core::{
    BlendMode, Capabilities, LayerTraits, PlaneKind, ResourceGroup, ScaleLimits, Size,
};

/// Which driver's constraints to add to the discovered capabilities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DriverProfile {
    /// Only what KMS reports.
    #[default]
    Generic,
    /// NXP i.MX95 display processing unit, vendor driver `dpu95`.
    ///
    /// Checked on 2026-11-28 against the Verdin iMX95 (kernel
    /// `6.6.157-7.8.0`, SoC revision 2.0) and against the driver source of
    /// that kernel (`linux-toradex` `64f85d65c112`, `docs/imx95-dpu-source-6.6.md`).
    /// Tags: **observed** = seen through KMS on the board
    /// (`docs/platform-capabilities.md`); **source** = read from that 6.6
    /// source, not run; **inferred** = concluded, neither seen nor stated.
    /// The resource groups below are never visible through KMS, so none of
    /// them is observed; confirm them with `TEST_ONLY` on the board.
    ///
    /// - Six planes per output (primary plus five overlays), all alike:
    ///   observed. At most six planes are active across both display
    ///   streams, because six fetch units and six layer blends are shared:
    ///   source (the `ANY` group).
    /// - Four of the six fetch units (FetchYUV) can read YCbCr or feed a
    ///   scaler; the other two (FetchLayer) take only an unscaled RGB layer.
    ///   A YCbCr or scaled layer needs a FetchYUV unit: source (FetchLayer's
    ///   empty capability mask is inferred from the absence of any
    ///   assignment).
    /// - One horizontal and one vertical scaler, shared by all planes of both
    ///   streams: source. The vertical scaler exists only because the board's
    ///   SoC is not revision 1.x (on 1.0/1.1 vertical scaling is refused):
    ///   source for the rule, observed revision 2.0, so "available here" is
    ///   inferred. Do not use this profile unchanged on a revision 1.x part.
    /// - Upscaling up to 2048x per axis, no downscaling: source
    ///   (`min_scale = 1/2048`, `max_scale = NO_SCALING`).
    /// - `max_source` 8192x8192 is the limit of an *unscaled* layer: source.
    ///   A scaled layer is limited to 2048 per axis, which a single
    ///   `max_source` cannot express, so that limit is **not modelled**: a
    ///   scaled source larger than 2048 passes the planner and is refused by
    ///   `TEST_ONLY`. The driver judges "unscaled" as `src_w == dst_w ||
    ///   src_h == dst_h`, so one-axis scaling also gets 8192: source.
    /// - Scaling is assumed on every non-cursor plane. KMS shows no scaling
    ///   property (observed: only `SCALING_FILTER`), and the driver sets the
    ///   same limits on all planes: source. The board has no cursor plane
    ///   (observed), so the cursor exemption never applies there.
    ///
    /// Not modelled (source, left to `TEST_ONLY`): the 2048 scaled-source
    /// limit, even source coordinates for subsampled formats, off-screen
    /// destinations, BT.709 full range on YCbCr, the bottom plane being
    /// forced opaque, and units already bound to the other display stream.
    /// On the Verdin the second stream has no connector, but the planner
    /// cannot see that either.
    ///
    /// The kernel check may therefore still refuse a frame this profile
    /// allows; it must not accept one the profile refuses, except through
    /// the inferred items above.
    Imx95Dpu,
    /// STMicroelectronics LTDC on the STM32MP257F-DK (ST driver
    /// `drivers/gpu/drm/stm/ltdc.c`, hardware version 4.x).
    ///
    /// Tags: **observed <date>** = `TEST_ONLY` against the kernel on the
    /// board (kernel `6.6.129`, `docs/stm32mp257-ltdc-board.md`);
    /// **source** = read from the ST `v6.18-stm32mp` tree
    /// (`docs/stm32mp257-ltdc-source-full.md`, `ltdc.c:1585-1609`);
    /// **inferred** = concluded, neither seen nor stated.
    ///
    /// - Three planes on one CRTC: primary, overlay, overlay (observed
    ///   2026-11-27, `docs/observed/stm32mp257.md`). Layer count is read from
    ///   `LTDC_LCR` at run time: source.
    /// - Upscaling only, `max_downscale` 1.0: source ("Downscaling is not
    ///   supported", `ltdc.c:1585-1600`); observed 2026-11-27: downscale
    ///   0.5, 0.75 and 0.9 rejected (EINVAL) on all three planes.
    /// - `max_upscale` 1080.0 (`1080 << 16`): observed 2026-11-27: no
    ///   upscale was rejected on the primary or the first overlay, on either
    ///   axis or both, up to the factor that shrinks the source to one pixel
    ///   (1920 horizontally, 1080 vertically and on both axes), so there is
    ///   no driver bound up to the mode size. 1080 is the largest factor
    ///   seen accepted on every axis and is used as the single bound;
    ///   larger factors need a source below one pixel and were not tried.
    ///   The source has no numeric cap either (the factor is only masked to
    ///   16 bits, `ltdc.c:1609`); any hardware cap is in the reference
    ///   manual, not read. Quality and bandwidth at such factors were not
    ///   looked at.
    /// - Which planes scale: the primary plane and the first overlay, in
    ///   plane order (observed 2026-11-27: planes 34 and 42 scale, plane 49,
    ///   the second overlay, accepts only 1.0). Source: scaling is a
    ///   per-layer hardware ability (`LXCR_C1R_SCA`, `ltdc.c:238`), so which
    ///   layers have it is a register fact, not in the source; that it is
    ///   layers 0 and 1 is inferred from the one board. Planes are
    ///   identified by kind and order among the overlays, not by KMS object
    ///   id, because ids change between kernels and boot; DRM enumerates
    ///   the planes in layer order (inferred; observed ids 34 < 42 < 49
    ///   follow it). The second and later overlays are left unclaimed
    ///   (`scaling` stays as KMS reported it), so the planner treats them as
    ///   unscaled only through its own default.
    /// - Straight alpha, not premultiplied: observed 2026-11-27 (session doc
    ///   `docs/stm32mp257-ltdc-session-2026-11-28.md`, Q2): the output CRC on
    ///   plane 42 (AR24) matches colour x pixel alpha x plane alpha over the
    ///   plane below, in all four cases that tell the modes apart. KMS has no
    ///   `pixel blend mode` property here, so discovery reports
    ///   premultiplied; the profile replaces that by
    ///   [`BlendMode::Coverage`] on every non-cursor plane. Source
    ///   (`docs/stm32mp257-ltdc-source-diff-6.6.md` row 42): the same blend
    ///   factors (`BF1_PAXCA`) are programmed for every layer, so planes 34
    ///   and 49, not measured, are inferred to blend alike. Plane alpha stays
    ///   as KMS reports it.
    /// - Transforms and formats are left as KMS reports them: neither the
    ///   session of 2026-11-27 nor session C (observed 2026-11-28, no
    ///   picture: the KVM capture failed) ran Q6 (rotation, reflections) or
    ///   NV12, so nothing observed calls for a change (inferred). Never
    ///   claim 90/270. Pinned by the host test
    ///   `stm32_ltdc_adds_no_transform_claims`.
    /// - YUV formats cannot be scaled on any plane (source,
    ///   `ltdc.c:1585-1600`); that is **not modelled** here and is left to
    ///   `TEST_ONLY`.
    /// - Not measured: scaling quality and bandwidth/underrun limits at large
    ///   factors; the source-size limit of 2048x2048 (source:
    ///   `STM_MAX_FB_WIDTH`/`HEIGHT` in `drv.c`) is a framebuffer limit and
    ///   is not set as `max_source`.
    ///
    /// Selection by driver name (`stm`) is not done here; the profile is
    /// chosen explicitly like the others.
    Stm32Ltdc,
    /// Rockchip RK3588 VOP2, mainline driver `rockchip` (checked against
    /// kernel 6.18 on the NanoPC-T6). The table and its evidence tags are in
    /// the module documentation of `src/profile/rk3588.rs`.
    Rk3588Vop2,
    /// Raspberry Pi BCM2711 HVS5 (vc4): scaling, `max_source` and an LBM
    /// group. The tagged table is the module doc of `profile/vc4.rs`.
    Vc4Hvs5,
    /// Raspberry Pi BCM2712 HVS6 (vc4): scaling, `max_source`, LBM and UPM
    /// groups, and no T-tiled or SAND64 pairs. The tagged table is the module
    /// doc of `profile/vc4.rs`.
    Vc4Hvs6,
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
            DriverProfile::Stm32Ltdc => {
                // Primary plus the first overlay scale; later overlays do not.
                let mut overlays = 0;
                for plane in caps.planes.as_mut_slice() {
                    // Straight alpha (observed 2026-11-27); the planner then
                    // refuses premultiplied layers on these planes.
                    if plane.kind != PlaneKind::Cursor {
                        plane.blend_modes = BlendMode::Coverage.mask();
                    }
                    let scales = match plane.kind {
                        PlaneKind::Primary => true,
                        PlaneKind::Overlay => {
                            overlays += 1;
                            overlays == 1
                        }
                        PlaneKind::Cursor => false,
                    };
                    if scales {
                        plane.scaling = Some(ScaleLimits {
                            max_upscale: 1080 << 16,
                            max_downscale: 1 << 16,
                        });
                    }
                }
            }
            DriverProfile::Rk3588Vop2 => rk3588::apply_rk3588_vop2(caps),
            DriverProfile::Vc4Hvs5 => vc4::apply_vc4(caps, vc4::Generation::Hvs5),
            DriverProfile::Vc4Hvs6 => vc4::apply_vc4(caps, vc4::Generation::Hvs6),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use composition_core::planner::{plan, BufferLookup};
    use composition_core::Transform;
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
    fn limits_match_the_6_6_driver() {
        let caps = dpu95_stream();
        for plane in caps.planes.as_slice() {
            // Unscaled sources up to 8192; the 2048 limit for scaled sources
            // is not modelled (left to TEST_ONLY), so it must not be applied
            // to unscaled layers either.
            assert_eq!(plane.max_source, Some(Size::new(8192, 8192)));
            // Upscale up to 2048x, no downscale.
            assert_eq!(
                plane.scaling,
                Some(ScaleLimits {
                    max_upscale: 2048 << 16,
                    max_downscale: 1 << 16,
                })
            );
        }
        // Six planes in total, four YCbCr/scaler-capable fetch units, one
        // horizontal and one vertical scaler.
        let groups = caps.resource_groups.as_slice();
        let capacity =
            |t: LayerTraits| groups.iter().find(|g| g.consumers == t).map(|g| g.capacity);
        assert_eq!(capacity(LayerTraits::ANY), Some(6));
        assert_eq!(
            capacity(LayerTraits::SCALED.union(LayerTraits::YUV)),
            Some(4)
        );
        assert_eq!(capacity(LayerTraits::SCALED_H), Some(1));
        assert_eq!(capacity(LayerTraits::SCALED_V), Some(1));
        assert_eq!(groups.len(), 4);
    }

    #[test]
    fn generic_profile_adds_nothing() {
        let mut caps = Capabilities::default();
        DriverProfile::Generic.apply(&mut caps);
        assert_eq!(caps, Capabilities::default());
    }

    /// Session C produced no picture, so the profile adds no transform claim
    /// and a layer asking for a transform KMS did not report is refused.
    #[test]
    fn stm32_ltdc_adds_no_transform_claims() {
        let mut caps = Capabilities::default();
        caps.outputs
            .push(OutputCaps {
                id: OUT,
                size: Size::new(1920, 1080),
                refresh_mhz: 60_000,
            })
            .unwrap();
        for (index, kind) in [PlaneKind::Primary, PlaneKind::Overlay, PlaneKind::Overlay]
            .into_iter()
            .enumerate()
        {
            let mut plane = PlaneCaps {
                id: PlaneId(34 + index as u32),
                kind,
                outputs: 1,
                transforms: Transform::Normal.mask(),
                ..PlaneCaps::default()
            };
            plane
                .formats
                .push(FormatModifier::linear(PixelFormat::ARGB8888))
                .unwrap();
            caps.planes.push(plane).unwrap();
        }
        DriverProfile::Stm32Ltdc.apply(&mut caps);
        for plane in caps.planes.as_slice() {
            assert_eq!(plane.transforms, Transform::Normal.mask());
        }
        for transform in [
            Transform::FlipH,
            Transform::FlipV,
            Transform::Rotate90,
            Transform::Rotate180,
            Transform::Rotate270,
        ] {
            let mut layer = full(RGB, 0);
            layer.transform = transform;
            layer.blend = BlendMode::Coverage;
            let frame = Frame {
                output: OUT,
                layers: &[layer],
            };
            let Err(PlanError::NoPlane { reasons, .. }) = plan(&frame, &caps, &Buffers) else {
                panic!("{transform:?} must be refused");
            };
            assert!(reasons.contains(RejectReasons::TRANSFORM), "{transform:?}");
        }
    }
}
