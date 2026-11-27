# Board scenarios: SoW §10 hardware integration tests, design only

SoW §10 asks for hardware integration tests at **startup, steady state, application stop/restart, lease loss, modeset and display recovery**. `docs/open-issues.md` X-02 records that none exist as repeatable board scripts. This page catalogues the six scenarios for each platform path and says how each would be run. It is a design: **every row is not verified on hardware**, and nothing here is a result. No scenario below has been run by this page's author; "exists today" names files in the repository, not board outcomes.

Status of this page: written 2026-11-28 from the repository at the commit it was committed on. The tags used elsewhere apply: a statement about what a tool does is *read from source* (the file is cited); a pass rule or step is a *proposal*.

The page holds no addresses, host names, users or serial paths. `<board>` is the ssh destination from the farm lease.

## 0. Common material

### Board rules (from `docs/imx95-board.md`, read only)

Applied to every i.MX95 row below as rules **B1-B5**:

- **B1** Write only under `/tmp` on the board; no packages.
- **B2** No U-Boot environment changes (`fw_setenv`, `saveenv`).
- **B3** Never write `mmcblk0boot0` / `mmcblk0boot1`.
- **B4** Weston owns the display. Taking it means stopping it; restore it before leaving and confirm `systemctl is-active` says active. Never commit on the display while Weston holds it. If Weston cannot be restored, say so.
- **B5** Remove what was copied to `/tmp` when done.

Rows for the Zephyr path add **R1**: change nothing the remoteproc script does not restore (`firmware` name, `firmware_class` search path, core state), and **R2**: the M7 is started only from Linux remoteproc from `/tmp`; the gate for that is decided by `docs/imx95-zephyr-feasibility.md` (another item), and until it says go, the Zephyr rows stay design.

### Tools that exist today

| Tool | File | What it does (read from source) |
| --- | --- | --- |
| Session helper | `scripts/imx95-session.sh`, `scripts/imx95-session-README.md` | deploy to `/tmp`, stop and restore Weston (`display-owner stop/start`), run a command with saved header/stdout/stderr, clean up |
| DRM harness | `tools/drm-harness` (`src/main.rs`, `src/scenarios.rs`) | scenarios `combos`, `two-planes`, `zorder-alpha`, `move`, `rotate`, and `revoke` (only `--source kernel-lease`); sources `master`, `kernel-lease`, `dlm`; `--seconds`, `--frames`, `--buffers`; a lessor and external-producer mode |
| Camera checker | `scripts/camera-check.py`, `scripts/camera-check-README.md` | judges a picture against an `EXPECT.json`; exit 0 PASS, 1 FAIL, 2 input error; `--compare` asserts changed/same between two pictures |
| M7 remoteproc script | `scripts/imx95-m7-rproc.sh` (`.test.sh`) | loads an ELF from `/tmp` through remoteproc, prints state before/after, collects dmesg and an optional `--log`, stops the core, restores firmware name and `firmware_class` path; exit 0 ok, 1 failure, 3 refused (core not offline), 4 restore failed. Host-tested against a fake sysfs only |
| Secrets scan | `scripts/check-no-board-secrets.sh` | run before committing any log |
| Linux test plan | `docs/m1-imx95-test-plan.md` | M1 scenarios, camera procedure, evidence layout |
| M2 plan | `docs/m2-comparison-plan.md` | the Zephyr scene, metrics, blocker report |
| Host tests | `crates/composition-drm/tests/vkms.rs`, `crates/composition-fake` | lease revocation on vkms and fake-backend scenarios only |

### Generic result rule

Each scenario writes one result file `result.txt` into its evidence directory with the first line `SCENARIO <name> PASS|FAIL|BLOCKED|NOT-RUN`, followed by the evidence lines it relied on. A scenario passes only if **both** the log rule and (where the row says so) the camera rule hold; a picture alone never passes (`docs/m1-imx95-test-plan.md` §1: a flattened buffer looks the same). The result-file convention is a proposal; no script writes it today.

### What is common to all rows and missing

