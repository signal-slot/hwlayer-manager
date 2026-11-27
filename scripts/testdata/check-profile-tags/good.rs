pub enum P {
    /// Synthetic profile.
    ///
    /// - Three planes: observed 2026-11-27.
    /// - Upscaling only: source (`ltdc.c:1585`, pin v6.18-stm32mp).
    /// - A multi-line bullet whose tag sits on the continuation
    ///   line: inferred.
    /// - Observed on the board
    ///   2026-11-28 across a line break.
    ///
    /// Prose outside bullets needs no tag.
    A,
}
//! - Module level bullet: source (pin abc).
