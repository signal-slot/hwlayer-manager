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

**Advice**: The result of the advisory planner (`composition_core::Advice`): per
layer, whether it sits on a plane or is composed, and why not. It is not a
`Plan` and has no way to commit. Defined in
[advisory-planner-design.md](advisory-planner-design.md); implemented
stateless in `crates/composition-core/src/advise.rs` (read from source,
2026-11-28).

**advisory planner (advise / ce_advise)**: The proposed question "which layers can
be promoted to planes, and what blocks the rest?", for a compositor that
composes the remainder itself. Rust form `composition_core::advise`, C form
`ce_advise` in `crates/composition-ffi`; an `Engine::advise` form does not
exist. Design and status:
[advisory-planner-design.md](advisory-planner-design.md); the gap it answers is
in [wp5-gap-analysis.md](wp5-gap-analysis.md).

**AFBC**: A compressed buffer modifier (Arm Frame Buffer Compression). The
repository mentions it only in the RK3588 notes; the engine's own model is
`FormatModifier`. On the NanoPC-T6, Cluster planes list AFBC modifiers and
Esmart planes list linear only (observed 2026-11-28); the source reading is in
[rk3588-vop2-formats-source.md](rk3588-vop2-formats-source.md). First used:
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

**Blocker**: The reason a layer is not placed on a plane in `Advice` (for
example a static plane limit, a clip, or an exhausted search budget). First
defined: [advisory-planner-design.md](advisory-planner-design.md); the type is
`composition_core::Blocker`.

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

**Cluster window**: A VOP2 window class on the RK3588 (Cluster0 to Cluster3) that
takes AFBC buffers and 10-bit RGB and can rotate them; the source gives it 4x
scaling. Read from source (2026-11-28) in
[rk3588-mainline-source.md](rk3588-mainline-source.md) and
[rk3588-vendor-vop2-source.md](rk3588-vendor-vop2-source.md); design use in
[rk3588-backend-design.md](rk3588-backend-design.md) §5.

**CM4**: The Raspberry Pi Compute Module 4 (BCM2711), not a Cortex-M4. First
used: `README.md`; its probe is in `docs/observed/bcm2711-cm4.md`.

**coexistence (WP5-07)**: The case where a compositor owns one connector while a
leased output on another is driven by a separate client. It is open issue
WP5-07 and gap W5-10; this glossary records the term only, not a result. See
[wp5-gap-analysis.md](wp5-gap-analysis.md) and the survey of idle boards in
[wp5-coexistence-survey.md](wp5-coexistence-survey.md) (not the trial).

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

**DCO**: The Developer Certificate of Origin, a sign-off line by which a human
certifies the right to submit a change. The repository records that Weston
requires a human DCO sign-off and restricts AI-authored contributions (read
from source at a pinned commit); this repository's `CONTRIBUTING.md` has no
sign-off text yet. Source record: [wp5-gap-analysis.md](wp5-gap-analysis.md)
(W5-07; WP5-05 in [open-issues.md](open-issues.md)).

**DeviceLost**: The event that the backend lost its display resources (reset,
lease revoked); everything pending is dropped and all buffers are released.
First defined: `crates/composition-core/src/presentation.rs`.

**display list (dlist)**: The list of per-plane words in HVS memory from which the
HVS composes one output channel; a plane costs a few words and one dlist memory
serves all channels. Read from source (2026-11-28) in
[vc4-hvs-dlist-source.md](vc4-hvs-dlist-source.md); design use in
[hvs-backend-design.md](hvs-backend-design.md) §3.

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

**DM firmware**: The TI Device Manager firmware that runs on the AM62P's wakeup-domain R5F (the "DM R5F") and answers Linux's and other cores' requests for clocks, power and resource ownership. Per the TI AM62Px TRM and TI's public system firmware documentation, not observed here. The M3 work names it because a remote-core DSS owner depends on what it grants; see `docs/am62p-imx95-matrix.md` and `docs/open-issues.md` (M3).

**DSI**: MIPI display serial interface. It appears here as a node in the i.MX95
source reading and in other SoCs' topology notes. First used: `SoW.md`.

