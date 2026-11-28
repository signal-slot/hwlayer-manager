//! The next MCU candidates of SoW §5.4 as `Capabilities`, planned on the SoW
//! WP0 scenes. Desk study: nothing here was observed on hardware, and no
//! board, driver or device was used.
//!
//! Every value below is **inferred from the source docs** of this repository,
//! not measured: [`docs/rt1170-lcdifv2-source.md`], [`docs/ra8p1-glcdc-source.md`]
//! and [`docs/stm32h7-ltdc-source.md`]. Those docs tag each of their rows
//! `read from source` or `inferred`; a model value built from such a row is
//! at best as certain as the row, and where a row says "open" the value here
//! is a stated guess. Where a guess is made, a `GUESS:` comment says so.
//! The tests pin what the planner and `advise` do on these shapes. They do
//! not show what a real RT1170, RA8P1 or STM32H7 does.
//!
//! Candidates (output sizes are only the shape of the test; the panels are
//! the ones named in the docs or, for the RA8P1, the size the doc used in its
//! memory arithmetic):
//!
//! - `rt1170_elcdif`: one plane, no alpha, no scaler, 720x1280.
//! - `rt1170_lcdifv2`: eight rectangle layers, per-layer alpha and blend,
//!   packed YUV 4:2:2, two colour-space converters, no scaler, 720x1280.
//! - `ra8p1_glcdc`: two layers, per-pixel alpha only, no YUV, no scaler,
//!   1280x800.
//! - `stm32h7_ltdc`: two layers, constant and per-pixel alpha, no YUV, no
//!   scaler, 480x272.
//!
//! The scenes are the SoW WP0 shapes, built the same way for every
//! candidate: (1) two UI layers; (2) video + UI; (3) UI + a translucent
//! priority layer on top; (4) unsupported geometry, a scaled layer or a layer
//! out of bounds. The outcome of each is asserted exactly: the plane
//! assignment, the composition fallback of `advise`, or the error.
//!
//! The tests rely on one shared reading: the layer stack order is fixed by
//! the hardware (the RT1170 doc leaves open which index is on top). The
//! planes below use `Zpos::Fixed(index)`.
//!
//! FINDING: what the model cannot express (no core source was changed).
//! Each item says which candidate it concerns and where the doc has it.
//!
//! - FINDING 1 (all): the latch. `Capabilities::replace_pending` is a single
//!   bool. The docs describe a per-layer shadow load on LCDIFv2 (applied at
//!   the next frame's VSYNC), a per-layer valid bit on GLCDC (a second write
//!   is refused, not queued) and a global reload on LTDC (immediate `IMR` or
//!   at vertical blanking `VBR`). The model cannot say which of these a
//!   backend uses, that two layers update atomically only if staged in the
//!   same frame, or that Zephyr's `display_stm32_ltdc` takes `IMR` from the
//!   line-0 interrupt. `replace_pending` is `false` for all four here; that
//!   is the safe reading, not a statement from the docs.
//! - FINDING 2 (RA8P1, STM32H7): the background plane. Both have a background
//!   colour below layer 1 (GLCDC `BG`, LTDC `BCCR`). The model has no
//!   background colour, so "layer 1 may be a window and the rest is the
//!   background" is stated only by `full_output_only: false`.
//! - FINDING 3 (RA8P1, STM32H7, LCDIFv2): CLUT. The 256-entry palettes (L8,
//!   AL44, AL88, GLCDC CLUT1/4/8, LCDIFv2 LUT 1/2/4/8 bpp) are indexed
//!   formats. The planner's formats are fourcc pairs and the palette is
//!   neither a format nor a resource; no CLUT format is in the lists below.
//! - FINDING 4 (RA8P1): the fade ramp. A timed alpha ramp per layer is the
//!   only constant alpha path of GLCDC. `plane_alpha: bool` cannot say "a
//!   ramp, not a value", so `plane_alpha` is `false`.
//! - FINDING 5 (RA8P1, STM32H7): chroma key. The GLCDC API has the types but
//!   `R_GLCDC_ColorKeySet` returns `FSP_ERR_UNSUPPORTED`; the LTDC has it in
//!   registers. The model has no colour key.
//! - FINDING 6 (RT1170): the 2D helper. The PXP (scale, rotate, blend,
//!   memory to memory) and the DMA2D (blend, format conversion, YCbCr in)
//!   sit outside the scan-out path. `advise` composes into a buffer the
//!   caller fills, but the model cannot say that the caller has a helper
//!   that can do the scaling or the YUV conversion a rejected layer needs.
//!   So a composed layer is a `Blocker`, never a promise that it can be
//!   composed.
//! - FINDING 7 (LCDIFv2): the layer-to-layer limits. The doc reads
//!   `FSL_FEATURE_LCDIFV2_LAYER_CSC_COUNT (2)`; which layers own the two
//!   converters is open. The model has a `ResourceGroup` with consumer
//!   `LayerTraits::YUV` and capacity 2 over all eight planes, which is the
//!   most the doc supports; per-plane YUV formats would be a guess.
//! - FINDING 8 (all): memory. 64-byte alignment (LCDIFv2, GLCDC), D-cache
//!   clean before scan-out, and the SDRAM or OCRAM placement are not in the
//!   model at all; `BufferLookup` carries only size, format and modifier.
//! - FINDING 9 (all): `Zpos::Fixed` for the order, but the docs name the
//!   order only as "layer 2 over layer 1" (LTDC, GLCDC); for the RT1170 it is
//!   open, so the tests would pass unchanged if the order were reversed
//!   (plane ids would swap), which is not tested here.
//! - FINDING 10 (STM32H7, RA8P1): transforms. The registers read have no
//!   rotation and no reflection (LTDC) or only a flipped buffer (GLCDC
//!   `line_descending_enable`, vertical only); the model's `transforms` mask
//!   is `Normal` only here. The GLCDC vertical flip is not modelled, it would
//!   be `Transform::FlipV` and the docs do not tag it as a plane transform.
//! - FINDING 11 (LCDIFv2): Porter-Duff. 12 operators and a global-alpha mode
//!   are reduced to `BlendMode::{Opaque, Coverage, Premultiplied}` and a
//!   bool; the other operators cannot be requested.
//! - FINDING 12 (Zephyr): through Zephyr's display API each controller is one
//!   layer (the LCDIFv2 driver drives layer 0, `display_renesas_ra` layer 1,
//!   `display_stm32_ltdc` layer 1). The `Capabilities` below describe the
//!   silicon as the docs read it, not what Zephyr exposes today; with the
//!   Zephyr drivers as they are, each candidate would be the one-plane
//!   `rt1170_elcdif` shape.

