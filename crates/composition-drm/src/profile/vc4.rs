//! Raspberry Pi vc4 HVS constraints (BCM2711 HVS5, BCM2712 HVS6).
//!
//! Both generations report the driver name `vc4`, so the generation is
//! chosen by the explicit variant (`Vc4Hvs5`, `Vc4Hvs6`), not detected.
//! Tags: **observed <date>** = atomic `TEST_ONLY` or KMS on a board:
//! BCM2711 = reTerminal (CM4), kernel `6.12.47+rpt-rpi-v8`; BCM2712 =
//! Raspberry Pi 5, kernel `6.18.50+rpt-rpi-2712` (the dumps under
//! `crates/composition-drm/tests/data/bcm2711/` and `.../bcm2712/`, and
//! `docs/observed/bcm2712.md`); **source** = read from `raspberrypi/linux`
//! `rpi-6.18.y` at `16ae2649dcfadf9d81445b1177a3191efa3cb82e`
//! (`docs/vc4-hvs-scaling-source.md`), not run; **inferred** = concluded,
//! neither seen nor stated. Neither board runs the pinned kernel, so every
//! source value is unconfirmed for the boards' kernels. Formats, zpos,
//! alpha, blend, rotation and plane reach come from KMS; this table only
//! adds what KMS does not carry.
//!
//! - Scaling on every non-cursor plane, both generations: up to 32x up and
//!   2x down per axis. Observed 2026-11-27 (`scale_probe`, XR24, factors
//!   1..32 up and 0.5/0.75/0.9 down, h, v and both, at half and at full mode
//!   size): every factor was accepted on every tested plane, on BCM2711
//!   primary 79 and overlays 127, 138, 149, 160, 292, and on BCM2712 primary
//!   83 and all 16 shared overlays; no plane class differed. These are the
//!   tool's ladder ends, not rejections, so the real range is wider. The
//!   earlier floor, 2x up and 1/2 down accepted on BCM2712 (observed
//!   2026-11-28), lies inside it. Not wider than the source maxima: no
//!   upscale ratio limit, downscale refused above 31.99998x horizontally
//!   (TPZ field, source `vc4_plane.c:686-690`), same on both generations.
//! - Cursor planes get no scaler: `scale_probe` refuses to test them
//!   (observed 2026-11-27), so their `scaling` stays as KMS reported it
//!   (inferred).
//! - `max_source` on non-cursor planes. HVS5: width 8191, the driver refuses
//!   a wider fetch (source, `SCALER5_POS2_WIDTH_MASK`); height 7680, the
//!   gen-5 mode limit (source), as a source bound inferred. HVS6: 8192 x 8192,
//!   inferred: the driver has no source-size check, but writes `width - 1`
//!   and the line count into 13-bit fields, so a larger source would be
//!   truncated silently (source), and 8192 is also the gen-6 mode limit
//!   (source).
//! - LBM group, both generations: a layer that scales vertically or has a
//!   YCbCr format takes LBM (source: no LBM when both y axes are unscaled,
//!   and multi-plane formats force the chroma y axis to the polyphase
//!   filter, so NV12 needs LBM even at 1:1). HVS5 has 61440 words and 64
//!   handles (source); a 1920-wide layer needs at most 7680 words (16 bits
//!   per pixel, polyphase, any format), so capacity 8 (inferred). HVS6 has
//!   1024 lines (source); a 1920-wide ARGB layer with opaque plane alpha and
//!   polyphase vertical scaling needs 240, the largest of the formats worked
//!   through, so capacity 4 (inferred). The sizes are for a fetched width of
//!   1920 pixels, the widest output recorded: narrower layers fit more, so
//!   the planner can refuse a frame the kernel would accept, and wider ones
//!   fit fewer, so `TEST_ONLY` can refuse a frame the planner allowed
//!   (inferred).
//! - UPM group, HVS6 only: every enabled plane takes one UPM handle per
//!   colour plane, 32 handles, 262144 bytes (source). A 1920-wide linear
//!   ARGB layer needs 15616 bytes, so 16 fit; two-plane NV12 needs two
//!   handles, so 16 fit; capacity 16, every layer counted (inferred).
//!   Three-plane formats (3 handles, 10 layers) and layers wider than 1920
//!   are undercounted and left to `TEST_ONLY` (inferred). With `MAX_PLANES`
//!   = 16 this group cannot bind one frame on its own (inferred).
//! - Modifiers, HVS6: pairs with the Broadcom T-tiled modifier or a SAND64
//!   base modifier are removed, because `vc4_format_mod_supported` rejects
//!   them on gen 6 (source, `vc4_plane.c:2767-2771`). The BCM2712 blob lists
//!   them (observed 2026-11-27, kernel `6.18.50+rpt-rpi-2712`); whether
//!   `TEST_ONLY` accepts them there is not measured (inferred: removing them
//!   risks only a lost choice). HVS5 keeps the KMS list, which matches the
//!   source (source; blob observed 2026-11-27).
//!
//! Not modelled (left to `TEST_ONLY`): dlist words (4064 shared by all
//! outputs, far above one frame's need: source and inferred), memory
//! bandwidth and the HVS clock (no check at commit time on gen 5 and 6:
//! source), LBM, UPM and dlist space held by other outputs (groups describe
//! one output's frame: inferred), the 64 LBM handles (above `MAX_PLANES`:
//! inferred), per-layer LBM/UPM sizes by width and format, YCbCr formats the
//! core does not recognise as YCbCr (the YUV trait covers NV12, NV21, NV16
//! and YUV420 only: inferred), the HVS channel pinning of outputs (source),
//! and `replace_pending`, which stays as reported (unknown: inferred).

