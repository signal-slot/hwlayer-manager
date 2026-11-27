//! Shared plane state of a test commit.
//!
//! The model, the pipe and the comparison all describe "what one plane is
//! asked to do" with [`PlaneState`], so they cannot disagree on its meaning.

/// DRM rotation/reflection bits (`DRM_MODE_ROTATE_*`, `DRM_MODE_REFLECT_*`).
pub mod rotation {
    /// No rotation.
    pub const ROTATE_0: u32 = 1 << 0;
    /// Rotate 90 degrees.
    pub const ROTATE_90: u32 = 1 << 1;
    /// Rotate 180 degrees.
    pub const ROTATE_180: u32 = 1 << 2;
    /// Rotate 270 degrees.
    pub const ROTATE_270: u32 = 1 << 3;
    /// Reflect along the X axis (horizontal flip).
    pub const REFLECT_X: u32 = 1 << 4;
    /// Reflect along the Y axis (vertical flip).
    pub const REFLECT_Y: u32 = 1 << 5;

    /// The `rotation` property value for a layer transform: a reflection
    /// alone gets [`ROTATE_0`], as DRM requires exactly one rotation bit (the
    /// C driver's `parse_layer_transform` does the same).
    pub const fn property_value(transform: u32) -> u32 {
        if transform & (ROTATE_0 | ROTATE_90 | ROTATE_180 | ROTATE_270) == 0 {
            transform | ROTATE_0
        } else {
            transform
        }
    }
}

/// Pixel blend mode of a plane or layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendMode {
    /// Source replaces the destination (`None` in KMS).
    Opaque,
    /// Source colour is already multiplied by alpha.
    Premultiplied,
    /// Source colour is multiplied by alpha during blending.
    Coverage,
}

/// Unsigned rectangle: used for `SRC_*` (16.16 fixed point in
/// [`PlaneState`], whole pixels in an `xcheck` layer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct URect {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
}

/// Rectangle with a signed origin: `CRTC_X/Y` and unsigned `CRTC_W/H`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    /// Left edge (may be negative).
    pub x: i32,
    /// Top edge (may be negative).
    pub y: i32,
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
}

/// One plane's requested state in a test commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlaneState {
    /// KMS plane object id.
    pub plane_id: u32,
    /// Framebuffer id (`FB_ID`), or `None` when the plane is disabled.
    pub fb: Option<u32>,
    /// Source rectangle, 16.16 fixed point (`SRC_X/Y/W/H`).
    pub src: URect,
    /// Destination rectangle on the CRTC (`CRTC_X/Y/W/H`).
    pub crtc: Rect,
    /// Requested `zpos`.
    pub zpos: i32,
    /// Plane alpha, 0..=0xFFFF.
    pub alpha: u16,
    /// Pixel blend mode.
    pub blend: BlendMode,
    /// Rotation bits, see [`rotation`].
    pub rotation: u32,
}
