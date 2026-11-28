# SPDX-License-Identifier: Apache-2.0
cmake_minimum_required(VERSION 3.20.0)
find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(imx95_m7_latch)

target_sources(app PRIVATE src/main.c)

if(CONFIG_IMX95_M7_LATCH_ENABLE_DPU)
  # The application programs the DPU itself through the NXP MCUX DPU HAL
  # (dpu_1), unchanged, from the hal_nxp module of the west workspace. Zephyr
  # only builds that HAL for its own display driver (drivers.cmake maps
  # CONFIG_DISPLAY_SOCIONEXT_DPU to driver.dpu_1), and that driver must stay
  # off here because it owns the same interrupt line. So the HAL source is
  # compiled as part of the application instead.
  set(DPU_HAL_DIR ${ZEPHYR_HAL_NXP_MODULE_DIR}/mcux/mcux-sdk-ng/drivers/dpu_1)
  if(NOT EXISTS ${DPU_HAL_DIR}/fsl_dpu.c)
    message(FATAL_ERROR "fsl_dpu.c not found in ${DPU_HAL_DIR}; is hal_nxp in the west workspace?")
  endif()
  target_sources(app PRIVATE ${DPU_HAL_DIR}/fsl_dpu.c)
  target_include_directories(app PRIVATE ${DPU_HAL_DIR})
endif()
