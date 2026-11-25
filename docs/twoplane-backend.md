# The two-plane backend and what a board test still needs

`backends/twoplane/` holds the engine's backend for a display driver with two planes on one output (`ce_twoplane.h`, `ce_twoplane.c`). It was taken from the MCU firmware of the AM62P demo, where it was written against Zephyr, and the Zephyr calls were replaced by a port table. It is C, uses no C library beyond `<stdbool.h>`, `<stddef.h>` and `<stdint.h>`, and is compiled by the firmware that uses it.

## What is tested, and where

`cargo test -p composition-twoplane` (part of `cargo test --workspace`):

- compiles the backend and `backends/twoplane/tests/c/twoplane_test.c` with the system C compiler, links them with `libcomposition_engine.a`, and runs them. The test's fake driver has shadow registers, a commit count, a latched count and a frame start the test triggers, at any point of the backend's writes, with or without running the vsync interrupt.
- compiles the backend alone with `-ffreestanding -nostdinc` and the compiler's own headers only, checks that it reads no other system header, and that the object needs no symbol other than those a compiler may call for structure copies (`memcpy`, `memset`, `memmove`, `memcmp`).

Both run on the host only. The backend has not been compiled for an Arm target from this repository.

## What a board test needs and does not have yet

No board run has been made from this repository. Nothing about the backend on hardware has been measured here.

A board test needs a firmware that provides the following. None of it is in this repository.

### The driver: `struct ce_twoplane_hw`

The demo firmware fills this table for the AM62P DSS and the i.MX 95 DPU (`display_port_<soc>.c` there, Zephyr, not in this repository). Calls that write the planes each count as one commit; the writes of a commit reach the panel together at a frame start.

| Call | What the backend expects of it |
| --- | --- |
| `app_format_ok(format)` | Whether the application plane takes this `CE_FORMAT_*` |
| `set_app_buffer(dev, buf)` | Points the application plane at `buf` (address, size, stride, format, frame number). Nonzero: refused, nothing written |
| `enable_app_plane(dev, on)` | Switches the application plane on or off |
| `safety_flip(dev)` | Presents `safety_buffer()` at the next frame start |
| `safety_buffer(dev, &w, &h, &stride)` | The safety buffer that is not on the plane, and the panel's size |
| `safety_front(dev)` | The safety buffer the plane scans at start-up |
| `safety_premultiplied(dev, on)` | The safety plane's blending of premultiplied pixels |
| `commit_count(dev)` | Commits written so far (32 bits, wraps) |
| `latched_count(dev)` | Commits the hardware has latched so far (32 bits, wraps); reaches `commit_count` once a commit's writes are on the panel |
| `set_vsync_callback(dev, cb, user)` | Has `cb` run in the driver's interrupt once a frame |

### The port: `struct ce_twoplane_port`

| Member | Zephyr equivalent in the demo |
| --- | --- |
| `irq_lock()`, `irq_unlock(key)` | `irq_lock`, `irq_unlock`: keeps the vsync callback out |
| `wake(wake_arg)` | `k_sem_give` on the semaphore of the thread that polls the engine; called at every vsync |
| `lookup(lookup_arg, id)` | `fw_comp_get`: the firmware's table of registered buffers (id, frame number, address, width, height, stride, format) |
| `device_ready(dev)` | `device_is_ready` |

### Also missing

- A firmware, build files and board configuration that use this copy of the backend. The demo builds its own copy of it.
- The table of registered buffers and the producer that fills it (the demo's compositor and its link to the A53).
- A way to flash, run and observe a board from this repository, and a test that judges what the panel shows.

### What the host test cannot show

- Real concurrency: the fake runs the vsync callback synchronously, at points the test picks, and defers it while the port's lock is held.
- That a driver's `latched_count` really follows its hardware (the GO bit on the AM62P, ExtDst0's sync busy bit on the i.MX 95), and the timing of latches.
- Memory ordering and caches between the R5F/M7, the display controller and the producer.
