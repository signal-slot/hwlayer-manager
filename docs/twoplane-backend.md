# The two-plane backend and what a board test still needs

`backends/twoplane/` holds the engine's backend for a display driver with two planes on one output (`ce_twoplane.h`, `ce_twoplane.c`). It was taken from the MCU firmware of the AM62P demo, where it was written against Zephyr, and the Zephyr calls were replaced by a port table. It is C, uses no C library beyond `<stdbool.h>`, `<stddef.h>` and `<stdint.h>`, and is compiled by the firmware that uses it.

## What is tested, and where

`cargo test -p composition-twoplane` (part of `cargo test --workspace`):

- compiles the backend and `backends/twoplane/tests/c/twoplane_test.c` with the system C compiler, links them with `libcomposition_engine.a`, and runs them. The test's fake driver has shadow registers, a commit count, a latched count and a frame start the test triggers, at any point of the backend's writes, with or without running the vsync interrupt.
- compiles the backend alone with `-ffreestanding -nostdinc` and the compiler's own headers only, checks that it reads no other system header, and that the object needs no symbol other than those a compiler may call for structure copies (`memcpy`, `memset`, `memmove`, `memcmp`).

Both run on the host only. The tests do not compile for Arm. Done by hand once (2026-11-26, clang 23, `-ffreestanding -nostdinc` with clang's own headers, `-O2`), the backend compiled without warnings for `thumbv7em-none-eabihf` (`-mcpu=cortex-m7`) and `armv7r-none-eabihf` (`-mcpu=cortex-r5`). Both objects need one outside symbol, `__aeabi_memclr4`, an Arm EABI helper the compiler calls to clear structures; the toolchain's runtime library (libgcc, compiler-rt) provides it. CI now compiles the backend for both cores and links each into a freestanding image with the engine's static library (job `arm-backend`, `scripts/check-arm-link.sh`, `backends/twoplane/arm/`; see `docs/ci.md`): `__aeabi_memclr4` is resolved there from `compiler_builtins` inside that library, . Nothing is run, on a device or in an emulator.

## What a board test needs and does not have yet

No board run has been made from this repository. Nothing about the backend on hardware has been measured here. `tests/board/am62p_r5f/` is the board test's application for the AM62P's R5F. Its binding to the demo's driver is written, the image has not been built from this repository's `main`, and the test has run only on the host ([board-test-am62p-r5f.md](board-test-am62p-r5f.md)).

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

## Latch semantics and evidence

What the sources say, **read from source** (no board was involved; details, line references and open points are in the studies):

- [`am62p-dss-gobit-source.md`](am62p-dss-gobit-source.md): plane registers are shadowed. Software writes them, then sets one bit, the GO bit (bit 5 of the video port's control register, one per video port), to validate the configuration. The hardware copies the shadow values to the active registers once, and clears GO when it has done so. The Linux `tidss` driver describes the copy as happening at the vertical front porch, before the vertical blank interrupt, and treats "GO reads 1" as "not latched yet". What the hardware does if GO is set again while it is still 1, or if a shadow register is written between GO and the copy, is not answered by any source read; the study marks the torn-configuration and merged-commit cases as inferred.
- [`am62p-dss-sharing-source.md`](am62p-dss-sharing-source.md): when Linux and a remote core both program one video port, they share its GO bit, so one core's commit can latch the other's half-written registers and the latched count no longer matches the commits.

Host tests that pin the backend's latch and release behaviour. They run on a fake driver in which the test decides when a frame start happens. They are **host references, not board results**:

- `backends/twoplane/tests/twoplane.rs`: builds and runs the C test (`tests/c/twoplane_test.c`) with a fake driver that has shadow registers, a commit count and a latched count; also the freestanding-compile check.
- `backends/twoplane/tests/latch_props.rs`: seeded property test of commit/latch accounting: latched count never above commits, a submission presented at most once, a superseded one dropped, every buffer released once, refused calls change nothing.
- `backends/twoplane/tests/m3_03_host.rs`: host simulation of the M3-03 cases (producer stops and restarts, safety flip over an in-flight commit, back-to-back commits, latch timeout).

Not measured: no AM62P board measurement of the GO bit, the latch, or its timing exists yet. Every timing in this section is read from a source or modelled on the host, not measured. The board result will be in docs/m3-am62p-report.md.