- No repeatable runner exists: each scenario is today a hand-typed sequence of the tools above (X-02).
- The trace format on the harness side and on the Zephyr side is not fixed or parsed (X-03); "log lines" below means the trace events of `crates/composition-core/src/trace.rs` (`validate`, `commit`, `latched`, `released`, dropped), as far as a board log shows them. Whether they appear on hardware is not verified.
- The camera is an MCP resource of the farm; its limits come from the lease (`docs/m1-imx95-test-plan.md` §2).

## 1. Path A: Linux DRM / lease on the i.MX95 (A55)

Board: Verdin iMX95, Mallow, LVDS panel, Reference Multimedia Image 7.8.0 (`docs/imx95-board.md`). Rules B1-B5 apply to every row. A DRM master run needs Weston stopped; a lease run needs a lessor (`drm-harness lessor`, or drm-lease-manager). Evidence directory: `<out>/<date>-<scenario>/`.

Common evidence for the path: harness stdout/stderr and its trace log (`--log`), `<name>.header` from `imx95-session.sh run` (date, kernel, image), `systemctl is-active weston` before and after, camera pictures where stated.

| # | Scenario | Steps | Pass / fail rule | Evidence | Exists today | Missing |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | **Startup** | 1. `imx95-session.sh deploy`; 2. `display-owner stop`; 3. `run` harness `two-planes --source master --seconds N`; 4. camera picture during the hold; 5. `display-owner start`; `cleanup`. Repeat with `--source kernel-lease`. | Pass: harness exits 0; log shows `validate`, a `commit` naming two distinct plane ids with distinct buffer ids, then `latched`; camera-check PASS on the two regions; Weston active at the end. Fail: any of those absent, a hang, or a kernel error needing reboot. | harness log, `result.txt`, two camera pictures plus `EXPECT.json` verdict, Weston before/after | harness `two-planes`; session script; camera-check; M1 test plan §4-5 | repeatable runner; `EXPECT.json` for the panel corners; fixed trace format (X-03); a time bound for "startup" is not defined |
| A2 | **Steady state** | As A1 with `move` (or `rotate --frames N --buffers 3`) for a long hold (duration proposed, e.g. 10 minutes, to be set once a first run shows the cost). | Pass: `latched` count equals `commit` count minus stated drops, each buffer is `released` only after the flip that replaced it, no unexpected `dropped`, harness exits 0, no growth in the harness's memory figure beyond a stated bound. Fail: missing release, late or dropped frames above the stated bound, crash. | trace log, harness summary (counts, latency, CPU, memory as the harness prints them), one camera pair via `--compare` for `move` | harness `move`, `rotate`; vkms test `buffers_are_released_only_after_the_next_page_flip` (host) | the bounds (drops, memory); a trace checker that applies the rule; long-run result; X-04 metrics are not measured |
| A3 | **Application stop / restart** | 1. run a harness scenario under the lease; 2. kill it with SIGTERM, then (separately) SIGKILL; 3. check the display state; 4. start the harness again; 5. repeat A1's checks. | Pass: after SIGTERM the process exits cleanly and the plane is released; after SIGKILL the kernel returns the lease/master, and the second run reaches `latched` on the same planes without a reboot. Fail: second run cannot acquire the lease or the display, or needs Weston restarted to recover. | exit statuses, second run's trace, `dmesg` tail (kernel lines only; no secrets), Weston state | harness runs repeatedly; the external-producer mode (`--producers external`) for killing a producer | a script that kills and restarts; the producer-crash case is "later" in the M1 plan (§ producer crash/restart); the engine's reaction to a dead producer is a design question |
| A4 | **Lease loss** | 1. harness `--source kernel-lease`, scenario `revoke`: rotate, then the lessor revokes mid-rotation. 2. Optional: the same through drm-lease-manager (`--source dlm`) if built. 3. Optional: restart and show a fresh lease works. | Pass (from the M1 plan's lease section): the engine reports `DeviceLost` once within one second of the revoke, no commit follows, resources are released, exit status is the documented one, no kernel error requiring reboot. Fail: no report, a commit after revoke, a hang. | harness log with the revoke timestamp and the `DeviceLost` line; exit status; `dmesg` tail | harness `revoke`; vkms test `leased_output_exposes_only_leased_planes_and_reports_revocation` (host, not hardware); `scripts/build-dlm-aarch64.sh` (build only) | no board result; whether Weston 7.8.0 offers leases is a separate item (`weston-imx-lease`); the one-second bound is from the plan and unmeasured |
| A5 | **Modeset** | 1. Stop Weston; 2. harness holds the panel's native mode; 3. a mode change (a second mode, if the panel exposes one; else a disable/enable of the CRTC) is requested through the engine; 4. camera before/after. | Pass: the engine reports the new output size, in-flight buffers are released, the next `commit` uses the new mode, camera `--compare` shows the expected change. Fail: stale buffers, a commit rejected without a reported reason, or a black panel after. If the panel exposes only one mode, the row is **BLOCKED** and says so, not passed. | trace log, mode list from the probe, two camera pictures | `crates/composition-drm` (mode handling, read for this page only as existing; not checked here); probe example lists modes | no harness scenario changes a mode; whether the LVDS panel offers more than one mode is not recorded; the i.MX95 modeset path through the pixel-link/LDB is untested |
| A6 | **Display recovery** | 1. harness running; 2. induce a loss: unplug is not possible on an LVDS panel, so use a CRTC disable by the lessor, or `display-owner start` (Weston takes the display back) while the harness runs; 3. observe; 4. start the harness again. | Pass: the engine reports loss (not a silent stall), releases buffers, and a new run after Weston has been stopped again reaches `latched`; the display shows the new run. Fail: hang, stuck panel, Weston not restorable. | trace log, Weston state, camera picture after recovery | `display-owner start/stop`; harness `revoke` covers the lease half | no scenario for non-lease loss; hotplug/reprobe on LVDS has no equivalent; what "recovery" must include (re-probe, re-plan) is not specified beyond SoW §10 |

## 2. Path B: Zephyr on the i.MX95 M7

Premise: the M7 is started from Linux remoteproc with an image in `/tmp`, through `scripts/imx95-m7-rproc.sh`. Whether the M7 can own the DPU at all, and whether the board's System Manager allows it, is **open** and belongs to `docs/imx95-zephyr-feasibility.md` (`docs/open-issues.md` M2 items). Where the M7 cannot own the display, the scanout rows are **BLOCKED** and the result is the blocker report of `docs/m2-comparison-plan.md` §7; the load and lifecycle rows can still be run with a hello image. Rules B1-B5, R1, R2 apply. The M7 apps (`tests/board/imx95_m7*`, `tests/zephyr/qemu_engine`) belong to other items; their present state is not asserted here. The Linux side does not use the display while the M7 does: Weston stopped and restored (B4) as in Path A.

Evidence directory: `<out>/<date>-zephyr-<scenario>/`, with the remoteproc script's before/after state lines, the dmesg lines it collects, the `--log` capture (rpmsg tty or UART, whichever the BSP study settles), and the Zephyr trace lines, whose timestamps are cycle counts converted on the host. Timestamps of M7 and A55 are never subtracted (`docs/m2-comparison-plan.md` §4.2).

| # | Scenario | Steps | Pass / fail rule | Evidence | Exists today | Missing |
| --- | --- | --- | --- | --- | --- | --- |
| Z1 | **Startup** | 1. Confirm the remoteproc is `offline` (the script refuses otherwise); 2. `imx95-m7-rproc.sh --run-secs N --log <file> /tmp/<elf> <remoteproc>`; 3. for a display image: Weston stopped first, camera picture while running; 4. Weston restored. | Pass: script exit 0 (`running` reached, stopped, firmware name and `firmware_class` path restored, state back to `offline`); the log shows the image's start banner; for the display image, a `latched` line and camera-check PASS on a one-plane pattern (Stage A of the M2 plan). Fail: script exit 1/3/4, no banner, or a picture without a latch line. | script's before/after lines, dmesg lines, log capture, camera picture and verdict, Weston state | `scripts/imx95-m7-rproc.sh` (host-tested on a fake sysfs, never run on a board); `docs/m2-comparison-plan.md` Stage A | the M7 image, the answer on DPU ownership, the log channel; the script is unproven on the real remoteproc |
| Z2 | **Steady state** | As Z1 with a fixed run (`--run-secs N`) while the image submits buffers in rotation and logs cycle-count events. | Pass: `latched` count matches `commit` count less stated drops; buffer reuse only after `released`; no watchdog timeout (the M2 plan's one-second no-event error); remoteproc state remains `running` until the script stops it. Fail: a missing interrupt counted as a latch, buffer written between submit and release, a stall. | cycle-count trace, the M2 plan metrics M-1..M-7 as available, remoteproc state at the end | M2 plan §3-5 (metrics and criteria on paper) | the scanout application and its latch hooks, the trace format (X-03), the shared trace checker |
| Z3 | **Application stop / restart** | 1. run Z1; 2. the script stops the core; 3. run it again with the same and with a different image; 4. check that Linux still sees the DPU/display unchanged (Weston restore). | Pass: second load reaches `running` and the banner again with no reboot; after the final stop `state` is `offline`, firmware name and `firmware_class` path equal their pre-run values (the script's restore check, exit 0), and Weston is active. Fail: restore failure (exit 4), a stuck `running`/`crashed` state, display left in a changed state. | two script outputs, state lines, Weston state | script covers one load/stop/restore cycle | a loop runner for repeated cycles; what the display hardware is left in after the M7 stops is unknown (the M7 may leave the DPU programmed, and Linux would not know) |
| Z4 | **Lease loss** | There is no DRM lease on the M7. The nearest analogue is loss of the display resource: the M7's access is withdrawn (Linux or the System Manager reclaims it), or Linux and the M7 contend. **Defined as not applicable** unless the feasibility doc shows a shared-ownership route; if one exists, the scenario becomes "the other owner takes the display back while the M7 runs". | If applicable: the Zephyr image detects the loss (an error or missing interrupt after the watchdog) and reports it rather than continuing to claim frames. Otherwise the row reads **N/A: no lease concept**, not "pass". | log lines of the detection, remoteproc state | nothing | the ownership answer; a detection design in the Zephyr driver; the SoW's lease wording does not map to the M7 and that mapping is not decided here |
| Z5 | **Modeset** | The image requests a different mode or re-enables the stream (display-controller timing change), then scans out again. | Pass: first latch after the change reports the new timing, no stale buffer shown, camera `--compare` shows the change. Fail: no latch, black panel, or the driver exposes no mode change (then **BLOCKED**). | trace, two camera pictures | nothing; upstream driver coverage is a source-study question (`dpu-zdrv-src`) | whether the upstream driver and the LVDS route allow a mode change; the panel's modes; the application |
| Z6 | **Display recovery** | Stop the core mid-scan (script stop), then (a) restore Weston and (b) reload the M7 image; check both paths show a picture. | Pass: after stop, Weston restored shows its desktop (camera check against a Weston reference picture) and a new M7 load scans out again. Fail: panel dark or frozen after either, or Weston unrecoverable. | camera pictures, Weston state, script outputs | script stop and restore; `display-owner start` | the answer on who programs the DPU after the M7 stops; no reference picture of Weston; the Linux/M7 hand-over procedure |

## 3. Path C: AM62P R5F

Named only. The AM62P R5F board run of the two-plane backend is **issue #12**, already queued, with `docs/open-issues.md` M3-01 and M3-02 and `docs/twoplane-backend.md` describing the host-tested state. This page plans nothing for it and does not change that issue. Every row is **not verified on hardware** and **not planned here**.

| # | Scenario | Plan here | Where it belongs |
| --- | --- | --- | --- |
| C1 | Startup | none | issue #12 |
| C2 | Steady state | none | issue #12 |
| C3 | Application stop / restart | none | issue #12 |
| C4 | Lease loss | none | issue #12 |
| C5 | Modeset | none | issue #12 |
| C6 | Display recovery | none | issue #12 |

Board rule for any later AM62P work (project rule, not from `docs/imx95-board.md`): a board booted into anything other than its Torizon boot chain must be put back and confirmed over ssh before the task ends. That is stated for the issue-#12 owner, not planned here.

## 4. What this catalogue leaves undone

- No runner script, result-file writer or trace checker exists; building them is outside this page.
- Every pass bound given as a number (one second to report revocation, run length, drop and memory bounds) is a proposal or comes from the M1 plan, and is unmeasured.
- Rows Z4-Z6 depend on the DPU ownership answer for the M7, which this page does not decide.
- Whether Weston 7.8.0 supports leases, and whether the panel exposes several modes, are not recorded in the repository as far as this page read.
