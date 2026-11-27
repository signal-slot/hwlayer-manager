# FICTIONAL M1 report (self-test fixture, not real data)

Date: 2099-01-02

Overall M1 verdict: passed

## Verdicts
- Condition 1: passed - builds
- Condition 2: passed - topology captured
- **Condition 3**: passed - planes 101 and 102 shown at once, z-order and alpha
  verified in `scripts/testdata/m1-report/fictional-both-planes.png`.
- Condition 4: passed
- Condition 5: failed - fictional
- Condition 6: blocked - fictional lease limit
- Condition 7: passed

## Condition 7 fields
- Board revision: Fictional Board rev Z
- Panel and interface: Fictional 7" panel, FICT-LVDS
- BSP/kernel version: fictional-bsp 0.0, kernel 0.0.0
- Lease route and version: none, fictional-lease 0.0
- Resolution and refresh: 640x480 @ 30 Hz
- Plane assignments: layer A -> plane 101, layer B -> plane 102
- Errors: none
- Frame loss: 0 of 100
- Latency: not measured
- CPU: 1 %
- Memory: not measured
