# M3 test plan: AM62P portability regression (R5F, DSS)

This is the test plan for milestone M3 of `SoW.md` (§9, WP4): the two-plane backend (`backends/twoplane`) on the MCU Cortex-R5F of a Toradex Verdin AM62P, driving the board's display controller (DSS). It is written in the style of `docs/m1-imx95-test-plan.md`. The results go into `docs/m3-am62p-report.md` (not written yet), not here. It covers the open issues M3-01, M3-02 and M3-05 and the camera rule of X-05 in `docs/open-issues.md`.

**Everything in this plan is planned. Nothing in it has been measured.** `docs/sow-traceability.md` says that no document in the repository records an AM62P board observation, and this plan does not change that. The numbers below are what the firmware expects, not what the board did.

The plan is board-independent: it holds no addresses, host names, users, credentials or serial paths. Where a command needs the board, it says `<board>`; the session setup supplies the real value.

## 0. Status marks

- **main**: exists on `main` today. Linked.
- **branch**: exists on the owner's branch `work/am62p-board-test` (tip `361e31f`), being brought up to date with `main` by another item of the plan. Written as a path in a code span, not a link, until it lands.
- **planned**: assigned to an item that has not landed. The interface below is the intent and may change; the tool's own `--help` or README wins over this text.
- **showcase**: a file of the demo checkout `$MCUI_SHOWCASE_DIR`, which is not part of this repository and is never copied into it.

| Piece | Status |
| --- | --- |
| Backend, fake-driver tests, CI jobs (`docs/twoplane-backend.md`, `docs/ci.md`) | main |
| `scripts/camera-check.py` and its README (`scripts/camera-check-README.md`) | main |
| `scripts/scenario-expect.py` (`scripts/scenario-expect-README.md`) | main; its scenarios are the Linux harness's, so it fits the AM62P scenes only if it takes an EXPECT description of them, otherwise it does not fit (see §5) |
| `tests/board/am62p_r5f/` (Zephyr app: `src/scenes.c`, `src/main.c`, `src/hw_am62p.c`, `CMakeLists.txt`, `prj.conf`, `host/fake_panel.c`) | branch |
| `docs/board-test-am62p-r5f.md` (build and run description) | branch |
| `board_scenes_behind_a_fake_panel` in `backends/twoplane/tests/twoplane.rs` | branch |
| `scripts/am62p-scenes.py` (EXPECT JSON per scene, parser of the `SCENE` and `RESULT` lines) | planned (`am62p-scene-tools`) |
| `scripts/am62p-r5f-run.sh` (remoteproc run on the board) | planned (`am62p-run-script`); modelled on `scripts/imx95-m7-rproc.sh` (main) |
| `docs/m3-am62p-report.md`, `docs/observed/am62p.md` | planned |
| remoteproc route description `$MCUI_SHOWCASE_DIR/docs/remoteproc-bringup.md`, the DSS driver `$MCUI_SHOWCASE_DIR/include/mcui/am62p_dss.h` | showcase |

## 1. What the board run must show

M3 asks for the two-plane backend to work on a second SoC, without a change to the engine core. The board run must show, in the order of the issues:

1. **M3-01.** Two-plane composition on the AM62P's R5F and DSS: a safety plane (ARGB8888, premultiplied) over an application plane (RGB565); the latched count tracks the commit count (the GO bit of the DSS shadow registers is cleared at a vertical blank); buffers are released only after the flip that replaced them was latched; a frame the backend cannot show is refused with no commit.
2. **M3-02.** The test is in this repository (or pinned), has a build, a run script and a judge of what the panel shows: the app under `tests/board/am62p_r5f/`, the run script, and `scripts/camera-check.py`.
3. **M3-05.** The C backend links into a real firmware build for the R5F (the Zephyr image), not only into the freestanding image of the CI job `arm-backend`.
4. **X-05.** The camera check is reused on a second board, with the picture-and-log rule of §3.

The run is of one Zephyr image that goes through eight scenes (§2). The firmware judges the counts, the status and the timing by itself and prints one `SCENE` line per scene and a final `RESULT` line. Only the photographs judge what the panel shows.

## 2. The eight scenes and their pass criteria

Source: the table of scenes in `docs/board-test-am62p-r5f.md` (branch) and `tests/board/am62p_r5f/src/scenes.c` (branch). The colours are flat. The safety plane is red (or fully transparent). The application plane is blue or green. The rectangle is a third of the panel's width and half its height, centred vertically, at 1/12 of the width from the left or right edge. Each scene is held for `TEST_HOLD_MS` (5000 ms by default) after its line is printed.