#![forbid(unsafe_code)]

use composition_core::planner::plan;
use composition_core::{
    advise, AdviseOptions, BlendMode, Blocker, BufferId, BufferInfo, BufferLookup, Capabilities,
    ComposeTarget, FormatModifier, Frame, Layer, LayerTraits, Modifier, OutputCaps, OutputId,
    PixelFormat, Placement, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect, RejectReasons,
    ResourceGroup, Size, Transform, Verdict, Zpos,
};

/// 24-bit packed RGB (DRM `RG24`).
const RGB888: PixelFormat = PixelFormat::fourcc(b"RG24");
/// 16-bit ARGB 1:5:5:5 (DRM `AR15`).
const ARGB1555: PixelFormat = PixelFormat::fourcc(b"AR15");
/// 16-bit ARGB 4:4:4:4 (DRM `AR12`).
const ARGB4444: PixelFormat = PixelFormat::fourcc(b"AR12");

const OUT: OutputId = OutputId(0);

struct Buffers(Vec<(BufferId, BufferInfo)>);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        self.0.iter().find(|(id, _)| *id == buffer).map(|(_, i)| *i)
    }
}

fn buf(id: u32, w: u32, h: u32, format: PixelFormat) -> (BufferId, BufferInfo) {
    (
        BufferId(id),
        BufferInfo {
            size: Size::new(w, h),
            format,
            modifier: Modifier::LINEAR,
        },
    )
}

/// One candidate: its capabilities, output size and the buffer the caller
/// would compose into when `advise` falls back.
struct Candidate {
    caps: Capabilities,
    size: Size,
    compose: ComposeTarget,
}

const OPAQUE: u8 = 1 << BlendMode::Opaque as u8;
const COVERAGE: u8 = 1 << BlendMode::Coverage as u8;
const PREMULTIPLIED: u8 = 1 << BlendMode::Premultiplied as u8;

fn plane(
    id: u32,
    kind: PlaneKind,
    z: i32,
    formats: &[PixelFormat],
    plane_alpha: bool,
    blend_modes: u8,
) -> PlaneCaps {
    let mut p = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs: 1,
        scaling: None,
        plane_alpha,
        blend_modes,
        transforms: Transform::Normal.mask(),
        zpos: Zpos::Fixed(z),
        ..PlaneCaps::default()
    };
    for f in formats {
        p.formats.push(FormatModifier::linear(*f)).unwrap();
    }
    p
}

