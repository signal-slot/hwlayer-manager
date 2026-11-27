//! The test-commit model, the fake kernel of the cross-check (design section 6).
//!
//! [`Model::check`] is a pure function of the parsed [`XCheck`] file and a set
//! of [`PlaneState`] values: it accepts the set or refuses it with a
//! [`Refusal`]. The errno class of a refusal is one of `EINVAL`, `ERANGE`,
//! `ENOSPC`, the three libliftoff treats as "try another allocation".
//!
//! The rules are written here from the file alone. Nothing in this module
//! calls or copies the planner module of `composition-core`; agreement with
//! it must not be circular. The one use of `composition-core` types is the
//! `imx95dpu` profile: its constraints are read back from a `Capabilities`
//! to which `DriverProfile::Imx95Dpu.apply` was applied, never re-typed.
//!
//! Rules (a refusal names the first one that fails):
//!
//! * both settings: the plane exists and is listed once; its output index is
//!   in the plane's `outputs` mask; the framebuffer exists and its
//!   format/modifier is in the plane's list; source and destination are not
//!   empty; the destination lies inside the output (frames are restricted to
//!   layers inside the output, design section 7); `zpos` equals the fixed
//!   value or lies in the range, and enabled planes use distinct `zpos`;
//!   `alpha` other than opaque needs the plane's alpha property; the blend
//!   mode is listed (a plane that lists none accepts [`BlendMode::Opaque`],
//!   KMS value 0); the rotation is exactly one `ROTATE_*` bit plus reflect
//!   bits, all listed (`ROTATE_0` is the property default and always passes);
//! * [`Setting::Full`] only: scaling within the plane's limits (no
//!   `scaling` means equal sizes), source size within `max_source`, a
//!   `full_output` plane covers the whole output, and each group's capacity
//!   holds.
//!
//! Disabled planes (`fb == None`) only have to exist and be unique.

use std::collections::HashSet;

use composition_core::caps::{Capabilities, LayerTraits, PlaneCaps, PlaneKind as CorePlaneKind};
use composition_core::types::PlaneId;
use composition_drm::DriverProfile;

use crate::state::{rotation, BlendMode, PlaneState};
use crate::xcheck::{Consumers, PlaneKind, Profile, ScaleDef, XCheck, ZposDef};

/// Which rules the model enforces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// Everything generic KMS properties can express: no groups, scaling,
    /// source limit or full-output rule.
    Structural,
    /// Everything, with the constraints of `profile` or the explicit lines.
    Full,
}

/// Kernel errno class of a refused test commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Errno {
    /// `EINVAL`: the combination is not valid.
    Einval,
    /// `ERANGE`: a size or scale is out of range.
    Erange,
    /// `ENOSPC`: a shared resource is exhausted.
    Enospc,
}

/// A refused test commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// Errno class.
    pub errno: Errno,
    /// Which rule failed, for reports.
    pub reason: String,
}

fn refuse<T>(errno: Errno, reason: String) -> Result<T, Refusal> {
    Err(Refusal { errno, reason })
}

/// Per-plane data the model checks against, after the profile was applied.
#[derive(Clone, Debug)]
struct PlaneRules {
    index: usize,
    scaling: Option<ScaleDef>,
    max_source: Option<(u32, u32)>,
}

/// A resource group with plane ids.
#[derive(Clone, Debug)]
struct GroupRules {
    consumers: u8,
    planes: Vec<u32>,
    capacity: u32,
}

/// The fake kernel for one file and one setting.
#[derive(Clone, Debug)]
pub struct Model<'a> {
    file: &'a XCheck,
    setting: Setting,
    rules: Vec<PlaneRules>,
    groups: Vec<GroupRules>,
}

