# M3-03: design of the safety-flip and producer-failure board tests

Status, 2026-11-28: **a design only. Nothing here is implemented, run or measured.** No scene below exists in code, no board has run any of them, and every number or timing named is a parameter to be chosen, not a result. The host checks named below are existing tests that were read, not re-run for this document.

This is desk work for `docs/open-issues.md` M3-03: safety-layer and producer-failure behaviour under real R5F, DSS and producer concurrency. It extends the board test's eight scenes (`tests/board/am62p_r5f/src/scenes.c` on the owner's branch `work/am62p-board-test`, described in `docs/board-test-am62p-r5f.md` there; not yet merged into `main`) with four more. The scene numbers 9 to 12 below continue that sequence. If the rebase of that branch changes `scenes.c`, this design follows the merged file, not the other way round.

## What the backend and the engine give the tests to hold on to

Read from `backends/twoplane/ce_twoplane.c`, `ce_twoplane.h`, `docs/twoplane-backend.md` and `crates/composition-ffi/include/composition_engine.h`.

- Every call that writes a plane is one commit. `hw->commit_count()` counts them, `hw->latched_count()` follows the hardware (on the AM62P the GO bit; whether a driver's counter does follow it is exactly what the host test cannot show). A commit is latched when `latched_count` has reached the `commit_count` read after the commit's writes (`reached()`, wrap-safe).
- The backend reports `replace_pending = 1`: a second `ce_present` before the first latched rides on the first. The engine then reports `CE_EVENT_DROPPED` for the first submission, releases its buffers (the drop comes before the release), and `CE_EVENT_PRESENTED` for the second when the hardware latches. If the hardware latches while the replacement's writes are in flight, the first is `PRESENTED`, never `DROPPED`, and the panel then shows part of the replacement (`adv_replace_latch_during_writes`: the contract is about the reported latch, not the pixels).
- The backend refuses a commit with `CE_ERR_BACKEND_BUSY` before writing anything if the previous latch is not yet reported. The caller polls and tries again.
- The engine has no clock. `ce_set_latch_timeout(ticks)` plus `ce_advance_time(now)` yields `CE_EVENT_LATCH_TIMEOUT` once, with the submission still pending and its buffers in use; recovery is the caller's.
- A buffer that is being drawn (`ce_begin_write`) can never be put on a plane: `ce_present` returns `CE_ERR_BUFFER_NOT_READY` and the driver is not touched. A buffer that is submitted or scanning cannot be unregistered (`CE_ERR_BUFFER_IN_USE`).
- The safety plane is never switched off, and only the driver's two safety buffers are accepted; a frame without a safety layer is refused (scenes 6 to 8 already check this).

### The producer, and what "concurrent" can mean here

The engine's `ce_present`, `ce_poll_event` and the buffer calls are called from one context at a time; the backend's comments call that "the engine lock" and `ce_twoplane_port` only protects state shared with the vsync interrupt. The board test today is one thread plus the vsync callback. In the demo firmware the producer is the A53 side, reached through a link that is not in this repository (M3-05). So the concurrency a board can show in this repository is:

1. the vsync interrupt against the engine's thread (already present in every scene, never stressed);
2. a second thread of the firmware acting as a producer (drawing buffers, calling `ce_begin_write` and `ce_finish_write`) and, separately, a thread that presents and polls, each serialised with the other by the firmware's own mutex. The design below assumes this; whether the mutex is acceptable (the demo's own locking is not in this repository) is an open question for whoever writes the scenes;
3. the DSS reading memory while the R5F or a producer writes it.

A true second producer on the A53 is **not** designed here. A safety flip issued from outside the engine (a call to `hw->safety_flip` that does not pass `ce_present`) is also outside what the engine promises: the adversarial host test drives the backend that way (`out_of_band_frame`) only to check the backend's own counters.

All timing in the scenes is aligned to the vsync callback: the firmware has `backend.vsyncs` (incremented in the interrupt) and the port's `wake`, so a scene can issue a call a chosen number of microseconds after a vsync and sweep that offset over one frame period. The frame period and the timer resolution on the R5F are to be read, not assumed. Panel timings come out of the board run (`docs/m3-am62p-report.md`, not yet written).