fn candidate(
    size: Size,
    planes: &[PlaneCaps],
    groups: &[ResourceGroup],
    compose: PixelFormat,
) -> Candidate {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size,
            refresh_mhz: 60_000,
        })
        .unwrap();
    for p in planes {
        caps.planes.push(*p).unwrap();
    }
    for g in groups {
        caps.resource_groups.push(*g).unwrap();
    }
    Candidate {
        caps,
        size,
        compose: ComposeTarget {
            format: compose,
            modifier: Modifier::LINEAR,
            blend: BlendMode::Opaque,
        },
    }
}

/// eLCDIF: one buffer, no alpha, no scaler (rt1170 doc section 2.1).
///
/// GUESS: `full_output_only` is `true`. The doc says eLCDIF scans out one
/// buffer and has no layer rectangle call (`fsl_elcdif.h:404-409`), so the
/// buffer is the whole panel; the doc does not state it as a rule.
/// Formats are Zephyr's list (section 2.1): RGB565, XRGB8888, RGB888.
fn rt1170_elcdif() -> Candidate {
    let mut p = plane(
        1,
        PlaneKind::Primary,
        0,
        &[PixelFormat::RGB565, PixelFormat::XRGB8888, RGB888],
        false,
        OPAQUE,
    );
    p.full_output_only = true;
    candidate(Size::new(720, 1280), &[p], &[], PixelFormat::XRGB8888)
}

/// LCDIFv2: 8 layers, rectangle windows, per-layer alpha and blend, packed
/// YUV 4:2:2, no scaler (rt1170 doc section 2.2), 2 colour-space converters.
///
/// GUESS: `Zpos::Fixed(index)`, layer 0 at the bottom. The doc says which
/// index is on top is open.
/// GUESS: all eight layers take every format. The doc lists one format set
/// for the layer type, not per layer.
/// GUESS: all three blend modes. The doc lists "embedded" (straight pixel
/// alpha) and Porter-Duff with a colour mode; premultiplied is read from the
/// latter, not stated.
/// GUESS: `full_output_only` is `false` (layers have offset and size).
fn rt1170_lcdifv2() -> Candidate {
    let formats = [
        PixelFormat::ARGB8888,
        PixelFormat::ABGR8888,
        PixelFormat::RGB565,
        RGB888,
        ARGB1555,
        ARGB4444,
        PixelFormat::YUYV,
        PixelFormat::UYVY,
    ];
    let blends = OPAQUE | COVERAGE | PREMULTIPLIED;
    let planes: Vec<PlaneCaps> = (0..8u32)
        .map(|i| {
            let kind = if i == 0 {
                PlaneKind::Primary
            } else {
                PlaneKind::Overlay
            };
            plane(i + 1, kind, i as i32, &formats, true, blends)
        })
        .collect();
    // FINDING 7: two converters, which layers own them is open.
    let csc = ResourceGroup {
        consumers: LayerTraits::YUV,
        planes: 0xFF,
        capacity: 2,
    };
    candidate(Size::new(720, 1280), &planes, &[csc], PixelFormat::ARGB8888)
}

/// GLCDC: two layers over a background, fixed order, per-pixel alpha only,
/// no YUV, no scaler (ra8p1 doc sections 2 and 3).
///
/// GUESS: `Coverage` blend (straight alpha) for both layers; the doc names
/// the hardware blend modes only as transparent / non-transparent /
/// on-lower-layer, private to `r_glcdc.c`.
/// GUESS: input format `RGB888` (32 bit) is `XRGB8888`.
fn ra8p1_glcdc() -> Candidate {
    let formats = [
        PixelFormat::ARGB8888,
        PixelFormat::XRGB8888,
        PixelFormat::RGB565,
        ARGB1555,
        ARGB4444,
    ];
    let blends = OPAQUE | COVERAGE;
    let planes = [
        plane(1, PlaneKind::Primary, 0, &formats, false, blends),
        plane(2, PlaneKind::Overlay, 1, &formats, false, blends),
    ];
    candidate(Size::new(1280, 800), &planes, &[], PixelFormat::XRGB8888)
}

