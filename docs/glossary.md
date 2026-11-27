# Glossary

Terms used in `SoW.md` and `docs/`, defined as this repository uses them.
Each entry has one or two sentences and the file where the term is first used
or defined ("first" by reading `SoW.md` first, then the other files; it is not
a `git log` search). Nothing here is a hardware claim: where a term names
hardware, the entry says what the repository records, not what a board does.
Where the repository's use is unclear, the entry says so.

Terms the repository does not use are listed at the end instead of being
defined from outside knowledge (search of 2026-11-28).

## A

**A55**: The Cortex-A55 application cores of the i.MX95, where Linux runs. The
repository contrasts them with the M7 when asking who owns the display. First
used: `README.md`; the ownership question is in `docs/platform-capabilities.md`.

**AFBC**: A compressed buffer modifier. The repository mentions it only in the
RK3588 notes; the engine's own model is `FormatModifier`. First used:
`docs/observed/rk3588-source.md`.

**AM62P**: The TI SoC of the existing Zephyr/R5F demo and of milestone M3. The
repository knows its display path from the SoW and the twoplane note only.
First used: `SoW.md` §5.2; compared with the i.MX95 in
`docs/am62p-imx95-matrix.md`.

**atomic commit (DRM)**: A KMS request that applies many property changes
together, or, with `TEST_ONLY`, only checks them. The engine's DRM backend
uses it for its own `commit`. First used: `SoW.md` §4.1.

## B

**backend**: Code that implements the `DisplayBackend` trait and talks to a
display: Linux DRM/KMS, an MCU display controller, or a fake. First used:
`SoW.md` §4; the trait is in `crates/composition-core/src/backend.rs`.

**BackendEvent**: What a backend reports to the engine (latched, released,
device lost, capabilities changed). First defined:
`crates/composition-core/src/backend.rs`.

**bootaux**: The U-Boot command that starts a Cortex-M core from a loaded
image. The repository records it from a Toradex M7 note, not from running it.
First used: [imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md); the MCUXpresso
text read does not cover it ([imx95-mcux-display.md](imx95-mcux-display.md)).

**buffer**: A registered image the producer draws into; it moves through the
states listed under *submitted / writable / latched / released*. First
defined: `crates/composition-core/src/presentation.rs`.

## C

**capabilities**: The engine's description of a display: outputs, planes,
formats, scaling, z-order and resource groups. First defined:
`crates/composition-core/src/caps.rs`.

**CM4**: The Raspberry Pi Compute Module 4 (BCM2711), not a Cortex-M4. First
used: `README.md`; its probe is in `docs/observed/bcm2711-cm4.md`.

**commit**: Two meanings that the repository keeps apart. `Engine::commit(plan)`
submits an accepted plan to a backend and returns a `SubmissionId`
(`SoW.md` §4); a DRM atomic commit is what the DRM backend issues to do it. A
commit does not mean the frame is on screen (see *latch*). First used:
`SoW.md`.

**CRTC**: The DRM object that scans out a set of planes to a connector. The
engine calls its counterpart an *output*; the repository does not write the
mapping out in one place. First used: `SoW.md` §4.1; used in
`crates/composition-drm/src/lib.rs`.

## D

**DeviceLost**: The event that the backend lost its display resources (reset,
lease revoked); everything pending is dropped and all buffers are released.
First defined: `crates/composition-core/src/presentation.rs`.

**DLM**: Short for *drm-lease-manager* in `docs/dlm-aarch64.md`.

**DPU**: The i.MX95 display processing unit. The repository knows it from
driver source (`dpu95`) and device-tree text, not from running it. First
used: `SoW.md`; the source reading is `docs/imx95-dpu-source-6.6.md`.

**DRM lease**: A DRM mechanism by which a master hands a subset of its objects
(connectors, CRTCs, planes) to another process as a file descriptor. The
repository treats a lease as exclusive: it does not let two processes drive
the same CRTC. First used: `SoW.md` §1 and §4.

**drm-lease-manager**: The AGL daemon that creates and hands out DRM leases. The
engine does not depend on it; it only accepts a lease file descriptor. First
used: `SoW.md` §1; build and run notes in `docs/dlm-aarch64.md`.

**DRM master**: The DRM file descriptor that may change modes and planes; a
process drives the display only if it holds it or a lease from it. First
used: `crates/composition-drm/src/lib.rs`; the board consequence is in
`docs/imx95-board.md`.

**DriverProfile**: An opt-in enum in `composition-drm` that adds a driver's
known constraints to what KMS reports. `Generic` adds nothing; `Imx95Dpu` adds
what was read from the `dpu95` source and is marked not measured. First
defined: `crates/composition-drm/src/profile.rs`.

**DSI**: MIPI display serial interface. It appears here as a node in the i.MX95
source reading and in other SoCs' topology notes. First used: `SoW.md`.

**DSS**: The TI AM62P display subsystem that the R5F demo drives. The
repository records only what the SoW and `docs/twoplane-backend.md` say. First
used: `SoW.md` §5.2.

## E

