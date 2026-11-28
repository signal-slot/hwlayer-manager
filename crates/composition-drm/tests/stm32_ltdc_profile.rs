#![forbid(unsafe_code)]
//! Pins the claims `DriverProfile::Stm32Ltdc` makes about the three planes of
//! the STM32MP257F-DK (LTDC, kernel 6.6.129). The fixture is the KMS
//! description as discovery reports it (`docs/observed/stm32mp257.md`,
//! observed 2026-11-27): planes 34 primary, 42 and 49 overlays, one CRTC,
//! zpos mutable 0..=2, plane alpha, blend premultiplied, normal transform.
//!
//! Tags: *observed* = a board session document, *source* = read from the ST
//! driver, *inferred* = concluded. These tests pin what the profile says
//! today; they do not prove hardware behaviour.

use composition_core::{
    BlendMode, Capabilities, FormatModifier, OutputCaps, OutputId, PixelFormat, PlaneCaps, PlaneId,
    PlaneKind, ScaleLimits, Size, Transform, Zpos,
};
use composition_drm::DriverProfile;

fn board() -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OutputId(0),
            size: Size::new(1920, 1080),
            refresh_mhz: 60_000,
        })
        .unwrap();
    for (index, kind) in [PlaneKind::Primary, PlaneKind::Overlay, PlaneKind::Overlay]
        .into_iter()
        .enumerate()
    {
        let mut plane = PlaneCaps {
            id: PlaneId([34, 42, 49][index]),
            kind,
            outputs: 1,
            plane_alpha: true,
            blend_modes: BlendMode::Premultiplied.mask(),
            transforms: Transform::Normal.mask(),
            zpos: Zpos::Range { min: 0, max: 2 },
            ..PlaneCaps::default()
        };
        plane
            .formats
            .push(FormatModifier::linear(PixelFormat::ARGB8888))
            .unwrap();
        caps.planes.push(plane).unwrap();
    }
    caps
}

fn profiled() -> Capabilities {
    let mut caps = board();
    DriverProfile::Stm32Ltdc.apply(&mut caps);
    caps
}

/// Observed 2026-11-27 (`docs/observed/stm32mp257.md`): three planes. The
/// profile keeps all of them and their order.
#[test]
fn keeps_three_planes_in_order() {
    let caps = profiled();
    let planes = caps.planes.as_slice();
    assert_eq!(planes.len(), 3);
    assert_eq!(planes[0].kind, PlaneKind::Primary);
    assert_eq!(planes[1].kind, PlaneKind::Overlay);
    assert_eq!(planes[2].kind, PlaneKind::Overlay);
    let ids: Vec<u32> = planes.iter().map(|p| p.id.0).collect();
    assert_eq!(ids, [34, 42, 49]);
}

/// Scaling: observed 2026-11-27 (`docs/stm32mp257-ltdc-board.md`): planes 34
/// and 42 accept upscales (bound seen up to 1080 on both axes), plane 49
/// only 1.0, all downscales rejected with EINVAL. The `max_upscale` 1080 and
/// "scaling is layers 0 and 1" are the profile's choice (inferred from the
/// one board); downscale 1.0 is also source (`ltdc.c:1585-1600`,
/// `docs/stm32mp257-ltdc-source-full.md`).
#[test]
fn scaling_on_primary_and_first_overlay_only() {
    let caps = profiled();
    let planes = caps.planes.as_slice();
    let claim = Some(ScaleLimits {
        max_upscale: 1080 << 16,
        max_downscale: 1 << 16,
    });
    assert_eq!(planes[0].scaling, claim);
    assert_eq!(planes[1].scaling, claim);
    // Plane 49: not claimed, left as KMS reported it (None).
    assert_eq!(planes[2].scaling, None);
}

