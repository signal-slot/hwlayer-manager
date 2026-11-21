//! Capability model: what the display hardware can actually do.
//!
//! A backend fills this in from what it measured or was told by the driver.
//! The planner trusts it and never assumes anything that is not stated here.

use crate::fixed::FixedVec;
use crate::limits::{MAX_FORMATS, MAX_OUTPUTS, MAX_PLANES, MAX_RESOURCE_GROUPS};
use crate::types::{Modifier, OutputId, PixelFormat, PlaneId, Size};

/// Role of a plane as reported by the hardware or driver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PlaneKind {
    /// The main plane of an output.
    #[default]
    Primary,
    /// An additional plane.
    Overlay,
    /// A small plane intended for a pointer image.
    Cursor,
}

/// A pixel format together with a layout modifier a plane accepts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FormatModifier {
    /// Pixel format.
    pub format: PixelFormat,
    /// Layout modifier.
    pub modifier: Modifier,
}

impl FormatModifier {
    /// A format with linear layout.
    pub const fn linear(format: PixelFormat) -> Self {
        Self {
            format,
            modifier: Modifier::LINEAR,
        }
    }
}

/// Scaling limits of a plane as 16.16 fixed-point factors.
///
/// `0x1_0000` means a factor of 1. A plane that cannot scale has no
/// `ScaleLimits` at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ScaleLimits {
    /// Largest enlargement, destination size divided by source size.
    pub max_upscale: u32,
    /// Largest reduction, source size divided by destination size.
    pub max_downscale: u32,
}

/// Stacking position of a plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Zpos {
    /// The position is fixed by the hardware.
    Fixed(i32),
    /// The position can be set anywhere in `min..=max`.
    Range {
        /// Lowest position.
        min: i32,
        /// Highest position.
        max: i32,
    },
}

impl Default for Zpos {
    fn default() -> Self {
        Zpos::Fixed(0)
    }
}

/// Capabilities of one hardware plane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaneCaps {
    /// Backend-defined identifier.
    pub id: PlaneId,
    /// Role of the plane.
    pub kind: PlaneKind,
    /// Outputs the plane can be attached to, as a bit mask over the indices
    /// of [`Capabilities::outputs`].
    pub outputs: u32,
    /// Accepted format/modifier pairs.
    pub formats: FixedVec<FormatModifier, MAX_FORMATS>,
    /// Scaling limits, or `None` if source and destination sizes must match.
    pub scaling: Option<ScaleLimits>,
    /// Largest accepted source size, or `None` if unlimited.
    pub max_source: Option<Size>,
    /// Whether a constant plane alpha other than opaque is supported.
    pub plane_alpha: bool,
    /// Supported blend modes, a mask of [`crate::BlendMode::mask`] bits.
    pub blend_modes: u8,
    /// Supported transforms, a mask of [`crate::Transform::mask`] bits.
    pub transforms: u8,
    /// Stacking position.
    pub zpos: Zpos,
    /// Whether the plane must cover the whole output when enabled.
    pub full_output_only: bool,
}

/// Properties of a layer that decide which shared resources it needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LayerTraits(pub u8);

impl LayerTraits {
    /// Set for every layer: the layer occupies a plane.
    pub const ANY: Self = Self(1 << 0);
    /// The layer is scaled horizontally.
    pub const SCALED_H: Self = Self(1 << 1);
    /// The layer is scaled vertically.
    pub const SCALED_V: Self = Self(1 << 2);
    /// The layer's buffer has a YCbCr format.
    pub const YUV: Self = Self(1 << 3);
    /// The layer is scaled along either axis.
    pub const SCALED: Self = Self(Self::SCALED_H.0 | Self::SCALED_V.0);

    /// Union of two sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns `true` if the two sets share a trait.
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

/// A resource shared by a set of planes, such as a scaler or a pool of
/// fetch units.
///
/// A layer placed on a member plane takes one unit if it has any of the
/// `consumers` traits. Use [`LayerTraits::ANY`] to limit how many member
/// planes are enabled at once.
///
/// Groups only describe one output's frame. A resource that is also drawn on
/// by other outputs can be exhausted without the planner knowing; the
/// backend check then has the last word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ResourceGroup {
    /// Which layers take a unit.
    pub consumers: LayerTraits,
    /// Member planes, as a bit mask over the indices of
    /// [`Capabilities::planes`].
    pub planes: u32,
    /// How many units exist.
    pub capacity: u8,
}

/// One output and its current mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OutputCaps {
    /// Backend-defined identifier.
    pub id: OutputId,
    /// Active resolution.
    pub size: Size,
    /// Refresh rate in millihertz, or 0 if unknown.
    pub refresh_mhz: u32,
}

/// Everything the planner needs to know about a backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// Available outputs.
    pub outputs: FixedVec<OutputCaps, MAX_OUTPUTS>,
    /// Available planes. On a leased DRM device this lists only the planes
    /// the lease actually grants.
    pub planes: FixedVec<PlaneCaps, MAX_PLANES>,
    /// Resources shared between planes.
    pub resource_groups: FixedVec<ResourceGroup, MAX_RESOURCE_GROUPS>,
    /// Whether a new commit may replace one that has not been latched yet.
    /// If `false`, committing while a submission is pending fails as busy.
    pub replace_pending: bool,
}

impl Capabilities {
    /// Removes every output, plane and resource group, and clears
    /// `replace_pending`, without building a new description on the stack.
    pub fn clear(&mut self) {
        self.outputs.clear();
        self.planes.clear();
        self.resource_groups.clear();
        self.replace_pending = false;
    }

    /// Index of `output` in [`Capabilities::outputs`].
    pub fn output_index(&self, output: OutputId) -> Option<usize> {
        self.outputs.iter().position(|o| o.id == output)
    }
}
