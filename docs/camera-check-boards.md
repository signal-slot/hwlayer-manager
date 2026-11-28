# camera-check across boards (X-05)

Written 2026-11-28. This page answers X-05 of [`open-issues.md`](open-issues.md): `scripts/camera-check.py` is not yet generic across boards or part of any regression run. It says, for every board path that produces or will produce pictures, where the EXPECT JSON comes from, which rotation and panel corners to use, and what log evidence must go with the picture.

**No picture and no board result is claimed here.** Every statement is tagged *read from source* followed by the file it was read from (scripts, their READMEs, docs). Nothing on this page was measured, photographed or run. Statements about what a plan *requires* are what the plan says, not what happened.

## The tool in one paragraph

*Read from source:* [`scripts/camera-check-README.md`](../scripts/camera-check-README.md). `camera-check.py PICTURE EXPECT.json [--compare PICTURE2] [--rotation N] [--panel full|auto] [--json]` rotates the picture clockwise by `rotation` (default 180, the board camera's picture is upside down), finds the panel (`auto` takes the bright area and needs a dark surround and a lit panel background; explicit corners are given in the rotated picture, order TL, TR, BR, BL; `full` takes the picture's own corners and is meant for KVM captures), measures regions (`rect` in panel-normalised coordinates, 16x16 sample grid) against `expect` colours within `tolerance` (default 40), and with `--compare` asserts `changed` or `same` between two pictures. Exit 0 is `VERDICT: PASS`, 1 is `FAIL`, 2 is a usage or input error. It does not depend on a board.

## The picture-and-log rule (all sections)

*Read from source:* [`m1-imx95-test-plan.md`](m1-imx95-test-plan.md) §1, [`m3-am62p-test-plan.md`](m3-am62p-test-plan.md) §3, and the closing note of `scripts/camera-check-README.md`.

- A picture alone cannot exclude a flattened buffer: two layers drawn into one buffer and scanned out on one plane look the same as two hardware planes.
- `camera-check` only shows what is on the panel. That a single flattened framebuffer was not used is shown by the commit's plane assignment and by the producers updating only one buffer. The picture is the visual half of the evidence.
- The i.MX95 plan states the rule as: a passing picture with a failing log is a failure. The AM62P plan adds the reverse: a passing log with a failing picture is a failure, and its report table has both columns for every scene.
- Pictures of one comparison are taken with the same camera position and exposure (README of `camera-check`; both plans).

So every section below names a log source next to the picture. A `VERDICT: PASS` without that log is not a pass of the board path.

## 1. i.MX95 M1

Source: [`m1-imx95-test-plan.md`](m1-imx95-test-plan.md) (camera procedure §2, condition 3 scenarios), read from source.

- **EXPECT JSON.** The plan says the harness fills each layer with a flat high-contrast colour and lists the layout in panel coordinates: a red rectangle in the left 60 % of the panel (lower plane), a blue one in the right 60 % (higher plane), the middle 20 % the overlap. The plan's table of states (z-order, alpha) gives the expected overlap colour. The plan does not name a generator for the JSON; the examples in `camera-check-README.md` (overlap colour, swapped z-order, 50 % alpha, move test) are the written-out forms.
- **Rotation.** The plan says the picture is rotated 180 degrees and the report states whether the expected layout or the picture was rotated; the plan says not to use the panel's rotation property to compensate. `camera-check` default 180 matches.
- **Corners.** The plan's table says the camera is the MCP server of target `imx95-camera`. It does not give corner values. The README says a black panel background needs explicit corners.
- **Log half.** The plan's commit log lists per layer the plane id and buffer id: two distinct planes from the probe's plane list, distinct allocations. The move test: one plane's position changes while the other's buffer, framebuffer id and plane properties do not, with buffer checksums before and after. A control run with `--flatten` is planned and must look the same as the two-plane picture and fail the move test; the plan says the control is recorded as "not run" if the harness has no such flag.
- **Pictures.** At least three per state, median verdict, every picture kept, colours judged by hue class and relative order with a wide tolerance (plan §2).

