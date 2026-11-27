//! The RK3588 profile is a placeholder: it leaves capabilities as KMS
//! reported them. The vc4 profiles are tested in `profile_vc4.rs`.

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
fn rk3588_vop2_is_a_no_op() {
    unchanged(DriverProfile::Rk3588Vop2);
}

#[test]
fn default_is_still_generic() {
    assert_eq!(DriverProfile::default(), DriverProfile::Generic);
}
