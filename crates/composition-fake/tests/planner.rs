//! Planner decisions against the fake backend: assignments and the explicit
//! errors reported when a frame cannot be placed (SoW §6, §6.7).
//!
//! Every case goes through `Engine::validate`/`present`, so the backend check
//! is part of the path. A rejected frame must leave the display untouched:
//! the planner never drops or weakens a layer to make a frame fit.

use composition_core::limits::MAX_LAYERS;
use composition_core::{
    BlendMode, BufferId, BufferInfo, Capabilities, Engine, Error, FormatModifier, Frame, Layer,
    LayerTraits, Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId,
    PlaneKind, Rect, RejectReasons, ResourceGroup, ScaleLimits, Size, Transform, Zpos,
};
use composition_fake::{FakeBackend, PlaneState};

const OUT: OutputId = OutputId(0);
const SCREEN: Size = Size::new(1280, 720);
const FULL: Rect = Rect::new(0, 0, 1280, 720);

const XRGB: PixelFormat = PixelFormat::XRGB8888;
const ARGB: PixelFormat = PixelFormat::ARGB8888;
const RGB565: PixelFormat = PixelFormat::RGB565;
const NV12: PixelFormat = PixelFormat::NV12;

/// A modifier no plane in these tests accepts unless it says so.
const TILED: Modifier = Modifier(0x0100_0000_0000_0001);

/// A scale factor of 1 in 16.16 fixed point.
const ONE: u32 = 0x1_0000;

type Placement = Vec<(usize, u32, i32)>;

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

fn scaling(plane: PlaneCaps, max_upscale: u32, max_downscale: u32) -> PlaneCaps {
    PlaneCaps {
        scaling: Some(ScaleLimits {
            max_upscale,
            max_downscale,
        }),
        ..plane
    }
}

fn caps(planes: &[PlaneCaps], groups: &[ResourceGroup]) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: SCREEN,
            refresh_mhz: 60_000,
        })
        .unwrap();
    for plane in planes {
        caps.planes.push(*plane).unwrap();
    }
    for group in groups {
        caps.resource_groups.push(*group).unwrap();
    }
    caps
}

fn register(engine: &mut Engine<FakeBackend>, id: u32, format: PixelFormat, modifier: Modifier) {
    let id = BufferId(id);
    engine
        .register_buffer(
            id,
            BufferInfo {
                size: SCREEN,
                format,
                modifier,
            },
        )
        .unwrap();
    engine.finish_write(id).unwrap();
}

/// An engine with full-screen linear buffers 1..=n, `formats[i]` being
/// buffer i+1, all ready to be shown.
fn engine(caps: Capabilities, formats: &[PixelFormat]) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(caps));
    for (i, format) in formats.iter().enumerate() {
        register(&mut engine, i as u32 + 1, *format, Modifier::LINEAR);
    }
    engine
}

fn layer(buffer: u32, z_order: i32) -> Layer {
    Layer::new(BufferId(buffer), FULL, FULL, z_order)
}

/// A layer shown at `width` x `height` from the top left corner.
fn window(buffer: u32, width: u32, height: u32, z_order: i32) -> Layer {
    Layer::new(
        BufferId(buffer),
        FULL,
        Rect::new(0, 0, width, height),
        z_order,
    )
}

fn validate(engine: &mut Engine<FakeBackend>, layers: &[Layer]) -> Result<Placement, Error> {
    let frame = Frame {
        output: OUT,
        layers,
    };
    engine.validate(&frame).map(|plan| {
        plan.assignments()
            .iter()
            .map(|a| (a.layer, a.plane.0, a.zpos))
            .collect()
    })
}

fn no_plane(layer: usize, reasons: RejectReasons) -> Error {
    Error::Plan(PlanError::NoPlane { layer, reasons })
}

/// A refused frame must change nothing on the display.
fn assert_untouched(engine: &Engine<FakeBackend>) {
    assert_eq!(engine.backend().commit_count(), 0);
    assert_eq!(engine.backend().pending(OUT), None);
    assert_eq!(engine.backend().scanout(OUT), None);
}

// ---- more layers than planes ----

