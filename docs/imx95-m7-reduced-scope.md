# i.MX95 M7: reduced-scope bring-up if the M7 cannot own the DPU

Date written: 2026-11-28. Method: reading this repository's recorded docs and
scripts. Nothing here was run. **Everything below is not measured.** No board
was touched for this file.

Tags on every statement: **read from source** (a source named in the cited
repository doc says so; the pins are in that doc) or **inferred** (my deduction,
no source says it). Statements about what a step "proves" are **inferred**.

## 1. Why a reduced scope

SoW M2 asks for scanout and VSYNC/latch reporting from Zephyr on the i.MX95 M7,
then overlay. Four facts, all from the repository's recorded docs, make that
uncertain on the Verdin iMX95 as it boots today:

- Upstream Zephyr's DPU and DSI nodes exist only for the M7 and are disabled by
  default; neither EVK board lists display support. **Read from source**
  (`docs/platform-capabilities.md`, section "i.MX95, Zephyr").
- In the stock System Manager configuration the display blocks (`DC*`,
  `BLK_CTRL_DISPLAYMIX`, `LVDS`, `GPV_DISPLAY`) are owned by LM0, the logical
  machine Linux runs in. **Read from source**
  ([docs/imx95-sm-display-lm.md](imx95-sm-display-lm.md) section 2). Giving
  them to the M7 needs a changed SM image, which cannot be applied on the
  shared board (same doc, section 6; **inferred** that the board rules forbid
  it: the SM image lives in the boot container, outside `/tmp`).
- The stock Zephyr DPU driver makes no clock, power-domain or SCMI call, drives
  one plane, and enables only the ExtDst0 shadow-load interrupt through a
  hardcoded register block. **Read from source**
  ([docs/imx95-m7-backend-contract.md](imx95-m7-backend-contract.md) section 2).
- Weston owns the display and holds DRM master. **Read from source**
  ([docs/imx95-board.md](imx95-board.md), "Board rules"). `weston-imx` has no
  DRM lease code at the revisions searched; whether the Toradex image uses it
  is not established. **Read from source / inferred**
  ([docs/weston-imx-lease.md](weston-imx-lease.md) section 1).

So the reduced scope keeps the display with Linux and asks what the M7 can
still prove. It is a fallback, not parity with the Linux result and not M2.

## 2. Common preconditions and board rules

All candidates need the M7 to start from Linux remoteproc with no change to the
root filesystem, U-Boot environment or eMMC boot partitions. The recorded
snapshot shows an `imx-rproc` remoteproc that is `offline`, with no M7 image
installed (**read from source** of
[docs/imx95-zephyr/observed-2026-11-27.txt](imx95-zephyr/observed-2026-11-27.txt),
which is itself tagged observed on 2026-11-27; not re-observed here). Whether
the stock SM lets the M7 logical machine run is **not measured**.

Rules from [docs/imx95-board.md](imx95-board.md), which every candidate must
respect (**read from source**):

- Write only under `/tmp`. No packages. No `fw_setenv` or `saveenv`. Never write
  `mmcblk0boot0` or `mmcblk0boot1`.
- Weston holds DRM master. If a step stops it, restore it and confirm
  `systemctl is-active weston` before leaving. The candidates are written so
  that they do not need to stop it; a variant that would is marked.
- No addresses, host names, users or serial paths in files or commits. Use
  `<board>` placeholders.
- Board time comes from a lease for the specific resource; observation only
  unless the lease grants more. M7 start and stop use
  `scripts/imx95-m7-rproc.sh`; its header says it writes only the remoteproc
  `firmware` and `state` files and the `firmware_class` path, under `/sys`, and
  restores them (**read from source**). Its behaviour on a board is **not
  measured**: it has only a host test, `scripts/imx95-m7-rproc.test.sh`.

Repository paths the candidates would use. The `tests/board/...` paths do not
exist in this snapshot; they are names for the items that build them
(**inferred**):

| Path | Role |
| --- | --- |
| `scripts/imx95-m7-rproc.sh` | load ELF from `/tmp`, run, capture, stop, restore |
| `tests/board/imx95_m7/` | M7 hello and DPU ownership probe application |
| `tests/board/imx95_m7_engine/` | M7 application linking `composition-ffi-staticlib` |
| `tests/board/imx95_m7_shm/` | M7 renderer for the shared-memory candidate |
| `crates/composition-ffi-staticlib` | the engine as `libcomposition_engine.a` (`no_std` with `panic-handler`) |
| `crates/composition-ffi/include/composition_engine.h` | C header the Zephyr app includes |
| `crates/composition-fake` | host reference for the scene the M7 would plan |

