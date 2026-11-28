# M3-03 test plan: safety flip and producer failure on the AM62P R5F and DSS

Status, 2026-11-28: **a plan. Nothing in it has been implemented, run or measured.** No board, camera or panel was used for this document, and it makes no hardware claim. Every number below is a parameter the firmware and the board session will use, not a result. Where a number comes from a file, the row says `read from source` and names the file. Where it is a choice or an estimate, the row says `inferred` and gives the reason. No value is `measured`: the report of the board run (`docs/m3-am62p-report.md`, not written yet) is the only place where one can become that.

This plan settles what [`m3-03-board-test-design.md`](m3-03-board-test-design.md) left open: pass criteria, repetitions, offsets, timeouts and burst sizes. The design says what each scene is and what it can and cannot show. This plan does not repeat that and does not change it. The format follows [`m3-am62p-test-plan.md`](m3-am62p-test-plan.md): status marks, console lines, per-test pass criteria, a picture-and-log rule, order of work. The open issue is M3-03 in `docs/open-issues.md`.

The plan holds no board address, host name, user, credential, serial path or serial number. Where a command needs the board it says `<board>`.

## 0. Status marks and tags

| Mark | Meaning |
| --- | --- |
| main | exists on `main` today |
| planned | does not exist; the interface here is the intent |
| `read from source` | the value is stated by a file of this repository or by a source study in `docs/` |
| `inferred` | a choice or an estimate; the reason is given |

| Piece | Status |
| --- | --- |
| The eight scenes, `tests/board/am62p_r5f/src/scenes.c`, `scenes.h`, `main.c`, `hw_am62p.c`, `host/fake_panel.c` | main |
| Scenes 9 to 12 of the design, their firmware, their host checks | planned (§10) |
| `scripts/camera-check.py`, `scripts/am62p-scenes.py` (`expect`, `summary`) | main; `am62p-scenes.py` knows scenes 1 to 8 only, so scenes 9 to 12 need an extension (§10) |
| `scripts/am62p-r5f-run.sh` | planned (another item of the plan) |
| The host simulation of the safety flip and producer failure on two-plane | planned (another item; this plan does not duplicate it, §10) |

## 1. What this plan decides, and the unknowns it keeps

Decided here: the repetitions, the offsets, the burst sizes, the timeouts, the console line formats, the photographs, and a verdict rule for each test.

Not decided here, and kept as unknowns the firmware or the board session must fill in:

- **The frame period `T`.** The panel's mode is not read by any document (`docs/am62p-dss-source.md` records the driver's limits, not the Verdin panel; its device-tree rows say no DSS node is enabled in the files read). Offsets are therefore stated as fractions of `T` in permille, and the firmware measures `T` itself (§3). Where an absolute figure is needed for an estimate this plan uses 16.7 ms (60 Hz) and marks it `inferred`.
- **The timer resolution of the R5F.** Not read. Fine offsets (§4.2) are given in microseconds and the firmware refuses any offset smaller than two timer ticks (§3).
- **The trace buffer's size and wrap behaviour.** `docs/am62p-r5f-remoteproc-source.md` §3 says the reader stops at the first NUL byte and that whether the Zephyr RAM console wraps is not read. The plan keeps the console output small (§3.3) and the host polls often.
- **The MPU attributes of the buffers' region** (scene 12) and **the camera's frame rate and exposure** (burst checks of scenes 10 and 11). Both are unknown today; the plan gives the rule for each case.

Verdicts. Each test ends in one of **PASS**, **FAIL** or **INCONCLUSIVE**. INCONCLUSIVE means the test ran correctly and the stress did not reach the case it exists for (for example no trial of scene 10 hit the "dropped" case). It is not a pass, and the report states what is not shown.

## 2. The tests

| Test | Design scene | Parts |
| --- | --- | --- |
| T9 | Scene 9, producer stops mid-sequence and restarts | 9a half-drawn buffer, 9b finished buffer never presented, 9c presented then silence |
| T10 | Scene 10, safety flip issued during an in-flight application commit | offset sweep |
| T11 | Scene 11, back-to-back commits within one frame | burst sizes, within-frame and split bursts |
| T12 | Scene 12, cache maintenance for buffers the DSS reads | 12a drawn and flushed, 12b drawn and not flushed |

Every test starts from the same baseline **B0**: the state of scene 2 (application plane on, blue; safety rectangle on the left, red). The firmware re-establishes B0 through the engine before each test (its commits are counted in the test's first line as `setup`, not in the test's delta). All four tests run once per image, in the order T9, T10, T11, T12, after the eight scenes and their `RESULT` line. Reason (`inferred`): the eight scenes are the baseline evidence; a failure in the new tests must not hide a failure of the old ones.

