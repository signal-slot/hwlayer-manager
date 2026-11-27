# M1 test plan: i.MX95 Linux DRM hardware composition

This is the WP0 "test scenarios" deliverable for milestone M1 of `SoW.md` (§9) on the Toradex Verdin iMX95 with the Toradex 10.1" LVDS panel, running Linux. It maps each of the seven acceptance conditions to scenarios, commands, pass and fail criteria, and the evidence to collect. The results go into `docs/m1-imx95-report.md`, not here.

The plan is board-independent: it holds no addresses, host names, users or serial paths. Where a command needs the board, it says `<board>`; the session setup supplies the real value.

## 0. Status marks

- **main** — exists on `main` today.
- **planned** — assigned to a running or queued item; the command line below is the intended interface and may change. The harness item's own `--help` wins over this text; update this plan when it lands.

| Tool | Status |
| --- | --- |
| `cargo test --workspace`, `cargo build` for `thumbv7em-none-eabihf` (CI, `docs/ci.md`) | main |
| `cargo run -p composition-drm --example probe` | main (prints derived `Capabilities`); raw KMS dump (connectors, `possible_crtcs`, `zpos`/alpha/blend/scaling properties, `IN_FORMATS`) planned (`probe-raw`) |
| `tools/drm-harness`, master-fd mode, scenarios `two-planes`, `zorder-alpha`, `move`, `combos`, `rotate` | planned (`harness`) |
| harness `--lease kernel-lease`, `--lease dlm`, scenario `revoke`, external producer processes | planned (`harness-sources`) |
| aarch64 cross-build recipe (`scripts/cross-aarch64.sh`), drm-lease-manager build (`scripts/build-dlm-aarch64.sh`) | planned |
| host picture checker for the camera frames | planned (`camera-check`); until then, regions are judged by eye and by sampled pixel values |

Harness conventions assumed throughout (planned): the harness takes `--device <path>` or `--lease <source>`, `--scenario <name>`, `--frames <n>`, and writes a structured trace (one line per event: `validate`, `commit`, `latched`, `released`, `dropped`, with timestamps and, for `commit`, the plane id and buffer id of every layer) to a log file given by `--log <file>`, plus a final summary (counts, latency, CPU, memory). Each layer is a separately allocated buffer, filled with a flat high-contrast colour (see §9). The harness holds the display for the scenario's duration, so the camera is taken while it runs (`--hold <seconds>` keeps the last frame on screen).

## 1. Common rules

**Flattened output is excluded.** SoW §9 condition 3 and WP2 say a composited single framebuffer does not count. Two buffers drawn into one framebuffer by the CPU or GPU and scanned out on one plane would show the same picture as two hardware planes, so a picture alone cannot prove hardware composition. The plan therefore never relies on the picture alone:

1. **Plane assignment from the commit.** The commit log lists, per layer, the plane id and the buffer id. Two layers on two distinct plane ids, both in the hardware plane list from the probe, and each layer backed by its own allocation (distinct buffer ids and DMA handles), is a necessary condition.
2. **The move test.** The harness moves one layer's plane (changes `CRTC_X`/`CRTC_Y`, or `SRC_*` for a crop) every frame while the other layer's buffer, framebuffer id and plane properties are unchanged, and writes no pixel into either buffer after the first fill (the buffer content is checked by checksum before and after). If the picture shows the moving rectangle sliding over a stationary one with correct overlap, only the display controller can have done it: nothing in software redrew the stationary buffer. A flattening path would have to rewrite a combined buffer each frame, which the harness would show as a buffer write and a changed checksum.
3. **The refusal test.** The `combos` scenario has combinations that the hardware refuses under TEST_ONLY (more layers than planes, an unsupported format, scaling a plane that cannot scale). A flattener would accept them. Refusals that match the probe's plane list support the claim that real planes are in use.

Condition 3 passes only when 1 and 2 hold and the camera confirms the picture. A passing picture with a failing log is a failure.