## 3. Candidate A: M7 read-only probe of DPU register ownership

Steps:

1. Build an upstream-Zephyr app in `tests/board/imx95_m7/` that, after printing
   a banner, performs single aligned 32-bit reads of a short list of DPU,
   DISPLAYMIX block-control and LVDS registers, one at a time, and prints the
   value or the fault. No write to any of those addresses.
2. Install a fault handler that records a bus fault, prints it and continues.
3. Load with `scripts/imx95-m7-rproc.sh --log <log> <elf> <index>`, run a few
   seconds; the script stops the core and restores settings.
4. Compare the reads with the Linux-side register view in
   [docs/imx95-dpu-source-6.6.md](imx95-dpu-source-6.6.md).

Proves (**inferred**): whether the M7's bus view reaches the DPU registers under
the stock SM grants, so whether M2 is blocked by resource assignment before any
driver question. A fault answers "blocked by resource assignment". A clean read
answers only "readable", not "ownable".

Does not prove: that the M7 could program the DPU; that clocks or power domains
are on for the M7 (it never asks SCMI); anything about VSYNC, scanout, overlay
or timing.

Risks (**inferred**): reading registers of a block Linux is using is not known
to be side-effect free; a clear-on-read status register would disturb Linux's
DPU driver, so the list must exclude interrupt status registers. Reads while a
clock is gated may hang the bus instead of faulting. Both **not measured**.

Verdict lines (printed by the M7 per register, then a summary; the last is
written by the host script):

```
M7P: reg <name> <value-hex>
M7P: reg <name> FAULT <fault-kind>
M7P: verdict A READABLE
M7P: verdict A BLOCKED <n>
M7P: verdict A HUNG
```

`READABLE`: all probed registers read. `BLOCKED <n>`: n registers faulted.
`HUNG`: no summary line within the run time.

Board rules: nothing beyond section 2. Weston stays up.

## 4. Candidate B: the engine on the M7, plan reported over a log channel

Steps:

1. Build `tests/board/imx95_m7_engine/` as an upstream-Zephyr app linking
   `composition-ffi-staticlib`, built with
   `cargo build -p composition-ffi-staticlib --no-default-features --features panic-handler --target thumbv7em-none-eabihf --release`
   (the build [docs/ci.md](ci.md) already runs on the host).
2. The app registers a C backend through `ce_backend_ops` that reports a fixed
   synthetic output and planes (a fake, not the DPU), runs one fixed scene
   through the planner, and prints the plan as text lines. The scene is taken
   from [docs/m2-comparison-plan.md](m2-comparison-plan.md), cut down to what
   the fake can describe.
3. The lines go out over the M7 rpmsg log channel if the resource table and
   OpenAMP are in the image, otherwise over the M7 console UART. The channel
   is a separate design item (`docs/imx95-m7-rpmsg-log.md` if it has landed;
   it is not in this snapshot). The host script's `--log` reads it.
4. Load, run, stop and restore with `scripts/imx95-m7-rproc.sh`.
5. On the host, run the same scene through `composition-fake` and diff the
   plan text.

Proves (**inferred**): the `no_std` engine, built for `thumbv7em-none-eabihf`,
starts and plans on the real Cortex-M7 under Zephyr; the M7 can be started and
stopped from Linux without touching the rootfs, U-Boot env or eMMC boot
partitions; a log channel to Linux works; the plan matches the host plan.

Does not prove: any display access, scanout, VSYNC or latch (the backend is a
fake, so every `Latched` event would be synthetic and must be labelled so);
overlay; DPU ownership; timing against Linux. It cannot support an M2 claim
about the display.

Verdict lines:

```
M7E: engine start ok abi=<n>
M7E: plan <output> <plane> <buffer> <x> <y> <w> <h>
M7E: backend=fake events=<n>
M7E: verdict B PASS plan-equal=yes
M7E: verdict B FAIL <reason>
```

`PASS` is printed on the host after an empty plan diff; the M7 cannot print it.
`FAIL` reasons: no start, no log, plan mismatch.

Board rules: nothing beyond section 2. The display is not touched.

## 5. Candidate C: M7 renders into shared memory, Linux scans out an overlay

Steps:

