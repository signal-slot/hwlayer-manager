# SPDX-License-Identifier: Apache-2.0
cmake_minimum_required(VERSION 3.20.0)
find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(imx95_m7_overlay)

target_sources(app PRIVATE src/main.c)

# Opt-in guard. Off by default: the image then contains no DPU code and
# main() only prints a refusal. Linux may own the DPU on the board in use
# (see docs/imx95-zephyr-feasibility.md); turn this on only on a board where
# the M7 has been shown to own the DPU, its clocks and its power domain.
option(IMX95_M7_OVERLAY_ENABLE_DPU "Allow this image to program the DPU" OFF)

# How the overlay is placed on the screen. The two upstream sources disagree
# (docs/imx95-m7-overlay-design.md section 2.2, item O2):
#   LAYEROFFSET - the HAL way: overlay fetch frame = full screen, layer placed
#                 with the fetch unit's LAYEROFFSET0, LayerBlend POSITION = 0.
#   POSITION    - the NXP Linux 6.6 way: overlay fetch frame = overlay size,
#                 LAYEROFFSET0 = 0, placed with the LayerBlend POSITION.
set(IMX95_M7_OVERLAY_PLACEMENT "LAYEROFFSET" CACHE STRING
    "Overlay placement method: LAYEROFFSET or POSITION")
set_property(CACHE IMX95_M7_OVERLAY_PLACEMENT PROPERTY STRINGS LAYEROFFSET POSITION)

if(IMX95_M7_OVERLAY_ENABLE_DPU)
  # The stock Zephyr DPU driver drives one fixed plane and programs the DPU at
  # boot, and the HAL source is only compiled together with it
  # (CONFIG_DISPLAY_SOCIONEXT_DPU). This app keeps the driver out (the dpu
  # node stays disabled) and compiles the upstream hal_nxp DPU source itself.
  set(DPU_HAL_DIR ${ZEPHYR_HAL_NXP_MODULE_DIR}/mcux/mcux-sdk-ng/drivers/dpu_1)
  if(NOT EXISTS ${DPU_HAL_DIR}/fsl_dpu.c)
    message(FATAL_ERROR "hal_nxp DPU source not found at ${DPU_HAL_DIR}")
  endif()
  target_sources(app PRIVATE ${DPU_HAL_DIR}/fsl_dpu.c)
  target_include_directories(app PRIVATE ${DPU_HAL_DIR})
  target_compile_definitions(app PRIVATE IMX95_M7_OVERLAY_ENABLE_DPU=1)

  if(IMX95_M7_OVERLAY_PLACEMENT STREQUAL "LAYEROFFSET")
    target_compile_definitions(app PRIVATE IMX95_M7_OVERLAY_PLACE_BY_POSITION=0)
  elseif(IMX95_M7_OVERLAY_PLACEMENT STREQUAL "POSITION")
    target_compile_definitions(app PRIVATE IMX95_M7_OVERLAY_PLACE_BY_POSITION=1)
  else()
    message(FATAL_ERROR
      "IMX95_M7_OVERLAY_PLACEMENT must be LAYEROFFSET or POSITION, "
      "not '${IMX95_M7_OVERLAY_PLACEMENT}'")
  endif()
endif()
