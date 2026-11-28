# M3 roadmap after the first AM62P board session

Written 2026-11-28. This page lists the SoW §9 M3 work (WP4, "AM62P portability regression") that remains once the first AM62P board session is done. It is a plan, not a result. **No hardware claim is made here, and no row is marked done.** The rows live in [open-issues.md](open-issues.md); this page does not change them. Their status is tracked in [sow-traceability.md](sow-traceability.md).

The session report is `docs/m3-am62p-report.md`. It is to come and is not linked here. Until it exists, no document records an AM62P board observation.

## What the first session covers

The first session targets M3-01, M3-02, the host side of M3-05, and the camera rule of X-05. The M3 acceptance conditions in [SoW.md](../SoW.md) §9 are: two-plane composition, correct VSYNC latch tracking, safe frame release, no visible intermediate-state regression, and a recorded difference matrix versus i.MX95. The session plan is [m3-am62p-test-plan.md](m3-am62p-test-plan.md). Its risks and fallbacks are in [m3-am62p-risks.md](m3-am62p-risks.md). The board test and its run route are in [board-test-am62p-r5f.md](board-test-am62p-r5f.md).

Everything below is not in the first session.

## Remaining items

| Item | Needs | Blocked by |
| --- | --- | --- |
| M3-03 | Board scenes for the safety flip and producer failure | A first session that shows the base scenes run |
| M3-05 | The engine linked into a real firmware build | Where the firmware build lives; the producer link |
| AM62P section of platform-capabilities | Text from the source studies and the session report | Plan 462's i.MX8M Plus change to the same file |
| M3-06 | A decision on an optional third target | The M1 result |

### M3-03: safety layer and producer failure under real concurrency

The open issue: priority-layer transitions and producer restart have host tests with a fake driver only. Real concurrency, caches and memory ordering between the R5F, the DSS and the producer are untested.

- **Design:** [m3-03-board-test-design.md](m3-03-board-test-design.md). It is a design only. It adds four scenes (9 to 12) to the eight of the board test. Every number in it is a parameter to choose, not a result.
- **Needs:**
  - the scenes written in `tests/board/am62p_r5f`;
  - a second firmware thread acting as producer, with a mutex the firmware owns;
  - the frame period and the timer resolution of the R5F, read from the board;
  - the camera judge, as in the first session ([camera-check-README.md](../scripts/camera-check-README.md)).
- **Blocked by:**
  - the first session: the eight base scenes must run first, and the real GO-bit behaviour is the input to the timing sweeps ([am62p-dss-gobit-source.md](am62p-dss-gobit-source.md));
  - a producer on the A53 side is outside this repository and is not designed, so only R5F-internal concurrency is in reach;
  - the demo's own locking is not in this repository (an open question in the design).
- **Do not claim:** that host simulation or the fake-driver tests show cache or ordering behaviour. They cannot ([twoplane-backend.md](twoplane-backend.md), "What the host test cannot show").

### M3-05: the link in a real firmware build

The open issue: CI compiles the C backend for the two Arm targets and links it into a freestanding image with `__aeabi_memclr4` resolved. This is host only and nothing is run. The link in a real firmware build is not done. See [ci.md](ci.md) and [twoplane-backend.md](twoplane-backend.md).

- **Needs:**
  - the engine's C API library (`composition-ffi`) built for the R5F target and linked into a Zephyr application;
  - the board test image ([board-test-am62p-r5f.md](board-test-am62p-r5f.md)) as the first place, since it already calls the C API and the backend;
  - the host checks `scripts/check-armv7r-build.sh` and `scripts/check-ffi-r5f-link.sh` as a guard before board time.
- **Blocked by:**
  - the first session, which shows whether the image builds and runs with the link as it is;
  - the location of the producer-side firmware (the demo build is outside this repository);
  - the toolchain on the build node. The Zephyr workspace and SDK are node-local; [check-am62p-board-env.sh](../scripts/check-am62p-board-env.sh) says whether a node can build the image.
- **Caveat:** the host checks show a link, not a run. Only the board shows the run.

### AM62P section of docs/platform-capabilities.md

M3-04 notes that this section says "Not researched yet". The matrix is [am62p-imx95-matrix.md](am62p-imx95-matrix.md).

- **Needs:** a section built from the source studies ([am62p-dss-source.md](am62p-dss-source.md), [am62p-dt-source.md](am62p-dt-source.md), [am62p-r5f-remoteproc-source.md](am62p-r5f-remoteproc-source.md), [am62p-dss-sharing-source.md](am62p-dss-sharing-source.md), [am62p-torizon-boot-source.md](am62p-torizon-boot-source.md)) and from the session's observations, each marked as read or observed.
- **Blocked by:** plan 462, which is adding i.MX8M Plus to the same file. Edit it only after that has landed, to avoid a conflict.
- **Also waits for:** the first session, for every cell that is observed and not read from a source.

### M3-06: optional third target

The open issue: an i.MX RT1170 or RA8P1 target is not started. It is optional until i.MX95 is demonstrated (SoW WP4, last bullet; §5.4).

- **Needs:** a recorded decision, after M1, whether to build it. If yes, the SoW rule applies: document the display topology, plane limits, memory access, synchronisation and a testable board set-up before any backend ([SoW.md](../SoW.md) §5.4).
- **Blocked by:** the M1 result ([m1-imx95-report.md](m1-imx95-report.md)). Not required for M3.

## X-05: camera checks

X-05 asks that the camera check be generic across boards. The first session reuses it on the AM62P ([camera-check-README.md](../scripts/camera-check-README.md)). What stays open after that is making it part of a regression run. Keep the picture-and-log rule: a picture alone cannot exclude a flattened buffer.

## Order

1. First session: base scenes, report, restore Torizon.
2. Observed AM62P column and the matrix ([am62p-imx95-matrix.md](am62p-imx95-matrix.md)); then the traceability rows.
3. M3-05 in the board test image, then the M3-03 scenes.
4. The AM62P section of platform-capabilities, after plan 462.
5. M3-06 decision, after M1.

Steps 3 and 4 do not depend on each other and can run in parallel.
