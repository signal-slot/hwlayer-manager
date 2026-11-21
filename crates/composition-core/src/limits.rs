//! Compile-time capacity limits of the core.
//!
//! Every table in the crate is sized by one of these constants, so the memory
//! footprint is fixed and no heap is needed.

/// Maximum number of outputs a backend can describe.
pub const MAX_OUTPUTS: usize = 4;
/// Maximum number of hardware planes a backend can describe.
///
/// Must not exceed 32: plane sets are stored as `u32` bit masks.
pub const MAX_PLANES: usize = 16;
/// Maximum number of layers in one frame.
pub const MAX_LAYERS: usize = 16;
/// Maximum number of format/modifier pairs per plane.
pub const MAX_FORMATS: usize = 32;
/// Maximum number of registered buffers.
pub const MAX_BUFFERS: usize = 64;
/// Maximum number of shared-resource groups.
pub const MAX_RESOURCE_GROUPS: usize = 8;
/// Capacity of the display event queue.
///
/// Sized so that a full reset (every output dropping a pending submission and
/// releasing all buffers) always fits into an empty queue.
pub const MAX_EVENTS: usize = MAX_OUTPUTS * (2 * MAX_LAYERS + 1) + 16;

const _: () = assert!(MAX_PLANES <= 32);
const _: () = assert!(MAX_OUTPUTS <= 32);
