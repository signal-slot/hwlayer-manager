//! Hardware-independent core of the composition engine.
//!
//! The crate is `no_std`, allocation-free and contains no `unsafe`. All
//! tables are fixed-capacity; the maxima live in [`limits`].
//!
//! Layering:
//!
//! - [`types`] and [`caps`]: the scene model and the capability model.
//! - [`planner`]: assigns layers to hardware planes, or reports why it cannot.
//! - [`advise`]: advisory planning for a compositor that composes what the
//!   planes cannot take; never commits.
//! - [`presentation`]: buffer and submission lifecycle
//!   (writable → ready → submitted → scanning → released).
//! - [`readiness`]: opaque tokens for buffers whose content completes
//!   asynchronously (the core's side of synchronization fences).
//! - [`backend`]: the contract a display backend implements.
//! - [`engine`]: ties the three together for a single backend.
//! - [`trace`]: structured traces of what the engine does, for an optional
//!   sink.

#![no_std]
#![forbid(unsafe_code)]

pub mod advise;
pub mod backend;
pub mod caps;
pub mod engine;
pub mod fixed;
pub mod limits;
pub mod planner;
pub mod presentation;
pub mod readiness;
pub mod trace;
pub mod types;

pub use advise::{
    advise, explain_layer, Advice, AdviceError, AdviseOptions, Blocker, ComposeTarget, LayerAdvice,
    Placement, COMPOSE_LAYER, MAX_CANDIDATES,
};
pub use backend::{BackendError, BackendEvent, DisplayBackend};
pub use caps::{
    Capabilities, FormatModifier, LayerTraits, OutputCaps, PlaneCaps, PlaneKind, ResourceGroup,
    ScaleLimits, Zpos,
};
pub use engine::{Engine, Error};
pub use planner::{
    Assignment, BufferLookup, Plan, PlanError, RejectReasons, SearchLimits, Verdict,
};
pub use presentation::{BufferState, DisplayEvent, PresentError, Presentation};
pub use readiness::{ReadinessState, ReadyToken};
pub use trace::{DropReason, NoTrace, Trace, TraceEvent};
pub use types::{
    BlendMode, BufferId, BufferInfo, Frame, Layer, Modifier, OutputId, PixelFormat, PlaneId, Rect,
    Size, SubmissionId, Transform, ALPHA_OPAQUE,
};