#[test]
fn more_layers_than_planes_is_refused_not_trimmed() {
    // (planes, layers): the first layer without a plane is the one reported.
    for (planes, layers) in [(1usize, 2usize), (2, 3), (2, 5)] {
        let planes: Vec<_> = (0..planes)
            .map(|i| plane(10 + i as u32, Zpos::Fixed(i as i32), &[XRGB]))
            .collect();
        let first_without = planes.len();
        let mut engine = engine(caps(&planes, &[]), &[XRGB; 5]);
        let frame: Vec<_> = (0..layers).map(|i| layer(i as u32 + 1, i as i32)).collect();
        let result = engine.present(&Frame {
            output: OUT,
            layers: &frame,
        });
        assert_eq!(
            result,
            Err(no_plane(first_without, RejectReasons::IN_USE)),
            "{layers} layers on {} planes",
            planes.len()
        );
        assert_untouched(&engine);
    }
}

#[test]
fn planes_of_other_outputs_do_not_count() {
    // Two planes exist, but only one can be attached to output 0.
    let mut other = plane(11, Zpos::Fixed(1), &[XRGB]);
    other.outputs = 1 << 1;
    let mut engine = engine(
        caps(&[plane(10, Zpos::Fixed(0), &[XRGB]), other], &[]),
        &[XRGB; 2],
    );
    assert_eq!(
        validate(&mut engine, &[layer(1, 0), layer(2, 1)]),
        Err(no_plane(1, RejectReasons::IN_USE | RejectReasons::OUTPUT))
    );
}

#[test]
fn layers_beyond_the_core_limit_are_a_distinct_error() {
    let planes: Vec<_> = (0..16)
        .map(|i| plane(i + 1, Zpos::Fixed(i as i32), &[XRGB]))
        .collect();
    let mut engine = engine(caps(&planes, &[]), &[XRGB]);
    let layers = vec![layer(1, 0); MAX_LAYERS + 1];
    assert_eq!(
        validate(&mut engine, &layers),
        Err(Error::Plan(PlanError::TooManyLayers {
            count: MAX_LAYERS + 1
        }))
    );
}

#[test]
fn layers_entirely_off_screen_need_no_plane() {
    let mut engine = engine(caps(&[plane(10, Zpos::Fixed(0), &[XRGB])], &[]), &[XRGB; 2]);
    let off = Layer::new(BufferId(2), FULL, Rect::new(2000, 0, 1280, 720), 1);
    assert_eq!(
        validate(&mut engine, &[layer(1, 0), off]),
        Ok(vec![(0, 10, 0)])
    );
}

// ---- format and modifier ----

#[test]
fn unsupported_format_or_modifier_reports_format() {
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(0), &[XRGB]),
                plane(11, Zpos::Fixed(1), &[XRGB, ARGB]),
            ],
            &[],
        ),
        &[RGB565, NV12, XRGB],
    );
    // No plane takes RGB565 or NV12.
    assert_eq!(
        validate(&mut engine, &[layer(1, 0)]),
        Err(no_plane(0, RejectReasons::FORMAT))
    );
    assert_eq!(
        validate(&mut engine, &[layer(2, 0)]),
        Err(no_plane(0, RejectReasons::FORMAT))
    );
    // A placeable layer below an unplaceable one: the unplaceable one is
    // reported and nothing is dropped.
    assert_eq!(
        validate(&mut engine, &[layer(3, 0), layer(1, 1)]),
        Err(no_plane(1, RejectReasons::FORMAT))
    );
    assert_eq!(validate(&mut engine, &[layer(3, 0)]), Ok(vec![(0, 10, 0)]));

    // Same pixel format, tiled layout: the modifier is part of the match.
    register(&mut engine, 9, XRGB, TILED);
    assert_eq!(
        validate(&mut engine, &[layer(9, 0)]),
        Err(no_plane(0, RejectReasons::FORMAT))
    );
}

#[test]
fn the_plane_accepting_the_modifier_is_chosen() {
    let mut tiled_plane = plane(11, Zpos::Fixed(1), &[]);
    tiled_plane
        .formats
        .push(FormatModifier {
            format: XRGB,
            modifier: TILED,
        })
        .unwrap();
    let mut engine = engine(
        caps(&[plane(10, Zpos::Fixed(0), &[XRGB]), tiled_plane], &[]),
        &[XRGB],
    );
    register(&mut engine, 9, XRGB, TILED);
    assert_eq!(validate(&mut engine, &[layer(9, 0)]), Ok(vec![(0, 11, 1)]));
    assert_eq!(validate(&mut engine, &[layer(1, 0)]), Ok(vec![(0, 10, 0)]));
}