**ExtDst**: An i.MX95 DPU block at the end of the pipeline. The repository
records that the stock Zephyr `socionext,dpu` driver uses `ExtDst0` and that
the twoplane note says `latched_count` follows its sync busy bit; that is a
design statement, not a measurement. First used: `docs/am62p-imx95-matrix.md`,
`docs/platform-capabilities.md`.

## F

**fence (IN_FENCE_FD, OUT_FENCE_PTR)**: DRM synchronisation fences, per plane and
per CRTC. The engine does not use them for M1. First used:
`docs/sync-fences.md`.

**fetch unit (FetchLayer, FetchYUV)**: DPU blocks that read a buffer from
memory; the source reading counts two FetchLayer and four FetchYUV and
allocates them per commit. The repository writes "fetch unit"; the word
"FetchUnit" itself does not occur. First used:
`docs/platform-capabilities.md`.

**FFI**: The C ABI of the engine in `crates/composition-ffi`, the only crate
allowed `unsafe`. Its header is `crates/composition-ffi/include/composition_engine.h`.

**frame**: A complete requested scene for one output: a list of layers; an empty
one blanks the output. It is not a displayed refresh. First defined:
`crates/composition-core/src/types.rs` (`Frame`).

**FrameGen**: The i.MX95 DPU block that generates the display timing; the
vendor sequence sets it up and a "shadow load" is triggered through it. Read
from source only. First used:
[imx95-m7-dpu-driver.md](imx95-m7-dpu-driver.md).

## G

**GO bit**: A register bit of the AM62P path; the twoplane note says
`latched_count` follows it. It is recorded from the demo and not checked on
hardware here. First used: `SoW.md` §5.2 ("single ownership of GO/commit");
`docs/twoplane-backend.md`.

## H

**HVS**: The Raspberry Pi (VC4) hardware video scaler. The repository has source
notes and topology records for it, each with its own status. First used:
`SoW.md`; `docs/observed/vc4-hvs-source.md`.

## I

**i.MX95**: The NXP SoC of the main board target (A55 cores and an M7). First
used: `SoW.md`.

**imx_rproc**: The Linux remoteproc driver for NXP cores, including the i.MX95
M7. The repository reads what it accepts as an ELF and its address map from
source. First used: [imx95-m7-rproc-load.md](imx95-m7-rproc-load.md).

## L

**latch**: The point at which hardware takes a submission and starts showing it
at a frame boundary. The engine reports it as `Presented`; a submission
replaced before then is `Dropped`. Which hardware event stands for it differs
per backend and is not measured here. First used: `SoW.md` §4;
`crates/composition-core/src/presentation.rs`.

**layer**: One placed region of a buffer in a frame. The planner assigns layers
to planes. First defined: `crates/composition-core/src/types.rs` (`Layer`).

**LayerBlend**: A DPU blend stage; the source reading counts six and allocates
them per commit. First used: `docs/platform-capabilities.md`.

**LDB**: The i.MX95 LVDS display bridge. The repository knows it as clocks
(`ldb`, `ldb_vco`) and a device-tree node only. First used:
`docs/imx95-dpu-source-6.6.md`.

**libliftoff**: A library that assigns layers to planes in some compositors. The
repository studies it as a comparison target and a possible cross-check. First
used: `SoW.md`; `docs/wp5-gap-analysis.md`.

**LM (logical machine)**: A System Manager partition of the SoC with
statically configured access rights to hardware and SCMI calls; the question
is whether the M7's LM or Linux's holds the display. First used:
[imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md); the design is in
[imx95-sm-display-lm.md](imx95-sm-display-lm.md).

**LMM**: The System Manager's logical machine manager code, which checks and
forwards SCMI clock and power requests per LM. Read from source only. First
used: [imx95-display-power.md](imx95-display-power.md).

