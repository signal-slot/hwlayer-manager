//! The Generic profile leaves capabilities as KMS reported them. The RK3588
//! and vc4 profiles have their tables now; their tests are in
//! `profile_rk3588.rs` and `profile_vc4.rs`.

use composition_core::{Capabilities, PlaneCaps, PlaneId, PlaneKind, ScaleLimits, Size};
use composition_drm::DriverProfile;

fn caps() -> Capabilities {
    let mut caps = Capabilities::default();
    for (i, kind) in [
        PlaneKind::Primary,
        PlaneKind::Overlay,
        PlaneKind::Overlay,
        PlaneKind::Cursor,
    ]
    .into_iter()
    .enumerate()
    {
        let mut plane = PlaneCaps {
            id: PlaneId(30 + i as u32),
            kind,
            outputs: 1,
            ..PlaneCaps::default()
        };
        if i == 1 {
            plane.scaling = Some(ScaleLimits {
                max_upscale: 4 << 16,
                max_downscale: 1 << 16,
            });
            plane.max_source = Some(Size::new(4096, 4096));
        }
        caps.planes.push(plane).unwrap();
    }
    caps.replace_pending = true;
    caps
}

fn unchanged(profile: DriverProfile) {
    let mut c = caps();
    let before = c;
    profile.apply(&mut c);
    assert_eq!(c, before);
    let mut empty = Capabilities::default();
    profile.apply(&mut empty);
    assert_eq!(empty, Capabilities::default());
}

#[test]
fn generic_is_a_no_op() {
    unchanged(DriverProfile::Generic);
}

#[test]
fn default_is_still_generic() {
    assert_eq!(DriverProfile::default(), DriverProfile::Generic);
}