/// LTDC: two layers over a background, fixed order, constant alpha and
/// per-pixel alpha, no YUV, no scaler (stm32h7 doc section 2).
///
/// GUESS: all three blend modes. The HAL accepts two values for each of the
/// two blending factors (`CA`, `PAxCA`), and the combinations read as
/// straight and premultiplied alpha; the blend equation is in RM0433, which
/// was not read.
fn stm32h7_ltdc() -> Candidate {
    let formats = [
        PixelFormat::ARGB8888,
        RGB888,
        PixelFormat::RGB565,
        ARGB1555,
        ARGB4444,
    ];
    let blends = OPAQUE | COVERAGE | PREMULTIPLIED;
    let planes = [
        plane(1, PlaneKind::Primary, 0, &formats, true, blends),
        plane(2, PlaneKind::Overlay, 1, &formats, true, blends),
    ];
    candidate(Size::new(480, 272), &planes, &[], PixelFormat::ARGB8888)
}

struct Scene {
    layers: Vec<Layer>,
    buffers: Buffers,
}

fn ui(id: u32, w: u32, h: u32, at: Rect, z: i32) -> Layer {
    Layer {
        blend: BlendMode::Coverage,
        ..Layer::new(BufferId(id), Rect::new(0, 0, w, h), at, z)
    }
}

/// Scene 1: a full-screen UI layer and a smaller UI window on top (ARGB).
fn two_ui(c: &Candidate) -> Scene {
    let (w, h) = (c.size.width, c.size.height);
    Scene {
        layers: vec![
            Layer::new(BufferId(1), Rect::new(0, 0, w, h), Rect::new(0, 0, w, h), 0),
            ui(2, w / 3, h / 3, Rect::new(10, 10, w / 3, h / 3), 1),
        ],
        buffers: Buffers(vec![
            buf(1, w, h, PixelFormat::ARGB8888),
            buf(2, w / 3, h / 3, PixelFormat::ARGB8888),
        ]),
    }
}

/// Scene 2: a half-size packed-YUV video window below a full-screen UI.
fn video_ui(c: &Candidate) -> Scene {
    let (w, h) = (c.size.width, c.size.height);
    Scene {
        layers: vec![
            Layer::new(
                BufferId(1),
                Rect::new(0, 0, w / 2, h / 2),
                Rect::new(0, 0, w / 2, h / 2),
                0,
            ),
            ui(2, w, h, Rect::new(0, 0, w, h), 1),
        ],
        buffers: Buffers(vec![
            buf(1, w / 2, h / 2, PixelFormat::YUYV),
            buf(2, w, h, PixelFormat::ARGB8888),
        ]),
    }
}

/// Scene 3: a full-screen UI and a translucent priority layer on top
/// (constant alpha one half, straight pixel alpha).
fn ui_priority(c: &Candidate) -> Scene {
    let (w, h) = (c.size.width, c.size.height);
    let mut s = two_ui(c);
    s.layers[1].alpha = 0x8000;
    s.layers[1].destination = Rect::new(10, 10, w / 3, h / 3);
    s
}

/// Scene 4a: the second layer is scaled 2x (a quarter-size source shown at
/// half size).
fn scaled(c: &Candidate) -> Scene {
    let (w, h) = (c.size.width, c.size.height);
    Scene {
        layers: vec![
            Layer::new(BufferId(1), Rect::new(0, 0, w, h), Rect::new(0, 0, w, h), 0),
            ui(2, w / 4, h / 4, Rect::new(10, 10, w / 2, h / 2), 1),
        ],
        buffers: Buffers(vec![
            buf(1, w, h, PixelFormat::ARGB8888),
            buf(2, w / 4, h / 4, PixelFormat::ARGB8888),
        ]),
    }
}

/// Scene 4b: the second layer reaches 10 pixels past the right edge.
fn out_of_bounds(c: &Candidate) -> Scene {
    let (w, h) = (c.size.width, c.size.height);
    let mut s = two_ui(c);
    s.layers[1].destination = Rect::new(w as i32 - 10, 10, w / 3, h / 3);
    s
}

/// What `advise` says about one layer.
#[derive(Debug, PartialEq)]
enum Slot {
    /// On the plane with this id.
    Plane(u32),
    /// Composed by the caller, with the blocker and the reasons.
    Compose(Blocker, RejectReasons),
    Hidden,
}

#[derive(Debug, PartialEq)]
struct Summary {
    direct: bool,
    promoted: usize,
    run: Option<(usize, usize)>,
    compose_plane: Option<u32>,
    layers: Vec<Slot>,
}