**DSS**: The TI AM62P display subsystem that the R5F demo drives. The
repository records only what the SoW and `docs/twoplane-backend.md` say. First
used: `SoW.md` §5.2.

## E

**Esmart window**: A VOP2 window class on the RK3588 (Esmart0 to Esmart3) that
reads linear buffers and NV formats and has the 8x scaler; rotation is
`REFLECT_Y` only. Read from source (2026-11-28) in
[rk3588-vop2-scale-source.md](rk3588-vop2-scale-source.md) and
[rk3588-vendor-vop2-source.md](rk3588-vendor-vop2-source.md). The repository
docs do not use a separate "Smart window" term.

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

**firmware KMS (fkms)**: The older Raspberry Pi display stack
(`vc4-fkms-v3d`) in which the VideoCore firmware owns the display hardware. The
HVS design takes it as out of scope and assumes full KMS (the kernel drives the
HVS itself); see [hvs-backend-design.md](hvs-backend-design.md) §7 and
[vc4-mainline-source.md](vc4-mainline-source.md).

**FrameGen**: The i.MX95 DPU block that generates the display timing; the
vendor sequence sets it up and a "shadow load" is triggered through it. Read
from source only. First used:
[imx95-m7-dpu-driver.md](imx95-m7-dpu-driver.md).

**firmware_class search path**: Where the Linux firmware loader looks for the file named in a remoteproc `firmware-name` (default `/lib/firmware`; an extra path can be set through the `firmware_class.path` kernel parameter or `/sys/module/firmware_class/parameters/path`), per kernel documentation. M3 plans to install the R5F image there; the repository has no AM62P observation of it yet. Compare `remoteproc`.

## G

**GO bit**: A register bit of the AM62P path; the twoplane note says
`latched_count` follows it. It is recorded from the demo and not checked on
hardware here. First used: `SoW.md` §5.2 ("single ownership of GO/commit");
`docs/twoplane-backend.md`.

## H

**HVS**: The Raspberry Pi (VC4) hardware video scaler. The repository has source
notes and topology records for it, each with its own status. First used:
`SoW.md`; `docs/observed/vc4-hvs-source.md`. See also `HVS4 / HVS5 / HVS6`.

**HVS4 / HVS5 / HVS6**: The HVS generations: HVS4 is BCM2835 to 2837 and out of
scope; HVS5 is the BCM2711 (reTerminal CM4); HVS6 is the BCM2712 (Raspberry Pi 5).
They differ in registers, LBM/UPM pools and formats, per
[hvs-backend-design.md](hvs-backend-design.md) §5, read from source (2026-11-28)
in [vc4-mainline-source.md](vc4-mainline-source.md).

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

**LBM**: The HVS line buffer memory, a pool shared by all channels; planes that
scale vertically (HVS5) or scale or are YUV (HVS6) need a part of it. Read from
source (2026-11-28) in
[vc4-hvs-lbm-upm-source.md](vc4-hvs-lbm-upm-source.md).

**LDB**: The i.MX95 LVDS display bridge. The repository knows it as clocks
(`ldb`, `ldb_vco`) and a device-tree node only. First used:
`docs/imx95-dpu-source-6.6.md`.

**libdlmclient**: The client library of AGL's `drm-lease-manager`; its API is
`dlm_get_lease`, `dlm_lease_fd` and `dlm_release_lease`, over a Unix
`SOCK_SEQPACKET` socket per lease name. Read from source and built for aarch64
as a static archive, not run on a board: [dlm-aarch64.md](dlm-aarch64.md).

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

**MCU R5F / DM R5F**: The AM62P has an R5F in the MCU domain, which the M3 board test targets and which is "the R5F" in this glossary, and a separate R5F in the wakeup domain that runs the DM firmware, per the TI AM62Px TRM. They are different cores with different roles; see `DM firmware` and `R5F`. See `docs/am62p-imx95-matrix.md`.