## What each kind of check can and cannot say

**Firmware.** Judges what the firmware can read: the status of every call, `commit_count` and `latched_count` before and after, the order and number of engine events (presented, dropped, released, latch timeout), buffer states from `ce_buffer_state`, and `backend.vsyncs`. It cannot read what the DSS scanned, and a latched count only says the hardware took the writes at a frame start, not what it read from memory afterwards. Output goes to the trace buffer as `SCENE`/`RESULT` lines as in the existing scenes; each new scene prints the same fields plus the new ones named below.

**Camera.** Judges flat colours and rectangles in photographs taken during the hold with `scripts/camera-check.py` (and `scripts/scenario-expect.py` if its expectations fit). It judges **the held end state only**. A photograph with an exposure of the order of a frame period or longer integrates over several frames and a rolling shutter reads rows at different times, so one photograph cannot show that no intermediate state was ever on the panel. A single frame of an intermediate state (half old, half new) can be caught only by chance, and its absence in N photographs is not evidence of its absence at all. "No visible intermediate state" (the wording of M3-03) can therefore be checked by the camera in two weaker ways: (a) the end state after the stress is the expected one, with no stale or mixed content; (b) with a camera whose frame rate and exposure are measured (not yet) to resolve single panel frames, a burst taken during the stress is scanned for frames that match neither the old nor the new state. Whether the AM62P camera can do (b) is unknown; the probe item for the camera has not reported. Without it, the camera check of the stress scenes is (a) only, and the report must say so.