// ---- scaling ----

#[test]
fn scaling_limits_are_exact_per_axis() {
    // A plane scaling by at most 2x either way.
    let scaler = scaling(plane(10, Zpos::Fixed(0), &[XRGB]), 2 * ONE, 2 * ONE);
    let mut engine = engine(caps(&[scaler], &[]), &[XRGB]);

    // (source, destination, accepted)
    let cases = [
        ((1280, 720), (640, 360), true),  // exactly 2x down
        ((1280, 720), (639, 360), false), // just over, width
        ((1280, 720), (640, 359), false), // just over, height
        ((640, 360), (1280, 720), true),  // exactly 2x up
        ((639, 360), (1280, 720), false), // just over, width
        ((640, 359), (1280, 720), false), // just over, height
        ((640, 360), (1280, 719), true),  // mixed, both within limits
        ((1280, 720), (1280, 720), true), // not scaled at all
    ];
    for ((sw, sh), (dw, dh), accepted) in cases {
        let layers = [Layer::new(
            BufferId(1),
            Rect::new(0, 0, sw, sh),
            Rect::new(0, 0, dw, dh),
            0,
        )];
        let expected = if accepted {
            Ok(vec![(0, 10, 0)])
        } else {
            Err(no_plane(0, RejectReasons::SCALING))
        };
        assert_eq!(
            validate(&mut engine, &layers),
            expected,
            "{sw}x{sh} -> {dw}x{dh}"
        );
    }
}

#[test]
fn a_plane_without_scaler_refuses_any_scaling() {
    let mut engine = engine(caps(&[plane(10, Zpos::Fixed(0), &[XRGB])], &[]), &[XRGB]);
    assert_eq!(
        validate(&mut engine, &[window(1, 1279, 720, 0)]),
        Err(no_plane(0, RejectReasons::SCALING))
    );
    // A cropped source shown 1:1 is not scaling.
    let cropped = Layer::new(
        BufferId(1),
        Rect::new(10, 10, 640, 360),
        Rect::new(0, 0, 640, 360),
        0,
    );
    assert_eq!(validate(&mut engine, &[cropped]), Ok(vec![(0, 10, 0)]));
}

#[test]
fn the_scaling_plane_is_chosen_for_the_scaled_layer() {
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(0), &[XRGB]),
                scaling(plane(11, Zpos::Fixed(1), &[XRGB]), ONE, 4 * ONE),
            ],
            &[],
        ),
        &[XRGB; 2],
    );
    // The scaled window is on top, where the scaling plane is.
    assert_eq!(
        validate(&mut engine, &[layer(1, 0), window(2, 640, 360, 1)]),
        Ok(vec![(0, 10, 0), (1, 11, 1)])
    );
    // Scaled below unscaled: only the upper plane scales, and then nothing
    // is left above it.
    assert_eq!(
        validate(&mut engine, &[window(2, 640, 360, 0), layer(1, 1)]),
        Err(no_plane(1, RejectReasons::IN_USE | RejectReasons::Z_ORDER))
    );
}

#[test]
fn source_larger_than_the_plane_accepts_reports_source_size() {
    let limited = PlaneCaps {
        max_source: Some(Size::new(1024, 600)),
        ..plane(10, Zpos::Fixed(0), &[XRGB])
    };
    let mut engine = engine(caps(&[limited], &[]), &[XRGB]);
    assert_eq!(
        validate(&mut engine, &[layer(1, 0)]),
        Err(no_plane(0, RejectReasons::SOURCE_SIZE))
    );
    let small = Layer::new(
        BufferId(1),
        Rect::new(0, 0, 1024, 600),
        Rect::new(0, 0, 1024, 600),
        0,
    );
    assert_eq!(validate(&mut engine, &[small]), Ok(vec![(0, 10, 0)]));
}

// ---- shared resources ----

fn three_scaling_planes(group: ResourceGroup) -> Capabilities {
    caps(
        &[
            scaling(plane(10, Zpos::Fixed(0), &[XRGB]), 2 * ONE, 2 * ONE),
            scaling(plane(11, Zpos::Fixed(1), &[XRGB]), 2 * ONE, 2 * ONE),
            scaling(plane(12, Zpos::Fixed(2), &[XRGB]), 2 * ONE, 2 * ONE),
        ],
        &[group],
    )
}