**meta-agl-drm-lease**: The AGL layer whose Weston `bbappend` patches an
upstream-based Weston 13.0.x to take a lease from `drm-lease-manager` through
`libdlmclient` (`--drm-lease=<name>`). It is a lease client route, not
`wp_drm_lease_device_v1`; read from source at tag `unagi_20.92.0`, not run
here: [weston-imx-lease.md](weston-imx-lease.md) (S7 to S9).

**modifier**: A buffer layout descriptor (for example linear). First defined:
`crates/composition-core/src/types.rs` (`Modifier`).

**multi-area / win1**: On the RK3588, `win1` is the second sub window of a Cluster
window; the vendor 6.1 driver makes it a plane only with the DT property
`support-multi-area`, and Esmart windows carry a multi-area feature flag. The
mainline table has eight windows and no `win1` entry. Read from source
(2026-11-28): [rk3588-vendor-vop2-source.md](rk3588-vendor-vop2-source.md),
[rk3588-mainline-source.md](rk3588-mainline-source.md); whether the board shows
them is open question Q8 in [rk3588-backend-design.md](rk3588-backend-design.md).

## O

**OLDI**: The TI LVDS-style panel transmitter the AM62P DSS video ports can feed, per the TI AM62Px TRM; the repository has not observed it. M3 names it as the possible panel link of the Verdin AM62P setup. See `docs/open-issues.md` (M3).

**OSTree deployment**: One bootable, versioned root filesystem tree managed by OSTree, which Torizon OS uses for atomic updates and rollback, per Toradex documentation. The M3 board rule (return the board to Torizon) means booting the deployment again. See `Torizon OS`.

**output**: The engine's scan-out target (`OutputId`); in DRM terms a CRTC with
its connector. First defined: `crates/composition-core/src/types.rs`.

**overlay manager**: The DSS block that combines the layers of one video port by z-order and blending, per the TI AM62Px TRM. The twoplane backend's two-plane model corresponds to it only as a design statement in `docs/twoplane-backend.md`; no register of it is read here.

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

**Placement::Compose**: The placement an advice gives a layer that is not put on
a plane but drawn by the caller into one composition buffer; the composed
layers become one synthetic layer. Proposed in
[advisory-planner-design.md](advisory-planner-design.md); the C value is
`CE_PLACE_COMPOSE`.

**port_mux / layer select**: The VOP2 registers that choose which video port
each window feeds and in what mixer order. The mainline driver reads the old
layer select register to find a port's start mixer. Read from source
(2026-11-28): [rk3588-mainline-source.md](rk3588-mainline-source.md),
[rk3588-dt-source.md](rk3588-dt-source.md).

**PPF**: A scaling filter mode of the vc4 HVS (the other is TPZ); the driver
picks it for upscales, for downscales up to about 1.5x and for multi-plane (YUV)
formats. Read from source (2026-11-28) in
[vc4-hvs-scaling-source.md](vc4-hvs-scaling-source.md) and
[vc4-mainline-source.md](vc4-mainline-source.md).

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

**reproducible demo (WP5-08)**: The SoW §8 WP5 deliverable of a demo that a
reviewer can rebuild and run from a clean checkout with pinned versions. Open
issue WP5-08; the plan, with nothing measured, is
[wp5-evidence-plan.md](wp5-evidence-plan.md); the cross-build recipe is
[dlm-aarch64.md](dlm-aarch64.md).

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
it; the DRM backend uses it to validate. The plane-count and scale-limit
sweeps of the RK3588 and HVS designs use it because it changes nothing on the
board (`docs/rk3588-backend-design.md` §7 Q4, `docs/hvs-backend-design.md` §7
Q1); on the HVS, memory-bandwidth underruns are not caught by it. First used:
`SoW.md` §4.1; `crates/composition-drm/src/lib.rs`.

**thumbv7em**: The Rust target family for Cortex-M4/M7 builds; the two CI
cross-builds use it. First used: `docs/ci.md`.

**TPZ**: The trapezoidal scaling filter of the vc4 HVS, used for steeper
downscales; its horizontal ratio field is 21 bits wide and the driver rejects
ratios above it. Read from source (2026-11-28) in
[vc4-hvs-scaling-source.md](vc4-hvs-scaling-source.md) and
[vc4-mainline-source.md](vc4-mainline-source.md).