If the final version of the branch differs from this table (a scene added, a count changed), the branch wins and this plan is updated in the same change as the report. The `expected` values the firmware prints in its own line win over the columns below.

### 2.1 Firmware criteria (from the trace buffer)

A scene passes the firmware part when its line ends in `ok`. The line is:

```
SCENE <n> <name>: status <s> (expected <s>), commits <c> latched <l> (+<d>, expected +<d>), presented yes|NO|-, released <r> (expected <r>): ok|FAIL
```

| # | Name | Status | Delta of commits (= of latched) | Presented | Releases | Camera: the panel shows |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `safety-only` | `CE_OK` | +1 (flip) | yes | 0 | red everywhere |
| 2 | `app-under-safety` | `CE_OK` | +3 (buffer, enable, flip) | yes | 1 | blue, red rectangle on the left, on top |
| 3 | `app-replaced` | `CE_OK` | +1 | yes | 1 | green, the red rectangle still on the left |
| 4 | `safety-flipped` | `CE_OK` | +1 (flip) | yes | 1 | green, red rectangle on the right |
| 5 | `app-off` | `CE_OK` | +1 (plane off) | yes | 1 | the controller's background (black expected), red rectangle on the right |
| 6 | `refused-no-safety` | `CE_ERR_BACKEND_REJECTED` (-16) | 0 | - | 0 | unchanged from scene 5 |
| 7 | `refused-blank` | `CE_ERR_BACKEND_REJECTED` | 0 | - | 0 | unchanged from scene 5 |
| 8 | `refused-foreign-safety` | `CE_ERR_BACKEND_REJECTED` | 0 | - | 0 | unchanged from scene 5 |

