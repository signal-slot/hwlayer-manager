//! Cross-check of the `composition-core` planner against libliftoff.
//!
//! See `docs/libliftoff-crosscheck.md`. Host-only, outside the workspace, not
//! in CI.
#![forbid(unsafe_code)]

pub mod compare;
pub mod core_side;
pub mod model;
pub mod pipe;
pub mod probe;
pub mod state;
pub mod sweep;
pub mod xcheck;