#[test]
fn a_shared_scaler_serves_one_scaled_layer_at_a_time() {
    let scaler = ResourceGroup {
        consumers: LayerTraits::SCALED,
        planes: 0b111,
        capacity: 1,
    };
    let mut engine = engine(three_scaling_planes(scaler), &[XRGB; 3]);

    // One scaled layer with two unscaled ones is fine.
    assert_eq!(
        validate(
            &mut engine,
            &[layer(1, 0), window(2, 640, 360, 1), layer(3, 2)]
        ),
        Ok(vec![(0, 10, 0), (1, 11, 1), (2, 12, 2)])
    );

    // Two scaled layers need two units. Every plane is tried for the second
    // one: taken, below the first, or out of scaler units.
    let result = validate(
        &mut engine,
        &[window(1, 640, 360, 0), window(2, 640, 360, 1)],
    );
    match result {
        Err(Error::Plan(PlanError::NoPlane { layer: 1, reasons })) => {
            assert!(reasons.contains(RejectReasons::RESOURCE), "{reasons:?}");
            assert!(!reasons.contains(RejectReasons::SCALING), "{reasons:?}");
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_untouched(&engine);
}

#[test]
fn a_shared_scaler_with_two_units_allows_two_but_not_three() {
    let scaler = ResourceGroup {
        consumers: LayerTraits::SCALED,
        planes: 0b111,
        capacity: 2,
    };
    let mut engine = engine(three_scaling_planes(scaler), &[XRGB; 3]);
    assert_eq!(
        validate(
            &mut engine,
            &[window(1, 640, 360, 0), window(2, 640, 360, 1)]
        ),
        Ok(vec![(0, 10, 0), (1, 11, 1)])
    );
    let result = validate(
        &mut engine,
        &[
            window(1, 640, 360, 0),
            window(2, 640, 360, 1),
            window(3, 640, 360, 2),
        ],
    );
    match result {
        Err(Error::Plan(PlanError::NoPlane { layer: 2, reasons })) => {
            assert!(reasons.contains(RejectReasons::RESOURCE), "{reasons:?}");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_group_limiting_enabled_planes_caps_the_layer_count() {
    // Three planes, but the fetch units behind two of them are shared: at
    // most one of those two is enabled at once.
    let any = ResourceGroup {
        consumers: LayerTraits::ANY,
        planes: 0b011,
        capacity: 1,
    };
    let mut engine = engine(three_scaling_planes(any), &[XRGB; 3]);
    assert!(validate(&mut engine, &[layer(1, 0), layer(2, 1)]).is_ok());
    // The second layer skips the shared plane 11 and lands on plane 12, so
    // the third has nothing above it. The report names what the deepest
    // attempt met, not the resource that forced the second layer upward.
    assert_eq!(
        validate(&mut engine, &[layer(1, 0), layer(2, 1), layer(3, 2)]),
        Err(no_plane(2, RejectReasons::IN_USE | RejectReasons::Z_ORDER))
    );
}

// ---- z-order ----

#[test]
fn fixed_zpos_planes_decide_stacking_whatever_the_frame_order() {
    // Plane 10 is the upper plane; frame order and plane order disagree.
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(5), &[XRGB]),
                plane(11, Zpos::Fixed(2), &[XRGB]),
            ],
            &[],
        ),
        &[XRGB; 2],
    );
    assert_eq!(
        validate(&mut engine, &[layer(1, 9), layer(2, 1)]),
        Ok(vec![(1, 11, 2), (0, 10, 5)])
    );
    assert_eq!(
        validate(&mut engine, &[layer(1, 1), layer(2, 9)]),
        Ok(vec![(0, 11, 2), (1, 10, 5)])
    );
}

#[test]
fn equal_z_order_stacks_in_frame_order_later_on_top() {
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(0), &[XRGB]),
                plane(11, Zpos::Fixed(1), &[XRGB]),
            ],
            &[],
        ),
        &[XRGB; 2],
    );
    // Buffer 1 first, buffer 2 on top; and the reverse. Layer indices follow
    // the frame, the planes follow the stacking.
    assert_eq!(
        validate(&mut engine, &[layer(1, 3), layer(2, 3)]),
        Ok(vec![(0, 10, 0), (1, 11, 1)])
    );
    assert_eq!(
        validate(&mut engine, &[layer(2, 3), layer(1, 3)]),
        Ok(vec![(0, 10, 0), (1, 11, 1)])
    );
}

