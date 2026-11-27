# SPDX-License-Identifier: Apache-2.0
#
# i.MX95 Cortex-M7 hello / DPU read-only probe.
# Built with upstream Zephyr only; see README.md for the pinned commit.

cmake_minimum_required(VERSION 3.20.0)

find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(hwlm_imx95_m7_probe)

target_sources(app PRIVATE src/main.c)

if(CONFIG_OPENAMP_RSC_TABLE)
  # resource_table.h and addr_translation.h come from Zephyr's
  # subsys/ipc/open-amp; libmetal and OpenAMP headers from their modules.
  target_include_directories(app PRIVATE
    ${LIBMETAL_INCLUDE_DIR}
    ${OPENAMP_INCLUDE_DIR}
    ${ZEPHYR_BASE}/subsys/ipc/open-amp
  )
endif()