## 2. AM62P M3

Source: `scripts/am62p-scenes.py`, [`am62p-scenes-README.md`](../scripts/am62p-scenes-README.md), [`m3-am62p-test-plan.md`](m3-am62p-test-plan.md) §2 to §6, read from source.

- **EXPECT JSON.** `python3 scripts/am62p-scenes.py expect --scene N|NAME --width W --height H [--tolerance N] [--rotation N] [--panel full|corners] [--corners ...]` prints the EXPECT of one of eight scenes. The colours and geometry are written into the script with the `scenes.c` line they come from (commit `361e31f`); if `scenes.c` changes, the script must be changed (README). Scenes 6 to 8 use the EXPECT of scene 5; the test plan adds a `--compare` against the picture of scene 5 asserting `same`.
- **Rotation.** README: default 180 for the board camera, 0 for a KVM picture. Test plan §5: the report says the rotation was applied by the checker and which value, and checks it on scene 2 (red rectangle on the left in the upright picture).
- **Corners.** README: `--panel full` is the default; `--panel corners --corners TLx,TLy,TRx,TRy,BRx,BRy,BLx,BLy` takes the panel corners in the rotated picture as fractions. Test plan §5: scene backgrounds are black, so auto detection is likely to find the plane union; use explicit corners read from a picture of scene 1 (red full screen lights the whole panel), or `full` if the capture is edge to edge; the report states which.
- **Tolerance.** Default 60 in the script. The plan says a tolerance widened to make a scene pass is reported with the values before and after.
- **Log half.** The firmware prints a `SCENE` line per scene and a `RESULT` line; `am62p-scenes.py summary CONSOLE|-` turns them into a table. The plan requires status as expected, `latched` equal to `commits` after the frame (the GO bit), the expected delta, no commit change while held, and the expected release count. Console source is the remoteproc trace buffer; the UART is optional (plan §4).
- **Picture and log together.** Plan §3: scene 3 must show green replacing blue with the red rectangle in place, scene 4 the red rectangle moving with the green in place; the log (no commit on the other plane, no release of its buffer) excludes a redraw of a combined buffer.
- **Self-test.** `scripts/am62p-scenes.test.sh` draws synthetic pictures from the firmware geometry, checks them with `camera-check.py` and checks that a wrong picture fails (README). This tests the tools, not the board.

## 3. KVM HDMI captures

Source: [`linux-board-scenarios.md`](linux-board-scenarios.md) ("How KVM pictures are judged"), `camera-check-README.md`, read from source.

- **EXPECT JSON.** The run page says expected colours come from the harness's colour constants, with the full frame as panel corners.
- **Rotation and corners.** README: `--panel full --rotation 0`. A KVM capture shows the whole screen edge to edge with no surround, so the picture's own corners are the panel. `"panel": "full"` and `"rotation": 0` can be set in the file or on the command line (the command line wins).
- **Timing.** The run page says the KVM picture runs about 10 to 20 s behind the HDMI output, so a picture is credited to a run by what it shows, and should be taken at least 20 s into a hold; its `--hook CMD` option gives `hold-start` and `hold-end` calls for taking the picture.
- **Rotation value in that run.** The run page says the `--rotation` value used was not recorded in its run report.
- **Log half.** The runner's log result and the counts (commits, latched, released) are the other column of the run page's tables.

## 4. twoplane-expect.py and alpha-mode-expect.py

Source: `scripts/twoplane-expect.py`, `scripts/alpha-mode-expect.py` (docstrings), read from source.

**twoplane-expect.py**