**Concurrency decision.** The tests use one thread and the vsync interrupt. A second producer thread with a mutex is not part of this plan. Reason (`inferred`): the design notes that the demo's own locking is not in this repository, so a mutex layout chosen now would test the test. The interrupt is the concurrency that `backends/twoplane` documents (`ce_twoplane_port` protects state shared with it). A producer thread is a later extension, listed in §10.

## 3. Common timing, console and measurement rules

### 3.1 Timing parameters

| Parameter | Value | Tag | Reason |
| --- | --- | --- | --- |
| `T`, the frame period | measured by the firmware at the start of T9 as `1000 ms / (vsyncs counted in 1000 ms)`, printed as `T_us` | inferred (the method); the value is a board result | The panel mode is not read. `backend.vsyncs` is incremented in the vsync interrupt (design, `scenes.c`). |
| Placeholder for estimates only | 16.7 ms | inferred | 60 Hz. Used only to estimate run time in §8. |
| Offset origin | the vsync callback (`backend.vsyncs` increments, the port's `wake`) | read from source (`docs/m3-03-board-test-design.md`: "aligned to the vsync callback") | |
| Where the DSS takes the new values | in the vertical front porch, before the vsync interrupt: the page-flip code in `drivers/gpu/drm/tidss/tidss_crtc.c` says new settings are taken at VFP and the GO bit clears then, "before the vblank interrupt" | read from source (`docs/am62p-dss-source.md`, row "Latched count vs commits", Linux tidss, not the R5F driver) | So the latch happens at a phase slightly **before** the callback, `T - d_vfp`. `d_vfp` is the front porch time and is not known (porch limits are in the same document; the panel's value is not). Offsets close to `T` therefore cover the latch (§4.2). Whether the R5F driver's GO bit behaves the same is `inferred`. |
| GO bit | the VP control register's GO bit, shadowed overlay and plane registers, per video port | read from source (`docs/am62p-dss-source.md`) | |
| Timer tick | read by the firmware (`k_cyc_to_ns` or equivalent) and printed as `tick_ns` | inferred | Not read. Offsets below two ticks are refused (`SKIP`, counted, §3.3). |
| Wait for a vsync | `wait_wake(ms)` with `ms = 4 * T` as the cap | inferred | Four frames is long past any healthy wait and short against `TEST_HOLD_MS`. |

The firmware places a call at **phase** `p` (permille of `T`, 0 to 999) by waiting for the vsync callback and then busy-waiting `p * T / 1000`. A call whose phase was missed by more than one tick is counted as `late` and the trial is repeated (at most 3 times per trial, then `SKIP`).

### 3.2 Timeouts

| Name | Value | Tag | Reason |
| --- | --- | --- | --- |
| Presented bound | 1000 ms from the submit | read from source (`docs/board-test-am62p-r5f.md`, `docs/m3-am62p-test-plan.md` §2.1 criterion 3) | The existing bound; kept so the same word means the same thing in all twelve scenes. |
| Latch timeout, `ce_set_latch_timeout` and `ce_advance_time` | 200 ms, driven from the firmware's timer with `ce_advance_time(now_ms)` | inferred | About 12 frames at 60 Hz: it does not fire on a healthy GO bit, and it fires well inside the presented bound. **On healthy hardware any `CE_EVENT_LATCH_TIMEOUT` in T9 to T12 is a failure** (§5 to §7). |
| `CE_ERR_BACKEND_BUSY` retry | wait one vsync (`wait_wake(T)`), try again, at most 3 retries per present | inferred | The backend refuses until the previous latch is reported (design); that report comes at the latest one frame later, plus the interrupt. Three retries is three frames. More is a failure. |
| Settle after a trial | until every submission of the trial has `PRESENTED` or `DROPPED`, at most 1000 ms | read from source (presented bound) | |
| Hold for photographs | 5000 ms, `TEST_HOLD_MS` | read from source (`tests/board/am62p_r5f/CMakeLists.txt`) | The existing value; the camera plan of `docs/m3-am62p-test-plan.md` §5 needs three pictures per state inside one hold. |
| Short hold (repetitions without photographs) | 1000 ms | inferred | At least five latch timeouts, so a pending latch timeout would show. |
| Host trace poll | at most 1 s between reads, each read stored with the host's time | inferred | The buffer is a ring of unknown size (§1); a short interval keeps a line from being overwritten between reads. |
| Host watchdog | 300 s without a new trace line, or 300 s after the last scene's start | inferred | The estimated run is about 100 s (§8); three times that means the image is stuck. A watchdog stop is FAIL of the test that was running. |
| `rproc` stop | up to 5000 ms (shutdown handshake) | read from source (`docs/am62p-r5f-remoteproc-source.md` §2 row C) | Applies to the restore (§9), not to any test. A second image needs a reboot. |

### 3.3 Console lines

Four kinds of line, all `printk`-like (`%s %d %u %x` only, as `scenes_env.log`):

1. `SCENE <n> <name>: ... : ok|FAIL`, **one per part**, in the family of the existing line, so `scripts/am62p-scenes.py summary` can read it (once extended, §10). The parts are named `producer-stop-a`, `producer-stop-b`, `producer-stop-c` (scene 9), `flip-during-commit` (10), `burst` (11, one line per burst size and class), `cache-flushed` and `cache-unflushed` (12).
2. `STEP <n> <name>: ...`, printed **only** for a trial that broke an invariant, at most the first 8 per part, then a counter. A healthy run prints no `STEP` line. This keeps a 300-trial sweep from filling the trace buffer.
3. `HIST <n> <name>: key value ...`, one per offset or per burst class: counts of the outcomes (§5, §6).
4. `RESULT M3-03 PASS|FAIL|INCONCLUSIVE (<parts failed>, <parts inconclusive>)`, printed once, after the last part. The existing `RESULT PASS (8 scenes, ...)` line is not changed and not repeated.

Trace budget: at most 60 lines and about 6 KiB for T9 to T12 together on a healthy run (`inferred`; the buffer's size is not read, and the report states it once read from the image's resource table). A line that would exceed the budget is dropped and counted, and the final `RESULT M3-03` line then says `trace_dropped <k>`; a non-zero count makes the run INCONCLUSIVE at best.

Common fields of a `SCENE` line, written `<common>` below:

```
status <s> (expected <s>), commits <c> latched <l> (+<d>, expected +<d>), events <e> latch_timeouts <t>
```

## 4. Offsets, repetitions and burst sizes: the rules behind the numbers

### 4.1 Repetitions

The repetition counts follow one rule (`inferred`): with `n` trials of a case and zero failures, the 95% upper bound on the failure rate is about `3/n` (the "rule of three"). A hazard that occurs in a fraction `q` of trials is missed with probability `(1 - q)^n`. For `n = 20`: `q = 0.2` is missed 1% of the time, `q = 0.1` is missed 12% of the time, and the bound on an unseen failure is 15%. For `n = 30` the bound is 10%. The plan uses 20 per offset (T10), 30 per burst case (T11), and says in the report what the bound is. It does not claim more. A race that happens in one trial in a thousand is outside what this plan can show; that is stated in §11.

Deterministic cases (T9, whose path the host tests already cover) get 3 repetitions at three phases, not a statistical count.

### 4.2 Offsets for T10

The first present of a trial is issued at phase 50 permille of the frame in which the trial starts. The second present is issued at the offsets of the table below, measured from the same vsync callback, so "offset" is the time between the two presents' phase origin. Offsets 1 to 7 are fractions of `T`; offsets 8 to 13 are given as `T - d` in microseconds to land around the inferred latch point (§3.1: before the callback by the front porch time, `d_vfp` unknown); offset 14 is in the next frame.

| # | Offset | Tag | What it covers |
| --- | --- | --- | --- |
| 1 | 100 permille | inferred | second present early in the frame, long before the latch |
| 2 | 250 permille | inferred | |
| 3 | 500 permille | inferred | |
| 4 | 750 permille | inferred | |
| 5 | 900 permille | inferred | |
| 6 | 950 permille | inferred | |
| 7 | 980 permille | inferred | close to the end of the frame |
| 8 | `T - 2000 us` | inferred | the latch point is probably within a few front-porch widths of the callback; a log-spaced approach (2 ms down to 50 us) is the cheapest way to cross it without knowing `d_vfp` |
| 9 | `T - 1000 us` | inferred | |
| 10 | `T - 500 us` | inferred | |
| 11 | `T - 250 us` | inferred | |
| 12 | `T - 100 us` | inferred | |
| 13 | `T - 50 us` | inferred | skipped (counted) if below two timer ticks |
| 14 | `T + 20 permille` | inferred | after the callback: the first commit is already latched, the second is an ordinary commit ("latched before") |

14 offsets times 20 repetitions is 280 trials. Reason for 20: §4.1. Reason for the spread: the hazard of interest is the latch landing before, between and after the second commit's writes (design, scene 10); the fraction offsets show the two safe sides, the microsecond offsets look for the boundary.

### 4.3 Burst sizes for T11

| Class | N | Start phase | Repetitions | Required | Tag |
| --- | --- | --- | --- | --- | --- |
| within-frame | 2 | 100 permille and 400 permille | 30 each | yes | inferred |
| within-frame | 3 | 100 and 400 permille | 30 each | yes | inferred |
| within-frame | 8 | 100 and 400 permille | 30 each | yes | inferred |
| split by a frame start | 3 | 900, 950, 980 permille | 20 each | yes | inferred |
| split by a frame start | 8 | 900, 950, 980 permille | 20 each | yes | inferred |
| large | 32 | 100 permille | 10 | recorded | inferred |
| large | 64 | 100 permille | 10 | recorded | inferred |

Reasons: N=2 is the smallest merge; N=3 is the design's example (application buffer 1, buffer 2, buffer 1 again, with a flip in between); N=8 is a producer that outruns the display by 8 to 1. The event queue holds `MAX_EVENTS = 4 * (2 * 16 + 1) + 16 = 148` events (read from source: `crates/composition-core/src/limits.rs`). A dropped submission produces a dropped event and releases, so about 3 to 4 events per present (`inferred` from the design's event order); 32 stays below the queue limit when polled, 64 is meant to reach it when the producer does not poll inside the burst. `CE_ERR_EVENT_QUEUE_FULL` at N=64 is recorded and not required (design: "Recorded, not required"). Whether a burst of N stays within one frame depends on how long a `ce_present` takes on the R5F, which is not known; the vsync count before and after each burst decides the class (§6). A "within-frame" burst that crossed a frame start is moved to the "split" class and counted as such, not discarded.

## 5. T9: producer stops mid-sequence and restarts

*Scenario.* Three sub-tests (design scene 9). Stop points: **9a** after `ce_begin_write` of an application buffer, half drawn (the first half of the buffer is painted with a marker colour, magenta `0xF81F` in RGB565, `inferred`), before `ce_finish_write`; **9b** after `ce_finish_write`, no `ce_present`; **9c** after a `ce_present` that was presented, no further call. Restart: finish the write if open, then `ce_present` a frame with a new application colour (yellow, `0xFFE0`, `inferred`: a third colour, so stale and new differ).

*Repetitions.* 3 per sub-test, at stop phases 100, 500 and 900 permille (`inferred`: three phases of the frame, deterministic path). The first repetition holds 5000 ms and is photographed; repetitions 2 and 3 hold 1000 ms and are not.

*Stop duration.* The stop lasts the hold (5000 ms or 1000 ms), which is at least five latch timeouts. During the stop the firmware does nothing but wait; the vsync interrupt keeps running.

*Expected console lines*, one per sub-test, after its three repetitions (`R` is the repetitions run, `V` the vsyncs counted):

```
SCENE 9 producer-stop-a: <common>, state writable (expected writable), vsyncs +<V> (min <M>), restart status 0 (expected 0), restart commits +1 (expected +1), presented yes, released 1 (expected 1), reps 3: ok
SCENE 9 producer-stop-b: <common>, vsyncs +<V> (min <M>), restart status 0 (expected 0), restart commits +1 (expected +1), presented yes, released 1 (expected 1), reps 3: ok
SCENE 9 producer-stop-c: <common>, vsyncs +<V> (min <M>), reps 3: ok
```

In 9a the `<common>` block is the **stop**: `status -33 (expected -33)` (`CE_ERR_BUFFER_NOT_READY`, read from source: `composition_engine.h`), `+0 (expected +0)` commits, `events 0`. In 9b and 9c the stop has no engine call, so `status` is `0 (expected 0)` for the last call before the stop and `+0` commits during it. `M` is `0.9 * hold_ms / T`, rounded down (`inferred`: 10% allowance for the measurement of `T` and the phase of the first and last frame; its purpose is to show the interrupt kept running, not to measure `T`).

*Pass rule.* A sub-test passes when, in all 3 repetitions:

1. 9a: `ce_present` of the half-drawn buffer returns `CE_ERR_BUFFER_NOT_READY`, `commit_count` is unchanged, `ce_buffer_state` is `WRITABLE`, no event is reported.
2. 9b and 9c: `commit_count` and `latched_count` are unchanged during the stop, no event is reported during the stop (in particular `latch_timeouts 0`).
3. `V >= M`: the vsync interrupt ran during the stop.
4. After restart (9a, 9b): status `CE_OK`, exactly one commit (+1: the application buffer; the plane is already on), `latched == commits`, exactly one `PRESENTED`, no `DROPPED`, the previous application buffer released once, and only after the `PRESENTED`.
5. The photographs below pass.

Any single violation is FAIL. A missing `SCENE 9` line is FAIL of that sub-test and of the sub-tests after it (as in `docs/m3-am62p-test-plan.md` §2.1). 9c's `CE_EVENT_LATCH_TIMEOUT` is **not** tested on the board: the design says there is no way to withhold the GO bit, so it stays a host-only check (`adv_latch_timeout_cycle`). The report says so.

*Photograph.* Per sub-test, repetition 1 only, three pictures of each state, judged with `scripts/camera-check.py` (median verdict, as `docs/m3-am62p-test-plan.md` §5):

- **During the stop:** EXPECT = B0 (blue, red rectangle on the left), and `--compare` against the B0 picture taken just before the test, every region `"same"`. In 9a the marker colour (magenta) must be absent from every region; a pixel set of that colour is FAIL.
- **After the restart:** EXPECT = yellow application plane, red rectangle on the left.

Pass rule for the photographs: `VERDICT: PASS` for all six states of the three sub-tests (during, after), and `"same"` for the three comparisons. A photograph that fails is FAIL of the sub-test, and the picture-and-log rule applies: both must hold.

## 6. T10: safety flip during an in-flight application commit

*Scenario.* Per trial, from B0 or its mirror: present 1 changes the application buffer (blue to green); present 2, at the offset of §4.2, changes the application colour as well and flips the safety rectangle (left to right). The trial alternates direction (green to blue, right to left) so that B0 comes back every second trial; the firmware makes the number of trials per offset even and ends with an explicit present of the end frame (green, rectangle on the right) before the hold. Present 2 may report `CE_ERR_BACKEND_BUSY`; the firmware waits one vsync and retries (§3.2) and counts the retries.

*Repetitions and offsets.* 14 offsets times 20 trials (§4.2, §4.1). 

*Per-trial invariants* (from the design's firmware check; each is a test of the firmware's own reading of the engine):

- **I1** every `ce_present` returns `CE_OK` or `CE_ERR_BACKEND_BUSY`; a retry loop ends within 3 retries.
- **I2** each submission ends with exactly one `PRESENTED` or `DROPPED`, within 1000 ms, never both, never neither.
- **I3** `PRESENTED + DROPPED` equals the number of successful presents.
- **I4** `latched_count` reaches `commit_count` within 1000 ms and never exceeds it.
- **I5** a buffer is released once, and only after the `PRESENTED` of the submission that replaced it, or at the `DROPPED` of a submission that never latched; no buffer is `SCANNING` and released.
- **I6** after the trial, the buffers of the last submission are `SCANNING`.
- **I7** `CE_EVENT_LATCH_TIMEOUT` is not reported.
- **I8** the last submission of the trial ends `PRESENTED`, never `DROPPED` (nothing replaces it).

*Outcome classes* (counted per offset, from the events and `backend.pending`; read from the design): `before` (first presented, second an ordinary commit), `dropped` (first dropped, second carries both), `mid` (the latch landed while the second present's writes were in flight: first presented, second pending). Which class a trial lands in is a **result**, not a requirement, except for coverage (pass rule 3).

*Expected console lines:*

```
HIST 10 flip-during-commit: offset <k> phase_pm <p> reps 20 before <b> dropped <d> mid <m> busy_retries <r> late <l> skipped <s>
SCENE 10 flip-during-commit: <common>, trials <n>, violations <v>, classes before <b> dropped <d> mid <m>, busy_retries <r>: ok
```

One `HIST` line per offset (14 lines). `<common>` here is the sum over all trials: `status 0 (expected 0)` is the last present's; `commits ... (+<d>, expected +<d>)` is the sum of the trials' writes against the expected sum; `events <e>` is the total, expected `2 * trials` terminal events plus releases. A violating trial adds a `STEP 10 flip-during-commit: trial <i> offset <k> invariant <Ik>` line.

*Pass rule.* **PASS** when all of these hold:

1. `violations 0`: no trial broke I1 to I8, and `commits` equals the expected sum.
2. `latch_timeouts 0`.
3. Coverage: at least one trial in class `dropped` and at least one in class `before`. (The class `mid` is not required.)
4. `skipped` is at most 2 of the 14 offsets (only offsets 12 and 13 may be skipped for the tick limit, §3.1) and `late` is at most 10% of trials.
5. The end-state photographs pass (below).

**FAIL** if rule 1, 2 or 5 is not met. **INCONCLUSIVE** if only rule 3 or 4 is not met (the sweep did not reach the case; the report names which and gives the `HIST` lines). A `mid` count above zero is **recorded** in the report with its offsets and is not a failure: the engine's contract is about the reported latch (design, scene 10).

*Photograph.* The held end frame (green, red rectangle on the right), three pictures, EXPECT from the extended `am62p-scenes.py` (§10): the application plane green and the rectangle on the right. The end state is one of the two defined states, never one plane new and one old; a picture showing green with the rectangle on the left, or blue with it on the right, is FAIL (that end state means the last frame did not reach one plane). Mixed-state burst scan (design): **only if** the camera probe has shown that the AM62P camera resolves single panel frames (frame rate and exposure measured, not known today). In that case a burst of at least `4 * T` around trial offset 12 and 13 is scanned for any frame whose safety rectangle and application colour disagree with both B0 and the end state; one such frame is recorded as **atomicity not shown** and makes the claim "no visible intermediate state" FAIL, but not the firmware part. Without the capability the report says: "the end state was judged; intermediate states were not."

## 7. T11: back-to-back commits within one frame

*Scenario.* A burst of `N` presents with no wait between them, each a different frame (application buffers 1, 2, 1, ..., with a flip in the middle; the colours of the intermediate frames differ from the last: blue, green, then yellow). The burst's start phase is given in §4.3. The firmware counts the vsyncs before the first and after the last present, and classes the burst **within** (`vsyncs delta 0`) or **split** (`delta >= 1`) by that count, not by the intended class.

*Expected console lines:*

```
HIST 11 burst: n <N> class within|split start_pm <p> reps <R> within <w> split <s> dropped_sum <d> presented_sum <p> busy <b> queue_full <q>
SCENE 11 burst: n <N> class within, <common>, bursts <R>, dropped <N-1 per burst>, presented 1 per burst, releases <r> (expected <r>), violations <v>: ok
SCENE 11 burst: n <N> class split, <common>, bursts <R>, presented <k> (max <delta+1>), violations <v>: ok
```

One `SCENE 11 burst` line per row of §4.3 with `Required` yes (five lines: N=2, 3 and 8 within-frame, N=3 and 8 split), and one `HIST` per row (seven). The large bursts print a `HIST` line and a `SCENE` line whose ending is `recorded`, not `ok`/`FAIL`: `SCENE 11 burst: n 64 ... queue_full <q>: recorded`.

*Pass rule per required row.* All bursts of the row satisfy:

1. **Within-frame bursts** (vsync delta 0): exactly `N - 1` `DROPPED`, exactly one `PRESENTED`, and it is for the last submission; `commit_count` rose by the writes of the `N` frames; after the next frame start `latched_count == commit_count` (the jump of `latched_count` equals the number of commits of the burst); every buffer of a dropped frame that is not carried into the last frame is released once, after its `DROPPED`; a buffer carried into the last frame is not released; no `CE_ERR_BACKEND_BUSY` (if it happens it is counted and polled and retried as in §3.2, and the row's `busy` count is reported, but a burst that needed a retry is moved out of the row's strict count and checked as split).
2. **Split bursts** (vsync delta `k >= 1`): `PRESENTED + DROPPED == N`; `1 <= PRESENTED <= k + 1`; the last submission ends `PRESENTED`; the releases follow I5 of §6; `latched_count == commit_count` after settle.
3. `latch_timeouts 0` and no `CE_ERR_EVENT_QUEUE_FULL` for `N <= 32` (queue limit 148, §4.3). A `CE_ERR_EVENT_QUEUE_FULL` at `N <= 32` is FAIL (the queue size or the polling is wrong).
4. A row with fewer than 20 bursts of its intended class after reclassification (a "within" row where more than a third of bursts split, or a "split" row whose burst never crossed) is **INCONCLUSIVE** for that row. The report says how many of each class ran.

**PASS** of T11 when every required row passes (rules 1 to 3) and the photographs pass. **FAIL** when any burst of any required row violates a rule. **INCONCLUSIVE** when every violation count is zero but a row fell under rule 4. The `recorded` rows (N=32 and N=64) never decide the verdict; a `CE_ERR_EVENT_QUEUE_FULL` at N=64 is reported with the count.

*Photograph.* Two held end states: the last burst of N=3 and of N=8 (within-frame, start phase 100 permille). Three pictures each, EXPECT from §10: the last frame (yellow application plane, rectangle on its final side). The intermediate colours (blue, green) are absent: a photograph that shows blue or green where yellow is expected is FAIL (the panel stayed on a dropped frame). If the burst-capable camera is available, a burst photographed during the stress is scanned for a dropped frame's colour appearing for one panel frame; one such frame is recorded as "the driver took a commit the engine reported as dropped" and makes the T11 verdict FAIL. Without the capability: "only the held end state was judged."

## 8. T12: cache maintenance for buffers the DSS reads

*Scenario.* **12a**: draw the application buffer, `env->flush(base, height * stride)`, present (the existing behaviour). **12b**: draw a new colour into the buffer **without** the flush and present. The design explains why 12b is a control, and that "no difference" is an answer. 12c is not in this plan (it is M3-05's).

*Repetitions.* 12a: 1 run with the 5000 ms hold and three pictures. 12b: 5 trials, each with a different pair of colours (blue to yellow, yellow to green, green to blue, blue to green, green to yellow; `inferred`: the cache state at the draw differs from trial to trial, because lines can be evicted at any time, so a single run would not show whether the outcome is stable), 2000 ms hold each (`inferred`: shorter than `TEST_HOLD_MS` because only one picture per trial is taken, but longer than the camera's settling), one picture each, plus three pictures of trial 1.

*Expected console lines:*

```
SCENE 12 cache-flushed: <common>, flush calls <k> (expected <k>), flush base <x> len <u> (expected len <u>), line aligned yes, region cacheable <0|1|unknown>: ok
SCENE 12 cache-unflushed: <common>, trials 5, flush calls 0 (expected 0), region cacheable <0|1|unknown>: recorded
```

`cacheable` is what the firmware reads from its own MPU configuration (§10); `unknown` is allowed until that code exists and is reported as such. `<common>` of 12b shows `status 0 (expected 0)`, `+1 (expected +1)` commits per trial, presented yes. The counts say nothing about caches (design); they are there to show the scene ran.

*Pass rule.*

- **12a PASS** when: `status 0`, one commit per present, `latched == commits`, presented within 1000 ms, the flush was called once per fill with `base` equal to the buffer's base and `len` equal to `height * stride`, both aligned to the cache line size the firmware reports (`inferred`: the line size is read from the core; not read here), and the three photographs `VERDICT: PASS` (flat colour, the exact colour of the new frame). **FAIL** otherwise.
- **12b is an observation, not a test of the firmware.** It is **PASS** when the trials ran, each trial's counts are consistent, and each picture was judged with the same EXPECT as 12a's new colour and classified `same` (full new colour) or `differs` (a mixture of old and new, a band or a pattern; `camera-check.py` fails a flat-colour test when old and new mix and does not describe the pattern). The classification of all five trials and the region's `cacheable` value go into the report side by side. **FAIL** only if a count rule of the common line is broken. **INCONCLUSIVE** if `differs` is seen and `cacheable` is `0` or `unknown`, or if the trials disagree with each other (some `same`, some `differs`) and the report cannot state a rule. These are findings to investigate, not results to hide.

*Photograph.* 12a paints the new application colour (yellow) over B0 (blue, red rectangle on the left), so EXPECT is a yellow application plane with the rectangle still on the left. 12b: the same EXPECT for the colour of that trial's frame.

## 9. Verdict, restore and evidence

*Overall.* **M3-03 board part PASS** when T9, T10, T11 and T12 are PASS, and the eight scenes of the earlier plan passed. It is **FAIL** if any test is FAIL, **INCONCLUSIVE** if none is FAIL and any is INCONCLUSIVE. The report gives one line per test with the evidence reference, and the lines of "what was not shown": the A53 as producer, burst-resolution camera frames if absent, `CE_EVENT_LATCH_TIMEOUT` on hardware, and the race rate below the bound of §4.1.

*What the report must hold for M3-03,* in addition to `docs/m3-am62p-test-plan.md` §8: the measured `T_us` and `tick_ns`; every `HIST` line; the `RESULT M3-03` line; all `STEP` lines; the photographs (used and discarded) with EXPECT, check output and the 12b classification; the trace buffer unedited with the read times; the count of dropped trace lines; and the open-issues text for M3-03 that follows from the verdicts. The report holds no board address, user, credential, serial path or serial number.

*Restore.* Unchanged from `docs/m3-am62p-test-plan.md` §7: every session ends with the remote processor stopped (stop up to 5000 ms), what was stopped or unbound restored in reverse order, the board rebooted into its Torizon boot chain, and Torizon confirmed up over ssh. A second image of the board test needs a reboot (`docs/am62p-r5f-remoteproc-source.md` §2). This plan adds nothing to it.

*Run time (inferred, not measured).* With `T` = 16.7 ms and a trial settle of four frames (67 ms): T9 about 21 s (three photographed holds of 5 s, six of 1 s), T10 about 19 s plus a 5 s end hold, T11 about 21 s of trials plus two 5 s holds, T12 about 15 s. About 100 s in all, so the 300 s watchdog of §3.2 is three times the estimate. The estimate is replaced by the report's value once there is one.

## 10. What the board firmware in tests/board/am62p_r5f must add later

Nothing here exists, and this plan does not change the firmware. For whoever writes scenes 9 to 12 (the host check first, then the firmware, as the design orders):

1. **A `scenes_m3_03.c`** (or the same file) with the four tests, in the host-compilable style of `scenes.h`: only `struct scenes_env` as the world. Needed additions to `scenes_env`: a microsecond clock with its tick (`now_ticks`, `tick_ns`), a busy-wait, a read of `backend.vsyncs` with the vsync wake (`wait_wake` exists), and a call that returns the instant of the last vsync.
2. **Phase placement** (§3.1): `wait_phase(permille)`, counting `late`, refusing offsets of less than two ticks.
3. **Measurement of `T`** at the start of T9, printed as `T_us` and `tick_ns`.
4. **Latch timeout wiring:** `ce_set_latch_timeout(200 ms)` and `ce_advance_time` from the timer, and the counter `latch_timeouts`.
5. **The console lines of §3.3 to §8**, the `STEP` limit (8 per part), the trace budget counter, and the final `RESULT M3-03` line. All lines use `%s %d %u %x` only.
6. **Colours:** a marker colour for the half-drawn buffer (magenta) and a third application colour (yellow), as `SCENES_*` constants next to the existing four.
7. **A record of flush calls** in the host fake (`host/fake_panel.c`): every call's base and length, compared with what each scene drew (design, scene 12); and the same check in the firmware for the line size alignment.
8. **The region's MPU attribute** printed as `cacheable <0|1|unknown>`: read from the Zephyr configuration or the core's registers (not read yet; design scene 12).
9. **Host checks first**, none of them duplicated here: the offset sweep, the three stop points, the burst of N at and across a frame start, and flush recording, on `board_scenes_behind_a_fake_panel` in `backends/twoplane/tests/twoplane.rs`. The planned host simulation of M3-03 (another item) covers the same ground in `backends/twoplane/tests/`; whichever lands first, these plan numbers are the parameters (14 offsets, 20 trials; the burst rows of §4.3; 3 repetitions of T9) so the host and the board run the same trials.
10. **Scripts:** `scripts/am62p-scenes.py` needs the scenes 9 to 12 (names `producer-stop-a/b/c`, `flip-during-commit`, `burst`, `cache-flushed`, `cache-unflushed`; EXPECT from B0 and the third colour), a `summary` that reads `SCENE`, `HIST`, `STEP` and `RESULT M3-03`, and accepts a name with a dash and a letter. `scripts/check-m3-report.sh` may need an M3-03 section check once the report has one. Until then the EXPECT files are written by hand from §5 to §8 and kept in the evidence directory.
11. **A second producer thread** with its mutex (design, "what concurrent can mean") and the A53 producer are not in this plan; they are listed in §11.

If the final `scenes.c` changes a name or a count of the eight scenes, the file wins and this plan follows.

## 11. What this plan does not cover

| Item | Status |
| --- | --- |
| A real A53 producer, the link to it, and the safety layer's behaviour when that producer dies | Not covered. M3-05 and the remaining part of M3-03. |
| A second producer thread in the R5F firmware, with its mutex | Not covered; §2 gives the reason. |
| Hardware atomicity of the GO bit against a frame start in the middle of one commit | Covered only as far as the end state and, if the camera can, a burst scan (§6). The firmware cannot see what the DSS scanned. |
| A race rarer than the bound of §4.1 (about 10% to 15% at 95% confidence for 20 to 30 trials) | Not covered. Zero violations is not "never". |
| `CE_EVENT_LATCH_TIMEOUT` on real hardware | Not covered (no way to withhold the GO bit); host-only. |
| Cache coherence between the R5F and the DSS as a property of the hardware | Not claimed; 12b is an observation (§8). |
| Throughput, latency to light, CPU and memory | Not covered. |
| Safety claims | None. The "safety plane" is a layer role of the test (SoW §7). |

## 12. Order of work

1. Host checks for T9 to T12 (the item that simulates M3-03, plus the flush recording), run in `cargo test --workspace`.
2. Firmware additions of §10 on top of the merged eight scenes; host build of `host/fake_panel.c` with the same code; the C compile check for the R5F.
3. Image build on a node with the board environment.
4. Board session after the eight scenes of `docs/m3-am62p-test-plan.md`: record, run, photograph, judge, report, restore, confirm Torizon.
5. Update the M3-03 text of `docs/open-issues.md` and `docs/sow-traceability.md` by the report's verdicts, and no more.
