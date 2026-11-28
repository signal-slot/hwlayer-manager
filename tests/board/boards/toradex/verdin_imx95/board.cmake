# SPDX-License-Identifier: Apache-2.0
#
# No flash runner on purpose. On the Verdin iMX95 the M7 image is the ELF
# (build/zephyr/zephyr.elf) that Linux remoteproc loads (see
# scripts/imx95-m7-rproc.sh). Upstream imx95_evk's spsdk runner builds an
# EVK boot container (flash.bin) and does not apply to this board.
