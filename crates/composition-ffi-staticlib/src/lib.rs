//! The C ABI of the composition engine (`composition-ffi`, header
//! `crates/composition-ffi/include/composition_engine.h`) as a static library,
//! `libcomposition_engine.a`, for C programs that contain no other Rust code.
//!
//! A program that already links a Rust library depends on `composition-ffi`
//! from that library instead, so that there is one copy of `core` and one
//! panic handler.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

// Named so that its C functions are linked into this library.
extern crate composition_ffi;

#[cfg(all(not(feature = "std"), feature = "panic-handler"))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