#[test]
fn z_order_a_fixed_plane_cannot_honour_is_reported() {
    // The only plane that takes ARGB is the bottom one.
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(0), &[ARGB]),
                plane(11, Zpos::Fixed(1), &[XRGB]),
            ],
            &[],
        ),
        &[ARGB, XRGB],
    );
    assert_eq!(
        validate(&mut engine, &[layer(2, 0), layer(1, 1)]),
        Err(no_plane(1, RejectReasons::FORMAT | RejectReasons::Z_ORDER))
    );
    assert_eq!(
        validate(&mut engine, &[layer(1, 0), layer(2, 1)]),
        Ok(vec![(0, 10, 0), (1, 11, 1)])
    );
}

#[test]
fn ranged_zpos_planes_get_strictly_increasing_positions() {
    let range = Zpos::Range { min: 0, max: 3 };
    let mut engine = engine(
        caps(
            &[
                plane(10, range, &[XRGB]),
                plane(11, range, &[XRGB]),
                plane(12, range, &[XRGB]),
            ],
            &[],
        ),
        &[XRGB; 3],
    );
    assert_eq!(
        validate(&mut engine, &[layer(1, 0), layer(2, 1), layer(3, 2)]),
        Ok(vec![(0, 10, 0), (1, 11, 1), (2, 12, 2)])
    );
}

#[test]
fn a_zpos_range_too_small_for_the_stack_is_a_z_order_error() {
    let range = Zpos::Range { min: 0, max: 1 };
    let mut engine = engine(
        caps(
            &[
                plane(10, range, &[XRGB]),
                plane(11, range, &[XRGB]),
                plane(12, range, &[XRGB]),
            ],
            &[],
        ),
        &[XRGB; 3],
    );
    assert_eq!(
        validate(&mut engine, &[layer(1, 0), layer(2, 1), layer(3, 2)]),
        Err(no_plane(2, RejectReasons::IN_USE | RejectReasons::Z_ORDER))
    );
}

// ---- alpha, blending, transform, full output ----

#[test]
fn alpha_blend_and_transform_each_report_their_own_reason() {
    // A plane with no constant alpha, no coverage blending, no transforms.
    let mut engine = engine(
        caps(&[plane(10, Zpos::Fixed(0), &[ARGB, XRGB])], &[]),
        &[ARGB, XRGB],
    );
    assert_eq!(validate(&mut engine, &[layer(1, 0)]), Ok(vec![(0, 10, 0)]));

    let all = Layer {
        alpha: 0x4000,
        blend: BlendMode::Coverage,
        transform: Transform::FlipH,
        ..layer(1, 0)
    };
    let cases = [
        (
            Layer {
                alpha: 0x8000,
                ..layer(1, 0)
            },
            RejectReasons::ALPHA,
        ),
        (
            // Constant alpha is refused for an opaque format too.
            Layer {
                alpha: 0x8000,
                ..layer(2, 0)
            },
            RejectReasons::ALPHA,
        ),
        (
            Layer {
                blend: BlendMode::Coverage,
                ..layer(1, 0)
            },
            RejectReasons::BLEND,
        ),
        (
            Layer {
                transform: Transform::FlipV,
                ..layer(1, 0)
            },
            RejectReasons::TRANSFORM,
        ),
        (
            all,
            RejectReasons::ALPHA | RejectReasons::BLEND | RejectReasons::TRANSFORM,
        ),
    ];
    for (case, reasons) in cases {
        assert_eq!(validate(&mut engine, &[case]), Err(no_plane(0, reasons)));
    }
}

