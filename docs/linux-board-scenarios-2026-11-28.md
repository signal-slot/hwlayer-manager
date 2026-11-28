# Linux board scenarios: A1-A6 on three DRM boards, 2026-11-28

Written 2026-11-28. It holds the results of the runs of rows A1-A6 of [`board-scenarios.md`](board-scenarios.md) on a NanoPC-T6 (RK3588), a Raspberry Pi 5 (BCM2712, `vc4`) and a reTerminal (CM4), with `scripts/board-scenarios.sh` over `tools/drm-harness` and trace emission checked by trace-check. The method is in [`linux-board-scenarios.md`](linux-board-scenarios.md) and is not repeated here.

## Portability evidence only

These runs are portability evidence on Linux DRM boards. **They never count toward M1, M2 or M3**: M1 is the i.MX95 Linux run (plan #453), M2 the i.MX95 Zephyr comparison (plan #455), and M3 the AM62P run (issue #12). A PASS here does not satisfy any acceptance condition of those milestones and is not to be cited as such.

Tags: **observed** (a board run, dated as in 2026-11-28), **read from source** (a repository file, named), **inferred** (a conclusion from the above), **not measured** (the result is not in the reports this page was written from, or the run could not measure it).

This page was written from the handoff notes of the three board items, not from the raw logs. Where a note does not contain a value, the cell says "not measured" with that reason; no value was filled in from memory. The runs are observed 2026-11-28 except where a row says otherwise. Kernel version and tested commit are not in the notes, so they are "not measured" below.

## NanoPC-T6 (RK3588)

Kernel and tested commit: not measured (not in the report). Display manager: observed 2026-11-28, never touched.

| Row | SCENARIO | Host time (UTC) | Trace counts | Event types confirmed | Picture (camera-check) |
| --- | --- | --- | --- | --- | --- |
| A1 | PASS (observed 2026-11-28) | not measured (not in the report) | not measured (not in the report) | not measured (not in the report) | not measured (the report gives no verdict for this row) |
| A2 | PASS (observed 2026-11-28) | not measured (not in the report) | not measured (not in the report) | not measured (not in the report) | not measured (the report gives no verdict for this row) |
| A3 | PASS (observed 2026-11-28) | not measured (not in the report) | not measured (not in the report) | not measured (not in the report) | not measured: two A3 holds have no valid picture (observed 2026-11-28); the other A3 holds are not in the report |
| A4 | PASS (observed 2026-11-28) | not measured (not in the report) | not measured (not in the report) | not measured (not in the report) | not measured (the report gives no verdict for this row) |
| A5 | PASS (observed 2026-11-28) | not measured (not in the report) | not measured (not in the report) | not measured (not in the report) | not measured (the report gives no verdict for this row) |
| A6 | FAIL (observed 2026-11-28) | not measured (not in the report) | not measured (not in the report) | not measured (not in the report) | not measured (the report gives no verdict for this row) |

- observed 2026-11-28: run 2 is the result of record; run 1 is a repeat that supports it.
- observed 2026-11-28: A6 FAILs because the engine does not release the buffers that were on screen when the CRTC was lost.
- observed 2026-11-28: the KVM pictures were taken from the host, timed by markers that the board-side hook wrote, because a hook on the board cannot reach the KVM.
- inferred: the `hold-start` hook may fire about 10 s late in `board-scenarios.sh`; this is inferred from times and not proven.
- observed 2026-11-28: the trace `at` field is 0 in every scenario except recovery.

## Raspberry Pi 5 (BCM2712, vc4)

Kernel and tested commit: not measured (not in the report). Result of record: pass 2, observed 2026-11-28 from 07:51:18Z to 08:05:27Z (host time). The report's earlier pass is not used.

| Row | SCENARIO | Host time (UTC) | Trace counts | Event types confirmed | Picture (camera-check) |
| --- | --- | --- | --- | --- | --- |
| A1 | not measured (per-row verdict not in the handoff) | within 07:51:18Z-08:05:27Z; row time not measured | not measured (not in the report) | not measured (not in the report) | not measured (the report gives no verdict for this row) |
| A2 | not measured (per-row verdict not in the handoff) | within 07:51:18Z-08:05:27Z; row time not measured | not measured (not in the report) | not measured (not in the report) | not measured (the report gives no verdict for this row) |
| A3 | not measured (per-row verdict not in the handoff) | within 07:51:18Z-08:05:27Z; row time not measured | not measured (not in the report) | not measured (not in the report) | not measured (the report gives no verdict for this row) |
| A4 | not measured (per-row verdict not in the handoff) | within 07:51:18Z-08:05:27Z; row time not measured | not measured (not in the report) | not measured (not in the report) | not measured (the report gives no verdict for this row) |
| A5 | not measured (per-row verdict not in the handoff) | within 07:51:18Z-08:05:27Z; row time not measured | not measured (not in the report) | not measured (not in the report) | not measured (the report gives no verdict for this row) |
| A6 | FAIL (observed 2026-11-28) | within 07:51:18Z-08:05:27Z; row time not measured | not measured (not in the report) | not measured (not in the report) | not measured (the report gives no verdict for this row) |

- observed 2026-11-28: A6 FAILs because `composition-drm` never releases the two buffers that were on screen after the loss. The fix belongs to a file held by another plan (#454).
- observed 2026-11-28: the KVM picture is 0-56 s behind the screen; later sessions should hold 60 s or more and take the picture about 50 s in, taking two pictures back to back and judging the second.
- observed 2026-11-28: for A2 and A4, the `hold-start` hook fires `--stop-wait` seconds late, because those runs print no `commit` record for `run_held` in `scripts/board-scenarios.sh` to wait for (read from source in the report, so the cause is also read from `scripts/board-scenarios.sh`).

## reTerminal (CM4)

Kernel and tested commit: not measured (not in the report). Connector `DSI-1`; planes 79 (primary) and 127 (overlay), observed 2026-11-28. Board state afterwards: back as found (observed 2026-11-28).

| Row | SCENARIO | Host time (UTC) | Trace counts | Event types confirmed | Picture (camera-check) |
| --- | --- | --- | --- | --- | --- |
| A1 | not measured (per-row table not in the handoff) | not measured | not measured (not in the handoff) | not measured (not in the handoff) | not measured: no camera or KVM |
| A2 | not measured (per-row table not in the handoff) | not measured | not measured (not in the handoff) | not measured (not in the handoff) | not measured: no camera or KVM |
| A3 | not measured (per-row table not in the handoff) | not measured | not measured (not in the handoff) | not measured (not in the handoff) | not measured: no camera or KVM |
| A4 | not measured (per-row table not in the handoff) | not measured | not measured (not in the handoff) | not measured (not in the handoff) | not measured: no camera or KVM |
| A5 | not measured (per-row table not in the handoff) | not measured | not measured (not in the handoff) | not measured (not in the handoff) | not measured: no camera or KVM |
| A6 | not measured (verdict not in the handoff; the report predicts FAIL for a board without a lease, inferred) | not measured | not measured (not in the handoff) | not measured (not in the handoff) | not measured: no camera or KVM |

- inferred: A6 fails on any board without a lease until the engine frees its buffers or redoes the modeset after its CRTC is turned off, or until the A6 pass rule changes. That needs files held by other plans (#454, #460).
- read from source: `scripts/board-scenarios.sh` skips trace-check on BLOCKED rows.

## Harness and script bugs still seen

- observed 2026-11-28: A6 FAILs on the T6 and RPi5 for the buffer-release reason above (composition-drm, held file; not a harness bug).
- observed 2026-11-28: late `hold-start` hook in `scripts/board-scenarios.sh` (about 10 s on the T6, inferred; `--stop-wait` seconds on the RPi5 for A2 and A4).
- observed 2026-11-28: the trace `at` field is always 0 except in the recovery scenario.
- observed 2026-11-28: `scripts/scenario-expect.py` has no recovery-colour EXPECT, and its `modeset` "changed" comparison does not fit a sink where both modes look the same.
- read from source: trace-check is not run on BLOCKED rows; running it there is a small follow-up.
- observed 2026-11-28: KVM pictures lag the screen (0-56 s on the RPi5); this is a capture-path property, not a harness bug.

## What is left

- inferred: A1-A6 per-row host times, trace counts and event-type confirmations for all three boards are to be filled in from the raw board logs; this page does not have them.
- inferred: folding these results into `board-scenarios.md` and `platform-capabilities.md` and updating X-02 and X-03 in `open-issues.md` is left to the owner of those files.