fn summary(c: &Candidate, s: &Scene, with_target: bool) -> Summary {
    let frame = Frame {
        output: OUT,
        layers: &s.layers,
    };
    let options = AdviseOptions {
        compose: with_target.then_some(c.compose),
        ..AdviseOptions::default()
    };
    let a = advise(&frame, &c.caps, &s.buffers, &options, |_, _| {
        Verdict::Accept
    })
    .unwrap();
    Summary {
        direct: a.is_direct(),
        promoted: a.promoted(),
        run: a.compose_run(),
        compose_plane: a.compose().map(|x| x.plane.0),
        layers: a
            .layers()
            .iter()
            .map(|l| match l.placement {
                Placement::Plane(x) => Slot::Plane(x.plane.0),
                Placement::Compose => Slot::Compose(l.blocker, l.reasons),
                Placement::Hidden => Slot::Hidden,
            })
            .collect(),
    }
}

/// `(plane id, zpos)` of each assignment, in layer order.
fn planned(c: &Candidate, s: &Scene) -> Result<Vec<(u32, i32)>, PlanError> {
    let frame = Frame {
        output: OUT,
        layers: &s.layers,
    };
    plan(&frame, &c.caps, &s.buffers).map(|p| {
        p.assignments()
            .iter()
            .map(|a| (a.plane.0, a.zpos))
            .collect()
    })
}

/// Both layers on planes 1 and 2, bottom to top, no composition.
fn direct_summary() -> Summary {
    Summary {
        direct: true,
        promoted: 2,
        run: None,
        compose_plane: None,
        layers: vec![Slot::Plane(1), Slot::Plane(2)],
    }
}

fn r(reasons: &[RejectReasons]) -> RejectReasons {
    reasons.iter().fold(RejectReasons::NONE, |acc, x| acc | *x)
}

const IN_USE: RejectReasons = RejectReasons::IN_USE;

/// One plane: both layers are composed into the only plane.
fn one_plane_compose(first: RejectReasons, second: RejectReasons) -> Summary {
    Summary {
        direct: false,
        promoted: 0,
        run: Some((0, 1)),
        compose_plane: Some(1),
        layers: vec![
            Slot::Compose(Blocker::Static, first),
            Slot::Compose(Blocker::Static, second),
        ],
    }
}

// ---------------------------------------------------------------------------
// Scene 1: two UI layers
// ---------------------------------------------------------------------------

#[test]
fn scene1_two_ui_elcdif_has_no_second_plane() {
    let c = rt1170_elcdif();
    let s = two_ui(&c);
    assert_eq!(
        planned(&c, &s),
        Err(PlanError::NoPlane {
            layer: 1,
            reasons: IN_USE
        })
    );
    // ARGB8888 is not in Zephyr's eLCDIF list, and a window or a blend is
    // beyond the plane; with a composition target both layers are composed
    // into the one plane.
    assert_eq!(
        summary(&c, &s, true),
        one_plane_compose(
            RejectReasons::FORMAT,
            r(&[
                RejectReasons::FORMAT,
                RejectReasons::BLEND,
                RejectReasons::FULL_OUTPUT
            ])
        )
    );
}

#[test]
fn scene1_two_ui_on_the_multi_layer_candidates_is_direct() {
    for (name, c) in [
        ("lcdifv2", rt1170_lcdifv2()),
        ("glcdc", ra8p1_glcdc()),
        ("ltdc", stm32h7_ltdc()),
    ] {
        let s = two_ui(&c);
        // Lowest z on the lowest fixed plane; the window is clipped to
        // nothing because it is inside the output.
        assert_eq!(planned(&c, &s), Ok(vec![(1, 0), (2, 1)]), "{name}");
        assert_eq!(summary(&c, &s, false), direct_summary(), "{name}");
        assert_eq!(summary(&c, &s, true), direct_summary(), "{name}");
    }
}

#[test]
fn scene1_lcdifv2_uses_the_next_planes_for_more_layers() {
    let c = rt1170_lcdifv2();
    // Eight layers fill the eight planes in order; a ninth has none.
    let scene = |count: u32| Scene {
        layers: (0..count)
            .map(|i| ui(i + 1, 64, 64, Rect::new(i as i32 * 10, 0, 64, 64), i as i32))
            .collect(),
        buffers: Buffers(
            (0..count)
                .map(|i| buf(i + 1, 64, 64, PixelFormat::ARGB8888))
                .collect(),
        ),
    };
    let expect: Vec<(u32, i32)> = (0..8).map(|i| (i + 1, i as i32)).collect();
    assert_eq!(planned(&c, &scene(8)), Ok(expect));
    let nine = scene(9);
    assert_eq!(
        planned(&c, &nine),
        Err(PlanError::NoPlane {
            layer: 8,
            reasons: IN_USE
        })
    );
}

