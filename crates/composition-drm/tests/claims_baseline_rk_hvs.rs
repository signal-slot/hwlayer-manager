//! Baseline: the claims `DriverProfile::Rk3588Vop2`, `Vc4Hvs5` and
//! `Vc4Hvs6` make today that no other test pins to a value. A later
//! profile-tighten change (design §7 of `docs/rk3588-backend-design.md`
//! and `docs/hvs-backend-design.md`) must edit this file visibly.
//!
//! Already pinned, so not repeated here:
//! - `profile_rk3588.rs`: on the recorded NanoPC-T6 planes — scaling
//!   8x up / 8x down and `max_source` 4096x4320 for both window classes;
//!   formats, modifiers, zpos, plane alpha, blend modes and
//!   `replace_pending` unchanged; `transforms == 0` as recorded, and the
//!   planner refusing Rotate90 and FlipV on every class.
//! - `profile_vc4.rs`: scaling 32x up / 2x down and the per-generation
//!   `max_source` (8191x7680, 8192x8192) on the non-cursor planes of both
//!   recorded boards, cursor planes left alone; LBM (8 / 4) and UPM (16)
//!   groups; on substituted recorded planes, bare T-tiled and bare SAND64
//!   removed on HVS6 and SAND128/256 kept, everything kept on HVS5;
//!   scaling accepted at the bounds and refused one step out.
//! - `profile_socs.rs`: the four-layer per-CRTC budget group of
//!   `Rk3588Vop2` — its mask, capacity and the sweep boundary.
//! - `parse_fuzz.rs` and `profile.rs` hold *invariants* on random or
//!   hand-built caps (`replace_pending` survives; a plane changes at most
//!   in scaling, `max_source`, HVS6's formats and LTDC's blend modes;
//!   idempotency) but pin no value, and the cursor exception of
//!   `Rk3588Vop2` only in a comment. `profile.rs` pins "transforms stay as
//!   reported" for `Stm32Ltdc`; the tests here do it for these three.
//!
//! So this file pins: the `Rk3588Vop2` scaling and source claims on the
//! two plane kinds the recorded fixture does not contain (a cursor and a
//! plane with no formats); `Rk3588Vop2`, `Vc4Hvs5` and `Vc4Hvs6` making no
//! transform claim; `replace_pending` on the two recorded vc4 boards and
//! with it set (already pinned for RK by `profile_rk3588.rs`); and the
//! exact modifier pairs `Vc4Hvs6` cuts, SAND64 with parameter bits among
//! them — a shape random fuzz can produce but never name.
//!
//! Tags quote the profile sources (`src/profile/rk3588.rs`,
//! `src/profile/vc4.rs`): **observed <date>** = KMS or `TEST_ONLY` on the
//! board; **source** = read from the pinned kernel tree; **inferred** =
//! concluded. They tag the claim, not the test.

mod common;

use common::recorded::{
    self, vendor_modifier, SAND128, SAND256, SAND32, SAND64, VC4_T_TILED, VENDOR_ARM,
};
use composition_core::{
    Capabilities, FormatModifier, LayerTraits, Modifier, PixelFormat, PlaneCaps, PlaneId,
    PlaneKind, ResourceGroup, ScaleLimits, Size, Transform,
};
use composition_drm::DriverProfile;

// Source: tests/data/bcm2711/probe-raw-2026-11-27.txt, `probe --raw` on the
// reTerminal (CM4, BCM2711, HVS5), kernel 6.12.47+rpt-rpi-v8, recorded
// 2026-11-27 (the same fixture `profile_vc4.rs` loads).
const CM4_PROBE: &str = include_str!("data/bcm2711/probe-raw-2026-11-27.txt");
// Source: tests/data/bcm2712/probe-raw-2026-11-27.txt, `probe --raw` on the
// Raspberry Pi 5 (BCM2712, HVS6), kernel 6.18.50+rpt-rpi-2712, recorded
// 2026-11-27 (the same fixture `profile_vc4.rs` loads).
const PI5_PROBE: &str = include_str!("data/bcm2712/probe-raw-2026-11-27.txt");

/// Scale limits of both RK3588 window classes.
const RK_SCALING: ScaleLimits = ScaleLimits {
    max_upscale: 8 << 16,
    max_downscale: 8 << 16,
};

/// Largest source of both RK3588 window classes.
const RK_MAX_SOURCE: Size = Size {
    width: 4096,
    height: 4320,
};

/// One bit per transform except `Normal`.
const ROT_BITS: u8 = Transform::Rotate90.mask()
    | Transform::Rotate180.mask()
    | Transform::Rotate270.mask()
    | Transform::FlipH.mask()
    | Transform::FlipV.mask();

