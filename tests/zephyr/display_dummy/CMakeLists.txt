# SPDX-License-Identifier: Apache-2.0
cmake_minimum_required(VERSION 3.20.0)
# CONFIG_DISPLAY makes Zephyr start QEMU with "-display sdl" instead of
# "-nographic", and QEMU then waits on a window. This app shows nothing: run
# headless (the later -display wins).
set(QEMU_EXTRA_FLAGS -display none)
find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(display_dummy)

# The engine as a static library for a Cortex-M3 (thumbv7m), built by cargo
# from this repository: no std, with the halting panic handler.
get_filename_component(HWL_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../.." ABSOLUTE)
set(CE_TARGET thumbv7m-none-eabi)
set(CE_TARGET_DIR "${CMAKE_BINARY_DIR}/composition-cargo")
set(CE_LIB "${CE_TARGET_DIR}/${CE_TARGET}/release/libcomposition_engine.a")

find_program(CARGO cargo REQUIRED)
add_custom_command(
  OUTPUT "${CE_LIB}"
  COMMAND ${CARGO} build --release --locked
          --manifest-path "${HWL_ROOT}/Cargo.toml"
          -p composition-ffi-staticlib
          --no-default-features --features panic-handler
          --target ${CE_TARGET}
          --target-dir "${CE_TARGET_DIR}"
  COMMENT "cargo build composition-ffi-staticlib (${CE_TARGET})"
  VERBATIM
)
add_custom_target(composition_engine_build DEPENDS "${CE_LIB}")
add_library(composition_engine STATIC IMPORTED GLOBAL)
set_target_properties(composition_engine PROPERTIES IMPORTED_LOCATION "${CE_LIB}")
add_dependencies(composition_engine composition_engine_build)

target_sources(app PRIVATE src/main.c)
target_include_directories(app PRIVATE "${HWL_ROOT}/crates/composition-ffi/include")
target_link_libraries(app PRIVATE composition_engine)
