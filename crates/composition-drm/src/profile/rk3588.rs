//! Rockchip RK3588 VOP2 constraints (driver `rockchip`, mainline).
//!
//! Tags: **observed <date>** = through KMS or `TEST_ONLY` on the NanoPC-T6
//! (kernel `6.18.55-current-rockchip64`, data in
//! `crates/composition-drm/tests/data/rk3588/`); **source** = read from
//! mainline v6.18 (`docs/rk3588-mainline-source.md`,
//! `docs/rk3588-vop2-limits-source.md`, `docs/rk3588-dt-source.md`; files
//! `rockchip_drm_vop2.c` = `V2C`, `rockchip_vop2_reg.c` = `REG`), or from
//! vendor develop-6.1 @ 77168c8d5ab82399f65a80e9f807b50ba37cf483 where it says
//! so; **inferred** = concluded, neither seen nor stated. Precedence: board
//! measurement, then mainline v6.18, then vendor 6.1. The vendor figures of
//! `docs/rk3588-backend-design.md` §4 (Cluster 4x, 4096x7680, AFBC-only
//! rotation) belong to that other kernel and are not used.
//!
//! - Eight windows, four Cluster and four Esmart, each one KMS plane: observed
//!   2026-11-27 (8 planes), source (`REG:1121-1268`). Every plane keeps the
//!   outputs mask, zpos range 0..=7, plane alpha and blend modes KMS reported
//!   (observed 2026-11-27); the profile does not touch them.
//! - Window class: a plane whose formats carry an Arm (vendor `0x08`, AFBC)
//!   modifier is Cluster, any other is Esmart. Observed 2026-11-27: every
//!   Cluster pair is AFBC, no Esmart pair is; source: only Cluster rows use
//!   `format_modifiers_afbc` (`REG:203-239`). KMS names neither class, and
//!   plane type or object id do not tell it (two Cluster planes are
//!   overlays, observed 2026-11-27), so ids are never used. The first-32
//!   cut keeps linear pairs first; a Cluster plane would be taken for Esmart
//!   only if it listed 32 or more linear pairs, which mainline Cluster does
//!   not (no `LINEAR` in its list: source; observed 2026-11-27: 0 linear of
//!   126). The limits below are the same for both classes on this kernel,
//!   so a wrong class changes nothing today (inferred).
//! - Scaling, both classes, up to 8x per axis: observed 2026-11-27 (every
//!   probed plane, Cluster and Esmart, accepted 8.0 on h, v and both and
//!   refused 8.0039 with ERANGE) and source (`max_scale = 8`, `V2C:1004-1005`,
//!   one bound for every window). `max_upscale` = `8 << 16`.
//! - Downscaling up to 8x per axis (`max_downscale` = `8 << 16`): source
//!   (`min_scale = 1/8`, `V2C:1004-1005`). Observed 2026-11-27 only down to
//!   0.5 (2x), accepted on every probed plane; below 0.5 was not measured.
//! - The per-window table factors (Cluster 4, Esmart 8, `REG:1136-1137`,
//!   `REG:1212-1213`) are read by no code (source), so the kernel accepts up
//!   to 8x on Cluster too, and so did the board (observed 2026-11-27). Picture
//!   quality of a Cluster window scaled beyond 4x was not looked at; it is
//!   **not modelled** (inferred: `TEST_ONLY` cannot catch it either).
//! - Plane 39 (a Cluster primary that cannot reach the connected CRTC) was not
//!   scaled on the board (observed 2026-11-27); it gets the Cluster values
//!   by class (inferred).
//! - `max_source` 4096x4320 on both classes: source (`max_input`,
//!   `REG:2525`, checked in `V2C:1042-1051` for every window, no per-class
//!   limit). Not measured on the board. The vendor 4096x7680 is not used.
//! - Transforms: none reported, `transforms` stays as KMS gave it (the DRM
//!   backend reports no rotation). Source says Cluster offers rotate 90/270
//!   and reflect X/Y and Esmart reflect Y (`REG:1131-1132`, `REG:1207`),
//!   but the board's `rotation` property was recorded with its current value
//!   only (rotate-0), not with its supported bits (observed 2026-11-27), so
//!   design §4 option (a) is not taken until a dump lists them.
//! - Formats and modifiers stay as KMS listed them: Cluster 14 formats under
//!   9 AFBC modifiers (126 pairs, no linear, no NV12), Esmart 21 linear
//!   formats including NV12 (observed 2026-11-27). Only the first 32 pairs of
//!   a Cluster plane are kept by the backend (`MAX_FORMATS`), observed
//!   2026-11-27 in the probe output; the profile does not change the cut.
//! - No resource groups. Each window has its own scaler registers, so no
//!   shared scaler: source. The layer budget is not modelled: the board
//!   data shows no budget below the plane count (observed 2026-11-27: no
//!   plane-combination sweep yet). Source gives `nlayers = 8 / nvps` per CRTC
//!   (`V2C:2483`, checked at `V2C:1832`), which with both HDMI ports in the
//!   NanoPC-T6 device tree would be 4 planes per CRTC (inferred, DT not read);
//!   until a `TEST_ONLY` sweep shows it, a frame with more than 4 planes on
//!   one output may pass the planner and be refused by the kernel.
//!
//! Not modelled (source, left to `TEST_ONLY`): the 4x4 minimum source and
//! destination, odd source x with a YUV format, AFBC width rounded down to a
//! multiple of 4, the per-port output limits (VP0-VP2 4096x2304, VP3
//! 2048x1536), the per-CRTC layer count above, and planes already used by the
//! other CRTC (the planner sees one output at a time).
//!
//! The kernel check may therefore still refuse a frame this profile allows;
//! it must not accept one the profile refuses, except through the inferred
//! items above.

use composition_core::{Capabilities, PlaneCaps, ScaleLimits, Size};

/// Arm modifier vendor (AFBC), in the top byte of a modifier.
const VENDOR_ARM: u64 = 0x08;

/// VOP2 window class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Window {
    Cluster,
    Esmart,
}

impl Window {
    /// Cluster if the plane lists any AFBC modifier.
    fn of(plane: &PlaneCaps) -> Self {
        if plane
            .formats
            .iter()
            .any(|f| f.modifier.0 >> 56 == VENDOR_ARM)
        {
            Window::Cluster
        } else {
            Window::Esmart
        }
    }

    /// Scaling limits and largest source of the class.
    const fn limits(self) -> (ScaleLimits, Size) {
        let scaling = ScaleLimits {
            max_upscale: 8 << 16,
            max_downscale: 8 << 16,
        };
        let max_source = Size::new(4096, 4320);
        match self {
            Window::Cluster => (scaling, max_source),
            Window::Esmart => (scaling, max_source),
        }
    }
}

/// Adds the RK3588 VOP2 constraints to `caps`.
pub(super) fn apply_rk3588_vop2(caps: &mut Capabilities) {
    for plane in caps.planes.as_mut_slice() {
        let (scaling, max_source) = Window::of(plane).limits();
        plane.scaling = Some(scaling);
        plane.max_source = Some(max_source);
    }
}
