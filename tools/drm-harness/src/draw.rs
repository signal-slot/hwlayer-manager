//! CPU drawing into 32-bit linear buffers (XRGB8888 / ARGB8888).
//!
//! Colours are `0xAARRGGBB`; the formats store them little-endian.

use composition_core::Rect;

/// Opaque colours used by the scenarios.
pub mod colour {
    /// Dark blue.
    pub const BLUE: u32 = 0xFF00_2080;
    /// Red.
    pub const RED: u32 = 0xFFE0_1010;
    /// Green.
    pub const GREEN: u32 = 0xFF10_C010;
    /// White.
    pub const WHITE: u32 = 0xFFFF_FFFF;
    /// Black.
    pub const BLACK: u32 = 0xFF00_0000;
    /// Mid grey.
    pub const GREY: u32 = 0xFF60_6060;
    /// Yellow.
    pub const YELLOW: u32 = 0xFFF0_E000;
    /// Red at half pixel alpha, premultiplied.
    pub const RED_HALF_PREMULTIPLIED: u32 = 0x8070_0808;
}

/// Fills `rect` (clipped to the buffer) with `colour`.
pub fn fill(pixels: &mut [u8], pitch: usize, rect: Rect, colour: u32) {
    let width = pitch / 4;
    let height = pixels.len() / pitch.max(1);
    let x0 = rect.x.max(0) as usize;
    let y0 = rect.y.max(0) as usize;
    let x1 = ((rect.x as i64 + rect.width as i64).max(0) as usize).min(width);
    let y1 = ((rect.y as i64 + rect.height as i64).max(0) as usize).min(height);
    if x0 >= x1 {
        return;
    }
    let bytes = colour.to_le_bytes();
    let mut row = Vec::with_capacity((x1 - x0) * 4);
    for _ in x0..x1 {
        row.extend_from_slice(&bytes);
    }
    for y in y0..y1 {
        let at = y * pitch + x0 * 4;
        pixels[at..at + row.len()].copy_from_slice(&row);
    }
}

/// Draws a `thickness`-pixel frame just inside `rect`.
pub fn border(pixels: &mut [u8], pitch: usize, rect: Rect, thickness: u32, colour: u32) {
    let t = thickness.min(rect.width / 2).min(rect.height / 2);
    let (x, y, w, h) = (rect.x, rect.y, rect.width, rect.height);
    fill(pixels, pitch, Rect::new(x, y, w, t), colour);
    fill(
        pixels,
        pitch,
        Rect::new(x, y + (h - t) as i32, w, t),
        colour,
    );
    fill(pixels, pitch, Rect::new(x, y, t, h), colour);
    fill(
        pixels,
        pitch,
        Rect::new(x + (w - t) as i32, y, t, h),
        colour,
    );
}

/// Fills `rect` with a checkerboard of `cell`-pixel squares.
pub fn checker(pixels: &mut [u8], pitch: usize, rect: Rect, cell: u32, a: u32, b: u32) {
    let cell = cell.max(1);
    let mut y = 0;
    while y < rect.height {
        let mut x = 0;
        while x < rect.width {
            let colour = if (x / cell + y / cell) % 2 == 0 { a } else { b };
            fill(
                pixels,
                pitch,
                Rect::new(
                    rect.x + x as i32,
                    rect.y + y as i32,
                    cell.min(rect.width - x),
                    cell.min(rect.height - y),
                ),
                colour,
            );
            x += cell;
        }
        y += cell;
    }
}

/// FNV-1a over the visible bytes of a buffer: `width * 4` bytes of each row.
pub fn checksum(pixels: &[u8], pitch: usize, width: u32, height: u32) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let row = width as usize * 4;
    for y in 0..height as usize {
        let start = y * pitch;
        for byte in &pixels[start..start + row] {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// One drawing step. Scenarios describe pictures as lists of steps, so that
/// the same picture can be drawn here or by a producer process (the steps
/// travel over its socket, see `producer.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// [`fill`].
    Fill {
        /// Area.
        rect: Rect,
        /// Colour.
        colour: u32,
    },
    /// [`border`].
    Border {
        /// Outer edge of the frame.
        rect: Rect,
        /// Width of the frame in pixels.
        thickness: u32,
        /// Colour.
        colour: u32,
    },
    /// [`checker`].
    Checker {
        /// Area.
        rect: Rect,
        /// Side of a square in pixels.
        cell: u32,
        /// Colour of the first square.
        a: u32,
        /// Colour of the other squares.
        b: u32,
    },
}

/// Draws `ops` in order.
pub fn apply(pixels: &mut [u8], pitch: usize, ops: &[Op]) {
    for op in ops {
        match *op {
            Op::Fill { rect, colour } => fill(pixels, pitch, rect, colour),
            Op::Border {
                rect,
                thickness,
                colour,
            } => border(pixels, pitch, rect, thickness, colour),
            Op::Checker { rect, cell, a, b } => checker(pixels, pitch, rect, cell, a, b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_draws_steps_in_order() {
        let pitch = 4 * 4;
        let mut pixels = vec![0u8; pitch * 4];
        apply(
            &mut pixels,
            pitch,
            &[
                Op::Fill {
                    rect: Rect::new(0, 0, 4, 4),
                    colour: colour::RED,
                },
                Op::Border {
                    rect: Rect::new(0, 0, 4, 4),
                    thickness: 1,
                    colour: colour::WHITE,
                },
            ],
        );
        let at = |x: usize, y: usize| {
            u32::from_le_bytes(pixels[y * pitch + x * 4..][..4].try_into().unwrap())
        };
        assert_eq!(at(0, 0), colour::WHITE);
        assert_eq!(at(1, 1), colour::RED);
        assert_eq!(at(3, 2), colour::WHITE);
    }
}
