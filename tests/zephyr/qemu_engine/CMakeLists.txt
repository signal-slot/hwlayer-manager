# Zephyr application that drives the composition engine through its C ABI
# (libcomposition_engine.a of crates/composition-ffi-staticlib) with a fake
# display backend written in C. Runs in QEMU; see README.md.

cmake_minimum_required(VERSION 3.20.0)

find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(qemu_engine LANGUAGES C)

# The repository root: tests/zephyr/qemu_engine -> ../../..
get_filename_component(REPO_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../.." ABSOLUTE)

# The engine library must be built for the same float ABI as this image
# (thumbv7em-none-eabihf: ARMv7E-M, FPv4-SP-D16, hard-float calling
# convention). prj.conf turns on CONFIG_FPU and CONFIG_FP_HARDABI for this.
#
# By default cargo builds the library into this build directory, so the
# result does not depend on CARGO_TARGET_DIR. Pass
# -DCOMPOSITION_ENGINE_LIB=/path/to/libcomposition_engine.a to use a library
# built elsewhere with the same command.
set(COMPOSITION_ENGINE_RUST_TARGET thumbv7em-none-eabihf)
if(NOT COMPOSITION_ENGINE_LIB)
  find_program(CARGO cargo REQUIRED)
  set(CARGO_TARGET_DIR_HERE "${CMAKE_BINARY_DIR}/cargo")
  set(COMPOSITION_ENGINE_LIB
      "${CARGO_TARGET_DIR_HERE}/${COMPOSITION_ENGINE_RUST_TARGET}/release/libcomposition_engine.a")
  # Always runs; cargo itself decides whether anything changed.
  add_custom_target(composition_engine_cargo
    COMMAND ${CARGO} build --locked
            -p composition-ffi-staticlib
            --no-default-features --features panic-handler
            --target ${COMPOSITION_ENGINE_RUST_TARGET} --release
            --target-dir "${CARGO_TARGET_DIR_HERE}"
    WORKING_DIRECTORY "${REPO_ROOT}"
    BYPRODUCTS "${COMPOSITION_ENGINE_LIB}"
    COMMENT "cargo build composition-ffi-staticlib for ${COMPOSITION_ENGINE_RUST_TARGET}"
    USES_TERMINAL
  )
endif()

add_library(composition_engine STATIC IMPORTED GLOBAL)
set_target_properties(composition_engine PROPERTIES
  IMPORTED_LOCATION "${COMPOSITION_ENGINE_LIB}"
  INTERFACE_INCLUDE_DIRECTORIES "${REPO_ROOT}/crates/composition-ffi/include"
)
if(TARGET composition_engine_cargo)
  add_dependencies(composition_engine composition_engine_cargo)
endif()

target_sources(app PRIVATE src/main.c)
target_link_libraries(app PRIVATE composition_engine)