**Control run.** Once per session, run the `two-planes` scenario with `--flatten` (planned; one buffer, one plane, both rectangles pre-drawn) and take the same camera picture. It must look the same as the two-plane picture, which shows the picture alone is not proof, and it must fail the `move` test (the stationary rectangle's buffer changes). If the harness has no flatten flag, record that the control was not run.

**Failure is reported as failure.** A condition without the evidence below is "blocked" or "failed" in the report, never "passed". Condition 3 not verified on the board means M1 is incomplete (SoW exit rule).

## 2. Camera procedure

The camera is an MCP server pointed at the panel (target `imx95-camera`). Its resource description gives its limits; read them from the lease.

- **Orientation.** The picture is rotated 180 degrees. Rotate the expected layout 180 degrees when predicting where each colour appears (left becomes right, top becomes bottom), or rotate the picture back before checking. State in the report which was done. Do not use the panel's own rotation property to compensate.
- **Frame use.** Use the whole `capture_debug` frame, or a stated region of it. Do not use a crop whose borders the camera's own processing could clip. Record the frame size and the region in the report. Keep the pattern large (each region at least one fifth of the panel's width) so that the camera's focus and perspective error do not matter.
- **Patterns.** Use a few saturated, well-separated colours: red, green, blue, and white or black as background. Never use grey, pastel or near-black on near-black; the panel's backlight, the camera's auto-exposure and gamma move those values. Each plane is one flat colour (a gradient hides edges). Edges are the evidence for z-order, so make the overlap region at least a quarter of the smaller rectangle.
- **Lighting.** Turn the room light on a steady level and keep daylight off the panel; flicker produces bands. Avoid reflections on the panel (angle the camera slightly or shade it). Let auto-exposure settle (take one throw-away frame first). Take at least three pictures per state and use the median verdict; discard a picture with banding or glare and say so. Do not move the camera between pictures of one scenario; for an alpha test, take a reference picture of each plane alone first, under the same exposure.
- **Colour comparison.** Compare colours by hue class and relative ordering, not absolute RGB. For the alpha blend, use the reference pictures of each plane alone, as seen by this camera, to calculate the expected blend (`alpha * top + (1 - alpha) * bottom`, per channel, using the straight or premultiplied formula the plane's `pixel blend mode` property reports) and compare against the observed overlap with a tolerance (see condition 3). Camera gamma and auto-exposure make the tolerance wide; the criterion is a monotone, correctly ordered response, not a calibrated one.
- **Regions to sample.** Take the median over a small block well inside each region (at least 20 pixels from every edge), never a single pixel.
- **Save.** Save every picture used in the verdict into the evidence directory with the scenario, the state, and the time in the file name. Pictures used and pictures discarded are both kept.

## 3. Condition 1: `no_std` core and builds

**What is tested.** The common core builds without `std`; host tests pass; the Linux i.MX95 build passes.

**How.**

- `cargo test --workspace` (main).
- `cargo build -p composition-core --target thumbv7em-none-eabihf` and the static library with `panic-handler`, as CI does (main; see `docs/ci.md`).
- aarch64 Linux build of `composition-drm`, the probe and the harness, with the documented cross-build recipe (planned: `scripts/cross-aarch64.sh`), and the binaries start on the board (`--help` returns).
- Check the CI run for the commit that is under test is green (CI is host-only and proves nothing about hardware).

**Pass.** All commands exit 0; the aarch64 binaries exist and run on the board; the commit is named. **Fail.** Any build or test failure, or the aarch64 binary does not run on the board's kernel/libc.

**Evidence.** Commit hash, tool versions (`rustc --version`, target list), command output tails, CI run link, `file` of each aarch64 binary, `--help` output from the board.

## 4. Condition 2: topology captured from the board

**What is tested.** The DRM topology as the board's kernel exposes it, not derived from other i.MX generations: CRTCs, connectors (the LVDS output), encoders, planes with type (primary, overlay, cursor), `possible_crtcs`, formats and modifiers (`IN_FORMATS`), `zpos`, `alpha`, `pixel blend mode`, scaling, rotation and colour properties, the mode and refresh.

**How (planned: `probe-raw`).**

```sh
cargo run -p composition-drm --example probe            # host run (vkms or the fake setup), as today
cargo build --release --target aarch64-unknown-linux-gnu -p composition-drm --example probe   # cross-build (planned recipe: scripts/cross-aarch64.sh); copy the binary to /tmp on <board>
/tmp/probe --device <card>                              # derived Capabilities + raw KMS properties
```

Run it first without DRM master (the probe makes no commits) and again while no other DRM client holds the device, so the output is not filtered by a running compositor. Store the full raw output unedited.

Then compare against `DriverProfile::Imx95Dpu` in `crates/composition-drm`. Each profile field is recorded in `docs/platform-capabilities.md` with the tags observed, read from source, inferred; where the profile differs, it is corrected or marked, and the difference is listed.

**Pass.**

- The raw dump lists at least one connector reporting connected with the LVDS panel's mode, at least one CRTC, and every plane the kernel exposes, with type and properties.
- Every field recorded in `docs/platform-capabilities.md` for i.MX95 carries a tag, and every field the board shows is tagged observed.
- Every disagreement between observation and `DriverProfile::Imx95Dpu` is corrected or marked (not silently left).
- The number of distinct non-cursor planes that can reach the LVDS CRTC is at least two. If it is one, condition 3 cannot pass on this board and the report says so.

**Fail.** Missing output, or output that may have been edited; a profile field left unmarked after a disagreement; any "observed" tag without raw output behind it.

**Evidence.** Raw probe output file, kernel version (`uname -r`), module and carrier revision and the model string as read from the board (never serial numbers), the diff table (observed vs profile).

## 5. Condition 3: two hardware planes, z-order, alpha

**What is tested.** Two layers with separately allocated buffers on two distinct planes displayed at the same time on the LVDS output; z-order (which one is on top) and alpha behaviour where the plane has an alpha property.

### 5.1 Scenario `two-planes` (planned)

```sh
/tmp/drm-harness --device <card> --scenario two-planes --frames 300 --hold 60 --log two-planes.log
```

Layout (panel coordinates, before the camera's 180 degree rotation): a red rectangle in the left 60 % of the panel (plane A, lower z-order), a blue rectangle in the right 60 % (plane B, higher), on a black or white background; the middle 20 % is the overlap.

**Pass.**
- The log's `commit` lines show two layers with two distinct plane ids, both listed in the probe's plane list, both types allowed by the probe (not the cursor), and both bound to the LVDS CRTC; distinct buffer ids and distinct allocations.
- Each commit was preceded by a successful TEST_ONLY `validate` of the same state.
- Camera: both colours visible, each in its expected region (rotation accounted for), each region's median classified as the expected hue.
- Overlap region shows the top plane's colour (blue) with no red in it.

**Fail.** Same plane id for both layers, one layer absent, either colour missing or in the wrong region, the overlap showing the bottom colour or a mix when alpha is opaque.

### 5.2 Scenario `zorder-alpha` (planned)

```sh
/tmp/drm-harness --device <card> --scenario zorder-alpha --hold 60 --log zorder-alpha.log
```

Runs the states below in sequence, with a pause for the camera in each; each state is a new atomic commit on the same two planes.

| State | Setting | Expected overlap colour |
| --- | --- | --- |
| a | A below B, both opaque | B's colour |
| b | z-order swapped (via `zpos`, if both planes' `zpos` are mutable; otherwise swap the buffers' plane assignment and say so) | A's colour |
| c | B at alpha 50 %, blend as the plane reports | blend of B and A |
| d | B at alpha 0 | A's colour, B absent |
| e | B at alpha 100 % again | B's colour |

**Pass.**
- States a, b, d, e: the overlap's median colour has hue class equal to the expected top plane, and is nearer in colour distance to that plane's reference picture than to the other's.
- State c: the overlap lies between the two references per channel, within a tolerance of 25 % of the channel range from `alpha * B + (1 - alpha) * A` (references taken as seen by the camera). The tolerance is wide on purpose (camera gamma and exposure); it is the correct ordering and the clear difference from a and e that matter.
- If the probe shows a plane has no `alpha` property, state c and d are "not applicable on this plane" and are reported so, with the probe line; condition 3 may still pass on z-order alone, but the report says alpha was not verified.
- The per-state `commit` logs show unchanged plane ids except where stated.

**Fail.** Overlap colour does not follow the setting in any state; 50 % looks like a or e; the commit log shows a changed buffer for a state that changed only a plane property (the engine redrew rather than reprogrammed).

### 5.3 Scenario `move` (planned): anti-flattening

```sh
/tmp/drm-harness --device <card> --scenario move --frames 600 --hold 60 --log move.log
```

Plane A shows a large stationary rectangle (buffer filled once). Plane B shows a smaller rectangle that moves by changing `CRTC_X`/`CRTC_Y` each frame. Neither buffer is written after the first fill.

**Pass.**
- The log shows A's plane id, framebuffer id and buffer id identical in every commit; B's plane id and framebuffer id identical, only its position property changing.
- The harness's checksum of both buffers is the same at start and end.
- Camera, two pictures at least 10 frames apart (the harness holds a different position on request, or the picture is taken while moving and the second at the hold): the stationary rectangle is at the same place and the same size in both pictures; the moving one is at different places; the overlap region follows the moving one with the expected top colour.
- A sampled block inside the part of A not covered by B has the same colour in both pictures.

**Fail.** Any write to a buffer after the first fill; the stationary rectangle's region differs between pictures; a plane id changes for no reason; one of the two layers is missing.

### 5.4 Evidence for condition 3

Log files for all three scenarios, the pictures (used and discarded) for every state, the reference pictures (each plane alone, background alone), the probe plane list, the checksum record, the plane-id table (layer → plane id → plane type), the control run (`--flatten`, or "not run"), the camera tool description (limits) as used.

## 6. Condition 4: supported and unsupported combinations

**What is tested.** Capability detection (the probe), planner validation (`composition-core`), and the kernel's atomic TEST_ONLY separately: each says accept or refuse, and the plan lists where they differ.

### 6.1 Scenario `combos` (planned)

```sh
/tmp/drm-harness --device <card> --scenario combos --log combos.log
```

For each case the harness prints three results on one line: the planner's verdict, the TEST_ONLY result (errno if refused), and the plane assignment. Cases (the exact set depends on the observed topology; the table below is the intent, and cases not applicable on this board are listed as such):

| Case | Expectation |
| --- | --- |
| 1 layer, full screen, format the plane lists | supported |
| 2 layers on two planes, overlap, opaque | supported |
| 2 layers, one with alpha | supported if both planes have alpha, otherwise planner refuses or reports it |
| N+1 layers, N the number of planes that reach the CRTC | refused: no legal assignment, the report names the unmet constraint |
| a format the plane's `IN_FORMATS` lacks | refused by the planner (capability), and TEST_ONLY refuses it when forced past the planner |
| a modifier the plane lacks | as above |
| scaling a layer on a plane without scaling (or beyond limits) | refused by the planner; TEST_ONLY refuses when forced |
| source rectangle or destination beyond the CRTC (clip) | as the profile says; compare with TEST_ONLY |
| a layer misaligned relative to the plane's alignment | as the profile says; compare with TEST_ONLY |
| layers needing a plane that the lease does not include | refused (covered again in condition 6) |

The "forced past the planner" path commits through the harness's `--skip-planner` (planned) to a TEST_ONLY only; it never commits a refused state for real.

**Pass.**
- At least one case supported and at least three refused (or as many as the topology allows), each refusal with a named reason in the report, none a crash.
- For each case, the planner's and the kernel's answers are recorded. A case where the planner refuses and the kernel would accept is an over-strict case; a case where the planner accepts and TEST_ONLY refuses is a profile error. Both are listed; the second is a **failure of condition 4** unless the harness then reports it as a refused frame at commit time with the explicit error (the engine's last line of defence), in which case it is a bug filed against `DriverProfile::Imx95Dpu`.
- Every accepted case is also shown to commit and display with the camera (use `two-planes` as the representative; other cases are TEST_ONLY and log only).
- Priority layers (SoW §6, item 7) are **planned / not applicable on main**: `composition-core` has no priority or protected-layer concept, so no priority case is part of condition 4's pass criteria. It is added when the core supports it.

**Fail.** A refused case that the engine commits anyway; an accepted case whose TEST_ONLY failed; a profile claim that disagrees with the kernel and is not recorded.

**Evidence.** `combos.log`, a table (case, planner, TEST_ONLY errno, assignment), the list of profile corrections that came out of it (handed to `profile-check`).

Host-side counterpart (main): the planner's unit tests (`cargo test -p composition-core`) cover refusals independently of the board; the report cites their names.

## 7. Condition 5: submission, latch, release, rotation

**What is tested.** `submitted`, `latched` (page-flip event with the sequence and time), and `released` are separate events; a buffer is not released before the flip event that replaced it is reported; repeated rotation overwrites no buffer that is displayed or pending; dropped frames are counted.

### 7.1 Scenario `rotate` (planned)

```sh
/tmp/drm-harness --device <card> --scenario rotate --frames 3000 --buffers 3 --log rotate.log
```

Each layer has `--buffers` (at least 3, and also 2 as a second run) buffers in rotation. Each buffer is filled with a pattern that encodes its frame number and layer (a header word plus a counter), and the harness fills a buffer only after it received `released` for it. The harness submits at a rate above the refresh (for some frames) and at the refresh rate, so dropped frames occur.

**Pass.**
- For every buffer id, in the log: `submitted` precedes `latched` precedes `released`; no `released` before its replacement's `latched`; no buffer is filled (a write begins) between its `submitted` and its `released`. A checker script over the log (planned) lists violations; zero is required.
- Every submission ends as exactly one of `latched` or `dropped`. `latched + dropped = submitted` at the end. The dropped count is reported in the summary and is not hidden when zero.
- A readback or checksum of the buffers on the board at the end shows each buffer holds the frame number it was last filled with (no overwrite while pending).
- Camera, a picture taken during the run: no tearing or mixed pattern, no layer showing another layer's pattern (the pattern encodes layer and frame).
- Page-flip event times are monotone, and the interval between latches is within one refresh period ± 20 % on all but the dropped frames; interval outliers are listed.
- The run does not leak: the buffer count at start equals the count at end, and the memory (condition 7) is flat.

**Fail.** Any ordering violation, a buffer written while pending or displayed, `latched + dropped != submitted`, a hung run (no event within 1 second), a missing event treated as a latch.

**Evidence.** `rotate.log`, the checker output, the summary, the readback record, one picture.

Host-side counterpart: the fake backend tests for delayed VSYNC and adversarial event order (planned: `fake-rotation-tests`) cover the same properties against the state machine without the board.

## 8. Condition 6: leased output without a Weston fork

**What is tested.** The engine works on a DRM lease fd, with the lease's own resources (not the whole device), without a modified Weston. If the BSP cannot do this, the limitation is reproduced and recorded, not hidden.

The board runs the Toradex Reference Multimedia Image (Weston by default). The lease route is chosen per `lease-feasibility` (a read-only survey of kernel config and the lease protocol). Routes, in order:

1. **Kernel lease** (planned): a small leaser process holds the DRM master, creates a lease with `DRM_IOCTL_MODE_CREATE_LEASE` for one connector, one CRTC and its planes, and passes the lease fd to the harness over a Unix socket, or runs the harness as a child with the fd inherited.
2. **drm-lease-manager / libdlmclient** (planned, best effort, built from `scripts/build-dlm-aarch64.sh`, run from `/tmp`, nothing installed into the image): the harness acquires the lease through the client library.
3. **Weston-owned output** is not tested in M1 (SoW §4.2, second mode); a note records that Weston holds the output in the default image and that Weston must be stopped, or the harness must use the output it does not own. Stopping Weston is part of the session setup and is restored at the end (the Toradex image's service must be running again).

### 8.1 Scenarios

```sh
/tmp/drm-harness --lease kernel-lease --scenario two-planes --log lease-kernel.log     # planned
/tmp/drm-harness --lease dlm --scenario two-planes --log lease-dlm.log                 # planned
/tmp/drm-harness --lease kernel-lease --scenario revoke --log lease-revoke.log         # planned
```

`revoke`: the leaser revokes the lease (or the lease-manager revokes it) while the harness is running; the harness must report revocation, stop committing, release resources, and not hang or crash. Optional: restart the harness after revocation and show a fresh lease works.

**Pass.**
- The lease fd's resource list (the harness prints what the lease exposes) contains a connector, a CRTC, and at least two planes; the harness uses only those; the `two-planes` pass criteria of condition 3 hold on the lease (same camera check, same plane id rules, plane ids belong to the lease's list).
- Weston's own code was not changed (the board's Weston binary is the image's; the report states its version).
- Revoke: the harness reports lease loss within one second of the revoke, no further commits are issued after it, the exit status is documented, and no kernel error requires a reboot.
- The lease manager's version and the leaser's source commit are named.

**Blocked, not failed.** The lease route cannot be demonstrated because of a kernel, BSP, lease-manager or Weston limitation (e.g. the kernel lacks the lease ioctls, the lease cannot include planes, `drm-lease-manager` cannot be built or run without installing into the image, the planes the CRTC needs are not part of the lease). The verdict "blocked" is allowed only if the limitation is reproducible and written as a blocking issue: the exact command, the exact error output, the kernel and library version, the steps to reproduce from a clean boot, and the issue's number. A limitation noted without a reproducing command is a failed condition.

**Fail.** The lease works only by modifying Weston; the harness uses planes outside the lease; commits continue after revocation; the limitation is unrecorded.

**Evidence.** `lease-*.log`, resource list output, the leaser's log, the versions, the camera pictures for the lease runs, or the limitation record (command, output, versions, issue link).

Note on the SoW's "independent Slint/Qt processes send rendered buffers to the engine": the producers are separate processes in the target deployment. M1's elementary harness may produce buffers in one process; the external-producer scenario below goes beyond it and is optional for condition 6.

### 8.2 Scenario: external producers (planned, optional)

```sh
/tmp/drm-harness --lease kernel-lease --scenario two-planes --producers external --log producers.log
```

Two small producer processes each allocate their own DMA-BUF and hand it over a Unix socket (`SCM_RIGHTS`). Passing: each plane is backed by a buffer allocated in a different process; the harness never writes to either; the picture is as in `two-planes`. If not on `main` by the time of the run, the report says "not run".

## 9. Condition 7: the report's measurements

**What is tested.** That the report states the items of condition 7, each with its source of measurement and date, from the logs of the scenarios above, not from memory.

| Item | Source |
| --- | --- |
| EVK/board revision | module and carrier revision and the model string read from the board (`/proc/device-tree/model`, hardware revision from the Toradex tools if installed). Serial numbers are not recorded |
| Display interface and panel | the probe's connector (LVDS), the panel's name from the resource description, the mode |
| BSP and kernel | `uname -r`, `/etc/os-release`, image version, NXP/Toradex kernel commit if the BSP exposes it, otherwise the version string with "commit not available" |
| Lease manager version | the library/daemon version or commit, or "kernel lease, no lease manager" or the blocking issue |
| Resolution and refresh | the probe's chosen mode, and the measured refresh from page-flip timestamps (median interval over the `rotate` run) |
| Plane assignments | the `commit` log table: layer, plane id, plane type, format, modifier, `zpos` |
| Observed errors | every non-zero errno in all logs, with the command; dmesg DRM lines from before and after each session (diff) |
| Frame loss | `dropped` count and `submitted`/`latched` counts from `rotate`; late frames (interval above 1.5 refresh periods) |
| Latency | harness's `commit` to `latched` time (median, p95, max), over at least 1000 frames, and a plane-assignment time (validate duration); end-to-end latency to light is not measured unless the camera can (it cannot at its frame rate; say so) |
| CPU | harness process CPU time over wall time during `rotate` (from `/proc/<pid>/stat`, or `getrusage`), and the board's load average, with the harness's own frame-generation cost reported separately |
| Memory | peak resident set (`VmHWM`) of the harness, and the DMA/CMA allocations (the buffers' total size, `/proc/meminfo` CMA fields before and after if available) |

**Pass.** Each row is filled with a measurement and the date it was taken, or says explicitly "not measurable" with the reason. Dates and the commit under test are in the report header.

**Fail.** A row is blank or contains a value without a source.

**Evidence.** The log files and the summaries named above; `dmesg` snapshots; version outputs.

## 10. Scenarios of SoW WP0 that M1 does not cover

| WP0 scenario | M1 status |
| --- | --- |
| Two independent UI layers | **Covered**: `two-planes`, `zorder-alpha`, `move` (conditions 3, 6) |
| Video + UI | **Later**. A video-format (NV12 or similar) layer on an overlay plane plus an ARGB UI plane is exercised in `combos` only as a format-support and TEST_ONLY case if the topology shows a YUV-capable plane; the picture and video decode path are not tested in M1 |
| Application + persistent priority/safety layer | **Later**, including priority-layer refusal, which is planned until the core has a priority concept. Enforced ownership/protection on the SoC is not tested on Linux; no safety claim (SoW §7) |
| Unsupported geometry | **Covered by `combos`**: scaling beyond limits, clip outside the CRTC, alignment |
| Producer crash/restart | **Later**. Partially: `revoke` tests the engine's response to losing the lease; a producer process killed while its buffer is on a plane (SoW §7, stale producers; and fallback policy) is not in M1. With external producers (§8.2) a crash test is a natural extension |

Hotplug, modeset, and display recovery (SoW §10 hardware integration tests: startup, steady state, app stop/restart, lease loss, modeset) are partly covered (startup, steady state, lease loss) and partly later (modeset, recovery).

## 11. Evidence directory and order of runs

Each session writes to its own directory named by the date and session (`m1/<date>-<session>/`), holding: the raw logs, the pictures (used and discarded), the version outputs, the command lines exactly as run, and a short text note of anything unexpected. Every command is run and recorded as typed; do not edit a log by hand.

Order of runs on the board:

1. `wp0-probe` session: probe (condition 2), versions (condition 7 items).
2. `m1-master-run` session: `two-planes`, control run, `zorder-alpha`, `move`, `combos`, `rotate` on a DRM master fd (conditions 3, 4, 5, 7).
3. `m1-lease-run` session: the lease scenarios with the same pass criteria (condition 6), re-run of `two-planes` through the lease.

At the end of every board session, restore the board to its running state (Weston service up, no harness or leaser left running, `/tmp` copies may stay) and say so in the log.

## 12. Verdict rules for the report

Each condition gets exactly one of **passed**, **failed**, **blocked**, with the evidence reference:

- Condition 1 passes on its commands alone.
- Condition 2 passes on the raw dump plus the tagged capabilities file plus the profile diff.
- Condition 3 passes only with the log criteria and the camera picture and the move test. Without them it is not passed, and **M1 is incomplete** (exit rule), whatever the others show.
- Condition 4 passes with supported and refused cases and the comparison table.
- Condition 5 passes with a zero-violation `rotate` run.
- Condition 6 passes with a working lease, or is "blocked" with a reproducible limitation record. A hidden limitation is "failed".
- Condition 7 passes when every row of §9 is filled.
- M1 as a whole is **passed** only if conditions 1 to 7 are each passed or (for condition 6) blocked with a recorded issue, and condition 3 is verified on the board.