- **EXPECT JSON.** Reads the stdout of `twoplane` or `plane_commit` (file or `-`) and writes `DIR/expect-<phase>.json` for every phase that ran, from `EXPECT phase=... "name" x= y= fx= fy= rgb=#rrggbb` lines (the lines are printed by `crates/composition-drm/examples/plane_commit.rs` and the twoplane example). A phase with a `skipped` token, or `swapped` after `SWAPZ skipped`, gets no file and is listed.
- **Rotation and corners.** Defaults are rotation 0 and `--panel full`; options `--rotation`, `--panel corners --corners`. Default tolerance 40 and sample size 0.02 of the panel (`--size`). So the default output suits a KVM capture; a board camera needs `--rotation 180` and, for a dark surround, corners.
- **Log half.** The EXPECT lines come from the program's own output, so that same output is the log; the commit-per-plane evidence is the program's other lines, which this page does not read.

**alpha-mode-expect.py**

- **EXPECT JSON.** Computes the overlay colour for premultiplied, straight and no-alpha models over each under-colour, from the constants of `crates/composition-drm/examples/twoplane.rs` (magenta `ff00ff` at pixel alpha `0x80` over green `00ff00` and background `202020`; other values by option). `--expect-json FILE --model MODEL --rect u0,v0,u1,v1` writes one EXPECT, one region, the first under colour.
- **Rotation and corners.** The written JSON has `rotation` 0 and full-panel fraction corners (`expect_doc`), the KVM form. For a board camera, change the rotation and corners by hand or by command line of `camera-check`.
- **What a picture can tell.** Its docstring says a well-formed premultiplied and straight run give nearly the same colour, so a camera check cannot separate them; the "mismatch" table (buffer written for one model, hardware blending another) is what separates an ignored or inverted alpha property. The camera tolerance matters: models are distinguishable only where the largest channel difference exceeds twice the tolerance.
- **Log half.** The `twoplane --blend X` run that produced the picture is named in the docstring as the run to compare against the "mismatch" row; the blend mode the run used is the log.

## What is missing for a generic regression run

All rows are *read from source* in the sense that the item named in the third column is absent from, or open in, the files cited; none was tried.

| # | Missing | Where the gap shows | Needed |
| --- | --- | --- | --- |
| 1 | One EXPECT source with one interface for all boards | Four generators (hand-written examples, `am62p-scenes.py`, `twoplane-expect.py`, `alpha-mode-expect.py`) with different option sets and different default rotations (180 or 0) | A common wrapper or a shared convention for rotation, panel and tolerance per capture path |
| 2 | Per-capture-path settings recorded in one place | Rotation per camera or KVM is stated in prose; the Linux run page says its value was not recorded | A small table or file: capture path, rotation, panel mode, corners, tolerance |
| 3 | Panel corners for black-background scenes | README: auto detection needs a lit background; AM62P plan: corners from a picture of scene 1 | A way to derive corners from a bright reference picture and reuse them for later pictures of the session |
| 4 | Picture and log judged by one command | `camera-check.py` takes no log; `am62p-scenes.py summary` takes no picture | A runner that requires both verdicts and fails on either |
| 5 | Median over three pictures | Both plans ask for three pictures and the median verdict; `camera-check.py` judges one picture per call | A script that takes several pictures of a state and reports the median verdict |
| 6 | Regression hook | No script in `scripts/check-all.sh` takes a picture; board evidence is not part of any host regression (`testing.md`) | A decision on whether picture checks run in a board-gated lane, and what the host self-tests (`camera-check.py --self-test`, `am62p-scenes.test.sh`) stand in for |
| 7 | Flattened-buffer control | i.MX95 plan: `--flatten` control run is planned and may be "not run"; the AM62P plan relies on the log alone | A control picture-and-log on at least one board |
| 8 | Keeping generators in sync with firmware | `am62p-scenes.py` copies `scenes.c` geometry by hand | A sync check (another item of the plan) |
| 9 | Picture timing on KVM | Run page: pictures lag 10 to 20 s | A timestamped capture tied to the hold hook |
| 10 | Zephyr path (M2) | X-05 names it as a reuse target; no document on this page's sources gives its EXPECT source | An EXPECT source and a capture path for M2 |