/// Straight alpha: observed 2026-11-27 on plane 42
/// (`docs/stm32mp257-ltdc-session-2026-11-28.md`, Q2, CRC); planes 34 and 49
/// are inferred from the source (`docs/stm32mp257-ltdc-source-diff-6.6.md`
/// row 42: same blend factors for every layer). KMS reports premultiplied;
/// the profile replaces it by coverage only.
#[test]
fn blend_is_coverage_only_on_every_plane() {
    for plane in profiled().planes.as_slice() {
        assert_eq!(
            plane.blend_modes,
            BlendMode::Coverage.mask(),
            "{:?}",
            plane.id
        );
    }
}

/// Plane alpha and zpos are not touched by the profile. As KMS reports them
/// they are observed 2026-11-27 (`docs/observed/stm32mp257.md`: alpha
/// 0..65535, zpos mutable 0..=2 on all three). That alpha and zpos *work* on
/// screen is not observed: no picture exists
/// (`docs/stm32mp257-ltdc-board.md`,
/// `docs/stm32mp257-ltdc-session-c-2026-11-28.md`, "not run").
#[test]
fn plane_alpha_and_zpos_stay_as_kms_reported() {
    let before = board();
    let after = profiled();
    for (a, b) in before.planes.as_slice().iter().zip(after.planes.as_slice()) {
        assert!(b.plane_alpha);
        assert_eq!(a.plane_alpha, b.plane_alpha);
        assert_eq!(a.zpos, b.zpos);
        assert_eq!(b.zpos, Zpos::Range { min: 0, max: 2 });
    }
}

/// Transforms and formats stay as KMS reported them: Q6 and NV12 were never
/// run with a picture (inferred;
/// `docs/stm32mp257-ltdc-session-c-2026-11-28.md`). The transform source
/// baseline is `docs/stm32mp257-ltdc-transform-source.md` and the YUV one
/// `docs/stm32mp257-ltdc-yuv-source.md` (source). Also no `max_source`
/// claim: 2048 is a framebuffer limit (source, `drv.c`).
#[test]
fn transforms_formats_and_source_size_unchanged() {
    let before = board();
    let after = profiled();
    for (a, b) in before.planes.as_slice().iter().zip(after.planes.as_slice()) {
        assert_eq!(a.transforms, b.transforms);
        assert_eq!(b.transforms, Transform::Normal.mask());
        assert_eq!(a.formats.as_slice(), b.formats.as_slice());
        assert_eq!(a.outputs, b.outputs);
        assert_eq!(a.full_output_only, b.full_output_only);
        assert_eq!(b.max_source, None);
    }
}

/// No resource groups are claimed: nothing in the sessions shows a shared
/// fetch-unit budget (inferred; `docs/observed/stm32mp257.md`).
#[test]
fn no_resource_groups_and_outputs_unchanged() {
    let before = board();
    let after = profiled();
    assert!(after.resource_groups.as_slice().is_empty());
    assert_eq!(before.outputs.as_slice(), after.outputs.as_slice());
}

/// Applying twice changes nothing more (inferred: the profile only sets
/// fields).
#[test]
fn apply_is_idempotent() {
    let once = profiled();
    let mut twice = profiled();
    DriverProfile::Stm32Ltdc.apply(&mut twice);
    assert_eq!(once, twice);
}

/// A cursor plane, if one were reported, gets no claims. The board has none
/// (observed 2026-11-27), so this is inferred from the profile's own rule.
#[test]
fn cursor_plane_is_left_alone() {
    let mut caps = board();
    let cursor = PlaneCaps {
        id: PlaneId(60),
        kind: PlaneKind::Cursor,
        outputs: 1,
        blend_modes: BlendMode::Premultiplied.mask(),
        ..PlaneCaps::default()
    };
    caps.planes.push(cursor).unwrap();
    DriverProfile::Stm32Ltdc.apply(&mut caps);
    let last = caps.planes.as_slice()[3];
    assert_eq!(last.scaling, None);
    assert_eq!(last.blend_modes, BlendMode::Premultiplied.mask());
}
