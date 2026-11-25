//! The composition engine's backend for a display driver with two planes on
//! one output, written in C: `ce_twoplane.h` and `ce_twoplane.c` in the
//! directory of this package. A firmware compiles them with its own C
//! compiler and links them with `libcomposition_engine.a`.
//!
//! This crate has no code. Its tests (`tests/twoplane.rs`) compile the
//! backend with the system C compiler, run it behind a fake driver through
//! the engine's C API, and compile it once more freestanding, without a C
//! library.

#![no_std]
#![forbid(unsafe_code)]
