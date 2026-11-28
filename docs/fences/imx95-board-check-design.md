# Test design: in-fence board check on the Verdin iMX95

Status: **design only, written for the M1 plan (#453) to run later.** No board
was touched, leased or measured for this document, and no harness code was
changed. Tags used below: **read from source** (the repository file named, or a
kernel reading already in `docs/`) and **inferred** (a conclusion the source
does not state). Nothing here is a measurement; the board result this design
asks for does not exist yet. The API names come from `docs/sync-fences.md` §3
and may differ from what the core-token and drm-fence items land: the landed
code wins, update this file then.

The check covers the Linux DRM path only. It says nothing about M2 or M3. Host
and vkms runs of the same scenarios are a different evidence class and are
never cited as i.MX95 evidence (`docs/sync-fences.md` §6).

## 1. What is being checked

`docs/sync-fences.md` §3.2 and the last advice bullet of §6 name three things
that a run implementing `IN_FENCE_FD` must show:

1. **Presence:** the property is in the probe output of the board's kernel.
2. **Hold:** a fence left unsignalled keeps the previous frame on screen.
3. **Error:** the kernel's behaviour for a fence that signals with an error.

Plus two engine-side questions from §3.2 and §7: how long after the signal the
frame is `Presented`, and how `latch_timeout` behaves while a frame waits on a
fence.

## 2. Preconditions

### 2.1 Property presence

- Every plane in the recorded probe lists `IN_FENCE_FD` as a mutable atomic
  range `-1..=2147483647` with default `-1`: planes 31, 42, 51, 59, 67, 75 and
  83. Both CRTCs (39 and 50) list `OUT_FENCE_PTR` as a mutable atomic range.
  Tag: **read from source** (`docs/imx95-probe-2026-11-28.txt`, a probe record
  kept in the repository; kernel `6.6.157-7.8.0`, driver `imx95-dpu`).
- The run must **re-read** presence in its own probe output (`probe --raw`)
  before any scenario and print one line per plane. A plane without
  `IN_FENCE_FD` makes the scenario BLOCKED for that plane, not failed. The
  recorded probe does not replace this: the check is about the kernel the run
  actually boots.
- Where the property comes from: **inferred** that the DRM atomic helpers
  create it for planes of an atomic driver. Whether the driver adds anything of
  its own is **unknown**: `docs/imx95-dpu-source-6.6.md` has no fence row.

### 2.2 Does the driver wait for the fence

- Whether the i.MX95 DPU commit path waits for the plane's in-fence before
  programming the flip is **unknown**. **Inferred:** the DRM atomic helper
  commit path waits for the fences of the new plane states if the driver uses
  it. `docs/imx95-dpu-source-6.6.md` records that the flip event is queued in
  `atomic_flush` and sent from the frame-complete interrupt (read from source),
  but not which commit-tail helper the driver uses. The scenarios below test
  the conclusion; they do not assume it (see §8, item 1).
- Implicit sync on an imported dma-buf is a separate question and is not
  tested here (`docs/sync-fences.md` §5). Every buffer in this check is a dumb
  buffer that the harness writes itself.

## 3. Fence sources on the board

The check needs a `sync_file` that the harness controls: unsignalled on
demand, signalled on demand, and, for one scenario, signalled with an error.
Three candidates; none is confirmed for this board.

| Source | What it gives | How to tell whether it is available | Status |
|---|---|---|---|
| `sw_sync` timeline (`CONFIG_SW_SYNC`, control node under the debugfs `sync` directory) | A software timeline: create a fence at point N, signal by advancing the timeline. Full control of the signal time. Signalling with an error is **inferred** not possible through the plain timeline interface | From the run's own ssh session, read-only: look for the node and for `CONFIG_SW_SYNC` in `/proc/config.gz` if it exists. Debugfs counts as available only if it is **already mounted** (§6) | **Unknown.** No document in the repository records `CONFIG_SW_SYNC` or debugfs for the Verdin iMX95. **Inferred** that a production-style BSP config may leave it off. `docs/imx8mp-x2-board.md` (a different board) records that its debugfs DRM directory was not mounted (read from source) |
| Writeback out-fence (`WRITEBACK_OUT_FENCE_PTR` on a writeback connector) | A hardware fence that signals when a writeback job ends. The signal time cannot be chosen | A writeback connector in the run's probe output. `examples/writeback.rs` is the only repository code that uses a fence | **Not available on the record:** the recorded probe lists the LVDS output only, no writeback connector (read from source, `docs/imx95-probe-2026-11-28.txt`). **Inferred** unlikely on this kernel; re-check in the run's own probe |
| GPU fence (job completion of the GPU driver, exported as a `sync_file`) | A genuine producer fence | GPU driver and userspace on the Reference Multimedia Image | **Unknown.** Nothing in the repository records them for this board. Adding a GPU client is outside a minimal run (no packages) and belongs to the WP5 producers |

Decision rule for the M1 plan, in order:

1. If `sw_sync` is available without any mount or configuration change, use it
   for all scenarios. It is the only source that can hold a fence open and
   release it at a chosen time.
2. Else if the run's probe lists a writeback connector, use its out-fence for
   scenario F2 only, with the limitation that the hold time cannot be chosen.
   F1, F3 and F4 are BLOCKED.
3. Else every scenario is BLOCKED with the reason `no fence source`, and the
   report says so. A GPU fence is not built for this check.

A software timeline is not a producer. It shows the kernel's behaviour on the
plane's `IN_FENCE_FD`, which is what §1 asks. It does not show that a GPU,
decoder or camera is complete at scanout, and the report must say so in the
same sentence as any pass.

## 4. Scenarios

Each scenario runs on the LVDS output with the `imx95` profile and the
`two-planes` geometry (a full-screen buffer and a half-size buffer on two
planes), with flat saturated colours. Frame A (for example red over green) is
committed without a fence and must be `Presented` first. The fenced frame B
uses different colours (for example blue over yellow) so the camera can tell
them apart.

### F1: an unsignalled fence holds the previous frame

1. Commit frame A, wait for `Presented`.
2. Create an unsignalled fence on the fence source and hand it with frame B's
   buffers through the readiness API (the `finish_write` variant of
   `docs/sync-fences.md` §3.1). The engine accepts the frame: readiness is
   pending, not refused.
3. Hold unsignalled for `T_hold` (default 5 s; long enough for the camera
   procedure of §5).
4. Expected: no `latched` and no `Presented` for frame B during `T_hold`; A's
   buffers stay `Scanning` and are not released; the camera shows frame A for
   the whole interval.
5. The fence is signalled in the last step so no commit stays pending (§6);
   F2 can use this state before that step.

Verdict lines (format of `tools/drm-harness/README.md`, Output):

- `VERDICT fence-hold PASS no latch for 5000 ms, frame A on screen`
- `VERDICT fence-hold FAIL frame B latched while the fence was unsignalled`
- `VERDICT fence-hold BLOCKED no fence source`, or
  `VERDICT fence-hold BLOCKED IN_FENCE_FD missing on plane <id>`

### F2: signal releases the flip; latency from signal to Presented

1. From F1 step 3 (or the same setup again), record `t_signal` on the
   monotonic clock immediately before the timeline is advanced, then signal.
2. Expected: exactly one `latched` for B's submission, then `Presented`, then
   later `Released` for A's buffers, in that order.
3. Report `signal_to_presented_ms` = time of `Presented` minus `t_signal`.
   Repeat 20 times; report minimum, median and maximum. **Inferred** size:
   one to two refresh periods (about 17 to 34 ms at the recorded
   1280x800 @ 59.993 Hz), because the flip event comes from the frame-complete
   interrupt (`docs/imx95-dpu-source-6.6.md`, read from source). This is a
   guess to compare against, not a claim.
4. The ordering checks of `docs/m1-imx95-test-plan.md` §7 apply:
   `latched + dropped = submitted`, no write to a buffer between its submit and
   its release, zero ordering violations.

Verdict lines:

- `VERDICT fence-signal PASS presented after signal n=20 median_ms=<x> max_ms=<y> violations=0`
- `VERDICT fence-signal FAIL <reason>`; BLOCKED as in F1.

The latency is reported, not a pass criterion. A threshold is set by the M1
plan after a first run, not here.

### F3: errored fence

- **F3a (needs a source that can signal an error):** hand frame B with the
  errored fence. Per `docs/sync-fences.md` §3.2 (to be confirmed, not assumed)
  the frame is refused or not shown, frame A stays, and the engine reports a
  drop or refusal through the existing event or error (no new variants) and
  does not release A's buffers early. Record what the kernel does:
  **inferred** that the atomic helpers log the error and may still run the
  commit, in which case frame B is shown. That outcome is a FAIL against the
  design ("not displaying is preferable") and goes to the open issues, not
  into a pass wording. BLOCKED when no source can signal an error; the
  scenario must not be faked by closing the descriptor.
- **F3b (needs no errored fence):** hand a negative descriptor, a closed one,
  and one that is not a `sync_file` (an ordinary file). Expected: refusal
  before any commit with the defined error of `docs/sync-fences.md` §3.4,
  frame A stays, frame B's buffers stay usable. This tests the backend's
  validation against the real kernel's ioctl behaviour.

Verdict lines:

- `VERDICT fence-error PASS <what the engine and the screen did>`
- `VERDICT fence-error FAIL frame B shown after an errored fence`
- `VERDICT fence-error BLOCKED no fence source that can signal an error`
- `VERDICT fence-badfd PASS|FAIL|BLOCKED <reason>`

### F4: waiting on a fence versus `latch_timeout`

`docs/sync-fences.md` §3.2 and §7: with `NONBLOCK`, an unsignalled
`IN_FENCE_FD` defers the commit in the kernel, so the flip event is late and
`LatchTimeout` can fire although nothing is wrong.

1. Run F1 and F2 with `latch_timeout` `L` below `T_hold`, then again with `L`
   above `T_hold`.
2. Record: whether `latch_timeout` was reported while the fence was
   unsignalled; whether the submission stayed pending (it must; a
   `latch_timeout` is not a drop, `docs/trace-format.md`); whether the frame
   latched after the signal; whether a second `latch_timeout` appears after
   the signal.
3. Expected: with `L` below `T_hold` a `latch_timeout` is reported
   (**inferred** from the design) and documents the caveat; with `L` above
   `T_hold` none. Whether the timeout should be suspended while waiting is
   decided by the implementation, not by this check.

Verdict line: `VERDICT fence-timeout PASS timeout_reported=<yes|no>
latched_after_signal=yes second_timeout=no`, or FAIL naming the failing item
(for example the submission was dropped by the timeout); BLOCKED as in F1.
Reporting a timeout while the fence is held is a recorded result, not a fail.

### Summary

The run ends with the existing `SUMMARY passed=.. failed=.. blocked=..`. The
report states property presence (§2.1) and the fence source (§3) before the
verdicts.

## 5. Camera evidence

The rules of `docs/m1-imx95-test-plan.md` §2 apply unchanged. The camera is
the farm camera resource pointed at the panel, as in the other i.MX95
procedures (tailvision camera; the M1 plan calls the target `imx95-camera`);
its exact name and limits come from the lease. In short: the picture is
rotated 180 degrees, whole `capture_debug` frames, saturated flat colours, a
throw-away frame first, at least three pictures per state with the median
verdict, the camera does not move inside a scenario, and every picture used or
discarded is saved with the scenario, the state and the time in its name.
`scripts/camera-check.py` judges the frames.

States for this check:

- F1: three pictures at the start of `T_hold` and three near its end. All six
  must show frame A. One picture of frame B inside the hold is a FAIL
  whatever the log says.
- F2: three pictures after the signal showing frame B.
- F3a: three pictures after the error, judging what is on screen.
- The pictures decide what is on the panel; the log decides what the engine
  did. If they disagree, the scenario is FAIL and both are kept.

The camera cannot time a flip, so the latency of F2 comes from the log only.

## 6. Leaving the board as found

These restate `docs/imx95-board.md` (Board rules) and
`scripts/imx95-session-README.md` (Safety), plus the fence-specific parts.

- **Weston.** Stop it with `scripts/imx95-session.sh display-owner stop` (it
  records whether the unit was active). At the end run `display-owner start`
  and check it printed that the unit is active again, also after a failed
  scenario; `display-owner stop --then <script>` arms an `EXIT` trap that does
  this. If it does not come back, say so in the report.
- **Files.** Only under `/tmp` on the board: the deployed `probe` and
  `drm-harness` (listed in `/tmp/imx95-session.files`, removed by `cleanup`),
  and any helper the run adds, which is listed there the same way. Evidence
  files are written on the host by `run`, not on the board.
- **No packages.** No opkg, dnf, apt or pip, nothing copied into `/usr` or
  `/etc`. A fence helper is a static binary built on the host.
- **No U-Boot environment change** and no write to `mmcblk0boot0` or
  `mmcblk0boot1`. `imx95-session.sh run` refuses command lines naming
  `fw_setenv`, `saveenv`, `mmcblk0boot` and the package managers.
- **No mounts, modules or kernel settings.** Mounting debugfs, loading a
  module, or writing any sysfs or debugfs control other than the fence
  timeline's own node is not allowed. If `sw_sync` needs a mount that is not
  there, the source is "not available" (§3).
- **No pending commit left behind.** Every fence created is signalled, and its
  descriptor closed, before the harness exits; the harness exits through its
  normal path. After the last scenario `display-owner start` must succeed (a
  leaked deferred commit could show as a stuck CRTC when Weston takes over;
  **inferred**).
- **Report the end state:** Weston unit active, the file list empty after
  `cleanup`, the CRTC mode as at the start.
- **No addresses or host names** in files or commit messages
  (`scripts/check-no-board-secrets.sh`); the ssh destination comes from the
  lease's environment variable only.

## 7. Telling the causes apart in the report

A FAIL in F1 or F3a can have these causes; the report names the one the
evidence supports and says **unknown** otherwise:

- the driver's commit path does not wait for the in-fence (§2.2);
- the backend did not set the property (check the `TEST_ONLY` result and the
  trace of the commit);
- the property was set on a plane other than the one showing frame B;
- the fence itself was already signalled (a timeline point already passed).

Do not cite host or vkms results as a cause or a pass for this board. Board
claims need the log and the pictures of this run; M1 conditions are only
claimed where the M1 plan's own criteria are met.

## 8. Future work this run needs

Listed against the APIs of `docs/sync-fences.md` §3. None of it is done by this
document.

1. **Source reading (docs):** which commit-tail path and fence waits the
   i.MX95 DPU driver uses at `6.6.157-7.8.0` (plan item `src-imx95`).
2. **Core token (`composition-core`, §3.1):** the per-buffer readiness token at
   `finish_write` time, and a defined refusal or drop for a token the backend
   cannot honour. F1 to F4 need to hand a pending token and see its effect on
   `latch_timeout`.
3. **DRM fence module (`composition-drm`, §3.2):** `IN_FENCE_FD` per plane and
   `OUT_FENCE_PTR` per CRTC only where the probe lists them, fence errors
   through the existing backend errors, the duplicate descriptor for
   `TEST_ONLY`. The drm-glue item connects the token to it.
4. **Harness (`tools/drm-harness`):** scenarios `fence-hold`, `fence-signal`,
   `fence-error`, `fence-badfd`, `fence-timeout` with the verdict lines of §4;
   a fence-source option (for example `--fence-source sw_sync|none`) that opens
   the timeline, creates and signals fences and records `t_signal`; one
   presence line per plane; a way to set the latch timeout; README table rows.
   Every fence is signalled before exit.
5. **Trace and checks:** a `signal_to_presented_ms` value in the existing
   summary lines (not a new trace event variant) and a checker for the order
   signal, latched, presented, released, modelled on `scripts/check-trace-log.sh`.
6. **Session script:** no change if the helper stays inside `drm-harness`;
   a separate helper binary goes into the deploy list so `cleanup` removes it.
7. **M1 plan (#453):** a scenario section and a verdict-rule entry, and the
   `docs/sync-fences.md` §6 wording only after a run backs it. That is the M1
   plan's own edit.
8. **Errored-fence source:** find from kernel source whether a source on this
   BSP can signal with an error, or whether F3a stays BLOCKED.

## 9. What the check does not show

- Anything about M2, M3 or the Zephyr path.
- That a GPU, decoder or camera producer is complete at scanout.
- Implicit synchronization on imported dma-bufs.
- A pass of M1 condition 5 by itself: the criteria of
  `docs/m1-imx95-test-plan.md` §7 remain, and this check adds the fence
  sentence of `docs/sync-fences.md` §6 only if its scenarios ran on the board
  and passed.