// ---------------------------------------------------------------------------
// Scene 2: video + UI
// ---------------------------------------------------------------------------

#[test]
fn scene2_video_ui_lcdifv2_takes_packed_yuv_directly() {
    let c = rt1170_lcdifv2();
    let s = video_ui(&c);
    assert_eq!(planned(&c, &s), Ok(vec![(1, 0), (2, 1)]));
    assert_eq!(summary(&c, &s, true), direct_summary());
}

#[test]
fn scene2_video_ui_lcdifv2_has_no_nv12() {
    let c = rt1170_lcdifv2();
    let mut s = video_ui(&c);
    let (w, h) = (c.size.width, c.size.height);
    s.buffers.0[0] = buf(1, w / 2, h / 2, PixelFormat::NV12);
    assert_eq!(
        planned(&c, &s),
        Err(PlanError::NoPlane {
            layer: 0,
            reasons: RejectReasons::FORMAT
        })
    );
}

#[test]
fn scene2_video_ui_lcdifv2_has_two_yuv_converters() {
    // FINDING 7: three packed-YUV layers need three converters; two exist.
    let c = rt1170_lcdifv2();
    let layers: Vec<Layer> = (0..3u32)
        .map(|i| {
            Layer::new(
                BufferId(i + 1),
                Rect::new(0, 0, 160, 90),
                Rect::new(i as i32 * 170, 0, 160, 90),
                i as i32,
            )
        })
        .collect();
    let buffers = Buffers(
        (0..3)
            .map(|i| buf(i + 1, 160, 90, PixelFormat::YUYV))
            .collect(),
    );
    let s = Scene { layers, buffers };
    let p = planned(&c, &s);
    assert!(
        matches!(
            p,
            Err(PlanError::NoPlane { layer: 2, reasons }) if reasons.contains(RejectReasons::RESOURCE)
        ),
        "{p:?}"
    );
}

#[test]
fn scene2_video_ui_without_yuv_composes_the_video() {
    for (name, c) in [("glcdc", ra8p1_glcdc()), ("ltdc", stm32h7_ltdc())] {
        let s = video_ui(&c);
        assert_eq!(
            planned(&c, &s),
            Err(PlanError::NoPlane {
                layer: 0,
                reasons: RejectReasons::FORMAT
            }),
            "{name}"
        );
        // The video layer is composed into plane 1 (the caller converts the
        // YUV, FINDING 6); the UI stays on plane 2 above it.
        assert_eq!(
            summary(&c, &s, true),
            Summary {
                direct: false,
                promoted: 1,
                run: Some((0, 0)),
                compose_plane: Some(1),
                layers: vec![
                    Slot::Compose(Blocker::Static, RejectReasons::FORMAT),
                    Slot::Plane(2),
                ],
            },
            "{name}"
        );
    }
}

#[test]
fn scene2_video_ui_elcdif_composes_everything() {
    let c = rt1170_elcdif();
    let s = video_ui(&c);
    assert_eq!(
        planned(&c, &s),
        Err(PlanError::NoPlane {
            layer: 1,
            reasons: IN_USE
        })
    );
    assert_eq!(
        summary(&c, &s, true),
        one_plane_compose(
            r(&[RejectReasons::FORMAT, RejectReasons::FULL_OUTPUT]),
            r(&[RejectReasons::FORMAT, RejectReasons::BLEND])
        )
    );
}

// ---------------------------------------------------------------------------
// Scene 3: UI + translucent priority layer on top
// ---------------------------------------------------------------------------

#[test]
fn scene3_priority_layer_with_constant_alpha() {
    // LCDIFv2 and LTDC have a constant plane alpha: direct.
    for (name, c) in [("lcdifv2", rt1170_lcdifv2()), ("ltdc", stm32h7_ltdc())] {
        let s = ui_priority(&c);
        assert_eq!(planned(&c, &s), Ok(vec![(1, 0), (2, 1)]), "{name}");
        assert_eq!(summary(&c, &s, true), direct_summary(), "{name}");
    }
}