/// A column-height parameter for a SAND modifier: some non-zero bits above
/// the base code. The profile compares the base modifier only, so the
/// exact SAND layout is not modelled (source, vc4.rs: "the kernel compares
/// the modifier without its parameter bits").
const COL32: u64 = 32 << 8;

fn pair(format: PixelFormat, modifier: u64) -> FormatModifier {
    FormatModifier {
        format,
        modifier: Modifier(modifier),
    }
}

fn plane(id: u32, kind: PlaneKind, formats: &[FormatModifier]) -> PlaneCaps {
    let mut plane = PlaneCaps {
        id: PlaneId(id),
        kind,
        outputs: 1,
        ..PlaneCaps::default()
    };
    for pair in formats {
        plane.formats.push(*pair).unwrap();
    }
    plane
}

fn caps_of<const N: usize>(planes: [PlaneCaps; N]) -> Capabilities {
    let mut caps = Capabilities::default();
    for plane in planes {
        caps.planes.push(plane).unwrap();
    }
    caps
}

fn formats_of(caps: &Capabilities) -> Vec<FormatModifier> {
    caps.planes.as_slice()[0].formats.as_slice().to_vec()
}

#[test]
fn rk3588_claims_scaling_and_source_on_every_plane_it_gets() {
    // Scaling 8x each way, both classes: observed 2026-11-27 (every probed
    // plane accepted 8.0 and refused 8.0039) and source (one `max_scale =
    // 8` bound for every window, V2C:1004-1005). `max_source` 4096x4320:
    // source (`max_input`, REG:2525, checked for every window; not
    // measured on the board). A plane with no formats lists no AFBC
    // modifier, so it is Esmart (source: `Window::of`, else-branch). The
    // cursor keeps the same claim (source: `apply_rk3588_vop2` has no
    // cursor branch; observed 2026-11-27: the recorded topology has no
    // cursor plane, so the cursor line is inferred). `parse_fuzz.rs`
    // records this as a defect with an ignored opposite test — fixing it
    // must edit this test too.
    let afbc = pair(PixelFormat::XRGB8888, vendor_modifier(VENDOR_ARM, 1));
    let linear = pair(PixelFormat::XRGB8888, 0);
    let mut caps = caps_of([
        plane(33, PlaneKind::Primary, &[afbc]),
        plane(34, PlaneKind::Overlay, &[linear]),
        plane(35, PlaneKind::Overlay, &[]),
        plane(36, PlaneKind::Cursor, &[linear]),
    ]);
    DriverProfile::Rk3588Vop2.apply(&mut caps);
    for plane in caps.planes.as_slice() {
        assert_eq!(plane.scaling, Some(RK_SCALING), "plane {}", plane.id.0);
        assert_eq!(
            plane.max_source,
            Some(RK_MAX_SOURCE),
            "plane {}",
            plane.id.0
        );
    }

    // The layer budget counts every plane, cursor included: capacity 4 is
    // observed 2026-11-27 and pinned on the recorded planes by
    // `profile_socs.rs`; the cursor bit in the mask is inferred (the
    // profile has no cursor branch).
    assert_eq!(
        caps.resource_groups.as_slice(),
        &[ResourceGroup {
            consumers: LayerTraits::ANY,
            planes: 0b1111,
            capacity: 4,
        }]
    );
}

#[test]
fn rk3588_adds_no_transform_claim() {
    // Transforms stay as KMS gave them: observed 2026-11-27 (the recorded
    // `rotation` property carries its current value only), while source
    // says Cluster offers rotate 90/270 and reflect X/Y and Esmart reflect
    // Y (REG:1131-1132, REG:1207) — design §4 option (a) is not taken.
    // The profile adds no bit and clears none, on both classes.
    let afbc = pair(PixelFormat::XRGB8888, vendor_modifier(VENDOR_ARM, 1));
    let linear = pair(PixelFormat::XRGB8888, 0);
    let mut caps = caps_of([
        plane(33, PlaneKind::Primary, &[afbc]),
        plane(34, PlaneKind::Overlay, &[linear]),
    ]);
    for plane in caps.planes.as_mut_slice() {
        plane.transforms = ROT_BITS;
    }
    DriverProfile::Rk3588Vop2.apply(&mut caps);
    let masks: Vec<u8> = caps.planes.iter().map(|p| p.transforms).collect();
    assert_eq!(masks, [ROT_BITS, ROT_BITS]);
}

