//! The composition engine's backend for a display driver with two planes on
//! one output, written in C: `ce_twoplane.h` and `ce_twoplane.c` in the
//! directory of this package. A firmware compiles them with its own C
//! compiler and links them with `libcomposition_engine.a`.
//!
//! The crate's only code is [`imx95`], the i.MX 95 DPU's tables for a
//! future port, as data with unit tests. Its integration tests (`tests/twoplane.rs`) compile the
//! backend with the system C compiler, run it behind a fake driver through
//! the engine's C API, and compile it once more freestanding, without a C
//! library.

#![no_std]
#![forbid(unsafe_code)]

pub mod imx95;