1. `tests/board/imx95_m7_shm/` renders a known pattern (gradient with a frame
   counter in a corner) into a DDR region both sides can address, and logs a
   counter and a checksum.
2. A small Linux program kept in the repository (a new directory under
   `tools/`, not created here) maps that region, checks the checksum, and tries
   to scan it out on an overlay plane using the atomic API wrapped by
   `crates/composition-drm`, without stopping Weston.
3. The program reports "could not" if it is not DRM master, the plane check
   fails, or the buffer cannot be imported.
4. Load, run, stop and restore with `scripts/imx95-m7-rproc.sh`.

The memory to use is open. The Linux device tree reserves DDR for the M7
([docs/imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md) section 2.4, read from
upstream patches, not from the 6.6.157-7.8.0 BSP tree). A different region
needs a device-tree change, which the rules do not allow. Importing that memory
into a DRM buffer needs `/dev/mem` (availability **not measured**) or a driver;
none is known in the image (**inferred**).

Proves (**inferred**):

- Up to the checksum: the M7 and Linux see the same bytes in a shared region.
  That is a memory-sharing result.
- The overlay attempt: whether a non-master Linux process can place a plane
  next to Weston. Expected outcome is "could not": an atomic commit needs DRM
  master, Weston holds it, and no lease source exists in `weston-imx` as read
  (section 1). **Inferred**, **not measured**.

Does not prove: the M7 scanning out (Linux does the scanout; the M7 never
touches the DPU), M7 VSYNC reporting, M7-owned latching, or a Linux versus
Zephyr scanout comparison. A pass would be Linux scanning M7-rendered pixels,
a different claim from M2.

Variant that stops Weston: the Linux program takes DRM master itself. That fits
[docs/imx95-board.md](imx95-board.md) only if Weston is restored and confirmed
active afterwards, and only with a display lease for that window. Not
recommended for a first run.

Verdict lines:

```
M7S: frame <n> checksum <hex>
SHM: map ok checksum-match=<yes|no>
SHM: overlay verdict C COULD-NOT <reason>
SHM: overlay verdict C PASS plane=<id>
```

`COULD-NOT` reasons: not master, no lease, import failed, plane rejected.
`PASS` means the kernel accepted the commit; that pixels were shown needs a
camera or panel check and is out of this scope.

## 6. Comparison

| | A: register probe | B: engine over log | C: shared-memory overlay |
| --- | --- | --- | --- |
| Needs SM change | no | no | no |
| Needs DT or U-Boot change | no | no | yes for a new region, else open |
| Touches the display | reads only | no | yes (Linux side) |
| Weston affected | no | no | no, unless the master variant |
| Answers the M2 ownership question | partly | no | no |
| Exercises the engine on the M7 | no | yes | no |
| Main unknown | register read behaviour | log channel | memory import, DRM master |

All cells **inferred**; none measured.

## 7. Recommendation

Run **B first**, and **A as a second image in the same session** only if B's
load and stop work. Do not run C until a lease source or an import path is
established.

Reasons (**inferred**):

- B depends only on what the remoteproc gate already requires (start and stop
  from Linux) and on the existing static-library build, so it is runnable even
  if every display question stays closed.
- B gives groundwork for WP1 and WP3 (the engine on the target core) with no
  display claim.
- A is the only candidate that speaks to ownership, but it needs a register
  list from the DPU source study and carries the unmeasured read-side-effect
  risk, so it follows B.
- C's expected result is "could not" for reasons already known from source; a
  board run would mostly confirm them.

Statement for the M2 report if only B and A run: scanout and VSYNC/latch from
Zephyr remain **not proven**, and the blocker report carries the SM ownership
and driver facts of section 1. Nothing here is parity with the Linux i.MX95
result or with any other SoC.

## 8. Order of a session

1. Check the lease and that no other item holds the board; confirm the M7
   remoteproc is `offline`.
2. Run B, then A, each through `scripts/imx95-m7-rproc.sh` with logs under `/tmp`.
3. Remove the files under `/tmp`; confirm the remoteproc `firmware` name and
   `firmware_class` path equal what the script recorded; confirm Weston is
   active.
4. Report the verdict lines verbatim, dated, tagged **observed**.

## Not measured

Every item in this file: no candidate was built or run; the M7's ability to
start under the stock SM; the existence of the log channel; register
readability; Linux import of M7 memory; Weston's behaviour towards a second
client.