#[test]
fn a_blend_mode_goes_to_the_plane_that_has_it() {
    let opaque_only = PlaneCaps {
        blend_modes: BlendMode::Opaque.mask(),
        ..plane(10, Zpos::Fixed(0), &[ARGB])
    };
    let blending = PlaneCaps {
        plane_alpha: true,
        ..plane(11, Zpos::Fixed(1), &[ARGB])
    };
    let mut engine = engine(caps(&[opaque_only, blending], &[]), &[ARGB; 2]);
    let translucent = Layer {
        blend: BlendMode::Premultiplied,
        alpha: 0x8000,
        ..layer(2, 1)
    };
    assert_eq!(
        validate(&mut engine, &[layer(1, 0), translucent]),
        Ok(vec![(0, 10, 0), (1, 11, 1)])
    );
    // Premultiplied at the bottom takes the blending plane, leaving nothing
    // above it for the layer on top.
    let bottom = Layer {
        blend: BlendMode::Premultiplied,
        ..layer(2, 0)
    };
    assert_eq!(
        validate(&mut engine, &[bottom, layer(1, 1)]),
        Err(no_plane(1, RejectReasons::IN_USE | RejectReasons::Z_ORDER))
    );
}

#[test]
fn opaque_formats_need_no_blend_support_but_alpha_formats_do() {
    let none = PlaneCaps {
        blend_modes: 0,
        ..plane(10, Zpos::Fixed(0), &[XRGB, ARGB])
    };
    let mut engine = engine(caps(&[none], &[]), &[XRGB, ARGB]);
    // XRGB carries no alpha, so ignoring it is free.
    assert_eq!(validate(&mut engine, &[layer(1, 0)]), Ok(vec![(0, 10, 0)]));
    // ARGB with Opaque blending still needs the plane to say it can ignore
    // pixel alpha.
    assert_eq!(
        validate(&mut engine, &[layer(2, 0)]),
        Err(no_plane(0, RejectReasons::BLEND))
    );
}

#[test]
fn a_full_output_plane_refuses_a_window() {
    let full = PlaneCaps {
        full_output_only: true,
        kind: PlaneKind::Primary,
        ..plane(10, Zpos::Fixed(0), &[XRGB])
    };
    let mut engine = engine(caps(&[full], &[]), &[XRGB]);
    let small = Layer::new(
        BufferId(1),
        Rect::new(0, 0, 640, 360),
        Rect::new(100, 100, 640, 360),
        0,
    );
    assert_eq!(
        validate(&mut engine, &[small]),
        Err(no_plane(0, RejectReasons::FULL_OUTPUT))
    );
    assert_eq!(validate(&mut engine, &[layer(1, 0)]), Ok(vec![(0, 10, 0)]));
}

// ---- protected / priority layers (§6.7) ----
//
// The core has no "protected" flag on a layer: a priority layer is simply a
// layer of the frame, and the frame is all or nothing. These tests pin that
// down: when the priority layer cannot be placed the whole frame is refused
// and the display keeps what it showed.

fn priority_layer() -> Layer {
    Layer {
        blend: BlendMode::Premultiplied,
        ..layer(2, 100)
    }
}

#[test]
fn a_frame_with_an_unplaceable_priority_layer_is_rejected_whole() {
    // Only one plane: the application and the priority layer cannot both be
    // shown, and neither is silently dropped.
    let mut engine = engine(
        caps(&[plane(10, Zpos::Fixed(0), &[XRGB, ARGB])], &[]),
        &[XRGB, ARGB],
    );
    let frame = [layer(1, 0), priority_layer()];
    assert_eq!(
        engine.present(&Frame {
            output: OUT,
            layers: &frame,
        }),
        Err(no_plane(1, RejectReasons::IN_USE))
    );
    assert_untouched(&engine);
}

#[test]
fn the_same_frame_is_placed_with_the_priority_layer_on_top_when_a_plane_exists() {
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(0), &[XRGB]),
                plane(11, Zpos::Fixed(1), &[ARGB]),
            ],
            &[],
        ),
        &[XRGB, ARGB],
    );
    engine
        .present(&Frame {
            output: OUT,
            layers: &[layer(1, 0), priority_layer()],
        })
        .unwrap();
    engine.backend_mut().vsync();
    assert_eq!(
        engine.backend().scanout(OUT).unwrap(),
        [
            PlaneState {
                plane: PlaneId(10),
                buffer: BufferId(1),
                zpos: 0
            },
            PlaneState {
                plane: PlaneId(11),
                buffer: BufferId(2),
                zpos: 1
            },
        ]
    );
}