#[test]
fn vc4_adds_no_transform_claim() {
    // Rotation comes from KMS: "Formats, zpos, alpha, blend, rotation and
    // plane reach come from KMS; this table only adds what KMS does not
    // carry" (source of the table, `src/profile/vc4.rs`; the rotation
    // property read from the boards' blobs, observed 2026-11-27). The
    // profile adds no bit and clears none, cursor planes included.
    for profile in [DriverProfile::Vc4Hvs5, DriverProfile::Vc4Hvs6] {
        let linear = pair(PixelFormat::XRGB8888, 0);
        let mut caps = caps_of([
            plane(79, PlaneKind::Primary, &[linear]),
            plane(127, PlaneKind::Overlay, &[linear]),
            plane(677, PlaneKind::Cursor, &[linear]),
        ]);
        for plane in caps.planes.as_mut_slice() {
            plane.transforms = ROT_BITS;
        }
        profile.apply(&mut caps);
        let masks: Vec<u8> = caps.planes.iter().map(|p| p.transforms).collect();
        assert_eq!(masks, [ROT_BITS, ROT_BITS, ROT_BITS], "{profile:?}");
    }
}

#[test]
fn vc4_replace_pending_stays_as_reported() {
    // `replace_pending` "stays as reported (unknown: inferred)"
    // (`src/profile/vc4.rs`). Both boards report false, and the profile
    // neither sets nor clears it — on the recorded boards and on a
    // hand-built description that says true. `profile_rk3588.rs` pins
    // this for `Rk3588Vop2` on its fixture; `parse_fuzz.rs` holds the
    // invariant only under random caps and pins no value.
    let boards: [(DriverProfile, &str); 2] = [
        (DriverProfile::Vc4Hvs5, CM4_PROBE),
        (DriverProfile::Vc4Hvs6, PI5_PROBE),
    ];
    for (profile, probe) in boards {
        let mut caps = recorded::load(probe).expect("fixture loads").caps;
        assert!(!caps.replace_pending, "{profile:?}: boards report false");
        profile.apply(&mut caps);
        assert!(!caps.replace_pending, "{profile:?} set replace_pending");
        caps.replace_pending = true;
        profile.apply(&mut caps);
        assert!(caps.replace_pending, "{profile:?} cleared replace_pending");
    }
}

#[test]
fn vc4_modifier_lists_after_the_profile() {
    // Modifiers, HVS6: "pairs with the Broadcom T-tiled modifier or a
    // SAND64 base modifier are removed, because `vc4_format_mod_supported`
    // rejects them on gen 6" (source, `vc4_plane.c:2767-2771`; the
    // BCM2712 blob lists them, observed 2026-11-27). HVS5 keeps the KMS
    // list (source; blob observed 2026-11-27). Whether `TEST_ONLY`
    // accepts them on gen 6 is not measured (inferred).
    //
    // "Base modifier" is SAND64 with its parameter bits ignored; the
    // T-tiled pair is matched by full equality (source: `pair.modifier ==
    // T_TILED`), and a T-tiled with bits above the code — which the DRM
    // spec does not define — is kept today (inferred: never observed).
    // Kept pairs stay in KMS order.
    let list = [
        pair(PixelFormat::XRGB8888, 0),
        pair(PixelFormat::XRGB8888, vendor_modifier(VENDOR_ARM, 1)),
        pair(PixelFormat::XRGB8888, vendor_modifier(VENDOR_ARM, 3)),
        pair(PixelFormat::NV12, SAND64),
        pair(PixelFormat::NV12, SAND64 | COL32),
        pair(PixelFormat::XRGB8888, SAND128 | COL32),
        pair(PixelFormat::NV12, SAND32),
        pair(PixelFormat::XRGB8888, VC4_T_TILED),
        pair(PixelFormat::XRGB8888, VC4_T_TILED | (1 << 8)),
        pair(PixelFormat::XRGB8888, SAND256),
    ];

    // HVS5 removes nothing, whatever the modifier.
    let mut caps = caps_of([plane(79, PlaneKind::Primary, &list)]);
    DriverProfile::Vc4Hvs5.apply(&mut caps);
    assert_eq!(formats_of(&caps), list);

    // HVS6 removes exactly the T-tiled pair and both SAND64 pairs; the
    // vendor check keeps the Arm pair whose low byte is 3.
    let mut caps = caps_of([plane(83, PlaneKind::Primary, &list)]);
    DriverProfile::Vc4Hvs6.apply(&mut caps);
    assert_eq!(
        formats_of(&caps),
        [list[0], list[1], list[2], list[5], list[6], list[8], list[9]]
    );
}