impl<'a> Model<'a> {
    /// Builds the model. For `profile imx95dpu` and [`Setting::Full`] the
    /// scaling, source limit and groups come from `DriverProfile::apply`.
    ///
    /// Fails only if the file cannot be put into a `Capabilities` (it
    /// exceeds the engine limits, which the reader already refuses).
    pub fn new(file: &'a XCheck, setting: Setting) -> Result<Self, String> {
        let mut rules: Vec<PlaneRules> = file
            .planes
            .iter()
            .enumerate()
            .map(|(index, p)| PlaneRules {
                index,
                scaling: p.scaling,
                max_source: p.max_source,
            })
            .collect();
        let mut groups: Vec<GroupRules> = file
            .groups
            .iter()
            .map(|g| GroupRules {
                consumers: g.consumers.0,
                planes: g.planes.clone(),
                capacity: g.capacity,
            })
            .collect();
        if setting == Setting::Full && file.profile == Profile::Imx95Dpu {
            let mut caps = Capabilities::default();
            for p in &file.planes {
                let kind = match p.kind {
                    PlaneKind::Primary => CorePlaneKind::Primary,
                    PlaneKind::Overlay => CorePlaneKind::Overlay,
                    PlaneKind::Cursor => CorePlaneKind::Cursor,
                };
                let caps_plane = PlaneCaps {
                    id: PlaneId(p.id),
                    kind,
                    ..PlaneCaps::default()
                };
                caps.planes
                    .push(caps_plane)
                    .map_err(|_| "too many planes for Capabilities".to_string())?;
            }
            DriverProfile::Imx95Dpu.apply(&mut caps);
            for (rule, pc) in rules.iter_mut().zip(caps.planes.iter()) {
                rule.scaling = pc.scaling.map(|s| ScaleDef {
                    up: s.max_upscale,
                    down: s.max_downscale,
                });
                rule.max_source = pc.max_source.map(|s| (s.width, s.height));
            }
            groups = caps
                .resource_groups
                .iter()
                .map(|g| GroupRules {
                    consumers: g.consumers.0,
                    planes: file
                        .planes
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| g.planes & (1u32 << i) != 0)
                        .map(|(_, p)| p.id)
                        .collect(),
                    capacity: u32::from(g.capacity),
                })
                .collect();
        }
        Ok(Self {
            file,
            setting,
            rules,
            groups,
        })
    }

    /// The setting this model enforces.
    pub fn setting(&self) -> Setting {
        self.setting
    }

    /// Accepts or refuses `states` as one test commit on output `output_id`.
    pub fn check(&self, output_id: u32, states: &[PlaneState]) -> Result<(), Refusal> {
        let file = self.file;
        let Some(out_index) = file.outputs.iter().position(|o| o.id == output_id) else {
            return refuse(Errno::Einval, format!("unknown output {output_id}"));
        };
        let out = &file.outputs[out_index];
        let full = self.setting == Setting::Full;

        let mut seen_planes = HashSet::new();
        let mut seen_zpos = HashSet::new();
        // (rule index, traits of the layer on that plane)
        let mut consumers: Vec<(u32, u8)> = Vec::new();

        for st in states {
            let id = st.plane_id;
            let Some(pi) = file.planes.iter().position(|p| p.id == id) else {
                return refuse(Errno::Einval, format!("plane {id}: unknown"));
            };
            if !seen_planes.insert(id) {
                return refuse(Errno::Einval, format!("plane {id}: listed twice"));
            }
            let Some(fb) = st.fb else { continue };
            let plane = &file.planes[pi];
            let rule = &self.rules[pi];
            debug_assert_eq!(rule.index, pi);

            if plane.outputs & (1u32 << out_index) == 0 {
                return refuse(
                    Errno::Einval,
                    format!("plane {id}: cannot reach output {output_id}"),
                );
            }
            let Some(buf) = file.buffers.iter().find(|b| b.id == fb) else {
                return refuse(
                    Errno::Einval,
                    format!("plane {id}: unknown framebuffer {fb}"),
                );
            };
            if !plane
                .formats
                .iter()
                .any(|f| f.fourcc == buf.fourcc && f.modifier == buf.modifier)
            {
                return refuse(
                    Errno::Einval,
                    format!(
                        "plane {id}: format {} modifier {:#x} not listed",
                        fourcc_str(buf.fourcc),
                        buf.modifier
                    ),
                );
            }

            // Geometry. `src` is 16.16, `crtc` whole pixels.
            if st.src.w == 0 || st.src.h == 0 || st.crtc.w == 0 || st.crtc.h == 0 {
                return refuse(
                    Errno::Einval,
                    format!("plane {id}: empty source or destination"),
                );
            }
            let c = st.crtc;
            let inside = c.x >= 0
                && c.y >= 0
                && i64::from(c.x) + i64::from(c.w) <= i64::from(out.width)
                && i64::from(c.y) + i64::from(c.h) <= i64::from(out.height);
            if !inside {
                return refuse(
                    Errno::Einval,
                    format!(
                        "plane {id}: destination outside the {}x{} output",
                        out.width, out.height
                    ),
                );
            }

            // zpos.
            match plane.zpos {
                ZposDef::Fixed(n) if st.zpos != n => {
                    return refuse(
                        Errno::Einval,
                        format!("plane {id}: zpos {} but fixed at {n}", st.zpos),
                    );
                }
                ZposDef::Range { min, max } if st.zpos < min || st.zpos > max => {
                    return refuse(
                        Errno::Einval,
                        format!("plane {id}: zpos {} outside {min}..{max}", st.zpos),
                    );
                }
                _ => {}
            }
            if !seen_zpos.insert(st.zpos) {
                return refuse(
                    Errno::Einval,
                    format!("plane {id}: zpos {} already in use", st.zpos),
                );
            }

            // alpha, blend, rotation.
            if st.alpha != 0xFFFF && !plane.alpha {
                return refuse(
                    Errno::Einval,
                    format!("plane {id}: no alpha property, alpha {:#x}", st.alpha),
                );
            }
            let blend_ok = plane.blend.contains(st.blend)
                || (plane.blend.0 == 0 && st.blend == BlendMode::Opaque);
            if !blend_ok {
                return refuse(
                    Errno::Einval,
                    format!("plane {id}: blend mode {:?} not listed", st.blend),
                );
            }
            let rot_mask = rotation::ROTATE_0
                | rotation::ROTATE_90
                | rotation::ROTATE_180
                | rotation::ROTATE_270;
            let reflect_mask = rotation::REFLECT_X | rotation::REFLECT_Y;
            let rot = st.rotation;
            if rot & !(rot_mask | reflect_mask) != 0 || (rot & rot_mask).count_ones() != 1 {
                return refuse(
                    Errno::Einval,
                    format!("plane {id}: malformed rotation {rot:#x}"),
                );
            }
            if rot & !rotation::ROTATE_0 & !plane.transforms != 0 {
                return refuse(
                    Errno::Einval,
                    format!("plane {id}: rotation {rot:#x} not listed"),
                );
            }

            if !full {
                continue;
            }

            // Scaling: compare per axis after rotation by 90/270, where the
            // source width maps to the destination height.
            let (sw, sh) = if rot & (rotation::ROTATE_90 | rotation::ROTATE_270) != 0 {
                (st.src.h, st.src.w)
            } else {
                (st.src.w, st.src.h)
            };
            if scale_exceeds(rule.scaling, sw, c.w) || scale_exceeds(rule.scaling, sh, c.h) {
                return refuse(
                    Errno::Erange,
                    format!("plane {id}: scaling beyond the plane's limits"),
                );
            }
            if let Some((mw, mh)) = rule.max_source {
                // Whole pixels, a fractional source rounds up.
                let w = (u64::from(st.src.w) + 0xFFFF) >> 16;
                let h = (u64::from(st.src.h) + 0xFFFF) >> 16;
                if w > u64::from(mw) || h > u64::from(mh) {
                    return refuse(
                        Errno::Erange,
                        format!("plane {id}: source {w}x{h} above {mw}x{mh}"),
                    );
                }
            }
            if plane.full_output && (c.x, c.y, c.w, c.h) != (0, 0, out.width, out.height) {
                return refuse(
                    Errno::Einval,
                    format!("plane {id}: must cover the whole output"),
                );
            }

            let mut traits = Consumers::ANY;
            if u64::from(sw) != u64::from(c.w) << 16 {
                traits |= Consumers::SCALED_H;
            }
            if u64::from(sh) != u64::from(c.h) << 16 {
                traits |= Consumers::SCALED_V;
            }
            if is_yuv(buf.fourcc) {
                traits |= Consumers::YUV;
            }
            consumers.push((id, traits));
        }

        if full {
            for (gi, g) in self.groups.iter().enumerate() {
                let used = consumers
                    .iter()
                    .filter(|(id, t)| g.planes.contains(id) && g.consumers & t != 0)
                    .count();
                if used as u64 > u64::from(g.capacity) {
                    return refuse(
                        Errno::Enospc,
                        format!("group {gi}: {used} consumers, capacity {}", g.capacity),
                    );
                }
            }
        }
        Ok(())
    }
}