#[test]
fn a_priority_layer_whose_plane_is_below_the_application_is_rejected() {
    // The priority layer asks to be on top, but the only plane for its
    // format is the lower one.
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(0), &[ARGB]),
                plane(11, Zpos::Fixed(1), &[XRGB]),
            ],
            &[],
        ),
        &[XRGB, ARGB],
    );
    assert_eq!(
        engine.present(&Frame {
            output: OUT,
            layers: &[layer(1, 0), priority_layer()],
        }),
        Err(no_plane(1, RejectReasons::FORMAT | RejectReasons::Z_ORDER))
    );
    assert_untouched(&engine);
}

#[test]
fn a_refusal_does_not_disturb_what_is_on_screen() {
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(0), &[XRGB]),
                plane(11, Zpos::Fixed(1), &[ARGB]),
            ],
            &[],
        ),
        &[XRGB, ARGB],
    );
    let good = [layer(1, 0), priority_layer()];
    engine
        .present(&Frame {
            output: OUT,
            layers: &good,
        })
        .unwrap();
    engine.backend_mut().vsync();
    let before = engine.backend().scanout(OUT).unwrap().to_vec();
    let commits = engine.backend().commit_count();

    // A frame that loses the priority layer's format is refused.
    register(&mut engine, 5, RGB565, Modifier::LINEAR);
    let bad = [layer(1, 0), layer(5, 100)];
    assert_eq!(
        engine.present(&Frame {
            output: OUT,
            layers: &bad,
        }),
        Err(no_plane(1, RejectReasons::FORMAT))
    );
    assert_eq!(engine.backend().scanout(OUT).unwrap(), &before[..]);
    assert_eq!(engine.backend().commit_count(), commits);
    assert_eq!(engine.backend().pending(OUT), None);
}

#[test]
fn a_hardware_refusal_rejects_the_frame_and_keeps_the_display() {
    // The capability model allows the frame, the hardware does not (an
    // undeclared constraint): BackendRejected, not a weaker frame.
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(0), &[XRGB]),
                plane(11, Zpos::Fixed(1), &[ARGB]),
            ],
            &[],
        ),
        &[XRGB, ARGB],
    );
    engine.backend_mut().set_check_hook(|_, _| false);
    assert_eq!(
        engine.present(&Frame {
            output: OUT,
            layers: &[layer(1, 0), layer(2, 1)],
        }),
        Err(Error::Plan(PlanError::BackendRejected { candidates: 1 }))
    );
    assert_untouched(&engine);
}

// ---- empty frames and malformed layers ----

#[test]
fn an_empty_frame_blanks_the_output_with_no_assignments() {
    let mut engine = engine(caps(&[plane(10, Zpos::Fixed(0), &[XRGB])], &[]), &[XRGB]);
    assert_eq!(validate(&mut engine, &[]), Ok(vec![]));
    engine
        .present(&Frame {
            output: OUT,
            layers: &[layer(1, 0)],
        })
        .unwrap();
    engine.backend_mut().vsync();
    assert_eq!(engine.backend().scanout(OUT).unwrap().len(), 1);
    engine
        .present(&Frame {
            output: OUT,
            layers: &[],
        })
        .unwrap();
    engine.backend_mut().vsync();
    assert_eq!(engine.backend().scanout(OUT).unwrap(), []);
}

#[test]
fn an_empty_frame_is_valid_even_without_planes() {
    let mut engine = engine(caps(&[], &[]), &[XRGB]);
    assert_eq!(validate(&mut engine, &[]), Ok(vec![]));
    assert_eq!(
        validate(&mut engine, &[layer(1, 0)]),
        Err(no_plane(0, RejectReasons::IN_USE))
    );
}

#[test]
fn missing_resources_report_the_exact_variant() {
    let mut engine = engine(caps(&[plane(10, Zpos::Fixed(0), &[XRGB])], &[]), &[XRGB]);

    assert_eq!(
        engine.validate(&Frame {
            output: OutputId(7),
            layers: &[],
        }),
        Err(Error::Plan(PlanError::UnknownOutput(OutputId(7))))
    );
    assert_eq!(
        validate(&mut engine, &[layer(1, 0), layer(99, 1)]),
        Err(Error::Plan(PlanError::UnknownBuffer {
            layer: 1,
            buffer: BufferId(99)
        }))
    );
    let outside = Layer::new(BufferId(1), Rect::new(1, 0, 1280, 720), FULL, 0);
    assert_eq!(
        validate(&mut engine, &[outside]),
        Err(Error::Plan(PlanError::InvalidSource { layer: 0 }))
    );
    let empty_source = Layer::new(BufferId(1), Rect::new(0, 0, 0, 720), FULL, 0);
    assert_eq!(
        validate(&mut engine, &[empty_source]),
        Err(Error::Plan(PlanError::InvalidSource { layer: 0 }))
    );
    let empty_destination = Layer::new(BufferId(1), FULL, Rect::new(0, 0, 1280, 0), 0);
    assert_eq!(
        validate(&mut engine, &[empty_destination]),
        Err(Error::Plan(PlanError::InvalidDestination { layer: 0 }))
    );
    assert_untouched(&engine);
}