**LVDS**: The panel link of the Verdin iMX95 setup (Toradex 10.1" panel). First
used: `SoW.md`; board facts in `docs/imx95-board.md`.

## M

**M7**: The Cortex-M7 core of the i.MX95, the planned Zephyr target of
milestone M2. Whether it can own the display is an open question. First used:
`SoW.md`; `docs/open-issues.md`.

**Mallow**: The Toradex carrier board of the Verdin iMX95 setup. First used:
[imx95-m7-console.md](imx95-m7-console.md); board rules in
[imx95-board.md](imx95-board.md).

**modifier**: A buffer layout descriptor (for example linear). First defined:
`crates/composition-core/src/types.rs` (`Modifier`).

## O

**output**: The engine's scan-out target (`OutputId`); in DRM terms a CRTC with
its connector. First defined: `crates/composition-core/src/types.rs`.

## P

**PD_DISPLAY**: The i.MX95 power domain of the display subsystem, number 13 in
the System Manager source. Who may switch it is read from source only. First
used: [imx95-display-power.md](imx95-display-power.md).

**plan**: The planner's result for a frame and capabilities: which layer goes to
which plane (`Plan`), or a `PlanError` with reasons. It is valid only for the
frame and capabilities it was made from. First defined:
`crates/composition-core/src/planner.rs`.

**plane**: A hardware scan-out source that a CRTC composes: primary, overlay or
cursor. First used: `SoW.md`; model in `crates/composition-core/src/caps.rs`
(`PlaneCaps`, `PlaneKind`).

**Presented / Dropped / Released**: The engine events for a submission's life: it
reached the screen, it never did, a buffer may be reused. First defined:
`crates/composition-core/src/presentation.rs` (`DisplayEvent`).

## R

**R5F**: The Cortex-R5F core of the AM62P that owns the safety plane in the
existing demo. What enforces the split is not recorded
(`docs/am62p-imx95-matrix.md`). First used: `SoW.md` §5.2.

**release**: The moment a buffer is no longer referenced by the display and may
be written again; reported as `Released`. A successful submission does not
imply it. First used: `SoW.md` §4; `crates/composition-core/src/presentation.rs`.

**remoteproc**: The Linux framework that loads firmware onto a remote core and
starts and stops it. For the i.MX95 M7 the repository has a load script and a
read-only observation. First used: `docs/imx95-zephyr/README.md`; script
`scripts/imx95-m7-rproc.sh`.

**resource group**: A shared hardware resource with a capacity (for example the
DPU's scalers or fetch units) that layers consume by trait. First defined:
`crates/composition-core/src/caps.rs` (`ResourceGroup`).

**resource table**: The ELF section a remoteproc firmware carries to describe
its resources; `imx_rproc` accepts an ELF without one. First used:
[imx95-m7-rproc-load.md](imx95-m7-rproc-load.md).

**rpmsg**: The message transport used with remoteproc cores. The repository
mentions it in the M2 plan and the load script; no rpmsg link has been run
here. First used: `docs/m2-comparison-plan.md`.

## S

**SCMI**: The Arm system control and management interface; on the i.MX95 the
System Manager provides clocks and power domains over it. First used:
`docs/imx95-dpu-source-6.6.md`; version lines in
`docs/imx95-zephyr/observed-2026-11-27.txt`.

**scanout**: Reading a buffer out to the display. The M2 goal is scanout from
Zephyr with VSYNC and latch reporting. First used: `SoW.md`.

**shadow load**: The DPU mechanism that applies staged register values at a
frame boundary; the Zephyr driver's use of it is the candidate latch signal.
Read from source, not measured. First used:
[imx95-m7-dpu-driver.md](imx95-m7-dpu-driver.md); timing in
[imx95-m7-latch-measure.md](imx95-m7-latch-measure.md).

**staticlib**: The `composition-ffi-staticlib` crate, which packages the FFI as a
static library for Zephyr and bare-metal builds. First used: `README.md`.

**submitted / writable / latched / released**: The buffer transitions in the SoW.
In code the states are `Writable`, `Ready`, `Submitted` and `Scanning`, and
`Released` is an event. First used: `SoW.md` §4;
`crates/composition-core/src/presentation.rs`.

**System Manager (SM)**: NXP firmware on the i.MX95 that provides SCMI and whose
configuration would decide which core owns which resource; the repository says
nobody had examined that configuration. First used:
`docs/platform-capabilities.md`.

## T

**TCM**: The M7's tightly coupled memories (ITCM and DTCM), the windows an
ELF segment may load into together with the reserved DDR. First used:
[imx95-m7-rproc-load.md](imx95-m7-rproc-load.md).

**TEST_ONLY**: The DRM atomic commit flag that checks a request without applying
it; the DRM backend uses it to validate. First used: `SoW.md` §4.1;
`crates/composition-drm/src/lib.rs`.

**thumbv7em**: The Rust target family for Cortex-M4/M7 builds; the two CI
cross-builds use it. First used: `docs/ci.md`.

**TRDC**: The bus access-control block that the System Manager programs; the
repository reads it as part of who may touch display registers, and did not
read its programming. First used:
[imx95-display-clocks.md](imx95-display-clocks.md); see also
[imx95-display-handover.md](imx95-display-handover.md).

**twoplane**: The C backend (`backends/twoplane`) for a two-plane display
controller, written for the AM62P model with an i.MX95 note. First used:
`docs/twoplane-backend.md`.

## V

**vblank**: The start of a display refresh, as a DRM event. First used: `SoW.md`.

**vkms**: The virtual KMS driver of the Linux kernel; host DRM tests and CI run
against it, not hardware. First used: `README.md`;
`crates/composition-drm/tests/vkms.rs`; CI in `docs/ci.md`.

**VOP2**: The Rockchip RK3588 display controller, in the topology notes. First
used: `SoW.md`; `docs/observed/rk3588-source.md`.

**VSYNC**: The same refresh boundary as a signal; M2 wants the Zephyr driver to
report it. First used: `SoW.md`; the gap is in `docs/open-issues.md`.

## W

**Weston**: The reference Wayland compositor on the Toradex image; it holds the
display unless a test has it. The repository reads its plane assignment
(`docs/wp5-gap-analysis.md`) and sets board rules about it
(`docs/imx95-board.md`). First used: `SoW.md`.

## Not defined: terms the repository does not use

Unclear in the repository: whether "output" always equals a CRTC, and what
enforces the AM62P's R5F/A53 split.