/// Whether scaling `src16` (16.16 source length) to `dst` pixels is outside
/// `limits` on one axis. `None` means the sizes must be equal.
fn scale_exceeds(limits: Option<ScaleDef>, src16: u32, dst: u32) -> bool {
    let src16 = u128::from(src16);
    let dst = u128::from(dst);
    let one = 1u128 << 16;
    match limits {
        None => src16 != dst * one,
        Some(l) => {
            // upscale factor dst/src <= up/2^16
            let too_big_up = dst * one * one > u128::from(l.up) * src16;
            // downscale factor src/dst <= down/2^16
            let too_big_down = src16 > u128::from(l.down) * dst;
            too_big_up || too_big_down
        }
    }
}

fn fourcc_str(f: [u8; 4]) -> String {
    f.iter().map(|&b| char::from(b)).collect()
}

/// YUV and other non-RGB layouts, written down here (not taken from core).
fn is_yuv(f: [u8; 4]) -> bool {
    const YUV: &[&[u8; 4]] = &[
        b"YUYV", b"YVYU", b"UYVY", b"VYUY", b"AYUV", b"VUY8", b"VUY2", b"XYUV", b"Y210", b"Y212",
        b"Y216", b"Y410", b"Y412", b"Y416", b"NV12", b"NV21", b"NV16", b"NV61", b"NV24", b"NV42",
        b"NV15", b"NV20", b"NV30", b"P010", b"P012", b"P016", b"P210", b"Q410", b"Q401", b"YU08",
        b"YV08", b"YU12", b"YV12", b"YU16", b"YV16", b"YU24", b"YV24", b"XVYU", b"YUV9", b"YVU9",
        b"Y0L0", b"X0L0", b"Y8  ", b"Y10 ",
    ];
    YUV.iter().any(|y| **y == f)
}

