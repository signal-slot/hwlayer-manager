# i.MX95 ownership summary

## remoteproc

| instance | name | state | firmware |
|---|---|---|---|
| remoteproc0 | imx-rproc | offline | example-m7.elf |
| remoteproc1 | imx-rproc-b | running | - |

## display nodes

| node | compatible | status |
|---|---|---|
| /display-subsystem | example,display-subsystem | okay |
| /soc/display-controller@0 | example,dpu | okay |
| /soc/ldb@0 | example,ldb | disabled |

## rproc nodes

| node | compatible | status |
|---|---|---|
| /soc/remoteproc-m7 | example,rproc-m7 | okay |

## reserved-memory nodes

| node | compatible | status |
|---|---|---|
| /reserved-memory | - | okay (default) |
| /reserved-memory/vdev0vring0@0 | shared-dma-pool | okay |

## SCMI / SM version

- [    0.5] arm-scmi: SCMI Protocol v2.0 'NXP:SM' Version 1.2.3

## clock entries (dpu/ldb/lvds/disp)

- `disp_axi_root 1 1 0 400000000 0 0 50000 Y`
- `ldb_pll_pixel 0 0 0 0 0 0 50000 N`

## genpd entries (dpu/ldb/lvds/disp)

- `display-domain on 0`
- `lvds-domain off-0 0`
