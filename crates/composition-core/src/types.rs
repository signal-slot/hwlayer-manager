//! Scene model: identifiers, geometry, pixel formats, layers and frames.

/// Identifies an output (a display pipeline: CRTC, or an MCU video port).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OutputId(pub u32);

/// Identifies a hardware plane. The value is backend-defined.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlaneId(pub u32);

/// Identifies a registered buffer.
///
/// The core never interprets it: file descriptors, physical addresses and
/// mappings belong to platform code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(pub u32);

/// Identifies one accepted frame submission. Values increase monotonically.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubmissionId(pub u64);

/// A width/height pair in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Size {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Size {
    /// Creates a size.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// An axis-aligned rectangle in whole pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Rect {
    /// Creates a rectangle.
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns `true` if the rectangle covers no pixels.
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Returns `true` if the rectangle lies entirely inside `0,0 .. size`.
    pub const fn fits_within(&self, size: Size) -> bool {
        self.x >= 0
            && self.y >= 0
            && self.x as i64 + self.width as i64 <= size.width as i64
            && self.y as i64 + self.height as i64 <= size.height as i64
    }

    /// Returns `true` if the rectangle is exactly `0,0 .. size`.
    pub const fn covers(&self, size: Size) -> bool {
        self.x == 0 && self.y == 0 && self.width == size.width && self.height == size.height
    }
}

/// A pixel format as a DRM-style FourCC code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PixelFormat(pub u32);

impl PixelFormat {
    /// Builds a format from its four-character code.
    pub const fn fourcc(code: &[u8; 4]) -> Self {
        Self(
            code[0] as u32
                | (code[1] as u32) << 8
                | (code[2] as u32) << 16
                | (code[3] as u32) << 24,
        )
    }

    /// 16-bit RGB 5:6:5.
    pub const RGB565: Self = Self::fourcc(b"RG16");
    /// 32-bit xRGB 8:8:8:8, alpha ignored.
    pub const XRGB8888: Self = Self::fourcc(b"XR24");
    /// 32-bit ARGB 8:8:8:8.
    pub const ARGB8888: Self = Self::fourcc(b"AR24");
    /// 32-bit xBGR 8:8:8:8, alpha ignored.
    pub const XBGR8888: Self = Self::fourcc(b"XB24");
    /// 32-bit ABGR 8:8:8:8.
    pub const ABGR8888: Self = Self::fourcc(b"AB24");
    /// 2-plane YCbCr 4:2:0.
    pub const NV12: Self = Self::fourcc(b"NV12");
    /// 2-plane YCbCr 4:2:0 with the chroma order swapped.
    pub const NV21: Self = Self::fourcc(b"NV21");
    /// 2-plane YCbCr 4:2:2.
    pub const NV16: Self = Self::fourcc(b"NV16");
    /// Packed YCbCr 4:2:2.
    pub const YUYV: Self = Self::fourcc(b"YUYV");
    /// Packed YCbCr 4:2:2, chroma first.
    pub const UYVY: Self = Self::fourcc(b"UYVY");
    /// 3-plane YCbCr 4:2:0.
    pub const YUV420: Self = Self::fourcc(b"YU12");

    /// Returns `true` if the format is a known YCbCr format.
    pub const fn is_known_yuv(self) -> bool {
        self.0 == Self::NV12.0
            || self.0 == Self::NV21.0
            || self.0 == Self::NV16.0
            || self.0 == Self::YUYV.0
            || self.0 == Self::UYVY.0
            || self.0 == Self::YUV420.0
    }

    /// Returns `true` if the format is known to carry no alpha channel.
    ///
    /// Unknown formats return `false`, so they are treated as if they might
    /// carry alpha.
    pub const fn is_known_opaque(self) -> bool {
        self.0 == Self::RGB565.0
            || self.0 == Self::XRGB8888.0
            || self.0 == Self::XBGR8888.0
            || self.is_known_yuv()
    }
}

/// A buffer layout modifier (tiling, compression). `LINEAR` is plain raster.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifier(pub u64);

impl Modifier {
    /// Plain linear layout.
    pub const LINEAR: Self = Self(0);
}

/// Immutable metadata of a registered buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BufferInfo {
    /// Size in pixels.
    pub size: Size,
    /// Pixel format.
    pub format: PixelFormat,
    /// Layout modifier.
    pub modifier: Modifier,
}

/// Rotation or reflection applied to a layer's source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Transform {
    /// No transform.
    #[default]
    Normal,
    /// Rotated by 90 degrees.
    Rotate90,
    /// Rotated by 180 degrees.
    Rotate180,
    /// Rotated by 270 degrees.
    Rotate270,
    /// Mirrored horizontally.
    FlipH,
    /// Mirrored vertically.
    FlipV,
}

impl Transform {
    /// Bit of this transform in a transform mask.
    pub const fn mask(self) -> u8 {
        1 << self as u8
    }

    /// Returns `true` if source width and height swap on screen.
    pub const fn swaps_axes(self) -> bool {
        matches!(self, Transform::Rotate90 | Transform::Rotate270)
    }
}

/// How a layer's pixels combine with the layers beneath it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendMode {
    /// Pixel alpha is ignored; the layer is opaque apart from its plane alpha.
    #[default]
    Opaque,
    /// Colour channels are already multiplied by pixel alpha.
    Premultiplied,
    /// Colour channels are not multiplied by pixel alpha.
    Coverage,
}

impl BlendMode {
    /// Bit of this mode in a blend-mode mask.
    pub const fn mask(self) -> u8 {
        1 << self as u8
    }
}

/// Plane alpha value meaning fully opaque.
pub const ALPHA_OPAQUE: u16 = 0xFFFF;

/// One layer of a frame: a region of a buffer placed on the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Layer {
    /// Buffer to show.
    pub buffer: BufferId,
    /// Region of the buffer to show, in buffer pixels.
    pub source: Rect,
    /// Where the region appears, in output pixels.
    pub destination: Rect,
    /// Stacking order: larger is closer to the viewer. Layers with equal
    /// values stack in their order in the frame, later on top.
    pub z_order: i32,
    /// Constant alpha for the whole layer, [`ALPHA_OPAQUE`] for none.
    pub alpha: u16,
    /// Blending against lower layers.
    pub blend: BlendMode,
    /// Rotation or reflection.
    pub transform: Transform,
}

impl Layer {
    /// An opaque, untransformed layer showing `source` of `buffer` at
    /// `destination`.
    pub const fn new(buffer: BufferId, source: Rect, destination: Rect, z_order: i32) -> Self {
        Self {
            buffer,
            source,
            destination,
            z_order,
            alpha: ALPHA_OPAQUE,
            blend: BlendMode::Opaque,
            transform: Transform::Normal,
        }
    }
}

/// The complete desired content of one output.
///
/// A frame with no layers blanks the output.
#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    /// Output to present on.
    pub output: OutputId,
    /// Layers in any order; stacking is given by [`Layer::z_order`].
    pub layers: &'a [Layer],
}