**TRDC**: The bus access-control block that the System Manager programs; the
repository reads it as part of who may touch display registers, and did not
read its programming. First used:
[imx95-display-clocks.md](imx95-display-clocks.md); see also
[imx95-display-handover.md](imx95-display-handover.md).

**twoplane**: The C backend (`backends/twoplane`) for a two-plane display
controller, written for the AM62P model with an i.MX95 note. First used:
`docs/twoplane-backend.md`.

**Torizon OS**: Toradex's OSTree-based embedded Linux for its modules, including the Verdin AM62P, per Toradex documentation. In this repository it is the boot chain a board must be returned to after any other boot (rule recorded in `docs/board-scenarios.md`). The i.MX95 board runs the NXP BSP instead (`docs/imx95-board.md`).

**trace buffer**: The log memory a remoteproc firmware declares in its resource table; Linux exposes it as `trace0` under the remoteproc debugfs directory, per kernel documentation. M3 reads the R5F test's console from it over ssh because the UART may be busy; the AM62P run has not happened yet. Compare `resource table`.

## U

**UPM**: The HVS6 (BCM2712) pool of unified buffer memory, one handle per colour
plane. Read from source (2026-11-28) in
[vc4-hvs-lbm-upm-source.md](vc4-hvs-lbm-upm-source.md).

## V

**vblank**: The start of a display refresh, as a DRM event. First used: `SoW.md`.

**Verdin**: Toradex's system-on-module family. The repository's i.MX95 setup is a Verdin iMX95 on a Mallow carrier (`docs/imx95-board.md`); the M3 target is the Verdin AM62P. First used: [imx95-m7-verdin-bsp.md](imx95-m7-verdin-bsp.md).

**VID / VIDL**: The DSS input pipelines that fetch a buffer for a plane, per the TI AM62Px TRM; "VIDL" is the lite variant with fewer features. The twoplane note treats the AM62P as two planes; which pipeline each one is, is not recorded. First used: `docs/twoplane-backend.md`.

**video port (VP)**: A DSS output that composes the overlay manager's result and drives one display interface with timing and vblank, per the TI AM62Px TRM. It plays the role of the engine's `output` on the AM62P; the mapping is not observed. See `docs/am62p-imx95-matrix.md`. The RK3588 VOP2 has the same kind of output, also called a video port (VP); the NanoPC-T6 registers two as CRTCs (observed 2026-11-28, [rk3588-backend-design.md](rk3588-backend-design.md) §2).

**vkms**: The virtual KMS driver of the Linux kernel; host DRM tests and CI run
against it, not hardware. First used: `README.md`;
`crates/composition-drm/tests/vkms.rs`; CI in `docs/ci.md`.

**VOP2**: The Rockchip RK3588 display controller, in the topology notes: four
video ports, four Cluster and four Esmart windows. Read from source
(2026-11-28) in [rk3588-mainline-source.md](rk3588-mainline-source.md). First
used: `SoW.md`; `docs/observed/rk3588-source.md`.

**VSYNC**: The same refresh boundary as a signal; M2 wants the Zephyr driver to
report it. First used: `SoW.md`; the gap is in `docs/open-issues.md`.

## W

**Weston**: The reference Wayland compositor on the Toradex image; it holds the
display unless a test has it. The repository reads its plane assignment
(`docs/wp5-gap-analysis.md`) and sets board rules about it
(`docs/imx95-board.md`). First used: `SoW.md`.

**wp_drm_lease_device_v1**: The Wayland DRM lease protocol (`drm-lease-v1`
staging) by which a compositor leases connectors to clients. The repository
records that Weston at the pinned commit has no such code and that the AGL
route does not use it; the protocol itself is recalled, not read. See
[wp5-gap-analysis.md](wp5-gap-analysis.md) and
[weston-imx-lease.md](weston-imx-lease.md).

## Not defined: terms the repository does not use

Unclear in the repository: whether "output" always equals a CRTC, and what
enforces the AM62P's R5F/A53 split.
