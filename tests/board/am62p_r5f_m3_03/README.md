# AM62P MCU R5F board test: M3-03 scenes 9 to 12 (Zephyr, draft)

Not built for or run on a board; no hardware result.

This is a Zephyr application for the MCU Cortex-R5F of a Verdin AM62P. It
implements scenes 9 to 12 of
[docs/m3-03-board-test-design.md](../../../docs/m3-03-board-test-design.md):
the safety layer and the producer's failure behaviour under real R5F and DSS
concurrency (`docs/open-issues.md` M3-03). Like the eight-scene app in
[tests/board/am62p_r5f](../am62p_r5f/README.md), it uses only the engine's C
API (`crates/composition-ffi/include/composition_engine.h`) and the two-plane
backend (`backends/twoplane`). It prints `SCENE` lines and one `RESULT`
line.

Status (2026-11-28): the sequence has run on the host behind the fake panel
only, with `scripts/check-am62p-m3-03-c.sh`. The result was `RESULT PASS`,
with scene 12b skipped on the host. That is a check of the scenes' logic, not
a board result. The image has never been built.

## Scenes

| Scene | Steps | The firmware judges | The photograph shows |
| --- | --- | --- | --- |
| 9 producer-stop | start; (a) stop between `ce_begin_write` and `ce_finish_write`, restart; (b) stop after `ce_finish_write` with no present; (c) stop after a present for longer than the latch timeout, restart | (a) present refused with `CE_ERR_BUFFER_NOT_READY` and no commit; no commit and no event while stopped; one commit per change after a restart; releases | the last frame stays during each stop: no magenta (half drawn), no yellow (finished, not presented) |
| 10 flip-sweep | an application commit, then a safety flip at an offset from the vsync: coarse offsets over one frame period, fine offsets around the next frame start | one end (presented or dropped) per submission, the second always presented; 2 releases per run; latched = commits; buffer states; a histogram per offset of "before", "mid-write" and "dropped" | the end state only |
| 11 burst | `M3_BURST_N` presents right after a frame start (application buffers X, Y, X, ..., one flip), repeated; then a long burst against the busy and queue-full refusals | within one frame: N-1 dropped, the last presented, the releases the engine owes, commits = writes; a burst that crossed a frame start is counted apart | the last frame of the burst; the other application colour must not stay on |
| 12 cache | 12a: draw, clean the cache, present; 12b: draw white without the cache clean, present | 12a: the clean's return value, and whether its start and length are cache-line aligned; counts as other scenes. No coherence claim | 12a: the new colour. 12b: all white means no difference; the old colour or bands of it mean the buffer is cached and the clean is needed |

Before every photographed state the firmware prints a `HOLD` line with what
the panel should show: the application colour and the side of the red
rectangle. The colours and the rectangle are those of scenes 1 to 8
(`../am62p_r5f/src/scenes.h`), plus `M3_APP_YELLOW`, `M3_APP_MAGENTA` and
`M3_APP_WHITE` in `src/m3_03.h`.

## Parameters

Every timing, sweep and burst parameter is a named constant at the top of
`src/m3_03_scenes.c`. Each is marked "to be chosen on the board" and can be
overridden with `-D` (for example
`west build ... -- -DEXTRA_CFLAGS=-DM3_SWEEP_REPS=10`). The values in the file
are placeholders that suit the host run. The M3 test plan sets the pass
criteria. `prj.conf` sets a 16 KB console buffer; its size is to be chosen on
the board as well.

## Files

- `src/m3_03.h`, `src/m3_03_scenes.c`: the sequence. It needs no operating
  system and no C library.
- `src/main.c`: the Zephyr port (printk, `k_busy_wait`, the uptime clock,
  `sys_cache_data_flush_range`).
- `host/m3_03_host.c`: the host program. It includes
  `../am62p_r5f/host/fake_panel.c` for its fake driver. It adds a fake clock
  (a frame start every 16 ms, each driver write takes 50 us) and checks every
  pixel of the panel at each hold.
- `CMakeLists.txt` uses `../am62p_r5f/src/hw_am62p.c` (the DSS driver
  binding) and `backends/twoplane/ce_twoplane.c` by path. Nothing is copied.

## Build (not done yet)

The build and the run go as for the eight-scene app, described in
[docs/board-test-am62p-r5f.md](../../../docs/board-test-am62p-r5f.md):

```sh
west build -b verdin_am62p/am62p54/mcu_r5f0_0 tests/board/am62p_r5f_m3_03
```

It needs the demo module's DSS driver and the board definition from the
paths that document names (`MCUI_SHOWCASE_DIR` and the others). None of that
is in this repository.

## What the host run cannot show

The host's fake panel latches only at its own frame starts and reads memory
directly. It cannot show real concurrency, the GO bit, or caches. So 12b
prints `SCENE 12 skipped (host): ...` there. The mid-write case of scene 10
appears on the host only because the fake clock lets a frame start fall
inside a driver write.