use composition_core::fixed::FixedVec;
use composition_core::{
    Capabilities, FormatModifier, LayerTraits, Modifier, PlaneKind, ResourceGroup, ScaleLimits,
    Size,
};

/// Which hardware video scaler generation a profile describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Generation {
    /// BCM2711 (Raspberry Pi 4 family).
    Hvs5,
    /// BCM2712 (Raspberry Pi 5).
    Hvs6,
}

/// Broadcom modifier vendor, the top byte of a modifier.
const VENDOR_BROADCOM: u64 = 0x07;
/// `DRM_FORMAT_MOD_BROADCOM_VC4_T_TILED`.
const T_TILED: Modifier = Modifier(VENDOR_BROADCOM << 56 | 1);
/// Broadcom code of SAND64 (`DRM_FORMAT_MOD_BROADCOM_SAND64`).
const SAND64_CODE: u64 = 3;

/// Whether gen 6 rejects the pair: T-tiled, or SAND64 with any column
/// height (the kernel compares the modifier without its parameter bits).
fn rejected_on_hvs6(pair: &FormatModifier) -> bool {
    let m = pair.modifier.0;
    pair.modifier == T_TILED || (m >> 56 == VENDOR_BROADCOM && m & 0xff == SAND64_CODE)
}

/// Adds the vc4 HVS constraints of `generation` to `caps`.
pub(super) fn apply_vc4(caps: &mut Capabilities, generation: Generation) {
    let max_source = match generation {
        Generation::Hvs5 => Size::new(8191, 7680),
        Generation::Hvs6 => Size::new(8192, 8192),
    };
    for plane in caps.planes.as_mut_slice() {
        if generation == Generation::Hvs6 && plane.formats.as_slice().iter().any(rejected_on_hvs6) {
            let mut kept = FixedVec::new();
            for pair in plane.formats.as_slice() {
                if !rejected_on_hvs6(pair) {
                    // Never more than the list held before.
                    let _ = kept.push(*pair);
                }
            }
            plane.formats = kept;
        }
        if plane.kind != PlaneKind::Cursor {
            plane.scaling = Some(ScaleLimits {
                max_upscale: 32 << 16,
                max_downscale: 2 << 16,
            });
            plane.max_source = Some(max_source);
        }
    }

    // Widened so that a (currently unreachable) count of 32 cannot overflow.
    let all = ((1u64 << caps.planes.len()) - 1) as u32;
    let lbm = LayerTraits::SCALED_V.union(LayerTraits::YUV);
    let groups: &[(LayerTraits, u8)] = match generation {
        Generation::Hvs5 => &[(lbm, 8)],
        Generation::Hvs6 => &[(lbm, 4), (LayerTraits::ANY, 16)],
    };
    for &(consumers, capacity) in groups {
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