#[test]
fn scene3_priority_layer_glcdc_has_no_constant_alpha() {
    // FINDING 4: the fade ramp is not a plane alpha, so the translucent
    // layer is composed into the upper plane; the UI stays on plane 1.
    let c = ra8p1_glcdc();
    let s = ui_priority(&c);
    assert_eq!(
        planned(&c, &s),
        Err(PlanError::NoPlane {
            layer: 1,
            reasons: RejectReasons::ALPHA
        })
    );
    assert_eq!(
        summary(&c, &s, true),
        Summary {
            direct: false,
            promoted: 1,
            run: Some((1, 1)),
            compose_plane: Some(2),
            layers: vec![
                Slot::Plane(1),
                Slot::Compose(Blocker::Static, RejectReasons::ALPHA),
            ],
        }
    );
    // An opaque-alpha priority layer (per-pixel alpha only) is direct: this
    // is scene 1.
    let s = two_ui(&c);
    assert_eq!(planned(&c, &s), Ok(vec![(1, 0), (2, 1)]));
}

#[test]
fn scene3_priority_layer_elcdif_composes_everything() {
    let c = rt1170_elcdif();
    let s = ui_priority(&c);
    assert_eq!(
        summary(&c, &s, true),
        one_plane_compose(
            RejectReasons::FORMAT,
            r(&[
                RejectReasons::FORMAT,
                RejectReasons::ALPHA,
                RejectReasons::BLEND,
                RejectReasons::FULL_OUTPUT
            ])
        )
    );
}

// ---------------------------------------------------------------------------
// Scene 4: unsupported geometry
// ---------------------------------------------------------------------------

#[test]
fn scene4_scaled_layer_is_refused_by_every_candidate() {
    for (name, c) in [
        ("lcdifv2", rt1170_lcdifv2()),
        ("glcdc", ra8p1_glcdc()),
        ("ltdc", stm32h7_ltdc()),
    ] {
        let s = scaled(&c);
        assert_eq!(
            planned(&c, &s),
            Err(PlanError::NoPlane {
                layer: 1,
                reasons: RejectReasons::SCALING
            }),
            "{name}"
        );
        // The unscaled UI stays on plane 1; the scaled layer is composed
        // into plane 2 (the caller scales, FINDING 6).
        assert_eq!(
            summary(&c, &s, true),
            Summary {
                direct: false,
                promoted: 1,
                run: Some((1, 1)),
                compose_plane: Some(2),
                layers: vec![
                    Slot::Plane(1),
                    Slot::Compose(Blocker::Static, RejectReasons::SCALING),
                ],
            },
            "{name}"
        );
        // Without a composition target nothing is promoted.
        let a = summary(&c, &s, false);
        assert!(!a.direct, "{name}");
        assert_eq!(a.compose_plane, None, "{name}");
    }
}

#[test]
fn scene4_scaled_layer_elcdif_composes_everything() {
    let c = rt1170_elcdif();
    let s = scaled(&c);
    assert_eq!(
        summary(&c, &s, true),
        one_plane_compose(
            RejectReasons::FORMAT,
            r(&[
                RejectReasons::FORMAT,
                RejectReasons::SCALING,
                RejectReasons::BLEND,
                RejectReasons::FULL_OUTPUT
            ])
        )
    );
}

#[test]
fn scene4_layer_past_the_edge_is_clipped_exactly() {
    // An unscaled layer reaching 10 pixels past the right edge is clipped to
    // a 10-pixel-wide window; the source keeps its origin.
    for (name, c) in [
        ("lcdifv2", rt1170_lcdifv2()),
        ("glcdc", ra8p1_glcdc()),
        ("ltdc", stm32h7_ltdc()),
    ] {
        let s = out_of_bounds(&c);
        let (w, h) = (c.size.width, c.size.height);
        let frame = Frame {
            output: OUT,
            layers: &s.layers,
        };
        let p = plan(&frame, &c.caps, &s.buffers).unwrap();
        assert_eq!(p.assignments().len(), 2, "{name}");
        assert_eq!(
            p.assignments()[1].source,
            Rect::new(0, 0, 10, h / 3),
            "{name}"
        );
        assert_eq!(
            p.assignments()[1].destination,
            Rect::new(w as i32 - 10, 10, 10, h / 3),
            "{name}"
        );
        assert_eq!(summary(&c, &s, true), direct_summary(), "{name}");
    }
}

#[test]
fn scene4_layer_wholly_off_the_output_is_hidden() {
    let c = stm32h7_ltdc();
    let mut s = two_ui(&c);
    s.layers[1].destination = Rect::new(c.size.width as i32 + 20, 10, 160, 90);
    assert_eq!(planned(&c, &s), Ok(vec![(1, 0)]));
    assert_eq!(
        summary(&c, &s, true),
        Summary {
            direct: true,
            promoted: 1,
            run: None,
            compose_plane: None,
            layers: vec![Slot::Plane(1), Slot::Hidden],
        }
    );
}