// Keep the `LayerTraits` import used: consumer bits are those of core's type
// (documented in `xcheck::Consumers`), checked once here.
const _: () = {
    assert!(Consumers::ANY == LayerTraits::ANY.0);
    assert!(Consumers::SCALED_H == LayerTraits::SCALED_H.0);
    assert!(Consumers::SCALED_V == LayerTraits::SCALED_V.0);
    assert!(Consumers::YUV == LayerTraits::YUV.0);
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Rect, URect};
    use crate::xcheck::read;

    const FULL: Setting = Setting::Full;
    const STRUCT: Setting = Setting::Structural;

    fn fx(v: u32) -> u32 {
        v << 16
    }

    /// Output 1 is 100x80, output 2 is 50x50; `groups` are `group` lines.
    fn file(planes: &str, groups: &str) -> XCheck {
        let text = format!(
            "xcheck 1\norigin synthetic 2026-11-28 model unit test\n\
             output 1 100 80 60000\noutput 2 50 50 60000\n{planes}{groups}\
             buffer 1 100 80 AR24 0x0\nbuffer 2 100 80 NV12 0x0\nbuffer 3 100 80 XR24 0x0\n\
             buffer 4 100 80 AR24 0x1\n"
        );
        read(&text).unwrap_or_else(|e| panic!("{e}\n{text}"))
    }

    fn pl(id: u32, kind: &str, rest: &str) -> String {
        format!(
            "plane {id} kind={kind} outputs=0x1 {rest}\n  format AR24 0x0\n  format NV12 0x0\n  format XR24 0x0\n"
        )
    }

    const BASE: &str =
        "zpos=fixed:0 alpha=0 blend=- transforms=- scaling=none max_source=none full_output=0";

    fn st(id: u32, fb: u32, zpos: i32) -> PlaneState {
        PlaneState {
            plane_id: id,
            fb: Some(fb),
            src: URect {
                x: 0,
                y: 0,
                w: fx(100),
                h: fx(80),
            },
            crtc: Rect {
                x: 0,
                y: 0,
                w: 100,
                h: 80,
            },
            zpos,
            alpha: 0xFFFF,
            blend: BlendMode::Opaque,
            rotation: rotation::ROTATE_0,
        }
    }

    fn sized(mut s: PlaneState, sw: u32, sh: u32, w: u32, h: u32) -> PlaneState {
        s.src = URect {
            x: 0,
            y: 0,
            w: fx(sw),
            h: fx(sh),
        };
        s.crtc = Rect { x: 0, y: 0, w, h };
        s
    }

    fn errno(r: Result<(), Refusal>) -> Errno {
        r.expect_err("expected a refusal").errno
    }

    fn both(f: &XCheck) -> [Model<'_>; 2] {
        [Model::new(f, STRUCT).unwrap(), Model::new(f, FULL).unwrap()]
    }

    fn one_plane(rest: &str) -> XCheck {
        file(&pl(10, "primary", rest), "")
    }

    #[test]
    fn plain_state_is_accepted_in_both_settings() {
        let f = one_plane(BASE);
        for m in both(&f) {
            assert_eq!(m.check(1, &[st(10, 1, 0)]), Ok(()));
        }
    }

    #[test]
    fn empty_set_and_disabled_plane_are_accepted() {
        let f = one_plane(BASE);
        let mut off = st(10, 1, 99);
        off.fb = None;
        for m in both(&f) {
            assert_eq!(m.check(1, &[]), Ok(()));
            // A disabled plane is not checked against zpos or anything else.
            assert_eq!(m.check(1, &[off]), Ok(()));
        }
    }

    #[test]
    fn unknown_or_repeated_plane_and_unknown_output() {
        let f = one_plane(BASE);
        for m in both(&f) {
            assert_eq!(errno(m.check(1, &[st(99, 1, 0)])), Errno::Einval);
            assert_eq!(
                errno(m.check(1, &[st(10, 1, 0), st(10, 1, 0)])),
                Errno::Einval
            );
            assert_eq!(errno(m.check(7, &[st(10, 1, 0)])), Errno::Einval);
        }
    }

    #[test]
    fn plane_must_reach_the_output() {
        let f = one_plane(BASE);
        for m in both(&f) {
            // Output 2 is index 1; the plane's mask is 0x1.
            let mut s = st(10, 1, 0);
            s.crtc = Rect {
                x: 0,
                y: 0,
                w: 50,
                h: 50,
            };
            s.src = URect {
                x: 0,
                y: 0,
                w: fx(50),
                h: fx(50),
            };
            let r = m.check(2, &[s]).unwrap_err();
            assert_eq!(r.errno, Errno::Einval);
            assert!(r.reason.contains("reach"), "{}", r.reason);
        }
    }

    #[test]
    fn format_and_modifier_must_be_listed() {
        let f = one_plane(BASE);
        for m in both(&f) {
            assert_eq!(m.check(1, &[st(10, 3, 0)]), Ok(()));
            // Buffer 4 has a modifier the plane does not list.
            assert_eq!(errno(m.check(1, &[st(10, 4, 0)])), Errno::Einval);
            // Unknown framebuffer.
            assert_eq!(errno(m.check(1, &[st(10, 9, 0)])), Errno::Einval);
        }
        let f2 = file(
            "plane 10 kind=primary outputs=0x1 zpos=fixed:0 alpha=0 blend=- transforms=- scaling=none max_source=none full_output=0\n  format RG16 0x0\n",
            "",
        );
        for m in both(&f2) {
            let r = m.check(1, &[st(10, 1, 0)]).unwrap_err();
            assert!(r.reason.contains("AR24"), "{}", r.reason);
        }
    }

    #[test]
    fn destination_must_be_inside_the_output() {
        let f = one_plane(BASE);
        for m in both(&f) {
            for crtc in [
                Rect {
                    x: -1,
                    y: 0,
                    w: 100,
                    h: 80,
                },
                Rect {
                    x: 0,
                    y: -1,
                    w: 100,
                    h: 80,
                },
                Rect {
                    x: 1,
                    y: 0,
                    w: 100,
                    h: 80,
                },
                Rect {
                    x: 0,
                    y: 1,
                    w: 100,
                    h: 80,
                },
            ] {
                let mut s = st(10, 1, 0);
                s.crtc = crtc;
                assert_eq!(errno(m.check(1, &[s])), Errno::Einval, "{crtc:?}");
            }
            // Empty destination or source.
            let mut s = st(10, 1, 0);
            s.crtc.w = 0;
            assert_eq!(errno(m.check(1, &[s])), Errno::Einval);
            let mut s = st(10, 1, 0);
            s.src.h = 0;
            assert_eq!(errno(m.check(1, &[s])), Errno::Einval);
        }
    }

    #[test]
    fn fixed_zpos_must_match() {
        let f = one_plane(
            "zpos=fixed:3 alpha=0 blend=- transforms=- scaling=none max_source=none full_output=0",
        );
        for m in both(&f) {
            assert_eq!(m.check(1, &[st(10, 1, 3)]), Ok(()));
            assert_eq!(errno(m.check(1, &[st(10, 1, 2)])), Errno::Einval);
        }
    }

    #[test]
    fn range_zpos_must_be_inside() {
        let f = one_plane("zpos=1..4 zpos_now=1 alpha=0 blend=- transforms=- scaling=none max_source=none full_output=0");
        for m in both(&f) {
            for z in [1, 2, 4] {
                assert_eq!(m.check(1, &[st(10, 1, z)]), Ok(()), "{z}");
            }
            for z in [0, 5, -1] {
                assert_eq!(errno(m.check(1, &[st(10, 1, z)])), Errno::Einval, "{z}");
            }
        }
    }

    #[test]
    fn zpos_values_must_be_unique() {
        let planes = format!(
            "{}{}",
            pl(10, "primary", "zpos=0..3 zpos_now=0 alpha=0 blend=- transforms=- scaling=none max_source=none full_output=0"),
            pl(11, "overlay", "zpos=0..3 zpos_now=1 alpha=0 blend=- transforms=- scaling=none max_source=none full_output=0"),
        );
        let f = file(&planes, "");
        for m in both(&f) {
            assert_eq!(m.check(1, &[st(10, 1, 0), st(11, 1, 1)]), Ok(()));
            let r = m.check(1, &[st(10, 1, 2), st(11, 1, 2)]).unwrap_err();
            assert_eq!(r.errno, Errno::Einval);
            assert!(r.reason.contains("zpos"), "{}", r.reason);
            // A disabled plane does not use its zpos.
            let mut off = st(11, 1, 2);
            off.fb = None;
            assert_eq!(m.check(1, &[st(10, 1, 2), off]), Ok(()));
        }
    }

    #[test]
    fn alpha_needs_the_property() {
        let without = one_plane(BASE);
        let with = one_plane(
            "zpos=fixed:0 alpha=1 blend=- transforms=- scaling=none max_source=none full_output=0",
        );
        let mut half = st(10, 1, 0);
        half.alpha = 0x8000;
        for m in both(&without) {
            assert_eq!(errno(m.check(1, &[half])), Errno::Einval);
        }
        for m in both(&with) {
            assert_eq!(m.check(1, &[half]), Ok(()));
        }
    }

    #[test]
    fn blend_mode_must_be_listed() {
        let f = one_plane("zpos=fixed:0 alpha=0 blend=opaque,premul transforms=- scaling=none max_source=none full_output=0");
        let mut s = st(10, 1, 0);
        for m in both(&f) {
            s.blend = BlendMode::Premultiplied;
            assert_eq!(m.check(1, &[s]), Ok(()));
            s.blend = BlendMode::Coverage;
            assert_eq!(errno(m.check(1, &[s])), Errno::Einval);
        }
        // A plane that lists none accepts only the default (opaque).
        let none = one_plane(BASE);
        for m in both(&none) {
            s.blend = BlendMode::Opaque;
            assert_eq!(m.check(1, &[s]), Ok(()));
            s.blend = BlendMode::Premultiplied;
            assert_eq!(errno(m.check(1, &[s])), Errno::Einval);
        }
    }

    #[test]
    fn rotation_must_be_listed() {
        let f = one_plane("zpos=fixed:0 alpha=0 blend=- transforms=normal,rot90,flip_h scaling=none max_source=none full_output=0");
        let mut s = st(10, 1, 0);
        // Square so that rot90 needs no scaling in the Full setting.
        s = sized(s, 80, 80, 80, 80);
        for m in both(&f) {
            s.rotation = rotation::ROTATE_90;
            assert_eq!(m.check(1, &[s]), Ok(()));
            s.rotation = rotation::ROTATE_0 | rotation::REFLECT_X;
            assert_eq!(m.check(1, &[s]), Ok(()));
            s.rotation = rotation::ROTATE_90 | rotation::REFLECT_X;
            assert_eq!(m.check(1, &[s]), Ok(()));
            s.rotation = rotation::ROTATE_180;
            assert_eq!(errno(m.check(1, &[s])), Errno::Einval);
            s.rotation = rotation::ROTATE_0 | rotation::REFLECT_Y;
            assert_eq!(errno(m.check(1, &[s])), Errno::Einval);
            // Malformed: two rotations, none, or an unknown bit.
            s.rotation = rotation::ROTATE_90 | rotation::ROTATE_0;
            assert_eq!(errno(m.check(1, &[s])), Errno::Einval);
            s.rotation = rotation::REFLECT_X;
            assert_eq!(errno(m.check(1, &[s])), Errno::Einval);
            s.rotation = rotation::ROTATE_0 | 1 << 9;
            assert_eq!(errno(m.check(1, &[s])), Errno::Einval);
        }
        // Default rotation passes on a plane that lists nothing.
        let none = one_plane(BASE);
        for m in both(&none) {
            assert_eq!(m.check(1, &[st(10, 1, 0)]), Ok(()));
        }
    }

    const SCALER: &str = "zpos=fixed:0 alpha=0 blend=- transforms=normal,rot90 scaling=up:0x20000/down:0x20000 max_source=none full_output=0";

    #[test]
    fn scaling_is_checked_only_in_full() {
        let f = one_plane(BASE); // scaling=none: sizes must be equal
        let s = sized(st(10, 1, 0), 50, 40, 100, 80);
        let [structural, full] = both(&f);
        assert_eq!(structural.check(1, &[s]), Ok(()));
        let r = full.check(1, &[s]).unwrap_err();
        assert_eq!(r.errno, Errno::Erange);
        assert!(r.reason.contains("scaling"), "{}", r.reason);
        // Equal sizes pass in Full.
        assert_eq!(full.check(1, &[st(10, 1, 0)]), Ok(()));
    }

    #[test]
    fn scaling_limits_are_per_axis_and_inclusive() {
        let f = one_plane(SCALER); // up 2x, down 2x
        let [structural, full] = both(&f);
        // exactly 2x up on both axes, exactly 2x down on both axes
        assert_eq!(
            full.check(1, &[sized(st(10, 1, 0), 50, 40, 100, 80)]),
            Ok(())
        );
        assert_eq!(
            full.check(1, &[sized(st(10, 1, 0), 100, 80, 50, 40)]),
            Ok(())
        );
        // beyond up on width only, beyond down on height only
        assert_eq!(
            errno(full.check(1, &[sized(st(10, 1, 0), 49, 40, 100, 80)])),
            Errno::Erange
        );
        assert_eq!(
            errno(full.check(1, &[sized(st(10, 1, 0), 100, 80, 50, 39)])),
            Errno::Erange
        );
        // Same states pass structurally.
        assert_eq!(
            structural.check(1, &[sized(st(10, 1, 0), 49, 40, 100, 80)]),
            Ok(())
        );
    }

    #[test]
    fn fractional_source_scales_by_its_exact_size() {
        let f = one_plane(SCALER);
        let full = Model::new(&f, FULL).unwrap();
        let mut s = st(10, 1, 0);
        // 49.5 px -> 100 px is more than 2x up.
        s.src.w = fx(49) + 0x8000;
        assert_eq!(errno(full.check(1, &[s])), Errno::Erange);
        // 50.5 px -> 100 px is within 2x up.
        s.src.w = fx(50) + 0x8000;
        assert_eq!(full.check(1, &[s]), Ok(()));
    }

    #[test]
    fn rotation_by_90_swaps_the_scaled_axes() {
        let f = one_plane(SCALER);
        let full = Model::new(&f, FULL).unwrap();
        let mut s = sized(st(10, 1, 0), 40, 80, 80, 40);
        s.rotation = rotation::ROTATE_90;
        // 40x80 rotated is 80x40 = destination: no scaling.
        assert_eq!(full.check(1, &[s]), Ok(()));
        s.rotation = rotation::ROTATE_0;
        // Unrotated, 40x80 -> 80x40 is 2x up in x, 2x down in y: at the limits.
        assert_eq!(full.check(1, &[s]), Ok(()));
        let mut t = sized(st(10, 1, 0), 20, 80, 80, 40);
        t.rotation = rotation::ROTATE_90;
        // 80x20 -> 80x40 is 2x up in y only: fine; 20x80 unrotated is 4x in x.
        assert_eq!(full.check(1, &[t]), Ok(()));
        t.rotation = rotation::ROTATE_0;
        assert_eq!(errno(full.check(1, &[t])), Errno::Erange);
    }

    #[test]
    fn source_limit_is_checked_only_in_full() {
        let f = one_plane("zpos=fixed:0 alpha=0 blend=- transforms=- scaling=up:0x100000/down:0x100000 max_source=64x64 full_output=0");
        let [structural, full] = both(&f);
        let s = sized(st(10, 1, 0), 100, 80, 100, 80);
        assert_eq!(structural.check(1, &[s]), Ok(()));
        let r = full.check(1, &[s]).unwrap_err();
        assert_eq!(r.errno, Errno::Erange);
        assert!(r.reason.contains("source"), "{}", r.reason);
        // At the limit, and one over in each dimension.
        assert_eq!(
            full.check(1, &[sized(st(10, 1, 0), 64, 64, 64, 64)]),
            Ok(())
        );
        assert_eq!(
            errno(full.check(1, &[sized(st(10, 1, 0), 65, 64, 65, 64)])),
            Errno::Erange
        );
        assert_eq!(
            errno(full.check(1, &[sized(st(10, 1, 0), 64, 65, 64, 65)])),
            Errno::Erange
        );
        // A fractional source rounds up: 64.5 px is 65 px.
        let mut frac = sized(st(10, 1, 0), 64, 64, 65, 64);
        frac.src.w = fx(64) + 0x8000;
        assert_eq!(errno(full.check(1, &[frac])), Errno::Erange);
    }

    #[test]
    fn full_output_plane_must_cover_the_output() {
        let f = one_plane("zpos=fixed:0 alpha=0 blend=- transforms=- scaling=up:0x100000/down:0x100000 max_source=none full_output=1");
        let [structural, full] = both(&f);
        let part = sized(st(10, 1, 0), 50, 40, 50, 40);
        assert_eq!(structural.check(1, &[part]), Ok(()));
        let r = full.check(1, &[part]).unwrap_err();
        assert_eq!(r.errno, Errno::Einval);
        assert!(r.reason.contains("whole output"), "{}", r.reason);
        let mut shifted = st(10, 1, 0);
        shifted.crtc.x = 1;
        shifted.crtc.w = 99;
        shifted.src.w = fx(99);
        assert_eq!(errno(full.check(1, &[shifted])), Errno::Einval);
        assert_eq!(full.check(1, &[st(10, 1, 0)]), Ok(()));
        // A scaled whole-output cover is still a cover.
        assert_eq!(
            full.check(1, &[sized(st(10, 1, 0), 50, 40, 100, 80)]),
            Ok(())
        );
    }

    fn group_file(group_lines: &str) -> XCheck {
        let scal = "alpha=0 blend=- transforms=- scaling=up:0x100000/down:0x100000 max_source=none full_output=0";
        let planes = format!(
            "{}{}{}",
            pl(10, "primary", &format!("zpos=fixed:0 {scal}")),
            pl(11, "overlay", &format!("zpos=fixed:1 {scal}")),
            pl(12, "overlay", &format!("zpos=fixed:2 {scal}")),
        );
        file(&planes, group_lines)
    }

    fn half(id: u32, fb: u32, zpos: i32, scaled_w: bool, scaled_h: bool) -> PlaneState {
        let s = st(id, fb, zpos);
        let (w, h) = (
            if scaled_w { 50 } else { 100 },
            if scaled_h { 40 } else { 80 },
        );
        sized(s, w, h, 100, 80)
    }

    #[test]
    fn group_any_counts_every_layer() {
        let f = group_file("group consumers=any planes=10,11 capacity=1\n");
        let [structural, full] = both(&f);
        let two = [st(10, 1, 0), st(11, 1, 1)];
        assert_eq!(structural.check(1, &two), Ok(()));
        let r = full.check(1, &two).unwrap_err();
        assert_eq!(r.errno, Errno::Enospc);
        assert!(r.reason.contains("capacity 1"), "{}", r.reason);
        // The third plane is not in the group; one member plus it is fine.
        assert_eq!(full.check(1, &[st(10, 1, 0), st(12, 1, 2)]), Ok(()));
    }

    #[test]
    fn group_scaled_h_and_scaled_v_count_their_axis() {
        let f = group_file("group consumers=scaled_h planes=10,11,12 capacity=1\n");
        let full = Model::new(&f, FULL).unwrap();
        // Two vertically scaled layers do not use the horizontal pool.
        assert_eq!(
            full.check(
                1,
                &[half(10, 1, 0, false, true), half(11, 1, 1, false, true)]
            ),
            Ok(())
        );
        // One horizontally scaled layer fits, two do not.
        assert_eq!(
            full.check(
                1,
                &[half(10, 1, 0, true, false), half(11, 1, 1, false, false)]
            ),
            Ok(())
        );
        assert_eq!(
            errno(full.check(
                1,
                &[half(10, 1, 0, true, false), half(11, 1, 1, true, true)]
            )),
            Errno::Enospc
        );

        let f = group_file("group consumers=scaled_v planes=10,11,12 capacity=1\n");
        let full = Model::new(&f, FULL).unwrap();
        assert_eq!(
            full.check(
                1,
                &[half(10, 1, 0, true, false), half(11, 1, 1, true, false)]
            ),
            Ok(())
        );
        assert_eq!(
            errno(full.check(
                1,
                &[half(10, 1, 0, false, true), half(11, 1, 1, true, true)]
            )),
            Errno::Enospc
        );
    }

    #[test]
    fn group_with_both_scaled_bits_counts_either_axis() {
        let f = group_file("group consumers=scaled_h,scaled_v planes=10,11,12 capacity=1\n");
        let full = Model::new(&f, FULL).unwrap();
        // One layer scaled in both axes counts once.
        assert_eq!(full.check(1, &[half(10, 1, 0, true, true)]), Ok(()));
        assert_eq!(
            errno(full.check(
                1,
                &[half(10, 1, 0, true, false), half(11, 1, 1, false, true)]
            )),
            Errno::Enospc
        );
    }

    #[test]
    fn group_yuv_counts_yuv_layers() {
        let f = group_file("group consumers=yuv planes=10,11,12 capacity=1\n");
        let full = Model::new(&f, FULL).unwrap();
        assert_eq!(
            full.check(1, &[st(10, 2, 0), st(11, 1, 1), st(12, 3, 2)]),
            Ok(())
        );
        let r = full.check(1, &[st(10, 2, 0), st(11, 2, 1)]).unwrap_err();
        assert_eq!(r.errno, Errno::Enospc);
    }

    #[test]
    fn group_only_counts_enabled_planes_in_the_group() {
        let f = group_file("group consumers=any planes=10,11 capacity=2\n");
        let full = Model::new(&f, FULL).unwrap();
        let mut off = st(11, 1, 1);
        off.fb = None;
        assert_eq!(full.check(1, &[st(10, 1, 0), off, st(12, 1, 2)]), Ok(()));
        assert_eq!(
            full.check(1, &[st(10, 1, 0), st(11, 1, 1), st(12, 1, 2)]),
            Ok(())
        );
        let f = group_file("group consumers=any planes=10,11 capacity=0\n");
        let full = Model::new(&f, FULL).unwrap();
        assert_eq!(errno(full.check(1, &[st(10, 1, 0)])), Errno::Enospc);
    }

    #[test]
    fn several_groups_are_all_enforced() {
        let f = group_file(
            "group consumers=any planes=10,11,12 capacity=3\ngroup consumers=yuv planes=10,11,12 capacity=1\n",
        );
        let full = Model::new(&f, FULL).unwrap();
        assert_eq!(
            full.check(1, &[st(10, 1, 0), st(11, 1, 1), st(12, 2, 2)]),
            Ok(())
        );
        assert_eq!(
            errno(full.check(1, &[st(10, 2, 0), st(11, 2, 1), st(12, 1, 2)])),
            Errno::Enospc
        );
    }

    #[test]
    fn profile_imx95dpu_supplies_the_constraints_in_full_only() {
        let scal = "alpha=0 blend=- transforms=- scaling=none max_source=none full_output=0";
        let mut planes = String::new();
        for i in 0..7u32 {
            planes += &pl(10 + i, "overlay", &format!("zpos=fixed:{i} {scal}"));
        }
        let text = format!(
            "xcheck 1\norigin synthetic 2026-11-28 model unit test\nprofile imx95dpu\n\
             output 1 100 80 60000\n{planes}buffer 1 100 80 AR24 0x0\nbuffer 2 100 80 NV12 0x0\n"
        );
        let f = read(&text).unwrap();
        let [structural, full] = both(&f);

        // Seven unscaled RGB layers: capacity of the ANY group is 6.
        let seven: Vec<_> = (0..7).map(|i| st(10 + i, 1, i as i32)).collect();
        assert_eq!(structural.check(1, &seven), Ok(()));
        assert_eq!(errno(full.check(1, &seven)), Errno::Enospc);
        assert_eq!(full.check(1, &seven[..6]), Ok(()));

        // Scaling comes from the profile: 2048x up is fine, any downscale is not.
        let up = sized(st(10, 1, 0), 1, 1, 100, 80);
        assert_eq!(full.check(1, &[up]), Ok(()));
        let down = sized(st(10, 1, 0), 100, 80, 99, 80);
        assert_eq!(errno(full.check(1, &[down])), Errno::Erange);
        assert_eq!(structural.check(1, &[down]), Ok(()));

        // The scaled/YUV group holds 4: five unscaled YUV layers do not fit.
        let five: Vec<_> = (0..5).map(|i| st(10 + i, 2, i as i32)).collect();
        assert_eq!(errno(full.check(1, &five)), Errno::Enospc);
        assert_eq!(full.check(1, &five[..4]), Ok(()));
        // Group 3 (scaled_h, capacity 1) holds one horizontally scaled layer;
        // the second fails there although the scaled/YUV group would hold it.
        let five: Vec<_> = (0..5)
            .map(|i| sized(st(10 + i, 1, i as i32), 50, 80, 100, 80))
            .collect();
        // The single horizontal scaler: group 3 (scaled_h, capacity 1) holds
        // one layer; the second fails there even though group 2 would hold it.
        assert_eq!(full.check(1, &five[..1]), Ok(()));
        assert_eq!(errno(full.check(1, &five[..2])), Errno::Enospc);
        // One vertical scale is fine, two are not.
        let v: Vec<_> = (0..2)
            .map(|i| sized(st(10 + i, 1, i as i32), 100, 40, 100, 80))
            .collect();
        assert_eq!(full.check(1, &v[..1]), Ok(()));
        assert_eq!(errno(full.check(1, &v)), Errno::Enospc);

        // Source limit 8192: a 1x1 buffer is fine; the test file's buffers
        // are 100x80, so check the model's limit through the constraint data.
        assert_eq!(full.rules[0].max_source, Some((8192, 8192)));
        assert_eq!(structural.rules[0].max_source, None);
        assert!(structural.groups.is_empty());
        assert_eq!(full.groups.len(), 4);
    }

    #[test]
    fn profile_does_not_apply_to_cursor_planes() {
        let text = "xcheck 1\norigin synthetic 2026-11-28 model unit test\nprofile imx95dpu\n\
            output 1 100 80 60000\n\
            plane 10 kind=cursor outputs=0x1 zpos=fixed:0 alpha=0 blend=- transforms=- scaling=none max_source=none full_output=0\n  format AR24 0x0\n\
            buffer 1 100 80 AR24 0x0\n";
        let f = read(text).unwrap();
        let full = Model::new(&f, FULL).unwrap();
        // No scaling limit was given to the cursor plane, so sizes must match.
        assert_eq!(full.rules[0].scaling, None);
        let s = sized(st(10, 1, 0), 50, 40, 100, 80);
        assert_eq!(errno(full.check(1, &[s])), Errno::Erange);
    }

    #[test]
    fn model_is_pure() {
        let f = one_plane(BASE);
        let m = Model::new(&f, FULL).unwrap();
        let s = [st(10, 1, 0)];
        assert_eq!(m.check(1, &s), m.check(1, &s));
        assert_eq!(m.setting(), FULL);
    }

    #[test]
    fn module_does_not_reference_the_planner() {
        let src = include_str!("model.rs");
        let needle = ["composition_core", "::planner"].concat();
        let hits = src.matches(needle.as_str()).count();
        // Only this test's own pattern text is allowed, and it is built from
        // two pieces so it does not match itself.
        assert_eq!(hits, 0);
    }
}