**Host.** The fake driver in `backends/twoplane/tests/c/twoplane_test.c` and `adversarial_test.c` (run by `cargo test -p composition-twoplane`) and the fake panel of `board_scenes_behind_a_fake_panel` (`backends/twoplane/tests/twoplane.rs` on the owner's branch) check the sequence logic and the scene's own judgement with a latch at any chosen point of the backend's writes. They are synchronous: the callback runs where the test says, and is deferred under the port's lock. They cannot show real concurrency, a real latch, or caches.

Order of work for each new scene: write the host check first (a scene that fails on the fake panel is a bug in the scene or the backend, found without a board), then the firmware check, then the camera check.

## Scene 9: producer stops mid-sequence and restarts

*What is meant.* The producer is the code that draws buffers and releases them to the engine (`ce_begin_write`, draw, `ce_finish_write`). It stops in three places, each followed by a restart: (a) after `ce_begin_write` of an application buffer and before `ce_finish_write` (the buffer stays `WRITABLE`, half drawn); (b) after `ce_finish_write` but with no `ce_present` (a ready buffer nobody shows); (c) after `ce_present`, with no further frames, for longer than the latch timeout. Restart means drawing and presenting again.

*Expected behaviour, from the code.* The safety plane keeps scanning its buffer, the application plane keeps its last frame. A present that names the half-drawn buffer is refused with `CE_ERR_BUFFER_NOT_READY` with no write to the driver. A restart in (a) completes the write and the next present commits one change. While stopped the engine reports nothing (`poll_event` empty) except what the latch timeout reports.

*Firmware check.*

- (a): `ce_present` naming the buffer returns `CE_ERR_BUFFER_NOT_READY`; `commit_count` is unchanged; `ce_buffer_state` is `WRITABLE`; no event. After `ce_finish_write` and a present: one commit, latched, presented, the previous application buffer released.
- (b): no commit and no event over a stated number of frames (`backend.vsyncs` advances, `commit_count` does not).
- (c): with `ce_set_latch_timeout` set and `ce_advance_time` driven from the timer: after the last present has been presented there is no event and the count stays (a latched frame is not pending). To also exercise `CE_EVENT_LATCH_TIMEOUT` itself the test needs a commit that does not latch: that cannot be made on the real hardware from the engine's calls, so it stays a host-only check (`adv_latch_timeout_cycle`). The board scene may use a deliberately wrong state only if the driver offers a way to withhold the GO bit; none is known. Not designed.
- After restart: the commit count rises by the number the sequence requires, `latched == commits`, one `PRESENTED` per submission, releases in the order the host tests expect (presented before the release it permits).
- Records: commits, latched and vsync counts at each stop and restart; number of events seen during each stop (expected 0).

*Camera check.* During each stop, photographs show the unchanged previous end state (same colours, same rectangle positions as before the stop): no black, no stale buffer, no half-drawn frame. After the restart, the new frame as `scenes.c` would paint it (a third colour for the application plane, so that stale and new differ). The half-drawn buffer in (a) should be painted with a visibly different colour in its first half so that a leak onto the panel could be seen; this relies on the buffer never reaching the plane, so a clean photograph is the expected result, not a strong one.

*Host check first.* Existing: `adv_replace_pending` (present of a `WRITABLE` buffer is `CE_ERR_BUFFER_NOT_READY` with `fake.writes` unchanged, `ce_unregister_buffer` of an in-use buffer is `CE_ERR_BUFFER_IN_USE`), `engine_redraw`, `adv_latch_timeout_cycle`, `adv_poll_during_pending`, `engine_frames`. To add on the host before the board: the three stop points as a sequence on `board_scenes_behind_a_fake_panel`, with the fake panel judging the held frame at each stop.

## Scene 10: safety flip issued during an in-flight app commit

*What is meant.* An application commit (`set_app_buffer`, then `enable_app_plane` if the plane was off) has been written and has not latched; a frame that needs a safety flip is presented before the frame start. Through the engine this is a `ce_present` with a changed safety buffer on top of a pending one, so `replace_pending` applies. The sweep: present the second frame at offsets across the frame period relative to the vsync, including the neighbourhoods of the frame start itself, so that the latch lands before, between and after the second commit's writes.

*Expected behaviour, from the code and the host tests.* If the latch comes before the second present, the first is `PRESENTED`, then the second is an ordinary commit. If it comes after, the first is `DROPPED`, its buffers are released, and both commits reach the panel together as the second's `PRESENTED`. If the latch falls between the second commit's writes the first is `PRESENTED`, and the panel may show the application change without the flip, or the reverse, for one frame: which of these the DSS shows depends on shadow-register behaviour the host fake only models. The board shows whether the AM62P's GO bit makes the writes of one commit atomic against a frame start that falls in the middle of them. That is the open question, and it is the one for which the camera is least able to answer.

*Firmware check.* For each offset, over a number of repetitions chosen later:

- every `ce_present` returns `CE_OK` or `CE_ERR_BACKEND_BUSY` (the latter then polled and retried; the number of retries is counted, not hidden);
- each submission ends with exactly one of `PRESENTED` or `DROPPED`, never both and never neither within the timeout;
- the number of `PRESENTED` plus `DROPPED` equals the number of successful presents;
- `commit_count` rose by the expected writes, `latched_count` reaches it within the timeout and never exceeds it;
- releases: a buffer is released only after the `PRESENTED` of the submission that replaced it, or at the `DROPPED` of one that never latched; each buffer released exactly once; no buffer is both `SCANNING` and released;
- the last submission's buffers are the ones the planes were told to scan (`ce_buffer_state` `SCANNING`);
- the offset at which each case occurred, as a histogram line (counts of "latched before", "dropped", "latch mid-write" as the backend can tell them from `backend.pending` and the events). Which cases are hit at all is a result, not a requirement.

*Camera check.* End state only: the safety rectangle is where the last frame put it (left or right), the application colour is the last frame's, and the two planes are consistent with each other (the end state of a flip together with an application change is one of the two defined states, never one plane new and one old). Intermediate states are not judged unless the camera measurement above allows burst checking; then, a burst around the stress is scanned for a frame in which the safety rectangle and the application colour disagree with both the old and the new state.

*Host check first.* Existing and relevant: `engine_replace_pending`, `adv_replace_pending`, `adv_replace_latch_during_writes`, `adv_latch_between_presents`, `adv_latch_during_plan_check`, `engine_latch_during_writes_isr` and `_no_isr`, `ops_latch_behind_unreported`, `ops_busy`, `engine_busy_race`, `adv_own_latch_deferred_isr`, `adv_flip_error_swallowed`, `adv_enable_error_swallowed`. The fake can latch at any point of the backend's writes, with and without the interrupt, so every ordering above is already covered logically. The new host test is the sweep itself on the fake panel: the same offsets as units of fake time, to check the scene's counting and the invariants above before the board.

## Scene 11: back-to-back commits within one frame

*What is meant.* Several `ce_present` calls between two frame starts, each a different frame (application buffer 1, 2, then 1 again, and a flip in between). The backend merges them (`replace_pending`); only the last one latches. This is the engine's ordinary path for a fast producer, and the board shows whether the driver's `commit_count` and `latched_count` stay consistent when several commits wait for one GO bit.

*Expected behaviour.* All but the last submission are `DROPPED` with their buffers released; the last is `PRESENTED` at the frame start; `latched_count` jumps by the number of commits at once; the buffer that is both in a dropped frame and in the last one is not released. The producer is not allowed to reuse a buffer until its release event; the scene's own producer thread obeys that and a negative control does not (see below).

*Firmware check.*

- The number of presents issued between two `backend.vsyncs` increments is recorded (the interrupt count before and after the burst) so that a burst that crossed a frame start is identified rather than assumed to be within one frame. A burst that crossed is counted as a different case.
- For a burst of N within one frame: N-1 `DROPPED`, 1 `PRESENTED` (the last), no `PRESENTED` for a dropped one, `commit_count` rose by the writes of the N frames, `latched_count` equals it after the next frame start, releases are the buffers of the dropped frames not carried into the last one, each once.
- A burst of N larger than a figure to be chosen, to see `CE_ERR_BACKEND_BUSY` and `CE_ERR_EVENT_QUEUE_FULL` behaviour on the real interrupt timing (the host test `adv_event_queue_full` shows the status; whether the board's queue limit is reached depends on the engine's built queue size, which this design does not read). Recorded, not required.
- Wrap: `commit_count` and `latched_count` are 32 bits and wrap; the host test `engine_wrap`, `ops_wrap` and `adv_wrap_rotation` cover this. The board does not need to reach it and the design does not try to.
- Negative control of the producer rule, only on the host: a producer that draws into a buffer that has been submitted is refused by the engine (`CE_ERR_BUFFER_IN_USE` from `ce_begin_write`); the board scene does not draw into a submitted buffer on purpose, because the only thing it could show is torn pixels and the engine already refuses it.

*Camera check.* End state only, the last frame of the burst. To make the dropped frames visible if they were shown, the intermediate frames use colours that differ from the last (for example blue, green, then a third colour), so that a photograph showing one of them is a failure. Because the photograph is taken after a hold, it can only show that the panel did not stay on a dropped frame. A burst photograph of the stress (if the camera allows) can show a dropped frame appearing for a single panel frame, which would mean the driver took a commit that the engine had reported as dropped.

*Host check first.* Existing: `engine_replace_pending`, `adv_replace_pending`, `adv_duplicate_latch`, `adv_unknown_submission_latch`, `adv_poll_during_pending`, `adv_event_queue_full`. To add: the burst of N at a fake frame start, with the fake panel judging the final frame, and the same burst split by a frame start in the middle.

## Scene 12: cache maintenance for buffers the DSS reads

*What is meant.* The R5F writes pixels into memory that the DSS reads by DMA. Whether the memory is cached by the R5F, and so whether the firmware must clean the cache after drawing and before the frame start (the existing scenes already call `env->flush(base, size)` after every fill), depends on the MPU attributes of the region the firmware places the buffers in. This is not read yet: the Zephyr configuration and the linker placement of the image on the branch are the places to read, and the DSS driver of the demo for what it does at its own side. Until that is read, the design does not say whether the scenes' flush is required, redundant or wrong for the region.

The scene is a pair of controls around a normal drawing:

- 12a, normal: draw, `env->flush`, present (the existing behaviour). Expected: the new content.
- 12b, negative control: draw a new colour into a buffer without the flush, then present. Expected, if the region is cached write-back: the panel shows the old content or a mixture of old and new, at cache-line granularity. This scene exists only to show that the camera and the firmware can see a missing flush at all; if the region turns out to be uncached, 12b equals 12a and the result is "no difference", which is also an answer.
- 12c, the other direction: where the buffer is written by another agent than the R5F (the A53 producer in the demo) the R5F-side cache must not hold stale lines of it. In this repository's test the only other writer is the DSS, which writes nothing here, so 12c is not a scene of this design and is listed as M3-05's.

*Firmware check.* The firmware can say: the status, commits and latched counts as in the other scenes (which tell nothing about caches: a latch is the registers, not the memory), and a read-back after the flush of a sample of words from the buffer through the same cached mapping (which would match in every case and prove nothing). What is a firmware check is the cache operation's own result: the return value of the flush call and the address and length passed (aligned to the cache line size the R5F reports). Cache coherence between the R5F and the DSS cannot be judged by the firmware alone. If the DSS has a per-frame CRC or a data-capture unit the firmware could read, that would be a firmware-visible check; whether the AM62P DSS has one is not known here and is for the tidss source study to say. Nothing is claimed.

*Camera check.* This is the scene where the camera is the main check. 12a: the new content with the exact colours and rectangle positions the other scenes expect. 12b: a difference from 12a at the cache-line pattern, judged as "not the full new colour" (a flat colour test fails if the old and new content mix); the expected failure mode is a band or a pattern of the old colour, which a flat-colour check can detect and not describe. The photographs and the check's output are the record; whether 12b differs from 12a is reported as observed, with the region's attributes next to it.

*Host check first.* The host fake panel reads the buffer directly, so it cannot show a missing flush. What it can check: that the scene calls `env->flush` with the buffer's base and `height * stride` bytes after every fill (already so in `fill_safety` and `fill_app`), by having the host `flush` record its calls and compare them with what each scene drew. Add that check (a draw without a flush on the host fails the scene) before the board scene; that catches a forgotten flush in the scene code, which is the cheaper bug to find first. It says nothing about the hardware.

## Whole-scene summary

| Scene | Firmware check | Camera check | Host check first |
| --- | --- | --- | --- |
| 9 producer stops and restarts | refusal of an unfinished buffer, no commit while stopped, counts and events after restart | held end state unchanged during the stop, new frame after the restart | `adv_replace_pending`, `engine_redraw`, `adv_latch_timeout_cycle`; add the three stop points on the fake panel |
| 10 safety flip during an in-flight app commit | one terminal event per submission, release order, counts, offset histogram | consistent end state; mixed-state burst scan only if the camera can resolve frames | `adv_replace_latch_during_writes`, `adv_latch_between_presents` and others; add the offset sweep |
| 11 back-to-back commits in one frame | N-1 dropped, one presented, one release per buffer, vsync count across the burst | end state is the last frame; dropped colours must not appear | `engine_replace_pending`, `adv_duplicate_latch`; add burst at a frame start and across one |
| 12 cache maintenance | flush calls' arguments and returns; counts (no coherence claim) | flat colour of 12a; difference or none for 12b | flush-call recording on the fake; nothing about hardware |

## What this design does not decide

- The mutex and thread layout of the producer thread against the presenting thread (above): chosen when the scenes are written, from how the demo firmware locks.
- The camera's capability for burst checking, and the exposure and hold time for the photographs; to come from the camera probe and the first run.
- The MPU attributes of the buffers' region and the DSS's data-capture ability; to be read from the Zephyr configuration of the board test and the tidss source study.
- The A53 as a real producer, the link to it, and the safety layer's behaviour when that producer dies: M3-05 and the remaining part of M3-03.
- Pass criteria and numbers (repetitions, offsets, timeouts, burst sizes). They belong in the M3 test plan and are not set here.

No code was changed for this document, and no device was used.