#[test]
fn a_layer_partly_off_screen_is_clipped_exactly_or_refused() {
    let mut engine = engine(caps(&[plane(10, Zpos::Fixed(0), &[XRGB])], &[]), &[XRGB]);
    // Unscaled and hanging off the right edge: clipped, source cut to match.
    let shifted = Layer::new(BufferId(1), FULL, Rect::new(100, 0, 1280, 720), 0);
    let plan = engine
        .validate(&Frame {
            output: OUT,
            layers: &[shifted],
        })
        .unwrap();
    assert_eq!(plan.assignments()[0].source, Rect::new(0, 0, 1180, 720));
    assert_eq!(
        plan.assignments()[0].destination,
        Rect::new(100, 0, 1180, 720)
    );
    // Rotated and hanging off the edge: refused rather than guessed.
    let rotated = Layer {
        transform: Transform::Rotate90,
        destination: Rect::new(100, 0, 720, 1280),
        ..layer(1, 0)
    };
    assert_eq!(
        validate(&mut engine, &[rotated]),
        Err(Error::Plan(PlanError::CannotClip { layer: 0 }))
    );
}

// ---- hardware composition is found whenever it exists ----

#[test]
fn backtracking_finds_the_assignment_the_first_choice_misses() {
    // Plane 10 is listed first but is the upper one. The bottom layer (XRGB)
    // fits both planes and the first try, plane 10, leaves nothing above it
    // for the ARGB layer. The search must fall back to plane 11 below.
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(1), &[XRGB, ARGB]),
                plane(11, Zpos::Fixed(0), &[XRGB]),
            ],
            &[],
        ),
        &[XRGB, ARGB],
    );
    assert_eq!(
        validate(&mut engine, &[layer(1, 0), layer(2, 1)]),
        Ok(vec![(0, 11, 0), (1, 10, 1)])
    );
}

#[test]
fn a_backend_refusal_moves_the_search_to_the_next_legal_assignment() {
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(0), &[XRGB]),
                plane(11, Zpos::Fixed(1), &[XRGB]),
                plane(12, Zpos::Fixed(2), &[XRGB]),
            ],
            &[],
        ),
        &[XRGB],
    );
    // The hardware dislikes plane 10 for a reason the model does not state.
    engine
        .backend_mut()
        .set_check_hook(|_, plan| plan.assignments().iter().all(|a| a.plane != PlaneId(10)));
    assert_eq!(validate(&mut engine, &[layer(1, 0)]), Ok(vec![(0, 11, 1)]));
    assert_eq!(engine.backend().check_count(), 2);
}

#[test]
fn every_layer_gets_its_own_plane_and_nothing_is_composed_in_software() {
    // Full hardware composition: one layer per plane, the planner never
    // merges layers, and a committed frame shows each buffer on its plane.
    let mut engine = engine(
        caps(
            &[
                plane(10, Zpos::Fixed(0), &[XRGB]),
                plane(11, Zpos::Fixed(1), &[ARGB]),
                plane(12, Zpos::Fixed(2), &[ARGB]),
            ],
            &[],
        ),
        &[XRGB, ARGB, ARGB],
    );
    let layers = [layer(1, 0), layer(2, 1), layer(3, 2)];
    engine
        .present(&Frame {
            output: OUT,
            layers: &layers,
        })
        .unwrap();
    engine.backend_mut().vsync();
    let shown: Vec<_> = engine
        .backend()
        .scanout(OUT)
        .unwrap()
        .iter()
        .map(|s| (s.plane.0, s.buffer.0, s.zpos))
        .collect();
    assert_eq!(shown, [(10, 1, 0), (11, 2, 1), (12, 3, 2)]);
}