#[test]
fn scene4_scaled_layer_past_the_edge_cannot_be_clipped() {
    // A 2x layer 101 pixels off the left edge would need half a source
    // pixel; this is refused before any plane is looked at.
    let c = rt1170_lcdifv2();
    let (w, h) = (c.size.width, c.size.height);
    let s = Scene {
        layers: vec![ui(1, w / 4, h / 4, Rect::new(-101, 0, w / 2, h / 2), 0)],
        buffers: Buffers(vec![buf(1, w / 4, h / 4, PixelFormat::ARGB8888)]),
    };
    assert_eq!(planned(&c, &s), Err(PlanError::CannotClip { layer: 0 }));
    // advise blames the clip, not a plane; with a target it composes the
    // layer into plane 1, without one it composes nothing.
    assert_eq!(
        summary(&c, &s, true),
        Summary {
            direct: false,
            promoted: 0,
            run: Some((0, 0)),
            compose_plane: Some(1),
            layers: vec![Slot::Compose(Blocker::Clip, RejectReasons::NONE)],
        }
    );
    assert_eq!(summary(&c, &s, false).compose_plane, None);
}

// ---------------------------------------------------------------------------
// The single-plane controller, the model's one-plane edge
// ---------------------------------------------------------------------------

#[test]
fn elcdif_takes_one_full_screen_xrgb_layer_directly() {
    let c = rt1170_elcdif();
    let (w, h) = (c.size.width, c.size.height);
    let s = Scene {
        layers: vec![Layer::new(
            BufferId(1),
            Rect::new(0, 0, w, h),
            Rect::new(0, 0, w, h),
            0,
        )],
        buffers: Buffers(vec![buf(1, w, h, PixelFormat::XRGB8888)]),
    };
    assert_eq!(planned(&c, &s), Ok(vec![(1, 0)]));
    // A window is not accepted by a full-output-only plane.
    let w2 = Scene {
        layers: vec![Layer::new(
            BufferId(1),
            Rect::new(0, 0, w / 2, h / 2),
            Rect::new(0, 0, w / 2, h / 2),
            0,
        )],
        buffers: Buffers(vec![buf(1, w / 2, h / 2, PixelFormat::XRGB8888)]),
    };
    assert_eq!(
        planned(&c, &w2),
        Err(PlanError::NoPlane {
            layer: 0,
            reasons: RejectReasons::FULL_OUTPUT
        })
    );
    // A layer larger than the output, clipped, covers the output: accepted.
    let big = Scene {
        layers: vec![Layer::new(
            BufferId(1),
            Rect::new(0, 0, w + 10, h + 10),
            Rect::new(-5, -5, w + 10, h + 10),
            0,
        )],
        buffers: Buffers(vec![buf(1, w + 10, h + 10, PixelFormat::XRGB8888)]),
    };
    assert_eq!(planned(&c, &big), Ok(vec![(1, 0)]));
}

#[test]
fn two_layer_candidates_compose_a_third_layer() {
    // Three UI layers on two planes: the planner refuses; advise composes the
    // lower two and keeps the top layer on plane 2.
    for (name, c) in [("glcdc", ra8p1_glcdc()), ("ltdc", stm32h7_ltdc())] {
        let (w, h) = (c.size.width, c.size.height);
        let layers = vec![
            Layer::new(BufferId(1), Rect::new(0, 0, w, h), Rect::new(0, 0, w, h), 0),
            ui(2, 64, 64, Rect::new(10, 10, 64, 64), 1),
            ui(3, 64, 64, Rect::new(100, 10, 64, 64), 2),
        ];
        let buffers = Buffers(vec![
            buf(1, w, h, PixelFormat::ARGB8888),
            buf(2, 64, 64, PixelFormat::ARGB8888),
            buf(3, 64, 64, PixelFormat::ARGB8888),
        ]);
        let s = Scene { layers, buffers };
        assert_eq!(
            planned(&c, &s),
            Err(PlanError::NoPlane {
                layer: 2,
                reasons: IN_USE
            }),
            "{name}"
        );
        // The two lower layers are composed together into plane 1 (a
        // contiguous run); the top layer stays on plane 2.
        assert_eq!(
            summary(&c, &s, true),
            Summary {
                direct: false,
                promoted: 1,
                run: Some((0, 1)),
                compose_plane: Some(1),
                layers: vec![
                    Slot::Compose(Blocker::Combination, RejectReasons::NONE),
                    Slot::Compose(Blocker::Combination, RejectReasons::NONE),
                    Slot::Plane(2),
                ],
            },
            "{name}"
        );
    }
}
