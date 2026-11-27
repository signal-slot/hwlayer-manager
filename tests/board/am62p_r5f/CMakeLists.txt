# SPDX-License-Identifier: Apache-2.0
#
# Board test of the composition engine on the Verdin AM62P's MCU R5F
# (docs/board-test-am62p-r5f.md). The board verdin_am62p/am62p54/mcu_r5f0_0,
# the SoC and the DSS driver come from yakogl and the demo module, passed in
# ZEPHYR_MODULES; nothing of them is in this repository.

cmake_minimum_required(VERSION 3.20.0)

find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(hwlayer_board_am62p_r5f C)

get_filename_component(HWLAYER_ROOT ${CMAKE_CURRENT_LIST_DIR}/../../.. ABSOLUTE)

# The engine: the static library of composition-ffi-staticlib, no std, with
# its own panic handler, for the R5F (Armv7-R, hard-float ABI: prj.conf sets
# CONFIG_FPU and CONFIG_FP_HARDABI to match). Cargo decides whether it is
# up to date, so it runs at every build.
set(CE_RUST_TARGET armv7r-none-eabihf)
set(CE_CARGO_TARGET_DIR ${CMAKE_BINARY_DIR}/cargo)
set(CE_LIBRARY ${CE_CARGO_TARGET_DIR}/${CE_RUST_TARGET}/release/libcomposition_engine.a)

add_custom_target(composition_engine_build
  COMMAND cargo build --locked --release
          -p composition-ffi-staticlib --no-default-features --features panic-handler
          --target ${CE_RUST_TARGET} --target-dir ${CE_CARGO_TARGET_DIR}
  WORKING_DIRECTORY ${HWLAYER_ROOT}
  BYPRODUCTS ${CE_LIBRARY}
  USES_TERMINAL
  VERBATIM)

add_library(composition_engine STATIC IMPORTED GLOBAL)
set_target_properties(composition_engine PROPERTIES IMPORTED_LOCATION ${CE_LIBRARY})
add_dependencies(composition_engine composition_engine_build)

target_sources(app PRIVATE
  src/main.c
  src/scenes.c
  src/hw_am62p.c
  ${HWLAYER_ROOT}/backends/twoplane/ce_twoplane.c)

target_include_directories(app PRIVATE
  src
  ${HWLAYER_ROOT}/backends/twoplane
  ${HWLAYER_ROOT}/crates/composition-ffi/include)

# Overridable at configuration: the largest panel the two application
# buffers fit, and how long each scene stays on the panel.
set(TEST_APP_MAX_WIDTH 1280 CACHE STRING "Application buffer width, pixels")
set(TEST_APP_MAX_HEIGHT 800 CACHE STRING "Application buffer height, pixels")
set(TEST_HOLD_MS 5000 CACHE STRING "Time each scene is held, ms")
target_compile_definitions(app PRIVATE
  TEST_APP_MAX_WIDTH=${TEST_APP_MAX_WIDTH}
  TEST_APP_MAX_HEIGHT=${TEST_APP_MAX_HEIGHT}
  TEST_HOLD_MS=${TEST_HOLD_MS})

target_link_libraries(app PRIVATE composition_engine)