The release count of scene 1 is not in the branch's table; 0 is the expectation from the sequence (nothing precedes it) and is confirmed against the firmware's printed `expected`. The commit count after the eight scenes adds up to 1, 4, 5, 6, 7, 7, 7, 7 (the host run's figures); on the board the same cumulative numbers are expected.

**Pass criteria per scene** (all of them, for every scene):

1. **Status** equals the expected status.
2. **GO bit: latched against commits.** After the frame was presented, `latched` equals `commits`, and the delta of both over the scene equals the expected delta. The commit count does not change while the scene is held (no commit after the frame, no spurious flip). A `latched` below `commits` after the hold means a commit that the DSS never took, which is a failure, not a delay.
3. **Presented** within 1000 ms: `ce_poll_event` reports the presented frame within 1000 ms of the submit. `NO` is a failure. Scenes 6 to 8 print `-`; a `presented` event there is a failure.
4. **Releases** equal the expected count. A buffer released before the flip that replaced it was latched is a failure (the count would run ahead of `latched`).
5. **Refusals** (scenes 6 to 8): status `CE_ERR_BACKEND_REJECTED`, **0 commits** and 0 latches in the scene, 0 releases, and the panel is unchanged. A refusal that committed anything is a failure even if the status is right.
6. **Photograph** judged by `scripts/camera-check.py` against the EXPECT JSON of the scene (§5): verdict `VERDICT: PASS`. Scenes 6 to 8 are judged with the same EXPECT as scene 5, and with `--compare` against the picture of scene 5 (`"same"`).

**The run passes** when `RESULT PASS (...)` is printed, all eight `SCENE` lines end in `ok`, and all photographs pass. `RESULT FAIL (setup)` (for example `SETUP: panel ... needs ...`, a panel larger than the default `TEST_APP_MAX_WIDTH` x `TEST_APP_MAX_HEIGHT` of 1280x800) is a failed run, not a skipped one; the build is changed with the two documented defines and the run repeated.

**Fail.** Any criterion above not met. A scene whose line is missing from the trace buffer (the run stopped or the buffer wrapped) is a failure of that scene and the later ones, not a pass.

### 2.2 What the firmware does not judge

The firmware cannot judge where the colour is on the panel, whether the safety plane is on top, or what the controller shows with the application plane off. Those are the photograph's. A scene can pass its line and fail its picture: that is a failure of the scene, and it means the counts count something that is not on the panel (a driver or address error, a cache problem, a wrong plane).

## 3. The picture-and-log rule

SoW §9 and WP2 do not accept a flattened framebuffer as hardware composition. A picture alone cannot exclude it: two layers drawn into one buffer and scanned out on one plane look the same as two planes (`docs/m1-imx95-test-plan.md` §1, X-05 in `docs/open-issues.md`). The same rule holds here, and the log is the other half of the evidence:

- **Log.** In scene 2, three commits (buffer, enable, flip) and two distinct buffers on two planes, which are the backend's two planes (the safety plane and the application plane) and not one. In scene 3 only the application plane's buffer changes (1 commit, 1 release), and the safety buffer and its rectangle are unchanged. In scene 4 only the safety plane flips (1 commit) and the application buffer is unchanged. In scene 5 only the application plane is switched off. The R5F application allocates the two application buffers and the two safety buffers separately (`app_mem` and the driver's two safety buffers), and the plane addresses the driver programmed are the buffers' addresses (to be printed by the firmware; if it does not, the report says so).
- **Picture.** Scene 3 shows green replacing blue while the red rectangle stays at the same place; scene 4 shows the red rectangle moving from left to right while the green stays. A single combined buffer would need a redraw of the other layer in each of those scenes, which the log (no commit on the other plane, no release of its buffer) excludes.
- **Rule.** A scene passes only when both hold. A passing picture with a failing log is a failure; a passing log with a failing picture is a failure. The report's table has both columns for every scene.
- **Control run.** There is no flatten flag in the firmware. The report says "control run not run" unless the branch adds one; the move-style argument above stands in for it and the report names it as such.

## 4. Procedure through the remoteproc route

The route is the showcase's, `$MCUI_SHOWCASE_DIR/docs/remoteproc-bringup.md` (showcase). It leaves the boot chain alone: Linux (Torizon) runs, and the R5F is loaded by the kernel's remoteproc. The R5F code and its state is that of this repository's app; no showcase file is copied into the repository.

1. **Preconditions.** The board is up under Torizon and reachable over ssh. The camera is leased and framed (§6). The image is built (step 2 of the plan: `MCUI_SHOWCASE_DIR`, `YAKOGL_DIR`, `ZEPHYR_WORKSPACE`, `ZEPHYR_SDK_INSTALL_DIR`; the build is in `docs/board-test-am62p-r5f.md`, branch). A node without those values cannot do the build, and the item says so and stops.
2. **Record before changing anything.** The remote processor's current firmware name, path and state, the state of every service and driver that `remoteproc-bringup.md` lists as stopped or unbound, `uname -r`, `/etc/os-release`, the board model from the device tree (no serial numbers) and the DRM and DSS state, as found. Keep the output for the report and for the restore (§7).
3. **Install the image.** Copy the image to `/tmp` on `<board>` (nothing is installed into the root file system), and set it as the R5F's firmware through the `firmware_class` search path, as `scripts/imx95-m7-rproc.sh` (main) does for the i.MX95 M7. The planned `scripts/am62p-r5f-run.sh` does this, the recording and the restore; until it exists, the same steps by hand with every command logged.
4. **Stop or unbind** what `remoteproc-bringup.md` lists, so that Linux does not hold the display and the R5F takes it (for example the Linux display driver and anything that uses it). Each one is logged as it was stopped, for §7.
5. **Start the remote processor** and read the console from its **trace buffer** over ssh (the remoteproc debugfs trace file of the R5F). **The trace buffer is the console.** Poll it, store every line with the time it was read (the host's time, to correlate with the photographs), and keep the raw file unedited. The trace buffer is a ring: poll often enough that no scene line is lost (each scene holds for `TEST_HOLD_MS`), and say in the report if a line was lost.
6. **Photograph each scene.** The line of a scene is printed when the scene starts and the scene holds for `TEST_HOLD_MS`; take the pictures in that hold (§5, §6), after the line was seen in the trace buffer. Three pictures per scene.
7. **Wait for `RESULT`.** After the sequence the app idles with the last scene on the panel. Take the final picture, then stop.
8. **A second image needs a reboot.** A second stop of the remote processor fails (`docs/board-test-am62p-r5f.md`, branch). A fix of the app that is run again is a new image, and the run is repeated after a reboot into Torizon, with the failed run kept in the report as run 1.
9. **Restore (§7)** at the end of every session.

**Serial console: optional.** The UART of the board (`resource:serial-am62p` in the farm) is busy and the plan does not depend on it. If it is free during the session, a capture of the R5F's UART console next to the trace buffer is an extra and is stored as such; the verdict uses the trace buffer. Where the two differ, both are reported. The serial path is not written in any file.

## 5. Judging the photographs

The camera is `resource:tailvision-am62p` (target `am62p-camera`); its description gives its limits, and they are read from the lease. The rules of `docs/m1-imx95-test-plan.md` §2 apply: saturated flat colours, large regions, steady light, one throw-away frame first, three pictures per state with the median verdict, no camera move between the pictures of one comparison, every picture saved (used and discarded) in the evidence directory with the scene and the time in its file name. The picture is judged by `scripts/camera-check.py` (main):

```sh
python3 scripts/camera-check.py PICTURE EXPECT.json [--compare PICTURE2] [--rotation N] [--json]
```

- **EXPECT JSON per scene.** The planned `scripts/am62p-scenes.py` writes one from the colours and rectangles of `scenes.c` (branch). `scripts/scenario-expect.py` fits only if the planned tool is absent and the layout can be written as its input; otherwise the EXPECT files are written by hand from the table in §2.1 and kept in the evidence directory. The regions lie inside one rectangle, well away from edges (the README of `camera-check.py`).
- **Panel detection.** Scene 5's background is expected black, and the blue and green scenes and the safety rectangle touch dark areas, so `"panel": "auto"` is likely to find the plane union and not the panel. Use explicit corners (read from a picture of scene 1, the red full screen, which lights the whole panel) or `"panel": "full"` if the capture is edge to edge. State which was used.
- **Rotation.** The board camera's picture is upside down (default `--rotation 180`). Say in the report that the rotation was applied by the checker, and which value, from the camera's description of its own framing; check it on scene 2, whose red rectangle is on the left in the upright picture.
- **Scene 5 and 6 to 8.** The controller's background with the application plane off is "black expected": the picture says what it is, and the report records the colour as observed. The EXPECT for scenes 6 to 8 is that of scene 5, with a `--compare` against the picture of scene 5 asserting `"same"` for every region.
- **Colours.** RGB565 blue and green and the ARGB red are all saturated; use a tolerance that the distance column shows margin for, and take the colours from a picture of this camera if the first run shows an offset (the README's advice). A tolerance widened to make a scene pass is stated in the report with the value before and after.
- **Verdict.** `VERDICT: PASS` per picture, and the median over the three pictures of a scene is the scene's picture verdict. Exit status 2 (usage or input error) is not a verdict: fix the input and run again.

## 6. Camera framing and the panel

The camera is framed once, at the start of the session, on the Verdin AM62P's display, and not moved during the run (the `--compare` of scenes 6 to 8 requires it). The report names the panel (as the board's resource description and the device tree give it, with its resolution and refresh from the driver's own mode if the firmware prints it), the display interface and the application plane size limits (`TEST_APP_MAX_WIDTH`, `TEST_APP_MAX_HEIGHT`). The read-only probe of the board under Torizon and the camera framing (a separate item) comes first and its notes are cited; this plan does not repeat them.

## 7. Restore and Torizon confirmation

The owner's rule (2026-11-25): before a task that booted the Verdin AM62P into anything other than its Torizon boot chain ends, the board is put back on the Torizon chain and Torizon is confirmed up over ssh. The remoteproc route does not change the boot chain, but it stops and unbinds Linux parts, loads a firmware into the R5F and leaves it running, so the restore applies to every session, and the confirmation is made after a reboot.

At the end of every session, including one that failed or was cut short:

1. Stop the remote processor if it is running (a failed stop is recorded, and the reboot in step 4 is then what restores it).
2. Restore the remote processor's firmware name and path to the values recorded in §4 step 2, and its state.
3. Restart or rebind everything that was stopped or unbound in §4 step 4, in the reverse order, and check each by the same command that recorded it.
4. Reboot the board into its Torizon boot chain. If the boot chain was changed in any way (a boot media or boot order edit, a different boot image), put it back first; the remoteproc route alone does not change it, and the report says so.
5. Wait for the board and confirm over ssh that Torizon is up: `/etc/os-release` names Torizon, `uname -r` is the version recorded at the start, the R5F's firmware name is the recorded one, and the services of step 3 are running. Store the output.
6. Delete nothing of the board's: the image under `/tmp` may stay (it goes with the reboot).

If the board does not come back, the session ends with that stated first in the report and in the final message; nothing else is claimed as done. A board left outside its Torizon chain blocks other projects' runs until someone recovers it by hand.

The camera lease and the serial console are released as the farm requires; no change is made to either.

## 8. What the report must contain

`docs/m3-am62p-report.md` (planned): one dated report, in the style of `docs/m1-imx95-report.md` if that has landed, with:

1. **Header.** Date, the commit under test (the repository's and the image's), the build environment (the showcase's, yakogl's and the Zephyr workspace's commits and the SDK version, with the demo and yakogl pinned by commit), and what was and was not run.
2. **Board, BSP, kernel, panel.** Verdin AM62P module and carrier revision and the model string read from the board (no serial numbers), Torizon image version (`/etc/os-release`), `uname -r`, the R5F's remoteproc name, the panel and its mode, the camera and its limits.
3. **The run.** The raw trace buffer (unedited) and its reading times; the serial capture if one was taken; the command lines exactly as run; the restore and the Torizon confirmation (§7).
4. **A table of the eight scenes**: status (got and expected), commits and latched (cumulative and delta), the delta expected, presented and its time, releases (got and expected), the firmware's `ok` or `FAIL`, the three photographs, the checker's verdict for each, and the scene's overall result under the picture-and-log rule.
5. **The four things of M3-01** in one place: two-plane composition (scenes 2 to 5), latched count against commits (the GO bit, every scene), releases, refused scenes (6 to 8, 0 commits).
6. **The photographs**: every one, used and discarded, with the EXPECT JSON, the checker's output, the corners or panel mode, the rotation, and any tolerance that was changed.
7. **Runs.** Each image and each reboot: what failed, what was fixed in `tests/board/am62p_r5f` or `backends/twoplane`, and the result of the repeat. A failed run stays in the report.
8. **Verdict.** Per scene and overall: **passed**, **failed** or **blocked**, with the evidence reference. Blocked only for a limitation that is reproduced and recorded (the environment missing, the panel too large for the default buffers, the driver not usable from the R5F), with the command, the output and the versions. A scene without both log and picture is not passed.
9. **What the run did not show** (§9), so the open-issues rows are updated by what was shown and no more.

The report holds no board address, user name, credential, serial path or serial number; `scripts/check-no-board-secrets.sh` (main) is run over it.

## 9. What M3 does not cover here

| Item | Status |
| --- | --- |
| **M3-03**: safety-layer and producer-failure behaviour under real R5F, DSS and producer concurrency (producer stop and restart, safety flip during commits, no visible intermediate state) | Not covered. The eight scenes flip the safety plane between commits, but only from one context and without a producer that stops or restarts. Host tests with a fake driver stay the only evidence for those, and the board design for them is a separate item. The report does not claim them. |
| **M3-04**: the difference matrix versus the i.MX95 | Desk work after the run: `docs/observed/am62p.md` and `docs/am62p-imx95-matrix.md` (main) take the AM62P column from the report. |
| **M3-05**: the link in a real firmware build | Covered by the build of the image, if the image builds and links with the backend. A build that only compiles is not the link. |
| **M3-06**: optional third target (i.MX RT1170 or RA8P1) | Not covered. Decided after M1; not required for M3. |
| Linux DRM path on the AM62P (tidss, the Linux side of the display) | Not covered. M3 is the R5F and the DSS; the Linux path is a source study, not part of this run. |
| Throughput, latency to light, CPU and memory measurements | Not covered. Only the 1000 ms presented bound and the vsync-latched counts are judged. |
| The AM62P section of `docs/platform-capabilities.md` | Later, after the other platform items have landed. |
| Hotplug, modeset and display recovery | Not covered. |
| Safety claims | None. The "safety plane" is a layer role of the test; nothing here is a safety argument (SoW §7). |

## 10. Order of work

1. Bring `work/am62p-board-test` up to date with `main` (host: fmt, clippy, `cargo test --workspace`, the two `thumbv7em` builds of `docs/ci.md`).
2. Build the image on a node that has `MCUI_SHOWCASE_DIR`, `ZEPHYR_WORKSPACE` and `ZEPHYR_SDK_INSTALL_DIR`.
3. Board session: record (§4 step 2), run, photograph, judge, report, fix and repeat after a reboot, restore and confirm Torizon (§7).
4. Desk: `docs/observed/am62p.md` and `docs/am62p-imx95-matrix.md`.
5. Last, a small change of the M3 rows of `docs/open-issues.md` and `docs/sow-traceability.md` by what the report showed.
