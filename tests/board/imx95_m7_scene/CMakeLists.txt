# SPDX-License-Identifier: Apache-2.0
#
# The WP3 comparison scene on the i.MX95 Cortex-M7 (upstream Zephyr), driven
# through the composition engine's C ABI and written as docs/trace-format.md
# lines.
#
# Linking the engine is done as in tests/board/imx95_m7_engine: by default
# the build runs cargo to make libcomposition_engine.a for
# thumbv7em-none-eabihf (no std, with the halting panic handler, release),
# into this build directory. To link a library built beforehand instead,
# pass -DCOMPOSITION_ENGINE_LIB=/path/to/libcomposition_engine.a.
cmake_minimum_required(VERSION 3.20.0)

find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(imx95_m7_scene LANGUAGES C)

get_filename_component(REPO_ROOT ${CMAKE_CURRENT_SOURCE_DIR}/../../.. ABSOLUTE)
set(CE_INCLUDE_DIR ${REPO_ROOT}/crates/composition-ffi/include)
set(CE_RUST_TARGET thumbv7em-none-eabihf)

set(COMPOSITION_ENGINE_LIB "" CACHE FILEPATH
  "Prebuilt libcomposition_engine.a; empty to build it with cargo")

if(COMPOSITION_ENGINE_LIB)
  set(CE_LIB ${COMPOSITION_ENGINE_LIB})
  add_custom_target(composition_engine_cargo)
else()
  find_program(CARGO cargo HINTS $ENV{HOME}/.cargo/bin REQUIRED)
  set(CE_CARGO_TARGET_DIR ${CMAKE_BINARY_DIR}/cargo)
  set(CE_LIB ${CE_CARGO_TARGET_DIR}/${CE_RUST_TARGET}/release/libcomposition_engine.a)
  # Always run: cargo itself decides whether anything changed.
  add_custom_target(composition_engine_cargo
    COMMAND ${CMAKE_COMMAND} -E env CARGO_TARGET_DIR=${CE_CARGO_TARGET_DIR}
            ${CARGO} build --locked
            -p composition-ffi-staticlib
            --no-default-features --features panic-handler
            --target ${CE_RUST_TARGET} --release
    WORKING_DIRECTORY ${REPO_ROOT}
    BYPRODUCTS ${CE_LIB}
    COMMENT "cargo build composition-ffi-staticlib (${CE_RUST_TARGET}, no std, release)"
    USES_TERMINAL
  )
endif()

add_library(composition_engine STATIC IMPORTED GLOBAL)
set_target_properties(composition_engine PROPERTIES IMPORTED_LOCATION ${CE_LIB})
add_dependencies(composition_engine composition_engine_cargo)

# The repository revision goes into the hdr line's build= text. Taken at
# configure time; "unknown" outside a git checkout. A "-dirty" suffix marks
# uncommitted changes.
execute_process(
  COMMAND git describe --always --dirty --abbrev=12
  WORKING_DIRECTORY ${REPO_ROOT}
  OUTPUT_VARIABLE SCENE_REPO_REV
  OUTPUT_STRIP_TRAILING_WHITESPACE
  ERROR_QUIET
  RESULT_VARIABLE SCENE_REPO_REV_RESULT
)
if(NOT SCENE_REPO_REV_RESULT EQUAL 0 OR SCENE_REPO_REV STREQUAL "")
  set(SCENE_REPO_REV unknown)
endif()

target_sources(app PRIVATE src/main.c src/trace.c)
target_include_directories(app PRIVATE ${CE_INCLUDE_DIR})
target_compile_definitions(app PRIVATE SCENE_REPO_REV="${SCENE_REPO_REV}")
target_link_libraries(app PRIVATE composition_engine)
