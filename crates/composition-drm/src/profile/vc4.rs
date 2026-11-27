//! Raspberry Pi vc4 HVS constraints (BCM2711 HVS5, BCM2712 HVS6).
//!
//! Table: not filled yet (placeholder). See `docs/hvs-backend-design.md`.

use composition_core::Capabilities;

/// Which hardware video scaler generation a profile describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Generation {
    /// BCM2711 (Raspberry Pi 4 family).
    Hvs5,
    /// BCM2712 (Raspberry Pi 5).
    Hvs6,
}

/// Adds the vc4 HVS constraints of `generation` to `caps`. No-op until the
/// table exists.
pub(super) fn apply_vc4(_caps: &mut Capabilities, _generation: Generation) {}
