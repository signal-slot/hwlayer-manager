# AM62P MCU R5F board test (Zephyr)

A Zephyr application for the MCU Cortex-R5F of a Verdin AM62P. It runs the
engine's C API and `backends/twoplane` through eight scenes and prints one
`SCENE` line per scene and a final `RESULT` line.

Status (2026-11-28): the scene sequence has run on the host behind a fake
panel. The image has not been built from this repository's `main`, and
nothing has run on a board.

The build and the run procedure (build variables, the remoteproc route, the
trace buffer as console, what to photograph) are in
[docs/board-test-am62p-r5f.md](../../../docs/board-test-am62p-r5f.md). In short,
with the board and the demo module's checkouts named there available:

```sh
west build -b verdin_am62p/am62p54/mcu_r5f0_0 tests/board/am62p_r5f
```

It needs the demo module's DSS driver and the board definition from the
paths the document names. None of that is in this repository.
